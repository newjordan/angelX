//! Baseline controls for one measured reward and its later competition metadata.
//! Real sandboxed verifier evidence; all changing peer/artifact state is owned.
use super::*;

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn peer_bytes(baseline: f64, phase: &str, named_shape: bool) -> Vec<u8> {
    let mut peer = serde_json::json!({
        "geomean_us": baseline, "name": format!("owned-{phase}"), "phase": phase
    });
    if named_shape {
        peer["shapes"] = serde_json::json!({"32768x1": baseline});
    }
    serde_json::to_vec(&peer).unwrap()
}

fn coherence_case(label: &str, before: f64, after: f64, named_shape: bool) {
    let _lock = crate::tests::env_lock();
    let _git_dir = crate::tests::TestEnvGuard::unset("GIT_DIR");
    let _git_tree = crate::tests::TestEnvGuard::unset("GIT_WORK_TREE");
    let _git_index = crate::tests::TestEnvGuard::unset("GIT_INDEX_FILE");
    let _git_common = crate::tests::TestEnvGuard::unset("GIT_COMMON_DIR");
    let root = std::env::temp_dir().join(format!(
        "angel-reward-coherence-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    // Use the owned canonical path on platforms whose temp directory is an alias.
    let owned = Owned(std::fs::canonicalize(root).unwrap());
    let workspace = owned.0.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(
        workspace.join("fixture.txt"),
        b"owned frozen coherence fixture\n",
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
            "owned coherence",
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
    let peer_before = peer_bytes(before, "scored", named_shape);
    let peer_after = peer_bytes(after, "metadata-after-peer-update", named_shape);
    // Different lengths force both production file-key caches to see the update.
    assert_ne!(peer_before.len(), peer_after.len());
    std::fs::write(&peer, &peer_before).unwrap();
    let _peer = crate::tests::TestEnvGuard::set("POPCORN_PEER_STATE", peer.to_str().unwrap());
    let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "popcorn_peer");
    let artifact_dir = owned.0.join("evaluator-artifacts");
    let _artifact = crate::tests::TestEnvGuard::set(
        "ANGEL_EVALUATOR_ARTIFACT_DIR",
        artifact_dir.to_str().unwrap(),
    );
    assert_eq!(
        crate::harness::load_living_peer_snapshot().unwrap().0,
        before
    );
    assert_eq!(
        crate::harness::load_living_peer_shape_baseline("32768x1"),
        named_shape.then_some(before)
    );
    let output = "shape=32768x1 score_us=50us\ntest result: ok. 17 passed; 0 failed; 0 ignored;\n";
    let command = format!("/usr/bin/printf '%s' '{output}'");
    let evidence = EvaluatorEvidence::run_shell(
        label,
        &command,
        &workspace,
        TEST_VERIFIER_CONTRACT,
        "owned coherence subject",
    )
    .expect("actual sandboxed verifier; setup failure is not reward evidence");
    evidence.validate_for_scoring().unwrap();
    evidence
        .require_verifier_contract("coherence control", &[TEST_VERIFIER_CONTRACT])
        .unwrap();
    assert!(evidence.succeeded());
    assert_eq!(evidence.exit_code(), Some(0));
    assert!(!evidence.timed_out() && !evidence.output_truncated());
    let parsed = crate::harness::parse_test_result(evidence.output());
    assert_eq!((parsed.passed, parsed.failed), (17, 0));
    assert_eq!(
        evidence.workspace_before_sha256(),
        evidence.workspace_sha256()
    );
    // Same order as run_coding_eval: score, persist artifact, then metadata.
    let scoring = score_coding_eval(&evidence).unwrap();
    let reward = scoring.reward;
    assert_eq!(
        score_coding_eval_reward("candidate claims nothing", &evidence).unwrap(),
        reward
    );
    let artifact_path = artifact::persist_if_configured(&evidence).unwrap().unwrap();
    let persisted = artifact::load_evaluator_artifact(&artifact_path).unwrap();
    assert_eq!(persisted.manifest_sha256(), evidence.manifest_sha256());
    assert_eq!(persisted.output(), evidence.output());
    std::fs::write(&peer, &peer_after).unwrap();
    assert_eq!(std::fs::read(&peer).unwrap(), peer_after);
    assert_eq!(
        crate::harness::load_living_peer_snapshot().unwrap().0,
        after
    );
    assert_eq!(
        crate::harness::load_living_peer_shape_baseline("32768x1"),
        named_shape.then_some(after)
    );
    let metadata = coding_eval_competition_meta(&evidence, &scoring).expect("competition signal");
    println!(
        "TYPED_REWARD_COHERENCE {}",
        serde_json::json!({
            "case": label, "command": command, "output": evidence.output(),
            "score_us": 50.0, "reward": reward, "metadata": metadata,
            "peer_before": serde_json::from_slice::<serde_json::Value>(&peer_before).unwrap(),
            "peer_after": serde_json::from_slice::<serde_json::Value>(&peer_after).unwrap(),
            "execution_id": evidence.execution_id(), "manifest_sha256": evidence.manifest_sha256(),
            "persisted_manifest_sha256": persisted.manifest_sha256(),
            "raw_output_sha256": evidence.raw_output_sha256(),
            "workspace_before_sha256": evidence.workspace_before_sha256(),
            "workspace_after_sha256": evidence.workspace_sha256(),
            "verifier_contract_sha256": evidence.verifier_contract_sha256(),
            "execution_policy_sha256": evidence.execution_policy_sha256(),
            "command_sha256": evidence.command_sha256(), "timed_out": evidence.timed_out(),
            "truncated": evidence.output_truncated(), "passed": parsed.passed, "failed": parsed.failed,
            "artifact_persisted_and_reloaded": true, "named_shape_baseline": named_shape,
        "peer_before_sha256": crate::cut::sha256_hex(&peer_before),
        "peer_after_sha256": crate::cut::sha256_hex(&peer_after),
        "artifact_bytes_sha256": crate::cut::sha256_hex(&std::fs::read(&artifact_path).unwrap())
        })
    );
    // Fixed numeric controls are independent of the reward implementation formula.
    let (expected_reward, expected_win, expected_gap) = match before {
        100.0 => (0.55, true, 50.0),
        40.0 => (0.0375, false, -25.0),
        _ => panic!("undeclared fixture baseline"),
    };
    assert!(
        (reward - expected_reward).abs() < 1e-6,
        "wrong scoring baseline"
    );
    assert_eq!(metadata["score_us"], 50.0);
    assert_eq!(metadata["reward"], serde_json::json!(reward));
    assert_eq!(metadata["shape_key"], "32768x1");
    assert_eq!(
        metadata["baseline_us"], before,
        "metadata must retain the baseline that actually produced reward {reward}; peer now {after}"
    );
    assert_eq!(metadata["beats_baseline"], expected_win);
    assert_eq!(metadata["gap_pct"], expected_gap);
}

#[test]
fn unchanged_winning_shape_baseline_stays_coherent() {
    coherence_case("stable-win", 100.0, 100.0, true);
}
#[test]
fn unchanged_losing_shape_baseline_stays_coherent() {
    coherence_case("stable-loss", 40.0, 40.0, true);
}
#[test]
fn winning_reward_keeps_scored_shape_baseline_after_peer_improves() {
    coherence_case("shape-win-to-loss", 100.0, 40.0, true);
}
#[test]
fn losing_reward_keeps_scored_shape_baseline_after_peer_regresses() {
    coherence_case("shape-loss-to-win", 40.0, 100.0, true);
}
#[test]
fn winning_reward_keeps_scored_geomean_fallback_after_peer_improves() {
    coherence_case("fallback-win-to-loss", 100.0, 40.0, false);
}
#[test]
fn losing_reward_keeps_scored_geomean_fallback_after_peer_regresses() {
    coherence_case("fallback-loss-to-win", 40.0, 100.0, false);
}
