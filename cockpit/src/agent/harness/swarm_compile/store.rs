use super::schema::{SCHEMA_VERSION, SwarmRun};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static RUN_SEQ: AtomicU64 = AtomicU64::new(0);
static EVENT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Debug)]
pub(super) struct RunStore {
    root: PathBuf,
}

pub(super) struct RunLease {
    file: std::fs::File,
}

impl Drop for RunLease {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // SAFETY: this object owns the descriptor until after the unlock call.
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[derive(Serialize)]
struct RunEvent<'a> {
    ts_ms: u64,
    run_id: &'a str,
    level: &'a str,
    event: &'a str,
    detail: String,
}

impl RunStore {
    pub(super) fn for_workspace(workspace: &Path) -> Self {
        let root = std::env::var_os("ANGEL_SWARM_RUNS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                crate::platform::workspace_store::angel_subdir("swarm-runs")
                    .join(crate::platform::workspace_store::workspace_key(workspace))
            });
        Self { root }
    }

    #[cfg(test)]
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(super) fn new_run_id(&self) -> String {
        let seq = RUN_SEQ.fetch_add(1, Ordering::Relaxed);
        format!(
            "swr-{:x}-{:x}-{seq:x}",
            super::runner::now_ms(),
            std::process::id()
        )
    }

    pub(super) fn run_dir(&self, run_id: &str) -> Result<PathBuf, String> {
        validate_run_id(run_id)?;
        Ok(self.root.join("runs").join(run_id))
    }

    pub(super) fn save(&self, run: &SwarmRun) -> Result<PathBuf, String> {
        validate_run_id(&run.id)?;
        if run.version != SCHEMA_VERSION || run.kind != "angel.swarm_run" {
            return Err("refusing to persist an unknown swarm-run schema".to_string());
        }
        let dir = self.run_dir(&run.id)?;
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        secure_tree(&self.root, &dir)?;
        let path = dir.join("run.json");
        let tmp = dir.join("run.json.tmp");
        let body = crate::platform::secrets::to_redacted_vec_pretty(run)
            .map_err(|e| format!("serialize run: {e}"))?;
        fs::write(&tmp, body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        secure_file(&tmp)?;
        fs::rename(&tmp, &path).map_err(|e| format!("replace {}: {e}", path.display()))?;
        Ok(path)
    }

    pub(super) fn load(&self, run_id: &str) -> Result<SwarmRun, String> {
        let path = self.run_dir(run_id)?.join("run.json");
        let raw = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let run: SwarmRun =
            serde_json::from_slice(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
        if run.id != run_id || run.version != SCHEMA_VERSION || run.kind != "angel.swarm_run" {
            return Err("swarm-run identity/schema mismatch".to_string());
        }
        Ok(run)
    }

    pub(super) fn has_run(&self, run_id: &str) -> Result<bool, String> {
        Ok(self.run_dir(run_id)?.join("run.json").exists())
    }

    pub(super) fn acquire_lease(&self, run_id: &str) -> Result<RunLease, String> {
        use std::os::fd::AsRawFd;
        let dir = self.run_dir(run_id)?;
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        secure_tree(&self.root, &dir)?;
        let path = dir.join("run.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        secure_file(&path)?;
        // SAFETY: `file` owns a live descriptor for the lifetime of RunLease.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::WouldBlock {
                return Err(format!("swarm run {run_id} is already executing"));
            }
            return Err(format!("lock {}: {error}", path.display()));
        }
        Ok(RunLease { file })
    }

    pub(super) fn event(
        &self,
        run_id: &str,
        level: &str,
        event: &str,
        detail: &str,
    ) -> Result<(), String> {
        let dir = self.run_dir(run_id)?;
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        secure_tree(&self.root, &dir)?;
        let record = RunEvent {
            ts_ms: super::runner::now_ms(),
            run_id,
            level,
            event,
            detail: compact_scrubbed(detail, 600),
        };
        let mut line = crate::platform::secrets::to_redacted_vec(&record)
            .map_err(|e| format!("encode event: {e}"))?;
        line.push(b'\n');
        let _guard = EVENT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = dir.join("events.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        secure_file(&path)?;
        file.write_all(&line)
            .map_err(|e| format!("append {}: {e}", path.display()))
    }

    pub(super) fn policy_path(&self) -> PathBuf {
        self.root.join("routing-policy.json")
    }

    pub(super) fn scratch_dir(&self, run_id: &str) -> Result<PathBuf, String> {
        Ok(self.run_dir(run_id)?.join("worktrees"))
    }
}

pub(super) fn compact_scrubbed(text: &str, max_chars: usize) -> String {
    let scrubbed = crate::knowledge::experience::scrub_secrets(text);
    if scrubbed.chars().count() <= max_chars {
        return scrubbed;
    }
    let mut out: String = scrubbed.chars().take(max_chars).collect();
    out.push_str(" …[truncated]");
    out
}

fn validate_run_id(run_id: &str) -> Result<(), String> {
    let valid = run_id.starts_with("swr-")
        && run_id.len() <= 96
        && run_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    valid
        .then_some(())
        .ok_or_else(|| "invalid swarm run id".to_string())
}

#[cfg(unix)]
fn secure_tree(root: &Path, leaf: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let runs = root.join("runs");
    for dir in [root, runs.as_path(), leaf] {
        if dir.exists() {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("secure {}: {e}", dir.display()))?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn secure_tree(_root: &Path, _leaf: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn secure_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("secure {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn secure_file(_path: &Path) -> Result<(), String> {
    Ok(())
}
