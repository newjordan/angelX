use super::*;

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
fn binding(app: &App) -> BoundRoute {
    BoundRoute {
        club: app.bag.in_hand(),
        route: app.bag.in_hand_route_identity(),
        indices: app.bag.selected_route_indices(),
        workspace: app.tools.current_workspace().to_path_buf(),
        cli_effort: false,
    }
}
fn arm(app: &mut App, text: &str) {
    app.install_launch_prompt(text.into(), binding(app));
}

#[test]
fn finite_initial_state_transitions_are_terminal_and_consume_once() {
    let app = app();
    for terminal in [
        LaunchState::Enqueued,
        LaunchState::Cancelled,
        LaunchState::Failed,
    ] {
        let mut input = LaunchInput {
            state: LaunchState::Pending,
            text: Some("literal".into()),
            binding: binding(&app),
        };
        assert!(input.transition(LaunchState::Pending).is_none());
        assert_eq!(input.transition(terminal).as_deref(), Some("literal"));
        for next in [
            LaunchState::Pending,
            LaunchState::Enqueued,
            LaunchState::Cancelled,
            LaunchState::Failed,
        ] {
            assert!(input.transition(next).is_none());
            assert_eq!(input.state, terminal);
        }
    }
}

#[test]
fn literal_enqueue_is_once_intro_independent_and_typed_ahead_separate() {
    let _env = crate::tests::env_lock();
    let mut app = app();
    let literal = "  /yolo on\n--doctor α  ";
    arm(&mut app, literal);
    assert!(app.startup_intro.dismissed());
    app.input = "independent draft".into();
    app.submit();
    assert!(app.pending_turn.is_none());
    app.advance_launch_input();
    assert_eq!(app.input, "independent draft");
    let pending = app.pending_turn.as_ref().unwrap();
    assert_eq!(pending.raw.as_ref(), literal);
    assert_eq!(pending.user_msg.content.as_ref(), literal);
    assert!(pending.launch.is_some());
    assert!(app.thinking.is_none());
    assert!(app.history.is_empty());
    assert_eq!(
        app.messages
            .iter()
            .filter(|m| matches!(m.role, Role::User))
            .count(),
        1
    );
    arm(&mut app, "must not install twice");
    app.advance_launch_input();
    assert_eq!(
        app.messages
            .iter()
            .filter(|m| matches!(m.role, Role::User))
            .count(),
        1
    );
    assert_eq!(
        app.launch_input.as_ref().unwrap().state,
        LaunchState::Enqueued
    );
}

#[test]
fn escape_cancels_into_exact_draft_and_retains_typed_ahead_without_enqueue() {
    let mut app = app();
    arm(&mut app, "  /quit\nα  ");
    app.input = "typed ahead".into();
    app.on_key(ratatui::crossterm::event::KeyEvent::new(
        ratatui::crossterm::event::KeyCode::Esc,
        ratatui::crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(app.input, "  /quit\nα  ");
    assert_eq!(app.launch_typed_ahead.as_deref(), Some("typed ahead"));
    app.advance_launch_input();
    assert!(app.pending_turn.is_none());
    assert_eq!(
        app.launch_input.as_ref().unwrap().state,
        LaunchState::Cancelled
    );
    app.on_key(ratatui::crossterm::event::KeyEvent::new(
        ratatui::crossterm::event::KeyCode::Esc,
        ratatui::crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(app.input, "typed ahead");
    assert_eq!(app.launch_typed_ahead.as_deref(), Some("  /quit\nα  "));
}

#[test]
fn quit_cancels_without_turn_and_fresh_app_never_replays() {
    let mut a = app();
    arm(&mut a, "private initial");
    a.input = "/quit".into();
    a.submit();
    assert_eq!(
        a.launch_input.as_ref().unwrap().state,
        LaunchState::Cancelled
    );
    assert!(a.pending_turn.is_none());
    assert!(a.history.is_empty());
    assert!(!app().launch_is_pending());
}

#[test]
fn binding_changes_cancel_and_busy_readiness_defers_without_animation_dependency() {
    let mut app = app();
    arm(&mut app, "initial");
    app.launch_input.as_mut().unwrap().binding.workspace = PathBuf::from("/not-the-bound-root");
    app.advance_launch_input();
    assert_eq!(app.input, "initial");
    assert!(app.pending_turn.is_none());
    app = self::app();
    arm(&mut app, "second");
    let raw: Arc<str> = Arc::from("other pending turn");
    app.pending_turn = Some(PendingTurn {
        raw: raw.clone(),
        user_msg: ChatMsg::user(raw.as_ref()),
        turn_evidence: None,
        echo_drawn: false,
        retry_draft: raw,
        clipboard_images: 0,
        launch: None,
    });
    app.advance_launch_input();
    assert!(app.launch_is_pending());
    assert!(app.messages.iter().all(|m| !matches!(m.role, Role::User)));
    app.pending_turn = None;
    app.exit_request = Some(ExitRequest::WaitingForIdle);
    app.advance_launch_input();
    assert!(!app.launch_is_pending());
    assert!(app.pending_turn.is_none());
}

#[test]
fn route_interactions_cancel_before_change_and_unavailable_is_failed() {
    let mut app = app();
    arm(&mut app, "  initial  ");
    app.launch_interaction();
    assert_eq!(app.input, "  initial  ");
    assert!(app.launch_route.is_none());
    assert!(app.pending_turn.is_none());
    arm(&mut app, "must not re-arm cancelled input");
    assert!(!app.launch_is_pending());
    app = self::app();
    arm(&mut app, "again");
    app.launch_input.as_mut().unwrap().binding.club = Arc::new(Unavailable);
    app.advance_launch_input();
    assert_eq!(
        app.launch_input.as_ref().unwrap().state,
        LaunchState::Failed
    );
    assert!(app.pending_turn.is_none());
    assert!(app.history.is_empty());
}
#[test]
fn native_launch_cli_effort_beats_ultrathink_and_a_later_shared_preference() {
    let _env = crate::tests::env_lock();
    let mut app = app();
    let club = Arc::new(EffortClub {
        effort: std::sync::Mutex::new("low".into()),
        levels: vec!["low".into(), "high".into()],
    });
    app.bag.replace_in_hand_club_for_test(club.clone());
    let mut b = binding(&app);
    b.cli_effort = true;
    b.club = crate::agent::club::launch_effort(club.clone(), "low", "cli").unwrap();
    app.install_launch_prompt("literal ultrathink".into(), b);
    app.advance_launch_input();
    assert_eq!(app.bag.reasoning_effort().as_deref(), Some("low"));
    let bound = &app
        .pending_turn
        .as_ref()
        .unwrap()
        .launch
        .as_ref()
        .unwrap()
        .club;
    club.set_reasoning_effort("high").unwrap();
    assert_eq!(
        bound.route_identity().reasoning_effort.as_deref(),
        Some("low")
    );
    assert!(
        matches!(bound.chat_with_effort(&[], &[], Some("high")).unwrap(), crate::agent::club::ClubReply::Text(s) if s == "low:cli")
    );
}
struct EffortClub {
    effort: std::sync::Mutex<String>,
    levels: Vec<String>,
}
impl Club for EffortClub {
    fn label(&self) -> &str {
        "effort-fixture"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        panic!("uncaptured call")
    }
    fn reasoning_effort(&self) -> Option<String> {
        Some(self.effort.lock().unwrap().clone())
    }
    fn reasoning_levels(&self) -> &[String] {
        &self.levels
    }
    fn set_reasoning_effort(&self, value: &str) -> Option<String> {
        *self.effort.lock().unwrap() = value.into();
        Some(value.into())
    }
    fn chat_streaming_with_effort_source(
        &self,
        _: &[ChatMsg],
        _: &[crate::agent::club::ToolDef],
        effort: Option<&str>,
        source: &'static str,
        _: &std::sync::atomic::AtomicBool,
        _: &mut dyn FnMut(crate::agent::club::StreamDelta),
    ) -> Result<crate::agent::club::ClubReply, String> {
        Ok(crate::agent::club::ClubReply::Text(format!(
            "{}:{source}",
            effort.unwrap()
        )))
    }
}

#[test]
fn native_launch_exact_selection_beats_ambient_preferences_and_uses_one_run_catalog() {
    use crate::tests::TestEnvGuard;
    let _env = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("native-launch-exact-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("store.json");
    std::fs::write(&path, serde_json::json!({"openai-codex":{"models":[{
        "id":"gpt-6.1-sol","name":"Sol","provider":"openai-codex","api":"openai-codex-responses",
        "contextWindow":8192,"maxTokens":4096,"input":["text","image"],"reasoning":true,
        "thinkingLevelMap":{"minimal":"low","xhigh":"xhigh","max":"max"}
    }]}}).to_string()).unwrap();
    let _codex = TestEnvGuard::set("CODEX_HOME", root.to_str().unwrap());
    let _catalog = TestEnvGuard::set(
        "ANGEL_OPENAI_CATALOG_SOURCE",
        &format!("pi-store:{}", path.display()),
    );
    let _auth = TestEnvGuard::set(
        "ANGEL_OPENAI_AUTH_JSON",
        r#"{"tokens":{"access_token":"fixture-only"}}"#,
    );
    let _driver = TestEnvGuard::set("ANGEL_DRIVER", "practice");
    let _model = TestEnvGuard::set("ANGEL_OPENAI_MODEL", "unknown-ambient-model");
    let _effort = TestEnvGuard::set(
        "ANGEL_OPENAI_REASONING_EFFORT",
        "unsupported-ambient-effort",
    );
    let _fallback = TestEnvGuard::set("ANGEL_FALLBACK", "practice");
    let launch = InteractiveLaunch {
        driver: Some("openai".into()),
        model: Some("gpt-6.1-sol".into()),
        effort: Some("pi:minimal".into()),
        ..Default::default()
    };
    let (mut bag, bound) = prepare(&launch, &root).unwrap();
    assert_eq!(bag.in_hand_route_identity().driver, "openai");
    assert_eq!(
        bound.club.route_identity().model.as_deref(),
        Some("gpt-6.1-sol")
    );
    assert_eq!(
        bound.club.route_identity().reasoning_effort.as_deref(),
        Some("low")
    );
    let run = bag.codex_startup.as_ref().unwrap().clone();
    assert_eq!(run.selection.model_source, "cli");
    assert_eq!(run.selection.effort_source, "cli");
    std::fs::write(&path, b"invalid replacement").unwrap();
    assert_eq!(
        run.vision_codex().unwrap().reasoning_effort().as_deref(),
        Some("low")
    );
    assert!(
        bag.select_launch_route("openai-api", Some("gpt-6.1-sol"))
            .is_err()
    );
    assert_eq!(bag.in_hand_route_identity().driver, "openai");
    assert!(
        prepare(&launch, &root).is_err(),
        "next run must reject the invalid source, not fall back"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_launch_workspace_cwd_drift_cancels_before_enqueue() {
    let _env = crate::tests::env_lock();
    let original = std::env::current_dir().unwrap();
    let target = std::env::temp_dir().join(format!("native-cwd-drift-{}", std::process::id()));
    std::fs::create_dir(&target).unwrap();
    struct Restore(PathBuf, PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.0).unwrap();
            std::fs::remove_dir(&self.1).unwrap();
        }
    }
    let _restore = Restore(original, target.clone());
    let mut app = app();
    arm(&mut app, "  literal  ");
    std::env::set_current_dir(&target).unwrap();
    app.advance_launch_input();
    assert_eq!(app.input, "  literal  ");
    assert!(app.pending_turn.is_none());
    assert_eq!(
        app.launch_input.as_ref().unwrap().state,
        LaunchState::Cancelled
    );
}

#[test]
fn native_launch_unelected_practice_floor_is_not_operational_readiness() {
    use crate::tests::TestEnvGuard;
    let _env = crate::tests::env_lock();
    let _driver = TestEnvGuard::unset("ANGEL_DRIVER");
    let _auth = TestEnvGuard::unset("ANGEL_OPENAI_AUTH_JSON");
    let _api = TestEnvGuard::set("ANGEL_API_CLUBS", "none");
    let launch = InteractiveLaunch {
        prompt: Some("literal".into()),
        ..Default::default()
    };
    assert!(prepare(&launch, &std::env::current_dir().unwrap()).is_err());
}

struct Unavailable;
impl Club for Unavailable {
    fn label(&self) -> &str {
        "unavailable"
    }
    fn is_available(&self) -> bool {
        false
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        panic!("must never call")
    }
}

#[test]
fn exact_route_selection_rejects_fuzzy_names_and_models_without_fallback() {
    let mut bag = Bag::for_render_test(&[("practice", &[("practice", true)])]);
    assert!(bag.select_launch_route("practice", None).is_ok());
    for name in [
        "Practice",
        "prac",
        "openai",
        "openai-api",
        "openai/fixture",
        "auto",
    ] {
        assert!(bag.select_launch_route(name, None).is_err());
    }
    assert!(
        bag.select_launch_route("practice", Some("not-this-model"))
            .is_err()
    );
    assert_eq!(bag.in_hand_label(), "practice");
}
