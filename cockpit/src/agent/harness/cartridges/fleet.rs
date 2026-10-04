//! Competition fleet telemetry for the loop HUD: the latest submission per
//! open benchmark. The active cartridge sweeps its board off the UI thread;
//! `App::advance` schedules the next sweep only after the last one settles,
//! so a slow board CLI never blocks or piles up behind the TUI.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Between sweeps, unless the cartridge sets `fleet_interval_secs`.
pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubmissionPhase {
    Queued,
    Running,
    Accepted,
    Rejected,
    TimedOut,
    Unknown,
}

impl SubmissionPhase {
    pub(crate) fn from_status(status: &str) -> Self {
        match status.trim().to_ascii_lowercase().as_str() {
            "queued" | "pending" | "submitted" => Self::Queued,
            "validating" | "running" | "in-flight" | "in_flight" => Self::Running,
            "accepted" | "promoted" | "complete" | "completed" => Self::Accepted,
            "timeout" | "timed_out" | "timed-out" => Self::TimedOut,
            "rejected" | "failed" | "promotion failed" | "error" | "cancelled" | "canceled" => {
                Self::Rejected
            }
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FleetSubmission {
    pub(crate) benchmark: String,
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) score: Option<String>,
    pub(crate) phase: SubmissionPhase,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FleetSnapshot {
    pub(crate) entries: Vec<FleetSubmission>,
    pub(crate) benchmark_count: usize,
    pub(crate) failed_benchmarks: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FleetState {
    entries: Vec<FleetSubmission>,
    benchmark_count: usize,
    scanning: bool,
    stale: bool,
    error: Option<String>,
}

impl FleetState {
    pub(crate) fn entries(&self) -> &[FleetSubmission] {
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

    pub(crate) fn apply(&mut self, snapshot: FleetSnapshot) {
        self.entries = snapshot.entries;
        self.benchmark_count = snapshot.benchmark_count;
        self.scanning = false;
        self.stale = snapshot.failed_benchmarks > 0;
        self.error = (snapshot.failed_benchmarks > 0).then(|| {
            format!(
                "{} benchmark status quer{} failed",
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


/// Start one sweep of `cartridge`'s fleet on its own thread.
pub(crate) fn spawn_poll(
    cartridge: &'static super::Cartridge,
) -> mpsc::Receiver<Result<FleetSnapshot, String>> {
    let (tx, rx) = mpsc::sync_channel(1);
    let worker_tx = tx.clone();
    let spawn = thread::Builder::new()
        .name("angel-comp-fleet".to_string())
        .spawn(move || {
            let _ = worker_tx.send(cartridge.fleet_sweep());
        });
    if let Err(error) = spawn {
        let _ = tx.send(Err(format!("start {} fleet watcher: {error}", cartridge.label())));
    }
    rx
}

/// A config-only cartridge's sweep: `command`'s stdout is one snapshot. It
/// runs in the session's directory with `ANGEL_CARTRIDGE_DIR` naming the
/// cartridge's folder.
pub(crate) fn sweep_command(
    command: &str,
    folder: Option<&std::path::Path>,
) -> Result<FleetSnapshot, String> {
    #[derive(serde::Deserialize)]
    struct Row {
        benchmark: String,
        id: String,
        status: String,
        #[serde(default)]
        score: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Sweep {
        #[serde(default)]
        benchmarks: usize,
        #[serde(default)]
        failed: usize,
        #[serde(default)]
        submissions: Vec<Row>,
    }
    let mut process = std::process::Command::new("bash");
    process.arg("-c").arg(command).stdin(std::process::Stdio::null());
    if let Some(folder) = folder {
        process.env("ANGEL_CARTRIDGE_DIR", folder);
    }
    let output = process
        .output()
        .map_err(|e| format!("fleet command: {e}"))?;
    if !output.status.success() {
        return Err(format!("fleet command exited {}", output.status));
    }
    let sweep: Sweep = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("fleet command printed no snapshot: {e}"))?;
    Ok(FleetSnapshot {
        entries: sweep
            .submissions
            .into_iter()
            .map(|row| FleetSubmission {
                phase: SubmissionPhase::from_status(&row.status),
                benchmark: row.benchmark,
                id: row.id,
                status: row.status,
                score: row.score,
            })
            .collect(),
        benchmark_count: sweep.benchmarks,
        failed_benchmarks: sweep.failed,
    })
}
