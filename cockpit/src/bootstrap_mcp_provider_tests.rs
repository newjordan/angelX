//! The production MCP path: `install_mcp_tools` against a real configured server,
//! then a mid-session drop with no restart.
//!
//! The registry-level tests prove the semantics (`reactive_activation.rs`); this
//! proves the wiring an operator actually uses — `mcp.json` → `bind_provider` →
//! provider-scoped tracked registrations → a `ProviderLease` released ahead of the
//! child's teardown (§5.1.3 inertial teardown in the order the paper asks for).
//! §6.1 again: the witness is a pid that stops existing.

use super::*;
use crate::tests::TestEnvGuard;

/// A stdio MCP server with no SDK dependency: it records its own pid, then
/// answers `initialize` / `tools/list` / `tools/call`.
const MOCK_SERVER: &str = r#"
echo $$ > "$MCP_PID_FILE"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"x"}}\n' "$id" ;;
    *'"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"echo","description":"echoes","inputSchema":{"type":"object"}}]}}\n' "$id" ;;
    *'"tools/call"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"pong"}]}}\n' "$id" ;;
  esac
done
"#;

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("angel_bootstrap_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

fn wait_for_reaped(pid: u32, budget: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + budget;
    while std::time::Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    !pid_alive(pid)
}

/// Write a `mcp.json` in the shape Claude Desktop uses and point the loader at it.
fn configure_server(root: &Path, pid_file: &Path) -> PathBuf {
    let config = root.join("mcp.json");
    let text = serde_json::json!({
        "mcpServers": {
            "mockmcp": {
                "command": "/bin/sh",
                "args": ["-c", MOCK_SERVER],
                "env": { "MCP_PID_FILE": pid_file.display().to_string() },
            }
        }
    })
    .to_string();
    std::fs::write(&config, text).expect("mcp.json");
    config
}

#[test]
#[ignore = "spawns a real MCP server subprocess; run with --ignored"]
fn a_configured_server_is_advertised_and_dropped_without_a_restart() {
    let _lock = crate::tests::env_lock();
    let root = scratch_dir("mcp_provider");
    let pid_file = root.join("pid");
    let config = configure_server(&root, &pid_file);
    let _env = TestEnvGuard::set("ANGEL_MCP_CONFIG", &config.display().to_string());

    let mut registry = ToolRegistry::new();
    install_mcp_tools(&mut registry, &root);

    let names: Vec<String> = registry.defs().iter().map(|def| def.name.clone()).collect();
    assert!(
        names.iter().any(|name| name == "mockmcp__echo"),
        "a configured server's tool is advertised: {names:?}"
    );
    let pid: u32 = std::fs::read_to_string(&pid_file)
        .expect("the server records its pid")
        .trim()
        .parse()
        .expect("pid");
    assert!(pid_alive(pid), "the server process is live");
    assert_eq!(
        registry
            .dispatch("mockmcp__echo", &serde_json::json!({}))
            .expect("live"),
        "pong"
    );

    // Drop it mid-session, exactly as the operator's toggle does.
    let dropped = registry.unregister_prefix("mockmcp__");
    assert_eq!(dropped, 2, "one remote tool plus the bounded surface tool");
    assert!(
        !registry
            .defs()
            .iter()
            .any(|def| def.name.starts_with("mockmcp__")),
        "a dropped provider advertises nothing"
    );
    assert!(
        registry
            .dispatch("mockmcp__echo", &serde_json::json!({}))
            .is_err()
    );
    assert_eq!(registry.tracked_registrations(), 0);
    assert_eq!(
        registry.activated_schema_failures(&registry.defs()),
        0,
        "the activation invariant holds across the unload"
    );
    assert!(
        wait_for_reaped(pid, std::time::Duration::from_secs(5)),
        "the dropped server's child is reaped"
    );

    // And the same session can bring it back: the warm entry was forgotten, so
    // this is a new process, not the one we dropped.
    install_mcp_tools(&mut registry, &root);
    assert!(
        registry
            .defs()
            .iter()
            .any(|def| def.name == "mockmcp__echo"),
        "re-adding the server re-advertises its tool with no cockpit restart"
    );
    let second_pid: u32 = std::fs::read_to_string(&pid_file)
        .expect("the second server records its pid")
        .trim()
        .parse()
        .expect("pid");
    assert_ne!(pid, second_pid, "re-adding connects a new process");
    let _ = registry.unregister_prefix("mockmcp__");
}
