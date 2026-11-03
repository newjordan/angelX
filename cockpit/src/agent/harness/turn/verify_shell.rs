//! Bounded shell verification classification in a reloadable native module.
use crate::platform::native_module::{self, Library, ReloadConfig, ReloadHost};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

type Verify = unsafe extern "C" fn(*const u8, usize, extern "C" fn(u32) -> i32, i32) -> i32;
unsafe extern "C" {
    fn angel_verify_shell_v1(
        input: *const u8,
        len: usize,
        whitespace: extern "C" fn(u32) -> i32,
        mode: i32,
    ) -> i32;
}
struct Generation {
    verify: Verify,
    _library: Option<Library>,
}
fn baseline() -> Arc<Generation> {
    static BASE: OnceLock<Arc<Generation>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(Generation {
            verify: angel_verify_shell_v1,
            _library: None,
        })
    }))
}
fn load(bytes: &[u8]) -> Result<Arc<Generation>, String> {
    let library = native_module::load(bytes)?;
    let version = library.symbol(c"angel_verify_shell_abi_version");
    let verify = library.symbol(c"angel_verify_shell_v1");
    if version.is_null() || verify.is_null() {
        return Err("missing verifier shell ABI symbols".into());
    }
    let version: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(version) };
    if unsafe { version() } != 1 {
        return Err("unsupported verifier shell ABI".into());
    }
    Ok(Arc::new(Generation {
        verify: unsafe { std::mem::transmute(verify) },
        _library: Some(library),
    }))
}
fn new_host(path: Option<std::path::PathBuf>) -> ReloadHost<Generation> {
    ReloadHost::new(
        path,
        baseline(),
        load,
        ReloadConfig {
            label: "verifier shell",
            thread_name: "angel-verifier-reload",
            fallback: "built-in classifier",
            worker_subject: "verifier shell",
            missing_is_unavailable: false,
            byte_limit: 16 * 1024 * 1024,
            interval: Duration::from_millis(500),
        },
    )
}
fn host() -> &'static ReloadHost<Generation> {
    static HOST: OnceLock<ReloadHost<Generation>> = OnceLock::new();
    HOST.get_or_init(|| {
        new_host(
            std::env::var_os("ANGEL_VERIFY_SHELL_MODULE")
                .filter(|v| !v.is_empty())
                .map(Into::into),
        )
    })
}
extern "C" fn whitespace(c: u32) -> i32 {
    char::from_u32(c).is_some_and(char::is_whitespace) as i32
}
fn assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
/// Avoid an FFI call for the common, simple test command with an environment
/// prefix. It is enabled only for the built-in generation.
fn fast_assignment_cargo(command: &str) -> Option<bool> {
    if !command
        .as_bytes()
        .first()
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
    {
        return None;
    }
    let first = command.split_whitespace().next()?;
    if !assignment(first) {
        return None;
    }
    if command.bytes().any(|c| {
        matches!(
            c,
            b'|' | b'&'
                | b';'
                | b'\n'
                | b'\r'
                | b'\''
                | b'"'
                | b'\\'
                | b'#'
                | b'`'
                | b'$'
                | b'<'
                | b'>'
                | b'('
                | b')'
                | b'{'
                | b'}'
        )
    }) {
        return None;
    }
    let words: Vec<_> = command
        .split_whitespace()
        .skip_while(|word| assignment(word))
        .collect();
    if !words.first().is_some_and(|p| {
        p.rsplit('/')
            .next()
            .unwrap_or(p)
            .eq_ignore_ascii_case("cargo")
    }) {
        return None;
    }
    Some(words.get(1).is_some_and(|w| {
        w.eq_ignore_ascii_case("test")
            || w.eq_ignore_ascii_case("check")
            || w.eq_ignore_ascii_case("clippy")
            || (w.eq_ignore_ascii_case("fmt")
                && words.iter().any(|w| w.eq_ignore_ascii_case("--check")))
    }))
}
/// 0 = status-bearing verifier, 1 = test anywhere, 2 = status-bearing build.
/// Invalid native results use the embedded implementation; failure there is
/// conservative and cannot manufacture a verification receipt.
pub(super) fn classify(command: &str, mode: i32) -> bool {
    if mode != 1 && command.contains("||") {
        return false;
    }
    let host = host();
    if mode == 1 && matches!(host, ReloadHost::Baseline(_)) {
        if let Some(result) = fast_assignment_cargo(command) {
            return result;
        }
    }
    let result = match host {
        ReloadHost::Baseline(generation) => classify_with(generation, command, mode),
        ReloadHost::Reloadable(_) => classify_with(&host.current(), command, mode),
    };
    result
        .or_else(|| classify_with(&baseline(), command, mode))
        .unwrap_or(false)
}
fn classify_with(generation: &Generation, command: &str, mode: i32) -> Option<bool> {
    match unsafe { (generation.verify)(command.as_ptr(), command.len(), whitespace, mode) } {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/verify_shell__tests.rs"]
mod tests;
