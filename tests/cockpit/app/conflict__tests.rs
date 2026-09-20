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
    let entries = register_conflicts("f.rs", &blocks);
    assert_eq!(entries.len(), 1);
    let id = entries[0].id;
    let entry = get_conflict(id).unwrap();
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
    let e = register_conflicts("x", &blocks)[0].clone();
    let out = splice_conflict(text, &e, "C\n").unwrap();
    assert_eq!(out, "C\n");
    clear_conflicts_for_test();
}
