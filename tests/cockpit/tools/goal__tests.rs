use super::*;

#[test]
fn goal_tool_set_show_clear_roundtrip() {
    let _guard = crate::tests::env_lock();
    let file = std::env::temp_dir().join(format!("angel_goal_tool_{}.json", std::process::id()));
    let _goal_file = crate::tests::TestEnvGuard::set(
        "ANGEL_GOAL_FILE",
        file.to_str().expect("temp path is utf8"),
    );
    let workspace = std::env::temp_dir().join(format!("angel_goal_tool_ws_{}", std::process::id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let tool = GoalTool::new(&workspace);

    // No goal yet.
    assert_eq!(
        tool.call(&serde_json::json!({"action": "show"})).unwrap(),
        "(no goal set)"
    );

    // Set a goal with a verifiable command and criteria.
    let set = tool
        .call(&serde_json::json!({
            "action": "set",
            "text": "harden the harness",
            "accept_cmd": "cargo test",
            "criteria": ["tests green"]
        }))
        .unwrap();
    assert!(set.contains("harden the harness"), "set: {set}");

    // It lands in the canonical store.
    let stored = crate::drive::goal::load_for(&workspace).expect("goal persisted");
    assert_eq!(stored.text, "harden the harness");
    assert_eq!(stored.accept_cmd.as_deref(), Some("cargo test"));
    assert_eq!(stored.acceptance, vec!["tests green".to_string()]);

    // show reads it back.
    let shown = tool.call(&serde_json::json!({"action": "show"})).unwrap();
    assert!(shown.contains("harden the harness"), "show: {shown}");
    assert!(shown.contains("cargo test"), "show: {shown}");

    // clear removes it from the store.
    let cleared = tool.call(&serde_json::json!({"action": "clear"})).unwrap();
    assert!(cleared.contains("cleared"), "clear: {cleared}");
    assert!(crate::drive::goal::load_for(&workspace).is_none());

    std::fs::remove_dir_all(&workspace).ok();
}

#[test]
fn goal_tool_set_requires_text() {
    let _guard = crate::tests::env_lock();
    let file = std::env::temp_dir().join(format!(
        "angel_goal_tool_missing_{}.json",
        std::process::id()
    ));
    let _goal_file = crate::tests::TestEnvGuard::set(
        "ANGEL_GOAL_FILE",
        file.to_str().expect("temp path is utf8"),
    );
    let workspace =
        std::env::temp_dir().join(format!("angel_goal_tool_missing_ws_{}", std::process::id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let tool = GoalTool::new(&workspace);
    let err = tool
        .call(&serde_json::json!({"action": "set"}))
        .expect_err("set without text must fail");
    assert!(err.contains("missing 'text'"), "err: {err}");
    std::fs::remove_dir_all(&workspace).ok();
}
