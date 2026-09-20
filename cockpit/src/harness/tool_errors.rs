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
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_errors_invalid_verifier_directory() {
        let error = "tool error: run_tests: `tmp/arena/task` is not a directory inside the workspace — crates here: (none)";
        let args = json!({"dir":"/tmp/arena/task"});
        let class = classify_tool_error("run_tests", &args, Some(error));
        assert_eq!(class, ToolErrorClass::Argument);
        assert!(avoidable(class, "run_tests", &args, Some(error)));
    }

    #[test]
    fn tool_errors_empty_diagnostic_is_never_agent_error() {
        for empty in ["", " \n\t"] {
            assert_eq!(
                classify_tool_error("shell", &json!({"cmd":"make check"}), Some(empty)),
                ToolErrorClass::Verifier,
                "{empty:?}"
            );
            assert_eq!(
                classify_tool_error("cargo", &json!({}), Some(empty)),
                ToolErrorClass::Environment,
                "{empty:?}"
            );
            assert!(!avoidable(
                ToolErrorClass::Verifier,
                "shell",
                &json!({"cmd":"make check"}),
                Some(empty)
            ));
        }
    }

    #[test]
    fn tool_errors_toolchain_pin_drift_is_policy_adjacent_environment() {
        let error = "tool error: spawn `cargo test` failed: trusted Cargo unavailable: pinned Cargo changed at /bin/cargo (captured sha256 abc); toolchain changed since pin; restart Angel to re-pin the toolchain";
        assert_eq!(
            classify_tool_error("run_tests", &json!({}), Some(error)),
            ToolErrorClass::Environment
        );
    }

    #[test]
    fn tool_errors_yolo_verification_policy() {
        let error = "tool error: typed Cargo verification is disabled while YOLO removes toolchain immutability; turn YOLO off for trusted evidence";
        let args = json!({"args":["test"]});
        let class = classify_tool_error("cargo", &args, Some(error));
        assert_eq!(class, ToolErrorClass::Policy);
        assert!(!avoidable(class, "cargo", &args, Some(error)));
    }

    #[test]
    fn tool_errors_inconclusive_rust_host_summary() {
        let error = "running 2 tests\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s";
        assert_eq!(
            classify_tool_error("shell", &json!({}), Some(error)),
            ToolErrorClass::Verifier
        );
        assert_eq!(
            classify_tool_error("shell", &json!({}), None),
            ToolErrorClass::None
        );
        assert_eq!(
            classify_tool_error("shell", &json!({"cmd":"cargo test"}), Some(error)),
            ToolErrorClass::Routing
        );
        assert_eq!(
            classify_tool_error("read_file", &json!({}), Some(error)),
            ToolErrorClass::Unknown
        );
    }

    #[test]
    fn tool_errors_inconclusive_node_host_summary() {
        let error = "✔ test\nℹ tests 3\nℹ suites 0\nℹ pass 3\nℹ fail 0\nℹ duration_ms 43.017736";
        assert_eq!(
            classify_tool_error("shell", &json!({}), Some(error)),
            ToolErrorClass::Verifier
        );
        for error in [
            "ℹ tests 3",
            "ℹ tests x\nℹ pass 3\nℹ fail 0",
            "execution cancelled before spawn",
        ] {
            assert_eq!(
                classify_tool_error("shell", &json!({}), Some(error)),
                ToolErrorClass::Unknown
            );
        }
        assert_eq!(
            classify_tool_error(
                "shell",
                &json!({}),
                Some(&format!("permission denied\n{error}"))
            ),
            ToolErrorClass::Environment
        );
    }

    #[test]
    fn tool_errors_harvested_causes_and_boundaries() {
        use ToolErrorClass::*;
        let cases = [
            // tools/shell/scope.rs:16,23
            ("shell", "read_only must be a boolean", Argument),
            (
                "shell",
                "write_paths must be an array of at most 128 existing source files",
                Argument,
            ),
            // tools/file.rs:550,561,222
            ("read_file", "missing 'path'", Argument),
            (
                "read_file",
                "'offset' must be a one-based positive integer",
                Argument,
            ),
            (
                "read_file",
                "index 3 out of range at segment \"3\"",
                Argument,
            ),
            // harness/registry.rs:1150; tools routed by turn/verify.rs:28
            ("imaginary", "unknown tool: imaginary", Routing),
            ("other", "unknown tool: other", Routing),
            // tools/shell/scope.rs:29,71
            (
                "shell",
                "read-only execution cannot grant write_paths",
                Policy,
            ),
            (
                "shell",
                "write_paths must stay inside the workspace",
                Policy,
            ),
            // harness/exec.rs:864 (OS error); tools/build.rs:1326 (exit code).
            (
                "shell",
                "spawn failed: No such file or directory (os error 2)",
                Environment,
            ),
            ("run_tests", "cargo command failed (exit 127)", Environment),
            // harness/exec.rs:487,1099
            (
                "shell",
                "[timed out after 120s — process killed; raise/disable via ANGEL_TOOL_TIMEOUT]",
                Transient,
            ),
            ("shell", "wait thread died", Transient),
            // tools/build.rs:1326,1794
            ("cargo", "cargo command failed (exit 101)", Verifier),
            (
                "run_tests",
                "pytest failed (exit 1) — chosen because of pytest.ini; pin another runner with `runner` or use `shell`",
                Verifier,
            ),
            // harness/registry.rs:1111-1112 via club/tool_parse.rs:148.
            (
                "shell",
                "unrecoverable tool arguments (3 bytes, digest unavailable); reissue `shell` with valid JSON",
                Model,
            ),
            (
                "read_file",
                "unrecoverable tool arguments (7 bytes, digest unavailable); reissue `read_file` with valid JSON",
                Model,
            ),
            // harness/exec.rs:827; tools/build.rs:990: intentionally no guessed cause.
            ("shell", "execution cancelled before spawn", Unknown),
            (
                "cargo",
                "internal error: non-verifier requested trusted Cargo mode",
                Unknown,
            ),
        ];
        for (tool, error, expected) in cases {
            let actual = classify_tool_error(tool, &json!({}), Some(error));
            assert_eq!(actual, expected, "{error}");
            if matches!(actual, Verifier | Transient) {
                assert!(!avoidable(actual, tool, &json!({}), Some(error)));
            }
        }
        for tool in ["read_file", "shell"] {
            assert_eq!(classify_tool_error(tool, &json!({}), Option::None), None);
        }
        // DIAGNOSIS.md quotes scope arguments, not verbatim OS error bodies.
        let args = json!({"read_only": false, "write_paths": [], "cmd": "cargo test"});
        assert_eq!(
            classify_tool_error("shell", &args, Some("temporary-directory failure")),
            Policy
        );
        assert!(avoidable(Policy, "shell", &args, Option::None));
        assert!(!avoidable(
            Policy,
            "shell",
            &json!({"cmd":"curl example"}),
            Option::None
        ));
        assert_eq!(
            classify_tool_error("shell", &args, Some("write_paths: []")),
            Policy
        );
        for cmd in [
            "cat src/main.rs",
            "sed -n '1,20p' src/main.rs",
            "cargo test",
        ] {
            assert_eq!(
                classify_tool_error("shell", &json!({"cmd":cmd}), Some("tool error: failed")),
                Routing
            );
            assert_eq!(
                classify_tool_error("shell", &json!({"cmd":cmd}), Option::None),
                None
            );
        }
        assert!(!avoidable(Environment, "shell", &args, Some("exit 127")));
        assert!(avoidable(
            Environment,
            "shell",
            &args,
            Some("exit 127; available shim: pinned cargo")
        ));
    }
}
