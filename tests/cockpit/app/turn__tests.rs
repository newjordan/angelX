use super::*;

#[test]
fn progress_reports_label_and_bounded_progress() {
    let thinking = Thinking::pending_for_test("practice");
    let (progress, secs, label) = thinking.progress();
    assert_eq!(label, "practice");
    assert!(progress >= 0.0);
    assert!(progress < 1.0);
    assert!(secs >= 0.0);
}

#[test]
fn spawn_acquires_foreground_lease_inside_the_worker() {
    // Pin: acquire_scoped must not run on the UI thread in Thinking::spawn.
    // A contended leases mutex would otherwise hitch Enter-after-echo.
    // include_str is the non-flaky substitute for "spawn does not block on a
    // held lease" (acquire is fail-fast, so a held lease cannot stall spawn).
    let src = include_str!("../../../cockpit/src/turn.rs");
    let start = src
        .find("pub(crate) fn spawn(")
        .expect("Thinking::spawn present");
    let spawn = &src[start..];
    let end = spawn
        .find("\n    pub(crate) fn begin_draining")
        .expect("begin_draining follows spawn");
    let spawn = &spawn[..end];
    let worker = spawn
        .find(".spawn(move ||")
        .expect("worker closure in Thinking::spawn");
    let ui = &spawn[..worker];
    let worker = &spawn[worker..];
    assert!(
        !ui.contains("acquire_scoped") && !ui.contains("resource_group_for_identity"),
        "UI-thread spawn must not wait on a foreground lease:\n{ui}"
    );
    assert!(
        worker.contains("acquire_scoped"),
        "foreground lease must be acquired inside the worker:\n{worker}"
    );
    assert!(
        worker.contains("foreground route resource conflict"),
        "lease errors must still surface as the turn result:\n{worker}"
    );
    assert!(
        worker.contains("fold_vision_sidecar_into_convo"),
        "vision sidecar rewrite must run inside the worker before hop 1:\n{worker}"
    );
    assert!(
        !ui.contains("fold_vision_sidecar_into_convo")
            && !ui.contains("apply_vision_sidecar")
            && !ui.contains("describe_media"),
        "UI-thread spawn must not run the vision sidecar:\n{ui}"
    );
    assert!(
        !ui.contains("club.token_usage") && !ui.contains("club.cache_usage"),
        "UI-thread spawn must not snapshot club usage mutexes:\n{ui}"
    );
    assert!(
        worker.contains("club.token_usage") && worker.contains("club.cache_usage"),
        "club usage snapshots must be taken inside the worker:\n{worker}"
    );
    assert!(
        !ui.contains("convo.to_vec()"),
        "UI-thread spawn must not materialize the history Vec:\n{ui}"
    );
    assert!(
        worker.contains("convo.to_vec()"),
        "shared history Arc must become a Vec inside the worker:\n{worker}"
    );
    assert!(
        !ui.contains("configured_max_hops"),
        "UI-thread spawn must not getenv ANGEL_MAX_HOPS:\n{ui}"
    );
    assert!(
        worker.contains("configured_max_hops"),
        "hop cap must be resolved inside the worker:\n{worker}"
    );
    assert!(
        !ui.contains("club.route_identity"),
        "UI-thread spawn must take the precomputed route identity:\n{ui}"
    );
}

#[test]
fn spawn_reports_a_held_foreground_lease_as_the_turn_result() {
    let _env = crate::tests::env_lock();
    let _backplane = crate::tests::TestEnvGuard::unset("ANGEL_BACKPLANE");
    let bag = crate::club::Bag::for_render_test(&[("spark", &[("swarm", true)])]);
    let mut registry = crate::harness::ToolRegistry::new();
    registry.set_backplane(crate::backplane::BackplaneRegistry::from_bag(&bag));
    let tools = Arc::new(registry);
    let club = bag.in_hand();
    let identity = club.route_identity();
    let backplane = tools.backplane();
    let (route_id, group) = backplane
        .resource_group_for_identity(&identity)
        .expect("render-test bag publishes a unique foreground route");
    let _held = backplane
        .acquire_scoped(
            &group,
            crate::backplane::LeaseMode::Serve,
            crate::backplane::WorkloadRole::Foreground,
            Some(route_id),
            false,
        )
        .expect("hold the foreground slot");

    let thinking = Thinking::spawn(
        club.label().to_string(),
        club.clone(),
        tools,
        Arc::from([ChatMsg::user("hello")]),
        Arc::new(crate::steer::SteerQueue::default()),
        crate::session::Session::disabled(),
        club.route_identity(),
        None,
    );
    let result = thinking
        .rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("worker must report the lease conflict without a provider hop");
    let error = result.expect_err("held foreground lease is a turn error");
    assert!(
        error.contains("foreground route resource conflict"),
        "{error}"
    );
}

#[test]
fn spawn_snapshots_club_usage_inside_the_worker() {
    let _env = crate::tests::env_lock();
    let workspace =
        crate::tests::TestGitWorkspace::new("spawn_snapshots_club_usage_inside_the_worker");
    let _backplane = crate::tests::TestEnvGuard::unset("ANGEL_BACKPLANE");
    let _hops = crate::tests::TestEnvGuard::unset("ANGEL_MAX_HOPS");
    let _traj = crate::tests::TestEnvGuard::unset("ANGEL_TRAJECTORY_LOG");

    struct ProbeClub {
        usage_threads: Mutex<Vec<std::thread::ThreadId>>,
        cache_threads: Mutex<Vec<std::thread::ThreadId>>,
    }
    impl Club for ProbeClub {
        fn respond(&self, prompt: &str) -> Result<String, String> {
            Ok(format!("probe:{prompt}"))
        }
        fn label(&self) -> &str {
            "probe-usage"
        }
        fn token_usage(&self) -> Option<TokenUsage> {
            self.usage_threads
                .lock()
                .expect("usage thread log")
                .push(std::thread::current().id());
            Some(TokenUsage {
                turns: 2,
                last_input: 40,
                last_output: 5,
                last_reasoning: 0,
                total_input: 111,
                total_output: 9,
                total_reasoning: 0,
            })
        }
        fn cache_usage(&self) -> CacheUsage {
            self.cache_threads
                .lock()
                .expect("cache thread log")
                .push(std::thread::current().id());
            CacheUsage {
                read_input_tokens: 22,
                read_accounting_responses: 1,
                ..CacheUsage::default()
            }
        }
    }

    let club = Arc::new(ProbeClub {
        usage_threads: Mutex::new(Vec::new()),
        cache_threads: Mutex::new(Vec::new()),
    });
    let ui_thread = std::thread::current().id();
    let thinking = Thinking::spawn(
        club.label().to_string(),
        club.clone(),
        Arc::new(workspace.registry()),
        Arc::from([ChatMsg::user("hello")]),
        Arc::new(crate::steer::SteerQueue::default()),
        crate::session::Session::disabled(),
        club.route_identity(),
        None,
    );
    let _ = thinking
        .rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("probe turn must settle");
    let before = thinking
        .spawn_usage
        .get()
        .copied()
        .expect("worker publishes spawn usage before hops");
    assert_eq!(
        before.usage_before.map(|usage| usage.total_input),
        Some(111)
    );
    assert_eq!(before.cache_before.read_input_tokens, 22);
    let usage_threads = club.usage_threads.lock().expect("usage thread log");
    let cache_threads = club.cache_threads.lock().expect("cache thread log");
    assert!(
        !usage_threads.is_empty() && usage_threads.iter().all(|id| *id != ui_thread),
        "token_usage must run off the UI thread: {usage_threads:?}"
    );
    assert!(
        !cache_threads.is_empty() && cache_threads.iter().all(|id| *id != ui_thread),
        "cache_usage must run off the UI thread: {cache_threads:?}"
    );
}

#[test]
fn spawn_materializes_the_shared_history_snapshot() {
    let _env = crate::tests::env_lock();
    let workspace =
        crate::tests::TestGitWorkspace::new("spawn_materializes_the_shared_history_snapshot");
    let _backplane = crate::tests::TestEnvGuard::unset("ANGEL_BACKPLANE");
    let _hops = crate::tests::TestEnvGuard::unset("ANGEL_MAX_HOPS");
    let _traj = crate::tests::TestEnvGuard::unset("ANGEL_TRAJECTORY_LOG");

    struct ProbeClub {
        seen: Mutex<Vec<Vec<(crate::club::ChatRole, String)>>>,
    }
    impl Club for ProbeClub {
        fn respond(&self, prompt: &str) -> Result<String, String> {
            Ok(format!("probe:{prompt}"))
        }
        fn label(&self) -> &str {
            "probe-history"
        }
        fn chat(
            &self,
            messages: &[ChatMsg],
            _tools: &[ToolDef],
        ) -> Result<crate::club::ClubReply, String> {
            self.seen.lock().expect("seen hop log").push(
                messages
                    .iter()
                    .map(|message| (message.role.clone(), message.content.to_string()))
                    .collect(),
            );
            assert_eq!(
                crate::harness::run_identity::live_turn().as_deref(),
                Some("snapshot-loop-owner")
            );
            Ok(crate::club::ClubReply::Text("probe:ok".to_string()))
        }
    }

    let club = Arc::new(ProbeClub {
        seen: Mutex::new(Vec::new()),
    });
    let history: Arc<[ChatMsg]> = Arc::from(vec![
        ChatMsg::system("orchestrator prompt"),
        ChatMsg::user("same snapshot as persist"),
    ]);
    let thinking = Thinking::spawn(
        club.label().to_string(),
        club.clone(),
        Arc::new(workspace.registry()),
        Arc::clone(&history),
        Arc::new(crate::steer::SteerQueue::default()),
        crate::session::Session::disabled(),
        club.route_identity(),
        Some("snapshot-loop-owner".into()),
    );
    let (convo, answer, _, _) = thinking
        .rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("probe turn must settle")
        .expect("probe turn succeeds");
    assert_eq!(answer, "probe:ok");
    assert_eq!(convo.len(), history.len() + 1);
    assert_eq!(convo[0].role, history[0].role);
    assert_eq!(&*convo[0].content, &*history[0].content);
    assert_eq!(convo[1].role, history[1].role);
    assert_eq!(&*convo[1].content, &*history[1].content);
    let seen = club.seen.lock().expect("seen hop log");
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0],
        vec![
            (
                crate::club::ChatRole::System,
                "orchestrator prompt".to_string()
            ),
            (
                crate::club::ChatRole::User,
                "same snapshot as persist".to_string()
            ),
        ]
    );
}

#[test]
fn cache_hit_pct_handles_both_provider_accounting_styles() {
    // OpenAI-style: cached tokens are inside prompt_tokens.
    assert_eq!(cache_hit_pct(75, 100), Some(75));
    // Anthropic-style: cache reads are disjoint from input_tokens.
    assert_eq!(cache_hit_pct(9_000, 1_000), Some(90));
    assert_eq!(cache_hit_pct(0, 100), Some(0));
    // Nothing reported at all → no rate, not 0%.
    assert_eq!(cache_hit_pct(0, 0), None);
}

#[test]
fn cache_meter_says_na_until_a_provider_reports_cache_fields() {
    let mut meter = CacheMeter::default();
    assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
    // A local backend turn: tokens maybe, but no cache accounting.
    meter.fold_turn(0, 5_000, false);
    assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
    assert_eq!(meter.session_input, 0, "unreported turns never dilute");
}

#[test]
fn cache_meter_folds_session_aggregate_and_last_turn() {
    let mut meter = CacheMeter::default();
    meter.fold_turn(50, 100, true);
    assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
    meter.fold_turn(250, 300, true);
    // Session: 300/400; last turn: 250/300.
    assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
    // A later cache-blind turn keeps the session rate but reports honestly.
    meter.fold_turn(0, 900, false);
    assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
}
