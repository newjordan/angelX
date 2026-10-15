use super::*;

#[test]
fn stream_reminder_never_places_system_after_conversation_history() {
    let mut with_system = serde_json::json!({
        "messages": [
            { "role": "system", "content": "base policy" },
            { "role": "user", "content": "work" },
            { "role": "assistant", "content": "partial" }
        ]
    });
    inject_stream_reminder(&mut with_system, "act now");
    let messages = with_system["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3, "reuse the leading system message");
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
    assert!(
        messages[0]["content"]
            .as_str()
            .unwrap()
            .ends_with("[stream-rule reminder] act now")
    );

    let mut without_system = serde_json::json!({
        "messages": [
            { "role": "user", "content": "work" },
            { "role": "assistant", "content": "partial" }
        ]
    });
    inject_stream_reminder(&mut without_system, "answer now");
    let messages = without_system["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[2]["role"], "assistant");
}

#[test]
fn live_meters_read_usage_without_taking_the_hop_mutex() {
    let src = include_str!("../../../cockpit/src/agent/club/http.rs");
    let token = src
        .find("fn token_usage(&self) -> Option<TokenUsage>")
        .expect("HttpClub token_usage");
    let cache = src
        .find("fn cache_usage(&self) -> CacheUsage")
        .expect("HttpClub cache_usage");
    let token_body = &src[token..cache];
    assert!(
        token_body.contains("self.usage.load()") && !token_body.contains(".lock("),
        "token_usage must stay lock-free for the draw path:\n{token_body}"
    );
    let cache_end = src[cache..]
        .find("\n    fn prompt_cache_capable")
        .map(|n| cache + n)
        .expect("prompt_cache_capable follows cache_usage");
    let cache_body = &src[cache..cache_end];
    assert!(
        cache_body.contains("self.cache_usage.load()") && !cache_body.contains(".lock("),
        "cache_usage must stay lock-free for the drain fold:\n{cache_body}"
    );
}

/// Restore-on-drop env guard (the `club/tests` one is module-private).
struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}
impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }
    fn unset(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(self.key) },
        }
        if self.key.ends_with("REASONING_EFFORT") || self.key.ends_with("REASONING_DIALECT") {
            resync_reasoning_effort_env_from_env();
        }
        if self.key.ends_with("MAX_TOKENS") {
            resync_max_tokens_env_from_env();
        }
        if self.key.ends_with("PROMPT_CACHE") || self.key.ends_with("PROMPT_CACHE_KEY") {
            resync_prompt_cache_from_env();
        }
        if self.key.contains("ANTHROPIC_CACHE") {
            resync_openrouter_anthropic_cache_from_env();
        }
    }
}

#[test]
fn structured_context_overflow_teaches_the_http_metadata_cache() {
    let detail = r#"{"error":{"code":400,"message":"request (104239 tokens) exceeds the available context size (102400 tokens)","type":"exceed_context_size_error","n_prompt_tokens":104239,"n_ctx":102400}}"#;
    assert_eq!(context_window_from_error_detail(detail), Some(102_400));

    let club = HttpClub::new("casper", "http://127.0.0.1:9/v1", "local", None);
    club.inject_metadata_for_tests(Metadata {
        context_window: 0,
        supports_cache: true,
        supports_reasoning: Some(true),
        supports_tools: true,
    });
    club.learn_context_window_from_error(detail);
    let learned = club
        .metadata
        .lock()
        .expect("metadata lock")
        .as_ref()
        .expect("learned metadata")
        .0;
    assert_eq!(learned.context_window, 102_400);
    assert!(learned.supports_cache);
    assert_eq!(learned.supports_reasoning, Some(true));
    assert!(learned.supports_tools);

    club.learn_context_window_from_error(r#"{"error":{"type":"invalid_request_error","n_ctx":7}}"#);
    let unchanged = club
        .metadata
        .lock()
        .expect("metadata lock")
        .as_ref()
        .expect("cached metadata")
        .0;
    assert_eq!(unchanged, learned);
}

#[test]
fn deepseek_v4_replays_reasoning_on_exact_live_messages_without_cross_matching() {
    let club = HttpClub::new(
        "deepseek",
        "https://api.deepseek.com/v1",
        "deepseek-v4-pro",
        None,
    );
    let repaired = vec![ToolCall {
        id: "angel_call_1_0".to_string(),
        name: "read".to_string(),
        args: serde_json::json!({"path": "src/main.rs"}),
    }];
    let first = ChatMsg::assistant_calls_with_reasoning(
        repaired.clone(),
        Some("first-private-reasoning".to_string()),
    );
    let second = ChatMsg::assistant_calls_with_reasoning(
        repaired.clone(),
        Some("second-private-reasoning".to_string()),
    );
    let body = club
        .build_body(
            &[
                ChatMsg::user("inspect"),
                first.clone(),
                ChatMsg::tool(&repaired[0].id, "source"),
                ChatMsg::user("inspect again"),
                second,
                ChatMsg::tool(&repaired[0].id, "source again"),
            ],
            &[],
            true,
        )
        .expect("DeepSeek body");
    assert_eq!(
        body["messages"][1]["reasoning_content"].as_str(),
        Some("first-private-reasoning")
    );
    assert_eq!(
        body["messages"][4]["reasoning_content"].as_str(),
        Some("second-private-reasoning")
    );
    let saved = serde_json::to_string(&first).expect("serialize live message");
    assert!(!saved.contains("private-reasoning"));
    assert!(!format!("{first:?}").contains("first-private-reasoning"));
    let restored: ChatMsg = serde_json::from_str(&saved).expect("restore message");
    assert!(restored.private_reasoning.is_none());

    // The extra field is provider-specific; public compatible gateways may
    // reject it, and a URL containing the official host as userinfo is not
    // trusted.
    let generic = HttpClub::new(
        "deepseek-proxy",
        "https://api.deepseek.com@gateway.example/v1",
        "deepseek-v4-pro",
        None,
    );
    let generic_body = generic
        .build_body(
            &[
                ChatMsg::user("inspect"),
                first,
                ChatMsg::tool(&repaired[0].id, "source"),
            ],
            &[],
            true,
        )
        .expect("generic body");
    assert!(
        generic_body["messages"][1]
            .get("reasoning_content")
            .is_none()
    );
}

/// DeepSeek's thinking mode: while `tools` ride the request, every earlier
/// assistant turn passes its reasoning back, final answers included; a turn
/// whose reasoning this process never held goes back empty, never invented.
#[test]
fn deepseek_passes_back_every_reasoned_turn_while_tools_ride_the_request() {
    let club = HttpClub::new(
        "deepseek-flash",
        "https://api.deepseek.com/v1",
        "deepseek-flash",
        None,
    );
    let call = vec![ToolCall {
        id: "call_1".to_string(),
        name: "read_file".to_string(),
        args: serde_json::json!({"path": "a.rs"}),
    }];
    let tool = crate::agent::club::ToolDef {
        name: "read_file".into(),
        description: "read".into(),
        params: serde_json::json!({"type": "object"}),
    };
    let history = vec![
        ChatMsg::user("fix it"),
        ChatMsg::assistant_calls_with_reasoning(call.clone(), Some("read first".to_string())),
        ChatMsg::tool("call_1", "source"),
        ChatMsg::assistant_with_reasoning("fixed", Some("the answer's thought".to_string())),
        ChatMsg::user("and the test?"),
        // A resumed session holds no reasoning for its restored turns.
        ChatMsg::assistant_calls(call.clone()),
        ChatMsg::tool("call_1", "source again"),
    ];
    let body = club
        .build_body(&history, std::slice::from_ref(&tool), true)
        .unwrap();
    let reasoning = |i: usize| body["messages"][i].get("reasoning_content").cloned();
    assert_eq!(reasoning(1), Some(serde_json::json!("read first")));
    assert_eq!(
        reasoning(3),
        Some(serde_json::json!("the answer's thought"))
    );
    assert_eq!(reasoning(5), Some(serde_json::json!("")));
    // Without tools the provider ignores reasoning: held reasoning still rides
    // (the prefix renders it), but nothing is filled in.
    let bare = club.build_body(&history, &[], true).unwrap();
    assert_eq!(
        bare["messages"][3]["reasoning_content"],
        "the answer's thought"
    );
    assert!(bare["messages"][5].get("reasoning_content").is_none());
    // A gateway that only looks like DeepSeek gets neither.
    let generic = HttpClub::new(
        "deepseek-proxy",
        "https://api.deepseek.com@gateway.example/v1",
        "deepseek-flash",
        None,
    );
    let body = generic.build_body(&history, &[tool], true).unwrap();
    assert!(body["messages"][3].get("reasoning_content").is_none());
    assert!(body["messages"][5].get("reasoning_content").is_none());
}

#[test]
fn private_reasoning_handoff_is_thread_local_and_consumed_once() {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers = ["session-a", "session-b"].map(|value| {
        let barrier = std::sync::Arc::clone(&barrier);
        std::thread::spawn(move || {
            set_pending_tool_reasoning(Some(value.to_string()));
            barrier.wait();
            assert_eq!(take_pending_tool_reasoning().as_deref(), Some(value));
            assert!(take_pending_tool_reasoning().is_none());
        })
    });
    for worker in workers {
        worker.join().expect("reasoning handoff worker");
    }
    assert!(take_pending_tool_reasoning().is_none());
}

#[test]
fn env_knob_names_are_precomputed_from_the_club_namespace() {
    let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "m", None);
    assert_eq!(club.env_prefix(), "DEEPSEEK_V4_PRO");
    assert_eq!(
        club.effort_env_name,
        "ANGEL_DEEPSEEK_V4_PRO_REASONING_EFFORT"
    );
    assert_eq!(
        club.dialect_env_name,
        "ANGEL_DEEPSEEK_V4_PRO_REASONING_DIALECT"
    );
    assert_eq!(club.max_tokens_env_name, "ANGEL_DEEPSEEK_V4_PRO_MAX_TOKENS");
    // Model-id labels on an aggregator resolve to the provider namespace.
    let openrouter = HttpClub::new(
        "tencent/hy3:free",
        "https://openrouter.ai/api/v1",
        "tencent/hy3:free",
        None,
    );
    assert_eq!(
        openrouter.effort_env_name,
        "ANGEL_OPENROUTER_REASONING_EFFORT"
    );
}

/// Seed-once cache: EnvGuard overrides of effort/dialect knobs stay
/// invisible until resync, then restore after the guard drops.
#[test]
fn reasoning_effort_env_seeds_once_and_resyncs_under_env_lock() {
    use crate::agent::club::Club;
    let _guard = crate::tests::env_lock();
    let per_club = "ANGEL_T_EFFORT_CACHE_REASONING_EFFORT";
    let dialect = "ANGEL_T_EFFORT_CACHE_REASONING_DIALECT";
    {
        let _global_clear = EnvGuard::unset("ANGEL_REASONING_EFFORT");
        let _club_clear = EnvGuard::unset(per_club);
        let _dial_clear = EnvGuard::unset(dialect);
        let _grok_clear = EnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
        resync_reasoning_effort_env_from_env();
        assert_eq!(reasoning_env_var("ANGEL_REASONING_EFFORT"), None);
        assert_eq!(reasoning_env_var(per_club), None);
        assert_eq!(reasoning_env_var(dialect), None);
        assert_eq!(reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"), None);

        let _global = EnvGuard::set("ANGEL_REASONING_EFFORT", "medium");
        let _club = EnvGuard::set(per_club, "low");
        let _dial = EnvGuard::set(dialect, "glm");
        let _grok = EnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "high");
        assert_eq!(
            reasoning_env_var("ANGEL_REASONING_EFFORT"),
            None,
            "cache must not re-read env until seed reset"
        );
        assert_eq!(reasoning_env_var(per_club), None);
        assert_eq!(reasoning_env_var(dialect), None);
        assert_eq!(reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"), None);

        resync_reasoning_effort_env_from_env();
        assert_eq!(
            reasoning_env_var("ANGEL_REASONING_EFFORT").as_deref(),
            Some("medium")
        );
        assert_eq!(reasoning_env_var(per_club).as_deref(), Some("low"));
        assert_eq!(reasoning_env_var(dialect).as_deref(), Some("glm"));
        assert_eq!(
            reasoning_env_var("ANGEL_GROK_REASONING_EFFORT").as_deref(),
            Some("high")
        );

        let _snap = EnvGuard::set("ANGEL_EFFORT_SNAPSHOT", "0");
        let club = HttpClub::new("t-effort-cache", "http://127.0.0.1:9/v1", "m", None);
        assert_eq!(club.effort_env_read().as_deref(), Some("low"));
        assert_eq!(club.reasoning_effort().as_deref(), Some("low"));
        assert_eq!(club.reasoning_dialect(), ReasoningDialect::GlmThinking);
    }
    resync_reasoning_effort_env_from_env();
    assert_eq!(
        reasoning_env_var("ANGEL_REASONING_EFFORT"),
        std::env::var("ANGEL_REASONING_EFFORT").ok()
    );
    assert_eq!(reasoning_env_var(per_club), std::env::var(per_club).ok());
    assert_eq!(reasoning_env_var(dialect), std::env::var(dialect).ok());
    assert_eq!(
        reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"),
        std::env::var("ANGEL_GROK_REASONING_EFFORT").ok()
    );
}

#[test]
fn effort_snapshot_invalidates_on_revision_and_ttl() {
    use crate::agent::club::Club;
    let _guard = crate::tests::env_lock();
    let _snap = EnvGuard::set("ANGEL_EFFORT_SNAPSHOT", "1");
    let _global = EnvGuard::unset("ANGEL_REASONING_EFFORT");
    let _per = EnvGuard::unset("ANGEL_T_EFFORT_SNAP_REASONING_EFFORT");
    let _dial = EnvGuard::unset("ANGEL_T_EFFORT_SNAP_REASONING_DIALECT");
    resync_reasoning_effort_env_from_env();
    let mut club = HttpClub::new("t-effort-snap", "http://127.0.0.1:9/v1", "m", None);
    // Same deterministic clock: TTL transitions are test-controlled.
    let clock = Arc::new(std::sync::Mutex::new(Instant::now()));
    let clock_reader = Arc::clone(&clock);
    club.now = Arc::new(move || *clock_reader.lock().unwrap());

    // Warm the snapshot at the default OpenAI dialect / no effort.
    assert!(club.reasoning_effort().is_none());
    assert_eq!(club.reasoning_dialect(), ReasoningDialect::OpenAiEffort);

    // THINK override bumps route_state_revision → snapshot invalidates now.
    assert_eq!(club.set_reasoning_effort("high").as_deref(), Some("high"));
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));

    // Dialect env change does not bump revision. Without resync the
    // process cache stays frozen even after TTL; after resync the expired
    // snapshot re-resolves from the new seed.
    let _dial_set = EnvGuard::set("ANGEL_T_EFFORT_SNAP_REASONING_DIALECT", "glm");
    let fresh = *clock.lock().unwrap();
    *club.effort_snapshot.lock().unwrap() = Some((
        club.route_state_revision(),
        reasoning_env_generation(),
        fresh,
        EffortSnapshot {
            dialect: ReasoningDialect::OpenAiEffort,
            gate_reason: None,
            override_effort: Some("high".into()),
            env_effort: None,
        },
    ));
    assert_eq!(
        club.reasoning_dialect(),
        ReasoningDialect::OpenAiEffort,
        "fresh snapshot must hold the prior dialect within TTL"
    );
    *club.effort_snapshot.lock().unwrap() = Some((
        club.route_state_revision(),
        reasoning_env_generation(),
        fresh - (EFFORT_ENV_TTL + Duration::from_millis(1)),
        EffortSnapshot {
            dialect: ReasoningDialect::OpenAiEffort,
            gate_reason: None,
            override_effort: Some("high".into()),
            env_effort: None,
        },
    ));
    assert_eq!(
        club.reasoning_dialect(),
        ReasoningDialect::OpenAiEffort,
        "expired snapshot must not getenv until the env cache is resynced"
    );

    resync_reasoning_effort_env_from_env();
    *club.effort_snapshot.lock().unwrap() = Some((
        club.route_state_revision(),
        reasoning_env_generation(),
        fresh - (EFFORT_ENV_TTL + Duration::from_millis(1)),
        EffortSnapshot {
            dialect: ReasoningDialect::OpenAiEffort,
            gate_reason: None,
            override_effort: Some("high".into()),
            env_effort: None,
        },
    ));
    assert_eq!(
        club.reasoning_dialect(),
        ReasoningDialect::GlmThinking,
        "expired snapshot must re-resolve dialect from the resynced cache"
    );
    // GLM ladder is binary; the high override still surfaces.
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
}

fn assert_provider_cancel_disconnects(partial: &'static str, chunked: bool, headers: bool) {
    use std::io::{Read, Write};
    let _guard = crate::tests::env_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (ready, received) = std::sync::mpsc::channel();
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let server = scope.spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let _request = read_http_request(&mut socket);
            if headers {
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n{}\r\n{partial}",
                    if chunked {
                        "Transfer-Encoding: chunked\r\n"
                    } else {
                        ""
                    }
                )
                .unwrap();
            }
            ready.send(()).unwrap();
            let mut byte = [0];
            assert_eq!(
                socket.read(&mut byte).unwrap(),
                0,
                "cancel must close the provider connection"
            );
        });
        let cancel_ref = &cancel;
        let canceller = scope.spawn(move || {
            received.recv_timeout(Duration::from_secs(2)).unwrap();
            std::thread::sleep(Duration::from_millis(100));
            let start = Instant::now();
            cancel_ref.store(true, Ordering::Relaxed);
            start
        });
        let club = HttpClub::new("cancel-fixture", url, "fixture", None);
        let body = serde_json::json!({"model":"fixture", "messages":[], "stream":true});
        let reply = club.stream_body_with_rules(
            body,
            &[],
            &cancel,
            &mut |_| {},
            &crate::agent::stream_rules::StreamRules::from_json_for_test("[]"),
        );
        let signal = canceller.join().unwrap();
        if headers {
            assert!(reply.is_ok(), "{reply:?}");
        } else {
            assert!(reply.is_err(), "cancelled request must not yield a reply");
        }
        assert!(signal.elapsed() < Duration::from_secs(2));
        server.join().unwrap();
    });
}

#[test]
fn provider_cancel_disconnects_hung_response_headers() {
    assert_provider_cancel_disconnects("", false, false);
}

#[test]
fn provider_cancel_disconnects_hung_sse_read() {
    assert_provider_cancel_disconnects(": ready\n\ndata: {", false, true);
}

#[test]
fn provider_cancel_disconnects_partial_http_chunk_header() {
    assert_provider_cancel_disconnects("a", true, true);
}

#[test]
fn provider_cancel_disconnects_partial_http_chunk_body() {
    assert_provider_cancel_disconnects("100\r\ndata: {", true, true);
}

/// Read one full HTTP request (headers + `Content-Length` body) off a
/// socket so a test can assert on what actually went over the wire.
/// (A local twin of `club/tests.rs`'s helper, which is module-private.)
fn read_http_request(sock: &mut std::net::TcpStream) -> String {
    use std::io::Read;
    let mut req = Vec::new();
    let mut buf = [0u8; 1024];
    while let Ok(n) = sock.read(&mut buf) {
        if n == 0 {
            break;
        }
        req.extend_from_slice(&buf[..n]);
        if let Some(pos) = req.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&req[..pos]).to_ascii_lowercase();
            let need = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if req.len() >= pos + 4 + need {
                break;
            }
        }
    }
    String::from_utf8_lossy(&req).into_owned()
}

/// Serve a fixed sequence of canned HTTP responses (one connection each),
/// capturing the raw request that preceded each. Once the sequence is
/// exhausted the listener drops, so an over-asking retry loop fails visibly.
fn serve_seq(
    responses: Vec<String>,
) -> (
    String,
    std::sync::Arc<Mutex<Vec<String>>>,
    std::thread::JoinHandle<()>,
) {
    serve_seq_with_accept_timeout(responses, std::time::Duration::from_secs(2))
}

/// As [`serve_seq`], with a bounded per-connection wait chosen by a test
/// whose setup can be delayed independently of the fixture thread.
fn serve_seq_with_accept_timeout(
    responses: Vec<String>,
    accept_timeout: std::time::Duration,
) -> (
    String,
    std::sync::Arc<Mutex<Vec<String>>>,
    std::thread::JoinHandle<()>,
) {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Instant;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener
        .set_nonblocking(true)
        .expect("fixture listener nonblocking");
    let addr = listener.local_addr().unwrap();
    let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
    let captured = std::sync::Arc::clone(&requests);
    let handle = std::thread::spawn(move || {
        for response in responses {
            let deadline = Instant::now() + accept_timeout;
            let mut sock = loop {
                match listener.accept() {
                    Ok((sock, _)) => break sock,
                    Err(err)
                        if err.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    _ => return,
                }
            };
            let _ = sock.set_nonblocking(false);
            let _ = sock.set_read_timeout(Some(std::time::Duration::from_secs(1)));
            let _ = sock.set_write_timeout(Some(std::time::Duration::from_secs(1)));
            let req = read_http_request(&mut sock);
            captured.lock().unwrap().push(req);
            let _ = sock.write_all(response.as_bytes());
            let _ = sock.flush();
            // Wait for the client to finish reading and hang up (read → 0).
            let mut buf = [0u8; 512];
            let _ = sock.read(&mut buf);
        }
    });
    (format!("http://{addr}"), requests, handle)
}

/// Send one SSE event, pause long enough to trip the client's socket read
/// timeout, then finish the same response. Models the Qwen/SGLang parser
/// gap where decoding continues but a large tool argument is withheld.
fn serve_delayed_stream(
    first_event: &'static str,
    delay: Duration,
    remaining_events: &'static str,
) -> (String, std::thread::JoinHandle<()>) {
    use std::io::Write;
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let _ = read_http_request(&mut sock);
        sock.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
        sock.write_all(first_event.as_bytes()).unwrap();
        sock.flush().unwrap();
        std::thread::sleep(delay);
        let _ = sock.write_all(remaining_events.as_bytes());
        let _ = sock.flush();
    });
    (format!("http://{addr}"), handle)
}

/// Keep sending SSE comments during a quiet gap: the keep-alive path that
/// refreshes the foreground watchdog.
fn serve_keepalive_stream(
    first_event: &'static str,
    ping_delay: Duration,
    ping_count: usize,
    remaining_events: &'static str,
) -> (String, std::thread::JoinHandle<()>) {
    use std::io::Write;
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let _ = read_http_request(&mut sock);
        sock.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
        sock.write_all(first_event.as_bytes()).unwrap();
        sock.flush().unwrap();
        for _ in 0..ping_count {
            std::thread::sleep(ping_delay);
            let _ = sock.write_all(b": ping\n\n");
            let _ = sock.flush();
        }
        let _ = sock.write_all(remaining_events.as_bytes());
        let _ = sock.flush();
    });
    (format!("http://{addr}"), handle)
}

/// A streamed call has no read deadline: a tool parser that holds its first
/// giant `data:` line back past `ANGEL_HTTP_TIMEOUT` (the buffered-call bound)
/// still lands its call.
#[test]
fn a_stream_survives_silence_past_the_buffered_read_bound() {
    let _guard = crate::tests::env_lock();
    {
        let _http_timeout = EnvGuard::set("ANGEL_HTTP_TIMEOUT", "1");
        resync_stream_knobs_from_env();

        // A tool-only response can cross the socket deadline inside its
        // very first giant `data:` line, before the accumulator has seen a
        // parseable event.
        let first = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,";
        let rest = concat!(
            "\"id\":\"call_1\",\"function\":{\"name\":\"write_file\",",
            "\"arguments\":\"{\\\"path\\\":\\\"note.md\\\",",
            "\\\"content\\\":\\\"hello\\\"}\"}}]},",
            "\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let (base, handle) = serve_delayed_stream(first, Duration::from_millis(1_500), rest);
        let club = HttpClub::new("qwen-parser-gap", base, "qwen", None);
        let body = serde_json::json!({
            "model": "qwen",
            "messages": [{"role": "user", "content": "write the note"}],
            "stream": true,
        });
        let tools = [ToolDef {
            name: "write_file".to_string(),
            description: "write a file".to_string(),
            params: serde_json::json!({"type": "object"}),
        }];
        let rules = crate::agent::stream_rules::StreamRules::from_json_for_test("[]");
        let cancel = AtomicBool::new(false);
        let mut heartbeats = 0usize;
        let reply = club
            .stream_body_with_rules(
                body,
                &tools,
                &cancel,
                &mut |delta| match delta {
                    StreamDelta::Content(_) => {}
                    StreamDelta::Reasoning(_) => {}
                    StreamDelta::Heartbeat => heartbeats += 1,
                },
                &rules,
            )
            .expect("a quiet stream is never severed");
        handle.join().unwrap();

        assert_eq!(heartbeats, 0, "silence carries no keep-alive");
        match reply {
            ClubReply::Calls(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].id, "call_1");
                assert_eq!(calls[0].name, "write_file");
                assert_eq!(calls[0].args["path"], "note.md");
                assert_eq!(calls[0].args["content"], "hello");
            }
            other => panic!("expected tool call, got {other:?}"),
        }
    }
    resync_stream_knobs_from_env();
}

#[test]
fn keepalives_refresh_the_foreground_watchdog_without_flooding_it() {
    let _guard = crate::tests::env_lock();
    {
        resync_stream_knobs_from_env();

        let first = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":",
            "\"assembling tool arguments\"}}]}\n\n",
        );
        let rest = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,",
            "\"id\":\"call_ping\",\"function\":{\"name\":\"write_file\",",
            "\"arguments\":\"{\\\"path\\\":\\\"ping.md\\\"}\"}}]},",
            "\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let (base, handle) = serve_keepalive_stream(first, Duration::from_millis(300), 5, rest);
        let club = HttpClub::new("qwen-keepalive-gap", base, "qwen", None);
        let body = serde_json::json!({
            "model": "qwen",
            "messages": [{"role": "user", "content": "write the note"}],
            "stream": true,
        });
        let tools = [ToolDef {
            name: "write_file".to_string(),
            description: "write a file".to_string(),
            params: serde_json::json!({"type": "object"}),
        }];
        let rules = crate::agent::stream_rules::StreamRules::from_json_for_test("[]");
        let cancel = AtomicBool::new(false);
        let mut heartbeats = 0usize;
        let reply = club
            .stream_body_with_rules(
                body,
                &tools,
                &cancel,
                &mut |delta| {
                    if matches!(delta, StreamDelta::Heartbeat) {
                        heartbeats += 1;
                    }
                },
                &rules,
            )
            .expect("a keep-alive stream is a live model");
        handle.join().unwrap();

        assert_eq!(
            heartbeats, 1,
            "keep-alives must refresh the watchdog without flooding it"
        );
        match reply {
            ClubReply::Calls(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].id, "call_ping");
                assert_eq!(calls[0].args["path"], "ping.md");
            }
            other => panic!("expected tool call, got {other:?}"),
        }
    }
    resync_stream_knobs_from_env();
}

/// A model thinking quietly past the buffered-call read bound is never cut
/// on a stream, with or without tools offered.
#[test]
fn a_tool_free_stream_is_never_cut_by_the_read_bound() {
    let _guard = crate::tests::env_lock();
    {
        let _http_timeout = EnvGuard::set("ANGEL_HTTP_TIMEOUT", "1");
        resync_stream_knobs_from_env();

        let first = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":",
            "\"still working\"}}]}\n\n",
        );
        let (base, handle) = serve_delayed_stream(
            first,
            Duration::from_millis(1_500),
            concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"answer\"},",
                "\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n",
            ),
        );
        let club = HttpClub::new("llama-local", base, "llama-4", None);
        let body = serde_json::json!({
            "model": "llama-4",
            "messages": [{"role": "user", "content": "answer"}],
            "stream": true,
        });
        let rules = crate::agent::stream_rules::StreamRules::from_json_for_test("[]");
        let cancel = AtomicBool::new(false);
        let mut heartbeats = 0usize;
        let reply = club
            .stream_body_with_rules(
                body,
                &[],
                &cancel,
                &mut |delta| {
                    if matches!(delta, StreamDelta::Heartbeat) {
                        heartbeats += 1;
                    }
                },
                &rules,
            )
            .expect("a quiet thinking model is never cut");
        handle.join().unwrap();

        assert!(
            matches!(reply, ClubReply::Text(ref t) if t == "answer"),
            "{reply:?}"
        );
        assert_eq!(heartbeats, 0);
    }
    resync_stream_knobs_from_env();
}

#[test]
fn credentialed_provider_request_never_contacts_redirect_target() {
    use std::io::Write;
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    let redirect_target = TcpListener::bind("127.0.0.1:0").unwrap();
    redirect_target.set_nonblocking(true).unwrap();
    let target_url = format!("http://{}/stolen", redirect_target.local_addr().unwrap());
    let source = TcpListener::bind("127.0.0.1:0").unwrap();
    let source_url = format!(
        "http://{}/v1/chat/completions",
        source.local_addr().unwrap()
    );
    let server = std::thread::spawn(move || {
        let (mut socket, _) = source.accept().unwrap();
        let request = read_http_request(&mut socket);
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: {target_url}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        socket.write_all(response.as_bytes()).unwrap();
        request
    });

    let club = HttpClub::new("redirect-test", source_url, "model", Some("secret".into()));
    let result = club
        .agent
        .post(&club.base_url)
        .set("Authorization", "Bearer secret")
        .send_string("{}");
    assert_eq!(result.expect("302 response").status(), 302);
    let request = server.join().unwrap();
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret")
    );

    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        match redirect_target.accept() {
            Ok(_) => panic!("credentialed client contacted the redirect target"),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("redirect target accept failed: {error}"),
        }
    }
}

#[test]
fn p05b_glm_reasoning_is_delivered_before_answer_and_answer_bytes_are_preserved() {
    use std::io::Write;
    let _guard = crate::tests::env_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (visible_tx, visible_rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let request = read_http_request(&mut socket);
        socket.write_all(concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"fixture progress\"}}]}\n\n",
            ).as_bytes()).unwrap();
        socket.flush().unwrap();
        // A test-only handshake proves delivery while the answer is still
        // withheld. This is not a product deadline or a latency benchmark.
        visible_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        socket
            .write_all(
                concat!(
                    "data: {\"choices\":[{\"delta\":{\"content\":\"  answer\\n\"}}]}\n\n",
                    "data: {\"choices\":[{\"delta\":{\"content\":\"雪  \"}}]}\n\n",
                    "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                    "data: [DONE]\n\n",
                )
                .as_bytes(),
            )
            .unwrap();
        request
    });
    let club = HttpClub::new("openrouter", base, "glm-5.3-flash", None);
    let body = club
        .build_body_with_effort(&[ChatMsg::user("fixture")], &[], true, Some("low"))
        .unwrap();
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["reasoning_effort"], "low");
    let mut answer = String::new();
    let mut reasoning = String::new();
    let reply = club
        .stream_body_with_rules(
            body,
            &[],
            &AtomicBool::new(false),
            &mut |delta| match delta {
                StreamDelta::Reasoning(text) => {
                    assert!(answer.is_empty());
                    reasoning.push_str(text);
                    visible_tx.send(()).unwrap();
                }
                StreamDelta::Content(text) => answer.push_str(text),
                StreamDelta::Heartbeat => panic!("unexpected heartbeat"),
            },
            &crate::agent::stream_rules::StreamRules::from_json_for_test("[]"),
        )
        .unwrap();
    let request = server.join().unwrap();
    let wire: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(wire["stream"], true);
    assert_eq!(wire["thinking"]["type"], "enabled");
    assert_eq!(reasoning, "fixture progress");
    assert_eq!(answer.as_bytes(), "  answer\n雪  ".as_bytes());
    assert!(matches!(reply, ClubReply::Text(text) if text.as_bytes() == answer.as_bytes()));
}

#[test]
fn p05b_plain_answer_and_reasoning_tool_calls_keep_their_payloads() {
    let _guard = crate::tests::env_lock();
    let plain = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"  plain\\n雪  \"}}]}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let tool = concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"fixture progress\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_fixture\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"fixture.txt\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
    let (base, _, server) = serve_seq(vec![plain, tool]);
    let club = HttpClub::new("fixture", base, "fixture", None);
    for is_tool in [false, true] {
        let mut content = String::new();
        let mut reasoning = String::new();
        let reply = club
            .stream_body_with_rules(
                serde_json::json!({"model":"fixture", "stream":true, "messages":[]}),
                &[],
                &AtomicBool::new(false),
                &mut |delta| match delta {
                    StreamDelta::Content(text) => content.push_str(text),
                    StreamDelta::Reasoning(text) => reasoning.push_str(text),
                    StreamDelta::Heartbeat => panic!("unexpected heartbeat"),
                },
                &crate::agent::stream_rules::StreamRules::from_json_for_test("[]"),
            )
            .unwrap();
        if is_tool {
            assert!(content.is_empty());
            assert_eq!(reasoning, "fixture progress");
            let ClubReply::Calls(calls) = reply else {
                panic!("expected tool calls")
            };
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].id, "call_fixture");
            assert_eq!(calls[0].name, "read_file");
            assert_eq!(calls[0].args, serde_json::json!({"path":"fixture.txt"}));
        } else {
            assert!(reasoning.is_empty());
            assert_eq!(content.as_bytes(), "  plain\n雪  ".as_bytes());
            assert!(matches!(reply, ClubReply::Text(text) if text == content));
        }
    }
    server.join().unwrap();
}

#[test]
fn p05c_cancel_during_zai_reasoning_tears_the_stream_down() {
    use std::io::Write;
    use std::sync::atomic::Ordering;
    let _guard = crate::tests::env_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (visible_tx, visible_rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let _request = read_http_request(&mut socket);
        socket.write_all(concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think first\"}}]}\n\n",
            ).as_bytes()).unwrap();
        socket.flush().unwrap();
        let _ = visible_rx.recv_timeout(Duration::from_secs(5));
        let _ = socket.write_all(
            concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"must not land\"}}]}\n\n",
                "data: [DONE]\n\n",
            )
            .as_bytes(),
        );
    });
    let club = HttpClub::new("openrouter", base, "glm-5.3-flash", None);
    let cancel = AtomicBool::new(false);
    let mut reasoning = String::new();
    let mut answer = String::new();
    let reply = club.stream_body_with_rules(
        serde_json::json!({"model":"glm-5.3-flash","stream":true,"messages":[]}),
        &[],
        &cancel,
        &mut |delta| match delta {
            StreamDelta::Reasoning(text) => {
                reasoning.push_str(text);
                cancel.store(true, Ordering::Relaxed);
                let _ = visible_tx.send(());
            }
            StreamDelta::Content(text) => answer.push_str(text),
            StreamDelta::Heartbeat => {}
        },
        &crate::agent::stream_rules::StreamRules::from_json_for_test("[]"),
    );
    let _ = server.join();
    assert_eq!(reasoning, "think first");
    assert!(!answer.contains("must not land"), "{answer}");
    match reply {
        Ok(ClubReply::Text(text)) => {
            assert!(!text.contains("must not land"), "{text}");
        }
        Err(err) => {
            let lower = err.to_ascii_lowercase();
            assert!(
                lower.contains("cancel") || lower.contains("interrupt") || lower.contains("abort"),
                "{err}"
            );
        }
        other => panic!("unexpected reply {other:?}"),
    }
}

/// Build a one-rule TTSR set without mutating process-global environment.
/// A temporary `ANGEL_STREAM_RULES` override can race any streaming test
/// that initializes the global `OnceLock`, permanently contaminating that
/// test process even when the mutating test itself holds `env_lock`.
fn probe_stream_rules() -> crate::agent::stream_rules::StreamRules {
    let rules = crate::agent::stream_rules::StreamRules::from_json_for_test(
        r#"[{"pattern":"TTSR-DRIFT-PROBE","reminder":"stay on script"}]"#,
    );
    assert!(!rules.is_empty(), "probe rule must parse");
    rules
}

/// The attempt loop encodes by move on the default path (no rules → no
/// deep clone of the full request tree). This pins the `can_retry` side:
/// a configured rule tripping mid-stream must still retry with the
/// ORIGINAL body — proving the clone survives — plus the injected
/// reminder.
#[test]
fn stream_rule_retry_discards_speculative_deltas_and_resends_original_body() {
    let _guard = crate::tests::env_lock();
    let rules = probe_stream_rules();
    let drifted = concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"discarded thought\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"heading TTSR-DRIFT-PROBE off script\"}}]}\n\n",
            "data: [DONE]\n\n",
        )
        .to_string();
    let clean = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"clean thought\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"back on script\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let (base, requests, handle) = serve_seq(vec![drifted, clean]);
    let club = HttpClub::new("ttsr-clone-probe", base, "m", None);
    let body = serde_json::json!({
        "model": "m",
        "messages": [{"role": "user", "content": "the original prompt"}],
        "stream": true,
    });
    let cancel = AtomicBool::new(false);
    let mut deltas = Vec::new();
    let reply = club
        .stream_body_with_rules(
            body,
            &[],
            &cancel,
            &mut |delta| match delta {
                StreamDelta::Content(text) => {
                    deltas.push(("content", text.to_string()));
                }
                StreamDelta::Reasoning(text) => {
                    deltas.push(("reasoning", text.to_string()));
                }
                StreamDelta::Heartbeat => {}
            },
            &rules,
        )
        .expect("rule retry recovers a clean stream");
    // The reply comes from the retried stream, not the drifted partial.
    match reply {
        ClubReply::Text(t) => assert_eq!(t, "back on script"),
        other => panic!("expected text, got {other:?}"),
    }
    assert_eq!(
        deltas,
        vec![
            ("reasoning", "clean thought".to_string()),
            ("content", "back on script".to_string()),
        ],
        "speculative deltas from the discarded attempt must never leak"
    );
    let reqs = requests.lock().unwrap();
    assert_eq!(reqs.len(), 2, "one drifted attempt + one rule retry");
    assert!(
        reqs[1].contains("the original prompt"),
        "retry lost the original body: {}",
        reqs[1]
    );
    assert!(
        reqs[1].contains("[stream-rule reminder] stay on script"),
        "retry must carry the injected reminder: {}",
        reqs[1]
    );
    assert!(
        !reqs[0].contains("stream-rule reminder"),
        "first attempt is the untouched request"
    );
    drop(reqs);
    handle.join().unwrap();
}

/// The `can_retry = false` side of the same gate: with the retry budget at
/// zero a matching rule must not fire, the body is encoded by move, and
/// the stream completes in exactly one request carrying the same bytes.
#[test]
fn stream_rule_gate_exhausted_encodes_by_move_single_attempt() {
    let _guard = crate::tests::env_lock();
    {
        let rules = probe_stream_rules();
        let _retries = EnvGuard::set("ANGEL_STREAM_RULE_RETRIES", "0");
        resync_stream_knobs_from_env();
        let drifted = concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"heading TTSR-DRIFT-PROBE off script\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n",
            )
            .to_string();
        let (base, requests, handle) = serve_seq(vec![drifted]);
        let club = HttpClub::new("ttsr-move-probe", base, "m", None);
        let body = serde_json::json!({
            "model": "m",
            "messages": [{"role": "user", "content": "the original prompt"}],
            "stream": true,
        });
        let cancel = AtomicBool::new(false);
        let reply = club
            .stream_body_with_rules(body, &[], &cancel, &mut |_| {}, &rules)
            .expect("no retry budget keeps the single stream");
        handle.join().unwrap();
        match reply {
            ClubReply::Text(t) => assert_eq!(t, "heading TTSR-DRIFT-PROBE off script"),
            other => panic!("expected text, got {other:?}"),
        }
        let reqs = requests.lock().unwrap();
        assert_eq!(reqs.len(), 1, "an exhausted budget must not re-send");
        assert!(
            reqs[0].contains("the original prompt"),
            "the moved body still encodes the request: {}",
            reqs[0]
        );
    }
    resync_stream_knobs_from_env();
}

/// Seed-once cache: EnvGuard overrides are invisible until resync, then
/// visible, then restored after the guard drops. No knob is a clock.
#[test]
fn stream_hop_knobs_seed_once_and_resync_under_env_lock() {
    let _guard = crate::tests::env_lock();
    {
        let _line_clear = EnvGuard::unset("ANGEL_STREAM_MAX_LINE_BYTES");
        let _retries_clear = EnvGuard::unset("ANGEL_STREAM_RULE_RETRIES");
        resync_stream_knobs_from_env();
        let (max_line, retries) = stream_hop_knobs();
        assert_eq!(max_line, DEFAULT_STREAM_MAX_LINE_BYTES);
        assert_eq!(retries, DEFAULT_STREAM_RULE_RETRIES);

        let _line = EnvGuard::set("ANGEL_STREAM_MAX_LINE_BYTES", "4096");
        let _retries = EnvGuard::set("ANGEL_STREAM_RULE_RETRIES", "0");
        let (max_line, retries) = stream_hop_knobs();
        assert_eq!(
            max_line, DEFAULT_STREAM_MAX_LINE_BYTES,
            "cache must not re-read env until seed reset"
        );
        assert_eq!(retries, DEFAULT_STREAM_RULE_RETRIES);

        resync_stream_knobs_from_env();
        let (max_line, retries) = stream_hop_knobs();
        assert_eq!(max_line, 4096);
        assert_eq!(retries, 0);
    }
    resync_stream_knobs_from_env();
    let (max_line, retries) = stream_hop_knobs();
    assert_eq!(
        max_line,
        env_usize("ANGEL_STREAM_MAX_LINE_BYTES", DEFAULT_STREAM_MAX_LINE_BYTES)
    );
    assert_eq!(
        retries,
        env_usize("ANGEL_STREAM_RULE_RETRIES", DEFAULT_STREAM_RULE_RETRIES)
    );
}

/// Seed-once cache: EnvGuard overrides of the global and per-club
/// `STREAM_USAGE` pins stay invisible until resync, then restore after the
/// guard drops.
#[test]
fn stream_usage_pins_seed_once_and_resync_under_env_lock() {
    let _guard = crate::tests::env_lock();
    let club_pin = "ANGEL_STREAM_USAGE_PIN_CACHE_PROBE_STREAM_USAGE";
    {
        let _global_clear = EnvGuard::unset("ANGEL_STREAM_USAGE");
        let _club_clear = EnvGuard::unset(club_pin);
        resync_stream_usage_pins_from_env();
        assert_eq!(stream_usage_global(), None);
        assert_eq!(stream_usage_club_pin(club_pin), None);

        let _global = EnvGuard::set("ANGEL_STREAM_USAGE", "1");
        let _club = EnvGuard::set(club_pin, "0");
        assert_eq!(
            stream_usage_global(),
            None,
            "cache must not re-read env until seed reset"
        );
        assert_eq!(stream_usage_club_pin(club_pin), None);

        resync_stream_usage_pins_from_env();
        assert_eq!(stream_usage_global(), Some(true));
        assert_eq!(stream_usage_club_pin(club_pin), Some(false));
    }
    resync_stream_usage_pins_from_env();
    assert_eq!(stream_usage_global(), env_truthy_pin("ANGEL_STREAM_USAGE"));
    assert_eq!(stream_usage_club_pin(club_pin), env_truthy_pin(club_pin));
}

/// Seed-once cache: EnvGuard overrides are invisible until resync, then
/// visible, then restored after the guard drops. Covers both knobs.
#[test]
fn ratelimit_knobs_seed_once_and_resync_under_env_lock() {
    let _guard = crate::tests::env_lock();
    {
        let _proactive_clear = EnvGuard::unset("ANGEL_RATELIMIT_PROACTIVE");
        let _wait_clear = EnvGuard::unset("ANGEL_RATELIMIT_MAX_WAIT");
        resync_ratelimit_knobs_from_env();
        let (proactive, max_wait) = ratelimit_send_knobs();
        assert!(proactive);
        assert_eq!(
            max_wait,
            Duration::from_secs(DEFAULT_RATELIMIT_MAX_WAIT_SECS)
        );

        let _proactive = EnvGuard::set("ANGEL_RATELIMIT_PROACTIVE", "0");
        let _wait = EnvGuard::set("ANGEL_RATELIMIT_MAX_WAIT", "1");
        let (proactive, max_wait) = ratelimit_send_knobs();
        assert!(proactive, "cache must not re-read env until seed reset");
        assert_eq!(
            max_wait,
            Duration::from_secs(DEFAULT_RATELIMIT_MAX_WAIT_SECS)
        );

        resync_ratelimit_knobs_from_env();
        let (proactive, max_wait) = ratelimit_send_knobs();
        assert!(!proactive);
        assert_eq!(max_wait, Duration::from_secs(1));
    }
    resync_ratelimit_knobs_from_env();
    let (proactive, max_wait) = ratelimit_send_knobs();
    let (want_proactive, want_wait) = ratelimit_knobs_from_env();
    assert_eq!(proactive, want_proactive);
    assert_eq!(max_wait, want_wait);
}

/// Seed-once cache: EnvGuard overrides of `ANGEL_{CLUB}_MAX_TOKENS` /
/// `ANGEL_CLUB_MAX_TOKENS` stay invisible until resync, then restore after
/// the guard drops. Per-club still beats global; parse/zero-filter is
/// unchanged.
#[test]
fn club_max_tokens_env_seeds_once_and_resyncs_under_env_lock() {
    use crate::agent::club::Club;
    let _guard = crate::tests::env_lock();
    let per_club = "ANGEL_MAX_TOKENS_CACHE_PROBE_MAX_TOKENS";
    {
        let _global_clear = EnvGuard::unset("ANGEL_CLUB_MAX_TOKENS");
        let _club_clear = EnvGuard::unset(per_club);
        resync_max_tokens_env_from_env();
        assert_eq!(max_tokens_env("ANGEL_CLUB_MAX_TOKENS"), None);
        assert_eq!(max_tokens_env(per_club), None);
        let club = HttpClub::new("max-tokens-cache-probe", "http://127.0.0.1:9/v1", "m", None);
        assert_eq!(
            club.route_metadata().output_budget,
            OutputBudgetPolicy::ProviderNative
        );

        let _global = EnvGuard::set("ANGEL_CLUB_MAX_TOKENS", "4096");
        let _club = EnvGuard::set(per_club, "512");
        assert_eq!(
            max_tokens_env("ANGEL_CLUB_MAX_TOKENS"),
            None,
            "cache must not re-read env until seed reset"
        );
        assert_eq!(max_tokens_env(per_club), None);
        assert_eq!(
            club.route_metadata().output_budget,
            OutputBudgetPolicy::ProviderNative,
            "cache must not re-read env until seed reset"
        );

        resync_max_tokens_env_from_env();
        assert_eq!(max_tokens_env("ANGEL_CLUB_MAX_TOKENS"), Some(4096));
        assert_eq!(max_tokens_env(per_club), Some(512));
        assert_eq!(
            club.route_metadata().output_budget,
            OutputBudgetPolicy::Explicit {
                tokens: 512,
                source: OutputBudgetSource::PerClubEnv,
            }
        );
        assert_eq!(
            club.route_metadata().output_budget_provenance.as_deref(),
            Some(per_club)
        );
    }
    resync_max_tokens_env_from_env();
    assert_eq!(
        max_tokens_env("ANGEL_CLUB_MAX_TOKENS"),
        parse_max_tokens_env("ANGEL_CLUB_MAX_TOKENS")
    );
    assert_eq!(max_tokens_env(per_club), parse_max_tokens_env(per_club));
}

/// Seed-once cache: EnvGuard overrides of the global and per-club
/// `PROMPT_CACHE` pins and `ANGEL_PROMPT_CACHE_KEY` stay invisible until
/// resync, then restore after the guard drops. Send policy is unchanged.
#[test]
fn prompt_cache_pins_seed_once_and_resync_under_env_lock() {
    let _guard = crate::tests::env_lock();
    let club_pin = "ANGEL_PROMPT_CACHE_PIN_CACHE_PROBE_PROMPT_CACHE";
    {
        let _global_clear = EnvGuard::unset("ANGEL_PROMPT_CACHE");
        let _club_clear = EnvGuard::unset(club_pin);
        let _key_clear = EnvGuard::unset("ANGEL_PROMPT_CACHE_KEY");
        resync_prompt_cache_from_env();
        assert_eq!(prompt_cache_global_mode(), PromptCacheMode::Capability);
        assert_eq!(prompt_cache_club_pin(club_pin), None);
        assert_eq!(prompt_cache_explicit_key(), None);

        let _global = EnvGuard::set("ANGEL_PROMPT_CACHE", "1");
        let _club = EnvGuard::set(club_pin, "0");
        let _key = EnvGuard::set("ANGEL_PROMPT_CACHE_KEY", "explicit-key");
        assert_eq!(
            prompt_cache_global_mode(),
            PromptCacheMode::Capability,
            "cache must not re-read env until seed reset"
        );
        assert_eq!(prompt_cache_club_pin(club_pin), None);
        assert_eq!(
            prompt_cache_explicit_key(),
            None,
            "cache must not re-read env until seed reset"
        );

        resync_prompt_cache_from_env();
        assert_eq!(prompt_cache_global_mode(), PromptCacheMode::ForceOn);
        assert_eq!(prompt_cache_club_pin(club_pin), Some(false));
        assert_eq!(prompt_cache_explicit_key().as_deref(), Some("explicit-key"));
    }
    resync_prompt_cache_from_env();
    assert_eq!(
        prompt_cache_global_mode(),
        prompt_cache_global_mode_from_env()
    );
    assert_eq!(prompt_cache_club_pin(club_pin), env_truthy_pin(club_pin));
    assert_eq!(
        prompt_cache_explicit_key(),
        prompt_cache_explicit_key_from_env()
    );
}

/// Seed-once cache: EnvGuard overrides of OpenRouter Claude cache knobs
/// stay invisible until resync, then restore after the guard drops.
/// Breakpoint floor clamp and TTL `1h` match stay unchanged.
#[test]
fn openrouter_anthropic_cache_cfg_seeds_once_and_resyncs_under_env_lock() {
    let _guard = crate::tests::env_lock();
    {
        let _enabled_clear = EnvGuard::unset("ANGEL_OPENROUTER_ANTHROPIC_CACHE");
        let _floor_clear = EnvGuard::unset("ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR");
        let _ttl_clear = EnvGuard::unset("ANGEL_ANTHROPIC_CACHE_TTL");
        resync_openrouter_anthropic_cache_from_env();
        assert_eq!(openrouter_anthropic_cache_cfg(), (true, 1024, false));

        let _enabled = EnvGuard::set("ANGEL_OPENROUTER_ANTHROPIC_CACHE", "0");
        let _floor = EnvGuard::set("ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR", "100");
        let _ttl = EnvGuard::set("ANGEL_ANTHROPIC_CACHE_TTL", "1h");
        assert_eq!(
            openrouter_anthropic_cache_cfg(),
            (true, 1024, false),
            "cache must not re-read env until seed reset"
        );

        resync_openrouter_anthropic_cache_from_env();
        assert_eq!(
            openrouter_anthropic_cache_cfg(),
            (false, 256, true),
            "floor still clamps 256..=65536; only exact 1h requests 1h TTL"
        );
    }
    resync_openrouter_anthropic_cache_from_env();
    assert_eq!(
        openrouter_anthropic_cache_cfg(),
        openrouter_anthropic_cache_cfg_from_env()
    );
}
/// Operator-ordered F01 contract: a useful output floor may exceed remaining allocation.
#[test]
fn formation_graph_wire_cap_fits_shared_remaining_allocation() {
    let _lock = crate::tests::env_lock();
    let budget = crate::agent::harness::formation_budget::Budget::new(Some(1000), None);
    let _scope = crate::agent::harness::formation_budget::enter(Some(budget.clone()));
    let payload = serde_json::json!({"choices":[{"message":{"role":"assistant","content":"done"}}],
            "usage":{"prompt_tokens":100,"completion_tokens":10,
                "prompt_tokens_details":{"cached_tokens":20,"cache_write_tokens":0},
                "completion_tokens_details":{"reasoning_tokens":2}}})
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        payload.len(),
        payload
    );
    let (base, requests, server) =
        serve_seq_with_accept_timeout(vec![response], std::time::Duration::from_secs(30));
    let club = HttpClub::new("graph-budget-test", &base, "graph-budget-test", None);
    club.post_chat(
        serde_json::json!({"model":"graph-budget-test", "messages":[], "max_tokens":5000}),
    )
    .unwrap();
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let wire: serde_json::Value =
        serde_json::from_str(requests[0].split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(wire["max_tokens"], 1024);
    let receipt = budget.snapshot();
    assert_eq!(receipt["reserved"], 0);
    assert_eq!(receipt["calls"][0]["max_output"], wire["max_tokens"]);
    assert!(receipt["calls"][0]["reserved"].as_u64().unwrap() > 1000);
    assert_eq!(receipt["over_allocation"], true);
    assert_eq!(receipt["paid_spent"], 90);
}

#[test]
fn formation_budget_turn_abort_preserves_stream_observation_before_attempt_drop() {
    let _lock = crate::tests::env_lock();
    let _tokens = EnvGuard::set("ANGEL_FORMATION_TOKEN_BUDGET", "5000");
    let _wall = EnvGuard::unset("ANGEL_FORMATION_WALL_SECS");
    let scope = crate::agent::harness::formation_budget::start_turn().unwrap();
    let budget = crate::agent::harness::formation_budget::current().unwrap();
    let club = HttpClub::new("glm-5.3", "http://127.0.0.1:9/v1", "glm-5.3", None);
    let mut accounting = club.accounting.attempt();
    accounting.reserve_formation(budget.reserve("glm-5.3", 1000, 100).unwrap());
    let mut attempt = StreamUsageCommit::from_attempt(&club, accounting);
    attempt.observe(
        &serde_json::json!({"usage":{"prompt_tokens":500,"completion_tokens":20,
            "total_tokens":520,"prompt_tokens_details":{"cached_tokens":100,"cache_write_tokens":0},
            "completion_tokens_details":{"reasoning_tokens":5}}}),
    );
    drop(scope); // The worker still owns its attempt when the turn exits.
    let receipt = crate::agent::harness::formation_budget::snapshot().unwrap();
    assert_eq!(receipt["reserved"], 0);
    assert_eq!(receipt["spent"], 520);
    assert_eq!(receipt["calls"][0]["settled_reason"], "aborted");
    drop(attempt);
    assert_eq!(budget.snapshot(), receipt);
}

#[test]
fn formation_graph_live_shaped_terminal_usage_releases_workers_before_fanin() {
    let _lock = crate::tests::env_lock();
    // G01c/root-live-b20 GLM nofault first attempts; fan-in is the
    // observed GLM worker_fail attempt (nofault fan-in usage is unknown).
    // This tests adapter accounting under both route labels, not a claim
    // that the hosted route reported these same token counts.
    for model in ["glm-5.3-flash", "deepseek-v4-flash"] {
        for total in [48_000, 288_000] {
            let budget = crate::agent::harness::formation_budget::Budget::new(Some(total), None);
            budget.set_unstarted(6);
            let club = HttpClub::new(model, "http://127.0.0.1:9/v1", model, None);
            let finish = |role: &str, prompt, input, output, cached, reasoning| {
                let _role = crate::agent::harness::formation_budget::enter_role(role);
                let mut accounting = club.accounting.attempt();
                accounting.reserve_formation(budget.reserve(model, prompt, 1024).unwrap());
                let mut stream = StreamUsageCommit::from_attempt(&club, accounting);
                stream.observe(&serde_json::json!({"choices":[{"delta":{"content":"fixture"}}]}));
                assert!(budget.snapshot()["reserved"].as_u64().unwrap() > 0);
                stream.observe(&serde_json::json!({"choices":[], "usage":{
                        "prompt_tokens":input,"completion_tokens":output,
                        "prompt_tokens_details":{"cached_tokens":cached},
                        "completion_tokens_details":{"reasoning_tokens":reasoning}}}));
                // Observing the last chunk must not release a running stream.
                assert!(budget.snapshot()["reserved"].as_u64().unwrap() > 0);
                stream
            };
            drop(finish("planner", 5295, 1268, 528, 0, 24));
            let workers = [
                finish("worker_alpha", 7840, 1848, 81, 384, 17),
                finish("worker_beta", 7837, 1848, 42, 384, 9),
                finish("worker_gamma", 7861, 1861, 80, 384, 54),
            ];
            let before = budget.available_tokens().unwrap();
            drop(workers);
            assert!(budget.available_tokens().unwrap() > before);
            drop(finish("reviewer", 10260, 2507, 428, 384, 12));
            drop(finish("fanin", 6710, 1695, 49, 960, 5));
            let receipt = budget.snapshot();
            assert_eq!(receipt["reserved"], 0);
            assert_eq!(receipt["spent"], 12_235);
            assert_eq!(receipt["paid_spent"], 9_739);
            assert_eq!(receipt["budget_exhausted"], false);
            println!("G01f terminal-frame fixture model={model} allocation={total} {receipt}");
        }
    }
}

#[test]
fn usage_contract_scripted_frames_settle_one_shared_formation_budget() {
    let _lock = crate::tests::env_lock();
    let _tokens = EnvGuard::set("ANGEL_FORMATION_TOKEN_BUDGET", "5000");
    let _wall = EnvGuard::unset("ANGEL_FORMATION_WALL_SECS");
    let _scope = crate::agent::harness::formation_budget::start_turn().unwrap();
    let budget = crate::agent::harness::formation_budget::current().unwrap();
    for model in ["glm-5.3-flash", "glm-5.3"] {
        let club = HttpClub::new(model, "http://127.0.0.1:9/v1", model, None);
        let before = club.usage_accounting();
        let mut accounting = club.accounting.attempt();
        accounting.reserve_formation(budget.reserve(model, 1000, 100).unwrap());
        {
            let mut attempt = StreamUsageCommit::from_attempt(&club, accounting);
            let frame = serde_json::json!({"usage":{"prompt_tokens":500,"completion_tokens":20,
                    "total_tokens":520,"prompt_tokens_details":{"cached_tokens":100,"cache_write_tokens":0},
                    "completion_tokens_details":{"reasoning_tokens":5}}});
            attempt.observe(&frame);
            attempt.observe(&frame); // Repeated cumulative SSE frames charge once.
        }
        let usage =
            crate::agent::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(usage.total_prompt, Some(500));
        assert_eq!(usage.generation_output, Some(20));
        assert_eq!(usage.uncached_input, Some(400));
        assert!(usage.core_complete);
    }
    let snapshot = budget.snapshot();
    assert_eq!(snapshot["spent"], 1040);
    assert_eq!(snapshot["reserved"], 0);
    assert_eq!(snapshot["budget_exhausted"], false);
    for (call, model) in snapshot["calls"]
        .as_array()
        .unwrap()
        .iter()
        .zip(["glm-5.3-flash", "glm-5.3"])
    {
        assert_eq!(call["route"], model);
        assert_eq!(call["normalized_usage"]["source"], "reported");
    }
    let club = HttpClub::new("missing", "http://127.0.0.1:9/v1", "missing", None);
    let mut accounting = club.accounting.attempt();
    accounting.reserve_formation(budget.reserve("missing", 100, 100).unwrap());
    drop(StreamUsageCommit::from_attempt(&club, accounting));
    assert_eq!(budget.snapshot()["exhaustion_reason"], "unknown_usage");
    println!(
        "G02c scripted shared budget: two routes, 1040 tokens, no double counting; missing usage remains unknown"
    );
}

/// The legend reaches the model on the Chat Completions wire and, translated,
/// on DeepSeek's Messages wire: the stamp's English at first sight.
#[test]
fn the_legend_reaches_the_chat_and_deepseek_messages_wires() {
    let _guard = crate::tests::env_lock();
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_BOOK_INTRO");
    let read_file = crate::agent::club::ToolDef {
        name: "read_file".into(),
        description: "read".into(),
        params: serde_json::json!({"type": "object"}),
    };
    let call = vec![ToolCall {
        id: "a".to_string(),
        name: "run_tests".to_string(),
        args: serde_json::json!({}),
    }];
    let history = vec![
        ChatMsg::user("Fix the failing test."),
        ChatMsg::assistant_calls(call),
        ChatMsg::tool("a", "tests: 0 passed, 1 failed\n⠧⠉"),
    ];
    let club = HttpClub::new(
        "deepseek-flash",
        "https://api.deepseek.com/v1",
        "deepseek-flash",
        None,
    );
    let chat = club.build_body(&history, &[read_file], true).unwrap();
    assert!(
        chat["messages"]
            .to_string()
            .contains("fix the first diagnostic before the next edit"),
        "chat wire"
    );
    let messages = super::deepseek_messages::to_messages_body(&chat).unwrap();
    assert!(
        messages["messages"]
            .to_string()
            .contains("fix the first diagnostic before the next edit"),
        "messages wire"
    );
}


/// One chunked SSE answer as a keep-alive server sends it, terminator included.
fn chunked_sse_answer(answer: &str) -> String {
    let event = format!(
        "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{answer}\"}},\"finish_reason\":\"stop\"}}]}}\n\ndata: [DONE]\n\n"
    );
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{event}\r\n0\r\n\r\n",
        event.len()
    )
}

/// A streamed model call leaves its connection to the pool once its terminal
/// event arrives, so the next call skips the TCP/TLS handshake (and DNS).
#[test]
#[cfg(target_os = "linux")] // where abortable requests pool
fn streamed_calls_reuse_one_provider_connection() {
    use std::io::Write;
    let _guard = crate::tests::env_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        for answer in ["first", "second"] {
            let request = read_http_request(&mut socket);
            assert!(request.starts_with("POST"), "no request on the kept connection: {request:?}");
            assert!(
                !request.to_ascii_lowercase().contains("connection: close"),
                "{request}"
            );
            socket.write_all(chunked_sse_answer(answer).as_bytes()).unwrap();
            socket.flush().unwrap();
        }
        listener.set_nonblocking(true).unwrap();
        assert!(
            listener.accept().is_err(),
            "a streamed call opened a second connection"
        );
    });
    let club = HttpClub::new("keepalive-fixture", base, "fixture", None);
    for answer in ["first", "second"] {
        let reply = club
            .stream_body_with_rules(
                serde_json::json!({"model":"fixture", "stream":true, "messages":[]}),
                &[],
                &AtomicBool::new(false),
                &mut |_| {},
                &crate::agent::stream_rules::StreamRules::from_json_for_test("[]"),
            )
            .unwrap();
        assert!(matches!(&reply, ClubReply::Text(text) if text == answer), "{reply:?}");
        // The finished body's terminator is read on its own thread; a real
        // turn runs its tools in that gap.
        std::thread::sleep(Duration::from_millis(100));
    }
    server.join().unwrap();
}

/// Serve chunked answers on as many connections as arrive, counting them.
fn serve_chunked_counting(
    answers: Vec<&'static str>,
) -> (
    String,
    std::sync::Arc<std::sync::atomic::AtomicUsize>,
    std::thread::JoinHandle<()>,
) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = std::sync::Arc::clone(&accepted);
    let handle = std::thread::spawn(move || {
        let mut answers = answers.into_iter().peekable();
        while answers.peek().is_some() {
            let (mut socket, _) = listener.accept().unwrap();
            count.fetch_add(1, Ordering::SeqCst);
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            while let Some(&answer) = answers.peek() {
                let request = read_http_request(&mut socket);
                if !request.starts_with("POST") {
                    break; // the client hung up; take the next connection
                }
                if socket.write_all(chunked_sse_answer(answer).as_bytes()).is_err() {
                    break;
                }
                answers.next();
            }
        }
    });
    (url, accepted, handle)
}

fn read_body(response: ureq::Response) -> std::io::Result<String> {
    use std::io::Read;
    let mut body = String::new();
    response.into_reader().read_to_string(&mut body).map(|_| body)
}

/// An abort handle is detached before its connection returns to the pool:
/// aborting a finished request never reaches the next request on that socket.
#[test]
#[cfg(target_os = "linux")] // where abortable requests pool
fn a_finished_request_abort_never_reaches_the_pooled_connection() {
    let (url, accepted, server) = serve_chunked_counting(vec!["one", "two"]);
    let agent = ureq::AgentBuilder::new().build();
    let first = ureq::AbortHandle::default();
    let reply = agent
        .post(&url)
        .with_abort_handle(first.clone())
        .send_string("{}")
        .unwrap();
    assert!(read_body(reply).unwrap().contains("one"));
    first.abort();
    let reply = agent
        .post(&url)
        .with_abort_handle(ureq::AbortHandle::default())
        .send_string("{}")
        .unwrap();
    assert!(read_body(reply).unwrap().contains("two"));
    server.join().unwrap();
    assert_eq!(accepted.load(Ordering::SeqCst), 1, "the pooled connection carried both");
}

/// A request aborted after its body arrived closes its socket instead of
/// pooling it; the next request connects afresh and still succeeds.
#[test]
fn an_aborted_request_socket_is_never_pooled() {
    let (url, accepted, server) = serve_chunked_counting(vec!["one", "two"]);
    let agent = ureq::AgentBuilder::new().build();
    let first = ureq::AbortHandle::default();
    let reply = agent
        .post(&url)
        .with_abort_handle(first.clone())
        .send_string("{}")
        .unwrap();
    first.abort();
    let _ = read_body(reply);
    let reply = agent
        .post(&url)
        .with_abort_handle(ureq::AbortHandle::default())
        .send_string("{}")
        .unwrap();
    assert!(read_body(reply).unwrap().contains("two"));
    server.join().unwrap();
    assert_eq!(accepted.load(Ordering::SeqCst), 2, "the aborted socket was reused");
}


/// Close a server-side socket the way a provider drops an idle keep-alive
/// connection: a clean FIN, or an RST (SO_LINGER 0).
fn drop_connection(sock: std::net::TcpStream, rst: bool) {
    if rst {
        use std::os::unix::io::AsRawFd;
        let linger = libc::linger {
            l_onoff: 1,
            l_linger: 0,
        };
        // SAFETY: a live socket fd and a correctly sized linger struct.
        unsafe {
            libc::setsockopt(
                sock.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_LINGER,
                &linger as *const libc::linger as *const libc::c_void,
                std::mem::size_of::<libc::linger>() as libc::socklen_t,
            );
        }
    }
    drop(sock);
}

/// How the fixture ends a kept connection before the next answer.
#[derive(Clone, Copy, Debug)]
enum IdleDrop {
    /// Close it while idle, before the next request arrives.
    WhileIdle,
    /// Take the next request, then close without a response: the race where
    /// the idle close crosses the request on the wire.
    AfterRequest,
}

/// Serve `answers` as chunked SSE. After the first answer on a connection the
/// server drops that connection (FIN or RST) as `how` says, then serves the
/// rest on whatever connection comes next. Returns the accepted-socket count
/// and every request that reached it.
fn serve_dropping_kept_connection(
    answers: Vec<&'static str>,
    how: IdleDrop,
    rst: bool,
) -> (
    String,
    std::sync::Arc<std::sync::atomic::AtomicUsize>,
    std::sync::Arc<Mutex<Vec<String>>>,
    std::thread::JoinHandle<()>,
) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
    let (count, seen) = (std::sync::Arc::clone(&accepted), std::sync::Arc::clone(&requests));
    let handle = std::thread::spawn(move || {
        let mut answers = answers.into_iter();
        let (mut first, _) = listener.accept().unwrap();
        count.fetch_add(1, Ordering::SeqCst);
        first
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request = read_http_request(&mut first);
        seen.lock().unwrap().push(request);
        first
            .write_all(chunked_sse_answer(answers.next().unwrap()).as_bytes())
            .unwrap();
        match how {
            IdleDrop::WhileIdle => {
                std::thread::sleep(Duration::from_millis(150));
                drop_connection(first, rst);
            }
            IdleDrop::AfterRequest => {
                let request = read_http_request(&mut first);
                seen.lock().unwrap().push(request);
                drop_connection(first, rst);
            }
        }
        for answer in answers {
            let (mut sock, _) = listener.accept().unwrap();
            count.fetch_add(1, Ordering::SeqCst);
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let request = read_http_request(&mut sock);
            seen.lock().unwrap().push(request);
            sock.write_all(chunked_sse_answer(answer).as_bytes()).unwrap();
        }
    });
    (url, accepted, requests, handle)
}

fn stream_once(club: &HttpClub, cancel: &AtomicBool) -> Result<ClubReply, String> {
    club.stream_body_with_rules(
        serde_json::json!({"model":"fixture", "stream":true, "messages":[{"role":"user","content":"same body"}]}),
        &[],
        cancel,
        &mut |_| {},
        &crate::agent::stream_rules::StreamRules::from_json_for_test("[]"),
    )
}

/// A pooled connection the provider closed while idle (FIN or RST) never
/// costs the next call an error or a retry: the dead socket is discarded and
/// the call goes out once on a fresh connection.
#[test]
#[cfg(target_os = "linux")] // where abortable requests pool
fn a_provider_closed_idle_connection_is_replaced_without_an_error() {
    let _guard = crate::tests::env_lock();
    // No harness-level retry may paper over a dead socket: one attempt only.
    let _retries = EnvGuard::set("ANGEL_HTTP_RETRIES", "0");
    for rst in [false, true] {
        let (url, accepted, requests, server) =
            serve_dropping_kept_connection(vec!["first", "second"], IdleDrop::WhileIdle, rst);
        let club = HttpClub::new("idle-drop-fixture", url, "fixture", None);
        let cancel = AtomicBool::new(false);
        let first = stream_once(&club, &cancel).unwrap();
        assert!(matches!(&first, ClubReply::Text(text) if text == "first"), "{first:?}");
        std::thread::sleep(Duration::from_millis(400)); // the server drops it meanwhile
        let second = stream_once(&club, &cancel);
        assert!(
            matches!(&second, Ok(ClubReply::Text(text)) if text == "second"),
            "rst={rst}: {second:?}"
        );
        server.join().unwrap();
        assert_eq!(accepted.load(Ordering::SeqCst), 2, "rst={rst}");
        assert_eq!(requests.lock().unwrap().len(), 2, "rst={rst}: one request per call");
    }
}

/// The idle close crossing the next request on the wire: the server reads it
/// and closes without a byte of response. The request is sent once more, on a
/// fresh connection and with the same body, and the call succeeds.
#[test]
#[cfg(target_os = "linux")]
fn a_request_lost_to_an_idle_close_is_resent_once_on_a_fresh_connection() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_HTTP_RETRIES", "0");
    for rst in [false, true] {
        let (url, accepted, requests, server) =
            serve_dropping_kept_connection(vec!["first", "second"], IdleDrop::AfterRequest, rst);
        let club = HttpClub::new("idle-race-fixture", url, "fixture", None);
        let cancel = AtomicBool::new(false);
        let first = stream_once(&club, &cancel).unwrap();
        assert!(matches!(&first, ClubReply::Text(text) if text == "first"), "{first:?}");
        std::thread::sleep(Duration::from_millis(100));
        let second = stream_once(&club, &cancel);
        assert!(
            matches!(&second, Ok(ClubReply::Text(text)) if text == "second"),
            "rst={rst}: {second:?}"
        );
        server.join().unwrap();
        assert_eq!(accepted.load(Ordering::SeqCst), 2, "rst={rst}");
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 3, "rst={rst}: the lost request and its one resend");
        let body = |request: &String| request.split_once("\r\n\r\n").map(|(_, b)| b.to_string());
        assert_eq!(body(&requests[1]), body(&requests[2]), "the resend carries the same body");
    }
}

/// Esc while the call is being resent after a dead recycled connection: the
/// first attempt dies on the kept socket, the resend reaches a fresh one that
/// never answers, and the operator cancels. The call ends at once, with no
/// reply and nothing left running on the wire.
#[test]
#[cfg(target_os = "linux")]
fn esc_during_the_resend_after_a_dead_kept_connection_ends_the_call() {
    use std::io::{Read, Write};
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_HTTP_RETRIES", "0");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (resent, resend_seen) = std::sync::mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let peer_cancel = Arc::clone(&cancel);
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let accept = || {
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < until => {
                            std::thread::sleep(Duration::from_millis(2));
                        }
                    Err(error) => panic!("bounded cancel fixture accept: {error}"),
                }
            }
        };
        let mut kept = accept();
        kept.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let _ = read_http_request(&mut kept);
        kept.write_all(chunked_sse_answer("first").as_bytes()).unwrap();
        let _ = read_http_request(&mut kept);
        drop_connection(kept, true); // the idle close crossed the request
        let mut fresh = accept();
        fresh.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let request = read_http_request(&mut fresh);
        resent.send(request).unwrap();
        // Never answer; the client's cancel must close this socket.
        let mut byte = [0u8];
        assert_eq!(
            fresh.read(&mut byte).expect("cancel must produce EOF, not timeout or a read error"),
            0,
            "cancel must close the resend's socket"
        );
        let peer_closed_at = Instant::now();
        assert!(peer_cancel.load(Ordering::Acquire), "peer EOF preceded cancellation");
        peer_closed_at
    });
    let club = HttpClub::new("idle-cancel-fixture", url, "fixture", None);
    let first = stream_once(&club, &cancel).unwrap();
    assert!(matches!(&first, ClubReply::Text(text) if text == "first"), "{first:?}");
    std::thread::sleep(Duration::from_millis(100));
    let (second, cancelled_at, returned_at, returned_after_cancel) = std::thread::scope(|scope| {
        let cancel_ref = &cancel;
        let canceller = scope.spawn(move || {
            let request = resend_seen.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(request.starts_with("POST"), "{request:?}");
            std::thread::sleep(Duration::from_millis(50));
            let at = Instant::now();
            cancel_ref.store(true, Ordering::Release);
            at
        });
        let second = stream_once(&club, &cancel);
        let returned_at = Instant::now();
        let returned_after_cancel = cancel.load(Ordering::Acquire);
        let cancelled_at = canceller.join().unwrap();
        (second, cancelled_at, returned_at, returned_after_cancel)
    });
    let peer_closed_at = server.join().unwrap();
    assert!(returned_after_cancel, "provider returned before cancellation was signalled");
    let client_latency = returned_at.checked_duration_since(cancelled_at)
        .expect("provider returned before cancellation timestamp");
    assert!(client_latency < Duration::from_secs(2), "cancel was not prompt: {client_latency:?}");
    assert!(
        !matches!(&second, Ok(ClubReply::Text(text)) if !text.is_empty()),
        "a cancelled call must not yield a reply: {second:?}"
    );
    let peer_latency = peer_closed_at.checked_duration_since(cancelled_at)
        .expect("peer EOF preceded cancellation timestamp");
    assert!(peer_latency < Duration::from_secs(2), "peer EOF was late: {peer_latency:?}");
    eprintln!("H01 case=pooled_cancel peer_eof=true peer_latency_ms={}", peer_latency.as_millis());
}

/// Esc that lands while the request is still on the dead kept socket: the
/// abort closes it, and the resend never goes out: an aborted request is not
/// sent again on any connection.
#[test]
#[cfg(target_os = "linux")]
fn esc_before_the_resend_sends_nothing_more() {
    use std::io::{Read, Write};
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_HTTP_RETRIES", "0");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (taken, request_taken) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut kept, _) = listener.accept().unwrap();
        kept.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let _ = read_http_request(&mut kept);
        kept.write_all(chunked_sse_answer("first").as_bytes()).unwrap();
        let _ = read_http_request(&mut kept);
        taken.send(()).unwrap();
        // Hold the request unanswered until the client's abort closes it.
        let mut byte = [0u8];
        let _ = kept.read(&mut byte);
        listener.set_nonblocking(true).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert!(listener.accept().is_err(), "a cancelled call resent its request");
    });
    let club = HttpClub::new("idle-cancel-early-fixture", url, "fixture", None);
    let cancel = AtomicBool::new(false);
    let first = stream_once(&club, &cancel).unwrap();
    assert!(matches!(&first, ClubReply::Text(text) if text == "first"), "{first:?}");
    std::thread::sleep(Duration::from_millis(100));
    std::thread::scope(|scope| {
        let cancel_ref = &cancel;
        scope.spawn(move || {
            request_taken.recv_timeout(Duration::from_secs(5)).unwrap();
            cancel_ref.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        let second = stream_once(&club, &cancel);
        assert!(started.elapsed() < Duration::from_secs(2), "the cancel was not prompt");
        assert!(
            !matches!(&second, Ok(ClubReply::Text(text)) if !text.is_empty()),
            "a cancelled call must not yield a reply: {second:?}"
        );
    });
    server.join().unwrap();
}

/// Answer the window probes as a slow provider would: every path after
/// `delay`, each on its own connection, recording the paths asked.
fn serve_slow_probes(
    delay: Duration,
) -> (
    String,
    std::sync::Arc<Mutex<Vec<String>>>,
    std::sync::Arc<AtomicBool>,
    std::thread::JoinHandle<()>,
) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let paths = std::sync::Arc::new(Mutex::new(Vec::new()));
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let (seen, halt) = (std::sync::Arc::clone(&paths), std::sync::Arc::clone(&stop));
    let handle = std::thread::spawn(move || {
        while !halt.load(Ordering::Relaxed) {
            let Ok((mut sock, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let _ = sock.set_nonblocking(false);
            let _ = sock.set_read_timeout(Some(Duration::from_secs(2)));
            let request = read_http_request(&mut sock);
            let path = request.split_whitespace().nth(1).unwrap_or("").to_string();
            seen.lock().unwrap().push(path.clone());
            std::thread::sleep(delay);
            let response = if path.ends_with("/models") {
                let body = r#"{"data":[{"id":"fixture","context_length":32768}]}"#;
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
            } else {
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            };
            let _ = sock.write_all(response.as_bytes());
        }
    });
    (base, paths, stop, handle)
}

/// Prewarm starts the seat's first window probe at task start and publishes
/// nothing early: until the first reader asks, the cache stays empty as
/// before; that reader gets the same metadata from the same requests, without
/// waiting for a probe that already finished.
#[test]
fn prewarm_runs_the_window_probe_early_and_publishes_it_where_it_always_was() {
    let _guard = crate::tests::env_lock();
    let delay = Duration::from_millis(120);
    let (base, plain_paths, stop, server) = serve_slow_probes(delay);
    let plain = Arc::new(HttpClub::new("prewarm-plain", base, "fixture", None));
    let started = Instant::now();
    let plain_meta = plain.metadata();
    let plain_wait = started.elapsed();
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();

    let (base, warm_paths, stop, server) = serve_slow_probes(delay);
    let warm = Arc::new(HttpClub::new("prewarm-warm", base, "fixture", None));
    Club::prewarm(Arc::clone(&warm));
    Club::prewarm(Arc::clone(&warm)); // a second prewarm starts nothing more
    std::thread::sleep(plain_wait + Duration::from_millis(200));
    assert_eq!(warm.metadata_cached(), None, "prewarm must not publish early");
    let started = Instant::now();
    let warm_meta = warm.metadata();
    let warm_wait = started.elapsed();
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();

    assert!(plain_meta.is_some());
    assert_eq!(warm_meta, plain_meta, "the same window, learned the same way");
    assert_eq!(
        *warm_paths.lock().unwrap(),
        *plain_paths.lock().unwrap(),
        "the same probe requests, once"
    );
    assert!(plain_wait >= delay, "{plain_wait:?}");
    assert!(warm_wait < delay, "the first reader waited for a finished probe: {warm_wait:?}");
}

/// A provider error that says `x-should-retry: false` is sent once: no club
/// retry, whatever its status, and the error carries the veto so the turn
/// does not replay it either. `true` or no header keep the status rules.
#[test]
fn a_provider_retry_veto_is_sent_once() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_HTTP_RETRIES", "1");
    let _rate_retries = EnvGuard::set("ANGEL_HTTP_RATELIMIT_RETRIES", "2");
    let _backoff = EnvGuard::set("ANGEL_HTTP_BACKOFF_MS", "1");
    let _rate_backoff = EnvGuard::set("ANGEL_HTTP_RATELIMIT_BACKOFF_MS", "1");
    let answer = |status: &str, header: Option<&str>| {
        let body = r#"{"error":{"message":"not this request"}}"#;
        let header = header
            .map(|value| format!("x-should-retry: {value}\r\n"))
            .unwrap_or_default();
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    };
    for (status, header, expected_requests, vetoed) in [
        ("503 Service Unavailable", Some("false"), 1, true),
        ("429 Too Many Requests", Some("false"), 1, true),
        ("503 Service Unavailable", Some("true"), 2, false),
        ("503 Service Unavailable", None, 2, false),
    ] {
        let (base, requests, handle) = serve_seq(vec![answer(status, header); 3]);
        let club = HttpClub::new("veto-fixture", base, "fixture", None);
        let error = stream_once(&club, &AtomicBool::new(false)).expect_err("an error status");
        handle.join().unwrap();
        assert_eq!(
            requests.lock().unwrap().len(),
            expected_requests,
            "{status} {header:?}: {error}"
        );
        assert_eq!(error.contains(PROVIDER_RETRY_VETO), vetoed, "{error}");
        assert!(error.starts_with(&format!("HTTP {}", &status[..3])), "{error}");
    }
}

/// A certificate the trust store rejected is the one transport failure that
/// cannot clear on retry; refused connections, resets and DNS stay retryable.
#[test]
fn only_a_rejected_certificate_is_a_permanent_transport_failure() {
    for error in [
        "https://api.x.ai/v1/chat/completions: Connection Failed: tls connection init failed: invalid peer certificate: UnknownIssuer",
        "Connection Failed: tls connection init failed: invalid peer certificate: NotValidForName",
        "tls connection init failed: invalid peer certificate: Expired",
    ] {
        assert!(error_indicates_certificate_rejected(error), "{error}");
    }
    for error in [
        "Connection Failed: Connect error: Connection refused (os error 111)",
        "Network Error: Connection reset by peer (os error 104)",
        "Dns Failed: resolve dns name 'api.x.ai:443': failed to lookup address information",
        "HTTP 503: overloaded",
    ] {
        assert!(!error_indicates_certificate_rejected(error), "{error}");
    }
}
