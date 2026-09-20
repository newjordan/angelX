//! Bounded Caddy snapshots. Writers serialize through a stable sibling lock;
//! readers see either the old or new complete data file after atomic rename.
use super::{GetCommand, RecipeVerification, STORE_TAIL_BYTES};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const MAX_NEW_ENTRIES: usize = 1024;
const MAX_RECORD_BYTES: usize = 16 * 1024;
/// M05 entry ceiling: the published snapshot never carries more rows than
/// this regardless of bytes; oldest rows are dropped first (compaction).
pub(crate) const MAX_STORE_ROWS: usize = 2048;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReadHealth {
    pub(crate) clipped_prefix_bytes: u64,
    pub(crate) malformed_rows: usize,
    pub(crate) invalid_utf8_rows: usize,
    pub(crate) unterminated_tail: bool,
    pub(crate) io_error: Option<io::ErrorKind>,
}

impl ReadHealth {
    pub(crate) fn degraded(&self) -> bool {
        self.malformed_rows > 0
            || self.invalid_utf8_rows > 0
            || self.unterminated_tail
            || self.io_error.is_some()
    }

    pub(crate) fn summary(&self) -> String {
        format!(
            "malformed={} invalid-utf8={} incomplete-tail={} io={:?}",
            self.malformed_rows, self.invalid_utf8_rows, self.unterminated_tail, self.io_error
        )
    }
}

pub(crate) struct Loaded<T> {
    pub(crate) rows: Vec<T>,
    pub(crate) health: ReadHealth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WriteStatus {
    Unchanged,
    Published,
    Busy,
    CorruptInput,
    Failed(io::ErrorKind),
    PreservedTemporary(PathBuf),
    FailedTemporaryCleanup {
        cause: io::ErrorKind,
        cleanup: io::ErrorKind,
        path: PathBuf,
    },
    PublishedSyncUncertain(io::ErrorKind),
}

#[derive(Clone, Debug)]
pub(crate) struct WriteReport {
    pub(crate) status: WriteStatus,
    /// Newly supplied records present in the published snapshot, not attempted writes.
    pub(crate) written: usize,
    pub(crate) retained: usize,
    pub(crate) evicted: usize,
    /// Admission exclusions plus new rows evicted by a successful publication.
    /// Failed publications do not report speculative retention as eviction.
    pub(crate) skipped_new: usize,
    pub(crate) read_health: ReadHealth,
}

impl Default for WriteReport {
    fn default() -> Self {
        Self {
            status: WriteStatus::Unchanged,
            written: 0,
            retained: 0,
            evicted: 0,
            skipped_new: 0,
            read_health: ReadHealth::default(),
        }
    }
}

/// Preserve a byte snapshot's explicit metadata bound, including when the file
/// grows after metadata. One preceding byte prevents dropping an intact first row.
pub(super) fn tail_bytes(path: &Path, max_bytes: u64) -> io::Result<(Vec<u8>, u64, bool)> {
    if !std::fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let len = metadata.len();
    #[cfg(test)]
    super::storage_regression_tests::after_tail_metadata(path);
    if max_bytes == 0 {
        return Ok((Vec::new(), len, false));
    }
    let start = len.saturating_sub(max_bytes);
    let probe_start = start.saturating_sub(1);
    file.seek(SeekFrom::Start(probe_start))?;
    let expected = len - probe_start;
    let mut bytes = Vec::new();
    (&mut file).take(expected).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != expected {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
    }
    let unterminated = !bytes.is_empty() && !bytes.ends_with(b"\n");
    if start == 0 {
        return Ok((bytes, 0, unterminated));
    }
    let drop = if bytes.first() == Some(&b'\n') {
        1
    } else {
        bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |at| at + 1)
    };
    bytes.drain(..drop);
    Ok((bytes, probe_start + drop as u64, unterminated))
}

fn inspect<T: for<'de> Deserialize<'de>>(path: &Path) -> (Loaded<T>, Vec<Vec<u8>>) {
    let mut health = ReadHealth::default();
    let (bytes, clipped, unterminated) = match tail_bytes(path, STORE_TAIL_BYTES) {
        Ok(snapshot) => snapshot,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Loaded {
                    rows: Vec::new(),
                    health,
                },
                Vec::new(),
            );
        }
        Err(error) => {
            health.io_error = Some(error.kind());
            return (
                Loaded {
                    rows: Vec::new(),
                    health,
                },
                Vec::new(),
            );
        }
    };
    health.clipped_prefix_bytes = clipped;
    health.unterminated_tail = unterminated;
    let mut rows = Vec::new();
    let mut raw = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if std::str::from_utf8(line).is_err() {
            health.invalid_utf8_rows += 1;
            continue;
        }
        match serde_json::from_slice(line) {
            Ok(value) => {
                rows.push(value);
                let mut retained = line.to_vec();
                retained.push(b'\n');
                raw.push(retained);
            }
            Err(_) => health.malformed_rows += 1,
        }
    }
    (Loaded { rows, health }, raw)
}

pub(super) fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Loaded<T> {
    inspect(path).0
}

/// Inspect abandoned staging files and held advisory locks without creating a
/// lock or removing a peer's files. An unlocked persistent lock is normal.
pub(super) fn probe_writer(dir: &Path, name: &str) -> Option<WriteStatus> {
    let staging = dir.join(format!(".{name}.next"));
    match std::fs::symlink_metadata(&staging) {
        Ok(_) => return Some(WriteStatus::PreservedTemporary(staging)),
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            return Some(WriteStatus::Failed(e.kind()));
        }
        _ => {}
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    match options.open(dir.join(format!("{name}.lock"))) {
        Ok(file) => match file.try_lock() {
            Ok(()) => {
                let _ = file.unlock();
                None
            }
            Err(TryLockError::WouldBlock) => Some(WriteStatus::Busy),
            Err(TryLockError::Error(e)) => Some(WriteStatus::Failed(e.kind())),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => Some(WriteStatus::Failed(e.kind())),
    }
}

struct LimitedRecord(Vec<u8>);
impl Write for LimitedRecord {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MAX_RECORD_BYTES - 1 {
            return Err(io::Error::from(io::ErrorKind::FileTooLarge));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// A concurrent process spawn can duplicate this open file description until
// exec. Explicitly unlock it: closing only our descriptor can leave the lock
// held by the child and make the next append spuriously report Busy.
struct StoreLock(File);

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Stable lock filename remains in place across data-file replacement. Try once;
/// a busy peer is reported immediately and no UI or worker sleeps for the lock.
pub(super) fn append<T: Serialize + for<'de> Deserialize<'de> + GetCommand>(
    repo_dir: &Path,
    name: &str,
    entries: &[T],
    current_head: Option<&str>,
) -> WriteReport {
    let mut report = WriteReport::default();
    if entries.is_empty() && !repo_dir.join(name).exists() {
        return report;
    }
    if let Err(error) = prepare_directory(repo_dir) {
        report.status = WriteStatus::Failed(error.kind());
        return report;
    }
    let lock_path = repo_dir.join(format!("{name}.lock"));
    match std::fs::symlink_metadata(&lock_path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            report.status = WriteStatus::Failed(io::ErrorKind::InvalidInput);
            return report;
        }
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            report.status = WriteStatus::Failed(error.kind());
            return report;
        }
        _ => {}
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = match options.open(lock_path) {
        Ok(lock) => lock,
        Err(error) => {
            report.status = WriteStatus::Failed(error.kind());
            return report;
        }
    };
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            report.status = WriteStatus::Busy;
            return report;
        }
        Err(TryLockError::Error(error)) => {
            report.status = WriteStatus::Failed(error.kind());
            return report;
        }
    }
    let _lock = StoreLock(lock);
    let path = repo_dir.join(name);
    let (mut loaded, old_lines) = inspect::<serde_json::Value>(&path);
    report.read_health = loaded.health;
    report.read_health.malformed_rows += loaded
        .rows
        .iter()
        .filter(|value| serde_json::from_value::<T>((*value).clone()).is_err())
        .count();
    report.retained = loaded.rows.len();
    if let Some(kind) = report.read_health.io_error {
        report.status = WriteStatus::Failed(kind);
        return report;
    }
    if report.read_health.degraded() {
        report.status = WriteStatus::CorruptInput;
        return report;
    }
    let mut seen = HashSet::new();
    for value in &loaded.rows {
        if name == "recipes.jsonl"
            && !value.get("verification").is_some_and(|value| {
                serde_json::from_value::<RecipeVerification>(value.clone()).is_ok()
            })
        {
            continue;
        }
        if let (Some(command), Some(ts)) = (
            value.get("command").and_then(|v| v.as_str()),
            value.get("ts_ms").and_then(|v| v.as_u64()),
        ) {
            seen.insert((command.to_string(), ts / 86_400_000));
        }
    }
    // M05 compaction: drop superseded rows (same command, older ts) so a
    // fresh verified execution refreshes the stored record instead of
    // accumulating history. Recipes supersede per command; hazards per
    // (command, diagnostic). Rows are kept positionally aligned with
    // `loaded.rows` (both skip malformed/invalid lines identically).
    let mut keep = vec![true; loaded.rows.len()];
    let mut newest: HashMap<String, (usize, (bool, u64))> = HashMap::new();
    for (index, value) in loaded.rows.iter().enumerate() {
        let key = match (value.get("command").and_then(|v| v.as_str()), name) {
            (Some(command), "recipes.jsonl") if !value["workspace_state"].is_null() => {
                command.to_string()
            }
            (Some(command), "recipes.jsonl") => format!(
                "legacy\0{command}\0{}\0{}",
                value["ts_ms"].as_u64().unwrap_or(0) / 86_400_000,
                !value["verification"].is_null()
            ),
            (Some(command), "hazards.jsonl") => format!(
                "{command}\u{0}{}",
                value
                    .get("diagnostic")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
            ),
            _ => continue,
        };
        let ts = (
            name != "recipes.jsonl" || !value["verification"].is_null(),
            value.get("ts_ms").and_then(|v| v.as_u64()).unwrap_or(0),
        );
        match newest.get(&key) {
            Some((best, best_ts)) => {
                if ts > *best_ts {
                    keep[*best] = false;
                    newest.insert(key, (index, ts));
                } else {
                    keep[index] = false;
                }
            }
            None => {
                newest.insert(key, (index, ts));
            }
        }
    }
    let superseded = keep.iter().filter(|keep| !**keep).count();
    let mut old_lines = old_lines;
    let mut state_changed = false;
    if name == "recipes.jsonl"
        && let Some(state) =
            current_head.and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
    {
        for (index, value) in loaded.rows.iter_mut().enumerate() {
            if !keep[index] || value.get("observed_workspace_state") == Some(&state) {
                continue;
            }
            let previous = value
                .get("observed_workspace_state")
                .filter(|v| !v.is_null());
            let verified = value.get("workspace_state").filter(|v| !v.is_null());
            if previous.or(verified) != Some(&state) {
                let count = value["stale_since_changes"].as_u64().unwrap_or(0);
                value["stale_since_changes"] = serde_json::json!(
                    count
                        .saturating_add(1)
                        .min(super::RECIPE_STALE_CHANGE_LIMIT as u64)
                );
            }
            value["observed_workspace_state"] = state.clone();
            old_lines[index] = serde_json::to_vec(value).expect("JSON value serializes");
            old_lines[index].push(b'\n');
            state_changed = true;
        }
    }
    let mut retained: VecDeque<(Vec<u8>, bool)> = old_lines
        .into_iter()
        .zip(keep)
        .filter(|(_, keep)| *keep)
        .map(|(line, _)| (line, false))
        .collect();
    let mut bytes: usize = retained.iter().map(|(line, _)| line.len()).sum();
    let day = super::now_ms() / 86_400_000;
    let admission_skipped = entries.len().saturating_sub(MAX_NEW_ENTRIES);
    report.skipped_new = admission_skipped;
    // Admit the newest suffix, preserving execution order and first-observation
    // tie semantics within that suffix.
    for entry in &entries[entries.len().saturating_sub(MAX_NEW_ENTRIES)..] {
        let key = (entry.command().to_string(), day);
        if name != "recipes.jsonl" && seen.contains(&key) {
            continue;
        }
        let mut record = LimitedRecord(Vec::new());
        if serde_json::to_writer(&mut record, entry).is_err() {
            report.status = WriteStatus::Failed(io::ErrorKind::InvalidData);
            report.evicted = 0;
            report.skipped_new = admission_skipped;
            return report;
        }
        record.0.push(b'\n');
        // Refresh a recipe even on the same day; replaying the exact receipt is
        // a no-op. Never let an older/unverified row supersede verified evidence.
        if name == "recipes.jsonl" {
            let incoming: serde_json::Value =
                serde_json::from_slice(&record.0).expect("serialized row");
            if incoming["workspace_state"].is_null() {
                // V1 records have no workspace identity: preserve the existing
                // day-bucket migration contract until a bound verifier replaces
                // them. Untrusted legacy text never suppresses typed execution.
                if seen.contains(&key) {
                    continue;
                }
                bytes += record.0.len();
                retained.push_back((record.0, true));
                seen.insert(key);
                continue;
            }
            let mut reject = false;
            for (line, _) in &retained {
                let old: serde_json::Value = serde_json::from_slice(line).expect("validated row");
                if old["command"] == incoming["command"]
                    && (old["ts_ms"].as_u64() > incoming["ts_ms"].as_u64()
                        || (old["ts_ms"] == incoming["ts_ms"]
                            && old["workspace_state"] == incoming["workspace_state"])
                        || (!old["verification"].is_null() && incoming["verification"].is_null()))
                {
                    reject = true;
                }
            }
            if reject {
                continue;
            }
            retained.retain(|(line, _)| {
                let old: serde_json::Value = serde_json::from_slice(line).expect("validated row");
                if old["command"] == incoming["command"] {
                    bytes -= line.len();
                    report.evicted += 1;
                    false
                } else {
                    true
                }
            });
        }
        bytes += record.0.len();
        retained.push_back((record.0, true));
        seen.insert(key);
    }
    // Input snapshots and supplied batches need not arrive in timestamp order.
    // Evict the oldest evidence, not whichever row happened to be written first.
    retained.make_contiguous().sort_by_cached_key(|(line, _)| {
        serde_json::from_slice::<serde_json::Value>(line).expect("validated row")["ts_ms"]
            .as_u64()
            .unwrap_or(0)
    });
    // Enforce both ceilings even when every offered row was a duplicate.
    while bytes > store_cap(name) || retained.len() > MAX_STORE_ROWS {
        let (old, fresh) = retained.pop_front().expect("oversized snapshot");
        bytes -= old.len();
        report.evicted += 1;
        report.skipped_new += usize::from(fresh);
    }
    if superseded > 0 {
        // Superseded rows dropped by compaction count as evictions, but they
        // were not new records rejected this call.
        report.evicted += superseded;
        report.retained = report.retained.saturating_sub(superseded);
    }

    report.written = retained.iter().filter(|(_, fresh)| *fresh).count();
    report.retained = retained.len();
    if report.written == 0
        && report.evicted == 0
        && !state_changed
        && report.read_health.clipped_prefix_bytes == 0
    {
        return report;
    }
    let mut body = Vec::with_capacity(bytes);
    for (line, _) in retained {
        body.extend_from_slice(&line);
    }
    match publish(repo_dir, name, &body) {
        Ok(None) => report.status = WriteStatus::Published,
        Ok(Some(kind)) => report.status = WriteStatus::PublishedSyncUncertain(kind),
        Err(status) => {
            report.status = status;
            report.written = 0;
            report.retained = loaded.rows.len();
            report.evicted = 0;
            report.skipped_new = admission_skipped;
        }
    }
    report
}

fn store_cap(name: &str) -> usize {
    let store = if name == "recipes.jsonl" {
        "caddy_recipes"
    } else {
        "caddy_hazards"
    };
    crate::store_caps::value(store, "max_bytes") as usize
}

fn publish(repo_dir: &Path, name: &str, body: &[u8]) -> Result<Option<io::ErrorKind>, WriteStatus> {
    // The stable lock excludes cooperative publishers. A crash-left temporary
    // is never overwritten: create_new failure preserves it for inspection.
    let temporary = repo_dir.join(format!(".{name}.next"));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            WriteStatus::PreservedTemporary(temporary.clone())
        } else {
            WriteStatus::Failed(error.kind())
        }
    })?;
    // Redact each JSONL record without its delimiter. redact_bytes preserves
    // pretty layout for a JSON document containing newlines; passing a single
    // row plus its trailing newline would turn it into unreadable JSONL after
    // the first secret is scrubbed (and lose the terminating newline).
    let body: Vec<u8> = body
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .flat_map(|line| {
            let mut redacted = crate::secrets::redact_bytes(line);
            redacted.push(b'\n');
            redacted
        })
        .collect();
    let result: io::Result<Option<io::ErrorKind>> = (|| {
        if body.len() > store_cap(name) {
            return Err(io::Error::from(io::ErrorKind::FileTooLarge));
        }
        file.write_all(&body)?;
        file.sync_all()?;
        std::fs::rename(&temporary, repo_dir.join(name))?;
        Ok(File::open(repo_dir)
            .and_then(|directory| directory.sync_all())
            .err()
            .map(|error| error.kind()))
    })();
    // Only this successfully-created file is owned; a collision is returned
    // before this cleanup path. After rename this name no longer exists.
    match result {
        Ok(value) => Ok(value),
        Err(error) => match std::fs::remove_file(&temporary) {
            Ok(()) => Err(WriteStatus::Failed(error.kind())),
            Err(cleanup) => Err(WriteStatus::FailedTemporaryCleanup {
                cause: error.kind(),
                cleanup: cleanup.kind(),
                path: temporary,
            }),
        },
    }
}

// Secure only the explicitly configured Caddy root and this project directory.
// PrivateDirectory creates missing components as 0700 and checks every ancestor
// without following symlinks; unrelated existing ancestors keep their modes.
fn prepare_directory(repo_dir: &Path) -> io::Result<()> {
    use crate::workspace_store::private_io::PrivateDirectory;
    let base = super::caddy_dir();
    if repo_dir.parent() == Some(base.as_path()) {
        PrivateDirectory::open(&base)?.secure_owner_only()?;
    }
    PrivateDirectory::open(repo_dir)?.secure_owner_only()
}

#[cfg(all(test, unix))]
mod directory_tests;
