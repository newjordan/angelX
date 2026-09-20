use super::*;

#[test]
fn build_request_is_jsonrpc_line() {
    let line = build_request(7, "tools/list", json!({}));
    assert!(line.ends_with('\n'));
    let v: Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 7);
    assert_eq!(v["method"], "tools/list");
}

#[test]
fn build_notification_has_no_id() {
    let line = build_notification("notifications/initialized", json!({}));
    let v: Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(v["method"], "notifications/initialized");
    assert!(v.get("id").is_none());
}

#[test]
fn parse_response_matches_id_and_surfaces_errors() {
    let ok = r#"{"jsonrpc":"2.0","id":3,"result":{"ok":true}}"#;
    assert_eq!(parse_response(ok, 3), Some(Ok(json!({"ok": true}))));
    // wrong id -> skip
    assert_eq!(parse_response(ok, 4), None);
    // a notification (no id) -> skip
    let notif = r#"{"jsonrpc":"2.0","method":"log","params":{}}"#;
    assert_eq!(parse_response(notif, 3), None);
    // error response -> Some(Err)
    let err = r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"no method"}}"#;
    assert_eq!(parse_response(err, 3), Some(Err("no method".to_string())));
    // garbage -> skip
    assert_eq!(parse_response("not json", 3), None);
}

#[test]
fn parse_tools_list_extracts_tools() {
    let result = json!({
        "tools": [
            { "name": "search", "description": "web search", "inputSchema": {"type": "object"} },
            { "name": "fetch" } // missing desc/schema -> defaults
        ]
    });
    let tools = parse_tools_list(&result);
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "search");
    assert_eq!(tools[0].description, "web search");
    assert_eq!(tools[1].name, "fetch");
    assert_eq!(tools[1].description, "");
    assert_eq!(tools[1].schema, json!({"type": "object"}));
}

#[test]
fn parse_tool_content_joins_text_and_flags_errors() {
    let ok =
        json!({ "content": [ {"type":"text","text":"hello"}, {"type":"text","text":"world"} ] });
    assert_eq!(parse_tool_content(&ok), Ok("hello\nworld".to_string()));
    let img = json!({ "content": [ {"type":"image","data":"…"} ] });
    assert_eq!(
        parse_tool_content(&img),
        Ok("[image content omitted]".to_string())
    );
    let err = json!({ "isError": true, "content": [ {"type":"text","text":"boom"} ] });
    assert_eq!(parse_tool_content(&err), Err("boom".to_string()));
}

#[test]
fn parse_mcp_resources_and_prompts() {
    let resources = json!({
        "resources": [
            {
                "uri": "file:///docs/plan.md",
                "name": "plan",
                "mimeType": "text/markdown",
                "description": "Project plan"
            }
        ]
    });
    let listed = parse_resources_list(&resources);
    assert!(listed.contains("file:///docs/plan.md"));
    assert!(listed.contains("Project plan"));

    let read = json!({
        "contents": [
            {
                "uri": "file:///docs/plan.md",
                "mimeType": "text/markdown",
                "text": "# Plan"
            }
        ]
    });
    let text = parse_resource_read(&read);
    assert!(text.contains("file:///docs/plan.md"));
    assert!(text.contains("# Plan"));

    let prompts = json!({
        "prompts": [
            {
                "name": "review",
                "description": "Review a diff",
                "arguments": [
                    { "name": "focus", "required": true },
                    { "name": "depth", "required": false }
                ]
            }
        ]
    });
    let listed_prompts = parse_prompts_list(&prompts);
    assert!(listed_prompts.contains("review(focus*, depth)"));
    assert!(listed_prompts.contains("Review a diff"));

    let prompt = json!({
        "description": "Use this review prompt",
        "messages": [
            { "role": "user", "content": { "type": "text", "text": "Review carefully" } }
        ]
    });
    let got = parse_prompt_get(&prompt);
    assert!(got.contains("Use this review prompt"));
    assert!(got.contains("user: Review carefully"));
}

#[test]
fn mcp_surface_tool_schema_and_arg_validation() {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg("cat >/dev/null")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn_owned()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let client = Arc::new(McpClient {
        name: "mock".to_string(),
        conn: Mutex::new(Conn {
            stdin,
            rx: mpsc::channel().1,
        }),
        next_id: AtomicU64::new(1),
        timeout: Duration::from_millis(1),
        child: Mutex::new(child),
    });
    let provider = Arc::new(McpProvider {
        spec: ServerSpec {
            name: "mock".to_string(),
            command: "/bin/sh".to_string(),
            args: vec![],
            env: vec![],
        },
        timeout: Duration::from_millis(1),
        workspace: None,
        state: Mutex::new(McpProviderState {
            generation: 1,
            client,
        }),
        replacement: Mutex::new(()),
    });
    let tool = McpSurfaceTool {
        provider,
        display_name: "mock__mcp".to_string(),
    };
    let def = tool.def();
    assert_eq!(def.name, "mock__mcp");
    assert!(
        def.params["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "read_resource")
    );
    assert_eq!(
        tool.call(&json!({"action": "read_resource"})),
        Err("uri is required for read_resource".to_string())
    );
    assert_eq!(
        tool.call(&json!({"action": "get_prompt", "name": "x", "arguments": []})),
        Err("arguments must be an object".to_string())
    );
}

#[test]
fn mangle_tool_name_sanitizes() {
    assert_eq!(mangle_tool_name("exa", "web.search"), "exa__web_search");
    assert_eq!(mangle_tool_name("my server", "do it!"), "my_server__do_it_");
}

#[test]
fn unhealthy_mcp_errors_only_classify_transport_failures() {
    assert!(is_unhealthy_mcp_error(
        "mock",
        "mcp mock closed the connection"
    ));
    assert!(is_unhealthy_mcp_error(
        "mock",
        "mcp mock timed out on tools/list"
    ));
    assert!(is_unhealthy_mcp_error(
        "mock",
        "mcp mock write: broken pipe"
    ));
    assert!(is_unhealthy_mcp_error("mock", "mcp conn poisoned"));
    assert!(!is_unhealthy_mcp_error(
        "mock",
        "mcp mock: invalid arguments"
    ));
    assert!(!is_unhealthy_mcp_error(
        "mock",
        "mcp mock: remote says write: retry"
    ));
    assert!(!is_unhealthy_mcp_error(
        "other",
        "mcp mock closed the connection"
    ));
}

#[test]
fn parse_mcp_config_reads_claude_desktop_shape() {
    let text = r#"{
            "mcpServers": {
                "exa": { "command": "npx", "args": ["-y", "exa-mcp"], "env": { "EXA_API_KEY": "k" } },
                "playwright": { "command": "npx", "args": ["@playwright/mcp"] },
                "broken": { "args": ["x"] }
            }
        }"#;
    let specs = parse_mcp_config(text);
    assert_eq!(
        specs.len(),
        2,
        "the command-less server is dropped: {specs:?}"
    );
    assert_eq!(specs[0].name, "exa");
    assert_eq!(specs[0].command, "npx");
    assert_eq!(specs[0].args, vec!["-y", "exa-mcp"]);
    assert_eq!(
        specs[0].env,
        vec![("EXA_API_KEY".to_string(), "k".to_string())]
    );
    assert_eq!(specs[1].name, "playwright");
}

#[test]
fn parse_mcp_config_tolerates_garbage() {
    assert!(parse_mcp_config("not json").is_empty());
    assert!(parse_mcp_config("{}").is_empty());
    assert!(parse_mcp_config(r#"{"mcpServers": {}}"#).is_empty());
}

#[test]
fn server_command_withholds_inherited_secrets_and_applies_spec_env_after() {
    use std::ffi::OsStr;
    let spec = ServerSpec {
        name: "exa".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), "env".into()],
        env: vec![("EXA_API_KEY".into(), "from-mcp-json".into())],
    };
    // The parent's environment: provider keys the server must not see, a
    // same-named key the spec overrides, and ordinary variables.
    let inherited = || {
        ["OPENAI_API_KEY", "GH_TOKEN", "EXA_API_KEY", "HOME", "PATH"]
            .into_iter()
            .map(OsString::from)
    };

    let command = server_command(&spec, inherited(), true);
    assert_eq!(command.get_program(), OsStr::new("/bin/sh"));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [OsStr::new("-c"), OsStr::new("env")]
    );
    let envs: HashMap<&OsStr, Option<&OsStr>> = command.get_envs().collect();
    assert_eq!(envs.get(OsStr::new("OPENAI_API_KEY")), Some(&None));
    assert_eq!(envs.get(OsStr::new("GH_TOKEN")), Some(&None));
    assert!(!envs.contains_key(OsStr::new("HOME")), "{envs:?}");
    assert!(!envs.contains_key(OsStr::new("PATH")), "{envs:?}");
    assert_eq!(
        envs.get(OsStr::new("EXA_API_KEY")),
        Some(&Some(OsStr::new("from-mcp-json"))),
        "the server's own key from mcp.json survives the strip"
    );

    let passthrough = server_command(&spec, inherited(), false);
    let envs: HashMap<&OsStr, Option<&OsStr>> = passthrough.get_envs().collect();
    assert_eq!(
        envs.len(),
        1,
        "ANGEL_TOOL_STRIP_SECRETS=0 withholds nothing: {envs:?}"
    );
    assert_eq!(
        envs.get(OsStr::new("EXA_API_KEY")),
        Some(&Some(OsStr::new("from-mcp-json")))
    );
}

#[test]
fn strip_inherited_secrets_defaults_on_and_is_not_bypassed_by_yolo() {
    let _guard = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_STRIP_SECRETS");
    assert!(
        crate::yolo::enabled(),
        "fixture: the YOLO profile is active"
    );
    assert!(strip_inherited_secrets());
    let _off = crate::tests::TestEnvGuard::set("ANGEL_TOOL_STRIP_SECRETS", "0");
    assert!(!strip_inherited_secrets());
}

/// End to end through `spawn_in`: a live server child cannot read a
/// secret-named variable the cockpit holds, while the key its spec sets
/// arrives. Ignored with its siblings (spawns /bin/sh).
#[test]
#[ignore = "spawns a /bin/sh mock server; run with --ignored"]
fn spawned_server_child_does_not_see_inherited_secrets() {
    let _guard = crate::tests::env_lock();
    let _leak = crate::tests::TestEnvGuard::set("ANGEL_T_MCP_LEAKED_KEY", "cockpit-secret");
    let _knob = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_STRIP_SECRETS");
    let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"x"}}\n' "$id" ;;
    *'"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"env","description":"env","inputSchema":{"type":"object"}}]}}\n' "$id" ;;
    *'"tools/call"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"leaked=%s own=%s"}]}}\n' "$id" "${ANGEL_T_MCP_LEAKED_KEY:-absent}" "${ANGEL_T_MCP_OWN_KEY:-absent}" ;;
  esac
done
"#;
    let spec = ServerSpec {
        name: "envmock".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![("ANGEL_T_MCP_OWN_KEY".into(), "from-spec".into())],
    };
    let (_provider, tools) = connect_server(&spec, Duration::from_secs(5)).expect("connect");
    assert_eq!(
        tools[0].call(&json!({})).unwrap(),
        "leaked=absent own=from-spec"
    );
}

/// End-to-end round-trip against a tiny mock MCP server written in `sh` —
/// proves spawn + initialize + tools/list + tools/call over real stdio pipes.
/// Ignored by default (depends on /bin/sh); run with `--ignored`.
#[test]
#[ignore = "spawns a /bin/sh mock server; run with --ignored"]
fn live_stdio_roundtrip_against_mock() {
    // A line-oriented JSON-RPC echo: replies to initialize/tools/list/tools/call.
    let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"x"}}\n' "$id" ;;
    *'"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"echo","description":"echoes","inputSchema":{"type":"object"}}]}}\n' "$id" ;;
    *'"tools/call"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"pong"}]}}\n' "$id" ;;
  esac
done
"#;
    let spec = ServerSpec {
        name: "mock".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
    };
    let (_client, tools) = connect_server(&spec, Duration::from_secs(5)).expect("connect");
    assert_eq!(tools.len(), 2);
    let def = tools[0].def();
    assert_eq!(def.name, "mock__echo");
    assert_eq!(tools[0].call(&json!({})).unwrap(), "pong");
    assert_eq!(tools[1].def().name, "mock__mcp");
}

/// A tool keeps its stable provider after the underlying process exits. The
/// next call resolves a fresh process generation without rebuilding the
/// registry or rediscovering tool definitions.
#[test]
#[ignore = "spawns /bin/sh mock servers; run with --ignored"]
fn crashed_server_is_replaced_without_registry_rebuild() {
    let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"x"}}\n' "$id" ;;
    *'"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"echo","description":"echoes","inputSchema":{"type":"object"}}]}}\n' "$id" ;;
    *'"tools/call"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"pong"}]}}\n' "$id"; exit 0 ;;
  esac
done
"#;
    let spec = ServerSpec {
        name: "recovering-mock".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
    };
    let (provider, tools) = connect_server(&spec, Duration::from_secs(5)).expect("connect");
    assert_eq!(provider.generation(), 1);
    assert_eq!(tools[0].call(&json!({})).unwrap(), "pong");

    let deadline = Instant::now() + Duration::from_secs(2);
    while provider.alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!provider.alive(), "mock process should have exited");

    assert_eq!(
        tools[0].call(&json!({})).unwrap(),
        "pong",
        "the original tool wrapper should resolve the replacement provider"
    );
    assert_eq!(provider.generation(), 2);
}

/// A transport failure during a potentially mutating tool call may have
/// happened after the server accepted the request. Restart for subsequent
/// work, but never replay that ambiguous call automatically.
#[test]
#[ignore = "spawns /bin/sh mock servers; run with --ignored"]
fn failed_tool_call_restarts_but_is_not_replayed() {
    let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"x"}}\n' "$id" ;;
    *'"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"mutate","description":"mutates","inputSchema":{"type":"object"}}]}}\n' "$id" ;;
    *'"tools/call"'*) exit 0 ;;
  esac
done
"#;
    let spec = ServerSpec {
        name: "ambiguous-mock".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
    };
    let (provider, tools) = connect_server(&spec, Duration::from_secs(5)).expect("connect");
    let error = tools[0].call(&json!({})).expect_err("call must fail");
    assert!(error.contains("not replayed"), "{error}");
    assert!(error.contains("restarted for the next call"), "{error}");
    assert_eq!(provider.generation(), 2);
    assert!(
        provider.alive(),
        "replacement should be ready for later work"
    );
}

/// Read-only MCP methods are safe to replay once. Persist one crash marker
/// outside the subprocess so the replacement generation can answer the
/// same request and prove the bounded retry path end to end.
#[test]
#[ignore = "spawns /bin/sh mock servers; run with --ignored"]
fn failed_read_only_request_restarts_and_replays_once() {
    let marker = std::env::temp_dir().join(format!("angel-mcp-read-replay-{}", std::process::id()));
    let _ = std::fs::remove_file(&marker);
    let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"x"}}\n' "$id" ;;
    *'"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[]}}\n' "$id" ;;
    *'"resources/list"'*)
      if [ -e "$MCP_REPLAY_MARKER" ]; then
        printf '{"jsonrpc":"2.0","id":%s,"result":{"resources":[{"uri":"file:///recovered","name":"recovered"}]}}\n' "$id"
      else
        : > "$MCP_REPLAY_MARKER"
        exit 0
      fi
      ;;
  esac
done
"#;
    let spec = ServerSpec {
        name: "read-replay-mock".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![(
            "MCP_REPLAY_MARKER".into(),
            marker.to_string_lossy().into_owned(),
        )],
    };
    let (provider, tools) = connect_server(&spec, Duration::from_secs(5)).expect("connect");
    assert_eq!(tools.len(), 1, "only the umbrella MCP surface is exposed");
    let resources = tools[0]
        .call(&json!({"action": "list_resources"}))
        .expect("read-only request should replay on the replacement");
    assert!(resources.contains("file:///recovered"), "{resources}");
    assert_eq!(provider.generation(), 2);
    let _ = std::fs::remove_file(marker);
}

/// A second discovery pass reuses the warm server (no respawn) — the `/cd`
/// registry-rebuild path. Same mock server; ignored with its sibling.
#[test]
#[ignore = "spawns a /bin/sh mock server; run with --ignored"]
fn rediscovery_reuses_the_warm_server() {
    let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"x"}}\n' "$id" ;;
    *'"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"echo","description":"echoes","inputSchema":{"type":"object"}}]}}\n' "$id" ;;
    *'"tools/call"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"pong"}]}}\n' "$id" ;;
  esac
done
"#;
    let specs = vec![ServerSpec {
        name: "warmmock".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
    }];
    let workspace = std::env::current_dir().unwrap();
    let (tools, notes) = discover_from_specs(&specs, Duration::from_secs(5), &workspace);
    assert_eq!(tools.len(), 2);
    assert!(notes[0].contains("2 tool(s)"), "{notes:?}");
    assert!(
        !notes[0].contains("(warm)"),
        "first pass is cold: {notes:?}"
    );
    let (tools, notes) = discover_from_specs(&specs, Duration::from_secs(5), &workspace);
    assert_eq!(tools.len(), 2, "warm pass re-wraps the same server");
    assert!(notes[0].contains("(warm)"), "second pass reuses: {notes:?}");
    assert_eq!(
        tools[0].call(&json!({})).unwrap(),
        "pong",
        "reused client still serves calls"
    );
    let foreign_workspace = std::env::temp_dir().join(format!(
        "angel-mcp-foreign-workspace-{}-{}",
        std::process::id(),
        crate::workspace_store::workspace_key(&workspace)
    ));
    std::fs::create_dir_all(&foreign_workspace).expect("foreign workspace");
    let (_, notes) = discover_from_specs(&specs, Duration::from_secs(5), &foreign_workspace);
    assert!(
        !notes[0].contains("(warm)"),
        "a different project must get a fresh process and cwd: {notes:?}"
    );
    let _ = std::fs::remove_dir(&foreign_workspace);
    // A changed spec must NOT reuse the warm client.
    let mut changed = specs.clone();
    changed[0].env = vec![("X".into(), "1".into())];
    let (_, notes) = discover_from_specs(&changed, Duration::from_secs(5), &workspace);
    assert!(
        !notes[0].contains("(warm)"),
        "spec change forces a fresh connect: {notes:?}"
    );
}
