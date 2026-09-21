//! Watcher, no-idle cadence, simple-tool-vs-runner, and mined-fixture tests.
//!
//! These drive the shipped functions on redacted fixtures — not a
//! re-implementation and not whole-trace transcripts.

use super::*;

const GOLD_ID: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";

fn call(name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: name.to_string(),
        name: name.to_string(),
        args,
    }
}

fn shell(cmd: &str) -> ToolCall {
    call("shell", serde_json::json!({"command": cmd}))
}

fn write_src() -> ToolCall {
    call(
        "str_replace",
        serde_json::json!({"path": "src/lib.rs", "old": "a", "new": "b"}),
    )
}

fn poll_status() -> ToolCall {
    shell("hilbert submissions 8806afb8-8dfa")
}

fn local_nvcc() -> ToolCall {
    shell("nvcc -arch=sm_100a -O3 -cubin -Xptxas -v -o /dev/null candidate.cu")
}

fn submit() -> ToolCall {
    shell("hilbert submit --note x")
}

#[test]
fn watcher_fixture_inflight_then_terminal_injects_id_status_score() {
    let mut source =
        FixtureStatusSource::from_json(BUILTIN_WATCH_FIXTURE_JSON).expect("builtin fixture parses");
    assert_eq!(source.id, GOLD_ID);
    let notify = run_fixture_watch(&mut source).expect("fixture reaches terminal");
    assert_eq!(notify.id, GOLD_ID);
    assert_eq!(notify.status, "accepted");
    assert_eq!(notify.score.as_deref(), Some("1844075.40"));
    let text = notify.injection_text();
    assert!(text.contains(GOLD_ID), "{text}");
    assert!(text.contains("accepted"), "{text}");
    assert!(text.contains("1844075.40"), "{text}");
    assert!(text.starts_with(WATCHER_NOTIFY_MARK), "{text}");
    assert!(text.starts_with(TELEMETRY_MARK), "{text}");
}

#[test]
fn watcher_fixture_entry_is_deterministic_across_two_runs() {
    let mut a = FixtureStatusSource::from_json(BUILTIN_WATCH_FIXTURE_JSON).unwrap();
    let mut b = FixtureStatusSource::from_json(BUILTIN_WATCH_FIXTURE_JSON).unwrap();
    let n1 = run_fixture_watch(&mut a).unwrap();
    let n2 = run_fixture_watch(&mut b).unwrap();
    assert_eq!(n1, n2);
    assert_eq!(n1.injection_text(), n2.injection_text());
}

#[test]
fn watcher_poll_stays_quiet_while_validating() {
    let mut source = FixtureStatusSource::from_json(BUILTIN_WATCH_FIXTURE_JSON).unwrap();
    let mut watcher = SubmissionWatcher::new();
    watcher.adopt(&source.id);
    let first = watcher.poll(Some(&mut source));
    assert!(first.is_none(), "first snapshot is still validating");
    assert_eq!(watcher.phase(), SlotPhase::InFlight);
    let second = watcher.poll(Some(&mut source));
    assert!(second.is_none(), "second snapshot is still validating");
    let third = watcher.poll(Some(&mut source)).expect("terminal");
    assert_eq!(third.status, "accepted");
    assert_eq!(watcher.phase(), SlotPhase::Terminal);
    assert!(watcher.poll(Some(&mut source)).is_none());
}

#[test]
fn watcher_exposes_truthful_submission_slot_telemetry() {
    let mut watcher = SubmissionWatcher::new();
    assert_eq!(watcher.telemetry(false).phase, SubmissionSlotPhase::Dormant);

    let empty = watcher.telemetry(true);
    assert_eq!(empty.phase, SubmissionSlotPhase::Empty);

    watcher.adopt(GOLD_ID);
    let fired = watcher.telemetry(true);
    assert_eq!(fired.phase, SubmissionSlotPhase::InFlight);
    assert_eq!(fired.id.as_deref(), Some(GOLD_ID));

    watcher.observe_snapshot(SlotSnapshot {
        id: GOLD_ID.into(),
        status: "accepted".into(),
        score: Some("1844075.40".into()),
        rejection_reason: None,
    });
    let accepted = watcher.telemetry(true);
    assert_eq!(accepted.phase, SubmissionSlotPhase::Accepted);
    assert_eq!(accepted.score.as_deref(), Some("1844075.40"));

    watcher.adopt("bbbbbbbb-bbbb-4ccc-8ddd-eeeeeeeeeeee");
    watcher.observe_snapshot(SlotSnapshot {
        id: "bbbbbbbb-bbbb-4ccc-8ddd-eeeeeeeeeeee".into(),
        status: "rejected".into(),
        score: None,
        rejection_reason: Some("below-crown".into()),
    });
    assert_eq!(watcher.telemetry(true).phase, SubmissionSlotPhase::Rejected);
}

#[test]
fn watcher_poll_rejects_other_submission_without_poisoning_current_slot() {
    struct FixedSource(SlotSnapshot);
    impl StatusSource for FixedSource {
        fn probe(&mut self, _id: &str) -> Result<SlotSnapshot, String> {
            Ok(self.0.clone())
        }
    }
    let mut watcher = SubmissionWatcher::new();
    watcher.adopt(GOLD_ID);
    let before = watcher.telemetry(true);
    let mut source = FixedSource(SlotSnapshot {
        id: "bbbbbbbb-bbbb-4ccc-8ddd-eeeeeeeeeeee".into(),
        status: "accepted".into(),
        score: Some("1".into()),
        rejection_reason: None,
    });
    assert!(watcher.poll(Some(&mut source)).is_none());
    assert_eq!(watcher.telemetry(true), before);
    assert!(watcher.pending_notify().is_none());
    source.0.id = GOLD_ID.into();
    let notify = watcher.poll(Some(&mut source)).expect("matching receipt");
    assert_eq!(notify.id, GOLD_ID);
    assert_eq!(notify.score.as_deref(), Some("1"));
}

#[test]
fn watcher_snapshot_parser_cannot_relabel_explicit_submission_id() {
    for observed in [
        serde_json::json!("other-id"),
        serde_json::json!(null),
        serde_json::json!(7),
    ] {
        let value = serde_json::json!({"id": observed, "status": "accepted"});
        let fixture = serde_json::json!({"id": GOLD_ID, "snapshots": [value]});
        assert!(FixtureStatusSource::from_json(&fixture.to_string()).is_err());
    }
    for snapshot in [
        serde_json::json!({"id": GOLD_ID, "status": "accepted"}),
        serde_json::json!({"status": "validating"}),
    ] {
        let fixture = serde_json::json!({"id": GOLD_ID, "snapshots": [snapshot]});
        let mut source = FixtureStatusSource::from_json(&fixture.to_string()).unwrap();
        assert_eq!(source.probe(GOLD_ID).unwrap().id, GOLD_ID);
    }
}

#[test]
fn watcher_unknown_status_never_becomes_an_accepted_receipt() {
    for status in [
        "",
        "unknown",
        "scoring",
        "retrying",
        "completed",
        "not accepted",
    ] {
        let mut watcher = SubmissionWatcher::new();
        watcher.adopt(GOLD_ID);
        let snapshot = SlotSnapshot {
            id: GOLD_ID.into(),
            status: status.into(),
            score: Some("1".into()),
            rejection_reason: None,
        };
        assert!(snapshot.is_in_flight(), "{status:?}");
        watcher.observe_snapshot(snapshot);
        assert_eq!(watcher.phase(), SlotPhase::InFlight, "{status:?}");
        assert_eq!(watcher.telemetry(true).phase, SubmissionSlotPhase::InFlight);
        assert!(watcher.pending_notify().is_none());
        watcher.observe_snapshot(SlotSnapshot {
            id: GOLD_ID.into(),
            status: " accepted ".into(),
            score: Some("2".into()),
            rejection_reason: None,
        });
        assert_eq!(watcher.telemetry(true).phase, SubmissionSlotPhase::Accepted);
        assert!(watcher.pending_notify().is_some());
    }
}

#[test]
fn watcher_rejected_fixture_injects_reason() {
    let json = r#"{
      "id": "bbbbbbbb-bbbb-4ccc-8ddd-eeeeeeeeeeee",
      "snapshots": [
        {"status": "validating"},
        {"status": "failed", "rejectionReason": "workflow-cancelled"}
      ]
    }"#;
    let mut source = FixtureStatusSource::from_json(json).unwrap();
    let notify = run_fixture_watch(&mut source).unwrap();
    let text = notify.injection_text();
    assert!(
        text.contains("bbbbbbbb-bbbb-4ccc-8ddd-eeeeeeeeeeee"),
        "{text}"
    );
    assert!(text.contains("failed"), "{text}");
    assert!(text.contains("workflow-cancelled"), "{text}");
}

#[test]
fn extract_submission_id_prefers_queued_receipt() {
    let text = "\
benchmark 8806afb8-8dfa-4a9d-8f11-4735c5fde7c9
Submission queued
aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee
";
    assert_eq!(
        extract_submission_id(text).as_deref(),
        Some("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee")
    );
}

#[test]
fn watcher_tool_text_cannot_adopt_or_terminalize() {
    let mut watcher = SubmissionWatcher::new();
    observe_tool_result_for_watch(
        &mut watcher,
        "shell",
        "Submission queued aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
        true,
    );
    assert!(watcher.slot_id().is_none());
    watcher.adopt(GOLD_ID);
    assert_eq!(watcher.phase(), SlotPhase::InFlight);
    observe_tool_result_for_watch(
        &mut watcher,
        "shell",
        &format!("VERDICT: {GOLD_ID} accepted score=1844075.40"),
        true,
    );
    assert_eq!(watcher.phase(), SlotPhase::InFlight);
    assert!(watcher.pending_notify().is_none());
}

/// In-flight hops must not allocate a full-text lowercase copy to read a
/// terminal row. Mixed-case verdicts still parse; validating stays in-flight.
#[test]
fn watcher_rejects_live_handoff_and_escaped_history_as_remote_evidence() {
    let id = "a391d01d-6a4f-4121-9e46-cb47ab960e08";
    let sentence = "Actual Yukon corrected-source submission a391d01d-6a4f-4121-9e46-cb47ab960e08 still validating/no score at06:19:57 UTC. a203658 and7518d413 CANCELLED, despite misleading watcher notifications accepted/failed.";
    let escaped = serde_json::json!([
        {"path":"status.md", "text":format!("Submission queued {id}\nstill validating")},
        {"path":"local.log", "text":"Comparator accepted; unrelated submission accepted score=532138"}
    ]).to_string();
    for tool in [
        "handoff",
        "read_file",
        "handle_read",
        "shell",
        "proc_run",
        "proc_status",
    ] {
        let mut watcher = SubmissionWatcher::new();
        for text in [sentence, escaped.as_str()] {
            observe_tool_result_for_watch(&mut watcher, tool, text, true);
            assert!(
                watcher.slot_id().is_none(),
                "{tool} created a slot from prose"
            );
        }
        watcher.adopt(id);
        let before = watcher.telemetry(true);
        for text in [sentence, escaped.as_str()] {
            observe_tool_result_for_watch(&mut watcher, tool, text, true);
            assert_eq!(
                watcher.telemetry(true),
                before,
                "{tool} changed typed evidence"
            );
            assert!(watcher.pending_notify().is_none());
        }
        watcher.observe_snapshot(SlotSnapshot {
            id: id.into(),
            status: "accepted".into(),
            score: Some("532138".into()),
            rejection_reason: None,
        });
        assert_eq!(
            watcher.pending_notify().unwrap().score.as_deref(),
            Some("532138")
        );
    }
}

#[test]
fn snapshot_and_extract_do_not_bleed_opponent_submissions_on_board() {
    let board_table = r#"
        11111111-2222-3333-4444-555555555555  rejected  score: 10.0  reason: SyntaxError
        22222222-3333-4444-5555-666666666666  validating  2026-08-31
        33333333-4444-5555-6666-777777777777  accepted  score: 99.5
    "#;

    // Must not blindly adopt opponent's UUID from board tables without receipt keywords
    assert_eq!(extract_submission_id(board_table), None);

    // Extract when it is an explicit submission receipt
    let receipt = "Submission queued: 22222222-3333-4444-5555-666666666666 in flight as active";
    assert_eq!(
        extract_submission_id(receipt),
        Some("22222222-3333-4444-5555-666666666666".to_string())
    );

    // Harness-history lines are NOT receipts: "Validate"/"Accept submission"
    // commits from `git log` carry "id" only inside "Val-id-ate". Adopting one
    // hands the watcher a phantom in-flight slot (2026-09-01 toymaker: the loop
    // believed repo-log uuid 0e31d3e6 was its own submission, "watched" it to a
    // rejected terminal, and pinned a stale frontier around it — without ever
    // submitting anything).
    let repo_log = "1403bb6 Validate submission 0e31d3e6-4dac-41a4-a4b8-ccefb84fbd52\n\
                    de2469b Accept submission dc6fcf57-2a03-4785-b9a4-929f4ba8fbf0\n\
                    148605a Reject submission 92983207-f6fe-4f8a-ae53-a8f8b246368a";
    assert_eq!(extract_submission_id(repo_log), None);

    // Standalone "id"/"uuid" tokens still read as receipts.
    assert_eq!(
        extract_submission_id("submission id 44444444-5555-4666-8777-888888888888 recorded"),
        Some("44444444-5555-4666-8777-888888888888".to_string())
    );

    // Our slot is validating — must not see opponent 1111's "rejected" or opponent 3333's "accepted"
    let our_id = "22222222-3333-4444-5555-666666666666";
    let mut watcher = SubmissionWatcher::new();
    watcher.adopt(our_id);
    observe_tool_result_for_watch(&mut watcher, "shell", board_table, true);
    assert_eq!(watcher.phase(), SlotPhase::InFlight);
    assert!(watcher.pending_notify().is_none());
}

/// Submit-next after a terminal notify must retarget the same watcher.
/// Slot A is driven to WATCHER NOTIFY, then a real submit receipt for B is
/// observed, then B's fixture is polled to terminal — the injection must
/// name B, not stay latched on A.
#[test]
fn watcher_submit_next_after_notify_retargets_and_injects_new_id() {
    const SLOT_A: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
    const SLOT_B: &str = "cccccccc-dddd-4eee-8fff-aaaaaaaaaaaa";
    let fixture_a = r#"{
      "id": "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
      "snapshots": [
        {"status": "validating"},
        {"status": "accepted", "officialScore": "1844075.40"}
      ]
    }"#;
    let fixture_b = r#"{
      "id": "cccccccc-dddd-4eee-8fff-aaaaaaaaaaaa",
      "snapshots": [
        {"status": "validating"},
        {"status": "rejected", "officialScore": "1839415.55", "rejectionReason": "below-crown"}
      ]
    }"#;

    let mut watcher = SubmissionWatcher::new();
    let mut src_a = FixtureStatusSource::from_json(fixture_a).unwrap();
    watcher.adopt(&src_a.id);
    let notify_a = poll_fixture_until_terminal(&mut watcher, &mut src_a).expect("A terminal");
    assert_eq!(notify_a.id, SLOT_A);
    assert_eq!(notify_a.status, "accepted");
    assert_eq!(notify_a.score.as_deref(), Some("1844075.40"));
    let text_a = notify_a.injection_text();
    assert!(text_a.contains(WATCHER_NOTIFY_MARK), "{text_a}");
    assert!(text_a.contains(SLOT_A), "{text_a}");
    assert!(
        text_a.contains("accepted") && text_a.contains("1844075.40"),
        "{text_a}"
    );
    assert_eq!(watcher.phase(), SlotPhase::Terminal);
    assert!(
        watcher.poll(Some(&mut src_a)).is_none(),
        "already-notified A must not re-fire"
    );

    watcher.adopt(SLOT_B);
    assert_eq!(
        watcher.slot_id(),
        Some(SLOT_B),
        "submit-next receipt must retarget off terminal A"
    );
    assert_eq!(watcher.phase(), SlotPhase::InFlight);

    let mut src_b = FixtureStatusSource::from_json(fixture_b).unwrap();
    let notify_b = poll_fixture_until_terminal(&mut watcher, &mut src_b).expect("B terminal");
    assert_eq!(notify_b.id, SLOT_B);
    assert_eq!(notify_b.status, "rejected");
    assert_eq!(notify_b.score.as_deref(), Some("1839415.55"));
    assert_eq!(notify_b.rejection_reason.as_deref(), Some("below-crown"));
    let text_b = notify_b.injection_text();
    assert!(text_b.contains(WATCHER_NOTIFY_MARK), "{text_b}");
    assert!(text_b.contains(SLOT_B), "{text_b}");
    assert!(
        !text_b.contains(SLOT_A),
        "second notify must not stay on A: {text_b}"
    );
    assert!(
        text_b.contains("rejected")
            && text_b.contains("1839415.55")
            && text_b.contains("below-crown"),
        "{text_b}"
    );
    assert_ne!(text_a, text_b);
}

#[test]
fn edits_and_preflight_intent_do_not_certify_readiness_or_require_submission() {
    let mut watcher = SubmissionWatcher::new();
    let mutate = classify_inflight_hop(&[write_src()]);
    assert_eq!(mutate, InFlightHopKind::MutateCandidate);
    assert_eq!(
        evaluate_inflight_hop(mutate, SlotPhase::Empty, false),
        CadenceVerdict::Accept
    );
    // Actual result text may describe failure, denial, an incomplete launch,
    // a completed edit, or a local test pass. None is a submission receipt or
    // a byte-bound universal proof. Do not create a ready state from any of it.
    let empty = watcher.telemetry(true);
    for result in [
        "ERROR: str_replace old text not found",
        "DENIED: write outside workspace",
        "Started process 42: local preflight is running",
        "Successfully replaced 1 occurrence in src/lib.rs",
        "Local preflight passed: 44 vectors OK; score=532432",
    ] {
        observe_tool_result_for_watch(&mut watcher, "shell", result, true);
        assert_eq!(watcher.telemetry(true), empty, "{result}");
        assert_eq!(
            next_required_action(watcher.phase(), false),
            NextRequiredAction::ImproveCandidate,
            "{result}"
        );
    }
    let poll = classify_inflight_hop(&[poll_status()]);
    assert_eq!(poll, InFlightHopKind::PollStatus);
    assert_eq!(
        evaluate_inflight_hop(poll, SlotPhase::Empty, false),
        CadenceVerdict::Accept
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::Idle, SlotPhase::Empty, false),
        CadenceVerdict::Accept
    );
    let submit = classify_inflight_hop(&[submit()]);
    assert_eq!(
        evaluate_inflight_hop(submit, SlotPhase::Empty, false),
        CadenceVerdict::Accept
    );
    watcher.adopt(GOLD_ID);
    assert_eq!(watcher.phase(), SlotPhase::InFlight);
    let improve = classify_inflight_hop(&[write_src()]);
    assert_eq!(
        evaluate_inflight_hop(improve, SlotPhase::InFlight, false),
        CadenceVerdict::Accept,
        "revolving door: improve the next best while the current bat is in flight"
    );

    for id in [
        "gold-always-be-improving-best-to-bat",
        "gold-revolving-door-improve-while-inflight",
        "regression-preflight-intent-is-not-readiness",
        "regression-edit-intent-is-not-readiness",
    ] {
        let fix = MINED_CADENCE_FIXTURES
            .iter()
            .find(|f| f.id == id)
            .unwrap_or_else(|| panic!("missing fixture {id}"));
        assert_eq!(fix.kind, CadenceKind::AlwaysBeImproving);
        let verdicts = evaluate_mined_fixture(fix);
        assert_eq!(verdicts.len(), fix.hops.len(), "{id}");
        for (spec, got) in fix.hops.iter().zip(verdicts.iter()) {
            assert_eq!(
                *got, spec.expected,
                "{id} hop {} {}",
                spec.tool, spec.args_hint
            );
        }
    }
}

#[test]
fn live_turn_failed_or_successful_tools_never_emit_submission_readiness() {
    let _guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _competition = EnvGuard::set("ANGEL_COMPETITION_MODE", "1");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _skills = EnvGuard::set("ANGEL_SKILL_HINT", "0");
    let root = scratch("watcher_readiness");

    struct ResultTool {
        name: String,
        result: Result<String, String>,
        calls: Arc<AtomicUsize>,
    }
    impl Tool for ResultTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name.clone(),
                description: "controlled result for watcher dispatch regression".into(),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.result.clone()
        }
    }
    struct OneTool {
        hop: AtomicUsize,
        tool: ToolCall,
    }
    impl Club for OneTool {
        fn label(&self) -> &str {
            "watcher-readiness-regression"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hop.fetch_add(1, Ordering::Relaxed) == 0 {
                Ok(ClubReply::Calls(vec![self.tool.clone()]))
            } else {
                Ok(ClubReply::Text(
                    "Candidate remains unverified; no submission was made.".into(),
                ))
            }
        }
    }
    for (tool, result) in [
        (write_src(), Err("old text not found".to_string())),
        (
            write_src(),
            Err("DENIED: write outside workspace".to_string()),
        ),
        (
            write_src(),
            Ok("Successfully replaced 1 occurrence".to_string()),
        ),
        (
            local_nvcc(),
            Ok("Started process 42; preflight still running".to_string()),
        ),
        (local_nvcc(), Ok("Local preflight passed".to_string())),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut registry = ToolRegistry::new();
        registry.set_workspace(root.clone());
        registry.register(Box::new(ResultTool {
            name: tool.name.clone(),
            result,
            calls: Arc::clone(&calls),
        }));
        let (events, received) = mpsc::channel();
        run_turn_observed(
            &OneTool {
                hop: AtomicUsize::new(0),
                tool,
            },
            &registry,
            &mut vec![ChatMsg::user(
                "Inspect this competition candidate; report its unverified state.",
            )],
            &AtomicBool::new(false),
            Some(3),
            &events,
        )
        .expect("controlled turn completes");
        assert_eq!(calls.load(Ordering::Relaxed), 1, "tool actually dispatched");
        let mut slots = 0;
        for event in received.try_iter() {
            match event {
                TurnEvent::SubmissionSlot(slot) => {
                    slots += 1;
                    assert_eq!(slot.phase, SubmissionSlotPhase::Empty);
                    assert!(slot.id.is_none() && slot.score.is_none());
                }
                TurnEvent::Notice(note) => {
                    assert!(!note.contains("best goes to bat"), "{note}");
                    assert!(!note.contains("SubmitNext"), "{note}");
                    assert!(!note.contains("FailSitOnPrepped"), "{note}");
                }
                _ => {}
            }
        }
        assert!(slots > 0, "actual live telemetry was emitted");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mutate_plus_local_preflight_while_inflight_is_accepted() {
    let mut watcher = SubmissionWatcher::new();
    watcher.adopt(GOLD_ID);
    assert_eq!(watcher.phase(), SlotPhase::InFlight);
    let hops = [
        vec![write_src()],
        vec![local_nvcc()],
        vec![write_src(), local_nvcc()],
    ];
    for calls in hops {
        let kind = classify_inflight_hop(&calls);
        let verdict = evaluate_inflight_hop(kind, watcher.phase(), false);
        assert_eq!(verdict, CadenceVerdict::Accept, "kind={kind:?}");
        assert!(!verdict.is_fail());
    }
}

#[test]
fn poll_only_or_idle_while_inflight_fails() {
    let mut watcher = SubmissionWatcher::new();
    watcher.adopt(GOLD_ID);
    let poll = classify_inflight_hop(&[poll_status()]);
    assert_eq!(poll, InFlightHopKind::PollStatus);
    assert_eq!(
        evaluate_inflight_hop(poll, SlotPhase::InFlight, false),
        CadenceVerdict::FailPollOnly
    );
    let idle = classify_inflight_hop(&[]);
    assert_eq!(idle, InFlightHopKind::Idle);
    assert_eq!(
        evaluate_inflight_hop(idle, SlotPhase::InFlight, false),
        CadenceVerdict::FailIdle
    );
}

#[test]
fn after_watcher_notify_receipt_check_is_accepted_then_improve_or_submit() {
    let mut watcher = SubmissionWatcher::new();
    watcher.adopt(GOLD_ID);
    watcher.observe_snapshot(SlotSnapshot {
        id: GOLD_ID.into(),
        status: "accepted".into(),
        score: Some("1844075.40".into()),
        rejection_reason: None,
    });
    assert_eq!(watcher.phase(), SlotPhase::Terminal);
    let just = watcher.take_just_notified();
    assert!(just);
    let receipt = classify_inflight_hop(&[poll_status()]);
    assert_eq!(
        evaluate_inflight_hop(receipt, SlotPhase::Terminal, just),
        CadenceVerdict::AcceptReceipt
    );
    assert_eq!(
        next_required_action(SlotPhase::Terminal, just),
        NextRequiredAction::ReceiptCheck
    );
    assert_eq!(
        next_required_action(SlotPhase::Terminal, false),
        NextRequiredAction::ImproveCandidate
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::MutateCandidate, SlotPhase::Terminal, false),
        CadenceVerdict::Accept
    );
}

#[test]
fn idle_on_submit_and_recon_thrash_are_competition_failures() {
    let idle_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-idle-on-submit")
        .unwrap();
    let verdicts = evaluate_mined_fixture(idle_fix);
    assert_eq!(verdicts[0], CadenceVerdict::Accept);
    assert_eq!(verdicts[1], CadenceVerdict::FailPollOnly);

    let recon_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-recon-thrash-as-progress")
        .unwrap();
    let verdicts = evaluate_mined_fixture(recon_fix);
    assert_eq!(verdicts[0], CadenceVerdict::Accept);
    assert_eq!(verdicts[1], CadenceVerdict::FailReconThrash);

    let recon = classify_inflight_hop(&[call("grep", serde_json::json!({"pattern": "TODO"}))]);
    assert_eq!(recon, InFlightHopKind::Recon);
    assert_eq!(
        evaluate_inflight_hop(recon, SlotPhase::InFlight, false),
        CadenceVerdict::FailReconThrash
    );
}

#[test]
fn simple_tool_preflight_is_required_before_runner() {
    let preflight = local_nvcc();
    let runner = submit();
    let yukon_runner = shell("yukon submit --note-file submission.md --model 'GPT 5.6 Sol'");
    assert_eq!(classify_tool_lane(&preflight), ToolLane::SimpleLocal);
    assert_eq!(classify_tool_lane(&runner), ToolLane::RunnerDispatch);
    assert_eq!(classify_tool_lane(&yukon_runner), ToolLane::RunnerDispatch);
    let sneaky = call(
        "write_file",
        serde_json::json!({
            "path": "src/lib.rs",
            "content": "hilbert submit --note x\npopcorn submit\n".repeat(200)
        }),
    );
    assert_eq!(
        classify_tool_lane(&sneaky),
        ToolLane::SimpleLocal,
        "write payload must not launder as runner dispatch"
    );
    assert!(is_local_preflight_call(&preflight));
    assert!(!runner_escalation_allowed(false, &runner));
    assert!(runner_escalation_allowed(true, &runner));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-local-preflight-before-runner")
        .unwrap();
    let (saw_preflight, allowed) = fixture_runner_gate(gold);
    assert!(saw_preflight, "gold fixture must preflight before submit");
    assert!(allowed, "runner after preflight must be allowed");

    let waste = CadenceFixture {
        id: "waste",
        kind: CadenceKind::LocalPreflightBeforeRunner,
        hops: &[HopSpec {
            tool: "shell",
            args_hint: "hilbert submit --note x",
            expected: CadenceVerdict::Accept,
        }],
    };
    let (saw_preflight, allowed) = fixture_runner_gate(&waste);
    assert!(!saw_preflight);
    assert!(!allowed, "bare submit without preflight is runner waste");
}

#[test]
fn mined_notify_cadence_accepts_receipt_then_submit_next() {
    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-receipt-then-submit-next")
        .expect("gold notify fixture");
    assert_eq!(gold.kind, CadenceKind::ReceiptThenSubmitNext);
    let seeded = watcher_after_terminal_notify(GOLD_ID);
    assert_eq!(seeded.phase(), SlotPhase::Terminal);
    assert!(seeded.pending_notify().is_some());
    let verdicts = evaluate_notify_fixture(gold);
    assert_eq!(verdicts.len(), gold.hops.len());
    for (spec, got) in gold.hops.iter().zip(verdicts.iter()) {
        assert_eq!(
            *got, spec.expected,
            "gold hop {} {}",
            spec.tool, spec.args_hint
        );
    }
    assert_eq!(
        next_required_action(SlotPhase::Terminal, true),
        NextRequiredAction::ReceiptCheck
    );
    assert_eq!(
        next_required_action(SlotPhase::Terminal, false),
        NextRequiredAction::ImproveCandidate
    );

    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-re-poll-after-receipt")
        .expect("re-poll fixture");
    let waste_verdicts = evaluate_notify_fixture(waste);
    assert_eq!(
        waste_verdicts,
        vec![CadenceVerdict::AcceptReceipt, CadenceVerdict::FailPollOnly]
    );
}

#[test]
fn mined_gold_cadences_accept_mutate_preflight_and_submit() {
    for id in [
        "gold-no-idle-while-inflight",
        "gold-local-preflight-before-runner",
        "gold-outcome-only-progress",
    ] {
        let fix = MINED_CADENCE_FIXTURES
            .iter()
            .find(|f| f.id == id)
            .unwrap_or_else(|| panic!("missing fixture {id}"));
        assert!(matches!(
            fix.kind,
            CadenceKind::NoIdleWhileInFlight
                | CadenceKind::LocalPreflightBeforeRunner
                | CadenceKind::OutcomeOnlyProgress
        ));
        let verdicts = evaluate_mined_fixture(fix);
        assert_eq!(verdicts.len(), fix.hops.len(), "{id}");
        for (spec, got) in fix.hops.iter().zip(verdicts.iter()) {
            assert_eq!(
                *got, spec.expected,
                "{id} hop {} {}",
                spec.tool, spec.args_hint
            );
        }
    }
}

#[test]
fn board_outline_is_recon_not_preflight() {
    let outline = call("outline", serde_json::json!({"path": "LIVING_HANDOFF.md"}));
    let defs = call(
        "defs",
        serde_json::json!({"path": ".angelX/notes/board.md"}),
    );
    let product = call(
        "outline",
        serde_json::json!({"path": "src/living_handoff_parser.rs"}),
    );
    assert!(
        !is_local_preflight_call(&outline),
        "outline of board is recon, not candidate preflight"
    );
    assert!(!is_local_preflight_call(&defs));
    assert!(
        is_local_preflight_call(&product),
        "product outline must remain preflight"
    );
    assert!(burns_first_write_budget(&outline));
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&outline)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        evaluate_inflight_hop(
            classify_inflight_hop(std::slice::from_ref(&outline)),
            SlotPhase::InFlight,
            false
        ),
        CadenceVerdict::FailReconThrash
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&outline), &submit()),
        "board outline must not launder runner escalation"
    );

    let fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-outline-as-recon")
        .expect("board-outline fixture");
    assert_eq!(
        evaluate_mined_fixture(fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

#[test]
fn pathless_defs_is_recon_not_preflight() {
    let symbol = call("defs", serde_json::json!({"symbol": "board_tip"}));
    let empty_outline = call("outline", serde_json::json!({}));
    let product = call("defs", serde_json::json!({"path": "src/kernel.cu"}));
    assert!(
        !is_local_preflight_call(&symbol),
        "defs(symbol=…) is workspace recon, not candidate preflight"
    );
    assert!(!is_local_preflight_call(&empty_outline));
    assert!(
        is_local_preflight_call(&product),
        "path-scoped product defs stay preflight"
    );
    assert!(burns_first_write_budget(&symbol));
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&symbol)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::Recon, SlotPhase::InFlight, false),
        CadenceVerdict::FailReconThrash
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&symbol), &submit()),
        "path-less defs must not launder runner escalation"
    );

    let fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-pathless-defs-as-preflight")
        .expect("pathless-defs fixture");
    assert_eq!(
        evaluate_mined_fixture(fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Native grep of a product file is candidate inspect (same class as
/// read_file/outline). Workspace grep and board-path grep stay recon.
#[test]
fn path_scoped_product_grep_is_preflight() {
    let product = call(
        "grep",
        serde_json::json!({"pattern": "stream", "path": "src/kernel.cu"}),
    );
    let workspace = call("grep", serde_json::json!({"pattern": "TODO"}));
    let board = call(
        "grep",
        serde_json::json!({"pattern": "stream", "path": "LIVING_HANDOFF.md"}),
    );
    assert!(
        is_local_preflight_call(&product),
        "path-scoped product grep is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "path-less workspace grep stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "grep of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::LocalPreflight, SlotPhase::InFlight, false),
        CadenceVerdict::Accept
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product grep may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "workspace grep must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-path-grep-while-inflight")
        .expect("path-grep gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-grep-as-preflight")
        .expect("board-grep fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Path-scoped list_dir of a product directory is candidate inspect.
/// Workspace-root list_dir and board-path list_dir stay recon.
#[test]
fn path_scoped_list_dir_is_preflight() {
    let product = call("list_dir", serde_json::json!({"path": "src"}));
    let workspace = call("list_dir", serde_json::json!({}));
    let board = call("list_dir", serde_json::json!({"path": "LIVING_HANDOFF.md"}));
    assert!(
        is_local_preflight_call(&product),
        "list_dir of a product dir is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "path-less list_dir stays workspace recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "list_dir of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product list_dir may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "workspace list_dir must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-list-dir-while-inflight")
        .expect("list-dir gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-list-dir-as-preflight")
        .expect("board-list-dir fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Path-scoped find_files of a product tree is candidate inspect.
/// Workspace inventory globs and board-name globs stay recon.
#[test]
fn path_scoped_find_files_is_preflight() {
    assert!(find_files_pattern_is_path_scoped("src/**/*.cu"));
    assert!(find_files_pattern_is_path_scoped("src/kernel.cu"));
    assert!(
        find_files_pattern_is_path_scoped("src\\kernel.cu"),
        "backslash src\\kernel.cu still counts as path-scoped"
    );
    assert!(!find_files_pattern_is_path_scoped("**/*.cu"));
    assert!(!find_files_pattern_is_path_scoped("*.rs"));
    assert!(!find_files_pattern_is_path_scoped("LIVING_HANDOFF.md"));
    assert!(!find_files_pattern_is_path_scoped(""));
    assert!(
        shell_hay_is_product_inspect("cat src\\kernel.cu"),
        "backslash inspect path still counts as product inspect"
    );
    assert!(
        !shell_hay_is_product_inspect("cat \\tmp\\kernel.cu"),
        "absolute backslash dump stays recon"
    );

    let product = call("find_files", serde_json::json!({"pattern": "src/**/*.cu"}));
    let workspace = call("find_files", serde_json::json!({"pattern": "**/*.cu"}));
    let board = call(
        "find_files",
        serde_json::json!({"pattern": "LIVING_HANDOFF.md"}),
    );
    assert!(
        is_local_preflight_call(&product),
        "path-scoped find_files is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace inventory find_files stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "find_files of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product find_files may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "workspace find_files must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-find-files-while-inflight")
        .expect("find-files gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-find-files-as-preflight")
        .expect("workspace-find-files fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-find-files-as-preflight")
        .expect("board-find-files fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Path-scoped file_search of a product fragment is candidate inspect.
/// Workspace name fragments and board-name queries stay recon.
#[test]
fn path_scoped_file_search_is_preflight() {
    assert!(file_search_query_is_path_scoped("src/kernel"));
    assert!(!file_search_query_is_path_scoped("kernel"));
    assert!(!file_search_query_is_path_scoped("LIVING_HANDOFF.md"));
    assert!(!file_search_query_is_path_scoped(""));

    let product = call("file_search", serde_json::json!({"query": "src/kernel"}));
    let workspace = call("file_search", serde_json::json!({"query": "kernel"}));
    let board = call(
        "file_search",
        serde_json::json!({"query": "LIVING_HANDOFF.md"}),
    );
    assert!(
        is_local_preflight_call(&product),
        "path-scoped file_search is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace name-fragment file_search stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "file_search of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product file_search may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "workspace file_search must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-file-search-while-inflight")
        .expect("file-search gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-file-search-as-preflight")
        .expect("workspace-file-search fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-file-search-as-preflight")
        .expect("board-file-search fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// `cat`/`head` of a product path is candidate inspect (submit then review
/// the tree). Workspace basenames and board files stay recon.
#[test]
fn path_scoped_shell_cat_is_preflight() {
    assert!(shell_hay_is_product_inspect("cat src/kernel.cu"));
    assert!(shell_hay_is_product_inspect("head -n 80 src/kernel.cu"));
    assert!(shell_hay_is_product_inspect("tail -20 ./src/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("cat kernel.cu"));
    assert!(!shell_hay_is_product_inspect("cat LIVING_HANDOFF.md"));
    assert!(!shell_hay_is_product_inspect("cat /tmp/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("cat src/kernel.cu | wc -l"));
    assert!(!shell_hay_is_product_inspect(""));

    let product = shell("cat src/kernel.cu");
    let head = shell("head -n 80 src/kernel.cu");
    let workspace = shell("cat kernel.cu");
    let board = shell("cat LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped cat is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&head),
        "path-scoped head is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename cat stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "cat of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board cat stays legal wait/poll, not preflight"
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product cat may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "workspace cat must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-cat-while-inflight")
        .expect("shell-cat gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-cat-as-preflight")
        .expect("workspace-shell-cat fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-cat-as-preflight")
        .expect("board-shell-cat fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `sed -n 1,80p src/kernel.cu` is the same class as path-scoped cat/head.
/// In-place `-i`, pathless dumps, and board sed stay recon / poll.
#[test]
fn path_scoped_shell_sed_is_preflight() {
    assert!(shell_hay_is_product_inspect("sed -n 1,80p src/kernel.cu"));
    assert!(shell_hay_is_product_inspect(
        "sed -ne 1,80p ./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_inspect(
        "sed --quiet 1,80p src/kernel.cu"
    ));
    assert!(shell_sed_is_quiet_print("sed -n 1,80p src/kernel.cu"));
    assert!(shell_sed_is_inplace("sed -i s/a/b/ src/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("sed -n 1,80p kernel.cu"));
    assert!(!shell_hay_is_product_inspect(
        "sed -n 1,80p LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_inspect("sed -i s/a/b/ src/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("sed 1,80p src/kernel.cu"));
    assert!(!shell_hay_is_product_inspect(
        "sed -n 1,80p src/kernel.cu | wc -l"
    ));

    let product = shell("sed -n 1,80p src/kernel.cu");
    let inplace = shell("sed -i s/a/b/ src/kernel.cu");
    let workspace = shell("sed -n 1,80p kernel.cu");
    let board = shell("sed -n 1,80p LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped sed -n is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&inplace),
        "sed -i must not launder as preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename sed stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "sed of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&inplace)),
        InFlightHopKind::Recon,
        "in-place sed is recon, not mutate-via-shell"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board sed stays legal wait/poll, not preflight"
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product sed -n may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&inplace), &submit()),
        "sed -i must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-sed-while-inflight")
        .expect("shell-sed gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-sed-as-preflight")
        .expect("workspace-shell-sed fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let inplace_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-inplace-shell-sed-as-preflight")
        .expect("inplace-shell-sed fixture");
    assert_eq!(
        evaluate_mined_fixture(inplace_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-sed-as-preflight")
        .expect("board-shell-sed fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `stat`/`file` of a product path is the same class as path-scoped cat.
/// Workspace basenames and board files stay recon / poll.
#[test]
fn path_scoped_shell_stat_is_preflight() {
    assert!(shell_hay_is_product_inspect("stat src/kernel.cu"));
    assert!(shell_hay_is_product_inspect("stat -c %s ./src/kernel.cu"));
    assert!(shell_hay_is_product_inspect("file src/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("stat kernel.cu"));
    assert!(!shell_hay_is_product_inspect("file kernel.cu"));
    assert!(!shell_hay_is_product_inspect("stat LIVING_HANDOFF.md"));
    assert!(!shell_hay_is_product_inspect("stat /tmp/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("stat src/kernel.cu | cat"));

    let product = shell("stat src/kernel.cu");
    let typed = shell("file src/kernel.cu");
    let workspace = shell("stat kernel.cu");
    let board = shell("stat LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped stat is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&typed),
        "path-scoped file is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename stat stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "stat of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board stat stays legal wait/poll, not preflight"
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product stat may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "workspace stat must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-stat-while-inflight")
        .expect("shell-stat gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-stat-as-preflight")
        .expect("workspace-shell-stat fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-stat-as-preflight")
        .expect("board-shell-stat fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `xxd`/`od`/`hexdump`/`md5sum` of a product path is the same class as
/// path-scoped cat. Workspace basenames and board files stay recon / poll.
#[test]
fn path_scoped_shell_xxd_is_preflight() {
    assert!(shell_hay_is_product_inspect("xxd src/kernel.cu"));
    assert!(shell_hay_is_product_inspect("od -Ax -tx1 ./src/kernel.cu"));
    assert!(shell_hay_is_product_inspect("hexdump -C src/kernel.cu"));
    assert!(shell_hay_is_product_inspect("md5sum src/kernel.cu"));
    assert!(shell_hay_is_product_inspect("sha256sum ./src/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("xxd kernel.cu"));
    assert!(!shell_hay_is_product_inspect("md5sum kernel.cu"));
    assert!(!shell_hay_is_product_inspect("xxd LIVING_HANDOFF.md"));
    assert!(!shell_hay_is_product_inspect("xxd /tmp/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("xxd src/kernel.cu | head"));

    let product = shell("xxd src/kernel.cu");
    let digest = shell("md5sum src/kernel.cu");
    let workspace = shell("xxd kernel.cu");
    let board = shell("md5sum LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped xxd is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&digest),
        "path-scoped md5sum is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename xxd stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "md5sum of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board checksum stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-xxd-while-inflight")
        .expect("shell-xxd gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-xxd-as-preflight")
        .expect("workspace-shell-xxd fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-md5sum-as-preflight")
        .expect("board-shell-md5sum fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `nm`/`objdump`/`readelf`/`size`/`base64` of a product path is the same
/// class as path-scoped xxd. Workspace basenames and board files stay recon /
/// poll.
#[test]
fn path_scoped_shell_objdump_is_preflight() {
    assert!(shell_hay_is_product_inspect("nm src/kernel.o"));
    assert!(shell_hay_is_product_inspect("objdump -d ./src/kernel.o"));
    assert!(shell_hay_is_product_inspect("readelf -h src/kernel.o"));
    assert!(shell_hay_is_product_inspect("size src/kernel.o"));
    assert!(shell_hay_is_product_inspect("base64 src/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("nm kernel.o"));
    assert!(!shell_hay_is_product_inspect("objdump -d kernel.o"));
    assert!(!shell_hay_is_product_inspect("nm LIVING_HANDOFF.md"));
    assert!(!shell_hay_is_product_inspect("objdump -d /tmp/kernel.o"));
    assert!(!shell_hay_is_product_inspect(
        "objdump -d src/kernel.o | head"
    ));

    let product = shell("objdump -d src/kernel.o");
    let symbols = shell("nm src/kernel.o");
    let workspace = shell("objdump -d kernel.o");
    let board = shell("nm LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped objdump is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&symbols),
        "path-scoped nm is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename objdump stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "nm of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board nm stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-objdump-while-inflight")
        .expect("shell-objdump gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-objdump-as-preflight")
        .expect("workspace-shell-objdump fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-nm-as-preflight")
        .expect("board-shell-nm fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `llvm-objdump`/`llvm-nm`/`llvm-readelf`/`llvm-size` of a product path is
/// the same class as GNU objdump/nm. Workspace basenames and board files stay
/// recon / poll.
#[test]
fn path_scoped_shell_llvm_objdump_is_preflight() {
    assert!(shell_hay_is_product_inspect("llvm-nm src/kernel.o"));
    assert!(shell_hay_is_product_inspect(
        "llvm-objdump -d ./src/kernel.o"
    ));
    assert!(shell_hay_is_product_inspect("llvm-readelf -h src/kernel.o"));
    assert!(shell_hay_is_product_inspect("llvm-size src/kernel.o"));
    assert!(!shell_hay_is_product_inspect("llvm-nm kernel.o"));
    assert!(!shell_hay_is_product_inspect("llvm-objdump -d kernel.o"));
    assert!(!shell_hay_is_product_inspect("llvm-nm LIVING_HANDOFF.md"));
    assert!(!shell_hay_is_product_inspect(
        "llvm-objdump -d /tmp/kernel.o"
    ));
    assert!(!shell_hay_is_product_inspect(
        "llvm-objdump -d src/kernel.o | head"
    ));

    let product = shell("llvm-objdump -d src/kernel.o");
    let symbols = shell("llvm-nm src/kernel.o");
    let workspace = shell("llvm-objdump -d kernel.o");
    let board = shell("llvm-nm LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped llvm-objdump is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&symbols),
        "path-scoped llvm-nm is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename llvm-objdump stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "llvm-nm of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board llvm-nm stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-llvm-objdump-while-inflight")
        .expect("shell-llvm-objdump gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-llvm-objdump-as-preflight")
        .expect("workspace-shell-llvm-objdump fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-llvm-nm-as-preflight")
        .expect("board-shell-llvm-nm fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `addr2line`/`eu-nm`/`eu-readelf`/`eu-objdump`/`llvm-dwarfdump` of a
/// product path is the same class as GNU/LLVM objdump. Workspace basenames
/// and board files stay recon / poll.
#[test]
fn path_scoped_shell_addr2line_is_preflight() {
    assert!(shell_hay_is_product_inspect("addr2line -e src/kernel.o"));
    assert!(shell_hay_is_product_inspect(
        "addr2line -e ./src/kernel.o 0x401000"
    ));
    assert!(shell_hay_is_product_inspect("eu-nm src/kernel.o"));
    assert!(shell_hay_is_product_inspect("eu-readelf -h src/kernel.o"));
    assert!(shell_hay_is_product_inspect("eu-objdump -d src/kernel.o"));
    assert!(shell_hay_is_product_inspect("llvm-dwarfdump src/kernel.o"));
    assert!(!shell_hay_is_product_inspect("addr2line -e kernel.o"));
    assert!(!shell_hay_is_product_inspect("eu-nm kernel.o"));
    assert!(!shell_hay_is_product_inspect("eu-nm LIVING_HANDOFF.md"));
    assert!(!shell_hay_is_product_inspect("addr2line -e /tmp/kernel.o"));
    assert!(!shell_hay_is_product_inspect(
        "addr2line -e src/kernel.o | head"
    ));

    let product = shell("addr2line -e src/kernel.o");
    let symbols = shell("eu-nm src/kernel.o");
    let workspace = shell("addr2line -e kernel.o");
    let board = shell("eu-nm LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped addr2line is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&symbols),
        "path-scoped eu-nm is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename addr2line stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "eu-nm of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board eu-nm stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-addr2line-while-inflight")
        .expect("shell-addr2line gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-addr2line-as-preflight")
        .expect("workspace-shell-addr2line fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-eu-nm-as-preflight")
        .expect("board-shell-eu-nm fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `llvm-addr2line`/`eu-addr2line`/`dwarfdump`/`rust-objdump` of a product
/// path is the same class as GNU addr2line. Workspace basenames and board
/// files stay recon / poll.
#[test]
fn path_scoped_shell_llvm_addr2line_is_preflight() {
    assert!(shell_hay_is_product_inspect(
        "llvm-addr2line -e src/kernel.o"
    ));
    assert!(shell_hay_is_product_inspect(
        "eu-addr2line -e ./src/kernel.o 0x401000"
    ));
    assert!(shell_hay_is_product_inspect("dwarfdump src/kernel.o"));
    assert!(shell_hay_is_product_inspect("eu-size src/kernel.o"));
    assert!(shell_hay_is_product_inspect("rust-objdump -d src/kernel.o"));
    assert!(shell_hay_is_product_inspect("rust-nm src/kernel.o"));
    assert!(shell_hay_is_product_inspect(
        "rust-addr2line -e src/kernel.o"
    ));
    assert!(!shell_hay_is_product_inspect("llvm-addr2line -e kernel.o"));
    assert!(!shell_hay_is_product_inspect("dwarfdump kernel.o"));
    assert!(!shell_hay_is_product_inspect(
        "eu-addr2line LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_inspect(
        "llvm-addr2line -e /tmp/kernel.o"
    ));
    assert!(!shell_hay_is_product_inspect(
        "llvm-addr2line -e src/kernel.o | head"
    ));

    let product = shell("llvm-addr2line -e src/kernel.o");
    let symbols = shell("rust-nm src/kernel.o");
    let workspace = shell("llvm-addr2line -e kernel.o");
    let board = shell("eu-addr2line LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped llvm-addr2line is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&symbols),
        "path-scoped rust-nm is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename llvm-addr2line stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "eu-addr2line of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board eu-addr2line stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-llvm-addr2line-while-inflight")
        .expect("shell-llvm-addr2line gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-llvm-addr2line-as-preflight")
        .expect("workspace-shell-llvm-addr2line fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-eu-addr2line-as-preflight")
        .expect("board-shell-eu-addr2line fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `llvm-symbolizer --obj=src/kernel.o` is the same class as addr2line.
/// `--obj=` / `--exe=` glue is a product path, not a skippable flag.
#[test]
fn path_scoped_shell_llvm_symbolizer_is_preflight() {
    assert_eq!(
        shell_inspect_flag_path("--obj=src/kernel.o"),
        Some("src/kernel.o")
    );
    assert_eq!(
        shell_inspect_flag_path("--exe=./src/kernel.o"),
        Some("./src/kernel.o")
    );
    assert_eq!(
        shell_inspect_flag_path("-e=src/kernel.o"),
        Some("src/kernel.o")
    );
    assert_eq!(shell_inspect_flag_path("--obj"), None);
    assert_eq!(shell_inspect_flag_path("-e"), None);
    assert_eq!(shell_inspect_flag_path("src/kernel.o"), None);

    assert!(shell_hay_is_product_inspect(
        "llvm-symbolizer --obj=src/kernel.o"
    ));
    assert!(shell_hay_is_product_inspect(
        "llvm-symbolizer --obj src/kernel.o"
    ));
    assert!(shell_hay_is_product_inspect(
        "addr2line --exe=src/kernel.o 0x401000"
    ));
    assert!(!shell_hay_is_product_inspect(
        "llvm-symbolizer --obj=kernel.o"
    ));
    assert!(!shell_hay_is_product_inspect(
        "llvm-symbolizer --obj=LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_inspect(
        "llvm-symbolizer --obj=/tmp/kernel.o"
    ));
    assert!(!shell_hay_is_product_inspect(
        "llvm-symbolizer --obj=src/kernel.o | head"
    ));

    let product = shell("llvm-symbolizer --obj=src/kernel.o");
    let split = shell("llvm-symbolizer --obj src/kernel.o");
    let workspace = shell("llvm-symbolizer --obj=kernel.o");
    let board = shell("llvm-symbolizer --obj=LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped llvm-symbolizer --obj= is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&split),
        "llvm-symbolizer --obj PATH still counts"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename --obj= stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "board --obj= must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board llvm-symbolizer stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-llvm-symbolizer-while-inflight")
        .expect("shell-llvm-symbolizer gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-llvm-symbolizer-as-preflight")
        .expect("workspace-shell-llvm-symbolizer fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-llvm-symbolizer-as-preflight")
        .expect("board-shell-llvm-symbolizer fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `diff`/`cmp`/`strings` of a product path is the same class as path-scoped
/// cat. Workspace basenames and board files stay recon / poll.
#[test]
fn path_scoped_shell_diff_is_preflight() {
    assert!(shell_hay_is_product_inspect(
        "diff -u src/kernel.cu src/ref.cu"
    ));
    assert!(shell_hay_is_product_inspect(
        "cmp ./src/kernel.cu src/ref.cu"
    ));
    assert!(shell_hay_is_product_inspect("strings src/kernel.cu"));
    assert!(!shell_hay_is_product_inspect("diff kernel.cu"));
    assert!(!shell_hay_is_product_inspect("strings kernel.cu"));
    assert!(!shell_hay_is_product_inspect("diff LIVING_HANDOFF.md"));
    assert!(!shell_hay_is_product_inspect("diff /tmp/kernel.cu"));
    assert!(!shell_hay_is_product_inspect(
        "diff -u src/kernel.cu | head"
    ));

    let product = shell("diff -u src/kernel.cu src/ref.cu");
    let dump = shell("strings src/kernel.cu");
    let workspace = shell("diff kernel.cu");
    let board = shell("diff LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped diff is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&dump),
        "path-scoped strings is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "workspace basename diff stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "diff of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board diff stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-diff-while-inflight")
        .expect("shell-diff gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-diff-as-preflight")
        .expect("workspace-shell-diff fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-diff-as-preflight")
        .expect("board-shell-diff fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `ls src` is the same class as path-scoped list_dir. Bare `ls` stays
/// recon; board listings stay poll and cannot arm submit.
#[test]
fn path_scoped_shell_ls_is_preflight() {
    assert!(shell_hay_is_product_ls("ls src"));
    assert!(shell_hay_is_product_ls("ls src/kernel"));
    assert!(shell_hay_is_product_ls("ls -la src"));
    assert!(!shell_hay_is_product_ls("ls"));
    assert!(!shell_hay_is_product_ls("ls -la"));
    assert!(!shell_hay_is_product_ls("ls -r src"));
    assert!(!shell_hay_is_product_ls("ls LIVING_HANDOFF.md"));
    assert!(!shell_hay_is_product_ls("ls /tmp"));
    assert!(!shell_hay_is_product_ls(""));

    let product = shell("ls src");
    let nested = shell("ls src/kernel");
    let workspace = shell("ls -la");
    let board = shell("ls LIVING_HANDOFF.md");
    let recursive = shell("ls -R src");
    assert!(
        is_local_preflight_call(&product),
        "ls src is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&nested),
        "ls src/kernel is candidate preflight"
    );
    assert!(!is_local_preflight_call(&workspace), "bare ls stays recon");
    assert!(
        !is_local_preflight_call(&recursive),
        "recursive ls stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "ls of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board ls stays legal wait/poll, not preflight"
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product ls may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "bare ls must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-ls-while-inflight")
        .expect("shell-ls gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-ls-as-preflight")
        .expect("workspace-shell-ls fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-ls-as-preflight")
        .expect("board-shell-ls fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `grep stream src/kernel.cu` is the same class as path-scoped grep.
/// Pathless `grep TODO` and board greps stay recon and cannot arm submit.
#[test]
fn path_scoped_shell_grep_is_preflight() {
    assert!(shell_hay_is_path_scoped_grep("grep stream src/kernel.cu"));
    assert!(shell_hay_is_path_scoped_grep("rg stream src/"));
    assert!(shell_hay_is_path_scoped_grep(
        "grep -n stream src/kernel.cu"
    ));
    assert!(!shell_hay_is_path_scoped_grep("grep TODO"));
    assert!(!shell_hay_is_path_scoped_grep("grep -r stream src/"));
    assert!(!shell_hay_is_path_scoped_grep(
        "grep stream LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_path_scoped_grep("git grep stream src/"));
    assert!(!shell_hay_is_path_scoped_grep(""));

    let product = shell("grep stream src/kernel.cu");
    let rg = shell("rg stream src/");
    let workspace = shell("grep TODO");
    let board = shell("grep stream LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped grep is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&rg),
        "path-scoped rg is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless grep stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "grep of a board name must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product grep may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "pathless grep must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-grep-while-inflight")
        .expect("shell-grep gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-grep-as-preflight")
        .expect("workspace-shell-grep fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-grep-as-preflight")
        .expect("board-shell-grep fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Working-tree git_diff is candidate inspect (submit then review the patch).
/// A board-path diff stays recon and cannot arm submit.
#[test]
fn git_diff_is_local_preflight_while_inflight() {
    let tree = call("git_diff", serde_json::json!({}));
    let product = call("git_diff", serde_json::json!({"path": "src/kernel.cu"}));
    let board = call("git_diff", serde_json::json!({"path": "LIVING_HANDOFF.md"}));
    assert!(
        is_local_preflight_call(&tree),
        "pathless working-tree git_diff is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&product),
        "product-path git_diff stays preflight"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git_diff of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&tree)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::LocalPreflight, SlotPhase::InFlight, false),
        CadenceVerdict::Accept
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&tree), &submit()),
        "working-tree git_diff may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&board), &submit()),
        "board git_diff must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-git-diff-while-inflight")
        .expect("git-diff gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-git-diff-as-preflight")
        .expect("board-git-diff fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Working-tree git_status is candidate inspect (submit then review the tree).
/// A stuffed board path stays recon and cannot arm submit.
#[test]
fn git_status_is_local_preflight_while_inflight() {
    let tree = call("git_status", serde_json::json!({}));
    let product = call("git_status", serde_json::json!({"path": "src/kernel.cu"}));
    let board = call(
        "git_status",
        serde_json::json!({"path": "LIVING_HANDOFF.md"}),
    );
    assert!(
        is_local_preflight_call(&tree),
        "pathless working-tree git_status is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&product),
        "product-path git_status stays preflight"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git_status of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&tree)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::LocalPreflight, SlotPhase::InFlight, false),
        CadenceVerdict::Accept
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&tree), &submit()),
        "working-tree git_status may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&board), &submit()),
        "board git_status must not launder runner escalation"
    );

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-git-status-while-inflight")
        .expect("git-status gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-git-status-as-preflight")
        .expect("board-git-status fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Shell `git diff` is the same world-card inspect as native git_diff.
/// Board-path and `git difftool` must not launder as preflight.
#[test]
fn shell_git_diff_is_local_preflight_while_inflight() {
    assert!(shell_hay_is_git_diff("git diff"));
    assert!(shell_hay_is_git_diff("foo=1 git diff --stat"));
    assert!(!shell_hay_is_git_diff("git difftool"));
    assert!(!shell_hay_is_git_diff("git status"));

    let tree = call("shell", serde_json::json!({"command": "git diff"}));
    let product = call(
        "shell",
        serde_json::json!({"command": "git diff -- src/kernel.cu"}),
    );
    let board = call(
        "shell",
        serde_json::json!({"command": "git diff -- LIVING_HANDOFF.md"}),
    );
    let tool = call("shell", serde_json::json!({"command": "git difftool"}));
    assert!(
        is_local_preflight_call(&tree),
        "shell git diff is candidate preflight"
    );
    assert!(is_local_preflight_call(&product));
    assert!(
        !is_local_preflight_call(&board),
        "shell git diff of a board path must not launder as preflight"
    );
    assert!(
        !is_local_preflight_call(&tool),
        "git difftool is not working-tree inspect"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&tree)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&tree),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-diff-while-inflight")
        .expect("shell-git-diff gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-diff-as-preflight")
        .expect("board-shell-git-diff fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Shell `git status` is the same world-card inspect as native git_status.
/// Board-path status must not launder as preflight.
#[test]
fn shell_git_status_is_local_preflight_while_inflight() {
    assert!(shell_hay_is_git_status("git status"));
    assert!(shell_hay_is_git_status(
        "GIT_PAGER=cat git status --porcelain"
    ));
    assert!(!shell_hay_is_git_status("git stash"));
    assert!(!shell_hay_is_git_status("git diff"));

    let tree = call("shell", serde_json::json!({"command": "git status"}));
    let product = call(
        "shell",
        serde_json::json!({"command": "git status -- src/kernel.cu"}),
    );
    let board = call(
        "shell",
        serde_json::json!({"command": "git status -- LIVING_HANDOFF.md"}),
    );
    let stash = call("shell", serde_json::json!({"command": "git stash"}));
    assert!(
        is_local_preflight_call(&tree),
        "shell git status is candidate preflight"
    );
    assert!(is_local_preflight_call(&product));
    assert!(
        !is_local_preflight_call(&board),
        "shell git status of a board path must not launder as preflight"
    );
    assert!(
        !is_local_preflight_call(&stash),
        "git stash is not working-tree inspect"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&tree)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&tree),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-status-while-inflight")
        .expect("shell-git-status gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-status-as-preflight")
        .expect("board-shell-git-status fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// Path-scoped `git_log` / `git log` / `git blame` is candidate inspect
/// (submit then review history of the file). Pathless dumps and board
/// history stay recon and cannot arm submit.
#[test]
fn path_scoped_git_log_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_history("git log -- src/kernel.cu"));
    assert!(shell_hay_is_product_git_history(
        "GIT_PAGER=cat git log --oneline -n 5 -- ./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history("git blame src/kernel.cu"));
    assert!(shell_hay_is_product_git_history(
        "git blame -L 1,40 ./src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history("git log"));
    assert!(!shell_hay_is_product_git_history("git log --oneline"));
    assert!(!shell_hay_is_product_git_history("git log -- kernel.cu"));
    assert!(!shell_hay_is_product_git_history(
        "git log -- LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history("git blame kernel.cu"));
    assert!(!shell_hay_is_product_git_history(
        "git log -- src/kernel.cu | cat"
    ));
    assert!(!shell_hay_is_product_git_history("git stash"));
    assert!(!shell_hay_is_product_git_history("git login"));

    let product = call("git_log", serde_json::json!({"path": "src/kernel.cu"}));
    let workspace = call("git_log", serde_json::json!({}));
    let board = call("git_log", serde_json::json!({"path": "LIVING_HANDOFF.md"}));
    let shell_product = shell("git log -- src/kernel.cu");
    let shell_blame = shell("git blame src/kernel.cu");
    let shell_workspace = shell("git log");
    let shell_board = shell("git log -- LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped git_log is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&shell_product),
        "path-scoped shell git log is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&shell_blame),
        "path-scoped git blame is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git_log stays recon"
    );
    assert!(
        !is_local_preflight_call(&shell_workspace),
        "pathless shell git log stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git_log of a board path must not launder as preflight"
    );
    assert!(
        !is_local_preflight_call(&shell_board),
        "shell git log of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::Recon
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&product), &submit()),
        "product git_log may arm submit-next"
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&workspace), &submit()),
        "pathless git_log must not launder runner escalation"
    );
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-git-log-while-inflight")
        .expect("git-log gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let gold_shell = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-log-while-inflight")
        .expect("shell-git-log gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold_shell),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-git-log-as-preflight")
        .expect("workspace-git-log fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-git-log-as-preflight")
        .expect("board-git-log fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let shell_waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-log-as-preflight")
        .expect("workspace-shell-git-log fixture");
    assert_eq!(
        evaluate_mined_fixture(shell_waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let shell_board = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-log-as-preflight")
        .expect("board-shell-git-log fixture");
    assert_eq!(
        evaluate_mined_fixture(shell_board),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// `git ls-files -- src/kernel.cu` is candidate inspect (submit then check
/// whether the file is tracked). Pathless dumps and board names stay recon.
#[test]
fn path_scoped_git_ls_files_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_history(
        "git ls-files -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "GIT_PAGER=cat git ls-files --cached ./src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history("git ls-files"));
    assert!(!shell_hay_is_product_git_history("git ls-files --cached"));
    assert!(!shell_hay_is_product_git_history(
        "git ls-files -- kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git ls-files -- LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git ls-files -- src/kernel.cu | cat"
    ));

    let product = shell("git ls-files -- src/kernel.cu");
    let workspace = shell("git ls-files");
    let board = shell("git ls-files -- LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped git ls-files is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git ls-files stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git ls-files of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-ls-files-while-inflight")
        .expect("shell-git-ls-files gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-ls-files-as-preflight")
        .expect("workspace-shell-git-ls-files fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-ls-files-as-preflight")
        .expect("board-shell-git-ls-files fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// `git ls-tree HEAD src/kernel.cu` is candidate inspect (submit then list
/// the blob). Pathless dumps and board names stay recon.
#[test]
fn path_scoped_git_ls_tree_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_history(
        "git ls-tree HEAD src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "GIT_PAGER=cat git ls-tree -r HEAD -- ./src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history("git ls-tree"));
    assert!(!shell_hay_is_product_git_history("git ls-tree HEAD"));
    assert!(!shell_hay_is_product_git_history(
        "git ls-tree HEAD kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git ls-tree HEAD LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git ls-tree HEAD src/kernel.cu | cat"
    ));

    let product = shell("git ls-tree HEAD src/kernel.cu");
    let workspace = shell("git ls-tree HEAD");
    let board = shell("git ls-tree HEAD LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped git ls-tree is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git ls-tree stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git ls-tree of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-ls-tree-while-inflight")
        .expect("shell-git-ls-tree gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-ls-tree-as-preflight")
        .expect("workspace-shell-git-ls-tree fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-ls-tree-as-preflight")
        .expect("board-shell-git-ls-tree fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// `git rev-list HEAD -- src/kernel.cu` / `git shortlog -- src/kernel.cu` is
/// candidate inspect (submit then walk that file). Pathless dumps and board
/// names stay recon.
#[test]
fn path_scoped_git_rev_list_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_history(
        "git rev-list HEAD -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "GIT_PAGER=cat git rev-list --count HEAD -- ./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git shortlog -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git reflog -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git whatchanged -- src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history("git rev-list"));
    assert!(!shell_hay_is_product_git_history("git rev-list --all"));
    assert!(!shell_hay_is_product_git_history("git shortlog"));
    assert!(!shell_hay_is_product_git_history(
        "git rev-list HEAD -- kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git rev-list HEAD -- LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git shortlog -- living_handoff.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git rev-list HEAD -- src/kernel.cu | cat"
    ));

    let product = shell("git rev-list HEAD -- src/kernel.cu");
    let workspace = shell("git rev-list --all");
    let board = shell("git rev-list HEAD -- LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped git rev-list is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git rev-list stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git rev-list of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-rev-list-while-inflight")
        .expect("shell-git-rev-list gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-rev-list-as-preflight")
        .expect("workspace-shell-git-rev-list fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-rev-list-as-preflight")
        .expect("board-shell-git-rev-list fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// `git hash-object src/kernel.cu` / `git rev-parse HEAD:src/kernel.cu` is
/// candidate inspect (submit then hash/resolve the blob). Pathless dumps and
/// board names stay recon.
#[test]
fn path_scoped_git_hash_object_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_history(
        "git hash-object src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "GIT_PAGER=cat git hash-object -w -- ./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git describe -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git archive HEAD src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git diff-tree HEAD -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git check-attr -a -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_show(
        "git rev-parse HEAD:src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history("git hash-object"));
    assert!(!shell_hay_is_product_git_history("git hash-object --stdin"));
    assert!(!shell_hay_is_product_git_history("git describe --tags"));
    assert!(!shell_hay_is_product_git_history(
        "git hash-object kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git hash-object LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git hash-object src/kernel.cu | cat"
    ));
    assert!(!shell_hay_is_product_git_show("git rev-parse HEAD"));
    assert!(!shell_hay_is_product_git_show(
        "git rev-parse HEAD:LIVING_HANDOFF.md"
    ));

    let product = shell("git hash-object src/kernel.cu");
    let workspace = shell("git hash-object --stdin");
    let board = shell("git hash-object LIVING_HANDOFF.md");
    let blob = shell("git rev-parse HEAD:src/kernel.cu");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped git hash-object is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&blob),
        "git rev-parse of a product blob is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git hash-object stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git hash-object of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-hash-object-while-inflight")
        .expect("shell-git-hash-object gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-hash-object-as-preflight")
        .expect("workspace-shell-git-hash-object fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-hash-object-as-preflight")
        .expect("board-shell-git-hash-object fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `git annotate src/kernel.cu` / `git diff-index HEAD -- src/kernel.cu` /
/// `git diff-files -- src/kernel.cu` is candidate inspect (submit then
/// blame/diff the file). Pathless dumps and board names stay recon.
#[test]
fn path_scoped_git_annotate_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_history(
        "git annotate src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "GIT_PAGER=cat git annotate -L 1,80 -- ./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git diff-index HEAD -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git diff-files -- ./src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history("git annotate"));
    assert!(!shell_hay_is_product_git_history("git diff-index"));
    assert!(!shell_hay_is_product_git_history("git diff-files"));
    assert!(!shell_hay_is_product_git_history("git annotate kernel.cu"));
    assert!(!shell_hay_is_product_git_history(
        "git annotate LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git diff-index HEAD -- LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git annotate src/kernel.cu | cat"
    ));

    let product = shell("git annotate src/kernel.cu");
    let index = shell("git diff-index HEAD -- src/kernel.cu");
    let files = shell("git diff-files -- src/kernel.cu");
    let workspace = shell("git annotate");
    let board = shell("git annotate LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped git annotate is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&index),
        "path-scoped git diff-index is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&files),
        "path-scoped git diff-files is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git annotate stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git annotate of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board annotate stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-annotate-while-inflight")
        .expect("shell-git-annotate gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-annotate-as-preflight")
        .expect("workspace-shell-git-annotate fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-annotate-as-preflight")
        .expect("board-shell-git-annotate fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `git name-rev HEAD -- src/kernel.cu` / `git check-ignore src/kernel.cu`
/// is candidate inspect. Pathless dumps and board names stay recon.
#[test]
fn path_scoped_git_name_rev_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_history(
        "git name-rev HEAD -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "GIT_PAGER=cat git name-rev --name-only -- ./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git check-ignore -v src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_history(
        "git check-ignore -- ./src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_history("git name-rev"));
    assert!(!shell_hay_is_product_git_history("git check-ignore"));
    assert!(!shell_hay_is_product_git_history("git name-rev kernel.cu"));
    assert!(!shell_hay_is_product_git_history(
        "git name-rev HEAD -- LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git check-ignore LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_history(
        "git name-rev HEAD -- src/kernel.cu | cat"
    ));

    let product = shell("git name-rev HEAD -- src/kernel.cu");
    let ignore = shell("git check-ignore -v src/kernel.cu");
    let workspace = shell("git name-rev");
    let board = shell("git check-ignore LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped git name-rev is candidate preflight"
    );
    assert!(
        is_local_preflight_call(&ignore),
        "path-scoped git check-ignore is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git name-rev stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git check-ignore of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus,
        "board check-ignore stays legal wait/poll, not preflight"
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-name-rev-while-inflight")
        .expect("shell-git-name-rev gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-name-rev-as-preflight")
        .expect("workspace-shell-git-name-rev fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-check-ignore-as-preflight")
        .expect("board-shell-git-check-ignore fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `git grep stream -- src/kernel.cu` is candidate inspect (submit then search
/// the file). Pathless dumps and board names stay recon.
#[test]
fn path_scoped_git_grep_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_grep(
        "git grep stream -- src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_grep(
        "GIT_PAGER=cat git grep -n stream -- ./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_grep(
        "git grep -e stream src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_grep("git grep TODO"));
    assert!(!shell_hay_is_product_git_grep("git grep"));
    assert!(!shell_hay_is_product_git_grep("git grep src/kernel.cu"));
    assert!(!shell_hay_is_product_git_grep("git grep -e src/kernel.cu"));
    assert!(!shell_hay_is_product_git_grep("git grep TODO -- kernel.cu"));
    assert!(!shell_hay_is_product_git_grep(
        "git grep TODO -- LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_grep(
        "git grep stream -- src/kernel.cu | cat"
    ));
    assert!(!shell_hay_is_path_scoped_grep(
        "git grep stream -- src/kernel.cu"
    ));

    let product = shell("git grep stream -- src/kernel.cu");
    let workspace = shell("git grep TODO");
    let as_pattern = shell("git grep src/kernel.cu");
    let board = shell("git grep TODO -- LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "path-scoped git grep is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git grep stays recon"
    );
    assert!(
        !is_local_preflight_call(&as_pattern),
        "git grep of a path used as the pattern stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "git grep of a board path must not launder as preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-grep-while-inflight")
        .expect("shell-git-grep gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-grep-as-preflight")
        .expect("workspace-shell-git-grep fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-grep-as-preflight")
        .expect("board-shell-git-grep fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
}

/// `git show HEAD:src/kernel.cu` is candidate inspect (submit then review the
/// blob). Pathless / inventory show stays recon; board blobs stay wait/poll.
#[test]
fn path_scoped_git_show_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_show("git show HEAD:src/kernel.cu"));
    assert!(shell_hay_is_product_git_show(
        "GIT_PAGER=cat git show HEAD:./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_show(
        "git show abcdef0:src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_show("git show"));
    assert!(!shell_hay_is_product_git_show("git show HEAD"));
    assert!(!shell_hay_is_product_git_show("git show HEAD:kernel.cu"));
    assert!(!shell_hay_is_product_git_show(
        "git show HEAD:LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_show(
        "git show --stat HEAD -- src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_show(
        "git show HEAD -- src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_show(
        "git show HEAD:src/kernel.cu | cat"
    ));

    let product = shell("git show HEAD:src/kernel.cu");
    let workspace = shell("git show HEAD");
    let inventory = shell("git show --stat HEAD -- src/kernel.cu");
    let board = shell("git show HEAD:LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "product blob git show is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git show stays recon"
    );
    assert!(
        !is_local_preflight_call(&inventory),
        "git show --stat inventory stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "board blob must stay wait/poll, not preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&inventory)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-show-while-inflight")
        .expect("shell-git-show gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-show-as-preflight")
        .expect("workspace-shell-git-show fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let inventory_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-inventory-shell-git-show-as-preflight")
        .expect("inventory-shell-git-show fixture");
    assert_eq!(
        evaluate_mined_fixture(inventory_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-show-as-preflight")
        .expect("board-shell-git-show fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

/// `git cat-file -p HEAD:src/kernel.cu` is the same class as git show blob
/// inspect. Pathless / --batch dumps stay recon; board blobs stay wait/poll.
#[test]
fn path_scoped_git_cat_file_is_preflight_while_inflight() {
    assert!(shell_hay_is_product_git_show(
        "git cat-file -p HEAD:src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_show(
        "GIT_PAGER=cat git cat-file -t HEAD:./src/kernel.cu"
    ));
    assert!(shell_hay_is_product_git_show(
        "git cat-file -s abcdef0:src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_show("git cat-file"));
    assert!(!shell_hay_is_product_git_show("git cat-file -p HEAD"));
    assert!(!shell_hay_is_product_git_show(
        "git cat-file -p HEAD:kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_show(
        "git cat-file -p HEAD:LIVING_HANDOFF.md"
    ));
    assert!(!shell_hay_is_product_git_show(
        "git cat-file --batch HEAD:src/kernel.cu"
    ));
    assert!(!shell_hay_is_product_git_show(
        "git cat-file -p HEAD:src/kernel.cu | cat"
    ));

    let product = shell("git cat-file -p HEAD:src/kernel.cu");
    let workspace = shell("git cat-file -p HEAD");
    let batch = shell("git cat-file --batch HEAD:src/kernel.cu");
    let board = shell("git cat-file -p HEAD:LIVING_HANDOFF.md");
    assert!(
        is_local_preflight_call(&product),
        "product blob git cat-file is candidate preflight"
    );
    assert!(
        !is_local_preflight_call(&workspace),
        "pathless git cat-file stays recon"
    );
    assert!(
        !is_local_preflight_call(&batch),
        "git cat-file --batch stays recon"
    );
    assert!(
        !is_local_preflight_call(&board),
        "board blob must stay wait/poll, not preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&product)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&workspace)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&batch)),
        InFlightHopKind::Recon
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&product),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&workspace),
        &submit()
    ));
    assert!(!runner_escalation_allowed(
        is_local_preflight_call(&board),
        &submit()
    ));

    let gold = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-shell-git-cat-file-while-inflight")
        .expect("shell-git-cat-file gold fixture");
    assert_eq!(
        evaluate_mined_fixture(gold),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
    let waste = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-workspace-shell-git-cat-file-as-preflight")
        .expect("workspace-shell-git-cat-file fixture");
    assert_eq!(
        evaluate_mined_fixture(waste),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let batch_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-batch-shell-git-cat-file-as-preflight")
        .expect("batch-shell-git-cat-file fixture");
    assert_eq!(
        evaluate_mined_fixture(batch_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let board_fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-shell-git-cat-file-as-preflight")
        .expect("board-shell-git-cat-file fixture");
    assert_eq!(
        evaluate_mined_fixture(board_fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

#[test]
fn board_read_is_not_preflight_while_watcher_owns_inflight() {
    let board = call(
        "read_file",
        serde_json::json!({"path": "LIVING_HANDOFF.md"}),
    );
    assert!(is_competition_wait_or_progress_call(&board));
    assert!(!burns_first_write_budget(&board));
    assert!(!is_first_write_progress_call(&board));
    assert!(
        !is_local_preflight_call(&board),
        "board digest must not count as candidate preflight"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&board)),
        InFlightHopKind::PollStatus
    );
    assert_eq!(
        evaluate_inflight_hop(
            classify_inflight_hop(std::slice::from_ref(&board)),
            SlotPhase::InFlight,
            false
        ),
        CadenceVerdict::FailPollOnly
    );
    assert_eq!(
        evaluate_inflight_hop(
            classify_inflight_hop(std::slice::from_ref(&board)),
            SlotPhase::Terminal,
            false
        ),
        CadenceVerdict::FailPollOnly
    );
    assert!(
        !runner_escalation_allowed(is_local_preflight_call(&board), &submit()),
        "board read must not launder runner escalation"
    );

    let candidate = call("read_file", serde_json::json!({"path": "candidate.cu"}));
    assert!(is_local_preflight_call(&candidate));
    assert!(burns_first_write_budget(&candidate));
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&candidate)),
        InFlightHopKind::LocalPreflight
    );

    let fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-read-as-poll")
        .expect("board-read fixture");
    let verdicts = evaluate_mined_fixture(fix);
    assert_eq!(
        verdicts,
        vec![CadenceVerdict::Accept, CadenceVerdict::FailPollOnly]
    );
}

#[test]
fn local_benchmark_shell_is_preflight_not_recon_while_inflight() {
    let bench = shell("python bench.py");
    let ncu = shell("ncu --metrics sm__throughput.avg.pct_of_peak_sustained_elapsed ./kernel");
    let remote = shell("popcorn-cli run --mode benchmark");
    let random = shell("python exploit.py");
    assert!(
        is_local_benchmark_call(&bench),
        "world-card local-benchmark must be recognized"
    );
    assert!(is_local_benchmark_call(&ncu));
    assert!(is_local_preflight_call(&bench));
    assert!(is_local_preflight_call(&ncu));
    assert_eq!(classify_tool_lane(&remote), ToolLane::RunnerDispatch);
    assert!(
        !is_local_benchmark_call(&remote),
        "remote --mode benchmark is a runner, not a local bench"
    );
    assert!(
        !is_local_benchmark_call(&random),
        "arbitrary python is still recon"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&bench)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&random)),
        InFlightHopKind::Recon
    );
    assert!(runner_escalation_allowed(
        is_local_preflight_call(&bench),
        &submit()
    ));

    let fix = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-local-bench-while-inflight")
        .expect("local-bench inflight fixture");
    assert_eq!(
        evaluate_mined_fixture(fix),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
}

#[test]
fn board_write_is_not_wait_or_poll_while_inflight() {
    let handoff = call(
        "write_file",
        serde_json::json!({"path": "LIVING_HANDOFF.md", "content": "tip: x\n"}),
    );
    assert!(
        is_competition_board_state_call(&handoff),
        "first-write still treats meta writes as bookkeeping wait"
    );
    assert!(
        !burns_first_write_budget(&handoff),
        "meta writes stay bookkeeping (do not burn first-write)"
    );
    assert!(!is_first_write_progress_call(&handoff));
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&handoff)),
        InFlightHopKind::Recon,
        "watcher must not treat a board rewrite as PollStatus/receipt"
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::Recon, SlotPhase::InFlight, false),
        CadenceVerdict::FailReconThrash
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::Recon, SlotPhase::Terminal, true),
        CadenceVerdict::FailReconThrash,
        "just_notified receipt is not a board rewrite"
    );

    let inflight = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-write-as-poll")
        .expect("board-write inflight fixture");
    assert_eq!(
        evaluate_mined_fixture(inflight),
        vec![CadenceVerdict::Accept, CadenceVerdict::FailReconThrash]
    );
    let fake_receipt = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "adversarial-board-write-as-receipt")
        .expect("board-write receipt fixture");
    assert_eq!(
        evaluate_notify_fixture(fake_receipt),
        vec![CadenceVerdict::FailReconThrash]
    );
}

#[test]
fn native_verifier_is_local_benchmark_not_recon_while_inflight() {
    let check = call("check", serde_json::json!({}));
    let tests = call("run_tests", serde_json::json!({}));
    let lint = call("lint", serde_json::json!({}));
    let recon = call("grep", serde_json::json!({"pattern": "TODO"}));
    assert!(
        is_local_preflight_call(&check),
        "native check is world-card local-benchmark"
    );
    assert!(is_local_preflight_call(&tests));
    assert!(is_local_preflight_call(&lint));
    assert!(
        !is_local_preflight_call(&recon),
        "workspace grep stays recon thrash"
    );
    assert_eq!(
        classify_inflight_hop(std::slice::from_ref(&check)),
        InFlightHopKind::LocalPreflight
    );
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::LocalPreflight, SlotPhase::InFlight, false),
        CadenceVerdict::Accept
    );
    assert!(
        runner_escalation_allowed(is_local_preflight_call(&check), &submit()),
        "native check may arm submit-next"
    );

    let inflight = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-native-benchmark-while-inflight")
        .expect("native-benchmark inflight fixture");
    assert_eq!(
        evaluate_mined_fixture(inflight),
        vec![
            CadenceVerdict::Accept,
            CadenceVerdict::Accept,
            CadenceVerdict::Accept
        ]
    );

    let after_notify = MINED_CADENCE_FIXTURES
        .iter()
        .find(|f| f.id == "gold-notify-then-native-benchmark")
        .expect("notify then native-benchmark fixture");
    assert_eq!(
        evaluate_notify_fixture(after_notify),
        vec![CadenceVerdict::Accept, CadenceVerdict::Accept]
    );
}

#[test]
fn first_write_guard_stays_soft_when_watcher_owns_inflight_poll() {
    // Wait/poll still does not burn first-write budget (optional receipt).
    // The new in-flight policy is a separate detector.
    let status = poll_status();
    assert!(is_competition_wait_or_progress_call(&status));
    assert!(!burns_first_write_budget(&status));
    assert_eq!(
        evaluate_inflight_hop(classify_inflight_hop(&[status]), SlotPhase::InFlight, false),
        CadenceVerdict::FailPollOnly
    );
    assert_eq!(
        evaluate_inflight_hop(
            classify_inflight_hop(&[poll_status()]),
            SlotPhase::Terminal,
            true
        ),
        CadenceVerdict::AcceptReceipt
    );
}

#[test]
fn ordinary_tool_log_is_not_a_slot_receipt() {
    let mut tail = "test result: ok. 12 passed; 0 failed\n".repeat(4000);
    tail.push_str("Submission queued aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\n");
    assert!(
        !text_may_carry_slot(&tail),
        "must not walk megabyte cargo-test tails for a late uuid"
    );
    assert!(text_may_carry_slot(
        "Submission queued aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"
    ));
    assert!(!text_may_carry_slot("cargo test --lib"));
}

/// Ordinary hops sniff the first 8 KiB in place — no lowercase copy.
/// Mixed-case receipts still arm; a UTF-8 split at the cap must not panic.
#[test]
fn slot_receipt_sniff_scans_in_place() {
    assert!(ascii_contains_ignore_case(
        "SUBMISSION QUEUED id",
        "submission queued"
    ));
    assert!(ascii_contains_ignore_case("in Flight as", "in flight as"));
    assert!(ascii_contains_ignore_case("IN-FLIGHT AS", "in-flight as"));
    assert!(!ascii_contains_ignore_case(
        "submission",
        "submission queued"
    ));
    assert!(!ascii_contains_ignore_case(
        "cargo test --lib",
        "submission queued"
    ));
    assert_eq!(
        ascii_find_ignore_case("xx SUBMISSION QUEUED yy", "submission queued"),
        Some(3)
    );

    assert!(text_may_carry_slot(
        "SUBMISSION QUEUED aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"
    ));
    assert!(text_may_carry_slot(&format!("In-Flight as {GOLD_ID}")));
    assert!(!text_may_carry_slot("test result: ok. 3 passed; 0 failed"));

    let mut split = "é".repeat((SLOT_HINT_SCAN / 2) + 8);
    assert!(
        slot_hint_head(&split).len() <= SLOT_HINT_SCAN,
        "head must stay inside the 8 KiB cap"
    );
    assert!(
        split.is_char_boundary(slot_hint_head(&split).len()),
        "head must land on a UTF-8 boundary"
    );
    split.push_str(&format!("Submission queued {GOLD_ID}\n"));
    assert!(
        !text_may_carry_slot(&split),
        "receipt past a mid-codepoint cap must stay unarmed"
    );

    assert_eq!(
        extract_submission_id(&format!("SUBMISSION QUEUED\n{GOLD_ID}\n")).as_deref(),
        Some(GOLD_ID)
    );
}

#[test]
fn ordinary_non_comp_hop_does_not_arm_watcher_path() {
    let watcher = SubmissionWatcher::new();
    assert!(!watcher.hop_path_active(false));
    assert!(watcher.hop_path_active(true));
    let mut armed = SubmissionWatcher::new();
    armed.adopt(GOLD_ID);
    assert!(armed.hop_path_active(false));
    assert_eq!(
        evaluate_inflight_hop(InFlightHopKind::Idle, SlotPhase::Empty, false),
        CadenceVerdict::Accept
    );
}

#[test]
fn observe_tool_result_skips_ordinary_logs_until_receipt() {
    let mut watcher = SubmissionWatcher::new();
    observe_tool_result_for_watch(
        &mut watcher,
        "shell",
        "test result: ok. 3 passed; 0 failed",
        false,
    );
    assert!(watcher.slot_id().is_none());
    assert!(!watcher.hop_path_active(false));
    observe_tool_result_for_watch(
        &mut watcher,
        "shell",
        "Submission queued aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
        false,
    );
    assert!(watcher.slot_id().is_none());
    assert!(!watcher.hop_path_active(false));
}

#[test]
fn watcher_nudges_carry_telemetry_mark() {
    assert!(WATCHER_NOTIFY_MARK.starts_with(TELEMETRY_MARK));
}
