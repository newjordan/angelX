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
//! This crate has no cockpit dependencies. The host supplies tool dispatch,
//! batch scheduling, identity propagation, and the construction limit. Keep V8
//! generics here so ordinary cockpit edits do not typecheck or codegen them.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::time::Duration;

use serde_json::Value;

type ToolInvoker<'a> = dyn Fn(&str, &Value) -> Result<String, String> + Sync + 'a;
type BatchRunner<'a> = dyn Fn(&[(String, Value)]) -> Vec<Result<String, String>> + Sync + 'a;
type BatchItemCap<'a> = dyn Fn() -> usize + Sync + 'a;

fn init_v8() {
    angel_live::v8::script::init();
}

struct HostCtx<'a> {
    invoke: &'a ToolInvoker<'a>,
    batch_runner: &'a BatchRunner<'a>,
    batch_item_cap: &'a BatchItemCap<'a>,
    console: RefCell<Vec<String>>,
    console_bytes: Cell<usize>,
    console_limit: usize,
    console_truncated: Cell<bool>,
}

unsafe fn ctx_from<'c>(data: v8::Local<v8::Value>) -> &'c HostCtx<'c> {
    unsafe {
        let ext = data.cast::<v8::External>();
        &*(ext.value() as *const HostCtx<'c>)
    }
}

fn throw(scope: &mut v8::PinScope, msg: &str) {
    if let Some(s) = v8::String::new(scope, msg) {
        let exc = v8::Exception::error(scope, s);
        scope.throw_exception(exc);
    }
}

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
    let max_batch_items = (ctx.batch_item_cap)();
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
    let results = (ctx.batch_runner)(&specs);
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

#[derive(Debug, Default, Clone)]
pub struct Outcome {
    pub result: String,
    pub console: Vec<String>,
    pub console_truncated: bool,
}

unsafe extern "C" fn near_heap_limit_cb(
    data: *mut c_void,
    current: usize,
    _initial: usize,
) -> usize {
    if !data.is_null() {
        let handle = unsafe { &*(data as *const v8::IsolateHandle) };
        handle.terminate_execution();
    }
    current + (current / 2).max(8 << 20)
}

pub fn run(
    script: &str,
    tool_names: &[String],
    invoke: &ToolInvoker<'_>,
    batch_runner: &BatchRunner<'_>,
    timeout: Duration,
    heap_mb: usize,
    batch_item_cap: &BatchItemCap<'_>,
) -> Result<Outcome, String> {
    init_v8();

    let ctx = HostCtx {
        invoke,
        batch_runner,
        batch_item_cap,
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
    isolate.set_microtasks_policy(v8::MicrotasksPolicy::Explicit);

    let heap_data = Box::into_raw(Box::new(isolate.thread_safe_handle())) as *mut c_void;
    isolate.add_near_heap_limit_callback(near_heap_limit_cb, heap_data);

    let handle = isolate.thread_safe_handle();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let watchdog = std::thread::spawn(move || {
        if done_rx.recv_timeout(timeout).is_err() {
            handle.terminate_execution();
        }
    });

    let out = run_inner(isolate, script, tool_names, &ctx);

    isolate.remove_near_heap_limit_callback(near_heap_limit_cb, 0);
    unsafe { drop(Box::from_raw(heap_data as *mut v8::IsolateHandle)) };

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

    let ext = v8::External::new(scope, ctx as *const HostCtx as *mut c_void);
    bind_fn(scope, context, "__angel_invoke", invoke_cb, ext);
    bind_fn(scope, context, "__angel_log", log_cb, ext);
    bind_fn(scope, context, "__angel_batch", batch_cb, ext);

    let source = build_source(tool_names, user_script);
    compile_run(scope, &source)?;
    drain(scope, context)?;
    read_result(scope, context)
}

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

fn is_js_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn build_source(tool_names: &[String], user_script: &str) -> String {
    let mut p = String::new();
    p.push_str(
        "globalThis.console={log:(...a)=>__angel_log(...a.map(String)),\
         error:(...a)=>__angel_log(...a.map(String)),\
         warn:(...a)=>__angel_log(...a.map(String)),\
         info:(...a)=>__angel_log(...a.map(String)),\
         debug:(...a)=>__angel_log(...a.map(String))};\n",
    );
    p.push_str(
        "globalThis.tool=(name,args)=>__angel_invoke(String(name),JSON.stringify(args??{}));\n",
    );
    p.push_str("globalThis.batch=(calls)=>JSON.parse(__angel_batch(JSON.stringify(calls??[])));\n");
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
    for name in tool_names {
        if is_js_ident(name) {
            p.push_str(&format!(
                "globalThis[{name:?}]=(args)=>__angel_invoke({name:?},JSON.stringify(args??{{}}));\n",
            ));
        }
    }
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

fn caught(tc: &mut v8::PinnedRef<'_, v8::TryCatch<v8::HandleScope>>) -> String {
    match tc.exception() {
        Some(e) => e.to_rust_string_lossy(tc),
        None => "(unknown)".to_string(),
    }
}

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
    Err("code_mode: async script did not settle before microtask drain limit".to_string())
}

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
#[path = "../../../tests/cockpit/code_mode_runtime.rs"]
mod code_mode_runtime;
