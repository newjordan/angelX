use super::*;

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn setup(root: &Path) -> PathBuf {
    std::fs::create_dir(root).unwrap();
    let workspace = root.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("fixture.txt"), "owned committed fixture\n").unwrap();
    for args in [
        vec!["init", "-q", "--template="],
        vec!["add", "fixture.txt"],
        vec![
            "-c",
            "user.name=Owned",
            "-c",
            "user.email=owned@example.invalid",
            "commit",
            "-q",
            "-m",
            "owned",
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
    workspace
}

fn fixture(f: impl FnOnce(&Path, &Path)) {
    let _lock = crate::tests::env_lock();
    let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "tests");
    let _log = crate::tests::TestEnvGuard::unset("ANGEL_TRAJECTORY_LOG");
    let _artifact = crate::tests::TestEnvGuard::unset("ANGEL_EVALUATOR_ARTIFACT_DIR");
    let _authority = crate::tests::TestEnvGuard::unset("ANGEL_CODING_TRAINING_AUTHORITY_DIR");
    let root = std::env::temp_dir().join(format!(
        "angel-coding-authority-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let workspace = setup(&root);
    let owned = Owned(root);
    f(&owned.0, &workspace);
}

const TASK: &str = "Keep the owned source intact — café";
const ANSWER: &str = "  Verified café\n";
const GREEN: &str = "printf '%s' 'test result: ok. 3 passed; 0 failed; 0 ignored;'";

struct LocalClub;
impl Club for LocalClub {
    fn label(&self) -> &str {
        "owned-no-model-coding-fixture"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Ok(ANSWER.into())
    }
}

fn real_producer(root: &Path, workspace: &Path) -> (CodingEvalReport, Value) {
    let store = root.join("authority");
    let _store = crate::tests::TestEnvGuard::set(
        "ANGEL_CODING_TRAINING_AUTHORITY_DIR",
        store.to_str().unwrap(),
    );
    let mut registry = crate::agent::harness::ToolRegistry::new();
    registry.set_workspace(workspace.to_path_buf());
    let report = run_coding_eval(&LocalClub, &registry, TASK, GREEN).unwrap();
    assert_eq!(report.answer, ANSWER);
    assert_eq!(report.reward, 1.0);
    assert!(report.training_capture_error.is_none());
    let id = report
        .training_decision
        .as_ref()
        .expect("actual producer decision");
    let bytes = read_object(&store, id, ".decision.json", JSON_LIMIT).unwrap();
    let decision: Decision = serde_json::from_slice(&bytes).unwrap();
    let request = json!({"schema":REQUEST_SCHEMA, "decision_sha256":id,
        "task":TASK, "answer":ANSWER, "reward":report.reward,
        "competition":decision.competition,
        "evaluator_evidence_manifest_sha256":decision.evaluator_evidence_manifest_sha256});
    (report, request)
}

fn evaluate(workspace: &Path, command: &str) -> EvaluatorEvidence {
    EvaluatorEvidence::run_shell(
        "owned training evaluator",
        command,
        workspace,
        TEST_VERIFIER_CONTRACT,
        &subject(TASK, ANSWER),
    )
    .unwrap()
}

#[test]
fn real_coding_eval_publishes_and_audits_exact_pair() {
    fixture(|root, workspace| {
        let (_, request) = real_producer(root, workspace);
        let bytes = serde_json::to_vec(&request).unwrap();
        let store = root.join("authority");
        let before = std::fs::read_dir(&store).unwrap().count();
        let receipt = audit(&store, &bytes).unwrap();
        assert_eq!(receipt["data_class"], "verified_coding_eval");
        assert_eq!(
            receipt["request_sha256"],
            crate::knowledge::cut::sha256_hex(&bytes)
        );
        assert_eq!(
            receipt["answer_sha256"],
            crate::knowledge::cut::sha256_hex(ANSWER.as_bytes())
        );
        assert_eq!(receipt["baseline"], Value::Null);
        assert_eq!(receipt["reward_contract"], "tests");
        assert_eq!(
            receipt["lineage"]["scope"],
            "evaluator_execution_and_exact_task_answer"
        );
        assert_eq!(std::fs::read_dir(&store).unwrap().count(), before);
        for (key, value) in [
            ("task", json!("different task")),
            ("answer", json!(ANSWER.trim())),
            ("reward", json!(0.99999999999)),
            ("competition", json!({"beats_baseline":true})),
            ("evaluator_evidence_manifest_sha256", json!("a".repeat(64))),
            ("decision_sha256", Value::Null),
            ("decision_sha256", json!("b".repeat(64))),
        ] {
            let mut changed = request.clone();
            changed[key] = value;
            assert!(
                audit(&store, &serde_json::to_vec(&changed).unwrap()).is_err(),
                "accepted changed {key}"
            );
        }
        let mut path_injection = request;
        path_injection["store"] = json!(root.join("untrusted"));
        assert!(audit(&store, &serde_json::to_vec(&path_injection).unwrap()).is_err());
        assert!(audit(&store, &vec![b' '; JSON_LIMIT + 1]).is_err());
    });
}

#[test]
fn original_peer_baseline_survives_change_and_replacement_is_rejected() {
    // env-lock-exempt: fixture in this module holds crate::tests::env_lock for the entire closure.
    fixture(|root, workspace| {
        let peer = root.join("peer.json");
        std::fs::write(
            &peer,
            r#"{"geomean_us":100.0,"name":"original","shapes":{"32768x1":100.0}}"#,
        )
        .unwrap();
        let _peer = crate::tests::TestEnvGuard::set("POPCORN_PEER_STATE", peer.to_str().unwrap());
        let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "popcorn_peer");
        let evidence = evaluate(
            workspace,
            "printf '%s' 'shape=32768x1 score_us=50us\ntest result: ok. 3 passed; 0 failed;' ",
        );
        let scoring = score_coding_eval(&evidence).unwrap();
        assert!((scoring.reward - 0.55).abs() < 1e-6);
        let store = root.join("authority");
        let published = publish(&store, TASK, ANSWER, &evidence, &scoring).unwrap();
        let request = json!({"schema":REQUEST_SCHEMA,"decision_sha256":published.decision_sha256,
            "task":TASK,"answer":ANSWER,"reward":published.reward,"competition":published.competition,
            "evaluator_evidence_manifest_sha256":published.manifest_sha256});
        std::fs::write(
            &peer,
            r#"{"geomean_us":40.0,"name":"changed-after-scoring","shapes":{"32768x1":40.0}}"#,
        )
        .unwrap();
        assert!(score_coding_eval(&evidence).unwrap().reward < 0.05);
        let receipt = audit(&store, &serde_json::to_vec(&request).unwrap()).unwrap();
        assert_eq!(receipt["baseline"]["baseline_us"], 100.0);
        let mut changed = request.clone();
        changed["competition"]["baseline_us"] = json!(40.0);
        assert!(audit(&store, &serde_json::to_vec(&changed).unwrap()).is_err());
        // Simulate corruption of the trusted object under its original ID; the
        // auditor does not accept a replacement baseline even if arithmetic fits.
        let path = store.join(format!("{}.decision.json", published.decision_sha256));
        let mut decision: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        decision["baseline"]["baseline_us"] = json!(40.0);
        std::fs::write(&path, serde_json::to_vec(&decision).unwrap()).unwrap();
        assert!(audit(&store, &serde_json::to_vec(&request).unwrap()).is_err());
    });
}

#[test]
fn failed_verifier_and_missing_authority_never_mint_training_decision() {
    // env-lock-exempt: fixture in this module holds crate::tests::env_lock for the entire closure.
    fixture(|root, workspace| {
        let evidence = evaluate(
            workspace,
            "printf '%s' 'test result: ok. 3 passed; 0 failed;'; exit 7",
        );
        let scoring = CodingEvalScore::without_competition(1.0);
        assert!(publish(&root.join("authority"), TASK, ANSWER, &evidence, &scoring).is_err());
        assert!(!root.join("authority").exists());
        let mut registry = crate::agent::harness::ToolRegistry::new();
        registry.set_workspace(workspace.into());
        let report = run_coding_eval(&LocalClub, &registry, TASK, GREEN).unwrap();
        assert_eq!(report.reward, 1.0);
        assert!(report.training_decision.is_none());
        let bad_store = root.join("not-a-directory");
        std::fs::write(&bad_store, b"owned").unwrap();
        let _authority = crate::tests::TestEnvGuard::set(
            "ANGEL_CODING_TRAINING_AUTHORITY_DIR",
            bad_store.to_str().unwrap(),
        );
        let report = run_coding_eval(&LocalClub, &registry, TASK, GREEN).unwrap();
        assert_eq!(report.reward, 1.0);
        assert!(report.training_decision.is_none() && report.training_capture_error.is_some());
    });
}

#[test]
fn artifact_tamper_missing_objects_and_symlinks_fail_closed() {
    fixture(|root, workspace| {
        let (_, request) = real_producer(root, workspace);
        let input = serde_json::to_vec(&request).unwrap();
        let store = root.join("authority");
        let manifest = request["evaluator_evidence_manifest_sha256"]
            .as_str()
            .unwrap();
        let path = store.join(format!("{manifest}.evidence"));
        let original = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"tampered").unwrap();
        assert!(audit(&store, &input).is_err());
        std::fs::remove_file(&path).unwrap();
        assert!(audit(&store, &input).is_err());
        #[cfg(unix)]
        {
            let other = root.join("untrusted-evidence");
            std::fs::write(&other, &original).unwrap();
            std::os::unix::fs::symlink(&other, &path).unwrap();
            assert!(audit(&store, &input).is_err());
            std::fs::remove_file(&path).unwrap();
        }
        std::fs::write(&path, original).unwrap();
        assert!(audit(&store, &input).is_ok());
        std::fs::remove_file(store.join(format!(
            "{}.decision.json",
            request["decision_sha256"].as_str().unwrap()
        )))
        .unwrap();
        assert!(audit(&store, &input).is_err());
    });
}

#[test]
#[ignore = "explicit owned /tmp fixture export for real consumer CLI validation"]
fn emit_owned_coding_training_fixture_for_consumer() {
    let _lock = crate::tests::env_lock();
    let _reward = crate::tests::TestEnvGuard::set("ANGEL_RL_REWARD", "tests");
    let _log = crate::tests::TestEnvGuard::unset("ANGEL_TRAJECTORY_LOG");
    let _artifact = crate::tests::TestEnvGuard::unset("ANGEL_EVALUATOR_ARTIFACT_DIR");
    let root = PathBuf::from(
        std::env::var_os("ANGEL_CODING_TRAINING_FIXTURE_EXPORT").expect("explicit fixture root"),
    );
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("angel-coding-training-owned-")
    );
    let workspace = setup(&root); // create_dir refuses existing operator content
    let (_, request) = real_producer(&root, &workspace);
    std::fs::write(
        root.join("request.json"),
        serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    let receipt = audit(
        &root.join("authority"),
        &serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("expected-receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    println!(
        "owned coding producer fixture retained at {}",
        root.display()
    );
}
