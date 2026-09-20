use super::*;
use crate::drive::reinforce::{Candidate, EvaluatorEvidence, TestReward};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

static FIXTURE_EVALUATION_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct NumericReward;
impl Reward for NumericReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let output = input.candidate_output(self.label())?;
        output.parse::<f32>().map_err(|error| error.to_string())
    }
    fn label(&self) -> &str {
        "numeric"
    }
}

struct PromptGenerator;
impl Generator for PromptGenerator {
    fn generate(&self, prompt: &str, task: &str, version: u64, n: usize) -> Vec<Candidate> {
        let score = match (prompt, task) {
            ("candidate", "case-a" | "audit-case-a") => 0.9,
            ("candidate", "case-b" | "audit-case-b") => 0.8,
            (_, "case-a" | "audit-case-a") => 0.7,
            _ => 0.6,
        };
        (0..n)
            .map(|_| Candidate {
                policy_version: version,
                latency: Duration::ZERO,
                output: score.to_string(),
            })
            .collect()
    }
}

struct MustNotGenerate;
impl Generator for MustNotGenerate {
    fn generate(&self, _prompt: &str, _task: &str, _version: u64, _n: usize) -> Vec<Candidate> {
        panic!("manifest rejection must happen before generation")
    }
}

struct MustNotScore;
impl Reward for MustNotScore {
    fn score(&self, _input: RewardInput<'_>) -> Result<f32, String> {
        panic!("a sealed campaign must not call the reward surface")
    }

    fn label(&self) -> &str {
        panic!("a sealed campaign must not inspect the reward surface")
    }
}

struct MustNotReflect;
impl crate::drive::reinforce::Reflector for MustNotReflect {
    fn improve(
        &self,
        _current_prompt: &str,
        _task: &str,
        _best: &str,
        _worst: &str,
    ) -> Result<String, String> {
        panic!("a sealed campaign must not call the reflector surface")
    }
}

struct MustNotEvaluate;
impl PolicyEvaluator for MustNotEvaluate {
    fn spec(&self) -> &super::super::evaluator::EvaluatorSpec {
        panic!("a sealed campaign must not inspect the evaluator surface")
    }

    fn evaluate(
        &self,
        _request: &super::super::evaluator::PolicyEvaluationRequest<'_>,
    ) -> Result<super::super::evaluator::PolicyEvaluationEvidence, String> {
        panic!("a sealed campaign must not call the evaluator surface")
    }
}

struct MustNotRunEvaluator {
    spec: super::super::evaluator::EvaluatorSpec,
}

impl PolicyEvaluator for MustNotRunEvaluator {
    fn spec(&self) -> &super::super::evaluator::EvaluatorSpec {
        &self.spec
    }

    fn evaluate(
        &self,
        _request: &super::super::evaluator::PolicyEvaluationRequest<'_>,
    ) -> Result<super::super::evaluator::PolicyEvaluationEvidence, String> {
        panic!("a committed evaluation slot must hydrate without evaluator execution")
    }
}

struct TechnicalGenerator;

impl Generator for TechnicalGenerator {
    fn generate(&self, prompt: &str, _task: &str, version: u64, n: usize) -> Vec<Candidate> {
        (0..n)
            .map(|_| Candidate {
                policy_version: version,
                latency: Duration::ZERO,
                output: if prompt == "candidate" {
                    "fixed".into()
                } else {
                    "broken".into()
                },
            })
            .collect()
    }
}

struct TechnicalCampaignGenerator;

impl Generator for TechnicalCampaignGenerator {
    fn generate(&self, prompt: &str, task: &str, version: u64, n: usize) -> Vec<Candidate> {
        (0..n)
            .map(|index| Candidate {
                policy_version: version,
                latency: Duration::ZERO,
                output: if task == "training" {
                    if index % 2 == 0 { "0.1" } else { "0.9" }.into()
                } else if prompt == "candidate" {
                    "fixed".into()
                } else {
                    "broken".into()
                },
            })
            .collect()
    }
}

struct ResumeAfterSelectionGenerator;

impl Generator for ResumeAfterSelectionGenerator {
    fn generate(&self, prompt: &str, task: &str, version: u64, n: usize) -> Vec<Candidate> {
        assert_ne!(task, "training", "frozen training must not be regenerated");
        (0..n)
            .map(|_| Candidate {
                policy_version: version,
                latency: Duration::ZERO,
                output: if prompt == "candidate" {
                    "fixed".into()
                } else {
                    "broken".into()
                },
            })
            .collect()
    }
}

struct CandidateReflector;

impl crate::drive::reinforce::Reflector for CandidateReflector {
    fn improve(
        &self,
        _current_prompt: &str,
        _task: &str,
        _best: &str,
        _worst: &str,
    ) -> Result<String, String> {
        Ok("candidate".into())
    }
}

struct FixtureEvaluator {
    spec: super::super::evaluator::EvaluatorSpec,
    root: std::path::PathBuf,
    command: String,
    inventory_command: String,
    substitute_command: bool,
    reject_final_audit: bool,
    swap_receipts: bool,
}

impl FixtureEvaluator {
    fn new(substitute_command: bool) -> Self {
        Self::with_inventory(substitute_command, "technical-a\ntechnical-b\n")
    }

    fn with_inventory(substitute_command: bool, inventory: &str) -> Self {
        Self::with_inventory_and_count(substitute_command, inventory, 2)
    }

    fn with_inventory_and_count(
        substitute_command: bool,
        inventory: &str,
        outcome_count: usize,
    ) -> Self {
        Self::with_fixture(
            substitute_command,
            inventory,
            "technical-a\ntechnical-b\n",
            outcome_count,
            b"selection-candidate.txt=<generated-output>",
        )
    }

    fn with_fixture(
        substitute_command: bool,
        inventory: &str,
        expected_inventory: &str,
        outcome_count: usize,
        fixture_identity: &[u8],
    ) -> Self {
        let outcome_ids = inventory.lines().take(outcome_count).collect::<Vec<_>>();
        let passing = outcome_ids
            .iter()
            .map(|id| format!("{id}\tpass"))
            .collect::<Vec<_>>()
            .join("\n");
        let failing = outcome_ids
            .iter()
            .map(|id| format!("{id}\tfail"))
            .collect::<Vec<_>>()
            .join("\n");
        let expected_inventory_sha256 =
            crate::knowledge::cut::sha256_hex(expected_inventory.as_bytes());
        let serial = FIXTURE_EVALUATION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "angel-promotion-fixture-{}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(root.join("fixture.txt"), fixture_identity).unwrap();
        let adapter = root.join("adapter");
        let adapter_source = format!(
            "#!/bin/sh\nset -eu\nif [ \"$1\" = inventory ]; then\n  printf '%s' '{inventory}'\n  exit 0\nfi\ncandidate=\nIFS= read -r candidate < \"$ANGEL_EVALUATOR_CANDIDATE_PATH\" || :\nif [ \"$candidate\" = fixed ]; then\n  printf '%s' 'angel.rlvr.test-outcome/v1\n{passing}\n'\nelse\n  printf '%s' 'angel.rlvr.test-outcome/v1\n{failing}\n'\nfi\n"
        );
        std::fs::write(&adapter, adapter_source).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&adapter).unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&adapter, permissions).unwrap();
        }
        let status = std::process::Command::new("git")
            .args(["add", "fixture.txt", "adapter"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        let adapter = std::fs::canonicalize(adapter).unwrap();
        let command = format!("{} outcome", adapter.display());
        let inventory_command = format!("{} inventory", adapter.display());
        Self {
            spec: super::super::evaluator::EvaluatorSpec::new(
                &root,
                &expected_inventory_sha256,
                "technical-pass/v1",
                &command,
                &inventory_command,
                &[adapter.as_path()],
            )
            .unwrap(),
            root,
            command,
            inventory_command,
            substitute_command,
            reject_final_audit: false,
            swap_receipts: false,
        }
    }

    fn rejecting_final_audit() -> Self {
        let mut evaluator = Self::for_audit();
        evaluator.reject_final_audit = true;
        evaluator
    }

    fn for_audit() -> Self {
        Self::with_fixture(
            false,
            "audit-technical-a\naudit-technical-b\n",
            "audit-technical-a\naudit-technical-b\n",
            2,
            b"audit-candidate.txt=<generated-output>",
        )
    }

    fn swapping_receipts() -> Self {
        let mut evaluator = Self::new(false);
        evaluator.swap_receipts = true;
        evaluator
    }

    fn underexecuting() -> Self {
        Self::with_inventory_and_count(false, "technical-a\ntechnical-b\n", 1)
    }
}

impl PolicyEvaluator for FixtureEvaluator {
    fn spec(&self) -> &super::super::evaluator::EvaluatorSpec {
        &self.spec
    }

    fn evaluate(
        &self,
        request: &PolicyEvaluationRequest<'_>,
    ) -> Result<PolicyEvaluationEvidence, String> {
        let evaluated_output =
            if self.reject_final_audit && request.cohort_role == CohortRole::FinalAudit.as_str() {
                "broken"
            } else {
                &request.candidate.output
            };
        let command = if self.substitute_command {
            "printf '%s' 'angel.rlvr.test-outcome/v1\ntechnical-a\tpass\ntest result: ok. 999 passed; 0 failed;\n'"
        } else {
            &self.command
        };
        let inventory = EvaluatorEvidence::run_shell(
            "promotion fixture inventory",
            &self.inventory_command,
            &self.root,
            &self.spec.inventory_contract(),
            &request.canonical_inventory_subject(),
        )?;
        let outcome = EvaluatorEvidence::run_shell_with_candidate(
            "promotion fixture evaluator",
            command,
            &self.root,
            self.spec.outcome_contract(),
            &request.canonical_subject(),
            evaluated_output.as_bytes(),
        )?;
        if self.swap_receipts {
            Ok(PolicyEvaluationEvidence {
                inventory: outcome,
                outcome: inventory,
            })
        } else {
            Ok(PolicyEvaluationEvidence { inventory, outcome })
        }
    }
}

impl Drop for FixtureEvaluator {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct ToggleEvaluator {
    inner: FixtureEvaluator,
    fail: AtomicBool,
}

impl PolicyEvaluator for ToggleEvaluator {
    fn spec(&self) -> &super::super::evaluator::EvaluatorSpec {
        self.inner.spec()
    }

    fn evaluate(
        &self,
        request: &PolicyEvaluationRequest<'_>,
    ) -> Result<PolicyEvaluationEvidence, String> {
        if self.fail.load(Ordering::SeqCst) {
            panic!("injected evaluator crash boundary")
        } else {
            self.inner.evaluate(request)
        }
    }
}

fn strict() -> PromotionConfig {
    PromotionConfig {
        min_cases: 2,
        samples_per_case: 4,
        absolute_floor: 0.80,
        min_mean_delta: 0.1,
        max_case_regression: 0.0,
        confidence_z: 1.96,
    }
}

fn promotion_manifest(
    id: &str,
    cases: &[HeldoutCase<'_>],
    config: &PromotionConfig,
) -> CohortManifest {
    CohortManifest::new(id, CohortRole::Promotion, cases, config).unwrap()
}

#[test]
fn complete_equal_budget_cohort_promotes_real_improvement() {
    let reward = NumericReward;
    let cases = [
        HeldoutCase {
            id: "a",
            task: "case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "b",
            task: "case-b",
            reward: &reward,
        },
    ];
    let config = strict();
    let manifest = promotion_manifest("complete", &cases, &config);
    let report = evaluate_promotion(
        &PromptGenerator,
        "incumbent",
        "candidate",
        3,
        &cases,
        &config,
        &manifest,
    )
    .unwrap();
    assert_eq!(report.decision, PromotionDecision::Promoted);
    assert!(report.promoted());
    assert!(report.cases.iter().all(|case| case.complete));
    assert_eq!(report.cases[0].incumbent_observed, 4);
    assert_eq!(report.cases[0].candidate_observed, 4);
    assert!(report.delta_lower_bound.unwrap() >= 0.1);
}

#[test]
fn receipt_backed_promotion_and_final_audit_require_real_complete_execution() {
    // Evaluator identity intentionally includes allowlisted process env
    // (notably HOME). Serialize against tests that temporarily rewrite it,
    // otherwise runtime revalidation can observe a mixed policy and turn a
    // complete cohort into a load-sensitive RejectedIncomplete result.
    let _env = crate::tests::env_lock();
    let receipt_root = std::env::temp_dir().join(format!(
        "angel-receipt-backed-promotion-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&receipt_root);
    let artifacts = receipt_root.join("artifacts");
    let ledger = receipt_root.join("ledger");
    let promotion_store = ReceiptStoreContext {
        artifact_root: &artifacts,
        ledger_root: &ledger,
        run_id: "technical-run",
        reproduction_id: "reproduction-1",
    };
    let reward = TechnicalReward::Pass;
    let evaluator = FixtureEvaluator::new(false);
    let evaluators: [&dyn PolicyEvaluator; 2] = [&evaluator, &evaluator];
    let cases = [
        HeldoutCase {
            id: "technical-a",
            task: "repair fixture a",
            reward: &reward,
        },
        HeldoutCase {
            id: "technical-b",
            task: "repair fixture b",
            reward: &reward,
        },
    ];
    let config = strict();
    let manifest = CohortManifest::new_with_evaluators(
        "technical-promotion",
        CohortRole::Promotion,
        &cases,
        &config,
        &evaluators,
    )
    .unwrap();
    assert!(
        CohortManifest::new_with_evaluators(
            "missing-evaluator",
            CohortRole::Promotion,
            &cases,
            &config,
            &[&evaluator],
        )
        .unwrap_err()
        .contains("exactly one evaluator per case")
    );
    assert!(
        evaluate_promotion(
            &MustNotGenerate,
            "incumbent",
            "candidate",
            4,
            &cases,
            &config,
            &manifest,
        )
        .unwrap_err()
        .contains("manifest drift")
    );
    let report = evaluate_promotion_with_receipts(
        &TechnicalGenerator,
        ReceiptPromotionRequest {
            incumbent_prompt: "incumbent",
            candidate_prompt: "candidate",
            incumbent_version: 4,
            cases: &cases,
            config: &config,
            manifest: &manifest,
            evaluators: &evaluators,
            receipt_store: promotion_store,
        },
    )
    .unwrap();
    assert_eq!(
        report.decision,
        PromotionDecision::Promoted,
        "receipt-backed promotion report: {report:#?}"
    );
    assert_eq!(report.evaluator_receipt_sha256s.len(), 32);
    assert_eq!(
        report
            .evaluator_receipt_sha256s
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        32,
        "every arm/case/sample must have distinct inventory and outcome receipts"
    );
    assert!(report.cases.iter().all(|case| {
        case.incumbent_mean == Some(0.0)
            && case.candidate_mean == Some(1.0)
            && case.incumbent_receipt_sha256s.len() == 8
            && case.candidate_receipt_sha256s.len() == 8
    }));

    let reduced_inventory =
        FixtureEvaluator::with_inventory(false, "technical-a\ntrivial-replacement\n");
    let reduced_evaluators: [&dyn PolicyEvaluator; 2] = [&reduced_inventory, &reduced_inventory];
    let reduced_manifest = CohortManifest::new_with_evaluators(
        "reduced-inventory",
        CohortRole::Promotion,
        &cases,
        &config,
        &reduced_evaluators,
    )
    .unwrap();
    let reduced = evaluate_promotion_with_receipts(
        &TechnicalGenerator,
        ReceiptPromotionRequest {
            incumbent_prompt: "incumbent",
            candidate_prompt: "candidate",
            incumbent_version: 4,
            cases: &cases,
            config: &config,
            manifest: &reduced_manifest,
            evaluators: &reduced_evaluators,
            receipt_store: ReceiptStoreContext {
                run_id: "reduced-inventory-run",
                ..promotion_store
            },
        },
    )
    .unwrap();
    assert_eq!(reduced.decision, PromotionDecision::RejectedIncomplete);
    assert!(reduced.cases.iter().all(|case| {
        case.failure
            .as_deref()
            .is_some_and(|failure| failure.contains("observed inventory mismatch"))
    }));

    let underexecuting = FixtureEvaluator::underexecuting();
    let underexecuting_evaluators: [&dyn PolicyEvaluator; 2] = [&underexecuting, &underexecuting];
    let underexecuting_manifest = CohortManifest::new_with_evaluators(
        "underexecuted-inventory",
        CohortRole::Promotion,
        &cases,
        &config,
        &underexecuting_evaluators,
    )
    .unwrap();
    let underexecuted = evaluate_promotion_with_receipts(
        &TechnicalGenerator,
        ReceiptPromotionRequest {
            incumbent_prompt: "incumbent",
            candidate_prompt: "candidate",
            incumbent_version: 4,
            cases: &cases,
            config: &config,
            manifest: &underexecuting_manifest,
            evaluators: &underexecuting_evaluators,
            receipt_store: ReceiptStoreContext {
                run_id: "underexecuted-inventory-run",
                ..promotion_store
            },
        },
    )
    .unwrap();
    assert_eq!(
        underexecuted.decision,
        PromotionDecision::RejectedIncomplete
    );
    assert!(underexecuted.cases.iter().all(|case| {
        case.failure
            .as_deref()
            .is_some_and(|failure| failure.contains("outcome inventory mismatch"))
    }));
    let must_not_run_a = MustNotRunEvaluator {
        spec: evaluator.spec().clone(),
    };
    let must_not_run_b = MustNotRunEvaluator {
        spec: evaluator.spec().clone(),
    };
    let must_not_run_evaluators: [&dyn PolicyEvaluator; 2] = [&must_not_run_a, &must_not_run_b];
    let resumed_run = evaluate_promotion_with_receipts(
        &MustNotGenerate,
        ReceiptPromotionRequest {
            incumbent_prompt: "incumbent",
            candidate_prompt: "candidate",
            incumbent_version: 4,
            cases: &cases,
            config: &config,
            manifest: &manifest,
            evaluators: &must_not_run_evaluators,
            receipt_store: promotion_store,
        },
    )
    .unwrap();
    assert_eq!(resumed_run.decision, PromotionDecision::Promoted);
    assert_eq!(
        resumed_run.evaluator_receipt_sha256s, report.evaluator_receipt_sha256s,
        "an exact cohort resume must recover the original receipt identities"
    );
    assert_eq!(
        std::fs::read_to_string(ledger.join("receipt-pair-consumption.jsonl"))
            .unwrap()
            .lines()
            .count(),
        16,
        "an exact cohort resume must not increase held-out N"
    );
    let report_path = std::fs::read_dir(ledger.join("reports"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            std::fs::read_to_string(path)
                .is_ok_and(|body| body.contains(manifest.manifest_sha256()))
        })
        .unwrap();
    let report_bytes = std::fs::read(&report_path).unwrap();
    let mut corrupted_report = String::from_utf8(report_bytes.clone()).unwrap();
    let marker = "\"record_sha256\":\"";
    let digest_offset = corrupted_report.find(marker).unwrap() + marker.len();
    let replacement = if &corrupted_report[digest_offset..digest_offset + 1] == "0" {
        "1"
    } else {
        "0"
    };
    corrupted_report.replace_range(digest_offset..digest_offset + 1, replacement);
    std::fs::write(&report_path, corrupted_report).unwrap();
    assert!(
        evaluate_promotion_with_receipts(
            &MustNotGenerate,
            ReceiptPromotionRequest {
                incumbent_prompt: "incumbent",
                candidate_prompt: "candidate",
                incumbent_version: 4,
                cases: &cases,
                config: &config,
                manifest: &manifest,
                evaluators: &must_not_run_evaluators,
                receipt_store: promotion_store,
            },
        )
        .unwrap_err()
        .contains("phase report authority context mismatch")
    );
    std::fs::write(&report_path, report_bytes).unwrap();

    let audit_cases = [
        HeldoutCase {
            id: "technical-audit-a",
            task: "repair untouched fixture a",
            reward: &reward,
        },
        HeldoutCase {
            id: "technical-audit-b",
            task: "repair untouched fixture b",
            reward: &reward,
        },
    ];
    let audit_evaluator = FixtureEvaluator::for_audit();
    let audit_evaluators: [&dyn PolicyEvaluator; 2] = [&audit_evaluator, &audit_evaluator];
    let audit_manifest = CohortManifest::new_with_evaluators(
        "technical-final-audit",
        CohortRole::FinalAudit,
        &audit_cases,
        &config,
        &audit_evaluators,
    )
    .unwrap();
    let audit = evaluate_final_audit_with_receipts(
        &TechnicalGenerator,
        ReceiptFinalAuditRequest {
            incumbent_prompt: "incumbent",
            frozen_candidate_prompt: "candidate",
            incumbent_version: 4,
            promotion_manifest: &manifest,
            audit_cases: &audit_cases,
            config: &config,
            audit_manifest: &audit_manifest,
            evaluators: &audit_evaluators,
            receipt_store: promotion_store,
        },
    )
    .unwrap();
    assert_eq!(audit.cohort_role, CohortRole::FinalAudit);
    assert_eq!(audit.decision, PromotionDecision::Promoted);
    assert_eq!(audit.evaluator_receipt_sha256s.len(), 32);

    let substituted = FixtureEvaluator::new(true);
    let substituted_evaluators: [&dyn PolicyEvaluator; 2] = [&substituted, &substituted];
    let substituted_manifest = CohortManifest::new_with_evaluators(
        "substituted-command",
        CohortRole::Promotion,
        &cases,
        &config,
        &substituted_evaluators,
    )
    .unwrap();
    let rejected = evaluate_promotion_with_receipts(
        &TechnicalGenerator,
        ReceiptPromotionRequest {
            incumbent_prompt: "incumbent",
            candidate_prompt: "candidate",
            incumbent_version: 4,
            cases: &cases,
            config: &config,
            manifest: &substituted_manifest,
            evaluators: &substituted_evaluators,
            receipt_store: ReceiptStoreContext {
                run_id: "substitution-run",
                ..promotion_store
            },
        },
    )
    .unwrap();
    assert_eq!(rejected.decision, PromotionDecision::RejectedIncomplete);
    assert!(rejected.cases.iter().all(|case| {
        case.failure
            .as_deref()
            .is_some_and(|failure| failure.contains("receipt identity mismatch"))
    }));
    let poisoned_a = MustNotRunEvaluator {
        spec: substituted.spec().clone(),
    };
    let poisoned_b = MustNotRunEvaluator {
        spec: substituted.spec().clone(),
    };
    let poisoned_evaluators: [&dyn PolicyEvaluator; 2] = [&poisoned_a, &poisoned_b];
    let poisoned_resume = evaluate_promotion_with_receipts(
        &MustNotGenerate,
        ReceiptPromotionRequest {
            incumbent_prompt: "incumbent",
            candidate_prompt: "candidate",
            incumbent_version: 4,
            cases: &cases,
            config: &config,
            manifest: &substituted_manifest,
            evaluators: &poisoned_evaluators,
            receipt_store: ReceiptStoreContext {
                run_id: "substitution-run",
                ..promotion_store
            },
        },
    )
    .unwrap();
    assert_eq!(
        poisoned_resume.decision,
        PromotionDecision::RejectedIncomplete
    );
    assert!(poisoned_resume.cases.iter().all(|case| {
        case.failure
            .as_deref()
            .is_some_and(|failure| failure.contains("slot is frozen to prior evaluator failure"))
    }));

    let swapped = FixtureEvaluator::swapping_receipts();
    let swapped_evaluators: [&dyn PolicyEvaluator; 2] = [&swapped, &swapped];
    let swapped_manifest = CohortManifest::new_with_evaluators(
        "swapped-receipt-roles",
        CohortRole::Promotion,
        &cases,
        &config,
        &swapped_evaluators,
    )
    .unwrap();
    let swapped_report = evaluate_promotion_with_receipts(
        &TechnicalGenerator,
        ReceiptPromotionRequest {
            incumbent_prompt: "incumbent",
            candidate_prompt: "candidate",
            incumbent_version: 4,
            cases: &cases,
            config: &config,
            manifest: &swapped_manifest,
            evaluators: &swapped_evaluators,
            receipt_store: ReceiptStoreContext {
                run_id: "swapped-receipt-run",
                ..promotion_store
            },
        },
    )
    .unwrap();
    assert_eq!(
        swapped_report.decision,
        PromotionDecision::RejectedIncomplete
    );
    assert!(swapped_report.cases.iter().all(|case| {
        case.failure
            .as_deref()
            .is_some_and(|failure| failure.contains("receipt identity mismatch"))
    }));
    let _ = std::fs::remove_dir_all(receipt_root);
}

#[test]
fn technical_campaign_resumes_frozen_training_and_phase_after_evaluator_failure() {
    let _env = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-technical-resume-stages-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let authority = crate::drive::reinforce::TechnicalCampaignAuthority::new_in(
        "resume-stages-run",
        root.clone(),
    )
    .unwrap();
    let selection_evaluator = ToggleEvaluator {
        inner: FixtureEvaluator::new(false),
        fail: AtomicBool::new(true),
    };
    let selection_evaluators: [&dyn PolicyEvaluator; 2] =
        [&selection_evaluator, &selection_evaluator];
    let audit_evaluator = FixtureEvaluator::for_audit();
    let audit_evaluators: [&dyn PolicyEvaluator; 2] = [&audit_evaluator, &audit_evaluator];
    let selection_cases = [
        TechnicalHeldoutCase {
            id: "resume-selection-a",
            task: "repair resume selection fixture a",
            reward: TechnicalReward::Pass,
        },
        TechnicalHeldoutCase {
            id: "resume-selection-b",
            task: "repair resume selection fixture b",
            reward: TechnicalReward::Pass,
        },
    ];
    let audit_cases = [
        TechnicalHeldoutCase {
            id: "resume-audit-a",
            task: "repair untouched resume audit fixture a",
            reward: TechnicalReward::Pass,
        },
        TechnicalHeldoutCase {
            id: "resume-audit-b",
            task: "repair untouched resume audit fixture b",
            reward: TechnicalReward::Pass,
        },
    ];
    let selection_config = strict();
    let audit_config = strict();
    let selection_manifest = CohortManifest::new_technical(
        "resume-stage-selection",
        CohortRole::Promotion,
        &selection_cases,
        &selection_config,
        &selection_evaluators,
    )
    .unwrap();
    let audit_manifest = CohortManifest::new_technical(
        "resume-stage-audit",
        CohortRole::FinalAudit,
        &audit_cases,
        &audit_config,
        &audit_evaluators,
    )
    .unwrap();
    let training_config = crate::drive::reinforce::ReinforceConfig {
        group_size: 4,
        oversample: 0.0,
        staleness_budget: 8,
        straggler_factor: 100.0,
        success_threshold: 0.5,
    };
    let run = |generator: &dyn Generator,
               reward: &dyn Reward,
               reflector: &dyn crate::drive::reinforce::Reflector| {
        crate::drive::reinforce::run_reinforce(
            generator,
            reward,
            reflector,
            crate::drive::reinforce::ReinforceRequest {
                task: "training",
                initial_prompt: "incumbent",
                rounds: 1,
                config: &training_config,
            },
            crate::drive::reinforce::TechnicalReinforceCampaign {
                authority: &authority,
                promotion: crate::drive::reinforce::TechnicalPromotionCohort {
                    cases: &selection_cases,
                    config: &selection_config,
                    manifest: &selection_manifest,
                    evaluators: &selection_evaluators,
                },
                final_audit: crate::drive::reinforce::TechnicalFinalAuditCohort {
                    cases: &audit_cases,
                    config: &audit_config,
                    manifest: &audit_manifest,
                    evaluators: &audit_evaluators,
                },
            },
        )
    };

    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(
            &TechnicalCampaignGenerator,
            &NumericReward,
            &CandidateReflector,
        )
    }));
    assert!(interrupted.is_err());
    assert!(root.join("ledger/candidate.plan").is_file());
    assert_eq!(
        std::fs::read_dir(root.join("ledger/plans"))
            .unwrap()
            .count(),
        1,
        "selection generation must be frozen before its first evaluator"
    );

    let corrupt_record_digest = |path: &Path| {
        let original = std::fs::read(path).unwrap();
        let mut corrupted = String::from_utf8(original.clone()).unwrap();
        let marker = "\"record_sha256\":\"";
        let offset = corrupted.find(marker).unwrap() + marker.len();
        let replacement = if &corrupted[offset..offset + 1] == "0" {
            "1"
        } else {
            "0"
        };
        corrupted.replace_range(offset..offset + 1, replacement);
        std::fs::write(path, corrupted).unwrap();
        original
    };
    let candidate_path = root.join("ledger/candidate.plan");
    let candidate_bytes = corrupt_record_digest(&candidate_path);
    assert!(
        run(
            &ResumeAfterSelectionGenerator,
            &MustNotScore,
            &MustNotReflect,
        )
        .unwrap_err()
        .contains("frozen training proposal authority context mismatch")
    );
    std::fs::write(&candidate_path, candidate_bytes).unwrap();

    let plan_path = std::fs::read_dir(root.join("ledger/plans"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let plan_bytes = corrupt_record_digest(&plan_path);
    assert!(
        run(
            &ResumeAfterSelectionGenerator,
            &MustNotScore,
            &MustNotReflect,
        )
        .unwrap_err()
        .contains("complete evaluation plan authority context mismatch")
    );
    std::fs::write(&plan_path, plan_bytes).unwrap();

    selection_evaluator.fail.store(false, Ordering::SeqCst);
    let resumed = run(
        &ResumeAfterSelectionGenerator,
        &MustNotScore,
        &MustNotReflect,
    )
    .unwrap();
    assert!(resumed.release_ready());
    assert_eq!(resumed.release_candidate().unwrap().prompt(), "candidate");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn production_technical_reinforce_requires_receipts_and_veto_only_final_audit() {
    let _env = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-production-technical-reinforce-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let authority = crate::drive::reinforce::TechnicalCampaignAuthority::new_in(
        "production-run",
        root.join("authority"),
    )
    .unwrap();
    let evaluator = FixtureEvaluator::new(false);
    let evaluators: [&dyn PolicyEvaluator; 2] = [&evaluator, &evaluator];
    let audit_evaluator = FixtureEvaluator::for_audit();
    let audit_evaluators: [&dyn PolicyEvaluator; 2] = [&audit_evaluator, &audit_evaluator];
    let promotion_cases = [
        TechnicalHeldoutCase {
            id: "production-a",
            task: "repair production fixture a",
            reward: TechnicalReward::Pass,
        },
        TechnicalHeldoutCase {
            id: "production-b",
            task: "repair production fixture b",
            reward: TechnicalReward::Pass,
        },
    ];
    let audit_cases = [
        TechnicalHeldoutCase {
            id: "audit-production-a",
            task: "repair untouched production fixture a",
            reward: TechnicalReward::Pass,
        },
        TechnicalHeldoutCase {
            id: "audit-production-b",
            task: "repair untouched production fixture b",
            reward: TechnicalReward::Pass,
        },
    ];
    let promotion_config = strict();
    let audit_config = strict();
    let promotion_manifest = CohortManifest::new_technical(
        "production-selection",
        CohortRole::Promotion,
        &promotion_cases,
        &promotion_config,
        &evaluators,
    )
    .unwrap();
    let audit_manifest = CohortManifest::new_technical(
        "production-final-audit",
        CohortRole::FinalAudit,
        &audit_cases,
        &audit_config,
        &audit_evaluators,
    )
    .unwrap();
    let relabeled_fixture_manifest = CohortManifest::new_technical(
        "mutated-selection-fixture",
        CohortRole::FinalAudit,
        &audit_cases,
        &audit_config,
        &evaluators,
    )
    .unwrap();
    assert_ne!(
        promotion_manifest.manifest_sha256(),
        relabeled_fixture_manifest.manifest_sha256(),
        "the regression fixture must change its aggregate manifest"
    );
    assert!(
        validate_audit_separation(&promotion_manifest, &relabeled_fixture_manifest)
            .unwrap_err()
            .contains("identity overlaps")
    );
    let mut bypass_floor = strict();
    bypass_floor.absolute_floor = 0.0;
    assert!(
        CohortManifest::new_technical(
            "caller-lowered-floor",
            CohortRole::Promotion,
            &promotion_cases,
            &bypass_floor,
            &evaluators,
        )
        .unwrap_err()
        .contains("technical release absolute_floor")
    );
    let training_config = crate::drive::reinforce::ReinforceConfig {
        group_size: 4,
        oversample: 0.0,
        staleness_budget: 8,
        straggler_factor: 100.0,
        success_threshold: 0.5,
    };
    let run_campaign = |task| {
        crate::drive::reinforce::run_reinforce(
            &TechnicalCampaignGenerator,
            &NumericReward,
            &CandidateReflector,
            crate::drive::reinforce::ReinforceRequest {
                task,
                initial_prompt: "incumbent",
                rounds: 1,
                config: &training_config,
            },
            crate::drive::reinforce::TechnicalReinforceCampaign {
                authority: &authority,
                promotion: crate::drive::reinforce::TechnicalPromotionCohort {
                    cases: &promotion_cases,
                    config: &promotion_config,
                    manifest: &promotion_manifest,
                    evaluators: &evaluators,
                },
                final_audit: crate::drive::reinforce::TechnicalFinalAuditCohort {
                    cases: &audit_cases,
                    config: &audit_config,
                    manifest: &audit_manifest,
                    evaluators: &audit_evaluators,
                },
            },
        )
    };
    let report = run_campaign("training").unwrap();
    assert_eq!(
        report.reinforcement.final_prompt, "candidate",
        "technical campaign report: {:#?}",
        report
    );
    assert!(report.reinforcement.rounds[0].optimized);
    assert!(report.release_ready());
    let release = report.release_candidate().unwrap();
    assert_eq!(release.prompt(), "candidate");
    assert_eq!(release.policy_version(), 1);
    assert_eq!(release.release_sha256().len(), 64);
    let release_sha256 = release.release_sha256().to_string();
    let release_anchor_sha256 = authority.release_anchor_sha256().unwrap();
    assert_eq!(release_anchor_sha256.len(), 64);
    authority
        .verify_release_anchor(&release_anchor_sha256)
        .unwrap();
    assert!(
        authority
            .verify_release_anchor("not-a-sha256")
            .unwrap_err()
            .contains("lowercase SHA-256")
    );
    assert!(
        authority
            .verify_release_anchor(&"0".repeat(64))
            .unwrap_err()
            .contains("external release anchor mismatch")
    );
    let audit = report.final_audit.unwrap();
    assert_eq!(audit.cohort_role, CohortRole::FinalAudit);
    assert_eq!(audit.evaluator_receipt_sha256s.len(), 32);
    let ledger_rows = std::fs::read_to_string(
        authority
            .ledger_root()
            .join("receipt-pair-consumption.jsonl"),
    )
    .unwrap()
    .lines()
    .count();
    assert_eq!(
        ledger_rows, 32,
        "selection and audit must atomically consume every receipt pair"
    );
    let resumed = run_campaign("training").unwrap();
    assert_eq!(
        resumed.release_candidate().unwrap().release_sha256(),
        release_sha256,
        "an exact crash-resume must recover the durable release byte-identically"
    );
    let must_not_evaluate = MustNotEvaluate;
    let panic_selection_evaluators: [&dyn PolicyEvaluator; 2] =
        [&must_not_evaluate, &must_not_evaluate];
    let panic_audit_evaluators: [&dyn PolicyEvaluator; 2] =
        [&must_not_evaluate, &must_not_evaluate];
    let sealed = crate::drive::reinforce::run_reinforce(
        &MustNotGenerate,
        &MustNotScore,
        &MustNotReflect,
        crate::drive::reinforce::ReinforceRequest {
            task: "training",
            initial_prompt: "incumbent",
            rounds: 1,
            config: &training_config,
        },
        crate::drive::reinforce::TechnicalReinforceCampaign {
            authority: &authority,
            promotion: crate::drive::reinforce::TechnicalPromotionCohort {
                cases: &promotion_cases,
                config: &promotion_config,
                manifest: &promotion_manifest,
                evaluators: &panic_selection_evaluators,
            },
            final_audit: crate::drive::reinforce::TechnicalFinalAuditCohort {
                cases: &audit_cases,
                config: &audit_config,
                manifest: &audit_manifest,
                evaluators: &panic_audit_evaluators,
            },
        },
    )
    .unwrap();
    assert_eq!(
        sealed.release_candidate().unwrap().release_sha256(),
        release_sha256,
        "terminal hydration must bypass every provider and evaluator surface"
    );
    let release_path = authority.release_path();
    let release_bytes = std::fs::read(&release_path).unwrap();
    let mut tampered_release = String::from_utf8(release_bytes.clone()).unwrap();
    let digest_offset = tampered_release.find(&release_sha256).unwrap();
    let replacement = if &tampered_release[digest_offset..digest_offset + 1] == "0" {
        "1"
    } else {
        "0"
    };
    tampered_release.replace_range(digest_offset..digest_offset + 1, replacement);
    std::fs::write(&release_path, tampered_release).unwrap();
    assert!(
        run_campaign("training")
            .unwrap_err()
            .contains("terminal release candidate digest mismatch")
    );
    assert!(
        authority
            .verify_release_anchor(&release_anchor_sha256)
            .unwrap_err()
            .contains("terminal release candidate digest mismatch")
    );
    std::fs::write(&release_path, release_bytes).unwrap();
    authority
        .verify_release_anchor(&release_anchor_sha256)
        .unwrap();
    let release_bytes = std::fs::read(&release_path).unwrap();
    let mut coherent_replacement: serde_json::Value =
        serde_json::from_slice(&release_bytes).unwrap();
    coherent_replacement["request_sha256"] = serde_json::Value::String("0".repeat(64));
    let mut coherent_replacement = serde_json::to_vec(&coherent_replacement).unwrap();
    coherent_replacement.push(b'\n');
    std::fs::write(&release_path, coherent_replacement).unwrap();
    assert_ne!(
        authority.release_anchor_sha256().unwrap(),
        release_anchor_sha256,
        "an internally coherent terminal replacement must change the external anchor"
    );
    assert!(
        authority
            .verify_release_anchor(&release_anchor_sha256)
            .unwrap_err()
            .contains("external release anchor mismatch")
    );
    std::fs::write(&release_path, release_bytes).unwrap();
    assert_eq!(
        std::fs::read_to_string(
            authority
                .ledger_root()
                .join("receipt-pair-consumption.jsonl"),
        )
        .unwrap()
        .lines()
        .count(),
        32,
        "an exact resume must not increase the evaluation cohort"
    );
    let release_commit = std::fs::read_to_string(authority.release_path()).unwrap();
    assert!(release_commit.contains(&release_sha256));
    assert!(release_commit.contains("receipt_ledger_head_sha256"));
    assert!(
        run_campaign("changed-training-task")
            .unwrap_err()
            .contains("immutable campaign record drift")
    );

    let veto_root = root.join("veto");
    let veto_authority =
        crate::drive::reinforce::TechnicalCampaignAuthority::new_in("veto-run", veto_root).unwrap();
    let veto_selection_evaluator = FixtureEvaluator::new(false);
    let veto_selection_evaluators: [&dyn PolicyEvaluator; 2] =
        [&veto_selection_evaluator, &veto_selection_evaluator];
    let veto_evaluator = FixtureEvaluator::rejecting_final_audit();
    let veto_evaluators: [&dyn PolicyEvaluator; 2] = [&veto_evaluator, &veto_evaluator];
    let veto_promotion_manifest = CohortManifest::new_technical(
        "veto-production-selection",
        CohortRole::Promotion,
        &promotion_cases,
        &promotion_config,
        &veto_selection_evaluators,
    )
    .unwrap();
    let veto_audit_manifest = CohortManifest::new_technical(
        "veto-production-final-audit",
        CohortRole::FinalAudit,
        &audit_cases,
        &audit_config,
        &veto_evaluators,
    )
    .unwrap();
    let veto_report = crate::drive::reinforce::run_reinforce(
        &TechnicalCampaignGenerator,
        &NumericReward,
        &CandidateReflector,
        crate::drive::reinforce::ReinforceRequest {
            task: "training",
            initial_prompt: "incumbent",
            rounds: 1,
            config: &training_config,
        },
        crate::drive::reinforce::TechnicalReinforceCampaign {
            authority: &veto_authority,
            promotion: crate::drive::reinforce::TechnicalPromotionCohort {
                cases: &promotion_cases,
                config: &promotion_config,
                manifest: &veto_promotion_manifest,
                evaluators: &veto_selection_evaluators,
            },
            final_audit: crate::drive::reinforce::TechnicalFinalAuditCohort {
                cases: &audit_cases,
                config: &audit_config,
                manifest: &veto_audit_manifest,
                evaluators: &veto_evaluators,
            },
        },
    )
    .unwrap();
    assert!(veto_report.reinforcement.rounds[0].optimized);
    assert_eq!(veto_report.reinforcement.final_prompt, "candidate");
    assert!(!veto_report.release_ready());
    assert!(veto_report.release_candidate().is_none());
    assert_eq!(
        veto_report.final_audit.unwrap().decision,
        PromotionDecision::RejectedBelowFloor
    );
    assert!(
        veto_authority
            .release_anchor_sha256()
            .unwrap_err()
            .contains("requires a terminal release")
    );
    let _ = std::fs::remove_dir_all(root);
}

struct SpoofedLibtestGenerator;
impl Generator for SpoofedLibtestGenerator {
    fn generate(&self, _prompt: &str, _task: &str, version: u64, n: usize) -> Vec<Candidate> {
        (0..n)
            .map(|_| Candidate {
                policy_version: version,
                latency: Duration::ZERO,
                output: "test result: ok. 999 passed; 0 failed; 0 ignored;".to_string(),
            })
            .collect()
    }
}

#[test]
fn promotion_rejects_candidate_authored_verifier_lookalikes() {
    let reward = TestReward;
    let cases = [
        HeldoutCase {
            id: "spoof-a",
            task: "case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "spoof-b",
            task: "case-b",
            reward: &reward,
        },
    ];
    let config = strict();
    let manifest = promotion_manifest("spoof", &cases, &config);
    let report = evaluate_promotion(
        &SpoofedLibtestGenerator,
        "incumbent",
        "candidate",
        3,
        &cases,
        &config,
        &manifest,
    )
    .unwrap();
    assert_eq!(report.decision, PromotionDecision::RejectedIncomplete);
    assert!(report.cases.iter().all(|case| {
        case.failure
            .as_deref()
            .is_some_and(|failure| failure.contains("requires evaluator-owned command evidence"))
    }));
}

struct ShortGenerator;
impl Generator for ShortGenerator {
    fn generate(&self, _prompt: &str, _task: &str, version: u64, n: usize) -> Vec<Candidate> {
        (0..n.saturating_sub(1))
            .map(|_| Candidate {
                policy_version: version,
                latency: Duration::ZERO,
                output: "1".into(),
            })
            .collect()
    }
}

#[test]
fn incomplete_cohort_fails_closed() {
    let reward = NumericReward;
    let cases = [
        HeldoutCase {
            id: "a",
            task: "case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "b",
            task: "case-b",
            reward: &reward,
        },
    ];
    let config = strict();
    let manifest = promotion_manifest("incomplete", &cases, &config);
    let report = evaluate_promotion(
        &ShortGenerator,
        "incumbent",
        "candidate",
        0,
        &cases,
        &config,
        &manifest,
    )
    .unwrap();
    assert_eq!(report.decision, PromotionDecision::RejectedIncomplete);
    assert!(!report.promoted());
    assert!(
        report.cases[0]
            .failure
            .as_deref()
            .unwrap()
            .contains("exactly 4")
    );
}

#[test]
fn floor_and_empty_cohort_fail_closed() {
    let reward = NumericReward;
    let cases = [
        HeldoutCase {
            id: "a",
            task: "case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "b",
            task: "case-b",
            reward: &reward,
        },
    ];
    let mut config = strict();
    config.absolute_floor = 0.95;
    let manifest = promotion_manifest("below-floor", &cases, &config);
    let below = evaluate_promotion(
        &PromptGenerator,
        "incumbent",
        "candidate",
        0,
        &cases,
        &config,
        &manifest,
    )
    .unwrap();
    assert_eq!(below.decision, PromotionDecision::RejectedBelowFloor);

    let empty_config = strict();
    let empty_manifest = promotion_manifest("empty", &[], &empty_config);
    let empty = evaluate_promotion(
        &PromptGenerator,
        "incumbent",
        "candidate",
        0,
        &[],
        &empty_config,
        &empty_manifest,
    )
    .unwrap();
    assert_eq!(empty.decision, PromotionDecision::RejectedEmptyCohort);
}

#[test]
fn undersized_or_duplicate_cohort_fails_closed() {
    let reward = NumericReward;
    let short = [HeldoutCase {
        id: "a",
        task: "case-a",
        reward: &reward,
    }];
    let config = strict();
    let manifest = promotion_manifest("short", &short, &config);
    let report = evaluate_promotion(
        &PromptGenerator,
        "incumbent",
        "candidate",
        0,
        &short,
        &config,
        &manifest,
    )
    .unwrap();
    assert_eq!(
        report.decision,
        PromotionDecision::RejectedInsufficientCohort
    );

    let duplicate = [
        HeldoutCase {
            id: "same",
            task: "case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "same",
            task: "case-b",
            reward: &reward,
        },
    ];
    assert!(
        CohortManifest::new("duplicate", CohortRole::Promotion, &duplicate, &config,)
            .unwrap_err()
            .contains("duplicate")
    );
}

#[test]
fn cohort_manifest_rejects_drift_relabeling_and_final_audit_overlap() {
    let reward = NumericReward;
    let config = strict();
    let promotion_cases = [
        HeldoutCase {
            id: "selection-a",
            task: "case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "selection-b",
            task: "case-b",
            reward: &reward,
        },
    ];
    let promotion = promotion_manifest("selection-v1", &promotion_cases, &config);

    let drifted_cases = [
        HeldoutCase {
            id: "selection-a",
            task: "case-b",
            reward: &reward,
        },
        HeldoutCase {
            id: "selection-b",
            task: "case-a",
            reward: &reward,
        },
    ];
    assert!(
        evaluate_promotion(
            &MustNotGenerate,
            "incumbent",
            "candidate",
            0,
            &drifted_cases,
            &config,
            &promotion,
        )
        .unwrap_err()
        .contains("manifest drift")
    );

    let mut drifted_config = config.clone();
    drifted_config.absolute_floor = 0.9;
    assert!(
        evaluate_promotion(
            &MustNotGenerate,
            "incumbent",
            "candidate",
            0,
            &promotion_cases,
            &drifted_config,
            &promotion,
        )
        .unwrap_err()
        .contains("manifest drift")
    );

    let relabeled = CohortManifest::new(
        "selection-v1",
        CohortRole::FinalAudit,
        &promotion_cases,
        &config,
    )
    .unwrap();
    assert!(
        evaluate_promotion(
            &MustNotGenerate,
            "incumbent",
            "candidate",
            0,
            &promotion_cases,
            &config,
            &relabeled,
        )
        .unwrap_err()
        .contains("expected promotion")
    );

    let id_overlapping_audit_cases = [
        HeldoutCase {
            id: "selection-b",
            task: "audit-case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "audit-c",
            task: "audit-case-b",
            reward: &reward,
        },
    ];
    let id_overlapping_audit = CohortManifest::new(
        "audit-id-overlap",
        CohortRole::FinalAudit,
        &id_overlapping_audit_cases,
        &config,
    )
    .unwrap();
    assert!(
        evaluate_final_audit(
            &MustNotGenerate,
            FinalAuditRequest {
                incumbent_prompt: "incumbent",
                frozen_candidate_prompt: "candidate",
                incumbent_version: 0,
                promotion_manifest: &promotion,
                audit_cases: &id_overlapping_audit_cases,
                config: &config,
                audit_manifest: &id_overlapping_audit,
            },
        )
        .unwrap_err()
        .contains("overlaps")
    );

    let task_overlapping_audit_cases = [
        HeldoutCase {
            id: "audit-relabeled",
            task: "case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "audit-c",
            task: "case-b",
            reward: &reward,
        },
    ];
    let task_overlapping_audit = CohortManifest::new(
        "audit-task-overlap",
        CohortRole::FinalAudit,
        &task_overlapping_audit_cases,
        &config,
    )
    .unwrap();
    assert!(
        evaluate_final_audit(
            &MustNotGenerate,
            FinalAuditRequest {
                incumbent_prompt: "incumbent",
                frozen_candidate_prompt: "candidate",
                incumbent_version: 0,
                promotion_manifest: &promotion,
                audit_cases: &task_overlapping_audit_cases,
                config: &config,
                audit_manifest: &task_overlapping_audit,
            },
        )
        .unwrap_err()
        .contains("relabels")
    );

    let audit_cases = [
        HeldoutCase {
            id: "audit-a",
            task: "audit-case-a",
            reward: &reward,
        },
        HeldoutCase {
            id: "audit-b",
            task: "audit-case-b",
            reward: &reward,
        },
    ];
    let audit =
        CohortManifest::new("audit-v1", CohortRole::FinalAudit, &audit_cases, &config).unwrap();
    let report = evaluate_final_audit(
        &PromptGenerator,
        FinalAuditRequest {
            incumbent_prompt: "incumbent",
            frozen_candidate_prompt: "candidate",
            incumbent_version: 0,
            promotion_manifest: &promotion,
            audit_cases: &audit_cases,
            config: &config,
            audit_manifest: &audit,
        },
    )
    .unwrap();
    assert_eq!(report.cohort_role, CohortRole::FinalAudit);
    assert_eq!(report.cohort_manifest_sha256, audit.manifest_sha256());
    assert_eq!(
        report.candidate_prompt_sha256,
        crate::knowledge::cut::sha256_hex(b"candidate")
    );
    assert_eq!(report.incumbent_version, 0);
    assert_eq!(report.candidate_version, 1);
    assert_eq!(report.decision, PromotionDecision::Promoted);
}
