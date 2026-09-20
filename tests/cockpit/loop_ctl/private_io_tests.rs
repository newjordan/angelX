use super::*;
use std::ffi::OsStr;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-loop-private-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn watchdog_actual_writes_are_private_under_zero_umask() {
    let _guard = crate::tests::env_lock();
    const CHILD: &str = "ANGEL_T_LOOP_PRIVATE_UMASK_CHILD";
    if std::env::var_os(CHILD).is_some() {
        unsafe {
            libc::umask(0);
        }
        let fixture = Fixture::new();
        let workspace = fixture.0.join("workspace");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&workspace)
            .unwrap();
        let base = fixture.0.join("watchdog");
        let _identity = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
        let _state =
            crate::tests::TestEnvGuard::set("ANGEL_LOOP_STATE_DIR", base.to_str().unwrap());
        let state = LoopState {
            workspace: Some(workspace.clone()),
            findings: vec!["one finding".into()],
            log: vec![serde_json::from_value(serde_json::json!({"iteration":1,"direction":"fixture","new_findings":1,"stale_count":0,"ts_ms":123})).unwrap()],
            ..LoopState::default()
        };
        persist_state_dir(&state);
        let identity = crate::platform::workspace_store::repo_identity(&workspace);
        let directory = base.join(&identity.key).join("state");
        for name in [
            "progress.json",
            "acceptance.json",
            "findings.jsonl",
            "iteration_log.jsonl",
        ] {
            let path = directory.join(name);
            let metadata = std::fs::metadata(&path).unwrap();
            assert_eq!(metadata.mode() & 0o777, 0o600, "{name}");
            assert_eq!(metadata.nlink(), 1);
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert!(value.is_object());
        }
        assert_eq!(std::fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
        assert_eq!(state.persisted_findings.get(), 1);
        assert_eq!(state.persisted_log.get(), 1);
        return;
    }
    struct Reap(std::process::Child);
    impl Drop for Reap {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Reap(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "drive::loop_ctl::private_io_tests::watchdog_actual_writes_are_private_under_zero_umask",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .stdin(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "watchdog writer child failed: {status}");
            break;
        }
        assert!(Instant::now() < deadline, "watchdog writer child timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn watchdog_jsonl_preserves_append_reset_and_missing_file_retry_semantics() {
    let fixture = Fixture::new();
    let directory =
        crate::platform::workspace_store::private_io::PrivateDirectory::open(&fixture.0).unwrap();
    let name = OsStr::new("findings.jsonl");
    let path = fixture.0.join(name);
    let written = std::cell::Cell::new(0);
    let mut rows = vec!["first".to_string(), "second".to_string()];
    sync_jsonl(&directory, name, &rows, &written, Clone::clone);
    let initial_inode = std::fs::metadata(&path).unwrap().ino();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond");
    assert_eq!(written.get(), 2);
    rows.push("third".into());
    sync_jsonl(&directory, name, &rows, &written, Clone::clone);
    assert_eq!(
        std::fs::metadata(&path).unwrap().ino(),
        initial_inode,
        "tail writes stay append-only"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "first\nsecond\nthird"
    );
    assert_eq!(written.get(), 3);
    std::fs::remove_file(&path).unwrap();
    rows.push("fourth".into());
    sync_jsonl(&directory, name, &rows, &written, Clone::clone);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "first\nsecond\nthird\nfourth"
    );
    assert_eq!(written.get(), 4);
    written.set(0);
    sync_jsonl(
        &directory,
        name,
        &["new run".to_string()],
        &written,
        Clone::clone,
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new run");
    assert_eq!(written.get(), 1);
    assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o777, 0o600);
}

#[test]
fn watchdog_hostile_entries_preserve_targets_and_retry_counters() {
    let fixture = Fixture::new();
    let directory =
        crate::platform::workspace_store::private_io::PrivateDirectory::open(&fixture.0).unwrap();
    let target = fixture.0.join("untouched");
    std::fs::write(&target, b"untouched").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    let name = OsStr::new("findings.jsonl");
    let path = fixture.0.join(name);
    for kind in ["symlink", "hardlink"] {
        if kind == "symlink" {
            symlink(&target, &path).unwrap();
        } else {
            std::fs::hard_link(&target, &path).unwrap();
        }
        for done in [0, 1] {
            let written = std::cell::Cell::new(done);
            sync_jsonl(
                &directory,
                name,
                &["first".to_string(), "second".to_string()],
                &written,
                Clone::clone,
            );
            assert_eq!(written.get(), done);
            assert_eq!(std::fs::read(&target).unwrap(), b"untouched");
            assert_eq!(std::fs::metadata(&target).unwrap().mode() & 0o777, 0o640);
        }
        std::fs::remove_file(&path).unwrap();
    }
    let written = std::cell::Cell::new(0);
    sync_jsonl(
        &directory,
        name,
        &["retry".to_string()],
        &written,
        Clone::clone,
    );
    assert_eq!(written.get(), 1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "retry");
}
