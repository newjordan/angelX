use super::journal::{ActionUpdateV1, JournalError, canonical_action_key};
use super::leases::{LeaseBookV1, LeaseError, LeasePhaseV1, SUSPECT_AFTER_MS, WorkLeaseKeyV1};
use super::recovery::{RecoveryDirectiveV1, recover_store};
use super::schema::{
    ActionIntentV1, ActionKindV1, ActionPhaseV1, CompetitionKeyV1, LaneIdV1, ScheduledActionV1,
};
use super::store::CompetitionStore;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

mod review_tests;

struct FakeClock(u64);

impl FakeClock {
    fn now(&self) -> u64 {
        self.0
    }

    fn advance(&mut self, elapsed_ms: u64) {
        self.0 = self.0.saturating_add(elapsed_ms);
    }
}

struct TestDir(PathBuf);

impl TestDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-competition-{tag}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn intent(subject: &str) -> ActionIntentV1 {
    let mut intent = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: "campaign-r1".into(),
        competition: CompetitionKeyV1 {
            platform_id: "fixture".into(),
            competition_id: "contest".into(),
            field_id: "kernels".into(),
            benchmark_id: "bench".into(),
            profile_id: "p1".into(),
            hardware_id: "gpu".into(),
        },
        kind: ActionKindV1::SubmitCandidate,
        subject_id: subject.into(),
        payload_sha256: crate::cut::sha256_hex(format!("payload:{subject}").as_bytes()),
        intent_version: "v1".into(),
    };
    intent.action_key = canonical_action_key(&intent).unwrap();
    intent
}

fn update(intent: &ActionIntentV1, phase: ActionPhaseV1, at_ms: u64) -> ActionUpdateV1 {
    let retryable = matches!(phase, ActionPhaseV1::Ambiguous);
    ActionUpdateV1 {
        intent: intent.clone(),
        attempt: 0,
        phase,
        retryable,
        at_ms,
        reconcile_key: (phase == ActionPhaseV1::Ambiguous).then(|| "reconcile-1".into()),
        receipt_sha256: (phase == ActionPhaseV1::Completed)
            .then(|| crate::cut::sha256_hex(b"official-ack")),
        next: (phase == ActionPhaseV1::Ambiguous).then(|| ScheduledActionV1 {
            action: "reconcile_submission".into(),
            next_attempt_at_ms: at_ms + 1_000,
        }),
    }
}

fn fenced_store(path: &Path) -> (CompetitionStore, WorkLeaseKeyV1, u64) {
    let store = CompetitionStore::new(path.into());
    let key = lease_key();
    let lease = store
        .update_leases(|leases| leases.grant(key.clone(), "journal-worker".into(), 0, 1_000_000, 0))
        .unwrap();
    (store, key, lease.fencing_generation)
}

fn append(
    store: &CompetitionStore,
    key: &WorkLeaseKeyV1,
    generation: u64,
    update: ActionUpdateV1,
) -> super::store::AppendReceiptV1 {
    store.append_action_fenced(key, generation, update).unwrap()
}

#[test]
fn started_action_reconciles_before_reissue_ac13() {
    let dir = TestDir::new("started-reconcile");
    let (store, key, generation) = fenced_store(dir.path());
    let action = intent("candidate-a");
    assert!(
        store
            .append_action_fenced(
                &key,
                generation,
                update(&action, ActionPhaseV1::Planned, 10),
            )
            .unwrap()
            .appended
    );
    let started = update(&action, ActionPhaseV1::Started, 20);
    assert!(append(&store, &key, generation, started.clone()).appended);
    assert!(!append(&store, &key, generation, started).appended);

    let report = recover_store(&CompetitionStore::new(dir.path().into())).unwrap();
    assert_eq!(report.journal.next_seq, 2, "exact retry must not append");
    assert!(matches!(
        report.actions.as_slice(),
        [action] if matches!(
            action.directive,
            RecoveryDirectiveV1::ReconcileBeforeReissue {
                attempt: 0,
                reconcile_key: None,
                ..
            }
        )
    ));
}

#[test]
fn journal_cutpoint_matrix_recovers_one_canonical_state_ac13() {
    let dir = TestDir::new("cutpoints");
    let action = intent("candidate-b");
    let (store, key, generation) = fenced_store(dir.path());
    let phases = [
        ActionPhaseV1::Planned,
        ActionPhaseV1::Started,
        ActionPhaseV1::Ambiguous,
        ActionPhaseV1::Completed,
    ];
    for (index, phase) in phases.into_iter().enumerate() {
        append(
            &store,
            &key,
            generation,
            update(&action, phase, index as u64 + 1),
        );
        let report = recover_store(&CompetitionStore::new(dir.path().into())).unwrap();
        let record = &report.journal.actions[&action.action_key];
        assert_eq!(record.update.phase, phase);
        assert_eq!(report.journal.next_seq, index as u64 + 1);
    }
}

#[test]
fn torn_tail_and_duplicate_ack_are_idempotent_ac13() {
    let dir = TestDir::new("torn-tail");
    let (store, key, generation) = fenced_store(dir.path());
    let action = intent("candidate-c");
    append(
        &store,
        &key,
        generation,
        update(&action, ActionPhaseV1::Planned, 1),
    );
    let mut journal = OpenOptions::new()
        .append(true)
        .open(store.journal_path())
        .unwrap();
    journal.write_all(br#"{"schema":"torn""#).unwrap();
    journal.sync_data().unwrap();

    let repaired = recover_store(&CompetitionStore::new(dir.path().into())).unwrap();
    assert!(repaired.torn_tail_discarded);
    assert_eq!(repaired.journal.next_seq, 1);
    assert!(
        !store
            .append_action_fenced(
                &key,
                generation,
                update(&action, ActionPhaseV1::Planned, 999),
            )
            .unwrap()
            .appended
    );
    append(
        &store,
        &key,
        generation,
        update(&action, ActionPhaseV1::Started, 2),
    );
    let completed = update(&action, ActionPhaseV1::Completed, 3);
    assert!(append(&store, &key, generation, completed.clone()).appended);
    assert!(!append(&store, &key, generation, completed).appended);
    assert_eq!(store.recover().unwrap().state.next_seq, 3);
}

#[test]
fn action_key_payload_conflict_and_exact_replay() {
    let action = intent("candidate-d");
    let mut state = super::journal::ActionJournalStateV1::default();
    let event = match state
        .prepare(update(&action, ActionPhaseV1::Planned, 1))
        .unwrap()
    {
        super::journal::PrepareActionV1::Append(event) => *event,
        _ => unreachable!(),
    };
    state.apply(&event).unwrap();
    assert!(matches!(
        state.prepare(update(&action, ActionPhaseV1::Planned, 2)),
        Ok(super::journal::PrepareActionV1::Replay { .. })
    ));
    let mut conflict = action.clone();
    conflict.payload_sha256 = crate::cut::sha256_hex(b"different");
    assert!(matches!(
        state.prepare(update(&conflict, ActionPhaseV1::Planned, 3)),
        Err(JournalError::Conflict(_))
    ));
}

fn lease_key() -> WorkLeaseKeyV1 {
    WorkLeaseKeyV1 {
        campaign_id: "campaign-r1".into(),
        lane_id: LaneIdV1::DeepCut,
        work_item_id: "work-7".into(),
    }
}
