use super::*;
use crate::tests::TestEnvGuard;

// The production API effort cache is seed-once. A fixture changing its env must
// resync under env_lock, and clear it again after the env guards are restored.
struct ReasoningCacheReset;
impl Drop for ReasoningCacheReset {
    fn drop(&mut self) {
        crate::agent::club::resync_reasoning_effort_env_from_env();
    }
}

struct FixtureDir(PathBuf);
impl FixtureDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-gate1-launch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("store.json"), serde_json::json!({"openai-codex":{"models":[{
            "id":"gate1-model","name":"Fixture","provider":"openai-codex","api":"openai-codex-responses",
            "contextWindow":8192,"maxTokens":4096,"input":["text","image"],"reasoning":true,
            "thinkingLevelMap":{"minimal":"low","xhigh":"xhigh","max":"max"}
        }]}}).to_string()).unwrap();
        Self(path)
    }
    fn environment(&self) -> Vec<TestEnvGuard> {
        vec![
            TestEnvGuard::set("HOME", self.0.to_str().unwrap()),
            TestEnvGuard::set("CODEX_HOME", self.0.to_str().unwrap()),
            TestEnvGuard::set(
                "ANGEL_OPENAI_CATALOG_SOURCE",
                &format!("pi-store:{}", self.0.join("store.json").display()),
            ),
            TestEnvGuard::set(
                "ANGEL_OPENAI_AUTH_JSON",
                r#"{"tokens":{"access_token":"fixture-only"}}"#,
            ),
            TestEnvGuard::set("ANGEL_DRIVER", "openai"),
            TestEnvGuard::set("ANGEL_OPENAI_MODEL", "gate1-model"),
            TestEnvGuard::set("ANGEL_OPENAI_REASONING_EFFORT", "unsupported-ambient"),
            TestEnvGuard::set("ANGEL_REASONING_EFFORT", "unsupported-generic"),
            TestEnvGuard::set("ANGEL_API_CLUBS", "none"),
            TestEnvGuard::set("ANGEL_PROBE", "0"),
            TestEnvGuard::set("ANGEL_BAG_PROBE", "0"),
            TestEnvGuard::set("ANGEL_SCAN", "0"),
            TestEnvGuard::set("ANGEL_TAILNET_RESOLVE", "0"),
            TestEnvGuard::set("ANGEL_FALLBACK", "practice"),
        ]
    }
}
impl Drop for FixtureDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn gate1_cli_effort_corrects_env_selected_subscription_without_driver_and_captures_intent() {
    let _lock = crate::tests::env_lock();
    let root = FixtureDir::new();
    let _env = root.environment();
    let mut launch = InteractiveLaunch {
        prompt: Some("literal".into()),
        ..Default::default()
    };
    assert!(
        prepare(&launch, &root.0).is_err(),
        "ambient effort is initially invalid"
    );
    launch.effort = Some("pi:minimal".into());
    let (bag, bound) = prepare(&launch, &root.0).unwrap();
    assert_eq!(bag.in_hand_route_identity().driver, "openai");
    assert!(bound.cli_effort);
    assert_eq!(
        bound.club.route_identity().model.as_deref(),
        Some("gate1-model")
    );
    assert_eq!(
        bound.club.route_identity().reasoning_effort.as_deref(),
        Some("low")
    );
    let defaults = bound.club.resolved_model_defaults();
    assert!(defaults["selection_error"].is_null());
    assert_eq!(defaults["requested_reasoning_effort"], "pi:minimal");
    assert_eq!(defaults["resolved_reasoning_effort"], "low");
    assert_eq!(defaults["reasoning_effort_source"], "cli");
    // The immutable launch capture remains independent of later session THINK.
    bag.set_reasoning_effort("high").unwrap();
    assert_eq!(bound.club.reasoning_effort().as_deref(), Some("low"));
}

#[test]
fn gate1_given_pi_model_without_effort_when_startup_then_preserves_native_fallback() {
    let _lock = crate::tests::env_lock();
    let root = FixtureDir::new();
    let _env = root.environment();
    // Given a checked Pi model with no producer default or ambient effort.
    let _unset =
        ["ANGEL_OPENAI_REASONING_EFFORT", "ANGEL_REASONING_EFFORT"].map(TestEnvGuard::unset);
    let launch = InteractiveLaunch {
        driver: Some("openai".into()),
        model: Some("gate1-model".into()),
        draft: Some("literal draft".into()),
        ..Default::default()
    };
    // When explicit model intent is admitted, no empty default is invented.
    let run = crate::agent::codex_startup::CodexStartup::load_for_launch(&launch)
        .expect("omitted effort retains supported native fallback");
    assert!(run.catalog.models()[0].default_reasoning_level.is_empty());
    assert_eq!(run.selection.model, "gate1-model");
    assert_eq!(run.selection.model_source, "cli");
    assert_eq!(
        run.selection.effort,
        crate::agent::openai_codex::OPENAI_LUNA_EFFORT
    );
    assert_eq!(run.selection.effort_source, "fallback");
    assert!(run.selection.error.is_none());
    // Then explicit invalid effort still fails; absence is not fallback permission.
    let invalid = InteractiveLaunch {
        driver: Some("openai".into()),
        model: Some("gate1-model".into()),
        effort: Some("invented".into()),
        ..Default::default()
    };
    assert!(crate::agent::codex_startup::CodexStartup::load_for_launch(&invalid).is_err());
    // Nor may an unsupported fallback silently clamp to another supported level.
    let path = root.0.join("store.json");
    let mut store: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    store["openai-codex"]["models"][0]["thinkingLevelMap"]
        [crate::agent::openai_codex::OPENAI_LUNA_EFFORT] = serde_json::Value::Null;
    std::fs::write(&path, store.to_string()).unwrap();
    assert!(crate::agent::codex_startup::CodexStartup::load_for_launch(&launch).is_err());
}

#[test]
fn gate1_cli_effort_corrects_explicit_api_responses_before_final_selection_check() {
    let _lock = crate::tests::env_lock();
    let _cache_reset = ReasoningCacheReset;
    let root = FixtureDir::new();
    let _env = root.environment();
    // ANGEL_API_CLUBS selects provider families; the concrete driver is openai-api.
    let _api = TestEnvGuard::set("ANGEL_API_CLUBS", "openai");
    let _key = TestEnvGuard::set("ANGEL_OPENAI_KEY", "fixture-only");
    let _url = TestEnvGuard::set("ANGEL_OPENAI_API_URL", "http://127.0.0.1:9/v1");
    let _transport = TestEnvGuard::set("ANGEL_OPENAI_API_TRANSPORT", "responses");
    let _effort = TestEnvGuard::set("ANGEL_OPENAI_API_REASONING_EFFORT", "unsupported-ambient");
    crate::agent::club::resync_reasoning_effort_env_from_env();
    let mut launch = InteractiveLaunch {
        driver: Some("openai-api".into()),
        model: Some("gpt-6-astra".into()),
        ..Default::default()
    };
    assert!(prepare(&launch, &root.0).is_err());
    launch.effort = Some("low".into());
    let (bag, bound) = prepare(&launch, &root.0).unwrap();
    assert_eq!(bag.in_hand_route_identity().driver, "openai-api");
    assert_eq!(
        bound.club.route_identity().model.as_deref(),
        Some("gpt-6-astra")
    );
    assert_eq!(bound.club.reasoning_effort().as_deref(), Some("low"));
    let defaults = bound.club.resolved_model_defaults();
    assert!(defaults["selection_error"].is_null());
    assert_eq!(defaults["model_source"], "cli");
    assert_eq!(defaults["requested_reasoning_effort"], "low");
    assert_eq!(defaults["resolved_reasoning_effort"], "low");
    assert_eq!(defaults["reasoning_effort_source"], "cli");
    let _bad_transport = TestEnvGuard::set("ANGEL_OPENAI_API_TRANSPORT", "invented");
    assert!(
        prepare(&launch, &root.0).is_err(),
        "CLI effort cannot bypass transport failure"
    );
}

#[test]
fn gate1_cli_effort_still_rejects_unknown_subscription_model_and_source_failure() {
    let _lock = crate::tests::env_lock();
    let root = FixtureDir::new();
    let _env = root.environment();
    let launch = InteractiveLaunch {
        effort: Some("low".into()),
        ..Default::default()
    };
    {
        let _unknown = TestEnvGuard::set("ANGEL_OPENAI_MODEL", "unknown-model");
        assert!(
            prepare(&launch, &root.0).is_err(),
            "no subscription fallback after a supported override"
        );
    }
    std::fs::write(root.0.join("store.json"), b"malformed").unwrap();
    assert!(
        prepare(&launch, &root.0).is_err(),
        "no source fallback after a supported override"
    );
}

fn app() -> App {
    let (_, rx) = std::sync::mpsc::channel();
    App::from_parts(
        Bag::for_render_test(&[("practice", &[("practice", true)])]),
        crate::ui::viewer::Viewer::static_preview(),
        Vec::new(),
        Arc::new(crate::agent::harness::ToolRegistry::new()),
        rx,
        crate::knowledge::session::Session::disabled(),
        Overwatch::disabled(),
    )
}
fn escape(app: &mut App) {
    app.on_key(ratatui::crossterm::event::KeyEvent::new(
        ratatui::crossterm::event::KeyCode::Esc,
        ratatui::crossterm::event::KeyModifiers::NONE,
    ));
}

#[test]
fn gate1_escape_after_launch_enqueue_before_dispatch_preserves_two_exact_drafts() {
    let _lock = crate::tests::env_lock();
    for echo_drawn in [false, true] {
        let mut app = app();
        let binding = BoundRoute {
            club: app.bag.in_hand(),
            route: app.bag.in_hand_route_identity(),
            indices: app.bag.selected_route_indices(),
            workspace: app.tools.current_workspace().to_path_buf(),
            cli_effort: false,
        };
        let prompt = "  /quit\nα  ";
        let ahead = " independent\nβ ";
        app.install_launch_prompt(prompt.into(), binding);
        app.input = ahead.into();
        app.advance_launch_input();
        assert_eq!(
            app.launch_input.as_ref().unwrap().state,
            LaunchState::Enqueued
        );
        app.pending_turn.as_mut().unwrap().echo_drawn = echo_drawn;
        assert!(app.thinking.is_none());
        escape(&mut app);
        assert!(app.pending_turn.is_none());
        assert!(app.thinking.is_none());
        assert!(app.history.is_empty());
        assert!(app.messages.iter().all(|m| !matches!(m.role, Role::User)));
        assert_eq!(app.input, prompt);
        assert_eq!(app.cursor, prompt.chars().count());
        assert_eq!(app.launch_typed_ahead.as_deref(), Some(ahead));
        assert_eq!(
            app.launch_input.as_ref().unwrap().state,
            LaunchState::Cancelled
        );
        assert!(app.launch_route.is_none());
        // Explicitly attempt the deferred dispatch seam: no pending turn means
        // zero worker starts, no replay, and no provider/tool invocation.
        app.launch_pending_turn();
        app.advance_launch_input();
        assert!(app.thinking.is_none());
        assert!(app.pending_turn.is_none());
        assert!(app.history.is_empty());
        escape(&mut app);
        assert_eq!(app.input, ahead);
        assert_eq!(app.launch_typed_ahead.as_deref(), Some(prompt));
    }
}

#[test]
fn gate1_ordinary_parked_turn_cancel_keeps_existing_composer_behavior() {
    let mut app = app();
    let raw: Arc<str> = Arc::from("ordinary prompt");
    app.pending_turn = Some(PendingTurn {
        raw: raw.clone(),
        user_msg: ChatMsg::user(raw.as_ref()),
        turn_evidence: None,
        echo_drawn: false,
        retry_draft: raw,
        clipboard_images: 0,
        launch: None,
    });
    app.input = "ordinary typed ahead".into();
    assert!(app.interrupt());
    assert_eq!(app.input, "ordinary prompt");
    assert!(app.launch_typed_ahead.is_none());
    assert!(app.launch_input.is_none());
    assert!(app.thinking.is_none());
}
