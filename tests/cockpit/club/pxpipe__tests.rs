use super::*;

#[test]
fn pxpipe_is_disabled_by_default() {
    let _guard = crate::tests::env_lock();
    let saved = std::env::var_os("ANGEL_PXPIPE");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_PXPIPE") };

    assert!(!pxpipe_enabled());

    match saved {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var("ANGEL_PXPIPE", value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_PXPIPE") },
    }
}

#[cfg(unix)]
#[test]
fn persistent_roundtrip_hang_is_killed_at_the_request_deadline() {
    use std::os::unix::process::CommandExt as _;

    let _guard = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_TIMEOUT_MS", "50");
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "sleep 30 & wait"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = command.spawn_owned().unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut daemon = PxpipeDaemon {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    };
    let started = std::time::Instant::now();

    let error = pxpipe_daemon_roundtrip(
        &mut daemon,
        PxpipeApi::Responses,
        "openai",
        "gpt-5.5",
        br#"{"model":"gpt-5.5"}"#,
    )
    .unwrap_err();

    assert!(error.contains("timed out after 50ms"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "persistent pxpipe daemon outlived its request deadline: {:?}",
        started.elapsed()
    );
    let _ = daemon.child.wait();
}
