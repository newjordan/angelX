use super::*;
use crate::drive::competition::board::SourceAccessClaimV1;
use crate::drive::competition::schema::ComparatorKindV1;

fn comparator() -> ObjectiveComparatorV1 {
    ObjectiveComparatorV1 {
        objective_id: "score".into(),
        version: "1".into(),
        kind: ComparatorKindV1::HigherIsBetter,
    }
}

fn entry(id: &str, score: &str, personal: bool, source: bool) -> BoardEntryV1 {
    BoardEntryV1 {
        entry_id: id.into(),
        participant_id: format!("participant-{id}"),
        submission_id: Some(format!("submission-{id}")),
        rank: None,
        score: ScoreV1::new(score).unwrap(),
        personal,
        source: source.then(|| SourceAccessClaimV1 {
            source_id: format!("source-{id}"),
            commit_oid: format!("commit-{id}"),
            tree_oid: format!("tree-{id}"),
            workspace_sha256: crate::knowledge::cut::sha256_hex(
                format!("workspace-{id}").as_bytes(),
            ),
            access_proof_sha256: crate::knowledge::cut::sha256_hex(
                format!("proof-{id}").as_bytes(),
            ),
        }),
    }
}

fn reduce(entries: &[BoardEntryV1], epoch: u64, observation: &str) -> FrontierV1 {
    reduce_frontier(
        entries,
        &comparator(),
        epoch,
        observation,
        |contract, candidate, baseline| Ok(contract.compare(candidate, baseline).unwrap()),
    )
    .unwrap()
}

/// FG-BASE-020 / AC-04.
#[test]
fn global_personal_and_accessible_frontiers_remain_distinct() {
    let frontier = reduce(
        &[
            entry("external", "100", false, false),
            entry("personal", "90", true, false),
            entry("accessible", "80", false, true),
        ],
        1,
        "observation-one",
    );
    assert_eq!(frontier.global_target.unwrap().entry_id, "external");
    assert_eq!(frontier.personal_best.unwrap().entry_id, "personal");
    assert_eq!(
        frontier.source_accessible_base.unwrap().entry_id,
        "accessible"
    );
}

/// FG-BASE-021 / AC-04.
#[test]
fn inaccessible_global_leader_changes_target_not_source_base() {
    let first = reduce(
        &[
            entry("external-old", "100", false, false),
            entry("accessible", "80", false, true),
        ],
        1,
        "observation-one",
    );
    let moved = reduce(
        &[
            entry("external-new", "110", false, false),
            entry("accessible", "80", false, true),
        ],
        2,
        "observation-two",
    );
    assert_eq!(
        moved.global_target.as_ref().unwrap().entry_id,
        "external-new"
    );
    assert_eq!(
        first.source_accessible_base.as_ref().unwrap().source,
        moved.source_accessible_base.as_ref().unwrap().source
    );
    assert_eq!(
        moved.source_accessible_base.as_ref().unwrap().entry_id,
        "accessible"
    );
    assert!(
        !first.same_decision(&moved),
        "target movement advances decision"
    );
}

#[test]
fn source_selection_metadata_is_not_part_of_decision_digest() {
    let entries = [entry("accessible", "80", true, true)];
    let first = reduce(&entries, 1, "observation-one");
    let refresh = reduce(&entries, 99, "observation-later");
    assert!(first.same_decision(&refresh));
    assert_eq!(
        first.decision_sha256(&comparator()).unwrap(),
        refresh.decision_sha256(&comparator()).unwrap()
    );
}
