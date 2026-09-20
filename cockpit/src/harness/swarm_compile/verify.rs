use super::schema::CommandProof;
use super::store::compact_scrubbed;
use crate::harness::{output_timed, parse_test_result, run_git, sandbox_command_path};
#[cfg(target_os = "linux")]
use crate::sandbox::{self, SandboxPolicy};
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "linux"))]
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static VERIFY_SEQ: AtomicU64 = AtomicU64::new(0);

pub(super) struct CommandSpec<'a> {
    pub label: &'a str,
    pub command: &'a str,
    pub marker: Option<&'a str>,
}

pub(super) struct RefVerifier {
    repo: PathBuf,
    workspace_rel: PathBuf,
    scratch: PathBuf,
}

impl RefVerifier {
    pub(super) fn new(repo: PathBuf, workspace_rel: PathBuf, scratch: PathBuf) -> Self {
        Self {
            repo,
            workspace_rel,
            scratch,
        }
    }

    pub(super) fn run(
        &self,
        git_ref: &str,
        specs: &[CommandSpec<'_>],
    ) -> Result<Vec<CommandProof>, String> {
        validate_ref(git_ref)?;
        std::fs::create_dir_all(&self.scratch)
            .map_err(|e| format!("create {}: {e}", self.scratch.display()))?;
        secure_dir(&self.scratch)?;
        specs
            .iter()
            .map(|spec| self.run_one(git_ref, spec))
            .collect()
    }

    fn run_one(&self, git_ref: &str, spec: &CommandSpec<'_>) -> Result<CommandProof, String> {
        let seq = VERIFY_SEQ.fetch_add(1, Ordering::Relaxed);
        let worktree = self
            .scratch
            .join(format!("verify-{}-{seq}", std::process::id()));
        let worktree_s = worktree.to_string_lossy().into_owned();
        run_git(
            &self.repo,
            &["worktree", "add", "-q", "--detach", &worktree_s, git_ref],
        )?;

        let workspace = worktree.join(&self.workspace_rel);
        let result = run_command(&workspace, &worktree, git_ref, spec);
        let cleanup = run_git(&self.repo, &["worktree", "remove", "--force", &worktree_s]);
        match (result, cleanup) {
            (Ok(proof), Ok(_)) => Ok(proof),
            (Err(err), Ok(_)) => Err(err),
            (Ok(_), Err(err)) => Err(format!("verification succeeded but cleanup failed: {err}")),
            (Err(err), Err(cleanup)) => Err(format!("{err}; cleanup failed: {cleanup}")),
        }
    }
}

fn run_command(
    workspace: &Path,
    worktree: &Path,
    git_ref: &str,
    spec: &CommandSpec<'_>,
) -> Result<CommandProof, String> {
    let timeout_secs = std::env::var("ANGEL_SWARM_VERIFY_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(900)
        .clamp(10, 7200);
    let started = Instant::now();
    #[cfg(target_os = "linux")]
    let mut command = sandbox::command("sh", ["-c", spec.command], &verification_policy(worktree))?;
    #[cfg(not(target_os = "linux"))]
    let mut command = {
        let mut command = Command::new("sh");
        command.arg("-c").arg(spec.command);
        command
    };
    command.current_dir(workspace);
    if let Some(path) = sandbox_command_path() {
        command.env("PATH", path);
    }
    let (output, timed_out) =
        output_timed(command, Some(std::time::Duration::from_secs(timeout_secs)))?;
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    let outcome = parse_test_result(&combined);
    let has_test_summary = combined.contains("test result:");
    let code = output.status.code();
    // A command runs from the configured workspace, but its filesystem sandbox
    // can write anywhere in the detached worktree. Check the whole checkout so
    // a mutation in a sibling path cannot masquerade as a clean proof.
    let dirty = run_git(worktree, &["status", "--porcelain"])?;
    let workspace_clean = dirty.trim().is_empty();
    Ok(CommandProof {
        label: spec.label.to_string(),
        git_ref: git_ref.to_string(),
        command: compact_scrubbed(spec.command, 400),
        success: output.status.success() && workspace_clean && !timed_out,
        exit_code: code,
        timed_out,
        marker_seen: spec.marker.is_some_and(|marker| combined.contains(marker)),
        workspace_clean,
        has_test_summary,
        passed_tests: if has_test_summary { outcome.passed } else { 0 },
        failed_tests: if has_test_summary { outcome.failed } else { 0 },
        output_tail: compact_scrubbed(&tail_chars(&combined, 1_200), 1_200),
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[cfg(target_os = "linux")]
fn verification_policy(worktree: &Path) -> SandboxPolicy {
    let mut roots = vec![
        worktree.to_path_buf(),
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/dev/shm"),
    ];
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        roots.push(PathBuf::from(runtime));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for relative in [".cargo", ".rustup", ".cache", ".npm"] {
            roots.push(home.join(relative));
        }
    }
    for key in [
        "CARGO_TARGET_DIR",
        "UV_CACHE_DIR",
        "PIP_CACHE_DIR",
        "npm_config_cache",
    ] {
        if let Some(path) = std::env::var_os(key) {
            roots.push(PathBuf::from(path));
        }
    }
    roots.retain(|path| path.exists());
    roots.sort();
    roots.dedup();
    SandboxPolicy {
        writable_roots: roots,
        allow_network: true,
        enforce: true,
        mandatory: true,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    }
}

pub(super) fn changed_paths(repo: &Path, from: &str, to: &str) -> Result<Vec<String>, String> {
    validate_ref(from)?;
    validate_ref(to)?;
    let out = run_git(repo, &["diff", "--name-only", from, to])?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

pub(super) fn paths_within_scope(paths: &[String], scope: &[String]) -> bool {
    !paths.is_empty()
        && !scope.is_empty()
        && paths.iter().all(|path| {
            scope.iter().any(|allowed| {
                let allowed = allowed.trim().trim_end_matches('/');
                !allowed.is_empty()
                    && (path == allowed
                        || path
                            .strip_prefix(allowed)
                            .is_some_and(|rest| rest.starts_with('/')))
            })
        })
}

pub(super) fn paths_disjoint(left: &[String], right: &[String]) -> bool {
    left.iter()
        .all(|path| !right.iter().any(|other| other == path))
}

pub(super) fn baseline_passes(proof: &CommandProof) -> bool {
    proof.success
        && (!proof.has_test_summary || (proof.passed_tests > 0 && proof.failed_tests == 0))
}

pub(super) fn regression_guard(base: &CommandProof, candidate: &CommandProof) -> bool {
    if !candidate.success {
        return false;
    }
    if !base.has_test_summary {
        return true;
    }
    candidate.has_test_summary
        && candidate.failed_tests == 0
        && candidate.passed_tests >= base.passed_tests
}

fn validate_ref(git_ref: &str) -> Result<(), String> {
    let valid = !git_ref.trim().is_empty()
        && !git_ref.starts_with('-')
        && git_ref.len() <= 240
        && !git_ref.contains(['\n', '\r', '\0']);
    valid
        .then_some(())
        .ok_or_else(|| "invalid verification ref".to_string())
}

fn tail_chars(text: &str, cap: usize) -> String {
    let count = text.chars().count();
    if count <= cap {
        return text.to_string();
    }
    text.chars().skip(count - cap).collect()
}

#[cfg(unix)]
fn secure_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("secure {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn secure_dir(_path: &Path) -> Result<(), String> {
    Ok(())
}
