use super::schema::*;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_JOURNAL_EVENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_BLOB_BYTES: usize = 16 * 1024 * 1024;
static ROLLOUT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum JournalEventKind {
    RolloutStarted {
        project: ProjectIdentity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task_binding: Option<TaskRolloutBindingV1>,
        capture: CaptureDescriptor,
        requested_route: RecordedRoute,
        started_ms: u64,
    },
    PolicyRequestCaptured {
        attempt: PolicyAttemptV1,
    },
    PolicyResponseCaptured {
        step_index: u32,
        attempt_index: u16,
        response: PolicyResponseRef,
        resolved_route: RecordedRoute,
        outcome: PolicyStepOutcome,
    },
    PolicyAttemptFailed {
        step_index: u32,
        attempt_index: u16,
        outcome: PolicyStepOutcome,
    },
    RewardAttached {
        receipt: RewardReceipt,
    },
    RewardAttachmentRejected {
        attempted: RewardReceipt,
    },
    CompatibilityCaptured {
        compatibility: RolloutCompatibilityV1,
    },
    AuxiliaryCoverageCaptured {
        coverage: super::super::auxiliary::AuxiliaryCoverage,
    },
    TurnStopped {
        termination: Termination,
        eligibility: TrainingEligibility,
        status: RolloutStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JournalEvent {
    pub(crate) schema: String,
    pub(crate) rollout_id: String,
    pub(crate) seq: u64,
    pub(crate) previous_sha256: String,
    pub(crate) kind: JournalEventKind,
    pub(crate) event_sha256: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct JournalCursor {
    pub(crate) next_seq: u64,
    pub(crate) head_sha256: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RolloutStore {
    root: PathBuf,
    configured_base: Option<PathBuf>,
}

pub(crate) struct RolloutLease {
    file: File,
}

impl Drop for RolloutLease {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: the lease owns this descriptor until after unlock.
            unsafe {
                libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}

impl RolloutStore {
    pub(super) fn canonical_root(&self) -> Result<PathBuf, String> {
        self.root
            .canonicalize()
            .map_err(|e| format!("resolve rollout store: {e}"))
    }
    pub(crate) fn for_workspace(workspace: &Path) -> Self {
        let repo_key = crate::workspace_store::repo_identity(workspace).key;
        let base = std::env::var_os("ANGEL_HARNESS_ROLLOUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::workspace_store::angel_subdir("harness-rollouts"));
        Self {
            root: base.join(repo_key),
            configured_base: Some(base),
        }
    }

    #[cfg(test)]
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            configured_base: None,
        }
    }

    // The production constructor records the explicit private base, so it can
    // migrate that directory without chmodding an arbitrary caller's parent.
    // File creation, journal digests and sealed-manifest publication stay with
    // their existing authority checks below.
    fn prepare_directory(&self, leaf: &Path) -> Result<(), String> {
        use crate::workspace_store::private_io::PrivateDirectory;
        for path in self
            .configured_base
            .iter()
            .map(PathBuf::as_path)
            .chain([self.root.as_path(), leaf])
        {
            PrivateDirectory::open(path)
                .and_then(|directory| directory.secure_owner_only())
                .map_err(|error| format!("prepare {}: {error}", path.display()))?;
        }
        Ok(())
    }

    pub(crate) fn project_identity(workspace: &Path) -> ProjectIdentity {
        let repo = crate::workspace_store::repo_identity(workspace);
        ProjectIdentity {
            workspace_key: crate::workspace_store::workspace_key(workspace),
            repo_key: repo.key,
            canonical_root_sha256: crate::cut::sha256_hex(repo.root.to_string_lossy().as_bytes()),
        }
    }

    pub(crate) fn new_rollout_id(&self) -> String {
        let seq = ROLLOUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        format!("rol-{:x}-{:x}-{seq:x}", now_ms(), std::process::id())
    }

    /// Discover project-local rollout IDs without opening any manifest. The
    /// stable sort is part of the offline corpus export contract.
    pub(crate) fn discover_rollout_ids(&self) -> Result<Vec<String>, String> {
        let runs = self.root.join("runs");
        reject_symlink(&self.root)?;
        reject_symlink(&runs)?;
        if !runs.exists() {
            return Ok(Vec::new());
        }
        let metadata =
            fs::metadata(&runs).map_err(|error| format!("stat {}: {error}", runs.display()))?;
        if !metadata.is_dir() {
            return Err(format!(
                "rollout runs path is not a directory: {}",
                runs.display()
            ));
        }
        let mut rollout_ids = Vec::new();
        for entry in
            fs::read_dir(&runs).map_err(|error| format!("read {}: {error}", runs.display()))?
        {
            let entry = entry.map_err(|error| format!("read {} entry: {error}", runs.display()))?;
            let file_type = entry
                .file_type()
                .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
            if file_type.is_symlink() {
                return Err(format!(
                    "refusing symlinked rollout run {}",
                    entry.path().display()
                ));
            }
            if !file_type.is_dir() {
                continue;
            }
            let rollout_id = entry
                .file_name()
                .into_string()
                .map_err(|_| "rollout id is not valid UTF-8".to_string())?;
            validate_rollout_id(&rollout_id)?;
            rollout_ids.push(rollout_id);
        }
        rollout_ids.sort();
        Ok(rollout_ids)
    }

    pub(crate) fn run_dir(&self, rollout_id: &str) -> Result<PathBuf, String> {
        validate_rollout_id(rollout_id)?;
        Ok(self.root.join("runs").join(rollout_id))
    }

    pub(crate) fn acquire_lease(&self, rollout_id: &str) -> Result<RolloutLease, String> {
        let run_dir = self.run_dir(rollout_id)?;
        reject_symlink(&self.root)?;
        reject_symlink(&self.root.join("runs"))?;
        reject_symlink(&run_dir)?;
        self.prepare_directory(&run_dir)?;
        secure_tree(&self.root, &run_dir)?;
        let path = run_dir.join("run.lock");
        reject_symlink(&path)?;
        let file = open_private(&path, OpenKind::ReadWrite)?;
        secure_file(&path)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: `file` remains owned by the returned lease.
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                return Err(format!(
                    "lock {}: {}",
                    path.display(),
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(RolloutLease { file })
    }

    pub(crate) fn put_blob(&self, bytes: &[u8]) -> Result<String, String> {
        if bytes.len() > MAX_BLOB_BYTES {
            return Err(format!("rollout blob exceeds {MAX_BLOB_BYTES} bytes"));
        }
        reject_sensitive_bytes(bytes)?;
        let sha256 = crate::cut::sha256_hex(bytes);
        let dir = self.root.join("blobs");
        reject_symlink(&self.root)?;
        reject_symlink(&dir)?;
        self.prepare_directory(&dir)?;
        secure_dir(&self.root)?;
        secure_dir(&dir)?;
        let path = dir.join(&sha256);
        if path.exists() {
            reject_symlink(&path)?;
            let existing = read_bounded(&path, MAX_BLOB_BYTES as u64)?;
            if existing != bytes {
                return Err("existing rollout blob does not match its digest".to_string());
            }
            check_private_file(&path)?;
            return Ok(sha256);
        }
        let sequence = ROLLOUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(".{sha256}.{}.{}.tmp", std::process::id(), sequence));
        reject_symlink(&tmp)?;
        let mut file = open_private(&tmp, OpenKind::CreateNew)?;
        secure_file(&tmp)?;
        file.write_all(bytes)
            .map_err(|error| format!("write {}: {error}", tmp.display()))?;
        file.sync_data()
            .map_err(|error| format!("sync {}: {error}", tmp.display()))?;
        fs::rename(&tmp, &path).map_err(|error| format!("install {}: {error}", path.display()))?;
        secure_file(&path)?;
        sync_dir(&dir)?;
        Ok(sha256)
    }

    pub(crate) fn read_blob(&self, reference: &BlobRef) -> Result<Vec<u8>, String> {
        if reference.storage != BlobStorage::Local {
            return Err("rollout blob body was not captured locally".to_string());
        }
        if reference.sha256.len() != 64
            || !reference
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid rollout blob digest".to_string());
        }
        if reference.bytes > MAX_BLOB_BYTES as u64 {
            return Err("rollout blob reference exceeds configured bound".to_string());
        }
        let blob_dir = self.root.join("blobs");
        reject_symlink(&self.root)?;
        reject_symlink(&blob_dir)?;
        let path = blob_dir.join(&reference.sha256);
        reject_symlink(&path)?;
        check_private_file(&path)?;
        let bytes = read_bounded(&path, MAX_BLOB_BYTES as u64)?;
        if bytes.len() as u64 != reference.bytes
            || crate::cut::sha256_hex(&bytes) != reference.sha256
        {
            return Err("rollout blob digest/length mismatch".to_string());
        }
        Ok(bytes)
    }

    pub(crate) fn read_blob_by_digest(&self, sha256: &str) -> Result<Vec<u8>, String> {
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("invalid rollout evidence digest".to_string());
        }
        let blob_dir = self.root.join("blobs");
        reject_symlink(&self.root)?;
        reject_symlink(&blob_dir)?;
        let path = blob_dir.join(sha256);
        reject_symlink(&path)?;
        check_private_file(&path)?;
        let bytes = read_bounded(&path, MAX_BLOB_BYTES as u64)?;
        if crate::cut::sha256_hex(&bytes) != sha256 {
            return Err("rollout evidence blob digest mismatch".to_string());
        }
        Ok(bytes)
    }

    pub(crate) fn append_event(
        &self,
        rollout_id: &str,
        cursor: &mut JournalCursor,
        kind: JournalEventKind,
    ) -> Result<JournalEvent, String> {
        // Fail before building the event hash or opening the journal. Tool
        // identifiers and other metadata are untrusted text too.
        let material =
            serde_json::to_vec(&kind).map_err(|error| format!("encode event metadata: {error}"))?;
        reject_sensitive_bytes(&material)?;
        let run_dir = self.run_dir(rollout_id)?;
        reject_symlink(&self.root)?;
        reject_symlink(&self.root.join("runs"))?;
        reject_symlink(&run_dir)?;
        self.prepare_directory(&run_dir)?;
        secure_tree(&self.root, &run_dir)?;
        let previous_sha256 = cursor.head_sha256.clone();
        let event_sha256 = event_digest(rollout_id, cursor.next_seq, &previous_sha256, &kind)?;
        let event = JournalEvent {
            schema: JOURNAL_SCHEMA.to_string(),
            rollout_id: rollout_id.to_string(),
            seq: cursor.next_seq,
            previous_sha256,
            kind,
            event_sha256,
        };
        let mut line =
            serde_json::to_vec(&event).map_err(|error| format!("encode journal event: {error}"))?;
        line.push(b'\n');
        if line.len() > MAX_JOURNAL_EVENT_BYTES {
            return Err(format!(
                "journal event exceeds {MAX_JOURNAL_EVENT_BYTES} bytes"
            ));
        }
        let path = run_dir.join("journal.jsonl");
        reject_symlink(&path)?;
        let current_len = match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => metadata.len(),
            Ok(_) => {
                return Err(format!(
                    "rollout journal is not a regular file: {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(format!("stat {}: {error}", path.display())),
        };
        if current_len.saturating_add(line.len() as u64) > MAX_JOURNAL_BYTES {
            return Err(format!(
                "rollout journal exceeds {MAX_JOURNAL_BYTES} byte bound"
            ));
        }
        let mut file = open_private(&path, OpenKind::Append)?;
        file.write_all(&line)
            .map_err(|error| format!("append {}: {error}", path.display()))?;
        file.sync_data()
            .map_err(|error| format!("sync {}: {error}", path.display()))?;
        cursor.next_seq = cursor.next_seq.saturating_add(1);
        cursor.head_sha256 = event.event_sha256.clone();
        Ok(event)
    }

    pub(crate) fn audit_journal(&self, rollout_id: &str) -> Result<Vec<JournalEvent>, String> {
        let path = self.run_dir(rollout_id)?.join("journal.jsonl");
        reject_symlink(&path)?;
        check_private_file(&path)?;
        let raw = read_bounded(&path, MAX_JOURNAL_BYTES)?;
        let complete_len = raw
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let mut events = Vec::new();
        let mut previous_sha256 = String::new();
        for (seq, line) in raw[..complete_len]
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .enumerate()
        {
            if line.len() > MAX_JOURNAL_EVENT_BYTES {
                return Err("journal event exceeds configured bound".to_string());
            }
            let event: JournalEvent = serde_json::from_slice(line)
                .map_err(|error| format!("parse journal event {seq}: {error}"))?;
            if event.schema != JOURNAL_SCHEMA
                || event.rollout_id != rollout_id
                || event.seq != seq as u64
                || event.previous_sha256 != previous_sha256
            {
                return Err(format!("journal identity/order mismatch at event {seq}"));
            }
            let expected =
                event_digest(rollout_id, event.seq, &event.previous_sha256, &event.kind)?;
            if expected != event.event_sha256 {
                return Err(format!("journal hash mismatch at event {seq}"));
            }
            previous_sha256 = event.event_sha256.clone();
            events.push(event);
        }
        if events.is_empty() {
            return Err("journal has no complete events".to_string());
        }
        Ok(events)
    }

    /// Discard only an unterminated final fragment after the complete prefix has
    /// already passed [`Self::audit_journal`]. Recovery must do this before it
    /// appends a process-loss terminal event, otherwise the new JSON object
    /// would be concatenated onto the torn bytes.
    pub(crate) fn discard_torn_tail(&self, rollout_id: &str) -> Result<(), String> {
        let path = self.run_dir(rollout_id)?.join("journal.jsonl");
        reject_symlink(&path)?;
        check_private_file(&path)?;
        let raw = read_bounded(&path, MAX_JOURNAL_BYTES)?;
        let complete_len = raw
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        if complete_len == raw.len() {
            return Ok(());
        }
        let file = open_private(&path, OpenKind::WriteExisting)?;
        file.set_len(complete_len as u64)
            .map_err(|error| format!("truncate {}: {error}", path.display()))?;
        file.sync_data()
            .map_err(|error| format!("sync {}: {error}", path.display()))
    }

    pub(crate) fn save_manifest(&self, manifest: &mut HarnessRolloutV1) -> Result<PathBuf, String> {
        validate_rollout_id(&manifest.rollout_id)?;
        if manifest.schema != ROLLOUT_SCHEMA {
            return Err("refusing unknown rollout schema".to_string());
        }
        validate_attempt_order(&manifest.attempts)?;
        manifest.manifest_sha256 = None;
        let digest_body =
            serde_json::to_vec(manifest).map_err(|error| format!("encode manifest: {error}"))?;
        reject_sensitive_bytes(&digest_body)?;
        manifest.manifest_sha256 = Some(crate::cut::sha256_hex(&digest_body));
        let body = serde_json::to_vec_pretty(manifest)
            .map_err(|error| format!("encode manifest: {error}"))?;
        if body.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(format!(
                "rollout manifest exceeds {MAX_MANIFEST_BYTES} byte bound"
            ));
        }
        let run_dir = self.run_dir(&manifest.rollout_id)?;
        self.prepare_directory(&run_dir)?;
        secure_tree(&self.root, &run_dir)?;
        let path = run_dir.join("manifest.json");
        if path.exists() {
            return Err("sealed rollout manifests are immutable".to_string());
        }
        reject_symlink(&path)?;
        let sequence = ROLLOUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let tmp = run_dir.join(format!(".manifest.{}.{}.tmp", std::process::id(), sequence));
        reject_symlink(&tmp)?;
        let mut file = open_private(&tmp, OpenKind::CreateNew)?;
        file.write_all(&body)
            .map_err(|error| format!("write {}: {error}", tmp.display()))?;
        file.sync_data()
            .map_err(|error| format!("sync {}: {error}", tmp.display()))?;
        fs::rename(&tmp, &path).map_err(|error| {
            let _ = fs::remove_file(&tmp);
            format!("replace {}: {error}", path.display())
        })?;
        secure_file(&path)?;
        sync_dir(&run_dir)?;
        Ok(path)
    }

    #[allow(dead_code)] // used by explicit recovery/audit and its tests
    pub(crate) fn load_manifest(
        &self,
        rollout_id: &str,
        expected_repo_key: &str,
    ) -> Result<HarnessRolloutV1, String> {
        let path = self.run_dir(rollout_id)?.join("manifest.json");
        reject_symlink(&path)?;
        check_private_file(&path)?;
        let raw = read_bounded(&path, MAX_MANIFEST_BYTES)?;
        let mut manifest: HarnessRolloutV1 = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse {}: {error}", path.display()))?;
        if manifest.schema != ROLLOUT_SCHEMA
            || manifest.rollout_id != rollout_id
            || manifest.project.repo_key != expected_repo_key
        {
            return Err("rollout manifest identity/schema mismatch".to_string());
        }
        validate_attempt_order(&manifest.attempts)?;
        let claimed = manifest
            .manifest_sha256
            .take()
            .ok_or_else(|| "rollout manifest is unsealed".to_string())?;
        let digest_body =
            serde_json::to_vec(&manifest).map_err(|error| format!("encode manifest: {error}"))?;
        if crate::cut::sha256_hex(&digest_body) != claimed {
            return Err("rollout manifest digest mismatch".to_string());
        }
        manifest.manifest_sha256 = Some(claimed);
        let events = self.audit_journal(rollout_id)?;
        let head = events
            .last()
            .map(|event| event.event_sha256.as_str())
            .unwrap_or_default();
        if manifest.journal_head_sha256 != head {
            return Err("rollout manifest journal head mismatch".to_string());
        }
        Ok(manifest)
    }

    #[cfg(test)]
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

fn event_digest(
    rollout_id: &str,
    seq: u64,
    previous_sha256: &str,
    kind: &JournalEventKind,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct EventMaterial<'a> {
        schema: &'static str,
        rollout_id: &'a str,
        seq: u64,
        previous_sha256: &'a str,
        kind: &'a JournalEventKind,
    }
    let body = serde_json::to_vec(&EventMaterial {
        schema: JOURNAL_SCHEMA,
        rollout_id,
        seq,
        previous_sha256,
        kind,
    })
    .map_err(|error| format!("encode event digest: {error}"))?;
    Ok(crate::cut::sha256_hex(&body))
}

pub(crate) fn validate_rollout_id(rollout_id: &str) -> Result<(), String> {
    let valid = rollout_id
        .strip_prefix("rol-")
        .filter(|_| rollout_id.len() <= 96)
        .map(|suffix| {
            let parts = suffix.split('-').collect::<Vec<_>>();
            parts.len() == 3
                && parts.iter().all(|part| {
                    !part.is_empty()
                        && part.len() <= 16
                        && part.chars().all(|character| character.is_ascii_hexdigit())
                })
        })
        .unwrap_or(false);
    valid
        .then_some(())
        .ok_or_else(|| "invalid rollout id".to_string())
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(unix)]
fn secure_tree(root: &Path, leaf: &Path) -> Result<(), String> {
    let runs = root.join("runs");
    for path in [root, runs.as_path(), leaf] {
        if path.exists() {
            secure_dir(path)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn secure_tree(_root: &Path, _leaf: &Path) -> Result<(), String> {
    Ok(())
}

#[derive(Clone, Copy)]
enum OpenKind {
    ReadExisting,
    ReadWrite,
    WriteExisting,
    Append,
    CreateNew,
}

fn open_private(path: &Path, kind: OpenKind) -> Result<File, String> {
    let mut options = OpenOptions::new();
    match kind {
        OpenKind::ReadExisting => {
            options.read(true);
        }
        OpenKind::ReadWrite => {
            options.read(true).write(true).create(true).truncate(false);
        }
        OpenKind::WriteExisting => {
            options.write(true);
        }
        OpenKind::Append => {
            options.append(true).create(true);
        }
        OpenKind::CreateNew => {
            options.write(true).create_new(true);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("stat {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "rollout path is not a regular file: {}",
            path.display()
        ));
    }
    if !matches!(kind, OpenKind::ReadExisting | OpenKind::WriteExisting) {
        secure_file(path)?;
    }
    Ok(file)
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut file = open_private(path, OpenKind::ReadExisting)?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("stat {}: {error}", path.display()))?;
    if metadata.len() > max_bytes {
        return Err(format!(
            "rollout file exceeds {max_bytes} byte bound: {}",
            path.display()
        ));
    }
    let mut raw = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut raw)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    if raw.len() as u64 > max_bytes {
        return Err(format!(
            "rollout file exceeds {max_bytes} byte bound: {}",
            path.display()
        ));
    }
    Ok(raw)
}

fn reject_symlink(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing symlinked rollout path {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("inspect {}: {error}", path.display())),
    }
}

fn check_private_file(path: &Path) -> Result<(), String> {
    reject_symlink(path)?;
    let metadata =
        fs::metadata(path).map_err(|error| format!("stat {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "rollout path is not a regular file: {}",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(format!(
                "rollout file permissions are not private: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn secure_dir(path: &Path) -> Result<(), String> {
    crate::workspace_store::private_io::PrivateDirectory::open_existing(path)
        .and_then(|directory| directory.secure_owner_only())
        .map_err(|error| format!("secure {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn secure_dir(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn secure_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    reject_symlink(path)?;
    let metadata =
        fs::metadata(path).map_err(|error| format!("stat {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "rollout path is not a regular file: {}",
            path.display()
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("secure {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn secure_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_data())
        .map_err(|error| format!("sync directory {}: {error}", path.display()))
}

fn reject_sensitive_bytes(bytes: &[u8]) -> Result<(), String> {
    if std::str::from_utf8(bytes)
        .ok()
        .is_some_and(crate::secrets::contains_secret)
    {
        return Err("refusing secret-bearing rollout bytes".to_string());
    }
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "../../../../tests/cockpit/harness/rollout__store__directory_tests.rs"]
mod directory_tests;
