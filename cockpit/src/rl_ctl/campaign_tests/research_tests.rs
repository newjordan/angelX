use super::*;
use crate::harness::ToolRegistry;
use serde_json::Value;

fn context(club: Arc<dyn Club>) -> LoopCampaignContext {
    LoopCampaignContext {
        loop_id: "sloptomizer-fixture".into(),
        task: "write the requested artifact".into(),
        verify: Some(VERIFY.into()),
        club,
        deadline: None,
        remaining_tokens: None,
    }
}
fn call(registry: &ToolRegistry, args: Value) -> Value {
    serde_json::from_str(&registry.dispatch("loop_research", &args).unwrap()).unwrap()
}
fn settled(registry: &ToolRegistry) -> Value {
    assert!(wait_until(
        || call(registry, json!({"action":"status"}))["running"] != true,
        Duration::from_secs(90)
    ));
    call(registry, json!({"action":"status"}))
}

/// Parent chooses the tool; baseline answers without changing files; candidate
/// sees the opted-in idea in task context and makes the actual tool edit.
struct ResearchClub {
    root_calls: AtomicUsize,
    attempts: AtomicUsize,
}
impl Club for ResearchClub {
    fn label(&self) -> &str {
        "sloptomizer-fixture"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Err("unused fixture entry".into())
    }
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        _: &AtomicBool,
        _: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if tools.iter().any(|t| t.name == "loop_research") {
            if self.root_calls.fetch_add(1, Ordering::AcqRel) == 0 {
                return Ok(ClubReply::Calls(vec![crate::club::ToolCall {
                    id: "research-launch".into(),
                    name: "loop_research".into(),
                    args: json!({"action":"run","idea":NOTE,"approach":"write-first","compare":true}),
                }]));
            }
            assert!(
                messages
                    .iter()
                    .any(|m| m.tool_call_id.as_deref() == Some("research-launch")
                        && m.content.contains("running"))
            );
            return Ok(ClubReply::Text("Continue useful main-loop work.".into()));
        }
        if messages.len() == 2 {
            self.attempts.fetch_add(1, Ordering::AcqRel);
            if messages.iter().any(|m| {
                m.role == ChatRole::User && m.content.contains(NOTE) && !m.content.contains(WORSE)
            }) {
                assert!(
                    !messages
                        .iter()
                        .any(|m| m.role == ChatRole::System && m.content.contains(NOTE)),
                    "research idea is task data, not system authority"
                );
                return Ok(ClubReply::Calls(vec![crate::club::ToolCall {
                    id: "write-candidate".into(),
                    name: "write_file".into(),
                    args: json!({"path":"result.txt","content":"done"}),
                }]));
            }
        }
        Ok(ClubReply::Text("Attempt complete.".into()))
    }
}

#[test]
fn sloptomizer_real_tool_turn_measures_pairs_learns_and_restores_without_parent_edits() {
    // env-lock-exempt: FixtureEnv owns env_lock until restoration guards drop.
    let env = FixtureEnv::new("slop-feedback");
    let workspace = env.workspace();
    let club = Arc::new(ResearchClub {
        root_calls: AtomicUsize::new(0),
        attempts: AtomicUsize::new(0),
    });
    let mut registry =
        ToolRegistry::with_team_self(workspace.path().into(), vec![], Some(club.clone()));
    registry.enable_tool_search();
    registry.rl().bind_loop(context(club.clone()));
    let cold = call(
        &registry,
        json!({"action":"suggest","candidates":[{"idea":NOTE,"approach":"write-first"}]}),
    );
    assert_eq!(cold["advice"]["observations"], 0);
    assert_eq!(club.attempts.load(Ordering::Acquire), 0);
    let (events, _rx) = mpsc::channel();
    let answer = crate::harness::run_turn(
        club.as_ref(),
        &registry,
        &mut vec![ChatMsg::user("Try a research approach")],
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();
    assert!(answer.contains("Continue useful"));
    let paired = settled(&registry);
    assert_eq!(paired["status"], "completed", "{paired}");
    assert!(paired["learning_error"].is_null(), "{paired}");
    assert_eq!(
        paired["measurements"],
        json!({"candidate_passed":true,"baseline_passed":false,"paired_delta":1})
    );
    assert_eq!(
        paired["baseline"]["snapshot_sha256"],
        paired["candidate"]["snapshot_sha256"]
    );
    assert_eq!(paired["route"], paired["candidate"]["resolved_route"]);
    assert!(
        paired["candidate"]["patch_path"]
            .as_str()
            .is_some_and(|p| Path::new(p).is_file())
    );
    assert!(!workspace.path().join("result.txt").exists());
    assert!(registry.rl().take_loop_tokens() > 0);
    assert_eq!(club.attempts.load(Ordering::Acquire), 2);
    let advice = call(&registry, json!({"action":"suggest"}));
    assert_eq!(advice["advice"]["observations"], 2);
    assert_eq!(advice["advice"]["pareto"]["idea"], NOTE);
    assert!(advice["advice"]["memory"].to_string().contains(NOTE));
    let ranking = advice["advice"]["memory_ranking"].as_array().unwrap();
    let scored = ranking.iter().find(|r| r["idea"] == NOTE).unwrap();
    assert!(
        scored["slow_score"] != cold["advice"]["memory_ranking"][0]["slow_score"],
        "slow learning must affect the next suggestion"
    );
    // The empty vocabulary starts with log likelihood zero. After learning,
    // compare positive and negative outcomes within the same learned model.
    assert!(
        scored["slow_score"].as_f64().unwrap()
            > ranking.iter().find(|r| r["idea"] != NOTE).unwrap()["slow_score"]
                .as_f64()
                .unwrap()
    );
    assert!(
        scored["mlp_score"] != cold["advice"]["memory_ranking"][0]["mlp_score"],
        "trained MLP must affect the next suggestion"
    );
    let bandit = advice["advice"]["bandit"].as_array().unwrap();
    assert!(
        bandit
            .iter()
            .any(|r| r["approach"] == "write-first" && r["successes"].as_f64() == Some(1.0))
    );
    assert!(
        bandit
            .iter()
            .any(|r| r["approach"] == "baseline" && r["successes"].as_f64() == Some(0.0))
    );

    call(
        &registry,
        json!({"action":"run","idea":WORSE,"approach":"no-edit","use_memory":false}),
    );
    let red = settled(&registry);
    assert_eq!(red["measurements"]["candidate_passed"], false, "{red}");
    assert!(red["measurements"]["paired_delta"].is_null());
    assert!(red["learning_error"].is_null(), "{red}");
    assert_eq!(
        call(&registry, json!({"action":"suggest"}))["advice"]["observations"],
        3
    );
    assert!(
        registry
            .rl()
            .loop_context_text(workspace.path())
            .contains(red["run_id"].as_str().unwrap())
    );

    let restored =
        ToolRegistry::with_team_self(workspace.path().into(), vec![], Some(club.clone()));
    restored.rl().bind_loop(context(club.clone()));
    let after = call(&restored, json!({"action":"suggest","methods":["memory"]}));
    assert_eq!(after["advice"]["observations"], 3);
    assert!(after["advice"]["pareto"].is_null());
    assert!(
        call(&restored, json!({"action":"results"}))["runs"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
    assert_eq!(
        club.attempts.load(Ordering::Acquire),
        3,
        "restart/query never relaunches work"
    );
    assert_eq!(
        call(
            &restored,
            json!({"action":"suggest","task":"a distinct objective"})
        )["advice"]["observations"],
        0
    );
    restored.rl().bind_loop(LoopCampaignContext {
        loop_id: "another-loop".into(),
        ..context(club)
    });
    assert!(
        !restored
            .rl()
            .loop_context_text(workspace.path())
            .contains(red["run_id"].as_str().unwrap())
    );
    assert!(
        restored
            .dispatch(
                "loop_research",
                &json!({"action":"results","run_id":"../escape"})
            )
            .is_err()
    );
    let other = env.workspace();
    assert!(
        restored
            .rl()
            .research_call(
                other.path(),
                &json!({"action":"results","run_id":red["run_id"]}),
                &AtomicBool::new(false)
            )
            .is_err()
    );
    let state = Path::new(after["state_path"].as_str().unwrap());
    std::fs::write(state, b"broken state").unwrap();
    assert!(
        restored
            .dispatch("loop_research", &json!({"action":"suggest"}))
            .unwrap_err()
            .contains("corrupt")
    );
    assert_eq!(std::fs::read(state).unwrap(), b"broken state");
}

#[test]
fn sloptomizer_unverified_attempt_retains_patch_without_learning_or_invented_delta() {
    // env-lock-exempt: FixtureEnv owns env_lock until restoration guards drop.
    let env = FixtureEnv::new("slop-unverified");
    let workspace = env.workspace();
    let club = Arc::new(ResearchClub {
        root_calls: AtomicUsize::new(0),
        attempts: AtomicUsize::new(0),
    });
    let registry =
        ToolRegistry::with_team_self(workspace.path().into(), vec![], Some(club.clone()));
    registry.rl().bind_loop(context(club));
    call(&registry, json!({"action":"run","idea":NOTE,"verify":null}));
    let result = settled(&registry);
    assert_eq!(result["status"], "completed", "{result}");
    assert!(result["measurements"]["candidate_passed"].is_null());
    assert_eq!(result["learning"], json!([]));
    assert!(result["candidate"]["patch_path"].as_str().is_some());
    assert_eq!(
        call(&registry, json!({"action":"suggest","verify":null}))["advice"]["observations"],
        0
    );
    assert!(!workspace.path().join("result.txt").exists());
}

#[test]
fn sloptomizer_stop_pause_replacement_and_drop_cancel_owned_attempts() {
    // env-lock-exempt: FixtureEnv owns env_lock until restoration guards drop.
    let env = FixtureEnv::new("slop-cancel");
    let workspace = env.workspace();
    for action in ["stop", "pause", "replace", "drop"] {
        let (entered, waiting) = mpsc::channel();
        let club = Arc::new(BlockingFixtureClub {
            entered: Mutex::new(Some(entered)),
        });
        let registry =
            ToolRegistry::with_team_self(workspace.path().into(), vec![], Some(club.clone()));
        registry.rl().bind_loop(context(club.clone()));
        let launched = call(
            &registry,
            json!({"action":"run","idea":"Wait for cancellation"}),
        );
        waiting.recv_timeout(Duration::from_secs(30)).unwrap();
        match action {
            "stop" => {
                assert_eq!(
                    call(&registry, json!({"action":"stop"}))["stop_requested"],
                    true
                );
            }
            "pause" => registry.rl().end_loop(),
            "replace" => registry.rl().bind_loop(LoopCampaignContext {
                loop_id: "replacement".into(),
                ..context(club.clone())
            }),
            _ => {}
        }
        if action != "drop" {
            let result = settled(&registry);
            assert_eq!(result["status"], "stopped", "{result}");
            assert!(result["learning"].is_null());
        }
        drop(registry);
        let record = Path::new(launched["artifacts"].as_str().unwrap()).join("research.json");
        assert!(
            wait_until(
                || std::fs::read(&record)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .is_some_and(|r| r["status"] == "stopped"),
                Duration::from_secs(30)
            ),
            "{action}"
        );
    }
}

#[test]
fn sloptomizer_runtime_errors_are_visible_without_spending_model_calls() {
    // env-lock-exempt: FixtureEnv owns env_lock until restoration guards drop.
    let env = FixtureEnv::new("slop-runtime");
    let workspace = env.workspace();
    let club = Arc::new(ResearchClub {
        root_calls: AtomicUsize::new(0),
        attempts: AtomicUsize::new(0),
    });
    let registry =
        ToolRegistry::with_team_self(workspace.path().into(), vec![], Some(club.clone()));
    registry.rl().bind_loop(context(club.clone()));
    let _python = TestEnvGuard::set(
        "ANGEL_RESEARCH_PYTHON",
        env.root.join("missing-python").to_str().unwrap(),
    );
    let error = registry
        .dispatch("loop_research", &json!({"action":"run","idea":NOTE}))
        .unwrap_err();
    assert!(error.contains("Python unavailable"), "{error}");
    assert_eq!(club.attempts.load(Ordering::Acquire), 0);
    assert_eq!(
        call(&registry, json!({"action":"status"}))["status"],
        "idle"
    );
}

struct LearningFailureClub {
    state: PathBuf,
    inner: ResearchClub,
}

struct FailedBaselineClub {
    candidate_calls: AtomicUsize,
}

impl Club for FailedBaselineClub {
    fn label(&self) -> &str {
        "sloptomizer-failed-baseline"
    }

    fn respond(&self, _: &str) -> Result<String, String> {
        Err("unused fixture entry".into())
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        _: &[ToolDef],
        _: &AtomicBool,
        _: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if messages
            .iter()
            .any(|message| message.content.contains(NOTE))
        {
            self.candidate_calls.fetch_add(1, Ordering::AcqRel);
        }
        Err("authentication failed: synthetic baseline fixture".into())
    }
}

#[test]
fn sloptomizer_failed_baseline_stops_before_candidate_spending_or_learning() {
    // env-lock-exempt: FixtureEnv owns env_lock until restoration guards drop.
    let env = FixtureEnv::new("slop-failed-baseline");
    let workspace = env.workspace();
    let club = Arc::new(FailedBaselineClub {
        candidate_calls: AtomicUsize::new(0),
    });
    let registry =
        ToolRegistry::with_team_self(workspace.path().into(), vec![], Some(club.clone()));
    registry.rl().bind_loop(context(club.clone()));
    call(
        &registry,
        json!({"action":"run","idea":NOTE,"compare":true}),
    );
    let result = settled(&registry);
    assert_eq!(result["status"], "failed", "{result}");
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("candidate was not started")
    );
    assert!(
        result["baseline"]["error"]
            .as_str()
            .unwrap()
            .contains("authentication failed")
    );
    assert!(result["candidate"].is_null());
    assert!(result["learning"].is_null());
    assert_eq!(club.candidate_calls.load(Ordering::Acquire), 0);
    assert_eq!(
        call(&registry, json!({"action":"suggest"}))["advice"]["observations"],
        0
    );
    let retained = call(
        &registry,
        json!({"action":"results","run_id":result["run_id"]}),
    );
    assert_eq!(retained["runs"][0]["status"], "failed");
    assert!(!workspace.path().join("result.txt").exists());
}
impl Club for LearningFailureClub {
    fn label(&self) -> &str {
        "sloptomizer-fixture"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Err("unused fixture entry".into())
    }
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        std::fs::write(&self.state, b"broken learning store during attempt").unwrap();
        self.inner.chat_streaming(messages, tools, cancel, delta)
    }
}

#[test]
fn sloptomizer_learning_failure_is_exposed_alongside_the_real_verifier_receipt() {
    // env-lock-exempt: FixtureEnv owns env_lock until restoration guards drop.
    let env = FixtureEnv::new("slop-learning-failure");
    let workspace = env.workspace();
    let club = Arc::new(ResearchClub {
        root_calls: AtomicUsize::new(0),
        attempts: AtomicUsize::new(0),
    });
    let registry =
        ToolRegistry::with_team_self(workspace.path().into(), vec![], Some(club.clone()));
    registry.rl().bind_loop(context(club));
    let cold = call(&registry, json!({"action":"suggest"}));
    let faulty = Arc::new(LearningFailureClub {
        state: PathBuf::from(cold["state_path"].as_str().unwrap()),
        inner: ResearchClub {
            root_calls: AtomicUsize::new(0),
            attempts: AtomicUsize::new(0),
        },
    });
    registry.rl().bind_loop(context(faulty));
    call(&registry, json!({"action":"run","idea":NOTE}));
    let result = settled(&registry);
    assert_eq!(result["measurements"]["candidate_passed"], true, "{result}");
    assert!(
        result["learning_error"]
            .as_str()
            .unwrap()
            .contains("corrupt")
    );
    assert_eq!(result["status"], "completed_with_learning_error");
    assert!(result["learning"].is_null());
    assert!(
        registry
            .rl()
            .loop_context_text(workspace.path())
            .contains("research state corrupt")
    );
    assert!(
        registry
            .dispatch("loop_research", &json!({"action":"observe","passed":true}))
            .is_err()
    );
    assert!(!workspace.path().join("result.txt").exists());
}
