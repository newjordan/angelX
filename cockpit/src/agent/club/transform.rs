//! Bounded, capability-free JavaScript projections for application protocols.
//!
//! A caller captures the script once per request and retains that snapshot
//! through retries. V8 receives no tools, filesystem, environment or network.

use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

const MAX_SCRIPT_BYTES: usize = 128 * 1024;
const MAX_INPUT_BYTES: usize = 24 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_INFLIGHT: usize = 4;
const DEADLINE: Duration = Duration::from_secs(5);

struct Permit;
fn slots() -> &'static (Mutex<usize>, Condvar) {
    static SLOTS: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();
    SLOTS.get_or_init(|| (Mutex::new(0), Condvar::new()))
}
impl Drop for Permit {
    fn drop(&mut self) {
        let (lock, available) = slots();
        let mut count = lock.lock().unwrap_or_else(|error| error.into_inner());
        *count -= 1;
        available.notify_one();
    }
}

fn acquire(cancel: Option<&AtomicBool>, deadline: Instant) -> Result<Permit, String> {
    let (lock, available) = slots();
    let mut count = lock.lock().unwrap_or_else(|error| error.into_inner());
    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Err("projection interrupted".into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("projection capacity timed out".into());
        }
        if *count < MAX_INFLIGHT {
            *count += 1;
            return Ok(Permit);
        }
        count = available
            .wait_timeout(count, remaining.min(Duration::from_millis(10)))
            .unwrap_or_else(|error| error.into_inner())
            .0;
    }
}

/// Read one complete script twice; a missing resource uses the embedded
/// baseline for installations created before the resource existed.
pub(crate) fn snapshot(path: &str, embedded: &'static str) -> Result<Arc<str>, String> {
    let root = crate::platform::runtime_paths::root();
    let path = root.join(Path::new(path));
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Arc::from(embedded));
        }
        Err(error) => return Err(format!("projection module: {error}")),
        Ok(_) => {}
    }
    let first = crate::platform::worker_snapshot::read(&path, "application projection")?;
    if first.len() > MAX_SCRIPT_BYTES {
        return Err("projection module exceeds its byte limit".into());
    }
    if first != crate::platform::worker_snapshot::read(&path, "application projection")? {
        return Err("projection module changed during capture".into());
    }
    let text = String::from_utf8(first).map_err(|_| "projection module is not UTF-8")?;
    Ok(Arc::from(text))
}

/// Invoke `project(input)` from a captured script, preserving JSON numeric
/// literals exactly rather than rounding large integers through JavaScript.
/// No credentials or host operations are available to the script. Cancellation
/// returns promptly; a cancelled isolate continues without I/O until its own
/// deadline. Permits bound the number of concurrent isolates; each isolate
/// has a V8 heap limit, which does not bound external backing stores or total
/// process memory.
pub(crate) fn evaluate(
    module: &str,
    input: &Value,
    cancel: Option<&AtomicBool>,
) -> Result<Value, String> {
    if module.len() > MAX_SCRIPT_BYTES {
        return Err("projection module exceeds its byte limit".into());
    }
    if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Err("projection interrupted".into());
    }
    let deadline = Instant::now() + DEADLINE;
    let permit = acquire(cancel, deadline)?;
    let raw = serde_json::to_string(input).map_err(|_| "could not encode projection")?;
    if raw.len() > MAX_INPUT_BYTES {
        return Err("projection input exceeds its byte limit".into());
    }
    // The script sees exact numeric tokens as wrappers. It may copy but must
    // not perform arithmetic on them; the native host validates the result.
    let quoted = serde_json::to_string(&raw).map_err(|_| "could not encode projection")?;
    let source = format!(
        r#"
class __TransformNumber {{ constructor(raw) {{ this.raw = raw; }} }}
const __transformObject = v => v !== null && typeof v === 'object' && !Array.isArray(v) && !(v instanceof __TransformNumber);
const __transformParse = raw => JSON.parse(raw, (_k, v, c) => typeof v === 'number' ? new __TransformNumber(c.source) : v);
const __transformStringify = v => JSON.stringify(v, (_k, x) => x instanceof __TransformNumber ? JSON.rawJSON(x.raw) : x);
const input = __transformParse({quoted});
{module}
return __transformStringify(project(input));
"#
    );
    if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Err("projection interrupted".into());
    }
    if Instant::now() >= deadline {
        return Err("projection timed out".into());
    }
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("angel-projection".into())
        .spawn(move || {
            let _permit = permit;
            let invoke = |_: &str, _: &Value| -> Result<String, String> {
                Err("projections do not have tools".into())
            };
            let schedule = |_: &[(String, Value)]| Vec::new();
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return;
            }
            let result =
                crate::agent::code_mode::run(&source, &[], &invoke, &schedule, remaining, 256)
                    .map_err(|_| "projection failed".to_string())
                    .and_then(|output| {
                        if output.result.len() > MAX_OUTPUT_BYTES {
                            return Err("projection output exceeds its byte limit".into());
                        }
                        serde_json::from_str(&output.result)
                            .map_err(|_| "invalid projection response".to_string())
                    });
            let _ = send.send(result);
        })
        .map_err(|_| "could not start projection".to_string())?;
    let output = loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Err("projection interrupted".into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("projection timed out".into());
        }
        match receive.recv_timeout(remaining.min(Duration::from_millis(10))) {
            Ok(result) => break result?,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("projection failed".into()),
        }
    };
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exact_numbers_and_capability_free_projection() {
        let input = json!({
            "integer": u64::MAX,
            "negative_zero": -0.0,
            "nested": { "x": 9007199254740993u64 },
        });
        let output = evaluate(
            "function project(x) { try { tool('read_file', {}); throw new Error('tool worked'); } catch (e) { if (String(e).includes('tool worked')) throw e; } return x; }",
            &input,
            None,
        ).unwrap();
        assert_eq!(output, input);
        assert_eq!(output["integer"].as_u64(), Some(u64::MAX));
        assert!(output["negative_zero"].as_f64().unwrap().is_sign_negative());
    }

    #[test]
    fn captured_generations_survive_edits_and_missing_resource_uses_baseline() {
        let dir = std::env::temp_dir().join(format!("angel-projection-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("module.js");
        let path = file.to_str().unwrap();
        let first = "function project(_) { return 'first'; }";
        let second = "function project(_) { return 'second'; }";
        let embedded = "function project(_) { return 'embedded'; }";
        std::fs::write(&file, first).unwrap();
        let pinned = snapshot(path, embedded).unwrap();
        std::fs::write(&file, second).unwrap();
        let updated = snapshot(path, embedded).unwrap();
        assert_eq!(evaluate(&pinned, &json!({}), None).unwrap(), json!("first"));
        assert_eq!(
            evaluate(&updated, &json!({}), None).unwrap(),
            json!("second")
        );
        std::fs::remove_file(&file).unwrap();
        assert_eq!(&*snapshot(path, embedded).unwrap(), embedded);
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn invalid_projection_cannot_return_private_exception_text() {
        let error = evaluate(
            "function project(x) { throw new Error(x.secret); }",
            &json!({"secret": "private-test-value"}),
            None,
        )
        .unwrap_err();
        assert_eq!(error, "projection failed");
    }

    #[test]
    fn cancellation_returns_before_a_stalled_projection_finishes() {
        let cancelled = AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                cancelled.store(true, Ordering::Release);
            });
            let start = Instant::now();
            let error = evaluate(
                "function project(_) { while (true) {} }",
                &json!({}),
                Some(&cancelled),
            )
            .unwrap_err();
            assert!(error.contains("interrupted"), "{error}");
            assert!(start.elapsed() < Duration::from_secs(2));
        });
    }
}
