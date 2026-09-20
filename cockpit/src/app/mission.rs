//! Mission-line projection for `/status`.
//!
//! The headless Mission ledger (`scripts/runtime/mission.mjs`) persists completion
//! objectives under `~/.angel0/missions/` (`ANGEL_MISSION_DIR` overrides). This
//! module is a read-only, best-effort projection: the most recently updated
//! mission becomes one `/status` line so the operator can see the objective's
//! state from inside the TUI without leaving for the CLI. A malformed,
//! foreign-schema, or transient `.tmp-` pointer is skipped — projection never
//! mutates and never fails the status command.

use serde_json::Value;
use std::path::PathBuf;

pub(crate) const MISSION_SCHEMA: &str = "angel.mission/v1";
const MAX_LINE_CHARS: usize = 140;

/// Render one parsed mission pointer as a one-line summary, or `None` when the
/// record is not a well-formed `angel.mission/v1` state. Pure.
pub(crate) fn mission_line(v: &Value) -> Option<String> {
    if v.get("schema")?.as_str()? != MISSION_SCHEMA {
        return None;
    }
    let status = v.get("status")?.as_str()?;
    let objective = v.get("objective")?.as_str()?;
    let rounds = v.get("roundsStarted").and_then(|n| n.as_u64()).unwrap_or(0);
    let max_rounds = v.get("maxRounds").and_then(|n| n.as_u64());
    let budget = match max_rounds {
        Some(cap) => format!("r{rounds}/{cap}"),
        None => format!("r{rounds}/∞"),
    };
    let mut line = format!("[{status}] {budget} ");
    // Collapse whitespace so a multi-line objective stays one terminal row.
    line.push_str(&objective.split_whitespace().collect::<Vec<_>>().join(" "));
    if let Some(reason) = v.get("blockedReason").and_then(|r| r.as_str())
        && !reason.is_empty()
    {
        line.push_str(&format!(" · blocked: {reason}"));
    }
    if line.chars().count() > MAX_LINE_CHARS {
        let cut: String = line.chars().take(MAX_LINE_CHARS - 1).collect();
        return Some(format!("{cut}…"));
    }
    Some(line)
}

/// Scan the mission directory and return the most recently updated mission's
/// summary line, or `None` when the directory holds no readable mission.
/// Read-only and best-effort.
pub(crate) fn latest_mission_line() -> Option<String> {
    let dir = mission_dir();
    let mut best: Option<(String, Value)> = None;
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // Pointers only; skip the append-only ledgers and transient temp files.
        if !name.ends_with(".json") || name.contains(".tmp-") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(entry.path()) else {
            continue; // one unreadable pointer never aborts the whole scan
        };
        let Ok(v) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(updated) = v
            .get("updatedAt")
            .and_then(|u| u.as_str())
            .map(str::to_string)
        else {
            continue;
        };
        if best.as_ref().is_none_or(|(ts, _)| updated > *ts) {
            best = Some((updated, v));
        }
    }
    let (_, v) = best?;
    mission_line(&v)
}

fn mission_dir() -> PathBuf {
    std::env::var_os("ANGEL_MISSION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".angel0")
                .join("missions")
        })
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/mission__tests.rs"]
mod tests;
