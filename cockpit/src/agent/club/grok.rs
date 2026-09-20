//! Grok/xAI OAuth seats for MoA grounding.
//!
//! # First-class OAuth (SOTA HTTP seat)
//!
//! Same idea as ChatGPT/Codex OAuth: reuse `~/.grok/auth.json` from
//! `grok login --oauth`, **silently refresh** via `auth.x.ai` when the access
//! JWT ages out, and call `https://api.x.ai/v1` with `Authorization: Bearer`.
//! Scope on the login includes `api:access` — this is a real API seat, not a
//! CLI-only toy. A separately labeled `grok-api` seat can use an explicit
//! xAI-compatible API key; OAuth and API-key seats never fall back into one
//! another.
//!
//! # Persistent ACP research surface (optional tools)
//!
//! The research scout / `grok_research` tool keeps one `grok agent stdio`
//! process alive and opens independent ACP sessions on it. Authentication uses
//! the CLI's `cached_token` method from the same account login. This avoids a
//! fresh CLI boot, leader-socket race, auth discovery, and child-exit polling on
//! every scout call. The ACP child scrubs API-key variables because this
//! method is explicitly the CLI OAuth surface.
//!
//! Reasoning effort is a first-class THINK control on the ACP research path:
//! the panel reads the model catalog's supported rungs and passes
//! `--reasoning-effort` on `grok agent` (not as a top-level flag) when the
//! persistent process starts.
//!
//! ACP is research-only. Grok 4.6's Build agent refuses host-tool markup, so
//! Angel driver hops with tools belong on the HTTP OAuth seat, which speaks
//! native Chat Completions function calling. An experimental
//! `ANGEL_GROK_HARNESS_SURFACE=1` path still denies Grok's nested tools and
//! recovers `<tool_call>` prose, but it is off by default because live Grok
//! 4.6 answers "I can't use that tool-call format" instead of calling tools.

use super::*;
use crate::agent::sandbox::process_owner::{Child, OwnedCommandExt};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub(crate) const GROK_SOTA_MODEL_OPTIONS: &[&str] = &[
    "grok-4.6",
    "grok-4",
    "grok-4-latest",
    "grok-4.5",
    "grok-4.3",
    "grok-4.20",
    "grok-4.20-fast",
    "grok-4.20-mini",
];

/// Fallback THINK ladder when `models_cache.json` has no effort list for the
/// pinned model. Matches the Grok 4.5 catalog shipped with the CLI.
const GROK_DEFAULT_EFFORT_LEVELS: &[&str] = &["low", "medium", "high"];
/// xAI's provider-authored Grok 4.6 capability ladder. Unlike the generic HTTP
/// ladder it has no `none` rung and adds `xhigh`; `high` is the default.
const GROK_46_EFFORT_LEVELS: &[&str] = &["low", "medium", "high", "xhigh"];
const GROK_DEFAULT_MODEL: &str = "grok-4.6";
const GROK_46_CONTEXT_WINDOW: u64 = 500_000;

/// Default wall clock for one ACP prompt. This bounds one failed provider
/// attempt, not the persistent agent lifetime. A research scout sits in front
/// of the real turn and must fail fast when its provider wedges. Override with
/// `ANGEL_GROK_TIMEOUT_SECS`.
const GROK_DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Startup/auth should never consume the whole prompt budget. The CLI refreshes
/// its cached OAuth token during this handshake when necessary.
const GROK_ACP_STARTUP_TIMEOUT_SECS: u64 = 30;

/// Response waits block on ACP events. This is only the maximum Esc-observation
/// latency forced by Club's AtomicBool cancellation API; it is not child/process
/// polling and does no work between events.
const GROK_ACP_CANCEL_GRANULARITY: Duration = Duration::from_millis(100);

/// No cockpit-imposed nested-turn cutoff by default. The old value of 12 could
/// truncate a valid Grok coding/research run even though the outer campaign was
/// healthy. Operators can still set `ANGEL_GROK_MAX_TURNS` to a positive value;
/// the one-attempt wall clock above remains the dead-process guard.
const GROK_DEFAULT_RESEARCH_MAX_TURNS: usize = 0;

/// Shown only when the credentials genuinely cannot be refreshed without the
/// operator — never merely because the access token aged out.
const GROK_LOGIN_HINT: &str = "Grok OAuth credentials need a fresh login; run `grok login --oauth` to rewrite ~/.grok/auth.json";

/// Built-in Grok CLI tool ids stripped on the harness surface so Angel owns
/// the tool loop. Names follow the CLI docs (`run_terminal_cmd`) plus the
/// event-log aliases observed in the field (`run_terminal_command`).
const GROK_HARNESS_DISALLOWED_TOOLS: &str = "\
run_terminal_cmd,run_terminal_command,read_file,search_replace,write,write_file,\
grep,list_dir,web_search,web_fetch,Agent,todo_write,get_command_or_subagent_output,\
spawn_subagent,open_page,image_gen,image_edit,monitor,workflow,use_tool,search_tool,\
bash,shell,read,edit";

/// Short system-prompt override for harness hops. Kept tiny so it never blows
/// ARG_MAX; the host tool catalog rides in the prompt body instead.
const GROK_HARNESS_SYSTEM_OVERRIDE: &str = "\
You are pure completion for the Angel host. You have no executable tools. \
When you need the host to act, emit one or more blocks of the form \
<tool_call name=\"NAME\">{json args}</tool_call> and stop. \
Do not narrate denials, do not claim tools already ran, and do not invent \
results. The host tool catalog and conversation follow in the user prompt.";

/// Experimental ACP host-tool markup switch. **Off by default.** Grok 4.6
/// refuses `<tool_call>` prose and keeps its own tool dialect, so Angel driver
/// hops must use the HTTP OAuth seat. Set `ANGEL_GROK_HARNESS_SURFACE=1` only
/// to retry the denied-tools + markup recovery path against an older CLI.
const GROK_HARNESS_SURFACE_ENV: &str = "ANGEL_GROK_HARNESS_SURFACE";

/// Which Grok ACP surface to open for this call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GrokAcpSurface {
    /// Pure completion: Angel tools via prose markup; Grok tools denied.
    Harness,
    /// Bounded research agent: Grok may use its own tools (web, etc.).
    Research,
}

type GrokEffortSnapshot = Option<(u64, u64, Instant, Option<String>)>;
type GrokAcpSessionCapture = Arc<Mutex<String>>;
type GrokAcpSessions = Arc<Mutex<HashMap<String, GrokAcpSessionCapture>>>;

pub(crate) struct GrokResearchClub {
    name: String,
    mode: GrokMode,
    usage: UsageCell,
    accounting: super::AccountingCell,
    /// Operator THINK-deck override; wins over env/config defaults for this
    /// process. Seat-scoped `chat_with_effort` requests bypass this field.
    effort_override: Mutex<Option<String>>,
    /// Revision+TTL cache of the non-request resolved effort (override/env/
    /// catalog default). Same contract as HttpClub's effort snapshot: THINK
    /// mutations invalidate immediately via `route_state_revision`; formation
    /// pin resync bumps [`super::http::reasoning_env_generation`] and invalidates
    /// immediately. Opt out with `ANGEL_EFFORT_SNAPSHOT=0`.
    effort_snapshot: Mutex<GrokEffortSnapshot>,
    route_state_revision: AtomicU64,
    /// Supported effort ids for the selected model, lowest → highest.
    reasoning_levels: Vec<String>,
    /// Operator-facing descriptions for each effort rung (deck subtitles).
    reasoning_descriptions: Vec<(String, String)>,
    /// Catalog default when no override/env is set.
    default_effort: Option<String>,
    /// Trusted context window from the Grok models cache, when known.
    context_window: Option<u64>,
    /// Long-lived account-OAuth ACP transport. A model/effort/surface mutation
    /// replaces the cached slot; existing in-flight calls retain their Arc and
    /// finish on the old process without being killed underneath another turn.
    acp: Mutex<Option<GrokAcpSlot>>,
}

enum GrokMode {
    AcpOAuth {
        command: String,
        model: Option<String>,
        timeout: Duration,
    },
}

/// Nested CLI turns allowed on a harness hop. Must be >1: with tools denied the
/// model often spends turn 1 on a blocked built-in, exits "Max turns reached",
/// and leaves only prose (or markup) on stdout. One turn made every driver hop
/// fail hard before Angel could recover a `<tool_call>`.
const GROK_HARNESS_MAX_TURNS: usize = 3;

impl GrokResearchClub {
    pub(crate) fn from_env() -> Option<Self> {
        if !grok_research_enabled() || !grok_oauth_available() {
            return None;
        }
        let model = grok_model_from_env();
        let catalog = grok_model_catalog(Some(&model));
        Some(Self {
            name: "grok-research".to_string(),
            mode: GrokMode::AcpOAuth {
                command: grok_command(),
                model: Some(model),
                timeout: env_secs("ANGEL_GROK_TIMEOUT_SECS", GROK_DEFAULT_TIMEOUT_SECS),
            },
            usage: UsageCell::default(),
            accounting: super::AccountingCell::default(),
            effort_override: Mutex::new(None),
            effort_snapshot: Mutex::new(None),
            route_state_revision: AtomicU64::new(0),
            reasoning_levels: catalog.levels,
            reasoning_descriptions: catalog.descriptions,
            default_effort: catalog.default_effort,
            context_window: catalog.context_window,
            acp: Mutex::new(None),
        })
    }

    fn resolved_effort(&self, request: Option<&str>) -> Option<String> {
        if let Some(requested) = request.map(str::trim).filter(|s| !s.is_empty()) {
            return self.canonicalize_effort(requested);
        }
        // Draw-path / no-request resolution: one revision+TTL snapshot instead
        // of re-locking override + re-reading env on every frame.
        if super::http::effort_snapshot_enabled_for_peers() {
            let rev = self.route_state_revision.load(Ordering::Relaxed);
            let env_gen = super::http::reasoning_env_generation();
            if let Ok(mut g) = self.effort_snapshot.lock() {
                if let Some((cached_rev, cached_gen, at, value)) = g.as_ref()
                    && *cached_rev == rev
                    && *cached_gen == env_gen
                    && at.elapsed() < super::http::EFFORT_ENV_TTL_FOR_PEERS
                {
                    return value.clone();
                }
                let value = self.resolved_effort_uncached();
                *g = Some((rev, env_gen, Instant::now(), value.clone()));
                return value;
            }
        }
        self.resolved_effort_uncached()
    }

    fn resolved_effort_uncached(&self) -> Option<String> {
        if let Some(over) = self.effort_override.lock().ok().and_then(|g| g.clone()) {
            return Some(over);
        }
        if let Some(env) = super::http::reasoning_env_var("ANGEL_GROK_REASONING_EFFORT")
            .or_else(|| super::http::reasoning_env_var("ANGEL_REASONING_EFFORT"))
            .map(|effort| effort.trim().to_string())
            .filter(|effort| !effort.is_empty())
            && let Some(canonical) = self.canonicalize_effort(&env)
        {
            return Some(canonical);
        }
        super::model_defaults::entry(&self.model_identity().unwrap_or_default())
            .and_then(|entry| entry.default_effort)
            .and_then(|effort| self.canonicalize_effort(&effort))
            .or_else(|| self.default_effort.clone())
            .or_else(|| self.reasoning_levels.first().cloned())
    }

    fn canonicalize_effort(&self, requested: &str) -> Option<String> {
        self.reasoning_levels
            .iter()
            .find(|level| level.eq_ignore_ascii_case(requested.trim()))
            .cloned()
    }

    fn respond_acp(
        &self,
        command: &str,
        model: Option<&str>,
        timeout: Duration,
        prompt: &str,
        cancel: &AtomicBool,
        effort: Option<&str>,
    ) -> Result<String, String> {
        // Bare respond is the research/scout path — allow Grok tools.
        self.respond_acp_surface(
            command,
            model,
            timeout,
            prompt,
            cancel,
            effort,
            GrokAcpSurface::Research,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn respond_acp_surface(
        &self,
        command: &str,
        model: Option<&str>,
        timeout: Duration,
        prompt: &str,
        cancel: &AtomicBool,
        effort: Option<&str>,
        surface: GrokAcpSurface,
    ) -> Result<String, String> {
        if grok_oauth_needs_login() {
            return Err(GROK_LOGIN_HINT.to_string());
        }
        let key = GrokAcpKey {
            command: command.to_string(),
            model: model.map(str::to_string),
            effort: effort.map(str::to_string),
            surface,
        };

        // One transparent restart is safe only for a transport death. ACP
        // sessions are independent, and a connection can die before the
        // prompt reaches Grok. We never retry timeout/cancel/protocol errors,
        // which could duplicate completed tool side effects.
        for attempt in 0..2 {
            let connection = match self.acp_connection(&key, timeout) {
                Ok(connection) => connection,
                Err(err) if attempt == 0 && err.restartable() => continue,
                Err(err) => return Err(err.operator_message()),
            };
            match connection.prompt(prompt, timeout, cancel, &self.accounting) {
                Ok(reply) => {
                    if let Some(usage) = reply.usage {
                        self.usage.record_turn(
                            usage.input.unwrap_or(0),
                            usage.output.unwrap_or(0),
                            usage.reasoning.unwrap_or(0),
                        );
                    }
                    return Ok(reply.text);
                }
                Err(err) => {
                    self.invalidate_acp_connection(&connection);
                    if attempt == 0 && err.restartable() {
                        continue;
                    }
                    return Err(err.operator_message());
                }
            }
        }
        Err("grok ACP transport failed after one clean restart".to_string())
    }

    fn acp_connection(
        &self,
        key: &GrokAcpKey,
        timeout: Duration,
    ) -> Result<Arc<GrokAcpConnection>, GrokAcpError> {
        let mut slot = self
            .acp
            .lock()
            .map_err(|_| GrokAcpError::Transport("connection slot poisoned".to_string()))?;
        if let Some(current) = slot.as_ref()
            && current.key == *key
            && !current.connection.poisoned.load(Ordering::Acquire)
        {
            return Ok(Arc::clone(&current.connection));
        }
        let startup_timeout = timeout.min(Duration::from_secs(GROK_ACP_STARTUP_TIMEOUT_SECS));
        let connection = GrokAcpConnection::spawn(key, startup_timeout)?;
        *slot = Some(GrokAcpSlot {
            key: key.clone(),
            connection: Arc::clone(&connection),
        });
        Ok(connection)
    }

    fn invalidate_acp_connection(&self, failed: &Arc<GrokAcpConnection>) {
        if let Ok(mut slot) = self.acp.lock()
            && slot
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(&current.connection, failed))
        {
            slot.take();
        }
    }

    fn chat_prompt(messages: &[ChatMsg]) -> String {
        // Prefer full conversation context: the default Club::chat path only
        // kept the latest user turn, so multi-turn Grok sessions looked like
        // amnesia. Keep the transcript compact and role-labeled. Include
        // structured tool calls / tool results so multi-hop harness turns do
        // not lose the tool transcript (content-only rows drop empty
        // assistant-with-calls messages).
        let mut parts = Vec::new();
        for msg in messages {
            let role = match msg.role {
                ChatRole::System => "System",
                ChatRole::User | ChatRole::Harness => "User",
                ChatRole::Assistant => "Assistant",
                ChatRole::Tool => "Tool",
            };
            let mut body = msg.content.trim().to_string();
            if !msg.tool_calls.is_empty() {
                for call in msg.tool_calls.iter() {
                    let args = match &call.args {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    body.push_str(&format!(
                        "<tool_call name=\"{}\">{}</tool_call>",
                        call.name, args
                    ));
                }
            }
            if let Some(id) = msg.tool_call_id.as_deref() {
                body = if body.is_empty() {
                    format!("[tool_result id={id}]")
                } else {
                    format!("[tool_result id={id}]\n{body}")
                };
            }
            if body.is_empty() {
                continue;
            }
            parts.push(format!("{role}: {body}"));
        }
        if parts.is_empty() {
            String::new()
        } else {
            parts.join("\n\n")
        }
    }

    /// Run one harness hop: pure Grok completion, then recover prose tool calls
    /// when the Angel harness offered tools.
    fn chat_on_surface(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        effort: Option<&str>,
        on_delta: Option<&mut dyn FnMut(StreamDelta)>,
    ) -> Result<ClubReply, String> {
        let transcript = Self::chat_prompt(messages);
        // Research/text seats keep Grok's own tools. Host-tool driver hops
        // belong on the HTTP OAuth seat: live Grok 4.6 refuses the markup
        // contract and narrates "I can't use that tool-call format".
        // `ANGEL_GROK_HARNESS_SURFACE=1` is an explicit experiment only.
        if !tools.is_empty() && !env_truthy(GROK_HARNESS_SURFACE_ENV, false) {
            return Err("Grok research ACP cannot drive Angel host tools. \
                 Use the grok HTTP OAuth seat (`ANGEL_DRIVER=grok`); \
                 it speaks native function calling to api.x.ai."
                .to_string());
        }
        let surface = if tools.is_empty() || !env_truthy(GROK_HARNESS_SURFACE_ENV, false) {
            GrokAcpSurface::Research
        } else {
            GrokAcpSurface::Harness
        };
        let prompt = if surface == GrokAcpSurface::Harness {
            harness_prompt_with_tool_contract(&transcript, tools)
        } else {
            transcript
        };
        let text = match &self.mode {
            GrokMode::AcpOAuth {
                command,
                model,
                timeout,
            } => {
                let resolved = self.resolved_effort(effort);
                self.respond_acp_surface(
                    command,
                    model.as_deref(),
                    *timeout,
                    &prompt,
                    cancel,
                    resolved.as_deref(),
                    surface,
                )?
            }
        };
        // Only recover prose markup on the harness surface. On the research
        // surface Grok already ran the work itself, so treating its narration
        // as pending calls would execute the same side effects twice.
        if surface == GrokAcpSurface::Harness {
            let recovered = filter_offered_tool_calls(extract_prose_tool_calls(&text), tools);
            if !recovered.is_empty() {
                // Do not stream markup that is about to become structured calls
                // — matches the OpenAI recovery path's hygiene.
                return Ok(ClubReply::Calls(recovered));
            }
        }
        if !text.is_empty()
            && let Some(on_delta) = on_delta
        {
            on_delta(StreamDelta::Content(&text));
        }
        Ok(ClubReply::Text(text))
    }

    #[cfg(test)]
    fn mode_kind(&self) -> &'static str {
        match self.mode {
            GrokMode::AcpOAuth { .. } => "acp-oauth",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GrokAcpKey {
    command: String,
    model: Option<String>,
    effort: Option<String>,
    surface: GrokAcpSurface,
}

struct GrokAcpSlot {
    key: GrokAcpKey,
    connection: Arc<GrokAcpConnection>,
}

struct GrokAcpConnection {
    child: Mutex<Option<Child>>,
    writer: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, PendingAcpRequest>>>,
    sessions: GrokAcpSessions,
    stderr_tail: Arc<Mutex<String>>,
    next_id: AtomicU64,
    poisoned: Arc<AtomicBool>,
    stdout_thread: Mutex<Option<JoinHandle<()>>>,
    stderr_thread: Mutex<Option<JoinHandle<()>>>,
}

struct PendingAcpRequest {
    method: String,
    reply: mpsc::Sender<Result<serde_json::Value, GrokAcpError>>,
}

struct GrokAcpReply {
    text: String,
    usage: Option<super::usage::ReportedUsage>,
}

#[derive(Clone, Debug)]
enum GrokAcpError {
    Cancelled,
    TimedOut { operation: String, seconds: u64 },
    Transport(String),
    Protocol(String),
    Auth(String),
}

impl GrokAcpError {
    fn restartable(&self) -> bool {
        matches!(self, Self::Transport(_))
    }

    fn operator_message(&self) -> String {
        match self {
            Self::Cancelled => "grok ACP prompt cancelled".to_string(),
            Self::TimedOut { operation, seconds } => {
                format!("grok ACP {operation} timed out after {seconds}s")
            }
            Self::Transport(detail) => format!("grok ACP transport failed: {detail}"),
            Self::Protocol(detail) => format!("grok ACP protocol failed: {detail}"),
            Self::Auth(detail) => {
                format!("grok ACP authentication failed: {detail} — {GROK_LOGIN_HINT}")
            }
        }
    }

    fn from_rpc(method: &str, error: &serde_json::Value) -> Self {
        let detail = error
            .get("message")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| error.to_string());
        if method == "authenticate" || looks_like_grok_auth_failure(&detail) {
            Self::Auth(detail)
        } else {
            Self::Protocol(format!("{method}: {detail}"))
        }
    }

    fn is_soft_turn_stop(&self) -> bool {
        matches!(self, Self::Protocol(detail) if is_soft_grok_agent_failure(detail))
    }
}

impl GrokAcpConnection {
    fn spawn(key: &GrokAcpKey, startup_timeout: Duration) -> Result<Arc<Self>, GrokAcpError> {
        let mut cmd = build_grok_acp_command(key);
        let mut child = cmd
            .spawn_owned()
            .map_err(|err| GrokAcpError::Transport(format!("spawn `{}`: {err}", key.command)))?;
        let stdin = child.stdin.take().ok_or_else(|| {
            kill_grok_child_tree(&mut child);
            GrokAcpError::Transport("ACP stdin pipe unavailable".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            kill_grok_child_tree(&mut child);
            GrokAcpError::Transport("ACP stdout pipe unavailable".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            kill_grok_child_tree(&mut child);
            GrokAcpError::Transport("ACP stderr pipe unavailable".to_string())
        })?;

        let connection = Arc::new(Self {
            child: Mutex::new(Some(child)),
            writer: Arc::new(Mutex::new(stdin)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            stderr_tail: Arc::new(Mutex::new(String::new())),
            next_id: AtomicU64::new(1),
            poisoned: Arc::new(AtomicBool::new(false)),
            stdout_thread: Mutex::new(None),
            stderr_thread: Mutex::new(None),
        });

        let stdout_thread = spawn_grok_acp_stdout_reader(
            stdout,
            Arc::clone(&connection.writer),
            Arc::clone(&connection.pending),
            Arc::clone(&connection.sessions),
            Arc::clone(&connection.stderr_tail),
            Arc::clone(&connection.poisoned),
        );
        let stderr_thread =
            spawn_grok_acp_stderr_reader(stderr, Arc::clone(&connection.stderr_tail));
        *connection
            .stdout_thread
            .lock()
            .expect("fresh ACP stdout lock") = Some(stdout_thread);
        *connection
            .stderr_thread
            .lock()
            .expect("fresh ACP stderr lock") = Some(stderr_thread);

        let startup_timeout = startup_timeout.max(Duration::from_secs(1));
        let never_cancel = AtomicBool::new(false);
        let init = connection.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": 1,
                "clientCapabilities": {
                    "fs": {"readTextFile": false, "writeTextFile": false},
                    "terminal": false
                },
                "clientInfo": {
                    "name": "angel0-cockpit",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
            startup_timeout,
            &never_cancel,
        )?;
        let cached_token = init
            .get("authMethods")
            .and_then(|methods| methods.as_array())
            .is_some_and(|methods| {
                methods.iter().any(|method| {
                    method.get("id").and_then(|id| id.as_str()) == Some("cached_token")
                })
            });
        if !cached_token {
            connection.poisoned.store(true, Ordering::Release);
            return Err(GrokAcpError::Auth(
                "the Grok agent did not offer the cached_token account-login method".to_string(),
            ));
        }
        connection.request(
            "authenticate",
            serde_json::json!({"methodId": "cached_token", "_meta": {"headless": true}}),
            startup_timeout,
            &never_cancel,
        )?;
        Ok(connection)
    }

    fn prompt(
        &self,
        prompt: &str,
        timeout: Duration,
        cancel: &AtomicBool,
        accounting: &super::AccountingCell,
    ) -> Result<GrokAcpReply, GrokAcpError> {
        let cwd = std::env::var("ANGEL_WORKSPACE")
            .ok()
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let session = self.request(
            "session/new",
            serde_json::json!({"cwd": cwd.to_string_lossy(), "mcpServers": []}),
            timeout,
            cancel,
        )?;
        let session_id = session
            .get("sessionId")
            .and_then(|id| id.as_str())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| GrokAcpError::Protocol("session/new returned no sessionId".to_string()))?
            .to_string();
        let capture = Arc::new(Mutex::new(String::new()));
        self.sessions
            .lock()
            .map_err(|_| GrokAcpError::Transport("session map poisoned".to_string()))?
            .insert(session_id.clone(), Arc::clone(&capture));

        let mut accounting = accounting.attempt();
        let result = self.request(
            "session/prompt",
            serde_json::json!({
                "sessionId": session_id,
                "prompt": [{"type": "text", "text": prompt}]
            }),
            timeout,
            cancel,
        );
        accounting.observe(
            result
                .as_ref()
                .ok()
                .and_then(super::usage::parse_grok_acp_usage)
                .map(|usage| super::UsageObservation {
                    raw: [
                        usage.input,
                        usage.output,
                        usage.reasoning,
                        usage.cache_read,
                        usage.cache_write,
                    ],
                    paths: usage.paths,
                    contract: super::UsageContract::default(),
                }),
        );
        self.sessions
            .lock()
            .ok()
            .and_then(|mut sessions| sessions.remove(&session_id));
        let text = capture
            .lock()
            .map_err(|_| GrokAcpError::Transport("reply capture poisoned".to_string()))?
            .trim()
            .to_string();

        let result = match result {
            Ok(result) => result,
            Err(err) if err.is_soft_turn_stop() && !text.is_empty() => serde_json::json!({}),
            Err(err) => {
                if matches!(err, GrokAcpError::Cancelled | GrokAcpError::TimedOut { .. }) {
                    let _ = self.notify(
                        "session/cancel",
                        serde_json::json!({"sessionId": session_id}),
                    );
                    self.poisoned.store(true, Ordering::Release);
                }
                return Err(err);
            }
        };
        if text.is_empty() {
            let stop = result
                .get("stopReason")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            return Err(GrokAcpError::Protocol(format!(
                "session/prompt returned no assistant text (stopReason={stop})"
            )));
        }
        let usage = super::usage::parse_grok_acp_usage(&result);
        Ok(GrokAcpReply { text, usage })
    }

    fn request(
        &self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<serde_json::Value, GrokAcpError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(GrokAcpError::Transport(
                self.transport_detail("connection closed"),
            ));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| GrokAcpError::Transport("pending-request map poisoned".to_string()))?
            .insert(
                id,
                PendingAcpRequest {
                    method: method.to_string(),
                    reply: tx,
                },
            );
        if let Err(err) = write_grok_acp_message(
            &self.writer,
            &serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        ) {
            self.pending
                .lock()
                .ok()
                .and_then(|mut pending| pending.remove(&id));
            self.poisoned.store(true, Ordering::Release);
            return Err(GrokAcpError::Transport(self.transport_detail(&err)));
        }

        let started = Instant::now();
        loop {
            if cancel.load(Ordering::Relaxed) {
                self.pending
                    .lock()
                    .ok()
                    .and_then(|mut pending| pending.remove(&id));
                return Err(GrokAcpError::Cancelled);
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                self.pending
                    .lock()
                    .ok()
                    .and_then(|mut pending| pending.remove(&id));
                return Err(GrokAcpError::TimedOut {
                    operation: method.to_string(),
                    seconds: timeout.as_secs(),
                });
            }
            let wait = (timeout - elapsed).min(GROK_ACP_CANCEL_GRANULARITY);
            match rx.recv_timeout(wait) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(GrokAcpError::Transport(
                        self.transport_detail("response channel closed"),
                    ));
                }
            }
        }
    }

    fn notify(&self, method: &str, params: serde_json::Value) -> Result<(), String> {
        write_grok_acp_message(
            &self.writer,
            &serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params}),
        )
    }

    fn transport_detail(&self, fallback: &str) -> String {
        let tail = self
            .stderr_tail
            .lock()
            .ok()
            .map(|tail| tail.trim().to_string());
        match tail.filter(|tail| !tail.is_empty()) {
            Some(tail) => format!("{fallback}; agent stderr: {tail}"),
            None => fallback.to_string(),
        }
    }
}

impl Drop for GrokAcpConnection {
    fn drop(&mut self) {
        self.poisoned.store(true, Ordering::Release);
        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.as_mut() {
                kill_grok_child_tree(child);
            }
            child.take();
        }
        if let Ok(mut handle) = self.stdout_thread.lock()
            && let Some(handle) = handle.take()
        {
            let _ = handle.join();
        }
        if let Ok(mut handle) = self.stderr_thread.lock()
            && let Some(handle) = handle.take()
        {
            let _ = handle.join();
        }
    }
}

fn build_grok_acp_command(key: &GrokAcpKey) -> Command {
    let mut cmd = Command::new(&key.command);
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .env_remove("PAGER")
        .arg("--no-auto-update")
        .arg("--oauth")
        .arg("--always-approve");
    if let Some(model) = key
        .model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cmd.arg("--model").arg(model);
    }
    match key.surface {
        GrokAcpSurface::Harness => {
            cmd.arg("--disallowed-tools")
                .arg(GROK_HARNESS_DISALLOWED_TOOLS);
            for deny in [
                "Bash", "Read", "Edit", "Write", "Grep", "WebFetch", "MCPTool",
            ] {
                cmd.arg("--deny").arg(deny);
            }
            cmd.arg("--max-turns")
                .arg(GROK_HARNESS_MAX_TURNS.to_string())
                .arg("--system-prompt-override")
                .arg(GROK_HARNESS_SYSTEM_OVERRIDE)
                .arg("--no-subagents")
                .arg("--no-plan");
        }
        GrokAcpSurface::Research => {
            let max_turns = env_usize("ANGEL_GROK_MAX_TURNS", GROK_DEFAULT_RESEARCH_MAX_TURNS);
            if max_turns > 0 {
                cmd.arg("--max-turns").arg(max_turns.to_string());
            }
        }
    }
    if let Ok(cwd) = std::env::var("ANGEL_WORKSPACE")
        && !cwd.trim().is_empty()
    {
        cmd.arg("--cwd").arg(cwd);
    }
    // Keep the ACP subprocess on its explicit OAuth path. The separately
    // constructed `grok-api` HTTP seat owns API-key authentication.
    scrub_grok_api_env_from_oauth_child(&mut cmd);
    // `--reasoning-effort` is a `grok agent` flag. Placing it before `agent`
    // is silently ignored, and Grok 4.6 then boots at catalog `xhigh`.
    cmd.arg("agent");
    if let Some(effort) = key
        .effort
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cmd.arg("--reasoning-effort").arg(effort);
    }
    cmd.arg("stdio");
    cmd
}

fn scrub_grok_api_env_from_oauth_child(cmd: &mut Command) {
    for key in [
        "XAI_API_KEY",
        "GROK_API_KEY",
        "ANGEL_GROK_KEY",
        "ANGEL_XAI_KEY",
        "GPU_COMP_GROK_KEY",
        "XAI_BASE_URL",
        "ANGEL_GROK_URL",
        "GROK_API_URL",
        "XAI_API_URL",
        "ANGEL_GROK_BASE_URL",
        "GPU_COMP_GROK_BASE_URL",
    ] {
        cmd.env_remove(key);
    }
}

fn write_grok_acp_message(
    writer: &Arc<Mutex<ChildStdin>>,
    message: &serde_json::Value,
) -> Result<(), String> {
    let mut writer = writer
        .lock()
        .map_err(|_| "ACP stdin lock poisoned".to_string())?;
    serde_json::to_writer(&mut *writer, message).map_err(|err| format!("encode request: {err}"))?;
    writer
        .write_all(b"\n")
        .map_err(|err| format!("write request: {err}"))?;
    writer
        .flush()
        .map_err(|err| format!("flush request: {err}"))
}

fn spawn_grok_acp_stdout_reader<R>(
    stdout: R,
    writer: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, PendingAcpRequest>>>,
    sessions: GrokAcpSessions,
    stderr_tail: Arc<Mutex<String>>,
    poisoned: Arc<AtomicBool>,
) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(err) => {
                    fail_grok_acp_pending(
                        &pending,
                        GrokAcpError::Transport(format!("read ACP stdout: {err}")),
                    );
                    poisoned.store(true, Ordering::Release);
                    return;
                }
            }
            let message: serde_json::Value = match serde_json::from_str(line.trim()) {
                Ok(message) => message,
                Err(err) => {
                    fail_grok_acp_pending(
                        &pending,
                        GrokAcpError::Protocol(format!("invalid JSON from agent: {err}")),
                    );
                    poisoned.store(true, Ordering::Release);
                    return;
                }
            };
            if message.get("method").and_then(|method| method.as_str()) == Some("session/update") {
                capture_grok_acp_update(&message, &sessions);
                continue;
            }
            if let (Some(id), Some(method)) = (
                message.get("id"),
                message.get("method").and_then(|method| method.as_str()),
            ) {
                if let Err(err) = answer_grok_acp_server_request(&writer, id, method, &message) {
                    fail_grok_acp_pending(&pending, GrokAcpError::Transport(err));
                    poisoned.store(true, Ordering::Release);
                    return;
                }
                continue;
            }
            let Some(id) = message.get("id").and_then(|id| id.as_u64()) else {
                continue;
            };
            let request = pending
                .lock()
                .ok()
                .and_then(|mut pending| pending.remove(&id));
            let Some(request) = request else {
                continue;
            };
            let result = if let Some(error) = message.get("error") {
                Err(GrokAcpError::from_rpc(&request.method, error))
            } else {
                Ok(message
                    .get("result")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})))
            };
            let _ = request.reply.send(result);
        }
        poisoned.store(true, Ordering::Release);
        let detail = stderr_tail
            .lock()
            .ok()
            .map(|tail| tail.trim().to_string())
            .filter(|tail| !tail.is_empty())
            .map(|tail| format!("agent exited; stderr: {tail}"))
            .unwrap_or_else(|| "agent exited".to_string());
        fail_grok_acp_pending(&pending, GrokAcpError::Transport(detail));
    })
}

fn capture_grok_acp_update(message: &serde_json::Value, sessions: &GrokAcpSessions) {
    let params = message.get("params").unwrap_or(&serde_json::Value::Null);
    let Some(session_id) = params.get("sessionId").and_then(|id| id.as_str()) else {
        return;
    };
    let update = params.get("update").unwrap_or(&serde_json::Value::Null);
    if update.get("sessionUpdate").and_then(|kind| kind.as_str()) != Some("agent_message_chunk") {
        return;
    }
    let Some(text) = update
        .pointer("/content/text")
        .and_then(|text| text.as_str())
    else {
        return;
    };
    let capture = sessions
        .lock()
        .ok()
        .and_then(|sessions| sessions.get(session_id).cloned());
    if let Some(capture) = capture
        && let Ok(mut out) = capture.lock()
    {
        out.push_str(text);
    }
}

fn answer_grok_acp_server_request(
    writer: &Arc<Mutex<ChildStdin>>,
    id: &serde_json::Value,
    method: &str,
    message: &serde_json::Value,
) -> Result<(), String> {
    let response = if method == "session/request_permission" {
        let option_id = message
            .pointer("/params/options")
            .and_then(|options| options.as_array())
            .and_then(|options| {
                options.iter().find(|option| {
                    option
                        .get("kind")
                        .and_then(|kind| kind.as_str())
                        .is_some_and(|kind| kind.to_ascii_lowercase().contains("allow"))
                })
            })
            .and_then(|option| option.get("optionId"))
            .cloned();
        match option_id {
            Some(option_id) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"outcome": {"outcome": "selected", "optionId": option_id}}
            }),
            None => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"outcome": {"outcome": "cancelled"}}
            }),
        }
    } else {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": format!("unsupported angel0 ACP client method: {method}")}
        })
    };
    write_grok_acp_message(writer, &response)
}

fn fail_grok_acp_pending(
    pending: &Arc<Mutex<HashMap<u64, PendingAcpRequest>>>,
    error: GrokAcpError,
) {
    let requests = pending
        .lock()
        .ok()
        .map(|mut pending| {
            pending
                .drain()
                .map(|(_, request)| request)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for request in requests {
        let _ = request.reply.send(Err(error.clone()));
    }
}

fn spawn_grok_acp_stderr_reader<R>(stderr: R, tail: Arc<Mutex<String>>) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut stderr = stderr;
        let mut chunk = [0_u8; 2048];
        while let Ok(n) = stderr.read(&mut chunk) {
            if n == 0 {
                break;
            }
            if let Ok(mut tail) = tail.lock() {
                tail.push_str(&String::from_utf8_lossy(&chunk[..n]));
                if tail.len() > 8192 {
                    let drop_bytes = tail.len() - 8192;
                    let boundary = tail
                        .char_indices()
                        .map(|(index, _)| index)
                        .find(|index| *index >= drop_bytes)
                        .unwrap_or(drop_bytes);
                    tail.drain(..boundary);
                }
            }
        }
    })
}

fn is_soft_grok_agent_failure(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.contains("max turns")
        || lower.contains("max-turns")
        || lower.contains("maximum number of turns")
}

/// Host-tool contract + transcript for a harness hop. Catalog lives in the
/// prompt body (not argv) so multi-tool schemas never hit ARG_MAX.
fn harness_prompt_with_tool_contract(transcript: &str, tools: &[ToolDef]) -> String {
    let mut out = String::with_capacity(transcript.len() + tools.len() * 128 + 512);
    out.push_str("## Angel host tools\n");
    out.push_str(
        "You cannot run tools yourself. To request work, emit one or more of:\n\
         <tool_call name=\"TOOL_NAME\">{json arguments object}</tool_call>\n\
         Then stop. The host will execute them and return `[tool_result …]` rows.\n\n",
    );
    if tools.is_empty() {
        out.push_str("(no host tools offered this hop — answer in plain text only)\n");
    } else {
        out.push_str("Available host tools:\n");
        for tool in tools {
            let desc = tool.description.trim();
            let params = compact_tool_params_hint(&tool.params);
            if desc.is_empty() {
                out.push_str(&format!("- `{}` params: {}\n", tool.name, params));
            } else {
                // Keep each tool line bounded so the contract stays scannable.
                let desc = if desc.len() > 160 {
                    format!("{}…", &desc[..160])
                } else {
                    desc.to_string()
                };
                out.push_str(&format!(
                    "- `{}` — {} | params: {}\n",
                    tool.name, desc, params
                ));
            }
        }
    }
    out.push_str(
        "\nExample:\n\
         <tool_call name=\"shell\">{\"command\":\"pwd\"}</tool_call>\n\
         \n## Conversation\n",
    );
    out.push_str(transcript);
    out
}

/// Compact JSON Schema-ish params for the harness catalog line.
fn compact_tool_params_hint(params: &serde_json::Value) -> String {
    // Prefer property names when present — models need keys more than prose.
    if let Some(props) = params
        .get("properties")
        .and_then(|p| p.as_object())
        .filter(|p| !p.is_empty())
    {
        let keys: Vec<&str> = props.keys().map(|k| k.as_str()).collect();
        return format!("{{{}}}", keys.join(", "));
    }
    let raw = params.to_string();
    if raw.len() <= 120 {
        raw
    } else {
        format!("{}…", &raw[..120])
    }
}

/// Keep only calls for tools Angel actually offered this hop.
fn filter_offered_tool_calls(calls: Vec<ToolCall>, tools: &[ToolDef]) -> Vec<ToolCall> {
    if tools.is_empty() || calls.is_empty() {
        return calls;
    }
    calls
        .into_iter()
        .filter(|call| {
            tools
                .iter()
                .any(|tool| tool.name.eq_ignore_ascii_case(&call.name))
        })
        .collect()
}

struct GrokCatalog {
    levels: Vec<String>,
    descriptions: Vec<(String, String)>,
    default_effort: Option<String>,
    context_window: Option<u64>,
}

fn grok_model_catalog(model: Option<&str>) -> GrokCatalog {
    // The CLI cache can lag a same-day API release. For the exact stable 4.6
    // id, prefer xAI's published contract so the cockpit does not silently hide
    // xhigh or budget a 500k route as unknown.
    if model.is_some_and(|model| model.eq_ignore_ascii_case("grok-4.6")) {
        return GrokCatalog {
            levels: GROK_46_EFFORT_LEVELS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            descriptions: vec![
                ("low".into(), "Fast reasoning".into()),
                ("medium".into(), "Balanced reasoning".into()),
                ("high".into(), "Default deep reasoning".into()),
                ("xhigh".into(), "Maximum Grok 4.6 reasoning effort".into()),
            ],
            default_effort: Some("high".to_string()),
            context_window: Some(GROK_46_CONTEXT_WINDOW),
        };
    }
    if let Some(catalog) = read_grok_model_catalog(model) {
        return catalog;
    }
    GrokCatalog {
        levels: GROK_DEFAULT_EFFORT_LEVELS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        descriptions: vec![
            ("low".into(), "Quick, fast implementations".into()),
            (
                "medium".into(),
                "Balanced effort with standard implementation and testing".into(),
            ),
            (
                "high".into(),
                "Highest implementation quality with extensive reasoning".into(),
            ),
        ],
        default_effort: Some("medium".to_string()),
        context_window: None,
    }
}

fn read_grok_model_catalog(model: Option<&str>) -> Option<GrokCatalog> {
    let path = grok_models_cache_file()?;
    let raw = grok_read("models-cache-read", path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let models = v.get("models")?.as_object()?;
    let entry = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|want| {
            models
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(want))
                .map(|(_, value)| value)
        })
        .or_else(|| models.values().next())?;
    let info = entry.get("info").unwrap_or(entry);
    let mut descriptions = Vec::new();
    let mut levels = Vec::new();
    if let Some(list) = info.get("reasoning_efforts").and_then(|x| x.as_array()) {
        for item in list {
            let id = item
                .get("value")
                .or_else(|| item.get("id"))
                .and_then(|x| x.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let Some(id) = id else {
                continue;
            };
            let id_l = id.to_ascii_lowercase();
            if levels.iter().any(|seen: &String| seen == &id_l) {
                continue;
            }
            let desc = item
                .get("description")
                .or_else(|| item.get("label"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            levels.push(id_l.clone());
            if !desc.is_empty() {
                descriptions.push((id_l, desc));
            }
        }
    }
    if levels.is_empty() {
        levels = GROK_DEFAULT_EFFORT_LEVELS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    }
    let default_effort = info
        .get("reasoning_efforts")
        .and_then(|x| x.as_array())
        .and_then(|list| {
            list.iter()
                .find(|item| item.get("default").and_then(|d| d.as_bool()) == Some(true))
                .and_then(|item| {
                    item.get("value")
                        .or_else(|| item.get("id"))
                        .and_then(|x| x.as_str())
                })
        })
        .or_else(|| info.get("reasoning_effort").and_then(|x| x.as_str()))
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .and_then(|want| {
            levels
                .iter()
                .find(|level| level.eq_ignore_ascii_case(&want))
                .cloned()
        })
        .or_else(|| levels.iter().find(|l| *l == "medium").cloned())
        .or_else(|| levels.first().cloned());
    let context_window = info
        .get("context_window")
        .and_then(|x| x.as_u64())
        .filter(|n| *n > 0);
    Some(GrokCatalog {
        levels,
        descriptions,
        default_effort,
        context_window,
    })
}

impl Club for GrokResearchClub {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        self.respond_cancellable(prompt, &AtomicBool::new(false))
    }

    fn respond_cancellable(&self, prompt: &str, cancel: &AtomicBool) -> Result<String, String> {
        match &self.mode {
            GrokMode::AcpOAuth {
                command,
                model,
                timeout,
            } => {
                let effort = self.resolved_effort(None);
                self.respond_acp(
                    command,
                    model.as_deref(),
                    *timeout,
                    prompt,
                    cancel,
                    effort.as_deref(),
                )
            }
        }
    }

    fn label(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        match &self.mode {
            GrokMode::AcpOAuth { command, .. } => Path::new(command).is_file() || command == "grok",
        }
    }

    fn live_model_name(&self) -> Option<String> {
        match &self.mode {
            GrokMode::AcpOAuth { model, .. } => model.clone(),
        }
    }

    fn usage_accounting(&self) -> super::AccountingView {
        self.accounting.view()
    }

    fn token_usage(&self) -> Option<TokenUsage> {
        let stats = self.usage.load();
        (stats.turns > 0).then_some(stats)
    }

    fn reasoning_effort(&self) -> Option<String> {
        self.resolved_effort(None)
    }

    fn reasoning_levels(&self) -> &[String] {
        &self.reasoning_levels
    }

    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        let canonical = self.canonicalize_effort(requested)?;
        if let Ok(mut g) = self.effort_override.lock() {
            *g = Some(canonical.clone());
        }
        self.route_state_revision.fetch_add(1, Ordering::Relaxed);
        Some(canonical)
    }

    fn route_state_revision(&self) -> u64 {
        self.route_state_revision.load(Ordering::Relaxed)
    }

    fn route_metadata(&self) -> RouteMetadata {
        RouteMetadata {
            description: Some("Grok account OAuth (persistent ACP research agent)".to_string()),
            context_window: self.context_window,
            input_modalities: vec!["text".to_string()],
            speed_tiers: Vec::new(),
            reasoning_descriptions: self.reasoning_descriptions.clone(),
            output_budget: OutputBudgetPolicy::ProviderNative,
            output_budget_provenance: None,
        }
    }

    fn header_route_metadata(&self) -> RouteMetadata {
        RouteMetadata {
            context_window: self.context_window,
            ..RouteMetadata::default()
        }
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.chat_on_surface(messages, tools, &AtomicBool::new(false), None, None)
    }

    fn chat_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
    ) -> Result<ClubReply, String> {
        self.chat_on_surface(messages, tools, &AtomicBool::new(false), effort, None)
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.chat_streaming_with_effort(messages, tools, None, cancel, on_delta)
    }

    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        // Honor Esc mid-child: the default chat_streaming path called
        // respond() and ignored cancel for the full timeout. When `tools` is
        // non-empty this is a harness hop (pure completion); when empty it is
        // a research/text seat (bounded agent).
        self.chat_on_surface(messages, tools, cancel, effort, Some(on_delta))
    }
}

fn grok_research_enabled() -> bool {
    env_truthy("ANGEL_GROK_RESEARCH", true)
}

/// Why the `grok_research` tool would be missing on this host, if it is.
/// Feeds the capability manifest so the model reads the actual reason instead
/// of guessing "no API key" and abandoning research turns.
pub(crate) fn grok_research_unavailable_reason() -> Option<&'static str> {
    if !grok_research_enabled() {
        return Some("disabled by ANGEL_GROK_RESEARCH=0");
    }
    if !grok_oauth_available() {
        return Some("no Grok OAuth login on this host (~/.grok/auth.json missing)");
    }
    None
}

pub(crate) fn grok_research_always() -> bool {
    env_truthy("ANGEL_GROK_RESEARCH_ALWAYS", false)
}

fn grok_model_from_env() -> String {
    env_first(&["ANGEL_GROK_MODEL", "GROK_MODEL"])
        .map(|model| resolve_grok_model_alias(&model))
        .unwrap_or_else(|| GROK_DEFAULT_MODEL.to_string())
}

pub(crate) fn resolve_grok_model_alias(raw: &str) -> String {
    resolve_grok_model_alias_with_available(raw, &grok_cli_model_ids())
}

fn resolve_grok_model_alias_with_available(raw: &str, available: &[String]) -> String {
    let trimmed = raw.trim();
    let normalized = trimmed.to_ascii_lowercase().replace(['_', ' '], "-");
    if GROK_SOTA_MODEL_OPTIONS
        .iter()
        .any(|model| model.eq_ignore_ascii_case(trimmed))
        && !matches!(normalized.as_str(), "grok-4" | "grok-4-latest")
    {
        return trimmed.to_ascii_lowercase();
    }
    match normalized.as_str() {
        // `grok` is an angel0 compatibility alias, not a documented xAI API id.
        // Always collapse it locally so it can never reach the wire.
        "grok" => GROK_DEFAULT_MODEL.to_string(),
        "4" | "new" | "latest" | "grok4" | "grok-4" | "4-new" | "grok4-new" | "grok-4-new"
        | "4-latest" | "grok4-latest" | "grok-4-latest" => {
            best_available_grok_4_model(available).unwrap_or_else(|| GROK_DEFAULT_MODEL.to_string())
        }
        "4.6" | "grok4.6" | "grok-4-6" | "grok-4.6" => GROK_DEFAULT_MODEL.to_string(),
        "4.5" | "grok4.5" | "grok-4-5" | "grok-4.5" => "grok-4.5".to_string(),
        "4.3" | "grok4.3" | "grok-4-3" | "grok-4.3" => "grok-4.3".to_string(),
        "4.20" | "grok4.20" | "grok-4-20" | "grok-4.20" => "grok-4.20".to_string(),
        "4.20-fast" | "grok4.20-fast" | "grok-4-20-fast" | "grok-4.20-fast" => {
            "grok-4.20-fast".to_string()
        }
        "4.20-mini" | "grok4.20-mini" | "grok-4-20-mini" | "grok-4.20-mini" => {
            "grok-4.20-mini".to_string()
        }
        _ => trimmed.to_string(),
    }
}

fn best_available_grok_4_model(available: &[String]) -> Option<String> {
    for candidate in [
        "grok-4.6",
        "grok-4.5",
        "grok-4.20",
        "grok-4.20-fast",
        "grok-4.20-mini",
        "grok-4.3",
        "grok-4",
    ] {
        if available.iter().any(|model| model == candidate) {
            return Some(candidate.to_string());
        }
    }
    available
        .iter()
        .filter(|model| model.starts_with("grok-4"))
        .max()
        .cloned()
}

fn grok_cli_model_ids() -> Vec<String> {
    let Some(path) = grok_models_cache_file() else {
        return Vec::new();
    };
    let Ok(raw) = grok_read("models-cache-read", path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    let Some(models) = v.get("models").and_then(|x| x.as_object()) else {
        return Vec::new();
    };
    models.keys().map(|key| key.to_ascii_lowercase()).collect()
}

fn grok_models_cache_file() -> Option<PathBuf> {
    if let Some(path) = env_first(&["ANGEL_GROK_MODELS_CACHE", "GROK_MODELS_CACHE"]) {
        return Some(expand_home(&path));
    }
    let home = env_first(&["ANGEL_GROK_HOME", "GROK_HOME"])
        .map(|p| expand_home(&p))
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".grok"))
        })?;
    Some(home.join("models_cache.json"))
}

fn env_truthy(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        })
        .unwrap_or(default)
}

fn grok_oauth_available() -> bool {
    grok_oauth_file().is_some_and(|path| {
        grok_step("oauth-file-stat", Duration::from_secs(5), move || {
            path.is_file()
        })
        .unwrap_or(false)
    })
}

fn grok_oauth_file() -> Option<PathBuf> {
    if let Some(path) = env_first(&["ANGEL_GROK_OAUTH_FILE", "GROK_AUTH_FILE"]) {
        return Some(expand_home(&path));
    }
    let home = env_first(&["ANGEL_GROK_HOME", "GROK_HOME"])
        .map(|p| expand_home(&p))
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".grok"))
        })?;
    Some(home.join("auth.json"))
}

fn grok_command() -> String {
    if let Some(cmd) = env_first(&["ANGEL_GROK_CMD", "GROK_CMD"]) {
        return cmd;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let local = PathBuf::from(home).join(".local/bin/grok");
    if local.is_file() {
        return local.to_string_lossy().to_string();
    }
    "grok".to_string()
}

fn expand_home(path: &str) -> PathBuf {
    let Some(rest) = path.strip_prefix("~/") else {
        return PathBuf::from(path);
    };
    std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(rest))
        .unwrap_or_else(|_| PathBuf::from(path))
}

/// Kill the persistent Grok ACP child and any tool subprocesses it spawned. Children are
/// put in their own process group at spawn time; fall back to a direct kill
/// when process-group signaling is unavailable.
fn kill_grok_child_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // Same pattern as harness/exec.rs: process_group(0) at spawn, killpg
        // on teardown so shell tools cannot orphan after Esc/timeout.
        let pid = child.id() as libc::pid_t;
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// One credential record in `~/.grok/auth.json` — the object that carries an
/// `expires_at`, plus the `refresh_token` stored beside it.
struct GrokCredential<'a> {
    /// Access-token expiry, when it parses. `None` means "can't tell", which is
    /// never treated as a reason to block a call.
    expires_at: Option<u64>,
    refresh_token: Option<&'a str>,
}

impl GrokCredential<'_> {
    /// True only when this record cannot serve a request without an operator
    /// re-login: the access token is spent *and* there is no refresh token to
    /// silently exchange for a new one.
    fn needs_login(&self, deadline: u64) -> bool {
        self.expires_at
            .is_some_and(|expiry| expiry <= deadline && self.refresh_token.is_none())
    }
}

/// Whether the Grok CLI would have to stop and ask the operator to log in.
///
/// An expired access token is *not* that state. The CLI exchanges the stored
/// `refresh_token` on the next call and rewrites `~/.grok/auth.json` itself, so
/// gating on the access-token clock alone hard-failed every Grok turn — the MoA
/// research scout most visibly — for the whole stretch between token expiry and
/// the next manual login, while the CLI would have served those turns fine.
fn grok_oauth_needs_login() -> bool {
    let Some(path) = grok_oauth_file() else {
        return false;
    };
    let Ok(raw) = grok_read("oauth-file-read", path.clone()) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    let mut credentials = Vec::new();
    collect_grok_credentials(&v, &mut credentials);
    if credentials.is_empty() {
        return false;
    }
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };
    let deadline = now.as_secs().saturating_add(300);
    // Any still-usable record is enough — the file can hold several accounts.
    credentials.iter().all(|cred| cred.needs_login(deadline))
}

fn collect_grok_credentials<'a>(v: &'a serde_json::Value, out: &mut Vec<GrokCredential<'a>>) {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(expires_at) = map.get("expires_at").and_then(|x| x.as_str()) {
                out.push(GrokCredential {
                    expires_at: parse_rfc3339_seconds(expires_at),
                    refresh_token: map
                        .get("refresh_token")
                        .and_then(|x| x.as_str())
                        .map(str::trim)
                        .filter(|token| !token.is_empty()),
                });
                return;
            }
            for value in map.values() {
                collect_grok_credentials(value, out);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_grok_credentials(value, out);
            }
        }
        _ => {}
    }
}

/// Does the CLI's own stderr read like a credential problem rather than a
/// model/network one? Keeps the login hint off unrelated failures.
fn looks_like_grok_auth_failure(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    ["401", "unauthorized", "unauthenticated", "invalid_grant"]
        .iter()
        .any(|needle| lower.contains(needle))
        || (lower.contains("token") && (lower.contains("expired") || lower.contains("invalid")))
        || lower.contains("log in")
        || lower.contains("login")
}

fn parse_rfc3339_seconds(s: &str) -> Option<u64> {
    let stamp = s.get(..19)?;
    let year: i32 = stamp.get(0..4)?.parse().ok()?;
    let month: u32 = stamp.get(5..7)?.parse().ok()?;
    let day: u32 = stamp.get(8..10)?.parse().ok()?;
    let hour: u64 = stamp.get(11..13)?.parse().ok()?;
    let min: u64 = stamp.get(14..16)?.parse().ok()?;
    let sec: u64 = stamp.get(17..19)?.parse().ok()?;
    if stamp.as_bytes().get(4) != Some(&b'-')
        || stamp.as_bytes().get(7) != Some(&b'-')
        || stamp.as_bytes().get(10) != Some(&b'T')
        || stamp.as_bytes().get(13) != Some(&b':')
        || stamp.as_bytes().get(16) != Some(&b':')
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || min > 59
        || sec > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(days.saturating_mul(86_400) + hour * 3600 + min * 60 + sec)
}

fn days_from_civil(mut year: i32, month: u32, day: u32) -> Option<u64> {
    year -= (month <= 2) as i32;
    let era = (year as i64).div_euclid(400);
    let yoe = year as i64 - era * 400;
    let mp = month as i64 + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    (days >= 0).then_some(days as u64)
}

// ---------------------------------------------------------------------------
// First-class Grok OAuth — load / refresh / persist (mirrors openai_codex)
// ---------------------------------------------------------------------------

const GROK_DEFAULT_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const GROK_DEFAULT_API_BASE: &str = "https://api.x.ai/v1";
/// Refresh this many seconds before access-token expiry.
const GROK_OAUTH_REFRESH_SLACK_SECS: u64 = 120;

/// On-disk Grok CLI OAuth credential (`~/.grok/auth.json` entry).
#[derive(Clone)]
struct GrokOauthAuth {
    path: PathBuf,
    /// Map key in auth.json, e.g. `https://auth.x.ai::b1a00492-…`.
    entry_key: String,
    access_token: String,
    refresh_token: String,
    client_id: String,
    token_url: String,
    /// Unix seconds when the access token expires, when known.
    expires_at: Option<u64>,
}

pub(crate) struct GrokOauthShared {
    auth: Mutex<GrokOauthAuth>,
    /// Single-flight refresh gate (in progress flag + waiter condvar).
    refresh_gate: (Mutex<bool>, std::sync::Condvar),
    agent: ureq::Agent,
}

// Startup diagnostics never include paths, tokens, response bodies, or URLs.
fn grok_diagnostic(message: std::fmt::Arguments<'_>) {
    // Match the HTTP transport: redirected/headless stderr retains diagnostics,
    // while background OAuth discovery must not overwrite the live TUI.
    if !std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        eprintln!("{message}");
    }
}

fn grok_step<T: Send + 'static>(
    name: &'static str,
    timeout: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let started = Instant::now();
    grok_diagnostic(format_args!(
        "[grok] {name} start 0 ms; bound {} ms",
        timeout.as_millis()
    ));
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    let result = rx.recv_timeout(timeout);
    grok_diagnostic(format_args!(
        "[grok] {name} {} {} ms",
        if result.is_ok() {
            "complete"
        } else {
            "timeout/disconnected"
        },
        started.elapsed().as_millis()
    ));
    result.map_err(|_| format!("Grok {name} timed out or worker disconnected"))
}

fn grok_read(step: &'static str, path: PathBuf) -> std::io::Result<String> {
    grok_step(step, Duration::from_secs(5), move || {
        // Reject FIFOs/devices; O_NONBLOCK also closes the metadata/open race.
        use std::io::Read;
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::other("Grok state is not a regular file"));
        }
        let mut raw = String::new();
        file.take(1_048_577).read_to_string(&mut raw)?;
        if raw.len() > 1_048_576 {
            return Err(std::io::Error::other("Grok state exceeds 1 MiB"));
        }
        Ok(raw)
    })
    .map_err(std::io::Error::other)?
}

fn grok_lock<T>(lock: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    let started = Instant::now();
    loop {
        match lock.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err("Grok startup lock poisoned".into());
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                if started.elapsed() >= Duration::from_secs(60) {
                    grok_diagnostic(format_args!(
                        "[grok] startup-lock timeout {} ms",
                        started.elapsed().as_millis()
                    ));
                    return Err("Grok startup lock timed out".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

impl GrokOauthAuth {
    fn load() -> Option<Self> {
        let path = grok_oauth_file()?;
        let raw = grok_read("oauth-file-read", path.clone()).ok()?;
        let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
        Self::from_value(&v, path)
    }

    fn from_value(v: &serde_json::Value, path: PathBuf) -> Option<Self> {
        let map = v.as_object()?;
        // Prefer the well-known Grok CLI OIDC entry; else first object with key+refresh.
        let preferred = "https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828";
        let (entry_key, entry) = if let Some(e) = map.get(preferred).and_then(|x| x.as_object()) {
            (preferred.to_string(), e)
        } else {
            map.iter().find_map(|(k, val)| {
                let obj = val.as_object()?;
                let key = obj.get("key").and_then(|x| x.as_str())?.trim();
                if key.is_empty() {
                    return None;
                }
                Some((k.clone(), obj))
            })?
        };
        let access_token = entry
            .get("key")
            .and_then(|x| x.as_str())?
            .trim()
            .to_string();
        if access_token.is_empty() {
            return None;
        }
        let refresh_token = entry
            .get("refresh_token")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let client_id = entry
            .get("oidc_client_id")
            .and_then(|x| x.as_str())
            .unwrap_or("b1a00492-073a-47ea-816f-4c329264a828")
            .trim()
            .to_string();
        let issuer = entry
            .get("oidc_issuer")
            .and_then(|x| x.as_str())
            .unwrap_or("https://auth.x.ai")
            .trim()
            .trim_end_matches('/');
        let token_url = if issuer.is_empty() {
            GROK_DEFAULT_TOKEN_URL.to_string()
        } else {
            format!("{issuer}/oauth2/token")
        };
        let expires_at = entry
            .get("expires_at")
            .and_then(|x| x.as_str())
            .and_then(parse_rfc3339_seconds)
            .or_else(|| jwt_exp_secs(&access_token));
        Some(Self {
            path,
            entry_key,
            access_token,
            refresh_token,
            client_id,
            token_url,
            expires_at,
        })
    }

    fn needs_refresh(&self, now: u64) -> bool {
        match self.expires_at {
            Some(exp) => exp.saturating_sub(GROK_OAUTH_REFRESH_SLACK_SECS) <= now,
            // Unknown expiry: trust JWT if present, else refresh to be safe when
            // we have a refresh token (stale cache without expires_at).
            None => jwt_exp_secs(&self.access_token)
                .map(|exp| exp.saturating_sub(GROK_OAUTH_REFRESH_SLACK_SECS) <= now)
                .unwrap_or(!self.refresh_token.is_empty()),
        }
    }
}

impl GrokOauthShared {
    fn new(auth: GrokOauthAuth) -> Arc<Self> {
        let agent = ureq::AgentBuilder::new()
            // The refresh request carries a long-lived refresh token; never
            // replay it to a redirect target.
            .redirects(0)
            .timeout(Duration::from_secs(55))
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(15))
            .user_agent(concat!("angel0-cockpit/", env!("CARGO_PKG_VERSION")))
            .build();
        Arc::new(Self {
            auth: Mutex::new(auth),
            refresh_gate: (Mutex::new(false), std::sync::Condvar::new()),
            agent,
        })
    }

    /// Return a live access token, refreshing under a single-flight gate when needed.
    fn bearer(self: &Arc<Self>) -> Result<String, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        {
            let auth = grok_lock(&self.auth)?;
            if !auth.needs_refresh(now) {
                return Ok(auth.access_token.clone());
            }
            if auth.refresh_token.is_empty() {
                return Err(format!(
                    "Grok OAuth access token expired with no refresh token; {GROK_LOGIN_HINT}"
                ));
            }
        }

        // Single-flight: one refresher, others wait.
        let (lock, cvar) = &self.refresh_gate;
        let mut inflight = grok_lock(lock)?;
        let started = Instant::now();
        grok_diagnostic(format_args!(
            "[grok] refresh-wait start 0 ms; bound 60000 ms"
        ));
        while *inflight {
            let remaining = Duration::from_secs(60).saturating_sub(started.elapsed());
            let (guard, wait) = cvar
                .wait_timeout(inflight, remaining)
                .map_err(|_| "grok oauth refresh wait poisoned".to_string())?;
            inflight = guard;
            if wait.timed_out() && *inflight {
                grok_diagnostic(format_args!(
                    "[grok] refresh-wait timeout {} ms",
                    started.elapsed().as_millis()
                ));
                return Err("Grok OAuth refresh wait timed out".into());
            }
        }
        grok_diagnostic(format_args!(
            "[grok] refresh-wait complete {} ms",
            started.elapsed().as_millis()
        ));
        // Re-check after wait — peer may have refreshed.
        {
            let auth = grok_lock(&self.auth)?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if !auth.needs_refresh(now) {
                return Ok(auth.access_token.clone());
            }
        }
        *inflight = true;
        drop(inflight);

        let started = Instant::now();
        grok_diagnostic(format_args!(
            "[grok] oauth-refresh start 0 ms; HTTP bound 55000 ms"
        ));
        let result = self.perform_refresh();
        grok_diagnostic(format_args!(
            "[grok] oauth-refresh {} {} ms",
            if result.is_ok() { "complete" } else { "failed" },
            started.elapsed().as_millis()
        ));
        let mut inflight = grok_lock(lock)?;
        *inflight = false;
        cvar.notify_all();
        drop(inflight);
        result
    }

    fn perform_refresh(&self) -> Result<String, String> {
        let (token_url, refresh_token, client_id) = {
            let auth = grok_lock(&self.auth)?;
            (
                auth.token_url.clone(),
                auth.refresh_token.clone(),
                auth.client_id.clone(),
            )
        };
        if refresh_token.is_empty() {
            return Err(format!(
                "Grok OAuth has no refresh token; {GROK_LOGIN_HINT}"
            ));
        }
        // OIDC public client: form body, auth method "none".
        let body = format!(
            "grant_type=refresh_token&refresh_token={}&client_id={}",
            urlencoding_encode(&refresh_token),
            urlencoding_encode(&client_id)
        );
        let resp = self
            .agent
            .post(&token_url)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .set("Accept", "application/json")
            .send_string(&body)
            .map_err(|e| format!("Grok OAuth refresh failed: {e}"))?;
        let status = resp.status();
        let v: serde_json::Value = resp
            .into_json()
            .map_err(|e| format!("Grok OAuth refresh decode: {e}"))?;
        if !(200..300).contains(&status) {
            let detail = v
                .get("error_description")
                .or_else(|| v.get("error"))
                .and_then(|x| x.as_str())
                .unwrap_or("token endpoint rejected refresh");
            return Err(format!(
                "Grok OAuth refresh HTTP {status}: {detail}; {GROK_LOGIN_HINT}"
            ));
        }
        let access = v
            .get("access_token")
            .and_then(|x| x.as_str())
            .ok_or_else(|| "Grok OAuth refresh: no access_token".to_string())?
            .to_string();
        let new_refresh = v
            .get("refresh_token")
            .and_then(|x| x.as_str())
            .map(str::to_string);
        let expires_in = v
            .get("expires_in")
            .and_then(|x| x.as_u64())
            .unwrap_or(21_600);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let expires_at = now.saturating_add(expires_in);

        let mut auth = grok_lock(&self.auth)?;
        auth.access_token = access.clone();
        if let Some(rt) = new_refresh
            && !rt.is_empty()
        {
            auth.refresh_token = rt;
        }
        auth.expires_at = Some(expires_at);
        let snapshot = auth.clone();
        drop(auth);
        let _ = grok_step("oauth-persist", Duration::from_secs(5), move || {
            snapshot.persist()
        });
        Ok(access)
    }
}

impl GrokOauthAuth {
    /// Best-effort atomic write-back of rotated tokens into auth.json.
    fn persist(&self) {
        let Ok(raw) = grok_read("oauth-persist-read", self.path.clone()) else {
            return;
        };
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return;
        };
        let Some(entry) = v.get_mut(&self.entry_key).and_then(|x| x.as_object_mut()) else {
            return;
        };
        entry.insert("key".into(), self.access_token.clone().into());
        entry.insert("refresh_token".into(), self.refresh_token.clone().into());
        if let Some(exp) = self.expires_at {
            entry.insert("expires_at".into(), rfc3339_from_secs(exp).into());
        }
        let Ok(out) = serde_json::to_vec_pretty(&v) else {
            return;
        };
        let tmp = self.path.with_extension(format!(
            "json.angel-{}-{}.tmp",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        if std::fs::write(&tmp, &out).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        if std::fs::rename(&tmp, &self.path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

fn jwt_exp_secs(token: &str) -> Option<u64> {
    let payload = token.split('.').nth(1)?;
    let pad = (4 - payload.len() % 4) % 4;
    let padded = format!("{payload}{}", "=".repeat(pad));
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, payload)
        .or_else(|_| base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE, &padded))
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get("exp")?.as_u64()
}

fn rfc3339_from_secs(secs: u64) -> String {
    // Enough for expires_at write-back; Grok CLI accepts fractional but integer is fine.
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;
    // Civil date from Unix day count (Howard Hinnant algorithm inverse of days_from_civil).
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn civil_from_days(mut days: i64) -> (i32, u32, u32) {
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64 + era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Build selectable Grok HTTP models. OAuth rows use the stable `grok` alias
/// plus concrete catalog siblings. When an API key is explicitly configured,
/// `grok-api` is added as a separate route with its own endpoint/model.
///
/// This seat is independent of `ANGEL_GROK_RESEARCH`. That flag only gates the
/// ACP scout / `grok_research` tool; the HTTP driver remains available so
/// `ANGEL_DRIVER=grok` and SOTA judge/aggregate pins keep working.
pub(crate) fn grok_http_clubs() -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    let mut links = Vec::new();
    if let Some(auth) = GrokOauthAuth::load() {
        // Failure here must not suppress a separately configured API-key seat.
        let shared = GrokOauthShared::new(auth);
        if let Err(err) = shared.bearer() {
            grok_diagnostic(format_args!("[grok] OAuth seat not ready: {err}"));
        } else {
            let configured = grok_model_from_env();
            let mut models = vec![configured];
            for candidate in
                std::iter::once(GROK_DEFAULT_MODEL.to_string()).chain(grok_cli_model_ids())
            {
                if GROK_SOTA_MODEL_OPTIONS
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(&candidate))
                    && !models
                        .iter()
                        .any(|model| model.eq_ignore_ascii_case(&candidate))
                {
                    models.push(candidate);
                }
            }
            let available = Arc::new(AtomicBool::new(true));
            links.extend(models.into_iter().enumerate().map(|(index, model)| {
                let alias = if index == 0 {
                    "grok".to_string()
                } else {
                    model.clone()
                };
                let label = alias.clone();
                let shared_for_provider = Arc::clone(&shared);
                let club: Arc<dyn Club> = Arc::new(
                    crate::agent::club::HttpClub::new(label, GROK_DEFAULT_API_BASE, model, None)
                        .with_token_provider(Arc::new(move || shared_for_provider.bearer()))
                        .sota_tuned(),
                );
                (alias, club, Arc::clone(&available))
            }));
        }
    }

    if super::api_club_enabled("grok-api") {
        if let Some(key) = env_first(&[
            "ANGEL_GROK_KEY",
            "ANGEL_XAI_KEY",
            "XAI_API_KEY",
            "GROK_API_KEY",
        ])
        .filter(|key| !key.trim().is_empty())
        {
            let url = env_first(&[
                "ANGEL_GROK_API_URL",
                "ANGEL_GROK_URL",
                "ANGEL_GROK_BASE_URL",
                "GROK_API_URL",
                "XAI_BASE_URL",
            ])
            .filter(|url| !url.trim().is_empty())
            .unwrap_or_else(|| GROK_DEFAULT_API_BASE.to_string());
            let model = env_first(&["ANGEL_GROK_API_MODEL", "GROK_API_MODEL"])
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| GROK_DEFAULT_MODEL.to_string());
            let club: Arc<dyn Club> = Arc::new(
                crate::agent::club::HttpClub::new("grok-api", url, model, Some(key)).sota_tuned(),
            );
            links.push((
                "grok-api".to_string(),
                club,
                Arc::new(AtomicBool::new(true)),
            ));
        }
    }
    links
}

#[cfg(test)]
pub(crate) fn extract_grok_text(v: &serde_json::Value) -> Option<String> {
    if let Some(text) = v.get("output_text").and_then(|x| x.as_str()) {
        let text = text.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    let mut parts = Vec::new();
    if let Some(output) = v.get("output").and_then(|x| x.as_array()) {
        for item in output {
            if item.get("type").and_then(|x| x.as_str()) != Some("message") {
                continue;
            }
            match item.get("content") {
                Some(serde_json::Value::String(s)) if !s.trim().is_empty() => {
                    parts.push(s.trim().to_string());
                }
                Some(serde_json::Value::Array(content)) => {
                    for part in content {
                        let kind = part.get("type").and_then(|x| x.as_str()).unwrap_or("");
                        if !kind.contains("text") {
                            continue;
                        }
                        if let Some(text) = part.get("text").and_then(|x| x.as_str()) {
                            let text = text.trim();
                            if !text.is_empty() {
                                parts.push(text.to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if !parts.is_empty() {
        return Some(parts.join("\n\n"));
    }
    v.pointer("/choices/0/message/content")
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
pub(crate) fn extract_grok_citations(v: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(citations) = v.get("citations").and_then(|x| x.as_array()) {
        for c in citations {
            if let Some(url) = c.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                push_unique(&mut out, url);
            }
        }
    }
    collect_annotation_urls(v, &mut out);
    out
}

#[cfg(test)]
fn collect_annotation_urls(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(url) = map.get("url").and_then(|x| x.as_str()) {
                push_unique(out, url.trim());
            }
            for value in map.values() {
                collect_annotation_urls(value, out);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_annotation_urls(value, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
fn push_unique(out: &mut Vec<String>, value: &str) {
    if value.is_empty() || out.iter().any(|seen| seen == value) {
        return;
    }
    out.push(value.to_string());
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/grok__tests.rs"]
mod tests;
