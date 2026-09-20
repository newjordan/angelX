use super::*;
use crate::caddy::Hazard;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-caddy-dirs-{}-{}-{}",
            std::process::id(),
            crate::caddy::now_ms(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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
fn mode(path: &Path) -> u32 {
    std::fs::symlink_metadata(path).unwrap().mode() & 0o777
}
fn hazard(command: &str) -> Hazard {
    Hazard {
        ts_ms: crate::caddy::now_ms(),
        command: command.into(),
        diagnostic: "owned fixture".into(),
        tool: "shell".into(),
    }
}

#[test]
fn actual_caddy_root_and_project_are_private_under_zero_umask_and_migration() {
    let _guard = crate::tests::env_lock();
    const CHILD: &str = "ANGEL_T_CADDY_DIRECTORY_UMASK_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // The parent test process never changes its umask.
        unsafe {
            libc::umask(0);
        }
        let fixture = Fixture::new();
        let shared = fixture.0.join("shared");
        std::fs::DirBuilder::new()
            .mode(0o750)
            .create(&shared)
            .unwrap();
        let base = shared.join("caddy");
        let project = base.join("project");
        let _env = crate::tests::TestEnvGuard::set("ANGEL_CADDY_DIR", base.to_str().unwrap());
        assert_eq!(
            append::<Hazard>(&project, "hazards.jsonl", &[], None).status,
            WriteStatus::Unchanged
        );
        assert!(!base.exists(), "empty evidence must not allocate a store");
        assert_eq!(
            append(&project, "hazards.jsonl", &[hazard("first")], None).status,
            WriteStatus::Published
        );
        for path in [&base, &project] {
            assert_eq!(mode(path), 0o700);
        }
        let data = project.join("hazards.jsonl");
        let lock = project.join("hazards.jsonl.lock");
        for path in [&data, &lock] {
            assert_eq!(mode(path), 0o600);
        }
        let before = std::fs::read(&data).unwrap();
        for path in [&base, &project] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o775)).unwrap();
        }
        assert_eq!(
            append(&project, "hazards.jsonl", &[hazard("second")], None).status,
            WriteStatus::Published
        );
        for path in [&base, &project] {
            assert_eq!(mode(path), 0o700);
        }
        for path in [&data, &lock] {
            assert_eq!(mode(path), 0o600);
        }
        assert_eq!(
            mode(&shared),
            0o750,
            "unrelated ancestor must retain its mode"
        );
        let after = std::fs::read(&data).unwrap();
        assert!(after.starts_with(&before));
        let rows: Vec<Hazard> = std::str::from_utf8(&after)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            rows.iter()
                .map(|row| row.command.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        return;
    }
    struct Reap(std::process::Child);
    impl Drop for Reap {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Reap(std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "caddy::storage::directory_tests::actual_caddy_root_and_project_are_private_under_zero_umask_and_migration", "--test-threads=1", "--nocapture"])
        .env(CHILD, "1").stdin(std::process::Stdio::null()).spawn().unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "Caddy directory child failed: {status}");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Caddy directory child timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn caddy_rejects_symlinked_ancestor_and_project_without_touching_targets() {
    let fixture = Fixture::new();
    let outside = fixture.0.join("outside");
    std::fs::DirBuilder::new()
        .mode(0o750)
        .create(&outside)
        .unwrap();
    std::fs::write(outside.join("sentinel"), b"unchanged").unwrap();
    for (name, suffix) in [("ancestor", "caddy/project"), ("project", "")] {
        let link = fixture.0.join(name);
        symlink(&outside, &link).unwrap();
        let project = if suffix.is_empty() {
            link.clone()
        } else {
            link.join(suffix)
        };
        assert!(matches!(
            append(&project, "hazards.jsonl", &[hazard("rejected")], None).status,
            WriteStatus::Failed(_)
        ));
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(mode(&outside), 0o750);
        assert_eq!(
            std::fs::read(outside.join("sentinel")).unwrap(),
            b"unchanged"
        );
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);
    }
}
