use super::*;

#[test]
fn verification_target_dep_rules_reject_ambiguous_make_syntax() {
    assert_eq!(
        dependencies(b"out: src/lib.rs src/with\\ space.rs\nphony.rs:\n"),
        Some(vec![
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/with space.rs")
        ])
    );
    for bytes in [
        b"out: src/$(INPUT).rs\n".as_slice(),
        b"out: src/foo\\q.rs\n",
        b"out: src/a.rs \\\n src/b.rs\n",
        b"out: src/a.rs # extra\n",
    ] {
        assert!(
            dependencies(bytes).is_none(),
            "ambiguous depfile must not certify coverage: {bytes:?}"
        );
    }
}

#[test]
fn verification_target_incomplete_machine_output_cannot_certify_sources() {
    let root = Path::new("/nonexistent-owned-target");
    for output in [
        "",
        "{\"reason\":\"build-finished\",\"success\":true}\n",
        "[output truncated]\n{\"reason\":\"build-finished\",\"success\":true}\n",
    ] {
        assert!(cargo_evidence(output, root, root).0.is_none());
    }
}
