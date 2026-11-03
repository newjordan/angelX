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

#[test]
fn sloptomizer_publishes_coherent_generations_and_retains_last_good() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-slop-live-{}", new_run_id()));
    let resource = root.join("bundle/cockpit/research/sloptomizer");
    std::fs::create_dir_all(&resource).unwrap();
    for (name, bytes) in FILES {
        let path = resource.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let _env = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.join("rl").to_str().unwrap());
    let _resources = crate::tests::TestEnvGuard::set(
        "ANGEL_RESOURCE_DIR",
        root.join("bundle").to_str().unwrap(),
    );
    let _source = crate::tests::TestEnvGuard::unset("ANGEL_RESEARCH_RUNTIME_DIR");
    let _python = crate::tests::TestEnvGuard::unset("ANGEL_RESEARCH_PYTHON");
    let publisher =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("research/sloptomizer/publish_runtime.py");
    let publish = || {
        let status = std::process::Command::new("python3")
            .arg(&publisher)
            .arg(&resource)
            .status()
            .unwrap();
        assert!(status.success());
    };
    let scope = root.join("learning");
    let call = || {
        transform(
            &scope,
            json!({"action":"suggest", "task":"fixture"}),
            &AtomicBool::new(false),
        )
        .unwrap()
    };
    let baseline = call();
    let first_generation = baseline["runtime_generation"].as_str().unwrap().to_string();
    let runner_path = resource.join("runner.py");
    let original = std::fs::read_to_string(&runner_path).unwrap();
    let edited = original.replace(
        "Past verifier outcomes guide exploration.",
        "Reloaded verifier guidance.",
    );
    assert_ne!(edited, original);
    std::fs::write(&runner_path, &edited).unwrap();
    // A partially saved dependency without a commit manifest is not used.
    let before_publish = call();
    assert_eq!(before_publish["runtime_generation"], first_generation);
    publish();
    let reloaded = call();
    assert_ne!(reloaded["runtime_generation"], first_generation);
    assert!(
        reloaded["advice"]["evidence_note"]
            .as_str()
            .unwrap()
            .contains("Reloaded verifier guidance")
    );
    let accepted_generation = reloaded["runtime_generation"].as_str().unwrap().to_string();
    let old_path = root
        .join("rl/sloptomizer-runtime")
        .join(&first_generation)
        .join("runner.py");
    assert_eq!(
        std::fs::read_to_string(old_path).unwrap(),
        original,
        "in-flight generations remain intact"
    );

    // A later partial edit leaves the accepted snapshot active and reports why.
    std::fs::write(&runner_path, "def broken(:\n").unwrap();
    let partial = call();
    assert_eq!(partial["runtime_generation"], accepted_generation);
    assert!(
        partial["runtime_warning"]
            .as_str()
            .unwrap()
            .contains("digest mismatch")
    );
    publish();
    let invalid = call();
    assert_eq!(invalid["runtime_generation"], accepted_generation);
    assert!(
        invalid["runtime_warning"]
            .as_str()
            .unwrap()
            .contains("validation failed")
    );
    assert!(
        invalid["advice"]["evidence_note"]
            .as_str()
            .unwrap()
            .contains("Reloaded verifier guidance")
    );
    std::fs::remove_file(resource.join("generation.json")).unwrap();
    let missing = call();
    assert_eq!(missing["runtime_generation"], accepted_generation);
    assert!(
        missing["runtime_warning"]
            .as_str()
            .unwrap()
            .contains("missing")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sloptomizer_generation_manifest_rejects_mixed_or_incomplete_dependencies() {
    let root = std::env::temp_dir().join(format!("angel-slop-snapshot-{}", new_run_id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut entries = serde_json::Map::new();
    for (name, bytes) in FILES {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
        entries.insert(
            (*name).to_string(),
            json!(crate::knowledge::cut::sha256_hex(bytes)),
        );
    }
    let manifest = json!({"schema":GENERATION_SCHEMA, "files":entries});
    std::fs::write(
        root.join("generation.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert_eq!(
        snapshot_candidate(&root).unwrap().unwrap(),
        embedded_files()
    );
    std::fs::write(root.join("relations.py"), "partial").unwrap();
    assert!(
        snapshot_candidate(&root)
            .unwrap_err()
            .contains("digest mismatch")
    );
    std::fs::remove_file(root.join("runner.py")).unwrap();
    assert!(snapshot_candidate(&root).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sloptomizer_candidate_validation_does_not_execute_runtime_code() {
    let root = std::env::temp_dir().join(format!("angel-slop-static-{}", new_run_id()));
    std::fs::create_dir_all(&root).unwrap();
    for (name, bytes) in FILES {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let runner = root.join("runner.py");
    let original = std::fs::read_to_string(&runner).unwrap();
    let marker = root.join("candidate-executed");
    let statement = format!(
        "import copy\nopen({}, 'w').write('ran')\n",
        serde_json::to_string(marker.to_str().unwrap()).unwrap()
    );
    std::fs::write(&runner, original.replace("import copy\n", &statement)).unwrap();
    validate_candidate(&root, &AtomicBool::new(false)).unwrap();
    assert!(
        !marker.exists(),
        "preparation must not execute candidate code"
    );
    std::fs::write(
        &runner,
        original.replace("import copy\n", "import angel_definitely_missing_module\n"),
    )
    .unwrap();
    assert!(validate_candidate(&root, &AtomicBool::new(false)).is_err());
    assert!(!marker.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sloptomizer_trial_failure_preserves_state_and_retries_unchanged_candidate() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-slop-trial-{}", new_run_id()));
    let source = root.join("source");
    std::fs::create_dir_all(&source).unwrap();
    for (name, bytes) in FILES {
        let path = source.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let _env = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.join("rl").to_str().unwrap());
    let _source =
        crate::tests::TestEnvGuard::set("ANGEL_RESEARCH_RUNTIME_DIR", source.to_str().unwrap());
    let _python = crate::tests::TestEnvGuard::unset("ANGEL_RESEARCH_PYTHON");
    let scope = root.join("learning");
    let observe = json!({"action":"observe", "task":"fixture", "observation":{
        "id":"trial", "idea":"fixture", "approach":"fixture", "task":"fixture",
        "receipt_sha256":"receipt", "source_sha256":"source", "passed":true
    }});
    transform(&scope, observe, &AtomicBool::new(false)).unwrap();
    let before = read(&scope.join("state.json")).unwrap();
    let runner = source.join("runner.py");
    let original = std::fs::read_to_string(&runner).unwrap();
    let sentinel = root.join("failure-enabled");
    std::fs::write(&sentinel, "fail").unwrap();
    let code = format!(
        "import copy\nif __import__('os').path.exists({}):\n    raise RuntimeError('trial failure')\n",
        serde_json::to_string(sentinel.to_str().unwrap()).unwrap()
    );
    std::fs::write(&runner, original.replace("import copy\n", &code)).unwrap();
    let publisher =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("research/sloptomizer/publish_runtime.py");
    assert!(
        std::process::Command::new("python3")
            .arg(publisher)
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    assert!(transform(&scope, json!({"action":"suggest"}), &AtomicBool::new(false)).is_err());
    assert_eq!(read(&scope.join("state.json")).unwrap(), before);
    let fallback = transform(&scope, json!({"action":"suggest"}), &AtomicBool::new(false)).unwrap();
    assert!(
        fallback["runtime_warning"]
            .as_str()
            .unwrap()
            .contains("trial failed")
    );
    std::fs::remove_file(sentinel).unwrap();
    let retry = transform(&scope, json!({"action":"suggest"}), &AtomicBool::new(false)).unwrap();
    assert_ne!(retry["runtime_generation"], fallback["runtime_generation"]);
    assert_eq!(read(&scope.join("state.json")).unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sloptomizer_inflight_trial_does_not_switch_other_requests() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-slop-inflight-{}", new_run_id()));
    let source = root.join("source");
    std::fs::create_dir_all(&source).unwrap();
    for (name, bytes) in FILES {
        let path = source.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let _env = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.join("rl").to_str().unwrap());
    let _source =
        crate::tests::TestEnvGuard::set("ANGEL_RESEARCH_RUNTIME_DIR", source.to_str().unwrap());
    let _python = crate::tests::TestEnvGuard::unset("ANGEL_RESEARCH_PYTHON");
    let (old, _, none) = runtime(&AtomicBool::new(false)).unwrap();
    assert!(none.is_none());
    let path = source.join("runner.py");
    let original = std::fs::read(&path).unwrap();
    let mut edited = original.clone();
    edited.extend_from_slice(b"\n# next generation\n");
    std::fs::write(&path, &edited).unwrap();
    let publisher =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("research/sloptomizer/publish_runtime.py");
    assert!(
        std::process::Command::new("python3")
            .arg(publisher)
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    let (candidate, _, trial) = runtime(&AtomicBool::new(false)).unwrap();
    let mut trial = trial.expect("new generation must be a trial");
    assert_ne!(candidate, old);
    let old_for_thread = old.clone();
    std::thread::spawn(move || {
        let (selected, _, trial) = runtime(&AtomicBool::new(false)).unwrap();
        assert_eq!(selected, old_for_thread);
        assert!(trial.is_none());
    })
    .join()
    .unwrap();
    assert_eq!(std::fs::read(old.join("runner.py")).unwrap(), original);
    assert_eq!(std::fs::read(candidate.join("runner.py")).unwrap(), edited);
    trial.outcome = Some(true);
    drop(trial);
    let (active, _, trial) = runtime(&AtomicBool::new(false)).unwrap();
    assert_eq!(active, candidate);
    assert!(trial.is_none());
    std::fs::remove_dir_all(root).unwrap();
}
