//! Lexical navigation remains available when an analyzer is empty or unavailable.
use super::NavKind;
use crate::harness::Tool;
use crate::tools::nav::{DefsTool, GrepTool, OutlineTool};
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn language_id<'a>(path: &Path, configured: &'a str) -> &'a str {
    match path.extension().and_then(|s| s.to_str()) {
        Some("js" | "mjs" | "cjs") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("tsx") => "typescriptreact",
        _ => configured,
    }
}

fn symbol_at(args: &Value, text: &str) -> Result<String, String> {
    if let Some(symbol) = args["symbol"].as_str() {
        return Ok(symbol.to_owned());
    }
    let (line, column) = super::resolve_position(args, text)?;
    let line = text
        .lines()
        .nth(line as usize)
        .ok_or("line outside document")?;
    let mut utf16 = 0;
    let byte = line
        .char_indices()
        .find_map(|(byte, ch)| {
            let here = utf16;
            utf16 += ch.len_utf16() as u64;
            (here == column).then_some(byte)
        })
        .ok_or("character outside document")?;
    let identifier = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    let start = line[..byte]
        .char_indices()
        .rev()
        .find(|(_, ch)| !identifier(*ch))
        .map_or(0, |(offset, ch)| offset + ch.len_utf8());
    let end = line[byte..]
        .char_indices()
        .find(|(_, ch)| !identifier(*ch))
        .map_or(line.len(), |(offset, _)| byte + offset);
    if start == end {
        return Err("position is not on a symbol".into());
    }
    Ok(line[start..end].to_owned())
}

pub(super) fn navigation(root: &Path, args: &Value, kind: NavKind) -> Result<String, String> {
    if matches!(kind, NavKind::Hover) {
        return Ok(String::new());
    }
    let path = args["path"].as_str().ok_or("path required")?;
    let text = crate::harness::confined_read(root, Path::new(path))?;
    let text = String::from_utf8(text).map_err(|e| e.to_string())?;
    let symbol = symbol_at(args, &text)?;
    match kind {
        NavKind::Definition => DefsTool { root: root.into() }.call(&json!({"name": symbol})),
        NavKind::References => GrepTool { root: root.into() }
            .call(&json!({"pattern": format!(r"\b{}\b", regex::escape(&symbol))})),
        NavKind::Hover => unreachable!(),
    }
}

// A name-only query starts from the heuristic declaration rather than a comment
// or unrelated first occurrence in the caller's file. Preserve every candidate;
// the analyzer's response is labelled confirmation, not a replacement search.
pub(super) fn definition_query(args: &Value, heuristic: &str, kind: NavKind) -> Value {
    let mut query = args.clone();
    if matches!(kind, NavKind::Definition) && args.get("line").is_none() {
        for candidate in heuristic.lines() {
            let mut fields = candidate.splitn(3, ':');
            if let (Some(path), Some(line)) = (fields.next(), fields.next())
                && let Ok(line) = line.parse::<u64>()
            {
                query["path"] = json!(path);
                query["line"] = json!(line);
                query["_declaration"] = json!(true);
                break;
            }
        }
    }
    query
}

pub(super) fn query_position(args: &Value, text: &str) -> Result<(u64, u64), String> {
    if args["_declaration"] == true {
        let line = args["line"].as_u64().ok_or("declaration line missing")? - 1;
        let source = text
            .lines()
            .nth(line as usize)
            .ok_or("declaration line outside file")?;
        let symbol = args["symbol"]
            .as_str()
            .ok_or("declaration symbol missing")?;
        let column = source.find(symbol).ok_or("declaration symbol not found")?;
        return Ok((line, source[..column].encode_utf16().count() as u64));
    }
    super::resolve_position(args, text)
}

pub(super) fn outline(root: &Path, args: &Value) -> Result<String, String> {
    let text = OutlineTool { root: root.into() }.call(args)?;
    Ok(text
        .lines()
        .map(|line| {
            if let Some((number, declaration)) = line.split_once(':')
                && number.parse::<usize>().is_ok()
            {
                return format!("{declaration}  L{number}");
            }
            line.to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

pub(super) fn merge(heuristic: &str, semantic: Result<String, String>, readiness: &str) -> String {
    match semantic {
        Ok(answer)
            if !answer.is_empty()
                && !answer.starts_with("no locations")
                && !answer.starts_with("no symbols") =>
        {
            format!(
                "source: merged\n{readiness}\nheuristic results:\n{heuristic}\nlsp results (confirmation / additional locations):\n{answer}"
            )
        }
        Ok(_) => format!("source: heuristic\n{readiness}\nlsp: empty\n{heuristic}"),
        Err(error) => format!("source: heuristic\n{readiness}\nlsp error: {error}\n{heuristic}"),
    }
}

#[cfg(test)]
mod tests {
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
}
