//! Persistent memory store for the cockpit. Memories are short facts the user
//! asks Angel to remember; they survive restarts (JSON on disk) and are injected
//! into every agent turn (see `App::submit`) so the model actually *uses* them.
//!
//! Backed by a canonical-project-keyed record under `~/.angelX/memories/`
//! (override with `ANGEL_MEMORY_FILE` in tests). Legacy global arrays and
//! mismatched project bindings are ignored rather than injected.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MEMORY_SCHEMA: &str = "angel-project-memories/v1";
pub(crate) const MAX_MEMORY_RECORD_BYTES: usize = 256 * 1024;
pub(crate) const MAX_MEMORY_ITEMS: usize = 128;
pub(crate) const MAX_MEMORY_ITEM_BYTES: usize = 4096;
pub(crate) const MAX_MEMORY_TOTAL_BYTES: usize = 28 * 1024;
const MAX_MEMORY_CONTEXT_BYTES: usize = 32 * 1024;

#[derive(Serialize, Deserialize)]
struct MemoryRecord {
    schema: String,
    workspace: PathBuf,
    project_key: String,
    memories: Vec<String>,
}

fn explicit_memory_file() -> Option<PathBuf> {
    std::env::var_os("ANGEL_MEMORY_FILE")
        .filter(|path| !path.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
}

fn store_path_for(workspace: &Path) -> PathBuf {
    if let Some(path) = explicit_memory_file() {
        return path;
    }
    let identity = crate::platform::workspace_store::repo_identity(workspace);
    crate::platform::workspace_store::workspace_json_path_in(
        &crate::platform::workspace_store::angel_subdir("memories"),
        &identity.root,
    )
}

/// Load the saved memories (empty if the file is absent or unreadable).
pub fn load_for(workspace: &Path) -> Vec<String> {
    let path = store_path_for(workspace);
    let Some(raw) = read_bounded_utf8(&path, MAX_MEMORY_RECORD_BYTES) else {
        return Vec::new();
    };
    let Ok(record) = serde_json::from_str::<MemoryRecord>(&raw) else {
        return Vec::new();
    };
    if record.schema != MEMORY_SCHEMA
        || !crate::platform::workspace_store::matches_project(
            workspace,
            &record.workspace,
            &record.project_key,
        )
    {
        return Vec::new();
    }
    if memories_within_limits(&record.memories) {
        record.memories
    } else {
        Vec::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MemorySaveError {
    InvalidMemories,
    UnsupportedWorkspacePath(PathBuf),
    ExistingStore(String),
    Serialize(String),
    Persist(String),
}

impl std::fmt::Display for MemorySaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMemories => write!(
                f,
                "memory values exceed the item/count/byte limits or contain an empty item"
            ),
            Self::UnsupportedWorkspacePath(path) => write!(
                f,
                "memory JSON does not support repository paths containing invalid UTF-8: {path:?}; use a UTF-8 repository path"
            ),
            Self::ExistingStore(error) => {
                write!(f, "existing memory record will not be overwritten: {error}")
            }
            Self::Serialize(error) => write!(f, "serialize memory record: {error}"),
            Self::Persist(error) => write!(f, "memory durability is unconfirmed: {error}"),
        }
    }
}

/// Save a validated project record. A failure is explicit; callers must not
/// acknowledge a memory mutation until this returns success. UTF-8 schemas and
/// keys remain unchanged; unsupported raw roots are never migrated by guessing.
pub fn save_for<S: AsRef<str>>(memories: &[S], workspace: &Path) -> Result<(), MemorySaveError> {
    if !memories_within_limits(memories) {
        return Err(MemorySaveError::InvalidMemories);
    }
    let identity = crate::platform::workspace_store::repo_identity(workspace);
    if identity.root.to_str().is_none() {
        return Err(MemorySaveError::UnsupportedWorkspacePath(identity.root));
    }
    let record = MemoryRecord {
        schema: MEMORY_SCHEMA.to_string(),
        workspace: identity.root,
        project_key: identity.key,
        memories: memories
            .iter()
            .map(|memory| memory.as_ref().to_string())
            .collect(),
    };
    let path = store_path_for(workspace);
    if path
        .try_exists()
        .map_err(|error| MemorySaveError::ExistingStore(error.to_string()))?
    {
        let existing_matches = read_bounded_utf8(&path, MAX_MEMORY_RECORD_BYTES)
            .and_then(|raw| serde_json::from_str::<MemoryRecord>(&raw).ok())
            .is_some_and(|record| {
                record.schema == MEMORY_SCHEMA
                    && crate::platform::workspace_store::matches_project(
                        workspace,
                        &record.workspace,
                        &record.project_key,
                    )
            });
        if !existing_matches {
            return Err(MemorySaveError::ExistingStore(format!(
                "{} is unreadable, malformed, oversized, or belongs to another project",
                path.display()
            )));
        }
    }
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| MemorySaveError::Serialize(error.to_string()))?;
    if bytes.len() > MAX_MEMORY_RECORD_BYTES {
        return Err(MemorySaveError::InvalidMemories);
    }
    crate::platform::workspace_store::write_private_atomic(&path, &bytes)
        .map_err(MemorySaveError::Persist)
}

/// Opening line of the injected memory block, and its closing sentinel. Kept as
/// constants so the submit path can strip a prior block as a precise span (header
/// line … sentinel line) and keep only the freshest copy per turn.
pub(crate) const MEMORY_BLOCK_HEADER: &str = "[memory — persistent facts to honor]";
pub(crate) const MEMORY_BLOCK_SENTINEL: &str = "[/memory]";

/// Render memories as bounded Harness-role turn context. Wrapped in
/// [`MEMORY_BLOCK_HEADER`] … [`MEMORY_BLOCK_SENTINEL`] so legacy copies remain
/// attributable and strippable.
pub fn context_block<S: AsRef<str>>(memories: &[S]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let mut out = format!("{MEMORY_BLOCK_HEADER}\n");
    let mut included = 0usize;
    for memory in memories.iter().take(MAX_MEMORY_ITEMS) {
        let memory = memory.as_ref();
        if memory.is_empty() || memory.len() > MAX_MEMORY_ITEM_BYTES {
            continue;
        }
        let encoded =
            serde_json::to_string(memory).unwrap_or_else(|_| "\"<invalid memory>\"".to_string());
        let line = format!("- {encoded}\n");
        if out.len() + line.len() + MEMORY_BLOCK_SENTINEL.len() + 96 > MAX_MEMORY_CONTEXT_BYTES {
            break;
        }
        out.push_str(&line);
        included += 1;
    }
    let omitted = memories.len().saturating_sub(included);
    if omitted > 0 {
        out.push_str(&format!(
            "[harness omitted {omitted} memory item(s) outside the bounded context]\n"
        ));
    }
    out.push_str(MEMORY_BLOCK_SENTINEL);
    out.push_str("\n\n");
    out
}

pub(crate) fn validate_add<S: AsRef<str>>(memories: &[S], candidate: &str) -> Result<(), String> {
    if candidate.len() > MAX_MEMORY_ITEM_BYTES {
        return Err(format!(
            "memory is {} bytes; maximum is {MAX_MEMORY_ITEM_BYTES}",
            candidate.len()
        ));
    }
    if memories.len() >= MAX_MEMORY_ITEMS {
        return Err(format!(
            "memory store is full at {MAX_MEMORY_ITEMS} items; forget one before adding another"
        ));
    }
    let total = memories
        .iter()
        .fold(0usize, |total, memory| {
            total.saturating_add(memory.as_ref().len())
        })
        .saturating_add(candidate.len());
    if total > MAX_MEMORY_TOTAL_BYTES {
        return Err(format!(
            "memory store would exceed its {MAX_MEMORY_TOTAL_BYTES}-byte context budget; \
             forget an item before adding another"
        ));
    }
    Ok(())
}

fn memories_within_limits<S: AsRef<str>>(memories: &[S]) -> bool {
    memories.len() <= MAX_MEMORY_ITEMS
        && memories.iter().all(|memory| {
            let memory = memory.as_ref();
            !memory.is_empty() && memory.len() <= MAX_MEMORY_ITEM_BYTES
        })
        && memories.iter().fold(0usize, |total, memory| {
            total.saturating_add(memory.as_ref().len())
        }) <= MAX_MEMORY_TOTAL_BYTES
}

fn read_bounded_utf8(path: &Path, max_bytes: usize) -> Option<String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = Path::new(path.file_name()?);
    let bytes = crate::agent::harness::confined_read_limited(parent, name, max_bytes)
        .ok()
        .flatten()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/memory__save_error_tests.rs"]
mod save_error_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/memory__tests.rs"]
mod tests;

pub(crate) mod store;
