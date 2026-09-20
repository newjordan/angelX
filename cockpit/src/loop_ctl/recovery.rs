//! One supervised, isolated experiment beside the ordinary loop flight slot.
use super::*;
use crate::harness::{LoopExperimentRequest, LoopExperimentResult, run_loop_experiment};
use std::sync::atomic::{AtomicBool, Ordering};

const EXPERIMENT_TOKENS: usize = 32_768;
const EXPERIMENT_SECONDS: u64 = 300;

fn experiment_due(st: &LoopState) -> bool {
    // Healthy fast wins must not starve the percentage-cut lane. Ordinary
    // loops admit recovery at a stall; PODRACE also maintains its deep lane.
    (st.podrace && st.iteration >= DEFAULT_LOOP_FIRST_CANDIDATE_ITERS) || stall_limit_reached(st)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExperimentRecord {
    pub key: String,
    pub hypothesis: String,
    pub iteration: usize,
    #[serde(default)]
    pub settled_iteration: Option<usize>,
    pub artifact_dir: PathBuf,
    pub status: String,
    pub reserved_tokens: usize,
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) context_ref: Option<crate::club::RecoveryContextRef>,
    #[serde(default)]
    pub(crate) context_is_supervisor_only: bool,
}

pub(crate) struct ExperimentPending {
    pub run_id: String,
    pub workspace: PathBuf,
    pub key: String,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<Result<LoopExperimentResult, String>>,
}

impl Drop for ExperimentPending {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

fn recovery_workspace_sha256(workspace: &Path) -> String {
    crate::cut::sha256_hex(workspace.as_os_str().as_encoded_bytes())
}

fn recovery_reference(
    run_id: &str,
    workspace: &Path,
    key: &str,
    summary: &str,
    producer: Option<crate::club::RecoveryProducerRef>,
) -> crate::club::RecoveryContextRef {
    let workspace_sha256 = recovery_workspace_sha256(workspace);
    crate::club::RecoveryContextRef {
        import_id: crate::cut::sha256_hex(
            &serde_json::to_vec(&(run_id, &workspace_sha256, key))
                .expect("import identity serializes"),
        ),
        loop_run_id: run_id.into(),
        workspace_sha256,
        experiment_key_sha256: crate::cut::sha256_hex(key.as_bytes()),
        summary_sha256: crate::cut::sha256_hex(summary.as_bytes()),
        producer,
    }
}

fn allowance(st: &LoopState) -> Option<(usize, u64)> {
    if budget_tripped(st).is_some() {
        return None;
    }
    let tokens = if st.token_budget == 0 {
        EXPERIMENT_TOKENS
    } else {
        (st.token_budget.saturating_sub(st.tokens_spent) / 4).min(EXPERIMENT_TOKENS)
    };
    let seconds = if st.deadline_secs == 0 {
        EXPERIMENT_SECONDS
    } else {
        st.deadline_secs
            .saturating_sub(now_ms().saturating_sub(st.started_ms) / 1000)
            .min(EXPERIMENT_SECONDS)
    };
    (tokens >= 4096 && seconds > 1).then_some((tokens, seconds))
}

impl crate::App {
    pub(crate) fn loop_start_experiment_if_due(&mut self) {
        if self.loop_experiment.is_some()
            || self.exit_request.is_some()
            || self.loop_ctl.status != LoopStatus::Running
            || self.loop_ctl.retry_after_error
            || self.loop_ctl.execution_blocker.is_some()
            || !(self.loop_first_candidate_overdue() || experiment_due(&self.loop_ctl))
        {
            return;
        }
        // A completed experiment gets at least three parent cycles for review.
        if self.loop_ctl.experiments.last().is_some_and(|last| {
            self.loop_ctl
                .iteration
                .saturating_sub(last.settled_iteration.unwrap_or(last.iteration))
                < 3
        }) {
            return;
        }
        let Some((tokens, seconds)) = allowance(&self.loop_ctl) else {
            return;
        };
        // Direct seat: no failover, automatic escalation, Deli, or swarm wrapper.
        let club = match self.loop_ctl.tier {
            EscalationTier::Sota => self.loop_sota_club().or_else(|| self.loop_local_club()),
            EscalationTier::Local | EscalationTier::Swarm => self.loop_local_club(),
        }
        .unwrap_or_else(|| self.bag.in_hand());
        if club.label() == "practice" {
            return;
        }
        let workspace = self.tools.current_workspace().to_path_buf();
        let hypothesis = self
            .loop_ctl
            .hypotheses
            .last()
            .cloned()
            .unwrap_or_else(|| self.loop_task_text());
        // Harvest already captured this advisory dedup identity. The worker
        // copies and verifies live bytes off-thread before making a model call.
        let Some(fingerprint) = self.loop_ctl.last_workspace_fingerprint else {
            return;
        };
        let key = format!("{fingerprint:016x}:{}", normalize(&hypothesis));
        if self
            .loop_ctl
            .experiments
            .iter()
            .any(|attempt| attempt.key == key)
        {
            return;
        }
        let artifact_dir = angel_subdir("loop-experiments").join(gen_id());
        let request = LoopExperimentRequest {
            workspace: workspace.clone(),
            artifact_dir: artifact_dir.clone(),
            task: format!(
                "[LOOP RECOVERY EXPERIMENT] Parent objective: {}\nHypothesis: {}\nOperator steering: {}\nRecent evidence: {}\nRun one discriminating local experiment with the available file, shell, and evaluation tools. Inspect repository instructions and discover its actual benchmark or RL evaluation workflow. Use a fixed baseline and report comparable measurements, failed checks, and the next decision. Preserve an unfinished experiment and its logs. No competitive submissions, redraws, external publishing, model switching, or nested workers. The parent owns fast wins and any integration. Return artifact paths and an honest result; no objective credit for prose or process exit alone.",
                self.loop_task_text(),
                hypothesis,
                self.loop_ctl.steer_notes.join("\n"),
                self.loop_ctl
                    .findings
                    .iter()
                    .rev()
                    .take(8)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            max_hops: 8,
            deadline_secs: seconds,
            token_budget: tokens as u64,
            verify_command: self.loop_ctl.accept_cmd.clone(),
            policy_note: None,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_cancel = Arc::clone(&cancel);
        // Reserve once, conservatively, rather than charging overlapping shared
        // provider counters as if they were exclusively this child's usage.
        self.loop_ctl.tokens_spent = self.loop_ctl.tokens_spent.saturating_add(tokens);
        self.loop_ctl.experiments.push(ExperimentRecord {
            key: key.clone(),
            hypothesis,
            iteration: self.loop_ctl.iteration,
            settled_iteration: None,
            artifact_dir: artifact_dir.clone(),
            status: "running".into(),
            reserved_tokens: tokens,
            summary: None,
            context_ref: None,
            context_is_supervisor_only: false,
        });
        self.loop_experiment = Some(ExperimentPending {
            run_id: self.loop_ctl.id.clone(),
            workspace,
            key,
            cancel,
            rx,
        });
        save(&self.loop_ctl);
        self.note(format!(
            "loop · deep experiment launched on {} · parent fast lane continues · artifacts {}",
            club.label(),
            artifact_dir.display()
        ));
        std::thread::spawn(move || {
            let result = run_loop_experiment(request, club, worker_cancel);
            let _ = tx.send(result);
        });
    }

    pub(crate) fn loop_cancel_experiment(&mut self) {
        if let Some(pending) = &self.loop_experiment {
            pending.cancel.store(true, Ordering::Release);
        }
    }

    pub(crate) fn loop_drain_experiment(&mut self) {
        if self.exit_request.is_some()
            || matches!(
                self.loop_ctl.status,
                LoopStatus::Idle
                    | LoopStatus::Paused
                    | LoopStatus::Stopped
                    | LoopStatus::Done
                    | LoopStatus::Failed
            )
            || budget_tripped(&self.loop_ctl).is_some()
        {
            self.loop_cancel_experiment();
        }
        let Some(pending) = self.loop_experiment.as_ref() else {
            // A power loss can leave a durable running record without a live
            // owner. Keep its evidence address and make recovery visible.
            if let Some(record) = self.loop_ctl.experiments.last_mut()
                && record.status == "running"
            {
                record.status = "interrupted".into();
                record.settled_iteration = Some(self.loop_ctl.iteration);
                record.context_is_supervisor_only = true;
                record.context_ref = None;
                record.summary = Some("Owner was interrupted; inspect retained working source and logs before choosing another experiment.".into());
                save(&self.loop_ctl);
            }
            return;
        };
        let result = match pending.rx.try_recv() {
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                Err("experiment worker disconnected; inspect retained artifacts".into())
            }
            Ok(result) => result,
        };
        let pending = self.loop_experiment.take().expect("checked owner");
        // Preserve artifacts on disk, but never import an old run's reply into
        // a replacement run or newly selected workspace.
        if pending.run_id != self.loop_ctl.id || pending.workspace != self.tools.current_workspace()
        {
            return;
        }
        let cancelled = pending.cancel.load(Ordering::Acquire);
        let (status, summary, producer, supervisor_only) = match result {
            Ok(result) => {
                let status = if cancelled {
                    "cancelled"
                } else if result.error.is_some() {
                    "failed"
                } else {
                    "returned"
                };
                let summary = if cancelled {
                    format!(
                        "deep experiment cancelled · artifacts {} · unfinished source and logs retained; cancelled reply discarded",
                        result.artifact_dir.display()
                    )
                } else {
                    format!(
                        "deep experiment {status}: {} · artifacts {} · {} · {}",
                        result.stop_reason,
                        result.artifact_dir.display(),
                        result.error.as_deref().unwrap_or(
                            "inspect candidate and evaluation receipts before integration"
                        ),
                        result.answer.chars().take(2400).collect::<String>()
                    )
                };
                let producer =
                    result
                        .result_sha256
                        .map(|result_sha256| crate::club::RecoveryProducerRef {
                            task_sha256: result.task_sha256,
                            source_snapshot_sha256: result.snapshot_sha256,
                            answer_sha256: crate::cut::sha256_hex(result.answer.as_bytes()),
                            result_sha256,
                            patch_sha256: result.patch_sha256,
                            rollout_id: result.rollout_id,
                            stop_reason: result.stop_reason,
                        });
                (status, summary, producer, !result.model_phase_entered)
            }
            Err(_) if cancelled => (
                "cancelled",
                "deep experiment cancelled; inspect retained artifacts".into(),
                None,
                true,
            ),
            Err(error) => ("failed", format!("deep experiment: {error}"), None, false),
        };
        let context_ref = (!cancelled && !supervisor_only).then(|| {
            recovery_reference(
                &pending.run_id,
                &pending.workspace,
                &pending.key,
                &summary,
                producer,
            )
        });
        if let Some(record) = self
            .loop_ctl
            .experiments
            .iter_mut()
            .find(|r| r.key == pending.key)
        {
            record.status = status.into();
            record.settled_iteration = Some(self.loop_ctl.iteration);
            record.summary = Some(summary.clone());
            record.context_ref = context_ref.clone();
            record.context_is_supervisor_only = supervisor_only || cancelled;
        }
        // Cancellation has no model-facing completion. The durable record still
        // points to unfinished work so explicit resume can inspect it.
        if !cancelled {
            self.loop_ctl.pending_proc_completions.push(summary.clone());
            if let Some(reference) = context_ref {
                self.loop_ctl.pending_recovery_contexts.push(reference);
            }
        }
        save(&self.loop_ctl);
        self.note(summary);
    }

    pub(crate) fn loop_recovery_context_refs(&self) -> Vec<crate::club::RecoveryContextRef> {
        let mut refs = Vec::new();
        if !self.loop_ctl.pending_proc_completions.is_empty() {
            for reference in &self.loop_ctl.pending_recovery_contexts {
                let mut reference = reference.clone();
                if !self
                    .loop_ctl
                    .pending_proc_completions
                    .iter()
                    .any(|summary| {
                        crate::cut::sha256_hex(summary.as_bytes()) == reference.summary_sha256
                    })
                {
                    // Lost/mutated queue pairing cannot prove the retained
                    // context clean. Keep the origin unknown, without reading
                    // an artifact or interpreting any model prose.
                    reference.producer = None;
                }
                refs.push(reference);
            }
        }
        // Old state may retain A's queued summary after B becomes latest,
        // without the newly introduced typed queue companion. Join only exact
        // durable recovery records, never arbitrary process text.
        for record in &self.loop_ctl.experiments {
            if record.status == "cancelled" || record.context_is_supervisor_only {
                continue;
            }
            if let Some(summary) = &record.summary
                && self
                    .loop_ctl
                    .pending_proc_completions
                    .iter()
                    .any(|queued| queued == summary)
            {
                let id = recovery_reference(
                    &self.loop_ctl.id,
                    self.tools.current_workspace(),
                    &record.key,
                    summary,
                    None,
                );
                if !refs
                    .iter()
                    .any(|reference| reference.import_id == id.import_id)
                {
                    refs.push(id);
                }
            }
        }
        if let Some(record) = self.loop_ctl.experiments.last()
            && record.status != "cancelled"
            && !record.context_is_supervisor_only
            && let Some(summary) = &record.summary
        {
            // Legacy/mismatched metadata remains useful unknown context. It
            // cannot silently become a clean parent turn or an audited link.
            let reference = record
                .context_ref
                .as_ref()
                .filter(|reference| {
                    reference.summary_sha256 == crate::cut::sha256_hex(summary.as_bytes())
                        && reference.loop_run_id == self.loop_ctl.id
                        && reference.experiment_key_sha256
                            == crate::cut::sha256_hex(record.key.as_bytes())
                        && reference.workspace_sha256
                            == recovery_workspace_sha256(self.tools.current_workspace())
                })
                .cloned()
                .unwrap_or_else(|| {
                    recovery_reference(
                        &self.loop_ctl.id,
                        self.tools.current_workspace(),
                        &record.key,
                        summary,
                        None,
                    )
                });
            refs.push(reference);
        }
        let mut message = ChatMsg::harness("");
        message.recovery_context = refs;
        crate::club::recovery_context_refs(&[message])
    }

    pub(crate) fn loop_experiment_context(&self, prompt: &mut String) {
        if let Some(record) = self.loop_ctl.experiments.last() {
            let summary = if record.status == "cancelled" {
                "Cancelled reply discarded; inspect retained source and logs before resuming the hypothesis."
            } else {
                record.summary.as_deref().unwrap_or(
                    "An isolated worker owns this hypothesis; preserve its unfinished work.",
                )
            };
            prompt.push_str(&format!(
                "\n\n[DEEP EXPERIMENT — {}] Hypothesis: {}. Artifacts: {}. {} Keep the fast lane moving with distinct validated candidates. Do not duplicate or replace the active experiment. On return, inspect its patch and actual evaluation receipts; integrate only after checking against the current parent workspace. A child result does not establish acceptance or a percentage improvement.",
                record.status, record.hypothesis, record.artifact_dir.display(),
                summary
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_fast_lane_still_admits_a_deep_experiment() {
        let mut state = LoopState {
            podrace: true,
            iteration: 3,
            measured_candidates: 2,
            submissions: 1,
            stale_count: 0,
            stall_stop: 4,
            ..Default::default()
        };
        assert!(experiment_due(&state));
        state.podrace = false;
        assert!(!experiment_due(&state));
        state.stale_count = 4;
        assert!(experiment_due(&state));
    }

    #[test]
    fn experiment_reservation_preserves_parent_allowance_and_deadline() {
        let mut state = LoopState {
            token_budget: 40_000,
            tokens_spent: 8_000,
            started_ms: now_ms(),
            deadline_secs: 90,
            ..Default::default()
        };
        assert_eq!(allowance(&state), Some((8_000, 90)));
        state.tokens_spent = 39_000;
        assert_eq!(allowance(&state), None);
        state.token_budget = 0;
        state.deadline_secs = 0;
        assert_eq!(
            allowance(&state),
            Some((EXPERIMENT_TOKENS, EXPERIMENT_SECONDS))
        );
        state.max_iters = 3;
        state.iteration = 3;
        assert_eq!(allowance(&state), None);
    }

    #[test]
    fn dropping_owner_requests_cancellation() {
        let cancel = Arc::new(AtomicBool::new(false));
        let (_tx, rx) = std::sync::mpsc::channel();
        let owner = ExperimentPending {
            run_id: "test".into(),
            workspace: PathBuf::new(),
            key: "test".into(),
            cancel: Arc::clone(&cancel),
            rx,
        };
        drop(owner);
        assert!(cancel.load(Ordering::Acquire));
    }
}
