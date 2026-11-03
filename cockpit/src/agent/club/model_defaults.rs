//! Measured defaults only. Exact model IDs avoid applying measurements to a new revision.
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const EMBEDDED: &str = include_str!("../../../../docs/telemetry/model-calibration.toml");

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Entry {
    pub model: String,
    pub default_effort: Option<String>,
    #[serde(rename = "turn_idle_secs")]
    pub _turn_idle_secs: Option<u64>,
    #[serde(default)]
    pub date: String,
    pub receipt: String,
    pub liveness_receipt: Option<String>,
    pub reason: String,
}

#[derive(Deserialize)]
struct Table {
    #[serde(default)]
    date: String,
    #[serde(default)]
    models: Vec<Entry>,
}

fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let table: Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut entries = table.models;
    for entry in &mut entries {
        if entry.date.is_empty() {
            entry.date.clone_from(&table.date);
        }
        if entry.date.is_empty() || entry.receipt.is_empty() || entry.reason.is_empty() {
            return Err("calibration requires date, receipt and reason".into());
        }
        if entry.default_effort.as_deref().is_some_and(|v| {
            !matches!(
                v,
                "auto" | "none" | "minimal" | "low" | "medium" | "high" | "max" | "xhigh" | "ultra"
            )
        }) {
            return Err("invalid calibration effort".into());
        }
    }
    Ok(entries)
}

fn load(path: Option<&Path>) -> Result<Vec<Entry>, String> {
    let mut entries = parse(EMBEDDED)?;
    if let Some(path) = path {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                for entry in parse(&text)? {
                    entries.retain(|old| !old.model.eq_ignore_ascii_case(&entry.model));
                    entries.push(entry);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(entries)
}

// A published calibration is immutable. Writers must create a complete new file
// and rename it over the destination; modifications to an accepted inode are
// ignored, so a truncating or partially written file cannot change a request.
const MAX_CALIBRATION_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileVersion {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

impl FileVersion {
    fn from_metadata(m: &std::fs::Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            len: m.len(),
            mtime: (m.mtime(), m.mtime_nsec()),
            ctime: (m.ctime(), m.ctime_nsec()),
        }
    }
    fn same_inode(self, other: Self) -> bool {
        self.dev == other.dev && self.ino == other.ino
    }
}

struct CalibrationCache {
    path: Option<PathBuf>,
    entries: Arc<Vec<Entry>>,
    accepted: Option<FileVersion>,
    rejected: Option<FileVersion>,
    // Keep published/rejected inodes alive so an atomic replacement cannot
    // reuse an inode number and be mistaken for an in-place modification.
    accepted_file: Option<std::fs::File>,
    rejected_file: Option<std::fs::File>,
}

impl CalibrationCache {
    fn new() -> Self {
        Self {
            path: None,
            entries: Arc::new(parse(EMBEDDED).expect("valid embedded model calibration")),
            accepted: None,
            rejected: None,
            accepted_file: None,
            rejected_file: None,
        }
    }

    fn snapshot(&mut self, path: Option<&Path>) -> Arc<Vec<Entry>> {
        if self.path.as_deref() != path {
            self.path = path.map(Path::to_path_buf);
            self.entries = Arc::new(parse(EMBEDDED).expect("valid embedded model calibration"));
            self.accepted = None;
            self.rejected = None;
            self.accepted_file = None;
            self.rejected_file = None;
        }
        let Some(path) = path else {
            return Arc::clone(&self.entries);
        };
        let Ok(metadata) = std::fs::metadata(path) else {
            return Arc::clone(&self.entries);
        };
        let version = FileVersion::from_metadata(&metadata);
        if self.accepted == Some(version) || self.rejected.is_some_and(|v| v.same_inode(version)) {
            return Arc::clone(&self.entries);
        }
        if self.accepted.is_some_and(|v| v.same_inode(version)) {
            self.rejected = Some(version);
            eprintln!(
                "[angel] model calibration changed in place; retain current defaults and publish by atomic rename"
            );
            return Arc::clone(&self.entries);
        }
        let read = || -> Result<(Vec<Entry>, std::fs::File), String> {
            if !metadata.is_file() || version.len > MAX_CALIBRATION_BYTES {
                return Err("calibration is not a bounded regular file".into());
            }
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path)
                .map_err(|e| e.to_string())?;
            if FileVersion::from_metadata(&file.metadata().map_err(|e| e.to_string())?) != version {
                return Err("calibration changed before read".into());
            }
            let mut bytes = Vec::new();
            (&file)
                .take(MAX_CALIBRATION_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_CALIBRATION_BYTES
                || FileVersion::from_metadata(&file.metadata().map_err(|e| e.to_string())?)
                    != version
                || FileVersion::from_metadata(&std::fs::metadata(path).map_err(|e| e.to_string())?)
                    != version
            {
                return Err("calibration changed during read".into());
            }
            let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
            let mut entries = parse(EMBEDDED)?;
            for entry in parse(text)? {
                entries.retain(|old| !old.model.eq_ignore_ascii_case(&entry.model));
                entries.push(entry);
            }
            Ok((entries, file))
        };
        match read() {
            Ok((entries, file)) => {
                self.entries = Arc::new(entries);
                self.accepted = Some(version);
                self.accepted_file = Some(file);
                self.rejected = None;
                self.rejected_file = None;
            }
            Err(_) => {
                self.rejected = Some(version);
                self.rejected_file = std::fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(path)
                    .ok()
                    .filter(|file| {
                        file.metadata()
                            .ok()
                            .is_some_and(|m| FileVersion::from_metadata(&m) == version)
                    });
                eprintln!(
                    "[angel] invalid or incomplete model calibration; retaining current defaults"
                );
            }
        }
        Arc::clone(&self.entries)
    }
}

thread_local! {
    static REQUEST_CALIBRATION: RefCell<Option<Arc<Vec<Entry>>>> = const { RefCell::new(None) };
}

fn live_snapshot() -> Arc<Vec<Entry>> {
    static CACHE: OnceLock<Mutex<CalibrationCache>> = OnceLock::new();
    let path =
        std::env::var_os("HOME").map(|p| Path::new(&p).join(".angelX/model-calibration.toml"));
    CACHE
        .get_or_init(|| Mutex::new(CalibrationCache::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .snapshot(path.as_deref())
}

fn snapshot() -> Arc<Vec<Entry>> {
    REQUEST_CALIBRATION
        .with(|slot| slot.borrow().clone())
        .unwrap_or_else(live_snapshot)
}

pub(crate) struct RequestCalibrationGuard(bool, std::marker::PhantomData<std::rc::Rc<()>>);

/// Pin a generation for one request, including retries and identity recording.
/// Nested calls retain the outer generation.
pub(crate) fn pin_request() -> RequestCalibrationGuard {
    REQUEST_CALIBRATION.with(|slot| {
        if slot.borrow().is_some() {
            return RequestCalibrationGuard(false, std::marker::PhantomData);
        }
        let current = live_snapshot();
        *slot.borrow_mut() = Some(current);
        RequestCalibrationGuard(true, std::marker::PhantomData)
    })
}

pub(crate) fn request_is_pinned() -> bool {
    REQUEST_CALIBRATION.with(|slot| slot.borrow().is_some())
}

impl Drop for RequestCalibrationGuard {
    fn drop(&mut self) {
        if self.0 {
            REQUEST_CALIBRATION.with(|slot| *slot.borrow_mut() = None);
        }
    }
}

pub(crate) fn entry(model: &str) -> Option<Entry> {
    snapshot()
        .iter()
        .find(|e| e.model.eq_ignore_ascii_case(model))
        .cloned()
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn resolve<T>(explicit: Option<T>, table: Option<T>, provider: T, date: &str) -> (T, String) {
    if let Some(value) = explicit {
        (value, "env".into())
    } else if let Some(value) = table {
        (value, format!("table {date}"))
    } else {
        (provider, "provider".into())
    }
}

pub(crate) fn budgets(model: &str, namespace: &str) -> Value {
    let entry = entry(model);
    let date = entry.as_ref().map_or("", |e| e.date.as_str());
    let prefix = if namespace.starts_with("ANGEL_") {
        namespace.to_string()
    } else {
        format!("ANGEL_{}", namespace.to_ascii_uppercase())
    };
    let (effort, effort_source) = resolve(
        env(&format!("{prefix}_REASONING_EFFORT")).or_else(|| env("ANGEL_REASONING_EFFORT")),
        entry.as_ref().and_then(|e| e.default_effort.clone()),
        "auto".into(),
        date,
    );
    let (idle, idle_source) = resolve(
        env("ANGEL_TURN_IDLE_TIMEOUT_SECS").and_then(|s| s.parse::<u64>().ok()),
        None,
        0,
        date,
    );
    json!({"reasoning_effort": effort, "reasoning_effort_source": effort_source,
        "turn_idle_timeout_secs": idle, "turn_idle_timeout_source": idle_source,
        "calibration_receipt": entry.as_ref().map(|e| &e.receipt),
        "calibration_liveness_receipt": entry.as_ref().and_then(|e| e.liveness_receipt.as_ref()),
        "calibration_notes": entry.as_ref().map(|e| &e.reason)})
}

pub(crate) fn summary(club: &dyn super::Club) -> String {
    let b = club.resolved_model_defaults();
    format!(
        "effort={}[{}] turn-idle={}s[{}]",
        club.reasoning_effort().unwrap_or_else(|| "auto".into()),
        b["reasoning_effort_source"].as_str().unwrap_or("provider"),
        b["turn_idle_timeout_secs"],
        b["turn_idle_timeout_source"].as_str().unwrap_or("provider")
    )
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/model_defaults__tests.rs"]
mod tests;
