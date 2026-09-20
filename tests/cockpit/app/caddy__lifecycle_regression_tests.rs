// Included in the existing Caddy storage regression module for owned fixtures
// and its thread-local, consumed-once metadata hook.
fn lifecycle_hazard(command: &str) -> Hazard {
    Hazard {
        ts_ms: now_ms(),
        command: command.into(),
        diagnostic: "owned fixture".into(),
        tool: "shell".into(),
    }
}

fn lifecycle_seed(fixture: &Fixture) -> Vec<u8> {
    let bytes = (serde_json::to_string(&lifecycle_hazard("old")).unwrap() + "\n").into_bytes();
    std::fs::write(fixture.0.join("hazards.jsonl"), &bytes).unwrap();
    bytes
}

fn lifecycle_child(test: &str, key: &str, path: &Path) -> std::process::Output {
    use std::process::{Command, Stdio};
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(key, path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            if output.status.success() {
                assert!(
                    String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed;"),
                    "child must execute exactly one owned control"
                );
            }
            return output;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("owned fixture child exceeded 10s");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn caddy_lifecycle_concurrent_same_command_commits_at_most_once() {
    for _ in 0..8 {
        let fixture = Fixture::new();
        lifecycle_seed(&fixture);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let results = std::thread::scope(|scope| {
            let handles = (0..2)
                .map(|_| {
                    let barrier = barrier.clone();
                    let path = &fixture.0;
                    scope.spawn(move || {
                        barrier.wait();
                        storage::append(path, "hazards.jsonl", &[lifecycle_hazard("same")], None)
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(results.iter().map(|r| r.written).sum::<usize>(), 1);
        assert_eq!(
            load_jsonl::<Hazard>(&fixture.0.join("hazards.jsonl"))
                .iter()
                .filter(|row| row.command == "same")
                .count(),
            1
        );
    }
}

#[test]
fn caddy_lifecycle_process_lock_contention_is_reported_without_waiting() {
    const KEY: &str = "ANGEL_T_CADDY_OWNED_LOCK_CHILD";
    if let Some(path) = std::env::var_os(KEY) {
        let report = storage::append(
            Path::new(&path),
            "hazards.jsonl",
            &[lifecycle_hazard("peer")],
            None,
        );
        assert_eq!(report.status, storage::WriteStatus::Busy);
        assert_eq!(report.written, 0);
        return;
    }
    let fixture = Fixture::new();
    let before = lifecycle_seed(&fixture);
    let lock = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(fixture.0.join("hazards.jsonl.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    // Keep a duplicate description alive. A fork between `spawn` and `exec`
    // has the same effect; this makes the release requirement deterministic.
    let _forked_lock = lock.try_clone().unwrap();
    let output = lifecycle_child(
        "knowledge::caddy::storage_regression_tests::caddy_lifecycle_process_lock_contention_is_reported_without_waiting",
        KEY,
        &fixture.0,
    );
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read(fixture.0.join("hazards.jsonl")).unwrap(),
        before
    );
    // Releasing the advisory lock before closing our descriptor keeps a
    // forked copy from making this post-contention check spuriously Busy.
    lock.unlock().unwrap();
    drop(lock);
    assert_eq!(
        storage::append(
            &fixture.0,
            "hazards.jsonl",
            &[lifecycle_hazard("peer")],
            None
        )
        .written,
        1
    );
    assert_eq!(
        storage::append(
            &fixture.0,
            "hazards.jsonl",
            &[lifecycle_hazard("peer")],
            None
        )
        .written,
        0
    );
}

#[test]
fn caddy_lifecycle_rotation_keeps_complete_newest_rows_and_reports_admission() {
    let fixture = Fixture::new();
    let initial = (0..4000)
        .map(|i| {
            serde_json::to_string(&lifecycle_hazard(&format!("old-{i}-{}", "x".repeat(150))))
                .unwrap()
                + "\n"
        })
        .collect::<String>();
    std::fs::write(fixture.0.join("hazards.jsonl"), initial).unwrap();
    let report = storage::append(
        &fixture.0,
        "hazards.jsonl",
        &[lifecycle_hazard("newest")],
        None,
    );
    assert_eq!(report.status, storage::WriteStatus::Published);
    assert!(report.read_health.clipped_prefix_bytes > 0);
    assert_eq!(report.written, 1);
    assert!(
        std::fs::metadata(fixture.0.join("hazards.jsonl"))
            .unwrap()
            .len()
            <= STORE_TAIL_BYTES
    );
    let loaded = storage::load::<Hazard>(&fixture.0.join("hazards.jsonl"));
    assert!(!loaded.health.degraded());
    assert_eq!(loaded.rows.last().unwrap().command, "newest");
    let many = (0..2000)
        .map(|i| lifecycle_hazard(&format!("batch-{i}")))
        .collect::<Vec<_>>();
    let report = storage::append(&fixture.0, "hazards.jsonl", &many, None);
    assert_eq!(report.skipped_new, 976);
    assert_eq!(report.written, 1024);
}

#[test]
fn caddy_lifecycle_corruption_is_explicit_and_refuses_destructive_repair() {
    use std::io::Write as _;
    let fixture = Fixture::new();
    lifecycle_seed(&fixture);
    let path = fixture.0.join("hazards.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    file.write_all(
        b"{broken}\n{\"missing-fields\":true}\n{\"command\":\"bad-\xff\"}\n{\"partial\":",
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let loaded = storage::load::<Hazard>(&path);
    assert_eq!(loaded.rows.len(), 1);
    assert_eq!(
        (
            loaded.health.malformed_rows,
            loaded.health.invalid_utf8_rows
        ),
        (3, 1)
    );
    assert!(loaded.health.unterminated_tail && loaded.health.degraded());
    let report = storage::append(
        &fixture.0,
        "hazards.jsonl",
        &[lifecycle_hazard("new")],
        None,
    );
    assert_eq!(report.status, storage::WriteStatus::CorruptInput);
    assert_eq!(report.written, 0);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn caddy_lifecycle_oversized_record_and_foreign_temporary_preserve_snapshot() {
    let fixture = Fixture::new();
    let before = lifecycle_seed(&fixture);
    let path = fixture.0.join("hazards.jsonl");
    let mut oversized = lifecycle_hazard("oversized");
    oversized.diagnostic = "x".repeat(32 * 1024);
    let report = storage::append(&fixture.0, "hazards.jsonl", &[oversized], None);
    assert_eq!(
        report.status,
        storage::WriteStatus::Failed(std::io::ErrorKind::InvalidData)
    );
    assert_eq!((report.written, report.evicted), (0, 0));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let foreign = fixture.0.join(".hazards.jsonl.next");
    std::fs::write(&foreign, b"foreign preserved bytes").unwrap();
    let report = storage::append(
        &fixture.0,
        "hazards.jsonl",
        &[lifecycle_hazard("new")],
        None,
    );
    assert_eq!(
        report.status,
        storage::WriteStatus::PreservedTemporary(foreign.clone())
    );
    assert_eq!(report.written, 0);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign preserved bytes");
}

#[cfg(target_os = "linux")]
#[test]
fn caddy_lifecycle_lock_release_does_not_depend_on_last_close() {
    use std::os::fd::FromRawFd;
    use std::rc::Rc;

    let fixture = Fixture::new();
    lifecycle_seed(&fixture);
    let lock_path = fixture.0.join("hazards.jsonl.lock");
    let duplicate = Rc::new(RefCell::new(None::<std::fs::File>));
    let captured = Rc::clone(&duplicate);
    let _hook = HookGuard;
    AFTER_METADATA.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move |_| {
            // A dup shares the lock's open file description, just as a child
            // does between fork and exec. Keep it alive across both appends.
            for entry in std::fs::read_dir("/proc/self/fd").unwrap().flatten() {
                if std::fs::read_link(entry.path()).ok().as_ref() == Some(&lock_path) {
                    let fd: i32 = entry.file_name().to_str().unwrap().parse().unwrap();
                    // SAFETY: append owns this live fd throughout the hook.
                    let copied = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
                    assert!(copied >= 0, "{}", std::io::Error::last_os_error());
                    // SAFETY: fcntl returned a new descriptor owned by this test.
                    *captured.borrow_mut() = Some(unsafe { std::fs::File::from_raw_fd(copied) });
                    return;
                }
            }
            panic!("append's lock descriptor was not found");
        }));
    });
    assert_eq!(
        storage::append(
            &fixture.0,
            "hazards.jsonl",
            &[lifecycle_hazard("first")],
            None
        )
        .written,
        1
    );
    assert!(duplicate.borrow().is_some());
    let second = storage::append(
        &fixture.0,
        "hazards.jsonl",
        &[lifecycle_hazard("second")],
        None,
    );
    assert_eq!(second.status, storage::WriteStatus::Published);
    assert_eq!(second.written, 1);
}

#[cfg(unix)]
#[test]
fn caddy_lifecycle_real_efbig_preserves_old_file_and_cleans_owned_stage() {
    const KEY: &str = "ANGEL_T_CADDY_OWNED_EFBIG_CHILD";
    if let Some(path) = std::env::var_os(KEY) {
        let dir = Path::new(&path);
        let file = dir.join("hazards.jsonl");
        let before = std::fs::read(&file).unwrap();
        unsafe {
            assert_ne!(libc::signal(libc::SIGXFSZ, libc::SIG_IGN), libc::SIG_ERR);
            let limit = libc::rlimit {
                rlim_cur: before.len() as u64 + 17,
                rlim_max: before.len() as u64 + 17,
            };
            assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &limit), 0);
        }
        let report = storage::append(
            dir,
            "hazards.jsonl",
            &[lifecycle_hazard("new-long-enough-to-hit-limit")],
            None,
        );
        assert_eq!(
            report.status,
            storage::WriteStatus::Failed(std::io::ErrorKind::FileTooLarge)
        );
        assert_eq!(report.written, 0);
        assert_eq!(std::fs::read(&file).unwrap(), before);
        assert!(!dir.join(".hazards.jsonl.next").exists());
        return;
    }
    let fixture = Fixture::new();
    lifecycle_seed(&fixture);
    let output = lifecycle_child(
        "knowledge::caddy::storage_regression_tests::caddy_lifecycle_real_efbig_preserves_old_file_and_cleans_owned_stage",
        KEY,
        &fixture.0,
    );
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn caddy_lifecycle_reader_retains_its_open_snapshot_across_publication() {
    let fixture = Fixture::new();
    let before = lifecycle_seed(&fixture);
    let target = fixture.0.clone();
    let _hook = HookGuard;
    AFTER_METADATA.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move |_| {
            assert_eq!(
                storage::append(&target, "hazards.jsonl", &[lifecycle_hazard("new")], None).written,
                1
            );
        }))
    });
    let bytes = storage::tail_bytes(&fixture.0.join("hazards.jsonl"), STORE_TAIL_BYTES)
        .unwrap()
        .0;
    assert_eq!(bytes, before);
    assert_eq!(
        storage::load::<Hazard>(&fixture.0.join("hazards.jsonl"))
            .rows
            .len(),
        2
    );
}

#[test]
fn caddy_lifecycle_exact_tail_boundary_preserves_whole_first_record() {
    let fixture = Fixture::new();
    let path = fixture.0.join("tail.jsonl");
    std::fs::write(&path, b"old\nfirst\nlast\n").unwrap();
    assert_eq!(read_tail(&path, 11).as_deref(), Some("first\nlast\n"));
}

#[test]
fn caddy_lifecycle_oversized_unterminated_tail_remains_corrupt_after_clipping() {
    let fixture = Fixture::new();
    let path = fixture.0.join("hazards.jsonl");
    let broken = vec![b'x'; STORE_TAIL_BYTES as usize + 100];
    std::fs::write(&path, &broken).unwrap();
    let loaded = storage::load::<Hazard>(&path);
    assert!(loaded.rows.is_empty());
    assert!(loaded.health.clipped_prefix_bytes > 0);
    assert!(loaded.health.unterminated_tail && loaded.health.degraded());
    let report = storage::append(
        &fixture.0,
        "hazards.jsonl",
        &[lifecycle_hazard("new")],
        None,
    );
    assert_eq!(report.status, storage::WriteStatus::CorruptInput);
    assert_eq!(report.written, 0);
    assert_eq!(std::fs::read(&path).unwrap(), broken);
}

#[cfg(unix)]
#[test]
fn caddy_lifecycle_nonregular_paths_report_io_without_opening_a_fifo() {
    use std::os::unix::ffi::OsStrExt;
    let fixture = Fixture::new();
    let fifo = fixture.0.join("hazards.jsonl.lock");
    let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let report = storage::append(
        &fixture.0,
        "hazards.jsonl",
        &[lifecycle_hazard("new")],
        None,
    );
    assert_eq!(
        report.status,
        storage::WriteStatus::Failed(std::io::ErrorKind::InvalidInput)
    );
    std::fs::remove_file(fifo).unwrap();
    let data = fixture.0.join("hazards.jsonl");
    std::fs::create_dir(&data).unwrap();
    assert_eq!(
        storage::load::<Hazard>(&data).health.io_error,
        Some(std::io::ErrorKind::InvalidInput)
    );
    let report = storage::append(
        &fixture.0,
        "hazards.jsonl",
        &[lifecycle_hazard("new")],
        None,
    );
    assert_eq!(
        report.status,
        storage::WriteStatus::Failed(std::io::ErrorKind::InvalidInput)
    );
}
