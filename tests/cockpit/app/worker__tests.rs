use super::*;

#[cfg(unix)]
fn shell(script: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(script);
    command
}

#[test]
#[cfg(unix)]
fn exchanges_large_request_without_pipe_deadlock() {
    let _lock = crate::tests::env_lock();
    let input = vec![b'x'; 1024 * 1024];
    let output = run(
        shell("cat"),
        &input,
        Duration::from_secs(5),
        input.len(),
        input.len(),
    )
    .unwrap();
    assert_eq!(output, input);
}

#[test]
#[cfg(unix)]
fn drains_output_while_sending_request() {
    let _lock = crate::tests::env_lock();
    // The child fills stdout before reading stdin. Sequential write-then-read
    // supervision would deadlock when both payloads exceed pipe capacity.
    let input = vec![b'x'; 1024 * 1024];
    let output = run(
        shell("head -c 1048576 /dev/zero; cat"),
        &input,
        Duration::from_secs(5),
        input.len(),
        input.len() * 2,
    )
    .unwrap();
    assert_eq!(output.len(), input.len() * 2);
    assert!(output[..input.len()].iter().all(|byte| *byte == 0));
    assert_eq!(&output[input.len()..], input);
}

#[test]
#[cfg(unix)]
fn node_worker_receives_stdin_and_returns_json() {
    let _lock = crate::tests::env_lock();
    let mut node = Command::new("node");
    node.arg("-e").arg("const fs = require('fs'); const request = JSON.parse(fs.readFileSync(0, 'utf8')); fs.writeFileSync(1, JSON.stringify(request));");
    let input = br#"{"query":"research","count":3}"#;
    let output = run(node, input, Duration::from_secs(5), 1024, 1024).unwrap();
    assert_eq!(output, input);
}

#[test]
#[cfg(unix)]
fn rejects_limits_and_failing_children() {
    let _lock = crate::tests::env_lock();
    assert!(
        run(shell("exit 0"), b"a", Duration::from_secs(1), 0, 10)
            .unwrap_err()
            .contains("input")
    );
    assert!(
        run(shell("printf 12345"), b"", Duration::from_secs(1), 0, 4)
            .unwrap_err()
            .contains("output")
    );
    assert!(
        run(shell("exit 7"), b"", Duration::from_secs(1), 0, 10)
            .unwrap_err()
            .contains("exited")
    );
}

#[test]
#[cfg(unix)]
fn timeout_kills_descendants_and_never_returns_partial_output() {
    let _lock = crate::tests::env_lock();
    let dir = std::env::temp_dir().join(format!("angel-worker-timeout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("survived");
    let script = format!(
        "(sleep 1; touch '{}') & printf partial; wait",
        marker.display()
    );
    let error = run(shell(&script), b"", Duration::from_millis(150), 0, 100).unwrap_err();
    assert!(error.contains("timed out"), "{error}");
    std::thread::sleep(Duration::from_millis(1100));
    assert!(!marker.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(unix)]
fn successful_worker_stops_descendants_that_closed_output() {
    let _lock = crate::tests::env_lock();
    let dir = crate::tests::TestGitWorkspace::new("worker-success-descendant");
    let marker = dir.path().join("survived");
    let script = format!(
        "(sleep 1; touch '{}') >/dev/null 2>&1 & printf complete; exit 0",
        marker.display()
    );
    let output = run(shell(&script), b"", Duration::from_secs(3), 0, 100).unwrap();
    assert_eq!(output, b"complete");
    std::thread::sleep(Duration::from_millis(1100));
    assert!(!marker.exists(), "a worker descendant survived success");
}

#[test]
#[cfg(unix)]
fn worker_waits_for_descendant_to_finish_output() {
    let _lock = crate::tests::env_lock();
    // The leader exits first, but its helper finishes the response. Treating
    // leader exit as completion would silently return a truncated result.
    let output = run(
        shell("printf hello; (sleep 0.1; printf ' world') & exit 0"),
        b"",
        Duration::from_secs(2),
        0,
        100,
    )
    .unwrap();
    assert_eq!(output, b"hello world");
}

#[test]
#[cfg(unix)]
fn exited_worker_with_stalled_output_times_out_without_partial_response() {
    let _lock = crate::tests::env_lock();
    let error = run(
        shell("(sleep 5) & printf partial; exit 0"),
        b"",
        Duration::from_millis(150),
        0,
        100,
    )
    .unwrap_err();
    assert!(error.contains("timed out"), "{error}");
}

#[test]
#[cfg(unix)]
fn failed_worker_does_not_wait_for_descendant_output() {
    let _lock = crate::tests::env_lock();
    let started = std::time::Instant::now();
    let error = run(
        shell("(sleep 5) & printf partial; exit 7"),
        b"",
        Duration::from_secs(2),
        0,
        100,
    )
    .unwrap_err();
    assert!(error.contains("exited"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
#[cfg(unix)]
fn cancellation_stops_an_inflight_worker() {
    let _lock = crate::tests::env_lock();
    use std::sync::atomic::{AtomicBool, Ordering};
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            cancel.store(true, Ordering::Release);
        });
        let started = std::time::Instant::now();
        let result = run_cancellable(shell("sleep 5"), b"", Duration::from_secs(5), 0, 10, || {
            cancel.load(Ordering::Acquire)
        });
        assert!(result.unwrap_err().contains("interrupted"));
        assert!(started.elapsed() < Duration::from_secs(2));
    });
}
