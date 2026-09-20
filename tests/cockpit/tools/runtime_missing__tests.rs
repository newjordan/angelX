use super::*;

#[test]
fn runtime_missing_envelope_recovers_after_successful_verifier() {
    let mut tools = vec![serde_json::json!({
        "tool": "run_tests", "error": format!("tool error: {}", RuntimeMissing::new("cargo", "PATH").encode()),
    })];
    assert_eq!(unresolved_verifier(&tools).unwrap().kind, "runtime_missing");
    tools.push(serde_json::json!({"tool": "read_file"}));
    assert!(unresolved_verifier(&tools).is_some());
    tools.push(serde_json::json!({"tool": "run_tests", "verify": "passed"}));
    assert!(unresolved_verifier(&tools).is_none());
}

#[test]
fn runtime_missing_round_trips_wrapped_tool_errors() {
    for runtime in ["node", "python3", "cargo"] {
        let error = RuntimeMissing::new(runtime, "PATH");
        let wrapped = format!("tool error: spawn failed: {}; context", error.encode());
        let parsed = RuntimeMissing::decode(&wrapped).unwrap();
        assert_eq!(parsed.runtime, runtime);
        assert!(parsed.hint.contains(pin(runtime).unwrap()));
        assert!(parsed.message.contains("install"));
    }
    assert!(RuntimeMissing::decode("permission denied").is_none());
}

#[test]
fn runtime_missing_loader_diagnostics_are_specific() {
    for (text, runtime) in [
        ("/usr/bin/env: ‘node’: No such file or directory", "node"),
        ("/bin/bash: line 1: python3: command not found", "python3"),
        ("/bin/sh: 1: cargo: not found", "cargo"),
    ] {
        assert_eq!(
            RuntimeMissing::decode(&from_command_not_found(text).unwrap())
                .unwrap()
                .runtime,
            runtime
        );
    }
    assert!(from_command_not_found("node: test file not found").is_none());
    assert!(from_command_not_found("cargo: Permission denied").is_none());
}

#[test]
fn runtime_missing_explicit_pin_never_falls_back() {
    let _env = crate::tests::env_lock();
    let missing = std::env::temp_dir().join("d06c-absent-runtime");
    let _pin = crate::tests::TestEnvGuard::set("ANGEL_NODE_BIN", &missing.to_string_lossy());
    let error = RuntimeMissing::decode(&resolve("node").unwrap_err()).unwrap();
    assert_eq!(error.runtime, "node");
    assert!(error.looked_for.contains("ANGEL_NODE_BIN="));
}
