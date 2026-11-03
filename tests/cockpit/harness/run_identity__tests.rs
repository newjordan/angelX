use super::*;

#[test]
fn run_identity_auxiliary_request_keeps_last_answer_receipt() {
    let _env = crate::tests::env_lock();
    let club =
        crate::agent::club::HttpClub::new("answer", "http://127.0.0.1:9/v1", "answer-model", None);
    crate::agent::harness::reset_turn_ledger(&club);
    crate::agent::harness::begin_model_request();
    let bind_model = |name: &str| {
        bind(
            Model {
                club: name.into(),
                driver: name.into(),
                id: name.into(),
                base_url: "fixture/v1".into(),
            },
            json!({"reasoning_effort":"low"}),
            json!(123),
        )
        .unwrap();
    };
    bind_model("answer-model");
    crate::agent::harness::end_model_request();
    select_answer_route(&crate::agent::club::RouteIdentity {
        driver: "answer-model".into(),
        model: Some("answer-model".into()),
        reasoning_effort: None,
    });
    let answer = serde_json::to_value(current_for_turn()).unwrap();
    bind_model("compact-model");
    assert_eq!(request_identity().unwrap().model.id, "compact-model");
    assert_eq!(
        serde_json::to_value(current_for_turn()).unwrap(),
        answer,
        "auxiliary work before cancellation must retain the last answer receipt"
    );
    crate::agent::harness::begin_model_request();
    bind_model("next-answer-model");
    crate::agent::harness::end_model_request();
    assert_eq!(current_for_turn().unwrap().model.id, "next-answer-model");
}

#[test]
fn run_identity_controls_skip_payload_and_preserve_wire_controls() {
    let wire = json!({"model":"fixture", "messages":[{"content":"private prompt"}],
        "tools":[{"description":"private schema"}], "api_key":"private key",
        "reasoning_effort":null, "reasoning":{"effort":"high"},
        "chat_template_kwargs":{"enable_thinking":false}, "max_completion_tokens":123});
    let controls = wire_identity_controls(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert_eq!(wire_effort(&controls), wire_effort(&wire));
    assert_eq!(controls["max_completion_tokens"], 123);
    for key in ["messages", "tools", "api_key"] {
        assert!(controls.get(key).is_none());
    }
    assert!(wire_identity_controls(b"{} trailing").is_err());
}

#[test]
fn run_identity_http_route_switch_updates_receipts_preserving_origin() {
    use crate::agent::club::{ChatMsg, Club};
    use std::io::{Read, Write};
    let _env = crate::tests::env_lock();
    let _pipe = crate::tests::TestEnvGuard::unset("ANGEL_PXPIPE");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let (requests, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let headers = String::from_utf8(request).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            requests
                .send(serde_json::from_slice::<Value>(&body).unwrap())
                .unwrap();
            let response = json!({"choices":[{"message":{"role":"assistant","content":"done"},
                "finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":2,
                "prompt_tokens_details":{"cached_tokens":0}}})
            .to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        }
    });
    let mut original = None;
    for (label, model) in [
        ("identity-first", "grok-4.7"),
        ("identity-second", "deepseek-v4-flash"),
    ] {
        let club = crate::agent::club::HttpClub::new(label, &endpoint, model, None);
        crate::agent::harness::reset_turn_ledger(&club);
        assert!(
            current_for_turn().is_none(),
            "a new turn cannot inherit the last seat"
        );
        crate::agent::harness::note_timing_origin(std::time::Instant::now());
        crate::agent::harness::begin_model_request();
        club.chat(&[ChatMsg::user("fixture")], &[]).unwrap();
        crate::agent::harness::end_model_request();
        select_answer_route(&club.resolved_route_identity());
        let wire = received
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        let identity = current_for_turn().unwrap();
        assert_eq!(identity.model.id, model);
        assert_eq!(identity.model.driver, label);
        assert_eq!(identity.model.base_url, endpoint_identity(&endpoint));
        assert_eq!(identity.effort, wire_effort(&wire));
        assert_eq!(
            identity.initial_request.as_ref().unwrap().model.id,
            current().unwrap().model.id
        );
        let immutable = serde_json::to_value(current()).unwrap();
        assert_eq!(
            immutable,
            *original.get_or_insert_with(|| immutable.clone())
        );
        let attempts = crate::agent::harness::provider_call_samples();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0]["identity"]["model"]["id"], model);
        assert_eq!(attempts[0]["identity"]["effort"], wire_effort(&wire));
    }
    server.join().unwrap();
    // A parallel synthesized answer has no single certified answering seat.
    select_answer_route(&Default::default());
    let identity = current_for_turn().unwrap();
    assert_eq!(identity.model.id, "unbound");
    assert_eq!(identity.model.base_url, "unbound");
    assert_eq!(identity.effort, "unbound");
    assert_eq!(serde_json::to_value(current()).unwrap(), original.unwrap());
}

fn leg(id: &str, generation: u64, digest: &str) -> LiveLeg {
    LiveLeg {
        id: id.into(),
        generation,
        digest: digest.into(),
        bend: None,
        error: None,
    }
}

fn bend_leg(id: &str, digest: &str, source: &str, output: &str) -> LiveLeg {
    LiveLeg {
        bend: Some(BendBuild {
            source_digest: source.into(),
            output_digest: output.into(),
        }),
        ..leg(id, 1, digest)
    }
}

/// Publishes the empty set again so a test never leaks legs into another.
struct LiveGuard;
impl Drop for LiveGuard {
    fn drop(&mut self) {
        publish_live(LiveSet::default());
    }
}

#[test]
fn live_set_hashes_content_not_generation_counters_or_directory() {
    let base = LiveSet::new(
        Some("/runs/a".into()),
        vec![leg("tool-router", 1, "aa"), leg("agent", 1, "bb")],
    );
    let moved = LiveSet::new(
        Some("/runs/b".into()),
        vec![leg("agent", 7, "bb"), leg("tool-router", 3, "aa")],
    );
    assert_eq!(base.sha256, moved.sha256);
    assert_eq!(base.legs[0].id, "agent", "legs are listed by id");
    let edited = LiveSet::new(
        None,
        vec![leg("tool-router", 2, "cc"), leg("agent", 1, "bb")],
    );
    assert_ne!(base.sha256, edited.sha256);
    // A candidate that never activated runs no code, so it changes nothing.
    let mut failed = leg("policy", 0, "");
    failed.error = Some("syntax error".into());
    let with_failed = LiveSet::new(
        None,
        vec![leg("tool-router", 1, "aa"), leg("agent", 1, "bb"), failed],
    );
    assert_eq!(base.sha256, with_failed.sha256);
    assert_eq!(
        with_failed.legs.len(),
        3,
        "the failed candidate stays visible"
    );
    assert_ne!(LiveSet::default().sha256, base.sha256);
}

#[test]
fn bend_receipt_digests_are_part_of_the_leg_identity() {
    let built =
        |output: &str| LiveSet::new(None, vec![bend_leg("ability-code", "aa", "src", output)]);
    assert_eq!(built("out").sha256, built("out").sha256);
    assert_ne!(built("out").sha256, built("other").sha256);
    let json = serde_json::to_value(&built("out").legs[0]).unwrap();
    assert_eq!(json["bend"]["source_digest"], "src");
    assert_eq!(json["bend"]["output_digest"], "out");
    assert!(
        serde_json::to_value(leg("agent", 1, "aa"))
            .unwrap()
            .get("bend")
            .is_none()
    );
}

#[test]
fn live_set_reads_loader_statuses_and_bounds_candidate_errors() {
    let statuses = [
        angel_live::Status {
            id: "ability-code".into(),
            generation: 4,
            digest: "d1".into(),
            error: None,
            build: Some(angel_live::BuildReceipt {
                source_digest: "s1".into(),
                output_digest: "o1".into(),
            }),
        },
        angel_live::Status {
            id: "tool-router".into(),
            generation: 2,
            digest: "d2".into(),
            error: Some("x".repeat(1000)),
            build: None,
        },
    ];
    let set = LiveSet::from_statuses(Some(Path::new("/live")), &statuses);
    assert_eq!(set.root.as_deref(), Some("/live"));
    assert_eq!(set.legs[0].generation, 4);
    assert_eq!(set.legs[0].bend.as_ref().unwrap().output_digest, "o1");
    assert_eq!(set.legs[1].error.as_ref().unwrap().len(), 240);
    assert_eq!(
        set.sha256,
        LiveSet::new(
            None,
            vec![
                bend_leg("ability-code", "d1", "s1", "o1"),
                leg("tool-router", 9, "d2")
            ]
        )
        .sha256
    );
}

#[test]
fn a_leg_change_moves_the_combined_identity_and_never_the_binary() {
    let _env = crate::tests::env_lock();
    let _live = LiveGuard;
    let bound = StaticIdentity {
        cockpit_source_sha256: "a".repeat(64),
        ..static_identity().unwrap().clone()
    };
    let unbound = StaticIdentity {
        cockpit_source_sha256: "unbound".into(),
        ..bound.clone()
    };
    assert!(bound.is_bound());
    assert!(!unbound.is_bound());
    let one = LiveSet::new(None, vec![leg("tool-router", 1, "aa")]);
    let two = LiveSet::new(None, vec![leg("tool-router", 2, "bb")]);
    let none = LiveSet::default();
    let combined = [&none, &one, &two].map(|live| combined_sha256(&bound, live));
    assert_eq!(combined[1], combined_sha256(&bound, &one));
    assert_ne!(combined[0], combined[1]);
    assert_ne!(combined[1], combined[2]);
    assert_ne!(combined[0], combined[2]);
    // The same legs under another binary are another identity.
    assert_ne!(
        combined_sha256(&bound, &one),
        combined_sha256(&unbound, &one)
    );

    let identity = |live: &LiveSet| {
        publish_live(live.clone());
        capture(
            Model {
                club: "fixture".into(),
                id: "fixture".into(),
                base_url: "none".into(),
                driver: "fixture".into(),
            },
            json!("none"),
            json!(1),
        )
        .unwrap()
    };
    let before = identity(&none);
    let after = identity(&one);
    assert_eq!(
        serde_json::to_value(&before.build).unwrap(),
        serde_json::to_value(&after.build).unwrap(),
        "a leg edit leaves the build fields alone"
    );
    assert_eq!(before.build.is_bound(), after.build.is_bound());
    assert_eq!(before.build.cockpit_source_sha256, source_sha256());
    assert_eq!(after.live, one);
    assert_ne!(before.combined_sha256, after.combined_sha256);
    assert_eq!(after.combined_sha256, combined_sha256(&after.build, &one));
}

#[test]
fn a_turn_pins_the_generation_published_at_its_start() {
    let _env = crate::tests::env_lock();
    let _live = LiveGuard;
    let model = || Model {
        club: "fixture".into(),
        id: "fixture".into(),
        base_url: "none".into(),
        driver: "fixture".into(),
    };
    let first = LiveSet::new(None, vec![leg("tool-router", 1, "aa")]);
    let second = LiveSet::new(None, vec![leg("tool-router", 2, "bb")]);
    publish_live(first.clone());
    reset_turn_identity();
    publish_live(second.clone());
    let pinned = capture(model(), json!("none"), json!(1)).unwrap();
    assert_eq!(
        pinned.live, first,
        "a mid-turn edit waits for the next turn"
    );
    reset_turn_identity();
    let next = capture(model(), json!("none"), json!(1)).unwrap();
    assert_eq!(next.live, second);
    assert_ne!(pinned.combined_sha256, next.combined_sha256);
}

#[test]
fn identity_records_without_legs_still_deserialize() {
    let _env = crate::tests::env_lock();
    let mut value = serde_json::to_value(
        capture(
            Model {
                club: "fixture".into(),
                id: "fixture".into(),
                base_url: "none".into(),
                driver: "fixture".into(),
            },
            json!("none"),
            json!(1),
        )
        .unwrap(),
    )
    .unwrap();
    for key in ["live", "combined_sha256"] {
        assert!(value.as_object_mut().unwrap().remove(key).is_some());
    }
    let old: RunIdentity = serde_json::from_value(value).unwrap();
    assert!(old.live.legs.is_empty());
    assert!(old.combined_sha256.is_empty());
}

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

#[cfg(target_os = "linux")]
#[test]
fn replaced_running_image_identity_fixture() {
    let Some(executable) = std::env::var_os("ANGEL_T_IDENTITY_RUNNING_IMAGE") else {
        return;
    };
    let _lock = crate::tests::env_lock();
    let executable = std::path::PathBuf::from(executable);
    assert_eq!(std::env::current_exe().unwrap(), executable);
    std::fs::remove_file(&executable).unwrap();
    let replacement = b"new pathname bytes must not identify the running process";
    std::fs::write(&executable, replacement).unwrap();
    assert!(!std::env::current_exe().unwrap().exists());

    let build = static_identity().unwrap();
    let expected = std::env::var("ANGEL_T_IDENTITY_RUNNING_SHA256").unwrap();
    assert_eq!(build.executable_sha256, expected);
    assert_ne!(
        build.executable_sha256,
        crate::knowledge::cut::sha256_hex(replacement)
    );
    assert!(build.executable_path.ends_with(" (deleted)"));
    assert_eq!(static_identity().unwrap().executable_sha256, expected);
    eprintln!("RUN_IDENTITY replaced_path=true running_image_preserved=true");
}

#[cfg(target_os = "linux")]
#[test]
fn run_identity_preserves_running_image_after_path_replacement() {
    use crate::agent::sandbox::process_owner::OwnedCommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-identity-running-image-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let executable = root.join("fixture-runner");
    let source = std::env::current_exe().unwrap();
    let expected = executable_sha256(Path::new("/proc/self/exe")).unwrap();
    // Never replace the suite's image. This test owns only the new link/copy and
    // the fixture unlinks it before writing replacement bytes at its pathname.
    if std::fs::hard_link(&source, &executable).is_err() {
        std::fs::copy(&source, &executable).unwrap();
    }
    let filter = format!(
        "{}::replaced_running_image_identity_fixture",
        module_path!()
            .split_once("::")
            .map_or(module_path!(), |(_, rest)| rest)
    );
    let mut child = Command::new(&executable)
        .args(["--exact", &filter, "--nocapture", "--test-threads=1"])
        .env("ANGEL_T_IDENTITY_RUNNING_IMAGE", &executable)
        .env("ANGEL_T_IDENTITY_RUNNING_SHA256", expected)
        .stdin(Stdio::null())
        .spawn_owned()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(12);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&root);
            panic!("running-image identity fixture exceeded its deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    std::fs::remove_dir_all(root).unwrap();
    assert!(status.success(), "running-image identity fixture failed");
}

#[test]
fn model_defaults_identity_capture() {
    let _env = crate::tests::env_lock();
    let _idle = crate::tests::TestEnvGuard::unset("ANGEL_TURN_IDLE_TIMEOUT_SECS");
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
    assert!(identity.budgets.get("stream_stall_secs").is_none());
    assert!(identity.budgets.get("codex_stream_stall_secs").is_none());
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
            "agent::harness::run_identity::tests::run_identity_carries_sealed_sandbox_when_active",
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
    let profile = crate::agent::sandbox::sealed::build(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap(),
        None,
    );
    crate::agent::sandbox::sealed::tests::test_activate(profile);
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
    let club =
        crate::agent::club::HttpClub::new("glm", "https://identity.invalid/v1", "glm-5.3", None);
    let body = club
        .build_body_with_effort(
            &[crate::agent::club::ChatMsg::user("fixture")],
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
    use crate::agent::club::{ChatMsg, Club};
    use crate::agent::harness::{TaskJsonContext, TaskJsonEnvelope, ToolRegistry, TurnEvent};
    let _env = crate::tests::env_lock();
    if std::env::var("ANGEL_T_IDENTITY_CHILD").as_deref() != Ok("1") {
        let root =
            std::env::temp_dir().join(format!("angel-identity-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "agent::harness::run_identity::tests::run_identity_scripted_task_json",
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
        crate::agent::sandbox::sealed::tests::test_activate(crate::agent::sandbox::sealed::build(
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
    let outcome = crate::agent::harness::run_turn_observed(
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
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        outcome,
        &history,
    );
    let value = serde_json::to_value(&envelope).unwrap();
    let id = &value["identity"];
    assert_eq!(
        value["authority_profile"],
        serde_json::to_value(crate::platform::authority_profile::active(true)).unwrap()
    );
    if sealed {
        assert_eq!(
            id["sandbox"],
            crate::agent::sandbox::sealed::identity().unwrap()
        );
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
        "live",
        "combined_sha256",
        "bound_at_ms",
    ] {
        assert!(!id[key].is_null(), "missing {key}");
    }
    assert_eq!(id["live"]["legs"], json!([]));
    assert_eq!(
        id["live"]["sha256"],
        serde_json::to_value(LiveSet::default().sha256).unwrap()
    );
    assert_eq!(id["dataset"]["kind"], "task_json");
    assert_eq!(id["budgets"]["max_hops"], 3);
    assert_eq!(id["budgets"]["turn_idle_timeout_secs"], 17);
    assert_eq!(id["budgets"]["compaction_budget_tokens"], 4096);
    assert_eq!(id["verifier"]["plan_kind"], "external-only");
    assert_eq!(
        crate::agent::harness::eval_trajectory_record(
            "identity-scripted",
            &history,
            "scripted answer",
            1.0,
            "fixture-manifest",
            1,
        )["identity"],
        *id
    );
    assert!(crate::agent::harness::ledger_status_text("").contains("model=identity-fixture-v1"));
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
    schema_rows.push(crate::agent::harness::eval_trajectory_record(
        "identity-scripted",
        &history,
        "scripted answer",
        1.0,
        "fixture-manifest",
        1,
    ));
    std::fs::write(&trace_fixture, serde_json::to_vec(&schema_rows).unwrap()).unwrap();
    if let Some(validator) = crate::tests::operator_script("runtime/trace_schema.py") {
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
    }
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
        Some(crate::knowledge::cut::sha256_hex(b"sealed fixture\n").as_str())
    );
    assert_eq!(dataset.path.as_deref(), path.to_str());
    std::fs::remove_file(&path).unwrap();
    assert!(Dataset::new("arena", Some(&path)).is_err());
}
