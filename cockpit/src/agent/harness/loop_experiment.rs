//! One controller-owned, same-route experiment in a copy of active live source.
//! The caller owns admission, token reservation, cancellation and result draining.

use super::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Component;

const SNAPSHOT_LIMIT: usize = 512 * 1024 * 1024;
const FILE_LIMIT: usize = 16 * 1024 * 1024;
const ACTIVE_PATHS: &[&str] = &[".", ":(exclude)off-limits/**", ":(exclude)**/off-limits/**"];

pub(crate) struct LoopExperimentRequest {
    pub workspace: PathBuf,
    pub artifact_dir: PathBuf,
    pub task: String,
    /// Zero means uncapped: the leaf runs until it answers, is cancelled, or
    /// exhausts an operator-set token budget. Productive work is never capped
    /// by default; a controller may still pin a bound for a specific run.
    pub max_hops: usize,
    /// Zero means no deadline. A positive value arms the controller's deadline
    /// watcher and bounds the verifier process with the time left.
    pub deadline_secs: u64,
    /// Conservatively reserved by the controller; zero means no token cap.
    pub token_budget: u64,
    /// Pinned by the controller, never taken from the child's answer.
    pub verify_command: Option<String>,
    /// Optional policy note applied to this attempt's system prompt. The
    /// reinforcement reflector proposes it; the controller supplies it, and it
    /// is never taken from the child's own text.
    pub policy_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoopExperimentVerification {
    pub command_sha256: String,
    pub candidate_sha256: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub stdout: String,
    pub stderr: String,
    pub output_truncated: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoopExperimentResult {
    pub answer: String,
    pub task_sha256: String,
    pub model_phase_entered: bool,
    pub rollout_id: Option<String>,
    pub patch_sha256: Option<String>,
    /// Hash of exact persisted result bytes; not recursively serialized.
    #[serde(skip)]
    pub result_sha256: Option<String>,
    pub stop_reason: String,
    pub error: Option<String>,
    pub snapshot_sha256: String,
    pub artifact_dir: PathBuf,
    pub patch_path: Option<PathBuf>,
    pub requested_route: crate::agent::club::RouteIdentity,
    pub resolved_route: crate::agent::club::RouteIdentity,
    /// Conservative text estimate, not provider billing or shared-club deltas.
    pub estimated_tokens: u64,
    pub verification: Option<LoopExperimentVerification>,
    /// Physical verifier evidence produced by the evaluator-owned execution of
    /// `verify_command`. Never serialized: a caller scores this evidence, it
    /// cannot reconstruct evidence from a receipt or from candidate text.
    #[serde(skip)]
    pub evidence: Option<crate::drive::reinforce::EvaluatorEvidence>,
    /// Content-addressed path of the persisted evidence artifact, when the
    /// append-only store accepted it.
    pub evidence_path: Option<PathBuf>,
    /// Typed capture is optional and never grants objective progress to parent.
    pub training_capture: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct SnapshotFile {
    sha256: String,
    executable: bool,
    /// The target a reproduced symlink names. Its text is hashed and recreated
    /// in the copy; the snapshot never reads through it.
    #[serde(skip_serializing_if = "Option::is_none")]
    symlink: Option<PathBuf>,
}

/// One inventoried path as the snapshot copies it.
enum LiveEntry {
    File {
        bytes: Vec<u8>,
        executable: bool,
    },
    /// A final-component symlink, reproduced as a link (see [`symlink_entry`]).
    Link(PathBuf),
}

/// Resolve `.` and `..` lexically; `None` when a `..` climbs above the start.
fn normalize_lexically(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

/// Whether a symlink at `path` (relative to the snapshot `root`) naming
/// `target` can be reproduced without aliasing the live tree. A relative link
/// must resolve to an active path inside the tree, so it points at the copy's
/// own file (`libfoo.so -> libfoo.so.1`). An absolute link must lie outside the
/// tree and must not be the tree or one of its ancestors, both as written and
/// as resolved (`libcrypto.so -> /usr/lib/.../libcrypto.so.3`). Everything else
/// could lead the copy back into the live source or its quarantine.
fn link_is_admissible(root: &Path, path: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let outside =
            |candidate: &Path| !candidate.starts_with(&root) && !root.starts_with(candidate);
        return normalize_lexically(target).is_some_and(|lexical| outside(&lexical))
            && target
                .canonicalize()
                .map_or(true, |resolved| outside(&resolved));
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    normalize_lexically(&parent.join(target)).is_some_and(|resolved| active_relative(&resolved))
}

fn active_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|part| match part {
            Component::Normal(name) => name != "off-limits" && name != ".git",
            _ => false,
        })
}

/// Resolve and create one owned, outside-the-source destination: the nearest
/// existing ancestor is canonicalized first (so a symlinked parent cannot land
/// inside the source or the quarantine), excluded paths are refused before
/// anything is created, and the leaf must be new. Shared by the unique artifact
/// directory and the frozen source, so both get the same protection.
fn resolve_owned_destination(
    workspace: &Path,
    destination: &Path,
    what: &str,
) -> Result<PathBuf, String> {
    if !destination.is_absolute() {
        return Err(format!("{what} path must be absolute"));
    }
    if destination
        .components()
        .any(|part| part.as_os_str() == "off-limits")
    {
        return Err(format!("{what} path is excluded"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| format!("{what} has no parent"))?;
    let ancestor = parent
        .ancestors()
        .find(|path| path.exists())
        .ok_or_else(|| format!("{what} path has no existing ancestor"))?;
    let resolved_ancestor = ancestor.canonicalize().map_err(|e| e.to_string())?;
    if resolved_ancestor
        .components()
        .any(|part| part.as_os_str() == "off-limits")
    {
        return Err(format!("{what} ancestor is excluded"));
    }
    if resolved_ancestor.starts_with(workspace) {
        return Err(format!("{what} ancestor is inside the source workspace"));
    }
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let resolved_parent = parent.canonicalize().map_err(|e| e.to_string())?;
    if resolved_parent.starts_with(workspace) {
        return Err(format!("{what} must live outside the source workspace"));
    }
    if resolved_parent
        .components()
        .any(|part| part.as_os_str() == "off-limits")
    {
        return Err(format!("{what} parent is excluded"));
    }
    std::fs::create_dir(destination).map_err(|e| format!("create unique {what}: {e}"))?;
    destination
        .canonicalize()
        .map_err(|e| format!("resolve unique {what}: {e}"))
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("experiment cancelled".into())
    } else {
        Ok(())
    }
}

/// Time left for one bounded child of the attempt. A zero deadline means the
/// operator declined a time cap, so the child is bounded by cancellation alone
/// rather than by a fabricated giant timeout.
fn remaining_bound(deadline_secs: u64, elapsed: Duration) -> Option<Duration> {
    (deadline_secs > 0).then(|| Duration::from_secs(deadline_secs).saturating_sub(elapsed))
}

fn git_output(root: &Path, args: &[&str], cancel: &AtomicBool) -> Result<Vec<u8>, String> {
    git_output_with_limit(root, args, cancel, 64 * 1024 * 1024)
}

fn git_output_with_limit(
    root: &Path,
    args: &[&str],
    cancel: &AtomicBool,
    output_limit: u64,
) -> Result<Vec<u8>, String> {
    check_cancel(cancel)?;
    let confine_forwarder = std::env::var_os("ANGEL_CODING_TRAINING_AUTHORITY_DIR").is_some();
    let mut command = if confine_forwarder {
        super::workspace_state::pinned_git_for_confined_forwarder(root, &[])
    } else {
        super::workspace_state::pinned_git_command(root, &[])
    };
    command
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.templateDir=",
            "-c",
            "user.name=Angel experiment",
            "-c",
            "user.email=experiment@localhost",
        ])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env_remove("GIT_TEMPLATE_DIR");
    // The ordinary tool transcript cap is 1 MiB, smaller than a real active
    // source path inventory. Redirect only stdout to an owned bounded spool;
    // retain the existing cancellation/process-group boundary for execution.
    static GIT_SPOOL_SEQ: AtomicU64 = AtomicU64::new(0);
    let spool = std::env::temp_dir().join(format!(
        "angel-experiment-git-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        GIT_SPOOL_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&spool).map_err(|e| e.to_string())?;
    struct Spool(PathBuf);
    impl Drop for Spool {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _spool_owner = Spool(spool.clone());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&spool, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let stdout_path = spool.join("stdout");
    let mut forwarding_policy = None;
    let mut forwarding = if confine_forwarder {
        // Candidate Git filters/config execute in this process. They receive
        // no write authority outside their working tree and this owned spool.
        let mut writable_roots = vec![spool.clone()];
        if args.first() != Some(&"ls-files") {
            writable_roots.push(root.to_path_buf());
        }
        let policy = crate::agent::sandbox::SandboxPolicy {
            writable_roots,
            allow_network: false,
            enforce: true,
            mandatory: true,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        };
        forwarding_policy = Some(policy.clone());
        crate::agent::sandbox::command("/bin/sh", std::iter::empty::<&str>(), &policy)
            .map_err(|e| e.to_string())?
    } else {
        Command::new("/bin/sh")
    };
    // Ignore SIGXFSZ (inherited across exec) so an over-bound write fails with
    // EFBIG and a nonzero exit instead of killing Git and dumping core.
    forwarding
        .args([
            "-c",
            "experiment_output=$1; shift; ulimit -f \"$1\" || exit; shift; trap '' XFSZ; exec \"$@\" > \"$experiment_output\"",
            "angel-experiment-git",
        ])
        .arg(&stdout_path)
        .arg(output_limit.div_ceil(512).max(1).to_string())
        .arg(command.get_program())
        .args(command.get_args())
        .current_dir(root);
    for (name, value) in command.get_envs() {
        if let Some(value) = value {
            forwarding.env(name, value);
        } else {
            forwarding.env_remove(name);
        }
    }
    if let Some(policy) = forwarding_policy {
        crate::agent::sandbox::set_helper_policy(&mut forwarding, &policy)
            .map_err(|e| e.to_string())?;
    }
    let capture =
        output_timed_captured_cancellable(forwarding, Some(Duration::from_secs(30)), Some(cancel))?;
    if capture.cancelled
        || capture.timed_out
        || !capture.output.status.success()
        || capture.stdout_truncated
        || capture.stderr_truncated
    {
        return Err(format!(
            "experiment Git {} failed (exit {:?}, cancelled {}, timeout {}): {}",
            args.first().unwrap_or(&""),
            capture.output.status.code(),
            capture.cancelled,
            capture.timed_out,
            String::from_utf8_lossy(&capture.output.stderr)
        ));
    }
    check_cancel(cancel)?;
    let file = std::fs::File::open(stdout_path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > output_limit {
        return Err(
            "experiment Git output exceeds its byte bound; working artifacts preserved".into(),
        );
    }
    let mut bytes = Vec::new();
    file.take(output_limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > output_limit {
        return Err("experiment Git output grew beyond its bound".into());
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_directory_beneath(root: &Path, relative: &Path) -> Result<std::fs::File, String> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    if !relative.as_os_str().is_empty() && !active_relative(relative) {
        return Err("excluded nested snapshot directory".into());
    }
    let mut directory = std::fs::File::open(root).map_err(|e| e.to_string())?;
    for part in relative.components() {
        let name =
            std::ffi::CString::new(part.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        // SAFETY: retained directory descriptor and a single checked component.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(format!(
                "nested snapshot directory refused: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: openat returned one owned descriptor.
        directory = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Ok(directory)
}

fn active_files(root: &Path, cancel: &AtomicBool) -> Result<Vec<PathBuf>, String> {
    fn inventory(
        root: &Path,
        prefix: &Path,
        depth: usize,
        paths: &mut Vec<PathBuf>,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        if depth > 8 {
            return Err("nested experiment repositories exceed depth 8".into());
        }
        check_cancel(cancel)?;
        let mut args = vec![
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--exclude=off-limits/",
            "--exclude=**/off-limits/",
            "-z",
            "--",
        ];
        args.extend_from_slice(ACTIVE_PATHS);
        let output = git_output(root, &args, cancel)?;
        // Gitlinks are directory entries without the trailing slash emitted for
        // untracked repositories. Read their index mode, not filesystem type:
        // an uninitialized gitlink must fail rather than disappear as a deletion.
        let mut staged_args = vec!["ls-files", "--stage", "-z", "--"];
        staged_args.extend_from_slice(ACTIVE_PATHS);
        let staged = git_output(root, &staged_args, cancel)?;
        let gitlinks = staged
            .split(|byte| *byte == 0)
            .filter(|record| record.starts_with(b"160000 "))
            .filter_map(|record| {
                record
                    .iter()
                    .position(|byte| *byte == b'\t')
                    .map(|at| &record[at + 1..])
            })
            .collect::<std::collections::BTreeSet<_>>();
        let mut nested_seen = std::collections::BTreeSet::new();
        for bytes in output
            .split(|byte| *byte == 0)
            .filter(|bytes| !bytes.is_empty())
        {
            #[cfg(unix)]
            let path = {
                use std::os::unix::ffi::OsStrExt;
                PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
            };
            #[cfg(not(unix))]
            let path =
                PathBuf::from(std::str::from_utf8(bytes).map_err(|_| "non UTF-8 snapshot path")?);
            if !active_relative(&path) {
                return Err("experiment enumeration contained an excluded or escaping path".into());
            }
            // Preserve live bytes in both embedded repositories and initialized
            // tracked submodules; never check out the recorded gitlink commit.
            if bytes.ends_with(b"/") || gitlinks.contains(bytes) {
                if !nested_seen.insert(path.clone()) {
                    continue;
                }
                #[cfg(unix)]
                {
                    let directory = open_directory_beneath(root, &path)?;
                    #[cfg(target_os = "linux")]
                    let nested = {
                        use std::os::fd::AsRawFd;
                        PathBuf::from(format!(
                            "/proc/{}/fd/{}",
                            std::process::id(),
                            directory.as_raw_fd()
                        ))
                    };
                    #[cfg(not(target_os = "linux"))]
                    let nested = root.join(&path);
                    // An empty/uninitialized submodule directory would make Git
                    // discover its parent repository. Reject that before inventory.
                    let git_prefix = git_output(&nested, &["rev-parse", "--show-prefix"], cancel)?;
                    if git_prefix != b"\n" {
                        return Err(format!(
                            "nested snapshot repository is uninitialized or not rooted at {}",
                            prefix.join(&path).display()
                        ));
                    }
                    inventory(&nested, &prefix.join(&path), depth + 1, paths, cancel)?;
                    drop(directory);
                }
                #[cfg(not(unix))]
                return Err("nested no-follow snapshots require Unix".into());
            } else {
                paths.push(prefix.join(path));
                if paths.len() > 100_000 {
                    return Err("experiment inventory exceeds 100000 paths".into());
                }
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    inventory(root, Path::new(""), 0, &mut paths, cancel)?;
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Walk every component through directory descriptors: no directory symlink
/// can lead this snapshot into excluded or external content, and a final
/// symlink is recorded as a link, never followed ([`symlink_entry`]).
#[cfg(unix)]
fn read_live_file(root: &Path, path: &Path) -> Result<Option<LiveEntry>, String> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    if !active_relative(path) {
        return Err("excluded snapshot path".into());
    }
    let mut directory = std::fs::File::open(root).map_err(|e| e.to_string())?;
    let parts = path.components().collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        let name =
            std::ffi::CString::new(part.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let last = index + 1 == parts.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if last { 0 } else { libc::O_DIRECTORY };
        // SAFETY: a valid retained directory descriptor and NUL-terminated name.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            // `O_NOFOLLOW` on a final symlink fails with ELOOP.
            if last && error.raw_os_error() == Some(libc::ELOOP) {
                return symlink_entry(&directory, &name, root, path).map(Some);
            }
            return Err(format!("snapshot refuses {}: {error}", path.display()));
        }
        // SAFETY: openat returned a new owned descriptor.
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if !last {
            directory = file;
            continue;
        }
        let before = file.metadata().map_err(|e| e.to_string())?;
        if !before.is_file() || before.len() > FILE_LIMIT as u64 {
            return Err(format!(
                "snapshot requires a regular file <= {FILE_LIMIT} bytes: {}",
                path.display()
            ));
        }
        let mut bytes = Vec::new();
        let mut reader = file;
        reader
            .by_ref()
            .take(FILE_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let after = reader.metadata().map_err(|e| e.to_string())?;
        if bytes.len() > FILE_LIMIT
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.mode() != after.mode()
        {
            return Err("source changed while snapshot was being copied".into());
        }
        return Ok(Some(LiveEntry::File {
            bytes,
            executable: before.mode() & 0o111 != 0,
        }));
    }
    Err("empty snapshot path".into())
}

/// Read a final-component symlink's target through its directory descriptor
/// and admit it when [`link_is_admissible`]. A build tree's `lib/libcrypto.so ->
/// /usr/lib/.../libcrypto.so.3` made every deep experiment of a night-long loop
/// fail at setup (apollo, 2026-09-24) while the snapshot refused all symlinks.
#[cfg(unix)]
fn symlink_entry(
    directory: &std::fs::File,
    name: &std::ffi::CStr,
    root: &Path,
    path: &Path,
) -> Result<LiveEntry, String> {
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStringExt;
    let mut buffer = vec![0u8; libc::PATH_MAX as usize];
    // SAFETY: a valid retained directory descriptor, a NUL-terminated name and
    // an owned buffer of the stated length.
    let read = unsafe {
        libc::readlinkat(
            directory.as_raw_fd(),
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    if read < 0 || read as usize >= buffer.len() {
        return Err(format!(
            "snapshot refuses {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    buffer.truncate(read as usize);
    let target = PathBuf::from(std::ffi::OsString::from_vec(buffer));
    if link_is_admissible(root, path, &target) {
        Ok(LiveEntry::Link(target))
    } else {
        Err(format!(
            "snapshot refuses symlink {} -> {}",
            path.display(),
            target.display()
        ))
    }
}

#[cfg(not(unix))]
fn read_live_file(_root: &Path, _path: &Path) -> Result<Option<LiveEntry>, String> {
    Err("no-follow experiment snapshots currently require Unix".into())
}

fn snapshot_live(
    root: &Path,
    destination: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<BTreeMap<PathBuf, SnapshotFile>, String> {
    let mut manifest = BTreeMap::new();
    let mut total = 0usize;
    for path in active_files(root, cancel)? {
        check_cancel(cancel)?;
        let Some(entry) = read_live_file(root, &path)? else {
            continue;
        };
        let (bytes, executable) = match entry {
            LiveEntry::File { bytes, executable } => (bytes, executable),
            LiveEntry::Link(target) => {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStrExt;
                    let text = target.as_os_str().as_bytes();
                    total = total.saturating_add(text.len());
                    if let Some(destination) = destination {
                        let at = destination.join(&path);
                        if let Some(parent) = at.parent() {
                            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        // Never chmod a link: that would follow it to its target.
                        std::os::unix::fs::symlink(&target, &at).map_err(|e| e.to_string())?;
                    }
                    manifest.insert(
                        path,
                        SnapshotFile {
                            sha256: crate::knowledge::cut::sha256_hex(text),
                            executable: false,
                            symlink: Some(target),
                        },
                    );
                }
                continue;
            }
        };
        total = total.saturating_add(bytes.len());
        if total > SNAPSHOT_LIMIT {
            return Err("active experiment snapshot exceeds 512 MiB".into());
        }
        if let Some(destination) = destination {
            let target = destination.join(&path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, &bytes).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(
                    &target,
                    std::fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
                )
                .map_err(|e| e.to_string())?;
            }
        }
        manifest.insert(
            path,
            SnapshotFile {
                sha256: crate::knowledge::cut::sha256_hex(&bytes),
                executable,
                symlink: None,
            },
        );
    }
    Ok(manifest)
}

fn manifest_hash(manifest: &BTreeMap<PathBuf, SnapshotFile>) -> Result<String, String> {
    serde_json::to_vec(manifest)
        .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
        .map_err(|e| e.to_string())
}

/// Digest of the active source exactly as an experiment sees it: tracked plus
/// untracked non-ignored files, quarantine excluded, no symlink following.
/// Shares the one walker with [`run_loop_experiment`].
pub(crate) fn source_snapshot_digest(source: &Path, cancel: &AtomicBool) -> Result<String, String> {
    let manifest = snapshot_live(source, None, cancel)?;
    manifest_hash(&manifest)
}

/// Materialize the active source into `destination` and commit it as a frozen
/// baseline. This is the same no-follow snapshot every experiment copies, with
/// modes preserved, so a caller never needs its own copier: ignored heavy
/// artifacts stay out, dirty operator edits are included, and cancellation,
/// quarantine exclusions and source protection come along.
pub(crate) fn freeze_active_source(
    source: &Path,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let source = source
        .canonicalize()
        .map_err(|error| format!("could not resolve experiment source: {error}"))?;
    if source
        .components()
        .any(|part| part.as_os_str() == "off-limits")
    {
        return Err("experiment source resolves into excluded content".into());
    }
    let destination = resolve_owned_destination(&source, destination, "frozen source")?;
    let manifest = snapshot_live(&source, Some(&destination), cancel)?;
    git_output(&destination, &["init", "-q"], cancel)?;
    git_output(&destination, &["add", "-f", "-A", "--", "."], cancel)?;
    git_output(
        &destination,
        &[
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Angel frozen source",
            "-c",
            "user.email=frozen@localhost",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "frozen active source",
        ],
        cancel,
    )?;
    manifest_hash(&manifest)
}

/// Tree hash of the committed state in `root` (content and modes only, so two
/// copies of the same bytes agree regardless of commit metadata). Used to prove
/// an attempt actually started from the case's frozen source.
pub(crate) fn committed_tree_hash(root: &Path, cancel: &AtomicBool) -> Result<String, String> {
    let bytes = git_output(root, &["rev-parse", "HEAD^{tree}"], cancel)?;
    let hash = String::from_utf8_lossy(&bytes).trim().to_string();
    (!hash.is_empty() && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(hash)
        .ok_or_else(|| "frozen source has no committed tree".to_string())
}

/// Binary patch of the candidate's *current* changes against the frozen
/// baseline, optionally limited to `paths` (relative to `root`). `git diff HEAD`
/// covers staged and unstaged edits alike, so a change made after the attempt's
/// own `git add` is still part of what gets bound and checked. New, nonignored
/// files after that recorded snapshot are not represented in Git's diff; refuse
/// them explicitly rather than letting unbound code enter the verifier.
pub(crate) fn candidate_patch_since_baseline(
    root: &Path,
    paths: &[String],
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let mut untracked = vec!["ls-files", "--others", "--exclude-standard", "-z", "--"];
    let owned = paths.iter().map(String::as_str).collect::<Vec<_>>();
    untracked.extend_from_slice(&owned);
    if !git_output(root, &untracked, cancel)?.is_empty() {
        return Err("candidate patch does not match its working copy: unrecorded files".into());
    }
    let mut args = vec!["diff", "--binary", "HEAD", "--"];
    args.extend_from_slice(&owned);
    git_output(root, &args, cancel)
}

/// Resolve configuration before the child can mutate its workspace. A store
/// beneath either writable root cannot serve as training authority.
fn recovery_authority(working: &Path, scratch: &Path) -> Result<Option<PathBuf>, String> {
    fn resolved(path: &Path) -> Result<PathBuf, String> {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(path)
        };
        if path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err("authority path must not contain parent traversal".into());
        }
        let ancestor = path
            .ancestors()
            .find(|path| std::fs::symlink_metadata(path).is_ok())
            .ok_or("authority path has no existing ancestor")?;
        Ok(ancestor
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join(path.strip_prefix(ancestor).map_err(|e| e.to_string())?))
    }
    let Some(configured) = std::env::var_os("ANGEL_CODING_TRAINING_AUTHORITY_DIR") else {
        return Ok(None);
    };
    let authority = resolved(Path::new(&configured))?;
    for writable in [working, scratch] {
        if authority.starts_with(resolved(writable)?) {
            return Err("training authority overlaps a candidate-writable recovery tree".into());
        }
    }
    Ok(Some(authority))
}

pub(crate) fn run_loop_experiment(
    mut request: LoopExperimentRequest,
    club: Arc<dyn Club>,
    cancel: Arc<AtomicBool>,
) -> Result<LoopExperimentResult, String> {
    if request
        .workspace
        .components()
        .any(|part| part.as_os_str() == "off-limits")
    {
        return Err("experiment workspace is excluded".into());
    }
    let workspace = request
        .workspace
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if workspace
        .components()
        .any(|part| part.as_os_str() == "off-limits")
    {
        return Err("experiment workspace resolves into excluded content".into());
    }
    // Resolve the caller's artifact path before any subprocess changes cwd.
    // Sandbox grants and all descendants must refer to the same owned root.
    if !request.artifact_dir.is_absolute() {
        request.artifact_dir = std::env::current_dir()
            .map_err(|e| format!("resolve experiment launch directory: {e}"))?
            .join(&request.artifact_dir);
    }
    request.artifact_dir =
        resolve_owned_destination(&workspace, &request.artifact_dir, "experiment artifacts")?;
    let requested_route = club.route_identity();
    let mut result = LoopExperimentResult {
        answer: String::new(),
        task_sha256: crate::knowledge::cut::sha256_hex(request.task.as_bytes()),
        model_phase_entered: false,
        rollout_id: None,
        patch_sha256: None,
        result_sha256: None,
        stop_reason: "setup_error".into(),
        error: None,
        snapshot_sha256: String::new(),
        artifact_dir: request.artifact_dir.clone(),
        patch_path: None,
        requested_route: requested_route.clone(),
        resolved_route: requested_route,
        estimated_tokens: 0,
        verification: None,
        evidence: None,
        evidence_path: None,
        training_capture: None,
    };
    let finished = AtomicBool::new(false);
    let expired = AtomicBool::new(false);
    let estimated = AtomicU64::new(0);
    let token_exhausted = AtomicBool::new(false);
    let start = Instant::now();
    let working = request.artifact_dir.join("working");
    let execution = std::thread::scope(|scope| {
        let cancel_watch = Arc::clone(&cancel);
        let finished_ref = &finished;
        let expired_ref = &expired;
        // A zero deadline declines a time cap: no watcher is armed at all, so
        // the leaf is bounded by cancellation (and any token budget) alone.
        if request.deadline_secs > 0 {
            scope.spawn(move || {
                while !finished_ref.load(Ordering::Acquire) {
                    if start.elapsed() >= Duration::from_secs(request.deadline_secs) {
                        expired_ref.store(true, Ordering::Release);
                        cancel_watch.store(true, Ordering::Release);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            });
        }
        let execution = (|| -> Result<(), String> {
            check_cancel(&cancel)?;
            let baseline = request.artifact_dir.join("baseline");
            std::fs::create_dir(&baseline).map_err(|e| e.to_string())?;
            std::fs::create_dir(&working).map_err(|e| e.to_string())?;
            let manifest = snapshot_live(&workspace, Some(&baseline), &cancel)?;
            if manifest != snapshot_live(&workspace, Some(&working), &cancel)? {
                return Err("source changed across snapshot passes; parent preserved".into());
            }
            result.snapshot_sha256 = manifest_hash(&manifest)?;
            std::fs::write(
                request.artifact_dir.join("baseline-manifest.json"),
                serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            git_output(&working, &["init", "-q"], &cancel)?;
            git_output(&working, &["add", "-f", "-A", "--", "."], &cancel)?;
            git_output(
                &working,
                &[
                    "commit",
                    "-q",
                    "--allow-empty",
                    "-m",
                    "owned active snapshot baseline",
                ],
                &cancel,
            )?;
            let cargo = PinnedCargo::capture(&workspace);
            let authority =
                recovery_authority(&working, &request.artifact_dir.join("evaluator-scratch"));
            let capture_enabled = matches!(authority, Ok(Some(_)));
            if let Err(error) = &authority {
                result.training_capture =
                    Some(serde_json::json!({"status":"disabled", "reason":error}));
            }
            let _git_scope = capture_enabled
                .then(|| super::workspace_state::ConfinedRecoveryGit::new(&working))
                .transpose()?;
            // Every experiment gets the full coding grant: network, the GPU and
            // scratch space. A confined capture registry once cut deep
            // experiments off the board CLI and the device nodes.
            let registry = Grant::Code.registry(&working, None, &cargo);
            // Every recovery leaf is an experiment, even without training capture.
            // Its raw log must not become legacy imitation before evaluation.
            let _eval_label = EvalLabelScope::new();
            // Operator-declined caps stay declined: the native turn receives no
            // hop cap and no timeout, and is bounded by cancellation (plus any
            // token budget) exactly like an ordinary interactive turn.
            let hop_cap = (request.max_hops > 0).then_some(request.max_hops);
            let mut system = String::from(
                "[LOOP RECOVERY EXPERIMENT] You are one owned leaf experiment on an isolated copy of the current active source. Test one falsifiable hypothesis with available local tools and retain negative results. Do not submit to competitions, publish, deploy, merge into the parent, launch other agents, or access off-limits paths. No public submission is authorized. Report the exact change, checks, result, and next useful action.",
            );
            if let Some(note) = request
                .policy_note
                .as_deref()
                .map(str::trim)
                .filter(|note| !note.is_empty())
            {
                system.push_str(
                    "\n\n[applied policy note — controller-supplied, applies to this attempt]\n",
                );
                system.push_str(note);
            }
            let mut history = vec![ChatMsg::system(system), ChatMsg::user(request.task.clone())];
            let (events, receiver) = mpsc::channel();
            let drain = scope.spawn(move || {
                let mut trace = Vec::new();
                let mut omitted = 0usize;
                for event in receiver {
                    let row = match event {
                        TurnEvent::ToolCall { id, name, args_summary } => Some(serde_json::json!({"kind":"tool_call", "id":format!("{id:?}"), "name":name, "args":args_summary.chars().take(4096).collect::<String>()})),
                        TurnEvent::ToolResult { id, name, summary, outcome } => Some(serde_json::json!({"kind":"tool_result", "id":format!("{id:?}"), "name":name, "summary":summary.chars().take(4096).collect::<String>(), "outcome":format!("{outcome:?}")})),
                        TurnEvent::Notice(note) => Some(serde_json::json!({"kind":"notice", "text":note.chars().take(4096).collect::<String>()})),
                        _ => None,
                    };
                    if let Some(row) = row {
                        if trace.len() < 1024 { trace.push(row); } else { omitted += 1; }
                    }
                }
                serde_json::json!({"events":trace, "omitted_events":omitted, "per_field_character_limit":4096})
            });
            // Estimate each checkpoint conservatively; this can charge a repeated
            // tool checkpoint twice, but never reads overlapping provider totals.
            let checkpoint = |messages: &[ChatMsg]| {
                check_cancel(&cancel)?;
                let bytes = messages.iter().fold(0u64, |sum, message| {
                    sum.saturating_add(message.content.len() as u64)
                });
                let cost = bytes.div_ceil(4).max(1);
                let total = estimated
                    .fetch_add(cost, Ordering::AcqRel)
                    .saturating_add(cost);
                if request.token_budget > 0 && total > request.token_budget {
                    token_exhausted.store(true, Ordering::Release);
                    cancel.store(true, Ordering::Release);
                    return Err("experiment reserved token estimate exhausted".into());
                }
                Ok(())
            };
            result.model_phase_entered = true;
            let outcome = run_turn_steered_checkpointed_observed(
                club.as_ref(),
                &registry,
                &mut history,
                &cancel,
                hop_cap,
                &events,
                None,
                &checkpoint,
            );
            drop(events);
            let trace = drain
                .join()
                .map_err(|_| "experiment event drain panicked")?;
            std::fs::write(
                request.artifact_dir.join("tool-events.json"),
                serde_json::to_vec_pretty(&trace).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            result.resolved_route = club.resolved_route_identity();
            match outcome {
                Ok(outcome) => {
                    result.rollout_id = outcome.rollout_id;
                    result.answer = outcome.answer;
                    result.stop_reason = outcome.stop_reason.as_str().into();
                }
                Err(failure) => {
                    result.rollout_id = failure.rollout_id;
                    result.stop_reason = failure.stop_reason.as_str().into();
                    result.error = Some(failure.message);
                }
            }
            // Even a stopped turn keeps its working snapshot. Export is optional
            // when cancellation prevents trusted Git from running.
            if !cancel.load(Ordering::Acquire) {
                let mut add = vec!["add", "-A", "--"];
                add.extend_from_slice(ACTIVE_PATHS);
                git_output(&working, &add, &cancel)?;
                let patch = git_output(
                    &working,
                    &[
                        "diff",
                        "--cached",
                        "--binary",
                        "HEAD",
                        "--",
                        ".",
                        ":(exclude)off-limits/**",
                        ":(exclude)**/off-limits/**",
                    ],
                    &cancel,
                )?;
                let path = request.artifact_dir.join("candidate.patch");
                result.patch_sha256 = Some(crate::knowledge::cut::sha256_hex(&patch));
                std::fs::write(&path, patch).map_err(|e| e.to_string())?;
                result.patch_path = Some(path);
                if let Some(command) = request
                    .verify_command
                    .as_deref()
                    .filter(|command| !command.trim().is_empty())
                {
                    let candidate_sha256 = manifest_hash(&snapshot_live(&working, None, &cancel)?)?;
                    // One evaluator-owned execution path for every attempt: the
                    // verifier runs sandboxed and network-denied and returns the
                    // physical evidence a reward may be scored from. Typed
                    // capture stays opt-in and never changes what was measured.
                    let execution = crate::drive::reinforce::recovery_eval::run(
                        crate::drive::reinforce::recovery_eval::RecoveryEvalRequest {
                            command,
                            workspace: &working,
                            scratch: &request.artifact_dir.join("evaluator-scratch"),
                            task: &request.task,
                            answer: &result.answer,
                            timeout: remaining_bound(request.deadline_secs, start.elapsed()),
                            cancel: &cancel,
                        },
                    );
                    match execution {
                        Ok(execution) => {
                            let evidence = execution.evidence;
                            result.verification = Some(LoopExperimentVerification {
                                command_sha256: evidence.command_sha256().to_owned(),
                                candidate_sha256,
                                exit_code: evidence.exit_code(),
                                timed_out: evidence.timed_out(),
                                cancelled: execution.cancelled,
                                stdout: String::from_utf8_lossy(evidence.stdout_bytes())
                                    .into_owned(),
                                stderr: String::from_utf8_lossy(evidence.stderr_bytes())
                                    .into_owned(),
                                output_truncated: evidence.output_truncated(),
                            });
                            let evidence_path = evidence.persist_append_only(
                                &request.artifact_dir.join("evaluator-evidence"),
                            );
                            result.evidence_path = evidence_path.as_ref().ok().cloned();
                            if !capture_enabled && result.training_capture.is_none() {
                                result.training_capture = Some(serde_json::json!({
                                    "status":"not_configured",
                                    "evidence_manifest_sha256":evidence.manifest_sha256(),
                                }));
                            }
                            if capture_enabled {
                                let typed =
                                    crate::drive::reinforce::recovery_training_supported(&evidence);
                                result.training_capture = Some(
                                    if execution.cancelled || cancel.load(Ordering::Acquire) {
                                        serde_json::json!({"status":"cancelled"})
                                    } else if result.stop_reason != "answer"
                                        || result.error.is_some()
                                    {
                                        serde_json::json!({"status":"ineligible", "reason":"native turn did not complete with an answer"})
                                    } else if let Err(reason) = typed {
                                        serde_json::json!({"status":"unsupported_or_ineligible", "reason":reason})
                                    } else if let Err(reason) = &evidence_path {
                                        serde_json::json!({"status":"capture_error", "reason":reason})
                                    } else {
                                        let row_path =
                                            request.artifact_dir.join("training-trajectory.jsonl");
                                        crate::knowledge::experience::note_turn_workspace(&working);
                                        match crate::drive::reinforce::finish_coding_eval(
                                            club.label(),
                                            &history,
                                            &request.task,
                                            &result.answer,
                                            &evidence,
                                            Some(&row_path),
                                            authority.as_ref().ok().and_then(Option::as_deref),
                                        ) {
                                            Ok(report) => serde_json::json!({
                                                "status":if report.training_capture_error.is_some() { "capture_error" } else if report.training_decision.is_some() { "captured" } else { "not_configured" },
                                                "decision_sha256":report.training_decision,
                                                "reason":report.training_capture_error,
                                                "trajectory_path":if row_path.exists() { Some(row_path) } else { None },
                                            }),
                                            Err(reason) => {
                                                serde_json::json!({"status":"unsupported_or_ineligible", "reason":reason})
                                            }
                                        }
                                    },
                                );
                                if let Some(status) = result.training_capture.as_mut() {
                                    status["evaluator_artifact"] =
                                        serde_json::json!(&evidence_path.as_ref().ok());
                                    let row_path =
                                        request.artifact_dir.join("training-trajectory.jsonl");
                                    if !row_path.exists() {
                                        // This is an actual producer observation, not a measured
                                        // label. Missing decision remains missing even if an
                                        // inbox writer later changes its claimed reward.
                                        let mut row = trajectory_record(
                                            club.label(),
                                            &history,
                                            &result.answer,
                                            0,
                                            execution.cancelled,
                                            None,
                                            now_ms(),
                                        );
                                        row["data_class"] =
                                            serde_json::json!("coding_eval_observation");
                                        row["evaluator_task"] = serde_json::json!(&request.task);
                                        row["evaluator_evidence_manifest_sha256"] =
                                            serde_json::json!(evidence.manifest_sha256());
                                        row["evaluator_decision_sha256"] = serde_json::Value::Null;
                                        row["coding_capture"] = status.clone();
                                        row["repo"] =
                                            crate::knowledge::experience::repo_value_for(&working);
                                        if let Ok(path) = &evidence_path
                                            && let Ok(bytes) = std::fs::read(path)
                                        {
                                            row["evaluator_artifact_sha256"] = serde_json::json!(
                                                crate::knowledge::cut::sha256_hex(&bytes)
                                            );
                                        }
                                        write_trajectory(&row);
                                        match append_trajectory(&row_path, &row) {
                                            Ok(()) => {
                                                status["trajectory_path"] =
                                                    serde_json::json!(row_path);
                                            }
                                            Err(error) => {
                                                status["observation_error"] =
                                                    serde_json::json!(error.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            // The evidence leaves the attempt with the result:
                            // scoring never reconstructs it from text.
                            result.evidence = Some(evidence);
                        }
                        Err(reason) => {
                            result.training_capture = Some(
                                serde_json::json!({"status":"execution_error", "reason":reason}),
                            );
                        }
                    }
                }
            }
            Ok(())
        })();
        finished.store(true, Ordering::Release);
        execution
    });
    result.estimated_tokens = estimated
        .load(Ordering::Acquire)
        .saturating_add(result.answer.len().div_ceil(4) as u64);
    if let Err(error) = execution {
        result.error = Some(error);
    }
    if expired.load(Ordering::Acquire) {
        result.stop_reason = "deadline".into();
    } else if token_exhausted.load(Ordering::Acquire) {
        result.stop_reason = "token_budget".into();
    } else if cancel.load(Ordering::Acquire) {
        result.stop_reason = "interrupt".into();
    }
    // Process settlement does not discard evidence: keep the working copy,
    // including ignored probe logs and evaluator-generated files, on every path.
    let result_bytes = serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?;
    std::fs::write(request.artifact_dir.join("result.json"), &result_bytes)
        .map_err(|e| format!("persist experiment receipt (artifacts retained): {e}"))?;
    result.result_sha256 = Some(crate::knowledge::cut::sha256_hex(&result_bytes));
    Ok(result)
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/loop_experiment__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/loop_experiment__training_tests.rs"]
mod training_tests;
