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
