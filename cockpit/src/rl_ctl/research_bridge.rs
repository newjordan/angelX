//! Bundled, offline original Sloptomizer algorithms. No Apollo checkout or pip.

use super::*;
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

// Literal paths keep the release closure mechanically checkable.
const FILES: &[(&str, &[u8])] = &[
    (
        "runner.py",
        include_bytes!("../../research/sloptomizer/runner.py"),
    ),
    (
        "UPSTREAM.json",
        include_bytes!("../../research/sloptomizer/UPSTREAM.json"),
    ),
    (
        "autoresearch/__init__.py",
        include_bytes!("../../research/sloptomizer/autoresearch/__init__.py"),
    ),
    (
        "autoresearch/gepa/__init__.py",
        include_bytes!("../../research/sloptomizer/autoresearch/gepa/__init__.py"),
    ),
    (
        "autoresearch/gepa/population.py",
        include_bytes!("../../research/sloptomizer/autoresearch/gepa/population.py"),
    ),
    (
        "autoresearch/gepa/select.py",
        include_bytes!("../../research/sloptomizer/autoresearch/gepa/select.py"),
    ),
    (
        "orchestrator/__init__.py",
        include_bytes!("../../research/sloptomizer/orchestrator/__init__.py"),
    ),
    (
        "orchestrator/self_improvement/__init__.py",
        include_bytes!("../../research/sloptomizer/orchestrator/self_improvement/__init__.py"),
    ),
    (
        "orchestrator/self_improvement/bandits.py",
        include_bytes!("../../research/sloptomizer/orchestrator/self_improvement/bandits.py"),
    ),
    (
        "orchestrator/self_improvement/stats.py",
        include_bytes!("../../research/sloptomizer/orchestrator/self_improvement/stats.py"),
    ),
    (
        "orchestrator/micro_llm/__init__.py",
        include_bytes!("../../research/sloptomizer/orchestrator/micro_llm/__init__.py"),
    ),
    (
        "orchestrator/micro_llm/core.py",
        include_bytes!("../../research/sloptomizer/orchestrator/micro_llm/core.py"),
    ),
    (
        "orchestrator/caseops_codec.py",
        include_bytes!("../../research/sloptomizer/orchestrator/caseops_codec.py"),
    ),
    (
        "orchestrator/caseops_crystal_codec.py",
        include_bytes!("../../research/sloptomizer/orchestrator/caseops_crystal_codec.py"),
    ),
    (
        "orchestrator/caseops_taxonomy_codec.py",
        include_bytes!("../../research/sloptomizer/orchestrator/caseops_taxonomy_codec.py"),
    ),
    (
        "orchestrator/caseops_vocab_analogy_codec.py",
        include_bytes!("../../research/sloptomizer/orchestrator/caseops_vocab_analogy_codec.py"),
    ),
    (
        "orchestrator/caseops_vocab_tensor_codec.py",
        include_bytes!("../../research/sloptomizer/orchestrator/caseops_vocab_tensor_codec.py"),
    ),
];

pub(super) fn ensure_dir(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(format!(
            "research directory is not a plain directory: {}",
            path.display()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                ensure_dir(parent)?;
            }
            match std::fs::DirBuilder::new().mode(0o700).create(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => ensure_dir(path),
                Err(e) => Err(e.to_string()),
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

pub(super) fn read(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("research store entry is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Preserve exact bytes, including receipt hashes. The generic transcript store
/// redacts content and therefore cannot write source code or learning state.
pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("research artifact needs parent")?;
    ensure_dir(parent)?;
    let temp = parent.join(format!(".{}.tmp", new_run_id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

struct StoreLock(File);
impl StoreLock {
    fn acquire(root: &Path, cancel: &AtomicBool) -> Result<Self, String> {
        ensure_dir(root)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("state.lock"))
            .map_err(|e| e.to_string())?;
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("research state operation cancelled".into());
            }
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Self(file));
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::WouldBlock {
                return Err(error.to_string());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for StoreLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn runtime() -> Result<PathBuf, String> {
    let identity = FILES
        .iter()
        .map(|(name, bytes)| format!("{name}:{}\n", crate::cut::sha256_hex(bytes)))
        .collect::<String>();
    let root = rl_root()
        .join("sloptomizer-runtime")
        .join(crate::cut::sha256_hex(identity.as_bytes()));
    ensure_dir(&root)?;
    for (name, bytes) in FILES {
        let path = root.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
                return Err("bundled research runtime contains a non-regular file".into());
            }
            Ok(_) if read(&path)? == *bytes => {}
            Ok(_) => write(&path, bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => write(&path, bytes)?,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(root)
}

struct Spool(PathBuf);
impl Drop for Spool {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Lock across read/transform/atomic publication: concurrent sessions cannot
/// lose feedback. A failed transform preserves the original state verbatim.
pub(super) fn transform(
    root: &Path,
    mut request: Value,
    cancel: &AtomicBool,
) -> Result<Value, String> {
    let _lock = StoreLock::acquire(root, cancel)?;
    let state_path = root.join("state.json");
    request["state"] = match std::fs::symlink_metadata(&state_path) {
        Ok(_) => {
            let state: Value = serde_json::from_slice(&read(&state_path)?)
                .map_err(|e| format!("research state corrupt; preserved: {e}"))?;
            if state["schema"] != "angel.sloptomizer-options/v1" {
                return Err("unsupported research state; original preserved".into());
            }
            state
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(e) => return Err(e.to_string()),
    };
    let runtime = runtime()?;
    let spool = Spool(root.join(new_run_id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&spool.0)
        .map_err(|e| e.to_string())?;
    write(
        &spool.0.join("request.json"),
        &serde_json::to_vec(&request).map_err(|e| e.to_string())?,
    )?;
    let mut command = std::process::Command::new(
        std::env::var_os("ANGEL_RESEARCH_PYTHON").unwrap_or_else(|| "python3".into()),
    );
    command
        .args(["-I", "-S", "-B"])
        .arg(runtime.join("runner.py"))
        .arg(spool.0.join("request.json"))
        .arg(spool.0.join("response.json"))
        .current_dir(&spool.0);
    let result = crate::harness::output_timed_captured_cancellable(command, None, Some(cancel))
        .map_err(|e| format!("Sloptomizer Python unavailable: {e}"))?;
    if result.cancelled || cancel.load(Ordering::Acquire) {
        return Err("research operation cancelled; state preserved".into());
    }
    if !result.output.status.success() {
        return Err(format!(
            "Sloptomizer failed; state preserved: {}",
            String::from_utf8_lossy(&result.output.stderr)
        ));
    }
    let response: Value = serde_json::from_slice(&read(&spool.0.join("response.json"))?)
        .map_err(|e| e.to_string())?;
    if !response["advice"].is_object()
        || response["state"]["schema"] != "angel.sloptomizer-options/v1"
    {
        return Err("Sloptomizer returned an invalid state; original preserved".into());
    }
    if response["changed"] == true {
        write(
            &state_path,
            &serde_json::to_vec(&response["state"]).map_err(|e| e.to_string())?,
        )?;
    }
    Ok(json!({"advice":response["advice"], "state_path":state_path, "updated":response["changed"]}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sloptomizer_bundled_originals_match_source_receipt() {
        let manifest: Value =
            serde_json::from_slice(FILES.iter().find(|(p, _)| *p == "UPSTREAM.json").unwrap().1)
                .unwrap();
        assert_eq!(
            manifest["source_head"],
            "0e3c17b539bf14ffa6c546f25b648bb58b4514ac"
        );
        let rows = manifest["files"].as_array().unwrap();
        assert_eq!(rows.len(), 10);
        for row in rows {
            let (_, bytes) = FILES
                .iter()
                .find(|(p, _)| Some(*p) == row["path"].as_str())
                .unwrap();
            assert_eq!(
                crate::cut::sha256_hex(bytes),
                row["sha256"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn sloptomizer_concurrent_feedback_is_durable_deduplicated_and_failure_preserves_state() {
        let _lock = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!("angel-slop-store-{}", new_run_id()));
        let _env = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.to_str().unwrap());
        let scope = root.join("learning");
        // Synthetic bridge inputs test storage, not objective verification. Only
        // the production controller admits observations through physical evidence.
        let event = |i| {
            json!({"action":"observe","task":"fixture objective","observation":{
                "id":format!("synthetic-{i}"),"idea":format!("fixture idea {i}"),"approach":"fixture",
                "task":"fixture objective","receipt_sha256":"synthetic-receipt","source_sha256":"synthetic-source","passed":true
            }})
        };
        let workers = (0..4)
            .map(|i| {
                let scope = scope.clone();
                let request = event(i);
                std::thread::spawn(move || {
                    transform(&scope, request, &AtomicBool::new(false)).unwrap()
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let repeated = transform(&scope, event(0), &AtomicBool::new(false)).unwrap();
        assert_eq!(repeated["advice"]["observations"], 4);
        assert_eq!(repeated["updated"], false);
        let before = read(&scope.join("state.json")).unwrap();
        let mut conflict = event(0);
        conflict["observation"]["passed"] = json!(false);
        assert!(
            transform(&scope, conflict, &AtomicBool::new(false))
                .unwrap_err()
                .contains("conflicting observation")
        );
        assert_eq!(read(&scope.join("state.json")).unwrap(), before);
        assert!(
            transform(&scope, event(4), &AtomicBool::new(true))
                .unwrap_err()
                .contains("cancelled")
        );
        assert_eq!(read(&scope.join("state.json")).unwrap(), before);
        write(&scope.join("state.json"), b"null").unwrap();
        assert!(
            transform(&scope, json!({"action":"suggest"}), &AtomicBool::new(false))
                .unwrap_err()
                .contains("unsupported")
        );
        assert_eq!(read(&scope.join("state.json")).unwrap(), b"null");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
