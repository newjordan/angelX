
use super::*;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};

static LSP_TEST_NONCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn lsp_prewarm_requires_an_explicit_truthy_opt_in() {
    assert!(!lsp_prewarm_enabled(None));
    assert!(!lsp_prewarm_enabled(Some("")));
    assert!(!lsp_prewarm_enabled(Some("0")));
    assert!(!lsp_prewarm_enabled(Some("false")));
    assert!(lsp_prewarm_enabled(Some("1")));
    assert!(lsp_prewarm_enabled(Some(" TRUE ")));
    assert!(lsp_prewarm_enabled(Some("yes")));
    assert!(lsp_prewarm_enabled(Some("on")));
}

#[test]
fn present_extensions_scans_nested_and_skips_vendor_trees() {
    let root = std::env::temp_dir().join(format!("angel_lsp_scan_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    std::fs::write(root.join("src/deep/main.RS"), "fn main() {}").unwrap();
    std::fs::write(root.join("README.md"), "# hi").unwrap();
    std::fs::write(root.join("node_modules/pkg/index.js"), "x").unwrap();
    let seen = present_extensions(&root, 4096);
    assert!(seen.contains("rs"), "nested + case-folded: {seen:?}");
    assert!(seen.contains("md"), "{seen:?}");
    assert!(
        !seen.contains("js"),
        "vendor trees must be skipped: {seen:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn frame_roundtrips_through_reader() {
    let a = json!({ "jsonrpc": "2.0", "id": 1, "result": { "ok": true } });
    let b = json!({ "jsonrpc": "2.0", "method": "textDocument/publishDiagnostics" });
    let mut bytes = build_frame(&a);
    bytes.extend(build_frame(&b)); // two concatenated frames
    let mut cur = Cursor::new(bytes);
    assert_eq!(read_frame(&mut cur).unwrap(), Some(a));
    assert_eq!(read_frame(&mut cur).unwrap(), Some(b));
    assert_eq!(read_frame(&mut cur).unwrap(), None); // EOF
}

#[test]
fn frame_header_is_content_length_crlf() {
    let f = build_frame(&json!({"x": 1}));
    let s = String::from_utf8(f).unwrap();
    assert!(s.starts_with("Content-Length: 7\r\n\r\n"), "got {s:?}");
    assert!(s.ends_with("{\"x\":1}"));
}

#[test]
fn read_frame_tolerates_extra_headers() {
    let body = "{\"a\":123}";
    let framed = format!(
        "Content-Length: {}\r\nContent-Type: x\r\n\r\n{}",
        body.len(),
        body
    );
    let mut cur = Cursor::new(framed.into_bytes());
    assert_eq!(read_frame(&mut cur).unwrap(), Some(json!({"a": 123})));
}

#[test]
fn doc_action_opens_then_changes_with_bumped_version() {
    assert_eq!(doc_action(None), DocAction::Open); // never seen -> open
    assert_eq!(doc_action(Some(1)), DocAction::Change { version: 2 });
    assert_eq!(doc_action(Some(2)), DocAction::Change { version: 3 });
}

#[test]
fn match_response_correlates_and_surfaces_errors() {
    let ok = json!({ "jsonrpc": "2.0", "id": 5, "result": { "v": 1 } });
    assert_eq!(match_response(&ok, 5), Some(Ok(json!({"v": 1}))));
    assert_eq!(match_response(&ok, 6), None); // wrong id
    let notif = json!({ "jsonrpc": "2.0", "method": "log" });
    assert_eq!(match_response(&notif, 5), None);
    let err = json!({ "jsonrpc": "2.0", "id": 5, "error": { "message": "boom" } });
    assert_eq!(match_response(&err, 5), Some(Err("boom".into())));
}

#[test]
fn match_response_ignores_server_request_with_colliding_id() {
    // A server→client REQUEST has an id AND a method. Even when its id equals
    // our pending request id, it must NOT be taken as our response (the bug
    // this guards: returning Null for the request's missing `result`).
    let server_req = json!({ "jsonrpc": "2.0", "id": 5, "method": "window/workDoneProgress/create",
                    "params": { "token": "t" } });
    assert_eq!(match_response(&server_req, 5), None);
}

#[test]
fn is_server_request_needs_id_and_method() {
    assert!(is_server_request(&json!({ "id": 1, "method": "x" })));
    assert!(!is_server_request(&json!({ "method": "x" }))); // notification
    assert!(!is_server_request(&json!({ "id": 1, "result": {} }))); // response
}

#[test]
fn server_request_reply_echoes_id_with_null_result() {
    let reply = server_request_reply(&json!({ "id": 7, "method": "m" }));
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["result"], Value::Null);
    assert_eq!(reply["jsonrpc"], "2.0");
}

#[test]
fn publish_diagnostics_matches_uri() {
    let n = json!({
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": "file:///a.rs", "diagnostics": [] }
    });
    assert!(publish_diagnostics_for(&n, "file:///a.rs").is_some());
    assert!(publish_diagnostics_for(&n, "file:///b.rs").is_none());
    let other = json!({ "method": "window/logMessage", "params": {} });
    assert!(publish_diagnostics_for(&other, "file:///a.rs").is_none());
}

#[test]
fn format_diagnostics_is_one_based_and_labels_severity() {
    let params = json!({
        "uri": "file:///x.rs",
        "diagnostics": [
            { "severity": 1, "range": { "start": { "line": 9, "character": 4 } },
              "message": "mismatched types", "code": "E0308" },
            { "severity": 2, "range": { "start": { "line": 0, "character": 0 } },
              "message": "unused\nvariable" }
        ]
    });
    let out = format_diagnostics("x.rs", &params);
    assert!(out.contains("2 diagnostic(s)"));
    assert!(
        out.contains("error 10:5  mismatched types [E0308]"),
        "got:\n{out}"
    );
    assert!(
        out.contains("warning 1:1  unused variable"),
        "newline flattened:\n{out}"
    );
}

#[test]
fn format_diagnostics_reports_clean() {
    let params = json!({ "uri": "file:///x.rs", "diagnostics": [] });
    assert!(format_diagnostics("x.rs", &params).contains("clean — 0 diagnostics"));
}

#[test]
fn pick_server_by_extension_is_case_insensitive() {
    let servers = default_servers();
    assert_eq!(
        pick_server(&servers, Path::new("a/b.rs")).unwrap().name,
        "rust"
    );
    assert_eq!(
        pick_server(&servers, Path::new("S.PY")).unwrap().name,
        "python"
    );
    assert_eq!(
        pick_server(&servers, Path::new("x.tsx")).unwrap().name,
        "typescript"
    );
    assert!(pick_server(&servers, Path::new("README")).is_none());
    assert!(pick_server(&servers, Path::new("data.bin")).is_none());
}

#[cfg(unix)]
#[test]
fn unprovisioned_rustup_proxy_does_not_auto_enable_lsp() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let _guard = crate::tests::env_lock();
    let dir = std::env::temp_dir().join(format!("angel-lsp-rustup-proxy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rustup = dir.join("rustup");
    std::fs::write(&rustup, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink("rustup", dir.join("rust-analyzer")).unwrap();
    // The fakes must resolve FIRST, but the system dirs must stay on PATH:
    // env_lock serializes env mutators, yet reader tests that spawn real
    // binaries (git, cargo, rust-analyzer) do not all hold it — a PATH
    // with no fallback starves them mid-run (the intermittent full-suite
    // flakes). Same fallback discipline as cargo_tool's hostile-PATH test.
    let path_with_fallback = format!("{}:/usr/local/bin:/usr/bin:/bin", dir.to_str().unwrap());
    let _path = crate::tests::TestEnvGuard::set("PATH", &path_with_fallback);

    reset_command_on_path_cache();
    assert!(!command_on_path("rust-analyzer"));

    // Atomic replace: a plain truncate+write lets a racing exec observe a
    // half-written script (the intermittent flake this pins shut).
    let tmp = dir.join("rustup.next");
    std::fs::write(&tmp, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::rename(&tmp, &rustup).unwrap();
    reset_command_on_path_cache();
    assert!(command_on_path("rust-analyzer"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn parse_lsp_config_reads_and_drops_commandless() {
    let text = r#"{
            "servers": {
                "rust":   { "command": "rust-analyzer", "extensions": [".rs"], "languageId": "rust" },
                "zig":    { "command": "zls", "args": [], "extensions": ["zig"] },
                "broken": { "extensions": ["x"] }
            }
        }"#;
    let mut s = parse_lsp_config(text);
    s.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(s.len(), 2, "command-less server dropped: {s:?}");
    let rust = s.iter().find(|x| x.name == "rust").unwrap();
    assert_eq!(rust.extensions, vec!["rs"]); // leading dot stripped
    let zig = s.iter().find(|x| x.name == "zig").unwrap();
    assert_eq!(zig.language_id, "zig"); // defaulted to the key
}

#[test]
fn parse_lsp_config_tolerates_garbage() {
    assert!(parse_lsp_config("not json").is_empty());
    assert!(parse_lsp_config("{}").is_empty());
    assert!(parse_lsp_config(r#"{"servers": {}}"#).is_empty());
}

#[test]
fn user_config_overrides_default_by_name() {
    // Build the merge by hand (load_servers reads env/HOME; this tests the rule).
    let mut servers = default_servers();
    let user = parse_lsp_config(
        r#"{"servers":{"rust":{"command":"my-ra","extensions":["rs","rsx"],"languageId":"rust"}}}"#,
    );
    for u in user {
        if let Some(e) = servers.iter_mut().find(|s| s.name == u.name) {
            *e = u;
        } else {
            servers.push(u);
        }
    }
    let rust = servers.iter().find(|s| s.name == "rust").unwrap();
    assert_eq!(rust.command, "my-ra");
    assert_eq!(rust.extensions, vec!["rs", "rsx"]);
}

#[test]
fn lsp_readiness_cached_across_documents_and_js_open_precedes_queries() {
    let _guard = crate::tests::env_lock();
    let root = lsp_test_counter("readiness");
    std::fs::create_dir_all(&root).unwrap();
    for file in ["a.js", "b.js"] {
        std::fs::write(root.join(file), "function foo() {}\n").unwrap();
    }
    let server = LspServer {
            name: "typescript".into(), command: "python3".into(),
            extensions: vec!["js".into()], language_id: "typescript".into(),
            args: vec!["-u".into(), "-c".into(), r#"
import json, sys
opened = {}
configured = False
def emit(value):
    body = json.dumps(value).encode()
    sys.stdout.buffer.write(('Content-Length: %d\r\n\r\n' % len(body)).encode() + body)
    sys.stdout.buffer.flush()
while True:
    header = sys.stdin.buffer.readline()
    if not header: break
    size = int(header.split(b':')[1])
    sys.stdin.buffer.readline()
    msg = json.loads(sys.stdin.buffer.read(size))
    method = msg.get('method')
    if method == 'initialize':
        emit({'id': msg['id'], 'result': {'capabilities': {}}})
    elif method == 'workspace/didChangeConfiguration':
        configured = msg['params']['settings']['implicitProjectConfiguration']['checkJs']
    elif method == 'textDocument/didOpen':
        doc = msg['params']['textDocument']
        assert configured and doc['languageId'] == 'javascript'
        opened[doc['uri']] = doc
        if len(opened) == 1:
            emit({'method': 'textDocument/publishDiagnostics', 'params': {'uri': doc['uri'], 'diagnostics': []}})
    elif method == 'textDocument/didChange':
        raise AssertionError('unchanged document must not be synced again')
    elif method == 'textDocument/documentSymbol':
        assert msg['params']['textDocument']['uri'] in opened
        emit({'id': msg['id'], 'result': [{'name': 'foo', 'kind': 12, 'range': {'start': {'line': 0, 'character': 0}}, 'selectionRange': {'start': {'line': 0, 'character': 9}}}]})
"#.into()],
        };
    let ctx = Arc::new(LspCtx {
        pool: Arc::new(LspPool::new(Duration::from_secs(2))),
        resolve_root: root.clone(),
        servers: vec![server],
        timeout: Duration::from_secs(2),
        settle: Duration::from_millis(5),
    });
    let tool = LspSymbolsTool {
        ctx: Arc::clone(&ctx),
    };
    let first = tool.call(&json!({"path": "a.js"})).unwrap();
    assert!(first.contains("readiness: diagnostics_received"), "{first}");
    for path in ["a.js", "b.js"] {
        let answer = tool.call(&json!({"path": path})).unwrap();
        assert!(
            answer.contains("readiness: cached_ready; readiness_wait_ms: 0"),
            "{answer}"
        );
        assert!(answer.contains("1 symbol(s)"), "{answer}");
    }
    assert_eq!(ctx.pool.state.lock().unwrap().clients.len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

/// A minimal LSP server in POSIX sh: Content-Length framing, replies to
/// initialize (advertising pull), pushes one error on didOpen, answers a pull
/// `textDocument/diagnostic` clean, and serves fixed definition/references/
/// hover results so the navigation tools can be exercised end-to-end.
const MOCK_LSP: &str = r#"
emit() { printf 'Content-Length: %s\r\n\r\n%s' "${#1}" "$1"; }
cr=$(printf '\r'); len=0
while IFS= read -r line; do
  line=${line%$cr}
  case "$line" in
    Content-Length:*) len=${line#Content-Length: } ;;
    "")
      [ "$len" -gt 0 ] || continue
      body=$(head -c "$len"); len=0
      id=$(printf '%s' "$body" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
      case "$body" in
        *'"method":"initialize"'*) emit "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"capabilities\":{\"diagnosticProvider\":{}}}}" ;;
        *'"textDocument/didOpen"'*)
          # A server->client REQUEST the client must auto-answer (id collides with
          # the client's own id space on purpose), then the diagnostics push.
          emit '{"jsonrpc":"2.0","id":1,"method":"window/workDoneProgress/create","params":{"token":"t"}}'
          emit '{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":"file:///mock.rs","diagnostics":[{"severity":1,"range":{"start":{"line":0,"character":4}},"message":"mock error","code":"M1"}]}}' ;;
        *'"textDocument/diagnostic"'*) emit "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"kind\":\"full\",\"items\":[]}}" ;;
        *'"textDocument/definition"'*) emit "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"uri\":\"file:///mock.rs\",\"range\":{\"start\":{\"line\":2,\"character\":3},\"end\":{\"line\":2,\"character\":6}}}}" ;;
        *'"textDocument/references"'*) emit "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":[{\"uri\":\"file:///mock.rs\",\"range\":{\"start\":{\"line\":2,\"character\":3},\"end\":{\"line\":2,\"character\":6}}},{\"uri\":\"file:///mock.rs\",\"range\":{\"start\":{\"line\":5,\"character\":0},\"end\":{\"line\":5,\"character\":3}}}]}" ;;
        *'"textDocument/hover"'*) emit "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"contents\":{\"kind\":\"markdown\",\"value\":\"fn foo() -> i32\"}}}" ;;
        *'"textDocument/documentSymbol"'*) emit "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":[{\"name\":\"Foo\",\"kind\":23,\"range\":{\"start\":{\"line\":0,\"character\":0}},\"selectionRange\":{\"start\":{\"line\":0,\"character\":7}},\"children\":[{\"name\":\"bar\",\"kind\":12,\"detail\":\"fn(&self)\",\"selectionRange\":{\"start\":{\"line\":1,\"character\":7}}}]}]}" ;;
        *'"workspace/symbol"'*) emit "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":[{\"name\":\"Foo\",\"kind\":23,\"location\":{\"uri\":\"file:///mock.rs\",\"range\":{\"start\":{\"line\":2,\"character\":0}}},\"containerName\":\"mymod\"}]}" ;;
      esac ;;
  esac
done
"#;

fn mock_server() -> LspServer {
    LspServer {
        name: "mock".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), MOCK_LSP.into()],
        extensions: vec!["rs".into()],
        language_id: "plaintext".into(),
    }
}

fn lsp_test_counter(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "angel-lsp-{label}-{}-{}",
        std::process::id(),
        LSP_TEST_NONCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn counting_server(counter: &Path, body: &str) -> LspServer {
    let mut server = mock_server();
    server.args = vec![
        "-c".into(),
        format!("printf x >> \"$0\"\n{body}"),
        counter.to_string_lossy().into_owned(),
    ];
    server
}

fn process_start_count(counter: &Path) -> usize {
    std::fs::read(counter).map_or(0, |starts| starts.len())
}

/// Full protocol over real stdio pipes, deterministic (no real server):
/// handshake → pull-capability → cold didOpen+push (error) → warm
/// didChange+pull (clean).
#[test]
fn mock_full_protocol_push_then_pull() {
    let client = LspClient::spawn(&mock_server(), Duration::from_secs(5)).expect("spawn");
    client.initialize(Path::new("/tmp")).expect("initialize");
    assert!(client.supports_pull(), "mock advertised diagnosticProvider");

    let uri = "file:///mock.rs";
    // cold: first open -> push diagnostics
    assert_eq!(
        client.sync_doc(uri, "plaintext", "broken").unwrap(),
        DocAction::Open
    );
    let push = client
        .collect_diagnostics(uri, Duration::from_millis(80))
        .expect("push");
    assert!(
        format_diagnostics("mock.rs", &push).contains("error 1:5  mock error [M1]"),
        "got: {}",
        format_diagnostics("mock.rs", &push)
    );
    // warm: change -> pull (empty/clean), version bumped, no respawn
    assert_eq!(
        client.sync_doc(uri, "plaintext", "fixed").unwrap(),
        DocAction::Change { version: 2 }
    );
    let pull = client
        .pull_diagnostics_with_timeout(uri, client.timeout)
        .expect("pull");
    assert!(format_diagnostics("mock.rs", &pull).contains("clean"));
}

#[test]
fn mock_pool_reuses_and_evicts() {
    let pool = LspPool::new(Duration::from_secs(5));
    let server = mock_server();
    assert!(pool.get_warm(&server, Path::new("/tmp")).is_none());
    let c1 = pool
        .get_or_spawn(&server, Path::new("/tmp"))
        .expect("spawn 1");
    assert!(pool.get_warm(&server, Path::new("/tmp")).is_some());
    let c2 = pool
        .get_or_spawn(&server, Path::new("/tmp"))
        .expect("reuse");
    assert!(
        Arc::ptr_eq(&c1, &c2),
        "second call must reuse the cached client"
    );
    assert_eq!(pool.state.lock().unwrap().clients.len(), 1);
    pool.evict("mock");
    assert!(pool.get_warm(&server, Path::new("/tmp")).is_none());
    assert_eq!(pool.state.lock().unwrap().clients.len(), 0);
}

#[test]
fn simultaneous_cold_calls_share_exactly_one_initialization() {
    let counter = lsp_test_counter("singleflight");
    let server = Arc::new(counting_server(&counter, MOCK_LSP));
    let pool = Arc::new(LspPool::new(Duration::from_secs(10)));
    let barrier = Arc::new(std::sync::Barrier::new(5));

    let clients = std::thread::scope(|scope| {
        let mut calls = Vec::new();
        for _ in 0..4 {
            let pool = Arc::clone(&pool);
            let server = Arc::clone(&server);
            let barrier = Arc::clone(&barrier);
            calls.push(scope.spawn(move || {
                barrier.wait();
                pool.get_or_spawn(&server, Path::new("/tmp"))
                    .expect("shared cold start")
            }));
        }
        barrier.wait();
        calls
            .into_iter()
            .map(|call| call.join().expect("cold caller panicked"))
            .collect::<Vec<_>>()
    });

    assert_eq!(process_start_count(&counter), 1, "must spawn exactly once");
    assert!(
        clients
            .iter()
            .all(|client| Arc::ptr_eq(client, &clients[0])),
        "all followers must receive the leader's initialized client"
    );
    std::fs::remove_file(counter).ok();
}

#[test]
fn deterministic_initialize_failure_suppresses_a_concurrent_storm() {
    let counter = lsp_test_counter("failure-storm");
    let server = Arc::new(counting_server(&counter, "exit 0"));
    let pool = Arc::new(LspPool::new(Duration::from_secs(1)));
    let barrier = Arc::new(std::sync::Barrier::new(5));

    let errors = std::thread::scope(|scope| {
        let mut calls = Vec::new();
        for _ in 0..4 {
            let pool = Arc::clone(&pool);
            let server = Arc::clone(&server);
            let barrier = Arc::clone(&barrier);
            calls.push(scope.spawn(move || {
                barrier.wait();
                pool.get_or_spawn(&server, Path::new("/tmp"))
                    .err()
                    .expect("server exits during initialize")
            }));
        }
        barrier.wait();
        calls
            .into_iter()
            .map(|call| call.join().expect("cold caller panicked"))
            .collect::<Vec<_>>()
    });

    assert_eq!(
        process_start_count(&counter),
        1,
        "failure storm spawned twice"
    );
    assert!(errors.windows(2).all(|pair| pair[0] == pair[1]));
    let cached = pool
        .get_or_spawn(&server, Path::new("/tmp"))
        .err()
        .expect("deterministic failure must remain negatively cached");
    assert_eq!(cached, errors[0]);
    assert_eq!(process_start_count(&counter), 1);
    assert_eq!(pool.state.lock().unwrap().failures.len(), 1);
    std::fs::remove_file(counter).ok();
}

#[test]
fn missing_executable_spawn_failure_is_negatively_cached() {
    let mut server = mock_server();
    server.command = format!(
        "/angel0-test-missing-lsp-{}-{}",
        std::process::id(),
        LSP_TEST_NONCE.fetch_add(1, Ordering::Relaxed)
    );
    server.args.clear();
    let pool = LspPool::new(Duration::from_secs(1));

    let first = pool
        .get_or_spawn(&server, Path::new("/tmp"))
        .err()
        .expect("missing executable must fail");
    let second = pool
        .get_or_spawn(&server, Path::new("/tmp"))
        .err()
        .expect("missing executable failure should be cached");
    assert_eq!(first, second);
    assert_eq!(pool.state.lock().unwrap().failures.len(), 1);
}

#[test]
fn initialization_timeout_does_not_poison_future_cold_starts() {
    let counter = lsp_test_counter("timeout-retry");
    let server = counting_server(&counter, "cat >/dev/null");
    let pool = LspPool::new(Duration::from_millis(40));

    for expected_starts in 1..=2 {
        let error = pool
            .get_or_spawn(&server, Path::new("/tmp"))
            .err()
            .expect("server deliberately never replies");
        assert!(error.contains("timed out"), "got: {error}");
        assert_eq!(process_start_count(&counter), expected_starts);
        assert!(
            pool.state.lock().unwrap().failures.is_empty(),
            "timeout must not enter the negative cache"
        );
    }
    assert!(!is_cacheable_initialize_failure("request cancelled"));
    assert!(!is_cacheable_initialize_failure("operation aborted"));
    std::fs::remove_file(counter).ok();
}

#[test]
fn negative_cache_retries_after_ttl_without_sleeping() {
    let counter = lsp_test_counter("ttl-retry");
    let server = counting_server(&counter, "exit 0");
    let start = Instant::now();
    let clock_value = Arc::new(Mutex::new(start));
    let clock_reader = Arc::clone(&clock_value);
    let pool = LspPool::with_clock(
        Duration::from_secs(1),
        Duration::from_secs(5),
        Arc::new(move || *clock_reader.lock().unwrap()),
    );

    pool.get_or_spawn(&server, Path::new("/tmp"))
        .err()
        .expect("first failure");
    pool.get_or_spawn(&server, Path::new("/tmp"))
        .err()
        .expect("cached failure");
    assert_eq!(process_start_count(&counter), 1);
    *clock_value.lock().unwrap() = start + Duration::from_secs(6);
    pool.get_or_spawn(&server, Path::new("/tmp"))
        .err()
        .expect("retry after TTL");
    assert_eq!(process_start_count(&counter), 2);
    std::fs::remove_file(counter).ok();
}

#[test]
fn fingerprint_change_invalidates_failure_and_success_clears_it() {
    let failed_counter = lsp_test_counter("fingerprint-failed");
    let healthy_counter = lsp_test_counter("fingerprint-healthy");
    let failed = counting_server(&failed_counter, "exit 0");
    let healthy = counting_server(&healthy_counter, MOCK_LSP);
    let pool = LspPool::new(Duration::from_secs(5));

    pool.get_or_spawn(&failed, Path::new("/tmp"))
        .err()
        .expect("first fingerprint fails deterministically");
    assert_eq!(pool.state.lock().unwrap().failures.len(), 1);
    pool.get_or_spawn(&healthy, Path::new("/tmp"))
        .expect("new fingerprint must bypass the old negative result");
    assert_eq!(process_start_count(&failed_counter), 1);
    assert_eq!(process_start_count(&healthy_counter), 1);
    let state = pool.state.lock().unwrap();
    assert!(state.failures.is_empty());
    assert_eq!(state.clients.len(), 1);
    drop(state);
    std::fs::remove_file(failed_counter).ok();
    std::fs::remove_file(healthy_counter).ok();
}

#[test]
fn dead_cached_server_is_evicted_and_next_request_respawns_once() {
    let dir = std::env::temp_dir().join(format!("angel-lsp-death-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("mock.rs");
    std::fs::write(&file, "fn foo() {}\nfoo();\n").unwrap();
    let ctx = mock_ctx(dir.clone());
    let first = ctx
        .pool
        .get_or_spawn(&ctx.servers[0], &ctx.resolve_root)
        .expect("warm server");
    first
        .child
        .lock()
        .unwrap()
        .kill()
        .expect("kill mock server");
    let _ = first.child.lock().unwrap().wait();

    let tool = LspNavTool {
        kind: NavKind::Definition,
        ctx: Arc::clone(&ctx),
    };
    let result = tool
        .call(&json!({ "path": file, "symbol": "foo" }))
        .expect("request should recover through one respawn");
    assert!(result.contains("1 location(s)"), "{result}");
    let replacement = ctx
        .pool
        .get_warm(&ctx.servers[0], &ctx.resolve_root)
        .expect("replacement");
    assert!(!Arc::ptr_eq(&first, &replacement));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn warm_client_survives_a_cd_into_a_subdirectory_of_its_index_root() {
    // The `/cd` registry rebuild re-lists LSP tools on the SAME warm
    // analyzer when the new workspace lies inside the old index root.
    let root = std::env::temp_dir().join(format!("angel-lsp-reuse-{}", std::process::id()));
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let ctx = mock_ctx(root.clone());
    let first = ctx
        .pool
        .get_or_spawn(&ctx.servers[0], &ctx.resolve_root)
        .expect("warm server");

    let rebound = Arc::new(LspCtx {
        pool: Arc::clone(&ctx.pool),
        resolve_root: sub.clone(),
        servers: ctx.servers.clone(),
        timeout: ctx.timeout,
        settle: ctx.settle,
    });
    let reused = rebound
        .pool
        .get_or_spawn(&rebound.servers[0], &rebound.resolve_root)
        .expect("reuse in subdirectory");
    assert!(
        Arc::ptr_eq(&first, &reused),
        "a /cd into a subdirectory must reuse the warm analyzer"
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn cross_root_cd_retires_the_warm_client_and_respawns() {
    let a = std::env::temp_dir().join(format!("angel-lsp-root-a-{}", std::process::id()));
    let b = std::env::temp_dir().join(format!("angel-lsp-root-b-{}", std::process::id()));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    let ctx = mock_ctx(a.clone());
    let first = ctx
        .pool
        .get_or_spawn(&ctx.servers[0], &ctx.resolve_root)
        .expect("warm server");

    let rebound = Arc::new(LspCtx {
        pool: Arc::clone(&ctx.pool),
        resolve_root: b.clone(),
        servers: ctx.servers.clone(),
        timeout: ctx.timeout,
        settle: ctx.settle,
    });
    let respawned = rebound
        .pool
        .get_or_spawn(&rebound.servers[0], &rebound.resolve_root)
        .expect("respawn at the new root");
    assert!(
        !Arc::ptr_eq(&first, &respawned),
        "a cross-root /cd must not reuse the stale analyzer"
    );
    assert_eq!(rebound.pool.state.lock().unwrap().clients.len(), 1);
    std::fs::remove_dir_all(a).ok();
    std::fs::remove_dir_all(b).ok();
}

#[test]
fn resolve_is_pinned_to_the_ctx_workspace_not_the_pool_index_root() {
    let root = std::env::temp_dir().join(format!("angel-lsp-resolve-{}", std::process::id()));
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("inside.rs"), "fn x() {}\n").unwrap();
    let ctx = mock_ctx(root.clone());
    // Spawn at the parent root so the pool's index root is `root`.
    let _ = ctx
        .pool
        .get_or_spawn(&ctx.servers[0], &ctx.resolve_root)
        .expect("warm server");
    let rebound = Arc::new(LspCtx {
        pool: Arc::clone(&ctx.pool),
        resolve_root: sub.clone(),
        servers: ctx.servers.clone(),
        timeout: ctx.timeout,
        settle: ctx.settle,
    });
    // Relative paths resolve against the NEW workspace, not the index root.
    let resolved = rebound
        .resolve("inside.rs")
        .expect("resolve in the new workspace");
    let sub_canon = sub.canonicalize().expect("sub workspace exists");
    assert!(
        resolved.starts_with(&sub_canon),
        "resolved={resolved:?} sub={sub_canon:?}"
    );
    // A sibling outside the new workspace is rejected even though it lies
    // inside the pool's index root — the boundary pins to the resolve root.
    std::fs::write(root.join("sibling.rs"), "fn y() {}\n").unwrap();
    assert!(
        rebound
            .resolve("../sibling.rs")
            .unwrap_err()
            .contains("outside the workspace"),
        "the boundary must follow the resolve root"
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn internal_warm_only_diagnostics_never_spawn_a_cold_server() {
    let dir = std::env::temp_dir().join(format!("angel-lsp-warm-only-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mock.rs"), "fn broken() {}\n").unwrap();
    let ctx = mock_ctx(dir.clone());
    let tool = LspDiagnosticsTool {
        ctx: Arc::clone(&ctx),
    };
    let error = tool
        .call(&json!({
            "path": "mock.rs",
            "_warm_only": true,
            "_deadline_ms": 50,
        }))
        .expect_err("cold internal call must skip instead of spawning");
    assert!(error.contains("not warm"), "got: {error}");
    assert!(ctx.pool.warm_clients().is_empty());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn request_timeout_override_is_a_real_deadline() {
    let client = LspClient::spawn(&mock_server(), Duration::from_secs(5)).expect("spawn");
    client.initialize(Path::new("/tmp")).expect("initialize");
    let started = Instant::now();
    let error = client
        .request_with_timeout(
            "angel/testNeverReplies",
            json!({}),
            Duration::from_millis(50),
        )
        .expect_err("mock deliberately ignores the method");
    assert!(error.contains("timed out"), "got: {error}");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "override must not inherit the five-second client timeout"
    );
}

/// An LspCtx whose only server is the sh mock (so pick_server routes `.rs` to
/// it), with the resolve root at `root`. Short settle so the cold
/// ensure_ready barrier is fast. The pool is a fresh local one so tests
/// never share warm state with each other.
fn mock_ctx(root: PathBuf) -> Arc<LspCtx> {
    Arc::new(LspCtx {
        pool: Arc::new(LspPool::new(Duration::from_secs(5))),
        resolve_root: root,
        servers: vec![mock_server()],
        timeout: Duration::from_secs(5),
        settle: Duration::from_millis(60),
    })
}

/// Navigation tools end-to-end over the mock: definition (single Location),
/// references (array), hover (markup) — exercises position resolution,
/// with_doc, and the formatters over real stdio pipes.
#[test]
fn mock_nav_definition_references_hover() {
    let dir = std::env::temp_dir().join(format!("angel-lsp-nav-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("mock.rs");
    std::fs::write(&f, "let x = 1;\nlet y = 2;\nfoo(x, y);\n").unwrap();
    let path = f.to_str().unwrap();
    let ctx = mock_ctx(dir.clone());

    let def = LspNavTool {
        kind: NavKind::Definition,
        ctx: Arc::clone(&ctx),
    };
    let d = def
        .call(&json!({ "path": path, "symbol": "foo" }))
        .expect("definition");
    assert!(
        d.contains("1 location(s)") && d.contains("mock.rs:3:4"),
        "got:\n{d}"
    );

    let refs = LspNavTool {
        kind: NavKind::References,
        ctx: Arc::clone(&ctx),
    };
    let r = refs
        .call(&json!({ "path": path, "line": 3, "character": 1 }))
        .expect("references");
    assert!(r.contains("2 location(s)"), "got:\n{r}");

    let hov = LspNavTool {
        kind: NavKind::Hover,
        ctx: Arc::clone(&ctx),
    };
    let h = hov
        .call(&json!({ "path": path, "symbol": "x" }))
        .expect("hover");
    assert_eq!(h, "fn foo() -> i32");

    let syms = LspSymbolsTool {
        ctx: Arc::clone(&ctx),
    };
    let s = syms.call(&json!({ "path": path })).expect("symbols");
    assert!(
        s.contains("2 symbol(s)") && s.contains("struct Foo") && s.contains("  fn bar"),
        "got:\n{s}"
    );

    // workspace/symbol over the now-warm mock client (no path to route by).
    let ws = LspWorkspaceSymbolTool { ctx };
    let w = ws
        .call(&json!({ "query": "Foo" }))
        .expect("workspace symbol");
    assert!(
        w.contains("1 match(es) for `Foo`") && w.contains("struct Foo  /mock.rs:3  (mymod)"),
        "got:\n{w}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn workspace_symbol_with_no_warm_server_is_a_friendly_note() {
    let ctx = mock_ctx(std::env::temp_dir());
    let ws = LspWorkspaceSymbolTool { ctx };
    // Nothing warmed the pool → a hint, not an error.
    let out = ws.call(&json!({ "query": "Foo" })).expect("ok");
    assert!(out.contains("no language server is warm"), "got:\n{out}");
}

#[test]
fn is_dead_client_only_flags_connection_failures() {
    assert!(is_dead_client("lsp rust closed the connection"));
    assert!(is_dead_client(
        "lsp rust: lsp write: Broken pipe (os error 32)"
    ));
    assert!(is_dead_client("lsp open_docs poisoned"));
    // logic / timeout / server errors are NOT dead-client
    assert!(!is_dead_client("tool error: symbol `x` not found in file"));
    assert!(!is_dead_client(
        "lsp rust timed out on textDocument/definition"
    ));
    assert!(!is_dead_client("lsp rust: unknown request"));
}

#[test]
fn tool_err_prefixes_once() {
    assert_eq!(tool_err("boom".into()), "tool error: boom");
    assert_eq!(tool_err("tool error: boom".into()), "tool error: boom"); // not doubled
}

/// A logic error (symbol not found) must surface cleanly AND leave the warm
/// server cached — re-indexing on a user mistake would be an own-goal.
#[test]
fn nav_logic_error_keeps_warm_server() {
    let dir = std::env::temp_dir().join(format!("angel-lsp-warm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("mock.rs");
    std::fs::write(&f, "let x = 1;\n").unwrap();
    let path = f.to_str().unwrap();
    let ctx = mock_ctx(dir.clone());
    let def = LspNavTool {
        kind: NavKind::Definition,
        ctx: Arc::clone(&ctx),
    };

    def.call(&json!({ "path": path, "symbol": "x" }))
        .expect("warm it");
    assert_eq!(ctx.pool.state.lock().unwrap().clients.len(), 1);

    let err = def
        .call(&json!({ "path": path, "symbol": "zzznotpresent" }))
        .unwrap_err();
    assert!(err.contains("not found"), "got: {err}");
    assert!(
        !err.contains("tool error: tool error"),
        "double prefix: {err}"
    );
    assert_eq!(
        ctx.pool.state.lock().unwrap().clients.len(),
        1,
        "a logic error must NOT evict the warm server"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// End-to-end against real rust-analyzer: a temp crate with a type error must
/// yield ≥1 error diagnostic. The SECOND call reuses the warm cached server
/// (didChange path) and must still see the (now-fixed) file as clean — proving
/// the pool + sync_doc reuse works. Slow + depends on the binary, so ignored
/// by default — run with `--ignored`.
#[test]
#[ignore = "spawns real rust-analyzer; slow. run with --ignored"]
fn live_rust_analyzer_reports_type_error() {
    let _guard = crate::tests::env_lock();
    reset_command_on_path_cache();
    assert!(
        command_on_path("rust-analyzer"),
        "rust-analyzer is not provisioned; the explicit live LSP contract cannot pass"
    );
    let dir = std::env::temp_dir().join(format!("angel-lsp-it-{}", std::process::id()));
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname=\"t\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    let main_rs = src.join("main.rs");
    // `let x: u32 = "s";` — a guaranteed type error.
    std::fs::write(&main_rs, "fn main() { let _x: u32 = \"s\"; }\n").unwrap();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LSP", "1") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LSP_TIMEOUT", "90") }; // headroom for cold indexing under load
    let ctx = Arc::new(LspCtx::new(dir.clone()));
    let diag = LspDiagnosticsTool {
        ctx: Arc::clone(&ctx),
    };
    let path = main_rs.to_str().unwrap();
    let arg = json!({ "path": path });

    // Cold call: waits for rust-analyzer to index, then sees the type error (push).
    let first = diag.call(&arg).expect("first run");
    assert!(
        first.contains("error"),
        "expected a type error, got:\n{first}"
    );

    // Fix the file and re-run: the second call REUSES the cached server
    // (didChange + pull, no respawn) and reports clean.
    std::fs::write(&main_rs, "fn main() { let _x: u32 = 1; }\n").unwrap();
    assert_eq!(
        ctx.pool.state.lock().unwrap().clients.len(),
        1,
        "server should be cached"
    );
    let second = diag.call(&arg).expect("second run");
    assert!(
        second.contains("clean"),
        "fixed file should be clean (warm pull), got:\n{second}"
    );

    // Navigation over the now-warm server: hover on the typed binding.
    let hover = LspNavTool {
        kind: NavKind::Hover,
        ctx: Arc::clone(&ctx),
    };
    let h = hover
        .call(&json!({ "path": path, "symbol": "_x" }))
        .expect("hover");
    assert!(h.contains("u32"), "hover should report the type, got:\n{h}");

    std::fs::remove_dir_all(&dir).ok();
}
