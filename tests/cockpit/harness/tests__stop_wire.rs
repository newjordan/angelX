//! Golden stop-reason wire values: what `stop_reason`, `interrupted`,
//! `deadline_reached` and `max_hops_reached` read on every path that ends a
//! turn, in memory and in the task JSON an evaluator reads.
//!
//! These pin today's values, including the two places where one stop reason
//! reads differently depending on the path that produced it (see the idle
//! timeout tests). Change a value here only on the operator's word.

use super::*;

/// (stop_reason, interrupted, deadline_reached, max_hops_reached)
type Wire = (String, bool, bool, bool);

fn w(stop_reason: &str, interrupted: bool, deadline: bool, max_hops: bool) -> Wire {
    (stop_reason.to_string(), interrupted, deadline, max_hops)
}

fn wire(result: &Result<TurnOutcome, TurnFailure>) -> Wire {
    match result {
        Ok(o) => w(
            o.stop_reason.as_str(),
            o.interrupted,
            o.deadline_reached,
            o.max_hops_reached,
        ),
        Err(f) => w(
            f.stop_reason.as_str(),
            f.interrupted,
            f.deadline_reached,
            f.max_hops_reached,
        ),
    }
}

/// The task JSON for `result`, reduced to the five values evaluators key on.
fn envelope_wire(
    result: Result<TurnOutcome, TurnFailure>,
    history: &[ChatMsg],
    dir: &Path,
) -> (String, Wire) {
    let ctx = TaskJsonContext {
        task_id: None,
        run_id: None,
        workspace: dir.to_path_buf(),
        club: None,
        model: None,
        reasoning_effort: None,
        output_budget: None,
        elapsed_ms: 1,
        timing: None,
        tools: Vec::new(),
        usage: None,
        runtime: None,
        session_id: None,
        artifacts: Vec::new(),
        memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
    };
    let envelope = match result {
        Ok(outcome) => TaskJsonEnvelope::from_outcome(ctx, outcome, history),
        Err(failure) => TaskJsonEnvelope::from_failure(ctx, failure),
    };
    let json = serde_json::to_value(envelope).unwrap();
    (
        json["status"].as_str().expect("status").to_string(),
        w(
            json["stop_reason"].as_str().expect("stop_reason"),
            json["interrupted"].as_bool().unwrap(),
            json["deadline_reached"].as_bool().unwrap(),
            json["max_hops_reached"].as_bool().unwrap(),
        ),
    )
}

/// Every knob a bare turn reads, pinned off, so the only thing that ends the
/// turn is what the test sets up.
fn quiet() -> Vec<EnvGuard> {
    vec![
        EnvGuard::set("ANGEL_BACKPLANE", "0"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_PROJECT_DOC", "0"),
        EnvGuard::set("ANGEL_TASK_RECON", "0"),
        EnvGuard::set("ANGEL_MEMPALACE", "0"),
        EnvGuard::unset("ANGEL_TRAJECTORY_LOG"),
        EnvGuard::set("ANGEL_ROLLOUT_CAPTURE", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "off"),
        EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0"),
        EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "0"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
        EnvGuard::unset("ANGEL_TURN_DEADLINE_SECS"),
        EnvGuard::unset("ANGEL_COMPETITION_TURN_DEADLINE_SECS"),
        EnvGuard::unset("ANGEL_EVALUATE_MAX_HOPS_WORKSPACE"),
        EnvGuard::unset("ANGEL_PROVIDER_RETRIES"),
        EnvGuard::unset("ANGEL_HARNESS_ROLLOUT_REQUIRED"),
        EnvGuard::unset("ANGEL_FORMATION_TOKEN_BUDGET"),
        EnvGuard::unset("ANGEL_FORMATION_WALL_SECS"),
        EnvGuard::unset("ANGEL_NEEDS_PRO"),
    ]
}

/// Answers `answer` on the first hop.
struct TextClub(&'static str);
impl Club for TextClub {
    fn label(&self) -> &str {
        "stop-wire-text"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        unreachable!()
    }
    fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        Ok(ClubReply::Text(self.0.to_string()))
    }
}

/// Calls the `reverse` tool every hop, after `pause`.
struct CallsClub {
    pause: Duration,
    hops: AtomicUsize,
}
impl Club for CallsClub {
    fn label(&self) -> &str {
        "stop-wire-calls"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        unreachable!()
    }
    fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        std::thread::sleep(self.pause);
        let n = self.hops.fetch_add(1, Ordering::SeqCst);
        Ok(ClubReply::Calls(vec![ToolCall {
            id: format!("c{n}"),
            name: "reverse".into(),
            args: serde_json::json!({ "text": "hello" }),
        }]))
    }
}

/// Fails every provider call with a plain transport error.
struct ErrClub;
impl Club for ErrClub {
    fn label(&self) -> &str {
        "stop-wire-err"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        unreachable!()
    }
    fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        Err("transient transport failure".to_string())
    }
}

fn run(
    club: &dyn Club,
    history: &mut Vec<ChatMsg>,
    cancel: &AtomicBool,
    max_hops: Option<usize>,
) -> Result<TurnOutcome, TurnFailure> {
    let dir = scratch("stop-wire");
    let mut registry = ToolRegistry::with_defaults();
    registry.set_workspace(dir);
    run_turn_observed(
        club,
        &registry,
        history,
        cancel,
        max_hops,
        &mpsc::channel::<TurnEvent>().0,
    )
}

/// Assert the in-memory value and the task JSON agree with `memory` and
/// (`status`, `wire`) respectively. Most paths show the same wire both places.
fn assert_wire(
    result: Result<TurnOutcome, TurnFailure>,
    history: &[ChatMsg],
    memory: (&str, bool, bool, bool),
    status: &str,
) {
    let memory = w(memory.0, memory.1, memory.2, memory.3);
    assert_eq!(wire(&result), memory, "in-memory value");
    let (got_status, got_wire) = envelope_wire(result, history, &scratch("stop-wire-env"));
    assert_eq!(got_status, status, "task JSON status");
    assert_eq!(got_wire, memory, "task JSON values");
}

#[test]
fn answer_reads_false_false_false() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(
        &TextClub("hi"),
        &mut history,
        &AtomicBool::new(false),
        Some(4),
    );
    assert!(result.is_ok());
    assert_wire(
        result,
        &history,
        ("answer", false, false, false),
        "completed",
    );
}

#[test]
fn interrupt_at_the_hop_boundary_reads_true_false_false() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(
        &TextClub("hi"),
        &mut history,
        &AtomicBool::new(true),
        Some(4),
    );
    assert!(result.is_ok());
    assert_wire(
        result,
        &history,
        ("interrupt", true, false, false),
        "stopped",
    );
}

#[test]
fn deadline_reads_true_true_false() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let _deadline = EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "1");
    let club = CallsClub {
        pause: Duration::from_millis(1100),
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(&club, &mut history, &AtomicBool::new(false), None);
    assert!(result.is_ok());
    assert_wire(result, &history, ("deadline", true, true, false), "stopped");
}

#[test]
fn max_hops_as_a_failure_reads_true_false_true() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(
        &TextClub("hi"),
        &mut history,
        &AtomicBool::new(false),
        Some(0),
    );
    assert!(result.is_err());
    assert_wire(result, &history, ("max_hops", true, false, true), "stopped");
}

#[test]
fn max_hops_as_an_outcome_reads_true_false_true() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let _evaluate = EnvGuard::set("ANGEL_EVALUATE_MAX_HOPS_WORKSPACE", "1");
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(
        &TextClub("hi"),
        &mut history,
        &AtomicBool::new(false),
        Some(0),
    );
    assert!(result.is_ok());
    assert_wire(result, &history, ("max_hops", true, false, true), "stopped");
}

#[test]
fn provider_error_reads_false_false_false_on_all_three_paths() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let _no_retry = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");

    // Site 1: the provider call itself fails.
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(&ErrClub, &mut history, &AtomicBool::new(false), Some(4));
    assert!(result.is_err());
    assert_wire(
        result,
        &history,
        ("provider_error", false, false, false),
        "error",
    );

    // Site 2: the formation budget knob does not parse, before any request.
    {
        let _bad = EnvGuard::set("ANGEL_FORMATION_TOKEN_BUDGET", "plenty");
        let mut history = vec![ChatMsg::user("hello")];
        let result = run(
            &TextClub("hi"),
            &mut history,
            &AtomicBool::new(false),
            Some(4),
        );
        let failure = result.as_ref().expect_err("budget knob must fail the turn");
        assert!(
            failure.message.contains("unsigned integer"),
            "{}",
            failure.message
        );
        assert_wire(
            result,
            &history,
            ("provider_error", false, false, false),
            "error",
        );
    }

    // Site 3: the conversation carries a book generation that cannot bind.
    {
        let mut message = ChatMsg::user("hello");
        message.book_memory = Some(Arc::new(
            serde_json::from_value(
                serde_json::json!({"route": null, "used": [], "catalog_digest": "not-a-digest"}),
            )
            .expect("book memory shape"),
        ));
        let mut history = vec![message];
        let result = run(
            &TextClub("hi"),
            &mut history,
            &AtomicBool::new(false),
            Some(4),
        );
        let failure = result
            .as_ref()
            .expect_err("bad generation must fail the turn");
        assert!(
            failure.message.starts_with("book generation:"),
            "{}",
            failure.message
        );
        assert_wire(
            result,
            &history,
            ("provider_error", false, false, false),
            "error",
        );
    }
}

#[test]
fn capture_failure_reads_false_false_false_before_the_first_request() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let _required = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1");

    // Capture is off but required.
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(
        &TextClub("hi"),
        &mut history,
        &AtomicBool::new(false),
        Some(4),
    );
    assert!(result.is_err());
    assert_wire(
        result,
        &history,
        ("capture_failure", false, false, false),
        "error",
    );

    // Capture is on but its store cannot start.
    let output = scratch("stop-wire-capture");
    let blocked_root = output.join("not-a-directory");
    std::fs::write(&blocked_root, b"not a directory").unwrap();
    let _mode = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    let _dir = EnvGuard::set(
        "ANGEL_HARNESS_ROLLOUT_DIR",
        blocked_root.to_str().expect("utf-8 path"),
    );
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(
        &TextClub("hi"),
        &mut history,
        &AtomicBool::new(false),
        Some(4),
    );
    assert!(result.is_err());
    assert_wire(
        result,
        &history,
        ("capture_failure", false, false, false),
        "error",
    );
    let _ = std::fs::remove_dir_all(output);
}

/// Swaps the rollout store's `runs` directory for a symlink, so every later
/// journal append is refused and a required capture fails mid-turn.
struct SabotageClub {
    output: PathBuf,
    reply: Result<&'static str, &'static str>,
}
impl Club for SabotageClub {
    fn label(&self) -> &str {
        "stop-wire-sabotage"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        unreachable!()
    }
    fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        for entry in std::fs::read_dir(&self.output).unwrap().flatten() {
            let runs = entry.path().join("runs");
            if runs.is_dir() && !runs.is_symlink() {
                let moved = entry.path().join("runs-moved");
                std::fs::rename(&runs, &moved).unwrap();
                std::os::unix::fs::symlink(&moved, &runs).unwrap();
            }
        }
        match self.reply {
            Ok(text) => Ok(ClubReply::Text(text.to_string())),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[test]
fn capture_failure_reads_false_false_false_when_capture_breaks_mid_turn() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let _no_retry = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _required = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1");
    let _mode = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    // An answer whose capture broke while the model was thinking.
    let output = scratch("stop-wire-capture-answer");
    let _dir = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_DIR", output.to_str().expect("utf-8"));
    let club = SabotageClub {
        output: output.clone(),
        reply: Ok("hi"),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(&club, &mut history, &AtomicBool::new(false), Some(4));
    let failure = result
        .as_ref()
        .expect_err("a broken capture fails the turn");
    assert!(
        failure
            .message
            .starts_with("required rollout capture failed:"),
        "{}",
        failure.message
    );
    assert_wire(
        result,
        &history,
        ("capture_failure", false, false, false),
        "error",
    );
    let _ = std::fs::remove_dir_all(output);

    // A provider failure whose capture broke: the capture failure replaces it.
    let output = scratch("stop-wire-capture-failure");
    let _dir = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_DIR", output.to_str().expect("utf-8"));
    let club = SabotageClub {
        output: output.clone(),
        reply: Err("transient transport failure"),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(&club, &mut history, &AtomicBool::new(false), Some(4));
    let failure = result
        .as_ref()
        .expect_err("a broken capture fails the turn");
    assert!(
        failure
            .message
            .starts_with("required rollout capture failed:"),
        "{}",
        failure.message
    );
    assert_wire(
        result,
        &history,
        ("capture_failure", false, false, false),
        "error",
    );
    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn checkpoint_failure_reads_false_false_false_on_both_paths() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let dir = scratch("stop-wire-checkpoint");
    let mut registry = ToolRegistry::with_defaults();
    registry.set_workspace(dir);
    let events = mpsc::channel::<TurnEvent>().0;

    // The first checkpoint, before the provider request, fails.
    let mut history = vec![ChatMsg::user("hello")];
    let result = run_turn_steered_checkpointed_observed(
        &TextClub("hi"),
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
        None,
        &|_| Err("disk full".to_string()),
    );
    let failure = result.as_ref().expect_err("checkpoint failure");
    assert!(
        failure.message.contains("provider request not started"),
        "{}",
        failure.message
    );
    assert_wire(
        result,
        &history,
        ("checkpoint_failure", false, false, false),
        "error",
    );

    // The request checkpoint passes; the tool-intent checkpoint fails.
    let calls = AtomicUsize::new(0);
    let club = CallsClub {
        pause: Duration::ZERO,
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let result = run_turn_steered_checkpointed_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
        None,
        &|_| {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(())
            } else {
                Err("disk full".to_string())
            }
        },
    );
    let failure = result.as_ref().expect_err("checkpoint failure");
    assert!(
        failure.message.contains("tool batch not started"),
        "{}",
        failure.message
    );
    assert_wire(
        result,
        &history,
        ("checkpoint_failure", false, false, false),
        "error",
    );
}

#[test]
fn execution_blocked_reads_false_false_false() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    struct Blocked;
    impl Tool for Blocked {
        fn name(&self) -> &str {
            "shell"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: "shell".into(),
                description: "sandbox helper that cannot confine".into(),
                params: serde_json::json!({"type": "object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            crate::agent::harness::exec::set_sandbox_receipt(Some(serde_json::json!({
                "helper_phase": "landlock",
                "helper_exit": 1,
                "helper_error": "Landlock unavailable; run angel --doctor",
            })));
            Err("outer sandbox helper failed (exit 1)".into())
        }
    }
    struct ShellClub;
    impl Club for ShellClub {
        fn label(&self) -> &str {
            "stop-wire-shell"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "build".into(),
                name: "shell".into(),
                args: serde_json::json!({"command": "true"}),
            }]))
        }
    }
    let dir = scratch("stop-wire-blocked");
    let mut registry = ToolRegistry::new();
    registry.set_workspace(dir);
    registry.register(Box::new(Blocked));
    let mut history = vec![ChatMsg::user("build the candidate")];
    let result = run_turn_observed(
        &ShellClub,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    );
    assert!(result.is_err());
    assert_wire(
        result,
        &history,
        ("execution_blocked", false, false, false),
        "error",
    );
}

#[test]
fn a_kill_cancel_leaves_an_interrupt_reading_true_false_false() {
    // The helper drives a real owned process and an operator cancel. Case
    // (true, false): the model answers after the cancel; (true, true): the
    // provider fails after the cancel (the loop turns that error into the
    // hop-boundary interrupt). Both end as the same four values.
    assert_eq!(
        assert_lifecycle_proc_envelope(true, false),
        w("interrupt", true, false, false)
    );
    assert_eq!(
        assert_lifecycle_proc_envelope(true, true),
        w("interrupt", true, false, false)
    );
}

#[test]
fn an_answer_with_no_kill_stays_an_answer() {
    assert_eq!(
        assert_lifecycle_proc_envelope(false, false),
        w("answer", false, false, false)
    );
}

/// The idle watchdog, landing on a turn that exits through a returned outcome:
/// the hop-boundary interrupt it provoked keeps its `interrupted: true`.
#[test]
fn idle_timeout_on_a_returned_outcome_reads_true_false_false() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let _idle = EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1");
    let trajectories = scratch("stop-wire-idle-trajectories");
    let _log = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let _log_dir = EnvGuard::set(
        "ANGEL_TRAJECTORY_DIR",
        trajectories.to_str().expect("utf-8 path"),
    );
    /// Asks for a tool that blocks until the turn's cancel flag is raised.
    struct Silent;
    impl Club for Silent {
        fn label(&self) -> &str {
            "stop-wire-silent-tool"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("no model".into())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "silent".into(),
                name: "hang".into(),
                args: serde_json::json!({}),
            }]))
        }
    }
    struct Hang;
    impl Tool for Hang {
        fn name(&self) -> &str {
            "hang"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: "hang".into(),
                description: "blocks until cancelled".into(),
                params: serde_json::json!({"type": "object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            Err("fixture requires cancellation".into())
        }
        fn call_with_cancel(
            &self,
            _: &Value,
            cancel: Option<&AtomicBool>,
        ) -> Result<String, String> {
            let cancel = cancel.ok_or("missing cancellation token")?;
            for _ in 0..2_000 {
                if cancel.load(Ordering::Acquire) {
                    return Err("hang cancelled".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err("the idle watchdog never fired".into())
        }
    }
    let dir = scratch("stop-wire-idle-outcome");
    let mut registry = ToolRegistry::new();
    registry.set_workspace(dir);
    registry.register(Box::new(Hang));
    let mut history = vec![ChatMsg::user("silent local fixture")];
    let result = run_turn_observed(
        &Silent,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    );
    assert!(result.is_ok(), "{:?}", result.as_ref().err());
    assert_wire(
        result,
        &history,
        ("idle_timeout", true, false, false),
        "stopped",
    );

    // The trajectory record is written inside the loop, before the watchdog's
    // verdict is stamped on the result, so it keeps the loop's own reason:
    // `interrupt`. The task JSON above says `idle_timeout`. Pinned as is; the
    // operator decides whether the two should agree.
    let log =
        std::fs::read_to_string(trajectories.join(format!("session-{}.jsonl", std::process::id())))
            .expect("the turn wrote a trajectory record");
    let record: Value = log
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .next_back()
        .expect("a trajectory record");
    assert_eq!(record["turn"]["stop_reason"], "interrupt", "{record}");
    let _ = std::fs::remove_dir_all(trajectories);
}

/// The same watchdog, landing on a turn whose interrupt a required-capture
/// failure replaced with a failure: `interrupted` reads false. One stop
/// reason, two values; recorded for the operator, not resolved here.
#[test]
fn idle_timeout_over_a_capture_failure_reads_false_false_false() {
    let _lock = crate::tests::env_lock();
    let _env = quiet();
    let _idle = EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1");
    let _no_retry = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _required = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1");
    let _mode = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    let output = scratch("stop-wire-idle-capture");
    let _dir = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_DIR", output.to_str().expect("utf-8"));
    /// Waits for the idle watchdog's cancel, breaks the capture store, then
    /// fails the provider call.
    struct Stalled {
        output: PathBuf,
    }
    impl Club for Stalled {
        fn label(&self) -> &str {
            "stop-wire-stalled-provider"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("no model".into())
        }
        fn chat_streaming(
            &self,
            _: &[ChatMsg],
            _: &[ToolDef],
            cancel: &AtomicBool,
            _: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            for _ in 0..2_000 {
                if cancel.load(Ordering::Acquire) {
                    for entry in std::fs::read_dir(&self.output).unwrap().flatten() {
                        let runs = entry.path().join("runs");
                        if runs.is_dir() && !runs.is_symlink() {
                            let moved = entry.path().join("runs-moved");
                            std::fs::rename(&runs, &moved).unwrap();
                            std::os::unix::fs::symlink(&moved, &runs).unwrap();
                        }
                    }
                    return Err("connection reset by peer".to_string());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("the idle watchdog never fired");
        }
    }
    let club = Stalled {
        output: output.clone(),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let result = run(&club, &mut history, &AtomicBool::new(false), Some(4));
    assert!(
        result.is_err(),
        "{:?}",
        result.as_ref().ok().map(|o| &o.answer)
    );
    assert_wire(
        result,
        &history,
        ("idle_timeout", false, false, false),
        "error",
    );
    let _ = std::fs::remove_dir_all(output);
}
