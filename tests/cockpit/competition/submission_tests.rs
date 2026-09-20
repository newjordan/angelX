use super::candidate::originate_candidate;
use super::candidate_store::CandidateRepositoryV1;
use super::candidate_tests::{artifact, board, digest, evidence};
use super::journal::{ActionJournalStateV1, ActionUpdateV1, PrepareActionV1};
use super::leases::WorkLeaseKeyV1;
use super::schema::{LaneIdV1, ScoreV1};
use super::store::CompetitionStore;
use super::submission::*;
use super::submission_pump::SubmissionPumpWorkV1;
use super::submission_reconcile::disposition_origin;
use super::submission_results::OfficialResultV1;
use super::submission_store::SubmissionStoreV1;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct TestDir(pub(super) PathBuf);

impl TestDir {
    pub(super) fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-submission-store-{tag}-{}-{}",
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

pub(super) fn repository_with(ids: &[&str]) -> CandidateRepositoryV1 {
    let current = board(1, "100", "source-a", "60");
    let mut repository =
        CandidateRepositoryV1::new(current.board_epoch, current.decision_sha256.clone()).unwrap();
    for id in ids {
        let candidate = originate_candidate(
            &repository.catalog,
            id,
            &format!("hypothesis-{id}"),
            &format!("episode-{id}"),
            &current,
            artifact(id),
            evidence(id),
        )
        .unwrap();
        repository.insert(candidate).unwrap();
    }
    repository
}

pub(super) fn spool_one() -> SubmissionSpoolV1 {
    let repository = repository_with(&["candidate-a"]);
    let mut spool = SubmissionSpoolV1::new();
    spool
        .enqueue(&repository, "campaign", "candidate-a")
        .unwrap();
    spool
}

pub(super) fn apply(journal: &mut ActionJournalStateV1, update: ActionUpdateV1) {
    match journal.prepare(update).unwrap() {
        PrepareActionV1::Append(event) => journal.apply(&event).unwrap(),
        PrepareActionV1::Replay { .. } => {}
    }
}

pub(super) fn first_submit(
    spool: &SubmissionSpoolV1,
    repository: &CandidateRepositoryV1,
    journal: &ActionJournalStateV1,
) -> (ActionUpdateV1, ActionUpdateV1, String, String) {
    match spool.decide(repository, journal, 10).unwrap().work.unwrap() {
        SubmissionPumpWorkV1::Submit {
            planned,
            started,
            idempotency_key,
            reconcile_key,
            ..
        } => (*planned, *started, idempotency_key, reconcile_key),
        _ => panic!("expected submit work"),
    }
}

/// FG-SUB-030-failed-or-stale-head-does-not-block-later-eligible-item
#[test]
fn fg_sub_030_failed_or_stale_head_does_not_block_later_eligible_item() {
    let repository = repository_with(&[
        "candidate-failed",
        "candidate-stale",
        "candidate-waiting",
        "candidate-runnable",
    ]);
    let mut spool = SubmissionSpoolV1::new();
    for id in repository.catalog.keys() {
        spool.enqueue(&repository, "campaign", id).unwrap();
    }
    let mut journal = ActionJournalStateV1::default();
    spool
        .mark_failed(&journal, 0, "definitely-rejected")
        .unwrap();
    spool.mark_stale(&journal, 1, "board-moved").unwrap();
    let (planned, started, _, _) = first_submit(&spool, &repository, &journal);
    let origin = disposition_origin(&started).unwrap();
    apply(&mut journal, planned);
    apply(&mut journal, started);
    let waiting = spool.items.get(&2).unwrap().clone();
    spool
        .record_disposition(
            &journal,
            2,
            origin,
            SubmissionDispositionV1::Ambiguous {
                reconcile_key: waiting.reconcile_key,
                request_sha256: waiting.request_sha256,
                next_attempt_at_ms: 1_000,
            },
        )
        .unwrap();
    let decision = spool.decide(&repository, &journal, 10).unwrap();
    match decision.work.unwrap() {
        SubmissionPumpWorkV1::Submit { submission_id, .. } => {
            assert_eq!(submission_id, spool.items.get(&3).unwrap().submission_id)
        }
        _ => panic!("later eligible item must remain runnable"),
    }
    assert!(decision.next.action.starts_with("submit_candidate:"));
}

/// FG-SUB-031-accept-then-timeout-reconciles-without-duplicate-submit
#[test]
fn fg_sub_031_accept_then_timeout_reconciles_without_duplicate_submit() {
    let mut spool = spool_one();
    let repository = repository_with(&["candidate-a"]);
    let mut journal = ActionJournalStateV1::default();
    let unopened = spool.clone();
    let item = spool.items.get(&0).unwrap();
    assert!(
        spool
            .record_disposition(
                &journal,
                0,
                SubmissionDispositionOriginV1::Submit {
                    action_key: item.action_key.clone(),
                },
                SubmissionDispositionV1::DefinitelyRejected {
                    reason_code: "unattested".into(),
                },
            )
            .is_err()
    );
    assert_eq!(spool, unopened);
    let (planned, started, idempotency, reconcile) = first_submit(&spool, &repository, &journal);
    let origin = disposition_origin(&started).unwrap();
    assert_eq!(planned.intent.action_key, started.intent.action_key);
    assert_ne!(idempotency, reconcile);
    apply(&mut journal, planned);
    apply(&mut journal, started);
    let item = spool.items.get(&0).unwrap().clone();
    spool
        .record_disposition(
            &journal,
            0,
            origin,
            SubmissionDispositionV1::Ambiguous {
                reconcile_key: item.reconcile_key,
                request_sha256: item.request_sha256,
                next_attempt_at_ms: 10,
            },
        )
        .unwrap();
    assert!(matches!(
        spool.decide(&repository, &journal, 10).unwrap().work,
        Some(SubmissionPumpWorkV1::Reconcile { .. })
    ));
}

/// FG-SUB-032-restart-from-started-reconciles-same-action-key
#[test]
fn fg_sub_032_restart_from_started_reconciles_same_action_key() {
    let dir = TestDir::new("started-restart");
    let spool_store = SubmissionStoreV1::new(dir.0.join("spool"));
    let spool = spool_one();
    let repository = repository_with(&["candidate-a"]);
    spool_store.persist(&spool).unwrap();
    let action_store = CompetitionStore::new(dir.0.join("actions"));
    let item = spool.items.get(&0).unwrap();
    let key = WorkLeaseKeyV1 {
        campaign_id: item.campaign_id.clone(),
        lane_id: LaneIdV1::FrontierGuard,
        work_item_id: item.submission_id.clone(),
    };
    let lease = action_store
        .update_leases(|leases| leases.grant(key.clone(), "worker".into(), 0, 10_000, 0))
        .unwrap();
    let (planned, started, _, _) =
        first_submit(&spool, &repository, &ActionJournalStateV1::default());
    action_store
        .append_action_fenced(&key, lease.fencing_generation, planned)
        .unwrap();
    action_store
        .append_action_fenced(&key, lease.fencing_generation, started)
        .unwrap();
    let recovered_spool = spool_store.recover().unwrap().unwrap();
    let recovered_actions = action_store.recover().unwrap().state;
    match recovered_spool
        .decide(&repository, &recovered_actions, 20)
        .unwrap()
        .work
    {
        Some(SubmissionPumpWorkV1::Reconcile {
            submission_id,
            reconcile_key,
            ..
        }) => {
            assert_eq!(submission_id, item.submission_id);
            assert_eq!(reconcile_key, item.reconcile_key);
        }
        _ => panic!("durable Started must reconcile before resend"),
    }
}

/// FG-SUB-033-duplicate-and-corrected-results-bind-once-with-revision-chain
#[test]
fn fg_sub_033_duplicate_and_corrected_results_bind_once_with_revision_chain() {
    let mut spool = spool_one();
    let repository = repository_with(&["candidate-a"]);
    let mut journal = ActionJournalStateV1::default();
    let (planned, started, _, _) = first_submit(&spool, &repository, &journal);
    let origin = disposition_origin(&started).unwrap();
    apply(&mut journal, planned);
    apply(&mut journal, started);
    let acknowledged = SubmissionDispositionV1::Acknowledged {
        platform_submission_id: "platform-7".into(),
        receipt_sha256: digest("ack"),
    };
    spool
        .record_disposition(&journal, 0, origin.clone(), acknowledged.clone())
        .unwrap();
    let acknowledged_revision = spool.revision;
    spool
        .record_disposition(&journal, 0, origin, acknowledged)
        .unwrap();
    assert_eq!(spool.revision, acknowledged_revision);
    let item = spool.items.get(&0).unwrap().clone();
    let first = OfficialResultV1::new(
        &item,
        "result-1",
        1,
        ScoreV1::new("101").unwrap(),
        digest("result-1"),
        None,
    )
    .unwrap();
    assert!(spool.bind_official(first.clone()).unwrap());
    assert!(!spool.bind_official(first).unwrap());
    let reused_receipt = OfficialResultV1::new(
        &item,
        "result-forged",
        2,
        ScoreV1::new("102").unwrap(),
        digest("result-1"),
        Some("result-1".into()),
    )
    .unwrap();
    assert!(spool.bind_official(reused_receipt).is_err());
    let correction = OfficialResultV1::new(
        &item,
        "result-2",
        2,
        ScoreV1::new("102").unwrap(),
        digest("result-2"),
        Some("result-1".into()),
    )
    .unwrap();
    assert!(spool.bind_official(correction.clone()).unwrap());
    assert!(!spool.bind_official(correction).unwrap());
    let stale_parent = OfficialResultV1::new(
        &item,
        "result-3",
        3,
        ScoreV1::new("103").unwrap(),
        digest("result-3"),
        Some("result-1".into()),
    )
    .unwrap();
    assert!(spool.bind_official(stale_parent).is_err());
    assert_eq!(spool.official.results.len(), 2);
    assert_eq!(
        spool.official.latest_by_submission.get(&item.submission_id),
        Some(&"result-2".into())
    );
}
