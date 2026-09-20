//! Provider lifecycle — the registry's inverse reaches the process.
//!
//! Registration coherence is one thing (`registration_inverse.rs`); what the
//! cockpit actually pays for is the other half: dropping an MCP server mid-session
//! must retract its schemas **and** reap its child, without restarting. These
//! tests walk the whole path with a real server process:
//!
//! * one retraction of a multi-tool provider keeps the server alive,
//! * the last retraction unloads it, forgets the warm entry, and reaps the child,
//! * a later discovery reconnects as a *new* process — not the one we dropped.
//!
//! The paper's boundary discipline applies (arXiv 2608.25512 §6.1): the child
//! process is inside the boundary and revertible; anything it emitted outside is
//! not. So the witness here is a pid that stops existing.

use super::*;
use crate::agent::harness::registration::ProviderScope;
use crate::agent::mcp::{McpComposition, ServerSpec};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A stdio MCP server with no SDK dependency: it records its own pid, then
/// answers `initialize` / `tools/list` / `tools/call`. This is the same fixture
/// shape `mcp.rs` uses for its warm-reuse tests, with the pid added so teardown
/// is observable from outside the process.
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

fn scratch_root(tag: &str) -> PathBuf {
    let root = scratch(tag);
    std::fs::create_dir_all(&root).expect("scratch dir");
    root
}

fn mock_spec(name: &str, pid_file: &Path) -> ServerSpec {
    ServerSpec {
        name: name.to_string(),
        command: "/bin/sh".to_string(),
        args: vec!["-c".to_string(), MOCK_SERVER.to_string()],
        env: vec![("MCP_PID_FILE".to_string(), pid_file.display().to_string())],
    }
}

fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Wait for the pid to leave the process table. Kill+wait happens when the last
/// provider reference drops, so this is the observable end state of an unload.
fn wait_for_reaped(pid: u32, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    !pid_alive(pid)
}

/// Register one server's tools the way `bootstrap::install_mcp_tools` does.
fn register_composition(registry: &mut ToolRegistry, composition: McpComposition) -> usize {
    let (tools, teardown) = composition.into_parts();
    let scope = ProviderScope::new(tools.len(), teardown);
    let registered = tools.len();
    for tool in tools {
        registry.track_registration(tool, scope.registration());
    }
    registered
}

/// A plain child process standing in for a provider's owned resource.
struct LiveChild {
    child: Arc<Mutex<Child>>,
    reaps: Arc<AtomicUsize>,
}

impl LiveChild {
    fn spawn() -> Arc<Self> {
        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("child");
        Arc::new(Self {
            child: Arc::new(Mutex::new(child)),
            reaps: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn pid(&self) -> u32 {
        self.child.lock().unwrap_or_else(|e| e.into_inner()).id()
    }

    fn reaps(&self) -> usize {
        self.reaps.load(Ordering::SeqCst)
    }

    fn teardown(self: &Arc<Self>) -> impl FnOnce() + Send + Sync + 'static {
        let this = Arc::clone(self);
        move || {
            {
                let mut child = this.child.lock().unwrap_or_else(|e| e.into_inner());
                let _ = child.kill();
                let _ = child.wait();
            }
            this.reaps.fetch_add(1, Ordering::SeqCst);
        }
    }
}

// --- scope semantics --------------------------------------------------------

#[test]
fn a_provider_unloads_only_when_its_last_registration_goes() {
    let teardowns = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&teardowns);
    let scope = ProviderScope::new(3, move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let first = scope.registration();
    let second = scope.registration();
    let third = scope.registration();
    assert_eq!(scope.remaining(), 3);

    assert!(first.fire());
    assert_eq!(
        teardowns.load(Ordering::SeqCst),
        0,
        "retracting one tool must not unload a live server"
    );
    second.fire();
    assert_eq!(teardowns.load(Ordering::SeqCst), 0);
    third.fire();
    assert_eq!(
        teardowns.load(Ordering::SeqCst),
        1,
        "the last retraction unloads the provider"
    );
    third.fire();
    assert_eq!(
        teardowns.load(Ordering::SeqCst),
        1,
        "an unloaded provider is not unloaded twice"
    );
    assert_eq!(scope.remaining(), 0);
}

// --- registry path, real child ---------------------------------------------

#[test]
fn unregister_prefix_unloads_a_provider_and_reaps_its_child() {
    let _lock = crate::tests::env_lock();
    let child = LiveChild::spawn();
    let pid = child.pid();
    let mut registry = ToolRegistry::new();
    let scope = ProviderScope::new(3, child.teardown());
    for name in ["childmock__a", "childmock__b", "childmock__c"] {
        registry.track_registration(probe_tool(name), scope.registration());
    }
    assert!(pid_alive(pid), "the provider's child starts alive");

    // Partial retraction leaves the provider (and its child) up.
    assert!(registry.unregister("childmock__a"));
    assert_eq!(scope.remaining(), 2);
    assert!(pid_alive(pid), "two registrations still hold the provider");
    assert_eq!(child.reaps(), 0);

    let unloaded_at = Instant::now();
    assert_eq!(registry.unregister_prefix("childmock__"), 2);
    let unload_ms = unloaded_at.elapsed().as_millis();

    assert_eq!(registry.tracked_registrations(), 0);
    assert_eq!(child.reaps(), 1, "the last retraction reaps the child");
    assert!(
        wait_for_reaped(pid, Duration::from_secs(3)),
        "no orphan process after the unload"
    );
    assert_eq!(registry.unregister_prefix("childmock__"), 0, "idempotent");
    eprintln!("provider unload: {unload_ms}ms for 2 registrations (no restart)");
}

fn probe_tool(name: &str) -> Box<dyn Tool> {
    Box::new(Probe {
        name: name.to_string(),
    })
}

struct Probe {
    name: String,
}

impl Tool for Probe {
    fn name(&self) -> &str {
        &self.name
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name.clone(),
            description: "lifecycle probe".to_string(),
            params: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    fn call(&self, _args: &Value) -> Result<String, String> {
        Ok("probe".to_string())
    }
}

// --- the real thing: a live MCP server, dropped mid-session ----------------

#[test]
#[ignore = "spawns a real MCP server subprocess; run with --ignored"]
fn dropping_a_live_mcp_server_retracts_its_schemas_and_reaps_its_child() {
    let _lock = crate::tests::env_lock();
    let root = scratch_root("mcp-provider-lifecycle");
    let pid_file = root.join("server.pid");
    let spec = mock_spec("mockmcp", &pid_file);

    let connect_at = Instant::now();
    let (mut compositions, notes) = crate::agent::mcp::discover_compositions(
        std::slice::from_ref(&spec),
        Duration::from_secs(10),
        &root,
    );
    let connect_ms = connect_at.elapsed().as_millis();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("2 tool(s)"), "{notes:?}");
    let composition = compositions.pop().expect("one connected server");
    let first_pid = composition.child_pid.expect("live child pid");
    assert!(pid_alive(first_pid), "the server process is live");

    let mut registry = ToolRegistry::new();
    let register_at = Instant::now();
    assert_eq!(register_composition(&mut registry, composition), 2);
    let register_ms = register_at.elapsed().as_millis();
    assert!(
        registry
            .defs()
            .iter()
            .any(|def| def.name == "mockmcp__echo"),
        "the server's tools are advertised while it is live"
    );
    assert_eq!(
        registry
            .dispatch("mockmcp__echo", &serde_json::json!({}))
            .expect("live server answers"),
        "pong"
    );

    // Drop the server mid-session: retract its tools, reap its process.
    let drop_at = Instant::now();
    assert_eq!(registry.unregister_prefix("mockmcp__"), 2);
    let drop_ms = drop_at.elapsed().as_millis();
    assert!(
        !registry
            .defs()
            .iter()
            .any(|def| def.name.starts_with("mockmcp__")),
        "a dropped server advertises nothing"
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
        "retraction keeps the activation invariant"
    );
    assert!(
        wait_for_reaped(first_pid, Duration::from_secs(5)),
        "dropping a server must not orphan its child"
    );

    // Adding it back is a fresh process, not the warm one we dropped.
    let readd_at = Instant::now();
    let (mut again, notes) =
        crate::agent::mcp::discover_compositions(&[spec], Duration::from_secs(10), &root);
    let readd_ms = readd_at.elapsed().as_millis();
    assert!(notes[0].contains("2 tool(s)"), "{notes:?}");
    let composition = again.pop().expect("reconnected server");
    let second_pid = composition.child_pid.expect("live child pid");
    assert_ne!(
        second_pid, first_pid,
        "a dropped server reconnects as a new process"
    );
    assert!(pid_alive(second_pid), "the replacement is live");
    let mut registry = ToolRegistry::new();
    assert_eq!(register_composition(&mut registry, composition), 2);
    assert_eq!(
        registry
            .dispatch("mockmcp__echo", &serde_json::json!({}))
            .expect("replacement answers"),
        "pong"
    );

    eprintln!(
        "mcp lifecycle: connect={connect_ms}ms register={register_ms}ms drop={drop_ms}ms re-add={readd_ms}ms (no cockpit restart)"
    );
}
