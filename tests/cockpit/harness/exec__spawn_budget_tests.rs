use super::*;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

/// Atomically publish a real executable after the caller's first spawn misses
/// it. No partially written executable or executable-file-busy race is needed.
fn publish_after(
    root: &Path,
    label: &str,
    delay: Duration,
    script: &str,
) -> (PathBuf, std::thread::JoinHandle<()>) {
    let program = root.join(format!("{label}.sh"));
    let staged = root.join(format!("{label}.staged"));
    std::fs::write(&staged, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o700)).unwrap();
    let published = program.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(delay);
        std::fs::rename(staged, published).unwrap();
    });
    (program, worker)
}

#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn fixed_spawn_budget_fixture() {
    use crate::agent::process_test_support::{FixtureCleanup, ServiceFixture};

    let root = std::env::var_os("ANGEL_T_FIXED_SPAWN")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    let _guard = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_SECS", "0");
    let _floor = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "0");
    let _grace = crate::tests::TestEnvGuard::set("ANGEL_TOOL_KILL_GRACE_MS", "0");
    let root = PathBuf::from(root);

    let started = Instant::now();
    let error = output_timed_fixed_captured(
        Command::new(root.join("absent-image")),
        Duration::from_millis(50),
    )
    .unwrap_err();
    let elapsed = started.elapsed();
    eprintln!("fixed missing image: budget=50ms elapsed={elapsed:?} error={error}");
    assert!(error.starts_with("spawn failed:"), "{error}");
    assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");

    let marker = root.join("late-launch.pid");
    let (program, publisher) = publish_after(
        &root,
        "late-image",
        Duration::from_millis(150),
        "printf '%s' \"$$\" > \"$1\"; printf late",
    );
    let mut command = Command::new(program);
    command.arg(&marker);
    let started = Instant::now();
    let result = output_timed_fixed_captured(command, Duration::from_millis(50));
    let elapsed = started.elapsed();
    publisher.join().unwrap();
    assert!(
        result.is_err(),
        "image appeared after the allowance: {result:?}"
    );
    assert!(
        !marker.exists(),
        "expired retry still launched the new image"
    );
    assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");

    let marker = root.join("timely-launch.pid");
    let (program, publisher) = publish_after(
        &root,
        "timely-image",
        Duration::from_millis(75),
        "printf '%s' \"$$\" > \"$1\"; printf ready",
    );
    let mut command = Command::new(program);
    command.arg(&marker);
    let result = output_timed_fixed_captured(command, Duration::from_secs(1));
    publisher.join().unwrap();
    let capture = result.unwrap();
    assert!(capture.output.status.success());
    assert!(!capture.timed_out);
    assert!(capture.output_complete());
    assert_eq!(capture.output.stdout, b"ready");
    let leader = std::fs::read_to_string(marker).unwrap().parse().unwrap();
    ServiceFixture::assert_reaped(leader);

    // Publishing after 300 ms leaves 700 ms of the original 1 s allowance. The
    // command needs 800 ms, so a refreshed 1 s child timer would wrongly accept it.
    let marker = root.join("active-launch.pid");
    let (program, publisher) = publish_after(
        &root,
        "active-image",
        Duration::from_millis(300),
        "sleep 0.8 & printf '%s %s' \"$$\" \"$!\" > \"$1\"; printf started; wait; printf finished",
    );
    let mut command = Command::new(program);
    command.arg(&marker);
    let started = Instant::now();
    let result = output_timed_fixed_captured(command, Duration::from_secs(1));
    let elapsed = started.elapsed();
    publisher.join().unwrap();
    let capture = result.unwrap();
    eprintln!("fixed retry + active child: budget=1000ms elapsed={elapsed:?}");
    assert!(
        capture.timed_out,
        "retry gave the child a fresh allowance: {capture:?}"
    );
    assert!(capture.output.stdout.starts_with(b"started"));
    assert!(!String::from_utf8_lossy(&capture.output.stdout).contains("finished"));
    assert!(elapsed < Duration::from_millis(1500), "{elapsed:?}");
    for pid in std::fs::read_to_string(marker).unwrap().split_whitespace() {
        ServiceFixture::assert_reaped(pid.parse().unwrap());
    }

    // Ordinary tool calls retain their existing transient-image setup grace
    // and child timer. The same delayed image + 800 ms command still completes.
    let marker = root.join("ordinary-launch.pid");
    let (program, publisher) = publish_after(
        &root,
        "ordinary-image",
        Duration::from_millis(300),
        "sleep 0.8 & printf '%s %s' \"$$\" \"$!\" > \"$1\"; printf started; wait; printf finished",
    );
    let mut command = Command::new(program);
    command.arg(&marker);
    let result = output_timed_captured(command, Some(Duration::from_secs(1)));
    publisher.join().unwrap();
    let capture = result.unwrap();
    assert!(capture.output.status.success(), "{capture:?}");
    assert!(
        !capture.timed_out,
        "ordinary setup grace changed: {capture:?}"
    );
    assert_eq!(capture.output.stdout, b"startedfinished");
    for pid in std::fs::read_to_string(marker).unwrap().split_whitespace() {
        ServiceFixture::assert_reaped(pid.parse().unwrap());
    }
}

#[test]
fn fixed_probes_share_the_spawn_retry_and_child_budget() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::fixed_spawn_budget_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_FIXED_SPAWN",
    );
}
