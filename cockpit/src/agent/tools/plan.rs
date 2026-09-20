//! Planning / scratchpad tools: `todo` (the agent's own in-registry plan, Claude
//! Code's TodoWrite pattern) and `notes` (a durable project-bound note store).
//! Both keep state behind an `Arc<Mutex>` so it persists across turns within a
//! session.

use crate::agent::club::ToolDef;
use crate::agent::harness::Tool;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Todo tool — the agent's own persisted plan/scratchpad (Claude Code TodoWrite
// pattern). State lives behind an Arc<Mutex>, and since the cockpit builds one
// ToolRegistry per session, the list persists across turns. Keeps long-horizon
// software work coherent.
// ---------------------------------------------------------------------------

pub(crate) const TODO_STATE_PREFIX: &str = "[todo-state/v1] ";
const TODO_MAX_ITEMS: usize = 64;
const TODO_TEXT_MAX_CHARS: usize = 500;

#[derive(Clone, serde::Serialize)]
pub(crate) struct TodoItem {
    id: usize,
    text: String,
    done: bool,
}

#[derive(Default)]
pub(crate) struct TodoState {
    items: Vec<TodoItem>,
    next_id: usize,
}

pub(crate) struct TodoTool {
    state: Arc<Mutex<TodoState>>,
}
impl TodoTool {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TodoState::default())),
        }
    }
}

fn render_todos(state: &TodoState) -> String {
    let machine = serde_json::json!({
        "next_id": state.next_id,
        "items": state.items,
    });
    format!("{TODO_STATE_PREFIX}{machine}")
}

fn todo_text(raw: &str) -> Result<Option<String>, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > TODO_TEXT_MAX_CHARS {
        return Err(format!(
            "todo text exceeds {TODO_TEXT_MAX_CHARS} characters"
        ));
    }
    Ok(Some(text.to_string()))
}

impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "todo".to_string(),
            description: "Your persistent task list for multi-step work. action=list (default) \
                          returns its canonical JSON state; add {text} appends; complete {id} \
                          checks one off; set {items:[...]} replaces the whole list."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "add", "complete", "set"], "description": "default 'list'" },
                    "text": { "type": "string", "maxLength": TODO_TEXT_MAX_CHARS, "description": "for action=add" },
                    "id": { "type": "integer", "description": "for action=complete" },
                    "items": { "type": "array", "maxItems": TODO_MAX_ITEMS, "items": { "type": "string", "maxLength": TODO_TEXT_MAX_CHARS }, "description": "for action=set" },
                },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let action = args["action"].as_str().unwrap_or("list");
        let mut st = self.state.lock().map_err(|_| "todo state poisoned")?;
        match action {
            "list" => Ok(render_todos(&st)),
            "add" => {
                let text = todo_text(args["text"].as_str().ok_or("missing 'text'")?)?
                    .ok_or("'text' must not be empty")?;
                st.next_id += 1;
                let id = st.next_id;
                st.items.push(TodoItem {
                    id,
                    text,
                    done: false,
                });
                Ok(format!("added #{id}\n{}", render_todos(&st)))
            }
            "complete" => {
                let id = args["id"].as_u64().ok_or("missing 'id'")? as usize;
                let found = st
                    .items
                    .iter_mut()
                    .find(|t| t.id == id)
                    .map(|t| t.done = true)
                    .is_some();
                if found {
                    Ok(format!("completed #{id}\n{}", render_todos(&st)))
                } else {
                    Err(format!("no todo #{id}"))
                }
            }
            "set" => {
                let arr = args["items"].as_array().ok_or("missing 'items' array")?;
                if arr.len() > TODO_MAX_ITEMS {
                    return Err(format!("'items' exceeds {TODO_MAX_ITEMS} entries"));
                }
                // Validate the complete replacement before touching live state:
                // one malformed later item must not leave a partial new plan.
                let texts = arr
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or_else(|| "each item must be a string".to_string())
                            .and_then(todo_text)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                st.items.clear();
                st.next_id = 0;
                for text in texts {
                    st.next_id += 1;
                    let id = st.next_id;
                    st.items.push(TodoItem {
                        id,
                        text,
                        done: false,
                    });
                }
                Ok(format!(
                    "set {} todo(s)\n{}",
                    st.items.len(),
                    render_todos(&st)
                ))
            }
            other => Err(format!(
                "unknown action {other:?} (use list|add|complete|set)"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Handoff tool — the agent's own compaction-proof continuity note. The todo
// list carries *steps*; this carries the *narrative* an agent would need to
// resume cold (goal, done, in-flight, next actions, load-bearing facts). The
// latest successful result is protected from tool-result aging and the
// compactor re-emits its body across compaction in Assistant role — the
// agent's prose never gains System authority.
// ---------------------------------------------------------------------------

/// Canonical success-result prefix. Detection depends on it being the very
/// first bytes of the result, so nothing may be prepended.
pub(crate) const HANDOFF_STATE_PREFIX: &str = "[handoff-note/v1]\n";
pub(crate) const HANDOFF_NOTE_MAX_CHARS: usize = 6_000;
/// A resume brief shorter than this cannot name a goal, state, and next step;
/// reject it at write time rather than discover the shortcut in the next
/// session. ("No shortcuts" is the operator's explicit contract here.)
pub(crate) const HANDOFF_NOTE_MIN_CHARS: usize = 200;

const HANDOFF_SCHEMA: &str = "angel-handoff/v1";

/// Age past which a loaded handoff carries a staleness banner. Fast-moving
/// competition facts (board frontier, scores, in-flight slots, branch tips)
/// rot in hours; a warm start must not present a days-old brief in the present
/// tense. 2026-09-01 toymaker: a 12-day-old brief pinned the lower-track
/// frontier at 53.13 while the live board stood at 67.67 — the loop spent the
/// morning working to beat a frontier its own account had already passed.
const HANDOFF_STALE_AFTER_SECS: u64 = 6 * 60 * 60;

fn handoff_age_compact(secs: u64) -> String {
    if secs < 3_600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3_600)
    } else {
        format!("{}d", secs / 86_400)
    }
}

/// Staleness banner for a persisted note, or `None` while it is fresh.
fn stale_handoff_banner(written_unix: u64) -> Option<String> {
    if written_unix == 0 {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let age = now.saturating_sub(written_unix);
    (age >= HANDOFF_STALE_AFTER_SECS).then(|| {
        format!(
            "[handoff is {} old — STALE: treat every live fact inside (board \
             frontier, scores, in-flight slots, branch tips) as EXPIRED until \
             re-verified against the live source]\n",
            handoff_age_compact(age)
        )
    })
}

#[derive(serde::Serialize, serde::Deserialize)]
struct HandoffRecord {
    schema: String,
    workspace: PathBuf,
    project_key: String,
    written_unix: u64,
    note: String,
}

fn handoff_path(workspace: &std::path::Path) -> PathBuf {
    if let Some(path) = std::env::var_os("ANGEL_HANDOFF_FILE").filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    let identity = crate::platform::workspace_store::repo_identity(workspace);
    crate::platform::workspace_store::workspace_json_path_in(
        &crate::platform::workspace_store::angel_subdir("handoff"),
        &identity.root,
    )
}

/// The persisted handoff for `workspace`, identity-validated — the fresh-
/// session warm-start reads it without constructing a registry. A stale note
/// arrives wearing its age banner so the successor re-verifies live facts
/// instead of resuming inside a frozen world.
pub(crate) fn load_workspace_handoff(workspace: &std::path::Path) -> Option<String> {
    let (note, written_unix) = HandoffTool::new(workspace).load_disk()?;
    Some(match stale_handoff_banner(written_unix) {
        Some(banner) => format!("{banner}{note}"),
        None => note,
    })
}

pub(crate) struct HandoffTool {
    state: Arc<Mutex<Option<String>>>,
    path: PathBuf,
    workspace: PathBuf,
    project_key: String,
}
impl HandoffTool {
    pub(crate) fn new(workspace: &std::path::Path) -> Self {
        let identity = crate::platform::workspace_store::repo_identity(workspace);
        Self {
            state: Arc::new(Mutex::new(None)),
            path: handoff_path(workspace),
            workspace: identity.root,
            project_key: identity.key,
        }
    }

    /// The persisted note for this workspace plus its write stamp,
    /// identity-validated. Used for the cold-session `show` path and by the
    /// harness warm start; callers turn the stamp into a staleness banner.
    fn load_disk(&self) -> Option<(String, u64)> {
        let raw = std::fs::read_to_string(&self.path).ok()?;
        let record = serde_json::from_str::<HandoffRecord>(&raw).ok()?;
        (record.schema == HANDOFF_SCHEMA
            && record.workspace == self.workspace
            && record.project_key == self.project_key
            && !record.note.trim().is_empty())
        .then_some((record.note, record.written_unix))
    }

    fn persist(&self, note: &str) -> Result<(), String> {
        let record = HandoffRecord {
            schema: HANDOFF_SCHEMA.to_string(),
            workspace: self.workspace.clone(),
            project_key: self.project_key.clone(),
            written_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            note: note.to_string(),
        };
        let bytes =
            serde_json::to_vec_pretty(&record).map_err(|e| format!("encode handoff: {e}"))?;
        crate::platform::workspace_store::write_private_atomic(&self.path, &bytes)
            .map_err(|error| format!("persist handoff: {error}"))
    }
}

impl Tool for HandoffTool {
    fn name(&self) -> &str {
        "handoff"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "handoff".to_string(),
            description: "Your compaction-proof, session-proof handoff note. Write the state a \
                          cold successor needs to resume this work: current goal, what is done \
                          (with evidence), what is in flight, exact next steps in order, and \
                          load-bearing facts (paths, hashes, commands, decisions with reasons). \
                          The newest note replaces the previous one, is carried verbatim across \
                          context compaction, and persists on disk so a FRESH session's \
                          action=show recovers it. Refresh it before long or risky stretches and \
                          before ending a run. action=show (default when no note is given) \
                          returns the current note; write {note} replaces it."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["show", "write"], "description": "default: 'write' when 'note' is present, else 'show'" },
                    "note": { "type": "string", "minLength": HANDOFF_NOTE_MIN_CHARS, "maxLength": HANDOFF_NOTE_MAX_CHARS, "description": "for action=write — the complete replacement resume brief" },
                },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let mut state = self.state.lock().map_err(|_| "handoff state poisoned")?;
        let action = args["action"]
            .as_str()
            .unwrap_or(if args["note"].is_string() {
                "write"
            } else {
                "show"
            });
        match action {
            "show" => {
                if let Some(note) = state.as_ref() {
                    return Ok(format!("{HANDOFF_STATE_PREFIX}{note}"));
                }
                match self.load_disk() {
                    // Cold-session pickup: the previous run's persisted brief.
                    // A stale note wears its age so the successor re-verifies
                    // board/score/slot facts before acting on them.
                    Some((note, written_unix)) => {
                        *state = Some(note.clone());
                        let banner = stale_handoff_banner(written_unix).unwrap_or_default();
                        Ok(format!("{HANDOFF_STATE_PREFIX}{banner}{note}"))
                    }
                    None => Ok(
                        "(no handoff note recorded yet — action=write {note} to set one)"
                            .to_string(),
                    ),
                }
            }
            "write" => {
                let note = args["note"].as_str().ok_or("missing 'note'")?.trim();
                if note.is_empty() {
                    return Err("'note' must not be empty".to_string());
                }
                let chars = note.chars().count();
                if chars < HANDOFF_NOTE_MIN_CHARS {
                    return Err(format!(
                        "handoff note is {chars} chars — below the {HANDOFF_NOTE_MIN_CHARS}-char \
                         floor. A resume brief must name the goal, completed work with evidence, \
                         in-flight state, ordered next steps, and load-bearing facts. Write the \
                         full brief."
                    ));
                }
                if chars > HANDOFF_NOTE_MAX_CHARS {
                    return Err(format!(
                        "handoff note exceeds {HANDOFF_NOTE_MAX_CHARS} characters — keep it a \
                         dense resume brief, not a transcript"
                    ));
                }
                // Disk is the cross-session contract: a failed persist is a
                // failed handoff, reported loudly rather than discovered by the
                // next session.
                self.persist(note)?;
                *state = Some(note.to_string());
                Ok(format!("{HANDOFF_STATE_PREFIX}{note}"))
            }
            other => Err(format!("unknown action {other:?} (use show|write)")),
        }
    }
}

// ---------------------------------------------------------------------------
// Notes tool — persistent, cross-session agent memory (a file on disk, unlike
// the in-memory todo). Every record is bound to canonical repository identity;
// legacy global markdown and mismatched records are inert.
// ---------------------------------------------------------------------------

const NOTES_SCHEMA: &str = "angel-project-notes/v1";

#[derive(serde::Serialize, serde::Deserialize)]
struct NotesRecord {
    schema: String,
    workspace: PathBuf,
    project_key: String,
    notes: Vec<String>,
}

fn notes_path(workspace: &std::path::Path) -> PathBuf {
    if let Some(path) = std::env::var_os("ANGEL_NOTES_FILE").filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    let identity = crate::platform::workspace_store::repo_identity(workspace);
    crate::platform::workspace_store::workspace_json_path_in(
        &crate::platform::workspace_store::angel_subdir("notes"),
        &identity.root,
    )
}

pub(crate) struct NotesTool {
    path: PathBuf,
    workspace: PathBuf,
    project_key: String,
}
impl NotesTool {
    pub(crate) fn new(workspace: &std::path::Path) -> Self {
        let identity = crate::platform::workspace_store::repo_identity(workspace);
        Self {
            path: notes_path(workspace),
            workspace: identity.root,
            project_key: identity.key,
        }
    }
    #[cfg(test)]
    pub(crate) fn at(path: PathBuf, workspace: &std::path::Path) -> Self {
        let identity = crate::platform::workspace_store::repo_identity(workspace);
        Self {
            path,
            workspace: identity.root,
            project_key: identity.key,
        }
    }

    fn load(&self) -> Option<NotesRecord> {
        let raw = std::fs::read_to_string(&self.path).ok()?;
        let record = serde_json::from_str::<NotesRecord>(&raw).ok()?;
        (record.schema == NOTES_SCHEMA
            && record.workspace == self.workspace
            && record.project_key == self.project_key)
            .then_some(record)
    }

    fn save(&self, notes: Vec<String>) -> Result<(), String> {
        if self.path.exists() && self.load().is_none() {
            return Err("notes file is legacy, malformed, or belongs to another project".into());
        }
        let record = NotesRecord {
            schema: NOTES_SCHEMA.to_string(),
            workspace: self.workspace.clone(),
            project_key: self.project_key.clone(),
            notes,
        };
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| format!("encode notes: {e}"))?;
        crate::platform::workspace_store::write_private_atomic(&self.path, &bytes)
            .map_err(|error| format!("persist notes: {error}"))
    }
}
impl Tool for NotesTool {
    fn name(&self) -> &str {
        "notes"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "notes".to_string(),
            description: "Your persistent project memory — survives restarts and is shared \
                          only across this repository's sessions. action=list \
                          (default) reads it; add {text} appends a note; clear wipes it. Use \
                          for durable findings and decisions."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "add", "clear"], "description": "default 'list'" },
                    "text": { "type": "string", "description": "for action=add" },
                },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        match args["action"].as_str().unwrap_or("list") {
            "list" => {
                let notes = self.load().map(|record| record.notes).unwrap_or_default();
                if notes.is_empty() {
                    Ok("(no notes)".to_string())
                } else {
                    Ok(notes
                        .into_iter()
                        .map(|note| format!("- {note}"))
                        .collect::<Vec<_>>()
                        .join("\n"))
                }
            }
            "add" => {
                let text = args["text"].as_str().ok_or("missing 'text'")?.trim();
                if text.is_empty() {
                    return Err("'text' must not be empty".to_string());
                }
                let mut notes = self.load().map(|record| record.notes).unwrap_or_default();
                notes.push(text.to_string());
                self.save(notes)?;
                Ok(format!("noted → {}", self.path.display()))
            }
            "clear" if self.path.exists() && self.load().is_none() => {
                Err("refusing to clear notes owned by another project or legacy state".to_string())
            }
            "clear" => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok("notes cleared".to_string()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    Ok("notes already empty".to_string())
                }
                Err(e) => Err(format!("clear notes: {e}")),
            },
            other => Err(format!("unknown action {other:?} (use list|add|clear)")),
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/plan__tests.rs"]
mod tests;
