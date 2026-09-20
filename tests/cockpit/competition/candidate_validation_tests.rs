use super::candidate::*;
use super::candidate_tests::{artifact, board, digest, evidence, origin, reseal};
use super::schema::{ComparatorKindV1, ObjectiveComparatorV1};

fn rotate_access_proof(board: &mut super::board::CanonicalBoardV1) {
    let proof = digest("rotated-access-proof");
    board
        .entries
        .iter_mut()
        .find_map(|entry| entry.source.as_mut())
        .unwrap()
        .access_proof_sha256 = proof.clone();
    board
        .frontier
        .source_accessible_base
        .as_mut()
        .unwrap()
        .source
        .access_proof_sha256 = proof;
    board.decision_sha256 = board.frontier.decision_sha256(&board.comparator).unwrap();
}

#[test]
fn lineage_rejects_missing_cyclic_incompatible_and_inaccessible_bases() {
    let old_board = board(1, "100", "source-a", "50");
    let mut catalog = CandidateCatalogV1::new();
    let ancestor = origin(&catalog, &old_board);
    catalog.insert(ancestor.candidate_id.clone(), ancestor.clone());
    assert!(
        port_replay_candidate(
            &catalog,
            "missing-child",
            "missing-parent",
            "episode-child",
            &board(2, "110", "source-b", "60"),
            artifact("missing"),
            evidence("missing"),
        )
        .is_err()
    );
    let mut incompatible = board(2, "110", "source-b", "60");
    incompatible.comparator.kind = ComparatorKindV1::LowerIsBetter;
    incompatible.decision_sha256 = incompatible
        .frontier
        .decision_sha256(&incompatible.comparator)
        .unwrap();
    assert!(
        port_replay_candidate(
            &catalog,
            "incompatible",
            &ancestor.candidate_id,
            "episode-child",
            &incompatible,
            artifact("incompatible"),
            evidence("incompatible"),
        )
        .is_err()
    );
    let mut inaccessible = old_board;
    inaccessible.frontier.source_accessible_base = None;
    inaccessible.decision_sha256 = inaccessible
        .frontier
        .decision_sha256(&inaccessible.comparator)
        .unwrap();
    assert!(
        originate_candidate(
            &catalog,
            "inaccessible",
            "hypothesis-2",
            "episode-2",
            &inaccessible,
            artifact("inaccessible"),
            evidence("inaccessible"),
        )
        .is_err()
    );
    let mut cyclic = ancestor;
    cyclic.candidate_id = "cycle".into();
    cyclic.parent_candidate_id = Some("cycle".into());
    cyclic.ancestor_candidate_ids = vec!["cycle".into()];
    cyclic.from_board_epoch = Some(1);
    reseal(&mut cyclic);
    catalog.insert("cycle".into(), cyclic);
    assert!(validate_candidate_catalog(&catalog).is_err());
}

#[test]
fn candidate_catalog_round_trip_preserves_validated_lineage() {
    let mut catalog = CandidateCatalogV1::new();
    let ancestor = origin(&catalog, &board(1, "100", "source-a", "60"));
    catalog.insert(ancestor.candidate_id.clone(), ancestor.clone());
    let child = port_replay_candidate(
        &catalog,
        "candidate-child",
        &ancestor.candidate_id,
        "episode-child",
        &board(2, "110", "source-b", "70"),
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
        &board(3, "120", "source-c", "80"),
        artifact("grandchild"),
        evidence("grandchild"),
    )
    .unwrap();
    catalog.insert(grandchild.candidate_id.clone(), grandchild);
    let bytes = serde_json::to_vec(&catalog).unwrap();
    let restored: CandidateCatalogV1 = serde_json::from_slice(&bytes).unwrap();
    validate_candidate_catalog(&restored).unwrap();
    assert_eq!(restored, catalog);
    let mut tampered = restored;
    tampered
        .get_mut("candidate-grandchild")
        .unwrap()
        .ancestor_candidate_ids
        .clear();
    assert!(validate_candidate_catalog(&tampered).is_err());
}

#[test]
fn port_requires_rebuilt_artifact_and_fresh_build_and_correctness_evidence() {
    let mut catalog = CandidateCatalogV1::new();
    let parent = origin(&catalog, &board(1, "100", "source-a", "60"));
    catalog.insert(parent.candidate_id.clone(), parent.clone());
    let before = catalog.clone();
    let mut moved = board(2, "100", "source-a", "60");
    rotate_access_proof(&mut moved);
    for (artifact, evidence) in [
        (parent.artifact.clone(), parent.evidence.clone()),
        (artifact("fresh-a"), parent.evidence.clone()),
        (parent.artifact.clone(), evidence("fresh-e")),
    ] {
        assert!(
            port_replay_candidate(
                &catalog,
                "stale-child",
                &parent.candidate_id,
                "episode-child",
                &moved,
                artifact,
                evidence,
            )
            .is_err()
        );
        assert_eq!(catalog, before);
    }
    let child = port_replay_candidate(
        &catalog,
        "fresh-child",
        &parent.candidate_id,
        "episode-child",
        &moved,
        artifact("fresh"),
        evidence("fresh"),
    )
    .unwrap();
    assert_eq!(child.hypothesis_id, parent.hypothesis_id);
    assert_eq!(child.ancestor_candidate_ids, [parent.candidate_id]);
    assert_eq!(catalog, before);
}

#[test]
fn duplicate_key_mismatch_and_adapter_required_fail_without_catalog_mutation() {
    let mut catalog = CandidateCatalogV1::new();
    let parent = origin(&catalog, &board(1, "100", "source-a", "60"));
    catalog.insert(parent.candidate_id.clone(), parent.clone());
    let before = catalog.clone();
    assert!(
        port_replay_candidate(
            &catalog,
            &parent.candidate_id,
            &parent.candidate_id,
            "episode-child",
            &board(2, "110", "source-b", "70"),
            artifact("duplicate"),
            evidence("duplicate"),
        )
        .is_err()
    );
    assert_eq!(catalog, before);

    let mut mismatched = catalog.clone();
    mismatched.insert("wrong-map-key".into(), parent.clone());
    assert!(validate_candidate_catalog(&mismatched).is_err());
    assert!(
        originate_candidate(
            &mismatched,
            "new-candidate",
            "new-hypothesis",
            "new-episode",
            &board(2, "110", "source-b", "70"),
            artifact("new"),
            evidence("new"),
        )
        .is_err()
    );

    let adapter = ObjectiveComparatorV1 {
        objective_id: parent.objective.objective_id.clone(),
        version: parent.objective.version.clone(),
        kind: ComparatorKindV1::AdapterDefined {
            contract_id: "fixture-comparator".into(),
            version_sha256: digest("adapter-comparator"),
        },
    };
    let mut adapter_parent = parent;
    adapter_parent.objective = adapter.clone();
    reseal(&mut adapter_parent);
    let mut adapter_catalog = CandidateCatalogV1::new();
    adapter_catalog.insert(adapter_parent.candidate_id.clone(), adapter_parent.clone());
    let mut next = board(2, "110", "source-b", "70");
    next.comparator = adapter;
    next.decision_sha256 = next.frontier.decision_sha256(&next.comparator).unwrap();
    assert!(
        port_replay_candidate(
            &adapter_catalog,
            "adapter-child",
            &adapter_parent.candidate_id,
            "episode-child",
            &next,
            artifact("adapter"),
            evidence("adapter"),
        )
        .is_err()
    );
}

#[test]
fn malformed_source_identity_and_object_ids_are_rejected() {
    let mut malformed = board(1, "100", "source-a", "60");
    let source = malformed
        .entries
        .iter_mut()
        .find_map(|e| e.source.as_mut())
        .unwrap();
    source.source_id.clear();
    source.commit_oid = "not-an-object-id".into();
    malformed
        .frontier
        .source_accessible_base
        .as_mut()
        .unwrap()
        .source = source.clone();
    malformed.decision_sha256 = malformed
        .frontier
        .decision_sha256(&malformed.comparator)
        .unwrap();
    assert!(
        originate_candidate(
            &CandidateCatalogV1::new(),
            "candidate",
            "hypothesis",
            "episode",
            &malformed,
            artifact("candidate"),
            evidence("candidate"),
        )
        .is_err()
    );
}
