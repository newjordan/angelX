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

// The key, the atomic writers and the private lock live in the Kernel's host
// services (`angel_live::host::workspace_store`) so the realm host stands on
// the same code; they are re-exported here at their old paths.
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

/// Remove one state file and durably publish the deletion. Missing files are
/// already in the requested state.
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

// ---------------------------------------------------------------------------
// Repo identity — the repository a workspace belongs to.
// ---------------------------------------------------------------------------

/// The repository a workspace belongs to: its canonical root (the **main
/// worktree**), the stable key every per-repo store files under, and the
/// `owner/repo` slug when the repo has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoIdentity {
    /// The main worktree's root — never a linked worktree, never a subdirectory.
    /// Falls back to the workspace itself when this is not a git repo.
    pub root: PathBuf,
    /// `workspace_key(root)`. The join key for the ledger, the dossier artifact,
    /// The Cut's manifest, and the work-context store.
    pub key: String,
    /// `owner/repo`, from the confirmed work context if there is one, else the
    /// `origin` remote. `None` for a repo with no remote (purely local work).
    pub slug: Option<String>,
}

/// Whether repo-identity resolution is on (`ANGEL_REPO_IDENTITY`, default
/// **on**). `0` restores the legacy path-only keying: every directory is its
/// own "repo" again.
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

/// Wake on the owned child's exit instead of imposing a 10ms floor on every
/// short metadata probe. Older kernels and other platforms keep bounded polling.
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
                // SAFETY: pid names our unreaped child; flags=0 and no pointers.
                let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, _pid, 0_u32) };
                (fd >= 0).then(|| {
                    // SAFETY: a successful syscall returns a new owned CLOEXEC fd.
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
            // SAFETY: one live descriptor, owned for the duration of poll.
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

/// A pipe can outlive its direct child when a descendant changes sessions.
/// Poll before each read so the caller's deadline also bounds EOF, without
/// leaving a reader thread blocked for that descendant's lifetime.
#[cfg(unix)]
pub(crate) struct DeadlinePipe<R> {
    pipe: R,
    deadline: Instant,
}

#[cfg(unix)]
impl<R> DeadlinePipe<R> {
    /// The owned pipe must have one reader, so readiness makes its next read
    /// nonblocking. Shared by bounded metadata capture and streamed evidence.
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
            // SAFETY: this thread owns the pipe and one live pollfd. No other
            // reader can consume its ready bytes before the following read.
            let ready = unsafe { libc::poll(&mut descriptor, 1, millis) };
            if ready == 0 {
                // Recheck the absolute deadline; very large caller budgets
                // can require more than one poll's i32 millisecond allowance.
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
        // SAFETY: this child was placed in a private process group at spawn.
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Capture one command without trusting an external timeout utility.
///
/// The reader drains concurrently and retains only `limit` bytes, so a noisy
/// child cannot deadlock on a full pipe or grow the cockpit without bound.
/// `None` means the deadline expired. The child leads a private process group
/// so timeout cleanup also closes pipes inherited by grandchildren. On Unix,
/// the same deadline bounds EOF even if a descendant escapes that group.
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
    // A successful parent can still leave a grandchild holding stdout open.
    // Reap that private group before joining the reader. The reader's own
    // deadline also bounds an out-of-group descendant retaining the pipe.
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

/// Capture a preconfigured metadata command within one caller-owned budget.
/// Exact environment and executable pinning stay on `command`; this boundary
/// supplies process-tree cleanup, concurrent draining, and the shared 1 MiB cap.
/// Callers must reject `None` rather than interpreting a timeout or overflow as
/// an empty successful response.
pub(crate) fn capture_bounded_command(command: Command, timeout: Duration) -> Option<Vec<u8>> {
    let (status, out) =
        capture_command_with_deadline(command, timeout, GIT_CAPTURE_LIMIT_BYTES).ok()??;
    (status.success() && !out.overflow).then_some(out.bytes)
}

/// Startup metadata only. Its larger allowance admits tracked path lists from
/// large repositories; timeout/overflow remains unavailable, never an empty tree.
pub(crate) fn capture_startup_repo_command(
    command: Command,
    timeout: Duration,
) -> std::io::Result<Option<Vec<u8>>> {
    capture_optional_probe_with_limit(command, timeout, 16 * 1024 * 1024)
}

/// Distinguish an unavailable probe from a valid negative response (e.g. non-Git
/// workspace). Optional turn telemetry can report partial evidence and continue.
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

/// Take the advisory store lock. Contention is REPORTED, never resolved by
/// dropping the write: a durable store operation waits for the lock, and every
/// 2 s of waiting emits a labelled `[memory-health]` receipt line so the stall is
/// visible (R04e progress truth). Only a real lock error returns `Err`.
#[cfg(unix)]
pub(crate) fn lock_store(file: &std::fs::File, step: &'static str) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let started = Instant::now();
    let mut reported_at = Duration::ZERO;
    loop {
        // SAFETY: caller owns this descriptor through the call and its guard.
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

/// Repository-oriented name retained for the exact Git evidence callers.
pub(crate) fn capture_repo_command(command: Command, timeout: Duration) -> Option<Vec<u8>> {
    capture_bounded_command(command, timeout)
}

/// Run a repository metadata probe with a whole-tree deadline and bounded
/// output. Shared by repo identity and the first-contact work landing so Git
/// and networked `gh` lookups have the same cross-platform failure boundary.
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

/// Optional repository metadata: preserve valid empty responses and label
/// unavailable probes before falling back, without stopping the turn.
fn run_identity_probe(mut command: Command) -> std::io::Result<Option<Vec<u8>>> {
    command.env("GIT_OPTIONAL_LOCKS", "0");
    capture_optional_probe(command, Duration::from_millis(300))
}

/// Optional repository metadata: preserve valid empty responses and label
/// unavailable probes before falling back, without stopping the turn.
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

/// The main worktree's path out of `git worktree list --porcelain` — **pure**.
///
/// git prints the main worktree first and the linked worktrees after it, always,
/// which makes this the one answer that resolves a linked worktree, a
/// subdirectory, a detached checkout, and a bare repo to the same identity. A
/// submodule reports *its own* main worktree (a submodule is a separate
/// repository with a separate remote), which is likewise correct.
#[cfg(test)]
pub fn main_worktree_from_porcelain(out: &str) -> Option<PathBuf> {
    out.lines()
        .find_map(|l| l.strip_prefix("worktree "))
        .map(|p| PathBuf::from(p.trim()))
        .filter(|p| !p.as_os_str().is_empty())
}

/// Read the first NUL-delimited worktree path without lossy conversion or trim.
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

/// The repository root for `workspace`: the main worktree when git knows one,
/// else `workspace` unchanged.
///
/// When git's answer names the *same directory* as `workspace`, the workspace
/// path is returned **as given** rather than git's canonicalized spelling — so a
/// repo that is already keyed by its own path keeps its exact key and its whole
/// accumulated history, even if some ancestor is a symlink.
#[cfg_attr(not(test), allow(dead_code))]
pub fn canonical_repo_root(workspace: &Path) -> PathBuf {
    if !identity_enabled() {
        return workspace.to_path_buf();
    }
    canonical_repo_root_enabled(workspace).unwrap_or_else(|| workspace.to_path_buf())
}

/// Resolve a repository root after the caller has already captured the
/// identity-mode setting. This keeps the cache key and resolved value in the
/// same mode when tests or an embedding process switch the compatibility
/// escape hatch between calls. `None` is specifically a transient or malformed
/// Git probe, so callers can fall back without permanently memoizing a
/// workspace-only identity.
fn canonical_repo_root_enabled(workspace: &Path) -> Option<PathBuf> {
    // The usual checkout has a complete `.git` directory. Finding the nearest
    // such marker is local metadata I/O, so it avoids letting a busy process
    // table turn an ordinary subdirectory into a transient workspace-only
    // identity. Stop at the first `.git` entry: a file is a linked-worktree or
    // submodule marker and must use Git below to find its main worktree.
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
        // Git answered definitively that this is not a repository. This
        // workspace-only identity is stable and may be cached.
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
        // Avoid treating an empty or unrelated `.git` directory as a trusted
        // identity boundary. Real non-bare repositories carry these three
        // pieces of local Git metadata.
        return (marker.join("HEAD").is_file()
            && marker.join("config").is_file()
            && marker.join("objects").is_dir())
        .then_some(candidate.to_path_buf());
    }
    None
}

/// The `owner/repo` slug for a repo root: the user's confirmed work context
/// first (it is the authoritative, human-blessed answer and costs no spawn),
/// else the `origin` remote — which a linked worktree shares with its main
/// checkout, so a worktree is no longer anonymous.
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

/// Repository identity depends on the compatibility mode as well as the
/// workspace path. In particular, caching a path-only identity while
/// `ANGEL_REPO_IDENTITY=0` must not make a later enabled lookup forget that
/// the workspace belongs to its repository.
type IdentityCache = Mutex<HashMap<(PathBuf, bool), RepoIdentity>>;

fn identity_cache() -> &'static IdentityCache {
    static CACHE: OnceLock<IdentityCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The repository `workspace` belongs to. Memoized per process: the ledger
/// stamps this on **every recorded command**, so the git spawns must happen once
/// per distinct workspace, not once per record. The cache is keyed by the
/// canonicalized path so two spellings of one directory (relative/absolute,
/// trailing separators, symlinked temp dirs) share one entry instead of
/// re-running the git probes.
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
    // One probe per workspace at a time: startup asks for the same identity from
    // its main thread and from the threads that warm it, and the later asker
    // should take the first one's answer rather than spawn the same git again.
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
        // A subdirectory, the skills directory and the checkout root itself all
        // resolve to one repository: ask git for its origin once per root.
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

/// Per-key mutexes that serialize one probe at a time without serializing
/// unrelated probes. Entries are a few bytes and live for the process.
fn identity_gate(key: String) -> std::sync::Arc<Mutex<()>> {
    static GATES: OnceLock<Mutex<HashMap<String, std::sync::Arc<Mutex<()>>>>> = OnceLock::new();
    let gates = GATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut gates = gates.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    std::sync::Arc::clone(gates.entry(key).or_default())
}

/// [`slug_for_root`] remembered per repository root, for the same lifetime and
/// under the same conditions as the identity cache that calls it (the caller
/// only gets here for a root that git resolved). The confirmed work context is
/// still read on every call, so a slug the operator sets mid-session wins.
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

/// True only when both serialized identity fields match the canonical
/// repository containing `workspace`. Instruction-bearing stores use this as
/// their common fail-closed boundary: a key alone or a path alone is never
/// sufficient, and legacy records with either field missing remain inert.
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
