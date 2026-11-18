//! Content-Length framing and JSON-RPC message handling for the LSP client
//! (module-breakup: the wire protocol, extracted from `lsp.rs`).
//!
//! Pure framing/parse/match functions over `serde_json::Value` — no process,
//! no threads, directly unit-tested. `LspClient` (the stdio lifecycle, still
//! in the parent) and the diagnostics tools consume these through the
//! parent's re-exports.

use serde_json::{Value, json};
use std::io::BufRead;

pub(crate) fn build_frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).unwrap_or_default();
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(&body);
    frame
}

pub(crate) fn read_frame<R: BufRead>(r: &mut R) -> std::io::Result<Option<Value>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse::<usize>().ok();
        }
    }
    let len = match content_length {
        Some(l) => l,
        None => return Ok(None),
    };
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
#[path = "../../../../tests/cockpit/app/lsp__protocol__frame_stress.rs"]
mod frame_stress;

pub(crate) fn match_response(v: &Value, id: u64) -> Option<Result<Value, String>> {
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

pub(crate) fn is_server_request(v: &Value) -> bool {
    v.get("id").is_some() && v.get("method").is_some()
}

pub(crate) fn server_request_reply(req: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": req.get("id").cloned().unwrap_or(Value::Null), "result": null })
}

pub(crate) fn publish_diagnostics_for<'a>(v: &'a Value, uri: &str) -> Option<&'a Value> {
    if v.get("method").and_then(|m| m.as_str()) != Some("textDocument/publishDiagnostics") {
        return None;
    }
    let params = v.get("params")?;
    (params.get("uri").and_then(|u| u.as_str()) == Some(uri)).then_some(params)
}

pub(crate) fn pull_report_to_params(uri: &str, report: &Value) -> Value {
    let items = report.get("items").cloned().unwrap_or_else(|| json!([]));
    json!({ "uri": uri, "diagnostics": items })
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/lsp__protocol__tests.rs"]
mod tests;
