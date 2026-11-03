use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeSet;

pub(crate) const COMPETITION_CONTRACT_V1: &str = "angel.competition-contract/v1";
pub(crate) const COMPETITION_SCHEMA_V1: &str = "angel.competition/v1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompetitionKeyV1 {
    pub(crate) platform_id: String,
    pub(crate) competition_id: String,
    pub(crate) field_id: String,
    pub(crate) benchmark_id: String,
    pub(crate) profile_id: String,
    pub(crate) hardware_id: String,
}

pub(crate) use crate::telemetry::ScoreV1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ComparatorKindV1 {
    HigherIsBetter,
    LowerIsBetter,
    AdapterDefined {
        contract_id: String,
        version_sha256: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObjectiveComparatorV1 {
    pub(crate) objective_id: String,
    pub(crate) version: String,
    pub(crate) kind: ComparatorKindV1,
}

impl ObjectiveComparatorV1 {
    /// Ordering is candidate-oriented: `Greater` always means improvement.
    pub(crate) fn compare(
        &self,
        candidate: &ScoreV1,
        baseline: &ScoreV1,
    ) -> Result<Ordering, CompareError> {
        match self.kind {
            ComparatorKindV1::HigherIsBetter => Ok(candidate.numeric_cmp(baseline)),
            ComparatorKindV1::LowerIsBetter => Ok(baseline.numeric_cmp(candidate)),
            ComparatorKindV1::AdapterDefined { .. } => Err(CompareError::AdapterRequired),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompareError {
    AdapterRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AdapterCapabilityV1 {
    Board,
    PersonalSubmissions,
    SourceAccess,
    Submit,
    Reconcile,
    Results,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdapterIdentityV1 {
    pub(crate) adapter_id: String,
    pub(crate) adapter_version: String,
    pub(crate) runtime_sha256: String,
    pub(crate) capabilities: BTreeSet<AdapterCapabilityV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DirectorHealthStateV1 {
    Fresh,
    Degraded,
    Retrying,
    NeedsAttention,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScheduledActionV1 {
    pub(crate) action: String,
    pub(crate) next_attempt_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectorHealthV1 {
    pub(crate) schema: String,
    pub(crate) state: DirectorHealthStateV1,
    pub(crate) last_good_revision: Option<u64>,
    pub(crate) reason: Option<String>,
    pub(crate) next: ScheduledActionV1,
    pub(crate) updated_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LaneIdV1 {
    FrontierGuard,
    DeepCut,
    ContextMiner,
    RedTeam,
    Transfer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RetryPolicyV1 {
    Never,
    Immediate,
    AfterMs(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActionKindV1 {
    ObserveBoard,
    AcquireSource,
    StartEpisode,
    DispatchWorker,
    VerifyCandidate,
    SubmitCandidate,
    ReconcileSubmission,
    BindOfficialResult,
    PublishCheckpoint,
    MigrateState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActionPhaseV1 {
    Planned,
    Started,
    Ambiguous,
    Completed,
    Failed,
}

impl ActionPhaseV1 {
    pub(crate) fn allows(self, next: Self, retryable: bool) -> bool {
        use ActionPhaseV1 as P;
        matches!(
            (self, next),
            (P::Planned, P::Started | P::Failed)
                | (P::Started, P::Ambiguous | P::Completed | P::Failed)
                | (P::Ambiguous, P::Completed | P::Failed)
        ) || (self == P::Failed && next == P::Planned && retryable)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionIntentV1 {
    pub(crate) action_key: String,
    pub(crate) campaign_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) kind: ActionKindV1,
    pub(crate) subject_id: String,
    pub(crate) payload_sha256: String,
    pub(crate) intent_version: String,
}
