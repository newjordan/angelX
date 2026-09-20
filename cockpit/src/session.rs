//! Session log + resume — so a cutoff is recoverable.
//!
//! Each cockpit run owns a [`Session`] that snapshots the full model-facing
//! thread to `~/.angel0/sessions/<id>.json` after every turn. Every snapshot is
//! bound to the canonical repository that created it; unbound legacy snapshots
//! and snapshots from another repository fail closed on list/resume. The write is
//! atomic (temp file + rename), so a crash leaves the last completed turn
//! intact. `/sessions` lists saved sessions (newest first) and `/resume [id]`
//! reloads one and continues writing to it.
//!
//! The id is `<unix_ms>-<pid>-<process_nonce>` so rapid project switches cannot
//! collide; listing sorts by
//! file mtime (most-recently-active first). Override the directory with
//! `ANGEL_SESSION_DIR` (used by tests).

use crate::club::{ChatMsg, ChatRole};
use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

const SESSION_SCHEMA: &str = "angel-project-session/v1";
const SESSION_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(not(test))]
const SESSION_WRITER_QUEUE_CAPACITY: usize = 8;
// Unit tests create many unrelated fake sessions in one process. Their shared
// writer is a harness artifact; the capacity-one admission regression below
// pins production semantics without making parallel fixtures contend.
#[cfg(test)]
const SESSION_WRITER_QUEUE_CAPACITY: usize = 128;

#[derive(Clone, Serialize, Deserialize)]
struct SessionRecord {
    schema: String,
    workspace: PathBuf,
    project_key: String,
    history: Vec<ChatMsg>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SessionSaveError {
    Unbound,
    CreateDirectory(String),
    Serialize(String),
    BindingMismatch,
    Write(String),
    Rename(String),
    WriterBacklog,
    WriterUnavailable,
    WriterTimeout(SessionWriterTimeout),
}

impl std::fmt::Display for SessionSaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unbound => write!(f, "session is not bound to a project"),
            Self::CreateDirectory(error) => write!(f, "create session directory: {error}"),
            Self::Serialize(error) => write!(f, "serialize session: {error}"),
            Self::BindingMismatch => write!(f, "session snapshot belongs to another project"),
            Self::Write(error) => write!(f, "write session snapshot: {error}"),
            Self::Rename(error) => write!(f, "commit session snapshot: {error}"),
            Self::WriterBacklog => write!(f, "session background writer queue is full"),
            Self::WriterUnavailable => write!(f, "session background writer is unavailable"),
            Self::WriterTimeout(detail) => write!(
                f,
                "session background writer flush timed out (attempt={} elapsed_ms={} queue_ms={} phase={} phase_ms={})",
                detail.attempt_id,
                detail.elapsed_ms,
                detail.queue_ms,
                detail.phase.label(),
                detail.phase_ms
            ),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum SessionSaveStatus {
    #[default]
    Healthy,
    Pending,
    Failed(SessionSaveError),
}

/// Payload-free state sampled when a wait expires. Values belong to that
/// attempt and that instant; later saves or late completion cannot rewrite them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionWriterTimeout {
    pub(crate) attempt_id: u64,
    pub(crate) elapsed_ms: u128,
    pub(crate) queue_ms: u128,
    pub(crate) phase: SessionWritePhase,
    pub(crate) phase_ms: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SessionWritePhase {
    Queued,
    Assemble,
    CreateDirectory,
    Serialize,
    CheckBinding,
    OpenTemporary,
    WriteTemporary,
    SyncFile,
    Rename,
    SyncDirectory,
}

impl SessionWritePhase {
    fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Assemble => "assemble",
            Self::CreateDirectory => "create-directory",
            Self::Serialize => "serialize",
            Self::CheckBinding => "check-binding",
            Self::OpenTemporary => "open-temporary",
            Self::WriteTemporary => "write-temporary",
            Self::SyncFile => "sync-file",
            Self::Rename => "rename",
            Self::SyncDirectory => "sync-directory",
        }
    }
}

struct SaveAttemptState {
    result: Option<Result<(), SessionSaveError>>,
    writer_started: Option<Instant>,
    phase: SessionWritePhase,
    phase_started: Instant,
}

struct SaveAttempt {
    id: u64,
    admitted_at: Instant,
    state: Mutex<SaveAttemptState>,
    completed: Condvar,
}

impl Default for SaveAttempt {
    fn default() -> Self {
        static NEXT_ATTEMPT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let now = Instant::now();
        Self {
            id: NEXT_ATTEMPT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            admitted_at: now,
            state: Mutex::new(SaveAttemptState {
                result: None,
                writer_started: None,
                phase: SessionWritePhase::Queued,
                phase_started: now,
            }),
            completed: Condvar::new(),
        }
    }
}

impl SaveAttempt {
    fn enter_phase(&self, phase: SessionWritePhase) {
        if let Ok(mut state) = self.state.lock() {
            let now = Instant::now();
            state.writer_started.get_or_insert(now);
            state.phase = phase;
            state.phase_started = now;
        }
    }

    fn finish(&self, result: Result<(), SessionSaveError>) {
        if let Ok(mut state) = self.state.lock() {
            state.result = Some(result);
            self.completed.notify_all();
        }
    }

    fn wait(&self, timeout: Duration) -> Result<(), SessionSaveError> {
        let state = self
            .state
            .lock()
            .map_err(|_| SessionSaveError::WriterUnavailable)?;
        let (state, _) = self
            .completed
            .wait_timeout_while(state, timeout, |value| value.result.is_none())
            .map_err(|_| SessionSaveError::WriterUnavailable)?;
        if let Some(result) = &state.result {
            return result.clone();
        }
        // Freeze the phase and timings under the completion mutex. Do not
        // publish timeout as the I/O result: the exact attempt may settle late.
        let now = Instant::now();
        Err(SessionSaveError::WriterTimeout(SessionWriterTimeout {
            attempt_id: self.id,
            elapsed_ms: now.saturating_duration_since(self.admitted_at).as_millis(),
            queue_ms: state
                .writer_started
                .unwrap_or(now)
                .saturating_duration_since(self.admitted_at)
                .as_millis(),
            phase: state.phase,
            phase_ms: now
                .saturating_duration_since(state.phase_started)
                .as_millis(),
        }))
    }

    fn status(&self) -> SessionSaveStatus {
        match self.state.lock() {
            Ok(state) => match state.result.as_ref() {
                Some(Ok(())) => SessionSaveStatus::Healthy,
                Some(Err(error)) => SessionSaveStatus::Failed(error.clone()),
                None => SessionSaveStatus::Pending,
            },
            Err(_) => SessionSaveStatus::Failed(SessionSaveError::WriterUnavailable),
        }
    }
}

#[derive(Default)]
struct SavePublication {
    latest_attempt: Option<Arc<SaveAttempt>>,
    // One shared snapshot per live session publication, including after save.
    // Payload buffers remain Arc-shared; this never accumulates older attempts.
    latest_snapshot: Option<PublishedSnapshot>,
}

struct PublishedSnapshot {
    session_id: String,
    path: PathBuf,
    workspace: PathBuf,
    project_key: String,
    history: Arc<[ChatMsg]>,
}

impl PublishedSnapshot {
    fn matches(&self, session: &Session, history: &[ChatMsg]) -> bool {
        self.session_id == session.id
            && self.path == session.path
            && session.workspace.as_ref() == Some(&self.workspace)
            && session.project_key.as_ref() == Some(&self.project_key)
            && same_persisted_history(&self.history, history)
    }
}

/// Compare every serialized message field. Transient reasoning and execution
/// receipts deliberately do not affect the persisted checkpoint identity.
fn same_persisted_history(left: &[ChatMsg], right: &[ChatMsg]) -> bool {
    use crate::club::Media;
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.role == right.role
                && left.content == right.content
                && left.recovery_context == right.recovery_context
                && left.tool_call_id == right.tool_call_id
                && (Arc::ptr_eq(&left.tool_calls, &right.tool_calls)
                    || (left.tool_calls.len() == right.tool_calls.len()
                        && left.tool_calls.iter().zip(right.tool_calls.iter()).all(
                            |(left, right)| {
                                left.id == right.id
                                    && left.name == right.name
                                    && left.args == right.args
                                    // Value equality aliases -0.0 and +0.0. Exit
                                    // reuse requires the exact persisted encoding.
                                    && serde_json::to_vec(&left.args)
                                        .ok()
                                        .zip(serde_json::to_vec(&right.args).ok())
                                        .is_some_and(|(left, right)| left == right)
                            },
                        )))
                && (Arc::ptr_eq(&left.attachments, &right.attachments)
                    || (left.attachments.len() == right.attachments.len()
                        && left.attachments.iter().zip(right.attachments.iter()).all(
                            |(left, right)| match (left, right) {
                                (
                                    Media::Image { mime: a, b64: x },
                                    Media::Image { mime: b, b64: y },
                                ) => a == b && x == y,
                                (
                                    Media::Audio { format: a, b64: x },
                                    Media::Audio { format: b, b64: y },
                                ) => a == b && x == y,
                                _ => false,
                            },
                        )))
        })
}

/// Bind keys + a history snapshot. `SessionRecord` (schema + serde document)
/// is assembled on the session-saver thread, not the caller. History is an
/// `Arc` so a submit path can share one Vec clone with `Thinking::spawn`.
struct SaveSnapshot {
    path: PathBuf,
    workspace: PathBuf,
    project_key: String,
    history: Arc<[ChatMsg]>,
    attempt: Arc<SaveAttempt>,
}

enum WriterJob {
    Save(SaveSnapshot),
    #[cfg(test)]
    Barrier(mpsc::Sender<()>),
}

fn writer_sender() -> &'static mpsc::SyncSender<WriterJob> {
    static WRITER: std::sync::OnceLock<mpsc::SyncSender<WriterJob>> = std::sync::OnceLock::new();
    WRITER.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<WriterJob>(SESSION_WRITER_QUEUE_CAPACITY);
        let _ = std::thread::Builder::new()
            .name("session-saver".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    match job {
                        WriterJob::Save(snapshot) => write_queued_snapshot(snapshot),
                        #[cfg(test)]
                        WriterJob::Barrier(reply) => {
                            let _ = reply.send(());
                        }
                    }
                }
            });
        tx
    })
}

fn write_queued_snapshot(snapshot: SaveSnapshot) {
    write_queued_snapshot_observed(snapshot, |_| {});
}

// The observer is empty in production. Tests can hold one owned attempt at a
// real phase boundary without a process-global hook or changing I/O semantics.
fn write_queued_snapshot_observed(
    snapshot: SaveSnapshot,
    mut observe: impl FnMut(SessionWritePhase),
) {
    let mut phase = |current| {
        snapshot.attempt.enter_phase(current);
        observe(current);
    };
    phase(SessionWritePhase::Assemble);
    let record = assemble_record(snapshot.workspace, snapshot.project_key, snapshot.history);
    let result = write_snapshot(&snapshot.path, &record, &mut phase);
    snapshot.attempt.finish(result);
}

fn enqueue_writer(
    sender: &mpsc::SyncSender<WriterJob>,
    job: WriterJob,
) -> Result<(), SessionSaveError> {
    sender.try_send(job).map_err(|error| match error {
        mpsc::TrySendError::Full(_) => SessionSaveError::WriterBacklog,
        mpsc::TrySendError::Disconnected(_) => SessionSaveError::WriterUnavailable,
    })
}

/// Directory holding session snapshots.
pub fn sessions_dir() -> PathBuf {
    std::env::var_os("ANGEL_SESSION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".angel0/sessions")
        })
}

fn gen_id() -> String {
    static NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let nonce = NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{now_ms:013}-{}-{nonce}", std::process::id())
}

/// A live session: an id and the file it snapshots to.
#[derive(Clone)]
pub struct Session {
    #[allow(dead_code)] // identity of the session; the file name derives from it
    pub id: String,
    path: PathBuf,
    disabled: bool,
    workspace: Option<PathBuf>,
    project_key: Option<String>,
    save_status: Arc<Mutex<SavePublication>>,
}

impl Session {
    /// Start a fresh session in the default directory.
    pub fn new() -> Self {
        Self::at(sessions_dir(), gen_id())
    }
    /// Session sink for static preview/test renders that must not touch disk.
    pub fn disabled() -> Self {
        Self {
            id: "disabled".to_string(),
            path: PathBuf::new(),
            disabled: true,
            workspace: None,
            project_key: None,
            save_status: Arc::new(Mutex::new(SavePublication::default())),
        }
    }

    pub(crate) fn is_disabled(&self) -> bool {
        self.disabled
    }
    /// Re-open an existing session id for the same canonical repository.
    pub fn with_id_for(id: &str, workspace: &Path) -> Self {
        let mut session = Self::at(sessions_dir(), id.to_string());
        session.bind(workspace);
        session
    }
    /// Explicit dir + id + repository binding (testable without env).
    #[cfg(test)]
    pub fn at_for(dir: PathBuf, id: String, workspace: &Path) -> Self {
        let mut session = Self::at(dir, id);
        session.bind(workspace);
        session
    }
    /// Explicit dir + id (testable without env).
    pub fn at(dir: PathBuf, id: String) -> Self {
        let path = dir.join(format!("{id}.json"));
        Self {
            id,
            path,
            disabled: false,
            workspace: None,
            project_key: None,
            save_status: Arc::new(Mutex::new(SavePublication::default())),
        }
    }

    /// Bind this sink to the canonical repository before it can persist.
    pub fn bind(&mut self, workspace: &Path) {
        let identity = crate::workspace_store::repo_identity(workspace);
        self.workspace = Some(identity.root);
        self.project_key = Some(identity.key);
    }
    #[allow(dead_code)] // accessor for the session file path (tests / future callers)
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomically snapshot the thread after every older queued snapshot.
    /// Boundary commands use this blocking form so an older turn save can
    /// never land afterward and roll the session back. No-op for an empty
    /// thread (avoids stub files).
    pub fn save(&self, history: &[ChatMsg]) -> Result<(), SessionSaveError> {
        if self.disabled || history.is_empty() {
            return Ok(());
        }
        self.checkpoint(history)
    }

    /// `save`, off the calling thread. The submit/turn-landing paths run on the
    /// UI thread, where assembling and serializing a whole long conversation
    /// plus the write is a visible frame hitch — those enqueue bind keys and a
    /// history snapshot to one global writer thread, which builds the
    /// `SessionRecord` and writes it. A single consumer and an eight-job bound
    /// keep writes ordered without retaining unlimited full-history snapshots
    /// behind a slow filesystem. Saturation fails visibly and nonblockingly;
    /// the last completed atomic snapshot remains recoverable.
    pub fn save_async(&self, history: &[ChatMsg]) -> Result<(), SessionSaveError> {
        if self.disabled || history.is_empty() {
            return Ok(());
        }
        self.save_history(Arc::from(history))
    }

    /// Enqueue an already-cloned history snapshot. Callers that also spawn a
    /// turn worker pass the same `Arc` so the UI thread pays one Vec clone.
    pub(crate) fn save_history(&self, history: Arc<[ChatMsg]>) -> Result<(), SessionSaveError> {
        if self.disabled || history.is_empty() {
            return Ok(());
        }
        self.queue_history(writer_sender(), history).map(|_| ())
    }

    fn queue_history(
        &self,
        sender: &mpsc::SyncSender<WriterJob>,
        history: Arc<[ChatMsg]>,
    ) -> Result<Arc<SaveAttempt>, SessionSaveError> {
        self.queue_history_with_reuse(sender, history, false)
    }

    fn queue_history_with_reuse(
        &self,
        sender: &mpsc::SyncSender<WriterJob>,
        history: Arc<[ChatMsg]>,
        reuse_latest: bool,
    ) -> Result<Arc<SaveAttempt>, SessionSaveError> {
        // Serialize nonblocking admission across cloned handles. The writer
        // completes only the per-attempt receipt and holds no publication lock.
        let mut published = self
            .save_status
            .lock()
            .map_err(|_| SessionSaveError::WriterUnavailable)?;
        if reuse_latest
            && let (Some(snapshot), Some(attempt)) =
                (&published.latest_snapshot, &published.latest_attempt)
            && snapshot.matches(self, &history)
            && matches!(
                attempt.status(),
                SessionSaveStatus::Pending | SessionSaveStatus::Healthy
            )
        {
            return Ok(Arc::clone(attempt));
        }
        let attempt = Arc::new(SaveAttempt::default());
        published.latest_attempt = Some(Arc::clone(&attempt));
        published.latest_snapshot = None;
        let (Some(workspace), Some(project_key)) = (&self.workspace, &self.project_key) else {
            attempt.finish(Err(SessionSaveError::Unbound));
            return Err(SessionSaveError::Unbound);
        };
        published.latest_snapshot = Some(PublishedSnapshot {
            session_id: self.id.clone(),
            path: self.path.clone(),
            workspace: workspace.clone(),
            project_key: project_key.clone(),
            history: Arc::clone(&history),
        });
        let result = enqueue_writer(
            sender,
            WriterJob::Save(SaveSnapshot {
                path: self.path.clone(),
                workspace: workspace.clone(),
                project_key: project_key.clone(),
                history,
                attempt: Arc::clone(&attempt),
            }),
        );
        if let Err(error) = result {
            attempt.finish(Err(error.clone()));
            return Err(error);
        }
        Ok(attempt)
    }

    /// Wait for the latest save attempted by this session at entry. A later
    /// save cannot replace this receipt or conceal its failure. Abrupt process
    /// death can still lose a queued snapshot; a timeout does not cancel I/O.
    pub(crate) fn flush_pending(&self) -> Result<(), SessionSaveError> {
        if self.disabled {
            return Ok(());
        }
        let attempt = self
            .save_status
            .lock()
            .map_err(|_| SessionSaveError::WriterUnavailable)?
            .latest_attempt
            .clone();
        attempt.map_or(Ok(()), |attempt| attempt.wait(SESSION_FLUSH_TIMEOUT))
    }

    /// Wait for this exact committed history, even if another cloned handle
    /// admits a newer save concurrently. FIFO admission still orders disk I/O.
    pub(crate) fn checkpoint(&self, history: &[ChatMsg]) -> Result<(), SessionSaveError> {
        if self.disabled || history.is_empty() {
            return self.flush_pending();
        }
        self.checkpoint_to(writer_sender(), history)
    }

    /// Ordinary exit may retry a timed-out checkpoint without queuing the
    /// same write again. Reuse is tied to exact persisted fields and binding;
    /// failed attempts get a fresh ticket, and ordinary checkpoints never reuse.
    pub(crate) fn checkpoint_for_exit(&self, history: &[ChatMsg]) -> Result<(), SessionSaveError> {
        if self.disabled || history.is_empty() {
            return self.flush_pending();
        }
        self.checkpoint_for_exit_to(writer_sender(), history)
    }

    fn checkpoint_for_exit_to(
        &self,
        sender: &mpsc::SyncSender<WriterJob>,
        history: &[ChatMsg],
    ) -> Result<(), SessionSaveError> {
        self.queue_history_with_reuse(sender, Arc::from(history), true)?
            .wait(SESSION_FLUSH_TIMEOUT)
    }

    fn checkpoint_to(
        &self,
        sender: &mpsc::SyncSender<WriterJob>,
        history: &[ChatMsg],
    ) -> Result<(), SessionSaveError> {
        self.queue_history(sender, Arc::from(history))?
            .wait(SESSION_FLUSH_TIMEOUT)
    }

    pub(crate) fn save_status(&self) -> SessionSaveStatus {
        self.save_status
            .lock()
            .map(|publication| {
                publication
                    .latest_attempt
                    .as_ref()
                    .map_or(SessionSaveStatus::Healthy, |attempt| attempt.status())
            })
            .unwrap_or_else(|_| SessionSaveStatus::Failed(SessionSaveError::WriterUnavailable))
    }
}

fn assemble_record(
    workspace: PathBuf,
    project_key: String,
    history: Arc<[ChatMsg]>,
) -> SessionRecord {
    #[cfg(test)]
    assert_eq!(
        std::thread::current().name(),
        Some("session-saver"),
        "SessionRecord must be assembled on the session-saver thread"
    );
    SessionRecord {
        schema: SESSION_SCHEMA.to_string(),
        workspace,
        project_key,
        history: history.to_vec(),
    }
}

fn write_snapshot(
    path: &Path,
    record: &SessionRecord,
    mut phase: impl FnMut(SessionWritePhase),
) -> Result<(), SessionSaveError> {
    phase(SessionWritePhase::CreateDirectory);
    let (directory, name) = crate::workspace_store::private_io::parent(path)
        .map_err(|error| SessionSaveError::CreateDirectory(error.to_string()))?;
    phase(SessionWritePhase::Serialize);
    let mut snapshot = serde_json::to_value(record)
        .map_err(|error| SessionSaveError::Serialize(error.to_string()))?;
    // Scrub decoded history, including nested tool arguments, before the
    // temporary file is opened. Binding fields must remain exact on resume.
    crate::secrets::redact_value(&mut snapshot["history"]);
    let json = serde_json::to_vec(&snapshot)
        .map_err(|error| SessionSaveError::Serialize(error.to_string()))?;
    phase(SessionWritePhase::CheckBinding);
    let previous = directory
        .existing(name)
        .map_err(|_| SessionSaveError::BindingMismatch)?;
    if let Some(previous) = previous.as_ref() {
        let existing_matches =
            serde_json::from_reader::<_, SessionRecord>(previous).is_ok_and(|existing| {
                existing.schema == SESSION_SCHEMA
                    && existing.workspace == record.workspace
                    && existing.project_key == record.project_key
            });
        if !existing_matches {
            return Err(SessionSaveError::BindingMismatch);
        }
    }
    // Keep the per-process temporary name for recovery/fault diagnostics, but
    // never reuse it: an existing entry is foreign staging and stays untouched.
    // The checked exclusive open creates mode 0600 before any payload bytes.
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let tmp_name = tmp
        .file_name()
        .ok_or_else(|| SessionSaveError::Write("session temporary has no file name".into()))?;
    phase(SessionWritePhase::OpenTemporary);
    let mut file = directory
        .create_new(tmp_name)
        .map_err(|error| SessionSaveError::Write(error.to_string()))?;
    let write_result = (|| {
        phase(SessionWritePhase::WriteTemporary);
        file.write_all(&json)?;
        phase(SessionWritePhase::SyncFile);
        file.sync_all()
    })();
    if let Err(error) = write_result {
        // Reclaim only the entry still naming our checked temporary FD.
        let _ = directory.remove_owned(tmp_name, &file);
        return Err(SessionSaveError::Write(error.to_string()));
    }
    phase(SessionWritePhase::Rename);
    if let Err(error) = directory.publish(tmp_name, &file, name, previous.as_ref()) {
        let _ = directory.remove_owned(tmp_name, &file);
        return Err(SessionSaveError::Rename(error.to_string()));
    }
    // Publication remains atomic and the held parent is made durable last.
    phase(SessionWritePhase::SyncDirectory);
    directory
        .sync()
        .map_err(|error| SessionSaveError::Rename(error.to_string()))?;
    Ok(())
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn load_record_path(path: &Path) -> Result<SessionRecord, String> {
    let data =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", path.display()))
}

fn matches_workspace(record: &SessionRecord, workspace: &Path) -> bool {
    record.schema == SESSION_SCHEMA
        && crate::workspace_store::matches_project(
            workspace,
            &record.workspace,
            &record.project_key,
        )
}

/// Load a session only when its serialized repository binding matches.
pub fn load_for(id: &str, workspace: &Path) -> Result<Vec<ChatMsg>, String> {
    let path = sessions_dir().join(format!("{id}.json"));
    let record = load_record_path(&path)?;
    if !matches_workspace(&record, workspace) {
        return Err(format!(
            "session {id} belongs to another project or predates project binding"
        ));
    }
    Ok(repair_interrupted_tool_batch(record.history))
}

const UNKNOWN_TOOL_OUTCOME: &str = "tool error: the previous cockpit exited after recording this call but before its result became durable; outcome unknown — inspect external and workspace state before retrying";

/// Close a crash-interrupted tool-call batch without guessing whether any
/// side effect happened. The pre-dispatch checkpoint deliberately ends with
/// an assistant call message; on resume, synthetic results restore provider
/// protocol validity while making replay an explicit operator/model decision.
fn repair_interrupted_tool_batch(mut history: Vec<ChatMsg>) -> Vec<ChatMsg> {
    let Some(last) = history.last() else {
        return history;
    };
    if last.role != ChatRole::Assistant || last.tool_calls.is_empty() {
        return history;
    }
    let call_ids: Vec<String> = last.tool_calls.iter().map(|call| call.id.clone()).collect();
    history.extend(
        call_ids
            .into_iter()
            .map(|call_id| ChatMsg::tool(call_id, UNKNOWN_TOOL_OUTCOME)),
    );
    history
}

/// Summary of a saved session, for listing.
pub struct SessionInfo {
    pub id: String,
    /// File mtime (ms since epoch) — sort key.
    pub mtime: u64,
    /// Number of user turns.
    pub turns: usize,
    /// First user message (for a human-readable hint).
    pub preview: String,
}

/// Saved sessions for one canonical repository, newest first.
pub fn list_for(workspace: &Path) -> Vec<SessionInfo> {
    list_dir_for(&sessions_dir(), workspace)
}

/// Saved sessions in `dir` that match `workspace` (testable).
pub fn list_dir_for(dir: &Path, workspace: &Path) -> Vec<SessionInfo> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let mtime = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let (turns, preview) = match load_record_path(&path) {
            Ok(record) if matches_workspace(&record, workspace) => {
                let h = record.history;
                let turns = h.iter().filter(|m| m.role == ChatRole::User).count();
                let preview = h
                    .iter()
                    .find(|m| m.role == ChatRole::User)
                    .map(|m| m.content.to_string())
                    .unwrap_or_default();
                (turns, preview)
            }
            _ => continue,
        };
        out.push(SessionInfo {
            id: id.to_string(),
            mtime,
            turns,
            preview,
        });
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.mtime)); // newest first
    out
}

#[cfg(test)]
mod private_io_tests;

#[cfg(test)]
mod exit_checkpoint_tests;

#[cfg(test)]
mod invalid_path_tests;

#[cfg(test)]
mod tests {
    use super::*;

    // WriterJob::Save carries a bound history snapshot; SessionRecord is
    // assembled on the session-saver thread.
    const _: fn(SaveSnapshot) -> WriterJob = WriterJob::Save;

    #[test]
    fn checkpoint_returns_its_own_failure_with_a_newer_save_pending() {
        let root = std::env::temp_dir().join(format!("angel_sess_receipt_{}", gen_id()));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(root.clone(), "receipt".into(), &workspace);
        std::fs::create_dir(session.path()).unwrap();
        let (sender, receiver) = mpsc::sync_channel(2);
        let checkpoint_session = session.clone();
        let checkpoint_sender = sender.clone();
        let checkpoint = std::thread::spawn(move || {
            checkpoint_session.checkpoint_to(&checkpoint_sender, &[ChatMsg::user("checkpoint A")])
        });
        let WriterJob::Save(first) = receiver.recv_timeout(Duration::from_secs(1)).unwrap() else {
            panic!("expected first save");
        };
        let second = session
            .queue_history(&sender, Arc::from(vec![ChatMsg::user("save B")]))
            .unwrap();
        assert_eq!(session.save_status(), SessionSaveStatus::Pending);
        std::thread::Builder::new()
            .name("session-saver".into())
            .spawn(move || write_queued_snapshot(first))
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(
            checkpoint.join().unwrap(),
            Err(SessionSaveError::BindingMismatch)
        );
        assert!(matches!(
            second.wait(Duration::ZERO),
            Err(SessionSaveError::WriterTimeout(_))
        ));
        assert_eq!(session.save_status(), SessionSaveStatus::Pending);
        std::fs::remove_dir(session.path()).unwrap();
        let WriterJob::Save(next) = receiver.recv_timeout(Duration::from_secs(1)).unwrap() else {
            panic!("expected second save");
        };
        std::thread::Builder::new()
            .name("session-saver".into())
            .spawn(move || write_queued_snapshot(next))
            .unwrap()
            .join()
            .unwrap();
        second.wait(Duration::ZERO).unwrap();
        session.flush_pending().unwrap();
        assert_eq!(session.save_status(), SessionSaveStatus::Healthy);
        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(&*retained[0].content, "save B");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn older_write_cannot_clear_a_newer_backlog_failure() {
        let root = std::env::temp_dir().join(format!("angel_sess_backlog_{}", gen_id()));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(root.clone(), "ordered-status".into(), &workspace);
        let (sender, receiver) = mpsc::sync_channel(1);
        let save = |text| {
            session
                .queue_history(&sender, Arc::from(vec![ChatMsg::user(text)]))
                .map(|_| ())
        };
        let drain_one = || {
            let WriterJob::Save(snapshot) = receiver.recv().unwrap() else {
                panic!("expected an actual queued snapshot");
            };
            std::thread::Builder::new()
                .name("session-saver".into())
                .spawn(move || write_queued_snapshot(snapshot))
                .unwrap()
                .join()
                .unwrap();
        };
        save("older accepted history").unwrap();
        assert_eq!(
            save("newer lost history"),
            Err(SessionSaveError::WriterBacklog)
        );
        drain_one();
        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(&*retained[0].content, "older accepted history");
        assert_eq!(
            session.save_status(),
            SessionSaveStatus::Failed(SessionSaveError::WriterBacklog)
        );
        save("successful retry of newest history").unwrap();
        drain_one();
        assert_eq!(session.save_status(), SessionSaveStatus::Healthy);
        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(&*retained[0].content, "successful retry of newest history");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn writer_queue_saturation_fails_fast_and_typed() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let (first_reply, _first_done) = mpsc::channel();
        assert_eq!(
            enqueue_writer(&sender, WriterJob::Barrier(first_reply)),
            Ok(())
        );

        let (overflow_reply, _overflow_done) = mpsc::channel();
        assert_eq!(
            enqueue_writer(&sender, WriterJob::Barrier(overflow_reply)),
            Err(SessionSaveError::WriterBacklog),
            "a full writer queue must not block or retain another job"
        );

        drop(receiver);
        let (disconnected_reply, _disconnected_done) = mpsc::channel();
        assert_eq!(
            enqueue_writer(&sender, WriterJob::Barrier(disconnected_reply)),
            Err(SessionSaveError::WriterUnavailable)
        );
    }

    #[test]
    fn session_roundtrip_and_list() {
        let dir = std::env::temp_dir().join(format!("angel_sess_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let s = Session::at_for(dir.clone(), "0000000000001-1".to_string(), &workspace);
        let hist = vec![
            ChatMsg::system("orchestrator prompt"),
            ChatMsg::user("hello there"),
            ChatMsg::assistant("hi back"),
        ];
        s.save(&hist).unwrap();

        let loaded = load_for_path_for_test(s.path(), &workspace).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(&*loaded[1].content, "hello there");
        assert_eq!(loaded[2].role, ChatRole::Assistant);

        let infos = list_dir_for(&dir, &workspace);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, "0000000000001-1");
        assert_eq!(infos[0].turns, 1);
        assert!(
            infos[0].preview.contains("hello there"),
            "got: {}",
            infos[0].preview
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saved_history_redacts_secrets_and_resumes_with_typed_tool_arguments() {
        let _lock = crate::tests::env_lock();
        let secret = "fixture-escaped-\"secret\"\\01234567";
        let _key = crate::tests::TestEnvGuard::set("ANGEL_T_SESSION_SECRET", secret);
        let dir = std::env::temp_dir().join(format!("angel_sess_redact_{}", std::process::id()));
        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(dir.clone(), "redacted".to_string(), &workspace);
        let mut message = ChatMsg::assistant("hf_abcdefghijklmnopqrstuvwxyz01234567");
        message.tool_calls = vec![crate::club::ToolCall {
            id: "call-1".to_string(),
            name: "fixture".to_string(),
            args: serde_json::json!({"access_token": "opaque-value", "count": 17, "ok": true, "nested": [secret, null]}),
        }].into();
        let history = vec![ChatMsg::user(secret), message];
        session.save(&history).unwrap();
        let loaded = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(&*loaded[0].content, "«redacted:ANGEL_T_SESSION_SECRET»");
        assert_eq!(&*loaded[1].content, crate::secrets::REDACTED);
        let args = &loaded[1].tool_calls[0].args;
        assert_eq!(args["access_token"], crate::secrets::REDACTED);
        assert_eq!(args["count"], 17);
        assert_eq!(args["ok"], true);
        assert_eq!(args["nested"][1], serde_json::Value::Null);
        assert_eq!(
            &*history[0].content, secret,
            "redaction does not edit live history"
        );
        session.save(&history).unwrap();
        assert!(load_for_path_for_test(session.path(), &workspace).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn session_binding_rejects_cross_project_and_legacy_records() {
        let dir = std::env::temp_dir().join(format!("angel_sess_scope_{}", std::process::id()));
        let alpha = dir.join("alpha");
        let beta = dir.join("beta");
        std::fs::create_dir_all(&alpha).unwrap();
        std::fs::create_dir_all(&beta).unwrap();

        let alpha_session = Session::at_for(dir.clone(), "alpha".to_string(), &alpha);
        alpha_session
            .save(&[ChatMsg::system("ALPHA_ONLY"), ChatMsg::user("alpha work")])
            .unwrap();
        assert_eq!(list_dir_for(&dir, &alpha).len(), 1);
        assert!(list_dir_for(&dir, &beta).is_empty());

        // Even an explicit/colliding sink id cannot overwrite a snapshot that
        // is already bound to another project.
        let colliding_beta = Session::at_for(dir.clone(), "alpha".to_string(), &beta);
        let mismatch = colliding_beta.save(&[
            ChatMsg::system("BETA_OVERWRITE"),
            ChatMsg::user("beta work"),
        ]);
        assert_eq!(mismatch, Err(SessionSaveError::BindingMismatch));
        let retained = load_for_path_for_test(alpha_session.path(), &alpha).unwrap();
        assert!(
            retained
                .iter()
                .any(|message| message.content.as_ref() == "ALPHA_ONLY")
        );
        assert!(
            !retained
                .iter()
                .any(|message| message.content.as_ref() == "BETA_OVERWRITE")
        );

        std::fs::write(
            dir.join("legacy.json"),
            serde_json::to_vec(&vec![ChatMsg::system("LEGACY_GLOBAL")]).unwrap(),
        )
        .unwrap();
        assert_eq!(
            list_dir_for(&dir, &alpha).len(),
            1,
            "legacy global sessions must be inert"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn load_for_path_for_test(path: &Path, workspace: &Path) -> Result<Vec<ChatMsg>, String> {
        let record = load_record_path(path)?;
        matches_workspace(&record, workspace)
            .then_some(record.history)
            .ok_or_else(|| "project mismatch".to_string())
    }

    #[test]
    fn empty_thread_writes_nothing() {
        let dir = std::env::temp_dir().join(format!("angel_sess_empty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = Session::at(dir.clone(), "x".to_string());
        s.save(&[]).unwrap();
        assert!(!s.path().exists(), "empty thread must not create a file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn disabled_session_writes_nothing() {
        let s = Session::disabled();
        s.save(&[ChatMsg::system("preview only")]).unwrap();
        assert!(
            !s.path().exists(),
            "disabled session must not create a file"
        );
    }

    #[test]
    fn nonempty_snapshot_requires_a_project_binding() {
        let dir = std::env::temp_dir().join(format!("angel_sess_unbound_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let session = Session::at(dir, "unbound".to_string());
        let result = session.save(&[ChatMsg::user("retain in memory")]);
        assert_eq!(result, Err(SessionSaveError::Unbound));
        assert_eq!(
            session.save_status(),
            SessionSaveStatus::Failed(SessionSaveError::Unbound)
        );
        assert!(!session.path().exists());
    }

    #[test]
    fn failed_rewrite_retains_the_previous_good_snapshot() {
        let dir = std::env::temp_dir().join(format!("angel_sess_io_{}", std::process::id()));
        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(dir.clone(), "stable".to_string(), &workspace);
        session.save(&[ChatMsg::user("last good")]).unwrap();

        let tmp = session
            .path()
            .with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::create_dir(&tmp).unwrap();
        let result = session.save(&[ChatMsg::user("must not replace")]);
        assert!(matches!(result, Err(SessionSaveError::Write(_))));
        assert!(matches!(
            session.save_status(),
            SessionSaveStatus::Failed(SessionSaveError::Write(_))
        ));
        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(&*retained[0].content, "last good");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn file_size_failure_reports_failed_retains_snapshot_and_cleans_temp() {
        const CHILD_ROOT: &str = "ANGEL_T_SESSION_EFBIG_ROOT";
        const FIXTURE_MARKER: &str = "owned-session-efbig-probe/v1";
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            let root = PathBuf::from(root);
            assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
            assert!(
                root.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("angel_sess_efbig_")
            );
            assert_eq!(
                std::fs::read_to_string(root.join("fixture-marker")).unwrap(),
                FIXTURE_MARKER
            );
            let workspace = root.join("workspace");
            let session = Session::at_for(root.join("sessions"), "probe".into(), &workspace);
            let original = std::fs::read(session.path()).unwrap();
            // This is kernel EFBIG injection, not a claim of a full filesystem.
            // Retain the hard ceiling and restore the original soft limit even
            // on panic, so coverage/profile output can complete during exit.
            struct RestoreLimit {
                limit: libc::rlimit,
                signal: libc::sighandler_t,
            }
            impl Drop for RestoreLimit {
                fn drop(&mut self) {
                    unsafe {
                        let _ = libc::setrlimit(libc::RLIMIT_FSIZE, &self.limit);
                        libc::signal(libc::SIGXFSZ, self.signal);
                    }
                }
            }
            let _restore = unsafe {
                let mut limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                assert_eq!(libc::getrlimit(libc::RLIMIT_FSIZE, &mut limit), 0);
                let signal = libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
                assert_ne!(signal, libc::SIG_ERR);
                RestoreLimit { limit, signal }
            };
            let limited = libc::rlimit {
                rlim_cur: 65_536,
                rlim_max: _restore.limit.rlim_max,
            };
            assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limited) }, 0);
            let error = session
                .checkpoint(&[ChatMsg::user("new snapshot ".repeat(30_000))])
                .unwrap_err();
            let expected = std::io::Error::from_raw_os_error(libc::EFBIG).to_string();
            assert!(
                matches!(&error, SessionSaveError::Write(detail) if detail == &expected),
                "{error}"
            );
            assert_eq!(
                session.save_status(),
                SessionSaveStatus::Failed(error.clone())
            );
            assert_eq!(session.flush_pending(), Err(error));
            assert_eq!(
                std::fs::read(session.path()).unwrap(),
                original,
                "failed snapshot must not replace committed bytes"
            );
            let temporary = session
                .path()
                .with_extension(format!("json.{}.tmp", std::process::id()));
            assert!(
                !temporary.exists(),
                "EFBIG left a partial temporary snapshot: {}",
                temporary.display()
            );
            return;
        }

        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        struct ReapChild(std::process::Child);
        impl Drop for ReapChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!("angel_sess_efbig_{}", gen_id())));
        let workspace = fixture.0.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(fixture.0.join("fixture-marker"), FIXTURE_MARKER).unwrap();
        let session = Session::at_for(fixture.0.join("sessions"), "probe".into(), &workspace);
        session
            .save(&[ChatMsg::user("last complete checkpoint")])
            .unwrap();
        let original = std::fs::read(session.path()).unwrap();
        let log = std::fs::File::create(fixture.0.join("child.log")).unwrap();
        let mut child = ReapChild(std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "session::tests::file_size_failure_reports_failed_retains_snapshot_and_cleans_temp", "--test-threads=1", "--nocapture"])
            .env(CHILD_ROOT, &fixture.0)
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn().unwrap());
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "owned EFBIG probe timed out"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(
            status.success(),
            "owned EFBIG probe failed: {}",
            std::fs::read_to_string(fixture.0.join("child.log")).unwrap()
        );
        assert_eq!(std::fs::read(session.path()).unwrap(), original);
        assert!(
            std::fs::read_dir(session.path().parent().unwrap())
                .unwrap()
                .flatten()
                .all(|entry| entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "tmp"))
        );
    }

    #[test]
    fn background_writer_publishes_io_failure() {
        let root = std::env::temp_dir().join(format!("angel_sess_bg_io_{}", std::process::id()));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let blocked = root.join("not-a-directory");
        std::fs::write(&blocked, "file").unwrap();
        let session = Session::at_for(blocked, "sink".to_string(), &workspace);
        session
            .save_async(&[ChatMsg::user("conversation remains live")])
            .unwrap();
        let result = session.flush_pending();
        assert!(matches!(result, Err(SessionSaveError::CreateDirectory(_))));
        assert!(matches!(
            session.save_status(),
            SessionSaveStatus::Failed(SessionSaveError::CreateDirectory(_))
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn save_async_enqueues_a_bound_snapshot_not_a_session_record() {
        let root = std::env::temp_dir().join(format!(
            "angel_sess_off_ui_{}_{}",
            std::process::id(),
            gen_id()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(root.clone(), "off-ui".to_string(), &workspace);
        let history = vec![
            ChatMsg::user("queued without a caller-built record"),
            ChatMsg::assistant("writer assembled the document"),
        ];
        session.save_async(&history).unwrap();
        session.flush_pending().unwrap();

        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(retained.len(), 2);
        assert_eq!(
            &*retained[0].content,
            "queued without a caller-built record"
        );
        assert_eq!(&*retained[1].content, "writer assembled the document");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn save_history_roundtrips_a_shared_arc_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "angel_sess_shared_arc_{}_{}",
            std::process::id(),
            gen_id()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(root.clone(), "shared-arc".to_string(), &workspace);
        let history: Arc<[ChatMsg]> = Arc::from(vec![
            ChatMsg::user("shared with spawn"),
            ChatMsg::assistant("same snapshot"),
        ]);
        session.save_history(Arc::clone(&history)).unwrap();
        session.flush_pending().unwrap();

        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(retained.len(), 2);
        assert_eq!(&*retained[0].content, &*history[0].content);
        assert_eq!(&*retained[1].content, &*history[1].content);
        assert_eq!(retained[0].role, ChatRole::User);
        assert_eq!(retained[1].role, ChatRole::Assistant);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn async_flush_barrier_makes_the_latest_snapshot_immediately_loadable() {
        let root = std::env::temp_dir().join(format!(
            "angel_sess_flush_{}_{}",
            std::process::id(),
            gen_id()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(root.clone(), "ordered".to_string(), &workspace);
        session
            .save_async(&[
                ChatMsg::user("latest queued user turn"),
                ChatMsg::assistant("latest queued assistant turn"),
            ])
            .unwrap();

        session.flush_pending().unwrap();

        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(retained.len(), 2);
        assert_eq!(&*retained[0].content, "latest queued user turn");
        assert_eq!(&*retained[1].content, "latest queued assistant turn");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn checkpoint_lands_after_an_older_queued_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "angel_sess_checkpoint_{}_{}",
            std::process::id(),
            gen_id()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(root.clone(), "checkpoint".to_string(), &workspace);
        session
            .save_async(&[ChatMsg::user("older queued snapshot")])
            .unwrap();

        session
            .checkpoint(&[
                ChatMsg::user("explicit checkpoint"),
                ChatMsg::assistant("confirmed durable"),
            ])
            .unwrap();

        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(retained.len(), 2);
        assert_eq!(&*retained[0].content, "explicit checkpoint");
        assert_eq!(&*retained[1].content, "confirmed durable");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn blocking_save_lands_after_an_older_queued_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "angel_sess_ordered_save_{}_{}",
            std::process::id(),
            gen_id()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let session = Session::at_for(root.clone(), "ordered-save".to_string(), &workspace);
        session
            .save_async(&[ChatMsg::user("older queued snapshot")])
            .unwrap();

        session
            .save(&[
                ChatMsg::user("newer boundary snapshot"),
                ChatMsg::assistant("must remain newest"),
            ])
            .unwrap();

        let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
        assert_eq!(retained.len(), 2);
        assert_eq!(&*retained[0].content, "newer boundary snapshot");
        assert_eq!(&*retained[1].content, "must remain newest");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resume_repairs_a_trailing_tool_intent_as_outcome_unknown() {
        let calls = vec![crate::club::ToolCall {
            id: "effect-1".to_string(),
            name: "shell".to_string(),
            args: serde_json::json!({"cmd": "external-side-effect"}),
        }];
        let repaired = repair_interrupted_tool_batch(vec![
            ChatMsg::user("perform it"),
            ChatMsg::assistant_calls(calls),
        ]);

        assert_eq!(repaired.len(), 3);
        assert_eq!(repaired[2].role, ChatRole::Tool);
        assert_eq!(repaired[2].tool_call_id.as_deref(), Some("effect-1"));
        assert!(repaired[2].content.contains("outcome unknown"));
        assert!(repaired[2].content.contains("before retrying"));

        let unchanged = vec![ChatMsg::user("hello"), ChatMsg::assistant("done")];
        let retained = repair_interrupted_tool_batch(unchanged);
        assert_eq!(retained.len(), 2);
        assert_eq!(&*retained[0].content, "hello");
        assert_eq!(&*retained[1].content, "done");
    }
}

#[cfg(test)]
mod writer_diagnostic_tests;

#[cfg(all(test, unix))]
mod process_fault_tests;
