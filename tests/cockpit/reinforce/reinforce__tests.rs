use super::*;

fn evidence(output: &str) -> EvaluatorEvidence {
    let process_output = verifier_process_output(output).unwrap();
    EvaluatorEvidence::from_executed_output(
        "test verifier",
        "printf %s <fixture>",
        Path::new("."),
        CODE_HEALTH_VERIFIER_CONTRACT,
        "test subject",
        process_output,
    )
    .unwrap()
}

fn verifier_process_output(output: &str) -> Result<std::process::Output, String> {
    // Absolute path: under a full parallel suite the process PATH can be
    // briefly hostile while other tests mutate env, and "printf" alone
    // then fails with ENOENT. The binary itself is standard on Linux.
    std::process::Command::new("/usr/bin/printf")
        .args(["%s", output])
        .output()
        .map_err(|error| error.to_string())
}

#[test]
fn reverse_task_is_deterministic_and_is_a_reversal() {
    assert_eq!(reverse_task(6, 42), reverse_task(6, 42), "deterministic");
    let (prompt, expected) = reverse_task(6, 42);
    let pd: Vec<&str> = prompt
        .trim_start_matches("reverse: ")
        .split_whitespace()
        .collect();
    let ed: Vec<&str> = expected.split_whitespace().collect();
    assert_eq!(pd.len(), 6);
    assert_eq!(
        pd.iter().rev().copied().collect::<Vec<_>>(),
        ed,
        "expected is the reversal"
    );
}

#[test]
fn reverse_reward_is_dense_and_exact() {
    let (_, e) = reverse_task(4, 7);
    assert_eq!(reverse_reward_score(&e, &e), 1.0, "exact = ceiling");
    assert_eq!(reverse_reward_score("nonsense words", &e), 0.0);
    // reasoning/prose prefix is tolerated (trailing K digit tokens are scored).
    assert_eq!(reverse_reward_score(&format!("the answer is {e}"), &e), 1.0);
    // corrupt one of 4 positions → dense partial reward (0.75).
    let mut toks: Vec<String> = e.split_whitespace().map(String::from).collect();
    toks[0] = ((toks[0].parse::<u8>().unwrap() + 1) % 10).to_string();
    let r = reverse_reward_score(&toks.join(" "), &e);
    assert!((r - 0.75).abs() < 1e-6, "one of four wrong → 0.75, got {r}");
}

#[test]
fn reinforce_control_metrics_track_known_quality() {
    // The control: drive evaluate_batch with a synthetic policy of KNOWN
    // quality and confirm the RLVR metrics follow it — solve_rate tracks the
    // fraction correct, and the GRPO "learning signal" (advantage spread) is
    // present only in the productive middle band, not when pinned all-pass /
    // all-fail.
    let cfg = ReinforceConfig::default(); // success_threshold = 1.0

    let run = |q: f32| {
        let (expected, cands) = control_group(q, 6, 8, 1);
        evaluate_batch(&ReverseControlReward { expected }, 0, &cfg, cands).1
    };

    let all = run(1.0);
    assert_eq!(all.solve_rate, 1.0);
    assert!(
        !all.has_learning_signal(),
        "all-pass → no advantage to learn from"
    );

    let none = run(0.0);
    assert_eq!(none.solve_rate, 0.0);
    assert!(!none.has_learning_signal(), "all-fail → no signal");

    let mid = run(0.5);
    assert!(
        (mid.solve_rate - 0.5).abs() < 0.13,
        "solve_rate tracks quality: {}",
        mid.solve_rate
    );
    assert!(
        mid.has_learning_signal(),
        "middle band must expose a learning signal"
    );
}

/// Reward = the candidate's output parsed as a number (tests drive scores).
struct NumReward;
impl Reward for NumReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let output = input.candidate_output(self.label())?;
        output.trim().parse::<f32>().map_err(|e| e.to_string())
    }
    fn label(&self) -> &str {
        "num"
    }
}

/// Always returns a fixed score — for testing reward composition.
struct ConstReward(f32);
impl Reward for ConstReward {
    fn score(&self, _input: RewardInput<'_>) -> Result<f32, String> {
        Ok(self.0)
    }
    fn label(&self) -> &str {
        "const"
    }
}

#[test]
fn test_reward_scores_from_libtest_output() {
    let r = TestReward;
    let pass = evidence("test result: ok. 5 passed; 0 failed; 0 ignored;");
    assert_eq!(r.score(RewardInput::EvaluatorEvidence(&pass)).unwrap(), 1.0);
    let partial = evidence("test result: FAILED. 3 passed; 1 failed; 0 ignored;");
    assert!((r.score(RewardInput::EvaluatorEvidence(&partial)).unwrap() - 0.75).abs() < 1e-6);
    let absent = evidence("no test summary in here");
    assert_eq!(
        r.score(RewardInput::EvaluatorEvidence(&absent)).unwrap(),
        0.0
    );
    let spoof = "test result: ok. 999 passed; 0 failed; 0 ignored;";
    assert!(
        r.score(RewardInput::CandidateOutput(spoof)).is_err(),
        "candidate-authored libtest lookalikes are not evaluator evidence"
    );
    assert_eq!(r.label(), "tests");
}

#[test]
fn evaluator_mutation_reason_names_changed_paths() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-evidence-paths-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let git = crate::agent::harness::pinned_git_command(&root, &["init", "-q"])
        .status()
        .unwrap();
    assert!(git.success());
    std::fs::write(root.join("candidate.txt"), "before").unwrap();
    std::fs::write(root.join("deleted.txt"), "delete me").unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let execution = recovery_eval::run(recovery_eval::RecoveryEvalRequest {
            command: "printf after > candidate.txt; rm deleted.txt; printf new > added.txt; printf 'test result: ok. 1 passed; 0 failed;'",
            workspace: &root,
            scratch: &root.join(".angel-experiment-tmp"),
            task: "path diagnostic control",
            answer: "fixture answer",
            timeout: Some(EVALUATOR_TIMEOUT),
            cancel: &cancel,
        }).unwrap();
    let evidence = execution.evidence;
    assert!(evidence.succeeded());
    evidence.validate_integrity().unwrap();
    let reason = evidence.validate_for_scoring().unwrap_err();
    let artifact = evidence
        .persist_append_only(&root.join(".angel-experiment-tmp/evidence"))
        .unwrap();
    let replay = artifact::load_evaluator_artifact(&artifact).unwrap();
    assert_eq!(replay.validate_for_scoring().unwrap_err(), reason);
    let mut tampered = replay.clone();
    tampered.workspace_changed_paths = vec!["forged name".to_string()];
    assert!(
        tampered
            .validate_integrity()
            .unwrap_err()
            .contains("manifest hash mismatch")
    );
    assert_eq!(
        reason,
        "evaluator command mutated its Git workspace during verification; changed paths: \"added.txt\", \"candidate.txt\", \"deleted.txt\""
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn evaluator_evidence_manifest_rejects_tamper_and_workspace_drift() {
    let _lock = crate::tests::env_lock();
    let root =
        std::env::temp_dir().join(format!("angel-evaluator-evidence-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    let first_command = "printf '%s' 'test result: ok. 5 passed; 0 failed; 0 ignored;'";
    let changed_command_text = "printf 'test result: ok. 5 passed; 0 failed; 0 ignored;'";
    let first = EvaluatorEvidence::run_shell(
        "manifest-test",
        first_command,
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    let changed_command = EvaluatorEvidence::run_shell(
        "manifest-test",
        changed_command_text,
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    let changed_subject = EvaluatorEvidence::run_shell(
        "manifest-test",
        first_command,
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v2",
    )
    .unwrap();
    assert_ne!(first.command_sha256(), changed_command.command_sha256());
    assert_ne!(first.manifest_sha256(), changed_command.manifest_sha256());
    assert_ne!(first.subject_sha256(), changed_subject.subject_sha256());
    assert_ne!(first.manifest_sha256(), changed_subject.manifest_sha256());

    if first.sandbox_stderr_prefix_len != 0 {
        let launcher_line =
            std::str::from_utf8(&first.stderr[..first.sandbox_stderr_prefix_len]).unwrap();
        let spoof = EvaluatorEvidence::run_shell(
            "launcher-prefix-spoof",
            &format!("printf '%s' '{}' >&2", launcher_line.replace('\'', "'\\''")),
            &root,
            CODE_HEALTH_VERIFIER_CONTRACT,
            "candidate-v1",
        )
        .unwrap();
        assert!(
            !spoof.stderr_is_empty(),
            "evaluator-authored diagnostic lookalikes remain stderr"
        );
        assert_eq!(spoof.evaluator_stderr(), launcher_line.as_bytes());

        let mut boundary_tamper = first.clone();
        boundary_tamper.sandbox_stderr_prefix_len = 0;
        boundary_tamper.output = combined_process_output(&first.stdout, &first.stderr);
        assert!(
            boundary_tamper
                .validate_integrity()
                .unwrap_err()
                .contains("manifest hash mismatch")
        );

        let artifacts = crate::tests::TestGitWorkspace::new("launcher-evidence-roundtrip");
        let artifact = first.persist_append_only(artifacts.path()).unwrap();
        let restored = artifact::load_evaluator_artifact(&artifact).unwrap();
        assert_eq!(
            restored.stderr, first.stderr,
            "raw launcher bytes survive persistence"
        );
        assert_eq!(
            restored.sandbox_stderr_prefix_len,
            first.sandbox_stderr_prefix_len
        );
        assert_eq!(restored.output(), first.output());
    }

    let mut raw_tamper = first.clone();
    raw_tamper.stdout.push(b'!');
    raw_tamper.output = combined_process_output(&raw_tamper.stdout, raw_tamper.evaluator_stderr());
    assert!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&raw_tamper))
            .unwrap_err()
            .contains("raw-output hash mismatch")
    );

    let mut manifest_tamper = first.clone();
    manifest_tamper.command_sha256 = crate::knowledge::cut::sha256_hex(b"cargo test --forged");
    assert!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&manifest_tamper))
            .unwrap_err()
            .contains("manifest hash mismatch")
    );

    let split_channels = EvaluatorEvidence::run_shell(
        "raw-boundary-test",
        "printf a; printf b >&2",
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    let one_channel = EvaluatorEvidence::run_shell(
        "raw-boundary-test",
        "printf 'a\nb'",
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    assert_eq!(split_channels.output(), one_channel.output());
    assert_ne!(
        split_channels.raw_output_sha256(),
        one_channel.raw_output_sha256(),
        "stdout/stderr boundaries are part of raw evidence"
    );

    let invalid_ff = EvaluatorEvidence::run_shell(
        "raw-byte-test",
        "printf '\\377'",
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    let invalid_fe = EvaluatorEvidence::run_shell(
        "raw-byte-test",
        "printf '\\376'",
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    assert_eq!(invalid_ff.output(), invalid_fe.output());
    assert_ne!(
        invalid_ff.raw_output_sha256(),
        invalid_fe.raw_output_sha256(),
        "lossy display text must not define raw evidence identity"
    );

    std::fs::write(root.join("tracked.txt"), "before").unwrap();
    git(&["add", "tracked.txt"]);
    let workspace_evidence_before = EvaluatorEvidence::run_shell(
        "workspace-test",
        first_command,
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    assert_eq!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&workspace_evidence_before))
            .unwrap(),
        1.0
    );

    let bare_tool = EvaluatorEvidence::run_shell(
        "bare-tool-control",
        "uname",
        &root,
        TEST_VERIFIER_CONTRACT,
        "bare-tool-subject",
    )
    .unwrap();
    assert!(!bare_tool.succeeded());
    assert!(bare_tool.output().contains("not found"));
    std::fs::write(root.join("tracked.txt"), "after").unwrap();
    let workspace_evidence_after = EvaluatorEvidence::run_shell(
        "workspace-test",
        first_command,
        &root,
        CODE_HEALTH_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    assert_ne!(
        workspace_evidence_before.workspace_sha256(),
        workspace_evidence_after.workspace_sha256()
    );
    assert_ne!(
        workspace_evidence_before.manifest_sha256(),
        workspace_evidence_after.manifest_sha256()
    );
    assert_eq!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&workspace_evidence_before))
            .unwrap(),
        1.0,
        "later workspace changes must not retroactively invalidate a captured receipt"
    );
    let _ = std::fs::remove_dir_all(&root);

    let non_git =
        std::env::temp_dir().join(format!("angel-evaluator-no-git-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&non_git);
    std::fs::create_dir_all(&non_git).unwrap();
    assert!(
        EvaluatorEvidence::run_shell(
            "non-git",
            first_command,
            &non_git,
            CODE_HEALTH_VERIFIER_CONTRACT,
            "candidate-v1",
        )
        .unwrap_err()
        .contains("Git-backed workspace")
    );
    let _ = std::fs::remove_dir_all(&non_git);
}

#[test]
fn evaluator_execution_rejects_timeout_truncation_and_source_mutation() {
    let root = std::env::temp_dir().join(format!(
        "angel-evaluator-execution-controls-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let git_bin = if std::path::Path::new("/usr/bin/git").exists() {
        "/usr/bin/git"
    } else {
        "git"
    };
    let git = |args: &[&str]| {
        let status = std::process::Command::new(git_bin)
            .args(args)
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    std::fs::write(root.join("tracked.txt"), "before").unwrap();
    git(&["add", "tracked.txt"]);

    let sanitized = EvaluatorEvidence::run_shell(
        "sanitized-env",
        "test \"${USER-unset}\" = unset && printf '%s' 'test result: ok. 1 passed; 0 failed;'",
        &root,
        TEST_VERIFIER_CONTRACT,
        "sanitized-env-subject",
    )
    .unwrap();
    assert_eq!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&sanitized))
            .unwrap(),
        1.0
    );

    let local_ipc = EvaluatorEvidence::run_shell(
            "unix-socket-control",
            "/usr/bin/python3 -c 'import socket; a,b=socket.socketpair(); a.close(); b.close()'; printf '%s' 'test result: ok. 1 passed; 0 failed;'",
            &root,
            TEST_VERIFIER_CONTRACT,
            "unix-socket-subject",
        )
        .unwrap();
    assert_eq!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&local_ipc))
            .unwrap(),
        1.0,
        "network denial must retain local Unix-domain IPC"
    );
    if !cfg!(target_os = "linux") {
        eprintln!(
            "SKIP evaluator_execution_rejects_timeout_truncation_and_source_mutation network-denial: Landlock/seccomp absent on {}",
            std::env::consts::OS
        );
    } else {
        let network_denied = EvaluatorEvidence::run_shell(
            "network-denial-control",
            "/usr/bin/python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_DGRAM)'",
            &root,
            TEST_VERIFIER_CONTRACT,
            "network-denial-subject",
        )
        .unwrap();
        assert!(!network_denied.succeeded());
        assert!(
            String::from_utf8_lossy(&network_denied.stderr).contains("Operation not permitted")
        );
    }

    let first = EvaluatorEvidence::run_shell(
        "execution-id",
        "printf '%s' 'test result: ok. 1 passed; 0 failed;'",
        &root,
        TEST_VERIFIER_CONTRACT,
        "same-subject",
    )
    .unwrap();
    let second = EvaluatorEvidence::run_shell(
        "execution-id",
        "printf '%s' 'test result: ok. 1 passed; 0 failed;'",
        &root,
        TEST_VERIFIER_CONTRACT,
        "same-subject",
    )
    .unwrap();
    assert_ne!(first.execution_id(), second.execution_id());
    assert_ne!(first.manifest_sha256(), second.manifest_sha256());

    let timed_out = EvaluatorEvidence::run_shell_with_timeout(
        "timeout-control",
        "printf '%s' 'test result: ok. 1 passed; 0 failed;'; /usr/bin/sleep 5",
        &root,
        TEST_VERIFIER_CONTRACT,
        "timeout-subject",
        Duration::from_millis(100),
    )
    .unwrap();
    assert!(timed_out.timed_out());
    assert!(timed_out.duration_ns() < Duration::from_secs(3).as_nanos() as u64);
    assert!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&timed_out))
            .unwrap_err()
            .contains("pinned timeout")
    );

    let truncated = EvaluatorEvidence::run_shell(
            "truncation-control",
            "/usr/bin/head -c 1048577 /dev/zero | /usr/bin/tr '\\000' x; printf '%s' 'test result: ok. 1 passed; 0 failed;'",
            &root,
            TEST_VERIFIER_CONTRACT,
            "truncation-subject",
        )
        .unwrap();
    assert!(truncated.output_truncated());
    assert!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&truncated))
            .unwrap_err()
            .contains("complete-capture limit")
    );

    let mutated = EvaluatorEvidence::run_shell(
        "mutation-control",
        "printf changed > tracked.txt; printf '%s' 'test result: ok. 1 passed; 0 failed;'",
        &root,
        TEST_VERIFIER_CONTRACT,
        "mutation-subject",
    )
    .unwrap();
    assert_eq!(
        mutated.workspace_before_sha256(),
        mutated.workspace_sha256()
    );
    assert!(!mutated.succeeded());
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        "before"
    );
    assert!(
        TestReward
            .score(RewardInput::EvaluatorEvidence(&mutated))
            .unwrap_err()
            .contains("evaluator command failed")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A club that always answers "done" — exercises run_turn without tools
/// (relies on Club::chat's default, which wraps respond as Text).
struct DoneClub;
impl Club for DoneClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("done".to_string())
    }
    fn label(&self) -> &str {
        "done"
    }
}

#[test]
fn coding_eval_drives_and_scores() {
    // Serialized against every other env-mutating test: this one and the
    // popcorn floor test below both drive `ANGEL_RL_REWARD`, and in a
    // parallel run each would observe the other's value between its own
    // set and restore.
    let _guard = crate::tests::env_lock();
    let reg = crate::agent::harness::ToolRegistry::new();
    let club = DoneClub;
    // Ensure default RLVR path (not leftover GpuComp popcorn pin).
    let prev_rl = std::env::var_os("ANGEL_RL_REWARD");
    let prev_gpu = std::env::var_os("ANGEL_GPU_COMP_LOCAL_MOA");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_RL_REWARD") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GPU_COMP_LOCAL_MOA") };

    // Passing verification → full reward.
    let rep = run_coding_eval(
        &club,
        &reg,
        "make the tests pass",
        "printf '%s' 'test result: ok. 4 passed; 0 failed; 0 ignored;'",
    )
    .unwrap();
    assert_eq!(rep.answer, "done");
    assert_eq!(rep.test.passed, 4);
    assert_eq!(rep.reward, 1.0);
    // Coding evals are the RLVR faucet: always report Hi/Q root view.
    assert!(rep.root_trajectory.get("fingerprint").is_some());
    assert!(rep.harness_treatment.get("handle_store").is_some());

    // Half the tests fail → half reward (verifiable, not a judge guess).
    let rep2 = run_coding_eval(
        &club,
        &reg,
        "x",
        "printf '%s' 'test result: FAILED. 1 passed; 1 failed; 0 ignored;'",
    )
    .unwrap();
    assert!((rep2.reward - 0.5).abs() < 1e-6, "reward {}", rep2.reward);

    match prev_rl {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_RL_REWARD", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_RL_REWARD") },
    }
    match prev_gpu {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_GPU_COMP_LOCAL_MOA", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_GPU_COMP_LOCAL_MOA") },
    }
}

#[test]
fn score_coding_eval_reward_popcorn_vs_hold_floor() {
    // See `coding_eval_drives_and_scores`: both tests own `ANGEL_RL_REWARD`
    // for their duration, so they must not run concurrently. Without this
    // the popcorn pin is cleared mid-test and `coding_eval_competition_meta`
    // returns None — the intermittent "popcorn meta" failure.
    let _guard = crate::tests::env_lock();
    let peer = std::env::temp_dir().join(format!(
        "angel-coding-eval-peer-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::write(
            &peer,
            r#"{"geomean_us":867.91,"name":"c3","shapes":{"32768x1":38800.0},"shape_bests":{"32768x1":{"us":38300.0,"name":"r7"}}}"#,
        )
        .unwrap();
    let prev_state = std::env::var_os("POPCORN_PEER_STATE");
    let prev_rl = std::env::var_os("ANGEL_RL_REWARD");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("POPCORN_PEER_STATE", &peer) };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_RL_REWARD", "popcorn_peer") };
    assert!(popcorn_reward_active());

    // Verifier emits PRIMARY shape timing under HOLD → positive reward.
    let under = EvaluatorEvidence::run_shell(
        "popcorn-under-hold",
        "printf '%s' 'shape=32768x1 score_us=38000.0 17/17 tests passed'",
        Path::new("."),
        TEST_VERIFIER_CONTRACT,
        "cand-under",
    )
    .unwrap();
    let under_r = score_coding_eval_reward("PRIMARY attack 32768x1", &under).unwrap();
    let above = EvaluatorEvidence::run_shell(
        "popcorn-above-hold",
        "printf '%s' 'shape=32768x1 score_us=38600.0 still under board'",
        Path::new("."),
        TEST_VERIFIER_CONTRACT,
        "cand-above",
    )
    .unwrap();
    let above_r = score_coding_eval_reward("PRIMARY attack 32768x1", &above).unwrap();
    assert!(
        under_r > above_r,
        "under HOLD {under_r} should beat above HOLD {above_r}"
    );
    assert!(under_r > 0.05);

    // Pure libtest verify under popcorn still labels via TestReward fallback.
    let tests = EvaluatorEvidence::run_shell(
        "libtest-fallback",
        "printf '%s' 'test result: ok. 2 passed; 0 failed; 0 ignored;'",
        Path::new("."),
        TEST_VERIFIER_CONTRACT,
        "cand-tests",
    )
    .unwrap();
    let test_r = score_coding_eval_reward("no timing in answer", &tests).unwrap();
    assert_eq!(test_r, 1.0);

    // Competition meta stamps score_us + shape for Forge join.
    let under_scoring = score_coding_eval(&under).unwrap();
    let meta = coding_eval_competition_meta(&under, &under_scoring).expect("popcorn meta");
    assert_eq!(meta["reward_contract"], "popcorn_peer");
    assert_eq!(meta["lesson"], "coding_eval");
    assert_eq!(meta["scope"], "shape");
    assert!((meta["score_us"].as_f64().unwrap() - 38000.0).abs() < 0.1);
    assert_eq!(meta["shape_key"], "32768x1");
    assert_eq!(meta["shape_n"], 32768);
    assert_eq!(meta["primary_hold"], true);
    assert_eq!(meta["beats_baseline"], true);
    assert!((meta["baseline_us"].as_f64().unwrap() - 38300.0).abs() < 0.1);
    assert!(
        coding_eval_competition_meta(&tests, &score_coding_eval(&tests).unwrap()).is_none(),
        "libtest-only verifies must not stamp competition"
    );

    let _ = std::fs::remove_file(&peer);
    match prev_state {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("POPCORN_PEER_STATE", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("POPCORN_PEER_STATE") },
    }
    match prev_rl {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_RL_REWARD", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_RL_REWARD") },
    }
}

#[test]
fn lint_reward_scores_from_clippy_output() {
    let r = LintReward;
    let clean = evidence("    Finished in 0.2s");
    assert_eq!(
        r.score(RewardInput::EvaluatorEvidence(&clean)).unwrap(),
        1.0
    );
    let w = "warning: unused\nwarning: dead code\nwarning: pkg generated 2 warnings";
    let warnings = evidence(w);
    assert!((r.score(RewardInput::EvaluatorEvidence(&warnings)).unwrap() - 1.0 / 3.0).abs() < 1e-6);
    let error = evidence("error[E0001]: bad");
    assert_eq!(
        r.score(RewardInput::EvaluatorEvidence(&error)).unwrap(),
        0.0
    );
    let test_only = EvaluatorEvidence::run_shell(
        "test-only",
        "printf '%s' 'test result: ok. 2 passed; 0 failed;'",
        Path::new("."),
        TEST_VERIFIER_CONTRACT,
        "candidate-v1",
    )
    .unwrap();
    assert!(
        r.score(RewardInput::EvaluatorEvidence(&test_only))
            .unwrap_err()
            .contains("does not accept evaluator contract"),
        "test-only evidence cannot mint lint credit"
    );
    assert!(r.score(RewardInput::CandidateOutput(w)).is_err());
    assert_eq!(r.label(), "lint");
}

#[test]
fn code_health_blends_tests_and_lint() {
    let c = CompositeReward::code_health();
    // all tests pass + clean lint → 1.0
    let clean = "test result: ok. 3 passed; 0 failed; 0 ignored;\n    Finished in 0.2s";
    let clean_evidence = evidence(clean);
    assert!(
        (c.score(RewardInput::EvaluatorEvidence(&clean_evidence))
            .unwrap()
            - 1.0)
            .abs()
            < 1e-6,
        "got {}",
        c.score(RewardInput::EvaluatorEvidence(&clean_evidence))
            .unwrap()
    );
    // tests pass + 1 warning → 0.8*1.0 + 0.2*0.5 = 0.9
    let warn = "test result: ok. 3 passed; 0 failed; 0 ignored;\nwarning: unused variable";
    let warn_evidence = evidence(warn);
    assert!(
        (c.score(RewardInput::EvaluatorEvidence(&warn_evidence))
            .unwrap()
            - 0.9)
            .abs()
            < 1e-6,
        "got {}",
        c.score(RewardInput::EvaluatorEvidence(&warn_evidence))
            .unwrap()
    );
    assert!(c.score(RewardInput::CandidateOutput(clean)).is_err());
}

#[test]
fn composite_reward_blends_weighted() {
    let c = CompositeReward::new()
        .with(Box::new(ConstReward(1.0)), 3.0)
        .with(Box::new(ConstReward(0.0)), 1.0);
    // weighted average: (1.0*3 + 0.0*1) / 4 = 0.75
    assert!((c.score(RewardInput::CandidateOutput("anything")).unwrap() - 0.75).abs() < 1e-6);
    // an empty composite is an error, not a silent zero.
    assert!(
        CompositeReward::new()
            .score(RewardInput::CandidateOutput("x"))
            .is_err()
    );
}

fn cand(version: u64, ms: u64, out: &str) -> Candidate {
    Candidate {
        policy_version: version,
        latency: Duration::from_millis(ms),
        output: out.to_string(),
    }
}

#[test]
fn staleness_budget_rejects_old_candidates() {
    let cfg = ReinforceConfig {
        staleness_budget: 16,
        ..Default::default()
    };
    let cands = vec![cand(0, 100, "1.0"), cand(100, 100, "2.0")]; // gap 100 vs 0
    let (rolls, m) = evaluate_batch(&NumReward, 100, &cfg, cands);
    assert_eq!(
        m.rejected_stale, 1,
        "the version-0 candidate is 100 steps stale"
    );
    assert!(!rolls[0].accepted && rolls[1].accepted);
}

#[test]
fn straggler_is_pruned() {
    let cfg = ReinforceConfig {
        straggler_factor: 2.0,
        staleness_budget: 1000,
        ..Default::default()
    };
    // medians ~10ms; the 500ms one is a straggler (> 2x median).
    let cands = vec![cand(0, 10, "1.0"), cand(0, 10, "1.0"), cand(0, 500, "1.0")];
    let (_r, m) = evaluate_batch(&NumReward, 0, &cfg, cands);
    assert_eq!(m.rejected_straggler, 1);
}

#[test]
fn disabled_straggler_pruning_preserves_actual_latencies() {
    let cfg = ReinforceConfig {
        straggler_factor: 0.0,
        ..Default::default()
    };
    let candidates = vec![cand(0, 10, "1.0"), cand(0, 10, "0.0"), cand(0, 500, "1.0")];
    let (rollouts, metrics) = evaluate_batch(&NumReward, 0, &cfg, candidates);
    assert_eq!(metrics.rejected_straggler, 0);
    assert_eq!(metrics.accepted, 3);
    assert_eq!(metrics.median_latency, Duration::from_millis(10));
    assert_eq!(metrics.p99_latency, Duration::from_millis(500));
    assert_eq!(rollouts[2].latency, Duration::from_millis(500));
}

#[test]
fn zero_advantage_is_detected() {
    let cfg = ReinforceConfig {
        staleness_budget: 1000,
        ..Default::default()
    };
    // All rewards equal → no advantage → skip the optimizer step.
    let cands = vec![cand(0, 10, "1.0"), cand(0, 10, "1.0"), cand(0, 10, "1.0")];
    let (_r, m) = evaluate_batch(&NumReward, 0, &cfg, cands);
    assert!(m.advantage_variance < 1e-6);
    assert!(!m.has_learning_signal());
}

#[test]
fn varied_rewards_have_learning_signal() {
    let cfg = ReinforceConfig {
        staleness_budget: 1000,
        success_threshold: 1.0,
        ..Default::default()
    };
    // Mixed solve/fail with spread → productive middle band.
    let cands = vec![
        cand(0, 10, "2.0"), // solve
        cand(0, 10, "0.0"), // fail
        cand(0, 10, "1.0"), // solve (>= threshold)
        cand(0, 10, "0.5"), // fail
    ];
    let (_r, m) = evaluate_batch(&NumReward, 0, &cfg, cands);
    assert!(m.advantage_variance > 0.0);
    assert!(m.solve_rate > 0.0 && m.solve_rate < 1.0);
    assert!(m.has_learning_signal());
    assert!((m.acceptance_rate() - 1.0).abs() < 1e-6);
}

#[test]
fn dispatch_count_oversamples() {
    let cfg = ReinforceConfig {
        group_size: 8,
        oversample: 0.6,
        ..Default::default()
    };
    assert_eq!(cfg.dispatch_count(), 13); // ceil(8 * 1.6)
}

/// A generator that returns fixed outputs (reward = output as number).
struct VecGen(Vec<String>);
impl Generator for VecGen {
    fn generate(&self, _p: &str, _t: &str, version: u64, _n: usize) -> Vec<Candidate> {
        self.0
            .iter()
            .map(|o| Candidate {
                policy_version: version,
                latency: Duration::from_millis(1),
                output: o.clone(),
            })
            .collect()
    }
}
struct FixedReflector;
impl Reflector for FixedReflector {
    fn improve(&self, _p: &str, _t: &str, _b: &str, _w: &str) -> Result<String, String> {
        Ok("improved prompt".to_string())
    }
}

#[test]
fn reinforce_promotes_a_real_change_once_not_repeated_noops() {
    let r#gen = VecGen(vec!["3".into(), "4".into(), "5".into(), "6".into()]);
    let cfg = ReinforceConfig {
        group_size: 2,
        staleness_budget: 1000,
        success_threshold: 5.0, // solve rate 0.5 → learning signal
        ..Default::default()
    };
    let rep = run_reinforce_unchecked_for_ablation(
        &r#gen,
        &NumReward,
        &FixedReflector,
        "task",
        "init",
        3,
        &cfg,
    );
    assert_eq!(rep.rounds.len(), 3);
    assert!(rep.rounds[0].optimized);
    assert!(rep.rounds[1..].iter().all(|round| !round.optimized));
    assert_eq!(rep.rounds.last().unwrap().version, 1);
    assert_eq!(rep.final_prompt, "improved prompt");
    assert_eq!(rep.rounds[0].best_reward, 6.0);
}

/// LIVE end-to-end: one reinforce round with spark as generator + judge +
/// reflector. Opt-in (ANGEL_LIVE_REINFORCE=1); skips if the fleet is down.
#[test]
fn live_reinforce_one_round() {
    use crate::agent::club::HttpClub;
    if std::env::var("ANGEL_LIVE_REINFORCE").is_err() {
        eprintln!("set ANGEL_LIVE_REINFORCE=1 to run the live reinforce test; skipping");
        return;
    }
    let Ok(url) = std::env::var("ANGEL_SPARK_URL") else {
        eprintln!("set ANGEL_SPARK_URL to an explicitly trusted endpoint; skipping");
        return;
    };
    let q = HttpClub::new("spark", url, "qwopus-coder", None);
    if !q.is_ready() {
        eprintln!("spark down; skipping");
        return;
    }
    let club: Arc<dyn Club> = Arc::new(q);
    let task = "Write a single punchy one-line tagline for a terminal AI cockpit named Angel.";
    let r#gen = ClubGenerator {
        club: Arc::clone(&club),
    };
    let reward = JudgeReward {
        judge: Arc::clone(&club),
        task: task.to_string(),
    };
    let reflector = ClubReflector {
        club: Arc::clone(&club),
    };
    let cfg = ReinforceConfig {
        group_size: 2,
        oversample: 0.0, // keep the live call count small (2 gen + 2 judge + 1 reflect)
        staleness_budget: 1000,
        success_threshold: 6.0,
        straggler_factor: 100.0, // don't prune on the first noisy timings
    };
    let rep = run_reinforce_unchecked_for_ablation(
        &r#gen,
        &reward,
        &reflector,
        task,
        "You are a helpful assistant.",
        1,
        &cfg,
    );
    assert_eq!(rep.rounds.len(), 1);
    let r0 = &rep.rounds[0];
    eprintln!("metrics: {:?}", r0.metrics);
    eprintln!(
        "best_reward: {}  optimized: {}",
        r0.best_reward, r0.optimized
    );
    eprintln!("final prompt: {}", rep.final_prompt);
    assert!(r0.metrics.generated >= 2, "should have generated a group");
    assert!(
        r0.best_reward.is_finite(),
        "judge should have scored something"
    );
}

#[test]
fn reinforce_skips_when_zero_advantage() {
    // All rewards equal → no advantage → no optimizer step, prompt unchanged.
    let r#gen = VecGen(vec!["5".into(), "5".into(), "5".into()]);
    let cfg = ReinforceConfig {
        group_size: 2,
        staleness_budget: 1000,
        success_threshold: 4.0,
        ..Default::default()
    };
    let rep = run_reinforce_unchecked_for_ablation(
        &r#gen,
        &NumReward,
        &FixedReflector,
        "task",
        "init",
        3,
        &cfg,
    );
    assert!(rep.rounds.iter().all(|r| !r.optimized));
    assert_eq!(rep.final_prompt, "init");
    assert_eq!(rep.rounds.last().unwrap().version, 0);
}

/// A generator where the reflected prompt looks useful on the training
/// batch but regresses the private validation task.
struct RegressingGateGen;
impl Generator for RegressingGateGen {
    fn generate(&self, prompt: &str, task: &str, version: u64, n: usize) -> Vec<Candidate> {
        (0..n)
            .map(|index| {
                let score = if task.starts_with("private-validation") {
                    if prompt == "improved prompt" {
                        0.4
                    } else {
                        0.9
                    }
                } else if index % 2 == 0 {
                    1.0
                } else {
                    0.0
                };
                Candidate {
                    policy_version: version,
                    latency: Duration::from_millis(1),
                    output: score.to_string(),
                }
            })
            .collect()
    }
}

struct NeverGenerate;
impl Generator for NeverGenerate {
    fn generate(
        &self,
        _system_prompt: &str,
        _task: &str,
        _version: u64,
        _n: usize,
    ) -> Vec<Candidate> {
        panic!("cohort role rejection must happen before generation")
    }
}

#[test]
fn heldout_gate_rejects_regression_that_unchecked_ablation_promotes() {
    let cfg = ReinforceConfig {
        group_size: 4,
        oversample: 0.0,
        staleness_budget: 1000,
        straggler_factor: 100.0,
        success_threshold: 0.5,
    };
    let heldout = [
        HeldoutCase {
            id: "private-a",
            task: "private-validation",
            reward: &NumReward,
        },
        HeldoutCase {
            id: "private-b",
            task: "private-validation-2",
            reward: &NumReward,
        },
    ];
    let promotion_cfg = PromotionConfig {
        min_cases: 2,
        samples_per_case: 4,
        absolute_floor: 0.8,
        min_mean_delta: 0.01,
        max_case_regression: 0.0,
        confidence_z: 1.96,
    };
    let promotion_manifest = CohortManifest::new(
        "private-selection-v1",
        CohortRole::Promotion,
        &heldout,
        &promotion_cfg,
    )
    .unwrap();

    let gated = run_nontechnical_reinforce(
        &RegressingGateGen,
        &NumReward,
        &FixedReflector,
        ReinforceRequest {
            task: "training",
            initial_prompt: "init",
            rounds: 1,
            config: &cfg,
        },
        PromotionCohort {
            cases: &heldout,
            config: &promotion_cfg,
            manifest: &promotion_manifest,
        },
    )
    .unwrap();
    assert!(!gated.rounds[0].optimized);
    assert_eq!(gated.final_prompt, "init");
    assert_eq!(
        gated.rounds[0].promotion.as_ref().unwrap().decision,
        promotion::PromotionDecision::RejectedBelowFloor
    );

    let unchecked = run_reinforce_unchecked_for_ablation(
        &RegressingGateGen,
        &NumReward,
        &FixedReflector,
        "training",
        "init",
        1,
        &cfg,
    );
    assert!(unchecked.rounds[0].optimized);
    assert_eq!(unchecked.final_prompt, "improved prompt");
    assert!(unchecked.rounds[0].promotion.is_none());
}

#[test]
fn reinforce_rejects_final_audit_manifest_before_generation() {
    let cfg = ReinforceConfig {
        group_size: 2,
        oversample: 0.0,
        staleness_budget: 1,
        straggler_factor: 2.0,
        success_threshold: 0.5,
    };
    let cases = [
        HeldoutCase {
            id: "audit-a",
            task: "private-validation",
            reward: &NumReward,
        },
        HeldoutCase {
            id: "audit-b",
            task: "private-validation-2",
            reward: &NumReward,
        },
    ];
    let promotion_cfg = PromotionConfig {
        min_cases: 2,
        samples_per_case: 2,
        absolute_floor: 0.5,
        min_mean_delta: 0.01,
        max_case_regression: 0.0,
        confidence_z: 1.96,
    };
    let final_audit = CohortManifest::new(
        "untouched-final-v1",
        CohortRole::FinalAudit,
        &cases,
        &promotion_cfg,
    )
    .unwrap();
    let error = run_nontechnical_reinforce(
        &NeverGenerate,
        &NumReward,
        &FixedReflector,
        ReinforceRequest {
            task: "training",
            initial_prompt: "init",
            rounds: 1,
            config: &cfg,
        },
        PromotionCohort {
            cases: &cases,
            config: &promotion_cfg,
            manifest: &final_audit,
        },
    )
    .unwrap_err();
    assert!(error.contains("expected promotion"), "got: {error}");
}

#[test]
fn parse_score_finds_first_number_with_prose_and_decimals() {
    assert_eq!(parse_score("7"), Some(7.0));
    assert_eq!(parse_score("I'd rate this a 8 out of 10"), Some(8.0));
    assert_eq!(parse_score("score: 9.5/10"), Some(9.5));
    // Only the first decimal point is consumed; the rest stops the token.
    assert_eq!(parse_score("4.25.15"), Some(4.25));
    // No digits at all → None.
    assert_eq!(parse_score("no number here"), None);
    assert_eq!(parse_score(""), None);
}

#[test]
fn acceptance_rate_guards_zero_and_variance_handles_small_sets() {
    let empty = BatchMetrics::default();
    assert_eq!(empty.acceptance_rate(), 0.0, "no generations → 0, not NaN");
    let some = BatchMetrics {
        generated: 4,
        accepted: 3,
        ..Default::default()
    };
    assert!((some.acceptance_rate() - 0.75).abs() < 1e-6);
    // variance is defined as 0 for <2 samples; >0 for a real spread.
    assert_eq!(variance(std::iter::empty::<f32>()), 0.0);
    assert_eq!(variance([5.0f32].into_iter()), 0.0);
    assert!(variance([0.0f32, 1.0].into_iter()) > 0.0);
}

#[test]
fn dispatch_count_oversamples_and_ceils() {
    let cfg = ReinforceConfig {
        group_size: 8,
        oversample: 0.25,
        ..Default::default()
    };
    // 8 * 1.25 = 10
    assert_eq!(cfg.dispatch_count(), 10);
    let cfg2 = ReinforceConfig {
        group_size: 3,
        oversample: 0.1, // 3 * 1.1 = 3.3 → ceil 4
        ..Default::default()
    };
    assert_eq!(cfg2.dispatch_count(), 4);
}

#[test]
fn evaluate_batch_all_stale_yields_zero_solve_rate_and_no_signal() {
    let cfg = ReinforceConfig {
        staleness_budget: 0, // current_version 5 vs policy 0 → stale
        ..Default::default()
    };
    let cands = vec![cand(0, 5, "1"), cand(0, 5, "2"), cand(0, 5, "3")];
    let (rollouts, m) = evaluate_batch(&NumReward, 5, &cfg, cands);
    assert!(
        rollouts.iter().all(|r| !r.accepted),
        "all rejected as stale"
    );
    assert_eq!(m.rejected_stale, 3);
    assert_eq!(m.accepted, 0);
    assert_eq!(m.solve_rate, 0.0, "empty accepted set → 0 solve rate");
    assert!(!m.has_learning_signal());
}

/// A club that echoes one canned reply — drives the club-backed actors.
struct SayClub(String);
impl Club for SayClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok(self.0.clone())
    }
    fn label(&self) -> &str {
        "say"
    }
}

#[test]
fn club_generator_maps_text_calls_and_errors() {
    // Text reply → that text; the generator requests no tools.
    let r#gen = ClubGenerator {
        club: Arc::new(SayClub("an answer".into())),
    };
    let cands = r#gen.generate("sys", "task", 7, 3);
    assert_eq!(cands.len(), 3);
    assert!(cands.iter().all(|c| c.output == "an answer"));
    assert!(cands.iter().all(|c| c.policy_version == 7));

    // An erroring club surfaces a "(generation error: …)" output, not a panic.
    // (The default Club::chat wraps respond, so an Err propagates through.)
    struct ErrClub;
    impl Club for ErrClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Err("backend down".into())
        }
        fn label(&self) -> &str {
            "err"
        }
    }
    let gen2 = ClubGenerator {
        club: Arc::new(ErrClub),
    };
    let c2 = gen2.generate("s", "t", 0, 1);
    assert!(
        c2[0].output.contains("generation error"),
        "got {}",
        c2[0].output
    );
}

#[test]
fn judge_reward_parses_score_and_errors_on_none() {
    let good = JudgeReward {
        judge: Arc::new(SayClub("I give it a 6".into())),
        task: "t".into(),
    };
    assert_eq!(
        good.score(RewardInput::CandidateOutput("output")).unwrap(),
        6.0
    );
    assert_eq!(good.label(), "judge");
    // A reply with no number is an error (not silently zero).
    let bad = JudgeReward {
        judge: Arc::new(SayClub("no idea".into())),
        task: "t".into(),
    };
    assert!(bad.score(RewardInput::CandidateOutput("output")).is_err());
}

#[test]
fn club_reflector_returns_the_clubs_new_prompt() {
    let r = ClubReflector {
        club: Arc::new(SayClub("improved system prompt".into())),
    };
    let out = r.improve("old", "task", "best", "worst").unwrap();
    assert_eq!(out, "improved system prompt");
}

#[test]
fn composite_default_is_empty_and_zero_weights_error() {
    // Default == new() == no components → score is an error.
    assert!(
        CompositeReward::default()
            .score(RewardInput::CandidateOutput("x"))
            .is_err()
    );
    assert_eq!(CompositeReward::new().label(), "composite");
    // All-zero weights → a distinct error, not a divide-by-zero.
    let z = CompositeReward::new().with(Box::new(ConstReward(1.0)), 0.0);
    let err = z.score(RewardInput::CandidateOutput("x")).unwrap_err();
    assert!(err.contains("weights sum to zero"), "got: {err}");
}

#[test]
fn reverse_reward_empty_expected_is_zero_and_control_label() {
    assert_eq!(reverse_reward_score("1 2 3", ""), 0.0);
    let rc = ReverseControlReward {
        expected: "1 2".into(),
    };
    assert_eq!(rc.label(), "reverse-control");
    assert_eq!(rc.score(RewardInput::CandidateOutput("1 2")).unwrap(), 1.0);
}

#[test]
fn popcorn_peer_parses_score_us_and_rewards_wins() {
    assert_eq!(
        PopcornPeerReward::parse_score_us("score_us=850.5 peer ok"),
        Some(850.5)
    );
    assert_eq!(
        PopcornPeerReward::parse_score_us("geomean_us: 867.9"),
        Some(867.9)
    );
    assert!((PopcornPeerReward::parse_score_us("⏱ 61.2 ± 0.4 µs").unwrap() - 61.2).abs() < 1e-9);
    let win = PopcornPeerReward::score_us_against_baseline(850.0, 868.0);
    let lose = PopcornPeerReward::score_us_against_baseline(900.0, 868.0);
    assert!(win > lose);
    assert!(win > 0.05);
    let r = PopcornPeerReward::new().with_baseline(868.0);
    assert_eq!(r.label(), "popcorn_peer");
    let s = r
        .score(RewardInput::CandidateOutput("leaderboard score_us=850.0"))
        .unwrap();
    assert!((s - win).abs() < 1e-5);
    assert!(
        r.score(RewardInput::CandidateOutput("no timing here"))
            .is_err()
    );
}

#[test]
fn popcorn_peer_parses_shape_key_forms() {
    assert_eq!(
        PopcornPeerReward::parse_shape_key("PRIMARY attack shape=32768x1 hold"),
        Some("32768x1".into())
    );
    assert_eq!(
        PopcornPeerReward::parse_shape_key("board 512x640 open lever"),
        Some("512x640".into())
    );
    assert_eq!(
        PopcornPeerReward::parse_shape_key("n=32768 batch=1 scored"),
        Some("32768x1".into())
    );
}

#[test]
fn popcorn_peer_shape_baseline_uses_hold_floor() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let peer = std::env::temp_dir().join(format!(
        "angel-peer-reward-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::write(
            &peer,
            r#"{"geomean_us":867.91,"name":"c3","shapes":{"32768x1":38800.0},"shape_bests":{"32768x1":{"us":38300.0,"name":"r7"}}}"#,
        )
        .unwrap();
    let prev = std::env::var_os("POPCORN_PEER_STATE");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("POPCORN_PEER_STATE", &peer) };
    let r = PopcornPeerReward::new();
    // Under HOLD (38300) → win; between HOLD and board → soft loss vs HOLD.
    let under_hold = r
        .score(RewardInput::CandidateOutput(
            "shape=32768x1 score_us=38000.0 PRIMARY",
        ))
        .unwrap();
    let above_hold = r
        .score(RewardInput::CandidateOutput(
            "shape=32768x1 score_us=38500.0 still under board",
        ))
        .unwrap();
    assert!(under_hold > above_hold, "{under_hold} vs {above_hold}");
    assert!(under_hold > 0.05);
    let _ = std::fs::remove_file(&peer);
    match prev {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("POPCORN_PEER_STATE", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("POPCORN_PEER_STATE") },
    }
}

#[test]
fn reward_from_env_resolves_popcorn_and_code_health() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let prev = std::env::var_os("ANGEL_RL_REWARD");
    let prev_gpu = std::env::var_os("ANGEL_GPU_COMP_LOCAL_MOA");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_RL_REWARD", "popcorn_peer") };
    assert_eq!(reward_from_env().label(), "popcorn_peer");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_RL_REWARD", "code_health") };
    assert_eq!(reward_from_env().label(), "composite");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_RL_REWARD") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GPU_COMP_LOCAL_MOA") };
    assert_eq!(reward_from_env().label(), "composite");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_GPU_COMP_LOCAL_MOA", "1") };
    assert_eq!(reward_from_env().label(), "popcorn_peer");
    match prev {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_RL_REWARD", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_RL_REWARD") },
    }
    match prev_gpu {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_GPU_COMP_LOCAL_MOA", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_GPU_COMP_LOCAL_MOA") },
    }
}
