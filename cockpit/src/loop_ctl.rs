//! The `/loop` controller: drive the full weight of the engine at a goal.
//!
//! Where [`deli`](crate::deli) runs a bounded iteration loop *inside one turn*,
//! the loop controller drives **successive full agentic turns** toward a goal/task
//! until a stop condition — re-armed across UI frames from
//! [`App::advance`](crate::App::advance). Every iteration is a normal
//! [`run_turn`](crate::harness::run_turn), so it inherits all the per-turn
//! guardrails (anti-spin, error breaker, no-progress, turn deadline, compaction)
//! for free, and can already reach the **local agent network** (delegate /
//! swarm-test tools) within the turn.
//!
//! What the controller adds on top — the "more robust than a default codex loop"
//! layer:
//! - **Operator-selected caps** (unbounded by default): max iterations,
//!   cumulative (estimated) tokens, and wall-clock deadline.
//! - **Stall → concrete next action**: reuses deli's dedup + pivot
//!   ([`crate::iterate`]) while keeping the selected model, effort, and tier.
//!   Stalls and missing measurement/submission receipts steer another attempt;
//!   they do not pause the campaign or automatically buy a swarm/SOTA route.
//!   `/loop sota` remains an explicit operator-approved mode switch.
//! - **Deli method built in**: `/loop deli` drives each iteration through the
//!   [`DeliClub`] deep-think-over-time method, so a loop iteration is itself a
//!   bounded research loop — no `ANGEL_DRIVER=deli` needed.
//! - **Hybrid done-detection (LEVI / RLVR)**: the model may *propose* done
//!   (`LOOP_DONE`), but when the goal binds a verifiable command
//!   ([`crate::goal::Goal::accept_cmd`]) that command is run off-thread and scored
//!   through the [`reinforce`](crate::reinforce) verifiable reward
//!   ([`CompositeReward::code_health`]) — a dense, ground-truth RLVR signal, the
//!   same substrate as angel's LEVI runs — not a bare exit code or the model's word.
//!   A failed gate is memoized by its exact predicate/configuration and a
//!   cryptographic Git workspace fingerprint, so a later unchanged done claim
//!   replays bounded red evidence without paying for the same process again.
//! - **Session-scoped and crash-resumable**: state is snapshotted atomically
//!   (mirroring [`session`](crate::session)); resuming that same cockpit session
//!   re-arms running/verifying loops under the existing task and budget gates.
//!   A second cockpit in the same workspace has an independent loop checkpoint
//!   and never inherits the first one's authority. Explicit pauses/stops remain
//!   parked; interrupted approval requests require operator action.
//!
//! The single-flight invariant is honored structurally: the controller only spawns
//! the next iteration when no turn / bg-job / pending I/O is in flight — the same
//! gate `submit` uses — so it can never double-spawn.
//!
//! Tunables (env, read at `start`): `ANGEL_LOOP_MAX_ITERS` (0 = no iteration cap),
//! `ANGEL_LOOP_DEADLINE_SECS` (0 = no time cap), `ANGEL_LOOP_TOKEN_BUDGET` (0 = no token cap),
//! `ANGEL_LOOP_STALL_STOP` (4), `ANGEL_LOOP_PIVOT` (2),
//! `ANGEL_LOOP_FIRST_CANDIDATE_ITERS` (3, podrace: steer measurement after this
//! many iterations without a verified measured candidate; 0 = off). Persisted to
//! `~/.angel0/loops/<workspace-key>--<session-key>.json` (override
//! `ANGEL_LOOP_FILE`).
//! `/loop podrace <task>` is the explicit competition profile: five days, no
//! iteration/token cap, outcome-action progress, and unattended stall recovery.

use crate::approval::Decision;
use crate::club::{ChatMsg, Club, is_sota_label, model_smartness};
use crate::deli::DeliClub;
use crate::iterate::{EvidenceRegime, WORKER_SYS, curated_prompt, finding_claim, normalize};
use crate::loop_dialog::{LoopDialogAction, LoopLaunchDialog, LoopLaunchSettings};
use crate::reinforce::{EvaluatorEvidence, Reward, RewardInput, TestReward};
use crate::swarm::SwarmClub;
use crate::toolstrip::ToolStripSnapshot;
use crate::transcript::{Message, Role};
use crate::turn::Thinking;
use crate::workspace_store::{angel_dir, angel_subdir, workspace_json_path_in};
use ratatui::{crossterm::event::KeyCode, layout::Rect};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

mod evidence;
mod recovery;
#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/recovery_tests.rs"]
mod recovery_tests;
use evidence::{
    apply_reply_with_tools, budget_tripped, clip_diff_stat, est_tokens, observe_workspace_change,
    register_costly_actions, register_outcome_actions, register_verified_outcome_actions,
    says_done, stall_limit_reached,
};
pub(crate) use recovery::ExperimentPending;

const DEFAULT_LOOP_MAX_ITERS: usize = 0;
const DEFAULT_LOOP_DEADLINE_SECS: u64 = 0;
const DEFAULT_LOOP_TOKEN_BUDGET: usize = 0;
const DEFAULT_LOOP_STALL_STOP: usize = 4;
const DEFAULT_LOOP_PIVOT: usize = 2;
const DEFAULT_LOOP_FIRST_CANDIDATE_ITERS: usize = 3;
/// Podrace: iterations after the first measured candidate before a missing
/// submission is overdue; twice this reinforces the next-action directive.
const DEFAULT_LOOP_FIRST_SUBMISSION_ITERS: usize = 4;
/// Most recent findings / directions injected into an iteration prompt. The
/// full lists persist and the dedup ledger spans the whole run; only the
/// prompt is windowed, so an endless run's per-iteration cost stays flat
/// instead of growing O(n) (cumulative O(n²) tokens).
const LOOP_PROMPT_FINDINGS: usize = 40;
const LOOP_PROMPT_DIRECTIONS: usize = 24;
const LOOP_PROMPT_HYPOTHESES: usize = 24;
const LOOP_EVIDENCE_REVIEW_INTERVAL: usize = 15;
const ACCEPTANCE_RED_MEMO_MAX_ATTEMPTS: u16 = 1_024;
const ACCEPTANCE_RED_MEMO_SUMMARY_CHARS: usize = 640;
const ACCEPTANCE_RED_MEMO_DETAIL_CHARS: usize = 3_200;

/// Lifecycle of the loop. Only `Running` arms new iterations; `Verifying` and
/// `AwaitingApproval` keep the loop "active" (polled) but don't arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoopStatus {
    #[default]
    Idle,
    /// Capturing the pre-edit baseline test count (reward-hack guard) before the
    /// first iteration runs.
    Baselining,
    Running,
    /// Running the goal's acceptance command off-thread.
    Verifying,
    /// Waiting on the human to approve a SOTA escalation.
    AwaitingApproval,
    Paused,
    Done,
    Stopped,
    Failed,
}

/// The operator-selected route; stalls do not change the tier:
/// `Local` → `Swarm` (auto) → `Sota` (by approval). The full weight of the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EscalationTier {
    /// Local fleet club (in-hand when it is already local; otherwise the
    /// smartest reachable non-SOTA seat). Can still delegate / swarm-test
    /// within the turn.
    #[default]
    Local,
    /// Local Mixture-of-Agents fan-out ([`SwarmClub`]) — more breadth, no approval.
    Swarm,
    /// A SOTA model, brought in only after explicit approval.
    Sota,
}

/// One iteration's summary (mirrors deli's `IterLog`, plus a timestamp).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopIterLog {
    pub iteration: usize,
    pub direction: String,
    /// Evidence-backed findings admitted as progress this iteration.
    pub new_findings: usize,
    #[serde(default)]
    pub reported_findings: usize,
    #[serde(default)]
    pub unverified_findings: usize,
    #[serde(default)]
    pub tool_calls: usize,
    #[serde(default)]
    pub tool_errors: usize,
    #[serde(default)]
    pub duplicate_costly_actions: usize,
    /// Successful candidate/validation/submission/score actions observed this
    /// iteration. Novel receipts contribute to stall reset in every loop mode;
    /// podrace still excludes prose-only novelty from competition progress.
    #[serde(default)]
    pub outcome_progress: usize,
    /// Successful outcome fingerprints not observed on an earlier iteration.
    /// Repeated status polling must not keep an otherwise stalled loop alive.
    #[serde(default)]
    pub novel_outcome_actions: usize,
    /// Verified measurement/submission receipts observed this iteration.
    /// In podrace these are the only receipts that count as progress.
    #[serde(default)]
    pub verified_outcome_actions: usize,
    /// The Git-backed workspace fingerprint changed during this iteration.
    #[serde(default)]
    pub workspace_changed: bool,
    #[serde(default)]
    pub evidence_review: bool,
    pub stale_count: usize,
    pub ts_ms: u64,
}

/// A trustworthy failed acceptance receipt that may be replayed while both the
/// exact predicate and the Git-backed workspace evidence remain unchanged.
/// Optional/defaulted so pre-memo loop state loads without a schema migration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcceptanceRedMemo {
    pub command: String,
    pub config_identity: String,
    pub workspace_fingerprint: String,
    pub summary: String,
    pub detail: String,
    /// Real executions and unchanged-red replays for this exact key. Bounded so
    /// a corrupt/endless persisted loop cannot overflow accounting.
    pub attempts: u16,
}

/// Runtime identity captured immediately before a real acceptance process is
/// launched. The post-run fingerprint must still match before a failure may be
/// memoized, preventing a mutating gate or concurrent edit from poisoning the
/// cache.
#[derive(Clone, Debug)]
pub struct PendingAcceptanceRun {
    pub command: String,
    pub config_identity: String,
    pub workspace_fingerprint: Option<String>,
}

/// Binary identity bound onto loop state (run-identity fields).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LoopBinaryIdentity {
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub source_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_info: Option<String>,
}

/// Paid vs cached token split accumulated from the turn ledger / estimates.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LoopTokenSplit {
    #[serde(default)]
    pub paid_input: u64,
    #[serde(default)]
    pub cached_input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoopVerifierId {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub sha256: String,
}

/// One locally measured candidate, sourced from verified "measured:" receipts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MeasuredCandidateRow {
    #[serde(default)]
    pub iteration: usize,
    #[serde(default)]
    pub utc: String,
    #[serde(default)]
    pub candidate_sha: String,
    #[serde(default)]
    pub base_sha: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
    #[serde(default)]
    pub verifier: LoopVerifierId,
    #[serde(default)]
    pub binary: LoopBinaryIdentity,
}

/// One `hilbert|yukon submit` journaled by submit_identity after the tool ran.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubmissionLogRow {
    #[serde(default)]
    pub iteration: usize,
    #[serde(default)]
    pub utc: String,
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub commit_or_patch_sha: String,
    #[serde(default)]
    pub note_file_sha256: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub harness: String,
    #[serde(default)]
    pub exit_code: i32,
    #[serde(default)]
    pub platform_response_excerpt: String,
    #[serde(default)]
    pub outcome: String,
}

/// The persisted state of a loop run.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LoopState {
    pub id: String,
    /// Cockpit session that owns this run. Defaulted only for legacy snapshots;
    /// production starts always bind it before the first save so concurrent
    /// shells in one workspace cannot share autonomous-loop authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_session_id: Option<String>,
    /// Workspace this loop belongs to. Loop state is persisted per workspace so
    /// a paused run from one folder does not appear in a fresh folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<PathBuf>,
    /// The task to pursue; empty => drive the active [`Goal`](crate::goal::Goal).
    pub task: String,
    pub status: LoopStatus,
    pub tier: EscalationTier,
    pub iteration: usize,
    // --- budgets (0 = off) ---
    pub max_iters: usize,
    pub deadline_secs: u64,
    pub token_budget: usize,
    /// True only for snapshots written with unbounded defaults.
    #[serde(default)]
    pub operator_caps: bool,
    #[serde(default)]
    pub paused_for_cap: bool,
    pub tokens_spent: usize,
    pub started_ms: u64,
    /// Runtime-only start of the currently executing loop cycle. This is the timer
    /// shown in UI surfaces; `started_ms` remains the whole-run deadline anchor.
    #[serde(skip)]
    pub cycle_started_ms: Option<u64>,

    // --- cadence / stall ---
    pub interval_secs: u64,
    pub pivot: usize,
    pub stall_stop: usize,
    pub stale_count: usize,
    pub min_findings: usize,

    // --- escalation ---
    pub sota_declined: bool,
    /// Deli mode: drive each iteration through the deli deep-think-over-time
    /// method ([`DeliClub`]) instead of a single agentic turn.
    pub deli: bool,
    /// Five-day unattended competition profile. Stall recovery stays live, but
    /// SOTA escalation (Codex / ChatGPT OAuth / other metered seats) still
    /// requires an explicit operator approval — it is a mode switch, not a
    /// silent failover.
    #[serde(default)]
    pub podrace: bool,

    // --- measured-candidate contract (competition loops) ---
    /// Podrace: steer measurement after this many registered iterations without a
    /// verified measured candidate (`0` = off). Read at loop start from
    /// `ANGEL_LOOP_FIRST_CANDIDATE_ITERS`.
    #[serde(default)]
    pub first_candidate_iters: usize,
    /// Novel verified benchmark/measure/verify/validate receipts this run.
    /// Count kept for older readers; enumerable rows live on the log.
    #[serde(default)]
    pub measured_candidates: usize,
    /// Duplicate of the count for readers that expect `_n`.
    #[serde(default)]
    pub measured_candidates_n: usize,
    /// Per-candidate identities (sha, score, verifier, binary).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measured_candidates_log: Vec<MeasuredCandidateRow>,
    /// Podrace: after the first measured candidate, this many further
    /// iterations without a submission receipt make the submission overdue
    /// (the directive rides first in the prompt); twice this many reinforces
    /// the directive (`0` = off). Read at loop start from
    /// `ANGEL_LOOP_FIRST_SUBMISSION_ITERS`. This steers measured candidates
    /// toward submission while the configured campaign budget remains active.
    #[serde(default)]
    pub first_submission_iters: usize,
    /// Iteration that registered the first measured candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_measured_iteration: Option<usize>,
    /// Novel verified submission receipts this run.
    #[serde(default)]
    pub submissions: usize,
    /// Every stamped `hilbert|yukon submit --model grok-4.6 --harness angel0` (accepted, refused, or rejected).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submissions_log: Vec<SubmissionLogRow>,
    /// angel0 binary identity at loop start / last change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<LoopBinaryIdentity>,
    /// Paid/cached/output token split alongside `tokens_spent`.
    #[serde(default)]
    pub tokens: LoopTokenSplit,
    /// The verification path is broken: a benchmark/verify/validate/submit
    /// action failed (bounded error tail) or the checkpoint declared
    /// `blocked`, and no verified receipt has landed since. Execution evidence
    /// is injected once per digest, then escalated after repeated blockage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier_blocked: Option<String>,
    #[serde(default)]
    pub verifier_failure: Option<crate::toolstrip::VerifierFailure>,
    #[serde(default)]
    pub blocked_repeat_count: usize,
    #[serde(default)]
    pub blocked_first_iteration: usize,
    #[serde(default)]
    pub blocked_prompt_digest: Option<String>,
    /// Durable loop-level ledger; the harness turn ledger lives on another thread.
    #[serde(default)]
    pub escalations: Vec<serde_json::Value>,

    // --- verifiable done-detection (pinned at start; reward-hack resistant) ---
    /// The goal's acceptance command, snapshotted at loop start so it can't change
    /// mid-run (a pinned, immutable predicate).
    pub accept_cmd: Option<String>,
    /// Pre-edit baseline of passing tests; a later "green" with fewer passes is a
    /// regression (tests deleted/disabled to fake a pass) and is rejected.
    pub baseline_passed: Option<usize>,
    /// Interrupted captures retain the status to restore after the baseline.
    /// Legacy captures lack this intent and conservatively restore Paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_resume_to: Option<LoopStatus>,
    /// Most recent red acceptance receipt. Reused only when the exact command,
    /// verifier configuration, and cryptographic workspace fingerprint match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failed_acceptance: Option<AcceptanceRedMemo>,

    // --- self-modification (`/self`; see self_loop.rs + docs/SELF_MODEL.md) ---
    /// This run edits the cockpit's **own crate** in an isolated git worktree.
    /// Done-detection runs the Layer-2 gate (build + tests + regression guard)
    /// instead of a shell acceptance command, and a green gate asks the operator
    /// to approve integration rather than finishing silently.
    #[serde(default)]
    pub self_edit: bool,
    /// The worktree branch holding the candidate self-edit (kept for integration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_branch: Option<String>,
    /// The live crate root the candidate merges back into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_root: Option<PathBuf>,
    /// The workspace the tools were rooted at before `/self` re-rooted them into
    /// the worktree — restored on integrate/discard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_prev_workspace: Option<PathBuf>,

    // --- operator steering (typed mid-run; see steer.rs) ---
    /// User notes queued while the loop was live. Each rides every subsequent
    /// iteration prompt: iterations run on fresh curated context, so a
    /// one-shot injection into the in-flight turn would be forgotten a cycle
    /// later.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steer_notes: Vec<String>,

    // --- accumulated ---
    /// Evidence-backed facts only. Model-proposed but unverified claims live in
    /// `hypotheses` and do not reset the stall counter.
    pub findings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hypotheses: Vec<String>,
    pub directions_tried: Vec<String>,
    pub log: Vec<LoopIterLog>,
    #[serde(default)]
    pub tool_calls_total: usize,
    #[serde(default)]
    pub tool_errors_total: usize,
    /// Canonical costly-action keys retained across iterations. This catches an
    /// identical benchmark/submit disguised by a new output filename.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub costly_actions_seen: Vec<String>,
    /// Successful mutation/verification/submission/status fingerprints already
    /// credited as loop progress. Persisted so a restart cannot replay the same
    /// receipt to erase a stall.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcome_actions_seen: Vec<String>,
    /// Last observed Git-backed workspace state. This is the primary progress
    /// authority for ordinary builder loops: real edits count even when the
    /// model fails to narrate them as a newly cited prose finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_workspace_fingerprint: Option<u64>,
    pub last_error: Option<String>,
    /// A failed turn awaits paced retry. Unlike historical `last_error`, this
    /// clears after a completed reply or explicit resume and survives restart.
    #[serde(default)]
    pub retry_after_error: bool,
    /// An execution prerequisite failed. Persists across restarts and cannot
    /// be cleared by research novelty, workspace changes, or automatic tiers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_blocker: Option<String>,
    /// The most recent setback the NEXT iteration must be told about — a failed
    /// acceptance check (with its actionable failure lines) or an iteration
    /// error. Iterations run on fresh curated context, so without this the next
    /// iteration re-walks the same path with no idea the last attempt failed or
    /// why. Injected into the next prompt, cleared when that reply is harvested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_setback: Option<String>,
    /// Literal background child outcomes waiting for the next authorized cycle.
    /// These are process receipts, never credited as findings or acceptance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_proc_completions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) pending_recovery_contexts: Vec<crate::club::RecoveryContextRef>,
    /// Durable recovery attempts, including interrupted artifact locations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub experiments: Vec<recovery::ExperimentRecord>,
    /// `HEAD` of the workspace when the run started: the anchor for the
    /// files-changed diff each iteration rides (commits mid-run stay visible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_rev: Option<String>,
    pub updated_ms: u64,

    // --- runtime-only (never serialized) ---
    #[serde(skip)]
    pub seen: HashSet<String>,
    #[serde(skip)]
    pub seen_hypotheses: HashSet<String>,
    /// Rows of `findings` / `log` already mirrored to the watchdog state dir
    /// this process-lifetime — the mirror appends the tail instead of
    /// rewriting O(n) content on every transition (O(n²) disk over an endless
    /// run). 0 (fresh start or resume) rewrites the file from scratch.
    #[serde(skip)]
    pub persisted_findings: std::cell::Cell<usize>,
    #[serde(skip)]
    pub persisted_log: std::cell::Cell<usize>,
    #[serde(skip)]
    pub wake_at: Option<Instant>,
    #[serde(skip)]
    pub awaiting_turn: bool,
    /// `Some` only for the ordinary acceptance process currently in flight;
    /// self-edit gates use their separate fixed predicate and never populate it.
    #[serde(skip)]
    pub pending_acceptance: Option<PendingAcceptanceRun>,
}

impl LoopState {
    fn observe_verifier_failure(&mut self, tools: &ToolStripSnapshot) {
        if !self.podrace {
            return;
        }
        if !tools.verified_outcome_actions.is_empty() {
            self.verifier_blocked = None;
            self.verifier_failure = None;
            self.blocked_repeat_count = 0;
            self.blocked_prompt_digest = None;
            return;
        }
        if let Some(failure) = tools.verifier_failure_details.last()
            && self.verifier_failure.as_ref().map(|f| &f.failure_digest)
                != Some(&failure.failure_digest)
        {
            self.blocked_repeat_count = 0;
            self.blocked_prompt_digest = None;
            // Keep the first diagnostic for the digest: whitespace-only
            // tail changes must still update the same transcript row.
            self.verifier_failure = Some(failure.clone());
        }
        if let Some(failure) = &self.verifier_failure {
            if self.blocked_repeat_count == 0 {
                self.blocked_first_iteration = self.iteration;
            }
            self.blocked_repeat_count = self.blocked_repeat_count.saturating_add(1);
            self.verifier_blocked = Some(failure.diagnostic());
        }
    }

    pub(crate) fn clear_caps(&mut self) {
        self.max_iters = 0;
        self.deadline_secs = 0;
        self.token_budget = 0;
        self.operator_caps = true;
    }

    pub(crate) fn cap_summary(&self) -> String {
        let mut caps = Vec::new();
        if self.max_iters > 0 {
            caps.push(format!("iterations {}", self.max_iters));
        }
        if self.deadline_secs > 0 {
            caps.push(format!("deadline {}s", self.deadline_secs));
        }
        if self.token_budget > 0 {
            caps.push(format!("tokens ~{}", self.token_budget));
        }
        if caps.is_empty() {
            "no cap".into()
        } else {
            format!("operator caps: {}", caps.join(" · "))
        }
    }

    /// A fresh run with budgets read from the environment.
    pub fn configured_from_env() -> Self {
        Self {
            id: gen_id(),
            operator_caps: true,
            max_iters: env_usize("ANGEL_LOOP_MAX_ITERS", DEFAULT_LOOP_MAX_ITERS),
            deadline_secs: env_u64("ANGEL_LOOP_DEADLINE_SECS", DEFAULT_LOOP_DEADLINE_SECS),
            token_budget: env_usize("ANGEL_LOOP_TOKEN_BUDGET", DEFAULT_LOOP_TOKEN_BUDGET),
            pivot: env_usize("ANGEL_LOOP_PIVOT", DEFAULT_LOOP_PIVOT),
            stall_stop: env_usize("ANGEL_LOOP_STALL_STOP", DEFAULT_LOOP_STALL_STOP),
            first_candidate_iters: env_usize(
                "ANGEL_LOOP_FIRST_CANDIDATE_ITERS",
                DEFAULT_LOOP_FIRST_CANDIDATE_ITERS,
            ),
            first_submission_iters: env_usize(
                "ANGEL_LOOP_FIRST_SUBMISSION_ITERS",
                DEFAULT_LOOP_FIRST_SUBMISSION_ITERS,
            ),
            ..Default::default()
        }
    }
}

/// Off-thread work the controller is waiting on (held on `App`, not serialized).
pub(crate) enum LoopPending {
    /// Baseline capture: passing-test count for the regression guard. The
    /// status is what the run resumes to once the count lands — `Running` at
    /// loop start, the prior status on a mid-run `/goal cmd` re-pin (a re-pin
    /// while Paused must not un-pause the run as a side effect).
    Baseline(Receiver<usize>, LoopStatus),
    /// The goal's acceptance command, running on a worker thread.
    Verify(Receiver<VerifyResult>),
    /// Retired command owners: discard results, retain the slot until settlement.
    RetiredBaseline(Receiver<usize>),
    RetiredVerify(Receiver<VerifyResult>),
    /// A SOTA-escalation approval, answered via the standard approval modal.
    Approval(Receiver<Decision>),
    /// An integration approval for a gate-green self-edit (see `self_loop.rs`):
    /// approve merges the worktree branch into the live tree, deny keeps it.
    SelfIntegrate(Receiver<Decision>),
}

/// Result of running the goal's acceptance command.
pub(crate) struct VerifyResult {
    pub passed: bool,
    pub summary: String,
    /// Bounded, actionable failure lines (failing test names, panics, compiler
    /// errors) for the next iteration's prompt — empty on a pass.
    pub detail: String,
}

enum AcceptanceGatePlan {
    Execute(PendingAcceptanceRun),
    Replay(VerifyResult),
}

fn acceptance_config_identity(baseline_passed: Option<usize>) -> String {
    format!(
        "{}|source={}|{}|threshold={:08x}|baseline={}",
        env!("CARGO_PKG_VERSION"),
        crate::harness::run_identity::source_sha256(),
        crate::reinforce::TEST_VERIFIER_CONTRACT,
        verify_threshold().to_bits(),
        baseline_passed
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
    )
}

fn acceptance_workspace_fingerprint(workspace: &Path) -> Option<String> {
    crate::harness::workspace_evidence_sha256(workspace)
        .filter(|fingerprint| fingerprint.len() == 64)
        .filter(|fingerprint| fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn clip_acceptance_receipt(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let mut clipped = text.chars().take(keep).collect::<String>();
    clipped.push('…');
    clipped
}

fn acceptance_memo_matches(
    memo: &AcceptanceRedMemo,
    command: &str,
    config_identity: &str,
    workspace_fingerprint: Option<&str>,
) -> bool {
    workspace_fingerprint.is_some_and(|fingerprint| {
        memo.command == command
            && memo.config_identity == config_identity
            && memo.workspace_fingerprint == fingerprint
    })
}

/// Decide whether an ordinary acceptance check must execute. `None` from the
/// cryptographic Git fingerprint is deliberately never reusable: ambiguity is
/// safer and cheaper than treating an untracked external state as unchanged.
fn prepare_acceptance_gate(
    state: &mut LoopState,
    command: &str,
    workspace: &Path,
) -> AcceptanceGatePlan {
    let config_identity = acceptance_config_identity(state.baseline_passed);
    let workspace_fingerprint = acceptance_workspace_fingerprint(workspace);
    if let Some(memo) = state.last_failed_acceptance.as_mut()
        && acceptance_memo_matches(
            memo,
            command,
            &config_identity,
            workspace_fingerprint.as_deref(),
        )
    {
        memo.attempts = memo
            .attempts
            .saturating_add(1)
            .min(ACCEPTANCE_RED_MEMO_MAX_ATTEMPTS);
        let summary = clip_acceptance_receipt(
            &format!(
                "unchanged red replay attempt {} (process skipped) · {}",
                memo.attempts, memo.summary
            ),
            ACCEPTANCE_RED_MEMO_SUMMARY_CHARS,
        );
        return AcceptanceGatePlan::Replay(VerifyResult {
            passed: false,
            summary,
            detail: memo.detail.clone(),
        });
    }

    // A changed command/config/workspace or an ambiguous fingerprint invalidates
    // the old receipt before the real process starts. If the process dies, the
    // old causal result must not silently return.
    state.last_failed_acceptance = None;
    AcceptanceGatePlan::Execute(PendingAcceptanceRun {
        command: command.to_string(),
        config_identity,
        workspace_fingerprint,
    })
}

/// Admit a failed result only when the command/config still match and the
/// workspace has the same trustworthy fingerprint before and after execution.
fn record_acceptance_result(
    state: &mut LoopState,
    run: PendingAcceptanceRun,
    result: &VerifyResult,
    workspace: &Path,
) {
    state.last_failed_acceptance = None;
    if result.passed
        || state.accept_cmd.as_deref() != Some(run.command.as_str())
        || acceptance_config_identity(state.baseline_passed) != run.config_identity
    {
        return;
    }
    let Some(before) = run.workspace_fingerprint else {
        return;
    };
    let Some(after) = acceptance_workspace_fingerprint(workspace) else {
        return;
    };
    if before != after {
        return;
    }
    state.last_failed_acceptance = Some(AcceptanceRedMemo {
        command: run.command,
        config_identity: run.config_identity,
        workspace_fingerprint: after,
        summary: clip_acceptance_receipt(&result.summary, ACCEPTANCE_RED_MEMO_SUMMARY_CHARS),
        detail: clip_acceptance_receipt(&result.detail, ACCEPTANCE_RED_MEMO_DETAIL_CHARS),
        attempts: 1,
    });
}

// ---------------------------------------------------------------------------
// Controller (impl App — lives here so it sits beside the LoopState it drives)
// ---------------------------------------------------------------------------

impl crate::App {
    fn loop_account_rl(&mut self) {
        let used = self.tools.rl().take_loop_tokens_for(&self.loop_ctl.id);
        self.loop_ctl.tokens_spent = self.loop_ctl.tokens_spent.saturating_add(used);
        self.loop_ctl.tokens.total = self.loop_ctl.tokens.total.saturating_add(used as u64);
    }

    fn loop_bind_rl(&mut self) -> Arc<dyn Club> {
        self.loop_account_rl();
        let club = self.loop_club();
        let deadline = (self.loop_ctl.deadline_secs > 0).then(|| {
            let elapsed = now_ms().saturating_sub(self.loop_ctl.started_ms);
            let remaining = self
                .loop_ctl
                .deadline_secs
                .saturating_mul(1000)
                .saturating_sub(elapsed);
            Instant::now()
                .checked_add(Duration::from_millis(remaining))
                .unwrap_or_else(Instant::now)
        });
        self.tools
            .rl()
            .bind_loop(crate::rl_ctl::LoopCampaignContext {
                loop_id: self.loop_ctl.id.clone(),
                task: self.loop_task_text(),
                verify: self.loop_ctl.accept_cmd.clone(),
                club: Arc::clone(&club),
                deadline,
                remaining_tokens: (self.loop_ctl.token_budget > 0).then(|| {
                    self.loop_ctl
                        .token_budget
                        .saturating_sub(self.loop_ctl.tokens_spent)
                }),
            });
        club
    }

    /// True while a loop should keep being polled (running, verifying, or waiting
    /// on approval). Drives the run-loop's tick cadence.
    pub(crate) fn loop_active(&self) -> bool {
        matches!(
            self.loop_ctl.status,
            LoopStatus::Baselining
                | LoopStatus::Running
                | LoopStatus::Verifying
                | LoopStatus::AwaitingApproval
        )
    }

    /// `/loop …` command surface. Subcommands are tokenized from the raw arg, so
    /// `input.rs` only needs the `Option<String>` shape (symmetric with `/goal`).
    pub(crate) fn loop_command(&mut self, arg: Option<String>) -> String {
        let raw = arg.unwrap_or_default();
        let raw = raw.trim();
        let (verb, rest) = match raw.split_once(char::is_whitespace) {
            Some((v, r)) => (v.to_ascii_lowercase(), r.trim().to_string()),
            None => (raw.to_ascii_lowercase(), String::new()),
        };
        match verb.as_str() {
            "" => self.loop_open_dialog(String::new(), 0, false),
            "status" => self.loop_status_text(),
            "stop" => {
                self.loop_cancel_inflight();
                self.loop_finish(LoopStatus::Stopped, "stopped by user");
                "loop stopped".to_string()
            }
            "pause" => {
                self.loop_cancel_inflight();
                self.loop_ctl.status = LoopStatus::Paused;
                self.loop_ctl.wake_at = None;
                self.loop_ctl.cycle_started_ms = None;
                save(&self.loop_ctl);
                self.start_lifecycle_ceremony(
                    crate::viz::lifecycle_viz::CeremonyKind::LoopPaused,
                    self.loop_task_text(),
                );
                "loop paused — /loop resume to continue".to_string()
            }
            "resume" | "continue" => self.loop_resume(),
            "restart" | "again" => self.loop_restart_dialog(rest, false),
            "clear" | "reset" => {
                self.loop_cancel_inflight();
                let workspace = self
                    .loop_ctl
                    .workspace
                    .clone()
                    .unwrap_or_else(|| self.tools.current_workspace().to_path_buf());
                let owner_session_id = self
                    .loop_ctl
                    .owner_session_id
                    .clone()
                    .unwrap_or_else(|| self.session.id.clone());
                self.loop_ctl = LoopState::default();
                clear_file_for(Some(&workspace), Some(&owner_session_id));
                "loop cleared".to_string()
            }
            "sota" => {
                if self.loop_pending.is_some() {
                    "busy — wait for existing verifier work to drain before /loop sota".to_string()
                } else if !self.loop_active() {
                    "no active loop — start one first".to_string()
                } else if self.loop_sota_available() {
                    self.loop_request_sota();
                    "requesting approval to escalate to a SOTA model…".to_string()
                } else {
                    "no SOTA club configured (need an openai/codex club in the bag)".to_string()
                }
            }
            "now" | "fire" => {
                let (iv, task) = split_interval(&rest);
                self.loop_start_immediate(task, iv, false, false)
            }
            "every" | "start" | "run" | "go" => {
                let (iv, task) = split_interval(&rest);
                let (verb_lead, sub_rest) = split_word(&task);
                if verb_lead == "now" || verb_lead == "fire" || verb_lead == "immediate" {
                    self.loop_start_immediate(sub_rest.to_string(), iv, false, false)
                } else {
                    self.loop_open_dialog(task, iv, false)
                }
            }
            // Deli deep-think-over-time as the iteration method.
            "deli" | "deepthink" => {
                let (iv, task) = split_interval(&rest);
                self.loop_open_dialog(task, iv, true)
            }
            "podrace" | "competition" | "compete" | "race" => {
                let (iv, task) = split_interval(&rest);
                let (verb_lead, sub_rest) = split_word(&task);
                if verb_lead == "now"
                    || verb_lead == "go"
                    || verb_lead == "fire"
                    || verb_lead == "immediate"
                {
                    self.loop_start_immediate(sub_rest.to_string(), iv, false, true)
                } else {
                    self.loop_open_dialog_profile(task, iv, false, true)
                }
            }
            "iters" | "max" => self.loop_set_knob(&rest, |st, n| st.max_iters = n, "max iters"),
            "endless" | "forever" | "infinite" => {
                if self.loop_ctl.status == LoopStatus::Idle {
                    self.loop_open_dialog(rest, 0, false)
                } else {
                    let resume = self.loop_ctl.status == LoopStatus::Paused
                        && (self.loop_ctl.paused_for_cap
                            || budget_tripped(&self.loop_ctl).is_some()
                            || self.loop_ctl.last_setback.as_deref().is_some_and(|s| {
                                s.starts_with("budget reached:")
                                    || s.starts_with("operator cap reached:")
                            }));
                    self.loop_ctl.clear_caps();
                    save(&self.loop_ctl);
                    if resume {
                        format!("all operator caps cleared · {}", self.loop_resume())
                    } else {
                        "all operator caps cleared · no cap".to_string()
                    }
                }
            }
            "pivot" => self.loop_set_knob(&rest, |st, n| st.pivot = n, "pivot"),
            "stall" => self.loop_set_knob(&rest, |st, n| st.stall_stop = n, "stall pivot"),
            "min" => self.loop_set_knob(&rest, |st, n| st.min_findings = n, "min findings"),
            _ => {
                // Bare `/loop <task>` (with optional leading interval) opens the
                // in-world launcher; Start creates the fresh run.
                let (iv, task) = split_interval(raw);
                self.loop_open_dialog(task, iv, false)
            }
        }
    }

    fn loop_open_dialog(&mut self, task: String, interval: u64, deli: bool) -> String {
        self.loop_open_dialog_profile(task, interval, deli, false)
    }

    fn loop_open_dialog_profile(
        &mut self,
        task: String,
        interval: u64,
        deli: bool,
        podrace: bool,
    ) -> String {
        if self.loop_active() {
            return "a loop is already driving — /loop status, /loop stop, or /loop restart"
                .to_string();
        }
        let mut task = task.trim().to_string();
        let mut interval = interval;
        let mut deli = deli;
        let has_goal = self
            .goal
            .as_ref()
            .map(|g| !g.text.trim().is_empty())
            .unwrap_or(false);
        if task.is_empty()
            && !has_goal
            && let Some(saved) = self.saved_loop_for_current_workspace()
        {
            task = saved.task;
            interval = saved.interval_secs;
            deli |= saved.deli;
        }
        if task.trim().is_empty() && !has_goal {
            return "nothing to loop on — /loop <task>, or set a /goal first".to_string();
        }
        self.loop_dialog = Some(if podrace {
            LoopLaunchDialog::podrace(task, interval)
        } else {
            LoopLaunchDialog::new(task, interval, deli)
        });
        self.loop_dialog_hits.clear();
        self.world.note_workshop();
        self.scryglass
            .navigate(crate::scryglass::StageRoute::Workshop);
        let _ = self
            .module_host
            .activate(&crate::runtime::ModuleId::new("artifacts"));
        if podrace {
            "competition podrace armed — 5 days, unlimited iterations/tokens; review then Start"
                .to_string()
        } else {
            "loop workshop opened — choose time, iterations, budget, then Start".to_string()
        }
    }

    fn loop_restart_dialog(&mut self, rest: String, deli: bool) -> String {
        let was_active = self.loop_active();
        self.loop_cancel_inflight();
        self.loop_ctl.last_failed_acceptance = None;
        let saved = self.saved_loop_for_current_workspace();
        let limits = if self.loop_ctl.status != LoopStatus::Idle {
            Some(&self.loop_ctl)
        } else {
            saved.as_ref()
        }
        .filter(|state| state.operator_caps)
        .map(|state| crate::loop_dialog::LoopLaunchSettings {
            deadline_secs: state.deadline_secs,
            max_iters: state.max_iters,
            token_budget: state.token_budget,
            podrace: state.podrace,
        });
        let task = if rest.trim().is_empty() {
            if !self.loop_ctl.task.trim().is_empty() {
                self.loop_ctl.task.clone()
            } else {
                saved.as_ref().map(|st| st.task.clone()).unwrap_or_default()
            }
        } else {
            rest.trim().to_string()
        };
        let interval = if self.loop_ctl.interval_secs > 0 {
            self.loop_ctl.interval_secs
        } else {
            saved.as_ref().map(|st| st.interval_secs).unwrap_or(0)
        };
        let deli = deli || self.loop_ctl.deli || saved.as_ref().is_some_and(|st| st.deli);
        let podrace = self.loop_ctl.podrace || saved.as_ref().is_some_and(|st| st.podrace);
        if was_active {
            self.loop_ctl.status = LoopStatus::Stopped;
            self.loop_ctl.wake_at = None;
            self.loop_ctl.cycle_started_ms = None;
        }
        if self.loop_ctl.status != LoopStatus::Idle {
            save(&self.loop_ctl);
        }
        let notice = self.loop_open_dialog_profile(task, interval, deli, podrace);
        if let (Some(dialog), Some(limits)) = (&mut self.loop_dialog, limits) {
            dialog.restore_settings(limits);
            "loop restart workshop opened — saved operator limits retained exactly; review then Start".into()
        } else {
            notice
        }
    }

    fn loop_resume(&mut self) -> String {
        let loaded_saved = self.loop_ctl.status == LoopStatus::Idle;
        if loaded_saved {
            let Some(saved) = self.saved_loop_for_current_workspace() else {
                return "no paused loop to resume".to_string();
            };
            self.loop_ctl = saved;
        }
        let legacy_terminal_stall =
            self.loop_ctl.status == LoopStatus::Stopped && stall_limit_reached(&self.loop_ctl);
        if !(matches!(
            self.loop_ctl.status,
            LoopStatus::Paused | LoopStatus::Stopped | LoopStatus::Failed
        ) || loaded_saved
            && matches!(
                self.loop_ctl.status,
                LoopStatus::Running | LoopStatus::Baselining
            ))
        {
            let status = format!("{:?}", self.loop_ctl.status).to_ascii_lowercase();
            return format!("saved loop is {status} — /loop restart resets counters");
        }
        if let Some(why) = budget_tripped(&self.loop_ctl) {
            return format!(
                "saved loop is at budget: operator cap {why}; /loop endless clears all caps and resumes, or raise the cap then /loop resume"
            );
        }
        if legacy_terminal_stall {
            // Older builds terminally stopped after exhausting the escalation
            // ladder. An explicit `/loop resume` is sufficient authority to
            // recover that persisted work without throwing its ledger away.
            self.loop_ctl.stale_count = 0;
        }
        if self.loop_task_text().trim().is_empty() {
            return "no task or goal to resume — /loop start <task>".to_string();
        }
        // Explicit resume may retry immediately; automatic blocker recovery
        // uses the existing wake timer and retains its diagnostic.
        self.loop_ctl.blocked_repeat_count = 0;
        self.loop_ctl.blocked_prompt_digest = None;
        self.loop_ctl.execution_blocker = None;
        self.loop_ctl.retry_after_error = false;
        self.loop_ctl.last_error = None;
        self.loop_ctl.paused_for_cap = false;
        if self.loop_ctl.baseline_resume_to.is_some()
            || self.loop_ctl.status == LoopStatus::Baselining
        {
            self.loop_ctl.baseline_resume_to = Some(LoopStatus::Running);
            self.loop_ctl.status = LoopStatus::Baselining;
        } else {
            self.loop_ctl.status = LoopStatus::Running;
        }
        self.loop_ctl.awaiting_turn = false;
        self.loop_ctl.cycle_started_ms = None;
        self.loop_ctl.wake_at = Some(Instant::now());
        self.loop_bind_rl();
        save(&self.loop_ctl);
        self.start_lifecycle_ceremony(
            crate::viz::lifecycle_viz::CeremonyKind::LoopStart,
            self.loop_task_text(),
        );
        if legacy_terminal_stall {
            "loop resumed from legacy terminal stall — accumulated evidence preserved".to_string()
        } else {
            "loop resumed".to_string()
        }
    }

    fn saved_loop_for_current_workspace(&self) -> Option<LoopState> {
        load_for_session(self.tools.current_workspace(), &self.session.id)
    }

    pub(crate) fn loop_dialog_key(&mut self, code: KeyCode) -> bool {
        let mut start = false;
        let mut cancel = false;
        if let Some(dialog) = self.loop_dialog.as_mut() {
            match code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => cancel = true,
                KeyCode::Tab | KeyCode::Down => dialog.focus_next(),
                KeyCode::BackTab | KeyCode::Up => dialog.focus_prev(),
                KeyCode::Left => dialog.adjust(false),
                KeyCode::Right => dialog.adjust(true),
                KeyCode::Enter | KeyCode::Char(' ') => match dialog.focused_action() {
                    Some(LoopDialogAction::Start) => start = true,
                    Some(LoopDialogAction::Cancel) => cancel = true,
                    _ => dialog.adjust(true),
                },
                KeyCode::Char('s') | KeyCode::Char('S') => start = true,
                _ => {}
            }
        }
        if start {
            let msg = self.loop_dialog_start();
            self.note(msg);
        } else if cancel {
            let msg = self.loop_dialog_cancel();
            self.note(msg);
        }
        true
    }

    pub(crate) fn loop_dialog_click(&mut self, x: u16, y: u16) -> bool {
        let action = self
            .loop_dialog_hits
            .iter()
            .find(|hit| point_in_rect(x, y, hit.rect))
            .map(|hit| hit.action);
        let Some(action) = action else {
            return false;
        };
        match action {
            LoopDialogAction::Start => {
                let msg = self.loop_dialog_start();
                self.note(msg);
            }
            LoopDialogAction::Cancel => {
                let msg = self.loop_dialog_cancel();
                self.note(msg);
            }
            _ => {
                if let Some(dialog) = self.loop_dialog.as_mut() {
                    dialog.apply(action);
                }
            }
        }
        true
    }

    fn loop_dialog_start(&mut self) -> String {
        let Some(dialog) = self.loop_dialog.take() else {
            return "loop workshop is not open".to_string();
        };
        let settings = dialog.settings();
        let is_handoff = dialog.is_handoff_rl();
        self.loop_dialog_hits.clear();
        self.scryglass
            .controller
            .leave_route(crate::scryglass::StageRoute::Workshop);
        let message = if is_handoff {
            self.handoff_rl_start_with_limits(dialog.task, dialog.interval_secs, Some(settings))
        } else {
            self.loop_start_with_limits(
                dialog.task,
                dialog.interval_secs,
                dialog.deli,
                Some(settings),
            )
        };
        // Starting the loop may activate its lifecycle ceremony, which focuses
        // Stage. Hand ownership back after that presentation is armed so compact
        // terminals keep the console visible while the operator steers.
        self.focus_module("core");
        message
    }

    fn loop_dialog_cancel(&mut self) -> String {
        let was_handoff = self.loop_dialog.as_ref().is_some_and(|d| d.is_handoff_rl());
        self.loop_dialog = None;
        self.loop_dialog_hits.clear();
        self.scryglass
            .controller
            .leave_route(crate::scryglass::StageRoute::Workshop);
        self.focus_module("core");
        if was_handoff {
            "handoff-rl workshop closed".to_string()
        } else {
            "loop workshop closed".to_string()
        }
    }

    fn loop_set_knob(
        &mut self,
        rest: &str,
        set: impl FnOnce(&mut LoopState, usize),
        name: &str,
    ) -> String {
        match parse_usize_or_endless(rest) {
            Some(n) => {
                set(&mut self.loop_ctl, n);
                save(&self.loop_ctl);
                if n == 0 && name == "max iters" {
                    format!("{name} = ∞ · {}", self.loop_ctl.cap_summary())
                } else {
                    format!("{name} = {n}")
                }
            }
            None => format!(
                "usage: /loop {} N",
                name.split_whitespace().next().unwrap_or(name)
            ),
        }
    }

    /// Skip the workshop: arm the loop immediately with full limits.
    pub(crate) fn loop_start_immediate(
        &mut self,
        task: String,
        interval: u64,
        deli: bool,
        podrace: bool,
    ) -> String {
        if self.loop_active() {
            return "a loop is already driving — /loop status, /loop stop, or /loop restart"
                .to_string();
        }
        let limits = if podrace {
            Some(LoopLaunchDialog::podrace(task.clone(), interval).settings())
        } else {
            Some(LoopLaunchDialog::new(task.clone(), interval, deli).settings())
        };
        self.loop_start_with_limits(task, interval, deli, limits)
    }

    fn loop_start_with_limits(
        &mut self,
        task: String,
        interval: u64,
        deli: bool,
        limits: Option<LoopLaunchSettings>,
    ) -> String {
        let (task, max_iters_override) = parse_loop_start_options(&task);
        let has_goal = self
            .goal
            .as_ref()
            .map(|g| !g.text.trim().is_empty())
            .unwrap_or(false);
        if task.is_empty() && !has_goal {
            return "nothing to loop on — /loop <task>, or set a /goal first".to_string();
        }
        let mut st = LoopState::configured_from_env();
        st.owner_session_id = Some(self.session.id.clone());
        st.workspace = Some(self.tools.current_workspace().to_path_buf());
        st.start_rev = git_head(st.workspace.as_deref());
        st.last_workspace_fingerprint = st
            .workspace
            .as_deref()
            .and_then(crate::harness::workspace_fingerprint);
        st.task = task.clone();
        st.interval_secs = interval;
        st.deli = deli;
        if let Some(limits) = limits {
            st.deadline_secs = limits.deadline_secs;
            st.max_iters = limits.max_iters;
            st.token_budget = limits.token_budget;
            st.podrace = limits.podrace;
        }
        if let Some(max_iters) = max_iters_override {
            st.max_iters = max_iters;
        }
        // Replace intent without releasing any older command owner.
        self.loop_retire_pending();
        // Pin the acceptance command at start so the verifiable predicate can't be
        // swapped mid-run (a pinned, immutable predicate).
        st.accept_cmd = self
            .goal
            .as_ref()
            .and_then(|g| g.accept_cmd.clone())
            .filter(|c| !c.trim().is_empty());
        st.started_ms = now_ms();
        st.updated_ms = now_ms();
        st.binary = Some(evidence::loop_binary_identity());
        st.tokens = LoopTokenSplit::default();
        st.wake_at = Some(Instant::now());
        self.loop_ctl = st;
        // If a verifiable command is pinned, capture the PRE-EDIT baseline pass
        // count first (reward-hack guard); otherwise run immediately.
        if let Some(cmd) = self.loop_ctl.accept_cmd.clone() {
            self.loop_spawn_baseline(cmd, LoopStatus::Running);
        } else {
            self.loop_ctl.status = LoopStatus::Running;
        }
        self.loop_bind_rl();
        save(&self.loop_ctl);
        let what = if task.is_empty() {
            format!(
                "goal: {}",
                self.goal.as_ref().map(|g| g.text.trim()).unwrap_or("")
            )
        } else {
            task
        };
        let method = if self.loop_ctl.podrace {
            " · competition podrace"
        } else if deli {
            " · deli deep-think"
        } else {
            ""
        };
        self.start_lifecycle_ceremony(crate::viz::lifecycle_viz::CeremonyKind::LoopStart, what.clone());
        format!(
            "loop started → {what}{method}\n  {} · stall pivot {} · pivot {} · interval {}s",
            self.loop_ctl.cap_summary(),
            self.loop_ctl.stall_stop,
            self.loop_ctl.pivot,
            self.loop_ctl.interval_secs
        )
    }

    /// Arm the next iteration if the loop is running, idle of in-flight work, and
    /// its interval has elapsed. Called from `advance()` when nothing is pending.
    pub(crate) fn loop_arm(&mut self) {
        if !self.loop_active() {
            self.tools.rl().end_loop();
        }
        self.loop_account_rl();
        if self.exit_request.is_some()
            || self.thinking.is_some()
            || self.bg_job.is_some()
            || self.loop_pending.is_some()
        {
            return; // single-flight: never collide with in-flight work
        }
        if self.loop_ctl.status == LoopStatus::Baselining && !self.loop_ctl.awaiting_turn {
            if self
                .loop_ctl
                .wake_at
                .is_some_and(|wake| Instant::now() < wake)
            {
                return;
            }
            if let Some(why) = budget_tripped(&self.loop_ctl) {
                self.loop_pause_for_budget(&why);
                return;
            }
            let resume_to = self
                .loop_ctl
                .baseline_resume_to
                .unwrap_or(LoopStatus::Paused);
            if let Some(cmd) = self
                .loop_ctl
                .accept_cmd
                .clone()
                .filter(|cmd| !cmd.trim().is_empty())
            {
                self.loop_spawn_baseline(cmd, resume_to);
            } else {
                self.loop_finish(
                    LoopStatus::Paused,
                    "baseline capture has no pinned command; rebind /goal cmd before resuming",
                );
            }
            return;
        }
        if self.loop_ctl.status != LoopStatus::Running || self.loop_ctl.awaiting_turn {
            return;
        }
        if let Some(w) = self.loop_ctl.wake_at
            && Instant::now() < w
        {
            return; // interval pacing — no blocking sleep, just a timer check
        }
        if let Some(why) = budget_tripped(&self.loop_ctl) {
            self.loop_pause_for_budget(&why);
            return;
        }
        // A goal-driving loop consumes the standing goal's round budget: each
        // armed iteration is one continuation round, and a spent budget (or a
        // blocked/done/paused goal) pauses the run instead of letting the
        // loop outlive its objective.
        if self.loop_ctl.task.trim().is_empty() {
            match self.loop_goal_gate() {
                Ok(Some(why)) => {
                    self.loop_pause_for_goal_gate(&why);
                    return;
                }
                Err(error) => {
                    self.loop_ctl.last_error = Some(error.clone());
                    self.loop_ctl.last_setback = Some(error.clone());
                    self.note(format!(
                        "loop · {error}; retry in at least 60s before starting a model turn"
                    ));
                    self.loop_schedule_next();
                    if self.loop_ctl.status == LoopStatus::Running {
                        let retry_at = Instant::now() + Duration::from_secs(60);
                        self.loop_ctl.wake_at =
                            Some(self.loop_ctl.wake_at.unwrap_or(retry_at).max(retry_at));
                        save(&self.loop_ctl);
                    }
                    return;
                }
                Ok(None) => {}
            }
        }
        // The goal can be cleared mid-run; a loop driving an empty objective is
        // never what's wanted, so stop cleanly rather than prompt on nothing.
        if self.loop_task_text().trim().is_empty() {
            self.loop_finish(LoopStatus::Stopped, "no task and the goal was cleared");
            return;
        }
        // Fold any steers still queued (typed between iterations, or missed by
        // a turn that ended first) into the persistent notes so this
        // iteration's prompt — and every later one — carries them.
        for msg in self.steer_queue.drain() {
            let text = msg.content.trim().to_string();
            if !text.is_empty() && !self.loop_ctl.steer_notes.contains(&text) {
                self.loop_ctl.steer_notes.push(text);
            }
        }
        self.loop_start_experiment_if_due();
        let club = self.loop_bind_rl();
        let convo = self.loop_iteration_convo();
        self.loop_ctl.blocked_prompt_digest = self
            .loop_ctl
            .verifier_failure
            .as_ref()
            .map(|f| f.failure_digest.clone());
        self.loop_ctl.pending_proc_completions.clear();
        self.loop_ctl.pending_recovery_contexts.clear();
        // Count the request (input) tokens toward the budget now; the reply
        // (output) tokens are added on harvest. Together they bound spend.
        let convo_tokens: usize = convo.iter().map(|m| est_tokens(&m.content)).sum();
        self.loop_ctl.tokens_spent += convo_tokens;
        self.loop_ctl.tokens.paid_input = self
            .loop_ctl
            .tokens
            .paid_input
            .saturating_add(convo_tokens as u64);
        self.loop_ctl.tokens.total = self
            .loop_ctl
            .tokens
            .total
            .saturating_add(convo_tokens as u64);
        evidence::refresh_binary(&mut self.loop_ctl);
        let requested_route = club.route_identity();
        let label = club.label().to_string();
        let n = self.loop_ctl.iteration + 1;
        let tier = match self.loop_ctl.tier {
            EscalationTier::Local => "",
            EscalationTier::Swarm => " · swarm",
            EscalationTier::Sota => " · SOTA",
        };
        let mut status = format!("loop · iteration {n}{tier} ({label})");
        // HUD truth: measured/submitted receipts and a blocked verifier are
        // the run's real competition state, not the iteration count.
        status.push_str(&format!(
            " · measured {} · submitted {}",
            self.loop_ctl.measured_candidates, self.loop_ctl.submissions
        ));
        if self.loop_ctl.verifier_blocked.is_some() {
            status.push_str(" · VERIFIER BLOCKED");
        }
        status.push_str(" …");
        self.note(status);
        self.loop_ctl.awaiting_turn = true;
        self.loop_ctl.cycle_started_ms = Some(now_ms());
        self.loop_ctl.wake_at = None;
        self.reasoning.clear();
        self.reasoning_text_sanitizer.reset();
        self.reasoning_shown = 0;
        self.thinking = Some(Thinking::spawn(
            label,
            club,
            Arc::clone(&self.tools),
            Arc::from(convo),
            Arc::clone(&self.steer_queue),
            self.session.clone(),
            requested_route,
            Some(self.loop_ctl.id.clone()),
        ));
    }

    /// Record a mid-run steer against the live loop so it rides every later
    /// iteration prompt (fresh-context iterations forget one-shot injections).
    /// A no-op when no loop is active; exact-duplicate notes are folded.
    pub(crate) fn loop_note_steer(&mut self, text: &str) {
        if !self.loop_active() {
            return;
        }
        let text = text.trim();
        if text.is_empty() || self.loop_ctl.steer_notes.iter().any(|n| n == text) {
            return;
        }
        self.loop_ctl.steer_notes.push(text.to_string());
        save(&self.loop_ctl);
    }

    /// `/goal cmd <check>` while a loop is live: re-pin the acceptance command on
    /// the run. The pin exists to stop the *model* swapping the predicate mid-run
    /// (reward hacking); the operator explicitly rebinding it from the composer is
    /// the intended override. The old command's baseline is dropped so its
    /// pass-count can't misjudge the new predicate — and, because the regression
    /// guard only applies while a baseline EXISTS, a fresh one is recaptured
    /// against the new command; dropping it alone silently disarmed the guard
    /// for the rest of the run. Returns `None` when no live run was re-pinned,
    /// else `Some(recapturing)`. A retired verifier/baseline queues fresh capture;
    /// a foreground turn still needs a quiet re-pin. Neither reports a capture
    /// as running before its command is actually admitted.
    pub(crate) fn repin_loop_accept_cmd(&mut self, cmd: &str) -> Option<bool> {
        if self.loop_ctl.self_edit {
            self.note(
                "self · acceptance is the fixed Layer-2 gate; /goal cmd does not re-pin it"
                    .to_string(),
            );
            return None;
        }
        if matches!(
            self.loop_ctl.status,
            LoopStatus::Idle | LoopStatus::Done | LoopStatus::Stopped | LoopStatus::Failed
        ) {
            return None;
        }
        self.loop_ctl.accept_cmd = Some(cmd.to_string());
        self.loop_ctl.baseline_passed = None;
        // An operator re-pin is an explicit predicate/configuration change,
        // even when the new text happens to equal the old command.
        self.loop_ctl.last_failed_acceptance = None;
        // Retire the old capture without releasing its command slot. Persist
        // new capture intent and admit it after the old command settles.
        let stale_resume = match self.loop_pending.as_ref() {
            Some(LoopPending::Baseline(_, resume_to)) => Some(*resume_to),
            Some(LoopPending::RetiredBaseline(_)) => Some(match self.loop_ctl.status {
                LoopStatus::Baselining => self
                    .loop_ctl
                    .baseline_resume_to
                    .unwrap_or(LoopStatus::Paused),
                status => status,
            }),
            Some(LoopPending::Verify(_) | LoopPending::RetiredVerify(_)) => {
                Some(match self.loop_ctl.status {
                    LoopStatus::Verifying => LoopStatus::Running,
                    LoopStatus::Baselining => self
                        .loop_ctl
                        .baseline_resume_to
                        .unwrap_or(LoopStatus::Paused),
                    status => status,
                })
            }
            _ => None,
        };
        if let Some(resume_to) = stale_resume {
            self.loop_retire_pending();
            self.loop_ctl.pending_acceptance = None;
            self.loop_spawn_baseline(cmd.to_string(), resume_to);
            return Some(false); // queued, not yet executing the new predicate
        }
        let quiescent =
            self.loop_pending.is_none() && !self.loop_ctl.awaiting_turn && self.thinking.is_none();
        let recapturing = if quiescent {
            let resume_to = stale_resume.unwrap_or(match self.loop_ctl.status {
                LoopStatus::Baselining => self
                    .loop_ctl
                    .baseline_resume_to
                    .unwrap_or(LoopStatus::Paused),
                s => s,
            });
            self.loop_spawn_baseline(cmd.to_string(), resume_to);
            matches!(self.loop_pending, Some(LoopPending::Baseline(..)))
        } else {
            false
        };
        save(&self.loop_ctl);
        Some(recapturing)
    }

    /// The effective task text the loop is pursuing — its own `task`, else the
    /// active goal's text.
    pub(crate) fn loop_task_text(&self) -> String {
        if self.loop_ctl.task.trim().is_empty() {
            self.goal
                .as_ref()
                .map(|g| g.text.clone())
                .unwrap_or_default()
        } else {
            self.loop_ctl.task.clone()
        }
    }

    /// The fresh, minimal conversation for one iteration: the orchestrator system
    /// prompt + the loop-worker instruction + the active goal, then a curated-state
    /// user prompt. Deliberately NOT `self.history` — fresh context per iteration
    /// is deli's anti-poisoning guard, and keeps loop turns out of the user thread.
    pub(crate) fn loop_iteration_convo(&self) -> Vec<ChatMsg> {
        // Loop workers do NOT inherit the cockpit system prompt (skills catalog,
        // magic keywords, harness capabilities). Their authority is the active
        // competition package's worker profile + WORKER_SYS + the goal: no
        // skill/secret surface hands off into an autonomous loop.
        let mut system = String::new();
        if self.loop_active() {
            system.push_str(
                crate::harness::comp_packages::active_package()
                    .worker_profile()
                    .system_prompt,
            );
            system.push_str("\n\n");
        }
        system.push_str(WORKER_SYS);
        let gb = self.goal_context_block(None);
        if !gb.is_empty() {
            system.push_str("\n\n");
            system.push_str(gb.trim_end());
        }

        let task = self.loop_task_text();
        // A structural pivot is asked for every `pivot` stale iterations — not on every
        // iteration after the first stall. In podrace mode `stale_count` is never reset, so the
        // old `>=` rode every prompt from iteration 2 on and the model re-framed its approach each
        // iteration instead of finishing an experiment (operator finding 2026-09-11, qwen38 loop).
        let pivot = self.loop_ctl.pivot > 0
            && self.loop_ctl.stale_count >= self.loop_ctl.pivot
            && self
                .loop_ctl
                .stale_count
                .is_multiple_of(self.loop_ctl.pivot);
        // Window what rides the prompt: the dedup ledger (`seen`) spans the whole
        // run, so a finding older than the window still registers as stale if
        // restated — only the prompt cost is bounded, not the loop's memory.
        let f_skip = self
            .loop_ctl
            .findings
            .len()
            .saturating_sub(LOOP_PROMPT_FINDINGS);
        let d_skip = self
            .loop_ctl
            .directions_tried
            .len()
            .saturating_sub(LOOP_PROMPT_DIRECTIONS);
        let h_skip = self
            .loop_ctl
            .hypotheses
            .len()
            .saturating_sub(LOOP_PROMPT_HYPOTHESES);
        // Open leads are presented by `curated_prompt` itself, so the in-turn
        // deli driver and this cross-turn controller show them identically.
        let mut prompt = curated_prompt(
            &task,
            &self.loop_ctl.findings[f_skip..],
            &self.loop_ctl.hypotheses[h_skip..],
            &self.loop_ctl.directions_tried[d_skip..],
            pivot,
            EvidenceRegime::Grounded,
        );
        prompt.push_str(&format!(
            "\n\niteration {} · {}",
            self.loop_ctl.iteration,
            self.loop_ctl.cap_summary()
        ));
        // Blocker-first: while the verification path is broken, restoring it
        // outranks every other instruction — a fresh optimization direction
        // is exactly the runaway this must prevent. Rides FIRST.
        if let Some(diagnostic) = &self.loop_ctl.verifier_blocked
            && self.loop_ctl.verifier_failure.as_ref().is_none_or(|f| {
                self.loop_ctl.blocked_prompt_digest.as_ref() != Some(&f.failure_digest)
            })
        {
            prompt.push_str(&format!(
                "\n\n[VERIFICATION PATH BLOCKED — restore it before anything else] \
                 {diagnostic}. Do not propose a new optimization direction. This \
                 iteration's only acceptable outcomes: (1) the official \
                 benchmark/verify command runs to completion, or (2) a direct local \
                 measurement via the repository's own benchmark script, or (3) an \
                 exact, minimal operator action (command + why) if neither is \
                 possible. Notes in the repository are not a blocker; missing inputs \
                 that a script in the repository can fetch are not a blocker."
            ));
        }
        // Submission-overdue: measured candidates exist, nothing has reached
        // the board. Rides right behind the blocker; research is not an
        // acceptable outcome for this iteration.
        if self.loop_submission_overdue() {
            let since = self.loop_iters_since_first_measurement().unwrap_or(0);
            prompt.push_str(&format!(
                "\n\n[SUBMISSION OVERDUE — {} measured candidate(s), 0 submissions, {since} \
                 iterations since the first measurement; continue with a concrete submission action] The board \
                 is the instrument. This iteration's only acceptable outcomes: (1) submit the \
                 best measured candidate through the official submit command (a candidate that \
                 measures at or ahead of the leader on the same local corpus goes in NOW), or \
                 (2) if every measured candidate measures behind the leader, say so in one \
                 line (score vs leader, same corpus) and produce and measure a new candidate \
                 this iteration, or (3) the exact blocking command and its error for the \
                 operator. Do not open a new research direction, do not re-measure what is \
                 already measured, and do not treat local-vs-hidden corpus doubt as a reason \
                 to withhold: the board settles it.",
                self.loop_ctl.measured_candidates
            ));
        }
        if f_skip > 0 || d_skip > 0 {
            prompt.push_str(&format!(
                "\n\n[window: the {f_skip} oldest findings and {d_skip} oldest directions are \
                 elided; restating them still counts as stale ground, not new]"
            ));
        }
        let next_iteration = self.loop_ctl.iteration + 1;
        if next_iteration.is_multiple_of(LOOP_EVIDENCE_REVIEW_INTERVAL) {
            prompt.push_str(
                "\n\n[EVIDENCE REVIEW CHECKPOINT] This is the mandatory 15-iteration audit. \
                 Reconcile every active claim against primary artifacts, identify contradictions \
                 and measured negative results, and state the literal target delta. Do not launch \
                 another costly benchmark/submit/deploy action until the existing evidence is \
                 reconciled. Unsupported novelty is not progress.",
            );
        }
        // The working tree is ground truth the prose findings can't carry:
        // without it an iteration re-makes or contradicts edits it can't see.
        if let Some(diff) = self.loop_workspace_diff() {
            prompt.push_str(
                "\n\n[files changed so far this run (git diff --stat) — build on these; \
                 do not re-make or blindly revert them]\n",
            );
            prompt.push_str(&diff);
        }
        // Operator steering rides every iteration: fresh-context iterations
        // would otherwise forget a note the user sent mid-run a cycle later.
        if !self.loop_ctl.steer_notes.is_empty() {
            prompt.push_str(
                "\n\n[operator steering — notes the user sent mid-run; honor them while \
                 pursuing the task]",
            );
            for note in &self.loop_ctl.steer_notes {
                prompt.push_str("\n- ");
                prompt.push_str(note);
            }
        }
        // Hand the fresh-context iteration its predecessor's failure — without
        // this it re-declares done down the exact same path, verbatim.
        if !self.loop_ctl.pending_proc_completions.is_empty() {
            prompt.push_str("\n\n[background process outcomes — inspect the retained logs and actual verifier; an exit is not a solve]");
            for outcome in &self.loop_ctl.pending_proc_completions {
                prompt.push_str("\n- ");
                prompt.push_str(outcome);
            }
        }
        if let Some(setback) = &self.loop_ctl.last_setback {
            prompt.push_str(
                "\n\n[previous iteration setback — address the cause below first; \
                 do not re-declare done until it is fixed]\n",
            );
            prompt.push_str(setback);
        }
        self.loop_experiment_context(&mut prompt);
        match self.loop_ctl.tier {
            EscalationTier::Swarm => prompt.push_str(
                "\n\nThe single-agent tier stalled — you are now a wider mixture of agents; \
                 attack from genuinely different angles in parallel.",
            ),
            EscalationTier::Sota => prompt.push_str(
                "\n\nYou are the operator-selected SOTA tier. Bring maximum \
                 rigor and a genuinely fresh attack.",
            ),
            EscalationTier::Local => {}
        }
        if self.loop_ctl.self_edit {
            prompt.push_str(
                "\n\nYou are improving the cockpit's OWN source code in an isolated git \
                 worktree (the current workspace root is that worktree's crate). Use the \
                 file and cargo tools to make the change, keep the crate building and its \
                 test suite green, and when the goal is fully achieved end your reply with \
                 a line containing exactly: LOOP_DONE\nA build+test gate with a pass-count \
                 regression guard verifies that claim; only a green gate can be integrated \
                 into the live tree, so never delete or disable tests to get there.",
            );
        } else if self.loop_ctl.accept_cmd.is_some() {
            prompt.push_str(
                "\n\nIf the goal is fully achieved, run the verifiable check and confirm it \
                 passes, then end your reply with a line containing exactly: LOOP_DONE",
            );
        } else {
            prompt.push_str(
                "\n\nNo verifiable acceptance command is bound. Do not declare the loop complete; \
                 keep surfacing concrete progress, blockers, or the next necessary action.",
            );
        }
        if self.loop_ctl.podrace {
            prompt.push_str(
                "\n\n[PODRACE]\nUse the bundled $competition-loop workflow. Take the shortest \
                 path from one measured bottleneck to a distinct validated submission, then decide \
                 from its official score. Run only required checks. Tool activity and research are \
                 not progress. Never resubmit unchanged code. While a result is pending, prepare the \
                 next concrete candidate instead of polling.",
            );
        }
        let mut user = ChatMsg::user(prompt);
        user.recovery_context = self.loop_recovery_context_refs();
        let mut conversation = vec![ChatMsg::system(system), user];
        let rl = self
            .tools
            .rl()
            .loop_context_text(self.tools.current_workspace());
        if !rl.is_empty() {
            conversation.push(ChatMsg::harness(rl));
        }
        // A campaign may have installed a measured, audited policy since the
        // original session system prompt was built. Fresh loop turns consume it.
        let learned = crate::continual_harness::context_block(self.tools.current_workspace());
        if !learned.is_empty() {
            conversation.push(ChatMsg::harness(learned));
        }
        conversation
    }

    /// Test helper for a completed iteration with no tool activity.
    #[cfg(test)]
    pub(crate) fn loop_harvest(&mut self, reply: String) {
        self.loop_harvest_with_tools(reply, ToolStripSnapshot::default());
    }

    /// An inner guard is a pause, not an invitation to buy a fresh turn with
    /// reset counters. Preserve the iteration's evidence without running the
    /// normal success/acceptance/retry ladder. Only explicit resume re-arms it.
    pub(crate) fn loop_harvest_stopped(
        &mut self,
        reply: String,
        tools: ToolStripSnapshot,
        reason: crate::harness::TurnStopReason,
    ) {
        self.loop_account_rl();
        self.clear_partial();
        let visible = self.display_reply(reply.clone());
        self.messages.push(Message::new(Role::Angel, visible));
        apply_reply_with_tools(&mut self.loop_ctl, &reply, &tools);
        self.loop_ctl.retry_after_error = false;
        let note = format!(
            "inner turn stopped ({}); automatic continuation paused — inspect the retained \
             evidence before `/loop resume`",
            reason.as_str()
        );
        self.loop_ctl.last_error = Some(note.clone());
        self.loop_ctl.last_setback = Some(note.clone());
        self.loop_finish(LoopStatus::Paused, &note);
    }

    /// Fold a completed loop iteration and its tool evidence into state, then
    /// run the done/stop ladder. Called from `advance()` instead of the normal
    /// turn handling.
    pub(crate) fn loop_harvest_with_tools(&mut self, reply: String, tools: ToolStripSnapshot) {
        self.loop_account_rl();
        self.loop_ctl.awaiting_turn = false;
        self.loop_ctl.retry_after_error = false;
        // Completed replies retire the active error; its transcript entry remains.
        // apply_reply_with_tools reinstates an error for recognized non-results.
        self.loop_ctl.last_error = None;
        self.clear_partial();
        let visible = self.display_reply(reply.clone());
        self.messages.push(Message::new(Role::Angel, visible));
        if self.loop_ctl.execution_blocker.is_some() {
            // A model reply cannot repair host confinement. Only explicit
            // resume clears this diagnostic and permits another iteration.
            self.loop_ctl.last_error = self.loop_ctl.execution_blocker.clone();
            self.loop_ctl.status = LoopStatus::Paused;
            self.loop_ctl.wake_at = None;
            save(&self.loop_ctl);
            return;
        }
        apply_reply_with_tools(&mut self.loop_ctl, &reply, &tools);
        if self.loop_escalate_blocked_verifier() {
            return;
        }

        // Missing receipts steer the next action; absence of a recognized
        // receipt does not establish that no benchmark command was executed.
        if self.loop_first_candidate_overdue() {
            self.loop_steer_measurement();
        }
        self.loop_note_first_measurement();
        if self.loop_submission_directive_due() {
            self.loop_steer_submission();
        }

        // --- done / stop ladder (cheapest + strongest first) ---
        if let Some(why) = budget_tripped(&self.loop_ctl) {
            self.loop_pause_for_budget(&why);
            return;
        }
        let claims_done = says_done(&reply);
        let min_met = self.loop_ctl.min_findings > 0
            && self.loop_ctl.findings.len() >= self.loop_ctl.min_findings;
        if claims_done || min_met {
            // A self-modification run verifies through the Layer-2 gate — build +
            // tests + regression guard in the worktree — never the model's word.
            if self.loop_ctl.self_edit {
                self.loop_spawn_self_gate();
                return;
            }
            // The pinned command is ground truth; an unverified done claim
            // asks the next iteration for concrete acceptance evidence.
            if let Some(cmd) = self.loop_ctl.accept_cmd.clone() {
                self.loop_spawn_verify(cmd);
                return;
            }
            if min_met {
                self.loop_finish(LoopStatus::Done, "findings goal met");
            } else {
                let setback = "unverified done claim: continue the task and produce concrete verification evidence; no acceptance command is bound";
                self.loop_ctl.last_setback = Some(setback.to_string());
                self.note(format!("loop · {setback}"));
                self.loop_ctl.stale_count = self.loop_ctl.stale_count.saturating_add(1);
                self.loop_schedule_next();
            }
            return;
        }
        if stall_limit_reached(&self.loop_ctl)
            && (!self.loop_ctl.podrace
                || self
                    .loop_ctl
                    .stale_count
                    .is_multiple_of(self.loop_ctl.stall_stop))
        {
            self.loop_pivot_after_stall();
            return;
        }
        self.loop_schedule_next();
    }

    /// A stall steers a concrete pivot on the selected route; only an explicit
    /// operator action may change the tier or request a more expensive model.
    fn loop_pivot_after_stall(&mut self) {
        if self.loop_ctl.verifier_failure.is_some() {
            self.loop_schedule_next();
            return;
        }
        if !self.loop_ctl.podrace {
            self.loop_ctl.stale_count = 0;
        }
        if !self.loop_first_candidate_overdue() && !self.loop_submission_overdue() {
            if self.loop_ctl.podrace {
                self.loop_ctl.last_setback = Some(
                "no comparable objective improvement recorded yet; progress remains unknown. Inspect existing evidence against the fixed baseline. Preserve an unfinished discriminating experiment until its result is available; do not abandon a sustained deep-cut hypothesis merely because this review is due. Retire only a disproved hypothesis, then choose the next concrete mechanism and smallest available check. Repeated competitive submissions of unchanged candidates are banned; do not redraw to obtain a new receipt.".to_string(),
            );
            } else {
                self.loop_ctl.last_setback = Some(
                "stalled without new verified progress: change one concrete constraint, run the smallest discriminating check, and use its result to choose the next action".to_string(),
            );
            }
        }
        self.note(
            "loop · stalled — continuing on the selected route with a concrete next-action pivot"
                .to_string(),
        );
        self.loop_schedule_next();
    }

    /// Failed turns use paced continuation, including after inner HTTP retries
    /// are exhausted. An explicit hop horizon remains an ordinary continuation.
    /// Evidence and stall accounting are retained on the selected route.
    pub(crate) fn loop_harvest_error_with_tools(&mut self, err: String, tools: ToolStripSnapshot) {
        self.loop_ctl.awaiting_turn = false;
        let hop_horizon = err.contains("-hop runaway guard");
        self.loop_ctl.retry_after_error = !hop_horizon;
        self.clear_partial();
        if crate::harness::is_execution_blocker(&err) {
            self.loop_ctl.iteration = self.loop_ctl.iteration.saturating_add(1);
            self.loop_ctl.tool_calls_total =
                self.loop_ctl.tool_calls_total.saturating_add(tools.calls);
            self.loop_ctl.tool_errors_total =
                self.loop_ctl.tool_errors_total.saturating_add(tools.errors);
            self.loop_ctl.execution_blocker = Some(err.clone());
            self.loop_ctl.last_error = Some(err.clone());
            self.loop_ctl.last_setback = Some(err.clone());
            self.loop_ctl.retry_after_error = false;
            if let Some(why) = budget_tripped(&self.loop_ctl) {
                self.loop_pause_for_budget(&why);
                return;
            }
            self.loop_ctl.status = LoopStatus::Paused;
            self.loop_ctl.wake_at = None;
            self.note(format!(
                "loop · confinement unavailable; automatic retry disabled: {err}"
            ));
            save(&self.loop_ctl);
            return;
        }
        let session_fault = crate::harness::classify_session_fault(&err);
        let provider_blocked = crate::club::error_requires_provider_action(&err);
        self.loop_ctl.last_error = Some(err.clone());
        self.loop_ctl.last_setback = Some(if self.loop_ctl.podrace && hop_horizon {
            "the prior turn hit an explicitly configured hop horizon; do not restart reconnaissance — continue from the current workspace and make validation, submission, or score retrieval the next action"
                .to_string()
        } else if let Some(fault) = session_fault {
            crate::harness::deterministic_recovery_note(fault)
        } else {
            format!("the previous iteration ended in an error, not a result: {err}")
        });
        self.loop_ctl.iteration += 1;
        self.loop_ctl.tool_calls_total = self.loop_ctl.tool_calls_total.saturating_add(tools.calls);
        self.loop_ctl.tool_errors_total =
            self.loop_ctl.tool_errors_total.saturating_add(tools.errors);
        // A provider/horizon failure does not erase a successful mutation or
        // external outcome already completed in the same turn. Novel receipts
        // and real workspace changes are progress in every loop profile.
        let unhealthy_tools =
            tools.calls >= 4 && tools.errors.saturating_add(tools.incomplete) * 4 >= tools.calls;
        let workspace_changed = observe_workspace_change(&mut self.loop_ctl);
        let outcome_progress = if unhealthy_tools {
            0
        } else {
            tools.outcome_actions.len()
        };
        let novel_outcome_actions = if unhealthy_tools {
            0
        } else {
            register_outcome_actions(&mut self.loop_ctl, &tools.outcome_actions)
        };
        // Verified receipts survive an error iteration too: a benchmark that
        // completed in this turn is still a measured candidate, and a failed
        // benchmark/verify/submit action still blocks the verification path.
        register_verified_outcome_actions(&mut self.loop_ctl, &tools.verified_outcome_actions);
        self.loop_ctl.observe_verifier_failure(&tools);
        if provider_blocked || session_fault.is_some() {
            // Provider configuration and local-session death are not research
            // stalls. The former waits; the latter opens a fresh context.
        } else if self.loop_ctl.podrace || (novel_outcome_actions == 0 && !workspace_changed) {
            // As on successful turns, raw execution receipts and arbitrary
            // workspace edits do not establish comparable objective progress.
            self.loop_ctl.stale_count += 1;
        } else {
            self.loop_ctl.stale_count = 0;
        }
        let duplicate_costly_actions =
            register_costly_actions(&mut self.loop_ctl, &tools.costly_actions);
        self.loop_ctl.log.push(LoopIterLog {
            iteration: self.loop_ctl.iteration,
            direction: String::new(),
            new_findings: 0,
            reported_findings: 0,
            unverified_findings: 0,
            tool_calls: tools.calls,
            tool_errors: tools.errors,
            duplicate_costly_actions,
            outcome_progress,
            novel_outcome_actions,
            verified_outcome_actions: tools.verified_outcome_actions.len(),
            workspace_changed,
            evidence_review: self
                .loop_ctl
                .iteration
                .is_multiple_of(LOOP_EVIDENCE_REVIEW_INTERVAL),
            stale_count: self.loop_ctl.stale_count,
            ts_ms: now_ms(),
        });
        if self.loop_escalate_blocked_verifier() {
            return;
        }
        if self.loop_first_candidate_overdue() {
            self.loop_steer_measurement();
        }
        self.loop_note_first_measurement();
        if self.loop_submission_directive_due() {
            self.loop_steer_submission();
        }
        if provider_blocked {
            self.loop_ctl.last_setback = Some(format!(
                "provider account/configuration blocked: {err}; retry on the selected route after the backoff"
            ));
            self.note(format!(
                "loop · provider blocked; retry in at least 60s on the selected route: {err}"
            ));
            self.loop_schedule_next();
            if self.loop_ctl.status == LoopStatus::Running {
                let retry_at = Instant::now() + Duration::from_secs(60);
                self.loop_ctl.wake_at =
                    Some(self.loop_ctl.wake_at.unwrap_or(retry_at).max(retry_at));
                save(&self.loop_ctl);
            }
            return;
        }
        if self.loop_ctl.verifier_failure.is_none() || hop_horizon || session_fault.is_some() {
            if hop_horizon {
                self.note(format!(
                "loop · configured hop horizon rolled into a continuation; campaign remains live: {err}"
            ));
            } else if session_fault.is_some() {
                self.note(format!(
                "loop · teacher-watch recovered a long local session ({}); campaign remains live: {err}",
                session_fault.unwrap().as_str()
            ));
            } else {
                self.note(format!(
                "loop · iteration error (counts as a stall); retry in at least 60s on the selected route: {err}"
            ));
            }
        }
        if let Some(why) = budget_tripped(&self.loop_ctl) {
            self.loop_pause_for_budget(&why);
            return;
        }
        if stall_limit_reached(&self.loop_ctl)
            && (!self.loop_ctl.podrace
                || self
                    .loop_ctl
                    .stale_count
                    .is_multiple_of(self.loop_ctl.stall_stop))
        {
            self.loop_pivot_after_stall();
            return;
        }
        self.loop_schedule_next();
    }

    /// Drain a pending off-thread result (verify / approval). Called each frame
    /// from `advance()` when no turn is in flight.
    pub(crate) fn loop_drain_pending(&mut self) {
        let Some(pending) = self.loop_pending.take() else {
            return;
        };
        match pending {
            LoopPending::RetiredBaseline(rx) => match rx.try_recv() {
                Err(TryRecvError::Empty) => {
                    self.loop_pending = Some(LoopPending::RetiredBaseline(rx));
                }
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    self.note("loop worker drained — verifier slot released".to_string());
                }
            },
            LoopPending::RetiredVerify(rx) => match rx.try_recv() {
                Err(TryRecvError::Empty) => {
                    self.loop_pending = Some(LoopPending::RetiredVerify(rx));
                }
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    self.note("loop worker drained — verifier slot released".to_string());
                }
            },
            LoopPending::Baseline(rx, resume_to) => match rx.try_recv() {
                Ok(n) => {
                    self.loop_ctl.baseline_passed = Some(n);
                    self.note(format!(
                        "loop · baseline captured: {n} passing test(s) — a later green below \
                         this is a regression"
                    ));
                    self.loop_resume_after_baseline(resume_to);
                }
                Err(TryRecvError::Empty) => {
                    self.loop_pending = Some(LoopPending::Baseline(rx, resume_to))
                }
                Err(TryRecvError::Disconnected) => {
                    // Losing a receiver must not silently remove the regression
                    // baseline. Retry capture through the ordinary single-flight gate.
                    self.loop_ctl.baseline_resume_to = Some(resume_to);
                    self.loop_ctl.baseline_passed = None;
                    self.loop_ctl.last_error = Some("baseline worker died".to_string());
                    self.loop_ctl.last_setback = Some("baseline worker died; retrying the pinned baseline capture before model work".to_string());
                    if let Some(why) = budget_tripped(&self.loop_ctl) {
                        self.loop_pause_for_budget(&why);
                    } else {
                        self.loop_ctl.status = LoopStatus::Baselining;
                        self.loop_ctl.wake_at = Some(Instant::now() + Duration::from_secs(60));
                        self.note("loop · baseline worker died; retry capture in 60s".to_string());
                        save(&self.loop_ctl);
                    }
                }
            },
            LoopPending::Verify(rx) => {
                match rx.try_recv() {
                    Ok(res) => {
                        if let Some(run) = self.loop_ctl.pending_acceptance.take() {
                            let still_current = self.loop_ctl.accept_cmd.as_deref()
                                == Some(run.command.as_str())
                                && acceptance_config_identity(self.loop_ctl.baseline_passed)
                                    == run.config_identity;
                            if !still_current {
                                self.loop_ctl.last_failed_acceptance = None;
                                self.note(
                                "loop · discarded a stale acceptance result after the predicate/configuration changed"
                                    .to_string(),
                            );
                                self.loop_schedule_next();
                                return;
                            }
                            let workspace =
                                self.loop_ctl.workspace.clone().unwrap_or_else(|| {
                                    self.tools.current_workspace().to_path_buf()
                                });
                            record_acceptance_result(&mut self.loop_ctl, run, &res, &workspace);
                        }
                        self.loop_apply_verify_result(res, false);
                    }
                    Err(TryRecvError::Empty) => self.loop_pending = Some(LoopPending::Verify(rx)),
                    Err(TryRecvError::Disconnected) => {
                        self.loop_ctl.pending_acceptance = None;
                        let error = "verify worker died";
                        self.loop_ctl.last_error = Some(error.to_string());
                        self.loop_ctl.last_setback = Some("verify worker died without an acceptance result; retry the check after the backoff, without treating it as a red receipt".to_string());
                        self.note("loop · verify worker died; retry in at least 60s on the selected route".to_string());
                        self.loop_schedule_next();
                        if self.loop_ctl.status == LoopStatus::Running {
                            let retry_at = Instant::now() + Duration::from_secs(60);
                            self.loop_ctl.wake_at =
                                Some(self.loop_ctl.wake_at.unwrap_or(retry_at).max(retry_at));
                            save(&self.loop_ctl);
                        }
                    }
                }
            }
            LoopPending::Approval(rx) => match rx.try_recv() {
                Ok(Decision::Deny) => {
                    self.loop_ctl.sota_declined = true;
                    self.note(
                        "loop · SOTA escalation denied — continuing on the selected route"
                            .to_string(),
                    );
                    self.loop_schedule_next();
                }
                Ok(_) => {
                    self.loop_ctl.tier = EscalationTier::Sota;
                    self.loop_ctl.stale_count = 0; // fresh stall budget for the new tier
                    self.note(
                        "loop · SOTA escalation approved — bringing in the SOTA model".to_string(),
                    );
                    self.loop_schedule_next();
                }
                Err(TryRecvError::Empty) => self.loop_pending = Some(LoopPending::Approval(rx)),
                Err(TryRecvError::Disconnected) => self.loop_schedule_next(),
            },
            LoopPending::SelfIntegrate(rx) => match rx.try_recv() {
                Ok(Decision::Deny) => {
                    let branch = self.loop_ctl.self_branch.clone().unwrap_or_default();
                    self.self_restore_workspace();
                    self.loop_finish(
                        LoopStatus::Done,
                        &format!(
                            "gate green — branch {branch} kept unmerged; /self integrate to \
                             merge later, /self discard to drop it"
                        ),
                    );
                }
                Ok(_) => self.self_integrate_now(),
                Err(TryRecvError::Empty) => {
                    self.loop_pending = Some(LoopPending::SelfIntegrate(rx))
                }
                Err(TryRecvError::Disconnected) => self.loop_finish(
                    LoopStatus::Paused,
                    "integration approval lost — /self integrate to retry",
                ),
            },
        }
    }

    fn loop_apply_verify_result(&mut self, res: VerifyResult, reused_red: bool) {
        if res.passed {
            if self.loop_ctl.self_edit {
                // The Layer-2 invariant: a green gate never reaches the live
                // tree without the operator's keystroke.
                self.self_request_integrate(&res.summary);
            } else {
                self.loop_finish(LoopStatus::Done, &format!("verified — {}", res.summary));
            }
            return;
        }

        if reused_red {
            self.note(format!(
                "loop · unchanged-red acceptance receipt replayed; process not spawned ({}) — continuing",
                res.summary
            ));
        } else {
            self.note(format!(
                "loop · acceptance check failed ({}) — continuing",
                res.summary
            ));
        }
        let mut setback = if reused_red {
            format!(
                "the unchanged acceptance check remains FAILED without rerunning the process: {}",
                res.summary
            )
        } else {
            format!("the acceptance check FAILED: {}", res.summary)
        };
        if !res.detail.is_empty() {
            setback.push('\n');
            setback.push_str(&res.detail);
        }
        self.loop_ctl.last_setback = Some(setback);
        self.loop_ctl.stale_count = self.loop_ctl.stale_count.saturating_add(1);
        self.loop_schedule_next();
    }

    /// Bounded `git diff --stat` of the loop's workspace since the run started
    /// (uncommitted-vs-HEAD when no start rev was captured — a resumed old
    /// run). `None` outside a git repo or when nothing changed. One fast git
    /// call per iteration arm — never on the render path.
    fn loop_workspace_diff(&self) -> Option<String> {
        let ws = self
            .loop_ctl
            .workspace
            .clone()
            .unwrap_or_else(|| self.tools.current_workspace().to_path_buf());
        let rev = self.loop_ctl.start_rev.as_deref().unwrap_or("HEAD");
        let stat =
            crate::workspace_store::capture_repo_probe("git", &["diff", "--stat", rev], &ws, 5)?;
        let stat = stat.trim_end();
        if stat.is_empty() {
            return None;
        }
        Some(clip_diff_stat(stat, 24))
    }

    /// Capture through the same command path used at initial launch. A saved
    /// intent survives a crash or explicit pause while this receiver is live.
    fn loop_spawn_baseline(&mut self, cmd: String, resume_to: LoopStatus) {
        self.loop_ctl.status = LoopStatus::Baselining;
        self.loop_ctl.baseline_resume_to = Some(resume_to);
        self.loop_ctl.baseline_passed = None;
        if self.thinking.is_some()
            || self.bg_job.is_some()
            || self.pending_turn.is_some()
            || self.loop_pending.is_some()
        {
            // Starting a new loop must not race an existing foreground owner.
            // Preserve capture intent and any accepted manual turn; the ordinary
            // idle path will retry baseline admission when those owners settle.
            self.loop_ctl.wake_at = Some(Instant::now());
            save(&self.loop_ctl);
            self.note("loop · baseline queued until existing work drains".to_string());
            return;
        }
        self.loop_ctl.wake_at = None;
        self.note(format!("loop · capturing pre-edit baseline: `{cmd}`"));
        let ws = self
            .loop_ctl
            .workspace
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        save(&self.loop_ctl);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(count_passed_in(&cmd, &ws));
        });
        self.loop_pending = Some(LoopPending::Baseline(rx, resume_to));
    }

    /// Restore the run once a baseline capture lands: back to `Running` arms
    /// the next iteration; any other prior status (a re-pin while Paused) is
    /// restored as-is — the capture must never un-park a run by side effect.
    fn loop_resume_after_baseline(&mut self, resume_to: LoopStatus) {
        self.loop_ctl.baseline_resume_to = None;
        if resume_to == LoopStatus::Running {
            self.loop_schedule_next();
        } else {
            self.loop_ctl.status = resume_to;
            save(&self.loop_ctl);
        }
    }

    /// Esc on a running loop parks it (Paused) and abandons the in-flight
    /// iteration. Resumable via `/loop resume`.
    pub(crate) fn loop_on_interrupt(&mut self) {
        if !self.loop_active() {
            return;
        }
        self.loop_cancel_inflight();
        self.loop_ctl.status = LoopStatus::Paused;
        self.loop_ctl.wake_at = None;
        save(&self.loop_ctl);
        self.start_lifecycle_ceremony(
            crate::viz::lifecycle_viz::CeremonyKind::LoopPaused,
            self.loop_task_text(),
        );
        self.note(
            "loop paused (interrupt) — current iteration abandoned; /loop resume to continue"
                .to_string(),
        );
    }

    fn loop_schedule_next(&mut self) {
        // All continuation paths, including blockers and red acceptance,
        // honor real campaign caps before they can arm another attempt.
        if let Some(why) = budget_tripped(&self.loop_ctl) {
            self.loop_pause_for_budget(&why);
            return;
        }
        self.loop_ctl.status = LoopStatus::Running;
        self.loop_ctl.cycle_started_ms = None;
        // Defense in depth against the `Instant + Duration` overflow panic: even
        // if a large `interval_secs` reached here from some path other than
        // `parse_interval` (which now clamps), `checked_add` degrades to a
        // far-future wake instead of tearing down the UI thread.
        let retry_floor = if self.loop_ctl.retry_after_error {
            60
        } else {
            0
        };
        let interval = Duration::from_secs(
            self.loop_ctl
                .interval_secs
                .min(MAX_LOOP_INTERVAL_SECS)
                .max(retry_floor),
        );
        self.loop_ctl.wake_at = Some(
            Instant::now()
                .checked_add(interval)
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600)),
        );
        self.loop_ctl.updated_ms = now_ms();
        save(&self.loop_ctl);
    }

    pub(crate) fn loop_finish(&mut self, status: LoopStatus, reason: &str) {
        self.tools.rl().end_loop();
        self.loop_account_rl();
        self.loop_ctl.status = status;
        self.loop_ctl.wake_at = None;
        self.loop_ctl.awaiting_turn = false;
        self.loop_ctl.cycle_started_ms = None;
        self.loop_ctl.updated_ms = now_ms();
        self.loop_retire_pending();
        self.loop_ctl.pending_acceptance = None;
        save(&self.loop_ctl);
        let label = match status {
            LoopStatus::Done => "done",
            LoopStatus::Paused => "paused",
            LoopStatus::Stopped => "stopped",
            LoopStatus::Failed => "failed",
            _ => "ended",
        };
        self.note(format!(
            "loop {label}: {} iteration(s), {} finding(s) — {reason}",
            self.loop_ctl.iteration,
            self.loop_ctl.findings.len()
        ));
        let kind = match status {
            LoopStatus::Done => crate::viz::lifecycle_viz::CeremonyKind::LoopDone,
            LoopStatus::Paused => crate::viz::lifecycle_viz::CeremonyKind::LoopPaused,
            LoopStatus::Stopped => crate::viz::lifecycle_viz::CeremonyKind::LoopStopped,
            LoopStatus::Failed => crate::viz::lifecycle_viz::CeremonyKind::LoopFailed,
            _ => crate::viz::lifecycle_viz::CeremonyKind::LoopStopped,
        };
        self.start_lifecycle_ceremony(kind, self.loop_task_text());
    }

    fn loop_pause_for_budget(&mut self, why: &str) {
        if budget_tripped(&self.loop_ctl).is_none() {
            return;
        }
        self.loop_ctl.paused_for_cap = true;
        self.loop_finish(
            LoopStatus::Paused,
            &format!(
                "operator budget reached: {why}; `/loop endless` clears all caps and resumes, or raise the cap then `/loop resume`"
            ),
        );
    }

    /// Podrace first-candidate clock: the run registered `first_candidate_iters`
    /// or more iterations without a single verified measured candidate.
    fn loop_first_candidate_overdue(&self) -> bool {
        self.loop_ctl.podrace
            && self.loop_ctl.first_candidate_iters > 0
            && self.loop_ctl.iteration >= self.loop_ctl.first_candidate_iters
            && self.loop_ctl.measured_candidates == 0
    }

    /// Pin the iteration of the first measured candidate (the submission
    /// clock starts there).
    fn loop_note_first_measurement(&mut self) {
        if self.loop_ctl.measured_candidates > 0 && self.loop_ctl.first_measured_iteration.is_none()
        {
            self.loop_ctl.first_measured_iteration = Some(self.loop_ctl.iteration);
        }
    }

    /// Iterations since the first measured candidate while no submission has
    /// landed; `None` when the clock is not running.
    fn loop_iters_since_first_measurement(&self) -> Option<usize> {
        let first = self.loop_ctl.first_measured_iteration?;
        (self.loop_ctl.podrace
            && self.loop_ctl.first_submission_iters > 0
            && self.loop_ctl.submissions == 0)
            .then(|| self.loop_ctl.iteration.saturating_sub(first))
    }

    /// Podrace submission clock, first stage: the directive rides first.
    fn loop_submission_overdue(&self) -> bool {
        self.loop_iters_since_first_measurement()
            .is_some_and(|since| since >= self.loop_ctl.first_submission_iters)
    }

    /// Podrace submission clock, second stage: measured but never submitted
    /// for twice the threshold — reinforce the next-action directive.
    fn loop_submission_directive_due(&self) -> bool {
        self.loop_iters_since_first_measurement()
            .is_some_and(|since| since >= self.loop_ctl.first_submission_iters.saturating_mul(2))
    }

    fn loop_steer_submission(&mut self) {
        let since = self.loop_iters_since_first_measurement().unwrap_or(0);
        let reason = format!(
            "{} measured candidate(s) and no submission in the {since} iterations since the first \
             measurement — the board is the instrument; submit the best measured candidate or \
             identify and resolve the exact blocking command",
            self.loop_ctl.measured_candidates
        );
        self.loop_ctl.last_setback = Some(reason.clone());
        self.note(reason);
    }

    /// Preserve the missing-receipt diagnostic and steer a real measurement
    /// without interpreting an unrecognized command as absent execution.
    fn loop_steer_measurement(&mut self) {
        if self.loop_ctl.verifier_failure.is_some() {
            return;
        }
        let diagnostic = self
            .loop_ctl
            .verifier_blocked
            .clone()
            .unwrap_or_else(|| "no verified measured-candidate receipt recorded".to_string());
        let reason = format!(
            "no measured candidate in {} iterations — {diagnostic}; make a bounded benchmark or verification with an explicit result the next action. Preserve any experiment already in flight; a supervised deep worker can own that check while the parent advances distinct fast wins",
            self.loop_ctl.iteration
        );
        self.loop_ctl.last_setback = Some(reason.clone());
        self.note(reason);
    }

    fn loop_escalate_blocked_verifier(&mut self) -> bool {
        let Some(failure) = self.loop_ctl.verifier_failure.clone() else {
            return false;
        };
        let count = self.loop_ctl.blocked_repeat_count;
        let note = format!("loop · verification blocked — {}", failure.diagnostic());
        self.loop_verifier_notice(&note, count);
        // The prompt already delivered this evidence. Do not re-add it as a
        // setback on every continuation (including model-only blocked replies).
        self.loop_ctl.last_setback = None;
        // Repeated blocked iterations produce an escalation record and a
        // recovery instruction. They pause only when the operator explicitly
        // configures ANGEL_LOOP_BLOCKED_REPEAT_LIMIT.
        let limit_explicit = std::env::var_os("ANGEL_LOOP_BLOCKED_REPEAT_LIMIT").is_some();
        let limit = env_usize("ANGEL_LOOP_BLOCKED_REPEAT_LIMIT", 3);
        if limit == 0 || count < limit {
            return false;
        }
        let workspace = self
            .loop_ctl
            .workspace
            .as_deref()
            .unwrap_or_else(|| self.tools.current_workspace());
        let reason = format!(
            "verifier_blocked_repeat: iterations {}–{} ({} identical blocked iterations)\n{}\nrun {} in {}; /loop resume after it exits 0",
            self.loop_ctl.blocked_first_iteration,
            self.loop_ctl.iteration,
            count,
            failure.diagnostic(),
            failure.command,
            crate::secrets::redact_str(&workspace.display().to_string())
        );
        self.loop_ctl.escalations.push(serde_json::json!({
            "kind": "verifier_blocked_repeat", "iteration_start": self.loop_ctl.blocked_first_iteration,
            "iteration_end": self.loop_ctl.iteration, "failure_digest": failure.failure_digest,
            "diagnostic": reason, "ts_ms": now_ms()
        }));
        crate::harness::note_escalation(self.loop_ctl.iteration, "verifier_blocked_repeat");
        if limit_explicit {
            self.loop_finish(LoopStatus::Paused, &reason);
            return true;
        }
        // Degrade + notify + continue: the next prompt carries the concrete instruction, the
        // repeat count restarts so a further run of the same failure escalates again.
        let diag: String = failure.diagnostic().chars().take(400).collect();
        self.loop_ctl.last_setback = Some(format!(
            "verification blocked {count}× identically — do not run that command again. Fix the error it \
             printed or verify another way, then make a bounded measurement with an explicit result the \
             next action. Last failure: {diag}"
        ));
        self.loop_ctl.blocked_repeat_count = 0;
        self.loop_ctl.blocked_prompt_digest = None;
        self.note(format!(
            "loop · verification blocked {count}× identically — escalated to the model, continuing (set ANGEL_LOOP_BLOCKED_REPEAT_LIMIT to pause instead)"
        ));
        false
    }

    /// Pause for a goal-side gate (round budget, blocked, done, paused).
    /// The recovery advice is goal-shaped: raise the objective's budget or
    /// re-arm it, then resume the loop.
    fn loop_pause_for_goal_gate(&mut self, why: &str) {
        self.loop_finish(
            LoopStatus::Paused,
            &format!(
                "{why}; /goal rounds <N> to raise the budget (or /goal resume after a blocker), then /loop resume"
            ),
        );
    }

    /// A done-claim on a `/self` run: run the Layer-2 gate — `cargo build` +
    /// `cargo test` + the pass-count regression guard — in the run's pinned
    /// worktree, off-thread. The gate is hard-coded (not an env-swappable
    /// command): a self-edit's admissibility must not be redefinable by the
    /// model or the environment mid-run.
    pub(crate) fn loop_spawn_self_gate(&mut self) {
        if self.loop_pending.is_some() {
            self.note("loop · existing verifier work must drain before another action".to_string());
            return;
        }
        self.loop_ctl.status = LoopStatus::Verifying;
        self.loop_ctl.awaiting_turn = false;
        self.loop_ctl.pending_acceptance = None;
        if self.loop_ctl.cycle_started_ms.is_none() {
            self.loop_ctl.cycle_started_ms = Some(now_ms());
        }
        self.note("self · running the Layer-2 gate: cargo build + cargo test …".to_string());
        let baseline = self.loop_ctl.baseline_passed;
        // The run's pinned worktree, not the tools' current root — a later
        // `/self integrate` may run after the workspace was restored.
        let workspace = self
            .loop_ctl
            .workspace
            .clone()
            .unwrap_or_else(|| self.tools.current_workspace().to_path_buf());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (v, detail) =
                crate::tools::self_model::run_self_gate_with_baseline(&workspace, baseline);
            let _ = tx.send(VerifyResult {
                passed: v.passed,
                summary: v.summary,
                detail,
            });
        });
        self.loop_pending = Some(LoopPending::Verify(rx));
        save(&self.loop_ctl);
    }

    fn loop_spawn_verify(&mut self, cmd: String) {
        if self.loop_pending.is_some() {
            self.note("loop · existing verifier work must drain before another action".to_string());
            return;
        }
        // Use the loop's pinned workspace. `/cd` detaches project-scoped loop
        // state, but this also keeps verification and its memo key aligned if
        // the live tool registry changes during a callback boundary.
        let workspace = self
            .loop_ctl
            .workspace
            .clone()
            .unwrap_or_else(|| self.tools.current_workspace().to_path_buf());
        match prepare_acceptance_gate(&mut self.loop_ctl, &cmd, &workspace) {
            AcceptanceGatePlan::Replay(result) => {
                self.loop_ctl.pending_acceptance = None;
                self.loop_apply_verify_result(result, true);
                return;
            }
            AcceptanceGatePlan::Execute(run) => {
                self.loop_ctl.pending_acceptance = Some(run);
            }
        }
        self.loop_ctl.status = LoopStatus::Verifying;
        self.loop_ctl.awaiting_turn = false;
        if self.loop_ctl.cycle_started_ms.is_none() {
            self.loop_ctl.cycle_started_ms = Some(now_ms());
        }
        self.note(format!("loop · verifying acceptance: `{cmd}`"));
        let baseline = self.loop_ctl.baseline_passed;
        let subject = format!(
            "loop_id={}:{}\niteration={}\ntask={}:{}",
            self.loop_ctl.id.len(),
            self.loop_ctl.id,
            self.loop_ctl.iteration,
            self.loop_ctl.task.len(),
            self.loop_ctl.task,
        );
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(run_accept_cmd(&cmd, baseline, &workspace, &subject));
        });
        self.loop_pending = Some(LoopPending::Verify(rx));
        save(&self.loop_ctl);
    }

    fn loop_request_sota(&mut self) {
        if self.loop_pending.is_some() {
            self.note("loop · existing verifier work must drain before another action".to_string());
            return;
        }
        if self.pending_approval.is_some() {
            return; // a modal is already up; we'll retry on a later frame
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let seat = self
            .loop_sota_club()
            .map(|c| c.label().to_string())
            .unwrap_or_else(|| "SOTA".to_string());
        let prompt = format!(
            "loop stalled after {} iteration(s) — switch to SOTA seat `{seat}` \
             (ChatGPT-OAuth/Codex or other metered link)? This is a paid mode switch. \
             (y approve · a approve-all · n deny)",
            self.loop_ctl.iteration
        );
        self.pending_approval = Some(crate::PendingApproval {
            prompt,
            scope_label: None,
            reply: tx,
        });
        self.loop_pending = Some(LoopPending::Approval(rx));
        self.loop_ctl.status = LoopStatus::AwaitingApproval;
        self.loop_ctl.awaiting_turn = false;
        self.loop_ctl.cycle_started_ms = None;
        save(&self.loop_ctl);
        self.start_lifecycle_ceremony(
            crate::viz::lifecycle_viz::CeremonyKind::LoopEscalate,
            self.loop_task_text(),
        );
    }

    /// Retire command ownership until terminal send/disconnect. Repeated
    /// stop/finish/detach keep the exact receiver; local decisions own no worker.
    pub(crate) fn loop_retire_pending(&mut self) {
        self.loop_cancel_experiment();
        self.loop_pending = match self.loop_pending.take() {
            Some(LoopPending::Baseline(rx, _)) => Some(LoopPending::RetiredBaseline(rx)),
            Some(LoopPending::Verify(rx)) => Some(LoopPending::RetiredVerify(rx)),
            retired @ Some(LoopPending::RetiredBaseline(_) | LoopPending::RetiredVerify(_)) => {
                retired
            }
            Some(LoopPending::Approval(_) | LoopPending::SelfIntegrate(_)) => {
                if let Some(approval) = self.pending_approval.take() {
                    let _ = approval.reply.send(Decision::Deny);
                }
                None
            }
            None => None,
        };
    }

    /// Retire an in-flight loop iteration while retaining its worker ownership.
    /// The existing advance drain path suppresses late output and releases the
    /// shared provider/tool slot only after terminal send or disconnect.
    fn loop_cancel_inflight(&mut self) {
        self.tools.rl().end_loop();
        self.loop_account_rl();
        if let Some(t) = self.thinking.as_mut() {
            t.begin_draining();
        }
        // Old steers die with their run. Give later operator input a fresh queue:
        // the retiring worker may already be past its cancellation check and
        // must never consume a post-stop follow-up at its next steer drain.
        let old_steers = std::mem::take(&mut self.steer_queue);
        let _ = old_steers.drain();
        self.clear_partial();
        self.loop_ctl.awaiting_turn = false;
        self.loop_ctl.cycle_started_ms = None;
        self.loop_ctl.pending_acceptance = None;
        self.loop_retire_pending();
        self.pending_approval = None;
    }

    /// Detach project-scoped autonomous state before `/cd` crosses a repository
    /// boundary. A live run is persisted as paused under its original project;
    /// nothing from it remains armed in the new project.
    pub(crate) fn loop_detach_for_workspace_change(&mut self) -> bool {
        let had_live_loop = self.loop_ctl.status != LoopStatus::Idle;
        self.loop_cancel_inflight();
        if matches!(
            self.loop_ctl.status,
            LoopStatus::Baselining
                | LoopStatus::Running
                | LoopStatus::Verifying
                | LoopStatus::AwaitingApproval
        ) {
            self.loop_ctl.status = LoopStatus::Paused;
            self.loop_ctl.wake_at = None;
            self.loop_ctl.cycle_started_ms = None;
            save(&self.loop_ctl);
        }
        self.loop_ctl = LoopState::default();
        self.loop_retire_pending();
        self.loop_dialog = None;
        self.loop_dialog_hits.clear();
        had_live_loop
    }

    /// The club that drives the current iteration, by escalation tier:
    /// - `Local` → a reachable local fleet club (in-hand when it is already
    ///   local; otherwise the smartest available non-SOTA seat),
    /// - `Swarm` → that club wrapped in [`SwarmClub`] (local Mixture-of-Agents),
    /// - `Sota`  → the SOTA club if reachable, else the local/in-hand club.
    ///
    /// Deli mode wraps whatever the tier selected in [`DeliClub`], so each
    /// iteration is itself a deep-think-over-time loop.
    fn loop_club(&self) -> Arc<dyn Club> {
        let local = self
            .loop_local_club()
            .unwrap_or_else(|| self.bag.in_hand_with_fallback());
        let base = match self.loop_ctl.tier {
            EscalationTier::Local => local,
            EscalationTier::Swarm => Arc::new(SwarmClub::from_env("swarm", local)),
            EscalationTier::Sota => self.loop_sota_club().unwrap_or(local),
        };
        if self.loop_ctl.deli {
            Arc::new(DeliClub::from_env("deli", base))
        } else {
            base
        }
    }

    fn loop_local_club(&self) -> Option<Arc<dyn Club>> {
        select_loop_local_club(&self.bag)
    }

    fn loop_sota_club(&self) -> Option<Arc<dyn Club>> {
        select_loop_sota_club(&self.bag)
    }

    fn loop_sota_available(&self) -> bool {
        self.loop_sota_club().is_some()
    }

    fn loop_status_text(&self) -> String {
        let st = &self.loop_ctl;
        if st.status == LoopStatus::Idle {
            return "loop: idle — /loop <task> (or set a /goal and /loop) to start".to_string();
        }
        let status = match st.status {
            LoopStatus::Idle => "idle",
            LoopStatus::Baselining => "baselining",
            LoopStatus::Running => "running",
            LoopStatus::Verifying => "verifying",
            LoopStatus::AwaitingApproval => "awaiting SOTA approval",
            LoopStatus::Paused => "paused",
            LoopStatus::Done => "done",
            LoopStatus::Stopped => "stopped",
            LoopStatus::Failed => "failed",
        };
        let what = if st.task.trim().is_empty() {
            self.goal
                .as_ref()
                .map(|g| g.text.trim().to_string())
                .unwrap_or_else(|| "(goal)".to_string())
        } else {
            st.task.clone()
        };
        let iters = if st.max_iters > 0 {
            format!("{}/{}", st.iteration, st.max_iters)
        } else {
            st.iteration.to_string()
        };
        let tier_name = match st.tier {
            EscalationTier::Local => "local",
            EscalationTier::Swarm => "swarm",
            EscalationTier::Sota => "SOTA",
        };
        let tier = if st.deli {
            format!("{tier_name} · deli")
        } else if st.podrace {
            format!("{tier_name} · podrace")
        } else {
            tier_name.to_string()
        };
        let tok_budget = if st.token_budget > 0 {
            st.token_budget.to_string()
        } else {
            "∞".to_string()
        };
        let deadline = if st.deadline_secs > 0 {
            st.deadline_secs.to_string()
        } else {
            "∞".to_string()
        };
        let cycle_line = match cycle_elapsed_secs(st) {
            Some(elapsed) => format!("\n  cycle   {elapsed}s"),
            None if st.status == LoopStatus::Running && st.wake_at.is_some() => {
                "\n  cycle   waiting".to_string()
            }
            None => String::new(),
        };
        let mut out = format!(
            "loop [{status}] → {what}\n  iter     {iters}\n  findings {} verified · {} hypotheses\n  \
             tools    {} calls · {} errors\n  stall    {}/{}\n  tier     {tier}\n  \
             tokens   ~{}/{tok_budget}\n  deadline {deadline}s{cycle_line}",
            st.findings.len(),
            st.hypotheses.len(),
            st.tool_calls_total,
            st.tool_errors_total,
            st.stale_count,
            st.stall_stop,
            st.tokens_spent,
        );
        if !st.steer_notes.is_empty() {
            out.push_str(&format!(
                "\n  steers   {} (ride every iteration)",
                st.steer_notes.len()
            ));
        }
        if st.self_edit {
            out.push_str(&format!(
                "\n  self     branch {} · worktree {}",
                st.self_branch.as_deref().unwrap_or("?"),
                st.workspace
                    .as_deref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            ));
        }
        if let Some(e) = &st.last_error {
            out.push_str(&format!("\n  last err {e}"));
        }
        if let Some(red) = &st.last_failed_acceptance {
            out.push_str(&format!(
                "\n  gate     red · {} attempt(s) on unchanged state",
                red.attempts
            ));
        }
        if let Some(experiment) = st.experiments.last() {
            out.push_str(&format!(
                "\n  deep     {} · {}\n  artifacts {}\n  reserved ~{} tokens (included above)",
                experiment.status,
                experiment.hypothesis,
                experiment.artifact_dir.display(),
                experiment.reserved_tokens,
            ));
        }
        out
    }

    /// Push a system notice into the transcript. (A sibling module can't call
    /// app_control's private `system_msg`, so the loop has its own; the `/self`
    /// teardown shares it.)
    pub(crate) fn note(&mut self, text: impl Into<Arc<str>>) {
        self.messages.push(Message::new(Role::System, text));
    }
}

pub(crate) fn cycle_elapsed_secs(st: &LoopState) -> Option<u64> {
    st.cycle_started_ms
        .map(|started| now_ms().saturating_sub(started) / 1000)
}

fn point_in_rect(x: u16, y: u16, rect: Rect) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

// ---------------------------------------------------------------------------
// Pure state-machine helpers live in `loop_ctl/evidence.rs` (module-breakup
// step 3: evidence admission, reply folding, and budget/stall policy).
// ---------------------------------------------------------------------------

/// Fold one iteration's reply into the state: parse, dedup, update stall / dirs /
/// log / token estimate. Returns the count of fresh findings.
/// The workspace-and-tool-aware evidence check. Strictly stronger than the shared
/// [`has_evidence_tag`](crate::iterate::has_evidence_tag): on top of requiring a
/// well-formed, non-placeholder citation, it resolves file-like sources against
/// the working tree and gates URL/command citations on a tool call that actually
/// succeeded. A text-only driver can't do either, which is why the weaker form
/// exists separately rather than this one being relaxed.
/// `HEAD` of a workspace, `None` outside a git repo. Captured at loop start
/// as the anchor for the per-iteration files-changed diff.
pub(crate) fn git_head(ws: Option<&Path>) -> Option<String> {
    crate::workspace_store::capture_repo_probe(
        "git",
        &["rev-parse", "HEAD"],
        ws.unwrap_or_else(|| Path::new(".")),
        5,
    )
}

/// Whether loop escalation may offer a SOTA-tier seat at all (`ANGEL_LOOP_SOTA`, default on).
fn loop_sota_enabled() -> bool {
    crate::harness::env_flag("ANGEL_LOOP_SOTA", true)
}

/// Whether OpenAI/Codex family seats may be chosen as the loop SOTA club.
/// All configured families are eligible by default. The operator can opt out.
fn loop_sota_allow_openai() -> bool {
    crate::harness::env_flag("ANGEL_LOOP_SOTA_ALLOW_OPENAI", true)
}

/// ChatGPT-OAuth / Codex / GPT product seats (not API-billing classification).
pub(crate) fn is_openai_family_label(label: &str) -> bool {
    let l = label.to_ascii_lowercase();
    l.contains("openai")
        || l.contains("codex")
        || l.contains("chatgpt")
        || l.starts_with("gpt-")
        || l == "gpt"
}

/// Whether a club label is eligible for the loop's Local tier.
/// Practice, swarm/MoA wrappers, and token-metered SOTA seats are out.
fn is_loop_local_label(label: &str) -> bool {
    let low = label.trim().to_ascii_lowercase();
    if low.is_empty() || crate::club::is_logical_wrapper_label(&low) {
        return false;
    }
    !is_sota_label(label)
}

/// Pick the loop's initial-tier club, preserving the operator's selected model.
///
/// Order:
/// 1. `ANGEL_LOOP_LOCAL_CLUB` exact pin (when present and available)
/// 2. In-hand concrete club, including an explicitly selected OAuth model
/// 3. Smartest available non-SOTA, non-practice seat
fn select_loop_local_club(bag: &crate::club::Bag) -> Option<Arc<dyn Club>> {
    let roster = bag.roster();
    let labels: Vec<String> = roster.iter().map(|c| c.label().to_string()).collect();
    let available: Vec<bool> = roster.iter().map(|c| c.is_available()).collect();
    let in_hand = bag.in_hand_with_fallback().label().to_string();
    let pin = std::env::var("ANGEL_LOOP_LOCAL_CLUB")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let chosen = preferred_loop_local_label(&in_hand, &labels, &available, pin.as_deref())?;
    roster
        .into_iter()
        .find(|c| c.label().eq_ignore_ascii_case(&chosen))
}

/// Pure ranking used by [`select_loop_local_club`].
fn preferred_loop_local_label(
    in_hand: &str,
    labels: &[String],
    available: &[bool],
    pin: Option<&str>,
) -> Option<String> {
    debug_assert_eq!(labels.len(), available.len());
    if let Some(pin) = pin
        && let Some((i, label)) = labels
            .iter()
            .enumerate()
            .find(|(_, l)| l.eq_ignore_ascii_case(pin))
        && available.get(i).copied().unwrap_or(false)
    {
        return Some(label.clone());
    }
    if !in_hand.trim().is_empty()
        && !crate::club::is_logical_wrapper_label(in_hand)
        && labels
            .iter()
            .zip(available.iter())
            .any(|(l, a)| l == in_hand && *a)
    {
        return Some(in_hand.to_string());
    }
    labels
        .iter()
        .zip(available.iter())
        .filter(|(_, a)| **a)
        .map(|(l, _)| l.clone())
        .filter(|l| is_loop_local_label(l))
        .max_by_key(|l| model_smartness(l, None, false))
}

/// Pick the loop's SOTA-tier club, preserving the selected model when available.
///
/// Order:
/// 1. `ANGEL_LOOP_SOTA_CLUB` exact pin (when present and available)
/// 2. In-hand club when it is already SOTA-class and allowed
/// 3. Other available SOTA-class seats, honoring any explicit
///    `ANGEL_LOOP_SOTA_ALLOW_OPENAI=0` restriction.
fn select_loop_sota_club(bag: &crate::club::Bag) -> Option<Arc<dyn Club>> {
    if !loop_sota_enabled() {
        return None;
    }
    let allow_openai = loop_sota_allow_openai();
    let roster = bag.roster();
    let labels: Vec<String> = roster.iter().map(|c| c.label().to_string()).collect();
    let available: Vec<bool> = roster.iter().map(|c| c.is_available()).collect();
    let in_hand = bag.in_hand_with_fallback().label().to_string();
    let pin = std::env::var("ANGEL_LOOP_SOTA_CLUB")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let chosen =
        preferred_loop_sota_label(&in_hand, &labels, &available, allow_openai, pin.as_deref())?;
    roster
        .into_iter()
        .find(|c| c.label().eq_ignore_ascii_case(&chosen))
}

/// Pure ranking used by [`select_loop_sota_club`] (testable without a live bag).
fn preferred_loop_sota_label(
    in_hand: &str,
    labels: &[String],
    available: &[bool],
    allow_openai: bool,
    pin: Option<&str>,
) -> Option<String> {
    debug_assert_eq!(labels.len(), available.len());
    if let Some(pin) = pin
        && let Some((i, label)) = labels
            .iter()
            .enumerate()
            .find(|(_, l)| l.eq_ignore_ascii_case(pin))
        && available.get(i).copied().unwrap_or(false)
    {
        return Some(label.clone());
    }

    let hand = in_hand.to_ascii_lowercase();
    let hand_stem = hand
        .split(['-', '·', ' ', '_'])
        .next()
        .unwrap_or("")
        .to_string();

    if is_sota_label(in_hand)
        && !crate::club::is_logical_wrapper_label(in_hand)
        && labels
            .iter()
            .zip(available.iter())
            .any(|(l, a)| l == in_hand && *a)
        && (allow_openai || !is_openai_family_label(in_hand))
    {
        return Some(in_hand.to_string());
    }

    let mut ranked: Vec<(u8, String)> = labels
        .iter()
        .zip(available.iter())
        .filter(|(_, a)| **a)
        .map(|(l, _)| l.clone())
        .filter(|l| is_sota_label(l) && !crate::club::is_logical_wrapper_label(l))
        .filter(|l| allow_openai || !is_openai_family_label(l))
        .map(|l| {
            let low = l.to_ascii_lowercase();
            let affinity = if !hand_stem.is_empty()
                && (low.contains(&hand_stem)
                    || hand.contains(low.split(['-', '·', ' ', '_']).next().unwrap_or("")))
            {
                0u8
            } else {
                1
            };
            (affinity, l)
        })
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    ranked.into_iter().map(|(_, l)| l).next()
}

/// Run the goal's acceptance command and judge it. Ground truth for done-detection
/// — far stronger than the model's "I'm done". When the command emits a libtest
/// summary, it is scored through the LEVI / `reinforce` test reward (with a
/// zero-test guard) rather than the bare exit code; otherwise it falls back to
/// exit status. Lint is not inferred from absent diagnostics in test-only
/// output. Only as strong as the configured command itself.
fn run_accept_cmd(
    cmd: &str,
    baseline_passed: Option<usize>,
    workspace: &Path,
    subject: &str,
) -> VerifyResult {
    let evidence = match EvaluatorEvidence::run_shell(
        "loop acceptance verifier",
        cmd,
        workspace,
        crate::reinforce::TEST_VERIFIER_CONTRACT,
        subject,
    ) {
        Ok(evidence) => evidence,
        Err(e) => {
            return VerifyResult {
                passed: false,
                summary: format!("could not run `{cmd}`: {e}"),
                detail: String::new(),
            };
        }
    };
    let combined = evidence.output();

    // LEVI / RLVR verifiable reward: when there's a test summary, score it densely.
    if combined.contains("test result:") {
        let t = crate::harness::parse_test_result(combined);
        let reward_result = TestReward.score(RewardInput::EvaluatorEvidence(&evidence));
        let (reward, evidence_note) = match reward_result {
            Ok(reward) => (reward, String::new()),
            Err(error) => (0.0, format!(" · REJECTED EVIDENCE: {error}")),
        };
        // Reward-hack guards: a "green" with zero executed tests is not a pass, and
        // a green whose pass-count fell below the pre-edit baseline is a regression
        // (tests deleted / disabled to fake a pass) — reject it.
        let regressed = baseline_passed.is_some_and(|b| t.passed < b);
        let passed = t.passed > 0 && reward >= verify_threshold() && !regressed;
        let note = if regressed {
            format!(
                " · REJECTED: {} passed < baseline {}",
                t.passed,
                baseline_passed.unwrap_or(0)
            )
        } else {
            String::new()
        };
        return VerifyResult {
            passed,
            summary: format!(
                "RLVR reward {reward:.2} · {} passed / {} failed{note}{evidence_note}",
                t.passed, t.failed
            ),
            detail: if passed {
                String::new()
            } else {
                failure_detail(combined)
            },
        };
    }

    // No verifiable signal → exit status.
    let passed = evidence.succeeded();
    let summary = if passed {
        format!("`{cmd}` exit 0")
    } else {
        let code = evidence
            .exit_code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        let tail = combined
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim();
        format!("`{cmd}` exit {code}: {tail}")
    };
    let detail = if passed {
        String::new()
    } else {
        failure_detail(combined)
    };
    VerifyResult {
        passed,
        summary,
        detail,
    }
}

/// The actionable slice of a failed check's output — failing test names,
/// panics, assertions, compiler errors — bounded so it can ride the next
/// iteration's prompt without flooding it. "3 passed / 2 failed" tells the
/// next iteration nothing it can act on; WHICH tests failed and WHY does.
/// Falls back to the output tail when nothing matches (a script that just
/// prints and exits nonzero). Shared with the `/self` Layer-2 gate.
pub(crate) fn failure_detail(combined: &str) -> String {
    const MAX_LINES: usize = 12;
    const MAX_LINE_CHARS: usize = 240;
    let mut seen = HashSet::new();
    let mut picked: Vec<&str> = combined
        .lines()
        .map(str::trim)
        .filter(|l| {
            if l.is_empty() {
                return false;
            }
            let ll = l.to_ascii_lowercase();
            l.contains("FAILED")
                || ll.contains("panicked at")
                || ll.contains("assert")
                || ll.starts_with("error")
        })
        .filter(|l| seen.insert(*l))
        .take(MAX_LINES)
        .collect();
    if picked.is_empty() {
        picked = combined
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .rev()
            .take(MAX_LINES / 2)
            .collect();
        picked.reverse();
    }
    picked
        .into_iter()
        .map(|l| {
            if l.chars().count() > MAX_LINE_CHARS {
                l.chars().take(MAX_LINE_CHARS).collect()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Reward threshold a verifiable check must meet to count as done. `ANGEL_LOOP_VERIFY_THRESHOLD` (default 1.0 = all green).
fn verify_threshold() -> f32 {
    std::env::var("ANGEL_LOOP_VERIFY_THRESHOLD")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1.0)
}

/// Run a command in `dir` and return its passing-test count (the pre-edit
/// baseline for the regression guard). 0 if the command can't run or emits no
/// test summary.
pub(crate) fn count_passed_in(cmd: &str, dir: &Path) -> usize {
    let mut command = std::process::Command::new("sh");
    command.arg("-c").arg(cmd).current_dir(dir);
    let Ok(capture) =
        crate::harness::output_timed_captured(command, crate::harness::tool_timeout())
    else {
        return 0;
    };
    if capture.timed_out || capture.cancelled {
        return 0;
    }
    let mut combined = String::from_utf8_lossy(&capture.output.stdout).into_owned();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&capture.output.stderr));
    crate::harness::parse_test_result(&combined).passed
}

/// Tolerant detection of the model's done sentinel: any line that — stripped of
/// surrounding markdown/punctuation — is `LOOP_DONE` (or `LOOPDONE`). Catches
/// `**LOOP_DONE**`, `LOOP_DONE.`, `- LOOP_DONE`, etc., without matching prose that
/// merely mentions it mid-sentence.
/// Split the leading whitespace-delimited word from the rest of the string.
pub(crate) fn split_word(s: &str) -> (&str, &str) {
    match s.trim().split_once(char::is_whitespace) {
        Some((w, rest)) => (w, rest.trim()),
        None => (s.trim(), ""),
    }
}

/// Split `"<interval> <task…>"`: if the first token parses as an interval, return
/// `(secs, task)`; otherwise the whole string is the task at interval 0.
pub(crate) fn split_interval(s: &str) -> (u64, String) {
    let s = s.trim();
    match s.split_once(char::is_whitespace) {
        Some((first, rest)) => match parse_interval(first) {
            Some(iv) => (iv, rest.trim().to_string()),
            None => (0, s.to_string()),
        },
        None => match parse_interval(s) {
            Some(iv) => (iv, String::new()),
            None => (0, s.to_string()),
        },
    }
}

pub(crate) fn parse_loop_start_options(task: &str) -> (String, Option<usize>) {
    let mut rest = task.trim();
    let mut max_iters = None;
    while let Some((first, tail)) = next_word(rest) {
        let token = first.trim().trim_start_matches('[').trim_end_matches(']');
        let lower = token.to_ascii_lowercase();
        let parsed = if is_endless_word(&lower) {
            Some(0)
        } else {
            lower
                .split_once('=')
                .and_then(|(key, value)| {
                    matches!(key, "iterations" | "iters" | "max" | "max_iters").then_some(value)
                })
                .and_then(|value| {
                    if is_endless_word(value) {
                        Some(0)
                    } else {
                        value.parse::<usize>().ok()
                    }
                })
        };
        let Some(value) = parsed else {
            break;
        };
        max_iters = Some(value);
        rest = tail.trim_start();
    }
    (rest.to_string(), max_iters)
}

pub(crate) fn parse_usize_or_endless(raw: &str) -> Option<usize> {
    let value = raw.trim().to_ascii_lowercase();
    if is_endless_word(&value) {
        Some(0)
    } else {
        value.parse::<usize>().ok()
    }
}

fn is_endless_word(value: &str) -> bool {
    matches!(
        value,
        "0" | "∞" | "endless" | "forever" | "infinite" | "infinity" | "no_limit" | "unlimited"
    )
}

fn next_word(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    match s.find(char::is_whitespace) {
        Some(idx) => Some((&s[..idx], &s[idx..])),
        None => Some((s, "")),
    }
}

/// Parse `30s` / `5m` / `2h` / bare seconds into seconds.
pub(crate) fn parse_interval_secs(tok: &str) -> Option<u64> {
    parse_interval(tok)
}

/// Parse `30s` / `5m` / `2h` / bare seconds into seconds.
fn parse_interval(tok: &str) -> Option<u64> {
    let t = tok.trim();
    if t.is_empty() {
        return None;
    }
    let (num, mult) = if let Some(n) = t.strip_suffix('s') {
        (n, 1)
    } else if let Some(n) = t.strip_suffix('m') {
        (n, 60)
    } else if let Some(n) = t.strip_suffix('h') {
        (n, 3600)
    } else {
        (t, 1)
    };
    // Clamp the interval to a sane maximum (~100 years). An unbounded value both
    // overflows the `n * mult` multiply (panic in debug / wrap in release) and,
    // downstream, panics `Instant::now() + Duration::from_secs(n)` (the Add impl
    // `expect`s on overflow), which would tear down the TUI from a single typed
    // `/loop 99999999999999999999s` command. `saturating_mul` + `.min` keeps the
    // clock arithmetic safe.
    num.trim()
        .parse::<u64>()
        .ok()
        .map(|n| n.saturating_mul(mult).min(MAX_LOOP_INTERVAL_SECS))
}

/// Upper bound on a loop wake interval (~100 years). Far above any real cadence,
/// and comfortably within the range where `Instant + Duration` cannot overflow.
const MAX_LOOP_INTERVAL_SECS: u64 = 100 * 365 * 24 * 3600;

#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/loop_ctl__interval_stress.rs"]
mod interval_stress;

// ---------------------------------------------------------------------------
// Persistence (mirror session.rs: atomic temp + rename; crash-safe)
// ---------------------------------------------------------------------------

#[cfg(test)]
fn store_path() -> PathBuf {
    explicit_loop_file().unwrap_or_else(|| angel_dir().join("loop.json"))
}

#[cfg(test)]
fn store_path_for(workspace: Option<&Path>) -> PathBuf {
    store_path_for_session(workspace, None)
}

fn store_path_for_session(workspace: Option<&Path>, session_id: Option<&str>) -> PathBuf {
    if let Some(path) = explicit_loop_file() {
        return path;
    }
    match workspace {
        Some(workspace) => {
            let identity = crate::workspace_store::repo_identity(workspace);
            if let Some(session_id) = session_id.filter(|id| !id.trim().is_empty()) {
                let session_key = crate::cut::sha256_hex(session_id.as_bytes());
                angel_subdir("loops").join(format!("{}--{session_key}.json", identity.key))
            } else {
                workspace_json_path_in(&angel_subdir("loops"), &identity.root)
            }
        }
        None => angel_dir().join("loop.json"),
    }
}

fn explicit_loop_file() -> Option<PathBuf> {
    std::env::var("ANGEL_LOOP_FILE")
        .ok()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

/// Persist the loop atomically. No-op for an `Idle` loop (avoids stub files).
/// Also mirrors a deli-style `state/` directory for an external watchdog when
/// `ANGEL_LOOP_STATE_DIR` is set — the seam a future sidecar tails for liveness.
pub fn save(st: &LoopState) {
    if st.status == LoopStatus::Idle {
        return;
    }
    let path = store_path_for_session(st.workspace.as_deref(), st.owner_session_id.as_deref());
    let Some(workspace) = st.workspace.as_deref() else {
        return;
    };
    if path.exists() {
        let existing_matches = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<LoopState>(&raw).ok())
            .and_then(|existing| existing.workspace)
            .is_some_and(|root| {
                let stored = crate::workspace_store::repo_identity(&root);
                crate::workspace_store::matches_project(workspace, &stored.root, &stored.key)
            });
        if !existing_matches {
            return;
        }
    }
    let Ok(json) = serde_json::to_string_pretty(st) else {
        return;
    };
    if crate::workspace_store::write_private_atomic(&path, json.as_bytes()).is_err() {
        return;
    }
    // The canonical restart checkpoint owns publication. Never advance the
    // watchdog projection when that checkpoint did not commit.
    persist_state_dir(st);
}

/// Best-effort mirror of the run's state to `ANGEL_LOOP_STATE_DIR/state/` (deli's
/// schema, extended) so an out-of-process watchdog can read progress + liveness.
/// A no-op when the dir is unset; IO failures are swallowed so it can't break a run.
fn persist_state_dir(st: &LoopState) {
    let Ok(base) = std::env::var("ANGEL_LOOP_STATE_DIR") else {
        return;
    };
    if base.trim().is_empty() {
        return;
    }
    let Some(workspace) = st.workspace.as_deref() else {
        return; // unscoped watchdog state could steer an unrelated project
    };
    let identity = crate::workspace_store::repo_identity(workspace);
    let dir = std::path::Path::new(&base)
        .join(&identity.key)
        .join("state");
    let Ok(directory) = crate::workspace_store::private_io::PrivateDirectory::open(&dir) else {
        return;
    };
    let status = serde_json::json!({
        "status": format!("{:?}", st.status).to_lowercase(),
        "tier": format!("{:?}", st.tier).to_lowercase(),
        "podrace": st.podrace,
        "iteration": st.iteration,
        "max_iters": st.max_iters,
        "total_findings": st.findings.len(),
        "total_hypotheses": st.hypotheses.len(),
        "stale_count": st.stale_count,
        "tool_calls_total": st.tool_calls_total,
        "tool_errors_total": st.tool_errors_total,
        "execution_blocker": st.execution_blocker,
        "outcome_actions_seen": st.outcome_actions_seen.len(),
        "workspace_fingerprint": st.last_workspace_fingerprint,
        "tokens_spent": st.tokens_spent,
        "token_budget": st.token_budget,
        "deadline_secs": st.deadline_secs,
        "run_elapsed_secs": now_ms().saturating_sub(st.started_ms) / 1000,
        "cycle_elapsed_secs": cycle_elapsed_secs(st),
        "heartbeat_ms": now_ms(),
        "pid": std::process::id(),
        "project_key": identity.key,
        "workspace": identity.root,
    });
    let _ = directory.replace(
        std::ffi::OsStr::new("progress.json"),
        status.to_string().as_bytes(),
    );
    let acceptance = serde_json::json!({
        "accept_cmd": st.accept_cmd,
        "baseline_passed": st.baseline_passed,
        "last_red": st.last_failed_acceptance.as_ref().map(|red| serde_json::json!({
            "attempts": red.attempts,
            "workspace_fingerprint": red.workspace_fingerprint,
            "summary": red.summary,
            "detail": red.detail,
        })),
    });
    let _ = directory.replace(
        std::ffi::OsStr::new("acceptance.json"),
        acceptance.to_string().as_bytes(),
    );
    sync_jsonl(
        &directory,
        std::ffi::OsStr::new("findings.jsonl"),
        &st.findings,
        &st.persisted_findings,
        |f| serde_json::json!({ "finding": f }).to_string(),
    );
    sync_jsonl(
        &directory,
        std::ffi::OsStr::new("iteration_log.jsonl"),
        &st.log,
        &st.persisted_log,
        |l| {
            serde_json::json!({
                "iteration": l.iteration,
                "direction": l.direction,
                "new_findings": l.new_findings,
                "reported_findings": l.reported_findings,
                "unverified_findings": l.unverified_findings,
                "tool_calls": l.tool_calls,
                "tool_errors": l.tool_errors,
                "duplicate_costly_actions": l.duplicate_costly_actions,
                "outcome_progress": l.outcome_progress,
                "novel_outcome_actions": l.novel_outcome_actions,
                "workspace_changed": l.workspace_changed,
                "evidence_review": l.evidence_review,
                "stale_count": l.stale_count,
                "ts_ms": l.ts_ms,
            })
            .to_string()
        },
    );
}

/// Mirror a push-only Vec as JSONL, appending only the rows not yet written
/// this process-lifetime — a full rewrite per save made an endless run's disk
/// cost O(n²). A zero counter (fresh start or resume) rewrites the file so a
/// prior run's stale mirror can't linger; a vanished file falls back to a
/// rewrite. Best-effort like the rest of the watchdog mirror: IO failures
/// leave the counter unchanged so the next save retries the same tail.
fn sync_jsonl<T>(
    directory: &crate::workspace_store::private_io::PrivateDirectory,
    name: &std::ffi::OsStr,
    rows: &[T],
    written: &std::cell::Cell<usize>,
    mut render: impl FnMut(&T) -> String,
) {
    use std::io::Write;
    let mut done = written.get().min(rows.len());
    if done == rows.len() && done > 0 {
        return;
    }
    let Ok(previous) = directory.existing(name) else {
        return;
    };
    if done > 0 && previous.is_none() {
        done = 0;
    }
    let mut buf = String::new();
    for (i, row) in rows[done..].iter().enumerate() {
        if done + i > 0 {
            buf.push('\n');
        }
        buf.push_str(&render(row));
    }
    let result = if done == 0 {
        // Initial/resumed mirrors replace stale rows atomically; new files are
        // private before the first byte and no preexisting entry is truncated.
        directory.replace(name, buf.as_bytes())
    } else {
        directory
            .append(name)
            .and_then(|mut file| file.write_all(buf.as_bytes()))
    };
    if result.is_ok() {
        written.set(rows.len());
    }
}

/// Load a saved loop. Rebuilds runtime ledgers and clears in-flight flags.
/// Previously running/verifying campaigns re-arm under the existing budget
/// and task gates. Explicit pauses/stops and interrupted approvals stay parked.
#[cfg(test)]
pub fn load() -> Option<LoopState> {
    load_from_path(store_path())
}

#[cfg(test)]
pub fn load_for(workspace: &Path) -> Option<LoopState> {
    let expected = crate::workspace_store::repo_identity(workspace);
    let state = load_from_path(store_path_for(Some(workspace)))?;
    let stored_workspace = state.workspace.as_deref()?;
    let stored = crate::workspace_store::repo_identity(stored_workspace);
    (stored.key == expected.key && stored.root == expected.root).then_some(state)
}

/// Load only the loop checkpoint owned by this cockpit session. Default storage
/// is keyed by both repository and session; `ANGEL_LOOP_FILE` remains an
/// explicit single-file override for tests and operator-directed recovery.
pub fn load_for_session(workspace: &Path, session_id: &str) -> Option<LoopState> {
    let expected = crate::workspace_store::repo_identity(workspace);
    let explicit = explicit_loop_file().is_some();
    let state = load_from_path(store_path_for_session(Some(workspace), Some(session_id)))?;
    let stored_workspace = state.workspace.as_deref()?;
    let stored = crate::workspace_store::repo_identity(stored_workspace);
    if stored.key != expected.key || stored.root != expected.root {
        return None;
    }
    (explicit || state.owner_session_id.as_deref() == Some(session_id)).then_some(state)
}

fn load_from_path(path: PathBuf) -> Option<LoopState> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut st: LoopState = serde_json::from_str(&raw).ok()?;
    // Legacy snapshots cannot distinguish inherited defaults from operator choices.
    // Do not infer authority to stop a run from those ambiguous values.
    if !st.operator_caps {
        st.paused_for_cap |= st.status == LoopStatus::Paused && budget_tripped(&st).is_some();
        st.clear_caps();
    }
    sanitize_loaded_acceptance_memo(&mut st);
    st.seen = st
        .findings
        .iter()
        .map(|f| normalize(f))
        .filter(|k| !k.is_empty())
        .collect();
    st.seen_hypotheses = st
        .hypotheses
        .iter()
        .map(|f| normalize(finding_claim(f)))
        .filter(|k| !k.is_empty())
        .collect();
    st.wake_at = None;
    st.awaiting_turn = false;
    if st.status == LoopStatus::Baselining {
        // Old files did not persist whether a baseline was started while paused.
        // Recapture the predicate, then park unless active intent was recorded.
        st.baseline_resume_to.get_or_insert(LoopStatus::Paused);
        st.cycle_started_ms = None;
        let delayed = st.last_error.as_deref() == Some("baseline worker died");
        st.wake_at = Some(Instant::now() + Duration::from_secs(if delayed { 60 } else { 0 }));
    } else if matches!(st.status, LoopStatus::Running | LoopStatus::Verifying) {
        st.status = LoopStatus::Running;
        st.cycle_started_ms = None;
        // Restart does not erase a retained blocker or turn it into an
        // immediate paid retry. Task/goal and budget checks still run in arm.
        let blocked = st.retry_after_error
            || st.execution_blocker.is_some()
            || st
                .last_error
                .as_deref()
                .is_some_and(|error| error.starts_with("goal durability checkpoint failed ("))
            || st.last_error.as_deref() == Some("verify worker died")
            || st
                .last_error
                .as_deref()
                .is_some_and(crate::club::error_requires_provider_action);
        let delay = if blocked {
            st.interval_secs.clamp(60, MAX_LOOP_INTERVAL_SECS)
        } else {
            0
        };
        st.wake_at = Some(
            Instant::now()
                .checked_add(Duration::from_secs(delay))
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600)),
        );
    } else if st.status == LoopStatus::AwaitingApproval {
        // A missing approval receiver is not consent to SOTA or integration.
        st.status = LoopStatus::Paused;
        st.cycle_started_ms = None;
    }
    Some(st)
}

fn sanitize_loaded_acceptance_memo(st: &mut LoopState) {
    if st.last_failed_acceptance.is_none() {
        return;
    }
    let Some(command) = st.accept_cmd.as_deref() else {
        st.last_failed_acceptance = None;
        return;
    };
    let config_identity = acceptance_config_identity(st.baseline_passed);
    let workspace_fingerprint = st
        .workspace
        .as_deref()
        .and_then(acceptance_workspace_fingerprint);
    let valid = st.last_failed_acceptance.as_ref().is_some_and(|memo| {
        acceptance_memo_matches(
            memo,
            command,
            &config_identity,
            workspace_fingerprint.as_deref(),
        )
    });
    if !valid {
        st.last_failed_acceptance = None;
        return;
    }
    if let Some(memo) = st.last_failed_acceptance.as_mut() {
        memo.summary = clip_acceptance_receipt(&memo.summary, ACCEPTANCE_RED_MEMO_SUMMARY_CHARS);
        memo.detail = clip_acceptance_receipt(&memo.detail, ACCEPTANCE_RED_MEMO_DETAIL_CHARS);
        memo.attempts = memo.attempts.clamp(1, ACCEPTANCE_RED_MEMO_MAX_ATTEMPTS);
    }
}

fn clear_file_for(workspace: Option<&Path>, session_id: Option<&str>) {
    let Some(workspace) = workspace else {
        return;
    };
    let path = store_path_for_session(Some(workspace), session_id);
    let owned = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<LoopState>(&raw).ok())
        .and_then(|state| state.workspace)
        .is_some_and(|root| {
            let stored = crate::workspace_store::repo_identity(&root);
            crate::workspace_store::matches_project(workspace, &stored.root, &stored.key)
        });
    if owned {
        let _ = crate::workspace_store::remove_durable(&path);
    }
}

fn gen_id() -> String {
    format!("{:013}-{}", now_ms(), std::process::id())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/loop_ctl__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/autonomy_tests.rs"]
mod autonomy_tests;

#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/baseline_recovery_tests.rs"]
mod baseline_recovery_tests;

#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/retry_tests.rs"]
mod retry_tests;

#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/retry_recovery_tests.rs"]
mod retry_recovery_tests;

#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/cancel_tests.rs"]
mod cancel_tests;

#[cfg(test)]
#[path = "../../tests/cockpit/loop_ctl/private_io_tests.rs"]
mod private_io_tests;
