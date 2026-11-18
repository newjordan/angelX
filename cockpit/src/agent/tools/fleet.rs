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
    let marker_len = marker(text.len()).len();
    if max < marker_len {
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
            return candidate;
        }
        keep = keep.saturating_sub(candidate.len() - max);
    }
}

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

pub(crate) struct GpuStatTool;

impl Tool for GpuStatTool {
    fn name(&self) -> &str {
        "gpu_stat"
    }
    fn def(&self) -> ToolDef {
        crate::agent::harness::core_catalog::baseline_def("gpu_stat")
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

pub(crate) struct FleetStatusTool;

impl Tool for FleetStatusTool {
    fn name(&self) -> &str {
        "fleet_status"
    }
    fn def(&self) -> ToolDef {
        crate::agent::harness::core_catalog::baseline_def("fleet_status")
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        run_recon("tailscale", &["status"], 15).map_err(|e| {
            format!("fleet_status: {e}. Is tailscale installed and this box on the tailnet?")
        })
    }
}

pub(crate) struct VastInstancesTool;

impl Tool for VastInstancesTool {
    fn name(&self) -> &str {
        "vast_instances"
    }
    fn def(&self) -> ToolDef {
        crate::agent::harness::core_catalog::baseline_def("vast_instances")
    }
    fn call(&self, _args: &Value) -> Result<String, String> {
        run_recon("vastai", &["show", "instances"], 20).map_err(|e| {
            format!("vast_instances: {e}. Needs the vastai CLI with an API key configured.")
        })
    }
}

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
