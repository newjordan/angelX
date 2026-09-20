//! Cheap Git-backed workspace fingerprints for evidence-based finalization.

use super::*;
use crate::sandbox::process_owner::OwnedCommandExt;
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const HASH_BUFFER_BYTES: usize = 64 * 1024;
const MAX_EXACT_FINGERPRINT_BYTES: u64 = 1024 * 1024;
const MAX_STREAMED_PATH_BYTES: usize = 1024 * 1024;
const GIT_STREAM_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const ACTIVE_WORKSPACE_PATHSPECS: &[&str] = &[
    ".",
    ":(exclude)off-limits/**",
    ":(top,exclude).angel-experiment-tmp/**",
];

const PINNED_GIT_CONFIG: &[&str] = &[
    "--no-pager",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.untrackedCache=false",
    "-c",
    "core.ignoreStat=false",
    "-c",
    "core.trustctime=true",
    "-c",
    "core.filemode=true",
    "-c",
    "core.hooksPath=/dev/null",
];

/// Incremental SHA-256 for bounded-memory workspace evidence. Keep framing at
/// the call sites while sharing the platform implementation with one-shot hashes.
#[derive(Clone)]
struct StreamingSha256(ring::digest::Context);

impl StreamingSha256 {
    fn new() -> Self {
        Self(ring::digest::Context::new(&ring::digest::SHA256))
    }

    fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    fn finalize(self) -> [u8; 32] {
        self.0.finish().as_ref().try_into().expect("SHA-256 digest")
    }
}

fn hex_digest(digest: [u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(64);
    for byte in digest {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// Hash a reader in constant memory with platform-optimized SHA-256. Callers
/// remain responsible for framing and any before/after identity checks around
/// a mutable file.
pub(crate) fn sha256_reader_hex(reader: &mut impl Read) -> std::io::Result<String> {
    let mut hasher = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finish();
    let mut text = String::with_capacity(64);
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(text, "{byte:02x}");
    }
    Ok(text)
}

fn workspace_is_home(root: &Path, home: Option<&OsStr>) -> bool {
    let Some(home) = home.filter(|value| !value.is_empty()) else {
        return false;
    };
    let home = Path::new(home);
    match (std::fs::canonicalize(root), std::fs::canonicalize(home)) {
        (Ok(root), Ok(home)) => root == home,
        _ => root == home,
    }
}

pub(crate) fn pinned_git_path() -> Option<&'static Path> {
    static PINNED_GIT: OnceLock<Option<PathBuf>> = OnceLock::new();
    PINNED_GIT
        .get_or_init(|| {
            ["/usr/bin/git", "/bin/git", "/usr/local/bin/git"]
                .into_iter()
                .map(PathBuf::from)
                .find_map(|path| {
                    path.is_file()
                        .then(|| std::fs::canonicalize(path).ok())
                        .flatten()
                })
        })
        .as_deref()
}

fn git_output(root: &Path, args: &[&str], deadline: Instant) -> Option<Vec<u8>> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return None;
    }
    crate::workspace_store::capture_repo_command(pinned_git_command(root, args), remaining)
}

/// Cheap changed-path coverage for typed Rust checks. No file-content walk or
/// Cargo invocation. Missing Git identity, hidden index entries, malformed or
/// capped output remains unknown instead of certifying a clean worktree.
pub(crate) fn changed_rust_paths_for_verification(root: &Path) -> Option<Vec<String>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let scopes = ["*.rs", ":(exclude)off-limits/**"];
    let changed = git_output(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--name-only",
            "--relative",
            "-z",
            "HEAD",
            "--",
            scopes[0],
            scopes[1],
        ],
        deadline,
    )?;
    let untracked = git_output(
        root,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            scopes[0],
            scopes[1],
        ],
        deadline,
    )?;
    let flags = git_output(
        root,
        &["ls-files", "-v", "-z", "--", scopes[0], scopes[1]],
        deadline,
    )?;
    if [changed.len(), untracked.len(), flags.len()]
        .into_iter()
        .any(|n| n > 2 * 1024 * 1024)
    {
        return None;
    }
    // `-v` lowercases assume-unchanged entries; S denotes skip-worktree.
    for row in flags.split(|b| *b == 0).filter(|row| !row.is_empty()) {
        if row.len() < 3 || row[0] != b'H' || row[1] != b' ' {
            return None;
        }
    }
    let mut paths = std::collections::BTreeSet::new();
    for raw in changed
        .split(|b| *b == 0)
        .chain(untracked.split(|b| *b == 0))
        .filter(|row| !row.is_empty())
    {
        paths.insert(std::str::from_utf8(raw).ok()?.to_owned());
        if paths.len() > 1024 {
            return None;
        }
    }
    Some(paths.into_iter().collect())
}

// Capture-enabled recovery owns these exact isolated roots. A process-global
// root map deliberately covers native metadata probes on child tool threads;
// parent workspaces and unconfigured turns do not inherit the restriction.
static CONFINED_RECOVERY_GIT: OnceLock<
    std::sync::Mutex<std::collections::BTreeMap<PathBuf, usize>>,
> = OnceLock::new();
pub(crate) struct ConfinedRecoveryGit(PathBuf);
impl ConfinedRecoveryGit {
    pub(crate) fn new(root: &Path) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let mut roots = CONFINED_RECOVERY_GIT
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| "recovery Git scope poisoned")?;
        *roots.entry(root.clone()).or_default() += 1;
        Ok(Self(root))
    }
}
impl Drop for ConfinedRecoveryGit {
    fn drop(&mut self) {
        if let Ok(mut roots) = CONFINED_RECOVERY_GIT.get_or_init(Default::default).lock()
            && let Some(count) = roots.get_mut(&self.0)
        {
            *count -= 1;
            if *count == 0 {
                roots.remove(&self.0);
            }
        }
    }
}

pub(crate) fn pinned_git_command(root: &Path, args: &[&str]) -> Command {
    let executable = pinned_git_path().unwrap_or_else(|| Path::new("git-unavailable"));
    let confined = CONFINED_RECOVERY_GIT
        .get()
        .and_then(|scopes| match scopes.lock() {
            Ok(roots) if roots.is_empty() => None,
            Ok(roots) => {
                let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
                roots
                    .keys()
                    .find(|owned| canonical.starts_with(owned))
                    .cloned()
            }
            Err(_) => Some(root.to_path_buf()),
        });
    let command = if let Some(owned) = confined {
        let policy = crate::sandbox::SandboxPolicy {
            writable_roots: vec![owned],
            allow_network: false,
            enforce: true,
            mandatory: true,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        };
        crate::sandbox::command(executable, std::iter::empty::<&str>(), &policy)
            .unwrap_or_else(|_| Command::new("/dev/null/recovery-git-sandbox-unavailable"))
    } else {
        Command::new(executable)
    };
    configure_pinned_git(command, root, args)
}

/// Only for the recovery stdout forwarder, which places this entire command
/// inside its own mandatory sandbox. Avoid nesting helpers: the outer helper
/// intentionally removes its policy environment before executing the payload.
pub(super) fn pinned_git_for_confined_forwarder(root: &Path, args: &[&str]) -> Command {
    let executable = pinned_git_path().unwrap_or_else(|| Path::new("git-unavailable"));
    configure_pinned_git(Command::new(executable), root, args)
}

fn configure_pinned_git(mut command: Command, root: &Path, args: &[&str]) -> Command {
    command
        .args(PINNED_GIT_CONFIG)
        .args(args)
        .current_dir(root)
        // These variables can redirect Git away from the workspace/index being
        // attested. Credential/pager configuration is irrelevant to local
        // evidence commands, so strip the entire redirect/config surface.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env_remove("GIT_EXTERNAL_DIFF")
        .env_remove("GIT_DIFF_OPTS")
        .env_remove("GIT_CONFIG")
        .env_remove("GIT_CONFIG_GLOBAL")
        .env_remove("GIT_CONFIG_SYSTEM")
        .env_remove("GIT_CONFIG_COUNT")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_NOSYSTEM")
        .env_remove("GIT_CEILING_DIRECTORIES")
        .env_remove("GIT_DISCOVERY_ACROSS_FILESYSTEM")
        .env_remove("GIT_LITERAL_PATHSPECS")
        .env_remove("GIT_GLOB_PATHSPECS")
        .env_remove("GIT_NOGLOB_PATHSPECS")
        .env_remove("GIT_ICASE_PATHSPECS")
        .env_remove("GIT_PREFIX")
        .env_remove("GIT_EXEC_PATH")
        .env_remove("GIT_NAMESPACE")
        .env_remove("GIT_SHALLOW_FILE")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_OPTIONAL_LOCKS", "0");
    command
}

#[derive(Clone, Debug)]
struct StreamDigest {
    bytes: u64,
    sha256: [u8; 32],
}

#[derive(Clone, Debug)]
struct GitStreamEvidence {
    status_code: i32,
    stdout: StreamDigest,
    // Drained and hashed in full for capture diagnostics, never workspace identity.
    #[cfg_attr(not(test), allow(dead_code))]
    stderr: StreamDigest,
}

impl GitStreamEvidence {
    fn success(&self) -> bool {
        self.status_code == 0
    }
}

fn hash_reader(mut reader: impl Read) -> std::io::Result<StreamDigest> {
    let mut hasher = StreamingSha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| std::io::Error::other("Git output length overflow"))?;
        hasher.update(&buffer[..read]);
    }
    Ok(StreamDigest {
        bytes: total,
        sha256: hasher.finalize(),
    })
}

/// Drain a piped child with constant memory. Stdout is observed in chunks and
/// hashed in full. Stderr is concurrently drained and hashed in full, including
/// outputs larger than the former cap, so neither pipe can stall the child.
fn stream_child_output(
    mut child: crate::sandbox::process_owner::Child,
    mut observe_stdout: impl FnMut(&[u8]) -> bool,
    deadline: Instant,
    isolated_group: bool,
) -> Option<GitStreamEvidence> {
    use std::os::fd::AsRawFd as _;

    let pid = child.id();
    let mut stdout = child.stdout.take()?;
    let stderr = child.stderr.take()?;
    let stderr_thread = match std::thread::Builder::new()
        .name("workspace-git-stderr".to_string())
        .spawn(move || hash_reader(stderr))
    {
        Ok(thread) => thread,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };

    let mut stdout_hash = StreamingSha256::new();
    let mut stdout_bytes = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let stdout_ok = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break false;
        }
        let mut descriptor = libc::pollfd {
            fd: stdout.as_raw_fd(),
            events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
            revents: 0,
        };
        let wait_ms = remaining.min(Duration::from_millis(50)).as_millis() as i32;
        // SAFETY: descriptor points to one live stdout fd for this call; poll
        // does not retain the pointer after returning.
        let ready = unsafe { libc::poll(&mut descriptor, 1, wait_ms) };
        if ready == 0 {
            continue;
        }
        if ready < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            break false;
        }
        match stdout.read(&mut buffer) {
            Ok(0) => break true,
            Ok(read) => {
                let Some(total) = stdout_bytes.checked_add(read as u64) else {
                    break false;
                };
                stdout_bytes = total;
                stdout_hash.update(&buffer[..read]);
                if !observe_stdout(&buffer[..read]) {
                    break false;
                }
            }
            Err(_) => break false,
        }
    };
    let status = if stdout_ok {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) | Err(_) => break None,
            }
        }
    } else {
        None
    };
    let completed = stdout_ok && status.is_some();
    if !completed {
        if isolated_group {
            // SAFETY: Git is spawned as the leader of this private group.
            unsafe {
                libc::killpg(pid as libc::pid_t, libc::SIGKILL);
            }
        }
        let _ = child.kill();
    }
    drop(stdout);
    let status = status.or_else(|| child.wait().ok());
    if isolated_group {
        // A successful Git parent must not leave a descendant retaining
        // stderr and wedging the reader join.
        // SAFETY: this is the private group established at spawn.
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGKILL);
        }
    }
    let stderr = stderr_thread.join().ok().and_then(Result::ok);
    if !completed {
        return None;
    }
    Some(GitStreamEvidence {
        status_code: status?.code().unwrap_or(-1),
        stdout: StreamDigest {
            bytes: stdout_bytes,
            sha256: stdout_hash.finalize(),
        },
        stderr: stderr?,
    })
}

/// Stream one pinned Git command with constant memory. The caller decides
/// which exit statuses are expected; every status is retained in the evidence.
fn git_stream_output(
    root: &Path,
    args: &[&str],
    observe_stdout: impl FnMut(&[u8]) -> bool,
) -> Option<GitStreamEvidence> {
    use std::os::unix::process::CommandExt as _;

    let mut command = pinned_git_command(root, args);
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command.spawn_owned().ok()?;
    stream_child_output(
        child,
        observe_stdout,
        Instant::now() + GIT_STREAM_COMMAND_TIMEOUT,
        true,
    )
}

fn nul_paths(bytes: &[u8]) -> impl Iterator<Item = PathBuf> + '_ {
    bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(path_from_git_bytes)
}

fn hash_workspace_path(root: &Path, relative: &Path, hasher: &mut DefaultHasher) {
    relative.hash(hasher);
    let path = root.join(relative);
    let Ok(metadata) = std::fs::symlink_metadata(&path) else {
        "missing".hash(hasher);
        return;
    };
    metadata.len().hash(hasher);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.mode().hash(hasher);
    }
    if metadata.file_type().is_symlink() {
        if let Ok(target) = std::fs::read_link(&path) {
            target.hash(hasher);
        }
        return;
    }
    if !metadata.is_file() {
        return;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        "unreadable".hash(hasher);
        return;
    };
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    if metadata.len() > MAX_EXACT_FINGERPRINT_BYTES {
        // The live fingerprint is a change detector, not the evaluator's audit
        // digest. Sampling both ends plus size/mtime prevents a stray model,
        // dataset, or build artifact from adding multi-gigabyte reads to every
        // turn while still noticing ordinary rewrites of a large file.
        if let Ok(read) = file.read(&mut buffer) {
            buffer[..read].hash(hasher);
        }
        if file
            .seek(SeekFrom::End(-(HASH_BUFFER_BYTES as i64)))
            .is_ok()
            && let Ok(read) = file.read(&mut buffer)
        {
            buffer[..read].hash(hasher);
        }
        if let Ok(modified) = metadata.modified() {
            modified.hash(hasher);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            metadata.ctime().hash(hasher);
            metadata.ctime_nsec().hash(hasher);
        }
        return;
    }
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => buffer[..read].hash(hasher),
            Err(_) => {
                "read-error".hash(hasher);
                break;
            }
        }
    }
}

/// Hash the actual changed tracked and untracked workspace contents.
///
/// The fingerprint is intentionally Git-backed: it avoids walking dependencies
/// and build products while still detecting opaque shell edits, staged changes,
/// deletions, renames, and untracked source files. `None` means the workspace is
/// not a usable Git checkout; direct mutation-tool tracking remains the fallback.
pub(crate) fn workspace_fingerprint(root: &Path) -> Option<u64> {
    // A desktop/terminal launcher often opens at $HOME. A home directory can be
    // a dotfiles Git checkout whose untracked set includes caches, model weights,
    // and every nested project. Fingerprinting that broad root is optional
    // policy evidence, so fail open instead of delaying the actual model call.
    if workspace_is_home(root, std::env::var_os("HOME").as_deref()) {
        return None;
    }
    // All four metadata probes share one wall-clock budget. A repository that
    // wedges late in the sequence cannot multiply the interactive stall by the
    // number of Git commands; failure simply disables this optional signal.
    let deadline = Instant::now() + Duration::from_secs(5);
    let [
        status_args,
        tracked_args,
        tracked_paths_args,
        untracked_args,
    ] = fingerprint_probe_args();
    // The four probes are independent read-only Git views of one tree, so run
    // them concurrently: turn start/end latency pays the slowest probe instead
    // of their sum. Any miss still fails open exactly like the sequential form.
    let (status, tracked, tracked_paths, untracked) = std::thread::scope(|scope| {
        let status = scope.spawn(|| git_output(root, &status_args, deadline));
        let tracked = scope.spawn(|| git_output(root, &tracked_args, deadline));
        let tracked_paths = scope.spawn(|| git_output(root, &tracked_paths_args, deadline));
        let untracked = scope.spawn(|| git_output(root, &untracked_args, deadline));
        (
            status.join().ok().flatten(),
            tracked.join().ok().flatten(),
            tracked_paths.join().ok().flatten(),
            untracked.join().ok().flatten(),
        )
    });

    Some(hash_fingerprint_probes(
        root,
        &status?,
        &tracked?,
        &tracked_paths?,
        &untracked?,
    ))
}

fn fingerprint_probe_args() -> [Vec<&'static str>; 4] {
    let mut status_args = vec![
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
    ];
    status_args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    // Enumerate every tracked path rather than trusting status/diff to reveal
    // it: assume-unchanged, skip-worktree, clean filters, and diff drivers can
    // deliberately hide real worktree bytes from Git's presentation layer.
    let mut tracked_args = vec!["ls-files", "--stage", "-z", "--"];
    tracked_args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    let mut tracked_paths_args = vec!["ls-files", "-z", "--"];
    tracked_paths_args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    let mut untracked_args = vec!["ls-files", "--others", "--exclude-standard", "-z", "--"];
    untracked_args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    [
        status_args,
        tracked_args,
        tracked_paths_args,
        untracked_args,
    ]
}

fn hash_fingerprint_probes(
    root: &Path,
    status: &[u8],
    tracked: &[u8],
    tracked_paths: &[u8],
    untracked: &[u8],
) -> u64 {
    let mut paths = nul_paths(tracked_paths)
        .chain(nul_paths(untracked))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();

    let mut hasher = DefaultHasher::new();
    status.hash(&mut hasher);
    tracked.hash(&mut hasher);
    for path in paths {
        hash_workspace_path(root, &path, &mut hasher);
    }
    hasher.finish()
}

fn append_evidence_blob(canonical: &mut StreamingSha256, label: &[u8], bytes: &[u8]) {
    canonical.update(&(label.len() as u64).to_be_bytes());
    canonical.update(label);
    canonical.update(&(bytes.len() as u64).to_be_bytes());
    canonical.update(bytes);
}

fn append_git_stream(canonical: &mut StreamingSha256, label: &[u8], evidence: &GitStreamEvidence) {
    // Stderr describes execution (including sandbox scan counts over ignored
    // scratch), not Git state. Keep draining it, but bind only status and exact
    // stdout here. Failed probes still fail closed at their callers.
    let mut framed = [0_u8; 44];
    framed[..4].copy_from_slice(&evidence.status_code.to_be_bytes());
    framed[4..12].copy_from_slice(&evidence.stdout.bytes.to_be_bytes());
    framed[12..44].copy_from_slice(&evidence.stdout.sha256);
    append_evidence_blob(canonical, label, &framed);
}

fn append_exact_file(
    canonical: &mut StreamingSha256,
    label: &[u8],
    path: &Path,
    expected_len: u64,
) -> Option<()> {
    canonical.update(&(label.len() as u64).to_be_bytes());
    canonical.update(label);
    canonical.update(&expected_len.to_be_bytes());
    let mut file = std::fs::File::open(path).ok()?;
    let mut observed = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        observed = observed.checked_add(read as u64)?;
        if observed > expected_len {
            return None;
        }
        canonical.update(&buffer[..read]);
    }
    (observed == expected_len).then_some(())
}

fn path_from_git_bytes(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()))
    }
    #[cfg(not(unix))]
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

fn append_symlink_target(canonical: &mut StreamingSha256, target: &Path) -> Option<()> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        append_evidence_blob(canonical, b"symlink-target", target.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let target_bytes = target
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        append_evidence_blob(canonical, b"symlink-target", &target_bytes);
    }
    #[cfg(not(any(unix, windows)))]
    {
        // Unknown platforms have no stable raw OsStr representation. Fail
        // closed instead of allowing lossy decoding to collapse two targets.
        append_evidence_blob(canonical, b"symlink-target", target.to_str()?.as_bytes());
    }
    Some(())
}

fn append_untracked_path(
    canonical: &mut StreamingSha256,
    root: &Path,
    path_bytes: &[u8],
) -> Option<()> {
    append_evidence_blob(canonical, b"untracked-path", path_bytes);
    let relative = path_from_git_bytes(path_bytes);
    let path = root.join(&relative);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(path).ok()?;
        append_symlink_target(canonical, &target)?;
    } else if metadata.is_file() {
        // Untracked bytes have no Git diff to represent them. Stream exact
        // content with the same constant-memory SHA sink; otherwise a
        // same-length middle rewrite could preserve the acceptance identity.
        if metadata.len() > MAX_EXACT_FINGERPRINT_BYTES {
            let exact = hash_reader(std::fs::File::open(&path).ok()?).ok()?;
            if exact.bytes != metadata.len() {
                return None;
            }
            let mut framed = [0_u8; 40];
            framed[..8].copy_from_slice(&exact.bytes.to_be_bytes());
            framed[8..].copy_from_slice(&exact.sha256);
            append_evidence_blob(canonical, b"file-stream", &framed);
        } else {
            append_exact_file(canonical, b"file-bytes", &path, metadata.len())?;
        }
    } else {
        append_evidence_blob(canonical, b"other-kind", b"");
    }
    Some(())
}

fn observe_nul_paths(
    canonical: &mut StreamingSha256,
    root: &Path,
    pending: &mut Vec<u8>,
    chunk: &[u8],
) -> bool {
    for byte in chunk {
        if *byte == 0 {
            if !pending.is_empty() && append_untracked_path(canonical, root, pending).is_none() {
                return false;
            }
            pending.clear();
        } else {
            if pending.len() >= MAX_STREAMED_PATH_BYTES {
                return false;
            }
            pending.push(*byte);
        }
    }
    true
}

fn observe_tracked_index_records(pending: &mut Vec<u8>, chunk: &[u8]) -> bool {
    for byte in chunk {
        if *byte == 0 {
            if pending.len() < 3 || pending[1] != b' ' {
                return false;
            }
            let tag = pending[0];
            // `git ls-files -v` lowercases every tag carrying the
            // assume-unchanged bit. `S` is skip-worktree; both flags become
            // lowercase `s`. Either hides tracked bytes from status/diff, so
            // exact evidence and verifier reuse must fail closed.
            if tag.is_ascii_lowercase() || tag.eq_ignore_ascii_case(&b'S') {
                return false;
            }
            pending.clear();
        } else {
            if pending.len() >= MAX_STREAMED_PATH_BYTES {
                return false;
            }
            pending.push(*byte);
        }
    }
    true
}

fn tracked_index_evidence(root: &Path) -> Option<GitStreamEvidence> {
    let mut pending = Vec::new();
    let mut args = vec!["ls-files", "-v", "-z", "--"];
    args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    let evidence = git_stream_output(root, &args, |chunk| {
        observe_tracked_index_records(&mut pending, chunk)
    })?;
    (evidence.success() && pending.is_empty()).then_some(evidence)
}

fn safe_git_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn append_tracked_stage_record(
    canonical: &mut StreamingSha256,
    root: &Path,
    record: &[u8],
) -> Option<()> {
    let tab = record.iter().position(|byte| *byte == b'\t')?;
    let mut header = record[..tab].split(|byte| *byte == b' ');
    let mode = header.next()?;
    let object = header.next()?;
    let stage = header.next()?;
    if header.next().is_some()
        || !matches!(mode, b"100644" | b"100755" | b"120000")
        || object.is_empty()
        || object.iter().any(|byte| !byte.is_ascii_hexdigit())
        || stage != b"0"
    {
        // Gitlinks (160000), conflicted stages, and unknown index modes cannot
        // honestly attest the bytes of one ordinary workspace tree.
        return None;
    }
    let path_bytes = &record[tab + 1..];
    let relative = path_from_git_bytes(path_bytes);
    if !safe_git_relative_path(&relative) {
        return None;
    }
    append_evidence_blob(canonical, b"tracked-path", path_bytes);
    append_evidence_blob(canonical, b"tracked-index-mode", mode);

    let path = root.join(&relative);
    let parent = path.parent()?;
    let canonical_root = std::fs::canonicalize(root).ok()?;
    let canonical_parent = std::fs::canonicalize(parent).ok()?;
    if !canonical_parent.starts_with(&canonical_root) {
        return None;
    }
    let before = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            append_evidence_blob(canonical, b"tracked-kind", b"missing");
            return Some(());
        }
        Err(_) => return None,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Permission/type bits are semantic workspace state. Inode and times
        // are used below only as a race guard: hashing them would make a
        // touch or byte-for-byte revert invalidate otherwise identical proof.
        append_evidence_blob(
            canonical,
            b"tracked-mode",
            &(before.mode() as u64).to_be_bytes(),
        );
    }
    #[cfg(not(unix))]
    append_evidence_blob(
        canonical,
        b"tracked-readonly",
        &[u8::from(before.permissions().readonly())],
    );

    if before.file_type().is_symlink() {
        append_evidence_blob(canonical, b"tracked-kind", b"symlink");
        append_symlink_target(canonical, &std::fs::read_link(&path).ok()?)?;
    } else if before.is_file() {
        append_evidence_blob(canonical, b"tracked-kind", b"file");
        append_exact_file(canonical, b"tracked-bytes", &path, before.len())?;
    } else {
        return None;
    }

    let after = std::fs::symlink_metadata(path).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.mode() != after.mode()
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return None;
        }
    }
    #[cfg(not(unix))]
    if before.len() != after.len() || before.file_type() != after.file_type() {
        return None;
    }
    Some(())
}

fn observe_tracked_stage_records(
    canonical: &mut StreamingSha256,
    root: &Path,
    pending: &mut Vec<u8>,
    chunk: &[u8],
) -> bool {
    for byte in chunk {
        if *byte == 0 {
            if pending.is_empty() || append_tracked_stage_record(canonical, root, pending).is_none()
            {
                return false;
            }
            pending.clear();
        } else {
            if pending.len() >= MAX_STREAMED_PATH_BYTES {
                return false;
            }
            pending.push(*byte);
        }
    }
    true
}

/// Diagnostic-only path identities. Use enumeration and raw file reads, never
/// status/diff, so collecting names does not invoke clean/smudge filters. These
/// observations explain a failed evidence comparison; they do not authorize it.
pub(crate) fn workspace_evidence_paths(
    root: &Path,
) -> Option<std::collections::BTreeMap<PathBuf, String>> {
    if workspace_is_home(root, std::env::var_os("HOME").as_deref()) {
        return None;
    }
    let mut paths = std::collections::BTreeMap::new();
    for tracked in [true, false] {
        let mut args = if tracked {
            vec!["ls-files", "--stage", "-z", "--"]
        } else {
            vec!["ls-files", "--others", "--exclude-standard", "-z", "--"]
        };
        args.extend(ACTIVE_WORKSPACE_PATHSPECS);
        let mut pending = Vec::new();
        let output = git_stream_output(root, &args, |chunk| {
            for byte in chunk {
                if *byte != 0 {
                    if pending.len() >= MAX_STREAMED_PATH_BYTES {
                        return false;
                    }
                    pending.push(*byte);
                    continue;
                }
                let mut digest = StreamingSha256::new();
                let path_bytes = if tracked {
                    let Some(tab) = pending.iter().position(|byte| *byte == b'\t') else {
                        return false;
                    };
                    if append_tracked_stage_record(&mut digest, root, &pending).is_none() {
                        return false;
                    }
                    // Include staged object identity as well as worktree bytes.
                    append_evidence_blob(&mut digest, b"index-record", &pending);
                    &pending[tab + 1..]
                } else {
                    if !safe_git_relative_path(&path_from_git_bytes(&pending))
                        || append_untracked_path(&mut digest, root, &pending).is_none()
                    {
                        return false;
                    }
                    pending.as_slice()
                };
                paths.insert(
                    path_from_git_bytes(path_bytes),
                    hex_digest(digest.finalize()),
                );
                pending.clear();
            }
            true
        })?;
        if !output.success() || !pending.is_empty() {
            return None;
        }
    }
    Some(paths)
}

/// Deterministic cryptographic identity for the active Git workspace state.
///
/// Unlike [`workspace_fingerprint`], this is an audit artifact rather than a
/// cheap live-change detector. It hashes porcelain status, the binary tracked
/// `HEAD` identity, binary tracked diff (or the staged diff before a first
/// commit), and the exact bytes/targets of sorted untracked paths. Ignored
/// dependencies and build products remain excluded by Git policy. The reserved
/// root `.angel-experiment-tmp` is excluded even if force-added. Subprocess
/// diagnostics (including sandbox receipts on stderr) are not workspace state.
pub(crate) fn workspace_evidence_sha256(root: &Path) -> Option<String> {
    if workspace_is_home(root, std::env::var_os("HOME").as_deref()) {
        return None;
    }
    let mut canonical = StreamingSha256::new();

    let tracked_index_before = tracked_index_evidence(root)?;
    append_git_stream(&mut canonical, b"tracked-index", &tracked_index_before);

    let head = git_stream_output(root, &["rev-parse", "--verify", "HEAD"], |_| true)?;
    let unborn = if head.success() {
        append_git_stream(&mut canonical, b"head", &head);
        false
    } else {
        // A missing commit is valid only when HEAD is still a well-formed
        // symbolic ref. Any other rev-parse failure is corrupt/unavailable
        // evidence and fails closed instead of masquerading as an unborn repo.
        let symbolic = git_stream_output(root, &["symbolic-ref", "-q", "HEAD"], |_| true)?;
        if !symbolic.success() {
            return None;
        }
        append_git_stream(&mut canonical, b"head-unborn-attempt", &head);
        append_git_stream(&mut canonical, b"head-symbolic-ref", &symbolic);
        append_evidence_blob(&mut canonical, b"head", b"unborn\n");
        true
    };

    let mut status_args = vec![
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
    ];
    status_args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    let status = git_stream_output(root, &status_args, |_| true)?;
    if !status.success() {
        return None;
    }
    append_git_stream(&mut canonical, b"status", &status);

    let mut diff_args = if unborn {
        vec![
            "diff",
            "--binary",
            "--no-ext-diff",
            "--no-textconv",
            "--cached",
            "--",
        ]
    } else {
        vec![
            "diff",
            "--binary",
            "--no-ext-diff",
            "--no-textconv",
            "HEAD",
            "--",
        ]
    };
    diff_args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    let tracked_diff = git_stream_output(root, &diff_args, |_| true)?;
    if !tracked_diff.success() {
        return None;
    }
    // This fixed-size frame covers every byte of arbitrarily large Git output;
    // unlike the former head/tail sampling, a same-length middle-only edit must
    // change the acceptance identity without materializing the diff.
    append_git_stream(&mut canonical, b"tracked-diff", &tracked_diff);

    // Git presentation diffs can be suppressed by clean filters, attributes,
    // or external drivers. Bind the actual bytes, kind, mode, and raw symlink
    // target of every ordinary tracked path instead of trusting that semantic
    // diff as the sole worktree identity. Gitlinks/conflicted stages fail closed.
    let mut pending_tracked = Vec::new();
    append_evidence_blob(&mut canonical, b"tracked-worktree-stream", b"v1");
    let mut tracked_args = vec!["ls-files", "--stage", "-z", "--"];
    tracked_args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    let tracked_worktree = git_stream_output(root, &tracked_args, |chunk| {
        observe_tracked_stage_records(&mut canonical, root, &mut pending_tracked, chunk)
    })?;
    if !tracked_worktree.success() || !pending_tracked.is_empty() {
        return None;
    }
    append_git_stream(&mut canonical, b"tracked-index-stage", &tracked_worktree);

    let mut pending_path = Vec::new();
    append_evidence_blob(&mut canonical, b"untracked-path-stream", b"v1");
    let mut untracked_args = vec!["ls-files", "--others", "--exclude-standard", "-z", "--"];
    untracked_args.extend(ACTIVE_WORKSPACE_PATHSPECS);
    let untracked = git_stream_output(root, &untracked_args, |chunk| {
        observe_nul_paths(&mut canonical, root, &mut pending_path, chunk)
    })?;
    if !untracked.success() || !pending_path.is_empty() {
        return None;
    }
    append_git_stream(&mut canonical, b"untracked-list", &untracked);

    let tracked_index_after = tracked_index_evidence(root)?;
    if tracked_index_before.status_code != tracked_index_after.status_code
        || tracked_index_before.stdout.bytes != tracked_index_after.stdout.bytes
        || tracked_index_before.stdout.sha256 != tracked_index_after.stdout.sha256
    {
        return None;
    }
    Some(hex_digest(canonical.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_git_probe_does_not_renew_an_expired_shared_budget() {
        let expired = Instant::now() - Duration::from_millis(1);
        assert!(git_output(Path::new("."), &["--version"], expired).is_none());
    }

    fn git(root: &Path, args: &[&str]) {
        let status = pinned_git_command(root, args).status().unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn commit(root: &Path, message: &str) {
        git(
            root,
            &[
                "-c",
                "user.name=Angel Test",
                "-c",
                "user.email=angel@example.invalid",
                "commit",
                "-qm",
                message,
            ],
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn evidence_ignores_scratch_and_sandbox_diagnostics_but_binds_candidate_bytes() {
        let _lock = crate::tests::env_lock();
        let fixture = crate::sandbox::HardlinkTestRoot::new();
        let root = fixture.path();
        git(root, &["init", "-q"]);
        std::fs::write(root.join("candidate.txt"), "before").unwrap();
        git(root, &["add", "candidate.txt"]);
        commit(root, "evidence baseline");
        let _confined = ConfinedRecoveryGit::new(root).unwrap();
        let before = workspace_evidence_sha256(root).unwrap();
        let paths_before = workspace_evidence_paths(root).unwrap();
        let probe_before = git_stream_output(root, &["rev-parse", "HEAD"], |_| true).unwrap();
        let scratch = root.join(".angel-experiment-tmp");
        std::fs::create_dir(&scratch).unwrap();
        std::fs::write(scratch.join("counter"), "x").unwrap();
        // Exclusion is a product contract, independent of .git/info/exclude.
        let probe_after = git_stream_output(root, &["rev-parse", "HEAD"], |_| true).unwrap();
        assert_eq!(probe_before.stdout.sha256, probe_after.stdout.sha256);
        assert_ne!(probe_before.stderr.sha256, probe_after.stderr.sha256);
        assert_eq!(workspace_evidence_sha256(root).unwrap(), before);
        assert_eq!(workspace_evidence_paths(root).unwrap(), paths_before);
        git(root, &["add", "-f", ".angel-experiment-tmp/counter"]);
        assert_eq!(workspace_evidence_sha256(root).unwrap(), before);
        std::fs::write(scratch.join("counter"), "xx").unwrap();
        assert_eq!(workspace_evidence_sha256(root).unwrap(), before);
        assert_eq!(workspace_evidence_paths(root).unwrap(), paths_before);

        std::fs::write(root.join("candidate.txt"), "after!").unwrap();
        assert_ne!(workspace_evidence_sha256(root).unwrap(), before);
        let paths_after = workspace_evidence_paths(root).unwrap();
        assert_ne!(
            paths_before[Path::new("candidate.txt")],
            paths_after[Path::new("candidate.txt")]
        );
        std::fs::write(root.join("new.txt"), "new").unwrap();
        assert!(
            workspace_evidence_paths(root)
                .unwrap()
                .contains_key(Path::new("new.txt"))
        );
        std::fs::remove_file(root.join("candidate.txt")).unwrap();
        assert_ne!(
            workspace_evidence_paths(root).unwrap()[Path::new("candidate.txt")],
            paths_after[Path::new("candidate.txt")]
        );
    }

    #[test]
    fn sha256_padding_boundaries_preserve_exact_digests() {
        // Fixed zero-byte vectors cross both the padding and compression edges.
        for (len, expected) in [
            (
                55,
                "02779466cdec163811d078815c633f21901413081449002f24aa3e80f0b88ef7",
            ),
            (
                56,
                "d4817aa5497628e7c77e6b606107042bbba3130888c5f47a375e6179be789fbb",
            ),
            (
                63,
                "c7723fa1e0127975e49e62e753db53924c1bd84b8ac1ac08df78d09270f3d971",
            ),
            (
                64,
                "f5a5fd42d16a20302798ef6ed309979b43003d2320d9f0e8ea9831a92759fb4b",
            ),
            (
                65,
                "98ce42deef51d40269d542f5314bef2c7468d401ad5d85168bfab4c0108f75f7",
            ),
            (
                119,
                "f616b0d54e78571a9611f343c9f8e022e859e920381ab0e4d3da01e193a7bd7e",
            ),
            (
                120,
                "6edd9f6f9cc92cded36e6c4a580933f9c9f1b90562b46903b806f21902a1a54f",
            ),
            (
                127,
                "15dae5979058bfbf4f9166029b6e340ea3ca374fef578a11dc9e6e923860d7ae",
            ),
            (
                128,
                "38723a2e5e8a17aa7950dc008209944e898f69a7bd10a23c839d341e935fd5ca",
            ),
        ] {
            let bytes = vec![0; len];
            assert_eq!(crate::cut::sha256_hex(&bytes), expected, "len={len}");
            assert_eq!(sha256_reader_hex(&mut bytes.as_slice()).unwrap(), expected);
            for split in 0..=len {
                let mut streaming = StreamingSha256::new();
                streaming.update(&bytes[..split]);
                let mut cloned = streaming.clone();
                streaming.update(&[]);
                streaming.update(&bytes[split..]);
                cloned.update(&bytes[split..]);
                assert_eq!(
                    hex_digest(streaming.finalize()),
                    expected,
                    "len={len}, split={split}"
                );
                assert_eq!(hex_digest(cloned.finalize()), expected);
            }
        }
    }

    #[test]
    fn streaming_sha256_matches_one_shot_across_large_chunk_boundaries() {
        let bytes = (0..(MAX_EXACT_FINGERPRINT_BYTES as usize + HASH_BUFFER_BYTES + 17))
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let expected = crate::cut::sha256_hex(&bytes);
        for chunk_size in [1, 7, 63, 64, 65, HASH_BUFFER_BYTES, HASH_BUFFER_BYTES + 13] {
            let mut streaming = StreamingSha256::new();
            for chunk in bytes.chunks(chunk_size) {
                streaming.update(chunk);
            }
            assert_eq!(
                hex_digest(streaming.finalize()),
                expected,
                "chunk={chunk_size}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn streamed_child_drains_large_stdout_and_stderr_exactly() {
        let chunk = "0123456789abcdef".repeat(4);
        let repetitions = 20_000_u64;
        let script = format!(
            "i=0; while [ \"$i\" -lt {repetitions} ]; do printf '%s' '{chunk}'; printf '%s' '{chunk}' >&2; i=$((i + 1)); done"
        );
        let child = Command::new("/bin/sh")
            .args(["-c", &script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn_owned()
            .unwrap();
        let mut observed_stdout = 0_u64;
        let evidence = stream_child_output(
            child,
            |bytes| {
                observed_stdout += bytes.len() as u64;
                true
            },
            Instant::now() + Duration::from_secs(3),
            false,
        )
        .expect("large dual-stream child must complete without a pipe stall");
        let expected = repetitions * chunk.len() as u64;
        assert!(expected > MAX_EXACT_FINGERPRINT_BYTES);
        assert_eq!(evidence.status_code, 0);
        assert_eq!(evidence.stdout.bytes, expected);
        assert_eq!(evidence.stderr.bytes, expected);
        assert_eq!(observed_stdout, expected);
    }

    #[cfg(unix)]
    #[test]
    fn streamed_child_deadline_kills_a_pipe_holding_process_group() {
        use std::os::unix::process::CommandExt as _;

        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 30 & wait"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let child = command.spawn_owned().unwrap();
        let started = Instant::now();
        assert!(
            stream_child_output(
                child,
                |_| true,
                Instant::now() + Duration::from_millis(50),
                true,
            )
            .is_none()
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "a wedged Git tree must not retain evaluator evidence forever"
        );
    }

    #[cfg(unix)]
    #[test]
    fn streamed_child_deadline_also_bounds_wait_after_early_pipe_close() {
        use std::os::unix::process::CommandExt as _;

        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "exec >/dev/null 2>&1; sleep 30"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let child = command.spawn_owned().unwrap();
        let started = Instant::now();
        assert!(
            stream_child_output(
                child,
                |_| true,
                Instant::now() + Duration::from_millis(50),
                true,
            )
            .is_none()
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "closing pipes early must not bypass the child deadline"
        );
    }

    #[test]
    fn evidence_hashes_middle_only_change_in_oversized_tracked_diff() {
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-evidence-large-middle-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);

        // One very long line makes the binary diff exceed the historical 1 MiB
        // cap while keeping its first/last 64 KiB unchanged across the two
        // mutations. The old head/tail sampler therefore collided here.
        let mut contents = vec![b'a'; MAX_EXACT_FINGERPRINT_BYTES as usize + 256 * 1024];
        let middle = contents.len() / 2;
        std::fs::write(root.join("large.txt"), &contents).unwrap();
        git(&root, &["add", "large.txt"]);
        commit(&root, "large baseline");

        contents[middle] = b'b';
        std::fs::write(root.join("large.txt"), &contents).unwrap();
        let first_diff =
            git_stream_output(&root, &["diff", "--binary", "HEAD", "--"], |_| true).unwrap();
        assert!(first_diff.success());
        assert!(
            first_diff.stdout.bytes > MAX_EXACT_FINGERPRINT_BYTES,
            "fixture must exercise oversized Git output: {} bytes",
            first_diff.stdout.bytes
        );
        let first = workspace_evidence_sha256(&root).unwrap();

        contents[middle] = b'c';
        std::fs::write(root.join("large.txt"), &contents).unwrap();
        assert_eq!(
            std::fs::metadata(root.join("large.txt")).unwrap().len(),
            contents.len() as u64
        );
        let second = workspace_evidence_sha256(&root).unwrap();
        assert_ne!(
            first, second,
            "same-length middle-only tracked changes need distinct audit identities"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evidence_streams_middle_only_change_in_large_untracked_file() {
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-evidence-large-untracked-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join("anchor.txt"), "anchor").unwrap();
        git(&root, &["add", "anchor.txt"]);
        commit(&root, "anchor");

        let mut contents = vec![b'x'; MAX_EXACT_FINGERPRINT_BYTES as usize + HASH_BUFFER_BYTES];
        let middle = contents.len() / 2;
        std::fs::write(root.join("untracked.bin"), &contents).unwrap();
        let first = workspace_evidence_sha256(&root).unwrap();
        contents[middle] = b'y';
        std::fs::write(root.join("untracked.bin"), &contents).unwrap();
        let second = workspace_evidence_sha256(&root).unwrap();
        assert_ne!(
            first, second,
            "large untracked middle bytes must participate in evidence identity"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evidence_fails_closed_for_hidden_tracked_index_flags() {
        let _env_lock = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-evidence-hidden-index-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join("tracked.txt"), "baseline").unwrap();
        git(&root, &["add", "tracked.txt"]);
        commit(&root, "hidden-index baseline");
        assert!(workspace_evidence_sha256(&root).is_some());

        git(
            &root,
            &["update-index", "--assume-unchanged", "tracked.txt"],
        );
        std::fs::write(root.join("tracked.txt"), "changed!").unwrap();
        assert_eq!(
            workspace_evidence_sha256(&root),
            None,
            "assume-unchanged bytes must disable exact evidence/cache reuse"
        );

        git(
            &root,
            &["update-index", "--no-assume-unchanged", "tracked.txt"],
        );
        std::fs::write(root.join("tracked.txt"), "baseline").unwrap();
        git(&root, &["update-index", "--skip-worktree", "tracked.txt"]);
        std::fs::write(root.join("tracked.txt"), "changed!").unwrap();
        assert_eq!(
            workspace_evidence_sha256(&root),
            None,
            "skip-worktree bytes must disable exact evidence/cache reuse"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn evidence_hashes_actual_tracked_bytes_hidden_by_clean_filters() {
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-evidence-clean-filter-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join(".gitattributes"), "probe filter=hide\n").unwrap();
        std::fs::write(root.join("probe"), "baseline\n").unwrap();
        git(&root, &["config", "filter.hide.clean", "sed s/.*/fixed/"]);
        git(&root, &["add", ".gitattributes", "probe"]);
        commit(&root, "filtered baseline");

        std::fs::write(root.join("probe"), "first dirty bytes\n").unwrap();
        let first = workspace_evidence_sha256(&root).unwrap();
        std::fs::write(root.join("probe"), "other dirty bytes\n").unwrap();
        let second = workspace_evidence_sha256(&root).unwrap();
        assert_ne!(
            first, second,
            "working-tree clean filters must not collapse distinct actual bytes"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evidence_ignores_git_repository_redirection_environment() {
        let _env_lock = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-evidence-env-root-{}",
            std::process::id()
        ));
        let decoy = std::env::temp_dir().join(format!(
            "angel-workspace-evidence-env-decoy-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&decoy);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&decoy).unwrap();
        for (repo, body) in [(&root, "root"), (&decoy, "decoy")] {
            git(repo, &["init", "-q"]);
            std::fs::write(repo.join("tracked.txt"), body).unwrap();
            git(repo, &["add", "tracked.txt"]);
            commit(repo, body);
        }
        std::fs::write(root.join("fixture-only.txt"), "root only").unwrap();
        git(&root, &["add", "fixture-only.txt"]);
        commit(&root, "fixture-only");
        let expected = workspace_evidence_sha256(&root).unwrap();

        let _git_dir =
            crate::tests::TestEnvGuard::set("GIT_DIR", decoy.join(".git").to_str().unwrap());
        let _git_work_tree =
            crate::tests::TestEnvGuard::set("GIT_WORK_TREE", decoy.to_str().unwrap());
        let _git_index = crate::tests::TestEnvGuard::set(
            "GIT_INDEX_FILE",
            decoy.join(".git/index").to_str().unwrap(),
        );
        git(&root, &["add", "fixture-only.txt"]);
        assert_eq!(workspace_evidence_sha256(&root), Some(expected));

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(decoy);
    }

    #[cfg(unix)]
    #[test]
    fn evidence_distinguishes_non_utf8_symlink_targets() {
        use std::os::unix::ffi::OsStringExt;
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "angel-workspace-evidence-non-utf8-symlink-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join("anchor.txt"), "anchor").unwrap();
        git(&root, &["add", "anchor.txt"]);
        commit(&root, "anchor");

        let link = root.join("untracked-link");
        let first_target = vec![b't', b'a', b'r', b'g', b'e', b't', b'-', 0x80];
        let second_target = vec![b't', b'a', b'r', b'g', b'e', b't', b'-', 0x81];
        assert_eq!(
            String::from_utf8_lossy(&first_target),
            String::from_utf8_lossy(&second_target),
            "fixture must collide under the former lossy encoding"
        );

        symlink(
            PathBuf::from(std::ffi::OsString::from_vec(first_target)),
            &link,
        )
        .unwrap();
        let first = workspace_evidence_sha256(&root).unwrap();
        std::fs::remove_file(&link).unwrap();
        symlink(
            PathBuf::from(std::ffi::OsString::from_vec(second_target)),
            &link,
        )
        .unwrap();
        let second = workspace_evidence_sha256(&root).unwrap();
        assert_ne!(
            first, second,
            "distinct raw symlink-target bytes need distinct audit identities"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evidence_streams_status_larger_than_memory_chunk() {
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-evidence-large-status-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join("anchor.txt"), "anchor").unwrap();
        git(&root, &["add", "anchor.txt"]);
        commit(&root, "anchor");

        let suffix = "s".repeat(205);
        for index in 0..5_000 {
            std::fs::write(root.join(format!("{index:05}-{suffix}.txt")), b"x").unwrap();
        }
        let status = git_stream_output(
            &root,
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            |_| true,
        )
        .unwrap();
        assert!(status.success());
        assert!(
            status.stdout.bytes > MAX_EXACT_FINGERPRINT_BYTES,
            "fixture produced only {} status bytes",
            status.stdout.bytes
        );
        assert!(workspace_evidence_sha256(&root).is_some());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evaluator_execution_pinned_git_ignores_hostile_path() {
        let root =
            std::env::temp_dir().join(format!("angel-pinned-git-control-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        let hostile = root.join("hostile");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&hostile).unwrap();
        git(&workspace, &["init", "-q"]);
        std::fs::write(workspace.join("tracked.txt"), "fixture").unwrap();
        git(&workspace, &["add", "tracked.txt"]);
        let expected = workspace_evidence_sha256(&workspace).unwrap();

        let fake_git = hostile.join("git");
        std::fs::write(&fake_git, "#!/bin/sh\nprintf forged\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&fake_git).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&fake_git, permissions).unwrap();
        }
        let hostile_path = format!("{}:/bin:/usr/bin", hostile.display());
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "harness::workspace_state::tests::evaluator_execution_pinned_git_child",
                "--nocapture",
            ])
            .env("PATH", &hostile_path)
            .env("ANGEL_PINNED_GIT_TEST_WORKSPACE", &workspace)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("PINNED_GIT_DIGEST={expected}")),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn broad_home_workspace_skips_optional_git_evidence() {
        let root =
            std::env::temp_dir().join(format!("angel-home-workspace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("project")).unwrap();

        assert!(workspace_is_home(&root, Some(root.as_os_str())));
        assert!(!workspace_is_home(
            &root.join("project"),
            Some(root.as_os_str())
        ));
        assert!(!workspace_is_home(&root, None));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evaluator_execution_pinned_git_child() {
        let Some(workspace) = std::env::var_os("ANGEL_PINNED_GIT_TEST_WORKSPACE") else {
            return;
        };
        let digest = workspace_evidence_sha256(Path::new(&workspace)).unwrap();
        println!("PINNED_GIT_DIGEST={digest}");
    }

    #[test]
    fn fingerprint_tracks_real_changes_and_ignores_reverts() {
        let _env_lock = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-fingerprint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join("tracked.txt"), "base").unwrap();
        git(&root, &["add", "tracked.txt"]);
        git(
            &root,
            &[
                "-c",
                "user.name=Angel Test",
                "-c",
                "user.email=angel@example.invalid",
                "commit",
                "-qm",
                "base",
            ],
        );

        let baseline = workspace_fingerprint(&root).unwrap();
        let evidence_baseline = workspace_evidence_sha256(&root).unwrap();
        assert_eq!(workspace_fingerprint(&root), Some(baseline));
        assert_eq!(
            workspace_evidence_sha256(&root),
            Some(evidence_baseline.clone())
        );
        std::fs::write(root.join("tracked.txt"), "changed").unwrap();
        assert_ne!(workspace_fingerprint(&root), Some(baseline));
        assert_ne!(
            workspace_evidence_sha256(&root),
            Some(evidence_baseline.clone())
        );
        std::fs::write(root.join("tracked.txt"), "base").unwrap();
        assert_eq!(workspace_fingerprint(&root), Some(baseline));
        assert_eq!(
            workspace_evidence_sha256(&root),
            Some(evidence_baseline.clone())
        );

        std::fs::write(root.join("new.txt"), "one").unwrap();
        let untracked = workspace_fingerprint(&root).unwrap();
        assert_ne!(untracked, baseline);
        std::fs::write(root.join("new.txt"), "two").unwrap();
        assert_ne!(workspace_fingerprint(&root), Some(untracked));

        std::fs::remove_file(root.join("new.txt")).unwrap();
        std::fs::write(root.join("tracked.txt"), "next revision").unwrap();
        git(&root, &["add", "tracked.txt"]);
        git(
            &root,
            &[
                "-c",
                "user.name=Angel Test",
                "-c",
                "user.email=angel@example.invalid",
                "commit",
                "-qm",
                "next",
            ],
        );
        assert_ne!(
            workspace_evidence_sha256(&root),
            Some(evidence_baseline),
            "clean revisions must have distinct evaluator evidence identities"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_fingerprint_equals_sequential_recomputation() {
        let _env_lock = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-fingerprint-equiv-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join("tracked.txt"), "committed").unwrap();
        git(&root, &["add", "tracked.txt"]);
        commit(&root, "base");
        std::fs::write(root.join("staged.txt"), "staged").unwrap();
        git(&root, &["add", "staged.txt"]);
        std::fs::write(root.join("tracked.txt"), "modified after commit").unwrap();
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("untracked.txt"), "untracked").unwrap();
        std::fs::write(root.join("nested").join("deep.txt"), "deep untracked").unwrap();

        // Expected value: the same four probes, run strictly one after another
        // through git_output, hashed by the same helper the live path uses.
        let deadline = Instant::now() + Duration::from_secs(30);
        let [
            status_args,
            tracked_args,
            tracked_paths_args,
            untracked_args,
        ] = fingerprint_probe_args();
        let status = git_output(&root, &status_args, deadline).unwrap();
        let tracked = git_output(&root, &tracked_args, deadline).unwrap();
        let tracked_paths = git_output(&root, &tracked_paths_args, deadline).unwrap();
        let untracked = git_output(&root, &untracked_args, deadline).unwrap();
        let expected =
            hash_fingerprint_probes(&root, &status, &tracked, &tracked_paths, &untracked);

        assert_eq!(workspace_fingerprint(&root), Some(expected));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fingerprint_is_absent_outside_git() {
        let root = std::env::temp_dir().join(format!(
            "angel-workspace-fingerprint-nongit-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(workspace_fingerprint(&root), None);
        assert_eq!(workspace_evidence_sha256(&root), None);
        let _ = std::fs::remove_dir_all(root);
    }
}
