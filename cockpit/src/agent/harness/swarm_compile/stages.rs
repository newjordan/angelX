use super::credit::review_verdict;
use super::prompts;
use super::schema::{NodeState, SwarmRun};
use super::store::compact_scrubbed;
use super::tool::SwarmCompilerEngine;
use super::verify::{CommandSpec, RefVerifier, changed_paths, paths_disjoint, paths_within_scope};
use crate::agent::harness::{DelegateMode, run_git};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

impl SwarmCompilerEngine {
    pub(super) fn execute_inner(
        &self,
        run: &mut SwarmRun,
        cancelled: Option<&AtomicBool>,
    ) -> Result<(), String> {
        let repo = PathBuf::from(&run.repo_root);
        let workspace_rel = workspace_rel(run);
        let verifier = RefVerifier::new(
            repo.clone(),
            workspace_rel,
            self.store.scratch_dir(&run.id)?,
        );
        ensure_not_cancelled(cancelled)?;
        if !self.ensure_baseline(run, &verifier)? {
            return Ok(());
        }
        ensure_not_cancelled(cancelled)?;
        self.ensure_investigator(run)?;
        ensure_not_cancelled(cancelled)?;
        if !self.ensure_test_author(run, &repo, &verifier)? {
            return Ok(());
        }
        ensure_not_cancelled(cancelled)?;
        if !self.ensure_implementer(run, &repo)? {
            return Ok(());
        }
        ensure_not_cancelled(cancelled)?;
        self.ensure_reviewer(run, &repo)?;
        ensure_not_cancelled(cancelled)?;
        if run.state.terminal() {
            return Ok(());
        }
        self.finalize_verification(run, &verifier)
    }

    fn ensure_baseline(&self, run: &mut SwarmRun, verifier: &RefVerifier) -> Result<bool, String> {
        if run.verification.baseline_accept.is_none() {
            let proof = verifier
                .run(
                    &run.base_oid,
                    &[CommandSpec {
                        label: "baseline_accept",
                        command: &run.accept_cmd,
                        marker: None,
                    }],
                )?
                .remove(0);
            run.verification.baseline_accept = Some(proof);
            self.persist(
                run,
                "baseline_measured",
                "full acceptance measured on frozen base",
            )?;
        }
        let passed = run
            .verification
            .baseline_accept
            .as_ref()
            .is_some_and(super::verify::baseline_passes);
        if !passed {
            self.reject(
                run,
                "base revision does not pass the full acceptance command",
            )?;
        }
        Ok(passed)
    }

    fn ensure_investigator(&self, run: &mut SwarmRun) -> Result<(), String> {
        if run.contribution("investigator").is_some() {
            return Ok(());
        }
        let base = run.base_oid.clone();
        let outcome = self.delegate_stage(
            run,
            "investigator",
            DelegateMode::ReadOnly,
            &base,
            prompts::investigator(&run.goal, &run.targeted_test_cmd, &run.accept_cmd),
        )?;
        let ephemeral = outcome.branch.clone();
        self.record_contribution(run, "investigator", &outcome, None, Vec::new(), None)?;
        self.discard_ephemeral(&run.id, &ephemeral);
        Ok(())
    }

    fn ensure_test_author(
        &self,
        run: &mut SwarmRun,
        repo: &std::path::Path,
        verifier: &RefVerifier,
    ) -> Result<bool, String> {
        if run.contribution("test_author").is_none() {
            let findings = run
                .contribution("investigator")
                .map(|item| item.summary.clone())
                .unwrap_or_default();
            let base = run.base_oid.clone();
            let outcome = self.delegate_stage(
                run,
                "test_author",
                DelegateMode::Write,
                &base,
                prompts::test_author(
                    &run.goal,
                    &findings,
                    &run.test_scope,
                    &run.targeted_test_cmd,
                    &run.red_marker,
                ),
            )?;
            let branch = outcome.branch.clone();
            let paths = changed_paths(repo, &run.base_oid, &branch)?;
            let in_scope = paths_within_scope(&paths, &run.test_scope);
            self.record_contribution(
                run,
                "test_author",
                &outcome,
                Some(branch.clone()),
                paths,
                None,
            )?;
            if !in_scope {
                run.parked_branch = Some(branch);
                self.finish_node(run, "test_author", NodeState::Failed)?;
                self.reject(
                    run,
                    "test author changed no files or wrote outside test_scope",
                )?;
                return Ok(false);
            }
        }

        if run.verification.test_only_target.is_none() {
            let branch = contribution_branch(run, "test_author")?;
            let proof = verifier
                .run(
                    &branch,
                    &[CommandSpec {
                        label: "test_only_target",
                        command: &run.targeted_test_cmd,
                        marker: Some(&run.red_marker),
                    }],
                )?
                .remove(0);
            run.verification.test_only_target = Some(proof);
            self.persist(run, "red_proof_measured", "test-only branch executed")?;
        }
        let proof = run
            .verification
            .test_only_target
            .as_ref()
            .ok_or_else(|| "test-only proof disappeared".to_string())?;
        let valid_red =
            !proof.success && !proof.timed_out && proof.marker_seen && proof.workspace_clean;
        if !valid_red {
            run.parked_branch = Some(contribution_branch(run, "test_author")?);
            self.finish_node(run, "test_author", NodeState::Failed)?;
            self.reject(
                run,
                "targeted test did not produce an intentional marker-bearing red result",
            )?;
        }
        Ok(valid_red)
    }

    fn ensure_implementer(
        &self,
        run: &mut SwarmRun,
        repo: &std::path::Path,
    ) -> Result<bool, String> {
        if run.contribution("implementer").is_some() {
            return Ok(true);
        }
        let findings = run
            .contribution("investigator")
            .map(|item| item.summary.clone())
            .unwrap_or_default();
        let test_paths = run
            .contribution("test_author")
            .map(|item| item.changed_paths.clone())
            .unwrap_or_default();
        let test_branch = contribution_branch(run, "test_author")?;
        let outcome = self.delegate_stage(
            run,
            "implementer",
            DelegateMode::Write,
            &test_branch,
            prompts::implementer(
                &run.goal,
                &findings,
                &test_paths,
                &run.targeted_test_cmd,
                &run.accept_cmd,
            ),
        )?;
        let branch = outcome.branch.clone();
        let impl_paths = changed_paths(repo, &test_branch, &branch)?;
        let immutable = !impl_paths.is_empty() && paths_disjoint(&test_paths, &impl_paths);
        self.record_contribution(
            run,
            "implementer",
            &outcome,
            Some(branch.clone()),
            impl_paths,
            None,
        )?;
        run.parked_branch = Some(branch);
        if !immutable {
            self.finish_node(run, "implementer", NodeState::Failed)?;
            self.reject(
                run,
                "implementation was empty or modified protected test files",
            )?;
        }
        Ok(immutable)
    }

    fn ensure_reviewer(&self, run: &mut SwarmRun, repo: &std::path::Path) -> Result<(), String> {
        if run.contribution("reviewer").is_some() {
            return Ok(());
        }
        let candidate = contribution_branch(run, "implementer")?;
        let full_diff = run_git(repo, &["diff", &run.base_oid, &candidate])?;
        if full_diff.trim().is_empty() {
            self.reject(run, "candidate branch has no diff from the frozen base")?;
            return Ok(());
        }
        let outcome = self.delegate_stage(
            run,
            "reviewer",
            DelegateMode::ReadOnly,
            &candidate,
            prompts::reviewer(
                &run.goal,
                &compact_scrubbed(&full_diff, 8_000),
                &run.targeted_test_cmd,
                &run.accept_cmd,
            ),
        )?;
        let verdict = review_verdict(&outcome.answer);
        let ephemeral = outcome.branch.clone();
        self.record_contribution(run, "reviewer", &outcome, None, Vec::new(), Some(verdict))?;
        self.discard_ephemeral(&run.id, &ephemeral);
        Ok(())
    }

    fn discard_ephemeral(&self, run_id: &str, branch: &str) {
        if let Err(error) = self.delegate.discard_branch(branch) {
            let _ = self
                .store
                .event(run_id, "warn", "ephemeral_branch_cleanup_failed", &error);
        }
    }
}

fn ensure_not_cancelled(cancelled: Option<&AtomicBool>) -> Result<(), String> {
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Err("campaign execution cancelled by operator".to_string());
    }
    Ok(())
}

fn contribution_branch(run: &SwarmRun, role: &str) -> Result<String, String> {
    run.contribution(role)
        .and_then(|item| item.branch.clone())
        .ok_or_else(|| format!("{role} contribution has no durable branch"))
}

fn workspace_rel(run: &SwarmRun) -> PathBuf {
    if run.workspace_rel == "." {
        PathBuf::new()
    } else {
        PathBuf::from(&run.workspace_rel)
    }
}
