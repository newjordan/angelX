use super::dossier::DossierV1;
use super::leases::{HEARTBEAT_CADENCE_MS, LeaseError, LeasePhaseV1, WorkLeaseKeyV1};
use super::schema::{DirectorHealthStateV1, LaneIdV1};
use super::store::CompetitionStore;
use super::worker_policy::{
    LeaseRecoveryStepV1, SemanticDirectiveV1, SemanticLoopStateV1, SemanticObservationV1,
    next_heartbeat_at_ms, record_semantic_iteration,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "angel-worker-policy-{tag}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key() -> WorkLeaseKeyV1 {
    WorkLeaseKeyV1 {
        campaign_id: "campaign-r3".into(),
        lane_id: LaneIdV1::ContextMiner,
        work_item_id: "work-hang".into(),
    }
}

fn dossier() -> DossierV1 {
    DossierV1::new(
        "dossier-r3".into(),
        "project".into(),
        "repository".into(),
        "revision-one".into(),
        1,
        crate::knowledge::cut::sha256_hex(b"journal-one"),
    )
    .unwrap()
}

fn observation(evidence: &[&str]) -> SemanticObservationV1 {
    SemanticObservationV1 {
        tool_calls: BTreeSet::from(["read_source".into()]),
        hypotheses: BTreeSet::from(["memory_pressure".into()]),
        read_targets: BTreeSet::from(["src/kernel.rs:symbol".into()]),
        patch_mechanisms: BTreeSet::from(["tiling".into()]),
        benchmark_setups: BTreeSet::from(["fixture-b200".into()]),
        payload_sha256: BTreeSet::from([crate::knowledge::cut::sha256_hex(b"payload")]),
        novel_evidence_sha256: evidence
            .iter()
            .map(|value| crate::knowledge::cut::sha256_hex(value.as_bytes()))
            .collect(),
    }
}

#[test]
fn lease_hang_crash_loop_replaced_from_checkpoint_ac07() {
    let dir = TestDir::new("hang-replace");
    let store = CompetitionStore::new(dir.0.clone());
    let key = key();
    let first = store
        .update_leases(|leases| leases.grant(key.clone(), "worker-one".into(), 0, 500_000, 4))
        .unwrap();
    store
        .update_leases(|leases| {
            leases.heartbeat(&key, first.fencing_generation, HEARTBEAT_CADENCE_MS, 9)
        })
        .unwrap();
    assert_eq!(next_heartbeat_at_ms(0).unwrap(), HEARTBEAT_CADENCE_MS);

    let before_suspect = HEARTBEAT_CADENCE_MS + 119_999;
    assert_eq!(
        super::worker_policy::recover_hung_worker(
            &store,
            &key,
            first.fencing_generation,
            before_suspect,
            "worker-two",
            900_000,
        )
        .unwrap(),
        LeaseRecoveryStepV1::Healthy(LeasePhaseV1::Active)
    );
    let suspect_at = HEARTBEAT_CADENCE_MS + 120_000;
    assert_eq!(
        super::worker_policy::recover_hung_worker(
            &CompetitionStore::new(dir.0.clone()),
            &key,
            first.fencing_generation,
            suspect_at,
            "worker-two",
            900_000,
        )
        .unwrap(),
        LeaseRecoveryStepV1::Nudged
    );
    assert_eq!(
        super::worker_policy::recover_hung_worker(
            &CompetitionStore::new(dir.0.clone()),
            &key,
            first.fencing_generation,
            suspect_at + 1,
            "worker-two",
            900_000,
        )
        .unwrap(),
        LeaseRecoveryStepV1::Inspected
    );
    let replacement = match super::worker_policy::recover_hung_worker(
        &CompetitionStore::new(dir.0.clone()),
        &key,
        first.fencing_generation,
        suspect_at + 2,
        "worker-two",
        900_000,
    )
    .unwrap()
    {
        LeaseRecoveryStepV1::Replaced(lease) => lease,
        other => panic!("expected replacement, got {other:?}"),
    };
    assert_eq!(replacement.fencing_generation, first.fencing_generation + 1);
    assert_eq!(replacement.checkpoint_revision, 9);
    let recovered = CompetitionStore::new(dir.0.clone())
        .recover_leases()
        .unwrap();
    assert_eq!(recovered.current(&key), Some(&replacement));
    assert_eq!(
        recovered.accept_landing(&key, first.fencing_generation),
        Err(LeaseError::StaleGeneration)
    );
}

#[test]
fn semantic_loop_pivots_twice_then_fresh_rollover_ac07() {
    let mut state = SemanticLoopStateV1::new("direction-one".into()).unwrap();
    let mut dossier = dossier();
    let observation = observation(&["evidence-one"]);
    let head = crate::knowledge::cut::sha256_hex(b"journal-two");
    assert!(matches!(
        record_semantic_iteration(
            &mut state,
            &mut dossier,
            &observation,
            1,
            "direction-two",
            "turn-two",
            &head,
        )
        .unwrap(),
        SemanticDirectiveV1::Continue(ref health)
            if health.state == DirectorHealthStateV1::Fresh
    ));
    let mut outcomes = Vec::new();
    for now_ms in 2..=5 {
        outcomes.push(
            record_semantic_iteration(
                &mut state,
                &mut dossier,
                &observation,
                now_ms,
                "direction-two",
                "turn-two",
                &head,
            )
            .unwrap(),
        );
    }
    assert!(matches!(outcomes[0], SemanticDirectiveV1::Continue(_)));
    assert!(matches!(outcomes[1], SemanticDirectiveV1::Pivot(_)));
    assert!(matches!(outcomes[2], SemanticDirectiveV1::Continue(_)));
    let health = match &outcomes[3] {
        SemanticDirectiveV1::FreshTurn(health) => health,
        other => panic!("expected fresh continuation, got {other:?}"),
    };
    assert_eq!(health.state, DirectorHealthStateV1::NeedsAttention);
    assert_eq!(health.next.action, "start_direction:direction-two");
    assert_eq!((state.pivot_count, state.archived.len()), (2, 1));
    assert_eq!(state.direction_id, "direction-two");
    assert_eq!(dossier.dossier_revision, 1);
    assert_eq!(dossier.checkpoint.as_ref().unwrap().turn_id, "turn-two");
}

#[test]
fn novel_evidence_resets_semantic_staleness() {
    let mut state = SemanticLoopStateV1::new("direction-one".into()).unwrap();
    let mut dossier = dossier();
    let head = crate::knowledge::cut::sha256_hex(b"journal-two");
    let first = observation(&["evidence-one"]);
    for now in 1..=2 {
        record_semantic_iteration(
            &mut state,
            &mut dossier,
            &first,
            now,
            "direction-two",
            "turn-two",
            &head,
        )
        .unwrap();
    }
    assert_eq!(state.stale_iterations, 1);
    let novel = observation(&["evidence-one", "evidence-two"]);
    assert!(matches!(
        record_semantic_iteration(
            &mut state,
            &mut dossier,
            &novel,
            3,
            "direction-two",
            "turn-two",
            &head,
        )
        .unwrap(),
        SemanticDirectiveV1::Continue(ref health)
            if health.state == DirectorHealthStateV1::Fresh
    ));
    assert_eq!(state.stale_iterations, 0);
}

#[test]
fn semantic_archive_rejects_same_direction_then_accepts_fallback() {
    let mut state = SemanticLoopStateV1::new("direction-one".into()).unwrap();
    let mut dossier = dossier();
    let observation = observation(&["evidence-one"]);
    let head = crate::knowledge::cut::sha256_hex(b"journal-two");
    for now in 1..=4 {
        record_semantic_iteration(
            &mut state,
            &mut dossier,
            &observation,
            now,
            "direction-two",
            "turn-two",
            &head,
        )
        .unwrap();
    }
    let before_state = state.clone();
    let before_dossier = dossier.clone();
    assert!(
        record_semantic_iteration(
            &mut state,
            &mut dossier,
            &observation,
            5,
            "direction-one",
            "turn-two",
            &head,
        )
        .is_err()
    );
    assert_eq!(
        (state.clone(), dossier.clone()),
        (before_state, before_dossier)
    );
    assert!(matches!(
        record_semantic_iteration(
            &mut state,
            &mut dossier,
            &observation,
            5,
            "direction-fallback",
            "turn-two",
            &head,
        )
        .unwrap(),
        SemanticDirectiveV1::FreshTurn(_)
    ));
    assert_eq!(state.direction_id, "direction-fallback");
}

fn worker_health_leg() -> angel_live::Program {
    crate::agent::harness::live_policy::shipped_leg("worker-health")
}

// The parity oracle: the decision body the shipped leg replaced. It lives here
// so production keeps only the features, the input and the contract check.
fn native_recovery_step(
    phase: &LeasePhaseV1,
) -> crate::drive::competition::worker_policy::LiveRecoveryStep {
    use crate::drive::competition::worker_policy::{LiveRecoveryStep, phase_code};
    match phase {
        LeasePhaseV1::Suspect => LiveRecoveryStep { state: 1, phase: 2 },
        LeasePhaseV1::Nudged => LiveRecoveryStep { state: 2, phase: 3 },
        LeasePhaseV1::Inspected => LiveRecoveryStep { state: 3, phase: 4 },
        other => LiveRecoveryStep {
            state: 0,
            phase: phase_code(other),
        },
    }
}

fn native_semantic_step(
    progress: bool,
    stale_iterations: u8,
) -> crate::drive::competition::worker_policy::LiveSemanticStep {
    use crate::drive::competition::worker_policy::LiveSemanticStep;
    if progress {
        LiveSemanticStep {
            state: 0,
            stale_next: 0,
        }
    } else if stale_iterations == 2 {
        LiveSemanticStep {
            state: 1,
            stale_next: u64::from(stale_iterations),
        }
    } else if stale_iterations == 4 {
        LiveSemanticStep {
            state: 3,
            stale_next: 0,
        }
    } else {
        LiveSemanticStep {
            state: 2,
            stale_next: u64::from(stale_iterations),
        }
    }
}

fn expected_recovery(
    phase: LeasePhaseV1,
) -> crate::drive::competition::worker_policy::LiveRecoveryStep {
    native_recovery_step(&phase)
}

#[test]
fn worker_health_leg_matches_native_for_every_feature_value() {
    let leg = worker_health_leg();
    let mut cases = 0;
    let mut micros = Vec::new();
    for code in 0..7u64 {
        let phase = match code {
            0 => LeasePhaseV1::Granted,
            1 => LeasePhaseV1::Active,
            2 => LeasePhaseV1::Suspect,
            3 => LeasePhaseV1::Nudged,
            4 => LeasePhaseV1::Inspected,
            5 => LeasePhaseV1::Fenced,
            _ => LeasePhaseV1::Released,
        };
        let expected = expected_recovery(phase);
        let input = serde_json::json!({"kind": "recovery", "phase": code});
        let started = std::time::Instant::now();
        let (step, source) = crate::agent::harness::live_policy::decide_with(
            Some(leg.clone()),
            "worker-health",
            &input,
            |step: &crate::drive::competition::worker_policy::LiveRecoveryStep| match step.state {
                0..=3 => Ok(()),
                _ => Err("out of contract".into()),
            },
            || expected.clone(),
        );
        micros.push(started.elapsed().as_nanos() / 1_000);
        assert!(matches!(
            source,
            crate::agent::harness::live_policy::LegSource::Live { .. }
        ));
        assert_eq!(step, expected);
        cases += 1;
    }
    for progress in [true, false] {
        for stale in 0..=6u8 {
            for extra in [0u8, 1, 9] {
                let expected = native_semantic_step(progress, stale + extra);
                let input = serde_json::json!({"kind": "semantic", "progress": progress, "stale_next": stale + extra});
                let started = std::time::Instant::now();
                let (step, source) = crate::agent::harness::live_policy::decide_with(
                    Some(leg.clone()),
                    "worker-health",
                    &input,
                    |step: &crate::drive::competition::worker_policy::LiveSemanticStep| match step
                        .state
                    {
                        0..=3 => Ok(()),
                        _ => Err("out of contract".into()),
                    },
                    || expected.clone(),
                );
                micros.push(started.elapsed().as_nanos() / 1_000);
                // Without progress the leg answers stale_next 0..=6 and
                // refuses anything larger, falling back to native. With
                // progress it resets the counter for any feature value.
                if !progress && stale + extra > 6 {
                    assert!(matches!(
                        source,
                        crate::agent::harness::live_policy::LegSource::LiveFallback { .. }
                    ));
                } else {
                    assert!(matches!(
                        source,
                        crate::agent::harness::live_policy::LegSource::Live { .. }
                    ));
                }
                assert_eq!(step, expected);
                cases += 1;
            }
        }
    }
    assert!(cases >= 30, "parity table too small: {cases}");
    micros.sort_unstable();
    let median = micros[micros.len() / 2];
    eprintln!("worker-health leg median {median} us over {cases} decisions");
}

#[test]
fn worker_health_out_of_contract_step_falls_back_to_native() {
    let leg = worker_health_leg();
    let (step, source) = crate::agent::harness::live_policy::decide_with(
        Some(leg),
        "worker-health",
        &serde_json::json!({"kind": "recovery", "phase": 9}),
        |step: &crate::drive::competition::worker_policy::LiveRecoveryStep| match step.state {
            0..=3 => Ok(()),
            _ => Err("out of contract".into()),
        },
        || crate::drive::competition::worker_policy::LiveRecoveryStep { state: 7, phase: 9 },
    );
    assert!(matches!(
        source,
        crate::agent::harness::live_policy::LegSource::LiveFallback { .. }
    ));
    assert_eq!(
        step,
        crate::drive::competition::worker_policy::LiveRecoveryStep { state: 7, phase: 9 }
    );
}
