//! Single owned recovery verifier execution: retain raw utility even when its
//! output does not qualify for a typed training decision.
use super::*;
use std::sync::atomic::AtomicBool;

pub(crate) struct RecoveryEvalRequest<'a> {
    pub command: &'a str,
    pub workspace: &'a Path,
    pub scratch: &'a Path,
    pub task: &'a str,
    pub answer: &'a str,
    /// `None` = the controller declined a time cap; the verifier is then
    /// bounded by cancellation alone.
    pub timeout: Option<Duration>,
    pub cancel: &'a AtomicBool,
}

pub(crate) struct RecoveryEvalExecution {
    pub evidence: EvaluatorEvidence,
    pub cancelled: bool,
}

pub(crate) fn run(request: RecoveryEvalRequest<'_>) -> Result<RecoveryEvalExecution, String> {
    if request.cancel.load(Ordering::Acquire) {
        return Err("recovery evaluator cancelled before dispatch".into());
    }
    let workspace = request
        .workspace
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let paths_before = crate::harness::workspace_evidence_paths(&workspace);
    let before = crate::harness::workspace_evidence_sha256(&workspace)
        .ok_or("recovery evaluator requires a Git-backed workspace")?;
    std::fs::create_dir_all(request.scratch).map_err(|e| e.to_string())?;
    let shell = std::fs::canonicalize("/bin/sh").map_err(|e| e.to_string())?;
    let shell_sha256 = crate::cut::sha256_hex(&std::fs::read(&shell).map_err(|e| e.to_string())?);
    // Preserve the ordinary operator command's inherited environment, while
    // confining all writes and network access. Only the environment hash is
    // retained; no credential values are exported into evidence or a row.
    let mut environment = std::env::vars_os().collect::<Vec<_>>();
    for (key, value) in [
        ("TMPDIR", request.scratch.to_path_buf()),
        ("TMP", request.scratch.to_path_buf()),
        ("TEMP", request.scratch.to_path_buf()),
        ("XDG_CACHE_HOME", request.scratch.join("cache")),
    ] {
        environment.retain(|(name, _)| name != key);
        environment.push((key.into(), value.into_os_string()));
    }
    environment.sort_by(|left, right| os_bytes(&left.0).cmp(os_bytes(&right.0)));
    let mut canonical = Vec::new();
    append_manifest_bytes(
        &mut canonical,
        b"schema",
        b"angel.recovery-evaluator-execution/v1",
    );
    append_manifest_bytes(&mut canonical, b"shell-sha256", shell_sha256.as_bytes());
    append_manifest_bytes(
        &mut canonical,
        b"writable-workspace",
        os_bytes(workspace.as_os_str()),
    );
    append_manifest_bytes(
        &mut canonical,
        b"writable-scratch",
        os_bytes(request.scratch.as_os_str()),
    );
    append_manifest_bytes(
        &mut canonical,
        b"timeout-ns",
        request
            .timeout
            .map(|timeout| timeout.as_nanos().to_string())
            .unwrap_or_else(|| "none".to_string())
            .as_bytes(),
    );
    append_manifest_bytes(&mut canonical, b"mandatory-network-denied", b"true");
    for (key, value) in &environment {
        append_manifest_bytes(&mut canonical, os_bytes(key), os_bytes(value));
    }
    let policy = crate::sandbox::SandboxPolicy {
        writable_roots: vec![workspace.clone(), request.scratch.to_path_buf()],
        allow_network: false,
        enforce: true,
        mandatory: true,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    };
    let mut process = crate::sandbox::command(&shell, ["-c", request.command], &policy)
        .map_err(|e| e.to_string())?;
    process.current_dir(&workspace).env_clear();
    for (key, value) in &environment {
        process.env(key, value);
    }
    crate::sandbox::set_helper_policy(&mut process, &policy).map_err(|e| e.to_string())?;
    if request.cancel.load(Ordering::Acquire) {
        return Err("recovery evaluator cancelled before dispatch".into());
    }
    let started = Instant::now();
    // The controller's independent deadline watcher also sets this cancel flag;
    // even a parent YOLO timeout override cannot lift cancellation ownership.
    let capture = crate::harness::output_timed_captured_cancellable(
        process,
        request.timeout,
        Some(request.cancel),
    )?;
    let cancelled = capture.cancelled || request.cancel.load(Ordering::Acquire);
    let after = crate::harness::workspace_evidence_sha256(&workspace)
        .ok_or("recovery evaluator destroyed its Git-backed workspace")?;
    let evidence = EvaluatorEvidence::from_captured_output(
        "native recovery evaluator",
        request.command,
        &workspace,
        TEST_VERIFIER_CONTRACT,
        &training::subject(request.task, request.answer),
        CapturedEvaluatorExecution {
            sandbox_stderr_prefix_len: captured_launcher_stderr_prefix_len(&capture.output.stderr),
            output: capture.output,
            workspace_before_sha256: before,
            workspace_changed_paths: changed_workspace_paths(
                paths_before,
                crate::harness::workspace_evidence_paths(&workspace),
            ),
            workspace_sha256: after,
            execution_policy_sha256: crate::cut::sha256_hex(&canonical),
            execution_id: next_evaluator_execution_id(),
            duration_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
            timed_out: capture.timed_out,
            stdout_total_bytes: capture.stdout_total_bytes,
            stderr_total_bytes: capture.stderr_total_bytes,
            stdout_truncated: capture.stdout_truncated,
            stderr_truncated: capture.stderr_truncated,
        },
    )?;
    Ok(RecoveryEvalExecution {
        evidence,
        cancelled,
    })
}
