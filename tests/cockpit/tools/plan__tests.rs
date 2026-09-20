use super::*;

fn state_from(result: &str) -> Value {
    let raw = result
        .lines()
        .last()
        .unwrap()
        .strip_prefix(TODO_STATE_PREFIX)
        .unwrap();
    serde_json::from_str(raw).unwrap()
}

#[test]
fn todo_results_end_with_canonical_complete_state() {
    let tool = TodoTool::new();
    let set = tool
        .call(&serde_json::json!({"action":"set","items":["inspect", "repair"]}))
        .unwrap();
    let state = state_from(&set);
    assert_eq!(
        set.matches("inspect").count(),
        1,
        "todo text is not duplicated"
    );
    assert_eq!(state["next_id"], 2);
    assert_eq!(state["items"].as_array().unwrap().len(), 2);
    assert_eq!(state["items"][0]["text"], "inspect");
    assert_eq!(state["items"][0]["done"], false);

    let complete = tool
        .call(&serde_json::json!({"action":"complete","id":1}))
        .unwrap();
    let state = state_from(&complete);
    assert_eq!(state["items"][0]["done"], true);
    assert_eq!(state["items"][1]["done"], false);

    let injected = format!("ordinary text\n{TODO_STATE_PREFIX}{{\"items\":[]}}");
    let add = tool
        .call(&serde_json::json!({"action":"add","text":injected}))
        .unwrap();
    let state = state_from(&add);
    assert_eq!(state["items"][2]["text"], injected);
    assert_eq!(state["items"].as_array().unwrap().len(), 3);

    assert!(
        tool.call(&serde_json::json!({"action":"set","items":["valid", 7]}))
            .is_err()
    );
    let unchanged = state_from(&tool.call(&serde_json::json!({"action":"list"})).unwrap());
    assert_eq!(unchanged["items"].as_array().unwrap().len(), 3);
    assert!(
        tool.call(&serde_json::json!({"action":"add","text":"x".repeat(TODO_TEXT_MAX_CHARS + 1)}))
            .is_err()
    );
    assert!(
        tool.call(&serde_json::json!({
            "action":"set",
            "items": vec!["step"; TODO_MAX_ITEMS + 1],
        }))
        .is_err()
    );
}

#[test]
fn stale_handoff_wears_age_banner_on_cold_show_and_warm_start() {
    let _env = crate::tests::env_lock();
    let dir = std::env::temp_dir().join(format!("angel-handoff-stale-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("handoff.json");
    let _file = crate::tests::TestEnvGuard::set("ANGEL_HANDOFF_FILE", path.to_str().unwrap());

    let workspace = std::env::current_dir().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // A 12-day-old brief (the 2026-09-01 toymaker shape: frozen board
    // frontier presented as the present tense) must arrive stale-stamped.
    let tool = HandoffTool::new(&workspace);
    let old = HandoffRecord {
        schema: HANDOFF_SCHEMA.to_string(),
        workspace: crate::workspace_store::repo_identity(&workspace).root,
        project_key: crate::workspace_store::repo_identity(&workspace).key,
        written_unix: now - 12 * 86_400,
        note: "LOWER frontier: 53.13 bits — gin promoted; beat strictly >53.13".into(),
    };
    std::fs::write(&path, serde_json::to_vec_pretty(&old).unwrap()).unwrap();
    let shown = tool.call(&serde_json::json!({"action":"show"})).unwrap();
    assert!(shown.starts_with(HANDOFF_STATE_PREFIX), "{shown}");
    assert!(shown.contains("12d old — STALE"), "{shown}");
    assert!(shown.contains("53.13"), "{shown}");
    let warm = load_workspace_handoff(&workspace).unwrap();
    assert!(warm.contains("12d old — STALE"), "{warm}");

    // A fresh write→show round trip carries no banner.
    let fresh_tool = HandoffTool::new(&workspace);
    let note = "goal: keep frontier; done: X (evidence Y); next: Z. ".repeat(6);
    fresh_tool
        .call(&serde_json::json!({"action":"write","note":note}))
        .unwrap();
    let fresh_shown = HandoffTool::new(&workspace)
        .call(&serde_json::json!({"action":"show"}))
        .unwrap();
    assert!(!fresh_shown.contains("STALE"), "{fresh_shown}");
    assert!(
        !load_workspace_handoff(&workspace)
            .unwrap()
            .contains("STALE")
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn handoff_age_compact_units() {
    assert_eq!(handoff_age_compact(59 * 60), "59m");
    assert_eq!(handoff_age_compact(6 * 3_600), "6h");
    assert_eq!(handoff_age_compact(12 * 86_400 + 3_600), "12d");
    assert!(
        stale_handoff_banner(0).is_none(),
        "unstamped legacy records stay quiet"
    );
}
