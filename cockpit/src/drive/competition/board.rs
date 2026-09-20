use super::adapters::AdapterFailureV1;
use super::frontier::FrontierV1;
use super::schema::{
    AdapterCapabilityV1, AdapterIdentityV1, CompetitionKeyV1, DirectorHealthStateV1,
    ObjectiveComparatorV1, ScheduledActionV1, ScoreV1,
};
use super::schema_validation::{validate_id, validate_sha256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub(crate) const BOARD_OBSERVATION_SCHEMA_V1: &str = "angel.competition-board-observation/v1";
pub(crate) const CANONICAL_BOARD_SCHEMA_V1: &str = "angel.competition-board/v1";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservationCursorV1 {
    pub(crate) opaque: Option<String>,
    pub(crate) sequence: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BoardObservationKindV1 {
    Full,
    Delta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservationSourceV1 {
    LiveApi,
    Cli,
    Fixture,
    LegacyImport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceAccessClaimV1 {
    pub(crate) source_id: String,
    pub(crate) commit_oid: String,
    pub(crate) tree_oid: String,
    pub(crate) workspace_sha256: String,
    pub(crate) access_proof_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoardEntryV1 {
    pub(crate) entry_id: String,
    pub(crate) participant_id: String,
    pub(crate) submission_id: Option<String>,
    pub(crate) rank: Option<u64>,
    pub(crate) score: ScoreV1,
    pub(crate) personal: bool,
    pub(crate) source: Option<SourceAccessClaimV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BoardChangeV1 {
    Upsert { entry: Box<BoardEntryV1> },
    Remove { entry_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservationProvenanceV1 {
    pub(crate) adapter: AdapterIdentityV1,
    pub(crate) source: ObservationSourceV1,
    pub(crate) observed_at_ms: u64,
    pub(crate) platform_event_at_ms: Option<u64>,
    pub(crate) raw_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoardObservationV1 {
    pub(crate) schema: String,
    pub(crate) observation_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) kind: BoardObservationKindV1,
    pub(crate) cursor: ObservationCursorV1,
    pub(crate) changes: Vec<BoardChangeV1>,
    pub(crate) provenance: ObservationProvenanceV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BoardFreshnessV1 {
    Fresh,
    Stale { since_ms: u64, reason: String },
    Unconfirmed { reason: String, refresh_due_ms: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CanonicalBoardV1 {
    pub(crate) schema: String,
    pub(crate) campaign_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) comparator: ObjectiveComparatorV1,
    pub(crate) board_epoch: u64,
    pub(crate) predecessor_epoch: Option<u64>,
    pub(crate) observation_revision: u64,
    pub(crate) entries: Vec<BoardEntryV1>,
    pub(crate) frontier: FrontierV1,
    pub(crate) decision_sha256: String,
    pub(crate) latest_observation_id: String,
    pub(crate) latest_provenance: ObservationProvenanceV1,
    pub(crate) last_sequence: Option<u64>,
    pub(crate) freshness: BoardFreshnessV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RawObservationStatusV1 {
    Persisted,
    AlreadyPresent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RawObservationReceiptV1 {
    pub(crate) observation_id: String,
    pub(crate) raw_sha256: String,
    pub(crate) journal_sequence: u64,
    pub(crate) status: RawObservationStatusV1,
}

pub(crate) trait RawBoardJournalV1 {
    fn persist_raw(
        &mut self,
        observation: &BoardObservationV1,
    ) -> Result<RawObservationReceiptV1, String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoardReduceEffectV1 {
    Initialized,
    DecisionAdvanced,
    DecisionRefreshed,
    DuplicateObservation,
    ReorderedIgnored,
    GapDetected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoardReduceOutcomeV1 {
    pub(crate) canonical: Option<CanonicalBoardV1>,
    pub(crate) effect: BoardReduceEffectV1,
    pub(crate) refresh: Option<ScheduledActionV1>,
    pub(crate) raw_receipt: RawObservationReceiptV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoardFetchFailureV1 {
    pub(crate) canonical: Option<CanonicalBoardV1>,
    pub(crate) failure: AdapterFailureV1,
    pub(crate) health: DirectorHealthStateV1,
    pub(crate) next: ScheduledActionV1,
}

#[derive(Debug)]
pub(crate) enum BoardReduceErrorV1 {
    Journal(String),
    Invalid(String),
    Adapter(AdapterFailureV1),
}

impl fmt::Display for BoardReduceErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Journal(error) => write!(f, "raw board journal: {error}"),
            Self::Invalid(error) => write!(f, "invalid board observation: {error}"),
            Self::Adapter(error) => write!(f, "board adapter failure: {:?}", error.class),
        }
    }
}

pub(super) fn validate_persisted_observation(
    observation: &BoardObservationV1,
    adapter: &AdapterIdentityV1,
    receipt: &RawObservationReceiptV1,
    current: Option<&CanonicalBoardV1>,
) -> Result<(), BoardReduceErrorV1> {
    let valid_source = |source: &SourceAccessClaimV1| {
        validate_id(&source.source_id, "invalid source id").is_ok()
            && valid_object_oid(&source.commit_oid)
            && valid_object_oid(&source.tree_oid)
            && validate_sha256(&source.workspace_sha256).is_ok()
            && validate_sha256(&source.access_proof_sha256).is_ok()
    };
    let malformed_source = observation.changes.iter().any(|change| {
        matches!(change, BoardChangeV1::Upsert { entry } if entry.source.as_ref().is_some_and(|source| !valid_source(source)))
    });
    if observation.schema != BOARD_OBSERVATION_SCHEMA_V1
        || validate_id(&observation.observation_id, "invalid observation id").is_err()
        || observation.competition.validate().is_err()
        || adapter.validate().is_err()
        || observation.provenance.adapter != *adapter
        || !adapter.capabilities.contains(&AdapterCapabilityV1::Board)
        || validate_sha256(&observation.provenance.raw_sha256).is_err()
        || receipt.observation_id != observation.observation_id
        || receipt.raw_sha256 != observation.provenance.raw_sha256
        || current.is_some_and(|board| board.competition != observation.competition)
        || malformed_source
    {
        return Err(BoardReduceErrorV1::Invalid(
            "schema, competition, adapter, source claim, or raw receipt mismatch".into(),
        ));
    }
    Ok(())
}

fn valid_object_oid(value: &str) -> bool {
    validate_sha256(value).is_ok()
        || (value.len() == 40
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
}

pub(super) fn apply_board_changes(
    current: Option<&CanonicalBoardV1>,
    observation: &BoardObservationV1,
) -> Result<Vec<BoardEntryV1>, BoardReduceErrorV1> {
    let mut entries: BTreeMap<String, BoardEntryV1> =
        if observation.kind == BoardObservationKindV1::Full {
            BTreeMap::new()
        } else {
            current
                .into_iter()
                .flat_map(|board| board.entries.iter().cloned())
                .map(|entry| (entry.entry_id.clone(), entry))
                .collect()
        };
    for change in &observation.changes {
        match change {
            BoardChangeV1::Upsert { entry }
                if !entry.entry_id.is_empty() && !entry.participant_id.is_empty() =>
            {
                entries.insert(entry.entry_id.clone(), entry.as_ref().clone());
            }
            BoardChangeV1::Remove { entry_id }
                if observation.kind == BoardObservationKindV1::Delta && !entry_id.is_empty() =>
            {
                entries.remove(entry_id);
            }
            BoardChangeV1::Upsert { .. } | BoardChangeV1::Remove { .. } => {
                return Err(BoardReduceErrorV1::Invalid(
                    "invalid board entry identity or full removal".into(),
                ));
            }
        }
    }
    Ok(entries.into_values().collect())
}

pub(super) fn refresh_action(next_attempt_at_ms: u64) -> ScheduledActionV1 {
    ScheduledActionV1 {
        action: "full_board_refresh".into(),
        next_attempt_at_ms,
    }
}

pub(super) fn remediation_action() -> ScheduledActionV1 {
    ScheduledActionV1 {
        action: "remediate_board_adapter".into(),
        next_attempt_at_ms: u64::MAX,
    }
}

pub(super) fn reduce_outcome(
    current: Option<&CanonicalBoardV1>,
    effect: BoardReduceEffectV1,
    refresh: Option<ScheduledActionV1>,
    raw_receipt: RawObservationReceiptV1,
) -> BoardReduceOutcomeV1 {
    BoardReduceOutcomeV1 {
        canonical: current.cloned(),
        effect,
        refresh,
        raw_receipt,
    }
}

pub(super) fn replay_receipt(
    observation: &BoardObservationV1,
    journal_sequence: u64,
) -> RawObservationReceiptV1 {
    RawObservationReceiptV1 {
        observation_id: observation.observation_id.clone(),
        raw_sha256: observation.provenance.raw_sha256.clone(),
        journal_sequence,
        status: RawObservationStatusV1::Persisted,
    }
}
