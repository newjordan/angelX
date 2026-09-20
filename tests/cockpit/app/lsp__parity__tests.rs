use super::*;

#[test]
fn lsp_language_ids_and_utf16_positions() {
    assert_eq!(language_id(Path::new("a.js"), "typescript"), "javascript");
    assert_eq!(
        language_id(Path::new("a.jsx"), "typescript"),
        "javascriptreact"
    );
    assert_eq!(
        language_id(Path::new("a.tsx"), "typescript"),
        "typescriptreact"
    );
    assert_eq!(
        symbol_at(&json!({"line": 1, "character": 7}), "//😀 foo()").unwrap(),
        "foo"
    );
}

#[test]
fn lsp_name_query_confirms_heuristic_declaration_not_comment() {
    let query = definition_query(
        &json!({"path": "comment.js", "symbol": "foo"}),
        "src/a.js:2:function foo() {}",
        NavKind::Definition,
    );
    assert_eq!(query["path"], "src/a.js");
    assert_eq!(
        query_position(&query, "// foo in comment\nfunction foo() {}"),
        Ok((1, 9))
    );
}

#[test]
fn lsp_empty_timeout_and_merge_preserve_lexical_answer() {
    for semantic in [Ok("no locations found".into()), Err("timed out".into())] {
        let answer = merge(
            "a.js:1:function foo() {}",
            semantic,
            "readiness: cached_ready",
        );
        assert!(answer.starts_with("source: heuristic"));
        assert!(answer.contains("a.js:1:function foo() {}"));
    }
    let answer = merge(
        "a.js:1:foo",
        Ok("a.js:2:foo".into()),
        "readiness: cached_ready",
    );
    assert!(answer.contains("source: merged"));
    assert!(answer.contains("a.js:1:foo"));
    assert!(answer.contains("a.js:2:foo"));
}
