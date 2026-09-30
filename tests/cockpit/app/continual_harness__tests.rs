use super::*;
use crate::agent::harness::Tool;

fn tmp_ws(tag: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "angel_ch_{tag}_{}_{}",
        std::process::id(),
        now_iso()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let harness_dir = root.join("harness-store");
    std::fs::create_dir_all(&harness_dir).unwrap();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CONTINUAL_HARNESS_DIR", &harness_dir) };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CONTINUAL_HARNESS", "1") };
    // Force path-keyed identity so temp dirs don't need git.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_REPO_IDENTITY", "0") };
    let ws = root.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    (ws, root)
}

fn cleanup(root: PathBuf) {
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CONTINUAL_HARNESS_DIR") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CONTINUAL_HARNESS") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_REPO_IDENTITY") };
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn create_update_delete_and_prompt_block() {
    let _g = crate::tests::env_lock();
    let (ws, root) = tmp_ws("crud");
    let edit = RefinementEdit {
        action: "create".into(),
        kind: EntryKind::Memory,
        id: Some("m1".into()),
        title: Some("Build gate".into()),
        content: Some("Always run cargo test -p bench before claiming done.".into()),
        path: Some("general".into()),
        reason: None,
    };
    let msg = apply_edits(
        &ws,
        Scope::Project,
        "test create",
        "unit test",
        "entry exists",
        &[edit],
    )
    .unwrap();
    assert!(msg.contains("create memory:m1"), "{msg}");

    let block = context_block(&ws);
    assert!(block.contains(HARNESS_BLOCK_HEADER));
    assert!(block.contains("Build gate"));
    assert!(block.contains(HARNESS_BLOCK_SENTINEL));

    let update = RefinementEdit {
        action: "update".into(),
        kind: EntryKind::Memory,
        id: Some("m1".into()),
        title: Some("Build gate".into()),
        content: Some("Always run cargo test -p bench --release before claiming done.".into()),
        path: None,
        reason: None,
    };
    apply_edits(
        &ws,
        Scope::Project,
        "test update",
        "unit",
        "updated",
        &[update],
    )
    .unwrap();
    let state = load_project(&ws);
    assert_eq!(state.entries["m1"].version, 2);
    assert!(state.entries["m1"].content.contains("--release"));

    let event_id = state.refinements.last().unwrap().id.clone();
    let rolled = rollback(&ws, Scope::Project, &event_id).unwrap();
    assert!(rolled.contains("rolled back"), "{rolled}");
    let state = load_project(&ws);
    assert_eq!(state.entries["m1"].version, 1);
    assert!(!state.entries["m1"].content.contains("--release"));

    let del = RefinementEdit {
        action: "delete".into(),
        kind: EntryKind::Memory,
        id: Some("m1".into()),
        title: None,
        content: None,
        path: None,
        reason: None,
    };
    apply_edits(&ws, Scope::Project, "test del", "unit", "gone", &[del]).unwrap();
    assert!(load_project(&ws).entries.is_empty());
    cleanup(root);
}

#[test]
fn seed_light_is_idempotent() {
    let _g = crate::tests::env_lock();
    let (ws, root) = tmp_ws("seed");
    let a = seed_light_prover(&ws);
    assert!(a.contains("edit") || a.contains("create"), "{a}");
    let n = load_project(&ws).entry_count();
    assert!(n >= 5, "expected seed entries, got {n}");
    let b = seed_light_prover(&ws);
    assert!(b.contains("already"), "{b}");
    assert_eq!(load_project(&ws).entry_count(), n);
    cleanup(root);
}

#[test]
fn tool_list_and_create() {
    let _g = crate::tests::env_lock();
    let (ws, root) = tmp_ws("tool");
    let tool = ContinualHarnessTool::new(&ws);
    let listed = tool.call(&serde_json::json!({"action": "list"})).unwrap();
    assert!(listed.contains("continual harness"), "{listed}");
    let created = tool
        .call(&serde_json::json!({
            "action": "create",
            "kind": "skill",
            "id": "smoke",
            "title": "Smoke build",
            "content": "cargo build -p bench --release",
            "evidence": "test"
        }))
        .unwrap();
    assert!(created.contains("create skill:smoke"), "{created}");
    cleanup(root);
}

#[test]
fn disabled_yields_empty_block() {
    let _g = crate::tests::env_lock();
    let (ws, root) = tmp_ws("off");
    seed_light_prover(&ws);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CONTINUAL_HARNESS", "0") };
    assert_eq!(context_block(&ws), "");
    cleanup(root);
}

/// An entry added mid-session reaches the next turn through the knowledge
/// broker; a block the conversation already carries (the start-up context or
/// a /loop prompt) is not repeated.
#[test]
fn a_mid_session_entry_reaches_the_next_turn_once() {
    let _g = crate::tests::env_lock();
    let (ws, root) = tmp_ws("broker");
    let _broker = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "1");
    let registry = crate::agent::harness::ToolRegistry::with_team(ws.clone(), Vec::new());
    let started = context_block(&ws);
    assert!(started.is_empty(), "{started}");
    let mut history = vec![
        crate::agent::club::ChatMsg::system("stable system"),
        crate::agent::club::ChatMsg::user("keep going"),
    ];
    let edit = RefinementEdit {
        action: "create".into(),
        kind: EntryKind::Memory,
        id: Some("policy".into()),
        title: Some("Installed campaign policy".into()),
        content: Some("Run the focused test before the full suite.".into()),
        path: Some("general".into()),
        reason: None,
    };
    apply_edits(
        &ws,
        Scope::Project,
        "rl install",
        "campaign",
        "entry exists",
        &[edit],
    )
    .unwrap();
    crate::agent::harness::refresh_knowledge_broker(&registry, &mut history, 120_000, &[], true);
    let carried = |history: &[crate::agent::club::ChatMsg]| {
        history
            .iter()
            .map(|m| m.content.matches("Installed campaign policy").count())
            .sum::<usize>()
    };
    assert_eq!(carried(&history), 1, "{history:?}");
    crate::agent::harness::refresh_knowledge_broker(&registry, &mut history, 120_000, &[], true);
    assert_eq!(carried(&history), 1, "{history:?}");

    // Already in the start-up context: not added again.
    let mut booted = vec![
        crate::agent::club::ChatMsg::system(format!("stable system\n{}", context_block(&ws))),
        crate::agent::club::ChatMsg::user("keep going"),
    ];
    crate::agent::harness::refresh_knowledge_broker(&registry, &mut booted, 120_000, &[], true);
    assert_eq!(carried(&booted), 1, "{booted:?}");
    cleanup(root);
}
