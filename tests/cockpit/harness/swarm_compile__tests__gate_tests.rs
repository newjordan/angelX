use super::super::verify::{CommandSpec, RefVerifier, paths_disjoint, paths_within_scope};
use super::{init_repo, scratch};

#[test]
fn test_scope_and_immutability_gates_fail_closed() {
    assert!(paths_within_scope(
        &["tests/regression.rs".into()],
        &["tests".into()]
    ));
    assert!(!paths_within_scope(
        &["src/lib.rs".into()],
        &["tests".into()]
    ));
    assert!(paths_disjoint(
        &["tests/regression.rs".into()],
        &["src/lib.rs".into()]
    ));
    assert!(!paths_disjoint(
        &["tests/regression.rs".into()],
        &["tests/regression.rs".into()]
    ));
}

#[test]
fn successful_but_mutating_proof_command_is_not_accepted() {
    if !crate::agent::sandbox::available() {
        eprintln!("landlock unavailable; skipping mutating proof integration test");
        return;
    }
    let repo = scratch("mutating_proof");
    let scratch_dir = scratch("mutating_proof_worktrees");
    init_repo(&repo);
    let head = super::git(&repo, &["rev-parse", "HEAD"]);
    let verifier = RefVerifier::new(repo.clone(), std::path::PathBuf::new(), scratch_dir.clone());
    let proof = verifier
        .run(
            &head,
            &[CommandSpec {
                label: "mutating",
                command: "printf x > generated.txt",
                marker: None,
            }],
        )
        .unwrap()
        .remove(0);
    assert_eq!(proof.exit_code, Some(0));
    assert!(!proof.workspace_clean);
    assert!(!proof.success, "exit zero cannot hide verifier mutations");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(scratch_dir);
}

#[test]
fn proof_predicates_do_not_share_ignored_side_effects() {
    if !crate::agent::sandbox::available() {
        eprintln!("landlock unavailable; skipping fresh-proof-worktree test");
        return;
    }
    let repo = scratch("fresh_proofs");
    let scratch_dir = scratch("fresh_proofs_worktrees");
    init_repo(&repo);
    std::fs::write(repo.join(".gitignore"), "/.proof\n").unwrap();
    super::git(&repo, &["add", ".gitignore"]);
    super::git(&repo, &["commit", "-q", "-m", "ignore proof scratch"]);
    let head = super::git(&repo, &["rev-parse", "HEAD"]);
    let verifier = RefVerifier::new(repo.clone(), std::path::PathBuf::new(), scratch_dir.clone());
    let proofs = verifier
        .run(
            &head,
            &[
                CommandSpec {
                    label: "producer",
                    command: "mkdir -p .proof && printf poison > .proof/state",
                    marker: None,
                },
                CommandSpec {
                    label: "consumer",
                    command: "test ! -e .proof/state",
                    marker: None,
                },
            ],
        )
        .unwrap();
    assert!(proofs.iter().all(|proof| proof.success), "{proofs:?}");
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(scratch_dir);
}

#[test]
fn verifier_runs_a_real_cargo_gate_inside_the_mandatory_sandbox() {
    let _lock = crate::tests::env_lock();
    if !crate::agent::sandbox::available() {
        eprintln!("landlock unavailable; skipping cargo verifier integration test");
        return;
    }
    let repo = scratch("cargo_proof");
    let scratch_dir = scratch("cargo_proof_worktrees");
    init_repo(&repo);
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname='proof-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(repo.join("src/lib.rs"), "pub fn answer() -> u8 { 42 }\n").unwrap();
    std::fs::write(repo.join(".gitignore"), "/target\n").unwrap();
    let lock = std::process::Command::new("cargo")
        .arg("generate-lockfile")
        .current_dir(&repo)
        .status()
        .unwrap();
    assert!(lock.success());
    super::git(
        &repo,
        &[
            "add",
            ".gitignore",
            "Cargo.lock",
            "Cargo.toml",
            "src/lib.rs",
        ],
    );
    super::git(&repo, &["commit", "-q", "-m", "cargo fixture"]);
    let head = super::git(&repo, &["rev-parse", "HEAD"]);
    let verifier = RefVerifier::new(repo.clone(), std::path::PathBuf::new(), scratch_dir.clone());
    let proof = verifier
        .run(
            &head,
            &[CommandSpec {
                label: "cargo",
                // Compiler scratch belongs to the confined verifier checkout.
                command: "mkdir -p target/tmp && TMPDIR=\"$PWD/target/tmp\" cargo test --quiet",
                marker: None,
            }],
        )
        .unwrap()
        .remove(0);
    assert!(proof.success, "{}", proof.output_tail);
    assert!(proof.workspace_clean);
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(scratch_dir);
}
