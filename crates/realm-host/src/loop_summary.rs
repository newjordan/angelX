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

/// Mirror of `loop_ctl::LoopStatus`.
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

/// Mirror of `loop_ctl::EscalationTier`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EscalationTier {
    #[default]
    Local,
    Swarm,
    Sota,
}

/// The confirmed checkpoint projection settlement credits are paid from
/// (mirror of `loop_ctl::DurableLoopEvents`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableEvents {
    pub loop_id: String,
    pub workspace: PathBuf,
    pub rows: Vec<(usize, bool)>,
    pub plans: BTreeMap<usize, Input>,
}

/// What the realm side reads of a loop run. Direct copies first, then the few
/// values `summary()` derives from the log and the settled submissions so the
/// reader never needs the log or the submission records themselves.
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
    /// `LoopState::durable_settlement_events()` at snapshot time.
    pub durable_settlement_events: Option<DurableEvents>,
    /// The newest log row is a turn whose every tool call failed.
    pub last_log_all_errors: bool,
    /// Log rows, newest first, that observed no tool success, no workspace
    /// change and no finding or outcome progress, up to the first row that did.
    pub idle_log_tail: usize,
    /// Settled submissions that were promoted records.
    pub promotions: usize,
    /// The newest promoted record's official score.
    pub last_promotion_score: Option<String>,
    /// Settled submissions whose promotion did not come.
    pub promotion_failures: usize,
    /// The newest failed promotion's official score.
    pub last_promotion_failure_score: Option<String>,
}
