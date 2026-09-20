use super::board::{BoardEntryV1, BoardObservationV1, ObservationCursorV1, SourceAccessClaimV1};
use super::schema::{
    AdapterIdentityV1, CompetitionKeyV1, ObjectiveComparatorV1, RetryPolicyV1, ScoreV1,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub(crate) mod fixture;
pub(crate) mod flywheel;
pub(crate) mod flywheel_files;
pub(crate) mod flywheel_results;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AdapterFailureClassV1 {
    Unsupported,
    Auth,
    RateLimited,
    Transport,
    Malformed,
    Invariant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdapterFailureV1 {
    pub(crate) class: AdapterFailureClassV1,
    pub(crate) retry: RetryPolicyV1,
    pub(crate) detail_sha256: String,
    pub(crate) provenance_sha256: String,
}

impl AdapterFailureV1 {
    pub(crate) fn unsupported(capability: &str) -> Self {
        Self {
            class: AdapterFailureClassV1::Unsupported,
            retry: RetryPolicyV1::Never,
            detail_sha256: crate::knowledge::cut::sha256_hex(capability.as_bytes()),
            provenance_sha256: crate::knowledge::cut::sha256_hex(b"adapter-capability/v1"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompetitionIdentityV1 {
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) comparator: ObjectiveComparatorV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PersonalSubmissionObservationV1 {
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) cursor: ObservationCursorV1,
    pub(crate) entries: Vec<BoardEntryV1>,
    pub(crate) observed_at_ms: u64,
    pub(crate) raw_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceAccessObservationV1 {
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) cursor: ObservationCursorV1,
    pub(crate) claims: Vec<SourceAccessClaimV1>,
    pub(crate) observed_at_ms: u64,
    pub(crate) raw_sha256: String,
}

/// Synchronous because the director invokes adapters from independent workers.
/// Submission/result capabilities extend this boundary in F3 without changing
/// board reduction semantics.
pub(crate) trait CompetitionAdapterV1: Send {
    fn identity(&self) -> &AdapterIdentityV1;

    fn identify_competition(&mut self) -> Result<CompetitionIdentityV1, AdapterFailureV1>;

    fn fetch_board(
        &mut self,
        cursor: Option<&ObservationCursorV1>,
    ) -> Result<BoardObservationV1, AdapterFailureV1>;

    fn fetch_personal_submissions(
        &mut self,
        _cursor: Option<&ObservationCursorV1>,
    ) -> Result<PersonalSubmissionObservationV1, AdapterFailureV1> {
        Err(AdapterFailureV1::unsupported("personal_submissions"))
    }

    fn observe_source_access(
        &mut self,
        _cursor: Option<&ObservationCursorV1>,
    ) -> Result<SourceAccessObservationV1, AdapterFailureV1> {
        Err(AdapterFailureV1::unsupported("source_access"))
    }

    /// Returned ordering is candidate-oriented: Greater always means better.
    fn compare_scores(
        &self,
        comparator: &ObjectiveComparatorV1,
        candidate: &ScoreV1,
        baseline: &ScoreV1,
    ) -> Result<Ordering, AdapterFailureV1> {
        comparator
            .compare(candidate, baseline)
            .map_err(|_| AdapterFailureV1::unsupported("adapter_defined_score_comparison"))
    }
}
