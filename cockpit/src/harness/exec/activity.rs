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
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn owned_cpu_crosses_sessions_without_crediting_siblings() {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        let _guard = crate::tests::env_lock();
        struct Probe {
            parent: crate::sandbox::process_owner::Child,
            payload: Option<u32>,
            payload_fd: Option<OwnedFd>,
        }
        impl Drop for Probe {
            fn drop(&mut self) {
                if let Some(fd) = &self.payload_fd {
                    unsafe {
                        libc::syscall(
                            libc::SYS_pidfd_send_signal,
                            fd.as_raw_fd(),
                            libc::SIGKILL,
                            std::ptr::null::<libc::siginfo_t>(),
                            0,
                        );
                    }
                }
                let _ = self.parent.kill();
                let _ = self.parent.wait();
            }
        }
        let dir = crate::tests::TestGitWorkspace::new("owned-cpu");
        let ready = dir.path().join("payload.pid");
        let script = r#"
import subprocess, sys
child = subprocess.Popen([sys.executable, '-c',
    'import time; end=time.monotonic()+10\nwhile time.monotonic()<end: pass'],
    start_new_session=True)
with open(sys.argv[1], 'w') as f:
    f.write(str(child.pid))
child.wait()
"#;
        let mut busy = Probe {
            parent: Command::new("python3")
                .args(["-c", script])
                .arg(&ready)
                .spawn_owned()
                .unwrap(),
            payload: None,
            payload_fd: None,
        };
        let sleeping = Probe {
            parent: Command::new("sleep").arg("10").spawn_owned().unwrap(),
            payload: None,
            payload_fd: None,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while busy.payload.is_none() && Instant::now() < deadline {
            busy.payload = std::fs::read_to_string(&ready)
                .ok()
                .and_then(|pid| pid.parse().ok());
            std::thread::sleep(Duration::from_millis(10));
        }
        let payload = busy.payload.expect("cross-session payload became ready");
        let identity = proc_activity(payload).unwrap();
        assert_eq!(identity.parent, busy.parent.id());
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, payload, 0) } as i32;
        assert!(fd >= 0, "pin fixture payload identity");
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        assert_eq!(proc_activity(payload).unwrap().started, identity.started);
        busy.payload_fd = Some(fd);
        assert_ne!(unsafe { libc::getsid(payload as i32) }, unsafe {
            libc::getsid(busy.parent.id() as i32)
        });
        assert_eq!(live_group_descendants(busy.parent.id()), 1);
        let key = (payload, proc_activity(payload).unwrap().started);
        let initial = cpu_ticks(busy.parent.id());
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let current = cpu_ticks(busy.parent.id());
            if current.get(&key).copied().unwrap_or(0) > initial.get(&key).copied().unwrap_or(0) {
                break;
            }
            assert!(Instant::now() < deadline, "owned payload CPU was invisible");
            std::thread::sleep(Duration::from_millis(20));
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while proc_activity(sleeping.parent.id()).is_none_or(|row| row.state != 'S') {
            assert!(Instant::now() < deadline, "sleeping sibling never settled");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!process_group_is_runnable(sleeping.parent.id()));
        assert_eq!(live_group_descendants(sleeping.parent.id()), 0);
        assert!(
            cpu_ticks(sleeping.parent.id())
                .keys()
                .all(|(pid, _)| *pid == sleeping.parent.id())
        );
    }

    #[test]
    fn child_activity_snapshot_keeps_cpu_output_and_owner_distinct() {
        let _guard = crate::tests::env_lock();
        let parent = AtomicBool::new(false);
        let nested = AtomicBool::new(false);
        let foreign = AtomicBool::new(false);
        let owner = &parent as *const _ as usize;
        let _link = link_child_owner(&nested, &parent);
        let mut child = ChildActivity::new(u32::MAX, Some(&nested));
        let initial = owned_child_snapshot(owner).unwrap();
        assert_eq!(initial.cpu_age_secs, None);
        assert_eq!(initial.output_age_secs, None);
        assert!(owned_child_snapshot(&foreign as *const _ as usize).is_none());
        child.program = "rustc".into();
        child.last_cpu = Some(Instant::now());
        child.observe(Some(Instant::now() - Duration::from_secs(40)));
        let busy = owned_child_snapshot(owner).unwrap();
        assert_eq!(busy.program, "rustc");
        assert_eq!(busy.cpu_age_secs, Some(0));
        assert!(busy.output_age_secs.unwrap() >= 40);
        child.last_cpu = Some(Instant::now() - Duration::from_secs(35));
        child.observe(None);
        assert!(owned_child_snapshot(owner).unwrap().cpu_age_secs.unwrap() >= 35);
        drop(child);
        assert!(
            owned_child_snapshot(owner).is_none(),
            "exited work must disappear"
        );
    }

    #[test]
    fn run_turn_helper_setup_grace_expires_without_progress_credit() {
        let _guard = crate::tests::env_lock();
        let cancel = AtomicBool::new(false);
        let owner = &cancel as *const _ as usize;
        let activity = ChildActivity::new(u32::MAX, Some(&cancel));
        let key = activity.key.unwrap();
        entries().lock().unwrap().get_mut(&key).unwrap().setup_until =
            Some(Instant::now() + Duration::from_secs(60));
        assert!(owned_child_setting_up(owner));
        assert!(!owned_child_active(owner));
        entries().lock().unwrap().get_mut(&key).unwrap().setup_until =
            Some(Instant::now() - Duration::from_secs(1));
        assert!(!owned_child_setting_up(owner));
        assert!(!owned_child_active(owner));
    }

    #[test]
    fn stale_cpu_credit_expires_at_shorter_turn_idle_timeout() {
        let _guard = crate::tests::env_lock();
        let _floor = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "30");
        let _turn = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "1");
        let _tool = crate::tests::TestEnvGuard::unset("ANGEL_TOOL_IDLE_SECS");
        let cancel = AtomicBool::new(false);
        let mut activity = ChildActivity::new(u32::MAX, Some(&cancel));
        activity.last_cpu = Some(Instant::now() - Duration::from_secs(2));
        activity.observe(None);
        assert!(!owned_child_active(&cancel as *const _ as usize));
        activity.last_cpu = Some(Instant::now());
        activity.observe(None);
        assert!(owned_child_active(&cancel as *const _ as usize));
    }

    #[test]
    fn run_turn_child_activity_is_owned_recent_and_released() {
        let _guard = crate::tests::env_lock();
        let _floor = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "2");
        let parent = AtomicBool::new(false);
        let nested = AtomicBool::new(false);
        let foreign = AtomicBool::new(false);
        let parent_key = &parent as *const _ as usize;
        let _link = link_child_owner(&nested, &parent);
        let mut activity = ChildActivity::new(u32::MAX, Some(&nested));
        assert!(
            !owned_child_active(parent_key),
            "spawn alone is not progress"
        );
        activity.observe(Some(Instant::now()));
        assert!(owned_child_active(parent_key));
        assert!(owned_child_active(&nested as *const _ as usize));
        assert!(owned_child_snapshot(&nested as *const _ as usize).is_some());
        assert!(!owned_child_active(&foreign as *const _ as usize));
        activity.observe(Some(Instant::now() - Duration::from_secs(3)));
        assert!(
            !owned_child_active(parent_key),
            "stale output is not liveness"
        );
        activity.observe(Some(Instant::now()));
        drop(activity);
        assert!(
            !owned_child_active(parent_key),
            "exited child releases credit"
        );
    }
}
