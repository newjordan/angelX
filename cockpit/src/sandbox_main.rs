//! Tiny sibling helper for sandboxed tool spawns.
//!
//! The cockpit binary links the full video/graphics stack, so re-execing it
//! for every `shell` tool call costs ~28 ms of dynamic-loader work before the
//! sandbox is even installed. This binary keeps the exact same `--sandbox-exec`
//! contract with only the sandbox module plus the two crate-level shims it
//! references; `sandbox::helper_executable` prefers it beside the cockpit
//! binary and falls back to re-execing the cockpit itself.

#![allow(dead_code)] // the helper compiles the whole sandbox module but only
// `exec_helper` is reachable from `main`.

#[path = "sandbox.rs"]
mod sandbox;

/// Shim for `crate::harness::env_flag` — copied byte-for-byte from
/// `src/harness/context.rs`; the helper must not pull the harness in.
mod harness {
    pub(crate) fn env_flag(key: &str, default: bool) -> bool {
        match std::env::var(key) {
            Ok(v) => {
                let v = v.trim().to_ascii_lowercase();
                !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
            }
            Err(_) => default,
        }
    }
}

/// Shim for `crate::tests::env_lock` — the bwrap tests serialize process-env
/// access through the crate-level lock; the helper's test target needs the
/// same single lock (same contract: poison-tolerant, no protected invariant).
#[cfg(test)]
#[path = "../../tests/cockpit/app/sandbox_main__tests.rs"]
mod tests;

/// Shim for `crate::yolo::enabled` — env-reading logic copied from
/// `src/yolo.rs` (`parse(ANGEL_YOLO)` ⇒ Full ⇒ enabled). The helper inherits
/// `ANGEL_YOLO` from its parent, so behaviour must match byte-for-byte.
mod yolo {
    pub(crate) fn enabled() -> bool {
        match std::env::var("ANGEL_YOLO") {
            Ok(value) => {
                let value = value.trim().to_ascii_lowercase();
                !matches!(
                    value.as_str(),
                    "" | "0" | "false" | "no" | "off" | "disable" | "disabled"
                )
            }
            Err(_) => false,
        }
    }
}

fn main() -> std::io::Result<()> {
    let raw_os_args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if raw_os_args
        .first()
        .is_some_and(|arg| arg == std::ffi::OsStr::new("--sandbox-exec"))
    {
        return sandbox::exec_helper(raw_os_args.into_iter().skip(1));
    }
    eprintln!("usage: angel-sandbox --sandbox-exec -- program [args…]");
    std::process::exit(2);
}
