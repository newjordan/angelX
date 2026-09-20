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
    crate::platform::workspace_lang::resolve_on_path(program).ok_or_else(|| match pin(program) {
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
#[path = "../../../../tests/cockpit/tools/runtime_missing__tests.rs"]
mod tests;
