use super::*;
#[test]
fn research_off_surface_receipt_advances_unproductive_streak() {
    let _guard = crate::tests::env_lock();
    TURN_LEDGER.with(|cell| *cell.borrow_mut() = TurnLedger::default());
    set_research_turn(true);
    note_tool_outcome(
        1,
        "shell",
        &serde_json::json!({"command":"ss -tlnp"}),
        "ports",
        "ok",
        false,
        None,
        None,
        Some(1),
        5,
    );
    note_progress_hop(false);
    TURN_LEDGER.with(|cell| {
        let ledger = cell.borrow();
        assert_eq!(ledger.tools[0]["research_note"], "Research/OffSurface");
        assert_eq!(ledger.tools[0]["avoidable"], true);
        assert_eq!(ledger.unproductive_streak, 1);
        assert!(ledger.research_sources.is_empty());
    });
}
