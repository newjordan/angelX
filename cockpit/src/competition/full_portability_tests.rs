use super::director::CompetitionDirectorStateV1;
use super::director_services::DirectorWorkerRefV1;
use super::director_store::{DirectorSnapshotV1, DirectorStoreV1};
use super::dossier::{DOSSIER_SCHEMA_V1, DossierV1};
use super::dossier_store::DossierStoreV1;
use super::full_portability::{export_full_portable_state, import_full_portable_state};
use super::full_portability_observations::PortableObservationReceiptV1;
use super::journal::{ActionUpdateV1, canonical_action_key};
use super::leases::{LeaseError, LeasePhaseV1, WorkLeaseKeyV1};
use super::portability::PortableEntryV1;
use super::recovery::recover_store;
use super::scheduler::SchedulerStateV1;
use super::schema::{ActionIntentV1, ActionKindV1, ActionPhaseV1, DirectorHealthStateV1, LaneIdV1};
use super::store::CompetitionStore;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "angel-full-portable-{tag}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn digest(value: &str) -> String {
    crate::cut::sha256_hex(value.as_bytes())
}

fn populate(root: &TestRoot, subject: &str) -> (Vec<u8>, WorkLeaseKeyV1, u64) {
    let board = super::migration_tests::board();
    let competition = board.competition.clone();
    let action_store = CompetitionStore::new(root.0.join("action"));
    let key = WorkLeaseKeyV1 {
        campaign_id: "campaign-i3".into(),
        lane_id: LaneIdV1::FrontierGuard,
        work_item_id: "frontier-work".into(),
    };
    let lease = action_store
        .update_leases(|leases| leases.grant(key.clone(), "worker-i3".into(), 10, 500_000, 7))
        .unwrap();
    let mut intent = ActionIntentV1 {
        action_key: String::new(),
        campaign_id: "campaign-i3".into(),
        competition,
        kind: ActionKindV1::DispatchWorker,
        subject_id: subject.into(),
        payload_sha256: digest(&format!("payload:{subject}")),
        intent_version: "full-portable/v1".into(),
    };
    intent.action_key = canonical_action_key(&intent).unwrap();
    action_store
        .append_action_fenced(
            &key,
            lease.fencing_generation,
            ActionUpdateV1 {
                intent,
                attempt: 0,
                phase: ActionPhaseV1::Planned,
                retryable: false,
                at_ms: 11,
                reconcile_key: None,
                receipt_sha256: None,
                next: None,
            },
        )
        .unwrap();
    let journal_head = action_store.recover().unwrap().state.head_sha256;
    let mut dossier = DossierV1::new(
        "dossier-i3".into(),
        "project-i3".into(),
        "repository-i3".into(),
        "revision-i3".into(),
        3,
        journal_head.clone(),
    )
    .unwrap();
    dossier
        .rollover_fresh_turn(
            "turn-i3".into(),
            journal_head.clone(),
            "portable director continuation".into(),
        )
        .unwrap();
    DossierStoreV1::new(root.0.join("dossier"))
        .sync(&dossier)
        .unwrap();

    let mut director = CompetitionDirectorStateV1::engage("campaign-i3", 0).unwrap();
    director
        .apply_board_outcome(
            &super::director_tests::outcome(board, super::board::BoardReduceEffectV1::Initialized),
            1,
        )
        .unwrap();
    let mut next = director.clone();
    next.workers.insert(
        lease.lease_id.clone(),
        DirectorWorkerRefV1 {
            work_item_id: key.work_item_id.clone(),
            lane: key.lane_id,
            lease_id: lease.lease_id.clone(),
            fencing_generation: lease.fencing_generation,
            checkpoint_revision: lease.checkpoint_revision,
        },
    );
    director
        .commit(
            next,
            DirectorHealthStateV1::Retrying,
            Some("portable fixture"),
            "resume-full-state",
            60,
        )
        .unwrap();
    let mut scheduler = SchedulerStateV1::new(2, 8).unwrap();
    scheduler.observe_board_move(9).unwrap();
    scheduler
        .checkpoint_lane(LaneIdV1::FrontierGuard, lease.checkpoint_revision)
        .unwrap();
    let snapshot =
        DirectorSnapshotV1::new(director, scheduler, journal_head, dossier.dossier_revision)
            .unwrap();
    DirectorStoreV1::new(root.0.join("director"))
        .persist(&snapshot)
        .unwrap();
    (
        export_full_portable_state(root.path(), "campaign-i3").unwrap(),
        key,
        lease.fencing_generation,
    )
}

#[test]
fn full_state_cross_root_import_is_canonical_and_fences_workers_ac15() {
    let source = TestRoot::new("source");
    let left = TestRoot::new("left");
    let right = TestRoot::new("right");
    let (bundle, key, old_generation) = populate(&source, "source-action");
    let encoded = String::from_utf8(bundle.clone()).unwrap();
    assert!(!encoded.contains(source.path().to_str().unwrap()));
    let left_receipt = import_full_portable_state(&bundle, left.path()).unwrap();
    let right_receipt = import_full_portable_state(&bundle, right.path()).unwrap();
    assert_eq!(left_receipt, right_receipt);
    assert_eq!(
        DirectorStoreV1::new(left.0.join("director"))
            .recover()
            .unwrap(),
        Some(left_receipt.snapshot.clone())
    );
    assert_eq!(
        DirectorStoreV1::new(right.0.join("director"))
            .recover()
            .unwrap(),
        Some(right_receipt.snapshot.clone())
    );
    let left_leases = CompetitionStore::new(left.0.join("action"))
        .recover_leases()
        .unwrap();
    let imported = left_leases.current(&key).unwrap();
    assert_eq!(imported.fencing_generation, old_generation + 1);
    assert_eq!(imported.phase, LeasePhaseV1::Fenced);
    assert_eq!(
        left_leases.accept_landing(&key, old_generation),
        Err(LeaseError::StaleGeneration)
    );
    assert_eq!(
        recover_store(&CompetitionStore::new(left.0.join("action")))
            .unwrap()
            .journal,
        recover_store(&CompetitionStore::new(right.0.join("action")))
            .unwrap()
            .journal
    );
    assert_eq!(
        DossierStoreV1::new(left.0.join("dossier")).load().unwrap(),
        DossierStoreV1::new(right.0.join("dossier")).load().unwrap()
    );
    assert_eq!(
        left_receipt
            .snapshot
            .director
            .board
            .as_ref()
            .unwrap()
            .board_epoch,
        3
    );
    assert_eq!(
        left_receipt.snapshot.director.submissions.schema,
        "angel.competition-submission-spool/v1"
    );
    assert_eq!(
        DossierStoreV1::new(left.0.join("dossier"))
            .load()
            .unwrap()
            .unwrap()
            .schema,
        DOSSIER_SCHEMA_V1
    );
}

#[test]
fn full_state_rejects_tamper_missing_and_cross_composition_ac15() {
    let source = TestRoot::new("adversarial-source");
    let donor = TestRoot::new("adversarial-donor");
    let target = TestRoot::new("adversarial-target");
    let (bundle, _, _) = populate(&source, "source-action");
    let (donor_bundle, _, _) = populate(&donor, "donor-action");

    let mut tampered: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
    tampered["snapshot"]["director"]["campaign_id"] = "tampered".into();
    assert!(
        import_full_portable_state(&serde_json::to_vec(&tampered).unwrap(), target.path()).is_err()
    );
    assert!(!target.path().exists());

    let mut missing: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
    missing["entries"]
        .as_array_mut()
        .unwrap()
        .retain(|entry| entry["path"] != "action/leases.snapshot.json");
    reseal(&mut missing);
    assert!(
        import_full_portable_state(&serde_json::to_vec(&missing).unwrap(), target.path()).is_err()
    );

    let mut mixed: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
    let donor: serde_json::Value = serde_json::from_slice(&donor_bundle).unwrap();
    let donor_action = donor["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == "action/actions.jsonl")
        .unwrap()
        .clone();
    *mixed["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["path"] == "action/actions.jsonl")
        .unwrap() = donor_action;
    reseal(&mut mixed);
    assert!(
        import_full_portable_state(&serde_json::to_vec(&mixed).unwrap(), target.path()).is_err()
    );
    assert!(!target.path().exists());
}

pub(super) fn reseal(value: &mut serde_json::Value) {
    let snapshot: DirectorSnapshotV1 = serde_json::from_value(value["snapshot"].clone()).unwrap();
    let observations: Vec<PortableObservationReceiptV1> =
        serde_json::from_value(value["observation_history"].clone()).unwrap();
    let entries: Vec<PortableEntryV1> = serde_json::from_value(value["entries"].clone()).unwrap();
    let material = serde_json::to_vec(&(
        value["schema"].as_str().unwrap(),
        value["contract"].as_str().unwrap(),
        value["campaign_id"].as_str().unwrap(),
        snapshot,
        observations,
        entries,
    ))
    .unwrap();
    value["bundle_sha256"] = crate::cut::sha256_hex(&material).into();
}
