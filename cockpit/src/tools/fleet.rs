//! Fleet recon plus the opt-in `machine_test` fair remote-machine queue.
//!
//! Recon only: nothing here mutates state — no process kills, no instance
//! destroys, no service restarts. That posture is deliberate (and matches the
//! operator's standing rule: read-only fleet surfaces are always safe). The
//! tools shell out to whatever CLI the box actually has and degrade to an
//! honest "not found" message instead of failing registration, so the same
//! toolbelt works on an NVIDIA rig, an Intel Arc box, or a CPU-only laptop.

use crate::club::ToolDef;
use crate::harness::{
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
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap_or(&manifest)
        .join("scripts")
        .join("angel-machine-queue.py")
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
mod tests {
    use super::*;

    #[test]
    fn nvidia_csv_formats_per_gpu_lines() {
        let csv = "0, NVIDIA RTX PRO 6000, 590.12, 87, 44231, 49140, 61, 310.5, 450.0\n\
                   1, NVIDIA RTX PRO 6000, 590.12, 12, 1024, 49140, 41, 90.0, 450.0";
        let out = format_nvidia_csv(csv);
        assert!(out.contains("GPU0 NVIDIA RTX PRO 6000 | util 87% | mem 44231/49140 MiB"));
        assert!(out.contains("GPU1"));
        assert!(out.contains("driver 590.12"));
    }

    #[test]
    fn nvidia_csv_omits_na_fields_on_unified_memory_parts() {
        // Real GB10 (DGX Spark) output: no dedicated VRAM or power limit.
        let csv = "0, NVIDIA GB10, 580.159.03, 0, [N/A], [N/A], 61, 21.41, [N/A]";
        let out = format_nvidia_csv(csv);
        assert_eq!(
            out,
            "GPU0 NVIDIA GB10 | util 0% | 61°C | 21.41 W | driver 580.159.03"
        );
    }

    #[test]
    fn nvidia_csv_passes_through_unparseable_text() {
        assert_eq!(format_nvidia_csv("weird output"), "weird output");
    }

    #[test]
    fn clip_head_tail_total_contract() {
        // Small text untouched.
        assert_eq!(clip_head_tail("hello", 100, ""), "hello");
        // max = 0 terminates with empty output.
        assert_eq!(clip_head_tail("abc", 0, ""), "");
        // Keep the input larger than every tested budget so marker assertions
        // exercise clipping, while the small-text assertion covers passthrough.
        let text = format!("h{}t", "…".repeat(4000));
        let marker_len = format!("…[truncated; snipped {} bytes]", text.len()).len();
        for max in 0..=marker_len + 80 {
            let out = clip_head_tail(&text, max, "");
            assert!(out.len() <= max, "max={max} len={}", out.len());
            if max < marker_len {
                assert!(!out.contains("…[truncated"));
                assert!(text.starts_with(&out));
            } else {
                let (head, rest) = out.split_once("…[truncated; snipped ").unwrap();
                let (count, tail) = rest.split_once(" bytes]").unwrap();
                assert!(text.starts_with(head) && text.ends_with(tail));
                assert_eq!(
                    count.parse::<usize>().unwrap(),
                    text.len() - head.len() - tail.len()
                );
            }
        }
        // 2/3/4-byte codepoints cut at every boundary: valid UTF-8, both ends kept.
        for cp in ["é", "…", "🦀"] {
            let big = cp.repeat(4000);
            let out = clip_head_tail(&big, 1000, "");
            assert!(std::str::from_utf8(out.as_bytes()).is_ok());
            assert!(out.len() <= 1000);
            assert!(out.starts_with(cp) && out.ends_with(cp));
            assert!(out.contains("snipped "));
        }
        // Note surfaces inside the marker.
        let out = clip_head_tail(&"a".repeat(50_000), 16_000, "full output remained live");
        assert!(out.len() <= 16_000);
        assert!(out.contains("full output remained live"));
    }

    #[test]
    fn clip_head_tail_never_introduces_replacements_after_lossy() {
        for b in [0u8, 0x80, 0xC3, 0xE2, 0xF0, 0xFF] {
            let raw = vec![b'a', b, b'z', b, b'a'];
            let text = String::from_utf8_lossy(&raw).into_owned();
            let expect = text.matches('\u{FFFD}').count();
            for max in [1usize, 2, 3, 4, 8, 20, 60, 200] {
                let out = clip_head_tail(&text, max, "");
                assert!(std::str::from_utf8(out.as_bytes()).is_ok());
                assert_eq!(
                    out.matches('\u{FFFD}').count(),
                    expect.min(out.len() / 3),
                    "max={max}"
                );
            }
        }
    }

    #[test]
    fn run_recon_truncates_with_marker() {
        // Focused production behavior: large recon output is head/tail clipped
        // and marked, small output passes through untouched.
        let big = run_recon("printf", &[&"x".repeat(20_000)], 15).unwrap();
        assert!(big.len() <= 12_000);
        assert!(big.contains("…[truncated; snipped "));
        assert!(big.starts_with("xxxx") && big.ends_with("xxxx"));
        let small = run_recon("printf", &["ok"], 15).unwrap();
        assert_eq!(small, "ok");
        let missing = run_recon("definitely-not-a-binary-34", &[], 15);
        assert!(missing.is_err());
    }

    #[test]
    fn machine_test_clipping_preserves_streaming_and_exit_status() {
        let _guard = crate::tests::env_lock();
        use crate::harness::ProcessStream;
        use std::path::PathBuf;
        use std::sync::Mutex;

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/machine_queue_output.py");
        assert!(path.is_file(), "missing fake client: {}", path.display());
        let config = MachineQueueConfig {
            client_script: path,
            host: "local-fake".into(),
            remote_script: "unused".into(),
            remote_db: "unused".into(),
            resource: "fake".into(),
            owner: "review".into(),
            competition: "review".into(),
            remote_cwd: ".".into(),
            wait_seconds: 0,
            lease_seconds: 15,
            quantum_seconds: 15,
        };
        let tool = MachineTestTool { config };
        let args = serde_json::json!({"command": "ok"});
        let plain = tool.call(&args).unwrap();
        assert!(plain.len() <= 16000);
        assert!(plain.contains('O') && plain.contains('E'));
        assert!(plain.contains("…[truncated; snipped "));
        assert!(!plain.contains("full output remained live"));

        let chunks = Arc::new(Mutex::new(Vec::<(ProcessStream, usize)>::new()));
        let seen = Arc::clone(&chunks);
        let progress: Arc<ToolOutputProgress> = Arc::new(move |stream, bytes| {
            seen.lock().unwrap().push((stream, bytes.len()));
        });
        let streamed = tool
            .call_with_cancel_and_progress(&args, None, Some(progress))
            .unwrap();
        assert!(streamed.contains("full output remained live"));
        let seen = chunks.lock().unwrap();
        assert!(
            seen.iter()
                .any(|(s, n)| *s == ProcessStream::Stdout && *n > 0)
        );
        assert!(
            seen.iter()
                .any(|(s, n)| *s == ProcessStream::Stderr && *n > 0)
        );

        let error = tool
            .call(&serde_json::json!({"command": "fail"}))
            .unwrap_err();
        assert!(error.contains("queued machine test failed (exit 7)"));
        assert!(error.contains('E'));
        assert!(!error.contains("full output remained live"));
    }

    #[test]
    fn machine_queue_is_opt_in_and_uses_competition_as_fair_owner() {
        let _guard = crate::tests::env_lock();
        let _host = crate::tests::TestEnvGuard::set("ANGEL_MACHINE_QUEUE_HOST", "mac-builder");
        let _competition = crate::tests::TestEnvGuard::set("ANGEL_COMPETITION_ID", "kernel-race");
        let _owner = crate::tests::TestEnvGuard::unset("ANGEL_MACHINE_QUEUE_OWNER");
        let config = MachineQueueConfig::from_env(Path::new("/tmp/project")).unwrap();
        assert_eq!(config.host, "mac-builder");
        assert_eq!(config.competition, "kernel-race");
        assert_eq!(config.owner, "kernel-race");
        assert_eq!(config.quantum_seconds, 1_800);
    }

    #[test]
    #[ignore = "reads the live box (nvidia-smi / tailscale); run with --ignored"]
    fn live_gpu_stat_and_fleet_status() {
        let gpu = GpuStatTool.call(&serde_json::json!({}));
        println!("gpu_stat →\n{gpu:?}\n");
        let fleet = FleetStatusTool.call(&serde_json::json!({}));
        println!("fleet_status →\n{fleet:?}\n");
        assert!(
            gpu.is_ok() || fleet.is_ok(),
            "neither GPU nor tailnet recon answered on this box"
        );
    }
}
