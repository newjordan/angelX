//! Harness telemetry marks on messages and nudges.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

fn check_dispatch_timing_without_previews(call_count: usize, yolo: bool) {
    let _guard = crate::tests::env_lock();
    let _mode = EnvGuard::set("ANGEL_ACTION_CAPSULES", "off");
    let _yolo = EnvGuard::set("ANGEL_YOLO", if yolo { "1" } else { "0" });
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let root = scratch("dispatch_timing");
    std::fs::write(root.join("one.txt"), "measured native read").unwrap();

    struct ReadThenAnswer {
        count: usize,
        hops: AtomicUsize,
    }
    impl Club for ReadThenAnswer {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "dispatch-timing-fixture"
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) != 0 {
                return Ok(ClubReply::Text("Read complete.".into()));
            }
            Ok(ClubReply::Calls(
                (0..self.count)
                    .map(|index| ToolCall {
                        id: format!("read-{index}"),
                        name: "read_file".into(),
                        // Parallel case covers both a successful and failed read.
                        args: serde_json::json!({"path": if index == 0 {"one.txt"} else {"missing.txt"}}),
                    })
                    .collect(),
            ))
        }
    }
    let registry = ToolRegistry::with_team(root.clone(), Vec::new());
    let club = ReadThenAnswer {
        count: call_count,
        hops: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user("Read the named files.")];
    let (events, received) = mpsc::channel();
    let outcome = run_turn_observed(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();
    assert_eq!(outcome.tools.len(), call_count);
    for (index, entry) in outcome.tools.iter().enumerate() {
        assert!(
            entry["ms"].as_u64().is_some(),
            "missing dispatch timing: {entry}"
        );
        assert_eq!(entry["exec"], if index == 0 { "ok" } else { "failed" });
        assert_eq!(entry["status"], entry["exec"]);
    }
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Tool && message.content.contains("measured native read")
    }));
    assert!(!received.try_iter().any(|event| {
        matches!(event, TurnEvent::Notice(text) if text.contains("action capsule"))
    }));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ledger_times_serial_calls_without_previews() {
    for yolo in [false, true] {
        check_dispatch_timing_without_previews(1, yolo);
    }
}

#[test]
fn ledger_times_parallel_calls_without_previews() {
    for yolo in [false, true] {
        check_dispatch_timing_without_previews(2, yolo);
    }
}

// --- telemetry suite ---

#[test]
fn telemetry_marked_harness_messages_are_dropped_but_user_quotes_survive() {
    // A harness-authored nudge (marked) rides the live tail but must never reach
    // the summarizer: render_transcript drops it so it can't be distilled into a
    // durable "the harness is broken" open-thread.
    let history = vec![
        ChatMsg::user("real user ask: add a feature"),
        ChatMsg::assistant("on it"),
        ChatMsg::harness(ERROR_NUDGE.to_string()),
        ChatMsg::harness(NOPROGRESS_NUDGE.to_string()),
        ChatMsg::harness(spin_redirect(true).to_string()),
        ChatMsg::harness(spin_redirect(false).to_string()),
    ];
    let rendered = render_transcript(&history);
    assert!(rendered.contains("real user ask"), "genuine content kept");
    assert!(rendered.contains("on it"), "genuine content kept");
    assert!(
        !rendered.contains("Every tool call in your last several turns failed"),
        "ERROR_NUDGE must not reach the summarizer:\n{rendered}"
    );
    assert!(
        !rendered.contains("re-reading files you already pulled into context"),
        "NOPROGRESS_NUDGE must not reach the summarizer:\n{rendered}"
    );
    assert!(
        !rendered.contains("stuck in a loop"),
        "spin perturbation must not reach the summarizer:\n{rendered}"
    );
    assert!(
        !rendered.contains("Change your approach"),
        "spin nudge must not reach the summarizer:\n{rendered}"
    );
    // Leading whitespace before the mark is still recognized (defensive trim).
    let padded = vec![ChatMsg::harness(format!("  {ERROR_NUDGE}"))];
    assert!(
        render_transcript(&padded).trim().is_empty(),
        "a whitespace-padded telemetry mark is still dropped"
    );
    // The same bytes quoted by an actual operator remain their conversation
    // content, including whitespace; only internal role plus marker is elided.
    for nudge in [
        ERROR_NUDGE.to_string(),
        NOPROGRESS_NUDGE.to_string(),
        spin_redirect(true).to_string(),
        spin_redirect(false).to_string(),
    ] {
        let quoted = format!("  {nudge}");
        assert_eq!(
            render_transcript(&[ChatMsg::user(quoted.as_str())]),
            format!("[user] {quoted}\n")
        );
    }
}

#[test]
fn every_harness_nudge_carries_the_telemetry_mark() {
    // Compile-time-ish pin: if a nudge string loses its mark, it would silently
    // start leaking into summaries again. Keep them all tagged.
    assert!(ERROR_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(FIRST_WRITE_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(NOPROGRESS_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(SPIN_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(SPIN_PERTURBATION.starts_with(TELEMETRY_MARK));
    assert!(MUTATION_THRASH_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(SELF_AUTHORED_VERIFY_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(PERIPHERAL_FANOUT_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(GREEN_VERIFY_DONE_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(NO_EDIT_ANSWER_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(FINAL_MILE_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(POST_EDIT_LOGIC_NUDGE.starts_with(TELEMETRY_MARK));
    assert!(WATCHER_NOTIFY_MARK.starts_with(TELEMETRY_MARK));
    assert!(
        FIRST_WRITE_NUDGE.contains("dependency")
            || FIRST_WRITE_NUDGE.contains("board wait")
            || FIRST_WRITE_NUDGE.contains("hilbert")
            || (FIRST_WRITE_NUDGE.contains("narrow validation")
                && FIRST_WRITE_NUDGE.contains("concrete blocker")),
        "first-write nudges must name legal escapes; generic pacing permits narrow validation and a blocker report"
    );
    assert!(spin_redirect(true).starts_with(TELEMETRY_MARK));
    assert!(spin_redirect(false).starts_with(TELEMETRY_MARK));
    // The standing relentless-execution directive is a real steer, NOT telemetry —
    // it must remain summarizable (unmarked), so the split in the false-start
    // correction path keeps only the correction tagged.
    assert!(!RELENTLESS_EXECUTION_DIRECTIVE.starts_with(TELEMETRY_MARK));
}
