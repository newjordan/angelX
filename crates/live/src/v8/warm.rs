//! Warm isolates for tool-less evaluation (live modules). Each run gets a fresh
//! context, so no script state survives a call; the isolate, its heap guard and
//! its watchdog thread are kept, which removes the per-call isolate start-up. A
//! failed or terminated run drops the isolate and the next run builds a new one.
//! A warm isolate is owned by one thread for its whole life, so no other
//! isolate is created or dropped between its creation and its drop.
use super::script::{
    bind_fn, build_source, compile_run, drain, init, near_heap_limit_cb, read_result, throw,
};
use ::v8;
use std::ffi::c_void;
use std::time::Duration;

/// What a live module's `tool(...)` / `batch(...)` answers: nothing executes.
const NO_TOOLS: &str = "live projections do not execute tools";

pub struct Warm {
    heap_mb: usize,
    live: Option<WarmIsolate>,
}

struct WarmIsolate {
    /// Module setups already run, newest last, keyed by generation digest.
    contexts: Vec<(String, v8::Global<v8::Context>)>,
    isolate: v8::OwnedIsolate,
    heap_data: *mut c_void,
    arm: std::sync::mpsc::Sender<Option<Duration>>,
    finished: std::sync::mpsc::Receiver<bool>,
}

impl WarmIsolate {
    fn new(heap_mb: usize) -> Self {
        init();
        let params = v8::CreateParams::default().heap_limits(0, heap_mb.max(8) << 20);
        let mut isolate = v8::Isolate::new(params);
        isolate.set_microtasks_policy(v8::MicrotasksPolicy::Explicit);
        let heap_data = Box::into_raw(Box::new(isolate.thread_safe_handle())) as *mut c_void;
        isolate.add_near_heap_limit_callback(near_heap_limit_cb, heap_data);
        // Wait for an armed deadline, then for the disarm; a missed disarm
        // terminates the script. Reports whether it fired. Dropping `arm` ends it.
        let handle = isolate.thread_safe_handle();
        let (arm, armed) = std::sync::mpsc::channel::<Option<Duration>>();
        let (done, finished) = std::sync::mpsc::channel::<bool>();
        std::thread::spawn(move || {
            while let Ok(Some(timeout)) = armed.recv() {
                let fired = armed.recv_timeout(timeout).is_err();
                if fired {
                    handle.terminate_execution();
                    let _ = armed.recv();
                }
                if done.send(fired).is_err() {
                    break;
                }
            }
        });
        Self {
            contexts: Vec::new(),
            isolate,
            heap_data,
            arm,
            finished,
        }
    }
}

impl Drop for WarmIsolate {
    fn drop(&mut self) {
        self.contexts.clear();
        self.isolate
            .remove_near_heap_limit_callback(near_heap_limit_cb, 0);
        unsafe { drop(Box::from_raw(self.heap_data as *mut v8::IsolateHandle)) };
    }
}

impl Warm {
    pub fn new(heap_mb: usize) -> Self {
        Self {
            heap_mb,
            live: None,
        }
    }

    /// Run `call` in the context where `setup` already ran for `key` (a
    /// generation digest), running the setup first if this isolate has not.
    /// A failed call forgets that context; the next call rebuilds it.
    pub fn run_cached(
        &mut self,
        key: &str,
        setup: &str,
        call: &str,
        timeout: Duration,
    ) -> Result<String, String> {
        let heap_mb = self.heap_mb;
        let warm = self.live.get_or_insert_with(|| WarmIsolate::new(heap_mb));
        let _ = warm.arm.send(Some(timeout));
        let out = run_cached_inner(warm, key, setup, call);
        let _ = warm.arm.send(None);
        let fired = warm.finished.recv().unwrap_or(true);
        let used = warm.isolate.get_heap_statistics().used_heap_size();
        if fired || used > self.heap_mb << 19 {
            self.live = None;
        } else if out.is_err() {
            warm.contexts.retain(|(k, _)| k != key);
        }
        out
    }

    pub fn run(&mut self, script: &str, timeout: Duration) -> Result<String, String> {
        let heap_mb = self.heap_mb;
        let warm = self.live.get_or_insert_with(|| WarmIsolate::new(heap_mb));
        let _ = warm.arm.send(Some(timeout));
        let out = run_inner(&mut warm.isolate, script);
        let _ = warm.arm.send(None);
        let fired = warm.finished.recv().unwrap_or(true);
        // Discarded contexts share one capped heap; rebuild at half the cap so
        // the near-limit guard never terminates a healthy call.
        let used = warm.isolate.get_heap_statistics().used_heap_size();
        if fired || out.is_err() || used > self.heap_mb << 19 {
            self.live = None;
        }
        out
    }
}

/// Contexts one warm isolate keeps; the oldest setup is dropped past this.
const WARM_CONTEXTS: usize = 32;

/// `tool(name, args)` and the handle store reach the host through this one
/// bridge; a live projection has no tools, so it always throws (after the same
/// argument check the code-mode runner makes).
fn invoke_cb(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let name = args.get(0).to_rust_string_lossy(scope);
    let raw = args.get(1).to_rust_string_lossy(scope);
    if !raw.trim().is_empty()
        && let Err(e) = serde_json::from_str::<serde_json::Value>(&raw)
    {
        throw(
            scope,
            &format!("code_mode: bad JSON args for `{name}`: {e}"),
        );
        return;
    }
    throw(scope, NO_TOOLS);
}

/// `console.*` output is not kept for a live projection.
fn log_cb(_: &mut v8::PinScope, _: v8::FunctionCallbackArguments, _: v8::ReturnValue) {}

/// `batch([{tool,args},...])`: validated like the code-mode runner's, then each
/// item reports that no tool ran.
fn batch_cb(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let raw = args.get(0).to_rust_string_lossy(scope);
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            throw(scope, &format!("code_mode: batch bad JSON: {e}"));
            return;
        }
    };
    let Some(calls) = parsed.as_array() else {
        throw(scope, "code_mode: batch expects an array of {tool, args}");
        return;
    };
    let limit = std::env::var("ANGEL_CODE_MODE_MAX_BATCH_ITEMS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(48);
    if calls.len() > limit {
        throw(
            scope,
            &format!(
                "code_mode: batch has {} items; limit is {limit}",
                calls.len()
            ),
        );
        return;
    }
    if calls
        .iter()
        .any(|c| c.get("tool").and_then(|v| v.as_str()).is_none())
    {
        throw(
            scope,
            "code_mode: each batch item needs a string `tool` field",
        );
        return;
    }
    let out = serde_json::to_string(&vec![
        serde_json::json!({"ok": false, "error": NO_TOOLS});
        calls.len()
    ])
    .unwrap_or_else(|_| "[]".to_string());
    match v8::String::new(scope, &out) {
        Some(s) => rv.set(s.into()),
        None => throw(scope, "code_mode: batch output too large for V8"),
    }
}

fn bind_host(scope: &mut v8::PinScope, context: v8::Local<v8::Context>) {
    let data: v8::Local<v8::Value> = v8::undefined(scope).into();
    bind_fn(scope, context, "__angel_invoke", invoke_cb, data);
    bind_fn(scope, context, "__angel_log", log_cb, data);
    bind_fn(scope, context, "__angel_batch", batch_cb, data);
}

fn run_cached_inner(
    warm: &mut WarmIsolate,
    key: &str,
    setup: &str,
    call: &str,
) -> Result<String, String> {
    let WarmIsolate {
        isolate, contexts, ..
    } = warm;
    v8::scope!(let handle_scope, isolate);
    let cached = contexts.iter().position(|(k, _)| k == key);
    let context = match cached {
        Some(i) => v8::Local::new(handle_scope, &contexts[i].1),
        None => v8::Context::new(handle_scope, Default::default()),
    };
    let scope = &mut v8::ContextScope::new(handle_scope, context);
    bind_host(scope, context);
    if cached.is_none() {
        let source = build_source(
            &[],
            &format!("globalThis.__angel_feature=(()=>{{\n{setup}\nreturn feature;\n}})();"),
        );
        compile_run(scope, &source)?;
        drain(scope, context)?;
        read_result(scope, context)?;
        if contexts.len() >= WARM_CONTEXTS {
            contexts.remove(0);
        }
        contexts.push((key.to_string(), v8::Global::new(scope, context)));
    }
    let source = build_source(
        &[],
        &format!("const feature=globalThis.__angel_feature;\n{call}"),
    );
    compile_run(scope, &source)?;
    drain(scope, context)?;
    read_result(scope, context)
}

fn run_inner(isolate: &mut v8::Isolate, user_script: &str) -> Result<String, String> {
    v8::scope!(let handle_scope, isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);
    bind_host(scope, context);
    let source = build_source(&[], user_script);
    compile_run(scope, &source)?;
    drain(scope, context)?;
    read_result(scope, context)
}
