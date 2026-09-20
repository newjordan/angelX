//! Staged for cockpit/src/workspace_store/storage_atomic_tests.rs.
use super::*;
use std::cell::RefCell;

type OpenHook = Box<dyn FnOnce(&Path)>;
thread_local! { static BEFORE_OPEN: RefCell<Option<OpenHook>> = RefCell::new(None); }
pub(super) fn before_atomic_open(path: &Path) {
    if let Some(hook) = BEFORE_OPEN.with(|slot| slot.borrow_mut().take()) {
        hook(path);
    }
}
struct HookGuard;
impl Drop for HookGuard {
    fn drop(&mut self) {
        BEFORE_OPEN.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "angel-atomic-regression-{}-{}-{serial}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rejected_create_new_preserves_foreign_temp_and_old_checkpoint() {
    let fixture = Fixture::new();
    let path = fixture.0.join("state.json");
    write_private_atomic(&path, b"old checkpoint").unwrap();
    let collision = std::rc::Rc::new(RefCell::new(None));
    let captured = std::rc::Rc::clone(&collision);
    let parent = fixture.0.clone();
    let _hook = HookGuard;
    BEFORE_OPEN.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move |temp| {
            assert_eq!(temp.parent(), Some(parent.as_path()));
            std::fs::write(temp, b"foreign staging bytes").unwrap();
            *captured.borrow_mut() = Some(temp.to_path_buf());
        }))
    });
    let error = write_private_atomic(&path, b"rejected replacement").unwrap_err();
    assert!(error.starts_with("open state temp"), "{error}");
    assert_eq!(std::fs::read(&path).unwrap(), b"old checkpoint");
    let foreign = collision.borrow().clone().expect("collision hook ran");
    assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign staging bytes");
    write_private_atomic(&path, b"successful next attempt").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"successful next attempt");
    assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign staging bytes");
}

#[cfg(unix)]
#[test]
fn successful_atomic_replacement_is_complete_private_and_leaves_no_temp() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    let path = fixture.0.join("state.json");
    std::fs::write(&path, b"old").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let body = b"complete new state\n".repeat(1000);
    write_private_atomic(&path, &body).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), body);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn private_atomic_write_error_cleans_only_owned_temp() {
    const CHILD_ROOT: &str = "ANGEL_T_PRIVATE_ATOMIC_EFBIG_ROOT";
    const MARKER: &str = "owned-private-atomic-efbig/v1";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("angel-atomic-regression-")
        );
        assert_eq!(
            std::fs::read_to_string(root.join("marker")).unwrap(),
            MARKER
        );
        struct RestoreLimit {
            limit: libc::rlimit,
            signal: libc::sighandler_t,
        }
        impl Drop for RestoreLimit {
            fn drop(&mut self) {
                unsafe {
                    let _ = libc::setrlimit(libc::RLIMIT_FSIZE, &self.limit);
                    libc::signal(libc::SIGXFSZ, self.signal);
                }
            }
        }
        let restore = unsafe {
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            assert_eq!(libc::getrlimit(libc::RLIMIT_FSIZE, &mut limit), 0);
            let signal = libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            assert_ne!(signal, libc::SIG_ERR);
            RestoreLimit { limit, signal }
        };
        let limited = libc::rlimit {
            rlim_cur: 65_536,
            rlim_max: restore.limit.rlim_max,
        };
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limited) }, 0);
        let path = root.join("state.json");
        let original = std::fs::read(&path).unwrap();
        let error = write_private_atomic(&path, &vec![b'x'; 300_000]).unwrap_err();
        assert!(
            error.starts_with("write state temp")
                && error.contains(&std::io::Error::from_raw_os_error(libc::EFBIG).to_string()),
            "{error}"
        );
        assert_eq!(std::fs::read(path).unwrap(), original);
        assert!(
            std::fs::read_dir(&root)
                .unwrap()
                .map(|entry| entry.unwrap())
                .all(|entry| entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "tmp"))
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
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("marker"), MARKER).unwrap();
    let path = fixture.0.join("state.json");
    write_private_atomic(&path, b"last good checkpoint").unwrap();
    let log = std::fs::File::create(fixture.0.join("child.log")).unwrap();
    let mut child = Reap(Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "workspace_store::storage_atomic_tests::private_atomic_write_error_cleans_only_owned_temp", "--test-threads=1", "--nocapture"])
        .env(CHILD_ROOT, &fixture.0).stdin(Stdio::null()).stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "owned write-failure child timed out"
        );
        thread::sleep(Duration::from_millis(5));
    };
    assert!(
        status.success(),
        "{}",
        std::fs::read_to_string(fixture.0.join("child.log")).unwrap()
    );
    assert_eq!(std::fs::read(path).unwrap(), b"last good checkpoint");
}
