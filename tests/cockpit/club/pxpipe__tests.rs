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

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn oneshot_inherited_output_fixture() {
    use crate::agent::process_test_support::FixtureCleanup;
    use crate::agent::process_test_support::capture_evidence_fixture::EvidenceFixture;

    let root = std::env::var_os("ANGEL_T_PXPIPE_EOF")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    let _guard = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE", "1");
    let _models = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_MODELS", "gpt-5");
    let _persistent = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_PERSISTENT", "0");
    let _node = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_NODE", "/usr/bin/python3");
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_TIMEOUT_MS", "100");
    let original = br#"{"model":"gpt-5","input":"original request"}"#.to_vec();
    for stream in ["stdout", "stderr"] {
        for required in [false, true] {
            let mut fixture = EvidenceFixture::new(
                std::path::Path::new(&root),
                &format!("pxpipe-{required}"),
                stream,
                "transformed prefix",
            );
            let command = fixture.command();
            let _helper = crate::tests::TestEnvGuard::set(
                "ANGEL_PXPIPE_HELPER",
                command.get_program().to_str().unwrap(),
            );
            let _required = crate::tests::TestEnvGuard::set(
                "ANGEL_PXPIPE_REQUIRED",
                if required { "1" } else { "0" },
            );
            let started = Instant::now();
            let result =
                maybe_pxpipe_transform(PxpipeApi::Responses, "openai", "gpt-5", original.clone());
            let elapsed = started.elapsed();
            fixture.finish();
            eprintln!("pxpipe {stream} required={required}: budget=100ms elapsed={elapsed:?}");
            assert!(
                elapsed < Duration::from_millis(500),
                "{stream}: {elapsed:?}"
            );
            if required {
                let error = result.unwrap_err();
                assert!(error.contains("helper timed out after 100ms"), "{error}");
            } else {
                assert_eq!(
                    result.unwrap(),
                    original,
                    "{stream}: fail-open body changed"
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn oneshot_inherited_output_uses_one_deadline_and_preserves_fallback() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::oneshot_inherited_output_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_PXPIPE_EOF",
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn oneshot_blocked_stdin_fixture() {
    use crate::agent::process_test_support::FixtureCleanup;
    use crate::agent::process_test_support::capture_evidence_fixture::EvidenceFixture;

    let root = std::env::var_os("ANGEL_T_PXPIPE_STDIN")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    // The escaped holder inherits stdin but never reads it. A request larger
    // than the pipe capacity must time out while writing, even after exit0 of
    // the launcher and independently of the output readers' deadlines.
    let mut fixture =
        EvidenceFixture::new(std::path::Path::new(&root), "pxpipe-input", "stdout", "{}");
    let started = Instant::now();
    let result = run_pxpipe_one_shot(
        &mut fixture.command(),
        &vec![b'x'; 2 * 1024 * 1024],
        Duration::from_millis(100),
    );
    let elapsed = started.elapsed();
    fixture.finish();
    eprintln!("pxpipe blocked stdin: budget=100ms elapsed={elapsed:?}");
    assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");
    let error = result.unwrap_err();
    assert!(error.contains("helper timed out after 100ms"), "{error}");
}

#[cfg(target_os = "linux")]
#[test]
fn oneshot_blocked_stdin_shares_the_request_deadline() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::oneshot_blocked_stdin_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_PXPIPE_STDIN",
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn oneshot_complete_output_fixture() {
    use crate::agent::process_test_support::{FixtureCleanup, ServiceFixture};

    let root = std::env::var_os("ANGEL_T_PXPIPE_COMPLETE")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    let mut command = Command::new("/usr/bin/python3");
    command.args([
        "-c",
        "import sys; sys.stdout.buffer.write(b'x' * (2 * 1024 * 1024)); sys.stdout.buffer.flush(); request = sys.stdin.buffer.read(); sys.stdout.buffer.write(request)",
    ]);
    let request = vec![b'y'; 1024 * 1024 + 17];
    let output = run_pxpipe_one_shot(&mut command, &request, Duration::from_secs(2)).unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.len(), 2 * 1024 * 1024 + request.len());
    assert!(output.stdout[..2 * 1024 * 1024].iter().all(|b| *b == b'x'));
    assert_eq!(&output.stdout[2 * 1024 * 1024..], request);

    // A finite launcher can leave a same-group wrapper holding the pipes.
    // Retirement closes those pipes without erasing the launcher's exit0.
    let marker = std::path::Path::new(&root).join("pxpipe-wrapper.pid");
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "sleep 30 & printf '%s' \"$!\" > \"$1\"; printf transformed",
        ])
        .arg("pxpipe-wrapper")
        .arg(&marker);
    let started = Instant::now();
    let output = run_pxpipe_one_shot(&mut command, b"", Duration::from_millis(200)).unwrap();
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(output.status.success());
    assert_eq!(output.stdout, b"transformed");
    let descendant = std::fs::read_to_string(marker).unwrap().parse().unwrap();
    ServiceFixture::assert_reaped(descendant);

    let _guard = crate::tests::env_lock();
    let helper = std::path::Path::new(&root).join("pxpipe-error.py");
    std::fs::write(
        &helper,
        "import sys\nsys.stdin.buffer.read()\nsys.stderr.write('  transform rejected  \\n')\nsys.exit(17)\n",
    )
    .unwrap();
    let _helper = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_HELPER", helper.to_str().unwrap());
    let _node = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_NODE", "/usr/bin/python3");
    let _persistent = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_PERSISTENT", "0");
    let error = run_pxpipe_helper(PxpipeApi::Responses, "openai", "gpt-5", b"{}").unwrap_err();
    assert!(error.contains("exit status: 17"), "{error}");
    assert!(error.ends_with(": transform rejected"), "{error}");
}

#[cfg(target_os = "linux")]
#[test]
fn oneshot_preserves_large_complete_bytes_status_and_owned_wrapper_cleanup() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::oneshot_complete_output_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_PXPIPE_COMPLETE",
    );
}
