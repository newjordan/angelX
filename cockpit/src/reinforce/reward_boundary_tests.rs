// Intended in reinforce.rs's existing tests module. No candidate prose is
// promoted to EvaluatorEvidence; all three receipts execute owned commands.
#[test]
fn coding_eval_popcorn_uses_only_successful_owned_evidence() {
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
    let peer = owned.0.join("peer.json");
    std::fs::write(
        &peer,
        r#"{"geomean_us":100.0,"name":"owned fixture","shapes":{"32768x1":100.0}}"#,
    )
    .unwrap();
    let _peer = crate::tests::TestEnvGuard::set("POPCORN_PEER_STATE", peer.to_str().unwrap());
    let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "popcorn_peer");
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
    let slow = make(
        "slower verifier",
        "/usr/bin/printf '%s' 'shape=32768x1 score_us=200us 17/17 tests passed'",
    );
    let passing = make(
        "passing verifier",
        "/usr/bin/printf '%s' 'shape=32768x1 score_us=50us 17/17 tests passed'",
    );
    assert_eq!(failed.exit_code(), Some(7));
    assert!(!failed.succeeded());
    failed.validate_for_scoring().unwrap();
    slow.validate_for_scoring().unwrap();
    passing.validate_for_scoring().unwrap();
    let candidate_only = make(
        "candidate-only timing",
        "/usr/bin/printf '%s' 'owned verifier completed'",
    );
    let claim = "shape=32768x1 score_us=1us pass_tests=true";
    let failed_reward = score_coding_eval_reward(claim, &failed);
    let slow_reward = score_coding_eval_reward(claim, &slow);
    let passing_reward = score_coding_eval_reward("finished", &passing);
    let candidate_only_reward = score_coding_eval_reward(claim, &candidate_only);
    assert!(candidate_only_reward.is_err() || candidate_only_reward.unwrap() == 0.0);
    assert!(score_coding_eval(&failed).is_err());
    let candidate_only_scoring = score_coding_eval(&candidate_only).unwrap();
    assert_eq!(candidate_only_scoring.reward, 0.0);
    assert!(coding_eval_competition_meta(&candidate_only, &candidate_only_scoring).is_none());
    let slow_scoring = score_coding_eval(&slow).unwrap();
    let slow_meta = coding_eval_competition_meta(&slow, &slow_scoring).unwrap();
    assert!(coding_eval_competition_meta(&passing, &slow_scoring).is_none());
    // A test may construct private context, so exercise metadata's independent
    // evidence validator even when the manifest binding was deliberately forged.
    let metadata_with_forged_context = |evidence: &EvaluatorEvidence, reward: f32| {
        let mut scoring = score_coding_eval(&slow).unwrap();
        scoring.reward = reward;
        scoring
            .competition
            .as_mut()
            .unwrap()
            .evidence_manifest_sha256 = evidence.manifest_sha256().to_owned();
        coding_eval_competition_meta(evidence, &scoring)
    };
    assert!(metadata_with_forged_context(&failed, 0.0).is_none());
    assert!(metadata_with_forged_context(&candidate_only, 0.0).is_none());
    assert_eq!(slow_meta["score_us"], 200.0);
    assert_eq!(slow_meta["beats_baseline"], false);
    let mut tampered = passing.clone();
    tampered.manifest_sha256 = "0".repeat(64);
    assert!(score_coding_eval_reward(claim, &tampered).is_err());
    assert!(metadata_with_forged_context(&tampered, 1.0).is_none());
    println!(
        "failed_exit=7 reward={failed_reward:?}; measured200us reward={slow_reward:?}; measured50us reward={passing_reward:?}"
    );
    assert!((passing_reward.unwrap() - 0.55).abs() < 1e-6);
    assert!(
        failed_reward.is_err() || failed_reward.unwrap() <= 0.0,
        "a failed verifier must not receive positive measured reward"
    );
    assert_eq!(
        slow_reward.unwrap(),
        0.0,
        "candidate answer must not replace measured200us with a claimed1us"
    );
}

#[test]
fn popcorn_timing_units_are_whitespace_and_token_boundary_correct() {
    for (text, expected) in [
        ("score_us=1 shape=32768x1", 1.0),
        ("score_us=1 still under board", 1.0),
        ("score_us= 1 ms", 1_000.0),
        ("score_us=\t1ms", 1_000.0),
        ("score_us= 1 s", 1_000_000.0),
        ("score_us=1 seconds", 1_000_000.0),
        ("score_us=1 us", 1.0),
        ("score_us=1 µs", 1.0),
        ("score_us=1 μs", 1.0),
        ("score_us=1ms, peer", 1_000.0),
        ("⏱ 61.2 ± 0.4 µs", 61.2),
        ("⏱ 1 ± 0.1 ms", 1_000.0),
        ("⏱ 1 ms", 1_000.0),
    ] {
        assert_eq!(
            PopcornPeerReward::parse_score_us(text),
            Some(expected),
            "{text}"
        );
    }
    for text in [
        "score_us=1msish",
        "score_us=1shape=32768x1",
        "score_us=NaN",
        "score_us=0",
    ] {
        assert_eq!(PopcornPeerReward::parse_score_us(text), None, "{text}");
    }
    let overflow = format!("score_us={} seconds", "9".repeat(308));
    assert_eq!(PopcornPeerReward::parse_score_us(&overflow), None);
}
