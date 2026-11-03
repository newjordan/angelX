//! Opt-in worker-owned session supervision. The legacy `System` API is left
//! unchanged; this wrapper admits validated session frames and retains a
//! bounded replay tail against the exact executable image that accepted them.
use super::{CapturedArtifact, System, Worker};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub const CONTRACT: &str = "kernel.owned-state";
pub const CONTRACT_VERSION: u32 = 1;
const MAX_TAIL_EVENTS: usize = 512;
const MAX_TAIL_BYTES: usize = 8 * 1024 * 1024;
const MAX_WITNESS_BYTES: usize = 16 * 1024;

/// Hash a caller-selected semantic reply without buffering its JSON encoding.
/// Callers should serialize a typed ReplayFrame projection (identity, cursor,
/// and reply), not a view dictionary. Large retained definitions therefore
/// have a fixed 32-byte recovery witness.
pub fn semantic_witness(value: &impl serde::Serialize) -> Result<Vec<u8>, String> {
    struct HashWriter(ring::digest::Context);
    impl Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = HashWriter(ring::digest::Context::new(&ring::digest::SHA256));
    serde_json::to_writer(&mut writer, value).map_err(|error| error.to_string())?;
    Ok(writer.0.finish().as_ref().to_vec())
}

fn guarded<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .unwrap_or_else(|_| Err("owned session validator or worker operation panicked".into()))
}

// Abort is documented as infallible, but must not prevent retiring a worker
// whose reply left host staging uncertain if an implementation panics anyway.
fn abort_safely(validator: &mut impl Validator) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| validator.abort()));
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub events: usize,
    pub bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            events: MAX_TAIL_EVENTS,
            bytes: MAX_TAIL_BYTES,
        }
    }
}

/// Caller-owned view transaction. `stage_event` sees only accepted live
/// SessionFrames. Replay calls `validate_replay` and compares semantic witnesses
/// without producing ordinary view deltas. `stage_baseline` receives the
/// worker's baseline view Frame. Any failed staged operation calls `abort`
/// before commit; `commit` must be an infallible, effects-free view swap.
/// `abort` is also required to be infallible and must discard only staged state.
pub trait Validator {
    fn validate_checkpoint(&mut self, checkpoint: &Value) -> Result<(), String>;
    fn validate_handoff(&mut self, prior: &Value, next: &Value) -> Result<(), String>;
    fn validate_bootstrap(&mut self, initial: &Value, post_baseline: &Value) -> Result<(), String>;
    fn stage_baseline(&mut self, frame: &Value) -> Result<(), String>;
    fn stage_event(&mut self, event: &Value, frame: &Value) -> Result<Vec<u8>, String>;
    fn validate_replay(
        &mut self,
        event: &Value,
        frame: &Value,
        witness: &[u8],
    ) -> Result<(), String>;
    fn validate_recovered(&mut self, checkpoint: &Value) -> Result<(), String>;
    fn commit(&mut self);
    fn abort(&mut self);
}

#[derive(Clone, Debug)]
struct Accepted {
    event: Value,
    witness: Vec<u8>,
}

/// One owned rules session, supervised by the existing watcher and process
/// machinery. It stores a checkpoint and bounded accepted inputs, never a
/// mutable Run or second effect ledger.
pub struct OwnedSystem {
    inner: System,
    id: String,
    timeout: Duration,
    required: BTreeMap<String, u32>,
    limits: Limits,
    checkpoint: Option<Value>,
    tail: Vec<Accepted>,
    tail_bytes: usize,
    pending_event: Option<Value>,
    pin: Option<Arc<CapturedArtifact>>,
}

impl OwnedSystem {
    pub fn watch(
        id: &str,
        artifact: PathBuf,
        timeout: Duration,
        mut required: BTreeMap<String, u32>,
        limits: Limits,
    ) -> Result<Self, String> {
        required.insert(CONTRACT.to_owned(), CONTRACT_VERSION);
        if limits.events == 0
            || limits.events > MAX_TAIL_EVENTS
            || limits.bytes == 0
            || limits.bytes > MAX_TAIL_BYTES
        {
            return Err("owned session limits exceed the bounded recovery-tail policy".into());
        }
        let inner = System::watch_with_contracts(id, artifact, timeout, required.clone())?;
        Ok(Self {
            inner,
            id: id.to_owned(),
            timeout,
            required,
            limits,
            checkpoint: None,
            tail: Vec::new(),
            tail_bytes: 0,
            pending_event: None,
            pin: None,
        })
    }

    pub fn status(&self) -> super::Status {
        self.inner.status()
    }
    pub fn pending_event(&self) -> Option<&Value> {
        self.pending_event.as_ref()
    }

    /// Restore, publish the worker's real baseline view, privately activate,
    /// then retain the post-baseline checkpoint as the recovery seed.
    pub fn initialize(
        &mut self,
        initial: Value,
        validator: &mut impl Validator,
    ) -> Result<Value, String> {
        if self.checkpoint.is_some() {
            return Err("owned session is already initialized".into());
        }
        let mut candidate = self.take_candidate()?;
        let result = guarded(|| {
            let restored = candidate.call("owned.restore", initial.clone())?;
            validator.validate_checkpoint(&restored)?;
            validator.validate_handoff(&initial, &restored)?;
            let baseline = candidate.call("owned.baseline", Value::Null)?;
            candidate.call("owned.activate", Value::Null)?;
            let checkpoint = candidate.call("owned.checkpoint", Value::Null)?;
            validator.validate_checkpoint(&checkpoint)?;
            validator.validate_bootstrap(&initial, &checkpoint)?;
            validator.stage_baseline(&baseline)?;
            validator.commit();
            Ok((baseline, checkpoint))
        });
        match result {
            Ok((baseline, checkpoint)) => {
                self.pin = Some(Arc::clone(&candidate.artifact));
                self.checkpoint = Some(checkpoint);
                self.install(candidate);
                Ok(baseline)
            }
            Err(e) => {
                if self.pin.is_none() {
                    self.pin = Some(Arc::clone(&candidate.artifact));
                }
                candidate.terminate();
                abort_safely(validator);
                Err(e)
            }
        }
    }

    /// Admit one event. A pending uncertain event permits only exact retry.
    /// Tail capacity is checkpointed before any new event RPC is issued.
    pub fn event(&mut self, event: Value, validator: &mut impl Validator) -> Result<Value, String> {
        let encoded = serde_json::to_vec(&event).map_err(|e| e.to_string())?.len();
        if encoded.saturating_add(MAX_WITNESS_BYTES) > self.limits.bytes {
            return Err("owned event exceeds recovery-tail capacity".into());
        }
        if self.checkpoint.is_none() {
            return Err("owned session is not initialized".into());
        }
        let retrying = match &self.pending_event {
            Some(pending) if pending == &event => true,
            Some(_) => return Err("a different owned event is awaiting exact retry".into()),
            None => false,
        };
        if !retrying {
            // A candidate can be accepted before a new input exists. If its
            // active-checkpoint RPC failed, recover the pinned owner before use.
            let refresh_result = self.refresh(validator);
            if refresh_result.is_err() && (self.active_failed() || self.inner.active.is_none()) {
                self.retire_active();
                self.recover_pinned(validator)?;
            }
        }
        if self.active_failed() || self.inner.active.is_none() {
            self.retire_active();
            self.recover_pinned(validator)?;
        }
        if !retrying
            && (self.tail.len() >= self.limits.events
                || self.tail_bytes.saturating_add(encoded + MAX_WITNESS_BYTES) > self.limits.bytes)
        {
            if let Err(error) = self.checkpoint_now(validator) {
                if self.inner.active.is_none() {
                    self.recover_pinned(validator)?;
                    self.checkpoint_now(validator)?;
                } else {
                    return Err(error);
                }
            }
        }
        if !retrying {
            self.pending_event = Some(event.clone());
        }
        let result = guarded(|| self.dispatch_event(&event, validator));
        if result.is_err() {
            self.retire_active();
            abort_safely(validator);
            // Preserve the exact pending input and recover only accepted events.
            let _ = self.recover_pinned(validator);
        }
        result
    }

    fn dispatch_event(
        &mut self,
        event: &Value,
        validator: &mut impl Validator,
    ) -> Result<Value, String> {
        // Allocate bounded tail storage before the mutating RPC; after commit,
        // pushing the accepted event cannot allocate.
        self.tail.try_reserve(1).map_err(|e| e.to_string())?;
        let retained_event = event.clone();
        let event_bytes = serde_json::to_vec(event).map_err(|e| e.to_string())?.len();
        let worker = self
            .inner
            .active
            .as_mut()
            .ok_or("owned worker unavailable")?;
        let frame = worker.call("owned.event", event.clone())?;
        let witness = match validator.stage_event(event, &frame) {
            Ok(witness) if witness.len() <= MAX_WITNESS_BYTES => witness,
            Ok(_) => return Err("semantic witness exceeds bound".into()),
            Err(error) => return Err(error),
        };
        let bytes = event_bytes.saturating_add(witness.len());
        if self.tail.len() >= self.limits.events
            || self.tail_bytes.saturating_add(bytes) > self.limits.bytes
        {
            return Err("owned recovery tail is full".into());
        }
        validator.commit();
        self.tail.push(Accepted {
            event: retained_event,
            witness,
        });
        self.tail_bytes += bytes;
        self.pending_event = None;
        Ok(frame)
    }

    /// Queries are read-only in the worker. Any transport or caller-validation
    /// uncertainty retires that process but leaves the accepted recovery tail.
    pub fn query(
        &mut self,
        query: Value,
        validate: impl FnOnce(&Value) -> Result<(), String>,
    ) -> Result<Value, String> {
        if self.pending_event.is_some() {
            return Err("cannot query while an exact owned event retry is pending".into());
        }
        let result = guarded(|| match self.inner.active.as_mut() {
            Some(worker) => worker.call("owned.query", query).and_then(|frame| {
                validate(&frame)?;
                Ok(frame)
            }),
            None => Err("owned worker unavailable".into()),
        });
        if result.is_err() {
            self.retire_active();
        }
        result
    }

    /// Restore the pinned executable after an uncertain RPC, replaying only
    /// the already accepted tail. A pending unaccepted input remains pending
    /// for its caller's exact retry and is never dispatched here.
    pub fn recover(&mut self, validator: &mut impl Validator) -> Result<(), String> {
        if self.checkpoint.is_none() {
            return Err("owned session is not initialized".into());
        }
        if self
            .inner
            .active
            .as_ref()
            .is_some_and(|worker| !worker.failed)
        {
            return Ok(());
        }
        self.retire_active();
        self.recover_pinned(validator)
    }

    /// Checkpointing advances the replay seed without changing the caller's
    /// already accepted view. The tail is cleared only after both validations.
    pub fn checkpoint_now(&mut self, validator: &mut impl Validator) -> Result<Value, String> {
        if self.pending_event.is_some() {
            return Err("cannot checkpoint while an exact owned event retry is pending".into());
        }
        if self.inner.active.is_none() || self.active_failed() {
            self.recover(validator)?;
        }
        let result = guarded(|| match self.inner.active.as_mut() {
            Some(worker) => worker.call("owned.checkpoint", Value::Null),
            None => Err("owned worker unavailable".into()),
        });
        let checkpoint = match result {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                self.retire_active();
                return Err(error);
            }
        };
        if let Err(error) = guarded(|| {
            validator
                .validate_checkpoint(&checkpoint)
                .and_then(|_| validator.validate_recovered(&checkpoint))
        }) {
            self.retire_active();
            abort_safely(validator);
            return Err(error);
        }
        self.checkpoint = Some(checkpoint.clone());
        self.tail.clear();
        self.tail_bytes = 0;
        Ok(checkpoint)
    }

    /// At a quiescent boundary, a new image restores the ACTIVE owner's current
    /// checkpoint. It gets baseline+activation but never replays old-policy tail.
    pub fn refresh(&mut self, validator: &mut impl Validator) -> Result<bool, String> {
        if self.pending_event.is_some() {
            return Err("cannot refresh while an exact owned event retry is pending".into());
        }
        if self.active_failed() || self.inner.active.is_none() {
            return Err("active owner must recover before image handoff".into());
        }
        let Some(mut candidate) = self.inner.pending.lock().unwrap().take() else {
            return Ok(false);
        };
        let current = guarded(|| match self.inner.active.as_mut() {
            Some(active) => active
                .call("owned.checkpoint", Value::Null)
                .and_then(|checkpoint| {
                    validator.validate_checkpoint(&checkpoint)?;
                    validator.validate_recovered(&checkpoint)?;
                    Ok(checkpoint)
                }),
            None => Err("no active owner to hand off".into()),
        });
        let current = match current {
            Ok(current) => current,
            Err(error) => {
                candidate.terminate();
                self.retire_active();
                abort_safely(validator);
                return Err(error);
            }
        };
        let result = guarded(|| {
            let restored = candidate.call("owned.restore", current.clone())?;
            validator.validate_checkpoint(&restored)?;
            validator.validate_handoff(&current, &restored)?;
            let baseline = candidate.call("owned.baseline", Value::Null)?;
            candidate.call("owned.activate", Value::Null)?;
            let candidate_checkpoint = candidate.call("owned.checkpoint", Value::Null)?;
            validator.validate_checkpoint(&candidate_checkpoint)?;
            validator.validate_handoff(&current, &candidate_checkpoint)?;
            validator.stage_baseline(&baseline)?;
            validator.commit();
            Ok(candidate_checkpoint)
        });
        match result {
            Ok(checkpoint) => {
                self.pin = Some(Arc::clone(&candidate.artifact));
                self.checkpoint = Some(checkpoint);
                self.tail.clear();
                self.tail_bytes = 0;
                self.install(candidate);
                Ok(true)
            }
            Err(error) => {
                candidate.terminate();
                if self.active_failed() {
                    self.retire_active();
                }
                abort_safely(validator);
                let mut status = self.inner.control.status.lock().unwrap();
                status.rejected = status.rejected.saturating_add(1);
                status.error = Some(format!("candidate rejected: {error}"));
                Err(error)
            }
        }
    }

    fn take_candidate(&mut self) -> Result<Worker, String> {
        if let Some(candidate) = self.inner.pending.lock().unwrap().take() {
            return Ok(candidate);
        }
        if self.checkpoint.is_none()
            && let Some(pin) = self.pin.as_ref().cloned()
        {
            return Worker::start(pin, self.timeout, &self.id, &self.required);
        }
        Err("system is preparing its first implementation".into())
    }
    fn install(&mut self, candidate: Worker) {
        let old = self.inner.active.replace(candidate);
        if let Some(mut old) = old {
            old.timeout = Duration::from_millis(50);
            let _ = old.call("dispose", Value::Null);
        }
        let mut status = self.inner.control.status.lock().unwrap();
        status.active = self
            .inner
            .active
            .as_ref()
            .map(|worker| worker.digest.clone());
        status.worker_pid = self.inner.active.as_ref().map(|worker| worker.child.id());
        status.swaps = status.swaps.saturating_add(1);
        status.error = None;
    }
    fn active_failed(&self) -> bool {
        self.inner
            .active
            .as_ref()
            .is_some_and(|worker| worker.failed)
    }
    fn retire_active(&mut self) {
        if let Some(mut worker) = self.inner.active.take() {
            worker.terminate();
        }
        let mut status = self.inner.control.status.lock().unwrap();
        status.worker_pid = None;
        status.error = Some("owned worker retired after uncertain RPC".into());
    }
    fn recover_pinned(&mut self, validator: &mut impl Validator) -> Result<(), String> {
        let pin = self
            .pin
            .as_ref()
            .ok_or("no pinned recovery artifact")?
            .clone();
        let mut worker = Worker::start(pin, self.timeout, &self.id, &self.required)?;
        let result = guarded(|| {
            let checkpoint = self
                .checkpoint
                .as_ref()
                .ok_or("missing recovery checkpoint")?
                .clone();
            let restored = worker.call("owned.restore", checkpoint.clone())?;
            validator.validate_checkpoint(&restored)?;
            validator.validate_handoff(&checkpoint, &restored)?;
            for accepted in &self.tail {
                let replay = worker.call("owned.replay", accepted.event.clone())?;
                validator.validate_replay(&accepted.event, &replay, &accepted.witness)?;
            }
            let baseline = worker.call("owned.baseline", Value::Null)?;
            worker.call("owned.activate", Value::Null)?;
            let actual = worker.call("owned.checkpoint", Value::Null)?;
            validator.validate_checkpoint(&actual)?;
            validator.validate_recovered(&actual)?;
            validator.stage_baseline(&baseline)?;
            validator.commit();
            Ok(())
        });
        match result {
            Ok(()) => {
                self.install(worker);
                Ok(())
            }
            Err(error) => {
                worker.terminate();
                abort_safely(validator);
                Err(error)
            }
        }
    }
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    #[cfg(target_os = "linux")]
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::Instant;

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct TestValidator {
        staged: Option<Value>,
        expected_replay: Option<Value>,
        diverge_replay: bool,
        reject_checkpoint: bool,
        reject_recovered: bool,
        reject_event: bool,
        panic_abort: bool,
    }
    impl Validator for TestValidator {
        fn validate_checkpoint(&mut self, cp: &Value) -> Result<(), String> {
            if self.reject_checkpoint {
                return Err("injected checkpoint validation fault".into());
            }
            if cp.get("n").and_then(Value::as_u64).is_some()
                && cp.get("rev").and_then(Value::as_u64).is_some()
            {
                Ok(())
            } else {
                Err("invalid fixture checkpoint".into())
            }
        }
        fn validate_handoff(&mut self, prior: &Value, next: &Value) -> Result<(), String> {
            if prior != next {
                return Err("fixture checkpoint differs".into());
            }
            self.expected_replay = Some(prior.clone());
            Ok(())
        }
        fn validate_bootstrap(&mut self, initial: &Value, post: &Value) -> Result<(), String> {
            let revision_ok = post["rev"].as_u64() == initial["rev"].as_u64().map(|rev| rev + 1);
            (initial["n"] == post["n"] && revision_ok)
                .then_some(())
                .ok_or_else(|| "fixture bootstrap mismatch".into())
        }
        fn stage_baseline(&mut self, frame: &Value) -> Result<(), String> {
            let n = frame
                .pointer("/bytes/0")
                .and_then(Value::as_u64)
                .ok_or("invalid baseline bytes")?;
            let rev = frame["revision"]
                .as_u64()
                .ok_or("invalid baseline revision")?;
            self.staged = Some(serde_json::json!({"n": n, "rev": rev}));
            self.expected_replay = self.staged.clone();
            Ok(())
        }
        fn stage_event(&mut self, _event: &Value, frame: &Value) -> Result<Vec<u8>, String> {
            if self.reject_event {
                return Err("injected event-frame validation failure".into());
            }
            let seq = frame["seq"]
                .as_u64()
                .ok_or("invalid SessionFrame sequence")?;
            let view = &frame["view"];
            let n = view
                .pointer("/bytes/0")
                .and_then(Value::as_u64)
                .ok_or("invalid SessionFrame view")?;
            let rev = view["revision"]
                .as_u64()
                .ok_or("invalid SessionFrame revision")?;
            if frame.pointer("/reply/type").and_then(Value::as_str) != Some("done") {
                return Err("invalid SessionFrame reply".into());
            }
            self.staged = Some(serde_json::json!({"n": n, "rev": rev}));
            self.expected_replay = self.staged.clone();
            let mut witness = seq.to_be_bytes().to_vec();
            witness.extend_from_slice(&rev.to_be_bytes());
            witness.extend_from_slice(b"done");
            Ok(witness)
        }
        fn validate_replay(
            &mut self,
            _event: &Value,
            frame: &Value,
            witness: &[u8],
        ) -> Result<(), String> {
            if self.diverge_replay {
                return Err("injected replay witness mismatch".into());
            }
            let seq = frame["seq"]
                .as_u64()
                .ok_or("invalid ReplayFrame sequence")?;
            let rev = frame["view_revision"]
                .as_u64()
                .ok_or("invalid ReplayFrame revision")?;
            if frame.pointer("/reply/type").and_then(Value::as_str) != Some("done") {
                return Err("invalid ReplayFrame reply".into());
            }
            let mut expected = seq.to_be_bytes().to_vec();
            expected.extend_from_slice(&rev.to_be_bytes());
            expected.extend_from_slice(b"done");
            if witness != expected.as_slice() {
                return Err("semantic witness mismatch".into());
            }
            let prior_n = self
                .expected_replay
                .as_ref()
                .and_then(|view| view["n"].as_u64())
                .ok_or("missing replay baseline")?;
            self.expected_replay = Some(serde_json::json!({"n": prior_n + 1, "rev": rev}));
            Ok(())
        }
        fn validate_recovered(&mut self, cp: &Value) -> Result<(), String> {
            if self.reject_recovered {
                return Err("injected stale accepted-view fault".into());
            }
            (self.expected_replay.as_ref() == Some(cp))
                .then_some(())
                .ok_or_else(|| "recovered fixture view mismatch".into())
        }
        fn commit(&mut self) {}
        fn abort(&mut self) {
            if self.panic_abort {
                panic!("injected abort panic");
            }
            self.staged = None;
            self.expected_replay = None;
        }
    }

    // Strict lifecycle: rejects the legacy methods and all invented session.* names.
    const STRICT_WORKER: &str = r##"#!/usr/bin/env python3
import json, os, struct, sys
REJECT_RESTORE = False
MALFORMED_CHECKPOINT = False
CRASH_BEFORE_SEQ = 0
CRASH_AFTER_SEQ = 0
state = {'n': 0, 'rev': 0}; staged = False; baseline = False; active = False
while True:
    header = sys.stdin.buffer.read(4)
    if not header: break
    size, = struct.unpack('>I', header)
    request = json.loads(sys.stdin.buffer.read(size))
    method, body = request['method'], request['body']
    if method == 'prepare':
        result = {'id': body['id'], 'api': 1, 'contracts': {'kernel.owned-state': 1}}
    elif method == 'owned.restore' and not active:
        if REJECT_RESTORE and body.get('n', 0) > 0: result = {'Err': 'candidate restore refused'}
        else: state = body; staged = True; baseline = False; result = state
    elif method == 'owned.baseline' and staged and not baseline:
        if body is not None: raise RuntimeError('baseline takes null')
        if state['rev'] == 0: state = {'n': state['n'], 'rev': 1}
        baseline = True
        result = {'api': 1, 'kind': 'baseline', 'session': 'fixture', 'raid': 0,
                  'base_revision': state['rev'] - 1, 'revision': state['rev'], 'bytes': [state['n']]}
    elif method == 'owned.activate' and staged and baseline:
        if body is not None: raise RuntimeError('activate takes null')
        active = True; staged = False; result = None
    elif method == 'owned.checkpoint' and active:
        if body is not None: raise RuntimeError('checkpoint takes null')
        result = {'wrong': 'checkpoint'} if MALFORMED_CHECKPOINT else state
    elif method == 'owned.event' and active:
        seq = body['seq']
        if seq == CRASH_BEFORE_SEQ and not os.path.exists(__file__ + '.before'):
            open(__file__ + '.before', 'w').close(); os._exit(18)
        if seq == CRASH_AFTER_SEQ and not os.path.exists(__file__ + '.after'):
            state = {'n': state['n'] + 1, 'rev': state['rev'] + 1}
            open(__file__ + '.after', 'w').close(); os._exit(17)
        state = {'n': state['n'] + 1, 'rev': state['rev'] + 1}
        result = {'api': 1, 'session': 'fixture', 'raid': 0, 'seq': seq,
                  'view': {'api': 1, 'kind': 'delta', 'session': 'fixture', 'raid': 0,
                           'base_revision': body['expected_view_revision'], 'revision': state['rev'], 'bytes': [state['n']]},
                  'reply': {'type': 'done'}}
    elif method == 'owned.replay' and staged and not baseline:
        state = {'n': state['n'] + 1, 'rev': state['rev'] + 1}
        result = {'api': 1, 'session': 'fixture', 'raid': 0, 'seq': body['seq'],
                  'view_revision': state['rev'], 'reply': {'type': 'done'}}
    elif method == 'owned.query' and active: result = {'api': 1, 'session': 'fixture', 'reply': {'type': 'cursor'}}
    elif method == 'dispose': result = None
    else: result = {'Err': 'unknown method or invalid lifecycle state: ' + method}
    wire = {'Ok': result} if not (isinstance(result, dict) and 'Err' in result) else result
    reply = json.dumps({'api': 1, 'generation': request['generation'], 'request': request['request'], 'result': wire}).encode()
    sys.stdout.buffer.write(struct.pack('>I', len(reply)) + reply); sys.stdout.buffer.flush()
    if method == 'dispose': break
"##;

    struct Fixture {
        root: PathBuf,
        artifact: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "angel-owned-test-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let artifact = root.join("worker");
            fs::write(&artifact, STRICT_WORKER).unwrap();
            fs::set_permissions(&artifact, fs::Permissions::from_mode(0o700)).unwrap();
            Self { root, artifact }
        }
        fn set_fault(&self, before_seq: u64, after_seq: u64) {
            let script = STRICT_WORKER
                .replace(
                    "CRASH_BEFORE_SEQ = 0",
                    &format!("CRASH_BEFORE_SEQ = {before_seq}"),
                )
                .replace(
                    "CRASH_AFTER_SEQ = 0",
                    &format!("CRASH_AFTER_SEQ = {after_seq}"),
                );
            self.atomic_publish(script);
        }
        fn reject_restore_candidate(&self) {
            self.atomic_publish(
                STRICT_WORKER.replace("REJECT_RESTORE = False", "REJECT_RESTORE = True"),
            );
        }
        fn malformed_candidate(&self) {
            self.atomic_publish(STRICT_WORKER.replace(
                "MALFORMED_CHECKPOINT = False",
                "MALFORMED_CHECKPOINT = True",
            ));
        }
        fn atomic_publish(&self, content: String) {
            let temp = self.artifact.with_extension("new");
            fs::write(&temp, content).unwrap();
            fs::set_permissions(&temp, fs::Permissions::from_mode(0o700)).unwrap();
            fs::rename(temp, &self.artifact).unwrap();
        }
        fn host(&self, limits: Limits) -> OwnedSystem {
            OwnedSystem::watch(
                "fixture",
                self.artifact.clone(),
                Duration::from_secs(1),
                BTreeMap::new(),
                limits,
            )
            .unwrap()
        }
        fn initialize(&self, host: &mut OwnedSystem, validator: &mut TestValidator) {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match host.initialize(serde_json::json!({"n": 0, "rev": 0}), validator) {
                    Ok(frame) => {
                        assert_eq!(frame["revision"].as_u64(), Some(1));
                        return;
                    }
                    Err(error)
                        if error.contains("preparing its first") && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => panic!("fixture initialization failed: {error}"),
                }
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn envelope(seq: u64, revision: u64) -> Value {
        serde_json::json!({"api": 1, "session": "fixture", "raid": 0, "seq": seq,
            "expected_view_revision": revision,
            "event": {"type": "tick", "value": {"inputs": {}, "step": true}}})
    }

    #[test]
    fn accepted_prefix_replays_after_post_commit_death_then_exact_retry_succeeds() {
        let fixture = Fixture::new();
        fixture.set_fault(0, 2);
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        let first = host.event(envelope(1, 1), &mut validator).unwrap();
        assert_eq!(
            first.pointer("/view/bytes/0").and_then(Value::as_u64),
            Some(1)
        );
        let pending = envelope(2, 2);
        assert!(host.event(pending.clone(), &mut validator).is_err());
        assert_eq!(host.pending_event(), Some(&pending));
        assert_eq!(host.tail.len(), 1);
        let retried = host.event(pending, &mut validator).unwrap();
        assert_eq!(
            retried.pointer("/view/bytes/0").and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(host.tail.len(), 2);
    }

    #[test]
    fn pre_commit_death_keeps_input_pending_until_exact_retry() {
        let fixture = Fixture::new();
        fixture.set_fault(1, 0);
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        let pending = envelope(1, 1);
        assert!(host.event(pending.clone(), &mut validator).is_err());
        assert_eq!(host.pending_event(), Some(&pending));
        let frame = host.event(pending, &mut validator).unwrap();
        assert_eq!(
            frame.pointer("/view/bytes/0").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(host.tail.len(), 1);
    }

    #[test]
    fn exact_retry_fences_conflicts_and_capacity_checkpoints_before_rpc() {
        let fixture = Fixture::new();
        fixture.set_fault(0, 2);
        let mut host = fixture.host(Limits {
            events: 1,
            bytes: 64 * 1024,
        });
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        host.event(envelope(1, 1), &mut validator).unwrap();
        let pending = envelope(2, 2);
        assert!(host.event(pending.clone(), &mut validator).is_err());
        assert_eq!(host.pending_event(), Some(&pending));
        assert!(host.event(envelope(3, 2), &mut validator).is_err());
        assert_eq!(host.tail.len(), 0);
        assert_eq!(
            host.checkpoint.as_ref().and_then(|cp| cp["n"].as_u64()),
            Some(1)
        );
        let frame = host.event(pending, &mut validator).unwrap();
        assert_eq!(
            frame.pointer("/view/bytes/0").and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(host.tail.len(), 1);
    }

    #[test]
    fn bad_candidate_keeps_old_owner_and_replay_witness_divergence_refuses() {
        let fixture = Fixture::new();
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        host.event(envelope(1, 1), &mut validator).unwrap();
        fixture.reject_restore_candidate();
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut rejected = false;
        while Instant::now() < deadline {
            if host.refresh(&mut validator).is_err() {
                rejected = true;
                break;
            }
            thread::sleep(Duration::from_millis(15));
        }
        assert!(rejected, "watcher did not prepare the rejected candidate");
        assert!(
            host.inner
                .active
                .as_ref()
                .is_some_and(|worker| !worker.failed)
        );
        host.event(envelope(2, 2), &mut validator).unwrap();
        host.retire_active();
        validator.diverge_replay = true;
        assert!(host.recover_pinned(&mut validator).is_err());
        assert!(host.inner.active.is_none());
        assert_eq!(host.tail.len(), 2);
        validator.diverge_replay = false;
        host.recover_pinned(&mut validator).unwrap();
    }

    #[test]
    fn query_validation_failure_retires_then_explicit_recovery_restores_owner() {
        let fixture = Fixture::new();
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        host.event(envelope(1, 1), &mut validator).unwrap();
        assert!(
            host.query(serde_json::json!({"type": "cursor"}), |_| {
                Err("injected query validation fault".into())
            })
            .is_err()
        );
        assert!(host.inner.active.is_none());
        assert_eq!(host.tail.len(), 1);
        host.recover(&mut validator).unwrap();
        let reply = host
            .query(serde_json::json!({"type": "cursor"}), |_| Ok(()))
            .unwrap();
        assert_eq!(
            reply.pointer("/reply/type").and_then(Value::as_str),
            Some("cursor")
        );
    }

    #[test]
    fn checkpoint_validation_failure_preserves_tail_and_can_recover() {
        let fixture = Fixture::new();
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        host.event(envelope(1, 1), &mut validator).unwrap();
        validator.reject_checkpoint = true;
        assert!(host.checkpoint_now(&mut validator).is_err());
        assert!(host.inner.active.is_none());
        assert_eq!(host.tail.len(), 1);
        validator.reject_checkpoint = false;
        host.recover(&mut validator).unwrap();
        host.checkpoint_now(&mut validator).unwrap();
        assert!(host.tail.is_empty());
    }

    #[test]
    fn semantic_witness_is_fixed_size_and_distinguishes_large_replies() {
        let large = serde_json::json!({"session": "s", "seq": 9,
            "reply": {"definition": "x".repeat(1_000_000)}});
        let other = serde_json::json!({"session": "s", "seq": 9,
            "reply": {"definition": "y".repeat(1_000_000)}});
        let first = semantic_witness(&large).unwrap();
        let second = semantic_witness(&other).unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(second.len(), 32);
        assert_ne!(first, second);
    }

    #[test]
    fn stale_active_view_and_malformed_candidate_are_rejected() {
        let fixture = Fixture::new();
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        host.event(envelope(1, 1), &mut validator).unwrap();
        fixture.malformed_candidate();
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline && host.inner.pending.lock().unwrap().is_none() {
            thread::sleep(Duration::from_millis(15));
        }
        assert!(
            host.inner.pending.lock().unwrap().is_some(),
            "candidate was not prepared"
        );
        validator.reject_recovered = true;
        assert!(host.refresh(&mut validator).is_err());
        assert!(
            host.inner.active.is_none(),
            "stale accepted view must retire old owner"
        );
        assert_eq!(
            host.tail.len(),
            1,
            "accepted events must remain recoverable"
        );
        validator.reject_recovered = false;
        host.recover(&mut validator).unwrap();
    }

    #[test]
    fn malformed_candidate_checkpoint_keeps_active_owner() {
        let fixture = Fixture::new();
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        fixture.malformed_candidate();
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline && host.inner.pending.lock().unwrap().is_none() {
            thread::sleep(Duration::from_millis(15));
        }
        assert!(host.refresh(&mut validator).is_err());
        assert!(
            host.inner
                .active
                .as_ref()
                .is_some_and(|worker| !worker.failed)
        );
    }

    #[test]
    fn failed_initialization_retries_corrected_seed_with_same_pinned_image() {
        let fixture = Fixture::new();
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: true,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        let rejected_seed = serde_json::json!({"n": 0, "rev": 0});
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            match host.initialize(rejected_seed.clone(), &mut validator) {
                Err(error)
                    if error.contains("preparing its first") && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    assert!(error.contains("injected checkpoint validation fault"));
                    break;
                }
                Ok(_) => panic!("injected invalid seed was accepted"),
            }
        }
        assert!(host.inner.active.is_none());
        validator.reject_checkpoint = false;
        let corrected_seed = serde_json::json!({"n": 7, "rev": 0});
        let baseline = host.initialize(corrected_seed, &mut validator).unwrap();
        assert_eq!(
            baseline.pointer("/bytes/0").and_then(Value::as_u64),
            Some(7)
        );
        assert_eq!(host.status().swaps, 1);
        assert!(
            host.inner
                .active
                .as_ref()
                .is_some_and(|worker| !worker.failed)
        );
    }

    #[test]
    fn abort_panic_cannot_keep_uncertain_process_or_discard_tail_and_pending_input() {
        let fixture = Fixture::new();
        let mut host = fixture.host(Limits::default());
        let mut validator = TestValidator {
            staged: None,
            expected_replay: None,
            diverge_replay: false,
            reject_checkpoint: false,
            reject_recovered: false,
            reject_event: false,
            panic_abort: false,
        };
        fixture.initialize(&mut host, &mut validator);
        host.event(envelope(1, 1), &mut validator).unwrap();
        let uncertain_pid = host.status().worker_pid.unwrap();
        let pending = envelope(2, 2);
        validator.reject_event = true;
        validator.panic_abort = true;
        assert!(host.event(pending.clone(), &mut validator).is_err());
        assert_eq!(host.pending_event(), Some(&pending));
        assert_eq!(host.tail.len(), 1);
        assert_ne!(host.status().worker_pid, Some(uncertain_pid));
        #[cfg(target_os = "linux")]
        assert!(!Path::new(&format!("/proc/{uncertain_pid}")).exists());

        validator.reject_event = false;
        validator.panic_abort = false;
        host.event(pending, &mut validator).unwrap();
        assert!(host.pending_event().is_none());
        assert_eq!(host.tail.len(), 2);
    }
}
