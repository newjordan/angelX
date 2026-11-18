//! Shared workspace-scoped persistence paths and **repo identity**.
//!
//! Cockpit features that store per-workspace state should use this module for
//! stable file naming instead of each feature inventing its own key/hash scheme.
//!
//! # Repo identity — why a workspace is not a repository
//!
//! [`workspace_key`] keys on the *directory path*, which is right for
//! directory-scoped state and wrong for anything that accumulates knowledge
//! about a **repository**. A git worktree of angelX (`git worktree add`) is a
//! different directory, so it minted a different key — and every organ that
//! learns per-repo (the Repo Dossier's ledger events, The Cut's authored-diff
//! manifest) filed its evidence under a throwaway identity that nothing would
//! ever read again. The substrate factory (`scripts/cut-forge.mjs`) runs
//! `angel --task` in exactly such disposable worktrees, so its entire output
//! was being written to a new, dead repo every run. Same for a `/cd` into a
//! subdirectory: `angelX/cockpit` was a different "repo" than `angelX`.
//!
//! [`repo_identity`] resolves a workspace to the repository it belongs to, the
//! way git itself does — the **main worktree** (`git worktree list` always
//! prints it first), never the linked one. A subdirectory, a linked worktree,
//! and the checkout itself all collapse to one identity; a *nested independent*
//! repo (its own `.git`) keeps its own, because that is what git reports.
//!
//! Two properties this deliberately preserves:
//!
//! * **Valid UTF-8 path keys stay compatible.** A repo whose workspace already
//!   **is** its main worktree keeps its exact key (see [`repo_identity`]).
//!   Invalid UTF-8 paths use a separate raw-byte hash namespace. Legacy lossy
//!   keys are ambiguous and are not automatically migrated or attributed.
//! * **Never crash, never block.** Not a repo, no remote, git missing, detached
//!   HEAD, bare repo, submodule: every failure falls back to the workspace path,
//!   i.e. the pre-existing behavior. `ANGEL_REPO_IDENTITY=0` forces that legacy
//!   path-keying wholesale.

use crate::agent::sandbox::process_owner::OwnedCommandExt;
pub(crate) mod private_io;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub fn angel_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".angelX")
}

pub fn angel_subdir(name: &str) -> PathBuf {
    angel_dir().join(name)
}

pub use angel_live::host::workspace_store::workspace_key;
#[cfg(not(test))]
pub(crate) use angel_live::host::workspace_store::{write_atomic, write_private_atomic, write_private_atomic_exact};

#[cfg(test)]
pub(crate) fn install_test_hooks() {
    use angel_live::host::workspace_store::fault::{AFTER_RENAME, BEFORE_OPEN, BEFORE_RENAME};
    BEFORE_OPEN.with(|slot| *slot.borrow_mut() = Some(Box::new(|path| { install_test_hooks(); storage_atomic_tests::before_atomic_open(path); })));
    BEFORE_RENAME.with(|slot| *slot.borrow_mut() = Some(Box::new(|path| { install_test_hooks(); storage_atomic_tests::before_atomic_rename(path); })));
    AFTER_RENAME.with(|slot| *slot.borrow_mut() = Some(Box::new(|path| { install_test_hooks(); storage_atomic_tests::after_atomic_rename(path) })));
}
#[cfg(test)]
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    install_test_hooks();
    angel_live::host::workspace_store::write_atomic(path, bytes)
}
#[cfg(test)]
pub(crate) fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    install_test_hooks();
    angel_live::host::workspace_store::write_private_atomic(path, bytes)
}
#[cfg(test)]
pub(crate) fn write_private_atomic_exact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    install_test_hooks();
    angel_live::host::workspace_store::write_private_atomic_exact(path, bytes)
}
#[cfg(test)]
pub(crate) use angel_live::host::workspace_store::try_private_lock;

pub fn workspace_json_path_in(dir: &Path, workspace: &Path) -> PathBuf {
    dir.join(format!("{}.json", workspace_key(workspace)))
}

pub(crate) fn remove_durable(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("remove state {}: {error}", path.display())),
    }
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("sync state directory {}: {error}", parent.display()))?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoIdentity {
    pub root: PathBuf,
    pub key: String,
    pub slug: Option<String>,
}

fn identity_enabled() -> bool {
    match std::env::var("ANGEL_REPO_IDENTITY") {
        Ok(v) => {
            let t = v.trim();
            !(t == "0"
                || t.eq_ignore_ascii_case("false")
                || t.eq_ignore_ascii_case("off")
                || t.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

const GIT_CAPTURE_LIMIT_BYTES: usize = 1_048_576;
const GIT_POLL_INTERVAL: Duration = Duration::from_millis(10);

struct CaptureExitWake {
    #[cfg(target_os = "linux")]
    fd: Option<std::os::fd::OwnedFd>,
}

impl CaptureExitWake {
    fn new(_pid: u32) -> Self {
        Self {
            #[cfg(target_os = "linux")]
            fd: {
                use std::os::fd::FromRawFd as _;
                let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, _pid, 0_u32) };
                (fd >= 0).then(|| {
                    unsafe { std::os::fd::OwnedFd::from_raw_fd(fd as i32) }
                })
            },
        }
    }

    fn wait(&self, remaining: Duration) {
        #[cfg(target_os = "linux")]
        if let Some(fd) = &self.fd {
            use std::os::fd::AsRawFd as _;
            let mut descriptor = libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let millis = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
            let ready = unsafe { libc::poll(&mut descriptor, 1, millis) };
            if (ready >= 0 && descriptor.revents & (libc::POLLERR | libc::POLLNVAL) == 0)
                || (ready < 0
                    && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted)
            {
                return;
            }
        }
        thread::sleep(remaining.min(GIT_POLL_INTERVAL));
    }
}

#[derive(Debug)]
struct CapturedStdout {
    bytes: Vec<u8>,
    overflow: bool,
}

#[cfg(unix)]
pub(crate) struct DeadlinePipe<R> {
    pipe: R,
    deadline: Instant,
}

#[cfg(unix)]
impl<R> DeadlinePipe<R> {
    pub(crate) fn new(pipe: R, deadline: Instant) -> Self {
        Self { pipe, deadline }
    }
}

#[cfg(unix)]
impl<R: std::io::Read + std::os::fd::AsRawFd> std::io::Read for DeadlinePipe<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            let remaining = self
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or(std::io::ErrorKind::TimedOut)?;
            let mut descriptor = libc::pollfd {
                fd: self.pipe.as_raw_fd(),
                events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                revents: 0,
            };
            let millis = remaining
                .as_millis()
                .saturating_add(1)
                .min(i32::MAX as u128) as i32;
            let ready = unsafe { libc::poll(&mut descriptor, 1, millis) };
            if ready == 0 {
                continue;
            }
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            match std::io::Read::read(&mut self.pipe, bytes) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }
}

fn drain_stdout<R>(
    mut stdout: R,
    limit: usize,
) -> thread::JoinHandle<std::io::Result<CapturedStdout>>
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
        let mut overflow = false;
        let mut chunk = [0_u8; 8 * 1024];
        loop {
            let read = stdout.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            let retained = read.min(limit.saturating_sub(bytes.len()));
            bytes.extend_from_slice(&chunk[..retained]);
            overflow |= retained < read;
        }
        Ok(CapturedStdout { bytes, overflow })
    })
}

fn kill_capture_group(child: &mut std::process::Child, pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn capture_command_with_deadline(
    mut command: Command,
    timeout: Duration,
    limit: usize,
) -> std::io::Result<Option<(ExitStatus, CapturedStdout)>> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command.spawn_owned()?;
    let pid = child.id();
    let Some(stdout) = child.stdout.take() else {
        kill_capture_group(&mut child, pid);
        return Err(std::io::Error::other("capture stdout pipe unavailable"));
    };
    let started = Instant::now();
    #[cfg(unix)]
    let stdout = DeadlinePipe::new(stdout, started + timeout);
    let reader = drain_stdout(stdout, limit);
    let exit_wake = CaptureExitWake::new(pid);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                exit_wake.wait(timeout.saturating_sub(started.elapsed()));
            }
            Ok(None) => {
                kill_capture_group(&mut child, pid);
                let _ = reader.join();
                return Ok(None);
            }
            Err(error) => {
                kill_capture_group(&mut child, pid);
                let _ = reader.join();
                return Err(error);
            }
        }
    };
    #[cfg(unix)]
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
    }
    let captured = match reader
        .join()
        .map_err(|_| std::io::Error::other("capture stdout reader panicked"))?
    {
        Ok(captured) => captured,
        Err(error) if error.kind() == std::io::ErrorKind::TimedOut => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(Some((status, captured)))
}

#[cfg(test)]
fn capture_with_deadline(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
    limit: usize,
) -> std::io::Result<Option<(ExitStatus, CapturedStdout)>> {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    capture_command_with_deadline(command, timeout, limit)
}

pub(crate) fn capture_bounded_command(command: Command, timeout: Duration) -> Option<Vec<u8>> {
    let (status, out) =
        capture_command_with_deadline(command, timeout, GIT_CAPTURE_LIMIT_BYTES).ok()??;
    (status.success() && !out.overflow).then_some(out.bytes)
}

pub(crate) fn capture_startup_repo_command(
    command: Command,
    timeout: Duration,
) -> std::io::Result<Option<Vec<u8>>> {
    capture_optional_probe_with_limit(command, timeout, 16 * 1024 * 1024)
}

pub(crate) fn capture_optional_probe(
    command: Command,
    timeout: Duration,
) -> std::io::Result<Option<Vec<u8>>> {
    capture_optional_probe_with_limit(command, timeout, GIT_CAPTURE_LIMIT_BYTES)
}

fn capture_optional_probe_with_limit(
    command: Command,
    timeout: Duration,
    limit: usize,
) -> std::io::Result<Option<Vec<u8>>> {
    let Some((status, out)) = capture_command_with_deadline(command, timeout, limit)? else {
        return Err(std::io::Error::from(std::io::ErrorKind::TimedOut));
    };
    if out.overflow {
        return Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge));
    }
    Ok(status.success().then_some(out.bytes))
}

#[cfg(unix)]
pub(crate) fn lock_store(file: &std::fs::File, step: &'static str) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let started = Instant::now();
    let mut reported_at = Duration::ZERO;
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            if reported_at > Duration::ZERO {
                eprintln!(
                    "[memory-health] step={step} status=recovered reason=lock_contended waited_ms={}",
                    started.elapsed().as_millis()
                );
            }
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if !matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ) {
            return Err(error);
        }
        let waited = started.elapsed();
        if waited - reported_at >= Duration::from_secs(2) {
            reported_at = waited;
            eprintln!(
                "[memory-health] step={step} status=partial reason=lock_contended elapsed_ms={} action=waiting_for_lock",
                waited.as_millis()
            );
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn capture_repo_command(command: Command, timeout: Duration) -> Option<Vec<u8>> {
    capture_bounded_command(command, timeout)
}

pub(crate) fn capture_repo_probe(
    program: &str,
    args: &[&str],
    cwd: &Path,
    secs: u64,
) -> Option<String> {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    let out = capture_bounded_command(command, Duration::from_secs(secs))?;
    let s = String::from_utf8_lossy(&out).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(test)]
fn git_capture(args: &[&str], cwd: &Path, secs: u64) -> Option<String> {
    capture_repo_probe("git", args, cwd, secs)
}

fn run_identity_probe(mut command: Command) -> std::io::Result<Option<Vec<u8>>> {
    command.env("GIT_OPTIONAL_LOCKS", "0");
    capture_optional_probe(command, Duration::from_millis(300))
}

pub(crate) fn identity_probe(command: Command, step: &'static str) -> Option<Vec<u8>> {
    match run_identity_probe(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "[turn-phase-partial] step={step} status=partial io={:?} action=continue_with_workspace_identity",
                error.kind()
            );
            None
        }
    }
}

#[cfg(test)]
pub fn main_worktree_from_porcelain(out: &str) -> Option<PathBuf> {
    out.lines()
        .find_map(|l| l.strip_prefix("worktree "))
        .map(|p| PathBuf::from(p.trim()))
        .filter(|p| !p.as_os_str().is_empty())
}

fn main_worktree_from_porcelain_z(out: &[u8]) -> Option<PathBuf> {
    let raw = out
        .split_inclusive(|byte| *byte == 0)
        .find_map(|field| field.strip_suffix(&[0])?.strip_prefix(b"worktree "))?;
    if raw.is_empty() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt as _;
        Some(PathBuf::from(std::ffi::OsString::from_vec(raw.to_vec())))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(raw).ok().map(PathBuf::from)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn canonical_repo_root(workspace: &Path) -> PathBuf {
    if !identity_enabled() {
        return workspace.to_path_buf();
    }
    canonical_repo_root_enabled(workspace).unwrap_or_else(|| workspace.to_path_buf())
}

fn canonical_repo_root_enabled(workspace: &Path) -> Option<PathBuf> {
    if std::env::var_os("GIT_DIR").is_none()
        && std::env::var_os("GIT_WORK_TREE").is_none()
        && let Some(root) = local_checkout_root(workspace)
    {
        let same = match (root.canonicalize(), workspace.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => root == workspace,
        };
        return Some(if same { workspace.to_path_buf() } else { root });
    }
    let mut command = Command::new("git");
    command
        .args(crate::agent::harness::GIT_NO_WORKSPACE_EXEC)
        .args(["worktree", "list", "--porcelain", "-z"])
        .current_dir(workspace);
    let out = match run_identity_probe(command) {
        Ok(Some(out)) => out,
        Ok(None) => return Some(workspace.to_path_buf()),
        Err(error) => {
            eprintln!(
                "[turn-phase-partial] step=repository_root status=partial io={:?} action=continue_with_workspace_identity",
                error.kind()
            );
            return None;
        }
    };
    let main = main_worktree_from_porcelain_z(&out)?;
    let same = match (main.canonicalize(), workspace.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => main == workspace,
    };
    Some(if same { workspace.to_path_buf() } else { main })
}

fn local_checkout_root(workspace: &Path) -> Option<PathBuf> {
    let canonical = workspace.canonicalize().ok()?;
    for candidate in canonical.ancestors() {
        let marker = candidate.join(".git");
        let metadata = match std::fs::symlink_metadata(&marker) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        };
        if !metadata.is_dir() {
            return None;
        }
        return (marker.join("HEAD").is_file()
            && marker.join("config").is_file()
            && marker.join("objects").is_dir())
        .then_some(candidate.to_path_buf());
    }
    None
}

fn slug_for_root(root: &Path) -> Option<String> {
    if let Some(slug) = crate::agent::tools::work_landing::load_context(root).and_then(|c| c.repo) {
        return Some(slug);
    }
    let mut command = Command::new("git");
    command
        .args(crate::agent::harness::GIT_NO_WORKSPACE_EXEC)
        .args(["remote", "get-url", "origin"])
        .current_dir(root);
    let bytes = identity_probe(command, "repository_slug")?;
    let url = String::from_utf8_lossy(&bytes);
    crate::agent::tools::work_landing::normalize_remote(&url).map(|s| s.slug())
}

type IdentityCache = Mutex<HashMap<(PathBuf, bool), RepoIdentity>>;

fn identity_cache() -> &'static IdentityCache {
    static CACHE: OnceLock<IdentityCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn repo_identity(workspace: &Path) -> RepoIdentity {
    let enabled = identity_enabled();
    let cache_key = (
        std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf()),
        enabled,
    );
    if let Ok(cache) = identity_cache().lock()
        && let Some(hit) = cache.get(&cache_key)
    {
        return hit.clone();
    }
    let gate = identity_gate(format!("identity:{enabled}:{}", cache_key.0.display()));
    let _probing = gate.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Ok(cache) = identity_cache().lock()
        && let Some(hit) = cache.get(&cache_key)
    {
        return hit.clone();
    }
    let root = if enabled {
        canonical_repo_root_enabled(workspace)
    } else {
        Some(workspace.to_path_buf())
    };
    let cacheable = root.is_some();
    let root = root.unwrap_or_else(|| workspace.to_path_buf());
    let slug = if cacheable {
        slug_for_root_memoized(&root)
    } else {
        slug_for_root(&root)
    };
    let id = RepoIdentity {
        key: workspace_key(&root),
        slug,
        root,
    };
    if cacheable && let Ok(mut cache) = identity_cache().lock() {
        cache.insert(cache_key, id.clone());
    }
    id
}

fn identity_gate(key: String) -> std::sync::Arc<Mutex<()>> {
    static GATES: OnceLock<Mutex<HashMap<String, std::sync::Arc<Mutex<()>>>>> = OnceLock::new();
    let gates = GATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut gates = gates.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    std::sync::Arc::clone(gates.entry(key).or_default())
}

fn slug_for_root_memoized(root: &Path) -> Option<String> {
    type SlugCache = Mutex<HashMap<PathBuf, Option<String>>>;
    static SLUGS: OnceLock<SlugCache> = OnceLock::new();
    let cache = SLUGS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(slug) = crate::agent::tools::work_landing::load_context(root).and_then(|c| c.repo) {
        return Some(slug);
    }
    let remembered = |cache: &SlugCache| {
        cache
            .lock()
            .ok()
            .and_then(|slugs| slugs.get(root).cloned())
    };
    if let Some(hit) = remembered(cache) {
        return hit;
    }
    let gate = identity_gate(format!("slug:{}", root.display()));
    let _probing = gate.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(hit) = remembered(cache) {
        return hit;
    }
    let slug = slug_for_root(root);
    if let Ok(mut slugs) = cache.lock() {
        slugs.insert(root.to_path_buf(), slug.clone());
    }
    slug
}

pub fn matches_project(workspace: &Path, root: &Path, key: &str) -> bool {
    let identity = repo_identity(workspace);
    identity.root == root && identity.key == key
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/workspace_store__storage_identity_tests.rs"]
mod storage_identity_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/workspace_store__storage_atomic_tests.rs"]
pub(crate) mod storage_atomic_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/workspace_store__tests.rs"]
mod tests;
