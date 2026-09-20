use super::credit::derive_credits;
use super::schema::{NodeState, ReviewVerdict, RunState, SwarmRun};
use super::tool::SwarmCompilerEngine;
use super::verify::{CommandSpec, RefVerifier, regression_guard};

impl SwarmCompilerEngine {
    pub(super) fn finalize_verification(
        &self,
        run: &mut SwarmRun,
        verifier: &RefVerifier,
    ) -> Result<(), String> {
        if run.verification.candidate_accept.is_none() {
            self.begin_node(run, "verifier")?;
            let candidate = contribution_branch(run, "implementer")?;
            let quality_labels = (0..run.quality_cmds.len())
                .map(|index| format!("quality_{}", index + 1))
                .collect::<Vec<_>>();
            let mut specs = vec![
                CommandSpec {
                    label: "candidate_target",
                    command: &run.targeted_test_cmd,
                    marker: Some(&run.red_marker),
                },
                CommandSpec {
                    label: "candidate_accept",
                    command: &run.accept_cmd,
                    marker: None,
                },
            ];
            for (label, command) in quality_labels.iter().zip(&run.quality_cmds) {
                specs.push(CommandSpec {
                    label,
                    command,
                    marker: None,
                });
            }
            let mut proofs = verifier.run(&candidate, &specs)?.into_iter();
            run.verification.candidate_target = proofs.next();
            run.verification.candidate_accept = proofs.next();
            run.verification.quality = proofs.collect();
            self.persist(
                run,
                "candidate_measured",
                "targeted, full, and quality commands executed on candidate",
            )?;
        }

        let baseline = run
            .verification
            .baseline_accept
            .as_ref()
            .ok_or_else(|| "baseline proof disappeared".to_string())?;
        let red = run
            .verification
            .test_only_target
            .as_ref()
            .ok_or_else(|| "test-only proof disappeared".to_string())?;
        let target = run
            .verification
            .candidate_target
            .as_ref()
            .ok_or_else(|| "candidate targeted proof disappeared".to_string())?;
        let accept = run
            .verification
            .candidate_accept
            .as_ref()
            .ok_or_else(|| "candidate acceptance proof disappeared".to_string())?;
        run.verification.differential = !red.success
            && !red.timed_out
            && red.marker_seen
            && target.success
            && !target.timed_out
            && !target.marker_seen;
        run.verification.regression_guard = regression_guard(baseline, accept);
        let quality_pass = run
            .verification
            .quality
            .iter()
            .all(|proof| proof.success && !proof.timed_out);
        run.verification.technical_pass = run.verification.differential
            && run.verification.regression_guard
            && accept.success
            && !accept.timed_out
            && quality_pass;
        run.verification.review_pass = run
            .contribution("reviewer")
            .is_some_and(|item| item.review_verdict == Some(ReviewVerdict::Pass));
        run.verification.passed = run.verification.technical_pass && run.verification.review_pass;
        run.credits = derive_credits(run);
        self.finish_node(run, "verifier", NodeState::Completed)?;

        if let Err(error) = self.policy.record(&run.id, &run.task_type, &run.credits) {
            let _ = self
                .store
                .event(&run.id, "warn", "routing_policy_update_failed", &error);
        } else {
            let _ = self.store.event(
                &run.id,
                "decision",
                "routing_policy_updated",
                "post-action credits folded into task-role priors",
            );
        }

        run.state = if run.verification.passed {
            RunState::Verified
        } else {
            RunState::Rejected
        };
        run.last_error = if run.verification.passed {
            None
        } else if !run.verification.review_pass {
            Some("adversarial reviewer blocked the candidate".to_string())
        } else {
            Some("candidate failed one or more deterministic proof gates".to_string())
        };
        let event = if run.verification.passed {
            "run_verified"
        } else {
            "run_rejected"
        };
        self.persist(
            run,
            event,
            "post-action verification and contribution credit complete",
        )?;
        self.cleanup_test_branch(run);
        Ok(())
    }

    fn cleanup_test_branch(&self, run: &SwarmRun) {
        let Some(branch) = run
            .contribution("test_author")
            .and_then(|item| item.branch.as_deref())
        else {
            return;
        };
        if run.parked_branch.as_deref() == Some(branch) {
            return;
        }
        if let Err(error) = self.delegate.discard_branch(branch) {
            let _ = self
                .store
                .event(&run.id, "warn", "test_branch_cleanup_failed", &error);
        }
    }
}

fn contribution_branch(run: &SwarmRun, role: &str) -> Result<String, String> {
    run.contribution(role)
        .and_then(|item| item.branch.clone())
        .ok_or_else(|| format!("{role} contribution has no durable branch"))
}
