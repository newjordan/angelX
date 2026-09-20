use super::schema::{DirectorHealthV1, LaneIdV1};
use super::schema_validation::{validate_id, validate_sha256};
use serde::{Deserialize, Serialize};

pub(crate) const DIRECTOR_SERVICE_SCHEMA_V1: &str = "angel.competition-director-service/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "service", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DirectorServiceKindV1 {
    RefreshBoard { full: bool },
    RemediateBoardAdapter,
    ContinueContext,
    ReplayCandidates { from_epoch: u64, to_epoch: u64 },
    StepSubmissions,
    AdvanceEpisodes,
    BindOfficialRewards,
    InspectWorkers,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectorServiceIntentV1 {
    pub(crate) schema: String,
    pub(crate) intent_id: String,
    pub(crate) campaign_id: String,
    pub(crate) kind: DirectorServiceKindV1,
    pub(crate) due_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectorEpisodeRefV1 {
    pub(crate) episode_id: String,
    pub(crate) candidate_id: String,
    pub(crate) submission_id: String,
    pub(crate) board_epoch: u64,
    pub(crate) journal_head_sha256: String,
    pub(crate) terminal: bool,
    pub(crate) latest_official_result_id: Option<String>,
    pub(crate) latest_reward_binding_sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectorWorkerRefV1 {
    pub(crate) work_item_id: String,
    pub(crate) lane: LaneIdV1,
    pub(crate) lease_id: String,
    pub(crate) fencing_generation: u64,
    pub(crate) checkpoint_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectorTransitionV1 {
    pub(crate) health: DirectorHealthV1,
    pub(crate) scheduled: Vec<DirectorServiceIntentV1>,
}

impl DirectorServiceIntentV1 {
    pub(crate) fn new(
        campaign_id: &str,
        kind: DirectorServiceKindV1,
        due_at_ms: u64,
    ) -> Result<Self, String> {
        validate_id(campaign_id, "invalid service campaign").map_err(str::to_string)?;
        validate_kind(&kind)?;
        let mut value = Self {
            schema: DIRECTOR_SERVICE_SCHEMA_V1.into(),
            intent_id: String::new(),
            campaign_id: campaign_id.into(),
            kind,
            due_at_ms,
        };
        value.intent_id = value.canonical_id()?;
        Ok(value)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != DIRECTOR_SERVICE_SCHEMA_V1
            || validate_id(&self.campaign_id, "invalid service campaign").is_err()
            || validate_sha256(&self.intent_id).is_err()
            || self.intent_id != self.canonical_id()?
        {
            return Err("invalid director service intent".into());
        }
        validate_kind(&self.kind)
    }

    pub(super) fn class(&self) -> &'static str {
        match self.kind {
            DirectorServiceKindV1::RefreshBoard { .. }
            | DirectorServiceKindV1::RemediateBoardAdapter => "board",
            DirectorServiceKindV1::ContinueContext => "context",
            DirectorServiceKindV1::ReplayCandidates { .. } => "replay",
            DirectorServiceKindV1::StepSubmissions => "submission",
            DirectorServiceKindV1::AdvanceEpisodes => "episode",
            DirectorServiceKindV1::BindOfficialRewards => "reward",
            DirectorServiceKindV1::InspectWorkers => "worker",
        }
    }

    fn canonical_id(&self) -> Result<String, String> {
        serde_json::to_vec(&(&self.campaign_id, &self.kind, self.due_at_ms))
            .map(|body| crate::cut::sha256_hex(&body))
            .map_err(|error| format!("encode director service intent: {error}"))
    }
}

impl DirectorEpisodeRefV1 {
    pub(crate) fn validate(&self) -> Result<(), String> {
        for id in [&self.episode_id, &self.candidate_id, &self.submission_id] {
            validate_id(id, "invalid episode attribution id").map_err(str::to_string)?;
        }
        if self.board_epoch == 0 {
            return Err("episode attribution requires a board epoch".into());
        }
        validate_sha256(&self.journal_head_sha256).map_err(str::to_string)?;
        for digest in [&self.latest_reward_binding_sha256] {
            if digest
                .as_deref()
                .is_some_and(|value| validate_sha256(value).is_err())
            {
                return Err("invalid episode attribution digest".into());
            }
        }
        if let Some(id) = &self.latest_official_result_id {
            validate_id(id, "invalid official result id").map_err(str::to_string)?;
        }
        Ok(())
    }
}

impl DirectorWorkerRefV1 {
    pub(crate) fn validate(&self) -> Result<(), String> {
        validate_id(&self.work_item_id, "invalid worker item").map_err(str::to_string)?;
        validate_id(&self.lease_id, "invalid worker lease").map_err(str::to_string)?;
        if self.fencing_generation == 0 {
            return Err("worker reference requires a fencing generation".into());
        }
        Ok(())
    }
}

fn validate_kind(kind: &DirectorServiceKindV1) -> Result<(), String> {
    if let DirectorServiceKindV1::ReplayCandidates {
        from_epoch,
        to_epoch,
    } = kind
        && (*from_epoch == 0 || to_epoch <= from_epoch)
    {
        return Err("candidate replay service requires an advancing epoch".into());
    }
    Ok(())
}
