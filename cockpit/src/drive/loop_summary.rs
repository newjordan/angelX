//! A plain owned snapshot of the loop facts the Delve/realm code reads.
//!
//! The settlement ledger, the crawl and the adventure mirror used to borrow the
//! whole [`LoopState`]. They now take a [`LoopSummary`] built at the call
//! boundary by [`LoopState::summary`], so none of them names `loop_ctl`. The
//! types live in the realm host (`angel_realm_host::loop_summary`), which
//! reads them; the enums mirror `loop_ctl`'s `LoopStatus` / `EscalationTier`
//! variant for variant, and the `From` impls here are exhaustive, so a new
//! variant fails to compile here instead of drifting.
use super::loop_ctl::{self, LoopState};
use crate::agent::harness::cartridges::fleet::SubmissionPhase;
use crate::agent::harness::cartridges::records::SubmissionStatus;

#[cfg(test)]
use super::together_settlement::planning::Input;
pub(crate) use angel_realm_host::loop_summary::{
    DurableEvents, EscalationTier, LoopStatus, LoopSummary,
};

impl From<loop_ctl::LoopStatus> for LoopStatus {
    fn from(status: loop_ctl::LoopStatus) -> Self {
        match status {
            loop_ctl::LoopStatus::Idle => Self::Idle,
            loop_ctl::LoopStatus::Baselining => Self::Baselining,
            loop_ctl::LoopStatus::Running => Self::Running,
            loop_ctl::LoopStatus::Verifying => Self::Verifying,
            loop_ctl::LoopStatus::AwaitingApproval => Self::AwaitingApproval,
            loop_ctl::LoopStatus::Paused => Self::Paused,
            loop_ctl::LoopStatus::Done => Self::Done,
            loop_ctl::LoopStatus::Stopped => Self::Stopped,
            loop_ctl::LoopStatus::Failed => Self::Failed,
        }
    }
}

impl From<loop_ctl::EscalationTier> for EscalationTier {
    fn from(tier: loop_ctl::EscalationTier) -> Self {
        match tier {
            loop_ctl::EscalationTier::Local => Self::Local,
            loop_ctl::EscalationTier::Swarm => Self::Swarm,
            loop_ctl::EscalationTier::Sota => Self::Sota,
        }
    }
}

impl LoopState {
    /// An owned snapshot of the fields the realm side reads.
    pub(crate) fn summary(&self) -> LoopSummary {
        let settled = || {
            self.submission_results
                .iter()
                .filter_map(|r| r.official.as_ref())
        };
        let promoted: Vec<_> = settled().filter(|o| o.promoted_improvement()).collect();
        let failed: Vec<_> = settled().filter(|o| promotion_failed(o)).collect();
        LoopSummary {
            status: self.status.into(),
            iteration: self.iteration,
            tier: self.tier.into(),
            submissions: self.submissions,
            podrace: self.podrace,
            pivot: self.pivot,
            measured_candidates: self.measured_candidates,
            verifier_blocked: self.verifier_blocked.clone(),
            task: self.task.clone(),
            stale_count: self.stale_count,
            durable_settlement_events: self.durable_settlement_events().map(|events| {
                DurableEvents {
                    loop_id: events.loop_id,
                    workspace: events.workspace,
                    rows: events.rows,
                    plans: events.plans,
                }
            }),
            last_log_all_errors: self.log.last().is_some_and(|entry| {
                entry.tool_errors > 0 && entry.tool_errors >= entry.tool_calls
            }),
            idle_log_tail: self
                .log
                .iter()
                .rev()
                .take_while(|entry| {
                    entry.tool_calls <= entry.tool_errors
                        && !entry.workspace_changed
                        && entry.new_findings == 0
                        && entry.outcome_progress == 0
                        && entry.novel_outcome_actions == 0
                        && entry.verified_outcome_actions == 0
                })
                .count(),
            promotions: promoted.len(),
            last_promotion_score: promoted.last().and_then(|o| o.official_score.clone()),
            promotion_failures: failed.len(),
            last_promotion_failure_score: failed.last().and_then(|o| o.official_score.clone()),
        }
    }
}

/// Whether a settled submission is a promotion that didn't happen: rejected
/// or timed out, or accepted and plainly no record. A result still waiting
/// on its promotion isn't one.
fn promotion_failed(o: &SubmissionStatus) -> bool {
    match SubmissionPhase::from_status(&o.status) {
        SubmissionPhase::Rejected | SubmissionPhase::TimedOut => true,
        SubmissionPhase::Accepted => {
            !o.promoted_improvement()
                && (o.improved == Some(false)
                    || o.promotion_status
                        .as_deref()
                        .is_some_and(|s| s != "promoted" && s != "unavailable"))
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/loop_ctl/loop_summary__tests.rs"]
mod tests;
