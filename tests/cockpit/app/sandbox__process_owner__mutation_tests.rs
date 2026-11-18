macro_rules! self_test_filter {
    ($name:literal) => {
        format!(
            "{}::{}",
            module_path!()
                .split_once("::")
                .map_or(module_path!(), |(_crate, rest)| rest),
            $name
        )
    };
}

use super::*;

#[test]
#[ignore = "subprocess fixture; invoked by cleanup_reaps_adopted_children"]
fn cleanup_child_fixture() {
    let Some(marker) = std::env::var_os("ANGEL_T_OWNER_MARKER") else {
        return;
    };
    install().unwrap();
    let payload = std::env::var("ANGEL_T_OWNER_PAYLOAD").unwrap();
    let status = std::process::Command::new("sh")
        .args(["-c", &payload, "owner-fixture"])
        .arg(&marker)
        .status()
        .unwrap();
    assert!(status.success());
    cleanup();
    let remaining = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
    assert_eq!(remaining, -1, "cleanup left a child or zombie");
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn cleanup_reaps_adopted_children() {
    use std::os::unix::process::CommandExt;
    struct Sentinel(std::process::Child);
    impl Drop for Sentinel {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let root = std::env::temp_dir().join(format!("angel-owner-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut sentinel = Sentinel(
        std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap(),
    );
    for (index, payload) in [
            "true",
            "sleep 30 & echo $! > \"$1\"",
            "setsid sh -c 'echo $$ > \"$1\"; sleep 30' child \"$1\" & while [ ! -s \"$1\" ]; do :; done",
            "setsid sh -c 'trap \"\" TERM; echo $$ > \"$1\"; sleep 30' child \"$1\" & while [ ! -s \"$1\" ]; do :; done",
        ].iter().enumerate() {
            let marker = root.join(format!("child-{index}"));
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", self_test_filter!("cleanup_child_fixture").as_str(), "--ignored", "--test-threads=1"])
                .env("ANGEL_T_OWNER_MARKER", &marker)
                .env("ANGEL_T_OWNER_PAYLOAD", payload)
                .stdout(std::process::Stdio::null())
                .process_group(0)
                .spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if Instant::now() >= deadline {
                    unsafe { libc::killpg(child.id() as i32, libc::SIGKILL) };
                    let _ = child.wait();
                    panic!("owned-child cleanup exceeded deadline: {index}");
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            assert!(status.success(), "ownership fixture failed: {index}");
            assert!(sentinel.0.try_wait().unwrap().is_none(), "unrelated sentinel killed");
            if let Ok(pid) = std::fs::read_to_string(&marker) {
                let pid: i32 = pid.trim().parse().unwrap();
                assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "owned PID survived: {pid}");
                assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
            }
        }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn lifecycle_unknown_external_owner_is_not_fabricated() {
    let unknown = KillReceipt::new(Some(libc::SIGTERM), "signal", "unknown_external");
    assert_eq!(unknown.owner_pid, None);
    let known = KillReceipt::new(None, "cancelled", "harness");
    assert_eq!(known.owner_pid, Some(std::process::id()));
}

#[test]
fn lifecycle_grace_boundary_escalates_at_deadline() {
    let deadline = Instant::now();
    assert_eq!(
        cleanup_signal(deadline - Duration::from_nanos(1), deadline),
        libc::SIGTERM
    );
    assert_eq!(cleanup_signal(deadline, deadline), libc::SIGKILL);
    assert_eq!(
        cleanup_signal(deadline + Duration::from_nanos(1), deadline),
        libc::SIGKILL
    );
}

#[test]
fn stopped_background_work_is_named_to_the_reader() {
    assert_eq!(stopped_background_notice(&[]), None);

    let one = stopped_background_notice(&["python3".to_string()]).unwrap();
    assert!(
        one.starts_with(
            "angel: 1 background process this command started was still running when it returned and was stopped (python3)."
        ),
        "{one}"
    );
    assert!(one.contains("`wait`") && one.ends_with('\n'), "{one}");

    let many = stopped_background_notice(
        &["timeout", "python3", "timeout", "python3"].map(String::from),
    )
    .unwrap();
    assert!(
        many.starts_with(
            "angel: 4 background processes this command started were still running when it returned and were stopped (timeout, python3)."
        ),
        "{many}"
    );

    let crowd = stopped_background_notice(&["a", "b", "c", "d", "e"].map(String::from)).unwrap();
    assert!(crowd.contains("(a, b, c, d, …)"), "{crowd}");
}
