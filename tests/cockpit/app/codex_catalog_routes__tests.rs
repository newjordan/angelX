use super::*;
use crate::agent::club::codex_selection::Selection;
use crate::agent::codex_catalog::{CatalogLoad, Source};

fn catalog(source: Source, window: Option<u64>) -> Arc<CatalogLoad> {
    Arc::new(CatalogLoad::from_models(
        source,
        vec![CodexModelInfo {
            slug: "gpt-6.1-sol".into(),
            display_name: "Sol".into(),
            context_window: window,
            input_modalities: vec!["text".into(), "image".into()],
            default_reasoning_level: "high".into(),
            supported_reasoning_levels: ["low", "medium", "high", "xhigh", "max"]
                .iter()
                .map(|l| CodexReasoningLevel {
                    effort: l.to_string(),
                    description: String::new(),
                })
                .collect(),
            pi_thinking: [
                ("minimal".into(), Some("low".into())),
                ("xhigh".into(), Some("xhigh".into())),
                ("max".into(), Some("max".into())),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        }],
    ))
}
fn intent(model: &str, effort: &str) -> Selection {
    Selection {
        model: model.into(),
        effort: effort.into(),
        model_source: "env",
        effort_source: "env",
        error: Some("initial projection is not route authority".into()),
    }
}
fn route(catalog: Arc<CatalogLoad>, selection: Selection) -> CodexClub {
    let spec = &catalog.models()[0];
    CodexClub::new_with_route_metadata_shared(
        "openai",
        selection.model.clone(),
        CodexClub::shared_state(ChatGptAuth::detached()),
        Some(selection.effort.clone()),
        spec.supported_reasoning_levels
            .iter()
            .map(|l| l.effort.clone())
            .collect(),
        spec.route_metadata(),
    )
    .with_catalog_selection(selection, catalog)
}
#[test]
fn catalog_r3_valid_think_correction_recovers_dispatch_defaults_and_revision() {
    let club = route(
        catalog(Source::PiStore, Some(8192)),
        intent("gpt-6.1-sol", "invented"),
    );
    assert!(club.capture_selection(None).is_err());
    assert!(club.resolved_model_defaults()["selection_error"].is_string());
    let old = club.route_state_revision();
    assert_eq!(club.set_reasoning_effort("high").as_deref(), Some("high"));
    assert!(club.route_state_revision() > old);
    let effective = club.capture_selection(None).unwrap();
    assert_eq!(
        club.defaults_for_selection(&effective)["selection_error"],
        serde_json::Value::Null
    );
    assert_eq!(club.resolved_model_defaults()["reasoning_effort"], "high");
    assert_eq!(
        club.build_request_with_effort(&[ChatMsg::user("test")], &[], effective.wire.as_deref())["reasoning"]
            ["effort"],
        "high"
    );
    let old = club.route_state_revision();
    assert_eq!(club.set_reasoning_effort("invented"), None);
    assert_eq!(club.route_state_revision(), old);
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
}
#[test]
fn catalog_r3_unknown_model_and_explicit_failure_block_overrides_before_auth() {
    let unknown = route(
        catalog(Source::PiStore, Some(8192)),
        intent("unknown", "high"),
    );
    unknown.set_reasoning_effort("low");
    assert!(
        unknown
            .capture_selection(Some("high"))
            .unwrap_err()
            .contains("model")
    );
    assert!(
        unknown
            .chat_with_effort(&[ChatMsg::user("never send")], &[], Some("high"))
            .unwrap_err()
            .contains("model")
    );
    let mut club = route(
        catalog(Source::PiStore, Some(8192)),
        intent("gpt-6.1-sol", "high"),
    );
    club.catalog = Some(Arc::new(CatalogLoad::load(
        Some("pi-store:relative"),
        || panic!("legacy"),
    )));
    assert!(
        club.chat(&[ChatMsg::user("never send")], &[])
            .unwrap_err()
            .contains("Selector")
    );
}
#[test]
fn catalog_r3_override_capture_isolated_from_concurrent_route_mutation_and_receipt() {
    let mut selection = intent("gpt-6.1-sol", "pi:minimal");
    selection.effort_source = "codex-config";
    let club = Arc::new(route(catalog(Source::PiStore, Some(8192)), selection));
    let ordinary = club.capture_selection(None).unwrap();
    let capture = club.capture_selection(Some("pi:high")).unwrap();
    assert_eq!(club.reasoning_effort().as_deref(), Some("low"));
    assert_eq!(
        club.resolved_model_defaults()["requested_reasoning_effort"],
        "pi:minimal"
    );
    let other = Arc::clone(&club);
    std::thread::spawn(move || {
        other.set_reasoning_effort("max").unwrap();
    })
    .join()
    .unwrap();
    let body =
        club.build_request_with_effort(&[ChatMsg::user("test")], &[], capture.wire.as_deref());
    let receipt = club.defaults_for_selection(&capture);
    assert_eq!(receipt["requested_reasoning_effort"], "pi:high");
    assert_eq!(receipt["reasoning_effort_source"], "env");
    let ordinary_receipt = club.defaults_for_selection(&ordinary);
    assert_eq!(ordinary_receipt["reasoning_effort"], "low");
    assert_eq!(ordinary_receipt["reasoning_effort_source"], "codex-config");
    assert_eq!(body["reasoning"]["effort"], receipt["reasoning_effort"]);
    assert_eq!(body["reasoning"]["effort"], "high");
    assert_eq!(
        club.capture_selection(None).unwrap().wire.as_deref(),
        Some("max")
    );
    assert_eq!(club.responses_url, RESPONSES_URL);
    assert!(body.get("max_output_tokens").is_none());
}
#[test]
fn catalog_r3_native_dispatch_to_fixture_receipt_matches_override_and_correction() {
    use base64::Engine;
    use std::io::{Read, Write};
    let _lock = crate::tests::env_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut bytes = Vec::new();
        let (header_len, body_len) = loop {
            let mut part = [0u8; 4096];
            let n = socket.read(&mut part).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&part[..n]);
            if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..end]);
                let len = header
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                break (end + 4, len);
            }
        };
        while bytes.len() < header_len + body_len {
            let mut part = [0u8; 4096];
            let n = socket.read(&mut part).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&part[..n]);
        }
        let body: serde_json::Value =
            serde_json::from_slice(&bytes[header_len..header_len + body_len]).unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"fixture-ok\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n").unwrap();
        body
    });
    let mut club = route(
        catalog(Source::PiStore, Some(8192)),
        intent("gpt-6.1-sol", "invented"),
    );
    let payload = serde_json::to_vec(&serde_json::json!({"exp":now_secs()+3600})).unwrap();
    club.shared.auth.lock().unwrap().access_token = format!(
        "e30.{}.fixture",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload)
    );
    club.responses_url_override = Some(format!("http://{address}"));
    assert!(club.chat(&[ChatMsg::user("invalid")], &[]).is_err());
    club.set_reasoning_effort("high").unwrap();
    assert!(
        matches!(club.chat_with_effort(&[ChatMsg::user("fixture")], &[], Some("pi:minimal")).unwrap(), ClubReply::Text(ref s) if s == "fixture-ok")
    );
    let body = server.join().unwrap();
    let receipt =
        crate::agent::harness::run_identity::current_for_turn().expect("native request receipt");
    assert_eq!(body["reasoning"]["effort"], "low");
    assert_eq!(
        receipt.effort["reasoning"]["effort"],
        body["reasoning"]["effort"]
    );
    assert_eq!(receipt.budgets["requested_reasoning_effort"], "pi:minimal");
    assert_eq!(receipt.budgets["resolved_reasoning_effort"], "low");
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
    assert!(body.get("max_output_tokens").is_none());
}
#[test]
fn native_launch_cli_capture_overrides_mutation_and_reaches_real_local_receipt() {
    use base64::Engine;
    use std::io::{Read, Write};
    let _lock = crate::tests::env_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut bytes = Vec::new();
        let (header_len, body_len) = loop {
            let mut part = [0u8; 4096];
            let n = socket.read(&mut part).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&part[..n]);
            if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..end]);
                let len = header
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                break (end + 4, len);
            }
        };
        while bytes.len() < header_len + body_len {
            let mut part = [0u8; 4096];
            let n = socket.read(&mut part).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&part[..n]);
        }
        let body: serde_json::Value =
            serde_json::from_slice(&bytes[header_len..header_len + body_len]).unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"fixture-ok\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n").unwrap();
        body
    });
    let mut selection = intent("gpt-6.1-sol", "high");
    selection.model_source = "cli";
    let mut inner = route(catalog(Source::PiStore, Some(8192)), selection);
    let payload = serde_json::to_vec(&serde_json::json!({"exp":now_secs()+3600})).unwrap();
    inner.shared.auth.lock().unwrap().access_token = format!(
        "e30.{}.fixture",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload)
    );
    inner.responses_url_override = Some(format!("http://{address}"));
    let inner = Arc::new(inner);
    let bound = crate::agent::club::launch_effort(inner.clone(), "pi:minimal", "cli").unwrap();
    inner.set_reasoning_effort("max").unwrap();
    assert_eq!(
        bound.route_identity().reasoning_effort.as_deref(),
        Some("low")
    );
    assert!(
        matches!(bound.chat(&[ChatMsg::user("literal fixture")], &[]).unwrap(), ClubReply::Text(ref s) if s == "fixture-ok")
    );
    let body = server.join().unwrap();
    let receipt = crate::agent::harness::run_identity::current_for_turn().unwrap();
    assert_eq!(body["model"], "gpt-6.1-sol");
    assert_eq!(body["reasoning"]["effort"], "low");
    assert_eq!(receipt.budgets["model_source"], "cli");
    assert_eq!(receipt.budgets["reasoning_effort_source"], "cli");
    assert_eq!(receipt.budgets["requested_reasoning_effort"], "pi:minimal");
    assert_eq!(receipt.budgets["resolved_reasoning_effort"], "low");
    assert_eq!(inner.reasoning_effort().as_deref(), Some("max"));
}

#[test]
fn native_launch_preserves_explicit_compaction_and_disable_settings() {
    let _lock = crate::tests::env_lock();
    let club = route(
        catalog(Source::PiStore, Some(8192)),
        intent("gpt-6.1-sol", "high"),
    );
    let _off = crate::tests::TestEnvGuard::unset("ANGEL_NO_AUTOCOMPACT");
    assert_eq!(crate::agent::harness::compaction_budget(&club, 4000), 4000);
    let _sota = crate::tests::TestEnvGuard::set("ANGEL_SOTA_CONTEXT_BUDGET", "4000");
    let _soft = crate::tests::TestEnvGuard::set("ANGEL_CONTEXT_SOFT_CAP", "5000");
    assert_eq!(crate::agent::harness::compaction_budget(&club, 0), 4000);
    let _off = crate::tests::TestEnvGuard::set("ANGEL_NO_AUTOCOMPACT", "1");
    assert_eq!(crate::agent::harness::compaction_budget(&club, 4000), 0);
}

#[test]
fn catalog_r4_both_metadata_interfaces_drive_real_budget_for_both_sources() {
    for source in [Source::PiStore, Source::Legacy] {
        let club = route(catalog(source, Some(8192)), intent("gpt-6.1-sol", "high"));
        assert_eq!(club.metadata().unwrap(), club.metadata_cached().unwrap());
        let metadata = club.metadata().unwrap();
        assert_eq!(metadata.context_window, 8192);
        assert!(metadata.supports_cache && metadata.supports_tools);
        assert_eq!(metadata.supports_reasoning, Some(true));
        assert_eq!(
            crate::agent::harness::compaction_budget(&club, 0),
            8192 - 8192 / 5
        );
        assert_eq!(
            club.route_metadata().output_budget,
            crate::agent::club::OutputBudgetPolicy::EndpointManaged
        );
        assert!(
            club.build_request(&[ChatMsg::user("test")], &[])
                .get("max_output_tokens")
                .is_none()
        );
    }
    let club = route(catalog(Source::Legacy, None), intent("gpt-6.1-sol", "high"));
    assert!(club.metadata().is_none());
    assert_eq!(crate::agent::harness::compaction_budget(&club, 0), 120000);
    let tiny = route(
        catalog(Source::Legacy, Some(512)),
        intent("gpt-6.1-sol", "high"),
    );
    assert!(
        tiny.capture_selection(None)
            .unwrap_err()
            .contains("capacity")
    );
}
#[test]
fn catalog_r4_actual_tool_consumers_use_known_window_keep_research_and_handle_large_capacity() {
    struct FixtureTool(&'static str);
    impl crate::agent::harness::Tool for FixtureTool {
        fn name(&self) -> &str {
            self.0
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.0.into(),
                description: "schema ".repeat(3000),
                params: serde_json::json!({"type":"object"}),
            }
        }
        fn call(&self, _: &serde_json::Value) -> Result<String, String> {
            Ok("fixture".into())
        }
    }
    let mut registry = crate::agent::harness::ToolRegistry::new();
    let research = [
        "web_search",
        "web_fetch",
        "science_search",
        "repo_search",
        "defs",
        "find_files",
        "outline",
        "gpu_stat",
    ];
    for name in research
        .into_iter()
        .chain(["read_file", "unrelated_large_tool"])
    {
        registry.register(Box::new(FixtureTool(name)));
    }
    let club = route(
        catalog(Source::PiStore, Some(8192)),
        intent("gpt-6.1-sol", "high"),
    );
    let window = club.metadata().map(|m| m.context_window);
    let full = registry.defs_for_run(None, false);
    let small = registry.defs_for_run(window, false);
    assert!(small.len() < full.len());
    let competition = registry.defs_for_driver_turn(window, true, true, true);
    for name in research {
        assert!(
            competition.iter().any(|d| d.name == name),
            "research tool {name} hidden"
        );
    }
    let text = "x".repeat(60000);
    assert_eq!(
        crate::agent::harness::cap_tool_output(&text, None).len(),
        60000
    );
    assert!(crate::agent::harness::cap_tool_output(&text, window).len() < 15000);
    if usize::BITS == 64 {
        let large = route(
            catalog(Source::PiStore, Some(u64::MAX)),
            intent("gpt-6.1-sol", "high"),
        );
        let window = large.metadata().map(|m| m.context_window);
        assert_eq!(registry.defs_for_run(window, false).len(), full.len());
        assert_eq!(
            crate::agent::harness::cap_tool_output(&text, window).len(),
            text.len()
        );
    }
}
#[test]
fn catalog_r2_vision_uses_same_catalog_shared_auth_and_independent_effort() {
    let run = crate::agent::codex_startup::CodexStartup {
        catalog: catalog(Source::PiStore, Some(8192)),
        selection: intent("gpt-6.1-sol", "pi:high"),
        shared: Some(CodexClub::shared_state(ChatGptAuth::detached())),
    };
    let vision = run.vision_codex().unwrap();
    assert!(Arc::ptr_eq(&vision.shared, run.shared.as_ref().unwrap()));
    assert!(Arc::ptr_eq(vision.catalog.as_ref().unwrap(), &run.catalog));
    assert_eq!(vision.metadata_cached().unwrap().context_window, 8192);
    assert_eq!(
        vision.resolved_model_defaults()["catalog"],
        run.catalog.status()
    );
    assert_eq!(vision.resolved_model_defaults()["reasoning_effort"], "high");
    vision.set_reasoning_effort("low").unwrap();
    assert_eq!(run.selection.effort, "pi:high");
    // Explicit vision and Kimi precedence are checked by the existing vision fixtures.
}
#[test]
fn catalog_r2_startup_publication_shared_by_bag_registry_vision_and_next_run() {
    use crate::agent::tools::vision::VisionBackend;
    use crate::tests::TestEnvGuard;
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("catalog-startup-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("models-store.json");
    std::fs::write(&path, serde_json::json!({"openai-codex":{"models":[{
        "id":"gpt-6.1-sol", "name":"Sol", "provider":"openai-codex", "api":"openai-codex-responses",
        "contextWindow":8192,"maxTokens":4096,"input":["text","image"],"reasoning":true,
        "thinkingLevelMap":{"xhigh":"xhigh","max":"max"}
    }]}}).to_string()).unwrap();
    let _home = TestEnvGuard::set("CODEX_HOME", root.to_str().unwrap());
    let _source = TestEnvGuard::set(
        "ANGEL_OPENAI_CATALOG_SOURCE",
        &format!("pi-store:{}", path.display()),
    );
    let _model = TestEnvGuard::set("ANGEL_OPENAI_MODEL", "gpt-6.1-sol");
    let _effort = TestEnvGuard::set("ANGEL_OPENAI_REASONING_EFFORT", "pi:high");
    let _auth = TestEnvGuard::set(
        "ANGEL_OPENAI_AUTH_JSON",
        r#"{"tokens":{"access_token":"fixture-only"}}"#,
    );
    let _unset = [
        "ANGEL_VISION_URL",
        "ANGEL_VISION_MODEL",
        "ANGEL_KIMI_URL",
        "ANGEL_KIMI_KEY",
    ]
    .map(TestEnvGuard::unset);
    let run = crate::agent::codex_startup::CodexStartup::load();
    assert!(run.selection.error.is_none());
    std::fs::write(&path, b"bad replacement").unwrap();
    let bag = crate::agent::club::Bag::standard_in_run(Arc::clone(&run));
    let club = bag
        .agents
        .iter()
        .find(|a| a.name == "openai")
        .unwrap()
        .slots[0]
        .club
        .clone();
    let backend = VisionBackend::new(Some(&run));
    assert!(backend.configured());
    assert_eq!(
        backend.0.as_ref().unwrap().resolved_model_defaults()["catalog"],
        run.catalog.status()
    );
    let registry = crate::agent::harness::ToolRegistry::with_team_self_in_run(
        root.clone(),
        vec![club.clone()],
        Some(club.clone()),
        Some(&run),
    );
    assert_eq!(
        registry.vision.0.as_ref().unwrap().metadata_cached(),
        club.metadata_cached()
    );
    assert!(
        registry
            .defs_for_run(None, false)
            .iter()
            .any(|d| d.name == "vision_look")
    );
    let _kimi_url = TestEnvGuard::set("ANGEL_KIMI_URL", "http://127.0.0.1:9/v1");
    let _kimi_key = TestEnvGuard::set("ANGEL_KIMI_KEY", "fixture");
    assert_eq!(
        VisionBackend::new(Some(&run)).0.unwrap().label(),
        "kimi-vision"
    );
    let _vision_url = TestEnvGuard::set("ANGEL_VISION_URL", "http://127.0.0.1:9/v1");
    let _vision_model = TestEnvGuard::set("ANGEL_VISION_MODEL", "explicit-fixture");
    assert_eq!(VisionBackend::new(Some(&run)).0.unwrap().label(), "vision");
    let next = crate::agent::codex_startup::CodexStartup::load();
    assert!(next.catalog.checked.is_err());
    assert!(next.selection.error.is_some());
    drop((_vision_url, _vision_model, _kimi_url, _kimi_key));
    assert!(!VisionBackend::new(Some(&next)).configured());
    assert!(backend.configured());
    assert_eq!(
        club.resolved_model_defaults()["catalog"],
        run.catalog.status()
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn catalog_r2_bag_alias_identity_and_picker_revision_preserved() {
    let run = Arc::new(crate::agent::codex_startup::CodexStartup {
        catalog: catalog(Source::PiStore, Some(8192)),
        selection: intent("gpt-6.1-sol", "high"),
        shared: Some(CodexClub::shared_state(ChatGptAuth::detached())),
    });
    let bag = crate::agent::club::Bag::standard_in_run(Arc::clone(&run));
    assert!(Arc::ptr_eq(bag.codex_startup.as_ref().unwrap(), &run));
    let openai = bag
        .agents
        .iter()
        .find(|a| a.name == "openai")
        .unwrap()
        .slots[0]
        .club
        .clone();
    let alias = bag
        .agents
        .iter()
        .flat_map(|a| &a.slots)
        .find(|s| s.label == "codex-run")
        .unwrap();
    assert!(Arc::ptr_eq(&openai, &alias.club));
    let choices = bag.route_choices();
    assert_eq!(
        choices.iter().filter(|c| c.model == "gpt-6.1-sol").count(),
        1
    );
    let before = openai.route_state_revision();
    openai.set_reasoning_effort("low").unwrap();
    let after = bag.route_choices();
    assert!(!Arc::ptr_eq(&choices, &after));
    assert_eq!(
        after
            .iter()
            .find(|c| c.model == "gpt-6.1-sol")
            .unwrap()
            .reasoning_effort
            .as_deref(),
        Some("low")
    );
    assert!(alias.club.route_state_revision() > before);
    assert_eq!(
        alias.club.resolved_model_defaults()["reasoning_effort"],
        "low"
    );
}
