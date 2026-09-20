//! Turn-owned child activity. Registration lifetime is the foreground wait;
//! neither a stale PID nor another turn's output can keep this turn alive.
use super::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

struct ChildState {
    progress: Option<Instant>,
    setup_until: Option<Instant>,
    started: Instant,
    program: String,
    output: Option<Instant>,
    cpu: Option<Instant>,
}

/// Display-only observations. CPU activity is liveness, never a verifier pass
/// or a completion estimate. Snapshots do no filesystem work on the UI thread.
#[derive(Clone, Debug)]
pub(crate) struct ChildSnapshot {
    pub(crate) program: String,
    pub(crate) elapsed_secs: u64,
    pub(crate) output_age_secs: Option<u64>,
    pub(crate) cpu_age_secs: Option<u64>,
    pub(crate) setting_up: bool,
    pub(crate) workers: usize,
}

pub(crate) fn owned_child_snapshot(owner: usize) -> Option<ChildSnapshot> {
    let owner = root_owner(owner);
    let entries = entries().lock().unwrap_or_else(|e| e.into_inner());
    let mut owned = entries.iter().filter(|((key, _), _)| *key == owner);
    let (_, mut latest) = owned.next()?;
    let mut workers = 1;
    for (_, state) in owned {
        workers += 1;
        if (state.progress, state.started) > (latest.progress, latest.started) {
            latest = state;
        }
    }
    Some(ChildSnapshot {
        program: latest.program.clone(),
        elapsed_secs: latest.started.elapsed().as_secs(),
        output_age_secs: latest.output.map(|at| at.elapsed().as_secs()),
        cpu_age_secs: latest.cpu.map(|at| at.elapsed().as_secs()),
        setting_up: latest
            .setup_until
            .is_some_and(|until| Instant::now() < until),
        workers,
    })
}

fn process_name(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|name| {
            name.chars()
                .filter(|c| c.is_ascii_alphanumeric() || "-_.".contains(*c))
                .take(32)
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "process".into())
}

type Entries = HashMap<(usize, u32), ChildState>;
fn entries() -> &'static Mutex<Entries> {
    static ENTRIES: OnceLock<Mutex<Entries>> = OnceLock::new();
    ENTRIES.get_or_init(Mutex::default)
}

fn aliases() -> &'static Mutex<HashMap<usize, usize>> {
    static ALIASES: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
    ALIASES.get_or_init(Mutex::default)
}
pub(super) fn root_owner(owner: usize) -> usize {
    let aliases = aliases().lock().unwrap_or_else(|e| e.into_inner());
    aliases.get(&owner).copied().unwrap_or(owner)
}
pub(crate) struct ChildOwnerLink(usize);
pub(crate) fn link_child_owner(child: &AtomicBool, parent: &AtomicBool) -> ChildOwnerLink {
    let key = child as *const _ as usize;
    let root = root_owner(parent as *const _ as usize);
    aliases()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, root);
    ChildOwnerLink(key)
}
impl Drop for ChildOwnerLink {
    fn drop(&mut self) {
        aliases()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

pub(super) struct ChildActivity {
    key: Option<(usize, u32)>,
    pid: u32,
    sampled: Instant,
    cpu: HashMap<(u32, u64), u64>,
    last_cpu: Option<Instant>,
    setup_until: Option<Instant>,
    started: Instant,
    program: String,
}

impl ChildActivity {
    pub(super) fn new(pid: u32, cancel: Option<&AtomicBool>) -> Self {
        let key = cancel.map(|cancel| (root_owner(cancel as *const _ as usize), pid));
        let setup_until =
            sandbox_helper_still_setting_up(pid).then(|| Instant::now() + Duration::from_secs(60));
        let started = Instant::now();
        let program = process_name(pid);
        if let Some(key) = key {
            entries().lock().unwrap_or_else(|e| e.into_inner()).insert(
                key,
                ChildState {
                    progress: None,
                    setup_until,
                    started,
                    program: program.clone(),
                    output: None,
                    cpu: None,
                },
            );
        }
        Self {
            key,
            pid,
            sampled: Instant::now(),
            cpu: cpu_ticks(pid),
            last_cpu: None,
            setup_until,
            started,
            program,
        }
    }

    pub(super) fn observe(&mut self, output: Option<Instant>) -> Option<Duration> {
        if self.sampled.elapsed() >= Duration::from_secs(1) {
            let cpu = cpu_ticks(self.pid);
            if let Some((&(pid, _), _)) = cpu.iter().max_by_key(|(pid, ticks)| {
                (
                    ticks.saturating_sub(self.cpu.get(pid).copied().unwrap_or(0)),
                    **ticks,
                )
            }) {
                self.program = process_name(pid);
            }
            if cpu
                .iter()
                .any(|(pid, ticks)| *ticks > self.cpu.get(pid).copied().unwrap_or(0))
            {
                self.last_cpu = Some(Instant::now());
            }
            self.cpu = cpu;
            self.sampled = Instant::now();
        }
        if self.setup_until.is_some_and(|until| {
            Instant::now() >= until || !sandbox_helper_still_setting_up(self.pid)
        }) {
            self.setup_until = None;
        }
        let latest = if self.setup_until.is_some() {
            // Setup work is exempt briefly, but never earns child progress.
            self.last_cpu = None;
            None
        } else {
            output.into_iter().chain(self.last_cpu).max()
        };
        if let Some(key) = self.key {
            entries().lock().unwrap_or_else(|e| e.into_inner()).insert(
                key,
                ChildState {
                    progress: latest,
                    setup_until: self.setup_until,
                    started: self.started,
                    program: self.program.clone(),
                    output,
                    cpu: self.last_cpu,
                },
            );
        }
        latest.map(|stamp| stamp.elapsed())
    }
}

impl Drop for ChildActivity {
    fn drop(&mut self) {
        if let Some(key) = self.key {
            entries()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&key);
        }
    }
}

pub(crate) fn owned_child_active(owner: usize) -> bool {
    let owner = root_owner(owner);
    let floor = [
        tool_idle_timeout(),
        tool_idle_floor(),
        crate::turn::configured_turn_idle_timeout_secs().map(Duration::from_secs),
    ]
    .into_iter()
    .flatten()
    .min();
    let Some(floor) = floor else {
        return false;
    };
    entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|((key, _), state)| {
            *key == owner && state.progress.is_some_and(|s| s.elapsed() < floor)
        })
}

/// Setup grace is tracked independently from output/CPU progress and expires.
pub(crate) fn owned_child_setting_up(owner: usize) -> bool {
    let owner = root_owner(owner);
    entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|((key, _), state)| {
            *key == owner
                && state
                    .setup_until
                    .is_some_and(|until| Instant::now() < until)
        })
}

#[cfg(target_os = "linux")]
fn cpu_ticks(leader: u32) -> HashMap<(u32, u64), u64> {
    let mut ticks = HashMap::new();
    let supervised = sandbox_helper_process(leader);
    for (pid, row) in owned_process_activity(leader) {
        // The supervisor polls waitpid; that overhead is not payload CPU.
        if !(matches!(row.state, 'Z' | 'X') || (supervised && pid == leader)) {
            ticks.insert((pid, row.started), row.cpu);
        }
    }
    ticks
}

#[cfg(not(target_os = "linux"))]
fn cpu_ticks(_leader: u32) -> HashMap<(u32, u64), u64> {
    HashMap::new()
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/exec__activity__tests.rs"]
mod tests;
