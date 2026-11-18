//! A plain owned snapshot of the loop facts the Delve/realm code reads.
//!
//! The settlement ledger, the crawl and the adventure mirror used to borrow the
//! whole `LoopState`. They now take a [`LoopSummary`] built at the call boundary
//! by the cockpit's `LoopState::summary`, so none of them names `loop_ctl`. The
//! enums below mirror `loop_ctl`'s `LoopStatus` / `EscalationTier` variant for
//! variant; the cockpit's `From` impls (`drive::loop_summary`) are exhaustive,
//! so a new variant fails to compile there instead of drifting.

use crate::together_settlement::planning::Input;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopStatus {
    #[default]
    Idle,
    Baselining,
    Running,
    Verifying,
    AwaitingApproval,
    Paused,
    Done,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EscalationTier {
    #[default]
    Local,
    Swarm,
    Sota,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableEvents {
    pub loop_id: String,
    pub workspace: PathBuf,
    pub rows: Vec<(usize, bool)>,
    pub plans: BTreeMap<usize, Input>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoopSummary {
    pub status: LoopStatus,
    pub iteration: usize,
    pub tier: EscalationTier,
    pub submissions: usize,
    pub podrace: bool,
    pub pivot: usize,
    pub measured_candidates: usize,
    pub verifier_blocked: Option<String>,
    pub task: String,
    pub stale_count: usize,
    pub durable_settlement_events: Option<DurableEvents>,
    pub last_log_all_errors: bool,
    pub idle_log_tail: usize,
    pub promotions: usize,
    pub last_promotion_score: Option<String>,
    pub promotion_failures: usize,
    pub last_promotion_failure_score: Option<String>,
}
