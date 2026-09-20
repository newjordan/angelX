//! RL campaign controller for the Realm/Reinforce stage.
//!
//! One production workflow, built on the existing technical learning path:
//!
//! * the **objective** is the operator's configured task plus the verifiable
//!   check that defines done (`/goal <task>` + `/goal cmd <check>`, or explicit
//!   `--task`/`--verify`, or repeatable `--case`), with an optional,
//!   independently authored `--audit` case;
//! * the source under test is **frozen once** per campaign, so every sample
//!   starts from the same bytes and the case has a stable identity;
//! * every **sample** is a real agent attempt on the currently selected club
//!   (`bag.in_hand()`), run in an isolated copy by
//!   [`crate::harness::run_loop_experiment`], with the candidate policy note
//!   applied to that attempt's system prompt;
//! * the **measurement** belongs to the evaluator: [`ObjectiveCaseEvaluator`]
//!   runs the operator's own verifier over the candidate's working copy under
//!   evaluator-owned execution and issues the receipts
//!   ([`crate::reinforce::promotion::TechnicalReward::ObjectivePass`]);
//! * the **gate** is `run_reinforce` — a receipt-backed promotion cohort plus a
//!   veto-only final audit over a manifest-disjoint case, and a
//!   `ReleaseCandidate` only when the audit approves it;
//! * an approved release is installed through [`crate::continual_harness`]
//!   (`EntryKind::Prompt`, id [`RL_POLICY_ENTRY_ID`]) so later ordinary and
//!   headless turns consume it, with `/refine rollback <event>` as the rollback
//!   handle and the run record as provenance.
//!
//! Without an `--audit` case the same machinery runs the measured promotion
//! cohort as **exploration**: a candidate policy is proposed, measured and
//! recorded, but never installed as validated learning, and the run says which
//! input is missing. Nothing here fabricates a task, invents progress, or
//! installs a policy that an approved audit did not clear.

use crate::club::{ChatMsg, Club, ClubReply, RouteIdentity};
use crate::continual_harness::{EntryKind, RefinementEdit, Scope};
use crate::reinforce::objective_case::ObjectiveCaseEvaluator;
use crate::reinforce::promotion::{
    CohortManifest, CohortRole, PromotionConfig, PromotionReport, ReceiptPromotionRequest,
    TechnicalHeldoutCase, TechnicalReward, evaluate_promotion_with_receipts,
};
use crate::reinforce::{
    Candidate, Generator, Reflector, ReinforceConfig, ReinforceRequest, Reward, RewardInput,
    TechnicalCampaignAuthority, TechnicalFinalAuditCohort, TechnicalPromotionCohort,
    TechnicalReinforceCampaign, run_reinforce,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod loop_campaign;
mod research;
mod research_bridge;
pub(crate) use loop_campaign::LoopCampaignContext;

/// Learned-policy entry in the workspace's continual-harness store. Later
/// ordinary and headless turns consume it through
/// [`crate::continual_harness::context_block`].
pub(crate) const RL_POLICY_ENTRY_ID: &str = "rl-policy";
const RL_POLICY_TITLE_PREFIX: &str = "RL policy v";

const DEFAULT_ROUNDS: usize = 1;
const DEFAULT_GROUP: usize = 3;
const DEFAULT_SAMPLES: usize = 2;
const LOG_TAIL: usize = 8;
const POINT_CAP: usize = 2_048;
const RUN_SCHEMA: &str = "angel.rl-campaign/v1";
/// Technical-release cohorts require a floor of at least 0.80; a command-success
/// objective is binary per sample, so the floor is where it belongs.
const OBJECTIVE_ABSOLUTE_FLOOR: f32 = 0.80;

fn objective_promotion_config(samples: usize, cases: usize) -> PromotionConfig {
    PromotionConfig {
        min_cases: cases.max(1),
        samples_per_case: samples,
        absolute_floor: OBJECTIVE_ABSOLUTE_FLOOR,
        min_mean_delta: 0.05,
        max_case_regression: 0.0,
        confidence_z: 1.0,
    }
}

fn reinforce_config(group: usize) -> ReinforceConfig {
    ReinforceConfig {
        group_size: group,
        // Every sample is a real agent attempt; do not dispatch extra attempts
        // only to prune them for latency.
        oversample: 0.0,
        staleness_budget: 1,
        // Timing-based pruning is disabled for this path: a thorough coding
        // attempt is never thrown away for having taken longer. Keep its actual
        // latency in the measured record.
        straggler_factor: 0.0,
        success_threshold: 1.0,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RlMode {
    #[default]
    Idle,
    Campaign,
}

/// One measured attempt: reward is the operator's verifier verdict on that
/// attempt's work, and `latency_ms` is that attempt's real cost —
/// generation time (measured around the attempt) plus the physical verifier's
/// wall time for the same sample. Both come from recorded measurements; nothing
/// here is synthesised, rounded up to a floor, or guessed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RunPoint {
    pub step: usize,
    pub reward: f32,
    pub latency_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RunProgress {
    pub points: Vec<RunPoint>,
    /// The objective this campaign was launched with, exactly as the operator
    /// spelled it. The stage and `/rl status` show it, so an expensive run is
    /// auditable while it is still going.
    pub objective: Option<LaunchedObjective>,
    pub planned_attempts: usize,
    pub log_tail: VecDeque<String>,
    pub outcome: Option<Result<CampaignOutcome, String>>,
    pub rounds_done: usize,
    pub rounds_planned: usize,
    pub passed: usize,
    pub red: usize,
    pub promoted: bool,
    pub policy_version: u64,
    observed: usize,
    best_reward: Option<f32>,
}

impl RunProgress {
    fn record_point(&mut self, point: RunPoint) {
        self.observed = self.observed.saturating_add(1);
        self.best_reward = Some(
            self.best_reward
                .map_or(point.reward, |best| best.max(point.reward)),
        );
        if point.reward >= 1.0 {
            self.passed = self.passed.saturating_add(1);
        } else {
            self.red = self.red.saturating_add(1);
        }
        if self.points.len() >= POINT_CAP {
            self.points = self
                .points
                .iter()
                .enumerate()
                .filter_map(|(index, point)| (index % 2 == 0).then_some(*point))
                .collect();
        }
        self.points.push(point);
    }

    fn note(&mut self, line: String) {
        self.log_tail.push_back(line);
        while self.log_tail.len() > LOG_TAIL {
            self.log_tail.pop_front();
        }
    }

    pub(crate) fn observed_attempts(&self) -> usize {
        self.observed.max(self.points.len())
    }

    /// Rebuild the curve from a durable record (the `/rl` stage opening on real
    /// history) without inventing a per-attempt count.
    pub(crate) fn replace_points(&mut self, points: Vec<RunPoint>) {
        self.points.clear();
        self.observed = 0;
        self.best_reward = None;
        self.passed = 0;
        self.red = 0;
        for point in points {
            self.record_point(point);
        }
    }

    pub(crate) fn best_reward(&self) -> Option<f32> {
        self.best_reward.or_else(|| {
            self.points
                .iter()
                .map(|point| point.reward)
                .reduce(f32::max)
        })
    }
}

/// What the campaign actually produced. Every field is a measurement or a
/// durable artifact path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct CampaignOutcome {
    pub attempted: usize,
    pub passed: usize,
    pub red: usize,
    pub rounds: usize,
    pub promoted_rounds: usize,
    pub policy_version: u64,
    pub decision: String,
    pub mean_delta: Option<f32>,
    /// True when an independently authored audit case approved a release.
    pub validated: bool,
    /// Whether an independently authored audit case was supplied at all.
    pub audit_supplied: bool,
    pub release_sha256: Option<String>,
    pub solve_rate: Option<f32>,
    pub advantage_variance: Option<f32>,
    pub reflection: bool,
    /// Continual-harness entry holding the installed policy note.
    pub accepted_entry: Option<String>,
    /// Refinement event recording the install (`/refine rollback <event>`).
    pub accepted_event: Option<String>,
    pub report_path: String,
    pub route: RouteIdentity,
    pub wall_s: f32,
}

/// What a running campaign was launched with: the selection cases, the audit
/// cases and the declared verifier scope, all verbatim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LaunchedObjective {
    pub cases: Vec<RlCase>,
    pub audit: Vec<RlCase>,
    pub verifier_scope: Vec<String>,
}

/// One case: a task and the verifiable check that measures it. Selection cases
/// run over the campaign workspace; an audit case names its own source scope so
/// the audit is a genuinely independent objective rather than a relabelled
/// repeat of the selection case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RlCase {
    pub id: String,
    pub task: String,
    pub verify: String,
    pub source: Option<PathBuf>,
}

/// Operator-selected campaign shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RlPlan {
    pub cases: Vec<RlCase>,
    /// Independently authored audit case(s). Empty = measured exploration only.
    pub audit: Vec<RlCase>,
    /// Verifier-owned inputs (relative to each case's frozen source). The
    /// candidate may author code and tests outside these paths, but it may not
    /// modify what judges it — which is what makes a passing verifier evidence
    /// rather than a claim the candidate authored about itself.
    pub verifier_scope: Vec<String>,
    pub rounds: usize,
    pub group: usize,
    pub samples: usize,
}

impl RlPlan {
    /// Parse the operator's raw `/rl run` command line: quoting is resolved by
    /// [`split_command_args`] first, so a multiword `--task` or `--verify` (or an
    /// audit source path with spaces) reaches the parser exactly as typed.
    pub(crate) fn parse_command_line(workspace: &Path, raw: &str) -> Result<Self, String> {
        let args = split_command_args(raw)?;
        Self::parse(workspace, &args)
    }

    /// Parse already-split `/rl run` arguments, falling back to the workspace's
    /// configured objective. Never invents a task: when nothing is configured
    /// the error names exactly what the operator has to supply.
    pub(crate) fn parse(workspace: &Path, args: &[String]) -> Result<Self, String> {
        let mut task: Option<String> = None;
        let mut verify: Option<String> = None;
        let mut explicit: Vec<RlCase> = Vec::new();
        let mut audit: Vec<RlCase> = Vec::new();
        let mut verifier_scope: Vec<String> = Vec::new();
        let mut rounds = DEFAULT_ROUNDS;
        let mut group = DEFAULT_GROUP;
        let mut samples = DEFAULT_SAMPLES;
        let mut words = args.iter();
        while let Some(word) = words.next() {
            match word.as_str() {
                "--rounds" => rounds = parse_size(words.next(), "--rounds")?,
                "--group" => group = parse_size(words.next(), "--group")?,
                "--samples" => samples = parse_size(words.next(), "--samples")?,
                "--task" => task = Some(required(words.next(), "--task")?),
                "--verify" => verify = Some(required(words.next(), "--verify")?),
                "--case" => {
                    let raw = required(words.next(), "--case")?;
                    explicit.push(parse_case(&raw, "case", explicit.len() + 1)?);
                }
                "--audit" => {
                    let raw = required(words.next(), "--audit")?;
                    audit.push(parse_audit_case(&raw, audit.len() + 1)?);
                }
                "--verify-scope" => {
                    let raw = required(words.next(), "--verify-scope")?;
                    if raw.starts_with('/') || raw.contains("..") {
                        return Err(format!(
                            "--verify-scope takes a path inside the case source, got {raw:?}"
                        ));
                    }
                    verifier_scope.push(raw.trim_end_matches('/').to_string());
                }
                "--help" | "-h" => return Err(Self::usage().to_string()),
                other => {
                    return Err(format!(
                        "unknown /rl run argument {other:?}\n{}",
                        Self::usage()
                    ));
                }
            }
        }
        let mut cases = explicit;
        match (task, verify) {
            (Some(task), Some(verify)) => cases.insert(
                0,
                RlCase {
                    id: "objective".to_string(),
                    task,
                    verify,
                    source: None,
                },
            ),
            (None, None) => {}
            (Some(_), None) => {
                return Err(
                    "--task needs the check that defines done — add --verify \"<command>\"".into(),
                );
            }
            (None, Some(_)) => {
                return Err(
                    "--verify needs the task it checks — add --task \"<objective>\"".into(),
                );
            }
        }
        if cases.is_empty() {
            cases = configured_cases(workspace)?;
        }
        for (index, case) in audit.iter().enumerate() {
            if cases.iter().any(|selection| selection.task == case.task) {
                return Err(format!(
                    "audit case {} repeats a selection task — an audit must be an independently authored objective",
                    index + 1
                ));
            }
        }
        if !audit.is_empty() && verifier_scope.is_empty() {
            return Err(
                "a releasable audit needs the verifier's own inputs: pass --verify-scope <path> \
                 (repeatable) for the files and directories the verifier owns, so a candidate \
                 cannot rewrite the thing that judges it. Without it a run stays measured \
                 exploration."
                    .to_string(),
            );
        }
        Ok(Self {
            cases,
            audit,
            verifier_scope,
            rounds,
            group,
            samples,
        })
    }

    pub(crate) fn usage() -> &'static str {
        "usage: /rl run [--rounds N] [--group N] [--samples N] [--task \"<objective>\" --verify \"<command>\"]\n\
         \x20      [--case \"<objective> :: <command>\"]…  selection cases (repeatable)\n\
         \x20      [--verify-scope <path>]…  verifier-owned inputs (required for --audit)\n\
         \x20      [--audit \"<objective> :: <command> :: <source path>\"]…  independent audit\n\
         \x20      with no arguments the workspace's configured objective is used\n\
         \x20      (/goal <task> plus /goal cmd <check>); without an audit case the run is\n\
         \x20      measured exploration and installs no learned policy"
    }

    pub(crate) fn planned_attempts(&self) -> usize {
        let arms = self.rounds.saturating_mul(self.group).saturating_add(
            self.rounds
                .saturating_mul(2)
                .saturating_mul(self.samples)
                .saturating_mul(self.cases.len()),
        );
        arms.saturating_add(
            self.rounds
                .saturating_mul(2)
                .saturating_mul(self.samples)
                .saturating_mul(self.audit.len()),
        )
    }
}

/// Operator-selected shape: any positive value is accepted. Only zero is
/// rejected, because a campaign with no rounds or no samples is not a smaller
/// campaign, it is an undefined one.
/// Split a `/rl run` command line into arguments without running anything.
///
/// The rules are deliberately small and lossless:
/// * whitespace separates arguments outside quotes;
/// * single quotes hold their contents literally until the next single quote;
/// * double quotes hold their contents literally, honouring an escaped quote and
///   an escaped backslash;
/// * outside quotes a backslash escapes the next character, so a path with a
///   space can also be written escaped;
/// * nothing is expanded, globbed, substituted, comment-stripped or rewritten —
///   a verifier command keeps its inner shell syntax intact;
/// * an unterminated quote is an error the operator can act on, raised before
///   anything is frozen or launched.
pub(crate) fn split_command_args(raw: &str) -> Result<Vec<String>, String> {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = raw.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(other) => word.push(other),
                        None => {
                            return Err("unterminated single quote in the /rl run arguments — \
                                        close it (or write it escaped) to launch"
                                .to_string());
                        }
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\\') => match chars.next() {
                            Some('"') => word.push('"'),
                            Some('\\') => word.push('\\'),
                            Some(other) => {
                                word.push('\\');
                                word.push(other);
                            }
                            None => {
                                return Err("unterminated double quote in the /rl run \
                                            arguments — close it (or write it escaped) to launch"
                                    .to_string());
                            }
                        },
                        Some('"') => break,
                        Some(other) => word.push(other),
                        None => {
                            return Err("unterminated double quote in the /rl run arguments — \
                                        close it (or write it escaped) to launch"
                                .to_string());
                        }
                    }
                }
            }
            '\\' => {
                in_word = true;
                match chars.next() {
                    Some(other) => word.push(other),
                    None => {
                        return Err("the /rl run arguments end with a backslash — escape a \
                                    character or drop it to launch"
                            .to_string());
                    }
                }
            }
            other if other.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            other => {
                in_word = true;
                word.push(other);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

/// Split the command verb (`run`, `stop`, `status`) from the rest of the line
/// without disturbing the remainder's quoting.
pub(crate) fn split_command_verb(raw: &str) -> (&str, &str) {
    let trimmed = raw.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(at) => (&trimmed[..at], trimmed[at..].trim_start()),
        None => (trimmed, ""),
    }
}

fn parse_size(raw: Option<&String>, flag: &str) -> Result<usize, String> {
    let raw = required(raw, flag)?;
    let value: usize = raw
        .parse()
        .map_err(|_| format!("{flag} needs a whole number, got {raw:?}"))?;
    if value == 0 {
        return Err(format!("{flag} must be at least 1"));
    }
    Ok(value)
}

fn required(raw: Option<&String>, flag: &str) -> Result<String, String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_case(raw: &str, prefix: &str, ordinal: usize) -> Result<RlCase, String> {
    let (task, verify) = raw.split_once("::").ok_or_else(|| {
        format!(
            "--{prefix} takes \"<objective> :: <command>\" so the task and its check stay distinct"
        )
    })?;
    let task = task.trim();
    let verify = verify.trim();
    if task.is_empty() || verify.is_empty() {
        return Err(format!(
            "--{prefix} needs both an objective and its verifiable check"
        ));
    }
    Ok(RlCase {
        id: format!("{prefix}-{ordinal}"),
        task: task.to_string(),
        verify: verify.to_string(),
        source: None,
    })
}

/// An audit case must name its own source scope: the release gate refuses an
/// audit whose frozen source, verifier or inventory identity overlaps the
/// selection cohort, so a repeat of the same objective can never be presented
/// as independent generalization.
fn parse_audit_case(raw: &str, ordinal: usize) -> Result<RlCase, String> {
    let mut parts = raw.splitn(3, "::");
    let task = parts.next().map(str::trim).unwrap_or_default();
    let verify = parts.next().map(str::trim).unwrap_or_default();
    let source = parts.next().map(str::trim).unwrap_or_default();
    if task.is_empty() || verify.is_empty() || source.is_empty() {
        return Err(
            "--audit takes \"<objective> :: <command> :: <source path>\" — an audit is an \
             independently authored objective over its own source scope"
                .to_string(),
        );
    }
    let source = PathBuf::from(source);
    if !source.is_dir() {
        return Err(format!(
            "audit source {} is not a directory",
            source.display()
        ));
    }
    Ok(RlCase {
        id: format!("audit-{ordinal}"),
        task: task.to_string(),
        verify: verify.to_string(),
        source: Some(source),
    })
}

fn configured_cases(workspace: &Path) -> Result<Vec<RlCase>, String> {
    const MISSING: &str = "no objective is configured for this workspace — set one with `/goal <task>` and the check that defines done (`/goal cmd <command>`), or pass `--task`/`--verify` (or repeatable `--case \"<objective> :: <command>\"`)";
    let goal = crate::goal::load_for(workspace).ok_or_else(|| MISSING.to_string())?;
    if goal.status != crate::goal::GoalStatus::Active {
        return Err(format!(
            "the configured objective is {:?} — `/goal resume` it, or pass --task/--verify for a different objective",
            goal.status
        ));
    }
    let task = goal.text.trim();
    if task.is_empty() {
        return Err("the configured objective has no task text — `/goal <task>`".into());
    }
    let verify = goal
        .accept_cmd
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .ok_or_else(|| {
            "the configured objective has no verifiable check — add one with `/goal cmd <command>`, or pass --verify \"<command>\""
                .to_string()
        })?;
    Ok(vec![RlCase {
        id: "objective".to_string(),
        task: task.to_string(),
        verify: verify.to_string(),
        source: None,
    }])
}

// ---------------------------------------------------------------------------
// Generator: the selected club does the real work
// ---------------------------------------------------------------------------

/// Prefix marking a candidate receipt; the case evaluator resolves the attempt
/// directory from it and refuses anything outside the campaign's attempt root.
const ATTEMPT_MARKER: &str = "attempt ";

struct CodingPolicyGenerator {
    club: Arc<dyn Club>,
    fixtures: BTreeMap<String, PathBuf>,
    attempts_root: PathBuf,
    progress: Arc<Mutex<RunProgress>>,
    timings: TimingBook,
    cancel: Arc<AtomicBool>,
    seq: AtomicUsize,
}

impl CodingPolicyGenerator {
    fn failure(reason: impl Into<String>) -> Candidate {
        Candidate {
            policy_version: 0,
            latency: Duration::ZERO,
            output: format!("(generation error: {})", reason.into()),
        }
    }

    fn attempt(&self, policy_note: &str, task: &str, version: u64) -> Candidate {
        let started = Instant::now();
        if self.cancel.load(Ordering::Acquire) {
            return Self::failure("campaign cancelled");
        }
        let step = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
        let Some(fixture) = self.fixtures.get(task) else {
            return Self::failure("no frozen source is bound to this task");
        };
        let attempt = self
            .attempts_root
            .join(format!("attempt-{step:04}-v{version}"));
        // Attempts start from the frozen case source: the case identity in the
        // receipt describes exactly the bytes the work began from.
        let request = crate::harness::LoopExperimentRequest {
            workspace: fixture.clone(),
            artifact_dir: attempt.clone(),
            task: task.to_string(),
            // Productive work is uncapped by default; `/rl stop` is the bound.
            max_hops: 0,
            deadline_secs: 0,
            token_budget: 0,
            // The evaluator owns measurement; the attempt only produces work.
            verify_command: None,
            policy_note: Some(policy_note.to_string()).filter(|note| !note.trim().is_empty()),
        };
        let measured = crate::harness::run_loop_experiment(
            request,
            Arc::clone(&self.club),
            Arc::clone(&self.cancel),
        );
        let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match measured {
            Ok(result) => {
                if let Ok(mut progress) = self.progress.lock() {
                    progress.note(format!(
                        "attempt {step} · {} · patch {} · {}ms",
                        result.stop_reason,
                        result
                            .patch_sha256
                            .as_deref()
                            .map(|sha| sha[..sha.len().min(8)].to_string())
                            .unwrap_or_else(|| "none".to_string()),
                        latency_ms
                    ));
                }
                let patch = result
                    .patch_sha256
                    .as_deref()
                    .map(|sha| sha[..sha.len().min(12)].to_string())
                    .unwrap_or_else(|| "none".to_string());
                let output = format!(
                    "{ATTEMPT_MARKER}{} · patch {patch} · stop {}\n{}",
                    attempt.display(),
                    result.stop_reason,
                    crate::reinforce::objective_case::attempt_evidence_pack(&attempt)
                );
                if let Ok(mut timings) = self.timings.lock() {
                    timings.insert(receipt_key(&output), latency_ms);
                }
                Candidate {
                    policy_version: version,
                    // Real generation time for this attempt, measured here and
                    // carried by the engine (never a placeholder zero).
                    latency: Duration::from_millis(latency_ms),
                    output,
                }
            }
            Err(reason) => Self::failure(reason),
        }
    }
}

impl Generator for CodingPolicyGenerator {
    fn generate(&self, system_prompt: &str, task: &str, version: u64, n: usize) -> Vec<Candidate> {
        (0..n)
            .map(|_| self.attempt(system_prompt, task, version))
            .collect()
    }
}

/// A cancelled campaign reports the operator's stop, never a child's own
/// wording for having been interrupted.
fn cancellation_aware<T>(result: Result<T, String>, cancel: &AtomicBool) -> Result<T, String> {
    match result {
        Err(_) if cancel.load(Ordering::Acquire) => Err("stopped by operator".to_string()),
        other => other,
    }
}

/// One attempt's measured cost for the stage: the recorded generation time plus
/// the recorded verifier wall time. A generation time that was not recorded is
/// reported as missing, never counted as zero work.
fn attempt_cost_ms(generation_ms: Option<u64>, verification_ms: Option<u64>) -> u64 {
    generation_ms
        .unwrap_or(0)
        .saturating_add(verification_ms.unwrap_or(0))
}

/// Generation time recorded per attempt, keyed by [`receipt_key`]. The engine
/// hands a reward only the candidate's text, so this in-process book keeps the
/// generator's own measurement available when the training batch is scored. It
/// is the same attempt identity the receipt already binds, not a second record.
type TimingBook = Arc<Mutex<BTreeMap<String, u64>>>;

/// Key that ties a candidate receipt back to the measurement of its attempt: the
/// first receipt line names the attempt and is stable for the sample.
fn receipt_key(receipt: &str) -> String {
    receipt.lines().next().unwrap_or_default().to_string()
}

/// Verdicts measured for this campaign's attempts, keyed by [`receipt_key`].
/// Used only to show the reflector what the verifier actually said; the receipt
/// stays the authoritative record.
type VerdictBook = Arc<Mutex<BTreeMap<String, String>>>;

// ---------------------------------------------------------------------------
// Training reward: the operator's verifier, not the model's word
// ---------------------------------------------------------------------------

/// Measured reward for the training batch. Candidate text only names an attempt;
/// the evaluator re-executes the operator's verifier over that attempt's working
/// copy, so no score is ever read from model output.
struct ObjectiveTrainingReward {
    evaluator: Arc<ObjectiveCaseEvaluator>,
    task: String,
    progress: Arc<Mutex<RunProgress>>,
    verdicts: VerdictBook,
    timings: TimingBook,
}

impl Reward for ObjectiveTrainingReward {
    fn label(&self) -> &str {
        TechnicalReward::ObjectivePass.label()
    }

    fn score(&self, input: RewardInput<'_>) -> Result<f32, String> {
        let (reward, receipt, generation_ms, verification_ms) = match input {
            RewardInput::EvaluatorEvidence(evidence) => (
                TechnicalReward::ObjectivePass.score(RewardInput::EvaluatorEvidence(evidence))?,
                String::new(),
                None,
                None,
            ),
            RewardInput::CandidateOutput(receipt) => {
                let (reward, verdict, verification_ms) =
                    self.evaluator.measure_attempt(receipt, &self.task)?;
                let key = receipt_key(receipt);
                if let Ok(mut verdicts) = self.verdicts.lock() {
                    verdicts.insert(key.clone(), verdict.clone());
                }
                let generation_ms = self
                    .timings
                    .lock()
                    .ok()
                    .and_then(|timings| timings.get(&key).copied());
                (reward, verdict, generation_ms, Some(verification_ms))
            }
        };
        // Only a sample with a recorded measurement is charted: a scored
        // evaluation that carries no timing is never drawn as a zero-cost
        // attempt.
        if (generation_ms.is_some() || verification_ms.is_some())
            && let Ok(mut progress) = self.progress.lock()
        {
            let step = progress.observed_attempts() + 1;
            progress.record_point(RunPoint {
                step,
                reward,
                latency_ms: attempt_cost_ms(generation_ms, verification_ms),
            });
            if !receipt.is_empty() {
                let generation = generation_ms
                    .map(|ms| format!("{ms}ms"))
                    .unwrap_or_else(|| "n/a".to_string());
                let verification = verification_ms
                    .map(|ms| format!("{ms}ms"))
                    .unwrap_or_else(|| "n/a".to_string());
                progress.note(format!(
                    "attempt {step} · {receipt} · {generation} generation + {verification} verify"
                ));
            }
        }
        Ok(reward)
    }
}

/// Reflection through the club's own cancellation-aware call path, so `/rl stop`
/// reaches a request that is already in flight rather than only the next one.
struct CancellableReflector {
    club: Arc<dyn Club>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<RunProgress>>,
    verdicts: VerdictBook,
}

impl CancellableReflector {
    /// The reflection prompt the reflector club sees: what the attempt did, plus
    /// the physical verdict the evaluator measured for it.
    fn prompt(&self, current_prompt: &str, task: &str, best: &str, worst: &str) -> String {
        let annotate = |output: &str| {
            let verdict = self
                .verdicts
                .lock()
                .ok()
                .and_then(|verdicts| verdicts.get(&receipt_key(output)).cloned());
            match verdict {
                Some(verdict) => format!("{output}\nphysical verifier: {verdict}"),
                None => output.to_string(),
            }
        };
        format!(
            "You optimize system prompts for a coding agent.\n\nTASK the agent must do:\n{task}\n\n\
             CURRENT policy note:\n{current_prompt}\n\nA HIGH-scoring attempt (measured by the \
             objective's own verifier):\n{}\n\nA LOW-scoring attempt:\n{}\n\nWrite an improved \
             policy note that steers the agent toward what the high-scoring attempt did. Reply \
             with ONLY the new note.",
            annotate(best),
            annotate(worst)
        )
    }
}

impl Reflector for CancellableReflector {
    fn improve(
        &self,
        current_prompt: &str,
        task: &str,
        best_output: &str,
        worst_output: &str,
    ) -> Result<String, String> {
        if self.cancel.load(Ordering::Acquire) {
            return Err("campaign cancelled".into());
        }
        let prompt = self.prompt(current_prompt, task, best_output, worst_output);
        let reply = match self.club.chat_streaming(
            &[ChatMsg::user(prompt)],
            &[],
            &self.cancel,
            &mut |_delta| {},
        ) {
            Ok(reply) => reply,
            Err(_) if self.cancel.load(Ordering::Acquire) => {
                return Err("campaign cancelled".to_string());
            }
            Err(error) => return Err(error),
        };
        let improved = match reply {
            ClubReply::Text(text) => text,
            _ => return Err("the reflection step must answer in text".into()),
        };
        if self.cancel.load(Ordering::Acquire) {
            return Err("campaign cancelled".into());
        }
        if let Ok(mut progress) = self.progress.lock() {
            progress.note(format!(
                "reflection proposed a {} char policy note",
                improved.trim().chars().count()
            ));
        }
        Ok(improved)
    }
}

// ---------------------------------------------------------------------------
// Campaign state
// ---------------------------------------------------------------------------

#[derive(Default)]
pub(crate) struct RlState {
    pub mode: RlMode,
    pub started: Option<Instant>,
    pub progress: Arc<Mutex<RunProgress>>,
    cancel: Option<Arc<AtomicBool>>,
    run_dir: Option<PathBuf>,
    loop_context: Option<LoopCampaignContext>,
    loop_owner: Option<String>,
    loop_tokens: Arc<AtomicUsize>,
    loop_account_owner: Option<String>,
    research: research::ResearchState,
}

impl RlState {
    pub fn running(&self) -> bool {
        self.mode == RlMode::Campaign
            && self
                .progress
                .lock()
                .map(|progress| progress.outcome.is_none())
                .unwrap_or(false)
    }

    pub fn progress_snapshot(&self) -> RunProgress {
        self.progress.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Cancel a running campaign. Every in-flight attempt and the reflector
    /// observe the flag — including a request already on the wire and a verifier
    /// process already running — so the run stops without installing a policy.
    /// The campaign thread reports the stop itself once its children have ended;
    /// nothing here claims completion on their behalf.
    pub fn stop(&mut self) -> bool {
        if !self.running() {
            return false;
        }
        let Some(cancel) = self.cancel.as_ref() else {
            return false;
        };
        if cancel.swap(true, Ordering::AcqRel) {
            return false;
        }
        true
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn run_dir(&self) -> Option<&Path> {
        self.run_dir.as_deref()
    }

    /// Launch one campaign on the operator's selected club. Returns the
    /// operator-facing launch line; all work happens on the campaign thread.
    pub fn start_campaign(
        &mut self,
        workspace: &Path,
        club: Arc<dyn Club>,
        args: &[String],
    ) -> Result<String, String> {
        let plan = RlPlan::parse(workspace, args)?;
        self.start_with_plan(plan, workspace, club)
    }

    /// Launch a campaign from the raw command line the operator typed, split by
    /// [`split_command_args`] so quoted multiword objectives and verifier
    /// commands arrive exactly as written.
    pub fn start_campaign_argv(
        &mut self,
        workspace: &Path,
        club: Arc<dyn Club>,
        raw: &str,
    ) -> Result<String, String> {
        let plan = RlPlan::parse_command_line(workspace, raw)?;
        self.start_with_plan(plan, workspace, club)
    }

    fn start_with_plan(
        &mut self,
        plan: RlPlan,
        workspace: &Path,
        club: Arc<dyn Club>,
    ) -> Result<String, String> {
        self.start_recorded(plan, workspace, club, None)
    }

    fn start_recorded(
        &mut self,
        plan: RlPlan,
        workspace: &Path,
        club: Arc<dyn Club>,
        owner: Option<LoopCampaignContext>,
    ) -> Result<String, String> {
        if self.running() {
            return Err("an RL campaign is already in progress (/rl stop first)".into());
        }
        let workspace = crate::workspace_store::repo_identity(workspace).root;
        let run_id = new_run_id();
        let run_dir = workspace_run_root(&workspace).join(&run_id);
        let attempts_root = run_dir.join("attempts");
        std::fs::create_dir_all(&attempts_root)
            .map_err(|error| format!("could not create RL run directory: {error}"))?;

        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(RunProgress {
            planned_attempts: plan.planned_attempts(),
            rounds_planned: plan.rounds,
            objective: Some(LaunchedObjective {
                cases: plan.cases.clone(),
                audit: plan.audit.clone(),
                verifier_scope: plan.verifier_scope.clone(),
            }),
            ..RunProgress::default()
        }));
        let route = club.route_identity();
        let mut record =
            loop_campaign::CampaignRecord::new(&run_id, owner.as_ref(), &plan, route.clone());
        record.write(&run_dir)?;
        self.loop_owner = owner.as_ref().map(|context| context.loop_id.clone());
        self.progress = Arc::clone(&progress);
        self.cancel = Some(Arc::clone(&cancel));
        self.run_dir = Some(run_dir.clone());
        self.mode = RlMode::Campaign;
        self.started = Some(Instant::now());

        let launch = format!(
            "rl · campaign started — {} selection case(s){} · {} round(s) · group {} · {} sample(s)/case · route {} · artifacts {}",
            plan.cases.len(),
            if plan.audit.is_empty() {
                " · no audit case, measured exploration".to_string()
            } else {
                format!(" · {} audit case(s)", plan.audit.len())
            },
            plan.rounds,
            plan.group,
            plan.samples,
            route.driver,
            run_dir.display()
        );
        let failed_progress = Arc::clone(&progress);
        let failed_run_dir = run_dir.clone();
        let mut failed_record = record.clone();
        let tokens = Arc::clone(&self.loop_tokens);
        let spawn = std::thread::Builder::new()
            .name("angel-rl-campaign".into())
            .spawn(move || {
                let (club, budget) =
                    loop_campaign::campaign_club(club, owner.as_ref(), tokens, &cancel);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_plan(&plan, &workspace, club, route, &run_dir, &progress, &cancel)
                }))
                .unwrap_or_else(|_| {
                    Err("RL campaign worker panicked; inspect retained artifacts".into())
                });
                let outcome = budget.finish(outcome);
                record.outcome = Some(outcome.clone());
                let outcome = match record.write(&run_dir) {
                    Ok(()) => outcome,
                    Err(error) => Err(format!(
                        "campaign settled but result persistence failed: {error}"
                    )),
                };
                if let Ok(mut guard) = progress.lock()
                    && guard.outcome.is_none()
                {
                    guard.outcome = Some(outcome);
                }
            });
        if let Err(error) = spawn {
            let error = format!("could not start the RL campaign thread: {error}");
            failed_record.outcome = Some(Err(error.clone()));
            let _ = failed_record.write(&failed_run_dir);
            if let Ok(mut progress) = failed_progress.lock() {
                progress.outcome = Some(Err(error.clone()));
            }
            return Err(error);
        }
        Ok(launch)
    }
}

fn new_run_id() -> String {
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!(
        "run-{millis}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

/// Root of the durable campaign store. `ANGEL_RL_DIR` overrides it (tests and
/// sandboxed runners must never write the operator's real `~/.angel0`).
fn rl_root() -> PathBuf {
    match std::env::var("ANGEL_RL_DIR") {
        Ok(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => crate::workspace_store::angel_subdir("rl"),
    }
}

fn workspace_run_root(workspace: &Path) -> PathBuf {
    rl_root().join(crate::workspace_store::workspace_key(
        &crate::workspace_store::repo_identity(workspace).root,
    ))
}

// ---------------------------------------------------------------------------
// The campaign itself
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_plan(
    plan: &RlPlan,
    workspace: &Path,
    club: Arc<dyn Club>,
    route: RouteIdentity,
    run_dir: &Path,
    progress: &Arc<Mutex<RunProgress>>,
    cancel: &Arc<AtomicBool>,
) -> Result<CampaignOutcome, String> {
    let started = Instant::now();
    let attempts_root = run_dir.join("attempts");
    let fixture_root = run_dir.join("fixture");
    // Freeze the source once: every sample starts from these exact bytes, and
    // the case receipts can name that identity.
    crate::harness::freeze_active_source(workspace, &fixture_root, cancel)?;

    let mut evaluators: BTreeMap<String, Arc<ObjectiveCaseEvaluator>> = BTreeMap::new();
    // Selection cases share the campaign source freeze; an audit case freezes its
    // own source so the gate sees a genuinely independent objective.
    let mut fixtures: BTreeMap<String, PathBuf> = BTreeMap::new();
    for case in plan.cases.iter() {
        fixtures.insert(case.task.clone(), fixture_root.clone());
    }
    for case in plan.audit.iter() {
        let source = case
            .source
            .clone()
            .ok_or_else(|| format!("audit case {} has no source scope", case.id))?;
        let dest = run_dir.join(format!("fixture-{}", case.id));
        crate::harness::freeze_active_source(&source, &dest, cancel)?;
        fixtures.insert(case.task.clone(), dest);
    }
    // Every measurement the evaluator takes — promotion and audit samples —
    // is reported to the stage; the authoritative verdict stays the receipt.
    let sink_progress = Arc::clone(progress);
    let sink: crate::reinforce::objective_case::MeasurementSink =
        Arc::new(move |reward, generation_ms, verification_ms| {
            if let Ok(mut guard) = sink_progress.lock() {
                let step = guard.observed_attempts() + 1;
                guard.record_point(RunPoint {
                    step,
                    reward,
                    latency_ms: generation_ms.saturating_add(verification_ms),
                });
            }
        });
    for case in plan.cases.iter().chain(plan.audit.iter()) {
        let case_fixture = fixtures
            .get(&case.task)
            .ok_or_else(|| format!("case {:?} has no source scope", case.id))?;
        let evaluator = Arc::new(
            ObjectiveCaseEvaluator::new(
                case_fixture,
                &case.verify,
                &attempts_root,
                &plan.verifier_scope,
                Arc::clone(cancel),
            )?
            .with_measurement_sink(Arc::clone(&sink)),
        );
        if evaluators.insert(case.task.clone(), evaluator).is_some() {
            return Err(format!("case task {:?} is bound twice", case.task));
        }
    }
    let training_case = plan
        .cases
        .first()
        .and_then(|case| evaluators.get(&case.task).cloned())
        .ok_or("a campaign needs at least one selection case")?;
    let verdicts: VerdictBook = Arc::new(Mutex::new(BTreeMap::new()));
    let timings: TimingBook = Arc::new(Mutex::new(BTreeMap::new()));

    let generator = CodingPolicyGenerator {
        club: Arc::clone(&club),
        fixtures: fixtures.clone(),
        attempts_root: attempts_root.clone(),
        progress: Arc::clone(progress),
        timings: Arc::clone(&timings),
        cancel: Arc::clone(cancel),
        seq: AtomicUsize::new(0),
    };
    let training_reward = ObjectiveTrainingReward {
        evaluator: Arc::clone(&training_case),
        task: plan.cases[0].task.clone(),
        progress: Arc::clone(progress),
        verdicts: Arc::clone(&verdicts),
        timings,
    };
    let reflector = CancellableReflector {
        club: Arc::clone(&club),
        cancel: Arc::clone(cancel),
        progress: Arc::clone(progress),
        verdicts,
    };
    let config = reinforce_config(plan.group);
    let promotion_config = objective_promotion_config(plan.samples, plan.cases.len());
    let audit_config = objective_promotion_config(plan.samples, plan.audit.len().max(1));

    let promotion_cases: Vec<TechnicalHeldoutCase<'_>> = plan
        .cases
        .iter()
        .map(|case| TechnicalHeldoutCase {
            id: case.id.as_str(),
            task: case.task.as_str(),
            reward: TechnicalReward::ObjectivePass,
        })
        .collect();
    let audit_cases: Vec<TechnicalHeldoutCase<'_>> = plan
        .audit
        .iter()
        .map(|case| TechnicalHeldoutCase {
            id: case.id.as_str(),
            task: case.task.as_str(),
            reward: TechnicalReward::ObjectivePass,
        })
        .collect();
    let promotion_evaluators: Vec<&dyn crate::reinforce::evaluator::PolicyEvaluator> = plan
        .cases
        .iter()
        .map(|case| {
            evaluators
                .get(&case.task)
                .map(|evaluator| {
                    evaluator.as_ref() as &dyn crate::reinforce::evaluator::PolicyEvaluator
                })
                .ok_or_else(|| format!("case {:?} has no evaluator", case.id))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let audit_evaluators: Vec<&dyn crate::reinforce::evaluator::PolicyEvaluator> = plan
        .audit
        .iter()
        .map(|case| {
            evaluators
                .get(&case.task)
                .map(|evaluator| {
                    evaluator.as_ref() as &dyn crate::reinforce::evaluator::PolicyEvaluator
                })
                .ok_or_else(|| format!("audit case {:?} has no evaluator", case.id))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let promotion_manifest = CohortManifest::new_technical(
        &format!(
            "objective-{}",
            crate::cut::sha256_hex(plan.cases[0].task.as_bytes())
        ),
        CohortRole::Promotion,
        &promotion_cases,
        &promotion_config,
        &promotion_evaluators,
    )?;
    let audit_manifest = if audit_cases.is_empty() {
        None
    } else {
        Some(CohortManifest::new_technical(
            &format!(
                "audit-{}",
                crate::cut::sha256_hex(plan.audit[0].task.as_bytes())
            ),
            CohortRole::FinalAudit,
            &audit_cases,
            &audit_config,
            &audit_evaluators,
        )?)
    };

    let audit_supplied = audit_manifest.is_some();
    let mut incumbent = current_policy_note(workspace).unwrap_or_default();
    let mut version = 0u64;
    let mut rounds_recorded: Vec<RecordedRound> = Vec::new();
    let mut release: Option<(String, String)> = None; // prompt, release sha
    let task = plan.cases[0].task.clone();

    for round in 0..plan.rounds {
        if cancel.load(Ordering::Acquire) {
            return Err("stopped by operator".into());
        }
        let campaign_id = format!("{}-r{}", run_id_of(run_dir), round + 1);
        let authority = TechnicalCampaignAuthority::new(campaign_id)?;
        match audit_manifest.as_ref() {
            // No independently authored audit case: measured exploration. The
            // promotion verdict is real and receipt-backed, and nothing is
            // installed as validated learning.
            None => {
                if cancel.load(Ordering::Acquire) {
                    return Err("stopped by operator".into());
                }
                let dispatch = config.dispatch_count();
                let candidates = generator.generate(&incumbent, &task, version, dispatch);
                let (rollouts, metrics) = crate::reinforce::evaluate_batch(
                    &training_reward,
                    version,
                    &config,
                    candidates,
                );
                if !metrics.has_learning_signal() {
                    if let Ok(mut guard) = progress.lock() {
                        guard.note(format!(
                            "round {} · no advantage spread (solve {:.0}%, variance {:.5}) — no proposal",
                            round + 1,
                            metrics.solve_rate * 100.0,
                            metrics.advantage_variance
                        ));
                    }
                    rounds_recorded.push(RecordedRound {
                        index: round + 1,
                        incumbent_version: version,
                        promotion: None,
                        audit: None,
                        release_sha256: None,
                        solve_rate: metrics.solve_rate,
                        advantage_variance: metrics.advantage_variance,
                    });
                    continue;
                }
                let best = rollouts
                    .iter()
                    .filter(|rollout| rollout.accepted)
                    .max_by(|left, right| left.reward.total_cmp(&right.reward));
                let worst = rollouts
                    .iter()
                    .filter(|rollout| rollout.accepted)
                    .min_by(|left, right| left.reward.total_cmp(&right.reward));
                let (Some(best), Some(worst)) = (best, worst) else {
                    rounds_recorded.push(RecordedRound {
                        index: round + 1,
                        incumbent_version: version,
                        promotion: None,
                        audit: None,
                        release_sha256: None,
                        solve_rate: metrics.solve_rate,
                        advantage_variance: metrics.advantage_variance,
                    });
                    continue;
                };
                // A stop during reflection is the operator's stop, not the
                // reflector's own wording for having been interrupted.
                let candidate = cancellation_aware(
                    reflector.improve(&incumbent, &task, &best.output, &worst.output),
                    cancel,
                )?;
                if candidate.trim().is_empty() || candidate.trim() == incumbent.trim() {
                    return Err("the reflector proposed no change to measure".into());
                }
                let heldout =
                    crate::reinforce::promotion::technical_cases_as_heldout(&promotion_cases);
                let report = cancellation_aware(
                    evaluate_promotion_with_receipts(
                        &generator,
                        ReceiptPromotionRequest {
                            incumbent_prompt: &incumbent,
                            candidate_prompt: &candidate,
                            incumbent_version: version,
                            cases: &heldout,
                            config: &promotion_config,
                            manifest: &promotion_manifest,
                            evaluators: &promotion_evaluators,
                            receipt_store: authority.exploration_receipt_store(),
                        },
                    ),
                    cancel,
                )?;
                if report.promoted() {
                    incumbent = candidate;
                    version = version.saturating_add(1);
                }
                rounds_recorded.push(RecordedRound {
                    index: round + 1,
                    incumbent_version: version,
                    promotion: Some(report),
                    audit: None,
                    release_sha256: None,
                    solve_rate: metrics.solve_rate,
                    advantage_variance: metrics.advantage_variance,
                });
            }
            // An independently authored audit case exists: the release path.
            Some(audit_manifest) => {
                let report = cancellation_aware(
                    run_reinforce(
                        &generator,
                        &training_reward,
                        &reflector,
                        ReinforceRequest {
                            task: task.as_str(),
                            initial_prompt: incumbent.as_str(),
                            // The technical campaign is a one-transition API; each
                            // /rl round is one auditable campaign.
                            rounds: 1,
                            config: &config,
                        },
                        TechnicalReinforceCampaign {
                            authority: &authority,
                            promotion: TechnicalPromotionCohort {
                                cases: &promotion_cases,
                                config: &promotion_config,
                                manifest: &promotion_manifest,
                                evaluators: &promotion_evaluators,
                            },
                            final_audit: TechnicalFinalAuditCohort {
                                cases: &audit_cases,
                                config: &audit_config,
                                manifest: audit_manifest,
                                evaluators: &audit_evaluators,
                            },
                        },
                    ),
                    cancel,
                )?;
                let metrics = report
                    .reinforcement()
                    .rounds
                    .last()
                    .map(|round| round.metrics.clone());
                let promotion = report
                    .reinforcement()
                    .rounds
                    .last()
                    .and_then(|round| round.promotion.clone());
                let audit = report.final_audit().cloned();
                let released = report.release_candidate().map(|release| {
                    (
                        release.prompt().to_string(),
                        release.release_sha256().to_string(),
                    )
                });
                if let Some((prompt, sha)) = released.clone() {
                    incumbent = prompt;
                    version = version.saturating_add(1);
                    release = Some((incumbent.clone(), sha));
                }
                let solve_rate = metrics.as_ref().map_or(0.0, |metrics| metrics.solve_rate);
                let advantage_variance = metrics
                    .as_ref()
                    .map_or(0.0, |metrics| metrics.advantage_variance);
                rounds_recorded.push(RecordedRound {
                    index: round + 1,
                    incumbent_version: version,
                    promotion,
                    audit,
                    release_sha256: released.map(|(_, sha)| sha),
                    solve_rate,
                    advantage_variance,
                });
            }
        }
    }

    if cancel.load(Ordering::Acquire) {
        return Err("stopped by operator".into());
    }

    // Install only what an approved audit released.
    let mut accepted_entry = None;
    let mut accepted_event = None;
    if let Some((prompt, _)) = release.as_ref()
        && let Some(round) = rounds_recorded
            .iter()
            .rev()
            .find(|round| round.release_sha256.is_some())
    {
        let persisted = persist_policy_note(
            workspace,
            prompt,
            round.incumbent_version,
            round.audit.as_ref().or(round.promotion.as_ref()),
            plan,
            &route,
            run_dir,
        )?;
        accepted_entry = Some(persisted.0);
        accepted_event = Some(persisted.1);
    }

    let snapshot = report_snapshot(progress);
    let last = rounds_recorded.last();
    let promoted_rounds = rounds_recorded
        .iter()
        .filter(|round| {
            round
                .promotion
                .as_ref()
                .is_some_and(PromotionReport::promoted)
        })
        .count();
    let decision = match last {
        Some(round) => match (&round.audit, &round.promotion) {
            (Some(audit), _) => audit.decision.label().to_string(),
            (None, Some(promotion)) => promotion.decision.label().to_string(),
            (None, None) => "no-reflection".to_string(),
        },
        None => "no-round".to_string(),
    };
    let mean_delta = last.and_then(|round| {
        round
            .audit
            .as_ref()
            .or(round.promotion.as_ref())
            .and_then(|report| report.mean_delta)
    });
    let validated = accepted_entry.is_some();
    let outcome = CampaignOutcome {
        attempted: snapshot.observed_attempts(),
        passed: snapshot.passed,
        red: snapshot.red,
        rounds: rounds_recorded.len(),
        promoted_rounds,
        policy_version: version,
        decision,
        mean_delta,
        validated,
        audit_supplied,
        release_sha256: release.as_ref().map(|(_, sha)| sha.clone()),
        solve_rate: last.map(|round| round.solve_rate),
        advantage_variance: last.map(|round| round.advantage_variance),
        reflection: rounds_recorded
            .iter()
            .any(|round| round.promotion.is_some()),
        accepted_entry: accepted_entry.clone(),
        accepted_event: accepted_event.clone(),
        report_path: run_dir.display().to_string(),
        route: route.clone(),
        wall_s: started.elapsed().as_secs_f32(),
    };
    write_run_record(
        run_dir,
        &RunRecord {
            schema: RUN_SCHEMA.to_string(),
            run_id: run_id_of(run_dir),
            workspace: workspace.display().to_string(),
            started_ms: now_ms(),
            wall_ms: started.elapsed().as_millis() as u64,
            cases: plan
                .cases
                .iter()
                .map(RecordedCase::from)
                .chain(plan.audit.iter().map(RecordedCase::from))
                .collect(),
            rounds: plan.rounds,
            group: plan.group,
            samples: plan.samples,
            route: route.clone(),
            audit_supplied,
            validated,
            accepted_entry: accepted_entry.clone(),
            accepted_event: accepted_event.clone(),
            decision: outcome.decision.clone(),
            mean_delta,
            rounds_recorded,
            points: snapshot
                .points
                .iter()
                .map(|point| RecordedPoint {
                    step: point.step,
                    reward: point.reward,
                    latency_ms: point.latency_ms,
                })
                .collect(),
            error: None,
        },
    )?;
    if let Ok(mut guard) = progress.lock() {
        guard.rounds_done = outcome.rounds;
        guard.promoted = promoted_rounds > 0;
        guard.policy_version = outcome.policy_version;
        guard.note(format!(
            "campaign finished · {} attempt(s) · {} passed · {} · {}",
            outcome.attempted,
            outcome.passed,
            outcome.decision,
            if validated {
                "audit approved, policy installed"
            } else if audit_supplied {
                "audit did not approve a release"
            } else {
                "exploration only — no audit case supplied"
            }
        ));
    }
    Ok(outcome)
}

fn run_id_of(run_dir: &Path) -> String {
    run_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

fn persist_policy_note(
    workspace: &Path,
    prompt: &str,
    version: u64,
    report: Option<&PromotionReport>,
    plan: &RlPlan,
    route: &RouteIdentity,
    run_dir: &Path,
) -> Result<(String, String), String> {
    if prompt.trim().is_empty() {
        return Err("an approved release carried an empty policy note".into());
    }
    let existing = crate::continual_harness::load_project(workspace)
        .entries
        .contains_key(RL_POLICY_ENTRY_ID);
    let edit = RefinementEdit {
        action: if existing { "update" } else { "create" }.to_string(),
        kind: EntryKind::Prompt,
        id: Some(RL_POLICY_ENTRY_ID.to_string()),
        title: Some(format!("{RL_POLICY_TITLE_PREFIX}{version}")),
        content: Some(prompt.to_string()),
        path: Some("rl".to_string()),
        reason: Some("released by an audited reinforcement campaign".to_string()),
    };
    let evidence = format!(
        "campaign {} · route {} · selection cases {} · audit cases {} · cohort manifest {} · {} · mean delta {}",
        run_dir.display(),
        route.driver,
        plan.cases.len(),
        plan.audit.len(),
        report.map_or("n/a".to_string(), |report| report
            .cohort_manifest_sha256
            .clone()),
        report.map_or("n/a".to_string(), |report| report
            .decision
            .label()
            .to_string()),
        report
            .and_then(|report| report.mean_delta)
            .map(|delta| format!("{delta:+.4}"))
            .unwrap_or_else(|| "n/a".to_string()),
    );
    let outcome = format!(
        "policy v{version} released after an approved audit · {} sample(s)/case over {} selection + {} audit case(s)",
        plan.samples,
        plan.cases.len(),
        plan.audit.len()
    );
    crate::continual_harness::apply_edits(
        workspace,
        Scope::Project,
        "rl campaign release",
        &evidence,
        &outcome,
        &[edit],
    )?;
    let event = crate::continual_harness::load_project(workspace)
        .refinements
        .last()
        .map(|event| event.id.clone())
        .ok_or_else(|| "the release was not recorded in the continual harness".to_string())?;
    Ok((RL_POLICY_ENTRY_ID.to_string(), event))
}

/// The policy note later turns currently consume, if a campaign released one.
pub(crate) fn current_policy_note(workspace: &Path) -> Option<String> {
    crate::continual_harness::load_project(workspace)
        .entries
        .get(RL_POLICY_ENTRY_ID)
        .map(|entry| entry.content.clone())
}

fn report_snapshot(progress: &Arc<Mutex<RunProgress>>) -> RunProgress {
    progress.lock().map(|p| p.clone()).unwrap_or_default()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Durable run record
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedCase {
    id: String,
    task: String,
    verify_sha256: String,
}

impl From<&RlCase> for RecordedCase {
    fn from(case: &RlCase) -> Self {
        Self {
            id: case.id.clone(),
            task: case.task.clone(),
            verify_sha256: crate::cut::sha256_hex(case.verify.as_bytes()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedPoint {
    step: usize,
    reward: f32,
    latency_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedRound {
    index: usize,
    incumbent_version: u64,
    promotion: Option<PromotionReport>,
    audit: Option<PromotionReport>,
    release_sha256: Option<String>,
    solve_rate: f32,
    advantage_variance: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RunRecord {
    schema: String,
    run_id: String,
    workspace: String,
    started_ms: u64,
    wall_ms: u64,
    cases: Vec<RecordedCase>,
    rounds: usize,
    group: usize,
    samples: usize,
    route: RouteIdentity,
    audit_supplied: bool,
    validated: bool,
    accepted_entry: Option<String>,
    accepted_event: Option<String>,
    decision: String,
    mean_delta: Option<f32>,
    rounds_recorded: Vec<RecordedRound>,
    points: Vec<RecordedPoint>,
    error: Option<String>,
}

fn write_run_record(run_dir: &Path, record: &RunRecord) -> Result<(), String> {
    std::fs::create_dir_all(run_dir).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(record).map_err(|error| error.to_string())?;
    let path = run_dir.join("report.json");
    let temporary = run_dir.join(format!("report.json.tmp.{}", std::process::id()));
    std::fs::write(&temporary, &bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, &path).map_err(|error| error.to_string())
}

/// Newest campaign record for this workspace, so the stage opens on real
/// history instead of an empty graph.
pub(crate) fn hydrate_latest_campaign(
    workspace: &Path,
) -> Option<(Vec<RunPoint>, CampaignOutcome)> {
    let root = workspace_run_root(workspace);
    let entries = std::fs::read_dir(&root).ok()?;
    let newest = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .max_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })?;
    let raw = std::fs::read_to_string(newest.join("report.json")).ok()?;
    let record: RunRecord = serde_json::from_str(&raw).ok()?;
    let points = record
        .points
        .iter()
        .map(|point| RunPoint {
            step: point.step,
            reward: point.reward,
            latency_ms: point.latency_ms,
        })
        .collect();
    let last = record.rounds_recorded.last();
    let outcome = CampaignOutcome {
        attempted: record.points.len(),
        passed: record
            .points
            .iter()
            .filter(|point| point.reward >= 1.0)
            .count(),
        red: record
            .points
            .iter()
            .filter(|point| point.reward < 1.0)
            .count(),
        rounds: record.rounds_recorded.len(),
        promoted_rounds: record
            .rounds_recorded
            .iter()
            .filter(|round| {
                round
                    .promotion
                    .as_ref()
                    .is_some_and(PromotionReport::promoted)
            })
            .count(),
        policy_version: last.map_or(0, |round| round.incumbent_version),
        decision: record.decision.clone(),
        mean_delta: record.mean_delta,
        validated: record.validated,
        audit_supplied: record.audit_supplied,
        release_sha256: last.and_then(|round| round.release_sha256.clone()),
        solve_rate: last.map(|round| round.solve_rate),
        advantage_variance: last.map(|round| round.advantage_variance),
        reflection: record
            .rounds_recorded
            .iter()
            .any(|round| round.promotion.is_some()),
        accepted_entry: record.accepted_entry.clone(),
        accepted_event: record.accepted_event.clone(),
        report_path: newest.display().to_string(),
        route: record.route.clone(),
        wall_s: record.wall_ms as f32 / 1000.0,
    };
    Some((points, outcome))
}

/// Summary of the durable technical-campaign authority, if one exists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AuthoritySummary {
    pub campaigns: usize,
    pub releases: usize,
    pub poisoned: usize,
}

pub(crate) fn authority_summary() -> Option<AuthoritySummary> {
    let root = crate::reinforce::authority_root();
    let entries = std::fs::read_dir(root).ok()?;
    let mut summary = AuthoritySummary::default();
    for entry in entries.filter_map(|entry| entry.ok()) {
        let ledger = entry.path().join("ledger");
        if !ledger.is_dir() {
            continue;
        }
        summary.campaigns += 1;
        if ledger.join("terminal-release.json").exists() {
            summary.releases += 1;
        }
        if let Ok(poison) = std::fs::read_dir(ledger.join("poison")) {
            summary.poisoned += poison.filter_map(|entry| entry.ok()).count();
        }
    }
    (summary.campaigns > 0).then_some(summary)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/rl_ctl__campaign_tests.rs"]
mod campaign_tests;

#[cfg(test)]
#[path = "../../tests/cockpit/app/rl_ctl__plan_tests.rs"]
mod plan_tests;
