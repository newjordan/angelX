use super::*;
#[test]
fn p06c_background_uses_owner_clock_and_accumulates_submillisecond_work() {
    let meter = Meter::default();
    let _scope = meter.enter(Instant::now());
    {
        let mut state = meter.0.lock().unwrap();
        state.nanos.insert("aging_ms", 1_500_000);
        state.nanos.insert("compaction_ms", 4_750_000);
    }
    let snapshot = meter.snapshot();
    assert_eq!(snapshot.aging_ms, 1);
    assert_eq!(snapshot.compaction_ms, 4);
    assert_eq!(snapshot.reflex_tick_ms, 0);
    assert_eq!(snapshot.idle_probe_ms, 0);
    let active = span("other_ms");
    assert_eq!(meter.0.lock().unwrap().active.len(), 1);
    drop(active);
    assert!(meter.0.lock().unwrap().active.is_empty());
}
#[test]
fn p06c_retention_pairs_reused_ids_in_protocol_order() {
    let meter = Meter::default();
    let _scope = meter.enter(Instant::now());
    produced("same", 600, 600);
    produced("same", 800, 800);
    let mut history = vec![
        ChatMsg::tool("same", "x".repeat(600)),
        ChatMsg::tool("same", "x".repeat(800)),
    ];
    observe(&history);
    assert_eq!(
        super::super::super::trajectory::tools_output_snapshot().retained_bytes,
        1400
    );
    history[0].content = "x".repeat(100).into();
    observe(&history);
    history.remove(0);
    observe(&history);
    let output = super::super::super::trajectory::tools_output_snapshot();
    assert_eq!(
        output,
        ToolsOutput {
            produced_bytes: 1400,
            retained_bytes: 800,
            aged_bytes: 500,
            dropped_bytes: 100
        }
    );
}
#[test]
fn p06c_retention_counts_admission_aging_removal_once_and_excludes_old_turn() {
    let meter = Meter::default();
    let _scope = meter.enter(Instant::now());
    let mut history = vec![ChatMsg::tool("old", "old output")];
    produced("new", 1000, 900);
    history.push(ChatMsg::tool("new", "x".repeat(900)));
    observe(&history);
    history[1].content = "é".repeat(100).into();
    observe(&history);
    observe(&history);
    let output = super::super::super::trajectory::tools_output_snapshot();
    assert_eq!(
        output,
        ToolsOutput {
            produced_bytes: 1000,
            retained_bytes: 200,
            aged_bytes: 700,
            dropped_bytes: 100
        }
    );
    history.pop();
    observe(&history);
    let output = super::super::super::trajectory::tools_output_snapshot();
    assert_eq!(output.dropped_bytes, 300);
    assert_eq!(output.retained_bytes, 0);
    assert_eq!(
        output.produced_bytes,
        output.retained_bytes + output.aged_bytes + output.dropped_bytes
    );
}
