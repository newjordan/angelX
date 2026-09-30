use super::*;

#[test]
fn scans_standard_two_way_conflict() {
    let text = "head\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\ntail\n";
    let blocks = scan_text_for_conflicts(text);
    assert_eq!(blocks.len(), 1);
    let b = &blocks[0];
    assert_eq!(b.start_line, 2);
    assert_eq!(b.separator_line, 4);
    assert_eq!(b.end_line, 6);
    assert_eq!(b.ours_lines, vec!["ours".to_string()]);
    assert_eq!(b.theirs_lines, vec!["theirs".to_string()]);
    assert_eq!(b.ours_label.as_deref(), Some("HEAD"));
    assert_eq!(b.theirs_label.as_deref(), Some("branch"));
}

#[test]
fn scans_diff3_base() {
    let text = "<<<<<<< ours\na\n||||||| base\nb\n=======\nc\n>>>>>>> theirs\n";
    let blocks = scan_text_for_conflicts(text);
    assert_eq!(blocks.len(), 1);
    assert_eq!(
        blocks[0].base_lines.as_ref().unwrap(),
        &vec!["b".to_string()]
    );
    assert_eq!(blocks[0].ours_lines, vec!["a".to_string()]);
    assert_eq!(blocks[0].theirs_lines, vec!["c".to_string()]);
}

#[test]
fn ignores_markers_not_at_column_zero_style() {
    // Prefix must be exact; `<<<<<<<x` without space is not a marker.
    let text = "code // <<<<<<< not a marker\nreal\n";
    assert!(scan_text_for_conflicts(text).is_empty());
}

#[test]
fn parse_uri_forms() {
    assert_eq!(
        parse_conflict_uri("conflict://").unwrap(),
        ConflictUri::List
    );
    assert_eq!(
        parse_conflict_uri("conflict://*").unwrap(),
        ConflictUri::All
    );
    assert_eq!(
        parse_conflict_uri("conflict://3").unwrap(),
        ConflictUri::One { id: 3, scope: None }
    );
    assert_eq!(
        parse_conflict_uri("conflict://3/theirs").unwrap(),
        ConflictUri::One {
            id: 3,
            scope: Some(ConflictScope::Theirs)
        }
    );
    assert_eq!(
        parse_conflict_uri("src/a.rs:conflict://2").unwrap(),
        ConflictUri::One { id: 2, scope: None }
    );
}

#[test]
fn register_and_splice_theirs() {
    let _env = crate::tests::env_lock();
    clear_conflicts_for_test();
    let text = "head\n<<<<<<< HEAD\nours line\n=======\ntheirs line\n>>>>>>> branch\ntail\n";
    let blocks = scan_text_for_conflicts(text);
    let entries = register_conflicts(Path::new("."), "f.rs", &blocks);
    assert_eq!(entries.len(), 1);
    let id = entries[0].id;
    let entry = get_conflict(Path::new("."), id).unwrap();
    let repl = resolve_replacement(&entry, "@theirs").unwrap();
    let out = splice_conflict(text, &entry, &repl).unwrap();
    assert_eq!(out, "head\ntheirs line\ntail\n");
    clear_conflicts_for_test();
}

#[test]
fn splice_custom_body() {
    let _env = crate::tests::env_lock();
    clear_conflicts_for_test();
    let text = "<<<<<<<\nA\n=======\nB\n>>>>>>>\n";
    let blocks = scan_text_for_conflicts(text);
    let e = register_conflicts(Path::new("."), "x", &blocks)[0].clone();
    let out = splice_conflict(text, &e, "C\n").unwrap();
    assert_eq!(out, "C\n");
    clear_conflicts_for_test();
}

#[test]
fn conflict_tools_isolate_workspaces_with_identical_relative_paths() {
    use crate::agent::harness::Tool;
    use crate::agent::tools::file::{ReadFileTool, WriteFileTool};
    use serde_json::json;
    use std::fs;

    let _env = crate::tests::env_lock();
    clear_conflicts_for_test();
    let base = std::env::temp_dir().join(format!("angel_conflict_scope_{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let a = base.join("a");
    let b = base.join("b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    let text_a = "<<<<<<< HEAD\nA ours\n=======\nA theirs\n>>>>>>> branch\n";
    let text_b = "<<<<<<< HEAD\nB ours\n=======\nB theirs\n>>>>>>> branch\n";
    fs::write(a.join("shared.rs"), text_a).unwrap();
    fs::write(b.join("shared.rs"), text_b).unwrap();
    let read_a = ReadFileTool { root: a.clone() };
    let read_b = ReadFileTool { root: b.clone() };
    let write_a = WriteFileTool { root: a.clone() };
    let write_b = WriteFileTool { root: b.clone() };

    read_a.call(&json!({"path": "shared.rs"})).unwrap();
    let id_a = list_conflicts(&a)[0].id;
    read_b.call(&json!({"path": "shared.rs"})).unwrap();
    let id_b = list_conflicts(&b)[0].id;
    assert_ne!(
        id_a, id_b,
        "same relative path must retain distinct workspace ids"
    );
    assert_eq!(list_conflicts(&a).len(), 1);
    assert_eq!(list_conflicts(&b).len(), 1);

    let listing_a = read_a.call(&json!({"path": "conflict://"})).unwrap();
    assert!(listing_a.contains(&format!("conflict://{id_a}  ")));
    assert!(!listing_a.contains(&format!("conflict://{id_b}  ")));
    for suffix in ["", "/ours", "/theirs", "/base"] {
        assert!(
            read_b
                .call(&json!({"path": format!("conflict://{id_a}{suffix}")}))
                .is_err()
        );
    }
    assert_eq!(
        read_a
            .call(&json!({"path": format!("conflict://{id_a}/ours")}))
            .unwrap(),
        "A ours\n"
    );
    assert!(
        write_b
            .call(&json!({
                "path": format!("conflict://{id_a}"), "content": "@theirs"
            }))
            .is_err()
    );
    assert!(
        get_conflict(&a, id_a).is_some(),
        "foreign resolution must preserve the owner entry"
    );
    assert_eq!(fs::read_to_string(a.join("shared.rs")).unwrap(), text_a);
    assert_eq!(fs::read_to_string(b.join("shared.rs")).unwrap(), text_b);

    write_a
        .call(&json!({"path": "conflict://*", "content": "@ours"}))
        .unwrap();
    assert_eq!(fs::read_to_string(a.join("shared.rs")).unwrap(), "A ours\n");
    assert_eq!(fs::read_to_string(b.join("shared.rs")).unwrap(), text_b);
    assert!(list_conflicts(&a).is_empty());
    assert!(
        get_conflict(&b, id_b).is_some(),
        "resolving a path must not invalidate its sibling workspace"
    );
    write_b
        .call(&json!({"path": format!("conflict://{id_b}"), "content": "@theirs"}))
        .unwrap();
    assert_eq!(
        fs::read_to_string(b.join("shared.rs")).unwrap(),
        "B theirs\n"
    );
    assert!(list_conflicts(&b).is_empty());
    fs::remove_dir_all(&base).unwrap();
    clear_conflicts_for_test();
}

#[cfg(unix)]
#[test]
fn conflict_registry_recognizes_workspace_aliases() {
    use crate::agent::harness::Tool;
    use crate::agent::tools::file::{ReadFileTool, WriteFileTool};
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::symlink;

    let _env = crate::tests::env_lock();
    clear_conflicts_for_test();
    let base = std::env::temp_dir().join(format!("angel_conflict_alias_{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let root = base.join("workspace");
    let alias = base.join("alias");
    fs::create_dir_all(&root).unwrap();
    symlink(&root, &alias).unwrap();
    let text = "<<<<<<<\nours\n=======\ntheirs\n>>>>>>>\n";
    fs::write(root.join("shared.rs"), text).unwrap();
    ReadFileTool {
        root: alias.clone(),
    }
    .call(&json!({"path": alias.join("shared.rs").to_str().unwrap()}))
    .unwrap();
    let id = list_conflicts(&alias)[0].id;
    assert_eq!(list_conflicts(&alias)[0].path, "shared.rs");
    ReadFileTool { root: root.clone() }
        .call(&json!({"path": "shared.rs"}))
        .unwrap();
    assert_eq!(list_conflicts(&root)[0].id, id);
    assert_eq!(list_conflicts(&root).len(), 1);
    assert_eq!(list_conflicts(&root), list_conflicts(&alias));
    WriteFileTool { root: root.clone() }
        .call(&json!({"path": format!("conflict://{id}"), "content": "@ours"}))
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("shared.rs")).unwrap(),
        "ours\n"
    );
    assert!(list_conflicts(&alias).is_empty());
    fs::remove_dir_all(&base).unwrap();
    clear_conflicts_for_test();
}
