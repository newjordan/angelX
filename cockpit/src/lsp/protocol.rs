//! Content-Length framing and JSON-RPC message handling for the LSP client
//! (module-breakup: the wire protocol, extracted from `lsp.rs`).
//!
//! Pure framing/parse/match functions over `serde_json::Value` — no process,
//! no threads, directly unit-tested. `LspClient` (the stdio lifecycle, still
//! in the parent) and the diagnostics tools consume these through the
//! parent's re-exports.

use serde_json::{Value, json};
use std::io::BufRead;

/// Frame a JSON-RPC message: `Content-Length: N\r\n\r\n` + UTF-8 body.
pub(crate) fn build_frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).unwrap_or_default();
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(&body);
    frame
}

/// Read one frame: parse headers up to the blank line, then exactly
/// Content-Length bytes of body. `Ok(None)` on EOF / malformed header (caller
/// treats both as "connection done"). A body that isn't valid JSON yields
/// `Ok(Some(Value::Null))` so the reader can skip it without dying.
pub(crate) fn read_frame<R: BufRead>(r: &mut R) -> std::io::Result<Option<Value>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None); // EOF
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // end of headers
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse::<usize>().ok();
        }
        // other headers (Content-Type) are ignored
    }
    let len = match content_length {
        Some(l) => l,
        None => return Ok(None), // headers ended with no length -> malformed
    };
    // A buggy/crashing/hostile language server can declare an enormous
    // Content-Length; `vec![0u8; len]` would abort the WHOLE process via
    // handle_alloc_error (or panic on capacity overflow), killing every pane and
    // session. Reject implausible frames (64 MiB is far above any real LSP
    // message) and reserve fallibly so even an in-range low-memory case is a
    // recoverable Err rather than an abort.
    const MAX_FRAME_BYTES: usize = 64 << 20;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("LSP frame Content-Length {len} exceeds {MAX_FRAME_BYTES}-byte cap"),
        ));
    }
    let mut buf = Vec::new();
    buf.try_reserve_exact(len)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::OutOfMemory, e.to_string()))?;
    buf.resize(len, 0u8);
    r.read_exact(&mut buf)?;
    Ok(Some(serde_json::from_slice(&buf).unwrap_or(Value::Null)))
}

#[cfg(test)]
mod frame_stress {
    use super::read_frame;

    #[test]
    fn oversized_content_length_is_rejected_not_allocated() {
        // A buggy/hostile server declaring a gigantic Content-Length must be
        // rejected before `vec![0u8; len]` aborts the whole process.
        let framed = format!("Content-Length: {}\r\n\r\n", 9_000_000_000u64);
        let mut cursor = std::io::Cursor::new(framed.into_bytes());
        assert!(read_frame(&mut cursor).is_err());
    }

    #[test]
    fn well_formed_small_frame_still_reads() {
        let body = "{\"jsonrpc\":\"2.0\"}";
        let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let mut cursor = std::io::Cursor::new(framed.into_bytes());
        let v = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
    }
}

/// `Some(Ok(result))` / `Some(Err(msg))` if `v` is the response to `id`; `None`
/// for notifications, other ids, or server→client requests.
pub(crate) fn match_response(v: &Value, id: u64) -> Option<Result<Value, String>> {
    // A message carrying a `method` is a request or notification FROM the server,
    // never a response to us — even if its `id` happens to collide with our
    // pending request id (the two id spaces are independent in JSON-RPC).
    if v.get("method").is_some() {
        return None;
    }
    if v.get("id").and_then(|x| x.as_u64()) != Some(id) {
        return None;
    }
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("lsp error");
        return Some(Err(msg.to_string()));
    }
    Some(Ok(v.get("result").cloned().unwrap_or(Value::Null)))
}

/// A message is a server→client REQUEST (needs a reply) when it has BOTH an `id`
/// and a `method` (a notification has a method but no id; a response has an id but
/// no method).
pub(crate) fn is_server_request(v: &Value) -> bool {
    v.get("id").is_some() && v.get("method").is_some()
}

/// The reply to a server→client request: a success with a null result. We don't
/// act on these (progress tokens, capability registration, configuration), but
/// answering keeps rust-analyzer from stalling/​warning on large projects — an
/// unanswered request leaves it waiting. `id` is echoed back verbatim.
pub(crate) fn server_request_reply(req: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": req.get("id").cloned().unwrap_or(Value::Null), "result": null })
}

/// `Some(params)` if `v` is a `publishDiagnostics` notification for `uri`.
pub(crate) fn publish_diagnostics_for<'a>(v: &'a Value, uri: &str) -> Option<&'a Value> {
    if v.get("method").and_then(|m| m.as_str()) != Some("textDocument/publishDiagnostics") {
        return None;
    }
    let params = v.get("params")?;
    (params.get("uri").and_then(|u| u.as_str()) == Some(uri)).then_some(params)
}

/// Normalize a pull `textDocument/diagnostic` report to the same
/// `{ uri, diagnostics: [...] }` shape as a push payload, so one formatter serves
/// both. A `RelatedFullDocumentDiagnosticReport` has `items`; an `unchanged`
/// report has none (→ empty).
pub(crate) fn pull_report_to_params(uri: &str, report: &Value) -> Value {
    let items = report.get("items").cloned().unwrap_or_else(|| json!([]));
    json!({ "uri": uri, "diagnostics": items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pull_report_normalizes_full_and_unchanged() {
        let full = json!({ "kind": "full", "items": [ { "severity": 1, "message": "x" } ] });
        let p = pull_report_to_params("file:///a.rs", &full);
        assert_eq!(p["uri"], "file:///a.rs");
        assert_eq!(p["diagnostics"].as_array().unwrap().len(), 1);
        let unchanged = json!({ "kind": "unchanged", "resultId": "1" });
        let p2 = pull_report_to_params("file:///a.rs", &unchanged);
        assert_eq!(p2["diagnostics"].as_array().unwrap().len(), 0); // no items -> clean
    }
}
