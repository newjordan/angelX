//! Linux ownership boundary shared by the cockpit and its single-threaded helper.
//! Subreapers retain double-forked/setsid descendants; only descendants of this
//! process are signalled. No host-wide process-name matching is used.
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[path = "process_owner/children.rs"]
mod children;
// The tiny sandbox binary shares this module but needs only the command trait.
#[allow(unused_imports)]
pub(crate) use children::{Child, ChildClaim, OwnedCommandExt, claim_spawn};

/// One process-death payload shared by foreground and background tools.
/// Unknown external senders are explicitly unknown; observed signals do not
/// prove who sent them.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct KillReceipt {
    pub(crate) signal: Option<i32>,
    pub(crate) reason: String,
    pub(crate) owner: String,
    pub(crate) owner_pid: Option<u32>,
}

impl KillReceipt {
    pub(crate) fn new(signal: Option<i32>, reason: &str, owner: &str) -> Self {
        Self {
            signal,
            reason: reason.into(),
            owner: owner.into(),
            owner_pid: (owner != "unknown_external").then(std::process::id),
        }
    }

    pub(crate) fn apply(&self, entry: &mut serde_json::Value) {
        entry["status"] = serde_json::json!("killed");
        entry["exec"] = serde_json::json!("cancelled");
        entry["err"] = serde_json::json!(true);
        entry["verify"] = serde_json::json!("inconclusive");
        entry["kill"] = serde_json::json!(self);
    }

    pub(crate) fn error(&self, detail: &str) -> String {
        format!(
            "process killed {}\n{detail}",
            serde_json::to_string(self).expect("kill receipt")
        )
    }

    pub(crate) fn from_error(result: &str) -> Option<Self> {
        let result = result.strip_prefix("tool error: ").unwrap_or(result);
        let body = result.strip_prefix("process killed ")?.lines().next()?;
        serde_json::from_str(body).ok()
    }
}

static SIGNAL: AtomicI32 = AtomicI32::new(0);
static ACTIVE_GRAPH_RUNS: AtomicUsize = AtomicUsize::new(0);

/// Keep the exit reaper from discarding an active graph's cancellation receipt.
/// The graph observes the signal and uses its existing cooperative wind-down;
/// dropping the last guard releases ordinary process shutdown after persistence.
pub(crate) struct GraphSignalGuard;

pub(crate) fn defer_exit_for_graph() -> GraphSignalGuard {
    ACTIVE_GRAPH_RUNS.fetch_add(1, Ordering::AcqRel);
    GraphSignalGuard
}

pub(crate) fn exit_requested() -> bool {
    SIGNAL.load(Ordering::Acquire) != 0
}

impl Drop for GraphSignalGuard {
    fn drop(&mut self) {
        ACTIVE_GRAPH_RUNS.fetch_sub(1, Ordering::AcqRel);
    }
}

extern "C" fn on_signal(signal: i32) {
    SIGNAL.store(signal, Ordering::Release);
}

#[cfg(target_os = "linux")]
fn install() -> std::io::Result<()> {
    // SAFETY: prctl changes only this process. The handler performs one atomic
    // store; allocation, locks, waiting and IO happen outside signal context.
    unsafe {
        // Fail before launching work if this host cannot support identity-pinned
        // cleanup; otherwise a denied pidfd syscall could strand the reaper.
        let fd = libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0) as i32;
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let result = libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd,
            0,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        );
        let error = std::io::Error::last_os_error();
        libc::close(fd);
        if result < 0 {
            return Err(error);
        }
        if libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) < 0 {
            return Err(std::io::Error::last_os_error());
        }
        for signal in [libc::SIGINT, libc::SIGTERM] {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_signal as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            if libc::sigaction(signal, &action, std::ptr::null_mut()) < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn descendants() -> Vec<(i32, u64)> {
    let mut rows = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<i32>().ok())
            else {
                continue;
            };
            let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            let Some((_, fields)) = stat.rsplit_once(')') else {
                continue;
            };
            let fields: Vec<_> = fields.split_whitespace().collect();
            if let (Some(parent), Some(start)) = (
                fields.get(1).and_then(|s| s.parse::<i32>().ok()),
                fields.get(19).and_then(|s| s.parse::<u64>().ok()),
            ) {
                rows.push((pid, parent, start));
            }
        }
    }
    let mut owned = vec![std::process::id() as i32];
    loop {
        let previous = owned.len();
        for &(pid, parent, _) in &rows {
            if owned.contains(&parent) && !owned.contains(&pid) {
                owned.push(pid);
            }
        }
        if owned.len() == previous {
            break;
        }
    }
    owned.remove(0);
    rows.into_iter()
        .filter(|(pid, _, _)| owned.contains(pid))
        .map(|(pid, _, start)| (pid, start))
        .collect()
}

/// Pin identity across the /proc-to-signal interval. A PID reused after the
/// snapshot must never turn cleanup into a signal to another worker's process.
#[cfg(target_os = "linux")]
fn signal_owned(pid: i32, start: u64, signal: i32) {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
    if fd < 0 {
        return;
    }
    let same_process = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(')').and_then(|(_, fields)| {
                fields
                    .split_whitespace()
                    .nth(19)
                    .and_then(|s| s.parse::<u64>().ok())
            })
        })
        == Some(start);
    unsafe {
        if same_process {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd,
                signal,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            );
        }
        libc::close(fd);
    }
}

/// Called only at process shutdown or in the dedicated helper. Direct-child
/// waiters in the cockpit may race this final reap, but no turn continues after
/// cockpit shutdown. Helpers have exactly one waiter (this thread).
#[cfg(target_os = "linux")]
pub(crate) fn cleanup() {
    children::stop_reaping();
    let grace = Instant::now() + Duration::from_millis(250);
    loop {
        // ECHILD proves the subreaper owns no remaining descendants. Test
        // that kernel fact before enumerating every process on the host.
        let waited = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        if waited < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
            break;
        }
        let owned = descendants();
        if owned.is_empty() {
            // A child can fork/reparent between /proc rows. ECHILD, rather than
            // an empty snapshot, proves this subreaper has nothing left to own.
            let waited = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
            if waited < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        let signal = cleanup_signal(Instant::now(), grace);
        for &(pid, start) in owned.iter().rev() {
            signal_owned(pid, start, signal);
        }
        // Reap adopted descendants too, including zombies with a new session.
        while unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) } > 0 {}
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn cleanup() {}

/// Main-process installation, before any tool can spawn. atexit also covers
/// explicit std::process::exit paths that skip Rust destructors.
pub(crate) fn initialize() -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        install()?;
        extern "C" fn at_exit() {
            cleanup();
        }
        unsafe {
            libc::atexit(at_exit);
        }
        std::thread::Builder::new()
            .name("angel-exit-reaper".into())
            .spawn(|| {
                let mut next_reap = Instant::now();
                loop {
                    let signal = SIGNAL.load(Ordering::Acquire);
                    if signal != 0 && ACTIVE_GRAPH_RUNS.load(Ordering::Acquire) == 0 {
                        std::process::exit(128 + signal);
                    }
                    if Instant::now() >= next_reap {
                        children::reap_unclaimed();
                        next_reap = Instant::now() + Duration::from_millis(250);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })?;
    }
    Ok(())
}

/// Fork only in the fresh, single-threaded sandbox helper, before confinement.
/// The parent remains a subreaper until the command and every descendant exit.
/// The child returns to install the policy and exec the actual command.
///
/// `detached` is the trusted lifecycle already read by the helper entrypoint.
/// Ordinary attached commands start a new session so they are not bound to a
/// private parent terminal. Detached owners keep the established process group
/// so group cancellation still reaps namespace descendants.
pub(crate) fn supervise(detached: bool) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        install()?;
        let parent = unsafe { libc::getpid() };
        let child = unsafe { libc::fork() };
        if child < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if child == 0 {
            unsafe {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::signal(libc::SIGTERM, libc::SIG_DFL);
                // The helper is already the process-group leader (`process_group(0)`).
                // This command child is not, so setsid can detach it from the
                // cockpit controlling terminal. Calling setsid on the helper
                // itself would fail with EPERM. Detached work must remain in
                // that owner group; do not create a new session for it.
                if !detached && libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // Close the parent-death race before exec. The cockpit is also
                // a subreaper if the supervisor itself is killed externally.
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    libc::_exit(125);
                }
            }
            return Ok(());
        }
        // Readiness only: waitpid below remains the exit-status authority.
        // If this readiness descriptor cannot open, keep bounded sleeps.
        // The existing install preflight still requires pidfd cleanup support.
        let exit_fd = unsafe { libc::syscall(libc::SYS_pidfd_open, child, 0) } as i32;
        let mut status = 0;
        loop {
            let result = unsafe { libc::waitpid(child, &mut status, libc::WNOHANG) };
            if result == child {
                break;
            }
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::Interrupted {
                    cleanup();
                    unsafe {
                        libc::_exit(125);
                    }
                }
            }
            if SIGNAL.load(Ordering::Acquire) != 0 {
                break;
            }
            if exit_fd >= 0 {
                let mut descriptor = libc::pollfd {
                    fd: exit_fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                // Preserve the cancellation ceiling even if a signal lands
                // just before poll; readiness can wake earlier than 5 ms.
                let ready = unsafe { libc::poll(&mut descriptor, 1, 5) };
                if (ready < 0
                    && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted)
                    || descriptor.revents & (libc::POLLERR | libc::POLLNVAL) != 0
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        if exit_fd >= 0 {
            unsafe { libc::close(exit_fd) };
        }
        let interrupted = SIGNAL.load(Ordering::Acquire);
        cleanup();
        let signal = if interrupted != 0 {
            interrupted
        } else if libc::WIFSIGNALED(status) {
            libc::WTERMSIG(status)
        } else {
            0
        };
        unsafe {
            if signal != 0 {
                libc::signal(signal, libc::SIG_DFL);
                libc::raise(signal);
                libc::_exit(128 + signal);
            }
            libc::_exit(if libc::WIFEXITED(status) {
                libc::WEXITSTATUS(status)
            } else {
                125
            });
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = detached;
        Ok(())
    }
}

// Keep the existing cleanup policy separate from the clock read so its exact
// escalation boundary is deterministic in the lifecycle contract tests.
#[cfg(target_os = "linux")]
fn cleanup_signal(now: Instant, grace: Instant) -> i32 {
    if now < grace {
        libc::SIGTERM
    } else {
        libc::SIGKILL
    }
}

#[cfg(all(test, target_os = "linux"))]
mod mutation_tests {
    use super::*;

    #[test]
    #[ignore = "subprocess fixture; invoked by cleanup_reaps_adopted_children"]
    fn cleanup_child_fixture() {
        let Some(marker) = std::env::var_os("ANGEL_T_OWNER_MARKER") else {
            return;
        };
        install().unwrap();
        let payload = std::env::var("ANGEL_T_OWNER_PAYLOAD").unwrap();
        let status = std::process::Command::new("sh")
            .args(["-c", &payload, "owner-fixture"])
            .arg(&marker)
            .status()
            .unwrap();
        assert!(status.success());
        cleanup();
        let remaining = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        assert_eq!(remaining, -1, "cleanup left a child or zombie");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
    }

    #[test]
    fn cleanup_reaps_adopted_children() {
        use std::os::unix::process::CommandExt;
        struct Sentinel(std::process::Child);
        impl Drop for Sentinel {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let root = std::env::temp_dir().join(format!("angel-owner-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        // A sibling of the fixture is not owned by its subreaper.
        let mut sentinel = Sentinel(
            std::process::Command::new("sleep")
                .arg("30")
                .spawn()
                .unwrap(),
        );
        for (index, payload) in [
            "true",
            "sleep 30 & echo $! > \"$1\"",
            "setsid sh -c 'echo $$ > \"$1\"; sleep 30' child \"$1\" & while [ ! -s \"$1\" ]; do :; done",
            "setsid sh -c 'trap \"\" TERM; echo $$ > \"$1\"; sleep 30' child \"$1\" & while [ ! -s \"$1\" ]; do :; done",
        ].iter().enumerate() {
            let marker = root.join(format!("child-{index}"));
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "sandbox::process_owner::mutation_tests::cleanup_child_fixture", "--ignored", "--test-threads=1"])
                .env("ANGEL_T_OWNER_MARKER", &marker)
                .env("ANGEL_T_OWNER_PAYLOAD", payload)
                .stdout(std::process::Stdio::null())
                .process_group(0)
                .spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if Instant::now() >= deadline {
                    unsafe { libc::killpg(child.id() as i32, libc::SIGKILL) };
                    let _ = child.wait();
                    panic!("owned-child cleanup exceeded deadline: {index}");
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            assert!(status.success(), "ownership fixture failed: {index}");
            assert!(sentinel.0.try_wait().unwrap().is_none(), "unrelated sentinel killed");
            if let Ok(pid) = std::fs::read_to_string(&marker) {
                let pid: i32 = pid.trim().parse().unwrap();
                assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "owned PID survived: {pid}");
                assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lifecycle_unknown_external_owner_is_not_fabricated() {
        let unknown = KillReceipt::new(Some(libc::SIGTERM), "signal", "unknown_external");
        assert_eq!(unknown.owner_pid, None);
        let known = KillReceipt::new(None, "cancelled", "harness");
        assert_eq!(known.owner_pid, Some(std::process::id()));
    }

    #[test]
    fn lifecycle_grace_boundary_escalates_at_deadline() {
        let deadline = Instant::now();
        assert_eq!(
            cleanup_signal(deadline - Duration::from_nanos(1), deadline),
            libc::SIGTERM
        );
        assert_eq!(cleanup_signal(deadline, deadline), libc::SIGKILL);
        assert_eq!(
            cleanup_signal(deadline + Duration::from_nanos(1), deadline),
            libc::SIGKILL
        );
    }
}
