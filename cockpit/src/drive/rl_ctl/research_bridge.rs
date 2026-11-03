//! Bundled, offline original Sloptomizer algorithms. No external checkout or pip.

use super::*;
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::sync::{Mutex, OnceLock};

// Literal paths keep the release closure mechanically checkable.
const FILES: &[(&str, &[u8])] = &[
    (
        "relations.py",
        include_bytes!("../../../research/sloptomizer/relations.py"),
    ),
    (
        "runner.py",
        include_bytes!("../../../research/sloptomizer/runner.py"),
    ),
    (
        "UPSTREAM.json",
        include_bytes!("../../../research/sloptomizer/UPSTREAM.json"),
    ),
    (
        "autoresearch/__init__.py",
        include_bytes!("../../../research/sloptomizer/autoresearch/__init__.py"),
    ),
    (
        "autoresearch/gepa/__init__.py",
        include_bytes!("../../../research/sloptomizer/autoresearch/gepa/__init__.py"),
    ),
    (
        "autoresearch/gepa/population.py",
        include_bytes!("../../../research/sloptomizer/autoresearch/gepa/population.py"),
    ),
    (
        "autoresearch/gepa/select.py",
        include_bytes!("../../../research/sloptomizer/autoresearch/gepa/select.py"),
    ),
    (
        "orchestrator/__init__.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/__init__.py"),
    ),
    (
        "orchestrator/self_improvement/__init__.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/self_improvement/__init__.py"),
    ),
    (
        "orchestrator/self_improvement/bandits.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/self_improvement/bandits.py"),
    ),
    (
        "orchestrator/self_improvement/stats.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/self_improvement/stats.py"),
    ),
    (
        "orchestrator/micro_llm/__init__.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/micro_llm/__init__.py"),
    ),
    (
        "orchestrator/micro_llm/core.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/micro_llm/core.py"),
    ),
    (
        "orchestrator/caseops_codec.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/caseops_codec.py"),
    ),
    (
        "orchestrator/caseops_crystal_codec.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/caseops_crystal_codec.py"),
    ),
    (
        "orchestrator/caseops_taxonomy_codec.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/caseops_taxonomy_codec.py"),
    ),
    (
        "orchestrator/caseops_vocab_analogy_codec.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/caseops_vocab_analogy_codec.py"),
    ),
    (
        "orchestrator/caseops_vocab_tensor_codec.py",
        include_bytes!("../../../research/sloptomizer/orchestrator/caseops_vocab_tensor_codec.py"),
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
        // Open before checking the handle type, without waiting for a FIFO's
        // writer. Regular files retain their ordinary read behavior.
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
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

type RuntimeFiles = Vec<(String, Vec<u8>)>;

fn embedded_files() -> RuntimeFiles {
    FILES
        .iter()
        .map(|(name, bytes)| ((*name).to_string(), bytes.to_vec()))
        .collect()
}

fn runtime_identity(files: &RuntimeFiles) -> String {
    let identity = files
        .iter()
        .map(|(name, bytes)| format!("{name}:{}\n", crate::knowledge::cut::sha256_hex(bytes)))
        .collect::<String>();
    crate::knowledge::cut::sha256_hex(identity.as_bytes())
}

/// Publish a whole dependency set under its content identity. Published
/// generations are never repaired in place: a running interpreter may still
/// be importing from them.
fn materialize(files: &RuntimeFiles) -> Result<PathBuf, String> {
    let root = rl_root()
        .join("sloptomizer-runtime")
        .join(runtime_identity(files));
    let parent = root.parent().ok_or("research runtime needs a parent")?;
    ensure_dir(parent)?;
    if !root.exists() {
        let stage = Spool(parent.join(format!(".candidate-{}", new_run_id())));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&stage.0)
            .map_err(|e| e.to_string())?;
        for (name, bytes) in files {
            write(&stage.0.join(name), bytes)?;
        }
        match std::fs::rename(&stage.0, &root) {
            Ok(()) => {}
            Err(_) if root.is_dir() => {}
            Err(e) => return Err(format!("publish research runtime: {e}")),
        }
    }
    let meta = std::fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("research runtime generation is not a plain directory".into());
    }
    fn check_entries(dir: &Path, root: &Path, files: &RuntimeFiles) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                if !files
                    .iter()
                    .any(|(name, _)| Path::new(name).starts_with(relative))
                {
                    return Err(
                        "research runtime generation contains an unexpected directory".into(),
                    );
                }
                check_entries(&path, root, files)?;
            } else if !kind.is_file() || !files.iter().any(|(name, _)| Path::new(name) == relative)
            {
                return Err("research runtime generation contains an unexpected entry".into());
            }
        }
        Ok(())
    }
    check_entries(&root, &root, files)?;
    for (name, bytes) in files {
        if read_runtime_file(&root.join(name), MAX_RUNTIME_FILE)? != *bytes {
            return Err("research runtime generation was modified; refusing to use it".into());
        }
    }
    Ok(root)
}

const GENERATION_SCHEMA: &str = "angel.sloptomizer-generation/v1";
const MAX_RUNTIME_FILE: u64 = 2 * 1024 * 1024;
const MAX_RUNTIME_TOTAL: usize = 8 * 1024 * 1024;

fn read_runtime_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("{} exceeds its size limit", path.display()));
    }
    Ok(bytes)
}

/// `generation.json` is published last by the operator. The digest for every
/// declared dependency must match, and the manifest must remain unchanged
/// throughout the read; an edit in progress cannot publish a mixed set.
fn snapshot_candidate(dir: &Path) -> Result<Option<RuntimeFiles>, String> {
    let manifest_path = dir.join("generation.json");
    match std::fs::symlink_metadata(&manifest_path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("inspect {}: {e}", manifest_path.display())),
    }
    let raw = read_runtime_file(&manifest_path, 64 * 1024)?;
    let manifest: Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("invalid research generation manifest: {e}"))?;
    if manifest["schema"] != GENERATION_SCHEMA {
        return Err("unsupported research generation manifest".into());
    }
    let entries = manifest["files"]
        .as_object()
        .ok_or("research generation needs a files object")?;
    if entries.len() != FILES.len() {
        return Err("research generation must name every bundled dependency exactly once".into());
    }
    let mut files = Vec::with_capacity(FILES.len());
    let mut total = 0usize;
    for (name, _) in FILES {
        let digest = entries
            .get(*name)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("research generation is missing {name}"))?;
        let bytes = read_runtime_file(&dir.join(name), MAX_RUNTIME_FILE)?;
        total = total.saturating_add(bytes.len());
        if total > MAX_RUNTIME_TOTAL {
            return Err("research generation exceeds its total size limit".into());
        }
        if crate::knowledge::cut::sha256_hex(&bytes) != digest {
            return Err(format!("research generation digest mismatch: {name}"));
        }
        files.push(((*name).to_string(), bytes));
    }
    if read_runtime_file(&manifest_path, 64 * 1024)? != raw {
        return Err("research generation changed while reading".into());
    }
    Ok(Some(files))
}

/// Compile candidate sources and check their direct local imports without
/// importing or executing any candidate code. Runtime-only behavior is checked
/// by the first real request before a trial can become last-known-good.
fn validate_candidate(root: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let probe = r#"import ast, importlib.util, pathlib, sys
p = pathlib.Path(sys.argv[1])
local_roots = {'autoresearch', 'orchestrator', 'relations', 'runner'}
def check(module):
    if not module:
        return
    if module.split('.')[0] in local_roots:
        candidate = p.joinpath(*module.split('.'))
        if not candidate.with_suffix('.py').is_file() and not (candidate / '__init__.py').is_file():
            raise ValueError('missing local import: ' + module)
    elif module.split('.')[0] not in sys.builtin_module_names and importlib.util.find_spec(module.split('.')[0]) is None:
        raise ValueError('missing standard-library import: ' + module)
for f in p.rglob('*.py'):
    tree = ast.parse(f.read_bytes(), str(f), 'exec')
    compile(tree, str(f), 'exec')
    if f.name == 'runner.py' and f.parent == p and not any(isinstance(x, (ast.FunctionDef, ast.AsyncFunctionDef)) and x.name == 'execute' for x in tree.body):
        raise ValueError('runner is missing execute')
    for node in tree.body:
        if isinstance(node, ast.Import):
            for alias in node.names: check(alias.name)
        elif isinstance(node, ast.ImportFrom):
            if node.level:
                package = list(f.relative_to(p).parts[:-1])
                if node.level > len(package): raise ValueError('invalid relative import')
                base = package[:len(package) - node.level + 1]
                check('.'.join(base + ([node.module] if node.module else [])))
            else: check(node.module)
"#;
    let mut command = std::process::Command::new(
        std::env::var_os("ANGEL_RESEARCH_PYTHON").unwrap_or_else(|| "python3".into()),
    );
    command
        .args(["-I", "-S", "-B", "-c", probe])
        .arg(root)
        .current_dir(root);
    if cancel.load(Ordering::Acquire) {
        return Err("research candidate validation cancelled".into());
    }
    let output = crate::agent::harness::exec::output_timed_fixed_captured_cancellable(
        command,
        Duration::from_secs(15),
        cancel,
    )
    .map_err(|e| format!("research candidate validation failed: {e}"))?;
    if output.cancelled || cancel.load(Ordering::Acquire) {
        return Err("research candidate validation cancelled".into());
    }
    if output.timed_out {
        return Err("research candidate validation timed out".into());
    }
    if !output.output.status.success() {
        return Err(format!(
            "research candidate validation failed: {}",
            crate::platform::secrets::redact_error(&String::from_utf8_lossy(&output.output.stderr))
        ));
    }
    Ok(())
}

struct ActiveRuntime {
    source: PathBuf,
    destination: PathBuf,
    python: std::ffi::OsString,
    files: RuntimeFiles,
    defer_once: Option<(String, String)>,
    pending: Option<RuntimeFiles>,
}

static RUNTIMES: OnceLock<Mutex<Vec<ActiveRuntime>>> = OnceLock::new();

/// A candidate does not become the fallback until one real, state-validated
/// request succeeds. Other requests continue on the accepted generation.
struct RuntimeTrial {
    source: PathBuf,
    destination: PathBuf,
    python: std::ffi::OsString,
    id: String,
    outcome: Option<bool>,
}

impl Drop for RuntimeTrial {
    fn drop(&mut self) {
        let Some(lock) = RUNTIMES.get() else { return };
        let mut runtimes = lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(active) = runtimes.iter_mut().find(|r| {
            r.source == self.source && r.destination == self.destination && r.python == self.python
        }) {
            if active
                .pending
                .as_ref()
                .is_some_and(|p| runtime_identity(p) == self.id)
            {
                let pending = active.pending.take().expect("trial was present");
                match self.outcome {
                    Some(true) => {
                        active.files = pending;
                        active.defer_once = None;
                    }
                    // A bad request or transient I/O failure does not prove the
                    // candidate is invalid. Let the next request use the old
                    // generation, then retry the same digest on a later request.
                    Some(false) => {
                        active.defer_once =
                            Some((self.id.clone(), "research candidate trial failed".into()))
                    }
                    None => {}
                }
            }
        }
    }
}

/// A process retains one accepted generation per resource root and storage
/// root. Different learning scopes still serialize their own writers by the
/// existing store lock; in-flight subprocesses keep their immutable path.
fn runtime(cancel: &AtomicBool) -> Result<(PathBuf, Option<String>, Option<RuntimeTrial>), String> {
    let source = std::env::var_os("ANGEL_RESEARCH_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            crate::platform::runtime_paths::cockpit_dir().join("research/sloptomizer")
        });
    let destination = rl_root();
    let python = std::env::var_os("ANGEL_RESEARCH_PYTHON").unwrap_or_else(|| "python3".into());
    let lock = RUNTIMES.get_or_init(|| Mutex::new(Vec::new()));
    let mut runtimes = loop {
        if cancel.load(Ordering::Acquire) {
            return Err("research operation cancelled".into());
        }
        match lock.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::Poisoned(e)) => break e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(5))
            }
        }
    };
    let index = runtimes
        .iter()
        .position(|r| r.source == source && r.destination == destination && r.python == python)
        .unwrap_or_else(|| {
            runtimes.push(ActiveRuntime {
                source: source.clone(),
                destination: destination.clone(),
                python: python.clone(),
                files: embedded_files(),
                defer_once: None,
                pending: None,
            });
            runtimes.len() - 1
        });
    let active = &mut runtimes[index];
    let mut warning = None;
    let mut trial = None;
    let mut selected = None;
    match snapshot_candidate(&source) {
        Ok(Some(files)) => {
            let id = runtime_identity(&files);
            if id != runtime_identity(&active.files)
                && active.pending.is_none()
                && active
                    .defer_once
                    .as_ref()
                    .is_some_and(|(deferred, _)| deferred == &id)
            {
                let (_, reason) = active
                    .defer_once
                    .take()
                    .expect("deferred candidate was present");
                warning = Some(format!(
                    "{reason}; using the previous generation for this request"
                ));
            } else if id != runtime_identity(&active.files) && active.pending.is_none() {
                let prepared = materialize(&files)
                    .and_then(|path| validate_candidate(&path, cancel).map(|()| path));
                match prepared {
                    Ok(path) => {
                        active.pending = Some(files);
                        selected = Some(path);
                        trial = Some(RuntimeTrial {
                            source: source.clone(),
                            destination: destination.clone(),
                            python: python.clone(),
                            id,
                            outcome: None,
                        });
                    }
                    Err(e) if cancel.load(Ordering::Acquire) => return Err(e),
                    Err(e) => {
                        active.defer_once =
                            Some((id, "research candidate validation failed".into()));
                        warning = Some(e);
                    }
                }
            }
        }
        Ok(None) if runtime_identity(&active.files) != runtime_identity(&embedded_files()) => {
            warning = Some(
                "research generation manifest is missing; keeping the previous generation".into(),
            );
        }
        Ok(None) => {}
        Err(e) => {
            warning = Some(format!("{e}; keeping the previous research generation"));
        }
    }
    let path = match selected {
        Some(path) => path,
        None => materialize(&active.files)?,
    };
    Ok((path, warning, trial))
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
    let (runtime, runtime_warning, mut trial) = runtime(cancel)?;
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
    let result = (|| -> Result<Value, String> {
        let result =
            crate::agent::harness::output_timed_captured_cancellable(command, None, Some(cancel))
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
        let mut response: Value = serde_json::from_slice(&read(&spool.0.join("response.json"))?)
            .map_err(|e| e.to_string())?;
        if !response["advice"].is_object()
            || !response["changed"].is_boolean()
            || response["state"]["schema"] != "angel.sloptomizer-options/v1"
            || !response["state"]["population"].is_array()
            || !response["state"]["observations"].is_array()
            || response["state"].get("learner").is_none()
        {
            return Err("Sloptomizer returned an invalid state; original preserved".into());
        }
        if response["changed"] == true {
            write(
                &state_path,
                &serde_json::to_vec(&response["state"]).map_err(|e| e.to_string())?,
            )?;
        }
        if let Some(warning) = runtime_warning.as_ref() {
            response["advice"]["runtime_warning"] = json!(warning);
        }
        Ok(
            json!({"advice":response["advice"], "state_path":state_path, "updated":response["changed"], "runtime_generation":runtime.file_name().and_then(|v| v.to_str()), "runtime_warning":runtime_warning}),
        )
    })();
    if let Some(trial) = trial.as_mut() {
        if result.is_ok() {
            trial.outcome = Some(true);
        } else if !cancel.load(Ordering::Acquire) {
            trial.outcome = Some(false);
        }
    }
    result
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/rl_ctl__research_bridge__tests.rs"]
mod tests;
