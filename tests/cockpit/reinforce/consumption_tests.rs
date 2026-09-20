use super::consumption::{
    ConsumptionOutcome, EvaluatorArtifactScoreRequest, ExpectedReceiptContext, ReceiptCountContext,
    ReceiptPairConsumptionRequest, ReceiptPairCountContext, ReceiptPairOutcome,
    consume_evaluator_artifact, consume_evaluator_artifact_pair,
    replay_evaluator_artifact_in_context,
};
use super::evaluator::PolicyEvaluationRequest;
use super::{Candidate, EvaluatorEvidence, TEST_VERIFIER_CONTRACT, TestReward};
use std::path::Path;
use std::time::Duration;

fn expected(evidence: &EvaluatorEvidence) -> ExpectedReceiptContext<'_> {
    ExpectedReceiptContext {
        subject_sha256: evidence.subject_sha256(),
        command_sha256: evidence.command_sha256(),
        verifier_contract_sha256: evidence.verifier_contract_sha256(),
        execution_policy_sha256: evidence.execution_policy_sha256(),
    }
}

fn count<'a>(
    run_id: &'a str,
    reproduction_id: &'a str,
    attempt_id: &'a str,
    retry_index: u32,
    prior_attempt_id: Option<&'a str>,
) -> ReceiptCountContext<'a> {
    ReceiptCountContext {
        run_id,
        reproduction_id,
        attempt_id,
        retry_index,
        prior_attempt_id,
    }
}

fn pair_count(campaign: &str, sample_index: usize) -> ReceiptPairCountContext<'_> {
    ReceiptPairCountContext {
        campaign_id: campaign,
        reproduction_id: "reproduction-1",
        phase: "selection",
        cohort_manifest_sha256: "manifest-sha256",
        case_id: "case-1",
        task_sha256: "task-sha256",
        prompt_sha256: "prompt-sha256",
        inventory_reward_contract_sha256: "inventory-reward-contract-sha256",
        outcome_reward_contract_sha256: "outcome-reward-contract-sha256",
        policy_version: 7,
        sample_index,
    }
}

#[test]
fn campaign_execution_lock_serializes_concurrent_resumers() {
    let root = std::env::temp_dir().join(format!(
        "angel-campaign-execution-lock-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let first = super::consumption::acquire_campaign_execution_lock(&root).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_root = root.clone();
    let worker = std::thread::spawn(move || {
        tx.send("waiting").unwrap();
        let _second = super::consumption::acquire_campaign_execution_lock(&worker_root).unwrap();
        tx.send("acquired").unwrap();
    });
    assert_eq!(rx.recv().unwrap(), "waiting");
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    drop(first);
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), "acquired");
    worker.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

fn consume_pair(
    inventory_path: &Path,
    inventory: &EvaluatorEvidence,
    outcome_path: &Path,
    outcome: &EvaluatorEvidence,
    count: &ReceiptPairCountContext<'_>,
    ledger: &Path,
) -> Result<ReceiptPairOutcome, String> {
    let inventory_expected = expected(inventory);
    let outcome_expected = expected(outcome);
    consume_evaluator_artifact_pair(ReceiptPairConsumptionRequest {
        inventory: EvaluatorArtifactScoreRequest {
            path: inventory_path,
            reward: &TestReward,
            expected: &inventory_expected,
        },
        outcome: EvaluatorArtifactScoreRequest {
            path: outcome_path,
            reward: &TestReward,
            expected: &outcome_expected,
        },
        count,
        ledger_root: ledger,
    })
}

#[test]
fn evaluator_pair_id_is_framework_derived_and_field_sensitive() {
    let slot = pair_count("campaign-1", 0);
    assert_eq!(
        super::consumption::canonical_receipt_pair_id(&slot),
        "b018852209c8aa872497cf160942468e8321bed80e9f4fb5a1eea712170b2ebe"
    );
    let changed_sample = pair_count("campaign-1", 1);
    assert_ne!(
        super::consumption::canonical_receipt_pair_id(&slot),
        super::consumption::canonical_receipt_pair_id(&changed_sample)
    );
    let mut changed_task = pair_count("campaign-1", 0);
    changed_task.task_sha256 = "different-task-sha256";
    assert_ne!(
        super::consumption::canonical_receipt_pair_id(&slot),
        super::consumption::canonical_receipt_pair_id(&changed_task)
    );

    let candidate_a = Candidate {
        policy_version: 7,
        latency: Duration::ZERO,
        output: "candidate-a".into(),
    };
    let candidate_b = Candidate {
        output: "candidate-b".into(),
        ..candidate_a.clone()
    };
    let request = |candidate| PolicyEvaluationRequest {
        cohort_manifest_sha256: "manifest-sha256",
        cohort_role: "selection",
        case_id: "case-1",
        task: "task",
        prompt_sha256: "prompt-sha256",
        policy_version: 7,
        sample_index: 0,
        candidate,
    };
    assert_eq!(
        request(&candidate_a).canonical_pair_id(),
        request(&candidate_b).canonical_pair_id(),
        "candidate output must not create a new logical evaluation slot"
    );
    assert_ne!(
        request(&candidate_a).canonical_subject(),
        request(&candidate_b).canonical_subject(),
        "the frozen evidence identity must still bind candidate output"
    );
}

#[test]
fn campaign_request_and_generated_plans_are_first_write_wins() {
    let root = std::env::temp_dir().join(format!("angel-campaign-freeze-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    super::consumption::bind_campaign_authority(&root, b"campaign-a").unwrap();
    super::consumption::bind_campaign_authority(&root, b"campaign-a").unwrap();
    assert!(
        super::consumption::bind_campaign_authority(&root, b"campaign-b")
            .unwrap_err()
            .contains("record drift")
    );
    let plan_a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let plan_b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    super::consumption::freeze_evaluation_plan(&root, plan_a, b"candidate-plan-a").unwrap();
    super::consumption::freeze_evaluation_plan(&root, plan_a, b"candidate-plan-a").unwrap();
    assert!(
        super::consumption::freeze_evaluation_plan(&root, plan_a, b"candidate-plan-b")
            .unwrap_err()
            .contains("record drift")
    );
    super::consumption::commit_terminal_release(&root, "genesis", b"terminal\n").unwrap();
    super::consumption::freeze_evaluation_plan(&root, plan_a, b"candidate-plan-a").unwrap();
    assert!(
        super::consumption::freeze_evaluation_plan(&root, plan_b, b"candidate-plan-b")
            .unwrap_err()
            .contains("sealed by a terminal release")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evaluator_consumption_ledger_separates_replay_resume_retry_and_reproduction() {
    // These fixtures spawn raw `git` (not the env-stripping pinned command),
    // so they hold the crate env lock: a concurrent test that sets GIT_DIR
    // (the workspace-evidence decoy) would otherwise redirect this repo's
    // git into a non-repository and fail these assertions at random.
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-evaluator-consumption-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("workspace");
    let store = root.join("artifacts");
    let ledger = root.join("ledger");
    std::fs::create_dir_all(&workspace).unwrap();
    let git = crate::agent::harness::pinned_git_path().unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["init", "-q"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(workspace.join("fixture.txt"), "frozen").unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["add", "fixture.txt"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );

    let make_evidence = || {
        EvaluatorEvidence::run_shell(
            "consumption-control",
            "printf '%s' 'test result: ok. 2 passed; 0 failed;'",
            &workspace,
            TEST_VERIFIER_CONTRACT,
            "cohort-role-case-arm-sample",
        )
        .unwrap()
    };
    let first = make_evidence();
    let first_path = first.persist_append_only(&store).unwrap();
    assert_eq!(
        replay_evaluator_artifact_in_context(&first_path, &TestReward, &expected(&first)).unwrap(),
        ConsumptionOutcome::ScoreOnly { reward: 1.0 }
    );
    assert!(
        !ledger.exists(),
        "score replay must not create counting state"
    );

    let initial = count("run-1", "reproduction-1", "attempt-0", 0, None);
    assert_eq!(
        consume_evaluator_artifact(
            &first_path,
            &TestReward,
            &expected(&first),
            &initial,
            &ledger,
        )
        .unwrap(),
        ConsumptionOutcome::Counted { reward: 1.0 }
    );
    assert_eq!(
        consume_evaluator_artifact(
            &first_path,
            &TestReward,
            &expected(&first),
            &initial,
            &ledger,
        )
        .unwrap(),
        ConsumptionOutcome::IdempotentResume { reward: 1.0 }
    );
    assert!(
        consume_evaluator_artifact(
            &first_path,
            &TestReward,
            &expected(&first),
            &count("run-1", "reproduction-1", "attempt-copy", 0, None),
            &ledger,
        )
        .unwrap_err()
        .contains("already counted")
    );

    let second = make_evidence();
    let second_path = second.persist_append_only(&store).unwrap();
    assert_eq!(
        consume_evaluator_artifact(
            &second_path,
            &TestReward,
            &expected(&second),
            &count("run-1", "reproduction-1", "attempt-1", 1, Some("attempt-0"),),
            &ledger,
        )
        .unwrap(),
        ConsumptionOutcome::Counted { reward: 1.0 }
    );

    let third = make_evidence();
    let third_path = third.persist_append_only(&store).unwrap();
    assert_eq!(
        consume_evaluator_artifact(
            &third_path,
            &TestReward,
            &expected(&third),
            &count("run-1", "reproduction-2", "attempt-0", 0, None),
            &ledger,
        )
        .unwrap(),
        ConsumptionOutcome::Counted { reward: 1.0 }
    );

    let wrong = ExpectedReceiptContext {
        subject_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        ..expected(&third)
    };
    assert!(
        replay_evaluator_artifact_in_context(&third_path, &TestReward, &wrong)
            .unwrap_err()
            .contains("subject context mismatch")
    );

    let malformed_root = root.join("malformed-ledger");
    std::fs::create_dir_all(&malformed_root).unwrap();
    std::fs::write(
        malformed_root.join("receipt-consumption.jsonl"),
        "not-json\n",
    )
    .unwrap();
    let fourth = make_evidence();
    let fourth_path = fourth.persist_append_only(&store).unwrap();
    assert!(
        consume_evaluator_artifact(
            &fourth_path,
            &TestReward,
            &expected(&fourth),
            &count("run-2", "reproduction-1", "attempt-0", 0, None),
            &malformed_root,
        )
        .unwrap_err()
        .contains("invalid receipt ledger row")
    );
    #[cfg(unix)]
    {
        let linked_root = root.join("linked-ledger");
        std::os::unix::fs::symlink(&ledger, &linked_root).unwrap();
        assert!(
            consume_evaluator_artifact(
                &fourth_path,
                &TestReward,
                &expected(&fourth),
                &count("run-3", "reproduction-1", "attempt-0", 0, None),
                &linked_root,
            )
            .unwrap_err()
            .contains("non-symlink directory")
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evaluator_consumption_ledger_serializes_concurrent_resume() {
    // These fixtures spawn raw `git` (not the env-stripping pinned command),
    // so they hold the crate env lock: a concurrent test that sets GIT_DIR
    // (the workspace-evidence decoy) would otherwise redirect this repo's
    // git into a non-repository and fail these assertions at random.
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-evaluator-consumption-concurrent-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("workspace");
    let store = root.join("artifacts");
    let ledger = root.join("ledger");
    std::fs::create_dir_all(&workspace).unwrap();
    let git = crate::agent::harness::pinned_git_path().unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["init", "-q"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(workspace.join("fixture.txt"), "frozen").unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["add", "fixture.txt"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );
    let evidence = EvaluatorEvidence::run_shell(
        "concurrent-consumption",
        "printf '%s' 'test result: ok. 1 passed; 0 failed;'",
        Path::new(&workspace),
        TEST_VERIFIER_CONTRACT,
        "same-attempt",
    )
    .unwrap();
    let path = evidence.persist_append_only(&store).unwrap();
    let workers = (0..8)
        .map(|_| {
            let path = path.clone();
            let ledger = ledger.clone();
            let evidence = evidence.clone();
            std::thread::spawn(move || {
                consume_evaluator_artifact(
                    &path,
                    &TestReward,
                    &expected(&evidence),
                    &count("run", "reproduction", "attempt", 0, None),
                    &ledger,
                )
                .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let outcomes = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ConsumptionOutcome::Counted { .. }))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ConsumptionOutcome::IdempotentResume { .. }))
            .count(),
        7
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evaluator_pair_ledger_is_atomic_resumable_and_recovers_a_torn_tail() {
    // These fixtures spawn raw `git` (not the env-stripping pinned command),
    // so they hold the crate env lock: a concurrent test that sets GIT_DIR
    // (the workspace-evidence decoy) would otherwise redirect this repo's
    // git into a non-repository and fail these assertions at random.
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-evaluator-pair-consumption-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("workspace");
    let store = root.join("artifacts");
    let ledger = root.join("ledger");
    std::fs::create_dir_all(&workspace).unwrap();
    let git = crate::agent::harness::pinned_git_path().unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["init", "-q"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(workspace.join("fixture.txt"), "frozen").unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["add", "fixture.txt"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );

    let make_evidence = |label: &str, subject: &str| {
        EvaluatorEvidence::run_shell(
            label,
            "printf '%s' 'test result: ok. 2 passed; 0 failed;'",
            &workspace,
            TEST_VERIFIER_CONTRACT,
            subject,
        )
        .unwrap()
    };
    let inventory = make_evidence("pair-inventory", "inventory-subject");
    let outcome = make_evidence("pair-outcome", "outcome-subject");
    let inventory_path = inventory.persist_append_only(&store).unwrap();
    let outcome_path = outcome.persist_append_only(&store).unwrap();
    let first = consume_pair(
        &inventory_path,
        &inventory,
        &outcome_path,
        &outcome,
        &pair_count("campaign-1", 0),
        &ledger,
    )
    .unwrap();
    assert!(first.counted);
    assert_eq!(first.inventory_reward, 1.0);
    assert_eq!(first.outcome_reward, 1.0);
    assert_eq!(
        std::fs::read_to_string(ledger.join("receipt-pair-consumption.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );

    // A fresh execution of exactly the frozen logical sample resumes the
    // original pair instead of increasing the cohort N.
    let resumed_inventory = make_evidence("pair-inventory", "inventory-subject");
    let resumed_outcome = make_evidence("pair-outcome", "outcome-subject");
    let resumed = consume_pair(
        &resumed_inventory.persist_append_only(&store).unwrap(),
        &resumed_inventory,
        &resumed_outcome.persist_append_only(&store).unwrap(),
        &resumed_outcome,
        &pair_count("campaign-1", 0),
        &ledger,
    )
    .unwrap();
    assert!(!resumed.counted);
    assert_eq!(resumed.receipt_sha256s, first.receipt_sha256s);
    let mut scorer_drift = pair_count("campaign-1", 0);
    scorer_drift.outcome_reward_contract_sha256 = "outcome-reward-contract-v2-sha256";
    assert!(
        consume_pair(
            &inventory_path,
            &inventory,
            &outcome_path,
            &outcome,
            &scorer_drift,
            &ledger,
        )
        .unwrap_err()
        .contains("slot is already frozen")
    );

    // A changed stochastic output cannot occupy the already frozen slot.
    let changed = make_evidence("pair-outcome", "changed-outcome-subject");
    assert!(
        consume_pair(
            &inventory_path,
            &inventory,
            &changed.persist_append_only(&store).unwrap(),
            &changed,
            &pair_count("campaign-1", 0),
            &ledger,
        )
        .unwrap_err()
        .contains("slot is already frozen")
    );
    assert!(
        consume_pair(
            &inventory_path,
            &inventory,
            &inventory_path,
            &inventory,
            &pair_count("campaign-1", 1),
            &ledger,
        )
        .unwrap_err()
        .contains("roles must be distinct")
    );
    assert!(
        consume_evaluator_artifact(
            &inventory_path,
            &TestReward,
            &expected(&inventory),
            &count("legacy-run", "reproduction", "attempt", 0, None),
            &ledger,
        )
        .unwrap_err()
        .contains("already counted in the pair ledger")
    );

    let legacy_first_ledger = root.join("legacy-first-ledger");
    assert_eq!(
        consume_evaluator_artifact(
            &inventory_path,
            &TestReward,
            &expected(&inventory),
            &count("legacy-run", "reproduction", "attempt", 0, None),
            &legacy_first_ledger,
        )
        .unwrap(),
        ConsumptionOutcome::Counted { reward: 1.0 }
    );
    assert!(
        consume_pair(
            &inventory_path,
            &inventory,
            &outcome_path,
            &outcome,
            &pair_count("campaign-legacy-cross-check", 0),
            &legacy_first_ledger,
        )
        .unwrap_err()
        .contains("already counted in the legacy ledger")
    );

    // A partial final frame is the only recoverable corruption class. It is
    // truncated under the ledger lock before the next complete atomic pair.
    use std::io::Write as _;
    std::fs::OpenOptions::new()
        .append(true)
        .open(ledger.join("receipt-pair-consumption.jsonl"))
        .unwrap()
        .write_all(b"{\"torn\":")
        .unwrap();
    let second_inventory = make_evidence("pair-inventory", "inventory-subject-2");
    let second_outcome = make_evidence("pair-outcome", "outcome-subject-2");
    assert!(
        consume_pair(
            &second_inventory.persist_append_only(&store).unwrap(),
            &second_inventory,
            &second_outcome.persist_append_only(&store).unwrap(),
            &second_outcome,
            &pair_count("campaign-1", 1),
            &ledger,
        )
        .unwrap()
        .counted
    );
    let contents = std::fs::read_to_string(ledger.join("receipt-pair-consumption.jsonl")).unwrap();
    assert_eq!(contents.lines().count(), 2);
    assert!(!contents.contains("torn"));
    let corrupted = contents.replacen("\"case_id\":\"case-1\"", "\"case_id\":\"case-x\"", 1);
    std::fs::write(ledger.join("receipt-pair-consumption.jsonl"), corrupted).unwrap();
    assert!(
        super::consumption::receipt_pair_ledger_head(&ledger)
            .unwrap_err()
            .contains("hash chain is invalid")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evaluator_pair_ledger_serializes_competing_outputs_for_one_slot() {
    // These fixtures spawn raw `git` (not the env-stripping pinned command),
    // so they hold the crate env lock: a concurrent test that sets GIT_DIR
    // (the workspace-evidence decoy) would otherwise redirect this repo's
    // git into a non-repository and fail these assertions at random.
    let _guard = crate::tests::env_lock();
    let root =
        std::env::temp_dir().join(format!("angel-evaluator-pair-race-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("workspace");
    let store = root.join("artifacts");
    let ledger = root.join("ledger");
    std::fs::create_dir_all(&workspace).unwrap();
    let git = crate::agent::harness::pinned_git_path().unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["init", "-q"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(workspace.join("fixture.txt"), "frozen").unwrap();
    assert!(
        std::process::Command::new(git)
            .args(["add", "fixture.txt"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );

    let pairs = ["candidate-a", "candidate-b"].map(|candidate| {
        let inventory = EvaluatorEvidence::run_shell(
            "race-inventory",
            "printf '%s' 'test result: ok. 1 passed; 0 failed;'",
            &workspace,
            TEST_VERIFIER_CONTRACT,
            &format!("{candidate}-inventory"),
        )
        .unwrap();
        let outcome = EvaluatorEvidence::run_shell(
            "race-outcome",
            "printf '%s' 'test result: ok. 1 passed; 0 failed;'",
            &workspace,
            TEST_VERIFIER_CONTRACT,
            &format!("{candidate}-outcome"),
        )
        .unwrap();
        let inventory_path = inventory.persist_append_only(&store).unwrap();
        let outcome_path = outcome.persist_append_only(&store).unwrap();
        (inventory, inventory_path, outcome, outcome_path)
    });
    let workers = pairs
        .into_iter()
        .map(|(inventory, inventory_path, outcome, outcome_path)| {
            let ledger = ledger.clone();
            std::thread::spawn(move || {
                consume_pair(
                    &inventory_path,
                    &inventory,
                    &outcome_path,
                    &outcome,
                    &pair_count("campaign-race", 0),
                    &ledger,
                )
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| result
                .as_ref()
                .is_err_and(|error| error.contains("slot is already frozen")))
            .count(),
        1
    );
    assert_eq!(
        std::fs::read_to_string(ledger.join("receipt-pair-consumption.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    let head = super::consumption::receipt_pair_ledger_head(&ledger).unwrap();
    let terminal = super::consumption::commit_terminal_release(
        &ledger,
        &head,
        b"{\"schema\":\"test-terminal\"}\n",
    )
    .unwrap();
    assert!(terminal.is_file());
    assert!(
        super::consumption::commit_terminal_release(
            &ledger,
            &head,
            b"{\"schema\":\"different-terminal\"}\n",
        )
        .unwrap_err()
        .contains("different terminal release")
    );
    let sealed_inventory = EvaluatorEvidence::run_shell(
        "sealed-inventory",
        "printf '%s' 'test result: ok. 1 passed; 0 failed;'",
        &workspace,
        TEST_VERIFIER_CONTRACT,
        "sealed-inventory-subject",
    )
    .unwrap();
    let sealed_outcome = EvaluatorEvidence::run_shell(
        "sealed-outcome",
        "printf '%s' 'test result: ok. 1 passed; 0 failed;'",
        &workspace,
        TEST_VERIFIER_CONTRACT,
        "sealed-outcome-subject",
    )
    .unwrap();
    assert!(
        consume_pair(
            &sealed_inventory.persist_append_only(&store).unwrap(),
            &sealed_inventory,
            &sealed_outcome.persist_append_only(&store).unwrap(),
            &sealed_outcome,
            &pair_count("campaign-race", 1),
            &ledger,
        )
        .unwrap_err()
        .contains("sealed by a terminal release")
    );
    let _ = std::fs::remove_dir_all(root);
}
