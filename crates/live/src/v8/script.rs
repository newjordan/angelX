//! Low-level V8 helpers shared by the live-module evaluator and the cockpit's
//! model-facing code-mode runner: one-time platform start, the heap guard, the
//! prelude that wraps a script in an async result-capture harness, and the
//! compile / drain / read-back steps. Both callers keep their own host
//! callbacks; only the mechanics that must not drift live here.
use ::v8;
use std::ffi::c_void;
use std::sync::Once;

/// One-time process-global V8 platform initialisation. `Once` makes it safe to
/// call from every run and from parallel test threads; V8 is never disposed.
static V8_INIT: Once = Once::new();

pub fn init() {
    V8_INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

/// Throw a JS `Error(msg)` in the current scope.
pub fn throw(scope: &mut v8::PinScope, msg: &str) {
    if let Some(s) = v8::String::new(scope, msg) {
        let exc = v8::Exception::error(scope, s);
        scope.throw_exception(exc);
    }
}

/// Near-heap-limit guard. Fired on the isolate thread as the capped heap nears
/// exhaustion: terminate the running script and hand V8 headroom (raise the
/// limit) so it can unwind and surface an `Err` instead of calling
/// `FatalProcessOutOfMemory`, which would ABORT the whole process. `data` is a
/// `*const IsolateHandle` boxed by the isolate's owner (valid until the owner
/// removes this callback). The `v8` 150 `NearHeapLimitCallback` type is
/// `unsafe extern "C"`, hence the signature here.
///
/// # Safety
/// `data` must be null or point to a live `v8::IsolateHandle`.
pub unsafe extern "C" fn near_heap_limit_cb(
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

/// Install a native function `name` on the global object, carrying `data` as
/// its callback data.
pub fn bind_fn(
    scope: &mut v8::PinScope,
    context: v8::Local<v8::Context>,
    name: &str,
    cb: impl v8::MapFnTo<v8::FunctionCallback>,
    data: v8::Local<v8::Value>,
) {
    let func = v8::Function::builder(cb)
        .data(data)
        .build(scope)
        .expect("build native fn");
    let key = v8::String::new(scope, name).expect("fn name");
    context.global(scope).set(scope, key.into(), func.into());
}

/// Whether `name` is a plain JS identifier we can expose as `globalThis.name`.
pub fn is_js_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Build the full source: a trusted prelude (console + per-tool wrappers + the
/// async result-capture harness) wrapping the verbatim user script.
pub fn build_source(tool_names: &[String], user_script: &str) -> String {
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
pub fn compile_run(scope: &mut v8::PinScope, src: &str) -> Result<(), String> {
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
pub fn drain(scope: &mut v8::PinScope, context: v8::Local<v8::Context>) -> Result<(), String> {
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

/// Resolve the run's outcome from the harness globals: `__angel_error` (if set)
/// → `Err`, else the stringified `__angel_result`.
pub fn read_result(
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
