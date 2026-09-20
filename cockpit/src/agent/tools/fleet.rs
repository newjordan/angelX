//! Fleet recon plus the opt-in `machine_test` fair remote-machine queue.
//!
//! Recon only: nothing here mutates state — no process kills, no instance
//! destroys, no service restarts. That posture is deliberate (and matches the
//! operator's standing rule: read-only fleet surfaces are always safe). The
//! tools shell out to whatever CLI the box actually has and degrade to an
//! honest "not found" message instead of failing registration, so the same
//! toolbelt works on an NVIDIA rig, an Intel Arc box, or a CPU-only laptop.

use crate::agent::club::ToolDef;
use crate::agent::harness::{
    Tool, ToolOutputProgress, ToolRegistry, env_flag, output_timed,
    output_timed_captured_cancellable_with_progress,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Run a read-only recon command with a snappy deadline, returning combined
/// stdout+stderr (capped). A missing binary or non-zero exit becomes an `Err`
/// with whatever the tool printed, so the caller can try the next probe.
fn run_recon(program: &str, args: &[&str], timeout_secs: u64) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    let (out, timed_out) = output_timed(cmd, Some(Duration::from_secs(timeout_secs)))
        .map_err(|e| format!("{program}: {e}"))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !stderr.trim().is_empty() {
        text.push_str("\n[stderr] ");
        text.push_str(stderr.trim());
    }
    if timed_out {
        return Err(format!("{program}: timed out after {timeout_secs}s"));
    }
    if !out.status.success() && text.trim().is_empty() {
        return Err(format!("{program}: exited {:?}", out.status.code()));
    }
    if !out.status.success() {
        return Err(format!("{program}: {}", text.trim()));
    }
    Ok(clip_head_tail(text.trim(), 12_000, ""))
}

/// Head/tail-clip `text` to at most `max` bytes, keeping both ends and
/// marking the elision with a truthful snipped-bytes marker.
///
/// Contract (deliberately total — no budget hangs or over-runs):
/// * always returns valid UTF-8 (every cut snaps to a char boundary);
/// * `max == 0` returns the empty string;
/// * text at or under `max` is returned unchanged;
/// * if `max` cannot hold even the shortest honest marker, the result is a
///   plain head clip of at most `max` bytes with **no** marker — never an
///   impossible or lying one;
/// * otherwise the returned string, marker included, is at most `max` bytes
///   and states exactly how many bytes were snipped. `note` is appended
///   inside the marker when non-empty.
fn clip_head_tail(text: &str, max: usize, note: &str) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let marker = |snipped: usize| {
        if note.is_empty() {
            format!("…[truncated; snipped {snipped} bytes]")
        } else {
            format!("…[truncated; snipped {snipped} bytes; {note}]")
        }
    };
    // The widest marker any candidate can need (snipped <= text.len(), so its
    // digit count is bounded by this one's).
    let marker_len = marker(text.len()).len();
    if max < marker_len {
        // No honest marker fits: plain head clip, no marker.
        let end = (0..=max.min(text.len()))
            .rev()
            .find(|&i| text.is_char_boundary(i))
            .unwrap_or(0);
        return text[..end].to_string();
    }
    let floor = |at: usize| {
        (0..=at.min(text.len()))
            .rev()
            .find(|&i| text.is_char_boundary(i))
            .unwrap_or(0)
    };
    let mut keep = max - marker_len;
    loop {
        let head_raw = keep * 3 / 4;
        let tail_raw = keep - head_raw;
        let head = floor(head_raw);
        let tail_start = floor(text.len().saturating_sub(tail_raw));
        let snipped = text.len() - head - (text.len() - tail_start);
        let candidate = format!(
            "{}{}{}",
            &text[..head],
            marker(snipped),
            &text[tail_start..]
        );
        if candidate.len() <= max || keep == 0 {
            // keep == 0 collapses to the bare marker, which fits because
            // marker(text.len()).len() <= max; so this always terminates.
            return candidate;
        }
        keep = keep.saturating_sub(candidate.len() - max);
    }
}

/// Format the nvidia-smi CSV rows (`index, name, driver, util, mem.used,
/// mem.total, temp, power.draw, power.limit`) into compact per-GPU lines.
/// Fields nvidia-smi reports as `[N/A]` (e.g. dedicated-VRAM numbers on
/// unified-memory parts like the GB10) are omitted rather than echoed.
/// Pure → testable.
pub(crate) fn format_nvidia_csv(csv: &str) -> String {
    fn known(f: &str) -> bool {
        !f.is_empty() && f != "[N/A]"
    }
    let mut lines = Vec::new();
    for row in csv.lines() {
        let f: Vec<&str> = row.split(',').map(str::trim).collect();
        if f.len() < 9 {
            continue;
        }
        let mut parts = vec![format!("GPU{} {}", f[0], f[1])];
        if known(f[3]) {
            parts.push(format!("util {}%", f[3]));
        }
        match (known(f[4]), known(f[5])) {
            (true, true) => parts.push(format!("mem {}/{} MiB", f[4], f[5])),
            (true, false) => parts.push(format!("mem {} MiB used", f[4])),
            _ => {}
        }
        if known(f[6]) {
            parts.push(format!("{}°C", f[6]));
        }
        match (known(f[7]), known(f[8])) {
            (true, true) => parts.push(format!("{}/{} W", f[7], f[8])),
            (true, false) => parts.push(format!("{} W", f[7])),
            _ => {}
        }
        if known(f[2]) {
            parts.push(format!("driver {}", f[2]));
        }
        lines.push(parts.join(" | "));
    }
    if lines.is_empty() {
        csv.trim().to_string()
    } else {
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// gpu_stat — local GPU utilization/VRAM/thermals/processes.
// ---------------------------------------------------------------------------

pub(crate) struct GpuStatTool;

impl Tool for GpuStatTool {
    fn name(&self) -> &str {
        "gpu_stat"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "gpu_stat".to_string(),
            description: "Read-only snapshot of the local GPUs: utilization, VRAM used/total, \
                          temperature, power draw, driver, and the compute processes holding \
                          VRAM. Tries nvidia-smi, then rocm-smi, then xpu-smi. Never mutates \
                          anything."
                .to_string(),
            params: serde_json::json!({ "type": "object", "properties": {} }),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        let mut attempts = Vec::new();
        match run_recon(
            "nvidia-smi",
            &[
                "--query-gpu=index,name,driver_version,utilization.gpu,memory.used,memory.total,\
                 temperature.gpu,power.draw,power.limit",
                "--format=csv,noheader,nounits",
            ],
            15,
        ) {
            Ok(csv) => {
                let mut out = format_nvidia_csv(&csv);
                if let Ok(procs) = run_recon(
                    "nvidia-smi",
                    &[
                        "--query-compute-apps=pid,process_name,used_memory",
                        "--format=csv,noheader,nounits",
                    ],
                    15,
                ) {
                    if !procs.trim().is_empty() {
                        out.push_str("\ncompute processes (pid, name, MiB):\n");
                        out.push_str(procs.trim());
                    } else {
                        out.push_str("\ncompute processes: none");
                    }
                }
                return Ok(out);
            }
            Err(e) => attempts.push(e),
        }
        match run_recon("rocm-smi", &["--showuse", "--showmemuse", "--showtemp"], 15) {
            Ok(out) => return Ok(out),
            Err(e) => attempts.push(e),
        }
        match run_recon("xpu-smi", &["stats", "-d", "0"], 15) {
            Ok(out) => return Ok(out),
            Err(e) => attempts.push(e),
        }
        Err(format!(
            "no GPU stats tool answered (tried nvidia-smi, rocm-smi, xpu-smi):\n{}",
            attempts.join("\n")
        ))
    }
}

// ---------------------------------------------------------------------------
// fleet_status — tailnet peers (the rig fleet rides on Tailscale).
// ---------------------------------------------------------------------------

pub(crate) struct FleetStatusTool;

impl Tool for FleetStatusTool {
    fn name(&self) -> &str {
        "fleet_status"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "fleet_status".to_string(),
            description: "Read-only Tailscale fleet snapshot: each peer's hostname, tailnet IP, \
                          OS, and online/offline state. Use to find which rigs are reachable \
                          before SSHing or probing an endpoint on one."
                .to_string(),
            params: serde_json::json!({ "type": "object", "properties": {} }),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        run_recon("tailscale", &["status"], 15).map_err(|e| {
            format!("fleet_status: {e}. Is tailscale installed and this box on the tailnet?")
        })
    }
}

// ---------------------------------------------------------------------------
// vast_instances — rented pods (read-only; costs money while running).
// ---------------------------------------------------------------------------

pub(crate) struct VastInstancesTool;

impl Tool for VastInstancesTool {
    fn name(&self) -> &str {
        "vast_instances"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "vast_instances".to_string(),
            description: "Read-only list of the account's Vast.ai instances (id, machine, GPU, \
                          status, $/hr). Recon only — report an idle instance, never destroy \
                          or restart one; that is the operator's call."
                .to_string(),
            params: serde_json::json!({ "type": "object", "properties": {} }),
        }
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        run_recon("vastai", &["show", "instances"], 20).map_err(|e| {
            format!("vast_instances: {e}. Needs the vastai CLI with an API key configured.")
        })
    }
}

// ---------------------------------------------------------------------------
// machine_test — fair cross-process lease for one scarce remote test box.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct MachineQueueConfig {
    client_script: PathBuf,
    host: String,
    remote_script: String,
    remote_db: String,
    resource: String,
    owner: String,
    competition: String,
    remote_cwd: String,
    wait_seconds: u64,
    lease_seconds: u64,
    quantum_seconds: u64,
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_u64(key: &str, default: u64) -> u64 {
    env_nonempty(key)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn machine_queue_client_path() -> PathBuf {
    if let Some(path) = env_nonempty("ANGEL_MACHINE_QUEUE_CLIENT") {
        return PathBuf::from(path);
    }
    crate::platform::runtime_paths::script("runtime/angel-machine-queue.py")
}

impl MachineQueueConfig {
    fn from_env(workspace: &Path) -> Option<Self> {
        let host = env_nonempty("ANGEL_MACHINE_QUEUE_HOST")?;
        let competition = env_nonempty("ANGEL_COMPETITION_ID").unwrap_or_else(|| {
            workspace
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("angel0")
                .to_string()
        });
        let owner =
            env_nonempty("ANGEL_MACHINE_QUEUE_OWNER").unwrap_or_else(|| competition.clone());
        Some(Self {
            client_script: machine_queue_client_path(),
            host,
            remote_script: env_nonempty("ANGEL_MACHINE_QUEUE_REMOTE_SCRIPT")
                .unwrap_or_else(|| ".local/bin/angel-machine-queue.py".to_string()),
            remote_db: env_nonempty("ANGEL_MACHINE_QUEUE_REMOTE_DB")
                .unwrap_or_else(|| "~/.angel0/machine-queue.sqlite3".to_string()),
            resource: env_nonempty("ANGEL_MACHINE_QUEUE_RESOURCE")
                .unwrap_or_else(|| "mac-test".to_string()),
            owner,
            competition,
            remote_cwd: env_nonempty("ANGEL_MACHINE_QUEUE_REMOTE_CWD")
                .unwrap_or_else(|| ".".to_string()),
            wait_seconds: env_u64("ANGEL_MACHINE_QUEUE_WAIT_SECS", 0),
            lease_seconds: env_u64("ANGEL_MACHINE_QUEUE_LEASE_SECS", 90).max(15),
            // A test invocation is one machine-time quantum.  On expiry the
            // process group is stopped and this owner must queue again, so a
            // broken suite cannot monopolize a rented box forever.
            quantum_seconds: env_u64("ANGEL_MACHINE_QUEUE_QUANTUM_SECS", 1_800),
        })
    }
}

pub(crate) struct MachineTestTool {
    config: MachineQueueConfig,
}

impl MachineTestTool {
    fn command(&self, args: &Value) -> Result<Command, String> {
        // `script` is a compatibility alias for the wrong-vocabulary calls
        // model authors keep sending; only used when `command` is absent.
        let test_command = args
            .get("command")
            .and_then(Value::as_str)
            .or_else(|| args.get("script").and_then(Value::as_str))
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .ok_or("missing 'command'")?;
        let competition = args
            .get("competition")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.config.competition);
        let cwd = args
            .get("cwd")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.config.remote_cwd);
        let mut command = Command::new("python3");
        command
            .arg(&self.config.client_script)
            .arg("client-run")
            .arg("--host")
            .arg(&self.config.host)
            .arg("--remote-script")
            .arg(&self.config.remote_script)
            .arg("--remote-db")
            .arg(&self.config.remote_db)
            .arg("--resource")
            .arg(&self.config.resource)
            .arg("--owner")
            .arg(&self.config.owner)
            .arg("--competition")
            .arg(competition)
            .arg("--command")
            .arg(test_command)
            .arg("--cwd")
            .arg(cwd)
            .arg("--wait-seconds")
            .arg(self.config.wait_seconds.to_string())
            .arg("--lease-seconds")
            .arg(self.config.lease_seconds.to_string())
            .arg("--max-run-seconds")
            .arg(self.config.quantum_seconds.to_string());
        Ok(command)
    }

    fn invoke(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        if !self.config.client_script.is_file() {
            return Err(format!(
                "machine queue client is missing at {}; set ANGEL_MACHINE_QUEUE_CLIENT",
                self.config.client_script.display()
            ));
        }
        let command = self.command(args)?;
        // Queue waits and remote tests are intentionally not subject to the
        // ordinary short tool deadline.  The queue quantum bounds machine
        // occupancy, while turn cancellation kills the SSH/client process
        // group and the remote broker releases its lease.
        let streamed = progress.is_some();
        let capture =
            output_timed_captured_cancellable_with_progress(command, None, cancel, progress)?;
        let mut output = String::from_utf8_lossy(&capture.output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&capture.output.stderr);
        if !stderr.trim().is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str("[stderr] ");
            output.push_str(stderr.trim());
        }
        let truncated = clip_head_tail(
            output.trim(),
            16_000,
            if streamed {
                "full output remained live in the tool stream"
            } else {
                ""
            },
        );
        if capture.cancelled {
            return Err(format!(
                "queued machine test cancelled by operator\n{truncated}"
            ));
        }
        if capture.timed_out {
            return Err(format!("queued machine test client timed out\n{truncated}"));
        }
        if !capture.output.status.success() {
            let code = capture
                .output
                .status
                .code()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "signal".to_string());
            return Err(format!(
                "queued machine test failed (exit {code})\n{truncated}"
            ));
        }
        Ok(truncated)
    }
}

impl Tool for MachineTestTool {
    fn name(&self) -> &str {
        "machine_test"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "machine_test".to_string(),
            description: format!(
                "Queue one test quantum on the shared remote machine '{}' (resource '{}'). \
                 Owners alternate round-robin; a queued caller waits without polling the model, \
                 streams the test output, heartbeats its lease, then releases and goes to the \
                 back of the line on its next call. Use one coherent test/benchmark command per \
                 call; do not background work on the remote box.",
                self.config.host, self.config.resource
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "One shell test or benchmark command to execute on the remote machine"
                    },
                    "competition": {
                        "type": "string",
                        "description": "Receipt label; defaults to ANGEL_COMPETITION_ID"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Existing working directory on the remote machine"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        self.invoke(args, None, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        self.invoke(args, cancel, None)
    }

    fn call_with_cancel_and_progress(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        self.invoke(args, cancel, progress)
    }
}

/// Register the read-only fleet recon tools, gated on `ANGEL_FLEET_TOOLS`
/// (default on). Missing CLIs surface as call-time messages, not
/// registration failures — the catalog stays stable across heterogeneous rigs.
pub(crate) fn maybe_register_fleet_tools(r: &mut ToolRegistry) {
    if env_flag("ANGEL_FLEET_TOOLS", true) {
        r.register(Box::new(GpuStatTool));
        r.register(Box::new(FleetStatusTool));
        r.register(Box::new(VastInstancesTool));
    }
    if let Some(config) = MachineQueueConfig::from_env(r.current_workspace()) {
        r.register_deferred(Box::new(MachineTestTool { config }));
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/fleet__tests.rs"]
mod tests;
