//! Process timeout, sandbox observation, sealed child env, and PATH shaping coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory. Shared helpers
//! (`EnvGuard`, `scratch`) remain in the parent module.

use super::*;

// --- output_timed / sandbox exec / sealed-env suite -------------------------

#[test]
fn sandboxed_relative_policy_keeps_workspace_identity_after_chdir() {
    let _guard = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let relative_parent = PathBuf::from(format!(
        ".angel-relative-policy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let relative_workspace = relative_parent.join("workspace");
    let parent = std::env::current_dir().unwrap().join(&relative_parent);
    let workspace = parent.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(parent.join("outside.txt"), "unchanged").unwrap();
    let mut policy = SandboxPolicy {
        writable_roots: Vec::new(),
        allow_network: false,
        enforce: true,
        mandatory: true,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    };
    policy.writable_roots.push(relative_workspace.clone());

    let observation = run_sandboxed_observed(
        "sh",
        &[
            "-c",
            "printf connected > inside.txt && \
             if (printf escaped > ../outside.txt) 2>/dev/null; then exit 99; fi",
        ],
        Some(&relative_workspace),
        &policy,
    )
    .expect("relative workspace command must launch");
    let inside = std::fs::read_to_string(workspace.join("inside.txt"));
    let outside = std::fs::read_to_string(parent.join("outside.txt")).unwrap();
    std::fs::remove_dir_all(parent).unwrap();

    assert_eq!(observation.exit, Some(0), "{}", observation.output);
    assert_eq!(inside.unwrap(), "connected");
    assert_eq!(
        outside, "unchanged",
        "normalizing a grant must not widen it"
    );
}

#[test]
fn output_timed_kills_hung_command() {
    // `output_timed` drops the timeout entirely when YOLO is live, and
    // `yolo::enabled()` reads the process-global `ANGEL_YOLO`. Several tests set
    // it to `1` process-wide, so without serializing here this test
    // intermittently runs its `sleep 5` with no timeout at all and reports the
    // hang as a clean completion. Same hazard `harness::tests::hooks` guards.
    let _guard = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let mut cmd = Command::new("sleep");
    cmd.arg("5");
    let start = Instant::now();
    let (_out, timed_out) = output_timed(cmd, Some(Duration::from_millis(300))).unwrap();
    assert!(timed_out, "a 5s sleep under a 300ms timeout must be killed");
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "should be killed promptly, not run to completion"
    );
}

#[test]
fn fixed_control_probe_deadline_survives_yolo() {
    let _guard = crate::tests::env_lock();
    let _yolo_on = EnvGuard::set("ANGEL_YOLO", "1");
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "sleep 30 & wait"]);
    let started = Instant::now();

    let captured = output_timed_fixed_captured(cmd, Duration::from_millis(50)).unwrap();

    assert!(captured.timed_out);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "fixed control probe inherited YOLO's unlimited deadline: {:?}",
        started.elapsed()
    );
}

#[test]
fn operator_cancel_kills_and_reaps_the_process_group_promptly() {
    let _guard = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let mut policy = SandboxPolicy::permissive();
    policy.enforce = false;
    run_sandboxed_observed("true", &[], None, &policy).expect("warm process runner");
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = Arc::clone(&cancel);
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        trigger.store(true, Ordering::Release);
    });
    let started = Instant::now();
    let observation = run_sandboxed_observed_cancellable(
        "sh",
        &["-c", "sleep 30 & wait"],
        None,
        &policy,
        Some(cancel.as_ref()),
    )
    .expect("cancelled command must still produce an observation");
    setter.join().unwrap();
    eprintln!(
        "operator process-group cancellation latency_ms={}",
        started.elapsed().as_millis()
    );

    assert!(observation.cancelled);
    assert!(!observation.timed_out);
    assert!(
        observation
            .output
            .contains("cancelled — process group killed"),
        "{}",
        observation.output
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "operator cancellation took {:?}",
        started.elapsed()
    );
}

#[test]
fn pre_cancelled_execution_never_spawns() {
    let cancel = AtomicBool::new(true);
    let mut cmd = Command::new("sleep");
    cmd.arg("30");
    let error =
        output_timed_captured_cancellable(cmd, None, Some(&cancel)).expect_err("must not spawn");
    assert!(error.contains("cancelled before spawn"), "{error}");
}

#[test]
fn output_timed_returns_fast_command() {
    let mut cmd = Command::new("echo");
    cmd.arg("hi-there");
    let (out, timed_out) = output_timed(cmd, Some(Duration::from_secs(5))).unwrap();
    assert!(!timed_out);
    assert!(String::from_utf8_lossy(&out.stdout).contains("hi-there"));
}

#[test]
fn sandboxed_progress_arrives_before_the_process_exits() {
    let policy = SandboxPolicy::permissive();
    run_sandboxed_observed("true", &[], None, &policy).expect("warm process runner");
    let chunks = Arc::new(Mutex::new(Vec::<(ProcessStream, Vec<u8>)>::new()));
    let progress_chunks = Arc::clone(&chunks);
    let saw_progress = Arc::new(AtomicBool::new(false));
    let callback_saw_progress = Arc::clone(&saw_progress);
    let progress: Arc<ToolOutputProgress> = Arc::new(move |stream, bytes| {
        progress_chunks
            .lock()
            .unwrap()
            .push((stream, bytes.to_vec()));
        callback_saw_progress.store(true, Ordering::Release);
    });
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = run_sandboxed_observed_cancellable_with_progress(
            "sh",
            &[
                "-c",
                "printf 'live stdout\\n'; sleep 0.5; printf 'final stderr\\n' >&2",
            ],
            None,
            &policy,
            None,
            Some(progress),
        );
        let _ = done_tx.send(result);
    });

    let deadline = Instant::now() + Duration::from_secs(2);
    while !saw_progress.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        saw_progress.load(Ordering::Acquire),
        "stdout should reach the progress sink promptly"
    );
    assert!(
        done_rx.try_recv().is_err(),
        "the live stdout chunk should arrive while the process is sleeping"
    );
    let observation = done_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("process completion")
        .expect("sandbox observation");
    assert_eq!(observation.exit, Some(0));

    let chunks = chunks.lock().unwrap();
    assert!(chunks.iter().any(|(stream, bytes)| {
        *stream == ProcessStream::Stdout && bytes.windows(11).any(|w| w == b"live stdout")
    }));
    assert!(chunks.iter().any(|(stream, bytes)| {
        *stream == ProcessStream::Stderr && bytes.windows(12).any(|w| w == b"final stderr")
    }));
}

#[test]
fn output_timed_keeps_terminal_summary_after_noisy_output() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(
        "printf 'early diagnostic\\n'; yes x | head -c 1100000; printf '\\ntest result: ok. 7 passed; 0 failed; 0 ignored; done\\n'",
    );
    let (out, timed_out) = output_timed(cmd, Some(Duration::from_secs(10))).unwrap();
    assert!(!timed_out);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("early diagnostic"), "head diagnostic lost");
    assert!(text.contains("output bytes omitted"), "cap marker missing");
    assert!(
        text.contains("test result: ok. 7 passed"),
        "terminal verifier summary lost:\n{text}"
    );
    assert_eq!(parse_test_result(&text).passed, 7);
}

#[test]
fn sandboxed_observation_keeps_stderr_tail_after_noisy_stdout() {
    let policy = SandboxPolicy::permissive();
    let observation = run_sandboxed_observed(
        "sh",
        &[
            "-c",
            "printf 'early diagnostic\\n'; yes x | head -c 12000; printf '\\nFINAL_COMPILER_ERROR: missing symbol\\n' >&2",
        ],
        None,
        &policy,
    )
    .unwrap();
    assert_eq!(observation.exit, Some(0));
    assert!(
        observation.output.contains("early diagnostic"),
        "head diagnostic lost: {}",
        observation.output
    );
    assert!(
        observation.output.contains("middle byte(s) elided"),
        "bounded elision marker missing: {}",
        observation.output
    );
    assert!(
        observation
            .output
            .contains("FINAL_COMPILER_ERROR: missing symbol"),
        "stderr tail lost: {}",
        observation.output
    );
    assert!(
        observation.output.len() < 4_200,
        "bounded view grew unexpectedly"
    );
}

#[test]
fn output_timed_does_not_hang_on_pipe_holding_grandchild() {
    // Regression for the 3-hour hang: `sh` backgrounds a `sleep` that inherits
    // the stdout pipe and keeps it open, then exits. Pre-fix, read_to_end
    // blocked at EOF until the sleep finished (or forever). The process-group
    // kill must close the inherited pipe so we return promptly with the output.
    //
    // Serialized and pinned YOLO-off for the same reason as
    // `output_timed_kills_hung_command`: a concurrent test setting `ANGEL_YOLO=1`
    // removes the timeout this regression depends on.
    let _guard = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("sleep 60 & echo done");
    let start = std::time::Instant::now();
    let (out, timed_out) = output_timed(cmd, Some(Duration::from_secs(10))).unwrap();
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "must not block on the lingering pipe-holding grandchild (took {:?})",
        start.elapsed()
    );
    assert!(
        !timed_out,
        "the direct child exited cleanly — not a timeout"
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("done"));
}

#[test]
fn yolo_preserves_background_descendants_after_the_shell_exits() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("yolo_background_descendant");
    let marker = root.join("alive");
    std::fs::create_dir_all(&root).unwrap();

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(format!(
        ": > '{}'; (sleep 0.1; printf alive > '{}') </dev/null >/dev/null 2>&1 & printf launched",
        marker.display(),
        marker.display(),
    ));
    let (out, timed_out) = output_timed(cmd, Some(Duration::from_millis(10))).unwrap();
    assert!(!timed_out, "YOLO removes even a caller-supplied deadline");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "launched");

    // The parent deliberately creates an empty marker before it exits. Wait for
    // the descendant's payload rather than pathname existence: create and write
    // are separate observable states even on a local filesystem.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed = None;
    loop {
        match std::fs::read_to_string(&marker) {
            Ok(body) if body == "alive" => {
                observed = Some(body);
                break;
            }
            Ok(body) => observed = Some(body),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("read descendant marker {}: {error}", marker.display()),
        }
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        observed.as_deref(),
        Some("alive"),
        "preserved descendant did not publish its marker payload"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sandbox_path_prefers_user_bins_before_snap_bin() {
    let root = std::env::temp_dir().join(format!("angel_path_{}", std::process::id()));
    let local = root.join(".local/bin");
    let user_bin = root.join("bin");
    let snap = root.join("snap/bin");
    std::fs::create_dir_all(&local).unwrap();
    std::fs::create_dir_all(&user_bin).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    let current = std::env::join_paths([snap.as_path(), local.as_path()]).unwrap();
    let out = prefer_user_bins(Some(root.clone()), current);
    let parts: Vec<PathBuf> = std::env::split_paths(&out).collect();

    assert_eq!(parts.first(), Some(&local));
    assert_eq!(parts.get(1), Some(&user_bin));
    assert_eq!(parts.iter().filter(|p| *p == &local).count(), 1);
    assert!(parts.contains(&snap));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sealed_tool_children_do_not_inherit_provider_credentials() {
    let _guard = crate::tests::env_lock();
    let _strip = EnvGuard::set("ANGEL_TOOL_STRIP_SECRETS", "1");
    let _credential = EnvGuard::set("ANGEL_FAKE_API_KEY", "must-not-cross");
    let mut policy = SandboxPolicy::permissive();
    policy.enforce = false;

    let observation = run_sandboxed_observed(
        "sh",
        &[
            "-c",
            "test -z \"${ANGEL_FAKE_API_KEY+x}\" && printf credential-free",
        ],
        None,
        &policy,
    )
    .expect("sealed child ran");

    assert_eq!(observation.exit, Some(0));
    assert_eq!(observation.output, "credential-free");
}

#[test]
fn yolo_bypasses_sealed_child_environment_scrubbing() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _strip = EnvGuard::set("ANGEL_TOOL_STRIP_SECRETS", "1");
    let _credential = EnvGuard::set("ANGEL_FAKE_API_KEY", "crosses-in-yolo");
    let mut policy = SandboxPolicy::permissive();
    policy.enforce = false;

    let observation = run_sandboxed_observed(
        "sh",
        &["-c", "printf %s \"$ANGEL_FAKE_API_KEY\""],
        None,
        &policy,
    )
    .expect("YOLO child ran");

    assert_eq!(observation.exit, Some(0));
    assert_eq!(observation.output, "crosses-in-yolo");
}

#[test]
fn sandbox_path_keeps_resolved_mise_tools_before_install_wrappers() {
    let root = std::env::temp_dir().join(format!("angelX_mise_path_{}", std::process::id()));
    let local = root.join(".local/bin");
    let installed = root.join(".local/share/mise/installs/gh/fixture/bin");
    std::fs::create_dir_all(&local).unwrap();
    std::fs::create_dir_all(&installed).unwrap();
    let current = std::env::join_paths([local.as_path(), installed.as_path()]).unwrap();
    let out = prefer_user_bins(Some(root.clone()), current);
    let parts: Vec<PathBuf> = std::env::split_paths(&out).collect();
    assert_eq!(parts.first(), Some(&installed));
    assert_eq!(parts.iter().filter(|p| *p == &installed).count(), 1);
    assert!(parts.contains(&local));
    let _ = std::fs::remove_dir_all(root);
}
