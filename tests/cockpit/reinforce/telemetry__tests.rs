use super::*;

/// The snapshot is a process-global; other reinforce tests may publish
/// concurrently. Retry the whole sequence until our source survives the
/// read-back so the assertion is about our own writes.
fn with_own_run(assertions: impl Fn(RlSnapshot)) {
    for _ in 0..16 {
        begin("telemetry-test", 2);
        round(1, 0);
        batch(
            BatchCounters {
                generated: 12,
                accepted: 10,
                solve_rate: 0.5,
                advantage_variance: 0.02,
                ..BatchCounters::default()
            },
            Some(1.0),
        );
        cohort_begin("promotion", 2, 4);
        case_slot(CaseSlot {
            id: "case-a".into(),
            requested_per_policy: 4,
            incumbent_observed: 4,
            candidate_observed: 4,
            delta: Some(0.2),
            complete: true,
            failed: false,
        });
        verdict(CohortVerdict {
            role: "promotion".into(),
            decision: "promoted".into(),
            promoted: true,
            mean_delta: Some(0.2),
            delta_lower_bound: Some(0.1),
        });
        let Some(snapshot) = current() else { continue };
        if snapshot.source == "telemetry-test" && snapshot.gate.is_some() {
            assertions(snapshot);
            return;
        }
    }
    panic!("telemetry snapshot never survived concurrent publishers");
}

#[test]
fn run_sequence_accumulates_structural_state() {
    with_own_run(|snapshot| {
        assert_eq!(snapshot.rounds, 2);
        assert_eq!(snapshot.round, 1);
        assert_eq!(snapshot.batch.map(|b| b.accepted), Some(10));
        assert_eq!(snapshot.cases.len(), 1);
        assert_eq!(snapshot.cases[0].delta, Some(0.2));
        assert!(snapshot.gate.as_ref().unwrap().promoted);
        assert!(
            snapshot
                .events
                .iter()
                .any(|event| event.label.contains("promotion · promoted"))
        );
        // Structural-only: no event may carry task or prompt text — the
        // only free-form field is the opaque case id.
        assert!(
            snapshot
                .events
                .iter()
                .all(|event| !event.label.contains("reverse:"))
        );
    });
}

#[test]
fn case_slot_updates_in_place() {
    begin("telemetry-upsert", 1);
    cohort_begin("promotion", 1, 4);
    for observed in [2usize, 4] {
        case_slot(CaseSlot {
            id: "case-x".into(),
            requested_per_policy: 4,
            incumbent_observed: observed,
            candidate_observed: observed,
            delta: None,
            complete: false,
            failed: false,
        });
    }
    if let Some(snapshot) = current()
        && snapshot.source == "telemetry-upsert"
    {
        // Concurrent suites may append their own cases to the global
        // snapshot; the upsert contract is only about OUR case id.
        let mine: Vec<_> = snapshot
            .cases
            .iter()
            .filter(|case| case.id == "case-x")
            .collect();
        assert_eq!(
            mine.len(),
            1,
            "case-x upserts in place: {:?}",
            snapshot.cases
        );
        assert_eq!(mine[0].incumbent_observed, 4);
    }
}

#[test]
fn event_timeline_stays_bounded() {
    begin("telemetry-bound", 1);
    for _ in 0..(MAX_EVENTS * 2) {
        phase(RlPhase::Generation);
        phase(RlPhase::Scoring);
    }
    if let Some(snapshot) = current() {
        assert!(snapshot.events.len() <= MAX_EVENTS);
    }
}
