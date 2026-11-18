//! Fixed native verifier adapters. Project scripts never become executable identities.

mod execute;
mod plan;
#[cfg(test)]
#[path = "../../../../../../tests/cockpit/tools/build__verifier__tests.rs"]
mod tests;

use super::{
    PinnedExecutable, SandboxPolicy, ToolOutputProgress, canonical_existing_file,
    capture_executable, path_within, revalidate_executable, task_writable_roots,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Tests,
    Check,
    Lint,
}

impl Kind {
    fn tool(self) -> &'static str {
        match self {
            Self::Tests => "run_tests",
            Self::Check => "check",
            Self::Lint => "lint",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Self::Tests => "tests",
            Self::Check => "check",
            Self::Lint => "lint",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Runtime {
    Node,
    Python,
    Go,
}

impl Runtime {
    fn label(self) -> &'static str {
        match self {
            Self::Node => "Node",
            Self::Python => "Python",
            Self::Go => "Go",
        }
    }
}

#[derive(Clone)]
pub(crate) struct PinnedNativeRuntimes {
    inner: Arc<NativeExecutables>,
}

struct NativeExecutables {
    node: Result<PinnedExecutable, String>,
    python: Result<PinnedExecutable, String>,
    go: Result<PinnedExecutable, String>,
}

impl PinnedNativeRuntimes {
    #[cfg(test)]
    pub(crate) fn python_available_for_test(&self) -> bool {
        self.executable(Runtime::Python).is_ok()
    }

    pub(crate) fn capture(workspace: &Path) -> Self {
        std::thread::scope(|scope| {
            let node = std::thread::Builder::new()
                .name("pin-node-verifier".into())
                .spawn_scoped(scope, || capture_runtime(Runtime::Node, workspace));
            let python = std::thread::Builder::new()
                .name("pin-python-verifier".into())
                .spawn_scoped(scope, || capture_runtime(Runtime::Python, workspace));
            let go = capture_runtime(Runtime::Go, workspace);
            let node = match node {
                Ok(worker) => worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
                Err(_) => capture_runtime(Runtime::Node, workspace),
            };
            let python = match python {
                Ok(worker) => worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
                Err(_) => capture_runtime(Runtime::Python, workspace),
            };
            Self {
                inner: Arc::new(NativeExecutables { node, python, go }),
            }
        })
    }

    fn executable(&self, runtime: Runtime) -> Result<&PinnedExecutable, String> {
        match runtime {
            Runtime::Node => &self.inner.node,
            Runtime::Python => &self.inner.python,
            Runtime::Go => &self.inner.go,
        }
        .as_ref()
        .map_err(Clone::clone)
    }
}

fn env_pin(runtime: Runtime) -> &'static str {
    match runtime {
        Runtime::Node => "ANGEL_NODE_BIN",
        Runtime::Python => "ANGEL_PYTHON_BIN",
        Runtime::Go => "ANGEL_GO_BIN",
    }
}

fn is_nvm_versioned_bin(dir: &Path) -> bool {
    let parts = dir
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if parts.len() < 5 || parts.last().map(String::as_str) != Some("bin") {
        return false;
    }
    let version = &parts[parts.len() - 2];
    version.len() > 1
        && version.starts_with('v')
        && version[1..].chars().all(|c| c.is_ascii_digit() || c == '.')
        && parts[parts.len() - 3] == "node"
        && parts[parts.len() - 4] == "versions"
        && parts[parts.len() - 5] == ".nvm"
}

fn runtime_candidates(runtime: Runtime) -> Vec<PathBuf> {
    let names: &[&str] = match runtime {
        Runtime::Node => &[
            "/opt/node",
            "/usr/bin/node",
            "/usr/local/bin/node",
            "/opt/homebrew/bin/node",
        ],
        Runtime::Go => &["/usr/local/go/bin/go", "/usr/bin/go", "/usr/local/bin/go"],
        Runtime::Python => &[
            "/usr/bin/python3",
            "/usr/local/bin/python3",
            "/opt/homebrew/bin/python3",
        ],
    };
    let mut paths = names.iter().map(PathBuf::from).collect::<Vec<_>>();
    if runtime == Runtime::Node
        && let Some(path) = std::env::var_os("PATH")
    {
        for dir in std::env::split_paths(&path).take(128) {
            if is_nvm_versioned_bin(&dir) {
                paths.push(dir.join("node"));
            }
        }
    }
    paths
}

fn capture_runtime(runtime: Runtime, workspace: &Path) -> Result<PinnedExecutable, String> {
    capture_runtime_inner(runtime, workspace).map_err(|reason| {
        let message = format!("{}; {reason}", missing_runtime(runtime));
        if reason == "no eligible runtime found at registry construction"
            || reason.contains("No such file or directory")
        {
            format!(
                "{}; {message}",
                crate::agent::tools::runtime_missing::RuntimeMissing::new(
                    match runtime {
                        Runtime::Node => "node",
                        Runtime::Python => "python3",
                        Runtime::Go => "go",
                    },
                    format!(
                        "{} and trusted installation paths (general PATH is not trusted)",
                        env_pin(runtime)
                    ),
                )
                .encode()
            )
        } else {
            message
        }
    })
}

fn capture_runtime_inner(runtime: Runtime, workspace: &Path) -> Result<PinnedExecutable, String> {
    if let Some(value) = std::env::var_os(env_pin(runtime)) {
        let pin = env_pin(runtime);
        let candidate = PathBuf::from(&value);
        if !candidate.is_absolute() {
            return Err(format!(
                "{pin} must be an absolute path to a native {} executable, got '{}'",
                runtime.label(),
                candidate.display()
            ));
        }
        let path = canonical_existing_file(&candidate, pin)
            .map_err(|reason| format!("{pin} is unusable: {reason}"))?;
        if task_writable_roots(workspace)
            .iter()
            .any(|root| path_within(&path, root))
        {
            return Err(format!(
                "{pin} points inside a task-writable root, refusing to trust it: {}",
                path.display()
            ));
        }
        if !native_image(&path) {
            return Err(format!(
                "{pin} does not point at a native executable image: {}",
                path.display()
            ));
        }
        return capture_executable(path, runtime.label())
            .map_err(|reason| format!("{pin} could not be pinned: {reason}"));
    }
    for candidate in runtime_candidates(runtime) {
        let Ok(path) = canonical_existing_file(&candidate, runtime.label()) else {
            continue;
        };
        if task_writable_roots(workspace)
            .iter()
            .any(|root| path_within(&path, root))
        {
            continue;
        }
        if !native_image(&path) {
            continue;
        }
        if let Ok(executable) = capture_executable(path, runtime.label()) {
            return Ok(executable);
        }
    }
    Err("no eligible runtime found at registry construction".to_string())
}

fn missing_runtime(runtime: Runtime) -> String {
    let install = match runtime {
        Runtime::Node => "/usr/bin/node or …/.nvm/versions/node/vX/bin/node",
        Runtime::Python => "/usr/bin/python3 or /usr/local/bin/python3",
        Runtime::Go => "/usr/local/go/bin/go or /usr/bin/go",
    };
    format!(
        "no trusted {}: set {}=/abs/path or install {install}; trust requires a native executable outside all task-writable roots (no workspace shims); restart to capture the runtime at registry construction",
        runtime.label().to_ascii_lowercase(),
        env_pin(runtime)
    )
}

fn native_image(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if metadata.len() > super::MAX_PINNED_EXECUTABLE_BYTES {
        return false;
    }
    let mut magic = [0; 4];
    if file.read_exact(&mut magic).is_err() {
        return false;
    }
    matches!(
        magic,
        [0x7f, b'E', b'L', b'F']
            | [0xcf, 0xfa, 0xed, 0xfe]
            | [0xfe, 0xed, 0xfa, 0xcf]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [0xbe, 0xba, 0xfe, 0xca]
    )
}

pub(super) fn dispatch(
    runtimes: &PinnedNativeRuntimes,
    kind: Kind,
    args: &Value,
    workspace: &Path,
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
) -> Option<Result<String, String>> {
    let plan = match plan::select(kind, args, workspace) {
        Ok(plan::Selection::Rust) => return None,
        Ok(plan::Selection::Native(plan)) => plan,
        Err(reason) => return Some(Ok(format!("verification inconclusive: {reason}"))),
    };
    let executable = match runtimes.executable(plan.runtime) {
        Ok(executable) => executable,
        Err(reason) => {
            let message = format!("verification inconclusive: {reason}");
            return Some(Ok(message));
        }
    };
    if crate::platform::yolo::enabled() {
        return Some(Err(
            "typed native verification is disabled while YOLO removes runtime immutability; turn YOLO off for trusted evidence".into(),
        ));
    }
    Some(execute::run(
        executable,
        &plan,
        (workspace, args),
        policy,
        cancel,
        progress,
    ))
}

pub(super) fn schema_args(runtime: &str, operation: &str) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "runtime": {"type":"string", "enum":["auto","rust","node","python"],
                "description":runtime},
            "args": {"type":"string", "description":operation}
        },
        "required":[]
    })
}
