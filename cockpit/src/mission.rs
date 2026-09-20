//! Mission-line projection for `/status`.
//!
//! The headless Mission ledger (`scripts/mission.mjs`) persists completion
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
mod tests {
    use super::*;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::tests::env_lock()
    }

    fn mission_json(status: &str, updated: &str) -> Value {
        serde_json::json!({
            "schema": MISSION_SCHEMA,
            "id": "test-mission",
            "revision": 1,
            "objective": "win the benchmark",
            "maxRounds": 10,
            "roundsStarted": 3,
            "status": status,
            "blockedReason": null,
            "blockedStreak": 0,
            "createdAt": updated,
            "updatedAt": updated,
        })
    }

    #[test]
    fn line_renders_status_rounds_and_objective() {
        let line = mission_line(&mission_json("active", "2026-07-07T00:00:00Z")).unwrap();
        assert_eq!(line, "[active] r3/10 win the benchmark");
    }

    #[test]
    fn line_renders_the_blocked_reason() {
        let mut v = mission_json("active", "2026-07-07T00:00:00Z");
        v["blockedReason"] = serde_json::json!("gpu down");
        v["status"] = serde_json::json!("blocked");
        let line = mission_line(&v).unwrap();
        assert_eq!(
            line,
            "[blocked] r3/10 win the benchmark · blocked: gpu down"
        );
    }

    #[test]
    fn line_rejects_foreign_schema_and_missing_fields() {
        assert_eq!(
            mission_line(&serde_json::json!({"schema": "other/v1"})),
            None
        );
        assert_eq!(
            mission_line(&serde_json::json!({"schema": MISSION_SCHEMA})),
            None
        );
        assert_eq!(mission_line(&serde_json::json!({})), None);
    }

    #[test]
    fn line_collapses_whitespace_and_bounds_length() {
        let mut v = mission_json("active", "2026-07-07T00:00:00Z");
        v["objective"] = serde_json::json!(format!("a very {} long", "x".repeat(300)));
        let line = mission_line(&v).unwrap();
        assert!(
            line.chars().count() <= MAX_LINE_CHARS,
            "{}",
            line.chars().count()
        );
        assert!(line.ends_with('…'));
        assert!(!line.contains('\n'));
    }

    #[test]
    fn latest_picks_the_newest_and_skips_garbage() {
        let _guard = env_lock();
        let dir = std::env::temp_dir().join(format!("angel_mission_proj_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_MISSION_DIR", &dir) };
        std::fs::write(
            dir.join("old.json"),
            mission_json("active", "2026-07-06T00:00:00Z").to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("new.json"),
            mission_json("paused", "2026-07-08T00:00:00Z").to_string(),
        )
        .unwrap();
        std::fs::write(dir.join("garbage.json"), "{ not json").unwrap();
        std::fs::write(dir.join("old.jsonl"), "ledger row, never read\n").unwrap();
        std::fs::write(dir.join("new.json.tmp-42"), "transient, never read").unwrap();
        assert_eq!(
            latest_mission_line().as_deref(),
            Some("[paused] r3/10 win the benchmark")
        );
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_MISSION_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn latest_returns_none_when_no_missions_exist() {
        let _guard = env_lock();
        let dir = std::env::temp_dir().join(format!("angel_mission_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_MISSION_DIR", &dir) };
        assert_eq!(latest_mission_line(), None);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_MISSION_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }
}
