use super::*;

#[test]
fn classifies_common_paths() {
    assert_eq!(classify_path("src/main.rs"), FileClass::Source);
    assert_eq!(classify_path("cockpit/src/foo.rs"), FileClass::Source);
    assert_eq!(classify_path("src/harness/tests/foo.rs"), FileClass::Test);
    assert_eq!(classify_path("tests/integration.rs"), FileClass::Test);
    assert_eq!(classify_path("foo_test.rs"), FileClass::Test);
    assert_eq!(classify_path("app.test.ts"), FileClass::Test);
    assert_eq!(classify_path("docs/plan.md"), FileClass::Docs);
    assert_eq!(classify_path("README.md"), FileClass::Docs);
    assert_eq!(classify_path("Cargo.toml"), FileClass::Config);
    assert_eq!(classify_path(".github/workflows/ci.yml"), FileClass::Config);
    assert_eq!(classify_path("Cargo.lock"), FileClass::Lock);
    assert_eq!(classify_path("package-lock.json"), FileClass::Lock);
}

#[test]
fn parse_porcelain_and_renames() {
    // Build porcelain without `\` line continuations (they strip leading
    // whitespace and would collapse ` M` into `M `).
    let porc = [
        "M  staged.rs",
        " M unstaged.rs",
        "?? new.rs",
        "R  old.rs -> moved.rs",
        "## main...origin/main",
        "",
    ]
    .join("\n");
    let files = parse_porcelain_files(&porc);
    let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        vec!["staged.rs", "unstaged.rs", "new.rs", "moved.rs"]
    );
    let by = |p: &str| files.iter().find(|f| f.path == p).unwrap();
    assert!(by("staged.rs").has_index_change());
    assert_eq!(by("unstaged.rs").index_status, ' ');
    assert_eq!(by("unstaged.rs").worktree_status, 'M');
    assert!(by("unstaged.rs").has_worktree_change());
    assert!(by("new.rs").is_untracked());
    assert_eq!(by("moved.rs").index_status, 'R');
}

#[test]
fn plan_splits_in_dependency_order() {
    let files = parse_porcelain_files(
        "\
 M src/lib.rs\n\
 M tests/lib_test.rs\n\
 M README.md\n\
 M Cargo.lock\n",
    );
    let plan = plan_atomic_commits(&files, true, None).unwrap();
    assert_eq!(plan.units.len(), 4);
    assert_eq!(plan.units[0].class, FileClass::Source);
    assert_eq!(plan.units[1].class, FileClass::Test);
    assert_eq!(plan.units[2].class, FileClass::Docs);
    assert_eq!(plan.units[3].class, FileClass::Lock);
    assert!(plan.skipped_lock_note);
}

#[test]
fn plan_single_uses_operator_message() {
    let files = parse_porcelain_files(" M a.rs\n M b.rs\n");
    let plan = plan_atomic_commits(&files, false, Some("feat: land both")).unwrap();
    assert_eq!(plan.units.len(), 1);
    assert_eq!(plan.units[0].subject, "feat: land both");
    assert_eq!(plan.units[0].paths.len(), 2);
}

#[test]
fn validates_message_shape() {
    assert!(validate_commit_message("feat: ok").is_ok());
    assert!(validate_commit_message("feat: ok\n\nbody line").is_ok());
    assert!(validate_commit_message("").is_err());
    assert!(validate_commit_message(&"x".repeat(80)).is_err());
    assert!(validate_commit_message("subject\nbody without blank").is_err());
}

#[test]
fn empty_tree_errors() {
    assert!(plan_atomic_commits(&[], true, None).is_err());
}

#[test]
fn suggest_subject_adds_conventional_scope() {
    let s = suggest_subject(FileClass::Source, &["cockpit/src/hashline.rs".into()]);
    assert!(s.starts_with("feat(cockpit):"), "{s}");
    assert!(s.contains("hashline"), "{s}");

    let s = suggest_subject(
        FileClass::Test,
        &[
            "cockpit/src/harness/tests/a.rs".into(),
            "cockpit/src/harness/tests/b.rs".into(),
        ],
    );
    assert!(s.starts_with("test(cockpit):"), "{s}");
    assert!(s.contains("cover"), "{s}");
}
