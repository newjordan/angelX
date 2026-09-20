use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const OVERWATCH_INTERVAL: Duration = Duration::from_millis(900);
const GPU_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const FLEET_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default)]
struct CpuTick {
    busy: u64,
    total: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct OverwatchSnapshot {
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub gpu_pct: f32,
    pub gpu_mem_pct: f32,
    pub fleet_cpu_pct: Option<f32>,
    pub fleet_gpu_pct: Option<f32>,
}

impl Default for OverwatchSnapshot {
    fn default() -> Self {
        Self {
            cpu_pct: 0.0,
            mem_pct: 0.0,
            gpu_pct: 0.0,
            gpu_mem_pct: 0.0,
            fleet_cpu_pct: None,
            fleet_gpu_pct: None,
        }
    }
}

impl OverwatchSnapshot {
    pub fn load_pct(self) -> f32 {
        self.cpu_pct
            .max(self.gpu_pct)
            .max(self.fleet_cpu_pct.unwrap_or(0.0))
            .max(self.fleet_gpu_pct.unwrap_or(0.0))
            .max(self.mem_pct * 0.55)
            .clamp(0.0, 100.0)
    }
}

#[derive(Clone, Copy, Debug)]
struct FleetOverwatchSample {
    cpu_pct: f32,
    gpu_pct: f32,
}

#[derive(Debug)]
pub struct Overwatch {
    pub snapshot: OverwatchSnapshot,
    last_sample: Instant,
    prev_cpu: Option<CpuTick>,
    fleet_tx: Option<mpsc::Sender<Option<FleetOverwatchSample>>>,
    fleet_rx: Option<mpsc::Receiver<Option<FleetOverwatchSample>>>,
    fleet_poll_started: Instant,
    fleet_inflight: bool,
    /// Latched when the overwatch binary is absent so a missing install costs
    /// one stat(), not one per frame for the rest of the session.
    fleet_missing: bool,
    gpu_tx: Option<mpsc::Sender<Option<(f32, f32)>>>,
    gpu_rx: Option<mpsc::Receiver<Option<(f32, f32)>>>,
    gpu_inflight: bool,
    enabled: bool,
}

impl Overwatch {
    pub fn new() -> Self {
        let (fleet_tx, fleet_rx) = mpsc::channel();
        let (gpu_tx, gpu_rx) = mpsc::channel();
        Self {
            snapshot: OverwatchSnapshot::default(),
            last_sample: Instant::now(),
            prev_cpu: None,
            fleet_tx: Some(fleet_tx),
            fleet_rx: Some(fleet_rx),
            fleet_poll_started: Instant::now() - Duration::from_secs(60),
            fleet_inflight: false,
            fleet_missing: false,
            gpu_tx: Some(gpu_tx),
            gpu_rx: Some(gpu_rx),
            gpu_inflight: false,
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        Self {
            snapshot: OverwatchSnapshot::default(),
            last_sample: Instant::now(),
            prev_cpu: None,
            fleet_tx: None,
            fleet_rx: None,
            fleet_poll_started: Instant::now(),
            fleet_inflight: false,
            fleet_missing: false,
            gpu_tx: None,
            gpu_rx: None,
            gpu_inflight: false,
            enabled: false,
        }
    }

    pub fn refresh(&mut self) {
        if !self.enabled {
            return;
        }
        if let Some(rx) = &self.fleet_rx {
            while let Ok(sample) = rx.try_recv() {
                self.fleet_inflight = false;
                if let Some(sample) = sample {
                    self.snapshot.fleet_cpu_pct = Some(sample.cpu_pct);
                    self.snapshot.fleet_gpu_pct = Some(sample.gpu_pct);
                }
            }
        }
        self.maybe_poll_fleet();

        if self.last_sample.elapsed() < OVERWATCH_INTERVAL {
            return;
        }
        self.last_sample = Instant::now();

        if let Some(next) = read_cpu_tick() {
            if let Some(prev) = self.prev_cpu {
                let busy = next.busy.saturating_sub(prev.busy) as f32;
                let total = next.total.saturating_sub(prev.total) as f32;
                if total > 0.0 {
                    self.snapshot.cpu_pct = (busy / total * 100.0).clamp(0.0, 100.0);
                }
            } else if let Some(load) = read_load_pct() {
                self.snapshot.cpu_pct = load;
            }
            self.prev_cpu = Some(next);
        }
        if let Some(mem) = read_mem_pct() {
            self.snapshot.mem_pct = mem;
        }
        // GPU sampling shells out to nvidia-smi (20-200ms) — that stall belongs
        // on a worker thread, not the render thread. Same drain-then-respawn
        // shape as the fleet poll above.
        if let Some(rx) = &self.gpu_rx {
            while let Ok(sample) = rx.try_recv() {
                self.gpu_inflight = false;
                if let Some((gpu, gpu_mem)) = sample {
                    self.snapshot.gpu_pct = gpu;
                    self.snapshot.gpu_mem_pct = gpu_mem;
                }
            }
        }
        if !self.gpu_inflight
            && overwatch_gpu_enabled()
            && let Some(tx) = self.gpu_tx.clone()
        {
            self.gpu_inflight = true;
            std::thread::spawn(move || {
                let _ = tx.send(read_gpu_pct());
            });
        }
    }

    fn maybe_poll_fleet(&mut self) {
        if cfg!(test)
            || self.fleet_inflight
            || self.fleet_missing
            || self.fleet_poll_started.elapsed() < Duration::from_secs(30)
        {
            return;
        }
        let cmd = overwatch_cmd();
        if !Path::new(cmd).exists() {
            self.fleet_missing = true;
            return;
        }
        self.fleet_inflight = true;
        self.fleet_poll_started = Instant::now();
        let Some(tx) = self.fleet_tx.clone() else {
            return;
        };
        let cmd = cmd.to_string();
        std::thread::spawn(move || {
            let sample = read_fleet_overwatch(&cmd);
            let _ = tx.send(sample);
        });
    }
}

/// A7: `refresh` / `read_gpu_pct` used to re-read `ANGEL_OVERWATCH_GPU` on every
/// sample interval. Launch config — cache in production; tests re-read.
fn overwatch_gpu_enabled() -> bool {
    #[cfg(not(test))]
    {
        static ON: OnceLock<bool> = OnceLock::new();
        *ON.get_or_init(|| std::env::var_os("ANGEL_OVERWATCH_GPU").is_some())
    }
    #[cfg(test)]
    std::env::var_os("ANGEL_OVERWATCH_GPU").is_some()
}

/// Fleet binary path (launch config): `ANGEL_OVERWATCH_CMD`, else the per-user
/// install `$HOME/.local/bin/overwatch`, else a bare `overwatch` PATH lookup.
/// Cached so a missing install only hits the filesystem when the 30s poll
/// fires, not with a fresh env lookup. Under test the fleet poll is disabled
/// (`maybe_poll_fleet`), so no test ever resolves or spawns a real install.
fn overwatch_cmd() -> &'static str {
    static CMD: OnceLock<String> = OnceLock::new();
    CMD.get_or_init(|| {
        overwatch_cmd_from(
            std::env::var("ANGEL_OVERWATCH_CMD").ok(),
            std::env::var_os("HOME"),
        )
    })
    .as_str()
}

/// Pure resolution of the fleet binary path from the two launch inputs; the
/// source carries no operator home directory.
fn overwatch_cmd_from(explicit: Option<String>, home: Option<std::ffi::OsString>) -> String {
    if let Some(cmd) = explicit
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
    {
        return cmd;
    }
    match home.filter(|h| !h.is_empty()) {
        Some(home) => Path::new(&home)
            .join(".local")
            .join("bin")
            .join("overwatch")
            .to_string_lossy()
            .into_owned(),
        None => "overwatch".to_string(),
    }
}

fn read_cpu_tick() -> Option<CpuTick> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().next()?;
    let mut nums = line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse::<u64>().ok());
    let user = nums.next()?;
    let nice = nums.next()?;
    let system = nums.next()?;
    let idle = nums.next()?;
    let iowait = nums.next().unwrap_or(0);
    let irq = nums.next().unwrap_or(0);
    let softirq = nums.next().unwrap_or(0);
    let steal = nums.next().unwrap_or(0);
    let idle_all = idle + iowait;
    let busy = user + nice + system + irq + softirq + steal;
    Some(CpuTick {
        busy,
        total: busy + idle_all,
    })
}

fn read_mem_pct() -> Option<f32> {
    let mem = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = None;
    let mut available = None;
    for line in mem.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("MemTotal:") => total = parts.next().and_then(|s| s.parse::<f32>().ok()),
            Some("MemAvailable:") => available = parts.next().and_then(|s| s.parse::<f32>().ok()),
            _ => {}
        }
    }
    let total = total?;
    let available = available?;
    if total <= 0.0 {
        return None;
    }
    Some(((total - available) / total * 100.0).clamp(0.0, 100.0))
}

fn read_load_pct() -> Option<f32> {
    let load = std::fs::read_to_string("/proc/loadavg").ok()?;
    let one_min = load.split_whitespace().next()?.parse::<f32>().ok()?;
    let cpus = std::thread::available_parallelism().ok()?.get() as f32;
    if cpus <= 0.0 {
        return None;
    }
    Some((one_min / cpus * 100.0).clamp(0.0, 100.0))
}

fn read_gpu_pct() -> Option<(f32, f32)> {
    if !overwatch_gpu_enabled() {
        return None;
    }
    let mut command = Command::new("nvidia-smi");
    command.args([
        "--query-gpu=utilization.gpu,memory.used,memory.total",
        "--format=csv,noheader,nounits",
    ]);
    let stdout = bounded_overwatch_stdout(command, GPU_PROBE_TIMEOUT)?;
    let stdout = String::from_utf8(stdout).ok()?;
    let mut gpu_peak = 0.0_f32;
    let mut mem_used = 0.0_f32;
    let mut mem_total = 0.0_f32;
    for line in stdout.lines() {
        let cols: Vec<f32> = line
            .split(',')
            .filter_map(|part| part.trim().parse::<f32>().ok())
            .collect();
        if cols.len() >= 3 {
            gpu_peak = gpu_peak.max(cols[0]);
            mem_used += cols[1];
            mem_total += cols[2];
        }
    }
    let mem_pct = if mem_total > 0.0 {
        (mem_used / mem_total * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    Some((gpu_peak.clamp(0.0, 100.0), mem_pct))
}

fn read_fleet_overwatch(cmd: &str) -> Option<FleetOverwatchSample> {
    let mut command = Command::new(cmd);
    command.args(["--once", "--only", "fleet"]);
    fleet_overwatch_from_command(command, FLEET_PROBE_TIMEOUT)
}

fn fleet_overwatch_from_command(
    command: Command,
    timeout: Duration,
) -> Option<FleetOverwatchSample> {
    let stdout = bounded_overwatch_stdout(command, timeout)?;
    parse_fleet_overwatch_totals(&String::from_utf8_lossy(&stdout))
}

fn bounded_overwatch_stdout(command: Command, timeout: Duration) -> Option<Vec<u8>> {
    let captured = crate::agent::harness::output_timed_fixed_captured(command, timeout).ok()?;
    (!captured.timed_out
        && captured.output.status.success()
        && !captured.stdout_truncated
        && !captured.stderr_truncated)
        .then_some(captured.output.stdout)
}

fn parse_fleet_overwatch_totals(text: &str) -> Option<FleetOverwatchSample> {
    for line in text.lines().rev() {
        if !line.contains("FLEET") || !line.contains("GPU") || !line.contains("CPU") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let mut gpu = None;
        let mut cpu = None;
        for window in parts.windows(2) {
            match window[0] {
                "GPU" => gpu = parse_pct(window[1]),
                "CPU" => cpu = parse_pct(window[1]),
                _ => {}
            }
        }
        if let (Some(gpu_pct), Some(cpu_pct)) = (gpu, cpu) {
            return Some(FleetOverwatchSample { cpu_pct, gpu_pct });
        }
    }
    None
}

fn parse_pct(raw: &str) -> Option<f32> {
    raw.trim_end_matches('%').parse::<f32>().ok()
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/overwatch__tests.rs"]
mod tests;
