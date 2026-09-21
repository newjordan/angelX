use super::*;
use std::os::unix::fs::{DirBuilderExt, symlink};
use std::path::PathBuf;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-private-io-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
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

fn lock_file(path: &Path) -> io::Result<File> {
    let (directory, name) = parent(path)?;
    directory.lock_file(name)
}

#[test]
fn private_append_migrates_owner_file_before_write_and_preserves_parent_mode() {
    let fixture = Fixture::new();
    std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o750)).unwrap();
    let path = fixture.0.join("ledger");
    std::fs::write(&path, b"previous\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o664)).unwrap();
    let mut file = append(&path).unwrap();
    assert_eq!(
        mode(&path),
        0o600,
        "must be private before the first new byte"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"previous\n");
    file.write_all(b"next\n").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"previous\nnext\n");
    assert_eq!(mode(&fixture.0), 0o750);
}

#[test]
fn private_store_rejects_symlinks_hardlinks_fifo_and_directory_entries() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let target = outside.0.join("target");
    std::fs::write(&target, b"do not touch").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    let link = fixture.0.join("link");
    symlink(&target, &link).unwrap();
    assert!(append(&link).is_err());
    assert!(lock_file(&link).is_err());
    assert!(replace(&link, b"bad").is_err());
    let hard = fixture.0.join("hard");
    std::fs::hard_link(&target, &hard).unwrap();
    assert!(append(&hard).is_err());
    assert!(lock_file(&hard).is_err());
    assert!(replace(&hard, b"bad").is_err());
    assert_eq!(
        mode(&target),
        0o640,
        "rejected targets must not be chmodded"
    );
    let fifo = fixture.0.join("fifo");
    let fifo_c = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) }, 0);
    assert!(append(&fifo).is_err());
    assert!(lock_file(&fifo).is_err());
    assert!(replace(&fifo, b"bad").is_err());
    let directory = fixture.0.join("directory");
    std::fs::create_dir(&directory).unwrap();
    assert!(append(&directory).is_err());
    assert!(lock_file(&directory).is_err());
    assert!(replace(&directory, b"bad").is_err());
    let ancestor = fixture.0.join("ancestor");
    symlink(&outside.0, &ancestor).unwrap();
    assert!(append(&ancestor.join("new")).is_err());
    assert!(!outside.0.join("new").exists());
    assert_eq!(std::fs::read(&target).unwrap(), b"do not touch");
}

#[test]
fn private_store_rotation_and_atomic_replacement_preserve_content() {
    let fixture = Fixture::new();
    let directory = PrivateDirectory::open(&fixture.0).unwrap();
    directory
        .append(OsStr::new("ledger"))
        .unwrap()
        .write_all(b"old\n")
        .unwrap();
    directory
        .rotate_if(OsStr::new("ledger"), OsStr::new("ledger.1"), 4)
        .unwrap();
    assert_eq!(std::fs::read(fixture.0.join("ledger.1")).unwrap(), b"old\n");
    assert_eq!(mode(&fixture.0.join("ledger.1")), 0o600);
    directory
        .append(OsStr::new("ledger"))
        .unwrap()
        .write_all(b"new\n")
        .unwrap();
    directory
        .replace(OsStr::new("ledger"), b"replacement\n")
        .unwrap();
    assert_eq!(
        std::fs::read(fixture.0.join("ledger")).unwrap(),
        b"replacement\n"
    );
    assert_eq!(mode(&fixture.0.join("ledger")), 0o600);
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 2);
    let outside = fixture.0.join("outside");
    std::fs::write(&outside, b"safe").unwrap();
    std::fs::remove_file(fixture.0.join("ledger.1")).unwrap();
    symlink(&outside, fixture.0.join("ledger.1")).unwrap();
    assert!(
        directory
            .rotate_if(OsStr::new("ledger"), OsStr::new("ledger.1"), 1)
            .is_err()
    );
    assert_eq!(std::fs::read(&outside).unwrap(), b"safe");
    assert_eq!(
        std::fs::read(fixture.0.join("ledger")).unwrap(),
        b"replacement\n"
    );
}

#[test]
fn private_store_parent_swap_keeps_operations_in_the_pinned_directory() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let parent = fixture.0.join("store");
    let directory = PrivateDirectory::open(&parent).unwrap();
    let held = fixture.0.join("held");
    std::fs::rename(&parent, &held).unwrap();
    symlink(&outside.0, &parent).unwrap();
    directory
        .append(OsStr::new("ledger"))
        .unwrap()
        .write_all(b"held\n")
        .unwrap();
    directory
        .replace(OsStr::new("ledger"), b"checked\n")
        .unwrap();
    assert_eq!(std::fs::read(held.join("ledger")).unwrap(), b"checked\n");
    assert!(!outside.0.join("ledger").exists());
}

#[test]
fn actual_store_writers_are_private_under_zero_umask() {
    let _guard = crate::tests::env_lock();
    const CHILD: &str = "ANGEL_T_PRIVATE_WRITER_UMASK_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // Only this dedicated single-test subprocess changes its process umask.
        unsafe {
            libc::umask(0);
        }
        let fixture = Fixture::new();
        let ledger = fixture.0.join("experience").join("ledger.jsonl");
        let row = serde_json::json!({"kind":"fixture", "count":7, "ok":true});
        crate::knowledge::experience::append_jsonl(&ledger, &row);
        assert_eq!(mode(&ledger), 0o600);
        assert_eq!(mode(&ledger.with_file_name("ledger.jsonl.lock")), 0o600);
        crate::knowledge::experience::replace_jsonl(&ledger, std::slice::from_ref(&row)).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&ledger).unwrap()).unwrap(),
            row
        );
        assert_eq!(mode(&ledger), 0o600);
        let trajectory = fixture.0.join("trajectories").join("session.jsonl");
        crate::agent::harness::append_trajectory(&trajectory, &row).unwrap();
        assert_eq!(mode(&trajectory), 0o600);
        for prefix in ["answer-", "authored-"] {
            let store = fixture.0.join(prefix);
            let shard = store.join(format!("{prefix}20000101.jsonl"));
            let mut evicted = 0;
            assert!(matches!(
                crate::knowledge::barrel::append_capped_shard(
                    &store,
                    prefix,
                    &shard,
                    &row.to_string(),
                    1024,
                    &mut evicted
                ),
                crate::knowledge::barrel::Append::Ok
            ));
            assert_eq!(mode(&shard), 0o600);
        }
        let workspace = fixture.0.join("workspace");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&workspace)
            .unwrap();
        let session = crate::knowledge::session::Session::at_for(
            fixture.0.join("sessions"),
            "private-session".into(),
            &workspace,
        );
        session
            .save(&[crate::agent::club::ChatMsg::user("owner-only session")])
            .unwrap();
        assert_eq!(mode(session.path()), 0o600);
        let _atlas = crate::tests::TestEnvGuard::set("ANGEL_ATLAS", "1");
        let atlas =
            crate::knowledge::atlas::AtlasService::open_in(&workspace, fixture.0.join("atlas"));
        atlas
            .add_operator(crate::knowledge::atlas::AtlasKind::Note, "fixture note")
            .unwrap();
        let project = fixture
            .0
            .join("atlas/projects")
            .join(format!("{}.json", atlas.project_key()));
        assert_eq!(mode(&project), 0o600);
        assert_eq!(mode(&project.with_extension("json.lock")), 0o600);
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
        .args(["--exact", "platform::workspace_store::private_io::tests::actual_store_writers_are_private_under_zero_umask", "--test-threads=1", "--nocapture"])
        .env(CHILD, "1").stdin(std::process::Stdio::null()).spawn().unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "private writer child failed: {status}");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "private writer child timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn trajectory_directory_permissions_use_checked_existing_targets_only() {
    let fixture = Fixture::new();
    let angel = fixture.0.join(".angelX");
    let store = angel.join("trajectories");
    std::fs::create_dir_all(&store).unwrap();
    for path in [&angel, &store] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o775)).unwrap();
    }
    crate::agent::harness::ensure_private_store(&store);
    assert_eq!(mode(&angel), 0o700);
    assert_eq!(mode(&store), 0o700);
    let custom_parent = fixture.0.join("custom-parent");
    let custom = custom_parent.join("store");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::set_permissions(&custom_parent, std::fs::Permissions::from_mode(0o750)).unwrap();
    std::fs::set_permissions(&custom, std::fs::Permissions::from_mode(0o775)).unwrap();
    crate::agent::harness::ensure_private_store(&custom);
    assert_eq!(mode(&custom), 0o700);
    assert_eq!(mode(&custom_parent), 0o750);
    let outside = fixture.0.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o755)).unwrap();
    let alias = fixture.0.join("alias");
    symlink(&outside, &alias).unwrap();
    crate::agent::harness::ensure_private_store(&alias);
    crate::agent::harness::ensure_private_store(&alias.join("missing"));
    assert_eq!(mode(&outside), 0o755);
    assert!(!outside.join("missing").exists());
}
