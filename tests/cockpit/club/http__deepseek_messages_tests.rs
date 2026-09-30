use super::*;
use serde_json::json;

/// The Chat body a DeepSeek seat builds, as DeepSeek's Messages API takes it
/// (the shapes of deepseek-harness `serialize.ts`).
#[test]
fn a_chat_body_becomes_deepseek_messages() {
    let chat = json!({
        "model": "deepseek-flash", "stream": true, "max_tokens": 256000,
        "thinking": {"type": "enabled"}, "reasoning_effort": "high",
        "stream_options": {"include_usage": true}, "prompt_cache_key": "k",
        "messages": [
            {"role": "system", "content": "core"},
            {"role": "system", "content": "brevity"},
            {"role": "user", "content": "fix it"},
            {"role": "assistant", "content": "", "reasoning_content": "look first",
             "tool_calls": [
                {"id": "a", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"x\"}"}},
                {"id": "b", "type": "function", "function": {"name": "grep", "arguments": "not json"}}]},
            {"role": "tool", "tool_call_id": "a", "content": "source"},
            {"role": "tool", "tool_call_id": "b", "content": ""},
            {"role": "user", "content": "⠺⠁ note"},
            {"role": "assistant", "content": "done", "reasoning_content": ""},
            {"role": "user", "content": "and now?"},
            {"role": "system", "content": "contract"}
        ],
        "tools": [{"type": "function", "function": {"name": "read_file", "description": "read",
            "parameters": {"type": "object", "properties": {"path": {"type": "string"}}}}}]
    });
    let body = to_messages_body(&chat).unwrap();
    assert_eq!(body["system"], "core\n\nbrevity");
    assert_eq!(body["max_tokens"], 256000);
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["output_config"]["effort"], "high");
    for gone in ["stream_options", "prompt_cache_key", "reasoning_effort"] {
        assert!(body.get(gone).is_none(), "{gone}");
    }
    assert_eq!(body["tools"][0]["name"], "read_file");
    assert_eq!(
        body["tools"][0]["input_schema"]["properties"]["path"]["type"],
        "string"
    );
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(
        messages[0],
        json!({"role": "user", "content": [{"type": "text", "text": "fix it"}]})
    );
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(
        messages[1]["content"][0],
        json!({"type": "thinking", "thinking": "look first"})
    );
    assert_eq!(messages[1]["content"][1]["input"], json!({"path": "x"}));
    assert_eq!(
        messages[1]["content"][2]["input"],
        json!({}),
        "malformed history goes as {{}}"
    );
    // Both results, then the harness note, in one user message; results first.
    let results = messages[2]["content"].as_array().unwrap();
    assert_eq!(results[0]["type"], "tool_result");
    assert_eq!(results[0]["tool_use_id"], "a");
    assert_eq!(results[1]["content"], json!([]));
    assert_eq!(results[2]["text"], "⠺⠁ note");
    // An answer without reasoning carries no empty thinking block.
    assert_eq!(
        messages[3]["content"],
        json!([{"type": "text", "text": "done"}])
    );
    // deepseek-flash takes a later system message in history, after the user turn.
    assert_eq!(
        messages[5],
        json!({"role": "system", "content": [{"type": "text", "text": "contract"}]})
    );
    // A leading-only route folds it into the user turn instead.
    let mut pro = chat.clone();
    pro["model"] = json!("deepseek-v4-pro");
    let body = to_messages_body(&pro).unwrap();
    let last = body["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["role"], "user");
    assert_eq!(last["content"][1]["text"], "contract");
    // Thinking off sends no effort.
    let mut off = chat;
    off["thinking"] = json!({"type": "disabled"});
    assert!(
        to_messages_body(&off)
            .unwrap()
            .get("output_config")
            .is_none()
    );
}

/// A Messages stream becomes the Chat frames the accumulator reads.
#[test]
fn a_messages_stream_becomes_chat_frames() {
    let mut stream = StreamTranslator::default();
    let mut line = |event: serde_json::Value| stream.line(&format!("data: {event}"));
    let frame = |line: Option<String>| -> serde_json::Value {
        serde_json::from_str(line.unwrap().strip_prefix("data: ").unwrap()).unwrap()
    };
    assert!(line(json!({"type": "message_start", "message": {"usage": {"input_tokens": 30, "cache_read_input_tokens": 70}}})).is_none());
    assert!(line(json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}})).is_none());
    assert_eq!(
        frame(line(
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "hm"}})
        ))["choices"][0]["delta"]["reasoning_content"],
        "hm"
    );
    assert!(line(json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "s"}})).is_none());
    assert!(line(json!({"type": "content_block_stop", "index": 0})).is_none());
    assert_eq!(
        frame(line(
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "ok"}})
        ))["choices"][0]["delta"]["content"],
        "ok"
    );
    let start = frame(line(
        json!({"type": "content_block_start", "index": 2, "content_block": {"type": "tool_use", "id": "t1", "name": "read_file", "input": {}}}),
    ));
    assert_eq!(start["choices"][0]["delta"]["tool_calls"][0]["index"], 0);
    assert_eq!(
        start["choices"][0]["delta"]["tool_calls"][0]["function"]["name"],
        "read_file"
    );
    let args = frame(line(
        json!({"type": "content_block_delta", "index": 2, "delta": {"type": "input_json_delta", "partial_json": "{\"path\""}}),
    ));
    assert_eq!(
        args["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
        "{\"path\""
    );
    assert!(line(json!({"type": "content_block_stop", "index": 2})).is_none());
    // A call with no parameters streams no JSON; it closes as `{}`.
    let _ = line(
        json!({"type": "content_block_start", "index": 3, "content_block": {"type": "tool_use", "id": "t2", "name": "list_dir", "input": {}}}),
    );
    let empty = frame(line(json!({"type": "content_block_stop", "index": 3})));
    assert_eq!(empty["choices"][0]["delta"]["tool_calls"][0]["index"], 1);
    assert_eq!(
        empty["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
        "{}"
    );
    let end = frame(line(
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 9}}),
    ));
    assert_eq!(end["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(
        end["usage"],
        json!({"prompt_tokens": 100, "completion_tokens": 9,
        "prompt_cache_hit_tokens": 70, "prompt_cache_miss_tokens": 30})
    );
    assert_eq!(
        line(json!({"type": "message_stop"})).as_deref(),
        Some("data: [DONE]")
    );
    assert_eq!(line(json!({"type": "ping"})).as_deref(), Some(": ping"));
    let error = frame(line(
        json!({"type": "error", "error": {"type": "overloaded_error", "message": "busy"}}),
    ));
    assert_eq!(error["error"]["message"], "busy");
    assert!(stream.line("event: message_stop").is_none());
}

#[test]
fn a_messages_response_becomes_a_chat_response() {
    let chat = to_chat_response(&json!({
        "type": "message", "role": "assistant", "stop_reason": "tool_use",
        "content": [{"type": "thinking", "thinking": "plan"}, {"type": "text", "text": "reading"},
            {"type": "tool_use", "id": "t", "name": "read_file", "input": {"path": "a"}}],
        "usage": {"input_tokens": 5, "cache_read_input_tokens": 15, "output_tokens": 3}
    }));
    let message = &chat["choices"][0]["message"];
    assert_eq!(message["reasoning_content"], "plan");
    assert_eq!(message["content"], "reading");
    assert_eq!(
        message["tool_calls"][0]["function"]["arguments"],
        "{\"path\":\"a\"}"
    );
    assert_eq!(chat["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(chat["usage"]["prompt_tokens"], 20);
    assert_eq!(chat["usage"]["prompt_cache_miss_tokens"], 5);
}

#[test]
fn the_messages_endpoint_follows_the_seat() {
    let _guard = crate::tests::env_lock();
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_DEEPSEEK_MESSAGES_URL");
    assert_eq!(
        messages_url("https://api.deepseek.com/v1"),
        "https://api.deepseek.com/anthropic/v1/messages"
    );
    assert_eq!(
        messages_url("http://127.0.0.1:9/v1/"),
        "http://127.0.0.1:9/v1/messages"
    );
}
