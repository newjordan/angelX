use super::board::{
    BoardEntryV1, BoardFreshnessV1, CANONICAL_BOARD_SCHEMA_V1, CanonicalBoardV1,
    ObservationProvenanceV1, ObservationSourceV1, SourceAccessClaimV1,
};
use super::candidate::*;
use super::frontier::reduce_frontier;
use super::schema::{
    AdapterCapabilityV1, AdapterIdentityV1, ComparatorKindV1, CompetitionKeyV1,
    ObjectiveComparatorV1, ScoreV1,
};

pub(super) fn digest(value: &str) -> String {
    crate::knowledge::cut::sha256_hex(value.as_bytes())
}

fn key() -> CompetitionKeyV1 {
    CompetitionKeyV1 {
        platform_id: "fixture".into(),
        competition_id: "contest".into(),
        field_id: "kernels".into(),
        benchmark_id: "gemm".into(),
        profile_id: "official".into(),
        hardware_id: "gpu".into(),
    }
}

fn comparator() -> ObjectiveComparatorV1 {
    ObjectiveComparatorV1 {
        objective_id: "throughput".into(),
        version: "1".into(),
        kind: ComparatorKindV1::HigherIsBetter,
    }
}

fn claim(id: &str) -> SourceAccessClaimV1 {
    SourceAccessClaimV1 {
        source_id: id.into(),
        commit_oid: digest(&format!("{id}-commit")),
        tree_oid: digest(&format!("{id}-tree")),
        workspace_sha256: digest(&format!("{id}-workspace")),
        access_proof_sha256: digest(&format!("{id}-proof")),
    }
}

fn entry(
    id: &str,
    score: &str,
    personal: bool,
    source: Option<SourceAccessClaimV1>,
) -> BoardEntryV1 {
    BoardEntryV1 {
        entry_id: id.into(),
        participant_id: format!("participant-{id}"),
        submission_id: Some(format!("submission-{id}")),
        rank: None,
        score: ScoreV1::new(score).unwrap(),
        personal,
        source,
    }
}

pub(super) fn board(
    epoch: u64,
    global_score: &str,
    source_id: &str,
    source_score: &str,
) -> CanonicalBoardV1 {
    let global = entry("global", global_score, false, None);
    let personal = entry("personal", "80", true, None);
    let source = entry(source_id, source_score, false, Some(claim(source_id)));
    let objective = comparator();
    let entries = vec![global, personal, source];
    let frontier = reduce_frontier(
        &entries,
        &objective,
        epoch,
        &format!("observation-{epoch}"),
        |comparator, candidate, baseline| Ok(comparator.compare(candidate, baseline).unwrap()),
    )
    .unwrap();
    let decision_sha256 = frontier.decision_sha256(&objective).unwrap();
    CanonicalBoardV1 {
        schema: CANONICAL_BOARD_SCHEMA_V1.into(),
        campaign_id: "campaign".into(),
        competition: key(),
        comparator: objective,
        board_epoch: epoch,
        predecessor_epoch: (epoch > 1).then_some(epoch - 1),
        observation_revision: epoch,
        entries,
        frontier,
        decision_sha256,
        latest_observation_id: format!("observation-{epoch}"),
        latest_provenance: ObservationProvenanceV1 {
            adapter: AdapterIdentityV1 {
                adapter_id: "fixture".into(),
                adapter_version: "1".into(),
                runtime_sha256: digest("adapter"),
                capabilities: [AdapterCapabilityV1::Board].into_iter().collect(),
            },
            source: ObservationSourceV1::Fixture,
            observed_at_ms: epoch,
            platform_event_at_ms: None,
            raw_sha256: digest(&format!("raw-{epoch}")),
        },
        last_sequence: Some(epoch),
        freshness: BoardFreshnessV1::Fresh,
    }
}

pub(super) fn artifact(id: &str) -> CandidateArtifactV1 {
    CandidateArtifactV1 {
        commit_oid: digest(&format!("{id}-commit")),
        tree_oid: digest(&format!("{id}-tree")),
        workspace_sha256: digest(&format!("{id}-workspace")),
        mechanism_tags: ["structural-cut".into()].into_iter().collect(),
    }
}

pub(super) fn evidence(id: &str) -> CandidateEvidenceV1 {
    CandidateEvidenceV1 {
        build_receipt_sha256: digest(&format!("{id}-build")),
        correctness_receipt_sha256: digest(&format!("{id}-correctness")),
        local_benchmark_receipt_sha256: Some(digest(&format!("{id}-local"))),
        uncertainty_sha256: Some(digest(&format!("{id}-uncertainty"))),
    }
}

pub(super) fn origin(catalog: &CandidateCatalogV1, board: &CanonicalBoardV1) -> CandidateV1 {
    originate_candidate(
        catalog,
        "candidate-origin",
        "hypothesis-1",
        "episode-origin",
        board,
        artifact("origin"),
        evidence("origin"),
    )
    .unwrap()
}

pub(super) fn reseal(candidate: &mut CandidateV1) {
    candidate.record_sha256.clear();
    candidate.record_sha256 =
        digest(std::str::from_utf8(&serde_json::to_vec(candidate).unwrap()).unwrap());
}

#[test]
fn fg_base_020_global_personal_and_accessible_frontiers_remain_distinct() {
    let candidate = origin(
        &CandidateCatalogV1::new(),
        &board(1, "100", "source-a", "60"),
    );
    let context = &candidate.board;
    assert_eq!(
        (
            context.global_target.as_ref().unwrap().entry_id.as_str(),
            context.personal_best.as_ref().unwrap().entry_id.as_str(),
            context.source_accessible_base.entry_id.as_str(),
            context.source_accessible_base.score.as_str(),
        ),
        ("global", "personal", "source-a", "60")
    );
}

#[test]
fn fg_base_021_inaccessible_global_leader_changes_target_not_source_base() {
    let first_board = board(1, "100", "source-a", "60");
    let mut catalog = CandidateCatalogV1::new();
    let candidate = origin(&catalog, &first_board);
    catalog.insert(candidate.candidate_id.clone(), candidate.clone());
    let moved = board(2, "110", "source-a", "60");
    assert_eq!(
        moved
            .frontier
            .global_target
            .as_ref()
            .unwrap()
            .score
            .as_str(),
        "110"
    );
    assert!(
        port_replay_candidate(
            &catalog,
            "unneeded-port",
            &candidate.candidate_id,
            "episode-replay",
            &moved,
            artifact("unneeded"),
            evidence("unneeded"),
        )
        .is_err()
    );
    assert_eq!(candidate.board.source_accessible_base.entry_id, "source-a");
}

#[test]
fn fg_base_022_inferior_base_candidate_creates_port_descendant_preserving_hypothesis() {
    let mut catalog = CandidateCatalogV1::new();
    let parent = origin(&catalog, &board(1, "100", "source-old", "60"));
    catalog.insert(parent.candidate_id.clone(), parent.clone());
    let child = port_replay_candidate(
        &catalog,
        "candidate-port",
        &parent.candidate_id,
        "episode-port",
        &board(2, "110", "source-new", "70"),
        artifact("port"),
        evidence("port"),
    )
    .unwrap();
    assert_eq!(
        child.parent_candidate_id.as_deref(),
        Some("candidate-origin")
    );
    assert_eq!(child.ancestor_candidate_ids, ["candidate-origin"]);
    assert_eq!(child.hypothesis_id, parent.hypothesis_id);
    assert_eq!(child.origin_episode_id, parent.origin_episode_id);
    assert_eq!(child.producing_episode_id, "episode-port");
    assert_eq!(child.from_board_epoch, Some(1));
    assert_eq!(parent.board.source_accessible_base.entry_id, "source-old");
}

#[test]
fn dc_ep_005_board_move_creates_replay_descendant_and_preserves_ancestor() {
    let mut catalog = CandidateCatalogV1::new();
    let ancestor = origin(&catalog, &board(1, "100", "source-a", "50"));
    catalog.insert(ancestor.candidate_id.clone(), ancestor.clone());
    let child = port_replay_candidate(
        &catalog,
        "candidate-child",
        &ancestor.candidate_id,
        "episode-child",
        &board(2, "110", "source-b", "60"),
        artifact("child"),
        evidence("child"),
    )
    .unwrap();
    catalog.insert(child.candidate_id.clone(), child.clone());
    let grandchild = port_replay_candidate(
        &catalog,
        "candidate-grandchild",
        &child.candidate_id,
        "episode-grandchild",
        &board(3, "120", "source-c", "70"),
        artifact("grandchild"),
        evidence("grandchild"),
    )
    .unwrap();
    assert_eq!(
        grandchild.ancestor_candidate_ids,
        ["candidate-origin", "candidate-child"]
    );
    assert_eq!(grandchild.hypothesis_id, ancestor.hypothesis_id);
    assert_eq!(catalog.get("candidate-origin"), Some(&ancestor));
}
