use super::*;

/// The real layout: the workspace is `/parent/repo` and delegated checkouts live
/// *beside* it under `/parent/.angel-worktrees/…`, which is exactly what makes a
/// worktree child commute with a writer in the shared checkout.
fn path(name: &str) -> PathBuf {
    PathBuf::from("/parent").join(name)
}

/// Disjoint checkouts commute: different worktrees, different refs, different
/// ports are one wave, and a workspace writer beside them is in it too.
#[test]
fn disjoint_children_are_one_wave() {
    let children = vec![
        ChildFootprint::worktree_child("a", path(".angel-worktrees/a"), "angel/a")
            .claim(Resource::Port(41001)),
        ChildFootprint::worktree_child("b", path(".angel-worktrees/b"), "angel/b")
            .claim(Resource::Port(41002)),
        ChildFootprint::workspace_writer("seat-1", path("repo")),
    ];
    let schedule = plan(&children);
    assert!(schedule.independent(), "{schedule:?}");
    assert_eq!(schedule.waves, vec![vec![0, 1, 2]]);
    assert_eq!(schedule.revert_order(), vec![2, 1, 0]);
    assert!(refusal(&children).is_none());
}

/// A shared resource names its owner: two seats writing the one workspace, two
/// children on the same ref, and two services on the same port all split into
/// serialized waves with a receipt.
#[test]
fn overlapping_children_are_serialized_and_the_receipt_names_it() {
    let shared_workspace = vec![
        ChildFootprint::workspace_writer("seat-1", path("repo")),
        ChildFootprint::workspace_writer("seat-2", path("repo")),
    ];
    let schedule = plan(&shared_workspace);
    assert!(!schedule.independent());
    assert_eq!(schedule.waves, vec![vec![0], vec![1]]);
    assert_eq!(schedule.revert_order(), vec![1, 0]);
    let receipt = refusal(&shared_workspace).expect("a receipt");
    assert!(
        receipt.contains("seat-1") && receipt.contains("seat-2"),
        "{receipt}"
    );
    assert!(
        receipt.contains("/parent/repo"),
        "the resource is named: {receipt}"
    );

    let same_ref = vec![
        ChildFootprint::worktree_child("a", path(".angel-worktrees/a"), "angel/same"),
        ChildFootprint::worktree_child("b", path(".angel-worktrees/b"), "angel/same"),
    ];
    let (_, _, resource) = first_conflict(&same_ref).expect("a conflict");
    assert_eq!(resource, Resource::Ref("angel/same".to_string()));

    let same_port = vec![
        ChildFootprint::new("a").claim(Resource::Port(9)),
        ChildFootprint::new("b").claim(Resource::Port(9)),
    ];
    assert!(!plan(&same_port).independent());
}

/// The gate is derived, not hardcoded: the only difference between "serialize
/// these seats" and "let them run together" is which resources they declare.
#[test]
fn a_worktree_claim_replaces_the_serialization_a_workspace_claim_imposes() {
    let seats = ["seat-1", "seat-2"];
    let shared: Vec<_> = seats
        .iter()
        .map(|seat| ChildFootprint::workspace_writer(*seat, path("repo")))
        .collect();
    assert!(!plan(&shared).independent());

    let isolated: Vec<_> = seats
        .iter()
        .enumerate()
        .map(|(index, seat)| {
            ChildFootprint::worktree_child(
                *seat,
                path(&format!(".angel-worktrees/{index}")),
                format!("angel/{index}"),
            )
        })
        .collect();
    assert!(plan(&isolated).independent());
}

/// A nested claim overlaps: a child owning a directory excludes a child inside
/// it, so a scratch root and a file under it are serialized.
#[test]
fn nested_paths_overlap() {
    let nested = vec![
        ChildFootprint::new("a").claim(Resource::Path(path("out"))),
        ChildFootprint::new("b").claim(Resource::Path(path("out/report.json"))),
    ];
    let (_, _, resource) = first_conflict(&nested).expect("a conflict");
    assert_eq!(resource, Resource::Path(path("out")));
    assert!(!plan(&nested).independent());
}
