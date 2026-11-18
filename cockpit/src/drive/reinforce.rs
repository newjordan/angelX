//! Self-reinforce loop — angel's adaptation of the async-RL systems concepts in
//! SemiAnalysis "RL Systems: Mind the Gap".
//!
//! angel does NOT train weights (its clubs are served externally), so the
//! numerics/weight-sync half of the article (FP8 vs BF16, KV-cache, PD
//! disaggregation) doesn't apply. But the *systems* invariants transfer cleanly:
//! the "policy" is the generation prompt + club-routing that GEPA evolves; the
//! "reward" is pluggable (DICE kernel speedup, or a judge club); and the loop is
//! a generator → reward → optimizer pipeline governed by:
//!   - a **staleness budget** (reject candidates from a policy too many optimizer
//!     steps behind — bounds off-policy-ness),
//!   - **oversampling + straggler pruning** (drop the long-tail rollouts that
//!     would otherwise stall the batch),
//!   - **advantage-variance / solve-rate guards** (a batch where every reward is
//!     equal has zero advantage — detect it and skip the optimizer step rather
//!     than learn from noise).
//!
//! This module is the reward-agnostic core; DICE (speedup_vs_torch) and a judge
//! club plug in via [`Reward`], and GEPA consumes the accepted rollouts.

use crate::agent::club::{ChatMsg, Club, ClubReply};
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub mod artifact;
mod campaign;
pub mod consumption;
#[cfg(test)]
#[path = "../../../tests/cockpit/reinforce/consumption_tests.rs"]
mod consumption_tests;
pub mod evaluator;
pub(crate) mod objective_case;
pub mod promotion;
pub(crate) mod recovery_eval;
pub mod telemetry;
pub(crate) mod training;
pub use campaign::TechnicalCampaignAuthority;
pub(crate) use campaign::authority_root;
use promotion::{
    CohortManifest, CohortRole, HeldoutCase, PromotionConfig, PromotionReport,
    ReceiptFinalAuditRequest, ReceiptPromotionRequest, ReceiptStoreContext, TechnicalHeldoutCase,
    evaluate_final_audit_with_receipts, evaluate_promotion, evaluate_promotion_with_receipts,
};

#[derive(Clone, Debug)]
pub struct EvaluatorEvidence {
    source: String,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    sandbox_stderr_prefix_len: usize,
    output: String,
    exit_code: Option<i32>,
    succeeded: bool,
    workspace_path_sha256: String,
    workspace_before_sha256: String,
    workspace_changed_paths: Vec<String>,
    workspace_sha256: String,
    command_sha256: String,
    execution_policy_sha256: String,
    execution_id: String,
    duration_ns: u64,
    timed_out: bool,
    stdout_total_bytes: u64,
    stderr_total_bytes: u64,
    stdout_truncated: bool,
    stderr_truncated: bool,
    verifier_contract_sha256: String,
    subject_sha256: String,
    raw_output_sha256: String,
    manifest_sha256: String,
}

struct CapturedEvaluatorExecution {
    output: std::process::Output,
    sandbox_stderr_prefix_len: usize,
    workspace_before_sha256: String,
    workspace_changed_paths: Vec<String>,
    workspace_sha256: String,
    execution_policy_sha256: String,
    execution_id: String,
    duration_ns: u64,
    timed_out: bool,
    stdout_total_bytes: u64,
    stderr_total_bytes: u64,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

fn changed_workspace_paths(
    before: Option<std::collections::BTreeMap<PathBuf, String>>,
    after: Option<std::collections::BTreeMap<PathBuf, String>>,
) -> Vec<String> {
    let (Some(before), Some(after)) = (before, after) else {
        return vec!["<path diagnostics unavailable>".to_string()];
    };
    before
        .keys()
        .chain(after.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| format!("{path:?}"))
        .collect()
}

impl EvaluatorEvidence {
    pub(crate) fn run_shell(
        source: &str,
        command: &str,
        workspace: &Path,
        verifier_contract: &str,
        subject: &str,
    ) -> Result<Self, String> {
        Self::run_shell_with_timeout_and_candidate(
            source,
            command,
            workspace,
            verifier_contract,
            subject,
            Some(EVALUATOR_TIMEOUT),
            None,
            false,
            false,
            None,
        )
    }

    pub(crate) fn run_shell_with_candidate(
        source: &str,
        command: &str,
        workspace: &Path,
        verifier_contract: &str,
        subject: &str,
        candidate: &[u8],
    ) -> Result<Self, String> {
        Self::run_shell_with_timeout_and_candidate(
            source,
            command,
            workspace,
            verifier_contract,
            subject,
            Some(EVALUATOR_TIMEOUT),
            Some(candidate),
            false,
            false,
            None,
        )
    }

    fn run_shell_with_timeout(
        source: &str,
        command: &str,
        workspace: &Path,
        verifier_contract: &str,
        subject: &str,
        timeout: Duration,
    ) -> Result<Self, String> {
        Self::run_shell_with_timeout_and_candidate(
            source,
            command,
            workspace,
            verifier_contract,
            subject,
            Some(timeout),
            None,
            false,
            false,
            None,
        )
    }

    pub(crate) fn run_shell_for_objective(
        source: &str,
        command: &str,
        workspace: &Path,
        verifier_contract: &str,
        subject: &str,
        candidate: &[u8],
        cancel: &AtomicBool,
    ) -> Result<Self, String> {
        Self::run_shell_with_timeout_and_candidate(
            source,
            command,
            workspace,
            verifier_contract,
            subject,
            None,
            Some(candidate),
            true,
            true,
            Some(cancel),
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "One receipt-producing path carries explicit execution authority for every caller"
    )]
    fn run_shell_with_timeout_and_candidate(
        source: &str,
        command: &str,
        workspace: &Path,
        verifier_contract: &str,
        subject: &str,
        timeout: Option<Duration>,
        candidate: Option<&[u8]>,
        writable_workspace: bool,
        operator_environment: bool,
        cancel: Option<&AtomicBool>,
    ) -> Result<Self, String> {
        let workspace =
            std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
        let paths_before = crate::agent::harness::workspace_evidence_paths(&workspace);
        let workspace_before_sha256 = crate::agent::harness::workspace_evidence_sha256(&workspace)
            .ok_or_else(|| {
                "evaluator evidence requires a Git-backed workspace snapshot".to_string()
            })?;
        let execution_id = next_evaluator_execution_id();
        let scratch = std::env::temp_dir().join(format!(
            "angel-evaluator-{}-{}",
            std::process::id(),
            &execution_id[..16]
        ));
        std::fs::create_dir(&scratch)
            .map_err(|error| format!("could not create evaluator scratch directory: {error}"))?;
        let candidate_path = scratch.join("candidate-output");
        if let Some(candidate) = candidate {
            crate::platform::workspace_store::write_atomic(&candidate_path, candidate)
                .map_err(|error| format!("could not stage evaluator candidate input: {error}"))?;
        }
        let policy = if operator_environment {
            EvaluatorExecutionPolicy::capture_operator(timeout)?
        } else {
            EvaluatorExecutionPolicy::capture(timeout.unwrap_or(EVALUATOR_TIMEOUT))?
        };
        let mut writable_roots = vec![scratch.clone()];
        if writable_workspace {
            writable_roots.push(workspace.clone());
        }
        let sandbox = crate::agent::sandbox::SandboxPolicy {
            writable_roots,
            allow_network: false,
            enforce: true,
            mandatory: true,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        };
        #[cfg(unix)]
        let mut process =
            crate::agent::sandbox::command(&policy.shell, ["-eu", "-c", command], &sandbox)
                .map_err(|error| format!("could not prepare evaluator sandbox: {error}"))?;
        #[cfg(not(unix))]
        let mut process = {
            let mut process = std::process::Command::new(&policy.shell);
            process.arg("-eu").arg("-c").arg(command);
            process
        };
        process.current_dir(&workspace).env_clear();
        for (name, value) in &policy.environment {
            process.env(name, value);
        }
        process
            .env("TMPDIR", &scratch)
            .env("TMP", &scratch)
            .env("TEMP", &scratch)
            .env("CARGO_TARGET_DIR", scratch.join("cargo-target"))
            .env("XDG_CACHE_HOME", scratch.join("cache"))
            .env("PYTHONDONTWRITEBYTECODE", "1");
        if candidate.is_some() {
            process.env("ANGEL_EVALUATOR_CANDIDATE_PATH", &candidate_path);
        }
        #[cfg(unix)]
        crate::agent::sandbox::set_helper_policy(&mut process, &sandbox)
            .map_err(|error| format!("could not restore evaluator sandbox policy: {error}"))?;
        let started = Instant::now();
        let capture_result = crate::agent::harness::output_timed_captured_cancellable(
            process,
            policy.timeout,
            cancel,
        );
        let duration_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let cleanup_result = std::fs::remove_dir_all(&scratch);
        let capture = capture_result
            .map_err(|error| format!("could not run evaluator command `{command}`: {error}"))?;
        cleanup_result
            .map_err(|error| format!("could not clean evaluator scratch directory: {error}"))?;
        let workspace_sha256 = crate::agent::harness::workspace_evidence_sha256(&workspace)
            .ok_or_else(|| "evaluator command destroyed its Git-backed workspace".to_string())?;
        let workspace_changed_paths = changed_workspace_paths(
            paths_before,
            crate::agent::harness::workspace_evidence_paths(&workspace),
        );
        Self::from_captured_output(
            source,
            command,
            &workspace,
            verifier_contract,
            subject,
            CapturedEvaluatorExecution {
                sandbox_stderr_prefix_len: captured_launcher_stderr_prefix_len(
                    &capture.output.stderr,
                ),
                output: capture.output,
                workspace_before_sha256,
                workspace_changed_paths,
                workspace_sha256,
                execution_policy_sha256: policy.manifest_sha256,
                execution_id,
                duration_ns,
                timed_out: capture.timed_out,
                stdout_total_bytes: capture.stdout_total_bytes,
                stderr_total_bytes: capture.stderr_total_bytes,
                stdout_truncated: capture.stdout_truncated,
                stderr_truncated: capture.stderr_truncated,
            },
        )
    }

    fn from_captured_output(
        source: &str,
        command: &str,
        workspace: &Path,
        verifier_contract: &str,
        subject: &str,
        execution: CapturedEvaluatorExecution,
    ) -> Result<Self, String> {
        let CapturedEvaluatorExecution {
            output,
            sandbox_stderr_prefix_len,
            workspace_before_sha256,
            workspace_changed_paths,
            workspace_sha256,
            execution_policy_sha256,
            execution_id,
            duration_ns,
            timed_out,
            stdout_total_bytes,
            stderr_total_bytes,
            stdout_truncated,
            stderr_truncated,
        } = execution;
        let evaluator_stderr = output
            .stderr
            .get(sandbox_stderr_prefix_len..)
            .ok_or("invalid sandbox stderr boundary")?;
        let combined = combined_process_output(&output.stdout, evaluator_stderr);
        let workspace_path_sha256 =
            crate::knowledge::cut::sha256_hex(workspace.to_string_lossy().as_bytes());
        let command_sha256 = crate::knowledge::cut::sha256_hex(command.as_bytes());
        let verifier_contract_sha256 =
            crate::knowledge::cut::sha256_hex(verifier_contract.as_bytes());
        let subject_sha256 = crate::knowledge::cut::sha256_hex(subject.as_bytes());
        let raw_output_sha256 = process_output_sha256(&output.stdout, &output.stderr);
        let exit_code = output.status.code();
        let succeeded = output.status.success();
        let manifest_sha256 = evaluator_manifest_sha256(EvaluatorManifestFields {
            source,
            workspace_path_sha256: &workspace_path_sha256,
            workspace_before_sha256: &workspace_before_sha256,
            workspace_changed_paths: &workspace_changed_paths,
            workspace_sha256: &workspace_sha256,
            command_sha256: &command_sha256,
            execution_policy_sha256: &execution_policy_sha256,
            execution_id: &execution_id,
            duration_ns,
            timed_out,
            stdout_total_bytes,
            stderr_total_bytes,
            stdout_truncated,
            stderr_truncated,
            verifier_contract_sha256: &verifier_contract_sha256,
            subject_sha256: &subject_sha256,
            sandbox_stderr_prefix_len,
            raw_output_sha256: &raw_output_sha256,
            exit_code,
            succeeded,
        });
        Ok(Self {
            source: source.to_string(),
            stdout: output.stdout,
            stderr: output.stderr,
            sandbox_stderr_prefix_len,
            output: combined,
            exit_code,
            succeeded,
            workspace_path_sha256,
            workspace_before_sha256,
            workspace_changed_paths,
            workspace_sha256,
            command_sha256,
            execution_policy_sha256,
            execution_id,
            duration_ns,
            timed_out,
            stdout_total_bytes,
            stderr_total_bytes,
            stdout_truncated,
            stderr_truncated,
            verifier_contract_sha256,
            subject_sha256,
            raw_output_sha256,
            manifest_sha256,
        })
    }

    #[cfg(test)]
    fn from_executed_output(
        source: &str,
        command: &str,
        workspace: &Path,
        verifier_contract: &str,
        subject: &str,
        output: std::process::Output,
    ) -> Result<Self, String> {
        let workspace =
            std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
        let workspace_sha256 = crate::agent::harness::workspace_evidence_sha256(&workspace)
            .ok_or_else(|| {
                "evaluator evidence requires a Git-backed workspace snapshot".to_string()
            })?;
        let stdout_total_bytes = output.stdout.len() as u64;
        let stderr_total_bytes = output.stderr.len() as u64;
        Self::from_captured_output(
            source,
            command,
            &workspace,
            verifier_contract,
            subject,
            CapturedEvaluatorExecution {
                output,
                sandbox_stderr_prefix_len: 0,
                workspace_before_sha256: workspace_sha256.clone(),
                workspace_changed_paths: Vec::new(),
                workspace_sha256,
                execution_policy_sha256: evaluator_execution_policy_sha256()?,
                execution_id: next_evaluator_execution_id(),
                duration_ns: 0,
                timed_out: false,
                stdout_total_bytes,
                stderr_total_bytes,
                stdout_truncated: false,
                stderr_truncated: false,
            },
        )
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub(crate) fn stdout_bytes(&self) -> &[u8] {
        &self.stdout
    }

    pub(crate) fn stderr_bytes(&self) -> &[u8] {
        &self.stderr
    }

    pub(crate) fn stderr_is_empty(&self) -> bool {
        self.evaluator_stderr().is_empty()
    }

    fn evaluator_stderr(&self) -> &[u8] {
        self.stderr
            .get(self.sandbox_stderr_prefix_len..)
            .unwrap_or(&self.stderr)
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub fn succeeded(&self) -> bool {
        self.succeeded
    }

    pub fn command_sha256(&self) -> &str {
        &self.command_sha256
    }

    pub fn workspace_sha256(&self) -> &str {
        &self.workspace_sha256
    }

    pub(crate) fn workspace_path_sha256(&self) -> &str {
        &self.workspace_path_sha256
    }

    pub fn workspace_before_sha256(&self) -> &str {
        &self.workspace_before_sha256
    }

    pub fn execution_policy_sha256(&self) -> &str {
        &self.execution_policy_sha256
    }

    pub fn timed_out(&self) -> bool {
        self.timed_out
    }

    pub fn execution_id(&self) -> &str {
        &self.execution_id
    }

    pub fn duration_ns(&self) -> u64 {
        self.duration_ns
    }

    pub fn output_truncated(&self) -> bool {
        self.stdout_truncated || self.stderr_truncated
    }

    pub fn verifier_contract_sha256(&self) -> &str {
        &self.verifier_contract_sha256
    }

    pub fn subject_sha256(&self) -> &str {
        &self.subject_sha256
    }

    pub fn raw_output_sha256(&self) -> &str {
        &self.raw_output_sha256
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    fn validate_integrity(&self) -> Result<(), String> {
        if self.sandbox_stderr_prefix_len != 0
            && self.sandbox_stderr_prefix_len != launcher_stderr_prefix_len(&self.stderr)
        {
            return Err("evaluator evidence sandbox stderr boundary mismatch".into());
        }
        if combined_process_output(&self.stdout, self.evaluator_stderr()) != self.output {
            return Err("evaluator evidence rendered-output mismatch".into());
        }
        let raw_output_sha256 = process_output_sha256(&self.stdout, &self.stderr);
        if raw_output_sha256 != self.raw_output_sha256 {
            return Err("evaluator evidence raw-output hash mismatch".into());
        }
        let manifest_sha256 = evaluator_manifest_sha256(EvaluatorManifestFields {
            source: &self.source,
            workspace_path_sha256: &self.workspace_path_sha256,
            workspace_before_sha256: &self.workspace_before_sha256,
            workspace_changed_paths: &self.workspace_changed_paths,
            workspace_sha256: &self.workspace_sha256,
            command_sha256: &self.command_sha256,
            execution_policy_sha256: &self.execution_policy_sha256,
            execution_id: &self.execution_id,
            duration_ns: self.duration_ns,
            timed_out: self.timed_out,
            stdout_total_bytes: self.stdout_total_bytes,
            stderr_total_bytes: self.stderr_total_bytes,
            stdout_truncated: self.stdout_truncated,
            stderr_truncated: self.stderr_truncated,
            verifier_contract_sha256: &self.verifier_contract_sha256,
            subject_sha256: &self.subject_sha256,
            sandbox_stderr_prefix_len: self.sandbox_stderr_prefix_len,
            raw_output_sha256: &self.raw_output_sha256,
            exit_code: self.exit_code,
            succeeded: self.succeeded,
        });
        if manifest_sha256 != self.manifest_sha256 {
            return Err("evaluator evidence manifest hash mismatch".into());
        }
        Ok(())
    }

    pub(crate) fn validate_for_scoring(&self) -> Result<(), String> {
        self.validate_integrity()?;
        if self.timed_out {
            return Err("evaluator command exceeded its pinned timeout".into());
        }
        if self.output_truncated() {
            return Err("evaluator command output exceeded the complete-capture limit".into());
        }
        if self.workspace_before_sha256 != self.workspace_sha256 {
            let paths = if self.workspace_changed_paths.is_empty() {
                "<Git metadata or presentation changed; no differing file bytes observed>"
                    .to_string()
            } else {
                self.workspace_changed_paths.join(", ")
            };
            return Err(format!(
                "evaluator command mutated its Git workspace during verification; changed paths: {paths}"
            ));
        }
        Ok(())
    }

    fn require_verifier_contract(&self, reward: &str, allowed: &[&str]) -> Result<(), String> {
        if allowed.iter().any(|contract| {
            crate::knowledge::cut::sha256_hex(contract.as_bytes()) == self.verifier_contract_sha256
        }) {
            return Ok(());
        }
        Err(format!(
            "{reward} reward does not accept evaluator contract {}",
            self.verifier_contract_sha256
        ))
    }
}

const EVALUATOR_TIMEOUT: Duration = Duration::from_secs(120);
const EVALUATOR_EXECUTION_SCHEMA: &str = "angel.rlvr.execution-policy/v3";
const EVALUATOR_CAPTURE_POLICY: &str = "landlock-source-read-only;scratch=v1;network=deny-inet-socket/seccomp+tcp-landlock;shell-flags=-eu;process-group;stdin=null;stdout=head64KiB+tail960KiB;stderr=head64KiB+tail960KiB";
const EVALUATOR_OBJECTIVE_CAPTURE_POLICY: &str = "landlock-workspace+scratch;network=deny-inet-socket/seccomp+tcp-landlock;shell-flags=-eu;process-group;stdin=null;env=inherited-operator;stdout=head64KiB+tail960KiB;stderr=head64KiB+tail960KiB";
const EVALUATOR_ENV_ALLOWLIST: &[&str] = &[
    "AR",
    "CARGO_HOME",
    "CC",
    "CXX",
    "HOME",
    "PKG_CONFIG_LIBDIR",
    "PKG_CONFIG_PATH",
    "PKG_CONFIG_SYSROOT_DIR",
    "RUSTDOCFLAGS",
    "RUSTFLAGS",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
];

struct EvaluatorExecutionPolicy {
    shell: PathBuf,
    timeout: Option<Duration>,
    environment: Vec<(OsString, OsString)>,
    manifest_sha256: String,
}

impl EvaluatorExecutionPolicy {
    fn capture(timeout: Duration) -> Result<Self, String> {
        Self::capture_with(Some(timeout), false)
    }

    fn capture_operator(timeout: Option<Duration>) -> Result<Self, String> {
        Self::capture_with(timeout, true)
    }

    fn capture_with(timeout: Option<Duration>, operator_environment: bool) -> Result<Self, String> {
        let shell = std::fs::canonicalize("/bin/sh").map_err(|error| {
            format!("could not resolve pinned evaluator shell /bin/sh: {error}")
        })?;
        let shell_bytes = std::fs::read(&shell).map_err(|error| {
            format!(
                "could not hash evaluator shell {}: {error}",
                shell.display()
            )
        })?;
        let mut environment = if operator_environment {
            std::env::vars_os().collect::<Vec<_>>()
        } else {
            EVALUATOR_ENV_ALLOWLIST
                .iter()
                .filter_map(|name| {
                    std::env::var_os(name).map(|value| (OsString::from(name), value))
                })
                .collect::<Vec<_>>()
        };
        environment.push((OsString::from("LANG"), OsString::from("C")));
        environment.push((OsString::from("LC_ALL"), OsString::from("C")));
        if !operator_environment {
            environment.push((OsString::from("PATH"), OsString::from("/nonexistent")));
        }
        environment.sort_by(|left, right| os_bytes(&left.0).cmp(os_bytes(&right.0)));

        let mut canonical = Vec::new();
        append_manifest_bytes(
            &mut canonical,
            b"schema",
            EVALUATOR_EXECUTION_SCHEMA.as_bytes(),
        );
        append_manifest_bytes(&mut canonical, b"shell-path", os_bytes(shell.as_os_str()));
        append_manifest_bytes(
            &mut canonical,
            b"shell-sha256",
            crate::knowledge::cut::sha256_hex(&shell_bytes).as_bytes(),
        );
        let git = crate::agent::harness::pinned_git_path()
            .ok_or_else(|| "could not resolve trusted evaluator Git binary".to_string())?;
        let git_bytes = std::fs::read(git)
            .map_err(|error| format!("could not hash evaluator Git {}: {error}", git.display()))?;
        append_manifest_bytes(&mut canonical, b"git-path", os_bytes(git.as_os_str()));
        append_manifest_bytes(
            &mut canonical,
            b"git-sha256",
            crate::knowledge::cut::sha256_hex(&git_bytes).as_bytes(),
        );
        append_manifest_bytes(
            &mut canonical,
            b"timeout-ms",
            timeout
                .map(|timeout| timeout.as_millis().to_string())
                .unwrap_or_else(|| "none".to_string())
                .as_bytes(),
        );
        append_manifest_bytes(
            &mut canonical,
            b"capture-policy",
            if operator_environment {
                EVALUATOR_OBJECTIVE_CAPTURE_POLICY.as_bytes()
            } else {
                EVALUATOR_CAPTURE_POLICY.as_bytes()
            },
        );
        if !operator_environment {
            for (name, value) in &environment {
                append_manifest_bytes(&mut canonical, os_bytes(name), os_bytes(value));
            }
        }
        let manifest_sha256 = crate::knowledge::cut::sha256_hex(&canonical);
        Ok(Self {
            shell,
            timeout,
            environment,
            manifest_sha256,
        })
    }
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

pub(crate) fn evaluator_execution_policy_sha256() -> Result<String, String> {
    EvaluatorExecutionPolicy::capture(EVALUATOR_TIMEOUT).map(|policy| policy.manifest_sha256)
}

pub(crate) fn evaluator_objective_execution_policy_sha256() -> Result<String, String> {
    EvaluatorExecutionPolicy::capture_operator(None).map(|policy| policy.manifest_sha256)
}

static EVALUATOR_EXECUTION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_evaluator_execution_id() -> String {
    let serial = EVALUATOR_EXECUTION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let identity = format!("{}:{now}:{serial}", std::process::id());
    crate::knowledge::cut::sha256_hex(identity.as_bytes())
}

fn append_manifest_bytes(canonical: &mut Vec<u8>, label: &[u8], bytes: &[u8]) {
    canonical.extend_from_slice(&(label.len() as u64).to_be_bytes());
    canonical.extend_from_slice(label);
    canonical.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    canonical.extend_from_slice(bytes);
}

fn process_output_sha256(stdout: &[u8], stderr: &[u8]) -> String {
    let mut canonical = Vec::with_capacity(stdout.len() + stderr.len() + 64);
    append_manifest_bytes(&mut canonical, b"stdout", stdout);
    append_manifest_bytes(&mut canonical, b"stderr", stderr);
    crate::knowledge::cut::sha256_hex(&canonical)
}

fn captured_launcher_stderr_prefix_len(stderr: &[u8]) -> usize {
    if cfg!(target_os = "linux") {
        launcher_stderr_prefix_len(stderr)
    } else {
        0
    }
}

fn launcher_stderr_prefix_len(stderr: &[u8]) -> usize {
    let Some(json) = stderr.strip_prefix(b"sandbox-hardlinks: ") else {
        return 0;
    };
    let Some(end) = json.iter().position(|byte| *byte == b'\n') else {
        return 0;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&json[..end]) else {
        return 0;
    };
    if value
        .get("scan_complete")
        .and_then(serde_json::Value::as_bool)
        .is_none()
        || value
            .get("hardlink_readonly_count")
            .and_then(serde_json::Value::as_u64)
            .is_none()
    {
        return 0;
    }
    stderr.len() - json.len() + end + 1
}

fn combined_process_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut combined = String::from_utf8_lossy(stdout).into_owned();
    if !combined.is_empty() && !combined.ends_with('\n') && !stderr.is_empty() {
        combined.push('\n');
    }
    combined.push_str(&String::from_utf8_lossy(stderr));
    combined
}

const EVALUATOR_EVIDENCE_SCHEMA: &str = "angel.rlvr.evaluator-evidence/v2";
pub(crate) const CODE_HEALTH_VERIFIER_CONTRACT: &str = "angel.rlvr.code-health/v2";
pub(crate) const TEST_VERIFIER_CONTRACT: &str = "angel.rlvr.tests/v1";
pub(crate) const COMMAND_SUCCESS_VERIFIER_CONTRACT: &str = "angel.rlvr.command-success/v1";
const LINT_VERIFIER_CONTRACT: &str = "angel.rlvr.lint/v1";

struct EvaluatorManifestFields<'a> {
    source: &'a str,
    workspace_path_sha256: &'a str,
    workspace_before_sha256: &'a str,
    workspace_changed_paths: &'a [String],
    workspace_sha256: &'a str,
    command_sha256: &'a str,
    execution_policy_sha256: &'a str,
    execution_id: &'a str,
    duration_ns: u64,
    timed_out: bool,
    stdout_total_bytes: u64,
    stderr_total_bytes: u64,
    stdout_truncated: bool,
    stderr_truncated: bool,
    verifier_contract_sha256: &'a str,
    subject_sha256: &'a str,
    sandbox_stderr_prefix_len: usize,
    raw_output_sha256: &'a str,
    exit_code: Option<i32>,
    succeeded: bool,
}

fn evaluator_manifest_sha256(fields: EvaluatorManifestFields<'_>) -> String {
    let EvaluatorManifestFields {
        source,
        workspace_path_sha256,
        workspace_before_sha256,
        workspace_changed_paths,
        workspace_sha256,
        command_sha256,
        execution_policy_sha256,
        execution_id,
        duration_ns,
        timed_out,
        stdout_total_bytes,
        stderr_total_bytes,
        stdout_truncated,
        stderr_truncated,
        verifier_contract_sha256,
        subject_sha256,
        raw_output_sha256,
        sandbox_stderr_prefix_len,
        exit_code,
        succeeded,
    } = fields;
    let mut canonical = String::new();
    for value in [
        EVALUATOR_EVIDENCE_SCHEMA.to_string(),
        source.to_string(),
        workspace_path_sha256.to_string(),
        workspace_before_sha256.to_string(),
        workspace_sha256.to_string(),
        command_sha256.to_string(),
        execution_policy_sha256.to_string(),
        execution_id.to_string(),
        duration_ns.to_string(),
        timed_out.to_string(),
        stdout_total_bytes.to_string(),
        stderr_total_bytes.to_string(),
        stdout_truncated.to_string(),
        stderr_truncated.to_string(),
        verifier_contract_sha256.to_string(),
        subject_sha256.to_string(),
        raw_output_sha256.to_string(),
        exit_code.map_or_else(|| "signal".to_string(), |code| code.to_string()),
        succeeded.to_string(),
    ] {
        canonical.push_str(&value.len().to_string());
        canonical.push(':');
        canonical.push_str(&value);
        canonical.push('\n');
    }
    if !workspace_changed_paths.is_empty() {
        canonical.push_str("changed-paths/v1\n");
        for path in workspace_changed_paths {
            canonical.push_str(&path.len().to_string());
            canonical.push(':');
            canonical.push_str(path);
            canonical.push('\n');
        }
    }
    if sandbox_stderr_prefix_len != 0 {
        canonical.push_str(&format!(
            "sandbox-stderr-prefix/v1:{sandbox_stderr_prefix_len}\n"
        ));
    }
    crate::knowledge::cut::sha256_hex(canonical.as_bytes())
}

#[derive(Clone, Copy, Debug)]
pub enum RewardInput<'a> {
    CandidateOutput(&'a str),
    EvaluatorEvidence(&'a EvaluatorEvidence),
}

impl<'a> RewardInput<'a> {
    fn candidate_output(self, label: &str) -> Result<&'a str, String> {
        match self {
            Self::CandidateOutput(output) => Ok(output),
            Self::EvaluatorEvidence(_) => Err(format!(
                "{label} reward requires candidate output, not evaluator evidence"
            )),
        }
    }

    fn evaluator_evidence(self, label: &str) -> Result<&'a EvaluatorEvidence, String> {
        match self {
            Self::EvaluatorEvidence(evidence) => Ok(evidence),
            Self::CandidateOutput(_) => Err(format!(
                "{label} reward requires evaluator-owned command evidence"
            )),
        }
    }
}

pub trait Reward: Send + Sync {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String>;
    fn label(&self) -> &str;
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub policy_version: u64,
    pub latency: Duration,
    pub output: String,
}

#[derive(Clone, Debug)]
pub struct Rollout {
    pub policy_version: u64,
    pub reward: f32,
    pub latency: Duration,
    pub accepted: bool,
    pub output: String,
}

#[derive(Clone, Debug)]
pub struct ReinforceConfig {
    pub group_size: usize,
    pub oversample: f32,
    pub staleness_budget: u64,
    pub straggler_factor: f32,
    pub success_threshold: f32,
}

impl Default for ReinforceConfig {
    fn default() -> Self {
        Self {
            group_size: 8,
            oversample: 0.6,
            staleness_budget: 16,
            straggler_factor: 2.0,
            success_threshold: 1.0,
        }
    }
}

impl ReinforceConfig {
    pub fn dispatch_count(&self) -> usize {
        ((self.group_size as f32) * (1.0 + self.oversample)).ceil() as usize
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct BatchMetrics {
    pub generated: usize,
    pub accepted: usize,
    pub rejected_stale: usize,
    pub rejected_straggler: usize,
    pub solve_rate: f32,
    pub advantage_variance: f32,
    pub median_latency: Duration,
    pub p99_latency: Duration,
}

impl BatchMetrics {
    pub fn acceptance_rate(&self) -> f32 {
        if self.generated == 0 {
            0.0
        } else {
            self.accepted as f32 / self.generated as f32
        }
    }

    pub fn has_learning_signal(&self) -> bool {
        self.advantage_variance > 1e-6 && self.solve_rate > 0.0 && self.solve_rate < 1.0
    }
}

pub fn evaluate_batch(
    reward: &dyn Reward,
    current_version: u64,
    cfg: &ReinforceConfig,
    candidates: Vec<Candidate>,
) -> (Vec<Rollout>, BatchMetrics) {
    let generated = candidates.len();

    let mut lats: Vec<Duration> = candidates.iter().map(|c| c.latency).collect();
    lats.sort_unstable();
    let median = lats.get(lats.len() / 2).copied().unwrap_or_default();
    let p99 = lats
        .get(((lats.len() as f32 * 0.99) as usize).min(lats.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default();
    let straggler_thresh =
        (cfg.straggler_factor > 0.0).then(|| median.mul_f32(cfg.straggler_factor.max(1.0)));

    let mut rollouts = Vec::with_capacity(generated);
    let mut rejected_stale = 0usize;
    let mut rejected_straggler = 0usize;

    for c in candidates {
        let reward_val = reward
            .score(RewardInput::CandidateOutput(&c.output))
            .unwrap_or(f32::NEG_INFINITY);
        let stale = current_version.saturating_sub(c.policy_version) > cfg.staleness_budget;
        let straggler = median > Duration::ZERO
            && straggler_thresh.is_some_and(|threshold| c.latency > threshold);
        let scored_ok = reward_val.is_finite();
        let accepted = !stale && !straggler && scored_ok;

        if stale {
            rejected_stale += 1;
        } else if straggler {
            rejected_straggler += 1;
        }

        rollouts.push(Rollout {
            policy_version: c.policy_version,
            reward: if scored_ok { reward_val } else { 0.0 },
            latency: c.latency,
            accepted,
            output: c.output,
        });
    }

    let accepted: Vec<&Rollout> = rollouts.iter().filter(|r| r.accepted).collect();
    let solved = accepted
        .iter()
        .filter(|r| r.reward >= cfg.success_threshold)
        .count();
    let solve_rate = if accepted.is_empty() {
        0.0
    } else {
        solved as f32 / accepted.len() as f32
    };
    let advantage_variance = variance(accepted.iter().map(|r| r.reward));

    let metrics = BatchMetrics {
        generated,
        accepted: accepted.len(),
        rejected_stale,
        rejected_straggler,
        solve_rate,
        advantage_variance,
        median_latency: median,
        p99_latency: p99,
    };
    (rollouts, metrics)
}

fn variance<I: Iterator<Item = f32>>(values: I) -> f32 {
    let v: Vec<f32> = values.collect();
    if v.len() < 2 {
        return 0.0;
    }
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / v.len() as f32
}

pub trait Generator: Send + Sync {
    fn generate(&self, system_prompt: &str, task: &str, version: u64, n: usize) -> Vec<Candidate>;
}

pub trait Reflector: Send + Sync {
    fn improve(
        &self,
        current_prompt: &str,
        task: &str,
        best: &str,
        worst: &str,
    ) -> Result<String, String>;
}

pub struct ClubGenerator {
    pub club: Arc<dyn Club>,
}

impl Generator for ClubGenerator {
    fn generate(&self, system_prompt: &str, task: &str, version: u64, n: usize) -> Vec<Candidate> {
        let msgs = [ChatMsg::system(system_prompt), ChatMsg::user(task)];
        (0..n)
            .map(|_| {
                let start = Instant::now();
                let output = match self.club.chat(&msgs, &[]) {
                    Ok(ClubReply::Text(t)) => t,
                    Ok(ClubReply::Calls(_)) => String::new(),
                    Err(e) => format!("(generation error: {e})"),
                };
                Candidate {
                    policy_version: version,
                    latency: start.elapsed(),
                    output,
                }
            })
            .collect()
    }
}

pub struct JudgeReward {
    pub judge: Arc<dyn Club>,
    pub task: String,
}

impl Reward for JudgeReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let output = input.candidate_output(self.label())?;
        let prompt = format!(
            "{}\n\nTASK:\n{}\n\nRESPONSE:\n{}",
            crate::agent::harness::book::r_relentless::GRADER.cells(),
            self.task,
            output
        );
        let reply = crate::agent::harness::book::connect::respond(
            &*self.judge,
            &crate::agent::harness::book::connect::workspace(),
            &prompt,
        )?;
        parse_score(&reply).ok_or_else(|| format!("no score in judge reply: {reply:.80}"))
    }
    fn label(&self) -> &str {
        "judge"
    }
}

pub struct TestReward;

impl Reward for TestReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let evidence = input.evaluator_evidence(self.label())?;
        evidence.validate_for_scoring()?;
        evidence.require_verifier_contract(
            self.label(),
            &[TEST_VERIFIER_CONTRACT, CODE_HEALTH_VERIFIER_CONTRACT],
        )?;
        if !evidence.succeeded() {
            return Err(format!(
                "{} evaluator command failed with exit {:?}",
                evidence.source(),
                evidence.exit_code()
            ));
        }
        Ok(parse_evaluator_test_result(evidence.output())?.reward())
    }
    fn label(&self) -> &str {
        "tests"
    }
}

pub fn coding_verifier_reward(evidence: &EvaluatorEvidence) -> Result<f32, String> {
    evidence.validate_for_scoring()?;
    evidence.require_verifier_contract(
        "coding objective",
        &[TEST_VERIFIER_CONTRACT, CODE_HEALTH_VERIFIER_CONTRACT],
    )?;
    match parse_evaluator_test_result(evidence.output()) {
        Ok(outcome) if outcome.passed.saturating_add(outcome.failed) > 0 => Ok(outcome.reward()),
        Ok(_) => Ok(0.0),
        Err(error) => {
            if evidence.output().contains("test result:") {
                Err(error)
            } else {
                Ok(f32::from(evidence.succeeded()))
            }
        }
    }
}

pub struct TechnicalPassReward;

impl Reward for TechnicalPassReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let evidence = input.evaluator_evidence(self.label())?;
        evidence.validate_for_scoring()?;
        let test = parse_evaluator_test_result(evidence.output())?;
        let executed = test.passed.saturating_add(test.failed);
        if executed == 0 {
            return Err("technical-pass evaluator observed zero tests".into());
        }
        Ok(f32::from(
            evidence.succeeded() && test.failed == 0 && test.passed > 0,
        ))
    }

    fn label(&self) -> &str {
        "technical-pass"
    }
}

pub struct LintReward;

impl Reward for LintReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let evidence = input.evaluator_evidence(self.label())?;
        evidence.validate_for_scoring()?;
        evidence.require_verifier_contract(
            self.label(),
            &[LINT_VERIFIER_CONTRACT, CODE_HEALTH_VERIFIER_CONTRACT],
        )?;
        if !evidence.succeeded() {
            return Err(format!(
                "{} evaluator command failed with exit {:?}",
                evidence.source(),
                evidence.exit_code()
            ));
        }
        Ok(crate::agent::harness::parse_lint(evidence.output()).reward())
    }
    fn label(&self) -> &str {
        "lint"
    }
}

pub fn reward_from_env() -> Box<dyn Reward> {
    let raw = std::env::var("ANGEL_RL_REWARD").unwrap_or_default();
    let key = raw.trim().to_ascii_lowercase();
    match key.as_str() {
        "code_health" | "tests_lint" | "test_lint" => Box::new(CompositeReward::code_health()),
        "test" | "tests" | "test_reward" => Box::new(TestReward),
        "lint" | "clippy" => Box::new(LintReward),
        _ => Box::new(CompositeReward::code_health()),
    }
}

pub fn reward_label_from_env() -> String {
    reward_from_env().label().to_string()
}

pub fn live_reward_label() -> &'static str {
    TestReward.label()
}

pub struct CompositeReward {
    components: Vec<(Box<dyn Reward>, f32)>,
}

impl CompositeReward {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }
    pub fn with(mut self, reward: Box<dyn Reward>, weight: f32) -> Self {
        self.components.push((reward, weight));
        self
    }
    pub fn code_health() -> Self {
        Self::new()
            .with(Box::new(TestReward), 0.8)
            .with(Box::new(LintReward), 0.2)
    }
}

impl Default for CompositeReward {
    fn default() -> Self {
        Self::new()
    }
}

impl Reward for CompositeReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        if self.components.is_empty() {
            return Err("composite reward has no components".to_string());
        }
        let mut total = 0.0;
        let mut wsum = 0.0;
        for (reward, weight) in &self.components {
            let weight = *weight;
            total += reward.score(input)? * weight;
            wsum += weight;
        }
        if wsum == 0.0 {
            return Err("composite reward weights sum to zero".to_string());
        }
        Ok(total / wsum)
    }
    fn label(&self) -> &str {
        "composite"
    }
}

pub struct ClubReflector {
    pub club: Arc<dyn Club>,
}

impl Reflector for ClubReflector {
    fn improve(
        &self,
        current_prompt: &str,
        task: &str,
        best: &str,
        worst: &str,
    ) -> Result<String, String> {
        let prompt = format!(
            "{}\n\nTASK:\n{task}\n\nCURRENT:\n{current_prompt}\n\nHIGH:\n{best}\n\nLOW:\n{worst}",
            crate::agent::harness::book::r_relentless::OPTIMIZER.cells(),
        );
        crate::agent::harness::book::connect::respond(
            &*self.club,
            &crate::agent::harness::book::connect::workspace(),
            &prompt,
        )
    }
}

fn parse_score(text: &str) -> Option<f32> {
    let mut num = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || (ch == '.' && !num.contains('.')) {
            num.push(ch);
        } else if !num.is_empty() {
            break;
        }
    }
    num.parse::<f32>().ok()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RoundReport {
    pub version: u64,
    pub metrics: BatchMetrics,
    pub best_reward: f32,
    pub optimized: bool,
    pub promotion: Option<PromotionReport>,
    pub prompt: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ReinforceReport {
    pub rounds: Vec<RoundReport>,
    pub final_prompt: String,
}

pub struct ReinforceRequest<'a> {
    pub task: &'a str,
    pub initial_prompt: &'a str,
    pub rounds: usize,
    pub config: &'a ReinforceConfig,
}

pub struct PromotionCohort<'a> {
    pub cases: &'a [HeldoutCase<'a>],
    pub config: &'a PromotionConfig,
    pub manifest: &'a CohortManifest,
}

pub struct TechnicalPromotionCohort<'a> {
    pub cases: &'a [TechnicalHeldoutCase<'a>],
    pub config: &'a PromotionConfig,
    pub manifest: &'a CohortManifest,
    pub evaluators: &'a [&'a dyn evaluator::PolicyEvaluator],
}

pub struct TechnicalFinalAuditCohort<'a> {
    pub cases: &'a [TechnicalHeldoutCase<'a>],
    pub config: &'a PromotionConfig,
    pub manifest: &'a CohortManifest,
    pub evaluators: &'a [&'a dyn evaluator::PolicyEvaluator],
}

pub struct TechnicalReinforceCampaign<'a> {
    pub authority: &'a TechnicalCampaignAuthority,
    pub promotion: TechnicalPromotionCohort<'a>,
    pub final_audit: TechnicalFinalAuditCohort<'a>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TechnicalReinforceReport {
    reinforcement: ReinforceReport,
    final_audit: Option<PromotionReport>,
    release_candidate: Option<ReleaseCandidate>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReleaseCandidate {
    prompt: String,
    policy_version: u64,
    promotion_manifest_sha256: String,
    audit_manifest_sha256: String,
    release_sha256: String,
}

impl ReleaseCandidate {
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn policy_version(&self) -> u64 {
        self.policy_version
    }

    pub fn release_sha256(&self) -> &str {
        &self.release_sha256
    }

    pub fn promotion_manifest_sha256(&self) -> &str {
        &self.promotion_manifest_sha256
    }

    pub fn audit_manifest_sha256(&self) -> &str {
        &self.audit_manifest_sha256
    }
}

impl TechnicalReinforceReport {
    pub fn reinforcement(&self) -> &ReinforceReport {
        &self.reinforcement
    }

    pub fn final_audit(&self) -> Option<&PromotionReport> {
        self.final_audit.as_ref()
    }

    pub fn release_ready(&self) -> bool {
        self.release_candidate.is_some()
    }

    pub fn release_candidate(&self) -> Option<&ReleaseCandidate> {
        self.release_candidate.as_ref()
    }
}

fn derive_release_candidate(
    reinforcement: &ReinforceReport,
    final_audit: &PromotionReport,
    campaign_id: &str,
    receipt_ledger_head_sha256: &str,
    promotion_manifest_sha256: &str,
    audit_manifest_sha256: &str,
) -> Result<ReleaseCandidate, String> {
    if !final_audit.approved_final_audit()
        || final_audit.cohort_manifest_sha256 != audit_manifest_sha256
    {
        return Err("terminal release requires an approved matching final audit".into());
    }
    let selection = reinforcement
        .rounds
        .last()
        .and_then(|round| round.promotion.as_ref())
        .ok_or_else(|| "terminal release has no selection report".to_string())?;
    if !selection.promoted() || selection.cohort_manifest_sha256 != promotion_manifest_sha256 {
        return Err("terminal release requires a promoted matching selection".into());
    }
    let policy_version = reinforcement.rounds.last().map_or(0, |round| round.version);
    let mut canonical = String::new();
    for value in [
        campaign_id.to_string(),
        receipt_ledger_head_sha256.to_string(),
        crate::knowledge::cut::sha256_hex(reinforcement.final_prompt.as_bytes()),
        policy_version.to_string(),
        promotion_manifest_sha256.to_string(),
        audit_manifest_sha256.to_string(),
    ]
    .into_iter()
    .chain(selection.evaluator_receipt_sha256s.iter().cloned())
    .chain(final_audit.evaluator_receipt_sha256s.iter().cloned())
    {
        canonical.push_str(&value.len().to_string());
        canonical.push(':');
        canonical.push_str(&value);
        canonical.push('\n');
    }
    Ok(ReleaseCandidate {
        prompt: reinforcement.final_prompt.clone(),
        policy_version,
        promotion_manifest_sha256: promotion_manifest_sha256.to_string(),
        audit_manifest_sha256: audit_manifest_sha256.to_string(),
        release_sha256: crate::knowledge::cut::sha256_hex(canonical.as_bytes()),
    })
}

fn try_hydrate_terminal_release(
    authority: &TechnicalCampaignAuthority,
    request_sha256: &str,
    campaign_id: &str,
    promotion_manifest_sha256: &str,
    audit_manifest_sha256: &str,
) -> Result<Option<TechnicalReinforceReport>, String> {
    let Some((report, receipt_ledger_head_sha256)) = authority.try_load_release(
        request_sha256,
        promotion_manifest_sha256,
        audit_manifest_sha256,
    )?
    else {
        return Ok(None);
    };
    let final_audit = report
        .final_audit()
        .ok_or_else(|| "terminal release has no final audit report".to_string())?;
    let expected_release = derive_release_candidate(
        report.reinforcement(),
        final_audit,
        campaign_id,
        &receipt_ledger_head_sha256,
        promotion_manifest_sha256,
        audit_manifest_sha256,
    )?;
    if report.release_candidate() != Some(&expected_release) {
        return Err("terminal release candidate digest mismatch".into());
    }
    Ok(Some(report))
}

pub fn run_nontechnical_reinforce(
    generator: &dyn Generator,
    reward: &dyn Reward,
    reflector: &dyn Reflector,
    request: ReinforceRequest<'_>,
    promotion: PromotionCohort<'_>,
) -> Result<ReinforceReport, String> {
    promotion.config.validate()?;
    promotion
        .manifest
        .validate(CohortRole::Promotion, promotion.cases, promotion.config)?;
    let run_telemetry = telemetry::begin_run("nontechnical", request.rounds);
    let report = run_reinforce_inner(
        generator,
        reward,
        reflector,
        request.task,
        request.initial_prompt,
        request.rounds,
        request.config,
        Some(PromotionGate::CandidateOutput {
            cases: promotion.cases,
            config: promotion.config,
            manifest: promotion.manifest,
        }),
    )
    .map(|run| run.report);
    if report.is_ok() {
        run_telemetry.finish();
    }
    report
}

pub fn run_reinforce(
    generator: &dyn Generator,
    training_reward: &dyn Reward,
    reflector: &dyn Reflector,
    request: ReinforceRequest<'_>,
    campaign: TechnicalReinforceCampaign<'_>,
) -> Result<TechnicalReinforceReport, String> {
    if request.rounds != 1 {
        return Err(
            "technical release campaign requires exactly one selection transition; run each generation as a new auditable campaign"
                .into(),
        );
    }
    let selection = &campaign.promotion;
    let audit = &campaign.final_audit;
    let receipt_store = campaign.authority.receipt_store();
    let mut campaign_request = String::new();
    for value in [
        crate::knowledge::cut::sha256_hex(request.task.as_bytes()),
        crate::knowledge::cut::sha256_hex(request.initial_prompt.as_bytes()),
        request.rounds.to_string(),
        request.config.group_size.to_string(),
        request.config.oversample.to_bits().to_string(),
        request.config.staleness_budget.to_string(),
        request.config.straggler_factor.to_bits().to_string(),
        request.config.success_threshold.to_bits().to_string(),
        selection.manifest.manifest_sha256().to_string(),
        audit.manifest.manifest_sha256().to_string(),
        promotion::promotion_math_contract_sha256(),
    ] {
        campaign_request.push_str(&value.len().to_string());
        campaign_request.push(':');
        campaign_request.push_str(&value);
        campaign_request.push('\n');
    }
    let campaign_request_sha256 = crate::knowledge::cut::sha256_hex(campaign_request.as_bytes());
    if let Some(report) = try_hydrate_terminal_release(
        campaign.authority,
        &campaign_request_sha256,
        receipt_store.run_id,
        selection.manifest.manifest_sha256(),
        audit.manifest.manifest_sha256(),
    )? {
        return Ok(report);
    }

    let _execution = campaign.authority.acquire_execution()?;
    if let Some(report) = try_hydrate_terminal_release(
        campaign.authority,
        &campaign_request_sha256,
        receipt_store.run_id,
        selection.manifest.manifest_sha256(),
        audit.manifest.manifest_sha256(),
    )? {
        return Ok(report);
    }

    campaign.authority.bind_request(&campaign_request_sha256)?;
    let selection_cases = promotion::technical_cases_as_heldout(selection.cases);
    selection.config.validate()?;
    selection.manifest.validate_technical(
        CohortRole::Promotion,
        selection.cases,
        selection.config,
        selection.evaluators,
    )?;
    let audit_cases = promotion::technical_cases_as_heldout(audit.cases);
    audit.config.validate()?;
    audit.manifest.validate_technical(
        CohortRole::FinalAudit,
        audit.cases,
        audit.config,
        audit.evaluators,
    )?;
    promotion::validate_audit_separation(selection.manifest, audit.manifest)?;
    let run_telemetry = telemetry::begin_run("technical", request.rounds);

    let frozen_training = campaign.authority.try_load_candidate(
        &campaign_request_sha256,
        request.initial_prompt,
        0,
    )?;
    let run = if let Some(frozen) = frozen_training {
        run_frozen_technical_proposal(
            generator,
            frozen,
            &selection_cases,
            selection.config,
            selection.manifest,
            selection.evaluators,
            receipt_store,
        )?
    } else {
        run_reinforce_inner(
            generator,
            training_reward,
            reflector,
            request.task,
            request.initial_prompt,
            request.rounds,
            request.config,
            Some(PromotionGate::TechnicalReceipts {
                cases: &selection_cases,
                config: selection.config,
                manifest: selection.manifest,
                evaluators: selection.evaluators,
                receipt_store,
                authority: campaign.authority,
                request_sha256: &campaign_request_sha256,
            }),
        )?
    };
    let final_audit = if let Some(selected) = run.last_selected {
        telemetry::phase(telemetry::RlPhase::FinalAudit);
        Some(evaluate_final_audit_with_receipts(
            generator,
            ReceiptFinalAuditRequest {
                incumbent_prompt: &selected.incumbent_prompt,
                frozen_candidate_prompt: &run.report.final_prompt,
                incumbent_version: selected.incumbent_version,
                promotion_manifest: selection.manifest,
                audit_cases: &audit_cases,
                config: audit.config,
                audit_manifest: audit.manifest,
                evaluators: audit.evaluators,
                receipt_store,
            },
        )?)
    } else {
        None
    };
    let (release_candidate, terminal_receipt_head) = if let Some(report) = final_audit
        .as_ref()
        .filter(|report| report.approved_final_audit())
    {
        let receipt_ledger_head_sha256 =
            consumption::receipt_pair_ledger_head(receipt_store.ledger_root)?;
        let release = derive_release_candidate(
            &run.report,
            report,
            receipt_store.run_id,
            &receipt_ledger_head_sha256,
            selection.manifest.manifest_sha256(),
            audit.manifest.manifest_sha256(),
        )?;
        (Some(release), Some(receipt_ledger_head_sha256))
    } else {
        (None, None)
    };
    let result = TechnicalReinforceReport {
        reinforcement: run.report,
        final_audit,
        release_candidate,
    };
    if let Some(receipt_ledger_head_sha256) = terminal_receipt_head {
        campaign.authority.commit_release(
            &campaign_request_sha256,
            &receipt_ledger_head_sha256,
            &result,
        )?;
        let committed_head = consumption::receipt_pair_ledger_head(receipt_store.ledger_root)?;
        if committed_head != receipt_ledger_head_sha256 {
            return Err("technical receipt ledger changed during release commit".into());
        }
        telemetry::released();
    }
    run_telemetry.finish();
    Ok(result)
}

pub fn run_reinforce_unchecked_for_ablation(
    generator: &dyn Generator,
    reward: &dyn Reward,
    reflector: &dyn Reflector,
    task: &str,
    initial_prompt: &str,
    rounds: usize,
    cfg: &ReinforceConfig,
) -> ReinforceReport {
    let run_telemetry = telemetry::begin_run("ablation", rounds);
    let report = run_reinforce_inner(
        generator,
        reward,
        reflector,
        task,
        initial_prompt,
        rounds,
        cfg,
        None,
    )
    .expect("unchecked promotion has no fallible gate")
    .report;
    run_telemetry.finish();
    report
}

enum PromotionGate<'a> {
    CandidateOutput {
        cases: &'a [HeldoutCase<'a>],
        config: &'a PromotionConfig,
        manifest: &'a CohortManifest,
    },
    TechnicalReceipts {
        cases: &'a [HeldoutCase<'a>],
        config: &'a PromotionConfig,
        manifest: &'a CohortManifest,
        evaluators: &'a [&'a dyn evaluator::PolicyEvaluator],
        receipt_store: ReceiptStoreContext<'a>,
        authority: &'a TechnicalCampaignAuthority,
        request_sha256: &'a str,
    },
}

struct SelectedMutation {
    incumbent_prompt: String,
    incumbent_version: u64,
}

struct ReinforceRun {
    report: ReinforceReport,
    last_selected: Option<SelectedMutation>,
}

#[allow(clippy::too_many_arguments)]
fn run_frozen_technical_proposal(
    generator: &dyn Generator,
    frozen: campaign::FrozenTrainingProposal,
    cases: &[HeldoutCase<'_>],
    config: &PromotionConfig,
    manifest: &CohortManifest,
    evaluators: &[&dyn evaluator::PolicyEvaluator],
    receipt_store: ReceiptStoreContext<'_>,
) -> Result<ReinforceRun, String> {
    let promotion = evaluate_promotion_with_receipts(
        generator,
        ReceiptPromotionRequest {
            incumbent_prompt: frozen.incumbent_prompt(),
            candidate_prompt: frozen.candidate_prompt(),
            incumbent_version: frozen.incumbent_version(),
            cases,
            config,
            manifest,
            evaluators,
            receipt_store,
        },
    )?;
    let optimized = promotion.promoted();
    let incumbent_prompt = frozen.incumbent_prompt().to_string();
    let incumbent_version = frozen.incumbent_version();
    let prompt = if optimized {
        frozen.candidate_prompt().to_string()
    } else {
        incumbent_prompt.clone()
    };
    let version = incumbent_version + u64::from(optimized);
    let last_selected = optimized.then_some(SelectedMutation {
        incumbent_prompt,
        incumbent_version,
    });
    Ok(ReinforceRun {
        report: ReinforceReport {
            rounds: vec![RoundReport {
                version,
                metrics: frozen.metrics(),
                best_reward: frozen.best_reward(),
                optimized,
                promotion: Some(promotion),
                prompt: prompt.clone(),
            }],
            final_prompt: prompt,
        },
        last_selected,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_reinforce_inner(
    generator: &dyn Generator,
    reward: &dyn Reward,
    reflector: &dyn Reflector,
    task: &str,
    initial_prompt: &str,
    rounds: usize,
    cfg: &ReinforceConfig,
    promotion_gate: Option<PromotionGate<'_>>,
) -> Result<ReinforceRun, String> {
    let mut prompt = initial_prompt.to_string();
    let mut version = 0u64;
    let mut report = ReinforceReport::default();
    let mut last_selected = None;

    for round in 0..rounds {
        telemetry::round(round + 1, version);
        let cands = generator.generate(&prompt, task, version, cfg.dispatch_count());
        let (rollouts, mut metrics) = evaluate_batch(reward, version, cfg, cands);

        let accepted: Vec<&Rollout> = rollouts.iter().filter(|r| r.accepted).collect();
        let best = accepted
            .iter()
            .max_by(|a, b| a.reward.total_cmp(&b.reward))
            .copied();
        let worst = accepted
            .iter()
            .min_by(|a, b| a.reward.total_cmp(&b.reward))
            .copied();
        let mut best_reward = best.map_or(f32::NEG_INFINITY, |r| r.reward);
        telemetry::batch(
            telemetry::BatchCounters {
                generated: metrics.generated,
                accepted: metrics.accepted,
                rejected_stale: metrics.rejected_stale,
                rejected_straggler: metrics.rejected_straggler,
                solve_rate: metrics.solve_rate,
                advantage_variance: metrics.advantage_variance,
            },
            Some(best_reward),
        );

        let mut optimized = false;
        let mut promotion = None;
        if metrics.has_learning_signal()
            && let (Some(b), Some(w)) = (best, worst)
        {
            telemetry::phase(telemetry::RlPhase::Reflection);
            if let Ok(mut new_prompt) = reflector.improve(&prompt, task, &b.output, &w.output)
                && !new_prompt.trim().is_empty()
                && new_prompt.trim() != prompt.trim()
            {
                if let Some(gate) = promotion_gate.as_ref() {
                    let gate_report = match gate {
                        PromotionGate::CandidateOutput {
                            cases,
                            config,
                            manifest,
                        } => evaluate_promotion(
                            generator,
                            &prompt,
                            &new_prompt,
                            version,
                            cases,
                            config,
                            manifest,
                        )?,
                        PromotionGate::TechnicalReceipts {
                            cases,
                            config,
                            manifest,
                            evaluators,
                            receipt_store,
                            authority,
                            request_sha256,
                        } => {
                            let frozen = authority.install_or_adopt_candidate(
                                request_sha256,
                                &prompt,
                                version,
                                &new_prompt,
                                metrics.clone(),
                                best_reward,
                            )?;
                            new_prompt = frozen.candidate_prompt().to_string();
                            metrics = frozen.metrics();
                            best_reward = frozen.best_reward();
                            evaluate_promotion_with_receipts(
                                generator,
                                ReceiptPromotionRequest {
                                    incumbent_prompt: &prompt,
                                    candidate_prompt: &new_prompt,
                                    incumbent_version: version,
                                    cases,
                                    config,
                                    manifest,
                                    evaluators,
                                    receipt_store: *receipt_store,
                                },
                            )?
                        }
                    };
                    optimized = gate_report.promoted();
                    promotion = Some(gate_report);
                } else {
                    optimized = true;
                }
                if optimized {
                    last_selected = Some(SelectedMutation {
                        incumbent_prompt: prompt.clone(),
                        incumbent_version: version,
                    });
                    prompt = new_prompt;
                    version += 1;
                }
            }
        }

        report.rounds.push(RoundReport {
            version,
            metrics,
            best_reward,
            optimized,
            promotion,
            prompt: prompt.clone(),
        });
    }

    report.final_prompt = prompt;
    Ok(ReinforceRun {
        report,
        last_selected,
    })
}

#[derive(Debug, Clone)]
pub struct CodingEvalReport {
    pub answer: String,
    pub test: crate::agent::harness::TestOutcome,
    pub reward: f32,
    pub evaluator_artifact: Option<PathBuf>,
    pub training_decision: Option<String>,
    pub training_capture_error: Option<String>,
    pub root_trajectory: serde_json::Value,
    pub harness_treatment: serde_json::Value,
}

fn parse_evaluator_test_result(output: &str) -> Result<crate::agent::harness::TestOutcome, String> {
    let mut outcome = crate::agent::harness::TestOutcome::default();
    for line in output.lines() {
        let Some(summary_text) = line.trim_start().strip_prefix("test result:") else {
            continue;
        };
        let tokens: Vec<&str> = summary_text
            .split(|c: char| c == ';' || c.is_whitespace())
            .filter(|token| !token.is_empty())
            .collect();
        let status = tokens
            .first()
            .map(|token| token.trim_end_matches('.').to_ascii_lowercase())
            .ok_or_else(|| "evaluator test summary is missing its status".to_string())?;
        if !matches!(status.as_str(), "ok" | "failed") {
            return Err("evaluator test summary has an invalid status".to_string());
        }
        let mut summary = crate::agent::harness::TestOutcome::default();
        let mut seen = [false; 3];
        for pair in tokens.windows(2) {
            let (target, index) = match pair[1] {
                "passed" => (&mut summary.passed, 0),
                "failed" => (&mut summary.failed, 1),
                "ignored" => (&mut summary.ignored, 2),
                _ => continue,
            };
            if seen[index] {
                return Err(format!(
                    "evaluator test summary repeats its {} count",
                    pair[1]
                ));
            }
            seen[index] = true;
            *target = pair[0]
                .parse::<usize>()
                .map_err(|_| format!("evaluator test summary has an invalid {} count", pair[1]))?;
        }
        if !seen[0] || !seen[1] {
            return Err("evaluator test summary requires passed and failed counts".to_string());
        }
        if status == "failed" && summary.failed == 0 {
            return Err("evaluator FAILED summary has no failed tests".to_string());
        }
        for (total, count, label) in [
            (&mut outcome.passed, summary.passed, "passed"),
            (&mut outcome.failed, summary.failed, "failed"),
            (&mut outcome.ignored, summary.ignored, "ignored"),
        ] {
            *total = total
                .checked_add(count)
                .ok_or_else(|| format!("evaluator test summary {label} count overflows"))?;
        }
    }
    outcome
        .passed
        .checked_add(outcome.failed)
        .ok_or_else(|| "evaluator executed test count overflows".to_string())?;
    Ok(outcome)
}

fn validate_coding_eval_evidence(evidence: &EvaluatorEvidence) -> Result<(), String> {
    evidence.validate_for_scoring()?;
    evidence.require_verifier_contract(
        "coding-eval",
        &[TEST_VERIFIER_CONTRACT, CODE_HEALTH_VERIFIER_CONTRACT],
    )?;
    if !evidence.succeeded() {
        return Err(format!(
            "{} evaluator command failed with exit {:?}",
            evidence.source(),
            evidence.exit_code()
        ));
    }
    let correctness = parse_evaluator_test_result(evidence.output())?;
    if correctness.failed > 0 {
        return Err(format!(
            "{} evaluator reported {} failed test(s) despite a successful exit",
            evidence.source(),
            correctness.failed
        ));
    }
    Ok(())
}

fn score_coding_eval(evidence: &EvaluatorEvidence) -> Result<f32, String> {
    TestReward.score(RewardInput::EvaluatorEvidence(evidence))
}

pub fn score_coding_eval_reward(
    _answer: &str,
    evidence: &EvaluatorEvidence,
) -> Result<f32, String> {
    score_coding_eval(evidence)
}

pub fn run_coding_eval(
    club: &dyn Club,
    registry: &crate::agent::harness::ToolRegistry,
    task_prompt: &str,
    verify_command: &str,
) -> Result<CodingEvalReport, String> {
    crate::agent::harness::run_identity::configure_dataset(
        crate::agent::harness::run_identity::Dataset::new("eval", None)?,
    );
    crate::agent::harness::run_identity::configure_verifier(serde_json::json!({
        "plan_kind":"coding-eval", "command":verify_command, "accept_cmd":"none", "evaluator_id":TEST_VERIFIER_CONTRACT,
    }));
    let mut history = vec![
        ChatMsg::system(crate::agent::harness::book::ar_seats::CODING_AGENT.cells()),
        ChatMsg::user(task_prompt),
    ];
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let (event_tx, _) = std::sync::mpsc::channel::<crate::agent::harness::TurnEvent>();
    let answer = {
        let _eval_owns_the_label = crate::agent::harness::EvalLabelScope::new();
        crate::agent::harness::run_turn(
            club,
            registry,
            &mut history,
            &cancel,
            Some(crate::agent::harness::default_max_hops()),
            &event_tx,
        )?
    };
    let subject = format!(
        "task={}:{}\nanswer={}:{}",
        task_prompt.len(),
        task_prompt,
        answer.len(),
        answer
    );
    let evidence = EvaluatorEvidence::run_shell(
        "run_coding_eval verifier",
        verify_command,
        registry.current_workspace(),
        TEST_VERIFIER_CONTRACT,
        &subject,
    )?;
    finish_coding_eval(
        club.label(),
        &history,
        task_prompt,
        &answer,
        &evidence,
        None,
        None,
    )
}

pub(crate) fn recovery_training_supported(evidence: &EvaluatorEvidence) -> Result<(), String> {
    validate_coding_eval_evidence(evidence)?;
    let test = parse_evaluator_test_result(evidence.output())?;
    if test.passed > 0 {
        Ok(())
    } else {
        Err("verifier produced no supported typed coding measurement".into())
    }
}

pub(crate) fn finish_coding_eval(
    club_label: &str,
    history: &[ChatMsg],
    task_prompt: &str,
    answer: &str,
    evidence: &EvaluatorEvidence,
    trajectory_output: Option<&Path>,
    captured_authority: Option<&Path>,
) -> Result<CodingEvalReport, String> {
    let test = parse_evaluator_test_result(evidence.output())?;
    let reward = score_coding_eval(evidence)?;
    let evaluator_artifact = artifact::persist_if_configured(evidence)?;
    let root_trajectory = crate::agent::harness::root_trajectory_json(history);
    let harness_treatment = crate::agent::harness::harness_treatment_json();
    let capture = match captured_authority {
        Some(root) => training::publish(root, task_prompt, answer, evidence, reward).map(Some),
        None => training::publish_if_configured(task_prompt, answer, evidence, reward),
    };
    let (training_decision, training_capture_error) = match capture {
        Ok(Some(published)) => {
            let emitted = crate::agent::harness::log_verified_coding_eval_trajectory(
                club_label,
                history,
                answer,
                task_prompt,
                &published,
            )
            .and_then(|row| {
                if let Some(path) = trajectory_output {
                    crate::agent::harness::append_trajectory(path, &row)
                        .map_err(|e| e.to_string())?;
                }
                Ok(())
            });
            (Some(published.decision_sha256), emitted.err())
        }
        Ok(None) => (None, None),
        Err(error) => (None, Some(error)),
    };
    Ok(CodingEvalReport {
        answer: answer.to_owned(),
        test,
        reward,
        evaluator_artifact,
        training_decision,
        training_capture_error,
        root_trajectory,
        harness_treatment,
    })
}

pub fn reverse_task(k: usize, seed: u64) -> (String, String) {
    let mut s = seed;
    let mut next = || {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) % 10
    };
    let digits: Vec<u64> = (0..k).map(|_| next()).collect();
    let join = |it: &mut dyn Iterator<Item = &u64>| {
        it.map(|d| d.to_string()).collect::<Vec<_>>().join(" ")
    };
    let prompt = format!("reverse: {}", join(&mut digits.iter()));
    let expected = join(&mut digits.iter().rev());
    (prompt, expected)
}

pub fn reverse_reward_score(output: &str, expected: &str) -> f32 {
    let exp: Vec<&str> = expected.split_whitespace().collect();
    if exp.is_empty() {
        return 0.0;
    }
    let toks: Vec<&str> = output
        .split_whitespace()
        .filter(|t| t.chars().all(|c| c.is_ascii_digit()) && !t.is_empty())
        .collect();
    let tail = if toks.len() >= exp.len() {
        &toks[toks.len() - exp.len()..]
    } else {
        &toks[..]
    };
    let correct = exp
        .iter()
        .enumerate()
        .filter(|(i, e)| tail.get(*i) == Some(*e))
        .count();
    correct as f32 / exp.len() as f32
}

pub struct ReverseControlReward {
    pub expected: String,
}

impl Reward for ReverseControlReward {
    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let output = input.candidate_output(self.label())?;
        Ok(reverse_reward_score(output, &self.expected))
    }
    fn label(&self) -> &str {
        "reverse-control"
    }
}

fn wrong_reverse_answer(expected: &str) -> String {
    expected
        .split_whitespace()
        .map(|t| {
            t.parse::<u8>()
                .map(|d| ((d + 1) % 10).to_string())
                .unwrap_or_else(|_| "x".to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn control_group(quality: f32, k: usize, group: usize, seed: u64) -> (String, Vec<Candidate>) {
    let (_, expected) = reverse_task(k, seed);
    let wrong = wrong_reverse_answer(&expected);
    let n_correct = (quality.clamp(0.0, 1.0) * group as f32).round() as usize;
    let cands = (0..group)
        .map(|i| Candidate {
            policy_version: 0,
            latency: Duration::ZERO,
            output: if i < n_correct {
                expected.clone()
            } else {
                wrong.clone()
            },
        })
        .collect();
    (expected, cands)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/reinforce/reinforce__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/reinforce/reinforce__reward_boundary_tests.rs"]
mod reward_boundary_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/reinforce/reward_boundary_baseline_tests.rs"]
mod reward_boundary_baseline_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/reinforce/reward_correctness_tests.rs"]
mod reward_correctness_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/reinforce/reward_counter_tests.rs"]
mod reward_counter_tests;
