use super::candidate_store::CandidateRepositoryV1;
use super::candidate_tests::{board, digest};
use super::journal::{ActionJournalStateV1, ActionUpdateV1};
use super::schema::{ActionPhaseV1, ScheduledActionV1, ScoreV1};
use super::submission::{
    SubmissionDispositionOriginV1, SubmissionDispositionV1, SubmissionSpoolV1,
};
use super::submission_pump::SubmissionPumpWorkV1;
use super::submission_reconcile::{ReconcileActionStateV1, disposition_origin, reconcile_intent};
use super::submission_results::OfficialResultV1;
use super::submission_tests::{apply, first_submit, repository_with};

fn needs_reconcile(
    candidate: &str,
) -> (
    CandidateRepositoryV1,
    SubmissionSpoolV1,
    ActionJournalStateV1,
) {
    let repository = repository_with(&[candidate]);
    let mut spool = SubmissionSpoolV1::new();
    spool.enqueue(&repository, "campaign", candidate).unwrap();
    let mut journal = ActionJournalStateV1::default();
    let (planned, started, _, _) = first_submit(&spool, &repository, &journal);
    let origin = disposition_origin(&started).unwrap();
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
                next_attempt_at_ms: 0,
            },
        )
        .unwrap();
    (repository, spool, journal)
}

fn reconcile_state(
    repository: &CandidateRepositoryV1,
    spool: &SubmissionSpoolV1,
    journal: &ActionJournalStateV1,
) -> ReconcileActionStateV1 {
    let recovered_spool: SubmissionSpoolV1 =
        serde_json::from_slice(&serde_json::to_vec(spool).unwrap()).unwrap();
    let recovered_journal: ActionJournalStateV1 =
        serde_json::from_slice(&serde_json::to_vec(journal).unwrap()).unwrap();
    match recovered_spool
        .decide(repository, &recovered_journal, 10)
        .unwrap()
        .work
        .unwrap()
    {
        SubmissionPumpWorkV1::Reconcile { action, .. } => action,
        _ => panic!("reconciliation must never become a submit resend"),
    }
}

fn followup(started: &ActionUpdateV1, phase: ActionPhaseV1, key: &str) -> ActionUpdateV1 {
    ActionUpdateV1 {
        intent: started.intent.clone(),
        attempt: started.attempt,
        phase,
        retryable: false,
        at_ms: started.at_ms + 1,
        reconcile_key: (phase == ActionPhaseV1::Ambiguous).then(|| key.into()),
        receipt_sha256: (phase == ActionPhaseV1::Completed).then(|| digest("reconcile-receipt")),
        next: (phase == ActionPhaseV1::Ambiguous).then(|| ScheduledActionV1 {
            action: "reconcile_submission".into(),
            next_attempt_at_ms: started.at_ms + 2,
        }),
    }
}

#[test]
fn reconcile_cutpoint_matrix_restarts_without_replanning_or_resending() {
    let (repository, spool, mut journal) = needs_reconcile("candidate-a");
    let (planned, started) = match reconcile_state(&repository, &spool, &journal) {
        ReconcileActionStateV1::Plan { planned, started } => (*planned, *started),
        _ => panic!("missing reconcile action must plan once"),
    };
    apply(&mut journal, planned);
    let restarted = match reconcile_state(&repository, &spool, &journal) {
        ReconcileActionStateV1::Start { started } => *started,
        _ => panic!("durable Planned must advance to Started"),
    };
    assert_eq!(restarted.intent.action_key, started.intent.action_key);
    apply(&mut journal, restarted.clone());
    let reconcile_origin = disposition_origin(&restarted).unwrap();
    assert_eq!(
        reconcile_state(&repository, &spool, &journal),
        ReconcileActionStateV1::Resume {
            phase: ActionPhaseV1::Started
        }
    );
    let mut resolved = spool.clone();
    resolved
        .record_disposition(
            &journal,
            0,
            reconcile_origin,
            SubmissionDispositionV1::Acknowledged {
                platform_submission_id: "platform-a".into(),
                receipt_sha256: digest("ack-a"),
            },
        )
        .unwrap();
    let key = spool.items.get(&0).unwrap().reconcile_key.clone();
    apply(
        &mut journal,
        followup(&restarted, ActionPhaseV1::Ambiguous, &key),
    );
    assert_eq!(
        reconcile_state(&repository, &spool, &journal),
        ReconcileActionStateV1::Resume {
            phase: ActionPhaseV1::Ambiguous
        }
    );
    apply(
        &mut journal,
        followup(&restarted, ActionPhaseV1::Completed, &key),
    );
    assert_eq!(
        reconcile_state(&repository, &spool, &journal),
        ReconcileActionStateV1::Resume {
            phase: ActionPhaseV1::Completed
        }
    );
}

#[test]
fn submit_and_reconcile_identity_guards_reject_bypass_and_cross_item_evidence() {
    let (repository_a, mut spool_a, mut journal_a) = needs_reconcile("candidate-a");
    let clean_journal_a = journal_a.clone();
    let before = spool_a.clone();
    let missing_origin = SubmissionDispositionOriginV1::Reconcile {
        action_key: reconcile_intent(spool_a.items.get(&0).unwrap())
            .unwrap()
            .action_key,
    };
    assert!(
        spool_a
            .record_disposition(
                &journal_a,
                0,
                missing_origin,
                SubmissionDispositionV1::Acknowledged {
                    platform_submission_id: "platform-a".into(),
                    receipt_sha256: digest("ack-a"),
                },
            )
            .is_err()
    );
    assert_eq!(spool_a, before);
    let repository_queued = repository_with(&["candidate-queued"]);
    let mut queued = SubmissionSpoolV1::new();
    queued
        .enqueue(&repository_queued, "campaign", "candidate-queued")
        .unwrap();
    let foreign_submit = journal_a.actions.values().next().unwrap().clone();
    let mut conflicting_submit = ActionJournalStateV1::default();
    conflicting_submit.actions.insert(
        queued.items.get(&0).unwrap().action_key.clone(),
        foreign_submit,
    );
    assert!(
        queued
            .decide(&repository_queued, &conflicting_submit, 0)
            .is_err()
    );
    assert!(queued.mark_stale(&conflicting_submit, 0, "bypass").is_err());
    let mut queued_journal = ActionJournalStateV1::default();
    let (planned, started, _, _) = first_submit(&queued, &repository_queued, &queued_journal);
    apply(&mut queued_journal, planned);
    apply(&mut queued_journal, started);
    assert!(queued.mark_failed(&queued_journal, 0, "bypass").is_err());
    assert!(queued.mark_stale(&queued_journal, 0, "bypass").is_err());

    let (repository_b, spool_b, mut journal_b) = needs_reconcile("candidate-b");
    let action_b = match reconcile_state(&repository_b, &spool_b, &journal_b) {
        ReconcileActionStateV1::Plan { planned, started } => {
            apply(&mut journal_b, *planned);
            *started
        }
        _ => unreachable!(),
    };
    let origin_b = disposition_origin(&action_b).unwrap();
    apply(&mut journal_b, action_b);
    let item_a = spool_a.items.get(&0).unwrap().clone();
    let item_b = spool_b.items.get(&0).unwrap();
    let record_b = journal_b
        .actions
        .get(&reconcile_intent(item_b).unwrap().action_key)
        .unwrap()
        .clone();
    journal_a
        .actions
        .insert(reconcile_intent(&item_a).unwrap().action_key, record_b);
    assert!(
        spool_a
            .record_disposition(
                &journal_a,
                0,
                SubmissionDispositionOriginV1::Reconcile {
                    action_key: reconcile_intent(&item_a).unwrap().action_key,
                },
                SubmissionDispositionV1::Acknowledged {
                    platform_submission_id: "platform-a".into(),
                    receipt_sha256: digest("ack-a"),
                },
            )
            .is_err()
    );

    let mut acknowledged_b = spool_b.clone();
    acknowledged_b
        .record_disposition(
            &journal_b,
            0,
            origin_b,
            SubmissionDispositionV1::Acknowledged {
                platform_submission_id: "platform-b".into(),
                receipt_sha256: digest("ack-b"),
            },
        )
        .unwrap();
    let result_b = OfficialResultV1::new(
        acknowledged_b.items.get(&0).unwrap(),
        "result-b",
        1,
        ScoreV1::new("1").unwrap(),
        digest("result-b"),
        None,
    )
    .unwrap();
    let mut action_a = clean_journal_a;
    let (planned_a, started_a) = match reconcile_state(&repository_a, &before, &action_a) {
        ReconcileActionStateV1::Plan { planned, started } => (*planned, *started),
        _ => unreachable!(),
    };
    apply(&mut action_a, planned_a);
    let origin_a = disposition_origin(&started_a).unwrap();
    apply(&mut action_a, started_a);
    let mut acknowledged_a = before;
    acknowledged_a
        .record_disposition(
            &action_a,
            0,
            origin_a,
            SubmissionDispositionV1::Acknowledged {
                platform_submission_id: "platform-a".into(),
                receipt_sha256: digest("ack-a"),
            },
        )
        .unwrap();
    assert!(acknowledged_a.bind_official(result_b).is_err());
    assert!(repository_a.require_eligible("candidate-a").is_ok());
}

#[test]
fn queued_candidate_from_old_board_cannot_run_after_eligibility_moves() {
    let mut repository = repository_with(&["candidate-a"]);
    let spool = super::submission_tests::spool_one();
    let moved = board(2, "110", "source-b", "70");
    repository
        .set_current_board(moved.board_epoch, moved.decision_sha256)
        .unwrap();
    let decision = spool
        .decide(&repository, &ActionJournalStateV1::default(), 10)
        .unwrap();
    assert!(decision.work.is_none());
    assert_eq!(decision.next.action, "scan_submission_spool");
}
