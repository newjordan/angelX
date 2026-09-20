//! LSP client — real compiler/analyzer intelligence for the cockpit. Instead of
//! grepping for symbols, the agent can ask a Language Server for ground-truth
//! answers (the marquee dev-env upgrade: straight from rust-analyzer / pyright /
//! tsserver, not a regex guess):
//!   - `lsp_diagnostics(path)` — errors/warnings
//!   - `lsp_definition` / `lsp_references` / `lsp_hover` — jump-to-def, find-uses,
//!     type/doc at a `symbol` (first occurrence) or explicit `line`/`character`.
//!   - `lsp_symbols(path)` — the analyzer's hierarchical file outline.
//!   - `lsp_workspace_symbol(query)` — find a symbol anywhere in the project.
//!
//! Transport is JSON-RPC 2.0 over the child's stdin/stdout with **Content-Length**
//! header framing (the LSP base protocol — distinct from MCP's newline framing in
//! `mcp.rs`, which this otherwise mirrors). A reader thread parses frames and
//! forwards each message over a channel; requests correlate by id, and
//! `textDocument/publishDiagnostics` notifications are drained after `didOpen`.
//! Server→client **requests** (progress tokens, capability registration) are
//! answered by the reader thread itself and not forwarded — and they're never
//! mistaken for our responses even on an id collision (the id spaces are
//! independent), since a message carrying a `method` is never a response.
//!
//! An [`LspPool`] keeps one warm, initialized server per language alive across
//! calls (behind a shared [`LspCtx`]), so a single rust-analyzer answers
//! diagnostics AND navigation, paying its index cost once. Cold queries wait for
//! the first diagnostics push (the analysis-ready signal); warm queries are
//! request/response (pull diagnostics / definition / references / hover).
//!
//! On by default when a server binary for a workspace language is on PATH;
//! `ANGEL_LSP=1` forces on, `ANGEL_LSP=0` forces off. Language servers are
//! heavy subprocesses, so nothing is spawned until a call is actually made,
//! and only then for the file's language. `ANGEL_LSP_PREWARM=1` explicitly
//! opts into eager startup for a known, tightly scoped project workspace.

use crate::club::ToolDef;
use crate::harness::Tool;
use crate::sandbox::process_owner::{Child, OwnedCommandExt};
use serde_json::{Value, json};

mod format;
mod parity;
mod protocol;
pub(crate) use format::{
    format_diagnostics, format_document_symbols, format_hover, format_locations,
    format_workspace_symbols, path_to_uri, resolve_position,
};
pub(crate) use protocol::{
    build_frame, is_server_request, match_response, publish_diagnostics_for, pull_report_to_params,
    read_frame, server_request_reply,
};
use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

/// One configured language server: how to spawn it + which files it owns.
#[derive(Debug, Clone, PartialEq)]
pub struct LspServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    /// File extensions (no dot, lowercase) this server handles.
    pub extensions: Vec<String>,
    /// LSP `languageId` to advertise in `didOpen` (e.g. "rust", "python").
    pub language_id: String,
}

/// Built-in servers, used when no config overrides them. Bare command names so
/// they resolve on `PATH`; a missing binary just means that language is skipped
/// at call time (reported as a tool error, never a crash).
fn default_servers() -> Vec<LspServer> {
    vec![
        LspServer {
            name: "rust".into(),
            command: "rust-analyzer".into(),
            args: vec![],
            extensions: vec!["rs".into()],
            language_id: "rust".into(),
        },
        LspServer {
            name: "python".into(),
            command: "pyright-langserver".into(),
            args: vec!["--stdio".into()],
            extensions: vec!["py".into(), "pyi".into()],
            language_id: "python".into(),
        },
        LspServer {
            name: "typescript".into(),
            command: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
            extensions: vec!["ts".into(), "tsx".into(), "js".into(), "jsx".into()],
            language_id: "typescript".into(),
        },
        LspServer {
            name: "bash".into(),
            command: "bash-language-server".into(),
            args: vec!["start".into()],
            extensions: vec!["sh".into(), "bash".into()],
            language_id: "shellscript".into(),
        },
        LspServer {
            name: "yaml".into(),
            command: "yaml-language-server".into(),
            args: vec!["--stdio".into()],
            extensions: vec!["yaml".into(), "yml".into()],
            language_id: "yaml".into(),
        },
    ]
}

/// Parse `~/.angel0/lsp.json`. Shape (each field optional but `command` required):
/// ```json
/// { "servers": { "rust": { "command": "rust-analyzer", "args": [],
///                          "extensions": ["rs"], "languageId": "rust" } } }
/// ```
/// Tolerates garbage (returns whatever parsed). A server without a command is
/// dropped. `languageId` defaults to the map key; `extensions` defaults to none.
fn parse_lsp_config(text: &str) -> Vec<LspServer> {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let servers = match v.get("servers").and_then(|s| s.as_object()) {
        Some(m) => m,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for (name, spec) in servers {
        let command = match spec.get("command").and_then(|c| c.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            _ => continue,
        };
        let args = spec
            .get("args")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let extensions = spec
            .get("extensions")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.trim_start_matches('.').to_lowercase()))
                    .collect()
            })
            .unwrap_or_default();
        let language_id = spec
            .get("languageId")
            .and_then(|l| l.as_str())
            .unwrap_or(name)
            .to_string();
        out.push(LspServer {
            name: name.clone(),
            command,
            args,
            extensions,
            language_id,
        });
    }
    out
}

/// User config (overrides) merged over the built-in defaults, matched by name.
fn load_servers() -> Vec<LspServer> {
    let path = std::env::var("ANGEL_LSP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".angel0/lsp.json")
        });
    let user = std::fs::read_to_string(&path)
        .map(|t| parse_lsp_config(&t))
        .unwrap_or_default();
    let mut servers = default_servers();
    for u in user {
        if let Some(existing) = servers.iter_mut().find(|s| s.name == u.name) {
            *existing = u;
        } else {
            servers.push(u);
        }
    }
    servers
}

/// Choose the server for a file by extension (case-insensitive).
fn pick_server<'a>(servers: &'a [LspServer], path: &Path) -> Option<&'a LspServer> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    servers
        .iter()
        .find(|s| s.extensions.iter().any(|e| e == &ext))
}

// ---------------------------------------------------------------------------
// Content-Length framing (the LSP base protocol)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The stdio client
// ---------------------------------------------------------------------------

/// What to send for a document we're about to analyze: a first `didOpen`, or a
/// `didChange` (with the next version) if the server already has it open. LSP
/// servers reject a second `didOpen` for the same URI, so a cached client must
/// switch to `didChange`.
#[derive(Debug, PartialEq)]
enum DocAction {
    Open,
    Unchanged,
    Change { version: i64 },
}

/// Decide the sync action from the document's last-seen version (`None` = never
/// opened on this client). Pure so the open/change/version logic is testable
/// without a live server.
fn doc_action(last_version: Option<i64>) -> DocAction {
    match last_version {
        None => DocAction::Open,
        Some(v) => DocAction::Change { version: v + 1 },
    }
}

/// A live connection to one language-server subprocess, reused across calls.
struct LspClient {
    name: String,
    /// Shared with the reader thread so it can answer server→client requests.
    stdin: Arc<Mutex<ChildStdin>>,
    /// Inbound responses + notifications (server requests are answered + dropped
    /// by the reader thread). The Mutex serializes request/collect so two callers
    /// never split each other's messages off the channel.
    rx: Mutex<mpsc::Receiver<Value>>,
    next_id: AtomicU64,
    timeout: Duration,
    child: Mutex<Child>,
    /// URIs this client has `didOpen`'d, with their current version — so the
    /// next analysis of the same file becomes a `didChange`, not a re-open.
    open_docs: Mutex<HashMap<String, i64>>,
    open_text: Mutex<HashMap<String, String>>,
    /// Whether the server advertised pull diagnostics (`diagnosticProvider`).
    /// Pull (request/response) is how we re-query a *warm* document, since push
    /// (`publishDiagnostics`) only reliably fires on the first analysis.
    supports_pull: AtomicBool,
    readiness: Mutex<Option<bool>>,
    diagnostics: Mutex<HashMap<String, Value>>,
}

impl LspClient {
    fn spawn(server: &LspServer, timeout: Duration) -> Result<Self, String> {
        let mut child = Command::new(&server.command)
            .args(&server.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn_owned()
            .map_err(|e| format!("spawn {}: {e}", server.command))?;
        let child_stdin = child.stdin.take().ok_or("no child stdin")?;
        let stdout = child.stdout.take().ok_or("no child stdout")?;
        let stdin = Arc::new(Mutex::new(child_stdin));
        let (tx, rx) = mpsc::channel();
        let reader_stdin = Arc::clone(&stdin);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            // Loop ends on EOF / broken frame (anything but `Ok(Some)`).
            while let Ok(Some(v)) = read_frame(&mut reader) {
                // Server→client requests (progress tokens, capability registration,
                // configuration) are answered here and NOT forwarded — an
                // unanswered one can stall the server on a big project.
                if is_server_request(&v) {
                    if let Ok(mut s) = reader_stdin.lock() {
                        let _ = Self::send(&mut *s, &server_request_reply(&v));
                    }
                    continue;
                }
                if tx.send(v).is_err() {
                    break; // client dropped
                }
            }
        });
        Ok(Self {
            name: server.name.clone(),
            stdin,
            rx: Mutex::new(rx),
            next_id: AtomicU64::new(1),
            timeout,
            child: Mutex::new(child),
            open_docs: Mutex::new(HashMap::new()),
            open_text: Mutex::new(HashMap::new()),
            supports_pull: AtomicBool::new(false),
            readiness: Mutex::new(None),
            diagnostics: Mutex::new(HashMap::new()),
        })
    }

    fn send<W: Write>(mut w: W, value: &Value) -> Result<(), String> {
        w.write_all(&build_frame(value))
            .map_err(|e| format!("lsp write: {e}"))?;
        w.flush().map_err(|e| format!("lsp flush: {e}"))
    }

    fn write_msg(&self, value: &Value) -> Result<(), String> {
        let mut s = self.stdin.lock().map_err(|_| "lsp stdin poisoned")?;
        Self::send(&mut *s, value).map_err(|e| format!("lsp {}: {e}", self.name))
    }

    /// Send a request; drain inbound messages until the matching response or the
    /// deadline. Notifications seen meanwhile are dropped (callers that need them
    /// — diagnostics — collect *after* their triggering notification). The `rx`
    /// lock serializes requests so they don't split each other's messages.
    fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.request_with_timeout(method, params, self.timeout)
    }

    fn request_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let rx = self.rx.lock().map_err(|_| "lsp rx poisoned")?;
        self.write_msg(&msg)?;
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("lsp {} timed out on {method}", self.name))?;
            match rx.recv_timeout(remaining) {
                Ok(v) => {
                    self.remember_diagnostics(&v);
                    if let Some(res) = match_response(&v, id) {
                        return res.map_err(|e| format!("lsp {}: {e}", self.name));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(format!("lsp {} timed out on {method}", self.name));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(format!("lsp {} closed the connection", self.name));
                }
            }
        }
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.write_msg(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    /// `initialize` (advertising the workspace root + pull-diagnostics support) +
    /// the `initialized` follow-up. Records whether the server offers pull
    /// diagnostics so warm re-queries can use it.
    fn initialize(&self, root: &Path) -> Result<(), String> {
        let res = self.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": path_to_uri(root),
                "capabilities": {
                    "workspace": {
                        "symbol": { "dynamicRegistration": false }
                    },
                    "textDocument": {
                        "publishDiagnostics": { "relatedInformation": false },
                        "diagnostic": { "dynamicRegistration": false, "relatedDocumentSupport": false },
                        "documentSymbol": { "hierarchicalDocumentSymbolSupport": true }
                    }
                },
                "clientInfo": { "name": "angel0-cockpit", "version": env!("CARGO_PKG_VERSION") },
            }),
        )?;
        let pull = res.pointer("/capabilities/diagnosticProvider").is_some();
        self.supports_pull.store(pull, Ordering::Relaxed);
        self.notify("initialized", json!({}))?;
        self.notify(
            "workspace/didChangeConfiguration",
            json!({"settings": {
                "implicitProjectConfiguration": {"checkJs": true}
            }}),
        )
    }

    fn remember_diagnostics(&self, message: &Value) {
        if message["method"] == "textDocument/publishDiagnostics"
            && let Some(uri) = message["params"]["uri"].as_str()
            && let Ok(mut diagnostics) = self.diagnostics.lock()
        {
            diagnostics.insert(uri.to_owned(), message["params"].clone());
        }
    }

    fn supports_pull(&self) -> bool {
        self.supports_pull.load(Ordering::Relaxed)
    }

    /// Pull the current diagnostics for `uri` (`textDocument/diagnostic`, LSP
    /// 3.17). Request/response, so it works on a warm server without waiting for
    /// a push. Normalizes the report to `{ uri, diagnostics: [...] }` so
    /// [`format_diagnostics`] handles it like a push payload.
    fn pull_diagnostics_with_timeout(&self, uri: &str, timeout: Duration) -> Result<Value, String> {
        let res = self.request_with_timeout(
            "textDocument/diagnostic",
            json!({ "textDocument": { "uri": uri } }),
            timeout,
        )?;
        Ok(pull_report_to_params(uri, &res))
    }

    /// Make the server hold `text` for `uri`: first time → `didOpen`; thereafter
    /// → `didChange` (full sync) with a bumped version. Returns the action taken
    /// so the caller can pick push (first analysis) vs pull (warm re-query).
    fn sync_doc(&self, uri: &str, language_id: &str, text: &str) -> Result<DocAction, String> {
        let mut contents = self
            .open_text
            .lock()
            .map_err(|_| "lsp open_text poisoned")?;
        if contents.get(uri).is_some_and(|previous| previous == text) {
            return Ok(DocAction::Unchanged);
        }
        let action = {
            let docs = self
                .open_docs
                .lock()
                .map_err(|_| "lsp open_docs poisoned")?;
            doc_action(docs.get(uri).copied())
        };
        self.diagnostics
            .lock()
            .map_err(|_| "lsp diagnostics poisoned")?
            .remove(uri);
        match &action {
            DocAction::Unchanged => unreachable!(),
            DocAction::Open => {
                self.notify(
                    "textDocument/didOpen",
                    json!({ "textDocument": {
                        "uri": uri, "languageId": language_id, "version": 1, "text": text,
                    }}),
                )?;
                self.open_docs
                    .lock()
                    .map_err(|_| "lsp open_docs poisoned")?
                    .insert(uri.into(), 1);
            }
            DocAction::Change { version } => {
                self.notify(
                    "textDocument/didChange",
                    json!({
                        "textDocument": { "uri": uri, "version": version },
                        "contentChanges": [ { "text": text } ], // full-document sync
                    }),
                )?;
                self.open_docs
                    .lock()
                    .map_err(|_| "lsp open_docs poisoned")?
                    .insert(uri.into(), *version);
            }
        }
        contents.insert(uri.to_owned(), text.to_owned());
        Ok(action)
    }

    /// Collect `publishDiagnostics` for `uri` after `didOpen`. Servers may emit an
    /// empty set first, then refine once indexing completes — so we wait up to
    /// `timeout` for the *first* report, then keep the latest seen during a short
    /// settle window. Returns the most recent params, or `None` if none arrived.
    fn collect_diagnostics(&self, uri: &str, settle: Duration) -> Option<Value> {
        self.collect_diagnostics_with_timeout(uri, self.timeout, settle)
    }

    fn collect_diagnostics_with_timeout(
        &self,
        uri: &str,
        timeout: Duration,
        settle: Duration,
    ) -> Option<Value> {
        let rx = self.rx.lock().ok()?;
        let deadline = Instant::now() + timeout;
        let mut latest = self.diagnostics.lock().ok()?.get(uri).cloned();
        let mut settle_until = latest.as_ref().map(|_| Instant::now() + settle);
        loop {
            let cap = settle_until.map_or(deadline, |settled| settled.min(deadline));
            // Use saturating (returns ZERO past the deadline), not
            // `checked_duration_since(..)?` — the `?` returned None when `now`
            // slipped just past `cap`, bailing the whole function and DISCARDING
            // the diagnostics already in `latest` (silently dropping real
            // post-edit compile errors). Falling through to `break` returns them.
            let remaining = cap.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(v) => {
                    self.remember_diagnostics(&v);
                    if let Some(params) = publish_diagnostics_for(&v, uri) {
                        latest = Some(params.clone());
                        // got a report — keep draining briefly for a refined one
                        settle_until = Some(Instant::now() + settle);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        latest
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ---------------------------------------------------------------------------
// The server cache
// ---------------------------------------------------------------------------

/// Keeps one live, initialized server per language alive across tool calls.
/// Spawning rust-analyzer re-indexes the whole crate (seconds); caching pays that
/// once, and a warm server is what `didChange`/defs/refs/hover need anyway.
///
/// One pool is shared process-wide (see [`shared_pool`]) so a registry rebuild
/// — `/cd`, headless turn setup — re-lists tools on the warm subprocess instead
/// of respawning it. Each cached client records the **index root** it was
/// initialized for; a caller whose workspace lies inside that root reuses the
/// client, and a cross-root caller retires it and respawns. Path resolution and
/// the workspace boundary stay pinned to the caller's **resolve root**
/// (`LspCtx::resolve_root`), never to the pool's index root.
struct LspPool {
    timeout: Duration,
    init_failure_ttl: Duration,
    now: Arc<dyn Fn() -> Instant + Send + Sync>,
    state: Mutex<LspPoolState>,
}

/// Every field which can change how a named server starts or speaks LSP. The
/// pool key remains the human language name, while this fingerprint prevents a
/// stale warm client, in-flight initialization, or negative result from
/// surviving a config change under that name.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LspServerFingerprint {
    command: String,
    args: Vec<String>,
    extensions: Vec<String>,
    language_id: String,
}

impl From<&LspServer> for LspServerFingerprint {
    fn from(server: &LspServer) -> Self {
        Self {
            command: server.command.clone(),
            args: server.args.clone(),
            extensions: server.extensions.clone(),
            language_id: server.language_id.clone(),
        }
    }
}

struct CachedLspClient {
    fingerprint: LspServerFingerprint,
    client: Arc<LspClient>,
    /// The workspace root this client was initialized for (its index root).
    /// A caller whose workspace lies inside it reuses the warm client; a
    /// cross-root caller retires and respawns.
    index_root: PathBuf,
}

#[derive(Clone)]
struct CachedInitFailure {
    fingerprint: LspServerFingerprint,
    error: String,
    expires_at: Instant,
}

/// One same-fingerprint cold start. The leader performs spawn + initialize
/// without holding the pool lock; followers wait here and receive the exact
/// same client/error. This is per server, so a slow language never blocks warm
/// requests or another language's cold start.
struct InitFlight {
    fingerprint: LspServerFingerprint,
    index_root: PathBuf,
    result: Mutex<Option<Result<Arc<LspClient>, String>>>,
    ready: Condvar,
}

impl InitFlight {
    fn new(fingerprint: LspServerFingerprint, index_root: PathBuf) -> Self {
        Self {
            fingerprint,
            index_root,
            result: Mutex::new(None),
            ready: Condvar::new(),
        }
    }

    fn wait(&self) -> Result<Arc<LspClient>, String> {
        let mut result = self
            .result
            .lock()
            .map_err(|_| "lsp initialization flight poisoned".to_string())?;
        while result.is_none() {
            result = self
                .ready
                .wait(result)
                .map_err(|_| "lsp initialization flight poisoned".to_string())?;
        }
        match result.as_ref().expect("checked above") {
            Ok(client) => Ok(Arc::clone(client)),
            Err(error) => Err(error.clone()),
        }
    }

    fn finish(&self, outcome: &Result<Arc<LspClient>, String>) {
        if let Ok(mut result) = self.result.lock() {
            *result = Some(outcome.clone());
            self.ready.notify_all();
        }
    }
}

#[derive(Default)]
struct LspPoolState {
    clients: HashMap<String, CachedLspClient>,
    flights: HashMap<String, Arc<InitFlight>>,
    failures: HashMap<String, CachedInitFailure>,
}

enum ColdStartDecision {
    Ready(Arc<LspClient>),
    Wait(Arc<InitFlight>),
    Lead(Arc<InitFlight>),
    Failed(String),
}

const LSP_INIT_FAILURE_TTL: Duration = Duration::from_secs(5);

fn is_cacheable_spawn_failure(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("no such file or directory")
        || error.contains("permission denied")
        || error.contains("not found")
        || error.contains("os error 2")
        || error.contains("os error 13")
}

fn is_cancellation_like(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("timed out")
        || error.contains("timeout")
        || error.contains("cancel")
        || error.contains("abort")
        || error.contains("interrupt")
}

fn is_cacheable_initialize_failure(error: &str) -> bool {
    !is_cancellation_like(error)
}

impl LspPool {
    fn new(timeout: Duration) -> Self {
        Self::with_clock(timeout, LSP_INIT_FAILURE_TTL, Arc::new(Instant::now))
    }

    fn with_clock(
        timeout: Duration,
        init_failure_ttl: Duration,
        now: Arc<dyn Fn() -> Instant + Send + Sync>,
    ) -> Self {
        Self {
            timeout,
            init_failure_ttl,
            now,
            state: Mutex::new(LspPoolState::default()),
        }
    }

    /// Return the warm client for `server`, spawning + initializing it on first
    /// use. Same-fingerprint cold callers share one initialization flight. The
    /// global pool lock is held only while choosing/publishing that flight, never
    /// during process startup, protocol I/O, or follower waiting.
    ///
    /// `workspace_root` is the caller's resolve root: it seeds the initialize
    /// root for a cold start, and a cached client is reused only when the
    /// workspace lies inside the client's index root (a `/cd` into a
    /// subdirectory keeps the warm analyzer; a cross-root move respawns it).
    fn get_or_spawn(
        &self,
        server: &LspServer,
        workspace_root: &Path,
    ) -> Result<Arc<LspClient>, String> {
        let fingerprint = LspServerFingerprint::from(server);
        let now = (self.now)();
        // Removing the last Arc may kill+wait for a subprocess. Keep retired
        // clients outside the locked state so even invalidation cannot turn into
        // a global pool stall.
        let mut retired_client = None;
        let decision = {
            let mut state = self.state.lock().map_err(|_| "lsp pool poisoned")?;

            let reusable = state.clients.get(&server.name).is_some_and(|cached| {
                cached.fingerprint == fingerprint && workspace_root.starts_with(&cached.index_root)
            });
            if state.clients.get(&server.name).is_some_and(|_| !reusable) {
                // Config change OR a workspace outside the warm client's index
                // root: retire it (dropped outside the lock) and respawn.
                retired_client = state.clients.remove(&server.name);
            }
            if let Some(cached) = state.clients.get(&server.name) {
                ColdStartDecision::Ready(Arc::clone(&cached.client))
            } else {
                let stale_failure = state.failures.get(&server.name).is_some_and(|failure| {
                    failure.fingerprint != fingerprint || failure.expires_at <= now
                });
                if stale_failure {
                    state.failures.remove(&server.name);
                }

                if let Some(failure) = state.failures.get(&server.name) {
                    ColdStartDecision::Failed(failure.error.clone())
                } else {
                    let matching_flight = state
                        .flights
                        .get(&server.name)
                        .filter(|flight| {
                            flight.fingerprint == fingerprint
                                && workspace_root.starts_with(&flight.index_root)
                        })
                        .cloned();
                    if let Some(flight) = matching_flight {
                        ColdStartDecision::Wait(flight)
                    } else {
                        let flight = Arc::new(InitFlight::new(
                            fingerprint.clone(),
                            workspace_root.to_path_buf(),
                        ));
                        state
                            .flights
                            .insert(server.name.clone(), Arc::clone(&flight));
                        ColdStartDecision::Lead(flight)
                    }
                }
            }
        };
        drop(retired_client);

        match decision {
            ColdStartDecision::Ready(client) => Ok(client),
            ColdStartDecision::Failed(error) => Err(error),
            ColdStartDecision::Wait(flight) => flight.wait(),
            ColdStartDecision::Lead(flight) => {
                self.run_cold_start(server, fingerprint, flight, workspace_root)
            }
        }
    }

    fn run_cold_start(
        &self,
        server: &LspServer,
        fingerprint: LspServerFingerprint,
        flight: Arc<InitFlight>,
        index_root: &Path,
    ) -> Result<Arc<LspClient>, String> {
        let (outcome, cache_failure) = match LspClient::spawn(server, self.timeout) {
            Ok(client) => {
                let client = Arc::new(client);
                match client.initialize(index_root) {
                    Ok(()) => (Ok(client), false),
                    Err(error) => {
                        let cache = is_cacheable_initialize_failure(&error);
                        (Err(error), cache)
                    }
                }
            }
            Err(error) => {
                let error = format!("{error} — is `{}` installed?", server.command);
                let cache = is_cacheable_spawn_failure(&error);
                (Err(error), cache)
            }
        };

        let mut displaced_client = None;
        if let Ok(mut state) = self.state.lock() {
            let owns_flight = state
                .flights
                .get(&server.name)
                .is_some_and(|current| Arc::ptr_eq(current, &flight));
            if owns_flight {
                state.flights.remove(&server.name);
                match &outcome {
                    Ok(client) => {
                        state.failures.remove(&server.name);
                        displaced_client = state.clients.insert(
                            server.name.clone(),
                            CachedLspClient {
                                fingerprint,
                                client: Arc::clone(client),
                                index_root: index_root.to_path_buf(),
                            },
                        );
                    }
                    Err(error) if cache_failure => {
                        state.failures.insert(
                            server.name.clone(),
                            CachedInitFailure {
                                fingerprint,
                                error: error.clone(),
                                expires_at: (self.now)() + self.init_failure_ttl,
                            },
                        );
                    }
                    Err(_) => {
                        state.failures.remove(&server.name);
                    }
                }
            }
        }
        drop(displaced_client);
        flight.finish(&outcome);
        outcome
    }

    /// Return an already initialized client without creating a subprocess. The
    /// post-edit hot path uses this so a mutation never inherits a cold server's
    /// multi-second startup/indexing cost; normal explicit LSP tools still spawn.
    /// The cached client must cover `workspace_root` (inside its index root) or
    /// the warm result is None — a cross-root warm hit would misanalyze.
    fn get_warm(&self, server: &LspServer, workspace_root: &Path) -> Option<Arc<LspClient>> {
        let fingerprint = LspServerFingerprint::from(server);
        let mut retired = None;
        let client = self.state.lock().ok().and_then(|mut state| {
            let cached = state.clients.get(&server.name);
            let reusable = cached.is_some_and(|cached| {
                cached.fingerprint == fingerprint && workspace_root.starts_with(&cached.index_root)
            });
            if cached.is_some_and(|_| !reusable) {
                retired = state.clients.remove(&server.name);
                state.failures.remove(&server.name);
                return None;
            }
            state
                .clients
                .get(&server.name)
                .map(|cached| Arc::clone(&cached.client))
        });
        drop(retired);
        client
    }

    /// Drop the cached client for a language (e.g. after it died), so the next
    /// call respawns it.
    fn evict(&self, name: &str) {
        let retired = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.clients.remove(name));
        drop(retired);
    }

    /// Snapshot of the currently-warm clients (language, client). Used by
    /// project-wide queries that have no file to route by.
    fn warm_clients(&self) -> Vec<(String, Arc<LspClient>)> {
        self.state
            .lock()
            .map(|state| {
                state
                    .clients
                    .iter()
                    .map(|(name, cached)| (name.clone(), Arc::clone(&cached.client)))
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Shared tool context — one warm pool + server map behind every LSP tool
// ---------------------------------------------------------------------------

fn env_duration(key: &str, default: Duration, unit: fn(u64) -> Duration) -> Duration {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(unit)
        .unwrap_or(default)
}

/// Is this error a dead/broken connection (vs a logic error, timeout, or server
/// error response)? Only these justify evicting + respawning the warm server —
/// a timeout is usually just a slow cold index under load, and a logic error is
/// the caller's, neither worth discarding an indexed server.
fn is_dead_client(err: &str) -> bool {
    err.contains("closed the connection")
        || err.contains("lsp write")
        || err.contains("lsp flush")
        || err.contains("Broken pipe")
        || err.contains("poisoned")
}

/// Ensure exactly one `tool error:` prefix (op errors from `request()` aren't
/// prefixed; `resolve_position`'s already is — don't double it).
fn tool_err(e: String) -> String {
    if e.starts_with("tool error:") {
        e
    } else {
        format!("tool error: {e}")
    }
}

/// The state shared by `lsp_diagnostics` and the navigation tools so a single
/// warm server answers all of them.
struct LspCtx {
    pool: Arc<LspPool>,
    /// The current workspace: relative paths resolve against it and the
    /// boundary check pins to it. Independent of the pool's per-client index
    /// roots — a `/cd` into a subdirectory re-resolves here while the warm
    /// analyzer (indexed at the parent) is reused.
    resolve_root: PathBuf,
    servers: Vec<LspServer>,
    timeout: Duration,
    settle: Duration,
}

/// The process-wide pool: one per server binary, surviving registry rebuilds.
fn shared_pool(timeout: Duration) -> Arc<LspPool> {
    static POOL: std::sync::OnceLock<Arc<LspPool>> = std::sync::OnceLock::new();
    Arc::clone(POOL.get_or_init(|| Arc::new(LspPool::new(timeout))))
}

impl LspCtx {
    fn new(workspace: PathBuf) -> Self {
        let timeout = env_duration(
            "ANGEL_LSP_TIMEOUT",
            Duration::from_secs(30),
            Duration::from_secs,
        );
        let settle = env_duration(
            "ANGEL_LSP_SETTLE_MS",
            Duration::from_millis(1200),
            Duration::from_millis,
        );
        Self {
            pool: shared_pool(timeout),
            resolve_root: workspace,
            servers: load_servers(),
            timeout,
            settle,
        }
    }

    fn resolve(&self, rel: &str) -> Result<PathBuf, String> {
        let p = Path::new(rel);
        let abs = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.resolve_root.join(p)
        };
        let abs = abs
            .canonicalize()
            .map_err(|e| format!("tool error: cannot resolve {rel}: {e}"))?;
        if !abs.starts_with(&self.resolve_root) {
            return Err(format!("tool error: {rel} is outside the workspace"));
        }
        Ok(abs)
    }

    /// Resolve `path`, pick its server, read it, get/spawn the warm client, sync
    /// the document, then run `op` against the prepared client. Only a *dead
    /// connection* triggers evict + respawn (once); a logic error (symbol not
    /// found), timeout, or server error returns as-is so a warm server isn't
    /// thrown away — re-indexing on a user mistake would be a costly own-goal. A
    /// legitimately empty result is `Ok`, not an error.
    fn with_doc<T>(
        &self,
        path: &str,
        op: impl Fn(&LspClient, &str, DocAction, &str) -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_doc_mode(path, false, op)
    }

    fn with_doc_mode<T>(
        &self,
        path: &str,
        warm_only: bool,
        op: impl Fn(&LspClient, &str, DocAction, &str) -> Result<T, String>,
    ) -> Result<T, String> {
        let abs = self.resolve(path)?;
        let server = pick_server(&self.servers, &abs).ok_or_else(|| {
            format!(
                "tool error: no language server configured for {} (configure ~/.angel0/lsp.json)",
                abs.display()
            )
        })?;
        let text = std::fs::read_to_string(&abs)
            .map_err(|e| format!("tool error: read {}: {e}", abs.display()))?;
        let uri = path_to_uri(&abs);
        let mut last = String::new();
        for _ in 0..2 {
            let client = if warm_only {
                self.pool
                    .get_warm(server, &self.resolve_root)
                    .ok_or_else(|| {
                        format!(
                            "tool error: {} is not warm; post-edit diagnostics skipped",
                            server.name
                        )
                    })?
            } else {
                self.pool
                    .get_or_spawn(server, &self.resolve_root)
                    .map_err(|e| format!("tool error: {e}"))?
            };
            let result = client
                .sync_doc(&uri, parity::language_id(&abs, &server.language_id), &text)
                .and_then(|action| op(&client, &uri, action, &text));
            match result {
                Ok(t) => return Ok(t),
                Err(e) if is_dead_client(&e) => {
                    last = e;
                    self.pool.evict(&server.name); // genuinely dead — respawn next loop
                    if warm_only {
                        return Err(tool_err(last));
                    }
                }
                Err(e) => return Err(tool_err(e)), // logic/timeout/server error: keep the warm server
            }
        }
        Err(tool_err(format!(
            "{last} (after respawning {})",
            server.command
        )))
    }

    /// Block until the server has analyzed a freshly-opened document (the first
    /// `publishDiagnostics` push is the ready signal — a position query before it
    /// returns null). Warm documents are already analyzed, so this is a no-op.
    fn ensure_ready(&self, client: &LspClient, uri: &str, _action: &DocAction) -> String {
        let mut state = client.readiness.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ready) = *state {
            return format!(
                "readiness: {}; readiness_wait_ms: 0",
                if ready {
                    "cached_ready"
                } else {
                    "cached_unconfirmed"
                }
            );
        }
        let started = Instant::now();
        let ready = client.collect_diagnostics(uri, self.settle).is_some();
        *state = Some(ready);
        format!(
            "readiness: {}; readiness_wait_ms: {}",
            if ready {
                "diagnostics_received"
            } else {
                "diagnostics_timeout"
            },
            started.elapsed().as_millis()
        )
    }
}

// ---------------------------------------------------------------------------
// The tools
// ---------------------------------------------------------------------------

/// `lsp_diagnostics(path)` — ground-truth errors/warnings from a language server.
pub struct LspDiagnosticsTool {
    ctx: Arc<LspCtx>,
}

impl Tool for LspDiagnosticsTool {
    fn name(&self) -> &str {
        "lsp_diagnostics"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "lsp_diagnostics".into(),
            description: "Ground-truth errors/warnings for a source file from a language server \
                          (rust-analyzer/pyright/tsserver/…). Opens the file and returns its \
                          diagnostics (severity line:col message). Use to verify an edit \
                          compiles/type-checks instead of guessing. Read-only."
                .into(),
            params: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File to analyze (workspace-relative or absolute)." }
                },
                "required": ["path"]
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or("tool error: `path` required")?;
        let ctx = &self.ctx;
        // Internal post-edit calls are deliberately absent from the advertised
        // schema: they reuse this tool while imposing a smaller hot-path budget
        // and refusing a synchronous cold spawn. Ordinary model-authored calls
        // retain the full configured LSP timeout and spawn behavior.
        let warm_only = args
            .get("_warm_only")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let deadline = args
            .get("_deadline_ms")
            .and_then(Value::as_u64)
            .map(|ms| Duration::from_millis(ms.clamp(50, 5_000)))
            .unwrap_or(ctx.timeout);
        let settle = ctx.settle.min(deadline);
        ctx.with_doc_mode(path, warm_only, |client, uri, action, _text| {
            // Cold open: wait for the push (analysis-ready). Warm change: pull.
            let warm_pull = !matches!(action, DocAction::Open) && client.supports_pull();
            let params = if warm_pull {
                client.pull_diagnostics_with_timeout(uri, deadline)?
            } else {
                match client.collect_diagnostics_with_timeout(uri, deadline, settle) {
                    Some(p) => p,
                    None => {
                        return Ok(format!(
                            "{path}: no diagnostics within {}ms (server may still be indexing)",
                            deadline.as_millis()
                        ));
                    }
                }
            };
            Ok(format_diagnostics(path, &params))
        })
    }
}

/// The position-based navigation queries, which share all machinery and differ
/// only by LSP method + result shape.
#[derive(Clone, Copy)]
enum NavKind {
    Definition,
    References,
    Hover,
}

impl NavKind {
    fn tool_name(self) -> &'static str {
        match self {
            NavKind::Definition => "lsp_definition",
            NavKind::References => "lsp_references",
            NavKind::Hover => "lsp_hover",
        }
    }
    fn method(self) -> &'static str {
        match self {
            NavKind::Definition => "textDocument/definition",
            NavKind::References => "textDocument/references",
            NavKind::Hover => "textDocument/hover",
        }
    }
    fn description(self) -> &'static str {
        match self {
            NavKind::Definition => {
                "Jump to where a symbol is DEFINED (path:line:col) via the \
                language server. Give `symbol` (first occurrence is queried) or an explicit \
                1-based `line`(+`character`). Read-only."
            }
            NavKind::References => {
                "List every USE of a symbol (path:line:col) via the language \
                server. Give `symbol` or 1-based `line`(+`character`). Read-only."
            }
            NavKind::Hover => {
                "Type signature / doc for a symbol via the language server. Give \
                `symbol` or 1-based `line`(+`character`). Read-only."
            }
        }
    }
}

/// `lsp_definition` / `lsp_references` / `lsp_hover` — position queries over the
/// same warm server pool as diagnostics.
pub struct LspNavTool {
    kind: NavKind,
    ctx: Arc<LspCtx>,
}

impl Tool for LspNavTool {
    fn name(&self) -> &str {
        self.kind.tool_name()
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.kind.tool_name().into(),
            description: self.kind.description().into(),
            params: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Source file (workspace-relative or absolute)." },
                    "symbol": { "type": "string", "description": "Symbol to locate (first occurrence in the file). Use this OR line/character." },
                    "line": { "type": "integer", "description": "1-based line of the symbol (alternative to `symbol`)." },
                    "character": { "type": "integer", "description": "1-based column on `line` (default 1)." }
                },
                "required": ["path"]
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or("tool error: `path` required")?;
        let ctx = &self.ctx;
        let kind = self.kind;
        let heuristic = parity::navigation(&ctx.resolve_root, args, kind)?;
        let query = parity::definition_query(args, &heuristic, kind);
        let path = query["path"].as_str().unwrap_or(path);
        let mut readiness = String::from("readiness: unavailable; readiness_wait_ms: 0");
        // The closure is retryable; collect metadata independently of its output.
        let observed = Mutex::new(String::new());
        let semantic = ctx.with_doc(path, |client, uri, action, text| {
            let (line, ch) = parity::query_position(&query, text)?;
            *observed.lock().unwrap() = ctx.ensure_ready(client, uri, &action);
            let mut params = json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": ch },
            });
            if matches!(kind, NavKind::References) {
                params["context"] = json!({ "includeDeclaration": true });
            }
            let result = client.request(kind.method(), params)?;
            Ok(match kind {
                NavKind::Hover => format_hover(&result),
                _ => format_locations(&result),
            })
        });
        if matches!(kind, NavKind::Hover)
            || (heuristic.starts_with("no matches")
                && semantic.as_ref().is_err_and(|e| e.contains("not found")))
        {
            return semantic;
        }
        if !observed.lock().unwrap().is_empty() {
            readiness = observed.into_inner().unwrap();
        }
        Ok(parity::merge(&heuristic, semantic, &readiness))
    }
}

/// `lsp_symbols(path)` — the analyzer's outline of a file (functions/structs/…),
/// hierarchical, beating the regex `outline`/`defs`. Position-free, so it's its
/// own tool rather than a `NavKind`.
pub struct LspSymbolsTool {
    ctx: Arc<LspCtx>,
}

impl Tool for LspSymbolsTool {
    fn name(&self) -> &str {
        "lsp_symbols"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "lsp_symbols".into(),
            description: "Outline a file's symbols (functions, structs, methods, …) from the \
                          language server — indented, with each symbol's line. Analyzer-grade \
                          (handles nesting/visibility), unlike the regex `outline`/`defs`. \
                          Read-only."
                .into(),
            params: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Source file to outline (workspace-relative or absolute)." }
                },
                "required": ["path"]
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or("tool error: `path` required")?;
        let ctx = &self.ctx;
        let heuristic = parity::outline(&ctx.resolve_root, args)?;
        let observed = Mutex::new(String::from("readiness: unavailable; readiness_wait_ms: 0"));
        let semantic = ctx.with_doc(path, |client, uri, action, _text| {
            *observed.lock().unwrap() = ctx.ensure_ready(client, uri, &action);
            let result = client.request(
                "textDocument/documentSymbol",
                json!({"textDocument": { "uri": uri }}),
            )?;
            Ok(format_document_symbols(&result))
        });
        Ok(parity::merge(
            &heuristic,
            semantic,
            &observed.into_inner().unwrap(),
        ))
    }
}

/// `lsp_workspace_symbol(query)` — find a symbol ANYWHERE in the project (not one
/// file). Queries every currently-warm server and merges; no path to route by, so
/// a server must already be warm (any prior `lsp_*` file call loads one).
pub struct LspWorkspaceSymbolTool {
    ctx: Arc<LspCtx>,
}

impl Tool for LspWorkspaceSymbolTool {
    fn name(&self) -> &str {
        "lsp_workspace_symbol"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "lsp_workspace_symbol".into(),
            description: "Find a symbol by name ANYWHERE in the project (functions/structs/…), \
                          via the language server's workspace index — more precise than grepping \
                          for the name. Returns `kind name  path:line`. A server must be warm \
                          first (run lsp_symbols/lsp_diagnostics on any project file). Read-only."
                .into(),
            params: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Symbol name (or prefix/substring) to search for." }
                },
                "required": ["query"]
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|q| q.as_str())
            .ok_or("tool error: `query` required")?;
        let warm = self.ctx.pool.warm_clients();
        if warm.is_empty() {
            return Ok(
                "no language server is warm yet — run lsp_symbols or lsp_diagnostics on a \
                       project file first to load one, then retry."
                    .into(),
            );
        }
        let mut merged: Vec<Value> = Vec::new();
        let mut errs: Vec<String> = Vec::new();
        for (lang, client) in &warm {
            match client.request("workspace/symbol", json!({ "query": query })) {
                Ok(Value::Array(a)) => merged.extend(a),
                Ok(_) => {} // null / non-array → no hits from this server
                Err(e) => {
                    // A dead client would otherwise error on every future query;
                    // evict it so the next file-based call respawns it.
                    if is_dead_client(&e) {
                        self.ctx.pool.evict(lang);
                    }
                    errs.push(format!("{lang}: {e}"));
                }
            }
        }
        if merged.is_empty() && !errs.is_empty() {
            return Err(format!("tool error: {}", errs.join("; ")));
        }
        Ok(format_workspace_symbols(query, &Value::Array(merged)))
    }
}

/// Build the LSP tool set, opt-in via `ANGEL_LSP`. Returns `(tools, notes)` like
/// [`crate::mcp::discover_mcp_compositions`]. Empty (and a note) when disabled. All tools
/// share one [`LspCtx`] so a single warm server answers diagnostics + navigation.
/// The lowercase file extensions present under `root` — a bounded breadth-first
/// scan (at most `cap` directory entries, vendor/build trees skipped) that
/// decides which language servers are worth pre-warming. A truncated scan just
/// means a rare language warms lazily at first use, exactly as before.
fn present_extensions(root: &Path, cap: usize) -> std::collections::HashSet<String> {
    let mut seen = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::from([root.to_path_buf()]);
    let mut visited = 0usize;
    while let Some(dir) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > cap {
                return seen;
            }
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !matches!(
                    name.as_ref(),
                    ".git" | "target" | "node_modules" | ".venv" | "venv" | "dist" | "build"
                ) {
                    queue.push_back(path);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                seen.insert(ext.to_lowercase());
            }
        }
    }
    seen
}

fn command_on_path_memo() -> &'static Mutex<HashMap<String, bool>> {
    static MEMO: std::sync::OnceLock<Mutex<HashMap<String, bool>>> = std::sync::OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Test-only: drop memoized probes so fixtures that swap PATH or the probed
/// binaries observe fresh results.
#[cfg(test)]
fn reset_command_on_path_cache() {
    if let Ok(mut memo) = command_on_path_memo().lock() {
        memo.clear();
    }
}

/// Is `bin` resolvable as an executable on PATH? No PATH → false.
///
/// Ordinary binaries keep the cheap filesystem check. Rustup installs command
/// proxies even when the corresponding component is absent, so a proxy gets
/// one `--version` preflight; otherwise an unprovisioned
/// `rust-analyzer` silently auto-enables LSP and every call fails after spawn.
/// Memoized per process and per `bin`: the rustup-proxy preflight spawns a
/// child, and startup paths (registry build) probe the same servers repeatedly.
fn command_on_path(bin: &str) -> bool {
    if let Ok(memo) = command_on_path_memo().lock()
        && let Some(&hit) = memo.get(bin)
    {
        return hit;
    }
    let resolved = command_on_path_probe(bin);
    if let Ok(mut memo) = command_on_path_memo().lock() {
        memo.insert(bin.to_string(), resolved);
    }
    resolved
}

fn command_on_path_probe(bin: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(bin);
        if !candidate.is_file() {
            return false;
        }
        let is_rustup_proxy = std::fs::canonicalize(&candidate)
            .ok()
            .and_then(|path| path.file_name().map(|name| name == "rustup"))
            .unwrap_or(false);
        if !is_rustup_proxy {
            return true;
        }
        // Preflight only: the proxy must never provision a toolchain on our
        // behalf. With an isolated or scrubbed HOME (sealed workers, sandboxed
        // benches) rustup would otherwise attempt an install and hold the
        // cockpit's startup for 11+ s before failing (measured 2026-09-05 on a
        // 26.7k-file workspace: 12.8 s to the first model request vs 0.9 s).
        // A bounded wait keeps any other proxy stall off the critical path.
        let Ok(mut child) = std::process::Command::new(&candidate)
            .arg("--version")
            .env("RUSTUP_AUTO_INSTALL", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn_owned()
        else {
            return false;
        };
        let deadline = std::time::Instant::now() + RUSTUP_PROXY_PREFLIGHT_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
        }
    })
}

/// Upper bound for the rustup-proxy `--version` preflight; a proxy that cannot
/// answer within this window is treated as unprovisioned rather than allowed
/// to stall discovery.
const RUSTUP_PROXY_PREFLIGHT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

fn lsp_prewarm_enabled(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

pub fn discover_lsp_tools(workspace: PathBuf) -> (Vec<Box<dyn Tool>>, Vec<String>) {
    // On by default when a server binary for a language actually present in
    // the workspace is on PATH. `ANGEL_LSP=1` forces on unconditionally;
    // `ANGEL_LSP=0` forces off; unset auto-enables only if a known server
    // (rust-analyzer / pyright / tsserver / bash-language-server /
    // yaml-language-server) is reachable. This keeps the post-edit
    // self-correct on for real projects while skipping it where nothing can
    // run — no spurious spawns or error noise.
    let on = match std::env::var("ANGEL_LSP").ok().as_deref() {
        Some("0") => return (Vec::new(), Vec::new()),
        Some(v) if !v.is_empty() => true,
        _ => {
            let present = present_extensions(&workspace, 4096);
            default_servers().iter().any(|s| {
                s.extensions.iter().any(|e| present.contains(e)) && command_on_path(&s.command)
            })
        }
    };
    if !on {
        return (Vec::new(), Vec::new());
    }
    let ctx = Arc::new(LspCtx::new(workspace));
    let langs: Vec<&str> = ctx.servers.iter().map(|s| s.name.as_str()).collect();
    let note = format!(
        "lsp: diagnostics + nav + symbols enabled ({})",
        langs.join(", ")
    );
    // Optional pre-warm: spawn + initialize servers for languages actually
    // present in the workspace, off-thread at registry build. This must stay
    // opt-in: launchers commonly open at $HOME, and eagerly treating that broad
    // directory as one rust-analyzer project can saturate disk and stdout before
    // the first model request. Explicit LSP tool calls still start the matching
    // server lazily, preserving all functionality without taxing chat startup.
    let prewarm = std::env::var("ANGEL_LSP_PREWARM")
        .ok()
        .is_some_and(|value| lsp_prewarm_enabled(Some(&value)));
    if prewarm {
        let warm = Arc::clone(&ctx);
        let _ = std::thread::Builder::new()
            .name("lsp-prewarm".into())
            .spawn(move || {
                let present = present_extensions(&warm.resolve_root, 4096);
                for server in &warm.servers {
                    if server.extensions.iter().any(|e| present.contains(e)) {
                        let _ = warm.pool.get_or_spawn(server, &warm.resolve_root);
                    }
                }
            });
    }
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(LspDiagnosticsTool {
            ctx: Arc::clone(&ctx),
        }),
        Box::new(LspNavTool {
            kind: NavKind::Definition,
            ctx: Arc::clone(&ctx),
        }),
        Box::new(LspNavTool {
            kind: NavKind::References,
            ctx: Arc::clone(&ctx),
        }),
        Box::new(LspNavTool {
            kind: NavKind::Hover,
            ctx: Arc::clone(&ctx),
        }),
        Box::new(LspSymbolsTool {
            ctx: Arc::clone(&ctx),
        }),
        Box::new(LspWorkspaceSymbolTool {
            ctx: Arc::clone(&ctx),
        }),
    ];
    (tools, vec![note])
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/lsp__tests.rs"]
mod tests;
