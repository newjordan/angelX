//! `proc_run` / `proc_status` / `proc_stop` — cockpit-owned background jobs.
//!
//! Jobs keep running across tool hops, with bounded rotating logs and durable
//! running/terminal receipts. Successful answers release jobs to the cockpit;
//! cancellation/failure stops that turn's jobs, and cockpit exit stops all jobs.
//! The helper subreaper contains detached descendants.
//! Historical receipts remain project-bound and starttime-checked for adoption;
//! a vanished process has an unknown exit and is never a successful tool result.

use crate::club::ToolDef;
use crate::harness::{Tool, ToolRegistry, env_flag, truncate_to_char_boundary};
use crate::sandbox::process_owner::{Child, OwnedCommandExt};
use crate::sandbox::{self, SandboxPolicy};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, ChildStdout, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// One tracked background process.
struct ProcEntry {
    owner: usize,
    cancelled: bool,
    kill: Option<sandbox::process_owner::KillReceipt>,
    project_root: PathBuf,
    project_key: String,
    name: String,
    command: String,
    pid: u32,
    log: PathBuf,
    started: Instant,
    child: Child,
    /// Captured exit description once reaped ("exit 0", "signal-killed").
    exit: Option<String>,
    exit_code: Option<i32>,
    completion_reported: bool,
    pending_completion: Option<ProcCompletion>,
    notification_workspace: PathBuf,
    /// First log-pump failure, retained so status never presents a partial log
    /// as complete. The pump keeps draining after this is set so a logging
    /// failure cannot back-pressure or SIGPIPE the managed process.
    log_warning: Arc<Mutex<Option<String>>>,
    /// Running receipt backing this entry (`None` when disabled or failed).
    /// Replaced by a retained terminal receipt when the independent reaper
    /// observes completion; not removed merely because a model asked status.
    receipt: Option<PathBuf>,
    confinement: String,
}

impl ProcEntry {
    /// Reap if exited; returns the current state label.
    fn state(&mut self) -> String {
        if let Some(e) = &self.exit {
            return e.clone();
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                let label = match status.code() {
                    Some(c) => format!("exited {c}"),
                    None => {
                        use std::os::unix::process::ExitStatusExt;
                        format!("signal-killed (signal {})", status.signal().unwrap_or(0))
                    }
                };
                use std::os::unix::process::ExitStatusExt;
                if let Some(signal) = status.signal() {
                    self.kill.get_or_insert_with(|| {
                        sandbox::process_owner::KillReceipt::new(
                            Some(signal),
                            "signal_death",
                            "unknown_external",
                        )
                    });
                }
                self.exit_code = status.code();
                self.exit = Some(label.clone());
                label
            }
            Ok(None) => "running".to_string(),
            Err(e) => {
                let label = format!("wait error: {e}; exit unknown");
                self.exit = Some(label.clone());
                label
            }
        }
    }
}

/// Literal process outcome, never a verifier or competition success claim.
#[derive(Clone, Debug)]
pub(crate) struct ProcCompletion {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) state: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) log: PathBuf,
    pub(crate) receipt: Option<PathBuf>,
    pub(crate) persistence_error: Option<String>,
}

impl ProcCompletion {
    /// Headless file tools cannot open the host-side capture path. Keep that
    /// operator diagnostic in the normal UI message, and give task models the
    /// supported process-log tool directly instead of an unusable file target.
    pub(crate) fn task_message(&self) -> String {
        let warning = if self.persistence_error.is_some() {
            "; terminal receipt persistence failed"
        } else {
            ""
        };
        format!(
            "background process [{}] {} {}{warning}. Inspect proc_status id={} for captured output. \
             Process exit is not benchmark acceptance or a verified solve.",
            self.id, self.name, self.state, self.id
        )
    }

    pub(crate) fn message(&self) -> String {
        let warning = self
            .persistence_error
            .as_ref()
            .map(|error| format!("; terminal receipt could not be persisted: {error}"))
            .unwrap_or_default();
        format!(
            "background process [{}] {} {}{warning}. Inspect proc_status id={} for captured output (retained log {}). \
             Process exit is not benchmark acceptance or a verified solve.",
            self.id,
            self.name,
            self.state,
            self.id,
            self.log.display()
        )
    }
}

fn completion_notices() -> &'static Mutex<BTreeMap<PathBuf, VecDeque<ProcCompletion>>> {
    static NOTICES: OnceLock<Mutex<BTreeMap<PathBuf, VecDeque<ProcCompletion>>>> = OnceLock::new();
    NOTICES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// UI-safe: one nonblocking lock and at most `limit` in-memory notices. The
/// caller supplies its already-canonical workspace; no path or log IO here.
pub(crate) fn take_completions(workspace: &Path, limit: usize) -> Vec<ProcCompletion> {
    let Ok(mut projects) = completion_notices().try_lock() else {
        return Vec::new();
    };
    let Some(pending) = projects.get_mut(workspace) else {
        return Vec::new();
    };
    let count = limit.min(8).min(pending.len());
    let notices = pending.drain(..count).collect();
    if pending.is_empty() {
        projects.remove(workspace);
    }
    notices
}

/// Keep one bounded, independent reaper for all proc_run children. In
/// particular it keeps running after a model answers or the UI changes folder.
fn ensure_completion_reaper() -> Result<(), String> {
    static REAPER: OnceLock<Result<(), String>> = OnceLock::new();
    REAPER
        .get_or_init(|| {
            std::thread::Builder::new()
                .name("angel-proc-reaper".into())
                .spawn(|| {
                    let mut cursor = 0;
                    loop {
                        reap_completion_batch(&mut cursor);
                        std::thread::sleep(Duration::from_millis(250));
                    }
                })
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .clone()
}

const COMPLETION_NOTICES_PER_PROJECT: usize = 128;
const COMPLETION_NOTICES_TOTAL: usize = 1024;

fn enqueue_completion(workspace: &Path, completion: &ProcCompletion) -> bool {
    let Ok(mut notices) = completion_notices().try_lock() else {
        return false;
    };
    enqueue_completion_into(&mut notices, workspace, completion)
}

fn enqueue_completion_into(
    notices: &mut BTreeMap<PathBuf, VecDeque<ProcCompletion>>,
    workspace: &Path,
    completion: &ProcCompletion,
) -> bool {
    if notices
        .get(workspace)
        .is_some_and(|queue| queue.len() >= COMPLETION_NOTICES_PER_PROJECT)
        || notices.values().map(VecDeque::len).sum::<usize>() >= COMPLETION_NOTICES_TOTAL
    {
        return false;
    }
    notices
        .entry(workspace.to_path_buf())
        .or_default()
        .push_back(completion.clone());
    true
}

fn reap_completion_batch(cursor: &mut u64) {
    use std::ops::Bound::{Excluded, Unbounded};
    let mut finished = Vec::new();
    {
        let Ok(mut procs) = table().try_lock() else {
            return;
        };
        let ids: Vec<u64> = procs
            .range((Excluded(*cursor), Unbounded))
            .take(64)
            .map(|(id, _)| *id)
            .collect();
        if ids.is_empty() {
            *cursor = 0;
            return;
        }
        for id in ids {
            *cursor = id;
            let entry = procs.get_mut(&id).expect("selected live table entry");
            entry.state();
            if let Some(completion) = entry.pending_completion.take() {
                if !enqueue_completion(&entry.notification_workspace, &completion) {
                    entry.pending_completion = Some(completion);
                }
                continue;
            }
            if entry.completion_reported || entry.exit.is_none() {
                continue;
            }
            entry.completion_reported = true;
            let receipt = entry.receipt.take();
            let terminal_path = receipt
                .as_ref()
                .and_then(|p| p.parent()?.parent())
                .map(|dir| {
                    dir.join("completions")
                        .join(format!("{id}-{}.json", entry.pid))
                });
            let completion = ProcCompletion {
                id,
                name: entry.name.clone(),
                state: entry.exit.clone().unwrap(),
                exit_code: entry.exit_code,
                log: entry.log.clone(),
                receipt: terminal_path.clone(),
                persistence_error: None,
            };
            let body = serde_json::json!({
                "schema": "angel-proc-completion/v1", "id": id, "pid": entry.pid,
                "name": entry.name, "command": entry.command, "log": entry.log,
                "project_root": entry.project_root, "project_key": entry.project_key,
                "exit": completion.state, "exit_code": completion.exit_code,
                "ok": completion.exit_code == Some(0),
                "confinement": entry.confinement,
                "completed_unix": now_unix(), "owner_pid": std::process::id(),
                "acceptance": "unverified",
                "kill": entry.kill,
                "status": if entry.cancelled { "Cancelled" } else if completion.exit_code == Some(0) { "Exited" } else { "Failed" },
                "verification": if entry.cancelled || completion.exit_code == Some(0) { "Inconclusive" } else { "Failed" },
            });
            finished.push((
                entry.notification_workspace.clone(),
                completion,
                receipt,
                body,
            ));
        }
    }
    // Filesystem work never holds the process table or notice queue lock.
    for (workspace, mut completion, running_receipt, body) in finished {
        if let Some(path) = &completion.receipt {
            let result = serde_json::to_vec_pretty(&body)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    crate::workspace_store::write_private_atomic(path, &bytes)
                        .map_err(|e| e.to_string())
                });
            match result {
                Ok(()) => {
                    if let Some(path) = running_receipt {
                        remove_receipt(&path);
                    }
                }
                Err(error) => completion.persistence_error = Some(error),
            }
        }
        if !enqueue_completion(&workspace, &completion) {
            // Backpressure retains one outcome in its existing process entry.
            // Its terminal receipt is already durable, and no queued result is
            // evicted to make room for a different workspace's burst.
            if let Ok(mut procs) = table().lock()
                && let Some(entry) = procs.get_mut(&completion.id)
            {
                entry.pending_completion = Some(completion);
            }
        }
    }
}

/// Stop only children launched with this turn's cancellation identity. The
/// pointer is an opaque key, never dereferenced or retained beyond turn cleanup.
pub(crate) fn stop_owned(owner: usize) {
    stop_owned_for(owner, "owner_reap");
}

/// A provider-only steer must not become cancellation of background work.
/// Conservative on contention; never block the cockpit waiting for the table.
pub(crate) fn background_work_pending() -> bool {
    table().try_lock().map_or(true, |entries| {
        entries.values().any(|entry| entry.exit.is_none())
    })
}

/// Headless finalization must not silently discard this task's live jobs.
/// Scope both the turn identity and workspace; unrelated cockpit jobs never
/// delay a task. Contention is conservatively pending and retried by caller.
pub(crate) fn owned_work_pending(owner: usize, workspace: &Path) -> bool {
    table().try_lock().map_or(true, |mut entries| {
        entries.values_mut().any(|entry| {
            if entry.owner != owner || entry.notification_workspace != workspace {
                return false;
            }
            entry.state();
            entry.exit.is_none()
        })
    })
}

/// A deadline flag lives for one operation, while its background children
/// belong to the enclosing turn. Transfer completed entries too: their kill
/// receipts must survive until the turn consumes them.
pub(crate) fn transfer_owned(from: usize, to: usize) {
    for entry in table().lock().unwrap().values_mut() {
        if entry.owner == from {
            entry.owner = to;
        }
    }
}

pub(crate) fn stop_owned_for(owner: usize, reason: &str) {
    {
        let mut procs = table().lock().unwrap();
        for entry in procs.values_mut().filter(|entry| entry.owner == owner) {
            entry.state();
        }
    }
    let entries: Vec<_> = table()
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, entry)| entry.owner == owner && entry.exit.is_none())
        .map(|(id, entry)| (*id, entry.notification_workspace.clone()))
        .collect();
    for (id, workspace) in entries {
        let _ = ProcStopTool::new(workspace).call_for(&serde_json::json!({"id": id}), reason);
    }
    let mut cursor = 0;
    loop {
        reap_completion_batch(&mut cursor);
        if cursor == 0 {
            break;
        }
    }
}

/// Read deaths on the turn thread, independently of the UI notification queue.
/// Release the opaque owner key so a later stack address reuse cannot inherit
/// an earlier turn's outcomes.
pub(crate) fn take_owned_kills(owner: usize) -> Vec<(u64, sandbox::process_owner::KillReceipt)> {
    let mut procs = table().lock().unwrap();
    let mut kills = Vec::new();
    for (id, entry) in procs.iter_mut().filter(|(_, entry)| entry.owner == owner) {
        if let Some(kill) = &entry.kill {
            kills.push((*id, kill.clone()));
        }
        entry.owner = 0;
    }
    kills
}

/// Final background receipts are flushed before the process-wide exit reap.
pub(crate) fn stop_all_owned() {
    let owners: std::collections::BTreeSet<_> = table()
        .lock()
        .unwrap()
        .values()
        .map(|entry| entry.owner)
        .collect();
    for owner in owners {
        stop_owned(owner);
    }
}

fn table() -> &'static Mutex<BTreeMap<u64, ProcEntry>> {
    static TABLE: OnceLock<Mutex<BTreeMap<u64, ProcEntry>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_id(cancel: Option<&std::sync::atomic::AtomicBool>) -> Result<u64, String> {
    // Separate native sessions share receipt/log storage. A per-process
    // AtomicU64 gave two real concurrent Yukon jobs handle215 in the same
    // repository. Reserve durably under an inter-process lock before spawn.
    let directory = proc_dir();
    std::fs::create_dir_all(&directory).map_err(|e| format!("process ID directory: {e}"))?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(directory.join(".id-allocation.lock"))
        .map_err(|e| format!("process ID lock: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Err("process launch cancelled while allocating handle".to_string());
        }
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            break;
        }
        let error = std::io::Error::last_os_error();
        if !matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ) {
            return Err(format!("process ID lock: {error}"));
        }
        if Instant::now() >= deadline {
            return Err("process ID allocator busy; no process was launched".to_string());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let counter = directory.join(".next-id");
    let id = match std::fs::read_to_string(&counter) {
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or("invalid process ID counter; no process was launched")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => first_free_id(),
        Err(error) => return Err(format!("read process ID counter: {error}")),
    };
    let next = id.checked_add(1).ok_or("process handle space exhausted")?;
    crate::workspace_store::write_private_atomic(&counter, format!("{next}\n").as_bytes())?;
    // Closing the lock file releases flock, including every early error path.
    Ok(id)
}

/// First handle id above every receipt on disk, so a daemon adopted from a
/// previous session never shares an id with one spawned in this session.
fn first_free_id() -> u64 {
    load_receipts()
        .iter()
        .map(|r| r.id)
        .max()
        .map_or(1, |m| m.saturating_add(1))
}

/// Log directory for background processes (`ANGEL_PROC_DIR`, default
/// `~/.angel0/proc`).
fn proc_confinement_label() -> String {
    if crate::yolo::enabled() {
        "yolo-command-authority".into()
    } else if std::env::var("ANGEL_SANDBOX_PROFILE")
        .map(|v| v.eq_ignore_ascii_case("sealed"))
        .unwrap_or(false)
    {
        "blocked-write".into()
    } else {
        "temp-root".into()
    }
}

pub(crate) fn proc_dir() -> PathBuf {
    if let Ok(d) = std::env::var("ANGEL_PROC_DIR")
        && !d.trim().is_empty()
    {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".angel0").join("proc")
}

/// A filesystem-safe short name derived from the user-supplied label (or the
/// command's first word). Pure → testable.
pub(crate) fn sanitize_name(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(24)
        .collect();
    let cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() {
        "proc".to_string()
    } else {
        cleaned
    }
}

/// Last `n` lines of `text`, joined. Pure → testable.
pub(crate) fn tail_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

fn read_log_tail(path: &PathBuf, n: usize) -> String {
    read_log_output(path, n, None)
}

fn read_log_output(path: &PathBuf, n: usize, contains: Option<&str>) -> String {
    // Keep ordinary snapshots small; filtered diagnostics can inspect a larger
    // bounded window without granting workspace access to host log paths.
    let window: u64 = if contains.is_some() {
        8 * 1024 * 1024
    } else {
        64 * 1024
    };
    use std::io::{Seek, SeekFrom};
    let rotated = rotated_log_path(path);
    let mut buf = Vec::new();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let Ok(directory) = open_log_directory(parent, false) else {
        return "(log unreadable)".to_string();
    };
    for candidate in [&rotated, path] {
        let Some(name) = candidate.file_name() else {
            continue;
        };
        let Ok(mut f) = open_log_file(&directory, name, false) else {
            continue;
        };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len > window {
            let _ = f.seek(SeekFrom::End(-(window as i64)));
        }
        let _ = (&mut f).take(window).read_to_end(&mut buf);
        if !buf.is_empty() && buf.last() != Some(&b'\n') {
            buf.push(b'\n');
        }
    }
    if buf.is_empty() {
        return "(log unreadable)".to_string();
    }
    let text = String::from_utf8_lossy(&buf);
    if let Some(needle) = contains {
        let matching: Vec<&str> = text.lines().filter(|line| line.contains(needle)).collect();
        let start = matching.len().saturating_sub(n);
        return format!(
            "[literal filter {needle:?}: {} matches in up to 8 MiB per retained log file; showing last {}]\n{}",
            matching.len(),
            matching.len() - start,
            matching[start..].join("\n")
        );
    }
    tail_lines(&text, n)
}

fn proc_log_max_bytes() -> u64 {
    std::env::var("ANGEL_PROC_LOG_MAX_BYTES")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(8 * 1024 * 1024)
        .clamp(2, 1024 * 1024 * 1024)
}

fn rotated_log_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".1");
    PathBuf::from(name)
}

fn log_name(name: &OsStr) -> std::io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in process log path")
    })
}

fn open_log_at(directory: &File, name: &OsStr, flags: i32) -> std::io::Result<File> {
    let name = log_name(name)?;
    // SAFETY: both descriptors/names remain live through openat. On success,
    // this function takes unique ownership of the returned descriptor.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

/// Pin every directory component instead of following a replaced store path.
/// New store directories, like their log files, are private from creation.
fn open_log_directory(path: &Path, create: bool) -> std::io::Result<File> {
    use std::path::Component;
    let mut directory = File::open(if path.is_absolute() { "/" } else { "." })?;
    for component in path.components() {
        let name = match component {
            Component::RootDir | Component::CurDir => continue,
            Component::Normal(name) => name,
            Component::ParentDir => OsStr::new(".."),
            Component::Prefix(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "unsupported process log path prefix",
                ));
            }
        };
        let next = match open_log_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY) {
            Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
                let c_name = log_name(name)?;
                // SAFETY: mkdirat receives a live directory FD and NUL-terminated name.
                if unsafe { libc::mkdirat(directory.as_raw_fd(), c_name.as_ptr(), 0o700) } < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(error);
                    }
                }
                open_log_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY)?
            }
            other => other?,
        };
        directory = next;
    }
    if directory.metadata()?.uid() != unsafe { libc::geteuid() } {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "process log directory has another owner",
        ));
    }
    // Existing directories retain their permissions. Only mkdirat-created
    // directories receive mode 0700; a configured parent is never chmodded.
    Ok(directory)
}

fn open_log_file(directory: &File, name: &OsStr, create: bool) -> std::io::Result<File> {
    let flags = if create {
        libc::O_WRONLY | libc::O_APPEND | libc::O_CREAT | libc::O_EXCL
    } else {
        libc::O_RDONLY
    };
    let file = open_log_at(directory, name, flags)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } || metadata.nlink() != 1
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "process log is not a singly-linked owner file",
        ));
    }
    if create {
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

fn log_stat_at(directory: &File, name: &OsStr) -> std::io::Result<libc::stat> {
    let name = log_name(name)?;
    let mut info = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstatat initializes the entire stat buffer when it returns success.
    if unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            info.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { info.assume_init() })
    }
}

fn private_log_stat(info: &libc::stat) -> bool {
    info.st_mode & libc::S_IFMT == libc::S_IFREG
        && info.st_uid == unsafe { libc::geteuid() }
        && info.st_nlink == 1
        && info.st_mode & 0o777 == 0o600
}

fn check_log_name(directory: &File, name: &OsStr, file: &File) -> std::io::Result<()> {
    let named = log_stat_at(directory, name)?;
    let opened = file.metadata()?;
    // dev_t is signed i32 on Apple and u64 on Linux; MetadataExt uses u64.
    #[allow(clippy::unnecessary_cast)]
    let same_identity = (named.st_dev as u64, named.st_ino as u64) == (opened.dev(), opened.ino());
    if !private_log_stat(&named) || !same_identity {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "process log path no longer names its private file",
        ));
    }
    Ok(())
}

struct RotatingLog {
    directory: File,
    name: OsString,
    file: File,
    segment_bytes: u64,
    current_bytes: u64,
}

impl RotatingLog {
    fn new(path: PathBuf, max_bytes: u64) -> Result<Self, String> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path
            .file_name()
            .ok_or_else(|| format!("log {path:?}: missing file name"))?
            .to_os_string();
        let directory = open_log_directory(parent, true)
            .map_err(|error| format!("log directory {parent:?}: {error}"))?;
        let file = open_log_file(&directory, &name, true)
            .map_err(|error| format!("log {path:?}: {error}"))?;
        Ok(Self {
            directory,
            name,
            file,
            segment_bytes: max_bytes.div_ceil(2).max(1),
            current_bytes: 0,
        })
    }

    fn append(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        check_log_name(&self.directory, &self.name, &self.file)?;
        let bytes = if bytes.len() as u64 > self.segment_bytes {
            &bytes[bytes.len().saturating_sub(self.segment_bytes as usize)..]
        } else {
            bytes
        };
        if self.current_bytes > 0
            && self.current_bytes.saturating_add(bytes.len() as u64) > self.segment_bytes
        {
            let mut rotated = self.name.clone();
            rotated.push(".1");
            match log_stat_at(&self.directory, &rotated) {
                Ok(info) if !private_log_stat(&info) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "rotated process log is not a private owner file",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let from = log_name(&self.name)?;
            let to = log_name(&rotated)?;
            // SAFETY: both names and the pinned directory FD remain valid.
            // renameat replaces the directory entry without following it.
            if unsafe {
                libc::renameat(
                    self.directory.as_raw_fd(),
                    from.as_ptr(),
                    self.directory.as_raw_fd(),
                    to.as_ptr(),
                )
            } < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            check_log_name(&self.directory, &rotated, &self.file)?;
            self.file = open_log_file(&self.directory, &self.name, true)?;
            self.current_bytes = 0;
        }
        self.file.write_all(bytes)?;
        self.current_bytes = self.current_bytes.saturating_add(bytes.len() as u64);
        Ok(())
    }
}

fn spawn_log_pump(
    reader: impl Read + Send + 'static,
    log: Arc<Mutex<RotatingLog>>,
    log_warning: Arc<Mutex<Option<String>>>,
    stream: &'static str,
) {
    let _ = std::thread::Builder::new()
        .name("angel-proc-log".to_string())
        .spawn(move || pump_log(reader, log, log_warning, stream));
}

fn record_log_warning(log_warning: &Mutex<Option<String>>, warning: String) {
    if let Ok(mut current) = log_warning.lock() {
        current.get_or_insert(warning);
    }
}

fn pump_log(
    mut reader: impl Read,
    log: Arc<Mutex<RotatingLog>>,
    log_warning: Arc<Mutex<Option<String>>>,
    stream: &str,
) {
    let mut chunk = [0u8; 16 * 1024];
    let mut logging = true;
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) if logging => match log.lock() {
                Ok(mut log) => {
                    if let Err(error) = log.append(&chunk[..read]) {
                        record_log_warning(
                            &log_warning,
                            format!("{stream} log write failed: {error}"),
                        );
                        logging = false;
                    }
                }
                Err(_) => {
                    record_log_warning(
                        &log_warning,
                        format!("{stream} log writer lock was poisoned"),
                    );
                    logging = false;
                }
            },
            Ok(_) => {
                // Logging is degraded, but the pipe must still be drained.
                // Stopping here can block a verbose child forever or deliver
                // SIGPIPE, turning a recoverable disk error into job failure.
            }
            Err(error) => {
                record_log_warning(&log_warning, format!("{stream} pipe read failed: {error}"));
                break;
            }
        }
    }
}

fn fmt_uptime(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

// ---------------------------------------------------------------------------
// Receipts — spawn records persisted under proc_dir()/receipts/ so a cockpit
// restart can explain and adopt the daemons it orphaned instead of losing them.
// ---------------------------------------------------------------------------

/// Receipt persistence gate (`ANGEL_PROC_RECEIPTS`, default on).
fn receipts_enabled() -> bool {
    env_flag("ANGEL_PROC_RECEIPTS", true)
}

pub(crate) fn receipts_dir() -> PathBuf {
    proc_dir().join("receipts")
}

/// One on-disk spawn receipt (`receipts_dir()/<id>-<pid>.json`). Written at
/// spawn, removed once the exit is observed (reap, `proc_stop`, or a status
/// call that finds the process gone). `starttime_ticks` is the `/proc` boot
/// starttime of the spawned pid — the guard that keeps a recycled pid from
/// masquerading as a still-running daemon.
struct Receipt {
    id: u64,
    pid: u32,
    pgid: libc::pid_t,
    name: String,
    command: String,
    log: PathBuf,
    project_root: PathBuf,
    project_key: String,
    started_unix: u64,
    starttime_ticks: u64,
    exit: Option<String>,
    /// Where this receipt lives on disk (for removal).
    path: PathBuf,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `(state, starttime)` from `/proc/<pid>/stat`: field 3 (single-char state)
/// and field 22 (starttime in clock ticks since boot). Parses after the last
/// `)` so a comm containing spaces or parens cannot shift the fields.
fn proc_stat_fields(pid: u32) -> Option<(char, u64)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = stat.rsplit_once(')')?.1;
    let mut fields = rest.split_whitespace();
    let state = fields.next()?.chars().next()?;
    // After state (field 3), starttime (field 22) is 19 fields further on.
    let starttime = fields.nth(18)?.parse().ok()?;
    Some((state, starttime))
}

/// True only when the receipt's pid exists, is not a zombie/dead slot, and its
/// `/proc` starttime matches the one recorded at spawn (pid-reuse guard).
fn receipt_alive(receipt: &Receipt) -> bool {
    if receipt.exit.is_some() || receipt.starttime_ticks == 0 {
        // Starttime was unreadable at spawn — never claim a live match.
        return false;
    }
    match proc_stat_fields(receipt.pid) {
        Some((state, starttime)) => {
            starttime == receipt.starttime_ticks && !matches!(state, 'Z' | 'X' | 'x')
        }
        None => false,
    }
}

fn write_receipt(
    id: u64,
    pid: u32,
    name: &str,
    command: &str,
    log: &Path,
    identity: &crate::workspace_store::RepoIdentity,
) -> Result<PathBuf, String> {
    let starttime_ticks = proc_stat_fields(pid).map(|(_, t)| t).unwrap_or(0);
    let path = receipts_dir().join(format!("{id}-{pid}.json"));
    let body = serde_json::json!({
        "id": id,
        "pid": pid,
        // process_group(0) at spawn → the child leads its own group, pgid == pid.
        "pgid": pid,
        "name": name,
        "command": command,
        "log": log.display().to_string(),
        "project_root": identity.root.display().to_string(),
        "project_key": identity.key,
        "started_unix": now_unix(),
        "starttime_ticks": starttime_ticks,
        "owner_pid": std::process::id(),
    });
    let bytes = serde_json::to_vec_pretty(&body).map_err(|e| e.to_string())?;
    crate::workspace_store::write_private_atomic(&path, &bytes)
        .map_err(|error| format!("receipt {}: {error}", path.display()))?;
    Ok(path)
}

fn receipt_from_value(v: &Value, path: PathBuf) -> Option<Receipt> {
    let pid = v["pid"].as_u64()? as u32;
    Some(Receipt {
        id: v["id"].as_u64()?,
        pid,
        pgid: v["pgid"].as_i64().unwrap_or(pid as i64) as libc::pid_t,
        name: v["name"].as_str()?.to_string(),
        command: v["command"].as_str().unwrap_or("").to_string(),
        log: PathBuf::from(v["log"].as_str()?),
        project_root: PathBuf::from(v["project_root"].as_str()?),
        project_key: v["project_key"].as_str()?.to_string(),
        started_unix: v["started_unix"].as_u64().unwrap_or(0),
        starttime_ticks: v["starttime_ticks"].as_u64().unwrap_or(0),
        exit: v["exit"].as_str().map(str::to_string),
        path,
    })
}

/// Every parseable receipt on disk (all projects). Unreadable or malformed
/// files are skipped, never fatal — receipts are best-effort recovery state.
fn load_receipts() -> Vec<Receipt> {
    let mut receipts = Vec::new();
    let directories = [receipts_dir(), proc_dir().join("completions")];
    for entry in directories
        .into_iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if let Some(receipt) = receipt_from_value(&value, path) {
            receipts.push(receipt);
        }
    }
    receipts
}

fn remove_receipt(path: &Path) {
    if let Err(error) = crate::workspace_store::remove_durable(path) {
        eprintln!(
            "[proc] failed to remove receipt {}: {error}",
            path.display()
        );
    }
}

/// This project's receipt for `id`, if one exists on disk.
fn find_receipt(
    id: u64,
    identity: &crate::workspace_store::RepoIdentity,
) -> Result<Option<Receipt>, String> {
    let mut found: Option<Receipt> = None;
    for receipt in load_receipts()
        .into_iter()
        .filter(|r| r.id == id && r.project_root == identity.root && r.project_key == identity.key)
    {
        if let Some(prior) = &found {
            if prior.pid != receipt.pid || prior.starttime_ticks != receipt.starttime_ticks {
                return Err(format!(
                    "process handle {id} is ambiguous across stored receipts; no process selected"
                ));
            }
            // A terminal receipt can briefly coexist with the running receipt
            // for the same child during publication. Prefer its known outcome.
            if prior.exit.is_some() {
                continue;
            }
        }
        found = Some(receipt);
    }
    Ok(found)
}

/// This project's receipts with no matching live-table entry — daemons spawned
/// by a previous cockpit session (or dropped from the table). A receipt whose
/// id AND pid match a live entry is the live entry's own record, not an orphan.
fn orphaned_receipts(
    identity: &crate::workspace_store::RepoIdentity,
    live: &BTreeMap<u64, ProcEntry>,
) -> Vec<Receipt> {
    load_receipts()
        .into_iter()
        .filter(|r| r.project_root == identity.root && r.project_key == identity.key)
        .filter(|r| live.get(&r.id).is_none_or(|entry| entry.pid != r.pid))
        .collect()
}

// ---------------------------------------------------------------------------
// proc_run
// ---------------------------------------------------------------------------

pub(crate) struct ProcRunTool {
    /// Default working directory for spawned processes (the workspace).
    workspace: PathBuf,
}

impl ProcRunTool {
    pub(crate) fn in_dir(dir: PathBuf) -> Self {
        Self { workspace: dir }
    }
}

impl Tool for ProcRunTool {
    fn workspace_write_scope_is_opaque(&self, _args: &Value) -> bool {
        // A retained background process can write ignored files after launch.
        // Status/stop observers deliberately do not acquire this capability.
        true
    }

    fn name(&self) -> &str {
        "proc_run"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "proc_run".to_string(),
            description: "Start a long-running command in the BACKGROUND (own process group, \
                          captured output) and return a handle id immediately. Do not redirect \
                          output: proc_status reads the captured log. Use for \
                          daemons and slow jobs — an inference server (llama-server, vllm \
                          serve), a dev server, a long build — then keep working. Take one \
                          proc_status snapshot only when its result can change your next action; \
                          if no independent work remains, use proc_wait for a bounded wait that \
                          returns early on completion rather than shell sleep. Use `shell` \
                          for commands that finish quickly. Jobs survive successful answers in \
                          this cockpit; turn cancellation/failure stops its jobs. proc_stop or \
                          cockpit exit stops remaining jobs and retains their receipts."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "command line (sh -c)" },
                    "name": {
                        "type": "string",
                        "description": "short label for the handle/log (default: first word)"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "working directory (default: the workspace)"
                    },
                },
                "required": ["command"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        if cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
            return Err("process launch cancelled before spawn".into());
        }
        // `script` is accepted as a compatibility alias: model-authored calls
        // (notably from code_mode) keep sending the wrong vocabulary, and the
        // command is unambiguous when `command` is absent.
        let command = args["command"]
            .as_str()
            .or_else(|| args["script"].as_str())
            .ok_or("missing 'command'")?;
        if command.trim().is_empty() {
            return Err("'command' must not be empty".to_string());
        }
        // Board submissions carry the harness's own attribution (see
        // `submit_identity`); this is the second way a submit can be run.
        let stamped_cwd = args["cwd"].as_str().map(PathBuf::from);
        let stamped = crate::tools::submit_identity::stamp(
            command,
            Some(stamped_cwd.as_deref().unwrap_or(&self.workspace)),
        )?;
        let command = stamped.as_ref().map_or(command, |s| s.command.as_str());
        let name = sanitize_name(
            args["name"]
                .as_str()
                .unwrap_or_else(|| command.split_whitespace().next().unwrap_or("proc")),
        );
        let identity = crate::workspace_store::repo_identity(&self.workspace);
        let dir = proc_dir().join(&identity.key);
        let id = next_id(cancel)?;
        let log = dir.join(format!("{id}-{name}.log"));
        let log_pump = Arc::new(Mutex::new(RotatingLog::new(
            log.clone(),
            proc_log_max_bytes(),
        )?));
        let log_warning = Arc::new(Mutex::new(None));

        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(self.workspace.clone());
        let mut cmd = sandbox::command("sh", ["-c", command], &policy)?;

        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let cwd = args["cwd"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.workspace.clone());
        cmd.current_dir(&cwd);
        // Own process group so proc_stop can reap the whole tree, and so the
        // turn can address this job independently of concurrent tool groups.
        cmd.process_group(0);
        let sandbox_status = sandbox::status::attach(&mut cmd)
            .map_err(|error| format!("sandbox status channel: {error}"))?;
        let mut child = cmd
            .spawn_owned()
            .map_err(|e| format!("spawn failed: {e}"))?;
        let stdout: ChildStdout = child.stdout.take().ok_or("stdout pipe unavailable")?;
        let stderr: ChildStderr = child.stderr.take().ok_or("stderr pipe unavailable")?;
        spawn_log_pump(
            stdout,
            Arc::clone(&log_pump),
            Arc::clone(&log_warning),
            "stdout",
        );
        spawn_log_pump(stderr, log_pump, Arc::clone(&log_warning), "stderr");
        // Wait only for helper setup, never for the background command. A
        // dedicated channel distinguishes setup errors from nested bwrap output.
        #[cfg(target_os = "linux")]
        let sandbox_receipt = loop {
            if let Some(receipt) = sandbox_status.receive() {
                break Some(receipt);
            }
            if child
                .try_wait()
                .map_err(|error| format!("helper wait: {error}"))?
                .is_some()
            {
                break sandbox_status.receive();
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        #[cfg(not(target_os = "linux"))]
        let sandbox_receipt = sandbox_status.receive();
        crate::harness::exec::set_sandbox_receipt(sandbox_receipt.clone());
        if let Some(error) = sandbox_receipt
            .as_ref()
            .and_then(|receipt| receipt["helper_error"].as_str())
        {
            return Err(format!("outer sandbox helper failed: {error}"));
        }
        let pid = child.id();
        // Persist the spawn receipt so a cockpit restart can adopt (or at
        // least explain) this daemon. Failure to persist must never fail the
        // spawn — but it must be said, not swallowed.
        let (receipt, receipt_note) = if receipts_enabled() {
            match write_receipt(id, pid, &name, command, &log, &identity) {
                Ok(path) => (Some(path), String::new()),
                Err(e) => (
                    None,
                    format!(
                        "\nreceipt not persisted ({e}) — a cockpit restart will lose track of \
                         this daemon"
                    ),
                ),
            }
        } else {
            (
                None,
                "\n(receipts off: ANGEL_PROC_RECEIPTS=0 — a cockpit restart will lose track of \
                 this daemon)"
                    .to_string(),
            )
        };
        if let (Some(path), Some(sandbox)) = (&receipt, &sandbox_receipt)
            && let Ok(bytes) = std::fs::read(path)
            && let Ok(mut value) = serde_json::from_slice::<Value>(&bytes)
        {
            value["sandbox_profile"] = sandbox["sandbox_profile"].clone();
            value["sandbox"] = sandbox.clone();
            if let Ok(bytes) = serde_json::to_vec(&value) {
                let _ = std::fs::write(path, bytes);
            }
        }
        table().lock().unwrap().insert(
            id,
            ProcEntry {
                owner: cancel.map_or(0, |c| c as *const _ as usize),
                cancelled: false,
                kill: None,
                project_root: identity.root,
                project_key: identity.key,
                name: name.clone(),
                command: command.to_string(),
                pid,
                log: log.clone(),
                started: Instant::now(),
                child,
                exit: None,
                exit_code: None,
                completion_reported: false,
                pending_completion: None,
                notification_workspace: std::fs::canonicalize(&self.workspace)
                    .unwrap_or_else(|_| self.workspace.clone()),
                log_warning,
                receipt,
                confinement: proc_confinement_label(),
            },
        );
        let monitor_note = ensure_completion_reaper().err().map(|error| {
            format!("\ncompletion monitor unavailable: {error}; retained process handle/receipt requires proc_status. Automatic completion notification is unavailable for this session.")
        }).unwrap_or_default();
        Ok(format!(
            "started [{id}] {name} (pid {pid})\nRead captured output through `proc_status` id={id}; use contains to filter errors.\nSnapshot with `proc_status` (no wait) \
             while you keep working; check later. Use `proc_stop` to kill the whole tree."
        ) + &receipt_note
            + &stamped.map_or(String::new(), |s| format!("\n{}", s.notice))
            + &monitor_note)
    }
}

// ---------------------------------------------------------------------------
// proc_status
// ---------------------------------------------------------------------------

pub(crate) struct ProcStatusTool {
    workspace: PathBuf,
}

impl ProcStatusTool {
    fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for ProcStatusTool {
    fn name(&self) -> &str {
        "proc_status"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "proc_status".to_string(),
            description: "Status of background processes started with proc_run. Without an id: \
                          one summary line per process. With an id: state + the tail of its \
                          log. Use contains with an id to search captured compiler errors by literal \
                          substring; captured host logs are not workspace files for grep/read_file. \
                          Always a snapshot — wait_ms is ignored so the turn cannot freeze \
                          thinking. If still running, do useful independent work. If all remaining \
                          work depends on this job, use proc_wait instead of shell sleep."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "description": "handle id from proc_run" },
                    "tail_lines": {
                        "type": "integer",
                        "description": "log lines to show for a single id (default 40)"
                    },
                    "contains": {
                        "type": "string",
                        "description": "With id only: literal case-sensitive log filter, 1–256 UTF-8 bytes, no newlines. Searches trailing 8 MiB of each retained log file; tail_lines limits matching lines."
                    },
                    "wait_ms": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 60000,
                        "description": "ignored (legacy). Snapshot only; never parks the turn."
                    },
                },
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let identity = crate::workspace_store::repo_identity(&self.workspace);
        let contains = match args.get("contains") {
            None => None,
            Some(value) => {
                let needle = value.as_str().ok_or("contains must be a string")?;
                if needle.is_empty() || needle.len() > 256 || needle.contains(['\n', '\r']) {
                    return Err("contains requires 1–256 UTF-8 bytes without newlines".into());
                }
                if args["id"].as_u64().is_none() {
                    return Err("contains requires a process id from proc_run".into());
                }
                Some(needle)
            }
        };
        if let Some(id) = args["id"].as_u64() {
            // wait_ms is accepted so old callers do not error, then dropped.
            // Parking here froze thinking for up to 60s per hop.
            let requested_wait_ms = args["wait_ms"].as_u64().unwrap_or(0);
            let n = args["tail_lines"].as_u64().unwrap_or(40).clamp(1, 400) as usize;
            let found = {
                let mut procs = table().lock().unwrap();
                procs
                    .get_mut(&id)
                    .filter(|entry| {
                        entry.project_root == identity.root && entry.project_key == identity.key
                    })
                    .map(|entry| {
                        (
                            entry.name.clone(),
                            entry.state(),
                            entry.started.elapsed(),
                            entry.pid,
                            entry.command.clone(),
                            entry.log.clone(),
                            entry
                                .log_warning
                                .lock()
                                .ok()
                                .and_then(|warning| warning.clone()),
                        )
                    })
            };
            let Some(snapshot) = found else {
                // Not in the live table — perhaps a daemon from a previous
                // cockpit session, findable through its on-disk receipt.
                return adopted_status(id, &identity, n, requested_wait_ms, contains);
            };
            let mut tail = match contains {
                Some(_) => read_log_output(&snapshot.5, n, contains),
                None => read_log_tail(&snapshot.5, n),
            };
            const MAX: usize = 16_000;
            if tail.len() > MAX {
                truncate_to_char_boundary(&mut tail, MAX);
                tail.push_str("…[truncated]");
            }
            let log_warning = snapshot
                .6
                .as_deref()
                .map(|warning| {
                    format!("\nlog warning: {warning}; later output was drained but not retained")
                })
                .unwrap_or_default();
            let running_note = if snapshot.1 == "running" {
                snapshot_running_trailer(requested_wait_ms)
            } else {
                String::new()
            };
            let text = format!(
                "[{id}] {} — {}, up {} (pid {})\ncmd: {}\ncaptured output: proc_status id={id}, optional contains filter\n--- captured log ---\n{tail}",
                snapshot.0,
                snapshot.1,
                fmt_uptime(snapshot.2),
                snapshot.3,
                snapshot.4,
            ) + &log_warning
                + &running_note;
            return if snapshot.1 == "running" || snapshot.1 == "exited 0" {
                Ok(text)
            } else {
                Err(text)
            };
        }
        let mut procs = table().lock().unwrap();
        let mut lines = Vec::new();
        let mut failed = false;
        for (id, entry) in procs.iter_mut().filter(|(_, entry)| {
            entry.project_root == identity.root && entry.project_key == identity.key
        }) {
            let state = entry.state();
            failed |= state != "running" && state != "exited 0";
            let log_state = if entry
                .log_warning
                .lock()
                .ok()
                .is_some_and(|warning| warning.is_some())
            {
                ", log degraded"
            } else {
                ""
            };
            lines.push(format!(
                "[{id}] {} — {state}, up {} (pid {}){log_state} — {}",
                entry.name,
                fmt_uptime(entry.started.elapsed()),
                entry.pid,
                entry.command
            ));
        }
        // Merge receipts with no live entry: daemons from a previous cockpit
        // session. Starttime-verified survivors are adopted; dead ones are
        // reported as failures with retained evidence and their last log lines.
        if receipts_enabled() {
            for receipt in orphaned_receipts(&identity, &procs) {
                if let Some(exit) = &receipt.exit {
                    failed |= exit != "exited 0";
                    lines.push(format!(
                        "[{}] {} — {exit} (retained terminal receipt); inspect with proc_status id={}",
                        receipt.id, receipt.name, receipt.id
                    ));
                } else if receipt_alive(&receipt) {
                    let up = now_unix().saturating_sub(receipt.started_unix);
                    lines.push(format!(
                        "[{}] {} — adopted from previous session · running, up {} (pid {}) — {}",
                        receipt.id,
                        receipt.name,
                        fmt_uptime(Duration::from_secs(up)),
                        receipt.pid,
                        receipt.command
                    ));
                } else {
                    let tail = read_log_tail(&receipt.log, 5).replace('\n', "\n    ");
                    lines.push(format!(
                        "[{}] {} — vanished — exit unknown (pid {}, daemon from a previous \
                         session) — {}\n    last log lines: {tail}",
                        receipt.id, receipt.name, receipt.pid, receipt.command
                    ));
                    failed = true;
                }
            }
        }
        let mut out = if lines.is_empty() {
            "no background processes in this project".to_string()
        } else {
            lines.join("\n")
        };
        if !receipts_enabled() {
            // The gate must speak: previous-session daemons are being skipped.
            out.push_str(
                "\n(receipts off: ANGEL_PROC_RECEIPTS=0 — daemons from previous sessions are \
                 not tracked)",
            );
        }
        if failed { Err(out) } else { Ok(out) }
    }
}

/// Trailer on a still-running snapshot so the model keeps its logic chain
/// instead of parking the turn on `wait_ms`.
fn snapshot_running_trailer(requested_wait_ms: u64) -> String {
    if requested_wait_ms == 0 {
        "\nstill running — advance useful independent work. If this job blocks all remaining \
         work, use proc_wait; do not substitute shell sleep."
            .to_string()
    } else {
        "\nstill running — snapshot only. wait_ms was ignored. Advance independent work; \
         if this job blocks all remaining work, use proc_wait instead of shell sleep."
            .to_string()
    }
}

/// Single-id `proc_status` resolution through an on-disk receipt, for a daemon
/// the live table does not know (spawned by a previous cockpit session).
/// Snapshot only — the same non-blocking contract as the live path. A dead
/// daemon is reported as vanished and its receipt cleared, so the report
/// happens exactly once.
fn adopted_status(
    id: u64,
    identity: &crate::workspace_store::RepoIdentity,
    n: usize,
    requested_wait_ms: u64,
    contains: Option<&str>,
) -> Result<String, String> {
    if !receipts_enabled() {
        return Err(format!(
            "unknown proc id {id} in this project (receipts off: ANGEL_PROC_RECEIPTS=0, so \
             daemons from previous sessions are not tracked)"
        ));
    }
    let receipt = find_receipt(id, identity)?
        .ok_or_else(|| format!("unknown proc id {id} in this project"))?;
    let alive = receipt_alive(&receipt);
    let mut tail = match contains {
        Some(_) => read_log_output(&receipt.log, n, contains),
        None => read_log_tail(&receipt.log, n),
    };
    const MAX: usize = 16_000;
    if tail.len() > MAX {
        truncate_to_char_boundary(&mut tail, MAX);
        tail.push_str("…[truncated]");
    }
    if let Some(exit) = &receipt.exit {
        let text = format!(
            "[{id}] {} — {exit} (retained terminal receipt)\ncmd: {}\ncaptured output: proc_status id={id}, optional contains filter\n--- captured log ---\n{tail}",
            receipt.name, receipt.command,
        );
        if exit == "exited 0" {
            Ok(text)
        } else {
            Err(text)
        }
    } else if alive {
        let up = now_unix().saturating_sub(receipt.started_unix);
        Ok(format!(
            "[{id}] {} — adopted from previous session · running, up {} (pid {})\ncmd: {}\ncaptured output: proc_status id={id}, optional contains filter\n--- captured log ---\n{tail}{}",
            receipt.name,
            fmt_uptime(Duration::from_secs(up)),
            receipt.pid,
            receipt.command,
            snapshot_running_trailer(requested_wait_ms),
        ))
    } else {
        Err(format!(
            "[{id}] {} — vanished — exit unknown (pid {}, daemon from a previous session); \
             receipt retained; verification inconclusive\ncmd: {}\ncaptured output: proc_status id={id}, optional contains filter\n--- captured log ---\n{tail}",
            receipt.name, receipt.pid, receipt.command,
        ))
    }
}

// ---------------------------------------------------------------------------
// proc_stop
// ---------------------------------------------------------------------------

pub(crate) struct ProcStopTool {
    workspace: PathBuf,
}

impl ProcStopTool {
    pub(crate) fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for ProcStopTool {
    fn name(&self) -> &str {
        "proc_stop"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "proc_stop".to_string(),
            description: "Stop a background process started with proc_run: SIGTERM to its whole \
                          process group, escalating to SIGKILL after a short grace period."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "description": "handle id from proc_run" },
                },
                "required": ["id"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_for(args, "proc_stop")
    }
}

impl ProcStopTool {
    fn call_for(&self, args: &Value, reason: &str) -> Result<String, String> {
        let id = args["id"].as_u64().ok_or("missing 'id'")?;
        let mut procs = table().lock().unwrap();
        let identity = crate::workspace_store::repo_identity(&self.workspace);
        let Some(entry) = procs.get_mut(&id).filter(|entry| {
            entry.project_root == identity.root && entry.project_key == identity.key
        }) else {
            // Not in the live table — perhaps adopted from a previous cockpit
            // session. Release the table lock: the adopted stop can wait out a
            // grace period and must not block the other proc tools.
            drop(procs);
            return adopted_stop(id, &identity);
        };
        if let Some(exit) = &entry.exit {
            return Ok(format!("[{id}] {} already {exit}", entry.name));
        }
        entry.state();
        if let Some(exit) = &entry.exit {
            return Ok(format!("[{id}] {} already {exit}", entry.name));
        }
        entry.cancelled = true;
        entry.kill.get_or_insert_with(|| {
            sandbox::process_owner::KillReceipt::new(
                None,
                reason,
                if reason == "proc_stop" {
                    "proc_stop"
                } else {
                    "turn_owner"
                },
            )
        });
        let pgid = entry.pid as libc::pid_t;
        // SAFETY: plain signal syscalls from the parent; no shared state.
        unsafe {
            libc::killpg(pgid, libc::SIGTERM);
        }
        let grace = Instant::now() + Duration::from_secs(3);
        let (escalated, status) = loop {
            match entry.child.try_wait() {
                Ok(Some(status)) => break (false, Some(status)),
                Ok(None) if Instant::now() >= grace => {
                    unsafe {
                        libc::killpg(pgid, libc::SIGKILL);
                    }
                    break (true, entry.child.wait().ok());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(format!("wait failed: {e}")),
            }
        };
        unsafe {
            libc::killpg(pgid, libc::SIGKILL);
        }
        use std::os::unix::process::ExitStatusExt;
        if let Some(kill) = entry.kill.as_mut() {
            kill.signal = status.and_then(|status| status.signal());
        }
        entry.exit_code = status.and_then(|status| status.code());
        entry.exit = Some(if let Some(code) = entry.exit_code {
            format!("stopped (exit {code} after SIGTERM)")
        } else if escalated {
            "killed (SIGKILL after grace)".to_string()
        } else {
            "stopped (SIGTERM)".to_string()
        });
        // The independent reaper persists and notifies this terminal state.
        Ok(format!(
            "[{id}] {} {} — log kept at {}",
            entry.name,
            entry.exit.as_deref().unwrap_or("stopped"),
            entry.log.display()
        ))
    }
}

/// `proc_stop` for a daemon known only by its on-disk receipt (adopted from a
/// previous cockpit session). Same SIGTERM → grace → SIGKILL contract as the
/// live path, addressed at the receipt's process group; liveness is judged by
/// the starttime-verified `/proc` check since there is no child handle to
/// `wait()` on. The receipt is removed once the process is confirmed gone.
fn adopted_stop(
    id: u64,
    identity: &crate::workspace_store::RepoIdentity,
) -> Result<String, String> {
    if !receipts_enabled() {
        return Err(format!(
            "unknown proc id {id} in this project (receipts off: ANGEL_PROC_RECEIPTS=0, so \
             daemons from previous sessions are not tracked)"
        ));
    }
    let receipt = find_receipt(id, identity)?
        .ok_or_else(|| format!("unknown proc id {id} in this project"))?;
    if let Some(exit) = &receipt.exit {
        return Ok(format!(
            "[{id}] {} already {exit}; terminal receipt retained",
            receipt.name
        ));
    }
    if !receipt_alive(&receipt) {
        remove_receipt(&receipt.path);
        return Ok(format!(
            "[{id}] {} already vanished — exit unknown; receipt cleared, log kept at {}",
            receipt.name,
            receipt.log.display()
        ));
    }
    // SAFETY: plain signal syscalls from the parent; the starttime check above
    // guards against signalling a recycled pid's process group.
    unsafe {
        libc::killpg(receipt.pgid, libc::SIGTERM);
    }
    let grace = Instant::now() + Duration::from_secs(3);
    let escalated = loop {
        if !receipt_alive(&receipt) {
            break false;
        }
        if Instant::now() >= grace {
            unsafe {
                libc::killpg(receipt.pgid, libc::SIGKILL);
            }
            break true;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if escalated {
        // Bounded confirmation poll — no child handle to reap.
        let deadline = Instant::now() + Duration::from_secs(2);
        while receipt_alive(&receipt) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    remove_receipt(&receipt.path);
    Ok(format!(
        "[{id}] {} (adopted from previous session) {} — log kept at {}",
        receipt.name,
        if escalated {
            "killed (SIGKILL after grace)"
        } else {
            "stopped (SIGTERM)"
        },
        receipt.log.display()
    ))
}

/// Explicit bounded waiting is separate from the always-immediate status tool.
/// Real proof tasks otherwise substitute multi-minute shell sleeps, missing
/// compiler failures and consuming their deadline without reacting to exits.
struct ProcWaitTool {
    workspace: PathBuf,
}

impl Tool for ProcWaitTool {
    fn name(&self) -> &str {
        "proc_wait"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "proc_wait".to_string(),
            description: "Wait for one background job when no useful independent work remains. \
                          Returns early when the job exits, or after at most 30 seconds, with \
                          its actual status and captured output. Cancellable. Use instead of \
                          shell sleep; use proc_status for an immediate snapshot. A running \
                          result means more work remains, not success. Process exit alone is \
                          not benchmark acceptance."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "handle id from proc_run"},
                    "tail_lines": {"type": "integer", "description": "captured log lines (default 40, maximum 400)"}
                },
                "required": ["id"]
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        let id = args["id"].as_u64().ok_or("missing or invalid 'id'")?;
        let identity = crate::workspace_store::repo_identity(&self.workspace);
        let started = Instant::now();
        loop {
            if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                return Err("process wait cancelled; no completion inferred".to_string());
            }
            let running = {
                let mut entries = table().lock().map_err(|_| "process table unavailable")?;
                entries
                    .get_mut(&id)
                    .filter(|entry| {
                        entry.project_root == identity.root && entry.project_key == identity.key
                    })
                    .is_some_and(|entry| entry.state() == "running")
            };
            if !running || started.elapsed() >= Duration::from_secs(30) {
                break;
            }
            // Never retain the process table lock while waiting. The reaper,
            // stop tool and completion delivery must continue independently.
            std::thread::sleep(Duration::from_millis(100));
        }
        // Reuse normal workspace/receipt lookup, bounded logs and exit-error
        // classification. Unknown or foreign handles never become success.
        ProcStatusTool::new(self.workspace.clone()).call(args)
    }
}

/// Register the `proc_*` background-process tools, gated on `ANGEL_PROC_TOOLS`
/// (default on). `workspace` becomes the default cwd for spawned processes.
pub(crate) fn maybe_register_proc_tools(r: &mut ToolRegistry, workspace: Option<PathBuf>) {
    if env_flag("ANGEL_PROC_TOOLS", true) {
        let workspace = workspace
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        r.register(Box::new(ProcRunTool::in_dir(workspace.clone())));
        r.register(Box::new(ProcStatusTool::new(workspace.clone())));
        r.register(Box::new(ProcWaitTool {
            workspace: workspace.clone(),
        }));
        r.register(Box::new(ProcStopTool::new(workspace)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TemporaryLogStore(PathBuf);

    impl TemporaryLogStore {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "angel-proc-{label}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&root)
                .unwrap();
            Self(std::fs::canonicalize(root).unwrap())
        }
    }

    impl Drop for TemporaryLogStore {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Lifecycle tests must never inherit the user's private process store.
    struct TestProcStore {
        root: TemporaryLogStore,
        saved: Vec<(&'static str, Option<OsString>)>,
        _env_lock: std::sync::MutexGuard<'static, ()>,
    }

    impl TestProcStore {
        fn new() -> Self {
            let env_lock = crate::tests::env_lock();
            let root = TemporaryLogStore::new("lifecycle");
            let saved = ["ANGEL_PROC_DIR", "ANGEL_PROC_RECEIPTS"]
                .into_iter()
                .map(|key| (key, std::env::var_os(key)))
                .collect();
            // SAFETY: process-global test environment changes hold env_lock.
            unsafe {
                std::env::set_var("ANGEL_PROC_DIR", root.0.join("process-store"));
                std::env::set_var("ANGEL_PROC_RECEIPTS", "1");
            }
            Self {
                root,
                saved,
                _env_lock: env_lock,
            }
        }

        fn runner(&self) -> ProcRunTool {
            ProcRunTool::in_dir(self.root.0.clone())
        }
    }

    impl Drop for TestProcStore {
        fn drop(&mut self) {
            // Reap only children belonging to this temporary project, including
            // failures before the test reached its explicit proc_stop call.
            if let Ok(mut procs) = table().lock() {
                let ids: Vec<_> = procs
                    .iter()
                    .filter(|(_, entry)| entry.project_root == self.root.0)
                    .map(|(id, _)| *id)
                    .collect();
                for id in ids {
                    if let Some(mut entry) = procs.remove(&id)
                        && matches!(entry.child.try_wait(), Ok(None))
                    {
                        // SAFETY: this live Child still owns its private process group.
                        unsafe { libc::killpg(entry.pid as libc::pid_t, libc::SIGKILL) };
                        let _ = entry.child.kill();
                        let _ = entry.child.wait();
                    }
                }
            }
            for receipt in load_receipts() {
                if receipt.project_root == self.root.0 && receipt_alive(&receipt) {
                    // SAFETY: fixture receipts pin the live child's starttime.
                    unsafe {
                        libc::killpg(receipt.pgid, libc::SIGKILL);
                        libc::waitpid(receipt.pid as libc::pid_t, std::ptr::null_mut(), 0);
                    }
                }
            }
            if let Ok(mut notices) = completion_notices().lock() {
                notices.remove(&self.root.0);
            }
            for (key, value) in &self.saved {
                // SAFETY: the fixture still owns the global environment lock.
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }

    #[test]
    fn completed_child_reaped_without_status_and_terminal_receipt_survives_restart() {
        let _env_lock = crate::tests::env_lock();
        struct RestoreEnv(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for RestoreEnv {
            fn drop(&mut self) {
                for (key, value) in &self.0 {
                    unsafe {
                        match value {
                            Some(value) => std::env::set_var(key, value),
                            None => std::env::remove_var(key),
                        }
                    }
                }
            }
        }
        let _restore = RestoreEnv(
            ["ANGEL_PROC_DIR", "ANGEL_PROC_RECEIPTS"]
                .into_iter()
                .map(|key| (key, std::env::var_os(key)))
                .collect(),
        );
        let root = std::env::temp_dir().join(format!(
            "angel-proc-completion-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        unsafe {
            std::env::set_var("ANGEL_PROC_DIR", root.join("process-store"));
            std::env::set_var("ANGEL_PROC_RECEIPTS", "1");
        }
        let run = ProcRunTool::in_dir(root.clone());
        let out = run
            .call(&serde_json::json!({
                "command": "printf 'SAT: no UNSAT proof at this m\\n'; exit 2",
                "name": "sat-proof"
            }))
            .unwrap();
        let (id, pid) = parse_handle(&out);
        let foreign = root.join("different-project");
        let deadline = Instant::now() + Duration::from_secs(5);
        let completion = loop {
            assert!(take_completions(&foreign, 8).is_empty());
            if let Some(notice) = take_completions(&root, 8).into_iter().find(|n| n.id == id) {
                break notice;
            }
            assert!(
                Instant::now() < deadline,
                "no independent completion notification"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(completion.exit_code, Some(2));
        assert_eq!(completion.state, "exited 2");
        assert!(completion.message().contains("not benchmark acceptance"));
        assert!(
            take_completions(&root, 8).is_empty(),
            "completion must be one-shot"
        );
        #[cfg(target_os = "linux")]
        assert!(
            proc_stat_fields(pid).is_none(),
            "child must be reaped without proc_status"
        );
        let terminal = completion.receipt.as_ref().expect("durable receipt path");
        let stored: Value =
            serde_json::from_str(&std::fs::read_to_string(terminal).unwrap()).unwrap();
        assert_eq!(stored["exit_code"], 2);
        assert_eq!(stored["acceptance"], "unverified");
        assert_eq!(stored["status"], "Failed");
        assert_eq!(stored["verification"], "Failed");
        assert!(!receipts_dir().join(format!("{id}-{pid}.json")).exists());
        table().lock().unwrap().remove(&id); // simulate a restarted cockpit
        for _ in 0..2 {
            let status = ProcStatusTool::new(root.clone())
                .call(&serde_json::json!({"id":id}))
                .expect_err("a retained nonzero exit must remain a failed tool receipt");
            assert!(
                status.contains("exited 2 (retained terminal receipt)"),
                "{status}"
            );
            assert!(!status.contains("vanished"), "{status}");
        }
        assert!(
            ProcStatusTool::new(foreign)
                .call(&serde_json::json!({"id":id}))
                .is_err()
        );
        let stopped = ProcStopTool::new(root.clone())
            .call(&serde_json::json!({"id":id}))
            .unwrap();
        assert!(stopped.contains("already exited 2"), "{stopped}");
        assert!(
            terminal.exists(),
            "query and stop must preserve terminal evidence"
        );
        remove_receipt(terminal);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn completion_queue_backpressure_does_not_evict_other_results() {
        let mut notices = BTreeMap::new();
        let completion = ProcCompletion {
            id: 1,
            name: "proof".into(),
            state: "exited 2".into(),
            exit_code: Some(2),
            log: "proof.log".into(),
            receipt: None,
            persistence_error: None,
        };
        let first = Path::new("/project-0");
        for project in 0..8 {
            let workspace = PathBuf::from(format!("/project-{project}"));
            for _ in 0..COMPLETION_NOTICES_PER_PROJECT {
                assert!(enqueue_completion_into(
                    &mut notices,
                    &workspace,
                    &completion
                ));
            }
        }
        assert!(!enqueue_completion_into(&mut notices, first, &completion));
        assert!(!enqueue_completion_into(
            &mut notices,
            Path::new("/new-project"),
            &completion
        ));
        assert_eq!(
            notices.values().map(VecDeque::len).sum::<usize>(),
            COMPLETION_NOTICES_TOTAL
        );
        notices.get_mut(first).unwrap().pop_front();
        assert!(enqueue_completion_into(&mut notices, first, &completion));
        assert_eq!(
            notices.len(),
            8,
            "full queues must not evict or create another project"
        );
    }

    #[test]
    fn sanitize_name_strips_shell_junk() {
        assert_eq!(sanitize_name("vllm serve ../x"), "vllm-serve----x");
        assert_eq!(sanitize_name("  "), "proc");
        assert_eq!(sanitize_name("llama-server"), "llama-server");
        assert!(sanitize_name(&"x".repeat(100)).len() <= 24);
    }

    #[test]
    fn tail_lines_takes_the_end() {
        assert_eq!(tail_lines("a\nb\nc\nd", 2), "c\nd");
        assert_eq!(tail_lines("a", 5), "a");
        assert_eq!(tail_lines("", 5), "");
    }

    #[test]
    fn rotating_log_retains_exactly_two_bounded_segments() {
        let store = TemporaryLogStore::new("rotate");
        let dir = store.0.clone();
        let path = dir.join("service.log");
        let mut log = RotatingLog::new(path.clone(), 64).unwrap();
        log.append(b"first-line-aaaaaaaaaaaaaaaaaaaa\n").unwrap();
        log.append(b"second-line-bbbbbbbbbbbbbbbbbbb\n").unwrap();
        log.append(b"third-line-cccccccccccccccccccc\n").unwrap();
        let rotated = rotated_log_path(&path);
        assert!(rotated.exists());
        assert!(std::fs::metadata(&path).unwrap().len() <= 32);
        assert!(std::fs::metadata(&rotated).unwrap().len() <= 32);
        assert!(
            std::fs::metadata(&path).unwrap().len() + std::fs::metadata(&rotated).unwrap().len()
                <= 64
        );
        let tail = read_log_tail(&path, 10);
        assert!(
            tail.contains("second-line") || tail.contains("third-line"),
            "{tail}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn private_rotating_log_creates_and_rotates_owner_only_files() {
        let store = TemporaryLogStore::new("private-modes");
        let directory = store.0.join("new-store/project");
        let path = directory.join("service.log");
        let mut log = RotatingLog::new(path.clone(), 16).unwrap();
        let original = std::fs::metadata(&path).unwrap().ino();
        assert_eq!(std::fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
        assert_eq!(
            std::fs::metadata(directory.parent().unwrap())
                .unwrap()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        log.append(b"1234").unwrap();
        log.append(b"5678").unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), original);
        log.append(b"next").unwrap();
        assert_eq!(std::fs::read(rotated_log_path(&path)).unwrap(), b"12345678");
        assert_eq!(std::fs::read(&path).unwrap(), b"next");
        for candidate in [&path, &rotated_log_path(&path)] {
            let info = std::fs::metadata(candidate).unwrap();
            assert_eq!(info.mode() & 0o777, 0o600);
            assert_eq!(info.uid(), unsafe { libc::geteuid() });
            assert_eq!(info.nlink(), 1);
        }
        log.append(b"again").unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(rotated_log_path(&path)).unwrap().mode() & 0o777,
            0o600
        );
        let existing = store.0.join("configured-parent");
        std::fs::create_dir(&existing).unwrap();
        std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o750)).unwrap();
        let _log = RotatingLog::new(existing.join("existing-parent.log"), 8).unwrap();
        assert_eq!(std::fs::metadata(existing).unwrap().mode() & 0o777, 0o750);
    }

    #[test]
    fn private_rotating_log_rejects_existing_files_and_symlink_ancestors() {
        use std::os::unix::fs::symlink;
        let store = TemporaryLogStore::new("private-create");
        let outside = store.0.join("outside.txt");
        std::fs::write(&outside, b"do not truncate").unwrap();
        let link = store.0.join("linked.log");
        symlink(&outside, &link).unwrap();
        assert!(RotatingLog::new(link, 64).is_err());
        let hardlink = store.0.join("hardlinked.log");
        std::fs::hard_link(&outside, &hardlink).unwrap();
        assert!(RotatingLog::new(hardlink, 64).is_err());
        assert!(RotatingLog::new(outside.clone(), 64).is_err());
        assert_eq!(std::fs::read(&outside).unwrap(), b"do not truncate");
        let target = store.0.join("target");
        std::fs::create_dir(&target).unwrap();
        let alias = store.0.join("alias");
        symlink(&target, &alias).unwrap();
        assert!(RotatingLog::new(alias.join("nested/service.log"), 64).is_err());
        assert!(std::fs::read_dir(target).unwrap().next().is_none());
    }

    #[test]
    fn private_rotating_log_rejects_append_and_rotation_substitutions() {
        use std::os::unix::fs::symlink;
        let store = TemporaryLogStore::new("private-substitution");
        let outside = store.0.join("outside.txt");
        std::fs::write(&outside, b"outside data").unwrap();
        let path = store.0.join("service.log");
        let mut log = RotatingLog::new(path.clone(), 8).unwrap();
        log.append(b"abcd").unwrap();
        std::fs::remove_file(&path).unwrap();
        symlink(&outside, &path).unwrap();
        assert!(log.append(b"more").is_err());
        assert_eq!(read_log_tail(&path, 10), "(log unreadable)");
        let path = store.0.join("rotation.log");
        let mut log = RotatingLog::new(path.clone(), 8).unwrap();
        log.append(b"abcd").unwrap();
        symlink(&outside, rotated_log_path(&path)).unwrap();
        assert!(log.append(b"more").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"abcd");
        let path = store.0.join("hardlink.log");
        let mut log = RotatingLog::new(path.clone(), 8).unwrap();
        log.append(b"abcd").unwrap();
        std::fs::hard_link(&path, store.0.join("second-name")).unwrap();
        assert!(log.append(b"more").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"abcd");
        assert_eq!(std::fs::read(outside).unwrap(), b"outside data");
    }

    #[test]
    fn private_rotating_log_parent_swap_stays_on_pinned_directory() {
        use std::os::unix::fs::symlink;
        let store = TemporaryLogStore::new("private-parent");
        let parent = store.0.join("logs");
        let path = parent.join("service.log");
        let mut log = RotatingLog::new(path.clone(), 8).unwrap();
        log.append(b"abcd").unwrap();
        let retained = store.0.join("retained");
        std::fs::rename(&parent, &retained).unwrap();
        let outside = store.0.join("outside");
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, &parent).unwrap();
        log.append(b"next").unwrap();
        assert_eq!(
            std::fs::read(retained.join("service.log")).unwrap(),
            b"next"
        );
        assert_eq!(
            std::fs::read(retained.join("service.log.1")).unwrap(),
            b"abcd"
        );
        assert!(std::fs::read_dir(outside).unwrap().next().is_none());
        assert_eq!(read_log_tail(&path, 10), "(log unreadable)");
    }

    #[test]
    fn log_pump_drains_after_log_write_failure() {
        struct CountingReader {
            remaining: usize,
            consumed: Arc<std::sync::atomic::AtomicUsize>,
        }

        impl Read for CountingReader {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                let read = bytes.len().min(self.remaining);
                bytes[..read].fill(b'x');
                self.remaining -= read;
                self.consumed.fetch_add(read, Ordering::Relaxed);
                Ok(read)
            }
        }

        let store = TemporaryLogStore::new("drain-failure");
        let dir = store.0.clone();
        let path = dir.join("service.log");
        let log = Arc::new(Mutex::new(RotatingLog::new(path.clone(), 64).unwrap()));

        // Make the first append fail deterministically. The pump must still
        // consume every later byte rather than closing a live child's pipe.
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
        let total = 3 * 16 * 1024 + 7;
        let consumed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let warning = Arc::new(Mutex::new(None));
        pump_log(
            CountingReader {
                remaining: total,
                consumed: Arc::clone(&consumed),
            },
            log,
            Arc::clone(&warning),
            "stdout",
        );

        assert_eq!(consumed.load(Ordering::Relaxed), total);
        let warning = warning.lock().unwrap().clone().expect("log warning");
        assert!(warning.contains("stdout log write failed"), "{warning}");
    }

    fn assert_owned_kill_receipt(reason: &str) {
        let fixture = TestProcStore::new();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let owner = &cancel as *const _ as usize;
        let args = serde_json::json!({"command":"sleep 300", "name":"kill-receipt"});
        let result = fixture
            .runner()
            .call_with_cancel(&args, Some(&cancel))
            .unwrap();
        let (id, pid) = parse_handle(&result);
        crate::harness::note_tool_outcome(
            1,
            "proc_run",
            &args,
            &result,
            "ok",
            false,
            None,
            None,
            None,
            result.len(),
        );
        stop_owned_for(owner, reason);
        let kills = take_owned_kills(owner);
        assert_eq!(kills.len(), 1);
        assert_eq!(kills[0].0, id);
        assert_eq!(kills[0].1.reason, reason);
        assert_eq!(kills[0].1.owner, "turn_owner");
        assert_eq!(kills[0].1.signal, Some(libc::SIGTERM));
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        assert!(
            take_owned_kills(owner).is_empty(),
            "owner identity released"
        );
        crate::harness::note_proc_kills(&kills);
        let ledger = crate::harness::tool_ledger_snapshot();
        let entry = ledger.iter().find(|entry| entry["proc_id"] == id).unwrap();
        assert_eq!(entry["status"], "killed");
        assert_eq!(entry["kill"]["reason"], reason);
        let terminal: Value = serde_json::from_slice(
            &std::fs::read(
                proc_dir()
                    .join("completions")
                    .join(format!("{id}-{pid}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(terminal["kill"], entry["kill"]);
        eprintln!(
            "LIFECYCLE_PROC_KILL reason={reason} signal=15 owner=turn_owner ledger=killed reaped=true"
        );
    }

    #[test]
    fn lifecycle_deadline_owner_transfer_survives_nested_unwind_and_terminal_kill() {
        let fixture = TestProcStore::new();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let owner = &cancel as *const _ as usize;
        for terminal in [false, true] {
            let mut handle = None;
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::harness::with_turn_deadline_cancel(
                    &cancel,
                    Some(Instant::now() + Duration::from_secs(60)),
                    |outer| {
                        crate::harness::with_turn_deadline_cancel(
                            outer,
                            Some(Instant::now() + Duration::from_secs(60)),
                            |inner| {
                                let result = fixture
                                    .runner()
                                    .call_with_cancel(
                                        &serde_json::json!({"command":"sleep 300"}),
                                        Some(inner),
                                    )
                                    .unwrap();
                                handle = Some(parse_handle(&result));
                                if terminal {
                                    stop_owned_for(inner as *const _ as usize, "deadline");
                                }
                                panic!("fixture unwinds after launch");
                            },
                        )
                    },
                );
            }));
            assert!(unwind.is_err());
            let (id, pid) = handle.unwrap();
            assert_eq!(table().lock().unwrap().get(&id).unwrap().owner, owner);
            stop_owned_for(owner, "owner_reap");
            let kills = take_owned_kills(owner);
            assert_eq!(kills.len(), 1);
            assert_eq!(kills[0].0, id);
            assert_eq!(
                kills[0].1.reason,
                if terminal { "deadline" } else { "owner_reap" }
            );
            assert!(!Path::new(&format!("/proc/{pid}")).exists());
        }
    }

    #[test]
    fn successful_turn_keeps_background_job_until_explicit_stop() {
        // env-lock-exempt: TestProcStore owns env_lock through all restoration guards.
        use crate::club::{ChatMsg, Club, ClubReply, ToolCall};
        use std::sync::atomic::AtomicBool;
        let fixture = TestProcStore::new();
        let _advisor = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "0");
        let _skills = crate::tests::TestEnvGuard::set("ANGEL_SKILL_HINT", "0");
        let _relentless = crate::tests::TestEnvGuard::set("ANGEL_RELENTLESS_EXECUTION", "0");
        struct ServiceClub(AtomicBool);
        impl Club for ServiceClub {
            fn label(&self) -> &str {
                "service-lifecycle"
            }
            fn respond(&self, _: &str) -> Result<String, String> {
                Ok("The service is running.".into())
            }
            fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
                if self.0.swap(true, Ordering::SeqCst) {
                    Ok(ClubReply::Text("The service is running.".into()))
                } else {
                    Ok(ClubReply::Calls(vec![ToolCall {
                        id: "start-service".into(),
                        name: "proc_run".into(),
                        args: serde_json::json!({"command":"sleep 300", "name":"service"}),
                    }]))
                }
            }
        }
        let mut registry = ToolRegistry::new();
        registry.set_workspace(fixture.root.0.clone());
        registry.register(Box::new(fixture.runner()));
        let (events, _received) = std::sync::mpsc::channel();
        let answer = crate::harness::run_turn(
            &ServiceClub(AtomicBool::new(false)),
            &registry,
            &mut vec![ChatMsg::user("Start a background service in this cockpit.")],
            &AtomicBool::new(false),
            Some(4),
            &events,
        )
        .unwrap();
        assert!(answer.contains("service is running"));
        let id = {
            let mut procs = table().lock().unwrap();
            let (id, entry) = procs
                .iter_mut()
                .find(|(_, entry)| entry.project_root == fixture.root.0)
                .expect("the turn started its service");
            assert_eq!(
                entry.state(),
                "running",
                "an answer must not kill the service"
            );
            assert_eq!(entry.owner, 0, "release the expired turn identity");
            *id
        };
        ProcStopTool::new(fixture.root.0.clone())
            .call(&serde_json::json!({"id":id}))
            .unwrap();
        assert_ne!(
            table().lock().unwrap().get_mut(&id).unwrap().state(),
            "running"
        );
    }

    #[test]
    fn lifecycle_proc_turn_cancel_receipt() {
        assert_owned_kill_receipt("cancelled");
    }

    #[test]
    fn lifecycle_proc_deadline_receipt() {
        assert_owned_kill_receipt("deadline");
    }

    #[test]
    fn lifecycle_proc_idle_receipt() {
        assert_owned_kill_receipt("tool_idle");
    }

    #[test]
    fn lifecycle_proc_owner_reap_receipt() {
        assert_owned_kill_receipt("owner_reap");
    }

    #[test]
    fn lifecycle_turn_end_stops_only_owned_proc_jobs() {
        let fixture = TestProcStore::new();
        let runner = fixture.runner();
        let cancel_a = std::sync::atomic::AtomicBool::new(false);
        let cancel_b = std::sync::atomic::AtomicBool::new(false);
        let a = runner
            .call_with_cancel(
                &serde_json::json!({"command":"sleep 300", "name":"owned-a"}),
                Some(&cancel_a),
            )
            .unwrap();
        let b = runner
            .call_with_cancel(
                &serde_json::json!({"command":"sleep 300", "name":"owned-b"}),
                Some(&cancel_b),
            )
            .unwrap();
        let (a_id, a_pid) = parse_handle(&a);
        let (b_id, _) = parse_handle(&b);
        stop_owned(&cancel_a as *const _ as usize);
        assert!(
            !Path::new(&format!("/proc/{a_pid}")).exists(),
            "owned worker not reaped"
        );
        let mut procs = table().lock().unwrap();
        assert!(procs.get_mut(&a_id).unwrap().state().contains("stopped"));
        assert_eq!(procs.get_mut(&b_id).unwrap().state(), "running");
        drop(procs);
        let terminal: Value = serde_json::from_slice(
            &std::fs::read(
                proc_dir()
                    .join("completions")
                    .join(format!("{a_id}-{a_pid}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(terminal["status"], "Cancelled");
        assert_eq!(terminal["verification"], "Inconclusive");
        assert!(
            ProcStatusTool::new(runner.workspace.clone())
                .call(&serde_json::json!({"id":a_id}))
                .is_err()
        );
        stop_owned(&cancel_b as *const _ as usize);
        eprintln!("LIFECYCLE_PROC_RECEIPT owned_pid_reaped=true unrelated_turn_preserved=true");
    }

    #[test]
    fn run_status_stop_roundtrip() {
        let store = TestProcStore::new();
        let run = store.runner();
        let workspace = run.workspace.clone();
        let out = run
            .call(&serde_json::json!({
                "command": "echo ready; sleep 30",
                "name": "test-daemon"
            }))
            .expect("spawn");
        let id: u64 = out
            .split('[')
            .nth(1)
            .and_then(|s| s.split(']').next())
            .and_then(|s| s.parse().ok())
            .expect("id in output");
        // Give the shell a beat to write "ready" to the log.
        std::thread::sleep(Duration::from_millis(300));
        let foreign =
            std::env::temp_dir().join(format!("angel-proc-foreign-{}", std::process::id()));
        assert!(
            ProcStatusTool::new(foreign.clone())
                .call(&serde_json::json!({ "id": id }))
                .is_err()
        );
        assert!(
            ProcStopTool::new(foreign)
                .call(&serde_json::json!({ "id": id }))
                .is_err()
        );
        let status = ProcStatusTool::new(workspace.clone())
            .call(&serde_json::json!({ "id": id }))
            .expect("status");
        assert!(status.contains("running"), "expected running: {status}");
        assert!(status.contains("ready"), "expected log tail: {status}");
        let stopped = ProcStopTool::new(workspace)
            .call(&serde_json::json!({ "id": id }))
            .expect("stop");
        assert!(
            stopped.contains("stopped") || stopped.contains("killed"),
            "unexpected stop result: {stopped}"
        );
    }

    #[test]
    fn proc_status_schema_does_not_advertise_host_wait() {
        let tool = ProcStatusTool::new(PathBuf::from("."));
        let def = tool.def();
        assert!(
            def.description.contains("wait_ms is ignored"),
            "{}",
            def.description
        );
        assert!(
            !def.description.contains("60000"),
            "must not teach a 60s park: {}",
            def.description
        );
        let wait = def.params["properties"]["wait_ms"]["description"]
            .as_str()
            .expect("wait_ms description");
        assert!(wait.contains("ignored"), "{wait}");
    }

    #[test]
    fn status_does_not_park_the_turn_on_wait_ms() {
        let store = TestProcStore::new();
        let run = store.runner();
        let workspace = run.workspace.clone();
        let out = run
            .call(&serde_json::json!({
                "command": "sleep 30; echo complete",
                "name": "wait-ignored-test"
            }))
            .expect("spawn");
        assert!(
            !out.contains("wait_ms"),
            "spawn must not teach wait_ms: {out}"
        );
        let id: u64 = out
            .split('[')
            .nth(1)
            .and_then(|s| s.split(']').next())
            .and_then(|s| s.parse().ok())
            .expect("id in output");
        struct StopOnDrop {
            workspace: PathBuf,
            id: u64,
        }
        impl Drop for StopOnDrop {
            fn drop(&mut self) {
                let _ = ProcStopTool::new(self.workspace.clone())
                    .call(&serde_json::json!({ "id": self.id }));
            }
        }
        let _stop = StopOnDrop {
            workspace: workspace.clone(),
            id,
        };
        let started = Instant::now();
        let status = ProcStatusTool::new(workspace)
            .call(&serde_json::json!({ "id": id, "wait_ms": 60_000 }))
            .expect("snapshot status");
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(750),
            "wait_ms must not park the turn ({elapsed:?}): {status}"
        );
        assert!(status.contains("running"), "{status}");
        assert!(
            status.contains("wait_ms was ignored"),
            "running snapshot must say wait_ms was ignored: {status}"
        );
    }

    #[test]
    fn finite_job_is_visible_on_a_later_snapshot() {
        let store = TestProcStore::new();
        let run = store.runner();
        let workspace = run.workspace.clone();
        let out = run
            .call(&serde_json::json!({
                "command": "sleep 0.1; echo complete",
                "name": "finite-test"
            }))
            .expect("spawn");
        let id: u64 = out
            .split('[')
            .nth(1)
            .and_then(|s| s.split(']').next())
            .and_then(|s| s.parse().ok())
            .expect("id in output");
        let status = ProcStatusTool::new(workspace);
        let started = Instant::now();
        let snapshot = loop {
            let text = status
                .call(&serde_json::json!({ "id": id }))
                .expect("status");
            if text.contains("exited 0") {
                break text;
            }
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "finite job never exited: {text}"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(snapshot.contains("complete"), "{snapshot}");
    }

    fn parse_handle(out: &str) -> (u64, u32) {
        let id = out
            .split('[')
            .nth(1)
            .and_then(|s| s.split(']').next())
            .and_then(|s| s.parse().ok())
            .expect("id in output");
        let pid = out
            .split("(pid ")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .and_then(|s| s.parse().ok())
            .expect("pid in output");
        (id, pid)
    }

    #[test]
    fn proc_stat_fields_reads_state_and_starttime() {
        let (state, starttime) = proc_stat_fields(std::process::id()).expect("own /proc stat");
        assert!(matches!(state, 'R' | 'S' | 'D'), "state {state}");
        assert!(starttime > 0);
    }

    #[test]
    fn receipts_adopt_then_vanish_across_simulated_restart() {
        let store = TestProcStore::new();
        let run = store.runner();
        let workspace = run.workspace.clone();
        let out = run
            .call(&serde_json::json!({
                "command": "echo receipt-ready; sleep 300",
                "name": "receipt-daemon"
            }))
            .expect("spawn");
        let (id, pid) = parse_handle(&out);
        // Receipt written at spawn, starttime matching the live /proc value.
        let receipt_path = receipts_dir().join(format!("{id}-{pid}.json"));
        assert!(receipt_path.exists(), "receipt written at spawn: {out}");
        let receipt: Value =
            serde_json::from_str(&std::fs::read_to_string(&receipt_path).unwrap()).unwrap();
        let (_, live_starttime) = proc_stat_fields(pid).expect("live /proc stat");
        assert_eq!(
            receipt["starttime_ticks"].as_u64().unwrap(),
            live_starttime,
            "receipt starttime must match /proc"
        );
        assert_eq!(receipt["pgid"].as_u64().unwrap(), pid as u64);
        // Simulate a cockpit restart for this daemon: clear its entry from the
        // static table. (The table is process-wide and the other proc tests
        // run in parallel against it, so clear only this entry — the effect
        // for the entry under test is identical to a full restart wipe.)
        table().lock().unwrap().remove(&id);
        // List mode adopts the starttime-verified survivor.
        let status = ProcStatusTool::new(workspace.clone())
            .call(&serde_json::json!({}))
            .expect("status");
        assert!(
            status.contains("adopted from previous session · running"),
            "{status}"
        );
        assert!(status.contains(&format!("(pid {pid})")), "{status}");
        // Single-id detail resolves through the receipt too, log tail included.
        let detail = ProcStatusTool::new(workspace.clone())
            .call(&serde_json::json!({ "id": id }))
            .expect("detail");
        assert!(
            detail.contains("adopted from previous session · running"),
            "{detail}"
        );
        assert!(detail.contains("receipt-ready"), "{detail}");
        // Kill it out from under the receipt: the next status observing the
        // death reports a failed, unknown exit and retains the evidence.
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGKILL);
            libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let vanished_marker = format!("(pid {pid}, daemon from a previous session)");
        let vanished = loop {
            let status = ProcStatusTool::new(workspace.clone())
                .call(&serde_json::json!({}))
                .expect_err("a vanished worker must not be a successful snapshot");
            if status.contains("vanished") && status.contains(&vanished_marker) {
                break status;
            }
            assert!(Instant::now() < deadline, "never saw vanished: {status}");
            std::thread::sleep(Duration::from_millis(100));
        };
        assert!(vanished.contains("exit unknown"), "{vanished}");
        assert!(vanished.contains("last log lines"), "{vanished}");
        assert!(
            receipt_path.exists(),
            "receipt retained when the vanish is observed"
        );
    }

    #[test]
    fn proc_stop_stops_an_adopted_daemon_by_pgid() {
        let store = TestProcStore::new();
        let run = store.runner();
        let workspace = run.workspace.clone();
        let out = run
            .call(&serde_json::json!({
                "command": "sleep 300",
                "name": "receipt-stoppee"
            }))
            .expect("spawn");
        let (id, pid) = parse_handle(&out);
        let receipt_path = receipts_dir().join(format!("{id}-{pid}.json"));
        assert!(receipt_path.exists(), "receipt written at spawn: {out}");
        // Orphan it (simulated restart), then stop it through the receipt.
        table().lock().unwrap().remove(&id);
        let stopped = ProcStopTool::new(workspace)
            .call(&serde_json::json!({ "id": id }))
            .expect("adopted stop");
        assert!(
            stopped.contains("adopted from previous session"),
            "{stopped}"
        );
        assert!(
            stopped.contains("stopped") || stopped.contains("killed"),
            "{stopped}"
        );
        assert!(!receipt_path.exists(), "receipt cleared after adopted stop");
        // The process itself must be gone (dead, or an unreaped zombie of this
        // test process, which /proc reports as state Z).
        let gone = proc_stat_fields(pid).is_none_or(|(state, _)| matches!(state, 'Z' | 'X' | 'x'));
        assert!(gone, "pid {pid} survived adopted proc_stop");
        // The simulated restart dropped Child, but this test remains its real
        // parent and must reap the stopped shell before leaving the fixture.
        unsafe { libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0) };
    }
}
