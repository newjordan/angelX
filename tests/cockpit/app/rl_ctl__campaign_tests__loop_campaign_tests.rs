use super::*;
use crate::agent::harness::ToolRegistry;
use serde_json::Value;

fn context(club: Arc<dyn Club>) -> LoopCampaignContext {
    LoopCampaignContext {
        loop_id: "loop-fixture".into(),
        task: "write the artifact this objective asks for".into(),
        verify: Some(VERIFY.into()),
        club,
        deadline: None,
        remaining_tokens: None,
    }
}

struct LoopFixtureClub {
    campaign: Arc<ObjectiveFixtureClub>,
    root_calls: AtomicUsize,
}

impl Club for LoopFixtureClub {
    fn label(&self) -> &str {
        "rl-loop-fixture"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Ok(NOTE.into())
    }
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if !tools.iter().any(|tool| tool.name == "rl_campaign") {
            return self.campaign.chat_streaming(messages, tools, cancel, delta);
        }
        let hop = self.root_calls.fetch_add(1, Ordering::AcqRel);
        if hop == 0 {
            return Ok(ClubReply::Calls(vec![crate::agent::club::ToolCall {
                id: "launch-rl".into(),
                name: "rl_campaign".into(),
                args: json!({"action":"run", "rounds":1, "group":2, "samples":2, "verifier_scope":[SCOPE]}),
            }]));
        }
        let receipt = messages
            .iter()
            .find(|m| m.tool_call_id.as_deref() == Some("launch-rl"))
            .expect("native tool result reaches model");
        assert!(
            receipt.content.contains("campaign started"),
            "{}",
            receipt.content
        );
        Ok(ClubReply::Text(
            "Campaign launched; continue useful work while it runs.".into(),
        ))
    }
}

#[test]
fn loop_native_campaign_is_always_advertised_and_runs_through_the_real_tool_turn() {
    // env-lock-exempt: FixtureEnv owns env_lock until its restoration guards drop.
    let env = FixtureEnv::new("loop-native");
    let workspace = env.workspace();
    let _bubble = TestEnvGuard::set("ANGEL_TOOL_BUBBLE", "0");
    let _activation = TestEnvGuard::set("ANGEL_TOOL_SEARCH_ACTIVE_MAX", "0");
    let _advisor = TestEnvGuard::set("ANGEL_ADVISOR", "0");
    let _skill = TestEnvGuard::set("ANGEL_SKILL_HINT", "0");
    let club = Arc::new(LoopFixtureClub {
        campaign: ObjectiveFixtureClub::new(NOTE),
        root_calls: AtomicUsize::new(0),
    });
    let mut registry =
        ToolRegistry::with_team_self(workspace.path().into(), vec![], Some(club.clone()));
    registry.enable_tool_search();
    assert!(
        !registry
            .defs_for_driver_turn(Some(200_000), false, false, true)
            .iter()
            .any(|t| t.name == "rl_campaign")
    );
    registry.rl().bind_loop(context(club.clone()));
    for profile in ["auto", "essential", "full"] {
        let _profile = TestEnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", profile);
        for metered in [false, true] {
            for window in [Some(512), Some(245_000)] {
                let defs = registry.defs_for_driver_turn(window, true, true, metered);
                assert_eq!(
                    defs.iter().filter(|t| t.name == "rl_campaign").count(),
                    1,
                    "{profile} {metered} {window:?}"
                );
                for name in [
                    "loop_research",
                    "consult_model",
                    "spawn",
                    "continual_harness",
                ] {
                    assert!(
                        defs.iter().any(|tool| tool.name == name),
                        "loop research entry {name} missing in {profile}"
                    );
                }
            }
        }
    }
    assert_eq!(
        club.root_calls.load(Ordering::Acquire),
        0,
        "availability spends no model requests"
    );
    let (events, _received) = mpsc::channel();
    let mut history = vec![ChatMsg::user(
        "Explore improvements for the current loop objective",
    )];
    let reply = crate::agent::harness::run_turn(
        club.as_ref(),
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();
    assert!(reply.contains("Campaign launched"));
    assert!(wait_until(
        || !registry.rl().running(),
        Duration::from_secs(90)
    ));
    let outcome = registry.rl().progress_snapshot().outcome.unwrap().unwrap();
    assert!(outcome.attempted > 0);
    assert!(
        outcome.passed > 0 && outcome.red > 0,
        "physical verifier reports both sides: {outcome:?}"
    );
    assert!(!outcome.validated);
    assert!(
        registry.rl().take_loop_tokens() > 0,
        "campaign work reaches loop spend accounting"
    );
    assert!(
        !workspace.path().join("result.txt").exists(),
        "campaign candidates remain isolated"
    );
    let run_id = Path::new(&outcome.report_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let retained: serde_json::Value = serde_json::from_str(
        &registry
            .dispatch("rl_campaign", &json!({"action":"results", "run_id":run_id}))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(retained["campaigns"][0]["status"], "completed");
    assert_eq!(
        retained["campaigns"][0]["outcome"]["Ok"]["attempted"],
        outcome.attempted
    );
    assert!(
        registry
            .dispatch(
                "rl_campaign",
                &json!({"action":"results", "run_id":"../escape"})
            )
            .is_err()
    );

    // Fresh registry + fresh loop context, as after restart/resume: no in-memory
    // campaign state is needed to recover the result into the next iteration.
    let mut app = crate::seed_preview_app();
    app.tools = Arc::new(ToolRegistry::with_team_self(
        workspace.path().into(),
        vec![],
        Some(club.clone()),
    ));
    app.loop_ctl.id = "loop-fixture".into();
    app.loop_ctl.task = context(club.clone()).task;
    app.loop_ctl.status = crate::drive::loop_ctl::LoopStatus::Running;
    app.tools.rl().bind_loop(context(club.clone()));
    let convo = app.loop_iteration_convo();
    assert!(convo.iter().any(|m| m.role == ChatRole::Harness
        && m.content.contains(run_id)
        && m.content.contains("completed")));
    assert!(
        !app.tools.rl().running(),
        "reading durable history does not relaunch"
    );
    app.tools.rl().bind_loop(LoopCampaignContext {
        loop_id: "different-loop".into(),
        ..context(club)
    });
    assert!(
        !app.tools
            .rl()
            .loop_context_text(workspace.path())
            .contains(run_id),
        "history cannot leak across loops"
    );
    let other = env.workspace();
    assert!(
        registry
            .rl()
            .tool_call(other.path(), &json!({"action":"results", "run_id":run_id}))
            .is_err(),
        "run lookup is workspace-scoped"
    );
}

#[test]
fn loop_pause_cancels_a_live_campaign_and_resume_restores_availability() {
    // env-lock-exempt: FixtureEnv owns env_lock until its restoration guards drop.
    let env = FixtureEnv::new("loop-pause");
    let workspace = env.workspace();
    let _file = TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        env.root.join("loop.json").to_str().unwrap(),
    );
    let (entered, waiting) = mpsc::channel();
    let club: Arc<dyn Club> = Arc::new(BlockingFixtureClub {
        entered: Mutex::new(Some(entered)),
    });
    let mut app = crate::seed_preview_app();
    app.tools = Arc::new(ToolRegistry::with_team_self(
        workspace.path().into(),
        vec![],
        Some(club.clone()),
    ));
    app.loop_ctl.id = "loop-fixture".into();
    app.loop_ctl.status = crate::drive::loop_ctl::LoopStatus::Running;
    app.loop_ctl.task = "write the artifact".into();
    app.loop_ctl.workspace = Some(workspace.path().into());
    app.tools.rl().bind_loop(context(club));
    app.tools
        .dispatch(
            "rl_campaign",
            &json!({"action":"run", "group":1,"samples":2}),
        )
        .unwrap();
    waiting
        .recv_timeout(Duration::from_secs(20))
        .expect("real model attempt is in flight");
    let run_dir = app.tools.rl().run_dir().unwrap().to_path_buf();
    let message = app.loop_command(Some("pause".into()));
    assert!(message.contains("paused"));
    assert!(!app.tools.rl().loop_enabled());
    assert!(wait_until(
        || !app.tools.rl().running(),
        Duration::from_secs(20)
    ));
    assert!(matches!(
        app.tools.rl().progress_snapshot().outcome,
        Some(Err(_))
    ));
    let record: Value =
        serde_json::from_slice(&std::fs::read(run_dir.join("campaign.json")).unwrap()).unwrap();
    assert!(
        record["outcome"]["Err"].is_string(),
        "cancellation is durable: {record}"
    );
    let resume = app.loop_command(Some("resume".into()));
    assert!(resume.contains("resumed"), "{resume}");
    assert!(app.tools.rl().loop_enabled());
    assert!(
        app.tools
            .defs_for_driver_turn(None, true, true, true)
            .iter()
            .any(|t| t.name == "rl_campaign")
    );
    app.loop_command(Some("stop".into()));
    assert!(!app.tools.rl().loop_enabled());
}

#[test]
fn explicit_loop_bounds_stop_campaign_work_and_retain_the_reason() {
    let env = FixtureEnv::new("loop-budget");
    let workspace = env.workspace();
    for token_budget in [true, false] {
        let (entered, _waiting) = mpsc::channel();
        let club: Arc<dyn Club> = Arc::new(BlockingFixtureClub {
            entered: Mutex::new(Some(entered)),
        });
        let mut binding = context(club);
        if token_budget {
            binding.remaining_tokens = Some(1);
        } else {
            binding.deadline = Some(Instant::now() + Duration::from_millis(300));
        }
        let mut state = RlState::default();
        state.bind_loop(binding);
        state
            .tool_call(
                workspace.path(),
                &json!({"action":"run", "group":1,"samples":2}),
            )
            .unwrap();
        let error = run_to_completion(&state, Duration::from_secs(20)).unwrap_err();
        assert!(
            error.contains(if token_budget {
                "token budget"
            } else {
                "deadline"
            }),
            "{error}"
        );
        assert!(state.loop_enabled(), "a bound does not hide the tool");
    }
}

#[test]
fn audited_native_tool_campaign_feeds_its_policy_into_the_next_loop_context() {
    let env = FixtureEnv::new("loop-audited");
    let workspace = env.workspace();
    let audit = audit_workspace();
    let club = ObjectiveFixtureClub::new(NOTE);
    let mut app = crate::seed_preview_app();
    app.tools = Arc::new(ToolRegistry::with_team_self(
        workspace.path().into(),
        vec![],
        Some(club.clone()),
    ));
    app.tools.rl().bind_loop(context(club));
    app.tools.dispatch("rl_campaign", &json!({"action":"run", "rounds":1, "group":4, "samples":4,
        "verifier_scope":[SCOPE], "audit":[{"task":"independent review objective", "verify":"sh tests/audit.sh", "source":audit.path()}]
    })).unwrap();
    assert!(wait_until(
        || !app.tools.rl().running(),
        Duration::from_secs(90)
    ));
    let outcome = app.tools.rl().progress_snapshot().outcome.unwrap().unwrap();
    assert!(outcome.validated, "{outcome:?}");
    assert!(outcome.accepted_entry.is_some() && outcome.accepted_event.is_some());
    let convo = app.loop_iteration_convo();
    assert!(
        convo
            .iter()
            .any(|m| m.role == ChatRole::Harness && m.content.contains(NOTE)),
        "fresh context consumes the measured policy"
    );
}
