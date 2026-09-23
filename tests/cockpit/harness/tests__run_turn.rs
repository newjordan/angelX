//! run_turn loop, post-edit diagnostics, first-write guard, and spin redirect coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory. Shared helpers
//! (`EnvGuard`, `scratch`) remain in the parent module. `ScriptedClub` remains in the parent module as a shared fixture.

use super::*;
use serde_json::json;

#[cfg(unix)]
#[test]
fn run_turn_r04e_blocked_identity_probe_still_requests_with_partial_notice() {
    use std::os::unix::fs::PermissionsExt;
    let _guard = crate::tests::env_lock();
    let root = scratch("r04e_blocked_git");
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    std::fs::write(bin.join("git"), "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    std::fs::set_permissions(bin.join("git"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut registry = ToolRegistry::with_defaults();
    registry.set_workspace(root.clone());
    let _path = EnvGuard::set("PATH", &format!("{}:/usr/bin:/bin", bin.display()));
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _trace = EnvGuard::set("ANGEL_TURN_PHASE_TRACE", "1");
    struct FirstRequest(std::time::Instant);
    impl Club for FirstRequest {
        fn label(&self) -> &str {
            "r04e-scripted"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok("done".into())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let elapsed = self.0.elapsed();
            eprintln!("r04e first_request_elapsed_ms={}", elapsed.as_millis());
            assert!(elapsed < std::time::Duration::from_secs(5));
            Ok(ClubReply::Text(
                "The fixture completed successfully.".into(),
            ))
        }
    }
    let (events, received) = mpsc::channel();
    let answer = run_turn(
        &FirstRequest(std::time::Instant::now()),
        &registry,
        &mut vec![ChatMsg::user("Reply with the fixture result")],
        &AtomicBool::new(false),
        Some(2),
        &events,
    )
    .unwrap();
    assert!(answer.contains("fixture completed"));
    assert!(received.try_iter().any(|event| matches!(event,
        TurnEvent::Notice(text) if text.contains("workspace_identity: partial"))));
    std::fs::remove_dir_all(root).unwrap();
}

struct BlockedSandboxClub {
    hops: AtomicUsize,
    continue_after_failure: bool,
}

impl Club for BlockedSandboxClub {
    fn respond(&self, _: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        "sandbox-blocker-replay"
    }
    fn chat(&self, messages: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        let hop = self.hops.fetch_add(1, Ordering::SeqCst);
        if hop > 0 && self.continue_after_failure {
            return Ok(ClubReply::Text("Diagnostic receipts collected.".into()));
        }
        assert_eq!(
            hop,
            0,
            "a failed native sandbox must not spend another model request; tool receipts: {:?}",
            messages
                .iter()
                .filter(|m| m.role == ChatRole::Tool)
                .map(|m| &m.content)
                .collect::<Vec<_>>()
        );
        Ok(ClubReply::Calls(vec![
            ToolCall {
                id: "native-build".into(),
                name: "shell".into(),
                args: serde_json::json!({"command": "printf '%s\\n' 'bwrap: setting up uid map: Permission denied' >&2; exit 1"}),
            },
            ToolCall {
                id: "synthetic-pass".into(),
                name: "reverse".into(),
                args: serde_json::json!({"text": "synthetic probe passed"}),
            },
        ]))
    }
}

#[test]
fn failed_native_sandbox_stops_before_next_request_despite_successful_batch_peer() {
    let _guard = crate::tests::env_lock();
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    // Simulate an outer helper's dedicated status channel. Candidate stderr
    // alone is explicitly insufficient to classify this failure.
    struct FailedHelper;
    impl Tool for FailedHelper {
        fn name(&self) -> &str {
            "shell"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: "shell".into(),
                description: "offline helper status replay".into(),
                params: json!({"type": "object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            let mut command = std::process::Command::new("python3");
            command.args(["-c", "import os; fd=int(os.environ['ANGEL_INTERNAL_SANDBOX_STATUS_FD']); os.write(fd, b'{\"helper_error\":\"Landlock unavailable; run angel --doctor\",\"helper_phase\":\"landlock\",\"helper_exit\":1}'); os.close(fd); raise SystemExit(1)"]);
            let channel = crate::agent::sandbox::status::attach(&mut command).unwrap();
            assert!(!command.status().unwrap().success());
            crate::agent::harness::exec::set_sandbox_receipt(channel.receive());
            Err("outer sandbox helper failed (exit 1)".into())
        }
    }
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ReverseTool));
    registry.register(Box::new(FailedHelper));
    let club = BlockedSandboxClub {
        hops: AtomicUsize::new(0),
        continue_after_failure: false,
    };
    let mut history = vec![ChatMsg::user(
        "Run the required candidate build and retain the synthetic probe",
    )];
    let (events, received) = mpsc::channel();
    let failure = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(800),
        &events,
    )
    .unwrap_err();
    assert_eq!(failure.stop_reason, TurnStopReason::ExecutionBlocked);
    assert_eq!(club.hops.load(Ordering::SeqCst), 1);
    assert!(is_execution_blocker(&failure.message));
    for id in ["native-build", "synthetic-pass"] {
        assert_eq!(
            history
                .iter()
                .filter(|m| m.role == ChatRole::Tool && m.tool_call_id.as_deref() == Some(id))
                .count(),
            1
        );
    }
    assert_eq!(
        received
            .try_iter()
            .filter(|event| matches!(event, TurnEvent::ToolResult { .. }))
            .count(),
        2
    );
}

#[test]
fn nested_bwrap_failure_continues_to_next_request() {
    let _guard = crate::tests::env_lock();
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    struct NestedBwrapClub(AtomicUsize);
    impl Club for NestedBwrapClub {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "nested-bwrap-replay"
        }
        fn chat(&self, messages: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "nested".into(),
                    name: "shell".into(),
                    args: json!({"command": "printf '%s\\n' 'bwrap: setting up uid map: Permission denied' >&2; exit 1"}),
                }]))
            } else {
                assert!(
                    messages
                        .iter()
                        .any(|m| m.role == ChatRole::Tool && m.content.contains("uid map"))
                );
                let result = messages
                    .iter()
                    .find(|m| m.role == ChatRole::Tool && m.content.contains("uid map"))
                    .unwrap();
                assert_eq!(result.content.matches("[doctor hint:").count(), 1);
                Ok(ClubReply::Text("The candidate's nested sandbox failed; host repair is needed for that command.".into()))
            }
        }
    }
    let registry = ToolRegistry::with_defaults();
    let club = NestedBwrapClub(AtomicUsize::new(0));
    let mut history = vec![ChatMsg::user("Diagnose the nested sandbox failure.")];
    let (events, _) = mpsc::channel();
    run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(800),
        &events,
    )
    .unwrap();
    assert_eq!(club.0.load(Ordering::SeqCst), 2);
}

#[test]
fn sandbox_fallback_notice_and_envelope_are_bound_to_the_call() {
    let _guard = crate::tests::env_lock();
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    struct FallbackHelper;
    impl Tool for FallbackHelper {
        fn name(&self) -> &str {
            "shell"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: "shell".into(),
                description: "offline fallback replay".into(),
                params: json!({"type":"object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            let cause = crate::agent::sandbox::compatibility::classify(
                true,
                true,
                true,
                false,
                "bwrap: setting up uid map: Permission denied",
            )
            .unwrap();
            crate::agent::harness::exec::set_sandbox_receipt(Some(json!({
                "sandbox_profile":"landlock-only", "cause":cause.class(), "notice":cause.notice(), "aliases_copied":[], "aliases_unprotected":[]
            })));
            Ok("diagnostic command completed".into())
        }
    }
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ReverseTool));
    registry.register(Box::new(FallbackHelper));
    let club = BlockedSandboxClub {
        hops: AtomicUsize::new(0),
        continue_after_failure: true,
    };
    let mut history = vec![ChatMsg::user(
        "Diagnose the sandbox and retain both receipts.",
    )];
    let (events, received) = mpsc::channel();
    run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(800),
        &events,
    )
    .unwrap();
    assert_eq!(club.hops.load(Ordering::SeqCst), 2);
    let ledger = crate::agent::harness::trajectory::tool_ledger_snapshot();
    let shell = ledger.iter().find(|row| row["tool"] == "shell").unwrap();
    assert_eq!(shell["sandbox_profile"], "landlock-only");
    assert_eq!(shell["sandbox"]["cause"], "userns denied by AppArmor");
    assert!(
        ledger
            .iter()
            .find(|row| row["tool"] == "reverse")
            .unwrap()
            .get("sandbox_profile")
            .is_none()
    );
    assert_eq!(received.try_iter().filter(|event| matches!(event, TurnEvent::Notice(text) if text.contains("→ Landlock-only confinement"))).count(), 1);
}

// --- run_turn / post-edit / first-write / spin suite ------------------------

struct RedAcceptanceClaimClub {
    calls: AtomicUsize,
}

impl Club for RedAcceptanceClaimClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("unused".into())
    }

    fn label(&self) -> &str {
        "red-acceptance-claim"
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ClubReply::Text(
            "Implemented everything; the task is complete.".into(),
        ))
    }
}

#[test]
fn red_task_acceptance_contract_cannot_be_bypassed_by_repeated_done_answers() {
    let _guard = crate::tests::env_lock();
    let _accept = EnvGuard::set("ANGEL_TASK_ACCEPT_CMD", "test -f never-created");
    let _accept_rejections = EnvGuard::set("ANGEL_TASK_ACCEPT_REJECTIONS", "2");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _no_edit = EnvGuard::set("ANGEL_NO_EDIT_ANSWER_GUARD", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let root = scratch("red_task_accept_claim");
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("git fixture command starts");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "angel@example.invalid"]);
    git(&["config", "user.name", "Angel Test"]);
    std::fs::write(root.join("seed.txt"), "seed\n").unwrap();
    git(&["add", "seed.txt"]);
    git(&["commit", "-q", "-m", "seed"]);
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.clone());
    let club = RedAcceptanceClaimClub {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("create the required file")];
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("red acceptance stops with truthful outcome metadata");

    assert_eq!(outcome.stop_reason, TurnStopReason::AcceptanceStop);
    assert_eq!(club.calls.load(Ordering::SeqCst), 2);
    assert!(outcome.answer.contains("acceptance remained red"));
    let receipt = outcome.acceptance.expect("acceptance telemetry");
    assert!(receipt.armed);
    assert!(!receipt.terminal_passed);
    assert_eq!(
        receipt.post_checks, 0,
        "unchanged red replays without a process"
    );
    assert!(
        history
            .iter()
            .filter(|message| {
                message.role == ChatRole::Harness && message.content.contains(TASK_ACCEPT_RED_NUDGE)
            })
            .count()
            >= 2
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_turn_dispatches_tools_then_answers() {
    // Builds the default registry (reads tool-gating env like
    // ANGEL_TOOL_SEARCH_ACTIVE_MAX / ANGEL_TOOL_SCHEMA_PROFILE), so hold the
    // shared env lock — otherwise a concurrent profile-setting test leaks into
    // this turn's tool activation and flakes the parallel runner.
    let _guard = crate::tests::env_lock();
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let club = ScriptedClub {
        hops: AtomicUsize::new(0),
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("reverse hello")];
    let answer = run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert_eq!(answer, "done: olleh");
    // Protocol integrity: user → tool result → final answer. Broker/system
    // context may add harness/system rows under other env; don't require a
    // brittle total non-harness count (flaked 6 vs 4 under parallel suite).
    assert_eq!(
        history.iter().filter(|m| m.role == ChatRole::User).count(),
        1
    );
    assert!(
        history
            .iter()
            .any(|m| m.role == ChatRole::Tool && m.content.as_ref() == "olleh")
    );
    assert!(
        history
            .iter()
            .any(|m| m.role == ChatRole::Assistant && m.content.contains("done: olleh")),
        "final assistant answer must land in history"
    );
    assert_eq!(
        club.hops.load(Ordering::SeqCst),
        2,
        "one tool hop + one answer hop"
    );
}

struct MeteredMilestoneClub {
    hops: AtomicUsize,
    total_input: std::sync::atomic::AtomicU64,
}

impl Club for MeteredMilestoneClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("unused".into())
    }

    fn label(&self) -> &str {
        "glm-milestone-test"
    }

    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let hop = self.hops.fetch_add(1, Ordering::SeqCst);
        self.total_input.fetch_add(600_000, Ordering::SeqCst);
        if hop == 0 {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "million-c1".into(),
                name: "reverse".into(),
                args: serde_json::json!({ "text": "hello" }),
            }]))
        } else {
            let last_tool = messages
                .iter()
                .rev()
                .find(|message| message.role == ChatRole::Tool)
                .map(|message| message.content.clone())
                .unwrap_or_default();
            Ok(ClubReply::Text(format!("done: {last_tool}")))
        }
    }

    fn token_usage(&self) -> Option<crate::agent::club::TokenUsage> {
        let turns = self.hops.load(Ordering::SeqCst) as u64;
        let total_input = self.total_input.load(Ordering::SeqCst);
        Some(crate::agent::club::TokenUsage {
            turns,
            last_input: u64::from(turns > 0) * 600_000,
            total_input,
            ..Default::default()
        })
    }
}

#[test]
fn million_metered_input_emits_telemetry_and_the_turn_keeps_running() {
    let _guard = crate::tests::env_lock();
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");
    let club = MeteredMilestoneClub {
        hops: AtomicUsize::new(0),
        total_input: std::sync::atomic::AtomicU64::new(0),
    };
    let registry = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("reverse hello")];
    let (event_tx, event_rx) = mpsc::channel::<TurnEvent>();

    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &event_tx,
    )
    .expect("the million-token milestone must not stop the turn");

    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(outcome.answer, "done: olleh");
    assert_eq!(club.hops.load(Ordering::SeqCst), 2);
    let milestones = event_rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::SpendMilestone { input_tokens } => Some(input_tokens),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(milestones, vec![1_000_000]);
    assert_eq!(
        crossed_sota_input_milestone(900_000, 2_100_000),
        Some(2_000_000)
    );
}

#[test]
fn multi_hop_checkpoint_tracks_each_provider_request_and_tool_intent() {
    let _guard = crate::tests::env_lock();
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");

    struct RecordingClub {
        scripted: ScriptedClub,
        requests: Mutex<Vec<Vec<ChatMsg>>>,
    }
    impl Club for RecordingClub {
        fn respond(&self, prompt: &str) -> Result<String, String> {
            self.scripted.respond(prompt)
        }
        fn label(&self) -> &str {
            self.scripted.label()
        }
        fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            self.requests.lock().unwrap().push(messages.to_vec());
            self.scripted.chat(messages, tools)
        }
    }
    let club = RecordingClub {
        scripted: ScriptedClub {
            hops: AtomicUsize::new(0),
        },
        requests: Mutex::new(Vec::new()),
    };
    let registry = ToolRegistry::with_defaults();
    // Harness context may follow operator input before the first request.
    let mut history = vec![
        ChatMsg::user("reverse hello"),
        ChatMsg::harness("checkpoint fixture context"),
    ];
    let boundaries = Mutex::new(Vec::new());
    let answer = run_turn_steered_checkpointed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
        None,
        &|prefix| {
            boundaries.lock().unwrap().push(prefix.to_vec());
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(answer, "done: olleh");
    let boundaries = boundaries.lock().unwrap();
    let requests = club.requests.lock().unwrap();
    assert_eq!(boundaries.len(), 3);
    assert_eq!(requests.len(), 2);
    for (checkpoint, request) in [
        (&boundaries[0], &requests[0]),
        (&boundaries[2], &requests[1]),
    ] {
        assert_eq!(
            serde_json::to_value(checkpoint).unwrap(),
            serde_json::to_value(request).unwrap(),
            "each provider request must match its durable input checkpoint"
        );
    }
    let intent = boundaries[1].last().unwrap();
    assert_eq!(intent.role, ChatRole::Assistant);
    assert_eq!(intent.tool_calls.len(), 1);
    assert_eq!(intent.tool_calls[0].id, "c1");
    let outcome = boundaries[2].last().unwrap();
    assert_eq!(outcome.role, ChatRole::Tool);
    assert_eq!(outcome.tool_call_id.as_deref(), Some("c1"));
    assert_eq!(outcome.content.as_ref(), "olleh");
}

#[test]
fn failed_tool_intent_checkpoint_prevents_dispatch() {
    let _guard = crate::tests::env_lock();
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");

    struct CountingEffect(Arc<AtomicUsize>);
    impl Tool for CountingEffect {
        fn name(&self) -> &str {
            "write_file"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().to_string(),
                description: "count a test side effect".to_string(),
                params: serde_json::json!({"type": "object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("effect happened".to_string())
        }
    }

    struct EffectClub;
    impl Club for EffectClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok("unused".to_string())
        }
        fn label(&self) -> &str {
            "checkpoint-effect"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "effect-1".to_string(),
                name: "write_file".to_string(),
                args: serde_json::json!({"path": "outside-state"}),
            }]))
        }
    }

    let effects = Arc::new(AtomicUsize::new(0));
    let checkpoints = AtomicUsize::new(0);
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingEffect(Arc::clone(&effects))));
    let mut history = vec![ChatMsg::user("perform the effect")];
    let failure = run_turn_steered_checkpointed(
        &EffectClub,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(2),
        &mpsc::channel::<TurnEvent>().0,
        None,
        &|prefix| {
            checkpoints.fetch_add(1, Ordering::SeqCst);
            let boundary = prefix.last().expect("checkpoint includes history");
            if boundary.role == ChatRole::Assistant && !boundary.tool_calls.is_empty() {
                assert_eq!(boundary.tool_calls[0].id, "effect-1");
                Err("durability unavailable".to_string())
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();

    assert!(failure.contains("tool batch not started"));
    assert_eq!(checkpoints.load(Ordering::SeqCst), 2);
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert!(history.iter().all(|message| message.role != ChatRole::Tool));
}

#[test]
fn failed_request_checkpoint_prevents_provider_dispatch() {
    let _guard = crate::tests::env_lock();
    struct CountingClub(Arc<AtomicUsize>);
    impl Club for CountingClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            unreachable!("chat owns this test")
        }
        fn label(&self) -> &str {
            "checkpoint-provider"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ClubReply::Text("must not run".to_string()))
        }
    }

    let provider_calls = Arc::new(AtomicUsize::new(0));
    let checkpoints = AtomicUsize::new(0);
    let mut history = vec![
        ChatMsg::user("checkpoint before provider"),
        ChatMsg::harness("checkpoint fixture context"),
    ];
    let supplied_prefix = serde_json::to_value(&history).unwrap();
    let failure = run_turn_steered_checkpointed(
        &CountingClub(Arc::clone(&provider_calls)),
        &ToolRegistry::new(),
        &mut history,
        &AtomicBool::new(false),
        Some(1),
        &mpsc::channel::<TurnEvent>().0,
        None,
        &|prefix| {
            checkpoints.fetch_add(1, Ordering::SeqCst);
            assert_eq!(serde_json::to_value(&prefix[..2]).unwrap(), supplied_prefix);
            Err("durability unavailable".to_string())
        },
    )
    .unwrap_err();

    assert!(
        failure.contains("provider request not started"),
        "{failure}"
    );
    assert_eq!(checkpoints.load(Ordering::SeqCst), 1);
    assert_eq!(provider_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn run_turn_records_each_real_policy_call_in_order_when_local_capture_is_enabled() {
    let _guard = crate::tests::env_lock();
    // Pin hop-shaping env so parallel suite load cannot inject skill-hint /
    // first-write / competition / verify / deferred / empty-reply retries that
    // inflate attempt count (flaked 4 vs 2 under harness gate load).
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _gpu_moa = EnvGuard::unset("ANGEL_GPU_COMP_LOCAL_MOA");
    let _spin = EnvGuard::set("ANGEL_SPIN_PERTURB", "0");
    let _spin_limit = EnvGuard::set("ANGEL_SPIN_LIMIT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _no_edit = EnvGuard::set("ANGEL_NO_EDIT_ANSWER_GUARD", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");
    let _storm = EnvGuard::set("ANGEL_TOOLCALL_STORM", "0");
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let output = scratch("rollout_boundary");
    let output_text = output.to_string_lossy().into_owned();
    let _mode = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    let _dir = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_DIR", &output_text);
    let club = ScriptedClub {
        hops: AtomicUsize::new(0),
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("reverse hello")];

    let answer = run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert_eq!(answer, "done: olleh");

    let repo_key = crate::platform::workspace_store::repo_identity(reg.current_workspace()).key;
    let runs = output.join(repo_key).join("runs");
    let run_dir = std::fs::read_dir(&runs)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(run_dir.len(), 1);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run_dir[0].join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["schema"], "angel-harness-rollout/v1");
    assert_eq!(manifest["status"], "finalized");
    // ScriptedClub: one tool hop + one final text hop. Protocol kinds matter more
    // than an exact attempt count if a single empty-reply retry ever races.
    let attempts = manifest["attempts"].as_array().unwrap();
    assert!(
        (2..=3).contains(&attempts.len()),
        "expected 2..=3 attempts, got {}: {:?}",
        attempts.len(),
        attempts
            .iter()
            .map(|a| a["response"]["action"]["kind"].clone())
            .collect::<Vec<_>>()
    );
    let kinds: Vec<&str> = attempts
        .iter()
        .filter_map(|a| a["response"]["action"]["kind"].as_str())
        .collect();
    assert!(
        kinds.contains(&"tool_calls"),
        "must record the tool hop: {kinds:?}"
    );
    assert_eq!(
        kinds.last().copied(),
        Some("text"),
        "final attempt must be the text answer: {kinds:?}"
    );
    assert_eq!(manifest["capture"]["private_reasoning_captured"], false);
    assert_eq!(manifest["capture"]["provider_headers_captured"], false);
    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn required_rollout_returns_the_exact_sealed_rollout_id() {
    let _guard = crate::tests::env_lock();
    let output = scratch("required_rollout_id");
    let output_text = output.to_string_lossy().into_owned();
    let _mode = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    let _required = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1");
    let _dir = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_DIR", &output_text);
    let club = ScriptedClub {
        hops: AtomicUsize::new(0),
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("reverse hello")];

    let outcome = run_turn_observed(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    let rollout_id = outcome.rollout_id.expect("required capture returns an id");
    assert!(rollout_id.starts_with("rol-"), "{rollout_id}");

    let repo_key = crate::platform::workspace_store::repo_identity(reg.current_workspace()).key;
    assert!(
        output
            .join(repo_key)
            .join("runs")
            .join(rollout_id)
            .join("manifest.json")
            .is_file()
    );
    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn required_rollout_fails_before_provider_spend_when_capture_is_off() {
    let _guard = crate::tests::env_lock();
    let _mode = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "off");
    let _required = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1");
    let club = ScriptedClub {
        hops: AtomicUsize::new(0),
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("must not be sent")];

    let failure = run_turn_observed(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap_err();
    assert_eq!(failure.stop_reason, TurnStopReason::CaptureFailure);
    assert_eq!(failure.hops, 0);
    assert_eq!(club.hops.load(Ordering::SeqCst), 0);
}

#[test]
fn required_rollout_fails_closed_when_store_startup_is_unavailable() {
    let _guard = crate::tests::env_lock();
    let output = scratch("required_rollout_unavailable");
    let blocked_root = output.join("not-a-directory");
    std::fs::write(&blocked_root, b"not a directory").unwrap();
    let output_text = blocked_root.to_string_lossy().into_owned();
    let _mode = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    let _required = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1");
    let _dir = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_DIR", &output_text);
    let club = ScriptedClub {
        hops: AtomicUsize::new(0),
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("must not be sent")];

    let failure = run_turn_observed(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap_err();
    assert_eq!(failure.stop_reason, TurnStopReason::CaptureFailure);
    assert_eq!(club.hops.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn run_turn_advertises_discovered_tool_on_the_immediately_following_hop() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "essential");
    let _cap = EnvGuard::set("ANGEL_TOOL_SEARCH_ACTIVE_MAX", "2");

    struct DiscoveryClub {
        step: AtomicUsize,
        schemas: Mutex<Vec<Vec<String>>>,
    }
    impl Club for DiscoveryClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "discovery"
        }
        fn chat(&self, _messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            let names = tools
                .iter()
                .map(|tool| tool.name.clone())
                .collect::<Vec<_>>();
            self.schemas.lock().unwrap().push(names.clone());
            match self.step.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    assert!(names.iter().any(|name| name == "tool_search"));
                    assert!(!names.iter().any(|name| name == "reverse"));
                    Ok(ClubReply::Calls(vec![ToolCall {
                        id: "search".into(),
                        name: "tool_search".into(),
                        args: serde_json::json!({"query":"reverse text", "limit":1}),
                    }]))
                }
                1 => {
                    assert!(
                        names.iter().any(|name| name == "reverse"),
                        "search result was not activated on the next provider request"
                    );
                    Ok(ClubReply::Calls(vec![ToolCall {
                        id: "reverse".into(),
                        name: "reverse".into(),
                        args: serde_json::json!({"text":"abc"}),
                    }]))
                }
                _ => Ok(ClubReply::Text("done".into())),
            }
        }
    }

    let club = DiscoveryClub {
        step: AtomicUsize::new(0),
        schemas: Mutex::new(Vec::new()),
    };
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let mut history = vec![ChatMsg::user("reverse abc using a discovered tool")];
    let answer = run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert_eq!(answer, "done");
    let schemas = club.schemas.lock().unwrap();
    assert_eq!(schemas.len(), 3);
    assert!(!schemas[0].contains(&"reverse".to_string()));
    assert!(schemas[1].contains(&"reverse".to_string()));
    assert!(schemas[2].contains(&"reverse".to_string()));
}

#[test]
fn ranged_read_file_stays_one_local_tool_hop() {
    // `run_turn` consults process-wide completion/action settings. Serialize
    // with tests that temporarily override those variables so this remains a
    // one-hop file-read assertion under the parallel test runner.
    let _guard = crate::tests::env_lock();
    // A8: suite load can leave skill/verify/first-write/deferred governors
    // armed and force extra model hops (flake: hops 4 vs 2). Pin them off.
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _no_edit = EnvGuard::set("ANGEL_NO_EDIT_ANSWER_GUARD", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");

    struct ReadPageThenAnswer {
        hops: AtomicUsize,
    }
    impl Club for ReadPageThenAnswer {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "read-page-test"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "page".into(),
                    name: "read_file".into(),
                    args: serde_json::json!({"path":"large.txt","offset":401,"limit":3}),
                }]))
            } else {
                Ok(ClubReply::Text("done".to_string()))
            }
        }
    }

    let root = scratch("read_page_turn");
    let source = (1..=800)
        .map(|line| format!("turn-line-{line:04}\n"))
        .collect::<String>();
    std::fs::write(root.join("large.txt"), source).unwrap();
    let mut registry = ToolRegistry::new();
    register_file_tools(&mut registry, root.clone());
    let club = ReadPageThenAnswer {
        hops: AtomicUsize::new(0),
    };
    let (events, received) = mpsc::channel();
    let mut history = vec![ChatMsg::user("read the focused range")];

    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(6),
        &events,
    )
    .unwrap();

    assert_eq!(answer, "done");
    let hops = club.hops.load(Ordering::SeqCst);
    assert!(
        (2..=3).contains(&hops),
        "tool hop then answer (allow one optional policy nudge hop), got {hops}"
    );
    let tool_results: Vec<_> = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .collect();
    assert_eq!(tool_results.len(), 1, "exactly one ranged read result");
    assert!(tool_results[0].content.contains("turn-line-0401"));
    assert!(tool_results[0].content.contains("turn-line-0403"));
    assert!(!tool_results[0].content.contains("turn-line-0400"));
    assert!(!tool_results[0].content.contains("middle line(s) elided"));

    let tool_calls: Vec<_> = received
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::ToolCall { name, .. } => Some(name),
            _ => None,
        })
        .collect();
    assert_eq!(
        tool_calls,
        ["read_file"],
        "no helper command or extra tool hop"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn action_capsule_observe_is_ui_only_and_does_not_add_a_model_hop() {
    let _guard = crate::tests::env_lock();
    let _mode = EnvGuard::set("ANGEL_ACTION_CAPSULES", "observe");
    // This test isolates the capsule's hop behavior. The completion-policy
    // tests separately prove that a mutation earns one final verification hop.
    let _verification = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    struct CountingWrite(Arc<AtomicUsize>);
    impl Tool for CountingWrite {
        fn name(&self) -> &str {
            "write_file"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: "write_file".to_string(),
                description: "test write".to_string(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("wrote test file".to_string())
        }
    }

    struct WriteThenAnswer {
        hops: AtomicUsize,
    }
    impl Club for WriteThenAnswer {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "capsule-test"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "write_1".into(),
                    name: "write_file".into(),
                    args: serde_json::json!({"path":"src/lib.rs","content":"x"}),
                }]))
            } else {
                Ok(ClubReply::Text("done".to_string()))
            }
        }
    }

    let writes = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.enable_action_capsules();
    registry.register(Box::new(CountingWrite(Arc::clone(&writes))));
    let club = WriteThenAnswer {
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("make a change")];
    let (events, event_rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();

    assert_eq!(answer, "done");
    assert_eq!(club.hops.load(Ordering::SeqCst), 2, "no extra model turn");
    assert_eq!(
        writes.load(Ordering::SeqCst),
        1,
        "write dispatch stays singular"
    );
    assert!(
        history
            .iter()
            .all(|m| !m.content.contains("action capsule"))
    );
    let notices: Vec<String> = event_rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(text) => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        notices
            .iter()
            .any(|text| text.contains("scoped operation(s) ready")),
        "preview missing: {notices:?}"
    );
    assert!(
        notices
            .iter()
            .any(|text| text.contains("action receipt · write_file applied")),
        "receipt missing: {notices:?}"
    );
}

#[test]
fn post_edit_diagnostics_cover_patch_targets_filter_noise_and_hard_cap_output() {
    let _guard = crate::tests::env_lock();
    let _paths = EnvGuard::set("ANGEL_POST_EDIT_DIAGNOSTIC_MAX_PATHS", "2");
    let _bytes = EnvGuard::set("ANGEL_POST_EDIT_DIAGNOSTIC_MAX_BYTES", "512");
    let _deadline = EnvGuard::set("ANGEL_POST_EDIT_DIAGNOSTIC_TIMEOUT_MS", "50");

    struct Diagnostics(std::sync::Arc<AtomicUsize>);
    impl Tool for Diagnostics {
        fn name(&self) -> &str {
            "lsp_diagnostics"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "fixed diagnostic fixture".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, args: &Value) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            let path = args["path"].as_str().unwrap();
            assert_eq!(args["_warm_only"], true);
            assert_eq!(args["_deadline_ms"], 50);
            Ok(format!(
                "{path}: 3 diagnostic(s)\n  error 1:1  {}\n  warning 2:1  warning-noise\n  error 3:1  second-error",
                "unicode-λ".repeat(90)
            ))
        }
    }

    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Diagnostics(std::sync::Arc::clone(&calls))));
    let args = serde_json::json!({
        "diff": "*** Begin Patch\n*** Update File: src/c.rs\n@@\n-old\n+new-c\n*** Update File: src/a.rs\n@@\n-old\n+new-a\n*** Update File: src/b.rs\n@@\n-old\n+new-b\n*** End Patch"
    });
    let targets = crate::knowledge::cut::mutation_targets("apply_patch", &args);
    assert_eq!(
        targets.len(),
        3,
        "fixture must exercise multi-file patch fanout"
    );
    let counters = PostEditDiagnosticCounters::default();
    let output = maybe_lsp_postcheck(&registry, true, &targets, "applied patch".into(), &counters);

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(counters.attempts.get(), 2);
    assert_eq!(counters.findings.get(), 4);
    assert_eq!(counters.failures.get(), 0);
    assert_eq!(counters.paths_skipped.get(), 1);
    assert!(counters.output_bytes.get() <= 512);
    assert!(output.contains("[post-edit LSP"));
    assert!(output.contains("error 1:1"));
    assert!(
        !output.contains("warning-noise"),
        "warnings waste repair context"
    );
    assert!(output.contains("post-edit diagnostics truncated"));
    assert!(output.is_char_boundary(output.len()));
}

#[test]
fn post_edit_diagnostics_off_and_lsp_failure_preserve_exact_mutation_receipt() {
    struct FailingDiagnostics(std::sync::Arc<AtomicUsize>);
    impl Tool for FailingDiagnostics {
        fn name(&self) -> &str {
            "lsp_diagnostics"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "failing diagnostic fixture".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err("server unavailable".into())
        }
    }

    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(FailingDiagnostics(std::sync::Arc::clone(&calls))));
    let targets = vec!["src/lib.rs".to_string()];
    let off = PostEditDiagnosticCounters::default();
    assert_eq!(
        maybe_lsp_postcheck(&registry, false, &targets, "edited".into(), &off),
        "edited"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0, "off arm must do zero work");

    let on = PostEditDiagnosticCounters::default();
    assert_eq!(
        maybe_lsp_postcheck(&registry, true, &targets, "edited".into(), &on),
        "edited",
        "diagnostic failure must not convert a successful edit into failure"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(on.attempts.get(), 1);
    assert_eq!(on.failures.get(), 1);
    assert_eq!(on.output_bytes.get(), 0);
}

#[test]
fn post_edit_diagnostic_reaches_next_provider_request_without_an_extra_tool_hop() {
    let _guard = crate::tests::env_lock();
    let _enabled = EnvGuard::set("ANGEL_POST_EDIT_DIAGNOSTICS", "1");
    let _verify = EnvGuard::set("ANGEL_CUT_VERIFY", "0");
    let _completion = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    struct StubTool {
        name: &'static str,
        result: &'static str,
    }
    impl Tool for StubTool {
        fn name(&self) -> &str {
            self.name
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name.into(),
                description: "same-hop fixture".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            Ok(self.result.into())
        }
    }

    struct EditThenRepair {
        hop: AtomicUsize,
    }
    impl Club for EditThenRepair {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "same-hop-diagnostics"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(if self.hop.fetch_add(1, Ordering::SeqCst) == 0 {
                ClubReply::Calls(vec![ToolCall {
                    id: "edit".into(),
                    name: "str_replace".into(),
                    args: serde_json::json!({
                        "path":"src/lib.rs", "old":"old", "new":"new"
                    }),
                }])
            } else if messages.iter().any(|message| {
                message.role == ChatRole::Tool
                    && message.content.contains("error 9:3  type mismatch")
            }) {
                ClubReply::Text("repair evidence received".into())
            } else {
                ClubReply::Text("diagnostic missing".into())
            })
        }
    }

    let mut registry = ToolRegistry::new();
    registry.register(Box::new(StubTool {
        name: "str_replace",
        result: "edited src/lib.rs",
    }));
    registry.register(Box::new(StubTool {
        name: "lsp_diagnostics",
        result: "src/lib.rs: 1 diagnostic(s)\n  error 9:3  type mismatch",
    }));
    let club = EditThenRepair {
        hop: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("repair the type error")];
    let (events, _rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(3),
        &events,
    )
    .unwrap();
    assert_eq!(answer, "repair evidence received");
    assert_eq!(club.hop.load(Ordering::SeqCst), 2);
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::Tool)
            .count(),
        1,
        "diagnostics must enrich the mutation result, not add a tool hop"
    );
}

/// The default budget is the L01 unbounded policy, so this pins the real
/// contract — a transient failure before visible output retries in the same hop
/// and recovers, announced once — rather than a retry *count* that no operator
/// configured.
#[test]
fn run_turn_default_provider_budget_recovers_a_transient_failure_in_hop() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::unset("ANGEL_PROVIDER_RETRIES");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    // Provider transport retries stay inside one hop; pin governors other suite
    // tests may arm so a parallel load cannot steal the recovery into an
    // extra hop / runaway stop (suite flake: 2-hop guard without answering).
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _storm = EnvGuard::set("ANGEL_TOOLCALL_STORM", "0");

    struct FlakyProviderClub {
        calls: AtomicUsize,
    }
    impl Club for FlakyProviderClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "flaky-provider"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err("provider returned empty stdout".to_string());
            }
            on_delta(crate::agent::club::StreamDelta::Content("recovered"));
            Ok(ClubReply::Text("recovered".to_string()))
        }
    }

    let club = FlakyProviderClub {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let (event_tx, event_rx) = mpsc::channel();
    // Headroom: recovery is one hop with an in-hop retry. Some(2) was tight
    // under env-governor bleed and failed the runaway guard without answering.
    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .expect("transport failure should recover inside hop budget");

    assert_eq!(answer, "recovered");
    // Exactly one failure + one recovery call (retries stay in-hop).
    assert_eq!(
        club.calls.load(Ordering::SeqCst),
        2,
        "provider recovery must use one retry, not thrash hops"
    );
    assert!(
        history
            .iter()
            .any(|m| m.role == ChatRole::Assistant && m.content.as_ref() == "recovered")
    );
    let notices: Vec<String> = event_rx
        .try_iter()
        .filter_map(|ev| match ev {
            TurnEvent::Notice(text) => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        notices
            .iter()
            .any(|text| text.contains("provider call failed before output; retrying")),
        "retry notice missing: {notices:?}"
    );
    assert!(
        !history
            .iter()
            .any(|m| m.role == ChatRole::Harness && m.content.contains("reply arrived empty")),
        "a generic transport failure must not inject the empty-reply re-prompt"
    );
    assert!(
        !notices.iter().any(|t| t.contains("runaway guard")),
        "recovery must not trip hop runaway: {notices:?}"
    );
}

/// A routine recoverable outage must not end a productive turn because an
/// implicit count ran out: with no operator setting, four consecutive cuts ride
/// out in the same hop and the fifth attempt answers. The old implicit default
/// of one retry died on the second cut.
#[test]
fn run_turn_default_retries_ride_out_more_cuts_than_the_old_implicit_one() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::unset("ANGEL_PROVIDER_RETRIES");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _storm = EnvGuard::set("ANGEL_TOOLCALL_STORM", "0");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");

    struct CutProvider {
        calls: AtomicUsize,
    }
    impl Club for CutProvider {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "cut-provider"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            if self.calls.fetch_add(1, Ordering::SeqCst) < 4 {
                on_delta(crate::agent::club::StreamDelta::Content("discard-partial"));
                return Err(crate::agent::club::INCOMPLETE_STREAM_ERR.to_string());
            }
            on_delta(crate::agent::club::StreamDelta::Content("recovered"));
            Ok(ClubReply::Text("recovered after four cuts".to_string()))
        }
    }

    let club = CutProvider {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let (event_tx, event_rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .expect("an unconfigured provider budget must ride out a transient outage");
    assert_eq!(answer, "recovered after four cuts");
    assert_eq!(club.calls.load(Ordering::SeqCst), 5);
    assert_eq!(
        event_rx
            .try_iter()
            .filter(|event| matches!(event, TurnEvent::SuppressPartial))
            .count(),
        4,
        "each retried attempt must retract its speculative text"
    );
    assert!(
        !history
            .iter()
            .any(|m| m.content.contains("discard-partial")),
        "speculative text must never enter the committed history"
    );
    assert_eq!(
        trajectory::progress_ledger_snapshot()["hop_stream_cuts"],
        json!([{"hop":1,"stream_cut":4}])
    );
}

/// The live report in one test: the transport's bare stall string arrives after
/// the model already streamed partial prose. That error used to be treated as
/// non-retryable the moment any text had been emitted, so the hop died on its
/// first stall and the receipt blamed a retry limit that never ran. It is the
/// same incomplete stream as any other missing terminal event and must retry
/// once even under an explicit budget of one.
#[test]
fn run_turn_retries_a_bare_transport_stall_after_partial_prose() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "1");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _storm = EnvGuard::set("ANGEL_TOOLCALL_STORM", "0");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");

    struct StallThenAnswer {
        calls: AtomicUsize,
    }
    impl Club for StallThenAnswer {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "stall-then-answer"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                on_delta(crate::agent::club::StreamDelta::Content(
                    "stalled partial prose",
                ));
                return Err(
                    "stream stalled: server kept the connection alive but sent no data \
                     for 60s (bound: ANGEL_STREAM_STALL_SECS)"
                        .to_string(),
                );
            }
            on_delta(crate::agent::club::StreamDelta::Content("recovered"));
            Ok(ClubReply::Text("recovered".to_string()))
        }
    }

    let club = StallThenAnswer {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let (event_tx, event_rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .expect("a stall after partial prose must retry, not kill the turn");
    assert_eq!(answer, "recovered");
    assert_eq!(club.calls.load(Ordering::SeqCst), 2);
    let notices: Vec<String> = event_rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(text) => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        notices
            .iter()
            .any(|text| text.contains("provider stream incomplete; retrying")),
        "the stall must be announced as an incomplete stream: {notices:?}"
    );
    assert!(
        !history
            .iter()
            .any(|m| m.content.contains("stalled partial prose")),
        "the retracted partial must not be committed"
    );
}

/// An explicit operator setting stays exact: `0` means one attempt, and the
/// failure receipt names that setting instead of an implicit deadline.
#[test]
fn run_turn_explicit_zero_provider_retries_fails_with_the_real_reason() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _teacher = EnvGuard::set("ANGEL_TEACHER_WATCH", "0");
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");

    struct AlwaysTransient {
        calls: AtomicUsize,
    }
    impl Club for AlwaysTransient {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "explicit-zero"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err("transient transport failure".to_string())
        }
    }

    let club = AlwaysTransient {
        calls: AtomicUsize::new(0),
    };
    let failure = run_turn_observed(
        &club,
        &ToolRegistry::with_defaults(),
        &mut vec![ChatMsg::user("hello")],
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect_err("an explicit zero-retry budget must fail on the first attempt");
    assert_eq!(failure.stop_reason, TurnStopReason::ProviderError);
    assert_eq!(club.calls.load(Ordering::SeqCst), 1);
    assert!(
        failure
            .message
            .contains("provider retry limit ANGEL_PROVIDER_RETRIES=0 exhausted"),
        "{}",
        failure.message
    );
}

/// A cut that carries half a tool call must discard it and replay the hop: the
/// replayed attempt is the only one that dispatches, and it dispatches exactly
/// once. A second dispatch of the same call, or a call assembled from severed
/// arguments, would be a duplicate execution.
#[test]
fn run_turn_partial_tool_call_is_never_dispatched_twice() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "1");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _storm = EnvGuard::set("ANGEL_TOOLCALL_STORM", "0");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");

    struct CutToolCall {
        calls: AtomicUsize,
    }
    impl Club for CutToolCall {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "cut-tool-call"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            match self.calls.fetch_add(1, Ordering::SeqCst) {
                // The transport's discard error: prose plus a half-assembled
                // call that must never reach the registry.
                0 => {
                    on_delta(crate::agent::club::StreamDelta::Content("discard-partial"));
                    Err(format!(
                        "{}; incomplete tool call discarded",
                        crate::agent::club::INCOMPLETE_STREAM_ERR
                    ))
                }
                1 => Ok(ClubReply::Calls(vec![tc(
                    "reverse",
                    json!({"text":"fixture"}),
                )])),
                _ => Ok(ClubReply::Text("complete".into())),
            }
        }
    }

    let club = CutToolCall {
        calls: AtomicUsize::new(0),
    };
    let root = scratch("partial_tool_call_not_twice");
    let mut registry = ToolRegistry::with_defaults();
    registry.set_workspace(root.clone());
    let mut history = vec![ChatMsg::user("Reverse the fixture text")];
    let (event_tx, event_rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .expect("a discarded partial call must not fail the turn");
    assert_eq!(answer, "complete");
    assert_eq!(club.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        event_rx
            .try_iter()
            .filter(|event| matches!(event, TurnEvent::ToolCall { .. }))
            .count(),
        1,
        "the replayed call must dispatch exactly once"
    );
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::Tool)
            .count(),
        1,
        "one executed call yields exactly one tool result"
    );
    assert!(
        !history
            .iter()
            .any(|message| message.content.contains("discard-partial")),
        "the cut attempt's speculative text must not be committed"
    );
    std::fs::remove_dir_all(root).unwrap();
}

/// A permanent account/configuration failure outranks the recoverable-stream
/// spelling. A wrapped error that carries both a rejected credential and stall
/// context must stay actionable instead of looping on the stall word alone.
#[test]
fn run_turn_permanent_error_with_stall_context_is_not_retried() {
    let _guard = crate::tests::env_lock();
    // Non-zero so the assertion observes attempt counts, not a hang.
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "3");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _teacher = EnvGuard::set("ANGEL_TEACHER_WATCH", "0");
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");

    const MIXED: &str = "HTTP 401 Unauthorized: invalid api key; upstream reported \
                         stream stalled: server kept the connection alive but sent no data \
                         for 60s (bound: ANGEL_STREAM_STALL_SECS)";
    // The disposition itself: recoverable text must not launder a permanent error.
    assert!(
        !retryable_provider_failure(MIXED, false, true),
        "a permanent error may not be retried even in the incomplete-stream class"
    );
    assert!(retryable_provider_failure(
        "stream stalled: server kept the connection alive but sent no data for 60s",
        true,
        true
    ));

    struct MixedFailure {
        calls: AtomicUsize,
    }
    impl Club for MixedFailure {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "mixed-failure"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(MIXED.to_string())
        }
    }

    let club = MixedFailure {
        calls: AtomicUsize::new(0),
    };
    let failure = run_turn_observed(
        &club,
        &ToolRegistry::with_defaults(),
        &mut vec![ChatMsg::user("hello")],
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect_err("a permanent provider error must surface immediately");
    assert_eq!(failure.stop_reason, TurnStopReason::ProviderError);
    assert_eq!(
        club.calls.load(Ordering::SeqCst),
        1,
        "the permanent error must outrank the recoverable spelling"
    );
    assert!(
        failure.message.contains("permanent provider error"),
        "{}",
        failure.message
    );
}

/// A cut on a later hop must not cost the turn the tool work already committed:
/// the replay re-sends the identical history (same committed tool result) and the
/// completed call executes exactly once.
#[test]
fn run_turn_stream_cut_replay_preserves_completed_tool_work() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "1");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _storm = EnvGuard::set("ANGEL_TOOLCALL_STORM", "0");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");

    struct CutAfterTool {
        calls: AtomicUsize,
        requests: Mutex<Vec<Value>>,
    }
    impl Club for CutAfterTool {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "cut-after-tool"
        }
        fn chat_streaming(
            &self,
            messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.requests
                .lock()
                .unwrap()
                .push(json!(crate::agent::club::messages_to_json(messages, true)));
            match self.calls.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(ClubReply::Calls(vec![tc(
                    "reverse",
                    json!({"text":"kept"}),
                )])),
                1 => {
                    on_delta(crate::agent::club::StreamDelta::Content("cut mid-answer"));
                    Err(format!(
                        "{}: stream read error",
                        crate::agent::club::INCOMPLETE_STREAM_ERR
                    ))
                }
                _ => {
                    on_delta(crate::agent::club::StreamDelta::Content("final"));
                    Ok(ClubReply::Text("final".into()))
                }
            }
        }
    }

    let club = CutAfterTool {
        calls: AtomicUsize::new(0),
        requests: Mutex::new(Vec::new()),
    };
    let root = scratch("cut_after_tool");
    let mut registry = ToolRegistry::with_defaults();
    registry.set_workspace(root.clone());
    let mut history = vec![ChatMsg::user("Reverse the fixture text, then report")];
    let (event_tx, event_rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .expect("a cut after committed tool work must recover");
    assert_eq!(answer, "final");
    assert_eq!(club.calls.load(Ordering::SeqCst), 3);
    let tool_results: Vec<&str> = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .map(|message| message.content.as_ref())
        .collect();
    assert_eq!(tool_results.len(), 1, "the completed call ran exactly once");
    assert_eq!(
        tool_results[0], "tpek",
        "the committed tool result is the executed reverse output"
    );
    assert_eq!(
        event_rx
            .try_iter()
            .filter(|event| matches!(event, TurnEvent::ToolCall { .. }))
            .count(),
        1,
        "the replayed hop must not dispatch the call again"
    );
    let requests = club.requests.lock().unwrap();
    assert_eq!(
        requests[1], requests[2],
        "the replay must resend the identical history, tool result included"
    );
    assert!(
        !history
            .iter()
            .any(|message| message.content.contains("cut mid-answer")),
        "the cut attempt's speculative text must not be committed"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn run_turn_cancel_during_provider_backoff_does_not_open_another_request() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "2");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "500");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    struct FailingProvider {
        calls: AtomicUsize,
    }
    impl Club for FailingProvider {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "cancelled-backoff"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err("transient transport failure".to_string())
        }
    }

    let club = FailingProvider {
        calls: AtomicUsize::new(0),
    };
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let cancel_after_notice = std::sync::Arc::clone(&cancel);
    let (event_tx, event_rx) = mpsc::channel();
    let canceller = std::thread::spawn(move || {
        while let Ok(event) = event_rx.recv() {
            if matches!(
                event,
                TurnEvent::Notice(ref text) if text.contains("provider call failed before output; retrying")
            ) {
                cancel_after_notice.store(true, Ordering::Release);
                return;
            }
        }
    });
    let mut history = vec![ChatMsg::user("hello")];

    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        cancel.as_ref(),
        Some(4),
        &event_tx,
    )
    .expect("cancellation during retry backoff is a soft interrupt");
    drop(event_tx);
    canceller.join().expect("canceller thread");

    assert!(answer.contains("interrupted"), "got: {answer}");
    assert_eq!(
        club.calls.load(Ordering::SeqCst),
        1,
        "Esc during backoff must not start another provider request"
    );
}

#[test]
fn run_turn_does_not_retry_permanent_provider_failures() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "3");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");

    struct CountingFailure {
        calls: AtomicUsize,
        error: &'static str,
    }
    impl Club for CountingFailure {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "status-failure"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(self.error.to_string())
        }
    }

    for error in [
        "HTTP 400: malformed messages",
        "HTTP 401: Unauthorized",
        "HTTP 402: payment required",
        "HTTP 403: invalid_api_key",
        "HTTP 404: model not found",
        "HTTP 406: unacceptable response format",
        "HTTP 415: unsupported media type",
        "HTTP 422: unsupported parameter",
        "HTTP 424: failed dependency",
        "HTTP 429: weekly quota exhausted",
    ] {
        assert!(is_permanent_provider_error(error), "{error}");
        let club = CountingFailure {
            calls: AtomicUsize::new(0),
            error,
        };
        let failure = run_turn_observed(
            &club,
            &ToolRegistry::new(),
            &mut vec![ChatMsg::user("hello")],
            &AtomicBool::new(false),
            Some(4),
            &mpsc::channel::<TurnEvent>().0,
        )
        .expect_err("a permanent provider failure must surface immediately");
        assert_eq!(failure.stop_reason, TurnStopReason::ProviderError);
        assert_eq!(club.calls.load(Ordering::SeqCst), 1, "{error}");
    }
    for transient in [
        "HTTP 408: timeout",
        "HTTP 425: too early",
        "HTTP 429: rate limited",
    ] {
        assert!(!is_permanent_provider_error(transient), "{transient}");
        let club = CountingFailure {
            calls: AtomicUsize::new(0),
            error: transient,
        };
        let failure = run_turn_observed(
            &club,
            &ToolRegistry::new(),
            &mut vec![ChatMsg::user("hello")],
            &AtomicBool::new(false),
            Some(4),
            &mpsc::channel::<TurnEvent>().0,
        )
        .expect_err("a transient HTTP status should spend the bounded retry allowance");
        assert_eq!(failure.stop_reason, TurnStopReason::ProviderError);
        assert_eq!(
            club.calls.load(Ordering::SeqCst),
            4,
            "{transient} should receive the initial call plus three retries"
        );
    }
    for transient in ["HTTP 500: internal error", "connection reset by peer"] {
        assert!(!is_permanent_provider_error(transient), "{transient}");
    }
}

#[test]
fn run_turn_reprompts_once_on_empty_reply_retry() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "2");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    // Empty-reply retries stay inside a single hop; pin governors that other
    // suite tests may leave armed so this assertion is not load-order flaky.
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    // An empty 200 can repeat deterministically on identical bytes, so the
    // retry must not be a pure replay: the harness injects one transient
    // re-prompt, and the club must see it on the retried request.
    struct EmptyThenOkClub {
        calls: AtomicUsize,
        retry_saw_reprompt: AtomicBool,
    }
    impl Club for EmptyThenOkClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "empty-then-ok"
        }
        fn chat_streaming(
            &self,
            messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            match self.calls.fetch_add(1, Ordering::SeqCst) {
                0 => Err("club returned an empty reply (no text and no tool calls)".to_string()),
                1 => Err("club returned an empty reply (no text and no tool calls)".to_string()),
                _ => {
                    self.retry_saw_reprompt.store(
                        messages.iter().any(|m| {
                            m.role == ChatRole::Harness && m.content.contains("reply arrived empty")
                        }),
                        Ordering::SeqCst,
                    );
                    on_delta(crate::agent::club::StreamDelta::Content("recovered"));
                    Ok(ClubReply::Text("recovered".to_string()))
                }
            }
        }
    }

    let club = EmptyThenOkClub {
        calls: AtomicUsize::new(0),
        retry_saw_reprompt: AtomicBool::new(false),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let (event_tx, _event_rx) = mpsc::channel();
    // Headroom: two empty provider failures + one recovery are same-hop retries.
    // Some(2) was tight when other env governors interacted under parallel load.
    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .unwrap();

    assert_eq!(answer, "recovered");
    assert_eq!(
        club.calls.load(Ordering::SeqCst),
        3,
        "exactly two empty failures + one recovery inside one hop"
    );
    assert!(
        club.retry_saw_reprompt.load(Ordering::SeqCst),
        "the retried request must carry the empty-reply re-prompt"
    );
    assert_eq!(
        history
            .iter()
            .filter(|m| m.role == ChatRole::Harness && m.content.contains("reply arrived empty"))
            .count(),
        1,
        "the re-prompt must be injected exactly once across consecutive empty replies"
    );
}

#[test]
fn run_turn_compacts_and_rebuilds_once_after_provider_context_overflow() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "3");
    let _recoveries = EnvGuard::set("ANGEL_CONTEXT_OVERFLOW_RECOVERIES", "1");
    let _budget = EnvGuard::set("ANGEL_CONTEXT_BUDGET_TOKENS", "100000");

    assert!(is_permanent_provider_error(
        "HTTP 413: context_length_exceeded: payload too large"
    ));

    const ACTIVE_TASK: &str = "preserve this exact active objective through forced recovery";
    const ACTIVE_PLAN: &str = "preserve this exact plan through forced recovery";

    struct OverflowOnce {
        calls: AtomicUsize,
    }
    impl Club for OverflowOnce {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok("## Task\n- continue the coding task\n## Files\n- src/lib.rs".into())
        }
        fn label(&self) -> &str {
            "overflow-once"
        }
        fn chat_streaming(
            &self,
            messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(
                    "HTTP 400: request (104239 tokens) exceeds the available context size \
                     (102400 tokens); type=exceed_context_size_error"
                        .into(),
                );
            }
            assert!(
                messages.iter().any(|message| message
                    .content
                    .starts_with(crate::agent::compaction::COMPACTION_NOTE_HEADER)),
                "retry must use rebuilt compacted history"
            );
            assert_eq!(
                messages
                    .iter()
                    .filter(|message| {
                        message.role == ChatRole::User && message.content.as_ref() == ACTIVE_TASK
                    })
                    .count(),
                1,
                "forced recovery must retain the active objective once, in User role"
            );
            assert_eq!(
                messages
                    .iter()
                    .filter(|message| {
                        message.role == ChatRole::Assistant
                            && message.content.starts_with("[current-plan/v1")
                            && message.content.contains(ACTIVE_PLAN)
                    })
                    .count(),
                1,
                "forced recovery must retain current plan state once, in Assistant role"
            );
            on_delta(crate::agent::club::StreamDelta::Content("recovered"));
            Ok(ClubReply::Text("recovered".into()))
        }
    }

    let club = OverflowOnce {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![
        ChatMsg::system("coding harness"),
        ChatMsg::user(ACTIVE_TASK),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "overflow-plan".into(),
            name: "todo".into(),
            args: serde_json::json!({"action":"list"}),
        }]),
        ChatMsg::tool(
            "overflow-plan",
            format!(
                "{}{}",
                crate::agent::tools::plan::TODO_STATE_PREFIX,
                serde_json::json!({
                    "next_id":1,
                    "items":[{"id":1,"text":ACTIVE_PLAN,"done":false}],
                })
            ),
        ),
    ];
    for index in 0..48 {
        history.push(ChatMsg::assistant(format!(
            "work-{index} {}",
            "a".repeat(400)
        )));
    }
    let before = context_tokens(&history, &[]);
    let (event_tx, event_rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &ToolRegistry::new(),
        &mut history,
        &AtomicBool::new(false),
        Some(3),
        &event_tx,
    )
    .expect("overflow recovery should rebuild rather than repeat the same payload");

    assert_eq!(answer, "recovered");
    assert_eq!(club.calls.load(Ordering::SeqCst), 2);
    assert!(context_tokens(&history, &[]) < before);
    let notices = event_rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(text) => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        notices.iter().any(
            |notice| notice.contains("compacted and rebuilt the request")
                && notice.contains("active budget")
        ),
        "overflow recovery notice missing: {notices:?}"
    );
    assert!(
        notices.iter().all(|notice| !notice.contains("retrying 1/")),
        "generic retry must not spend a call on the rejected payload: {notices:?}"
    );
}

#[test]
fn run_turn_retries_after_reasoning_only_then_provider_failure() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "3");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    // Same parallel-suite governor bleed as the transport-retry case.
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    struct ReasoningThenFailClub {
        calls: AtomicUsize,
    }
    impl Club for ReasoningThenFailClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "reasoning-then-fail"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            _cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                on_delta(crate::agent::club::StreamDelta::Reasoning(
                    "partial reasoning",
                ));
                return Err("stream reset after reasoning".to_string());
            }
            on_delta(crate::agent::club::StreamDelta::Content("recovered"));
            Ok(ClubReply::Text("recovered".to_string()))
        }
    }

    let club = ReasoningThenFailClub {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("hello")];
    let (event_tx, event_rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .expect("reasoning-only failure should retry");

    assert_eq!(answer, "recovered");
    assert_eq!(club.calls.load(Ordering::SeqCst), 2);
    let events = event_rx.try_iter().collect::<Vec<_>>();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, TurnEvent::Reasoning(_)))
            .count(),
        1
    );
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::Notice(text) if text.contains("retrying")
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, TurnEvent::SuppressPartial))
    );
}

#[test]
fn run_turn_steered_injects_queued_note_at_the_next_hop_boundary() {
    let _guard = crate::tests::env_lock();
    // Simulates the user typing while the model works: the steer is pushed
    // DURING hop 1 (while "the model" is busy issuing a tool call), so it must
    // be injected at the next hop boundary — after that hop's tool results —
    // and be visible in the request that produces the final answer, without
    // the turn being interrupted.
    struct MidRunSteerClub {
        hops: AtomicUsize,
        steers: Arc<crate::agent::steer::SteerQueue>,
    }
    impl Club for MidRunSteerClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "mid-run-steer"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
                // The user types while this hop is in flight.
                self.steers.push(ChatMsg::user("also check the tests"));
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "c1".into(),
                    name: "reverse".into(),
                    args: serde_json::json!({ "text": "hello" }),
                }]))
            } else {
                let steered = messages.iter().any(|m| {
                    m.role == ChatRole::Harness
                        && m.content.as_ref() == crate::agent::steer::STEER_CONTEXT
                }) && messages.iter().any(|m| {
                    m.role == ChatRole::User && m.content.as_ref() == "also check the tests"
                });
                Ok(ClubReply::Text(format!("steered: {steered}")))
            }
        }
    }

    let steers = Arc::new(crate::agent::steer::SteerQueue::default());
    let club = MidRunSteerClub {
        hops: AtomicUsize::new(0),
        steers: Arc::clone(&steers),
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("reverse hello")];
    let (event_tx, event_rx) = mpsc::channel();
    let answer = run_turn_steered(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &event_tx,
        Some(&steers),
    )
    .unwrap();
    // The model saw the role-separated steer in its next request…
    assert_eq!(answer, "steered: true");
    assert!(steers.is_empty(), "queue is drained once consumed");
    // …the framing and untouched User note sit after the hop's tool result
    // (pairing intact)…
    let context_at = history
        .iter()
        .position(|m| {
            m.role == ChatRole::Harness && m.content.as_ref() == crate::agent::steer::STEER_CONTEXT
        })
        .expect("Harness steer context lands in history");
    let steer_at = history
        .iter()
        .position(|m| m.role == ChatRole::User && m.content.as_ref() == "also check the tests")
        .expect("untouched User steer lands in history");
    let tool_at = history
        .iter()
        .position(|m| m.role == ChatRole::Tool)
        .expect("tool result in history");
    assert!(
        context_at > tool_at && steer_at > context_at,
        "steer context and note must land after the hop's tool results"
    );
    // …and the UI got a delivery notice for the activity trace.
    let mut delivered = false;
    while let Ok(ev) = event_rx.try_recv() {
        if let TurnEvent::Notice(n) = ev {
            delivered |= n.contains("steer delivered");
        }
    }
    assert!(delivered, "delivery notice emitted");
}

#[test]
fn run_turn_does_not_accept_recon_preamble_as_final_answer() {
    let _guard = crate::tests::env_lock();
    struct FalseStartClub {
        hops: AtomicUsize,
    }
    impl Club for FalseStartClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "false-start"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            match self.hops.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(ClubReply::Text(
                    "Starting with recon — I need to check the README before I brief the council."
                        .into(),
                )),
                1 => Ok(ClubReply::Calls(vec![ToolCall {
                    id: "r".into(),
                    name: "reverse".into(),
                    args: serde_json::json!({ "text": "council" }),
                }])),
                _ => Ok(ClubReply::Text("done from evidence".into())),
            }
        }
    }

    let club = FalseStartClub {
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user(
        "check the readme, discuss with council, and synthesize",
    )];
    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();

    assert_eq!(
        answer,
        "Starting with recon — I need to check the README before I brief the council."
    );
}

#[test]
fn run_turn_retries_repeated_recon_preambles_without_erasing_status_prose() {
    let _guard = crate::tests::env_lock();

    struct RepeatedFalseStartClub {
        hops: AtomicUsize,
    }
    impl Club for RepeatedFalseStartClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "repeated-false-start"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            match self.hops.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(ClubReply::Text(
                    "Starting reconnaissance. Let me pull the README first.".into(),
                )),
                1 => Ok(ClubReply::Text(
                    "Starting reconnaissance now — reading the README and briefing council.".into(),
                )),
                2 => Ok(ClubReply::Calls(vec![ToolCall {
                    id: "r".into(),
                    name: "reverse".into(),
                    args: serde_json::json!({ "text": "evidence" }),
                }])),
                _ => Ok(ClubReply::Text("done from evidence".into())),
            }
        }
    }

    let club = RepeatedFalseStartClub {
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user(
        "check the readme, discuss with council, and synthesize",
    )];
    let (tx, _rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &tx,
    )
    .unwrap();

    assert_eq!(
        answer,
        "Starting reconnaissance. Let me pull the README first."
    );
}

#[test]
fn run_turn_refuses_raw_tool_markup_as_final_answer() {
    let _guard = crate::tests::env_lock();
    // The live failure: a text "answer" carrying an unexecuted tool-call
    // wrapper plus invented results. The turn must push back instead of
    // presenting the hallucination to the user.
    struct MarkupClub {
        hops: AtomicUsize,
    }
    impl Club for MarkupClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "markup"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            match self.hops.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(ClubReply::Text(
                    "<SHELL>{invalid json}Status report:\n- everything looks fine".into(),
                )),
                1 => Ok(ClubReply::Calls(vec![ToolCall {
                    id: "r".into(),
                    name: "reverse".into(),
                    args: serde_json::json!({ "text": "status" }),
                }])),
                _ => Ok(ClubReply::Text("real status from evidence".into())),
            }
        }
    }

    let club = MarkupClub {
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("report")];
    let (tx, rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &tx,
    )
    .unwrap();

    assert_eq!(answer, "real status from evidence");
    assert!(
        history
            .iter()
            .any(|m| { m.role == ChatRole::Harness && m.content.contains("no tool was executed") })
    );
    assert!(
        rx.try_iter()
            .any(|ev| matches!(ev, TurnEvent::SuppressPartial))
    );
}

#[test]
fn run_turn_parallel_read_only_batch_preserves_order() {
    let _guard = crate::tests::env_lock();
    // Two read-only tools batched in one turn take the parallel path; results
    // must still land in call order so each tool_call_id pairs correctly.
    struct TwoCalls {
        hops: AtomicUsize,
    }
    impl Club for TwoCalls {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "two"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(ClubReply::Calls(vec![
                    ToolCall {
                        id: "a".into(),
                        name: "reverse".into(),
                        args: serde_json::json!({ "text": "abc" }),
                    },
                    ToolCall {
                        id: "b".into(),
                        name: "reverse".into(),
                        args: serde_json::json!({ "text": "xyz" }),
                    },
                ]))
            } else {
                Ok(ClubReply::Text("done".into()))
            }
        }
    }
    let club = TwoCalls {
        hops: AtomicUsize::new(0),
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("go")];
    let answer = run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert_eq!(answer, "done");
    let tools: Vec<_> = history
        .iter()
        .filter(|m| m.role == ChatRole::Tool)
        .collect();
    assert_eq!(tools.len(), 2, "both parallel results recorded");
    assert_eq!(tools[0].tool_call_id.as_deref(), Some("a"));
    assert_eq!(tools[1].tool_call_id.as_deref(), Some("b"));
    assert!(tools[0].content.contains("cba"), "got {}", tools[0].content);
    assert!(tools[1].content.contains("zyx"), "got {}", tools[1].content);
}

#[test]
fn run_turn_mixed_batch_parallelizes_safe_runs_without_crossing_the_effect_barrier() {
    let _guard = crate::tests::env_lock();

    #[derive(Default)]
    struct ProbeState {
        phase_one_active: AtomicUsize,
        phase_one_max: AtomicUsize,
        phase_one_done: AtomicUsize,
        barrier_done: AtomicBool,
        phase_two_active: AtomicUsize,
        phase_two_max: AtomicUsize,
    }

    struct ProbeRead(Arc<ProbeState>);
    impl Tool for ProbeRead {
        fn name(&self) -> &str {
            "reverse"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().to_string(),
                description: "segmented scheduling probe".to_string(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, args: &Value) -> Result<String, String> {
            let phase = args["phase"].as_u64().unwrap();
            let (active, max) = if phase == 1 {
                (&self.0.phase_one_active, &self.0.phase_one_max)
            } else {
                assert!(
                    self.0.barrier_done.load(Ordering::Acquire),
                    "a later safe segment overtook the effect barrier"
                );
                (&self.0.phase_two_active, &self.0.phase_two_max)
            };
            let now = active.fetch_add(1, Ordering::AcqRel) + 1;
            max.fetch_max(now, Ordering::AcqRel);
            std::thread::sleep(Duration::from_millis(40));
            active.fetch_sub(1, Ordering::AcqRel);
            if phase == 1 {
                self.0.phase_one_done.fetch_add(1, Ordering::AcqRel);
            }
            Ok(format!("phase {phase} read"))
        }
    }

    struct ProbeBarrier(Arc<ProbeState>);
    impl Tool for ProbeBarrier {
        fn name(&self) -> &str {
            "shell"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().to_string(),
                description: "segmented scheduling barrier".to_string(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            assert_eq!(self.0.phase_one_active.load(Ordering::Acquire), 0);
            assert_eq!(self.0.phase_one_done.load(Ordering::Acquire), 2);
            self.0.barrier_done.store(true, Ordering::Release);
            Ok("barrier complete".to_string())
        }
    }

    struct MixedCalls {
        hops: AtomicUsize,
    }
    impl Club for MixedCalls {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "mixed-segment-probe"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) > 0 {
                return Ok(ClubReply::Text("done".to_string()));
            }
            Ok(ClubReply::Calls(
                [
                    ("a", "reverse", 1),
                    ("b", "reverse", 1),
                    ("barrier", "shell", 0),
                    ("c", "reverse", 2),
                    ("d", "reverse", 2),
                ]
                .into_iter()
                .map(|(id, name, phase)| ToolCall {
                    id: id.to_string(),
                    name: name.to_string(),
                    args: serde_json::json!({"phase": phase}),
                })
                .collect(),
            ))
        }
    }

    let state = Arc::new(ProbeState::default());
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ProbeRead(Arc::clone(&state))));
    registry.register(Box::new(ProbeBarrier(Arc::clone(&state))));
    let mut history = vec![ChatMsg::user("run mixed batch")];
    let answer = run_turn(
        &MixedCalls {
            hops: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();

    assert_eq!(answer, "done");
    assert_eq!(state.phase_one_max.load(Ordering::Acquire), 2);
    assert_eq!(state.phase_two_max.load(Ordering::Acquire), 2);
    let ids = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .filter_map(|message| message.tool_call_id.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["a", "b", "barrier", "c", "d"]);
}

#[test]
fn run_turn_anti_spin_stops_repeated_identical_calls() {
    let _guard = crate::tests::env_lock();
    let _operator_cap = EnvGuard::set("ANGEL_SPIN_LIMIT", "8");
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    struct Stuck;
    impl Club for Stuck {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "stuck"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "x".into(),
                name: "reverse".into(),
                args: serde_json::json!({ "text": "x" }),
            }]))
        }
    }
    let mut history = vec![ChatMsg::user("go")];
    // Unbounded hops, no cancel: only the anti-spin guardrail (default limit 8)
    // ends it. max_hops=50 is a safety net so the test fails loud, never hangs.
    let out = run_turn(
        &Stuck,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(50),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert!(out.contains("stopped"), "got: {out}");
}

/// A8: anti-spin identity is canonical JSON — key order cannot dodge the guard.
#[test]
fn anti_spin_signature_is_stable_under_json_key_order() {
    let a = ToolCall {
        id: "1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "offset": 1}),
    };
    let b = ToolCall {
        id: "2".into(),
        name: "read_file".into(),
        // Same fields, different map insertion order in the object literal…
        args: serde_json::json!({"offset": 1, "path": "src/main.rs"}),
    };
    assert_eq!(
        anti_spin_batch_signature(std::slice::from_ref(&a)),
        anti_spin_batch_signature(std::slice::from_ref(&b)),
        "key order must not change anti-spin identity"
    );
    assert_eq!(
        anti_spin_batch_signature(std::slice::from_ref(&a)),
        toolcall_storm_signature(&a),
        "single-call batch signature matches storm identity"
    );
}

/// Hop-loop anti-spin hashes args in place. Key order is stable; megabyte
/// write bodies do not inflate the identity.
#[test]
fn anti_spin_batch_fingerprint_skips_storm_string_join() {
    let a = ToolCall {
        id: "1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "offset": 1}),
    };
    let b = ToolCall {
        id: "2".into(),
        name: "read_file".into(),
        args: serde_json::json!({"offset": 1, "path": "src/main.rs"}),
    };
    assert_eq!(
        anti_spin_batch_fingerprint(std::slice::from_ref(&a)),
        anti_spin_batch_fingerprint(std::slice::from_ref(&b)),
        "key order must not change the hop-loop fingerprint"
    );

    let body_a = "fn main() { /* leaderboard hilbert submit */ }\n".repeat(4_000);
    let body_b = "fn main() { /* other */ }\n".repeat(4_000);
    assert!(body_a.len() > 100_000);
    let write_a = ToolCall {
        id: "w1".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body_a.clone()}),
    };
    let write_a2 = ToolCall {
        id: "w2".into(),
        name: "write_file".into(),
        args: serde_json::json!({"content": body_a, "path": "src/main.rs"}),
    };
    let write_b = ToolCall {
        id: "w3".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body_b}),
    };
    let fp_a = anti_spin_batch_fingerprint(std::slice::from_ref(&write_a));
    assert_eq!(
        fp_a,
        anti_spin_batch_fingerprint(std::slice::from_ref(&write_a2))
    );
    assert_ne!(
        fp_a,
        anti_spin_batch_fingerprint(std::slice::from_ref(&write_b)),
        "distinct write bodies must not collide"
    );
    assert_ne!(
        fp_a,
        anti_spin_batch_fingerprint(&[]),
        "empty batch is a distinct identity"
    );
}

/// Ordinary write hops must not canonicalize megabyte bodies for anti-spin.
/// Distinct payloads still fingerprint differently.
#[test]
fn toolcall_storm_signature_hashes_mutation_payloads() {
    let body_a = "fn main() { /* leaderboard hilbert submit */ }\n".repeat(4_000);
    let body_b = "fn main() { /* other */ }\n".repeat(4_000);
    assert!(body_a.len() > 100_000);
    let write_a = ToolCall {
        id: "1".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body_a}),
    };
    let write_a2 = ToolCall {
        id: "2".into(),
        name: "write_file".into(),
        args: serde_json::json!({"content": body_a, "path": "src/main.rs"}),
    };
    let write_b = ToolCall {
        id: "3".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body_b}),
    };
    let sig_a = toolcall_storm_signature(&write_a);
    let sig_a2 = toolcall_storm_signature(&write_a2);
    let sig_b = toolcall_storm_signature(&write_b);
    assert_eq!(sig_a, sig_a2, "key order must not change hashed identity");
    assert_ne!(sig_a, sig_b, "different payloads must not collide");
    assert!(
        sig_a.len() < 256,
        "storm identity must stay path-sized, got {}",
        sig_a.len()
    );
    assert!(
        !sig_a.contains("fn main"),
        "raw body must not enter the sig"
    );
    assert_eq!(
        anti_spin_batch_signature(std::slice::from_ref(&write_a)),
        sig_a
    );
}

/// 0-1 key objects skip the hop-loop key-sort Vec. Multi-key objects still
/// fingerprint independently of insertion order.
#[test]
fn payload_fingerprint_skips_key_sort_for_tiny_objects() {
    assert!(!payload_object_needs_key_sort(0));
    assert!(!payload_object_needs_key_sort(1));
    assert!(payload_object_needs_key_sort(2));

    let empty = serde_json::json!({});
    let one_a = serde_json::json!({"path": "src/main.rs"});
    let one_b = serde_json::json!({"path": "src/lib.rs"});
    assert_eq!(payload_fingerprint(&empty), payload_fingerprint(&empty));
    assert_eq!(payload_fingerprint(&one_a), payload_fingerprint(&one_a));
    assert_ne!(
        payload_fingerprint(&one_a),
        payload_fingerprint(&one_b),
        "distinct single-key args must not collide"
    );
    assert_ne!(
        payload_fingerprint(&empty),
        payload_fingerprint(&one_a),
        "empty and one-key objects are distinct"
    );

    let two_ab = serde_json::json!({"path": "src/main.rs", "offset": 1});
    let two_ba = serde_json::json!({"offset": 1, "path": "src/main.rs"});
    assert_eq!(
        payload_fingerprint(&two_ab),
        payload_fingerprint(&two_ba),
        "2+ key objects still sort for a stable fingerprint"
    );

    let read = ToolCall {
        id: "1".into(),
        name: "read_file".into(),
        args: one_a,
    };
    assert_eq!(
        anti_spin_batch_fingerprint(std::slice::from_ref(&read)),
        anti_spin_batch_fingerprint(std::slice::from_ref(&read)),
        "single-arg hops stay stable without a key-sort Vec"
    );
}

/// Storm/anti-spin must hash write bodies in place. Display-serializing the
/// JSON string (quotes + escapes) is the leftover megabyte hop-loop tax.
#[test]
fn payload_fingerprint_hashes_string_bodies_in_place() {
    let body = "fn main() { /* leaderboard hilbert submit */ }\n".repeat(4_000);
    assert!(body.len() > 100_000);
    let value = serde_json::Value::String(body.clone());
    let fp = payload_fingerprint(&value);
    assert_eq!(
        fp,
        short_payload_hash(&body),
        "string payloads hash raw bytes, not JSON Display"
    );
    assert_ne!(
        fp,
        short_payload_hash(&value.to_string()),
        "Display form includes quotes; using it would re-allocate the body"
    );
    let write = ToolCall {
        id: "1".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body}),
    };
    let sig = toolcall_storm_signature(&write);
    assert!(
        sig.contains(&fp),
        "storm identity must use the in-place fingerprint, got {sig}"
    );
    assert!(!sig.contains("fn main"), "raw body must not enter the sig");
    let patch = ToolCall {
        id: "2".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({"input": body}),
    };
    assert!(
        toolcall_storm_signature(&patch).contains(&fp),
        "apply_patch input is a payload key and must hash in place"
    );
}

/// A8: product paths that only *prefix* a board marker must still burn first-write
/// and must not count as legal board wait (or anti-spin exemption).
#[test]
fn board_marker_token_boundary_rejects_product_path_launder() {
    let product_reads = [
        (
            "read_file",
            serde_json::json!({"path": "src/living_handoff_parser.rs"}),
        ),
        (
            "read_file",
            serde_json::json!({"path": "crates/board_tip_util/src/lib.rs"}),
        ),
        (
            "read_file",
            serde_json::json!({"path": "docs/prehandoff.md"}),
        ),
        (
            "shell",
            serde_json::json!({"command": "cat src/living_handoff_parser.rs"}),
        ),
        (
            "shell",
            serde_json::json!({"command": "head -20 crates/my-board-tip-helper/mod.rs"}),
        ),
    ];
    for (name, args) in product_reads {
        let call = ToolCall {
            id: "p".into(),
            name: name.into(),
            args: args.clone(),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "product path must not be board wait: {name} {args}"
        );
        assert!(
            burns_first_write_budget(&call),
            "product path must burn first-write: {name} {args}"
        );
        let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(
            anti_spin_counts_batch(mutation, outcome, wait, burns),
            "product recon must still count as spin: {name}"
        );
    }
    // Real board digests stay wait/poll (and skip anti-spin).
    for (name, args) in [
        (
            "read_file",
            serde_json::json!({"path": "LIVING_HANDOFF.md"}),
        ),
        (
            "shell",
            serde_json::json!({"command": "head -40 /tmp/living-handoff.md"}),
        ),
        (
            "read_file",
            serde_json::json!({"path": ".angelX/notes/board.md"}),
        ),
    ] {
        let call = ToolCall {
            id: "b".into(),
            name: name.into(),
            args: args.clone(),
        };
        assert!(
            is_competition_board_state_call(&call),
            "board digest must remain wait: {name} {args}"
        );
        assert!(
            !burns_first_write_budget(&call),
            "board digest: {name} {args}"
        );
        let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(
            !anti_spin_counts_batch(mutation, outcome, wait, burns),
            "pure board wait must not count as spin: {name}"
        );
    }
    // Product mutation paths that embed the marker are still first-write progress.
    assert!(!is_meta_note_mutation_path("src/living_handoff_parser.rs"));
    assert!(!is_meta_note_mutation_path(
        "crates/board_tip_util/src/lib.rs"
    ));
    assert!(is_meta_note_mutation_path("LIVING_HANDOFF.md"));
    assert!(is_meta_note_mutation_path("/tmp/living-handoff.md"));
}

/// A8: pure board wait/poll must not advance anti-spin (first-write × competition).
#[test]
fn anti_spin_does_not_count_pure_competition_board_wait() {
    let board = ToolCall {
        id: "1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "LIVING_HANDOFF.md"}),
    };
    let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&board));
    assert!(!mutation && !outcome && wait && !burns);
    assert!(
        !anti_spin_counts_batch(mutation, outcome, wait, burns),
        "pure board digest must not count as spin thrash"
    );

    let submit = ToolCall {
        id: "2".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "hilbert submit --note cand"}),
    };
    let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&submit));
    assert!(outcome && wait && !burns && !mutation);
    assert!(
        anti_spin_counts_batch(mutation, outcome, wait, burns),
        "repeated submit/score still counts as spin"
    );

    let recon = ToolCall {
        id: "3".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/main.rs"}),
    };
    let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&recon));
    assert!(!mutation && !outcome && !wait && burns);
    assert!(
        anti_spin_counts_batch(mutation, outcome, wait, burns),
        "free-form recon still counts as spin"
    );

    let tip = ToolCall {
        id: "4".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "head -40 /tmp/living-handoff.md"}),
    };
    let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&tip));
    assert!(wait && !burns && !mutation && !outcome);
    assert!(!anti_spin_counts_batch(mutation, outcome, wait, burns));
}

/// Scenario: an agent that keeps re-reading files already in context (churn)
/// trips the no-progress nudge — end-to-end through run_turn, not just
/// classify_hop. Alternating two REAL files keeps anti-spin (consecutive
/// identical) and the error breaker (reads succeed) from firing first.
#[test]
fn yolo_turn_no_progress_nudges_on_reread_churn() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    struct ReReader {
        hop: AtomicUsize,
    }
    impl Club for ReReader {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "rereader"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let n = self.hop.fetch_add(1, Ordering::Relaxed);
            if n >= 14 {
                // Finite regression path: the former YOLO bypass would reach
                // this unsupported answer without ever emitting the nudge.
                return Ok(ClubReply::Text("late answer".into()));
            }
            // Two real, workspace-local files so read_file succeeds.
            let path = if n.is_multiple_of(2) {
                "Cargo.toml"
            } else {
                "README.md"
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("r{n}"),
                name: "read_file".into(),
                args: serde_json::json!({ "path": path }),
            }]))
        }
    }
    let mut history = vec![ChatMsg::user("go")];
    // The club never answers, so the turn ends at the max_hops runaway guard
    // (an Err by design); we assert on `history`, which run_turn fills in place.
    let _ = run_turn(
        &ReReader {
            hop: AtomicUsize::new(0),
        },
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(12),
        &mpsc::channel::<TurnEvent>().0,
    );
    // Reads must have succeeded (else the error breaker, not no-progress, runs).
    let read_ok = history
        .iter()
        .filter(|m| m.role == ChatRole::Tool)
        .all(|m| !m.content.starts_with("tool error:"));
    assert!(
        read_ok,
        "reads errored — test would exercise the error breaker, not churn"
    );
    let nudged = history
        .iter()
        .any(|m| m.role == ChatRole::Harness && m.content.contains("re-reading files you already"));
    assert!(
        !nudged,
        "no artificial no-progress nudge should fire on reads"
    );
}

#[test]
fn yolo_proc_status_polling_does_not_artificially_churn_stop() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _spin = EnvGuard::set("ANGEL_SPIN_LIMIT", "0");
    let _churn_nudge = EnvGuard::set("ANGEL_NOPROGRESS_LIMIT", "0");
    let _churn_stop = EnvGuard::set("ANGEL_NOPROGRESS_STOP", "0");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");

    struct StatusTool {
        polls: Arc<AtomicUsize>,
    }
    impl Tool for StatusTool {
        fn name(&self) -> &str {
            "proc_status"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "test background-process status".into(),
                params: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": {"type": "integer"},
                        "tail_lines": {"type": "integer"}
                    }
                }),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            let poll = self.polls.fetch_add(1, Ordering::Relaxed);
            Ok(format!("running · uptime={}s · log-tail-{poll}", poll + 1))
        }
    }

    struct Poller {
        hops: AtomicUsize,
    }
    impl Club for Poller {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "proc-status-poller"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.hops.fetch_add(1, Ordering::Relaxed);
            if hop >= 8 {
                return Ok(ClubReply::Text("late unsupported completion".into()));
            }
            let first = ToolCall {
                id: format!("status-{hop}-a"),
                name: "proc_status".into(),
                args: serde_json::json!({"id": 42, "tail_lines": hop + 1}),
            };
            if hop.is_multiple_of(2) {
                Ok(ClubReply::Calls(vec![first]))
            } else {
                Ok(ClubReply::Calls(vec![
                    first,
                    ToolCall {
                        id: format!("status-{hop}-b"),
                        name: "proc_status".into(),
                        args: serde_json::json!({"id": 42, "tail_lines": hop + 20}),
                    },
                ]))
            }
        }
    }

    let polls = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(StatusTool {
        polls: Arc::clone(&polls),
    }));
    let club = Poller {
        hops: AtomicUsize::new(0),
    };
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut vec![ChatMsg::user("monitor the existing background job")],
        &AtomicBool::new(false),
        None,
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("turn completes normally without synthetic churn stop");

    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(outcome.answer, "late unsupported completion");
}

#[test]
fn repeated_poll_guard_preserves_distinct_inspection_and_work_progress() {
    let mut guard = RepeatedPollGuard::new(3);
    for fingerprint in 0..80 {
        assert!(!guard.observe(Some(fingerprint), false));
    }
    assert!(!guard.observe(Some(1), false));
    assert!(!guard.observe(Some(1), false));
    assert!(
        !guard.observe(Some(1), true),
        "workspace changes reset polls"
    );
    assert!(!guard.observe(Some(1), false));
    assert!(!guard.observe(Some(1), false));
    assert!(!guard.observe(None, false), "non-poll work resets polls");
    assert!(!guard.observe(Some(1), false));
    assert!(!guard.observe(Some(2), false));
    assert!(!guard.observe(Some(1), false));
    assert!(!guard.observe(Some(2), false));
    assert!(
        guard.observe(Some(1), false),
        "alternating calls still count"
    );
    let mut disabled = RepeatedPollGuard::new(0);
    for _ in 0..80 {
        assert!(!disabled.observe(Some(1), false));
    }
}

#[test]
fn repeated_poll_default_stops_glm_timestamp_treadmill_with_paired_history() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _spin = EnvGuard::set("ANGEL_SPIN_LIMIT", "0");
    let _suppress = EnvGuard::set("ANGEL_POLL_GUARD", "0");
    let _repeat = EnvGuard::unset("ANGEL_POLL_REPEAT_LIMIT");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "full");
    struct TimestampPoll(Arc<AtomicUsize>);
    impl Tool for TimestampPoll {
        fn name(&self) -> &str {
            "shell"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "local fixture; never executes a shell".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            let n = self.0.fetch_add(1, Ordering::Relaxed);
            Ok(format!("PENDING\n12:00:{n:02}"))
        }
    }
    struct AlternatingPoll(AtomicUsize);
    impl Club for AlternatingPoll {
        fn label(&self) -> &str {
            "glm-poll-fixture"
        }
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat(&self, _history: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            let n = self.0.fetch_add(1, Ordering::Relaxed);
            let command = if n.is_multiple_of(2) {
                "tail -c 130 /tmp/benchmark/results/digest_summary.txt; date -u +%H:%M:%S"
            } else {
                "tail -c 130 /tmp/benchmark/results/digest_summary.txt; cat /tmp/benchmark/summary.json 2>/dev/null || echo PENDING; date -u +%H:%M:%S"
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("poll-{n}"),
                name: "shell".into(),
                args: serde_json::json!({"command":command}),
            }]))
        }
    }
    let dispatches = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TimestampPoll(dispatches.clone())));
    let club = AlternatingPoll(AtomicUsize::new(0));
    let mut history = vec![ChatMsg::user("monitor the existing benchmark")];
    let result = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(40),
        &mpsc::channel().0,
    )
    .unwrap();
    assert_eq!(result.stop_reason, TurnStopReason::Spin);
    assert!(result.answer.contains("repeated passive polling"));
    assert_eq!(
        club.0.load(Ordering::Relaxed),
        15,
        "no paid hop after eighth matching poll"
    );
    assert_eq!(dispatches.load(Ordering::Relaxed), 15);
    let calls: Vec<_> = history
        .iter()
        .flat_map(|message| message.tool_calls.iter())
        .collect();
    assert_eq!(calls.len(), 15);
    for call in calls {
        assert_eq!(
            history
                .iter()
                .filter(|message| message.tool_call_id.as_deref() == Some(call.id.as_str()))
                .count(),
            1
        );
    }
}

#[test]
fn default_waiting_policy_dispatches_status_checks_and_explicit_waits() {
    let _guard = crate::tests::env_lock();
    let _poll = EnvGuard::unset("ANGEL_POLL_GUARD");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "full");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    struct WaitProbe(&'static str, Arc<AtomicUsize>);
    impl Tool for WaitProbe {
        fn name(&self) -> &str {
            self.0
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.0.into(),
                description: "Records dispatch without launching a process".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            let observation = self.1.fetch_add(1, Ordering::SeqCst);
            Ok(format!("experiment observation {observation}"))
        }
    }
    struct WaitingClub(AtomicUsize);
    impl Club for WaitingClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            unreachable!()
        }
        fn label(&self) -> &str {
            "experiment-waiting-fixture"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.0.fetch_add(1, Ordering::SeqCst);
            let (name, args) = match hop {
                0..=2 => ("proc_status", serde_json::json!({"id":42})),
                3 => ("shell", serde_json::json!({"command":"sleep 240"})),
                _ => return Ok(ClubReply::Text("done".into())),
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("wait-{hop}"),
                name: name.into(),
                args,
            }]))
        }
    }
    let polls = Arc::new(AtomicUsize::new(0));
    let waits = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(WaitProbe("proc_status", Arc::clone(&polls))));
    registry.register(Box::new(WaitProbe("shell", Arc::clone(&waits))));
    let mut history = vec![ChatMsg::user(
        "Monitor the current experiment and wait as needed.",
    )];
    let answer = run_turn(
        &WaitingClub(AtomicUsize::new(0)),
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert_eq!(answer, "done");
    assert_eq!(polls.load(Ordering::SeqCst), 3);
    assert_eq!(waits.load(Ordering::SeqCst), 1);
    assert!(history.iter().all(|message| {
        !message
            .content
            .contains("passive status/sleep call not started")
    }));
}

#[test]
fn actionable_turn_suppresses_repeat_proc_status_until_work_advances() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _poll_guard = EnvGuard::set("ANGEL_POLL_GUARD", "1");
    let _poll_limit = EnvGuard::set("ANGEL_POLL_ONLY_LIMIT", "1");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");

    struct CountingTool {
        name: &'static str,
        calls: Arc<AtomicUsize>,
    }
    impl Tool for CountingTool {
        fn name(&self) -> &str {
            self.name
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name.into(),
                description: "poll-guard test tool".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(if self.name == "proc_status" {
                "[42] benchmark — running\nstill running — snapshot only".into()
            } else {
                "candidate updated".into()
            })
        }
    }

    struct PollThenWork {
        hop: AtomicUsize,
    }
    impl Club for PollThenWork {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "poll-then-work"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.hop.fetch_add(1, Ordering::Relaxed);
            if hop == 4 {
                return Ok(ClubReply::Calls(vec![
                    ToolCall {
                        id: "sleep-4".into(),
                        name: "shell".into(),
                        args: serde_json::json!({
                            "command":"sleep 240; tail -1 /tmp/recon-progress.txt"
                        }),
                    },
                    ToolCall {
                        id: "edit-4".into(),
                        name: "write_file".into(),
                        args: serde_json::json!({
                            "path":"src/candidate.rs","content":"improved again"
                        }),
                    },
                ]));
            }
            let call = match hop {
                0 | 3 => ToolCall {
                    id: format!("poll-{hop}"),
                    name: "proc_status".into(),
                    args: serde_json::json!({"id":42}),
                },
                1 => ToolCall {
                    id: "progress-tail-1".into(),
                    name: "shell".into(),
                    args: serde_json::json!({
                        "command":"tail -1 /tmp/recon-progress.txt"
                    }),
                },
                2 => ToolCall {
                    id: "edit-2".into(),
                    name: "write_file".into(),
                    args: serde_json::json!({"path":"src/candidate.rs","content":"improved"}),
                },
                _ => return Ok(ClubReply::Text("done".into())),
            };
            Ok(ClubReply::Calls(vec![call]))
        }
    }

    let polls = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingTool {
        name: "proc_status",
        calls: Arc::clone(&polls),
    }));
    registry.register(Box::new(CountingTool {
        name: "write_file",
        calls: Arc::clone(&writes),
    }));
    let mut history = vec![ChatMsg::user(
        "please implement the candidate fix while the benchmark runs",
    )];
    let outcome = run_turn_observed(
        &PollThenWork {
            hop: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("poll guard should redirect into useful work");

    assert_eq!(outcome.answer, "done");
    assert_eq!(
        polls.load(Ordering::Relaxed),
        2,
        "the progress-file poll is skipped and proc_status is re-enabled after work"
    );
    assert_eq!(
        writes.load(Ordering::Relaxed),
        2,
        "productive sibling executes while the long sleep is skipped"
    );
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Harness && message.content.contains("PASSIVE WAIT BLOCKED")
    }));
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Tool
            && message
                .content
                .contains("passive status/sleep call not started")
    }));
    assert_eq!(
        history
            .iter()
            .filter(|message| {
                message.role == ChatRole::Tool
                    && message
                        .content
                        .contains("passive status/sleep call not started")
            })
            .count(),
        2,
        "both the repeated progress poll and long sleep are suppressed"
    );
}

#[test]
fn varied_passive_poll_treadmill_is_bounded_by_shared_spin_identity() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _poll_guard = EnvGuard::set("ANGEL_POLL_GUARD", "1");
    let _poll_limit = EnvGuard::set("ANGEL_POLL_ONLY_LIMIT", "1");
    let _spin = EnvGuard::set("ANGEL_SPIN_LIMIT", "4");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");

    struct StatusTool {
        polls: Arc<AtomicUsize>,
    }
    impl Tool for StatusTool {
        fn name(&self) -> &str {
            "proc_status"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "treadmill test status tool".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.polls.fetch_add(1, Ordering::Relaxed);
            Ok("still running".into())
        }
    }

    // Every hop emits a DIFFERENT passive batch — alternating status polls
    // (varying args) and long sleeps (varying durations). Per-batch hashing
    // let this run forever; the shared sentinel must accumulate to the stop.
    struct VariedTreadmill {
        hop: AtomicUsize,
    }
    impl Club for VariedTreadmill {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "varied-treadmill"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.hop.fetch_add(1, Ordering::Relaxed);
            let call = if hop.is_multiple_of(2) {
                ToolCall {
                    id: format!("poll-{hop}"),
                    name: "proc_status".into(),
                    args: serde_json::json!({"id": 42, "tail_lines": hop + 1}),
                }
            } else {
                ToolCall {
                    id: format!("sleep-{hop}"),
                    name: "shell".into(),
                    args: serde_json::json!({"command": format!("sleep {}", 300 + hop)}),
                }
            };
            Ok(ClubReply::Calls(vec![call]))
        }
    }

    let polls = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(StatusTool {
        polls: Arc::clone(&polls),
    }));
    let mut history = vec![ChatMsg::user("keep an eye on the benchmark for me")];
    let outcome = run_turn_observed(
        &VariedTreadmill {
            hop: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(40),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("treadmill must stop cleanly, not exhaust hops");

    assert_eq!(outcome.stop_reason, TurnStopReason::Spin);
    assert!(
        outcome.answer.contains("passive wait"),
        "stop note names the treadmill: {}",
        outcome.answer
    );
    assert!(
        polls.load(Ordering::Relaxed) <= 1,
        "only the within-budget first poll may execute"
    );
    // The denial receipts escalate with the streak so the model sees the
    // count climbing instead of an identical wall.
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Tool && message.content.contains("passive-wait denial ×2")
    }));
}

#[test]
fn competition_tool_prose_does_not_claim_watcher_ownership_or_suppress_status() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _poll_guard = EnvGuard::set("ANGEL_POLL_GUARD", "1");
    let _poll_limit = EnvGuard::set("ANGEL_POLL_ONLY_LIMIT", "1");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");

    struct CompetitionTool {
        name: &'static str,
        calls: Arc<AtomicUsize>,
    }
    impl Tool for CompetitionTool {
        fn name(&self) -> &str {
            self.name
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name.into(),
                description: "competition poll-guard test tool".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, args: &Value) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.name == "shell"
                && crate::agent::tools::shell::shell_command_arg(args)
                    .is_some_and(|command| command.contains("submit"))
            {
                Ok("Submission queued 11111111-2222-4333-8444-555555555555".into())
            } else {
                Ok("candidate updated".into())
            }
        }
    }

    struct SubmitThenPoll {
        hop: AtomicUsize,
    }
    impl Club for SubmitThenPoll {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "submit-then-poll"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.hop.fetch_add(1, Ordering::Relaxed);
            let call = match hop {
                0 => ToolCall {
                    id: "submit".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"command":"yukon submit --note candidate"}),
                },
                1 => ToolCall {
                    id: "poll".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"command":"yukon submissions"}),
                },
                2 => ToolCall {
                    id: "next-edit".into(),
                    name: "write_file".into(),
                    args: serde_json::json!({"path":"src/next.rs","content":"next"}),
                },
                _ => return Ok(ClubReply::Text("next candidate advancing".into())),
            };
            Ok(ClubReply::Calls(vec![call]))
        }
    }

    let shell_calls = Arc::new(AtomicUsize::new(0));
    let write_calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CompetitionTool {
        name: "shell",
        calls: Arc::clone(&shell_calls),
    }));
    registry.register(Box::new(CompetitionTool {
        name: "write_file",
        calls: Arc::clone(&write_calls),
    }));
    let mut history = vec![ChatMsg::user(
        "run the competition-loop: improve and submit candidates",
    )];
    let outcome = run_turn_observed(
        &SubmitThenPoll {
            hop: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("untrusted receipt prose must not claim watcher-owned status");

    assert_eq!(outcome.answer, "next candidate advancing");
    assert_eq!(
        shell_calls.load(Ordering::Relaxed),
        2,
        "manual status query remains available without a trusted watcher slot"
    );
    assert_eq!(write_calls.load(Ordering::Relaxed), 1);
    assert!(!history.iter().any(|message| {
        message.role == ChatRole::Tool
            && message
                .content
                .contains("passive status/sleep call not started")
    }));
    assert!(!history.iter().any(|message| {
        message.role == ChatRole::Harness && message.content.contains(WATCHER_NOTIFY_MARK)
    }));
}

#[test]
fn passive_sleep_parser_targets_wait_commands_not_prose() {
    let shell = |command: &str| ToolCall {
        id: "sleep".into(),
        name: "shell".into(),
        args: serde_json::json!({"command":command}),
    };
    assert_eq!(
        shell_passive_sleep_secs(&shell("sleep 240; tail -1 progress")),
        Some(240)
    );
    assert_eq!(
        shell_passive_sleep_secs(&shell("check; sleep 2m; tail -1 log")),
        Some(120)
    );
    assert_eq!(
        shell_passive_sleep_secs(&shell("echo sleep is unacceptable")),
        None
    );
    assert_eq!(
        shell_passive_sleep_secs(&shell("sleep $WAIT; tail -1 log")),
        Some(u64::MAX)
    );
    assert!(is_progress_artifact_snapshot_call(&shell(
        "tail -1 /tmp/recon-progress.txt"
    )));
    assert!(is_progress_artifact_snapshot_call(&shell(
        "grep FAIL build.log"
    )));
    assert!(!is_progress_artifact_snapshot_call(&shell(
        "grep progress src/solver.rs"
    )));

    let long_sleep = vec![shell("sleep 240; tail -1 /tmp/recon-progress.txt")];
    let mut active_guard = PassivePollGuard::default();
    assert!(active_guard.should_suppress(&long_sleep, true, None, 1, 2));
    let mut monitoring_guard = PassivePollGuard::default();
    assert!(
        !monitoring_guard.should_suppress(&long_sleep, false, None, 1, 2),
        "dedicated read-only monitoring turns retain operator-requested waiting"
    );
}

#[test]
fn run_turn_suppresses_exact_repeated_inspection_before_next_request() {
    let _guard = crate::tests::env_lock();
    let _handles = crate::tests::TestEnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let _dedup = EnvGuard::set("ANGEL_TOOL_RESULT_DEDUP", "1");
    let _floor = EnvGuard::set("ANGEL_TOOL_RESULT_DEDUP_MIN_BYTES", "1");
    // Inspection/dedup owns its input; the test binary may be invoked outside
    // Cargo's package directory (for example an adjacent pinned qualification).
    let root = scratch("repeated_inspection");
    // Dedup deliberately keeps small outputs when its receipt would cost more
    // than the original. Supply enough real file content to exercise elision.
    let manifest = format!(
        "[package]\nname = \"inspection-fixture\"\nversion = \"0.0.0\"\n{}",
        "# deterministic inspection payload\n".repeat(32)
    );
    std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.clone());
    registry.register(Box::new(crate::agent::tools::file::ReadFileTool {
        root: root.clone(),
    }));

    struct RepeatThenInspect {
        step: AtomicUsize,
        cache_stable: bool,
    }
    impl Club for RepeatThenInspect {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "repeat-inspection"
        }
        fn chat(&self, messages: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let step = self.step.fetch_add(1, Ordering::Relaxed);
            if step < 2 {
                return Ok(ClubReply::Calls(vec![ToolCall {
                    id: format!("read-{step}"),
                    name: "read_file".into(),
                    args: serde_json::json!({"path":"Cargo.toml"}),
                }]));
            }
            let results = messages
                .iter()
                .filter(|message| message.role == ChatRole::Tool)
                .collect::<Vec<_>>();
            assert_eq!(results.len(), 2);
            assert_eq!(
                results[0].content.contains(TOOL_DUPLICATE_MARK),
                !self.cache_stable,
                "only the token-first ablation rewrites an already-sent result"
            );
            assert!(!results[1].content.contains(TOOL_DUPLICATE_MARK));
            assert!(results[1].content.contains("[package]"));
            if self.cache_stable {
                assert_eq!(results[0].content, results[1].content);
            }
            Ok(ClubReply::Text("done".into()))
        }
    }

    for cache_stable in [true, false] {
        let _cache = if cache_stable {
            EnvGuard::unset("ANGEL_CACHE_STABLE")
        } else {
            EnvGuard::set("ANGEL_CACHE_STABLE", "0")
        };
        let mut history = vec![ChatMsg::user("inspect twice")];
        let answer = run_turn(
            &RepeatThenInspect {
                step: AtomicUsize::new(0),
                cache_stable,
            },
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(6),
            &mpsc::channel::<TurnEvent>().0,
        )
        .unwrap();
        assert_eq!(answer, "done");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn first_write_is_explicit_operator_opt_in() {
    let _guard = crate::tests::env_lock();
    let _unset = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _rejections = EnvGuard::unset("ANGEL_FIRST_WRITE_REJECTIONS");

    assert_eq!(configured_first_write_limit(), 0);
    assert_eq!(configured_first_write_rejection_limit(false), 0);
    assert_eq!(configured_first_write_rejection_limit(true), 0);
    assert_eq!(configured_turn_deadline_secs_for(true), 0);

    let _explicit = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "7");
    assert_eq!(configured_first_write_limit(), 7);
}

#[test]
fn competition_goal_and_fire_imperatives_keep_cadence_armed() {
    let _guard = crate::tests::env_lock();
    let _unset = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _comp = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _gpu = EnvGuard::unset("ANGEL_GPU_COMP_LOCAL_MOA");
    let _pace = EnvGuard::set("ANGEL_TASK_PACE", "rapid");
    let _resolved_pace = EnvGuard::unset("ANGEL_TASK_PACE_RESOLVED");

    // 2026-08-15 flock incident: a mid-run operator steer with no launch
    // phrase used to DISARM the cadence — the message demanding action was
    // exactly what reverted the turn to ungoverned conversation mode. The
    // standing goal is durable operator intent and keeps it armed.
    let goal_ctx = "[harness turn context]\n\
        [goal — standing objective; keep every action aligned to it]\n\
        objective: \"place winning submission on the board relentlessly\"\n\
        [/goal]";
    let steered = vec![
        ChatMsg::user("run the competition-loop"),
        ChatMsg::harness(goal_ctx),
        ChatMsg::assistant("working"),
        ChatMsg::user("you are wasting time on the track"),
    ];
    assert_eq!(competition_mode_trigger(&steered), Some("competition-goal"));
    assert!(competition_mode_active(&steered));

    // A bare operator fire imperative arms the cadence with no goal at all.
    let fire = vec![ChatMsg::user("stop overthinking and FIRE NOW")];
    assert_eq!(competition_mode_trigger(&fire), Some("fire now"));
    let mixed = vec![ChatMsg::user("Please run the Competition-Loop now")];
    assert_eq!(
        competition_mode_trigger(&mixed),
        Some("competition-loop"),
        "mixed-case launch phrases still match without a lowercase copy"
    );

    // A non-competitive goal must not arm anything: the anti-sticky contract
    // for ordinary conversation stays intact.
    let calm_goal = "[harness turn context]\n\
        [goal — standing objective; keep every action aligned to it]\n\
        objective: \"refactor the parser for clarity\"\n\
        [/goal]";
    let calm = vec![
        ChatMsg::user("run the competition-loop"),
        ChatMsg::harness(calm_goal),
        ChatMsg::assistant("done"),
        ChatMsg::user("hey pro, how goes?"),
    ];
    assert_eq!(competition_mode_trigger(&calm), None);
    assert!(!competition_mode_active(&calm));
}

#[test]
fn deep_competition_keeps_context_without_arming_submission_cadence() {
    let _guard = crate::tests::env_lock();
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _comp = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _gpu = EnvGuard::unset("ANGEL_GPU_COMP_LOCAL_MOA");
    let _pace = EnvGuard::set("ANGEL_TASK_PACE", "deep");
    let _resolved_pace = EnvGuard::unset("ANGEL_TASK_PACE_RESOLVED");
    let history = vec![ChatMsg::user(
        "run the competition-loop as a slow-burn major solve; do not submit",
    )];

    assert!(competition_mode_active(&history));
    let pace = configured_task_pace(&history);
    assert_eq!(pace, TaskPace::Deep);
    assert_eq!(configured_first_write_limit(), 0);
    assert!(!first_write_nudge(true, pace).contains("submit the current"));
    assert!(first_write_nudge(true, pace).contains("never implied"));
    let (posture, card) = competition_posture(pace);
    assert!(posture.contains("PACE — DEEP"));
    // Operator law 2026-09-11: deep pace still ships a gate-passing candidate.
    assert!(posture.contains("are never instructions to submit"));
    assert!(posture.contains("passes the local gate IS"));
    assert!(posture.contains("Never wrap builds, engine boots, or benchmarks in `timeout`"));
    assert!(card.contains("never submit solely"));
    assert!(card.contains("never sit on a candidate that passed the local gate"));
    assert!(passive_poll_nudge(pace).contains("not a request"));
}

#[test]
fn deferred_action_catches_long_plan_without_tools() {
    let plan = "I'll analyze the kernel first, then investigate the hot path, \
        research alternatives, and figure out the best approach before submitting.";
    assert!(looks_like_deferred_action_only(plan));
    assert!(
        looks_like_deferred_action_only(
            "I'LL ANALYZE the kernel first, then INVESTIGATE the hot path."
        ),
        "mixed-case intent/work still counts as a deferred plan"
    );
    assert!(
        looks_like_deferred_action_only("LET ME inspect the source and look at the hot path."),
        "mixed-case LET ME inspect still counts"
    );
    assert!(!looks_like_deferred_action_only(
        "Submitted candidate abc123. Score pending. Done for this hop."
    ));
    assert!(
        !looks_like_deferred_action_only("SUBMITTED candidate abc123. I CHECKED. DONE."),
        "mixed-case closure still ends the deferred-plan match"
    );
}

#[test]
fn competition_outcome_call_detects_submit_shell() {
    let submit = ToolCall {
        id: "1".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "hilbert submit --note 'cand'"}),
    };
    let read = ToolCall {
        id: "2".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/main.rs"}),
    };
    let yukon = ToolCall {
        id: "3".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "yukon submissions eigenlabs/flock-challenge"}),
    };
    assert!(is_competition_outcome_call(&submit));
    assert!(is_competition_outcome_call(&yukon));
    assert!(!is_competition_outcome_call(&read));
}

#[test]
fn meta_note_mutations_are_not_first_write_progress() {
    let handoff = ToolCall {
        id: "1".into(),
        name: "write_file".into(),
        args: serde_json::json!({
            "path": "LIVING_HANDOFF.md",
            "content": "tip: still validating\n"
        }),
    };
    let notes = ToolCall {
        id: "2".into(),
        name: "str_replace".into(),
        args: serde_json::json!({
            "path": ".angelX/notes/board.md",
            "old": "a",
            "new": "b"
        }),
    };
    let core = ToolCall {
        id: "3".into(),
        name: "str_replace".into(),
        args: serde_json::json!({
            "path": "src/lib.rs",
            "old": "a",
            "new": "b"
        }),
    };
    // Live A8 receipt (Turbo iter 77): after two post-budget recon denials,
    // Grok wrote a placeholder below the candidate's nested `.scratch` and the
    // direct write incorrectly disarmed first-write. The owner `.scratch` that
    // contains a real worktree must remain legal; only the nested artifact is
    // bookkeeping.
    let scratch_probe = ToolCall {
        id: "4".into(),
        name: "write_file".into(),
        args: serde_json::json!({
            "path": "/repo/.scratch/worktrees/crown/.scratch/iter36-lincheck-probe.rs",
            "content": "fn main() {}"
        }),
    };
    let scratch_worktree_product = ToolCall {
        id: "5".into(),
        name: "write_file".into(),
        args: serde_json::json!({
            "path": "/repo/.scratch/worktrees/crown/crates/prover/src/lib.rs",
            "content": "pub fn candidate() {}"
        }),
    };
    assert!(is_meta_note_mutation_path("LIVING_HANDOFF.md"));
    assert!(!is_first_write_progress_call(&handoff));
    assert!(!is_first_write_progress_call(&notes));
    assert!(is_first_write_progress_call(&core));
    assert_eq!(mutation_arg_path(&core.args), Some("src/lib.rs"));
    assert_eq!(
        mutation_arg_path(&notes.args),
        Some(".angelX/notes/board.md")
    );
    assert!(mutation_call_has_product_path(&core));
    assert!(!mutation_call_has_product_path(&notes));
    assert!(!mutation_call_has_product_path(&handoff));
    assert!(is_meta_note_mutation_path(
        "/repo/.scratch/worktrees/crown/.scratch/iter36-lincheck-probe.rs"
    ));
    assert!(!is_first_write_progress_call(&scratch_probe));
    assert!(burns_first_write_budget(&scratch_probe));
    assert!(!is_meta_note_mutation_path(
        "/repo/.scratch/worktrees/crown/crates/prover/src/lib.rs"
    ));
    assert!(is_first_write_progress_call(&scratch_worktree_product));
    let pathless = ToolCall {
        id: "0".into(),
        name: "integrate".into(),
        args: serde_json::json!({}),
    };
    assert!(
        mutation_call_has_product_path(&pathless),
        "pathless mutation stays historical first-write yes"
    );
    assert_eq!(mutation_arg_path(&pathless.args), None);
    // A4: basename markers must not substring-match product files.
    for product in [
        "src/footnotes.md",
        "docs/endnotes.md",
        "keynote.md",
        "mynotes.md",
        "host-slot.json",
        "dataslot.json",
        "submissions.jsonl",
    ] {
        assert!(
            !is_meta_note_mutation_path(product),
            "product path must not be meta bookkeeping: {product}"
        );
        let call = ToolCall {
            id: "p".into(),
            name: "write_file".into(),
            args: serde_json::json!({"path": product, "content": "x"}),
        };
        assert!(
            is_first_write_progress_call(&call),
            "product mutation must count as first-write progress: {product}"
        );
    }
    // Relative angel notes store (no leading slash) is still bookkeeping.
    for meta in [
        ".angelX/notes/session.txt",
        ".angelX/handoff/tip.txt",
        "workspace/.angelX/notes/run.md",
    ] {
        assert!(
            is_meta_note_mutation_path(meta),
            "angel notes/handoff store must be meta: {meta}"
        );
    }
    // Exact board basenames stay meta.
    for meta in [
        "notes.md",
        "note.md",
        "slot.json",
        "submissions.json",
        "handoff.md",
    ] {
        assert!(is_meta_note_mutation_path(meta), "board basename: {meta}");
    }
}

/// First-write classify matches product vs meta paths in place.
/// Mixed-case board names stay bookkeeping; `src\\kernel.cu` still counts.
#[test]
fn mutation_call_has_product_path_matches_in_place() {
    assert_eq!(
        mutation_arg_path(&serde_json::json!({"path": "  src/kernel.cu  "})),
        Some("src/kernel.cu")
    );
    assert_eq!(
        mutation_arg_path(&serde_json::json!({"file_path": "src\\kernel.cu"})),
        Some("src\\kernel.cu")
    );
    assert_eq!(mutation_arg_path(&serde_json::json!({"path": ""})), None);
    assert_eq!(mutation_arg_path(&serde_json::json!({})), None);

    let product = ToolCall {
        id: "1".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src\\kernel.cu", "content": "x"}),
    };
    let board = ToolCall {
        id: "2".into(),
        name: "str_replace".into(),
        args: serde_json::json!({
            "path": "NOTES.md",
            "old": "a",
            "new": "b"
        }),
    };
    let pathless = ToolCall {
        id: "3".into(),
        name: "integrate".into(),
        args: serde_json::json!({}),
    };
    let mixed = ToolCall {
        id: "4".into(),
        name: "multi_edit".into(),
        args: serde_json::json!({
            "edits": [
                {"path": "LIVING_HANDOFF.md", "old": "a", "new": "b"},
                {"path": "src/lib.rs", "old": "a", "new": "b"}
            ]
        }),
    };
    assert!(mutation_call_has_product_path(&product));
    assert!(is_first_write_progress_call(&product));
    assert!(!mutation_call_has_product_path(&board));
    assert!(!is_first_write_progress_call(&board));
    assert!(
        mutation_call_has_product_path(&pathless),
        "pathless mutation stays historical first-write yes"
    );
    assert!(
        mutation_call_has_product_path(&mixed),
        "multi_edit with one product path is progress"
    );
    assert_eq!(mutation_arg_path(&product.args), Some("src\\kernel.cu"));
}

#[test]
fn competition_wait_and_board_state_do_not_burn_first_write_budget() {
    let status = ToolCall {
        id: "1".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "hilbert submissions 8806afb8-8dfa"}),
    };
    let handoff = ToolCall {
        id: "2".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "LIVING_HANDOFF.md"}),
    };
    let tip = ToolCall {
        id: "3".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "head -40 /tmp/living-handoff.md"}),
    };
    let thrash = ToolCall {
        id: "4".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "find . -name '*.rs' | head"}),
    };
    let src = ToolCall {
        id: "5".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/main.rs"}),
    };
    assert!(is_competition_outcome_call(&status));
    assert!(is_competition_wait_or_progress_call(&status));
    assert!(!burns_first_write_budget(&status));
    assert!(is_competition_board_state_call(&handoff));
    assert!(!burns_first_write_budget(&handoff));
    assert!(is_competition_board_state_call(&tip));
    assert!(!burns_first_write_budget(&tip));
    // Free-form recon still burns the budget.
    assert!(burns_first_write_budget(&thrash));
    assert!(burns_first_write_budget(&src));
    // Wide find that only *mentions* living handoff still burns.
    let sneak = ToolCall {
        id: "6".into(),
        name: "shell".into(),
        args: serde_json::json!({
            "command": "find / -name living_handoff.md 2>/dev/null"
        }),
    };
    assert!(
        burns_first_write_budget(&sneak),
        "inventory thrash must not hide behind board markers"
    );
}

/// A7: single-pass hop flags match the prior multi-`any` classification.
#[test]
fn hop_budget_flags_match_per_call_predicates() {
    let calls = vec![
        ToolCall {
            id: "1".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path": "src/main.rs"}),
        },
        ToolCall {
            id: "2".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": "cat LIVING_HANDOFF.md"}),
        },
        ToolCall {
            id: "3".into(),
            name: "str_replace".into(),
            args: serde_json::json!({
                "path": "src/lib.rs",
                "old": "a",
                "new": "b"
            }),
        },
        ToolCall {
            id: "4".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": "hilbert submit --note x"}),
        },
    ];
    let (mutation, outcome, wait, burns) = hop_budget_flags(&calls);
    assert_eq!(mutation, calls.iter().any(is_first_write_progress_call));
    assert_eq!(outcome, calls.iter().any(is_competition_outcome_call));
    assert_eq!(wait, calls.iter().any(is_competition_wait_or_progress_call));
    assert_eq!(burns, calls.iter().any(burns_first_write_budget));
    assert!(mutation && outcome && wait && burns);
}

/// Default hops skip competition hay. Slot / first-write / competition still
/// classify; board wait keeps anti-spin immunity only on that path.
#[test]
fn hop_budget_flags_for_loop_skips_hay_on_ordinary_hops() {
    assert!(!hop_budget_classify_applied(false, 0, false));
    assert!(hop_budget_classify_applied(true, 0, false));
    assert!(hop_budget_classify_applied(false, 7, false));
    assert!(hop_budget_classify_applied(false, 0, true));

    let board = ToolCall {
        id: "b".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "LIVING_HANDOFF.md"}),
    };
    let full = hop_budget_flags(std::slice::from_ref(&board));
    assert_eq!(
        hop_budget_flags_for_loop(true, std::slice::from_ref(&board)),
        full,
        "armed classify must match hop_budget_flags"
    );
    assert!(!full.0 && !full.1 && full.2 && !full.3);
    assert!(!anti_spin_counts_batch(full.0, full.1, full.2, full.3));

    let ordinary = hop_budget_flags_for_loop(false, std::slice::from_ref(&board));
    assert_eq!(ordinary, (false, false, false, true));
    assert!(
        anti_spin_counts_batch(ordinary.0, ordinary.1, ordinary.2, ordinary.3),
        "ordinary hops count every batch as spin — no board-wait immunity without a slot"
    );

    let body = "fn main() {\n    // leaderboard submissions hilbert submit living_handoff.md\n}\n"
        .repeat(4_000);
    assert!(body.len() > 100_000);
    let write = ToolCall {
        id: "w".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body}),
    };
    assert_eq!(
        hop_budget_flags_for_loop(false, std::slice::from_ref(&write)),
        (false, false, false, true),
        "ordinary hops must not walk write bodies for competition markers"
    );
    assert_eq!(
        hop_budget_flags_for_loop(true, std::slice::from_ref(&write)),
        hop_budget_flags(std::slice::from_ref(&write))
    );
}

/// Ordinary hops must not serialize write/patch bodies to hunt for
/// competition markers. Product text mentioning leaderboard/handoff is
/// not a submit or board wait; hay stays path-sized.
#[test]
fn hop_budget_flags_skip_mutation_payload_scan() {
    assert!(is_competition_payload_key("write_file", "content"));
    assert!(is_competition_payload_key("str_replace", "new"));
    assert!(is_competition_payload_key("apply_patch", "patch"));
    assert!(is_competition_payload_key("apply_patch", "input"));
    assert!(is_competition_payload_key("code_mode", "script"));
    assert!(!is_competition_payload_key("read_file", "path"));
    assert!(!is_competition_payload_key("write_file", "path"));
    assert!(!is_competition_payload_key("shell", "command"));

    let body = "fn main() {\n    // leaderboard submissions hilbert submit living_handoff.md handoff.md\n}\n"
        .repeat(4_000);
    assert!(
        body.len() > 100_000,
        "payload must be large enough to matter"
    );
    let write = ToolCall {
        id: "w".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body}),
    };
    assert!(is_first_write_progress_call(&write));
    assert!(
        !is_competition_outcome_call(&write),
        "write payload must not launder as submit"
    );
    assert!(
        !is_competition_board_state_call(&write),
        "write payload must not launder as board wait"
    );
    assert!(!burns_first_write_budget(&write));
    let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&write));
    assert!(mutation && !outcome && !wait && !burns);
    let (name, hay, is_shell) = competition_call_text(&write);
    assert_eq!(name, "write_file");
    assert!(!is_shell);
    assert!(
        hay.len() < 256,
        "classification hay must stay path-sized, got {}",
        hay.len()
    );
    assert!(hay.contains("src/main.rs"));
    assert!(
        !hay.contains("fn main"),
        "write body must stay out of the hay"
    );

    let patch = format!(
        "*** Update File: LIVING_HANDOFF.md\n@@\n-{}\n+leaderboard submissions\n",
        "x".repeat(50_000)
    );
    let meta = ToolCall {
        id: "p".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({"diff": patch}),
    };
    assert!(!is_first_write_progress_call(&meta));
    assert!(
        is_competition_board_state_call(&meta),
        "meta-only patch path must remain board wait"
    );
    assert!(!burns_first_write_budget(&meta));
    let (_, patch_hay, _) = competition_call_text(&meta);
    assert!(
        patch_hay.len() < 512,
        "apply_patch hay must not include hunks, got {}",
        patch_hay.len()
    );
    assert!(
        ascii_contains_ignore_case(&patch_hay, "living_handoff.md"),
        "meta patch path must remain in the hay: {patch_hay}"
    );
    assert!(!patch_hay.contains(&"x".repeat(32)));
}

/// Classify hops borrow a single remaining path. Mixed-case
/// `LIVING_HANDOFF.md` still classifies as board wait; write bodies stay
/// out of the hay.
#[test]
fn competition_call_text_borrows_single_path() {
    let read = ToolCall {
        id: "1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src\\kernel.cu"}),
    };
    assert_eq!(classify_hay_borrow(&read), Some("src\\kernel.cu"));
    let (name, hay, is_shell) = competition_call_text(&read);
    assert!(std::ptr::eq(name, read.name.as_str()));
    assert!(
        std::ptr::eq(hay.as_ref(), read.args["path"].as_str().unwrap()),
        "classify must borrow the path string"
    );
    assert!(!is_shell);
    assert!(
        burns_first_write_budget(&read),
        "product read still burns first-write"
    );

    let body = "fn main() { /* leaderboard submissions hilbert submit */ }\n".repeat(4_000);
    assert!(body.len() > 100_000);
    let write = ToolCall {
        id: "2".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body}),
    };
    assert_eq!(classify_hay_borrow(&write), Some("src/main.rs"));
    let (_, write_hay, _) = competition_call_text(&write);
    assert!(std::ptr::eq(
        write_hay.as_ref(),
        write.args["path"].as_str().unwrap()
    ));
    assert!(
        !is_competition_outcome_call(&write),
        "write payload must not launder as submit"
    );

    let board = ToolCall {
        id: "3".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "LIVING_HANDOFF.md"}),
    };
    assert_eq!(classify_hay_borrow(&board), Some("LIVING_HANDOFF.md"));
    let (_, board_hay, _) = competition_call_text(&board);
    assert!(std::ptr::eq(
        board_hay.as_ref(),
        board.args["path"].as_str().unwrap()
    ));
    assert!(
        is_competition_board_state_call(&board),
        "mixed-case LIVING_HANDOFF still counts as board wait"
    );
    assert!(!burns_first_write_budget(&board));

    let grep = ToolCall {
        id: "4".into(),
        name: "grep".into(),
        args: serde_json::json!({"pattern": "stream", "path": "src/kernel.cu"}),
    };
    assert!(
        classify_hay_borrow(&grep).is_none(),
        "pattern+path must still build a hay"
    );
}

/// Classify hops borrow a single apply_patch target from the diff header.
/// Multi-file patches still build a hay; hunk bodies stay out.
#[test]
fn competition_call_text_borrows_apply_patch_path() {
    let hunk = "fn main() { /* leaderboard submissions hilbert submit */ }\n".repeat(4_000);
    assert!(hunk.len() > 100_000);
    let diff = format!(
        "*** Begin Patch\n*** Update File: src/kernel.cu\n@@\n-{}\n+{hunk}\n*** End Patch\n",
        "x".repeat(32)
    );
    let call = ToolCall {
        id: "p".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({ "diff": diff }),
    };
    assert_eq!(classify_apply_patch_borrow(&call), Some("src/kernel.cu"));
    assert_eq!(classify_hay_borrow(&call), Some("src/kernel.cu"));
    let (_, hay, is_shell) = competition_call_text(&call);
    assert!(!is_shell);
    assert_eq!(hay.as_ref(), "src/kernel.cu");
    let diff = call.args["diff"].as_str().unwrap();
    let hay_ptr = hay.as_ptr();
    let diff_ptr = diff.as_ptr();
    assert!(
        hay_ptr >= diff_ptr && hay_ptr < unsafe { diff_ptr.add(diff.len()) },
        "classify must borrow the path from the diff"
    );
    assert!(is_first_write_progress_call(&call));
    assert!(!is_competition_outcome_call(&call));
    assert!(!is_competition_board_state_call(&call));

    let meta = ToolCall {
        id: "m".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({
            "diff": "*** Update File: LIVING_HANDOFF.md\n@@\n+tip\n"
        }),
    };
    assert_eq!(
        classify_apply_patch_borrow(&meta),
        Some("LIVING_HANDOFF.md")
    );
    assert!(
        is_competition_board_state_call(&meta),
        "mixed-case handoff patch still counts as board wait"
    );
    assert!(!is_first_write_progress_call(&meta));

    let multi = ToolCall {
        id: "2".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({
            "diff": "*** Update File: src/a.rs\n+one\n*** Update File: src/b.rs\n+two\n"
        }),
    };
    assert!(
        classify_apply_patch_borrow(&multi).is_none(),
        "multi-file patches still build a hay"
    );
    let (_, multi_hay, _) = competition_call_text(&multi);
    assert!(multi_hay.contains("src/a.rs"));
    assert!(multi_hay.contains("src/b.rs"));
}

/// Classify hops borrow a lowercase shell command, and a command whose
/// only uppercase is in a path (`cat LIVING_HANDOFF.md`). Mixed-case
/// program words still fold so `SUBMIT` counts as outcome.
#[test]
fn competition_call_text_borrows_lowercase_shell() {
    assert!(!classify_shell_hay_needs_lower("hilbert submit --note x"));
    assert!(!classify_shell_hay_needs_lower("cat LIVING_HANDOFF.md"));
    assert!(!classify_shell_hay_needs_lower("addr2line -e src/kernel.o"));
    assert!(classify_shell_hay_needs_lower("hilbert SUBMIT --note x"));
    assert!(classify_shell_hay_needs_lower("GIT DIFF src/kernel.cu"));

    let lower = ToolCall {
        id: "1".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "hilbert submit --note x"}),
    };
    let (_, hay, is_shell) = competition_call_text(&lower);
    assert!(is_shell);
    assert!(
        std::ptr::eq(hay.as_ref(), lower.args["command"].as_str().unwrap()),
        "already-lowercase shell must be borrowed"
    );
    assert!(is_competition_outcome_call(&lower));

    let board = ToolCall {
        id: "2".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "cat LIVING_HANDOFF.md"}),
    };
    let (_, board_hay, _) = competition_call_text(&board);
    assert!(
        std::ptr::eq(board_hay.as_ref(), board.args["command"].as_str().unwrap()),
        "path-cased board digest must be borrowed"
    );
    assert!(is_competition_board_state_call(&board));
    assert!(!burns_first_write_budget(&board));

    let mixed = ToolCall {
        id: "3".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "hilbert SUBMIT --note x"}),
    };
    let (_, mixed_hay, _) = competition_call_text(&mixed);
    assert!(matches!(mixed_hay, std::borrow::Cow::Owned(_)));
    assert!(mixed_hay.contains("hilbert submit"));
    assert!(
        is_competition_outcome_call(&mixed),
        "mixed-case SUBMIT still counts as submit"
    );
}

/// Classify hops borrow the tool name. Mixed-case `SHELL` still classifies
/// as a submit outcome; the name is not lowercased into a new String.
#[test]
fn competition_call_text_borrows_tool_name() {
    let call = ToolCall {
        id: "1".into(),
        name: "SHELL".into(),
        args: serde_json::json!({"command": "hilbert submit --note x"}),
    };
    let (name, hay, is_shell) = competition_call_text(&call);
    assert!(
        std::ptr::eq(name, call.name.as_str()),
        "classify must borrow the tool name"
    );
    assert_eq!(name, "SHELL");
    assert!(is_shell);
    assert!(hay.contains("hilbert submit"));
    assert!(
        is_competition_outcome_call(&call),
        "mixed-case SHELL still counts as submit"
    );
}

/// A4 adversarial: content-search thrash that only *mentions* a board path must
/// still burn first-write budget (was limited to `grep -r` / `rg` / `find`).
#[test]
fn competition_board_state_does_not_launder_grep_recon() {
    let cases = [
        "grep living_handoff src/ -n",
        "grep -n living-handoff cockpit/",
        "egrep -n board_tip .",
        "git grep living_handoff",
        "ag living_handoff",
        "fd living_handoff",
        "FOO=1 rg living_handoff",
        // Long-form and combined short recursive ls (bare `ls -r` substring used
        // to miss `-laR` / `-lR` / `-1R` clusters).
        "ls --recursive LIVING_HANDOFF.md",
        "ls --recursive /tmp/living-handoff.md",
        "ls --recurse .angel/notes",
        "ls -laR LIVING_HANDOFF.md",
        "ls -lR /tmp/living-handoff.md",
        "ls -1R .angel/notes",
        "ls --color=auto -R /tmp/living",
        "/bin/ls -alR living_handoff",
    ];
    for cmd in cases {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "search thrash must not count as board wait: {cmd}"
        );
        assert!(
            burns_first_write_budget(&call),
            "search thrash must burn first-write budget: {cmd}"
        );
    }
    // Legal digests stay wait/poll.
    let legal = [
        "cat LIVING_HANDOFF.md",
        "head -40 /tmp/living-handoff.md",
        "wc -l .angel/notes/board.md",
    ];
    for cmd in legal {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            is_competition_board_state_call(&call),
            "board digest must remain wait: {cmd}"
        );
        assert!(!burns_first_write_budget(&call), "board digest: {cmd}");
    }
}

/// A4 adversarial: apply_patch that only touches living-handoff / board notes
/// must not count as first-write progress (paths used to be empty → historical yes).
#[test]
fn apply_patch_meta_note_only_is_not_first_write_progress() {
    let meta_unified = ToolCall {
        id: "1".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({
            "diff": "--- a/LIVING_HANDOFF.md\n+++ b/LIVING_HANDOFF.md\n@@ -1 +1,2 @@\n tip\n+still validating\n"
        }),
    };
    let meta_freeform = ToolCall {
        id: "2".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({
            "diff": "*** Begin Patch\n*** Update File: .angel/notes/board.md\n@@\n-a\n+b\n*** End Patch\n"
        }),
    };
    let core = ToolCall {
        id: "3".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({
            "diff": "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1,2 @@\n pub fn x() {}\n+pub fn y() {}\n"
        }),
    };
    let mixed = ToolCall {
        id: "4".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({
            "diff": "--- a/LIVING_HANDOFF.md\n+++ b/LIVING_HANDOFF.md\n@@ -1 +1,2 @@\n tip\n+x\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1,2 @@\n fn main() {}\n+// ok\n"
        }),
    };

    let meta_paths = mutation_call_paths(&meta_unified);
    assert!(
        meta_paths.iter().any(|p| p.contains("LIVING_HANDOFF")),
        "patch paths should surface handoff target: {meta_paths:?}"
    );
    assert!(
        !is_first_write_progress_call(&meta_unified),
        "meta-only unified patch must not be first-write progress"
    );
    assert!(
        !is_first_write_progress_call(&meta_freeform),
        "meta-only freeform patch must not be first-write progress"
    );
    assert!(
        is_first_write_progress_call(&core),
        "product apply_patch remains first-write progress"
    );
    assert!(
        is_first_write_progress_call(&mixed),
        "mixed meta+core patch is still progress"
    );
    // Meta patches are bookkeeping — not recon thrash and not product progress.
    assert!(
        !burns_first_write_budget(&meta_unified),
        "meta-only patch must not burn first-write inspection budget"
    );
}

/// Hop-loop classify walks apply_patch headers in place. A megabyte product
/// hunk must not be copied into mutation_targets or competition hay.
#[test]
fn apply_patch_classify_skips_hunk_bodies() {
    let hunk = "fn main() { /* leaderboard submissions hilbert submit living_handoff.md */ }\n"
        .repeat(4_000);
    assert!(
        hunk.len() > 100_000,
        "payload must be large enough to matter"
    );
    let call = ToolCall {
        id: "p".into(),
        name: "apply_patch".into(),
        args: serde_json::json!({
            "diff": format!(
                "*** Begin Patch\n*** Update File: src/kernel.cu\n@@\n-{}\n+{hunk}\n*** End Patch\n",
                "x".repeat(32)
            )
        }),
    };
    assert_eq!(
        crate::knowledge::cut::mutation_targets("apply_patch", &call.args),
        vec!["src/kernel.cu".to_string()]
    );
    assert!(mutation_call_has_product_path(&call));
    assert!(is_first_write_progress_call(&call));
    assert!(
        mutation_requires_verification(&call),
        "product apply_patch still requires verification"
    );
    assert!(
        !is_competition_outcome_call(&call),
        "hunk text must not launder as submit"
    );
    assert!(
        !is_competition_board_state_call(&call),
        "hunk text must not launder as board wait"
    );
    let (name, hay, is_shell) = competition_call_text(&call);
    assert_eq!(name, "apply_patch");
    assert!(!is_shell);
    assert!(
        hay.len() < 256,
        "classification hay must stay path-sized, got {}",
        hay.len()
    );
    assert!(hay.contains("src/kernel.cu"));
    assert!(
        !hay.contains("fn main"),
        "hunk body must stay out of the hay"
    );
    assert!(!hay.contains("leaderboard"));
}

/// A4 adversarial: git inventory/history that only *names* a board path must
/// not launder as competition wait/poll.
#[test]
fn competition_board_state_does_not_launder_git_inventory() {
    let cases = [
        "git ls-files LIVING_HANDOFF.md",
        "git ls-files '*handoff*'",
        "git ls-tree -r HEAD --name-only | head",
        "git blame LIVING_HANDOFF.md",
        "git log --oneline -- living_handoff.md",
        "git log -1 -- board.md",
    ];
    for cmd in cases {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "git inventory/history must not count as board wait: {cmd}"
        );
        assert!(
            burns_first_write_budget(&call),
            "git inventory/history must burn first-write budget: {cmd}"
        );
        let (_, _, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(!wait && burns, "hop flags for: {cmd}");
    }
    // Legal digests stay wait/poll.
    let legal = [
        "cat LIVING_HANDOFF.md",
        "head -40 /tmp/living-handoff.md",
        "git show HEAD:LIVING_HANDOFF.md",
    ];
    for cmd in legal {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            is_competition_board_state_call(&call),
            "board digest must remain wait: {cmd}"
        );
        assert!(!burns_first_write_budget(&call), "board digest: {cmd}");
    }
}

/// A4 adversarial: `git shortlog` / `rev-list` / `reflog` that only *name* a
/// board path laundered as competition wait (history inventory, not a tip digest).
/// Full-binary content search (`ripgrep`/`ugrep`) must also burn first-write.
#[test]
fn competition_board_state_does_not_launder_git_history_inventory() {
    let cases = [
        "git shortlog -- living_handoff.md",
        "git shortlog -sn -- board.md",
        "git rev-list --all -- board.md",
        "git rev-list HEAD -- LIVING_HANDOFF.md",
        "git reflog -- board.md",
        "git reflog show -- tip.md",
        "FOO=1 git shortlog -- /tmp/living-handoff.md",
        "/usr/bin/git rev-list --count HEAD -- living-handoff.md",
        // Full search binary names (not only `rg ` / `grep ` prefixes).
        "ripgrep living_handoff",
        "ripgrep -n board_tip .",
        "/usr/bin/ripgrep living-handoff",
        "ugrep -r living_handoff",
    ];
    for cmd in cases {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "history inventory / content search must not count as board wait: {cmd}"
        );
        assert!(
            burns_first_write_budget(&call),
            "history inventory / content search must burn first-write: {cmd}"
        );
        let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(!wait && burns, "hop flags for: {cmd}");
        assert!(
            anti_spin_counts_batch(mutation, outcome, wait, burns),
            "recon must still count as spin: {cmd}"
        );
    }
    // Content digests stay wait/poll.
    let legal = [
        "cat LIVING_HANDOFF.md",
        "head -40 /tmp/living-handoff.md",
        "git show HEAD:LIVING_HANDOFF.md",
        "wc -l .angel/notes/board.md",
    ];
    for cmd in legal {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            is_competition_board_state_call(&call),
            "board digest must remain wait: {cmd}"
        );
        assert!(!burns_first_write_budget(&call), "board digest: {cmd}");
    }
}

/// A8 adversarial: `git diff` / `git status` that only *name* a board path used
/// to miss the git inventory thrash list (ls-files/blame/log only) and launder
/// as competition wait — skipping first-write burn and anti-spin counts.
#[test]
fn competition_board_state_does_not_launder_git_diff_status() {
    let cases = [
        "git diff LIVING_HANDOFF.md",
        "git diff --stat HEAD -- board.md",
        "git diff -- living_handoff.md",
        "git status -- LIVING_HANDOFF.md",
        "git status -sb -- board.md",
        "FOO=1 git diff /tmp/living-handoff.md",
        "/usr/bin/git status --porcelain -- tip.md",
    ];
    for cmd in cases {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "git diff/status must not count as board wait: {cmd}"
        );
        assert!(
            burns_first_write_budget(&call),
            "git diff/status must burn first-write budget: {cmd}"
        );
        let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(!wait && burns, "hop flags for: {cmd}");
        assert!(
            anti_spin_counts_batch(mutation, outcome, wait, burns),
            "git diff/status recon must still count as spin: {cmd}"
        );
    }
    // Content digests stay wait/poll (`git show` is a tip read, not inventory).
    let legal = [
        "cat LIVING_HANDOFF.md",
        "head -40 /tmp/living-handoff.md",
        "git show HEAD:LIVING_HANDOFF.md",
    ];
    for cmd in legal {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            is_competition_board_state_call(&call),
            "board digest must remain wait: {cmd}"
        );
        assert!(!burns_first_write_budget(&call), "board digest: {cmd}");
        let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(
            !anti_spin_counts_batch(mutation, outcome, wait, burns),
            "pure board wait must not count as spin: {cmd}"
        );
    }
}

/// A4 adversarial: native grep/find_files tools must not launder recon as board wait.
#[test]
fn competition_board_state_does_not_launder_native_recon_tools() {
    let cases = [
        (
            "grep",
            serde_json::json!({"pattern": "fn main", "path": "LIVING_HANDOFF.md"}),
        ),
        (
            "grep",
            serde_json::json!({"pattern": "living_handoff", "path": "src"}),
        ),
        ("find_files", serde_json::json!({"glob": "*handoff*"})),
        (
            "file_search",
            serde_json::json!({"query": "living_handoff"}),
        ),
        ("outline", serde_json::json!({"path": "LIVING_HANDOFF.md"})),
        ("defs", serde_json::json!({"symbol": "board_tip"})),
        ("list_dir", serde_json::json!({"path": "/tmp/living"})),
        (
            "code_mode",
            serde_json::json!({"recipe": "repo_recon", "query": "living_handoff tip"}),
        ),
    ];
    for (name, args) in cases {
        let call = ToolCall {
            id: "t".into(),
            name: name.into(),
            args: args.clone(),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "native recon must not count as board wait: {name} {args}"
        );
        assert!(
            burns_first_write_budget(&call),
            "native recon must burn first-write: {name}"
        );
        let (_, _, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(!wait && burns, "hop flags for {name}");
    }
    // read_file of the board remains a legal digest.
    let digest = ToolCall {
        id: "d".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "LIVING_HANDOFF.md"}),
    };
    assert!(is_competition_board_state_call(&digest));
    assert!(!burns_first_write_budget(&digest));
    // Shell digests still legal wait.
    let tip = ToolCall {
        id: "s".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "head -40 /tmp/living-handoff.md"}),
    };
    assert!(is_competition_board_state_call(&tip));
    assert!(!burns_first_write_budget(&tip));
}

/// A8 adversarial: native git_diff / git_status / git_log tools must not launder
/// as competition board wait when args only *name* a handoff path — same class
/// as shell `git diff|status|log` thrash (first-write burn + anti-spin count).
#[test]
fn competition_board_state_does_not_launder_native_git_inventory() {
    let cases = [
        ("git_diff", serde_json::json!({"path": "LIVING_HANDOFF.md"})),
        (
            "git_diff",
            serde_json::json!({"path": "board.md", "stat": true}),
        ),
        (
            "git_status",
            serde_json::json!({"path": "living_handoff.md"}),
        ),
        (
            "git_status",
            serde_json::json!({"path": "/tmp/living-handoff.md"}),
        ),
        ("git_log", serde_json::json!({"path": "LIVING_HANDOFF.md"})),
        (
            "git_log",
            serde_json::json!({"path": "board.md", "max_count": 5}),
        ),
    ];
    for (name, args) in cases {
        let call = ToolCall {
            id: "t".into(),
            name: name.into(),
            args: args.clone(),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "native git inventory must not count as board wait: {name} {args}"
        );
        assert!(
            burns_first_write_budget(&call),
            "native git inventory must burn first-write: {name}"
        );
        let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(!wait && burns, "hop flags for {name}");
        assert!(
            anti_spin_counts_batch(mutation, outcome, wait, burns),
            "native git inventory must still count as spin: {name}"
        );
    }
    // Legal digests stay wait/poll.
    let digest = ToolCall {
        id: "d".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "LIVING_HANDOFF.md"}),
    };
    assert!(is_competition_board_state_call(&digest));
    assert!(!burns_first_write_budget(&digest));
    let show = ToolCall {
        id: "s".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "git show HEAD:LIVING_HANDOFF.md"}),
    };
    assert!(is_competition_board_state_call(&show));
    assert!(!burns_first_write_budget(&show));
}

/// A8 adversarial: `git show --stat` / pathspec forms and `git whatchanged` that
/// only *name* a board path used to launder as competition wait (legal tip read
/// is only `git show REV:path` blob syntax). Must burn first-write and count as spin.
#[test]
fn competition_board_state_does_not_launder_git_show_inventory() {
    let cases = [
        "git show --stat HEAD -- board.md",
        "git show --name-only HEAD -- living_handoff.md",
        "git show --name-status HEAD -- LIVING_HANDOFF.md",
        "git show --numstat HEAD -- tip.md",
        "git show HEAD -- board.md",
        "git show HEAD board.md",
        "git whatchanged -- board.md",
        "git whatchanged -1 -- living_handoff.md",
        "FOO=1 git show --stat -- /tmp/living-handoff.md",
        "/usr/bin/git show --name-only HEAD -- tip.md",
        "timeout 5 git show --stat HEAD -- board.md",
    ];
    for cmd in cases {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "git show/whatchanged inventory must not count as board wait: {cmd}"
        );
        assert!(
            burns_first_write_budget(&call),
            "git show/whatchanged inventory must burn first-write: {cmd}"
        );
        let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(!wait && burns, "hop flags for: {cmd}");
        assert!(
            anti_spin_counts_batch(mutation, outcome, wait, burns),
            "git show inventory must still count as spin: {cmd}"
        );
    }
    // Pure blob digests stay wait/poll.
    let legal = [
        "git show HEAD:LIVING_HANDOFF.md",
        "git show HEAD:.angel/notes/board.md",
        "git show main:tip.md",
        "cat LIVING_HANDOFF.md",
        "head -40 /tmp/living-handoff.md",
    ];
    for cmd in legal {
        let call = ToolCall {
            id: "t".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": cmd}),
        };
        assert!(
            is_competition_board_state_call(&call),
            "board blob digest must remain wait: {cmd}"
        );
        assert!(!burns_first_write_budget(&call), "board digest: {cmd}");
        let (mutation, outcome, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(
            !anti_spin_counts_batch(mutation, outcome, wait, burns),
            "pure board wait must not count as spin: {cmd}"
        );
    }
}

/// A4 adversarial: LSP workspace/symbol search must not launder as board wait
/// when the query or path only *mentions* a handoff marker.
#[test]
fn competition_board_state_does_not_launder_lsp_recon() {
    let cases = [
        (
            "lsp_workspace_symbol",
            serde_json::json!({"query": "living_handoff"}),
        ),
        (
            "lsp_workspace_symbol",
            serde_json::json!({"query": "board_tip"}),
        ),
        (
            "lsp_symbols",
            serde_json::json!({"path": "LIVING_HANDOFF.md"}),
        ),
        (
            "lsp_references",
            serde_json::json!({"path": "src/lib.rs", "symbol": "board_tip"}),
        ),
        (
            "lsp_definition",
            serde_json::json!({"path": "/tmp/living/handoff.md", "symbol": "tip"}),
        ),
    ];
    for (name, args) in cases {
        let call = ToolCall {
            id: "t".into(),
            name: name.into(),
            args: args.clone(),
        };
        assert!(
            !is_competition_board_state_call(&call),
            "LSP recon must not count as board wait: {name} {args}"
        );
        assert!(
            burns_first_write_budget(&call),
            "LSP recon must burn first-write: {name}"
        );
        let (_, _, wait, burns) = hop_budget_flags(std::slice::from_ref(&call));
        assert!(!wait && burns, "hop flags for {name}");
    }
    // Legal board digests stay wait/poll.
    let digest = ToolCall {
        id: "d".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": ".angelX/notes/board.md"}),
    };
    assert!(is_competition_board_state_call(&digest));
    assert!(!burns_first_write_budget(&digest));
}

/// A8 adversarial: path/prose substrings must not launder recon as competition
/// wait (first-write budget + anti-starvation interaction).
#[test]
fn competition_outcome_ignores_score_path_and_validating_prose() {
    let score_src = ToolCall {
        id: "1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/score.rs"}),
    };
    let score_cat = ToolCall {
        id: "2".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "sed -n '1,40p' crates/game/src/score.rs"}),
    };
    let validating_write = ToolCall {
        id: "3".into(),
        name: "write_file".into(),
        args: serde_json::json!({
            "path": "src/lib.rs",
            "content": "tip: still validating the parser\n"
        }),
    };
    let submit_form = ToolCall {
        id: "4".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "web/submit_form.html"}),
    };
    let real_submit = ToolCall {
        id: "5".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "hilbert submit --note cand"}),
    };
    let real_score = ToolCall {
        id: "6".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "popcorn-cli status && score --json"}),
    };

    assert!(
        !is_competition_outcome_call(&score_src),
        "read_file of score.rs is recon, not a competition outcome"
    );
    assert!(
        burns_first_write_budget(&score_src),
        "score.rs recon must burn first-write budget"
    );
    assert!(
        !is_competition_outcome_call(&score_cat),
        "cat of a score.rs path must not count as outcome"
    );
    assert!(burns_first_write_budget(&score_cat));
    assert!(
        !is_competition_outcome_call(&validating_write),
        "prose 'validating' in a product edit is not a board poll"
    );
    // Product mutation is first-write progress (does not burn inspection budget).
    assert!(is_first_write_progress_call(&validating_write));
    assert!(!is_competition_outcome_call(&submit_form));
    assert!(burns_first_write_budget(&submit_form));

    assert!(is_competition_outcome_call(&real_submit));
    assert!(!burns_first_write_budget(&real_submit));
    assert!(is_competition_outcome_call(&real_score));
    assert!(!burns_first_write_budget(&real_score));
}

#[test]
fn shell_argv_has_token_matches_in_place() {
    assert!(shell_argv_has_token("hilbert SUBMIT --note cand", "submit"));
    assert!(shell_argv_has_token("GIT BLAME src/kernel.cu", "blame"));
    assert!(shell_argv_has_token("score --JSON", "score"));
    assert!(
        shell_argv_has_token("tool --SUBMIT", "submit"),
        "long-opt --SUBMIT still counts as the submit token"
    );
    assert!(
        !shell_argv_has_token("sed -n '1,40p' crates/game/src/score.rs", "score"),
        "score.rs path fragment is not an argv token"
    );
    assert!(
        !shell_argv_has_token("read_file web/submit_form.html", "submit"),
        "submit_form.html is not an argv token"
    );
    assert!(!shell_argv_has_token(
        "still validating the parser",
        "submit"
    ));
}

#[test]
fn first_write_does_not_artificially_reject_or_drop_calls() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _rejections = EnvGuard::set("ANGEL_FIRST_WRITE_REJECTIONS", "0");

    struct InspectionOnly {
        call: AtomicUsize,
    }
    struct CountedRead(Arc<AtomicUsize>);
    impl Tool for CountedRead {
        fn name(&self) -> &str {
            "read_file"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().to_string(),
                description: "count executions".to_string(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("observed".to_string())
        }
    }
    impl Club for InspectionOnly {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "inspection-only"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let call = self.call.fetch_add(1, Ordering::Relaxed);
            if call >= 4 {
                return Ok(ClubReply::Text("inspection complete".into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("read-{call}"),
                name: "read_file".into(),
                args: serde_json::json!({ "path": "Cargo.toml" }),
            }]))
        }
    }

    for (budget, expected_nudges) in [("0", 0), ("2", 1), ("10", 0)] {
        let _budget = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", budget);
        let mut history = vec![ChatMsg::user("repair the code")];
        let executions = Arc::new(AtomicUsize::new(0));
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(CountedRead(Arc::clone(&executions))));
        let (event_tx, _event_rx) = mpsc::channel::<TurnEvent>();
        let outcome = run_turn_observed(
            &InspectionOnly {
                call: AtomicUsize::new(0),
            },
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(30),
            &event_tx,
        )
        .expect("turn completes normally without artificial first-write stop");

        assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
        assert_eq!(outcome.answer, "inspection complete");
        assert_eq!(executions.load(Ordering::SeqCst), 4);
        assert_eq!(
            history
                .iter()
                .filter(|message| {
                    message.role == ChatRole::Harness
                        && message.content.contains("ACTIONABLE CANDIDATE PROGRESS")
                })
                .count(),
            expected_nudges,
            "the exhausted budget emits once; disabled and unreached budgets stay silent"
        );
        assert_eq!(
            history
                .iter()
                .map(|message| message.tool_calls.len())
                .sum::<usize>(),
            4
        );
        assert_eq!(
            history
                .iter()
                .filter(|message| message.role == ChatRole::Tool)
                .count(),
            4
        );
    }
}

#[test]
fn first_write_rejection_blocks_post_budget_recon_but_allows_product_mutation() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _budget = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "1");
    let _rejections = EnvGuard::set("ANGEL_FIRST_WRITE_REJECTIONS", "3");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _diagnostics = EnvGuard::set("ANGEL_LSP_POSTCHECK", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");

    struct ReconThenMutation {
        step: AtomicUsize,
    }
    struct CountedTool {
        name: &'static str,
        executions: Arc<AtomicUsize>,
    }
    impl Tool for CountedTool {
        fn name(&self) -> &str {
            self.name
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name.to_string(),
                description: "count executions".to_string(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            Ok(format!("{} executed", self.name))
        }
    }
    impl Club for ReconThenMutation {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "recon-then-mutation"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            let step = self.step.fetch_add(1, Ordering::SeqCst);
            let call = match step {
                0 | 1 => ToolCall {
                    id: format!("read-{step}"),
                    name: "read_file".into(),
                    args: serde_json::json!({"path":"src/candidate.rs"}),
                },
                2 => {
                    assert!(messages.iter().any(|message| {
                        message.role == ChatRole::Tool
                            && message.content.contains(FIRST_WRITE_REJECT_RESULT)
                    }));
                    ToolCall {
                        id: "write-2".into(),
                        name: "write_file".into(),
                        args: serde_json::json!({"path":"src/candidate.rs","content":"better"}),
                    }
                }
                _ => return Ok(ClubReply::Text("candidate advanced".into())),
            };
            Ok(ClubReply::Calls(vec![call]))
        }
    }

    let reads = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountedTool {
        name: "read_file",
        executions: Arc::clone(&reads),
    }));
    registry.register(Box::new(CountedTool {
        name: "write_file",
        executions: Arc::clone(&writes),
    }));
    let mut history = vec![ChatMsg::user(
        "run the competition-loop and improve the candidate",
    )];
    let outcome = run_turn_observed(
        &ReconThenMutation {
            step: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(10),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("mutation remains available after one rejected inspection");

    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(outcome.answer, "candidate advanced");
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(writes.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_mutation_does_not_disarm_first_write_recon_guard() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _budget = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "1");
    let _rejections = EnvGuard::set("ANGEL_FIRST_WRITE_REJECTIONS", "3");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _diagnostics = EnvGuard::set("ANGEL_LSP_POSTCHECK", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");

    struct FailedMutationThenRecon {
        step: AtomicUsize,
    }
    struct CountedRead(Arc<AtomicUsize>);
    struct FailingWrite(Arc<AtomicUsize>);
    impl Tool for CountedRead {
        fn name(&self) -> &str {
            "read_file"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().to_string(),
                description: "count executions".to_string(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("observed".to_string())
        }
    }
    impl Tool for FailingWrite {
        fn name(&self) -> &str {
            "write_file"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().to_string(),
                description: "fail after dispatch".to_string(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err("simulated write failure".to_string())
        }
    }
    impl Club for FailedMutationThenRecon {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "failed-mutation-then-recon"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            let step = self.step.fetch_add(1, Ordering::SeqCst);
            let call = match step {
                0 => ToolCall {
                    id: "read-before-write".into(),
                    name: "read_file".into(),
                    args: serde_json::json!({"path":"src/candidate.rs"}),
                },
                1 => ToolCall {
                    id: "failed-write".into(),
                    name: "write_file".into(),
                    args: serde_json::json!({"path":"src/candidate.rs","content":"better"}),
                },
                2 => ToolCall {
                    id: "read-after-failed-write".into(),
                    name: "read_file".into(),
                    args: serde_json::json!({"path":"src/another.rs"}),
                },
                _ => {
                    assert!(messages.iter().any(|message| {
                        message.role == ChatRole::Tool
                            && message.content.contains(FIRST_WRITE_REJECT_RESULT)
                    }));
                    return Ok(ClubReply::Text("failed write stayed guarded".into()));
                }
            };
            Ok(ClubReply::Calls(vec![call]))
        }
    }

    let reads = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountedRead(Arc::clone(&reads))));
    registry.register(Box::new(FailingWrite(Arc::clone(&writes))));
    let mut history = vec![ChatMsg::user(
        "run the competition-loop and improve the candidate",
    )];
    let outcome = run_turn_observed(
        &FailedMutationThenRecon {
            step: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(10),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("a dispatched write failure keeps subsequent reconnaissance guarded");

    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(outcome.answer, "failed write stayed guarded");
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(writes.load(Ordering::SeqCst), 1);
}

#[test]
fn first_write_rejection_limit_stops_a_turn_that_keeps_inspecting() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _budget = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "1");
    let _rejections = EnvGuard::set("ANGEL_FIRST_WRITE_REJECTIONS", "2");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");

    struct InspectionForever {
        call: AtomicUsize,
    }
    struct CountedRead(Arc<AtomicUsize>);
    impl Tool for CountedRead {
        fn name(&self) -> &str {
            "read_file"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().to_string(),
                description: "count executions".to_string(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("observed".to_string())
        }
    }
    impl Club for InspectionForever {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "inspection-forever"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            let call = self.call.fetch_add(1, Ordering::SeqCst);
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("read-{call}"),
                name: "read_file".into(),
                args: serde_json::json!({"path":format!("src/candidate-{call}.rs")}),
            }]))
        }
    }

    let reads = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountedRead(Arc::clone(&reads))));
    let mut history = vec![ChatMsg::user(
        "run the competition-loop and improve the candidate",
    )];
    let outcome = run_turn_observed(
        &InspectionForever {
            call: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(10),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("first-write circuit breaker returns a typed stopped outcome");

    assert_eq!(outcome.stop_reason, TurnStopReason::Spin);
    assert!(outcome.answer.contains("stopped after 2 post-budget"));
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn final_mile_dispatches_tools_without_artificial_call_dropping() {
    let _guard = crate::tests::env_lock();
    let _reserve = EnvGuard::set("ANGEL_FINAL_MILE_HOPS", "3");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _verify_gate = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    struct MutateThenInspect {
        hop: AtomicUsize,
    }
    impl Club for MutateThenInspect {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "mutate-then-inspect"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            match self.hop.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(ClubReply::Calls(vec![ToolCall {
                    id: "mutation".into(),
                    name: "str_replace".into(),
                    args: serde_json::json!({"path":"src/lib.rs","old":"a","new":"b"}),
                }])),
                1 => Ok(ClubReply::Calls(vec![
                    ToolCall {
                        id: "inspect-a".into(),
                        name: "reverse".into(),
                        args: serde_json::json!({"text":"a"}),
                    },
                    ToolCall {
                        id: "inspect-b".into(),
                        name: "reverse".into(),
                        args: serde_json::json!({"text":"b"}),
                    },
                ])),
                _ => Ok(ClubReply::Text("reported verifier blocker".into())),
            }
        }
    }

    let mutations = Arc::new(AtomicUsize::new(0));
    let inspections = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingProbe {
        name: "str_replace",
        calls: Arc::clone(&mutations),
    }));
    registry.register(Box::new(CountingProbe {
        name: "reverse",
        calls: Arc::clone(&inspections),
    }));
    let mut history = vec![ChatMsg::user("make a focused code change")];
    let (event_tx, _event_rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &MutateThenInspect {
            hop: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .unwrap();

    assert_eq!(answer, "reported verifier blocker");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    assert_eq!(
        inspections.load(Ordering::SeqCst),
        2,
        "inspections must dispatch directly without artificial call-dropping"
    );
}

#[test]
fn final_mile_answer_window_retains_tools_before_max_hops() {
    let _guard = crate::tests::env_lock();
    let _reserve = EnvGuard::set("ANGEL_FINAL_MILE_HOPS", "3");
    let _answer_window = EnvGuard::set("ANGEL_FINAL_MILE_ANSWER_HOPS", "2");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _verify_gate = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    struct MutateInspectThenAnswer {
        hop: AtomicUsize,
    }
    impl Club for MutateInspectThenAnswer {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "final-mile-answer-window"
        }
        fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            assert!(
                !tools.is_empty(),
                "schemas remain present in the final window"
            );
            match self.hop.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(ClubReply::Calls(vec![ToolCall {
                    id: "mutation".into(),
                    name: "str_replace".into(),
                    args: serde_json::json!({"path":"src/lib.rs","old":"a","new":"b"}),
                }])),
                _ if crate::agent::club::final_response_requested(messages) => Ok(ClubReply::Text(
                    "final answer from the bounded response window".into(),
                )),
                call => Ok(ClubReply::Calls(vec![ToolCall {
                    id: format!("inspect-{call}"),
                    name: "reverse".into(),
                    args: serde_json::json!({"text":"inspect"}),
                }])),
            }
        }
    }

    let mutations = Arc::new(AtomicUsize::new(0));
    let inspections = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingProbe {
        name: "str_replace",
        calls: Arc::clone(&mutations),
    }));
    registry.register(Box::new(CountingProbe {
        name: "reverse",
        calls: Arc::clone(&inspections),
    }));
    let mut history = vec![ChatMsg::user("make a focused code change")];
    let (event_tx, _event_rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &MutateInspectThenAnswer {
            hop: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .unwrap();

    assert_eq!(answer, "final answer from the bounded response window");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    assert_eq!(inspections.load(Ordering::SeqCst), 1);
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Harness && message.content.as_ref() == FINAL_MILE_ANSWER_NUDGE
    }));
}

#[cfg(unix)]
#[test]
fn final_mile_headless_answer_reports_unfinished_background_work() {
    let _guard = crate::tests::env_lock();
    let root = scratch("final-mile-background");
    let _env = [
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_FINAL_MILE_HOPS", "3"),
        EnvGuard::set("ANGEL_FINAL_MILE_ANSWER_HOPS", "2"),
        EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0"),
        EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0"),
        EnvGuard::set("ANGEL_BACKPLANE", "0"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_ROLLOUT_CAPTURE", "0"),
        EnvGuard::set("ANGEL_TASK_RECON", "0"),
        EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "60"),
        EnvGuard::set(
            "ANGEL_PROC_DIR",
            root.join("process-store").to_str().unwrap(),
        ),
    ];
    struct BackgroundThenAnswer<'a> {
        hops: AtomicUsize,
        workspace: &'a std::path::Path,
        finished: bool,
    }
    impl Club for BackgroundThenAnswer<'_> {
        fn label(&self) -> &str {
            "final-mile-background"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat(&self, messages: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
                return Ok(ClubReply::Calls(vec![
                    ToolCall {
                        id: "mutation".into(),
                        name: "str_replace".into(),
                        args: json!({"path":"subject","old":"a","new":"b"}),
                    },
                    ToolCall {
                        id: "background".into(),
                        name: "proc_run".into(),
                        args: json!({"command": if self.finished { "while [ ! -f release ]; do sleep 0.01; done" } else { "sleep 30" }}),
                    },
                ]));
            }
            assert!(crate::agent::club::final_response_requested(messages));
            assert!(
                messages
                    .iter()
                    .any(|m| m.role == ChatRole::Tool && m.content.starts_with("started [")),
                "background job must actually launch"
            );
            if self.finished {
                std::fs::write(self.workspace.join("release"), "go").unwrap();
                let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
                loop {
                    if crate::agent::tools::proc::take_completions(self.workspace, 8)
                        .iter()
                        .any(|notice| notice.exit_code == Some(0))
                    {
                        break;
                    }
                    assert!(std::time::Instant::now() < until, "fixture failed to exit");
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            Ok(ClubReply::Text("Recorded handoff.".into()))
        }
    }
    for finished in [false, true] {
        let cancel = AtomicBool::new(false);
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        registry.register(Box::new(CountingProbe {
            name: "str_replace",
            calls: Arc::new(AtomicUsize::new(0)),
        }));
        registry.register(Box::new(crate::agent::tools::proc::ProcRunTool::in_dir(
            root.clone(),
        )));
        let binding = TaskRolloutBindingV1::new(
            None,
            None,
            "a".repeat(64),
            "b".repeat(64),
            "fixture".into(),
            "c".repeat(64),
        );
        let mut history = vec![ChatMsg::user(
            "Change the subject and run the background job.",
        )];
        let (tx, _rx) = mpsc::channel();
        let outcome = run_task_turn_observed(
            &BackgroundThenAnswer {
                hops: AtomicUsize::new(0),
                workspace: &root,
                finished,
            },
            &registry,
            &mut history,
            &cancel,
            Some(3),
            &tx,
            &binding,
            None,
        );
        let outcome = outcome.unwrap();
        assert_eq!(outcome.answer, "Recorded handoff.");
        assert_eq!(
            outcome.stop_reason,
            if finished {
                TurnStopReason::Answer
            } else {
                TurnStopReason::MaxHops
            }
        );
        assert_eq!(outcome.max_hops_reached, !finished);
        assert!(!outcome.deadline_reached);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn final_mile_answer_window_never_executes_provider_calls_after_withdrawal() {
    let _guard = crate::tests::env_lock();
    let _reserve = EnvGuard::set("ANGEL_FINAL_MILE_HOPS", "1");
    let _answer_window = EnvGuard::set("ANGEL_FINAL_MILE_ANSWER_HOPS", "1");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _verify_gate = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");

    struct MutateThenIgnoreWithdrawnTools {
        hop: AtomicUsize,
    }
    impl Club for MutateThenIgnoreWithdrawnTools {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "final-mile-withheld-call"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            let (id, name) = if self.hop.fetch_add(1, Ordering::SeqCst) == 0 {
                ("mutation", "str_replace")
            } else {
                ("withheld", "reverse")
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: id.into(),
                name: name.into(),
                args: serde_json::json!({"text":"must not run"}),
            }]))
        }
    }

    let mutations = Arc::new(AtomicUsize::new(0));
    let inspections = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingProbe {
        name: "str_replace",
        calls: Arc::clone(&mutations),
    }));
    registry.register(Box::new(CountingProbe {
        name: "reverse",
        calls: Arc::clone(&inspections),
    }));
    let mut history = vec![ChatMsg::user("make a focused code change")];
    let (event_tx, _event_rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &MutateThenIgnoreWithdrawnTools {
            hop: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(2),
        &event_tx,
    )
    .unwrap();

    assert!(answer.contains("attempted unavailable tool calls (reverse)"));
    assert!(answer.contains("None executed"));
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    assert_eq!(inspections.load(Ordering::SeqCst), 0);
}

#[test]
fn turn_deadline_cancels_an_in_flight_tool_instead_of_waiting_for_its_own_timeout() {
    let _guard = crate::tests::env_lock();
    let _deadline = EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "1");
    let _spin = EnvGuard::set("ANGEL_SPIN_LIMIT", "0");
    let _error = EnvGuard::set("ANGEL_ERROR_LIMIT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");

    struct BlockingClub;
    impl Club for BlockingClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "turn-deadline-club"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "blocking-check".into(),
                name: "check".into(),
                args: serde_json::json!({}),
            }]))
        }
    }

    struct DeadlineAwareTool {
        observed_cancel: Arc<AtomicBool>,
    }
    impl Tool for DeadlineAwareTool {
        fn name(&self) -> &str {
            "check"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "deadline-aware blocking test tool".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            Err("fixture requires cancellation".into())
        }
        fn call_with_cancel(
            &self,
            _args: &Value,
            cancel: Option<&AtomicBool>,
        ) -> Result<String, String> {
            let cancel = cancel.ok_or("missing cancellation token")?;
            for _ in 0..1_000 {
                if cancel.load(Ordering::Acquire) {
                    self.observed_cancel.store(true, Ordering::Release);
                    return Err("blocking check cancelled".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err("turn deadline did not reach the blocking tool".into())
        }
    }

    let observed_cancel = Arc::new(AtomicBool::new(false));
    let mut registry = ToolRegistry::new();
    registry.set_workspace(scratch("turn_deadline_in_flight_tool"));
    registry.register(Box::new(DeadlineAwareTool {
        observed_cancel: Arc::clone(&observed_cancel),
    }));
    let operator_cancel = AtomicBool::new(false);
    let started = Instant::now();
    let outcome = run_turn_observed(
        &BlockingClub,
        &registry,
        &mut vec![ChatMsg::user("run the blocking check")],
        &operator_cancel,
        None,
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("turn deadline is a structured stop");

    assert_eq!(outcome.stop_reason, TurnStopReason::Deadline);
    assert!(outcome.deadline_reached);
    assert!(observed_cancel.load(Ordering::Acquire));
    assert!(!operator_cancel.load(Ordering::Acquire));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "turn deadline must beat the tool fixture's five-second ceiling"
    );
}

#[test]
fn turn_deadline_cancels_an_in_flight_provider_without_retrying_it() {
    let _guard = crate::tests::env_lock();
    let _deadline = EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "1");
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "3");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");

    struct DeadlineAwareProvider {
        calls: AtomicUsize,
        observed_cancel: AtomicBool,
        partial: bool,
    }
    impl Club for DeadlineAwareProvider {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "turn-deadline-provider"
        }
        fn chat_streaming(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.partial {
                on_delta(crate::agent::club::StreamDelta::Content("unfinished"));
            }
            for _ in 0..1_000 {
                if cancel.load(Ordering::Acquire) {
                    self.observed_cancel.store(true, Ordering::Release);
                    return Err(if self.partial {
                        format!("{} cancelled", crate::agent::club::INCOMPLETE_STREAM_ERR)
                    } else {
                        "provider request cancelled".into()
                    });
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err("turn deadline did not reach the provider".into())
        }
    }

    for partial in [false, true] {
        let club = DeadlineAwareProvider {
            calls: AtomicUsize::new(0),
            observed_cancel: AtomicBool::new(false),
            partial,
        };
        let operator_cancel = AtomicBool::new(false);
        // Deadline latency must not include fingerprinting the developer's
        // whole checkout during turn preflight.
        let mut registry = ToolRegistry::new();
        registry.set_workspace(scratch("turn_deadline_in_flight_provider"));
        let started = Instant::now();
        let (events, received) = mpsc::channel();
        let outcome = run_turn_observed(
            &club,
            &registry,
            &mut vec![ChatMsg::user("answer eventually")],
            &operator_cancel,
            None,
            &events,
        )
        .expect("turn deadline is a structured stop");

        assert_eq!(outcome.stop_reason, TurnStopReason::Deadline);
        assert!(outcome.deadline_reached);
        assert!(club.observed_cancel.load(Ordering::Acquire));
        assert_eq!(club.calls.load(Ordering::SeqCst), 1);
        assert!(!operator_cancel.load(Ordering::Acquire));
        if partial {
            assert!(
                received
                    .try_iter()
                    .any(|event| matches!(event, TurnEvent::SuppressPartial))
            );
        }
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "turn deadline must beat the provider fixture's five-second ceiling"
        );
    }
}

#[test]
fn turn_deadline_during_provider_backoff_prevents_another_request() {
    let _guard = crate::tests::env_lock();
    let _deadline = EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "1");
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "3");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "1100");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    struct FailingProvider(AtomicUsize);
    impl Club for FailingProvider {
        fn label(&self) -> &str {
            "deadline-backoff-fixture"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err("transient provider failure".into())
        }
    }
    let club = FailingProvider(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.set_workspace(scratch("turn_deadline_backoff"));
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut vec![ChatMsg::user("answer eventually")],
        &AtomicBool::new(false),
        None,
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("deadline during backoff is a structured stop");
    assert_eq!(outcome.stop_reason, TurnStopReason::Deadline);
    assert_eq!(club.0.load(Ordering::SeqCst), 1);
}

#[test]
fn run_turn_cancelled_partial_stream_is_an_interrupt_without_retry() {
    let _guard = crate::tests::env_lock();
    let _deadline = EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "0");
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "3");
    let operator_cancel = AtomicBool::new(false);
    struct CancelledProvider<'a> {
        operator_cancel: &'a AtomicBool,
        calls: AtomicUsize,
    }
    impl Club for CancelledProvider<'_> {
        fn label(&self) -> &str {
            "cancelled-partial-fixture"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!("streaming fixture")
        }
        fn chat_streaming(
            &self,
            _: &[ChatMsg],
            _: &[ToolDef],
            cancel: &AtomicBool,
            on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            on_delta(crate::agent::club::StreamDelta::Content("unfinished"));
            self.operator_cancel.store(true, Ordering::Release);
            for _ in 0..1_000 {
                if cancel.load(Ordering::Acquire) {
                    return Err(format!(
                        "{} cancelled",
                        crate::agent::club::INCOMPLETE_STREAM_ERR
                    ));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("operator cancellation did not reach the provider");
        }
    }
    let club = CancelledProvider {
        operator_cancel: &operator_cancel,
        calls: AtomicUsize::new(0),
    };
    let mut registry = ToolRegistry::new();
    registry.set_workspace(scratch("cancelled_partial_stream"));
    let mut history = vec![ChatMsg::user("answer until interrupted")];
    let (events, received) = mpsc::channel();
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &operator_cancel,
        None,
        &events,
    )
    .expect("operator cancellation is a structured stop");
    assert_eq!(outcome.stop_reason, TurnStopReason::Interrupt);
    assert_eq!(club.calls.load(Ordering::SeqCst), 1);
    assert!(
        received
            .try_iter()
            .any(|event| matches!(event, TurnEvent::SuppressPartial))
    );
    assert!(
        !history
            .iter()
            .any(|message| message.role == ChatRole::Assistant)
    );
}

#[test]
fn dispatched_failure_and_denial_outcomes_keep_their_existing_meaning() {
    let call = ToolCall {
        id: "ordinary".into(),
        name: "reverse".into(),
        args: serde_json::json!({"text":"hello"}),
    };
    assert_eq!(
        turn_event_outcome(&call, "tool error: exploded", false).execution,
        ExecutionOutcome::Failed
    );
    assert_eq!(
        turn_event_outcome(&call, "action capsule denied — not executed", true).execution,
        ExecutionOutcome::Denied
    );
    assert_eq!(
        turn_event_outcome(&call, "Worker Panicked while applying", false).execution,
        ExecutionOutcome::Panicked,
        "mixed-case panic still classifies without a lowercase copy"
    );
    assert_eq!(
        turn_event_outcome(&call, "Cancelled by the operator", false).execution,
        ExecutionOutcome::Cancelled
    );
    assert_eq!(
        turn_event_outcome(
            &call,
            "[timed out after 30s — no output ever, 4 live descendants; process killed]",
            false,
        )
        .execution,
        ExecutionOutcome::Failed
    );
    assert_eq!(
        turn_event_outcome(
            &call,
            "partial output\n[timed out after 30s — process killed]",
            false,
        )
        .execution,
        ExecutionOutcome::Failed
    );
    assert_eq!(
        turn_event_outcome(&call, "ok: Finished cargo check", false).execution,
        ExecutionOutcome::Succeeded
    );
}

#[test]
fn spin_redirect_perturbs_by_default() {
    // Perturbation text inverts the premise and reframes by analogy.
    let p = spin_redirect(true);
    assert!(
        p.contains("OPPOSITE"),
        "perturbation must test the opposite hypothesis"
    );
    assert!(
        p.contains("analogy"),
        "perturbation must offer a cross-domain reframe"
    );
    // The plain nudge is the bland fallback — no reframe.
    let n = spin_redirect(false);
    assert!(!n.contains("OPPOSITE"));
    assert!(n.contains("Change your approach"));
}

#[test]
fn run_turn_injects_perturbation_when_spinning() {
    let _guard = crate::tests::env_lock();
    let _operator_cap = EnvGuard::set("ANGEL_SPIN_LIMIT", "8");
    struct Stuck;
    impl Club for Stuck {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "stuck"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "x".into(),
                name: "reverse".into(),
                args: serde_json::json!({ "text": "x" }),
            }]))
        }
    }
    let mut history = vec![ChatMsg::user("go")];
    // Default env (perturbation on): hold the process-wide env lock so another
    // policy-ablation test cannot change the assumed defaults mid-turn.
    let _ = run_turn(
        &Stuck,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(50),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert!(
        history
            .iter()
            .any(|m| m.role == ChatRole::Harness && m.content.contains("OPPOSITE")),
        "a perturbation redirect should be injected once while spinning"
    );
}

#[test]
fn alternating_tool_cycle_stops_after_paired_outcomes_repeat() {
    let _guard = crate::tests::env_lock();
    let _operator_cap = EnvGuard::set("ANGEL_SPIN_LIMIT", "8");
    let _cycle_period = EnvGuard::set("ANGEL_TOOL_CYCLE_MAX_PERIOD", "5");
    let _cycle_repeats = EnvGuard::set("ANGEL_TOOL_CYCLE_REPEATS", "3");
    let _churn = EnvGuard::set("ANGEL_NOPROGRESS_LIMIT", "0");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");

    struct Alternating {
        calls: AtomicUsize,
    }
    impl Club for Alternating {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "alternating-cycle"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed);
            let text = if call.is_multiple_of(2) {
                "alpha"
            } else {
                "bravo"
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("cycle-{call}"),
                name: "reverse".into(),
                args: serde_json::json!({ "text": text }),
            }]))
        }
    }

    let club = Alternating {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("go")];
    let outcome = run_turn_observed(
        &club,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(20),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("a detected cycle is a structured stopped outcome");

    assert_eq!(outcome.stop_reason, TurnStopReason::Spin);
    assert_eq!(outcome.hops, 6);
    assert!(outcome.answer.contains("2-batch tool cycle"));
    assert_eq!(club.calls.load(Ordering::Relaxed), 6);
    let call_count = history.iter().map(|m| m.tool_calls.len()).sum::<usize>();
    let result_count = history.iter().filter(|m| m.role == ChatRole::Tool).count();
    assert_eq!(call_count, result_count, "cycle stop must preserve pairing");
}

#[test]
fn tool_cycle_detector_is_bounded_and_outcome_sensitive() {
    let mut detector = ToolBatchCycle::new(5, 3);
    for observation in [1, 2, 1, 2, 1] {
        assert_eq!(detector.observe(observation), None);
    }
    assert_eq!(detector.observe(2), Some(2));
    assert!(detector.retained() <= 15);

    detector.clear();
    for observation in [1, 2, 1, 3, 1, 2] {
        assert_eq!(detector.observe(observation), None);
    }
    assert_eq!(detector.retained(), 6);

    let call = ToolCall {
        id: "ignored-by-cycle-identity".into(),
        name: "reverse".into(),
        args: serde_json::json!({"text": "same"}),
    };
    let passed = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    let first = tool_batch_cycle_observation(
        std::slice::from_ref(&call),
        &[("result-a".into(), None, passed)],
    );
    let changed_result = tool_batch_cycle_observation(
        std::slice::from_ref(&call),
        &[("result-b".into(), None, passed)],
    );
    assert_ne!(first, changed_result, "changing tool evidence is progress");
}

/// Cycle identity hashes args in place (same as hop-loop anti-spin). Key
/// order is stable; megabyte write bodies do not rebuild storm strings.
#[test]
fn tool_batch_cycle_observation_hashes_calls_in_place() {
    let passed = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    let results = [("ok".into(), None, passed)];
    let a = ToolCall {
        id: "1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "offset": 1}),
    };
    let b = ToolCall {
        id: "2".into(),
        name: "read_file".into(),
        args: serde_json::json!({"offset": 1, "path": "src/main.rs"}),
    };
    assert_eq!(
        tool_batch_cycle_observation(std::slice::from_ref(&a), &results),
        tool_batch_cycle_observation(std::slice::from_ref(&b), &results),
        "key order must not change the cycle observation"
    );

    let body_a = "fn main() { /* cycle */ }\n".repeat(4_000);
    let body_b = "fn main() { /* other */ }\n".repeat(4_000);
    assert!(body_a.len() > 80_000);
    let write_a = ToolCall {
        id: "w1".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body_a}),
    };
    let write_b = ToolCall {
        id: "w2".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/main.rs", "content": body_b}),
    };
    let obs_a = tool_batch_cycle_observation(std::slice::from_ref(&write_a), &results);
    let obs_b = tool_batch_cycle_observation(std::slice::from_ref(&write_b), &results);
    assert_ne!(obs_a, obs_b, "distinct write bodies must not collide");
    assert_ne!(
        obs_a,
        anti_spin_batch_fingerprint(std::slice::from_ref(&write_a)),
        "results must still participate in the observation"
    );
}

/// Default hops hash the batch once for anti-spin and reuse that fingerprint
/// for cycle identity. Recompute must match; a different fp must not.
#[test]
fn tool_batch_cycle_observation_reuses_anti_spin_fingerprint() {
    let passed = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    let results = [("ok".into(), None, passed)];
    let call = ToolCall {
        id: "1".into(),
        name: "write_file".into(),
        args: serde_json::json!({
            "path": "src/main.rs",
            "content": "fn main() {}\n".repeat(2_000)
        }),
    };
    let calls = [call];
    let fp = anti_spin_batch_fingerprint(&calls);
    assert_eq!(
        tool_batch_cycle_observation(&calls, &results),
        tool_batch_cycle_observation_from_calls_fp(fp, &results),
        "reused anti-spin fp must match a fresh cycle hash"
    );
    assert_ne!(
        tool_batch_cycle_observation_from_calls_fp(fp, &results),
        tool_batch_cycle_observation_from_calls_fp(fp ^ 1, &results),
        "distinct call fingerprints must not collide"
    );
    let other = [("other".into(), None, passed)];
    assert_ne!(
        tool_batch_cycle_observation_from_calls_fp(fp, &results),
        tool_batch_cycle_observation_from_calls_fp(fp, &other),
        "result text must still participate"
    );
}

// --- residual run_turn guards (folded from parent) -----------------------

#[test]
fn yolo_turn_stops_on_consecutive_tool_errors() {
    let _guard = crate::tests::env_lock();
    let _operator_cap = EnvGuard::set("ANGEL_ERROR_LIMIT", "8");
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    // A club that calls read_file on a fresh missing path each hop: every call
    // errors at dispatch, but the args change, so anti-spin never fires — only
    // the error breaker should stop it.
    use std::sync::atomic::AtomicUsize;
    struct Erroring {
        n: AtomicUsize,
    }
    impl Club for Erroring {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "err"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let i = self.n.fetch_add(1, Ordering::Relaxed);
            if i >= 10 {
                // The old YOLO path disabled the error breaker and would
                // incorrectly accept this eventual answer.
                return Ok(ClubReply::Text("late answer".into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("c{i}"),
                name: "read_file".into(),
                args: serde_json::json!({ "path": format!("missing/no_{i}.txt") }),
            }]))
        }
    }
    let mut history = vec![ChatMsg::user("go")];
    let out = run_turn(
        &Erroring {
            n: AtomicUsize::new(0),
        },
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(50), // generous hop guard; the error breaker (default 8) fires first
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert!(
        out.contains("errored"),
        "error breaker should stop the thrash: {out}"
    );
    // The one-time error nudge landed in history before the stop.
    assert!(
        history
            .iter()
            .any(|m| m.role == ChatRole::Harness && m.content.contains("tool call")),
        "an error nudge should be injected once while thrashing"
    );
}

#[test]
fn run_turn_gives_up_after_max_hops() {
    let _env_guard = crate::tests::env_lock();
    let trajectory_dir = scratch("max_hops_trajectory");
    let _trajectory_log = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let _trajectory_path = EnvGuard::set(
        "ANGEL_TRAJECTORY_DIR",
        trajectory_dir.to_string_lossy().as_ref(),
    );
    struct Loopy;
    impl Club for Loopy {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "loopy"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "x".into(),
                name: "reverse".into(),
                args: serde_json::json!({ "text": "x" }),
            }]))
        }
    }
    let mut history = vec![ChatMsg::user("go")];
    let failure = run_turn_observed(
        &Loopy,
        &ToolRegistry::with_defaults(),
        &mut history,
        &AtomicBool::new(false),
        Some(3),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect_err("max-hop guard is a structured failure");
    assert_eq!(failure.stop_reason, TurnStopReason::MaxHops);
    assert_eq!(failure.hops, 3);
    assert!(failure.interrupted);
    assert!(failure.max_hops_reached);
    let call_count = history.iter().map(|m| m.tool_calls.len()).sum::<usize>();
    let result_count = history.iter().filter(|m| m.role == ChatRole::Tool).count();
    assert_eq!(call_count, 3);
    assert_eq!(result_count, call_count, "max-hop history must close calls");

    let log = std::fs::read_to_string(
        trajectory_dir.join(format!("session-{}.jsonl", std::process::id())),
    )
    .unwrap();
    // ANGEL_TRAJECTORY_LOG is process-global: unrelated parallel turn tests may
    // append to the same per-process JSONL while this wiring proof is active.
    // Select this test's structured runaway record instead of assuming it owns
    // the final line.
    let record: Value = log
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|record| {
            record["hops"] == 3
                && record["answer"]
                    .as_str()
                    .is_some_and(|answer| answer.contains("3-hop runaway guard"))
        })
        .expect("3-hop runaway trajectory record");
    assert_eq!(record["interrupted"], true);
    // Turn ledger: one entry per dispatched call, the timing split, no usage
    // (this club reports no token counters, so the delta is omitted, not zero).
    let tools = record["tools"]
        .as_array()
        .expect("trajectory record carries the per-call tool ledger");
    assert_eq!(tools.len(), 3, "{tools:?}");
    assert!(
        tools.iter().all(|t| t["tool"] == "reverse"
            && t["exec"] == "ok"
            && t["err"] == false
            && t["ms"].as_u64().is_some()),
        "{tools:?}"
    );
    assert_eq!(tools[0]["hop"], 1);
    assert_eq!(tools[2]["hop"], 3);
    assert!(
        tools.iter().all(|t| t["bytes"].as_u64().is_some()),
        "{tools:?}"
    );
    assert_eq!(record["timing"]["schema"], "angel-task-timing/v2");
    assert!(record["timing"]["background"].is_object());
    assert!(record["tools_output"]["produced_bytes"].as_u64().unwrap() > 0);
    let output = &record["tools_output"];
    assert_eq!(
        output["produced_bytes"].as_u64().unwrap(),
        ["retained_bytes", "aged_bytes", "dropped_bytes"]
            .iter()
            .map(|k| output[k].as_u64().unwrap())
            .sum::<u64>()
    );
    assert!(record["store_rotations"].is_array());
    assert_eq!(record["timing"]["tool_calls"], 3);
    assert!(record["timing"]["model_calls"].as_u64().unwrap() >= 3);
    // Usage follows the driver's accounting view: a club without counters is
    // reported as an untracked source rather than omitted.
    if let Some(usage) = record.get("usage") {
        assert!(usage.is_object(), "{usage}");
    }
    assert_eq!(record["hops"], 3);
    assert!(
        record["answer"]
            .as_str()
            .unwrap()
            .contains("3-hop runaway guard")
    );

    // The legacy API remains an error string for existing callers.
    let legacy = run_turn(
        &Loopy,
        &ToolRegistry::with_defaults(),
        &mut vec![ChatMsg::user("go")],
        &AtomicBool::new(false),
        Some(0),
        &mpsc::channel::<TurnEvent>().0,
    );
    assert!(legacy.is_err());

    let _evaluate_workspace = EnvGuard::set("ANGEL_EVALUATE_MAX_HOPS_WORKSPACE", "1");
    let mut evaluable_history = vec![ChatMsg::user("go")];
    let outcome = run_turn_observed(
        &Loopy,
        &ToolRegistry::with_defaults(),
        &mut evaluable_history,
        &AtomicBool::new(false),
        Some(3),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("evaluator opt-in must preserve a scoreable stopped workspace");
    assert_eq!(outcome.stop_reason, TurnStopReason::MaxHops);
    assert_eq!(outcome.hops, 3);
    assert!(outcome.interrupted);
    assert!(outcome.max_hops_reached);
    assert_eq!(
        evaluable_history
            .iter()
            .filter(|message| message.role == ChatRole::Tool)
            .count(),
        3,
        "evaluable horizon stops must still close every tool call"
    );
    let _ = std::fs::remove_dir_all(trajectory_dir);
}

#[test]
fn yolo_preserves_the_callers_max_hop_guard() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _spin = EnvGuard::set("ANGEL_SPIN_LIMIT", "0");

    struct EventuallyAnswers {
        calls: AtomicUsize,
    }
    impl Club for EventuallyAnswers {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "bounded-yolo"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed);
            if call >= 6 {
                return Ok(ClubReply::Text("late answer".into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("call-{call}"),
                name: "reverse".into(),
                args: serde_json::json!({ "text": call.to_string() }),
            }]))
        }
    }

    let club = EventuallyAnswers {
        calls: AtomicUsize::new(0),
    };
    let failure = run_turn_observed(
        &club,
        &ToolRegistry::with_defaults(),
        &mut vec![ChatMsg::user("go")],
        &AtomicBool::new(false),
        Some(3),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect_err("YOLO must not erase the caller's max-hop contract");

    assert_eq!(failure.stop_reason, TurnStopReason::MaxHops);
    assert_eq!(failure.hops, 3);
    assert_eq!(club.calls.load(Ordering::Relaxed), 3);
}

#[test]
fn yolo_preserves_the_explicit_turn_deadline() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _deadline = EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "37");

    assert_eq!(configured_turn_deadline_secs(), 37);
}

#[test]
fn yolo_preserves_the_anti_spin_guard() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _spin = EnvGuard::set("ANGEL_SPIN_LIMIT", "4");

    struct EventuallyAnswers {
        calls: AtomicUsize,
    }
    impl Club for EventuallyAnswers {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "spinning-yolo"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed);
            if call >= 6 {
                return Ok(ClubReply::Text("late answer".into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "same".into(),
                name: "reverse".into(),
                args: serde_json::json!({ "text": "same" }),
            }]))
        }
    }

    let club = EventuallyAnswers {
        calls: AtomicUsize::new(0),
    };
    let outcome = run_turn_observed(
        &club,
        &ToolRegistry::with_defaults(),
        &mut vec![ChatMsg::user("go")],
        &AtomicBool::new(false),
        None,
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("spin stop is a structured stopped outcome");

    assert_eq!(outcome.stop_reason, TurnStopReason::Spin);
    assert_eq!(outcome.hops, 4);
    assert!(outcome.answer.contains("same tool call repeated"));
}

#[test]
fn run_turn_soft_interrupt_stops_unbounded_loop() {
    let _guard = crate::tests::env_lock();
    // A club that loops on tool calls forever — mirrors nex2's unbounded
    // extended-reasoning style. With max_hops = None, only the soft
    // interrupt (not a cap) ends it.
    struct Loopy;
    impl Club for Loopy {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "loopy"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "x".into(),
                name: "reverse".into(),
                args: serde_json::json!({ "text": "x" }),
            }]))
        }
    }
    // Pre-tripped flag: the first hop boundary bails immediately, so the
    // unbounded loop terminates deterministically with Ok (not an error).
    let cancel = AtomicBool::new(true);
    let mut history = vec![ChatMsg::user("go")];
    let out = run_turn(
        &Loopy,
        &ToolRegistry::with_defaults(),
        &mut history,
        &cancel,
        None,
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("soft interrupt returns Ok, not an error");
    assert!(out.contains("interrupted"), "got: {out}");
}

// --- opt-in tool-call repair: scavenge + duplicate-call storm ---------------

/// Counting probe registered under an *essential* tool name so it survives the
/// bounded-task schema filter and is actually offered to the club.
struct CountingProbe {
    name: &'static str,
    calls: Arc<AtomicUsize>,
}

impl Tool for CountingProbe {
    fn name(&self) -> &str {
        self.name
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name.to_string(),
            description: "counting probe".to_string(),
            params: serde_json::json!({"type":"object"}),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("probe result".to_string())
    }
}

struct MalformedCallIdClub {
    hops: AtomicUsize,
}

impl Club for MalformedCallIdClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(String::new())
    }

    fn label(&self) -> &str {
        "malformed-call-ids"
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.hops.fetch_add(1, Ordering::SeqCst) > 0 {
            return Ok(ClubReply::Text("done".to_string()));
        }
        Ok(ClubReply::Calls(vec![
            ToolCall {
                id: String::new(),
                name: "read_file".to_string(),
                args: serde_json::json!({"case": 0}),
            },
            ToolCall {
                id: " \t".to_string(),
                name: "read_file".to_string(),
                args: serde_json::json!({"case": 1}),
            },
            ToolCall {
                id: "duplicate".to_string(),
                name: "read_file".to_string(),
                args: serde_json::json!({"case": 2}),
            },
            ToolCall {
                id: "duplicate".to_string(),
                name: "read_file".to_string(),
                args: serde_json::json!({"case": 3}),
            },
            // A valid provider ID resembling the generated namespace must be
            // preserved and force the index-zero repair onto a safe suffix.
            ToolCall {
                id: "angel_h1_call_0".to_string(),
                name: "read_file".to_string(),
                args: serde_json::json!({"case": 4}),
            },
        ]))
    }
}

#[test]
fn run_turn_repairs_blank_and_duplicate_tool_call_ids_before_dispatch() {
    let _guard = crate::tests::env_lock();
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _no_edit = EnvGuard::set("ANGEL_NO_EDIT_ANSWER_GUARD", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _storm = EnvGuard::set("ANGEL_TOOLCALL_STORM", "0");

    let executions = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingProbe {
        name: "read_file",
        calls: Arc::clone(&executions),
    }));
    let club = MalformedCallIdClub {
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("run every probe")];
    let (events, event_rx) = mpsc::channel();

    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .expect("repaired calls dispatch and the turn completes");
    assert_eq!(answer, "done");
    assert_eq!(
        executions.load(Ordering::SeqCst),
        5,
        "every provider call must execute exactly once"
    );

    let call_ids = history
        .iter()
        .find(|message| !message.tool_calls.is_empty())
        .expect("assistant tool-call message")
        .tool_calls
        .iter()
        .map(|call| call.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        call_ids,
        vec![
            "angel_h1_call_0_1",
            "angel_h1_call_1",
            "angel_h1_call_2",
            "angel_h1_call_3",
            "angel_h1_call_0",
        ],
        "repair is deterministic and preserves the valid collision-like provider ID"
    );
    let unique = call_ids.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), call_ids.len());
    assert!(call_ids.iter().all(|id| !id.trim().is_empty()));

    // Exercise the completed-call compaction walk: its pending-call map is
    // keyed by these IDs, so duplicate normalization must survive this seam.
    let _ = shrink_completed_tool_arguments(&mut history, 0, 1);
    let result_ids = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .map(|message| {
            message
                .tool_call_id
                .clone()
                .expect("every tool result is paired")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        result_ids, call_ids,
        "tool results preserve call order and IDs"
    );
    let notices = event_rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(text) => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        notices
            .iter()
            .any(|notice| notice.contains("normalized 4/5")),
        "one bounded repair notice must speak: {notices:?}"
    );
}

/// A model that finishes a hop with an EMPTY structured `tool_calls` array while
/// the call itself sits in the answer text (the DeepSeek-reasoner failure mode).
struct StrandedCall {
    text: &'static str,
    hops: AtomicUsize,
}

impl Club for StrandedCall {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn label(&self) -> &str {
        "stranded"
    }
    fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
        if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(ClubReply::Text(self.text.to_string()))
        } else {
            Ok(ClubReply::Text("done".to_string()))
        }
    }
}

/// Returns (probe executions, final answer, notices).
fn run_stranded_turn(text: &'static str) -> (usize, String, Vec<String>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingProbe {
        name: "read_file",
        calls: Arc::clone(&calls),
    }));
    let (events, event_rx) = mpsc::channel();
    let answer = run_turn(
        &StrandedCall {
            text,
            hops: AtomicUsize::new(0),
        },
        &registry,
        &mut vec![ChatMsg::user("what does the manifest say?")],
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();
    let notices = event_rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(text) => Some(text),
            _ => None,
        })
        .collect();
    (calls.load(Ordering::SeqCst), answer, notices)
}

const STRANDED_READ_CALL: &str = r#"{"name": "read_file", "arguments": {"path": "Cargo.toml"}}"#;

#[test]
fn stranded_tool_call_stays_prose_until_scavenge_is_armed() {
    let _guard = crate::tests::env_lock();
    let _off = EnvGuard::unset("ANGEL_TOOLCALL_SCAVENGE");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let (executions, answer, notices) = run_stranded_turn(STRANDED_READ_CALL);
    assert_eq!(executions, 0, "nothing may dispatch with the knob unset");
    assert_eq!(answer, STRANDED_READ_CALL, "the text stays the answer");
    assert!(!notices.iter().any(|text| text.starts_with("scavenge:")));
}

#[test]
fn armed_scavenge_recovers_a_stranded_call_and_speaks() {
    let _guard = crate::tests::env_lock();
    let _armed = EnvGuard::set("ANGEL_TOOLCALL_SCAVENGE", "1");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let (executions, answer, notices) = run_stranded_turn(STRANDED_READ_CALL);
    assert_eq!(executions, 1, "the recovered call really dispatches");
    assert_eq!(answer, "done");
    assert!(
        notices
            .iter()
            .any(|text| text == "scavenge: recovered read_file call from prose"),
        "gate stayed silent: {notices:?}"
    );
}

#[test]
fn armed_scavenge_leaves_an_unoffered_tool_name_as_prose() {
    let _guard = crate::tests::env_lock();
    let _armed = EnvGuard::set("ANGEL_TOOLCALL_SCAVENGE", "1");
    // Parallel suite can leave skill/verify/first-write governors armed; those
    // force an extra hop so the scripted club's second "done" becomes the
    // answer instead of the stranded prose.
    let _max_hops_env = EnvGuard::unset("ANGEL_MAX_HOPS");
    let _spin = EnvGuard::unset("ANGEL_SPIN_LIMIT");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");
    let text = r#"{"name": "rm_rf", "arguments": {"path": "/"}}"#;
    let (executions, answer, notices) = run_stranded_turn(text);
    assert_eq!(executions, 0, "unoffered tool must never dispatch");
    assert_eq!(
        answer, text,
        "unoffered stranded markup stays the final prose answer (not a follow-up hop)"
    );
    assert!(!notices.iter().any(|text| text.starts_with("scavenge:")));
}

/// Re-issues one `grep` call: the same call three times (the second with its
/// argument keys reordered), then a different one, then the answer.
struct StormRepeater {
    hops: AtomicUsize,
}

impl Club for StormRepeater {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn label(&self) -> &str {
        "storm-repeater"
    }
    fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
        let hop = self.hops.fetch_add(1, Ordering::SeqCst);
        let args = match hop {
            0 | 2 => serde_json::json!({"pattern":"x","path":"src"}),
            1 => serde_json::json!({"path":"src","pattern":"x"}),
            3 => serde_json::json!({"pattern":"y","path":"src"}),
            _ => return Ok(ClubReply::Text("done".to_string())),
        };
        Ok(ClubReply::Calls(vec![ToolCall {
            id: format!("g{hop}"),
            name: "grep".into(),
            args,
        }]))
    }
}

/// Returns (probe executions, history, emitted events).
fn run_storm_turn() -> (usize, Vec<ChatMsg>, Vec<TurnEvent>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingProbe {
        name: "grep",
        calls: Arc::clone(&calls),
    }));
    let mut history = vec![ChatMsg::user("find the pattern")];
    let (events, event_rx) = mpsc::channel();
    let answer = run_turn(
        &StormRepeater {
            hops: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &events,
    )
    .unwrap();
    assert_eq!(answer, "done");
    (
        calls.load(Ordering::SeqCst),
        history,
        event_rx.try_iter().collect(),
    )
}

#[test]
fn duplicate_calls_all_dispatch_until_the_storm_guard_is_armed() {
    let _guard = crate::tests::env_lock();
    let _off = EnvGuard::unset("ANGEL_TOOLCALL_STORM");
    let (executions, history, events) = run_storm_turn();
    assert_eq!(
        executions, 4,
        "every duplicate still runs with the knob unset"
    );
    assert!(
        !history
            .iter()
            .any(|message| message.content.contains("duplicate call suppressed"))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, TurnEvent::Notice(text) if text.starts_with("storm:")))
    );
}

#[test]
fn armed_storm_guard_suppresses_the_third_identical_call_and_speaks() {
    let _guard = crate::tests::env_lock();
    let _armed = EnvGuard::set("ANGEL_TOOLCALL_STORM", "1");
    let _window = EnvGuard::set("ANGEL_TOOLCALL_STORM_WINDOW", "6");
    let (executions, history, events) = run_storm_turn();
    assert_eq!(
        executions, 3,
        "the third identical call never reaches the tool; the different one does"
    );
    let suppressed: Vec<&ChatMsg> = history
        .iter()
        .filter(|message| {
            message.role == ChatRole::Tool && message.content.contains("duplicate call suppressed")
        })
        .collect();
    assert_eq!(suppressed.len(), 1, "exactly one call is answered in place");
    assert!(
        suppressed[0]
            .content
            .contains("issued this exact `grep` call 3 times"),
        "reflection missing: {}",
        suppressed[0].content
    );
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::Notice(text) if text == "storm: suppressed duplicate grep call (x3)"
    )));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                TurnEvent::ToolCall { id, .. } if id.0 == "g2"
            ))
            .count(),
        1,
        "the authored suppressed call must remain paired"
    );
    let outcomes = events
        .iter()
        .filter_map(|event| match event {
            TurnEvent::ToolResult { id, outcome, .. } if id.0 == "g2" => Some(outcome),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(outcomes, vec![&ToolOutcome::not_started()]);
    assert!(!outcomes[0].attributable_success());
}

/// Window-threshold storms re-issue the same notice text every hop. Speak it
/// once; later identical suppressions stay silent in the operator stream.
#[test]
fn identical_storm_notices_are_spoken_once() {
    let _guard = crate::tests::env_lock();
    let _armed = EnvGuard::set("ANGEL_TOOLCALL_STORM", "1");
    let _window = EnvGuard::set("ANGEL_TOOLCALL_STORM_WINDOW", "3");
    let _spin = EnvGuard::set("ANGEL_SPIN_LIMIT", "0");
    let _cycle = EnvGuard::set("ANGEL_TOOL_CYCLE_REPEATS", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CountingProbe {
        name: "shell",
        calls: Arc::clone(&calls),
    }));
    struct Repeater {
        hops: AtomicUsize,
    }
    impl Club for Repeater {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "storm-notice-repeater"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.hops.fetch_add(1, Ordering::SeqCst);
            if hop >= 8 {
                return Ok(ClubReply::Text("done".into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("s{hop}"),
                name: "shell".into(),
                args: serde_json::json!({"command": "pwd"}),
            }]))
        }
    }
    let mut history = vec![ChatMsg::user("keep checking")];
    let (events, event_rx) = mpsc::channel();
    let answer = run_turn(
        &Repeater {
            hops: AtomicUsize::new(0),
        },
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(12),
        &events,
    )
    .unwrap();
    assert_eq!(answer, "done");
    let notices: Vec<String> = event_rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(text) if text.starts_with("storm:") => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(
        notices.len(),
        1,
        "same-tool suppressions must not restack: {notices:?}"
    );
    assert_eq!(
        notices[0], "storm: suppressed duplicate shell call (x3)",
        "first crossing still names the threshold"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "only the first two identical shells should execute"
    );
}

#[test]
fn storm_signatures_ignore_key_order_and_age_out_of_the_window() {
    let call = |args| ToolCall {
        id: "x".into(),
        name: "grep".into(),
        args,
    };
    let a = call(serde_json::json!({"pattern":"x","path":"src"}));
    let reordered = call(serde_json::json!({"path":"src","pattern":"x"}));
    let other = call(serde_json::json!({"pattern":"y","path":"src"}));
    assert_eq!(
        toolcall_storm_signature(&a),
        toolcall_storm_signature(&reordered)
    );
    assert_ne!(
        toolcall_storm_signature(&a),
        toolcall_storm_signature(&other)
    );

    let mut storm = ToolCallStorm::new(2);
    assert_eq!(storm.observe(std::slice::from_ref(&a)), vec![1]);
    assert_eq!(storm.observe(std::slice::from_ref(&reordered)), vec![2]);
    assert_eq!(storm.observe(std::slice::from_ref(&other)), vec![1]);
    assert_eq!(storm.observe(std::slice::from_ref(&other)), vec![2]);
    // Two hops of something else pushed the earlier sightings out of the window.
    assert_eq!(storm.observe(std::slice::from_ref(&a)), vec![1]);
    // Duplicates inside one batch count against each other too.
    assert_eq!(storm.observe(&[a.clone(), a.clone()]), vec![2, 3]);
}

// --- needs-pro: model self-report escalation --------------------------------

/// Answers with a fixed text on every hop and keeps every request it was
/// handed, so a test can assert what the tier ladder put in front of each seat.
struct TierClub {
    label: &'static str,
    answer: &'static str,
    requests: Mutex<Vec<Vec<ChatMsg>>>,
}

impl TierClub {
    fn new(label: &'static str, answer: &'static str) -> Self {
        Self {
            label,
            answer,
            requests: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    fn last_request(&self) -> Vec<ChatMsg> {
        self.requests
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

impl Club for TierClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        self.label
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.requests.lock().unwrap().push(messages.to_vec());
        Ok(ClubReply::Text(self.answer.to_string()))
    }
}

/// One bounded turn served by `fast`, with `roster` behind the registry so a
/// named escalation seat can resolve. Returns (answer, history, notices).
fn needs_pro_turn(
    fast: &TierClub,
    roster: Vec<Arc<dyn Club>>,
) -> (String, Vec<ChatMsg>, Vec<String>) {
    let workspace = scratch("needs_pro");
    let reg = ToolRegistry::with_team(workspace.clone(), roster);
    let mut history = vec![
        ChatMsg::system("bootstrap preamble"),
        ChatMsg::user("do the hard thing"),
    ];
    let (events, rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        fast,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();
    let notices = rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(note) => Some(note),
            _ => None,
        })
        .collect();
    let _ = std::fs::remove_dir_all(workspace);
    (answer, history, notices)
}

fn contract_installed(history: &[ChatMsg]) -> bool {
    history
        .iter()
        .any(|message| message.content.starts_with("[tier contract]"))
}

fn assistant_marker_present(messages: &[ChatMsg]) -> bool {
    messages
        .iter()
        .filter(|message| message.role == ChatRole::Assistant)
        .any(|message| message.content.contains(NEEDS_PRO_OPEN))
}

#[test]
fn needs_pro_unarmed_treats_the_marker_as_ordinary_answer_text() {
    let _guard = crate::tests::env_lock();
    let _armed = EnvGuard::unset("ANGEL_NEEDS_PRO");
    let _seat = EnvGuard::unset("ANGEL_NEEDS_PRO_SEAT");
    let fast = TierClub::new("fast-seat", "<<<NEEDS_PRO: too hard>>>");
    let pro = Arc::new(TierClub::new("pro-seat", "pro answer"));
    let (answer, history, notices) = needs_pro_turn(&fast, vec![pro.clone()]);
    assert_eq!(
        answer, "<<<NEEDS_PRO: too hard>>>",
        "unset: the marker is answer text like any other"
    );
    assert_eq!(fast.calls(), 1);
    assert_eq!(pro.calls(), 0, "no seat is called without the knob");
    assert!(!contract_installed(&history), "no contract when unarmed");
    assert!(
        !notices.iter().any(|note| note.starts_with("needs-pro:")),
        "an unarmed feature says nothing: {notices:?}"
    );
}

#[test]
fn needs_pro_armed_without_a_usable_seat_speaks_and_installs_no_contract() {
    let _guard = crate::tests::env_lock();
    let _armed = EnvGuard::set("ANGEL_NEEDS_PRO", "1");
    let marker = "<<<NEEDS_PRO: too hard>>>";

    let _unset = EnvGuard::unset("ANGEL_NEEDS_PRO_SEAT");
    let fast = TierClub::new("fast-seat", marker);
    let pro = Arc::new(TierClub::new("pro-seat", "pro answer"));
    let (answer, history, notices) = needs_pro_turn(&fast, vec![pro.clone()]);
    assert_eq!(answer, marker, "no seat, no escalation: the marker is text");
    assert_eq!(pro.calls(), 0);
    assert!(!contract_installed(&history), "a lie is not injected");
    assert!(
        notices.iter().any(|note| note
            == "needs-pro: armed but no ANGEL_NEEDS_PRO_SEAT — self-escalation disabled"),
        "silent gating reads as nonexistence: {notices:?}"
    );

    let _named = EnvGuard::set("ANGEL_NEEDS_PRO_SEAT", "nobody-here");
    let fast = TierClub::new("fast-seat", marker);
    let pro = Arc::new(TierClub::new("pro-seat", "pro answer"));
    let (answer, history, notices) = needs_pro_turn(&fast, vec![pro.clone()]);
    assert_eq!(answer, marker);
    assert_eq!(pro.calls(), 0);
    assert!(!contract_installed(&history));
    assert!(
        notices.iter().any(|note| note
            == "needs-pro: armed but ANGEL_NEEDS_PRO_SEAT=nobody-here matches no club in the \
                roster — self-escalation disabled"),
        "an unresolvable seat names itself: {notices:?}"
    );
}

#[test]
fn needs_pro_marker_escalates_once_to_the_named_seat_and_says_why() {
    let _guard = crate::tests::env_lock();
    let _armed = EnvGuard::set("ANGEL_NEEDS_PRO", "1");
    let _seat = EnvGuard::set("ANGEL_NEEDS_PRO_SEAT", "pro-seat");
    let fast = TierClub::new("fast-seat", "<<<NEEDS_PRO: needs a deeper proof>>>\n");
    let pro = Arc::new(TierClub::new("pro-seat", "pro answer"));
    let (answer, history, notices) = needs_pro_turn(&fast, vec![pro.clone()]);

    assert_eq!(answer, "pro answer");
    assert_eq!(fast.calls(), 1, "exactly one attempt on the fast seat");
    assert_eq!(pro.calls(), 1, "exactly one escalation");
    assert!(
        notices
            .iter()
            .any(|note| note == "needs-pro: escalating to pro-seat — needs a deeper proof"),
        "the escalation and its rationale must be visible: {notices:?}"
    );

    let fast_request = fast.last_request();
    assert!(
        fast_request
            .iter()
            .any(|m| m.role == ChatRole::System && m.content.as_ref() == FAST_TIER_CONTRACT),
        "the fast seat is told it may escalate"
    );
    let pro_request = pro.last_request();
    assert!(
        pro_request
            .iter()
            .any(|m| m.role == ChatRole::System && m.content.as_ref() == PRO_TIER_CONTRACT),
        "the escalation seat gets the no-op contract"
    );
    assert!(
        !pro_request
            .iter()
            .any(|m| m.content.as_ref() == FAST_TIER_CONTRACT),
        "the contract is replaced, never stacked"
    );
    assert!(
        !assistant_marker_present(&pro_request),
        "nothing about the aborted attempt is re-sent upstream"
    );
    assert!(
        !assistant_marker_present(&history),
        "the marker response is discarded from history"
    );
}

#[test]
fn needs_pro_marker_on_the_escalation_seat_is_stripped_not_escalated_again() {
    let _guard = crate::tests::env_lock();
    let _armed = EnvGuard::set("ANGEL_NEEDS_PRO", "1");
    let _seat = EnvGuard::set("ANGEL_NEEDS_PRO_SEAT", "pro-seat");
    let fast = TierClub::new("fast-seat", "<<<NEEDS_PRO>>>");
    let pro = Arc::new(TierClub::new(
        "pro-seat",
        "<<<NEEDS_PRO: still hard>>>\nbut here is the answer",
    ));
    let (answer, _history, notices) = needs_pro_turn(&fast, vec![pro.clone()]);

    assert_eq!(answer, "but here is the answer", "the rest still lands");
    assert_eq!(pro.calls(), 1, "the ladder cannot cycle");
    assert!(
        notices
            .iter()
            .any(|note| note == "needs-pro: escalating to pro-seat — no reason given"),
        "a bare marker escalates without inventing a rationale: {notices:?}"
    );
    assert!(
        notices
            .iter()
            .any(|note| note == "needs-pro: already on pro-seat, marker ignored"),
        "the ignored marker is announced: {notices:?}"
    );
}

#[test]
fn needs_pro_marker_reads_only_as_a_leading_control_line() {
    use NeedsProTier::{Fast, Off, Pro};
    assert_eq!(
        read_self_report("<<<NEEDS_PRO>>>", Fast),
        SelfReport::Escalate(None)
    );
    assert_eq!(
        read_self_report("\n  <<<NEEDS_PRO>>>   \n", Fast),
        SelfReport::Escalate(None),
        "surrounding whitespace is tolerated"
    );
    assert_eq!(
        read_self_report("<<<NEEDS_PRO: needs deeper reasoning>>>", Fast),
        SelfReport::Escalate(Some("needs deeper reasoning".to_string()))
    );
    assert_eq!(
        read_self_report("<<<NEEDS_PRO>>>", Off),
        SelfReport::None,
        "the marker means nothing to an unarmed turn"
    );
    assert_eq!(
        read_self_report("You can emit <<<NEEDS_PRO>>> to escalate.", Fast),
        SelfReport::None,
        "a reply explaining the protocol is prose"
    );
    assert_eq!(
        read_self_report("<<<NEEDS_PRO>>> and then I continued anyway", Fast),
        SelfReport::None,
        "the marker must be the whole first line"
    );
    assert_eq!(read_self_report("<<<NEEDS_PROX>>>", Fast), SelfReport::None);
    assert_eq!(
        read_self_report("<<<NEEDS_PRO: still hard>>>\nrest of it", Pro),
        SelfReport::Ignored("rest of it".to_string())
    );
}

#[test]
fn sustained_reasoning_preserves_selected_effort_unless_operator_sets_a_budget() {
    let _guard = crate::tests::env_lock();
    let _adaptive = EnvGuard::unset("ANGEL_ADAPTIVE_REASONING");
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "full");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");

    struct DeepThinkingClub {
        hops: AtomicUsize,
        efforts: Mutex<Vec<String>>,
        levels: Vec<String>,
    }
    impl Club for DeepThinkingClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            unreachable!("fixture uses tool chat")
        }
        fn label(&self) -> &str {
            "glm-reasoning-test"
        }
        fn model_identity(&self) -> Option<String> {
            Some("glm-5.3".into())
        }
        fn reasoning_levels(&self) -> &[String] {
            &self.levels
        }
        fn reasoning_effort(&self) -> Option<String> {
            Some("high".into())
        }
        fn bind_run_identity(&self, effort: Option<&str>) -> Result<(), String> {
            self.efforts
                .lock()
                .unwrap()
                .push(effort.unwrap_or("high").to_string());
            Ok(())
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "thinking-probe".into(),
                    name: "reverse".into(),
                    args: serde_json::json!({"text":"abc"}),
                }]))
            } else {
                Ok(ClubReply::Text("done".into()))
            }
        }
        fn token_usage(&self) -> Option<crate::agent::club::TokenUsage> {
            let turns = self.hops.load(Ordering::SeqCst) as u64;
            Some(crate::agent::club::TokenUsage {
                turns,
                last_reasoning: if turns > 0 { 40_000 } else { 0 },
                total_reasoning: turns * 40_000,
                ..Default::default()
            })
        }
    }

    for (budget, expected) in [(None, ["high", "high"]), (Some("32768"), ["high", "low"])] {
        let _budget = match budget {
            Some(value) => EnvGuard::set("ANGEL_GLM_THINKING_BURN", value),
            None => EnvGuard::unset("ANGEL_GLM_THINKING_BURN"),
        };
        let club = DeepThinkingClub {
            hops: AtomicUsize::new(0),
            efforts: Mutex::new(Vec::new()),
            levels: vec!["low".into(), "high".into()],
        };
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(ReverseTool));
        let answer = run_turn(
            &club,
            &registry,
            &mut vec![ChatMsg::user("Use the reverse tool, then answer.")],
            &AtomicBool::new(false),
            Some(8),
            &mpsc::channel::<TurnEvent>().0,
        )
        .unwrap();
        assert_eq!(answer, "done");
        assert_eq!(*club.efforts.lock().unwrap(), expected, "budget={budget:?}");
    }
}

#[test]
fn adaptive_reasoning_effort_downgrades_on_clean_execution_and_escalates_on_friction() {
    use crate::agent::club::Club;
    use crate::agent::harness::turn::resolve_adaptive_reasoning_effort;

    struct MockReasoningClub {
        levels: Vec<String>,
        effort: Option<String>,
    }
    impl Club for MockReasoningClub {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "mock"
        }
        fn reasoning_levels(&self) -> &[String] {
            &self.levels
        }
        fn reasoning_effort(&self) -> Option<String> {
            self.effort.clone()
        }
    }

    // 1. OpenAI-style levels: ["none", "low", "medium", "high"]
    let openai = MockReasoningClub {
        levels: vec!["none".into(), "low".into(), "medium".into(), "high".into()],
        effort: Some("high".into()),
    };
    assert_eq!(
        resolve_adaptive_reasoning_effort(&openai, 0, false).as_deref(),
        Some("high"),
        "hop 0 intake uses high planning effort"
    );
    assert_eq!(
        resolve_adaptive_reasoning_effort(&openai, 1, false).as_deref(),
        Some("low"),
        "hop > 0 without friction downgrades to low effort"
    );
    assert_eq!(
        resolve_adaptive_reasoning_effort(&openai, 2, true).as_deref(),
        Some("high"),
        "friction/error escalates back to high effort"
    );

    // 2. GLM/Qwen style levels: ["none", "high"]
    let glm = MockReasoningClub {
        levels: vec!["none".into(), "high".into()],
        effort: Some("high".into()),
    };
    assert_eq!(
        resolve_adaptive_reasoning_effort(&glm, 0, false).as_deref(),
        Some("high")
    );
    assert_eq!(
        resolve_adaptive_reasoning_effort(&glm, 1, false).as_deref(),
        Some("none"),
        "binary thinking models disable thinking on clean tool hops"
    );
    assert_eq!(
        resolve_adaptive_reasoning_effort(&glm, 2, true).as_deref(),
        Some("high"),
        "binary thinking models re-enable thinking on errors"
    );

    // 3. Non-reasoning model: []
    let non_reasoning = MockReasoningClub {
        levels: vec![],
        effort: None,
    };
    assert_eq!(
        resolve_adaptive_reasoning_effort(&non_reasoning, 0, false),
        None
    );
    assert_eq!(
        resolve_adaptive_reasoning_effort(&non_reasoning, 1, false),
        None
    );
}

#[test]
fn provider_recovery_actual_http_eof_preserves_partial_without_answer_or_text_action() {
    provider_death_fixture(
        "data: {\"choices\":[{\"delta\":{\"content\":\"Useful partial provider work\"}}]}\n\n",
        false,
        false,
    );
}

#[test]
fn provider_recovery_empty_success_reply_is_retried_and_fails() {
    let _guard = crate::tests::env_lock();
    let _retry = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "1");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    struct EmptyProvider(AtomicUsize);
    impl Club for EmptyProvider {
        fn label(&self) -> &str {
            // Exercise the ordinary cloud retry policy; unknown labels route
            // through the separate local-seat teacher-watch recovery policy.
            "glm"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ClubReply::Text(" \n\t".to_string()))
        }
    }
    let club = EmptyProvider(AtomicUsize::new(0));
    let failure = run_turn_observed(
        &club,
        &ToolRegistry::new(),
        &mut vec![ChatMsg::user("hello")],
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect_err("blank success is a provider failure");
    assert_eq!(failure.stop_reason, TurnStopReason::ProviderError);
    assert_eq!(club.0.load(Ordering::SeqCst), 2);
    assert!(
        trajectory::progress_ledger_snapshot()["escalations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "provider_unavailable")
    );
}

#[test]
fn provider_recovery_actual_http_death_before_frame() {
    provider_death_fixture("", false, false);
}

#[test]
fn provider_recovery_actual_http_death_after_tool_frame() {
    provider_death_fixture(
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"cut-tool\",\"type\":\"function\",\"function\":{\"name\":\"shell\",\"arguments\":\"{}\"}}]}}]}\n\n",
        false,
        false,
    );
}

#[test]
fn provider_recovery_actual_http_empty_terminal_is_not_answer() {
    provider_death_fixture("data: [DONE]\n\n", false, false);
}

#[test]
fn provider_recovery_actual_http_genuine_finish_reason() {
    provider_death_fixture(
        "data: {\"choices\":[{\"delta\":{\"content\":\"recovered\"},\"finish_reason\":\"stop\"}]}\n\n",
        true,
        false,
    );
}

#[test]
fn provider_recovery_actual_http_genuine_done() {
    provider_death_fixture(
        "data: {\"choices\":[{\"delta\":{\"content\":\"recovered\"}}]}\n\ndata: [DONE]\n\n",
        true,
        false,
    );
}

#[test]
fn provider_recovery_actual_http_partial_retry_recovers() {
    provider_death_fixture(
        "data: {\"choices\":[{\"delta\":{\"content\":\"discard me\"}}]}\n\n",
        false,
        true,
    );
}

#[test]
fn provider_recovery_configured_failover_is_named_in_envelope() {
    let _lock = crate::tests::env_lock();
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    struct Primary(AtomicUsize);
    impl Club for Primary {
        fn label(&self) -> &str {
            "glm"
        }
        fn model_identity(&self) -> Option<String> {
            Some("primary-model".into())
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("connection refused".into())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(ClubReply::Calls(vec![tc(
                    "read_file",
                    serde_json::json!({"path":"fixture.txt"}),
                )]))
            } else {
                Err("connection refused".into())
            }
        }
    }
    struct Secondary;
    impl Club for Secondary {
        fn label(&self) -> &str {
            "secondary"
        }
        fn model_identity(&self) -> Option<String> {
            Some("secondary-model".into())
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok("configured recovery".into())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(ClubReply::Text("configured recovery".into()))
        }
    }
    let root = scratch("provider_recovery_route_switch");
    std::fs::write(root.join("fixture.txt"), "owned fixture\n").unwrap();
    let mut registry = ToolRegistry::with_defaults();
    registry.set_workspace(root.clone());
    let primary: Arc<dyn Club> = Arc::new(Primary(AtomicUsize::new(0)));
    primary.bind_run_identity(None).unwrap();
    let club = crate::agent::club::FallbackClub::new(vec![primary, Arc::new(Secondary)]);
    let mut history = vec![ChatMsg::user("Read the fixture, then answer")];
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(outcome.answer, "configured recovery");
    let envelope = TaskJsonEnvelope::from_outcome(
        TaskJsonContext {
            task_id: None,
            run_id: None,
            workspace: root.clone(),
            club: Some("glm".into()),
            model: Some("primary-model".into()),
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 0,
            timing: None,
            tools: Vec::new(),
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        outcome,
        &history,
    );
    let json = serde_json::to_value(envelope).unwrap();
    assert_eq!(json["identity"]["model"]["id"], "secondary-model");
    assert_eq!(json["model"], "secondary-model");
    let switches: Vec<_> = json["escalations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["kind"] == "route_switch")
        .collect();
    assert_eq!(switches.len(), 1);
    assert_eq!(switches[0]["hop"], 2);
    assert_eq!(switches[0]["from"]["model"], "primary-model");
    assert_eq!(switches[0]["to"]["model"], "secondary-model");
    let _ = std::fs::remove_dir_all(root);
}

#[derive(Clone, Copy)]
enum PostHopDeath {
    ConnectionRefused,
    EofBeforeByte,
    EofAfterHeaders,
    Empty200,
}

#[test]
fn provider_recovery_after_tool_local_connection_refused() {
    provider_death_fixture_with_transport(
        "local",
        false,
        false,
        Some(PostHopDeath::ConnectionRefused),
    );
}

#[test]
fn provider_recovery_after_tool_connection_refused() {
    provider_death_fixture_with_transport("", false, false, Some(PostHopDeath::ConnectionRefused));
}

#[test]
fn provider_recovery_after_tool_eof_before_byte() {
    provider_death_fixture_with_transport("", false, false, Some(PostHopDeath::EofBeforeByte));
}

#[test]
fn provider_recovery_after_tool_eof_after_headers() {
    provider_death_fixture_with_transport("", false, false, Some(PostHopDeath::EofAfterHeaders));
}

#[test]
fn provider_recovery_after_tool_empty_200() {
    provider_death_fixture_with_transport("", false, false, Some(PostHopDeath::Empty200));
}

fn provider_death_fixture(body: &str, succeeds: bool, recover: bool) {
    provider_death_fixture_with_transport(body, succeeds, recover, None);
}

fn provider_death_fixture_with_transport(
    body: &str,
    succeeds: bool,
    recover: bool,
    death: Option<PostHopDeath>,
) {
    use std::io::{Read, Write};
    let _lock = crate::tests::env_lock();
    let local = body == "local";
    let _watch = EnvGuard::set("ANGEL_TEACHER_WATCH", "1");
    let _ask = EnvGuard::set("ANGEL_TEACHER_WATCH_ASK", "0");
    let root = scratch("provider_recovery_partial");
    let store = root.join("rollouts");
    let trajectory_dir = root.join("trajectory");
    let _trajectory = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let _trajectory_dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", trajectory_dir.to_str().unwrap());
    let _mode = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    let _store = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_DIR", store.to_str().unwrap());
    let _required = EnvGuard::set("ANGEL_HARNESS_ROLLOUT_REQUIRED", "1");
    let _retry = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "2");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _http_retry = EnvGuard::set("ANGEL_HTTP_RETRIES", "0");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let server_hits = Arc::clone(&hits);
    let stop = Arc::new(AtomicBool::new(false));
    struct StopFixtureOnDrop(Arc<AtomicBool>);
    impl Drop for StopFixtureOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let _stop_fixture = StopFixtureOnDrop(Arc::clone(&stop));
    let server_stop = Arc::clone(&stop);
    let body = body.to_owned();
    let server = std::thread::spawn(move || {
        // Keep the selected EOF shape alive until the owned turn finishes.
        // Expiring mid-retry silently changes EOF into connection refusal.
        while !server_stop.load(Ordering::SeqCst) {
            let mut socket = match listener.accept() {
                Ok((socket, _)) => socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => panic!("owned accept: {error}"),
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut bytes = [0u8; 4096];
                let count = socket.read(&mut bytes).unwrap();
                assert!(count > 0 && request.len() + count < 512 * 1024);
                request.extend_from_slice(&bytes[..count]);
                if let Some(end) = request.windows(4).position(|s| s == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                    let len: usize = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse().ok())
                        .unwrap_or(0);
                    if request.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            if request.starts_with(b"GET ") {
                // The real HttpClub resolves local context/capabilities before
                // its chat request. These metadata probes have no request body.
                let body =
                    r#"{"n_ctx":131072,"data":[{"id":"glm-5.3-flash","context_length":131072}]}"#;
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                continue;
            }
            assert!(request.starts_with(b"POST "));
            let attempt = server_hits.fetch_add(1, Ordering::SeqCst);
            if let Some(shape) = death {
                if attempt == 0 {
                    let body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"read-first\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\\\"fixture.txt\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";
                    if matches!(shape, PostHopDeath::ConnectionRefused) {
                        // Close admission before the client sees the successful
                        // tool reply; the next request must be refused.
                        drop(listener);
                        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                        return;
                    }
                    write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}").unwrap();
                } else {
                    match shape {
                        PostHopDeath::ConnectionRefused => unreachable!(),
                        PostHopDeath::EofBeforeByte => {}
                        PostHopDeath::EofAfterHeaders => {
                            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n").unwrap();
                        }
                        PostHopDeath::Empty200 => {
                            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                        }
                    }
                }
                continue;
            }
            let body = if recover && attempt > 0 {
                "data: {\"choices\":[{\"delta\":{\"content\":\"recovered\"},\"finish_reason\":\"stop\"}]}\n\n"
            } else {
                body.as_str()
            };
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}").unwrap();
        }
    });
    let club = crate::agent::club::HttpClub::new(
        if local { "r01-scripted" } else { "glm" },
        format!("http://{address}"),
        "glm-5.3-flash",
        None,
    );
    let mut registry = ToolRegistry::with_defaults();
    registry.set_workspace(root.join("work"));
    std::fs::create_dir_all(registry.current_workspace()).unwrap();
    std::fs::write(
        registry.current_workspace().join("fixture.txt"),
        "owned fixture\n",
    )
    .unwrap();
    let mut history = vec![ChatMsg::user("Give the owned partial response")];
    let (events, received) = mpsc::channel();
    let result = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
    );
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    let rollout_id = if succeeds || recover {
        let outcome = result.expect("terminal answer remains successful");
        assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
        assert_eq!(outcome.answer, "recovered");
        assert_eq!(hits.load(Ordering::SeqCst), if recover { 2 } else { 1 });
        if recover {
            assert_eq!(
                trajectory::progress_ledger_snapshot()["hop_stream_cuts"],
                json!([{"hop":1,"stream_cut":1}])
            );
            assert!(
                history
                    .iter()
                    .filter(|m| m.role == ChatRole::Assistant)
                    .all(|m| m.content.as_ref() == "recovered")
            );
        }
        assert!(
            trajectory::progress_ledger_snapshot()["escalations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        outcome.rollout_id.unwrap()
    } else {
        let failure = result.expect_err("provider death must never return an answer");
        assert_eq!(failure.stop_reason, TurnStopReason::ProviderError);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            match death {
                Some(PostHopDeath::ConnectionRefused) => 1,
                Some(_) => 4,
                None => 3,
            },
            "successful hop (if any), then three failed attempts; refused connections never reach the server"
        );
        assert_eq!(
            history
                .iter()
                .filter(|m| m.role == ChatRole::Assistant)
                .count(),
            usize::from(death.is_some()),
            "only the completed tool-call reply survives"
        );
        assert!(
            history
                .iter()
                .filter(|m| m.role == ChatRole::Assistant)
                .all(|m| m.content.is_empty())
        );
        assert_eq!(failure.hops, if death.is_some() { 2 } else { 1 });
        assert!(
            trajectory::progress_ledger_snapshot()["escalations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "provider_unavailable")
        );
        let rollout_id = failure.rollout_id.clone().unwrap();
        let envelope = TaskJsonEnvelope::from_failure(
            TaskJsonContext {
                task_id: None,
                run_id: None,
                workspace: registry.current_workspace().to_path_buf(),
                club: None,
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 0,
                timing: None,
                tools: Vec::new(),
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
            },
            failure,
        );
        let json = serde_json::to_value(envelope).unwrap();
        assert_eq!(json["status"], "error");
        assert!(json.get("answer").is_none());
        assert_eq!(json["stop_reason"], "provider_error");
        assert_eq!(
            json.get("hop_stream_cuts"),
            trajectory::progress_ledger_snapshot()
                .get("hop_stream_cuts")
                .filter(|v| v.as_array().is_some_and(|a| !a.is_empty()))
        );
        assert!(
            json["escalations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "provider_unavailable")
        );
        let log = std::fs::read_to_string(
            trajectory_dir.join(format!("session-{}.jsonl", std::process::id())),
        )
        .unwrap();
        let records: Vec<Value> = log
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(records.iter().any(|record| {
            record["escalations"]
                .as_array()
                .is_some_and(|entries| entries.iter().any(|e| e["kind"] == "provider_unavailable"))
        }));
        rollout_id
    };
    assert_eq!(
        received
            .try_iter()
            .filter(|e| matches!(e, TurnEvent::ToolCall { .. }))
            .count(),
        usize::from(death.is_some()),
        "only the completed first hop may execute a tool"
    );
    let repo = crate::platform::workspace_store::repo_identity(registry.current_workspace()).key;
    let manifest_path = store
        .join(repo)
        .join("runs")
        .join(rollout_id)
        .join("manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["status"], "finalized");
    let attempts = manifest["attempts"].as_array().unwrap();
    assert_eq!(
        attempts.len(),
        if recover {
            2
        } else if succeeds {
            1
        } else if local {
            2
        } else {
            3 + usize::from(death.is_some())
        }
    );
    let failed_attempts = if recover {
        &attempts[..1]
    } else if succeeds {
        &attempts[..0]
    } else {
        &attempts[usize::from(death.is_some())..]
    };
    for attempt in failed_attempts {
        assert!(
            attempt["response"].is_null(),
            "incomplete stream is not a model TextAction"
        );
        assert!(
            serde_json::to_string(&attempt["outcome"])
                .unwrap()
                .contains("provider_failed")
        );
    }
    if !succeeds && !recover {
        assert_ne!(manifest["eligibility"]["status"], "eligible");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// Model-free R03 soak: each hop changes its shell arguments while the
/// verifier keeps returning a stalled result. Neither tool mutates the tree.
#[test]
fn r03_changing_unproductive_actions_escalate_with_stalled_verifier() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _error = EnvGuard::set("ANGEL_ERROR_LIMIT", "8");
    let _experience = EnvGuard::set("ANGEL_EXPERIENCE", "0");
    let _atlas = EnvGuard::set("ANGEL_ATLAS", "0");
    let _trajectory = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0");
    let _rollout = EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off");
    struct StalledTool(&'static str);
    impl Tool for StalledTool {
        fn name(&self) -> &str {
            self.0
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.0.into(),
                description: "model-free stalled fixture".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            Err(if self.0 == "run_tests" {
                "tests: timed out after 120s"
            } else {
                "diagnostic unavailable"
            }
            .into())
        }
    }
    struct Changing(AtomicUsize);
    impl Club for Changing {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "r03-scripted"
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.0.fetch_add(1, Ordering::Relaxed);
            assert!(
                hop < 9,
                "existing eight-error guard did not escalate: {} tools={:?}",
                trajectory::progress_ledger_snapshot(),
                trajectory::tool_ledger_snapshot()
            );
            Ok(ClubReply::Calls(vec![
                ToolCall {
                    id: format!("shell-{hop}"),
                    name: "shell".into(),
                    args: serde_json::json!({"cmd":format!("printf diagnostic-{hop}"), "read_only":true}),
                },
                ToolCall {
                    id: format!("verify-{hop}"),
                    name: "run_tests".into(),
                    // Different filters force real stalled attempts instead of a cached
                    // verifier replay (which correctly is not a dispatch error).
                    args: serde_json::json!({"filter":format!("case_{hop}")}),
                },
            ]))
        }
    }
    let mut registry = ToolRegistry::new();
    let workspace = scratch("r03_soak");
    registry.set_workspace(workspace.clone());
    registry.register(Box::new(StalledTool("shell")));
    registry.register(Box::new(StalledTool("run_tests")));
    let mut history = vec![ChatMsg::user("Diagnose the stalled verifier")];
    let club = Changing(AtomicUsize::new(0));
    let out = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(12),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    assert!(out.contains("errored"), "{out}");
    assert_eq!(club.0.load(Ordering::Relaxed), 8);
    let progress = trajectory::progress_ledger_snapshot();
    assert!(progress["first_verified_at_ms"].is_null());
    assert_eq!(progress["unproductive_streak_max"], 8);
    let escalations = progress["escalations"].as_array().unwrap();
    assert!(
        escalations
            .iter()
            .any(|e| e["kind"] == "error_advisory" && e["hop"].as_u64().unwrap() <= 4)
    );
    assert!(
        escalations
            .iter()
            .any(|e| e["kind"] == "error_stop" && e["hop"].as_u64().unwrap() <= 8)
    );
    let tools = trajectory::tool_ledger_snapshot();
    assert_eq!(tools.len(), 16);
    assert!(
        tools
            .iter()
            .filter(|t| t["tool"] == "shell")
            .all(|t| t["duplicate_of"].is_null())
    );
    assert!(
        tools
            .iter()
            .filter(|t| t["tool"] == "run_tests")
            .all(|t| t["error_class"] == "Transient" && t["avoidable"] == false)
    );
    std::fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn dispatch_receipt_errors_stop_at_existing_limit() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "0");
    let _capsules = EnvGuard::set("ANGEL_ACTION_CAPSULES", "observe");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _error = EnvGuard::set("ANGEL_ERROR_LIMIT", "8");
    let _experience = EnvGuard::set("ANGEL_EXPERIENCE", "0");
    let _atlas = EnvGuard::set("ANGEL_ATLAS", "0");
    let _trajectory = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0");
    let _rollout = EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off");
    struct FailedShell;
    impl Tool for FailedShell {
        fn name(&self) -> &str {
            "shell"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: "shell".into(),
                description: "offline dispatch failure".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            Err("sandbox: mount not confined; fixture detail".into())
        }
    }
    struct Errors(AtomicUsize);
    impl Club for Errors {
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn label(&self) -> &str {
            "offline-dispatch-receipt"
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.0.fetch_add(1, Ordering::Relaxed);
            assert!(hop < 8, "existing dispatch breaker failed");
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("r-{hop}"),
                name: "shell".into(),
                args: serde_json::json!({"cmd":format!("printf {hop}"), "read_only":true}),
            }]))
        }
    }
    let workspace = scratch("dispatch_receipt");
    let mut registry = ToolRegistry::new();
    registry.set_workspace(workspace.clone());
    registry.enable_action_capsules();
    registry.register(Box::new(FailedShell));
    let club = Errors(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel();
    let out = run_turn(
        &club,
        &registry,
        &mut vec![ChatMsg::user("Diagnose this dispatch failure")],
        &AtomicBool::new(false),
        None,
        &tx,
    )
    .unwrap();
    assert!(
        out.contains("stopped after 8 hops where every tool call errored"),
        "{out}"
    );
    assert_eq!(club.0.load(Ordering::Relaxed), 8);
    let ledger = trajectory::tool_ledger_snapshot();
    assert_eq!(ledger.len(), 8);
    assert!(
        ledger
            .iter()
            .all(|entry| entry["error_class"] == "Policy" && entry["avoidable"] == true)
    );
    assert!(
        trajectory::progress_ledger_snapshot()["escalations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "error_stop")
    );
    assert!(
        ledger.iter().all(
            |entry| entry["error"] == "tool error: sandbox: mount not confined; fixture detail"
        )
    );
    let events: Vec<_> = rx.try_iter().collect();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, TurnEvent::Notice(n) if n.starts_with("action receipt")))
            .count(),
        8
    );
    let (mut app, _sender) = crate::tests::seed_live_streaming_app(events);
    app.advance();
    let rows: Vec<_> = app
        .messages
        .iter()
        .filter(|m| m.text.contains("action receipt"))
        .collect();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].text.contains("×8"));
    println!(
        "dispatch receipt: provider hops=8 ledger entries=8 error_class=Policy avoidable=true stop=error_stop transcript rows=1 count=8"
    );
    std::fs::remove_dir_all(workspace).unwrap();
}

/// Changing successful no-ops evade identical-action and error guards. These
/// dialogues exercise the progress ledger's independent escalation boundary.
#[test]
fn r03b_unproductive_dialogues_escalate_stop_and_reset() {
    let _guard = crate::tests::env_lock();
    let _caddy = EnvGuard::set("ANGEL_CADDY", "0");
    let _env = [
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off"),
        EnvGuard::set("ANGEL_RELENTLESS_EXECUTION", "0"),
        EnvGuard::set("ANGEL_UNPRODUCTIVE_STREAK_ESCALATE", "8"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    struct Probe(&'static str);
    impl Tool for Probe {
        fn name(&self) -> &str {
            self.0
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.0.into(),
                description: "scripted probe".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            Ok(if self.0 == "run_tests" {
                "tests: 0 passed, 1 failed"
            } else {
                "no change"
            }
            .into())
        }
    }
    struct Dialogue {
        calls: AtomicUsize,
        progress_at: usize,
        finish: usize,
    }
    impl Club for Dialogue {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "r03b-scripted"
        }
        fn chat(&self, history: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            assert!(hop <= 35, "dialogue failed to terminate");
            if hop == 9 {
                assert!(history.iter().any(|m| {
                    m.role == ChatRole::Harness
                        && m.content
                            .contains("8 consecutive actions changed nothing verifiable")
                }));
            }
            if hop == self.finish {
                return Ok(ClubReply::Text("Blocker: no usable candidate.".into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("probe-{hop}"),
                name: if hop == self.progress_at {
                    "run_tests"
                } else {
                    "probe"
                }
                .into(),
                args: serde_json::json!({"case":hop}),
            }]))
        }
    }
    for (task, competition, stop, progress_at, expected_hops, stopped) in [
        ("1", "0", "16", 0, 16, true),
        ("0", "0", "16", 0, 21, false),
        ("1", "1", "16", 0, 21, false),
        ("1", "0", "0", 0, 21, false),
        ("1", "0", "unset", 0, 21, false),
        ("1", "0", "16", 10, 26, true),
    ] {
        let _mode = [
            EnvGuard::set("ANGEL_TASK_ACTIVE", task),
            EnvGuard::set("ANGEL_COMPETITION_MODE", competition),
            if stop == "unset" {
                EnvGuard::unset("ANGEL_UNPRODUCTIVE_STREAK_STOP")
            } else {
                EnvGuard::set("ANGEL_UNPRODUCTIVE_STREAK_STOP", stop)
            },
        ];
        let root = scratch("r03b");
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        registry.register(Box::new(Probe("probe")));
        registry.register(Box::new(Probe("run_tests")));
        let club = Dialogue {
            calls: AtomicUsize::new(0),
            progress_at,
            finish: if stopped { 34 } else { 21 },
        };
        let mut history = vec![ChatMsg::user("Investigate the blocker")];
        let (tx, rx) = mpsc::channel();
        let outcome = run_turn_observed(
            &club,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(40),
            &tx,
        )
        .unwrap();
        assert_eq!(
            outcome.hops, expected_hops,
            "task={task} competition={competition} progress={progress_at}"
        );
        assert_eq!(club.calls.load(Ordering::SeqCst), expected_hops);
        assert_eq!(
            outcome.stop_reason,
            if stopped {
                TurnStopReason::EscalatedUnproductive
            } else {
                TurnStopReason::Answer
            }
        );
        let ledger = trajectory::progress_ledger_snapshot();
        let escalations: Vec<_> = ledger["escalations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "unproductive_streak")
            .collect();
        assert_eq!(escalations[0]["hop"], 8);
        assert_eq!(escalations[0]["streak"], 8);
        assert_eq!(escalations[0]["last_verifier"], "not_run");
        if progress_at == 10 {
            assert_eq!(escalations[1]["hop"], 18);
            assert_eq!(escalations[1]["last_verifier"], "failed");
        }
        let notices: Vec<_> = rx
            .try_iter()
            .filter_map(|e| match e {
                TurnEvent::Notice(n) if n.starts_with("unproductive streak:") => Some(n),
                _ => None,
            })
            .collect();
        assert_eq!(notices.len(), escalations.len());
        assert!(notices.iter().all(
            |n| crate::ui::views::turn_event_view::notice_coalesce_key(n)
                == Some("unproductive-streak")
        ));
        if stopped {
            assert!(outcome.answer.contains("16 consecutive unproductive hops"));
            let tools = trajectory::tool_ledger_snapshot();
            for tool in tools.iter().rev().take(3) {
                assert!(
                    outcome
                        .answer
                        .contains(tool["args_digest"].as_str().unwrap())
                );
            }
        }
        let envelope = TaskJsonEnvelope::from_outcome(
            TaskJsonContext {
                task_id: None,
                run_id: None,
                workspace: root.clone(),
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
            },
            outcome,
            &history,
        );
        let envelope = serde_json::to_value(envelope).unwrap();
        assert_eq!(
            envelope["status"],
            if stopped { "stopped" } else { "completed" }
        );
        assert_eq!(
            envelope["stop_reason"],
            if stopped {
                "escalated_unproductive"
            } else {
                "answer"
            }
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn task_timing_scripted_turn_reconciles_lifecycle_and_tools() {
    let _guard = crate::tests::env_lock();
    let _skill = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _competition = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let _first_write = EnvGuard::unset("ANGEL_FIRST_WRITE_CALLS");
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            clear_task_lifecycle();
        }
    }
    let _reset = Reset;
    begin_task_lifecycle(std::time::Instant::now());
    let club = ScriptedClub {
        hops: AtomicUsize::new(0),
    };
    let registry = ToolRegistry::with_team(scratch("task_timing_scripted"), Vec::new());
    let mut history = vec![ChatMsg::user("reverse hello")];
    let (events, _received) = mpsc::channel();
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        None,
        &events,
    )
    .unwrap();
    assert_eq!(outcome.answer, "done: olleh");
    let t = outcome.timing.unwrap();
    assert_eq!(
        t.wall_ms,
        t.startup_ms + t.shutdown_ms + t.model_ms + t.tool_ms + t.other_ms
    );
    assert_eq!(t.model_calls, 2);
    assert_eq!(t.calls["model_calls"].as_array().unwrap().len(), 2);
    let tools_ms: u128 = outcome
        .tools
        .iter()
        .map(|tool| u128::from(tool["ms"].as_u64().unwrap()))
        .sum();
    assert_eq!(t.tool_ms, tools_ms + t.tool_overhead_ms);
    assert!(
        outcome
            .tools
            .iter()
            .all(|tool| tool.get("execution_ms").is_some())
    );
}

#[test]
fn run_turn_research_sources_answers_and_circular_anti_spin() {
    let _guard = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off"),
        EnvGuard::set("ANGEL_RELENTLESS_EXECUTION", "0"),
        EnvGuard::set("ANGEL_COMPETITION_MODE", "0"),
        EnvGuard::set("ANGEL_UNPRODUCTIVE_STREAK_ESCALATE", "8"),
        EnvGuard::set("ANGEL_UNPRODUCTIVE_STREAK_STOP", "16"),
        EnvGuard::set("ANGEL_TASK_ACTIVE", "1"),
        EnvGuard::set("ANGEL_SPIN_LIMIT", "0"),
        EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "2"),
        EnvGuard::set("ANGEL_FINAL_MILE_HOPS", "6"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    struct Source;
    impl Tool for Source {
        fn name(&self) -> &str {
            "web_search"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "scripted sources".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, args: &Value) -> Result<String, String> {
            Ok(format!(
                "1. Evidence\n   https://example.test/doc/{}\n   supported fact",
                args["query"].as_u64().unwrap()
            ))
        }
    }
    struct Dialogue {
        calls: AtomicUsize,
        circular: bool,
        sources: usize,
        answer: &'static str,
    }
    impl Club for Dialogue {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "research-scripted"
        }
        fn chat(&self, history: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            assert!(
                history
                    .iter()
                    .any(|m| m.role == ChatRole::Harness && m.content.contains("NO citations"))
            );
            let hop = self.calls.fetch_add(1, Ordering::SeqCst);
            if hop == if self.circular { 18 } else { self.sources } {
                return Ok(ClubReply::Text(self.answer.into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("source-{hop}"),
                name: "web_search".into(),
                args: serde_json::json!({"query": if self.circular { 0 } else {hop}}),
            }]))
        }
    }
    for (circular, sources, cap, draft, answer) in [
        (
            false,
            3,
            8,
            None,
            "42 [a](https://example.test/doc/0) [b](https://example.test/doc/1) [c](https://example.test/doc/2)",
        ),
        (
            false,
            3,
            3,
            Some("Last research draft: 42"),
            "Last research draft: 42",
        ),
        (
            false,
            3,
            3,
            None,
            "Evidence is missing; no research draft was produced before the hop cap.",
        ),
        (
            false,
            8,
            20,
            None,
            "The answer is 42. [Evidence](https://example.test/doc/7)",
        ),
        (
            false,
            8,
            20,
            None,
            "Evidence is missing; the corpus does not establish the answer.",
        ),
        (
            false,
            8,
            20,
            None,
            "Evidence is missing. [Model supplied background](https://example.test/doc/1)",
        ),
        (
            false,
            3,
            8,
            None,
            "**No evidence exists to answer this question, so I cite nothing.**\n\nThe docs (pax-token, pax-upgrade) contain no such value.",
        ),
        (
            false,
            3,
            8,
            None,
            "The corpus contains no evidence answering this question. The two docs are https://example.test/doc/0 and https://example.test/doc/1, but neither mentions crew.",
        ),
        (
            true,
            8,
            20,
            None,
            "Evidence is missing; the repeated search cannot answer this.",
        ),
    ] {
        let root = scratch("research-dialogue");
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        registry.register(Box::new(Source));
        let club = Dialogue {
            calls: AtomicUsize::new(0),
            circular,
            sources,
            answer,
        };
        let mut history = vec![ChatMsg::user(
            "Research question: answer using sources and citations. If evidence is missing, cite nothing.",
        )];
        if let Some(draft) = draft {
            history.push(ChatMsg::assistant("Earlier draft"));
            history.push(ChatMsg::assistant(draft));
        }
        let (tx, rx) = mpsc::channel();
        let outcome = run_turn_observed(
            &club,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(cap),
            &tx,
        )
        .unwrap();
        let progress = trajectory::progress_ledger_snapshot();
        assert_eq!(progress["research_turn"], true);
        if circular {
            assert_eq!(outcome.stop_reason, TurnStopReason::EscalatedUnproductive);
            assert!(rx.try_iter().any(|e| matches!(e, TurnEvent::Notice(n) if n.contains("added no new sources") && n.contains("NO citations") && !n.contains("verifier"))));
        } else if cap == 3 {
            assert_eq!(club.calls.load(Ordering::SeqCst), 3);
            assert_eq!(outcome.hops, 3);
            assert_eq!(outcome.answer, answer);
            assert_eq!(outcome.stop_reason, TurnStopReason::MaxHops);
            assert!(outcome.max_hops_reached);
            assert!(outcome.stop_notice.is_some());
        } else {
            assert_eq!(outcome.hops, sources + 1);
            assert_eq!(club.calls.load(Ordering::SeqCst), sources + 1);
            assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
            if answer.starts_with("Evidence is missing")
                || answer.starts_with("**No evidence exists")
                || answer.starts_with("The corpus contains no evidence")
            {
                assert_eq!(
                    outcome.answer,
                    crate::agent::harness::turn::research::DISCLOSURE
                );
                assert!(!outcome.answer.contains("https://"));
                assert_eq!(history.last().unwrap().content.as_ref(), outcome.answer);
            } else {
                assert_eq!(outcome.answer, answer, "keep supported model citations");
            }
            assert_eq!(progress["research_sources"], sources);
            assert_eq!(progress["research_answer_delivered"], true);
            assert_eq!(progress["unproductive_streak_max"], 0);
            assert!(!rx.try_iter().any(|e| matches!(e, TurnEvent::Notice(n) if n.contains("unproductive streak:") || n.contains("verifier's last outcome"))));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn run_turn_research_task_mode_boundary_envelopes_keep_drafts() {
    let _guard = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off"),
        EnvGuard::set("ANGEL_COMPETITION_MODE", "0"),
        EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "1"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    const ANSWER: &str = "42 [source](https://example.test/doc/1)";
    struct LateAnswer;
    impl Club for LateAnswer {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "research-deadline-scripted"
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            std::thread::sleep(std::time::Duration::from_millis(1100));
            Ok(ClubReply::Text(ANSWER.into()))
        }
    }
    for (max_hops, expected) in [(0, TurnStopReason::MaxHops), (8, TurnStopReason::Deadline)] {
        let root = scratch("research-boundary");
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        let mut history = vec![ChatMsg::user("Use research-answer for this question.")];
        if max_hops == 0 {
            history.push(ChatMsg::assistant(ANSWER));
        }
        let outcome = run_turn_observed(
            &LateAnswer,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(max_hops),
            &mpsc::channel().0,
        )
        .unwrap();
        assert!(outcome.stop_notice.as_ref().is_some_and(|n| !n.is_empty()));
        assert_eq!(outcome.answer, ANSWER);
        assert_eq!(outcome.stop_reason, expected);
        let envelope = TaskJsonEnvelope::from_outcome(
            TaskJsonContext {
                task_id: None,
                run_id: None,
                workspace: root.clone(),
                club: None,
                model: None,
                reasoning_effort: None,
                output_budget: None,
                elapsed_ms: 1200,
                tools: Vec::new(),
                timing: None,
                usage: None,
                runtime: None,
                session_id: None,
                artifacts: Vec::new(),
                memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
            },
            outcome,
            &history,
        );
        let json = serde_json::to_value(envelope).unwrap();
        assert!(json["stop_notice"].as_str().is_some_and(|n| !n.is_empty()));
        assert_eq!(json["answer"], ANSWER);
        assert_eq!(json["stop_reason"], expected.as_str());
        assert_eq!(json["status"], "stopped");
        assert_eq!(
            json["deadline_reached"],
            expected == TurnStopReason::Deadline
        );
        assert_eq!(
            json["max_hops_reached"],
            expected == TurnStopReason::MaxHops
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn run_turn_research_compose_search_loops_decline_early_and_wall() {
    let _guard = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off"),
        EnvGuard::set("ANGEL_COMPETITION_MODE", "0"),
        EnvGuard::set("ANGEL_SPIN_LIMIT", "0"),
        EnvGuard::set("ANGEL_RELENTLESS_EXECUTION", "0"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    struct Source(&'static str, bool);
    impl Tool for Source {
        fn name(&self) -> &str {
            self.0
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.0.into(),
                description: "fixture evidence".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            if self.1 {
                std::thread::sleep(Duration::from_millis(1100));
            }
            Ok("https://example.test/doc/1\nThe answer is 42.".into())
        }
    }
    struct Dialogue {
        hop: AtomicUsize,
        mode: &'static str,
        composed: AtomicBool,
    }
    impl Club for Dialogue {
        fn label(&self) -> &str {
            "research-compose-fixture"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat(&self, history: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.hop.fetch_add(1, Ordering::SeqCst);
            let compose = history.last().is_some_and(|m| {
                m.content.as_ref() == crate::agent::harness::turn::research::COMPOSE
            });
            if compose || (self.mode == "early" && hop == 2) {
                self.composed.store(compose, Ordering::SeqCst);
                assert_eq!(
                    crate::agent::club::final_response_requested(history),
                    compose
                );
                let answer = if self.mode == "decline" || (self.mode == "repeat" && hop < 2) {
                    "Evidence is missing; I cite nothing."
                } else {
                    "42 [source](https://example.test/doc/1)"
                };
                return Ok(ClubReply::Text(answer.into()));
            }
            let fetch = hop == 1 && self.mode != "decline";
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("call-{hop}"),
                name: if fetch { "web_fetch" } else { "web_search" }.into(),
                args: if fetch {
                    serde_json::json!({"url":"https://example.test/doc/1"})
                } else {
                    serde_json::json!({"query":if self.mode == "cap" {format!("clock-{hop}")} else {"clock".into()}})
                },
            }]))
        }
    }
    for (mode, expected_hops, composed) in [
        ("cap", 8, true),
        ("repeat", 5, true),
        ("decline", 3, true),
        ("early", 3, false),
        ("wall", 2, false),
    ] {
        let _deadline = EnvGuard::set(
            "ANGEL_TURN_DEADLINE_SECS",
            if mode == "wall" { "1" } else { "0" },
        );
        let root = scratch("research-compose");
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        registry.register(Box::new(Source("web_search", false)));
        registry.register(Box::new(Source("web_fetch", mode == "wall")));
        let club = Dialogue {
            hop: AtomicUsize::new(0),
            mode,
            composed: AtomicBool::new(false),
        };
        let mut history = vec![ChatMsg::user(
            "Research question: answer from sources and citations.",
        )];
        let outcome = run_turn_observed(
            &club,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(8),
            &mpsc::channel().0,
        )
        .unwrap();
        assert_eq!(outcome.hops, expected_hops, "{mode}");
        assert_eq!(club.composed.load(Ordering::SeqCst), composed, "{mode}");
        assert_eq!(club.hop.load(Ordering::SeqCst), expected_hops, "{mode}");
        if mode == "wall" {
            assert!(outcome.deadline_reached);
            assert!(
                !history
                    .iter()
                    .any(|m| m.role == ChatRole::Assistant && !m.content.trim().is_empty())
            );
            assert!(
                !history
                    .iter()
                    .any(|m| m.content.as_ref() == crate::agent::harness::turn::research::COMPOSE)
            );
        } else if mode == "decline" {
            assert_eq!(
                outcome.answer,
                crate::agent::harness::turn::research::DISCLOSURE
            );
        } else {
            assert_eq!(
                outcome.answer, "42 [source](https://example.test/doc/1)",
                "{mode}"
            );
        }
        assert_eq!(
            outcome.stop_reason,
            if mode == "wall" {
                TurnStopReason::Deadline
            } else {
                TurnStopReason::Answer
            }
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn run_turn_r06_edit_run_lane_and_read_only_streaks() {
    let _guard = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_TASK_ACTIVE", "1"),
        EnvGuard::set("ANGEL_COMPETITION_MODE", "0"),
        EnvGuard::set("ANGEL_UNPRODUCTIVE_STREAK_STOP", "60"),
        EnvGuard::set("ANGEL_UNPRODUCTIVE_STREAK_ESCALATE", "8"),
        EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "off"),
        EnvGuard::set("ANGEL_RELENTLESS_EXECUTION", "0"),
        EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0"),
        EnvGuard::set("ANGEL_TOOLCALL_STORM", "0"),
        EnvGuard::set("ANGEL_SPIN_LIMIT", "0"),
        EnvGuard::set("ANGEL_TOOL_CYCLE_REPEATS", "0"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    struct FixtureTool {
        root: PathBuf,
        name: &'static str,
    }
    impl Tool for FixtureTool {
        fn name(&self) -> &str {
            self.name
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name.into(),
                description: "R06 local fixture".into(),
                params: json!({"type":"object"}),
            }
        }
        fn call(&self, args: &Value) -> Result<String, String> {
            match self.name {
                "write_file" => {
                    std::fs::write(
                        self.root.join("candidate.txt"),
                        args["content"].as_str().unwrap(),
                    )
                    .unwrap();
                    Ok("written".into())
                }
                "run_tests" => Ok("tests: 0 passed, 1 failed\ntest fixture::red ... FAILED".into()),
                _ => Ok("same contents".into()),
            }
        }
    }
    struct Lane {
        count: AtomicUsize,
        mode: usize,
    }
    impl Club for Lane {
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn label(&self) -> &str {
            "r06-scripted"
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let hop = self.count.fetch_add(1, Ordering::SeqCst) + 1;
            if hop > 200 {
                return Ok(ClubReply::Text(
                    "Fixture complete; tests remain red.".into(),
                ));
            }
            let (name, args) = match self.mode {
                0 if hop % 3 != 2 => (
                    "write_file",
                    json!({"path":"candidate.txt", "content":format!("candidate {hop}")}),
                ),
                0 | 2 => ("run_tests", json!({})),
                3 => (
                    "write_file",
                    json!({"path":"candidate.txt", "content":"unchanged"}),
                ),
                _ => ("read_file", json!({"path":format!("read-{hop}.txt")})),
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("r06-{hop}"),
                name: name.into(),
                args,
            }]))
        }
    }
    for mode in 0..4 {
        let root = scratch("r06_progress");
        std::fs::write(root.join("candidate.txt"), "unchanged").unwrap();
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        for name in ["write_file", "run_tests", "read_file"] {
            registry.register(Box::new(FixtureTool {
                root: root.clone(),
                name,
            }));
        }
        let club = Lane {
            count: AtomicUsize::new(0),
            mode,
        };
        let outcome = run_turn_observed(
            &club,
            &registry,
            &mut vec![ChatMsg::user("Exercise the local fixture lane")],
            &AtomicBool::new(false),
            Some(210),
            &mpsc::channel().0,
        )
        .unwrap();
        if mode == 0 {
            assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
            assert_eq!(outcome.hops, 201);
            assert!(
                trajectory::progress_ledger_snapshot()["unproductive_streak_max"]
                    .as_u64()
                    .unwrap()
                    < 60
            );
        } else {
            assert_eq!(outcome.stop_reason, TurnStopReason::EscalatedUnproductive);
            assert_eq!(outcome.hops, if mode == 2 { 61 } else { 60 });
            assert!(outcome.answer.contains("last credited progress:"));
        }
        println!(
            "R06 scripted mode={mode} hops={} stop={:?} ledger={}",
            outcome.hops,
            outcome.stop_reason,
            trajectory::progress_ledger_snapshot()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn run_turn_r06_busy_child_survives_turn_idle() {
    let _guard = crate::tests::env_lock();
    // The default is a fast CPU-only regression; the qualification command
    // selects 1200 seconds and periodic output with a 60-second turn idle clock.
    let seconds = std::env::var("ANGEL_T_R06_BUSY_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5);
    let idle = if seconds >= 60 { "60" } else { "2" };
    let _env = [
        EnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", idle),
        EnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", idle),
        EnvGuard::set("ANGEL_TOOL_IDLE_SECS", idle),
        EnvGuard::set("ANGEL_TOOL_HARD_TIMEOUT", "0"),
        EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", &(seconds + 30).to_string()),
        EnvGuard::set("ANGEL_YOLO", "0"),
        EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    struct Busy(u64);
    impl Tool for Busy {
        fn name(&self) -> &str {
            "busy_fixture"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "owned finite busy child".into(),
                params: json!({"type":"object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            unreachable!()
        }
        fn call_with_cancel(
            &self,
            _: &Value,
            cancel: Option<&AtomicBool>,
        ) -> Result<String, String> {
            let mut cmd = std::process::Command::new("/usr/bin/python3");
            cmd.args(["-c", &format!("import time\nstart=time.monotonic()\nnext_output=start+5\nx=0\nwhile time.monotonic()-start < {}:\n x=(x+1)%1000003\n if {} and time.monotonic() >= next_output:\n  print('busy', flush=True)\n  next_output+=5\nprint('busy-done', flush=True)", self.0, if self.0 >= 60 { "True" } else { "False" })]);
            let capture = super::super::exec::output_timed_captured_cancellable(
                cmd,
                Some(Duration::from_secs(self.0 + 20)),
                cancel,
            )?;
            assert!(!capture.cancelled && !capture.timed_out);
            assert!(capture.output.status.success());
            Ok(String::from_utf8(capture.output.stdout).unwrap())
        }
    }
    let root = scratch("r06_busy");
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.clone());
    registry.register(Box::new(Busy(seconds)));
    struct BusyClub(AtomicUsize);
    impl Club for BusyClub {
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn label(&self) -> &str {
            "busy-scripted"
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                ClubReply::Calls(vec![tc("busy_fixture", json!({}))])
            } else {
                ClubReply::Text("Busy child completed.".into())
            })
        }
    }
    let club = BusyClub(AtomicUsize::new(0));
    let start = Instant::now();
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut vec![ChatMsg::user("Run the local busy fixture")],
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel().0,
    )
    .unwrap();
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert!(start.elapsed() >= Duration::from_secs(seconds));
    println!(
        "R06 busy_child seconds={seconds} idle_seconds={idle} elapsed_ms={} stop={:?}",
        start.elapsed().as_millis(),
        outcome.stop_reason
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn run_turn_r06_actual_sse_cut_at_4kb_recovers() {
    let body = format!(
        "data: {}\n\n",
        json!({"choices":[{"delta":{"content":"x".repeat(4096)}}]})
    );
    provider_death_fixture(&body, false, true);
    println!(
        "R06 SSE: cut after 4096 content bytes; same hop recovered, partial history discarded, stream_cut=1"
    );
}

#[test]
fn run_turn_r06_stream_cut_budget_renews_on_each_hop() {
    let _guard = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_PROVIDER_RETRIES", "1"),
        EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0"),
        EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0"),
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    struct Cuts {
        requests: Mutex<Vec<Value>>,
    }
    impl Club for Cuts {
        fn label(&self) -> &str {
            "glm"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            unreachable!()
        }
        fn chat_streaming(
            &self,
            history: &[ChatMsg],
            _: &[ToolDef],
            _: &AtomicBool,
            delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
        ) -> Result<ClubReply, String> {
            let mut requests = self.requests.lock().unwrap();
            requests.push(json!(crate::agent::club::messages_to_json(history, true)));
            match requests.len() {
                1 | 3 => {
                    delta(crate::agent::club::StreamDelta::Content("discard-partial"));
                    Err(crate::agent::club::INCOMPLETE_STREAM_ERR.into())
                }
                2 => Ok(ClubReply::Calls(vec![tc(
                    "reverse",
                    json!({"text":"fixture"}),
                )])),
                4 => Ok(ClubReply::Text("complete".into())),
                _ => panic!("per-hop retry budget escaped"),
            }
        }
    }
    let root = scratch("r06_stream_hops");
    let mut registry = ToolRegistry::with_defaults();
    registry.set_workspace(root.clone());
    let club = Cuts {
        requests: Mutex::new(Vec::new()),
    };
    let mut history = vec![ChatMsg::user("Exercise the scripted retry fixture")];
    let (tx, rx) = mpsc::channel();
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &tx,
    )
    .unwrap();
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(outcome.hops, 2);
    let requests = club.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(requests[2], requests[3]);
    assert!(
        !history
            .iter()
            .any(|m| m.content.contains("discard-partial"))
    );
    assert_eq!(
        rx.try_iter()
            .filter(|e| matches!(e, TurnEvent::SuppressPartial))
            .count(),
        2
    );
    assert_eq!(
        trajectory::progress_ledger_snapshot()["hop_stream_cuts"],
        json!([{"hop":1,"stream_cut":1},{"hop":2,"stream_cut":1}])
    );
    println!(
        "R06 stream cuts: 2 hops, 1 retry each, 4 attempts, identical per-hop requests, two partial retractions"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn run_turn_research_at_wall_without_draft_does_not_compose() {
    let _guard = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "1"),
        EnvGuard::set("ANGEL_COMPETITION_MODE", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    struct AtWall(AtomicUsize);
    impl Club for AtWall {
        fn label(&self) -> &str {
            "research-at-wall"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn chat(&self, history: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            assert_eq!(
                self.0.fetch_add(1, Ordering::SeqCst),
                0,
                "no request after the wall"
            );
            assert!(!crate::agent::club::final_response_requested(history));
            std::thread::sleep(Duration::from_millis(1100));
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "late-search".into(),
                name: "web_search".into(),
                args: serde_json::json!({"query":"evidence"}),
            }]))
        }
    }
    let root = scratch("research-at-wall");
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.clone());
    let club = AtWall(AtomicUsize::new(0));
    let mut history = vec![ChatMsg::user(
        "Use research-answer. Research origin: http://127.0.0.1:12345",
    )];
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel().0,
    )
    .unwrap();
    assert_eq!(club.0.load(Ordering::SeqCst), 1);
    assert_eq!(outcome.stop_reason, TurnStopReason::Deadline);
    assert!(outcome.deadline_reached);
    assert!(
        !history
            .iter()
            .any(|m| m.content.as_ref() == crate::agent::harness::turn::research::COMPOSE)
    );
    assert!(
        !history
            .iter()
            .any(|m| m.role == ChatRole::Assistant && !m.content.trim().is_empty())
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn run_turn_research_compose_rechecks_reservation_after_identity_binding() {
    let _guard = crate::tests::env_lock();
    let _env = [
        EnvGuard::set("ANGEL_TURN_DEADLINE_SECS", "1"),
        EnvGuard::set("ANGEL_COMPETITION_MODE", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_CADDY", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
    ];
    struct BindingDelay {
        delay: Duration,
        requests: AtomicUsize,
    }
    impl Club for BindingDelay {
        fn label(&self) -> &str {
            "research-reservation"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn bind_run_identity(&self, _: Option<&str>) -> Result<(), String> {
            std::thread::sleep(self.delay);
            Ok(())
        }
        fn chat(&self, history: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            assert!(crate::agent::club::final_response_requested(history));
            Ok(ClubReply::Text(
                "Evidence is missing; I cite nothing.".into(),
            ))
        }
    }
    for delay_ms in [0, 950, 1100] {
        let root = scratch("research-reservation");
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        let club = BindingDelay {
            delay: Duration::from_millis(delay_ms),
            requests: AtomicUsize::new(0),
        };
        let mut history = vec![ChatMsg::user("Use research-answer for this question.")];
        let outcome = run_turn_observed(
            &club,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(1),
            &mpsc::channel().0,
        )
        .unwrap();
        assert_eq!(
            club.requests.load(Ordering::SeqCst),
            usize::from(delay_ms == 0)
        );
        assert_eq!(
            outcome.stop_reason,
            if delay_ms == 0 {
                TurnStopReason::Answer
            } else {
                TurnStopReason::Deadline
            }
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

/// Runs `make` once, then claims completion on every later call.
struct FlakyGreenClub {
    calls: AtomicUsize,
}

impl Club for FlakyGreenClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("unused".into())
    }

    fn label(&self) -> &str {
        "flaky-green"
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(ClubReply::Calls(vec![ToolCall {
                id: "call_make".into(),
                name: "shell".into(),
                args: serde_json::json!({"command": "make"}),
            }]));
        }
        Ok(ClubReply::Text(
            "All tests pass; the task is complete.".into(),
        ))
    }
}

fn confirm_green_fixture(name: &str, makefile: &str) -> PathBuf {
    let root = scratch(name);
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("git fixture command starts");
        assert!(output.status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "angel@example.invalid"]);
    git(&["config", "user.name", "Angel Test"]);
    std::fs::write(root.join("Makefile"), makefile).unwrap();
    git(&["add", "Makefile"]);
    git(&["commit", "-q", "-m", "seed"]);
    root
}

/// A minimal `shell` for turn tests: runs the command in its root, and a
/// non-zero exit is a tool error, as with the real shell tool.
struct RootShell(PathBuf);

impl Tool for RootShell {
    fn name(&self) -> &str {
        "shell"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "shell".into(),
            description: "Run a shell command in the workspace.".into(),
            params: serde_json::json!({
                "type": "object",
                "properties": {"command": {"type": "string"}},
                "required": ["command"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let command = args["command"].as_str().ok_or("missing 'command'")?;
        let out = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.0)
            .output()
            .map_err(|e| e.to_string())?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if out.status.success() {
            Ok(text)
        } else {
            Err(format!("exit {}\n{text}", out.status.code().unwrap_or(-1)))
        }
    }
}

fn run_confirm_green_turn(root: &Path) -> (TurnOutcome, Vec<ChatMsg>, usize) {
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.to_path_buf());
    registry.register(Box::new(RootShell(root.to_path_buf())));
    let club = FlakyGreenClub {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("make the tests pass")];
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("turn completes");
    (outcome, history, club.calls.load(Ordering::SeqCst))
}

/// A test run that passed once is re-run on the unchanged code before "done"
/// is accepted. A pass that does not hold denies the completion with the
/// failing output (polyglot-v1 cpp-robot-name passed about one run in four); a
/// stable pass is confirmed and accepted.
#[test]
fn a_green_that_does_not_hold_denies_completion_with_the_failing_output() {
    let _guard = crate::tests::env_lock();
    let _runs = EnvGuard::set("ANGEL_CONFIRM_GREEN_RUNS", "2");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _no_edit = EnvGuard::set("ANGEL_NO_EDIT_ANSWER_GUARD", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");

    let flaky = confirm_green_fixture(
        "confirm_green_flaky",
        "all:\n\t@if [ -f .ran ]; then echo 'REQUIRE( names.count(name) == 0 ) failed'; exit 1; fi; touch .ran\n",
    );
    let (outcome, history, calls) = run_confirm_green_turn(&flaky);
    assert_eq!(calls, 3, "the first done is denied, the second accepted");
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    let denial = history
        .iter()
        .find(|m| m.role == ChatRole::Harness && m.content.contains(CONFIRM_GREEN_NUDGE))
        .expect("the completion was denied");
    assert!(
        denial.content.contains("names.count(name) == 0"),
        "{}",
        denial.content
    );
    assert!(
        denial.content.contains("`shell: make`"),
        "{}",
        denial.content
    );
    let _ = std::fs::remove_dir_all(flaky);

    let stable = confirm_green_fixture("confirm_green_stable", "all:\n\t@true\n");
    let (outcome, history, calls) = run_confirm_green_turn(&stable);
    assert_eq!(calls, 2, "a green that holds is accepted at once");
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert!(
        !history
            .iter()
            .any(|m| m.role == ChatRole::Harness && m.content.contains(CONFIRM_GREEN_NUDGE))
    );
    let _ = std::fs::remove_dir_all(stable);
}

#[test]
fn confirm_green_is_opt_in() {
    let _guard = crate::tests::env_lock();
    let _runs = EnvGuard::unset("ANGEL_CONFIRM_GREEN_RUNS");
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    assert_eq!(
        confirm_green_extra_runs(false),
        0,
        "off in task mode unless set"
    );
    let _set = EnvGuard::set("ANGEL_CONFIRM_GREEN_RUNS", "2");
    assert_eq!(confirm_green_extra_runs(false), 2);
    let _many = EnvGuard::set("ANGEL_CONFIRM_GREEN_RUNS", "9");
    assert_eq!(confirm_green_extra_runs(true), 5, "capped at 5");
}

/// The command shapes Grok 4.7 used on cpp-robot-name: a test run with a
/// fallback that can mask its failure.
#[test]
fn test_runs_behind_a_fallback_are_recognised() {
    let shell = |command: &str| ToolCall {
        id: String::new(),
        name: "shell".into(),
        args: serde_json::json!({"command": command}),
    };
    for command in [
        "cmake -S . -B build && cmake --build build && (./build/robot-name || ctest --test-dir build --output-on-failure)",
        "cmake -S . -B build && cmake --build build && ./build/robot-name 2>/dev/null || ctest --test-dir build --output-on-failure",
        "make || true",
    ] {
        assert!(test_run_behind_fallback(&shell(command)), "{command}");
    }
    for command in ["ls || true", "cmake --build build", "echo ok"] {
        assert!(!test_run_behind_fallback(&shell(command)), "{command}");
    }
}

/// Stands in for run_tests: always red.
struct RedRunTests;

impl Tool for RedRunTests {
    fn name(&self) -> &str {
        "run_tests"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "run_tests".into(),
            description: "Run the workspace tests.".into(),
            params: serde_json::json!({"type": "object", "properties": {}}),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        Err("tests failed: 0 passed, 1 failed\nREQUIRE( names.count(name) == 0 ) failed".into())
    }
}

/// Runs a masked test command once, then claims completion on every later call.
struct MaskedGreenClub {
    calls: AtomicUsize,
}

impl Club for MaskedGreenClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        "masked-green"
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(ClubReply::Calls(vec![ToolCall {
                id: "call_make".into(),
                name: "shell".into(),
                args: serde_json::json!({"command": "make || true"}),
            }]));
        }
        Ok(ClubReply::Text(
            "All tests pass; the task is complete.".into(),
        ))
    }
}

/// A green that rests on a fallback-masked command is confirmed with angelX's
/// own run_tests; when that is red, the completion is denied.
#[test]
fn a_masked_green_is_confirmed_with_run_tests() {
    let _guard = crate::tests::env_lock();
    let _runs = EnvGuard::set("ANGEL_CONFIRM_GREEN_RUNS", "2");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _no_edit = EnvGuard::set("ANGEL_NO_EDIT_ANSWER_GUARD", "0");
    let _deferred = EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0");
    let _skill_hint = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let root = confirm_green_fixture("confirm_green_masked", "all:\n\t@false\n");
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.clone());
    registry.register(Box::new(RootShell(root.clone())));
    registry.register(Box::new(RedRunTests));
    let club = MaskedGreenClub {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("make the tests pass")];
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("turn completes");
    assert_eq!(club.calls.load(Ordering::SeqCst), 3);
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    let denial = history
        .iter()
        .find(|m| m.role == ChatRole::Harness && m.content.contains(CONFIRM_GREEN_NUDGE))
        .expect("the masked green was not accepted");
    assert!(denial.content.contains("`run_tests`"), "{}", denial.content);
    assert!(
        denial.content.contains("names.count(name) == 0"),
        "{}",
        denial.content
    );
    let _ = std::fs::remove_dir_all(root);
}

/// First reply is cut off at the output cap; the retry answers.
struct CappedOnceClub {
    calls: AtomicUsize,
}

impl Club for CappedOnceClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        "capped-once"
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err("response incomplete: configured request cap was 8192 tokens".into());
        }
        Ok(ClubReply::Text("done in smaller pieces".into()))
    }
}

/// A reply cut off at a fixed output cap is not re-sent blind: the model is told
/// before the retry, so the retry is a different request (polyglot-v1
/// rust-decimal on DeepSeek re-sent the same capped request 25 times).
#[test]
fn an_output_cap_cut_off_tells_the_model_before_the_retry() {
    let _guard = crate::tests::env_lock();
    let _retries = EnvGuard::set("ANGEL_PROVIDER_RETRIES", "2");
    let _backoff = EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _no_edit = EnvGuard::set("ANGEL_NO_EDIT_ANSWER_GUARD", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let registry = ToolRegistry::new();
    let club = CappedOnceClub {
        calls: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("write the implementation")];
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("the retry answers");
    assert_eq!(club.calls.load(Ordering::SeqCst), 2);
    assert!(
        outcome.answer.contains("smaller pieces"),
        "{}",
        outcome.answer
    );
    let notes: Vec<_> = history
        .iter()
        .filter(|m| m.role == ChatRole::Harness && m.content.contains(OUTPUT_CAP_NUDGE))
        .collect();
    assert_eq!(notes.len(), 1, "one note before the retry");
    assert!(
        notes[0].content.contains("8192 tokens"),
        "{}",
        notes[0].content
    );
    assert!(is_output_cap_truncation(
        crate::agent::club::TRUNCATED_OUTPUT_ERR
    ));
    assert!(!is_output_cap_truncation("openai responses: HTTP 500"));
}

/// A Yukon benchmark.json declares the editable surface (both schemas), and the
/// operator can set it directly.
#[test]
fn task_edit_scope_reads_the_env_then_benchmark_json() {
    let _guard = crate::tests::env_lock();
    let _unset = EnvGuard::unset("ANGEL_TASK_EDITABLE_PATHS_JSON");
    let root = scratch("edit_scope_manifest");
    assert_eq!(task_edit_scope(&root), None, "no manifest, no scope");
    std::fs::write(
        root.join("benchmark.json"),
        r#"{"schemaVersion":1,"editablePaths":["ds4","harness/"],"optionalEditablePaths":["mtp-head.manifest.json"]}"#,
    )
    .unwrap();
    assert_eq!(
        task_edit_scope(&root),
        Some(vec![
            "ds4".into(),
            "harness".into(),
            "mtp-head.manifest.json".into()
        ])
    );
    std::fs::write(
        root.join("benchmark.json"),
        r#"{"schemaVersion":2,"tracks":[{"name":"subset","editablePaths":["candidates/subset"]},{"name":"pinning","editablePaths":["candidates/pinning"]}]}"#,
    )
    .unwrap();
    assert_eq!(
        task_edit_scope(&root),
        Some(vec![
            "candidates/pinning".into(),
            "candidates/subset".into()
        ])
    );
    let _set = EnvGuard::set(
        "ANGEL_TASK_EDITABLE_PATHS_JSON",
        r#"["src/lib.rs", "src/extra/"]"#,
    );
    assert_eq!(
        task_edit_scope(&root),
        Some(vec!["src/lib.rs".into(), "src/extra".into()])
    );
    let _ = std::fs::remove_dir_all(root);
}

/// Edits outside the declared scope get one note per path per turn; edits inside
/// it, and turns without a scope, get none.
#[test]
fn edits_outside_the_edit_scope_are_noted_once() {
    let root = PathBuf::from("/ws/task");
    let scope = vec!["src/lib.rs".to_string(), "crates/core".to_string()];
    let write = |path: &str| ToolCall {
        id: String::new(),
        name: "write_file".into(),
        args: serde_json::json!({"path": path, "content": "x"}),
    };
    let mut noted = std::collections::HashSet::new();
    assert_eq!(
        edit_scope_note(Some(&scope), &root, &write("src/lib.rs"), &mut noted),
        None
    );
    assert_eq!(
        edit_scope_note(
            Some(&scope),
            &root,
            &write("/ws/task/crates/core/a.rs"),
            &mut noted
        ),
        None,
        "absolute paths inside the scope are fine"
    );
    let note = edit_scope_note(Some(&scope), &root, &write("Cargo.toml"), &mut noted)
        .expect("an out-of-scope edit is noted");
    assert!(
        note.contains("Cargo.toml is outside this task's editable paths"),
        "{note}"
    );
    assert_eq!(
        edit_scope_note(Some(&scope), &root, &write("Cargo.toml"), &mut noted),
        None,
        "once per path per turn"
    );
    assert_eq!(
        edit_scope_note(None, &root, &write("Cargo.toml"), &mut noted),
        None
    );
}

// --- polyglot-v1 root causes: storm window, red verdicts, red completions ---

/// Plays a fixed list of moves, then answers "done".
enum Move {
    Call(ToolCall),
    Say(&'static str),
}

struct Script {
    moves: Vec<Move>,
    next: AtomicUsize,
}

impl Script {
    fn new(moves: Vec<Move>) -> Self {
        Self {
            moves,
            next: AtomicUsize::new(0),
        }
    }
    fn chats(&self) -> usize {
        self.next.load(Ordering::SeqCst)
    }
}

impl Club for Script {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn label(&self) -> &str {
        "script"
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let index = self.next.fetch_add(1, Ordering::SeqCst);
        Ok(match self.moves.get(index) {
            Some(Move::Call(call)) => ClubReply::Calls(vec![ToolCall {
                id: format!("s{index}"),
                ..call.clone()
            }]),
            Some(Move::Say(text)) => ClubReply::Text((*text).to_string()),
            None => ClubReply::Text("done".to_string()),
        })
    }
}

fn run_tests_call(attempt: usize) -> Move {
    Move::Call(tc("run_tests", json!({ "attempt": attempt })))
}

fn write_call(path: &str, content: &str) -> Move {
    Move::Call(tc(
        "write_file",
        json!({ "path": path, "content": content }),
    ))
}

/// Stands in for run_tests with the real runner's red format: red until the
/// workspace has a `fixed` file, green after.
struct Suite {
    root: PathBuf,
    runs: Arc<AtomicUsize>,
}

impl Tool for Suite {
    fn name(&self) -> &str {
        "run_tests"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "run_tests".into(),
            description: "Run the workspace tests.".into(),
            params: json!({"type": "object"}),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        if self.root.join("fixed").exists() {
            Ok("tests: 2 passed, 0 failed".into())
        } else {
            Err("cargo test failed (exit 101)\n---- sub_id stdout ----\n\
                 assertion `left == right` failed: sub_id\n\
                 test result: FAILED. 1 passed; 1 failed"
                .into())
        }
    }
}

/// A plain file writer, so edits change real bytes in the workspace.
struct Writer(PathBuf);

impl Tool for Writer {
    fn name(&self) -> &str {
        "write_file"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "write_file".into(),
            description: "Write a file.".into(),
            params: json!({"type": "object"}),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let path = args["path"].as_str().ok_or("missing 'path'")?;
        let content = args["content"].as_str().unwrap_or("");
        std::fs::write(self.0.join(path), content).map_err(|e| e.to_string())?;
        Ok(format!("wrote {} bytes to {path}", content.len()))
    }
}

fn root_turn_env() -> Vec<EnvGuard> {
    vec![
        EnvGuard::set("ANGEL_SKILL_HINT", "0"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0"),
        EnvGuard::set("ANGEL_NO_EDIT_ANSWER_GUARD", "0"),
        EnvGuard::set("ANGEL_DEFERRED_ACTION_LIMIT", "0"),
        EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0"),
        EnvGuard::set("ANGEL_EXPERIENCE", "0"),
        EnvGuard::set("ANGEL_ATLAS", "0"),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "0"),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUT", "off"),
        EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD"),
        EnvGuard::unset("ANGEL_CONFIRM_GREEN_RUNS"),
        EnvGuard::unset("ANGEL_TASK_WALL_SECS"),
        EnvGuard::unset("ANGEL_TURN_DEADLINE_SECS"),
        EnvGuard::unset("ANGEL_COMPETITION_MODE"),
    ]
}

/// A Git workspace, so the turn can tell whether code changed since a test run.
fn root_fixture(name: &str) -> PathBuf {
    let root = scratch(name);
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("git fixture command starts");
        assert!(output.status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "angel@example.invalid"]);
    git(&["config", "user.name", "Angel Test"]);
    std::fs::write(root.join("lib.rs"), "pub fn sub() {}\n").unwrap();
    git(&["add", "lib.rs"]);
    git(&["commit", "-q", "-m", "seed"]);
    root
}

/// Runs `script` in `root` with the suite and writer registered. Returns the
/// outcome, the history, and how many times the suite ran.
fn run_root_turn(root: &Path, script: &Script) -> (TurnOutcome, Vec<ChatMsg>, usize) {
    let runs = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.to_path_buf());
    registry.register(Box::new(Suite {
        root: root.to_path_buf(),
        runs: Arc::clone(&runs),
    }));
    registry.register(Box::new(Writer(root.to_path_buf())));
    let mut history = vec![ChatMsg::user("make the tests pass")];
    let outcome = run_turn_observed(
        script,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(30),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("turn completes");
    (outcome, history, runs.load(Ordering::SeqCst))
}

/// The same call after an edit reads different code, so it is not a
/// duplicate. polyglot-v1: 35 of 37 storm suppressions came right after an
/// edit, and gpt-6-luna gave up on js-food-chain when its re-run was refused.
#[test]
fn the_storm_window_restarts_when_the_workspace_changes() {
    let test = tc("run_tests", json!({}));
    let mut storm = ToolCallStorm::new(6);
    assert_eq!(storm.observe(std::slice::from_ref(&test)), vec![1]);
    assert_eq!(storm.observe(std::slice::from_ref(&test)), vec![2]);
    storm.workspace_changed();
    assert_eq!(
        storm.observe(std::slice::from_ref(&test)),
        vec![1],
        "a run after an edit is a first sighting"
    );
    assert_eq!(storm.observe(std::slice::from_ref(&test)), vec![2]);
    assert_eq!(
        storm.observe(std::slice::from_ref(&test)),
        vec![3],
        "with nothing changed in between, repeats still count"
    );
}

#[test]
fn a_call_after_an_edit_in_the_same_batch_is_counted_afresh() {
    let test = tc("run_tests", json!({}));
    let edit = tc(
        "str_replace",
        json!({"path": "lib.rs", "old": "a", "new": "b"}),
    );
    let mut storm = ToolCallStorm::new(6);
    storm.observe(std::slice::from_ref(&test));
    storm.observe(std::slice::from_ref(&test));
    assert_eq!(storm.observe(&[edit.clone(), test.clone()]), vec![1, 1]);
    assert_eq!(
        storm.observe(&[edit.clone(), test.clone(), test.clone()]),
        vec![2, 1, 2],
        "the edit itself still counts, and so do repeats after it"
    );
}

/// Edit → test → edit → test → edit → test with the guard armed: every test
/// run executes, because each one follows a change.
#[test]
fn an_armed_storm_guard_lets_each_test_run_after_an_edit_through() {
    let _guard = crate::tests::env_lock();
    let _env = root_turn_env();
    let _armed = EnvGuard::set("ANGEL_TOOLCALL_STORM", "1");
    let _window = EnvGuard::set("ANGEL_TOOLCALL_STORM_WINDOW", "6");
    let _task = EnvGuard::unset("ANGEL_TASK_ACTIVE");
    let root = root_fixture("storm_edits");
    let test = || Move::Call(tc("run_tests", json!({})));
    let script = Script::new(vec![
        test(),
        write_call("lib.rs", "pub fn sub() { 1 }\n"),
        test(),
        write_call("lib.rs", "pub fn sub() { 2 }\n"),
        test(),
    ]);
    let (outcome, history, runs) = run_root_turn(&root, &script);
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(runs, 3, "no test run after an edit is suppressed");
    assert!(
        !history
            .iter()
            .any(|m| m.content.contains("duplicate call suppressed")),
        "no duplicate verdict after an edit"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_red_verifier_verdict_is_not_a_dispatch_failure() {
    let shell = |command: &str| tc("shell", json!({ "command": command }));
    let red =
        "tool error: shell command failed (exit 101)\ntest result: FAILED. 20 passed; 24 failed";
    assert!(is_red_verifier_run(
        &shell("cargo test 2>&1 | tail -20"),
        red
    ));
    assert!(!is_dispatch_failure(&shell("cargo test"), red));
    assert!(!is_dispatch_failure(
        &shell("cd ws && npx jest forth.spec.js 2>&1 | sed -n '1,80p'"),
        "tool error: shell command failed (exit 1)\nTests: 2 failed, 47 passed"
    ));
    let runner = "tool error: npm test (jest ./*) failed (exit 1) — chosen because of package.json; \
                  pin another runner with `runner` or use `shell`\nTests: 2 failed, 47 passed";
    assert!(is_red_verifier_run(&tc("run_tests", json!({})), runner));

    // Runs that never reached a verdict are still dispatch failures.
    for (call, result) in [
        (
            shell("cargo test"),
            "tool error: shell command failed (exit 127)\ncargo: not found",
        ),
        (
            shell("cargo test"),
            "tool error: shell command failed (exit 126)",
        ),
        (
            tc("run_tests", json!({})),
            "tool error: cargo test failed (exit signal)",
        ),
        (
            tc("run_tests", json!({})),
            "tool error: tests: timed out after 120s",
        ),
        (
            shell("ls missing"),
            "tool error: shell command failed (exit 2)",
        ),
        (
            tc("read_file", json!({"path": "x"})),
            "tool error: no such file: x",
        ),
    ] {
        assert!(is_dispatch_failure(&call, result), "{result}");
    }
    assert!(!is_dispatch_failure(
        &tc("run_tests", json!({})),
        "tests: 1 passed, 0 failed"
    ));
}

/// A model re-running a red suite is looking at its own failing tests, not
/// hitting a broken tool. polyglot-v1 rust-decimal: DeepSeek V4.1 Flash was
/// stopped at 67 s of 600 s and told to fix a path while its tests were red.
#[test]
fn red_test_runs_do_not_trip_the_consecutive_error_stop() {
    let _guard = crate::tests::env_lock();
    let _env = root_turn_env();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let _error = EnvGuard::set("ANGEL_ERROR_LIMIT", "2");
    let _red_gate = EnvGuard::set("ANGEL_RED_COMPLETION_DENIALS", "0");
    let root = scratch("red_runs_error_stop");
    let script = Script::new((0..7).map(run_tests_call).collect());
    let (outcome, history, runs) = run_root_turn(&root, &script);
    assert_eq!(runs, 7);
    assert_eq!(
        outcome.stop_reason,
        TurnStopReason::Answer,
        "seven red runs under ANGEL_ERROR_LIMIT=2 must not end the turn"
    );
    assert!(
        !history
            .iter()
            .any(|m| m.content.contains("ERROR CASCADE REDIRECTION")),
        "no path/state advice for failing tests"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// Task mode: "done" while the last test run on the same code is red is
/// denied with the failure; after a fix and a green run it is accepted.
#[test]
fn a_completion_on_red_code_is_denied_while_budget_remains() {
    let _guard = crate::tests::env_lock();
    let _env = root_turn_env();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let _limit = EnvGuard::unset("ANGEL_RED_COMPLETION_DENIALS");
    let root = root_fixture("red_completion");
    let script = Script::new(vec![
        run_tests_call(0),
        Move::Say("sub_id still fails; I could not finish."),
        write_call("fixed", "1"),
        run_tests_call(1),
        Move::Say("All tests pass."),
    ]);
    let (outcome, history, runs) = run_root_turn(&root, &script);
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(
        script.chats(),
        5,
        "the red answer was denied, the green one accepted"
    );
    assert_eq!(runs, 2);
    let denials: Vec<&ChatMsg> = history
        .iter()
        .filter(|m| m.role == ChatRole::Harness && m.content.contains(RED_COMPLETION_NUDGE))
        .collect();
    assert_eq!(denials.len(), 1);
    assert!(
        denials[0].content.contains("sub_id"),
        "{}",
        denials[0].content
    );
    assert!(
        denials[0].content.contains("steps are left"),
        "{}",
        denials[0].content
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A model that is truly stuck can still report: after the denial limit its
/// answer stands.
#[test]
fn a_red_completion_is_accepted_after_the_denial_limit() {
    let _guard = crate::tests::env_lock();
    let _env = root_turn_env();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let _limit = EnvGuard::unset("ANGEL_RED_COMPLETION_DENIALS");
    let root = root_fixture("red_completion_limit");
    let script = Script::new(vec![
        run_tests_call(0),
        Move::Say("Blocked: the fixture is wrong."),
        Move::Say("Blocked: the fixture is wrong."),
        Move::Say("Blocked: the fixture is wrong."),
    ]);
    let (outcome, history, _) = run_root_turn(&root, &script);
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(script.chats(), 4, "two denials, then the answer stands");
    assert_eq!(
        history
            .iter()
            .filter(|m| m.role == ChatRole::Harness && m.content.contains(RED_COMPLETION_NUDGE))
            .count(),
        2
    );
    let _ = std::fs::remove_dir_all(root);
}

/// The check applies only to the code the red run saw, and only in task mode.
#[test]
fn a_red_completion_is_accepted_after_an_edit_or_outside_task_mode() {
    let _guard = crate::tests::env_lock();
    let _env = root_turn_env();
    let _limit = EnvGuard::unset("ANGEL_RED_COMPLETION_DENIALS");

    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let root = root_fixture("red_completion_edited");
    let script = Script::new(vec![
        run_tests_call(0),
        write_call("lib.rs", "pub fn sub() { 0 }\n"),
        Move::Say("Fixed sub_id."),
    ]);
    let (outcome, _, _) = run_root_turn(&root, &script);
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(
        script.chats(),
        3,
        "new code, so the red run no longer describes it"
    );
    let _ = std::fs::remove_dir_all(root);

    let _interactive = EnvGuard::unset("ANGEL_TASK_ACTIVE");
    let root = root_fixture("red_completion_interactive");
    let script = Script::new(vec![run_tests_call(0), Move::Say("sub_id still fails.")]);
    let (outcome, _, _) = run_root_turn(&root, &script);
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    assert_eq!(
        script.chats(),
        2,
        "an interactive answer goes to the person"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// run_tests answering from a fixed list; the last answer repeats.
struct ScriptedSuite {
    answers: Vec<Result<&'static str, &'static str>>,
    next: AtomicUsize,
}

impl Tool for ScriptedSuite {
    fn name(&self) -> &str {
        "run_tests"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "run_tests".into(),
            description: "Run the workspace tests.".into(),
            params: json!({"type": "object"}),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        let index = self.next.fetch_add(1, Ordering::SeqCst);
        let answer = self.answers[index.min(self.answers.len() - 1)];
        answer.map(str::to_string).map_err(str::to_string)
    }
}

/// A shell whose every command exits 0.
struct GreenShell;

impl Tool for GreenShell {
    fn name(&self) -> &str {
        "shell"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "shell".into(),
            description: "Run a shell command.".into(),
            params: json!({"type": "object"}),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        Ok("Ran 4 tests in 0.001s\n\nOK".into())
    }
}

const SCRIPTED_RED: &str = "python3 -m unittest discover -v failed (exit 1)\nFAILED (errors=1)";

/// Task mode: a red run_tests, then `next`, then "All tests pass." Returns how
/// many model calls the turn took (3 = no denial) and whether one was denied.
fn red_then(
    name: &str,
    second: Result<&'static str, &'static str>,
    next: ToolCall,
) -> (usize, bool) {
    let root = root_fixture(name);
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.clone());
    registry.register(Box::new(ScriptedSuite {
        answers: vec![Err(SCRIPTED_RED), second],
        next: AtomicUsize::new(0),
    }));
    registry.register(Box::new(GreenShell));
    let script = Script::new(vec![
        run_tests_call(0),
        Move::Call(next),
        Move::Say("All tests pass."),
    ]);
    let mut history = vec![ChatMsg::user("make the tests pass")];
    let outcome = run_turn_observed(
        &script,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(30),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("turn completes");
    assert_eq!(outcome.stop_reason, TurnStopReason::Answer);
    let denied = history
        .iter()
        .any(|m| m.content.contains(RED_COMPLETION_NUDGE));
    let _ = std::fs::remove_dir_all(root);
    (script.chats(), denied)
}

/// A red run followed by a run that did not fail, on the same code, is not red
/// code. angelX's runner reports a pytest pass without a pinned pytest as
/// Inconclusive rather than Passed, and `tests || fallback` or `tests | tail;
/// ls` carries no verdict; each must still replace the red record. polyglot-v1
/// GLM py-beer-song (and five other GLM passes), GLM py-book-store and Grok
/// py-robot-name ended exactly this way.
#[test]
fn a_run_that_did_not_fail_after_a_red_one_is_not_denied() {
    let _guard = crate::tests::env_lock();
    let _env = root_turn_env();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let _limit = EnvGuard::unset("ANGEL_RED_COMPLETION_DENIALS");
    const UNLABELED_GREEN: &str = "tests: 8 passed, 0 failed, 0 skipped — reward unlabeled \
         (verification inconclusive: no immutable system pytest installation is available)";
    let shell = |command: &str| tc("shell", json!({ "command": command }));
    for (name, second, next) in [
        (
            "red_then_inconclusive_green",
            Ok(UNLABELED_GREEN),
            tc("run_tests", json!({"attempt": 1})),
        ),
        (
            "red_then_fallback_green",
            Err(SCRIPTED_RED),
            shell("python3 -m pytest -q -- robot_name_test.py || python3 robot_name_test.py"),
        ),
        (
            "red_then_unread_green",
            Err(SCRIPTED_RED),
            shell("python3 -m unittest book_store_test -v 2>&1 | tail -6; ls"),
        ),
    ] {
        assert_eq!(red_then(name, second, next), (3, false), "{name}");
    }
}

/// Only a test run replaces the red record: listing files after a red run
/// says nothing about the code, so "done" on it is still denied.
#[test]
fn a_command_that_is_not_a_test_run_keeps_the_red_record() {
    let _guard = crate::tests::env_lock();
    let _env = root_turn_env();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let _limit = EnvGuard::unset("ANGEL_RED_COMPLETION_DENIALS");
    let (chats, denied) = red_then(
        "red_then_ls",
        Err(SCRIPTED_RED),
        tc("shell", json!({"command": "ls; cat lib.rs"})),
    );
    assert!(denied, "the red run still describes this code");
    assert_eq!(chats, 5, "two denials, then the answer stands");
}

#[test]
fn a_test_run_is_found_anywhere_in_a_shell_command() {
    let shell = |command: &str| tc("shell", json!({ "command": command }));
    for command in [
        "cd ws && python3 -m unittest book_store_test -v 2>&1 | tail -6; ls",
        "python3 -m pytest -q || python3 robot_name_test.py",
        "cargo test 2>&1 | tail -20",
        "npx jest forth.spec.js | grep -v PASS",
    ] {
        assert!(shell_runs_tests_anywhere(&shell(command)), "{command}");
    }
    for command in [
        "ls; cat lib.rs",
        "echo 'cargo test'",
        "grep -n pytest README.md",
    ] {
        assert!(!shell_runs_tests_anywhere(&shell(command)), "{command}");
    }
    assert!(!shell_runs_tests_anywhere(&tc("run_tests", json!({}))));
}

#[test]
fn red_completion_denials_default_to_task_mode_only() {
    let _guard = crate::tests::env_lock();
    let _unset = EnvGuard::unset("ANGEL_RED_COMPLETION_DENIALS");
    assert_eq!(red_completion_denial_limit(true, false), 2);
    assert_eq!(red_completion_denial_limit(false, false), 0);
    assert_eq!(
        red_completion_denial_limit(true, true),
        0,
        "never in competition"
    );
    let _set = EnvGuard::set("ANGEL_RED_COMPLETION_DENIALS", "9");
    assert_eq!(red_completion_denial_limit(false, false), 4, "capped at 4");
    assert_eq!(red_completion_denial_limit(true, true), 0);
    let _off = EnvGuard::set("ANGEL_RED_COMPLETION_DENIALS", "0");
    assert_eq!(red_completion_denial_limit(true, false), 0);
}

#[test]
fn a_denied_completion_needs_budget_to_act_on() {
    assert_eq!(
        task_budget_left(10, Some(60), false, 100, 600).as_deref(),
        Some("About 500 s and 50 steps are left.")
    );
    assert_eq!(
        task_budget_left(10, None, false, 0, 0).as_deref(),
        Some("This task has no time or step limit.")
    );
    assert_eq!(
        task_budget_left(10, Some(60), true, 0, 0),
        None,
        "final mile"
    );
    assert_eq!(
        task_budget_left(58, Some(60), false, 0, 0),
        None,
        "two steps left"
    );
    assert_eq!(
        task_budget_left(10, None, false, 451, 600),
        None,
        "less than a quarter of the wall left"
    );
    assert!(task_budget_left(10, None, false, 450, 600).is_some());
}
