//! Staged for tests/cockpit/app/workspace_store__storage_atomic_tests.rs.
use super::*;
use std::cell::RefCell;

type OpenHook = Box<dyn FnOnce(&Path)>;
thread_local! { static BEFORE_OPEN: RefCell<Option<OpenHook>> = RefCell::new(None); }
pub(super) fn before_atomic_open(path: &Path) {
    if let Some(hook) = BEFORE_OPEN.with(|slot| slot.borrow_mut().take()) {
        hook(path);
    }
}
type RenameHook = Box<dyn FnOnce(&Path)>;
thread_local! { static BEFORE_RENAME: RefCell<Option<RenameHook>> = RefCell::new(None); }
pub(super) fn before_atomic_rename(path: &Path) {
    if let Some(hook) = BEFORE_RENAME.with(|slot| slot.borrow_mut().take()) {
        hook(path);
    }
}
type PublishedHook = Box<dyn FnOnce(&Path) -> Result<(), String>>;
thread_local! { static AFTER_RENAME: RefCell<Option<PublishedHook>> = RefCell::new(None); }
pub(super) fn after_atomic_rename(path: &Path) -> Result<(), String> {
    AFTER_RENAME
        .with(|slot| slot.borrow_mut().take())
        .map_or(Ok(()), |hook| hook(path))
}
/// One-shot fault in the caller's thread, after publication and before the
/// directory durability acknowledgement. Other tests/writers are unaffected.
pub(crate) fn fail_after_publish(path: PathBuf) {
    AFTER_RENAME.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move |published| {
            if published != path {
                fail_after_publish(path);
                return Ok(());
            }
            Err("injected directory sync failure after publish".into())
        }));
    });
}

#[test]
fn private_transaction_lock_excludes_other_processes_without_unlinking_its_inode() {
    const CHILD: &str = "ANGEL_T_PRIVATE_LOCK_ROOT";
    if let Some(root) = std::env::var_os(CHILD) {
        let path = PathBuf::from(root).join("state.json");
        assert!(try_private_lock(&path).unwrap_err().contains("busy"));
        return;
    }
    let fixture = Fixture::new();
    let path = fixture.0.join("state.json");
    let guard = try_private_lock(&path).unwrap();
    assert!(try_private_lock(&path).is_err());
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "platform::workspace_store::storage_atomic_tests::private_transaction_lock_excludes_other_processes_without_unlinking_its_inode", "--test-threads=1"])
        .env(CHILD, &fixture.0).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    assert!(wait_for_child(child, Duration::from_secs(10), "private lock").success());
    drop(guard);
    assert!(fixture.0.join(".state.json.lock").is_file());
    let _next = try_private_lock(&path).unwrap();
}
struct HookGuard;
impl Drop for HookGuard {
    fn drop(&mut self) {
        BEFORE_OPEN.with(|slot| {
            slot.borrow_mut().take();
        });
        BEFORE_RENAME.with(|slot| { slot.borrow_mut().take(); });
        AFTER_RENAME.with(|slot| { slot.borrow_mut().take(); });
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

fn wait_for_child(child: std::process::Child, timeout: Duration, label: &str) -> ExitStatus {
    let pid = child.id();
    let (sender, receiver) = std::sync::mpsc::channel();
    let waiter = thread::spawn(move || {
        let mut child = child;
        let _ = sender.send(child.wait());
    });
    match receiver.recv_timeout(timeout) {
        Ok(status) => {
            let _ = waiter.join();
            status.unwrap()
        }
        Err(_) => {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
            let _ = waiter.join();
            panic!("{label} child timed out after {timeout:?}");
        }
    }
}

#[test]
fn abort_between_temp_write_and_rename_keeps_old_state_and_reads_ignore_temps() {
    const CHILD_ROOT: &str = "ANGEL_T_ATOMIC_ABORT_ROOT";
    fn record(summary: &str) -> crate::platform::guardian::ResurrectionRecord {
        crate::platform::guardian::ResurrectionRecord {
            dead_instance: "dead-1".to_string(),
            dead_host: "rig".to_string(),
            old_session_id: "s-1".to_string(),
            new_session_id: None,
            guardian_host: "rig".to_string(),
            started_ms: 1,
            finished_ms: 2,
            success: true,
            summary: summary.to_string(),
        }
    }
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let peers = PathBuf::from(root);
        // A real abrupt death: no unwind, no cleanup, no rename.
        BEFORE_RENAME.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|_| std::process::abort()))
        });
        crate::platform::guardian::write_record(&peers, &record("never published")).unwrap();
        unreachable!("the abort hook fires before the rename");
    }
    let fixture = Fixture::new();
    let peers = fixture.0.join("peers");
    crate::platform::guardian::write_record(&peers, &record("old complete record")).unwrap();
    let log_path = fixture.0.join("child.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "platform::workspace_store::storage_atomic_tests::abort_between_temp_write_and_rename_keeps_old_state_and_reads_ignore_temps",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CHILD_ROOT, &peers)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let status = wait_for_child(child, Duration::from_secs(10), "atomic abort");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        assert_eq!(
            status.signal(),
            Some(libc::SIGABRT),
            "child did not abort at the hook: {}",
            std::fs::read_to_string(&log_path).unwrap()
        );
    }
    assert!(!status.success());

    // The last complete record is the only published state the crash left.
    let read = crate::platform::guardian::read_record(&peers, "dead-1").unwrap();
    assert_eq!(read.summary, "old complete record");

    // The unpublished body survived only in the crashed temp.
    let temps: Vec<PathBuf> = std::fs::read_dir(&peers)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|candidate| {
            candidate
                .extension()
                .is_some_and(|extension| extension == "tmp")
        })
        .collect();
    assert_eq!(temps.len(), 1, "{temps:?}");
    let unpublished = String::from_utf8(std::fs::read(&temps[0]).unwrap()).unwrap();
    assert!(unpublished.contains("never published"), "{unpublished}");

    // Older crashes may leave torn or truncated staging files. The real read
    // paths open the published name or scan for the record suffix, so neither
    // reader adopts a temp's bytes.
    let torn = peers.join(".dead-1.resurrection.json.999.0.tmp");
    let truncated = peers.join(".dead-1.resurrection.json.999.1.tmp");
    std::fs::write(&torn, b"{\"dead_instance\":\"dead").unwrap();
    std::fs::write(&truncated, b"").unwrap();
    assert_eq!(
        crate::platform::guardian::read_record(&peers, "dead-1")
            .unwrap()
            .summary,
        "old complete record"
    );
    let records = crate::platform::guardian::read_records(&peers);
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0].summary, "old complete record");

    // Recovery is the next complete write: fresh bytes publish and no temp
    // is adopted.
    crate::platform::guardian::write_record(&peers, &record("recovered record")).unwrap();
    assert_eq!(
        crate::platform::guardian::read_record(&peers, "dead-1")
            .unwrap()
            .summary,
        "recovered record"
    );
    assert_eq!(std::fs::read(&torn).unwrap(), b"{\"dead_instance\":\"dead");
    assert_eq!(std::fs::read(&truncated).unwrap(), b"");
    assert_eq!(unpublished, String::from_utf8(std::fs::read(&temps[0]).unwrap()).unwrap());
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

#[test]
fn bulk_atomic_replacement_is_complete_and_leaves_no_temp() {
    let fixture = Fixture::new();
    let path = fixture.0.join("report.txt");
    std::fs::write(&path, b"old report").unwrap();
    let body = b"complete bulk body\n".repeat(500);
    write_atomic(&path, &body).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), body);
    write_atomic(&path, b"shorter").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"shorter");
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn bulk_atomic_rejected_create_new_preserves_old_file() {
    let fixture = Fixture::new();
    let path = fixture.0.join("report.txt");
    write_atomic(&path, b"old report").unwrap();
    let parent = fixture.0.clone();
    let _hook = HookGuard;
    BEFORE_OPEN.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move |temp| {
            assert_eq!(temp.parent(), Some(parent.as_path()));
            std::fs::write(temp, b"foreign staging bytes").unwrap();
        }))
    });
    let error = write_atomic(&path, b"rejected replacement").unwrap_err();
    assert!(error.starts_with("open state temp"), "{error}");
    assert_eq!(std::fs::read(&path).unwrap(), b"old report");
}

#[test]
fn concurrent_bulk_writers_never_publish_torn_bytes() {
    let fixture = Fixture::new();
    let path = fixture.0.join("shared.bin");
    write_atomic(&path, &vec![0_u8; 4096]).unwrap();
    let writers: Vec<_> = (1_u8..=4)
        .map(|tag| {
            let path = path.clone();
            std::thread::spawn(move || {
                let body = vec![tag; 8192];
                for _ in 0..25 {
                    write_atomic(&path, &body).unwrap();
                    let seen = std::fs::read(&path).unwrap();
                    assert!(
                        seen.iter().all(|byte| *byte == seen[0]),
                        "reader observed torn bytes"
                    );
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let final_body = std::fs::read(&path).unwrap();
    assert_eq!(final_body.len(), 8192);
    assert!(final_body.iter().all(|byte| *byte == final_body[0]));
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
        .args(["--exact", "platform::workspace_store::storage_atomic_tests::private_atomic_write_error_cleans_only_owned_temp", "--test-threads=1", "--nocapture"])
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
