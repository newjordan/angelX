use super::*;
use crate::club::{ClubReply, StreamDelta};
use std::collections::VecDeque;

struct StubClub {
    name: String,
    replies: Mutex<VecDeque<String>>,
    prompts: Mutex<Vec<String>>,
}

impl StubClub {
    fn shared(name: &str, replies: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_string(),
            replies: Mutex::new(replies.iter().map(|s| s.to_string()).collect()),
            prompts: Mutex::new(Vec::new()),
        })
    }

    fn prompts(&self) -> Vec<String> {
        self.prompts.lock().unwrap().clone()
    }
}

impl Club for StubClub {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        self.prompts.lock().unwrap().push(prompt.to_string());
        Ok(self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| "done".to_string()))
    }

    fn label(&self) -> &str {
        &self.name
    }
}

struct TrackingClub {
    active: AtomicUsize,
    max_active: AtomicUsize,
    calls: AtomicUsize,
}

impl TrackingClub {
    fn shared() -> Arc<Self> {
        Arc::new(Self {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
        })
    }
}

impl Club for TrackingClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(35));
        self.active.fetch_sub(1, Ordering::SeqCst);
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(format!("result-{call}"))
    }

    fn label(&self) -> &str {
        "tracking"
    }
}

struct ConcurrentEffortClub {
    effort: Mutex<String>,
    set_calls: AtomicUsize,
    rendezvous: std::sync::Barrier,
    seen: Mutex<Vec<(String, String)>>,
}

impl ConcurrentEffortClub {
    fn shared() -> Arc<Self> {
        Arc::new(Self {
            effort: Mutex::new("medium".to_string()),
            set_calls: AtomicUsize::new(0),
            rendezvous: std::sync::Barrier::new(2),
            seen: Mutex::new(Vec::new()),
        })
    }

    fn node_id(messages: &[ChatMsg]) -> String {
        let system = messages
            .first()
            .map(|message| message.content.as_ref())
            .unwrap_or_default();
        ["low", "high"]
            .into_iter()
            .find(|id| system.contains(&format!("node '{id}'")))
            .unwrap_or("unknown")
            .to_string()
    }

    fn record(&self, messages: &[ChatMsg], effort: String) -> Result<ClubReply, String> {
        self.rendezvous.wait();
        let node = Self::node_id(messages);
        self.seen.lock().unwrap().push((node.clone(), effort));
        Ok(ClubReply::Text(format!("{node}-answer")))
    }
}

impl Club for ConcurrentEffortClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("concurrent-effort test must use structured chat".to_string())
    }

    fn label(&self) -> &str {
        "shared-effort"
    }

    fn reasoning_effort(&self) -> Option<String> {
        self.effort.lock().ok().map(|effort| effort.clone())
    }

    fn reasoning_levels(&self) -> &[String] {
        static LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        LEVELS.get_or_init(|| vec!["low".to_string(), "high".to_string()])
    }

    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        let canonical = self
            .reasoning_levels()
            .iter()
            .find(|level| level.eq_ignore_ascii_case(requested.trim()))?
            .clone();
        self.set_calls.fetch_add(1, Ordering::SeqCst);
        *self.effort.lock().ok()? = canonical.clone();
        Some(canonical)
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        let effort = self
            .reasoning_effort()
            .ok_or_else(|| "missing shared effort".to_string())?;
        self.record(messages, effort)
    }

    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        _tools: &[ToolDef],
        effort: Option<&str>,
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.record(
            messages,
            effort
                .ok_or_else(|| "missing per-call effort".to_string())?
                .to_string(),
        )
    }
}

struct FailureCancellationClub {
    fail_node: &'static str,
    sibling_started: AtomicBool,
    sibling_saw_cancel: AtomicBool,
}

impl FailureCancellationClub {
    fn failing(fail_node: &'static str) -> Arc<Self> {
        Arc::new(Self {
            fail_node,
            sibling_started: AtomicBool::new(false),
            sibling_saw_cancel: AtomicBool::new(false),
        })
    }
}

impl Club for FailureCancellationClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("failure-cancellation test must use chat_streaming".to_string())
    }

    fn label(&self) -> &str {
        "failure-cancellation"
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        _tools: &[ToolDef],
        cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        let system = messages
            .first()
            .map(|m| m.content.as_ref())
            .unwrap_or_default();
        if system.contains(&format!("node '{}'", self.fail_node)) {
            // Exercise the adversarial ordering: the sibling must enter
            // the provider before the named node fails. Arrival order must
            // never decide which node owns the failure.
            if self.fail_node == "first" {
                let deadline = Instant::now() + Duration::from_secs(2);
                while !self.sibling_started.load(Ordering::SeqCst) {
                    assert!(Instant::now() < deadline, "sibling never started");
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            return Err("forced graph node failure".to_string());
        }

        self.sibling_started.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if cancel.load(Ordering::SeqCst) {
                self.sibling_saw_cancel.store(true, Ordering::SeqCst);
                return Err("sibling observed graph cancellation".to_string());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err("sibling timed out waiting for graph cancellation".to_string())
    }
}

struct WindDownAttributionClub {
    started: AtomicBool,
}

impl WindDownAttributionClub {
    fn shared() -> Arc<Self> {
        Arc::new(Self {
            started: AtomicBool::new(false),
        })
    }
}

impl Club for WindDownAttributionClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("wind-down attribution test must use chat_streaming".to_string())
    }

    fn label(&self) -> &str {
        "wind-down-attribution"
    }

    fn chat_streaming(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.started.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(4);
        while !cancel.load(Ordering::Acquire) {
            if Instant::now() >= deadline {
                return Err("fixture timed out before graph cancellation".to_string());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err("provider wrapper reported cooperative cancellation".to_string())
    }
}

struct RetryEpochClub {
    a_calls: AtomicUsize,
    b_calls: AtomicUsize,
    c_calls: AtomicUsize,
    review_calls: AtomicUsize,
    sink_calls: AtomicUsize,
    sink_prompt: Mutex<Option<String>>,
}

impl RetryEpochClub {
    fn shared() -> Arc<Self> {
        Arc::new(Self {
            a_calls: AtomicUsize::new(0),
            b_calls: AtomicUsize::new(0),
            c_calls: AtomicUsize::new(0),
            review_calls: AtomicUsize::new(0),
            sink_calls: AtomicUsize::new(0),
            sink_prompt: Mutex::new(None),
        })
    }

    fn wait_for(counter: &AtomicUsize, expected: usize, label: &str) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(2);
        while counter.load(Ordering::SeqCst) < expected {
            if Instant::now() >= deadline {
                return Err(format!("timed out waiting for {label}"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
}

impl Club for RetryEpochClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("retry-epoch test must use structured chat".to_string())
    }

    fn label(&self) -> &str {
        "retry-epoch"
    }

    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let system = messages
            .first()
            .map(|message| message.content.as_ref())
            .unwrap_or_default();
        let prompt = messages
            .last()
            .map(|message| message.content.as_ref())
            .unwrap_or_default();

        if system.contains("node 'a'") {
            let attempt = self.a_calls.fetch_add(1, Ordering::SeqCst) + 1;
            return Ok(ClubReply::Text(format!("a-epoch-{attempt}")));
        }
        if system.contains("node 'b'") {
            self.b_calls.fetch_add(1, Ordering::SeqCst);
            let epoch = if prompt.contains("a-epoch-2") { 2 } else { 1 };
            return Ok(ClubReply::Text(format!("b-epoch-{epoch}")));
        }
        if system.contains("node 'c'") {
            let attempt = self.c_calls.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                // Keep the first C generation in flight until A has been
                // retried, proving that its late landing is discarded.
                Self::wait_for(&self.a_calls, 2, "retry target generation two")?;
                return Ok(ClubReply::Text("c-stale-from-a1".to_string()));
            }
            if !prompt.contains("a-epoch-2") {
                return Err("fresh C attempt did not receive A generation two".to_string());
            }
            return Ok(ClubReply::Text("c-fresh-from-a2".to_string()));
        }
        if system.contains("node 'review'") {
            let attempt = self.review_calls.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                Self::wait_for(&self.c_calls, 1, "sibling C to enter flight")?;
                return Ok(ClubReply::Text(
                    "VERDICT: FAIL — retry the root generation".to_string(),
                ));
            }
            if !prompt.contains("b-epoch-2") {
                return Err("review retry did not receive B generation two".to_string());
            }
            return Ok(ClubReply::Text("VERDICT: PASS".to_string()));
        }
        if system.contains("node 'sink'") {
            self.sink_calls.fetch_add(1, Ordering::SeqCst);
            *self.sink_prompt.lock().unwrap() = Some(prompt.to_string());
            return Ok(ClubReply::Text("sink-complete".to_string()));
        }
        Err("unknown retry-epoch graph node".to_string())
    }
}

fn node(id: &str, prompt: &str, deps: &[&str]) -> GraphNodeSpec {
    GraphNodeSpec {
        id: id.to_string(),
        prompt: prompt.to_string(),
        persona: None,
        club: None,
        tools: None,
        depends_on: deps.iter().map(|s| s.to_string()).collect(),
        effort: None,
        trainable: false,
        pool: None,
        pool_max_inflight: None,
        gate: None,
    }
}

fn spec(name: &str, nodes: Vec<GraphNodeSpec>) -> GraphSpec {
    GraphSpec {
        name: name.to_string(),
        description: String::new(),
        deadline_secs: Some(60),
        token_budget: None,
        nodes,
    }
}

fn engine(club: Arc<StubClub>) -> AgentGraphEngine {
    // A quiet per-engine workspace: fingerprinting the shared temp root
    // sees other tests' churn and trips run_turn's verification nudge.
    static WS: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "agent-graph-test-{}-{}",
        std::process::id(),
        WS.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::create_dir_all(&dir);
    AgentGraphEngine::new(dir, Some(club as Arc<dyn Club>), Vec::new()).without_persistence()
}

#[test]
fn agent_graph_live_shaped_final_usage_without_limits() {
    use std::io::{Read, Write};
    let _lock = crate::tests::env_lock();
    let _deadline = crate::tests::TestEnvGuard::unset("ANGEL_GRAPH_DEADLINE_SECS");
    let _tokens = crate::tests::TestEnvGuard::unset("ANGEL_FORMATION_TOKEN_BUDGET");
    let _wall = crate::tests::TestEnvGuard::unset("ANGEL_FORMATION_WALL_SECS");
    crate::club::resync_stream_knobs_from_env();
    let _allocation = super::super::formation_budget::enter(None);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        let headers = String::from_utf8(request).unwrap();
        let length: usize = headers
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().unwrap())
            })
            .unwrap();
        socket.read_exact(&mut vec![0; length]).unwrap();
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        socket.write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"complete\"},\"finish_reason\":\"stop\"}]}\n\n").unwrap();
        socket.flush().unwrap();
        std::thread::sleep(Duration::from_millis(30));
        socket.write_all(b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":120,\"completion_tokens\":20,\"prompt_tokens_details\":{\"cached_tokens\":30},\"completion_tokens_details\":{\"reasoning_tokens\":5}}}\n\ndata: [DONE]\n\n").unwrap();
    });
    let club = Arc::new(crate::club::HttpClub::new(
        "g01g-fixture",
        &url,
        "g01g-fixture",
        None,
    ));
    let mut engine = engine(StubClub::shared("unused", &[]));
    engine.self_club = Some(club);
    let mut graph = spec("final-usage", vec![node("fanin", "answer", &[])]);
    graph.deadline_secs = None;
    graph.nodes[0].tools = Some("none".into());
    let outcome = engine.run(&graph, "task", &AtomicBool::new(false)).unwrap();
    server.join().unwrap();
    let allocation = outcome.episode.token_allocation.as_ref().unwrap();
    assert_eq!(allocation["paid_spent"], 110);
    assert_eq!(allocation["calls"][0]["normalized_usage"]["input"], 120);
    assert_eq!(allocation["calls"][0]["normalized_usage"]["output"], 20);
    assert_eq!(allocation["token_budget"], serde_json::Value::Null);
    assert!(
        outcome
            .episode
            .traces
            .iter()
            .all(|t| t.wall_remaining_ms.is_none())
    );
    outcome.episode.validate().unwrap();
    println!(
        "G01g final-only SSE usage: paid_spent=110; no allocation or graph deadline; {allocation}"
    );
}

#[test]
fn agent_graph_stalled_stream_retries_and_sums_usage() {
    use std::io::{Read, Write};
    let _lock = crate::tests::env_lock();
    let _stall = crate::tests::TestEnvGuard::set("ANGEL_STREAM_STALL_SECS", "1");
    let _backoff = crate::tests::TestEnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "1");
    let _deadline = crate::tests::TestEnvGuard::unset("ANGEL_GRAPH_DEADLINE_SECS");
    let _tokens = crate::tests::TestEnvGuard::unset("ANGEL_FORMATION_TOKEN_BUDGET");
    let _wall = crate::tests::TestEnvGuard::unset("ANGEL_FORMATION_WALL_SECS");
    let _allocation = super::super::formation_budget::enter(None);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for n in 0..3 {
            let Ok((mut socket, _)) = listener.accept() else {
                break;
            };
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                if socket.read_exact(&mut byte).is_err() {
                    return;
                }
                request.push(byte[0]);
            }
            let headers = String::from_utf8_lossy(&request);
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap_or(0);
            let _ = socket.read_exact(&mut vec![0; length]);
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            if n == 0 {
                let _ = socket
                    .write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n");
                let _ = socket.flush();
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
            let _ = socket.write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"complete\"},\"finish_reason\":\"stop\"}]}\n\n");
            let _ = socket.write_all(b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":50,\"completion_tokens\":10,\"prompt_tokens_details\":{\"cached_tokens\":0}}}\n\ndata: [DONE]\n\n");
        }
    });
    let club = Arc::new(crate::club::HttpClub::new(
        "g01i-stall",
        &url,
        "g01i-stall",
        None,
    ));
    let mut engine = engine(StubClub::shared("unused", &[]));
    engine.self_club = Some(club);
    let mut graph = spec(
        "stall-retry",
        vec![
            node("worker", "work", &[]),
            node("fanin", "join", &["worker"]),
        ],
    );
    graph.deadline_secs = None;
    graph.nodes[0].tools = Some("none".into());
    graph.nodes[1].tools = Some("none".into());
    let outcome = engine.run(&graph, "task", &AtomicBool::new(false)).unwrap();
    let _ = server.join();
    assert!(outcome.episode.fanin.finished_with_result);
    let worker = outcome
        .episode
        .traces
        .iter()
        .find(|t| t.role == "worker")
        .unwrap();
    assert!(
        worker.stall_retries >= 1,
        "stall_retries={}",
        worker.stall_retries
    );
    let allocation = outcome.episode.token_allocation.as_ref().unwrap();
    // The stalled attempt sent no usage, so the total stays unknown (accounting law);
    // the reported subset is numeric and the unreported attempt is counted.
    assert!(
        allocation["paid_spent_reported"].as_u64().is_some(),
        "paid_spent_reported should be numeric: {allocation}"
    );
    assert!(
        allocation["unreported_attempts"].as_u64().unwrap_or(0) >= 1,
        "the stalled attempt must be counted as unreported: {allocation}"
    );
    outcome.episode.validate().unwrap();
}

/// A cancelled graph run must not sit out a whole backoff: the seat's retry
/// wait is sliced so the cancel lands within one slice, and an already
/// cancelled seat issues no wait at all.
#[test]
fn graph_retry_backoff_wait_is_cancellable() {
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancel);
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        signal.store(true, Ordering::Relaxed);
    });
    let started = Instant::now();
    wait_graph_backoff(&cancel, 10_000, 0);
    let waited = started.elapsed();
    setter.join().unwrap();
    assert!(
        waited < Duration::from_secs(1),
        "cancel during backoff must land within a slice ({waited:?})"
    );
    cancel.store(true, Ordering::Relaxed);
    let started = Instant::now();
    wait_graph_backoff(&cancel, 10_000, 3);
    assert!(started.elapsed() < Duration::from_millis(50));
}

/// Scripted provider for graph retry-budget tests: every POST is counted and
/// answered with one shape, so the number of provider attempts a node spends
/// is exact. `cut` streams real prose, then holds the socket past the stall
/// window (a recoverable cut); `permanent` answers 401 with stall context in
/// the body (a permanent failure wearing a recoverable spelling).
fn graph_fault_server(
    mode: &'static str,
    hold_ms: u64,
) -> (String, Arc<AtomicUsize>, Arc<AtomicBool>) {
    use std::io::{Read as _, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let server_hits = Arc::clone(&hits);
    let server_stop = Arc::clone(&stop);
    std::thread::spawn(move || {
        while !server_stop.load(Ordering::Relaxed) {
            let mut socket = match listener.accept() {
                Ok((socket, _)) => socket,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 4096];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                match socket.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&chunk[..n]),
                }
            }
            if request.starts_with(b"GET ") {
                let body = r#"{"data":[{"id":"g01j","context_length":131072}]}"#;
                let _ = write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                continue;
            }
            // Drain the body so the client never sees a reset on close.
            let head = String::from_utf8_lossy(&request).to_ascii_lowercase();
            let length: usize = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(0);
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| index + 4)
                .unwrap_or(request.len());
            let mut remaining = length.saturating_sub(request.len() - header_end);
            while remaining > 0 {
                match socket.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => remaining = remaining.saturating_sub(n),
                }
            }
            server_hits.fetch_add(1, Ordering::SeqCst);
            if mode == "permanent" {
                let body = r#"{"error":{"message":"stream stalled upstream; invalid api key","type":"authentication_error"}}"#;
                let _ = write!(
                    socket,
                    "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                continue;
            }
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            let _ = socket.write_all(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"partial prose\"}}]}\n\n",
            );
            let _ = socket.flush();
            std::thread::sleep(Duration::from_millis(hold_ms));
        }
    });
    (url, hits, stop)
}

/// An explicit provider budget is a hard operator limit. A tool-bearing node
/// is a full turn whose own budget owns recovery, so the graph must not start
/// another turn after that allowance is spent: one attempt plus one retry is
/// two provider requests, never four.
#[test]
fn agent_graph_tool_bearing_node_spends_the_explicit_retry_budget_once() {
    let _lock = crate::tests::env_lock();
    let _stall = crate::tests::TestEnvGuard::set("ANGEL_STREAM_STALL_SECS", "1");
    let _retries = crate::tests::TestEnvGuard::set("ANGEL_PROVIDER_RETRIES", "1");
    let _backoff = crate::tests::TestEnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _deadline = crate::tests::TestEnvGuard::unset("ANGEL_GRAPH_DEADLINE_SECS");
    let _tokens = crate::tests::TestEnvGuard::unset("ANGEL_FORMATION_TOKEN_BUDGET");
    let _wall = crate::tests::TestEnvGuard::unset("ANGEL_FORMATION_WALL_SECS");
    let _allocation = super::super::formation_budget::enter(None);
    let (url, hits, stop) = graph_fault_server("cut", 1_200);
    let club = Arc::new(crate::club::HttpClub::new(
        "g01j-budget",
        &url,
        "g01j-budget",
        None,
    ));
    let mut engine = engine(StubClub::shared("unused", &[]));
    engine.self_club = Some(club);
    let mut graph = spec("budget-once", vec![node("worker", "work", &[])]);
    graph.deadline_secs = None;
    graph.nodes[0].tools = Some("read_only".into());
    let failure = match engine.run(&graph, "task", &AtomicBool::new(false)) {
        Ok(_) => panic!("a stalling provider must fail the worker node"),
        Err(GraphRunError::Run(failure)) => *failure,
        Err(GraphRunError::Preflight(message)) => panic!("graph preflight failed: {message}"),
    };
    stop.store(true, Ordering::Relaxed);
    let attempts = hits.load(Ordering::SeqCst);
    assert_eq!(
        attempts, 2,
        "one attempt plus the explicit single retry, with no node-level second turn"
    );
    assert!(!failure.episode.fanin.finished_with_result);
    let worker = failure
        .episode
        .traces
        .iter()
        .find(|t| t.role == "worker")
        .unwrap();
    assert_eq!(
        worker.stall_retries, 0,
        "the graph adds no retry of its own; the turn spent the budget"
    );
    assert_eq!(
        worker.termination.kind,
        GraphTraceTerminationKind::ProviderFailure
    );
    println!("G01j explicit budget: {attempts} provider attempts for ANGEL_PROVIDER_RETRIES=1");
}

/// A permanent failure that also reads like a stall must stay actionable in a
/// toolless node: the graph's own retry may not launder a rejected credential
/// into repeated paid attempts.
#[test]
fn agent_graph_toolless_node_does_not_retry_a_permanent_failure() {
    let _lock = crate::tests::env_lock();
    let _retries = crate::tests::TestEnvGuard::set("ANGEL_PROVIDER_RETRIES", "2");
    let _backoff = crate::tests::TestEnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _deadline = crate::tests::TestEnvGuard::unset("ANGEL_GRAPH_DEADLINE_SECS");
    let _tokens = crate::tests::TestEnvGuard::unset("ANGEL_FORMATION_TOKEN_BUDGET");
    let _wall = crate::tests::TestEnvGuard::unset("ANGEL_FORMATION_WALL_SECS");
    let _allocation = super::super::formation_budget::enter(None);
    let (url, hits, stop) = graph_fault_server("permanent", 0);
    let club = Arc::new(crate::club::HttpClub::new(
        "g01k-permanent",
        &url,
        "g01k-permanent",
        None,
    ));
    let mut engine = engine(StubClub::shared("unused", &[]));
    engine.self_club = Some(club);
    let mut graph = spec("permanent-once", vec![node("worker", "work", &[])]);
    graph.deadline_secs = None;
    graph.nodes[0].tools = Some("none".into());
    let failure = match engine.run(&graph, "task", &AtomicBool::new(false)) {
        Ok(_) => panic!("a rejected credential must fail the worker node"),
        Err(GraphRunError::Run(failure)) => *failure,
        Err(GraphRunError::Preflight(message)) => panic!("graph preflight failed: {message}"),
    };
    stop.store(true, Ordering::Relaxed);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "a rejected credential is not retried, stall context or not"
    );
    assert!(!failure.episode.fanin.finished_with_result);
    assert!(
        failure.reason.contains("HTTP 401"),
        "the actionable error must survive to the receipt: {}",
        failure.reason
    );
    let worker = failure
        .episode
        .traces
        .iter()
        .find(|t| t.role == "worker")
        .unwrap();
    assert_eq!(worker.stall_retries, 0);
}

struct AllocatedGraphClub {
    calls: AtomicUsize,
    walls: Mutex<Vec<Duration>>,
}
impl Club for AllocatedGraphClub {
    fn supports_formation_budget(&self) -> bool {
        true
    }
    fn label(&self) -> &str {
        "allocated-graph"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        self.walls
            .lock()
            .unwrap()
            .push(super::super::formation_budget::request_wall_remaining().unwrap());
        let budget = super::super::formation_budget::current().unwrap();
        let reservation = budget.reserve(self.label(), 10, 10)?;
        self.calls.fetch_add(1, Ordering::SeqCst);
        reservation.settle(Some(crate::club::UsageObservation {
            raw: [Some(10), Some(10), Some(2), Some(0), Some(0)],
            contract: crate::club::UsageContract {
                cache: crate::club::CacheConvention::Included,
                reasoning: crate::club::ReasoningConvention::Included,
            },
            ..Default::default()
        }));
        Ok("completed node".into())
    }
}

/// Operator-ordered F01 contract: graph nodes and fan-in complete despite over-allocation.
#[test]
fn agent_graph_overallocation_admits_nodes_and_fanin() {
    let _lock = crate::tests::env_lock();
    let budget = super::super::formation_budget::Budget::new(Some(20), None);
    let _scope = super::super::formation_budget::enter(Some(budget.clone()));
    let club = Arc::new(AllocatedGraphClub {
        calls: AtomicUsize::new(0),
        walls: Mutex::new(Vec::new()),
    });
    let mut engine = engine(StubClub::shared("unused", &[]));
    engine.self_club = Some(club.clone());
    let graph = spec(
        "budget-overallocation",
        vec![
            node("planner", "plan", &[]),
            node("worker", "work", &["planner"]),
            node("fanin", "join", &["worker"]),
        ],
    );
    let outcome = engine.run(&graph, "task", &AtomicBool::new(false)).unwrap();
    assert_eq!(club.calls.load(Ordering::SeqCst), 3);
    assert_eq!(budget.snapshot()["reserved"], 0);
    assert_eq!(budget.snapshot()["paid_spent"], 60);
    assert_eq!(budget.snapshot()["remaining"], -40);
    assert_eq!(
        outcome.episode.token_allocation.as_ref().unwrap()["over_allocation_tokens"],
        40
    );
    assert!(outcome.episode.incomplete_by_budget.is_empty());
    for role in ["worker", "fanin"] {
        let trace = outcome
            .episode
            .traces
            .iter()
            .find(|t| t.role == role)
            .unwrap();
        assert_eq!(trace.termination.kind, GraphTraceTerminationKind::Answer);
    }
    assert!(
        outcome
            .episode
            .traces
            .iter()
            .all(|t| t.wall_remaining_ms.is_some_and(|ms| ms <= 60_000))
    );
    outcome.episode.validate().unwrap();
}

/// Operator-ordered F01 contract: allocation mismatch and untracked providers never deny nodes.
#[test]
fn agent_graph_inherited_allocation_reports_untracked_provider_without_refusal() {
    let _lock = crate::tests::env_lock();
    let budget = super::super::formation_budget::Budget::new(Some(0), None);
    let _scope = super::super::formation_budget::enter(Some(budget));
    let engine = engine(StubClub::shared("untracked", &["complete"]));
    let mut graph = spec("untracked-allocation", vec![node("worker", "work", &[])]);
    graph.token_budget = Some(4000);
    let outcome = engine.run(&graph, "task", &AtomicBool::new(false)).unwrap();
    let allocation = outcome.episode.token_allocation.as_ref().unwrap();
    assert_eq!(allocation["token_budget"], 0);
    assert_eq!(allocation["untracked_provider"], true);
    assert!(allocation["spent"].is_null());
    assert!(allocation["paid_spent"].is_null());
    assert!(outcome.episode.incomplete_by_budget.is_empty());
}

#[test]
fn agent_graph_shared_allocation_and_remaining_wall_reach_fanin() {
    let _lock = crate::tests::env_lock();
    let _scope = super::super::formation_budget::enter(None);
    let _wall = super::super::formation_budget::RequestWallScope::enter(
        Instant::now() + Duration::from_secs(5),
    );
    let club = Arc::new(AllocatedGraphClub {
        calls: AtomicUsize::new(0),
        walls: Mutex::new(Vec::new()),
    });
    let mut engine = engine(StubClub::shared("unused", &[]));
    engine.self_club = Some(club.clone());
    let mut graph = spec(
        "budget-complete",
        vec![
            node("planner", "plan", &[]),
            node("worker", "work", &["planner"]),
            node("fanin", "join", &["worker"]),
        ],
    );
    graph.token_budget = Some(60);
    let outcome = engine.run(&graph, "task", &AtomicBool::new(false)).unwrap();
    assert_eq!(club.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        outcome.episode.token_allocation.as_ref().unwrap()["paid_spent"],
        60
    );
    assert!(outcome.episode.incomplete_by_budget.is_empty());
    let walls = club.walls.lock().unwrap();
    assert_eq!(walls.len(), 3);
    assert!(walls.windows(2).all(|w| w[1] <= w[0]));
    assert!(walls[0] <= Duration::from_secs(5));
    assert!(
        outcome
            .episode
            .traces
            .iter()
            .all(|t| t.wall_remaining_ms.unwrap() <= 5000)
    );
    outcome.episode.validate().unwrap();
}

/// The shared batching advisory follows the grant: a tool-bearing node gets
/// it, a no-tool node is never told to batch calls it cannot make.
#[test]
fn node_prompt_batching_advisory_follows_the_grant() {
    let spec = node("worker", "do the thing", &[]);
    let tooled = node_system_prompt("g", &spec, "", Grant::ReadOnly);
    assert!(tooled.contains(TOOL_BATCHING_HINT), "{tooled}");
    assert!(
        !tooled.contains("code_mode"),
        "node grants vary, so the advisory stays tool-agnostic:\n{tooled}"
    );
    let bare = node_system_prompt("g", &spec, "", Grant::None);
    assert!(!bare.contains(TOOL_BATCHING_HINT), "{bare}");
    assert!(bare.contains("no tools this run"), "{bare}");
}

#[test]
fn checked_in_graph_persona_resolves_and_is_injected_into_real_node_prompt() {
    let _lock = crate::tests::env_lock();
    let empty_user = std::env::temp_dir().join(format!(
        "agent-graph-persona-user-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&empty_user).unwrap();
    let _user = crate::tests::TestEnvGuard::set("ANGEL_PERSONAS_DIR", empty_user.to_str().unwrap());
    let _bundled = crate::tests::TestEnvGuard::unset("ANGEL_BUNDLED_PERSONAS_DIR");
    struct GraphPersonaProbe {
        systems: Mutex<Vec<String>>,
    }
    impl Club for GraphPersonaProbe {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Err("graph persona probe must use structured chat".to_string())
        }
        fn label(&self) -> &str {
            "persona-graph"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            self.systems
                .lock()
                .unwrap()
                .push(messages.first().unwrap().content.to_string());
            Ok(ClubReply::Text("review complete".to_string()))
        }
    }
    let club = Arc::new(GraphPersonaProbe {
        systems: Mutex::new(Vec::new()),
    });
    let mut review = node("review", "audit {task}", &[]);
    review.persona = Some("reviewer".to_string());
    let workspace = std::env::temp_dir().join(format!(
        "agent-graph-persona-workspace-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&workspace).unwrap();
    let outcome = AgentGraphEngine::new(
        workspace,
        Some(Arc::clone(&club) as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence()
    .run(
        &spec("persona-graph", vec![review]),
        "the patch",
        &AtomicBool::new(false),
    )
    .expect("checked-in reviewer persona must resolve in graph execution");
    assert_eq!(outcome.answer, "review complete");
    let systems = club.systems.lock().unwrap();
    assert_eq!(systems.len(), 1);
    assert!(systems[0].contains("lead code reviewer"), "{}", systems[0]);
}

#[test]
fn validation_rejects_cycles_unknown_deps_and_cross_edge_placeholders() {
    let cyclic = spec(
        "loopy",
        vec![node("a", "x", &["b"]), node("b", "x", &["a"])],
    );
    let err = validate_graph_spec(&cyclic).unwrap_err();
    assert!(err.contains("cycle"), "{err}");

    let dangling = spec("dangling", vec![node("a", "x", &["ghost"])]);
    let err = validate_graph_spec(&dangling).unwrap_err();
    assert!(err.contains("unknown node 'ghost'"), "{err}");

    let leak = spec(
        "leak",
        vec![node("a", "x", &[]), node("b", "{output:a}", &[])],
    );
    let err = validate_graph_spec(&leak).unwrap_err();
    assert!(err.contains("does not depend_on"), "{err}");

    let mut gated = spec(
        "gate-target",
        vec![node("a", "x", &[]), node("check", "x", &["a"])],
    );
    gated.nodes[1].gate = Some(GraphGateSpec {
        verdict: true,
        expect: None,
        retry: Some("check".to_string()),
        credit_to: None,
        max_retries: 1,
    });
    let err = validate_graph_spec(&gated).unwrap_err();
    assert!(err.contains("ancestor"), "{err}");
}

#[test]
fn validation_rejects_unordered_code_nodes_even_inside_a_diamond() {
    let mut graph = spec(
        "code-diamond",
        vec![
            node("root", "inspect", &[]),
            node("left-write", "change left", &["root"]),
            node("right-write", "change right", &["root"]),
            node("join", "review", &["left-write", "right-write"]),
        ],
    );
    graph.nodes[0].tools = Some("read_only".to_string());
    graph.nodes[1].tools = Some("code".to_string());
    graph.nodes[2].tools = Some("code".to_string());
    graph.nodes[3].tools = Some("read_only".to_string());

    let error = validate_graph_spec(&graph).unwrap_err();
    assert!(error.contains("code nodes 'left-write' and 'right-write'"));
    assert!(error.contains("unordered"), "{error}");
    assert!(error.contains("depends_on"), "{error}");
    assert!(error.contains("worktree-isolated delegate"), "{error}");

    let club = StubClub::shared("must-not-run", &[]);
    let cancel = AtomicBool::new(false);
    let run_error = engine(Arc::clone(&club))
        .run(&graph, "task", &cancel)
        .unwrap_err();
    assert!(matches!(&run_error, GraphRunError::Preflight(_)));
    assert!(run_error.contains("unordered"), "{run_error}");
    assert!(
        club.prompts().is_empty(),
        "unsafe graph must fail before provider dispatch"
    );
}

#[test]
fn validation_allows_transitively_ordered_code_and_parallel_read_only_nodes() {
    // Reverse declaration order is deliberate: array position must not be
    // mistaken for dependency reachability.
    let mut graph = spec(
        "ordered-code",
        vec![
            node("finish-write", "finish", &["bridge"]),
            node("reader-left", "inspect left", &[]),
            node("bridge", "verify first write", &["start-write"]),
            node("reader-right", "inspect right", &[]),
            node("start-write", "start", &[]),
            node(
                "join",
                "review all",
                &["finish-write", "reader-left", "reader-right"],
            ),
        ],
    );
    graph.nodes[0].tools = Some("code".to_string());
    graph.nodes[1].tools = Some("read_only".to_string());
    graph.nodes[2].tools = Some("read_only".to_string());
    graph.nodes[3].tools = Some("read_only".to_string());
    graph.nodes[4].tools = Some("code".to_string());
    graph.nodes[5].tools = Some("read_only".to_string());

    validate_graph_spec(&graph)
        .expect("transitive depends_on orders code nodes while read-only branches remain parallel");
}

#[test]
fn validation_orders_retry_gates_with_every_affected_code_writer() {
    let gate = GraphGateSpec {
        verdict: true,
        expect: None,
        retry: Some("root".to_string()),
        credit_to: None,
        max_retries: 1,
    };

    let mut unsafe_sibling = spec(
        "retry-writer-race",
        vec![
            node("root", "draft", &[]),
            node("writer", "mutate", &["root"]),
            node("review", "judge", &["root"]),
        ],
    );
    unsafe_sibling.nodes[1].tools = Some("code".to_string());
    unsafe_sibling.nodes[2].tools = Some("read_only".to_string());
    unsafe_sibling.nodes[2].gate = Some(gate.clone());
    let error = validate_graph_spec(&unsafe_sibling).unwrap_err();
    assert!(error.contains("retry gate 'review'"), "{error}");
    assert!(error.contains("code node 'writer'"), "{error}");
    assert!(error.contains("unordered"), "{error}");

    let mut writer_before_gate = unsafe_sibling.clone();
    writer_before_gate.nodes[2].depends_on = vec!["writer".to_string()];
    validate_graph_spec(&writer_before_gate)
        .expect("a writer that lands before the retry gate cannot leak an in-flight write");

    let mut writer_after_gate = spec(
        "retry-writer-after",
        vec![
            node("root", "draft", &[]),
            node("review", "judge", &["root"]),
            node("writer", "mutate", &["review"]),
        ],
    );
    writer_after_gate.nodes[1].tools = Some("read_only".to_string());
    writer_after_gate.nodes[1].gate = Some(gate);
    writer_after_gate.nodes[2].tools = Some("code".to_string());
    validate_graph_spec(&writer_after_gate)
        .expect("a downstream writer cannot start until the retry gate passes");
}

#[test]
fn validation_rejects_ambiguous_or_incompatible_role_pools() {
    let mut unbound = spec("unbound", vec![node("a", "x", &[])]);
    unbound.nodes[0].pool_max_inflight = Some(2);
    let err = validate_graph_spec(&unbound).unwrap_err();
    assert!(err.contains("without a pool"), "{err}");

    let mut empty = spec("empty", vec![node("a", "x", &[])]);
    empty.nodes[0].pool = Some("  ".to_string());
    let err = validate_graph_spec(&empty).unwrap_err();
    assert!(err.contains("empty pool name"), "{err}");

    let mut zero = spec("zero", vec![node("a", "x", &[])]);
    zero.nodes[0].pool = Some("research".to_string());
    zero.nodes[0].pool_max_inflight = Some(0);
    let err = validate_graph_spec(&zero).unwrap_err();
    assert!(err.contains("must be 1..=64"), "{err}");

    let mut conflict = spec("conflict", vec![node("a", "x", &[]), node("b", "x", &[])]);
    for pooled in &mut conflict.nodes {
        pooled.pool = Some("research".to_string());
    }
    conflict.nodes[0].pool_max_inflight = Some(1);
    conflict.nodes[1].pool_max_inflight = Some(2);
    let err = validate_graph_spec(&conflict).unwrap_err();
    assert!(err.contains("conflicting max_inflight"), "{err}");

    conflict.nodes[1].pool_max_inflight = Some(1);
    conflict.nodes[1].persona = Some("reviewer".to_string());
    let err = validate_graph_spec(&conflict).unwrap_err();
    assert!(err.contains("role identity differs"), "{err}");
}

#[test]
fn role_pool_capacity_serializes_leased_tasks() {
    let _guard = crate::tests::env_lock();
    let _seats = crate::tests::TestEnvGuard::set("ANGEL_GRAPH_MAX_SEATS", "3");
    let _inflight = crate::tests::TestEnvGuard::set("ANGEL_SPAWN_INFLIGHT_MAX", "64");
    let club = TrackingClub::shared();
    let mut graph = spec(
        "pooled",
        vec![
            node("research-a", "a {task}", &[]),
            node("research-b", "b {task}", &[]),
            node("research-c", "c {task}", &[]),
        ],
    );
    for worker in &mut graph.nodes {
        worker.pool = Some("research".to_string());
        worker.pool_max_inflight = Some(1);
    }
    let workspace = std::env::temp_dir().join(format!(
        "agent-graph-pool-test-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&workspace).expect("test workspace");
    let cancel = AtomicBool::new(false);
    let outcome = AgentGraphEngine::new(
        workspace.clone(),
        Some(Arc::clone(&club) as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence()
    .run(&graph, "task", &cancel)
    .expect("pooled graph completes");
    assert_eq!(club.max_active.load(Ordering::SeqCst), 1);
    assert_eq!(
        outcome
            .snapshot
            .nodes
            .iter()
            .map(|node| node.lease_id)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2), Some(3)]
    );
    assert!(
        outcome
            .snapshot
            .events
            .iter()
            .any(|event| { event.contains("pool:research lease#1 cap1") })
    );
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn bundled_graph_files_validate() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("graphs");
    let mut seen = 0usize;
    for entry in std::fs::read_dir(&dir)
        .expect("bundled graphs dir exists")
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let loaded = load_graph_file(&path);
        assert!(
            loaded.error.is_none(),
            "{} is broken: {:?}",
            path.display(),
            loaded.error
        );
        seen += 1;
    }
    assert!(
        seen >= 2,
        "expected the bundled starter graphs, found {seen}"
    );
}

#[test]
fn bundled_spec_shape_parses() {
    let toml_body = r#"
name = "fixture"
description = "shape check"

[[node]]
id = "a"
prompt = "{task}"

[[node]]
id = "b"
depends_on = ["a"]
prompt = "improve: {output:a}"
gate = { verdict = true, retry = "a", max_retries = 2 }
"#;
    let parsed: GraphSpec = toml::from_str(toml_body).expect("parses");
    validate_graph_spec(&parsed).expect("validates");
    assert_eq!(parsed.nodes[1].gate.as_ref().unwrap().max_retries, 2);
}

#[test]
fn linear_graph_flows_state_downstream() {
    let club = StubClub::shared("stub", &["alpha-notes", "final-report"]);
    let graph = spec(
        "linear",
        vec![
            node("research", "Research: {task}", &[]),
            node(
                "write",
                "Write from notes: {output:research}",
                &["research"],
            ),
        ],
    );
    let cancel = AtomicBool::new(false);
    let outcome = engine(Arc::clone(&club))
        .run(&graph, "the task", &cancel)
        .unwrap();
    assert_eq!(outcome.answer, "final-report");
    let prompts = club.prompts();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("the task"), "{}", prompts[0]);
    assert!(prompts[1].contains("alpha-notes"), "{}", prompts[1]);
    assert_eq!(outcome.snapshot.phase, GraphRunPhase::Done);
}

#[test]
fn completed_run_persists_auditable_episode_sidecar() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "agent-graph-episode-persist-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let _episode_dir = crate::tests::TestEnvGuard::set(
        "ANGEL_AGENT_GRAPH_DIR",
        root.to_str().expect("UTF-8 test root"),
    );
    let club = StubClub::shared("stub", &["answer"]);
    let graph = spec("persisted", vec![node("solver", "{task}", &[])]);
    let cancel = AtomicBool::new(false);
    let outcome = AgentGraphEngine::new(workspace.clone(), Some(club as Arc<dyn Club>), Vec::new())
        .run(&graph, "task", &cancel)
        .expect("graph completes");
    assert_eq!(
        outcome.episode_persistence,
        GraphEpisodePersistence::Persisted
    );
    assert!(
        outcome
            .snapshot
            .events
            .iter()
            .any(|event| event.contains("receipt persisted"))
    );
    let path = root
        .join(crate::workspace_store::workspace_key(&workspace))
        .join("episodes")
        .join(format!("{}.json", outcome.episode.episode_id));
    let workspace_record_dir = path
        .parent()
        .and_then(Path::parent)
        .expect("workspace record directory");
    assert!(
        std::fs::read_dir(workspace_record_dir)
            .expect("workspace record directory is readable")
            .flatten()
            .all(|entry| entry.file_name() == "episodes"),
        "body-bearing legacy run sidecars must not be written"
    );
    let body = std::fs::read_to_string(&path).expect("episode sidecar");
    let persisted: GraphEpisodeV1 = serde_json::from_str(&body).expect("episode schema");
    persisted.validate().expect("persisted receipt audits");
    assert_eq!(persisted.receipt_sha256, outcome.episode.receipt_sha256);
    #[cfg(unix)]
    {
        assert_eq!(
            std::fs::metadata(&path)
                .expect("episode metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().expect("episode directory"))
                .expect("episode directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    persist_episode_receipt(&workspace, &outcome.episode)
        .expect("identical receipt publication is idempotent");
    std::fs::write(&path, "tampered").expect("tamper fixture");
    let collision = persist_episode_receipt(&workspace, &outcome.episode).unwrap_err();
    assert!(collision.contains("refusing overwrite"), "{collision}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("collision leaves existing bytes"),
        "tampered"
    );
    #[cfg(unix)]
    {
        std::fs::remove_file(&path).expect("remove collision fixture");
        let symlink_target = root.join("episode-symlink-target");
        std::fs::write(&symlink_target, "do not follow").expect("symlink target fixture");
        std::os::unix::fs::symlink(&symlink_target, &path).expect("episode symlink fixture");
        let error = persist_episode_receipt(&workspace, &outcome.episode).unwrap_err();
        assert!(error.contains("not a regular file"), "{error}");
        assert_eq!(
            std::fs::read_to_string(&symlink_target).expect("symlink target unchanged"),
            "do not follow"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn episode_persistence_failure_speaks_without_hiding_the_answer() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "agent-graph-episode-failure-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let blocked = root.join("not-a-directory");
    std::fs::write(&blocked, "occupied").expect("blocking file");
    let _episode_dir = crate::tests::TestEnvGuard::set(
        "ANGEL_AGENT_GRAPH_DIR",
        blocked.to_str().expect("UTF-8 test root"),
    );
    let club = StubClub::shared("stub", &["answer"]);
    let graph = spec("persistence-failure", vec![node("solver", "{task}", &[])]);
    let cancel = AtomicBool::new(false);
    let outcome = AgentGraphEngine::new(workspace, Some(club as Arc<dyn Club>), Vec::new())
        .run(&graph, "task", &cancel)
        .expect("model result remains available");
    assert_eq!(outcome.answer, "answer");
    match &outcome.episode_persistence {
        GraphEpisodePersistence::Failed(error) => {
            assert!(
                error.contains("create episode receipt directory"),
                "{error}"
            )
        }
        other => panic!("expected a speaking persistence failure, got {other:?}"),
    }
    assert!(
        outcome
            .snapshot
            .events
            .iter()
            .any(|event| event.contains("episode receipt persistence failed"))
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fan_in_joins_parallel_branches() {
    let _guard = crate::tests::env_lock();
    let club = StubClub::shared("stub", &["alpha", "beta", "gamma"]);
    let graph = spec(
        "fan",
        vec![
            node("left", "L: {task}", &[]),
            node("right", "R: {task}", &[]),
            node("join", "Fold the branches. {task}", &["left", "right"]),
        ],
    );
    let cancel = AtomicBool::new(false);
    let outcome = engine(Arc::clone(&club)).run(&graph, "t", &cancel).unwrap();
    assert_eq!(outcome.answer, "gamma");
    let join = outcome
        .episode
        .traces
        .iter()
        .find(|t| t.role == "join")
        .unwrap();
    let ready = join.inputs_ready_at.unwrap();
    assert!(join.started_at.unwrap().monotonic_ns >= ready.monotonic_ns);
    let latest = outcome
        .episode
        .traces
        .iter()
        .filter(|t| t.role != "join")
        .map(|t| t.finished_at.unwrap().monotonic_ns)
        .max()
        .unwrap();
    assert_eq!(ready.monotonic_ns, latest);
    for trace in &outcome.episode.traces {
        assert!(trace.finished_at.unwrap().monotonic_ns >= trace.started_at.unwrap().monotonic_ns);
        assert!(trace.started_at.unwrap().utc_ms > 0);
        assert!(trace.duration_ms.unwrap() >= 0.0);
    }

    let join_prompt = club.prompts().last().cloned().unwrap();
    assert!(join_prompt.contains("alpha"), "{join_prompt}");
    assert!(join_prompt.contains("beta"), "{join_prompt}");
    assert!(join_prompt.contains("Upstream results"), "{join_prompt}");
}

#[test]
fn fan_in_budget_is_total_fair_and_receipt_typed() {
    let join = node(
        "join",
        "A1: {output:a}\nA2: {output:a}\nB: {output:b}",
        &["a", "b"],
    );
    let outputs = HashMap::from([
        ("a".to_string(), "A".repeat(100)),
        ("b".to_string(), "B".repeat(100)),
    ]);
    let prompt = render_node_prompt(&join, "task", &outputs, &HashMap::new(), 10);
    assert_eq!(prompt.matches("angel-agent-graph-result/v1").count(), 3);
    assert_eq!(prompt.matches("source=\"a\"").count(), 2);
    assert_eq!(
        prompt.matches("excerpt_chars=2 omitted_chars=98").count(),
        2
    );
    assert_eq!(
        prompt.matches("excerpt_chars=5 omitted_chars=95").count(),
        1
    );
    assert!(!prompt.contains("AAA"), "{prompt}");
    assert!(!prompt.contains("BBBBBB"), "{prompt}");
    assert!(prompt.contains("…[result body truncated]"), "{prompt}");
}

#[test]
fn graph_template_expands_only_authored_placeholders() {
    let join = node(
        "join",
        "Task: {task}\nA: {output:a}\nB: {output:b}",
        &["a", "b"],
    );
    let outputs = HashMap::from([
        ("a".to_string(), "upstream literal {output:b}".to_string()),
        ("b".to_string(), "beta".to_string()),
    ]);
    let prompt = render_node_prompt(
        &join,
        "task literal {output:b}",
        &outputs,
        &HashMap::new(),
        100,
    );
    assert_eq!(prompt.matches("source=\"b\"").count(), 1, "{prompt}");
    assert_eq!(prompt.matches("{output:b}").count(), 2, "{prompt}");
    assert!(prompt.contains("upstream literal {output:b}"), "{prompt}");
    assert!(prompt.contains("task literal {output:b}"), "{prompt}");
}

#[test]
fn gate_fail_loops_back_with_feedback_then_passes() {
    let club = StubClub::shared(
        "stub",
        &[
            "draft one",
            "VERDICT: FAIL — too vague",
            "draft two",
            "VERDICT: PASS",
        ],
    );
    let mut graph = spec(
        "reviewed",
        vec![
            node("write", "Draft it: {task}", &[]),
            node("review", "Review: {output:write}", &["write"]),
        ],
    );
    graph.nodes[0].trainable = true;
    graph.nodes[1].gate = Some(GraphGateSpec {
        verdict: true,
        expect: None,
        retry: Some("write".to_string()),
        credit_to: None,
        max_retries: 2,
    });
    let cancel = AtomicBool::new(false);
    let outcome = engine(Arc::clone(&club)).run(&graph, "t", &cancel).unwrap();
    assert_eq!(outcome.snapshot.phase, GraphRunPhase::Done);
    let prompts = club.prompts();
    assert_eq!(prompts.len(), 4, "write, review, write retry, review retry");
    assert!(
        prompts[2].contains("too vague"),
        "retry carries reviewer feedback: {}",
        prompts[2]
    );
    assert!(prompts[3].contains("draft two"), "{}", prompts[3]);
    let review = outcome
        .snapshot
        .nodes
        .iter()
        .find(|n| n.id == "review")
        .unwrap();
    assert_eq!(review.retries, 1);
    assert_eq!(review.gate.as_deref(), Some("PASS"));
    // The final answer is the gate sink's PASS verdict output.
    assert!(outcome.answer.contains("VERDICT: PASS"));
    outcome.episode.validate().expect("episode receipt audits");
    assert_eq!(outcome.episode.schema, GRAPH_EPISODE_SCHEMA);
    assert_eq!(outcome.episode.traces.len(), 4);
    assert_eq!(outcome.episode.eligible_trace_count, 0);
    let writes = outcome
        .episode
        .traces
        .iter()
        .filter(|trace| trace.role == "write")
        .collect::<Vec<_>>();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].attempt_index, 0);
    assert_eq!(writes[1].attempt_index, 1);
    assert_eq!(
        writes[1].retry_of_trace_id.as_deref(),
        Some(writes[0].trace_id.as_str())
    );
    assert_eq!(writes[0].control_credit[0].outcome, "rejected");
    assert_eq!(writes[1].control_credit[0].outcome, "accepted");
    assert!(
        writes
            .iter()
            .all(|trace| trace.reward.is_none() && trace.reward_source.is_none())
    );
    assert!(writes.iter().all(|trace| {
        trace.requested_route_config.output_budget_policy == "provider_native"
            && trace.requested_route_config.max_output_tokens.is_none()
            && trace.requested_route_config.sampling_policy == "provider_or_route_default"
            && trace.requested_route_config.route == trace.requested_route
            && trace.resolved_route_config.route == trace.resolved_route
    }));
    assert!(writes.iter().all(|trace| {
        !trace.eligibility.training_eligible
            && trace
                .eligibility
                .reasons
                .iter()
                .any(|reason| reason == "external_reward_missing")
    }));

    let mut tampered = outcome.episode.clone();
    tampered.traces[0]
        .output
        .as_mut()
        .expect("writer output")
        .sha256 = "0".repeat(64);
    assert!(tampered.validate().is_err());

    let mut route_tampered = outcome.episode.clone();
    route_tampered.traces[0]
        .requested_route_config
        .sampling_policy = "invented".to_string();
    assert!(route_tampered.validate().is_err());
}

#[test]
fn gate_retry_discards_stale_sibling_generation_before_fan_in() {
    let _guard = crate::tests::env_lock();
    let _seats = crate::tests::TestEnvGuard::set("ANGEL_GRAPH_MAX_SEATS", "3");
    let _inflight = crate::tests::TestEnvGuard::set("ANGEL_SPAWN_INFLIGHT_MAX", "64");
    let club = RetryEpochClub::shared();
    let mut graph = spec(
        "causal-retry",
        vec![
            node("a", "A: {task}", &[]),
            node("b", "B: {output:a}", &["a"]),
            node("c", "C: {output:a}", &["a"]),
            node("review", "Review: {output:b}", &["b"]),
            node(
                "sink",
                "Join review={output:review} c={output:c}",
                &["review", "c"],
            ),
        ],
    );
    graph.nodes[3].gate = Some(GraphGateSpec {
        verdict: true,
        expect: None,
        retry: Some("a".to_string()),
        credit_to: None,
        max_retries: 2,
    });
    let workspace = std::env::temp_dir().join(format!(
        "agent-graph-retry-epoch-test-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&workspace).expect("test workspace");
    let cancel = AtomicBool::new(false);
    let outcome = AgentGraphEngine::new(
        workspace.clone(),
        Some(Arc::clone(&club) as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence()
    .run(&graph, "task", &cancel)
    .expect("fresh causal generation reaches the sink");

    assert_eq!(outcome.answer, "sink-complete");
    assert_eq!(club.a_calls.load(Ordering::SeqCst), 2);
    assert_eq!(club.b_calls.load(Ordering::SeqCst), 2);
    assert_eq!(club.c_calls.load(Ordering::SeqCst), 2);
    assert_eq!(club.review_calls.load(Ordering::SeqCst), 2);
    assert_eq!(club.sink_calls.load(Ordering::SeqCst), 1);
    let sink_prompt = club
        .sink_prompt
        .lock()
        .unwrap()
        .clone()
        .expect("sink prompt captured");
    assert!(sink_prompt.contains("c-fresh-from-a2"), "{sink_prompt}");
    assert!(!sink_prompt.contains("c-stale-from-a1"), "{sink_prompt}");
    assert!(
        outcome
            .snapshot
            .events
            .iter()
            .any(|event| { event.contains("c discarded stale generation 0 (current 1)") })
    );

    let stale_c = outcome
        .episode
        .traces
        .iter()
        .find(|trace| trace.role == "c" && trace.attempt_index == 0)
        .expect("stale C trace retained for audit");
    let fresh_c = outcome
        .episode
        .traces
        .iter()
        .find(|trace| trace.role == "c" && trace.attempt_index == 1)
        .expect("fresh C trace retained for fan-in");
    let sink = outcome
        .episode
        .traces
        .iter()
        .find(|trace| trace.role == "sink")
        .expect("sink trace");
    assert!(sink.parent_trace_ids.contains(&fresh_c.trace_id));
    assert!(!sink.parent_trace_ids.contains(&stale_c.trace_id));
    outcome.episode.validate().expect("episode receipt audits");
    assert!(!cancel.load(Ordering::SeqCst));
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn graph_trace_receipts_are_digest_only_even_without_secret_markers() {
    let benign = GraphTextArtifactV1::capture("ordinary proprietary prompt text");
    assert!(!benign.secret_rejected);
    assert!(benign.preview.is_none());
    assert!(benign.preview_truncated);
    benign
        .validate()
        .expect("digest-only benign artifact audits");

    let artifact = GraphTextArtifactV1::capture("authorization: Bearer very-secret-token");
    assert!(artifact.secret_rejected);
    assert!(artifact.preview.is_none());
    artifact
        .validate()
        .expect("digest-only secret artifact audits");
}

#[test]
fn graph_initial_wave_that_does_not_fit_launches_zero_nodes() {
    let _budget_scope = DescendantBudgetScope::for_test(1);
    let budget = current_descendant_budget().unwrap();
    let club = StubClub::shared("stub", &["must-not-launch", "must-not-launch"]);
    let graph = spec(
        "over-budget",
        vec![node("a", "a", &[]), node("b", "b", &[])],
    );
    let cancel = AtomicBool::new(false);
    let error = engine(Arc::clone(&club))
        .run(&graph, "task", &cancel)
        .unwrap_err();

    assert!(error.contains("agent graph initial wave"), "{error}");
    assert!(error.contains("zero calls admitted"), "{error}");
    assert!(club.prompts().is_empty(), "rejected graph launched a node");
    assert_eq!(descendant_budget_status(&budget).unwrap().spent, 0);
    assert!(!cancel.load(Ordering::SeqCst));
}

#[test]
fn graph_retry_wave_shares_root_budget_and_fails_before_dispatch() {
    let _budget_scope = DescendantBudgetScope::for_test(2);
    let budget = current_descendant_budget().unwrap();
    let club = StubClub::shared("stub", &["draft", "VERDICT: FAIL — revise"]);
    let mut graph = spec(
        "retry-over-budget",
        vec![
            node("write", "Draft: {task}", &[]),
            node("review", "Review: {output:write}", &["write"]),
        ],
    );
    graph.nodes[1].gate = Some(GraphGateSpec {
        verdict: true,
        expect: None,
        retry: Some("write".to_string()),
        credit_to: None,
        max_retries: 1,
    });
    let cancel = AtomicBool::new(false);
    let error = engine(Arc::clone(&club))
        .run(&graph, "task", &cancel)
        .unwrap_err();

    assert!(error.contains("gate 'review' retry denied"), "{error}");
    assert!(
        error.contains("requested=2 total=2 spent=2 remaining=0"),
        "{error}"
    );
    assert!(error.contains("zero calls admitted"), "{error}");
    assert_eq!(club.prompts().len(), 2, "denied retry launched a node");
    assert_eq!(descendant_budget_status(&budget).unwrap().spent, 2);
    assert!(
        !cancel.load(Ordering::SeqCst),
        "budget exhaustion must not poison caller cancellation authority"
    );
}

#[test]
fn graph_retry_receipt_reconciles_every_started_attempt() {
    let _budget_scope = DescendantBudgetScope::for_test(4);
    let budget = current_descendant_budget().unwrap();
    let club = StubClub::shared(
        "stub",
        &[
            "draft-one",
            "VERDICT: FAIL — revise",
            "draft-two",
            "VERDICT: PASS",
        ],
    );
    let mut graph = spec(
        "retry-fits",
        vec![
            node("write", "Draft: {task}", &[]),
            node("review", "Review: {output:write}", &["write"]),
        ],
    );
    graph.nodes[1].gate = Some(GraphGateSpec {
        verdict: true,
        expect: None,
        retry: Some("write".to_string()),
        credit_to: None,
        max_retries: 1,
    });
    let cancel = AtomicBool::new(false);
    let outcome = engine(Arc::clone(&club))
        .run(&graph, "task", &cancel)
        .unwrap();

    assert_eq!(club.prompts().len(), 4);
    assert_eq!(descendant_budget_status(&budget).unwrap().spent, 4);
    assert!(
        outcome
            .snapshot
            .events
            .iter()
            .any(|event| { event.contains("descendant_calls=4/4 remaining=0 admitted=2") })
    );
    assert!(!cancel.load(Ordering::SeqCst));
}

#[test]
fn gate_exhaustion_fails_loudly() {
    let club = StubClub::shared(
        "stub",
        &["d1", "VERDICT: FAIL — no", "d2", "VERDICT: FAIL — still no"],
    );
    let mut graph = spec(
        "strict",
        vec![
            node("write", "Draft: {task}", &[]),
            node("review", "Review: {output:write}", &["write"]),
        ],
    );
    graph.nodes[1].gate = Some(GraphGateSpec {
        verdict: true,
        expect: None,
        retry: Some("write".to_string()),
        credit_to: None,
        max_retries: 1,
    });
    let cancel = AtomicBool::new(false);
    let err = engine(club).run(&graph, "t", &cancel).unwrap_err();
    assert!(err.contains("gate 'review'"), "{err}");
    assert!(err.contains("still no"), "{err}");
    let failure = err
        .run_failure()
        .expect("executed gate failure retains its sealed receipt");
    assert_eq!(failure.snapshot.phase, GraphRunPhase::Failed);
    assert_eq!(
        failure.episode.termination.kind,
        GraphEpisodeTerminationKind::Failed
    );
    assert_eq!(
        failure.episode_persistence,
        GraphEpisodePersistence::Disabled
    );
    assert!(is_sha256(&failure.episode.episode_id));
    assert!(
        !cancel.load(Ordering::SeqCst),
        "gate rejection must not poison the caller's cancellation authority"
    );
}

#[test]
fn failed_graph_tool_surfaces_the_persisted_episode_receipt() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "agent-graph-failure-tool-receipt-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = root.join("workspace");
    let graphs = root.join("graphs");
    let bundled = root.join("bundled-empty");
    let receipts = root.join("receipts");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&graphs).expect("graph catalog");
    std::fs::create_dir_all(&bundled).expect("empty bundled graph catalog");
    std::fs::write(
        graphs.join("strict-receipt.toml"),
        r#"
name = "strict-receipt"

[[node]]
id = "write"
prompt = "Draft: {task}"

[[node]]
id = "review"
depends_on = ["write"]
prompt = "Review: {output:write}"
gate = { verdict = true, retry = "write", max_retries = 1 }
"#,
    )
    .expect("graph fixture");
    let _graphs = crate::tests::TestEnvGuard::set(
        "ANGEL_GRAPHS_DIR",
        graphs.to_str().expect("UTF-8 graphs path"),
    );
    let _bundled = crate::tests::TestEnvGuard::set(
        "ANGEL_BUNDLED_GRAPHS_DIR",
        bundled.to_str().expect("UTF-8 bundled path"),
    );
    let _receipts = crate::tests::TestEnvGuard::set(
        "ANGEL_AGENT_GRAPH_DIR",
        receipts.to_str().expect("UTF-8 receipt path"),
    );
    let club = StubClub::shared(
        "stub",
        &["d1", "VERDICT: FAIL — no", "d2", "VERDICT: FAIL — still no"],
    );
    let club_dyn = Arc::clone(&club) as Arc<dyn Club>;
    let tool = AgentGraphTool::new(workspace.clone(), Some(club_dyn), Vec::new());
    let error = tool
        .call(&serde_json::json!({"graph": "strict-receipt", "task": "task"}))
        .expect_err("gate exhaustion fails the graph tool");

    let header = error.lines().next().expect("bounded receipt header");
    assert!(header.contains("phase=Failed"), "{header}");
    assert!(header.contains("receipt=persisted"), "{header}");
    assert!(header.contains("nodes=1/2"), "{header}");
    assert!(!header.contains("still no"), "raw failure leaked: {header}");
    let episode_id = header
        .split_whitespace()
        .find_map(|field| field.strip_prefix("episode="))
        .expect("episode field");
    assert!(is_sha256(episode_id), "{episode_id}");
    assert!(error.contains("still no"), "{error}");
    assert!(error.chars().count() < 2_000, "failure payload unbounded");

    let path = receipts
        .join(crate::workspace_store::workspace_key(&workspace))
        .join("episodes")
        .join(format!("{episode_id}.json"));
    let persisted: GraphEpisodeV1 =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("failed episode persisted"))
            .expect("failed episode schema");
    persisted.validate().expect("failed receipt audits");
    assert_eq!(
        persisted.termination.kind,
        GraphEpisodeTerminationKind::Failed
    );
    assert_eq!(persisted.termination.phase, GraphRunPhase::Failed);
    assert_eq!(persisted.traces.len(), 4);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn operator_cancel_is_attributed_in_the_sealed_node_receipt() {
    let _guard = crate::tests::env_lock();
    let club = WindDownAttributionClub::shared();
    let graph = spec("operator-cancel", vec![node("worker", "work", &[])]);
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let worker_club = Arc::clone(&club);
    // Toolchain identity capture is deliberately an engine-construction
    // preflight. Keep it outside the node-start deadline: this test is
    // about cancellation of a running seat, not cold rustup/disk latency.
    let engine = AgentGraphEngine::new(
        std::env::temp_dir(),
        Some(worker_club as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence();
    let worker = std::thread::spawn(move || engine.run(&graph, "task", &worker_cancel));
    let deadline = Instant::now() + Duration::from_secs(2);
    while !club.started.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline, "graph node never started");
        std::thread::sleep(Duration::from_millis(1));
    }
    cancel.store(true, Ordering::Release);
    let error = worker.join().expect("graph worker joins").unwrap_err();
    let failure = error.run_failure().expect("sealed cancellation receipt");

    assert_eq!(failure.snapshot.phase, GraphRunPhase::Cancelled);
    assert_eq!(
        failure.episode.termination.kind,
        GraphEpisodeTerminationKind::OperatorCancelled
    );
    assert_eq!(failure.episode.traces.len(), 1);
    let trace = &failure.episode.traces[0];
    assert_eq!(trace.role, "worker");
    assert_eq!(trace.termination.kind, GraphTraceTerminationKind::Interrupt);
    assert_eq!(trace.termination.stop_reason, "cancelled by operator");
    assert!(
        trace.cancel_requested_at.unwrap().monotonic_ns >= trace.started_at.unwrap().monotonic_ns
    );
    assert!(
        trace.stopped_at.unwrap().monotonic_ns >= trace.cancel_requested_at.unwrap().monotonic_ns
    );
    assert_eq!(trace.stopped_at, trace.finished_at);

    let cancel_digest = crate::cut::sha256_hex(b"cancelled by operator");
    assert_eq!(
        trace.termination.detail_sha256.as_deref(),
        Some(cancel_digest.as_str())
    );
    failure.episode.validate().expect("cancel receipt audits");
}

#[test]
fn graph_deadline_is_attributed_in_the_sealed_node_receipt() {
    let club = WindDownAttributionClub::shared();
    let mut graph = spec("deadline", vec![node("worker", "work", &[])]);
    graph.deadline_secs = Some(1);
    let cancel = AtomicBool::new(false);
    let error = AgentGraphEngine::new(
        std::env::temp_dir(),
        Some(club as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence()
    .run(&graph, "task", &cancel)
    .unwrap_err();
    let failure = error.run_failure().expect("sealed deadline receipt");

    assert_eq!(failure.snapshot.phase, GraphRunPhase::Failed);
    assert_eq!(
        failure.episode.termination.kind,
        GraphEpisodeTerminationKind::Deadline
    );
    assert_eq!(failure.episode.traces.len(), 1);
    let trace = &failure.episode.traces[0];
    assert_eq!(trace.termination.kind, GraphTraceTerminationKind::Deadline);
    assert!(trace.termination.deadline_reached);
    assert!(
        trace
            .termination
            .stop_reason
            .contains("deadline 1s reached")
    );
    failure.episode.validate().expect("deadline receipt audits");
}

#[test]
fn failed_graph_receipt_bounds_raw_provider_errors_but_keeps_their_digest() {
    struct HugeFailureClub;
    impl Club for HugeFailureClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Err(format!("provider-marker:{}", "x".repeat(12_000)))
        }

        fn label(&self) -> &str {
            "huge-failure"
        }
    }

    let raw = format!("provider-marker:{}", "x".repeat(12_000));
    let graph = spec("bounded-failure", vec![node("worker", "work", &[])]);
    let cancel = AtomicBool::new(false);
    let error = AgentGraphEngine::new(
        std::env::temp_dir(),
        Some(Arc::new(HugeFailureClub) as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence()
    .run(&graph, "task", &cancel)
    .unwrap_err();
    let failure = error.run_failure().expect("sealed provider failure");
    let trace = &failure.episode.traces[0];
    let expected_digest = crate::cut::sha256_hex(raw.as_bytes());

    assert_eq!(
        trace.termination.detail_sha256.as_deref(),
        Some(expected_digest.as_str())
    );
    assert!(trace.termination.stop_reason.chars().count() <= GRAPH_TRACE_STOP_REASON_CAP);
    assert!(
        failure
            .snapshot
            .error
            .as_deref()
            .unwrap_or_default()
            .chars()
            .count()
            <= GRAPH_FAILURE_REASON_CAP
    );
    assert!(
        failure
            .snapshot
            .events
            .iter()
            .all(|event| event.chars().count() <= GRAPH_EVENT_TEXT_CAP + 16)
    );
    let rendered = render_graph_run_failure(&graph, failure, Duration::ZERO);
    assert!(rendered.chars().count() < 1_700, "payload was not bounded");
    assert!(
        !rendered
            .lines()
            .next()
            .unwrap_or_default()
            .contains("provider-marker"),
        "raw provider body leaked into receipt header"
    );
    failure.episode.validate().expect("bounded receipt audits");
}

#[test]
fn failure_header_marks_episode_persistence_failure_fail_closed() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "agent-graph-persist-failure-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = root.join("workspace");
    let blocked_root = root.join("not-a-directory");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::write(&blocked_root, "blocks receipt directory").expect("blocking file");
    let _receipts = crate::tests::TestEnvGuard::set(
        "ANGEL_AGENT_GRAPH_DIR",
        blocked_root.to_str().expect("UTF-8 blocked path"),
    );
    let club = FailureCancellationClub::failing("worker");
    let graph = spec("persist-failure", vec![node("worker", "work", &[])]);
    let cancel = AtomicBool::new(false);
    let error = AgentGraphEngine::new(workspace, Some(club as Arc<dyn Club>), Vec::new())
        .run(&graph, "task", &cancel)
        .unwrap_err();
    let failure = error.run_failure().expect("sealed failure is retained");
    assert!(matches!(
        &failure.episode_persistence,
        GraphEpisodePersistence::Failed(_)
    ));
    let rendered = render_graph_run_failure(&graph, failure, Duration::ZERO);
    let header = rendered.lines().next().expect("failure header");
    assert!(header.contains("receipt=failed"), "{header}");
    assert!(!header.contains("not-a-directory"), "path leaked: {header}");
    assert!(
        rendered.contains("episode persistence failed"),
        "{rendered}"
    );
    assert!(rendered.chars().count() < 2_000, "{rendered}");
    failure
        .episode
        .validate()
        .expect("in-memory receipt audits");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn internal_graph_failure_cancels_seats_not_caller() {
    let _guard = crate::tests::env_lock();
    let _seats = crate::tests::TestEnvGuard::set("ANGEL_GRAPH_MAX_SEATS", "2");
    let _inflight = crate::tests::TestEnvGuard::set("ANGEL_SPAWN_INFLIGHT_MAX", "64");
    let club = FailureCancellationClub::failing("first");
    let graph = spec(
        "failure-wind-down",
        vec![node("first", "first", &[]), node("sibling", "sibling", &[])],
    );
    let workspace = std::env::temp_dir().join(format!(
        "agent-graph-cancel-test-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&workspace).expect("test workspace");
    let cancel = AtomicBool::new(false);
    let err = AgentGraphEngine::new(
        workspace.clone(),
        Some(Arc::clone(&club) as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence()
    .run(&graph, "task", &cancel)
    .unwrap_err();

    assert!(err.contains("forced graph node failure"), "{err}");
    let failure = err.run_failure().expect("sealed node-failure receipt");
    let failed = failure
        .episode
        .traces
        .iter()
        .find(|trace| trace.role == "first")
        .expect("causal failing node trace");
    let sibling = failure
        .episode
        .traces
        .iter()
        .find(|trace| trace.role == "sibling")
        .expect("cancelled sibling trace");
    assert_eq!(
        failed.termination.kind,
        GraphTraceTerminationKind::ProviderFailure
    );
    assert_eq!(
        sibling.termination.kind,
        GraphTraceTerminationKind::Abandoned
    );
    assert!(
        sibling
            .termination
            .stop_reason
            .contains("node 'first' failed"),
        "{:?}",
        sibling.termination
    );
    failure.episode.validate().expect("failure receipt audits");
    assert!(
        club.sibling_saw_cancel.load(Ordering::SeqCst),
        "the in-flight sibling must observe the graph-local cancellation flag"
    );
    assert!(
        !cancel.load(Ordering::SeqCst),
        "internal graph failure must leave the caller cancellation flag false"
    );
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn concurrent_nodes_isolate_reasoning_effort_on_one_shared_club() {
    let _guard = crate::tests::env_lock();
    let _seats = crate::tests::TestEnvGuard::set("ANGEL_GRAPH_MAX_SEATS", "2");
    let _inflight = crate::tests::TestEnvGuard::set("ANGEL_SPAWN_INFLIGHT_MAX", "64");
    let club = ConcurrentEffortClub::shared();
    let mut graph = spec(
        "effort-isolation",
        vec![node("low", "low lane", &[]), node("high", "high lane", &[])],
    );
    graph.nodes[0].effort = Some("low".to_string());
    graph.nodes[1].effort = Some("high".to_string());
    let workspace = std::env::temp_dir().join(format!(
        "agent-graph-effort-isolation-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&workspace).expect("test workspace");
    let cancel = AtomicBool::new(false);
    let outcome = AgentGraphEngine::new(
        workspace.clone(),
        Some(Arc::clone(&club) as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence()
    .run(&graph, "task", &cancel)
    .expect("parallel effort-pinned nodes complete");

    let mut seen = club.seen.lock().unwrap().clone();
    seen.sort();
    assert_eq!(
        seen,
        vec![
            ("high".to_string(), "high".to_string()),
            ("low".to_string(), "low".to_string()),
        ]
    );
    assert_eq!(
        club.set_calls.load(Ordering::SeqCst),
        0,
        "graph preflight must not mutate the shared club"
    );
    assert_eq!(club.reasoning_effort().as_deref(), Some("medium"));

    let mut trace_efforts = outcome
        .episode
        .traces
        .iter()
        .map(|trace| {
            (
                trace.role.clone(),
                trace.requested_route.reasoning_effort.clone(),
                trace.resolved_route.reasoning_effort.clone(),
            )
        })
        .collect::<Vec<_>>();
    trace_efforts.sort();
    assert_eq!(
        trace_efforts,
        vec![
            (
                "high".to_string(),
                Some("high".to_string()),
                Some("high".to_string()),
            ),
            (
                "low".to_string(),
                Some("low".to_string()),
                Some("low".to_string()),
            ),
        ],
        "trace receipts must describe the effort actually carried by each call"
    );
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn effort_rejection_speaks_in_events() {
    let club = StubClub::shared("stub", &["fine"]);
    let mut graph = spec("pinned", vec![node("a", "{task}", &[])]);
    graph.nodes[0].effort = Some("medium".to_string());
    let cancel = AtomicBool::new(false);
    let outcome = engine(club).run(&graph, "t", &cancel).unwrap();
    assert!(
        outcome
            .snapshot
            .events
            .iter()
            .any(|event| event.contains("effort 'medium' not accepted")),
        "{:?}",
        outcome.snapshot.events
    );
}

#[test]
fn unknown_club_speaks_the_options() {
    let club = StubClub::shared("stub", &[]);
    let mut graph = spec("routed", vec![node("a", "{task}", &[])]);
    graph.nodes[0].club = Some("nonexistent".to_string());
    let cancel = AtomicBool::new(false);
    let err = engine(club).run(&graph, "t", &cancel).unwrap_err();
    assert!(err.contains("node 'a'"), "{err}");
    assert!(err.contains("unknown club 'nonexistent'"), "{err}");
}

#[test]
fn find_graph_miss_lists_catalog_with_broken_reasons() {
    let catalog = vec![
        LoadedGraph {
            name: "good".to_string(),
            source: PathBuf::from("good.toml"),
            spec: Some(spec("good", vec![node("a", "{task}", &[])])),
            error: None,
        },
        LoadedGraph {
            name: "bad".to_string(),
            source: PathBuf::from("bad.toml"),
            spec: None,
            error: Some("TOML parse: boom".to_string()),
        },
    ];
    let err = find_graph(&catalog, "missing").unwrap_err();
    assert!(err.contains("good"), "{err}");
    assert!(err.contains("BROKEN: TOML parse: boom"), "{err}");
    let err = find_graph(&catalog, "bad").unwrap_err();
    assert!(err.contains("broken"), "{err}");
}

#[test]
fn gate_evaluation_reads_the_last_verdict_line() {
    let gate = GraphGateSpec {
        verdict: true,
        expect: None,
        retry: None,
        credit_to: None,
        max_retries: 1,
    };
    assert!(evaluate_gate(&gate, "thinking…\nVERDICT: PASS").is_ok());
    assert!(evaluate_gate(&gate, " \tVeRdIcT:\tpass  ").is_ok());
    assert!(evaluate_gate(&gate, "VERDICT: PASS\nwait\nverdict: fail — nope").is_err());
    let failure = evaluate_gate(&gate, "VERDICT: FAIL — tests do not pass").unwrap_err();
    assert!(failure.contains("tests do not pass"), "{failure}");
    assert_eq!(
        evaluate_gate(&gate, "VERDICT: FAIL").unwrap_err(),
        "verdict FAIL"
    );
    assert!(evaluate_gate(&gate, "VERDICT: NOT PASS").is_err());
    assert!(evaluate_gate(&gate, "VERDICT: BYPASS").is_err());
    assert!(evaluate_gate(&gate, "VERDICT: PASSING").is_err());
    assert!(evaluate_gate(&gate, "VERDICT: COMPASS").is_err());
    assert!(evaluate_gate(&gate, "VERDICT: PASS — except tests fail").is_err());
    assert!(evaluate_gate(&gate, "VERDICT : PASS").is_err());
    assert!(evaluate_gate(&gate, "no verdict at all").is_err());
    let expect = GraphGateSpec {
        verdict: false,
        expect: Some("SHIP".to_string()),
        retry: None,
        credit_to: None,
        max_retries: 1,
    };
    assert!(evaluate_gate(&expect, "ok SHIP it").is_ok());
    assert!(evaluate_gate(&expect, "hold").is_err());
}

#[test]
fn reward_binding_cli_round_trips_a_real_episode_end_to_end() {
    // The full scorer chain, pinned: real episode receipt on disk → the
    // real cockpit binary (`angel --bind-graph-reward`) → stdout receipt
    // that matches the store. This is what sidecar scorers actually run.
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "agent-graph-reward-cli-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let _episode_dir = crate::tests::TestEnvGuard::set(
        "ANGEL_AGENT_GRAPH_DIR",
        root.to_str().expect("UTF-8 test root"),
    );
    let club = StubClub::shared("stub", &["answer"]);
    let graph = spec("bind-cli", vec![node("solver", "{task}", &[])]);
    let cancel = AtomicBool::new(false);
    let outcome = AgentGraphEngine::new(workspace.clone(), Some(club as Arc<dyn Club>), Vec::new())
        .run(&graph, "task", &cancel)
        .expect("graph completes");
    let episode_id = outcome.episode.episode_id.clone();
    let trace_id = outcome.episode.traces[0].trace_id.clone();

    // Cargo exposes the built binary's path to integration-test processes
    // via CARGO_BIN_EXE_angel; for unit tests inside the bin crate it is
    // absent, so fall back to the sibling `angel` next to deps/<test-exe>.
    let binary = std::env::var("CARGO_BIN_EXE_angel")
        .ok()
        .or_else(|| {
            std::env::current_exe()
                .ok()?
                .parent()?
                .parent()
                .map(|target| target.join("angel").to_string_lossy().into_owned())
        })
        .expect("locate the cockpit binary");
    let output = std::process::Command::new(binary)
        .args([
            "--bind-graph-reward",
            "--workspace",
            workspace.to_str().expect("UTF-8 workspace"),
            &episode_id,
            &trace_id,
            r#"{"score":1.0}"#,
            "--source",
            "coding_eval",
            "--contract",
            "coding_eval_v1",
        ])
        .env(
            "ANGEL_AGENT_GRAPH_DIR",
            root.to_str().expect("UTF-8 test root"),
        )
        .output()
        .expect("real binary runs");
    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is the binding receipt");
    assert_eq!(receipt["schema"], GRAPH_REWARD_BINDING_SCHEMA);
    assert_eq!(receipt["episode_id"], episode_id.as_str());
    assert_eq!(receipt["trace_id"], trace_id.as_str());
    assert_eq!(receipt["reward"]["score"], 1.0);
    // And the receipt the CLI printed is byte-identical to the store's.
    let binding_path = root
        .join(crate::workspace_store::workspace_key(&workspace))
        .join("episodes")
        .join(format!("{episode_id}.{trace_id}.reward.json"));
    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&binding_path).expect("binding on disk"))
            .expect("binding parses");
    assert_eq!(receipt, on_disk);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn external_reward_binding_is_digest_tight_audited_and_idempotent() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "agent-graph-reward-bind-{}-{}",
        std::process::id(),
        GRAPH_EPISODE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let _episode_dir = crate::tests::TestEnvGuard::set(
        "ANGEL_AGENT_GRAPH_DIR",
        root.to_str().expect("UTF-8 test root"),
    );
    let club = StubClub::shared("stub", &["answer"]);
    let graph = spec("bind", vec![node("solver", "{task}", &[])]);
    let cancel = AtomicBool::new(false);
    let outcome = AgentGraphEngine::new(workspace.clone(), Some(club as Arc<dyn Club>), Vec::new())
        .run(&graph, "task", &cancel)
        .expect("graph completes");
    let episode_id = outcome.episode.episode_id.clone();
    let trace = &outcome.episode.traces[0];
    let trace_id = trace.trace_id.clone();
    let output_sha = trace.output.as_ref().expect("trace output").sha256.clone();

    // The sealed episode receipt itself stays reward-free forever.
    assert!(trace.reward.is_none() && trace.reward_source.is_none());
    assert!(!trace.eligibility.training_eligible);

    // A valid binding records both digests it was bound against.
    let binding = bind_episode_reward(
        &workspace,
        &episode_id,
        &trace_id,
        serde_json::json!({ "score": 1.0 }),
        "coding_eval",
        "coding_eval_v1",
    )
    .expect("external scorer binds reward");
    assert_eq!(binding.schema, GRAPH_REWARD_BINDING_SCHEMA);
    assert_eq!(
        binding.episode_receipt_sha256,
        outcome.episode.receipt_sha256
    );
    assert_eq!(binding.trace_output_sha256, output_sha);
    assert_eq!(binding.reward_source, "coding_eval");
    assert_eq!(binding.reward_contract, "coding_eval_v1");

    // The binding receipt is owner-only on disk.
    let binding_path = root
        .join(crate::workspace_store::workspace_key(&workspace))
        .join("episodes")
        .join(format!("{episode_id}.{trace_id}.reward.json"));
    assert!(binding_path.is_file(), "{}", binding_path.display());
    #[cfg(unix)]
    {
        assert_eq!(
            std::fs::metadata(&binding_path)
                .expect("binding metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    // Identical re-binding is idempotent; a conflicting one is refused.
    let again = bind_episode_reward(
        &workspace,
        &episode_id,
        &trace_id,
        serde_json::json!({ "score": 1.0 }),
        "coding_eval",
        "coding_eval_v1",
    )
    .expect("identical binding idempotent");
    assert_eq!(again, binding);
    let conflict = bind_episode_reward(
        &workspace,
        &episode_id,
        &trace_id,
        serde_json::json!({ "score": 2.0 }),
        "coding_eval",
        "coding_eval_v1",
    )
    .unwrap_err();
    assert!(conflict.contains("refusing overwrite"), "{conflict}");

    // Unknown trace, null reward, and oversized payloads are refused.
    let unknown = "f".repeat(64);
    assert!(
        bind_episode_reward(
            &workspace,
            &episode_id,
            &unknown,
            serde_json::json!({ "score": 1.0 }),
            "coding_eval",
            "coding_eval_v1",
        )
        .unwrap_err()
        .contains("not part of this episode")
    );
    assert!(
        bind_episode_reward(
            &workspace,
            &episode_id,
            &trace_id,
            serde_json::Value::Null,
            "coding_eval",
            "coding_eval_v1",
        )
        .unwrap_err()
        .contains("non-null")
    );
    assert!(
        bind_episode_reward(
            &workspace,
            &episode_id,
            &trace_id,
            serde_json::json!({ "pad": "x".repeat(20_000) }),
            "coding_eval",
            "coding_eval_v1",
        )
        .unwrap_err()
        .contains("maximum")
    );

    // A tampered episode receipt fails its digest audit before any bind.
    let episode_path = root
        .join(crate::workspace_store::workspace_key(&workspace))
        .join("episodes")
        .join(format!("{episode_id}.json"));
    std::fs::write(&episode_path, "tampered").expect("tamper fixture");
    let tampered = bind_episode_reward(
        &workspace,
        &episode_id,
        &trace_id,
        serde_json::json!({ "score": 1.0 }),
        "coding_eval",
        "coding_eval_v1",
    )
    .unwrap_err();
    assert!(tampered.contains("decode episode receipt"), "{tampered}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn graph_reports_incomplete_when_fanin_has_no_result() {
    let club = StubClub::shared("stub", &["plan-ok", "   "]);
    let graph = spec(
        "empty-fanin",
        vec![
            node("planner", "plan {task}", &[]),
            node("fanin", "join {output:planner}", &["planner"]),
        ],
    );
    let cancel = AtomicBool::new(false);
    let error = engine(Arc::clone(&club))
        .run(&graph, "task", &cancel)
        .unwrap_err();
    let failure = error.run_failure().expect("incomplete receipt");
    assert!(!failure.snapshot.fanin.finished_with_result);
    assert_eq!(
        failure.episode.termination.kind,
        GraphEpisodeTerminationKind::Failed
    );
    assert_eq!(
        failure.episode.incomplete_reason.as_deref(),
        Some("node_failed:fanin")
    );
    failure.episode.validate().unwrap();
}

#[test]
fn graph_completed_when_fanin_finishes_with_result() {
    let club = StubClub::shared("stub", &["plan-ok", "joined"]);
    let graph = spec(
        "ok-fanin",
        vec![
            node("planner", "plan {task}", &[]),
            node("fanin", "join {output:planner}", &["planner"]),
        ],
    );
    let cancel = AtomicBool::new(false);
    let outcome = engine(Arc::clone(&club))
        .run(&graph, "task", &cancel)
        .unwrap();
    assert!(outcome.snapshot.fanin.finished_with_result);
    assert_eq!(outcome.snapshot.fanin.result.as_deref(), Some("joined"));
    assert_eq!(
        outcome.episode.termination.kind,
        GraphEpisodeTerminationKind::Completed
    );
    outcome.episode.validate().unwrap();
}

#[test]
fn graph_planner_only_is_incomplete_not_completed() {
    let club = StubClub::shared("stub", &["solo-plan"]);
    let graph = spec("planner-only", vec![node("planner", "plan {task}", &[])]);
    let cancel = AtomicBool::new(false);
    let outcome = engine(Arc::clone(&club)).run(&graph, "task", &cancel);
    // A lone planner is the sink: it did finish with a result.
    let outcome = outcome.unwrap();
    assert!(outcome.snapshot.fanin.finished_with_result);
    assert_eq!(
        outcome.episode.termination.kind,
        GraphEpisodeTerminationKind::Completed
    );
}

#[test]
fn graph_node_failure_incomplete_reason_is_node_failed() {
    let club = FailureCancellationClub::failing("joiner");
    let mut graph = spec(
        "join-fail",
        vec![
            node("planner", "plan", &[]),
            node("joiner", "join", &["planner"]),
        ],
    );
    for n in &mut graph.nodes {
        n.tools = Some("none".into());
    }
    let cancel = AtomicBool::new(false);
    let error = AgentGraphEngine::new(
        std::env::temp_dir(),
        Some(club as Arc<dyn Club>),
        Vec::new(),
    )
    .without_persistence()
    .run(&graph, "task", &cancel)
    .unwrap_err();
    let failure = error.run_failure().unwrap();
    assert!(
        failure
            .snapshot
            .incomplete_reason
            .as_deref()
            .unwrap_or("")
            .starts_with("node_failed:"),
        "{:?}",
        failure.snapshot.incomplete_reason
    );
    assert!(!failure.snapshot.fanin.finished_with_result);
}

fn snap_for_turn(
    id: &str,
    phase: GraphRunPhase,
    fanin: bool,
    nodes_done: usize,
    nodes_total: usize,
) -> GraphSnapshot {
    GraphSnapshot {
        episode_id: id.to_string(),
        workspace_key_sha256: "a".repeat(64),
        graph: "g01-mixed-roles".into(),
        task: "t".into(),
        phase,
        nodes: (0..nodes_total)
            .map(|i| GraphNodeSnap {
                id: format!("n{i}"),
                deps: Vec::new(),
                persona: String::new(),
                club: String::new(),
                grant: String::new(),
                pool: None,
                pool_limit: None,
                lease_id: None,
                phase: if i < nodes_done {
                    GraphNodePhase::Done
                } else {
                    GraphNodePhase::Pending
                },
                elapsed_ms: 0,
                retries: 0,
                gate: None,
                output_chars: 0,
            })
            .collect(),
        events: Vec::new(),
        final_answer: None,
        error: None,
        elapsed_ms: 1,
        fanin: GraphFaninTruth {
            finished_with_result: fanin,
            result: fanin.then(|| "ok".into()),
            reason: None,
        },
        incomplete_reason: (!fanin).then(|| "incomplete".into()),
    }
}

#[test]
fn two_episodes_judge_last_done() {
    let _lock = crate::tests::env_lock();
    clear_graph_turn_episodes();
    publish_graph_snapshot(snap_for_turn("aa", GraphRunPhase::Failed, false, 1, 6));
    publish_graph_snapshot(snap_for_turn("bb", GraphRunPhase::Done, true, 6, 6));
    let list = graph_turn_episodes();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].episode_id, "aa");
    assert_eq!(list[1].episode_id, "bb");
    let judged = judged_graph_snapshot().expect("judged");
    assert_eq!(judged.episode_id, "bb");
    assert!(judged.fanin.finished_with_result);
    assert_eq!(judged.phase, GraphRunPhase::Done);
    publish_graph_snapshot(snap_for_turn("cc", GraphRunPhase::Failed, false, 1, 6));
    let judged = judged_graph_snapshot().expect("still last done");
    assert_eq!(judged.episode_id, "bb");
    clear_graph_turn_episodes();
}
