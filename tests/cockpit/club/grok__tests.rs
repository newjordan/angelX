use super::*;
use std::ffi::OsString;

/// Delegates to the crate-wide test env lock (process env is global — a
/// module-local lock can't serialize against other modules' env tests).
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::tests::env_lock()
}

struct EnvGuard {
    key: &'static str,
    old: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }

    fn unset(key: &'static str) -> Self {
        let old = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(old) = &self.old {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(self.key, old) };
        } else {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(self.key) };
        }
        if self.key.ends_with("REASONING_EFFORT") || self.key.ends_with("REASONING_DIALECT") {
            super::http::resync_reasoning_effort_env_from_env();
        }
    }
}

#[test]
fn grok_uses_oauth_file_even_with_api_key_env() {
    let _lock = env_lock();
    let dir = std::env::temp_dir().join(format!("angel0-grok-auth-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let auth = dir.join("auth.json");
    std::fs::write(&auth, r#"{"entry":{"key":"oauth-token"}}"#).unwrap();
    let _file = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
    let _key = EnvGuard::set("ANGEL_GROK_KEY", "api-token");
    let _cmd = EnvGuard::set("ANGEL_GROK_CMD", "/bin/true");
    let _api = EnvGuard::unset("ANGEL_GROK_USE_API");
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");

    let club = GrokResearchClub::from_env().expect("oauth-backed grok scout");
    assert_eq!(club.mode_kind(), "acp-oauth");
}

#[test]
fn grok_acp_requires_account_login_even_with_legacy_api_flag() {
    let _lock = env_lock();
    let _file = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", "/definitely/missing/auth.json");
    let _key = EnvGuard::set("ANGEL_GROK_KEY", "api-token");
    let _api = EnvGuard::set("ANGEL_GROK_USE_API", "1");
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");

    assert!(GrokResearchClub::from_env().is_none());
}

#[test]
fn grok_acp_does_not_reuse_the_http_api_key() {
    let _lock = env_lock();
    let _file = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", "/definitely/missing/auth.json");
    let _key = EnvGuard::set("ANGEL_GROK_KEY", "api-token");
    let _api = EnvGuard::unset("ANGEL_GROK_USE_API");
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");

    assert!(GrokResearchClub::from_env().is_none());
}

#[test]
fn grok_acp_child_is_account_oauth_headless_and_scrubbed() {
    let _lock = env_lock();
    let _xai = EnvGuard::set("XAI_API_KEY", "must-not-leak");
    let _grok = EnvGuard::set("GROK_API_KEY", "must-not-leak");
    let key = GrokAcpKey {
        command: "grok".to_string(),
        model: Some("grok-4.6".to_string()),
        effort: Some("low".to_string()),
        surface: GrokAcpSurface::Research,
    };
    let cmd = build_grok_acp_command(&key);
    let args = cmd
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(args.iter().any(|arg| arg == "--no-auto-update"));
    assert!(args.iter().any(|arg| arg == "--oauth"));
    assert!(args.iter().any(|arg| arg == "--always-approve"));
    let agent_at = args
        .iter()
        .position(|arg| arg == "agent")
        .expect("ACP argv starts the agent subcommand");
    let effort_at = args
        .iter()
        .position(|arg| arg == "--reasoning-effort")
        .expect("ACP argv forwards reasoning effort");
    assert!(
        effort_at > agent_at,
        "reasoning effort is a grok agent flag: {args:?}"
    );
    assert_eq!(args.last().map(String::as_str), Some("stdio"));
    let removed = cmd
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(key, _)| key.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    for secret in [
        "XAI_API_KEY",
        "GROK_API_KEY",
        "ANGEL_GROK_KEY",
        "ANGEL_XAI_KEY",
    ] {
        assert!(
            removed.iter().any(|key| key == secret),
            "{secret} was not scrubbed"
        );
    }
}

fn grok_login_state_with_credentials(tag: &str, auth_json: &str) -> bool {
    let dir =
        std::env::temp_dir().join(format!("angel0-grok-refresh-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let auth = dir.join("auth.json");
    std::fs::write(&auth, auth_json).unwrap();
    let _file = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
    grok_oauth_needs_login()
}

/// The regression: a spent access token with a refresh token beside it is
/// the CLI's normal steady state, not a fatal one. Blocking here took the
/// whole Grok club — research scout included — offline between logins.
#[test]
fn grok_runs_with_expired_access_token_when_refresh_token_is_present() {
    let _lock = env_lock();
    let needs_login = grok_login_state_with_credentials(
        "refreshable",
        r#"{"https://auth.x.ai::acct":{"key":"oauth-token","refresh_token":"rt",
                "expires_at":"2020-01-01T00:00:00.000000000Z"}}"#,
    );
    assert!(!needs_login);
}

#[test]
fn grok_demands_login_only_when_no_refresh_token_can_renew() {
    let _lock = env_lock();
    let needs_login = grok_login_state_with_credentials(
        "stale",
        r#"{"https://auth.x.ai::acct":{"key":"oauth-token",
                "expires_at":"2020-01-01T00:00:00.000000000Z"}}"#,
    );
    assert!(needs_login);
}

#[test]
fn grok_runs_with_unexpired_access_token() {
    let _lock = env_lock();
    let needs_login = grok_login_state_with_credentials(
        "fresh",
        r#"{"https://auth.x.ai::acct":{"key":"oauth-token",
                "expires_at":"2099-01-01T00:00:00.000000000Z"}}"#,
    );
    assert!(!needs_login);
}

/// A second account keeps the club alive even when one record is stale.
#[test]
fn grok_runs_when_any_credential_record_is_usable() {
    let _lock = env_lock();
    let needs_login = grok_login_state_with_credentials(
        "mixed",
        r#"{"a":{"expires_at":"2020-01-01T00:00:00.000000000Z"},
                "b":{"refresh_token":"rt","expires_at":"2020-01-01T00:00:00.000000000Z"}}"#,
    );
    assert!(!needs_login);
}

#[test]
fn grok_login_hint_rides_only_on_auth_failures() {
    assert!(looks_like_grok_auth_failure(
        "request failed: 401 Unauthorized"
    ));
    assert!(looks_like_grok_auth_failure("error: invalid_grant"));
    assert!(!looks_like_grok_auth_failure(
        "model grok-4.99 is not available"
    ));
    assert!(!looks_like_grok_auth_failure("connection reset by peer"));
}

#[test]
fn grok_effort_ladder_is_selectable_and_canonical() {
    let _lock = env_lock();
    let _grok = EnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
    let _global = EnvGuard::unset("ANGEL_REASONING_EFFORT");
    super::http::resync_reasoning_effort_env_from_env();
    let club = GrokResearchClub {
        name: "grok-research".to_string(),
        mode: GrokMode::AcpOAuth {
            command: "/bin/true".to_string(),
            model: Some("grok-4.5".to_string()),
            timeout: Duration::from_secs(5),
        },
        usage: UsageCell::default(),
        accounting: super::AccountingCell::default(),
        effort_override: Mutex::new(None),
        effort_snapshot: Mutex::new(None),
        route_state_revision: AtomicU64::new(0),
        reasoning_levels: vec!["low".into(), "medium".into(), "high".into()],
        reasoning_descriptions: vec![
            ("low".into(), "fast".into()),
            ("medium".into(), "balanced".into()),
            ("high".into(), "deep".into()),
        ],
        default_effort: Some("medium".to_string()),
        context_window: Some(500_000),
        acp: Mutex::new(None),
    };

    assert_eq!(
        club.reasoning_levels(),
        vec!["low".to_string(), "medium".to_string(), "high".to_string()]
    );
    assert_eq!(club.reasoning_effort().as_deref(), Some("low"));
    assert_eq!(club.set_reasoning_effort("HIGH").as_deref(), Some("high"));
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
    assert!(club.set_reasoning_effort("invented").is_none());
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
    assert_eq!(club.route_metadata().context_window, Some(500_000));
    assert_eq!(
        club.route_metadata()
            .reasoning_description("high")
            .map(str::to_string)
            .as_deref(),
        Some("deep")
    );
}

#[test]
fn grok_effort_env_seeds_once_and_resyncs_under_env_lock() {
    let _lock = env_lock();
    let _snap = EnvGuard::set("ANGEL_EFFORT_SNAPSHOT", "0");
    let club = GrokResearchClub {
        name: "grok-research".to_string(),
        mode: GrokMode::AcpOAuth {
            command: "/bin/true".to_string(),
            model: Some("grok-4.5".to_string()),
            timeout: Duration::from_secs(5),
        },
        usage: UsageCell::default(),
        accounting: super::AccountingCell::default(),
        effort_override: Mutex::new(None),
        effort_snapshot: Mutex::new(None),
        route_state_revision: AtomicU64::new(0),
        reasoning_levels: vec!["low".into(), "medium".into(), "high".into()],
        reasoning_descriptions: Vec::new(),
        default_effort: Some("medium".to_string()),
        context_window: None,
        acp: Mutex::new(None),
    };
    {
        let _grok = EnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
        let _global = EnvGuard::unset("ANGEL_REASONING_EFFORT");
        super::http::resync_reasoning_effort_env_from_env();
        assert_eq!(club.reasoning_effort().as_deref(), Some("low"));

        let _set = EnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "high");
        assert_eq!(
            club.reasoning_effort().as_deref(),
            Some("low"),
            "cache must not re-read env until seed reset"
        );
        super::http::resync_reasoning_effort_env_from_env();
        assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
        assert_eq!(club.set_reasoning_effort("low").as_deref(), Some("low"));
        assert_eq!(club.reasoning_effort().as_deref(), Some("low"));
    }
    super::http::resync_reasoning_effort_env_from_env();
}

#[test]
fn grok_effort_snapshot_invalidates_on_env_resync() {
    let _lock = env_lock();
    let _snap = EnvGuard::set("ANGEL_EFFORT_SNAPSHOT", "1");
    let club = GrokResearchClub {
        name: "grok-research".to_string(),
        mode: GrokMode::AcpOAuth {
            command: "/bin/true".to_string(),
            model: Some("grok-4.6".to_string()),
            timeout: Duration::from_secs(5),
        },
        usage: UsageCell::default(),
        accounting: super::AccountingCell::default(),
        effort_override: Mutex::new(None),
        effort_snapshot: Mutex::new(None),
        route_state_revision: AtomicU64::new(0),
        reasoning_levels: vec!["low".into(), "medium".into(), "high".into(), "xhigh".into()],
        reasoning_descriptions: Vec::new(),
        default_effort: Some("medium".to_string()),
        context_window: None,
        acp: Mutex::new(None),
    };
    {
        let _grok = EnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
        let _global = EnvGuard::unset("ANGEL_REASONING_EFFORT");
        super::http::resync_reasoning_effort_env_from_env();
        assert_eq!(club.reasoning_effort().as_deref(), Some("low"));

        let _set = EnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "xhigh");
        assert_eq!(
            club.reasoning_effort().as_deref(),
            Some("low"),
            "snapshot must hold the seeded env until resync"
        );
        super::http::resync_reasoning_effort_env_from_env();
        assert_eq!(
            club.reasoning_effort().as_deref(),
            Some("xhigh"),
            "formation-style resync must invalidate the snapshot immediately"
        );
    }
    super::http::resync_reasoning_effort_env_from_env();
}

#[test]
fn grok_acp_command_forwards_effort_without_an_arbitrary_turn_cap() {
    let _lock = env_lock();
    let _turns = EnvGuard::unset("ANGEL_GROK_MAX_TURNS");
    let mut key = GrokAcpKey {
        command: "grok".to_string(),
        model: Some("grok-4.5".to_string()),
        effort: Some("high".to_string()),
        surface: GrokAcpSurface::Research,
    };
    let cmd = build_grok_acp_command(&key);
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let agent_at = args
        .iter()
        .position(|arg| arg == "agent")
        .expect("ACP argv starts the agent subcommand");
    let effort_at = args
        .iter()
        .position(|arg| arg == "--reasoning-effort")
        .expect("ACP argv forwards reasoning effort");
    assert!(
        effort_at > agent_at,
        "reasoning effort is a grok agent flag, not a top-level flag: {args:?}"
    );
    assert_eq!(args.get(effort_at + 1).map(String::as_str), Some("high"));
    assert!(args.windows(2).any(|w| w == ["--model", "grok-4.5"]));
    assert_eq!(args.last().map(String::as_str), Some("stdio"));
    assert!(args.iter().any(|a| a == "--always-approve"));
    assert!(
        !args.iter().any(|arg| arg == "--max-turns"),
        "the cockpit must not truncate Grok's nested work by default: {args:?}"
    );

    let _turns = EnvGuard::set("ANGEL_GROK_MAX_TURNS", "48");
    key.surface = GrokAcpSurface::Research;
    let capped = build_grok_acp_command(&key);
    let capped: Vec<String> = capped
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert!(capped.windows(2).any(|w| w == ["--max-turns", "48"]));
}

#[test]
fn grok_harness_surface_denies_nested_agent_tools() {
    // Regression for "angel freezes when grok is the driver": the harness
    // hop must not open Grok's own multi-tool agent loop.
    let key = GrokAcpKey {
        command: "grok".to_string(),
        model: Some("grok-4.5".to_string()),
        effort: Some("low".to_string()),
        surface: GrokAcpSurface::Harness,
    };
    let cmd = build_grok_acp_command(&key);
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(
        args.windows(2)
            .any(|w| w[0] == "--disallowed-tools" && w[1].contains("run_terminal_cmd")),
        "harness surface must strip Grok shell tools: {args:?}"
    );
    assert!(
        args.iter().any(|a| a == "--deny"),
        "harness surface must also deny by permission prefix: {args:?}"
    );
    assert!(
        args.windows(2)
            .any(|w| { w[0] == "--max-turns" && w[1] == GROK_HARNESS_MAX_TURNS.to_string() }),
        "harness needs a small nested budget (>1) so denied tools do not hard-fail: {args:?}"
    );
    assert!(
        args.windows(2)
            .any(|w| { w[0] == "--system-prompt-override" && w[1].contains("pure completion") }),
        "harness must override the CLI agent persona: {args:?}"
    );
    assert!(
        args.iter().any(|a| a == "--no-subagents"),
        "harness must not spawn nested agents: {args:?}"
    );
}

#[test]
fn harness_prompt_contract_lists_host_tools_and_markup() {
    let tools = [ToolDef {
        name: "shell".into(),
        description: "run a shell command".into(),
        params: serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"},
                "cwd": {"type": "string"}
            }
        }),
    }];
    let body = harness_prompt_with_tool_contract("User: list tmp", &tools);
    assert!(
        body.contains("<tool_call name=\"TOOL_NAME\">"),
        "contract must teach the markup dialect: {body}"
    );
    assert!(
        body.contains("`shell`"),
        "contract must name offered tools: {body}"
    );
    assert!(
        body.contains("{command, cwd}") || body.contains("{cwd, command}"),
        "contract should surface param keys: {body}"
    );
    assert!(
        body.contains("User: list tmp"),
        "transcript must follow the contract: {body}"
    );
}

#[test]
fn filter_offered_tool_calls_drops_unknown_names() {
    let tools = [ToolDef {
        name: "shell".into(),
        description: String::new(),
        params: serde_json::json!({}),
    }];
    let calls = vec![
        ToolCall {
            id: "1".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": "pwd"}),
        },
        ToolCall {
            id: "2".into(),
            name: "run_terminal_cmd".into(),
            args: serde_json::json!({"command": "pwd"}),
        },
    ];
    let kept = filter_offered_tool_calls(calls, &tools);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].name, "shell");
}

#[test]
fn max_turns_reached_is_a_soft_acp_failure() {
    assert!(is_soft_grok_agent_failure(
        "Max turns reached\nError: max turns reached"
    ));
    assert!(is_soft_grok_agent_failure("error: maximum number of turns"));
    assert!(!is_soft_grok_agent_failure("401 Unauthorized"));
    assert!(!is_soft_grok_agent_failure("connection reset by peer"));
}

#[test]
fn grok_chat_prompt_keeps_conversation_context() {
    let prompt = GrokResearchClub::chat_prompt(&[
        ChatMsg::system("be brief"),
        ChatMsg::user("hi"),
        ChatMsg::assistant("hello"),
        ChatMsg::user("again"),
    ]);
    assert!(prompt.contains("System: be brief"));
    assert!(prompt.contains("User: hi"));
    assert!(prompt.contains("Assistant: hello"));
    assert!(prompt.contains("User: again"));
}

#[test]
fn grok_chat_prompt_keeps_tool_calls_and_results() {
    let prompt = GrokResearchClub::chat_prompt(&[
        ChatMsg::user("list tmp"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "c1".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": "ls /tmp"}),
        }]),
        ChatMsg::tool("c1", "file.txt\n"),
    ]);
    assert!(
        prompt.contains("<tool_call name=\"shell\">"),
        "assistant tool calls must survive the Grok transcript: {prompt}"
    );
    assert!(
        prompt.contains("[tool_result id=c1]"),
        "tool results must survive the Grok transcript: {prompt}"
    );
    assert!(prompt.contains("file.txt"));
}

#[test]
fn grok_harness_reply_recovers_only_offered_prose_tool_calls() {
    let tools = [ToolDef {
        name: "shell".into(),
        description: "run a shell command".into(),
        params: serde_json::json!({"type":"object"}),
    }];
    let text = "<tool_call name=\"shell\">{\"command\":\"pwd\"}</tool_call>\n\
                    <tool_call name=\"run_terminal_cmd\">{\"command\":\"id\"}</tool_call>";
    let calls = filter_offered_tool_calls(extract_prose_tool_calls(text), &tools);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "shell");
}

#[test]
fn grok_model_aliases_cover_grok_4_family_options() {
    assert_eq!(GROK_SOTA_MODEL_OPTIONS[0], "grok-4.6");
    assert!(GROK_SOTA_MODEL_OPTIONS.contains(&"grok-4.6"));
    assert!(GROK_SOTA_MODEL_OPTIONS.contains(&"grok-4"));
    assert!(GROK_SOTA_MODEL_OPTIONS.contains(&"grok-4.5"));
    let available = vec![
        "grok-composer-2.5-fast".to_string(),
        "grok-4.5".to_string(),
        "grok-4.6".to_string(),
    ];
    assert_eq!(
        resolve_grok_model_alias_with_available("4", &available),
        "grok-4.6"
    );
    assert_eq!(
        resolve_grok_model_alias_with_available("grok-4", &available),
        "grok-4.6"
    );
    assert_eq!(
        resolve_grok_model_alias_with_available("latest", &available),
        "grok-4.6"
    );
    assert_eq!(
        resolve_grok_model_alias_with_available("grok 4 new", &available),
        "grok-4.6"
    );
    assert_eq!(
        resolve_grok_model_alias_with_available("grok", &[]),
        "grok-4.6"
    );
    assert_eq!(
        resolve_grok_model_alias_with_available("grok4.6", &[]),
        "grok-4.6"
    );
    assert_eq!(
        resolve_grok_model_alias_with_available("latest", &[]),
        "grok-4.6"
    );
    assert_eq!(
        resolve_grok_model_alias_with_available("grok4.5", &available),
        "grok-4.5"
    );
    assert_eq!(
        resolve_grok_model_alias_with_available("grok-4-20-fast", &available),
        "grok-4.20-fast"
    );
}

#[test]
fn grok_46_builtin_catalog_is_exact_and_network_free() {
    let catalog = grok_model_catalog(Some("grok-4.6"));
    assert_eq!(catalog.levels, vec!["low", "medium", "high", "xhigh"]);
    assert_eq!(catalog.default_effort.as_deref(), Some("high"));
    assert_eq!(catalog.context_window, Some(500_000));
}

#[test]
fn grok_model_default_and_bare_alias_are_canonical_before_launch() {
    let _lock = env_lock();
    let _angel_model = EnvGuard::unset("ANGEL_GROK_MODEL");
    let _legacy_model = EnvGuard::unset("GROK_MODEL");
    assert_eq!(grok_model_from_env(), "grok-4.6");

    let _bare = EnvGuard::set("ANGEL_GROK_MODEL", "grok");
    assert_eq!(grok_model_from_env(), "grok-4.6");
}

#[test]
fn parses_grok_oauth_expiry() {
    assert_eq!(parse_rfc3339_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(
        parse_rfc3339_seconds("1970-01-02T00:00:00.123Z"),
        Some(86_400)
    );
}

#[test]
fn grok_startup_step_timeout_and_completion_are_bounded() {
    let _lock = env_lock();
    let (release, wait) = std::sync::mpsc::channel();
    let started = Instant::now();
    let result = grok_step("fixture-stall", Duration::from_millis(20), move || {
        wait.recv().unwrap();
    });
    assert!(result.unwrap_err().contains("timed out"));
    assert!(started.elapsed() < Duration::from_secs(1));
    release.send(()).unwrap();
    assert_eq!(
        grok_step("fixture-ready", Duration::from_secs(1), || 42),
        Ok(42)
    );
}

#[test]
#[cfg(unix)]
fn grok_startup_file_read_rejects_fifo_without_waiting_for_writer() {
    let _lock = env_lock();
    let directory = std::env::temp_dir().join(format!("grok-startup-fifo-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("auth-fifo");
    let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: name is a valid NUL-terminated pathname owned by this test.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let started = Instant::now();
    assert!(
        grok_read("fixture-fifo", path)
            .unwrap_err()
            .to_string()
            .contains("regular file")
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn grok_oauth_auth_loads_and_skips_refresh_when_fresh() {
    let _lock = env_lock();
    let dir = std::env::temp_dir().join(format!("angel0-grok-oauth-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let auth_path = dir.join("auth.json");
    let far_future = "2099-01-01T00:00:00Z";
    std::fs::write(
        &auth_path,
        format!(
            r#"{{"https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828":{{
                  "key":"oauth-access-token",
                  "refresh_token":"rt-test",
                  "expires_at":"{far_future}",
                  "oidc_client_id":"b1a00492-073a-47ea-816f-4c329264a828",
                  "oidc_issuer":"https://auth.x.ai"
                }}}}"#
        ),
    )
    .unwrap();
    let _file = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth_path.to_str().unwrap());
    let auth = GrokOauthAuth::load().expect("load oauth fixture");
    assert_eq!(auth.access_token, "oauth-access-token");
    assert!(!auth.needs_refresh(1_700_000_000));
    let shared = GrokOauthShared::new(auth);
    assert_eq!(shared.bearer().unwrap(), "oauth-access-token");
}

#[test]
fn grok_oauth_http_catalog_lists_46_and_live_supported_models() {
    let _lock = env_lock();
    let dir = std::env::temp_dir().join(format!("angel0-grok-oauth-club-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let auth_path = dir.join("auth.json");
    let models_path = dir.join("models_cache.json");
    std::fs::write(
        &auth_path,
        r#"{"https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828":{
              "key":"oauth-access-token",
              "refresh_token":"rt-test",
              "expires_at":"2099-01-01T00:00:00Z",
              "oidc_client_id":"b1a00492-073a-47ea-816f-4c329264a828",
              "oidc_issuer":"https://auth.x.ai"
            }}"#,
    )
    .unwrap();
    std::fs::write(&models_path, r#"{"models":{"grok-4.5":{},"grok-4.6":{}}}"#).unwrap();
    let _file = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth_path.to_str().unwrap());
    let _models = EnvGuard::set("ANGEL_GROK_MODELS_CACHE", models_path.to_str().unwrap());
    let _model = EnvGuard::set("ANGEL_GROK_MODEL", "grok-4.5");
    let _research_off = EnvGuard::set("ANGEL_GROK_RESEARCH", "0");
    let _key = EnvGuard::set("XAI_API_KEY", "must-not-select-paid-api");
    let _allow = EnvGuard::set("ANGEL_API_CLUBS", "grok");
    assert!(
        GrokResearchClub::from_env().is_none(),
        "ACP research stays gated on ANGEL_GROK_RESEARCH"
    );
    let links = grok_http_clubs();
    let routes = links
        .iter()
        .map(|(alias, club, _)| (alias.as_str(), club.model_identity().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        routes,
        vec![
            ("grok", "grok-4.5".to_string()),
            ("grok-4.6", "grok-4.6".to_string()),
            ("grok-api", "grok-4.6".to_string()),
        ]
    );
    assert_eq!(links[0].1.label(), "grok");
    assert_ne!(links[0].1.label(), "grok-research");
    assert!(Arc::ptr_eq(&links[0].2, &links[1].2));
    assert!(!Arc::ptr_eq(&links[0].2, &links[2].2));

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_GROK_MODEL", "grok-4.6") };
    let current = grok_http_clubs();
    assert_eq!(current.len(), 3);
    assert_eq!(current[0].0, "grok");
    assert_eq!(current[0].1.model_identity().as_deref(), Some("grok-4.6"));
    assert_eq!(current[1].0, "grok-4.5");
    assert_eq!(current[1].1.model_identity().as_deref(), Some("grok-4.5"));
    assert_eq!(current[2].0, "grok-api");
    assert_eq!(current[2].1.model_identity().as_deref(), Some("grok-4.6"));
}

#[test]
fn grok_api_route_uses_explicit_key_model_path_and_tools() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let _lock = env_lock();
    let _oauth = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", "/definitely/missing/auth.json");
    let _key = EnvGuard::set("XAI_API_KEY", "fixture-api-key");
    let _allow = EnvGuard::set("ANGEL_API_CLUBS", "grok");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        let header_end = request.len();
        let header_text = String::from_utf8_lossy(&request[..header_end]);
        let content_length = header_text
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then_some(value)
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let mut body = vec![0_u8; content_length];
        socket.read_exact(&mut body).unwrap();
        request.extend(body);
        let response = serde_json::json!({"choices":[{"message":{
                "role":"assistant", "tool_calls":[{"id":"call_fixture","type":"function",
                    "function":{"name":"shell","arguments":"{\"command\":\"pwd\"}"}}]
            },"finish_reason":"tool_calls"}]})
        .to_string();
        let wire = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        );
        socket.write_all(wire.as_bytes()).unwrap();
        String::from_utf8(request).unwrap()
    });
    let _url = EnvGuard::set("ANGEL_GROK_API_URL", &url);
    let _model = EnvGuard::set("ANGEL_GROK_API_MODEL", "fixture-model");
    let (_, club, _) = grok_http_clubs()
        .into_iter()
        .find(|(alias, _, _)| alias == "grok-api")
        .expect("explicit API key should create grok-api");
    let tools = [ToolDef {
        name: "shell".to_string(),
        description: "Run a shell command".to_string(),
        params: serde_json::json!({"type":"object","properties":{"command":{"type":"string"}}}),
    }];
    let reply = club.chat(&[ChatMsg::user("run pwd")], &tools).unwrap();
    assert!(matches!(reply, ClubReply::Calls(calls)
            if calls.len() == 1 && calls[0].id == "call_fixture"
            && calls[0].name == "shell" && calls[0].args["command"] == "pwd"));
    let request = server.join().unwrap();
    assert!(
        request.starts_with("POST /v1/chat/completions "),
        "{request}"
    );
    assert!(
        request.contains("Authorization: Bearer fixture-api-key"),
        "{request}"
    );
    let body = request.split_once("\r\n\r\n").unwrap().1;
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "fixture-model");
    assert_eq!(body["tools"][0]["function"]["name"], "shell");
}

#[test]
fn grok_api_route_is_absent_when_api_family_is_none() {
    let _lock = env_lock();
    let _oauth = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", "/definitely/missing/auth.json");
    let _key = EnvGuard::set("XAI_API_KEY", "fixture-api-key");
    let _allow = EnvGuard::set("ANGEL_API_CLUBS", "none");
    assert!(grok_http_clubs().is_empty());
}

#[cfg(unix)]
fn fake_grok_acp_fixture(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("angel0-grok-acp-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let auth = dir.join("auth.json");
    let script = dir.join("fake-grok-acp");
    let spawns = dir.join("spawns.log");
    std::fs::write(
        &auth,
        r#"{"https://auth.x.ai::acct":{"key":"oauth-token","refresh_token":"rt",
                "expires_at":"2099-01-01T00:00:00Z"}}"#,
    )
    .unwrap();
    std::fs::write(
            &script,
            r#"#!/bin/sh
if [ -n "${XAI_API_KEY+x}" ] || [ -n "${GROK_API_KEY+x}" ] || [ -n "${ANGEL_GROK_KEY+x}" ] || [ -n "${ANGEL_XAI_KEY+x}" ]; then
  printf 'paid API key leaked to ACP child\n' >&2
  exit 91
fi
printf 'spawn\n' >> "${ANGEL_FAKE_GROK_SPAWNS}"
spawn_no=$(wc -l < "${ANGEL_FAKE_GROK_SPAWNS}" | tr -d ' ')
session_no=0
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"initialize"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":1,"authMethods":[{"id":"cached_token"}],"agentCapabilities":{}}}\n' "$id"
      ;;
    *'"method":"authenticate"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id"
      ;;
    *'"method":"session/new"'*)
      session_no=$((session_no + 1))
      printf '{"jsonrpc":"2.0","id":%s,"result":{"sessionId":"s%s"}}\n' "$id" "$session_no"
      ;;
    *'"method":"session/prompt"'*)
      if [ "${ANGEL_FAKE_GROK_CRASH_ONCE:-0}" = 1 ] && [ "$spawn_no" = 1 ]; then
        printf 'intentional transport crash\n' >&2
        exit 92
      fi
      if [ "${ANGEL_FAKE_GROK_HANG:-0}" = 1 ]; then
        sleep 30
        continue
      fi
      printf '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s%s","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"fake-%s"}}}}\n' "$session_no" "$session_no"
      printf '{"jsonrpc":"2.0","id":%s,"result":{"stopReason":"end_turn","_meta":{"usage":{"inputTokens":10,"outputTokens":2,"reasoningTokens":1}}}}\n' "$id"
      ;;
    *'"method":"session/cancel"'*)
      ;;
  esac
done
"#,
        )
        .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).unwrap();
    std::fs::write(&spawns, "").unwrap();
    (auth, script, spawns)
}

#[cfg(unix)]
#[test]
fn grok_acp_reuses_one_process_for_sequential_sessions_and_records_usage() {
    let _lock = env_lock();
    let (auth, script, spawns) = fake_grok_acp_fixture("reuse");
    let _auth = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
    let _cmd = EnvGuard::set("ANGEL_GROK_CMD", script.to_str().unwrap());
    let _spawn_log = EnvGuard::set("ANGEL_FAKE_GROK_SPAWNS", spawns.to_str().unwrap());
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");
    let club = GrokResearchClub::from_env().expect("fake ACP club");

    assert_eq!(club.respond("first").unwrap(), "fake-1");
    assert_eq!(club.respond("second").unwrap(), "fake-2");
    assert_eq!(std::fs::read_to_string(&spawns).unwrap().lines().count(), 1);
    let usage = club.token_usage().expect("ACP usage");
    assert_eq!(usage.turns, 2);
    assert_eq!(usage.total_input, 20);
    assert_eq!(usage.total_output, 4);
    assert_eq!(usage.total_reasoning, 2);
}

#[cfg(unix)]
#[test]
fn grok_acp_recovers_once_after_a_real_transport_crash() {
    let _lock = env_lock();
    let (auth, script, spawns) = fake_grok_acp_fixture("restart");
    let _auth = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
    let _cmd = EnvGuard::set("ANGEL_GROK_CMD", script.to_str().unwrap());
    let _spawn_log = EnvGuard::set("ANGEL_FAKE_GROK_SPAWNS", spawns.to_str().unwrap());
    let _crash = EnvGuard::set("ANGEL_FAKE_GROK_CRASH_ONCE", "1");
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");
    let club = GrokResearchClub::from_env().expect("fake ACP club");

    assert_eq!(club.respond("recover me").unwrap(), "fake-1");
    assert_eq!(std::fs::read_to_string(&spawns).unwrap().lines().count(), 2);
}

#[cfg(unix)]
#[test]
fn grok_acp_preset_cancel_returns_promptly_and_invalidates_the_process() {
    let _lock = env_lock();
    let (auth, script, spawns) = fake_grok_acp_fixture("cancel");
    let _auth = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
    let _cmd = EnvGuard::set("ANGEL_GROK_CMD", script.to_str().unwrap());
    let _spawn_log = EnvGuard::set("ANGEL_FAKE_GROK_SPAWNS", spawns.to_str().unwrap());
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");
    let club = GrokResearchClub::from_env().expect("fake ACP club");
    let cancel = AtomicBool::new(true);
    let started = Instant::now();
    let err = club.respond_cancellable("stop", &cancel).unwrap_err();
    assert!(err.contains("cancelled"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[cfg(unix)]
#[test]
fn grok_research_chat_with_host_tools_fails_closed_by_default() {
    let _lock = env_lock();
    let (auth, script, spawns) = fake_grok_acp_fixture("host-tools");
    let _auth = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
    let _cmd = EnvGuard::set("ANGEL_GROK_CMD", script.to_str().unwrap());
    let _spawn_log = EnvGuard::set("ANGEL_FAKE_GROK_SPAWNS", spawns.to_str().unwrap());
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");
    let _markup = EnvGuard::unset(GROK_HARNESS_SURFACE_ENV);
    let club = GrokResearchClub::from_env().expect("fake ACP club");
    let tools = [ToolDef {
        name: "shell".into(),
        description: "run a shell command".into(),
        params: serde_json::json!({"type": "object"}),
    }];
    let err = club
        .chat(&[ChatMsg::user("pwd")], &tools)
        .expect_err("ACP must not pretend to drive Angel host tools");
    assert!(
        err.contains("HTTP OAuth"),
        "error must point at the HTTP driver: {err}"
    );
    assert_eq!(
        std::fs::read_to_string(&spawns).unwrap().lines().count(),
        0,
        "fail-closed before spawning an ACP child"
    );
}

/// Explicit live acceptance for the installed Grok account seat. Kept
/// ignored in ordinary CI because it consumes provider quota and requires
/// `grok login --oauth`; release validation runs it deliberately.
#[test]
#[ignore = "live Grok account OAuth + ACP acceptance"]
fn live_grok_acp_reuses_process_and_runs_its_own_research_tool() {
    let _lock = env_lock();
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");
    let _effort = EnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "low");
    let _timeout = EnvGuard::set("ANGEL_GROK_TIMEOUT_SECS", "120");
    let club = GrokResearchClub::from_env().expect("live account-OAuth Grok ACP club");

    let first = club
        .respond("Reply with exactly ANGEL_ACP_LIVE_ONE and nothing else. Do not use tools.")
        .expect("first live ACP prompt");
    assert_eq!(first.trim(), "ANGEL_ACP_LIVE_ONE");
    let first_pid = club
        .acp
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|slot| slot.connection.child.lock().ok()?.as_ref().map(Child::id))
        .expect("persistent ACP pid after first prompt");

    let second = club
        .respond("Reply with exactly ANGEL_ACP_LIVE_TWO and nothing else. Do not use tools.")
        .expect("second live ACP prompt");
    assert_eq!(second.trim(), "ANGEL_ACP_LIVE_TWO");
    let second_pid = club
        .acp
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|slot| slot.connection.child.lock().ok()?.as_ref().map(Child::id))
        .expect("persistent ACP pid after second prompt");
    assert_eq!(first_pid, second_pid, "sequential prompts must reuse ACP");

    let pwd = club
        .respond("Run pwd using your shell tool and include the resulting path in the reply.")
        .expect("live ACP research tool prompt");
    assert!(
        pwd.contains('/') || pwd.contains("tmp") || pwd.contains("home"),
        "research ACP should run Grok's own shell and report a path: {pwd}"
    );
    let tools = [ToolDef {
        name: "shell".to_string(),
        description: "Run a shell command in the Angel host".to_string(),
        params: serde_json::json!({
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"]
        }),
    }];
    let err = club
        .chat(
            &[ChatMsg::user(
                "Request the offered shell tool with command pwd.",
            )],
            &tools,
        )
        .expect_err("ACP must not claim Angel host-tool driving");
    assert!(err.contains("HTTP OAuth"), "{err}");
}

/// HTTP OAuth is the Angel host-tool driver. Gated off in ordinary CI.
#[test]
#[ignore = "live Grok account OAuth HTTP tool-calling acceptance"]
fn live_grok_oauth_http_calls_an_offered_host_tool() {
    let _lock = env_lock();
    let _research = EnvGuard::set("ANGEL_GROK_RESEARCH", "0");
    let _effort = EnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "low");
    let clubs = grok_http_clubs();
    let (_, club, _) = clubs
        .into_iter()
        .find(|(alias, _, _)| alias == "grok")
        .expect("HTTP grok seat loads from auth.json even with research off");
    let tools = [ToolDef {
        name: "shell".to_string(),
        description: "Run a shell command".to_string(),
        params: serde_json::json!({
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"]
        }),
    }];
    let reply = club
        .chat(
            &[ChatMsg::user(
                "Call the offered shell tool with command pwd, then stop. Do not answer in prose.",
            )],
            &tools,
        )
        .expect("live HTTP grok tool call");
    match reply {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "shell");
            assert_eq!(
                calls[0].args.get("command").and_then(|v| v.as_str()),
                Some("pwd")
            );
        }
        ClubReply::Text(text) => panic!("expected structured host tool call, got: {text}"),
    }
}

#[test]
fn grok_research_tool_advertises_and_guards_query() {
    use crate::agent::harness::Tool;
    let _lock = env_lock();
    let dir = std::env::temp_dir().join(format!("angel0-grok-tool-auth-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let auth = dir.join("auth.json");
    std::fs::write(&auth, r#"{"entry":{"key":"oauth-token"}}"#).unwrap();
    let _file = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
    let _cmd = EnvGuard::set("ANGEL_GROK_CMD", "/bin/true");
    let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");

    let tool =
        crate::agent::tools::web::GrokResearchTool::from_env().expect("grok tool builds from env");
    assert_eq!(tool.name(), "grok_research");
    assert_eq!(tool.def().name, "grok_research");
    let err = tool
        .call(&serde_json::json!({ "query": "   " }))
        .expect_err("an empty query is rejected before any network call");
    assert!(err.contains("empty"), "{err}");
}
// Intended inside club/grok.rs's existing tests module. This uses the actual
// process fixture and crate-wide env guard; the extracted review probe is separate.
#[cfg(unix)]
#[test]
fn grok_usage_projection_separates_session_setup_and_retains_empty_reply_usage() {
    let _lock = env_lock();
    let _crash = EnvGuard::unset("ANGEL_FAKE_GROK_CRASH_ONCE");
    let _hang = EnvGuard::unset("ANGEL_FAKE_GROK_HANG");
    for case in ["setup-failure", "empty-text", "normal"] {
        let (auth, script, spawns) = fake_grok_acp_fixture(&format!("usage-{case}"));
        let mut fixture = std::fs::read_to_string(&script).unwrap();
        // Record generation dispatch separately from spawn without additional
        // environment variables or an interpolated shell path.
        fixture = fixture.replace(
            "    *'\"method\":\"session/prompt\"'*)",
            "    *'\"method\":\"session/prompt\"'*)\n      printf 'prompt\\n' >> \"${ANGEL_FAKE_GROK_SPAWNS}\"",
        );
        if case == "setup-failure" {
            fixture = fixture.replace(
                "      session_no=$((session_no + 1))",
                "      printf '{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":{}}\\n' \"$id\"\n      continue",
            );
        } else if case == "empty-text" {
            fixture = fixture
                .lines()
                .filter(|line| !line.contains("\"sessionUpdate\""))
                .collect::<Vec<_>>()
                .join("\n");
            fixture.push('\n');
        }
        std::fs::write(&script, fixture).unwrap();
        let _auth = EnvGuard::set("ANGEL_GROK_OAUTH_FILE", auth.to_str().unwrap());
        let _cmd = EnvGuard::set("ANGEL_GROK_CMD", script.to_str().unwrap());
        let _spawn_log = EnvGuard::set("ANGEL_FAKE_GROK_SPAWNS", spawns.to_str().unwrap());
        let _enabled = EnvGuard::set("ANGEL_GROK_RESEARCH", "1");
        let club = GrokResearchClub::from_env().expect("owned fake ACP club");
        let before = club.usage_accounting();
        let result = club.respond("owned accounting fixture");
        let usage = club.usage_accounting().delta(&before);
        let prompts = std::fs::read_to_string(&spawns)
            .unwrap()
            .lines()
            .filter(|line| *line == "prompt")
            .count();
        match case {
            "setup-failure" => {
                assert!(result.unwrap_err().contains("session/new"));
                assert_eq!((prompts, usage.attempts), (0, 0));
                assert!(usage.input.is_none() && usage.output.is_none());
            }
            "empty-text" => {
                assert!(result.unwrap_err().contains("no assistant text"));
                assert_eq!((prompts, usage.attempts), (1, 1));
                assert_eq!((usage.input, usage.output), (Some(10), Some(2)));
            }
            "normal" => {
                assert_eq!(result.unwrap(), "fake-1");
                assert_eq!((prompts, usage.attempts), (1, 1));
                assert_eq!((usage.input, usage.output), (Some(10), Some(2)));
            }
            _ => unreachable!(),
        }
    }
}
#[cfg(unix)]
mod non_http_usage_contract_fixture_tests {
    include!("grok__non_http_usage_contract_fixture_tests.rs");
}
