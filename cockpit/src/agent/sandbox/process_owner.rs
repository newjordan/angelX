//! Linux ownership boundary shared by the cockpit and its single-threaded helper.
//! Subreapers retain double-forked/setsid descendants; only descendants of this
//! process are signalled. No host-wide process-name matching is used.
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[path = "process_owner/children.rs"]
mod children;
#[allow(unused_imports)]
pub(crate) use children::{Child, ChildClaim, OwnedCommandExt, claim_spawn};

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
#[cfg(target_os = "linux")]
static EXIT_ROUTE: std::sync::OnceLock<fn(i32)> = std::sync::OnceLock::new();

#[cfg(target_os = "linux")]
pub(crate) fn route_exit(route: fn(i32)) {
    let _ = EXIT_ROUTE.set(route);
}

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

#[cfg(target_os = "linux")]
static REAPER_WAKE: AtomicI32 = AtomicI32::new(-1);

#[cfg(target_os = "linux")]
extern "C" fn on_signal(signal: i32) {
    SIGNAL.store(signal, Ordering::Release);
    let wake = REAPER_WAKE.load(Ordering::Relaxed);
    if wake >= 0 {
        unsafe {
            let errno = *libc::__errno_location();
            let one = 1u64;
            libc::write(wake, (&raw const one).cast(), std::mem::size_of::<u64>());
            *libc::__errno_location() = errno;
        }
    }
}

#[cfg(target_os = "linux")]
fn reaper_sleep(wait: Duration) {
    reaper_sleep_on(REAPER_WAKE.load(Ordering::Relaxed), wait);
}

#[cfg(target_os = "linux")]
fn reaper_sleep_on(wake: i32, wait: Duration) {
    if wake < 0 {
        std::thread::sleep(wait.min(Duration::from_millis(10)));
        return;
    }
    let mut probe = libc::pollfd {
        fd: wake,
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = wait.as_millis().saturating_add(1).min(1_000) as libc::c_int;
    unsafe {
        if libc::poll(&mut probe, 1, millis) > 0 {
            let mut count = 0u64;
            libc::read(wake, (&raw mut count).cast(), std::mem::size_of::<u64>());
        }
    }
}

#[cfg(target_os = "linux")]
fn install() -> std::io::Result<()> {
    unsafe {
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
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            if signal == libc::SIGHUP {
                let mut current: libc::sigaction = std::mem::zeroed();
                if libc::sigaction(signal, std::ptr::null(), &mut current) == 0
                    && current.sa_sigaction == libc::SIG_IGN
                {
                    continue;
                }
            }
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
static PERSISTENT_SPARED: std::sync::Mutex<Vec<(i32, u64)>> = std::sync::Mutex::new(Vec::new());

#[cfg(target_os = "linux")]
fn process_starttime(pid: i32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, fields) = stat.rsplit_once(')')?;
    fields
        .split_whitespace()
        .nth(19)
        .and_then(|s| s.parse::<u64>().ok())
}

#[cfg(target_os = "linux")]
pub(crate) fn spare_persistent(pid: u32) {
    if let Some(start) = process_starttime(pid as i32)
        && let Ok(mut spared) = PERSISTENT_SPARED.lock()
    {
        spared.push((pid as i32, start));
    }
}

#[cfg(target_os = "linux")]
fn descendants() -> Vec<(i32, u64)> {
    let spared: Vec<(i32, u64)> = PERSISTENT_SPARED
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
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
            if let (Some(parent), Some(session), Some(start)) = (
                fields.get(1).and_then(|s| s.parse::<i32>().ok()),
                fields.get(3).and_then(|s| s.parse::<i32>().ok()),
                fields.get(19).and_then(|s| s.parse::<u64>().ok()),
            ) {
                rows.push((pid, parent, session, start));
            }
        }
    }
    let spared_sessions: Vec<i32> = spared.iter().map(|&(pid, _)| pid).collect();
    let is_spared = |pid: i32, session: i32, start: u64| {
        spared.contains(&(pid, start)) || spared_sessions.contains(&session)
    };
    let mut owned = vec![std::process::id() as i32];
    loop {
        let previous = owned.len();
        for &(pid, parent, session, start) in &rows {
            if owned.contains(&parent) && !owned.contains(&pid) && !is_spared(pid, session, start)
            {
                owned.push(pid);
            }
        }
        if owned.len() == previous {
            break;
        }
    }
    owned.remove(0);
    rows.into_iter()
        .filter(|(pid, _, _, _)| owned.contains(pid))
        .map(|(pid, _, _, start)| (pid, start))
        .collect()
}

/// Names of owned descendants that are still running. An exit nobody has collected yet is
/// not running work.
#[cfg(target_os = "linux")]
fn running_descendants() -> Vec<String> {
    descendants()
        .into_iter()
        .filter_map(|(pid, _)| {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            let (head, fields) = stat.rsplit_once(')')?;
            if fields.split_whitespace().next() == Some("Z") {
                return None;
            }
            Some(
                head.split_once('(')
                    .map_or_else(String::new, |(_, name)| name.to_string()),
            )
        })
        .collect()
}

/// What the reader of a command's result is told when work it left in the background was
/// stopped as the command returned. `None` when nothing was left running.
#[cfg(target_os = "linux")]
pub(crate) fn stopped_background_notice(names: &[String]) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    let mut shown: Vec<&str> = Vec::new();
    let mut more = false;
    for name in names.iter().map(String::as_str).filter(|name| !name.is_empty()) {
        if shown.contains(&name) {
            continue;
        }
        if shown.len() == 4 {
            more = true;
            break;
        }
        shown.push(name);
    }
    let mut named = shown.join(", ");
    if more {
        named.push_str(", …");
    }
    let (count, were) = if names.len() == 1 {
        ("1 background process".to_string(), "was")
    } else {
        (format!("{} background processes", names.len()), "were")
    };
    let named = if named.is_empty() {
        String::new()
    } else {
        format!(" ({named})")
    };
    Some(format!(
        "angel: {count} this command started {were} still running when it returned and {were} \
         stopped{named}. Work started with `&` ends with the command: wait for it in the same \
         command (`wait`) or run it in the foreground.\n"
    ))
}

/// A command owns its processes until it returns; whatever it left running is stopped by
/// `cleanup`. Say so on the command's own stderr first, so the result does not read as if the
/// work were still going.
#[cfg(target_os = "linux")]
fn report_stopped_background() {
    // The tail of a pipeline or a process substitution is on its way out, not background
    // work: give it a moment to finish before counting.
    let settle = Instant::now() + Duration::from_millis(100);
    let mut running = running_descendants();
    while !running.is_empty() && Instant::now() < settle {
        std::thread::sleep(Duration::from_millis(10));
        running = running_descendants();
    }
    if let Some(notice) = stopped_background_notice(&running) {
        unsafe {
            libc::write(2, notice.as_ptr().cast(), notice.len());
        }
    }
}

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

#[cfg(target_os = "linux")]
fn only_spared_remain() -> bool {
    let spared: Vec<(i32, u64)> = PERSISTENT_SPARED
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    if spared.is_empty() {
        return false;
    }
    let me = std::process::id() as i32;
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
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
        let (Some(parent), Some(session), Some(start)) = (
            fields.get(1).and_then(|s| s.parse::<i32>().ok()),
            fields.get(3).and_then(|s| s.parse::<i32>().ok()),
            fields.get(19).and_then(|s| s.parse::<u64>().ok()),
        ) else {
            continue;
        };
        if parent != me {
            continue;
        }
        let is_spared =
            spared.contains(&(pid, start)) || spared.iter().any(|&(leader, _)| leader == session);
        if !is_spared {
            return false;
        }
    }
    true
}

#[cfg(target_os = "linux")]
pub(crate) fn cleanup() {
    children::stop_reaping();
    let grace = Instant::now() + Duration::from_millis(250);
    loop {
        let waited = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        if waited < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
            break;
        }
        let owned = descendants();
        if owned.is_empty() {
            if only_spared_remain() {
                break;
            }
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
        while unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) } > 0 {}
        std::thread::sleep(Duration::from_millis(10));
    }
}

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
        let wake = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        REAPER_WAKE.store(wake, Ordering::Relaxed);
        std::thread::Builder::new()
            .name("angel-exit-reaper".into())
            .spawn(|| {
                let mut next_reap = Instant::now();
                loop {
                    let signal = SIGNAL.load(Ordering::Acquire);
                    if signal != 0 && ACTIVE_GRAPH_RUNS.load(Ordering::Acquire) == 0 {
                        if let Some(route) = EXIT_ROUTE.get() {
                            route(128 + signal);
                            std::thread::sleep(Duration::from_secs(2));
                            cleanup();
                            unsafe { libc::_exit(128 + signal) };
                        }
                        std::process::exit(128 + signal);
                    }
                    if Instant::now() >= next_reap {
                        children::reap_unclaimed();
                        next_reap = Instant::now() + Duration::from_millis(250);
                    }
                    reaper_sleep(if SIGNAL.load(Ordering::Acquire) != 0 {
                        Duration::from_millis(10)
                    } else {
                        next_reap.saturating_duration_since(Instant::now())
                    });
                }
            })?;
    }
    Ok(())
}

pub(crate) fn supervise(detached: bool) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        install()?;
        let owner = (!detached).then(|| unsafe { libc::getppid() });
        let parent = unsafe { libc::getpid() };
        let child = unsafe { libc::fork() };
        if child < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if child == 0 {
            unsafe {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::signal(libc::SIGTERM, libc::SIG_DFL);
                if !detached && libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    libc::_exit(125);
                }
            }
            return Ok(());
        }
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
            if owner.is_some_and(|owner| unsafe { libc::getppid() } != owner) {
                SIGNAL.store(libc::SIGTERM, Ordering::Release);
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
        if !detached && interrupted == 0 && libc::WIFEXITED(status) {
            report_stopped_background();
        }
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
        if !detached {
            unsafe {
                let fd = libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC);
                if fd >= 0 {
                    libc::ioctl(fd, libc::TIOCNOTTY as libc::c_ulong);
                    libc::close(fd);
                }
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn cleanup_signal(now: Instant, grace: Instant) -> i32 {
    if now < grace {
        libc::SIGTERM
    } else {
        libc::SIGKILL
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "../../../../tests/cockpit/app/sandbox__process_owner__mutation_tests.rs"]
mod mutation_tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "../../../../tests/cockpit/app/sandbox__process_owner__reaper_wake_tests.rs"]
mod reaper_wake_tests;
