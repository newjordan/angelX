//! Per-call wire diagnostics for streamed model calls.
//!
//! Every streamed call gets a [`WireCall`]: counters the stream loop bumps
//! (bytes, events, keep-alives, text and reasoning size, first token), a
//! heartbeat thread that reports on stderr while the call runs long (including
//! while it is blocked in a socket read), and one JSONL record at the end when
//! `ANGEL_WIRE_LOG_DIR` is set (or `ANGEL_WIRE_LOG=1`, which writes to
//! `~/.angelX/wire`). Anything the parser ignored or repaired is noted on stderr
//! as it happens and kept in the record.
//!
//! The heartbeat prints every 60 s; `ANGEL_WIRE_HEARTBEAT_SECS` overrides it
//! and `0` turns heartbeats off. Observation only: nothing here changes a
//! request or a reply.

use super::{ClubReply, ToolCall};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static CALL_SEQ: AtomicU64 = AtomicU64::new(0);
const ARGS_RECORD_LIMIT: usize = 2_000;
const ARGS_NOTE_LIMIT: usize = 300;
const ERROR_RECORD_LIMIT: usize = 500;

/// How often a live call prints its status line to stderr
/// (`ANGEL_WIRE_HEARTBEAT_SECS`, default 60; `0` is silent). Observation only.
pub(crate) fn heartbeat_every() -> Duration {
    let configured = std::env::var("ANGEL_WIRE_HEARTBEAT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok());
    Duration::from_secs(configured.unwrap_or(60))
}

fn log_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("ANGEL_WIRE_LOG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    if std::env::var("ANGEL_WIRE_LOG").is_ok_and(|v| v.trim() == "1") {
        return std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".angelX/wire"));
    }
    None
}

/// The workspace this process was launched on (`--workspace`), else its cwd:
/// the key that ties a record to its task.
fn task_hint() -> &'static str {
    static HINT: OnceLock<String> = OnceLock::new();
    HINT.get_or_init(|| {
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|arg| arg == "--workspace")
            .and_then(|index| args.get(index + 1).cloned())
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|dir| dir.display().to_string())
            })
            .unwrap_or_default()
    })
}

fn truncate(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[derive(Default)]
struct Counters {
    bytes: AtomicU64,
    events: AtomicU64,
    keepalives: AtomicU64,
    text_chars: AtomicU64,
    reasoning_chars: AtomicU64,
    tool_frames: AtomicU64,
    /// Milliseconds after start, plus one; zero means not yet.
    first_token_ms: AtomicU64,
    last_byte_ms: AtomicU64,
}

struct Shared {
    label: String,
    started: Instant,
    counters: Counters,
    phase: Mutex<&'static str>,
}

impl Shared {
    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn first_token(&self) -> Option<Duration> {
        match self.counters.first_token_ms.load(Ordering::Relaxed) {
            0 => None,
            ms => Some(Duration::from_millis(ms - 1)),
        }
    }

    fn status_line(&self) -> String {
        let c = &self.counters;
        let elapsed = self.started.elapsed().as_secs_f64();
        let phase = *self.phase.lock().unwrap_or_else(|e| e.into_inner());
        let phase_text = match (phase, self.first_token()) {
            ("streaming", Some(first)) => {
                format!("streaming, first token at {:.1}s", first.as_secs_f64())
            }
            ("streaming", None) => "waiting for first token".to_string(),
            ("auth", _) => "refreshing credentials".to_string(),
            ("request", _) => "waiting for response headers".to_string(),
            (other, _) => other.to_string(),
        };
        let last_byte = c.last_byte_ms.load(Ordering::Relaxed);
        let since_byte = if c.bytes.load(Ordering::Relaxed) == 0 {
            "no bytes yet".to_string()
        } else {
            format!(
                "last byte {:.0}s ago",
                (self.elapsed_ms().saturating_sub(last_byte)) as f64 / 1000.0
            )
        };
        format!(
            "[wire:{}] {elapsed:.0}s · {phase_text} · {} B, {} events, {} keep-alives, {since_byte} \
             · text {} · reasoning {} · tool frames {}",
            self.label,
            c.bytes.load(Ordering::Relaxed),
            c.events.load(Ordering::Relaxed),
            c.keepalives.load(Ordering::Relaxed),
            c.text_chars.load(Ordering::Relaxed),
            c.reasoning_chars.load(Ordering::Relaxed),
            c.tool_frames.load(Ordering::Relaxed),
        )
    }
}

#[derive(Default)]
struct Detail {
    event_types: BTreeMap<String, u64>,
    noted_kinds: BTreeMap<String, u64>,
    notes: Vec<String>,
    finish_reason: Option<String>,
    usage: Option<Value>,
    /// The reasoning summary a Responses seat streamed, as the provider
    /// exposed it.
    reasoning_summary: String,
}

type Heartbeat = (mpsc::Sender<()>, std::thread::JoinHandle<()>);

/// One streamed model call under observation.
pub(crate) struct WireCall {
    seq: u64,
    club: String,
    model: String,
    dialect: &'static str,
    shared: Arc<Shared>,
    detail: Mutex<Detail>,
    heartbeat_every: Mutex<Duration>,
    heartbeat: Mutex<Option<Heartbeat>>,
}

impl WireCall {
    pub(crate) fn new(club: &str, model: &str, dialect: &'static str) -> Self {
        Self {
            seq: CALL_SEQ.fetch_add(1, Ordering::Relaxed) + 1,
            club: club.to_string(),
            model: model.to_string(),
            dialect,
            shared: Arc::new(Shared {
                label: format!("{club}/{model}"),
                started: Instant::now(),
                counters: Counters::default(),
                phase: Mutex::new("request"),
            }),
            detail: Mutex::new(Detail::default()),
            heartbeat_every: Mutex::new(Duration::ZERO),
            heartbeat: Mutex::new(None),
        }
    }

    /// Start the status heartbeat. Only the first call starts one; a later
    /// attempt on the same call keeps it.
    pub(crate) fn arm(&self) {
        let every = heartbeat_every();
        let mut heartbeat = self.heartbeat.lock().unwrap_or_else(|e| e.into_inner());
        if heartbeat.is_some() || every.is_zero() {
            return;
        }
        *self
            .heartbeat_every
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = every;
        let (stop, wait) = mpsc::channel::<()>();
        let observed = Arc::clone(&self.shared);
        let handle = std::thread::spawn(move || {
            while let Err(mpsc::RecvTimeoutError::Timeout) = wait.recv_timeout(every) {
                eprintln!("{}", observed.status_line());
            }
        });
        *heartbeat = Some((stop, handle));
    }

    fn stop_heartbeat(&self) {
        let taken = self
            .heartbeat
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some((stop, handle)) = taken {
            drop(stop);
            let _ = handle.join();
        }
    }

    fn detail(&self) -> std::sync::MutexGuard<'_, Detail> {
        self.detail.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn phase(&self, phase: &'static str) {
        *self.shared.phase.lock().unwrap_or_else(|e| e.into_inner()) = phase;
    }

    pub(crate) fn bytes(&self, n: usize) {
        let c = &self.shared.counters;
        c.bytes.fetch_add(n as u64, Ordering::Relaxed);
        c.last_byte_ms
            .store(self.shared.elapsed_ms(), Ordering::Relaxed);
    }

    /// A protocol event of `kind`; the per-call histogram shows the model's wire
    /// profile (e.g. how much of a Responses stream was reasoning summary).
    pub(crate) fn event(&self, kind: &str) {
        self.shared.counters.events.fetch_add(1, Ordering::Relaxed);
        *self
            .detail()
            .event_types
            .entry(kind.to_string())
            .or_default() += 1;
    }

    pub(crate) fn keepalive(&self) {
        self.shared
            .counters
            .keepalives
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn first_token(&self) {
        let ms = self.shared.elapsed_ms() + 1;
        let _ = self.shared.counters.first_token_ms.compare_exchange(
            0,
            ms,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    pub(crate) fn text(&self, chars: usize) {
        self.first_token();
        self.shared
            .counters
            .text_chars
            .fetch_add(chars as u64, Ordering::Relaxed);
    }

    pub(crate) fn reasoning(&self, chars: usize) {
        self.first_token();
        self.shared
            .counters
            .reasoning_chars
            .fetch_add(chars as u64, Ordering::Relaxed);
    }

    /// A reasoning summary delta: counted as reasoning, and kept for the record.
    pub(crate) fn reasoning_summary(&self, text: &str) {
        self.reasoning(text.len());
        self.detail().reasoning_summary.push_str(text);
    }

    pub(crate) fn tool_frame(&self) {
        self.first_token();
        self.shared
            .counters
            .tool_frames
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_finish_reason(&self, reason: Option<&str>) {
        self.detail().finish_reason = reason.map(str::to_string);
    }

    pub(crate) fn set_usage(&self, usage: Value) {
        self.detail().usage = Some(usage);
    }

    /// Something the parser ignored, dropped or repaired. Printed at once and
    /// kept in the record; a repeated `kind` prints only its first occurrence.
    pub(crate) fn note(&self, kind: &str, message: &str) {
        let first = {
            let mut detail = self.detail();
            detail.notes.push(format!("{kind}: {message}"));
            let count = detail.noted_kinds.entry(kind.to_string()).or_default();
            *count += 1;
            *count == 1
        };
        if first {
            eprintln!("[wire:{}] note {kind}: {message}", self.shared.label);
        }
    }

    /// Stop the heartbeat, print a summary when the call was slow, failed or
    /// noted anything, and append the JSONL record when a log dir is set.
    pub(crate) fn finish(self, result: &Result<ClubReply, String>) {
        self.stop_heartbeat();
        let duration = self.shared.started.elapsed();
        let c = &self.shared.counters;
        let outcome = match result {
            Ok(ClubReply::Text(_)) => "text",
            Ok(ClubReply::Calls(_)) => "calls",
            Err(e) if e.starts_with(super::INCOMPLETE_STREAM_ERR) => "incomplete",
            Err(_) => "error",
        };
        let calls: Vec<Value> = match result {
            Ok(ClubReply::Calls(calls)) => {
                calls.iter().map(|call| self.call_record(call)).collect()
            }
            _ => Vec::new(),
        };
        let detail = std::mem::take(&mut *self.detail());
        let first_token = self.shared.first_token();
        let failed = matches!(outcome, "incomplete" | "error");
        let every = *self
            .heartbeat_every
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let slow = !every.is_zero() && duration >= every;
        if failed || slow || !detail.notes.is_empty() {
            let tools = calls
                .iter()
                .map(|call| {
                    format!(
                        "{}({}B)",
                        call["name"].as_str().unwrap_or("?"),
                        call["args_len"].as_u64().unwrap_or(0)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let first = first_token
                .map(|d| format!("{:.1}s", d.as_secs_f64()))
                .unwrap_or_else(|| "none".to_string());
            let mut line = format!(
                "[wire:{}] {outcome} in {:.1}s · first token {first} · text {} · reasoning {} · {} keep-alives",
                self.shared.label,
                duration.as_secs_f64(),
                c.text_chars.load(Ordering::Relaxed),
                c.reasoning_chars.load(Ordering::Relaxed),
                c.keepalives.load(Ordering::Relaxed),
            );
            if !tools.is_empty() {
                line.push_str(&format!(" · tools [{tools}]"));
            }
            if let Some(reason) = &detail.finish_reason {
                line.push_str(&format!(" · finish {reason}"));
            }
            if let Err(error) = result {
                line.push_str(&format!(" · {}", truncate(error, ARGS_NOTE_LIMIT)));
            }
            eprintln!("{line}");
        }
        let Some(dir) = log_dir() else {
            return;
        };
        let record = json!({
            "ts": SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0),
            "pid": std::process::id(),
            "seq": self.seq,
            "task": task_hint(),
            "club": self.club,
            "model": self.model,
            "dialect": self.dialect,
            "outcome": outcome,
            "error": result.as_ref().err().map(|e| truncate(e, ERROR_RECORD_LIMIT)),
            "duration_ms": duration.as_millis() as u64,
            "first_token_ms": first_token.map(|d| d.as_millis() as u64),
            "bytes": c.bytes.load(Ordering::Relaxed),
            "events": c.events.load(Ordering::Relaxed),
            "keepalives": c.keepalives.load(Ordering::Relaxed),
            "text_chars": c.text_chars.load(Ordering::Relaxed),
            "reasoning_chars": c.reasoning_chars.load(Ordering::Relaxed),
            "reasoning_summary": (!detail.reasoning_summary.is_empty())
                .then_some(&detail.reasoning_summary),
            "tool_frames": c.tool_frames.load(Ordering::Relaxed),
            "event_types": detail.event_types,
            "finish_reason": detail.finish_reason,
            "usage": detail.usage,
            "notes": detail.notes,
            "reply_text_chars": match result {
                Ok(ClubReply::Text(text)) => Some(text.len()),
                _ => None,
            },
            "calls": calls,
        });
        let path = dir.join(format!("wire-{}.jsonl", std::process::id()));
        let written = std::fs::create_dir_all(&dir).and_then(|()| {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            writeln!(file, "{record}")
        });
        if let Err(error) = written {
            eprintln!("[wire] cannot append {}: {error}", path.display());
        }
    }

    /// One dispatched call, with its arguments as the model sent them. Arguments
    /// that did not parse as JSON are noted: the tool then saw `_raw`.
    fn call_record(&self, call: &ToolCall) -> Value {
        let raw = match call.args.get("_raw").and_then(Value::as_str) {
            Some(raw) => {
                self.note(
                    "unparsed_args",
                    &format!(
                        "{} arguments are not JSON: {}",
                        call.name,
                        truncate(raw, ARGS_NOTE_LIMIT)
                    ),
                );
                raw.to_string()
            }
            None => call.args.to_string(),
        };
        json!({
            "id": call.id,
            "name": call.name,
            "args_len": raw.len(),
            "args": truncate(&raw, ARGS_RECORD_LIMIT),
            "args_parsed": call.args.get("_raw").is_none(),
        })
    }
}

impl Drop for WireCall {
    fn drop(&mut self) {
        self.stop_heartbeat();
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/wire_log__tests.rs"]
mod tests;
