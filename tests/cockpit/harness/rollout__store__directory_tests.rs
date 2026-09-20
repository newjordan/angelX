use super::*;
use crate::agent::harness::rollout::recorder::RolloutRecorder;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-rollout-dirs-{}-{}-{}",
            std::process::id(),
            now_ms(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().mode() & 0o777
}
fn check_tree(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert!(!metadata.file_type().is_symlink());
    if metadata.is_dir() {
        assert_eq!(mode(path), 0o700, "{}", path.display());
        for entry in fs::read_dir(path).unwrap() {
            check_tree(&entry.unwrap().path());
        }
    } else {
        assert!(metadata.is_file());
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(mode(path), 0o600, "{}", path.display());
    }
}

#[test]
fn actual_rollout_base_and_descendants_are_private_under_zero_umask_and_migration() {
    let _guard = crate::tests::env_lock();
    const CHILD: &str = "ANGEL_T_ROLLOUT_DIRECTORY_UMASK_CHILD";
    if std::env::var_os(CHILD).is_some() {
        unsafe {
            libc::umask(0);
        }
        let fixture = Fixture::new();
        let shared = fixture.0.join("shared");
        fs::DirBuilder::new().mode(0o750).create(&shared).unwrap();
        let workspace = fixture.0.join("workspace");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&workspace)
            .unwrap();
        // Both blob-first and recorder/lease-first construction create the
        // explicit base and repo subtree privately before payload writes.
        for first in ["blob-first", "lease-first"] {
            let base = shared.join(first);
            let _env = crate::tests::TestEnvGuard::set(
                "ANGEL_HARNESS_ROLLOUT_DIR",
                base.to_str().unwrap(),
            );
            let store = RolloutStore::for_workspace(&workspace);
            assert!(!base.exists(), "constructing a store must not allocate it");
            let project = RolloutStore::project_identity(&workspace);
            if first == "blob-first" {
                store.put_blob(b"fixture body").unwrap();
            }
            let mut recorder = RolloutRecorder::start_for_test(
                store.clone(),
                CaptureMode::Shadow,
                project.clone(),
            )
            .unwrap();
            let id = recorder.rollout_id().unwrap().to_string();
            let blob = store.put_blob(b"fixture body").unwrap();
            assert_eq!(blob, crate::knowledge::cut::sha256_hex(b"fixture body"));
            recorder
                .finish_turn("fixture answer", false, false, false, Some("done"))
                .unwrap();
            drop(recorder);
            check_tree(&base);
            let manifest_path = store.run_dir(&id).unwrap().join("manifest.json");
            let before = fs::read(&manifest_path).unwrap();
            let original = store.load_manifest(&id, &project.repo_key).unwrap();
            let runs = store.root.join("runs");
            let run = store.run_dir(&id).unwrap();
            let blobs = store.root.join("blobs");
            for path in [&base, &store.root, &runs, &run, &blobs] {
                fs::set_permissions(path, fs::Permissions::from_mode(0o775)).unwrap();
            }
            let lease = store.acquire_lease(&id).unwrap();
            assert_eq!(store.put_blob(b"fixture body").unwrap(), blob);
            drop(lease);
            check_tree(&base);
            assert_eq!(
                fs::read(&manifest_path).unwrap(),
                before,
                "directory migration must not rewrite sealed evidence"
            );
            let migrated = store.load_manifest(&id, &project.repo_key).unwrap();
            assert_eq!(migrated.manifest_sha256, original.manifest_sha256);
            assert_eq!(migrated.journal_head_sha256, original.journal_head_sha256);
            assert_eq!(
                fs::read(store.root.join("blobs").join(&blob)).unwrap(),
                b"fixture body"
            );
            assert_eq!(
                mode(&shared),
                0o750,
                "unrelated ancestor must retain its mode"
            );
        }
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
        .args(["--exact", "harness::rollout::store::directory_tests::actual_rollout_base_and_descendants_are_private_under_zero_umask_and_migration", "--test-threads=1", "--nocapture"])
        .env(CHILD, "1").stdin(std::process::Stdio::null()).spawn().unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "rollout directory child failed: {status}");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "rollout directory child timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn rollout_rejects_symlinked_base_ancestor_and_project_without_touching_targets() {
    let fixture = Fixture::new();
    let outside = fixture.0.join("outside");
    fs::DirBuilder::new().mode(0o750).create(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"unchanged").unwrap();
    for (name, suffix) in [("ancestor", "base"), ("base", "")] {
        let link = fixture.0.join(name);
        symlink(&outside, &link).unwrap();
        let base = if suffix.is_empty() {
            link.clone()
        } else {
            link.join(suffix)
        };
        let store = RolloutStore {
            root: base.join("project"),
            configured_base: Some(base),
        };
        assert!(store.put_blob(b"rejected").is_err());
        assert!(store.acquire_lease("rol-a-b-c").is_err());
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(mode(&outside), 0o750);
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    }
    let base = fixture.0.join("owned-base");
    fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
    let root = base.join("project");
    symlink(&outside, &root).unwrap();
    let store = RolloutStore {
        root,
        configured_base: Some(base),
    };
    assert!(store.put_blob(b"rejected").is_err());
    assert!(store.acquire_lease("rol-a-b-c").is_err());
    assert_eq!(mode(&outside), 0o750);
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
}
