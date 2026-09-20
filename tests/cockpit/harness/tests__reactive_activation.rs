//! Reactive activation — a tool is advertised only while its declared spec holds.
//!
//! Transfer of arXiv 2608.25512 §3.2 (Def 19/21/22): activation is *derived* from
//! a coeffect context `Σ`, so a provider that goes away takes its schemas with it
//! in the same turn — nothing to remember, no restart. §3.4 supplies the other
//! half (a change at an unrelated key is neutral by independence, so nothing
//! reloads), and §5.1.3 the case a value comparison cannot see: a *replaced*
//! provider that provides an equal value is still a change.
//!
//! §6.1 supplies the discipline: the runtime checks no witness, so every claim
//! below is exercised against the real store and the real registry — and, in the
//! ignored test, against a real MCP server process whose pid must stop existing.

use super::*;
use crate::agent::harness::coeffect::{Classification, Key, Requirement};
use crate::agent::harness::registration::ProviderScope;
use crate::agent::harness::registry::ProviderLease;
use crate::agent::mcp::{McpComposition, ServerSpec};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A tool that declares a precondition of its own — the `Tool::requires` path.
struct SpecTool {
    name: String,
    requires: Vec<Key>,
}

impl Tool for SpecTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name.clone(),
            description: format!("{} ({} declared key(s))", self.name, self.requires.len()),
            params: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    fn requires(&self) -> &[Key] {
        &self.requires
    }

    fn call(&self, _args: &Value) -> Result<String, String> {
        Ok(format!("{} ok", self.name))
    }
}

fn spec_tool(name: &str, requires: Vec<Key>) -> Box<dyn Tool> {
    Box::new(SpecTool {
        name: name.to_string(),
        requires,
    })
}

fn advertised(registry: &ToolRegistry, name: &str) -> bool {
    registry.defs().iter().any(|def| def.name == name)
}

fn turn_defs(registry: &ToolRegistry) -> Vec<ToolDef> {
    registry.defs_for_turn(None, false, false)
}

fn classified(log: &[(String, Classification)], name: &str) -> Classification {
    log.iter()
        .find(|(candidate, _)| candidate == name)
        .map(|(_, classification)| *classification)
        .unwrap_or_else(|| panic!("{name} declared no spec: {log:?}"))
}

// --- derivation -------------------------------------------------------------

#[test]
fn a_tool_is_advertised_only_while_its_declared_spec_holds() {
    let _lock = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register(spec_tool("web_fetch", vec![Key::capability("web")]));
    assert!(
        !advertised(&registry, "web_fetch"),
        "an unsatisfied spec keeps the tool out of the advertisement"
    );

    let lease = registry.bind_coeffect(Key::capability("web"));
    assert!(
        advertised(&registry, "web_fetch"),
        "binding the key activates with no other call"
    );

    let released = lease.release();
    assert_eq!(
        classified(&released, "web_fetch"),
        Classification::Deactivating
    );
    assert!(
        !advertised(&registry, "web_fetch"),
        "releasing the key takes the schema out with it"
    );
    assert_eq!(
        registry.activated_schema_failures(&registry.defs()),
        0,
        "the activation invariant holds on both hops"
    );
    assert!(lease.release().is_empty(), "a lease releases once");
}

#[test]
fn the_three_way_classification_drives_the_advertisement() {
    let _lock = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register(spec_tool("video_probe", vec![Key::provider("mcp:video")]));
    let activations = registry.tool_activations();

    let lease = registry.bind_provider("mcp:video");
    assert!(advertised(&registry, "video_probe"));

    // A retune under the live provider: same identity, different value. Neutral
    // by §3.4/Def 27, and the memo proof is that nothing is rebuilt.
    let generation = registry.tool_activation_generation();
    let before = registry.defs_arc();
    let retune = {
        let store = activations.store();
        let mut store = store.lock().unwrap_or_else(|e| e.into_inner());
        store.retune(Key::provider("mcp:video"), coeffect::Value::Count(7))
    };
    let log = activations.observe(&retune);
    assert_eq!(
        classified(&log, "video_probe"),
        Classification::Neutral,
        "a retune is invisible to the declaration"
    );
    assert_eq!(
        registry.tool_activation_generation(),
        generation,
        "a neutral change moves no generation"
    );
    assert!(
        Arc::ptr_eq(&before, &registry.defs_arc()),
        "and reuses the advertised set verbatim instead of rebuilding it"
    );

    // An unrelated key is neutral *for this tool* by independence, even though the
    // store did move: another tool could have declared that key, so the memo is
    // keyed on the store while this tool's advertisement stays put.
    let unrelated = {
        let store = activations.store();
        let mut store = store.lock().unwrap_or_else(|e| e.into_inner());
        store.bind(
            Key::resource("workspace"),
            coeffect::Value::Text("/tmp".into()),
        )
    };
    let log = activations.observe(&unrelated);
    assert_eq!(
        classified(&log, "video_probe"),
        Classification::Neutral,
        "an unrelated key is neutral by independence"
    );
    assert!(advertised(&registry, "video_probe"));

    let released = lease.release();
    assert_eq!(
        classified(&released, "video_probe"),
        Classification::Deactivating
    );
    assert_ne!(registry.tool_activation_generation(), generation);
    assert!(!advertised(&registry, "video_probe"));
}

#[test]
fn a_replaced_provider_is_a_change_even_when_the_value_is_equal() {
    let _lock = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register(spec_tool("clock", vec![Key::provider("svc:clock")]));
    let activations = registry.tool_activations();
    let first = registry.bind_provider("svc:clock");
    let generation = registry.tool_activation_generation();

    // The same provider, replaced by one that provides an equal value: §5.1.3.
    let change = {
        let store = activations.store();
        let mut store = store.lock().unwrap_or_else(|e| e.into_inner());
        store.bind(Key::provider("svc:clock"), coeffect::Value::Flag(true))
    };
    let reaction = change.react(&Requirement::any([Key::provider("svc:clock")]));
    assert!(
        reaction.identity_changed,
        "a replacement is an identity change"
    );
    assert!(reaction.reloads(), "so the dependent must re-derive");
    let log = activations.observe(&change);
    assert_eq!(classified(&log, "clock"), Classification::Neutral);
    assert_ne!(
        registry.tool_activation_generation(),
        generation,
        "the advertisement is re-derived even though the value is equal"
    );
    assert!(advertised(&registry, "clock"));
    first.release();
}

#[test]
fn a_tool_search_cannot_surface_a_departed_provider() {
    let _lock = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register_deferred(spec_tool("searchable", vec![Key::provider("mcp:search")]));
    registry.enable_tool_search();
    let lease = registry.bind_provider("mcp:search");

    let hit = registry
        .dispatch("tool_search", &serde_json::json!({"query": "searchable"}))
        .expect("tool_search runs");
    assert!(
        hit.contains("searchable"),
        "a live provider is searchable: {hit}"
    );
    assert!(
        registry
            .tool_activations()
            .snapshot()
            .contains("searchable"),
        "the hit activates the schema"
    );

    lease.release();
    let miss = registry
        .dispatch("tool_search", &serde_json::json!({"query": "searchable"}))
        .expect("tool_search runs");
    assert!(
        !miss.contains("- searchable"),
        "a departed provider cannot be surfaced by a search: {miss}"
    );
    assert!(
        !registry.tool_activations().admits("searchable"),
        "and it is no longer admitted"
    );
    assert!(
        registry.tool_activations().snapshot().is_empty(),
        "and its activation leaves with it"
    );
    assert_eq!(
        registry.activated_schema_failures(&turn_defs(&registry)),
        0,
        "no stale schema reaches the provider request"
    );
}

#[test]
fn a_withdrawn_provider_leaves_no_schema_behind_in_the_turn() {
    let _lock = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register_deferred(spec_tool("fetch_page", vec![Key::provider("mcp:web")]));
    registry.enable_tool_search();
    let lease = registry.bind_provider("mcp:web");
    registry
        .dispatch("tool_search", &serde_json::json!({"query": "fetch_page"}))
        .expect("tool_search runs");
    assert!(
        turn_defs(&registry)
            .iter()
            .any(|def| def.name == "fetch_page"),
        "the activated schema is advertised for the next request"
    );

    // Withdraw the provider mid-turn: the derived set must follow, and the
    // activation invariant must hold at every hop.
    let log = lease.release();
    assert_eq!(classified(&log, "fetch_page"), Classification::Deactivating);
    let defs = turn_defs(&registry);
    assert!(
        !defs.iter().any(|def| def.name == "fetch_page"),
        "the schema is retracted within the same turn"
    );
    assert_eq!(registry.activated_schema_failures(&defs), 0);
    assert!(registry.tool_activations().snapshot().is_empty());

    // The intent is remembered: when the provider comes back, so does the schema.
    let lease = registry.bind_provider("mcp:web");
    assert!(
        registry.tool_activations().admits("fetch_page"),
        "the returning provider satisfies the declaration again"
    );
    registry
        .dispatch("tool_search", &serde_json::json!({"query": "fetch_page"}))
        .expect("tool_search runs");
    assert!(
        turn_defs(&registry)
            .iter()
            .any(|def| def.name == "fetch_page"),
        "a provider that returns re-advertises its tools"
    );
    lease.release();
}

#[test]
fn a_lease_releases_once_however_it_is_fired() {
    let _lock = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register(spec_tool("export", vec![Key::capability("export")]));
    let lease = registry.bind_coeffect(Key::capability("export"));
    assert_eq!(lease.key().as_str(), "capability:export");
    assert!(advertised(&registry, "export"));

    // The lease composes with the registration path: its inverse fires the same
    // release, and the once-only guard makes the second path a no-op rather than
    // a double release.
    let inverse = lease.as_disposable();
    assert!(inverse.fire());
    assert!(!advertised(&registry, "export"));
    assert!(
        lease.release().is_empty(),
        "the inverse already released it"
    );
}

// --- the live provider ------------------------------------------------------

/// A stdio MCP server with no SDK dependency: it records its own pid, then
/// answers `initialize` / `tools/list` / `tools/call`. Same fixture the provider
/// lifecycle tests use, so the pid is observable from outside the process.
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

fn wait_for_reaped(pid: u32, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    !Path::new(&format!("/proc/{pid}")).exists()
}

/// Register a live server's tools exactly the way `bootstrap::install_mcp_tools`
/// does: bind the provider, then track each tool against the scope whose
/// teardown releases that binding before it lets the process go.
fn install_live_provider(
    registry: &mut ToolRegistry,
    composition: McpComposition,
) -> Arc<ProviderLease> {
    let provider = composition.server.clone();
    let lease = registry.bind_provider(&provider);
    let (tools, teardown) = composition.into_parts();
    let scope = ProviderScope::new(tools.len(), {
        let lease = Arc::clone(&lease);
        move || {
            lease.release();
            teardown();
        }
    });
    for tool in tools {
        registry.track_registration_in_provider(tool, scope.registration(), &provider, true);
    }
    registry.enable_tool_search();
    lease
}

#[test]
#[ignore = "spawns a real MCP server subprocess; run with --ignored"]
fn a_live_provider_withdrawal_retracts_schemas_without_a_restart() {
    let _lock = crate::tests::env_lock();
    let root = scratch_root("reactive-activation-live");
    let pid_file = root.join("pid");
    let (compositions, notes) = crate::agent::mcp::discover_compositions(
        &[mock_spec("mockmcp", &pid_file)],
        Duration::from_secs(10),
        &root,
    );
    assert_eq!(notes.len(), 1, "server notes: {notes:?}");
    assert!(
        notes[0].contains("2 tool(s)"),
        "one remote tool plus the bounded surface tool: {notes:?}"
    );
    let composition = compositions.into_iter().next().expect("one composition");
    let pid = composition.child_pid.expect("live child pid");

    let connect_started = Instant::now();
    let mut registry = ToolRegistry::new();
    let _lease = install_live_provider(&mut registry, composition);
    let connect_ms = connect_started.elapsed().as_millis();

    let name = "mockmcp__echo";
    let hit = registry
        .dispatch("tool_search", &serde_json::json!({"query": "echo"}))
        .expect("tool_search runs");
    assert!(
        hit.contains(name),
        "the live server's tool is searchable: {hit}"
    );
    assert!(
        turn_defs(&registry).iter().any(|def| def.name == name),
        "and its schema is advertised while the provider is live"
    );
    assert_eq!(
        registry
            .dispatch(name, &serde_json::json!({}))
            .expect("live"),
        "pong"
    );

    // Drop the provider mid-session: the derivation must follow in the same turn,
    // and the process must go with it.
    let drop_started = Instant::now();
    assert_eq!(
        registry.unregister_prefix("mockmcp__"),
        2,
        "a server contributes its own tool plus one bounded surface tool"
    );
    let drop_ms = drop_started.elapsed().as_millis();
    let defs = turn_defs(&registry);
    assert!(
        !defs.iter().any(|def| def.name == name),
        "a dropped provider advertises nothing"
    );
    assert_eq!(
        registry.activated_schema_failures(&defs),
        0,
        "and no stale schema reaches the next request"
    );
    assert!(
        wait_for_reaped(pid, Duration::from_secs(5)),
        "dropping a provider must not orphan its child"
    );

    // Re-add: a fresh process, a searchable tool, no cockpit restart.
    let readd_started = Instant::now();
    let (compositions, notes) = crate::agent::mcp::discover_compositions(
        &[mock_spec("mockmcp", &pid_file)],
        Duration::from_secs(10),
        &root,
    );
    assert_eq!(notes.len(), 1, "server notes: {notes:?}");
    let composition = compositions.into_iter().next().expect("one composition");
    let second_pid = composition.child_pid.expect("live child pid");
    assert_ne!(pid, second_pid, "re-adding connects a new process");
    let _lease = install_live_provider(&mut registry, composition);
    registry
        .dispatch("tool_search", &serde_json::json!({"query": "echo"}))
        .expect("tool_search runs");
    assert!(
        turn_defs(&registry).iter().any(|def| def.name == name),
        "the re-added provider is advertised again"
    );
    let readd_ms = readd_started.elapsed().as_millis();
    println!(
        "reactive activation: connect={connect_ms}ms drop={drop_ms}ms re-add={readd_ms}ms (no cockpit restart)"
    );
    let _ = registry.unregister_prefix("mockmcp__");
}

/// A provider-scoped registration must not discard what the tool declares about
/// itself: the provider's liveness and the tool's own key are both required.
#[test]
fn a_provider_scoped_registration_keeps_the_tools_own_requirement() {
    let _lock = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    let scope = ProviderScope::new(1, || {});
    registry.track_registration_in_provider(
        spec_tool("export", vec![Key::capability("export")]),
        scope.registration(),
        "mcp:files",
        false,
    );

    // Provider live, the tool's own capability missing: still not advertised.
    let provider = registry.bind_provider("mcp:files");
    assert!(
        !advertised(&registry, "export"),
        "the tool's own declared key still gates it"
    );
    let capability = registry.bind_coeffect(Key::capability("export"));
    assert!(advertised(&registry, "export"));

    // Capability live, provider gone: withdrawn anyway, in the same turn.
    provider.release();
    assert!(!advertised(&registry, "export"));
    capability.release();
}

/// T1's discipline applied to a provider: the lease *is* the release, so the last
/// hand to hold it takes the provider out of `Σ` even without an explicit call.
#[test]
fn a_lease_dropped_without_release_still_unloads_its_provider() {
    let _lock = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register(spec_tool("search", vec![Key::provider("mcp:search")]));
    {
        let lease = registry.bind_provider("mcp:search");
        assert!(advertised(&registry, "search"));
        drop(lease);
    }
    assert!(
        !advertised(&registry, "search"),
        "dropping the last lease takes the provider out of Σ"
    );
}
