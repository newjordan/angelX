use super::*;
use crate::agent::sandbox::process_owner::OwnedCommandExt;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn idle_completion_fixture() {
    let _guard = crate::tests::env_lock();
    let Some(root) = std::env::var_os("ANGEL_T_PTY_OWNER_FIXTURE") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    crate::agent::sandbox::process_owner::initialize().unwrap();
    let marker = root.join("descendant.pid");
    let script = root.join("fixture-shell");
    std::fs::write(
        &script,
        r#"#!/usr/bin/python3
import os,signal,time
from pathlib import Path
marker = Path(os.environ['ANGEL_T_PTY_CHILD_MARKER'])
child = os.fork()
if child == 0:
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    os.setsid()
    marker.write_text(str(os.getpid()))
    time.sleep(1.5)
    os._exit(0)
while not marker.exists():
    time.sleep(0.001)
os._exit(23)
"#,
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _shell = crate::tests::TestEnvGuard::set("SHELL", script.to_str().unwrap());
    let _marker =
        crate::tests::TestEnvGuard::set("ANGEL_T_PTY_CHILD_MARKER", marker.to_str().unwrap());
    let pane = ShellPane::spawn_in(8, 40, Some(&root)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if pane.child.lock().unwrap().claim.is_none() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "idle PTY shell exit was not reaped"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        pane.child
            .lock()
            .unwrap()
            .handle
            .try_wait()
            .unwrap()
            .unwrap()
            .exit_code(),
        23
    );
    let pid: u32 = std::fs::read_to_string(marker)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let stat_path = format!("/proc/{pid}/stat");
    let stat = std::fs::read_to_string(&stat_path).unwrap();
    assert_ne!(
        stat.rsplit_once(')').unwrap().1.split_whitespace().next(),
        Some("Z"),
        "the output-holding descendant must still be alive when the shell is reaped"
    );
    // Teardown must not signal the already-reaped shell's numeric PID.
    drop(pane);
    let deadline = Instant::now() + Duration::from_secs(3);
    while std::path::Path::new(&stat_path).exists() {
        assert!(
            Instant::now() < deadline,
            "PTY descendant was not reaped after natural exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!(
        "LIFECYCLE_PTY idle_exit=23 claim_released=true pipe_holder_alive_at_receipt=true descendant_reaped=true"
    );
}

#[test]
fn idle_pty_exit_is_reaped_even_while_a_descendant_keeps_output_open() {
    let _lock = crate::tests::env_lock();
    if !ShellPane::can_spawn() {
        eprintln!("PTY ownership fixture unavailable: host cannot allocate a PTY");
        return;
    }
    let root = std::env::temp_dir().join(format!("angel-pty-owner-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "pty::process_ownership_tests::idle_completion_fixture",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("ANGEL_T_PTY_OWNER_FIXTURE", &root)
        .stdin(Stdio::null())
        .spawn_owned()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
            let _ = child.wait();
            panic!("PTY ownership fixture exceeded its deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "PTY ownership subprocess failed");
    std::fs::remove_dir_all(root).unwrap();
}
