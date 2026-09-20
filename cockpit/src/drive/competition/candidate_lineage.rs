use super::candidate::{CandidateCatalogV1, CandidateV1};
use std::collections::BTreeSet;

pub(super) fn validate_candidate_catalog(catalog: &CandidateCatalogV1) -> Result<(), String> {
    for (id, candidate) in catalog {
        (id == &candidate.candidate_id)
            .then_some(())
            .ok_or("candidate catalog key mismatch")?;
        candidate.validate_record()?;
        validate_lineage(candidate, catalog)?;
    }
    Ok(())
}

pub(super) fn validate_lineage(
    candidate: &CandidateV1,
    catalog: &CandidateCatalogV1,
) -> Result<(), String> {
    let unique: BTreeSet<_> = candidate.ancestor_candidate_ids.iter().collect();
    if unique.len() != candidate.ancestor_candidate_ids.len()
        || candidate
            .ancestor_candidate_ids
            .contains(&candidate.candidate_id)
    {
        return Err("cyclic candidate ancestry".into());
    }
    match (candidate.from_board_epoch, &candidate.parent_candidate_id) {
        (None, None)
            if candidate.ancestor_candidate_ids.is_empty()
                && candidate.origin_episode_id == candidate.producing_episode_id =>
        {
            Ok(())
        }
        (Some(from_board_epoch), Some(parent_id)) => {
            let parent = catalog
                .get(parent_id)
                .ok_or_else(|| "missing parent candidate".to_string())?;
            parent
                .objective
                .compare(
                    &candidate.board.source_accessible_base.score,
                    &parent.board.source_accessible_base.score,
                )
                .map_err(|_| "adapter-defined comparator required for base movement".to_string())?;
            let mut expected = parent.ancestor_candidate_ids.clone();
            expected.push(parent.candidate_id.clone());
            let parent_base = &parent.board.source_accessible_base;
            let child_base = &candidate.board.source_accessible_base;
            let same_base = parent_base.entry_id == child_base.entry_id
                && parent_base.participant_id == child_base.participant_id
                && parent_base.submission_id == child_base.submission_id
                && parent_base.score == child_base.score
                && parent_base.source == child_base.source;
            let reused_artifact = parent.artifact.commit_oid == candidate.artifact.commit_oid
                && parent.artifact.tree_oid == candidate.artifact.tree_oid
                && parent.artifact.workspace_sha256 == candidate.artifact.workspace_sha256;
            if expected != candidate.ancestor_candidate_ids
                || parent_id == &candidate.candidate_id
                || from_board_epoch != parent.board.board_epoch
                || candidate.board.board_epoch <= parent.board.board_epoch
                || same_base
                || reused_artifact
                || candidate.evidence.build_receipt_sha256 == parent.evidence.build_receipt_sha256
                || candidate.evidence.correctness_receipt_sha256
                    == parent.evidence.correctness_receipt_sha256
                || candidate.competition != parent.competition
                || candidate.objective != parent.objective
                || candidate.hypothesis_id != parent.hypothesis_id
                || candidate.origin_episode_id != parent.origin_episode_id
                || candidate.producing_episode_id == parent.producing_episode_id
            {
                return Err("incompatible candidate lineage".into());
            }
            Ok(())
        }
        _ => Err("missing parent or origin lineage".into()),
    }
}
