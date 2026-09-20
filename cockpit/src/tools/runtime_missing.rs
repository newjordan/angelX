//! Shared onboarding diagnostics carried through the string-valued tool API.
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct RuntimeMissing {
    pub(crate) kind: String,
    pub(crate) runtime: String,
    pub(crate) looked_for: String,
    pub(crate) hint: String,
    pub(crate) message: String,
}

impl RuntimeMissing {
    pub(crate) fn new(runtime: &str, looked_for: impl Into<String>) -> Self {
        let (runtime, pin, install) = match runtime {
            "node" | "npm" => ("node", "ANGEL_NODE_BIN", "Node.js >=20.19"),
            "python" | "python3" => ("python3", "ANGEL_PYTHON_BIN", "Python 3"),
            "cargo" => ("cargo", "ANGEL_CARGO_BIN", "Rust with cargo and rustc"),
            _ => (runtime, "ANGEL_GO_BIN", "Go"),
        };
        let looked_for = looked_for.into();
        let hint = format!(
            "install {install} or set {pin}=/absolute/path/to/{runtime}; restart Angel to capture the runtime"
        );
        Self {
            kind: "runtime_missing".into(),
            runtime: runtime.into(),
            message: format!("missing {runtime}: looked for {looked_for}; {hint}"),
            looked_for,
            hint,
        }
    }

    pub(crate) fn encode(&self) -> String {
        serde_json::to_string(self).expect("runtime diagnostic is serializable")
    }

    pub(crate) fn decode(text: &str) -> Option<Self> {
        let start = text.find("{\"kind\":\"runtime_missing\"")?;
        let mut stream = serde_json::Deserializer::from_str(&text[start..]).into_iter::<Self>();
        stream
            .next()?
            .ok()
            .filter(|error| error.kind == "runtime_missing")
    }
}

pub(crate) fn pin(program: &str) -> Option<&'static str> {
    match program {
        "node" => Some("ANGEL_NODE_BIN"),
        "python3" => Some("ANGEL_PYTHON_BIN"),
        "cargo" => Some("ANGEL_CARGO_BIN"),
        _ => None,
    }
}

pub(crate) fn resolve(program: &str) -> Result<std::path::PathBuf, String> {
    if let Some(pin) = pin(program)
        && let Some(value) = std::env::var_os(pin)
    {
        let path = std::path::PathBuf::from(value);
        if !path.is_absolute() {
            return Err(format!("{pin} must be an absolute executable path"));
        }
        if !path.is_file() {
            return Err(RuntimeMissing::new(program, format!("{pin}={}", path.display())).encode());
        }
        return Ok(path);
    }
    crate::workspace_lang::resolve_on_path(program).ok_or_else(|| match pin(program) {
        Some(pin) => {
            RuntimeMissing::new(program, format!("PATH ({program}); {pin} unset")).encode()
        }
        None => format!("{program} not found on PATH"),
    })
}

/// Only inspect a failed process's loader/shell diagnostics, never arbitrary
/// mentions of a runtime in successful test output.
pub(crate) fn from_command_not_found(output: &str) -> Option<String> {
    for runtime in ["node", "python3", "cargo"] {
        if output.lines().any(|line| {
            let line = line.replace(['\'', '‘', '’', '\"'], "");
            line.ends_with(&format!("{runtime}: command not found"))
                || line.ends_with(&format!("{runtime}: not found"))
                || line.contains(&format!(
                    "/usr/bin/env: {runtime}: No such file or directory"
                ))
        }) {
            return Some(
                RuntimeMissing::new(
                    runtime,
                    format!("child PATH ({runtime}); {}", pin(runtime).unwrap(),),
                )
                .encode(),
            );
        }
    }
    None
}

/// A later verifier result supersedes the earlier infrastructure error. Do not
/// change turn policy or prevent the model from recovering before it finishes.
pub(crate) fn unresolved_verifier(tools: &[serde_json::Value]) -> Option<RuntimeMissing> {
    let last = tools
        .iter()
        .rev()
        .find(|tool| matches!(tool["tool"].as_str(), Some("run_tests" | "check" | "lint")))?;
    last["error"].as_str().and_then(RuntimeMissing::decode)
}

#[cfg(test)]
mod tests {
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
}
