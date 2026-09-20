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
