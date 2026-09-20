//! Read-only Yukon competition fleet telemetry for the loop HUD.
//!
//! Yukon exposes personal submissions per benchmark rather than through one
//! global command. A one-shot worker enumerates the currently open benchmarks,
//! fans those read-only queries out off the UI thread, and returns one bounded
//! snapshot. `App::advance` schedules the next sweep only after the previous
//! worker settles, so a slow CLI can never block or multiply behind the TUI.

use crate::sandbox::process_owner::OwnedCommandExt;
use std::collections::HashSet;
use std::env::VarError;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(45);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(50);
const MAX_CLI_OUTPUT_BYTES: usize = 1_048_576;
const BENCHMARK_SCOPE_ENV: &str = "ANGEL_YUKON_FLEET_BENCHMARKS";
const MAX_SCOPED_BENCHMARKS: usize = 32;
const MAX_BENCHMARK_NAME_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum YukonSubmissionPhase {
    Queued,
    Running,
    Accepted,
    Rejected,
    Unknown,
}

impl YukonSubmissionPhase {
    fn from_status(status: &str) -> Self {
        match status.trim().to_ascii_lowercase().as_str() {
            "queued" | "pending" | "submitted" => Self::Queued,
            "validating" | "running" | "in-flight" | "in_flight" => Self::Running,
            "accepted" | "promoted" | "complete" | "completed" => Self::Accepted,
            "rejected" | "failed" | "promotion failed" | "error" | "cancelled" | "canceled" => {
                Self::Rejected
            }
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct YukonSubmission {
    pub(crate) benchmark: String,
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) score: Option<String>,
    pub(crate) phase: YukonSubmissionPhase,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct YukonFleetSnapshot {
    pub(crate) entries: Vec<YukonSubmission>,
    pub(crate) benchmark_count: usize,
    pub(crate) failed_benchmarks: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct YukonFleetState {
    entries: Vec<YukonSubmission>,
    benchmark_count: usize,
    scanning: bool,
    stale: bool,
    error: Option<String>,
}

impl YukonFleetState {
    pub(crate) fn entries(&self) -> &[YukonSubmission] {
        &self.entries
    }

    pub(crate) fn benchmark_count(&self) -> usize {
        self.benchmark_count
    }

    pub(crate) fn scanning(&self) -> bool {
        self.scanning
    }

    pub(crate) fn stale(&self) -> bool {
        self.stale
    }

    pub(crate) fn has_error(&self) -> bool {
        self.error.is_some()
    }

    pub(crate) fn begin_scan(&mut self) {
        self.scanning = true;
    }

    pub(crate) fn apply(&mut self, snapshot: YukonFleetSnapshot) {
        self.entries = snapshot.entries;
        self.benchmark_count = snapshot.benchmark_count;
        self.scanning = false;
        self.stale = snapshot.failed_benchmarks > 0;
        self.error = (snapshot.failed_benchmarks > 0).then(|| {
            format!(
                "{} Yukon benchmark status quer{} failed",
                snapshot.failed_benchmarks,
                if snapshot.failed_benchmarks == 1 {
                    "y"
                } else {
                    "ies"
                }
            )
        });
    }

    pub(crate) fn fail(&mut self, error: String) {
        self.scanning = false;
        self.stale = !self.entries.is_empty();
        self.error = Some(error);
    }
}

pub(crate) fn spawn_poll() -> mpsc::Receiver<Result<YukonFleetSnapshot, String>> {
    let (tx, rx) = mpsc::sync_channel(1);
    let worker_tx = tx.clone();
    let spawn = thread::Builder::new()
        .name("angel-yukon-fleet".to_string())
        .spawn(move || {
            let _ = worker_tx.send(poll());
        });
    if let Err(error) = spawn {
        let _ = tx.send(Err(format!("start Yukon fleet watcher: {error}")));
    }
    rx
}

fn poll() -> Result<YukonFleetSnapshot, String> {
    let benchmarks = match configured_benchmarks()? {
        Some(benchmarks) => benchmarks,
        None => {
            let benchmark_output = run_yukon(&["benchmark", "list"], false)?;
            parse_benchmark_list(&benchmark_output)
        }
    };
    if benchmarks.is_empty() {
        return Ok(YukonFleetSnapshot::default());
    }

    let benchmark_count = benchmarks.len();
    let results = thread::scope(|scope| {
        let handles = benchmarks
            .into_iter()
            .map(|benchmark| {
                scope.spawn(move || {
                    let output = run_yukon(&["submissions", &benchmark], true)?;
                    Ok::<_, String>(
                        latest_submission(&benchmark, &output)
                            .into_iter()
                            .collect::<Vec<_>>(),
                    )
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "Yukon submission query panicked".to_string())
                    .and_then(|result| result)
            })
            .collect::<Vec<_>>()
    });

    let mut entries = Vec::new();
    let mut failed_benchmarks = 0usize;
    for result in results {
        match result {
            Ok(mut rows) => entries.append(&mut rows),
            Err(_) => failed_benchmarks = failed_benchmarks.saturating_add(1),
        }
    }
    if failed_benchmarks == benchmark_count {
        return Err("every Yukon benchmark status query failed".to_string());
    }
    entries.sort_by(|a, b| {
        phase_rank(a.phase)
            .cmp(&phase_rank(b.phase))
            .then_with(|| a.benchmark.cmp(&b.benchmark))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(YukonFleetSnapshot {
        entries,
        benchmark_count,
        failed_benchmarks,
    })
}

fn configured_benchmarks() -> Result<Option<Vec<String>>, String> {
    match std::env::var(BENCHMARK_SCOPE_ENV) {
        Ok(raw) => parse_benchmark_scope(&raw).map(Some),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(format!("{BENCHMARK_SCOPE_ENV} must be UTF-8")),
    }
}

fn parse_benchmark_scope(raw: &str) -> Result<Vec<String>, String> {
    let mut benchmarks = Vec::new();
    let mut seen = HashSet::new();
    for (index, raw_name) in raw.split(',').enumerate() {
        let name = raw_name.trim();
        if name.is_empty() {
            continue;
        }
        let valid = name.len() <= MAX_BENCHMARK_NAME_BYTES
            && name.contains('/')
            && name.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.')
            });
        if !valid {
            return Err(format!(
                "{BENCHMARK_SCOPE_ENV} entry {} is not a valid benchmark name",
                index + 1
            ));
        }
        if seen.insert(name.to_string()) {
            benchmarks.push(name.to_string());
        }
        if benchmarks.len() > MAX_SCOPED_BENCHMARKS {
            return Err(format!(
                "{BENCHMARK_SCOPE_ENV} cannot exceed {MAX_SCOPED_BENCHMARKS} benchmarks"
            ));
        }
    }
    if benchmarks.is_empty() {
        return Err(format!(
            "{BENCHMARK_SCOPE_ENV} must name at least one benchmark"
        ));
    }
    Ok(benchmarks)
}

fn phase_rank(phase: YukonSubmissionPhase) -> u8 {
    match phase {
        YukonSubmissionPhase::Running => 0,
        YukonSubmissionPhase::Queued => 1,
        YukonSubmissionPhase::Rejected => 2,
        YukonSubmissionPhase::Accepted => 3,
        YukonSubmissionPhase::Unknown => 4,
    }
}

fn run_yukon(args: &[&str], empty_failure_is_ok: bool) -> Result<String, String> {
    let mut child = Command::new("yukon")
        .args(args)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_owned()
        .map_err(|error| format!("start Yukon CLI: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "capture Yukon stdout: pipe unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "capture Yukon stderr: pipe unavailable".to_string())?;
    let stdout = drain_pipe(stdout);
    let stderr = drain_pipe(stderr);
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < COMMAND_TIMEOUT => {
                thread::sleep(CHILD_POLL_INTERVAL);
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout.join();
                let _ = stderr.join();
                return Err(format!(
                    "Yukon CLI timed out after {}s",
                    COMMAND_TIMEOUT.as_secs()
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout.join();
                let _ = stderr.join();
                return Err(format!("wait for Yukon CLI: {error}"));
            }
        }
    };
    let stdout = join_pipe(stdout, "stdout")?;
    let stderr = join_pipe(stderr, "stderr")?;
    if stdout.overflow || stderr.overflow {
        return Err("Yukon CLI output exceeded the watcher limit".to_string());
    }
    let stdout = strip_ansi(&String::from_utf8_lossy(&stdout.bytes));
    let stderr = strip_ansi(&String::from_utf8_lossy(&stderr.bytes));
    if !(status.success()
        || empty_failure_is_ok && stdout.trim().is_empty() && stderr.trim().is_empty())
    {
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(if detail.is_empty() {
            format!("Yukon CLI exited with {status}")
        } else {
            format!("Yukon CLI: {detail}")
        });
    }
    Ok(stdout)
}

#[derive(Debug)]
struct CapturedPipe {
    bytes: Vec<u8>,
    overflow: bool,
}

fn drain_pipe<R>(mut pipe: R) -> thread::JoinHandle<Result<CapturedPipe, String>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut bytes = Vec::with_capacity(MAX_CLI_OUTPUT_BYTES.min(64 * 1024));
        let mut overflow = false;
        let mut chunk = [0_u8; 8 * 1024];
        loop {
            let read = pipe
                .read(&mut chunk)
                .map_err(|error| format!("read Yukon CLI output: {error}"))?;
            if read == 0 {
                break;
            }
            let remaining = MAX_CLI_OUTPUT_BYTES.saturating_sub(bytes.len());
            let retained = read.min(remaining);
            bytes.extend_from_slice(&chunk[..retained]);
            if retained < read {
                overflow = true;
            }
        }
        Ok(CapturedPipe { bytes, overflow })
    })
}

fn join_pipe(
    handle: thread::JoinHandle<Result<CapturedPipe, String>>,
    name: &str,
) -> Result<CapturedPipe, String> {
    handle
        .join()
        .map_err(|_| format!("Yukon {name} reader panicked"))?
}

pub(crate) fn parse_benchmark_list(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            (fields.len() >= 2 && fields[0].contains('/') && fields[1].eq_ignore_ascii_case("open"))
                .then(|| fields[0].to_string())
        })
        .collect()
}

pub(crate) fn parse_submission_table(benchmark: &str, output: &str) -> Vec<YukonSubmission> {
    output
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 4 || !looks_like_submission_id(fields[0]) {
                return None;
            }
            let (status, score_index) = if fields[2].eq_ignore_ascii_case("promotion")
                && fields
                    .get(3)
                    .is_some_and(|field| field.eq_ignore_ascii_case("failed"))
            {
                ("promotion failed".to_string(), 4)
            } else {
                (fields[2].to_string(), 3)
            };
            Some(YukonSubmission {
                benchmark: benchmark.to_string(),
                id: fields[0].to_string(),
                phase: YukonSubmissionPhase::from_status(&status),
                status,
                score: fields
                    .get(score_index)
                    .filter(|score| **score != "n/a")
                    .map(|score| (*score).to_string()),
            })
        })
        .collect()
}

/// Yukon prints personal submissions oldest to newest. The fleet HUD tracks
/// the current frontier for each benchmark, not the complete submission
/// history, so one challenge contributes at most one glyph and count.
fn latest_submission(benchmark: &str, output: &str) -> Option<YukonSubmission> {
    parse_submission_table(benchmark, output).pop()
}

fn looks_like_submission_id(value: &str) -> bool {
    (7..=36).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        if chars.peek() == Some(&'[') {
            let _ = chars.next();
            for control in chars.by_ref() {
                if ('@'..='~').contains(&control) {
                    break;
                }
            }
        } else {
            let _ = chars.next();
        }
    }
    out
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/app/yukon_fleet__tests.rs"]
mod tests;
