use super::*;

#[cfg(target_os = "linux")]
#[test]
fn owned_cpu_crosses_sessions_without_crediting_siblings() {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    let _guard = crate::tests::env_lock();
    struct Probe {
        parent: crate::sandbox::process_owner::Child,
        payload: Option<u32>,
        payload_fd: Option<OwnedFd>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            if let Some(fd) = &self.payload_fd {
                unsafe {
                    libc::syscall(
                        libc::SYS_pidfd_send_signal,
                        fd.as_raw_fd(),
                        libc::SIGKILL,
                        std::ptr::null::<libc::siginfo_t>(),
                        0,
                    );
                }
            }
            let _ = self.parent.kill();
            let _ = self.parent.wait();
        }
    }
    let dir = crate::tests::TestGitWorkspace::new("owned-cpu");
    let ready = dir.path().join("payload.pid");
    let script = r#"
import subprocess, sys
child = subprocess.Popen([sys.executable, '-c',
    'import time; end=time.monotonic()+10\nwhile time.monotonic()<end: pass'],
    start_new_session=True)
with open(sys.argv[1], 'w') as f:
    f.write(str(child.pid))
child.wait()
"#;
    let mut busy = Probe {
        parent: Command::new("python3")
            .args(["-c", script])
            .arg(&ready)
            .spawn_owned()
            .unwrap(),
        payload: None,
        payload_fd: None,
    };
    let sleeping = Probe {
        parent: Command::new("sleep").arg("10").spawn_owned().unwrap(),
        payload: None,
        payload_fd: None,
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while busy.payload.is_none() && Instant::now() < deadline {
        busy.payload = std::fs::read_to_string(&ready)
            .ok()
            .and_then(|pid| pid.parse().ok());
        std::thread::sleep(Duration::from_millis(10));
    }
    let payload = busy.payload.expect("cross-session payload became ready");
    let identity = proc_activity(payload).unwrap();
    assert_eq!(identity.parent, busy.parent.id());
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, payload, 0) } as i32;
    assert!(fd >= 0, "pin fixture payload identity");
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    assert_eq!(proc_activity(payload).unwrap().started, identity.started);
    busy.payload_fd = Some(fd);
    assert_ne!(unsafe { libc::getsid(payload as i32) }, unsafe {
        libc::getsid(busy.parent.id() as i32)
    });
    assert_eq!(live_group_descendants(busy.parent.id()), 1);
    let key = (payload, proc_activity(payload).unwrap().started);
    let initial = cpu_ticks(busy.parent.id());
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let current = cpu_ticks(busy.parent.id());
        if current.get(&key).copied().unwrap_or(0) > initial.get(&key).copied().unwrap_or(0) {
            break;
        }
        assert!(Instant::now() < deadline, "owned payload CPU was invisible");
        std::thread::sleep(Duration::from_millis(20));
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while proc_activity(sleeping.parent.id()).is_none_or(|row| row.state != 'S') {
        assert!(Instant::now() < deadline, "sleeping sibling never settled");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!process_group_is_runnable(sleeping.parent.id()));
    assert_eq!(live_group_descendants(sleeping.parent.id()), 0);
    assert!(
        cpu_ticks(sleeping.parent.id())
            .keys()
            .all(|(pid, _)| *pid == sleeping.parent.id())
    );
}

#[test]
fn child_activity_snapshot_keeps_cpu_output_and_owner_distinct() {
    let _guard = crate::tests::env_lock();
    let parent = AtomicBool::new(false);
    let nested = AtomicBool::new(false);
    let foreign = AtomicBool::new(false);
    let owner = &parent as *const _ as usize;
    let _link = link_child_owner(&nested, &parent);
    let mut child = ChildActivity::new(u32::MAX, Some(&nested));
    let initial = owned_child_snapshot(owner).unwrap();
    assert_eq!(initial.cpu_age_secs, None);
    assert_eq!(initial.output_age_secs, None);
    assert!(owned_child_snapshot(&foreign as *const _ as usize).is_none());
    child.program = "rustc".into();
    child.last_cpu = Some(Instant::now());
    child.observe(Some(Instant::now() - Duration::from_secs(40)));
    let busy = owned_child_snapshot(owner).unwrap();
    assert_eq!(busy.program, "rustc");
    assert_eq!(busy.cpu_age_secs, Some(0));
    assert!(busy.output_age_secs.unwrap() >= 40);
    child.last_cpu = Some(Instant::now() - Duration::from_secs(35));
    child.observe(None);
    assert!(owned_child_snapshot(owner).unwrap().cpu_age_secs.unwrap() >= 35);
    drop(child);
    assert!(
        owned_child_snapshot(owner).is_none(),
        "exited work must disappear"
    );
}

#[test]
fn run_turn_helper_setup_grace_expires_without_progress_credit() {
    let _guard = crate::tests::env_lock();
    let cancel = AtomicBool::new(false);
    let owner = &cancel as *const _ as usize;
    let activity = ChildActivity::new(u32::MAX, Some(&cancel));
    let key = activity.key.unwrap();
    entries().lock().unwrap().get_mut(&key).unwrap().setup_until =
        Some(Instant::now() + Duration::from_secs(60));
    assert!(owned_child_setting_up(owner));
    assert!(!owned_child_active(owner));
    entries().lock().unwrap().get_mut(&key).unwrap().setup_until =
        Some(Instant::now() - Duration::from_secs(1));
    assert!(!owned_child_setting_up(owner));
    assert!(!owned_child_active(owner));
}

#[test]
fn stale_cpu_credit_expires_at_shorter_turn_idle_timeout() {
    let _guard = crate::tests::env_lock();
    let _floor = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "30");
    let _turn = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1");
    let _tool = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_SECS");
    let cancel = AtomicBool::new(false);
    let mut activity = ChildActivity::new(u32::MAX, Some(&cancel));
    activity.last_cpu = Some(Instant::now() - Duration::from_secs(2));
    activity.observe(None);
    assert!(!owned_child_active(&cancel as *const _ as usize));
    activity.last_cpu = Some(Instant::now());
    activity.observe(None);
    assert!(owned_child_active(&cancel as *const _ as usize));
}

#[test]
fn run_turn_child_activity_is_owned_recent_and_released() {
    let _guard = crate::tests::env_lock();
    let _floor = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "2");
    let parent = AtomicBool::new(false);
    let nested = AtomicBool::new(false);
    let foreign = AtomicBool::new(false);
    let parent_key = &parent as *const _ as usize;
    let _link = link_child_owner(&nested, &parent);
    let mut activity = ChildActivity::new(u32::MAX, Some(&nested));
    assert!(
        !owned_child_active(parent_key),
        "spawn alone is not progress"
    );
    activity.observe(Some(Instant::now()));
    assert!(owned_child_active(parent_key));
    assert!(owned_child_active(&nested as *const _ as usize));
    assert!(owned_child_snapshot(&nested as *const _ as usize).is_some());
    assert!(!owned_child_active(&foreign as *const _ as usize));
    activity.observe(Some(Instant::now() - Duration::from_secs(3)));
    assert!(
        !owned_child_active(parent_key),
        "stale output is not liveness"
    );
    activity.observe(Some(Instant::now()));
    drop(activity);
    assert!(
        !owned_child_active(parent_key),
        "exited child releases credit"
    );
}
