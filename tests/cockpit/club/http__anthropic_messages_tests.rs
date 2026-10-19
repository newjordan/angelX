use super::super::deepseek_messages::{
    Flavor, StreamTranslator, to_chat_response_for, to_messages_body_for,
};
use super::*;
use crate::agent::club::ClubReply;
use crate::agent::club::sse::{SseEvent, StreamAccumulator, parse_sse_line};
use serde_json::json;

const RECORDED_TOOL_STREAM: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/anthropic/sonnet-5-5-tool-stream.sse"
));

fn thinking_block() -> Value {
    json!({"type": "thinking", "thinking": "", "signature": "EqQBsig/1=="})
}

fn redacted_block() -> Value {
    json!({"type": "redacted_thinking", "data": "EmwKredacted+/=="})
}

#[test]
fn the_messages_url_never_doubles_v1() {
    for (base, url) in [
        (
            "https://api.anthropic.com",
            "https://api.anthropic.com/v1/messages",
        ),
        (
            "https://api.anthropic.com/",
            "https://api.anthropic.com/v1/messages",
        ),
        (
            "http://127.0.0.1:8123/v1",
            "http://127.0.0.1:8123/v1/messages",
        ),
        (
            "http://127.0.0.1:8123/v1/",
            "http://127.0.0.1:8123/v1/messages",
        ),
        (
            "http://127.0.0.1:8123/v1/messages",
            "http://127.0.0.1:8123/v1/messages",
        ),
    ] {
        assert_eq!(messages_url(base), url, "{base}");
    }
}

#[test]
fn every_request_names_its_key_version_beta_and_workspace() {
    let headers = request_headers("sk-test", None);
    assert_eq!(
        headers,
        vec![
            ("x-api-key", "sk-test".to_string()),
            ("anthropic-version", "2023-06-01".to_string()),
            (
                "anthropic-beta",
                "thinking-binding-controls-2026-08-01".to_string()
            ),
        ]
    );
    assert!(!headers.iter().any(|(name, _)| *name == "authorization"));
    let scoped = request_headers("sk-test", Some(" wrkspc_01 "));
    assert_eq!(
        scoped.last(),
        Some(&("anthropic-workspace-id", "wrkspc_01".to_string()))
    );
    assert_eq!(request_headers("sk-test", Some("  ")).len(), 3);
}

#[test]
fn effort_takes_the_api_rungs() {
    for rung in ["low", "medium", "high", "xhigh", "max"] {
        assert_eq!(effort(Some(rung)), rung);
    }
    assert_eq!(effort(Some(" HIGH ")), "high");
    assert_eq!(effort(None), "medium");
    assert_eq!(effort(Some("none")), "low", "thinking cannot be turned off");
    assert_eq!(effort(Some("ultra")), "medium");
}

/// The Chat body the seat builds, as the first-party Messages API takes it.
#[test]
fn a_chat_body_becomes_an_anthropic_messages_body() {
    let replay = json!([thinking_block(), redacted_block()]).to_string();
    let chat = json!({
        "model": "claude-sonnet-5-5", "stream": true,
        "reasoning_effort": "high", "thinking": {"type": "enabled"},
        "temperature": 0.2, "top_p": 0.9, "top_k": 5, "tool_choice": "required",
        "stream_options": {"include_usage": true}, "prompt_cache_key": "k",
        "stop": "STOP",
        "messages": [
            {"role": "system", "content": "core"},
            {"role": "system", "content": "brevity"},
            {"role": "user", "content": "fix it"},
            {"role": "assistant", "content": "Reading both.", "reasoning_content": replay,
             "tool_calls": [
                {"id": "a", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"x\"}"}},
                {"id": "b", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"y\"}"}}]},
            {"role": "tool", "tool_call_id": "a", "content": "source"},
            {"role": "tool", "tool_call_id": "b", "content": ""},
            {"role": "user", "content": "⠺⠁ note"},
            // Another seat's plain-text reasoning cannot be signed: dropped.
            {"role": "assistant", "content": "done", "reasoning_content": "foreign words"},
            {"role": "user", "content": "   "},
            {"role": "user", "content": "and now?"},
            {"role": "system", "content": "contract"},
            // A trailing assistant turn is a prefill: never sent.
            {"role": "assistant", "content": "Sure, "}
        ],
        "tools": [
            {"type": "function", "function": {"name": "read_file", "description": "read",
                "parameters": {"type": "object", "properties": {"path": {"type": "string"}}}}},
            {"type": "function", "function": {"name": "grep", "description": "search",
                "parameters": {"type": "object", "properties": {}}}}
        ]
    });
    let body = to_messages_body_for(&chat, Flavor::Anthropic).unwrap();
    assert_eq!(
        body["thinking"],
        json!({"type": "adaptive", "block_binding": {"prefix_mismatch_behavior": "drop_block"}})
    );
    assert_eq!(body["output_config"], json!({"effort": "high"}));
    assert_eq!(body["max_tokens"], 32000, "the API requires a cap");
    assert_eq!(body["stop_sequences"], json!(["STOP"]));
    for gone in [
        "temperature",
        "top_p",
        "top_k",
        "tool_choice",
        "stream_options",
        "prompt_cache_key",
        "reasoning_effort",
    ] {
        assert!(body.get(gone).is_none(), "{gone}");
    }
    // System: one block per leading message, the breakpoint on the last.
    assert_eq!(
        body["system"],
        json!([{"type": "text", "text": "core"},
            {"type": "text", "text": "brevity", "cache_control": {"type": "ephemeral"}}])
    );
    // Tools: the breakpoint on the last definition only.
    assert!(body["tools"][0].get("cache_control").is_none());
    assert_eq!(
        body["tools"][1]["cache_control"],
        json!({"type": "ephemeral"})
    );
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "user");
    // The tool-call turn: its signed blocks verbatim and first, then its text
    // and calls.
    let turn = messages[1]["content"].as_array().unwrap();
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(turn[0], thinking_block());
    assert_eq!(turn[1], redacted_block());
    assert_eq!(turn[2], json!({"type": "text", "text": "Reading both."}));
    assert_eq!(turn[3]["type"], "tool_use");
    assert_eq!(turn[3]["input"], json!({"path": "x"}));
    assert_eq!(turn[4]["id"], "b");
    // Both results lead one user message; an empty result carries no content.
    let results = messages[2]["content"].as_array().unwrap();
    assert_eq!(results[0]["tool_use_id"], "a");
    assert_eq!(
        results[0]["content"],
        json!([{"type": "text", "text": "source"}])
    );
    assert_eq!(
        results[1],
        json!({"type": "tool_result", "tool_use_id": "b"})
    );
    assert_eq!(results[2]["text"], "⠺⠁ note");
    assert_eq!(
        messages[3]["content"],
        json!([{"type": "text", "text": "done"}])
    );
    // The whitespace turn is gone; the later system message folds into the
    // last user turn, which ends the request and carries the moving breakpoint.
    let last = messages.last().unwrap();
    assert_eq!(messages.len(), 5);
    assert_eq!(last["role"], "user");
    assert_eq!(
        last["content"][0],
        json!({"type": "text", "text": "and now?"})
    );
    assert_eq!(
        last["content"][1],
        json!({"type": "text", "text": "contract", "cache_control": {"type": "ephemeral"}})
    );
    assert_eq!(body.to_string().matches("cache_control").count(), 3);
    assert!(!body.to_string().contains("foreign words"));
}

#[test]
fn no_effort_named_sends_medium() {
    let chat = json!({"model": "claude-sonnet-5-5", "max_tokens": 32000,
        "messages": [{"role": "user", "content": "hi"}]});
    let body = to_messages_body_for(&chat, Flavor::Anthropic).unwrap();
    assert_eq!(body["output_config"]["effort"], "medium");
    assert!(body.get("system").is_none());
    assert!(body.get("tools").is_none());
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    let only_assistant = json!({"model": "m", "messages": [{"role": "assistant", "content": "x"}]});
    assert!(to_messages_body_for(&only_assistant, Flavor::Anthropic).is_err());
}

/// A recorded tool hop: the accumulator sees two calls and the usage split,
/// and the reply's signed blocks come back verbatim for the next hop.
#[test]
fn a_recorded_stream_replays_its_signed_thinking() {
    let mut stream = StreamTranslator::new(Flavor::Anthropic);
    let mut acc = StreamAccumulator::default();
    let mut usage = Value::Null;
    let mut done = false;
    for line in RECORDED_TOOL_STREAM.lines() {
        let translated = stream.line(line).unwrap_or_default();
        match parse_sse_line(&translated) {
            SseEvent::Done => done = true,
            SseEvent::Chunk(chunk) => {
                assert!(chunk.get("error").is_none(), "{chunk}");
                if let Some(frame_usage) = chunk.get("usage") {
                    usage = frame_usage.clone();
                }
                acc.apply_chunk(&chunk);
            }
            SseEvent::Ignore => {}
        }
    }
    assert!(done);
    assert_eq!(acc.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(acc.content, "Reading both files.");
    assert_eq!(
        usage,
        json!({"prompt_tokens": 22772, "completion_tokens": 187,
            "cache_read_input_tokens": 20480, "cache_creation_input_tokens": 1880})
    );
    // angelX's accounting reads the uncached input as prompt - read - write.
    let parsed = crate::agent::club::usage::parse_http_usage(&usage).unwrap();
    assert_eq!(
        (parsed.input, parsed.cache_read, parsed.cache_write),
        (Some(22772), Some(20480), Some(1880))
    );
    let replay: Value = serde_json::from_str(&stream.take_replay().unwrap()).unwrap();
    assert_eq!(
        replay,
        json!([
            {"type": "thinking", "thinking": "", "signature": "EqQBCkYIBxgCKkBfixturesignatureAAAA+/=="},
            {"type": "redacted_thinking", "data": "EmwKAhgBEgy3fixtureredacteddataBBBB+/=="}
        ])
    );
    assert_eq!(
        stream.native_usage().unwrap()["cache_read_input_tokens"],
        20480
    );
    match acc.into_reply(true) {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].id, "toolu_01FixtureA");
            assert_eq!(calls[0].args, json!({"path": "src/lib.rs"}));
            assert_eq!(calls[1].args, json!({"path": "tests/lib.rs"}));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn thinking_text_and_signature_deltas_assemble_verbatim() {
    let mut stream = StreamTranslator::new(Flavor::Anthropic);
    let mut line = |event: Value| stream.line(&format!("data: {event}"));
    assert!(
        line(json!({"type": "content_block_start", "index": 0,
        "content_block": {"type": "thinking", "thinking": "", "signature": ""}}))
        .is_none()
    );
    let shown = line(json!({"type": "content_block_delta", "index": 0,
        "delta": {"type": "thinking_delta", "thinking": "plan "}}));
    assert!(shown.unwrap().contains("\"reasoning_content\":\"plan \""));
    let _ = line(json!({"type": "content_block_delta", "index": 0,
        "delta": {"type": "thinking_delta", "thinking": "it"}}));
    assert!(
        line(json!({"type": "content_block_delta", "index": 0,
        "delta": {"type": "signature_delta", "signature": "c2ln"}}))
        .is_none()
    );
    let _ = line(json!({"type": "content_block_stop", "index": 0}));
    let replay: Value = serde_json::from_str(&stream.take_replay().unwrap()).unwrap();
    assert_eq!(
        replay,
        json!([{"type": "thinking", "thinking": "plan it", "signature": "c2ln"}])
    );
    // A block that never got its signature cannot be replayed.
    let mut unsigned = StreamTranslator::new(Flavor::Anthropic);
    let _ = unsigned.line(r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"x","signature":""}}"#);
    assert!(unsigned.take_replay().is_none());
    // DeepSeek's flavor keeps replaying its accumulated text.
    assert!(
        StreamTranslator::new(Flavor::DeepSeek)
            .take_replay()
            .is_none()
    );
}

/// A refusal is an error naming the classifier's category, streamed or not.
#[test]
fn a_refusal_names_its_category() {
    let mut stream = StreamTranslator::new(Flavor::Anthropic);
    let refusal = stream
        .line(&format!(
            "data: {}",
            json!({"type": "message_delta",
                "delta": {"stop_reason": "refusal", "stop_details":
                    {"type": "refusal", "category": "cyber", "explanation": "declined"}},
                "usage": {"input_tokens": 10, "output_tokens": 2}})
        ))
        .unwrap();
    let SseEvent::Chunk(chunk) = parse_sse_line(&refusal) else {
        panic!("{refusal}");
    };
    let error = crate::agent::club::retry::extract_api_error(&chunk).unwrap();
    assert!(error.contains("stop_details.category: cyber"), "{error}");
    assert!(error.contains("declined"), "{error}");
    assert_eq!(chunk["usage"]["prompt_tokens"], 10);
    let whole = to_chat_response_for(
        &json!({"type": "message", "stop_reason": "refusal", "content": [],
            "stop_details": {"type": "refusal", "category": null},
            "usage": {"input_tokens": 3, "output_tokens": 0}}),
        Flavor::Anthropic,
    );
    let error = crate::agent::club::retry::extract_api_error(&whole).unwrap();
    assert!(
        error.contains("stop_details.category: unspecified"),
        "{error}"
    );
    // A cut-off reply is a length stop.
    let mut stream = StreamTranslator::new(Flavor::Anthropic);
    let cut = stream
        .line(r#"data: {"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":32000}}"#)
        .unwrap();
    assert!(cut.contains("\"finish_reason\":\"length\""), "{cut}");
}

#[test]
fn a_whole_reply_keeps_its_signed_blocks() {
    let chat = to_chat_response_for(
        &json!({"type": "message", "role": "assistant", "stop_reason": "tool_use",
            "content": [thinking_block(), {"type": "text", "text": "reading"},
                {"type": "tool_use", "id": "t", "name": "read_file", "input": {"path": "a"}}],
            "usage": {"input_tokens": 5, "cache_read_input_tokens": 15,
                "cache_creation_input_tokens": 7, "output_tokens": 3}}),
        Flavor::Anthropic,
    );
    let message = &chat["choices"][0]["message"];
    let replay: Value =
        serde_json::from_str(message["reasoning_content"].as_str().unwrap()).unwrap();
    assert_eq!(replay, json!([thinking_block()]));
    assert_eq!(message["content"], "reading");
    assert_eq!(chat["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(chat["usage"]["prompt_tokens"], 27);
    assert_eq!(chat["usage"]["cache_creation_input_tokens"], 7);
}

/// One request as the loopback server read it: request line, headers, body.
struct Seen {
    head: String,
    body: Value,
}

fn read_request(socket: &mut std::net::TcpStream) -> Option<Seen> {
    use std::io::Read;
    let mut raw = Vec::new();
    let mut byte = [0u8; 1];
    while !raw.ends_with(b"\r\n\r\n") {
        if socket.read(&mut byte).ok()? == 0 {
            return None;
        }
        raw.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&raw).to_string();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    socket.read_exact(&mut body).ok()?;
    Some(Seen {
        head,
        body: serde_json::from_slice(&body).unwrap_or(Value::Null),
    })
}

/// Answer each POST with the next scripted SSE stream; anything else is a 404.
fn serve_streams(
    streams: Vec<String>,
) -> (
    String,
    std::sync::mpsc::Receiver<Seen>,
    std::thread::JoinHandle<()>,
) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let (seen_tx, seen_rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let mut streams = streams.into_iter();
        let mut next = streams.next();
        while let Some(stream) = next.take() {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let Some(seen) = read_request(&mut socket) else {
                next = Some(stream);
                continue;
            };
            if !seen.head.starts_with("POST") {
                let _ = socket.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                next = Some(stream);
                continue;
            }
            let _ = seen_tx.send(seen);
            let _ = socket.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{stream}",
                    stream.len()
                )
                .as_bytes(),
            );
            next = streams.next();
        }
    });
    (base, seen_rx, handle)
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// The seat as the bench launches it (env only), over a loopback server: the
/// first hop's request carries the whole first-party contract, and the second
/// replays the first reply's signed thinking verbatim, first in its turn.
#[test]
fn the_seat_round_trips_its_signed_thinking_on_the_wire() {
    use crate::agent::club::{ChatMsg, ToolDef};
    use crate::tests::TestEnvGuard;
    let _guard = crate::tests::env_lock();
    let answer = [
        r#"data: {"type":"message_start","message":{"usage":{"input_tokens":9,"cache_read_input_tokens":22000,"cache_creation_input_tokens":300,"output_tokens":1}}}"#,
        r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":"Ehop2=="}}"#,
        r#"data: {"type":"content_block_stop","index":0}"#,
        r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
        r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Fixed."}}"#,
        r#"data: {"type":"content_block_stop","index":1}"#,
        r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":12}}"#,
        r#"data: {"type":"message_stop"}"#,
    ]
    .join("\n\n")
        + "\n\n";
    let (base, seen, server) = serve_streams(vec![RECORDED_TOOL_STREAM.to_string(), answer]);
    let _env = [
        TestEnvGuard::set("ANGEL_API_CLUBS", "anthropic"),
        TestEnvGuard::set("ANGEL_ANTHROPIC_URL", &base),
        TestEnvGuard::set("ANGEL_ANTHROPIC_KEY", "sk-ant-test"),
        TestEnvGuard::set("ANGEL_ANTHROPIC_WORKSPACE_ID", "wrkspc_test"),
        TestEnvGuard::set("ANGEL_ANTHROPIC_EFFORT", "xhigh"),
        TestEnvGuard::set("ANGEL_ANTHROPIC_MAX_TOKENS", "32000"),
        TestEnvGuard::unset("ANGEL_ANTHROPIC_MODEL"),
        TestEnvGuard::unset("ANGEL_ANTHROPIC_REASONING_EFFORT"),
        TestEnvGuard::unset("ANGEL_REASONING_EFFORT"),
        TestEnvGuard::unset("ANGEL_CLUB_MAX_TOKENS"),
    ];
    super::super::resync_reasoning_effort_env_from_env();
    super::super::resync_max_tokens_env_from_env();
    let (alias, club, _) = crate::agent::club::optional_anthropic_http_club().unwrap();
    assert_eq!(alias, "anthropic");
    assert_eq!(club.label(), "claude-sonnet-5-5");
    let before = club.usage_accounting();
    let tools = vec![ToolDef {
        name: "read_file".into(),
        description: "read a file".into(),
        params: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
    }];
    let mut history = vec![
        ChatMsg::system("You fix code."),
        ChatMsg::user("Fix the failing test."),
    ];
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let reply = club
        .chat_streaming(&history, &tools, &cancel, &mut |_| {})
        .unwrap();
    let ClubReply::Calls(calls) = reply else {
        panic!("{reply:?}");
    };
    assert_eq!(calls.len(), 2);
    let reasoning = crate::agent::club::take_pending_tool_reasoning();
    let content = crate::agent::club::take_pending_tool_content();
    assert_eq!(content.as_deref(), Some("Reading both files."));

    let first = seen.recv().unwrap();
    assert!(
        first.head.starts_with("POST /v1/messages "),
        "{}",
        first.head
    );
    assert_eq!(header(&first.head, "x-api-key"), Some("sk-ant-test"));
    assert_eq!(header(&first.head, "anthropic-version"), Some("2023-06-01"));
    assert_eq!(
        header(&first.head, "anthropic-beta"),
        Some("thinking-binding-controls-2026-08-01")
    );
    assert_eq!(
        header(&first.head, "anthropic-workspace-id"),
        Some("wrkspc_test")
    );
    assert!(header(&first.head, "authorization").is_none());
    let body = &first.body;
    assert_eq!(body["model"], "claude-sonnet-5-5");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 32000);
    assert_eq!(body["output_config"], json!({"effort": "xhigh"}));
    assert_eq!(
        body["thinking"]["block_binding"]["prefix_mismatch_behavior"],
        "drop_block"
    );
    for gone in [
        "temperature",
        "top_p",
        "top_k",
        "tool_choice",
        "reasoning_effort",
    ] {
        assert!(body.get(gone).is_none(), "{gone}");
    }
    assert_eq!(
        body["tools"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    let system = body["system"].as_array().unwrap();
    assert_eq!(system[0]["text"], "You fix code.");
    assert_eq!(
        system.last().unwrap()["cache_control"],
        json!({"type": "ephemeral"})
    );
    let last_block = body["messages"].as_array().unwrap().last().unwrap()["content"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(last_block["cache_control"], json!({"type": "ephemeral"}));

    history.push(ChatMsg::assistant_calls_full(
        calls.clone(),
        reasoning,
        content,
    ));
    history.push(ChatMsg::tool(calls[0].id.clone(), "pub fn add() {}"));
    history.push(ChatMsg::tool(calls[1].id.clone(), "#[test] fn adds() {}"));
    let reply = club
        .chat_streaming(&history, &tools, &cancel, &mut |_| {})
        .unwrap();
    assert!(
        matches!(&reply, ClubReply::Text(text) if text == "Fixed."),
        "{reply:?}"
    );
    let second = seen.recv().unwrap().body;
    let messages = second["messages"].as_array().unwrap();
    let turn = messages
        .iter()
        .find(|message| message["role"] == "assistant")
        .unwrap()["content"]
        .as_array()
        .unwrap();
    assert_eq!(
        turn[0],
        json!({"type": "thinking", "thinking": "", "signature": "EqQBCkYIBxgCKkBfixturesignatureAAAA+/=="})
    );
    assert_eq!(
        turn[1],
        json!({"type": "redacted_thinking", "data": "EmwKAhgBEgy3fixtureredacteddataBBBB+/=="})
    );
    assert_eq!(
        turn[2],
        json!({"type": "text", "text": "Reading both files."})
    );
    assert_eq!(turn[3]["id"], "toolu_01FixtureA");
    assert_eq!(turn[4]["id"], "toolu_01FixtureB");
    // Parallel results go back in one user message.
    let results = messages.last().unwrap()["content"].as_array().unwrap();
    assert_eq!(messages.last().unwrap()["role"], "user");
    assert_eq!(results[0]["tool_use_id"], "toolu_01FixtureA");
    assert_eq!(results[1]["tool_use_id"], "toolu_01FixtureB");
    assert_eq!(second["tools"], body["tools"], "tools stay byte-stable");
    assert_eq!(second["system"], body["system"], "system stays byte-stable");
    // The answer's own signed block is kept for the hop after it.
    let answer_reasoning = crate::agent::club::take_pending_tool_reasoning().unwrap();
    assert!(answer_reasoning.contains("Ehop2=="), "{answer_reasoning}");
    // Usage reached angelX's accounting with cached and uncached input apart.
    let usage = crate::agent::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    let usage = serde_json::to_value(usage).unwrap();
    assert_eq!(usage["cache_read"], 20480 + 22000, "{usage}");
    assert_eq!(usage["cache_write"], 1880 + 300, "{usage}");
    assert_eq!(usage["uncached_input"], 412 + 9, "{usage}");
    assert_eq!(usage["output"], 187 + 12, "{usage}");
    drop(_env);
    super::super::resync_reasoning_effort_env_from_env();
    super::super::resync_max_tokens_env_from_env();
    server.join().unwrap();
}
