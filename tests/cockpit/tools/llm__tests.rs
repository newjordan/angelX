
use super::*;

#[test]
fn normalize_base_strips_v1_and_slashes() {
    assert_eq!(normalize_base("http://h:8000"), "http://h:8000");
    assert_eq!(normalize_base("http://h:8000/"), "http://h:8000");
    assert_eq!(normalize_base("http://h:8000/v1"), "http://h:8000");
    assert_eq!(normalize_base("http://h:8000/v1/"), "http://h:8000");
    assert_eq!(normalize_base(" http://h:8000/V1 "), "http://h:8000");
}

#[test]
fn model_ids_reads_openai_shape() {
    let v: Value = serde_json::json!({
        "data": [ { "id": "qwen3.5-35b" }, { "id": "gemma4" } ]
    });
    assert_eq!(model_ids(&v), vec!["qwen3.5-35b", "gemma4"]);
    assert!(model_ids(&serde_json::json!({})).is_empty());
}

#[test]
#[ignore = "spawns a python mock OpenAI endpoint; run with --ignored"]
fn live_probe_and_bench_against_mock_endpoint() {
    let _lock = crate::tests::env_lock();
    const PORT: u16 = 18477;
    const PY: &str = r#"
import http.server, json, time
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        if self.path == '/v1/models':
            body = json.dumps({'data': [{'id': 'mock-7b'}]}).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404); self.end_headers()
    def do_POST(self):
        n = int(self.headers.get('Content-Length', 0)); self.rfile.read(n)
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for i in range(24):
            chunk = {'choices': [{'delta': {'content': 'tok '}}]}
            self.wfile.write(('data: ' + json.dumps(chunk) + '\n\n').encode())
            self.wfile.flush(); time.sleep(0.005)
        usage = {'choices': [], 'usage': {'completion_tokens': 24}}
        self.wfile.write(('data: ' + json.dumps(usage) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n'); self.wfile.flush()
import sys
http.server.HTTPServer(('127.0.0.1', int(sys.argv[1])), H).serve_forever()
"#;
    let mut server = std::process::Command::new("python3")
        .arg("-c")
        .arg(PY)
        .arg(PORT.to_string())
        .spawn()
        .expect("python3 available");
    let url = format!("http://127.0.0.1:{PORT}");
    // Wait for the mock to come up.
    let probe = (0..50)
        .find_map(|_| {
            std::thread::sleep(Duration::from_millis(100));
            LlmProbeTool.call(&serde_json::json!({ "url": url })).ok()
        })
        .expect("mock endpoint never came up");
    let bench = LlmBenchTool.call(&serde_json::json!({
        "url": format!("{url}/v1"),
        "runs": 2,
        "max_tokens": 32,
    }));
    let _ = server.kill();
    let _ = server.wait();
    assert!(probe.contains("mock-7b"), "probe: {probe}");
    let bench = bench.expect("bench against mock");
    println!("llm_bench →\n{bench}");
    assert!(bench.contains("mean of 2"), "bench: {bench}");
    assert!(bench.contains("from server usage"), "bench: {bench}");
    assert!(bench.contains("model mock-7b"), "bench: {bench}");
}

#[test]
fn chunk_token_detection_covers_content_and_reasoning() {
    let content: Value = serde_json::json!({
        "choices": [{ "delta": { "content": "hi" } }]
    });
    let reasoning: Value = serde_json::json!({
        "choices": [{ "delta": { "reasoning_content": "hmm" } }]
    });
    let role_only: Value = serde_json::json!({
        "choices": [{ "delta": { "role": "assistant", "content": "" } }]
    });
    let usage_only: Value = serde_json::json!({ "usage": { "completion_tokens": 42 } });
    assert!(chunk_has_token(&content));
    assert!(chunk_has_token(&reasoning));
    assert!(!chunk_has_token(&role_only));
    assert!(!chunk_has_token(&usage_only));
}
