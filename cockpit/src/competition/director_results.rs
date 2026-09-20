use super::director::CompetitionDirectorStateV1;
use super::director_services::{DirectorEpisodeRefV1, DirectorTransitionV1, DirectorWorkerRefV1};
use super::journal::ActionJournalStateV1;
use super::rewards::{BindDispositionV1, OfficialResultV1, RewardContextV1};
use super::submission_pump::SubmissionPumpDecisionV1;

impl CompetitionDirectorStateV1 {
    pub(crate) fn register_episode(
        &mut self,
        episode: DirectorEpisodeRefV1,
        _now_ms: u64,
    ) -> Result<DirectorTransitionV1, String> {
        episode.validate()?;
        self.validate()?;
        validate_episode_join(self, &episode)?;
        if self.episodes.get(&episode.episode_id) == Some(&episode) {
            return Ok(unchanged(self));
        }
        let mut next = self.clone();
        match next.episodes.get(&episode.episode_id) {
            Some(_) => return Err("director episode reference conflicts".into()),
            None => {
                next.episodes.insert(episode.episode_id.clone(), episode);
            }
        }
        self.commit_preserving_health(next)
    }

    pub(crate) fn register_worker(
        &mut self,
        worker: DirectorWorkerRefV1,
        _now_ms: u64,
    ) -> Result<DirectorTransitionV1, String> {
        worker.validate()?;
        self.validate()?;
        if self.workers.get(&worker.lease_id) == Some(&worker) {
            return Ok(unchanged(self));
        }
        let mut next = self.clone();
        match next.workers.get(&worker.lease_id) {
            Some(_) => return Err("director worker reference conflicts".into()),
            None => {
                next.workers.insert(worker.lease_id.clone(), worker);
            }
        }
        self.commit_preserving_health(next)
    }

    pub(crate) fn submission_step(
        &self,
        journal: &ActionJournalStateV1,
        now_ms: u64,
    ) -> Result<SubmissionPumpDecisionV1, String> {
        let candidates = self
            .candidates
            .as_ref()
            .ok_or_else(|| "submission service awaits a canonical board".to_string())?;
        self.submissions.decide(candidates, journal, now_ms)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind_result(
        &mut self,
        context: &RewardContextV1,
        result: OfficialResultV1,
        bottleneck_class: String,
        mechanism: String,
        promotes_evidence_id: Option<String>,
        uncertainty_millis: u16,
        cost_microunits: u64,
        now_ms: u64,
    ) -> Result<DirectorTransitionV1, String> {
        validate_result_join(self, context, &result)?;
        let mut next = self.clone();
        let (disposition, binding) = next.rewards.bind_official(context, result)?;
        let pattern_appended = next.patterns.record_official(
            &binding,
            bottleneck_class,
            mechanism,
            promotes_evidence_id,
            uncertainty_millis,
            cost_microunits,
        )?;
        let episode = next
            .episodes
            .get_mut(&context.episode_id)
            .ok_or_else(|| "reward episode attribution is absent".to_string())?;
        let already_attributed = episode.latest_official_result_id.as_ref()
            == Some(&binding.official_result_id)
            && episode.latest_reward_binding_sha256.as_ref() == Some(&binding.binding_id);
        episode.latest_official_result_id = Some(binding.official_result_id.clone());
        episode.latest_reward_binding_sha256 = Some(binding.binding_id);
        if disposition == BindDispositionV1::Duplicate && !pattern_appended && already_attributed {
            return Ok(DirectorTransitionV1 {
                health: self.health.clone(),
                scheduled: self.services.clone(),
            });
        }
        let _ = now_ms;
        self.commit_preserving_health(next)
    }
}

pub(super) fn validate_episode_join(
    director: &CompetitionDirectorStateV1,
    episode: &DirectorEpisodeRefV1,
) -> Result<(), String> {
    let candidates = director
        .candidates
        .as_ref()
        .ok_or_else(|| "episode attribution awaits candidate state".to_string())?;
    let candidate = candidates
        .catalog
        .get(&episode.candidate_id)
        .ok_or_else(|| "episode attribution candidate is absent".to_string())?;
    let submission = director
        .submissions
        .items
        .values()
        .find(|item| item.submission_id == episode.submission_id)
        .ok_or_else(|| "episode attribution submission is absent".to_string())?;
    if candidate.producing_episode_id != episode.episode_id
        || candidate.board.board_epoch != episode.board_epoch
        || submission.candidate_id != candidate.candidate_id
        || submission.candidate_record_sha256 != candidate.record_sha256
    {
        return Err("episode, candidate, submission, or board attribution mismatch".into());
    }
    Ok(())
}

fn validate_result_join(
    director: &CompetitionDirectorStateV1,
    context: &RewardContextV1,
    result: &OfficialResultV1,
) -> Result<(), String> {
    let episode = director
        .episodes
        .get(&context.episode_id)
        .ok_or_else(|| "reward episode attribution is absent".to_string())?;
    if episode.candidate_id != context.candidate_id
        || episode.submission_id != context.submission_id
        || result.episode_id != context.episode_id
    {
        return Err("reward result attribution identity mismatch".into());
    }
    let candidate = director
        .candidates
        .as_ref()
        .and_then(|repository| repository.catalog.get(&context.candidate_id))
        .ok_or_else(|| "reward candidate attribution is absent".to_string())?;
    let source_base = &candidate.board.source_accessible_base;
    if context.comparable_base_id != source_base.entry_id
        || result.comparable_base_id != source_base.entry_id
        || result.base_score != source_base.score
        || result.competition != candidate.competition
        || result.objective != candidate.objective
    {
        return Err("reward result is not bound to candidate source-base evidence".into());
    }
    let official = director
        .submissions
        .official
        .results
        .iter()
        .find(|entry| entry.result_id == result.result_id)
        .ok_or_else(|| "reward lacks submission-bound official result".to_string())?;
    if official.submission_id != result.submission_id
        || official.candidate_id != result.candidate_id
        || official.result_revision != result.result_revision
        || official.competition != result.competition
        || official.objective != result.objective
        || official.candidate_score != result.candidate_score
        || official.receipt_sha256 != result.provenance.receipt_sha256
    {
        return Err("official result does not match submission evidence".into());
    }
    match &result.corrects_binding_id {
        None if official.corrects_result_id.is_none() && result.result_revision == 1 => {}
        Some(parent_binding_id) => {
            let parent = director
                .rewards
                .bindings()
                .iter()
                .find(|binding| &binding.binding_id == parent_binding_id)
                .ok_or_else(|| "reward correction parent is absent".to_string())?;
            if official.corrects_result_id.as_deref() != Some(&parent.official_result_id)
                || parent.official_result_revision.checked_add(1) != Some(result.result_revision)
            {
                return Err("reward correction does not match official revision chain".into());
            }
        }
        _ => return Err("initial reward result does not match initial official result".into()),
    }
    Ok(())
}

fn unchanged(director: &CompetitionDirectorStateV1) -> DirectorTransitionV1 {
    DirectorTransitionV1 {
        health: director.health.clone(),
        scheduled: director.services.clone(),
    }
}
