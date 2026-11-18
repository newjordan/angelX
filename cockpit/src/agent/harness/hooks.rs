//! Lifecycle hooks: PreToolUse / PostToolUse / SessionStart shell commands.

use super::*;

pub(crate) const HOOK_BLOCKED_PREFIX: &str = "⠰⠙⠉ ";
const LEGACY_HOOK_BLOCKED_PREFIX: &str = "[angel-hook-blocked/v1] ";
const HOOK_ENV_VALUE_MAX_BYTES: usize = 32 * 1024;
const HOOK_COMMAND_MAX_BYTES: usize = 64 * 1024;

pub(crate) fn is_hook_blocked_result(result: &str) -> bool {
    result.starts_with(HOOK_BLOCKED_PREFIX) || result.starts_with(LEGACY_HOOK_BLOCKED_PREFIX)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HookRule {
    pub(crate) matcher: String,
    pub(crate) command: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SessionStartSource {
    Startup,
    Resume,
    Clear,
    Fork,
    Workspace,
}

impl SessionStartSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Resume => "resume",
            Self::Clear => "clear",
            Self::Fork => "fork",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Hooks {
    pub(crate) pre: Vec<HookRule>,
    pub(crate) post: Vec<HookRule>,
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

    pub(crate) fn is_empty(&self) -> bool {
        self.pre.is_empty() && self.post.is_empty()
    }

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

    pub(crate) fn session_start(
        &self,
        session_id: &str,
        workspace: &Path,
        source: SessionStartSource,
    ) {
        if session_id.is_empty() || self.session_start.is_empty() {
            return;
        }
        let cwd = canonical_workspace(workspace);
        let payload = session_start_payload(session_id, &cwd, source);
        for rule in &self.session_start {
            if hook_matches(&rule.matcher, source.as_str()) {
                let _ = run_session_start_hook(&rule.command, &payload, &cwd, session_id);
            }
        }
    }
}

pub(crate) fn fire_session_start(session_id: &str, workspace: &Path, source: SessionStartSource) {
    #[cfg(test)]
    if std::env::var_os("ANGEL_HOOKS_CONFIG").is_none() {
        return;
    }
    if session_id.is_empty() {
        return;
    }
    let hooks = Hooks::load();
    if hooks.session_start.is_empty() {
        return;
    }
    hooks.session_start(session_id, workspace, source);
}

fn canonical_workspace(workspace: &Path) -> PathBuf {
    std::fs::canonicalize(workspace).unwrap_or_else(|_| {
        if workspace.is_absolute() {
            workspace.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(workspace))
                .unwrap_or_else(|_| workspace.to_path_buf())
        }
    })
}

fn session_start_payload(session_id: &str, cwd: &Path, source: SessionStartSource) -> Vec<u8> {
    let mut body = serde_json::to_vec(&serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": session_id,
        "cwd": cwd.to_string_lossy(),
        "source": source.as_str(),
    }))
    .unwrap_or_default();
    body.push(b'\n');
    body
}

fn hook_reason(output: &str) -> &str {
    let output = output.trim();
    if output.is_empty() {
        "(no message)"
    } else {
        output
    }
}

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
    match output_timed(cmd, Some(timeout)) {
        Ok((_, true)) => HookRun::TimedOut,
        Ok((out, false)) => {
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

pub(crate) fn run_session_start_hook(
    command: &str,
    payload: &[u8],
    cwd: &Path,
    session_id: &str,
) -> HookRun {
    use crate::agent::sandbox::process_owner::OwnedCommandExt;
    use std::io::Write;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    if command.len() > HOOK_COMMAND_MAX_BYTES {
        return HookRun::Unavailable(format!(
            "command is {} bytes (limit {HOOK_COMMAND_MAX_BYTES})",
            command.len()
        ));
    }
    let cwd_env = cwd.to_string_lossy();
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .env("ANGEL_SESSION_ID", session_id)
        .env("ANGEL_WORKSPACE", cwd_env.as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    if cwd.is_dir() {
        cmd.current_dir(cwd);
    }
    let timeout = Duration::from_secs(env_usize("ANGEL_HOOK_TIMEOUT", 10) as u64);
    let mut child = match cmd.spawn_owned() {
        Ok(child) => child,
        Err(error) => return HookRun::Unavailable(error.to_string()),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload);
    }
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or_else(|| {
                    use std::os::unix::process::ExitStatusExt;
                    status.signal().map(|signal| 128 + signal).unwrap_or(1)
                });
                return if code == 0 {
                    HookRun::Passed {
                        output: String::new(),
                    }
                } else {
                    HookRun::Rejected {
                        code,
                        output: String::new(),
                    }
                };
            }
            Ok(None) if started.elapsed() >= timeout => {
                kill_process_group(child.id());
                let _ = child.wait();
                return HookRun::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => return HookRun::Unavailable(error.to_string()),
        }
    }
}

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
