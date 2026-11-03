//! Pinned non-deferred core tool definitions; schema authority remains native.
use crate::agent::club::ToolDef;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    sync::{Arc, OnceLock},
};

const EMBEDDED: &str = include_str!("../../../../lib/tools/definitions.json");
const LIMIT: u64 = 1024 * 1024;
// A version-one override published before these baseline-only entries existed
// remains complete for its original generation; they are filled natively.
const V1_ADDITIONS: &[&str] = &[
    "rl_campaign",
    "benchmark_compare",
    "jev_decide",
    "knowledge_graph",
    "get_context_remaining",
    "handle_read",
    "delegate",
    "integrate",
    "recall",
    "swarm_compile",
];
const V1_NEW_ADDITIONS: &[&str] = &[
    "lsp_diagnostics",
    "lsp_symbols",
    "lsp_workspace_symbol",
    "ui_inspect",
    "ui_verify",
    "lsp_definition",
    "lsp_references",
    "lsp_hover",
    "atlas",
];
const V1_DYNAMIC_ADDITIONS: &[&str] = &[
    "grep",
    "defs",
    "todo",
    "handoff",
    "semantic_read",
    "science_search",
    "repo_search",
];
const V1_CAMPAIGN_ADDITIONS: &[&str] = &[
    "labyrinth_campaign_spec",
    "loop_research",
    "labyrinth_campaign",
];
// These entries are native book or internal schema, not free-form guidance.
const NATIVE_DESCRIPTIONS: &[&str] = &["labyrinth_campaign_spec", "labyrinth_campaign"];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    description: String,
    params: Value,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Generation {
    version: u32,
    tools: BTreeMap<String, Definition>,
}

fn baseline() -> Arc<Generation> {
    static BASE: OnceLock<Arc<Generation>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(serde_json::from_str(EMBEDDED).expect("valid core tool definitions"))
    }))
}

pub(crate) fn baseline_def(name: &str) -> ToolDef {
    let base = baseline();
    let entry = base.tools.get(name).expect("known core tool definition");
    ToolDef {
        name: name.to_string(),
        description: entry.description.clone(),
        params: entry.params.clone(),
    }
}

fn read_override(path: &Path, base: &Generation) -> Result<Generation, String> {
    if !path.is_absolute() {
        return Err("tool catalog override must be an absolute path".into());
    }
    let read = || -> Result<Vec<u8>, String> {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path).map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > LIMIT {
            return Err("tool catalog is not a bounded regular file".into());
        }
        let mut bytes = Vec::new();
        file.take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > LIMIT {
            return Err("tool catalog exceeds byte limit".into());
        }
        Ok(bytes)
    };
    let bytes = read()?;
    let mut candidate: Generation =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid tool catalog: {e}"))?;
    if bytes != read()? {
        return Err("tool catalog changed during capture".into());
    }
    let complete = candidate.tools.len() == base.tools.len();
    let absent = |names: &[&str]| {
        names
            .iter()
            .all(|name| !candidate.tools.contains_key(*name))
    };
    let legacy_57 = candidate.tools.len() + V1_CAMPAIGN_ADDITIONS.len() == base.tools.len()
        && absent(V1_CAMPAIGN_ADDITIONS);
    let legacy_50 =
        candidate.tools.len() + V1_DYNAMIC_ADDITIONS.len() + V1_CAMPAIGN_ADDITIONS.len()
            == base.tools.len()
            && absent(V1_DYNAMIC_ADDITIONS)
            && absent(V1_CAMPAIGN_ADDITIONS);
    let legacy_41 = candidate.tools.len()
        + V1_NEW_ADDITIONS.len()
        + V1_DYNAMIC_ADDITIONS.len()
        + V1_CAMPAIGN_ADDITIONS.len()
        == base.tools.len()
        && absent(V1_NEW_ADDITIONS)
        && absent(V1_DYNAMIC_ADDITIONS)
        && absent(V1_CAMPAIGN_ADDITIONS);
    let legacy_31 = candidate.tools.len()
        + V1_ADDITIONS.len()
        + V1_NEW_ADDITIONS.len()
        + V1_DYNAMIC_ADDITIONS.len()
        + V1_CAMPAIGN_ADDITIONS.len()
        == base.tools.len()
        && absent(V1_ADDITIONS)
        && absent(V1_NEW_ADDITIONS)
        && absent(V1_DYNAMIC_ADDITIONS)
        && absent(V1_CAMPAIGN_ADDITIONS);
    let legacy = legacy_31 || legacy_41 || legacy_50 || legacy_57;
    if candidate.version != base.version
        || !(complete || legacy)
        || candidate.tools.iter().any(|(name, def)| {
            def.description.trim().is_empty()
                || base.tools.get(name).is_none_or(|old| {
                    old.params != def.params
                        || def.description.len() > old.description.len()
                        || (NATIVE_DESCRIPTIONS.contains(&name.as_str())
                            && def.description != old.description)
                })
        })
    {
        return Err(
            "tool catalog must preserve all names and native schemas with bounded descriptions"
                .into(),
        );
    }
    if legacy {
        for name in V1_ADDITIONS
            .iter()
            .chain(V1_NEW_ADDITIONS)
            .chain(V1_DYNAMIC_ADDITIONS)
            .chain(V1_CAMPAIGN_ADDITIONS)
        {
            candidate
                .tools
                .entry((*name).to_string())
                .or_insert_with(|| base.tools[*name].clone());
        }
    }
    Ok(candidate)
}

fn capture(path: Option<&Path>) -> Arc<Generation> {
    let base = baseline();
    let Some(path) = path else {
        return base;
    };
    match read_override(path, &base) {
        Ok(next)
            if next.tools.iter().all(|(name, def)| {
                base.tools
                    .get(name)
                    .is_some_and(|original| def.description == original.description)
            }) =>
        {
            base
        }
        Ok(next) => Arc::new(next),
        Err(error) => {
            eprintln!("[angel] tool catalog override ignored: {error}");
            base
        }
    }
}

pub(crate) fn snapshot() -> Arc<Generation> {
    let path = std::env::var_os("ANGEL_CORE_TOOL_CATALOG").filter(|v| !v.is_empty());
    capture(path.as_deref().map(Path::new))
}

/// Bind one generation to the turn-local book Arc. The book's transient field
/// travels with existing code-mode/seat propagation and is excluded from bytes.
pub(crate) fn bind_book(
    mut book: Arc<super::book::catalog::Catalog>,
) -> Arc<super::book::catalog::Catalog> {
    if book.tool_generation.is_none() {
        let generation = current().unwrap_or_else(snapshot);
        Arc::make_mut(&mut book).tool_generation = Some(generation);
    }
    book
}
pub(crate) fn current() -> Option<Arc<Generation>> {
    super::book::catalog::current()?.tool_generation.clone()
}

// Only these schema fields are native constants applied by their Tool::def.
// Normalization permits description reload after a future native limit change,
// while every other part of the schema must remain byte-for-byte equivalent.
fn schema_matches(name: &str, baseline: &Value, native: &Value) -> bool {
    if baseline == native {
        return true;
    }
    // The nested campaign specification is always supplied by the native host.
    if name == "loop_research" {
        let mut normalized = native.clone();
        let Some(old) = baseline
            .pointer("/properties/campaign_spec")
            .filter(|v| v.is_object())
        else {
            return false;
        };
        let Some(value) = normalized
            .pointer_mut("/properties/campaign_spec")
            .filter(|v| v.is_object())
        else {
            return false;
        };
        *value = old.clone();
        return *baseline == normalized;
    }
    let fields: &[&str] = match name {
        "grep" => &[
            "/properties/paths/maxItems",
            "/properties/skip_files/maxItems",
            "/properties/context/maximum",
        ],
        "defs" => &["/properties/names/maxItems"],
        "todo" => &[
            "/properties/text/maxLength",
            "/properties/items/maxItems",
            "/properties/items/items/maxLength",
        ],
        "handoff" => &["/properties/note/minLength", "/properties/note/maxLength"],
        "semantic_read" => &[
            "/properties/paths/maxItems",
            "/properties/max_results/maximum",
        ],
        "science_search" | "repo_search" => &["/properties/query/maxLength"],
        _ => return false,
    };
    let mut normalized = native.clone();
    for field in fields {
        let Some(old) = baseline
            .pointer(field)
            .filter(|value| value.as_u64().is_some())
        else {
            return false;
        };
        let Some(value) = normalized
            .pointer_mut(field)
            .filter(|value| value.as_u64().is_some())
        else {
            return false;
        };
        *value = old.clone();
    }
    *baseline == normalized
}

impl Generation {
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub(crate) fn is_baseline(&self) -> bool {
        std::ptr::eq(self, baseline().as_ref())
    }

    /// Leave lean or otherwise dynamic definitions in native control.
    pub(crate) fn overlay(&self, definition: &mut ToolDef) {
        if NATIVE_DESCRIPTIONS.contains(&definition.name.as_str())
            || !self.contains(&definition.name)
        {
            return;
        }
        let Some(changed) = self.tools.get(&definition.name) else {
            return;
        };
        let base = baseline();
        let original = &base.tools[&definition.name];
        if definition.description == original.description
            && schema_matches(&definition.name, &original.params, &definition.params)
        {
            definition.description.clone_from(&changed.description);
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/core_catalog__tests.rs"]
mod tests;
