use super::dossier::DossierV1;
use super::dossier_store::DossierStoreV1;
use super::journal::{ActionJournalStateV1, ActionUpdateV1, PrepareActionV1, canonical_action_key};
use super::recovery::{RecoveryDirectiveV1, recover_store};
use super::schema::{ActionIntentV1, ActionKindV1, ActionPhaseV1, CompetitionKeyV1};
use super::store::CompetitionStore;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "angel-cutpoint-{tag}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn intent() -> ActionIntentV1 {
    let mut intent = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: "campaign-cutpoint".into(),
        competition: CompetitionKeyV1 {
            platform_id: "fixture".into(),
            competition_id: "contest".into(),
            field_id: "kernels".into(),
            benchmark_id: "bench".into(),
            profile_id: "profile".into(),
            hardware_id: "gpu".into(),
        },
        kind: ActionKindV1::SubmitCandidate,
        subject_id: "candidate-cutpoint".into(),
        payload_sha256: crate::knowledge::cut::sha256_hex(b"cutpoint-payload"),
        intent_version: "v1".into(),
    };
    intent.action_key = canonical_action_key(&intent).unwrap();
    intent
}

fn update(intent: &ActionIntentV1, phase: ActionPhaseV1, at_ms: u64) -> ActionUpdateV1 {
    ActionUpdateV1 {
        intent: intent.clone(),
        attempt: 0,
        phase,
        retryable: false,
        at_ms,
        reconcile_key: None,
        receipt_sha256: (phase == ActionPhaseV1::Completed)
            .then(|| crate::knowledge::cut::sha256_hex(b"cutpoint-receipt")),
        next: None,
    }
}

fn event(state: &ActionJournalStateV1, phase: ActionPhaseV1, at_ms: u64) -> Vec<u8> {
    let event = match state.prepare(update(&intent(), phase, at_ms)).unwrap() {
        PrepareActionV1::Append(event) => *event,
        PrepareActionV1::Replay { .. } => unreachable!(),
    };
    let mut body = serde_json::to_vec(&event).unwrap();
    body.push(b'\n');
    body
}

fn write_synced(path: &std::path::Path, body: &[u8]) {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .unwrap();
    file.write_all(body).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn journal_cutpoint_matrix_recovers_one_canonical_state_ac13() {
    let before = TestRoot::new("journal-before");
    assert_eq!(
        CompetitionStore::new(before.0.clone())
            .recover()
            .unwrap()
            .state,
        ActionJournalStateV1::default()
    );

    let torn = TestRoot::new("journal-torn");
    std::fs::create_dir(&torn.0).unwrap();
    write_synced(&torn.0.join("actions.jsonl"), br#"{"schema":"torn""#);
    let recovered = CompetitionStore::new(torn.0.clone()).recover().unwrap();
    assert!(recovered.torn_tail_discarded);
    assert_eq!(recovered.state, ActionJournalStateV1::default());

    let durable = TestRoot::new("journal-durable");
    std::fs::create_dir(&durable.0).unwrap();
    write_synced(
        &durable.0.join("actions.jsonl"),
        &event(&ActionJournalStateV1::default(), ActionPhaseV1::Planned, 1),
    );
    let report = recover_store(&CompetitionStore::new(durable.0.clone())).unwrap();
    assert_eq!(report.journal.next_seq, 1);
    assert!(matches!(
        report.actions[0].directive,
        RecoveryDirectiveV1::ExecutePlanned { attempt: 0 }
    ));
    std::fs::remove_file(durable.0.join("actions.snapshot.json")).unwrap();
    let restarted = recover_store(&CompetitionStore::new(durable.0.clone())).unwrap();
    assert_eq!(restarted.journal, report.journal);
}

fn dossier(head: &[u8]) -> DossierV1 {
    DossierV1::new(
        "dossier-cutpoint".into(),
        "project".into(),
        "repository".into(),
        "revision-one".into(),
        1,
        crate::knowledge::cut::sha256_hex(head),
    )
    .unwrap()
}

#[test]
fn dossier_current_post_rename_ambiguity_recovers_visible_state_ac13() {
    let root = TestRoot::new("dossier-current");
    let store = DossierStoreV1::new(root.0.clone());
    let mut old_state = dossier(b"old-head");
    old_state
        .rollover_fresh_turn(
            "turn-old".into(),
            crate::knowledge::cut::sha256_hex(b"old-head-2"),
            "old checkpoint".into(),
        )
        .unwrap();
    store.sync(&old_state).unwrap();
    let current = root.0.join("current.json");
    let old_pointer = std::fs::read(&current).unwrap();

    let mut new_state = old_state.clone();
    new_state
        .rollover_fresh_turn(
            "turn-new".into(),
            crate::knowledge::cut::sha256_hex(b"new-head"),
            "new checkpoint".into(),
        )
        .unwrap();
    store.sync(&new_state).unwrap();
    let new_pointer = std::fs::read(&current).unwrap();

    write_synced(&current, &old_pointer);
    let pre_rename = root.0.join(".current.before-rename.tmp");
    write_synced(&pre_rename, br#"{"schema":"torn""#);
    assert_eq!(store.load().unwrap(), Some(old_state));

    let post_rename = root.0.join(".current.after-rename.tmp");
    write_synced(&post_rename, &new_pointer);
    std::fs::rename(&post_rename, &current).unwrap();
    // The rename is the visibility commit. A later chmod/fsync error makes the
    // caller uncertain; recovery must accept the complete visible generation.
    assert_eq!(store.load().unwrap(), Some(new_state.clone()));
    store.sync(&new_state).unwrap();
    assert_eq!(store.load().unwrap(), Some(new_state));
}
