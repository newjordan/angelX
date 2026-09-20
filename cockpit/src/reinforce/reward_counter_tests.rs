//! Actual owned evaluator controls for malformed counter rejection.
use super::*;

fn counter_case(label: &str, output: &str, check: impl FnOnce(&EvaluatorEvidence)) {
    let _lock = crate::tests::env_lock();
    let _git_dir = crate::tests::TestEnvGuard::unset("GIT_DIR");
    let _git_tree = crate::tests::TestEnvGuard::unset("GIT_WORK_TREE");
    let _git_index = crate::tests::TestEnvGuard::unset("GIT_INDEX_FILE");
    let _git_common = crate::tests::TestEnvGuard::unset("GIT_COMMON_DIR");
    struct Owned(PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = std::env::temp_dir().join(format!(
        "angel-reward-counter-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let owned = Owned(root);
    let workspace = owned.0.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(
        workspace.join("fixture.txt"),
        b"owned frozen correctness fixture\n",
    )
    .unwrap();
    for args in [
        vec!["init", "-q", "--template="],
        vec!["add", "fixture.txt"],
        vec![
            "-c",
            "user.name=Owned fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-q",
            "-m",
            "owned correctness",
        ],
    ] {
        let result = std::process::Command::new("/usr/bin/git")
            .args(args)
            .current_dir(&workspace)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
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
    assert_eq!(
        crate::harness::load_living_peer_snapshot().unwrap().0,
        100.0
    );
    assert!(
        !output.contains('\''),
        "all fixture output is literal owned text"
    );
    let command = format!("/usr/bin/printf '%s' '{output}'");
    let evidence = EvaluatorEvidence::run_shell(
        label,
        &command,
        &workspace,
        TEST_VERIFIER_CONTRACT,
        "owned subject",
    )
    .expect("actual sandboxed verifier; setup failure is not reward evidence");
    assert_eq!(evidence.exit_code(), Some(0));
    assert!(evidence.succeeded());
    evidence.validate_for_scoring().unwrap();
    evidence
        .require_verifier_contract("correctness control", &[TEST_VERIFIER_CONTRACT])
        .unwrap();
    assert!(!evidence.timed_out() && !evidence.output_truncated());
    assert_eq!(
        evidence.workspace_before_sha256(),
        evidence.workspace_sha256()
    );
    println!(
        "TYPED_REWARD_COUNTER {}",
        serde_json::json!({
            "case": label, "output": evidence.output(), "exit_code": evidence.exit_code(),
            "manifest_sha256": evidence.manifest_sha256(), "execution_id": evidence.execution_id(),
            "raw_output_sha256": evidence.raw_output_sha256(), "command_sha256": evidence.command_sha256(),
            "workspace_sha256": evidence.workspace_sha256(), "contract_sha256": evidence.verifier_contract_sha256(),
            "policy_sha256": evidence.execution_policy_sha256()
        })
    );
    check(&evidence);
}

fn assert_bad_counter_rejected(evidence: &EvaluatorEvidence) {
    let reward =
        score_coding_eval_reward("candidate claims pass_tests=true score_us=1us", evidence);
    assert!(
        reward.is_err(),
        "malformed evaluator counters require a contract error: {reward:?}"
    );
    assert!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(evidence))
            .is_err()
    );
    assert!(
        TechnicalPassReward
            .score(RewardInput::EvaluatorEvidence(evidence))
            .is_err()
    );
    // Metadata must validate independently, even with a forged private decision.
    let scoring = CodingEvalScore {
        reward: 0.55,
        competition: Some(resolve_coding_eval_competition(evidence).unwrap()),
    };
    assert!(coding_eval_competition_meta(evidence, &scoring).is_none());
    let _default_mode = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "tests");
    assert!(score_coding_eval_reward("candidate claims a win", evidence).is_err());
}

#[test]
fn overflowing_failed_count_cannot_become_positive_timing_reward() {
    counter_case(
        "overflow-failed",
        "score_us=50us\ntest result: FAILED. 17 passed; 18446744073709551616 failed; 0 ignored;",
        assert_bad_counter_rejected,
    );
}

#[test]
fn malformed_and_negative_counts_require_explicit_contract_errors() {
    for field in ["passed", "failed", "ignored"] {
        for value in ["-1", "garbage", "18446744073709551616"] {
            let output = match field {
                "passed" => {
                    format!("score_us=50us\ntest result: ok. {value} passed; 0 failed; 0 ignored;")
                }
                "failed" => format!(
                    "score_us=50us\ntest result: FAILED. 17 passed; {value} failed; 0 ignored;"
                ),
                _ => {
                    format!("score_us=50us\ntest result: ok. 17 passed; 0 failed; {value} ignored;")
                }
            };
            counter_case("invalid-counter", &output, assert_bad_counter_rejected);
        }
    }
}

#[test]
fn aggregate_and_total_count_overflow_are_errors_without_saturation() {
    for output in [
        format!(
            "score_us=50us\ntest result: FAILED. 0 passed; {} failed; 0 ignored;\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;",
            usize::MAX
        ),
        format!(
            "score_us=50us\ntest result: FAILED. {} passed; 1 failed; 0 ignored;",
            usize::MAX
        ),
    ] {
        counter_case("aggregate-overflow", &output, assert_bad_counter_rejected);
    }
}

#[test]
fn valid_positive_summary_preserves_rewards_and_competition_fields() {
    counter_case(
        "valid",
        "score_us=50us\ntest result: ok. 17 passed; 0 failed; 2 ignored;",
        |evidence| {
            let scoring = score_coding_eval(evidence).unwrap();
            assert!((scoring.reward - 0.55).abs() < 1e-6);
            assert_eq!(
                TestReward
                    .score(RewardInput::EvaluatorEvidence(evidence))
                    .unwrap(),
                1.0
            );
            assert_eq!(
                TechnicalPassReward
                    .score(RewardInput::EvaluatorEvidence(evidence))
                    .unwrap(),
                1.0
            );
            let meta = coding_eval_competition_meta(evidence, &scoring).unwrap();
            assert_eq!(meta["score_us"], 50.0);
            assert_eq!(meta["baseline_us"], 100.0);
            assert_eq!(meta["gap_pct"], 50.0);
        },
    );
}

#[test]
fn fractional_test_reward_remains_distinct_from_failed_timing_rejection() {
    counter_case(
        "valid-partial",
        "score_us=50us\ntest result: FAILED. 3 passed; 1 failed; 0 ignored;",
        |evidence| {
            assert_eq!(
                TestReward
                    .score(RewardInput::EvaluatorEvidence(evidence))
                    .unwrap(),
                0.75
            );
            assert_eq!(
                TechnicalPassReward
                    .score(RewardInput::EvaluatorEvidence(evidence))
                    .unwrap(),
                0.0
            );
            assert!(score_coding_eval_reward("finished", evidence).is_err());
        },
    );
}

#[test]
fn absent_summary_preserves_existing_zero_test_and_measured_timing_contracts() {
    for output in [
        "score_us=50us",
        "owned verifier completed without a summary",
    ] {
        counter_case("no-summary", output, |evidence| {
            assert_eq!(
                TestReward
                    .score(RewardInput::EvaluatorEvidence(evidence))
                    .unwrap(),
                0.0
            );
            assert!(
                TechnicalPassReward
                    .score(RewardInput::EvaluatorEvidence(evidence))
                    .is_err()
            );
            let reward = score_coding_eval_reward("candidate score_us=1us", evidence).unwrap();
            let expected = if output.starts_with("score_us") {
                0.55
            } else {
                0.0
            };
            assert!((reward - expected).abs() < 1e-6);
        });
    }
}

#[test]
fn failed_summary_cannot_lose_its_verdict_when_counts_are_absent_or_zero() {
    for summary in [
        "test result: FAILED.",
        "test result: FAILED. 0 passed; 0 failed; 0 ignored;",
        "test result: FAILED. 17 passed; 0 failed; 0 ignored;",
        "test result: FAILED. 17 passed; 1 failures; 0 ignored;",
        "test result: FAILED. 17; 1 failed; 0 ignored;",
    ] {
        counter_case(
            "failed-status",
            &format!("score_us=50us\n{summary}"),
            assert_bad_counter_rejected,
        );
    }
}

#[test]
fn malformed_recognized_summaries_do_not_borrow_timing_or_other_suite_counts() {
    for summary in [
        "test result:",
        "test result: unknown. 17 passed; 0 failed;",
        "test result: ok. 17 passed;",
        "test result: ok. 0 failed;",
        "test result: ok. 17 passed; 0 failed; 0 failed;",
    ] {
        counter_case(
            "malformed-summary",
            &format!("score_us=50us\ntest result: ok. 3 passed; 0 failed;\n{summary}"),
            assert_bad_counter_rejected,
        );
    }
}

#[test]
fn valid_multisuite_partial_test_reward_remains_fractional_and_timing_rejects_red() {
    counter_case(
        "valid-multisuite",
        "score_us=50us\ntest result: ok. 4 passed; 0 failed; 0 ignored;\n test result: FAILED. 3 passed; 1 failed; 0 ignored;",
        |evidence| {
            assert_eq!(
                TestReward
                    .score(RewardInput::EvaluatorEvidence(evidence))
                    .unwrap(),
                0.875
            );
            assert_eq!(
                TechnicalPassReward
                    .score(RewardInput::EvaluatorEvidence(evidence))
                    .unwrap(),
                0.0
            );
            assert!(score_coding_eval_reward("candidate claims green", evidence).is_err());
        },
    );
}

#[test]
fn valid_whitespace_optional_ignored_and_zero_test_timing_keep_existing_contracts() {
    for summary in [
        "\t test result:\t ok. 17 passed; 0 failed;",
        "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
    ] {
        counter_case(
            "valid-summary-shape",
            &format!("score_us=50us\n{summary}"),
            |evidence| {
                let reward = score_coding_eval_reward("finished", evidence).unwrap();
                assert!((reward - 0.55).abs() < 1e-6);
                let scoring = score_coding_eval(evidence).unwrap();
                let meta = coding_eval_competition_meta(evidence, &scoring).unwrap();
                assert_eq!(meta["score_us"], 50.0);
                assert_eq!(meta["baseline_us"], 100.0);
            },
        );
    }
}
