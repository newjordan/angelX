//! Cause labels for observed tool failures. Pure, conservative, and independent
//! of the older experience buckets. Text alone cannot prove a runtime shim exists.
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum ToolErrorClass {
    None,
    Argument,
    Routing,
    Policy,
    Environment,
    Transient,
    Verifier,
    Model,
    Unknown,
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn typed_alternative(tool: &str, args: &Value) -> bool {
    if tool != "shell" {
        return false;
    }
    let cmd = args
        .get("cmd")
        .or_else(|| args.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim_start();
    [
        "cat ",
        "sed -n ",
        "cargo test",
        "cargo check",
        "cargo clippy",
        "cargo fmt",
    ]
    .iter()
    .any(|prefix| cmd.starts_with(prefix))
}

/// `err == None` means a successful call, not a missing diagnostic. Callers
/// also supply non-error verifier failures/timeouts as observed failures.
pub(crate) fn classify_tool_error(tool: &str, args: &Value, err: Option<&str>) -> ToolErrorClass {
    use ToolErrorClass::*;
    let Some(err) = err else { return None };
    if crate::tools::runtime_missing::RuntimeMissing::decode(err).is_some() {
        return Environment;
    }
    if err.contains("stalled {") {
        return Transient;
    }
    let text = err.to_ascii_lowercase();
    if text.trim().is_empty() {
        // A failed dispatch that produced no diagnostic at all is never agent
        // error: a command-shaped tool failed silently (verifier-like command
        // failure), while anything else lost its reason between spawn and
        // receipt (environment).
        return if tool == "shell" {
            Verifier
        } else {
            Environment
        };
    }
    if contains_any(
        &text,
        &[
            "malformed tool call",
            "unrecoverable tool arguments",
            "invalid tool call json",
            "hallucinated capability",
        ],
    ) {
        return Model;
    }
    if contains_any(&text, &["unknown tool:", "unknown tool "]) {
        return Routing;
    }
    // Invalid scope schema is an argument error; an enforced valid scope is policy.
    if contains_any(
        &text,
        &[
            "must be a boolean",
            "must be an array",
            "entries must be strings",
            "missing field",
            "missing '",
            "must be a one-based positive integer",
            "must not be empty",
            "requires integer",
            "schema validation",
            "missing argument",
            "missing required",
            "invalid argument",
            "out of range",
            "out-of-range",
            "empty cargo args",
            "trailing backslash escape",
            "unterminated",
            "requires a handle id",
            "requires a workspace path",
            "omit that flag",
            "is not a directory inside the workspace — crates here:",
        ],
    ) {
        return Argument;
    }
    if contains_any(
        &text,
        &[
            "read-only",
            "read only",
            "write_paths: []",
            "approval",
            "denied by",
            "action capsule denied",
            "confinement",
            "not confined",
            "mount not confined",
            "outside the workspace",
            "stay inside the workspace",
            "escaped workspace",
            "path traversal",
            "cannot grant",
            "cannot traverse symlink",
            "typed cargo verification is disabled while yolo removes toolchain immutability",
        ],
    ) || (args
        .get("write_paths")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
        && contains_any(
            &text,
            &[
                "permission denied",
                "operation not permitted",
                "temporary-directory failure",
                "read-only file system",
            ],
        ))
    {
        return Policy;
    }
    // Missing runtimes are never legitimate red verifier results.
    if contains_any(
        &text,
        &[
            "exit 127",
            "exit=127",
            "exit code 127",
            "enoent",
            "no such file or directory",
            "command not found",
            "missing runtime",
            "permission denied",
            "spawn failed:",
            "unavailable in the pinned toolchain",
            "toolchain changed since pin",
            "re-pin the toolchain",
        ],
    ) {
        return Environment;
    }
    if contains_any(
        &text,
        &[
            "timed out",
            "timeout",
            "provider error",
            "stream error",
            "http transport error",
            "wait thread died",
        ],
    ) {
        return Transient;
    }
    if typed_alternative(tool, args) {
        return Routing;
    }
    // A failed/inconclusive verification can carry passing host test output.
    // This identifies a verifier outcome, not trusted evidence or a test failure.
    // Successful calls have already returned None above.
    if tool == "shell" && has_test_summary(&text) {
        return Verifier;
    }
    if matches!(
        tool,
        "run_tests" | "check" | "lint" | "cargo" | "machine_test" | "fmt"
    ) && contains_any(&text, &["failed", "tests: fail", "build: fail"])
    {
        return Verifier;
    }
    Unknown
}

fn has_test_summary(text: &str) -> bool {
    let rust = text.lines().any(|line| {
        (line.starts_with("test result: ok. ") || line.starts_with("test result: failed. "))
            && line.contains(" passed; ")
            && line.contains(" failed; ")
    });
    let node_count = |prefix: &str| {
        text.lines().any(|line| {
            line.strip_prefix(prefix)
                .is_some_and(|count| !count.is_empty() && count.bytes().all(|b| b.is_ascii_digit()))
        })
    };
    rust || (node_count("ℹ tests ") && node_count("ℹ pass ") && node_count("ℹ fail "))
}

pub(crate) fn avoidable(
    class: ToolErrorClass,
    tool: &str,
    args: &Value,
    err: Option<&str>,
) -> bool {
    match class {
        ToolErrorClass::Argument | ToolErrorClass::Routing => true,
        ToolErrorClass::Policy => typed_alternative(tool, args),
        // Require explicit evidence; never infer that a shim is installed merely
        // because a typed tool exists.
        ToolErrorClass::Environment => err.is_some_and(|e| e.contains("available shim:")),
        _ => false,
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/tool_errors__tests.rs"]
mod tests;
