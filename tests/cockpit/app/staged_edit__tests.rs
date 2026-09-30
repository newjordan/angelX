use super::*;
use crate::agent::harness::Tool;
use crate::agent::tools::file::ResolveEditTool;
use std::fs;

fn proposal_id(card: &str) -> u32 {
    card.split('#')
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn stage_update(root: &Path) -> u32 {
    fs::write(root.join("owned.txt"), "original\n").unwrap();
    proposal_id(
        &stage_batch(
            root,
            vec![StagedAction::Update {
                path: "owned.txt".into(),
                original: b"original\n".to_vec(),
                content: "accepted\n".into(),
            }],
            vec![],
        )
        .unwrap(),
    )
}

#[test]
fn staged_edit_workspace_isolation() {
    let _guard = crate::tests::env_lock();
    clear_staged_for_test();
    let root = std::env::temp_dir().join(format!("angel_stage_scope_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let owner = root.join("owner");
    let other = root.join("other");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&other).unwrap();
    let id = stage_update(&owner);
    let owner_tool = ResolveEditTool {
        root: owner.clone(),
    };
    let other_tool = ResolveEditTool {
        root: other.clone(),
    };

    let list = other_tool
        .call(&serde_json::json!({"action": "list"}))
        .unwrap();
    assert!(
        list.contains("no staged"),
        "foreign proposal leaked: {list}"
    );
    for action in ["accept", "reject"] {
        let error = other_tool
            .call(&serde_json::json!({"action": action, "id": id}))
            .unwrap_err();
        assert!(error.contains("unknown staged edit"), "{error}");
        assert_eq!(
            fs::read_to_string(owner.join("owned.txt")).unwrap(),
            "original\n"
        );
        let list = owner_tool
            .call(&serde_json::json!({"action": "list"}))
            .unwrap();
        assert!(
            list.contains(&format!("#{id}")),
            "foreign request consumed proposal: {list}"
        );
    }
    owner_tool
        .call(&serde_json::json!({"action": "accept", "id": id}))
        .unwrap();
    assert_eq!(
        fs::read_to_string(owner.join("owned.txt")).unwrap(),
        "accepted\n"
    );
    let id = stage_update(&owner);
    owner_tool
        .call(&serde_json::json!({"action": "reject", "id": id}))
        .unwrap();
    assert_eq!(
        fs::read_to_string(owner.join("owned.txt")).unwrap(),
        "original\n"
    );
    fs::remove_dir_all(root).unwrap();
    clear_staged_for_test();
}

#[test]
fn staged_edit_oversized_ids_do_not_alias() {
    let _guard = crate::tests::env_lock();
    clear_staged_for_test();
    let root = std::env::temp_dir().join(format!("angel_stage_id_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let id = stage_update(&root);
    let tool = ResolveEditTool { root: root.clone() };
    for action in ["accept", "reject"] {
        let error = tool
            .call(&serde_json::json!({
                "action": action, "id": u64::from(id) + (1_u64 << 32),
            }))
            .unwrap_err();
        assert!(error.contains("id"), "{error}");
        assert_eq!(
            fs::read_to_string(root.join("owned.txt")).unwrap(),
            "original\n"
        );
    }
    tool.call(&serde_json::json!({"action": "accept", "id": id}))
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("owned.txt")).unwrap(),
        "accepted\n"
    );
    fs::remove_dir_all(root).unwrap();
    clear_staged_for_test();
}

#[cfg(unix)]
#[test]
fn staged_edit_workspace_alias_resolves_to_same_owner() {
    let _guard = crate::tests::env_lock();
    clear_staged_for_test();
    let root = std::env::temp_dir().join(format!("angel_stage_alias_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let owner = root.join("owner");
    let alias = root.join("alias");
    fs::create_dir_all(&owner).unwrap();
    std::os::unix::fs::symlink(&owner, &alias).unwrap();
    let id = stage_update(&alias);
    let tool = ResolveEditTool {
        root: owner.clone(),
    };
    let list = tool.call(&serde_json::json!({"action": "list"})).unwrap();
    assert!(list.contains(&format!("#{id}")), "{list}");
    tool.call(&serde_json::json!({"action": "accept", "id": id}))
        .unwrap();
    assert_eq!(
        fs::read_to_string(owner.join("owned.txt")).unwrap(),
        "accepted\n"
    );
    fs::remove_dir_all(root).unwrap();
    clear_staged_for_test();
}

#[cfg(unix)]
#[test]
fn staged_edit_absolute_alias_paths_stay_with_original_workspace() {
    let _guard = crate::tests::env_lock();
    clear_staged_for_test();
    let root = std::env::temp_dir().join(format!("angel_stage_absolute_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let owner = root.join("owner");
    let other = root.join("other");
    let alias = root.join("alias");
    for workspace in [&owner, &other] {
        fs::create_dir_all(workspace).unwrap();
        for name in ["update.txt", "remove.txt", "move.txt"] {
            fs::write(workspace.join(name), "original\n").unwrap();
        }
    }
    std::os::unix::fs::symlink(&owner, &alias).unwrap();
    let path = |name| alias.join(name).to_string_lossy().into_owned();
    let id = proposal_id(
        &stage_batch(
            &alias,
            vec![
                StagedAction::Update {
                    path: path("update.txt"),
                    original: b"original\n".to_vec(),
                    content: "updated\n".into(),
                },
                StagedAction::Remove {
                    path: path("remove.txt"),
                    original: b"original\n".to_vec(),
                },
                StagedAction::Move {
                    from: path("move.txt"),
                    to: path("moved.txt"),
                    original: b"original\n".to_vec(),
                    content: "moved\n".into(),
                },
            ],
            vec![],
        )
        .unwrap(),
    );
    fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&other, &alias).unwrap();
    let tool = ResolveEditTool {
        root: owner.clone(),
    };
    tool.call(&serde_json::json!({"action": "accept", "id": id}))
        .unwrap();
    assert_eq!(
        fs::read_to_string(owner.join("update.txt")).unwrap(),
        "updated\n"
    );
    assert!(!owner.join("remove.txt").exists());
    assert!(!owner.join("move.txt").exists());
    assert_eq!(
        fs::read_to_string(owner.join("moved.txt")).unwrap(),
        "moved\n"
    );
    for name in ["update.txt", "remove.txt", "move.txt"] {
        assert_eq!(fs::read_to_string(other.join(name)).unwrap(), "original\n");
    }
    assert!(!other.join("moved.txt").exists());
    fs::remove_dir_all(root).unwrap();
    clear_staged_for_test();
}

#[test]
fn stage_list_reject() {
    let _guard = crate::tests::env_lock();
    clear_staged_for_test();
    let root = std::env::temp_dir().join(format!("angel_stage_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.rs"), "old\n").unwrap();
    let card = stage_batch(
        &root,
        vec![StagedAction::Update {
            path: "a.rs".into(),
            original: b"old\n".to_vec(),
            content: "new\n".into(),
        }],
        vec![],
    )
    .unwrap();
    assert!(card.contains("proposed"), "{card}");
    // How to accept or reject is the `⠨⠑` pages; the id rides beside them.
    let steps = crate::agent::harness::book::d46_recovery::run(&[
        crate::agent::harness::book::d46_recovery::STAGED_ACCEPT,
        crate::agent::harness::book::d46_recovery::STAGED_REJECT,
    ]);
    assert!(card.contains(&format!("{steps} id=")), "{card}");
    assert!(
        crate::agent::harness::book::d46_recovery::STAGED_ACCEPT
            .text()
            .contains("resolve_edit")
    );
    let list = list_staged(&root);
    assert!(list.contains("staged edit"), "{list}");
    let id = list
        .lines()
        .find_map(|l| l.split('#').nth(1)?.split_whitespace().next()?.parse().ok())
        .expect("id");
    let rej = reject(&root, id, Some("changed mind")).unwrap();
    assert!(rej.contains("rejected"), "{rej}");
    assert!(list_staged(&root).contains("no staged"));
    assert_eq!(fs::read_to_string(root.join("a.rs")).unwrap(), "old\n");
    let _ = fs::remove_dir_all(&root);
    clear_staged_for_test();
}

#[test]
fn stage_accept_writes() {
    let _guard = crate::tests::env_lock();
    clear_staged_for_test();
    let root = std::env::temp_dir().join(format!("angel_stage_acc_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("b.rs"), "one\n").unwrap();
    let card = stage_batch(
        &root,
        vec![StagedAction::Update {
            path: "b.rs".into(),
            original: b"one\n".to_vec(),
            content: "two\n".into(),
        }],
        vec![],
    )
    .unwrap();
    let id: u32 = card
        .split('#')
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let acc = accept(&root, id, Some("looks good")).unwrap();
    assert!(acc.contains("accepted"), "{acc}");
    assert_eq!(fs::read_to_string(root.join("b.rs")).unwrap(), "two\n");
    let _ = fs::remove_dir_all(&root);
    clear_staged_for_test();
}
