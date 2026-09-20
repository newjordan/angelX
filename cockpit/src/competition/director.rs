use super::board::{BoardFreshnessV1, CanonicalBoardV1};
use super::candidate_store::CandidateRepositoryV1;
use super::director_services::{
    DirectorEpisodeRefV1, DirectorServiceIntentV1, DirectorServiceKindV1, DirectorTransitionV1,
    DirectorWorkerRefV1,
};
use super::patterns::FieldPatternMemoryV1;
use super::rewards::RewardLedgerV1;
use super::schema::{
    COMPETITION_SCHEMA_V1, DirectorHealthStateV1, DirectorHealthV1, ScheduledActionV1,
};
use super::schema_validation::validate_id;
use super::submission::SubmissionSpoolV1;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const DIRECTOR_STATE_SCHEMA_V1: &str = "angel.competition-director-state/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompetitionDirectorStateV1 {
    pub(crate) schema: String,
    pub(crate) revision: u64,
    pub(crate) campaign_id: String,
    pub(crate) board: Option<CanonicalBoardV1>,
    pub(crate) candidates: Option<CandidateRepositoryV1>,
    pub(crate) submissions: SubmissionSpoolV1,
    pub(crate) rewards: RewardLedgerV1,
    pub(crate) patterns: FieldPatternMemoryV1,
    pub(crate) episodes: BTreeMap<String, DirectorEpisodeRefV1>,
    pub(crate) workers: BTreeMap<String, DirectorWorkerRefV1>,
    pub(crate) services: Vec<DirectorServiceIntentV1>,
    pub(crate) health: DirectorHealthV1,
}

impl CompetitionDirectorStateV1 {
    pub(crate) fn engage(campaign_id: &str, now_ms: u64) -> Result<Self, String> {
        validate_id(campaign_id, "invalid director campaign").map_err(str::to_string)?;
        let refresh = DirectorServiceIntentV1::new(
            campaign_id,
            DirectorServiceKindV1::RefreshBoard { full: true },
            now_ms,
        )?;
        let context = DirectorServiceIntentV1::new(
            campaign_id,
            DirectorServiceKindV1::ContinueContext,
            now_ms,
        )?;
        let state = Self {
            schema: DIRECTOR_STATE_SCHEMA_V1.into(),
            revision: 1,
            campaign_id: campaign_id.into(),
            board: None,
            candidates: None,
            submissions: SubmissionSpoolV1::new(),
            rewards: RewardLedgerV1::default(),
            patterns: FieldPatternMemoryV1::default(),
            episodes: BTreeMap::new(),
            workers: BTreeMap::new(),
            services: vec![refresh],
            health: health(
                DirectorHealthStateV1::Retrying,
                None,
                Some("engagement awaits initial board observation"),
                "full_board_refresh",
                now_ms,
            )?,
        };
        let mut state = state;
        state.upsert_service(context);
        state.validate()?;
        Ok(state)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != DIRECTOR_STATE_SCHEMA_V1 || self.revision == 0 {
            return Err("invalid director state schema or revision".into());
        }
        validate_id(&self.campaign_id, "invalid director campaign").map_err(str::to_string)?;
        self.health.validate().map_err(str::to_string)?;
        if self
            .health
            .last_good_revision
            .is_some_and(|value| value > self.revision)
        {
            return Err("director last-good revision moved ahead".into());
        }
        self.validate_board_and_candidates()?;
        self.submissions.validate()?;
        self.rewards.validate()?;
        self.patterns.validate()?;
        self.validate_references()?;
        super::director_validation::validate_cross_composition(self)?;
        let mut service_ids = BTreeSet::new();
        let mut service_classes = BTreeSet::new();
        for service in &self.services {
            service.validate()?;
            if service.campaign_id != self.campaign_id
                || !service_ids.insert(&service.intent_id)
                || !service_classes.insert(service.class())
            {
                return Err("duplicate or cross-campaign director service".into());
            }
        }
        Ok(())
    }

    fn validate_board_and_candidates(&self) -> Result<(), String> {
        super::director_validation::validate_board_and_candidates(self)
    }

    fn validate_references(&self) -> Result<(), String> {
        for (id, episode) in &self.episodes {
            episode.validate()?;
            if id != &episode.episode_id {
                return Err("director episode reference key mismatch".into());
            }
            super::director_results::validate_episode_join(self, episode)?;
        }
        for (id, worker) in &self.workers {
            worker.validate()?;
            if id != &worker.lease_id {
                return Err("director worker reference key mismatch".into());
            }
        }
        Ok(())
    }

    pub(super) fn upsert_service(&mut self, intent: DirectorServiceIntentV1) {
        self.services.retain(|item| item.class() != intent.class());
        self.services.push(intent);
        self.services.sort_by(|left, right| {
            left.due_at_ms
                .cmp(&right.due_at_ms)
                .then_with(|| left.class().cmp(right.class()))
        });
    }

    pub(super) fn remove_service(&mut self, class: &str) {
        self.services.retain(|item| item.class() != class);
    }

    pub(super) fn commit(
        &mut self,
        mut next: Self,
        health_state: DirectorHealthStateV1,
        reason: Option<&str>,
        action: &str,
        at_ms: u64,
    ) -> Result<DirectorTransitionV1, String> {
        next.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| "director revision exhausted".to_string())?;
        let last_good = if next
            .board
            .as_ref()
            .is_some_and(|board| matches!(board.freshness, BoardFreshnessV1::Fresh))
        {
            Some(next.revision)
        } else {
            self.health.last_good_revision.or_else(|| {
                self.board.as_ref().and_then(|board| {
                    matches!(board.freshness, BoardFreshnessV1::Fresh).then_some(self.revision)
                })
            })
        };
        next.health = health(health_state, last_good, reason, action, at_ms)?;
        next.validate()?;
        let transition = DirectorTransitionV1 {
            health: next.health.clone(),
            scheduled: next.services.clone(),
        };
        *self = next;
        Ok(transition)
    }

    pub(super) fn commit_preserving_health(
        &mut self,
        mut next: Self,
    ) -> Result<DirectorTransitionV1, String> {
        next.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| "director revision exhausted".to_string())?;
        next.health = self.health.clone();
        next.validate()?;
        let transition = DirectorTransitionV1 {
            health: next.health.clone(),
            scheduled: next.services.clone(),
        };
        *self = next;
        Ok(transition)
    }
}

fn health(
    state: DirectorHealthStateV1,
    last_good_revision: Option<u64>,
    reason: Option<&str>,
    action: &str,
    at_ms: u64,
) -> Result<DirectorHealthV1, String> {
    let value = DirectorHealthV1 {
        schema: COMPETITION_SCHEMA_V1.into(),
        state,
        last_good_revision,
        reason: reason.map(str::to_string),
        next: ScheduledActionV1 {
            action: action.into(),
            next_attempt_at_ms: at_ms,
        },
        updated_at_ms: at_ms,
    };
    value.validate().map_err(str::to_string)?;
    Ok(value)
}
