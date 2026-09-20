use super::episode::{
    DEEP_CUT_EPISODE_SCHEMA_V1, DEEP_CUT_START_INTENT_VERSION_V1, EpisodeStartedV1,
};
use super::journal::{ACTION_JOURNAL_SCHEMA_V1, ActionJournalEventV1, canonical_action_key};
use super::schema::{ActionKindV1, ActionPhaseV1, COMPETITION_CONTRACT_V1, LaneIdV1};
use super::schema_validation::{validate_id, validate_sha256};
use serde::{Deserialize, Serialize};

pub(crate) const DEEP_CUT_ROLE_ID: &str = "/root/deep_cut";
pub(crate) const DEEP_CUT_PROFILE_SCHEMA_V1: &str = "angel.deep-cut-profile/v1";
pub(crate) const DEEP_CUT_PROFILE_VERSION_V1: &str = "deep-cut/v1";
pub(crate) const DEEP_CUT_PROMPT_VERSION_V1: &str = "deep-cut-prompt/v1";
pub(crate) const DEEP_CUT_POLICY_SHA256_V1: &str =
    "c6bf617aa9ae0feb30461f15cef07bed19f07525dd7a218f31a309f1ada31db0";
pub(crate) const DEEP_CUT_PROMPT_V1: &str = "Search for structural, measurable percentage cuts. Start a durable episode before effects; preserve actions, costs, failures, and replay ancestry; treat local measurements as shaping evidence; never self-award official reward or self-certify a replayed mechanism.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeepCutProfileV1 {
    pub(crate) schema: String,
    pub(crate) contract: String,
    pub(crate) role_id: String,
    pub(crate) profile_version: String,
    pub(crate) prompt_version: String,
    pub(crate) prompt_sha256: String,
    pub(crate) capture_policy_version: String,
    pub(crate) reward_policy_version: String,
    pub(crate) pattern_policy_version: String,
    pub(crate) default_lane: LaneIdV1,
    pub(crate) episode_start_required_before_effects: bool,
    pub(crate) retain_negative_outcomes: bool,
    pub(crate) replay_requires_new_episode: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeepCutProfileIdentityV1 {
    pub(crate) schema: String,
    pub(crate) role_id: String,
    pub(crate) profile_version: String,
    pub(crate) policy_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EpisodeSourceLineageV1 {
    pub(crate) base_id: String,
    pub(crate) source_board_epoch: u64,
    pub(crate) commit_oid: String,
    pub(crate) tree_oid: String,
    pub(crate) workspace_sha256: String,
    pub(crate) access_proof_sha256: String,
    pub(crate) observation_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplayAncestryV1 {
    pub(crate) replay_of_episode_id: String,
    pub(crate) replay_of_candidate_id: Option<String>,
    pub(crate) preserved_hypothesis_id: String,
    pub(crate) predecessor_board_epoch: u64,
}

impl DeepCutProfileV1 {
    pub(crate) fn embedded() -> Self {
        Self {
            schema: DEEP_CUT_PROFILE_SCHEMA_V1.into(),
            contract: COMPETITION_CONTRACT_V1.into(),
            role_id: DEEP_CUT_ROLE_ID.into(),
            profile_version: DEEP_CUT_PROFILE_VERSION_V1.into(),
            prompt_version: DEEP_CUT_PROMPT_VERSION_V1.into(),
            prompt_sha256: crate::knowledge::cut::sha256_hex(DEEP_CUT_PROMPT_V1.as_bytes()),
            capture_policy_version: "deep-cut-capture/v1".into(),
            reward_policy_version: "deep-cut-official-reward/v1".into(),
            pattern_policy_version: "deep-cut-field-pattern/v1".into(),
            default_lane: LaneIdV1::DeepCut,
            episode_start_required_before_effects: true,
            retain_negative_outcomes: true,
            replay_requires_new_episode: true,
        }
    }

    pub(crate) fn identity(&self) -> Result<DeepCutProfileIdentityV1, String> {
        self.validate()?;
        Ok(DeepCutProfileIdentityV1 {
            schema: DEEP_CUT_PROFILE_SCHEMA_V1.into(),
            role_id: DEEP_CUT_ROLE_ID.into(),
            profile_version: DEEP_CUT_PROFILE_VERSION_V1.into(),
            policy_sha256: DEEP_CUT_POLICY_SHA256_V1.into(),
        })
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let digest = serde_json::to_vec(self)
            .map(|body| crate::knowledge::cut::sha256_hex(&body))
            .map_err(|error| format!("encode deep-cut profile policy: {error}"))?;
        if self.schema != DEEP_CUT_PROFILE_SCHEMA_V1
            || self.contract != COMPETITION_CONTRACT_V1
            || self.role_id != DEEP_CUT_ROLE_ID
            || self.profile_version != DEEP_CUT_PROFILE_VERSION_V1
            || self.prompt_version != DEEP_CUT_PROMPT_VERSION_V1
            || self.prompt_sha256
                != crate::knowledge::cut::sha256_hex(DEEP_CUT_PROMPT_V1.as_bytes())
            || self.default_lane != LaneIdV1::DeepCut
            || !self.episode_start_required_before_effects
            || !self.retain_negative_outcomes
            || !self.replay_requires_new_episode
            || digest != DEEP_CUT_POLICY_SHA256_V1
        {
            return Err("deep-cut profile does not match embedded v1 policy".into());
        }
        Ok(())
    }
}

impl DeepCutProfileIdentityV1 {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != DEEP_CUT_PROFILE_SCHEMA_V1
            || self.role_id != DEEP_CUT_ROLE_ID
            || self.profile_version != DEEP_CUT_PROFILE_VERSION_V1
            || self.policy_sha256 != DEEP_CUT_POLICY_SHA256_V1
        {
            return Err("untrusted deep-cut profile identity".into());
        }
        Ok(())
    }
}

impl EpisodeStartedV1 {
    pub(crate) fn canonical_start_payload_sha256(&self) -> Result<String, String> {
        #[derive(Serialize)]
        struct Material<'a> {
            schema: &'static str,
            intent_version: &'static str,
            receipt: EpisodeStartMaterialV1<'a>,
        }
        serde_json::to_vec(&Material {
            schema: DEEP_CUT_EPISODE_SCHEMA_V1,
            intent_version: DEEP_CUT_START_INTENT_VERSION_V1,
            receipt: EpisodeStartMaterialV1::from(self),
        })
        .map(|body| crate::knowledge::cut::sha256_hex(&body))
        .map_err(|error| format!("encode episode start payload: {error}"))
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        self.profile.validate()?;
        self.competition.validate().map_err(str::to_string)?;
        self.objective.validate().map_err(str::to_string)?;
        for (value, label) in [
            (&self.episode_id, "invalid episode id"),
            (&self.campaign_id, "invalid campaign id"),
            (&self.hypothesis_id, "invalid hypothesis id"),
            (&self.source.base_id, "invalid source base id"),
            (&self.source.commit_oid, "invalid source commit"),
            (&self.source.tree_oid, "invalid source tree"),
            (&self.worker_instance_id, "invalid worker id"),
            (&self.model_id, "invalid model id"),
            (&self.requested_route, "invalid route"),
        ] {
            validate_id(value, label).map_err(str::to_string)?;
        }
        for digest in [
            &self.board_decision_sha256,
            &self.source.workspace_sha256,
            &self.source.access_proof_sha256,
            &self.source.observation_sha256,
            &self.tool_strategy_sha256,
        ] {
            validate_sha256(digest).map_err(str::to_string)?;
        }
        if self.source.source_board_epoch != self.board_epoch {
            return Err("source lineage must bind the current board epoch".into());
        }
        self.validate_replay()?;
        self.validate_start_journal_event()
    }

    fn validate_replay(&self) -> Result<(), String> {
        match (&self.origin_episode_id, &self.replay) {
            (None, None) => Ok(()),
            (Some(origin), Some(replay)) => {
                validate_id(origin, "invalid origin episode id").map_err(str::to_string)?;
                validate_id(&replay.replay_of_episode_id, "invalid replay episode id")
                    .map_err(str::to_string)?;
                validate_id(
                    &replay.preserved_hypothesis_id,
                    "invalid preserved hypothesis id",
                )
                .map_err(str::to_string)?;
                if let Some(candidate) = &replay.replay_of_candidate_id {
                    validate_id(candidate, "invalid replay candidate id")
                        .map_err(str::to_string)?;
                }
                if origin != &replay.replay_of_episode_id
                    || origin == &self.episode_id
                    || replay.preserved_hypothesis_id != self.hypothesis_id
                    || replay.predecessor_board_epoch >= self.board_epoch
                {
                    return Err("invalid replay ancestry".into());
                }
                Ok(())
            }
            _ => Err("origin and replay ancestry must appear together".into()),
        }
    }

    fn validate_start_journal_event(&self) -> Result<(), String> {
        let event = &self.start_journal_event;
        let update = &event.update;
        let intent = &update.intent;
        intent.validate().map_err(str::to_string)?;
        let previous_ok = (event.seq == 0 && event.previous_sha256.is_empty())
            || (event.seq > 0 && validate_sha256(&event.previous_sha256).is_ok());
        if event.schema != ACTION_JOURNAL_SCHEMA_V1
            || !previous_ok
            || event.event_sha256 != action_event_sha256(event)?
            || update.phase != ActionPhaseV1::Started
            || update.retryable
            || update.reconcile_key.is_some()
            || update.receipt_sha256.is_some()
            || update.next.is_some()
            || update.at_ms > self.started_at_ms
            || intent.kind != ActionKindV1::StartEpisode
            || intent.campaign_id != self.campaign_id
            || intent.competition != self.competition
            || intent.subject_id != self.episode_id
            || intent.intent_version != DEEP_CUT_START_INTENT_VERSION_V1
            || intent.payload_sha256 != self.canonical_start_payload_sha256()?
            || canonical_action_key(intent).map_err(|error| error.to_string())? != intent.action_key
        {
            return Err("invalid durable StartEpisode journal receipt".into());
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct EpisodeStartMaterialV1<'a> {
    episode_id: &'a str,
    campaign_id: &'a str,
    competition: &'a super::schema::CompetitionKeyV1,
    objective: &'a super::schema::ObjectiveComparatorV1,
    profile: &'a DeepCutProfileIdentityV1,
    board: (u64, u64, &'a str),
    source: &'a EpisodeSourceLineageV1,
    hypothesis_id: &'a str,
    replay: (&'a Option<String>, &'a Option<ReplayAncestryV1>),
    worker: (&'a str, &'a str, &'a str, &'a Option<String>),
    tool_strategy_sha256: &'a str,
    started_at_ms: u64,
}

impl<'a> From<&'a EpisodeStartedV1> for EpisodeStartMaterialV1<'a> {
    fn from(start: &'a EpisodeStartedV1) -> Self {
        Self {
            episode_id: &start.episode_id,
            campaign_id: &start.campaign_id,
            competition: &start.competition,
            objective: &start.objective,
            profile: &start.profile,
            board: (
                start.board_epoch,
                start.board_observation_revision,
                &start.board_decision_sha256,
            ),
            source: &start.source,
            hypothesis_id: &start.hypothesis_id,
            replay: (&start.origin_episode_id, &start.replay),
            worker: (
                &start.worker_instance_id,
                &start.model_id,
                &start.requested_route,
                &start.reasoning_effort,
            ),
            tool_strategy_sha256: &start.tool_strategy_sha256,
            started_at_ms: start.started_at_ms,
        }
    }
}

fn action_event_sha256(event: &ActionJournalEventV1) -> Result<String, String> {
    #[derive(Serialize)]
    struct Material<'a> {
        schema: &'a str,
        seq: u64,
        previous_sha256: &'a str,
        update: &'a super::journal::ActionUpdateV1,
    }
    serde_json::to_vec(&Material {
        schema: &event.schema,
        seq: event.seq,
        previous_sha256: &event.previous_sha256,
        update: &event.update,
    })
    .map(|body| crate::knowledge::cut::sha256_hex(&body))
    .map_err(|error| format!("encode StartEpisode journal receipt: {error}"))
}
