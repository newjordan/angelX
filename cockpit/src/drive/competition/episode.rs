use super::journal::ActionJournalEventV1;
use super::profile::{DeepCutProfileIdentityV1, EpisodeSourceLineageV1, ReplayAncestryV1};
use super::schema::{CompetitionKeyV1, ObjectiveComparatorV1};
use serde::{Deserialize, Serialize};

pub(crate) const DEEP_CUT_EPISODE_SCHEMA_V1: &str = "angel.deep-cut-episode/v1";
pub(crate) const DEEP_CUT_START_INTENT_VERSION_V1: &str = "deep-cut-episode-start/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EpisodeStartedV1 {
    pub(crate) episode_id: String,
    pub(crate) campaign_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) profile: DeepCutProfileIdentityV1,
    pub(crate) board_epoch: u64,
    pub(crate) board_observation_revision: u64,
    pub(crate) board_decision_sha256: String,
    pub(crate) source: EpisodeSourceLineageV1,
    pub(crate) hypothesis_id: String,
    pub(crate) origin_episode_id: Option<String>,
    pub(crate) replay: Option<ReplayAncestryV1>,
    pub(crate) worker_instance_id: String,
    pub(crate) model_id: String,
    pub(crate) requested_route: String,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) tool_strategy_sha256: String,
    pub(crate) start_journal_event: ActionJournalEventV1,
    pub(crate) started_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EpisodeEvidenceKindV1 {
    Action,
    GraphEpisode,
    GraphTrace,
    HarnessRollout,
    Candidate,
    Verifier,
    LocalMeasurement,
    Submission,
    OfficialResult,
    RewardBinding,
    PatternUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EpisodeEvidenceLinkV1 {
    pub(crate) kind: EpisodeEvidenceKindV1,
    pub(crate) identity: String,
    pub(crate) receipt_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EpisodeTerminalOutcomeV1 {
    Success,
    Regression,
    Rejection,
    CorrectnessFailure,
    Timeout,
    Crash,
    Abandoned,
    Stale,
    Replayed,
    Recovered,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EpisodeTerminalV1 {
    pub(crate) outcome: EpisodeTerminalOutcomeV1,
    pub(crate) recovered_from: Option<EpisodeTerminalOutcomeV1>,
    pub(crate) detail_sha256: Option<String>,
    pub(crate) elapsed_ms: u64,
    pub(crate) model_calls: u64,
    pub(crate) tool_calls: u64,
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) monetary_microunits: u64,
    pub(crate) terminal_at_ms: u64,
    pub(crate) action_journal_end_sequence: u64,
    pub(crate) action_journal_head_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "receipt", rename_all = "snake_case")]
pub(crate) enum EpisodeEventKindV1 {
    Started(Box<EpisodeStartedV1>),
    EvidenceLinked(EpisodeEvidenceLinkV1),
    Terminal(EpisodeTerminalV1),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EpisodeEventV1 {
    pub(crate) schema: String,
    pub(crate) episode_id: String,
    pub(crate) sequence: u64,
    pub(crate) previous_sha256: String,
    pub(crate) event: EpisodeEventKindV1,
    pub(crate) event_sha256: String,
}

impl EpisodeEventV1 {
    pub(crate) fn started(start: EpisodeStartedV1) -> Result<Self, String> {
        start.validate()?;
        let episode_id = start.episode_id.clone();
        Self::new(
            episode_id,
            0,
            String::new(),
            EpisodeEventKindV1::Started(Box::new(start)),
        )
    }

    pub(super) fn new(
        episode_id: String,
        sequence: u64,
        previous_sha256: String,
        event: EpisodeEventKindV1,
    ) -> Result<Self, String> {
        let mut value = Self {
            schema: DEEP_CUT_EPISODE_SCHEMA_V1.into(),
            episode_id,
            sequence,
            previous_sha256,
            event,
            event_sha256: String::new(),
        };
        value.event_sha256 = value.canonical_sha256()?;
        Ok(value)
    }

    pub(super) fn canonical_sha256(&self) -> Result<String, String> {
        let mut canonical = self.clone();
        canonical.event_sha256.clear();
        serde_json::to_vec(&canonical)
            .map(|body| crate::knowledge::cut::sha256_hex(&body))
            .map_err(|error| format!("encode deep-cut episode event: {error}"))
    }
}

pub(super) fn validate_event_body(event: &EpisodeEventKindV1) -> Result<(), String> {
    match event {
        EpisodeEventKindV1::Started(start) => start.validate(),
        EpisodeEventKindV1::EvidenceLinked(link) => validate_link(link),
        EpisodeEventKindV1::Terminal(terminal) => validate_terminal(terminal),
    }
}

pub(super) fn validate_link(link: &EpisodeEvidenceLinkV1) -> Result<(), String> {
    super::schema_validation::validate_id(&link.identity, "invalid episode evidence identity")
        .and_then(|_| super::schema_validation::validate_sha256(&link.receipt_sha256))
        .map_err(str::to_string)
}

pub(super) fn validate_terminal(terminal: &EpisodeTerminalV1) -> Result<(), String> {
    if (terminal.outcome == EpisodeTerminalOutcomeV1::Recovered)
        != terminal.recovered_from.is_some()
        || terminal
            .detail_sha256
            .as_deref()
            .is_some_and(|value| super::schema_validation::validate_sha256(value).is_err())
        || super::schema_validation::validate_sha256(&terminal.action_journal_head_sha256).is_err()
    {
        return Err("invalid deep-cut episode terminal".into());
    }
    Ok(())
}

pub(super) fn post_terminal_evidence(kind: EpisodeEvidenceKindV1) -> bool {
    matches!(
        kind,
        EpisodeEvidenceKindV1::OfficialResult
            | EpisodeEvidenceKindV1::RewardBinding
            | EpisodeEvidenceKindV1::PatternUpdate
    )
}
