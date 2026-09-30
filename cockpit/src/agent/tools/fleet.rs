//! Fleet recon.
//!
//! Recon only: nothing here mutates state — no process kills, no instance
//! destroys, no service restarts. That posture is deliberate (and matches the
//! operator's standing rule: read-only fleet surfaces are always safe). The
//! tools shell out to whatever CLI the box actually has and degrade to an
//! honest "not found" message instead of failing registration, so the same
//! toolbelt works on an NVIDIA rig, an Intel Arc box, or a CPU-only laptop.

use crate::agent::club::ToolDef;
use crate::agent::harness::{Tool, ToolRegistry, env_flag, output_timed};
use serde_json::Value;
use std::process::Command;
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
/// unified-memory parts) are omitted rather than echoed.
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
                          OS, and online/offline state. ⠱⠛"
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
                          status, $/hr). ⠱⠓"
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

/// Register the read-only fleet recon tools, gated on `ANGEL_FLEET_TOOLS`
/// (default on). Missing CLIs surface as call-time messages, not
/// registration failures — the catalog stays stable across heterogeneous rigs.
pub(crate) fn maybe_register_fleet_tools(r: &mut ToolRegistry) {
    if env_flag("ANGEL_FLEET_TOOLS", true) {
        r.register(Box::new(GpuStatTool));
        r.register(Box::new(FleetStatusTool));
        r.register(Box::new(VastInstancesTool));
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/fleet__tests.rs"]
mod tests;
