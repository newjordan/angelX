use super::*;
use crate::competition::journal::{ActionJournalStateV1, ActionRecordV1, PrepareActionV1};
use crate::competition::lease_store::canonical_lease_id;
use crate::competition::recovery::directive;

#[test]
fn stale_generation_landing_is_fenced_ac07() {
    let mut clock = FakeClock(1_000);
    let mut leases = LeaseBookV1::default();
    let key = lease_key();
    let first = leases
        .grant(key.clone(), "worker-a".into(), clock.now(), 500_000, 7)
        .unwrap();
    leases
        .heartbeat(&key, first.fencing_generation, clock.now(), 8)
        .unwrap();
    clock.advance(SUSPECT_AFTER_MS);
    assert_eq!(
        leases
            .poll(&key, first.fencing_generation, clock.now())
            .unwrap(),
        LeasePhaseV1::Suspect
    );
    leases.nudge(&key, first.fencing_generation).unwrap();
    leases.inspect(&key, first.fencing_generation).unwrap();
    let replacement = leases
        .replace(
            &key,
            first.fencing_generation,
            "worker-b".into(),
            clock.now(),
            clock.now() + 500_000,
        )
        .unwrap();
    assert_eq!(replacement.checkpoint_revision, 8);
    assert_eq!(
        leases.accept_landing(&key, first.fencing_generation),
        Err(LeaseError::StaleGeneration)
    );
    leases
        .accept_landing(&key, replacement.fencing_generation)
        .unwrap();
}

#[test]
fn authoritative_append_rechecks_fence_after_replacement_ac07() {
    let dir = TestDir::new("authoritative-fence");
    let (store, key, generation) = fenced_store(dir.path());
    let action = intent("candidate-fence");
    assert!(
        append(
            &store,
            &key,
            generation,
            update(&action, ActionPhaseV1::Planned, 1),
        )
        .appended
    );
    let replacement = store
        .update_leases(|leases| {
            leases.poll(&key, generation, SUSPECT_AFTER_MS)?;
            leases.nudge(&key, generation)?;
            leases.inspect(&key, generation)?;
            leases.replace(
                &key,
                generation,
                "replacement".into(),
                SUSPECT_AFTER_MS,
                500_000,
            )
        })
        .unwrap();
    assert!(
        store
            .append_action_fenced(&key, generation, update(&action, ActionPhaseV1::Started, 2),)
            .is_err()
    );
    assert!(
        append(
            &store,
            &key,
            replacement.fencing_generation,
            update(&action, ActionPhaseV1::Started, 2),
        )
        .appended
    );
}

#[test]
fn direct_unfenced_append_is_rejected_ac07() {
    let dir = TestDir::new("unfenced-append");
    let store = CompetitionStore::new(dir.path().into());
    let action = intent("candidate-unfenced");
    assert!(
        store
            .append_action(update(&action, ActionPhaseV1::Planned, 1))
            .is_err()
    );
}

#[test]
fn lease_restart_regrant_preserves_generation_and_checkpoint_ac07() {
    let dir = TestDir::new("lease-restart");
    let store = CompetitionStore::new(dir.path().into());
    let key = lease_key();
    let first = store
        .update_leases(|leases| leases.grant(key.clone(), "first".into(), 0, 500_000, 7))
        .unwrap();
    store
        .update_leases(|leases| leases.release(&key, first.fencing_generation, 11))
        .unwrap();

    let restarted = CompetitionStore::new(dir.path().into());
    let recovered = restarted.recover_leases().unwrap();
    let current = recovered.current(&key).unwrap();
    assert_eq!(
        (current.fencing_generation, current.checkpoint_revision),
        (1, 11)
    );
    assert_eq!(current.phase, LeasePhaseV1::Released);
    assert!(
        restarted
            .update_leases(|leases| leases.grant(key.clone(), "bad".into(), 1, 500_000, 10))
            .is_err()
    );
    let second = restarted
        .update_leases(|leases| leases.grant(key.clone(), "second".into(), 1, 500_000, 11))
        .unwrap();
    assert_eq!(
        (second.fencing_generation, second.checkpoint_revision),
        (2, 11)
    );
}

#[test]
fn replacement_validation_failure_is_atomic_ac07() {
    let mut leases = LeaseBookV1::default();
    let key = lease_key();
    let first = leases
        .grant(key.clone(), "first".into(), 0, 500_000, 3)
        .unwrap();
    leases
        .poll(&key, first.fencing_generation, SUSPECT_AFTER_MS)
        .unwrap();
    leases.nudge(&key, first.fencing_generation).unwrap();
    leases.inspect(&key, first.fencing_generation).unwrap();
    assert_eq!(
        leases.replace(&key, first.fencing_generation, "bad".into(), 10, 10),
        Err(LeaseError::InvalidIdentity)
    );
    let current = leases.current(&key).unwrap();
    assert_eq!(current.fencing_generation, first.fencing_generation);
    assert_eq!(current.phase, LeasePhaseV1::Inspected);
}

#[test]
fn counter_boundaries_fail_closed_ac07_ac13() {
    let action = intent("candidate-max");
    let mut journal = ActionJournalStateV1 {
        campaign_id: None,
        next_seq: u64::MAX,
        head_sha256: crate::cut::sha256_hex(b"head"),
        actions: Default::default(),
    };
    let event = match journal
        .prepare(update(&action, ActionPhaseV1::Planned, 1))
        .unwrap()
    {
        PrepareActionV1::Append(event) => *event,
        _ => unreachable!(),
    };
    let before = journal.clone();
    assert!(matches!(
        journal.apply(&event),
        Err(JournalError::InvalidEvent(_))
    ));
    assert_eq!(journal, before);

    let mut retry = update(&action, ActionPhaseV1::Ambiguous, 2);
    retry.phase = ActionPhaseV1::Failed;
    retry.attempt = u32::MAX;
    let record = ActionRecordV1 {
        update: retry,
        last_seq: 1,
        last_event_sha256: crate::cut::sha256_hex(b"event"),
    };
    assert!(directive(&record).is_err());

    let key = lease_key();
    let mut leases = LeaseBookV1::default();
    let first = leases
        .grant(key.clone(), "max".into(), 0, 500_000, 9)
        .unwrap();
    leases.release(&key, first.fencing_generation, 9).unwrap();
    let mut max = leases.current(&key).unwrap().clone();
    max.fencing_generation = u64::MAX;
    max.lease_id = canonical_lease_id(&key, u64::MAX, &max.worker_instance_id, max.granted_at_ms);
    let mut leases = LeaseBookV1::restore(vec![max]).unwrap();
    assert_eq!(
        leases.grant(key, "overflow".into(), 1, 500_000, 9),
        Err(LeaseError::CounterExhausted)
    );
}

#[test]
fn empty_retry_action_is_rejected_ac13() {
    let action = intent("candidate-empty-retry");
    let mut bad = update(&action, ActionPhaseV1::Ambiguous, 1);
    bad.phase = ActionPhaseV1::Failed;
    bad.next.as_mut().unwrap().action.clear();
    assert!(matches!(
        ActionJournalStateV1::default().prepare(bad),
        Err(JournalError::InvalidIntent("invalid next action"))
    ));
}
