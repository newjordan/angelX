use super::*;

struct TemporaryLogStore(PathBuf);

impl TemporaryLogStore {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel-proc-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        Self(std::fs::canonicalize(root).unwrap())
    }
}

impl Drop for TemporaryLogStore {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Lifecycle tests must never inherit the user's private process store.
struct TestProcStore {
    root: TemporaryLogStore,
    saved: Vec<(&'static str, Option<OsString>)>,
    _env_lock: std::sync::MutexGuard<'static, ()>,
}

impl TestProcStore {
    fn new() -> Self {
        let env_lock = crate::tests::env_lock();
        let root = TemporaryLogStore::new("lifecycle");
        let saved = ["ANGEL_PROC_DIR", "ANGEL_PROC_RECEIPTS"]
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect();
        // SAFETY: process-global test environment changes hold env_lock.
        unsafe {
            std::env::set_var("ANGEL_PROC_DIR", root.0.join("process-store"));
            std::env::set_var("ANGEL_PROC_RECEIPTS", "1");
        }
        Self {
            root,
            saved,
            _env_lock: env_lock,
        }
    }

    fn runner(&self) -> ProcRunTool {
        ProcRunTool::in_dir(self.root.0.clone())
    }
}

impl Drop for TestProcStore {
    fn drop(&mut self) {
        // Reap only children belonging to this temporary project, including
        // failures before the test reached its explicit proc_stop call.
        if let Ok(mut procs) = table().lock() {
            let ids: Vec<_> = procs
                .iter()
                .filter(|(_, entry)| entry.project_root == self.root.0)
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                if let Some(mut entry) = procs.remove(&id)
                    && matches!(entry.child.try_wait(), Ok(None))
                {
                    // SAFETY: this live Child still owns its private process group.
                    unsafe { libc::killpg(entry.pid as libc::pid_t, libc::SIGKILL) };
                    let _ = entry.child.kill();
                    let _ = entry.child.wait();
                }
            }
        }
        for receipt in load_receipts() {
            if receipt.project_root == self.root.0 && receipt_alive(&receipt) {
                // SAFETY: fixture receipts pin the live child's starttime.
                unsafe {
                    libc::killpg(receipt.pgid, libc::SIGKILL);
                    libc::waitpid(receipt.pid as libc::pid_t, std::ptr::null_mut(), 0);
                }
            }
        }
        if let Ok(mut notices) = completion_notices().lock() {
            notices.remove(&self.root.0);
        }
        for (key, value) in &self.saved {
            // SAFETY: the fixture still owns the global environment lock.
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

#[test]
fn completed_child_reaped_without_status_and_terminal_receipt_survives_restart() {
    let _env_lock = crate::tests::env_lock();
    struct RestoreEnv(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }
    let _restore = RestoreEnv(
        ["ANGEL_PROC_DIR", "ANGEL_PROC_RECEIPTS"]
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect(),
    );
    let root = std::env::temp_dir().join(format!(
        "angel-proc-completion-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    unsafe {
        std::env::set_var("ANGEL_PROC_DIR", root.join("process-store"));
        std::env::set_var("ANGEL_PROC_RECEIPTS", "1");
    }
    let run = ProcRunTool::in_dir(root.clone());
    let out = run
        .call(&serde_json::json!({
            "command": "printf 'SAT: no UNSAT proof at this m\\n'; exit 2",
            "name": "sat-proof"
        }))
        .unwrap();
    let (id, pid) = parse_handle(&out);
    let foreign = root.join("different-project");
    let deadline = Instant::now() + Duration::from_secs(5);
    let completion = loop {
        assert!(take_completions(&foreign, 8).is_empty());
        if let Some(notice) = take_completions(&root, 8).into_iter().find(|n| n.id == id) {
            break notice;
        }
        assert!(
            Instant::now() < deadline,
            "no independent completion notification"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(completion.exit_code, Some(2));
    assert_eq!(completion.state, "exited 2");
    assert!(completion.message().contains("not benchmark acceptance"));
    assert!(
        take_completions(&root, 8).is_empty(),
        "completion must be one-shot"
    );
    #[cfg(target_os = "linux")]
    assert!(
        proc_stat_fields(pid).is_none(),
        "child must be reaped without proc_status"
    );
    let terminal = completion.receipt.as_ref().expect("durable receipt path");
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(terminal).unwrap()).unwrap();
    assert_eq!(stored["exit_code"], 2);
    assert_eq!(stored["acceptance"], "unverified");
    assert_eq!(stored["status"], "Failed");
    assert_eq!(stored["verification"], "Failed");
    assert!(!receipts_dir().join(format!("{id}-{pid}.json")).exists());
    table().lock().unwrap().remove(&id); // simulate a restarted cockpit
    for _ in 0..2 {
        let status = ProcStatusTool::new(root.clone())
            .call(&serde_json::json!({"id":id}))
            .expect_err("a retained nonzero exit must remain a failed tool receipt");
        assert!(
            status.contains("exited 2 (retained terminal receipt)"),
            "{status}"
        );
        assert!(!status.contains("vanished"), "{status}");
    }
    assert!(
        ProcStatusTool::new(foreign)
            .call(&serde_json::json!({"id":id}))
            .is_err()
    );
    let stopped = ProcStopTool::new(root.clone())
        .call(&serde_json::json!({"id":id}))
        .unwrap();
    assert!(stopped.contains("already exited 2"), "{stopped}");
    assert!(
        terminal.exists(),
        "query and stop must preserve terminal evidence"
    );
    remove_receipt(terminal);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn completion_queue_backpressure_does_not_evict_other_results() {
    let mut notices = BTreeMap::new();
    let completion = ProcCompletion {
        id: 1,
        name: "proof".into(),
        state: "exited 2".into(),
        exit_code: Some(2),
        log: "proof.log".into(),
        receipt: None,
        persistence_error: None,
    };
    let first = Path::new("/project-0");
    for project in 0..8 {
        let workspace = PathBuf::from(format!("/project-{project}"));
        for _ in 0..COMPLETION_NOTICES_PER_PROJECT {
            assert!(enqueue_completion_into(
                &mut notices,
                &workspace,
                &completion
            ));
        }
    }
    assert!(!enqueue_completion_into(&mut notices, first, &completion));
    assert!(!enqueue_completion_into(
        &mut notices,
        Path::new("/new-project"),
        &completion
    ));
    assert_eq!(
        notices.values().map(VecDeque::len).sum::<usize>(),
        COMPLETION_NOTICES_TOTAL
    );
    notices.get_mut(first).unwrap().pop_front();
    assert!(enqueue_completion_into(&mut notices, first, &completion));
    assert_eq!(
        notices.len(),
        8,
        "full queues must not evict or create another project"
    );
}

#[test]
fn sanitize_name_strips_shell_junk() {
    assert_eq!(sanitize_name("vllm serve ../x"), "vllm-serve----x");
    assert_eq!(sanitize_name("  "), "proc");
    assert_eq!(sanitize_name("llama-server"), "llama-server");
    assert!(sanitize_name(&"x".repeat(100)).len() <= 24);
}

#[test]
fn tail_lines_takes_the_end() {
    assert_eq!(tail_lines("a\nb\nc\nd", 2), "c\nd");
    assert_eq!(tail_lines("a", 5), "a");
    assert_eq!(tail_lines("", 5), "");
}

#[test]
fn rotating_log_retains_exactly_two_bounded_segments() {
    let store = TemporaryLogStore::new("rotate");
    let dir = store.0.clone();
    let path = dir.join("service.log");
    let mut log = RotatingLog::new(path.clone(), 64).unwrap();
    log.append(b"first-line-aaaaaaaaaaaaaaaaaaaa\n").unwrap();
    log.append(b"second-line-bbbbbbbbbbbbbbbbbbb\n").unwrap();
    log.append(b"third-line-cccccccccccccccccccc\n").unwrap();
    let rotated = rotated_log_path(&path);
    assert!(rotated.exists());
    assert!(std::fs::metadata(&path).unwrap().len() <= 32);
    assert!(std::fs::metadata(&rotated).unwrap().len() <= 32);
    assert!(
        std::fs::metadata(&path).unwrap().len() + std::fs::metadata(&rotated).unwrap().len() <= 64
    );
    let tail = read_log_tail(&path, 10);
    assert!(
        tail.contains("second-line") || tail.contains("third-line"),
        "{tail}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn private_rotating_log_creates_and_rotates_owner_only_files() {
    let store = TemporaryLogStore::new("private-modes");
    let directory = store.0.join("new-store/project");
    let path = directory.join("service.log");
    let mut log = RotatingLog::new(path.clone(), 16).unwrap();
    let original = std::fs::metadata(&path).unwrap().ino();
    assert_eq!(std::fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        std::fs::metadata(directory.parent().unwrap())
            .unwrap()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    log.append(b"1234").unwrap();
    log.append(b"5678").unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().ino(), original);
    log.append(b"next").unwrap();
    assert_eq!(std::fs::read(rotated_log_path(&path)).unwrap(), b"12345678");
    assert_eq!(std::fs::read(&path).unwrap(), b"next");
    for candidate in [&path, &rotated_log_path(&path)] {
        let info = std::fs::metadata(candidate).unwrap();
        assert_eq!(info.mode() & 0o777, 0o600);
        assert_eq!(info.uid(), unsafe { libc::geteuid() });
        assert_eq!(info.nlink(), 1);
    }
    log.append(b"again").unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert_eq!(
        std::fs::metadata(rotated_log_path(&path)).unwrap().mode() & 0o777,
        0o600
    );
    let existing = store.0.join("configured-parent");
    std::fs::create_dir(&existing).unwrap();
    std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o750)).unwrap();
    let _log = RotatingLog::new(existing.join("existing-parent.log"), 8).unwrap();
    assert_eq!(std::fs::metadata(existing).unwrap().mode() & 0o777, 0o750);
}

#[test]
fn private_rotating_log_rejects_existing_files_and_symlink_ancestors() {
    use std::os::unix::fs::symlink;
    let store = TemporaryLogStore::new("private-create");
    let outside = store.0.join("outside.txt");
    std::fs::write(&outside, b"do not truncate").unwrap();
    let link = store.0.join("linked.log");
    symlink(&outside, &link).unwrap();
    assert!(RotatingLog::new(link, 64).is_err());
    let hardlink = store.0.join("hardlinked.log");
    std::fs::hard_link(&outside, &hardlink).unwrap();
    assert!(RotatingLog::new(hardlink, 64).is_err());
    assert!(RotatingLog::new(outside.clone(), 64).is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), b"do not truncate");
    let target = store.0.join("target");
    std::fs::create_dir(&target).unwrap();
    let alias = store.0.join("alias");
    symlink(&target, &alias).unwrap();
    assert!(RotatingLog::new(alias.join("nested/service.log"), 64).is_err());
    assert!(std::fs::read_dir(target).unwrap().next().is_none());
}

#[test]
fn private_rotating_log_rejects_append_and_rotation_substitutions() {
    use std::os::unix::fs::symlink;
    let store = TemporaryLogStore::new("private-substitution");
    let outside = store.0.join("outside.txt");
    std::fs::write(&outside, b"outside data").unwrap();
    let path = store.0.join("service.log");
    let mut log = RotatingLog::new(path.clone(), 8).unwrap();
    log.append(b"abcd").unwrap();
    std::fs::remove_file(&path).unwrap();
    symlink(&outside, &path).unwrap();
    assert!(log.append(b"more").is_err());
    assert_eq!(read_log_tail(&path, 10), "(log unreadable)");
    let path = store.0.join("rotation.log");
    let mut log = RotatingLog::new(path.clone(), 8).unwrap();
    log.append(b"abcd").unwrap();
    symlink(&outside, rotated_log_path(&path)).unwrap();
    assert!(log.append(b"more").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"abcd");
    let path = store.0.join("hardlink.log");
    let mut log = RotatingLog::new(path.clone(), 8).unwrap();
    log.append(b"abcd").unwrap();
    std::fs::hard_link(&path, store.0.join("second-name")).unwrap();
    assert!(log.append(b"more").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"abcd");
    assert_eq!(std::fs::read(outside).unwrap(), b"outside data");
}

#[test]
fn private_rotating_log_parent_swap_stays_on_pinned_directory() {
    use std::os::unix::fs::symlink;
    let store = TemporaryLogStore::new("private-parent");
    let parent = store.0.join("logs");
    let path = parent.join("service.log");
    let mut log = RotatingLog::new(path.clone(), 8).unwrap();
    log.append(b"abcd").unwrap();
    let retained = store.0.join("retained");
    std::fs::rename(&parent, &retained).unwrap();
    let outside = store.0.join("outside");
    std::fs::create_dir(&outside).unwrap();
    symlink(&outside, &parent).unwrap();
    log.append(b"next").unwrap();
    assert_eq!(
        std::fs::read(retained.join("service.log")).unwrap(),
        b"next"
    );
    assert_eq!(
        std::fs::read(retained.join("service.log.1")).unwrap(),
        b"abcd"
    );
    assert!(std::fs::read_dir(outside).unwrap().next().is_none());
    assert_eq!(read_log_tail(&path, 10), "(log unreadable)");
}

#[test]
fn log_pump_drains_after_log_write_failure() {
    struct CountingReader {
        remaining: usize,
        consumed: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Read for CountingReader {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            let read = bytes.len().min(self.remaining);
            bytes[..read].fill(b'x');
            self.remaining -= read;
            self.consumed.fetch_add(read, Ordering::Relaxed);
            Ok(read)
        }
    }

    let store = TemporaryLogStore::new("drain-failure");
    let dir = store.0.clone();
    let path = dir.join("service.log");
    let log = Arc::new(Mutex::new(RotatingLog::new(path.clone(), 64).unwrap()));

    // Make the first append fail deterministically. The pump must still
    // consume every later byte rather than closing a live child's pipe.
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    let total = 3 * 16 * 1024 + 7;
    let consumed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let warning = Arc::new(Mutex::new(None));
    pump_log(
        CountingReader {
            remaining: total,
            consumed: Arc::clone(&consumed),
        },
        log,
        Arc::clone(&warning),
        "stdout",
    );

    assert_eq!(consumed.load(Ordering::Relaxed), total);
    let warning = warning.lock().unwrap().clone().expect("log warning");
    assert!(warning.contains("stdout log write failed"), "{warning}");
}

fn assert_owned_kill_receipt(reason: &str) {
    let fixture = TestProcStore::new();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let owner = &cancel as *const _ as usize;
    let args = serde_json::json!({"command":"sleep 300", "name":"kill-receipt"});
    let result = fixture
        .runner()
        .call_with_cancel(&args, Some(&cancel))
        .unwrap();
    let (id, pid) = parse_handle(&result);
    crate::harness::note_tool_outcome(
        1,
        "proc_run",
        &args,
        &result,
        "ok",
        false,
        None,
        None,
        None,
        result.len(),
    );
    stop_owned_for(owner, reason);
    let kills = take_owned_kills(owner);
    assert_eq!(kills.len(), 1);
    assert_eq!(kills[0].0, id);
    assert_eq!(kills[0].1.reason, reason);
    assert_eq!(kills[0].1.owner, "turn_owner");
    assert_eq!(kills[0].1.signal, Some(libc::SIGTERM));
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
    assert!(
        take_owned_kills(owner).is_empty(),
        "owner identity released"
    );
    crate::harness::note_proc_kills(&kills);
    let ledger = crate::harness::tool_ledger_snapshot();
    let entry = ledger.iter().find(|entry| entry["proc_id"] == id).unwrap();
    assert_eq!(entry["status"], "killed");
    assert_eq!(entry["kill"]["reason"], reason);
    let terminal: Value = serde_json::from_slice(
        &std::fs::read(
            proc_dir()
                .join("completions")
                .join(format!("{id}-{pid}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(terminal["kill"], entry["kill"]);
    eprintln!(
        "LIFECYCLE_PROC_KILL reason={reason} signal=15 owner=turn_owner ledger=killed reaped=true"
    );
}

#[test]
fn lifecycle_deadline_owner_transfer_survives_nested_unwind_and_terminal_kill() {
    let fixture = TestProcStore::new();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let owner = &cancel as *const _ as usize;
    for terminal in [false, true] {
        let mut handle = None;
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::harness::with_turn_deadline_cancel(
                &cancel,
                Some(Instant::now() + Duration::from_secs(60)),
                |outer| {
                    crate::harness::with_turn_deadline_cancel(
                        outer,
                        Some(Instant::now() + Duration::from_secs(60)),
                        |inner| {
                            let result = fixture
                                .runner()
                                .call_with_cancel(
                                    &serde_json::json!({"command":"sleep 300"}),
                                    Some(inner),
                                )
                                .unwrap();
                            handle = Some(parse_handle(&result));
                            if terminal {
                                stop_owned_for(inner as *const _ as usize, "deadline");
                            }
                            panic!("fixture unwinds after launch");
                        },
                    )
                },
            );
        }));
        assert!(unwind.is_err());
        let (id, pid) = handle.unwrap();
        assert_eq!(table().lock().unwrap().get(&id).unwrap().owner, owner);
        stop_owned_for(owner, "owner_reap");
        let kills = take_owned_kills(owner);
        assert_eq!(kills.len(), 1);
        assert_eq!(kills[0].0, id);
        assert_eq!(
            kills[0].1.reason,
            if terminal { "deadline" } else { "owner_reap" }
        );
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }
}

#[test]
fn successful_turn_keeps_background_job_until_explicit_stop() {
    // env-lock-exempt: TestProcStore owns env_lock through all restoration guards.
    use crate::club::{ChatMsg, Club, ClubReply, ToolCall};
    use std::sync::atomic::AtomicBool;
    let fixture = TestProcStore::new();
    let _advisor = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "0");
    let _skills = crate::tests::TestEnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _relentless = crate::tests::TestEnvGuard::set("ANGEL_RELENTLESS_EXECUTION", "0");
    struct ServiceClub(AtomicBool);
    impl Club for ServiceClub {
        fn label(&self) -> &str {
            "service-lifecycle"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok("The service is running.".into())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.0.swap(true, Ordering::SeqCst) {
                Ok(ClubReply::Text("The service is running.".into()))
            } else {
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "start-service".into(),
                    name: "proc_run".into(),
                    args: serde_json::json!({"command":"sleep 300", "name":"service"}),
                }]))
            }
        }
    }
    let mut registry = ToolRegistry::new();
    registry.set_workspace(fixture.root.0.clone());
    registry.register(Box::new(fixture.runner()));
    let (events, _received) = std::sync::mpsc::channel();
    let answer = crate::harness::run_turn(
        &ServiceClub(AtomicBool::new(false)),
        &registry,
        &mut vec![ChatMsg::user("Start a background service in this cockpit.")],
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();
    assert!(answer.contains("service is running"));
    let id = {
        let mut procs = table().lock().unwrap();
        let (id, entry) = procs
            .iter_mut()
            .find(|(_, entry)| entry.project_root == fixture.root.0)
            .expect("the turn started its service");
        assert_eq!(
            entry.state(),
            "running",
            "an answer must not kill the service"
        );
        assert_eq!(entry.owner, 0, "release the expired turn identity");
        *id
    };
    ProcStopTool::new(fixture.root.0.clone())
        .call(&serde_json::json!({"id":id}))
        .unwrap();
    assert_ne!(
        table().lock().unwrap().get_mut(&id).unwrap().state(),
        "running"
    );
}

#[test]
fn lifecycle_proc_turn_cancel_receipt() {
    assert_owned_kill_receipt("cancelled");
}

#[test]
fn lifecycle_proc_deadline_receipt() {
    assert_owned_kill_receipt("deadline");
}

#[test]
fn lifecycle_proc_idle_receipt() {
    assert_owned_kill_receipt("tool_idle");
}

#[test]
fn lifecycle_proc_owner_reap_receipt() {
    assert_owned_kill_receipt("owner_reap");
}

#[test]
fn lifecycle_turn_end_stops_only_owned_proc_jobs() {
    let fixture = TestProcStore::new();
    let runner = fixture.runner();
    let cancel_a = std::sync::atomic::AtomicBool::new(false);
    let cancel_b = std::sync::atomic::AtomicBool::new(false);
    let a = runner
        .call_with_cancel(
            &serde_json::json!({"command":"sleep 300", "name":"owned-a"}),
            Some(&cancel_a),
        )
        .unwrap();
    let b = runner
        .call_with_cancel(
            &serde_json::json!({"command":"sleep 300", "name":"owned-b"}),
            Some(&cancel_b),
        )
        .unwrap();
    let (a_id, a_pid) = parse_handle(&a);
    let (b_id, _) = parse_handle(&b);
    stop_owned(&cancel_a as *const _ as usize);
    assert!(
        !Path::new(&format!("/proc/{a_pid}")).exists(),
        "owned worker not reaped"
    );
    let mut procs = table().lock().unwrap();
    assert!(procs.get_mut(&a_id).unwrap().state().contains("stopped"));
    assert_eq!(procs.get_mut(&b_id).unwrap().state(), "running");
    drop(procs);
    let terminal: Value = serde_json::from_slice(
        &std::fs::read(
            proc_dir()
                .join("completions")
                .join(format!("{a_id}-{a_pid}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(terminal["status"], "Cancelled");
    assert_eq!(terminal["verification"], "Inconclusive");
    assert!(
        ProcStatusTool::new(runner.workspace.clone())
            .call(&serde_json::json!({"id":a_id}))
            .is_err()
    );
    stop_owned(&cancel_b as *const _ as usize);
    eprintln!("LIFECYCLE_PROC_RECEIPT owned_pid_reaped=true unrelated_turn_preserved=true");
}

#[test]
fn run_status_stop_roundtrip() {
    let store = TestProcStore::new();
    let run = store.runner();
    let workspace = run.workspace.clone();
    let out = run
        .call(&serde_json::json!({
            "command": "echo ready; sleep 30",
            "name": "test-daemon"
        }))
        .expect("spawn");
    let id: u64 = out
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .and_then(|s| s.parse().ok())
        .expect("id in output");
    // Give the shell a beat to write "ready" to the log.
    std::thread::sleep(Duration::from_millis(300));
    let foreign = std::env::temp_dir().join(format!("angel-proc-foreign-{}", std::process::id()));
    assert!(
        ProcStatusTool::new(foreign.clone())
            .call(&serde_json::json!({ "id": id }))
            .is_err()
    );
    assert!(
        ProcStopTool::new(foreign)
            .call(&serde_json::json!({ "id": id }))
            .is_err()
    );
    let status = ProcStatusTool::new(workspace.clone())
        .call(&serde_json::json!({ "id": id }))
        .expect("status");
    assert!(status.contains("running"), "expected running: {status}");
    assert!(status.contains("ready"), "expected log tail: {status}");
    let stopped = ProcStopTool::new(workspace)
        .call(&serde_json::json!({ "id": id }))
        .expect("stop");
    assert!(
        stopped.contains("stopped") || stopped.contains("killed"),
        "unexpected stop result: {stopped}"
    );
}

#[test]
fn proc_status_schema_does_not_advertise_host_wait() {
    let tool = ProcStatusTool::new(PathBuf::from("."));
    let def = tool.def();
    assert!(
        def.description.contains("wait_ms is ignored"),
        "{}",
        def.description
    );
    assert!(
        !def.description.contains("60000"),
        "must not teach a 60s park: {}",
        def.description
    );
    let wait = def.params["properties"]["wait_ms"]["description"]
        .as_str()
        .expect("wait_ms description");
    assert!(wait.contains("ignored"), "{wait}");
}

#[test]
fn status_does_not_park_the_turn_on_wait_ms() {
    let store = TestProcStore::new();
    let run = store.runner();
    let workspace = run.workspace.clone();
    let out = run
        .call(&serde_json::json!({
            "command": "sleep 30; echo complete",
            "name": "wait-ignored-test"
        }))
        .expect("spawn");
    assert!(
        !out.contains("wait_ms"),
        "spawn must not teach wait_ms: {out}"
    );
    let id: u64 = out
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .and_then(|s| s.parse().ok())
        .expect("id in output");
    struct StopOnDrop {
        workspace: PathBuf,
        id: u64,
    }
    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            let _ = ProcStopTool::new(self.workspace.clone())
                .call(&serde_json::json!({ "id": self.id }));
        }
    }
    let _stop = StopOnDrop {
        workspace: workspace.clone(),
        id,
    };
    let started = Instant::now();
    let status = ProcStatusTool::new(workspace)
        .call(&serde_json::json!({ "id": id, "wait_ms": 60_000 }))
        .expect("snapshot status");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(750),
        "wait_ms must not park the turn ({elapsed:?}): {status}"
    );
    assert!(status.contains("running"), "{status}");
    assert!(
        status.contains("wait_ms was ignored"),
        "running snapshot must say wait_ms was ignored: {status}"
    );
}

#[test]
fn finite_job_is_visible_on_a_later_snapshot() {
    let store = TestProcStore::new();
    let run = store.runner();
    let workspace = run.workspace.clone();
    let out = run
        .call(&serde_json::json!({
            "command": "sleep 0.1; echo complete",
            "name": "finite-test"
        }))
        .expect("spawn");
    let id: u64 = out
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .and_then(|s| s.parse().ok())
        .expect("id in output");
    let status = ProcStatusTool::new(workspace);
    let started = Instant::now();
    let snapshot = loop {
        let text = status
            .call(&serde_json::json!({ "id": id }))
            .expect("status");
        if text.contains("exited 0") {
            break text;
        }
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "finite job never exited: {text}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(snapshot.contains("complete"), "{snapshot}");
}

fn parse_handle(out: &str) -> (u64, u32) {
    let id = out
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .and_then(|s| s.parse().ok())
        .expect("id in output");
    let pid = out
        .split("(pid ")
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.parse().ok())
        .expect("pid in output");
    (id, pid)
}

#[test]
fn proc_stat_fields_reads_state_and_starttime() {
    let (state, starttime) = proc_stat_fields(std::process::id()).expect("own /proc stat");
    assert!(matches!(state, 'R' | 'S' | 'D'), "state {state}");
    assert!(starttime > 0);
}

#[test]
fn receipts_adopt_then_vanish_across_simulated_restart() {
    let store = TestProcStore::new();
    let run = store.runner();
    let workspace = run.workspace.clone();
    let out = run
        .call(&serde_json::json!({
            "command": "echo receipt-ready; sleep 300",
            "name": "receipt-daemon"
        }))
        .expect("spawn");
    let (id, pid) = parse_handle(&out);
    // Receipt written at spawn, starttime matching the live /proc value.
    let receipt_path = receipts_dir().join(format!("{id}-{pid}.json"));
    assert!(receipt_path.exists(), "receipt written at spawn: {out}");
    let receipt: Value =
        serde_json::from_str(&std::fs::read_to_string(&receipt_path).unwrap()).unwrap();
    let (_, live_starttime) = proc_stat_fields(pid).expect("live /proc stat");
    assert_eq!(
        receipt["starttime_ticks"].as_u64().unwrap(),
        live_starttime,
        "receipt starttime must match /proc"
    );
    assert_eq!(receipt["pgid"].as_u64().unwrap(), pid as u64);
    // Simulate a cockpit restart for this daemon: clear its entry from the
    // static table. (The table is process-wide and the other proc tests
    // run in parallel against it, so clear only this entry — the effect
    // for the entry under test is identical to a full restart wipe.)
    table().lock().unwrap().remove(&id);
    // List mode adopts the starttime-verified survivor.
    let status = ProcStatusTool::new(workspace.clone())
        .call(&serde_json::json!({}))
        .expect("status");
    assert!(
        status.contains("adopted from previous session · running"),
        "{status}"
    );
    assert!(status.contains(&format!("(pid {pid})")), "{status}");
    // Single-id detail resolves through the receipt too, log tail included.
    let detail = ProcStatusTool::new(workspace.clone())
        .call(&serde_json::json!({ "id": id }))
        .expect("detail");
    assert!(
        detail.contains("adopted from previous session · running"),
        "{detail}"
    );
    assert!(detail.contains("receipt-ready"), "{detail}");
    // Kill it out from under the receipt: the next status observing the
    // death reports a failed, unknown exit and retains the evidence.
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
        libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let vanished_marker = format!("(pid {pid}, daemon from a previous session)");
    let vanished = loop {
        let status = ProcStatusTool::new(workspace.clone())
            .call(&serde_json::json!({}))
            .expect_err("a vanished worker must not be a successful snapshot");
        if status.contains("vanished") && status.contains(&vanished_marker) {
            break status;
        }
        assert!(Instant::now() < deadline, "never saw vanished: {status}");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(vanished.contains("exit unknown"), "{vanished}");
    assert!(vanished.contains("last log lines"), "{vanished}");
    assert!(
        receipt_path.exists(),
        "receipt retained when the vanish is observed"
    );
}

#[test]
fn proc_stop_stops_an_adopted_daemon_by_pgid() {
    let store = TestProcStore::new();
    let run = store.runner();
    let workspace = run.workspace.clone();
    let out = run
        .call(&serde_json::json!({
            "command": "sleep 300",
            "name": "receipt-stoppee"
        }))
        .expect("spawn");
    let (id, pid) = parse_handle(&out);
    let receipt_path = receipts_dir().join(format!("{id}-{pid}.json"));
    assert!(receipt_path.exists(), "receipt written at spawn: {out}");
    // Orphan it (simulated restart), then stop it through the receipt.
    table().lock().unwrap().remove(&id);
    let stopped = ProcStopTool::new(workspace)
        .call(&serde_json::json!({ "id": id }))
        .expect("adopted stop");
    assert!(
        stopped.contains("adopted from previous session"),
        "{stopped}"
    );
    assert!(
        stopped.contains("stopped") || stopped.contains("killed"),
        "{stopped}"
    );
    assert!(!receipt_path.exists(), "receipt cleared after adopted stop");
    // The process itself must be gone (dead, or an unreaped zombie of this
    // test process, which /proc reports as state Z).
    let gone = proc_stat_fields(pid).is_none_or(|(state, _)| matches!(state, 'Z' | 'X' | 'x'));
    assert!(gone, "pid {pid} survived adopted proc_stop");
    // The simulated restart dropped Child, but this test remains its real
    // parent and must reap the stopped shell before leaving the fixture.
    unsafe { libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0) };
}
