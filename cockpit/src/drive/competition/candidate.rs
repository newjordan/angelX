use super::board::{CANONICAL_BOARD_SCHEMA_V1, CanonicalBoardV1, SourceAccessClaimV1};
use super::frontier::{GlobalTargetV1, PersonalBestV1, SourceAccessibleBaseV1};
use super::schema::{CompetitionKeyV1, ObjectiveComparatorV1};
use super::schema_validation::{validate_id, validate_sha256};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const CANDIDATE_SCHEMA_V1: &str = "angel.competition-candidate/v1";
pub(crate) type CandidateCatalogV1 = BTreeMap<String, CandidateV1>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateArtifactV1 {
    pub(crate) commit_oid: String,
    pub(crate) tree_oid: String,
    pub(crate) workspace_sha256: String,
    pub(crate) mechanism_tags: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateEvidenceV1 {
    pub(crate) build_receipt_sha256: String,
    pub(crate) correctness_receipt_sha256: String,
    pub(crate) local_benchmark_receipt_sha256: Option<String>,
    pub(crate) uncertainty_sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateBoardContextV1 {
    pub(crate) board_epoch: u64,
    pub(crate) board_decision_sha256: String,
    pub(crate) global_target: Option<GlobalTargetV1>,
    pub(crate) personal_best: Option<PersonalBestV1>,
    pub(crate) source_accessible_base: SourceAccessibleBaseV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateV1 {
    pub(crate) schema: String,
    pub(crate) candidate_id: String,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) hypothesis_id: String,
    pub(crate) origin_episode_id: String,
    pub(crate) producing_episode_id: String,
    pub(crate) parent_candidate_id: Option<String>,
    pub(crate) ancestor_candidate_ids: Vec<String>,
    pub(crate) from_board_epoch: Option<u64>,
    pub(crate) board: CandidateBoardContextV1,
    pub(crate) artifact: CandidateArtifactV1,
    pub(crate) evidence: CandidateEvidenceV1,
    pub(crate) record_sha256: String,
}

pub(crate) fn originate_candidate(
    catalog: &CandidateCatalogV1,
    candidate_id: &str,
    hypothesis_id: &str,
    episode_id: &str,
    board: &CanonicalBoardV1,
    artifact: CandidateArtifactV1,
    evidence: CandidateEvidenceV1,
) -> Result<CandidateV1, String> {
    validate_candidate_catalog(catalog)?;
    if catalog.contains_key(candidate_id) {
        return Err("candidate id already exists".into());
    }
    let mut candidate = CandidateV1 {
        schema: CANDIDATE_SCHEMA_V1.into(),
        candidate_id: candidate_id.into(),
        competition: board.competition.clone(),
        objective: board.comparator.clone(),
        hypothesis_id: hypothesis_id.into(),
        origin_episode_id: episode_id.into(),
        producing_episode_id: episode_id.into(),
        parent_candidate_id: None,
        ancestor_candidate_ids: Vec::new(),
        from_board_epoch: None,
        board: context_from_board(board)?,
        artifact,
        evidence,
        record_sha256: String::new(),
    };
    seal(&mut candidate)?;
    super::candidate_lineage::validate_lineage(&candidate, catalog)?;
    Ok(candidate)
}

pub(crate) fn port_replay_candidate(
    catalog: &CandidateCatalogV1,
    candidate_id: &str,
    parent_id: &str,
    producing_episode_id: &str,
    board: &CanonicalBoardV1,
    artifact: CandidateArtifactV1,
    evidence: CandidateEvidenceV1,
) -> Result<CandidateV1, String> {
    validate_candidate_catalog(catalog)?;
    if catalog.contains_key(candidate_id) {
        return Err("candidate id already exists".into());
    }
    let parent = catalog
        .get(parent_id)
        .ok_or_else(|| "missing parent candidate".to_string())?;
    let target = context_from_board(board)?;
    if parent.competition != board.competition || parent.objective != board.comparator {
        return Err("incompatible competition or comparator".into());
    }
    if target.board_epoch <= parent.board.board_epoch {
        return Err("replay requires a newer board epoch".into());
    }
    let mut ancestors = parent.ancestor_candidate_ids.clone();
    ancestors.push(parent.candidate_id.clone());
    let mut candidate = CandidateV1 {
        schema: CANDIDATE_SCHEMA_V1.into(),
        candidate_id: candidate_id.into(),
        competition: parent.competition.clone(),
        objective: parent.objective.clone(),
        hypothesis_id: parent.hypothesis_id.clone(),
        origin_episode_id: parent.origin_episode_id.clone(),
        producing_episode_id: producing_episode_id.into(),
        parent_candidate_id: Some(parent.candidate_id.clone()),
        ancestor_candidate_ids: ancestors,
        from_board_epoch: Some(parent.board.board_epoch),
        board: target,
        artifact,
        evidence,
        record_sha256: String::new(),
    };
    seal(&mut candidate)?;
    super::candidate_lineage::validate_lineage(&candidate, catalog)?;
    Ok(candidate)
}

pub(crate) fn validate_candidate_catalog(catalog: &CandidateCatalogV1) -> Result<(), String> {
    super::candidate_lineage::validate_candidate_catalog(catalog)
}

impl CandidateV1 {
    pub(crate) fn validate_record(&self) -> Result<(), String> {
        if self.schema != CANDIDATE_SCHEMA_V1 || self.record_sha256 != self.canonical_sha256()? {
            return Err("invalid candidate schema or record digest".into());
        }
        self.competition.validate().map_err(str::to_string)?;
        self.objective.validate().map_err(str::to_string)?;
        for (id, label) in [
            (&self.candidate_id, "invalid candidate id"),
            (&self.hypothesis_id, "invalid hypothesis id"),
            (&self.origin_episode_id, "invalid origin episode id"),
            (&self.producing_episode_id, "invalid producing episode id"),
        ] {
            validate_id(id, label).map_err(str::to_string)?;
        }
        validate_context(&self.board)?;
        validate_artifact(&self.artifact, &self.evidence)?;
        Ok(())
    }

    fn canonical_sha256(&self) -> Result<String, String> {
        let mut canonical = self.clone();
        canonical.record_sha256.clear();
        serde_json::to_vec(&canonical)
            .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
            .map_err(|error| format!("encode candidate record: {error}"))
    }
}

fn context_from_board(board: &CanonicalBoardV1) -> Result<CandidateBoardContextV1, String> {
    if board.schema != CANONICAL_BOARD_SCHEMA_V1
        || board.board_epoch == 0
        || board.frontier.decision_sha256(&board.comparator)? != board.decision_sha256
    {
        return Err("invalid canonical board".into());
    }
    board.competition.validate().map_err(str::to_string)?;
    board.comparator.validate().map_err(str::to_string)?;
    let source = board
        .frontier
        .source_accessible_base
        .clone()
        .ok_or_else(|| "board has no source-accessible base".to_string())?;
    let entry = board
        .entries
        .iter()
        .find(|entry| entry.entry_id == source.entry_id)
        .ok_or_else(|| "selected source base is missing from board".to_string())?;
    if entry.source.as_ref() != Some(&source.source)
        || entry.score != source.score
        || source.source_board_epoch != board.board_epoch
    {
        return Err("selected source base is not source-accessible at this epoch".into());
    }
    Ok(CandidateBoardContextV1 {
        board_epoch: board.board_epoch,
        board_decision_sha256: board.decision_sha256.clone(),
        global_target: board.frontier.global_target.clone(),
        personal_best: board.frontier.personal_best.clone(),
        source_accessible_base: source,
    })
}

fn validate_context(context: &CandidateBoardContextV1) -> Result<(), String> {
    let base = &context.source_accessible_base;
    let source: &SourceAccessClaimV1 = &base.source;
    if base.source_board_epoch != context.board_epoch {
        return Err("candidate source epoch mismatch".into());
    }
    validate_sha256(&context.board_decision_sha256).map_err(str::to_string)?;
    validate_id(&source.source_id, "invalid source id").map_err(str::to_string)?;
    validate_object_oid(&source.commit_oid, "invalid source commit")?;
    validate_object_oid(&source.tree_oid, "invalid source tree")?;
    validate_sha256(&source.workspace_sha256).map_err(str::to_string)?;
    validate_sha256(&source.access_proof_sha256).map_err(str::to_string)
}

fn validate_artifact(
    artifact: &CandidateArtifactV1,
    evidence: &CandidateEvidenceV1,
) -> Result<(), String> {
    validate_object_oid(&artifact.commit_oid, "invalid candidate commit")?;
    validate_object_oid(&artifact.tree_oid, "invalid candidate tree")?;
    validate_sha256(&artifact.workspace_sha256).map_err(str::to_string)?;
    if artifact.mechanism_tags.is_empty() {
        return Err("candidate requires a mechanism tag".into());
    }
    for tag in &artifact.mechanism_tags {
        validate_id(tag, "invalid mechanism tag").map_err(str::to_string)?;
    }
    for digest in [
        Some(&evidence.build_receipt_sha256),
        Some(&evidence.correctness_receipt_sha256),
        evidence.local_benchmark_receipt_sha256.as_ref(),
        evidence.uncertainty_sha256.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_sha256(digest).map_err(str::to_string)?;
    }
    Ok(())
}

fn validate_object_oid(value: &str, error: &str) -> Result<(), String> {
    let sha1 = value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    (validate_sha256(value).is_ok() || sha1)
        .then_some(())
        .ok_or_else(|| error.to_string())
}

fn seal(candidate: &mut CandidateV1) -> Result<(), String> {
    candidate.record_sha256 = candidate.canonical_sha256()?;
    candidate.validate_record()
}
