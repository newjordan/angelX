//! The first-party Anthropic Messages API, as the `anthropic` seat speaks it
//! (Claude Sonnet 5.5 and its family). The Messages translator next door
//! carries the shape this wire shares with DeepSeek's; this module holds what
//! the first-party API asks beyond it: adaptive thinking bound to its prefix,
//! effort, signed thinking replayed verbatim, cache breakpoints, the version,
//! beta and workspace headers, and refusals.
//! `ANGEL_ANTHROPIC_THINKING_BINDING=0` sends plain adaptive thinking with no
//! beta header, for a relay that admits only the published request fields.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(super) const API_VERSION: &str = "2023-06-01";
pub(super) const THINKING_BINDING_BETA: &str = "thinking-binding-controls-2026-08-01";
pub(crate) const DEFAULT_MAX_TOKENS: u32 = 32_000;
pub(crate) const DEFAULT_EFFORT: &str = "medium";
const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

pub(super) fn messages_url(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/v1/messages") {
        base.to_string()
    } else if base.ends_with("/v1") {
        format!("{base}/messages")
    } else {
        format!("{base}/v1/messages")
    }
}

pub(super) fn binding_controls() -> bool {
    crate::agent::harness::env_flag("ANGEL_ANTHROPIC_THINKING_BINDING", true)
}

pub(super) fn request_headers(
    key: &str,
    workspace: Option<&str>,
    bearer: bool,
) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("x-api-key", key.to_string()),
        ("anthropic-version", API_VERSION.to_string()),
    ];
    // Relays and metering gateways authenticate with a Bearer route token
    // (substituting the real credential upstream); the first-party API uses
    // `x-api-key` alone. The caller decides by route, never by model name.
    if bearer {
        headers.push(("authorization", format!("Bearer {key}")));
    }
    if binding_controls() {
        headers.push(("anthropic-beta", THINKING_BINDING_BETA.to_string()));
    }
    if let Some(workspace) = workspace.map(str::trim).filter(|id| !id.is_empty()) {
        headers.push(("anthropic-workspace-id", workspace.to_string()));
    }
    headers
}

pub(super) fn effort(requested: Option<&str>) -> &'static str {
    let requested = requested
        .map(|effort| effort.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if let Some(rung) = EFFORTS.iter().copied().find(|rung| *rung == requested) {
        return rung;
    }
    match requested.as_str() {
        "none" | "minimal" | "off" => "low",
        _ => DEFAULT_EFFORT,
    }
}

fn is_signed_thinking(block: &Value) -> bool {
    let present = |key: &str| {
        block
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
    };
    match block.get("type").and_then(Value::as_str) {
        Some("thinking") => present("signature"),
        Some("redacted_thinking") => present("data"),
        _ => false,
    }
}

pub(super) fn replay_blocks(reasoning: &str) -> Vec<Value> {
    match serde_json::from_str::<Value>(reasoning) {
        Ok(Value::Array(blocks)) => blocks.into_iter().filter(is_signed_thinking).collect(),
        _ => Vec::new(),
    }
}

pub(super) fn replay_json(content: &[Value]) -> Option<String> {
    let blocks = content
        .iter()
        .filter(|block| is_signed_thinking(block))
        .cloned()
        .collect::<Vec<_>>();
    (!blocks.is_empty()).then(|| Value::Array(blocks).to_string())
}

#[derive(Default)]
pub(super) struct ThinkingCapture {
    blocks: BTreeMap<u64, Value>,
}

impl ThinkingCapture {
    pub(super) fn start(&mut self, index: u64, block: &Value) {
        if matches!(
            block.get("type").and_then(Value::as_str),
            Some("thinking" | "redacted_thinking")
        ) {
            self.blocks.insert(index, block.clone());
        }
    }

    fn append(&mut self, index: u64, key: &str, text: &str) {
        let Some(block) = self.blocks.get_mut(&index).and_then(Value::as_object_mut) else {
            return;
        };
        let mut joined = block
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        joined.push_str(text);
        block.insert(key.to_string(), json!(joined));
    }

    pub(super) fn thinking(&mut self, index: u64, text: &str) {
        self.append(index, "thinking", text);
    }

    pub(super) fn signature(&mut self, index: u64, signature: &str) {
        self.append(index, "signature", signature);
    }

    pub(super) fn take_json(&mut self) -> Option<String> {
        let blocks = std::mem::take(&mut self.blocks)
            .into_values()
            .filter(is_signed_thinking)
            .collect::<Vec<_>>();
        (!blocks.is_empty()).then(|| Value::Array(blocks).to_string())
    }
}

pub(super) fn chat_usage(usage: &Map<String, Value>) -> Value {
    let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let (input, read, write) = (
        count("input_tokens"),
        count("cache_read_input_tokens"),
        count("cache_creation_input_tokens"),
    );
    json!({
        "prompt_tokens": input + read + write,
        "completion_tokens": count("output_tokens"),
        "cache_read_input_tokens": read,
        "cache_creation_input_tokens": write,
    })
}

pub(super) fn refusal_error(details: Option<&Value>) -> Value {
    let field = |key: &str| {
        details
            .and_then(|details| details.get(key))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
    };
    let category = field("category").unwrap_or("unspecified");
    let mut message = format!(
        "Anthropic declined the request (stop_reason refusal, stop_details.category: {category})"
    );
    if let Some(explanation) = field("explanation") {
        message.push_str(": ");
        message.push_str(explanation);
    }
    json!({"type": "refusal", "category": category, "message": message})
}

fn mark_cache(block: Option<&mut Value>) {
    if let Some(block) = block.and_then(Value::as_object_mut) {
        block.insert("cache_control".into(), json!({"type": "ephemeral"}));
    }
}

pub(super) fn finish_body(body: &mut Map<String, Value>, chat: &Value) -> Result<(), String> {
    body.insert(
        "thinking".into(),
        if binding_controls() {
            json!({"type": "adaptive", "block_binding": {"prefix_mismatch_behavior": "drop_block"}})
        } else {
            json!({"type": "adaptive"})
        },
    );
    body.insert(
        "output_config".into(),
        json!({"effort": effort(chat.get("reasoning_effort").and_then(Value::as_str))}),
    );
    let messages = body
        .get_mut("messages")
        .and_then(Value::as_array_mut)
        .ok_or("Anthropic Messages: the request carries no messages")?;
    while messages
        .last()
        .is_some_and(|message| message["role"] == "assistant")
    {
        messages.pop();
    }
    let last = messages
        .last_mut()
        .ok_or("Anthropic Messages: the request carries no user turn")?;
    mark_cache(
        last["content"]
            .as_array_mut()
            .and_then(|blocks| blocks.last_mut()),
    );
    for key in ["tools", "system"] {
        mark_cache(
            body.get_mut(key)
                .and_then(Value::as_array_mut)
                .and_then(|items| items.last_mut()),
        );
    }
    mark_cache(
        body.get_mut("system")
            .and_then(Value::as_array_mut)
            .and_then(|blocks| {
                let standing = blocks.len().checked_sub(2)?;
                blocks.get_mut(standing)
            }),
    );
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/club/http__anthropic_messages_tests.rs"]
mod tests;
