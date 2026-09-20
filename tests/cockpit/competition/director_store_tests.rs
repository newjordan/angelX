use super::director::CompetitionDirectorStateV1;
use super::director_store::{
    DirectorCommitCutpointV1, DirectorPublishDispositionV1, DirectorSnapshotV1,
    DirectorStoreErrorV1, DirectorStoreV1,
};
use super::migration::migrate_director_state;
use super::scheduler::SchedulerStateV1;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "angel-director-{tag}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )))
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

fn snapshot() -> DirectorSnapshotV1 {
    DirectorSnapshotV1::new(
        CompetitionDirectorStateV1::engage("campaign-i3", 10).unwrap(),
        SchedulerStateV1::new(2, 8).unwrap(),
        crate::knowledge::cut::sha256_hex(b"empty-action-head"),
        0,
    )
    .unwrap()
}

fn next_snapshot(current: &DirectorSnapshotV1) -> DirectorSnapshotV1 {
    let mut director = current.director.clone();
    let next = director.clone();
    director.commit_preserving_health(next).unwrap();
    DirectorSnapshotV1::new(
        director,
        current.scheduler.clone(),
        current.action_journal_head_sha256.clone(),
        current.dossier_revision,
    )
    .unwrap()
}

#[test]
fn director_snapshot_roundtrip_replay_and_revision_order_ac13() {
    let dir = TestDir::new("roundtrip");
    let store = DirectorStoreV1::new(dir.path().into());
    let first = snapshot();
    assert_eq!(
        store.persist(&first).unwrap(),
        DirectorPublishDispositionV1::Published
    );
    assert_eq!(store.recover().unwrap(), Some(first.clone()));
    assert_eq!(
        store.persist(&first).unwrap(),
        DirectorPublishDispositionV1::Replayed
    );
    let second = next_snapshot(&first);
    assert_eq!(
        store.persist(&second).unwrap(),
        DirectorPublishDispositionV1::Published
    );
    assert_eq!(store.recover().unwrap(), Some(second));

    let skipped = next_snapshot(&next_snapshot(&next_snapshot(&first)));
    assert!(store.persist(&skipped).is_err());
}

#[test]
fn director_snapshot_cutpoints_are_deterministically_qualified_ac13() {
    let pre_dir = TestDir::new("pre-rename");
    let mut pre = DirectorStoreV1::new(pre_dir.path().into());
    pre.inject_failure(DirectorCommitCutpointV1::BeforeRename);
    assert!(matches!(
        pre.persist(&snapshot()),
        Err(DirectorStoreErrorV1::Commit {
            cutpoint: DirectorCommitCutpointV1::BeforeRename,
            ..
        })
    ));
    assert_eq!(
        DirectorStoreV1::new(pre_dir.path().into())
            .recover()
            .unwrap(),
        None
    );

    let post_dir = TestDir::new("post-rename");
    let expected = snapshot();
    let mut post = DirectorStoreV1::new(post_dir.path().into());
    post.inject_failure(DirectorCommitCutpointV1::AfterRenameAmbiguous);
    assert!(matches!(
        post.persist(&expected),
        Err(DirectorStoreErrorV1::Commit {
            cutpoint: DirectorCommitCutpointV1::AfterRenameAmbiguous,
            ..
        })
    ));
    assert_eq!(
        DirectorStoreV1::new(post_dir.path().into())
            .recover()
            .unwrap(),
        Some(expected)
    );

    let update_dir = TestDir::new("replacement-cutpoints");
    let first = snapshot();
    let second = next_snapshot(&first);
    DirectorStoreV1::new(update_dir.path().into())
        .persist(&first)
        .unwrap();
    let mut failed_update = DirectorStoreV1::new(update_dir.path().into());
    failed_update.inject_failure(DirectorCommitCutpointV1::BeforeRename);
    assert!(failed_update.persist(&second).is_err());
    assert_eq!(
        DirectorStoreV1::new(update_dir.path().into())
            .recover()
            .unwrap(),
        Some(first)
    );
    let mut ambiguous_update = DirectorStoreV1::new(update_dir.path().into());
    ambiguous_update.inject_failure(DirectorCommitCutpointV1::AfterRenameAmbiguous);
    assert!(ambiguous_update.persist(&second).is_err());
    assert_eq!(
        DirectorStoreV1::new(update_dir.path().into())
            .recover()
            .unwrap(),
        Some(second.clone())
    );
    let third = next_snapshot(&second);
    let mut sync_failure = DirectorStoreV1::new(update_dir.path().into());
    sync_failure.inject_directory_sync_failure();
    assert!(matches!(
        sync_failure.persist(&third),
        Err(DirectorStoreErrorV1::Commit {
            cutpoint: DirectorCommitCutpointV1::AfterRenameAmbiguous,
            ..
        })
    ));
    assert_eq!(
        DirectorStoreV1::new(update_dir.path().into())
            .recover()
            .unwrap(),
        Some(third)
    );
}

#[test]
fn director_snapshot_malformed_or_tampered_load_fails_closed_ac13() {
    let dir = TestDir::new("tamper");
    let store = DirectorStoreV1::new(dir.path().into());
    let expected = snapshot();
    store.persist(&expected).unwrap();
    let mut value = serde_json::to_value(&expected).unwrap();
    value["director"]["campaign_id"] = "different-campaign".into();
    std::fs::write(store.snapshot_path(), serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(store.recover().is_err());

    std::fs::write(store.snapshot_path(), br#"{"schema":"torn""#).unwrap();
    assert!(store.recover().is_err());
}

#[test]
fn migration_marker_survives_health_commit_and_blocks_unqualified_snapshot_ac15() {
    let raw = serde_json::to_vec(&super::migration_tests::legacy()).unwrap();
    let mut migration = migrate_director_state(&raw, 50).unwrap();
    migration.commit_health_drift_for_test().unwrap();
    assert_eq!(
        migration.health().reason.as_deref(),
        Some("unrelated health drift")
    );
    let mut scheduler = SchedulerStateV1::new(2, 8).unwrap();
    scheduler.observe_board_move(9).unwrap();
    assert!(
        DirectorSnapshotV1::new_migrated(
            &migration,
            scheduler,
            crate::knowledge::cut::sha256_hex(b"invented-legacy-head"),
            0,
        )
        .is_err()
    );
}
