use super::*;
/// One owned loopback server: response remains open until the client closes.
fn fixture(progressing: bool, headers: bool) -> (String, std::thread::JoinHandle<()>) {
    fixture_response(
        progressing,
        headers.then_some(&b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nseed"[..]),
    )
}
pub(crate) fn error_fixture() -> (String, std::thread::JoinHandle<()>) {
    fixture_response(
        false,
        Some(b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\nseed"),
    )
}
fn fixture_response(
    progressing: bool,
    response: Option<&'static [u8]>,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/fixture", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        if let Some(response) = response {
            // Deliberately no length, chunked encoding, or Connection: close.
            stream.write_all(response).unwrap();
        }
        if progressing {
            for _ in 0..12 {
                std::thread::sleep(Duration::from_millis(100));
                if stream.write_all(b"x").is_err() {
                    return;
                }
            }
        } else {
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte).unwrap(),
                0,
                "helper must close the socket"
            );
        }
    });
    (url, worker)
}
fn test_context(stall_ms: u64, deadline_ms: Option<u64>) -> Context {
    Context {
        deadline: deadline_ms.map(|ms| Instant::now() + Duration::from_millis(ms)),
        stall: Duration::from_millis(stall_ms),
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
fn stalled(error: &str, bound: &str, bytes: usize) {
    let (_, receipt) = error.split_once("stalled ").expect(error);
    let receipt: serde_json::Value = serde_json::from_str(receipt).unwrap();
    assert_eq!(receipt["bound"], bound);
    assert_eq!(receipt["bytes_received"], bytes);
    assert_eq!(
        receipt["partial_body"],
        if bytes == 0 { "" } else { "seed" }
    );
    assert_eq!(
        crate::harness::tool_errors::classify_tool_error(
            "web_fetch",
            &serde_json::json!({}),
            Some(error)
        ),
        crate::harness::tool_errors::ToolErrorClass::Transient
    );
    println!("HTTP partial receipt: {receipt}");
}
#[test]
fn hung_body_stalls_and_closes_socket() {
    let _lock = crate::tests::env_lock();
    let (url, server) = fixture(false, true);
    let start = Instant::now();
    let error = with_context(test_context(350, None), || {
        request("GET", &url, false, 0)
            .call()
            .unwrap()
            .into_string()
            .unwrap_err()
            .to_string()
    });
    server.join().unwrap();
    stalled(&error, "stall", 4);
    assert!(start.elapsed() < Duration::from_secs(2));
}
#[test]
fn body_progress_resets_stall_without_total_cap() {
    let _lock = crate::tests::env_lock();
    let (url, server) = fixture(true, true);
    let start = Instant::now();
    let body = with_context(test_context(350, None), || {
        request("GET", &url, false, 0)
            .call()
            .unwrap()
            .into_string()
            .unwrap()
    });
    server.join().unwrap();
    assert_eq!(body, "seedxxxxxxxxxxxx");
    assert!(start.elapsed() >= Duration::from_millis(1200));
}
#[test]
fn turn_deadline_interrupts_body_even_in_yolo() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let (url, server) = fixture(false, true);
    let start = Instant::now();
    let error = with_context(test_context(4000, Some(350)), || {
        request("GET", &url, true, 0)
            .call()
            .unwrap()
            .into_string()
            .unwrap_err()
            .to_string()
    });
    server.join().unwrap();
    stalled(&error, "deadline", 4);
    assert!(start.elapsed() < Duration::from_secs(2));
}
#[test]
fn default_tool_cancel_interrupts_read_and_retains_partial() {
    use crate::harness::Tool;
    let _lock = crate::tests::env_lock();
    let (url, server) = fixture(false, true);
    let cancel = AtomicBool::new(false);
    let start = Instant::now();
    let error = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(350));
            cancel.store(true, Ordering::Release);
        });
        with_context(test_context(4000, None), || {
            super::super::web::WebFetchTool
                .call_with_cancel(&serde_json::json!({"url": url}), Some(&cancel))
                .unwrap_err()
        })
    });
    server.join().unwrap();
    stalled(&error, "cancel", 4);
    assert!(start.elapsed() < Duration::from_secs(2));
}
#[test]
fn deadline_interrupts_headers() {
    let _lock = crate::tests::env_lock();
    let (url, server) = fixture(false, false);
    let error = with_context(test_context(4000, Some(350)), || {
        match request("GET", &url, false, 0).call() {
            Ok(_) => panic!("headers never arrived"),
            Err(error) => error.to_string(),
        }
    });
    server.join().unwrap();
    stalled(&error, "deadline", 0);
}
#[test]
fn tool_timeout_never_extends_parent_deadline() {
    let outer = Instant::now() + Duration::from_millis(100);
    let mut ctx = context();
    ctx.deadline = Some(outer);
    with_context(ctx, || {
        with_timeout(Duration::from_secs(5), || {
            assert_eq!(context().deadline, Some(outer));
        })
    });
}
#[test]
fn http_child_entry() {
    if std::env::var_os("ANGEL_T_HTTP_CHILD").is_some() {
        let code = if helper_main().is_ok() { 0 } else { 1 };
        std::process::exit(code);
    }
}
