//! pxpipe request compression for image-capable SOTA clubs.
//!
//! pxpipe renders bulky OpenAI request context into PNG input parts. That only
//! belongs on models with reliable image input, so angel0 gates it by label/model
//! and keeps every unsupported provider byte-identical.

use super::*;
use crate::sandbox::process_owner::{Child, OwnedCommandExt};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::Duration;

const DEFAULT_PXPIPE_MODELS: &[&str] = &["gpt-5", "gpt-4o", "gpt-4.1"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PxpipeApi {
    ChatCompletions,
    Responses,
}

impl PxpipeApi {
    fn arg(self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat",
            Self::Responses => "responses",
        }
    }
}

/// Transform a serialized OpenAI-style request with pxpipe when explicitly
/// enabled and the current model is eligible. Opt-in mode is fail-open: a
/// missing helper/package leaves the original request untouched. Set
/// `ANGEL_PXPIPE_REQUIRED=1` to fail closed after opting in.
pub(crate) fn maybe_pxpipe_transform(
    api: PxpipeApi,
    label: &str,
    model: &str,
    body: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if body.is_empty() || !pxpipe_enabled() || !pxpipe_model_allowed(label, model, None) {
        return Ok(body);
    }

    match run_pxpipe_helper(api, label, model, &body) {
        Ok(out) if !out.is_empty() => Ok(out),
        Ok(_) => pxpipe_fallback(
            body,
            label,
            model,
            "helper returned an empty request body".to_string(),
        ),
        Err(err) => pxpipe_fallback(body, label, model, err),
    }
}

fn pxpipe_fallback(
    body: Vec<u8>,
    label: &str,
    model: &str,
    err: String,
) -> Result<Vec<u8>, String> {
    if pxpipe_required() {
        return Err(format!(
            "pxpipe transform failed for {label}/{model}: {err}"
        ));
    }
    log_pxpipe_once(
        format!("{label}:{model}:{err}"),
        format!("[pxpipe] pass-through for {label}/{model}: {err}"),
    );
    Ok(body)
}

/// The persistent helper: one node process in `serve` mode answering framed
/// requests. Replaces a fresh `node` spawn + `pxpipe-proxy` import per SOTA
/// request (~50-150ms each, on every hop). Protocol per request:
/// `{"api","label","model","len"}\n` + body bytes → `{"ok","len"|"err"}\n`
/// (+ body bytes on ok).
struct PxpipeDaemon {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

/// The daemon slot; the mutex also serializes requests (one in flight).
static PXPIPE_DAEMON: OnceLock<Mutex<Option<PxpipeDaemon>>> = OnceLock::new();
/// Latched after the first completed round-trip. A helper that has *never*
/// answered (an `ANGEL_PXPIPE_HELPER` override that predates serve mode)
/// downgrades to one-shot permanently instead of paying a respawn per request;
/// a proven daemon that later dies just respawns.
static PXPIPE_DAEMON_PROVEN: AtomicBool = AtomicBool::new(false);
static PXPIPE_DAEMON_OFF: AtomicBool = AtomicBool::new(false);

fn pxpipe_persistent() -> bool {
    env_flag("ANGEL_PXPIPE_PERSISTENT", true)
}

fn pxpipe_timeout() -> Duration {
    let millis = env_first(&["ANGEL_PXPIPE_TIMEOUT_MS"])
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5_000)
        .clamp(50, 30_000);
    Duration::from_millis(millis)
}

fn pxpipe_watchdog(
    pid: u32,
    timeout: Duration,
) -> (mpsc::SyncSender<()>, std::thread::JoinHandle<bool>) {
    let (cancel, rx) = mpsc::sync_channel(1);
    let watchdog = std::thread::spawn(move || match rx.recv_timeout(timeout) {
        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => false,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            #[cfg(unix)]
            // SAFETY: every pxpipe child is placed in its own process group at
            // spawn, with the child pid as group id.
            unsafe {
                libc::killpg(pid as libc::pid_t, libc::SIGKILL);
            }
            true
        }
    });
    (cancel, watchdog)
}

fn finish_pxpipe_watchdog(
    cancel: mpsc::SyncSender<()>,
    watchdog: std::thread::JoinHandle<bool>,
) -> bool {
    let _ = cancel.send(());
    watchdog.join().unwrap_or(true)
}

fn spawn_pxpipe_daemon() -> Result<PxpipeDaemon, String> {
    let helper = pxpipe_helper_path();
    if !helper.is_file() {
        return Err(format!("helper not found at {}", helper.display()));
    }
    let node = env_first(&["ANGEL_PXPIPE_NODE"]).unwrap_or_else(|| "node".to_string());
    let mut command = Command::new(node);
    command
        .arg(helper)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command
        .spawn_owned()
        .map_err(|e| format!("spawn helper: {e}"))?;
    let stdin = child.stdin.take().ok_or("helper stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("helper stdout unavailable")?;
    Ok(PxpipeDaemon {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

/// One framed request/response on the live daemon. Outer `Err` = transport
/// failure (dead pipe, bad framing — the daemon gets respawned); inner `Err` =
/// the helper answered but rejected the transform (final, like a one-shot
/// non-zero exit — the warm daemon is kept).
fn pxpipe_daemon_roundtrip(
    d: &mut PxpipeDaemon,
    api: PxpipeApi,
    label: &str,
    model: &str,
    body: &[u8],
) -> Result<Result<Vec<u8>, String>, String> {
    let timeout = pxpipe_timeout();
    let (cancel, watchdog) = pxpipe_watchdog(d.child.id(), timeout);
    let result = (|| {
        let header = serde_json::json!({
            "api": api.arg(), "label": label, "model": model, "len": body.len(),
        });
        let mut frame = header.to_string().into_bytes();
        frame.push(b'\n');
        frame.extend_from_slice(body);
        d.stdin
            .write_all(&frame)
            .and_then(|()| d.stdin.flush())
            .map_err(|e| format!("write helper: {e}"))?;
        let mut line = String::new();
        match d.stdout.read_line(&mut line) {
            Ok(0) => return Err("helper closed the pipe".to_string()),
            Ok(_) => {}
            Err(e) => return Err(format!("read helper: {e}")),
        }
        let resp: serde_json::Value =
            serde_json::from_str(line.trim()).map_err(|e| format!("bad helper response: {e}"))?;
        if !resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            let err = resp
                .get("err")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown helper error");
            return Ok(Err(format!("helper transform failed: {err}")));
        }
        let len = resp
            .get("len")
            .and_then(|v| v.as_u64())
            .ok_or("helper response missing len")? as usize;
        let mut out = vec![0u8; len];
        d.stdout
            .read_exact(&mut out)
            .map_err(|e| format!("read helper body: {e}"))?;
        Ok(Ok(out))
    })();
    if finish_pxpipe_watchdog(cancel, watchdog) {
        Err(format!("helper timed out after {}ms", timeout.as_millis()))
    } else {
        result
    }
}

/// Transform via the persistent daemon, spawning (or once respawning) it as
/// needed. Same outer/inner error split as [`pxpipe_daemon_roundtrip`].
fn run_pxpipe_daemon(
    api: PxpipeApi,
    label: &str,
    model: &str,
    body: &[u8],
) -> Result<Result<Vec<u8>, String>, String> {
    let slot = PXPIPE_DAEMON.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    let mut last = String::new();
    for _ in 0..2 {
        if guard.is_none() {
            *guard = Some(spawn_pxpipe_daemon()?);
        }
        let d = guard.as_mut().expect("daemon slot just filled");
        match pxpipe_daemon_roundtrip(d, api, label, model, body) {
            Ok(done) => {
                PXPIPE_DAEMON_PROVEN.store(true, std::sync::atomic::Ordering::Relaxed);
                return Ok(done);
            }
            Err(e) => {
                last = e;
                if let Some(mut dead) = guard.take() {
                    let _ = dead.child.kill();
                    let _ = dead.child.wait();
                }
                // A helper that never completed a round-trip won't on a respawn
                // either (it likely doesn't speak serve mode) — don't thrash.
                if !PXPIPE_DAEMON_PROVEN.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
            }
        }
    }
    Err(last)
}

pub(super) fn run_pxpipe_helper(
    api: PxpipeApi,
    label: &str,
    model: &str,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    // Persistent daemon first. Debug mode stays on one-shot (its per-request
    // stderr goes to the caller there; the daemon's is detached), and a
    // transport-dead daemon falls back — permanently when it never worked.
    if pxpipe_persistent()
        && !env_flag("ANGEL_PXPIPE_DEBUG", false)
        && !PXPIPE_DAEMON_OFF.load(std::sync::atomic::Ordering::Relaxed)
    {
        match run_pxpipe_daemon(api, label, model, body) {
            Ok(done) => return done,
            Err(infra) => {
                if !PXPIPE_DAEMON_PROVEN.load(std::sync::atomic::Ordering::Relaxed) {
                    PXPIPE_DAEMON_OFF.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                log_pxpipe_once(
                    format!("daemon:{infra}"),
                    format!(
                        "[pxpipe] persistent helper unavailable ({infra}); using one-shot spawns"
                    ),
                );
            }
        }
    }
    let helper = pxpipe_helper_path();
    if !helper.is_file() {
        return Err(format!("helper not found at {}", helper.display()));
    }
    let node = env_first(&["ANGEL_PXPIPE_NODE"]).unwrap_or_else(|| "node".to_string());
    let mut command = Command::new(node);
    command
        .arg(helper)
        .arg(api.arg())
        .env("ANGEL_PXPIPE_LABEL", label)
        .env("ANGEL_PXPIPE_MODEL", model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command
        .spawn_owned()
        .map_err(|e| format!("spawn helper: {e}"))?;
    let timeout = pxpipe_timeout();
    let (cancel, watchdog) = pxpipe_watchdog(child.id(), timeout);
    let write_result = child
        .stdin
        .as_mut()
        .ok_or_else(|| "helper stdin unavailable".to_string())
        .and_then(|stdin| {
            stdin
                .write_all(body)
                .map_err(|e| format!("write helper stdin: {e}"))
        });
    if let Err(error) = write_result {
        #[cfg(unix)]
        // SAFETY: this one-shot child leads a private process group.
        unsafe {
            libc::killpg(child.id() as libc::pid_t, libc::SIGKILL);
        }
        let _ = child.kill();
        let _ = child.wait();
        let _ = finish_pxpipe_watchdog(cancel, watchdog);
        return Err(error);
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("wait helper: {e}"));
    if finish_pxpipe_watchdog(cancel, watchdog) {
        return Err(format!("helper timed out after {}ms", timeout.as_millis()));
    }
    let output = output?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "helper exited with {}: {}",
            output.status,
            truncate_for_log(stderr.trim(), 500)
        ));
    }
    Ok(output.stdout)
}

fn pxpipe_helper_path() -> PathBuf {
    if let Some(path) = env_first(&["ANGEL_PXPIPE_HELPER"]) {
        return PathBuf::from(path);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().unwrap_or(&manifest);
    root.join("scripts").join("pxpipe-transform.mjs")
}

fn pxpipe_enabled() -> bool {
    env_flag("ANGEL_PXPIPE", false)
}

fn pxpipe_required() -> bool {
    env_flag("ANGEL_PXPIPE_REQUIRED", false)
}

fn env_flag(key: &str, default: bool) -> bool {
    match std::env::var(key)
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
    {
        Some(v) if is_falsey(&v) => false,
        Some(_) => true,
        None => default,
    }
}

pub(crate) fn pxpipe_model_allowed(
    label: &str,
    model: &str,
    override_models: Option<&str>,
) -> bool {
    let selectors = override_models
        .map(parse_pxpipe_models)
        .or_else(|| {
            env_first(&["ANGEL_PXPIPE_MODELS", "PXPIPE_MODELS"]).map(|s| parse_pxpipe_models(&s))
        })
        .unwrap_or_else(|| {
            DEFAULT_PXPIPE_MODELS
                .iter()
                .map(|s| normalize_model_id(s))
                .collect()
        });
    if selectors.is_empty() {
        return false;
    }
    let label = normalize_model_id(label);
    let model = normalize_model_id(model);
    selectors
        .iter()
        .any(|s| model_matches(&label, s) || model_matches(&model, s))
}

fn parse_pxpipe_models(raw: &str) -> Vec<String> {
    if is_falsey(raw) {
        return Vec::new();
    }
    raw.split(',')
        .map(normalize_model_id)
        .filter(|s| !s.is_empty())
        .collect()
}

fn model_matches(target: &str, selector: &str) -> bool {
    !target.is_empty()
        && !selector.is_empty()
        && (target == selector
            || target.starts_with(&format!("{selector}-"))
            || target.starts_with(&format!("{selector}.")))
}

fn normalize_model_id(s: &str) -> String {
    let mut out = String::new();
    let mut in_variant = false;
    for ch in s.trim().chars() {
        match ch {
            '[' => in_variant = true,
            ']' => in_variant = false,
            _ if !in_variant => out.push(ch.to_ascii_lowercase()),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn is_falsey(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off" | "none"
    )
}

fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // `max` is a byte budget; slicing at it directly panics when it lands
        // inside a multi-byte UTF-8 char (the only caller passes arbitrary
        // subprocess stderr, which freely contains non-ASCII). Walk back to the
        // nearest char boundary first.
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod truncate_stress {
    use super::truncate_for_log;

    #[test]
    fn never_splits_a_multibyte_char() {
        // Subprocess stderr is arbitrary UTF-8; a byte budget landing inside a
        // multi-byte char used to panic. Try every budget across a run of 2- and
        // 3-byte chars — none may panic.
        let s: String = "é字".repeat(300); // é = 2 bytes, 字 = 3 bytes
        for max in 1..s.len() {
            let out = truncate_for_log(&s, max);
            assert!(out.is_empty() || out.ends_with("...") || out == s);
        }
        // Short input is returned verbatim.
        assert_eq!(truncate_for_log("hi", 100), "hi");
    }
}

fn log_pxpipe_once(key: String, msg: String) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut seen) = seen.lock() else {
        return;
    };
    if seen.insert(key) {
        eprintln!("{msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pxpipe_is_disabled_by_default() {
        let _guard = crate::tests::env_lock();
        let saved = std::env::var_os("ANGEL_PXPIPE");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_PXPIPE") };

        assert!(!pxpipe_enabled());

        match saved {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var("ANGEL_PXPIPE", value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_PXPIPE") },
        }
    }

    #[cfg(unix)]
    #[test]
    fn persistent_roundtrip_hang_is_killed_at_the_request_deadline() {
        use std::os::unix::process::CommandExt as _;

        let _guard = crate::tests::env_lock();
        let _timeout = crate::tests::TestEnvGuard::set("ANGEL_PXPIPE_TIMEOUT_MS", "50");
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 30 & wait"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = command.spawn_owned().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut daemon = PxpipeDaemon {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };
        let started = std::time::Instant::now();

        let error = pxpipe_daemon_roundtrip(
            &mut daemon,
            PxpipeApi::Responses,
            "openai",
            "gpt-5.5",
            br#"{"model":"gpt-5.5"}"#,
        )
        .unwrap_err();

        assert!(error.contains("timed out after 50ms"), "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "persistent pxpipe daemon outlived its request deadline: {:?}",
            started.elapsed()
        );
        let _ = daemon.child.wait();
    }
}
