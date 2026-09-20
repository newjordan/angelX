use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};

pub(crate) const CAMPAIGN_SCHEMA: &str = "angel.campaign/v1";
pub(crate) const ROUND_SCHEMA: &str = "angel.campaign-round/v1";
pub(crate) const MAX_RECORD_BYTES: u64 = 1024 * 1024;
pub(crate) const MAX_OBJECTIVE_BYTES: usize = 16 * 1024;
pub(crate) const MAX_CRITERION_BYTES: usize = 4 * 1024;
pub(crate) const MAX_COMMAND_BYTES: usize = 8 * 1024;
pub(crate) const MAX_PATH_BYTES: usize = 1024;
pub(crate) const MAX_CRITERIA: usize = 64;
pub(crate) const MAX_VERIFIERS_PER_CRITERION: usize = 8;
pub(crate) const MAX_SCOPE_PER_CRITERION: usize = 64;
pub(crate) const MAX_PROOFS_PER_CRITERION: usize = 32;
pub(crate) const MAX_QUALITY_COMMANDS: usize = 16;
pub(crate) const MAX_CHANGED_PATHS: usize = 256;
pub(crate) const MAX_PROOFS_PER_ROUND: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectBinding {
    pub(crate) canonical_root: PathBuf,
    pub(crate) project_key: String,
    pub(crate) workspace_rel: PathBuf,
}

impl ProjectBinding {
    pub(crate) fn for_workspace(workspace: &Path) -> Self {
        let identity = crate::platform::workspace_store::repo_identity(workspace);
        let workspace_rel = workspace
            .strip_prefix(&identity.root)
            .ok()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self {
            canonical_root: identity.root,
            project_key: identity.key,
            workspace_rel,
        }
    }

    pub(crate) fn matches(&self, workspace: &Path) -> bool {
        crate::platform::workspace_store::matches_project(
            workspace,
            &self.canonical_root,
            &self.project_key,
        ) && valid_relative_path(&self.workspace_rel)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CampaignStatus {
    Draft,
    Ready,
    Running,
    VerifyingRound,
    ReviewingRound,
    FinalVerifying,
    AwaitingOperator,
    Paused,
    ReadyToIntegrate,
    Integrated,
    Stopped,
    Failed,
}

impl CampaignStatus {
    pub(crate) fn is_running_like(self) -> bool {
        matches!(
            self,
            Self::Running
                | Self::VerifyingRound
                | Self::ReviewingRound
                | Self::FinalVerifying
                | Self::AwaitingOperator
        )
    }

    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::Integrated | Self::Stopped | Self::Failed)
    }

    pub(crate) fn allows(self, next: Self) -> bool {
        use CampaignStatus as S;
        matches!(
            (self, next),
            (S::Draft, S::Ready | S::Stopped | S::Failed)
                | (S::Ready, S::Running | S::Paused | S::Stopped | S::Failed)
                | (
                    S::Running,
                    S::VerifyingRound | S::FinalVerifying | S::Paused | S::Stopped | S::Failed
                )
                | (
                    S::VerifyingRound,
                    S::ReviewingRound | S::Running | S::Paused | S::Stopped | S::Failed
                )
                | (
                    S::ReviewingRound,
                    S::Ready
                        | S::Running
                        | S::AwaitingOperator
                        | S::Paused
                        | S::ReadyToIntegrate
                        | S::Stopped
                        | S::Failed
                )
                | (
                    S::FinalVerifying,
                    S::ReadyToIntegrate | S::Paused | S::Stopped | S::Failed
                )
                | (
                    S::AwaitingOperator,
                    S::Running | S::ReadyToIntegrate | S::Paused | S::Stopped | S::Failed
                )
                | (
                    S::Paused,
                    S::Ready
                        | S::Running
                        | S::ReviewingRound
                        | S::AwaitingOperator
                        | S::ReadyToIntegrate
                        | S::Stopped
                        | S::Failed
                )
                | (
                    S::ReadyToIntegrate,
                    S::Integrated | S::Paused | S::Stopped | S::Failed
                )
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CriterionId(u16);

impl CriterionId {
    pub(crate) fn new(value: u16) -> Result<Self, String> {
        if value == 0 || usize::from(value) > MAX_CRITERIA {
            return Err(format!("criterion id must be AC-1..AC-{MAX_CRITERIA}"));
        }
        Ok(Self(value))
    }

    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        let normalized = raw.trim().to_ascii_uppercase();
        let number = normalized
            .strip_prefix("AC-")
            .ok_or_else(|| format!("invalid criterion id {raw:?}; expected AC-N"))?
            .parse::<u16>()
            .map_err(|_| format!("invalid criterion id {raw:?}; expected AC-N"))?;
        Self::new(number)
    }
}

impl fmt::Display for CriterionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AC-{}", self.0)
    }
}

impl Serialize for CriterionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for CriterionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CriterionStatus {
    Pending,
    Active,
    Blocked,
    TechnicallyVerified,
    Verified,
    Deferred,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NetworkPolicy {
    Inherited,
    Offline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VerifierKind {
    Command,
    Aggregate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerifierSpec {
    pub(crate) id: String,
    pub(crate) kind: VerifierKind,
    pub(crate) command: String,
    pub(crate) network: NetworkPolicy,
    pub(crate) timeout_secs: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProofRef {
    pub(crate) kind: ProofKind,
    pub(crate) run_id: String,
    pub(crate) artifact_path: PathBuf,
    pub(crate) sha256: String,
    pub(crate) git_oid: String,
    pub(crate) summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProofKind {
    Baseline,
    RedTest,
    TargetedTest,
    FullTest,
    Quality,
    CodeReview,
    AlignmentReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OperatorWaiver {
    pub(crate) reason: String,
    pub(crate) authored_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AcceptanceCriterion {
    pub(crate) id: CriterionId,
    pub(crate) text: String,
    pub(crate) required: bool,
    pub(crate) status: CriterionStatus,
    pub(crate) verifiers: Vec<VerifierSpec>,
    pub(crate) test_scope: Vec<PathBuf>,
    pub(crate) proof_refs: Vec<ProofRef>,
    pub(crate) waiver: Option<OperatorWaiver>,
}

impl AcceptanceCriterion {
    pub(crate) fn pending(id: CriterionId, text: String) -> Self {
        Self {
            id,
            text,
            required: true,
            status: CriterionStatus::Pending,
            verifiers: Vec::new(),
            test_scope: Vec::new(),
            proof_refs: Vec::new(),
            waiver: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CampaignPolicy {
    pub(crate) max_rounds: u32,
    pub(crate) token_budget: u64,
    pub(crate) deadline_secs: u64,
    pub(crate) stall_rounds: u32,
    pub(crate) full_alignment_interval: u32,
    pub(crate) max_target_criteria_per_round: u8,
    pub(crate) require_independent_reviewer: bool,
    pub(crate) default_network_policy: NetworkPolicy,
}

impl Default for CampaignPolicy {
    fn default() -> Self {
        Self {
            max_rounds: 0,
            token_budget: 0,
            deadline_secs: 0,
            stall_rounds: 3,
            full_alignment_interval: 5,
            max_target_criteria_per_round: 2,
            require_independent_reviewer: true,
            default_network_policy: NetworkPolicy::Inherited,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RoundState {
    Planned,
    Compiling,
    TechnicallyVerified,
    AlignmentReviewing,
    Accepted,
    Blocked,
    Paused,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RoundContract {
    pub(crate) schema: String,
    pub(crate) campaign_id: String,
    pub(crate) campaign_revision: u64,
    pub(crate) round: u32,
    pub(crate) base_oid: String,
    pub(crate) objective: String,
    pub(crate) target_criteria: Vec<CriterionId>,
    #[serde(default)]
    pub(crate) targeted_test_cmd: String,
    pub(crate) accept_cmd: String,
    pub(crate) quality_cmds: Vec<String>,
    pub(crate) test_scope: Vec<PathBuf>,
    pub(crate) network_policy: NetworkPolicy,
    pub(crate) digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewReceipt {
    pub(crate) route: String,
    pub(crate) model_revision: String,
    pub(crate) independence: ReviewIndependence,
    pub(crate) verdict: ReviewVerdict,
    pub(crate) reviewed_contract_digest: String,
    pub(crate) reviewed_candidate_oid: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewIndependence {
    DifferentRouteAndRevision,
    DifferentRevision,
    SameRoute,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewVerdict {
    Pass,
    Block,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RoundReceipt {
    pub(crate) contract: RoundContract,
    pub(crate) state: RoundState,
    pub(crate) swarm_run_id: Option<String>,
    pub(crate) candidate_branch: Option<String>,
    pub(crate) candidate_oid: Option<String>,
    pub(crate) changed_paths: Vec<PathBuf>,
    pub(crate) proofs: Vec<ProofRef>,
    pub(crate) code_review: Option<ReviewReceipt>,
    pub(crate) alignment_review: Option<ReviewReceipt>,
    pub(crate) tokens: u64,
    pub(crate) elapsed_ms: u64,
    pub(crate) failure: Option<String>,
}

/// Deterministic scheduler output. Only campaign code can construct this from a
/// validated active contract; a future swarm adapter consumes it internally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExecutionRequest {
    pub(crate) campaign_id: String,
    pub(crate) campaign_revision: u64,
    pub(crate) contract_digest: String,
    pub(crate) base_oid: String,
    pub(crate) target_criteria: Vec<CriterionId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CampaignRecord {
    pub(crate) schema: String,
    pub(crate) id: String,
    pub(crate) revision: u64,
    pub(crate) project: ProjectBinding,
    pub(crate) status: CampaignStatus,
    pub(crate) paused_from: Option<CampaignStatus>,
    pub(crate) objective: String,
    pub(crate) objective_digest: String,
    pub(crate) criteria: Vec<AcceptanceCriterion>,
    pub(crate) policy: CampaignPolicy,
    pub(crate) frozen_base_oid: Option<String>,
    pub(crate) campaign_ref: Option<String>,
    pub(crate) campaign_head_oid: Option<String>,
    pub(crate) active_contract: Option<RoundContract>,
    pub(crate) rounds: Vec<RoundReceipt>,
    pub(crate) created_ms: u64,
    pub(crate) updated_ms: u64,
}

impl CampaignRecord {
    pub(crate) fn draft(id: String, workspace: &Path, objective: String, now_ms: u64) -> Self {
        Self {
            schema: CAMPAIGN_SCHEMA.to_string(),
            id,
            revision: 1,
            project: ProjectBinding::for_workspace(workspace),
            status: CampaignStatus::Draft,
            paused_from: None,
            objective_digest: digest_text(&objective),
            objective,
            criteria: Vec::new(),
            policy: CampaignPolicy::default(),
            frozen_base_oid: None,
            campaign_ref: None,
            campaign_head_oid: None,
            active_contract: None,
            rounds: Vec::new(),
            created_ms: now_ms,
            updated_ms: now_ms,
        }
    }

    pub(crate) fn transition(&mut self, next: CampaignStatus, now_ms: u64) -> Result<(), String> {
        if !self.status.allows(next) {
            return Err(format!(
                "illegal campaign transition {:?} -> {:?}",
                self.status, next
            ));
        }
        self.status = next;
        self.revision = self.revision.saturating_add(1);
        self.updated_ms = now_ms;
        self.validate()
    }

    pub(crate) fn execution_request(&self) -> Result<ExecutionRequest, String> {
        let contract = self
            .active_contract
            .as_ref()
            .ok_or_else(|| "campaign has no active round contract".to_string())?;
        if !matches!(
            self.status,
            CampaignStatus::Running
                | CampaignStatus::VerifyingRound
                | CampaignStatus::ReviewingRound
        ) {
            return Err("campaign is not in an executable state".to_string());
        }
        Ok(ExecutionRequest {
            campaign_id: self.id.clone(),
            campaign_revision: self.revision,
            contract_digest: contract.digest.clone(),
            base_oid: contract.base_oid.clone(),
            target_criteria: contract.target_criteria.clone(),
        })
    }

    pub(crate) fn validate_for(&self, workspace: &Path) -> Result<(), String> {
        if !self.project.matches(workspace) {
            return Err("campaign record belongs to a different project".to_string());
        }
        self.validate()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != CAMPAIGN_SCHEMA {
            return Err("unknown campaign schema".to_string());
        }
        validate_campaign_id(&self.id)?;
        validate_bounded("objective", &self.objective, 1, MAX_OBJECTIVE_BYTES)?;
        if self.objective_digest != digest_text(&self.objective) {
            return Err("campaign objective digest mismatch".to_string());
        }
        if self.revision == 0 || self.created_ms == 0 || self.updated_ms < self.created_ms {
            return Err("invalid campaign revision/timestamps".to_string());
        }
        if self.criteria.len() > MAX_CRITERIA {
            return Err(format!("campaign exceeds {MAX_CRITERIA} criteria"));
        }
        if self.policy.stall_rounds == 0
            || self.policy.full_alignment_interval == 0
            || !(1..=2).contains(&self.policy.max_target_criteria_per_round)
        {
            return Err("invalid campaign policy".to_string());
        }
        let mut ids = BTreeSet::new();
        for criterion in &self.criteria {
            if !ids.insert(criterion.id) {
                return Err(format!("duplicate criterion {}", criterion.id));
            }
            validate_criterion(criterion)?;
        }
        let mut round_numbers = BTreeSet::new();
        for receipt in &self.rounds {
            if !round_numbers.insert(receipt.contract.round) {
                return Err(format!(
                    "duplicate campaign round {}",
                    receipt.contract.round
                ));
            }
        }
        for (index, criterion) in self.criteria.iter().enumerate() {
            let expected = u16::try_from(index + 1)
                .ok()
                .and_then(|value| CriterionId::new(value).ok());
            if Some(criterion.id) != expected {
                return Err("criterion ids must be contiguous and ordered".to_string());
            }
        }
        if !matches!(
            self.status,
            CampaignStatus::Draft | CampaignStatus::Stopped | CampaignStatus::Failed
        ) {
            let required = self.criteria.iter().filter(|criterion| criterion.required);
            if required.clone().next().is_none()
                || required
                    .filter(|criterion| criterion.status != CriterionStatus::Deferred)
                    .any(|criterion| criterion.verifiers.is_empty())
            {
                return Err(
                    "non-draft campaigns require a verifier for every required criterion"
                        .to_string(),
                );
            }
        }
        if self.status == CampaignStatus::Paused && self.paused_from.is_none() {
            return Err("paused campaign is missing its prior state".to_string());
        }
        if self.status != CampaignStatus::Paused && self.paused_from.is_some() {
            return Err("only paused campaigns may record a prior state".to_string());
        }
        if let Some(contract) = &self.active_contract {
            validate_round_contract(contract, self)?;
        }
        for receipt in &self.rounds {
            validate_round_receipt(receipt, self)?;
        }
        if let Some(oid) = &self.frozen_base_oid {
            validate_oid("frozen base", oid)?;
        }
        if let Some(oid) = &self.campaign_head_oid {
            validate_oid("campaign head", oid)?;
        }
        if let Some(reference) = &self.campaign_ref {
            validate_bounded("campaign ref", reference, 1, 255)?;
            if reference.chars().any(char::is_whitespace)
                || reference.contains("..")
                || reference.contains("@{")
            {
                return Err("invalid campaign ref".to_string());
            }
        }
        Ok(())
    }
}

fn validate_criterion(criterion: &AcceptanceCriterion) -> Result<(), String> {
    validate_bounded("criterion", &criterion.text, 1, MAX_CRITERION_BYTES)?;
    if criterion.verifiers.len() > MAX_VERIFIERS_PER_CRITERION {
        return Err("criterion has too many verifiers".to_string());
    }
    if criterion.test_scope.len() > MAX_SCOPE_PER_CRITERION {
        return Err("criterion has too many scope paths".to_string());
    }
    if criterion.proof_refs.len() > MAX_PROOFS_PER_CRITERION {
        return Err("criterion has too many proof references".to_string());
    }
    let mut verifier_ids = BTreeSet::new();
    for verifier in &criterion.verifiers {
        validate_token("verifier id", &verifier.id, 64)?;
        if !verifier_ids.insert(&verifier.id) {
            return Err(format!("duplicate verifier id {}", verifier.id));
        }
        validate_bounded("verifier command", &verifier.command, 1, MAX_COMMAND_BYTES)?;
        if verifier.timeout_secs == 0 || verifier.timeout_secs > 86_400 {
            return Err("verifier timeout must be 1..86400 seconds".to_string());
        }
    }
    for path in &criterion.test_scope {
        if !valid_relative_path(path) || path.as_os_str().len() > MAX_PATH_BYTES {
            return Err(format!("invalid criterion scope {}", path.display()));
        }
    }
    if let Some(waiver) = &criterion.waiver {
        validate_bounded("waiver reason", &waiver.reason, 1, 2048)?;
    }
    Ok(())
}

fn validate_round_contract(
    contract: &RoundContract,
    campaign: &CampaignRecord,
) -> Result<(), String> {
    if contract.schema != ROUND_SCHEMA
        || contract.campaign_id != campaign.id
        || contract.campaign_revision == 0
        || contract.campaign_revision > campaign.revision
        || contract.round == 0
        || contract.target_criteria.is_empty()
        || contract.target_criteria.len()
            > usize::from(campaign.policy.max_target_criteria_per_round)
    {
        return Err("invalid round contract identity".to_string());
    }
    validate_oid("round base", &contract.base_oid)?;
    validate_bounded(
        "round objective",
        &contract.objective,
        1,
        MAX_OBJECTIVE_BYTES,
    )?;
    let known = campaign
        .criteria
        .iter()
        .map(|criterion| criterion.id)
        .collect::<BTreeSet<_>>();
    if contract
        .target_criteria
        .iter()
        .any(|criterion| !known.contains(criterion))
    {
        return Err("round contract references an unknown criterion".to_string());
    }
    if contract
        .target_criteria
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != contract.target_criteria.len()
    {
        return Err("round contract repeats a target criterion".to_string());
    }
    validate_bounded(
        "round targeted test command",
        &contract.targeted_test_cmd,
        1,
        MAX_COMMAND_BYTES,
    )?;
    validate_bounded(
        "round accept command",
        &contract.accept_cmd,
        1,
        MAX_COMMAND_BYTES,
    )?;
    if contract.quality_cmds.len() > MAX_QUALITY_COMMANDS
        || contract.test_scope.len() > MAX_SCOPE_PER_CRITERION
        || contract
            .quality_cmds
            .iter()
            .any(|command| command.is_empty() || command.len() > MAX_COMMAND_BYTES)
        || contract
            .test_scope
            .iter()
            .any(|path| !valid_relative_path(path) || path.as_os_str().len() > MAX_PATH_BYTES)
    {
        return Err("invalid round command or scope".to_string());
    }
    if contract.digest != digest_round_contract(contract)? {
        return Err("round contract digest mismatch".to_string());
    }
    Ok(())
}

fn validate_round_receipt(receipt: &RoundReceipt, campaign: &CampaignRecord) -> Result<(), String> {
    validate_round_contract(&receipt.contract, campaign)?;
    if receipt.changed_paths.len() > MAX_CHANGED_PATHS
        || receipt.proofs.len() > MAX_PROOFS_PER_ROUND
        || receipt
            .changed_paths
            .iter()
            .any(|path| !valid_relative_path(path))
        || receipt
            .failure
            .as_ref()
            .is_some_and(|failure| failure.len() > 4096)
    {
        return Err("invalid round receipt".to_string());
    }
    for proof in &receipt.proofs {
        validate_proof(proof)?;
    }
    if let Some(run_id) = &receipt.swarm_run_id {
        validate_token("swarm run id", run_id, 96)?;
    }
    if let Some(branch) = &receipt.candidate_branch {
        validate_bounded("candidate branch", branch, 1, 255)?;
        if branch.chars().any(char::is_whitespace) || branch.contains("..") {
            return Err("invalid candidate branch".to_string());
        }
    }
    if let Some(oid) = &receipt.candidate_oid {
        validate_oid("candidate", oid)?;
    }
    for review in [
        receipt.code_review.as_ref(),
        receipt.alignment_review.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_bounded("review route", &review.route, 1, 256)?;
        validate_bounded("review model revision", &review.model_revision, 1, 256)?;
        validate_bounded(
            "review contract digest",
            &review.reviewed_contract_digest,
            1,
            128,
        )?;
        validate_oid("review candidate", &review.reviewed_candidate_oid)?;
        if review.reviewed_contract_digest != receipt.contract.digest {
            return Err("review receipt targets a different round contract".to_string());
        }
        if receipt.candidate_oid.as_deref() != Some(review.reviewed_candidate_oid.as_str()) {
            return Err("review receipt targets a different candidate".to_string());
        }
    }
    if matches!(
        receipt.state,
        RoundState::TechnicallyVerified | RoundState::Accepted
    ) && (receipt.swarm_run_id.is_none()
        || receipt.candidate_oid.is_none()
        || receipt.proofs.is_empty())
    {
        return Err("verified round is missing its proof identity".to_string());
    }
    if receipt.state == RoundState::Accepted {
        let code_review = receipt
            .code_review
            .as_ref()
            .ok_or_else(|| "accepted round is missing code review".to_string())?;
        let alignment_review = receipt
            .alignment_review
            .as_ref()
            .ok_or_else(|| "accepted round is missing alignment review".to_string())?;
        if code_review.verdict != ReviewVerdict::Pass
            || alignment_review.verdict != ReviewVerdict::Pass
        {
            return Err("accepted round contains a blocking review".to_string());
        }
        if campaign.policy.require_independent_reviewer
            && matches!(
                alignment_review.independence,
                ReviewIndependence::SameRoute | ReviewIndependence::Unavailable
            )
        {
            return Err("accepted round lacks an independent alignment reviewer".to_string());
        }
    }
    Ok(())
}

pub(crate) fn digest_round_contract(contract: &RoundContract) -> Result<String, String> {
    let mut material = contract.clone();
    material.digest.clear();
    serde_json::to_vec(&material)
        .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
        .map_err(|error| format!("encode round contract digest: {error}"))
}

fn validate_proof(proof: &ProofRef) -> Result<(), String> {
    validate_token("proof run id", &proof.run_id, 96)?;
    if !valid_relative_path(&proof.artifact_path)
        || proof.artifact_path.as_os_str().len() > MAX_PATH_BYTES
    {
        return Err("invalid proof artifact path".to_string());
    }
    if proof.sha256.len() != 64
        || !proof
            .sha256
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("invalid proof SHA-256".to_string());
    }
    validate_oid("proof Git", &proof.git_oid)?;
    validate_bounded("proof summary", &proof.summary, 1, 2048)
}

pub(crate) fn digest_text(text: &str) -> String {
    crate::knowledge::cut::sha256_hex(text.as_bytes())
}

pub(crate) fn validate_campaign_id(id: &str) -> Result<(), String> {
    let valid = id.starts_with("cmp-")
        && id.len() <= 96
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    valid
        .then_some(())
        .ok_or_else(|| "invalid campaign id".to_string())
}

pub(crate) fn valid_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

pub(crate) fn validate_bounded(
    label: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), String> {
    let len = value.len();
    if len < min || len > max || value.contains('\0') {
        return Err(format!("{label} must be {min}..{max} bytes"));
    }
    Ok(())
}

fn validate_token(label: &str, value: &str, max: usize) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.len() <= max
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    valid
        .then_some(())
        .ok_or_else(|| format!("invalid {label}"))
}

fn validate_oid(label: &str, value: &str) -> Result<(), String> {
    let valid = (7..=64).contains(&value.len())
        && value.chars().all(|character| character.is_ascii_hexdigit());
    valid
        .then_some(())
        .ok_or_else(|| format!("invalid {label} Git OID"))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/campaign__schema__l01_tests.rs"]
mod l01_tests;
