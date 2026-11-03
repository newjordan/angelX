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
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/cockpit/fixtures/tool-error-cases.json");
    let rows: Vec<(String, String, String)> =
        serde_json::from_slice(&std::fs::read(path).expect("tool-error cases fixture"))
            .expect("valid tool-error cases fixture");
    assert_eq!(rows.len(), 19, "tool-error case count");
    let cases = rows.iter().map(|(tool, error, expected)| {
        let expected = match expected.as_str() {
            "Argument" => Argument,
            "Routing" => Routing,
            "Policy" => Policy,
            "Environment" => Environment,
            "Transient" => Transient,
            "Verifier" => Verifier,
            "Model" => Model,
            "Unknown" => Unknown,
            _ => panic!("unknown tool-error case class: {expected}"),
        };
        (tool.as_str(), error.as_str(), expected)
    });
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
