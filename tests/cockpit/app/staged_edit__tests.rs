use super::*;
use std::fs;

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
    assert!(card.contains("resolve_edit"), "{card}");
    let list = list_staged();
    assert!(list.contains("staged edit"), "{list}");
    let id = list
        .lines()
        .find_map(|l| l.split('#').nth(1)?.split_whitespace().next()?.parse().ok())
        .expect("id");
    let rej = reject(id, Some("changed mind")).unwrap();
    assert!(rej.contains("rejected"), "{rej}");
    assert!(list_staged().contains("no staged"));
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
    let acc = accept(id, Some("looks good")).unwrap();
    assert!(acc.contains("accepted"), "{acc}");
    assert_eq!(fs::read_to_string(root.join("b.rs")).unwrap(), "two\n");
    let _ = fs::remove_dir_all(&root);
    clear_staged_for_test();
}
