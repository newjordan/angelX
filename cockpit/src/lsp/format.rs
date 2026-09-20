//! Agent-readable formatting for LSP results (module-breakup: protocol
//! payloads → terminal text, extracted from `lsp.rs`).
//!
//! Everything here is pure over `serde_json::Value` / `&str` / `&Path`:
//! URI ↔ path conversion, symbol/position resolution, and the formatters the
//! diagnostics/nav/symbols tools render. Directly unit-tested.

use serde_json::Value;
use std::path::Path;

pub(crate) fn severity_label(sev: Option<u64>) -> &'static str {
    match sev {
        Some(1) => "error",
        Some(2) => "warning",
        Some(3) => "info",
        Some(4) => "hint",
        _ => "diag",
    }
}

/// Render a `publishDiagnostics` params payload as agent-readable lines
/// (1-based line:col, like an editor and like compiler output).
pub(crate) fn format_diagnostics(path_display: &str, params: &Value) -> String {
    let diags = match params.get("diagnostics").and_then(|d| d.as_array()) {
        Some(d) => d,
        None => return format!("{path_display}: no diagnostics field"),
    };
    if diags.is_empty() {
        return format!("{path_display}: clean — 0 diagnostics");
    }
    let mut lines = Vec::with_capacity(diags.len());
    for d in diags {
        let sev = severity_label(d.get("severity").and_then(|s| s.as_u64()));
        let line = d
            .pointer("/range/start/line")
            .and_then(|x| x.as_u64())
            .unwrap_or(0)
            + 1;
        let col = d
            .pointer("/range/start/character")
            .and_then(|x| x.as_u64())
            .unwrap_or(0)
            + 1;
        let msg = d
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .replace('\n', " ");
        let code = match d.get("code") {
            Some(Value::String(s)) => format!(" [{s}]"),
            Some(Value::Number(n)) => format!(" [{n}]"),
            _ => String::new(),
        };
        lines.push(format!("  {sev} {line}:{col}  {msg}{code}"));
    }
    format!(
        "{path_display}: {} diagnostic(s)\n{}",
        diags.len(),
        lines.join("\n")
    )
}

/// `file://` URI for an absolute path (minimal, not full percent-encoding —
/// spaces are encoded since they're the common case; rust-analyzer/pyright accept
/// the rest verbatim on local paths).
pub(crate) fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace(' ', "%20");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

/// Decode a `file://` URI back to a path string (inverse of [`path_to_uri`] for
/// the common local case: strip the scheme, undo `%20`).
pub(crate) fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://")
        .unwrap_or(uri)
        .replace("%20", " ")
}

/// First occurrence of `symbol` in `text`, as a 0-based (line, utf16-col)
/// position for an LSP request. Column counts UTF-16 code units (the LSP default
/// encoding) so non-ASCII lines map correctly. `None` if absent.
pub(crate) fn find_symbol_position(text: &str, symbol: &str) -> Option<(u64, u64)> {
    if symbol.is_empty() {
        return None;
    }
    for (line_idx, line) in text.lines().enumerate() {
        if let Some(byte_col) = line.find(symbol) {
            let utf16_col: usize = line[..byte_col].encode_utf16().count();
            return Some((line_idx as u64, utf16_col as u64));
        }
    }
    None
}

/// Resolve the query position from tool args: explicit 1-based `line`(+`character`)
/// wins; otherwise locate `symbol`'s first occurrence in `text`. Returns 0-based
/// (line, character) for the LSP request.
pub(crate) fn resolve_position(args: &Value, text: &str) -> Result<(u64, u64), String> {
    if let Some(line) = args.get("line").and_then(|l| l.as_u64()) {
        let ch = args.get("character").and_then(|c| c.as_u64()).unwrap_or(1);
        return Ok((line.saturating_sub(1), ch.saturating_sub(1)));
    }
    if let Some(sym) = args.get("symbol").and_then(|s| s.as_str()) {
        return find_symbol_position(text, sym)
            .ok_or_else(|| format!("tool error: symbol `{sym}` not found in file"));
    }
    Err("tool error: provide `symbol` or `line` (+ optional `character`)".into())
}

/// One LSP `Location`/`LocationLink` → `path:line:col` (1-based). Handles both the
/// `Location { uri, range }` and `LocationLink { targetUri, targetSelectionRange }`
/// shapes servers return interchangeably.
pub(crate) fn location_line(loc: &Value) -> Option<String> {
    let uri = loc
        .get("uri")
        .or_else(|| loc.get("targetUri"))
        .and_then(|u| u.as_str())?;
    let range = loc
        .get("range")
        .or_else(|| loc.get("targetSelectionRange"))
        .or_else(|| loc.get("targetRange"))?;
    let line = range
        .pointer("/start/line")
        .and_then(|x| x.as_u64())
        .unwrap_or(0)
        + 1;
    let col = range
        .pointer("/start/character")
        .and_then(|x| x.as_u64())
        .unwrap_or(0)
        + 1;
    Some(format!("  {}:{}:{}", uri_to_path(uri), line, col))
}

/// Render a definition/references result (null | Location | Location[]) as a list
/// of `path:line:col`.
pub(crate) fn format_locations(result: &Value) -> String {
    let locs: Vec<&Value> = match result {
        Value::Null => vec![],
        Value::Array(a) => a.iter().collect(),
        obj => vec![obj],
    };
    let lines: Vec<String> = locs.iter().filter_map(|l| location_line(l)).collect();
    if lines.is_empty() {
        return "no locations found".into();
    }
    format!("{} location(s):\n{}", lines.len(), lines.join("\n"))
}

/// Extract readable text from a hover result: `contents` may be a `MarkupContent
/// { value }`, a bare string, a `{ language, value }`, or an array of those.
pub(crate) fn format_hover(result: &Value) -> String {
    fn one(v: &Value) -> Option<String> {
        match v {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => o.get("value").and_then(|x| x.as_str()).map(String::from),
            _ => None,
        }
    }
    let contents = match result.get("contents") {
        Some(c) => c,
        None => return "no hover info".into(),
    };
    let text = match contents {
        Value::Array(a) => a.iter().filter_map(one).collect::<Vec<_>>().join("\n"),
        other => one(other).unwrap_or_default(),
    };
    let text = text.trim();
    if text.is_empty() {
        "no hover info".into()
    } else {
        text.to_string()
    }
}

/// LSP `SymbolKind` enum → a short label.
pub(crate) fn symbol_kind_label(kind: Option<u64>) -> &'static str {
    match kind {
        Some(2) => "module",
        Some(5) => "class",
        Some(6) => "method",
        Some(7) => "property",
        Some(8) => "field",
        Some(9) => "ctor",
        Some(10) => "enum",
        Some(11) => "interface",
        Some(12) => "fn",
        Some(13) => "var",
        Some(14) => "const",
        Some(22) => "enum-member",
        Some(23) => "struct",
        Some(25) => "operator",
        Some(26) => "type-param",
        _ => "symbol",
    }
}

/// One symbol's start line (1-based), from whichever shape the server used:
/// `DocumentSymbol` (`selectionRange`/`range`) or `SymbolInformation`
/// (`location.range`).
pub(crate) fn symbol_line(sym: &Value) -> u64 {
    sym.pointer("/selectionRange/start/line")
        .or_else(|| sym.pointer("/range/start/line"))
        .or_else(|| sym.pointer("/location/range/start/line"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0)
        + 1
}

/// Render a `textDocument/documentSymbol` result as an indented outline. Handles
/// both the hierarchical `DocumentSymbol[]` (recurses `children`) and the flat
/// `SymbolInformation[]` shapes. `out` accumulates `"  kind name  Lline detail"`.
pub(crate) fn walk_symbols(sym: &Value, depth: usize, out: &mut Vec<String>) {
    let kind = symbol_kind_label(sym.get("kind").and_then(|k| k.as_u64()));
    let name = sym.get("name").and_then(|n| n.as_str()).unwrap_or("?");
    let detail = sym
        .get("detail")
        .and_then(|d| d.as_str())
        .filter(|d| !d.is_empty())
        .map(|d| format!("  {d}"))
        .unwrap_or_default();
    out.push(format!(
        "{}{} {}  L{}{}",
        "  ".repeat(depth),
        kind,
        name,
        symbol_line(sym),
        detail
    ));
    if let Some(children) = sym.get("children").and_then(|c| c.as_array()) {
        for child in children {
            walk_symbols(child, depth + 1, out);
        }
    }
}

/// Format a `documentSymbol` result (an array, hierarchical or flat) as a symbol
/// outline. Empty/absent → a friendly note.
pub(crate) fn format_document_symbols(result: &Value) -> String {
    let arr = match result.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return "no symbols".into(),
    };
    let mut out = Vec::new();
    for sym in arr {
        walk_symbols(sym, 0, &mut out);
    }
    format!("{} symbol(s):\n{}", out.len(), out.join("\n"))
}

/// One `WorkspaceSymbol`/`SymbolInformation` → `kind name  path:line  (container)`.
pub(crate) fn workspace_symbol_line(sym: &Value) -> Option<String> {
    let name = sym.get("name").and_then(|n| n.as_str())?;
    let kind = symbol_kind_label(sym.get("kind").and_then(|k| k.as_u64()));
    let uri = sym.pointer("/location/uri").and_then(|u| u.as_str())?;
    let line = symbol_line(sym); // reads /location/range/start/line
    let container = sym
        .get("containerName")
        .and_then(|c| c.as_str())
        .filter(|c| !c.is_empty())
        .map(|c| format!("  ({c})"))
        .unwrap_or_default();
    Some(format!(
        "  {} {}  {}:{}{}",
        kind,
        name,
        uri_to_path(uri),
        line,
        container
    ))
}

/// Format a merged `workspace/symbol` result (flat `SymbolInformation[]`) into a
/// project-wide hit list. `results` is one server's array.
pub(crate) fn format_workspace_symbols(query: &str, results: &Value) -> String {
    let lines: Vec<String> = results
        .as_array()
        .map(|a| a.iter().filter_map(workspace_symbol_line).collect())
        .unwrap_or_default();
    if lines.is_empty() {
        return format!("no workspace symbols matching `{query}`");
    }
    format!(
        "{} match(es) for `{}`:\n{}",
        lines.len(),
        query,
        lines.join("\n")
    )
}

#[cfg(test)]
mod tests {
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
        let one =
            json!({ "uri": "file:///a.rs", "range": { "start": { "line": 4, "character": 2 } } });
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
}
