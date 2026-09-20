use super::*;
use serde_json::json;

#[test]
fn trace_schema_emitters_join_attempts_and_preserve_unknowns() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| {
        *cell.borrow_mut() = TurnLedger {
            turn_id: Some("fixture".into()),
            ..TurnLedger::default()
        };
    });
    note_tool_outcome(
        1,
        "shell",
        &json!({"command":"cargo test"}),
        "passed",
        "ok",
        false,
        None,
        Some("passed"),
        Some(12),
        6,
    );
    note_tool_outcome(
        2,
        "present",
        &json!({"kind":"report","url":"report.md"}),
        "queued",
        "ok",
        false,
        None,
        None,
        Some(1),
        6,
    );
    note_tool_outcome(
        3,
        "machine_test",
        &json!({"command":"pytest"}),
        "unknown",
        "failed",
        true,
        None,
        None,
        None,
        7,
    );
    TURN_LEDGER.with(|cell| {
        let ledger = cell.borrow();
        assert_eq!(ledger.tools[0]["attempt_id"], "fixture:0");
        assert_eq!(
            ledger.verifier[0]["attempt_id"],
            ledger.tools[0]["attempt_id"]
        );
        assert_eq!(ledger.verifier[0]["command"], "cargo test");
        assert!(ledger.verifier[0]["exit"].is_null());
        assert_eq!(
            ledger.artifacts[0]["path_digest"],
            crate::knowledge::cut::sha256_hex(b"report.md")
        );
        assert!(ledger.artifacts[0]["shown"].is_null());
        assert_eq!(ledger.lease.as_ref().unwrap()["kind"], "fleet");
        assert!(ledger.tools[2]["ms"].is_null());
    });
}

#[test]
fn trace_schema_provider_attempts_are_nested_and_retained() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    assert_eq!(begin_provider_attempt(1), None);
    note_timing_origin(std::time::Instant::now());
    begin_model_request();
    let accounting = crate::agent::club::AccountingCell::default();
    drop(accounting.attempt());
    let mut retry = accounting.attempt();
    retry.observe(Some(crate::agent::club::UsageObservation {
        raw: [Some(10), Some(3), None, None, None],
        ..crate::agent::club::UsageObservation::default()
    }));
    drop(retry);
    end_model_request();
    let samples = provider_call_samples();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[1]["retry_of"], 0);
    assert_eq!(samples[1]["model_call"], 0);
    assert!(samples[0]["usage"].is_null());
    assert_eq!(samples[1]["usage"][0], 10);
    assert!(
        samples
            .iter()
            .all(|s| s["end"].as_u64().unwrap() >= s["start"].as_u64().unwrap())
    );
    assert_eq!(begin_provider_attempt(7), None);
}

#[test]
fn r03_pending_exit_hop_is_visible_and_finalized_once() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    let hop = begin_progress_hop();
    assert_eq!(progress_ledger_snapshot()["unproductive_streak_max"], 1);
    drop(hop);
    assert_eq!(progress_ledger_snapshot()["unproductive_streak_max"], 1);
    let hop = begin_progress_hop();
    note_verified(20);
    drop(hop);
    let hop = begin_progress_hop();
    note_verified(40); // The same green alone is not new verifier evidence.
    note_progress_hop(false);
    drop(hop);
    let hop = begin_progress_hop();
    assert_eq!(progress_ledger_snapshot()["unproductive_streak_max"], 2);
    drop(hop);
    assert_eq!(progress_ledger_snapshot()["unproductive_streak_max"], 2);
}

#[test]
fn dispatch_receipt_ledger_environment_and_policy() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    for (hop, error) in [
        "tool error: helper: ENOENT",
        "tool error: sandbox: mount not confined",
    ]
    .into_iter()
    .enumerate()
    {
        note_tool_outcome(
            hop,
            "shell",
            &json!({"cmd":"printf fixture"}),
            error,
            "failed",
            true,
            None,
            None,
            Some(1),
            error.len(),
        );
    }
    let entries = tool_ledger_snapshot();
    assert_eq!(entries[0]["error_class"], "Environment");
    assert_eq!(entries[1]["error_class"], "Policy");
    assert!(entries.iter().all(|entry| entry["avoidable"] == true));
    assert_eq!(entries[0]["error"], "tool error: helper: ENOENT");
    println!(
        "dispatch receipt ledger: Environment and Policy dispatch failures avoidable=true; original error retained"
    );
}

#[test]
fn r03b_edit_and_verifier_run_progress_resets_escalation() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    note_tool_outcome(
        1,
        "run_tests",
        &json!({}),
        "tests: 0 passed, 1 failed",
        "ok",
        false,
        None,
        Some("failed"),
        None,
        25,
    );
    note_progress_hop(false);
    for hop in 2..=9 {
        note_tool_outcome(
            hop,
            "read_file",
            &json!({"path":format!("file-{hop}")}),
            "new contents",
            "ok",
            false,
            None,
            None,
            None,
            12,
        );
        note_progress_hop(false);
    }
    let (notice, stop) = unproductive_escalation(9, 8, 16);
    assert!(notice.unwrap().contains("8 consecutive"));
    assert!(stop.is_none());
    note_progress_hop(true); // byte-changing edits immediately reset the streak
    assert_eq!(progress_ledger_snapshot()["unproductive_streak_max"], 8);
    assert_eq!(
        TURN_LEDGER.with(|cell| cell.borrow().unproductive_streak),
        0
    );
    note_tool_outcome(
        11,
        "run_tests",
        &json!({}),
        "tests: 0 passed, 1 failed",
        "ok",
        false,
        None,
        Some("failed"),
        None,
        25,
    );
    note_progress_hop(false); // a verifier on the new bytes earns run credit
    assert_eq!(
        TURN_LEDGER.with(|cell| cell.borrow().unproductive_streak),
        0
    );
    assert!(!TURN_LEDGER.with(|cell| cell.borrow().streak_escalated));
    note_tool_outcome(
        12,
        "run_tests",
        &json!({"filter":"new"}),
        "tests: 0 passed, 1 failed; different output chatter",
        "ok",
        false,
        None,
        Some("failed"),
        None,
        50,
    );
    note_progress_hop(false);
    assert_eq!(
        TURN_LEDGER.with(|cell| cell.borrow().unproductive_streak),
        0 // a distinct verifier command is a new run
    );
}

#[test]
fn t05_r03_ledger_canonical_duplicates_progress_and_reset() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    let first: Value =
        serde_json::from_str(r#"{"path":"src/lib.rs","nested":{"b":2,"a":1}}"#).unwrap();
    let reordered: Value =
        serde_json::from_str(r#"{"nested":{"a":1,"b":2},"path":"src/lib.rs"}"#).unwrap();
    for (hop, args) in [(1, &first), (2, &reordered)] {
        note_tool_outcome(
            hop,
            "read_file",
            args,
            "contents",
            "ok",
            false,
            None,
            None,
            Some(1),
            8,
        );
        note_progress_hop(false);
    }
    assert_eq!(normalized_read_path("./src/../src/lib.rs"), "src/lib.rs");
    assert_eq!(normalized_read_path("../../src/lib.rs"), "../../src/lib.rs");
    let entries = tool_ledger_snapshot();
    assert!(entries[0]["duplicate_of"].is_null());
    assert_eq!(entries[1]["duplicate_of"], 0);
    assert_eq!(entries[0]["error_class"], "None");
    assert_eq!(entries[0]["avoidable"], false);
    assert_eq!(progress_ledger_snapshot()["unproductive_streak_max"], 2);
    note_tool_outcome(
        3,
        "read_file",
        &json!({"path":"missing"}),
        "No such file or directory",
        "failed",
        true,
        None,
        None,
        None,
        0,
    );
    note_progress_hop(false);
    assert_eq!(progress_ledger_snapshot()["unproductive_streak_max"], 3);
    note_progress_hop(true);
    note_tool_outcome(
        5,
        "run_tests",
        &json!({}),
        "tests: 1 passed, 0 failed",
        "ok",
        false,
        None,
        Some("passed"),
        None,
        25,
    );
    note_verified(17);
    note_verified(99);
    note_progress_hop(false);
    note_tool_outcome(
        6,
        "run_tests",
        &json!({"filter":"changed"}),
        "tests: 1 passed, 0 failed",
        "ok",
        false,
        None,
        Some("passed"),
        None,
        25,
    );
    note_progress_hop(false);
    note_escalation(6, "test_advisory");
    let mut record = json!({});
    attach_turn_ledger(&mut record, None);
    assert_eq!(record["first_verified_at_ms"], 17);
    assert_eq!(record["unproductive_streak_max"], 3);
    assert_eq!(record["escalations"][0]["hop"], 6);
    assert_eq!(record["tools"].as_array().unwrap().len(), 5);
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    assert!(tool_ledger_snapshot().is_empty());
    assert!(progress_ledger_snapshot()["first_verified_at_ms"].is_null());
}
