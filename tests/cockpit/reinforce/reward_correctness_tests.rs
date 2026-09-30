//! Actual owned evaluator receipts. These controls never create evidence from candidate text.
use super::*;

fn typed_case(label: &str, output: &str, check: impl FnOnce(Result<f32, String>)) {
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
        "angel-reward-correctness-{label}-{}-{}",
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
    let parsed = crate::agent::harness::parse_test_result(evidence.output());
    let reward = score_coding_eval_reward("candidate says pass_tests=true score_us=1us", &evidence);
    println!(
        "TYPED_REWARD_CORRECTNESS {}",
        serde_json::json!({
            "case": label, "command": command, "output": evidence.output(),
            "exit_code": evidence.exit_code(), "succeeded": evidence.succeeded(),
            "passed": parsed.passed, "failed": parsed.failed, "ignored": parsed.ignored,
            "reward": reward, "execution_id": evidence.execution_id(),
            "manifest_sha256": evidence.manifest_sha256(), "raw_output_sha256": evidence.raw_output_sha256(),
            "workspace_before_sha256": evidence.workspace_before_sha256(),
            "workspace_after_sha256": evidence.workspace_sha256(),
            "verifier_contract_sha256": evidence.verifier_contract_sha256(),
            "execution_policy_sha256": evidence.execution_policy_sha256(),
            "command_sha256": evidence.command_sha256(),
            "timed_out": evidence.timed_out(), "truncated": evidence.output_truncated()
        })
    );
    check(reward);
}

fn assert_rejected(reward: Result<f32, String>) {
    assert!(
        reward.is_err() || reward.as_ref().is_ok_and(|value| *value <= 0.0),
        "a candidate claim must not override a parsed failed correctness result: {reward:?}"
    );
}

fn assert_positive(reward: Result<f32, String>) {
    assert_eq!(reward, Ok(1.0));
}

#[test]
fn exit_zero_with_explicit_failed_summary_cannot_earn_a_reward() {
    let output =
        "shape=32768x1 score_us=50us\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;\n";
    assert_eq!(crate::agent::harness::parse_test_result(output).failed, 1);
    typed_case("failed-summary", output, assert_rejected);
}

#[test]
fn passing_suite_cannot_hide_another_failed_suite() {
    let output = "test result: ok. 17 passed; 0 failed; 0 ignored;\nshape=32768x1 score_us=50us pass_tests=true\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;\n";
    let parsed = crate::agent::harness::parse_test_result(output);
    assert_eq!((parsed.passed, parsed.failed), (17, 1));
    typed_case("mixed-suites", output, |reward| {
        let value = reward.unwrap();
        assert!(
            (value - 17.0 / 18.0).abs() < 1e-6,
            "the failed suite counts against the pass rate: {value}"
        );
    });
}

#[test]
fn zero_failed_summary_keeps_positive_reward() {
    typed_case(
        "passed-summary",
        "shape=32768x1 score_us=50us\ntest result: ok. 17 passed; 0 failed; 2 ignored;\n",
        assert_positive,
    );
}

#[test]
fn prose_about_failures_does_not_become_a_failed_correctness_verdict() {
    let output = "documentation: previous tests failed before the repair\nshape=32768x1 score_us=50us\ntest result: ok. 17 passed; 0 failed; 0 ignored;\n";
    assert_eq!(crate::agent::harness::parse_test_result(output).failed, 0);
    typed_case("failure-prose", output, assert_positive);
}
