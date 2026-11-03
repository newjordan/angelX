//! A browser seat's rules engine: the page (`crates/web/shell/boot.js`) runs
//! the game's shipped Bend modules in its own JavaScript, from the `rules.js`
//! its bundle build served beside it (`crates/web/rules_js.rs`), as a native
//! host runs the same files in V8. A rule asks by module id; the page
//! answers `{value}` or `{error}`.
use serde_json::Value;

#[link(wasm_import_module = "angel")]
unsafe extern "C" {
    /// Render module `id` on the JSON `input`. The page keeps its answer and
    /// says how many bytes it is.
    fn rules(id: *const u8, id_len: usize, input: *const u8, input_len: usize) -> u32;
    /// Copy the page's kept answer into `out`, which holds as many bytes as
    /// `rules` said.
    fn rules_take(out: *mut u8);
}

pub(super) fn engine(id: &str, input: &Value) -> Result<Value, String> {
    let input = input.to_string();
    // SAFETY: the page reads `id` and `input` during the call, and writes
    // exactly the length it returned into `answer`, during the next.
    let len = unsafe { rules(id.as_ptr(), id.len(), input.as_ptr(), input.len()) } as usize;
    let mut answer = vec![0u8; len];
    unsafe { rules_take(answer.as_mut_ptr()) };
    let mut envelope: Value =
        serde_json::from_slice(&answer).map_err(|e| format!("{id}: the page's answer: {e}"))?;
    match envelope.get_mut("value").map(Value::take) {
        Some(value) => Ok(value),
        None => Err(envelope["error"]
            .as_str()
            .unwrap_or("the page's rules answered no value")
            .to_string()),
    }
}
