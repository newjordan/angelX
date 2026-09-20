//! code-mode — run a model-authored JS program in an embedded V8 isolate, with
//! the cockpit's tools exposed as host functions. One `code_mode` call collapses
//! what would otherwise be many tool round-trips (loops, conditionals, batching,
//! filtering) into a *single* agent turn. Strip-mined from Codex's `code-mode`
//! crate, but rebuilt **synchronous**: the cockpit is tokio-free, so host tool
//! calls block and return inline, and the (optionally `async`) user script is
//! settled by draining V8's microtask queue rather than running an event loop.
//!
//! ## Capability boundary
//! V8 has no I/O of its own — no `fetch`, no `require`, no filesystem. The *only*
//! capabilities a script gets are the host functions we bind, and every one of
//! those is an existing cockpit tool that already carries its own
//! workspace-confinement / landlock / hook checks. So code-mode inherits the
//! cockpit's tool safety exactly; it adds no new reach, it just lets the model
//! orchestrate the same tools programmatically.
//!
//! ## Why nested outputs are NOT capped
//! The harness centrally caps every *top-level* tool result before it enters
//! history. Inside code-mode that cap is deliberately skipped for the nested
//! calls: a script that reads a file, edits the text, and writes it back needs
//! the *full* content, not a head/tail-truncated view. Only code-mode's final
//! return value re-enters history, where the usual central cap applies upstream.
//!
//! This module is intentionally free of any harness types so it can be unit
//! tested with a mock `invoke` closure (no `ToolRegistry`, no V8 knowledge in the
//! test). The harness glue (`run_tool`) lives in `harness.rs`.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ops::Range;
use std::sync::Once;
use std::time::Duration;

use serde_json::Value;

type ToolInvoker<'a> = dyn Fn(&str, &Value) -> Result<String, String> + Sync + 'a;
type BatchScheduler<'a> = dyn Fn(&[(String, Value)]) -> Vec<Range<usize>> + Sync + 'a;

/// One-time process-global V8 platform initialisation. `Once` makes it safe to
/// call from every `run()` and from parallel test threads; V8 is never disposed
/// (the cockpit is a long-running process that may run code-mode again).
static V8_INIT: Once = Once::new();

fn init_v8() {
    V8_INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

/// The Rust-side context handed to the V8 callbacks through an `External`
/// pointer. Its lifetime is the duration of a single `run()`; the isolate (and
/// thus any callback that could read this pointer) is created and dropped within
/// that same call, so the erased pointer is never dereferenced after `ctx` dies.
struct HostCtx<'a> {
    /// Dispatch a tool by name with JSON args → its text output (or an error,
    /// surfaced to JS as a thrown `Error`). `+ Sync` so `batch()` can call it
    /// from several `thread::scope` workers at once.
    invoke: &'a ToolInvoker<'a>,
    /// Partition one `batch()` into consecutive execution segments. The
    /// harness supplies its ordinary read/write-footprint scheduler; keeping
    /// the decision outside this V8-only module avoids inventing a second tool
    /// taxonomy that can drift from the turn-loop scheduler.
    batch_scheduler: &'a BatchScheduler<'a>,
    /// `console.log`/`error`/… lines, in order, for the run summary.
    console: RefCell<Vec<String>>,
    console_bytes: Cell<usize>,
    console_limit: usize,
    console_truncated: Cell<bool>,
}

/// Reconstruct the `HostCtx` reference a callback was built with from its
/// `External` data pointer. Sound only because the isolate never outlives the
/// `HostCtx` that produced the `External` (see the struct doc).
unsafe fn ctx_from<'c>(data: v8::Local<v8::Value>) -> &'c HostCtx<'c> {
    unsafe {
        let ext = data.cast::<v8::External>();
        &*(ext.value() as *const HostCtx<'c>)
    }
}

/// Throw a JS `Error(msg)` in the current scope (used to surface tool errors).
fn throw(scope: &mut v8::PinScope, msg: &str) {
    if let Some(s) = v8::String::new(scope, msg) {
        let exc = v8::Exception::error(scope, s);
        scope.throw_exception(exc);
    }
}

/// `__angel_invoke(name, argsJson) -> string`: the single native bridge. The
/// per-tool named wrappers in the prelude are thin JS shims over this.
fn invoke_cb(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let ctx = unsafe { ctx_from(args.data()) };
    let name = args.get(0).to_rust_string_lossy(scope);
    let raw = args.get(1).to_rust_string_lossy(scope);
    let parsed: Value = if raw.trim().is_empty() {
        Value::Object(Default::default())
    } else {
        match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                throw(
                    scope,
                    &format!("code_mode: bad JSON args for `{name}`: {e}"),
                );
                return;
            }
        }
    };
    match (ctx.invoke)(&name, &parsed) {
        Ok(out) => match v8::String::new(scope, &out) {
            Some(s) => rv.set(s.into()),
            None => throw(
                scope,
                &format!("code_mode: `{name}` output too large for V8"),
            ),
        },
        Err(e) => throw(scope, &e),
    }
}

/// `__angel_batch(callsJson) -> resultsJson`: run an array of `{tool, args}`
/// calls in footprint-safe segments and return an array (same order) of
/// `{ok, output}` / `{ok:false, error}`. The mass-testing primitive — fan a
/// safe test matrix / command set out across threads in one turn while
/// effect/conflict barriers remain ordered. Per-item errors are captured (a
/// single failure never aborts the batch).
fn batch_cb(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let ctx = unsafe { ctx_from(args.data()) };
    let raw = args.get(0).to_rust_string_lossy(scope);
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            throw(scope, &format!("code_mode: batch bad JSON: {e}"));
            return;
        }
    };
    let calls = match parsed.as_array() {
        Some(a) => a,
        None => {
            throw(scope, "code_mode: batch expects an array of {tool, args}");
            return;
        }
    };
    let max_batch_items = batch_item_cap();
    if calls.len() > max_batch_items {
        throw(
            scope,
            &format!(
                "code_mode: batch has {} items; limit is {max_batch_items}",
                calls.len()
            ),
        );
        return;
    }
    let mut specs: Vec<(String, Value)> = Vec::with_capacity(calls.len());
    for c in calls {
        match c.get("tool").and_then(|v| v.as_str()) {
            Some(n) => specs.push((
                n.to_string(),
                c.get("args")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Default::default())),
            )),
            None => {
                throw(
                    scope,
                    "code_mode: each batch item needs a string `tool` field",
                );
                return;
            }
        }
    }
    let segments = (ctx.batch_scheduler)(&specs);
    let results = run_scheduled_batch(ctx.invoke, &specs, segments, batch_cap());
    let arr: Vec<Value> = results
        .into_iter()
        .map(|r| match r {
            Ok(o) => serde_json::json!({ "ok": true, "output": o }),
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        })
        .collect();
    let out = serde_json::to_string(&Value::Array(arr)).unwrap_or_else(|_| "[]".to_string());
    match v8::String::new(scope, &out) {
        Some(s) => rv.set(s.into()),
        None => throw(scope, "code_mode: batch output too large for V8"),
    }
}

/// Concurrency cap for `batch()`: `ANGEL_CODE_MODE_BATCH_CONCURRENCY`, else the
/// machine's parallelism clamped to 16.
fn batch_cap() -> usize {
    if let Ok(v) = std::env::var("ANGEL_CODE_MODE_BATCH_CONCURRENCY")
        && let Ok(n) = v.trim().parse::<usize>()
        && n > 0
    {
        return n;
    }
    std::thread::available_parallelism()
        .map(|n| n.get().min(16))
        .unwrap_or(8)
}

/// A separate construction bound prevents a script from materialising a huge
/// batch specification before the per-call dispatch budget can reject it.
fn batch_item_cap() -> usize {
    if crate::yolo::enabled() {
        return usize::MAX;
    }
    std::env::var("ANGEL_CODE_MODE_MAX_BATCH_ITEMS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(48)
}

/// Run `specs` through `invoke` concurrently, `cap` at a time, preserving order.
/// Pure (no V8) so it's unit-testable. A worker panic becomes that item's error.
pub fn run_batch(
    invoke: &(dyn Fn(&str, &Value) -> Result<String, String> + Sync),
    specs: &[(String, Value)],
    cap: usize,
) -> Vec<Result<String, String>> {
    let cap = cap.max(1);
    let mut out: Vec<Result<String, String>> = Vec::with_capacity(specs.len());
    let mut idx = 0;
    while idx < specs.len() {
        let end = (idx + cap).min(specs.len());
        let chunk = &specs[idx..end];
        let chunk_out: Vec<Result<String, String>> = std::thread::scope(|s| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(name, a)| {
                    let model = crate::harness::run_identity::live_model();
                    let turn = crate::harness::run_identity::live_turn();
                    s.spawn(move || {
                        let _turn = crate::harness::run_identity::LiveTurnScope::enter(turn);
                        let _model = crate::harness::run_identity::LiveModelScope::enter(model);
                        invoke(name, a)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err("code_mode: batch worker panicked".to_string()))
                })
                .collect()
        });
        out.extend(chunk_out);
        idx = end;
    }
    out
}

/// Run consecutive scheduler segments in declaration order, concurrently
/// within multi-item segments. A malformed scheduler response fails closed to
/// one serial segment per call rather than risking reordered effects.
pub fn run_scheduled_batch(
    invoke: &(dyn Fn(&str, &Value) -> Result<String, String> + Sync),
    specs: &[(String, Value)],
    segments: Vec<Range<usize>>,
    cap: usize,
) -> Vec<Result<String, String>> {
    let mut cursor = 0;
    let valid = segments.iter().all(|segment| {
        let contiguous =
            segment.start == cursor && segment.start < segment.end && segment.end <= specs.len();
        cursor = segment.end;
        contiguous
    }) && cursor == specs.len();
    let segments = if valid {
        segments
    } else {
        (0..specs.len()).map(|index| index..index + 1).collect()
    };

    let mut out = Vec::with_capacity(specs.len());
    for segment in segments {
        if segment.len() > 1 {
            out.extend(run_batch(invoke, &specs[segment], cap));
        } else {
            let (name, args) = &specs[segment.start];
            out.push(invoke(name, args));
        }
    }
    out
}

/// `__angel_log(...parts)`: collect console output into the host buffer.
fn log_cb(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut _rv: v8::ReturnValue) {
    let ctx = unsafe { ctx_from(args.data()) };
    if ctx.console_bytes.get() >= ctx.console_limit {
        ctx.console_truncated.set(true);
        return;
    }
    let mut parts = Vec::new();
    for i in 0..args.length() {
        parts.push(args.get(i).to_rust_string_lossy(scope));
    }
    let mut line = parts.join(" ");
    let remaining = ctx.console_limit.saturating_sub(ctx.console_bytes.get());
    if line.len() > remaining {
        let mut end = remaining;
        while end > 0 && !line.is_char_boundary(end) {
            end -= 1;
        }
        line.truncate(end);
        ctx.console_truncated.set(true);
    }
    ctx.console_bytes
        .set(ctx.console_bytes.get().saturating_add(line.len()));
    if !line.is_empty() {
        ctx.console.borrow_mut().push(line);
    }
}

/// The result of a code-mode run: the script's return value (stringified) plus
/// any captured console output.
#[derive(Debug, Default, Clone)]
pub struct Outcome {
    pub result: String,
    pub console: Vec<String>,
    pub console_truncated: bool,
}

/// Near-heap-limit guard. Fired on the isolate thread as the capped heap nears
/// exhaustion: terminate the running script and hand V8 headroom (raise the
/// limit) so it can unwind and surface an `Err` instead of calling
/// `FatalProcessOutOfMemory`, which would ABORT the whole cockpit process.
/// `data` is a `*const IsolateHandle` boxed by `run` (valid until `run` removes
/// this callback). The `v8` 150 `NearHeapLimitCallback` type is
/// `unsafe extern "C"`, hence the signature here.
unsafe extern "C" fn near_heap_limit_cb(
    data: *mut c_void,
    current: usize,
    _initial: usize,
) -> usize {
    if !data.is_null() {
        let handle = unsafe { &*(data as *const v8::IsolateHandle) };
        handle.terminate_execution();
    }
    // Give V8 enough extra headroom to unwind past the termination point.
    current + (current / 2).max(8 << 20)
}

/// Run `script` with `tool_names` bound as host functions. `invoke` dispatches a
/// nested tool call. `timeout` bounds wall-clock execution (a watchdog thread
/// terminates the isolate past it); `heap_mb` caps the isolate heap (a
/// near-limit callback terminates the script before V8 aborts the process).
///
/// Returns the (stringified) value the script `return`s, or an `Err` for a
/// compile error, an uncaught runtime error, or a timeout.
pub fn run(
    script: &str,
    tool_names: &[String],
    invoke: &ToolInvoker<'_>,
    batch_scheduler: &BatchScheduler<'_>,
    timeout: Duration,
    heap_mb: usize,
) -> Result<Outcome, String> {
    init_v8();

    let ctx = HostCtx {
        invoke,
        batch_scheduler,
        console: RefCell::new(Vec::new()),
        console_bytes: Cell::new(0),
        console_limit: std::env::var("ANGEL_CODE_MODE_MAX_CONSOLE_BYTES")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(65_536),
        console_truncated: Cell::new(false),
    };

    let params = v8::CreateParams::default().heap_limits(0, heap_mb.max(8) << 20);
    let isolate = &mut v8::Isolate::new(params);
    // Drain microtasks ourselves so the async result-capture is deterministic.
    isolate.set_microtasks_policy(v8::MicrotasksPolicy::Explicit);

    // Heap guard: install a near-heap-limit callback so exhausting the capped
    // heap TERMINATES the script (Err) instead of triggering V8's
    // FatalProcessOutOfMemory, which aborts the whole process. The boxed handle
    // is freed after `run_inner` once the callback is removed.
    let heap_data = Box::into_raw(Box::new(isolate.thread_safe_handle())) as *mut c_void;
    isolate.add_near_heap_limit_callback(near_heap_limit_cb, heap_data);

    // Watchdog: if the script runs past `timeout`, terminate it from another
    // thread. `IsolateHandle` is the documented cross-thread-safe surface.
    let handle = isolate.thread_safe_handle();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let watchdog = std::thread::spawn(move || {
        if done_rx.recv_timeout(timeout).is_err() {
            handle.terminate_execution();
        }
    });

    let out = run_inner(isolate, script, tool_names, &ctx);

    // Remove the heap callback before the isolate is dropped, then free the
    // boxed handle (no further callback can fire after removal).
    isolate.remove_near_heap_limit_callback(near_heap_limit_cb, 0);
    unsafe { drop(Box::from_raw(heap_data as *mut v8::IsolateHandle)) };

    // Release the watchdog (it may already have fired — terminate after the run
    // is a harmless no-op on an isolate we're about to drop).
    let _ = done_tx.send(());
    let _ = watchdog.join();

    out.map(|result| Outcome {
        result,
        console: ctx.console.into_inner(),
        console_truncated: ctx.console_truncated.get(),
    })
}

fn run_inner(
    isolate: &mut v8::Isolate,
    user_script: &str,
    tool_names: &[String],
    ctx: &HostCtx,
) -> Result<String, String> {
    v8::scope!(let handle_scope, isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    // One External carries the host-context pointer to both native callbacks.
    let ext = v8::External::new(scope, ctx as *const HostCtx as *mut c_void);
    bind_fn(scope, context, "__angel_invoke", invoke_cb, ext);
    bind_fn(scope, context, "__angel_log", log_cb, ext);
    bind_fn(scope, context, "__angel_batch", batch_cb, ext);

    let source = build_source(tool_names, user_script);
    compile_run(scope, &source)?;
    drain(scope, context)?;
    read_result(scope, context)
}

/// Install a native function `name` on the global object, carrying `ext` as its
/// callback data.
fn bind_fn(
    scope: &mut v8::PinScope,
    context: v8::Local<v8::Context>,
    name: &str,
    cb: impl v8::MapFnTo<v8::FunctionCallback>,
    ext: v8::Local<v8::External>,
) {
    let func = v8::Function::builder(cb)
        .data(ext.into())
        .build(scope)
        .expect("build native fn");
    let key = v8::String::new(scope, name).expect("fn name");
    context.global(scope).set(scope, key.into(), func.into());
}

/// Whether `name` is a plain JS identifier we can expose as `globalThis.name`.
fn is_js_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Build the full source: a trusted prelude (console + per-tool wrappers + the
/// async result-capture harness) wrapping the verbatim user script.
fn build_source(tool_names: &[String], user_script: &str) -> String {
    let mut p = String::new();
    // console → host log buffer.
    p.push_str(
        "globalThis.console={log:(...a)=>__angel_log(...a.map(String)),\
         error:(...a)=>__angel_log(...a.map(String)),\
         warn:(...a)=>__angel_log(...a.map(String)),\
         info:(...a)=>__angel_log(...a.map(String)),\
         debug:(...a)=>__angel_log(...a.map(String))};\n",
    );
    // Generic dispatcher: `tool(name, args)` reaches any tool in the bound
    // capability set, including names that are not valid JS identifiers.
    p.push_str(
        "globalThis.tool=(name,args)=>__angel_invoke(String(name),JSON.stringify(args??{}));\n",
    );
    // Parallel fan-out: `batch([{tool,args},...])` -> [{ok,output}|{ok:false,error}]
    // run CONCURRENTLY. Results are parsed objects (unlike single calls, which
    // return the tool's raw text). The mass-testing / mass-build primitive.
    p.push_str("globalThis.batch=(calls)=>JSON.parse(__angel_batch(JSON.stringify(calls??[])));\n");
    // Handle store: park intermediate bulk without returning it from the script.
    // `handle_put(body, {producer?, identity?})` → opaque `hnd_…` id.
    // `handle_get(id, {offset?, max_bytes?})` → capped slice text.
    p.push_str(
        "globalThis.handle_put=(body,meta)=>{\
           const m=(meta&&typeof meta==='object')?meta:{};\
           return __angel_invoke('__handle_put',JSON.stringify({\
             body:String(body??''),\
             producer:m.producer??'code_mode',\
             identity:m.identity??'code_mode|put'\
           }));\
         };\n\
         globalThis.handle_get=(id,opts)=>{\
           const o=(opts&&typeof opts==='object')?opts:{};\
           return __angel_invoke('__handle_get',JSON.stringify({\
             handle:String(id??''),\
             offset:o.offset??0,\
             max_bytes:o.max_bytes\
           }));\
         };\n",
    );
    // Per-tool named wrappers: `read_file({path})`, `grep({pattern}) `, …
    for name in tool_names {
        if is_js_ident(name) {
            p.push_str(&format!(
                "globalThis[{name:?}]=(args)=>__angel_invoke({name:?},JSON.stringify(args??{{}}));\n",
            ));
        }
    }
    // Result-capture harness. The user script runs inside an async IIFE so a
    // top-level `return` and top-level `await` both work; the result is
    // stringified (objects via JSON) and stashed on a global the host reads back.
    format!(
        "{p}\
         globalThis.__angel_done=false;globalThis.__angel_result=\"\";globalThis.__angel_error=null;\n\
         (async()=>{{\n{user_script}\n}})().then(\n\
         (v)=>{{try{{globalThis.__angel_result=(v===undefined||v===null)?\"\":\
         (typeof v===\"string\"?v:JSON.stringify(v));}}catch(e){{globalThis.__angel_result=String(v);}}\
         globalThis.__angel_done=true;}},\n\
         (e)=>{{globalThis.__angel_error=(e&&e.stack)?String(e.stack):String(e);globalThis.__angel_done=true;}}\n\
         );\n"
    )
}

/// Compile and run `src` once, mapping compile/runtime/termination failures to
/// `Err`. User-script errors don't surface here — the harness catches them into
/// `__angel_error` — so a failure here is a genuine compile error or a timeout.
fn compile_run(scope: &mut v8::PinScope, src: &str) -> Result<(), String> {
    v8::tc_scope!(let tc, scope);
    let code = v8::String::new(tc, src).ok_or("code_mode: script too large to compile")?;
    let script = match v8::Script::compile(tc, code, None) {
        Some(s) => s,
        None => return Err(format!("code_mode: compile error: {}", caught(tc))),
    };
    match script.run(tc) {
        Some(_) => Ok(()),
        None => {
            if tc.has_terminated() {
                Err("code_mode: execution timed out".to_string())
            } else {
                Err(format!("code_mode: runtime error: {}", caught(tc)))
            }
        }
    }
}

/// Format the currently-caught exception as a short string.
fn caught(tc: &mut v8::PinnedRef<'_, v8::TryCatch<v8::HandleScope>>) -> String {
    match tc.exception() {
        Some(e) => e.to_rust_string_lossy(tc),
        None => "(unknown)".to_string(),
    }
}

/// Drain the microtask queue until the async harness settles (`__angel_done`).
/// With synchronous host calls the chain converges in one checkpoint; we loop
/// defensively and let the watchdog bound any pathology.
fn drain(scope: &mut v8::PinScope, context: v8::Local<v8::Context>) -> Result<(), String> {
    for _ in 0..100_000 {
        scope.perform_microtask_checkpoint();
        if scope.is_execution_terminating() {
            return Err("code_mode: execution timed out".to_string());
        }
        if read_bool(scope, context, "__angel_done") {
            return Ok(());
        }
    }
    Ok(()) // fall through — read_result reports whatever settled
}

/// Resolve the run's outcome from the harness globals: `__angel_error` (if set)
/// → `Err`, else the stringified `__angel_result`.
fn read_result(
    scope: &mut v8::PinScope,
    context: v8::Local<v8::Context>,
) -> Result<String, String> {
    if let Some(err) = read_string(scope, context, "__angel_error")
        && !err.is_empty()
    {
        return Err(format!("code_mode: {err}"));
    }
    Ok(read_string(scope, context, "__angel_result").unwrap_or_default())
}

fn read_string(
    scope: &mut v8::PinScope,
    context: v8::Local<v8::Context>,
    key: &str,
) -> Option<String> {
    let global = context.global(scope);
    let k = v8::String::new(scope, key)?;
    let v = global.get(scope, k.into())?;
    if v.is_null_or_undefined() {
        return None;
    }
    Some(v.to_rust_string_lossy(scope))
}

fn read_bool(scope: &mut v8::PinScope, context: v8::Local<v8::Context>, key: &str) -> bool {
    let global = context.global(scope);
    match v8::String::new(scope, key).and_then(|k| global.get(scope, k.into())) {
        Some(v) => v.boolean_value(scope),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock tool host: a few arithmetic/echo "tools" so the runtime can be
    /// exercised end-to-end without a ToolRegistry.
    fn mock(name: &str, args: &Value) -> Result<String, String> {
        match name {
            // echo: return the args back as compact JSON.
            "echo" => Ok(args.to_string()),
            // double: {n} -> n*2 as a string.
            "double" => {
                let n = args.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
                Ok((n * 2).to_string())
            }
            // boom: always errors (to test catchable throws).
            "boom" => Err("kaboom".to_string()),
            other => Err(format!("unknown tool: {other}")),
        }
    }

    fn parallel_all(specs: &[(String, Value)]) -> Vec<Range<usize>> {
        if specs.is_empty() {
            Vec::new()
        } else {
            std::iter::once(0..specs.len()).collect()
        }
    }

    fn parallel_none(specs: &[(String, Value)]) -> Vec<Range<usize>> {
        (0..specs.len()).map(|index| index..index + 1).collect()
    }

    fn run_ok(script: &str) -> Outcome {
        run(
            script,
            &["echo".into(), "double".into(), "boom".into()],
            &mock,
            &parallel_all,
            Duration::from_secs(5),
            128,
        )
        .expect("run ok")
    }

    #[test]
    fn returns_number_stringified() {
        assert_eq!(run_ok("return 1 + 2;").result, "3");
    }

    #[test]
    fn returns_string_verbatim() {
        assert_eq!(run_ok("return 'hello';").result, "hello");
    }

    #[test]
    fn returns_object_as_json() {
        let out = run_ok("return {a: 1, b: [2, 3]};");
        // JSON.stringify ordering is insertion order.
        assert_eq!(out.result, r#"{"a":1,"b":[2,3]}"#);
    }

    #[test]
    fn undefined_return_is_empty() {
        assert_eq!(run_ok("let x = 1;").result, "");
    }

    #[test]
    fn calls_named_host_tool() {
        // double({n:21}) -> "42"
        assert_eq!(run_ok("return double({n: 21});").result, "42");
    }

    #[test]
    fn await_is_optional_and_harmless() {
        assert_eq!(
            run_ok("const a = await double({n: 5}); return a;").result,
            "10"
        );
    }

    #[test]
    fn loops_and_batches_across_calls() {
        // Sum double(1..=4) = 2+4+6+8 = 20, in ONE run (would be 4 round-trips).
        let out = run_ok(
            "let total = 0; for (let i = 1; i <= 4; i++) { total += Number(double({n: i})); } return total;",
        );
        assert_eq!(out.result, "20");
    }

    #[test]
    fn generic_tool_dispatch_reaches_any_tool() {
        // tool('double', {n:9}) works even without relying on the named wrapper.
        assert_eq!(run_ok("return tool('double', {n: 9});").result, "18");
    }

    #[test]
    fn host_error_is_a_catchable_js_exception() {
        let out = run_ok(
            "try { boom({}); return 'unreached'; } catch (e) { return 'caught:' + e.message; }",
        );
        assert_eq!(out.result, "caught:kaboom");
    }

    #[test]
    fn uncaught_host_error_fails_the_run() {
        let err = run(
            "return boom({});",
            &["boom".into()],
            &mock,
            &parallel_all,
            Duration::from_secs(5),
            128,
        )
        .unwrap_err();
        assert!(err.contains("kaboom"), "got: {err}");
    }

    #[test]
    fn console_log_is_captured() {
        let out = run_ok("console.log('hi', 7); console.error('oops'); return 'ok';");
        assert_eq!(out.result, "ok");
        assert_eq!(out.console, vec!["hi 7".to_string(), "oops".to_string()]);
        assert!(!out.console_truncated);
    }

    #[test]
    fn console_capture_is_byte_bounded() {
        let out = run_ok("console.log('x'.repeat(70000)); return 'ok';");
        assert_eq!(out.result, "ok");
        assert!(out.console_truncated);
        assert!(out.console.iter().map(String::len).sum::<usize>() <= 65_536);
    }

    #[test]
    fn syntax_error_is_reported() {
        let err = run(
            "return ((( ;",
            &[],
            &mock,
            &parallel_all,
            Duration::from_secs(5),
            128,
        )
        .unwrap_err();
        assert!(err.contains("compile error"), "got: {err}");
    }

    #[test]
    fn infinite_loop_times_out() {
        let err = run(
            "while (true) {}",
            &[],
            &mock,
            &parallel_all,
            Duration::from_millis(300),
            128,
        )
        .unwrap_err();
        assert!(err.contains("timed out"), "got: {err}");
    }

    #[test]
    fn heap_bomb_terminates_instead_of_aborting() {
        // Unbounded allocation under a tiny (8MB) heap cap must be TERMINATED by
        // the near-heap-limit callback (returns Err) rather than crash the whole
        // process via V8's FatalProcessOutOfMemory. The timeout is generous (30s)
        // so the failure is provably the heap guard, not the wall-clock watchdog
        // (the heap fills in well under a second). If this test process *aborts*
        // instead of failing the assertion, the heap guard regressed.
        let err = run(
            "let a = []; for (;;) { a.push(new Array(200000).fill(7)); }",
            &[],
            &mock,
            &parallel_all,
            Duration::from_secs(30),
            8,
        )
        .unwrap_err();
        // Termination surfaces through has_terminated() as "execution timed out".
        assert!(
            err.contains("timed out") || err.contains("terminated"),
            "expected a termination error, got: {err}"
        );
    }

    #[test]
    fn bad_json_args_throw() {
        // Force a non-serializable arg path by calling the bridge directly with a
        // value JSON.stringify drops to undefined → "{}" parses fine; instead test
        // that a thrown error inside a tool propagates with the tool name.
        let err = run(
            "return tool('nope', {});",
            &[],
            &mock,
            &parallel_all,
            Duration::from_secs(5),
            128,
        )
        .unwrap_err();
        assert!(err.contains("unknown tool: nope"), "got: {err}");
    }

    /// Like `mock`, plus a `sleep` tool so concurrency is observable.
    fn slow_mock(name: &str, args: &Value) -> Result<String, String> {
        if name == "sleep" {
            let ms = args.get("ms").and_then(|v| v.as_u64()).unwrap_or(0);
            std::thread::sleep(Duration::from_millis(ms));
            return Ok(format!("slept {ms}"));
        }
        mock(name, args)
    }

    #[test]
    fn batch_preserves_order_and_captures_errors() {
        let specs = vec![
            ("double".to_string(), serde_json::json!({ "n": 1 })),
            ("boom".to_string(), serde_json::json!({})),
            ("double".to_string(), serde_json::json!({ "n": 3 })),
        ];
        let out = run_batch(&mock, &specs, 4);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].as_ref().unwrap(), "2");
        assert!(out[1].is_err(), "middle item should carry its own error");
        assert_eq!(out[2].as_ref().unwrap(), "6");
    }

    #[test]
    fn batch_runs_concurrently() {
        // 8 × 50ms run concurrently (cap 16) → well under the 400ms serial sum.
        let specs: Vec<(String, Value)> = (0..8)
            .map(|_| ("sleep".to_string(), serde_json::json!({ "ms": 50 })))
            .collect();
        let start = std::time::Instant::now();
        let out = run_batch(&slow_mock, &specs, 16);
        let elapsed = start.elapsed();
        assert_eq!(out.len(), 8);
        assert!(out.iter().all(|r| r.is_ok()));
        assert!(
            elapsed < Duration::from_millis(250),
            "expected concurrent execution, took {elapsed:?}"
        );
    }

    #[test]
    fn batch_respects_concurrency_cap() {
        // cap 2 over 4 × 50ms → ~2 waves ≈ 100ms+, not the 200ms full-serial.
        let specs: Vec<(String, Value)> = (0..4)
            .map(|_| ("sleep".to_string(), serde_json::json!({ "ms": 50 })))
            .collect();
        let start = std::time::Instant::now();
        let out = run_batch(&slow_mock, &specs, 2);
        let elapsed = start.elapsed();
        assert_eq!(out.len(), 4);
        assert!(
            elapsed >= Duration::from_millis(90),
            "cap 2 should serialize into ~2 waves: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_millis(350),
            "but still overlap within a wave: {elapsed:?}"
        );
    }

    #[test]
    fn scheduled_batch_parallelizes_safe_runs_around_serial_barrier() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let active = [AtomicUsize::new(0), AtomicUsize::new(0)];
        let max_active = [AtomicUsize::new(0), AtomicUsize::new(0)];
        let barrier_seen = AtomicBool::new(false);
        let invoke = |name: &str, args: &Value| {
            if name == "barrier" {
                if active[0].load(Ordering::SeqCst) != 0 {
                    return Err("barrier crossed the first safe run".to_string());
                }
                barrier_seen.store(true, Ordering::SeqCst);
                return Ok(name.to_string());
            }
            let phase = args["phase"].as_u64().unwrap() as usize;
            if phase == 1 && !barrier_seen.load(Ordering::SeqCst) {
                return Err("second safe run crossed the barrier".to_string());
            }
            let now = active[phase].fetch_add(1, Ordering::SeqCst) + 1;
            max_active[phase].fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
            active[phase].fetch_sub(1, Ordering::SeqCst);
            Ok(name.to_string())
        };
        let specs = vec![
            ("a".to_string(), serde_json::json!({"phase": 0})),
            ("b".to_string(), serde_json::json!({"phase": 0})),
            ("barrier".to_string(), serde_json::json!({})),
            ("c".to_string(), serde_json::json!({"phase": 1})),
            ("d".to_string(), serde_json::json!({"phase": 1})),
        ];

        let results = run_scheduled_batch(&invoke, &specs, vec![0..2, 2..3, 3..5], 16);

        assert_eq!(
            results
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .expect("all segments"),
            ["a", "b", "barrier", "c", "d"]
        );
        assert_eq!(max_active[0].load(Ordering::SeqCst), 2);
        assert_eq!(max_active[1].load(Ordering::SeqCst), 2);
    }

    #[test]
    fn malformed_batch_schedule_fails_closed_to_declaration_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let active = AtomicUsize::new(0);
        let max_active = AtomicUsize::new(0);
        let invoke = |name: &str, _: &Value| {
            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
            max_active.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(10));
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(name.to_string())
        };
        let specs = (0..4)
            .map(|index| (index.to_string(), serde_json::json!({})))
            .collect::<Vec<_>>();

        let results = run_scheduled_batch(&invoke, &specs, vec![0..2, 3..4], 16);

        assert_eq!(
            results
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .expect("fallback serial calls"),
            ["0", "1", "2", "3"]
        );
        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn batch_from_js_returns_structured_results() {
        // End-to-end through V8: batch() returns parsed objects, in order, with
        // per-item ok/error, all in one run.
        let out = run_ok(
            "const r = batch([{tool:'double',args:{n:5}},{tool:'boom'},{tool:'double',args:{n:6}}]); \
             return r.map(x => x.ok ? x.output : 'ERR').join(',');",
        );
        assert_eq!(out.result, "10,ERR,12");
    }

    #[test]
    fn batch_scheduler_can_force_effects_serial_in_declaration_order() {
        let started = std::time::Instant::now();
        let out = run(
            "const r=batch([1,2,3,4].map(()=>({tool:'sleep',args:{ms:40}}))); \
             return r.length;",
            &["sleep".into()],
            &slow_mock,
            &parallel_none,
            Duration::from_secs(5),
            128,
        )
        .expect("serial batch");
        assert_eq!(out.result, "4");
        assert!(
            started.elapsed() >= Duration::from_millis(140),
            "scheduler-fenced batch unexpectedly overlapped: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn batch_rejects_oversized_spec_before_dispatch() {
        let err = run(
            "return batch(Array.from({length:49},()=>({tool:'double',args:{n:1}})));",
            &["double".into()],
            &mock,
            &parallel_all,
            Duration::from_secs(5),
            128,
        )
        .unwrap_err();
        assert!(
            err.contains("batch has 49 items; limit is 48"),
            "got: {err}"
        );
    }

    #[test]
    fn is_js_ident_filters() {
        assert!(is_js_ident("read_file"));
        assert!(is_js_ident("_x"));
        assert!(!is_js_ident("read-file"));
        assert!(!is_js_ident("2cool"));
        assert!(!is_js_ident(""));
    }

    /// Scenario: a realistic "mass-test" program — map inputs to specs, run them
    /// concurrently with `batch()` (one failure mixed in), aggregate the
    /// successes, return JSON. Exercises loop + map + batch + per-item error +
    /// JSON return in a single turn (the marquee code-mode use case).
    #[test]
    fn mass_test_aggregation_program() {
        let out = run_ok(
            "const inputs = [1,2,3,4,5];\n\
             const specs = inputs.map(n => ({tool:'double', args:{n}}));\n\
             specs.push({tool:'boom'});\n\
             const results = batch(specs);\n\
             let sum = 0, fails = 0;\n\
             for (const r of results) { if (r.ok) sum += parseInt(r.output,10); else fails++; }\n\
             return JSON.stringify({sum, fails, count: results.length});",
        );
        assert_eq!(
            out.result, r#"{"sum":30,"fails":1,"count":6}"#,
            "got: {}",
            out.result
        );
    }
}
