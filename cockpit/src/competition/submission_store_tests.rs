use super::submission_store::{SubmissionPublishFailureV1, SubmissionStoreV1};
use super::submission_tests::{TestDir, repository_with, spool_one};
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn submission_store_restarts_and_invalid_publish_is_failure_atomic() {
    let dir = TestDir::new("spool-atomic");
    let store = SubmissionStoreV1::new(dir.0.clone());
    let spool = spool_one();
    store.persist(&spool).unwrap();
    assert_eq!(store.recover().unwrap(), Some(spool.clone()));
    let before = std::fs::read(store.snapshot_path()).unwrap();
    let mut invalid = spool;
    invalid.revision += 1;
    invalid.items.get_mut(&0).unwrap().action_key = "forged".into();
    assert!(store.persist(&invalid).is_err());
    assert_eq!(std::fs::read(store.snapshot_path()).unwrap(), before);
}

#[test]
fn submission_store_torn_and_malformed_snapshots_fail_closed() {
    let torn_dir = TestDir::new("spool-torn");
    std::fs::create_dir_all(&torn_dir.0).unwrap();
    let torn = SubmissionStoreV1::new(torn_dir.0.clone());
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(torn.snapshot_path())
        .unwrap();
    file.write_all(br#"{"schema":"angel.competition"#).unwrap();
    file.sync_all().unwrap();
    assert!(torn.recover().is_err());

    let malformed_dir = TestDir::new("spool-malformed");
    std::fs::create_dir_all(&malformed_dir.0).unwrap();
    let malformed = SubmissionStoreV1::new(malformed_dir.0.clone());
    std::fs::write(malformed.snapshot_path(), b"{malformed}").unwrap();
    assert!(malformed.recover().is_err());
}

#[test]
fn submission_publish_cutpoints_preserve_or_qualify_one_authoritative_revision() {
    let old = spool_one();
    let repository = repository_with(&["candidate-a", "candidate-b"]);
    let mut new = old.clone();
    new.enqueue(&repository, "campaign", "candidate-b").unwrap();

    let pre_dir = TestDir::new("spool-pre-rename");
    let mut pre = SubmissionStoreV1::new(pre_dir.0.clone());
    pre.persist(&old).unwrap();
    pre.inject_failure(SubmissionPublishFailureV1::PreRename);
    assert!(pre.persist(&new).is_err());
    let retry = SubmissionStoreV1::new(pre_dir.0.clone());
    assert_eq!(retry.recover().unwrap(), Some(old.clone()));
    retry.persist(&new).unwrap();
    assert_eq!(retry.recover().unwrap(), Some(new.clone()));

    let post_dir = TestDir::new("spool-post-rename");
    let mut post = SubmissionStoreV1::new(post_dir.0.clone());
    post.persist(&old).unwrap();
    post.inject_failure(SubmissionPublishFailureV1::PostRename);
    assert!(post.persist(&new).is_err());
    let qualify = SubmissionStoreV1::new(post_dir.0.clone());
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
