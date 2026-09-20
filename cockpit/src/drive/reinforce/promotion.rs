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
    crate::knowledge::cut::sha256_hex(
        format!("{REWARD_SCORING_CONTRACT_SCHEMA}\0{label}").as_bytes(),
    )
}

pub(crate) fn promotion_math_contract_sha256() -> String {
    crate::knowledge::cut::sha256_hex(PROMOTION_MATH_CONTRACT_SCHEMA.as_bytes())
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
            task_sha256s.push(crate::knowledge::cut::sha256_hex(case.task.as_bytes()));
        }
        let cases_sha256 = crate::knowledge::cut::sha256_hex(canonical_cases.as_bytes());
        let canonical_config = format!(
            "min_cases={};samples_per_case={};absolute_floor={:08x};min_mean_delta={:08x};max_case_regression={:08x};confidence_z={:08x}",
            config.min_cases,
            config.samples_per_case,
            config.absolute_floor.to_bits(),
            config.min_mean_delta.to_bits(),
            config.max_case_regression.to_bits(),
            config.confidence_z.to_bits(),
        );
        let config_sha256 = crate::knowledge::cut::sha256_hex(canonical_config.as_bytes());
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
        let manifest_sha256 = crate::knowledge::cut::sha256_hex(canonical_manifest.as_bytes());
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
                    let outcome_subject_sha256 = crate::knowledge::cut::sha256_hex(
                        evaluation_request.canonical_subject().as_bytes(),
                    );
                    let inventory_subject_sha256 = crate::knowledge::cut::sha256_hex(
                        evaluation_request.canonical_inventory_subject().as_bytes(),
                    );
                    let campaign_id = crate::knowledge::cut::sha256_hex(
                        format!("{}\0{}", store.run_id, store.reproduction_id).as_bytes(),
                    );
                    let task_sha256 = crate::knowledge::cut::sha256_hex(case.task.as_bytes());
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
                let outcome_subject_sha256 = crate::knowledge::cut::sha256_hex(
                    evaluation_request.canonical_subject().as_bytes(),
                );
                let inventory_subject_sha256 = crate::knowledge::cut::sha256_hex(
                    evaluation_request.canonical_inventory_subject().as_bytes(),
                );
                let PolicyEvaluationEvidence { inventory, outcome } =
                    evaluator.evaluate(&evaluation_request)?;
                evaluator.spec().validate_runtime()?;
                inventory.validate_for_scoring()?;
                outcome.validate_for_scoring()?;

                let outcome_contract_sha256 = crate::knowledge::cut::sha256_hex(
                    evaluator.spec().outcome_contract().as_bytes(),
                );
                let inventory_contract = evaluator.spec().inventory_contract();
                let inventory_contract_sha256 =
                    crate::knowledge::cut::sha256_hex(inventory_contract.as_bytes());
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
                let campaign_id = crate::knowledge::cut::sha256_hex(
                    format!("{}\0{}", store.run_id, store.reproduction_id).as_bytes(),
                );
                let task_sha256 = crate::knowledge::cut::sha256_hex(case.task.as_bytes());
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
                    let campaign_id = crate::knowledge::cut::sha256_hex(
                        format!("{}\0{}", store.run_id, store.reproduction_id).as_bytes(),
                    );
                    let task_sha256 = crate::knowledge::cut::sha256_hex(case.task.as_bytes());
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
                            crate::knowledge::cut::sha256_hex(error.as_bytes())
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
            output_sha256: crate::knowledge::cut::sha256_hex(candidate.output.as_bytes()),
        }
    }

    fn thaw(&self) -> Result<super::Candidate, String> {
        if self.latency_nanos >= 1_000_000_000
            || self.output_sha256 != crate::knowledge::cut::sha256_hex(self.output.as_bytes())
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
        .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
        .map_err(|error| format!("could not hash complete evaluation plan: {error}"))
}

fn phase_report_sha256(report: &FrozenPhaseReport) -> Result<String, String> {
    let mut unsigned = report.clone();
    unsigned.record_sha256.clear();
    serde_json::to_vec(&unsigned)
        .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
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
    let incumbent_prompt_sha256 = crate::knowledge::cut::sha256_hex(incumbent_prompt.as_bytes());
    let candidate_prompt_sha256 = crate::knowledge::cut::sha256_hex(candidate_prompt.as_bytes());
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
        let campaign_id = crate::knowledge::cut::sha256_hex(
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
        let plan_id = crate::knowledge::cut::sha256_hex(identity.as_bytes());
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
                        || frozen_case.task_sha256
                            != crate::knowledge::cut::sha256_hex(case.task.as_bytes())
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
                        task_sha256: crate::knowledge::cut::sha256_hex(case.task.as_bytes()),
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
#[path = "../../../../tests/cockpit/reinforce/promotion__tests.rs"]
mod tests;
