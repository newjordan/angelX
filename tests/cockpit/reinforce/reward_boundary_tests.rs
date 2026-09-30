// Intended in reinforce.rs's existing tests module. No candidate prose is
// promoted to EvaluatorEvidence; all three receipts execute owned commands.
#[test]
fn coding_eval_uses_only_successful_owned_evidence() {
    let _lock = crate::tests::env_lock();
    let _git_dir = crate::tests::TestEnvGuard::unset("GIT_DIR");
    let _git_tree = crate::tests::TestEnvGuard::unset("GIT_WORK_TREE");
    let _git_index = crate::tests::TestEnvGuard::unset("GIT_INDEX_FILE");
    let _git_common = crate::tests::TestEnvGuard::unset("GIT_COMMON_DIR");
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let path = std::env::temp_dir().join(format!(
        "angel-reward-boundary-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    let owned = Owned(path);
    let workspace = owned.0.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    for args in [vec!["init", "-q"], vec!["add", "fixture.txt"]] {
        std::fs::write(
            workspace.join("fixture.txt"),
            "owned frozen verifier fixture",
        )
        .unwrap();
        let output = std::process::Command::new("/usr/bin/git")
            .args(args)
            .current_dir(&workspace)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let make = |label, command| {
        EvaluatorEvidence::run_shell(
            label,
            command,
            &workspace,
            TEST_VERIFIER_CONTRACT,
            "owned subject",
        )
        .unwrap()
    };
    let failed = make(
        "failed verifier",
        "/usr/bin/printf '%s' 'test result: FAILED. 0 passed; 1 failed;'; exit 7",
    );
    let passing = make(
        "passing verifier",
        "/usr/bin/printf '%s' 'test result: ok. 17 passed; 0 failed;'",
    );
    assert_eq!(failed.exit_code(), Some(7));
    assert!(!failed.succeeded());
    failed.validate_for_scoring().unwrap();
    passing.validate_for_scoring().unwrap();
    let candidate_only = make(
        "candidate-only timing",
        "/usr/bin/printf '%s' 'owned verifier completed'",
    );
    let claim = "test result: ok. 99 passed; 0 failed; pass_tests=true";
    let failed_reward = score_coding_eval_reward(claim, &failed);
    let passing_reward = score_coding_eval_reward("finished", &passing);
    let candidate_only_reward = score_coding_eval_reward(claim, &candidate_only);
    assert!(score_coding_eval(&failed).is_err());
    assert_eq!(
        candidate_only_reward.unwrap(),
        0.0,
        "a candidate's claim must never stand in for evaluator evidence"
    );
    let mut tampered = passing.clone();
    tampered.manifest_sha256 = "0".repeat(64);
    assert!(score_coding_eval_reward(claim, &tampered).is_err());
    println!("failed_exit=7 reward={failed_reward:?}; passing reward={passing_reward:?}");
    assert_eq!(passing_reward.unwrap(), 1.0);
    assert!(
        failed_reward.is_err(),
        "a failed verifier must not receive a reward"
    );
}
