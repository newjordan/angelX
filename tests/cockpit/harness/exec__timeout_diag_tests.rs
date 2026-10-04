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
    // The knob is named by the timeout's `⠨⠛` page.
    assert!(
        output.contains(&crate::agent::harness::book::d46_recovery::TIMEOUT_KNOB.cells()),
        "{output}"
    );
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
fn turn_idle_does_not_create_a_tool_idle_kill_policy() {
    let _env = crate::tests::env_lock();
    let _turn = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1");
    let _tool = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_SECS");
    assert_eq!(tool_idle_timeout(), None);
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
    // The knob is the `⠨⠛` page on the line after the facts.
    let knob = crate::agent::harness::book::d46_recovery::TIMEOUT_KNOB;
    assert!(note.ends_with(&format!("]\n{}", knob.cells())), "{note}");
    assert!(knob.text().contains("ANGEL_TOOL_TIMEOUT"));
    // Without diagnostics the historical facts are byte-identical.
    assert_eq!(
        timeout_note(120, None),
        format!(
            "\n[timed out after 120s — process killed]\n{}",
            knob.cells()
        )
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
fn tool_timeout_is_an_operator_cap_only() {
    let _env = crate::tests::env_lock();
    let _competition = crate::tests::TestEnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _timeout = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_TIMEOUT");
    let _idle = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_SECS");
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    assert_eq!(tool_timeout(), None);
    assert_eq!(tool_idle_timeout(), None);
    let _armed = crate::tests::TestEnvGuard::set("ANGEL_COMPETITION_MODE", "1");
    assert_eq!(tool_timeout(), None);
    drop(_armed);
    let _tool_idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_SECS", "75");
    assert_eq!(tool_idle_timeout(), Some(Duration::from_secs(75)));
    let _cap = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "600");
    assert_eq!(tool_timeout(), Some(Duration::from_secs(600)));
    let _zero = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "0");
    assert_eq!(tool_timeout(), None);
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

/// An explicit call budget caps `tool_timeout` for the calls made inside it,
/// and only there: the previous bound comes back when it ends.
#[test]
fn call_budget_caps_tool_bounds_and_restores_them() {
    let _env = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_TIMEOUT");
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    assert_eq!(tool_timeout(), None);
    with_call_budget(Some(Duration::from_secs(180)), || {
        assert_eq!(tool_timeout(), Some(Duration::from_secs(180)));
        with_call_budget(Some(Duration::from_secs(30)), || {
            assert_eq!(tool_timeout(), Some(Duration::from_secs(30)));
        });
        assert_eq!(tool_timeout(), Some(Duration::from_secs(180)));
    });
    assert_eq!(tool_timeout(), None);
    with_call_budget(None, || {
        assert_eq!(tool_timeout(), None);
    });
    // Nesting keeps the tighter budget; None leaves the outer one in force.
    with_call_budget(Some(Duration::from_secs(30)), || {
        with_call_budget(None, || {
            assert_eq!(tool_timeout(), Some(Duration::from_secs(30)));
        });
        with_call_budget(Some(Duration::from_secs(180)), || {
            assert_eq!(tool_timeout(), Some(Duration::from_secs(30)));
        });
    });
}

/// A busy process inside an explicit call budget is killed at that budget.
#[test]
fn busy_process_is_killed_at_the_call_budget() {
    let _env = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_TIMEOUT");
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
        "the busy child must stop at the 2s budget ({elapsed:?})"
    );
}

/// YOLO may ignore ordinary timers, but it must not erase a task foreground cap.
/// With idle policies disabled, a sleeping child exercises the wall deadline
/// rather than the busy-process hard ceiling used by the adjacent regression.
#[test]
fn sleeping_process_keeps_call_budget_under_yolo() {
    let _env = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "0");
    let _hard = crate::tests::TestEnvGuard::set("ANGEL_TOOL_HARD_TIMEOUT", "0");
    let _floor = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "0");
    let _idle = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_SECS");
    let _grace = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_KILL_GRACE_MS");
    for cancellable in [false, true] {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let started = Instant::now();
        let capture = with_call_budget(Some(Duration::from_secs(1)), || {
            let mut cmd = Command::new("sh");
            // Finite even if the regression returns: no leaked unbounded sleep.
            cmd.args(["-c", "sleep 5"]);
            output_timed_extensible_cancellable_with_progress(
                cmd,
                tool_timeout(),
                cancellable.then_some(&cancel),
                None,
            )
            .unwrap()
        });
        assert!(capture.timed_out, "YOLO erased the foreground cap");
        assert!(started.elapsed() < Duration::from_secs(4));
    }
}

#[test]
fn crash_signals_keep_their_names_without_crashing_a_process() {
    // Named here rather than raised: a real SIGSEGV in a test reaches
    // systemd-coredump and the desktop reports it as a crash.
    assert_eq!(signal_name(libc::SIGSEGV), "SIGSEGV");
    assert_eq!(signal_name(libc::SIGPIPE), "SIGPIPE");
    assert_eq!(signal_name(libc::SIGTERM), "SIGTERM");
    assert_eq!(signal_name(libc::SIGABRT), format!("signal-{}", libc::SIGABRT));
}
