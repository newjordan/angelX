//! The turn idle watcher's grace for a due tool-idle escalation, while tool
//! calls overlap: a parallel batch runs its calls on separate threads, and a
//! `code_mode` call reports each nested call as its own call/result pair
//! inside its own.

use super::*;

/// Every knob a bare turn reads, pinned off, so the only thing that can end
/// the turn early is the idle watcher.
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
        EnvGuard::set("ANGEL_ROLLOUT_CAPTURE", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "off"),
        EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
        EnvGuard::unset("ANGEL_TURN_DEADLINE_SECS"),
        EnvGuard::unset("ANGEL_COMPETITION_TURN_DEADLINE_SECS"),
        EnvGuard::unset("ANGEL_PROVIDER_RETRIES"),
    ]
}

/// A tool that returns at once.
struct Quick(&'static str);

impl Tool for Quick {
    fn name(&self) -> &str {
        self.0
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.0.into(),
            description: "returns at once".into(),
            params: serde_json::json!({"type": "object"}),
        }
    }
    fn call(&self, _: &Value) -> Result<String, String> {
        Ok("quick done".into())
    }
}

/// A tool that works for `ms` milliseconds and then returns, unless the turn's
/// cancel flag goes up first. It stands in for a tool whose own idle
/// escalation settles inside the watcher's grace window.
struct Slow {
    name: &'static str,
    ms: u64,
    saw_cancel: Arc<AtomicBool>,
}

impl Tool for Slow {
    fn name(&self) -> &str {
        self.name
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name.into(),
            description: "works for a while".into(),
            params: serde_json::json!({"type": "object"}),
        }
    }
    fn call(&self, _: &Value) -> Result<String, String> {
        Err("fixture requires the cancel flag".into())
    }
    fn call_with_cancel(&self, _: &Value, cancel: Option<&AtomicBool>) -> Result<String, String> {
        let cancel = cancel.ok_or("missing cancellation token")?;
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(self.ms) {
            if cancel.load(Ordering::Acquire) {
                self.saw_cancel.store(true, Ordering::Release);
                return Err("slow tool cancelled".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok("slow done".into())
    }
}

/// Two calls in one batch, both read-only so the harness runs them in
/// parallel; then a plain answer.
struct TwoCallsThenAnswer {
    hops: AtomicUsize,
}

impl Club for TwoCallsThenAnswer {
    fn label(&self) -> &str {
        "idle-watch-two-calls"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Err("no model".into())
    }
    fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(ClubReply::Calls(vec![
                ToolCall {
                    id: "quick".into(),
                    name: "grep".into(),
                    args: serde_json::json!({}),
                },
                ToolCall {
                    id: "slow".into(),
                    name: "outline".into(),
                    args: serde_json::json!({}),
                },
            ]));
        }
        Ok(ClubReply::Text("done".into()))
    }
}

/// The turn idle limit and the tool idle limit are both one second, so a
/// tool-idle escalation is "due within the grace" and the watcher owes the
/// slow call its three seconds. The quick call returns first. Before the
/// watcher counted calls, that one result cleared "a tool is running" while
/// the slow call still ran, the grace was lost, and the turn was cut as idle a
/// second in.
#[test]
fn a_finished_sibling_does_not_take_the_grace_from_a_call_still_running() {
    let _lock = crate::tests::env_lock();
    let _quiet = quiet();
    let _turn_idle = EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1");
    let _tool_idle = EnvGuard::set("ANGEL_TOOL_IDLE_SECS", "1");
    let saw_cancel = Arc::new(AtomicBool::new(false));
    let dir = scratch("idle-watch-sibling");
    let mut registry = ToolRegistry::new();
    registry.set_workspace(dir.clone());
    registry.register(Box::new(Quick("grep")));
    registry.register(Box::new(Slow {
        name: "outline",
        ms: 2_200,
        saw_cancel: Arc::clone(&saw_cancel),
    }));
    let club = TwoCallsThenAnswer {
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("two overlapping calls")];
    let (tx, rx) = mpsc::channel();
    let result = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &tx,
    );
    let events: Vec<_> = rx.try_iter().collect();
    let results = events
        .iter()
        .filter(|event| matches!(event, TurnEvent::ToolResult { .. }))
        .count();
    assert_eq!(results, 2, "both calls reported a result");
    assert!(
        !saw_cancel.load(Ordering::Acquire),
        "the idle watcher cancelled a call that was still inside its grace window"
    );
    let outcome = result.expect("the turn runs to its answer");
    assert_eq!(
        outcome.stop_reason,
        TurnStopReason::Answer,
        "{}",
        outcome.answer
    );
    assert_eq!(outcome.answer, "done");
    let _ = std::fs::remove_dir_all(dir);
}

/// With one call in flight, nothing changed: the grace holds and the turn
/// settles. Pins the single-call behaviour the counter must keep.
#[test]
fn a_lone_slow_call_keeps_its_grace() {
    struct OneCallThenAnswer {
        hops: AtomicUsize,
    }
    impl Club for OneCallThenAnswer {
        fn label(&self) -> &str {
            "idle-watch-one-call"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("no model".into())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
                return Ok(ClubReply::Calls(vec![ToolCall {
                    id: "slow".into(),
                    name: "outline".into(),
                    args: serde_json::json!({}),
                }]));
            }
            Ok(ClubReply::Text("done".into()))
        }
    }
    let _lock = crate::tests::env_lock();
    let _quiet = quiet();
    let _turn_idle = EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1");
    let _tool_idle = EnvGuard::set("ANGEL_TOOL_IDLE_SECS", "1");
    let saw_cancel = Arc::new(AtomicBool::new(false));
    let dir = scratch("idle-watch-lone");
    let mut registry = ToolRegistry::new();
    registry.set_workspace(dir.clone());
    registry.register(Box::new(Slow {
        name: "outline",
        ms: 2_200,
        saw_cancel: Arc::clone(&saw_cancel),
    }));
    let club = OneCallThenAnswer {
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("one slow call")];
    let result = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    );
    assert!(!saw_cancel.load(Ordering::Acquire));
    let outcome = result.expect("the turn runs to its answer");
    assert_eq!(
        outcome.stop_reason,
        TurnStopReason::Answer,
        "{}",
        outcome.answer
    );
    let _ = std::fs::remove_dir_all(dir);
}
