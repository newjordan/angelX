//! Lifecycle hooks: PreToolUse / PostToolUse / SessionStart shell commands.

use super::*;

/// Machine-stable prefix for a PreToolUse policy denial: the `[angel-hook-blocked/v1]`
/// marker's page address (`⠰⠙⠉`), which the append-only book keeps stable.
/// Keep the human reason after this marker, but never make turn accounting
/// infer denial from prose.
pub(crate) const HOOK_BLOCKED_PREFIX: &str = "⠰⠙⠉ ";
/// The marker as sessions saved before it became a page carry it.
const LEGACY_HOOK_BLOCKED_PREFIX: &str = "[angel-hook-blocked/v1] ";
const HOOK_ENV_VALUE_MAX_BYTES: usize = 32 * 1024;
const HOOK_COMMAND_MAX_BYTES: usize = 64 * 1024;

/// `source` values for a SessionStart payload. Matchers use the same `*` / `|`
/// rules as tool names, compared against this source.
pub(crate) const SESSION_SOURCE_STARTUP: &str = "startup";
pub(crate) const SESSION_SOURCE_RESUME: &str = "resume";
pub(crate) const SESSION_SOURCE_CLEAR: &str = "clear";
pub(crate) const SESSION_SOURCE_FORK: &str = "fork";
pub(crate) const SESSION_SOURCE_WORKSPACE: &str = "workspace";

pub(crate) fn is_hook_blocked_result(result: &str) -> bool {
    result.starts_with(HOOK_BLOCKED_PREFIX) || result.starts_with(LEGACY_HOOK_BLOCKED_PREFIX)
}

// ---------------------------------------------------------------------------
// Lifecycle hooks — run configured shell commands at PreToolUse / PostToolUse /
// SessionStart (the Codex/Claude-Code hooks model). A PreToolUse hook can BLOCK
// a tool. Config at ~/.angelX/hooks.json (override ANGEL_HOOKS_CONFIG):
//   { "hooks": {
//       "PreToolUse":   [ { "matcher": "shell|cargo", "command": "..." } ],
//       "PostToolUse":  [ { "matcher": "*",           "command": "..." } ],
//       "SessionStart": [ { "matcher": "*",           "command": "..." } ] } }
// Tool hooks run via `sh -c` with ANGEL_TOOL_NAME / ANGEL_TOOL_ARGS (and, for
// PostToolUse, ANGEL_TOOL_RESULT) in the env, and stdin is `/dev/null`.
// SessionStart uses the same `sh -c` launch. Its stdin is one JSON object:
//   {"hook_event_name":"SessionStart","session_id":"<id>","cwd":"<workspace>","source":"startup|resume|clear|fork|workspace"}
// and the child env sets ANGEL_SESSION_ID and ANGEL_WORKSPACE to that workspace.
// The payload has no conversation content, credentials, or transcript path.
// SessionStart is best-effort, like PostToolUse: a missing, failing, or slow
// hook does not block or change the session.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HookRule {
    pub(crate) matcher: String,
    pub(crate) command: String,
}

#[derive(Clone, Default)]
pub(crate) struct Hooks {
    pub(crate) pre: Vec<HookRule>,
    pub(crate) post: Vec<HookRule>,
    /// Session announcements. Not a tool gate: [`Hooks::is_empty`] ignores them
    /// so dispatch, approval, and tool-batch parallelism stay unchanged.
    pub(crate) session_start: Vec<HookRule>,
}

impl Hooks {
    pub(crate) fn load() -> Self {
        let path = std::env::var_os("ANGEL_HOOKS_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".angelX/hooks.json")
            });
        // Re-parsed only when the file changes (mtime+len key) — load() runs at
        // the top of every turn. Hook edits still take effect live because a
        // save bumps the key; a missing file caches as empty the same way.
        type Key = Option<(std::time::SystemTime, u64)>;
        static CACHE: std::sync::Mutex<Option<(PathBuf, Key, Hooks)>> = std::sync::Mutex::new(None);
        let key: Key = std::fs::metadata(&path)
            .ok()
            .and_then(|m| Some((m.modified().ok()?, m.len())));
        if let Ok(guard) = CACHE.lock()
            && let Some((p, k, hooks)) = guard.as_ref()
            && *p == path
            && *k == key
        {
            return hooks.clone();
        }
        let hooks = match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
        };
        if let Ok(mut guard) = CACHE.lock() {
            *guard = Some((path, key, hooks.clone()));
        }
        hooks
    }

    pub(crate) fn parse(text: &str) -> Self {
        let v: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let read = |event: &str| -> Vec<HookRule> {
            v.get("hooks")
                .and_then(|h| h.get(event))
                .and_then(|e| e.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|r| {
                            let command = r.get("command")?.as_str()?.to_string();
                            let matcher = r
                                .get("matcher")
                                .and_then(|m| m.as_str())
                                .unwrap_or("*")
                                .to_string();
                            Some(HookRule { matcher, command })
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            pre: read("PreToolUse"),
            post: read("PostToolUse"),
            session_start: read("SessionStart"),
        }
    }

    /// Tool hooks only. SessionStart-only config must not serialize tool batches
    /// or take the pre/post path.
    pub(crate) fn is_empty(&self) -> bool {
        self.pre.is_empty() && self.post.is_empty()
    }

    /// Run PreToolUse hooks; `Some(reason)` means a hook BLOCKED the tool (the
    /// hook's output becomes the tool result and the tool never runs).
    pub(crate) fn pre_tool_use(&self, name: &str, args: &Value) -> Option<String> {
        if crate::platform::yolo::enabled() {
            return None;
        }
        for rule in &self.pre {
            if hook_matches(&rule.matcher, name) {
                let outcome = run_hook(&rule.command, name, args, None);
                let fail_open = crate::agent::harness::env_flag("ANGEL_PRE_HOOK_FAIL_OPEN", false);
                let failure = match outcome {
                    HookRun::Passed { .. } => continue,
                    // A non-zero exit is a deliberate policy rejection. The
                    // infrastructure fail-open switch must never override it.
                    HookRun::Rejected { code, output } => {
                        format!("hook exited {code}: {}", hook_reason(&output))
                    }
                    HookRun::TimedOut if fail_open => continue,
                    HookRun::TimedOut => "hook timed out".to_string(),
                    HookRun::Unavailable(_) if fail_open => continue,
                    HookRun::Unavailable(reason) => {
                        format!("hook unavailable: {}", hook_reason(&reason))
                    }
                };
                // A configured PreToolUse hook is policy, not decoration.
                // Infrastructure failure therefore denies by default; an
                // operator may deliberately restore the historical fail-open
                // posture for timeout/spawn failures with one explicit switch.
                return Some(format!(
                    "{HOOK_BLOCKED_PREFIX}tool '{name}' blocked by PreToolUse hook: {failure}"
                ));
            }
        }
        None
    }

    pub(crate) fn post_tool_use(&self, name: &str, args: &Value, result: &str) {
        for rule in &self.post {
            if hook_matches(&rule.matcher, name) {
                let _ = run_hook(&rule.command, name, args, Some(result));
            }
        }
    }
}

/// Announce the session the process holds now. No-op when no SessionStart rule
/// matches `source`. Hook failure, timeout, and a missing config do not change
/// the session; callers invoke this only after that session is already current.
pub(crate) fn emit_session_start(session_id: &str, workspace: &Path, source: &str) {
    let hooks = Hooks::load();
    if hooks.session_start.is_empty() {
        return;
    }
    let cwd_path = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let cwd = path_for_hook(&cwd_path);
    for rule in &hooks.session_start {
        if hook_matches(&rule.matcher, source) {
            let _ = run_session_start_hook(&rule.command, session_id, &cwd, source);
        }
    }
}

fn path_for_hook(path: &Path) -> String {
    path.to_str()
        .map(str::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn hook_reason(output: &str) -> &str {
    let output = output.trim();
    if output.is_empty() {
        "(no message)"
    } else {
        output
    }
}

/// `*` matches every tool; otherwise a `|`-separated list — a segment matches if
/// it equals the tool name or is a substring of it.
pub(crate) fn hook_matches(matcher: &str, name: &str) -> bool {
    let m = matcher.trim();
    m == "*"
        || m.split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .any(|s| s == name || name.contains(s))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HookRun {
    Passed { output: String },
    Rejected { code: i32, output: String },
    TimedOut,
    Unavailable(String),
}

fn bounded_hook_env(field: &str, value: &str) -> String {
    if value.len() <= HOOK_ENV_VALUE_MAX_BYTES {
        return value.to_string();
    }
    serde_json::json!({
        "schema": "angel-hook-payload/v1",
        "field": field,
        "omitted": true,
        "bytes": value.len(),
        "sha256": crate::knowledge::cut::sha256_hex(value.as_bytes()),
    })
    .to_string()
}

/// Run one hook via `sh -c`, passing bounded tool context in the environment.
/// Oversized argument/results become a small JSON receipt with byte length and
/// SHA-256, preventing E2BIG from turning the riskiest calls into policy bypasses.
/// Every infrastructure outcome is explicit so PreToolUse can fail closed while
/// PostToolUse remains best-effort.
pub(crate) fn run_hook(command: &str, name: &str, args: &Value, result: Option<&str>) -> HookRun {
    if command.len() > HOOK_COMMAND_MAX_BYTES {
        return HookRun::Unavailable(format!(
            "command is {} bytes (limit {HOOK_COMMAND_MAX_BYTES})",
            command.len()
        ));
    }
    let args = bounded_hook_env("args", &args.to_string());
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .env("ANGEL_TOOL_NAME", bounded_hook_env("name", name))
        .env("ANGEL_TOOL_ARGS", args);
    if let Some(r) = result {
        cmd.env("ANGEL_TOOL_RESULT", bounded_hook_env("result", r));
    }
    let timeout = Duration::from_secs(env_usize("ANGEL_HOOK_TIMEOUT", 10) as u64);
    classify_hook_output(output_timed(cmd, Some(timeout)))
}

fn classify_hook_output(result: Result<(std::process::Output, bool), String>) -> HookRun {
    match result {
        Ok((_, true)) => HookRun::TimedOut,
        Ok((out, false)) => {
            // A hook terminated by a signal did not succeed. Treat it as a
            // conventional non-zero shell status instead of silently allowing
            // the guarded tool through.
            let code = out.status.code().unwrap_or_else(|| {
                use std::os::unix::process::ExitStatusExt;
                out.status.signal().map(|signal| 128 + signal).unwrap_or(1)
            });
            let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&out.stderr));
            if code == 0 {
                HookRun::Passed { output: s }
            } else {
                HookRun::Rejected { code, output: s }
            }
        }
        Err(error) => HookRun::Unavailable(error),
    }
}

/// One SessionStart command via `sh -c`. Stdin is the JSON object plus a
/// trailing newline. `ANGEL_SESSION_ID` and `ANGEL_WORKSPACE` name the session
/// the process holds now (the workspace overrides an inherited startup value).
///
/// The wait is `ANGEL_HOOK_TIMEOUT` and stays armed when YOLO lifts tool
/// deadlines, so a slow hook cannot stall session establishment. A zero budget
/// is already expired and does not spawn. Callers ignore this result.
pub(crate) fn run_session_start_hook(
    command: &str,
    session_id: &str,
    cwd: &str,
    source: &str,
) -> HookRun {
    if command.len() > HOOK_COMMAND_MAX_BYTES {
        return HookRun::Unavailable(format!(
            "command is {} bytes (limit {HOOK_COMMAND_MAX_BYTES})",
            command.len()
        ));
    }
    let timeout = Duration::from_secs(env_usize("ANGEL_HOOK_TIMEOUT", 10) as u64);
    if timeout.is_zero() {
        return HookRun::TimedOut;
    }
    let mut body = serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": session_id,
        "cwd": cwd,
        "source": source,
    })
    .to_string();
    body.push('\n');
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .env("ANGEL_SESSION_ID", session_id)
        .env("ANGEL_WORKSPACE", cwd);
    classify_hook_output(output_timed_fixed_with_stdin(cmd, timeout, body.as_bytes()))
}

/// Dispatch a tool through the hook pipeline: PreToolUse (may block) → dispatch →
/// PostToolUse. The no-hooks fast path is a plain dispatch.
pub(crate) fn dispatch_with_hooks(
    registry: &ToolRegistry,
    hooks: &Hooks,
    name: &str,
    args: &Value,
) -> String {
    dispatch_with_hooks_events(registry, hooks, name, args, None)
}

pub(crate) fn dispatch_with_hooks_events(
    registry: &ToolRegistry,
    hooks: &Hooks,
    name: &str,
    args: &Value,
    event_context: Option<(&ToolEventId, &mpsc::Sender<TurnEvent>)>,
) -> String {
    dispatch_with_hooks_events_cancel(registry, hooks, name, args, event_context, None)
}

pub(crate) fn dispatch_with_hooks_events_cancel(
    registry: &ToolRegistry,
    hooks: &Hooks,
    name: &str,
    args: &Value,
    event_context: Option<(&ToolEventId, &mpsc::Sender<TurnEvent>)>,
    cancel: Option<&AtomicBool>,
) -> String {
    super::exec::set_sandbox_receipt(None);
    if !hooks.is_empty()
        && let Some(reason) = hooks.pre_tool_use(name, args)
    {
        return reason;
    }
    // code_mode is dispatched here, not via `registry.dispatch`, because the V8
    // runtime needs to re-enter the registry (+ hooks) for its nested tool calls
    // — which the registered tool itself can't reach (it can't own its container).
    // The pre/post hooks above/below still wrap the whole code_mode turn; nested
    // calls get their own pre/post inside `run_code_mode_tool`.
    let result = if name == "code_mode" {
        run_code_mode_tool(registry, hooks, args, event_context, cancel)
    } else {
        registry
            .dispatch_with_cancel(name, args, cancel)
            .unwrap_or_else(|e| format!("tool error: {e}"))
    };
    if let Some(receipt) = super::exec::sandbox_receipt()
        && let Some(notice) = receipt.get("notice").and_then(Value::as_str)
        && let Some((_, events)) = event_context
    {
        let _ = events.send(TurnEvent::Notice(notice.into()));
        if receipt["aliases_unprotected"]
            .as_array()
            .is_some_and(|paths| !paths.is_empty())
        {
            let _ = events.send(TurnEvent::Notice(
                "sandbox: alias copy incomplete; see aliases_unprotected in the sandbox receipt"
                    .into(),
            ));
        }
    }
    if !hooks.is_empty() {
        hooks.post_tool_use(name, args, &result);
    }
    result
}
