//! MCP client — let the cockpit consume external **stdio** MCP servers (exa,
//! playwright, the `@modelcontextprotocol/server-*` family, …). Each configured
//! server is spawned as a subprocess; we do the JSON-RPC `initialize` handshake,
//! list its tools, and expose each action as a cockpit [`Tool`] named
//! `<server>__<tool>` so the agent calls it like any local tool. Each server also
//! gets one bounded `<server>__mcp` surface tool for MCP resources and prompts,
//! so external context/templates are available without exploding the schema list.
//!
//! Transport is newline-delimited JSON-RPC 2.0 over the child's stdin/stdout (the
//! MCP stdio framing). A reader thread forwards every line over a channel so
//! request/response round-trips can be bounded by a deadline without OS-level
//! pipe timeouts. Everything is **opt-in**: with no config file (`~/.angel0/mcp.json`)
//! nothing is spawned and behavior is unchanged.
//!
//! A server is a long-lived third-party daemon spawned outside the tool sandbox,
//! so it does not receive the cockpit's provider credentials by inheritance:
//! secret-named variables are withheld under the same `ANGEL_TOOL_STRIP_SECRETS`
//! knob as tool children (see [`server_command`]); a server's own key is
//! configured explicitly through its `env` entry in `mcp.json`.

use crate::club::ToolDef;
use crate::harness::Tool;
use crate::sandbox::process_owner::{Child, OwnedCommandExt};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

/// The MCP protocol revision we advertise in `initialize`.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// One configured server: how to spawn it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerSpec {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// A tool advertised by a server's `tools/list`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteTool {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

// ---------------------------------------------------------------------------
// Pure protocol helpers (unit-tested without a subprocess)
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 request line (newline-terminated for stdio framing).
fn build_request(id: u64, method: &str, params: Value) -> String {
    format!(
        "{}\n",
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    )
}

/// A JSON-RPC 2.0 notification line (no id, so no response is expected).
fn build_notification(method: &str, params: Value) -> String {
    format!(
        "{}\n",
        json!({ "jsonrpc": "2.0", "method": method, "params": params })
    )
}

/// Interpret one inbound line against the `id` we're waiting for:
/// - `Some(Ok(result))`   — the matching success response
/// - `Some(Err(message))` — the matching error response
/// - `None`               — a different id, a notification, or unparseable: skip
fn parse_response(line: &str, id: u64) -> Option<Result<Value, String>> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    if v.get("id").and_then(|i| i.as_u64()) != Some(id) {
        return None;
    }
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown MCP error");
        return Some(Err(msg.to_string()));
    }
    Some(Ok(v.get("result").cloned().unwrap_or(Value::Null)))
}

/// Pull `(name, description, inputSchema)` out of a `tools/list` result.
fn parse_tools_list(result: &Value) -> Vec<RemoteTool> {
    result
        .get("tools")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let name = t.get("name").and_then(|n| n.as_str())?.to_string();
                    let description = t
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string();
                    let schema = t
                        .get("inputSchema")
                        .cloned()
                        .unwrap_or_else(|| json!({ "type": "object" }));
                    Some(RemoteTool {
                        name,
                        description,
                        schema,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Flatten a `tools/call` result's `content` array into text. A truthy `isError`
/// turns the joined text into an `Err` so the agent sees it as a failure.
fn parse_tool_content(result: &Value) -> Result<String, String> {
    let mut text = String::new();
    if let Some(items) = result.get("content").and_then(|c| c.as_array()) {
        for item in items {
            match item.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(t);
                    }
                }
                Some(other) => {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&format!("[{other} content omitted]"));
                }
                None => {}
            }
        }
    }
    if text.is_empty() {
        // Fall back to the raw result so nothing is silently dropped.
        text = result.to_string();
    }
    if result.get("isError").and_then(|e| e.as_bool()) == Some(true) {
        Err(text)
    } else {
        Ok(text)
    }
}

/// Human-readable summary of an MCP `resources/list` result.
fn parse_resources_list(result: &Value) -> String {
    let Some(resources) = result.get("resources").and_then(|r| r.as_array()) else {
        return "no resources".to_string();
    };
    if resources.is_empty() {
        return "no resources".to_string();
    }
    let mut out = String::from("resources");
    for r in resources {
        let uri = r.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        if uri.is_empty() {
            continue;
        }
        let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let mime = r.get("mimeType").and_then(|v| v.as_str()).unwrap_or("");
        out.push('\n');
        out.push_str("- ");
        out.push_str(uri);
        if !name.is_empty() {
            out.push_str(" · ");
            out.push_str(name);
        }
        if !mime.is_empty() {
            out.push_str(" · ");
            out.push_str(mime);
        }
        if !desc.is_empty() {
            out.push_str(" — ");
            out.push_str(desc);
        }
    }
    out
}

/// Flatten MCP `resources/read` content. Text is returned directly; blobs and
/// unknown content kinds are represented so the model knows something existed.
fn parse_resource_read(result: &Value) -> String {
    let Some(contents) = result.get("contents").and_then(|c| c.as_array()) else {
        return result.to_string();
    };
    let mut out = String::new();
    for item in contents {
        let uri = item.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        if !uri.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(uri);
            out.push('\n');
        } else if !out.is_empty() {
            out.push_str("\n\n");
        }
        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
            out.push_str(text);
        } else if item.get("blob").is_some() {
            out.push_str("[binary resource content omitted]");
        } else {
            out.push_str(&item.to_string());
        }
    }
    if out.is_empty() {
        result.to_string()
    } else {
        out
    }
}

/// Human-readable summary of an MCP `prompts/list` result.
fn parse_prompts_list(result: &Value) -> String {
    let Some(prompts) = result.get("prompts").and_then(|p| p.as_array()) else {
        return "no prompts".to_string();
    };
    if prompts.is_empty() {
        return "no prompts".to_string();
    }
    let mut out = String::from("prompts");
    for p in prompts {
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let desc = p.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let args = p
            .get("arguments")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        let n = a.get("name").and_then(|v| v.as_str())?;
                        let req = a.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
                        Some(if req { format!("{n}*") } else { n.to_string() })
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        out.push('\n');
        out.push_str("- ");
        out.push_str(name);
        if !args.is_empty() {
            out.push('(');
            out.push_str(&args);
            out.push(')');
        }
        if !desc.is_empty() {
            out.push_str(" — ");
            out.push_str(desc);
        }
    }
    out
}

/// Flatten MCP `prompts/get` result into readable messages.
fn parse_prompt_get(result: &Value) -> String {
    let mut out = String::new();
    if let Some(desc) = result.get("description").and_then(|v| v.as_str())
        && !desc.is_empty()
    {
        out.push_str(desc);
        out.push('\n');
    }
    let Some(messages) = result.get("messages").and_then(|m| m.as_array()) else {
        return if out.is_empty() {
            result.to_string()
        } else {
            out
        };
    };
    for msg in messages {
        let role = msg
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("message");
        let content = msg.get("content").unwrap_or(&Value::Null);
        let text = match content.get("text").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => content.to_string(),
        };
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(role);
        out.push_str(": ");
        out.push_str(&text);
        out.push('\n');
    }
    out
}

/// Cockpit tool name for a remote tool: `<server>__<tool>`, sanitized to the
/// `[A-Za-z0-9_-]` set most providers accept for function names.
fn mangle_tool_name(server: &str, tool: &str) -> String {
    let clean = |s: &str| {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
    };
    format!("{}__{}", clean(server), clean(tool))
}

/// Parse a Claude-desktop-style config blob into server specs:
/// `{ "mcpServers": { "<name>": { "command": "...", "args": [...], "env": {...} } } }`
fn parse_mcp_config(text: &str) -> Vec<ServerSpec> {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let map = match v.get("mcpServers").and_then(|m| m.as_object()) {
        Some(m) => m,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for (name, cfg) in map {
        let command = match cfg.get("command").and_then(|c| c.as_str()) {
            Some(c) => c.to_string(),
            None => continue, // a server with no command is unusable
        };
        let args = cfg
            .get("args")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let env = cfg
            .get("env")
            .and_then(|e| e.as_object())
            .map(|e| {
                e.iter()
                    .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        out.push(ServerSpec {
            name: name.clone(),
            command,
            args,
            env,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Load configured servers from `ANGEL_MCP_CONFIG` (default `~/.angel0/mcp.json`).
/// Missing file → no servers.
fn load_mcp_servers() -> Vec<ServerSpec> {
    let path = std::env::var_os("ANGEL_MCP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".angel0/mcp.json")
        });
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_mcp_config(&text),
        Err(_) => Vec::new(),
    }
}

/// Whether inherited secret-named variables are withheld from server children.
/// The same knob and default as the tool-child scrub in `harness::exec`
/// (`ANGEL_TOOL_STRIP_SECRETS`, on). Unlike that scrub, YOLO does not bypass
/// it: a server is spawned at discovery and respawned on crash mid-session, so
/// a posture-dependent environment would let one generation of the same daemon
/// carry keys another lacks; a server's own credential belongs in its
/// `mcp.json` `env` entry, which is applied regardless of the knob.
fn strip_inherited_secrets() -> bool {
    crate::harness::env_flag("ANGEL_TOOL_STRIP_SECRETS", true)
}

/// Build the server process for `spec`. `inherited` names the parent
/// environment the child would otherwise receive unchanged; when
/// `strip_secrets`, every secret-named entry among them is withheld. `spec.env`
/// is applied afterwards, so a key configured in `mcp.json` always reaches the
/// server even when the cockpit holds a same-named variable. Pure over its
/// inputs so the environment contract is unit-testable without a process.
fn server_command(
    spec: &ServerSpec,
    inherited: impl IntoIterator<Item = OsString>,
    strip_secrets: bool,
) -> Command {
    let mut command = Command::new(&spec.command);
    command.args(&spec.args);
    if strip_secrets {
        for name in inherited {
            if crate::experience::is_secret_name(&name.to_string_lossy()) {
                command.env_remove(name);
            }
        }
    }
    command.envs(spec.env.iter().cloned());
    command
}

// ---------------------------------------------------------------------------
// The stdio client
// ---------------------------------------------------------------------------

struct Conn {
    stdin: ChildStdin,
    rx: mpsc::Receiver<String>,
}

/// A live connection to one MCP server subprocess.
pub struct McpClient {
    name: String,
    conn: Mutex<Conn>,
    next_id: AtomicU64,
    timeout: Duration,
    child: Mutex<Child>,
}

impl McpClient {
    /// Spawn `spec` and wire a reader thread. Does NOT handshake — call
    /// [`McpClient::initialize`] next.
    fn spawn(spec: &ServerSpec, timeout: Duration) -> Result<Self, String> {
        Self::spawn_in(spec, timeout, None)
    }

    fn spawn_in(
        spec: &ServerSpec,
        timeout: Duration,
        cwd: Option<&std::path::Path>,
    ) -> Result<Self, String> {
        let mut command = server_command(
            spec,
            std::env::vars_os().map(|(name, _)| name),
            strip_inherited_secrets(),
        );
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let mut child = command
            .spawn_owned()
            .map_err(|e| format!("spawn {}: {e}", spec.command))?;
        let stdin = child.stdin.take().ok_or("no child stdin")?;
        let stdout = child.stdout.take().ok_or("no child stdout")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break; // client dropped
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            name: spec.name.clone(),
            conn: Mutex::new(Conn { stdin, rx }),
            next_id: AtomicU64::new(1),
            timeout,
            child: Mutex::new(child),
        })
    }

    /// Send a request and read inbound lines until the matching response arrives
    /// or the deadline passes. Serialized by the `conn` lock.
    fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let line = build_request(id, method, params);
        let conn = self.conn.lock().map_err(|_| "mcp conn poisoned")?;
        {
            let mut stdin = &conn.stdin;
            stdin
                .write_all(line.as_bytes())
                .map_err(|e| format!("mcp {} write: {e}", self.name))?;
            stdin
                .flush()
                .map_err(|e| format!("mcp {} flush: {e}", self.name))?;
        }
        let deadline = Instant::now() + self.timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("mcp {} timed out on {method}", self.name))?;
            match conn.rx.recv_timeout(remaining) {
                Ok(l) => {
                    if let Some(res) = parse_response(&l, id) {
                        return res.map_err(|e| format!("mcp {}: {e}", self.name));
                    }
                    // a notification or other id — keep reading
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(format!("mcp {} timed out on {method}", self.name));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(format!("mcp {} closed the connection", self.name));
                }
            }
        }
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        let line = build_notification(method, params);
        let conn = self.conn.lock().map_err(|_| "mcp conn poisoned")?;
        let mut stdin = &conn.stdin;
        stdin
            .write_all(line.as_bytes())
            .map_err(|e| format!("mcp {} notify: {e}", self.name))?;
        stdin
            .flush()
            .map_err(|e| format!("mcp {} flush: {e}", self.name))?;
        Ok(())
    }

    /// The `initialize` request + `notifications/initialized` follow-up.
    fn initialize(&self) -> Result<(), String> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "angel0-cockpit", "version": env!("CARGO_PKG_VERSION") },
            }),
        )?;
        self.notify("notifications/initialized", json!({}))
    }

    fn list_tools(&self) -> Result<Vec<RemoteTool>, String> {
        let result = self.request("tools/list", json!({}))?;
        Ok(parse_tools_list(&result))
    }

    fn list_resources(&self) -> Result<String, String> {
        let result = self.request("resources/list", json!({}))?;
        Ok(parse_resources_list(&result))
    }

    fn read_resource(&self, uri: &str) -> Result<String, String> {
        let result = self.request("resources/read", json!({ "uri": uri }))?;
        Ok(parse_resource_read(&result))
    }

    fn list_prompts(&self) -> Result<String, String> {
        let result = self.request("prompts/list", json!({}))?;
        Ok(parse_prompts_list(&result))
    }

    fn get_prompt(&self, name: &str, arguments: Value) -> Result<String, String> {
        let result = self.request(
            "prompts/get",
            json!({ "name": name, "arguments": arguments }),
        )?;
        Ok(parse_prompt_get(&result))
    }

    /// Spawn `spec` and complete the MCP handshake, returning a shareable client
    /// ready for [`McpClient::call_tool`]. Unlike [`connect_server`] this does not
    /// list/wrap tools — it's for host code (e.g. the memory store) that talks to
    /// one known server directly rather than exposing its tools to the agent.
    pub fn connect(spec: &ServerSpec, timeout: Duration) -> Result<Arc<Self>, String> {
        let client = Arc::new(Self::spawn(spec, timeout)?);
        client.initialize()?;
        Ok(client)
    }

    /// Invoke a remote tool by its raw (un-mangled) name and return its text
    /// content. Public so host modules can drive a server outside the agent loop.
    pub fn call_tool(&self, name: &str, args: &Value) -> Result<String, String> {
        let result = self.request("tools/call", json!({ "name": name, "arguments": args }))?;
        parse_tool_content(&result)
    }

    /// Whether the server subprocess is still running — the warm-reuse guard.
    fn alive(&self) -> bool {
        self.child
            .lock()
            .map(|mut c| matches!(c.try_wait(), Ok(None)))
            .unwrap_or(false)
    }

    /// The child's process id, for teardown proofs: a retracted provider must
    /// leave no live child behind.
    fn pid(&self) -> Option<u32> {
        self.child.lock().ok().map(|child| child.id())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // The child has no kill-on-drop; reap it so we don't leak subprocesses.
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum McpReplay {
    Never,
    ReadOnly,
}

struct McpProviderState {
    generation: u64,
    client: Arc<McpClient>,
}

/// A stable capability identity backed by a replaceable MCP process. Tools keep
/// this provider across process failure, so the next request can resolve a new
/// implementation without rebuilding the whole cockpit registry. Replacement
/// is serialized per server and keyed by the exact observed generation.
struct McpProvider {
    spec: ServerSpec,
    timeout: Duration,
    workspace: Option<PathBuf>,
    state: Mutex<McpProviderState>,
    replacement: Mutex<()>,
}

impl McpProvider {
    fn connect(
        spec: &ServerSpec,
        timeout: Duration,
        workspace: Option<&std::path::Path>,
    ) -> Result<Arc<Self>, String> {
        let client = Arc::new(McpClient::spawn_in(spec, timeout, workspace)?);
        client.initialize()?;
        Ok(Arc::new(Self {
            spec: spec.clone(),
            timeout,
            workspace: workspace.map(std::path::Path::to_path_buf),
            state: Mutex::new(McpProviderState {
                generation: 1,
                client,
            }),
            replacement: Mutex::new(()),
        }))
    }

    fn name(&self) -> &str {
        &self.spec.name
    }

    fn snapshot(&self) -> Result<(u64, Arc<McpClient>), String> {
        self.state
            .lock()
            .map(|state| (state.generation, Arc::clone(&state.client)))
            .map_err(|_| format!("mcp {} provider state poisoned", self.spec.name))
    }

    fn alive(&self) -> bool {
        self.snapshot().is_ok_and(|(_, client)| client.alive())
    }

    /// The live child's pid, when this provider still has one.
    fn child_pid(&self) -> Option<u32> {
        self.snapshot().ok().and_then(|(_, client)| client.pid())
    }

    #[cfg(test)]
    fn generation(&self) -> u64 {
        self.snapshot()
            .map(|(generation, _)| generation)
            .unwrap_or(0)
    }

    /// Replace only the implementation the caller actually observed. A second
    /// caller arriving after another replacement adopts that newer generation
    /// instead of spawning a competing process.
    fn replace_if_current(
        &self,
        observed_generation: u64,
        observed: &Arc<McpClient>,
    ) -> Result<Arc<McpClient>, String> {
        let replacement = self
            .replacement
            .lock()
            .map_err(|_| format!("mcp {} replacement gate poisoned", self.spec.name))?;
        let state = self
            .state
            .lock()
            .map_err(|_| format!("mcp {} provider state poisoned", self.spec.name))?;
        if state.generation != observed_generation || !Arc::ptr_eq(&state.client, observed) {
            return Ok(Arc::clone(&state.client));
        }
        drop(state);

        // Keep the slow spawn/handshake outside `state`: callers can still
        // observe the old generation while one replacement is single-flighted.
        let next = Arc::new(McpClient::spawn_in(
            &self.spec,
            self.timeout,
            self.workspace.as_deref(),
        )?);
        if let Err(error) = next.initialize() {
            drop(replacement);
            drop(next);
            return Err(error);
        }

        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                drop(replacement);
                drop(next);
                return Err(format!("mcp {} provider state poisoned", self.spec.name));
            }
        };
        // Every replacement holds `replacement`, so this should only differ if
        // a future administrative path changes the provider. Preserve the exact
        // observed-generation rule instead of silently overwriting it.
        if state.generation != observed_generation || !Arc::ptr_eq(&state.client, observed) {
            let current = Arc::clone(&state.client);
            drop(state);
            drop(replacement);
            drop(next);
            return Ok(current);
        }
        let retired = std::mem::replace(&mut state.client, Arc::clone(&next));
        state.generation = state.generation.saturating_add(1);
        drop(state);
        drop(replacement);
        // Killing/reaping a retired process can block; never do it while the
        // provider state or replacement lock is held.
        drop(retired);
        Ok(next)
    }

    fn run<T>(
        &self,
        replay: McpReplay,
        operation: impl Fn(&McpClient) -> Result<T, String>,
    ) -> Result<T, String> {
        let (mut generation, mut client) = self.snapshot()?;
        if !client.alive() {
            self.replace_if_current(generation, &client)?;
            (generation, client) = self.snapshot()?;
        }

        match operation(&client) {
            Ok(value) => Ok(value),
            Err(error) if is_unhealthy_mcp_error(self.name(), &error) => {
                let replacement = self.replace_if_current(generation, &client);
                match (replay, replacement) {
                    (McpReplay::ReadOnly, Ok(client)) => operation(&client),
                    (McpReplay::Never, Ok(_)) => Err(format!(
                        "{error}; MCP provider restarted for the next call (this call was not replayed)"
                    )),
                    (_, Err(restart)) => {
                        Err(format!("{error}; MCP provider restart failed: {restart}"))
                    }
                }
            }
            Err(error) => Err(error),
        }
    }

    fn list_tools(&self) -> Result<Vec<RemoteTool>, String> {
        self.run(McpReplay::ReadOnly, McpClient::list_tools)
    }

    fn call_tool(&self, name: &str, args: &Value) -> Result<String, String> {
        self.run(McpReplay::Never, |client| client.call_tool(name, args))
    }

    fn list_resources(&self) -> Result<String, String> {
        self.run(McpReplay::ReadOnly, McpClient::list_resources)
    }

    fn read_resource(&self, uri: &str) -> Result<String, String> {
        self.run(McpReplay::ReadOnly, |client| client.read_resource(uri))
    }

    fn list_prompts(&self) -> Result<String, String> {
        self.run(McpReplay::ReadOnly, McpClient::list_prompts)
    }

    fn get_prompt(&self, name: &str, arguments: Value) -> Result<String, String> {
        self.run(McpReplay::ReadOnly, |client| {
            client.get_prompt(name, arguments.clone())
        })
    }
}

fn is_unhealthy_mcp_error(server: &str, error: &str) -> bool {
    error == "mcp conn poisoned"
        || error == format!("mcp {server} closed the connection")
        || error.starts_with(&format!("mcp {server} timed out on "))
        || error.starts_with(&format!("mcp {server} write: "))
        || error.starts_with(&format!("mcp {server} flush: "))
        || error.starts_with(&format!("mcp {server} notify: "))
}

/// A discovered remote tool, bound to its stable provider.
struct McpTool {
    provider: Arc<McpProvider>,
    remote_name: String,
    display_name: String,
    description: String,
    schema: Value,
}

impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.display_name
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.display_name.clone(),
            description: self.description.clone(),
            params: self.schema.clone(),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.provider.call_tool(&self.remote_name, args)
    }
}

/// A per-server MCP surface tool for capabilities that are not normal actions:
/// resources (readable context) and prompts (templates). Keeping these behind
/// one bounded schema prevents a server with many resources/prompts from
/// exploding the tool list.
struct McpSurfaceTool {
    provider: Arc<McpProvider>,
    display_name: String,
}

impl Tool for McpSurfaceTool {
    fn name(&self) -> &str {
        &self.display_name
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.display_name.clone(),
            description: format!(
                "List/read MCP resources and list/get MCP prompts from the '{}' server.",
                self.provider.name()
            ),
            params: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list_resources", "read_resource", "list_prompts", "get_prompt"]
                    },
                    "uri": {
                        "type": "string",
                        "description": "Required for read_resource."
                    },
                    "name": {
                        "type": "string",
                        "description": "Required for get_prompt."
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Optional prompt arguments for get_prompt."
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        match args.get("action").and_then(|v| v.as_str()) {
            Some("list_resources") => self.provider.list_resources(),
            Some("read_resource") => {
                let uri = args
                    .get("uri")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .ok_or("uri is required for read_resource")?;
                self.provider.read_resource(uri)
            }
            Some("list_prompts") => self.provider.list_prompts(),
            Some("get_prompt") => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .ok_or("name is required for get_prompt")?;
                let arguments = args.get("arguments").cloned().unwrap_or_else(|| json!({}));
                if !arguments.is_object() {
                    return Err("arguments must be an object".to_string());
                }
                self.provider.get_prompt(name, arguments)
            }
            Some(other) => Err(format!("unknown MCP surface action: {other}")),
            None => Err("action is required".to_string()),
        }
    }
}

/// Live clients kept warm only inside one canonical project. MCP configuration
/// is user-global, but a server process has cwd and internal session state; it
/// must not survive `/cd` into another repository.
type WarmServer = (ServerSpec, String, Arc<McpProvider>);
type ServerTools = (Arc<McpProvider>, Vec<Box<dyn Tool>>);

static WARM_SERVERS: OnceLock<Mutex<HashMap<String, WarmServer>>> = OnceLock::new();

/// Spawn + handshake every configured server and return their tools as cockpit
/// [`Tool`] objects, each bound to the teardown that unloads its server. Per-server
/// failures are reported in the returned `notes` and skipped — one bad server never
/// blocks the others or the cockpit. Returns an empty set when no `mcp.json` exists.
///
/// Servers connect **concurrently** (the work is dominated by waiting on
/// subprocess startup and the remote handshake), so one dead server costs its
/// own `ANGEL_MCP_TIMEOUT`, not the chain's sum — and a server already warm
/// from a previous registry build is reused without respawning at all.
///
/// The composition shape is what makes a mid-session unload possible: the registry
/// retracts each tool with its inverse, and the server's process goes when its last
/// tool does (arXiv 2608.25512 §5.1.1 parent composition).
pub(crate) fn discover_mcp_compositions(
    workspace: &std::path::Path,
) -> (Vec<McpComposition>, Vec<String>) {
    discover_compositions(&load_mcp_servers(), mcp_timeout(), workspace)
}

fn mcp_timeout() -> Duration {
    std::env::var("ANGEL_MCP_TIMEOUT")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(15))
}

/// One connected server: the tools it contributed, the process behind them, and
/// the handle that unloads the composition.
pub(crate) struct McpComposition {
    pub(crate) server: String,
    pub(crate) tools: Vec<Box<dyn Tool>>,
    /// Pid of the process behind this composition, for teardown proofs.
    #[allow(dead_code)]
    pub(crate) child_pid: Option<u32>,
    provider: Arc<McpProvider>,
}

impl McpComposition {
    /// Split into the tools the registry registers and the teardown that
    /// reverts the composition. The teardown owns this composition's provider
    /// reference: when the registry retracts the last tool, it forgets the warm
    /// entry and drops that reference, and `McpClient`'s drop reaps the child.
    pub(crate) fn into_parts(self) -> (Vec<Box<dyn Tool>>, impl FnOnce() + Send + Sync + 'static) {
        let server = self.server;
        let provider = self.provider;
        let tools = self.tools;
        (tools, move || {
            forget_warm_server(&server);
            drop(provider);
        })
    }
}

/// Drop the warm entry for `server`: a later discovery must reconnect instead of
/// reusing a provider the cockpit explicitly unloaded.
fn forget_warm_server(server: &str) {
    if let Some(warm) = WARM_SERVERS.get()
        && let Ok(mut map) = warm.lock()
    {
        map.remove(server);
    }
}

/// The spec-driven half of [`discover_mcp_compositions`], split out so the
/// warm-reuse path is testable without touching the global config.
pub(crate) fn discover_compositions(
    specs: &[ServerSpec],
    timeout: Duration,
    workspace: &std::path::Path,
) -> (Vec<McpComposition>, Vec<String>) {
    if specs.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let warm = WARM_SERVERS.get_or_init(|| Mutex::new(HashMap::new()));
    let project_key = crate::workspace_store::repo_identity(workspace).key;
    // Snapshot the reusable candidates outside the worker threads: same spec,
    // subprocess still running. The final liveness proof is answering
    // `tools/list` below — a wedged server fails that and reconnects fresh.
    let candidates: Vec<Option<Arc<McpProvider>>> = {
        let map = warm.lock().unwrap();
        specs
            .iter()
            .map(|spec| {
                map.get(&spec.name)
                    .filter(|(cached, key, provider)| {
                        cached == spec && key == &project_key && provider.alive()
                    })
                    .map(|(_, _, provider)| Arc::clone(provider))
            })
            .collect()
    };
    type Connected = (bool, Result<ServerTools, String>);
    let results: Vec<Connected> = std::thread::scope(|s| {
        let handles: Vec<_> = specs
            .iter()
            .zip(&candidates)
            .map(|(spec, cand)| {
                s.spawn(move || {
                    if let Some(provider) = cand
                        && let Ok(remote) = provider.list_tools()
                    {
                        return (
                            true,
                            Ok((Arc::clone(provider), wrap_tools(spec, provider, remote))),
                        );
                    }
                    (false, connect_server_in(spec, timeout, workspace))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| (false, Err("connect thread panicked".to_string())))
            })
            .collect()
    });
    let mut compositions: Vec<McpComposition> = Vec::new();
    let mut notes = Vec::new();
    let mut map = warm.lock().unwrap();
    for (spec, (reused, res)) in specs.iter().zip(results) {
        match res {
            Ok((provider, server_tools)) => {
                notes.push(format!(
                    "mcp: {} — {} tool(s){}",
                    spec.name,
                    server_tools.len(),
                    if reused { " (warm)" } else { "" }
                ));
                let child_pid = provider.child_pid();
                map.insert(
                    spec.name.clone(),
                    (spec.clone(), project_key.clone(), Arc::clone(&provider)),
                );
                compositions.push(McpComposition {
                    server: spec.name.clone(),
                    tools: server_tools,
                    child_pid,
                    provider,
                });
            }
            Err(e) => {
                map.remove(&spec.name);
                notes.push(format!("mcp: {} unavailable — {e}", spec.name));
            }
        }
    }
    (compositions, notes)
}

/// The flat form: every discovered tool, on its own. The registry's tracked path
/// uses [`discover_mcp_compositions`] instead, so that a server can be retracted
/// as one composition rather than tool by tool.
#[allow(dead_code)]
fn discover_from_specs(
    specs: &[ServerSpec],
    timeout: Duration,
    workspace: &std::path::Path,
) -> (Vec<Box<dyn Tool>>, Vec<String>) {
    let (compositions, notes) = discover_compositions(specs, timeout, workspace);
    let tools = compositions
        .into_iter()
        .flat_map(|composition| composition.tools)
        .collect();
    (tools, notes)
}

/// Connect to one server: spawn, initialize, list, wrap each tool. Returns the
/// provider alongside the tools so the caller can keep it warm for reuse.
#[cfg(test)]
fn connect_server(spec: &ServerSpec, timeout: Duration) -> Result<ServerTools, String> {
    let provider = McpProvider::connect(spec, timeout, None)?;
    let remote = provider.list_tools()?;
    let tools = wrap_tools(spec, &provider, remote);
    Ok((provider, tools))
}

fn connect_server_in(
    spec: &ServerSpec,
    timeout: Duration,
    workspace: &std::path::Path,
) -> Result<ServerTools, String> {
    let provider = McpProvider::connect(spec, timeout, Some(workspace))?;
    let remote = provider.list_tools()?;
    let tools = wrap_tools(spec, &provider, remote);
    Ok((provider, tools))
}

/// Wrap a live provider's advertised tools (plus the umbrella surface tool) as
/// cockpit [`Tool`] objects.
fn wrap_tools(
    spec: &ServerSpec,
    provider: &Arc<McpProvider>,
    remote: Vec<RemoteTool>,
) -> Vec<Box<dyn Tool>> {
    let mut tools: Vec<Box<dyn Tool>> = remote
        .into_iter()
        .map(|rt| {
            Box::new(McpTool {
                provider: Arc::clone(provider),
                display_name: mangle_tool_name(&spec.name, &rt.name),
                remote_name: rt.name,
                description: rt.description,
                schema: rt.schema,
            }) as Box<dyn Tool>
        })
        .collect();
    tools.push(Box::new(McpSurfaceTool {
        provider: Arc::clone(provider),
        display_name: mangle_tool_name(&spec.name, "mcp"),
    }));
    tools
}

#[cfg(test)]
mod tests {
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
        let ok = json!({ "content": [ {"type":"text","text":"hello"}, {"type":"text","text":"world"} ] });
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
        let marker =
            std::env::temp_dir().join(format!("angel-mcp-read-replay-{}", std::process::id()));
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
}
