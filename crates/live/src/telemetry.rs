//! Plain telemetry data the cockpit and the realm (game) host share.
//!
//! A leaf module of the contract crate: it names no cockpit or delve type, so
//! both sides depend on it and neither rebuilds the other. The cockpit keeps
//! each type's old path as a re-export (`agent::harness::ToolOutcome`,
//! `knowledge::graph_crop::Chart`, `ui::viz::lifecycle_viz::MotionMode`, ...);
//! behavior that needs cockpit or delve types stays on their side as a free
//! function or an extension trait.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

// ---- tool outcomes (was agent::harness::registry) ----

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolEventId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Succeeded,
    /// The harness rejected the call before dispatch, so no tool execution
    /// occurred and no mutation or verification credit may be attributed.
    NotStarted,
    Failed,
    Denied,
    Cancelled,
    Panicked,
}

impl ExecutionOutcome {
    /// Stable machine projection. Keep this explicit so multi-word variants do
    /// not inherit Rust's `Debug` spelling (for example, `NotStarted` must not
    /// leak as the ambiguous `notstarted`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::NotStarted => "not_started",
            Self::Failed => "failed",
            Self::Denied => "denied",
            Self::Cancelled => "cancelled",
            Self::Panicked => "panicked",
        }
    }
}

/// Verifier truth is independent from dispatch truth. A verifier process may
/// run successfully while reporting a red or inconclusive project state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationOutcome {
    NotApplicable,
    Passed,
    Failed,
    Inconclusive,
}

impl VerificationOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not-applicable",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolOutcome {
    pub execution: ExecutionOutcome,
    pub verification: VerificationOutcome,
}

impl ToolOutcome {
    pub fn attributable_success(self) -> bool {
        self.execution == ExecutionOutcome::Succeeded
            && matches!(
                self.verification,
                VerificationOutcome::NotApplicable | VerificationOutcome::Passed
            )
    }
}

// ---- competition submissions (was agent::harness::cartridges::fleet) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionPhase {
    Queued,
    Running,
    Accepted,
    Rejected,
    TimedOut,
    Unknown,
}

impl SubmissionPhase {
    pub fn from_status(status: &str) -> Self {
        match status.trim().to_ascii_lowercase().as_str() {
            "queued" | "pending" | "submitted" => Self::Queued,
            "validating" | "running" | "in-flight" | "in_flight" => Self::Running,
            "accepted" | "promoted" | "complete" | "completed" => Self::Accepted,
            "timeout" | "timed_out" | "timed-out" => Self::TimedOut,
            "rejected" | "failed" | "promotion failed" | "error" | "cancelled" | "canceled" => {
                Self::Rejected
            }
            _ => Self::Unknown,
        }
    }
}

// ---- subsystem health (was knowledge::{memory::store, atlas, atlas_clerk}) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryHealth {
    Disabled,
    Healthy,
    Degraded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AtlasHealth {
    Disabled,
    Healthy,
    Degraded,
    Inert,
}

impl AtlasHealth {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Inert => "inert",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClerkHealth {
    #[default]
    Idle,
    Running,
    WaitingForRoute,
    ResourceConflict,
    Degraded,
}

impl ClerkHealth {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::WaitingForRoute => "waiting",
            Self::ResourceConflict => "conflict",
            Self::Degraded => "degraded",
        }
    }
}

// ---- graph garden data (was knowledge::graph_crop) ----

pub const MAX_PLOTS: usize = 4;
pub const MAX_POINTS: u8 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Bar,
    Line,
    Scatter,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartSpec {
    pub title: String,
    pub kind: ChartKind,
    pub x_label: String,
    pub y_label: String,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub expected_points: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataPoint {
    pub label: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum GraphRequest {
    Begin {
        plot: String,
        spec: ChartSpec,
    },
    Point {
        plot: String,
        generation: u64,
        index: u8,
        point: DataPoint,
    },
    Finish {
        plot: String,
        generation: u64,
    },
    Clear {
        plot: String,
    },
}

impl GraphRequest {
    pub fn plot(&self) -> &str {
        match self {
            Self::Begin { plot, .. }
            | Self::Point { plot, .. }
            | Self::Finish { plot, .. }
            | Self::Clear { plot } => plot,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let plot = self.plot();
        if plot.is_empty()
            || plot.len() > 32
            || !plot
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err("plot must be 1-32 lowercase letters, digits, or hyphens".into());
        }
        match self {
            Self::Begin { spec, .. } => spec.validate(),
            Self::Point {
                generation,
                index,
                point,
                ..
            } => {
                if *generation == 0
                    || *index >= MAX_POINTS
                    || !text_valid(&point.label, 32)
                    || !number(point.x)
                    || !number(point.y)
                {
                    return Err("point needs a generation, index 0-31, a short label, and finite coordinates within +/-1e12".into());
                }
                Ok(())
            }
            Self::Finish { generation, .. } if *generation == 0 => {
                Err("generation must be positive".into())
            }
            _ => Ok(()),
        }
    }
}

fn number(n: f64) -> bool {
    n.is_finite() && n.abs() <= 1e12
}
pub fn text_valid(s: &str, max: usize) -> bool {
    !s.trim().is_empty()
        && s.len() <= max
        && !s.chars().any(|c| {
            c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
}
impl ChartSpec {
    pub fn validate(&self) -> Result<(), String> {
        if !text_valid(&self.title, 64)
            || !text_valid(&self.x_label, 32)
            || !text_valid(&self.y_label, 32)
            || !(1..=MAX_POINTS).contains(&self.expected_points)
            || ![self.x_min, self.x_max, self.y_min, self.y_max]
                .into_iter()
                .all(number)
            || self.x_min >= self.x_max
            || self.y_min >= self.y_max
        {
            return Err("chart needs short labels, 1-32 points, and finite increasing x/y domains within +/-1e12".into());
        }
        if self.kind == ChartKind::Bar && !(self.y_min <= 0.0 && self.y_max >= 0.0) {
            return Err("bar chart y domain must include zero".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chart {
    pub generation: u64,
    pub spec: ChartSpec,
    pub points: BTreeMap<u8, DataPoint>,
    pub finished: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphReceipt {
    pub revision: u64,
    pub request: GraphRequest,
    /// Authoritative post-call snapshot, including a tombstone after clear.
    pub chart: Option<Chart>,
}

/// Dedicated telemetry retains the real arguments and receipt, not truncated prose.
#[derive(Clone, Debug)]
pub enum GraphEvent {
    Requested(GraphRequest),
    Returned(GraphReceipt),
}

/// The garden's bounded chart ledger: applies each [`GraphRequest`] and returns
/// the receipt the world projects. Plain state; the cockpit's graph tool keeps
/// one.
#[derive(Default)]
pub struct GraphStore {
    plots: BTreeMap<String, Option<Chart>>,
    revision: u64,
}
impl GraphStore {
    pub fn apply(&mut self, request: GraphRequest) -> Result<GraphReceipt, String> {
        request.validate()?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("graph revision exhausted")?;
        let plot = request.plot().to_string();
        let chart = match &request {
            GraphRequest::Begin { spec, .. } => {
                if !self.plots.contains_key(&plot) && self.plots.len() >= MAX_PLOTS {
                    return Err("garden has four plots; reuse an existing plot id".into());
                }
                Some(Chart {
                    generation: revision,
                    spec: spec.clone(),
                    points: BTreeMap::new(),
                    finished: false,
                })
            }
            GraphRequest::Clear { .. } => {
                if !self.plots.contains_key(&plot) {
                    return Err("unknown plot".into());
                }
                None
            }
            GraphRequest::Point {
                generation,
                index,
                point,
                ..
            } => {
                let mut chart = self.current(&plot, *generation)?.clone();
                if chart.finished {
                    return Err("chart is finished; begin a new generation to regrow it".into());
                }
                if *index >= chart.spec.expected_points
                    || point.x < chart.spec.x_min
                    || point.x > chart.spec.x_max
                    || point.y < chart.spec.y_min
                    || point.y > chart.spec.y_max
                {
                    return Err("point lies outside the declared count or domain; begin with the correct scale".into());
                }
                chart.points.insert(*index, point.clone());
                Some(chart)
            }
            GraphRequest::Finish { generation, .. } => {
                let mut chart = self.current(&plot, *generation)?.clone();
                if chart.points.len() != usize::from(chart.spec.expected_points) {
                    return Err("chart still has missing data points".into());
                }
                chart.finished = true;
                Some(chart)
            }
        };
        self.revision = revision;
        self.plots.insert(plot, chart.clone());
        Ok(GraphReceipt {
            revision,
            request,
            chart,
        })
    }
    fn current(&self, plot: &str, generation: u64) -> Result<&Chart, String> {
        let chart = self
            .plots
            .get(plot)
            .and_then(Option::as_ref)
            .ok_or("prepare the plot with begin first")?;
        if generation != chart.generation {
            return Err("stale generation; use the generation returned by begin".into());
        }
        Ok(chart)
    }
}

// ---- ceremony and motion (was ui::viz::lifecycle_viz) ----

/// Frame rate and frame count of the lifecycle cut-in plates.
pub const FPS: f32 = 12.0;
pub const FRAME_COUNT: usize = 44;
const DURATION_SECS: f32 = 3.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CeremonyKind {
    GoalSet,
    GoalDone,
    GoalCleared,
    LoopStart,
    LoopPaused,
    LoopDone,
    LoopStopped,
    LoopFailed,
    LoopEscalate,
}

impl CeremonyKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::GoalSet => " tourney · objective challenged ",
            Self::GoalDone => " tourney · objective crowned ",
            Self::GoalCleared => " tourney · objective banners lowered ",
            Self::LoopStart => " tourney · challenge raised ",
            Self::LoopPaused => " tourney · riders held ",
            Self::LoopDone => " tourney · victory pass ",
            Self::LoopStopped => " tourney · field cleared ",
            Self::LoopFailed => " tourney · lance lost ",
            Self::LoopEscalate => " tourney · grand joust ",
        }
    }

    pub const fn duration_secs(self) -> f32 {
        DURATION_SECS
    }

    /// Status word the lifecycle cut-in shows beside the plate.
    pub const fn work_state(self) -> &'static str {
        match self {
            Self::GoalSet => "Goal set",
            Self::GoalDone => "Goal completed",
            Self::GoalCleared => "Goal cleared",
            Self::LoopStart => "Loop started",
            Self::LoopPaused => "Loop paused",
            Self::LoopDone => "Loop completed",
            Self::LoopStopped => "Loop stopped",
            Self::LoopFailed => "Loop failed",
            Self::LoopEscalate => "Loop escalated",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionMode {
    Full,
    Reduced,
    Off,
}

impl MotionMode {
    pub fn from_env() -> Self {
        std::env::var("ANGEL_TUI_MOTION")
            .ok()
            .as_deref()
            .map(Self::parse)
            .unwrap_or(Self::Full)
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "reduced" => Self::Reduced,
            "off" => Self::Off,
            _ => Self::Full,
        }
    }

    pub const fn animates(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Wall time used for chrome clocks. `off` stays at `frozen`; `reduced`
    /// ticks at most once per second.
    pub fn chrome_elapsed(
        self,
        started: std::time::Instant,
        frozen: std::time::Instant,
    ) -> std::time::Duration {
        match self {
            Self::Off => frozen.saturating_duration_since(started),
            Self::Reduced => std::time::Duration::from_secs(started.elapsed().as_secs()),
            Self::Full => started.elapsed(),
        }
    }
}

// ---- agent-activity seats (was ui::viz::agentviz) ----

/// One seat's lifecycle within the current stage. `Running` is the implied
/// default for any seat without a published update, so `seat_states` can stay
/// empty on publish and existing [`stage`] callers keep working unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeatState {
    #[default]
    Running,
    /// The seat came back with a usable result.
    Returned,
    /// The seat came back with an error.
    Failed,
    /// The seat was abandoned at the wave deadline/quorum/cancel barrier.
    Cut,
}

/// The latest published fan-out (a moa/swarm stage or a `spawn` formation), as
/// the miniworld reads it each tick to muster an army: stage name, one label
/// per seat and each seat's lifecycle. Plain data; the cockpit's
/// `ui::viz::agentviz` publishes it and keeps the old path as a re-export.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StageSnapshot {
    /// Stable publish identity; seat updates change `seq`, not this identity.
    pub stage_id: u64,
    pub name: String,
    pub agents: Vec<String>,
    /// Per-seat lifecycle, index-aligned with `agents`. Starts empty on every
    /// publish (all seats implicitly `Running`) and may stay shorter than
    /// `agents`; read through [`StageSnapshot::seat_state`].
    pub seat_states: Vec<SeatState>,
    /// Monotonic per publish — the world re-musters when this moves. Seat
    /// updates bump it too, so consumers keyed off the seq re-read for pips.
    pub seq: u64,
}

impl StageSnapshot {
    /// The state of seat `idx`; `Running` when no update has been published.
    pub fn seat_state(&self, idx: usize) -> SeatState {
        self.seat_states.get(idx).copied().unwrap_or_default()
    }

    /// How many seats have come back with a result (`Returned`) — the "3/6
    /// back" count for status rows.
    pub fn returned(&self) -> usize {
        self.seat_states
            .iter()
            .filter(|s| matches!(s, SeatState::Returned))
            .count()
    }
}

// ---- research measurements (was drive::research_workspace::measurement) ----

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub value: ScoreV1,
    pub baseline: Option<ScoreV1>,
    pub dataset: Dataset,
    pub lower_is_better: bool,
    pub receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dataset {
    FullDevelopment,
    Diagnostic,
    OfficialReport,
}

impl Measurement {
    pub fn dataset_label(&self) -> &'static str {
        match self.dataset {
            Dataset::FullDevelopment => "full dev",
            Dataset::Diagnostic => "subset",
            Dataset::OfficialReport => "official report",
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.receipt_sha256.len() != 64
            || !self
                .receipt_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid measurement receipt reference");
        }
        Ok(())
    }

    /// How the value stands against its baseline, by exact decimal order. The
    /// order is candidate-oriented, the same one the competition comparator
    /// gives a higher-is-better or lower-is-better objective.
    pub fn comparison(&self) -> &'static str {
        let Some(baseline) = &self.baseline else {
            return match self.dataset {
                Dataset::FullDevelopment => "reported",
                Dataset::Diagnostic => "sample",
                Dataset::OfficialReport => "reported",
            };
        };
        let order = if self.lower_is_better {
            baseline.numeric_cmp(&self.value)
        } else {
            self.value.numeric_cmp(baseline)
        };
        match order {
            Ordering::Greater => "better",
            Ordering::Less => "worse",
            Ordering::Equal => "same",
        }
    }
}

// ---- exact decimal scores (was drive::competition::schema) ----

const MAX_SCORE_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScoreV1(String);

impl ScoreV1 {
    pub fn new(value: impl Into<String>) -> Result<Self, ScoreError> {
        let value = value.into();
        validate_canonical_decimal(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn numeric_cmp(&self, other: &Self) -> Ordering {
        decimal_cmp(&self.0, &other.0)
    }
}

impl Serialize for ScoreV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ScoreV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreError;

impl fmt::Display for ScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("score must be a canonical finite decimal")
    }
}

fn validate_canonical_decimal(value: &str) -> Result<(), ScoreError> {
    if value.is_empty() || value.len() > MAX_SCORE_BYTES || value.starts_with('+') {
        return Err(ScoreError);
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let mut parts = unsigned.split('.');
    let integer = parts.next().ok_or(ScoreError)?;
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
        || fraction.is_some_and(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.ends_with('0')
        })
        || (value.starts_with('-') && unsigned == "0")
    {
        return Err(ScoreError);
    }
    Ok(())
}

fn decimal_cmp(left: &str, right: &str) -> Ordering {
    let left_negative = left.starts_with('-');
    let right_negative = right.starts_with('-');
    if left_negative != right_negative {
        return if left_negative {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    let magnitude =
        decimal_magnitude_cmp(left.trim_start_matches('-'), right.trim_start_matches('-'));
    if left_negative {
        magnitude.reverse()
    } else {
        magnitude
    }
}

fn decimal_magnitude_cmp(left: &str, right: &str) -> Ordering {
    let (left_integer, left_fraction) = left.split_once('.').unwrap_or((left, ""));
    let (right_integer, right_fraction) = right.split_once('.').unwrap_or((right, ""));
    left_integer
        .len()
        .cmp(&right_integer.len())
        .then_with(|| left_integer.cmp(right_integer))
        .then_with(|| {
            let width = left_fraction.len().max(right_fraction.len());
            (0..width)
                .map(|index| left_fraction.as_bytes().get(index).copied().unwrap_or(b'0'))
                .cmp((0..width).map(|index| {
                    right_fraction
                        .as_bytes()
                        .get(index)
                        .copied()
                        .unwrap_or(b'0')
                }))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurement(value: &str, baseline: Option<&str>, lower: bool, dataset: &str) -> Measurement {
        serde_json::from_value(serde_json::json!({
            "value": value, "baseline": baseline, "dataset": dataset,
            "lower_is_better": lower, "receipt_sha256": "a".repeat(64),
        }))
        .unwrap()
    }

    #[test]
    fn a_comparison_follows_the_exact_decimal_order_and_the_objective_direction() {
        let lower = measurement(
            "0.844070999999999999999",
            Some("0.844071"),
            true,
            "full_development",
        );
        assert_eq!(lower.comparison(), "better");
        let higher = measurement(
            "0.844070999999999999999",
            Some("0.844071"),
            false,
            "full_development",
        );
        assert_eq!(higher.comparison(), "worse");
        let same = measurement("2.5", Some("2.5"), true, "diagnostic");
        assert_eq!(same.comparison(), "same");
        assert_eq!(
            measurement("1", None, true, "diagnostic").comparison(),
            "sample"
        );
        assert_eq!(
            measurement("1", None, true, "official_report").comparison(),
            "reported"
        );
    }

    #[test]
    fn scores_stay_canonical_decimals_in_and_out() {
        assert!(ScoreV1::new("-0").is_err());
        assert!(ScoreV1::new("1.50").is_err());
        let score: ScoreV1 = serde_json::from_str("\"-12.5\"").unwrap();
        assert_eq!(serde_json::to_string(&score).unwrap(), "\"-12.5\"");
        assert_eq!(
            score.numeric_cmp(&ScoreV1::new("3").unwrap()),
            Ordering::Less
        );
    }

    #[test]
    fn graph_requests_keep_their_wire_shape_and_validation() {
        let request: GraphRequest = serde_json::from_value(serde_json::json!({
            "op": "finish", "plot": "loss-curve", "generation": 2,
        }))
        .unwrap();
        assert_eq!(request.plot(), "loss-curve");
        assert!(request.validate().is_ok());
        let bad = GraphRequest::Clear {
            plot: "Loss Curve".into(),
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn motion_modes_parse_as_the_env_spells_them() {
        assert_eq!(MotionMode::parse(" Reduced "), MotionMode::Reduced);
        assert_eq!(MotionMode::parse("off"), MotionMode::Off);
        assert_eq!(MotionMode::parse("anything"), MotionMode::Full);
        assert!(!MotionMode::Off.animates());
    }
}
