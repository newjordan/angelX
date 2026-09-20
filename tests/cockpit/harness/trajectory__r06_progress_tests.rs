use super::*;
use serde_json::json;

#[test]
fn trajectory_r06_new_failure_signature_and_inconclusive_run_credit() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    let run = |hop, result: &str, exec, verdict| {
        note_tool_outcome(
            hop,
            "run_tests",
            &json!({}),
            result,
            exec,
            false,
            None,
            Some(verdict),
            None,
            result.len(),
        );
        note_progress_hop(false);
        TURN_LEDGER.with(|cell| cell.borrow().unproductive_streak)
    };
    assert_eq!(run(1, "no conclusive summary", "ok", "inconclusive"), 0);
    assert_eq!(run(2, "changed chatter only", "ok", "inconclusive"), 1);
    assert_eq!(run(3, "test suite::one ... FAILED", "ok", "failed"), 0);
    assert_eq!(
        run(
            4,
            "test suite::one ... FAILED\nfinished in 2s",
            "ok",
            "failed"
        ),
        1
    );
    assert_eq!(run(5, "test suite::two ... FAILED", "denied", "failed"), 2);
    assert_eq!(run(6, "test suite::two ... FAILED", "ok", "failed"), 0);
    assert_eq!(run(7, "test suite::one ... FAILED", "ok", "failed"), 1);
    assert!(progress_ledger_snapshot()["first_verified_at_ms"].is_null());
    assert_eq!(
        progress_ledger_snapshot()["last_credited_progress"],
        json!({"hop":6,"kind":"new_failing_test"})
    );
}

#[test]
fn trajectory_r06_detected_build_is_progress_without_green_credit() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    for (hop, command, expected) in [
        (1, "cargo build", 0),
        (2, "cargo build", 1),
        (3, "echo 'cargo build'", 2),
        (4, "npm run build", 0),
    ] {
        note_tool_outcome(
            hop,
            "shell",
            &json!({"command":command}),
            "build failed",
            "ok",
            false,
            None,
            None,
            None,
            12,
        );
        note_progress_hop(false);
        assert_eq!(
            TURN_LEDGER.with(|cell| cell.borrow().unproductive_streak),
            expected
        );
    }
    assert!(progress_ledger_snapshot()["first_verified_at_ms"].is_null());
}

#[test]
fn trajectory_remote_verification_receipt_resets_unproductive_streak() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    let receipts = [
        "LEG ON.1 pass=True dec=0.02506 pf=0.0006094 acc=0.938",
        "FOLD_AB_DONE",
        "STACK_AB_DONE",
        "officialScore: 2.684",
    ];
    for (i, receipt) in receipts.iter().enumerate() {
        note_tool_outcome(
            i + 1,
            "shell",
            &json!({"command": "ssh sparky './bench.sh'"}),
            receipt,
            "ok",
            false,
            None,
            None,
            None,
            100,
        );
        note_progress_hop(false);
        assert_eq!(
            TURN_LEDGER.with(|cell| cell.borrow().unproductive_streak),
            0,
            "new receipt '{receipt}' should reset unproductive streak"
        );
        note_tool_outcome(
            i + 1,
            "shell",
            &json!({"command": "ssh sparky './bench.sh'"}),
            receipt,
            "ok",
            false,
            None,
            None,
            None,
            100,
        );
        note_progress_hop(false);
        assert_eq!(
            TURN_LEDGER.with(|cell| cell.borrow().unproductive_streak),
            1,
            "repeated receipt cannot manufacture progress"
        );
    }
}
