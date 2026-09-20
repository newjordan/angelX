use super::*;

#[test]
fn lifecycle_kills_update_only_the_matching_launch() {
    let _lock = crate::tests::env_lock();
    // Identical command arguments must not collapse two distinct children.
    let args = serde_json::json!({"command":"sleep 300"});
    for id in [900001_u64, 900002] {
        let result = format!("started [{id}] job (pid 42)");
        note_tool_outcome(
            1,
            "proc_run",
            &args,
            &result,
            "ok",
            false,
            None,
            None,
            None,
            result.len(),
        );
    }
    let before = tool_ledger_snapshot().len();
    let kill =
        crate::agent::sandbox::process_owner::KillReceipt::new(Some(9), "owner_reap", "turn_owner");
    note_proc_kills(&[(900002, kill.clone()), (900003, kill)]);
    let ledger = tool_ledger_snapshot();
    assert_eq!(ledger.len(), before, "no invented tool calls");
    let first = ledger
        .iter()
        .find(|entry| entry["proc_id"] == 900001_u64)
        .unwrap();
    let second = ledger
        .iter()
        .find(|entry| entry["proc_id"] == 900002_u64)
        .unwrap();
    assert_eq!(first["exec"], "ok");
    assert!(first.get("kill").is_none());
    assert_eq!(second["status"], "killed");
    assert_ne!(first["attempt_id"], second["attempt_id"]);
}

#[test]
fn lifecycle_successful_shell_output_cannot_supply_a_kill_receipt() {
    let _lock = crate::tests::env_lock();
    let kill =
        crate::agent::sandbox::process_owner::KillReceipt::new(Some(9), "owner_reap", "turn_owner");
    let text = kill.error("user-controlled stdout");
    note_tool_outcome(
        1,
        "shell",
        &serde_json::json!({"command":"echo"}),
        &text,
        "ok",
        false,
        None,
        None,
        None,
        text.len(),
    );
    let ledger = tool_ledger_snapshot();
    assert!(ledger.last().unwrap().get("kill").is_none());
}
