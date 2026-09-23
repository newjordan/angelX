use super::*;

#[test]
fn t06c_output_resets_idle_and_unlimited_wait_still_escalates() {
    let _env = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_SECS", "1");
    let _floor = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "0");
    take_tool_idle_escalation();
    let mut cmd = Command::new("sh");
    cmd.args([
        "-c",
        "for n in 1 2 3 4 5; do echo progress; sleep 0.3; done",
    ]);
    let capture = output_timed_captured(cmd, None).unwrap();
    assert!(capture.output.status.success());
    assert!(!capture.tool_idle);
    assert!(!take_tool_idle_escalation());
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "cat < /dev/null; sleep 8"]);
    let started = Instant::now();
    let capture = output_timed_captured(cmd, None).unwrap();
    assert!(capture.tool_idle && capture.timed_out);
    assert!(take_tool_idle_escalation());
    assert!(started.elapsed() < Duration::from_secs(3));
    eprintln!(
        "T06C_PROGRESS_RECEIPT output_resets_idle=true unlimited_wait_escalates=true elapsed_ms={}",
        started.elapsed().as_millis()
    );
}

#[test]
fn timed_out_shell_tool_note_names_state_age_and_descendants() {
    let _env = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "2");
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    let policy = SandboxPolicy::permissive();
    // Emits one line, then hangs with a forked child (`sleep` stays a
    // separate process because of the trailing command).
    let observation = run_sandboxed_observed(
        "sh",
        &["-c", "echo started; sleep 30; echo done"],
        None,
        &policy,
    )
    .expect("sandboxed run");
    assert!(observation.timed_out);
    let output = &observation.output;
    assert!(output.contains("timed out after 2s"), "{output}");
    assert!(output.contains("no output for"), "{output}");
    assert!(output.contains("child state"), "{output}");
    assert!(output.contains("live descendant"), "{output}");
    assert!(output.contains("ANGEL_TOOL_TIMEOUT"), "{output}");
}

#[test]
fn extension_decision_pins_the_kill_decision_contract() {
    // The decision is pure and timing-free: the racing behavioral lapse
    // test could lose a scheduler race under a loaded parallel suite, so
    // the contract lives here instead.
    let ten = Duration::from_secs(10);
    let six = Duration::from_secs(6);
    // A strictly-larger live budget extends.
    assert_eq!(
        extension_decision(Some(six), Some(Some(ten))),
        Some(Some(ten))
    );
    // Unchanged or shorter policies keep the historical kill.
    assert_eq!(extension_decision(Some(six), Some(Some(six))), None);
    assert_eq!(
        extension_decision(Some(six), Some(Some(Duration::from_secs(2)))),
        None
    );
    // 0 = unlimited adopts unlimited.
    assert_eq!(extension_decision(Some(six), Some(None)), Some(None));
    // Fixed-deadline callers never consult the policy at all.
    assert_eq!(extension_decision(Some(six), None), None);
    // An already-unlimited run has no decision to make.
    assert_eq!(extension_decision(None, Some(Some(ten))), None);
}

#[test]
fn idle_floor_kills_sleep_before_the_full_tool_timeout() {
    let _env = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "120");
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "1");
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    assert_eq!(tool_idle_floor(), Some(Duration::from_secs(1)));
    assert_eq!(tool_timeout(), Some(Duration::from_secs(120)));
    let policy = SandboxPolicy::permissive();
    // Tests spawn `--bin angel --sandbox-exec`. That first call runs
    // `cargo build` inside OnceLock; do not count it as idle-wait.
    let warmup =
        run_sandboxed_observed("sh", &["-c", "echo warmup"], None, &policy).expect("warmup");
    assert!(
        warmup.output.contains("warmup"),
        "sandbox helper warmup failed: {}",
        warmup.output
    );
    let never = std::sync::atomic::AtomicBool::new(false);
    // The live shell tool always passes a cancel flag. Both wait loops
    // have to idle-kill; the 120s sleep hang was the cancellable path.
    for cancel in [None, Some(&never)] {
        let started = Instant::now();
        let observation = run_sandboxed_observed_cancellable(
            "sh",
            &["-c", "sleep 30; echo never"],
            None,
            &policy,
            cancel,
        )
        .expect("sandboxed run");
        let elapsed = started.elapsed();
        eprintln!(
            "idle-floor probe cancel={} timed_out={} elapsed={elapsed:?} out={:?}",
            cancel.is_some(),
            observation.timed_out,
            observation.output
        );
        assert!(observation.timed_out, "{}", observation.output);
        assert!(
            elapsed < Duration::from_secs(8),
            "idle sleep must not sit out the 120s tool timeout ({elapsed:?}) out={}",
            observation.output
        );
        assert!(
            !observation.output.contains("never"),
            "sleep must be killed before it finishes: {}",
            observation.output
        );
        assert!(
            !observation.output.contains("timed out after 120s"),
            "idle kill must report elapsed time, not the unused budget: {}",
            observation.output
        );
        assert!(
            observation.output.contains("ANGEL_TOOL_IDLE_FLOOR_SECS"),
            "idle kill must name the idle knob, not ANGEL_TOOL_TIMEOUT: {}",
            observation.output
        );
        assert!(
            observation
                .output
                .contains("silent sleeping wait reaped by the idle floor"),
            "idle kill must name the legitimate-wait case: {}",
            observation.output
        );
    }
}

#[test]
fn turn_idle_does_not_create_a_tool_idle_kill_policy() {
    let _env = crate::tests::env_lock();
    let _turn = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1");
    let _tool = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_SECS");
    assert_eq!(tool_idle_timeout(), None);
}

#[test]
fn busy_process_survives_idle_floor_and_follows_hard_timeout() {
    let _env = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "0");
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "1");
    let _hard = crate::tests::TestEnvGuard::set("ANGEL_TOOL_HARD_TIMEOUT", "3");
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    assert_eq!(tool_idle_floor(), Some(Duration::from_secs(1)));
    assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(3)));
    assert_eq!(tool_timeout(), None);
    let policy = SandboxPolicy::permissive();
    let warmup =
        run_sandboxed_observed("sh", &["-c", "echo warmup"], None, &policy).expect("warmup");
    assert!(
        warmup.output.contains("warmup"),
        "sandbox helper warmup failed: {}",
        warmup.output
    );
    for i in 0..50 {
        let again = run_sandboxed_observed("sh", &["-c", "echo warmup"], None, &policy)
            .unwrap_or_else(|e| panic!("warmup loop {i}: {e}"));
        assert!(
            again.output.contains("warmup"),
            "warmup loop {i} dropped helper output: {}",
            again.output
        );
    }
    let started = Instant::now();
    // `exec yes` keeps the sandbox child itself in R; a sleeping `sh`
    // waiting on a grandchild would look idle and die at the floor.
    let observation = run_sandboxed_observed("sh", &["-c", "exec yes >/dev/null"], None, &policy)
        .expect("sandboxed run");
    let elapsed = started.elapsed();
    assert!(observation.timed_out, "{}", observation.output);
    assert!(
        elapsed >= Duration::from_millis(2500),
        "busy child must survive the 1s idle floor ({elapsed:?}) out={}",
        observation.output
    );
    assert!(
        elapsed < Duration::from_secs(8),
        "busy child must follow the 3s hard timeout ({elapsed:?}) out={}",
        observation.output
    );
    assert!(
        !observation.output.contains("timed out after 120s"),
        "hard-timeout kill must not report the unused default budget: {}",
        observation.output
    );
}

#[test]
fn unchanged_live_policy_keeps_the_historical_kill() {
    let _env = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "2");
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    let policy = SandboxPolicy::permissive();
    let observation = run_sandboxed_observed("sh", &["-c", "sleep 30; echo never"], None, &policy)
        .expect("sandboxed run");
    assert!(observation.timed_out);
    assert!(
        !observation.output.contains("deadline extended"),
        "an unchanged policy must not claim extension: {}",
        observation.output
    );
    assert!(observation.output.contains("timed out after 2s"));
}

#[test]
fn timeout_note_reports_diagnostics_and_keeps_the_bare_form() {
    let diag = TimeoutDiagnostics {
        child_state: Some('S'),
        live_descendants: 2,
        last_output_age_secs: Some(118),
        grace_ms: 0,
        exited_in_grace: false,
    };
    let note = timeout_note(120, Some(&diag));
    assert!(
        note.contains("no output for 118s, child state S, 2 live descendants"),
        "{note}"
    );
    assert!(note.contains("timed out after 120s"), "{note}");
    assert!(note.contains("ANGEL_TOOL_TIMEOUT"), "{note}");
    // Without diagnostics the historical note is byte-identical.
    assert_eq!(
        timeout_note(120, None),
        "\n[timed out after 120s — process killed; raise/disable via ANGEL_TOOL_TIMEOUT]"
    );
    // A silent single process reads truthfully, grace outcomes included.
    let diag = TimeoutDiagnostics {
        child_state: None,
        live_descendants: 0,
        last_output_age_secs: None,
        grace_ms: 500,
        exited_in_grace: true,
    };
    let summary = diag.summary();
    assert!(summary.contains("no output ever"), "{summary}");
    assert!(summary.contains("child already gone"), "{summary}");
    assert!(summary.contains("0 live descendants"), "{summary}");
    assert!(
        summary.contains("exited on SIGTERM within 500ms grace"),
        "{summary}"
    );
}

#[test]
fn idle_floor_note_names_the_knob_and_the_legitimate_wait() {
    let diag = TimeoutDiagnostics {
        child_state: Some('S'),
        live_descendants: 3,
        last_output_age_secs: None,
        grace_ms: 0,
        exited_in_grace: false,
    };
    let note = idle_floor_note(30, Some(&diag));
    assert!(note.contains("timed out after 30s"), "{note}");
    assert!(
        note.contains("no output ever, child state S, 3 live descendants"),
        "{note}"
    );
    assert!(
        note.contains("silent sleeping wait reaped by the idle floor"),
        "{note}"
    );
    assert!(
        note.contains("ANGEL_TOOL_IDLE_FLOOR_SECS"),
        "the idle receipt must name its own knob: {note}"
    );
    assert!(note.contains("proc_run"), "{note}");
    // The idle receipt must not point at the budget knob it never hit.
    assert!(!note.contains("ANGEL_TOOL_TIMEOUT"), "{note}");
    // The bare form carries the same guidance.
    let bare = idle_floor_note(30, None);
    assert!(bare.contains("ANGEL_TOOL_IDLE_FLOOR_SECS"), "{bare}");
    assert!(bare.contains("silent sleeping wait"), "{bare}");
}

#[test]
fn tool_ceilings_and_idle_floor_are_operator_caps_only() {
    let _env = crate::tests::env_lock();
    let _idle = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_FLOOR_SECS");
    let _competition = crate::tests::TestEnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _moa = crate::tests::TestEnvGuard::unset("ANGEL_GPU_COMP_LOCAL_MOA");
    let _timeout = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_TIMEOUT");
    let _hard = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_HARD_TIMEOUT");
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    // Safe defaults prevent silent deadlocks and runaway busy loops from freezing the cockpit TUI.
    assert_eq!(tool_timeout(), None);
    assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(900)));
    assert_eq!(tool_idle_timeout(), None);
    assert_eq!(tool_idle_floor(), Some(Duration::from_secs(120)));
    let _armed = crate::tests::TestEnvGuard::set("ANGEL_COMPETITION_MODE", "1");
    assert_eq!(tool_idle_floor(), Some(Duration::from_secs(120)));
    drop(_armed);
    // Explicit operator caps are honoured verbatim; `0` keeps them off.
    let _explicit = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "45");
    assert_eq!(tool_idle_floor(), Some(Duration::from_secs(45)));
    let _idle_zero = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "0");
    assert_eq!(tool_idle_floor(), None);
    let _tool_idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_SECS", "75");
    assert_eq!(tool_idle_timeout(), Some(Duration::from_secs(75)));
    let _cap = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "600");
    assert_eq!(tool_timeout(), Some(Duration::from_secs(600)));
    let _zero = crate::tests::TestEnvGuard::set("ANGEL_TOOL_HARD_TIMEOUT", "0");
    assert_eq!(tool_hard_timeout(), None);
}

#[test]
fn yolo_does_not_disable_containment_ceilings() {
    let _env = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let _hard = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_HARD_TIMEOUT");
    let _idle = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_FLOOR_SECS");
    assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(900)));
    assert_eq!(tool_idle_floor(), Some(Duration::from_secs(120)));
    let _hard_zero = crate::tests::TestEnvGuard::set("ANGEL_TOOL_HARD_TIMEOUT", "0");
    assert_eq!(tool_hard_timeout(), None);
    let _idle_zero = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "0");
    assert_eq!(tool_idle_floor(), None);
}

/// The 2026-09-21 live hang, end to end: a `/yolo on` session ran
/// `python3 -m http.server` through the shell tool and the turn sat on it
/// until the operator noticed. Yolo strips `ANGEL_TOOL_TIMEOUT`, so the idle
/// floor is the only thing standing between a silent foreground server and a
/// wedged turn. The accessor test above cannot see a bypass reintroduced
/// inside the wait loop; this one can.
#[test]
fn yolo_idle_floor_reaps_a_silent_foreground_server() {
    let _env = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let _timeout = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_TIMEOUT");
    let _tool_idle = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_SECS");
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "1");
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    assert_eq!(tool_timeout(), None);
    let policy = SandboxPolicy::permissive();
    // See `idle_floor_kills_sleep_before_the_full_tool_timeout`: the first
    // helper spawn may build, which is not idle-wait.
    let warmup =
        run_sandboxed_observed("sh", &["-c", "echo warmup"], None, &policy).expect("warmup");
    assert!(
        warmup.output.contains("warmup"),
        "sandbox helper warmup failed: {}",
        warmup.output
    );
    // The live shell tool always passes a cancel flag nobody sets.
    let never = std::sync::atomic::AtomicBool::new(false);
    let started = Instant::now();
    let observation = run_sandboxed_observed_cancellable(
        "sh",
        &["-c", "sleep 30; echo never"],
        None,
        &policy,
        Some(&never),
    )
    .expect("sandboxed run");
    let elapsed = started.elapsed();
    assert!(observation.timed_out, "{}", observation.output);
    assert!(
        elapsed < Duration::from_secs(8),
        "yolo must not let a silent server own the turn ({elapsed:?}) out={}",
        observation.output
    );
    assert!(
        !observation.output.contains("never"),
        "{}",
        observation.output
    );
    assert!(
        observation.output.contains("use proc_run"),
        "the receipt must point the model at proc_run: {}",
        observation.output
    );
}

#[test]
fn timeout_kill_defaults_to_immediate_sigkill_with_diagnostics() {
    let _env = crate::tests::env_lock();
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    let mut cmd = Command::new("sh");
    // The trailing `true` keeps `sh` from exec-replacing itself with
    // `sleep`, so the group holds exactly one live descendant.
    cmd.arg("-c").arg("sleep 30; true");
    let capture = output_timed_captured(cmd, Some(Duration::from_millis(300))).unwrap();
    assert!(capture.timed_out);
    let diag = capture.timeout_diag.expect("timeout diagnostics");
    assert_eq!(diag.grace_ms, 0, "default = exact historical behavior");
    assert!(!diag.exited_in_grace);
    assert_eq!(diag.child_state, Some('S'));
    assert_eq!(diag.live_descendants, 1, "the sh child's sleep is alive");
    assert_eq!(
        diag.last_output_age_secs, None,
        "the tool never wrote a byte"
    );
}

#[test]
fn detached_pipe_holder_cannot_strand_output_reader_threads() {
    let _env = crate::tests::env_lock();
    let pid_path = std::env::temp_dir().join(format!(
        "angel-detached-pipe-holder-{}-{}.pid",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&pid_path);
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(format!(
            "setsid sh -c 'echo $$ > {0}; sleep 30' & while [ ! -s {0} ]; do sleep 0.01; done; echo shell-done",
            pid_path.display()
        ));
    let started = Instant::now();
    let capture = output_timed_captured(cmd, Some(Duration::from_secs(5))).unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "both inherited pipes must share one bounded drain deadline: {:?}",
        started.elapsed()
    );
    assert_eq!(capture.output.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&capture.output.stdout).contains("shell-done"),
        "the interrupted reader must return its bounded prefix"
    );

    let detached_pid = (0..100).find_map(|_| {
        let pid = std::fs::read_to_string(&pid_path)
            .ok()
            .and_then(|text| text.trim().parse::<libc::pid_t>().ok());
        if pid.is_none() {
            std::thread::sleep(Duration::from_millis(10));
        }
        pid
    });
    let held_stdout = capture.grandchild_holds_stdout;
    let held_stderr = capture.grandchild_holds_stderr;
    if let Some(pid) = detached_pid {
        // SAFETY: the fixture made this pid a new process-group leader.
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
    }
    let _ = std::fs::remove_file(pid_path);
    assert!(held_stdout && held_stderr);
    eprintln!(
        "T06C_DRAIN_RECEIPT grandchild_holds_stdout={held_stdout} grandchild_holds_stderr={held_stderr} elapsed_ms={}",
        started.elapsed().as_millis()
    );
}

#[test]
fn sigterm_grace_lets_a_polite_child_exit_before_sigkill() {
    let _env = crate::tests::env_lock();
    // Non-fixed caller deadlines are still removed under YOLO. This test
    // asserts the explicit deadline fires, so pin that contract off.
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _grace = crate::tests::TestEnvGuard::set("ANGEL_TOOL_KILL_GRACE_MS", "2000");
    let mut cmd = Command::new("sleep");
    cmd.arg("30");
    let started = Instant::now();
    let capture = output_timed_captured(cmd, Some(Duration::from_millis(300))).unwrap();
    assert!(capture.timed_out);
    let diag = capture.timeout_diag.expect("timeout diagnostics");
    assert_eq!(diag.grace_ms, 2000);
    assert!(
        diag.exited_in_grace,
        "sleep dies on SIGTERM well inside the grace window"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "grace must end at the child's exit, not run its full length ({:?})",
        started.elapsed()
    );
}

#[test]
fn cancellable_timeout_path_also_captures_diagnostics() {
    let _env = crate::tests::env_lock();
    // Same as the grace test: an explicit non-fixed deadline must not be
    // confused with the containment ceilings, which stay armed under YOLO.
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    let cancel = AtomicBool::new(false);
    let mut cmd = Command::new("sleep");
    cmd.arg("30");
    let capture =
        output_timed_captured_cancellable(cmd, Some(Duration::from_millis(300)), Some(&cancel))
            .unwrap();
    assert!(capture.timed_out);
    assert!(capture.timeout_diag.is_some());
}

/// A call budget caps both tool bounds for the calls made inside it, and only
/// there: the previous bounds come back when it ends.
#[test]
fn call_budget_caps_tool_bounds_and_restores_them() {
    let _env = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_TIMEOUT");
    let _hard = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_HARD_TIMEOUT");
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(900)));
    assert_eq!(tool_timeout(), None);
    with_call_budget(Some(Duration::from_secs(180)), || {
        assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(180)));
        assert_eq!(tool_timeout(), Some(Duration::from_secs(180)));
        with_call_budget(Some(Duration::from_secs(30)), || {
            assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(30)));
        });
        assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(180)));
    });
    assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(900)));
    assert_eq!(tool_timeout(), None);
    with_call_budget(None, || {
        assert_eq!(tool_hard_timeout(), Some(Duration::from_secs(900)));
    });
}

/// A busy process (an infinite loop in code under test) never trips the idle
/// floor and would run to the 900 s hard timeout; inside a call budget it is
/// killed at the budget instead.
#[test]
fn busy_process_is_killed_at_the_call_budget() {
    let _env = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_TIMEOUT");
    let _hard = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_HARD_TIMEOUT");
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "1");
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let policy = SandboxPolicy::permissive();
    let warmup =
        run_sandboxed_observed("sh", &["-c", "echo warmup"], None, &policy).expect("warmup");
    assert!(warmup.output.contains("warmup"), "{}", warmup.output);
    let started = Instant::now();
    let observation = with_call_budget(Some(Duration::from_secs(2)), || {
        run_sandboxed_observed("sh", &["-c", "exec yes >/dev/null"], None, &policy)
            .expect("sandboxed run")
    });
    let elapsed = started.elapsed();
    assert!(observation.timed_out, "{}", observation.output);
    assert!(
        elapsed < Duration::from_secs(8),
        "the busy child must stop at the 2s budget, not the 900s hard timeout ({elapsed:?})"
    );
}
