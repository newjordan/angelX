//! Realm/Reinforce stage: the live view of one integrated RL campaign.
//!
//! There is a single runtime path. `/rl run` freezes the case source, runs real
//! agent attempts on the selected club, measures each attempt with the
//! objective's own verifier through the case evaluator, gates the proposed
//! policy note on a receipt-backed promotion cohort (plus a veto-only audit when
//! the operator supplies one), and installs a released note through the
//! continual harness. The stage renders:
//! - the measured attempt series from `rl_ctl` — reward and the attempt's real
//!   cost (generation time plus verifier wall time), never a synthetic curve;
//! - the cohort lattice, case slots and gate/audit verdicts narrated by the
//!   structural telemetry spine (`reinforce::telemetry`).
//!
//! Pure render functions over snapshots; no state of its own.

use crate::hud;
use crate::reinforce::telemetry::{self, RlPhase, RlSnapshot};
use crate::rl_ctl::{CampaignOutcome, RlMode, RlState, RunProgress};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span, Text},
};

/// Read-only lenses over one RL run. The branch view preserves the live
/// operator graph; Research and Sankey reshape the same evidence for review.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RlView {
    #[default]
    Branch,
    Research,
    Sankey,
}

impl RlView {
    pub(crate) const ALL: [Self; 3] = [Self::Branch, Self::Research, Self::Sankey];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Branch => "BRANCH",
            Self::Research => "RESEARCH",
            Self::Sankey => "SANKEY",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "branch" | "tree" => Some(Self::Branch),
            "research" | "table" | "ledger" => Some(Self::Research),
            "sankey" | "flow" => Some(Self::Sankey),
            _ => None,
        }
    }

    pub(crate) fn step(self, delta: isize) -> Self {
        let current = Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

pub(crate) fn title(state: &RlState) -> String {
    match state.mode {
        RlMode::Idle => " reinforce · idle ".to_string(),
        RlMode::Campaign => {
            let progress = state.progress_snapshot();
            match &progress.outcome {
                None => {
                    let attempt = progress.observed_attempts();
                    format!(
                        " reinforce · campaign · attempt {attempt}/{} ",
                        progress.planned_attempts
                    )
                }
                Some(Ok(outcome)) if outcome.validated => {
                    format!(" reinforce · released v{} ", outcome.policy_version)
                }
                Some(Ok(outcome)) if outcome.promoted_rounds > 0 => {
                    format!(
                        " reinforce · measured v{}· not installed ",
                        outcome.policy_version
                    )
                }
                Some(Ok(_)) => " reinforce · incumbent retained ".to_string(),
                Some(Err(_)) => " reinforce · error ".to_string(),
            }
        }
    }
}

pub(crate) fn render_view(
    state: &RlState,
    view: RlView,
    time: f32,
    width: u16,
    height: u16,
) -> Text<'static> {
    let width = width as usize;
    let height = height as usize;
    if width < 12 || height == 0 {
        return Text::default();
    }
    let mut lines = match view {
        RlView::Branch => match state.mode {
            RlMode::Idle => idle_lines(width),
            RlMode::Campaign => attempt_lines(&state.progress_snapshot(), time, width, height),
        },
        RlView::Research => research_lines(state, width, height),
        RlView::Sankey => sankey_lines(state, width),
    };
    lines.truncate(height);
    Text::from(lines)
}

fn idle_lines(width: usize) -> Vec<Line<'static>> {
    let mut lines = vec![
        dim_line("No RL campaign is running.", width),
        Line::default(),
        text_line(
            "/rl run — measured campaign on the configured objective:",
            width,
        ),
        text_line(
            "  /goal <task> + /goal cmd <check>, or --task/--verify",
            width,
        ),
        text_line(
            "  real attempts run on the selected club; the case's own",
            width,
        ),
        text_line(
            "  verifier measures each attempt (evaluator-owned execution)",
            width,
        ),
        text_line(
            "  --verify-scope <path>  verifier-owned inputs; required",
            width,
        ),
        text_line(
            "  --audit \"<objective> :: <command> :: <source>\"  for an",
            width,
        ),
        text_line(
            "      independent audit that can release a policy note",
            width,
        ),
        text_line(
            "  without --audit: measured exploration, nothing installed",
            width,
        ),
        text_line(
            "  /rl status — signals and release trail · /rl stop — cancel",
            width,
        ),
    ];
    if let Some(summary) = crate::rl_ctl::authority_summary() {
        lines.push(Line::default());
        lines.push(text_line(
            &format!(
                "authority · {} campaign(s) · {} release(s) · {} poisoned slot(s)",
                summary.campaigns, summary.releases, summary.poisoned
            ),
            width,
        ));
    }
    lines
}

// ---------------------------------------------------------------------------
// Pipeline node row
// ---------------------------------------------------------------------------

/// Shared with `graph_viz` — the Round Table stage renders its DAG layers with
/// the same node cells and pulse.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeState {
    Pending,
    Active,
    Done,
    Failed,
    Rejected,
}

pub(crate) fn node_style(state: NodeState, time: f32) -> Style {
    match state {
        NodeState::Pending => Style::new().fg(hud::HUD_DIM),
        NodeState::Active => {
            // Lantern pulse on the live node so the graph reads as running.
            let bright = (time * 2.4).sin() > 0.0;
            let style = Style::new().fg(hud::HUD_GOLD).add_modifier(Modifier::BOLD);
            if bright {
                style
            } else {
                style.add_modifier(Modifier::DIM)
            }
        }
        NodeState::Done => Style::new().fg(hud::HUD_VERIFIED),
        NodeState::Failed => Style::new()
            .fg(hud::HUD_DANGER)
            .add_modifier(Modifier::BOLD),
        NodeState::Rejected => Style::new().fg(hud::HUD_DANGER),
    }
}

pub(crate) fn node_row(nodes: &[(&str, NodeState)], time: f32, width: usize) -> Line<'static> {
    let mut spans = Vec::with_capacity(nodes.len() * 2);
    let mut used = 0usize;
    for (index, (label, state)) in nodes.iter().enumerate() {
        let cell = format!("[{label}]");
        let link = if index + 1 < nodes.len() { "━" } else { "" };
        if used + cell.chars().count() + 1 > width {
            break;
        }
        used += cell.chars().count() + link.chars().count();
        spans.push(Span::styled(cell, node_style(*state, time)));
        if !link.is_empty() {
            spans.push(Span::styled(
                link.to_string(),
                Style::new().fg(hud::HUD_DIM),
            ));
        }
    }
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Campaign: measured attempts
// ---------------------------------------------------------------------------

fn attempt_lines(
    progress: &RunProgress,
    time: f32,
    width: usize,
    height: usize,
) -> Vec<Line<'static>> {
    let running = progress.outcome.is_none();
    let failed = matches!(progress.outcome, Some(Err(_)));
    let stage = |done: bool, active: bool| -> NodeState {
        if failed && active {
            NodeState::Failed
        } else if done {
            NodeState::Done
        } else if active {
            NodeState::Active
        } else {
            NodeState::Pending
        }
    };
    let started = !progress.points.is_empty() || !progress.log_tail.is_empty();
    let gated = progress.outcome.is_some();
    let mut lines = vec![node_row(
        &[
            ("attempt", stage(started, !started)),
            ("verify", stage(gated, started && running)),
            ("cohort", stage(gated && !failed, false)),
            (
                "install",
                if gated && progress.promoted {
                    NodeState::Done
                } else if failed {
                    NodeState::Failed
                } else {
                    NodeState::Pending
                },
            ),
        ],
        time,
        width,
    )];

    if let Some(point) = progress.points.last() {
        lines.push(text_line(
            &format!(
                "attempt {}/{} · reward {:.2} · {}ms",
                point.step, progress.planned_attempts, point.reward, point.latency_ms
            ),
            width,
        ));
    } else if running {
        lines.push(dim_line("first attempt running…", width));
    }

    let overhead = lines.len() + 2;
    let chart_h = height.saturating_sub(overhead + 1);
    if chart_h >= 3 && progress.points.len() >= 2 {
        let rewards: Vec<f32> = progress.points.iter().map(|point| point.reward).collect();
        let latency_h = if chart_h >= 8 { 3 } else { 0 };
        let reward_h = chart_h - latency_h;
        lines.push(caption_line(
            "measured reward",
            rewards.last().copied(),
            width,
        ));
        lines.extend(crate::chart::line_chart(&rewards, width as u16, reward_h as u16).lines);
        if latency_h > 0 {
            let latency: Vec<f32> = progress
                .points
                .iter()
                .map(|point| point.latency_ms as f32)
                .collect();
            // One definition: generation time plus the physical verifier's wall
            // time for that attempt (see `rl_ctl::RunPoint`).
            lines.push(caption_line(
                "attempt ms · generate+verify",
                latency.last().copied(),
                width,
            ));
            lines.extend(crate::chart::line_chart(&latency, width as u16, latency_h as u16).lines);
        }
    }

    match &progress.outcome {
        Some(Ok(outcome)) => lines.push(campaign_outcome_line(outcome, width)),
        Some(Err(error)) => lines.push(Line::from(Span::styled(
            fit(&format!("✗ {error}"), width),
            Style::new().fg(hud::HUD_DANGER),
        ))),
        None => {}
    }
    for log in &progress.log_tail {
        if lines.len() >= height {
            break;
        }
        lines.push(dim_line(log, width));
    }
    // The receipt-backed cohort narration, when the telemetry spine has a
    // measured campaign to describe.
    if telemetry::current().is_some() && lines.len() < height {
        lines.push(Line::default());
        lines.extend(campaign_lines(
            time,
            width,
            height.saturating_sub(lines.len()),
        ));
    }
    lines
}

fn campaign_outcome_line(outcome: &CampaignOutcome, width: usize) -> Line<'static> {
    let (mark, color) = if outcome.validated {
        ("✓ RELEASED · audited", hud::HUD_VERIFIED)
    } else if outcome.promoted_rounds > 0 {
        ("◆ MEASURED · not installed", hud::HUD_GOLD)
    } else {
        ("· INCUMBENT RETAINED", hud::HUD_DIM)
    };
    Line::from(Span::styled(
        fit(
            &format!(
                "{mark} · {} attempt(s) · {} passed · {} · delta {} · {:.0}s",
                outcome.attempted,
                outcome.passed,
                outcome.decision,
                outcome
                    .mean_delta
                    .map(|delta| format!("{delta:+.3}"))
                    .unwrap_or_else(|| "n/a".to_string()),
                outcome.wall_s
            ),
            width,
        ),
        Style::new().fg(color).add_modifier(Modifier::BOLD),
    ))
}

// ---------------------------------------------------------------------------
// Campaign cohort narration (telemetry spine)
// ---------------------------------------------------------------------------

fn campaign_lines(time: f32, width: usize, height: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let snapshot = telemetry::current();
    if let Some(snapshot) = &snapshot {
        lines.push(campaign_node_row(snapshot, time, width));
        lines.push(text_line(
            &format!(
                "{} · round {}/{} · policy v{}",
                snapshot.source, snapshot.round, snapshot.rounds, snapshot.policy_version
            ),
            width,
        ));
        if let Some(batch) = &snapshot.batch {
            lines.push(text_line(
                &format!(
                    "batch · {}/{} accepted ({} stale, {} slow) · solve {:.0}% · advar {:.4}",
                    batch.accepted,
                    batch.generated,
                    batch.rejected_stale,
                    batch.rejected_straggler,
                    batch.solve_rate * 100.0,
                    batch.advantage_variance
                ),
                width,
            ));
        }
        if !snapshot.cases.is_empty() {
            lines.push(dim_line(
                &format!(
                    "held-out cohort · {} · {} case(s)",
                    snapshot.cohort_role.as_deref().unwrap_or("promotion"),
                    snapshot.cohort_cases_total
                ),
                width,
            ));
            for case in &snapshot.cases {
                lines.push(case_line(case, width));
            }
        }
        for verdict in [snapshot.gate.as_ref(), snapshot.audit.as_ref()]
            .into_iter()
            .flatten()
        {
            lines.push(verdict_line(verdict, width));
        }
        if snapshot.released {
            lines.push(Line::from(Span::styled(
                fit("◆ release candidate minted", width),
                Style::new().fg(hud::HUD_GOLD).add_modifier(Modifier::BOLD),
            )));
        }
    } else {
        lines.push(dim_line("waiting for the first measured cohort…", width));
    }

    // Event timeline fills whatever height remains.
    if let Some(snapshot) = &snapshot {
        let remaining = height.saturating_sub(lines.len() + 1);
        if remaining >= 2 && !snapshot.events.is_empty() {
            lines.push(dim_line("─ timeline ─", width));
            let start = snapshot.events.len().saturating_sub(remaining);
            for event in snapshot.events.iter().skip(start) {
                lines.push(dim_line(&format!("· {}", event.label), width));
            }
        }
    }
    lines
}

fn campaign_node_row(snapshot: &RlSnapshot, time: f32, width: usize) -> Line<'static> {
    let technical = snapshot.source == "technical";
    let order = |phase: RlPhase| -> usize {
        match phase {
            RlPhase::Idle => 0,
            RlPhase::Generation => 0,
            RlPhase::Scoring => 1,
            RlPhase::Reflection => 2,
            RlPhase::Gate => 3,
            RlPhase::FinalAudit => 4,
            RlPhase::Release => 5,
            RlPhase::Done => usize::MAX,
            RlPhase::Failed => usize::MAX - 1,
        }
    };
    let current = order(snapshot.phase);
    let failed = snapshot.phase == RlPhase::Failed;
    let done = snapshot.phase == RlPhase::Done;
    let state_for = |index: usize| -> NodeState {
        if done {
            NodeState::Done
        } else if failed {
            if index == 0 {
                NodeState::Failed
            } else {
                NodeState::Pending
            }
        } else if index < current {
            NodeState::Done
        } else if index == current {
            NodeState::Active
        } else {
            NodeState::Pending
        }
    };
    let gate_state = || -> NodeState {
        match &snapshot.gate {
            Some(verdict) if !verdict.promoted && current >= 3 => NodeState::Rejected,
            _ => state_for(3),
        }
    };
    let mut nodes = vec![
        ("generate", state_for(0)),
        ("score", state_for(1)),
        ("reflect", state_for(2)),
        ("gate", gate_state()),
    ];
    if technical {
        nodes.push(("audit", state_for(4)));
        nodes.push((
            "release",
            if snapshot.released {
                NodeState::Done
            } else {
                state_for(5)
            },
        ));
    }
    node_row(&nodes, time, width)
}

fn case_line(case: &telemetry::CaseSlot, width: usize) -> Line<'static> {
    let bar = |observed: usize, requested: usize| -> String {
        let cells = 4usize;
        let filled = if requested == 0 {
            0
        } else {
            (observed * cells).div_ceil(requested).min(cells)
        };
        format!("{}{}", "█".repeat(filled), "░".repeat(cells - filled))
    };
    let delta = case
        .delta
        .map(|delta| format!(" Δ{delta:+.3}"))
        .unwrap_or_else(|| " Δ —".to_string());
    let style = if case.failed {
        Style::new().fg(hud::HUD_DANGER)
    } else if !case.complete {
        Style::new().fg(hud::HUD_GOLD)
    } else if case.delta.unwrap_or(0.0) >= 0.0 {
        Style::new().fg(hud::HUD_VERIFIED)
    } else {
        Style::new().fg(hud::HUD_DANGER)
    };
    Line::from(Span::styled(
        fit(
            &format!(
                "  {} · inc {} {}/{} · cand {} {}/{}{delta}",
                case.id,
                bar(case.incumbent_observed, case.requested_per_policy),
                case.incumbent_observed,
                case.requested_per_policy,
                bar(case.candidate_observed, case.requested_per_policy),
                case.candidate_observed,
                case.requested_per_policy,
            ),
            width,
        ),
        style,
    ))
}

fn verdict_line(verdict: &telemetry::CohortVerdict, width: usize) -> Line<'static> {
    let color = if verdict.promoted {
        hud::HUD_VERIFIED
    } else {
        hud::HUD_DANGER
    };
    let deltas = match (verdict.mean_delta, verdict.delta_lower_bound) {
        (Some(mean), Some(bound)) => format!(" · Δ̄{mean:+.3} (≥{bound:+.3})"),
        _ => String::new(),
    };
    Line::from(Span::styled(
        fit(
            &format!("{} · {}{deltas}", verdict.role, verdict.decision),
            width,
        ),
        Style::new().fg(color).add_modifier(Modifier::BOLD),
    ))
}

// ---------------------------------------------------------------------------
// Research ledger
// ---------------------------------------------------------------------------

fn research_lines(state: &RlState, width: usize, height: usize) -> Vec<Line<'static>> {
    match state.mode {
        RlMode::Idle => vec![
            heading_line("OPTIMIZATION LEDGER · NO ACTIVE EVIDENCE", width),
            dim_line(
                "Rows appear only after measured RL observations arrive.",
                width,
            ),
            Line::default(),
            text_line(
                "/rl run · attempts, their verifier verdicts and cost",
                width,
            ),
            text_line(
                "campaign rows · cohort cases, deltas and the audit verdict",
                width,
            ),
        ],
        RlMode::Campaign => research_campaign_lines(state, width, height),
    }
}
/// Measured-attempt ledger: the operator's own verifier verdict per attempt,
/// then the receipt-backed cohort detail when the telemetry spine has it.
fn research_campaign_lines(state: &RlState, width: usize, height: usize) -> Vec<Line<'static>> {
    let progress = state.progress_snapshot();
    let mut lines = vec![heading_line(
        "OPTIMIZATION LEDGER · MEASURED ATTEMPTS",
        width,
    )];
    let Some(first) = progress.points.first().copied() else {
        lines.push(dim_line("Waiting for the first measured attempt…", width));
        return lines;
    };
    let last = progress.points.last().copied().unwrap_or(first);
    let best = progress.best_reward().unwrap_or(first.reward);
    lines.push(text_line(
        &format!(
            "{} attempt(s) · reward {:.2} → {:.2} · best {:.2} · {} passed / {} red",
            progress.observed_attempts(),
            first.reward,
            last.reward,
            best,
            progress.passed,
            progress.red,
        ),
        width,
    ));
    match &progress.outcome {
        Some(Ok(outcome)) => lines.push(campaign_outcome_line(outcome, width)),
        Some(Err(error)) => lines.push(Line::from(Span::styled(
            fit(&format!("run error · {error}"), width),
            Style::new().fg(hud::HUD_DANGER),
        ))),
        None => lines.push(dim_line(
            &format!(
                "live · round {}/{} · attempt {}/{}",
                progress.rounds_done,
                progress.rounds_planned,
                progress.observed_attempts(),
                progress.planned_attempts
            ),
            width,
        )),
    }

    let available_rows = height.saturating_sub(lines.len() + 2);
    if available_rows > 1 {
        lines.push(table_header_line(attempt_table_header(width), width));
        let indices = sampled_indices(progress.points.len(), available_rows - 1);
        for index in indices {
            let point = progress.points[index];
            let is_last = index + 1 == progress.points.len();
            lines.push(attempt_row_line(
                point,
                first.reward,
                best,
                is_last,
                progress.outcome.is_none(),
                width,
            ));
        }
    }
    if telemetry::current().is_some() {
        lines.extend(research_cohort_lines(
            width,
            height.saturating_sub(lines.len()),
        ));
    }
    lines
}

fn attempt_table_header(width: usize) -> &'static str {
    if width >= 44 {
        " STEP   REWARD   Δ FIRST   LATENCY  STATE"
    } else if width >= 30 {
        " STEP   REWARD   Δ FIRST   STATE"
    } else {
        " STEP · REWARD · STATE"
    }
}

fn attempt_row_line(
    point: crate::rl_ctl::RunPoint,
    first_reward: f32,
    best_reward: f32,
    is_last: bool,
    running: bool,
    width: usize,
) -> Line<'static> {
    let (state, style) = if is_last && running {
        ("LIVE", Style::new().fg(hud::HUD_GOLD))
    } else if (point.reward - best_reward).abs() <= f32::EPSILON {
        ("BEST", Style::new().fg(hud::HUD_PHOSPHOR))
    } else if point.reward >= 1.0 {
        ("PASS", Style::new().fg(hud::HUD_VERIFIED))
    } else {
        ("RED", Style::new().fg(hud::HUD_DANGER))
    };
    let row = if width >= 44 {
        format!(
            "{:>5}  {:>7.2}  {:+9.2}  {:>7}ms  {state}",
            point.step,
            point.reward,
            point.reward - first_reward,
            point.latency_ms,
        )
    } else if width >= 30 {
        format!(
            "{:>5}  {:>7.2}  {:+9.2}  {state}",
            point.step,
            point.reward,
            point.reward - first_reward,
        )
    } else {
        format!("{:>5} · {:.2} · {state}", point.step, point.reward,)
    };
    Line::from(Span::styled(fit(&row, width), style))
}

fn research_cohort_lines(width: usize, height: usize) -> Vec<Line<'static>> {
    let mut lines = vec![heading_line(
        "OPTIMIZATION LEDGER · PROMOTION COHORT",
        width,
    )];
    let snapshot = telemetry::current();
    if let Some(snapshot) = &snapshot {
        lines.push(text_line(
            &format!(
                "{} · {} · round {}/{} · policy v{}",
                snapshot.source,
                snapshot.phase.label(),
                snapshot.round,
                snapshot.rounds,
                snapshot.policy_version
            ),
            width,
        ));
        if let Some(batch) = &snapshot.batch {
            lines.push(dim_line(
                &format!(
                    "batch · {}/{} accepted · {} stale · {} slow · solve {:.0}%",
                    batch.accepted,
                    batch.generated,
                    batch.rejected_stale,
                    batch.rejected_straggler,
                    batch.solve_rate * 100.0,
                ),
                width,
            ));
        }
        if !snapshot.cases.is_empty() && lines.len() < height {
            lines.push(table_header_line(
                if width >= 48 {
                    " CASE             INC   CAND    Δ SCORE  STATE"
                } else {
                    " CASE        INC  CAND   Δ SCORE"
                },
                width,
            ));
            let available = height.saturating_sub(lines.len());
            for case in snapshot.cases.iter().take(available) {
                lines.push(cohort_case_table_line(case, width));
            }
        }
        if let Some(verdict) = snapshot.audit.as_ref().or(snapshot.gate.as_ref())
            && lines.len() < height
        {
            lines.push(verdict_line(verdict, width));
        }
    } else {
        lines.push(dim_line("Waiting for promotion telemetry…", width));
    }

    lines
}

fn cohort_case_table_line(case: &telemetry::CaseSlot, width: usize) -> Line<'static> {
    let delta = case
        .delta
        .map(|value| format!("{value:+.3}"))
        .unwrap_or_else(|| "—".to_string());
    let (state, style) = if case.failed {
        ("FAILED", Style::new().fg(hud::HUD_DANGER))
    } else if !case.complete {
        ("PARTIAL", Style::new().fg(hud::HUD_GOLD))
    } else if case.delta.unwrap_or(0.0) >= 0.0 {
        ("CLEAR", Style::new().fg(hud::HUD_VERIFIED))
    } else {
        ("REGRESS", Style::new().fg(hud::HUD_DANGER))
    };
    let id_width = if width >= 48 { 16 } else { 11 };
    let id = fit(&case.id, id_width);
    let incumbent = format!("{}/{}", case.incumbent_observed, case.requested_per_policy);
    let candidate = format!("{}/{}", case.candidate_observed, case.requested_per_policy);
    let row = if width >= 48 {
        format!(" {id:<16} {incumbent:>5} {candidate:>6} {delta:>10}  {state}")
    } else {
        format!(" {id:<11} {incumbent:>4} {candidate:>5} {delta:>9}")
    };
    Line::from(Span::styled(fit(&row, width), style))
}

fn sampled_indices(len: usize, slots: usize) -> Vec<usize> {
    if len == 0 || slots == 0 {
        return Vec::new();
    }
    if len <= slots {
        return (0..len).collect();
    }
    if slots == 1 {
        return vec![len - 1];
    }
    let mut indices = Vec::with_capacity(slots);
    for slot in 0..slots {
        let index = slot * (len - 1) / (slots - 1);
        if indices.last().copied() != Some(index) {
            indices.push(index);
        }
    }
    indices
}

// ---------------------------------------------------------------------------
// Sankey flow
// ---------------------------------------------------------------------------

fn sankey_lines(state: &RlState, width: usize) -> Vec<Line<'static>> {
    match state.mode {
        RlMode::Idle => vec![
            heading_line("SANKEY · AWAITING MEASURED FLOW", width),
            dim_line("Schema only — no volumes are inferred.", width),
            Line::default(),
            text_line("GENERATE ━ SCORE ┳ PROMOTE", width),
            text_line("                 ┗ REJECT / ARCHIVE", width),
        ],
        RlMode::Campaign => sankey_promotion_lines(width),
    }
}

#[allow(clippy::too_many_arguments)]
fn flow_branch_line(
    branch: &str,
    label: &str,
    count: usize,
    total: usize,
    bar_width: usize,
    sink: &str,
    color: ratatui::style::Color,
    width: usize,
) -> Line<'static> {
    let percent = count as f32 / total.max(1) as f32 * 100.0;
    let filled = if count == 0 {
        0
    } else {
        (count * bar_width).div_ceil(total).max(1).min(bar_width)
    };
    let bar = if filled == 0 {
        "·".to_string()
    } else {
        "━".repeat(filled)
    };
    Line::from(Span::styled(
        fit(
            &format!("{branch} {label:<7} {count:>4} {percent:>5.1}% {bar}→ {sink}"),
            width,
        ),
        Style::new().fg(color),
    ))
}

fn sankey_promotion_lines(width: usize) -> Vec<Line<'static>> {
    let mut lines = vec![heading_line("SANKEY · PROMOTION FLOW", width)];
    let snapshot = telemetry::current();
    if let Some(snapshot) = &snapshot {
        lines.push(text_line(
            &format!(
                "{} · round {}/{} · {}",
                snapshot.source,
                snapshot.round,
                snapshot.rounds,
                snapshot.phase.label()
            ),
            width,
        ));
        if let Some(batch) = &snapshot.batch {
            let known = batch
                .accepted
                .saturating_add(batch.rejected_stale)
                .saturating_add(batch.rejected_straggler);
            let other = batch.generated.saturating_sub(known);
            let bar_width = width.saturating_sub(29).clamp(3, 18);
            lines.push(dim_line(
                &format!("GENERATE · {} rollouts ━┳", batch.generated),
                width,
            ));
            lines.push(flow_branch_line(
                "├",
                "ACCEPT",
                batch.accepted,
                batch.generated,
                bar_width,
                "score",
                hud::HUD_VERIFIED,
                width,
            ));
            lines.push(flow_branch_line(
                "├",
                "STALE",
                batch.rejected_stale,
                batch.generated,
                bar_width,
                "archive",
                hud::HUD_DANGER,
                width,
            ));
            lines.push(flow_branch_line(
                "├",
                "SLOW",
                batch.rejected_straggler,
                batch.generated,
                bar_width,
                "archive",
                hud::HUD_GOLD,
                width,
            ));
            if other > 0 {
                lines.push(flow_branch_line(
                    "└",
                    "OTHER",
                    other,
                    batch.generated,
                    bar_width,
                    "reject",
                    hud::HUD_DIM,
                    width,
                ));
            }
        } else {
            lines.push(dim_line("Waiting for batch-volume telemetry…", width));
        }
        let observed: usize = snapshot
            .cases
            .iter()
            .map(|case| case.candidate_observed)
            .sum();
        if observed > 0 {
            lines.push(text_line(
                &format!(
                    "SCORE ━ COHORT · {} case(s) / {observed} observations",
                    snapshot.cases.len()
                ),
                width,
            ));
        }
        if let Some(verdict) = snapshot.audit.as_ref().or(snapshot.gate.as_ref()) {
            let (destination, color) = if verdict.promoted {
                ("RELEASE", hud::HUD_VERIFIED)
            } else {
                ("REJECT / ARCHIVE", hud::HUD_DANGER)
            };
            lines.push(Line::from(Span::styled(
                fit(
                    &format!("COHORT ━ {} ━ {destination}", verdict.decision),
                    width,
                ),
                Style::new().fg(color).add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                fit("COHORT ━ GATE PENDING", width),
                Style::new().fg(hud::HUD_GOLD),
            )));
        }
    } else {
        lines.push(dim_line("Waiting for measured campaign flow…", width));
    }
    lines
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn heading_line(text: &str, width: usize) -> Line<'static> {
    Line::from(Span::styled(
        fit(text, width),
        Style::new().fg(hud::HUD_BLUE).add_modifier(Modifier::BOLD),
    ))
}

fn table_header_line(text: &str, width: usize) -> Line<'static> {
    Line::from(Span::styled(
        fit(text, width),
        Style::new()
            .fg(hud::HUD_DIM)
            .add_modifier(Modifier::UNDERLINED),
    ))
}

fn caption_line(label: &str, last: Option<f32>, width: usize) -> Line<'static> {
    let text = match last {
        Some(value) => format!("{label} · {value:.3}"),
        None => label.to_string(),
    };
    dim_line(&text, width)
}

fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else if width <= 1 {
        "~".to_string()
    } else {
        let mut out: String = text.chars().take(width - 1).collect();
        out.push('~');
        out
    }
}

fn text_line(text: &str, width: usize) -> Line<'static> {
    Line::from(Span::styled(
        fit(text, width),
        Style::new().fg(hud::HUD_TEXT),
    ))
}

fn dim_line(text: &str, width: usize) -> Line<'static> {
    Line::from(Span::styled(
        fit(text, width),
        Style::new().fg(hud::HUD_DIM),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flatten(text: &Text<'static>) -> String {
        text.lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn idle_renders_launch_hints() {
        let state = RlState::default();
        let flat = flatten(&render_view(&state, RlView::Branch, 0.0, 60, 14));
        assert!(flat.contains("/rl run"), "{flat}");
        assert!(flat.contains("--audit"), "{flat}");
        assert!(flat.contains("/rl status"), "{flat}");
    }

    fn measured_progress(rewards: &[f32]) -> RlState {
        let state = RlState::default();
        {
            let mut progress = state.progress.lock().unwrap();
            progress.planned_attempts = 12;
            progress.passed = rewards.iter().filter(|reward| **reward >= 1.0).count();
            progress.red = rewards.len() - progress.passed;
            progress.points = rewards
                .iter()
                .enumerate()
                .map(|(index, reward)| crate::rl_ctl::RunPoint {
                    step: index + 1,
                    reward: *reward,
                    latency_ms: 1_200 + index as u64 * 10,
                })
                .collect();
            progress
                .log_tail
                .push_back("attempt 1 · answer".to_string());
        }
        let mut state = state;
        state.mode = RlMode::Campaign;
        state
    }

    #[test]
    fn campaign_renders_measured_reward_curve_and_attempt_status() {
        let rewards: Vec<f32> = (1..=8).map(|step| step as f32 / 8.0).collect();
        let state = measured_progress(&rewards);
        let flat = flatten(&render_view(&state, RlView::Branch, 1.0, 52, 20));
        assert!(flat.contains("attempt 8/12"), "{flat}");
        assert!(flat.contains("measured reward"), "{flat}");
        assert!(
            flat.contains("[attempt]") && flat.contains("[install]"),
            "{flat}"
        );
        assert!(
            flat.chars()
                .any(|ch| ('\u{2801}'..='\u{28FF}').contains(&ch)),
            "expected braille dots in the reward curve: {flat}"
        );
    }

    #[test]
    fn released_campaign_banner_says_audited_and_installed() {
        let state = measured_progress(&[0.0, 1.0, 1.0, 1.0]);
        {
            let mut progress = state.progress.lock().unwrap();
            progress.promoted = true;
            progress.outcome = Some(Ok(CampaignOutcome {
                attempted: 4,
                passed: 3,
                red: 1,
                rounds: 1,
                promoted_rounds: 1,
                policy_version: 1,
                decision: "promoted".into(),
                mean_delta: Some(0.75),
                validated: true,
                audit_supplied: true,
                release_sha256: Some("a".repeat(64)),
                solve_rate: Some(0.5),
                advantage_variance: Some(0.25),
                reflection: true,
                accepted_entry: Some("rl-policy".into()),
                accepted_event: Some("r1".into()),
                report_path: "/tmp/rl/run-1".into(),
                route: crate::club::RouteIdentity {
                    driver: "fixture".into(),
                    model: None,
                    reasoning_effort: None,
                },
                wall_s: 42.0,
            }));
        }
        let flat = flatten(&render_view(&state, RlView::Branch, 0.0, 64, 12));
        assert!(flat.contains("RELEASED · audited"), "{flat}");
        assert!(title(&state).contains("released v1"), "{}", title(&state));
    }

    #[test]
    fn measured_but_uninstalled_campaign_is_not_called_released() {
        let state = measured_progress(&[1.0, 1.0]);
        {
            let mut progress = state.progress.lock().unwrap();
            progress.outcome = Some(Ok(CampaignOutcome {
                attempted: 2,
                passed: 2,
                red: 0,
                rounds: 1,
                promoted_rounds: 1,
                policy_version: 1,
                decision: "promoted".into(),
                mean_delta: Some(1.0),
                validated: false,
                audit_supplied: false,
                release_sha256: None,
                solve_rate: Some(1.0),
                advantage_variance: Some(0.0),
                reflection: true,
                accepted_entry: None,
                accepted_event: None,
                report_path: "/tmp/rl/run-2".into(),
                route: crate::club::RouteIdentity {
                    driver: "fixture".into(),
                    model: None,
                    reasoning_effort: None,
                },
                wall_s: 9.0,
            }));
        }
        let flat = flatten(&render_view(&state, RlView::Branch, 0.0, 64, 12));
        assert!(flat.contains("MEASURED · not installed"), "{flat}");
        assert!(!flat.contains("RELEASED"), "{flat}");
        assert!(title(&state).contains("not installed"), "{}", title(&state));
    }

    #[test]
    fn research_view_is_a_sampled_measured_ledger() {
        let rewards: Vec<f32> = (1..=120)
            .map(|step| if step % 3 == 0 { 1.0 } else { 0.0 })
            .collect();
        let state = measured_progress(&rewards);
        let flat = flatten(&render_view(&state, RlView::Research, 0.0, 64, 14));
        assert!(flat.contains("OPTIMIZATION LEDGER"), "{flat}");
        assert!(
            flat.contains("REWARD") && flat.contains("LATENCY"),
            "{flat}"
        );
        assert!(
            flat.contains("120"),
            "last attempt must survive sampling\n{flat}"
        );
        assert!(flat.contains("PASS") || flat.contains("RED"), "{flat}");
    }

    #[test]
    fn sankey_view_has_no_flow_before_a_cohort_telemetry_snapshot() {
        let state = measured_progress(&[0.0, 1.0]);
        let flat = flatten(&render_view(&state, RlView::Sankey, 0.0, 64, 12));
        assert!(flat.contains("SANKEY"), "{flat}");
        assert!(!flat.contains("HELD-OUT"), "{flat}");
    }

    #[test]
    fn rl_view_steps_wrap_in_both_directions() {
        assert_eq!(RlView::Branch.step(-1), RlView::Sankey);
        assert_eq!(RlView::Sankey.step(1), RlView::Branch);
    }

    #[test]
    fn render_respects_height_budget() {
        let state = measured_progress(&[0.0, 1.0, 1.0]);
        state.progress.lock().unwrap().outcome = None;
        for height in [1u16, 3, 6, 24] {
            for view in RlView::ALL {
                let text = render_view(&state, view, 0.0, 40, height);
                assert!(text.lines.len() <= height as usize);
            }
        }
    }

    #[test]
    fn campaign_row_marks_rejected_gate() {
        let snapshot = RlSnapshot {
            source: "nontechnical".into(),
            phase: RlPhase::Done,
            gate: Some(telemetry::CohortVerdict {
                role: "promotion".into(),
                decision: "rejected-below-floor".into(),
                promoted: false,
                mean_delta: Some(-0.5),
                delta_lower_bound: Some(-0.6),
            }),
            ..RlSnapshot::default()
        };
        let line = campaign_node_row(&snapshot, 0.0, 60);
        let flat: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(flat.contains("[gate]"), "{flat}");
        assert!(
            !flat.contains("[audit]"),
            "nontechnical run has no audit node: {flat}"
        );
    }
}
