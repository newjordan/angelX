//! Import a Codex rollout session into the cockpit as conversation history.
//! Codex stores rollouts as JSONL under `$CODEX_HOME/sessions/YYYY/MM/DD/`; each
//! `response_item` of payload type `message` carries a role + text parts. We map
//! developer→system, user→user, assistant→assistant and drop tool/reasoning items.

use crate::club::{ChatMsg, ChatRole};
use std::path::{Path, PathBuf};

fn sessions_root() -> PathBuf {
    let base = std::env::var("CODEX_HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(home).join(".codex")
        });
    base.join("sessions")
}

fn collect_rollouts(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rollouts(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl")
            && path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with("rollout-"))
                .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

/// Newest-first rollout paths under the Codex sessions root (capped at `limit`).
pub fn list(limit: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rollouts(&sessions_root(), &mut files);
    files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    files.reverse();
    files.truncate(limit);
    files
}

/// Parse a Codex rollout JSONL into cockpit history (messages only).
pub fn parse(path: &Path) -> Result<Vec<ChatMsg>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    Ok(parse_rollout(&raw))
}

/// Pure parser over rollout text — unit-testable without files.
pub fn parse_rollout(raw: &str) -> Vec<ChatMsg> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("response_item") {
            continue;
        }
        let Some(p) = v.get("payload") else { continue };
        if p.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        let role = match p.get("role").and_then(|r| r.as_str()) {
            Some("user") => ChatRole::User,
            Some("assistant") => ChatRole::Assistant,
            Some("developer") | Some("system") => ChatRole::System,
            _ => continue,
        };
        let text = p
            .get("content")
            .and_then(|c| c.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|seg| seg.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        out.push(match role {
            ChatRole::System => ChatMsg::system(text),
            ChatRole::Assistant => ChatMsg::assistant(text),
            _ => ChatMsg::user(text),
        });
    }
    out
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/codex_import__tests.rs"]
mod tests;
