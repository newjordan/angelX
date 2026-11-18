//! Tool hand-off: a foreground shell call that runs past one fixed limit is
//! handed to the background-job table (`proc`) instead of being waited on. The
//! process keeps running and nothing is stopped; its exit channel moves to the
//! job, its output keeps draining into the job's log, and the model gets its
//! turn back with a receipt (`⡌⠙⠏⠙`) carrying the job id and what the process
//! is doing, which the local helper then reads (`compactor::start_handoff`).
//!
//! Only the shell tool arms it, for one sandboxed call at a time
//! ([`with_handoff`] → [`armed`]), so internal probes and verifiers keep their
//! own contracts. `ANGEL_TOOL_HANDOFF_SECS` sets the limit (default
//! [`HANDOFF_SECS`]); `0` turns hand-off off.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) const HANDOFF_SECS: u64 = 600;

const RECENT_BYTES: usize = 64 * 1024;
const RECEIPT_TAIL_BYTES: usize = 1500;
const RECEIPT_TREE_ROWS: usize = 8;

#[derive(Clone, Debug)]
pub(crate) struct Handoff {
    pub(crate) after: Duration,
    pub(crate) workspace: PathBuf,
    pub(crate) command: String,
}

thread_local! {
    static PENDING: std::cell::RefCell<Option<Handoff>> = const { std::cell::RefCell::new(None) };
    static ARMED: std::cell::RefCell<Option<Handoff>> = const { std::cell::RefCell::new(None) };
}

pub(crate) fn tool_handoff_after() -> Option<Duration> {
    let secs = std::env::var("ANGEL_TOOL_HANDOFF_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(HANDOFF_SECS);
    (secs > 0).then(|| Duration::from_secs(secs))
}

pub(crate) fn with_handoff<T>(spec: Option<Handoff>, f: impl FnOnce() -> T) -> T {
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            PENDING.with(|cell| cell.borrow_mut().take());
        }
    }
    PENDING.with(|cell| *cell.borrow_mut() = spec);
    let _clear = Clear;
    f()
}

pub(crate) fn take_pending() -> Option<Handoff> {
    PENDING.with(|cell| cell.borrow_mut().take())
}

pub(crate) fn armed<T>(spec: Option<Handoff>, f: impl FnOnce() -> T) -> T {
    struct Disarm;
    impl Drop for Disarm {
        fn drop(&mut self) {
            ARMED.with(|cell| cell.borrow_mut().take());
        }
    }
    ARMED.with(|cell| *cell.borrow_mut() = spec);
    let _disarm = Disarm;
    f()
}

pub(crate) fn take_armed() -> Option<Handoff> {
    ARMED.with(|cell| cell.borrow_mut().take())
}

pub(crate) type Sink = Arc<dyn Fn(&[u8]) + Send + Sync>;

#[derive(Default)]
pub(crate) struct Tap {
    state: Mutex<TapState>,
}

#[derive(Default)]
struct TapState {
    recent: VecDeque<u8>,
    sink: Option<Sink>,
}

impl std::fmt::Debug for Tap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tap")
    }
}

impl Tap {
    pub(crate) fn feed(&self, bytes: &[u8]) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(sink) = &state.sink {
            sink(bytes);
            return;
        }
        state.recent.extend(bytes.iter().copied());
        let overflow = state.recent.len().saturating_sub(RECENT_BYTES);
        if overflow > 0 {
            state.recent.drain(..overflow);
        }
    }

    pub(crate) fn hand_off<T, E>(
        &self,
        adopt: impl FnOnce(&[u8]) -> Result<(T, Sink), E>,
    ) -> Result<(T, Vec<u8>), E> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let recent: Vec<u8> = state.recent.iter().copied().collect();
        let (job, sink) = adopt(&recent)?;
        state.sink = Some(sink);
        state.recent.clear();
        Ok((job, recent))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HandedOff {
    pub(crate) id: u64,
    pub(crate) pid: u32,
    pub(crate) command: String,
    pub(crate) ran: Duration,
    pub(crate) silent: Option<Duration>,
    pub(crate) tree: Vec<String>,
    pub(crate) tail: String,
}

impl HandedOff {
    pub(crate) fn new(
        id: u64,
        pid: u32,
        command: &str,
        ran: Duration,
        silent: Option<Duration>,
        recent: &[u8],
    ) -> Self {
        let text = String::from_utf8_lossy(recent);
        let start = text.len().saturating_sub(RECEIPT_TAIL_BYTES);
        let start = (start..text.len())
            .find(|&at| text.is_char_boundary(at))
            .unwrap_or(text.len());
        Self {
            id,
            pid,
            command: command.to_string(),
            ran,
            silent,
            tree: process_tree(pid),
            tail: text[start..].trim().to_string(),
        }
    }

    pub(crate) fn receipt(&self) -> String {
        let silence = match self.silent {
            Some(silent) => format!(", no output {}s", silent.as_secs()),
            None => ", no output ever".to_string(),
        };
        let mut command = self.command.trim().to_string();
        crate::agent::harness::truncate_to_char_boundary(&mut command, 400);
        let mut receipt = format!(
            "{}{} pid {} ran {}s{silence}]\n$ {command}",
            crate::agent::harness::compactor::HANDOFF_RECEIPT_MARK,
            self.id,
            self.pid,
            self.ran.as_secs(),
        );
        for row in &self.tree {
            receipt.push('\n');
            receipt.push_str(row);
        }
        if !self.tail.is_empty() {
            receipt.push('\n');
            receipt.push_str(&self.tail);
        }
        receipt.push('\n');
        receipt.push_str(&crate::agent::harness::book::st_connected::HANDOFF.cells());
        receipt.push_str(&crate::agent::harness::book::p_processes::PROC_STARTED.cells());
        receipt
    }
}

#[cfg(target_os = "linux")]
fn process_tree(leader: u32) -> Vec<String> {
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;
    let uptime = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|text| text.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.0);
    super::owned_process_activity(leader)
        .into_iter()
        .take(RECEIPT_TREE_ROWS)
        .map(|(pid, row)| {
            let alive = (uptime - row.started as f64 / ticks).max(0.001);
            let cpu = (row.cpu as f64 / ticks / alive * 100.0).round() as u64;
            let mut line = std::fs::read(format!("/proc/{pid}/cmdline"))
                .map(|raw| {
                    String::from_utf8_lossy(&raw)
                        .replace('\0', " ")
                        .trim()
                        .to_string()
                })
                .unwrap_or_default();
            crate::agent::harness::truncate_to_char_boundary(&mut line, 160);
            format!("  {pid} {} cpu {cpu}% {line}", row.state)
        })
        .collect()
}

#[cfg(not(target_os = "linux"))]
fn process_tree(_leader: u32) -> Vec<String> {
    Vec::new()
}
