//! Fixed-budget promotion gate for prompt-policy mutations.
//!
//! Training reward is allowed to propose a mutation. It is not allowed to
//! promote one. Promotion uses a separate cohort whose scores are never passed
//! to the reflector, evaluates incumbent and candidate with the same budget,
//! and fails closed when any sample or case is missing.

use super::evaluator::{
    PolicyEvaluationEvidence, PolicyEvaluationRequest, PolicyEvaluator, canonical_inventory,
    canonical_test_outcome,
};
use super::{Generator, Reward, RewardInput};
use serde::{Deserialize, Serialize};
use std::path::Path;

const COHORT_MANIFEST_SCHEMA: &str = "angel.rlvr.cohort-manifest/v2";
const TECHNICAL_RELEASE_MINIMUM_PASS_RATE: f32 = 0.80;
const REWARD_SCORING_CONTRACT_SCHEMA: &str = "angel.rlvr.reward-scoring/v1";
const PROMOTION_MATH_CONTRACT_SCHEMA: &str = "angel.rlvr.promotion-math/v1";

fn reward_contract_sha256(label: &str) -> String {
    crate::cut::sha256_hex(format!("{REWARD_SCORING_CONTRACT_SCHEMA}\0{label}").as_bytes())
}

pub(crate) fn promotion_math_contract_sha256() -> String {
    crate::cut::sha256_hex(PROMOTION_MATH_CONTRACT_SCHEMA.as_bytes())
}

#[derive(Clone, Debug)]
pub struct PromotionConfig {
    /// Minimum distinct cases required for a promotion cohort.
    pub min_cases: usize,
    /// Equal generation budget for incumbent and candidate on every case.
    pub samples_per_case: usize,
    /// Candidate mean on every case must meet this mandatory floor.
    pub absolute_floor: f32,
    /// Required lower confidence bound for the mean candidate-incumbent delta.
    pub min_mean_delta: f32,
    /// Maximum tolerated regression on any individual case.
    pub max_case_regression: f32,
    /// Normal critical value used for the conservative delta bound.
    pub confidence_z: f32,
}

impl PromotionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.min_cases == 0 {
            return Err("promotion min_cases must be greater than zero".into());
        }
        if self.samples_per_case < 2 {
            return Err("promotion samples_per_case must be at least two".into());
        }
        if !self.absolute_floor.is_finite() {
            return Err("promotion absolute_floor must be finite".into());
        }
        if !self.min_mean_delta.is_finite() || self.min_mean_delta <= 0.0 {
            return Err("promotion min_mean_delta must be finite and positive".into());
        }
        if !self.max_case_regression.is_finite() || self.max_case_regression < 0.0 {
            return Err("promotion max_case_regression must be finite and non-negative".into());
        }
        if !self.confidence_z.is_finite() || self.confidence_z < 0.0 {
            return Err("promotion confidence_z must be finite and non-negative".into());
        }
        Ok(())
    }

    fn validate_technical_release(&self) -> Result<(), String> {
        self.validate()?;
        if !(TECHNICAL_RELEASE_MINIMUM_PASS_RATE..=1.0).contains(&self.absolute_floor) {
            return Err(format!(
                "technical release absolute_floor must be in [{TECHNICAL_RELEASE_MINIMUM_PASS_RATE}, 1]"
            ));
        }
        if self.confidence_z <= 0.0 {
            return Err("technical release confidence_z must be positive".into());
        }
        Ok(())
    }
}

/// A validation case kept out of the reflector's training examples.
pub struct HeldoutCase<'a> {
    pub id: &'a str,
    pub task: &'a str,
    pub reward: &'a dyn Reward,
}

/// Sealed technical scoring policies accepted by the production coding gate.
/// Callers cannot supply an arbitrary reward that ignores red evaluator
/// outcomes while borrowing the receipt machinery's authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TechnicalReward {
    Pass,
    /// A command-success objective case: the evaluator runs the operator's own
    /// verifier command, so the reward is that process's outcome. A red
    /// objective is a measured zero (the run exists to make it green), and when
    /// the verifier does emit canonical libtest summaries they are parsed
    /// strictly and must be green.
    ObjectivePass,
}

impl Reward for TechnicalReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        match self {
            Self::Pass => {
                let evidence = input.evaluator_evidence(self.label())?;
                evidence.validate_for_scoring()?;
                if !evidence.succeeded() || !evidence.stderr_is_empty() {
                    return Err(
                        "technical evaluator outcome process was not canonically successful".into(),
                    );
                }
                let outcome = canonical_test_outcome(evidence.stdout_bytes())?;
                Ok(if outcome.passed > 0 && outcome.failed == 0 {
                    1.0
                } else {
                    0.0
                })
            }
            Self::ObjectivePass => {
                let evidence = input.evaluator_evidence(self.label())?;
                evidence.validate_for_scoring()?;
                // Case identity (frozen source, verifier command, execution
                // policy, sample subject) is enforced before this reward runs:
                // the receipt path checks the exact `EvaluatorSpec` manifest and
                // subject it was produced under, and the training path produces
                // the command-success contract. A contract string here could
                // only re-assert something the caller already bound, so this
                // reward scores the measurement itself and never the text.
                let output = evidence.output();
                if output.contains("test result:") {
                    // Preserve the stronger structured verdict when the
                    // operator's own command reports libtest results.
                    let outcome = super::parse_evaluator_test_result(output)?;
                    if outcome.passed.saturating_add(outcome.failed) == 0 {
                        return Ok(0.0);
                    }
                    return Ok(f32::from(evidence.succeeded() && outcome.failed == 0));
                }
                Ok(f32::from(evidence.succeeded()))
            }
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Pass => "technical-pass",
            Self::ObjectivePass => "objective-pass",
        }
    }
}

pub struct TechnicalHeldoutCase<'a> {
    pub id: &'a str,
    pub task: &'a str,
    pub reward: TechnicalReward,
}

pub(crate) fn technical_cases_as_heldout<'a>(
    cases: &'a [TechnicalHeldoutCase<'a>],
) -> Vec<HeldoutCase<'a>> {
    cases
        .iter()
        .map(|case| HeldoutCase {
            id: case.id,
            task: case.task,
            reward: &case.reward,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum CohortRole {
    Promotion,
    FinalAudit,
}

impl CohortRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Promotion => "promotion",
            Self::FinalAudit => "final-audit",
        }
    }
}

/// Frozen identity for one ordered evaluator cohort. Case order is material:
/// the gate alternates incumbent/candidate evaluation order by case index, so a
/// reorder is an experiment change rather than an equivalent set.
#[derive(Clone, Debug)]
pub struct CohortManifest {
    id: String,
    role: CohortRole,
    case_ids: Vec<String>,
    task_sha256s: Vec<String>,
    evaluator_manifest_sha256s: Vec<String>,
    evaluator_fixture_sha256s: Vec<String>,
    evaluator_verifier_sha256s: Vec<String>,
    evaluator_inventory_sha256s: Vec<String>,
    cases_sha256: String,
    config_sha256: String,
    manifest_sha256: String,
}

impl CohortManifest {
    pub fn new(
        id: &str,
        role: CohortRole,
        cases: &[HeldoutCase<'_>],
        config: &PromotionConfig,
    ) -> Result<Self, String> {
        Self::new_bound(id, role, cases, config, None)
    }

    pub fn new_with_evaluators(
        id: &str,
        role: CohortRole,
        cases: &[HeldoutCase<'_>],
        config: &PromotionConfig,
        evaluators: &[&dyn PolicyEvaluator],
    ) -> Result<Self, String> {
        Self::new_bound(id, role, cases, config, Some(evaluators))
    }

    pub fn new_technical(
        id: &str,
        role: CohortRole,
        cases: &[TechnicalHeldoutCase<'_>],
        config: &PromotionConfig,
        evaluators: &[&dyn PolicyEvaluator],
    ) -> Result<Self, String> {
        config.validate_technical_release()?;
        let cases = technical_cases_as_heldout(cases);
        Self::new_with_evaluators(id, role, &cases, config, evaluators)
    }

    pub fn validate_technical(
        &self,
        expected_role: CohortRole,
        cases: &[TechnicalHeldoutCase<'_>],
        config: &PromotionConfig,
        evaluators: &[&dyn PolicyEvaluator],
    ) -> Result<(), String> {
        config.validate_technical_release()?;
        let cases = technical_cases_as_heldout(cases);
        self.validate_with_evaluators(expected_role, &cases, config, evaluators)
    }

    fn new_bound(
        id: &str,
        role: CohortRole,
        cases: &[HeldoutCase<'_>],
        config: &PromotionConfig,
        evaluators: Option<&[&dyn PolicyEvaluator]>,
    ) -> Result<Self, String> {
        config.validate()?;
        let id = id.trim();
        if id.is_empty() {
            return Err("cohort manifest id must not be empty".into());
        }
        validate_cases(cases)?;
        if evaluators.is_some_and(|items| items.len() != cases.len()) {
            return Err("evaluator-bound cohort requires exactly one evaluator per case".into());
        }

        let mut canonical_cases = String::new();
        let mut case_ids = Vec::with_capacity(cases.len());
        let mut task_sha256s = Vec::with_capacity(cases.len());
        let mut evaluator_manifest_sha256s = Vec::with_capacity(cases.len());
        let mut evaluator_fixture_sha256s = Vec::with_capacity(cases.len());
        let mut evaluator_verifier_sha256s = Vec::with_capacity(cases.len());
        let mut evaluator_inventory_sha256s = Vec::with_capacity(cases.len());
        for (index, case) in cases.iter().enumerate() {
            if let Some(items) = evaluators {
                items[index].spec().validate_runtime()?;
            }
            append_len_prefixed(&mut canonical_cases, case.id);
            append_len_prefixed(&mut canonical_cases, case.task);
            append_len_prefixed(&mut canonical_cases, case.reward.label());
            if let Some(items) = evaluators {
                append_len_prefixed(&mut canonical_cases, items[index].spec().manifest_sha256());
                append_len_prefixed(&mut canonical_cases, items[index].spec().command_sha256());
                append_len_prefixed(
                    &mut canonical_cases,
                    items[index].spec().execution_policy_sha256(),
                );
                evaluator_manifest_sha256s.push(items[index].spec().manifest_sha256().to_string());
                evaluator_fixture_sha256s.push(items[index].spec().fixture_sha256().to_string());
                evaluator_verifier_sha256s
                    .push(items[index].spec().verifier_bundle_sha256().to_string());
                evaluator_inventory_sha256s
                    .push(items[index].spec().expected_inventory_sha256().to_string());
            } else {
                append_len_prefixed(&mut canonical_cases, "candidate-output/v1");
            }
            case_ids.push(case.id.to_string());
            task_sha256s.push(crate::cut::sha256_hex(case.task.as_bytes()));
        }
        let cases_sha256 = crate::cut::sha256_hex(canonical_cases.as_bytes());
        let canonical_config = format!(
            "min_cases={};samples_per_case={};absolute_floor={:08x};min_mean_delta={:08x};max_case_regression={:08x};confidence_z={:08x}",
            config.min_cases,
            config.samples_per_case,
            config.absolute_floor.to_bits(),
            config.min_mean_delta.to_bits(),
            config.max_case_regression.to_bits(),
            config.confidence_z.to_bits(),
        );
        let config_sha256 = crate::cut::sha256_hex(canonical_config.as_bytes());
        let mut canonical_manifest = String::new();
        for value in [
            COHORT_MANIFEST_SCHEMA,
            id,
            role.as_str(),
            &cases.len().to_string(),
            &cases_sha256,
            &config_sha256,
        ] {
            append_len_prefixed(&mut canonical_manifest, value);
        }
        let manifest_sha256 = crate::cut::sha256_hex(canonical_manifest.as_bytes());
        Ok(Self {
            id: id.to_string(),
            role,
            case_ids,
            task_sha256s,
            evaluator_manifest_sha256s,
            evaluator_fixture_sha256s,
            evaluator_verifier_sha256s,
            evaluator_inventory_sha256s,
            cases_sha256,
            config_sha256,
            manifest_sha256,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn role(&self) -> CohortRole {
        self.role
    }

    pub fn cases_sha256(&self) -> &str {
        &self.cases_sha256
    }

    pub fn config_sha256(&self) -> &str {
        &self.config_sha256
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    pub fn validate(
        &self,
        expected_role: CohortRole,
        cases: &[HeldoutCase<'_>],
        config: &PromotionConfig,
    ) -> Result<(), String> {
        if self.role != expected_role {
            return Err(format!(
                "cohort {} has role {}, expected {}",
                self.id,
                self.role.as_str(),
                expected_role.as_str()
            ));
        }
        let rebuilt = Self::new(&self.id, expected_role, cases, config)?;
        if rebuilt.manifest_sha256 != self.manifest_sha256 {
            return Err(format!(
                "cohort {} manifest drift: expected {}, rebuilt {}",
                self.id, self.manifest_sha256, rebuilt.manifest_sha256
            ));
        }
        Ok(())
    }

    pub fn validate_with_evaluators(
        &self,
        expected_role: CohortRole,
        cases: &[HeldoutCase<'_>],
        config: &PromotionConfig,
        evaluators: &[&dyn PolicyEvaluator],
    ) -> Result<(), String> {
        if self.role != expected_role {
            return Err(format!(
                "cohort {} has role {}, expected {}",
                self.id,
                self.role.as_str(),
                expected_role.as_str()
            ));
        }
        let rebuilt =
            Self::new_with_evaluators(&self.id, expected_role, cases, config, evaluators)?;
        if rebuilt.manifest_sha256 != self.manifest_sha256 {
            return Err(format!(
                "cohort {} evaluator manifest drift: expected {}, rebuilt {}",
                self.id, self.manifest_sha256, rebuilt.manifest_sha256
            ));
        }
        Ok(())
    }
}

fn append_len_prefixed(buffer: &mut String, value: &str) {
    buffer.push_str(&value.len().to_string());
    buffer.push(':');
    buffer.push_str(value);
    buffer.push('\n');
}

fn validate_cases(cases: &[HeldoutCase<'_>]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::with_capacity(cases.len());
    for case in cases {
        if case.id.trim().is_empty() {
            return Err("promotion case id must not be empty".into());
        }
        if case.task.trim().is_empty() {
            return Err(format!("promotion case {} task must not be empty", case.id));
        }
        if case.reward.label().trim().is_empty() {
            return Err(format!(
                "promotion case {} reward label must not be empty",
                case.id
            ));
        }
        if !ids.insert(case.id) {
            return Err(format!("duplicate promotion case id: {}", case.id));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum PromotionDecision {
    Promoted,
    RejectedEmptyCohort,
    RejectedInsufficientCohort,
    RejectedIncomplete,
    RejectedBelowFloor,
    RejectedCaseRegression,
    RejectedInsufficientDelta,
}

impl PromotionDecision {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Promoted => "promoted",
            Self::RejectedEmptyCohort => "rejected-empty-cohort",
            Self::RejectedInsufficientCohort => "rejected-insufficient-cohort",
            Self::RejectedIncomplete => "rejected-incomplete",
            Self::RejectedBelowFloor => "rejected-below-floor",
            Self::RejectedCaseRegression => "rejected-case-regression",
            Self::RejectedInsufficientDelta => "rejected-insufficient-delta",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CasePromotionReport {
    pub id: String,
    pub requested_per_policy: usize,
    pub incumbent_observed: usize,
    pub candidate_observed: usize,
    pub incumbent_mean: Option<f32>,
    pub candidate_mean: Option<f32>,
    pub delta: Option<f32>,
    pub complete: bool,
    pub failure: Option<String>,
    pub incumbent_receipt_sha256s: Vec<String>,
    pub candidate_receipt_sha256s: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PromotionReport {
    pub cohort_role: CohortRole,
    pub cohort_manifest_sha256: String,
    pub incumbent_version: u64,
    pub candidate_version: u64,
    pub incumbent_prompt_sha256: String,
    pub candidate_prompt_sha256: String,
    pub decision: PromotionDecision,
    pub cases: Vec<CasePromotionReport>,
    pub mean_delta: Option<f32>,
    pub delta_lower_bound: Option<f32>,
    pub evaluator_receipt_sha256s: Vec<String>,
}

impl PromotionReport {
    pub fn promoted(&self) -> bool {
        self.decision == PromotionDecision::Promoted
    }

    pub fn approved_final_audit(&self) -> bool {
        self.cohort_role == CohortRole::FinalAudit && self.promoted()
    }
}

/// Publish a cohort's final verdict to the structural run telemetry. Display
/// only — no policy or task content leaves the report.
fn publish_verdict(report: &PromotionReport) {
    super::telemetry::verdict(super::telemetry::CohortVerdict {
        role: report.cohort_role.as_str().to_string(),
        decision: report.decision.label().to_string(),
        promoted: report.promoted(),
        mean_delta: report.mean_delta,
        delta_lower_bound: report.delta_lower_bound,
    });
}

#[derive(Debug)]
struct ScoreSeries {
    requested: usize,
    observed: usize,
    values: Vec<f32>,
    failure: Option<String>,
    receipt_sha256s: Vec<String>,
}

impl ScoreSeries {
    fn complete(&self) -> bool {
        self.failure.is_none() && self.observed == self.requested
    }

    fn mean(&self) -> Option<f32> {
        (!self.values.is_empty())
            .then(|| self.values.iter().sum::<f32>() / self.values.len() as f32)
    }

    fn sample_variance(&self) -> f32 {
        if self.values.len() < 2 {
            return 0.0;
        }
        let mean = self.mean().unwrap_or(0.0);
        self.values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f32>()
            / (self.values.len() - 1) as f32
    }
}

struct ScorePolicyRequest<'a, 'b> {
    candidates: Vec<super::Candidate>,
    prompt_sha256: &'a str,
    version: u64,
    case: &'a HeldoutCase<'a>,
    samples: usize,
    evaluator: Option<&'a dyn PolicyEvaluator>,
    cohort_manifest_sha256: &'a str,
    cohort_role: CohortRole,
    used_receipts: &'b mut std::collections::HashSet<String>,
    receipt_store: Option<ReceiptStoreContext<'a>>,
}

struct ExpectedInventoryReward<'a> {
    expected_sha256: &'a str,
}

impl Reward for ExpectedInventoryReward<'_> {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let evidence = input.evaluator_evidence(self.label())?;
        evidence.validate_for_scoring()?;
        if !evidence.succeeded() {
            return Err("evaluator inventory command failed".into());
        }
        if !evidence.stderr_is_empty() {
            return Err("evaluator inventory command wrote non-canonical stderr".into());
        }
        let observed = canonical_inventory(evidence.stdout_bytes())?;
        if observed.sha256 != self.expected_sha256 {
            return Err(format!(
                "evaluator observed inventory mismatch: expected {}, observed {observed}",
                self.expected_sha256,
                observed = observed.sha256,
            ));
        }
        Ok(1.0)
    }

    fn label(&self) -> &str {
        "evaluator-inventory/v1"
    }
}

struct PersistReceiptPairRequest<'a> {
    inventory: &'a super::EvaluatorEvidence,
    inventory_reward: &'a dyn Reward,
    inventory_expected: &'a super::consumption::ExpectedReceiptContext<'a>,
    outcome: &'a super::EvaluatorEvidence,
    outcome_reward: &'a dyn Reward,
    outcome_expected: &'a super::consumption::ExpectedReceiptContext<'a>,
    pair_count: &'a super::consumption::ReceiptPairCountContext<'a>,
    store: ReceiptStoreContext<'a>,
}

fn persist_and_consume_receipt_pair(
    request: PersistReceiptPairRequest<'_>,
) -> Result<super::consumption::ReceiptPairOutcome, String> {
    let PersistReceiptPairRequest {
        inventory,
        inventory_reward,
        inventory_expected,
        outcome,
        outcome_reward,
        outcome_expected,
        pair_count,
        store,
    } = request;
    // Both append-only artifacts must exist before the single authoritative
    // pair row is committed. A crash can therefore leave harmless unreferenced
    // artifacts, but never half of a counted logical sample.
    let inventory_artifact = inventory.persist_append_only(store.artifact_root)?;
    let outcome_artifact = outcome.persist_append_only(store.artifact_root)?;
    super::consumption::consume_evaluator_artifact_pair(
        super::consumption::ReceiptPairConsumptionRequest {
            inventory: super::consumption::EvaluatorArtifactScoreRequest {
                path: &inventory_artifact,
                reward: inventory_reward,
                expected: inventory_expected,
            },
            outcome: super::consumption::EvaluatorArtifactScoreRequest {
                path: &outcome_artifact,
                reward: outcome_reward,
                expected: outcome_expected,
            },
            count: pair_count,
            ledger_root: store.ledger_root,
        },
    )
}

fn score_policy(request: ScorePolicyRequest<'_, '_>) -> ScoreSeries {
    let ScorePolicyRequest {
        candidates,
        prompt_sha256,
        version,
        case,
        samples,
        evaluator,
        cohort_manifest_sha256,
        cohort_role,
        used_receipts,
        receipt_store,
    } = request;
    let observed = candidates.len();
    if observed != samples {
        return ScoreSeries {
            requested: samples,
            observed,
            values: Vec::new(),
            failure: Some(format!(
                "generator returned {observed} samples; exactly {samples} required"
            )),
            receipt_sha256s: Vec::new(),
        };
    }
    let mut values = Vec::with_capacity(samples);
    let mut receipt_sha256s = Vec::with_capacity(samples);
    let inventory_reward_contract_sha256 = reward_contract_sha256("evaluator-inventory/v1");
    let outcome_reward_contract_sha256 = reward_contract_sha256(case.reward.label());
    for (index, candidate) in candidates.into_iter().enumerate() {
        if candidate.policy_version != version {
            return ScoreSeries {
                requested: samples,
                observed,
                values,
                failure: Some(format!(
                    "sample {index} policy version {} != requested {version}",
                    candidate.policy_version
                )),
                receipt_sha256s,
            };
        }
        if candidate.output.trim().is_empty()
            || candidate
                .output
                .trim_start()
                .starts_with("(generation error:")
        {
            return ScoreSeries {
                requested: samples,
                observed,
                values,
                failure: Some(format!("sample {index} is a generation failure")),
                receipt_sha256s,
            };
        }
        if evaluator.is_some() {
            let hydrated =
                (|| -> Result<Option<super::consumption::ReceiptPairOutcome>, String> {
                    let store =
                        receipt_store.ok_or_else(|| "receipt store is required".to_string())?;
                    let evaluation_request = PolicyEvaluationRequest {
                        cohort_manifest_sha256,
                        cohort_role: cohort_role.as_str(),
                        case_id: case.id,
                        task: case.task,
                        prompt_sha256,
                        policy_version: version,
                        sample_index: index,
                        candidate: &candidate,
                    };
                    let outcome_subject_sha256 =
                        crate::cut::sha256_hex(evaluation_request.canonical_subject().as_bytes());
                    let inventory_subject_sha256 = crate::cut::sha256_hex(
                        evaluation_request.canonical_inventory_subject().as_bytes(),
                    );
                    let campaign_id = crate::cut::sha256_hex(
                        format!("{}\0{}", store.run_id, store.reproduction_id).as_bytes(),
                    );
                    let task_sha256 = crate::cut::sha256_hex(case.task.as_bytes());
                    let pair_count = super::consumption::ReceiptPairCountContext {
                        campaign_id: &campaign_id,
                        reproduction_id: store.reproduction_id,
                        phase: cohort_role.as_str(),
                        cohort_manifest_sha256,
                        case_id: case.id,
                        task_sha256: &task_sha256,
                        prompt_sha256,
                        inventory_reward_contract_sha256: &inventory_reward_contract_sha256,
                        outcome_reward_contract_sha256: &outcome_reward_contract_sha256,
                        policy_version: version,
                        sample_index: index,
                    };
                    if let Some(failure) = super::consumption::load_receipt_pair_poison(
                        store.ledger_root,
                        &pair_count,
                    )? {
                        return Err(format!(
                            "slot is frozen to prior evaluator failure: {failure}"
                        ));
                    }
                    super::consumption::load_receipt_pair(
                        store.ledger_root,
                        &pair_count,
                        &inventory_subject_sha256,
                        &outcome_subject_sha256,
                    )
                })();
            match hydrated {
                Ok(Some(pair)) => {
                    if pair.inventory_reward != 1.0 {
                        return ScoreSeries {
                            requested: samples,
                            observed,
                            values,
                            failure: Some(format!(
                                "sample {index} persisted inventory is incomplete"
                            )),
                            receipt_sha256s,
                        };
                    }
                    for receipt_sha256 in pair.receipt_sha256s {
                        if !used_receipts.insert(receipt_sha256.clone()) {
                            return ScoreSeries {
                                requested: samples,
                                observed,
                                values,
                                failure: Some(format!(
                                    "sample {index} reused a persisted evaluator receipt"
                                )),
                                receipt_sha256s,
                            };
                        }
                        receipt_sha256s.push(receipt_sha256);
                    }
                    values.push(pair.outcome_reward);
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    return ScoreSeries {
                        requested: samples,
                        observed,
                        values,
                        failure: Some(format!(
                            "sample {index} persisted evaluator slot failed: {error}"
                        )),
                        receipt_sha256s,
                    };
                }
            }
        }
        let mut receipt_reward = None;
        let evidence = if let Some(evaluator) = evaluator {
            let evaluated = (|| -> Result<(super::EvaluatorEvidence, f32), String> {
                evaluator.spec().validate_runtime()?;
                let evaluation_request = PolicyEvaluationRequest {
                    cohort_manifest_sha256,
                    cohort_role: cohort_role.as_str(),
                    case_id: case.id,
                    task: case.task,
                    prompt_sha256,
                    policy_version: version,
                    sample_index: index,
                    candidate: &candidate,
                };
                let outcome_subject_sha256 =
                    crate::cut::sha256_hex(evaluation_request.canonical_subject().as_bytes());
                let inventory_subject_sha256 = crate::cut::sha256_hex(
                    evaluation_request.canonical_inventory_subject().as_bytes(),
                );
                let PolicyEvaluationEvidence { inventory, outcome } =
                    evaluator.evaluate(&evaluation_request)?;
                evaluator.spec().validate_runtime()?;
                inventory.validate_for_scoring()?;
                outcome.validate_for_scoring()?;

                let outcome_contract_sha256 =
                    crate::cut::sha256_hex(evaluator.spec().outcome_contract().as_bytes());
                let inventory_contract = evaluator.spec().inventory_contract();
                let inventory_contract_sha256 =
                    crate::cut::sha256_hex(inventory_contract.as_bytes());
                let policy_sha256 = evaluator.spec().execution_policy_sha256();
                let outcome_identity_matches = outcome.command_sha256()
                    == evaluator.spec().command_sha256()
                    && outcome.execution_policy_sha256() == policy_sha256
                    && outcome.verifier_contract_sha256() == outcome_contract_sha256
                    && outcome.subject_sha256() == outcome_subject_sha256;
                let inventory_identity_matches = inventory.command_sha256()
                    == evaluator.spec().inventory_command_sha256()
                    && inventory.execution_policy_sha256() == policy_sha256
                    && inventory.verifier_contract_sha256() == inventory_contract_sha256
                    && inventory.subject_sha256() == inventory_subject_sha256;
                if !outcome_identity_matches || !inventory_identity_matches {
                    let mut drifted = Vec::new();
                    for (label, matches) in [
                        (
                            "outcome-command",
                            outcome.command_sha256() == evaluator.spec().command_sha256(),
                        ),
                        (
                            "outcome-execution-policy",
                            outcome.execution_policy_sha256() == policy_sha256,
                        ),
                        (
                            "outcome-verifier-contract",
                            outcome.verifier_contract_sha256() == outcome_contract_sha256,
                        ),
                        (
                            "outcome-subject",
                            outcome.subject_sha256() == outcome_subject_sha256,
                        ),
                        (
                            "inventory-command",
                            inventory.command_sha256()
                                == evaluator.spec().inventory_command_sha256(),
                        ),
                        (
                            "inventory-execution-policy",
                            inventory.execution_policy_sha256() == policy_sha256,
                        ),
                        (
                            "inventory-verifier-contract",
                            inventory.verifier_contract_sha256() == inventory_contract_sha256,
                        ),
                        (
                            "inventory-subject",
                            inventory.subject_sha256() == inventory_subject_sha256,
                        ),
                    ] {
                        if !matches {
                            drifted.push(label);
                        }
                    }
                    return Err(format!(
                        "evaluator receipt identity mismatch: {}",
                        drifted.join(", ")
                    ));
                }
                if inventory.workspace_path_sha256() != outcome.workspace_path_sha256()
                    || inventory.workspace_before_sha256() != outcome.workspace_before_sha256()
                {
                    return Err(
                        "inventory and outcome receipts do not share initial source state".into(),
                    );
                }
                if inventory.workspace_before_sha256() != evaluator.spec().fixture_sha256()
                    && evaluator.spec().kind() == super::evaluator::VerifierKind::LibtestInventory
                {
                    return Err(format!(
                        "evaluator physical fixture mismatch: expected {}, observed {}",
                        evaluator.spec().fixture_sha256(),
                        inventory.workspace_before_sha256()
                    ));
                }
                if !inventory.succeeded() || !inventory.stderr_is_empty() {
                    return Err("evaluator inventory process was not canonically successful".into());
                }
                let observed_inventory = canonical_inventory(inventory.stdout_bytes())?;
                if observed_inventory.sha256 != evaluator.spec().expected_inventory_sha256() {
                    return Err(format!(
                        "evaluator observed inventory mismatch: expected {}, observed {}",
                        evaluator.spec().expected_inventory_sha256(),
                        observed_inventory.sha256
                    ));
                }
                // A command-success case measures the operator's own process: its
                // exit status IS the verdict (a red objective is a measured
                // zero), and its stdout/stderr are ordinary verifier output. The
                // frozen fixture, the shared start state, the bound candidate
                // subject and the execution policy above still bind exactly what
                // was measured. A sealed libtest case keeps the canonical form
                // rules, where the outcome report carries the per-test verdict.
                if evaluator.spec().kind() == super::evaluator::VerifierKind::LibtestInventory {
                    if !outcome.succeeded() || !outcome.stderr_is_empty() {
                        return Err(
                            "evaluator outcome process was not canonically successful".into()
                        );
                    }
                    let observed_outcome = canonical_test_outcome(outcome.stdout_bytes())?;
                    if observed_outcome.inventory_sha256 != observed_inventory.sha256 {
                        return Err(format!(
                            "evaluator outcome inventory mismatch: inventory {}, outcome {}",
                            observed_inventory.sha256, observed_outcome.inventory_sha256
                        ));
                    }
                } else {
                    let _ = canonical_test_outcome; // canonical rows are not required here
                }
                let store = receipt_store.ok_or_else(|| "receipt store is required".to_string())?;
                let inventory_reward = ExpectedInventoryReward {
                    expected_sha256: evaluator.spec().expected_inventory_sha256(),
                };
                let inventory_expected = super::consumption::ExpectedReceiptContext {
                    subject_sha256: inventory.subject_sha256(),
                    command_sha256: evaluator.spec().inventory_command_sha256(),
                    verifier_contract_sha256: &inventory_contract_sha256,
                    execution_policy_sha256: policy_sha256,
                };
                let outcome_expected = super::consumption::ExpectedReceiptContext {
                    subject_sha256: outcome.subject_sha256(),
                    command_sha256: evaluator.spec().command_sha256(),
                    verifier_contract_sha256: &outcome_contract_sha256,
                    execution_policy_sha256: policy_sha256,
                };
                let pair_id = evaluation_request.canonical_pair_id();
                let campaign_id = crate::cut::sha256_hex(
                    format!("{}\0{}", store.run_id, store.reproduction_id).as_bytes(),
                );
                let task_sha256 = crate::cut::sha256_hex(case.task.as_bytes());
                let pair_count = super::consumption::ReceiptPairCountContext {
                    campaign_id: &campaign_id,
                    reproduction_id: store.reproduction_id,
                    phase: cohort_role.as_str(),
                    cohort_manifest_sha256,
                    case_id: case.id,
                    task_sha256: &task_sha256,
                    prompt_sha256,
                    inventory_reward_contract_sha256: &inventory_reward_contract_sha256,
                    outcome_reward_contract_sha256: &outcome_reward_contract_sha256,
                    policy_version: version,
                    sample_index: index,
                };
                if pair_id != super::consumption::canonical_receipt_pair_id(&pair_count) {
                    return Err("framework receipt pair identity drift".into());
                }
                let pair = persist_and_consume_receipt_pair(PersistReceiptPairRequest {
                    inventory: &inventory,
                    inventory_reward: &inventory_reward,
                    inventory_expected: &inventory_expected,
                    outcome: &outcome,
                    outcome_reward: case.reward,
                    outcome_expected: &outcome_expected,
                    pair_count: &pair_count,
                    store,
                })?;
                if pair.inventory_reward != 1.0 {
                    return Err("evaluator inventory did not produce a complete match".into());
                }
                for receipt_sha256 in pair.receipt_sha256s {
                    if !used_receipts.insert(receipt_sha256.clone()) {
                        return Err("reused an evaluator receipt".into());
                    }
                    receipt_sha256s.push(receipt_sha256);
                }
                Ok((outcome, pair.outcome_reward))
            })();
            match evaluated {
                Ok((outcome, reward)) => {
                    receipt_reward = Some(reward);
                    Some(outcome)
                }
                Err(error) => {
                    let store = receipt_store.expect("evaluator path requires receipt store");
                    let campaign_id = crate::cut::sha256_hex(
                        format!("{}\0{}", store.run_id, store.reproduction_id).as_bytes(),
                    );
                    let task_sha256 = crate::cut::sha256_hex(case.task.as_bytes());
                    let pair_count = super::consumption::ReceiptPairCountContext {
                        campaign_id: &campaign_id,
                        reproduction_id: store.reproduction_id,
                        phase: cohort_role.as_str(),
                        cohort_manifest_sha256,
                        case_id: case.id,
                        task_sha256: &task_sha256,
                        prompt_sha256,
                        inventory_reward_contract_sha256: &inventory_reward_contract_sha256,
                        outcome_reward_contract_sha256: &outcome_reward_contract_sha256,
                        policy_version: version,
                        sample_index: index,
                    };
                    let bounded_failure = if error.len() <= 16 * 1024 && !error.contains('\0') {
                        error.clone()
                    } else {
                        format!(
                            "oversized evaluator failure sha256={}",
                            crate::cut::sha256_hex(error.as_bytes())
                        )
                    };
                    let persisted_failure =
                        super::consumption::install_or_adopt_receipt_pair_poison(
                            store.ledger_root,
                            &pair_count,
                            &bounded_failure,
                        );
                    let failure = match persisted_failure {
                        Ok(failure) => format!(
                            "sample {index} persisted evaluator slot failed: slot is frozen to prior evaluator failure: {failure}"
                        ),
                        Err(poison_error) => format!(
                            "sample {index} evaluator failed: {bounded_failure}; poison persistence failed: {poison_error}"
                        ),
                    };
                    return ScoreSeries {
                        requested: samples,
                        observed,
                        values,
                        failure: Some(failure),
                        receipt_sha256s,
                    };
                }
            }
        } else {
            None
        };
        let input = evidence.as_ref().map_or_else(
            || RewardInput::CandidateOutput(&candidate.output),
            RewardInput::EvaluatorEvidence,
        );
        let value = match receipt_reward.map_or_else(|| case.reward.score(input), Ok) {
            Ok(value) if value.is_finite() => value,
            Ok(_) => {
                return ScoreSeries {
                    requested: samples,
                    observed,
                    values,
                    failure: Some(format!("sample {index} reward is non-finite")),
                    receipt_sha256s,
                };
            }
            Err(error) => {
                return ScoreSeries {
                    requested: samples,
                    observed,
                    values,
                    failure: Some(format!("sample {index} evaluator failed: {error}")),
                    receipt_sha256s,
                };
            }
        };
        values.push(value);
    }

    ScoreSeries {
        requested: samples,
        observed,
        values,
        failure: None,
        receipt_sha256s,
    }
}

/// Compare a proposed prompt to its incumbent on an equal-budget, complete
/// validation cohort. The reflector receives none of the scores or outputs.
pub(crate) fn evaluate_promotion(
    generator: &dyn Generator,
    incumbent_prompt: &str,
    candidate_prompt: &str,
    incumbent_version: u64,
    cases: &[HeldoutCase<'_>],
    config: &PromotionConfig,
    manifest: &CohortManifest,
) -> Result<PromotionReport, String> {
    manifest.validate(CohortRole::Promotion, cases, config)?;
    evaluate_cohort(
        generator,
        CohortEvaluationRequest {
            incumbent_prompt,
            candidate_prompt,
            incumbent_version,
            cases,
            config,
            cohort_role: CohortRole::Promotion,
            cohort_manifest_sha256: manifest.manifest_sha256(),
            evaluators: None,
            receipt_store: None,
        },
    )
}

pub struct ReceiptPromotionRequest<'a> {
    pub incumbent_prompt: &'a str,
    pub candidate_prompt: &'a str,
    pub incumbent_version: u64,
    pub cases: &'a [HeldoutCase<'a>],
    pub config: &'a PromotionConfig,
    pub manifest: &'a CohortManifest,
    pub evaluators: &'a [&'a dyn PolicyEvaluator],
    pub receipt_store: ReceiptStoreContext<'a>,
}

#[derive(Clone, Copy)]
pub struct ReceiptStoreContext<'a> {
    pub artifact_root: &'a Path,
    pub ledger_root: &'a Path,
    pub run_id: &'a str,
    pub reproduction_id: &'a str,
}

pub(crate) fn evaluate_promotion_with_receipts(
    generator: &dyn Generator,
    request: ReceiptPromotionRequest<'_>,
) -> Result<PromotionReport, String> {
    let ReceiptPromotionRequest {
        incumbent_prompt,
        candidate_prompt,
        incumbent_version,
        cases,
        config,
        manifest,
        evaluators,
        receipt_store,
    } = request;
    manifest.validate_with_evaluators(CohortRole::Promotion, cases, config, evaluators)?;
    evaluate_cohort(
        generator,
        CohortEvaluationRequest {
            incumbent_prompt,
            candidate_prompt,
            incumbent_version,
            cases,
            config,
            cohort_role: CohortRole::Promotion,
            cohort_manifest_sha256: manifest.manifest_sha256(),
            evaluators: Some(evaluators),
            receipt_store: Some(receipt_store),
        },
    )
}

/// Evaluate a frozen candidate on a manifest-distinct, disjoint final-audit
/// cohort. This API only returns evidence; it has no access to the mutable policy
/// prompt/version and therefore cannot promote a candidate.
pub struct FinalAuditRequest<'a> {
    pub incumbent_prompt: &'a str,
    pub frozen_candidate_prompt: &'a str,
    pub incumbent_version: u64,
    pub promotion_manifest: &'a CohortManifest,
    pub audit_cases: &'a [HeldoutCase<'a>],
    pub config: &'a PromotionConfig,
    pub audit_manifest: &'a CohortManifest,
}

pub(crate) fn evaluate_final_audit(
    generator: &dyn Generator,
    request: FinalAuditRequest<'_>,
) -> Result<PromotionReport, String> {
    let FinalAuditRequest {
        incumbent_prompt,
        frozen_candidate_prompt,
        incumbent_version,
        promotion_manifest,
        audit_cases,
        config,
        audit_manifest,
    } = request;
    audit_manifest.validate(CohortRole::FinalAudit, audit_cases, config)?;
    validate_audit_separation(promotion_manifest, audit_manifest)?;
    evaluate_cohort(
        generator,
        CohortEvaluationRequest {
            incumbent_prompt,
            candidate_prompt: frozen_candidate_prompt,
            incumbent_version,
            cases: audit_cases,
            config,
            cohort_role: CohortRole::FinalAudit,
            cohort_manifest_sha256: audit_manifest.manifest_sha256(),
            evaluators: None,
            receipt_store: None,
        },
    )
}

pub struct ReceiptFinalAuditRequest<'a> {
    pub incumbent_prompt: &'a str,
    pub frozen_candidate_prompt: &'a str,
    pub incumbent_version: u64,
    pub promotion_manifest: &'a CohortManifest,
    pub audit_cases: &'a [HeldoutCase<'a>],
    pub config: &'a PromotionConfig,
    pub audit_manifest: &'a CohortManifest,
    pub evaluators: &'a [&'a dyn PolicyEvaluator],
    pub receipt_store: ReceiptStoreContext<'a>,
}

pub(crate) fn evaluate_final_audit_with_receipts(
    generator: &dyn Generator,
    request: ReceiptFinalAuditRequest<'_>,
) -> Result<PromotionReport, String> {
    let ReceiptFinalAuditRequest {
        incumbent_prompt,
        frozen_candidate_prompt,
        incumbent_version,
        promotion_manifest,
        audit_cases,
        config,
        audit_manifest,
        evaluators,
        receipt_store,
    } = request;
    audit_manifest.validate_with_evaluators(
        CohortRole::FinalAudit,
        audit_cases,
        config,
        evaluators,
    )?;
    validate_audit_separation(promotion_manifest, audit_manifest)?;
    evaluate_cohort(
        generator,
        CohortEvaluationRequest {
            incumbent_prompt,
            candidate_prompt: frozen_candidate_prompt,
            incumbent_version,
            cases: audit_cases,
            config,
            cohort_role: CohortRole::FinalAudit,
            cohort_manifest_sha256: audit_manifest.manifest_sha256(),
            evaluators: Some(evaluators),
            receipt_store: Some(receipt_store),
        },
    )
}

pub(crate) fn validate_audit_separation(
    promotion_manifest: &CohortManifest,
    audit_manifest: &CohortManifest,
) -> Result<(), String> {
    if promotion_manifest.role != CohortRole::Promotion {
        return Err("final audit requires a promotion-role selection manifest".into());
    }
    if promotion_manifest.manifest_sha256 == audit_manifest.manifest_sha256 {
        return Err("final-audit manifest must differ from promotion manifest".into());
    }
    let promotion_ids = promotion_manifest
        .case_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    if let Some(overlap) = audit_manifest
        .case_ids
        .iter()
        .find(|id| promotion_ids.contains(id.as_str()))
    {
        return Err(format!(
            "final-audit case {overlap} overlaps the promotion cohort"
        ));
    }
    let promotion_tasks = promotion_manifest
        .task_sha256s
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    if let Some((index, _)) = audit_manifest
        .task_sha256s
        .iter()
        .enumerate()
        .find(|(_, task)| promotion_tasks.contains(task.as_str()))
    {
        return Err(format!(
            "final-audit case {} relabels a promotion task",
            audit_manifest.case_ids[index]
        ));
    }
    for (label, promotion, audit) in [
        (
            "evaluator manifest",
            &promotion_manifest.evaluator_manifest_sha256s,
            &audit_manifest.evaluator_manifest_sha256s,
        ),
        (
            "fixture",
            &promotion_manifest.evaluator_fixture_sha256s,
            &audit_manifest.evaluator_fixture_sha256s,
        ),
        (
            "verifier bundle",
            &promotion_manifest.evaluator_verifier_sha256s,
            &audit_manifest.evaluator_verifier_sha256s,
        ),
        (
            "expected inventory",
            &promotion_manifest.evaluator_inventory_sha256s,
            &audit_manifest.evaluator_inventory_sha256s,
        ),
    ] {
        let promotion_identities = promotion
            .iter()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>();
        if audit
            .iter()
            .any(|identity| promotion_identities.contains(identity.as_str()))
        {
            return Err(format!(
                "final-audit {label} identity overlaps promotion selection"
            ));
        }
    }
    Ok(())
}

struct CohortEvaluationRequest<'a> {
    incumbent_prompt: &'a str,
    candidate_prompt: &'a str,
    incumbent_version: u64,
    cases: &'a [HeldoutCase<'a>],
    config: &'a PromotionConfig,
    cohort_role: CohortRole,
    cohort_manifest_sha256: &'a str,
    evaluators: Option<&'a [&'a dyn PolicyEvaluator]>,
    receipt_store: Option<ReceiptStoreContext<'a>>,
}

const PHASE_PLAN_SCHEMA: &str = "angel.rlvr.complete-phase-plan/v2";
const PHASE_REPORT_SCHEMA: &str = "angel.rlvr.phase-report/v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FrozenPhaseSample {
    policy_version: u64,
    latency_secs: u64,
    latency_nanos: u32,
    output: String,
    output_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FrozenPhaseCase {
    id: String,
    task_sha256: String,
    incumbent: Vec<FrozenPhaseSample>,
    candidate: Vec<FrozenPhaseSample>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FrozenPhasePlan {
    schema: String,
    plan_id: String,
    campaign_id: String,
    cohort_manifest_sha256: String,
    cohort_role: String,
    incumbent_prompt_sha256: String,
    candidate_prompt_sha256: String,
    incumbent_version: u64,
    candidate_version: u64,
    samples_per_case: usize,
    promotion_math_contract_sha256: String,
    cases: Vec<FrozenPhaseCase>,
    record_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FrozenPhaseReport {
    schema: String,
    plan_id: String,
    campaign_id: String,
    cohort_manifest_sha256: String,
    cohort_role: String,
    promotion_math_contract_sha256: String,
    report: PromotionReport,
    record_sha256: String,
}

type GeneratedArmPair = (Vec<super::Candidate>, Vec<super::Candidate>);
type GeneratedPhaseBatches = Vec<GeneratedArmPair>;

impl FrozenPhaseSample {
    fn freeze(candidate: &super::Candidate) -> Self {
        Self {
            policy_version: candidate.policy_version,
            latency_secs: candidate.latency.as_secs(),
            latency_nanos: candidate.latency.subsec_nanos(),
            output: candidate.output.clone(),
            output_sha256: crate::cut::sha256_hex(candidate.output.as_bytes()),
        }
    }

    fn thaw(&self) -> Result<super::Candidate, String> {
        if self.latency_nanos >= 1_000_000_000
            || self.output_sha256 != crate::cut::sha256_hex(self.output.as_bytes())
        {
            return Err("frozen evaluation sample integrity mismatch".into());
        }
        Ok(super::Candidate {
            policy_version: self.policy_version,
            latency: std::time::Duration::new(self.latency_secs, self.latency_nanos),
            output: self.output.clone(),
        })
    }
}

fn phase_plan_sha256(plan: &FrozenPhasePlan) -> Result<String, String> {
    let mut unsigned = plan.clone();
    unsigned.record_sha256.clear();
    serde_json::to_vec(&unsigned)
        .map(|bytes| crate::cut::sha256_hex(&bytes))
        .map_err(|error| format!("could not hash complete evaluation plan: {error}"))
}

fn phase_report_sha256(report: &FrozenPhaseReport) -> Result<String, String> {
    let mut unsigned = report.clone();
    unsigned.record_sha256.clear();
    serde_json::to_vec(&unsigned)
        .map(|bytes| crate::cut::sha256_hex(&bytes))
        .map_err(|error| format!("could not hash phase report: {error}"))
}

fn persist_and_verify_phase_report(
    store: ReceiptStoreContext<'_>,
    plan_id: &str,
    campaign_id: &str,
    report: PromotionReport,
) -> Result<PromotionReport, String> {
    let mut record = FrozenPhaseReport {
        schema: PHASE_REPORT_SCHEMA.to_string(),
        plan_id: plan_id.to_string(),
        campaign_id: campaign_id.to_string(),
        cohort_manifest_sha256: report.cohort_manifest_sha256.clone(),
        cohort_role: report.cohort_role.as_str().to_string(),
        promotion_math_contract_sha256: promotion_math_contract_sha256(),
        report: report.clone(),
        record_sha256: String::new(),
    };
    record.record_sha256 = phase_report_sha256(&record)?;
    let mut proposed = serde_json::to_vec(&record)
        .map_err(|error| format!("could not encode phase report: {error}"))?;
    proposed.push(b'\n');
    let adopted = if let Some(existing) =
        super::consumption::load_phase_report(store.ledger_root, plan_id)?
    {
        existing
    } else {
        super::consumption::install_or_adopt_phase_report(store.ledger_root, plan_id, &proposed)?
    };
    let persisted: FrozenPhaseReport = serde_json::from_slice(&adopted)
        .map_err(|error| format!("could not decode phase report: {error}"))?;
    if persisted.schema != PHASE_REPORT_SCHEMA
        || persisted.plan_id != plan_id
        || persisted.campaign_id != campaign_id
        || persisted.cohort_manifest_sha256 != report.cohort_manifest_sha256
        || persisted.cohort_role != report.cohort_role.as_str()
        || persisted.promotion_math_contract_sha256 != promotion_math_contract_sha256()
        || persisted.record_sha256 != phase_report_sha256(&persisted)?
    {
        return Err("phase report authority context mismatch".into());
    }
    if persisted.report != report {
        return Err("phase report deterministic reconstruction mismatch".into());
    }
    Ok(persisted.report)
}

fn evaluate_cohort(
    generator: &dyn Generator,
    request: CohortEvaluationRequest<'_>,
) -> Result<PromotionReport, String> {
    let CohortEvaluationRequest {
        incumbent_prompt,
        candidate_prompt,
        incumbent_version,
        cases,
        config,
        cohort_role,
        cohort_manifest_sha256,
        evaluators,
        receipt_store,
    } = request;
    config.validate()?;
    let candidate_version = incumbent_version.saturating_add(1);
    let incumbent_prompt_sha256 = crate::cut::sha256_hex(incumbent_prompt.as_bytes());
    let candidate_prompt_sha256 = crate::cut::sha256_hex(candidate_prompt.as_bytes());
    if cases.is_empty() {
        let report = PromotionReport {
            cohort_role,
            cohort_manifest_sha256: cohort_manifest_sha256.to_string(),
            incumbent_version,
            candidate_version,
            incumbent_prompt_sha256,
            candidate_prompt_sha256,
            decision: PromotionDecision::RejectedEmptyCohort,
            cases: Vec::new(),
            mean_delta: None,
            delta_lower_bound: None,
            evaluator_receipt_sha256s: Vec::new(),
        };
        publish_verdict(&report);
        return Ok(report);
    }
    if cases.len() < config.min_cases {
        let report = PromotionReport {
            cohort_role,
            cohort_manifest_sha256: cohort_manifest_sha256.to_string(),
            incumbent_version,
            candidate_version,
            incumbent_prompt_sha256,
            candidate_prompt_sha256,
            decision: PromotionDecision::RejectedInsufficientCohort,
            cases: Vec::new(),
            mean_delta: None,
            delta_lower_bound: None,
            evaluator_receipt_sha256s: Vec::new(),
        };
        publish_verdict(&report);
        return Ok(report);
    }
    validate_cases(cases)?;
    super::telemetry::cohort_begin(cohort_role.as_str(), cases.len(), config.samples_per_case);

    let receipt_plan = if evaluators.is_some() {
        let store = receipt_store.ok_or_else(|| "receipt store is required".to_string())?;
        let campaign_id = crate::cut::sha256_hex(
            format!("{}\0{}", store.run_id, store.reproduction_id).as_bytes(),
        );
        let mut identity = String::new();
        for value in [
            campaign_id.clone(),
            cohort_manifest_sha256.to_string(),
            cohort_role.as_str().to_string(),
            incumbent_prompt_sha256.clone(),
            candidate_prompt_sha256.clone(),
            incumbent_version.to_string(),
            candidate_version.to_string(),
            config.samples_per_case.to_string(),
            promotion_math_contract_sha256(),
        ] {
            identity.push_str(&value.len().to_string());
            identity.push(':');
            identity.push_str(&value);
            identity.push('\n');
        }
        let plan_id = crate::cut::sha256_hex(identity.as_bytes());
        Some((store, campaign_id, plan_id))
    } else {
        None
    };

    let generate_batches = || {
        cases
            .iter()
            .enumerate()
            .map(|(case_index, case)| {
                if case_index % 2 == 0 {
                    let incumbent = generator.generate(
                        incumbent_prompt,
                        case.task,
                        incumbent_version,
                        config.samples_per_case,
                    );
                    let candidate = generator.generate(
                        candidate_prompt,
                        case.task,
                        candidate_version,
                        config.samples_per_case,
                    );
                    (incumbent, candidate)
                } else {
                    let candidate = generator.generate(
                        candidate_prompt,
                        case.task,
                        candidate_version,
                        config.samples_per_case,
                    );
                    let incumbent = generator.generate(
                        incumbent_prompt,
                        case.task,
                        incumbent_version,
                        config.samples_per_case,
                    );
                    (incumbent, candidate)
                }
            })
            .collect::<Vec<_>>()
    };

    // Freeze full outputs for the complete phase before any evaluator runs.
    // A resumed campaign therefore reuses the exact stochastic generation
    // cohort instead of recalling the provider or rerolling private evidence.
    let generated_batches = if let Some((store, campaign_id, plan_id)) = receipt_plan.as_ref() {
        let decode_plan = |bytes: &[u8]| -> Result<GeneratedPhaseBatches, String> {
            let plan: FrozenPhasePlan = serde_json::from_slice(bytes)
                .map_err(|error| format!("could not decode complete evaluation plan: {error}"))?;
            if plan.schema != PHASE_PLAN_SCHEMA
                || plan.plan_id != plan_id.as_str()
                || plan.campaign_id != campaign_id.as_str()
                || plan.cohort_manifest_sha256 != cohort_manifest_sha256
                || plan.cohort_role != cohort_role.as_str()
                || plan.incumbent_prompt_sha256 != incumbent_prompt_sha256
                || plan.candidate_prompt_sha256 != candidate_prompt_sha256
                || plan.incumbent_version != incumbent_version
                || plan.candidate_version != candidate_version
                || plan.samples_per_case != config.samples_per_case
                || plan.promotion_math_contract_sha256 != promotion_math_contract_sha256()
                || plan.cases.len() != cases.len()
                || plan.record_sha256 != phase_plan_sha256(&plan)?
            {
                return Err("complete evaluation plan authority context mismatch".into());
            }
            plan.cases
                .iter()
                .zip(cases)
                .map(|(frozen_case, case)| {
                    if frozen_case.id != case.id
                        || frozen_case.task_sha256 != crate::cut::sha256_hex(case.task.as_bytes())
                    {
                        return Err("complete evaluation plan case mismatch".into());
                    }
                    Ok((
                        frozen_case
                            .incumbent
                            .iter()
                            .map(FrozenPhaseSample::thaw)
                            .collect::<Result<Vec<_>, _>>()?,
                        frozen_case
                            .candidate
                            .iter()
                            .map(FrozenPhaseSample::thaw)
                            .collect::<Result<Vec<_>, _>>()?,
                    ))
                })
                .collect()
        };

        if let Some(existing) =
            super::consumption::load_evaluation_plan(store.ledger_root, plan_id)?
        {
            decode_plan(&existing)?
        } else {
            let proposed_batches = generate_batches();
            let mut plan = FrozenPhasePlan {
                schema: PHASE_PLAN_SCHEMA.to_string(),
                plan_id: plan_id.clone(),
                campaign_id: campaign_id.clone(),
                cohort_manifest_sha256: cohort_manifest_sha256.to_string(),
                cohort_role: cohort_role.as_str().to_string(),
                incumbent_prompt_sha256: incumbent_prompt_sha256.clone(),
                candidate_prompt_sha256: candidate_prompt_sha256.clone(),
                incumbent_version,
                candidate_version,
                samples_per_case: config.samples_per_case,
                promotion_math_contract_sha256: promotion_math_contract_sha256(),
                cases: cases
                    .iter()
                    .zip(&proposed_batches)
                    .map(|(case, (incumbent, candidate))| FrozenPhaseCase {
                        id: case.id.to_string(),
                        task_sha256: crate::cut::sha256_hex(case.task.as_bytes()),
                        incumbent: incumbent.iter().map(FrozenPhaseSample::freeze).collect(),
                        candidate: candidate.iter().map(FrozenPhaseSample::freeze).collect(),
                    })
                    .collect(),
                record_sha256: String::new(),
            };
            plan.record_sha256 = phase_plan_sha256(&plan)?;
            let mut bytes = serde_json::to_vec(&plan)
                .map_err(|error| format!("could not encode complete evaluation plan: {error}"))?;
            bytes.push(b'\n');
            let adopted = super::consumption::install_or_adopt_evaluation_plan(
                store.ledger_root,
                plan_id,
                &bytes,
            )?;
            decode_plan(&adopted)?
        }
    } else {
        generate_batches()
    };

    let mut reports = Vec::with_capacity(cases.len());
    let mut deltas = Vec::with_capacity(cases.len());
    let mut delta_variances = Vec::with_capacity(cases.len());
    let mut used_receipts = std::collections::HashSet::new();
    let mut evaluator_receipt_sha256s = Vec::new();

    for (case_index, (case, (incumbent_candidates, candidate_candidates))) in
        cases.iter().zip(generated_batches).enumerate()
    {
        // Balance evaluation order across cases so monotonic backend drift does
        // not always favor the candidate (or always favor the incumbent).
        let evaluator = evaluators.map(|items| items[case_index]);
        let mut score = |prompt_sha256: &str, version: u64, candidates: Vec<super::Candidate>| {
            score_policy(ScorePolicyRequest {
                candidates,
                prompt_sha256,
                version,
                case,
                samples: config.samples_per_case,
                evaluator,
                cohort_manifest_sha256,
                cohort_role,
                used_receipts: &mut used_receipts,
                receipt_store,
            })
        };
        let (incumbent, candidate) = if case_index % 2 == 0 {
            let incumbent = score(
                &incumbent_prompt_sha256,
                incumbent_version,
                incumbent_candidates,
            );
            let candidate = score(
                &candidate_prompt_sha256,
                candidate_version,
                candidate_candidates,
            );
            (incumbent, candidate)
        } else {
            let candidate = score(
                &candidate_prompt_sha256,
                candidate_version,
                candidate_candidates,
            );
            let incumbent = score(
                &incumbent_prompt_sha256,
                incumbent_version,
                incumbent_candidates,
            );
            (incumbent, candidate)
        };
        let complete = incumbent.complete() && candidate.complete();
        let incumbent_mean = incumbent.mean();
        let candidate_mean = candidate.mean();
        let delta = complete.then(|| candidate_mean.unwrap() - incumbent_mean.unwrap());
        if let Some(delta) = delta {
            deltas.push(delta);
            let n = config.samples_per_case as f32;
            delta_variances.push(incumbent.sample_variance() / n + candidate.sample_variance() / n);
        }
        let failure = incumbent
            .failure
            .as_deref()
            .map(|message| format!("incumbent: {message}"))
            .or_else(|| {
                candidate
                    .failure
                    .as_deref()
                    .map(|message| format!("candidate: {message}"))
            });
        evaluator_receipt_sha256s.extend(incumbent.receipt_sha256s.iter().cloned());
        evaluator_receipt_sha256s.extend(candidate.receipt_sha256s.iter().cloned());
        super::telemetry::case_slot(super::telemetry::CaseSlot {
            id: case.id.to_string(),
            requested_per_policy: config.samples_per_case,
            incumbent_observed: incumbent.observed,
            candidate_observed: candidate.observed,
            delta,
            complete,
            failed: failure.is_some(),
        });
        reports.push(CasePromotionReport {
            id: case.id.to_string(),
            requested_per_policy: config.samples_per_case,
            incumbent_observed: incumbent.observed,
            candidate_observed: candidate.observed,
            incumbent_mean,
            candidate_mean,
            delta,
            complete,
            failure,
            incumbent_receipt_sha256s: incumbent.receipt_sha256s,
            candidate_receipt_sha256s: candidate.receipt_sha256s,
        });
    }

    let report = if reports.iter().any(|report| !report.complete) {
        PromotionReport {
            cohort_role,
            cohort_manifest_sha256: cohort_manifest_sha256.to_string(),
            incumbent_version,
            candidate_version,
            incumbent_prompt_sha256,
            candidate_prompt_sha256,
            decision: PromotionDecision::RejectedIncomplete,
            cases: reports,
            mean_delta: None,
            delta_lower_bound: None,
            evaluator_receipt_sha256s,
        }
    } else {
        let mean_delta = deltas.iter().sum::<f32>() / deltas.len() as f32;
        let mean_delta_variance = delta_variances.iter().sum::<f32>() / deltas.len().pow(2) as f32;
        let lower_bound = mean_delta - config.confidence_z * mean_delta_variance.sqrt();

        let decision = if reports
            .iter()
            .any(|report| report.candidate_mean.unwrap() < config.absolute_floor)
        {
            PromotionDecision::RejectedBelowFloor
        } else if reports
            .iter()
            .any(|report| report.delta.unwrap() < -config.max_case_regression)
        {
            PromotionDecision::RejectedCaseRegression
        } else if lower_bound < config.min_mean_delta {
            PromotionDecision::RejectedInsufficientDelta
        } else {
            PromotionDecision::Promoted
        };

        PromotionReport {
            cohort_role,
            cohort_manifest_sha256: cohort_manifest_sha256.to_string(),
            incumbent_version,
            candidate_version,
            incumbent_prompt_sha256,
            candidate_prompt_sha256,
            decision,
            cases: reports,
            mean_delta: Some(mean_delta),
            delta_lower_bound: Some(lower_bound),
            evaluator_receipt_sha256s,
        }
    };
    publish_verdict(&report);
    if let Some((store, campaign_id, plan_id)) = receipt_plan {
        persist_and_verify_phase_report(store, &plan_id, &campaign_id, report)
    } else {
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reinforce::{Candidate, EvaluatorEvidence, TestReward};
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
    impl crate::reinforce::Reflector for MustNotReflect {
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

    impl crate::reinforce::Reflector for CandidateReflector {
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
            let expected_inventory_sha256 = crate::cut::sha256_hex(expected_inventory.as_bytes());
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
            let evaluated_output = if self.reject_final_audit
                && request.cohort_role == CohortRole::FinalAudit.as_str()
            {
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
        let reduced_evaluators: [&dyn PolicyEvaluator; 2] =
            [&reduced_inventory, &reduced_inventory];
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
        let underexecuting_evaluators: [&dyn PolicyEvaluator; 2] =
            [&underexecuting, &underexecuting];
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
            case.failure.as_deref().is_some_and(|failure| {
                failure.contains("slot is frozen to prior evaluator failure")
            })
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
        let authority =
            crate::reinforce::TechnicalCampaignAuthority::new_in("resume-stages-run", root.clone())
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
        let training_config = crate::reinforce::ReinforceConfig {
            group_size: 4,
            oversample: 0.0,
            staleness_budget: 8,
            straggler_factor: 100.0,
            success_threshold: 0.5,
        };
        let run = |generator: &dyn Generator,
                   reward: &dyn Reward,
                   reflector: &dyn crate::reinforce::Reflector| {
            crate::reinforce::run_reinforce(
                generator,
                reward,
                reflector,
                crate::reinforce::ReinforceRequest {
                    task: "training",
                    initial_prompt: "incumbent",
                    rounds: 1,
                    config: &training_config,
                },
                crate::reinforce::TechnicalReinforceCampaign {
                    authority: &authority,
                    promotion: crate::reinforce::TechnicalPromotionCohort {
                        cases: &selection_cases,
                        config: &selection_config,
                        manifest: &selection_manifest,
                        evaluators: &selection_evaluators,
                    },
                    final_audit: crate::reinforce::TechnicalFinalAuditCohort {
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
        let authority = crate::reinforce::TechnicalCampaignAuthority::new_in(
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
        let training_config = crate::reinforce::ReinforceConfig {
            group_size: 4,
            oversample: 0.0,
            staleness_budget: 8,
            straggler_factor: 100.0,
            success_threshold: 0.5,
        };
        let run_campaign = |task| {
            crate::reinforce::run_reinforce(
                &TechnicalCampaignGenerator,
                &NumericReward,
                &CandidateReflector,
                crate::reinforce::ReinforceRequest {
                    task,
                    initial_prompt: "incumbent",
                    rounds: 1,
                    config: &training_config,
                },
                crate::reinforce::TechnicalReinforceCampaign {
                    authority: &authority,
                    promotion: crate::reinforce::TechnicalPromotionCohort {
                        cases: &promotion_cases,
                        config: &promotion_config,
                        manifest: &promotion_manifest,
                        evaluators: &evaluators,
                    },
                    final_audit: crate::reinforce::TechnicalFinalAuditCohort {
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
        let sealed = crate::reinforce::run_reinforce(
            &MustNotGenerate,
            &MustNotScore,
            &MustNotReflect,
            crate::reinforce::ReinforceRequest {
                task: "training",
                initial_prompt: "incumbent",
                rounds: 1,
                config: &training_config,
            },
            crate::reinforce::TechnicalReinforceCampaign {
                authority: &authority,
                promotion: crate::reinforce::TechnicalPromotionCohort {
                    cases: &promotion_cases,
                    config: &promotion_config,
                    manifest: &promotion_manifest,
                    evaluators: &panic_selection_evaluators,
                },
                final_audit: crate::reinforce::TechnicalFinalAuditCohort {
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
            crate::reinforce::TechnicalCampaignAuthority::new_in("veto-run", veto_root).unwrap();
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
        let veto_report = crate::reinforce::run_reinforce(
            &TechnicalCampaignGenerator,
            &NumericReward,
            &CandidateReflector,
            crate::reinforce::ReinforceRequest {
                task: "training",
                initial_prompt: "incumbent",
                rounds: 1,
                config: &training_config,
            },
            crate::reinforce::TechnicalReinforceCampaign {
                authority: &veto_authority,
                promotion: crate::reinforce::TechnicalPromotionCohort {
                    cases: &promotion_cases,
                    config: &promotion_config,
                    manifest: &veto_promotion_manifest,
                    evaluators: &veto_selection_evaluators,
                },
                final_audit: crate::reinforce::TechnicalFinalAuditCohort {
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
            case.failure.as_deref().is_some_and(|failure| {
                failure.contains("requires evaluator-owned command evidence")
            })
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
            crate::cut::sha256_hex(b"candidate")
        );
        assert_eq!(report.incumbent_version, 0);
        assert_eq!(report.candidate_version, 1);
        assert_eq!(report.decision, PromotionDecision::Promoted);
    }
}
