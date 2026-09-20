use super::board::{
    BoardEntryV1, BoardFreshnessV1, CANONICAL_BOARD_SCHEMA_V1, CanonicalBoardV1,
    ObservationProvenanceV1, ObservationSourceV1, SourceAccessClaimV1,
};
use super::candidate_store::CandidateRepositoryV1;
use super::director_services::{DirectorServiceIntentV1, DirectorServiceKindV1};
use super::director_store::{DirectorPublishDispositionV1, DirectorSnapshotV1, DirectorStoreV1};
use super::dossier::DossierV1;
use super::dossier_store::DossierStoreV1;
use super::frontier::reduce_frontier;
use super::journal::ActionUpdateV1;
use super::leases::WorkLeaseKeyV1;
use super::migration::{
    LEGACY_DIRECTOR_SCHEMA_V0, LegacyDirectorStateV0, LegacyPendingActionV0, LegacyPersistenceV0,
    MigrationDispositionV1, migrate_director_state,
};
use super::migration_qualification::{migration_action_intent, qualify_migration};
use super::patterns::FieldPatternMemoryV1;
use super::rewards::RewardLedgerV1;
use super::scheduler::SchedulerStateV1;
use super::schema::{
    ActionPhaseV1, AdapterCapabilityV1, AdapterIdentityV1, ComparatorKindV1, CompetitionKeyV1,
    LaneIdV1, ObjectiveComparatorV1, ScoreV1,
};
use super::store::CompetitionStore;
use super::submission::SubmissionSpoolV1;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
fn digest(value: &str) -> String {
    crate::knowledge::cut::sha256_hex(value.as_bytes())
}
pub(super) fn board() -> CanonicalBoardV1 {
    let source = SourceAccessClaimV1 {
        source_id: "source-base".into(),
        commit_oid: digest("commit"),
        tree_oid: digest("tree"),
        workspace_sha256: digest("workspace"),
        access_proof_sha256: digest("access"),
    };
    let entry = BoardEntryV1 {
        entry_id: "entry-base".into(),
        participant_id: "participant".into(),
        submission_id: Some("submission-base".into()),
        rank: Some(1),
        score: ScoreV1::new("1.25").unwrap(),
        personal: true,
        source: Some(source.clone()),
    };
    let comparator = ObjectiveComparatorV1 {
        objective_id: "score".into(),
        version: "v1".into(),
        kind: ComparatorKindV1::HigherIsBetter,
    };
    let entries = vec![entry];
    let frontier = reduce_frontier(
        &entries,
        &comparator,
        3,
        "observation-legacy",
        |comparator, candidate, baseline| Ok(comparator.compare(candidate, baseline).unwrap()),
    )
    .unwrap();
    let decision_sha256 = frontier.decision_sha256(&comparator).unwrap();
    CanonicalBoardV1 {
        schema: CANONICAL_BOARD_SCHEMA_V1.into(),
        campaign_id: "campaign-i3".into(),
        competition: CompetitionKeyV1 {
            platform_id: "fixture".into(),
            competition_id: "contest".into(),
            field_id: "kernels".into(),
            benchmark_id: "bench".into(),
            profile_id: "profile".into(),
            hardware_id: "gpu".into(),
        },
        comparator,
        board_epoch: 3,
        predecessor_epoch: Some(2),
        observation_revision: 9,
        entries,
        frontier,
        decision_sha256,
        latest_observation_id: "observation-legacy".into(),
        latest_provenance: ObservationProvenanceV1 {
            adapter: AdapterIdentityV1 {
                adapter_id: "legacy-fixture".into(),
                adapter_version: "v0".into(),
                runtime_sha256: digest("adapter"),
                capabilities: BTreeSet::from([
                    AdapterCapabilityV1::Board,
                    AdapterCapabilityV1::SourceAccess,
                ]),
            },
            source: ObservationSourceV1::LegacyImport,
            observed_at_ms: 11,
            platform_event_at_ms: Some(10),
            raw_sha256: digest("raw-observation"),
        },
        last_sequence: Some(7),
        freshness: BoardFreshnessV1::Fresh,
    }
}
pub(super) fn legacy() -> LegacyDirectorStateV0 {
    let board = board();
    LegacyDirectorStateV0 {
        schema: LEGACY_DIRECTOR_SCHEMA_V0.into(),
        revision: 12,
        campaign_id: "campaign-i3".into(),
        candidates: Some(
            CandidateRepositoryV1::new(board.board_epoch, board.decision_sha256.clone()).unwrap(),
        ),
        last_good_board: Some(board),
        submissions: SubmissionSpoolV1::new(),
        rewards: RewardLedgerV1::default(),
        patterns: FieldPatternMemoryV1::default(),
        episodes: BTreeMap::new(),
        workers: BTreeMap::new(),
        pending_actions: vec![LegacyPendingActionV0 {
            intent: DirectorServiceIntentV1::new(
                "campaign-i3",
                DirectorServiceKindV1::StepSubmissions,
                40,
            )
            .unwrap(),
            provenance_sha256: digest("pending-submission-provenance"),
        }],
        action_journal: LegacyPersistenceV0::Absent,
        dossier: LegacyPersistenceV0::Absent,
    }
}
#[test]
fn installed_upgrade_migrates_v1_and_resumes_ac15() {
    let legacy = legacy();
    let provenance = legacy
        .last_good_board
        .as_ref()
        .unwrap()
        .latest_provenance
        .clone();
    let raw = serde_json::to_vec(&legacy).unwrap();
    let first = migrate_director_state(&raw, 50).unwrap();
    let replay = migrate_director_state(&raw, 50).unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.disposition, MigrationDispositionV1::MigratedLegacyV0);
    assert!(first.qualification_required().unwrap());
    assert_eq!(
        first.health().state,
        super::schema::DirectorHealthStateV1::NeedsAttention
    );
    assert_eq!(first.board().unwrap().latest_provenance, provenance);
    assert!(matches!(
        first.board().unwrap().freshness,
        BoardFreshnessV1::Stale { .. }
    ));
    assert!(first.services().iter().any(|service| {
        matches!(service.kind, DirectorServiceKindV1::StepSubmissions) && service.due_at_ms == 40
    }));
    assert!(first.services().iter().any(|service| matches!(
        service.kind,
        DirectorServiceKindV1::RefreshBoard { full: true }
    )));
    let root = test_root("installed-upgrade");
    let action_store = CompetitionStore::new(root.join("action"));
    let key = WorkLeaseKeyV1 {
        campaign_id: "campaign-i3".into(),
        lane_id: LaneIdV1::ContextMiner,
        work_item_id: "qualify-migration".into(),
    };
    let lease = action_store
        .update_leases(|book| book.grant(key.clone(), "migration-worker".into(), 50, 500, 0))
        .unwrap();
    let competition = first.board().unwrap().competition.clone();
    let intent = migration_action_intent(&first, competition.clone()).unwrap();
    for (phase, receipt_sha256) in [
        (ActionPhaseV1::Planned, None),
        (ActionPhaseV1::Started, None),
        (ActionPhaseV1::Completed, Some(first.target_sha256.clone())),
    ] {
        action_store
            .append_action_fenced(
                &key,
                lease.fencing_generation,
                ActionUpdateV1 {
                    intent: intent.clone(),
                    attempt: 0,
                    phase,
                    retryable: false,
                    at_ms: 51,
                    reconcile_key: None,
                    receipt_sha256,
                    next: None,
                },
            )
            .unwrap();
    }
    let journal = action_store.recover().unwrap().state;
    let mut dossier = DossierV1::new(
        "migration-dossier".into(),
        "migration-project".into(),
        "migration-repository".into(),
        "migration-revision".into(),
        3,
        journal.head_sha256.clone(),
    )
    .unwrap();
    dossier
        .rollover_fresh_turn(
            "migration-turn".into(),
            journal.head_sha256.clone(),
            "qualify migrated persistence".into(),
        )
        .unwrap();
    DossierStoreV1::new(root.join("dossier"))
        .sync(&dossier)
        .unwrap();
    let qualified = qualify_migration(&first, &journal, &dossier, competition, 60).unwrap();
    assert_eq!(
        qualified.receipt.pending_action_provenance,
        first.pending_action_provenance
    );
    let mut scheduler = SchedulerStateV1::new(2, 8).unwrap();
    scheduler.observe_board_move(9).unwrap();
    let qualified_state = qualified.state.clone();
    let snapshot = DirectorSnapshotV1::new_with_migration(
        qualified.state,
        scheduler,
        qualified.receipt.journal_head_sha256.clone(),
        qualified.receipt.dossier_revision,
        Some(qualified.receipt),
    )
    .unwrap();
    let store = DirectorStoreV1::new(root.clone());
    assert_eq!(
        store.persist(&snapshot).unwrap(),
        DirectorPublishDispositionV1::Published
    );
    assert_eq!(
        store.persist(&snapshot).unwrap(),
        DirectorPublishDispositionV1::Replayed
    );
    assert_eq!(store.recover().unwrap(), Some(snapshot));
    let migrated_raw = serde_json::to_vec(&qualified_state).unwrap();
    let already = migrate_director_state(&migrated_raw, 50).unwrap();
    assert_eq!(already.disposition, MigrationDispositionV1::AlreadyV1);
    assert!(already.matches_director(&qualified_state));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn migration_replaces_board_conflicts_but_preserves_pending_provenance() {
    for kind in [
        DirectorServiceKindV1::RefreshBoard { full: false },
        DirectorServiceKindV1::RemediateBoardAdapter,
    ] {
        let mut legacy = legacy();
        let intent = DirectorServiceIntentV1::new("campaign-i3", kind, 41).unwrap();
        let id = intent.intent_id.clone();
        let provenance = digest(&format!("pending:{id}"));
        legacy.pending_actions.push(LegacyPendingActionV0 {
            intent,
            provenance_sha256: provenance.clone(),
        });
        let migrated = migrate_director_state(&serde_json::to_vec(&legacy).unwrap(), 50).unwrap();
        assert_eq!(migrated.pending_action_provenance[&id], provenance);
        let board = migrated
            .services()
            .iter()
            .filter(|service| service.class() == "board")
            .collect::<Vec<_>>();
        assert!(matches!(
            board.as_slice(),
            [service] if matches!(service.kind, DirectorServiceKindV1::RefreshBoard { full: true })
        ));
    }
}
#[test]
fn migration_rejects_unknown_newer_and_malformed_without_mutating_source() {
    let mut value = serde_json::to_value(legacy()).unwrap();
    value["schema"] = "angel.competition-director-state/v99".into();
    let future = serde_json::to_vec(&value).unwrap();
    let preserved = future.clone();
    assert!(migrate_director_state(&future, 50).is_err());
    assert_eq!(future, preserved);
    let mut malformed = serde_json::to_value(legacy()).unwrap();
    malformed["revision"] = 0.into();
    assert!(migrate_director_state(&serde_json::to_vec(&malformed).unwrap(), 50).is_err());
}
fn test_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("angel-migration-{tag}-{}", std::process::id()))
}
