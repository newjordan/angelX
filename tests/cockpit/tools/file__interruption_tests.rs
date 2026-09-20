use super::*;

struct TestRoot(PathBuf);
impl TestRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-patch-exit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn patch_exit_drain_sigint_between_real_patch_writes() {
    let _env = crate::tests::env_lock();
    const CHILD: &str = "ANGEL_T_PATCH_SIGNAL_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        crate::agent::sandbox::process_owner::initialize().unwrap();
        apply_freeform_patch(
            &PathBuf::from(root),
            "*** Begin Patch\n*** Update File: first.txt\n@@\n-before\n+after\n\
                 *** Update File: second.txt\n@@\n-before\n+after\n*** End Patch\n",
        )
        .unwrap();
        // Keep the fixture alive if the patch wins the scheduling race.
        loop {
            std::thread::park();
        }
    }
    let root = TestRoot::new();
    std::fs::write(root.path().join("first.txt"), "before\n").unwrap();
    let second_before = format!("before\n{}", "padding\n".repeat(1_048_576));
    std::fs::write(root.path().join("second.txt"), &second_before).unwrap();
    std::fs::write(root.path().join("UNRELATED.md"), "keep\n").unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "agent::tools::file::interruption_tests::patch_exit_drain_sigint_between_real_patch_writes",
            "--nocapture",
        ])
        .env(CHILD, root.path())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    loop {
        if std::fs::read(root.path().join("first.txt")).unwrap() == b"after\n" {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("fixture did not reach its first write");
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "fixture exited before first write"
        );
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
    // SAFETY: this is the still-owned, unreaped test child only.
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGINT) }, 0);
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(130));
    assert_eq!(
        std::fs::read_to_string(root.path().join("first.txt")).unwrap(),
        "after\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("second.txt")).unwrap(),
        second_before.replacen("before", "after", 1)
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("UNRELATED.md")).unwrap(),
        "keep\n"
    );
}

#[test]
fn patch_exit_drain_completes_active_transaction() {
    let _env = crate::tests::env_lock();
    const CHILD: &str = "ANGEL_T_PATCH_EXIT_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        let root = PathBuf::from(root);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _guard = exit_safe_patch::Guard::enter().unwrap();
            std::fs::write(root.join("first.txt"), "landed\n").unwrap();
            ready_tx.send(()).unwrap();
            // A real process exit starts between these fixture writes.
            std::thread::sleep(std::time::Duration::from_millis(100));
            std::fs::write(root.join("second.txt"), "landed\n").unwrap();
        });
        ready_rx.recv().unwrap();
        std::process::exit(130);
    }
    let root = TestRoot::new();
    std::fs::write(root.path().join("UNRELATED.md"), "keep\n").unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "agent::tools::file::interruption_tests::patch_exit_drain_completes_active_transaction",
            "--nocapture",
        ])
        .env(CHILD, root.path())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(130));
    for name in ["first.txt", "second.txt"] {
        assert_eq!(
            std::fs::read_to_string(root.path().join(name)).unwrap(),
            "landed\n"
        );
    }
    assert_eq!(
        std::fs::read_to_string(root.path().join("UNRELATED.md")).unwrap(),
        "keep\n"
    );
}
