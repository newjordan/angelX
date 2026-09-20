use super::candidate_store::CandidateRepositoryV1;
use super::candidate_tests::digest;
use super::journal::{ActionJournalStateV1, ActionUpdateV1};
use super::schema::ActionPhaseV1;
use super::submission::{
    SubmissionDispositionOriginV1, SubmissionDispositionV1, SubmissionItemStateV1,
    SubmissionSpoolV1,
};
use super::submission_pump::SubmissionPumpWorkV1;
use super::submission_reconcile::{ReconcileActionStateV1, disposition_origin, reconcile_intent};
use super::submission_tests::{apply, first_submit, repository_with};

fn started_submit(
    ids: &[&str],
) -> (
    CandidateRepositoryV1,
    SubmissionSpoolV1,
    ActionJournalStateV1,
    ActionUpdateV1,
) {
    let repository = repository_with(ids);
    let mut spool = SubmissionSpoolV1::new();
    for id in ids {
        spool.enqueue(&repository, "campaign", id).unwrap();
    }
    let mut journal = ActionJournalStateV1::default();
    let (planned, started, _, _) = first_submit(&spool, &repository, &journal);
    apply(&mut journal, planned);
    apply(&mut journal, started.clone());
    (repository, spool, journal, started)
}

fn followup(started: &ActionUpdateV1, phase: ActionPhaseV1) -> ActionUpdateV1 {
    ActionUpdateV1 {
        intent: started.intent.clone(),
        attempt: started.attempt,
        phase,
        retryable: false,
        at_ms: started.at_ms + 1,
        reconcile_key: None,
        receipt_sha256: (phase == ActionPhaseV1::Completed).then(|| digest("effect-receipt")),
        next: None,
    }
}

fn acknowledge() -> SubmissionDispositionV1 {
    SubmissionDispositionV1::Acknowledged {
        platform_submission_id: "platform-origin".into(),
        receipt_sha256: digest("ack-origin"),
    }
}

#[test]
fn reconcile_origin_after_completed_submit_is_explicitly_authorized() {
    let (repository, mut spool, mut journal, submit_started) = started_submit(&["candidate-a"]);
    apply(
        &mut journal,
        followup(&submit_started, ActionPhaseV1::Completed),
    );
    let (planned, started) = match spool.decide(&repository, &journal, 10).unwrap().work {
        Some(SubmissionPumpWorkV1::Reconcile {
            action: ReconcileActionStateV1::Plan { planned, started },
            ..
        }) => (*planned, *started),
        _ => panic!("completed submit must reconcile"),
    };
    apply(&mut journal, planned);
    let origin = disposition_origin(&started).unwrap();
    apply(&mut journal, started);
    spool
        .record_disposition(&journal, 0, origin.clone(), acknowledge())
        .unwrap();
    assert_eq!(
        spool.items.get(&0).unwrap().disposition_origin,
        Some(origin)
    );
}

#[test]
fn started_submit_cannot_claim_missing_reconcile_origin() {
    let (_, mut spool, journal, _) = started_submit(&["candidate-a"]);
    let item = spool.items.get(&0).unwrap();
    let origin = SubmissionDispositionOriginV1::Reconcile {
        action_key: reconcile_intent(item).unwrap().action_key,
    };
    let before = spool.clone();
    assert!(
        spool
            .record_disposition(&journal, 0, origin, acknowledge())
            .is_err()
    );
    assert_eq!(spool, before);
}

#[test]
fn delayed_original_submit_response_remains_valid_during_reconcile() {
    let (_, mut spool, journal, submit_started) = started_submit(&["candidate-a"]);
    let origin = disposition_origin(&submit_started).unwrap();
    let item = spool.items.get(&0).unwrap().clone();
    spool
        .record_disposition(
            &journal,
            0,
            origin.clone(),
            SubmissionDispositionV1::Ambiguous {
                reconcile_key: item.reconcile_key,
                request_sha256: item.request_sha256,
                next_attempt_at_ms: 10,
            },
        )
        .unwrap();
    spool
        .record_disposition(&journal, 0, origin.clone(), acknowledge())
        .unwrap();
    assert!(matches!(
        spool.items.get(&0).unwrap().state,
        SubmissionItemStateV1::Acknowledged { .. }
    ));
    assert_eq!(
        spool.items.get(&0).unwrap().disposition_origin,
        Some(origin)
    );
}

#[test]
fn failed_reconcile_is_not_replanned_and_does_not_block_later_work() {
    let (repository, mut spool, mut journal, submit_started) =
        started_submit(&["candidate-a", "candidate-b"]);
    let origin = disposition_origin(&submit_started).unwrap();
    let item = spool.items.get(&0).unwrap().clone();
    spool
        .record_disposition(
            &journal,
            0,
            origin,
            SubmissionDispositionV1::Ambiguous {
                reconcile_key: item.reconcile_key,
                request_sha256: item.request_sha256,
                next_attempt_at_ms: 0,
            },
        )
        .unwrap();
    let (planned, started) = match spool.decide(&repository, &journal, 10).unwrap().work {
        Some(SubmissionPumpWorkV1::Reconcile {
            action: ReconcileActionStateV1::Plan { planned, started },
            ..
        }) => (*planned, *started),
        _ => unreachable!(),
    };
    apply(&mut journal, planned);
    apply(&mut journal, started.clone());
    apply(&mut journal, followup(&started, ActionPhaseV1::Failed));
    match spool.decide(&repository, &journal, 11).unwrap().work {
        Some(SubmissionPumpWorkV1::Submit { submission_id, .. }) => {
            assert_eq!(submission_id, spool.items.get(&1).unwrap().submission_id)
        }
        _ => panic!("failed reconcile must skip to later eligible work"),
    }
}
