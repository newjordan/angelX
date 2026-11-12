//! Executable systems: immutable generations, framed RPC, state migration and
//! one admission boundary. The host owns effects and the committed checkpoint.
//! Workers may compute on its copy; only a validated reply commits a new state.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

pub const API: u32 = 1;
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

pub mod owned;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api: u32,
    pub generation: String,
    pub request: u64,
    pub method: String,
    pub body: Value,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub api: u32,
    pub generation: String,
    pub request: u64,
    pub result: Result<Value, String>,
}

/// A pure computation on explicit inputs, served by an already active system.
/// It cannot migrate state or admit a waiting candidate: those require apply's
/// checkpoint/validation boundary. Contract versions are advertised at prepare.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compute {
    pub contract: String,
    pub version: u32,
    pub input: Value,
}

fn read_bytes(r: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut size = [0; 4];
    r.read_exact(&mut size)
        .map_err(|e| format!("system frame header: {e}"))?;
    let size = u32::from_be_bytes(size) as usize;
    if size > MAX_FRAME {
        return Err(format!("system frame exceeds {MAX_FRAME} bytes"));
    }
    let mut bytes = vec![0; size];
    r.read_exact(&mut bytes)
        .map_err(|e| format!("system frame body: {e}"))?;
    Ok(bytes)
}
fn write_bytes(w: &mut impl Write, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_FRAME {
        return Err(format!("system frame exceeds {MAX_FRAME} bytes"));
    }
    w.write_all(&(bytes.len() as u32).to_be_bytes())
        .and_then(|_| w.write_all(bytes))
        .and_then(|_| w.flush())
        .map_err(|e| format!("system frame write: {e}"))
}
pub fn read_frame<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> Result<T, String> {
    serde_json::from_slice(&read_bytes(r)?).map_err(|e| format!("system frame JSON: {e}"))
}
pub fn write_frame(w: &mut impl Write, body: &impl Serialize) -> Result<(), String> {
    write_bytes(w, &serde_json::to_vec(body).map_err(|e| e.to_string())?)
}
/// The frames of a worker that advertised `"codec": "bj1"` (see `crate::bj`).
fn read_bj<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> Result<T, String> {
    let bytes = read_bytes(r)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("system frame text: {e}"))?;
    serde_json::from_value(crate::bj::from_str(text)?).map_err(|e| format!("system frame: {e}"))
}
fn write_bj(w: &mut impl Write, body: &impl Serialize) -> Result<(), String> {
    let value = serde_json::to_value(body).map_err(|e| e.to_string())?;
    write_bytes(w, crate::bj::to_string(&value)?.as_bytes())
}

/// Worker entry point. stdout belongs exclusively to this protocol; diagnostics
/// go to stderr. Restore must be side effect free; effects are host-owned.
pub fn serve(mut dispatch: impl FnMut(&str, Value) -> Result<Value, String>) -> Result<(), String> {
    let (mut input, mut output) = (std::io::stdin().lock(), std::io::stdout().lock());
    loop {
        let request: Request = read_frame(&mut input)?;
        let result = if request.api != API {
            Err("unsupported system API".into())
        } else {
            dispatch(&request.method, request.body)
        };
        let stop = request.method == "dispose";
        write_frame(
            &mut output,
            &Reply {
                api: API,
                generation: request.generation,
                request: request.request,
                result,
            },
        )?;
        if stop {
            return Ok(());
        }
    }
}

type Exchange = (Request, bool, mpsc::SyncSender<Result<Reply, String>>);

const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
static CAPTURE_SERIAL: AtomicU64 = AtomicU64::new(0);

/// Owns one worker's immutable executable bytes. The path is unique to this
/// capture, even when another capture has identical content.
struct CapturedArtifact {
    path: PathBuf,
    digest: String,
    _directory: Arc<ArtifactDirectory>,
}

impl Drop for CapturedArtifact {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Shared so a retained artifact lease can outlive its System owner safely.
struct ArtifactDirectory {
    path: PathBuf,
}

impl Drop for ArtifactDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Worker {
    child: Child,
    io: Option<JoinHandle<()>>,
    queue: Option<mpsc::SyncSender<Exchange>>,
    digest: String,
    serial: u64,
    timeout: Duration,
    artifact: Arc<CapturedArtifact>,
    failed: bool,
    terminated: bool,
    contracts: BTreeMap<String, u32>,
    bj: bool,
    resident_worker: bool,
    resident: Option<Value>,
}
impl Worker {
    fn start(
        artifact: Arc<CapturedArtifact>,
        timeout: Duration,
        id: &str,
        required: &BTreeMap<String, u32>,
    ) -> Result<Self, String> {
        let mut command = Command::new(&artifact.path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut attempts = 0;
        let mut child = loop {
            match command.spawn() {
                Ok(child) => break child,
                // Some overlay filesystems briefly retain a writable reference
                // after close. Retry only launch (no request has been admitted).
                Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) && attempts < 5 => {
                    attempts += 1;
                    thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(format!("spawn {}: {e}", artifact.path.display())),
            }
        };
        let (mut input, mut output) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let (queue, incoming) = mpsc::sync_channel::<Exchange>(1);
        let io = thread::Builder::new()
            .name(format!("system-{id}-rpc"))
            .spawn(move || {
                while let Ok((request, bj, reply)) = incoming.recv() {
                    let result = if bj {
                        write_bj(&mut input, &request).and_then(|_| read_bj(&mut output))
                    } else {
                        write_frame(&mut input, &request).and_then(|_| read_frame(&mut output))
                    };
                    let failed = result.is_err();
                    let _ = reply.send(result);
                    if failed {
                        break;
                    }
                }
            })
            .map_err(|e| {
                let _ = child.kill();
                let _ = child.wait();
                e.to_string()
            })?;
        let mut worker = Self {
            child,
            io: Some(io),
            queue: Some(queue),
            digest: artifact.digest.clone(),
            serial: 0,
            timeout,
            artifact,
            failed: false,
            terminated: false,
            contracts: BTreeMap::new(),
            bj: false,
            resident_worker: false,
            resident: None,
        };
        let hello = worker.call("prepare", serde_json::json!({"id": id, "api": API}))?;
        if hello.get("id").and_then(Value::as_str) != Some(id)
            || hello.get("api").and_then(Value::as_u64) != Some(API as u64)
        {
            return Err("candidate has an incompatible system identity or API".into());
        }
        match hello.get("codec").and_then(Value::as_str) {
            None => (),
            Some("bj1") => worker.bj = true,
            Some(other) => return Err(format!("candidate needs unsupported codec {other}")),
        }
        worker.resident_worker = hello.get("resident").and_then(Value::as_bool) == Some(true);
        worker.contracts = serde_json::from_value(
            hello
                .get("contracts")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        )
        .map_err(|e| format!("invalid system contracts: {e}"))?;
        for (contract, version) in required {
            if worker.contracts.get(contract) != Some(version) {
                return Err(format!(
                    "candidate requires {contract} contract version {version}"
                ));
            }
        }
        Ok(worker)
    }
    fn call(&mut self, method: &str, body: Value) -> Result<Value, String> {
        if self.failed {
            return Err("system worker failed; publish a replacement to recover".into());
        }
        self.serial += 1;
        let (tx, rx) = mpsc::sync_channel(1);
        let request = Request {
            api: API,
            generation: self.digest.clone(),
            request: self.serial,
            method: method.into(),
            body,
        };
        self.queue
            .as_ref()
            .ok_or("worker retired")?
            .send((request, self.bj, tx))
            .map_err(|_| "worker RPC stopped")?;
        let reply = match rx.recv_timeout(self.timeout) {
            Ok(Ok(reply)) => reply,
            other => {
                self.terminate();
                return Err(format!("system {method} deadline/transport: {other:?}"));
            }
        };
        if reply.api != API || reply.generation != self.digest || reply.request != self.serial {
            self.terminate();
            return Err("stale or mismatched system reply".into());
        }
        reply.result
    }
    /// One `apply`. A resident worker is sent what changed since the state it
    /// last answered and answers with ops on the state it was sent; any error
    /// leaves `resident` empty, so the next call re-seeds the worker first.
    ///
    /// With `keep` the state comes back to the caller, so a resident worker's
    /// copy is a clone of it; without, the state becomes that copy and nothing
    /// is cloned (the host's step reads no state back, and a state is
    /// hundreds of kilobytes a tick).
    fn step(
        &mut self,
        mut state: Value,
        event: Value,
        validate: impl Fn(&Value) -> Result<(), String>,
        keep: bool,
    ) -> Result<(Option<Value>, Value), String> {
        let resident = self.resident_worker;
        let (key, sent) = if resident {
            let held = match self.resident.take() {
                Some(held) => held,
                None => self.call("restore", state.clone())?,
            };
            ("delta", Value::Array(crate::delta::diff(&held, &state)))
        } else {
            ("state", std::mem::take(&mut state))
        };
        let mut reply = self.call(
            "apply",
            Value::Object(serde_json::Map::from_iter([
                (key.into(), sent),
                ("event".into(), event),
            ])),
        )?;
        if resident {
            let ops = reply.get("delta").and_then(Value::as_array);
            crate::delta::patch(&mut state, ops.ok_or("system reply missing delta")?)?;
        } else {
            state = reply
                .get_mut("state")
                .map(Value::take)
                .ok_or("system reply missing state")?;
        }
        validate(&state)?;
        let output = reply
            .get_mut("output")
            .map(Value::take)
            .unwrap_or(Value::Null);
        if resident && !keep {
            self.resident = Some(state);
            return Ok((None, output));
        }
        if resident {
            self.resident = Some(state.clone());
        }
        Ok((keep.then_some(state), output))
    }
    fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        self.failed = true;
        // Kill before reaping: afterwards the PID could belong to another
        // process group. Termination is idempotent across timeout and Drop.
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.queue.take();
        if let Some(io) = self.io.take() {
            let _ = io.join();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub active: Option<String>,
    pub swaps: u64,
    pub rejected: u64,
    pub error: Option<String>,
    pub worker_pid: Option<u32>,
}
struct Control {
    stop: Mutex<bool>,
    wake: Condvar,
    status: Mutex<Status>,
}

/// Calls require exclusive access, so handoff happens only between completed
/// requests. Candidate copying, process launch and prepare run off that path.
pub struct System {
    active: Option<Worker>,
    pending: Arc<Mutex<Option<Worker>>>,
    control: Arc<Control>,
    watcher: Option<JoinHandle<()>>,
    _directory: Arc<ArtifactDirectory>,
    record: Option<Record>,
}
impl System {
    pub fn watch(id: &str, artifact: PathBuf, timeout: Duration) -> Result<Self, String> {
        Self::watch_with_contracts(id, artifact, timeout, BTreeMap::new())
    }

    /// Required computation contracts are checked before a candidate can retire
    /// the working process. Legacy workers remain usable by hosts requiring none.
    pub fn watch_with_contracts(
        id: &str,
        artifact: PathBuf,
        timeout: Duration,
        required: BTreeMap<String, u32>,
    ) -> Result<Self, String> {
        static DIRECTORY_SERIAL: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let serial = DIRECTORY_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "angel-system-{}-{nonce}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let directory = Arc::new(ArtifactDirectory { path: directory });
        let pending = Arc::new(Mutex::new(None));
        let waiting = Arc::clone(&pending);
        let control = Arc::new(Control {
            stop: Mutex::new(false),
            wake: Condvar::new(),
            status: Mutex::new(Status::default()),
        });
        let record = std::env::var_os("ANGEL_SYSTEM_RECORD")
            .filter(|dir| !dir.is_empty())
            .map(|dir| Record::new(dir.into(), id));
        let (dir, ctl, id) = (Arc::clone(&directory), control.clone(), id.to_owned());
        let watcher = thread::Builder::new()
            .name(format!("system-{id}-watch"))
            .spawn(move || {
                let (mut seen, mut accepted) = (None, String::new());
                loop {
                    let stamp = fs::metadata(&artifact)
                        .ok()
                        .and_then(|m| m.modified().ok().map(|t| (m.len(), t)));
                    if Some(stamp) != seen {
                        seen = Some(stamp);
                        let result = capture(&artifact, &dir).and_then(|captured| {
                            if captured.digest == accepted {
                                return Ok(None);
                            }
                            let digest = captured.digest.clone();
                            let worker = Worker::start(captured, timeout, &id, &required)?;
                            accepted = digest;
                            Ok(Some(worker))
                        });
                        match result {
                            Ok(Some(worker)) => {
                                // Coalesce rapid builds to the newest prepared image.
                                // Retirement stays on the watcher, off the tick path.
                                let previous = waiting.lock().unwrap().replace(worker);
                                drop(previous);
                            }
                            Ok(None) => (),
                            Err(e) => {
                                let mut status = ctl.status.lock().unwrap();
                                status.rejected += 1;
                                status.error = Some(e);
                            }
                        }
                    }
                    let stop = ctl.stop.lock().unwrap();
                    if *stop {
                        break;
                    }
                    let (stop, _) = ctl
                        .wake
                        .wait_timeout(stop, Duration::from_millis(300))
                        .unwrap();
                    if *stop {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            active: None,
            pending,
            control,
            watcher: Some(watcher),
            _directory: directory,
            record,
        })
    }
    pub fn status(&self) -> Status {
        self.control.status.lock().unwrap().clone()
    }

    /// Runs only on the active generation, between state requests. Explicit
    /// inputs/results avoid a full checkpoint for pure policy evaluation. The
    /// caller validates output before committing any externally owned effect.
    pub fn compute(
        &mut self,
        contract: &str,
        version: u32,
        input: Value,
        validate: impl Fn(&Value) -> Result<(), String>,
    ) -> Result<Value, String> {
        let result = (|| {
            let worker = self
                .active
                .as_mut()
                .ok_or("system is preparing its first implementation")?;
            if worker.contracts.get(contract) != Some(&version) {
                return Err(format!(
                    "active system does not support {contract} version {version}"
                ));
            }
            let result = worker.call(
                "compute",
                serde_json::to_value(Compute {
                    contract: contract.into(),
                    version,
                    input,
                })
                .map_err(|e| e.to_string())?,
            )?;
            validate(&result)?;
            Ok(result)
        })();
        if let Err(error) = &result {
            self.control.status.lock().unwrap().error = Some(error.clone());
        }
        result
    }

    /// Returns the next checkpoint and host-directed output. `validate` admits
    /// both migrated and computed state before ownership changes. On any error,
    /// the caller's checkpoint remains untouched; requests are never replayed.
    /// A resident worker is sent only the delta from the state it last answered
    /// (`Worker::step`); callers still pass and receive whole states.
    pub fn apply(
        &mut self,
        state: Value,
        event: Value,
        validate: impl Fn(&Value) -> Result<(), String>,
    ) -> Result<(Value, Value), String> {
        self.apply_keeping(state, event, validate, true)
            .map(|(state, output)| (state.unwrap_or(Value::Null), output))
    }

    /// `apply` for a caller that has what it needs of the state once `validate`
    /// has seen it, and wants only the output: a resident worker's copy of the
    /// state is the state itself, not a clone of it.
    pub fn apply_discarding(
        &mut self,
        state: Value,
        event: Value,
        validate: impl Fn(&Value) -> Result<(), String>,
    ) -> Result<Value, String> {
        self.apply_keeping(state, event, validate, false)
            .map(|(_, output)| output)
    }

    fn apply_keeping(
        &mut self,
        mut state: Value,
        event: Value,
        validate: impl Fn(&Value) -> Result<(), String>,
        keep: bool,
    ) -> Result<(Option<Value>, Value), String> {
        // A recording reads the state after the step.
        let keep = keep || self.record.is_some();
        let tape = self
            .record
            .is_some()
            .then(|| (state.clone(), event.clone()));
        let candidate = self.pending.lock().unwrap().take();
        if let Some(mut candidate) = candidate {
            let staged = candidate.call("restore", state.clone()).and_then(|s| {
                validate(&s)?;
                Ok(s)
            });
            match staged.and_then(|s| candidate.call("activate", Value::Null).map(|_| s)) {
                Ok(s) => {
                    candidate.resident = candidate.resident_worker.then(|| s.clone());
                    state = s;
                    let old = self.active.replace(candidate);
                    let mut status = self.control.status.lock().unwrap();
                    status.active = self.active.as_ref().map(|w| w.digest.clone());
                    status.worker_pid = self.active.as_ref().map(|w| w.child.id());
                    status.swaps += 1;
                    status.error = None;
                    drop(status);
                    if let Some(mut old) = old {
                        old.timeout = Duration::from_millis(50);
                        let _ = old.call("dispose", Value::Null);
                    }
                }
                Err(e) => {
                    let mut status = self.control.status.lock().unwrap();
                    status.rejected += 1;
                    status.error = Some(format!("candidate rejected: {e}"));
                }
            }
        }
        let worker = self
            .active
            .as_mut()
            .ok_or("system is preparing its first implementation")?;
        let result = worker.step(state, event, validate, keep);
        if let (Ok((Some(after), _)), Some((record, (before, event)))) =
            (&result, self.record.as_mut().zip(tape))
        {
            record.push(&before, &event, after);
        }
        if let Err(e) = &result {
            self.control.status.lock().unwrap().error = Some(e.clone());
        }
        result
    }
}
impl Drop for System {
    fn drop(&mut self) {
        *self.control.stop.lock().unwrap() = true;
        self.control.wake.notify_all();
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
        self.active.take();
        self.pending.lock().unwrap().take();
        // The directory is removed by its shared owner only after the last
        // retained artifact lease has removed its own file.
    }
}

/// `ANGEL_SYSTEM_RECORD=<dir>`: one JSONL line per successful apply, in
/// `<dir>/<id>-<pid>-<unix secs>.jsonl`, for replaying a worker offline. The
/// first line is the `restore` state; an apply whose `before` is the previous
/// line's state writes null for it. Failing to write is logged once, never fatal.
struct Record {
    dir: PathBuf,
    id: String,
    file: Option<fs::File>,
    last: Option<Value>,
    failed: bool,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Line<'a> {
    Restore {
        state: &'a Value,
    },
    Apply {
        before: Option<&'a Value>,
        event: &'a Value,
        after: &'a Value,
    },
}
impl Record {
    fn new(dir: PathBuf, id: &str) -> Self {
        Self {
            dir,
            id: id.into(),
            file: None,
            last: None,
            failed: false,
        }
    }
    fn open(&self) -> std::io::Result<fs::File> {
        fs::create_dir_all(&self.dir)?;
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |t| t.as_secs());
        let stem = format!("{}-{}-{secs}", self.id, std::process::id());
        // Two systems of one id in one second must not share a file.
        for n in 0.. {
            let name = if n == 0 {
                format!("{stem}.jsonl")
            } else {
                format!("{stem}-{n}.jsonl")
            };
            match fs::OpenOptions::new()
                .append(true)
                .create_new(true)
                .open(self.dir.join(name))
            {
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                opened => return opened,
            }
        }
        unreachable!()
    }
    fn write(&mut self, line: &Line) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(line).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        let file = match self.file.as_mut() {
            Some(file) => file,
            None => self.file.insert(self.open().map_err(|e| e.to_string())?),
        };
        file.write_all(&bytes).map_err(|e| e.to_string())
    }
    fn push(&mut self, before: &Value, event: &Value, after: &Value) {
        if self.failed {
            return;
        }
        let first = self.last.is_none();
        let wrote = (|| {
            if first {
                self.write(&Line::Restore { state: before })?;
                self.last = Some(before.clone());
            }
            let same = self.last.as_ref() == Some(before);
            self.write(&Line::Apply {
                before: (!same).then_some(before),
                event,
                after,
            })
        })();
        self.last = Some(after.clone());
        if let Err(e) = wrote {
            self.failed = true;
            eprintln!("angel-live: system {} record disabled: {e}", self.id);
        }
    }
}

fn capture(
    source: &Path,
    directory: &Arc<ArtifactDirectory>,
) -> Result<Arc<CapturedArtifact>, String> {
    let mut input = fs::File::open(source).map_err(|e| format!("{}: {e}", source.display()))?;
    let before = input.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() || before.len() > MAX_ARTIFACT_BYTES {
        return Err("system artifact must be a regular executable under 512 MiB".into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut input)
        .take(MAX_ARTIFACT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err("system artifact must be a regular executable under 512 MiB".into());
    }
    let after = input.metadata().map_err(|e| e.to_string())?;
    let current = fs::metadata(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || before.len() != bytes.len() as u64
        || before.len() != current.len()
        || before.modified().ok() != current.modified().ok()
    {
        return Err("system artifact changed during capture; publish by atomic rename".into());
    }
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    let (path, mut output) = loop {
        let serial = CAPTURE_SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = directory.path.join(format!("{digest}-{serial:016x}"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(output) => break (path, output),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("capture {}: {error}", path.display())),
        }
    };
    let write = (|| {
        output.write_all(&bytes).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            output
                .set_permissions(fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())
    })();
    drop(output);
    if let Err(error) = write {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(Arc::new(CapturedArtifact {
        path,
        digest,
        _directory: Arc::clone(directory),
    }))
}

#[cfg(all(test, unix))]
mod artifact_lease_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        directory: Arc<ArtifactDirectory>,
    }

    impl Fixture {
        fn new() -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "angel-system-lease-test-{}-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            let directory_path = root.join("captured");
            fs::create_dir(&directory_path).unwrap();
            Self {
                source: root.join("worker"),
                root,
                directory: Arc::new(ArtifactDirectory {
                    path: directory_path,
                }),
            }
        }

        fn publish(&self, source: &str) {
            fs::write(&self.source, source).unwrap();
            fs::set_permissions(&self.source, fs::Permissions::from_mode(0o700)).unwrap();
        }

        fn capture(&self) -> Arc<CapturedArtifact> {
            capture(&self.source, &self.directory).unwrap()
        }

        fn start(&self, artifact: Arc<CapturedArtifact>) -> Worker {
            Worker::start(
                artifact,
                Duration::from_secs(2),
                "lease-test",
                &BTreeMap::new(),
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    const ECHO_WORKER: &str = r##"#!/usr/bin/env python3
import json, struct, sys
while True:
    header = sys.stdin.buffer.read(4)
    if not header:
        break
    size, = struct.unpack('>I', header)
    request = json.loads(sys.stdin.buffer.read(size))
    if request['method'] == 'prepare':
        result = {'id': request['body']['id'], 'api': 1, 'contracts': {}}
    else:
        result = None
    reply = json.dumps({'api': 1, 'generation': request['generation'],
                        'request': request['request'], 'result': {'Ok': result}}).encode()
    sys.stdout.buffer.write(struct.pack('>I', len(reply)) + reply)
    sys.stdout.buffer.flush()
    if request['method'] == 'dispose':
        break
"##;

    #[test]
    fn same_digest_captures_have_independent_unique_leases() {
        let fixture = Fixture::new();
        fixture.publish(ECHO_WORKER);
        let first = fixture.capture();
        let second = fixture.capture();
        assert_eq!(first.digest, second.digest);
        assert_ne!(first.path, second.path);
        let first_path = first.path.clone();
        let second_path = second.path.clone();
        drop(first);
        assert!(!first_path.exists());
        assert!(second_path.exists());
        drop(second);
        assert!(!second_path.exists());
    }

    #[test]
    fn worker_termination_keeps_the_image_leased_for_restart() {
        let fixture = Fixture::new();
        fixture.publish(ECHO_WORKER);
        let artifact = fixture.capture();
        let path = artifact.path.clone();
        let mut first = fixture.start(Arc::clone(&artifact));
        first.terminate();
        assert!(matches!(first.child.try_wait(), Ok(Some(_))));
        assert!(path.exists());
        drop(first);
        assert!(path.exists());

        let mut restarted = fixture.start(Arc::clone(&artifact));
        assert_eq!(restarted.artifact.path, path);
        assert_eq!(restarted.digest, artifact.digest);
        restarted.terminate();
        assert!(matches!(restarted.child.try_wait(), Ok(Some(_))));
        drop(restarted);
        assert!(path.exists());
        drop(artifact);
        assert!(!path.exists());
    }

    #[test]
    fn failed_spawn_and_prepare_release_the_last_artifact_lease() {
        let spawn_fixture = Fixture::new();
        spawn_fixture.publish("not a valid executable image");
        let spawn_lease = spawn_fixture.capture();
        let spawn_path = spawn_lease.path.clone();
        assert!(
            Worker::start(
                spawn_lease,
                Duration::from_millis(100),
                "bad-spawn",
                &BTreeMap::new(),
            )
            .is_err()
        );
        assert!(!spawn_path.exists());

        let prepare_fixture = Fixture::new();
        prepare_fixture.publish("#!/bin/sh\nsleep 10\n");
        let prepare_lease = prepare_fixture.capture();
        let prepare_path = prepare_lease.path.clone();
        assert!(
            Worker::start(
                prepare_lease,
                Duration::from_millis(50),
                "bad-prepare",
                &BTreeMap::new(),
            )
            .is_err()
        );
        assert!(!prepare_path.exists());
    }

    #[test]
    fn replacing_a_coalesced_candidate_reaps_pid_and_releases_its_artifact() {
        let fixture = Fixture::new();
        fixture.publish(ECHO_WORKER);
        let old = fixture.start(fixture.capture());
        let old_path = old.artifact.path.clone();
        let old_pid = old.child.id();
        let mut pending = Some(old);

        fixture.publish(&format!("{ECHO_WORKER}\n# second immutable capture\n"));
        let newer = fixture.start(fixture.capture());
        let newer_path = newer.artifact.path.clone();
        let previous = pending.replace(newer);
        drop(previous);
        assert!(!old_path.exists());
        assert!(newer_path.exists());
        assert!(matches!(unsafe { libc::kill(old_pid as i32, 0) }, -1));

        let retired = pending.take().unwrap();
        let retired_pid = retired.child.id();
        drop(retired);
        assert!(!newer_path.exists());
        assert!(matches!(unsafe { libc::kill(retired_pid as i32, 0) }, -1));
    }
}
