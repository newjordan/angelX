//! DeepSeek's Anthropic-compatible Messages wire, as DeepSeek's own harness
//! speaks it (deepseek-harness `packages/llm/llm-deepseek`, Messages-only since
//! 2026-09-19). The seat still builds its Chat Completions body with every
//! angelX policy (effort, reasoning pass-back, output cap, splices); this module
//! translates that finished body into a Messages request, and each Messages
//! stream line back into the Chat Completions frame the accumulator reads.
//! The first-party Anthropic API shares this shape; its own rules live in
//! [`super::anthropic_messages`] and apply under [`Flavor::Anthropic`].

use super::anthropic_messages;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Whose Messages API a seat speaks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Flavor {
    /// DeepSeek's Anthropic-compatible endpoint (`ANGEL_DEEPSEEK_API=messages`).
    #[default]
    DeepSeek,
    /// The first-party Anthropic API (the `anthropic` seat).
    Anthropic,
}

/// DeepSeek's Messages endpoint for the seat's configured Chat base URL: the
/// provider's own host serves it under `/anthropic`; a forwarder or gateway in
/// front of the seat serves it beside its Chat path.
pub(super) fn messages_url(base_url: &str) -> String {
    if let Ok(url) = std::env::var("ANGEL_DEEPSEEK_MESSAGES_URL")
        && !url.trim().is_empty()
    {
        return url.trim().to_string();
    }
    let official = url::Url::parse(base_url).ok().is_some_and(|url| {
        url.host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("api.deepseek.com"))
    });
    if official {
        "https://api.deepseek.com/anthropic/v1/messages".to_string()
    } else {
        format!("{}/messages", base_url.trim_end_matches('/'))
    }
}

/// Whether the route takes a later `system` message as the whole new system
/// prompt (`/models` `system_prompt_update: in-history`: deepseek-flash). Other
/// routes read only the leading one (`leading-only`: deepseek-v4-pro).
pub(super) fn system_in_history(model: &str) -> bool {
    model.eq_ignore_ascii_case("deepseek-flash")
}

/// The Messages endpoint a seat of this flavor posts to.
pub(super) fn messages_url_for(flavor: Flavor, base_url: &str) -> String {
    match flavor {
        Flavor::DeepSeek => messages_url(base_url),
        Flavor::Anthropic => anthropic_messages::messages_url(base_url),
    }
}

fn text_block(text: &str) -> Value {
    json!({"type": "text", "text": text})
}

/// Whether a text block carries something this flavor accepts: the
/// first-party API rejects a block of only whitespace.
fn keeps_text(text: &str, flavor: Flavor) -> bool {
    match flavor {
        Flavor::DeepSeek => !text.is_empty(),
        Flavor::Anthropic => !text.trim().is_empty(),
    }
}

/// Chat content (a string or text/image parts) as Messages input blocks.
fn input_blocks(content: &Value, flavor: Flavor) -> Vec<Value> {
    match content {
        Value::String(text) if keeps_text(text, flavor) => vec![text_block(text)],
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                Some("text") => part
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|text| keeps_text(text, flavor))
                    .map(text_block),
                Some("image_url") => {
                    let url = part.pointer("/image_url/url").and_then(Value::as_str)?;
                    let (meta, data) = url.strip_prefix("data:")?.split_once(',')?;
                    let media_type = meta.strip_suffix(";base64")?;
                    Some(json!({"type": "image", "source": {
                        "type": "base64", "media_type": media_type, "data": data}}))
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Historical tool arguments DeepSeek cannot represent go as `{}`, as its own
/// harness sends them; the durable history keeps the original.
fn tool_input(arguments: &Value) -> Value {
    let parsed = match arguments {
        Value::String(raw) => serde_json::from_str::<Value>(raw).ok(),
        other => Some(other.clone()),
    };
    parsed.filter(Value::is_object).unwrap_or_else(|| json!({}))
}

fn assistant_blocks(message: &Value, flavor: Flavor) -> Vec<Value> {
    let mut blocks = Vec::new();
    if let Some(thinking) = message
        .get("reasoning_content")
        .and_then(Value::as_str)
        .filter(|thinking| !thinking.is_empty())
    {
        match flavor {
            Flavor::DeepSeek => blocks.push(json!({"type": "thinking", "thinking": thinking})),
            // The turn's signed blocks, verbatim and first.
            Flavor::Anthropic => blocks.extend(anthropic_messages::replay_blocks(thinking)),
        }
    }
    blocks.extend(input_blocks(
        message.get("content").unwrap_or(&Value::Null),
        flavor,
    ));
    for call in message
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        blocks.push(json!({
            "type": "tool_use",
            "id": call.get("id").cloned().unwrap_or(Value::Null),
            "name": call.pointer("/function/name").cloned().unwrap_or(Value::Null),
            "input": tool_input(call.pointer("/function/arguments").unwrap_or(&Value::Null)),
        }));
    }
    blocks
}

fn push_merged(messages: &mut Vec<Value>, role: &str, content: Vec<Value>) {
    if content.is_empty() {
        return;
    }
    if let Some(last) = messages.last_mut()
        && last["role"] == role
        && let Some(existing) = last["content"].as_array_mut()
    {
        existing.extend(content);
        return;
    }
    messages.push(json!({"role": role, "content": content}));
}

/// Translate a finished Chat Completions body into DeepSeek's Messages body.
#[cfg(test)]
pub(super) fn to_messages_body(chat: &Value) -> Result<Value, String> {
    to_messages_body_for(chat, Flavor::DeepSeek)
}

/// Translate a finished Chat Completions body into a Messages body of this
/// flavor.
pub(super) fn to_messages_body_for(chat: &Value, flavor: Flavor) -> Result<Value, String> {
    let model = chat
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let in_history = flavor == Flavor::DeepSeek && system_in_history(model);
    let chat_messages = chat
        .get("messages")
        .and_then(Value::as_array)
        .ok_or("DeepSeek Messages: the request carries no messages")?;
    let mut system = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    for message in chat_messages {
        let content = message.get("content").unwrap_or(&Value::Null);
        match message.get("role").and_then(Value::as_str) {
            Some("system") => {
                let text = input_blocks(content, flavor)
                    .iter()
                    .filter_map(|block| block["text"].as_str().map(str::to_string))
                    .collect::<String>();
                if messages.is_empty() {
                    system.push(text);
                } else if in_history && messages.last().is_some_and(|last| last["role"] == "user") {
                    // A later system message on an in-history route is the new
                    // prompt, after the user turn it follows (append-only).
                    messages.push(json!({"role": "system", "content": [text_block(&text)]}));
                } else if !text.is_empty() {
                    push_merged(&mut messages, "user", vec![text_block(&text)]);
                }
            }
            Some("assistant") => push_merged(
                &mut messages,
                "assistant",
                assistant_blocks(message, flavor),
            ),
            Some("tool") => {
                let result = input_blocks(content, flavor);
                let mut block = json!({
                    "type": "tool_result",
                    "tool_use_id": message.get("tool_call_id").cloned().unwrap_or(Value::Null),
                });
                // An empty result goes without content on the first-party API.
                if flavor == Flavor::DeepSeek || !result.is_empty() {
                    block["content"] = Value::Array(result);
                }
                push_merged(&mut messages, "user", vec![block]);
            }
            _ => push_merged(&mut messages, "user", input_blocks(content, flavor)),
        }
    }
    // Every tool result leads its user message, answering the calls just made.
    for message in &mut messages {
        if message["role"] == "user"
            && let Some(blocks) = message["content"].as_array_mut()
        {
            blocks.sort_by_key(|block| block["type"] != "tool_result");
        }
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert(
        "stream".into(),
        chat.get("stream").cloned().unwrap_or(json!(false)),
    );
    let default_max_tokens = match flavor {
        Flavor::DeepSeek => 256_000,
        Flavor::Anthropic => anthropic_messages::DEFAULT_MAX_TOKENS,
    };
    body.insert(
        "max_tokens".into(),
        chat.get("max_tokens")
            .cloned()
            .unwrap_or(json!(default_max_tokens)),
    );
    body.insert("messages".into(), Value::Array(messages));
    let system = system
        .into_iter()
        .filter(|text| keeps_text(text, flavor))
        .collect::<Vec<_>>();
    match flavor {
        Flavor::DeepSeek if !system.is_empty() => {
            body.insert("system".into(), json!(system.join("\n\n")));
        }
        // One block per leading system message, so the last can carry the
        // cache breakpoint and each stays byte-stable across hops.
        Flavor::Anthropic if !system.is_empty() => {
            let blocks = system.iter().map(|text| text_block(text)).collect();
            body.insert("system".into(), Value::Array(blocks));
        }
        _ => {}
    }
    // The first-party API takes no sampling controls and sets its own thinking
    // (`anthropic_messages::finish_body`).
    if flavor == Flavor::DeepSeek {
        if let Some(thinking) = chat.get("thinking") {
            body.insert("thinking".into(), thinking.clone());
        }
        let thinking_on = chat
            .pointer("/thinking/type")
            .and_then(Value::as_str)
            .is_none_or(|kind| kind != "disabled");
        if thinking_on && let Some(effort) = chat.get("reasoning_effort") {
            body.insert("output_config".into(), json!({"effort": effort}));
        }
        for key in ["temperature", "top_p"] {
            if let Some(value) = chat.get(key) {
                body.insert(key.into(), value.clone());
            }
        }
    }
    if let Some(stop) = chat.get("stop") {
        let stop = match (flavor, stop) {
            (Flavor::Anthropic, Value::String(one)) => json!([one]),
            _ => stop.clone(),
        };
        body.insert("stop_sequences".into(), stop);
    }
    if let Some(tools) = chat.get("tools").and_then(Value::as_array) {
        let tools = tools
            .iter()
            .map(|tool| {
                let function = tool.get("function").unwrap_or(tool);
                json!({
                    "name": function.get("name").cloned().unwrap_or(Value::Null),
                    "description": function.get("description").cloned().unwrap_or(json!("")),
                    "input_schema": function.get("parameters").cloned()
                        .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
                })
            })
            .collect::<Vec<_>>();
        if !tools.is_empty() {
            body.insert("tools".into(), Value::Array(tools));
        }
    }
    if flavor == Flavor::Anthropic {
        anthropic_messages::finish_body(&mut body, chat)?;
    }
    Ok(Value::Object(body))
}

fn finish_reason(stop_reason: &str) -> &'static str {
    match stop_reason {
        "tool_use" => "tool_calls",
        "max_tokens" | "model_context_window_exceeded" => "length",
        _ => "stop",
    }
}

/// Messages usage (`input_tokens` is uncached, as DeepSeek's harness reads it)
/// in the Chat fields DeepSeek reports there: prompt = hit + miss.
fn chat_usage(usage: &Map<String, Value>) -> Value {
    let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let (input, read, write) = (
        count("input_tokens"),
        count("cache_read_input_tokens"),
        count("cache_creation_input_tokens"),
    );
    json!({
        "prompt_tokens": input + read + write,
        "completion_tokens": count("output_tokens"),
        "prompt_cache_hit_tokens": read,
        "prompt_cache_miss_tokens": input + write,
    })
}

fn frame(delta: Value) -> Option<String> {
    Some(format!(
        "data: {}",
        json!({"choices": [{"index": 0, "delta": delta}]})
    ))
}

fn flavor_usage(flavor: Flavor, usage: &Map<String, Value>) -> Value {
    match flavor {
        Flavor::DeepSeek => chat_usage(usage),
        Flavor::Anthropic => anthropic_messages::chat_usage(usage),
    }
}

/// One Messages stream, translated line by line into Chat Completions frames.
#[derive(Default)]
pub(super) struct StreamTranslator {
    flavor: Flavor,
    usage: Map<String, Value>,
    /// Content-block index → ordinal among this reply's tool calls.
    tools: HashMap<u64, (u64, bool)>,
    /// The first-party API's signed thinking blocks, for replay.
    thinking: anthropic_messages::ThinkingCapture,
}

impl StreamTranslator {
    pub(super) fn new(flavor: Flavor) -> Self {
        Self {
            flavor,
            ..Self::default()
        }
    }

    /// The reply's thinking blocks as the private reasoning slot keeps them
    /// (first-party flavor only; DeepSeek replays the accumulated text).
    pub(super) fn take_replay(&mut self) -> Option<String> {
        if self.flavor == Flavor::Anthropic {
            self.thinking.take_json()
        } else {
            None
        }
    }

    /// The provider's own usage fields, for the wire log (first-party only).
    pub(super) fn native_usage(&self) -> Option<Value> {
        (self.flavor == Flavor::Anthropic && !self.usage.is_empty())
            .then(|| Value::Object(self.usage.clone()))
    }

    /// The Chat line for one Messages SSE line: `None` for a line that carries
    /// nothing (the `event:` name, a block's close), an SSE comment for a ping.
    pub(super) fn line(&mut self, line: &str) -> Option<String> {
        let payload = line.trim().strip_prefix("data:")?.trim();
        let event: Value = serde_json::from_str(payload).ok()?;
        let index = event.get("index").and_then(Value::as_u64).unwrap_or(0);
        match event.get("type").and_then(Value::as_str)? {
            "message_start" => {
                if let Some(usage) = event.pointer("/message/usage").and_then(Value::as_object) {
                    self.usage.extend(usage.clone());
                }
                None
            }
            "content_block_start" => {
                let block = event.get("content_block")?;
                if self.flavor == Flavor::Anthropic {
                    self.thinking.start(index, block);
                }
                match block.get("type").and_then(Value::as_str)? {
                    "text" => block
                        .get("text")
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                        .and_then(|text| frame(json!({"content": text}))),
                    "thinking" => block
                        .get("thinking")
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                        .and_then(|text| frame(json!({"reasoning_content": text}))),
                    "tool_use" => {
                        let ordinal = self.tools.len() as u64;
                        self.tools.insert(index, (ordinal, false));
                        frame(json!({"tool_calls": [{
                            "index": ordinal,
                            "id": block.get("id"),
                            "type": "function",
                            "function": {"name": block.get("name"), "arguments": ""},
                        }]}))
                    }
                    _ => None,
                }
            }
            "content_block_delta" => {
                let delta = event.get("delta")?;
                match delta.get("type").and_then(Value::as_str)? {
                    "text_delta" => frame(json!({"content": delta.get("text")})),
                    "thinking_delta" => {
                        if self.flavor == Flavor::Anthropic {
                            let text = delta.get("thinking").and_then(Value::as_str);
                            self.thinking.thinking(index, text.unwrap_or_default());
                        }
                        frame(json!({"reasoning_content": delta.get("thinking")}))
                    }
                    "signature_delta" => {
                        if self.flavor == Flavor::Anthropic {
                            let signature = delta.get("signature").and_then(Value::as_str);
                            self.thinking
                                .signature(index, signature.unwrap_or_default());
                        }
                        None
                    }
                    "input_json_delta" => {
                        let (ordinal, streamed) = self.tools.get_mut(&index)?;
                        *streamed = true;
                        let ordinal = *ordinal;
                        frame(json!({"tool_calls": [{
                            "index": ordinal,
                            "function": {"arguments": delta.get("partial_json")},
                        }]}))
                    }
                    _ => None,
                }
            }
            "content_block_stop" => {
                // A call with no parameters streams no JSON; its input is `{}`.
                let (ordinal, streamed) = *self.tools.get(&index)?;
                (!streamed).then_some(())?;
                frame(json!({"tool_calls": [{"index": ordinal, "function": {"arguments": "{}"}}]}))
            }
            "message_delta" => {
                if let Some(usage) = event.get("usage").and_then(Value::as_object) {
                    self.usage.extend(usage.clone());
                }
                let stop_reason = event.pointer("/delta/stop_reason").and_then(Value::as_str);
                let usage = flavor_usage(self.flavor, &self.usage);
                if self.flavor == Flavor::Anthropic && stop_reason == Some("refusal") {
                    let details = event
                        .pointer("/delta/stop_details")
                        .or_else(|| event.get("stop_details"));
                    return Some(format!(
                        "data: {}",
                        json!({"error": anthropic_messages::refusal_error(details), "usage": usage})
                    ));
                }
                let reason = stop_reason.map(finish_reason);
                Some(format!(
                    "data: {}",
                    json!({"choices": [{"index": 0, "delta": {}, "finish_reason": reason}],
                        "usage": usage})
                ))
            }
            "message_stop" => Some("data: [DONE]".to_string()),
            "error" => Some(format!("data: {}", json!({"error": event.get("error")}))),
            "ping" => Some(": ping".to_string()),
            _ => None,
        }
    }
}

/// A non-streaming Messages response as a Chat Completions response.
#[cfg(test)]
pub(super) fn to_chat_response(message: &Value) -> Value {
    to_chat_response_for(message, Flavor::DeepSeek)
}

/// A non-streaming Messages response of this flavor as a Chat Completions
/// response.
pub(super) fn to_chat_response_for(message: &Value, flavor: Flavor) -> Value {
    if message.get("type").and_then(Value::as_str) == Some("error") {
        return json!({"error": message.get("error")});
    }
    let usage = message
        .get("usage")
        .and_then(Value::as_object)
        .map(|usage| flavor_usage(flavor, usage))
        .unwrap_or(Value::Null);
    if flavor == Flavor::Anthropic
        && message.get("stop_reason").and_then(Value::as_str) == Some("refusal")
    {
        let error = anthropic_messages::refusal_error(message.get("stop_details"));
        return json!({"error": error, "usage": usage});
    }
    let (mut text, mut thinking, mut calls) = (String::new(), String::new(), Vec::new());
    for block in message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => text.push_str(block["text"].as_str().unwrap_or_default()),
            Some("thinking") => thinking.push_str(block["thinking"].as_str().unwrap_or_default()),
            Some("tool_use") => calls.push(json!({
                "id": block.get("id"),
                "type": "function",
                "function": {"name": block.get("name"),
                    "arguments": block.get("input").map(Value::to_string).unwrap_or_else(|| "{}".into())},
            })),
            _ => {}
        }
    }
    if flavor == Flavor::Anthropic {
        // The signed blocks themselves are what the next hop replays.
        let content = message.get("content").and_then(Value::as_array);
        thinking = content
            .and_then(|content| anthropic_messages::replay_json(content))
            .unwrap_or_default();
    }
    let mut chat_message = json!({"role": "assistant", "content": text});
    if !thinking.is_empty() {
        chat_message["reasoning_content"] = json!(thinking);
    }
    if !calls.is_empty() {
        chat_message["tool_calls"] = json!(calls);
    }
    json!({
        "choices": [{"index": 0, "message": chat_message,
            "finish_reason": message.get("stop_reason").and_then(Value::as_str).map(finish_reason)}],
        "usage": usage,
    })
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/club/http__deepseek_messages_tests.rs"]
mod tests;
