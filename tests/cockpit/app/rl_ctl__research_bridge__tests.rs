use super::*;

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn sloptomizer_nonregular_store_fixture() {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::FileTypeExt as _;

    let root = std::env::var_os("ANGEL_T_RESEARCH_FIFO")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = crate::agent::process_test_support::FixtureCleanup::new();
    let root = PathBuf::from(root);
    let state = root.join("state.json");
    let name = std::ffi::CString::new(state.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let worker_cancel = std::sync::Arc::clone(&cancel);
    let worker_root = root.clone();
    let (send, receive) = std::sync::mpsc::channel();
    let started = std::time::Instant::now();
    let worker = std::thread::spawn(move || {
        send.send(transform(
            &worker_root,
            json!({"action":"suggest"}),
            &worker_cancel,
        ))
        .unwrap();
    });
    let first = receive.recv_timeout(Duration::from_millis(500));
    let blocked = first.is_err();
    let result = match first {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // Release the old blocking-open implementation before failing the
            // regression, so it leaves no stuck worker. The fixture's outer
            // process deadline also bounds assertion/setup failures.
            cancel.store(true, Ordering::Release);
            let writer = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&state)
                .unwrap();
            let result = receive.recv_timeout(Duration::from_secs(1)).unwrap();
            drop(writer);
            result
        }
        Err(error) => panic!("research worker disconnected: {error}"),
    };
    worker.join().unwrap();
    assert!(!blocked, "regular-file check waited for a FIFO writer");
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(
        result
            .unwrap_err()
            .contains("research store entry is not a regular file")
    );
    assert!(
        std::fs::symlink_metadata(&state)
            .unwrap()
            .file_type()
            .is_fifo()
    );

    let plain = root.join("plain.json");
    let bytes = b"{\"receipt\":\"exact bytes\"}\n\0\xff";
    std::fs::write(&plain, bytes).unwrap();
    assert_eq!(read(&plain).unwrap(), bytes);
    let linked = root.join("linked.json");
    std::os::unix::fs::symlink(&plain, &linked).unwrap();
    assert!(read(&linked).is_err(), "O_NOFOLLOW must remain in force");
    assert_eq!(std::fs::read(plain).unwrap(), bytes);
}

#[cfg(target_os = "linux")]
#[test]
fn sloptomizer_rejects_fifo_state_without_blocking_and_preserves_plain_reads() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::sloptomizer_nonregular_store_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_RESEARCH_FIFO",
    );
}

#[test]
fn sloptomizer_bundled_originals_match_source_receipt() {
    let manifest: Value =
        serde_json::from_slice(FILES.iter().find(|(p, _)| *p == "UPSTREAM.json").unwrap().1)
            .unwrap();
    assert_eq!(
        manifest["source_head"],
        "0e3c17b539bf14ffa6c546f25b648bb58b4514ac"
    );
    let rows = manifest["files"].as_array().unwrap();
    assert_eq!(rows.len(), 10);
    for row in rows {
        let (_, bytes) = FILES
            .iter()
            .find(|(p, _)| Some(*p) == row["path"].as_str())
            .unwrap();
        assert_eq!(
            crate::knowledge::cut::sha256_hex(bytes),
            row["sha256"].as_str().unwrap()
        );
    }
}

#[test]
fn sloptomizer_concurrent_feedback_is_durable_deduplicated_and_failure_preserves_state() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-slop-store-{}", new_run_id()));
    let _env = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.to_str().unwrap());
    let scope = root.join("learning");
    // Synthetic bridge inputs test storage, not objective verification. Only
    // the production controller admits observations through physical evidence.
    let event = |i| {
        json!({"action":"observe","task":"fixture objective","observation":{
            "id":format!("synthetic-{i}"),"idea":format!("fixture idea {i}"),"approach":"fixture",
            "task":"fixture objective","receipt_sha256":"synthetic-receipt","source_sha256":"synthetic-source","passed":true
        }})
    };
    let workers = (0..4)
        .map(|i| {
            let scope = scope.clone();
            let request = event(i);
            std::thread::spawn(move || transform(&scope, request, &AtomicBool::new(false)).unwrap())
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
    let repeated = transform(&scope, event(0), &AtomicBool::new(false)).unwrap();
    assert_eq!(repeated["advice"]["observations"], 4);
    assert_eq!(repeated["updated"], false);
    let before = read(&scope.join("state.json")).unwrap();
    let mut conflict = event(0);
    conflict["observation"]["passed"] = json!(false);
    assert!(
        transform(&scope, conflict, &AtomicBool::new(false))
            .unwrap_err()
            .contains("conflicting observation")
    );
    assert_eq!(read(&scope.join("state.json")).unwrap(), before);
    assert!(
        transform(&scope, event(4), &AtomicBool::new(true))
            .unwrap_err()
            .contains("cancelled")
    );
    assert_eq!(read(&scope.join("state.json")).unwrap(), before);
    write(&scope.join("state.json"), b"null").unwrap();
    assert!(
        transform(&scope, json!({"action":"suggest"}), &AtomicBool::new(false))
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(read(&scope.join("state.json")).unwrap(), b"null");
    std::fs::remove_dir_all(&root).unwrap();
}
