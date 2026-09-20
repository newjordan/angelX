//! Continual harness — Prime Agent-inspired durable, editable agent state.
//!
//! Port of the high-value slice of [Prime Agent](https://github.com/newjordan/prime-agent)'s
//! continual harness / `/refine` subsystem into angel0:
//!
//! * **Kinds:** `prompt` (supplemental notes only — base system prompt stays
//!   immutable), `memory` (durable facts), `skill` (reusable procedure
//!   descriptions for angel skill/tool routing), `subagent` (delegation specs
//!   for `spawn` / agent graphs).
//! * **Scopes:** `project` (repo-keyed under `~/.angel0/continual-harness/`) and
//!   `global` (`~/.angel0/continual-harness/global.json`).
//! * **Edits:** create / update / delete with recorded refinement events and
//!   snapshot-based rollback.
//! * **Injection:** compact overview into the system prompt and headless
//!   `--task` warm-start.
//!
//! This is **not** an IPython RLM kernel. Angel's execution surface stays tool
//! and shell based; harness entries are routing/context artifacts the agent (and
//! the `/refine` command) can grow from evidence.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA: u32 = 1;
const GLOBAL_FILE: &str = "global.json";

pub(crate) const HARNESS_BLOCK_HEADER: &str = "[continual harness — supplemental state]";
pub(crate) const HARNESS_BLOCK_SENTINEL: &str = "[/continual harness]";

const DEFAULT_MAX_ENTRIES_PER_KIND: usize = 6;
const DEFAULT_MAX_REFINEMENTS: usize = 5;
const DEFAULT_MAX_CONTENT: usize = 180;
const DEFAULT_MAX_BLOCK_BYTES: usize = 3 * 1024;
const MAX_TITLE: usize = 120;
const MAX_CONTENT: usize = 4 * 1024;
const MAX_ENTRIES_TOTAL: usize = 64;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub(crate) enum EntryKind {
    Prompt,
    Memory,
    Skill,
    Subagent,
}

impl EntryKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Memory => "memory",
            Self::Skill => "skill",
            Self::Subagent => "subagent",
        }
    }

    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "prompt" | "prompts" => Some(Self::Prompt),
            "memory" | "memories" => Some(Self::Memory),
            "skill" | "skills" => Some(Self::Skill),
            "subagent" | "subagents" | "agent" => Some(Self::Subagent),
            _ => None,
        }
    }

    fn all() -> [Self; 4] {
        [Self::Prompt, Self::Memory, Self::Skill, Self::Subagent]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Scope {
    Project,
    Global,
}

impl Scope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Global => "global",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "project" | "local" | "repo" => Some(Self::Project),
            "global" | "user" => Some(Self::Global),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HarnessEntry {
    pub id: String,
    pub kind: EntryKind,
    pub title: String,
    pub content: String,
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default = "default_scope_project")]
    pub scope: Scope,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default = "default_version")]
    pub version: u32,
}

fn default_path() -> String {
    "general".into()
}
fn default_scope_project() -> Scope {
    Scope::Project
}
fn default_version() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RefinementEvent {
    pub id: String,
    pub trigger: String,
    pub changes: Vec<String>,
    #[serde(default)]
    pub evidence: String,
    #[serde(default)]
    pub outcome: String,
    pub created_at: String,
    /// Full before/after snapshots for rollback (JSON-encoded entries).
    #[serde(default)]
    pub snapshots: Vec<EditSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EditSnapshot {
    pub action: String,
    pub kind: EntryKind,
    pub id: String,
    #[serde(default)]
    pub before: Option<HarnessEntry>,
    #[serde(default)]
    pub after: Option<HarnessEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HarnessState {
    #[serde(default = "default_schema")]
    pub schema: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, HarnessEntry>,
    #[serde(default)]
    pub refinements: Vec<RefinementEvent>,
}

fn default_schema() -> u32 {
    SCHEMA
}

impl HarnessState {
    fn empty() -> Self {
        Self {
            schema: SCHEMA,
            entries: BTreeMap::new(),
            refinements: Vec::new(),
        }
    }

    fn entry_count(&self) -> usize {
        self.entries.len()
    }

    fn by_kind(&self, kind: EntryKind) -> Vec<&HarnessEntry> {
        let mut out: Vec<&HarnessEntry> =
            self.entries.values().filter(|e| e.kind == kind).collect();
        out.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then(a.title.cmp(&b.title))
                .then(a.id.cmp(&b.id))
        });
        out
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RefinementEdit {
    pub action: String, // create | update | delete
    pub kind: EntryKind,
    pub id: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub path: Option<String>,
    /// Operator/tool free-text; retained for API symmetry with Prime edits.
    #[allow(dead_code)]
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Paths + load/save
// ---------------------------------------------------------------------------

fn root_dir() -> PathBuf {
    if let Ok(p) = std::env::var("ANGEL_CONTINUAL_HARNESS_DIR")
        && !p.trim().is_empty()
    {
        return PathBuf::from(p);
    }
    crate::workspace_store::angel_subdir("continual-harness")
}

fn project_state_path(workspace: &Path) -> PathBuf {
    let identity = crate::workspace_store::repo_identity(workspace);
    crate::workspace_store::workspace_json_path_in(&root_dir(), &identity.root)
}

fn global_state_path() -> PathBuf {
    root_dir().join(GLOBAL_FILE)
}

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Compact UTC-ish stamp; fine for local ledger ordering.
    format!("{secs}")
}

fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("r{nanos:x}")
}

fn slug(raw: &str, fallback: &str) -> String {
    let normalized: String = raw
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let trimmed = normalized.trim_matches('_');
    let cut: String = trimmed.chars().take(80).collect();
    if cut.is_empty() {
        fallback.to_string()
    } else {
        cut
    }
}

fn load_state(path: &Path) -> HarnessState {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HarnessState::empty();
    };
    match serde_json::from_str::<HarnessState>(&raw) {
        Ok(mut state) if !raw.trim().is_empty() => {
            if state.schema == 0 {
                state.schema = SCHEMA;
            }
            // Drop garbage entries that would poison prompt injection.
            state.entries.retain(|_, e| {
                !e.id.is_empty()
                    && !e.title.is_empty()
                    && e.title.len() <= MAX_TITLE
                    && e.content.len() <= MAX_CONTENT
            });
            state
        }
        _ => HarnessState::empty(),
    }
}

fn save_state(path: &Path, state: &HarnessState) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir continual-harness: {e}"))?;
    }
    let bytes = crate::secrets::to_redacted_vec_pretty(state)
        .map_err(|e| format!("encode harness: {e}"))?;
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("write harness: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("commit harness: {e}"))
}

pub(crate) fn load_project(workspace: &Path) -> HarnessState {
    load_state(&project_state_path(workspace))
}

pub(crate) fn load_global() -> HarnessState {
    load_state(&global_state_path())
}

/// Merge already-loaded global + project stores. Project ids win on collision
/// by taking a `project:` display prefix only when both scopes share an id.
fn merge_states(global: HarnessState, project: HarnessState) -> HarnessState {
    let mut merged = HarnessState::empty();
    merged.schema = SCHEMA.max(global.schema).max(project.schema);
    for (id, mut entry) in global.entries {
        entry.scope = Scope::Global;
        merged.entries.insert(id, entry);
    }
    for (id, mut entry) in project.entries {
        entry.scope = Scope::Project;
        let key = if merged.entries.contains_key(&id) {
            format!("project:{id}")
        } else {
            id
        };
        merged.entries.insert(key, entry);
    }
    merged.refinements = global.refinements;
    merged.refinements.extend(project.refinements);
    merged
}

/// Merge global + project for prompt injection. Project ids win on collision
/// by taking a `project:` display prefix only when both scopes share an id.
pub(crate) fn load_merged(workspace: &Path) -> HarnessState {
    merge_states(load_global(), load_project(workspace))
}

// ---------------------------------------------------------------------------
// Apply edits
// ---------------------------------------------------------------------------

fn validate_edit(edit: &RefinementEdit, id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("entry id is empty".into());
    }
    match edit.action.as_str() {
        "create" | "update" => {
            let title = edit.title.as_deref().unwrap_or("").trim();
            let content = edit.content.as_deref().unwrap_or("").trim();
            if title.is_empty() {
                return Err(format!("{} requires title", edit.action));
            }
            if title.len() > MAX_TITLE {
                return Err(format!("title exceeds {MAX_TITLE} bytes"));
            }
            if content.is_empty() {
                return Err(format!("{} requires content", edit.action));
            }
            if content.len() > MAX_CONTENT {
                return Err(format!("content exceeds {MAX_CONTENT} bytes"));
            }
        }
        "delete" => {}
        other => {
            return Err(format!(
                "unknown action {other:?} (use create|update|delete)"
            ));
        }
    }
    Ok(())
}

/// Apply a batch of edits to one scope store. Returns a human summary.
pub(crate) fn apply_edits(
    workspace: &Path,
    scope: Scope,
    trigger: &str,
    evidence: &str,
    outcome: &str,
    edits: &[RefinementEdit],
) -> Result<String, String> {
    if edits.is_empty() {
        return Err("no edits".into());
    }
    let path = match scope {
        Scope::Project => project_state_path(workspace),
        Scope::Global => global_state_path(),
    };
    let mut state = load_state(&path);
    let event_id = new_id();
    let mut snapshots = Vec::new();
    let mut changes = Vec::new();
    let mut applied = 0usize;
    let mut failures = Vec::new();

    for edit in edits {
        let id = edit
            .id
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                slug(
                    edit.title.as_deref().unwrap_or(edit.kind.as_str()),
                    edit.kind.as_str(),
                )
            });
        if let Err(e) = validate_edit(edit, &id) {
            failures.push(format!("{id}: {e}"));
            continue;
        }
        let before = state.entries.get(&id).cloned();
        match edit.action.as_str() {
            "delete" => {
                if before.is_none() {
                    failures.push(format!("{id}: entry not found"));
                    continue;
                }
                state.entries.remove(&id);
                snapshots.push(EditSnapshot {
                    action: "delete".into(),
                    kind: edit.kind,
                    id: id.clone(),
                    before,
                    after: None,
                });
                changes.push(format!("delete {}:{}", edit.kind.as_str(), id));
                applied += 1;
            }
            "create" => {
                if before.is_some() {
                    failures.push(format!("{id}: already exists (use update)"));
                    continue;
                }
                if state.entry_count() >= MAX_ENTRIES_TOTAL {
                    failures.push(format!("{id}: harness full ({MAX_ENTRIES_TOTAL} entries)"));
                    continue;
                }
                let now = now_iso();
                let after = HarnessEntry {
                    id: id.clone(),
                    kind: edit.kind,
                    title: edit.title.clone().unwrap_or_default().trim().to_string(),
                    content: edit.content.clone().unwrap_or_default().trim().to_string(),
                    path: edit
                        .path
                        .clone()
                        .filter(|p| !p.trim().is_empty())
                        .unwrap_or_else(|| "general".into()),
                    scope,
                    source: "refine".into(),
                    created_at: now.clone(),
                    updated_at: now,
                    version: 1,
                };
                state.entries.insert(id.clone(), after.clone());
                snapshots.push(EditSnapshot {
                    action: "create".into(),
                    kind: edit.kind,
                    id: id.clone(),
                    before: None,
                    after: Some(after),
                });
                changes.push(format!("create {}:{}", edit.kind.as_str(), id));
                applied += 1;
            }
            "update" => {
                let Some(prev) = before.clone() else {
                    failures.push(format!("{id}: entry not found (use create)"));
                    continue;
                };
                let now = now_iso();
                let after = HarnessEntry {
                    id: id.clone(),
                    kind: edit.kind,
                    title: edit
                        .title
                        .clone()
                        .filter(|t| !t.trim().is_empty())
                        .unwrap_or(prev.title.clone()),
                    content: edit
                        .content
                        .clone()
                        .filter(|c| !c.trim().is_empty())
                        .unwrap_or(prev.content.clone()),
                    path: edit
                        .path
                        .clone()
                        .filter(|p| !p.trim().is_empty())
                        .unwrap_or(prev.path.clone()),
                    scope: prev.scope,
                    source: "refine".into(),
                    created_at: prev.created_at.clone(),
                    updated_at: now,
                    version: prev.version.saturating_add(1),
                };
                state.entries.insert(id.clone(), after.clone());
                snapshots.push(EditSnapshot {
                    action: "update".into(),
                    kind: edit.kind,
                    id: id.clone(),
                    before: Some(prev),
                    after: Some(after),
                });
                changes.push(format!("update {}:{}", edit.kind.as_str(), id));
                applied += 1;
            }
            _ => failures.push(format!("{id}: bad action")),
        }
    }

    if applied == 0 {
        return Err(format!(
            "no edits applied{}",
            if failures.is_empty() {
                String::new()
            } else {
                format!(" — {}", failures.join("; "))
            }
        ));
    }

    state.refinements.push(RefinementEvent {
        id: event_id.clone(),
        trigger: compact_text(trigger, 240),
        changes: changes.clone(),
        evidence: compact_text(evidence, 480),
        outcome: compact_text(outcome, 240),
        created_at: now_iso(),
        snapshots,
    });
    // Cap history growth.
    if state.refinements.len() > 64 {
        let drop_n = state.refinements.len() - 64;
        state.refinements.drain(0..drop_n);
    }
    save_state(&path, &state)?;

    let mut msg = format!(
        "refine · {scope} · {applied} edit(s) · id={event_id}\n{}",
        changes
            .iter()
            .map(|c| format!("  · {c}"))
            .collect::<Vec<_>>()
            .join("\n"),
        scope = scope.as_str(),
    );
    if !failures.is_empty() {
        msg.push_str(&format!("\nskipped: {}", failures.join("; ")));
    }
    Ok(msg)
}

/// Roll back one recorded refinement on its original scope store.
pub(crate) fn rollback(workspace: &Path, scope: Scope, event_id: &str) -> Result<String, String> {
    let path = match scope {
        Scope::Project => project_state_path(workspace),
        Scope::Global => global_state_path(),
    };
    let mut state = load_state(&path);
    let Some(event) = state.refinements.iter().find(|e| e.id == event_id).cloned() else {
        return Err(format!(
            "refinement {event_id} not found in {} store",
            scope.as_str()
        ));
    };
    if event.snapshots.is_empty() {
        return Err(format!("refinement {event_id} has no snapshots to reverse"));
    }

    for snap in event.snapshots.iter().rev() {
        match snap.action.as_str() {
            "create" => {
                state.entries.remove(&snap.id);
            }
            "delete" => {
                if let Some(before) = &snap.before {
                    state.entries.insert(snap.id.clone(), before.clone());
                }
            }
            "update" => {
                if let Some(before) = &snap.before {
                    state.entries.insert(snap.id.clone(), before.clone());
                } else {
                    state.entries.remove(&snap.id);
                }
            }
            _ => {}
        }
    }
    state.refinements.push(RefinementEvent {
        id: new_id(),
        trigger: format!("rollback of {event_id}"),
        changes: vec![format!("rollback {event_id}")],
        evidence: event.trigger.clone(),
        outcome: "restored prior entry snapshots".into(),
        created_at: now_iso(),
        snapshots: Vec::new(),
    });
    save_state(&path, &state)?;
    Ok(format!(
        "rolled back {event_id} on {} store · {} reverse step(s)",
        scope.as_str(),
        event.snapshots.len()
    ))
}

// ---------------------------------------------------------------------------
// Prompt formatting
// ---------------------------------------------------------------------------

fn compact_text(text: &str, max: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max {
        return normalized;
    }
    let cut: String = normalized.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

fn enabled() -> bool {
    crate::harness::env_flag("ANGEL_CONTINUAL_HARNESS", true)
}

/// Compact overview for system prompt / task warm-start. Empty when disabled
/// or no entries exist.
pub(crate) fn context_block(workspace: &Path) -> String {
    if !enabled() {
        return String::new();
    }
    let state = load_merged(workspace);
    if state.entries.is_empty() {
        return String::new();
    }
    let max_per = env_usize(
        "ANGEL_CONTINUAL_HARNESS_MAX_PER_KIND",
        DEFAULT_MAX_ENTRIES_PER_KIND,
    );
    let max_ref = env_usize(
        "ANGEL_CONTINUAL_HARNESS_MAX_REFINEMENTS",
        DEFAULT_MAX_REFINEMENTS,
    );
    let max_content = env_usize("ANGEL_CONTINUAL_HARNESS_MAX_CONTENT", DEFAULT_MAX_CONTENT);
    let max_bytes = env_usize("ANGEL_CONTINUAL_HARNESS_MAX_BYTES", DEFAULT_MAX_BLOCK_BYTES);

    let mut lines = vec![
        HARNESS_BLOCK_HEADER.to_string(),
        "Supplemental continual-harness state from prior refinements. Base system prompt is immutable; treat these as routing hints and durable lessons. Prefer the `continual_harness` tool (or `/refine`) for small evidence-backed create/update/delete edits — never rewrite the whole store.".to_string(),
        String::new(),
    ];

    for kind in EntryKind::all() {
        let entries = state.by_kind(kind);
        if entries.is_empty() {
            continue;
        }
        lines.push(format!("{}: {}", kind.as_str(), entries.len()));
        for entry in entries.iter().take(max_per) {
            lines.push(format!(
                "- [{}:{}] {} ({} v{}): {}",
                entry.scope.as_str(),
                entry.id,
                entry.title,
                entry.path,
                entry.version,
                compact_text(&entry.content, max_content),
            ));
        }
        if entries.len() > max_per {
            lines.push(format!(
                "- +{} more {} entries",
                entries.len() - max_per,
                kind.as_str()
            ));
        }
        lines.push(String::new());
    }

    if !state.refinements.is_empty() {
        lines.push(format!("recent refinements: {}", state.refinements.len()));
        for event in state.refinements.iter().rev().take(max_ref) {
            let changes = if event.changes.is_empty() {
                "no applied edits".into()
            } else {
                event.changes.join(", ")
            };
            lines.push(format!(
                "- [{}] {}: {}",
                event.id,
                compact_text(&event.trigger, max_content),
                compact_text(&changes, max_content),
            ));
        }
    }

    lines.push(HARNESS_BLOCK_SENTINEL.to_string());
    let mut block = lines.join("\n");
    if block.len() > max_bytes {
        // Keep header + sentinel; truncate middle.
        let head = HARNESS_BLOCK_HEADER;
        let tail = HARNESS_BLOCK_SENTINEL;
        let budget = max_bytes
            .saturating_sub(head.len() + tail.len() + 16)
            .max(64);
        let mid: String = block.chars().skip(head.len()).take(budget).collect();
        block = format!("{head}\n{mid}…\n{tail}");
    }
    format!("\n{block}\n")
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
        .max(1)
}

// ---------------------------------------------------------------------------
// /refine CLI
// ---------------------------------------------------------------------------

/// `/refine` surface — status, CRUD helpers, and rollback.
pub(crate) fn run(arg: Option<&str>, workspace: &Path) -> String {
    if !enabled() {
        return "continual harness disabled (ANGEL_CONTINUAL_HARNESS=0)".into();
    }
    let raw = arg.unwrap_or("").trim();
    if raw.is_empty() || raw == "status" || raw == "list" {
        return status_text(workspace);
    }
    let mut parts = raw.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("").to_ascii_lowercase();
    let rest = parts.next().unwrap_or("").trim();

    match cmd.as_str() {
        "status" | "list" => status_text(workspace),
        "help" | "?" => usage().into(),
        "add" | "create" => parse_and_add(workspace, rest, "create"),
        "update" | "set" => parse_and_add(workspace, rest, "update"),
        "del" | "delete" | "rm" => parse_and_delete(workspace, rest),
        "rollback" => parse_and_rollback(workspace, rest),
        "seed-light" => seed_light_prover(workspace),
        _ => format!("unknown /refine subcommand {cmd:?}\n{}", usage()),
    }
}

fn usage() -> &'static str {
    "usage: /refine [status|list]\n\
     /refine add <kind> <id> <title> — <content>   kinds: prompt|memory|skill|subagent\n\
     /refine update <kind> <id> <title> — <content>\n\
     /refine del <id> [--global]\n\
     /refine rollback <event-id> [--global]\n\
     /refine seed-light   · project memories for the Lighter prover challenge\n\
     Optional leading `--global` on add/update/del/rollback targets the user store."
}

fn parse_scope_prefix(rest: &str) -> (Scope, &str) {
    let rest = rest.trim();
    if let Some(r) = rest.strip_prefix("--global") {
        (Scope::Global, r.trim())
    } else if let Some(r) = rest.strip_prefix("--project") {
        (Scope::Project, r.trim())
    } else {
        (Scope::Project, rest)
    }
}

fn parse_and_add(workspace: &Path, rest: &str, action: &str) -> String {
    let (scope, rest) = parse_scope_prefix(rest);
    // kind id title — content
    let Some((meta, content)) = rest.split_once("—").or_else(|| rest.split_once(" - ")) else {
        return format!("need `title — content`\n{}", usage());
    };
    let mut bits = meta.split_whitespace();
    let kind_s = bits.next().unwrap_or("");
    let id = bits.next().unwrap_or("").to_string();
    let title = bits.collect::<Vec<_>>().join(" ");
    let Some(kind) = EntryKind::parse(kind_s) else {
        return format!("unknown kind {kind_s:?} (prompt|memory|skill|subagent)");
    };
    if id.is_empty() || title.is_empty() {
        return format!("need kind, id, and title\n{}", usage());
    }
    let edit = RefinementEdit {
        action: action.into(),
        kind,
        id: Some(id),
        title: Some(title),
        content: Some(content.trim().to_string()),
        path: Some("general".into()),
        reason: Some(format!("/refine {action}")),
    };
    apply_edits(
        workspace,
        scope,
        &format!("/refine {action}"),
        "operator slash command",
        "entry written",
        &[edit],
    )
    .unwrap_or_else(|e| format!("/refine: {e}"))
}

fn parse_and_delete(workspace: &Path, rest: &str) -> String {
    let (scope, rest) = parse_scope_prefix(rest);
    let id = rest.split_whitespace().next().unwrap_or("").to_string();
    if id.is_empty() {
        return format!("need entry id\n{}", usage());
    }
    // Kind is required by the type but delete only needs id — look up from store.
    let state = match scope {
        Scope::Project => load_project(workspace),
        Scope::Global => load_global(),
    };
    let kind = state
        .entries
        .get(&id)
        .map(|e| e.kind)
        .unwrap_or(EntryKind::Memory);
    let edit = RefinementEdit {
        action: "delete".into(),
        kind,
        id: Some(id),
        title: None,
        content: None,
        path: None,
        reason: Some("/refine del".into()),
    };
    apply_edits(
        workspace,
        scope,
        "/refine del",
        "operator slash command",
        "entry removed",
        &[edit],
    )
    .unwrap_or_else(|e| format!("/refine: {e}"))
}

fn parse_and_rollback(workspace: &Path, rest: &str) -> String {
    let (scope, rest) = parse_scope_prefix(rest);
    let id = rest.split_whitespace().next().unwrap_or("");
    if id.is_empty() {
        return format!("need refinement event id\n{}", usage());
    }
    rollback(workspace, scope, id).unwrap_or_else(|e| format!("/refine: {e}"))
}

fn status_text(workspace: &Path) -> String {
    let project = load_project(workspace);
    let global = load_global();
    let project_count = project.entry_count();
    let global_count = global.entry_count();
    let merged = merge_states(global, project);
    let mut out = format!(
        "continual harness · project {} · global {} · merged {}\n\
         project store: {}\n\
         global store:  {}\n",
        project_count,
        global_count,
        merged.entry_count(),
        project_state_path(workspace).display(),
        global_state_path().display(),
    );
    if merged.entries.is_empty() {
        out.push_str("no entries — /refine add memory <id> <title> — <content>\n");
        out.push_str("or: /refine seed-light  (Lighter prover challenge lessons)\n");
        return out;
    }
    for kind in EntryKind::all() {
        let entries = merged.by_kind(kind);
        if entries.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{} ({})\n", kind.as_str(), entries.len()));
        for e in entries {
            out.push_str(&format!(
                "  [{}:{}] {} — {}\n",
                e.scope.as_str(),
                e.id,
                e.title,
                compact_text(&e.content, 100)
            ));
        }
    }
    if !merged.refinements.is_empty() {
        out.push_str("\nrecent refinements\n");
        for event in merged.refinements.iter().rev().take(8) {
            out.push_str(&format!(
                "  [{}] {} · {}\n",
                event.id,
                compact_text(&event.trigger, 60),
                event.changes.join(", ")
            ));
        }
    }
    out
}

/// Project-scoped lessons for the lighter-prover benchmark workspace.
/// Safe no-op when entries already exist with the same ids.
pub(crate) fn seed_light_prover(workspace: &Path) -> String {
    let seeds: &[(&str, EntryKind, &str, &str)] = &[
        (
            "editable_surface",
            EntryKind::Memory,
            "Editable surface",
            "Candidates may edit root Cargo.toml/Cargo.lock, circuit/, bench/, and vendor/. \
benchmark-tools/, fixtures, scripts, workflows, and benchmark.json are protected. \
Do not patch trusted verifier wrappers.",
        ),
        (
            "score_authority",
            EntryKind::Memory,
            "Score authority",
            "Only the trusted CPU verifier writes a score. The checked-in all-empty witness is \
public-synthetic-provisional smoke — never compare it with private-active-ranked scores.",
        ),
        (
            "bench_entry",
            EntryKind::Memory,
            "Bench entrypoints",
            "bench crate builds the candidate prove worker. Prefer cargo build -p bench --release \
and the repo's benchmark.sh / BENCHMARK.md flow over inventing new harnesses.",
        ),
        (
            "seatbelt_note",
            EntryKind::Memory,
            "Process containment",
            "Ranked mode runs the worker under Seatbelt-style containment with cleared env and \
no network. Local edits should keep the prove binary self-contained.",
        ),
        (
            "prove_loop",
            EntryKind::Skill,
            "Prove-verify loop",
            "Procedure: (1) read BENCHMARK.md + bench/ and circuit/ layout, (2) make the smallest \
change in editable paths, (3) cargo build -p bench --release, (4) run the documented smoke/bench \
command, (5) read diagnostics and iterate. Never claim ranked score from synthetic smoke.",
        ),
        (
            "kernel_focus",
            EntryKind::Subagent,
            "Kernel/path specialist",
            "When spawning help: assign a child to profile one hot path (witness build, \
constraint eval, or proof post-process) with a narrow file set; parent integrates diffs serially.",
        ),
        (
            "no_telemetry_creds",
            EntryKind::Prompt,
            "Telemetry caution",
            "This challenge may upload agent transcripts via yukon hooks. Do not read or paste \
secrets, home env files, or unrelated private repos into the session.",
        ),
    ];

    let mut edits = Vec::new();
    let existing = load_project(workspace);
    for (id, kind, title, content) in seeds {
        if existing.entries.contains_key(*id) {
            continue;
        }
        edits.push(RefinementEdit {
            action: "create".into(),
            kind: *kind,
            id: Some((*id).into()),
            title: Some((*title).into()),
            content: Some((*content).into()),
            path: Some("lighter-prover".into()),
            reason: Some("seed-light".into()),
        });
    }
    if edits.is_empty() {
        return "seed-light: project harness already has the light-prover entries".into();
    }
    apply_edits(
        workspace,
        Scope::Project,
        "seed-light",
        "Prime-style continual harness bootstrap for Lighter prover challenge",
        "agent starts with challenge-correct boundaries",
        &edits,
    )
    .unwrap_or_else(|e| format!("seed-light: {e}"))
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

pub(crate) struct ContinualHarnessTool {
    workspace: PathBuf,
}

impl ContinualHarnessTool {
    pub(crate) fn new(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
        }
    }
}

impl crate::harness::Tool for ContinualHarnessTool {
    fn name(&self) -> &str {
        "continual_harness"
    }

    fn def(&self) -> crate::club::ToolDef {
        crate::club::ToolDef {
            name: "continual_harness".into(),
            description: "Continual harness (Prime-style): durable supplemental prompt notes, \
memories, skill procedures, and subagent specs that survive turns. Prefer small evidence-backed \
edits after a repeated failure, reusable tactic, or durable preference. action=list (default), \
create, update, delete, rollback, seed_light. Scope project (default) or global."
                .into(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "create", "update", "delete", "rollback", "seed_light"],
                        "description": "default list"
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["project", "global"],
                        "description": "default project"
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["prompt", "memory", "skill", "subagent"],
                        "description": "required for create/update"
                    },
                    "id": { "type": "string", "description": "entry id (or refinement id for rollback)" },
                    "title": { "type": "string" },
                    "content": { "type": "string" },
                    "path": { "type": "string", "description": "grouping path, default general" },
                    "evidence": { "type": "string", "description": "why this edit is justified" },
                    "outcome": { "type": "string", "description": "expected improvement" },
                },
                "required": [],
            }),
        }
    }

    fn call(&self, args: &serde_json::Value) -> Result<String, String> {
        if !enabled() {
            return Err("continual harness disabled (ANGEL_CONTINUAL_HARNESS=0)".into());
        }
        let action = args["action"].as_str().unwrap_or("list");
        let scope = args["scope"]
            .as_str()
            .and_then(Scope::parse)
            .unwrap_or(Scope::Project);

        match action {
            "list" => Ok(status_text(&self.workspace)),
            "seed_light" => Ok(seed_light_prover(&self.workspace)),
            "create" | "update" => {
                let kind = args["kind"]
                    .as_str()
                    .and_then(EntryKind::parse)
                    .ok_or("missing or invalid 'kind' (prompt|memory|skill|subagent)")?;
                let id = args["id"].as_str().map(str::trim).filter(|s| !s.is_empty());
                let title = args["title"]
                    .as_str()
                    .ok_or("missing 'title'")?
                    .trim()
                    .to_string();
                let content = args["content"]
                    .as_str()
                    .ok_or("missing 'content'")?
                    .trim()
                    .to_string();
                let path = args["path"].as_str().map(|s| s.to_string());
                let evidence = args["evidence"].as_str().unwrap_or("agent tool edit");
                let outcome = args["outcome"].as_str().unwrap_or("harness updated");
                let edit = RefinementEdit {
                    action: action.into(),
                    kind,
                    id: id.map(str::to_string),
                    title: Some(title),
                    content: Some(content),
                    path,
                    reason: Some(evidence.into()),
                };
                apply_edits(
                    &self.workspace,
                    scope,
                    &format!("tool:{action}"),
                    evidence,
                    outcome,
                    &[edit],
                )
            }
            "delete" => {
                let id = args["id"]
                    .as_str()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or("missing 'id'")?
                    .to_string();
                let state = match scope {
                    Scope::Project => load_project(&self.workspace),
                    Scope::Global => load_global(),
                };
                let kind = state
                    .entries
                    .get(&id)
                    .map(|e| e.kind)
                    .ok_or_else(|| format!("entry {id} not found"))?;
                let evidence = args["evidence"].as_str().unwrap_or("agent tool delete");
                let edit = RefinementEdit {
                    action: "delete".into(),
                    kind,
                    id: Some(id),
                    title: None,
                    content: None,
                    path: None,
                    reason: Some(evidence.into()),
                };
                apply_edits(
                    &self.workspace,
                    scope,
                    "tool:delete",
                    evidence,
                    "entry removed",
                    &[edit],
                )
            }
            "rollback" => {
                let id = args["id"]
                    .as_str()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or("missing refinement 'id'")?;
                rollback(&self.workspace, scope, id)
            }
            other => Err(format!(
                "unknown action {other:?} (list|create|update|delete|rollback|seed_light)"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "../../tests/cockpit/app/continual_harness__tests.rs"]
mod tests;
