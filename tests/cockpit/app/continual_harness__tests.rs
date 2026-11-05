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

fn memory(id: &str, title: &str, content: &str) -> RefinementEdit {
    RefinementEdit {
        action: "create".into(),
        kind: EntryKind::Memory,
        id: Some(id.into()),
        title: Some(title.into()),
        content: Some(content.into()),
        path: Some("general".into()),
        reason: None,
    }
}

fn seed_eight(ws: &Path) {
    let mut edits: Vec<RefinementEdit> = (0..7)
        .map(|i| {
            memory(
                &format!("a{i}"),
                &format!("A ledger lesson {i}"),
                "Price the cap band before banking a work saving.",
            )
        })
        .collect();
    edits.push(memory(
        "zz",
        "Zz payload trap",
        "The submission ships the working tree, so a regenerated __pycache__ goes with it.",
    ));
    apply_edits(
        ws,
        Scope::Project,
        "seed",
        "unit test",
        "entries exist",
        &edits,
    )
    .unwrap();
}

#[test]
fn the_block_shows_the_entries_the_turn_is_about() {
    let _g = crate::tests::env_lock();
    let (ws, root) = tmp_ws("ranked");
    seed_eight(&ws);

    // Past the six-per-kind window, the name-last entry still rides when the
    // turn is about it.
    let ranked = context_block_for(&ws, "why did the pycache ship in the payload?");
    assert!(ranked.contains("Zz payload trap"), "{ranked}");
    assert!(ranked.contains("+2 more memory entries"), "{ranked}");
    assert_eq!(ranked.matches("- [project:").count(), 6, "{ranked}");

    let unrelated = context_block_for(&ws, "ledger lesson cap band");
    assert!(!unrelated.contains("Zz payload trap"), "{unrelated}");
    cleanup(root);
}

#[test]
fn search_returns_full_text_of_matching_entries_best_first() {
    let _g = crate::tests::env_lock();
    let (ws, root) = tmp_ws("search");
    seed_eight(&ws);
    let tool = ContinualHarnessTool::new(&ws);
    let found = tool
        .call(&serde_json::json!({"action": "search", "query": "pycache working tree", "limit": 3}))
        .unwrap();
    assert!(found.contains("1 of 8 entries"), "{found}");
    assert!(
        found.contains("so a regenerated __pycache__ goes with it."),
        "{found}"
    );

    let none = tool
        .call(&serde_json::json!({"action": "search", "query": "blake3"}))
        .unwrap();
    assert!(none.contains("0 of 8 entries"), "{none}");
    assert!(
        tool.call(&serde_json::json!({"action": "search"}))
            .unwrap_err()
            .contains("query")
    );
    cleanup(root);
}

#[test]
fn the_entry_cap_is_an_env_knob() {
    let _g = crate::tests::env_lock();
    let (ws, root) = tmp_ws("cap");
    let _cap = crate::tests::TestEnvGuard::set("ANGEL_CONTINUAL_HARNESS_MAX_ENTRIES", "2");
    let msg = apply_edits(
        &ws,
        Scope::Project,
        "seed",
        "unit test",
        "entries exist",
        &[
            memory("m1", "One", "first"),
            memory("m2", "Two", "second"),
            memory("m3", "Three", "third"),
        ],
    )
    .unwrap();
    assert!(msg.contains("m3: harness full (2 entries)"), "{msg}");
    assert_eq!(load_project(&ws).entry_count(), 2);
    cleanup(root);
}
