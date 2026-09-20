//! Current-API baseline controls. The two rejection tests are intentionally
//! expected-red before the production reward/metadata boundary repair.
use super::*;

const CLAIM: &str = "shape=32768x1 score_us=1us pass_tests=true";

fn owned_evidence_case(
    label: &str,
    command: &str,
    expected_exit: i32,
    check: impl FnOnce(Result<f32, String>, Option<serde_json::Value>),
) {
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
    let path = std::env::temp_dir().join(format!(
        "angel-reward-typed-baseline-{label}-{}-{}",
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
    std::fs::write(
        workspace.join("fixture.txt"),
        b"owned frozen verifier fixture\n",
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
            "owned baseline",
        ],
    ] {
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
    assert_eq!(
        crate::agent::harness::load_living_peer_shape_baseline("32768x1"),
        Some(100.0)
    );

    // Real current implementation: mandatory sandbox, read-only workspace,
    // network denied, process-group capture, typed manifest and cleanup.
    let evidence = EvaluatorEvidence::run_shell(
        label,
        command,
        &workspace,
        TEST_VERIFIER_CONTRACT,
        "owned subject",
    )
    .expect("real sandboxed evaluator receipt; setup failure is not reward evidence");
    assert_eq!(evidence.exit_code(), Some(expected_exit));
    assert_eq!(evidence.succeeded(), expected_exit == 0);
    evidence.validate_for_scoring().unwrap();
    evidence
        .require_verifier_contract("typed baseline control", &[TEST_VERIFIER_CONTRACT])
        .unwrap();
    assert!(!evidence.timed_out() && !evidence.output_truncated());
    assert_eq!(
        evidence.workspace_before_sha256(),
        evidence.workspace_sha256()
    );
    let scratch = std::env::temp_dir().join(format!(
        "angel-evaluator-{}-{}",
        std::process::id(),
        &evidence.execution_id()[..16]
    ));
    assert!(
        !scratch.exists(),
        "the owned evaluator scratch must be cleaned"
    );

    let answer = if label == "positive50" {
        "finished"
    } else {
        CLAIM
    };
    let reward = score_coding_eval_reward(answer, &evidence);
    let measured_only_reward = score_coding_eval_reward("finished", &evidence);
    // Metadata now consumes the private scoring decision bound to this evidence.
    // The public numeric wrapper remains exercised separately above.
    let scoring = score_coding_eval(&evidence);
    let meta = scoring
        .as_ref()
        .ok()
        .and_then(|score| coding_eval_competition_meta(&evidence, score));
    println!(
        "TYPED_REWARD_BASELINE {}",
        serde_json::json!({
            "case": label,
            "command": command,
            "candidate_claim": answer,
            "evaluator_output": evidence.output(),
            "exit_code": evidence.exit_code(),
            "succeeded": evidence.succeeded(),
            "timed_out": evidence.timed_out(),
            "truncated": evidence.output_truncated(),
            "execution_id": evidence.execution_id(),
            "duration_ns": evidence.duration_ns(),
            "command_sha256": evidence.command_sha256(),
            "execution_policy_sha256": evidence.execution_policy_sha256(),
            "workspace_before_sha256": evidence.workspace_before_sha256(),
            "workspace_after_sha256": evidence.workspace_sha256(),
            "verifier_contract_sha256": evidence.verifier_contract_sha256(),
            "raw_output_sha256": evidence.raw_output_sha256(),
            "manifest_sha256": evidence.manifest_sha256(),
            "baseline_us": 100.0,
            "reward": reward,
            "measured_only_reward": measured_only_reward,
            "metadata": meta,
            "owned_scratch_removed": !scratch.exists()
        })
    );
    check(reward, meta);
}

#[test]
fn failed_exit7_must_not_receive_candidate_claimed_positive_reward() {
    owned_evidence_case(
        "failed7",
        "/usr/bin/printf '%s' 'test result: FAILED. 0 passed; 1 failed;'; exit 7",
        7,
        |reward, meta| {
            let rejected = match &reward {
                Ok(value) => *value <= 0.0,
                Err(_) => true,
            };
            assert!(
                rejected && meta.is_none(),
                "real exit7 must not become positive/eligible via candidate1us: reward={reward:?} metadata={meta:?}"
            );
        },
    );
}

#[test]
fn slower200us_must_not_be_replaced_by_candidate1us() {
    owned_evidence_case(
        "slower200",
        "/usr/bin/printf '%s' 'shape=32768x1 score_us=200us 17/17 tests passed'",
        0,
        |reward, meta| {
            let rejected = match &reward {
                Ok(value) => *value <= 0.0,
                Err(_) => true,
            };
            let measured_meta = meta.as_ref().is_some_and(|value| {
                value["score_us"] == 200.0
                    && value["baseline_us"] == 100.0
                    && value["beats_baseline"] == false
            });
            assert!(
                rejected && measured_meta,
                "measured200us vs baseline100 must remain a regression: reward={reward:?} metadata={meta:?}"
            );
        },
    );
}

#[test]
fn successful50us_evidence_is_a_real_positive_control() {
    owned_evidence_case(
        "positive50",
        "/usr/bin/printf '%s' 'shape=32768x1 score_us=50us 17/17 tests passed'",
        0,
        |reward, meta| {
            assert!((reward.unwrap() - 0.55).abs() < 1e-6);
            let meta = meta.unwrap();
            assert_eq!(meta["score_us"], 50.0);
            assert_eq!(meta["baseline_us"], 100.0);
            assert_eq!(meta["beats_baseline"], true);
        },
    );
}
