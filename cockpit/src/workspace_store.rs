//! Shared workspace-scoped persistence paths and **repo identity**.
//!
//! Cockpit features that store per-workspace state should use this module for
//! stable file naming instead of each feature inventing its own key/hash scheme.
//!
//! # Repo identity — why a workspace is not a repository
//!
//! [`workspace_key`] keys on the *directory path*, which is right for
//! directory-scoped state and wrong for anything that accumulates knowledge
//! about a **repository**. A git worktree of angel0 (`git worktree add`) is a
//! different directory, so it minted a different key — and every organ that
//! learns per-repo (the Repo Dossier's ledger events, The Cut's authored-diff
//! manifest) filed its evidence under a throwaway identity that nothing would
//! ever read again. The substrate factory (`scripts/cut-forge.mjs`) runs
//! `angel --task` in exactly such disposable worktrees, so its entire output
//! was being written to a new, dead repo every run. Same for a `/cd` into a
//! subdirectory: `angel0/cockpit` was a different "repo" than `angel0`.
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

use crate::sandbox::process_owner::OwnedCommandExt;
pub(crate) mod private_io;

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub fn angel_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".angel0")
}

pub fn angel_subdir(name: &str) -> PathBuf {
    angel_dir().join(name)
}

/// A stable, filesystem-safe key for a workspace path: a sanitized,
/// length-bounded prefix plus a deterministic hash suffix.
pub fn workspace_key(workspace: &Path) -> String {
    let s = workspace.to_string_lossy();
    let mut hasher = DefaultHasher::new();
    if workspace.to_str().is_some() {
        // Keep existing keys for all UTF-8 paths; invalid paths must not
        // collapse to the same replacement-character spelling.
        s.hash(&mut hasher);
    } else {
        b"angel-workspace-non-utf8-v1".hash(&mut hasher);
        workspace.as_os_str().as_encoded_bytes().hash(&mut hasher);
    }
    let h = hasher.finish();
    let sanitized: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed: String = sanitized.trim_matches('-').chars().take(60).collect();
    let trimmed = if trimmed.is_empty() {
        "ws".to_string()
    } else {
        trimmed
    };
    format!("{trimmed}-{h:016x}")
}

pub fn workspace_json_path_in(dir: &Path, workspace: &Path) -> PathBuf {
    dir.join(format!("{}.json", workspace_key(workspace)))
}

/// Atomically publish a private state file and make the file plus directory
/// entry durable before returning. A unique same-directory temporary keeps
/// concurrent cockpit processes from corrupting each other's staging bytes.
pub(crate) fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let redacted = crate::secrets::redact_bytes(bytes);
    let bytes = redacted.as_slice();
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create state directory {}: {error}", parent.display()))?;
    static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("state path has no file name: {}", path.display()))?
        .to_string_lossy();
    let temp = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        serial
    ));
    #[cfg(test)]
    storage_atomic_tests::before_atomic_open(&temp);
    let mut owns_temp = false;
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options
            .open(&temp)
            .map_err(|error| format!("open state temp {}: {error}", temp.display()))?;
        owns_temp = true;
        file.write_all(bytes)
            .map_err(|error| format!("write state temp {}: {error}", temp.display()))?;
        file.sync_all()
            .map_err(|error| format!("sync state temp {}: {error}", temp.display()))?;
        drop(file);
        std::fs::rename(&temp, path).map_err(|error| {
            format!(
                "publish state {} from {}: {error}",
                path.display(),
                temp.display()
            )
        })?;
        owns_temp = false;
        #[cfg(unix)]
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("sync state directory {}: {error}", parent.display()))?;
        Ok(())
    })();
    if result.is_err() && owns_temp {
        let _ = std::fs::remove_file(&temp);
    }
    result
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
/// so timeout cleanup also closes pipes inherited by grandchildren.
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
    let reader = drain_stdout(stdout, limit);
    let started = Instant::now();
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
    // Reap that private group before joining the reader, making the deadline
    // guarantee cover the whole spawned tree rather than only direct `git`.
    #[cfg(unix)]
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
    }
    let captured = reader
        .join()
        .map_err(|_| std::io::Error::other("capture stdout reader panicked"))??;
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
    if let Some(slug) = crate::tools::work_landing::load_context(root).and_then(|c| c.repo) {
        return Some(slug);
    }
    let mut command = Command::new("git");
    command
        .args(["remote", "get-url", "origin"])
        .current_dir(root);
    let bytes = identity_probe(command, "repository_slug")?;
    let url = String::from_utf8_lossy(&bytes);
    crate::tools::work_landing::normalize_remote(&url).map(|s| s.slug())
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
    let root = if enabled {
        canonical_repo_root_enabled(workspace)
    } else {
        Some(workspace.to_path_buf())
    };
    let cacheable = root.is_some();
    let root = root.unwrap_or_else(|| workspace.to_path_buf());
    let id = RepoIdentity {
        key: workspace_key(&root),
        slug: slug_for_root(&root),
        root,
    };
    if cacheable && let Ok(mut cache) = identity_cache().lock() {
        cache.insert(cache_key, id.clone());
    }
    id
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
mod storage_identity_tests;

#[cfg(test)]
mod storage_atomic_tests;

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn r04e_contended_store_lock_waits_and_never_drops_the_write() {
        use std::os::fd::AsRawFd;
        let _guard = crate::tests::env_lock();
        let path = std::env::temp_dir().join(format!("r04e-lock-{}", std::process::id()));
        let held = std::fs::File::create(&path).unwrap();
        let contender = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        super::lock_store(&held, "r04e_test_lock").unwrap();
        let held_fd = held.as_raw_fd();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(2500));
            // SAFETY: the descriptor stays open in `held` until this thread has released it.
            assert_eq!(unsafe { libc::flock(held_fd, libc::LOCK_UN) }, 0);
        });
        let started = std::time::Instant::now();
        // The contender waits through one contention receipt and then acquires;
        // the write is never dropped.
        super::lock_store(&contender, "r04e_test_lock").unwrap();
        assert!(started.elapsed() >= std::time::Duration::from_millis(2000));
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        releaser.join().unwrap();
        drop(contender);
        drop(held);
        std::fs::remove_file(path).unwrap();
    }

    use super::*;

    #[test]
    fn stdout_drain_retains_a_bounded_prefix_and_marks_overflow() {
        let input = vec![b'x'; 128 * 1024];
        let captured = drain_stdout(std::io::Cursor::new(input), 32 * 1024)
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(captured.bytes.len(), 32 * 1024);
        assert!(captured.bytes.iter().all(|byte| *byte == b'x'));
        assert!(captured.overflow);
    }

    #[cfg(unix)]
    #[test]
    fn command_capture_enforces_its_own_deadline_and_kills_descendants() {
        let started = Instant::now();
        let captured = capture_with_deadline(
            "sh",
            &["-c", "sleep 30"],
            Path::new("."),
            Duration::from_millis(50),
            1024,
        )
        .unwrap();
        assert!(captured.is_none(), "deadline should report a timeout");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "deadline cleanup must not wait for the child tree"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn capture_exit_notification_observes_real_child_and_preserves_exit_status() {
        let mut child = Command::new("sh")
            .args(["-c", "sleep 0.02; exit 7"])
            .spawn_owned()
            .unwrap();
        let wake = CaptureExitWake::new(child.id());
        if wake.fd.is_none() {
            child.wait().unwrap();
            eprintln!("pidfd unavailable; bounded polling fallback remains active");
            return;
        }
        wake.wait(Duration::from_secs(2));
        assert_eq!(child.try_wait().unwrap().unwrap().code(), Some(7));
        CaptureExitWake { fd: None }.wait(Duration::ZERO);
    }

    #[cfg(unix)]
    #[test]
    fn command_capture_reaps_successful_parents_background_pipe_holders() {
        let started = Instant::now();
        let (status, captured) = capture_with_deadline(
            "sh",
            &["-c", "sleep 30 & printf okay"],
            Path::new("."),
            Duration::from_secs(1),
            1024,
        )
        .unwrap()
        .expect("the direct child should finish before its deadline");
        assert!(status.success());
        assert_eq!(captured.bytes, b"okay");
        assert!(!captured.overflow);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "a grandchild holding stdout must not stall reader join"
        );
    }

    #[test]
    fn workspace_key_is_deterministic_and_distinct() {
        let a = workspace_key(Path::new("/home/u/alpha"));
        let a2 = workspace_key(Path::new("/home/u/alpha"));
        let b = workspace_key(Path::new("/home/u/beta"));
        assert_eq!(a, a2, "same path -> same stable file key");
        assert_ne!(a, b, "different paths get distinct keys");
        assert!(!a.contains('/'), "key is filesystem-safe");
    }

    #[test]
    fn workspace_json_path_uses_key_under_namespace_dir() {
        let dir = Path::new("/tmp/angel/loops");
        let path = workspace_json_path_in(dir, Path::new("/home/u/alpha"));
        assert_eq!(path.parent(), Some(dir));
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("json"));
    }

    #[test]
    fn private_atomic_write_publishes_complete_replacement() {
        static TEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = TEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "angel-private-atomic-{}-{serial}",
            std::process::id()
        ));
        let path = dir.join("state.json");
        write_private_atomic(&path, b"first").unwrap();
        write_private_atomic(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn project_match_requires_both_canonical_root_and_key() {
        let workspace = Path::new("/home/u/alpha");
        let identity = repo_identity(workspace);
        assert!(matches_project(workspace, &identity.root, &identity.key));
        assert!(!matches_project(
            workspace,
            Path::new("/home/u/beta"),
            &identity.key
        ));
        assert!(!matches_project(workspace, &identity.root, "wrong-key"));
    }

    // --- repo identity: the main worktree is the repository. ---

    #[test]
    fn main_worktree_is_the_first_porcelain_entry() {
        // `git worktree list --porcelain` from ANY worktree prints the main one
        // first — that is what makes it the canonical identity.
        let out = "worktree /home/u/repo\nHEAD abc123\nbranch refs/heads/main\n\n\
                   worktree /tmp/cut-forge/item-3\nHEAD abc123\nbranch refs/heads/item-3\n";
        assert_eq!(
            main_worktree_from_porcelain(out),
            Some(PathBuf::from("/home/u/repo"))
        );
    }

    #[test]
    fn main_worktree_handles_bare_and_detached_and_garbage() {
        // Bare main repo: the first entry is the bare dir (+ a `bare` line).
        assert_eq!(
            main_worktree_from_porcelain("worktree /srv/git/repo.git\nbare\n"),
            Some(PathBuf::from("/srv/git/repo.git"))
        );
        // Detached worktree: `detached` instead of `branch` — still an entry.
        assert_eq!(
            main_worktree_from_porcelain("worktree /home/u/repo\nHEAD abc\ndetached\n"),
            Some(PathBuf::from("/home/u/repo"))
        );
        // Never panics on nonsense / empty output.
        assert_eq!(main_worktree_from_porcelain(""), None);
        assert_eq!(main_worktree_from_porcelain("HEAD abc\n"), None);
        assert_eq!(main_worktree_from_porcelain("worktree \n"), None);
    }

    #[test]
    fn a_non_repo_path_keeps_its_own_identity() {
        // Not a git repo → the workspace IS the identity (the legacy behavior),
        // and nothing crashes.
        let ws = std::env::temp_dir().join(format!("angel-ws-store-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&ws);
        let id = repo_identity(&ws);
        assert_eq!(id.root, ws, "no git → fall back to the workspace path");
        assert_eq!(id.key, workspace_key(&ws));
        assert_eq!(id.slug, None);
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// `git worktree add` a real linked worktree of a throwaway repo and prove it
    /// inherits the main checkout's identity — the whole point of this module.
    /// Uses a fresh temp repo, so the source tree's `.git` is never touched.
    #[test]
    fn a_linked_worktree_inherits_the_main_repos_identity() {
        let _guard = crate::tests::env_lock();
        let base = std::env::temp_dir().join(format!("angel-wt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let repo = base.join("main");
        let linked = base.join("linked");
        std::fs::create_dir_all(&repo).unwrap();

        let git = |args: &[&str], cwd: &Path| {
            Command::new("git")
                .args(args)
                .current_dir(cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        };
        // A repo needs one commit before `worktree add` will work.
        let setup = git(&["init", "-q"], &repo)
            .map(|s| s.success())
            .unwrap_or(false)
            && git(&["config", "user.email", "t@t"], &repo).is_ok()
            && git(&["config", "user.name", "t"], &repo).is_ok()
            && std::fs::write(repo.join("f.txt"), "x").is_ok()
            && git(&["add", "-A"], &repo).is_ok()
            && git(&["commit", "-qm", "c"], &repo)
                .map(|s| s.success())
                .unwrap_or(false)
            && git(
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    "angel-linked-test",
                    linked.to_str().unwrap(),
                    "HEAD",
                ],
                &repo,
            )
            .map(|s| s.success())
            .unwrap_or(false);
        if !setup {
            let _ = std::fs::remove_dir_all(&base); // no git available — skip
            return;
        }

        // The linked worktree — a different directory, and a DIFFERENT
        // workspace_key — resolves to the main checkout, so its evidence lands
        // in the main repo's dossier instead of a key that dies with it.
        assert_ne!(workspace_key(&linked), workspace_key(&repo));
        let id = repo_identity(&linked);
        assert_eq!(id.root.canonicalize().ok(), repo.canonicalize().ok());
        assert_eq!(id.key, repo_identity(&repo).key);
        // …and a subdirectory of the worktree resolves there too.
        let sub = linked.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(repo_identity(&sub).key, id.key);

        // Durable knowledge identity is deliberately repository-wide, but live
        // workspace/Git state must remain checkout-specific. This is the
        // branch-cache boundary: consulting any process-local identity or
        // confinement cache must never retarget one checkout to another.
        let main_branch = git_capture(&["rev-parse", "--abbrev-ref", "HEAD"], &repo, 5).unwrap();
        assert_eq!(
            git_capture(&["rev-parse", "--abbrev-ref", "HEAD"], &linked, 5).as_deref(),
            Some("angel-linked-test")
        );
        let main_boundary = crate::harness::WorkspaceBoundary::cached(&repo);
        let linked_boundary = crate::harness::WorkspaceBoundary::cached(&linked);
        assert_eq!(main_boundary.repository.key, linked_boundary.repository.key);
        assert_ne!(
            main_boundary.canonical_root, linked_boundary.canonical_root,
            "linked worktrees must retain distinct live filesystem boundaries"
        );
        std::fs::write(repo.join("f.txt"), "main dirty").unwrap();
        std::fs::write(linked.join("f.txt"), "linked dirty").unwrap();
        assert_ne!(
            crate::harness::workspace_fingerprint(&repo),
            crate::harness::workspace_fingerprint(&linked),
            "branch-local dirty state must never share a cached fingerprint"
        );
        assert_eq!(
            git_capture(&["rev-parse", "--abbrev-ref", "HEAD"], &repo, 5).as_deref(),
            Some(main_branch.as_str())
        );
        assert_eq!(
            git_capture(&["rev-parse", "--abbrev-ref", "HEAD"], &linked, 5).as_deref(),
            Some("angel-linked-test")
        );

        let _ = git(
            &["worktree", "remove", "--force", linked.to_str().unwrap()],
            &repo,
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn identity_can_be_switched_off() {
        // The escape hatch restores pure path-keying (no git spawn at all).
        let _guard = crate::tests::env_lock();
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_REPO_IDENTITY", "0") };
        let root = canonical_repo_root(&crate_dir);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_REPO_IDENTITY") };
        assert_eq!(root, crate_dir, "off → the workspace is its own repo again");
    }
}
