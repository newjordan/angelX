use super::*;
use serde_json::json;

#[test]
fn path_to_uri_encodes_spaces() {
    assert_eq!(path_to_uri(Path::new("/a/b.rs")), "file:///a/b.rs");
    assert_eq!(path_to_uri(Path::new("/a b/c.rs")), "file:///a%20b/c.rs");
}

#[test]
fn uri_to_path_decodes() {
    assert_eq!(uri_to_path("file:///a/b.rs"), "/a/b.rs");
    assert_eq!(uri_to_path("file:///a%20b/c.rs"), "/a b/c.rs");
}

#[test]
fn find_symbol_position_first_occurrence_utf16() {
    let text = "let x = 1;\nfoo(x);\n";
    assert_eq!(find_symbol_position(text, "foo"), Some((1, 0)));
    assert_eq!(find_symbol_position(text, "x"), Some((0, 4)));
    assert_eq!(find_symbol_position(text, "nope"), None);
    // UTF-16 column: an emoji (2 code units) before the symbol.
    assert_eq!(find_symbol_position("😀 bar", "bar"), Some((0, 3)));
}

#[test]
fn resolve_position_prefers_line_then_symbol() {
    let text = "alpha\nbeta\n";
    // explicit 1-based line/char -> 0-based
    assert_eq!(
        resolve_position(&json!({ "line": 2, "character": 3 }), text).unwrap(),
        (1, 2)
    );
    // line defaults character to 1 -> col 0
    assert_eq!(
        resolve_position(&json!({ "line": 1 }), text).unwrap(),
        (0, 0)
    );
    // symbol fallback
    assert_eq!(
        resolve_position(&json!({ "symbol": "beta" }), text).unwrap(),
        (1, 0)
    );
    // neither -> error
    assert!(resolve_position(&json!({}), text).is_err());
    assert!(resolve_position(&json!({ "symbol": "zzz" }), text).is_err());
}

#[test]
fn format_locations_handles_shapes() {
    // single Location
    let one = json!({ "uri": "file:///a.rs", "range": { "start": { "line": 4, "character": 2 } } });
    assert!(format_locations(&one).contains("1 location(s)"));
    assert!(format_locations(&one).contains("/a.rs:5:3"));
    // array of LocationLink (targetUri/targetSelectionRange)
    let many = json!([
        { "targetUri": "file:///a.rs", "targetSelectionRange": { "start": { "line": 0, "character": 0 } } },
        { "targetUri": "file:///b.rs", "targetSelectionRange": { "start": { "line": 9, "character": 1 } } }
    ]);
    let out = format_locations(&many);
    assert!(out.contains("2 location(s)"));
    assert!(
        out.contains("/a.rs:1:1") && out.contains("/b.rs:10:2"),
        "got:\n{out}"
    );
    // null / empty
    assert!(format_locations(&Value::Null).contains("no locations"));
    assert!(format_locations(&json!([])).contains("no locations"));
}

#[test]
fn format_hover_extracts_markup_string_and_array() {
    let markup = json!({ "contents": { "kind": "markdown", "value": "fn foo() -> i32" } });
    assert_eq!(format_hover(&markup), "fn foo() -> i32");
    let bare = json!({ "contents": "plain doc" });
    assert_eq!(format_hover(&bare), "plain doc");
    let arr = json!({ "contents": [ "a", { "language": "rust", "value": "b" } ] });
    assert_eq!(format_hover(&arr), "a\nb");
    assert_eq!(format_hover(&json!({})), "no hover info");
    assert_eq!(format_hover(&json!({ "contents": "   " })), "no hover info");
}

#[test]
fn format_document_symbols_indents_hierarchy_and_flat() {
    // hierarchical DocumentSymbol[] with one child
    let hier = json!([
        { "name": "Foo", "kind": 23,
          "range": { "start": { "line": 0, "character": 0 } },
          "selectionRange": { "start": { "line": 0, "character": 7 } },
          "children": [
            { "name": "bar", "kind": 12, "detail": "fn(&self)",
              "selectionRange": { "start": { "line": 4, "character": 7 } } }
          ] }
    ]);
    let out = format_document_symbols(&hier);
    assert!(out.contains("2 symbol(s)"), "counts nested symbols: {out}");
    assert!(out.contains("struct Foo  L1"), "got:\n{out}");
    assert!(
        out.contains("  fn bar  L5  fn(&self)"),
        "child indented w/ detail:\n{out}"
    );
    // flat SymbolInformation[] (location.range)
    let flat = json!([
        { "name": "main", "kind": 12, "location": { "uri": "file:///x.rs", "range": { "start": { "line": 9, "character": 3 } } } }
    ]);
    assert!(format_document_symbols(&flat).contains("fn main  L10"));
    // empty / non-array
    assert_eq!(format_document_symbols(&json!([])), "no symbols");
    assert_eq!(format_document_symbols(&Value::Null), "no symbols");
}

#[test]
fn symbol_kind_label_maps_common_kinds() {
    assert_eq!(symbol_kind_label(Some(12)), "fn");
    assert_eq!(symbol_kind_label(Some(23)), "struct");
    assert_eq!(symbol_kind_label(Some(13)), "var");
    assert_eq!(symbol_kind_label(Some(999)), "symbol");
    assert_eq!(symbol_kind_label(None), "symbol");
}

#[test]
fn format_workspace_symbols_lists_hits_with_container() {
    let results = json!([
        { "name": "Foo", "kind": 23, "containerName": "mymod",
          "location": { "uri": "file:///a.rs", "range": { "start": { "line": 9, "character": 0 } } } },
        { "name": "foo", "kind": 12,
          "location": { "uri": "file:///b.rs", "range": { "start": { "line": 0, "character": 3 } } } }
    ]);
    let out = format_workspace_symbols("foo", &results);
    assert!(out.contains("2 match(es) for `foo`"), "got:\n{out}");
    assert!(out.contains("struct Foo  /a.rs:10  (mymod)"), "got:\n{out}");
    assert!(out.contains("fn foo  /b.rs:1"), "got:\n{out}");
    // empty / non-array
    assert!(format_workspace_symbols("x", &json!([])).contains("no workspace symbols"));
    assert!(format_workspace_symbols("x", &Value::Null).contains("no workspace symbols"));
}
