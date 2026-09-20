use super::dossier::{
    AnchorFreshnessV1, AnchorKeyV1, AnchorProvenanceV1, DossierV1, RelevantReadV1,
};
use super::dossier_store::DossierStoreV1;
use super::schema::ScheduledActionV1;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct TestDir(PathBuf);

impl TestDir {
    fn new(tag: &str) -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "angel-dossier-{tag}-{}-{}",
                std::process::id(),
                TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("reserve dossier test directory: {error}"),
            }
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn dossier() -> DossierV1 {
    DossierV1::new(
        "dossier-r2".into(),
        "angel".into(),
        "angel-x".into(),
        "revision-one".into(),
        7,
        crate::cut::sha256_hex(b"journal-head-one"),
    )
    .unwrap()
}

pub(super) fn relevant(source: &str, revision: &str, content: &str) -> RelevantReadV1 {
    RelevantReadV1 {
        key: AnchorKeyV1 {
            source_id: source.into(),
            canonical_path: format!("src/{source}.rs"),
            symbol_or_range: format!("{source}:1-20"),
            source_revision: revision.into(),
            content_sha256: crate::cut::sha256_hex(content.as_bytes()),
        },
        provenance: AnchorProvenanceV1 {
            kind: "repository_read".into(),
            reference: format!("tool://read/{source}/{revision}"),
            receipt_sha256: crate::cut::sha256_hex(
                format!("receipt:{source}:{revision}").as_bytes(),
            ),
        },
        confidence_millis: 900,
        reacquire: ScheduledActionV1 {
            action: format!("reacquire:{source}"),
            next_attempt_at_ms: 1,
        },
    }
}

pub(super) fn stored_one(tag: &str) -> (TestDir, DossierV1, String, String) {
    let dir = TestDir::new(tag);
    let mut dossier = dossier();
    dossier
        .admit_relevant(relevant("source", "revision-one", "content"))
        .unwrap();
    DossierStoreV1::new(dir.path().into())
        .sync(&dossier)
        .unwrap();
    let pointer: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("current.json")).unwrap()).unwrap();
    let generation = pointer["generation"].as_str().unwrap().to_string();
    let anchor = dossier.anchors.keys().next().unwrap().clone();
    (dir, dossier, generation, anchor)
}

fn overwrite(path: &Path, body: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .unwrap();
    file.write_all(body).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn novel_relevant_reads_256_never_count_denied_ac09() {
    let dir = TestDir::new("novel-256");
    let mut dossier = dossier();
    for index in 0..256 {
        assert!(
            dossier
                .admit_relevant(relevant(
                    &format!("source-{index}"),
                    "revision-one",
                    &format!("content-{index}"),
                ))
                .unwrap()
        );
    }
    assert_eq!(
        (dossier.anchors.len(), dossier.dossier_revision),
        (256, 256)
    );
    let store = DossierStoreV1::new(dir.path().into());
    store.sync(&dossier).unwrap();
    assert_eq!(store.load().unwrap().unwrap(), dossier);
}

#[test]
fn unchanged_reread_reuses_anchor_changed_digest_reacquires_ac09() {
    let mut dossier = dossier();
    let first = relevant("stable-source", "revision-one", "content-one");
    assert!(dossier.admit_relevant(first.clone()).unwrap());
    assert!(!dossier.admit_relevant(first).unwrap());
    assert_eq!((dossier.anchors.len(), dossier.dossier_revision), (1, 1));

    let changed_digest = relevant("stable-source", "revision-one", "content-two");
    assert!(dossier.admit_relevant(changed_digest).unwrap());
    assert_eq!((dossier.anchors.len(), dossier.dossier_revision), (2, 2));
    assert_eq!(
        dossier
            .anchors
            .values()
            .filter(|anchor| matches!(anchor.freshness, AnchorFreshnessV1::Stale { .. }))
            .count(),
        1
    );

    let changed_revision = relevant("stable-source", "revision-two", "content-two");
    dossier.admit_relevant(changed_revision).unwrap();
    let mut changed_path = relevant("stable-source", "revision-two", "content-two");
    changed_path.key.canonical_path = "renamed/stable.rs".into();
    assert!(dossier.admit_relevant(changed_path.clone()).unwrap());
    changed_path.key.symbol_or_range = "renamed_symbol:30-40".into();
    assert!(dossier.admit_relevant(changed_path).unwrap());
    assert_eq!((dossier.anchors.len(), dossier.dossier_revision), (5, 5));
    assert_eq!(
        dossier
            .anchors
            .values()
            .filter(|anchor| matches!(anchor.freshness, AnchorFreshnessV1::Fresh))
            .count(),
        1
    );
}

#[test]
fn dossier_compact_restart_preserves_revision_and_provenance_ac09() {
    let dir = TestDir::new("compact-restart");
    let mut dossier = dossier();
    for index in 0..130 {
        dossier
            .admit_relevant(relevant(
                &format!("evidence-{index}"),
                "revision-one",
                &format!("payload-{index}"),
            ))
            .unwrap();
    }
    dossier
        .rollover_fresh_turn(
            "fresh-turn-2".into(),
            crate::cut::sha256_hex(b"journal-head-two"),
            "storage pressure and context turn exhausted".into(),
        )
        .unwrap();
    dossier
        .admit_relevant(relevant("after-rollover", "revision-one", "continued"))
        .unwrap();
    assert_eq!(dossier.dossier_revision, 132);
    let expected_provenance = dossier.anchors.values().next().unwrap().provenance.clone();
    let store = DossierStoreV1::new(dir.path().into());
    store.sync(&dossier).unwrap();
    let restored = DossierStoreV1::new(dir.path().into())
        .load()
        .unwrap()
        .unwrap();
    assert_eq!(restored, dossier);
    assert_eq!(
        restored.anchors.values().next().unwrap().provenance,
        expected_provenance
    );
    assert_eq!(restored.checkpoint.as_ref().unwrap().dossier_revision, 132);
}

#[test]
fn checkpoint_and_revision_failures_preserve_current() {
    let (dir, mut current, _, _) = stored_one("checkpoint-atomic");
    current
        .admit_relevant(relevant("source", "revision-two", "changed"))
        .unwrap();
    current
        .rollover_fresh_turn(
            "turn-2".into(),
            crate::cut::sha256_hex(b"head-2"),
            "continue".into(),
        )
        .unwrap();
    let store = DossierStoreV1::new(dir.path().into());
    store.sync(&current).unwrap();
    let current_path = dir.path().join("current.json");
    let before = std::fs::read(&current_path).unwrap();
    let rejected = |state: &DossierV1| {
        assert!(store.sync(state).is_err());
        assert_eq!(std::fs::read(&current_path).unwrap(), before);
    };
    let mut bad = current.clone();
    bad.checkpoint.as_mut().unwrap().stale_anchor_ids.clear();
    rejected(&bad);
    let mut bad = current.clone();
    bad.checkpoint.as_mut().unwrap().checkpoint_id = crate::cut::sha256_hex(b"wrong");
    rejected(&bad);
    let mut bad = current.clone();
    bad.checkpoint.as_mut().unwrap().reason.push_str(" changed");
    rejected(&bad);
    let mut rollback = dossier();
    rollback
        .admit_relevant(relevant("rollback", "revision-one", "one"))
        .unwrap();
    rejected(&rollback);
    let mut conflict = current.clone();
    conflict.board_epoch += 1;
    rejected(&conflict);

    for path in ["/absolute/path", "src/../escape", "C:/absolute/path"] {
        let mut invalid = relevant("path", "revision-one", "content");
        invalid.key.canonical_path = path.into();
        assert!(rollback.admit_relevant(invalid).is_err());
    }
    assert_eq!(rollback.dossier_revision, 1);
}

#[test]
fn malformed_current_pointer_fails_closed() {
    let (dir, _, _, _) = stored_one("malformed-pointer");
    overwrite(&dir.path().join("current.json"), br#"{"schema":"broken""#);
    assert!(DossierStoreV1::new(dir.path().into()).load().is_err());
}

#[test]
fn torn_anchor_fails_closed_without_partial_load() {
    let (dir, _, generation, anchor_id) = stored_one("torn-anchor");
    let path = dir
        .path()
        .join("generations")
        .join(generation)
        .join("anchors/shard-000000")
        .join(format!("{anchor_id}.json"));
    overwrite(&path, br#"{"anchor_id":"torn""#);
    assert!(DossierStoreV1::new(dir.path().into()).load().is_err());
}
