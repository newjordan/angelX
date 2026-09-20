use super::*;

#[test]
fn run_identity_executable_and_unbound() {
    let _env = crate::tests::env_lock();
    let build = static_identity().unwrap();
    let output = std::process::Command::new("sha256sum")
        .arg(std::env::current_exe().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        build.executable_sha256,
        String::from_utf8(output.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
    );
    assert_eq!(
        build.cockpit_source_sha256,
        option_env!("ANGEL_BUILD_SOURCE_SHA256").unwrap_or("unbound")
    );
    assert_eq!(
        build.toolchain.profile,
        option_env!("ANGEL_BUILD_PROFILE").unwrap_or("unbound")
    );
}

#[test]
fn model_defaults_identity_capture() {
    let _env = crate::tests::env_lock();
    let _idle = crate::tests::TestEnvGuard::unset("ANGEL_TURN_IDLE_TIMEOUT_SECS");
    let _stall = crate::tests::TestEnvGuard::unset("ANGEL_STREAM_STALL_SECS");
    let _effort = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
    let _grok = crate::tests::TestEnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
    let identity = capture(
        Model {
            club: "grok".into(),
            id: "grok-4.6".into(),
            base_url: "127.0.0.1".into(),
            driver: "grok".into(),
        },
        json!({"reasoning_effort":"low"}),
        json!(123),
    )
    .unwrap();
    assert_eq!(identity.budgets["reasoning_effort"], "low");
    assert_eq!(
        identity.budgets["reasoning_effort_source"],
        "table 2026-09-09"
    );
    assert_eq!(identity.budgets["stream_stall_secs"], 240);
    assert_eq!(identity.budgets["turn_idle_timeout_secs"], 0);
}

#[test]
fn run_identity_effective_budgets_and_wire_controls() {
    let _env = crate::tests::env_lock();
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "17");
    prepare_turn(Some(3), 4096, 2, 1024);
    let wire = json!({"reasoning_effort": "low", "thinking": {"type":"enabled"}});
    let id = capture(
        Model {
            club: "scripted".into(),
            id: "fixture".into(),
            base_url: "none".into(),
            driver: "scripted".into(),
        },
        wire_effort(&wire),
        json!(123),
    )
    .unwrap();
    assert_eq!(id.budgets["max_hops"], 3);
    assert_eq!(id.budgets["turn_idle_timeout_secs"], 17);
    assert_eq!(id.budgets["compaction_budget_tokens"], 4096);
    assert_eq!(id.budgets["output_tokens"], 123);
    assert_eq!(id.effort["reasoning_effort"], wire["reasoning_effort"]);
    assert_eq!(wire_effort(&json!({})), "none");
    assert_eq!(
        endpoint_identity("https://user:password@example.test:8443/v1?key=secret#secret"),
        "example.test:8443/v1"
    );
    assert_eq!(
        endpoint_identity("http://127.0.0.1:8080/v1"),
        "127.0.0.1:8080/v1 insecure-local"
    );
    assert_eq!(
        endpoint_identity("http://localhost:11434/v1"),
        "localhost:11434/v1 insecure-local"
    );
}

#[test]
fn run_identity_carries_sealed_sandbox_when_active() {
    let _env = crate::tests::env_lock();
    // Sealed activation is process-permanent (OnceLock), so the active
    // branch runs in a forked test binary of exactly this test.
    if !cfg!(target_os = "linux") {
        eprintln!(
            "SKIP run_identity_carries_sealed_sandbox_when_active: sealed requires Linux Landlock + bwrap; unsupported on {}",
            std::env::consts::OS
        );
        return;
    }
    if std::env::var("ANGEL_T_SEALED_CHILD").as_deref() != Ok("1") {
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "harness::run_identity::tests::run_identity_carries_sealed_sandbox_when_active",
            "--nocapture",
        ])
        .env("ANGEL_T_SEALED_CHILD", "1");
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert!(
        capture(
            Model {
                club: "scripted".into(),
                id: "fixture".into(),
                base_url: "none".into(),
                driver: "scripted".into(),
            },
            json!("none"),
            json!(123),
        )
        .unwrap()
        .sandbox
        .is_none()
    );
    let profile = crate::sandbox::sealed::build(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap(),
        None,
    );
    crate::sandbox::sealed::tests::test_activate(profile);
    let id = capture(
        Model {
            club: "scripted".into(),
            id: "fixture".into(),
            base_url: "none".into(),
            driver: "scripted".into(),
        },
        json!("none"),
        json!(123),
    )
    .unwrap();
    assert_eq!(id.sandbox.as_ref().unwrap()["name"], "sealed");
    let digest = id.sandbox.as_ref().unwrap()["digest"].as_str().unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
}

#[test]
fn run_identity_http_resolves_requested_none_to_wire_low() {
    let _env = crate::tests::env_lock();
    let club = crate::club::HttpClub::new("glm", "https://identity.invalid/v1", "glm-5.3", None);
    let body = club
        .build_body_with_effort(
            &[crate::club::ChatMsg::user("fixture")],
            &[],
            false,
            Some("none"),
        )
        .unwrap();
    // Exercise the real request builder without sending a request or opening a socket.
    assert_eq!(body["reasoning_effort"], "low");
    let id = capture(
        Model {
            club: "glm".into(),
            id: body["model"].as_str().unwrap().into(),
            base_url: endpoint_identity("https://identity.invalid/v1"),
            driver: "glm".into(),
        },
        wire_effort(&body),
        json!("provider-native"),
    )
    .unwrap();
    assert_eq!(id.effort["reasoning_effort"], body["reasoning_effort"]);
    assert_eq!(id.effort["thinking"], body["thinking"]);
}

#[test]
fn run_identity_scripted_task_json() {
    use crate::club::{ChatMsg, Club};
    use crate::harness::{TaskJsonContext, TaskJsonEnvelope, ToolRegistry, TurnEvent};
    let _env = crate::tests::env_lock();
    if std::env::var("ANGEL_T_IDENTITY_CHILD").as_deref() != Ok("1") {
        let root =
            std::env::temp_dir().join(format!("angel-identity-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "harness::run_identity::tests::run_identity_scripted_task_json",
            "--nocapture",
        ])
        .current_dir(&root)
        .env("ANGEL_T_IDENTITY_CHILD", "1")
        .env("ANGEL_EXPERIENCE", "0")
        .env("ANGEL_SKILL_HINT", "0")
        .env("ANGEL_AUTO_RECALL", "0")
        .env("ANGEL_AUTO_COMPACT", "0")
        .env("ANGEL_HARNESS_ROLLOUTS", "off")
        .env("ANGEL_CONTEXT_BUDGET_TOKENS", "4096")
        .env("ANGEL_TRAJECTORY_LOG", "1")
        .env("ANGEL_NEEDS_PRO", "0")
        .env("ANGEL_TURN_IDLE_TIMEOUT_SECS", "17")
        .env_remove("ANGEL_TASK_ACCEPT_CMD")
        .env_remove("ANGEL_FIRST_WRITE_CALLS");
        for key in [
            "ANGEL_ATLAS_DIR",
            "ANGEL_CADDY_DIR",
            "ANGEL_DOSSIER_DIR",
            "ANGEL_TRAJECTORY_DIR",
        ] {
            cmd.env(key, root.join(key));
        }
        for profile in ["ordinary", "sealed"] {
            cmd.env("ANGEL_T_IDENTITY_PROFILE", profile);
            cmd.env("ANGEL_TRAJECTORY_DIR", root.join(profile));
            let output = cmd.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    let sealed = std::env::var("ANGEL_T_IDENTITY_PROFILE").as_deref() == Ok("sealed");
    if sealed {
        crate::sandbox::sealed::tests::test_activate(crate::sandbox::sealed::build(
            &std::env::current_dir().unwrap(),
            None,
        ));
    }
    struct Scripted;
    impl Club for Scripted {
        fn respond(&self, _: &str) -> Result<String, String> {
            assert!(
                current().is_some(),
                "identity must precede the first model request"
            );
            Ok("scripted answer".into())
        }
        fn label(&self) -> &str {
            "identity-scripted"
        }
        fn model_identity(&self) -> Option<String> {
            Some("identity-fixture-v1".into())
        }
    }
    configure_dataset(Dataset::new("task_json", None).unwrap());
    let mut registry = ToolRegistry::new();
    registry.external_evaluator_only = true;
    let mut history = vec![ChatMsg::user("Say scripted answer.")];
    let outcome = crate::harness::run_turn_observed(
        &Scripted,
        &registry,
        &mut history,
        &std::sync::atomic::AtomicBool::new(false),
        Some(3),
        &std::sync::mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();
    let envelope = TaskJsonEnvelope::from_outcome(
        TaskJsonContext {
            task_id: Some("identity-fixture".into()),
            run_id: None,
            workspace: std::env::current_dir().unwrap(),
            club: Some("identity-scripted".into()),
            model: Some("identity-fixture-v1".into()),
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 0,
            timing: None,
            tools: vec![],
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: vec![],
            memory_health: crate::caddy::StoreHealthSummary::default(),
        },
        outcome,
        &history,
    );
    let value = serde_json::to_value(&envelope).unwrap();
    let id = &value["identity"];
    assert_eq!(
        value["authority_profile"],
        serde_json::to_value(crate::authority_profile::active(true)).unwrap()
    );
    if sealed {
        assert_eq!(id["sandbox"], crate::sandbox::sealed::identity().unwrap());
    } else {
        assert!(id.get("sandbox").is_none());
    }
    for key in [
        "executable_sha256",
        "executable_path",
        "cockpit_source_sha256",
        "toolchain",
        "model",
        "effort",
        "dataset",
        "budgets",
        "verifier",
        "bound_at_ms",
    ] {
        assert!(!id[key].is_null(), "missing {key}");
    }
    assert_eq!(id["dataset"]["kind"], "task_json");
    assert_eq!(id["budgets"]["max_hops"], 3);
    assert_eq!(id["budgets"]["turn_idle_timeout_secs"], 17);
    assert_eq!(id["budgets"]["compaction_budget_tokens"], 4096);
    assert_eq!(id["verifier"]["plan_kind"], "external-only");
    assert_eq!(
        crate::harness::eval_trajectory_record(
            "identity-scripted",
            &history,
            "scripted answer",
            1.0,
            "fixture-manifest",
            1,
        )["identity"],
        *id
    );
    assert!(crate::harness::ledger_status_text("").contains("model=identity-fixture-v1"));
    let log_dir = std::path::PathBuf::from(std::env::var_os("ANGEL_TRAJECTORY_DIR").unwrap());
    let mut rows = Vec::new();
    for entry in std::fs::read_dir(&log_dir).unwrap().flatten() {
        if entry.path().extension().is_some_and(|ext| ext == "jsonl") {
            for line in std::fs::read_to_string(entry.path()).unwrap().lines() {
                let row: Value = serde_json::from_str(line).unwrap();
                assert_eq!(&row["identity"], id);
                rows.push(row);
            }
        }
    }
    assert!(!rows.is_empty());
    let trace_fixture = std::env::current_dir().unwrap().join("schema-records.json");
    let mut schema_rows = rows.clone();
    schema_rows.push(crate::harness::eval_trajectory_record(
        "identity-scripted",
        &history,
        "scripted answer",
        1.0,
        "fixture-manifest",
        1,
    ));
    std::fs::write(&trace_fixture, serde_json::to_vec(&schema_rows).unwrap()).unwrap();
    let validator =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/trace_schema.py");
    let checked = std::process::Command::new("python3")
        .arg(validator)
        .arg(&trace_fixture)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
    if let Some(dir) = std::env::var_os("ANGEL_T_IDENTITY_RECEIPT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::write(
            dir.join("example-identity.json"),
            serde_json::to_vec_pretty(id).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join("example-task-result.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join("example-trajectory.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn run_identity_dataset_file_digest_and_read_failure() {
    let _env = crate::tests::env_lock();
    let path =
        std::env::temp_dir().join(format!("angel-identity-dataset-{}.txt", std::process::id()));
    std::fs::write(&path, b"sealed fixture\n").unwrap();
    let dataset = Dataset::new("arena", Some(&path)).unwrap();
    assert_eq!(
        dataset.sha256.as_deref(),
        Some(crate::cut::sha256_hex(b"sealed fixture\n").as_str())
    );
    assert_eq!(dataset.path.as_deref(), path.to_str());
    std::fs::remove_file(&path).unwrap();
    assert!(Dataset::new("arena", Some(&path)).is_err());
}
