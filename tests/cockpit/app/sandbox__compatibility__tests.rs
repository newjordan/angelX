use super::*;
#[test]
fn sandbox_doctor_classes_and_exact_remedies() {
    let cases = [
        (
            false,
            true,
            false,
            false,
            "",
            Cause::MissingBwrap,
            "sudo apt install bubblewrap",
        ),
        (
            true,
            false,
            false,
            false,
            "",
            Cause::MissingUserns,
            "CONFIG_USER_NS=y",
        ),
        (
            true,
            true,
            false,
            true,
            "",
            Cause::UsernsDisabled,
            "sudo sysctl -w kernel.unprivileged_userns_clone=1",
        ),
        (
            true,
            true,
            true,
            false,
            "bwrap: setting up uid map: Permission denied",
            Cause::AppArmor,
            "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0",
        ),
    ];
    for (b, u, a, c, stderr, expected, remedy) in cases {
        let cause = classify(b, u, a, c, stderr).unwrap();
        assert_eq!(cause, expected);
        assert!(cause.remedy().contains(remedy));
        assert!(cause.notice().contains("Landlock-only"));
    }
    assert_eq!(classify(true, true, true, false, ""), None);
}

#[test]
fn probe_wait_preserves_fast_child_exit_status() {
    let mut child = std::process::Command::new("sh")
        .args(["-c", "exit 7"])
        .spawn_owned()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(7));
            break;
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("probe wait did not observe child exit");
        }
        wait_for_probe_exit(child.id(), remaining);
    }
}

#[test]
fn probe_wait_deadline_does_not_reap_or_accept_a_running_child() {
    let mut child = std::process::Command::new("sleep")
        .arg("5")
        .spawn_owned()
        .unwrap();
    let started = std::time::Instant::now();
    wait_for_probe_exit(child.id(), std::time::Duration::from_millis(20));
    let status = child.try_wait().unwrap();
    let _ = child.kill();
    let _ = child.wait();
    assert!(status.is_none(), "readiness wait must not invent an exit status");
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
fn probe_wait_invalid_pid_uses_bounded_fallback() {
    let started = std::time::Instant::now();
    // pidfd_open rejects negative PIDs. This also exercises the portable path.
    wait_for_probe_exit(u32::MAX, std::time::Duration::from_millis(2));
    assert!(started.elapsed() >= std::time::Duration::from_millis(2));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}
