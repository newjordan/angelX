use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;

pub(crate) const COMPETITION_CONTRACT_V1: &str = "angel.competition-contract/v1";
pub(crate) const COMPETITION_SCHEMA_V1: &str = "angel.competition/v1";
const MAX_SCORE_BYTES: usize = 128;

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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ScoreV1(String);

impl ScoreV1 {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ScoreError> {
        let value = value.into();
        validate_canonical_decimal(&value)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn numeric_cmp(&self, other: &Self) -> Ordering {
        decimal_cmp(&self.0, &other.0)
    }
}

impl Serialize for ScoreV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ScoreV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScoreError;

impl fmt::Display for ScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("score must be a canonical finite decimal")
    }
}

fn validate_canonical_decimal(value: &str) -> Result<(), ScoreError> {
    if value.is_empty() || value.len() > MAX_SCORE_BYTES || value.starts_with('+') {
        return Err(ScoreError);
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let mut parts = unsigned.split('.');
    let integer = parts.next().ok_or(ScoreError)?;
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
        || fraction.is_some_and(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.ends_with('0')
        })
        || (value.starts_with('-') && unsigned == "0")
    {
        return Err(ScoreError);
    }
    Ok(())
}

fn decimal_cmp(left: &str, right: &str) -> Ordering {
    let left_negative = left.starts_with('-');
    let right_negative = right.starts_with('-');
    if left_negative != right_negative {
        return if left_negative {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    let magnitude =
        decimal_magnitude_cmp(left.trim_start_matches('-'), right.trim_start_matches('-'));
    if left_negative {
        magnitude.reverse()
    } else {
        magnitude
    }
}

fn decimal_magnitude_cmp(left: &str, right: &str) -> Ordering {
    let (left_integer, left_fraction) = left.split_once('.').unwrap_or((left, ""));
    let (right_integer, right_fraction) = right.split_once('.').unwrap_or((right, ""));
    left_integer
        .len()
        .cmp(&right_integer.len())
        .then_with(|| left_integer.cmp(right_integer))
        .then_with(|| {
            let width = left_fraction.len().max(right_fraction.len());
            (0..width)
                .map(|index| left_fraction.as_bytes().get(index).copied().unwrap_or(b'0'))
                .cmp((0..width).map(|index| {
                    right_fraction
                        .as_bytes()
                        .get(index)
                        .copied()
                        .unwrap_or(b'0')
                }))
        })
}

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
