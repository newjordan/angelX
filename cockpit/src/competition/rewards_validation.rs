use super::rewards::{
    BindDispositionV1, OFFICIAL_REWARD_SCHEMA_V1, OfficialResultSourceV1, OfficialResultV1,
    OfficialRewardBindingV1, ProvisionalShapingV1, RewardContextV1, RewardLedgerV1, make_binding,
};
use super::schema::AdapterCapabilityV1;
use super::schema_validation::{validate_id, validate_sha256};

pub(super) fn validate_context(context: &RewardContextV1) -> Result<(), String> {
    for (value, label) in [
        (&context.episode_id, "invalid episode id"),
        (&context.candidate_id, "invalid candidate id"),
        (&context.submission_id, "invalid submission id"),
        (&context.comparable_base_id, "invalid comparable base id"),
    ] {
        validate_id(value, label).map_err(str::to_string)?;
    }
    context.competition.validate().map_err(str::to_string)?;
    context.objective.validate().map_err(str::to_string)?;
    context.profile.validate()
}

pub(super) fn validate_result(
    context: &RewardContextV1,
    result: &OfficialResultV1,
) -> Result<(), String> {
    validate_id(&result.result_id, "invalid official result id").map_err(str::to_string)?;
    result
        .provenance
        .adapter
        .validate()
        .map_err(str::to_string)?;
    validate_sha256(&result.provenance.receipt_sha256).map_err(str::to_string)?;
    if result.result_revision == 0
        || result.provenance.source != OfficialResultSourceV1::OfficialSubmissionResult
        || !result
            .provenance
            .adapter
            .capabilities
            .contains(&AdapterCapabilityV1::Results)
    {
        return Err("result is not official submission evidence".into());
    }
    if result.episode_id != context.episode_id
        || result.candidate_id != context.candidate_id
        || result.submission_id != context.submission_id
        || result.comparable_base_id != context.comparable_base_id
        || result.competition != context.competition
        || result.objective != context.objective
        || result.profile != context.profile
    {
        return Err("official result is incompatible with reward context".into());
    }
    if let Some(id) = &result.corrects_binding_id {
        validate_sha256(id).map_err(str::to_string)?;
    }
    Ok(())
}

pub(super) fn context_from_binding(binding: &OfficialRewardBindingV1) -> RewardContextV1 {
    RewardContextV1 {
        episode_id: binding.episode_id.clone(),
        candidate_id: binding.candidate_id.clone(),
        submission_id: binding.submission_id.clone(),
        comparable_base_id: binding.comparable_base_id.clone(),
        competition: binding.competition.clone(),
        objective: binding.objective.clone(),
        profile: binding.profile.clone(),
    }
}

pub(super) fn result_from_binding(binding: &OfficialRewardBindingV1) -> OfficialResultV1 {
    OfficialResultV1 {
        result_id: binding.official_result_id.clone(),
        result_revision: binding.official_result_revision,
        episode_id: binding.episode_id.clone(),
        candidate_id: binding.candidate_id.clone(),
        submission_id: binding.submission_id.clone(),
        comparable_base_id: binding.comparable_base_id.clone(),
        competition: binding.competition.clone(),
        objective: binding.objective.clone(),
        profile: binding.profile.clone(),
        candidate_score: binding.candidate_score.clone(),
        base_score: binding.base_score.clone(),
        provenance: binding.official_provenance.clone(),
        corrects_binding_id: binding.corrects_binding_id.clone(),
    }
}

pub(super) fn same_reward_identity(
    left: &OfficialRewardBindingV1,
    right: &OfficialRewardBindingV1,
) -> bool {
    left.episode_id == right.episode_id
        && left.candidate_id == right.candidate_id
        && left.submission_id == right.submission_id
        && left.comparable_base_id == right.comparable_base_id
        && left.competition == right.competition
        && left.objective == right.objective
        && left.profile == right.profile
}

pub(super) fn same_official_event(
    left: &OfficialRewardBindingV1,
    right: &OfficialRewardBindingV1,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.binding_id.clear();
    right.binding_id.clear();
    left.official_result_id.clear();
    right.official_result_id.clear();
    left == right
}

pub(super) fn validate_ledger(
    shaping: &[ProvisionalShapingV1],
    bindings: &[OfficialRewardBindingV1],
) -> Result<(), String> {
    let mut replay = RewardLedgerV1::default();
    for event in shaping {
        if !replay.record_shaping(event.clone())? {
            return Err("duplicate shaping event in reward ledger".into());
        }
    }
    for binding in bindings {
        binding.validate()?;
        let (disposition, rebuilt) =
            replay.bind_official(&context_from_binding(binding), result_from_binding(binding))?;
        if disposition != BindDispositionV1::Appended || &rebuilt != binding {
            return Err("non-canonical official reward ledger".into());
        }
    }
    if replay.shaping() != shaping || replay.bindings() != bindings {
        return Err("reward ledger replay mismatch".into());
    }
    Ok(())
}

pub(super) fn validate_binding(binding: &OfficialRewardBindingV1) -> Result<(), String> {
    if binding.schema != OFFICIAL_REWARD_SCHEMA_V1 {
        return Err("unknown official reward schema".into());
    }
    validate_sha256(&binding.binding_id).map_err(str::to_string)?;
    let context = context_from_binding(binding);
    let result = result_from_binding(binding);
    validate_context(&context)?;
    validate_result(&context, &result)?;
    if make_binding(result)? != *binding {
        return Err("official reward binding digest or reduction mismatch".into());
    }
    Ok(())
}
