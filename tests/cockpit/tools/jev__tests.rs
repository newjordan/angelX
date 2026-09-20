use super::*;
use crate::tests::TestEnvGuard as EnvGuard;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

fn input() -> Value {
    json!({"state":"Synthetic fixture: 3 paired latency samples, 20% reduction, all correctness checks passed.",
    "questions":[
        {"id":"risk","type":"noul","instructions":"Is a regression evident?"},
        {"id":"next","type":"choice","instructions":"What should be checked next?","options":["repeat benchmark","abandon candidate"]},
        {"id":"evidence","type":"score","instructions":"How complete is this evidence?","options":["missing","diagnostic","held-out"]}
    ]})
}
fn response() -> Value {
    json!({"model":"jev-fixture", "answers":{
        "risk":{"type":"noul","noul":0.25},
        "next":{"type":"choice","choice":"repeat benchmark","confidence":0.8,"probabilities":{"repeat benchmark":0.9,"abandon candidate":0.1}},
        "evidence":{"type":"score","score":0.6,"confidence":0.4,"probabilities":{"0":0.5,"1":0.4,"2":0.1}}
    },"usage":{"input_tokens":123,"output_tokens":20}})
}
fn tool(endpoint: String) -> JevTool {
    JevTool {
        key: "jev-fixture-credential".into(),
        model: "jev-fixture".into(),
        endpoint,
        timeout: Duration::from_secs(3),
        max_calls: 1,
        session: Mutex::new(Session {
            attempts: 0,
            cache: VecDeque::new(),
        }),
    }
}
fn server(status: u16, body: String) -> (String, thread::JoinHandle<Value>) {
    delayed_server(status, body, Duration::ZERO)
}
fn delayed_server(
    status: u16,
    body: String,
    delay: Duration,
) -> (String, thread::JoinHandle<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/systemone", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut headers = String::new();
        let mut size = 0;
        loop {
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
            if let Some(length) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                size = length.trim().parse::<usize>().unwrap();
            }
            headers.push_str(&line);
        }
        assert!(headers.starts_with("POST /v1/systemone "));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("authorization: bearer jev-fixture-credential")
        );
        let mut bytes = vec![0; size];
        reader.read_exact(&mut bytes).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("jev-fixture-credential"));
        thread::sleep(delay);
        // Timeout and response-cap tests deliberately close the client early.
        let _ = write!(
            stream,
            "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nLocation: http://127.0.0.1:1/never\r\n\r\n{body}",
            body.len()
        );
        serde_json::from_slice(&bytes).unwrap()
    });
    (url, handle)
}
#[test]
fn types_percentages_and_rubrics_preserve_distinct_meanings() {
    let request = request_body(&input(), "jev-fixture").unwrap();
    let out = normalize_response(&request, &response()).unwrap();
    assert_eq!(out["source"], "model_estimate");
    assert_eq!(out["answers"]["risk"]["probability_percent"], 25.0);
    assert!(out["answers"]["risk"].get("confidence_percent").is_none());
    assert_eq!(out["answers"]["next"]["confidence_percent"], 80.0);
    assert_eq!(out["answers"]["evidence"]["rubric_position_percent"], 30.0);
    assert_eq!(out["usage"]["input_tokens"], 123);
}
#[test]
fn malformed_provider_answers_are_rejected() {
    let request = request_body(&input(), "jev-fixture").unwrap();
    for (pointer, value) in [
        ("/answers/risk/noul", json!(1.2)),
        ("/answers/risk/type", json!("choice")),
        ("/answers/next/choice", json!("invented")),
        ("/answers/next/choice", json!("abandon candidate")),
        ("/answers/next/confidence", json!(-0.1)),
        (
            "/answers/next/probabilities",
            json!({"repeat benchmark":0.9}),
        ),
        ("/answers/next/probabilities/repeat benchmark", json!(0.5)),
        ("/answers/evidence/score", json!(1.9)),
        ("/usage/input_tokens", json!(null)),
    ] {
        let mut payload = response();
        *payload.pointer_mut(pointer).unwrap() = value;
        assert!(normalize_response(&request, &payload).is_err(), "{pointer}");
    }
}
#[test]
fn invalid_inputs_and_credentials_never_dispatch() {
    let _lock = crate::tests::env_lock();
    let t = tool("http://127.0.0.1:1".into());
    for invalid in [
        json!({}),
        json!({"state":"x","questions":[]}),
        json!({"state":"é".repeat(24_576),"questions":input()["questions"]}),
        json!({"state":"jev-fixture-credential","questions":input()["questions"]}),
    ] {
        assert!(t.call(&invalid).is_err());
    }
    let mut duplicate = input();
    duplicate["questions"][1]["id"] = json!("risk");
    assert!(t.call(&duplicate).is_err());
    assert_eq!(t.session.lock().unwrap().attempts, 0);
}
#[test]
fn real_transport_auth_cache_and_budget_are_connected() {
    let _lock = crate::tests::env_lock();
    let (url, server) = server(200, response().to_string());
    let t = tool(url);
    let first = t.decide(&input()).unwrap();
    assert_eq!(first["cached"], false);
    assert_eq!(first["requests_remaining"], 0);
    assert_eq!(t.decide(&input()).unwrap()["cached"], true);
    let mut changed = input();
    changed["state"] = json!("Different synthetic evidence");
    assert!(t.decide(&changed).unwrap_err().contains("budget exhausted"));
    assert_eq!(
        server.join().unwrap(),
        request_body(&input(), "jev-fixture").unwrap()
    );
}
#[test]
fn transport_errors_do_not_echo_body_or_follow_redirects() {
    let _lock = crate::tests::env_lock();
    for status in [401, 429, 302] {
        let (url, server) = server(status, "jev-fixture-credential private-input".into());
        let error = tool(url).call(&input()).unwrap_err();
        assert!(!error.contains("credential") && !error.contains("private-input"));
        assert!(error.contains(&status.to_string()));
        server.join().unwrap();
    }
}
#[test]
fn response_bytes_and_slow_headers_have_hard_bounds() {
    let _lock = crate::tests::env_lock();
    let (url, server) = server(200, "x".repeat(MAX_RESPONSE + 1));
    assert!(tool(url).call(&input()).unwrap_err().contains("exceeds"));
    server.join().unwrap();
    let (url, server) = delayed_server(200, response().to_string(), Duration::from_millis(500));
    let mut t = tool(url);
    t.timeout = Duration::from_millis(100);
    let start = Instant::now();
    assert!(t.call(&input()).unwrap_err().contains("timed out"));
    assert!(start.elapsed() < Duration::from_secs(2));
    server.join().unwrap();
}
#[test]
fn tool_is_deferred_discoverable_and_has_an_off_control() {
    let _lock = crate::tests::env_lock();
    let _key = EnvGuard::set("TYPESAFE_API_KEY", "jev-fixture-credential");
    let _enabled = EnvGuard::set("ANGEL_JEV", "1");
    let mut registry = ToolRegistry::new();
    maybe_register_jev(&mut registry);
    assert!(registry.has_tool("jev_decide"));
    assert!(registry.has_tool("benchmark_compare"));
    assert!(!registry.defs().iter().any(|d| d.name == "jev_decide"));
    registry.enable_tool_search();
    let discovery = registry
        .dispatch("tool_search", &json!({"query":"jev_decide"}))
        .unwrap();
    assert!(discovery.contains("jev_decide"));
    let _off = EnvGuard::set("ANGEL_JEV", "0");
    let mut disabled = ToolRegistry::new();
    maybe_register_jev(&mut disabled);
    assert!(!disabled.has_tool("jev_decide"));
    assert!(disabled.has_tool("benchmark_compare"));
}
#[test]
fn ordinary_and_team_kits_both_register_jev() {
    let _lock = crate::tests::env_lock();
    let _key = EnvGuard::set("TYPESAFE_API_KEY", "jev-fixture-credential");
    let _enabled = EnvGuard::set("ANGEL_JEV", "1");
    let solo = ToolRegistry::with_defaults();
    let team = ToolRegistry::with_team(std::env::current_dir().unwrap(), vec![]);
    for registry in [solo, team] {
        assert!(registry.has_tool("jev_decide"));
        assert!(registry.has_tool("benchmark_compare"));
    }
}
