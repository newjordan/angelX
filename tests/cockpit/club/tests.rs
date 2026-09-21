//! Tests for the club module (moved verbatim from club.rs).
use super::*;

/// Delegates to the crate-wide test env lock (process env is global — a
/// module-local lock can't serialize against other modules' env tests).
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::tests::env_lock()
}

struct ScopedEnv {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl ScopedEnv {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        match self.previous.take() {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(self.key) },
        }
        if self.key.ends_with("REASONING_EFFORT") || self.key.ends_with("REASONING_DIALECT") {
            resync_reasoning_effort_env_from_env();
        }
        if self.key.ends_with("MAX_TOKENS") {
            resync_max_tokens_env_from_env();
        }
        if self.key.ends_with("PROMPT_CACHE") || self.key.ends_with("PROMPT_CACHE_KEY") {
            resync_prompt_cache_from_env();
        }
        if self.key.contains("ANTHROPIC_CACHE") {
            resync_openrouter_anthropic_cache_from_env();
        }
    }
}

/// Every `ANGEL_SOTA_MOA_*` knob the roster/role code reads. Tests that assert
/// DEFAULT routing must scope all of them out — a wrapper that sources the
/// operator's live `.angel.env` (the dogfood runner, an interactive shell)
/// otherwise leaks profile pins into the verdict, and a partial per-test scrub
/// rots silently as knobs are added.
const SOTA_MOA_ENV_KEYS: &[&str] = &[
    "ANGEL_SOTA_MOA_AGG_CLUB",
    "ANGEL_SOTA_MOA_ALLOW_DEGRADED",
    "ANGEL_SOTA_MOA_ALLOW_SINGLE",
    "ANGEL_SOTA_MOA_ALWAYS",
    "ANGEL_SOTA_MOA_CHEAP_LINKS",
    "ANGEL_SOTA_MOA_CITE",
    "ANGEL_SOTA_MOA_COST_PROFILE",
    "ANGEL_SOTA_MOA_DELEGATE",
    "ANGEL_SOTA_MOA_DISSENT_GATE",
    "ANGEL_SOTA_MOA_DRAFT_MAX_CHARS",
    "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
    "ANGEL_SOTA_MOA_GROK_RESEARCH",
    "ANGEL_SOTA_MOA_HEDGE",
    "ANGEL_SOTA_MOA_JUDGE",
    "ANGEL_SOTA_MOA_JUDGE_CLUB",
    "ANGEL_SOTA_MOA_JUDGE_DIMS",
    "ANGEL_SOTA_MOA_JUDGE_PANEL",
    "ANGEL_SOTA_MOA_KEEP",
    "ANGEL_SOTA_MOA_LAYERS",
    "ANGEL_SOTA_MOA_MAX",
    "ANGEL_SOTA_MOA_MAX_PARALLEL",
    "ANGEL_SOTA_MOA_MAX_TESTS",
    "ANGEL_SOTA_MOA_MAX_WAVES",
    "ANGEL_SOTA_MOA_MAX_WIDTH",
    "ANGEL_SOTA_MOA_MODE",
    "ANGEL_SOTA_MOA_PROPOSE_CLUB",
    "ANGEL_SOTA_MOA_REFLECT",
    "ANGEL_SOTA_MOA_RESEARCH",
    "ANGEL_SOTA_MOA_SAMPLES",
    "ANGEL_SOTA_MOA_SEARCH_URL",
    "ANGEL_SOTA_MOA_STRICT_ROUTES",
    "ANGEL_SOTA_MOA_USE_SWARM_KNOBS",
    "ANGEL_SOTA_MOA_VERIFY",
    "ANGEL_SOTA_MOA_VERIFY_CLUB",
    "ANGEL_SOTA_MOA_VERIFY_GUARD",
    "ANGEL_SOTA_MOA_WIDTH",
];

/// Scope out the entire SOTA-MoA env surface (restored on drop, newest-last).
/// Callers must hold `env_lock()` first; set test-specific values AFTER this.
fn scrub_sota_moa_env() -> Vec<ScopedEnv> {
    SOTA_MOA_ENV_KEYS
        .iter()
        .map(|key| ScopedEnv::unset(key))
        .collect()
}

#[test]
fn repair_tool_args_salvages_malformed_calls() {
    let value = |parsed: ToolArgsParse| match parsed {
        ToolArgsParse::Exact { value, .. } | ToolArgsParse::Repaired { value, .. } => value,
        ToolArgsParse::Unrecoverable { .. } => panic!("expected usable arguments"),
    };
    // Valid passes through untouched.
    assert_eq!(
        value(repair_tool_args(r#"{"cmd":"ls"}"#)),
        serde_json::json!({"cmd":"ls"})
    );
    // Truncated mid-string (stream cut off) → string + object closed.
    assert_eq!(
        value(repair_tool_args(r#"{"cmd":"ls -la"#)),
        serde_json::json!({"cmd":"ls -la"})
    );
    // Truncated nested array.
    assert_eq!(
        value(repair_tool_args(r#"{"path":"src","lines":[1,2"#)),
        serde_json::json!({"path":"src","lines":[1,2]})
    );
    // Trailing comma.
    assert_eq!(
        value(repair_tool_args(r#"{"a":1,}"#)),
        serde_json::json!({"a":1})
    );
    // ```json fenced.
    assert_eq!(
        value(repair_tool_args("```json\n{\"a\":1}\n```")),
        serde_json::json!({"a":1})
    );
    // Genuinely empty arguments are exact; malformed prose is never `{}`.
    assert!(matches!(
        repair_tool_args(""),
        ToolArgsParse::Exact { value, .. } if value == serde_json::json!({})
    ));
    assert!(matches!(
        repair_tool_args("not json at all"),
        ToolArgsParse::Unrecoverable { length: 15, .. }
    ));
    // A brace inside a string must not be mistaken for structure.
    assert_eq!(
        value(repair_tool_args(r#"{"msg":"a } b"#)),
        serde_json::json!({"msg":"a } b"})
    );
}

#[test]
fn extract_prose_tool_calls_recovers_only_explicit_wrappers() {
    // <tool_call> with inline-object arguments.
    let c = extract_prose_tool_calls(
        r#"sure: <tool_call>{"name":"grep","arguments":{"pattern":"x"}}</tool_call>"#,
    );
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "grep");
    assert_eq!(c[0].args, serde_json::json!({"pattern":"x"}));

    // arguments as a JSON string (repaired) + truncated/unterminated wrapper.
    let c = extract_prose_tool_calls(r#"<tool_call>{"name":"shell","arguments":{"cmd":"ls"#);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "shell");
    assert_eq!(c[0].args, serde_json::json!({"cmd":"ls"}));

    // XML-ish name/args wrapper emitted as content by some providers.
    let c = extract_prose_tool_calls(
        r#"<tool_call>
<name>shell</name>
<args>{"cmd":"cat /workspace/fixture-repo/README.md 2>/dev/null || echo NO_README_FOUND"}</args>
</tool_call>"#,
    );
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "shell");
    assert_eq!(
        c[0].args["cmd"],
        "cat /workspace/fixture-repo/README.md 2>/dev/null || echo NO_README_FOUND"
    );

    // Name carried as an attribute on the open tag, bare args object as the
    // body, prose around the wrapper (dialect seen live from a SOTA model).
    let c = extract_prose_tool_calls(
        "On it.\n<tool_call name=\"shell\">\n{\"command\": \"pwd && ls -lah\", \"with_approval\": false}\n</tool_call>\nRunning now.",
    );
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "shell");
    assert_eq!(c[0].args["command"], "pwd && ls -lah");
    assert_eq!(c[0].args["with_approval"], false);

    // Single-quoted attribute + truncated body (stream cut mid-args).
    let c = extract_prose_tool_calls(r#"<tool_call name='shell'>{"cmd":"ls"#);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "shell");
    assert_eq!(c[0].args, serde_json::json!({"cmd":"ls"}));

    // Longer tag names must not open a wrapper, and an attribute form with
    // an empty name (and no name in the body) stays inert.
    assert!(extract_prose_tool_calls("the <tool_calls> field was empty").is_empty());
    assert!(extract_prose_tool_calls(r#"<tool_call name="">{"cmd":"ls"}</tool_call>"#).is_empty());

    // [TOOL_CALLS] array form.
    let c = extract_prose_tool_calls(r#"[TOOL_CALLS][{"name":"todo","arguments":{}}]"#);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "todo");

    // Shouted-tag dialect, verbatim from a live session: no closing tag,
    // hallucinated prose right after the args object.
    let c = extract_prose_tool_calls(
        r#"<SHELL>{"cmd": "pwd && ls -la | sed -n '1,80p' && (git status --short 2>/dev/null || true)", "timeout": 10000}Status report:

- Workspace: `/workspace/fixture-repo`"#,
    );
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "shell");
    assert_eq!(c[0].args["timeout"], 10000);
    assert!(
        c[0].args["cmd"]
            .as_str()
            .unwrap()
            .starts_with("pwd && ls -la")
    );

    // Several shouted tags in one message; truncated last one repairs.
    let c = extract_prose_tool_calls(
        r#"<SHELL>{"cmd":"a"}</SHELL> then <DELEGATE>{"club":"coder","task":"x"#,
    );
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].name, "shell");
    assert_eq!(c[1].name, "delegate");
    assert_eq!(c[1].args["club"], "coder");

    // TitleCase / short / brace-less tags are genuine markup, not calls.
    assert!(extract_prose_tool_calls(r#"<Body>{"a":1}</Body>"#).is_empty());
    assert!(extract_prose_tool_calls(r#"<B>{"a":1}</B>"#).is_empty());
    assert!(extract_prose_tool_calls("replace <TOKEN> with your key").is_empty());
    assert!(extract_prose_tool_calls("use Vec<STRING> where a < b").is_empty());

    // A genuine prose answer must NOT be hijacked (no wrapper → nothing).
    assert!(
        extract_prose_tool_calls(
            "Here's the plan: I'll call grep with arguments to find the name."
        )
        .is_empty()
    );
    assert!(extract_prose_tool_calls("").is_empty());
}

fn scavenge_tools() -> Vec<ToolDef> {
    ["shell", "read_file"]
        .into_iter()
        .map(|name| ToolDef {
            name: name.to_string(),
            description: String::new(),
            params: serde_json::json!({"type":"object"}),
        })
        .collect()
}

#[test]
fn scavenge_recovers_a_wrapperless_call_that_names_an_offered_tool() {
    let tools = scavenge_tools();

    // The bare failure mode: prose around a fully-formed call object.
    let c = scavenge_stranded_tool_calls(
        "I'll check the tree.\n{\"name\": \"shell\", \"arguments\": {\"cmd\": \"ls -la\"}}\nThen I'll read the manifest.",
        &tools,
    );
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "shell");
    assert_eq!(c[0].args, serde_json::json!({"cmd":"ls -la"}));

    // Fenced, `tool`/`parameters` keyed, and stringified arguments (the wire shape).
    let c = scavenge_stranded_tool_calls(
        "```json\n{\"tool\": \"read_file\", \"parameters\": \"{\\\"path\\\":\\\"Cargo.toml\\\"}\"}\n```",
        &tools,
    );
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "read_file");
    assert_eq!(c[0].args, serde_json::json!({"path":"Cargo.toml"}));

    // A call nested inside an envelope object is still reachable.
    let c = scavenge_stranded_tool_calls(
        r#"{"tool_call":{"name":"shell","arguments":{"cmd":"pwd"}}}"#,
        &tools,
    );
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].name, "shell");
}

#[test]
fn scavenge_refuses_anything_that_is_not_an_exact_call_shape() {
    let tools = scavenge_tools();

    // Malformed JSON, an unoffered name, non-object arguments, and a missing
    // arguments key are all prose.
    for text in [
        r#"{"name":"shell","arguments":{"cmd":"ls""#,
        r#"{"name":"rm_rf","arguments":{"path":"/"}}"#,
        r#"{"name":"shell","arguments":"ls -la"}"#,
        r#"{"name":"shell","arguments":["ls"]}"#,
        r#"{"name":"shell"}"#,
        "call shell with {\"cmd\": \"ls\"} to list the tree",
        "The `arguments` object is documented above.",
        "",
    ] {
        assert!(
            scavenge_stranded_tool_calls(text, &tools).is_empty(),
            "must stay prose: {text}"
        );
    }

    // No tools offered → nothing the text "calls" could ever execute.
    assert!(
        scavenge_stranded_tool_calls(r#"{"name":"shell","arguments":{"cmd":"ls"}}"#, &[])
            .is_empty()
    );
}

#[test]
fn sota_label_covers_metered_provider_names() {
    for label in [
        "meta",
        "muse-spark-1.3",
        "sota-moa",
        "openai",
        "codex-run",
        "codex",
        "kimi-k3",
        "moonshot",
        "LongCat-2.0",
        "deepseek-v4-pro",
        "cerebras",
        "glm-5.2",
        "glm-5.3",
        "glm-5.3-flash",
        "grok-research",
        "xai",
        "hy",
        "openrouter",
        "tencent/hy3:free",
        "tencent/hy3-preview:free",
        "openrouter/free",
        "poolside/laguna-s-2.1:free",
        "cohere/north-mini-code:free",
        "z-ai/glm-5.2:free",
        "stealth/ox-alpha",
        "thinkingmachines/inkling:free",
        "thinkingmachines/inkling-small:free",
        "longcat",
        "mathgod",
        "math-god",
    ] {
        assert!(is_sota_label(label), "{label} should be treated as SOTA");
    }
    assert!(!is_sota_label("practice"));
    assert!(!is_sota_label("atlas"));
    assert!(
        !is_sota_label("qwen3.6-27b-mtp-pi-tune"),
        "local Qwen checkpoints must stay local-eligible"
    );
}

#[test]
fn sota_label_match_does_not_allocate_a_lowercase_copy() {
    let src = include_str!("../../../cockpit/src/agent/club/mod.rs");
    let start = src
        .find("pub(crate) fn is_sota_label")
        .expect("is_sota_label present");
    let body = &src[start..];
    let end = body
        .find("\npub(crate) fn")
        .or_else(|| body.find("\n/// "))
        .unwrap_or(body.len().min(1200));
    let body = &body[..end];
    assert!(
        !body.contains("to_ascii_lowercase"),
        "Agent bay meters ask this every frame:\n{body}"
    );
}

#[test]
fn mathgod_and_wrapper_labels_match_without_a_lowercase_copy() {
    assert!(is_mathgod_label("Math God"));
    assert!(is_mathgod_label("MATHGOD"));
    assert!(!is_mathgod_label("mathgodly"));
    assert!(is_logical_wrapper_label("SOTA-MOA"));
    assert!(is_logical_wrapper_label("gpu-comp-local"));
    assert!(!is_logical_wrapper_label("spark"));
    let src = include_str!("../../../cockpit/src/agent/club/mod.rs");
    for name in [
        "pub(crate) fn is_mathgod_label",
        "pub(crate) fn is_logical_wrapper_label",
    ] {
        let start = src.find(name).expect(name);
        let body = &src[start..start.saturating_add(500)];
        assert!(
            !body.contains("to_ascii_lowercase"),
            "{name} is on the tab-strip draw path:\n{body}"
        );
    }
}

/// Mirrors the OpenRouter seat in `bag.rs`. `tencent/hy3:free` was retired in
/// July 2026 (the slug 404s); the default breadth route is Ox Alpha.
/// Paid `tencent/hy3` remains a valid explicit pin.
#[test]
fn openrouter_sota_link_defaults_to_breadth_route() {
    const ROUTE: &str = "stealth/union-alpha";
    let _g = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "openrouter");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OPENROUTER_URL") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("OPENROUTER_BASE_URL") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OPENROUTER_MODEL") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("OPENROUTER_MODEL") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OPENROUTER_KEY") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("OPENROUTER_API_KEY") };

    assert!(
        optional_sota_http_club(
            "openrouter",
            ROUTE,
            &["ANGEL_OPENROUTER_URL", "OPENROUTER_BASE_URL"],
            "https://openrouter.ai/api/v1",
            &["ANGEL_OPENROUTER_MODEL", "OPENROUTER_MODEL"],
            Some(ROUTE),
            &["ANGEL_OPENROUTER_KEY", "OPENROUTER_API_KEY"],
        )
        .is_none()
    );

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("OPENROUTER_API_KEY", "test-key") };
    let (_, club, _) = optional_sota_http_club(
        "openrouter",
        ROUTE,
        &["ANGEL_OPENROUTER_URL", "OPENROUTER_BASE_URL"],
        "https://openrouter.ai/api/v1",
        &["ANGEL_OPENROUTER_MODEL", "OPENROUTER_MODEL"],
        Some(ROUTE),
        &["ANGEL_OPENROUTER_KEY", "OPENROUTER_API_KEY"],
    )
    .expect("key plus default model builds OpenRouter club");
    assert_eq!(club.label(), ROUTE);
    // A model-id label must still read as remote, or local-only fallback would
    // adopt the cloud seat.
    assert!(is_sota_label(ROUTE));
    assert_eq!(static_model_window(ROUTE), Some((262_144, false)));
    assert_eq!(resolve_openrouter_model_alias("tencent/hy3:free"), ROUTE);
    assert_eq!(resolve_openrouter_model_alias("union alpha"), ROUTE);
    assert_eq!(resolve_openrouter_model_alias("union-alpha"), ROUTE);
    assert_eq!(
        static_model_window("tencent/hy3:free"),
        Some((262_000, true))
    );
    assert_eq!(
        static_model_window("tencent/hy3-preview:free"),
        Some((262_000, true))
    );
    assert_eq!(
        static_model_window("openrouter/free"),
        Some((200_000, false))
    );
    assert_eq!(
        static_model_window("stealth/ox-alpha"),
        Some((1_000_000, false))
    );
    assert_eq!(
        static_model_window("thinkingmachines/inkling:free"),
        Some((1_000_000, false))
    );
    assert_eq!(
        static_model_window("thinkingmachines/inkling-small:free"),
        Some((1_000_000, false))
    );
    let body = HttpClub::new(
        "openrouter",
        "https://openrouter.ai/api/v1",
        ROUTE,
        Some("test-key".to_string()),
    )
    .build_body(&[ChatMsg::user("hi")], &[], false)
    .expect("OpenRouter default request body builds");
    assert_eq!(body.get("model").and_then(|v| v.as_str()), Some(ROUTE));

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("OPENROUTER_API_KEY") };
}

#[test]
fn openrouter_requires_explicit_provider_and_model_without_a_free_catalog() {
    let _guard = env_lock();
    let _disabled = ScopedEnv::unset("ANGEL_API_CLUBS");
    let _key = ScopedEnv::set("OPENROUTER_API_KEY", "test-key");
    let _model = ScopedEnv::unset("ANGEL_OPENROUTER_MODEL");
    let _fallback_model = ScopedEnv::unset("OPENROUTER_MODEL");

    assert!(!openrouter_configured());
    assert!(optional_openrouter_http_clubs().is_empty());
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "openrouter");
    assert!(!openrouter_configured(), "a model pin is also required");
    assert!(optional_openrouter_http_clubs().is_empty());

    let _pin = ScopedEnv::set("ANGEL_OPENROUTER_MODEL", "operator/pinned-model");
    assert!(openrouter_configured());
    let pinned = optional_openrouter_http_clubs();
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].0, "openrouter");
    assert_eq!(
        pinned[0].1.model_identity().as_deref(),
        Some("operator/pinned-model")
    );
}

#[test]
fn api_club_policy_defaults_to_credentials_and_supports_explicit_scopes() {
    let _guard = env_lock();
    let _disabled = ScopedEnv::unset("ANGEL_API_CLUBS");
    let _key = ScopedEnv::set("TEST_CLUB_POLICY_KEY", "test-key");
    let build = |alias| {
        optional_sota_http_club(
            alias,
            alias,
            &[],
            "http://127.0.0.1:1/v1",
            &[],
            Some("test-model"),
            &["TEST_CLUB_POLICY_KEY"],
        )
    };
    assert!(build("meta").is_some());
    let _disabled_choice = ScopedEnv::set("ANGEL_API_CLUBS", "");
    assert!(build("meta").is_none());
    let _none_choice = ScopedEnv::set("ANGEL_API_CLUBS", "none");
    assert!(build("meta").is_none());
    let _wildcard = ScopedEnv::set("ANGEL_API_CLUBS", "*");
    assert!(build("meta").is_some());
    let _all = ScopedEnv::set("ANGEL_API_CLUBS", "all");
    assert!(build("meta").is_some());
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", " glm , deepseek ");
    assert!(build("glm").is_some());
    assert!(build("glm-5.3-flash").is_some());
    assert!(build("deepseek-flash").is_some());
    assert!(build("grok-api").is_none());
    assert!(build("openai-api").is_none());
    assert!(build("meta").is_none());
    let _grok = ScopedEnv::set("ANGEL_API_CLUBS", "grok");
    assert!(build("grok-api").is_some());
    let _openai = ScopedEnv::set("ANGEL_API_CLUBS", "openai");
    assert!(build("openai-api").is_some());
}

#[test]
fn openai_api_route_is_separate_and_builds_chat_completions_request() {
    let _guard = env_lock();
    let _scope = ScopedEnv::set("ANGEL_API_CLUBS", "openai");
    let _key = ScopedEnv::set("ANGEL_OPENAI_KEY", "test-key");
    let _model_absent = ScopedEnv::unset("ANGEL_OPENAI_API_MODEL");
    let _model_fallback_absent = ScopedEnv::unset("OPENAI_MODEL");
    assert!(optional_openai_api_http_club().is_none());
    let _model = ScopedEnv::set("ANGEL_OPENAI_API_MODEL", "gpt-test");
    let _url = ScopedEnv::set("ANGEL_OPENAI_API_URL", "http://127.0.0.1:1/v1");
    let (alias, club, _) = optional_openai_api_http_club().expect("API key route configured");
    assert_eq!(alias, "openai-api");
    assert_eq!(club.label(), "openai-api");
    assert_eq!(club.model_identity().as_deref(), Some("gpt-test"));
    let body = HttpClub::new(
        "openai-api",
        "http://127.0.0.1:1/v1",
        "gpt-test",
        Some("test-key".to_string()),
    )
    .build_body(&[ChatMsg::user("offline test")], &[], false)
    .expect("OpenAI-compatible chat body builds");
    assert_eq!(body["model"], "gpt-test");
    assert_eq!(body["messages"][0]["role"], "user");
}

#[test]
fn openai_api_route_sends_auth_tools_and_propagates_tool_calls() {
    use std::io::Write;
    use std::net::TcpListener;

    let _guard = env_lock();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let request = read_http_request(&mut socket);
        assert!(
            request.contains("Authorization: Bearer test-key"),
            "{request}"
        );
        let body = request
            .split_once("\r\n\r\n")
            .map(|(_, body)| serde_json::from_str::<serde_json::Value>(body).unwrap())
            .expect("chat-completions request body");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["tools"][0]["function"]["name"], "shell");
        assert_eq!(body["tools"][0]["function"]["parameters"]["type"], "object");
        let response = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "tool_calls":[{"id":"call-1","type":"function",
                "function":{"name":"shell","arguments":"{\"cmd\":\"pwd\"}"}}]
        },"finish_reason":"tool_calls"}]})
        .to_string();
        let wire = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),
            response
        );
        socket.write_all(wire.as_bytes()).unwrap();
    });

    let _scope = ScopedEnv::set("ANGEL_API_CLUBS", "openai");
    let _key = ScopedEnv::set("ANGEL_OPENAI_KEY", "test-key");
    let _model = ScopedEnv::set("ANGEL_OPENAI_API_MODEL", "gpt-test");
    let _url = ScopedEnv::set("ANGEL_OPENAI_API_URL", &format!("http://{addr}/v1"));
    let (_, club, _) = optional_openai_api_http_club().expect("configured API route");
    let tools = [ToolDef {
        name: "shell".to_string(),
        description: "Run a command".to_string(),
        params: serde_json::json!({"type":"object","properties":{"cmd":{"type":"string"}}}),
    }];
    let reply = club
        .chat(&[ChatMsg::user("inspect")], &tools)
        .expect("offline OpenAI-compatible fixture response");
    server.join().unwrap();
    match reply {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].id, "call-1");
            assert_eq!(calls[0].name, "shell");
            assert_eq!(calls[0].args["cmd"], "pwd");
        }
        other => panic!("expected propagated tool call, got {other:?}"),
    }
}

#[test]
fn kimi_sota_link_defaults_to_moonshot_k3() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "kimi");
    for key in [
        "ANGEL_KIMI_KEY",
        "KIMI_API_KEY",
        "MOONSHOT_API_KEY",
        "ANGEL_KIMI_URL",
        "KIMI_API_URL",
        "MOONSHOT_API_URL",
        "ANGEL_KIMI_MODEL",
        "KIMI_MODEL",
        "MOONSHOT_MODEL",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    assert!(
        optional_sota_http_club(
            "kimi",
            "kimi-k3",
            &["ANGEL_KIMI_URL", "KIMI_API_URL", "MOONSHOT_API_URL"],
            "https://api.moonshot.ai/v1",
            &["ANGEL_KIMI_MODEL", "KIMI_MODEL", "MOONSHOT_MODEL"],
            Some("kimi-k3"),
            &["ANGEL_KIMI_KEY", "KIMI_API_KEY", "MOONSHOT_API_KEY"],
        )
        .is_none()
    );

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("MOONSHOT_API_KEY", "test-key") };
    let (alias, club, _) = optional_sota_http_club(
        "kimi",
        "kimi-k3",
        &["ANGEL_KIMI_URL", "KIMI_API_URL", "MOONSHOT_API_URL"],
        "https://api.moonshot.ai/v1",
        &["ANGEL_KIMI_MODEL", "KIMI_MODEL", "MOONSHOT_MODEL"],
        Some("kimi-k3"),
        &["ANGEL_KIMI_KEY", "KIMI_API_KEY", "MOONSHOT_API_KEY"],
    )
    .expect("Moonshot key should configure the default Kimi link");
    assert_eq!(alias, "kimi");
    assert_eq!(club.label(), "kimi-k3");
    assert_eq!(static_model_window("kimi-k3"), Some((1_048_576, true)));
    // The Kimi Code plan endpoint names the same model `k3` and its K2.7
    // coding routes `kimi-for-coding*`; both must size correctly.
    assert_eq!(static_model_window("k3"), Some((1_048_576, true)));
    assert_eq!(static_model_window("k3-256k"), Some((262_144, true)));
    assert_eq!(
        static_model_window("kimi-for-coding"),
        Some((262_144, true))
    );
    assert_eq!(
        static_model_window("kimi-for-coding-highspeed"),
        Some((262_144, true))
    );
    assert_eq!(static_model_window("qwen3.7-plus"), Some((1_000_000, true)));
    assert_eq!(
        static_model_window("qwen3-coder-next"),
        Some((1_000_000, true))
    );
    assert_eq!(static_model_window("qwen3.8-27b"), Some((262_144, true)));
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("MOONSHOT_API_KEY") };
}

#[test]
fn deepseek_v4_catalog_migrates_retired_ids_before_the_wire() {
    let _guard = env_lock();
    let _dialect = ScopedEnv::unset("ANGEL_DEEPSEEK_REASONING_DIALECT");
    let _effort = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    assert_eq!(
        DEEPSEEK_API_MODEL_OPTIONS,
        &["deepseek-v4-pro", "deepseek-flash"]
    );
    assert_eq!(
        resolve_deepseek_model_alias("deepseek-v4-pro"),
        "deepseek-v4-pro"
    );
    assert_eq!(
        resolve_deepseek_model_alias("deepseek-flash"),
        "deepseek-flash"
    );
    // Retired ids: the two legacy chat endpoints plus the superseded V4 Flash
    // ids. All of them alias the live V4.1 Flash on the wire.
    for retired in [
        "deepseek-chat",
        "deepseek-reasoner",
        "deepseek-v4-flash",
        "deepseek-v4-flash-vision-exp",
    ] {
        let canonical = resolve_deepseek_model_alias(retired);
        assert_eq!(canonical, "deepseek-flash");
        let body = HttpClub::new("deepseek", "https://api.deepseek.com/v1", canonical, None)
            .build_body(&[ChatMsg::user("hi")], &[], false)
            .expect("canonical DeepSeek request body");
        assert_eq!(body["model"], serde_json::json!("deepseek-flash"));
    }
    assert_eq!(
        resolve_deepseek_model_alias("deepseek-v4-flash-dspark"),
        "deepseek-v4-flash-dspark",
        "the Spark-local V4 serve is not the cloud V4.1 Flash"
    );
    assert_eq!(
        resolve_deepseek_model_alias("private/deepseek-next"),
        "private/deepseek-next",
        "unknown explicit gateway ids remain forward-compatible"
    );
    // 1M context + always-on cache for both V4 seats, current id and aliases.
    for model in [
        "deepseek-v4-pro",
        "deepseek-flash",
        "deepseek-v4-flash",
        "deepseek-v4-flash-vision-exp",
    ] {
        assert_eq!(
            static_model_window(model),
            Some((1_000_000, true)),
            "{model}"
        );
    }
    assert_eq!(static_model_window("deepseek-v4-flash-dspark"), None);
    // Id-scoped capability facts; the *route* decides whether it may claim them.
    assert_eq!(
        static_model_input_modalities("deepseek-flash"),
        Some(&["text", "image"][..])
    );
    assert_eq!(
        static_model_input_modalities("deepseek-v4-pro"),
        Some(&["text"][..])
    );
    assert_eq!(
        static_model_input_modalities("deepseek-v4-flash-dspark"),
        None
    );
    let pro = HttpClub::new(
        "deepseek",
        "https://api.deepseek.com/v1",
        "deepseek-v4-pro",
        None,
    );
    assert_eq!(pro.reasoning_levels(), vec!["none", "low", "high", "max"]);
    assert!(pro.set_reasoning_effort("xhigh").is_none());
    assert!(pro.set_reasoning_effort("minimal").is_none());
    assert_eq!(pro.set_reasoning_effort("low").as_deref(), Some("low"));
    assert_eq!(pro.set_reasoning_effort("MAX").as_deref(), Some("max"));
    let body = pro
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("DeepSeek V4 max-effort request body");
    assert_eq!(body["reasoning_effort"], serde_json::json!("max"));
    assert_eq!(body["thinking"]["type"], "enabled");
    assert!(
        body.get("max_tokens").is_none(),
        "an interactive DeepSeek hop must not invent an output cap"
    );
}

/// The THINK control on the current provider contract: `none` is the documented
/// disable toggle, low/high/max are depth rungs, and an unpinned control leaves
/// the provider's own default (thinking on at `high`) untouched.
#[test]
fn deepseek_thinking_control_encodes_the_live_provider_surface() {
    let _guard = env_lock();
    let _effort = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    let _pin = ScopedEnv::unset("ANGEL_DEEPSEEK_FLASH_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new(
        "deepseek-flash",
        "https://api.deepseek.com/v1",
        "deepseek-flash",
        None,
    );
    assert_eq!(club.reasoning_levels(), vec!["none", "low", "high", "max"]);
    for (requested, expected) in [("low", "low"), ("HIGH", "high"), ("max", "max")] {
        assert_eq!(
            club.set_reasoning_effort(requested).as_deref(),
            Some(expected),
            "{requested}"
        );
        let body = club
            .build_body(&[ChatMsg::user("hi")], &[], false)
            .expect("DeepSeek body");
        assert_eq!(body["thinking"]["type"], "enabled", "{requested}");
        assert_eq!(
            body["reasoning_effort"],
            serde_json::json!(expected),
            "{requested}"
        );
        assert!(body.get("max_tokens").is_none(), "{requested}");
    }
    assert_eq!(club.set_reasoning_effort("none").as_deref(), Some("none"));
    let off = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("DeepSeek body");
    assert_eq!(off["thinking"]["type"], "disabled");
    assert!(
        off.get("reasoning_effort").is_none(),
        "`none` is a thinking toggle, not a reasoning_effort rung"
    );
    assert!(off.get("max_tokens").is_none());
    // Unpinned: neither control field is sent, so the provider's own default
    // stands and we never imitate it with an output-token cap.
    let plain = HttpClub::new(
        "deepseek-flash",
        "https://api.deepseek.com/v1",
        "deepseek-flash",
        None,
    );
    let unpinned = plain
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("DeepSeek body");
    assert!(unpinned.get("thinking").is_none());
    assert!(unpinned.get("reasoning_effort").is_none());
    assert!(unpinned.get("max_tokens").is_none());
}

/// The seat root benchmarks with: the operator's configured DeepSeek route,
/// pointed at a loopback observer/forwarder that relays solely to the official
/// API. The provider contract must survive that URL, so opaque continuation
/// state still reaches the real model instead of a stripped adapter path.
#[test]
fn configured_deepseek_seat_keeps_its_provider_contract_behind_a_forwarder() {
    // Locate the tool-call turn by role: a SOTA-tuned body may splice a brevity
    // system message in front, so indices are not stable.
    fn reasoning_on_wire(body: &serde_json::Value) -> Option<String> {
        body["messages"]
            .as_array()?
            .iter()
            .find(|message| message["role"] == "assistant")
            .and_then(|message| message["reasoning_content"].as_str())
            .map(str::to_string)
    }
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "deepseek");
    let _key = ScopedEnv::set("ANGEL_DEEPSEEK_KEY", "test-key");
    let _pin = ScopedEnv::unset("ANGEL_DEEPSEEK_FLASH_MODEL");
    let _pin_alt = ScopedEnv::unset("DEEPSEEK_FLASH_MODEL");
    let _url = ScopedEnv::set("ANGEL_DEEPSEEK_URL", "http://127.0.0.1:8799/v4");
    let (alias, club, _) = optional_sota_http_club(
        "deepseek-flash",
        "deepseek-flash",
        &["ANGEL_DEEPSEEK_URL"],
        "https://api.deepseek.com/v1",
        &["ANGEL_DEEPSEEK_FLASH_MODEL", "DEEPSEEK_FLASH_MODEL"],
        Some("deepseek-flash"),
        &["ANGEL_DEEPSEEK_KEY", "DEEPSEEK_API_KEY"],
    )
    .expect("configured DeepSeek Flash seat");
    assert_eq!(alias, "deepseek-flash");
    assert_eq!(club.label(), "deepseek-flash");
    assert_eq!(club.model_identity().as_deref(), Some("deepseek-flash"));
    assert_eq!(
        club.route_metadata().input_modalities,
        ["text", "image"],
        "the configured route keeps the provider's declared capability"
    );
    assert_eq!(club.reasoning_levels(), vec!["none", "low", "high", "max"]);

    let calls = vec![ToolCall {
        id: "angel_call_1_0".to_string(),
        name: "read".to_string(),
        args: serde_json::json!({"path": "src/main.rs"}),
    }];
    let assistant = ChatMsg::assistant_calls_with_reasoning(
        calls.clone(),
        Some("tool-round-private-reasoning".to_string()),
    );
    let history = [
        ChatMsg::user("inspect"),
        assistant,
        ChatMsg::tool(&calls[0].id, "source"),
    ];
    // Body-level replay needs the concrete club; the seat constructor applies
    // exactly this contract (the seat's own declared `text+image` above proves
    // it, since a loopback URL with no contract declares nothing).
    let body_club = HttpClub::new(
        "deepseek-flash",
        "http://127.0.0.1:8799/v4",
        "deepseek-flash",
        None,
    )
    .with_provider_contract(ProviderContract::DeepSeek);
    let replayed = body_club
        .build_body(&history, &[], false)
        .expect("seat body");
    assert_eq!(
        reasoning_on_wire(&replayed).as_deref(),
        Some("tool-round-private-reasoning"),
        "the configured DeepSeek route replays its own opaque state"
    );
    // Provider/model boundary: the same history on a foreign endpoint, and on a
    // non-DeepSeek model, must never carry it.
    let foreign = HttpClub::new(
        "deepseek-flash",
        "https://gateway.example/v1",
        "deepseek-flash",
        None,
    );
    assert!(
        foreign.route_metadata().input_modalities.is_empty(),
        "an unnamed gateway declares nothing instead of the cloud model's capability"
    );
    assert_eq!(
        reasoning_on_wire(&foreign.build_body(&history, &[], false).unwrap()),
        None
    );
    let mismatched = HttpClub::new(
        "deepseek-flash",
        "https://api.deepseek.com/v1",
        "deepseek-r1-distill-32b",
        None,
    );
    assert_eq!(
        reasoning_on_wire(&mismatched.build_body(&history, &[], false).unwrap()),
        None
    );
}

/// A local or fleet serve reusing a provider id is a different checkpoint: it
/// declares nothing (so the vision gate keeps its own text-only identity), never
/// replays provider-private reasoning, and is never sent the provider's
/// `thinking` toggle.
#[test]
fn local_deepseek_serve_reusing_a_provider_id_is_not_the_provider() {
    let _guard = env_lock();
    let local = HttpClub::new(
        "dsflash",
        "http://127.0.0.1:18888/v1",
        "deepseek-v4-flash",
        None,
    );
    assert!(local.route_metadata().input_modalities.is_empty());
    assert_eq!(local.reasoning_levels(), vec!["none", "low", "high", "max"]);
    assert_eq!(local.set_reasoning_effort("none").as_deref(), Some("none"));
    let body = local
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("local body");
    assert_eq!(body["reasoning_effort"], serde_json::json!("none"));
    assert!(
        body.get("thinking").is_none(),
        "the provider's thinking toggle belongs to the provider's route only"
    );
}

#[test]
fn deepseek_sota_builder_canonicalizes_legacy_env_model_without_network() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "deepseek");
    let _key = ScopedEnv::set("ANGEL_DEEPSEEK_KEY", "test-key");
    let _model = ScopedEnv::set("ANGEL_DEEPSEEK_MODEL", "deepseek-chat");
    let (_, club, _) = optional_sota_http_club(
        "deepseek",
        "deepseek-v4-pro",
        &["ANGEL_DEEPSEEK_URL"],
        "https://api.deepseek.com/v1",
        &["ANGEL_DEEPSEEK_MODEL", "DEEPSEEK_MODEL"],
        Some("deepseek-v4-pro"),
        &["ANGEL_DEEPSEEK_KEY", "DEEPSEEK_API_KEY"],
    )
    .expect("configured DeepSeek seat");
    assert_eq!(club.model_identity().as_deref(), Some("deepseek-flash"));
    assert_eq!(club.label(), "deepseek-flash");
}

#[test]
fn grok_46_http_metadata_and_xhigh_are_model_scoped() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    let _grok = ScopedEnv::unset("ANGEL_GROK_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    assert_eq!(static_model_window("grok-4.6"), Some((500_000, false)));

    let current = HttpClub::new("grok", "https://api.x.ai/v1", "grok-4.6", None);
    assert_eq!(
        current.reasoning_levels(),
        vec!["low", "medium", "high", "xhigh"]
    );
    assert_eq!(
        current.set_reasoning_effort("XHIGH").as_deref(),
        Some("xhigh")
    );
    let body = current
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("Grok 4.6 request body");
    assert_eq!(body["model"], serde_json::json!("grok-4.6"));
    assert_eq!(body["reasoning_effort"], serde_json::json!("xhigh"));

    let older = HttpClub::new("grok-legacy", "https://api.x.ai/v1", "grok-4.5", None);
    assert!(
        !older
            .reasoning_levels()
            .iter()
            .any(|level| level == "xhigh")
    );
    assert!(older.set_reasoning_effort("xhigh").is_none());
}

/// Mirrors the Qwen seat in `bag.rs`: Alibaba's coding-plan endpoint, absent
/// until a plan key is present.
#[test]
fn qwen_sota_link_defaults_to_the_coding_plan_endpoint() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "qwen");
    for key in [
        "ANGEL_QWEN_KEY",
        "QWEN_API_KEY",
        "DASHSCOPE_API_KEY",
        "ANGEL_QWEN_URL",
        "QWEN_API_URL",
        "DASHSCOPE_API_URL",
        "ANGEL_QWEN_MODEL",
        "QWEN_MODEL",
        "DASHSCOPE_MODEL",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    let build = || {
        optional_sota_http_club(
            "qwen",
            "qwen3.7-plus",
            &["ANGEL_QWEN_URL", "QWEN_API_URL", "DASHSCOPE_API_URL"],
            "https://coding-intl.dashscope.aliyuncs.com/v1",
            &["ANGEL_QWEN_MODEL", "QWEN_MODEL", "DASHSCOPE_MODEL"],
            Some("qwen3.7-plus"),
            &["ANGEL_QWEN_KEY", "QWEN_API_KEY", "DASHSCOPE_API_KEY"],
        )
    };
    assert!(build().is_none(), "no key means no seat");

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_QWEN_KEY", "sk-sp-test") };
    let (alias, club, _) = build().expect("a plan key configures the Qwen link");
    assert_eq!(alias, "qwen");
    assert_eq!(club.label(), "qwen3.7-plus");
    // Remote seat: local-only fallback must not adopt it. Local Qwen
    // checkpoints keep the opposite answer.
    assert!(is_sota_label("qwen3.7-plus"));
    assert!(is_sota_label("qwen"));
    assert!(!is_sota_label("qwen3.6-27b-mtp-pi-tune"));
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_QWEN_KEY") };
}

#[test]
fn grok_response_parser_reads_responses_api_text_and_citations() {
    let v = serde_json::json!({
        "output": [
            {
                "type": "message",
                "content": [
                    {
                        "type": "output_text",
                        "text": "fresh research summary"
                    }
                ]
            }
        ],
        "citations": ["https://x.ai/news"],
        "usage": {
            "input_tokens": 12,
            "output_tokens": 7,
            "reasoning_tokens": 3
        }
    });
    assert_eq!(
        extract_grok_text(&v).as_deref(),
        Some("fresh research summary")
    );
    assert_eq!(extract_grok_citations(&v), vec!["https://x.ai/news"]);
}

#[test]
fn http_club_records_openai_compatible_usage() {
    let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "model", None);
    assert_eq!(club.token_usage(), None);
    club.record_usage_json(&serde_json::json!({
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 5,
            "completion_tokens_details": { "reasoning_tokens": 3 },
            "prompt_tokens_details": { "cached_tokens": 7 },
            "cache_creation_input_tokens": 4
        }
    }));
    assert_eq!(
        club.token_usage(),
        Some(TokenUsage {
            turns: 1,
            last_input: 12,
            last_output: 5,
            last_reasoning: 3,
            total_input: 12,
            total_output: 5,
            total_reasoning: 3,
        })
    );
    assert_eq!(
        club.cache_usage(),
        CacheUsage {
            control_requests: 0,
            read_input_tokens: 7,
            write_input_tokens: 4,
            read_accounting_responses: 1,
            write_accounting_responses: 1,
        }
    );
}

/// DeepSeek's native API reports its prefix-cache split at the top level rather
/// than in an OpenAI `prompt_tokens_details` block. The hit half is a cache read
/// in the meter's accounting (it is already inside `prompt_tokens`), so a
/// DeepSeek turn shows a real hit rate instead of "n/a".
#[test]
fn http_club_records_deepseek_native_prompt_cache_usage() {
    let club = HttpClub::new("deepseek", "https://api.deepseek.com/v1", "model", None);
    club.record_usage_json(&serde_json::json!({
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 5,
            "prompt_cache_hit_tokens": 75,
            "prompt_cache_miss_tokens": 25
        }
    }));
    assert_eq!(
        club.cache_usage(),
        CacheUsage {
            control_requests: 0,
            read_input_tokens: 75,
            write_input_tokens: 0,
            read_accounting_responses: 1,
            write_accounting_responses: 0,
        }
    );
    assert_eq!(
        crate::agent::turn::cache_hit_pct(club.cache_usage().read_input_tokens, 100),
        Some(75),
        "hits are counted inside prompt_tokens, so the reported input is the denominator"
    );
}

#[test]
fn http_club_exposes_exact_pinned_model_without_renaming_mode_tab() {
    let club = HttpClub::new("deepseek", "http://127.0.0.1:9/v1", "deepseek-v4-pro", None);
    assert_eq!(club.live_model_name(), None);
    assert_eq!(club.model_identity().as_deref(), Some("deepseek-v4-pro"));
    assert_eq!(club.label(), "deepseek");
}

#[test]
fn sota_moa_requires_multiple_links_by_default() {
    let _guard = env_lock();
    let _scrub = scrub_sota_moa_env();
    let one: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![(
        "deepseek".to_string(),
        Arc::new(PracticeClub {
            latency: Duration::ZERO,
        }),
        Arc::new(AtomicBool::new(true)),
    )];
    assert!(sota_moa_club(&[]).is_none());
    assert!(sota_moa_club(&one).is_none());

    let two: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "deepseek".to_string(),
            Arc::new(PracticeClub {
                latency: Duration::ZERO,
            }),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "longcat".to_string(),
            Arc::new(PracticeClub {
                latency: Duration::ZERO,
            }),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let moa = sota_moa_club(&two).expect("two configured links build a SOTA-MOA club");
    assert_eq!(moa.label(), "sota-moa");
    assert!(moa.reports_to_palace());
}

#[test]
fn tailnet_label_takes_first_dns_segment_lowercased() {
    assert_eq!(
        tailnet_label("Atlas-1.tail-example.ts.net.").as_deref(),
        Some("atlas-1")
    );
    assert_eq!(
        tailnet_label("compute-a.tail-example.ts.net.").as_deref(),
        Some("compute-a")
    );
    assert_eq!(tailnet_label("").as_deref(), None);
}

#[test]
fn parse_tailnet_disambiguates_by_dns_name() {
    // Two boxes both report HostName "Atlas"; the DNSName label is what's
    // unique (atlas vs atlas-1). Online is honored; Self defaults to online.
    let v = serde_json::json!({
        "Self": {
            "HostName": "Apollo",
            "DNSName": "apollo.tail.ts.net.",
            "TailscaleIPs": ["127.0.0.1", "fd7a::1"]
        },
        "Peer": {
            "k1": {
                "HostName": "Atlas",
                "DNSName": "atlas-1.tail.ts.net.",
                "TailscaleIPs": ["100.64.0.2", "fd7a::2"],
                "Online": true
            },
            "k2": {
                "HostName": "Atlas",
                "DNSName": "atlas.tail.ts.net.",
                "TailscaleIPs": ["100.64.0.5", "fd7a::3"],
                "Online": false
            },
            "k3": {
                "HostName": "compute-a",
                "DNSName": "compute-a.tail.ts.net.",
                "TailscaleIPs": ["100.64.0.3"],
                "Online": true
            }
        }
    });
    let m = parse_tailnet(&v);
    assert_eq!(m.get("atlas-1").map(|h| h.ip.as_str()), Some("100.64.0.2"));
    assert_eq!(m.get("atlas").map(|h| h.ip.as_str()), Some("100.64.0.5"));
    assert!(m["atlas-1"].online && !m["atlas"].online);
    assert!(m["apollo"].online, "Self defaults to online");
    // IPv4 picked, not the IPv6 that follows it.
    assert_eq!(m["compute-a"].ip, "100.64.0.3");
}

#[test]
fn url_from_uses_tailnet_then_fallback() {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "compute-a".to_string(),
        TailHost {
            ip: "100.64.0.3".into(),
            online: true,
        },
    );
    assert_eq!(
        url_from("compute-a", 8000, "9.9.9.9", &m),
        "http://100.64.0.3:8000/v1"
    );
    // Unknown host uses the caller-provided fallback.
    assert_eq!(
        url_from("ghost", 8080, "9.9.9.9", &m),
        "http://9.9.9.9:8080/v1"
    );
}

#[test]
fn spark_tailnet_aliases_resolve_the_configured_spark_peer() {
    let _g = env_lock();
    // The peer's tailnet name and Hydra host id come from the operator's env,
    // never from source; mixed case and whitespace are tolerated.
    let _aliases = ScopedEnv::set(
        SPARK_HOST_ALIASES_ENV,
        " Spark-Tailnet-Peer ,spark-hydra-peer,",
    );
    let mut m = std::collections::HashMap::new();
    m.insert(
        "spark-tailnet-peer".to_string(),
        TailHost {
            ip: "127.0.0.1".into(),
            online: true,
        },
    );
    assert_eq!(
        spark_host_aliases(),
        vec!["spark", "spark-tailnet-peer", "spark-hydra-peer"]
    );
    assert_eq!(canonical_fleet_box("spark-tailnet-peer"), "spark");
    assert_eq!(canonical_fleet_box("SPARK-HYDRA-PEER"), "spark");
    assert_eq!(canonical_fleet_box("turbo"), "turbo");
    assert_eq!(
        url_from("spark", 8001, "127.0.0.1", &m),
        "http://127.0.0.1:8001/v1"
    );
    assert!(host_online("spark", &m));
    assert_eq!(
        tailnet_lookup("spark", &m).map(|h| h.ip.as_str()),
        Some("127.0.0.1")
    );
}

#[test]
fn spark_aliases_are_not_hardcoded_without_the_env() {
    let _g = env_lock();
    let _aliases = ScopedEnv::unset(SPARK_HOST_ALIASES_ENV);
    assert_eq!(spark_host_aliases(), vec!["spark"]);
    assert_eq!(canonical_fleet_box(" Spark "), "spark");
    assert_eq!(canonical_fleet_box("spark-hydra-peer"), "spark-hydra-peer");
    let mut m = std::collections::HashMap::new();
    m.insert(
        "spark".to_string(),
        TailHost {
            ip: "100.64.0.9".into(),
            online: true,
        },
    );
    // Without aliases the box still resolves through its own tailnet label.
    assert_eq!(
        url_from("spark", 8001, "127.0.0.1", &m),
        "http://100.64.0.9:8001/v1"
    );
    assert_eq!(
        url_from("spark-hydra-peer", 8001, "127.0.0.1", &m),
        "http://127.0.0.1:8001/v1"
    );
}

#[test]
fn toymaker_is_a_distinct_spark_peer() {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "toymaker".to_string(),
        TailHost {
            ip: "127.0.0.1".into(),
            online: true,
        },
    );
    assert_eq!(canonical_fleet_box("toymaker"), "toymaker");
    assert_eq!(
        url_from("toymaker", 8002, "127.0.0.1", &m),
        "http://127.0.0.1:8002/v1"
    );
    assert!(host_online("toymaker", &m));
}

#[test]
fn resolve_club_url_dsflash_env_pin_wins_over_tailnet() {
    // Local Spark ds4-server is the free `dsflash` seat. Operators pin
    // ANGEL_DSFLASH_URL to the localhost (or tunnel) surface so the bag does
    // not resolve to a different tailnet endpoint.
    let _g = env_lock();
    let _url = ScopedEnv::set("ANGEL_DSFLASH_URL", "http://127.0.0.1:8000/v1");
    let mut m = std::collections::HashMap::new();
    m.insert(
        "compute-a".to_string(),
        TailHost {
            ip: "100.64.0.3".into(),
            online: true,
        },
    );
    assert_eq!(
        resolve_club_url("DSFLASH", "compute-a", 8000, "127.0.0.1", &m),
        "http://127.0.0.1:8000/v1"
    );
    // Without the pin, the slot still resolves to the spark host/port.
    let _clear = ScopedEnv::unset("ANGEL_DSFLASH_URL");
    assert_eq!(
        resolve_club_url("DSFLASH", "compute-a", 8000, "127.0.0.1", &m),
        "http://100.64.0.3:8000/v1"
    );
}

#[test]
fn missing_tailnet_host_uses_the_loopback_fallback() {
    let m = std::collections::HashMap::new();
    assert_eq!(
        url_from("atlas", 8080, "127.0.0.1", &m),
        "http://127.0.0.1:8080/v1"
    );
}

#[test]
#[ignore = "needs the local tailscale daemon (run with --ignored)"]
fn tailnet_hosts_resolves_live_fleet() {
    let m = live_tailnet_hosts();
    assert!(!m.is_empty(), "tailscale returned no hosts");
    // The Spark serves gemma/spark; it should resolve to a 100.x address.
    let spark = m.get("compute-a").expect("compute-a on the tailnet");
    assert!(spark.ip.starts_with("100."), "got {}", spark.ip);
    eprintln!(
        "resolved gemma → {}",
        url_from("compute-a", 8000, "0.0.0.0", &m)
    );
}

#[cfg(unix)]
#[test]
fn tailnet_status_hang_is_bounded_and_fails_closed() {
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "sleep 30 & wait"]);
    let started = Instant::now();

    let hosts = tailnet_hosts_from_command(command, Duration::from_millis(50));

    assert!(hosts.is_empty());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "hung tailnet probe exceeded its fixed failure boundary: {:?}",
        started.elapsed()
    );
}

#[test]
fn host_online_semantics() {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "turbo".to_string(),
        TailHost {
            ip: "100.0.0.1".into(),
            online: false,
        },
    );
    m.insert(
        "compute-a".to_string(),
        TailHost {
            ip: "100.0.0.2".into(),
            online: true,
        },
    );
    assert!(host_online("compute-a", &m));
    assert!(!host_online("turbo", &m), "offline host not auto-selected");
    assert!(
        !host_online("absent", &m),
        "absent-from-tailnet → not selectable"
    );
    // Empty map = tailscale unavailable → assume usable (fallback/env path).
    let empty = std::collections::HashMap::new();
    assert!(host_online("anything", &empty));
}

fn instant_practice() -> PracticeClub {
    PracticeClub {
        latency: Duration::ZERO,
    }
}

struct EchoCoordinatorDriver;

#[test]
fn attribution_wrappers_run_turn_dispatch_actual_answering_seat() {
    let _env = env_lock();
    let _yolo = ScopedEnv::set("ANGEL_YOLO", "0");
    let _skill = ScopedEnv::set("ANGEL_SKILL_HINT", "0");
    let _advisor = ScopedEnv::set("ANGEL_ADVISOR", "0");
    let _verify = ScopedEnv::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _preflight = ScopedEnv::set("ANGEL_SOTA_MOA_TOOL_PREFLIGHT", "0");
    let _mode = ScopedEnv::set("ANGEL_MOA_MODE", "manual");
    struct Driver;
    struct Exhausted;
    impl Club for Exhausted {
        fn label(&self) -> &str {
            "exhausted-fixture"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("quota exhausted: 429 insufficient_quota".into())
        }
    }
    impl Club for Driver {
        fn label(&self) -> &str {
            "actual-fixture-driver"
        }
        fn model_identity(&self) -> Option<String> {
            Some("actual-fixture-model".into())
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok("finished".into())
        }
        fn chat(&self, messages: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if messages.iter().any(|m| m.role == ChatRole::Tool) {
                Ok(ClubReply::Text("finished".into()))
            } else {
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "identity-probe-1".into(),
                    name: "attribution_probe".into(),
                    args: serde_json::json!({}),
                }]))
            }
        }
    }
    struct Probe(Arc<Mutex<Vec<String>>>);
    impl crate::agent::harness::Tool for Probe {
        fn name(&self) -> &str {
            "attribution_probe"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "Offline attribution fixture".into(),
                params: serde_json::json!({"type":"object","properties":{}}),
            }
        }
        fn call(&self, _: &serde_json::Value) -> Result<String, String> {
            assert_eq!(
                crate::agent::harness::run_identity::live_turn().as_deref(),
                Some("fixture-loop-owner")
            );
            let stamped =
                crate::agent::tools::submit_identity::stamp("yukon submit --model copied", None)?
                    .unwrap();
            self.0.lock().unwrap().push(stamped.command);
            Ok("offline attribution observed; no process or network launched".into())
        }
    }
    let driver: Arc<dyn Club> = Arc::new(Driver);
    let wrappers: Vec<Box<dyn Club>> = vec![
        Box::new(GpuCompLocalMoaClub::new(driver.clone())),
        Box::new(crate::agent::swarm::SwarmClub::from_env(
            "swarm",
            driver.clone(),
        )),
        Box::new(
            crate::agent::swarm::SwarmClub::from_env("swarm", Arc::new(Exhausted))
                .with_quota_fallbacks(vec![driver.clone()]),
        ),
        Box::new(GpuCompLocalMoaClub::new(Arc::new(FallbackClub::new(vec![
            Arc::new(FailingCoordinatorDriver),
            driver,
        ])))),
    ];
    for wrapper in wrappers {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let mut registry = crate::agent::harness::ToolRegistry::with_defaults();
        registry.register(Box::new(Probe(observed.clone())));
        let _owner = crate::agent::harness::run_identity::LiveTurnScope::enter(Some(
            "fixture-loop-owner".into(),
        ));
        let mut history = vec![ChatMsg::user("Use attribution_probe once, then finish.")];
        let result = crate::agent::harness::run_turn(
            &*wrapper,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(3),
            &std::sync::mpsc::channel().0,
        );
        assert!(result.is_ok(), "{}: {result:?}", wrapper.label());
        let rows = observed.lock().unwrap();
        assert_eq!(rows.len(), 1, "{}: {history:?}", wrapper.label());
        assert!(
            rows[0].contains("--model 'actual-fixture-model' --harness 'angelX'"),
            "{}",
            rows[0]
        );
    }
}

#[test]
fn attribution_shared_fallback_keeps_concurrent_callers_separate() {
    let _env = env_lock();
    struct Seat(&'static str, bool);
    impl Club for Seat {
        fn label(&self) -> &str {
            self.0
        }
        fn model_identity(&self) -> Option<String> {
            Some(self.0.into())
        }
        fn respond(&self, prompt: &str) -> Result<String, String> {
            if self.1 && prompt.contains("fallback") {
                Err("quota exhausted".into())
            } else {
                Ok("answer".into())
            }
        }
    }
    let club = Arc::new(FallbackClub::new(vec![
        Arc::new(Seat("primary-seat", true)),
        Arc::new(Seat("fallback-seat", false)),
    ]));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        for (prompt, model) in [("primary", "primary-seat"), ("fallback", "fallback-seat")] {
            let club = club.clone();
            let barrier = barrier.clone();
            scope.spawn(move || {
                club.chat(&[ChatMsg::user(prompt)], &[]).unwrap();
                barrier.wait();
                assert_eq!(club.resolved_route_identity().model.as_deref(), Some(model));
                let _live = crate::agent::harness::run_identity::LiveModelScope::enter(
                    club.resolved_route_identity().model,
                );
                assert!(
                    crate::agent::tools::submit_identity::stamp("yukon submit", None)
                        .unwrap()
                        .unwrap()
                        .command
                        .contains(model)
                );
            });
        }
    });
}

impl Club for EchoCoordinatorDriver {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        Ok(prompt.to_string())
    }

    fn label(&self) -> &str {
        "turbo"
    }

    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let system = messages
            .iter()
            .find(|m| m.role == ChatRole::System)
            .map(|m| m.content.as_ref())
            .unwrap_or("");
        let user = messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.as_ref())
            .unwrap_or("");
        Ok(ClubReply::Text(format!("system={system}\nuser={user}")))
    }
}

struct FailingCoordinatorDriver;

impl Club for FailingCoordinatorDriver {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("upstream returned no text and no tool calls".to_string())
    }

    fn label(&self) -> &str {
        "turbo"
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        Err("upstream returned no text and no tool calls".to_string())
    }
}

struct DarkCoordinatorDriver;

impl Club for DarkCoordinatorDriver {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("turbo model not available".to_string())
    }

    fn label(&self) -> &str {
        "turbo"
    }

    fn is_available(&self) -> bool {
        false
    }
}

#[test]
fn practice_echoes_the_prompt() {
    assert!(
        instant_practice()
            .respond("hello angel")
            .unwrap()
            .contains("hello angel")
    );
}

#[test]
fn practice_handles_empty_input() {
    assert_eq!(instant_practice().respond("   ").unwrap(), "…");
}

#[test]
fn practice_reports_its_label() {
    assert_eq!(PracticeClub::new().label(), "practice");
}

#[test]
fn gpu_comp_coordinator_delegates_questions_with_the_mission_contract() {
    let club = GpuCompLocalMoaClub::new(Arc::new(EchoCoordinatorDriver));
    assert_eq!(club.label(), "gpu-comp-local-moa");
    let text = club.respond("what is your primary objective?").unwrap();
    assert!(text.contains("what is your primary objective?"), "{text}");
    assert!(text.contains("primary objective"), "{text}");
    assert!(text.contains("measured experiments"), "{text}");
    assert!(text.contains("/moa gpu"), "{text}");
    assert!(
        text.contains("Do not repeat setup instructions"),
        "coordinator prompt should prevent the static-card loop: {text}"
    );
    assert!(
        text.contains("one configured Turbo coordinator")
            && text.contains("without its execution receipt"),
        "the logical club must identify its actual driver and require dispatch evidence: {text}"
    );
}

#[test]
fn gpu_comp_coordinator_propagates_driver_failure_as_an_error() {
    let club = GpuCompLocalMoaClub::new(Arc::new(FailingCoordinatorDriver));
    let err = club
        .chat(&[ChatMsg::user("continue the overnight run")], &[])
        .expect_err("driver failure must not masquerade as a successful assistant finding");
    assert!(err.contains("Turbo driver is not reachable"), "{err}");
    assert!(err.contains("no text and no tool calls"), "{err}");
}

#[test]
fn gpu_comp_coordinator_is_unavailable_when_turbo_is_down() {
    let club = GpuCompLocalMoaClub::new(Arc::new(DarkCoordinatorDriver));
    assert!(
        !club.is_available(),
        "gpu-comp must not advertise as up when its turbo driver is dark"
    );
}

#[test]
fn in_hand_mode_is_cached_until_the_active_route_changes() {
    let bag = Bag::for_render_test(&[("spark", &[("swarm", true), ("gemma", true)])]);
    let first = bag.in_hand_mode();
    let second = bag.in_hand_mode();
    assert_eq!(first, second);
    let chrome = bag.in_hand_chrome();
    assert_eq!(chrome.mode.as_deref(), first.as_deref());
    assert_eq!(chrome.route, bag.in_hand().route_identity());
    assert_eq!(bag.in_hand_route_identity(), chrome.route);
    assert!(
        std::sync::Arc::ptr_eq(&chrome, &bag.in_hand_chrome()),
        "second call must reuse the same in-hand chrome Arc"
    );
    assert!(
        bag.cached_in_hand_mode
            .lock()
            .expect("in-hand mode cache")
            .is_some(),
        "second call must have filled the FrameChrome cache"
    );
}

#[test]
fn banned_boxes_release_path_is_seed_once() {
    let src = include_str!("../../../cockpit/src/agent/club/bag.rs");
    let start = src
        .find("pub(crate) fn banned_boxes()")
        .expect("banned_boxes present");
    let body = &src[start..];
    let end = body
        .find("\n/// An **agent**")
        .expect("agent docs follow banned_boxes");
    let body = &body[..end];
    assert!(
        body.contains("OnceLock") && body.contains("cfg(not(test))"),
        "release roster must not getenv ANGEL_BANNED_BOXES every call:\n{body}"
    );
}

#[test]
fn banned_boxes_parses_comma_list_and_ignores_empty() {
    let _env = env_lock();
    {
        let _unset = crate::tests::TestEnvGuard::unset("ANGEL_BANNED_BOXES");
        assert!(super::banned_boxes().is_empty());
    }
    {
        let _set = crate::tests::TestEnvGuard::set("ANGEL_BANNED_BOXES", " Spark, ,turbo ");
        let banned = super::banned_boxes();
        assert_eq!(banned.as_ref(), ["spark", "turbo"]);
    }
}

#[test]
fn tabs_are_cached_until_generation_or_availability_changes() {
    let bag = Bag::for_render_test(&[("spark", &[("swarm", true)]), ("turbo", &[("qwen", true)])]);
    let first = bag.tabs();
    let second = bag.tabs();
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "unchanged bag must reuse the tab-strip Arc"
    );
}

#[test]
fn gpu_comp_config_is_a_visible_logical_tab() {
    let bag = Bag::for_render_test(&[
        ("spark", &[("swarm", false)]),
        ("gpu-comp", &[("local-moa", true)]),
        ("practice", &[("practice", true)]),
    ]);
    let tabs = bag.tabs();
    assert!(
        tabs.iter()
            .any(|t| t.label == "gpu-comp" && t.mode.as_deref() == Some("local-moa")),
        "tabs: {tabs:?}"
    );
    assert!(
        !tabs.iter().any(|t| t.label == "practice"),
        "practice remains the hidden local floor: {tabs:?}"
    );
}

#[test]
fn resolve_driver_accepts_gpu_comp_aliases() {
    let available = Arc::new(AtomicBool::new(true));
    let mut agents = vec![Agent {
        name: "gpu-comp".to_string(),
        slots: vec![Slot {
            label: "local-moa".to_string(),
            club: Arc::new(GpuCompLocalMoaClub::new(Arc::new(EchoCoordinatorDriver))),
            available,
        }],
        active: 0,
    }];

    for pref in [
        "gpu",
        "gpu-comp",
        "gpu_comp_local_moa",
        "gpu comp local moa",
        "overnight",
    ] {
        assert_eq!(resolve_driver(&mut agents, pref), Some(0), "pref {pref}");
    }
}

#[test]
fn resolve_driver_accepts_mathgod_aliases() {
    let available = Arc::new(AtomicBool::new(true));
    let sol: Arc<dyn Club> = Arc::new(LabelClub("gpt-5.6-sol"));
    let grok: Arc<dyn Club> = Arc::new(LabelClub("grok"));
    let mut agents = vec![Agent {
        name: "mathgod".to_string(),
        slots: vec![Slot {
            label: "mathgod".to_string(),
            club: Arc::new(crate::agent::swarm::SwarmClub::mathgod(
                sol,
                grok,
                Vec::new(),
            )),
            available,
        }],
        active: 0,
    }];
    for pref in ["math", "math-god", "mathgod", "math_god"] {
        assert_eq!(resolve_driver(&mut agents, pref), Some(0), "pref {pref}");
    }
}

#[test]
fn tab_cycle_and_strip_skip_mathgod_unless_already_in_hand() {
    let mut bag = Bag::for_render_test(&[
        ("openai", &[("gpt-5.6-sol", true)]),
        ("mathgod", &[("mathgod", true)]),
        ("glm", &[("glm-5.3", true)]),
    ]);
    assert_eq!(bag.in_hand_label(), "openai");
    assert!(
        bag.tabs()
            .iter()
            .all(|tab| !tab.label.eq_ignore_ascii_case("mathgod")),
        "mathgod must not sit in the Tab strip waiting to be landed on"
    );
    bag.cycle();
    assert_eq!(
        bag.in_hand_label(),
        "glm",
        "Tab must skip mathgod and land on the next real box"
    );
    bag.cycle();
    assert_eq!(bag.in_hand_label(), "openai");
}

#[test]
fn resolve_driver_accepts_dsflash_aliases() {
    let available = Arc::new(AtomicBool::new(true));
    let mut agents = vec![Agent {
        name: "spark".to_string(),
        slots: vec![
            Slot {
                label: "gemma".to_string(),
                club: Arc::new(LabelClub("gemma")),
                available: Arc::clone(&available),
            },
            Slot {
                label: "dsflash".to_string(),
                club: Arc::new(LabelClub("dsflash")),
                available: Arc::clone(&available),
            },
        ],
        active: 0,
    }];

    for pref in [
        "dsflash",
        "ds4",
        "ds-flash",
        "dsflash-local",
        "spark-flash",
        "deepseek-flash-local",
    ] {
        assert_eq!(resolve_driver(&mut agents, pref), Some(0), "pref {pref}");
        assert_eq!(
            agents[0].active, 1,
            "pref {pref} should land on the dsflash slot"
        );
    }
}

#[test]
fn practice_chat_falls_back_to_respond() {
    let reply = instant_practice()
        .chat(&[ChatMsg::user("ping")], &[])
        .unwrap();
    match reply {
        ClubReply::Text(t) => assert!(t.contains("ping")),
        ClubReply::Calls(_) => panic!("practice swing should never call tools"),
    }
}

#[test]
fn user_message_without_media_serializes_as_string() {
    let msgs = messages_to_json(&[ChatMsg::user("hi")], true);
    assert_eq!(msgs[0]["content"], serde_json::json!("hi"));
}

#[test]
fn user_message_with_image_serializes_as_parts() {
    let media = Media::Image {
        mime: "image/png".into(),
        b64: "AAAA".into(),
    };
    let msgs = messages_to_json(
        &[ChatMsg::user_with_media("what is this", vec![media])],
        true,
    );
    let content = &msgs[0]["content"];
    assert!(content.is_array(), "content should be a parts array");
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image_url");
    assert_eq!(content[1]["image_url"]["url"], "data:image/png;base64,AAAA");
}

#[test]
fn cloned_messages_share_immutable_media_and_tool_call_payloads() {
    let media_message = ChatMsg::user_with_media(
        "inspect",
        vec![Media::Image {
            mime: "image/png".into(),
            b64: "A".repeat(512 * 1024),
        }],
    );
    let media_clone = media_message.clone();
    assert!(std::sync::Arc::ptr_eq(
        &media_message.attachments,
        &media_clone.attachments
    ));
    assert!(std::sync::Arc::ptr_eq(
        &media_message.content,
        &media_clone.content
    ));

    let call_message = ChatMsg::assistant_calls(vec![ToolCall {
        id: "call-1".to_string(),
        name: "write_blob".to_string(),
        args: serde_json::json!({"payload": "B".repeat(512 * 1024)}),
    }]);
    let call_clone = call_message.clone();
    assert!(std::sync::Arc::ptr_eq(
        &call_message.tool_calls,
        &call_clone.tool_calls
    ));
    assert!(std::sync::Arc::ptr_eq(
        &call_message.content,
        &call_clone.content
    ));

    let text_message = ChatMsg::user("C".repeat(512 * 1024));
    let text_clone = text_message.clone();
    assert!(std::sync::Arc::ptr_eq(
        &text_message.content,
        &text_clone.content
    ));

    // The shared representation remains transparent on the persisted wire.
    let encoded = serde_json::to_string(&call_clone).unwrap();
    let decoded: ChatMsg = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.tool_calls.len(), 1);
    assert_eq!(decoded.tool_calls[0].args, call_message.tool_calls[0].args);
    let wire: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert!(
        wire["content"].is_string(),
        "ChatMsg.content must stay a JSON string, got {wire}"
    );
}

/// End-to-end round-trip against a mock serve-mode helper: two transforms ride
/// ONE node process (same pid, incrementing request counter) — the persistent
/// daemon path. Ignored by default (spawns node); run with `--ignored`.
#[test]
#[ignore = "spawns a node mock helper; run with --ignored"]
fn pxpipe_persistent_daemon_reuses_one_helper_process() {
    let _guard = env_lock();
    let script = r#"
let n = 0
let buf = Buffer.alloc(0)
let header = null
for await (const chunk of process.stdin) {
  buf = Buffer.concat([buf, Buffer.from(chunk)])
  for (;;) {
    if (!header) {
      const nl = buf.indexOf(0x0a)
      if (nl < 0) break
      header = JSON.parse(buf.subarray(0, nl).toString('utf8'))
      buf = buf.subarray(nl + 1)
    }
    if (buf.length < header.len) break
    const body = buf.subarray(0, header.len)
    buf = buf.subarray(header.len)
    const out = Buffer.from(`${process.pid}:${++n}:${header.api}:${body.toString('utf8')}`)
    header = null
    process.stdout.write(JSON.stringify({ ok: true, len: out.length }) + '\n')
    process.stdout.write(out)
  }
}
"#;
    let path = std::env::temp_dir().join(format!("angel_pxmock_{}.mjs", std::process::id()));
    std::fs::write(&path, script).unwrap();
    let saved = std::env::var_os("ANGEL_PXPIPE_HELPER");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_PXPIPE_HELPER", &path) };

    let a = run_pxpipe_helper(PxpipeApi::ChatCompletions, "openai", "gpt-5.5", b"hello")
        .expect("first daemon round-trip");
    let b = run_pxpipe_helper(PxpipeApi::Responses, "openai", "gpt-5.5", b"world")
        .expect("second daemon round-trip");
    let a = String::from_utf8(a).unwrap();
    let b = String::from_utf8(b).unwrap();
    let (pid_a, rest_a) = a.split_once(':').expect("pid:...");
    let (pid_b, rest_b) = b.split_once(':').expect("pid:...");
    assert_eq!(rest_a, "1:chat:hello", "{a}");
    assert_eq!(rest_b, "2:responses:world", "{b}");
    assert_eq!(pid_a, pid_b, "both requests must ride one helper process");

    match saved {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_PXPIPE_HELPER", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_PXPIPE_HELPER") },
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn pxpipe_default_scope_is_openai_image_capable_family_only() {
    // Reads the ANGEL_PXPIPE_MODELS default — serialize against the tests
    // that set it, or their set_var lands mid-assert (the old flake).
    let _guard = env_lock();
    assert!(pxpipe_model_allowed("openai", "gpt-5.5", None));
    assert!(pxpipe_model_allowed("gpt-5.5", "gpt-5.5", None));
    assert!(pxpipe_model_allowed("codex", "gpt-5.6", None));
    assert!(pxpipe_model_allowed("openai", "gpt-5.6[fast]", None));
    assert!(!pxpipe_model_allowed("openai", "text-router", None));
    assert!(!pxpipe_model_allowed("codex", "codex-mini-latest", None));
    assert!(!pxpipe_model_allowed("deepseek", "deepseek-v4-pro", None));
    assert!(!pxpipe_model_allowed("longcat", "LongCat-2.0", None));
    assert!(!pxpipe_model_allowed("cerebras", "cerebras", None));
}

#[test]
fn pxpipe_model_scope_respects_explicit_list_and_off() {
    assert!(!pxpipe_model_allowed("openai", "gpt-5.5", Some("off")));
    assert!(pxpipe_model_allowed(
        "longcat",
        "LongCat-2.0",
        Some("longcat")
    ));
    assert!(pxpipe_model_allowed(
        "gpt-5.6-preview",
        "gpt-5.6-preview",
        Some("gpt-5.6")
    ));
    assert!(!pxpipe_model_allowed(
        "deepseek",
        "deepseek-v4-pro",
        Some("gpt-5.6")
    ));
}

#[test]
fn pxpipe_transform_missing_helper_fails_open_unless_required() {
    let _guard = env_lock();
    let saved_pxpipe = std::env::var_os("ANGEL_PXPIPE");
    let saved_helper = std::env::var_os("ANGEL_PXPIPE_HELPER");
    let saved_models = std::env::var_os("ANGEL_PXPIPE_MODELS");
    let saved_required = std::env::var_os("ANGEL_PXPIPE_REQUIRED");

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_PXPIPE", "1") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe {
        std::env::set_var(
            "ANGEL_PXPIPE_HELPER",
            "/definitely/not/pxpipe-transform.mjs",
        )
    };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_PXPIPE_MODELS", "openai") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_PXPIPE_REQUIRED") };

    let body = br#"{"model":"gpt-5.5","messages":[]}"#.to_vec();
    let transformed = maybe_pxpipe_transform(
        PxpipeApi::ChatCompletions,
        "openai",
        "gpt-5.5",
        body.clone(),
    )
    .expect("fail-open returns the original body");
    assert_eq!(transformed, body);

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_PXPIPE_REQUIRED", "1") };
    let err = maybe_pxpipe_transform(
        PxpipeApi::ChatCompletions,
        "openai",
        "gpt-5.5",
        body.clone(),
    )
    .expect_err("required pxpipe should fail closed");
    assert!(err.contains("pxpipe transform failed"), "{err}");

    match saved_pxpipe {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_PXPIPE", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_PXPIPE") },
    }
    match saved_helper {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_PXPIPE_HELPER", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_PXPIPE_HELPER") },
    }
    match saved_models {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_PXPIPE_MODELS", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_PXPIPE_MODELS") },
    }
    match saved_required {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_PXPIPE_REQUIRED", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_PXPIPE_REQUIRED") },
    }
}

#[cfg(unix)]
#[test]
fn pxpipe_oneshot_hung_helper_is_killed_at_the_request_deadline() {
    let _guard = env_lock();
    let path = std::env::temp_dir().join(format!(
        "angel-pxpipe-hang-{}-{}.sh",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::write(&path, "#!/bin/sh\nsleep 30 & wait\n").unwrap();
    let _persistent = ScopedEnv::set("ANGEL_PXPIPE_PERSISTENT", "0");
    let _node = ScopedEnv::set("ANGEL_PXPIPE_NODE", "/bin/sh");
    let _helper = ScopedEnv::set("ANGEL_PXPIPE_HELPER", path.to_str().unwrap());
    let _timeout = ScopedEnv::set("ANGEL_PXPIPE_TIMEOUT_MS", "50");
    let started = Instant::now();

    let error = run_pxpipe_helper(
        PxpipeApi::Responses,
        "openai",
        "gpt-5.5",
        br#"{"model":"gpt-5.5"}"#,
    )
    .unwrap_err();

    assert!(error.contains("timed out after 50ms"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "pxpipe request remained blocked after its deadline: {:?}",
        started.elapsed()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn sota_tuned_http_body_keeps_native_output_shape_by_default() {
    let _guard = env_lock();
    let saved = std::env::var_os("ANGEL_SOTA_CAVEMAN");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_CAVEMAN") };

    let club = HttpClub::new("sota-caveman-probe", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    let body = club
        .build_body(&[ChatMsg::user("answer this")], &[], false)
        .expect("build SOTA body");
    let messages = body["messages"].as_array().expect("messages array");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "answer this");

    match saved {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_SOTA_CAVEMAN", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_SOTA_CAVEMAN") },
    }
}

#[test]
fn sota_caveman_can_be_enabled_for_http_body() {
    let _guard = env_lock();
    let saved = std::env::var_os("ANGEL_SOTA_CAVEMAN");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SOTA_CAVEMAN", "1") };

    let club = HttpClub::new("sota-caveman-probe", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    let body = club
        .build_body(&[ChatMsg::user("answer this")], &[], false)
        .expect("build SOTA body");
    let messages = body["messages"].as_array().expect("messages array");
    assert_eq!(messages[0]["role"], "system");
    assert!(
        messages[0]["content"]
            .as_str()
            .unwrap_or_default()
            .contains("angelX SOTA brevity mode"),
        "{body}"
    );
    assert_eq!(messages[1]["content"], "answer this");

    match saved {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_SOTA_CAVEMAN", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_SOTA_CAVEMAN") },
    }
}

#[test]
fn image_from_path_reads_and_encodes() {
    let p = std::env::temp_dir().join(format!("angel_img_{}.png", std::process::id()));
    image::RgbImage::from_pixel(1, 1, image::Rgb([0, 0, 0]))
        .save(&p)
        .unwrap();
    match Media::image_from_path(&p).unwrap() {
        Media::Image { mime, b64 } => {
            assert_eq!(mime, "image/png");
            assert!(!b64.is_empty());
        }
        _ => panic!("expected image"),
    }
    let _ = std::fs::remove_file(&p);
}

#[test]
fn reported_model_id_accepts_common_models_shapes() {
    assert_eq!(
        HttpClub::reported_model_id(&serde_json::json!({
            "data": [{ "id": "qwen-live" }]
        }))
        .as_deref(),
        Some("qwen-live")
    );
    assert_eq!(
        HttpClub::reported_model_id(&serde_json::json!({
            "models": [{ "name": "llama-live" }]
        }))
        .as_deref(),
        Some("llama-live")
    );
    assert_eq!(HttpClub::reported_model_id(&serde_json::json!({})), None);
}

/// Tiny multi-request JSON server used to prove cold-cache probes single-flight.
fn serve_counted_json(
    body: &'static str,
) -> (
    String,
    Arc<std::sync::atomic::AtomicUsize>,
    Arc<AtomicBool>,
    std::thread::JoinHandle<()>,
) {
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_server = Arc::clone(&hits);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_server = Arc::clone(&stop);
    let handle = std::thread::spawn(move || {
        while !stop_server.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut sock, _)) => {
                    hits_server.fetch_add(1, Ordering::SeqCst);
                    let mut request = [0u8; 2048];
                    let _ = sock.read(&mut request);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(response.as_bytes());
                    let _ = sock.flush();
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(_) => break,
            }
        }
    });
    (format!("http://{addr}"), hits, stop, handle)
}

#[test]
fn concurrent_model_resolution_is_single_flight() {
    let (base, hits, stop, server) = serve_counted_json(r#"{"data":[{"id":"gemma4"}]}"#);
    let club = Arc::new(HttpClub::new("gemma", base, "", None));
    let barrier = Arc::new(std::sync::Barrier::new(17));
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let club = Arc::clone(&club);
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                let body = club.build_body(&[ChatMsg::user("hi")], &[], false).unwrap();
                assert_eq!(body["model"], "gemma4");
            });
        }
        barrier.wait();
    });
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "a shared cold HttpClub must query /models once, not once per agent"
    );
}

#[test]
fn concurrent_metadata_resolution_is_single_flight() {
    let (base, hits, stop, server) =
        serve_counted_json(r#"{"default_generation_settings":{"n_ctx":32768}}"#);
    let club = Arc::new(HttpClub::new("gemma", format!("{base}/v1"), "gemma4", None));
    let barrier = Arc::new(std::sync::Barrier::new(17));
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let club = Arc::clone(&club);
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(club.metadata().unwrap().context_window, 32768);
            });
        }
        barrier.wait();
    });
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "metadata refresh must not stampede /props under a cold swarm wave"
    );
}

/// LIVE: Bag::standard assembles the fleet and puts the smartest *available*
/// real box in hand (never the practice floor) when any model is up. Skips
/// when the fleet is down.
#[test]
fn live_bag_picks_driver_when_up() {
    let _guard = env_lock();
    let Some(url) = std::env::var("ANGEL_SPARK_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
    else {
        eprintln!("ANGEL_SPARK_URL is absent; skipping explicit live bag contract");
        return;
    };
    let probe = HttpClub::new("spark", url, "qwopus-coder", None);
    if !probe.is_ready() {
        eprintln!("spark down; skipping live bag test");
        return;
    }
    let bag = Bag::standard();
    eprintln!("live bag in_hand = {}", bag.in_hand_label());
    // Pessimistic loading: the brain never STARTS on an unconfirmed/broken box.
    // It's either the always-available practice floor (pending an upgrade once a
    // model confirms) or a model already verified reachable — never a corpse.
    // (The upgrade-off-the-floor path is covered by live_brain_settles_off_a_dead_default.)
    assert!(
        bag.agents[bag.in_hand].available(),
        "the in-hand box must be reachable (never a broken endpoint), got {}",
        bag.in_hand_label()
    );
}

/// LIVE regression for the "hello → Connection refused" bug: a launch landed
/// (optimistically) on a high-scored but DEAD box and hard-failed the first
/// message. After the seed-correction prober + per-tick settle, the brain must
/// drift to a box that's actually reachable, and a real turn through it must NOT
/// return a transport error. Skips unless a local model is up.
#[test]
fn live_brain_settles_off_a_dead_default() {
    // Serialized: this test mutates process-global environment that other
    // tests also read or write. Without the lock they interleave and each
    // observes the other's value between its own set and restore.
    let _guard = crate::tests::env_lock();
    let _driver = ScopedEnv::unset("ANGEL_DRIVER"); // exercise the heuristic, not an override
    let Some(url) = std::env::var("ANGEL_TURBO_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
    else {
        eprintln!("ANGEL_TURBO_URL is absent; skipping explicit live settle contract");
        return;
    };
    let turbo = HttpClub::new("turbo-probe", url, "probe", None);
    if !turbo.is_ready() {
        eprintln!("no local model up; skipping live settle test");
        return;
    }
    let mut bag = Bag::standard();
    eprintln!(
        "initial in_hand = {} (pessimistic: floor until a model loads)",
        bag.in_hand_label()
    );
    // Mirror the run loop: each tick elects the smartest reachable model as the
    // prober confirms boxes up (a broken endpoint never confirms, so it's never
    // picked).
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut settled = None;
    while Instant::now() < deadline {
        bag.settle_brain();
        let club = bag.in_hand();
        if bag.in_hand_label() != "practice" && club.is_available() {
            settled = Some(bag.in_hand_label().to_string());
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let label = settled.unwrap_or_else(|| {
        panic!(
            "brain never settled on a reachable model (stuck on {})",
            bag.in_hand_label()
        )
    });
    eprintln!("brain settled on reachable box: {label}");
    // The real proof: a turn through the settled brain reaches a live backend —
    // no "Connection refused" transport error like the one the user hit.
    match bag.in_hand().respond("Reply with the single word: ok") {
        Ok(reply) => eprintln!("settled brain replied ok ({} chars)", reply.len()),
        Err(e) => panic!("settled brain still errored: {e}"),
    }
}

/// LIVE: turbo chats through the canonical local serving surface.
/// Kept out of the hermetic inventory: a readiness probe can pass immediately
/// before the serving process disappears, and provider retries would then turn
/// ordinary CI into a minute-long tailnet-dependent failure.
#[test]
#[ignore = "requires an explicitly provisioned live Turbo endpoint"]
fn live_turbo_chat_canonical_surface() {
    let url = std::env::var("ANGEL_TURBO_URL")
        .expect("ANGEL_TURBO_URL must explicitly name the live Turbo endpoint");
    let club = HttpClub::new(
        "turbo",
        url,
        std::env::var("ANGEL_TURBO_MODEL").unwrap_or_default(),
        std::env::var("ANGEL_BRAIN_KEY").ok(),
    );
    assert!(
        club.is_ready(),
        "Turbo is unavailable; the explicit canonical live-chat contract did not run"
    );
    let reply = club.respond("Reply with exactly: nex2 online").unwrap();
    eprintln!("turbo reply: {reply}");
    assert!(!reply.trim().is_empty(), "turbo should reply");
}

// --- agent/box bag helpers (no network) ----------------------------------

struct LabelClub(&'static str);

impl Club for LabelClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("ok".to_string())
    }

    fn label(&self) -> &str {
        self.0
    }
}

struct LiveModelClub {
    label: &'static str,
    model: &'static str,
}

impl Club for LiveModelClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("ok".to_string())
    }

    fn label(&self) -> &str {
        self.label
    }

    fn live_model_name(&self) -> Option<String> {
        Some(self.model.to_string())
    }
}

fn slot(label: &str, available: bool) -> Slot {
    Slot {
        label: label.to_string(),
        club: Arc::new(instant_practice()),
        available: Arc::new(AtomicBool::new(available)),
    }
}

/// A follow-backend HttpClub that has already learned a live checkpoint id —
/// fed the `/models` JSON shape directly, no network.
fn learned_club(label: &str, model_id: &str) -> HttpClub {
    let club = HttpClub::new(label, "http://127.0.0.1:1", "", None).follow_backend();
    club.remember_reported_model(&serde_json::json!({"data": [{"id": model_id}]}));
    club
}

/// A single-mode box (a box whose only model has the given availability).
fn solo(name: &str, available: bool) -> Agent {
    Agent {
        name: name.to_string(),
        slots: vec![slot(name, available)],
        active: 0,
    }
}

/// Build a bag from explicit agents, in hand on the first.
fn bag_of(agents: Vec<Agent>) -> Bag {
    Bag {
        agents,
        in_hand: 0,
        discovery_rx: None,
        meta: Vec::new(),
        driver_pref: None,
        pending_brain: false,
        generation: 0,
        cached_route_choices: std::sync::Mutex::new(None),
        cached_tabs: std::sync::Mutex::new(None),
        cached_in_hand_mode: std::sync::Mutex::new(None),
    }
}

/// Convenience: a bag of single-mode boxes with the given availabilities.
fn boxes(avail: &[bool]) -> Bag {
    bag_of(
        avail
            .iter()
            .enumerate()
            .map(|(i, &a)| solo(&format!("box{i}"), a))
            .collect(),
    )
}

#[test]
fn route_choices_reuse_one_snapshot_across_idle_maintenance() {
    let mut bag = Bag::for_reasoning_render_test();
    let first = bag.route_choices();

    // This is the ordinary idle-loop sequence. It must remain allocation-free
    // while discovery, selection, availability, and route state are unchanged.
    for _ in 0..32 {
        bag.drain_discovered();
        bag.settle_brain();
        let current = bag.route_choices();
        assert!(
            Arc::ptr_eq(&first, &current),
            "idle maintenance rebuilt the immutable route snapshot"
        );
    }
}

#[test]
fn route_choices_invalidate_on_availability_and_effort_changes() {
    let bag = Bag::for_reasoning_render_test();
    let initial = bag.route_choices();

    bag.set_route_available_for_test(1, 0, false);
    let unavailable = bag.route_choices();
    assert!(!Arc::ptr_eq(&initial, &unavailable));
    assert!(!unavailable[1].available);
    assert!(Arc::ptr_eq(&unavailable, &bag.route_choices()));

    assert_eq!(bag.set_reasoning_effort("high").as_deref(), Some("high"));
    let high = bag.route_choices();
    assert!(!Arc::ptr_eq(&unavailable, &high));
    assert_eq!(high[0].reasoning_effort.as_deref(), Some("high"));
    assert!(Arc::ptr_eq(&high, &bag.route_choices()));
}

#[test]
fn route_choices_invalidate_when_a_live_checkpoint_changes() {
    let club = Arc::new(learned_club("fleet", "model-old"));
    let bag = bag_of(vec![Agent {
        name: "fleet".to_string(),
        slots: vec![Slot {
            label: "fleet".to_string(),
            club: Arc::clone(&club) as Arc<dyn Club>,
            available: Arc::new(AtomicBool::new(true)),
        }],
        active: 0,
    }]);
    let old = bag.route_choices();
    assert_eq!(old[0].model, "model-old");

    club.remember_reported_model(&serde_json::json!({"data": [{"id": "model-new"}]}));
    let new = bag.route_choices();
    assert!(!Arc::ptr_eq(&old, &new));
    assert_eq!(new[0].model, "model-new");
    assert!(Arc::ptr_eq(&new, &bag.route_choices()));
}

#[test]
fn tab_cycles_boxes() {
    let mut bag = boxes(&[true, true]);
    bag.cycle();
    assert_eq!(bag.in_hand, 1);
    bag.cycle();
    assert_eq!(bag.in_hand, 0);
}

#[test]
fn tab_skips_offline_boxes() {
    // Middle box is fully offline: Tab must hop straight over it.
    let mut bag = boxes(&[true, false, true]);
    bag.cycle();
    assert_eq!(bag.in_hand, 2, "Tab should skip the offline middle box");
    bag.cycle();
    assert_eq!(bag.in_hand, 0, "Tab wraps, still skipping the offline box");
}

#[test]
fn tab_with_no_other_reachable_box_stays_put() {
    let mut bag = boxes(&[true, false, false]);
    bag.cycle();
    assert_eq!(bag.in_hand, 0);
}

#[test]
fn tabs_hide_offline_boxes_but_keep_in_hand() {
    // box0 (in_hand) offline, box1 up, box2 offline. Strip shows the in-hand
    // (flagged down) and the one live box — never the other dead one.
    let bag = boxes(&[false, true, false]);
    let tabs = bag.tabs();
    let seen: Vec<(bool, bool)> = tabs.iter().map(|t| (t.in_hand, t.available)).collect();
    assert_eq!(tabs.len(), 2, "only in-hand + reachable boxes are shown");
    assert_eq!(
        seen[0],
        (true, false),
        "in-hand box shown even while offline"
    );
    assert_eq!(seen[1], (false, true), "the reachable box is shown");
}

#[test]
fn tabs_hide_practice_floor() {
    let bag = Bag::practice_only();
    assert!(
        bag.tabs().is_empty(),
        "practice is an internal floor, not a selectable TUI model"
    );

    let mut bag = bag_of(vec![solo("practice", true), solo("turbo", true)]);
    bag.cycle();
    assert_eq!(bag.in_hand_label(), "turbo");
    assert_eq!(bag.tabs().len(), 1);
    assert_eq!(bag.tabs()[0].label, "turbo");
}

#[test]
fn settle_moves_off_a_fully_offline_box() {
    let mut bag = boxes(&[false, true]);
    bag.settle_in_hand();
    assert_eq!(bag.in_hand, 1);
    bag.settle_in_hand(); // now up → no-op
    assert_eq!(bag.in_hand, 1);
}

#[test]
fn subcontrol_cycles_modes_skipping_offline_and_reports_mode() {
    // One box (spark) with modes: swarm(up), gemma(up), coder(down).
    let spark = Agent {
        name: "spark".to_string(),
        slots: vec![
            slot("swarm", true),
            slot("gemma", true),
            slot("coder", false),
        ],
        active: 0,
    };
    let mut bag = bag_of(vec![spark, solo("atlas", true)]);
    assert_eq!(bag.in_hand_label(), "spark");
    assert_eq!(bag.in_hand_mode().as_deref(), Some("swarm"));
    bag.cycle_sub(true);
    assert_eq!(
        bag.in_hand_mode().as_deref(),
        Some("gemma"),
        "→ moves to the next live mode"
    );
    bag.cycle_sub(true);
    assert_eq!(
        bag.in_hand_mode().as_deref(),
        Some("swarm"),
        "skips offline coder, wraps to swarm"
    );
    bag.cycle_sub(false);
    assert_eq!(
        bag.in_hand_mode().as_deref(),
        Some("gemma"),
        "← moves back, still skipping coder"
    );
    // A single-mode box reports no mode and ignores the subcontrol.
    bag.cycle();
    assert_eq!(bag.in_hand_label(), "atlas");
    assert_eq!(bag.in_hand_mode().as_deref(), None);
    bag.cycle_sub(true);
    assert_eq!(bag.in_hand_mode().as_deref(), None);
}

#[test]
fn settle_keeps_the_box_and_switches_to_a_live_mode() {
    // Active mode (swarm) dies but gemma on the same box is up → stay on the
    // box, just move the active mode. Don't jump to another box needlessly.
    let spark = Agent {
        name: "spark".to_string(),
        slots: vec![slot("swarm", false), slot("gemma", true)],
        active: 0,
    };
    let mut bag = bag_of(vec![spark, solo("atlas", true)]);
    bag.settle_in_hand();
    assert_eq!(
        bag.in_hand_label(),
        "spark",
        "stays on the box that's still up"
    );
    assert_eq!(
        bag.in_hand_mode().as_deref(),
        Some("gemma"),
        "moves to the live mode"
    );
}

#[test]
fn tab_landing_settles_onto_a_live_mode() {
    // box0 up; box1 has a dead default mode but a live second mode. Tab to box1
    // must land on the live mode, not the dead default.
    let box1 = Agent {
        name: "box1".to_string(),
        slots: vec![slot("dead", false), slot("live", true)],
        active: 0,
    };
    let mut bag = bag_of(vec![solo("box0", true), box1]);
    bag.cycle();
    assert_eq!(bag.in_hand, 1);
    assert_eq!(bag.in_hand_mode().as_deref(), Some("live"));
}

#[test]
fn follow_backend_adopts_a_swapped_checkpoint() {
    let club = learned_club("turbo", "nex2-mini");
    assert_eq!(club.live_model_name().as_deref(), Some("nex2-mini"));
    // The rig swaps checkpoints → the club follows the live id.
    club.remember_reported_model(&serde_json::json!({"data": [{"id": "ornith-1.0-35b-turbo"}]}));
    assert_eq!(
        club.live_model_name().as_deref(),
        Some("ornith-1.0-35b-turbo")
    );
    // A pinned club (configured model) keeps its identity and exposes no live
    // name, so env pins and SOTA catalog defaults never drift.
    let pinned = HttpClub::new("sota", "http://127.0.0.1:1", "deepseek-v4-pro", None);
    pinned.remember_reported_model(&serde_json::json!({"data": [{"id": "other-model"}]}));
    assert_eq!(pinned.live_model_name(), None);
}

#[test]
fn display_label_prefers_the_live_checkpoint() {
    let live = Slot {
        label: "gemma".to_string(),
        club: Arc::new(learned_club("gemma", "Qwen3.6-27B-MTP-pi-tune-Q4_K_M.gguf")),
        available: Arc::new(AtomicBool::new(true)),
    };
    assert_eq!(live.display_label(), "qwen3.6-27b-mtp-pi-tune");
    // Fixed-identity slots (practice/swarm/pinned) keep their static label.
    assert_eq!(slot("swarm", true).display_label(), "swarm");
}

#[test]
fn resolve_driver_matches_a_live_model_id() {
    let mut agents = vec![Agent {
        name: "atlas".to_string(),
        slots: vec![Slot {
            label: "atlas".to_string(),
            club: Arc::new(learned_club("atlas", "Qwen3.6-27B-MTP-pi-tune-Q4_K_M.gguf")),
            available: Arc::new(AtomicBool::new(true)),
        }],
        active: 0,
    }];
    // ANGEL_DRIVER can name the live checkpoint (full or short form), not just
    // a baked-in mode hint.
    assert_eq!(
        resolve_driver(&mut agents, "Qwen3.6-27B-MTP-pi-tune-Q4_K_M.gguf"),
        Some(0)
    );
    assert_eq!(
        resolve_driver(&mut agents, "qwen3.6-27b-mtp-pi-tune"),
        Some(0)
    );
    assert_eq!(resolve_driver(&mut agents, "no-such-model"), None);
}

#[test]
fn brain_election_scores_the_live_checkpoint() {
    let (a_up, b_up) = (
        Arc::new(AtomicBool::new(true)),
        Arc::new(AtomicBool::new(true)),
    );
    let boxed = |name: &str, id: &str, avail: &Arc<AtomicBool>| Agent {
        name: name.to_string(),
        slots: vec![Slot {
            label: name.to_string(),
            club: Arc::new(learned_club(name, id)),
            available: Arc::clone(avail),
        }],
        active: 0,
    };
    let mut bag = bag_of(vec![
        boxed("small", "tiny-7b", &a_up),
        boxed("big", "ornith-1.0-35b-turbo", &b_up),
    ]);
    // Meta as recorded at assembly, before any live id was known (empty model
    // ids — the pre-refresh state that used to pin every static slot to the
    // unknown-size floor).
    let meta = |agent, avail: &Arc<AtomicBool>| SlotMeta {
        agent,
        slot: 0,
        model: String::new(),
        n_params: None,
        is_swarm: false,
        auto: true,
        available: Arc::clone(avail),
    };
    bag.meta = vec![meta(0, &a_up), meta(1, &b_up)];
    bag.pending_brain = true;
    bag.settle_brain();
    assert_eq!(
        bag.in_hand_label(),
        "big",
        "election must score the live 35B id, not the empty startup snapshot"
    );
}

#[test]
fn endpoint_host_port_parses_only_plain_http() {
    assert_eq!(
        parse_endpoint_host_port("http://100.1.2.3:8090/v1"),
        Some(("100.1.2.3".to_string(), 8090))
    );
    assert_eq!(
        parse_endpoint_host_port("http://127.0.0.1:8777"),
        Some(("127.0.0.1".to_string(), 8777))
    );
    assert_eq!(
        parse_endpoint_host_port("http://localhost:8080/v1/"),
        Some(("localhost".to_string(), 8080))
    );
    // Cloud https URLs and portless endpoints are not scannable fleet targets.
    assert_eq!(
        parse_endpoint_host_port("https://api.deepseek.com/v1"),
        None
    );
    assert_eq!(parse_endpoint_host_port("http://nohost/v1"), None);
}

#[test]
fn hydra_targets_take_only_chat_surfaces() {
    let _g = env_lock();
    let _aliases = ScopedEnv::set(SPARK_HOST_ALIASES_ENV, "spark-hydra-peer");
    let v = serde_json::json!({"ok": true, "surfaces": [
        {"surface_id": "turbo:8090", "host_id": "turbo",
         "endpoint": "http://100.64.0.4:8090/v1",
         "model_id": "ornith-1.0-35b-turbo", "roles": ["chat"]},
        {"surface_id": "spark-hydra-peer:8001", "host_id": "spark-hydra-peer",
         "endpoint": "http://127.0.0.1:8001/v1",
         "model_id": "qwen38-27b", "roles": ["chat"]},
        // A gen surface on the same head is not a chat club.
        {"surface_id": "apollo.trellis.3dgen", "host_id": "apollo",
         "endpoint": "http://127.0.0.1:18080/v1",
         "model_id": "microsoft/TRELLIS.2-4B", "roles": ["asset.3d", "image-to-3d"]},
        // Chat, but no scannable plain-http endpoint.
        {"surface_id": "cloud", "host_id": "cloud",
         "endpoint": "https://api.example.com/v1", "roles": ["chat"]},
        // Chat, bare endpoint, blank host_id → host label falls back to the ip.
        {"surface_id": "x", "host_id": "",
         "endpoint": "http://100.66.1.2:8012", "roles": ["chat"]},
    ]});
    assert_eq!(
        parse_hydra_chat_targets(&v),
        vec![
            ("turbo".to_string(), "100.64.0.4".to_string(), 8090),
            ("spark".to_string(), "127.0.0.1".to_string(), 8001),
            ("100.66.1.2".to_string(), "100.66.1.2".to_string(), 8012),
        ]
    );
    // Degenerate replies parse to nothing rather than erroring.
    assert!(parse_hydra_chat_targets(&serde_json::json!({"ok": false})).is_empty());
}

#[test]
fn default_scan_ports_include_canonical_turbo_serving() {
    let _guard = env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SCAN_PORTS") };
    assert!(scan_ports().contains(&8093));
    assert!(scan_ports().contains(&8002));
    assert!(scan_ports().contains(&18888));
}

#[test]
fn fleet_network_capabilities_are_opt_in() {
    let _guard = env_lock();
    let _scan = ScopedEnv::unset("ANGEL_SCAN");
    let _tailnet = ScopedEnv::unset("ANGEL_TAILNET_RESOLVE");
    let _hydra_read = ScopedEnv::unset("ANGEL_HYDRA_DISCOVER");
    let _hydra_write = ScopedEnv::unset("ANGEL_HYDRA_PUBLISH");

    assert!(!fleet_scan_enabled());
    assert!(!tailnet_resolution_enabled());
    assert!(!hydra_discovery_enabled());
    assert!(!hydra_publish_enabled());
    assert!(tailnet_hosts().is_empty());
    assert!(hydra_chat_targets().is_empty());
    assert!(scan_fleet().is_empty());

    let _scan_on = ScopedEnv::set("ANGEL_SCAN", "1");
    let _tailnet_on = ScopedEnv::set("ANGEL_TAILNET_RESOLVE", "true");
    let _hydra_read_on = ScopedEnv::set("ANGEL_HYDRA_DISCOVER", "yes");
    let _hydra_write_on = ScopedEnv::set("ANGEL_HYDRA_PUBLISH", "on");
    assert!(fleet_scan_enabled());
    assert!(tailnet_resolution_enabled());
    assert!(hydra_discovery_enabled());
    assert!(hydra_publish_enabled());
}

#[test]
fn private_host_detection_handles_ipv6_and_pool_sizes_for_swarm_waves() {
    assert!(is_private_host("http://[::1]:8000/v1"));
    assert!(is_private_host("http://[fd00::1234]:8000/v1"));
    assert!(!is_private_host("https://[2606:4700:4700::1111]:443/v1"));
    assert!(is_private_host("http://100.64.0.3:8000/v1"));
    assert!(!is_private_host("https://api.deepseek.com/v1"));

    let _guard = env_lock();
    let saved = std::env::var_os("ANGEL_HTTP_IDLE_CONNECTIONS");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_HTTP_IDLE_CONNECTIONS") };
    assert_eq!(idle_connections_per_host("http://100.64.0.3:8000/v1"), 96);
    assert_eq!(idle_connections_per_host("https://api.deepseek.com/v1"), 16);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_HTTP_IDLE_CONNECTIONS", "80") };
    assert_eq!(idle_connections_per_host("http://100.64.0.3:8000/v1"), 80);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_HTTP_IDLE_CONNECTIONS", "9999") };
    assert_eq!(idle_connections_per_host("http://100.64.0.3:8000/v1"), 512);
    match saved {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_HTTP_IDLE_CONNECTIONS", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_HTTP_IDLE_CONNECTIONS") },
    }
}

#[test]
fn fleet_scan_thread_count_is_bounded() {
    let _guard = env_lock();
    let saved = std::env::var_os("ANGEL_SCAN_CONCURRENCY");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SCAN_CONCURRENCY") };
    assert_eq!(scan_concurrency(0), 0);
    assert_eq!(scan_concurrency(4), 4);
    assert_eq!(scan_concurrency(400), 32);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SCAN_CONCURRENCY", "80") };
    assert_eq!(scan_concurrency(400), 80);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SCAN_CONCURRENCY", "9999") };
    assert_eq!(scan_concurrency(400), 256);
    match saved {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_SCAN_CONCURRENCY", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_SCAN_CONCURRENCY") },
    }
}

#[test]
fn model_serving_exclusion_policy_is_explicit_and_exact() {
    let _guard = env_lock();
    let _legacy = ScopedEnv::unset("ANGEL_ATLAS_MODEL_SERVING");
    let _excluded = ScopedEnv::unset("ANGEL_MODEL_SERVING_EXCLUDE_HOSTS");

    // No machine name is special by default, including Atlas-shaped names.
    assert!(model_serving_target_allowed("turbo", "100.64.0.4"));
    assert!(model_serving_target_allowed("atlas", "100.64.0.2"));
    assert!(model_serving_target_allowed("atlas-1", "100.64.0.3"));
    assert!(model_serving_target_allowed("atlas-10", "100.64.0.4"));
    assert!(model_serving_target_allowed("custom", "100.64.0.2"));

    let _excluded = ScopedEnv::set("ANGEL_MODEL_SERVING_EXCLUDE_HOSTS", " Atlas , 100.64.0.3 ");
    assert!(!model_serving_target_allowed("atlas", "100.64.0.2"));
    assert!(!model_serving_target_allowed("other", "100.64.0.3"));
    assert!(model_serving_target_allowed("atlas-1", "100.64.0.4"));
    assert!(model_serving_target_allowed("atlas-10", "100.64.0.5"));

    let _legacy_enabled = ScopedEnv::set("ANGEL_ATLAS_MODEL_SERVING", "1");
    assert!(!model_serving_target_allowed("atlas", "100.64.0.2"));
}

#[test]
fn parse_params_picks_largest_b_token() {
    assert_eq!(parse_param_billions("Qwen3.6-27B-MTP-pi-tune"), Some(27.0));
    assert_eq!(parse_param_billions("siq-1-35b-turbo"), Some(35.0));
    assert_eq!(parse_param_billions("deepseek-r1-14b"), Some(14.0));
    // A version digit not followed by `b` is NOT a param count.
    assert_eq!(parse_param_billions("gemma4"), None);
    assert_eq!(parse_param_billions("nvfp4_model"), None);
}

#[test]
fn smartness_ranks_bigger_and_swarm_higher() {
    // Backend-reported params anchor the score; the swarm amplifies its inner
    // model (26B gemma4 × 1.5 tops a raw 27B); an unsized model sits on the floor.
    let atlas = model_smartness("Qwen3.6-27B", Some(27_320_697_856), false);
    let swarm = model_smartness("gemma4", None, true);
    let coder = model_smartness("qwopus-coder", None, false);
    assert!(swarm > atlas, "swarm(26B×1.5) should top atlas(27B)");
    assert!(atlas > coder, "a sized 27B should top an unsized coder");
}

#[test]
fn short_label_strips_path_and_ext() {
    assert_eq!(
        short_model_label("/home/x/qworld/nvfp4_model"),
        "nvfp4_model"
    );
    assert_eq!(short_model_label("Foo.GGUF"), "foo");
    assert!(short_model_label("Qwen3.6-27B-MTP-pi-tune-Q4_K_M.gguf").len() <= 24);
}

#[test]
fn short_label_truncates_non_ascii_on_char_boundary() {
    let label = short_model_label("aaaaaaaaaaaaaaaaaaaaaaaើ-tail.gguf");
    assert_eq!(label, "aaaaaaaaaaaaaaaaaaaaaaa");
}

#[test]
fn longcat_is_sota_and_driver_resolvable_by_model_label() {
    assert!(is_sota_label("LongCat-2.0"));

    let mut agents = vec![Agent {
        name: "sota".to_string(),
        slots: vec![Slot {
            label: "longcat".to_string(),
            club: Arc::new(LabelClub("LongCat-2.0")),
            available: Arc::new(AtomicBool::new(true)),
        }],
        active: 0,
    }];
    assert_eq!(resolve_driver(&mut agents, "longcat"), Some(0));
    assert_eq!(resolve_driver(&mut agents, "LongCat-2.0"), Some(0));
}

#[test]
fn one_sota_link_remains_a_directly_resolvable_driver() {
    let available = Arc::new(AtomicBool::new(true));
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![(
        "longcat".to_string(),
        Arc::new(LabelClub("LongCat-2.0")),
        Arc::clone(&available),
    )];
    let mut meta = Vec::new();
    let mut agents = vec![direct_sota_agent(0, &links, &mut meta)];

    assert_eq!(resolve_driver(&mut agents, "longcat"), Some(0));
    assert_eq!(
        agents[0].slots[agents[0].active].club.label(),
        "LongCat-2.0"
    );
    assert_eq!(meta.len(), 1);
    assert!(meta[0].available.load(Ordering::Relaxed));
}

#[test]
fn longcat_scores_above_local_fleet_for_auto_driver() {
    assert!(model_smartness("LongCat-2.0", None, false) > model_smartness("gemma4", None, true));
    assert!(
        model_smartness("LongCat-2.0", None, false) > model_smartness("Qwen3.6-27B", None, false)
    );
}

#[test]
fn zai_key_defaults_glm_to_zai_coding_endpoint() {
    let _guard = env_lock();
    for key in [
        "ANGEL_GLM_KEY",
        "GLM_API_KEY",
        "ZAI_API_KEY",
        "ZHIPU_API_KEY",
        "BIGMODEL_API_KEY",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ZAI_API_KEY", "test-key") };
    assert_eq!(default_glm_url(), "https://api.z.ai/api/coding/paas/v4");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ZAI_API_KEY") };
}

#[test]
fn explicit_glm_provider_url_selects_its_provider_specific_key_first() {
    let _guard = env_lock();
    let _angel = ScopedEnv::set("ANGEL_GLM_KEY", "stale-generic");
    let _zai = ScopedEnv::set("ZAI_API_KEY", "live-zai");
    let _zhipu = ScopedEnv::set("ZHIPU_API_KEY", "live-zhipu");

    assert_eq!(
        env_first(glm_key_env_order("https://api.z.ai/api/coding/paas/v4")).as_deref(),
        Some("live-zai")
    );
    assert_eq!(
        env_first(glm_key_env_order("https://open.bigmodel.cn/api/paas/v4")).as_deref(),
        Some("live-zhipu")
    );
    assert_eq!(
        env_first(glm_key_env_order("http://127.0.0.1:8000/v1")).as_deref(),
        Some("stale-generic")
    );
}

#[test]
#[ignore = "live Z.ai TLS trust probe; run explicitly during provider incidents"]
fn zai_https_handshake_uses_the_host_trust_store() {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(10))
        .build();
    match agent
        .get("https://api.z.ai/api/coding/paas/v4/models")
        .call()
    {
        Ok(_) | Err(ureq::Error::Status(_, _)) => {}
        Err(ureq::Error::Transport(error)) => {
            panic!("Z.ai TLS transport failed before HTTP status: {error}")
        }
    }
}

#[test]
#[ignore = "live, metered GLM completion/tool-call probe; run explicitly during provider incidents"]
fn live_glm_53_and_flash_stream_and_call_tools_through_http_club() {
    fn local_env_value(name: &str) -> Option<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.angel.env");
        let contents = std::fs::read_to_string(path).ok()?;
        contents.lines().find_map(|line| {
            let line = line.trim().strip_prefix("export ").unwrap_or(line.trim());
            let (key, value) = line.split_once('=')?;
            if key.trim() != name {
                return None;
            }
            let value = value.trim();
            Some(
                value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .or_else(|| {
                        value
                            .strip_prefix('\'')
                            .and_then(|value| value.strip_suffix('\''))
                    })
                    .unwrap_or(value)
                    .to_string(),
            )
        })
    }

    let key = local_env_value("ANGEL_GLM_KEY")
        .filter(|value| !value.contains("${"))
        .or_else(|| local_env_value("ZAI_API_KEY"))
        .expect("live GLM probe needs ANGEL_GLM_KEY or ZAI_API_KEY in .angel.env");
    let url = local_env_value("ANGEL_GLM_URL")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .unwrap_or_else(|| "https://api.z.ai/api/coding/paas/v4".to_string());
    let cancel = AtomicBool::new(false);
    let tool = ToolDef {
        name: "angel_live_probe".to_string(),
        description: "Record one provider transport health probe.".to_string(),
        params: serde_json::json!({
            "type": "object",
            "properties": { "value": { "type": "string" } },
            "required": ["value"],
            "additionalProperties": false
        }),
    };

    for model in ["glm-5.3", "glm-5.3-flash"] {
        let club = HttpClub::new("glm", &url, model, Some(key.clone()));
        let started = Instant::now();
        let mut first_delta = None;
        let mut first_content = None;
        let mut streamed = String::new();
        let reply = club
            .chat_streaming_with_effort(
                &[ChatMsg::user(
                    "Reply with exactly ANGEL_GLM_LIVE_OK and no other text.",
                )],
                &[],
                Some("high"),
                &cancel,
                &mut |delta| {
                    let elapsed = started.elapsed();
                    first_delta.get_or_insert(elapsed);
                    if let StreamDelta::Content(text) = delta {
                        first_content.get_or_insert(elapsed);
                        streamed.push_str(text);
                    }
                },
            )
            .unwrap_or_else(|error| panic!("{model} streaming completion failed: {error}"));
        let total = started.elapsed();
        let text = match reply {
            ClubReply::Text(text) => text,
            ClubReply::Calls(_) => panic!("{model} returned a tool call for a tool-less probe"),
        };
        assert!(
            text.contains("ANGEL_GLM_LIVE_OK") && streamed.contains("ANGEL_GLM_LIVE_OK"),
            "{model} did not stream the expected marker"
        );
        eprintln!(
            "LIVE_GLM model={model} case=text first_delta_ms={} first_content_ms={} total_ms={}",
            first_delta.expect("stream emitted no delta").as_millis(),
            first_content
                .expect("stream emitted no content")
                .as_millis(),
            total.as_millis()
        );

        let started = Instant::now();
        let mut first_delta = None;
        let reply = club
            .chat_streaming_with_effort(
                &[ChatMsg::user(
                    "Call angel_live_probe exactly once with value set to ping. Do not answer in prose.",
                )],
                std::slice::from_ref(&tool),
                Some("high"),
                &cancel,
                &mut |_| {
                    first_delta.get_or_insert(started.elapsed());
                },
            )
            .unwrap_or_else(|error| panic!("{model} streaming tool call failed: {error}"));
        let total = started.elapsed();
        let calls = match reply {
            ClubReply::Calls(calls) => calls,
            ClubReply::Text(_) => panic!("{model} answered in prose instead of calling the tool"),
        };
        assert_eq!(calls.len(), 1, "{model} emitted the wrong call count");
        assert_eq!(calls[0].name, "angel_live_probe");
        assert_eq!(calls[0].args["value"], "ping");
        eprintln!(
            "LIVE_GLM model={model} case=tool first_delta_ms={} total_ms={}",
            first_delta
                .map(|elapsed| elapsed.as_millis().to_string())
                .unwrap_or_else(|| "none".to_string()),
            total.as_millis()
        );
    }
}

#[test]
#[ignore = "live, metered end-to-end GLM task-mode probe; requires the release binary"]
fn live_glm_53_and_flash_complete_a_real_harness_tool_turn() {
    fn local_env_value(name: &str) -> Option<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.angel.env");
        let contents = std::fs::read_to_string(path).ok()?;
        contents.lines().find_map(|line| {
            let line = line.trim().strip_prefix("export ").unwrap_or(line.trim());
            let (key, value) = line.split_once('=')?;
            if key.trim() != name {
                return None;
            }
            let value = value.trim();
            Some(
                value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .or_else(|| {
                        value
                            .strip_prefix('\'')
                            .and_then(|value| value.strip_suffix('\''))
                    })
                    .unwrap_or(value)
                    .to_string(),
            )
        })
    }

    let key = local_env_value("ANGEL_GLM_KEY")
        .filter(|value| !value.contains("${"))
        .or_else(|| local_env_value("ZAI_API_KEY"))
        .expect("live GLM probe needs ANGEL_GLM_KEY or ZAI_API_KEY in .angel.env");
    let url = local_env_value("ANGEL_GLM_URL")
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .unwrap_or_else(|| "https://api.z.ai/api/coding/paas/v4".to_string());
    let binary = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/release/angel");
    assert!(
        binary.is_file(),
        "build the release binary before the live harness probe"
    );

    for (driver, configured_model) in [("glm", "glm-5.3"), ("glm-5.3-flash", "glm-5.3")] {
        let root = std::env::temp_dir().join(format!(
            "angel-glm-harness-probe-{}-{}",
            std::process::id(),
            driver
        ));
        let sessions = root.join("sessions");
        std::fs::create_dir_all(&sessions).expect("create live harness probe workspace");
        let started = Instant::now();
        let output = std::process::Command::new(&binary)
            .args([
                "--task-json",
                "--workspace",
                root.to_str().expect("UTF-8 probe path"),
                "--max-hops",
                "4",
                "--deadline-secs",
                "90",
                "You must use the shell tool exactly once to run `printf ANGEL_HARNESS_TOOL_OK` in the workspace. Observe its output, then answer with ANGEL_HARNESS_GLM_OK.",
            ])
            .env("ANGEL_DRIVER", driver)
            .env("ANGEL_GLM_MODEL", configured_model)
            .env("ANGEL_GLM_KEY", &key)
            .env("ANGEL_GLM_URL", &url)
            .env("ANGEL_REASONING_EFFORT", "high")
            .env("ANGEL_SOTA_MOA_GROK_RESEARCH", "0")
            .env("ANGEL_GROK_RESEARCH", "0")
            .env("ANGEL_SESSION_DIR", &sessions)
            .output()
            .unwrap_or_else(|error| panic!("failed to launch {driver} harness probe: {error}"));
        let elapsed = started.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{driver} harness probe exited {:?}: {stderr}",
            output.status.code()
        );
        assert!(
            stdout.contains("ANGEL_HARNESS_GLM_OK"),
            "{driver} harness probe completed without the requested final marker: {stdout}"
        );
        assert!(
            stdout.contains("ANGEL_HARNESS_TOOL_OK"),
            "{driver} harness probe did not record the shell result: {stdout}"
        );
        eprintln!(
            "LIVE_GLM_HARNESS driver={driver} exit={} total_ms={}",
            output.status.code().unwrap_or(-1),
            elapsed.as_millis()
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
#[ignore = "live, metered end-to-end Grok OAuth task-mode probe; requires the release binary"]
fn live_grok_oauth_http_completes_a_real_harness_tool_turn() {
    let binary = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/release/angel");
    assert!(
        binary.is_file(),
        "build the release binary before the live harness probe"
    );
    let auth = std::env::var("ANGEL_GROK_OAUTH_FILE")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| std::path::PathBuf::from(home).join(".grok/auth.json"))
        })
        .expect("live Grok probe needs HOME or ANGEL_GROK_OAUTH_FILE");
    assert!(
        auth.is_file(),
        "live Grok probe needs ~/.grok/auth.json from `grok login --oauth`"
    );

    let root =
        std::env::temp_dir().join(format!("angel-grok-harness-probe-{}", std::process::id()));
    let sessions = root.join("sessions");
    std::fs::create_dir_all(&sessions).expect("create live Grok harness probe workspace");
    let started = Instant::now();
    let output = std::process::Command::new(&binary)
        .args([
            "--task-json",
            "--workspace",
            root.to_str().expect("UTF-8 probe path"),
            "--max-hops",
            "4",
            "--deadline-secs",
            "90",
            "You must use the shell tool exactly once to run `printf ANGEL_HARNESS_TOOL_OK > grok-tool-ok.txt` in the workspace. Then answer with ANGEL_HARNESS_GROK_OK.",
        ])
        .env("ANGEL_DRIVER", "grok")
        .env("ANGEL_GROK_MODEL", "grok-4.6")
        .env("ANGEL_GROK_REASONING_EFFORT", "low")
        .env("ANGEL_GROK_RESEARCH", "0")
        .env("ANGEL_SOTA_MOA_GROK_RESEARCH", "0")
        .env("ANGEL_SESSION_DIR", &sessions)
        .output()
        .unwrap_or_else(|error| panic!("failed to launch grok harness probe: {error}"));
    let elapsed = started.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "grok harness probe exited {:?}: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("ANGEL_HARNESS_GROK_OK"),
        "grok harness probe completed without the requested final marker: {stdout}"
    );
    let receipt = root.join("grok-tool-ok.txt");
    let tool_body = std::fs::read_to_string(&receipt).unwrap_or_default();
    assert!(
        tool_body.contains("ANGEL_HARNESS_TOOL_OK"),
        "grok harness probe did not write the shell receipt ({receipt:?}): {stdout}\n{stderr}"
    );
    eprintln!(
        "LIVE_GROK_HARNESS driver=grok exit={} total_ms={}",
        output.status.code().unwrap_or(-1),
        elapsed.as_millis()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn bigmodel_style_keys_keep_bigmodel_glm_endpoint() {
    let _guard = env_lock();
    for key in [
        "ANGEL_GLM_KEY",
        "GLM_API_KEY",
        "ZAI_API_KEY",
        "ZHIPU_API_KEY",
        "BIGMODEL_API_KEY",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("BIGMODEL_API_KEY", "test-key") };
    assert_eq!(default_glm_url(), "https://open.bigmodel.cn/api/paas/v4");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("BIGMODEL_API_KEY") };
}

#[test]
fn glm_catalog_lists_current_and_previous_models_without_duplicates() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "glm");
    let _key = ScopedEnv::set("ANGEL_GLM_KEY", "test-key");
    let _model = ScopedEnv::unset("ANGEL_GLM_MODEL");
    let _fallback_model = ScopedEnv::unset("GLM_MODEL");

    assert_eq!(
        GLM_API_MODEL_OPTIONS,
        &["glm-5.3", "glm-5.3-flash", "glm-5.2"]
    );
    assert_eq!(resolve_glm_model_alias("glm-flash"), "glm-5.3-flash");
    assert_eq!(resolve_glm_model_alias("GLM-5.3-Flash"), "glm-5.3-flash");
    assert_eq!(
        resolve_glm_model_alias("private/glm-next"),
        "private/glm-next"
    );
    assert_eq!(
        static_model_window("glm-5.3-flash"),
        Some((1_000_000, true))
    );
    let links = optional_glm_http_clubs();
    let routes = links
        .iter()
        .map(|(alias, club, _)| (alias.as_str(), club.model_identity().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        routes,
        vec![
            ("glm", "glm-5.3".to_string()),
            ("glm-5.3-flash", "glm-5.3-flash".to_string()),
            ("glm-5.2", "glm-5.2".to_string()),
        ]
    );

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_GLM_MODEL", "glm-5.2") };
    let pinned = optional_glm_http_clubs();
    assert_eq!(pinned.len(), 3);
    assert_eq!(pinned[0].0, "glm");
    assert_eq!(pinned[0].1.model_identity().as_deref(), Some("glm-5.2"));
    assert_eq!(pinned[1].0, "glm-5.3");
    assert_eq!(pinned[1].1.model_identity().as_deref(), Some("glm-5.3"));
    assert_eq!(pinned[2].0, "glm-5.3-flash");
    assert_eq!(
        pinned[2].1.model_identity().as_deref(),
        Some("glm-5.3-flash")
    );

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_GLM_MODEL", "glm-flash") };
    let flash_pin = optional_glm_http_clubs();
    assert_eq!(flash_pin.len(), 3);
    assert_eq!(flash_pin[0].0, "glm");
    assert_eq!(flash_pin[0].1.label(), "glm-5.3-flash");
    assert_eq!(
        flash_pin[0].1.model_identity().as_deref(),
        Some("glm-5.3-flash")
    );
    assert_eq!(flash_pin[1].0, "glm-5.3");
    assert_eq!(flash_pin[2].0, "glm-5.2");
}

#[test]
fn sota_moa_judge_defaults_to_codex_oauth_before_longcat() {
    let _guard = env_lock();
    let _scrub = scrub_sota_moa_env();
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "longcat".to_string(),
            Arc::new(LabelClub("LongCat-2.0")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "codex-run".to_string(),
            Arc::new(LabelClub("openai")),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let judge = pick_sota_role(&links, "ANGEL_SOTA_MOA_JUDGE_CLUB", SOTA_MOA_JUDGE_PREFS);
    assert_eq!(judge.label(), "openai");
}

#[test]
fn sota_moa_strict_routes_remove_the_cross_seat_failover_bench() {
    let _guard = env_lock();
    let _scrub = scrub_sota_moa_env();
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "glm".to_string(),
            Arc::new(LabelClub("glm-5.3-flash")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "openai".to_string(),
            Arc::new(LabelClub("openai")),
            Arc::new(AtomicBool::new(true)),
        ),
    ];

    assert_eq!(sota_moa_fallbacks(false, &links, &links).len(), 2);
    let _strict = ScopedEnv::set("ANGEL_SOTA_MOA_STRICT_ROUTES", "1");
    assert!(sota_moa_fallbacks(false, &links, &links).is_empty());
}

#[test]
fn sota_moa_verify_defaults_to_codex_oauth_before_longcat() {
    let _guard = env_lock();
    let _scrub = scrub_sota_moa_env();
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "deepseek".to_string(),
            Arc::new(LabelClub("deepseek-v4-pro")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "kimi".to_string(),
            Arc::new(LabelClub("kimi-k3")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "longcat".to_string(),
            Arc::new(LabelClub("longcat")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "codex-run".to_string(),
            Arc::new(LabelClub("openai")),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let verify = pick_sota_role(&links, "ANGEL_SOTA_MOA_VERIFY_CLUB", SOTA_MOA_VERIFY_PREFS);
    assert_eq!(verify.label(), "openai");
}

#[test]
fn cheap_moa_pool_excludes_metered_frontier_links_and_routes_expected_roles() {
    let _guard = env_lock();
    let keys = [
        "ANGEL_SOTA_MOA_PROPOSE_CLUB",
        "ANGEL_SOTA_MOA_JUDGE_CLUB",
        "ANGEL_SOTA_MOA_VERIFY_CLUB",
        "ANGEL_SOTA_MOA_AGG_CLUB",
    ];
    let _scrub = scrub_sota_moa_env();
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "codex-run".to_string(),
            Arc::new(LabelClub("openai")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "deepseek".to_string(),
            Arc::new(LabelClub("deepseek-v4-pro")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "kimi".to_string(),
            Arc::new(LabelClub("kimi-k3")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "longcat".to_string(),
            Arc::new(LabelClub("longcat")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "openrouter".to_string(),
            Arc::new(LabelClub("tencent/hy3:free")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "gemma".to_string(),
            Arc::new(LabelClub("gemma")),
            Arc::new(AtomicBool::new(false)),
        ),
    ];
    let cheap = ordered_cheap_links(&links);
    assert_eq!(
        cheap
            .iter()
            .map(|(alias, _, _)| alias.as_str())
            .collect::<Vec<_>>(),
        vec!["gemma", "openrouter", "longcat"]
    );
    assert_eq!(
        pick_sota_role(&cheap, keys[0], SOTA_MOA_CHEAP_PROPOSE_PREFS).label(),
        "longcat"
    );
    assert_eq!(
        pick_sota_role(&cheap, keys[1], SOTA_MOA_CHEAP_JUDGE_PREFS).label(),
        "longcat"
    );
    assert_eq!(
        pick_sota_role(&cheap, keys[2], SOTA_MOA_CHEAP_VERIFY_PREFS).label(),
        "tencent/hy3:free"
    );
    assert_eq!(
        pick_sota_role(&cheap, keys[3], SOTA_MOA_CHEAP_AGG_PREFS).label(),
        "longcat"
    );
}

#[test]
fn sota_moa_gemma_extra_proposer_is_default_and_can_be_disabled() {
    let _guard = env_lock();
    let key = "ANGEL_SOTA_MOA_EXTRA_PROPOSERS";
    let _scrub = scrub_sota_moa_env();
    let breadth: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "gemma".to_string(),
            Arc::new(LabelClub("gemma")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "atlas".to_string(),
            Arc::new(LabelClub("atlas")),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let extras = extra_sota_proposers(&breadth);
    assert_eq!(extras.len(), 1);
    assert_eq!(extras[0].0.label(), "gemma");

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "none") };
    assert!(extra_sota_proposers(&breadth).is_empty());
}

#[test]
fn cheap_moa_link_allowlist_can_hard_pin_longcat_only() {
    let _guard = env_lock();
    let keys = [
        "ANGEL_SOTA_MOA_COST_PROFILE",
        "ANGEL_SOTA_MOA_CHEAP_LINKS",
        "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
    ];
    let _scrub = scrub_sota_moa_env();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(keys[0], "cheap") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(keys[1], "longcat") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(keys[2], "gemma") };
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "gemma".to_string(),
            Arc::new(LabelClub("gemma")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "openrouter".to_string(),
            Arc::new(LabelClub("tencent/hy3:free")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "longcat".to_string(),
            Arc::new(LabelClub("longcat")),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let cheap = ordered_cheap_links(&links);
    assert_eq!(cheap.len(), 1);
    assert_eq!(cheap[0].0, "longcat");
    assert!(extra_sota_proposers(&links[..1]).is_empty());
    assert!(
        sota_moa_club_with_breadth(&links[2..], &links[..1]).is_some(),
        "cheap homogeneous MoA permits one provider with multiple agent calls"
    );
}

#[test]
fn mathgod_club_is_a_regular_bag_label_when_sol_and_grok_exist() {
    let bit = || Arc::new(AtomicBool::new(true));
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        ("glm".to_string(), Arc::new(LabelClub("glm-5.3")), bit()),
        ("grok".to_string(), Arc::new(LabelClub("grok-4.6")), bit()),
        (
            "openai".to_string(),
            Arc::new(LabelClub("gpt-5.6-sol")),
            bit(),
        ),
        (
            "deepseek".to_string(),
            Arc::new(LabelClub("deepseek-v4-pro")),
            bit(),
        ),
        (
            "deepseek-flash".to_string(),
            Arc::new(LabelClub("deepseek-v4-flash")),
            bit(),
        ),
    ];
    let (club, _) = mathgod_club(&links).expect("mathgod club");
    assert_eq!(club.label(), "mathgod");
    let mix: Vec<_> = mathgod_mix_seats(&links, links[2].1.as_ref(), links[1].1.as_ref())
        .into_iter()
        .map(|(club, _)| club.label().to_string())
        .collect();
    assert_eq!(
        mix,
        vec!["glm-5.3".to_string(), "deepseek-v4-pro".to_string()],
        "mix is GLM + DeepSeek Pro; Flash is not a Math God seat"
    );
    assert!(
        mathgod_club(&links[..2]).is_none(),
        "no Sol → no mathgod club"
    );
}

#[test]
fn sota_moa_role_can_be_pinned_by_live_grok_model_name() {
    let _guard = env_lock();
    let key = "ANGEL_SOTA_MOA_AGG_CLUB";
    let _scrub = scrub_sota_moa_env();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "grok-4.5") };
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "longcat".to_string(),
            Arc::new(LabelClub("LongCat-2.0")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "grok".to_string(),
            Arc::new(LiveModelClub {
                label: "grok-research",
                model: "grok-4.5",
            }),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let agg = pick_sota_role(&links, key, SOTA_MOA_AGG_PREFS);
    assert_eq!(agg.label(), "grok-research");
}

#[test]
fn sota_moa_aggregate_defaults_to_smartest_link_codex_oauth() {
    let _guard = env_lock();
    let _scrub = scrub_sota_moa_env();
    // The AGG seat is the deep thinker (tool-capable driver + final synthesis):
    // it must land on the smartest configured link, never a breadth-tier one.
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "deepseek".to_string(),
            Arc::new(LabelClub("deepseek-v4-pro")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "codex-run".to_string(),
            Arc::new(LabelClub("openai")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "longcat".to_string(),
            Arc::new(LabelClub("longcat")),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let agg = pick_sota_role(&links, "ANGEL_SOTA_MOA_AGG_CLUB", SOTA_MOA_AGG_PREFS);
    assert_eq!(agg.label(), "openai");
}

#[test]
fn sota_moa_aggregate_degrades_to_next_smartest_without_codex() {
    let _guard = env_lock();
    let _scrub = scrub_sota_moa_env();
    // With the frontier link absent, the seat walks DOWN the intelligence
    // ranking (Kimi beats DeepSeek and LongCat) — never sideways to a free tier.
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            "longcat".to_string(),
            Arc::new(LabelClub("longcat")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "deepseek".to_string(),
            Arc::new(LabelClub("deepseek-v4-pro")),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            "kimi".to_string(),
            Arc::new(LabelClub("kimi-k3")),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let agg = pick_sota_role(&links, "ANGEL_SOTA_MOA_AGG_CLUB", SOTA_MOA_AGG_PREFS);
    assert_eq!(agg.label(), "kimi-k3");
}

#[test]
fn bag_reports_sota_moa_modes_for_model_diagnostics() {
    let bag = Bag::for_render_test(&[(
        "sota",
        &[("sota-moa", true), ("deepseek", true), ("longcat", false)],
    )]);
    let status = bag.sota_moa_status();
    assert!(status.contains("available"), "{status}");
    assert!(status.contains("sota-moa"), "{status}");
    assert!(status.contains("longcat!"), "{status}");
}

#[test]
fn longcat_defaults_to_openai_v1_endpoint_and_model() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "longcat");
    for key in [
        "ANGEL_LONGCAT_KEY",
        "LONGCAT_API_KEY",
        "ANGEL_LONGCAT_URL",
        "LONGCAT_URL",
        "LONGCAT_API_URL",
        "ANGEL_LONGCAT_MODEL",
        "LONGCAT_MODEL",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("LONGCAT_API_KEY", "test-key") };
    let Some((alias, club, _)) = optional_sota_http_club(
        "longcat",
        "longcat",
        &["ANGEL_LONGCAT_URL", "LONGCAT_URL", "LONGCAT_API_URL"],
        "https://api.longcat.chat/openai/v1",
        &["ANGEL_LONGCAT_MODEL", "LONGCAT_MODEL"],
        Some("LongCat-2.0"),
        &["ANGEL_LONGCAT_KEY", "LONGCAT_API_KEY"],
    ) else {
        panic!("LONGCAT_API_KEY should configure the default LongCat link");
    };
    assert_eq!(alias, "longcat");
    assert_eq!(club.label(), "longcat");
    let http = HttpClub::new(
        "longcat",
        "https://api.longcat.chat/openai/v1/",
        "LongCat-2.0",
        Some("test-key".to_string()),
    );
    assert_eq!(
        http.chat_completions_url(),
        "https://api.longcat.chat/openai/v1/chat/completions"
    );

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("LONGCAT_URL", "https://longcat.invalid/openai/v1") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("LONGCAT_MODEL", "longcat-test") };
    assert!(
        optional_sota_http_club(
            "longcat",
            "longcat",
            &["ANGEL_LONGCAT_URL", "LONGCAT_URL", "LONGCAT_API_URL"],
            "https://api.longcat.chat/openai/v1",
            &["ANGEL_LONGCAT_MODEL", "LONGCAT_MODEL"],
            Some("LongCat-2.0"),
            &["ANGEL_LONGCAT_KEY", "LONGCAT_API_KEY"],
        )
        .is_some()
    );
    for key in [
        "ANGEL_LONGCAT_KEY",
        "LONGCAT_API_KEY",
        "ANGEL_LONGCAT_URL",
        "LONGCAT_URL",
        "LONGCAT_API_URL",
        "ANGEL_LONGCAT_MODEL",
        "LONGCAT_MODEL",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
}

#[test]
fn smartest_available_skips_dead_and_non_auto() {
    let meta = |agent, model: &str, auto, up: &Arc<AtomicBool>| SlotMeta {
        agent,
        slot: 0,
        model: model.to_string(),
        n_params: None,
        is_swarm: false,
        auto,
        available: Arc::clone(up),
    };
    let dead = Arc::new(AtomicBool::new(false));
    let live = Arc::new(AtomicBool::new(true));
    let mut agents = vec![solo("big", false), solo("tiny", true)];
    // The bigger model is down → the live smaller one wins.
    let m1 = vec![
        meta(0, "huge-70b", true, &dead),
        meta(1, "tiny-7b", true, &live),
    ];
    assert_eq!(smartest_available(&mut agents, &m1), Some(1));
    // A bigger live model flagged non-auto (openai/practice) is ignored.
    let m2 = vec![
        meta(0, "huge-70b", false, &live),
        meta(1, "tiny-7b", true, &live),
    ];
    assert_eq!(smartest_available(&mut agents, &m2), Some(1));
}

/// A club whose reachability is driven by an external flag — lets the live
/// prober thread be exercised with no network.
struct FlakyClub {
    up: Arc<AtomicBool>,
}
impl Club for FlakyClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn label(&self) -> &str {
        "flaky"
    }
    fn is_available(&self) -> bool {
        self.up.load(Ordering::Relaxed)
    }
}

/// Block until `cond` holds or the deadline passes; returns whether it held.
fn wait_until(deadline: Duration, cond: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    cond()
}

#[test]
fn prober_drops_then_restores_a_club_off_the_ui_thread() {
    // Drive the *real* prober thread with a fast interval and a no-network
    // club. Proves the probe → hysteresis → flip → recover path actually
    // runs, not just the seeded-atomic logic.
    let up = Arc::new(AtomicBool::new(true));
    let club: Arc<dyn Club> = Arc::new(FlakyClub {
        up: Arc::clone(&up),
    });
    let bit = Arc::new(AtomicBool::new(true));
    let read = Arc::clone(&bit);
    spawn_prober(vec![(club, bit)], Duration::from_millis(20));

    // While the club reports up, the bit must stay set (a few sweeps).
    std::thread::sleep(Duration::from_millis(120));
    assert!(read.load(Ordering::Relaxed), "stays available while up");

    // Take it down: hysteresis (2 misses) should drop it within a few sweeps.
    up.store(false, Ordering::Relaxed);
    assert!(
        wait_until(Duration::from_secs(2), || !read.load(Ordering::Relaxed)),
        "prober drops a club that stops answering"
    );

    // Bring it back: recovery is immediate on the first success.
    up.store(true, Ordering::Relaxed);
    assert!(
        wait_until(Duration::from_secs(2), || read.load(Ordering::Relaxed)),
        "prober restores a club that comes back"
    );
}

#[test]
fn unconfirmed_dead_box_drops_on_first_probe() {
    // The startup-seed correction: a box only optimistically seeded up (never
    // verified) must drop on its FIRST failed probe — it can't keep the brain in
    // hand for two sweeps. This is the fix for a launch landing on a down box
    // and hard-failing the first message. One sweep fires immediately; the next
    // is 10s out, so this asserts the drop happened on sweep #1, not via misses.
    let up = Arc::new(AtomicBool::new(false));
    let club: Arc<dyn Club> = Arc::new(FlakyClub {
        up: Arc::clone(&up),
    });
    let bit = Arc::new(AtomicBool::new(true)); // optimistic seed, never confirmed
    let read = Arc::clone(&bit);
    spawn_prober(vec![(club, bit)], Duration::from_secs(10));
    assert!(
        wait_until(Duration::from_millis(500), || !read.load(Ordering::Relaxed)),
        "an unconfirmed, unreachable box must drop on the first sweep"
    );
}

#[test]
fn prober_step_corrects_unproven_seed_but_holds_confirmed() {
    // Unproven optimistic seed (never confirmed) drops on the first failed probe.
    let v = prober_step(false, false, 0);
    assert!(!v.available && !v.confirmed, "unproven seed drops at once");

    // A successful probe always publishes up and confirms (and clears misses).
    let v = prober_step(true, false, 3);
    assert!(v.available && v.confirmed && v.misses == 0);

    // Anti-flap: a CONFIRMED box survives a single miss (1 < threshold of 2)…
    let v = prober_step(false, true, 0);
    assert!(
        v.available && v.misses == 1,
        "one miss must not drop a confirmed box"
    );
    // …but drops once consecutive misses reach the threshold.
    let v = prober_step(false, true, 1);
    assert!(
        !v.available && v.misses == 2,
        "second consecutive miss drops it"
    );

    // The miss counter saturates rather than overflowing the u8.
    let v = prober_step(false, true, u8::MAX);
    assert!(!v.available && v.misses == u8::MAX);
}

// --- transport hardening (pure, no network) ------------------------------

#[test]
fn retryable_statuses_are_transient_only() {
    for code in [408, 425, 429, 500, 502, 503, 504] {
        assert!(is_retryable_status(code), "{code} should retry");
    }
    for code in [200, 400, 401, 403, 404, 409, 422, 501] {
        assert!(!is_retryable_status(code), "{code} should NOT retry");
    }
}

#[test]
fn quota_exhaustion_is_distinguished_from_transient_rate_limit() {
    // The real payload that motivated this: a weekly/monthly plan cap.
    assert!(is_quota_exhausted(
        r#"{"error":{"code":"1310","message":"Weekly/Monthly Limit Exhausted. Your limit will reset at 2026-07-03 11:33:39"}}"#
    ));
    assert!(is_quota_exhausted(
        r#"{"error":{"message":"You have exhausted your monthly quota"}}"#
    ));
    // A transient per-minute limit must stay retryable (must NOT match).
    assert!(!is_quota_exhausted(
        r#"{"error":{"message":"Rate limit exceeded, please try again in 3s"}}"#
    ));
    assert!(!is_quota_exhausted("Too Many Requests"));
}

/// A transient request-window 429 gets a retry ladder independent of the
/// ordinary transport/5xx ladder. This is the production shape emitted by Z.ai
/// when a large GLM fanout briefly outruns account concurrency.
#[test]
fn transient_429_uses_independent_retry_budget() {
    let _guard = env_lock();
    let _ordinary_retries = ScopedEnv::set("ANGEL_HTTP_RETRIES", "1");
    let _rate_retries = ScopedEnv::set("ANGEL_HTTP_RATELIMIT_RETRIES", "2");
    let _rate_backoff = ScopedEnv::set("ANGEL_HTTP_RATELIMIT_BACKOFF_MS", "0");
    let _rate_cap = ScopedEnv::set("ANGEL_HTTP_RATELIMIT_BACKOFF_CAP", "0");
    let _jitter = ScopedEnv::set("ANGEL_HTTP_JITTER", "0");
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_server = Arc::clone(&hits);
    let server = std::thread::spawn(move || {
        for attempt in 0..4 {
            let Ok((mut socket, _)) = listener.accept() else {
                return;
            };
            hits_server.fetch_add(1, Ordering::SeqCst);
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request);
            let (status, body) = match attempt {
                0 => (
                    "503 Service Unavailable",
                    r#"{"error":{"message":"gateway warming"}}"#,
                ),
                1 | 2 => (
                    "429 Too Many Requests",
                    r#"{"error":{"code":"1302","message":"Rate limit reached for requests"}}"#,
                ),
                _ => (
                    "200 OK",
                    r#"{"choices":[{"message":{"role":"assistant","content":"recovered"},"finish_reason":"stop"}]}"#,
                ),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes());
            let _ = socket.flush();
        }
    });

    let club = HttpClub::new("glm", format!("http://{addr}"), "glm-5.3-flash", None);
    let reply = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect("the transient rate window should recover");
    server.join().unwrap();

    match reply {
        ClubReply::Text(text) => assert_eq!(text, "recovered"),
        other => panic!("expected recovered text, got {other:?}"),
    }
    assert_eq!(hits.load(Ordering::SeqCst), 4);
}

/// A weekly/monthly-exhaustion 429 is fatal: the client returns after a single
/// request instead of sleeping through the retry budget on a quota that won't
/// reset for days.
#[test]
fn quota_exhausted_429_fails_fast_without_retry() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_srv = Arc::clone(&hits);
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut sock) = conn else { break };
            hits_srv.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 2048];
            let _ = sock.read(&mut buf);
            let body = r#"{"error":{"code":"1310","message":"Weekly/Monthly Limit Exhausted."}}"#;
            let resp = format!(
                "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes());
            let _ = sock.flush();
        }
    });

    let club = HttpClub::new("LongCat-2.0", format!("http://{addr}"), "m", None);
    let err = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect_err("an exhausted quota is fatal, not retryable");
    assert!(err.contains("429"), "{err}");
    assert!(err.contains("Exhausted"), "{err}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "must not retry a quota that won't reset for days"
    );

    // The 429 armed the circuit breaker: the next call fails instantly with
    // the cooldown message and provider detail, without touching the network,
    // and the club reports itself unavailable/cooling for routing purposes.
    let err2 = club
        .chat(&[ChatMsg::user("hi again")], &[])
        .expect_err("a cooling-down quota gate fails the call immediately");
    assert!(err2.contains("quota exhausted"), "{err2}");
    assert!(err2.contains("Exhausted"), "{err2}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "the gate must short-circuit without a network request"
    );
    assert!(club.quota_cooldown().is_some());
    assert!(
        !club.is_available(),
        "an exhausted link is down for routing"
    );
}

/// The failover trigger fires on real plan-cap payloads (raw 429 body or the
/// gate's own cooldown message) and never on transient limits or unrelated
/// errors — failing over on a plain 429 would double-spend on a healthy link.
#[test]
fn quota_error_predicate_matches_exhaustion_not_transient_429() {
    assert!(error_indicates_quota_exhausted(
        r#"HTTP 429: {"error":{"code":"1310","message":"Weekly/Monthly Limit Exhausted. Your limit will reset at 2026-07-03 11:33:39"}}"#
    ));
    assert!(error_indicates_quota_exhausted(
        "quota exhausted (cooling down, next probe in 60m): Weekly/Monthly Limit Exhausted."
    ));
    assert!(error_indicates_quota_exhausted(
        "HTTP 429: You have hit your usage limit for the week"
    ));
    assert!(!error_indicates_quota_exhausted(
        "HTTP 429: Too Many Requests"
    ));
    assert!(!error_indicates_quota_exhausted("HTTP 500: internal error"));
    assert!(!error_indicates_quota_exhausted(
        "transport error after 3 attempt(s): connection refused"
    ));
}

#[test]
fn auth_error_predicate_matches_rejected_credentials() {
    assert!(error_indicates_auth_failed(
        r#"HTTP 401: {"error":{"code":"1000","message":"身 份 验 证 失 败 。 "}}"#
    ));
    assert!(error_indicates_auth_failed(
        "HTTP 403: {\"error\":\"invalid_api_key\"}"
    ));
    assert!(error_indicates_auth_failed("HTTP 401: Unauthorized"));
    assert!(!error_indicates_auth_failed("HTTP 429: Too Many Requests"));
    assert!(!error_indicates_auth_failed("HTTP 500: internal error"));
}

#[test]
fn backoff_doubles_then_caps_without_overflow() {
    let base = Duration::from_millis(500);
    let cap = Duration::from_secs(8);
    assert_eq!(backoff_delay(base, cap, 0), Duration::from_millis(500));
    assert_eq!(backoff_delay(base, cap, 1), Duration::from_millis(1000));
    assert_eq!(backoff_delay(base, cap, 2), Duration::from_millis(2000));
    assert_eq!(backoff_delay(base, cap, 3), Duration::from_millis(4000));
    // 8000ms would be the next double; it pins to the 8s cap and stays there.
    assert_eq!(backoff_delay(base, cap, 4), cap);
    assert_eq!(backoff_delay(base, cap, 5), cap);
    // A wildly large shift must not overflow — just cap.
    assert_eq!(backoff_delay(base, cap, 1000), cap);
}

#[test]
fn jittered_zero_ratio_is_identity() {
    // ratio 0 preserves the old deterministic backoff exactly.
    let d = Duration::from_millis(4000);
    assert_eq!(jittered(d, 0.0), d);
    assert_eq!(jittered(d, -1.0), d);
}

#[test]
fn jittered_stays_within_equal_jitter_bounds() {
    // With ratio 0.5 the wait must land in [d/2, d] — never longer than the
    // computed backoff (so the cap still holds) and never below the floor
    // (so retries don't busy-spin). Sample many times to exercise the rng.
    let d = Duration::from_millis(4000);
    for _ in 0..1000 {
        let w = jittered(d, 0.5);
        assert!(
            w >= Duration::from_millis(2000) && w <= d,
            "jittered out of [2000,4000]ms bounds: {w:?}"
        );
    }
    // ratio 1.0 = full jitter: floor drops to 0, never exceeds d.
    for _ in 0..1000 {
        let w = jittered(d, 1.0);
        assert!(w <= d, "full jitter exceeded d: {w:?}");
    }
}

#[test]
fn jitter_fraction_is_in_unit_interval() {
    for _ in 0..1000 {
        let f = jitter_fraction();
        assert!((0.0..1.0).contains(&f), "fraction out of [0,1): {f}");
    }
}

// --- FallbackClub doubles ---------------------------------------------
/// Errors from `respond` (so the default chat/chat_streaming error *before*
/// emitting anything — the failover-eligible case).
struct FailClub(&'static str);
impl Club for FailClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Err(format!("{} down", self.0))
    }
    fn label(&self) -> &str {
        self.0
    }
}
/// Records whether it was ever asked to chat (to prove it was/wasn't reached).
struct SpyClub(Arc<AtomicBool>);
impl Club for SpyClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        self.0.store(true, Ordering::Relaxed);
        Ok("spy".to_string())
    }
    fn label(&self) -> &str {
        "spy"
    }
}
/// Emits a token, then dies mid-stream (a reply that can't be replayed).
struct PartialClub;
impl Club for PartialClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Err("partial".to_string())
    }
    fn label(&self) -> &str {
        "partial"
    }
    fn chat_streaming(
        &self,
        _m: &[ChatMsg],
        _t: &[ToolDef],
        _c: &AtomicBool,
        on: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        on(StreamDelta::Content("half "));
        Err("died mid-stream".to_string())
    }
}

struct RouteMetaClub(Mutex<String>);
impl Club for RouteMetaClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("ok".to_string())
    }
    fn label(&self) -> &str {
        "openai"
    }
    fn model_identity(&self) -> Option<String> {
        Some("gpt-5.6-sol".to_string())
    }
    fn reasoning_effort(&self) -> Option<String> {
        self.0.lock().ok().map(|effort| effort.clone())
    }
    fn reasoning_levels(&self) -> &[String] {
        static TEST_LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        TEST_LEVELS.get_or_init(|| vec!["low".to_string(), "ultra".to_string()])
    }
    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        let selected = self
            .reasoning_levels()
            .iter()
            .find(|level| level.eq_ignore_ascii_case(requested))?;
        *self.0.lock().ok()? = selected.clone();
        Some(selected.clone())
    }
    fn token_usage(&self) -> Option<TokenUsage> {
        Some(TokenUsage {
            turns: 2,
            last_input: 10,
            last_output: 4,
            last_reasoning: 2,
            total_input: 30,
            total_output: 9,
            total_reasoning: 5,
        })
    }
}

fn run_stream(club: &dyn Club, cancel: &AtomicBool) -> (Result<ClubReply, String>, String) {
    let mut acc = String::new();
    let r = club.chat_streaming(&[], &[], cancel, &mut |d| {
        if let StreamDelta::Content(t) = d {
            acc.push_str(t);
        }
    });
    (r, acc)
}

#[test]
fn fallback_skips_a_dead_primary() {
    let chain: Vec<Arc<dyn Club>> = vec![
        Arc::new(FailClub("turbo")),
        Arc::new(PracticeClub {
            latency: Duration::ZERO,
        }),
    ];
    let fc = FallbackClub::new(chain);
    assert_eq!(fc.label(), "turbo", "reports the primary label");
    let (r, _) = run_stream(&fc, &AtomicBool::new(false));
    assert!(
        matches!(r, Ok(ClubReply::Text(_))),
        "fell through to the practice swing: {r:?}"
    );
    assert_eq!(
        fc.resolved_route_identity().driver,
        "practice",
        "quality attribution must credit the backend that actually answered"
    );
}

#[test]
fn fallback_preserves_primary_route_metadata_for_the_turn_ledger() {
    let chain: Vec<Arc<dyn Club>> = vec![
        Arc::new(RouteMetaClub(Mutex::new("ultra".to_string()))),
        Arc::new(PracticeClub {
            latency: Duration::ZERO,
        }),
    ];
    let club = FallbackClub::new(chain);
    assert_eq!(club.model_identity().as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(club.reasoning_effort().as_deref(), Some("ultra"));
    assert_eq!(club.reasoning_levels(), vec!["low", "ultra"]);
    assert_eq!(club.set_reasoning_effort("LOW").as_deref(), Some("low"));
    assert_eq!(club.reasoning_effort().as_deref(), Some("low"));
    assert_eq!(club.set_reasoning_effort("invented"), None);
    assert_eq!(club.reasoning_effort().as_deref(), Some("low"));
    let usage = club
        .token_usage()
        .expect("fallback usage delegates/sums chain");
    assert_eq!((usage.total_input, usage.total_output), (30, 9));
    club.respond("hi").unwrap();
    assert_eq!(
        club.resolved_route_identity(),
        RouteIdentity {
            driver: "openai".to_string(),
            model: Some("gpt-5.6-sol".to_string()),
            reasoning_effort: Some("low".to_string()),
        }
    );
}

#[test]
fn parse_reset_duration_handles_bare_and_suffixed() {
    assert_eq!(parse_reset_duration("6"), Some(Duration::from_secs(6)));
    assert_eq!(parse_reset_duration("1s"), Some(Duration::from_secs(1)));
    assert_eq!(parse_reset_duration("6m0s"), Some(Duration::from_secs(360)));
    assert_eq!(
        parse_reset_duration("1h2m3s"),
        Some(Duration::from_secs(3723))
    );
    let ms = parse_reset_duration("20ms").unwrap();
    assert!(ms >= Duration::from_millis(19) && ms <= Duration::from_millis(21));
    assert_eq!(parse_reset_duration(""), None);
    assert_eq!(parse_reset_duration("5x"), None); // unknown unit
    assert_eq!(parse_reset_duration("5m3"), None); // trailing unitless number
}

#[test]
fn parse_rate_limit_headers_reads_remaining_and_reset() {
    use std::collections::HashMap;
    let mut h = HashMap::new();
    h.insert(
        "x-ratelimit-remaining-requests".to_string(),
        "0".to_string(),
    );
    h.insert("x-ratelimit-reset-requests".to_string(), "2s".to_string());
    assert_eq!(
        parse_rate_limit_headers(|k| h.get(k).cloned()),
        Some((0, Duration::from_secs(2)))
    );
    // Missing remaining header → None (proactive backoff never engages).
    let empty: HashMap<String, String> = HashMap::new();
    assert_eq!(parse_rate_limit_headers(|k| empty.get(k).cloned()), None);
    // Remaining present, reset missing → conservative 1s default.
    let mut h2 = HashMap::new();
    h2.insert(
        "x-ratelimit-remaining-requests".to_string(),
        "5".to_string(),
    );
    assert_eq!(
        parse_rate_limit_headers(|k| h2.get(k).cloned()),
        Some((5, Duration::from_secs(1)))
    );
}

#[test]
fn fallback_errors_when_every_club_fails() {
    let chain: Vec<Arc<dyn Club>> = vec![Arc::new(FailClub("a")), Arc::new(FailClub("b"))];
    let fc = FallbackClub::new(chain);
    let (r, _) = run_stream(&fc, &AtomicBool::new(false));
    let e = r.unwrap_err();
    assert!(
        e.contains("[b]") && e.contains("down"),
        "last error surfaces: {e}"
    );
}

#[test]
fn fallback_does_not_replay_after_tokens_emitted() {
    let spied = Arc::new(AtomicBool::new(false));
    let chain: Vec<Arc<dyn Club>> =
        vec![Arc::new(PartialClub), Arc::new(SpyClub(Arc::clone(&spied)))];
    let fc = FallbackClub::new(chain);
    let (r, acc) = run_stream(&fc, &AtomicBool::new(false));
    assert!(r.is_err(), "a half-streamed reply must not be replayed");
    assert_eq!(acc, "half ", "the partial tokens were already shown");
    assert!(
        !spied.load(Ordering::Relaxed),
        "must NOT fall through after output"
    );
}

#[test]
fn fallback_honors_cancel_without_calling_clubs() {
    let spied = Arc::new(AtomicBool::new(false));
    let chain: Vec<Arc<dyn Club>> = vec![Arc::new(SpyClub(Arc::clone(&spied)))];
    let fc = FallbackClub::new(chain);
    let cancel = AtomicBool::new(true);
    let (r, _) = run_stream(&fc, &cancel);
    assert!(r.is_err(), "cancel short-circuits");
    assert!(
        !spied.load(Ordering::Relaxed),
        "no club called once cancelled"
    );
}

// --- Reasoning-effort broadcast double --------------------------------
/// A minimal Club that accepts a fixed set of reasoning-effort values,
/// storing the canonical lowercase one it last accepted (Mutex so the impl
/// stays `&self`). Rejected values leave state untouched and return None.
struct EffortClub {
    label: &'static str,
    accepted: &'static [&'static str],
    effort: Mutex<Option<String>>,
}

impl Club for EffortClub {
    fn label(&self) -> &str {
        self.label
    }

    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("ok".to_string())
    }

    fn reasoning_effort(&self) -> Option<String> {
        self.effort.lock().ok()?.clone()
    }

    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        let canonical = requested.to_ascii_lowercase();
        if self.accepted.iter().any(|v| *v == canonical) {
            *self.effort.lock().ok()? = Some(canonical.clone());
            Some(canonical)
        } else {
            None
        }
    }
}

/// Chat-side double: records the per-call effort each `chat_with_effort`
/// receives, optionally failing so the chain advances to the next member.
struct ChatEffortRecorder {
    label: &'static str,
    fail: bool,
    seen: Mutex<Vec<Option<String>>>,
}

impl Club for ChatEffortRecorder {
    fn label(&self) -> &str {
        self.label
    }

    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("ok".to_string())
    }

    fn chat_with_effort(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        effort: Option<&str>,
    ) -> Result<ClubReply, String> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(effort.map(str::to_string));
        }
        if self.fail {
            Err("boom".to_string())
        } else {
            Ok(ClubReply::Text("ok".to_string()))
        }
    }
}

#[test]
fn fallback_chat_forwards_per_call_effort_to_the_answering_member() {
    let primary = Arc::new(ChatEffortRecorder {
        label: "primary",
        fail: true,
        seen: Mutex::new(Vec::new()),
    });
    let secondary = Arc::new(ChatEffortRecorder {
        label: "secondary",
        fail: false,
        seen: Mutex::new(Vec::new()),
    });
    let fc = FallbackClub::new(vec![
        Arc::clone(&primary) as Arc<dyn Club>,
        Arc::clone(&secondary) as Arc<dyn Club>,
    ]);
    let reply = fc.chat_with_effort(&[ChatMsg::user("hi")], &[], Some("high"));
    assert!(matches!(reply, Ok(ClubReply::Text(t)) if t == "ok"));
    // The seat's effort request rode the call through the failover.
    assert_eq!(
        primary.seen.lock().unwrap().as_slice(),
        &[Some("high".to_string())]
    );
    assert_eq!(
        secondary.seen.lock().unwrap().as_slice(),
        &[Some("high".to_string())]
    );
}

#[test]
fn fallback_broadcasts_accepted_effort_to_every_chain_member() {
    let primary = Arc::new(EffortClub {
        label: "primary",
        accepted: &["low", "medium", "high"],
        effort: Mutex::new(None),
    });
    let secondary = Arc::new(EffortClub {
        label: "secondary",
        accepted: &["low", "medium", "high"],
        effort: Mutex::new(None),
    });
    let fc = FallbackClub::new(vec![
        Arc::clone(&primary) as Arc<dyn Club>,
        Arc::clone(&secondary) as Arc<dyn Club>,
    ]);
    assert_eq!(fc.set_reasoning_effort("high").as_deref(), Some("high"));
    assert_eq!(primary.reasoning_effort().as_deref(), Some("high"));
    assert_eq!(secondary.reasoning_effort().as_deref(), Some("high"));
}

#[test]
fn fallback_rejected_effort_touches_no_member() {
    let primary = Arc::new(EffortClub {
        label: "primary",
        accepted: &["low", "medium", "high"],
        effort: Mutex::new(None),
    });
    let secondary = Arc::new(EffortClub {
        label: "secondary",
        accepted: &["low", "medium", "high"],
        effort: Mutex::new(None),
    });
    let fc = FallbackClub::new(vec![
        Arc::clone(&primary) as Arc<dyn Club>,
        Arc::clone(&secondary) as Arc<dyn Club>,
    ]);
    assert_eq!(fc.set_reasoning_effort("invented").as_deref(), None);
    assert_eq!(primary.reasoning_effort().as_deref(), None);
    assert_eq!(secondary.reasoning_effort().as_deref(), None);
}

#[test]
fn fallback_secondary_rejection_keeps_primary_canonical() {
    let primary = Arc::new(EffortClub {
        label: "primary",
        accepted: &["low", "medium", "high"],
        effort: Mutex::new(None),
    });
    let secondary = Arc::new(EffortClub {
        label: "secondary",
        accepted: &["low"],
        effort: Mutex::new(None),
    });
    let fc = FallbackClub::new(vec![
        Arc::clone(&primary) as Arc<dyn Club>,
        Arc::clone(&secondary) as Arc<dyn Club>,
    ]);
    assert_eq!(fc.set_reasoning_effort("high").as_deref(), Some("high"));
    assert_eq!(primary.reasoning_effort().as_deref(), Some("high"));
    assert_eq!(secondary.reasoning_effort().as_deref(), None);
}

#[test]
fn retry_after_parses_delta_seconds_and_clamps() {
    let cap = Duration::from_secs(8);
    assert_eq!(retry_after(Some("5"), cap), Some(Duration::from_secs(5)));
    assert_eq!(retry_after(Some("  3 "), cap), Some(Duration::from_secs(3)));
    // Over the cap clamps down.
    assert_eq!(retry_after(Some("999"), cap), Some(cap));
    // HTTP-date form and garbage are ignored (fall back to computed backoff).
    assert_eq!(
        retry_after(Some("Wed, 21 Oct 2026 07:28:00 GMT"), cap),
        None
    );
    assert_eq!(retry_after(Some("soon"), cap), None);
    assert_eq!(retry_after(None, cap), None);
}

#[test]
fn extract_api_error_handles_envelope_shapes() {
    let nested = serde_json::json!({ "error": { "message": "model is loading" } });
    assert_eq!(
        extract_api_error(&nested).as_deref(),
        Some("model is loading")
    );

    let stringy = serde_json::json!({ "error": "bad request" });
    assert_eq!(extract_api_error(&stringy).as_deref(), Some("bad request"));

    // An object error with no `message` still yields *something* to surface.
    let coded = serde_json::json!({ "error": { "code": 500 } });
    assert!(extract_api_error(&coded).is_some());

    // A normal, healthy response is not an error.
    let ok = serde_json::json!({ "choices": [ { "message": { "content": "hi" } } ] });
    assert_eq!(extract_api_error(&ok), None);
}

#[test]
fn truncate_json_caps_long_bodies() {
    let small = serde_json::json!({ "a": 1 });
    assert_eq!(truncate_json(&small, 200), small.to_string());

    let big = serde_json::json!({ "junk": "x".repeat(500) });
    let out = truncate_json(&big, 64);
    assert!(out.ends_with('…'));
    assert_eq!(out.chars().count(), 65); // 64 chars + the ellipsis
}

// --- SSE streaming (pure parse/accumulate, no network) -------------------

#[test]
fn sse_line_decoding() {
    match parse_sse_line("data: {\"choices\":[]}") {
        SseEvent::Chunk(v) => assert!(v["choices"].is_array()),
        _ => panic!("expected a chunk"),
    }
    assert!(matches!(parse_sse_line("data: [DONE]"), SseEvent::Done));
    // Keep-alive comment, blank separator, and garbage all ignored.
    assert!(matches!(parse_sse_line(": ping"), SseEvent::Ignore));
    assert!(matches!(parse_sse_line(""), SseEvent::Ignore));
    assert!(matches!(parse_sse_line("data: not json"), SseEvent::Ignore));
}

#[test]
fn stream_accumulates_text_deltas_and_forwards_them() {
    let mut acc = StreamAccumulator::default();
    let mut live = String::new();
    for piece in ["Hel", "lo", " world"] {
        let chunk = serde_json::json!({ "choices": [ { "delta": { "content": piece } } ] });
        if let Some(d) = acc.apply_chunk(&chunk).content {
            live.push_str(&d); // what the UI sink would receive, live
        }
    }
    assert_eq!(live, "Hello world");
    match acc.into_reply(true) {
        ClubReply::Text(t) => assert_eq!(t, "Hello world"),
        _ => panic!("expected text"),
    }
}

#[test]
fn stream_separates_reasoning_from_content() {
    // Reasoning models stream `reasoning_content` separately; it must surface
    // on its own channel and never leak into the final answer.
    let mut acc = StreamAccumulator::default();
    let d1 = acc.apply_chunk(
        &serde_json::json!({ "choices": [ { "delta": { "reasoning_content": "let me think" } } ] }),
    );
    assert_eq!(d1.reasoning.as_deref(), Some("let me think"));
    assert!(d1.content.is_none());
    let d2 =
        acc.apply_chunk(&serde_json::json!({ "choices": [ { "delta": { "content": "42" } } ] }));
    assert_eq!(d2.content.as_deref(), Some("42"));
    assert!(d2.reasoning.is_none());
    match acc.into_reply(true) {
        ClubReply::Text(t) => assert_eq!(t, "42", "reasoning must not pollute the answer"),
        _ => panic!("expected text"),
    }
}

#[test]
fn stream_assembles_tool_call_across_chunks() {
    let mut acc = StreamAccumulator::default();
    // id + name land first; argument JSON streams as fragments.
    let chunks = [
        serde_json::json!({ "choices": [ { "delta": { "tool_calls": [
                { "index": 0, "id": "call_1", "function": { "name": "shell", "arguments": "" } } ] } } ] }),
        serde_json::json!({ "choices": [ { "delta": { "tool_calls": [
                { "index": 0, "function": { "arguments": "{\"cmd\":\"" } } ] } } ] }),
        serde_json::json!({ "choices": [ { "delta": { "tool_calls": [
                { "index": 0, "function": { "arguments": "ls\"}" } } ] } } ] }),
        serde_json::json!({ "choices": [ { "delta": {}, "finish_reason": "tool_calls" } ] }),
    ];
    for c in &chunks {
        acc.apply_chunk(c);
    }
    assert_eq!(acc.finish_reason.as_deref(), Some("tool_calls"));
    match acc.into_reply(true) {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].id, "call_1");
            assert_eq!(calls[0].name, "shell");
            assert_eq!(calls[0].args["cmd"], "ls"); // fragments parsed into JSON
        }
        _ => panic!("expected tool calls"),
    }
}

#[test]
fn stream_prefers_tool_calls_over_preamble_text() {
    // Preamble text then a tool call → Calls wins, matching the blocking path.
    let mut acc = StreamAccumulator::default();
    acc.apply_chunk(
        &serde_json::json!({ "choices": [ { "delta": { "content": "let me check" } } ] }),
    );
    acc.apply_chunk(
        &serde_json::json!({ "choices": [ { "delta": { "tool_calls": [
            { "index": 0, "id": "c", "function": { "name": "grep", "arguments": "{}" } } ] } } ] }),
    );
    assert!(matches!(acc.into_reply(true), ClubReply::Calls(_)));
}

#[test]
fn extracts_xml_function_tool_envelope() {
    let raw = r#"
            </function>
            </tool_call>
            <tool_call>
            <function= delegate extra>{"club":"coder","task":"Inspect repo. Do not edit."}</function>
            </tool_call>
            <tool_call>
            <function=shell>{"cmd":"pwd && ls","timeout":10000}</function>
            </tool_call>
        "#;
    let calls = extract_prose_tool_calls(raw);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].id, "prose_0");
    assert_eq!(calls[0].name, "delegate");
    assert_eq!(calls[0].args["club"], "coder");
    assert_eq!(calls[0].args["task"], "Inspect repo. Do not edit.");
    assert_eq!(calls[1].name, "shell");
    assert_eq!(calls[1].args["cmd"], "pwd && ls");
}

#[test]
fn stream_recovers_xml_function_tool_envelope() {
    let mut acc = StreamAccumulator::default();
    for piece in [
        "<tool_call><function= shell>",
        "{\"cmd\":\"pwd; ls src\"}",
        "</function></tool_call>",
    ] {
        acc.apply_chunk(&serde_json::json!({ "choices": [ { "delta": { "content": piece } } ] }));
    }
    match acc.into_reply(true) {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].id, "prose_0");
            assert_eq!(calls[0].name, "shell");
            assert_eq!(calls[0].args["cmd"], "pwd; ls src");
        }
        _ => panic!("expected recovered prose tool call"),
    }
}

#[test]
fn stream_keeps_wrapper_looking_text_when_no_tools_offered() {
    // The SOTA-MOA quorum failure seen live: a text-only swarm worker's draft
    // *quoted* tool-call syntax, prose recovery turned the draft into Calls,
    // and the swarm rejected it ("requested a tool (none offered)"). With no
    // tools offered there is nothing to execute — the draft must stay Text.
    let mut acc = StreamAccumulator::default();
    acc.apply_chunk(&serde_json::json!({ "choices": [ { "delta": { "content":
            "The parser looks for <tool_call><function=shell>{\"cmd\":\"ls\"}</function></tool_call> wrappers." } } ] }));
    match acc.into_reply(false) {
        ClubReply::Text(t) => assert!(t.contains("<tool_call>")),
        ClubReply::Calls(_) => panic!("tool-less request must never yield recovered calls"),
    }
}

#[test]
fn raw_tool_markup_detector_flags_unexecuted_wrappers_only() {
    // Bare wrappers in chat text — the model "called" a tool that never ran.
    assert!(contains_raw_tool_markup(
        r#"<tool_call name="shell">{"command":"ls"}</tool_call>"#
    ));
    assert!(contains_raw_tool_markup(
        r#"ok <function=shell>{"cmd":"ls"}</function>"#
    ));
    assert!(contains_raw_tool_markup("[TOOL_CALLS][]"));
    assert!(contains_raw_tool_markup(
        r#"<longcat_tool_call>shell
<longcat_arg_key>command</longcat_arg_key>
<longcat_arg_value>find /workspace/fixture-overworld -maxdepth 4 -type f</longcat_arg_value>
</longcat_tool_call>"#
    ));
    assert!(contains_raw_tool_markup(
        r#"angel <longcat_tool_call>shell
<parameter=command>cd /workspace/fixture-overworld && ls -la</parameter>
</function>"#
    ));
    assert!(contains_raw_tool_markup(
        r#"<PARAMETER=command>cd /tmp && pwd</parameter>"#
    ));
    // Shouted tag counts even when its JSON is too mangled to recover.
    assert!(contains_raw_tool_markup(r#"<SHELL>{cmd: pwd}"#));
    // Quoted syntax is exempt: fenced blocks and inline code spans.
    assert!(!contains_raw_tool_markup(
        "never print `<tool_call>` markup as chat text"
    ));
    assert!(!contains_raw_tool_markup(
        "```rust\nrest.find(\"<tool_call\");\n<SHELL>{\"cmd\":\"x\"}\n```"
    ));
    // Ordinary prose, generics, placeholders, comparisons.
    assert!(!contains_raw_tool_markup("use Vec<STRING> where a < b"));
    assert!(!contains_raw_tool_markup("replace <TOKEN> with your key"));
    assert!(!contains_raw_tool_markup("Status report:\n- Workspace: ok"));
}

#[test]
fn stream_recovers_named_attr_tool_call_envelope() {
    // The failure seen live: after a "forcing tool use" nudge the model
    // streamed `<tool_call name="shell">{args}</tool_call>` as content. It
    // must come back as Calls, not as a Text answer full of raw XML.
    let mut acc = StreamAccumulator::default();
    for piece in [
        "I'll inspect the workspace.\n<tool_call name=\"shell\">\n",
        "{\"command\": \"pwd && ls -lah\", \"with_approval\": false}\n",
        "</tool_call>",
    ] {
        acc.apply_chunk(&serde_json::json!({ "choices": [ { "delta": { "content": piece } } ] }));
    }
    match acc.into_reply(true) {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].id, "prose_0");
            assert_eq!(calls[0].name, "shell");
            assert_eq!(calls[0].args["command"], "pwd && ls -lah");
        }
        _ => panic!("expected recovered named-attribute tool call"),
    }
}

#[test]
fn stream_recovers_xml_name_args_tool_envelope() {
    let mut acc = StreamAccumulator::default();
    for piece in [
        "<tool_call>\n<name>shell</name>\n",
        "<args>{\"cmd\":\"cat README.md\"}</args>\n",
        "</tool_call>",
    ] {
        acc.apply_chunk(&serde_json::json!({ "choices": [ { "delta": { "content": piece } } ] }));
    }
    match acc.into_reply(true) {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].id, "prose_0");
            assert_eq!(calls[0].name, "shell");
            assert_eq!(calls[0].args["cmd"], "cat README.md");
        }
        _ => panic!("expected recovered XML name/args tool call"),
    }
}

#[test]
fn extracts_bare_invocation_sequence_only_when_all_tool_syntax() {
    let raw = r#"delegate("coder", "Inspect repo")self_map({"module":"swarm"})"#;
    let calls = extract_prose_tool_calls(raw);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "delegate");
    assert_eq!(calls[0].args["club"], "coder");
    assert_eq!(calls[0].args["task"], "Inspect repo");
    assert_eq!(calls[1].name, "self_map");
    assert_eq!(calls[1].args["module"], "swarm");

    let prose = r#"I might call delegate("coder", "Inspect repo") later."#;
    assert!(
        extract_prose_tool_calls(prose).is_empty(),
        "bare invocations embedded in prose must stay prose"
    );
}

#[test]
fn stream_absorbs_non_streaming_message_shape() {
    // A server that ignored `stream:true` returns the whole `message`; the
    // accumulator still extracts the answer rather than reporting it empty.
    let mut acc = StreamAccumulator::default();
    let full = serde_json::json!({
        "choices": [ { "message": { "content": "complete answer" }, "finish_reason": "stop" } ]
    });
    assert_eq!(
        acc.apply_chunk(&full).content.as_deref(),
        Some("complete answer")
    );
    match acc.into_reply(true) {
        ClubReply::Text(t) => assert_eq!(t, "complete answer"),
        _ => panic!("expected text"),
    }
}

/// End-to-end streaming over a real localhost socket: exercises the whole
/// `chat_streaming` HTTP path (send → `into_reader` → line reads → SSE parse →
/// accumulate → live delta forwarding) against a canned event-stream, with no
/// external network. This covers the integration the pure tests above can't.
#[test]
fn http_club_streams_text_over_a_real_socket() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        // Drain the request so the client's write completes, then stream.
        let mut buf = [0u8; 2048];
        let _ = sock.read(&mut buf);
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            ": keep-alive comment line\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\", world\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        // No Content-Length: `Connection: close` delimits the body by EOF, the
        // way real event-streams keep the socket open then end it.
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = sock.write_all(resp.as_bytes());
    });

    let club = HttpClub::new("test", format!("http://{addr}"), "m", None);
    let mut deltas = Vec::new();
    let cancel = AtomicBool::new(false);
    let reply = club
        .chat_streaming(&[ChatMsg::user("hi")], &[], &cancel, &mut |d| {
            if let StreamDelta::Content(t) = d {
                deltas.push(t.to_string())
            }
        })
        .unwrap();
    server.join().unwrap();

    // Deltas were forwarded live, in order (the keep-alive comment skipped).
    assert_eq!(deltas, vec!["Hello".to_string(), ", world".to_string()]);
    match reply {
        ClubReply::Text(t) => assert_eq!(t, "Hello, world"),
        other => panic!("expected text reply, got {other:?}"),
    }
}

#[test]
fn http_club_marks_prose_when_stream_eof_arrives_without_a_terminal_event() {
    let _guard = env_lock();
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let _ = sock.read(&mut buf);
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"partial answer\"}}]}\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = sock.write_all(response.as_bytes());
    });

    let club = HttpClub::new("test", format!("http://{addr}"), "m", None);
    let mut deltas = Vec::new();
    let reply = club
        .chat_streaming(
            &[ChatMsg::user("hi")],
            &[],
            &AtomicBool::new(false),
            &mut |delta| {
                if let StreamDelta::Content(text) = delta {
                    deltas.push(text.to_string());
                }
            },
        )
        .expect_err("partial prose is a transport failure");
    server.join().unwrap();

    assert_eq!(deltas.first().map(String::as_str), Some("partial answer"));
    assert!(
        deltas
            .last()
            .is_some_and(|text| text.contains("provider stream ended before completion")),
        "the live transcript must expose the disconnect: {deltas:?}"
    );
    assert!(reply.starts_with(INCOMPLETE_STREAM_ERR));
    assert!(reply.contains("partial answer"));
}

#[test]
fn http_club_stream_usage_replaces_repeated_frames_within_one_attempt() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let _ = sock.read(&mut buf);
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1}}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"prompt_tokens_details\":{\"cached_tokens\":2}}}\n\n",
            "data: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = sock.write_all(response.as_bytes());
    });

    let club = HttpClub::new("test", format!("http://{addr}"), "m", None);
    let reply = club
        .chat_streaming(
            &[ChatMsg::user("hi")],
            &[],
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .expect("stream completes");
    server.join().unwrap();
    assert!(matches!(reply, ClubReply::Text(ref text) if text == "ok"));
    assert_eq!(
        club.token_usage(),
        Some(TokenUsage {
            turns: 1,
            last_input: 3,
            last_output: 2,
            last_reasoning: 0,
            total_input: 3,
            total_output: 2,
            total_reasoning: 0,
        })
    );
    assert_eq!(
        club.cache_usage(),
        CacheUsage {
            read_input_tokens: 2,
            read_accounting_responses: 1,
            ..CacheUsage::default()
        }
    );
}

#[test]
fn interrupted_stream_marker_is_visible_and_normalizes_trailing_space() {
    assert_eq!(
        mark_stream_interrupted("useful partial  \n"),
        "useful partial\n\n[response interrupted: provider stream ended before completion]"
    );
}

#[test]
fn http_club_rejects_stream_cutoff_finish_reason_length() {
    // This assertion describes a native provider limit, not an operator cap.
    // Serialize with cap-mutating tests and scrub both cached input sources.
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _per_club = ScopedEnv::unset("ANGEL_TEST_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let _ = sock.read(&mut buf);
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = sock.write_all(resp.as_bytes());
    });

    let club = HttpClub::new("test", format!("http://{addr}"), "m", None);
    let cancel = AtomicBool::new(false);
    let err = club
        .chat_streaming(&[ChatMsg::user("hi")], &[], &cancel, &mut |_| {})
        .expect_err("length finish must fail closed");
    server.join().unwrap();

    assert!(
        err.contains("provider/model reached its native output limit"),
        "{err}"
    );
}

/// Serve one canned raw HTTP response over localhost, returning the base URL
/// and the server handle. Drains the request first, then — after writing —
/// blocks on a final read until the client closes, so a buffered `into_json`
/// finishes reading the body before the socket goes away (no spurious RST).
fn serve_once(response: String) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf);
        let _ = sock.write_all(response.as_bytes());
        let _ = sock.flush();
        // Wait for the client to finish reading and hang up (read → 0).
        let _ = sock.read(&mut buf);
    });
    (format!("http://{addr}"), handle)
}

/// A `200 application/json` fixture with a correct `Content-Length`, so the
/// buffered path reads an exact body rather than depending on EOF timing.
fn json_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// An HTTP error response with a JSON body, for teaching-from-rejection tests.
fn http_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// A SOTA-tuned link keeps a reply cut off at the token cap as usable material
/// (marked) rather than discarding it — the buffered path a MoA proposer uses.
/// This is the direct fix for the MoA "cutting out" on a long instruction.
#[test]
fn buffered_sota_link_keeps_truncated_prose() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _per_club = ScopedEnv::unset("ANGEL_DEEPSEEK_V4_PRO_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let resp = json_response(
        r#"{"choices":[{"message":{"content":"a partial but usable draft"},"finish_reason":"length"}]}"#,
    );
    let (base, handle) = serve_once(resp);
    let club = HttpClub::new("deepseek-v4-pro", base, "m", None).sota_tuned();
    let reply = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect("a SOTA link keeps a truncated prose draft");
    handle.join().unwrap();
    match reply {
        ClubReply::Text(t) => {
            assert!(t.contains("a partial but usable draft"), "{t}");
            assert!(
                t.contains("provider/model reached its native output limit"),
                "{t}"
            );
        }
        ClubReply::Calls(_) => panic!("expected kept prose, not tool calls"),
    }
    assert_eq!(club.truncation_usage().episodes, 1);
    assert_eq!(club.truncation_usage().retained_partials, 1);
}

/// The default (main tool-loop) club still fails closed on a cut-off reply, so
/// a truncated answer never silently reaches the user or a tool-calling turn.
#[test]
fn buffered_untuned_club_rejects_truncated_prose() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _per_club = ScopedEnv::unset("ANGEL_TEST_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let resp = json_response(
        r#"{"choices":[{"message":{"content":"a partial draft"},"finish_reason":"length"}]}"#,
    );
    let (base, handle) = serve_once(resp);
    let club = HttpClub::new("test", base, "m", None);
    let err = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect_err("untuned clubs fail closed on truncation");
    handle.join().unwrap();
    assert!(
        err.contains("provider/model reached its native output limit"),
        "{err}"
    );
}

/// A truncated *tool call* has half-written args, so even a tuned link must
/// fail closed rather than keep it — the safety invariant the fix preserves.
#[test]
fn truncated_tool_call_fails_closed_even_on_sota_link() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _per_club = ScopedEnv::unset("ANGEL_DEEPSEEK_V4_PRO_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let resp = json_response(
        r#"{"choices":[{"message":{"tool_calls":[{"id":"c1","function":{"name":"shell","arguments":"{\"cmd\":\"l"}}]},"finish_reason":"length"}]}"#,
    );
    let (base, handle) = serve_once(resp);
    let club = HttpClub::new("deepseek-v4-pro", base, "m", None).sota_tuned();
    let err = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect_err("a truncated tool call is unsafe to keep");
    handle.join().unwrap();
    assert!(
        err.contains("provider/model reached its native output limit"),
        "{err}"
    );
}

/// Streaming variant: the tuned link keeps the prose it already streamed when
/// the stream ends on `finish_reason=length`.
#[test]
fn stream_sota_link_keeps_truncated_prose() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _per_club = ScopedEnv::unset("ANGEL_DEEPSEEK_V4_PRO_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let resp = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"streamed partial\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let (base, handle) = serve_once(resp);
    let club = HttpClub::new("deepseek-v4-pro", base, "m", None).sota_tuned();
    let cancel = AtomicBool::new(false);
    let reply = club
        .chat_streaming(&[ChatMsg::user("hi")], &[], &cancel, &mut |_| {})
        .expect("a SOTA link keeps a truncated stream");
    handle.join().unwrap();
    match reply {
        ClubReply::Text(t) => {
            assert!(t.contains("streamed partial"), "{t}");
            assert!(
                t.contains("provider/model reached its native output limit"),
                "{t}"
            );
        }
        ClubReply::Calls(_) => panic!("expected kept prose, not tool calls"),
    }
    assert_eq!(club.truncation_usage().episodes, 1);
    assert_eq!(club.truncation_usage().retained_partials, 1);
}

/// SOTA and local links both leave output length provider-native by default;
/// an explicit operator override still wins.
#[test]
fn sota_link_omits_product_side_max_tokens_default() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _tuned = ScopedEnv::unset("ANGEL_MOA_DEFAULTS_PROBE_MAX_TOKENS");
    let _plain = ScopedEnv::unset("ANGEL_MOA_PLAIN_PROBE_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let tuned =
        HttpClub::new("moa-defaults-probe", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    let body = tuned
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build tuned body");
    assert!(
        body.get("max_tokens").is_none(),
        "SOTA requests must leave output length provider-native: {body}"
    );
    assert_eq!(
        tuned.route_metadata().output_budget,
        OutputBudgetPolicy::ProviderNative
    );

    let plain = HttpClub::new("moa-plain-probe", "http://127.0.0.1:9/v1", "m", None);
    let body = plain
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build plain body");
    assert!(
        body.get("max_tokens").is_none(),
        "local fleet body must stay byte-identical: {body}"
    );
    assert_eq!(
        plain.route_metadata().output_budget,
        OutputBudgetPolicy::ProviderNative
    );
}

#[test]
fn max_tokens_env_override_beats_sota_default() {
    // A per-club name nothing else touches, so setting its env key can't race
    // another test's body assertion.
    let key = "ANGEL_MOA_OVERRIDE_PROBE_MAX_TOKENS";
    let _guard = env_lock();
    let _global = ScopedEnv::set("ANGEL_CLUB_MAX_TOKENS", "4096");
    let _per_club = ScopedEnv::set(key, "512");
    resync_max_tokens_env_from_env();
    let tuned =
        HttpClub::new("moa-override-probe", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    let body = tuned.build_body(&[ChatMsg::user("hi")], &[], false);
    assert_eq!(
        body.expect("build body")["max_tokens"],
        serde_json::json!(512)
    );
    let metadata = tuned.route_metadata();
    assert_eq!(
        metadata.output_budget,
        OutputBudgetPolicy::Explicit {
            tokens: 512,
            source: OutputBudgetSource::PerClubEnv,
        }
    );
    assert_eq!(
        metadata.output_budget_provenance.as_deref(),
        Some("ANGEL_MOA_OVERRIDE_PROBE_MAX_TOKENS")
    );
}

#[test]
fn longcat_uses_its_exact_output_budget_prefix() {
    let _guard = env_lock();
    let _global = ScopedEnv::set("ANGEL_CLUB_MAX_TOKENS", "4096");
    let _longcat = ScopedEnv::set("ANGEL_LONGCAT_MAX_TOKENS", "512");
    resync_max_tokens_env_from_env();
    let club = HttpClub::new("longcat", "http://127.0.0.1:9/v1", "m", None);
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build LongCat body");
    assert_eq!(body["max_tokens"], serde_json::json!(512));
    assert_eq!(
        club.route_metadata()
            .output_budget
            .label(club.route_metadata().output_budget_provenance.as_deref()),
        "output: 512 · ANGEL_LONGCAT_MAX_TOKENS"
    );
}

#[test]
fn explicit_512_truncation_names_the_configured_request_cap() {
    let _guard = env_lock();
    let _cap = ScopedEnv::set("ANGEL_CAP_MARKER_PROBE_MAX_TOKENS", "512");
    resync_max_tokens_env_from_env();
    let response = json_response(
        r#"{"choices":[{"message":{"content":"retained partial"},"finish_reason":"length"}]}"#,
    );
    let (base, handle) = serve_once(response);
    let club = HttpClub::new("cap-marker-probe", base, "m", None).sota_tuned();
    let reply = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect("usable capped prose is retained");
    handle.join().unwrap();
    assert!(matches!(
        reply,
        ClubReply::Text(text)
            if text.contains("response incomplete: configured request cap was 512 tokens")
    ));
}

#[test]
fn openrouter_provider_prefix_can_disable_reasoning_for_model_id_labels() {
    let _guard = env_lock();
    let key = "ANGEL_OPENROUTER_REASONING_EFFORT";
    let previous = std::env::var_os(key);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "none") };
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new(
        "tencent/hy3:free",
        "https://openrouter.ai/api/v1",
        "tencent/hy3:free",
        Some("test-key".to_string()),
    )
    .sota_tuned();
    let body = club
        .build_body(&[ChatMsg::user("verify this")], &[], false)
        .expect("build OpenRouter body");
    match previous {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var(key, value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(key) },
    }
    resync_reasoning_effort_env_from_env();
    assert_eq!(body["reasoning_effort"], serde_json::json!("none"));
}

#[test]
fn http_set_reasoning_effort_canonicalizes_and_reports() {
    let _guard = env_lock();
    let _per_club = ScopedEnv::unset("ANGEL_DEEPSEEK_V4_PRO_REASONING_EFFORT");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "m", None);
    assert_eq!(club.set_reasoning_effort("HIGH").as_deref(), Some("high"));
    assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
}

#[test]
fn http_set_reasoning_effort_rejects_unknown_value() {
    let _guard = env_lock();
    let _per_club = ScopedEnv::unset("ANGEL_DEEPSEEK_V4_PRO_REASONING_EFFORT");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "m", None);
    // Establish a known override, then confirm an invented value leaves it intact.
    assert_eq!(
        club.set_reasoning_effort("medium").as_deref(),
        Some("medium")
    );
    assert!(club.set_reasoning_effort("invented").is_none());
    assert_eq!(club.reasoning_effort().as_deref(), Some("medium"));
}

#[test]
fn http_override_beats_env_in_build_body() {
    let _guard = env_lock();
    let _per_club = ScopedEnv::set("ANGEL_DEEPSEEK_V4_PRO_REASONING_EFFORT", "low");
    let _global = ScopedEnv::set("ANGEL_REASONING_EFFORT", "low");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "m", None);
    assert_eq!(club.set_reasoning_effort("high").as_deref(), Some("high"));
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    // Override wins over the env effort.
    assert_eq!(body["reasoning_effort"], serde_json::json!("high"));
}

#[test]
fn http_per_call_effort_beats_stored_override_and_env() {
    let _guard = env_lock();
    let _per_club = ScopedEnv::set("ANGEL_DEEPSEEK_V4_PRO_REASONING_EFFORT", "low");
    let _global = ScopedEnv::set("ANGEL_REASONING_EFFORT", "low");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "m", None);
    assert_eq!(
        club.set_reasoning_effort("medium").as_deref(),
        Some("medium")
    );
    // A seat's per-call request outranks the THINK-deck override and env…
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("high"))
        .expect("build body");
    assert_eq!(body["reasoning_effort"], serde_json::json!("high"));
    // …without writing anything into the club's own state.
    assert_eq!(club.reasoning_effort().as_deref(), Some("medium"));
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert_eq!(body["reasoning_effort"], serde_json::json!("medium"));
}

#[test]
fn http_env_only_build_body_is_unchanged() {
    let _guard = env_lock();
    let _per_club = ScopedEnv::set("ANGEL_DEEPSEEK_V4_PRO_REASONING_EFFORT", "medium");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "m", None);
    // No override: the env effort is sent exactly (byte-identical to the
    // pre-override env-only behavior).
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert_eq!(body["reasoning_effort"], serde_json::json!("medium"));
}

#[test]
fn http_non_reasoning_metadata_suppresses_effort() {
    let _guard = env_lock();
    let _per_club = ScopedEnv::set("ANGEL_DEEPSEEK_V4_PRO_REASONING_EFFORT", "high");
    let _global = ScopedEnv::set("ANGEL_REASONING_EFFORT", "high");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "m", None);
    // Cache a *declared* "no" (a catalog's supported_parameters said so) —
    // cached read only, no probe. Only declared truth may gate.
    club.inject_metadata_for_tests(Metadata {
        supports_reasoning: Some(false),
        ..Default::default()
    });
    assert!(club.reasoning_levels().is_empty());
    assert!(club.set_reasoning_effort("high").is_none());
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert!(body.get("reasoning_effort").is_none());
    // Gates must speak: the env-requested effort was withheld and the gate
    // recorded why, instead of stripping silently.
    let gate = club.effort_gate_usage();
    assert_eq!(gate.withheld, 1);
    let last = gate.last.expect("a withheld effort records its reason");
    assert!(
        last.contains("withheld") && last.contains("catalog"),
        "{last}"
    );
    // A per-seat MoA effort hits the same declared gate and is counted as its
    // own new fact.
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("low"))
        .expect("build body");
    assert!(body.get("reasoning_effort").is_none());
    assert_eq!(club.effort_gate_usage().withheld, 2);
    // The same standing env effort on a later hop is not a new fact — no
    // re-count, so the turn loop notices once instead of every turn.
    let _ = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert_eq!(club.effort_gate_usage().withheld, 3, "seat→env flip is new");
    let _ = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert_eq!(club.effort_gate_usage().withheld, 3);
}

#[test]
fn folklore_names_promote_but_never_declare_no() {
    assert_eq!(folklore_reasoning("deepseek-r1-distill-32b"), Some(true));
    assert_eq!(folklore_reasoning("o3-mini"), Some(true));
    // An unrecognized name is *unknown*, not a declared "no" — the old boolean
    // form silently stripped operator effort for every model on this line.
    assert_eq!(folklore_reasoning("tencent/hy3:free"), None);
    assert_eq!(folklore_reasoning("gemma-4-27b-it"), None);
}

#[test]
fn catalog_capabilities_reads_by_id_declarations() {
    let catalog = serde_json::json!({
        "data": [
            { "id": "a/chat", "context_length": 32_768,
              "supported_parameters": ["temperature", "top_p"] },
            { "id": "b/reasoner", "context_length": 131_072,
              "supported_parameters": ["temperature", "reasoning", "include_reasoning"] },
        ]
    });
    assert_eq!(
        catalog_capabilities(&catalog, "b/reasoner"),
        (Some(131_072), Some(true))
    );
    assert_eq!(
        catalog_capabilities(&catalog, "a/chat"),
        (Some(32_768), Some(false)),
        "a supported_parameters list without reasoning is a real declaration"
    );
    assert_eq!(
        catalog_capabilities(&catalog, "missing/model"),
        (None, None)
    );
    // Single-model servers (vLLM) keep the data[0] window and stay unknown on
    // reasoning — no supported_parameters, no invented declaration.
    let vllm = serde_json::json!({ "data": [ { "id": "m", "max_model_len": 8_192 } ] });
    assert_eq!(catalog_capabilities(&vllm, "m"), (Some(8_192), None));
}

#[test]
fn reasoning_rejection_detection_is_conservative() {
    use ReasoningDialect::*;
    assert!(is_reasoning_field_rejection(
        "HTTP 400: Unknown parameter: 'reasoning_effort'",
        OpenAiEffort
    ));
    assert!(is_reasoning_field_rejection(
        "api error: unexpected field `thinking`",
        GlmThinking
    ));
    assert!(is_reasoning_field_rejection(
        "HTTP 422: chat_template_kwargs is not supported",
        QwenEnableThinking
    ));
    // Server errors, quota, transport: never a capability lesson.
    assert!(!is_reasoning_field_rejection(
        "HTTP 500: reasoning_effort broke",
        OpenAiEffort
    ));
    assert!(!is_reasoning_field_rejection(
        "HTTP 429: slow down",
        OpenAiEffort
    ));
    // A 400 about something else entirely teaches nothing.
    assert!(!is_reasoning_field_rejection(
        "HTTP 400: messages required",
        OpenAiEffort
    ));
    // Naming another dialect's field is not this dialect's lesson.
    assert!(!is_reasoning_field_rejection(
        "HTTP 400: bad thinking",
        OpenAiEffort
    ));
}

#[test]
fn unknown_capability_lights_the_think_deck() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new(
        "t-unknown-cap",
        "http://127.0.0.1:9/v1",
        "friendly-chat",
        None,
    );
    // No cached metadata at all: capability is unknown, and unknown never
    // gates — the deck offers the ladder and the wire carries the choice; a
    // strict server corrects a wrong guess via the rejection lesson.
    assert_eq!(
        club.reasoning_levels(),
        vec!["none", "low", "medium", "high"]
    );
    assert_eq!(club.set_reasoning_effort("HIGH").as_deref(), Some("high"));
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert_eq!(body["reasoning_effort"], serde_json::json!("high"));
    assert_eq!(club.effort_gate_usage(), EffortGateUsage::default());
}

#[test]
fn backend_rejection_teaches_strips_and_retries() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let rejection = r#"{"error":{"message":"Unknown parameter: 'reasoning_effort'"}}"#;
    let ok = r#"{"choices":[{"message":{"content":"hello"},"finish_reason":"stop"}]}"#;
    let (base, requests, handle) = serve_seq(vec![
        http_response("400 Bad Request", rejection),
        json_response(ok),
    ]);
    let club = HttpClub::new("t-reject-probe", base, "m", None);
    assert_eq!(club.set_reasoning_effort("high").as_deref(), Some("high"));
    let reply = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect("the rejection lesson retries without the field");
    match reply {
        ClubReply::Text(t) => assert_eq!(t, "hello"),
        _ => panic!("expected a text reply"),
    }
    let sent = requests.lock().unwrap();
    assert_eq!(sent.len(), 2);
    assert!(sent[0].contains("reasoning_effort"));
    assert!(
        !sent[1].contains("reasoning_effort"),
        "the retry must drop the rejected field"
    );
    drop(sent);
    handle.join().unwrap();
    // The lesson sticks: counted, voiced, and the deck goes dark for this club.
    let gate = club.effort_gate_usage();
    assert_eq!(gate.rejections, 1);
    assert!(gate.last.expect("rejection recorded").contains("rejected"));
    assert!(club.reasoning_levels().is_empty());
    assert!(club.set_reasoning_effort("high").is_none());
    assert!(club.reasoning_effort().is_none());
}

/// Read one full HTTP request (headers + `Content-Length` body) off a socket and
/// return it as a string, so a test can assert on what actually went over the
/// wire (the truncation-retry escalation, notably).
fn read_http_request(sock: &mut std::net::TcpStream) -> String {
    use std::io::Read;
    let mut req = Vec::new();
    let mut buf = [0u8; 1024];
    while let Ok(n) = sock.read(&mut buf) {
        if n == 0 {
            break;
        }
        req.extend_from_slice(&buf[..n]);
        if let Some(pos) = req.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&req[..pos]).to_ascii_lowercase();
            let need = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if req.len() >= pos + 4 + need {
                break;
            }
        }
    }
    String::from_utf8_lossy(&req).into_owned()
}

/// Serve a fixed sequence of canned HTTP responses (one connection each) and
/// capture the raw request that preceded each — the multi-round sibling of
/// [`serve_once`]. Once the sequence is exhausted the listener drops, so any
/// extra connection is refused (a retry loop that over-asks fails visibly).
fn serve_seq(
    responses: Vec<String>,
) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener
        .set_nonblocking(true)
        .expect("fixture listener nonblocking");
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let handle = std::thread::spawn(move || {
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut sock = loop {
                match listener.accept() {
                    Ok((sock, _)) => break sock,
                    Err(err)
                        if err.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    _ => return,
                }
            };
            let _ = sock.set_nonblocking(false);
            let _ = sock.set_read_timeout(Some(Duration::from_secs(1)));
            let _ = sock.set_write_timeout(Some(Duration::from_secs(1)));
            let req = read_http_request(&mut sock);
            captured.lock().unwrap().push(req);
            let _ = sock.write_all(response.as_bytes());
            let _ = sock.flush();
            // Wait for the client to finish reading and hang up (read → 0).
            let mut buf = [0u8; 512];
            let _ = sock.read(&mut buf);
        }
    });
    (format!("http://{addr}"), requests, handle)
}

/// A tool-call reply cut at the token cap — unusable (half-written args), so the
/// club fails closed and the truncation retry has something to recover from.
fn truncated_tool_call_response() -> String {
    json_response(
        r#"{"choices":[{"message":{"tool_calls":[{"id":"c1","function":{"name":"shell","arguments":"{\"cmd\":\"l"}}]},"finish_reason":"length"}]}"#,
    )
}

/// The pure escalation decision follows the current bounded Hermes ladder:
/// exponential 2×/4×/8×/16× asks, omitting duplicate ceiling values.
#[test]
fn truncation_retry_escalation_decision() {
    assert_eq!(
        truncation_retry_schedule(Some(8192), true, 65536, 4),
        vec![16384, 32768, 65536]
    );
    assert_eq!(
        truncation_retry_schedule(Some(30000), true, 65536, 4),
        vec![60000, 65536],
        "the hard ceiling ends the ladder instead of repeating 65536"
    );
    assert_eq!(
        truncation_retry_schedule(Some(8192), true, 65536, 99),
        vec![16384, 32768, 65536],
        "attempt count is hard-capped"
    );
    assert_eq!(
        truncation_retry_schedule(Some(8192), true, 65536, 1),
        vec![16384],
        "one retry remains an exact legacy ablation"
    );
    // No cap on the wire (local fleet body) → never retry: the cut was the
    // backend's own completion limit and a bigger ask can't help.
    assert!(truncation_retry_schedule(None, true, 65536, 4).is_empty());
    // Disabled by the knob.
    assert!(truncation_retry_schedule(Some(8192), false, 65536, 4).is_empty());
    // Already at/above the ceiling — nothing left to escalate.
    assert!(truncation_retry_schedule(Some(65536), true, 65536, 4).is_empty());
    assert!(truncation_retry_schedule(Some(70000), true, 65536, 4).is_empty());
}

/// A provider that holds the SSE stream open with keep-alive comments or empty
/// JSON metadata frames while
/// queueing forever (observed live: z.ai under load — one socket, 35 minutes,
/// zero data) must fail the call at the data deadline, not hang the turn: the
/// pings keep resetting the per-read timeout, so only the stall watchdog can
/// end this.
#[test]
fn stream_stall_watchdog_fails_loudly_on_transport_only_streams() {
    let _guard = env_lock();
    {
        let _stall = ScopedEnv::set("ANGEL_STREAM_STALL_SECS", "1");
        resync_stream_knobs_from_env();
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let _ = read_http_request(&mut sock);
            let _ = sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            // ~3s of pings and empty data frames outlives the 1s stall window;
            // neither is model output, so the client must abort early.
            for _ in 0..30 {
                if sock
                    .write_all(b": ping\n\ndata: {\"choices\":[]}\n\n")
                    .is_err()
                {
                    return; // client hung up — the watchdog fired
                }
                let _ = sock.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        let club = HttpClub::new("t-stall-probe", format!("http://{addr}"), "m", None);
        let started = std::time::Instant::now();
        let err = club
            .chat_streaming(
                &[ChatMsg::user("hi")],
                &[],
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .expect_err("a keep-alive-only stream must fail, not hang");
        assert!(err.contains("stream stalled"), "{err}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "the stall bound must fire well before the pings end ({:?})",
            started.elapsed()
        );
        handle.join().unwrap();
    }
    resync_stream_knobs_from_env();
}

/// Real content deltas must not let a provider stretch one generation without
/// bound. This is distinct from the stall watchdog above: every frame here is
/// valid model output, so only the wall-clock stream deadline can terminate it.
#[test]
fn stream_hard_deadline_bounds_a_provider_that_dribbles_real_data() {
    let _guard = env_lock();
    {
        let _stall = ScopedEnv::set("ANGEL_STREAM_STALL_SECS", "5");
        let _hard = ScopedEnv::set("ANGEL_STREAM_HARD_SECS", "1");
        resync_stream_knobs_from_env();
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let _ = read_http_request(&mut sock);
            let _ = sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            for _ in 0..30 {
                if sock
                    .write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n")
                    .is_err()
                {
                    return;
                }
                let _ = sock.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        let club = HttpClub::new("t-hard-stream-probe", format!("http://{addr}"), "m", None);
        let started = std::time::Instant::now();
        let mut visible = String::new();
        let err = club
            .chat_streaming(
                &[ChatMsg::user("hi")],
                &[],
                &AtomicBool::new(false),
                &mut |delta| {
                    if let StreamDelta::Content(text) = delta {
                        visible.push_str(text);
                    }
                },
            )
            .expect_err("a data-dribbling stream must hit the wall-clock deadline");
        assert!(err.contains("stream hard deadline exceeded"), "{err}");
        assert!(
            !visible.is_empty(),
            "the fixture must send real model deltas"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "the hard bound must fire while data is still arriving ({:?})",
            started.elapsed()
        );
        handle.join().unwrap();
    }
    resync_stream_knobs_from_env();
}

/// Serve one SSE response, then hold the socket open and silent. Detection is
/// the client's job; the helper returns early once the client hangs up so the
/// test does not pay for the whole hold.
fn serve_one_frame_then_silence(frame: &'static str, hold_ms: u64) -> std::net::SocketAddr {
    use std::io::{Read as _, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else {
            return;
        };
        let _ = read_http_request(&mut sock);
        let _ = sock.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        );
        let _ = sock.write_all(frame.as_bytes());
        let _ = sock.flush();
        let _ = sock.set_read_timeout(Some(std::time::Duration::from_millis(100)));
        let mut probe = [0u8; 64];
        let mut waited = 0u64;
        while waited < hold_ms {
            match sock.read(&mut probe) {
                Ok(0) => return,
                Ok(_) => {}
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
            waited += 100;
        }
    });
    addr
}

/// A stall that lands *after* the model already streamed prose is the same
/// incomplete stream as any other missing terminal event: the partial is kept
/// but marked interrupted, and the error carries the class the turn loop uses to
/// decide the hop may be replayed. The read-timeout branch used to return a bare
/// `stream stalled` string here, which read as "not retryable" once text had
/// been emitted — while the identical fault behind keep-alives recovered.
#[test]
fn stream_stall_after_partial_prose_is_an_interrupted_incomplete_stream() {
    let _guard = env_lock();
    {
        let _stall = ScopedEnv::set("ANGEL_STREAM_STALL_SECS", "1");
        let _read = ScopedEnv::set("ANGEL_HTTP_TIMEOUT", "2");
        let _retry = ScopedEnv::set("ANGEL_HTTP_RETRIES", "0");
        resync_stream_knobs_from_env();
        let addr = serve_one_frame_then_silence(
            "data: {\"choices\":[{\"delta\":{\"content\":\"partial prose\"}}]}\n\n",
            2_500,
        );
        let club = HttpClub::new("t-stall-partial", format!("http://{addr}"), "m", None);
        let mut visible = String::new();
        let err = club
            .chat_streaming(
                &[ChatMsg::user("hi")],
                &[],
                &AtomicBool::new(false),
                &mut |delta| {
                    if let StreamDelta::Content(text) = delta {
                        visible.push_str(text);
                    }
                },
            )
            .expect_err("a stalled stream must never return a reply");
        assert!(
            err.starts_with(INCOMPLETE_STREAM_ERR),
            "the turn loop classifies recovery on this prefix: {err}"
        );
        assert!(visible.contains("partial prose"), "{visible}");
        assert!(
            visible.contains(STREAM_INTERRUPTED_SUFFIX),
            "the retained partial must be marked interrupted: {visible}"
        );
    }
    resync_stream_knobs_from_env();
}

/// A stall that arrives with a half-assembled tool call must fail as an
/// incomplete stream that names the discarded call: nothing may be dispatched
/// from severed arguments, and the turn must still be allowed to replay the hop.
#[test]
fn stream_stall_discards_a_partial_tool_call() {
    let _guard = env_lock();
    {
        let _stall = ScopedEnv::set("ANGEL_STREAM_STALL_SECS", "1");
        let _read = ScopedEnv::set("ANGEL_HTTP_TIMEOUT", "2");
        let _retry = ScopedEnv::set("ANGEL_HTTP_RETRIES", "0");
        resync_stream_knobs_from_env();
        let addr = serve_one_frame_then_silence(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"cut\",\
             \"type\":\"function\",\"function\":{\"name\":\"read_file\",\
             \"arguments\":\"{\\\"path\\\":\\\"half\"}}]}}]}\n\n",
            2_500,
        );
        let club = HttpClub::new("t-stall-tool", format!("http://{addr}"), "m", None);
        let err = club
            .chat_streaming(
                &[ChatMsg::user("hi")],
                &[],
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .expect_err("a severed tool call must never become a dispatched reply");
        assert!(
            err.starts_with(INCOMPLETE_STREAM_ERR),
            "the discarded call must stay in the recoverable class: {err}"
        );
        assert!(err.contains("incomplete tool call discarded"), "{err}");
    }
    resync_stream_knobs_from_env();
}

/// Blank frames are not progress: a frame that carries no id, name, or
/// arguments must not reset the data deadline, or a server can park a hop at the
/// wall-clock ceiling by pinging with empty tool-call envelopes. Only a real
/// chunk, reasoning delta, completion, or call fragment counts.
#[test]
fn blank_tool_call_frames_do_not_hold_a_stream_open() {
    let _guard = env_lock();
    {
        let _stall = ScopedEnv::set("ANGEL_STREAM_STALL_SECS", "1");
        let _read = ScopedEnv::set("ANGEL_HTTP_TIMEOUT", "1");
        let _retry = ScopedEnv::set("ANGEL_HTTP_RETRIES", "0");
        resync_stream_knobs_from_env();
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let _ = read_http_request(&mut sock);
            let _ = sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            // Five seconds of frames that carry no new call information: blank
            // envelopes alternating with a repeated id/name and no argument
            // bytes, and no ping line in sight. None of it may reset the data
            // deadline.
            let blank: &[u8] = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{}]}}]}\n\n";
            let repeated: &[u8] = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[\
                {\"index\":0,\"id\":\"same\",\"function\":{\"name\":\"read_file\"}}]}}]}\n\n";
            for n in 0..50 {
                if sock
                    .write_all(if n % 2 == 0 { blank } else { repeated })
                    .is_err()
                {
                    return;
                }
                let _ = sock.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        let club = HttpClub::new("t-blank-frames", format!("http://{addr}"), "m", None);
        let started = std::time::Instant::now();
        let err = club
            .chat_streaming(
                &[ChatMsg::user("hi")],
                &[],
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .expect_err("blank frames alone must not hold a stream open");
        // The envelopes did open a call ("same"/read_file) and then produced no
        // argument bytes, so the honest class is the recoverable incomplete
        // stream that discards the unfinished call — never a reply, and never a
        // bare stall that the turn would refuse to replay.
        assert!(err.starts_with(INCOMPLETE_STREAM_ERR), "{err}");
        assert!(err.contains("incomplete tool call discarded"), "{err}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "the data deadline must fire while blank frames are still arriving ({:?})",
            started.elapsed()
        );
        handle.join().unwrap();
    }
    resync_stream_knobs_from_env();
}

/// Multiple unusable tool-call truncations progressively get more output room;
/// recovery is returned as one ordinary tool call without exposing retries.
#[test]
fn truncated_tool_call_recovers_on_the_progressive_retry_ladder() {
    let _guard = env_lock();
    let _max_tokens = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let full = json_response(
        r#"{"choices":[{"message":{"tool_calls":[{"id":"c1","function":{"name":"shell","arguments":"{\"cmd\":\"ls\"}"}}]},"finish_reason":"stop"}]}"#,
    );
    let (base, requests, handle) = serve_seq(vec![
        truncated_tool_call_response(),
        truncated_tool_call_response(),
        full,
    ]);
    let club = HttpClub::new("truncation-retry-probe", base, "m", None).sota_tuned();
    club.stage_inferred_output_budget_for_test(8192);
    let reply = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect("the progressive retries recover the full tool call");
    assert_eq!(
        club.truncation_usage(),
        TruncationUsage {
            episodes: 1,
            retained_partials: 0,
            retries: 2,
            recoveries: 1,
            failures: 0,
        }
    );
    handle.join().unwrap();
    match reply {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "shell");
            assert_eq!(calls[0].args, serde_json::json!({"cmd":"ls"}));
        }
        other => panic!("expected tool calls, got {other:?}"),
    }
    let reqs = requests.lock().unwrap();
    assert_eq!(reqs.len(), 3, "two retries recover before the hard bound");
    assert!(
        reqs[0].contains("\"max_tokens\":8192"),
        "first request carries the SOTA default: {}",
        reqs[0]
    );
    assert!(
        reqs[1].contains("\"max_tokens\":16384"),
        "the first retry doubles the original cap: {}",
        reqs[1]
    );
    assert!(
        reqs[2].contains("\"max_tokens\":32768"),
        "the second retry quadruples the original cap: {}",
        reqs[2]
    );
}

#[test]
fn structured_tool_call_with_object_arguments_parses_successfully() {
    let response = json_response(
        r#"{"choices":[{"message":{"tool_calls":[{"id":"c1","function":{"name":"shell","arguments":{"command":"ls -la"}}}]},"finish_reason":"stop"}]}"#,
    );
    let (base, _requests, handle) = serve_seq(vec![response]);
    let club = HttpClub::new("object-args-probe", base, "m", None);
    let reply = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect("structured tool call with object args should parse");
    handle.join().unwrap();
    match reply {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "shell");
            assert_eq!(calls[0].args, serde_json::json!({"command":"ls -la"}));
        }
        other => panic!("expected tool calls with object args, got {other:?}"),
    }
}

/// Exhausting all three retries surfaces the original deterministic error and
/// stops after exactly four total requests.
#[test]
fn truncation_retry_ladder_is_hard_bounded_and_keeps_the_original_error() {
    let _guard = env_lock();
    let _max_tokens = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let (base, requests, handle) = serve_seq(vec![
        truncated_tool_call_response(),
        truncated_tool_call_response(),
        truncated_tool_call_response(),
        truncated_tool_call_response(),
    ]);
    let club = HttpClub::new("truncation-retry-twice-probe", base, "m", None).sota_tuned();
    club.stage_inferred_output_budget_for_test(8192);
    let err = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect_err("exhausted truncation recovery still fails closed");
    assert_eq!(
        club.truncation_usage(),
        TruncationUsage {
            episodes: 1,
            retained_partials: 0,
            retries: 3,
            recoveries: 0,
            failures: 1,
        }
    );
    handle.join().unwrap();
    assert!(
        err.contains("configured request cap was 8192 tokens"),
        "{err}"
    );
    assert_eq!(
        requests.lock().unwrap().len(),
        4,
        "one initial request plus three retries, never a loop"
    );
}

/// `ANGEL_TRUNCATION_RETRY=0` restores the old single-shot behavior: the first
/// truncation surfaces immediately and nothing is re-sent.
#[test]
fn truncation_retry_disabled_by_env() {
    let _guard = env_lock();
    let key = "ANGEL_TRUNCATION_RETRY";
    let prev = std::env::var_os(key);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "0") };
    let (base, requests, handle) = serve_seq(vec![truncated_tool_call_response()]);
    let club = HttpClub::new("truncation-retry-off-probe", base, "m", None).sota_tuned();
    let result = club.chat(&[ChatMsg::user("hi")], &[]);
    match prev {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var(key, v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(key) },
    }
    handle.join().unwrap();
    let err = result.expect_err("disabled retry fails closed on the first truncation");
    assert!(
        err.contains("provider/model reached its native output limit"),
        "{err}"
    );
    assert_eq!(requests.lock().unwrap().len(), 1, "no retry when disabled");
    assert_eq!(club.truncation_usage().failures, 1);
}

#[test]
fn truncation_retry_count_env_restores_the_one_retry_control() {
    let _guard = env_lock();
    let _max_tokens = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let key = "ANGEL_TRUNCATION_RETRIES";
    let prev = std::env::var_os(key);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "1") };
    let (base, requests, handle) = serve_seq(vec![
        truncated_tool_call_response(),
        truncated_tool_call_response(),
    ]);
    let club = HttpClub::new("truncation-retry-count-probe", base, "m", None).sota_tuned();
    club.stage_inferred_output_budget_for_test(8192);
    let result = club.chat(&[ChatMsg::user("hi")], &[]);
    match prev {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var(key, value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(key) },
    }
    handle.join().unwrap();
    assert!(
        result
            .expect_err("one retry remains truncated")
            .contains("configured request cap was 8192 tokens")
    );
    assert_eq!(
        club.truncation_usage(),
        TruncationUsage {
            episodes: 1,
            retained_partials: 0,
            retries: 1,
            recoveries: 0,
            failures: 1,
        }
    );
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].contains("\"max_tokens\":16384"));
}

/// The streaming path retries a truncated reply only when nothing has streamed
/// to the caller yet — an emitted stream can't be replayed without duplicating
/// output. Here the first stream dies at the cap before any content, so the
/// retry ladder runs and the caller sees exactly one clean stream.
#[test]
fn stream_truncation_retries_when_nothing_streamed() {
    let _guard = env_lock();
    let _max_tokens = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let truncated = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let full = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"recovered\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let (base, requests, handle) = serve_seq(vec![truncated.clone(), truncated, full]);
    let club = HttpClub::new("stream-truncation-retry-probe", base, "m", None).sota_tuned();
    club.stage_inferred_output_budget_for_test(8192);
    let cancel = AtomicBool::new(false);
    let mut deltas = Vec::new();
    let reply = club
        .chat_streaming(&[ChatMsg::user("hi")], &[], &cancel, &mut |d| {
            if let StreamDelta::Content(t) = d {
                deltas.push(t.to_string())
            }
        })
        .expect("empty truncated streams climb the bounded ladder");
    assert_eq!(
        club.truncation_usage(),
        TruncationUsage {
            episodes: 1,
            retained_partials: 0,
            retries: 2,
            recoveries: 1,
            failures: 0,
        }
    );
    handle.join().unwrap();
    match reply {
        ClubReply::Text(t) => assert_eq!(t, "recovered"),
        other => panic!("expected text, got {other:?}"),
    }
    assert_eq!(
        deltas,
        vec!["recovered".to_string()],
        "no duplicated stream output"
    );
    let reqs = requests.lock().unwrap();
    assert_eq!(reqs.len(), 3, "two blank retries before recovery");
    assert!(
        reqs[1].contains("\"max_tokens\":16384"),
        "the first stream retry doubles the cap: {}",
        reqs[1]
    );
    assert!(reqs[2].contains("\"max_tokens\":32768"));
}

#[test]
fn stream_truncation_never_replays_after_visible_preamble() {
    let _guard = env_lock();
    let partial = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"preparing\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\\\"l\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let (base, requests, handle) = serve_seq(vec![partial]);
    let club = HttpClub::new("visible-stream-truncation-probe", base, "m", None).sota_tuned();
    let cancel = AtomicBool::new(false);
    let mut deltas = Vec::new();
    let error = club
        .chat_streaming(&[ChatMsg::user("hi")], &[], &cancel, &mut |delta| {
            if let StreamDelta::Content(text) = delta {
                deltas.push(text.to_string());
            }
        })
        .expect_err("a half-written tool call still fails closed");
    handle.join().unwrap();

    assert!(
        error.contains("provider/model reached its native output limit"),
        "{error}"
    );
    assert_eq!(deltas, ["preparing"]);
    assert_eq!(
        requests.lock().unwrap().len(),
        1,
        "visible output forbids replay"
    );
    assert_eq!(
        club.truncation_usage(),
        TruncationUsage {
            episodes: 1,
            retained_partials: 0,
            retries: 0,
            recoveries: 0,
            failures: 1,
        }
    );
}

/// `ANGEL_<CLUB>_PROMPT_CACHE` pins the per-club cache key on or off ahead of
/// capability detection; unset keeps the global gate's behavior byte-identical,
/// and the explicit global key still always wins.
#[test]
fn per_club_prompt_cache_pin_overrides_detection() {
    let _guard = env_lock();
    let key = "ANGEL_CACHE_PIN_PROBE_PROMPT_CACHE";
    let prev = std::env::var_os(key);
    let prev_explicit = std::env::var_os("ANGEL_PROMPT_CACHE_KEY");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_PROMPT_CACHE_KEY") };
    resync_prompt_cache_from_env();
    let club = HttpClub::new("cache-pin-probe", "http://127.0.0.1:9/v1", "m", None);

    // Pinned on: the stable per-club key goes out even though the backend never
    // proved cache support (no metadata was ever probed).
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "1") };
    resync_prompt_cache_from_env();
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build pinned-on body");
    let generated_key = body["prompt_cache_key"]
        .as_str()
        .expect("generated cache key is a string")
        .to_string();
    assert!(
        generated_key.starts_with("angel-cache-pin-probe-"),
        "{generated_key}"
    );
    assert!(generated_key.len() <= 64, "{generated_key}");

    let second = HttpClub::new("cache-pin-probe", "http://127.0.0.1:9/v1", "m", None);
    let second_body = second
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build second pinned-on body");
    assert_ne!(
        second_body["prompt_cache_key"], body["prompt_cache_key"],
        "concurrent conversations must not share a cache-routing identity"
    );

    // Pinned off: the key is never sent for this club.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(key, "0") };
    resync_prompt_cache_from_env();
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build pinned-off body");
    assert!(body.get("prompt_cache_key").is_none(), "{body}");

    // …but the explicit global key still always wins, even over a pin-off.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_PROMPT_CACHE_KEY", "explicit-key") };
    resync_prompt_cache_from_env();
    let body = club.build_body(&[ChatMsg::user("hi")], &[], false);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_PROMPT_CACHE_KEY") };
    resync_prompt_cache_from_env();
    assert_eq!(
        body.expect("build explicit-key body")["prompt_cache_key"],
        serde_json::json!("explicit-key")
    );

    // Unset: the global capability-gated default (unknown backend → no key),
    // byte-identical to before the pin existed.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var(key) };
    resync_prompt_cache_from_env();
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build unpinned body");
    assert!(body.get("prompt_cache_key").is_none(), "{body}");

    match prev {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var(key, v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(key) },
    }
    match prev_explicit {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(v) => unsafe { std::env::set_var("ANGEL_PROMPT_CACHE_KEY", v) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_PROMPT_CACHE_KEY") },
    }
    resync_prompt_cache_from_env();
}

/// Measured 2026-08-10: the GLM coding plan (z.ai) and the Kimi Code plan both
/// run automatic prefix caches with OpenAI-dialect `cached_tokens` accounting
/// (paired-probe hits of 1920/1946 and 2030/2030 respectively), so both count
/// as cache-capable and inherit the cache-first defaults alongside DeepSeek;
/// an unmeasured backend stays conservative.
#[test]
fn prompt_cache_capability_covers_measured_glm_kimi_and_deepseek_families() {
    let capable = [
        ("glm", "https://api.z.ai/api/coding/paas/v4", "glm-5.2"),
        ("glm", "https://api.z.ai/api/coding/paas/v4", "glm-5.3"),
        (
            "glm",
            "https://api.z.ai/api/coding/paas/v4",
            "glm-5.3-flash",
        ),
        ("kimi", "https://api.kimi.com/coding/v1", "k3"),
        ("deepseek", "https://api.deepseek.com/v1", "deepseek-v4-pro"),
    ];
    for (name, url, model) in capable {
        let club = HttpClub::new(name, url, model, None);
        assert!(
            club.backend_prompt_cache_capable(),
            "{name} must inherit cache-first defaults"
        );
    }
    let unknown = HttpClub::new(
        "longcat",
        "https://api.longcat.chat/openai/v1",
        "LongCat-Flash-Chat",
        None,
    );
    assert!(
        !unknown.backend_prompt_cache_capable(),
        "no measured cache evidence for longcat yet — stay byte-identical"
    );
}

/// `ANGEL_STREAM_USAGE` asks streaming bodies to carry
/// `stream_options.include_usage` so providers that stay silent unless asked
/// (OpenAI and most gateways) attach cached-token counts to the final SSE
/// chunk. The per-club pin is read first, a buffered request never carries the
/// field, and the unset default keeps the body byte-identical.
#[test]
fn stream_usage_gate_sends_include_usage_only_when_armed() {
    let _guard = env_lock();
    let global = "ANGEL_STREAM_USAGE";
    let pin = "ANGEL_STREAM_USAGE_PROBE_STREAM_USAGE";
    let prev = [std::env::var_os(global), std::env::var_os(pin)];
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var(global) };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var(pin) };
    resync_stream_usage_pins_from_env();
    let club = HttpClub::new("stream-usage-probe", "http://127.0.0.1:9/v1", "m", None);

    // Unset + unknown backend: byte-identical default body.
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], true)
        .expect("build default streaming body");
    assert!(body.get("stream_options").is_none(), "{body}");

    // Unset + detected cache-capable backend: the usage frame is requested by
    // default — a backend with a known prefix cache demonstrably speaks this
    // dialect, and without the frame the cache meter stays blind.
    let capable = HttpClub::new(
        "stream-usage-probe",
        "http://127.0.0.1:9/v1",
        "gpt-4o",
        None,
    );
    let body = capable
        .build_body(&[ChatMsg::user("hi")], &[], true)
        .expect("build capability-default streaming body");
    assert_eq!(
        body["stream_options"],
        serde_json::json!({ "include_usage": true })
    );
    // …and the explicit global off overrides detection.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(global, "0") };
    resync_stream_usage_pins_from_env();
    let body = capable
        .build_body(&[ChatMsg::user("hi")], &[], true)
        .expect("build detection-overridden body");
    assert!(body.get("stream_options").is_none(), "{body}");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var(global) };
    resync_stream_usage_pins_from_env();

    // Global on: the streaming body asks for the usage frame…
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(global, "1") };
    resync_stream_usage_pins_from_env();
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], true)
        .expect("build armed streaming body");
    assert_eq!(
        body["stream_options"],
        serde_json::json!({ "include_usage": true })
    );
    // …but a buffered request never carries it.
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build armed buffered body");
    assert!(body.get("stream_options").is_none(), "{body}");

    // Per-club off beats the global on.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(pin, "0") };
    resync_stream_usage_pins_from_env();
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], true)
        .expect("build pinned-off body");
    assert!(body.get("stream_options").is_none(), "{body}");

    // Per-club on beats the global off.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(global, "0") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(pin, "on") };
    resync_stream_usage_pins_from_env();
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], true)
        .expect("build pinned-on body");
    assert_eq!(
        body["stream_options"],
        serde_json::json!({ "include_usage": true })
    );

    for (key, value) in [global, pin].into_iter().zip(prev) {
        match value {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var(key, v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(key) },
        }
    }
    resync_stream_usage_pins_from_env();
}

#[test]
fn openrouter_claude_marks_only_large_supported_system_prefixes_cacheable() {
    let _guard = env_lock();
    let keys = [
        "ANGEL_OPENROUTER_ANTHROPIC_CACHE",
        "ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR",
        "ANGEL_ANTHROPIC_CACHE_TTL",
    ];
    let previous = keys.map(std::env::var_os);
    for key in keys {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    resync_openrouter_anthropic_cache_from_env();

    let openrouter = HttpClub::new(
        "anthropic/claude-sonnet-4.6",
        "https://openrouter.ai/api/v1",
        "anthropic/claude-sonnet-4.6",
        Some("test-key".into()),
    );
    let large = "stable system prefix ".repeat(300);
    let body = openrouter
        .build_body(
            &[ChatMsg::system(large.clone()), ChatMsg::user("fix it")],
            &[],
            false,
        )
        .unwrap();
    assert_eq!(body["messages"][0]["content"][0]["text"], large);
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert!(
        body["messages"][0]["content"][0]["cache_control"]
            .get("ttl")
            .is_none()
    );
    assert_eq!(openrouter.cache_usage().control_requests, 1);

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_ANTHROPIC_CACHE_TTL", "1h") };
    resync_openrouter_anthropic_cache_from_env();
    let one_hour = openrouter
        .build_body(&[ChatMsg::system(large.clone())], &[], false)
        .unwrap();
    assert_eq!(
        one_hour["messages"][0]["content"][0]["cache_control"]["ttl"],
        "1h"
    );
    assert_eq!(openrouter.cache_usage().control_requests, 2);

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_OPENROUTER_ANTHROPIC_CACHE", "0") };
    resync_openrouter_anthropic_cache_from_env();
    let disabled = openrouter
        .build_body(&[ChatMsg::system(large.clone())], &[], false)
        .unwrap();
    assert!(disabled["messages"][0]["content"].is_string());

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OPENROUTER_ANTHROPIC_CACHE") };
    resync_openrouter_anthropic_cache_from_env();
    let short = openrouter
        .build_body(&[ChatMsg::system("short")], &[], false)
        .unwrap();
    assert!(short["messages"][0]["content"].is_string());

    let local = HttpClub::new(
        "claude-local",
        "http://127.0.0.1:9/v1",
        "claude-local",
        None,
    );
    let local_body = local
        .build_body(&[ChatMsg::system(large.clone())], &[], false)
        .unwrap();
    assert!(local_body["messages"][0]["content"].is_string());

    let non_claude = HttpClub::new(
        "openai/gpt-5",
        "https://openrouter.ai/api/v1",
        "openai/gpt-5",
        Some("test-key".into()),
    );
    let non_claude_body = non_claude
        .build_body(&[ChatMsg::system(large)], &[], false)
        .unwrap();
    assert!(non_claude_body["messages"][0]["content"].is_string());

    for (key, value) in keys.into_iter().zip(previous) {
        match value {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var(key, value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(key) },
        }
    }
    resync_openrouter_anthropic_cache_from_env();
}

/// The OpenRouter Claude path also advances a moving breakpoint on the last
/// plain-text message so the growing conversation body caches — with only the
/// system marker, everything after the preamble was re-billed at full price on
/// every call. Tool-role tails are skipped (their tool_result translation is
/// the gateway's business) and the total-size floor keeps tiny one-shot bodies
/// unmarked and byte-identical.
#[test]
fn openrouter_claude_moving_tail_breakpoint_caches_the_conversation_body() {
    let _guard = env_lock();
    let keys = [
        "ANGEL_OPENROUTER_ANTHROPIC_CACHE",
        "ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR",
        "ANGEL_ANTHROPIC_CACHE_TTL",
    ];
    let previous = keys.map(std::env::var_os);
    for key in keys {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    resync_openrouter_anthropic_cache_from_env();

    let openrouter = HttpClub::new(
        "anthropic/claude-sonnet-4.6",
        "https://openrouter.ai/api/v1",
        "anthropic/claude-sonnet-4.6",
        Some("test-key".into()),
    );
    let large = "stable system prefix ".repeat(300);

    // The last plain-text message carries the moving marker; intermediate
    // conversation messages stay plain strings; one control-request count per
    // request regardless of how many breakpoints were placed.
    let body = openrouter
        .build_body(
            &[
                ChatMsg::system(large.clone()),
                ChatMsg::user("first ask"),
                ChatMsg::assistant("draft answer"),
                ChatMsg::user("now fix it"),
            ],
            &[],
            false,
        )
        .unwrap();
    let messages = body["messages"].as_array().unwrap();
    let last = messages.len() - 1;
    assert_eq!(body["messages"][last]["content"][0]["text"], "now fix it");
    assert_eq!(
        body["messages"][last]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert!(body["messages"][1]["content"].is_string());
    assert!(body["messages"][2]["content"].is_string());
    assert_eq!(openrouter.cache_usage().control_requests, 1);

    // A tool-role tail is never marked; the marker falls back to the last
    // plain-text message before it.
    let body = openrouter
        .build_body(
            &[
                ChatMsg::system(large.clone()),
                ChatMsg::user("run the tool"),
                ChatMsg::tool("call-1", "tool output line"),
            ],
            &[],
            false,
        )
        .unwrap();
    let messages = body["messages"].as_array().unwrap();
    let tool_index = messages
        .iter()
        .position(|message| message["role"] == "tool")
        .expect("tool message present");
    assert!(body["messages"][tool_index]["content"].is_string());
    let user_index = messages
        .iter()
        .position(|message| message["content"][0]["text"] == "run the tool")
        .expect("user message carries the moving marker");
    assert_eq!(
        body["messages"][user_index]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );

    // A tiny one-shot body stays unmarked end to end.
    let short = openrouter
        .build_body(&[ChatMsg::system("short"), ChatMsg::user("hi")], &[], false)
        .unwrap();
    assert!(short["messages"][0]["content"].is_string());
    assert!(short["messages"][1]["content"].is_string());

    for (key, value) in keys.into_iter().zip(previous) {
        match value {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var(key, value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(key) },
        }
    }
    resync_openrouter_anthropic_cache_from_env();
}

#[test]
fn glm_dialect_emits_thinking_block_not_reasoning_effort() {
    let _guard = env_lock();
    let _dialect = ScopedEnv::unset("ANGEL_GLM_REASONING_DIALECT");
    let _per_club = ScopedEnv::unset("ANGEL_GLM_REASONING_EFFORT");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    let _default = ScopedEnv::unset("ANGEL_GLM_THINKING_DEFAULT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new(
        "glm",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.2",
        None,
    );
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("high"))
        .expect("build body");
    assert_eq!(body["thinking"], serde_json::json!({ "type": "enabled" }));
    assert!(body.get("reasoning_effort").is_none());
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("NONE"))
        .expect("build body");
    assert_eq!(body["thinking"], serde_json::json!({ "type": "disabled" }));
    // No effort requested → explicitly disabled (action default). Omitting the
    // field used to leave Z.ai thinking open and burn multi-hour empty hops.
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert_eq!(body["thinking"], serde_json::json!({ "type": "disabled" }));
    assert_eq!(body["clear_thinking"], serde_json::json!(true));
}

#[test]
fn glm_53_flash_keeps_thinking_enabled_and_pins_effort() {
    let _guard = env_lock();
    let _dialect = ScopedEnv::unset("ANGEL_GLM_REASONING_DIALECT");
    let _per_club = ScopedEnv::unset("ANGEL_GLM_REASONING_EFFORT");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    let _default = ScopedEnv::unset("ANGEL_GLM_THINKING_DEFAULT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new(
        "glm-5.3-flash",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.3-flash",
        None,
    );
    assert_eq!(
        club.reasoning_effort().as_deref(),
        Some("low"),
        "flash reports its idle rung `low` when nothing is pinned"
    );
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert_eq!(body["thinking"], serde_json::json!({ "type": "enabled" }));
    assert_eq!(
        body["reasoning_effort"],
        serde_json::json!("low"),
        "flash idles at `low` (arena 2026-09-06: 28 s median vs 64 s on auto): {body}"
    );
    {
        let _pin = ScopedEnv::set("ANGEL_GLM_FLASH_IDLE_EFFORT", "auto");
        let body = club
            .build_body(&[ChatMsg::user("hi")], &[], false)
            .expect("build body");
        assert_eq!(
            body["reasoning_effort"],
            serde_json::json!("low"),
            "the exact-model calibration outranks the provider idle fallback: {body}"
        );
        assert_eq!(
            club.resolved_model_defaults()["reasoning_effort_source"],
            "table 2026-09-09"
        );
    }
    {
        let _pin = ScopedEnv::set("ANGEL_REASONING_EFFORT", "high");
        resync_reasoning_effort_env_from_env();
        // These are launch defaults; a fresh club avoids the previous club's
        // intentional short-lived draw/request effort snapshot.
        let pinned = HttpClub::new(
            "glm-5.3-flash",
            "https://api.z.ai/api/coding/paas/v4",
            "glm-5.3-flash",
            None,
        );
        let body = pinned
            .build_body(&[ChatMsg::user("hi")], &[], false)
            .expect("build body");
        assert_eq!(body["thinking"], serde_json::json!({ "type": "enabled" }));
        assert_eq!(body["reasoning_effort"], serde_json::json!("high"));
        assert_eq!(
            pinned.resolved_model_defaults()["reasoning_effort_source"],
            "env"
        );
    }
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("NONE"))
        .expect("build body");
    assert_eq!(body["thinking"], serde_json::json!({ "type": "enabled" }));
    assert_eq!(body["reasoning_effort"], serde_json::json!("low"));
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("high"))
        .expect("build body");
    assert_eq!(body["thinking"], serde_json::json!({ "type": "enabled" }));
    assert_eq!(body["reasoning_effort"], serde_json::json!("high"));
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("max"))
        .expect("build body");
    assert_eq!(body["thinking"], serde_json::json!({ "type": "enabled" }));
    assert_eq!(body["reasoning_effort"], serde_json::json!("max"));
}

#[test]
fn glm_53_locked_ladder_keeps_max_after_a_glm_52_query() {
    let _guard = env_lock();
    let _dialect = ScopedEnv::unset("ANGEL_GLM_REASONING_DIALECT");
    let _per_club = ScopedEnv::unset("ANGEL_GLM_REASONING_EFFORT");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    let _default = ScopedEnv::unset("ANGEL_GLM_THINKING_DEFAULT");
    resync_reasoning_effort_env_from_env();
    let glm52 = HttpClub::new(
        "glm",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.2",
        None,
    );
    let glm53 = HttpClub::new(
        "glm-5.3",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.3",
        None,
    );
    let flash = HttpClub::new(
        "glm-5.3-flash",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.3-flash",
        None,
    );
    // Query the older binary seat first so a shared cache cannot steal `max`.
    assert_eq!(glm52.reasoning_levels(), vec!["none", "high"]);
    assert!(glm52.set_reasoning_effort("max").is_none());
    assert_eq!(glm53.reasoning_levels(), vec!["low", "high", "max"]);
    assert_eq!(flash.reasoning_levels(), vec!["low", "high", "max"]);
    assert_eq!(glm53.set_reasoning_effort("max").as_deref(), Some("max"));
    assert_eq!(flash.set_reasoning_effort("max").as_deref(), Some("max"));
}

#[test]
fn qwen3_dialect_toggles_enable_thinking() {
    let _guard = env_lock();
    let _dialect = ScopedEnv::unset("ANGEL_ATLAS_REASONING_DIALECT");
    let _per_club = ScopedEnv::unset("ANGEL_ATLAS_REASONING_EFFORT");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new("atlas", "http://127.0.0.1:9/v1", "qwen3.6-27b", None);
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("high"))
        .expect("build body");
    assert_eq!(
        body["chat_template_kwargs"]["enable_thinking"],
        serde_json::json!(true)
    );
    assert!(body.get("reasoning_effort").is_none());
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("none"))
        .expect("build body");
    assert_eq!(
        body["chat_template_kwargs"]["enable_thinking"],
        serde_json::json!(false)
    );
}

#[test]
fn dialect_env_override_forces_openai_spelling() {
    let _guard = env_lock();
    let _dialect = ScopedEnv::set("ANGEL_GLM_REASONING_DIALECT", "openai");
    let _per_club = ScopedEnv::unset("ANGEL_GLM_REASONING_EFFORT");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new(
        "glm",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.2",
        None,
    );
    let body = club
        .build_body_with_effort(&[ChatMsg::user("hi")], &[], false, Some("high"))
        .expect("build body");
    assert_eq!(body["reasoning_effort"], serde_json::json!("high"));
    assert!(body.get("thinking").is_none());
}

#[test]
fn glm_dialect_is_capability_truth_over_name_substring_metadata() {
    let _guard = env_lock();
    let _dialect = ScopedEnv::unset("ANGEL_GLM_REASONING_DIALECT");
    let _per_club = ScopedEnv::unset("ANGEL_GLM_REASONING_EFFORT");
    let _global = ScopedEnv::unset("ANGEL_REASONING_EFFORT");
    resync_reasoning_effort_env_from_env();
    let club = HttpClub::new(
        "glm",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.2",
        None,
    );
    // Even a *declared* "no" (a catalog claim) yields to a resolved GLM
    // dialect: the dialect is stronger capability truth for these backends.
    club.inject_metadata_for_tests(Metadata {
        supports_reasoning: Some(false),
        ..Default::default()
    });
    assert_eq!(club.reasoning_levels(), vec!["none", "high"]);
    assert!(
        club.set_reasoning_effort("medium").is_none(),
        "binary dialect offers two honest rungs"
    );
    assert_eq!(club.set_reasoning_effort("HIGH").as_deref(), Some("high"));
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("build body");
    assert_eq!(body["thinking"], serde_json::json!({ "type": "enabled" }));
}

// ---------------------------------------------------------------------------
// Outbound provisioning (economizer + optimizer, `provision.rs`)
// ---------------------------------------------------------------------------

/// An extraction-shaped ask on a metered link earns a tail-side semantic
/// contract without inventing a numeric output cap; the identical ask on a
/// local fleet link leaves the body byte-identical.
#[test]
fn econ_contract_keeps_extraction_provider_native_by_default() {
    let _guard = env_lock();
    let _econ = ScopedEnv::unset("ANGEL_ECON");
    let _cap = ScopedEnv::unset("ANGEL_ECON_EXTRACT_MAX_TOKENS");
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _stop = ScopedEnv::unset("ANGEL_STOP_SEQS");
    resync_max_tokens_env_from_env();
    let ask = "Summarize the failures as json with fields host and cause. \
               Do not include any additional explanation.";
    let tuned = HttpClub::new("econ-probe", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    let body = tuned
        .build_body(&[ChatMsg::user(ask)], &[], false)
        .expect("tuned body");
    let msgs = body["messages"].as_array().expect("messages");
    let last = msgs.last().expect("nonempty");
    assert_eq!(
        last["role"], "system",
        "contract splices at the tail: {body}"
    );
    assert!(
        last["content"]
            .as_str()
            .unwrap_or_default()
            .contains("angelX output contract"),
        "{body}"
    );
    assert!(body.get("max_tokens").is_none(), "{body}");

    let plain = HttpClub::new("econ-plain-probe", "http://127.0.0.1:9/v1", "m", None);
    let body = plain
        .build_body(&[ChatMsg::user(ask)], &[], false)
        .expect("plain body");
    assert_eq!(
        body["messages"].as_array().expect("messages").len(),
        1,
        "local fleet body must stay byte-identical: {body}"
    );
    assert!(body.get("max_tokens").is_none(), "{body}");
}

#[test]
fn econ_extraction_cap_requires_an_explicit_operator_value() {
    let _guard = env_lock();
    let _econ = ScopedEnv::unset("ANGEL_ECON");
    let _cap = ScopedEnv::set("ANGEL_ECON_EXTRACT_MAX_TOKENS", "4096");
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let ask = "Summarize the failures as json with fields host and cause. \
               Do not include any additional explanation.";
    let tuned =
        HttpClub::new("econ-explicit-probe", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    let body = tuned
        .build_body(&[ChatMsg::user(ask)], &[], false)
        .expect("tuned body");
    assert_eq!(body["max_tokens"], serde_json::json!(4096), "{body}");
}

/// A prose ask on a metered link keeps the provider-native output invariant:
/// no contract, no cap — a bounded budget is only ever paired with a bounded
/// ask.
#[test]
fn econ_leaves_prose_asks_provider_native() {
    let _guard = env_lock();
    let _econ = ScopedEnv::unset("ANGEL_ECON");
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _stop = ScopedEnv::unset("ANGEL_STOP_SEQS");
    resync_max_tokens_env_from_env();
    let tuned = HttpClub::new("econ-prose-probe", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    let body = tuned
        .build_body(
            &[ChatMsg::user(
                "Explain the tradeoffs between LoRA and full fine-tuning.",
            )],
            &[],
            false,
        )
        .expect("body");
    assert!(body.get("max_tokens").is_none(), "{body}");
    assert!(
        !body["messages"]
            .as_array()
            .expect("messages")
            .iter()
            .any(|m| m["content"]
                .as_str()
                .unwrap_or_default()
                .contains("angelX output contract")),
        "{body}"
    );
}

/// Operator stop sequences ride the request; the per-club spelling wins and
/// `\n` escapes are honored.
#[test]
fn stop_seqs_env_applies_per_club() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_STOP_SEQS");
    let _per = ScopedEnv::set("ANGEL_STOP_PROBE_STOP_SEQS", "###,\\n\\n");
    let club = HttpClub::new("stop-probe", "http://127.0.0.1:9/v1", "m", None);
    let body = club
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("body");
    assert_eq!(body["stop"], serde_json::json!(["###", "\n\n"]), "{body}");
}

/// Duplication fires only where it is an *optimizer*: a free link whose
/// backend declared it cannot reason, on a tool-less turn. Metered links and
/// undeclared capability never duplicate by default.
#[test]
fn optimizer_dup_scopes_to_free_declared_nonreasoning() {
    let _guard = env_lock();
    let _dup = ScopedEnv::unset("ANGEL_OPT_DUP");
    let _cap = ScopedEnv::unset("ANGEL_OPT_DUP_MAX_CHARS");
    let meta = Metadata {
        supports_reasoning: Some(false),
        ..Default::default()
    };
    let free = HttpClub::new("dup-probe", "http://127.0.0.1:9/v1", "m", None);
    free.inject_metadata_for_tests(meta);
    let body = free
        .build_body(
            &[ChatMsg::user("what is the capital of France?")],
            &[],
            false,
        )
        .expect("body");
    let msgs = body["messages"].as_array().expect("messages");
    assert_eq!(
        msgs.len(),
        2,
        "free non-reasoning link repeats the ask: {body}"
    );
    assert_eq!(msgs[0]["content"], msgs[1]["content"], "{body}");

    let paid = HttpClub::new("dup-paid-probe", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    paid.inject_metadata_for_tests(meta);
    let body = paid
        .build_body(
            &[ChatMsg::user("what is the capital of France?")],
            &[],
            false,
        )
        .expect("body");
    let users = body["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter(|m| m["role"] == "user")
        .count();
    assert_eq!(users, 1, "duplication costs money on metered links: {body}");

    let unknown = HttpClub::new("dup-unknown-probe", "http://127.0.0.1:9/v1", "m", None);
    let body = unknown
        .build_body(&[ChatMsg::user("hi")], &[], false)
        .expect("body");
    assert_eq!(
        body["messages"].as_array().expect("messages").len(),
        1,
        "undeclared capability never duplicates on a guess: {body}"
    );
}

/// The judge's JSON verdict maps onto a directive; a reasoning verdict
/// deliberately provisions nothing (never squeeze a thinking model).
#[test]
fn judge_json_parses_and_reasoning_provisions_nothing() {
    let d = parse_judge_json(
        r#"Sure: {"task":"extraction","max_tokens":512,"contract":"Emit only the CSV rows.","stop":["\n\n"]}"#,
    )
    .expect("directive");
    assert_eq!(d.max_tokens, Some(512));
    let contract = d.contract.as_deref().unwrap_or_default();
    assert!(
        contract.contains("angelX output contract") && contract.contains("Emit only the CSV rows."),
        "{contract}"
    );
    assert_eq!(d.stop, vec!["\n\n".to_string()]);

    let r = parse_judge_json(r#"{"task":"reasoning","max_tokens":9000}"#).expect("directive");
    assert_eq!(r, ProvisionDirective::default());
    assert!(parse_judge_json("no json here").is_none());
}

/// Whitespace squeezing collapses padded blank runs, is a no-op on already
/// tight text, and refuses to touch anything carrying a code fence.
#[test]
fn econ_squeeze_ws_collapses_padding_but_never_code() {
    assert_eq!(
        econ_squeeze_ws("a  \n\n\n\nb\n"),
        Some("a\n\nb\n".to_string())
    );
    assert_eq!(econ_squeeze_ws("a\n\nb"), None);
    assert!(econ_squeeze_ws("pad   \n\n\n\n```\nx\n```\n").is_none());
}

#[test]
fn effort_scoped_club_cannot_be_overridden_by_a_nested_per_call_request() {
    struct EffortWireRecorder {
        seen: Mutex<Vec<Option<String>>>,
        levels: Vec<String>,
    }

    impl Club for EffortWireRecorder {
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok("unused".to_string())
        }

        fn label(&self) -> &str {
            "effort-wire-recorder"
        }

        fn reasoning_levels(&self) -> &[String] {
            &self.levels
        }

        fn chat_with_effort(
            &self,
            _messages: &[ChatMsg],
            _tools: &[ToolDef],
            effort: Option<&str>,
        ) -> Result<ClubReply, String> {
            self.seen.lock().unwrap().push(effort.map(str::to_string));
            Ok(ClubReply::Text("ok".to_string()))
        }
    }

    let inner = Arc::new(EffortWireRecorder {
        seen: Mutex::new(Vec::new()),
        levels: vec!["low".to_string(), "high".to_string()],
    });
    let (scoped, applied) = scoped_reasoning_effort(Arc::clone(&inner) as Arc<dyn Club>, "low");
    assert_eq!(applied.as_deref(), Some("low"));

    scoped
        .chat_with_effort(&[ChatMsg::user("nested")], &[], Some("high"))
        .expect("scoped request completes");
    assert_eq!(
        inner.seen.lock().unwrap().as_slice(),
        &[Some("low".to_string())],
        "the immutable seat pin must win on the wire"
    );
    assert_eq!(
        scoped.route_identity().reasoning_effort.as_deref(),
        Some("low")
    );
    assert_eq!(
        scoped.resolved_route_identity().reasoning_effort.as_deref(),
        Some("low")
    );
}

/// A reasoning-only streaming reply (raw ` ` with no answer) earns exactly
/// one direct-answer retry; the reminder rides in the request body without
/// touching history, and the recovered answer is returned as plain text.
#[test]
fn reasoning_only_stream_retries_with_direct_answer_reminder() {
    let _guard = env_lock();
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let server = std::thread::spawn(move || {
        let responses: [&str; 2] = [
            concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"<think>only thought\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\" but never answered\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n",
            ),
            concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"the direct answer\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n",
            ),
        ];
        for response in responses {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            captured.lock().unwrap().push(read_http_request(&mut sock));
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                 Connection: close\r\n\r\n{response}"
            );
            let _ = sock.write_all(resp.as_bytes());
            let mut buf = [0u8; 512];
            let _ = sock.read(&mut buf);
        }
    });

    let club = HttpClub::new("test-ro", format!("http://{addr}"), "m", None);
    let cancel = AtomicBool::new(false);
    let reply = club
        .chat_streaming(&[ChatMsg::user("hi")], &[], &cancel, &mut |_| {})
        .expect("the direct-answer retry must recover the reply");
    server.join().unwrap();

    match reply {
        ClubReply::Text(t) => assert_eq!(t, "the direct answer"),
        other => panic!("expected text reply, got {other:?}"),
    }
    let reqs = requests.lock().unwrap();
    assert_eq!(
        reqs.len(),
        2,
        "exactly one retry after a reasoning-only stream"
    );
    assert!(
        !reqs[0].contains("Skip the reasoning"),
        "the first request must not carry the recovery reminder"
    );
    assert!(
        reqs[1].contains("Skip the reasoning"),
        "the retry must carry the direct-answer reminder"
    );
    let retry_body = reqs[1]
        .split_once("\r\n\r\n")
        .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(body).ok())
        .expect("retry request JSON");
    let retry_messages = retry_body["messages"].as_array().expect("retry messages");
    assert_eq!(
        retry_messages[0]["role"], "system",
        "strict Qwen templates require the recovery system block first"
    );
    assert_eq!(retry_messages[1]["role"], "user");
}

/// Two reasoning-only buffered replies in a row: the single retry fires, then
/// the club fails closed with the original reasoning-only error — no loop.
#[test]
fn reasoning_only_buffered_reply_retries_once_then_fails_closed() {
    let _guard = env_lock();
    let reasoning_only = json_response(
        r#"{"choices":[{"message":{"role":"assistant","content":"<think>why</think>"},"finish_reason":"stop"}]}"#,
    );
    let (url, requests, handle) = serve_seq(vec![reasoning_only.clone(), reasoning_only]);
    let club = HttpClub::new("test-ro2", url, "m", None);
    let err = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect_err("two reasoning-only replies must fail closed");
    handle.join().unwrap();
    assert!(
        err.contains("reasoning but no answer"),
        "the original reasoning-only error must surface: {err}"
    );
    let reqs = requests.lock().unwrap();
    assert_eq!(
        reqs.len(),
        2,
        "exactly one retry attempt, then the original error"
    );
    assert!(
        reqs[1].contains("Skip the reasoning"),
        "the retry body must carry the direct-answer reminder"
    );
}

/// The renamed vLLM field must retain the same bounded empty-answer recovery.
#[test]
fn vllm_reasoning_only_buffered_reply_retries_once_then_fails_closed() {
    let _guard = env_lock();
    let reasoning_only = json_response(
        r#"{"choices":[{"message":{"role":"assistant","reasoning":"working","content":null},"finish_reason":"stop"}]}"#,
    );
    let (url, requests, handle) = serve_seq(vec![reasoning_only.clone(), reasoning_only]);
    let club = HttpClub::new("test-vllm-reasoning", url, "m", None);
    let error = club.chat(&[ChatMsg::user("hi")], &[]).unwrap_err();
    handle.join().unwrap();
    assert!(error.contains("reasoning but no answer"), "{error}");
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].contains("Skip the reasoning"));
}

#[test]
fn vllm_reasoning_buffered_answer_preserves_literal_tags() {
    let _guard = env_lock();
    let reply = json_response(
        r#"{"choices":[{"message":{"role":"assistant","reasoning":"working","content":"<think>literal documentation</think>"},"finish_reason":"stop"}]}"#,
    );
    let (url, requests, handle) = serve_seq(vec![reply]);
    let club = HttpClub::new("test-vllm-reasoning", url, "m", None);
    match club.chat(&[ChatMsg::user("hi")], &[]).unwrap() {
        ClubReply::Text(text) => assert_eq!(text, "<think>literal documentation</think>"),
        other => panic!("expected an answer, got {other:?}"),
    }
    handle.join().unwrap();
    assert_eq!(requests.lock().unwrap().len(), 1);
}

/// `ANGEL_REASONING_ONLY_RETRY=0` disables the recovery: one request, original error.
#[test]
fn reasoning_only_retry_disabled_by_env() {
    let _guard = env_lock();
    let _knob = ScopedEnv::set("ANGEL_REASONING_ONLY_RETRY", "0");
    let reasoning_only = json_response(
        r#"{"choices":[{"message":{"role":"assistant","content":"<think>why</think>"},"finish_reason":"stop"}]}"#,
    );
    let (url, requests, handle) = serve_seq(vec![reasoning_only]);
    let club = HttpClub::new("test-ro3", url, "m", None);
    let err = club
        .chat(&[ChatMsg::user("hi")], &[])
        .expect_err("reasoning-only reply must still error when recovery is off");
    handle.join().unwrap();
    assert!(err.contains("reasoning but no answer"), "{err}");
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn learned_output_budget_only_raises_inferred_defaults() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _per = ScopedEnv::unset("ANGEL_BUDGET_LEARN_PROBE_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let club = HttpClub::new("budget-learn-probe", "http://127.0.0.1:9/v1", "m", None);
    let messages = [ChatMsg::user("long tool turn")];
    club.learn_output_budget(32_768);
    assert!(
        club.build_body(&messages, &[], false)
            .unwrap()
            .get("max_tokens")
            .is_none()
    );
    {
        let _saved = ScopedEnv::set("ANGEL_BUDGET_LEARN_PROBE_MAX_TOKENS", "8192");
        resync_max_tokens_env_from_env();
        assert_eq!(
            club.build_body(&messages, &[], false).unwrap()["max_tokens"],
            8192
        );
    }
    resync_max_tokens_env_from_env();
    club.stage_inferred_output_budget_for_test(8192);
    assert_eq!(
        club.build_body(&messages, &[], false).unwrap()["max_tokens"],
        32768
    );
    club.learn_output_budget(16_384);
    club.stage_inferred_output_budget_for_test(8192);
    assert_eq!(
        club.build_body(&messages, &[], false).unwrap()["max_tokens"],
        32768
    );
}

#[test]
fn glm_provider_namespace_survives_model_labels_and_custom_endpoints() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "glm");
    let _key = ScopedEnv::set("ANGEL_GLM_KEY", "owned-fixture-key");
    let _global = ScopedEnv::set("ANGEL_CLUB_MAX_TOKENS", "4096");
    let _cap = ScopedEnv::set("ANGEL_GLM_MAX_TOKENS", "1024");
    let _old_label_cap = ScopedEnv::set("ANGEL_GLM_5_3_FLASH_MAX_TOKENS", "777");
    resync_max_tokens_env_from_env();
    for endpoint in [
        "https://api.z.ai/api/coding/paas/v4",
        "https://open.bigmodel.cn/api/paas/v4",
        "http://127.0.0.1:9/private-gateway",
    ] {
        let _url = ScopedEnv::set("ANGEL_GLM_URL", endpoint);
        for model in ["glm-5.3-flash", "private/custom-glm"] {
            let _model = ScopedEnv::set("ANGEL_GLM_MODEL", model);
            let links = optional_glm_http_clubs();
            let club = &links[0].1;
            assert_eq!(links[0].0, "glm");
            assert_eq!(club.label(), model);
            assert_eq!(club.model_identity().as_deref(), Some(model));
            for (_, club, _) in &links {
                assert_eq!(club.env_namespace(), Some("GLM"));
                let budget = crate::agent::harness::TaskOutputBudget::from_route_metadata(
                    &club.route_metadata(),
                );
                let encoded = serde_json::to_value(budget).unwrap();
                assert_eq!(encoded["policy"], "explicit");
                assert_eq!(encoded["tokens"], 1024);
                assert_eq!(encoded["source"], "per-club-env");
                assert_eq!(encoded["provenance"], "ANGEL_GLM_MAX_TOKENS");
            }
        }
    }
}

#[test]
fn glm_provider_namespace_keeps_global_and_native_budget_fallback() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "glm");
    let _key = ScopedEnv::set("ANGEL_GLM_KEY", "owned-fixture-key");
    let _url = ScopedEnv::set("ANGEL_GLM_URL", "http://127.0.0.1:9/v1");
    let _model = ScopedEnv::set("ANGEL_GLM_MODEL", "glm-5.3-flash");
    let _cap = ScopedEnv::unset("ANGEL_GLM_MAX_TOKENS");
    let _old = ScopedEnv::unset("ANGEL_GLM_5_3_FLASH_MAX_TOKENS");
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    assert_eq!(
        optional_glm_http_clubs()[0]
            .1
            .route_metadata()
            .output_budget,
        OutputBudgetPolicy::ProviderNative
    );
    {
        let _global = ScopedEnv::set("ANGEL_CLUB_MAX_TOKENS", "2048");
        resync_max_tokens_env_from_env();
        assert_eq!(
            optional_glm_http_clubs()[0]
                .1
                .route_metadata()
                .output_budget,
            OutputBudgetPolicy::Explicit {
                tokens: 2048,
                source: OutputBudgetSource::GlobalEnv
            }
        );
    }
}

#[test]
fn glm_provider_namespace_reaches_actual_local_request_body() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let _guard = env_lock();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "glm");
    let _key = ScopedEnv::set("ANGEL_GLM_KEY", "owned-fixture-key");
    let _url = ScopedEnv::set("ANGEL_GLM_URL", &format!("http://{address}/v1"));
    let _model = ScopedEnv::set("ANGEL_GLM_MODEL", "glm-5.3-flash");
    let _cap = ScopedEnv::set("ANGEL_GLM_MAX_TOKENS", "1024");
    let _old = ScopedEnv::unset("ANGEL_GLM_5_3_FLASH_MAX_TOKENS");
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _retry = ScopedEnv::set("ANGEL_HTTP_RETRIES", "0");
    resync_max_tokens_env_from_env();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "no owned local request arrived");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("owned listener failed: {error}"),
            }
        };
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let (head_end, content_length) = loop {
            let mut chunk = [0_u8; 4096];
            let read = socket.read(&mut chunk).unwrap();
            assert!(read > 0);
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&bytes[..end]);
                let length = head
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                break (end + 4, length);
            }
            assert!(bytes.len() < 65536);
        };
        while bytes.len() < head_end + content_length {
            let mut chunk = [0_u8; 4096];
            let read = socket.read(&mut chunk).unwrap();
            assert!(read > 0);
            bytes.extend_from_slice(&chunk[..read]);
        }
        let body: serde_json::Value =
            serde_json::from_slice(&bytes[head_end..head_end + content_length]).unwrap();
        let response = r#"{"choices":[{"message":{"role":"assistant","content":"owned"},"finish_reason":"stop"}]}"#;
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        body
    });
    let links = optional_glm_http_clubs();
    let reply = links[0]
        .1
        .chat(&[ChatMsg::user("owned local request")], &[]);
    let body = server.join().unwrap();
    assert!(reply.is_ok(), "{reply:?}");
    assert_eq!(body["model"], "glm-5.3-flash");
    assert_eq!(body["max_tokens"], 1024);
}

#[test]
fn output_budget_configured_openrouter_namespace_reaches_wire_and_task_receipt() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "openrouter");
    let _key = ScopedEnv::set("ANGEL_OPENROUTER_KEY", "owned-fixture-key");
    let _model = ScopedEnv::set("ANGEL_OPENROUTER_MODEL", "glm-5.3-flash");
    let _global = ScopedEnv::set("ANGEL_CLUB_MAX_TOKENS", "4096");
    let _cap = ScopedEnv::set("ANGEL_OPENROUTER_MAX_TOKENS", "8192");
    let _label_cap = ScopedEnv::set("ANGEL_GLM_5_3_FLASH_MAX_TOKENS", "777");
    let _retry = ScopedEnv::set("ANGEL_HTTP_RETRIES", "0");
    resync_max_tokens_env_from_env();
    let (url, requests, server) = serve_seq(vec![json_response(
        r#"{"choices":[{"message":{"content":"owned answer"},"finish_reason":"stop"}]}"#,
    )]);
    let _url = ScopedEnv::set("ANGEL_OPENROUTER_URL", &url);
    let links = optional_openrouter_http_clubs();
    let club = &links[0].1;
    club.chat(&[ChatMsg::user("owned task")], &[]).unwrap();
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert!(
        requests[0].contains("\"max_tokens\":8192"),
        "selected seat cap must reach wire"
    );
    let encoded = serde_json::to_value(
        crate::agent::harness::TaskOutputBudget::from_route_metadata(&club.route_metadata()),
    )
    .unwrap();
    assert_eq!(encoded["tokens"], 8192);
    assert_eq!(encoded["provenance"], "ANGEL_OPENROUTER_MAX_TOKENS");
    assert_eq!(club.env_namespace(), Some("OPENROUTER"));
    assert_eq!(club.model_identity().as_deref(), Some("glm-5.3-flash"));
}

#[test]
fn output_budget_operator_cap_survives_previously_learned_growth() {
    let _guard = env_lock();
    let _global = ScopedEnv::set("ANGEL_CLUB_MAX_TOKENS", "8192");
    resync_max_tokens_env_from_env();
    let club = HttpClub::new("hard-cap-owned", "http://127.0.0.1:9/v1", "m", None);
    club.learn_output_budget(32768);
    let body = club
        .build_body(&[ChatMsg::user("owned task")], &[], false)
        .unwrap();
    assert_eq!(
        body["max_tokens"], 8192,
        "learning cannot override operator authority"
    );
    assert_eq!(
        club.route_metadata().output_budget,
        OutputBudgetPolicy::Explicit {
            tokens: 8192,
            source: OutputBudgetSource::GlobalEnv
        }
    );
}

fn output_budget_truncation_control(streaming: bool, extraction: bool) {
    let _guard = env_lock();
    let _global = if extraction {
        ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS")
    } else {
        ScopedEnv::set("ANGEL_CLUB_MAX_TOKENS", "8192")
    };
    let _extract = ScopedEnv::set("ANGEL_ECON_EXTRACT_MAX_TOKENS", "8192");
    let _econ = ScopedEnv::set("ANGEL_ECON", "1");
    let _retry = ScopedEnv::set("ANGEL_HTTP_RETRIES", "0");
    let _trunc = ScopedEnv::set("ANGEL_TRUNCATION_RETRY", "1");
    resync_max_tokens_env_from_env();
    let response = if streaming {
        concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
            "data: [DONE]\n\n"
        )
        .into()
    } else {
        truncated_tool_call_response()
    };
    let (url, requests, server) = serve_seq(vec![response]);
    let club = HttpClub::new("hard-cap-owned", url, "m", None).sota_tuned();
    let messages = [ChatMsg::user(if extraction {
        "Return only the JSON, nothing else"
    } else {
        "owned task"
    })];
    let result = if streaming {
        club.chat_streaming(&messages, &[], &AtomicBool::new(false), &mut |_| {})
    } else {
        club.chat(&messages, &[])
    };
    server.join().unwrap();
    assert!(
        result
            .unwrap_err()
            .contains("configured request cap was 8192 tokens")
    );
    assert_eq!(
        club.truncation_usage().retries,
        0,
        "no automatic request may exceed the operator ceiling"
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert!(requests.lock().unwrap()[0].contains("\"max_tokens\":8192"));
}

#[test]
fn output_budget_explicit_nonstream_truncation_never_inflates() {
    output_budget_truncation_control(false, false);
}
#[test]
fn output_budget_explicit_stream_truncation_never_inflates() {
    output_budget_truncation_control(true, false);
}
#[test]
fn output_budget_explicit_extraction_truncation_never_inflates() {
    output_budget_truncation_control(false, true);
}

#[test]
fn output_budget_openrouter_native_and_global_fallback_keep_configured_model() {
    let _guard = env_lock();
    let _enabled = ScopedEnv::set("ANGEL_API_CLUBS", "openrouter");
    let _key = ScopedEnv::set("ANGEL_OPENROUTER_KEY", "owned-fixture-key");
    let _url = ScopedEnv::set("ANGEL_OPENROUTER_URL", "http://127.0.0.1:9/v1");
    let _model = ScopedEnv::set("ANGEL_OPENROUTER_MODEL", "private/custom-model");
    let _cap = ScopedEnv::unset("ANGEL_OPENROUTER_MAX_TOKENS");
    let _label = ScopedEnv::set("ANGEL_PRIVATE_CUSTOM_MODEL_MAX_TOKENS", "777");
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    resync_max_tokens_env_from_env();
    let links = optional_openrouter_http_clubs();
    assert_eq!(
        links[0].1.route_metadata().output_budget,
        OutputBudgetPolicy::ProviderNative
    );
    assert_eq!(
        links[0].1.model_identity().as_deref(),
        Some("private/custom-model")
    );
    let _global = ScopedEnv::set("ANGEL_CLUB_MAX_TOKENS", "4096");
    resync_max_tokens_env_from_env();
    assert_eq!(
        links[0].1.route_metadata().output_budget,
        OutputBudgetPolicy::Explicit {
            tokens: 4096,
            source: OutputBudgetSource::GlobalEnv
        }
    );
}

#[test]
fn output_budget_extraction_limit_outranks_judge_and_learned_defaults() {
    let _guard = env_lock();
    let _global = ScopedEnv::unset("ANGEL_CLUB_MAX_TOKENS");
    let _extract = ScopedEnv::set("ANGEL_ECON_EXTRACT_MAX_TOKENS", "1024");
    let _econ = ScopedEnv::set("ANGEL_ECON", "1");
    resync_max_tokens_env_from_env();
    let club = HttpClub::new("extract-owned", "http://127.0.0.1:9/v1", "m", None).sota_tuned();
    club.learn_output_budget(32768);
    club.stage_inferred_output_budget_for_test(16384);
    let ask = [ChatMsg::user("Return only the JSON, nothing else")];
    assert_eq!(
        club.build_body(&ask, &[], false).unwrap()["max_tokens"],
        1024
    );
    club.stage_inferred_output_budget_for_test(16384);
    let already = [ChatMsg::system("angelX output contract"), ask[0].clone()];
    assert_eq!(
        club.build_body(&already, &[], false).unwrap()["max_tokens"],
        1024
    );
}

// Bounded actual-socket recovery controls. No real provider or host tool effects.
fn provider_recovery_server(
    responses: Vec<(String, bool)>,
) -> (
    String,
    Arc<AtomicBool>,
    Arc<std::sync::atomic::AtomicUsize>,
    std::thread::JoinHandle<()>,
) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let stopped = Arc::clone(&stop);
    let count = Arc::clone(&hits);
    let server = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(6);
        while !stopped.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            let mut socket = match listener.accept() {
                Ok((socket, _)) => socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => panic!("owned provider accept: {error}"),
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let _ = read_http_request(&mut socket);
            let index = count.fetch_add(1, Ordering::SeqCst);
            let Some((response, pings)) = responses.get(index) else {
                let _ = socket.write_all(b"HTTP/1.1 500 Unexpected retry\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                continue;
            };
            if socket.write_all(response.as_bytes()).is_err() {
                continue;
            }
            let _ = socket.flush();
            if *pings {
                for _ in 0..40 {
                    if stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    if socket.write_all(b": ping\n\n").is_err() {
                        break;
                    }
                    let _ = socket.flush();
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    });
    (base, stop, hits, server)
}

struct ProviderRecoveryKnobs;
impl Drop for ProviderRecoveryKnobs {
    fn drop(&mut self) {
        resync_stream_knobs_from_env();
    }
}

#[test]
fn provider_recovery_glm_keepalive_partial_prose_is_marked_interrupted() {
    let _lock = env_lock();
    let _restore = ProviderRecoveryKnobs;
    let _stall = ScopedEnv::set("ANGEL_STREAM_STALL_SECS", "1");
    let _hard = ScopedEnv::set("ANGEL_STREAM_HARD_SECS", "5");
    let _retries = ScopedEnv::set("ANGEL_HTTP_RETRIES", "0");
    resync_stream_knobs_from_env();
    let body = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial answer\"}}],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":2,\"prompt_tokens_details\":{\"cached_tokens\":4}}}\n\n"
    );
    let (base, stop, hits, server) = provider_recovery_server(vec![(body.into(), true)]);
    let club = HttpClub::new("glm", base, "glm-5.3-flash", None);
    let before = club.usage_accounting();
    let began = std::time::Instant::now();
    let (reply, visible) = run_stream(&club, &AtomicBool::new(false));
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(began.elapsed() < Duration::from_secs(4));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    let error = reply.expect_err("interrupted prose must fail transport");
    assert!(error.starts_with(INCOMPLETE_STREAM_ERR));
    assert!(error.contains("partial answer"));
    assert!(
        visible.contains("provider stream ended before completion"),
        "live partial looked complete: {visible}"
    );
    let usage = crate::agent::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(
        (usage.attempts, usage.input, usage.output, usage.cache_read),
        (1, Some(7), Some(2), Some(4))
    );
    assert_eq!(
        usage.total_prompt,
        Some(7),
        "Chat Completions prompt includes cached tokens on loopback"
    );
}

#[test]
fn provider_recovery_glm_cut_tool_is_not_replayed_and_next_call_is_clean() {
    let _lock = env_lock();
    let _retry = ScopedEnv::set("ANGEL_HTTP_RETRIES", "2");
    let first = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"cut\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\"}}]}}],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":2}}\n\n"
    );
    let second = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"fresh\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\\\"owned.txt\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":3}}\n\n",
        "data: [DONE]\n\n"
    );
    let (base, stop, hits, server) =
        provider_recovery_server(vec![(first.into(), false), (second.into(), false)]);
    let club = HttpClub::new("glm", base, "glm-5.3-flash", None);
    let before = club.usage_accounting();
    let first_reply = club.chat_streaming(
        &[ChatMsg::user("owned tool request")],
        &[],
        &AtomicBool::new(false),
        &mut |_| {},
    );
    let first_hits = hits.load(Ordering::SeqCst);
    let second_reply = club.chat_streaming(
        &[ChatMsg::user("fresh operator retry")],
        &[],
        &AtomicBool::new(false),
        &mut |_| {},
    );
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(
        first_reply
            .unwrap_err()
            .contains("incomplete tool call discarded")
    );
    assert_eq!(
        first_hits, 1,
        "partial tool output must not trigger hidden replay"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    let ClubReply::Calls(calls) = second_reply.expect("fresh completed call") else {
        panic!("expected tool call")
    };
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "fresh");
    assert_eq!(calls[0].args, serde_json::json!({"path":"owned.txt"}));
    let usage = crate::agent::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(
        (usage.attempts, usage.input, usage.output),
        (2, Some(18), Some(5))
    );
}

fn provider_recovery_exhausted_status(status: &str, count: usize, usage_expected: bool) {
    let _lock = env_lock();
    let _ordinary = ScopedEnv::set("ANGEL_HTTP_RETRIES", "1");
    let _rate = ScopedEnv::set("ANGEL_HTTP_RATELIMIT_RETRIES", "2");
    let _ordinary_wait = ScopedEnv::set("ANGEL_HTTP_BACKOFF_MS", "0");
    let _rate_wait = ScopedEnv::set("ANGEL_HTTP_RATELIMIT_BACKOFF_MS", "0");
    let _jitter = ScopedEnv::set("ANGEL_HTTP_JITTER", "0");
    let body = if usage_expected {
        r#"{"error":{"code":"1302","message":"Rate limit reached for requests"},"usage":{"prompt_tokens":7,"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":4}}}"#
    } else {
        r#"{"error":{"message":"gateway unavailable"}}"#
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nRetry-After: 0\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let (base, stop, hits, server) = provider_recovery_server(vec![(response, false); count]);
    let club = HttpClub::new("glm", base, "glm-5.3-flash", None);
    let before = club.usage_accounting();
    let began = std::time::Instant::now();
    let reply = club.chat_streaming(
        &[ChatMsg::user("owned retry exhaustion")],
        &[],
        &AtomicBool::new(false),
        &mut |_| {},
    );
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(reply.unwrap_err().contains(&status[..3]));
    assert!(began.elapsed() < Duration::from_secs(4));
    assert_eq!(hits.load(Ordering::SeqCst), count);
    let usage = crate::agent::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
    assert_eq!(usage.attempts, count as u64);
    if usage_expected {
        assert_eq!(
            (usage.input, usage.output, usage.cache_read),
            (Some(21), Some(6), Some(12))
        );
    } else {
        assert_eq!(
            (usage.input, usage.output, usage.cache_read),
            (None, None, None)
        );
        assert!(!usage.core_complete);
    }
    assert_eq!(usage.total_prompt, usage_expected.then_some(21));
}

#[test]
fn provider_recovery_glm_rate_limit_exhaustion_is_bounded() {
    provider_recovery_exhausted_status("429 Too Many Requests", 3, true);
}

#[test]
fn provider_recovery_glm_server_failure_exhaustion_keeps_usage_unknown() {
    provider_recovery_exhausted_status("500 Internal Server Error", 2, false);
}

#[test]
fn provider_recovery_explicit_finish_before_keepalives_commits_text_and_tools() {
    let _lock = env_lock();
    let _restore = ProviderRecoveryKnobs;
    let _stall = ScopedEnv::set("ANGEL_STREAM_STALL_SECS", "1");
    let _hard = ScopedEnv::set("ANGEL_STREAM_HARD_SECS", "5");
    resync_stream_knobs_from_env();
    for tool in [false, true] {
        let frame = if tool {
            serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"finished","function":{"name":"read_file","arguments":"{\"path\":\"owned.txt\"}"}}]},"finish_reason":"tool_calls"}]})
        } else {
            serde_json::json!({"choices":[{"delta":{"content":"complete answer"},"finish_reason":"stop"}]})
        };
        let body = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {frame}\n\n"
        );
        let (base, stop, hits, server) = provider_recovery_server(vec![(body, true)]);
        let club = HttpClub::new("glm", base, "glm-5.3-flash", None);
        let (reply, visible) = run_stream(&club, &AtomicBool::new(false));
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        match reply.expect("explicit finish reason commits without DONE") {
            ClubReply::Text(text) => {
                assert!(!tool);
                assert_eq!(text, "complete answer");
            }
            ClubReply::Calls(calls) => {
                assert!(tool);
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].id, "finished");
            }
        }
        assert!(!visible.contains("provider stream ended before completion"));
    }
}

#[test]
fn auth_matrix_http_bearer_and_redacted_errors() {
    let _lock = crate::tests::env_lock();
    let secret = "fixture-j-auth-secret12";
    unsafe { std::env::set_var("ANGEL_J_AUTH_SECRET", secret) };
    let cases = [
        (
            "200",
            json_response(r#"{"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]}"#),
        ),
        (
            "401",
            http_response(
                "401 Unauthorized",
                r#"{"error":{"message":"bad key fixture-j-auth-secret12"}}"#,
            ),
        ),
        (
            "429",
            http_response(
                "429 Too Many Requests",
                r#"{"error":{"message":"rate fixture-j-auth-secret12"}}"#,
            ),
        ),
        (
            "500",
            http_response(
                "500 Internal Server Error",
                r#"{"error":{"message":"boom fixture-j-auth-secret12"}}"#,
            ),
        ),
        ("malformed", json_response("not-json")),
    ];
    let mut rows = Vec::new();
    for (label, response) in cases {
        let (base, requests, handle) = serve_seq(vec![response]);
        let club = HttpClub::new("auth-matrix", &base, "m", Some(secret.to_string()));
        let result = club.respond("hi");
        let _ = handle.join();
        let captured = requests.lock().unwrap().join("\n");
        assert!(
            captured.contains(&format!("Bearer {secret}")),
            "{label}: Authorization missing secret"
        );
        let err = result.err().unwrap_or_default();
        assert!(
            !err.contains(secret),
            "{label}: secret leaked in error: {err}"
        );
        let identity = crate::agent::harness::run_identity::endpoint_identity(&base);
        assert!(identity.contains("insecure-local"), "{label}: {identity}");
        rows.push(format!("http|{label}|pass|{identity}"));
    }
    // https URL against plain HTTP must fail TLS/handshake without leaking secret
    let (base, _requests, handle) = serve_seq(vec![json_response("{}")]);
    let https = base.replacen("http://", "https://", 1);
    let club = HttpClub::new("auth-matrix-tls", &https, "m", Some(secret.to_string()));
    let err = club.respond("hi").unwrap_err();
    let _ = handle.join();
    assert!(!err.contains(secret), "tls reject leaked secret: {err}");
    rows.push(format!(
        "https|plain-http-reject|pass|{}",
        crate::agent::harness::run_identity::endpoint_identity(&https)
    ));
    if std::env::var_os("ANGEL_J_WRITE_AUTH_MATRIX").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../docs/audits/evidence/2026-09-08-straight-a/J/auth-matrix.md");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut md =
            String::from("# Auth/TLS matrix\n\nadapter|case|result|identity\n---|---|---|---\n");
        for row in &rows {
            md.push_str(row);
            md.push('\n');
        }
        let _ = std::fs::write(path, md);
    }
}

#[test]
fn d06b_practice_identity_records_opt_in() {
    let _guard = crate::tests::env_lock();
    let opted_in = {
        let _practice = ScopedEnv::set("ANGEL_PRACTICE", "1");
        assert_eq!(PracticeClub::new().live_model_name(), None);
        PracticeClub::new().route_identity().model
    };
    let _practice = ScopedEnv::unset("ANGEL_PRACTICE");
    let fallback = PracticeClub::new().route_identity().model;
    assert_eq!(PracticeClub::new().live_model_name(), None);
    assert_eq!(opted_in.as_deref(), Some("practice (ANGEL_PRACTICE=1)"));
    assert_eq!(fallback.as_deref(), Some("practice"));
}

#[test]
fn final_mile_native_drivers_retain_schemas_and_disable_calls() {
    let _guard = env_lock();
    let tools = [ToolDef {
        name: "read_file".into(),
        description: "Read".into(),
        params: serde_json::json!({"type":"object", "properties":{}}),
    }];
    for (label, model) in [
        ("openrouter", "offline-model"),
        ("openai", "gpt-5"),
        ("grok", "grok-4.6"),
    ] {
        let club = HttpClub::new(label, "http://127.0.0.1:9/v1", model, None);
        let mut messages = vec![ChatMsg::system("stable"), ChatMsg::user("work")];
        let before = club.build_body(&messages, &tools, true).unwrap();
        messages.push(ChatMsg::harness(FINAL_MILE_ANSWER_NUDGE));
        let after = club.build_body(&messages, &tools, true).unwrap();
        assert_eq!(before["tools"], after["tools"], "{label}");
        assert_eq!(after["tool_choice"], "none", "{label}");
        assert_eq!(before["model"], after["model"]);
        let old = before["messages"].as_array().unwrap();
        assert_eq!(
            old.as_slice(),
            &after["messages"].as_array().unwrap()[..old.len()]
        );
        messages.push(ChatMsg::user("continue"));
        assert!(!final_response_requested(&messages));
        assert!(
            club.build_body(&messages, &tools, true)
                .unwrap()
                .get("tool_choice")
                .is_none()
        );
    }
    assert!(!final_response_requested(&[ChatMsg::user(
        FINAL_MILE_ANSWER_NUDGE
    )]));
    assert!(!final_response_requested(&[ChatMsg::tool(
        "x",
        FINAL_MILE_ANSWER_NUDGE
    )]));
}

#[test]
fn sota_moa_configured_remote_extra_participates_and_names_both_seats() {
    let _lock = env_lock();
    let _scrub = scrub_sota_moa_env();
    let _always = ScopedEnv::set("ANGEL_SOTA_MOA_ALWAYS", "1");
    let _strict = ScopedEnv::set("ANGEL_SOTA_MOA_STRICT_ROUTES", "1");
    let _primary = ScopedEnv::set("ANGEL_SOTA_MOA_PROPOSE_CLUB", "glm-5.3-flash");
    let _aggregate = ScopedEnv::set("ANGEL_SOTA_MOA_AGG_CLUB", "glm-5.3-flash");
    let _extras = ScopedEnv::set(
        "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
        "glm-5.3,glm-5.3-flash,glm-5.3",
    );
    let _research = ScopedEnv::set("ANGEL_SOTA_MOA_GROK_RESEARCH", "0");
    let _width = ScopedEnv::set("ANGEL_SOTA_MOA_WIDTH", "2");
    struct Seat {
        name: &'static str,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl Club for Seat {
        fn label(&self) -> &str {
            self.name
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(format!("A concrete implementation plan from {}", self.name))
        }
    }
    let primary = Arc::new(Seat {
        name: "glm-5.3-flash",
        calls: Default::default(),
    });
    let second = Arc::new(Seat {
        name: "glm-5.3",
        calls: Default::default(),
    });
    let links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = vec![
        (
            primary.name.into(),
            primary.clone(),
            Arc::new(AtomicBool::new(true)),
        ),
        (
            second.name.into(),
            second.clone(),
            Arc::new(AtomicBool::new(true)),
        ),
    ];
    let formation = sota_moa_club_with_breadth(&links, &[]).unwrap();
    let mut telemetry = String::new();
    formation
        .chat_streaming(
            &[ChatMsg::user("compare two implementation plans")],
            &[],
            &AtomicBool::new(false),
            &mut |delta| {
                if let StreamDelta::Reasoning(text) = delta {
                    telemetry.push_str(text);
                }
            },
        )
        .unwrap();
    assert_eq!(second.calls.load(Ordering::SeqCst), 1);
    assert_eq!(primary.calls.load(Ordering::SeqCst), 2);
    assert!(telemetry.contains("glm-5.3-flash"), "{telemetry}");
    assert!(
        telemetry.contains("propose glm-5.3-flash,glm-5.3 ·"),
        "{telemetry}"
    );
    println!(
        "G02c remote roster: glm-5.3-flash proposer + aggregate; glm-5.3 proposer; both named in telemetry"
    );
}
#[test]
fn model_choices_collapse_shared_aliases_but_preserve_distinct_connections() {
    let mut bag = Bag::for_render_test(&[
        ("provider", &[("same-model", true)]),
        ("sota", &[("same-model", true)]),
    ]);
    assert_eq!(
        bag.route_choices().len(),
        2,
        "distinct connections remain selectable"
    );
    bag.agents[1].slots[0].club = Arc::clone(&bag.agents[0].slots[0].club);
    assert!(bag.select_route(0, 0));
    let choices = bag.route_choices();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].agent_index, 0);
    assert!(choices[0].selected);
    assert!(bag.select_route(1, 0));
    let choices = bag.route_choices();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].agent_index, 1, "keep the active alias selected");
    assert!(choices[0].selected);
    assert_eq!(
        bag.route_choices(),
        choices,
        "cache retains the same visible identity"
    );
}
