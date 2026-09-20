use super::candidate::port_replay_candidate;
use super::candidate_store::*;
use super::candidate_tests::{artifact, board, evidence, origin};
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-candidate-store-{tag}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repository() -> CandidateRepositoryV1 {
    let current = board(1, "100", "source-a", "60");
    let candidate = origin(&Default::default(), &current);
    let mut repository =
        CandidateRepositoryV1::new(current.board_epoch, current.decision_sha256.clone()).unwrap();
    repository.insert(candidate).unwrap();
    repository
}

fn advanced(mut repository: CandidateRepositoryV1) -> CandidateRepositoryV1 {
    let moved = board(2, "110", "source-b", "70");
    repository
        .set_current_board(moved.board_epoch, moved.decision_sha256)
        .unwrap();
    repository
}

#[test]
fn candidate_restart_preserves_named_current_epoch_eligibility() {
    let dir = TestDir::new("restart");
    let store = CandidateStoreV1::new(dir.0.clone());
    let mut repository = repository();
    store.persist(&repository).unwrap();
    let recovered = store.recover().unwrap().unwrap();
    assert_eq!(recovered, repository);
    recovered.require_eligible("candidate-origin").unwrap();

    let moved = board(2, "110", "source-b", "70");
    repository
        .set_current_board(moved.board_epoch, moved.decision_sha256.clone())
        .unwrap();
    assert!(repository.require_eligible("candidate-origin").is_err());
    store.persist(&repository).unwrap();

    let child = port_replay_candidate(
        &repository.catalog,
        "candidate-current",
        "candidate-origin",
        "episode-current",
        &moved,
        artifact("candidate-current"),
        evidence("candidate-current"),
    )
    .unwrap();
    repository.insert(child).unwrap();
    repository.require_eligible("candidate-current").unwrap();
    store.persist(&repository).unwrap();
    assert_eq!(store.recover().unwrap(), Some(repository));
}

#[test]
fn candidate_store_invalid_update_is_atomic_and_torn_or_malformed_fail_closed() {
    let dir = TestDir::new("atomic");
    let store = CandidateStoreV1::new(dir.0.clone());
    let repository = repository();
    store.persist(&repository).unwrap();
    let before = std::fs::read(store.snapshot_path()).unwrap();
    let mut invalid = repository.clone();
    invalid.revision += 1;
    invalid.eligibility.clear();
    assert!(store.persist(&invalid).is_err());
    assert_eq!(std::fs::read(store.snapshot_path()).unwrap(), before);
    assert_eq!(store.recover().unwrap(), Some(repository));

    let torn_dir = TestDir::new("torn");
    std::fs::create_dir_all(&torn_dir.0).unwrap();
    let torn = CandidateStoreV1::new(torn_dir.0.clone());
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(torn.snapshot_path())
        .unwrap();
    file.write_all(br#"{"schema":"angel.competition"#).unwrap();
    file.sync_all().unwrap();
    assert!(torn.recover().is_err());

    let malformed_dir = TestDir::new("malformed");
    let malformed = CandidateStoreV1::new(malformed_dir.0.clone());
    std::fs::create_dir_all(&malformed_dir.0).unwrap();
    std::fs::write(malformed.snapshot_path(), b"{malformed}").unwrap();
    assert!(malformed.recover().is_err());
}

#[test]
fn candidate_publish_cutpoints_preserve_or_qualify_one_authoritative_revision() {
    let pre_dir = TestDir::new("pre-rename");
    let mut pre = CandidateStoreV1::new(pre_dir.0.clone());
    let old = repository();
    let new = advanced(old.clone());
    pre.persist(&old).unwrap();
    pre.inject_failure(CandidatePublishFailureV1::PreRename);
    assert!(pre.persist(&new).is_err());
    let retry = CandidateStoreV1::new(pre_dir.0.clone());
    assert_eq!(retry.recover().unwrap(), Some(old));
    retry.persist(&new).unwrap();
    assert_eq!(retry.recover().unwrap(), Some(new.clone()));

    let post_dir = TestDir::new("post-rename");
    let mut post = CandidateStoreV1::new(post_dir.0.clone());
    post.persist(&repository()).unwrap();
    post.inject_failure(CandidatePublishFailureV1::PostRename);
    assert!(post.persist(&new).is_err());
    let qualify = CandidateStoreV1::new(post_dir.0.clone());
    assert_eq!(qualify.recover().unwrap(), Some(new.clone()));
    #[cfg(unix)]
    {
        std::fs::set_permissions(
            qualify.snapshot_path(),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
    }
    qualify.persist(&new).unwrap();
    #[cfg(unix)]
    assert_eq!(
        std::fs::metadata(qualify.snapshot_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
