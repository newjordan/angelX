//! Finite metadata controls using actual successful evaluator evidence.
use super::*;

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn finite_metadata_case(label: &str, baseline: f64, score_text: &str, expected_gap: Option<f64>) {
    let _lock = crate::tests::env_lock();
    let _git_dir = crate::tests::TestEnvGuard::unset("GIT_DIR");
    let _git_tree = crate::tests::TestEnvGuard::unset("GIT_WORK_TREE");
    let _git_index = crate::tests::TestEnvGuard::unset("GIT_INDEX_FILE");
    let _git_common = crate::tests::TestEnvGuard::unset("GIT_COMMON_DIR");
    let root = std::env::temp_dir().join(format!(
        "angel-reward-finite-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let owned = Owned(std::fs::canonicalize(root).unwrap());
    let workspace = owned.0.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("fixture.txt"), "owned finite fixture\n").unwrap();
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
            "owned finite fixture",
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
        serde_json::to_vec(&serde_json::json!({
            "geomean_us": baseline, "name": "owned-finite-control"
        }))
        .unwrap(),
    )
    .unwrap();
    let _peer = crate::tests::TestEnvGuard::set("POPCORN_PEER_STATE", peer.to_str().unwrap());
    let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "popcorn_peer");
    assert_eq!(
        crate::agent::harness::load_living_peer_snapshot()
            .unwrap()
            .0,
        baseline
    );
    let output =
        format!("score_us={score_text}us\ntest result: ok. 17 passed; 0 failed; 0 ignored;\n");
    let command = format!("/usr/bin/printf '%s' '{output}'");
    let evidence = EvaluatorEvidence::run_shell(
        label,
        &command,
        &workspace,
        TEST_VERIFIER_CONTRACT,
        "owned finite metadata subject",
    )
    .expect("actual sandboxed verifier; setup failure is not reward evidence");
    validate_coding_eval_evidence(&evidence).unwrap();
    let score_us = PopcornPeerReward::parse_score_us(evidence.output()).unwrap();
    assert!(score_us.is_finite() && score_us > 0.0);
    let scoring = score_coding_eval(&evidence).unwrap();
    assert!(scoring.reward.is_finite());
    let metadata = coding_eval_competition_meta(&evidence, &scoring).unwrap();
    println!(
        "TYPED_REWARD_FINITE_METADATA {}",
        serde_json::json!({
            "case": label, "baseline_us": baseline, "score_us": score_us,
            "reward": scoring.reward, "metadata": metadata,
            "manifest_sha256": evidence.manifest_sha256(),
            "execution_id": evidence.execution_id(),
            "raw_output_sha256": evidence.raw_output_sha256(),
            "workspace_before_sha256": evidence.workspace_before_sha256(),
            "workspace_after_sha256": evidence.workspace_sha256(),
            "command_sha256": evidence.command_sha256(),
            "execution_policy_sha256": evidence.execution_policy_sha256(),
            "verifier_contract_sha256": evidence.verifier_contract_sha256()
        })
    );
    assert_eq!(metadata["baseline_us"], baseline);
    assert_eq!(metadata["score_us"], score_us);
    assert_eq!(metadata["beats_baseline"], score_us < baseline);
    match expected_gap {
        Some(expected) => {
            let actual = metadata["gap_pct"]
                .as_f64()
                .expect("representable gap must remain numeric");
            assert!(actual.is_finite());
            assert!((actual - expected).abs() <= expected.abs() * 1e-12);
        }
        None => assert!(
            metadata.get("gap_pct").is_none(),
            "unrepresentable gap must be absent, not null or fabricated"
        ),
    }
}

#[test]
fn finite_normal_gap_keeps_existing_percentage() {
    finite_metadata_case("normal", 100.0, "50", Some(50.0));
}

#[test]
fn unrepresentable_gap_is_omitted() {
    finite_metadata_case("ratio-overflow", 1e-308, "50", None);
}

#[test]
fn finite_gap_survives_rounding_overflow() {
    // The timing parser accepts decimal notation, not an exponent token.
    let score = format!("1{}", "0".repeat(306));
    finite_metadata_case("rounding-overflow", 1.0, &score, Some(-1e308));
}
