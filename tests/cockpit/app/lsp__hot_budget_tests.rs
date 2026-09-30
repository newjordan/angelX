use super::*;

struct HotFixture {
    root: PathBuf,
    trace: PathBuf,
    server: LspServer,
    client: Arc<LspClient>,
    ctx: Arc<LspCtx>,
}

impl HotFixture {
    fn new(tag: &str, mode: &str, reply_delay_ms: u64) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-lsp-hot-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("queued.rs"), "fn queued() {}\n").unwrap();
        let trace = root.join("frames.jsonl");
        let server = LspServer {
            name: "hot-budget".into(),
            command: "python3".into(),
            args: vec![
                "-c".into(),
                r#"import json, os, sys, time
trace, mode, delay = sys.argv[1], sys.argv[2], int(sys.argv[3]) / 1000
def send(value):
    body = json.dumps(value).encode()
    sys.stdout.buffer.write(f'Content-Length: {len(body)}\r\n\r\n'.encode() + body)
    sys.stdout.buffer.flush()
while True:
    size = None
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            sys.exit(0)
        if line in (b'\r\n', b'\n'):
            break
        if line.startswith(b'Content-Length:'):
            size = int(line.split(b':', 1)[1])
    message = json.loads(sys.stdin.buffer.read(size))
    method = message.get('method')
    if method == 'initialize':
        capabilities = {'diagnosticProvider': {}} if mode == 'pull-push' else {}
        send({'id': message['id'], 'result': {'capabilities': capabilities}})
    elif method in ('textDocument/didOpen', 'textDocument/didChange'):
        document = message['params']['textDocument']
        with open(trace, 'a') as output:
            output.write(json.dumps({'method': method, 'version': document['version']}) + '\n')
        time.sleep(delay)
        send({'method': 'textDocument/publishDiagnostics', 'params': {'uri': document['uri'], 'diagnostics': []}})
        if mode == 'stop-after-open':
            time.sleep(30)
        elif mode in ('reply-stall', 'stdout-eof'):
            while not os.path.exists(trace + '.trigger'):
                time.sleep(.005)
            if mode == 'reply-stall':
                send({'id': 'x' * (1024 * 1024), 'method': 'window/workDoneProgress/create', 'params': {}})
            else:
                os.close(1)
            time.sleep(30)
    elif method == 'textDocument/diagnostic':
        uri = message['params']['textDocument']['uri']
        send({'method': 'textDocument/publishDiagnostics', 'params': {'uri': uri, 'diagnostics': [{'message': 'deferred evidence'}]}})
        send({'id': message['id'], 'result': {'kind': 'full', 'items': []}})
"#
                .into(),
                trace.to_string_lossy().into_owned(),
                mode.into(),
                reply_delay_ms.to_string(),
            ],
            extensions: vec!["rs".into()],
            language_id: "rust".into(),
        };
        let timeout = Duration::from_millis(800);
        let pool = Arc::new(LspPool::new(timeout));
        let client = pool.get_or_spawn(&server, &root).unwrap();
        let ctx = Arc::new(LspCtx {
            pool,
            resolve_root: root.clone(),
            servers: vec![server.clone()],
            timeout,
            settle: Duration::from_millis(5),
        });
        Self {
            root,
            trace,
            server,
            client,
            ctx,
        }
    }

    fn tool(&self) -> LspDiagnosticsTool {
        LspDiagnosticsTool {
            ctx: Arc::clone(&self.ctx),
        }
    }

    fn uri(&self) -> String {
        path_to_uri(&self.root.join("queued.rs"))
    }

    fn frames(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.trace)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn assert_warm(&self) {
        let warm = self.ctx.pool.get_warm(&self.server, &self.root).unwrap();
        assert!(Arc::ptr_eq(&warm, &self.client));
        assert!(self.client.stdin.lock().unwrap().is_some());
    }
}

impl Drop for HotFixture {
    fn drop(&mut self) {
        self.ctx.pool.evict(&self.server.name);
        let _ = self.client.child.lock().unwrap().retire();
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn hot_args() -> Value {
    json!({"path": "queued.rs", "_warm_only": true, "_deadline_ms": 100})
}

#[test]
fn blocked_document_write_uses_hot_budget_without_advancing_version() {
    let _lock = crate::tests::env_lock();
    let fixture = HotFixture::new("blocked-write", "stop-after-open", 0);
    assert!(
        fixture
            .tool()
            .call(&json!({"path": "queued.rs"}))
            .unwrap()
            .contains("clean")
    );
    let uri = fixture.uri();
    let original = fixture.client.open_text.lock().unwrap()[&uri].clone();
    assert_eq!(fixture.client.open_docs.lock().unwrap()[&uri], 1);
    std::fs::write(fixture.root.join("queued.rs"), "x".repeat(1024 * 1024)).unwrap();
    let started = Instant::now();
    let result = fixture.tool().call(&hot_args());
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(400),
        "{elapsed:?}: {result:?}"
    );
    assert!(result.unwrap_err().contains("lsp write: timed out"));
    assert_eq!(fixture.client.open_docs.lock().unwrap()[&uri], 1);
    assert_eq!(fixture.client.open_text.lock().unwrap()[&uri], original);
    assert!(fixture.client.stdin.lock().unwrap().is_none());
    assert!(
        fixture
            .ctx
            .pool
            .get_warm(&fixture.server, &fixture.root)
            .is_none()
    );
    assert_eq!(fixture.frames().len(), 1, "no complete didChange arrived");
}

fn assert_unwritten_queue_expiry<T>(fixture: &HotFixture, held: T) {
    let tool = fixture.tool();
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        let _ = sender.send(tool.call(&hot_args()));
    });
    let observed = receiver.recv_timeout(Duration::from_millis(500));
    let completed = observed.is_ok();
    let elapsed = started.elapsed();
    drop(held); // Every regression path releases its lock before join/assert.
    let result =
        observed.unwrap_or_else(|_| receiver.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
    assert!(completed, "warm queue escaped its budget: {elapsed:?}");
    assert!(elapsed < Duration::from_millis(400));
    assert!(result.unwrap_err().contains("timed out waiting"));
    assert!(fixture.frames().is_empty());
    let uri = fixture.uri();
    assert!(!fixture.client.open_docs.lock().unwrap().contains_key(&uri));
    assert!(!fixture.client.open_text.lock().unwrap().contains_key(&uri));
    assert_eq!(
        fixture.client.diagnostics.lock().unwrap()[&uri],
        json!({"uri": uri, "diagnostics": [{"message": "cached evidence"}]})
    );
    fixture.assert_warm();
    assert!(
        fixture
            .tool()
            .call(&json!({"path": "queued.rs"}))
            .unwrap()
            .contains("clean")
    );
    assert_eq!(
        fixture.frames(),
        vec![json!({"method": "textDocument/didOpen", "version": 1})]
    );
}

#[test]
fn warm_admission_document_and_stdin_queues_expire_without_frames_or_cache_changes() {
    let _lock = crate::tests::env_lock();
    for stage in ["admission", "document", "stdin"] {
        let fixture = HotFixture::new(stage, "normal", 0);
        let uri = fixture.uri();
        fixture.client.diagnostics.lock().unwrap().insert(
            uri.clone(),
            json!({"uri": uri, "diagnostics": [{"message": "cached evidence"}]}),
        );
        match stage {
            "admission" => {
                assert_unwritten_queue_expiry(&fixture, fixture.ctx.pool.state.lock().unwrap())
            }
            "document" => {
                assert_unwritten_queue_expiry(&fixture, fixture.client.open_text.lock().unwrap())
            }
            "stdin" => {
                assert_unwritten_queue_expiry(&fixture, fixture.client.stdin.lock().unwrap())
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn synchronization_and_diagnostic_response_share_one_hot_budget() {
    let _lock = crate::tests::env_lock();
    let fixture = HotFixture::new("shared-budget", "normal", 80);
    let held = fixture.client.stdin.lock().unwrap();
    let tool = fixture.tool();
    let (ready, started_rx) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        let _ = ready.send(());
        let _ = sender.send(tool.call(&hot_args()));
    });
    started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    std::thread::sleep(Duration::from_millis(70));
    drop(held);
    let result = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.join().unwrap();
    assert!(started.elapsed() < Duration::from_millis(400));
    assert!(result.unwrap().contains("no diagnostics within 100ms"));
    fixture.assert_warm();
    assert_eq!(fixture.client.open_docs.lock().unwrap()[&fixture.uri()], 1);
    assert!(
        fixture
            .tool()
            .call(&json!({"path": "queued.rs"}))
            .unwrap()
            .contains("clean")
    );
    assert_eq!(
        fixture.frames().len(),
        1,
        "completed didOpen is not repeated"
    );
}

fn finish_held_call<T>(tool: LspDiagnosticsTool, held: T) -> Result<String, String> {
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        let _ = sender.send(tool.call(&hot_args()));
    });
    let observed = receiver.recv_timeout(Duration::from_millis(500));
    let completed = observed.is_ok();
    let elapsed = started.elapsed();
    drop(held);
    let result =
        observed.unwrap_or_else(|_| receiver.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
    assert!(
        completed && elapsed < Duration::from_millis(400),
        "{elapsed:?}: {result:?}"
    );
    result
}

#[test]
fn unchanged_document_cache_queue_uses_hot_budget_and_keeps_cached_evidence() {
    let _lock = crate::tests::env_lock();
    let fixture = HotFixture::new("cache-queue", "normal", 0);
    fixture.tool().call(&json!({"path": "queued.rs"})).unwrap();
    let uri = fixture.uri();
    let held = fixture.client.diagnostics.lock().unwrap();
    let original = held[&uri].clone();
    let result = finish_held_call(fixture.tool(), held).unwrap();
    assert!(result.contains("no diagnostics within 100ms"), "{result}");
    assert_eq!(fixture.client.diagnostics.lock().unwrap()[&uri], original);
    assert_eq!(fixture.frames().len(), 1);
    fixture.assert_warm();
    assert!(
        fixture
            .tool()
            .call(&json!({"path": "queued.rs"}))
            .unwrap()
            .contains("clean")
    );
}

#[test]
fn cache_queue_during_pull_preserves_unreceived_push_and_client_reuse() {
    let _lock = crate::tests::env_lock();
    let fixture = HotFixture::new("remember-queue", "pull-push", 0);
    fixture.tool().call(&json!({"path": "queued.rs"})).unwrap();
    let held_stdin = fixture.client.stdin.lock().unwrap();
    let tool = fixture.tool();
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        let _ = sender.send(tool.call(&hot_args()));
    });
    let cap = Instant::now() + Duration::from_millis(80);
    let queued = loop {
        if fixture.client.rx.try_lock().is_err() {
            break true;
        }
        if Instant::now() >= cap {
            break false;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    // The request owns rx while queued for stdin, so the cache lock can be
    // acquired before its first received notification without a timing race.
    let held_cache = fixture.client.diagnostics.lock().unwrap();
    drop(held_stdin);
    let observed = receiver.recv_timeout(Duration::from_millis(500));
    let completed = observed.is_ok();
    let elapsed = started.elapsed();
    drop(held_cache);
    let result =
        observed.unwrap_or_else(|_| receiver.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
    assert!(
        queued && completed && elapsed < Duration::from_millis(400),
        "{elapsed:?}: {result:?}"
    );
    assert!(
        result
            .unwrap_err()
            .contains("timed out waiting for diagnostics cache")
    );
    fixture.assert_warm();
    // Reuse the ordinary push collector: the previous request's notification
    // was never consumed and still grounds a later call.
    fixture.client.supports_pull.store(false, Ordering::Relaxed);
    let retry = fixture.tool().call(&json!({"path": "queued.rs"})).unwrap();
    assert!(retry.contains("deferred evidence"), "{retry}");
    assert_eq!(fixture.frames().len(), 1);
    assert!(
        fixture.client.diagnostics.lock().unwrap()[&fixture.uri()]
            .to_string()
            .contains("deferred evidence")
    );
}

#[test]
fn failed_write_pool_queue_is_bounded_and_explicit_retry_replaces_invalid_client() {
    let _lock = crate::tests::env_lock();
    let fixture = HotFixture::new("eviction-queue", "stop-after-open", 0);
    fixture.tool().call(&json!({"path": "queued.rs"})).unwrap();
    std::fs::write(fixture.root.join("queued.rs"), "x".repeat(1024 * 1024)).unwrap();
    let tool = fixture.tool();
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        let _ = sender.send(tool.call(&hot_args()));
    });
    let cap = Instant::now() + Duration::from_millis(80);
    let admitted = loop {
        if fixture.client.open_docs.try_lock().is_err() {
            break true;
        }
        if Instant::now() >= cap {
            break false;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    let held = fixture.ctx.pool.state.lock().unwrap();
    let observed = receiver.recv_timeout(Duration::from_millis(500));
    let completed = observed.is_ok();
    let elapsed = started.elapsed();
    let retained = held
        .clients
        .get(&fixture.server.name)
        .map(|cached| Arc::clone(&cached.client));
    drop(held);
    let result =
        observed.unwrap_or_else(|_| receiver.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
    assert!(
        admitted && completed && elapsed < Duration::from_millis(400),
        "{elapsed:?}: {result:?}"
    );
    assert!(result.unwrap_err().contains("lsp write: timed out"));
    assert!(Arc::ptr_eq(&retained.unwrap(), &fixture.client));
    assert!(fixture.client.unusable.load(Ordering::Relaxed));
    assert!(fixture.client.stdin.lock().unwrap().is_none());
    assert!(
        fixture
            .client
            .request("must-not-write", json!({}))
            .unwrap_err()
            .contains("closed the connection")
    );
    assert!(fixture.ctx.pool.warm_clients().is_empty());

    // An explicit call replaces the invalid record, opens version1 on the
    // fresh process, and does not inherit the failed frame/document version.
    std::fs::write(fixture.root.join("queued.rs"), "fn retried() {}\n").unwrap();
    assert!(
        fixture
            .tool()
            .call(&json!({"path": "queued.rs"}))
            .unwrap()
            .contains("clean")
    );
    let replacement = fixture
        .ctx
        .pool
        .get_warm(&fixture.server, &fixture.root)
        .unwrap();
    assert!(!Arc::ptr_eq(&replacement, &fixture.client));
    assert_eq!(replacement.open_docs.lock().unwrap()[&fixture.uri()], 1);
    assert_eq!(
        fixture.frames(),
        vec![
            json!({"method": "textDocument/didOpen", "version": 1}),
            json!({"method": "textDocument/didOpen", "version": 1})
        ]
    );
    // Late cleanup of the old caller must not evict this replacement.
    fixture
        .ctx
        .pool
        .evict_client_until(&fixture.server.name, &fixture.client, Some(Instant::now()))
        .unwrap();
    assert!(Arc::ptr_eq(
        &fixture
            .ctx
            .pool
            .get_warm(&fixture.server, &fixture.root)
            .unwrap(),
        &replacement
    ));
}

#[test]
fn fifo_document_is_rejected_without_waiting_for_a_writer_or_touching_the_client() {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let _lock = crate::tests::env_lock();
    for warm in [false, true] {
        let fixture = HotFixture::new("fifo", "normal", 0);
        if !warm {
            fixture.ctx.pool.evict(&fixture.server.name);
        }
        let path = fixture.root.join("queued.rs");
        std::fs::remove_file(&path).unwrap();
        let raw = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: the owned fixture pathname is NUL terminated and absent.
        assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
        let tool = fixture.tool();
        let (sender, receiver) = mpsc::channel();
        let started = Instant::now();
        let worker = std::thread::spawn(move || {
            let _ = sender.send(tool.call(&hot_args()));
        });
        let observed = receiver.recv_timeout(Duration::from_millis(500));
        let completed = observed.is_ok();
        let elapsed = started.elapsed();
        if !completed {
            // Release the old blocking read on a red regression path before
            // join/assertion; no helper process or open pipe is left behind.
            let mut writer = std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&path)
                .unwrap();
            writer.write_all(b"fn released() {}\n").unwrap();
        }
        let result =
            observed.unwrap_or_else(|_| receiver.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
        assert!(
            completed && elapsed < Duration::from_millis(400),
            "{elapsed:?}: {result:?}"
        );
        assert!(result.unwrap_err().contains("regular file"));
        assert!(fixture.frames().is_empty());
        assert!(fixture.client.open_docs.lock().unwrap().is_empty());
        if warm {
            fixture.assert_warm();
        } else {
            assert!(fixture.ctx.pool.warm_clients().is_empty());
        }
    }
}

#[test]
fn regular_document_aliases_work_and_nontext_or_directory_reads_preserve_sync_state() {
    let _lock = crate::tests::env_lock();
    let fixture = HotFixture::new("document-read", "normal", 0);
    let alias = fixture.root.join("alias.rs");
    std::os::unix::fs::symlink("queued.rs", &alias).unwrap();
    assert!(
        fixture
            .tool()
            .call(&json!({"path": "alias.rs"}))
            .unwrap()
            .contains("clean")
    );
    let uri = fixture.uri();
    let original = fixture.client.open_text.lock().unwrap()[&uri].clone();
    std::fs::write(fixture.root.join("queued.rs"), [0xff, 0xfe]).unwrap();
    assert!(
        fixture
            .tool()
            .call(&hot_args())
            .unwrap_err()
            .contains("UTF-8")
    );
    let directory = fixture.root.join("directory.rs");
    std::fs::create_dir(&directory).unwrap();
    assert!(
        fixture
            .tool()
            .call(&json!({"path": "directory.rs"}))
            .unwrap_err()
            .contains("regular file")
    );
    assert_eq!(fixture.client.open_docs.lock().unwrap()[&uri], 1);
    assert_eq!(fixture.client.open_text.lock().unwrap()[&uri], original);
    assert_eq!(fixture.frames().len(), 1);
    fixture.assert_warm();
}

#[test]
fn reader_failure_invalidates_cached_clean_reports_and_explicit_retry_opens_a_new_client() {
    let _lock = crate::tests::env_lock();
    for mode in ["reply-stall", "stdout-eof"] {
        let fixture = HotFixture::new(mode, mode, 0);
        assert!(
            fixture
                .tool()
                .call(&json!({"path": "queued.rs"}))
                .unwrap()
                .contains("clean")
        );
        // A healthy, unchanged document reuses its complete cached evidence.
        assert!(fixture.tool().call(&hot_args()).unwrap().contains("clean"));
        fixture.assert_warm();
        let trigger = PathBuf::from(format!("{}.trigger", fixture.trace.display()));
        std::fs::write(&trigger, "go").unwrap();
        let cap = Instant::now() + Duration::from_secs(2);
        while !fixture.client.unusable.load(Ordering::Acquire) && Instant::now() < cap {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            fixture.client.unusable.load(Ordering::Acquire),
            "reader did not finish: {mode}"
        );
        // The service intentionally stays alive; transport health must not be
        // inferred from the direct child's liveness or a previous clean push.
        assert!(fixture.client.child.lock().unwrap().alive());
        let uri = fixture.uri();
        assert!(
            fixture
                .client
                .diagnostics
                .lock()
                .unwrap()
                .contains_key(&uri)
        );
        assert!(
            fixture
                .client
                .sync_doc(&uri, "rust", "fn queued() {}\n")
                .unwrap_err()
                .contains("closed the connection")
        );
        assert!(
            fixture
                .client
                .collect_diagnostics_with_timeout(
                    &uri,
                    Duration::from_millis(100),
                    Duration::from_millis(5)
                )
                .is_none()
        );
        assert!(fixture.ctx.pool.warm_clients().is_empty());
        let hot = fixture.tool().call(&hot_args()).unwrap_err();
        assert!(hot.contains("not warm"), "{mode}: {hot}");
        assert_eq!(
            fixture.frames().len(),
            1,
            "no extra frame follows reader failure"
        );
        if mode == "reply-stall" {
            assert!(fixture.client.stdin.lock().unwrap().is_none());
        }

        std::fs::remove_file(&trigger).unwrap();
        assert!(
            fixture
                .tool()
                .call(&json!({"path": "queued.rs"}))
                .unwrap()
                .contains("clean")
        );
        let replacement = fixture
            .ctx
            .pool
            .get_warm(&fixture.server, &fixture.root)
            .unwrap();
        assert!(!Arc::ptr_eq(&replacement, &fixture.client));
        assert_eq!(replacement.open_docs.lock().unwrap()[&uri], 1);
        assert_eq!(
            fixture.frames(),
            vec![
                json!({"method": "textDocument/didOpen", "version": 1}),
                json!({"method": "textDocument/didOpen", "version": 1})
            ]
        );
    }
}
