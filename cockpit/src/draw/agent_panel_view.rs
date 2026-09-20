//! Agent inspector and persistent route-control presentation.
//!
//! Agent state and input behavior remain on `App`; this module owns the
//! terminal-native identity, portrait, telemetry, feedback, and persistent
//! provider-exposed reasoning surfaces.

use super::*;

use ratatui::buffer::CellWidth;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::collections::VecDeque;
use std::ops::Range;

fn extend_reasoning_range(into: &mut Option<Range<usize>>, range: Range<usize>) {
    match into {
        Some(current) => current.end = range.end,
        None => *into = Some(range),
    }
}

/// Index plain `trim: false` word wrapping using Ratatui's public grapheme
/// semantics. Ranges keep source clusters intact; only visible rows are painted.
fn reasoning_row_ranges(text: &str, width: u16) -> Vec<Range<usize>> {
    if text.is_empty() || width == 0 {
        return Vec::new();
    }
    let width = usize::from(width);
    let mut rows = Vec::new();
    for physical in text.lines() {
        let origin = physical.as_ptr() as usize - text.as_ptr() as usize;
        let before = rows.len();
        let mut line: Option<Range<usize>> = None;
        let mut word: Option<Range<usize>> = None;
        let mut whitespace: VecDeque<(Range<usize>, usize)> = VecDeque::new();
        let (mut line_width, mut word_width, mut whitespace_width) = (0, 0, 0);
        let mut previous_word = false;
        let source = Line::raw(physical);
        for glyph in source.styled_graphemes(Style::default()) {
            let cells = usize::from(glyph.symbol.cell_width());
            if cells > width {
                continue;
            }
            let start = glyph.symbol.as_ptr() as usize - text.as_ptr() as usize;
            let range = start..start + glyph.symbol.len();
            let is_whitespace = glyph.is_whitespace();
            let finished_word = previous_word && is_whitespace;
            let first_segment_overflow =
                line.is_none() && word_width + whitespace_width + cells > width;
            if finished_word || first_segment_overflow {
                for (range, _) in whitespace.drain(..) {
                    extend_reasoning_range(&mut line, range);
                }
                line_width += whitespace_width;
                if let Some(range) = word.take() {
                    extend_reasoning_range(&mut line, range);
                }
                line_width += word_width;
                whitespace_width = 0;
                word_width = 0;
            }
            let line_full = line_width >= width;
            // Include the incoming cluster's full width. Ratatui 0.30's
            // >= limit check can put a two-cell glyph in the final column.
            let pending_word_overflow =
                cells > 0 && line_width + whitespace_width + word_width + cells > width;
            if line_full || pending_word_overflow {
                let mut remaining = width.saturating_sub(line_width);
                rows.push(line.take().unwrap_or(origin..origin));
                line_width = 0;
                while let Some((_, cells)) = whitespace.front() {
                    if *cells > remaining {
                        break;
                    }
                    whitespace_width -= *cells;
                    remaining -= *cells;
                    whitespace.pop_front();
                }
                if is_whitespace && whitespace.is_empty() {
                    continue;
                }
            }
            if is_whitespace {
                whitespace_width += cells;
                whitespace.push_back((range, cells));
            } else {
                word_width += cells;
                extend_reasoning_range(&mut word, range);
            }
            previous_word = !is_whitespace;
        }
        for (range, _) in whitespace {
            extend_reasoning_range(&mut line, range);
        }
        if let Some(range) = word {
            extend_reasoning_range(&mut line, range);
        }
        if let Some(range) = line {
            rows.push(range);
        } else if rows.len() == before {
            rows.push(origin..origin);
        }
    }
    rows
}

fn reasoning_visible_rows<'a>(
    text: &'a str,
    rows: &[Range<usize>],
    start: usize,
    height: usize,
    width: u16,
) -> Vec<Line<'a>> {
    rows.iter()
        .skip(start)
        .take(height)
        .map(|range| {
            let source = Line::raw(&text[range.clone()]);
            Line::from(
                source
                    .styled_graphemes(Style::default())
                    .filter(|glyph| (1..=width).contains(&glyph.symbol.cell_width()))
                    .map(|glyph| {
                        let start = glyph.symbol.as_ptr() as usize - text.as_ptr() as usize;
                        Span::raw(&text[start..start + glyph.symbol.len()])
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn reasoning_flow_suffix_fits(settled: &str, fresh: &str, cells: usize) -> bool {
    // The decorative splitter counts chars. Keep uncertain cluster boundaries
    // and wider tails in the normal grapheme-safe paragraph instead.
    fresh.is_ascii()
        && fresh.len() <= cells
        && !fresh.contains(['\n', '\r'])
        && settled.chars().next_back().is_none_or(|ch| ch.is_ascii())
}

pub(crate) fn profile_badge(profile: AgentProfile) -> AgentBadge {
    match profile.key {
        AgentKey::Turbo => AgentBadge::Turbo,
        AgentKey::Atlas => AgentBadge::Atlas,
        AgentKey::Sparky => AgentBadge::Sparky,
        AgentKey::Apollo => AgentBadge::Apollo,
        // Codex has its own portrait but reuses the generic glyph badge — adding a
        // distinct terminal mark would ripple through glyphs.rs's badge table.
        AgentKey::Codex => AgentBadge::Unknown,
        AgentKey::GpuComp => AgentBadge::Turbo,
        AgentKey::MathGod => AgentBadge::Unknown,
        AgentKey::Unknown => AgentBadge::Unknown,
    }
}

fn agent_profile_caption_label(app: &App) -> String {
    if let Some(preview) = app.agent_menu_profile_preview() {
        return format!("preview {preview}");
    }
    // Live checkpoint/mode — not the machine slug sitting next to "Sparky".
    if let Some(mode) = app.bag.in_hand_chrome().mode.clone() {
        return mode;
    }
    if let Some(thinking) = &app.thinking
        && let Some(model) = thinking.requested_route.model.as_deref()
    {
        return model.to_string();
    }
    String::new()
}

pub(crate) fn render_agent_header_info(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let profile = app.active_profile();
    let active = portrait_active(app);
    let caption = agent_profile_caption_label(app);
    let mut profile_lines = bay_profile_lines(app, profile, &caption, active);
    let identity = profile_lines.remove(0);
    let state = profile_lines.remove(0);
    let mut detail = profile_lines.remove(0);
    // Treebeard / RLM lane is a process env policy — surface it so the operator
    // sees Hi/Q is active without digging into trajectories. Prefer last-turn
    // root offload_ratio (true LID mass); fall back to session handle pulse.
    // Living B200 peer geomean joins the strip when popcorn-peer state exists.
    if crate::harness::is_treebeard() {
        detail
            .spans
            .push(Span::styled("  ·  ", Style::new().fg(HUD_DIM)));
        let label = treebeard_header_strip_label();
        detail.spans.push(Span::styled(
            label,
            Style::new().fg(HUD_PHOSPHOR).add_modifier(Modifier::BOLD),
        ));
    }
    let mut identity_and_state = identity.spans;
    identity_and_state.push(Span::styled("  ", Style::new().fg(HUD_DIM)));
    identity_and_state.extend(state.spans);

    let mut telemetry = Vec::new();
    if let Some(metrics) = agent_host_metrics_line(app, active) {
        telemetry.push(Span::raw(metrics));
    }
    if let Some(token_line) = agent_token_lines(app, area.width, 1).into_iter().next() {
        telemetry.push(Span::styled("  ", Style::new().fg(HUD_DIM)));
        telemetry.extend(token_line.spans);
    }

    let (context_pressure, capability_line) =
        agent_route_capability_paint(app, area.width as usize);
    let route_receipt = fresh_brain_route_receipt(app).map(|label| format!("route set · {label}"));
    let mut route = Vec::new();
    if let Some(receipt) = route_receipt {
        route.push(Span::styled(
            receipt,
            Style::new().fg(HUD_PHOSPHOR).add_modifier(Modifier::BOLD),
        ));
        if capability_line.is_some() {
            route.push(Span::styled("  ", Style::new().fg(HUD_DIM)));
        }
    }
    if let Some(capability) = capability_line {
        let capability_style = match context_pressure {
            ContextPressure::Normal => Style::new().fg(HUD_BLUE),
            ContextPressure::Warning(_) => Style::new()
                .fg(ratatui::style::Color::Yellow)
                .add_modifier(Modifier::BOLD),
            ContextPressure::Critical(_) => Style::new()
                .fg(ratatui::style::Color::Red)
                .add_modifier(Modifier::BOLD),
        };
        route.push(Span::styled(capability, capability_style));
    }

    let lines = vec![
        Line::from(identity_and_state),
        detail,
        Line::from(telemetry),
        Line::from(route),
    ];
    frame.render_widget(Paragraph::new(lines).style(dim_panel_style()), area);
}

/// One env gate for the six-state portrait. `ANGEL_PORTRAIT_STATES=0` opts
/// out, restoring the legacy active/idle caption wrapper and the untinted
/// border. Read once per process — flipping it means a relaunch, exactly like
/// the other visual-motion knobs.
pub(crate) fn portrait_states_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("ANGEL_PORTRAIT_STATES").map_or(true, |v| v != "0"))
}

/// Fold live App signals into the six-state portrait. Sampling only — the
/// precedence ladder and marker expiry are the pure `agent_view` fns, so the
/// whole state machine tests without a terminal.
pub(crate) fn portrait_state(app: &App) -> agent_view::PortraitState {
    agent_view::portrait_state_from(
        app.pending_approval.is_some(),
        app.tool_strip.current().is_some_and(|entry| !entry.done),
        app.thinking.is_some(),
        app.portrait_turn_marker
            .map(|(marker, at)| (marker, at.elapsed())),
    )
}

/// Caption lines for the bay/header: six-state unless the operator opted out
/// (`ANGEL_PORTRAIT_STATES=0`), which keeps the legacy two-signal wrapper.
fn bay_profile_lines<'a>(
    app: &App,
    profile: AgentProfile,
    label: &'a str,
    active: bool,
) -> Vec<Line<'a>> {
    if portrait_states_enabled() {
        agent_view::compact_profile_lines_stateful(profile, label, portrait_state(app))
    } else {
        agent_view::compact_profile_lines(profile, label, active)
    }
}

pub(crate) fn portrait_active(app: &App) -> bool {
    if app.thinking.is_some() {
        return true;
    }
    if !app.reasoning_roll_pending() {
        return false;
    }
    // A finished turn may keep rolling its reasoning after the answer lands.
    // Keep that portrait energized only while the selected visual identity is
    // still the one that produced the answer; switching routes must not make a
    // fresh portrait glow from the previous model's private trace.
    app.last_completed_route.as_ref().is_some_and(|last| {
        profile_for(&last.route.driver, false).key
            == profile_for(app.bag.in_hand_label(), false).key
    })
}

pub(crate) fn label_has_moa(label: &str) -> bool {
    label
        .as_bytes()
        .windows(3)
        .any(|window| window.eq_ignore_ascii_case(b"moa"))
}

fn active_agent_is_moa(app: &App, mode: Option<&str>) -> bool {
    if let Some(thinking) = &app.thinking {
        return label_has_moa(&thinking.club_label)
            || thinking
                .club
                .as_ref()
                .is_some_and(|c| label_has_moa(c.label()));
    }
    label_has_moa(app.bag.in_hand_label()) || mode.is_some_and(label_has_moa)
}

/// Token meters and MoA ledger reports. Comp / lean and backdrop-off skip
/// the filesystem read and string build; default bay still paints them.
pub(crate) fn agent_token_meter_allowed() -> bool {
    crate::App::side_column_visuals_allowed()
}

/// Bay/header-info CPU/mem/gpu strip. Comp / lean and backdrop-off skip
/// the format; the main header still paints host load. Default bay still
/// paints the duplicate strip.
pub(crate) fn agent_host_metrics_allowed() -> bool {
    crate::App::side_column_visuals_allowed()
}

/// Optional host-metrics line. Hidden / comp-mode return None without
/// formatting percents.
pub(crate) fn agent_host_metrics_line(
    app: &App,
    active: bool,
) -> Option<std::borrow::Cow<'static, str>> {
    if !agent_host_metrics_allowed() {
        return None;
    }
    Some(status_view::agent_metrics_line(
        app.overwatch.snapshot.cpu_pct,
        app.overwatch.snapshot.mem_pct,
        app.overwatch.snapshot.gpu_pct,
        active,
    ))
}

/// Ctx/speed capability essay. Comp / lean and backdrop-off skip the
/// club-lock metadata walk and string build; default bay still paints it.
pub(crate) fn agent_route_capability_allowed() -> bool {
    crate::App::side_column_visuals_allowed()
}

/// Bay/header capability line. Hidden / comp-mode return None without
/// taking a club lock.
pub(crate) fn agent_route_capability_paint(
    app: &App,
    width: usize,
) -> (ContextPressure, Option<String>) {
    if !agent_route_capability_allowed() {
        return (ContextPressure::Normal, None);
    }
    let context_used = app
        .tools
        .gauge
        .used_tokens
        .load(std::sync::atomic::Ordering::Relaxed);
    let route_metadata = app.bag.in_hand().header_route_metadata();
    (
        context_pressure(&route_metadata, context_used),
        compact_header_route_capability_line(&route_metadata, context_used, width),
    )
}

pub(crate) fn agent_token_lines(app: &App, width: u16, max_lines: usize) -> Vec<Line<'static>> {
    if max_lines == 0 || !agent_token_meter_allowed() {
        return Vec::new();
    }
    // One chrome snapshot for the idle MoA/SOTA checks. The thinking path
    // already has club_label; do not clone in-hand mode twice per meter frame.
    let idle_chrome = (app.thinking.is_none()).then(|| app.bag.in_hand_chrome());
    let idle_mode = idle_chrome
        .as_ref()
        .and_then(|chrome| chrome.mode.as_deref());
    if active_agent_is_moa(app, idle_mode)
        && let Some(report) = crate::swarm::ledger::token_report(app.tools.current_workspace())
    {
        let lines = status_view::moa_token_report_lines(&report, width, max_lines);
        if !lines.is_empty() {
            return lines;
        }
    }

    let usage_and_sota = if let Some(thinking) = &app.thinking {
        let usage = thinking.club.as_ref().and_then(|c| c.token_usage());
        let is_sota = thinking
            .club
            .as_ref()
            .is_some_and(|c| club::is_sota_label(c.label()))
            || club::is_sota_label(&thinking.club_label)
            || usage.is_some();
        (usage, is_sota)
    } else {
        let in_hand = app.bag.in_hand();
        let usage = in_hand.token_usage();
        let is_sota = club::is_sota_label(in_hand.label())
            || club::is_sota_label(app.bag.in_hand_label())
            || idle_mode.is_some_and(club::is_sota_label)
            || usage.is_some();
        (usage, is_sota)
    };
    if usage_and_sota.1 {
        vec![Line::from(status_view::token_usage_meter(usage_and_sota.0))]
    } else {
        Vec::new()
    }
}

/// Kitty portraits and the WebGPU portal are side-column compose.
/// Comp / lean and backdrop-off skip the encode; default bay still paints.
pub(crate) fn side_column_kitty_compose_allowed() -> bool {
    crate::App::side_column_visuals_allowed()
}

/// Skip Kitty encode when the side column is hidden or Comp / lean is armed.
pub(crate) fn maybe_paint_side_column_kitty<F, T>(paint: F) -> Option<T>
where
    F: FnOnce() -> T,
{
    if side_column_kitty_compose_allowed() {
        Some(paint())
    } else {
        None
    }
}

fn render_terminal_agent_portrait(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    profile: AgentProfile,
    chrome: &FrameChrome,
) -> bool {
    maybe_paint_side_column_kitty(|| {
        if portrait_states_enabled() {
            let state = portrait_state(app);
            let long_context = !matches!(
                agent_route_capability_paint(app, 1).0,
                ContextPressure::Normal
            );
            let pose = crate::helm::pose(
                state,
                long_context,
                app.visual_motion,
                app.started.elapsed(),
            );
            let path = crate::runtime_paths::cockpit_dir().join(crate::helm::sheet(profile.key));
            let rendered = app.viewer.render_helm(frame, area, &path, pose as u8);
            // Warm the next real idle pose without changing the visible state.
            let next = if state == agent_view::PortraitState::Idle
                && app.visual_motion == lifecycle_viz::MotionMode::Full
            {
                crate::helm::pose(
                    state,
                    false,
                    app.visual_motion,
                    app.started.elapsed() + Duration::from_secs(2),
                )
            } else {
                crate::helm::Pose::Idle
            };
            app.viewer.prefetch_helm(area, &path, next as u8);
            return rendered;
        }
        let high_effort = portrait_uses_high_effort(chrome.effort());
        let Some(asset) = profile.asset(high_effort) else {
            return false;
        };
        let path = crate::runtime_paths::cockpit_dir().join(asset);
        let rendered = app.viewer.render_portrait(frame, area, &path);
        if let Some(opposite) = profile
            .asset(!high_effort)
            .filter(|opposite| *opposite != asset)
        {
            let opposite = crate::runtime_paths::cockpit_dir().join(opposite);
            app.viewer.prefetch_portrait(area, &opposite);
        }
        rendered
    })
    .unwrap_or(false)
}

/// Outer header box height (borders included) for the thin identity/chip strip.
pub(crate) const HEADER_PORTRAIT_BOX_H: u16 = 3;
const HEADER_PORTRAIT_MIN_INNER_W: u16 = 24;
const HEADER_PORTRAIT_MIN_INNER_H: u16 = 1;

pub(crate) fn header_portrait_bar_fits(area: Rect) -> bool {
    area.width >= HEADER_PORTRAIT_MIN_INNER_W && area.height == HEADER_PORTRAIT_MIN_INNER_H
}

/// Angel0/version left, MODEL/THINK/FORMATION chips in the remaining space,
/// workspace on the right. Portrait paint stays in the agent bay.
pub(crate) fn render_header_portrait_bar(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    chrome: &FrameChrome,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let version = env!("CARGO_PKG_VERSION");
    let save_degraded = matches!(
        app.session.save_status(),
        crate::session::SessionSaveStatus::Failed(_)
    );
    let ident = if save_degraded {
        "SESSION SAVE DEGRADED".to_string()
    } else {
        format!("angel0 {version}")
    };
    let mut ident_w = control_width_u16(&ident).saturating_add(1);
    let context = super::header_workspace_context(app);
    let mut context_w = if context.is_empty() {
        0
    } else {
        control_width_u16(&context).saturating_add(2)
    };
    // Keep the actual route controls clickable; drop workspace then identity
    // before starving MODEL/THINK/FORMATION.
    let route = app
        .moa_one_shot
        .as_ref()
        .or(app.moa_session.as_ref())
        .map(|engagement| crate::formations::formation(engagement.formation).name)
        .or(chrome.mode())
        .unwrap_or_else(|| app.bag.in_hand_label());
    let effort = chrome.effort().unwrap_or("native");
    let model_need = control_width(route) + 10;
    let think_need = control_width(effort) + 10 + if chrome.effort_selectable() { 0 } else { 6 };
    let formation_need = control_width(&app.moa_formation_chip_label()) + 4;
    let min_chips =
        (model_need + think_need + formation_need + 2).min(usize::from(area.width)) as u16;
    let chrome_w = ident_w.saturating_add(context_w);
    if chrome_w.saturating_add(min_chips) > area.width {
        let overflow = chrome_w.saturating_add(min_chips) - area.width;
        let drop_context = context_w.min(overflow);
        context_w -= drop_context;
        ident_w = ident_w
            .saturating_sub(overflow - drop_context)
            .max(6.min(area.width));
    }
    let chips_w = area.width.saturating_sub(ident_w.saturating_add(context_w));
    let ident_area = Rect {
        width: ident_w,
        ..area
    };
    let chips_area = Rect {
        x: area.x.saturating_add(ident_w),
        y: area.y,
        width: chips_w,
        height: area.height,
    };
    let context_area = Rect {
        x: area.x.saturating_add(ident_w).saturating_add(chips_w),
        y: area.y,
        width: context_w,
        height: area.height,
    };

    let ident_spans = if save_degraded {
        vec![Span::styled(
            ident,
            Style::new()
                .fg(hud::HUD_DANGER)
                .add_modifier(Modifier::BOLD),
        )]
    } else {
        vec![
            Span::styled(
                "angel0",
                Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {version}"), Style::new().fg(HUD_DIM)),
        ]
    };
    frame.render_widget(
        Paragraph::new(Line::from(ident_spans)).style(chrome_style()),
        ident_area,
    );
    render_agent_controls_in(frame, app, chips_area, chrome);
    if context_w > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("▸ ", Style::new().fg(HUD_DIM)),
                Span::styled(context, Style::new().fg(hud::HUD_GOLD)),
            ]))
            .alignment(Alignment::Right)
            .style(chrome_style()),
            context_area,
        );
    }
}

pub(crate) fn truncate_control_value(value: &str, max_cells: usize) -> String {
    if unicode_width::UnicodeWidthStr::width(value) <= max_cells {
        return value.to_string();
    }
    if max_cells == 0 {
        return String::new();
    }
    if max_cells == 1 {
        return "…".to_string();
    }
    // Preserve both the route's family and differentiating slug tail. A simple
    // right ellipsis made gpt-5.6-sol / terra / luna identical in narrow panes.
    let keep = max_cells - 1;
    let head_budget = keep / 2;
    let tail_budget = keep - head_budget;
    let mut head = String::new();
    let mut head_cells = 0usize;
    for ch in value.chars() {
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if head_cells.saturating_add(width) > head_budget {
            break;
        }
        head.push(ch);
        head_cells += width;
    }
    let mut tail = Vec::new();
    let mut tail_cells = 0usize;
    for ch in value.chars().rev() {
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if tail_cells.saturating_add(width) > tail_budget {
            break;
        }
        tail.push(ch);
        tail_cells += width;
    }
    tail.reverse();
    let mut out = head;
    out.push('…');
    out.extend(tail);
    out
}

fn control_width(value: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(value)
}

fn control_width_u16(value: &str) -> u16 {
    control_width(value).min(u16::MAX as usize) as u16
}

fn pending_feedback_route(app: &App) -> Option<String> {
    let last = app
        .last_completed_route
        .as_ref()
        .filter(|last| last.verdict.is_none())?;
    let mut label = last
        .route
        .model
        .clone()
        .unwrap_or_else(|| last.route.driver.clone());
    if let Some(effort) = last.route.reasoning_effort.as_deref() {
        label.push('@');
        label.push_str(effort);
    }
    Some(label)
}

const BRAIN_ROUTE_RECEIPT_LIFETIME: Duration = Duration::from_secs(3);

fn brain_route_receipt_is_fresh(app: &mut App) -> bool {
    let Some(receipt) = app.brain_route_receipt.as_ref() else {
        return false;
    };
    if receipt.applied_at.elapsed() < BRAIN_ROUTE_RECEIPT_LIFETIME {
        return true;
    }
    app.brain_route_receipt = None;
    false
}

fn fresh_brain_route_receipt(app: &mut App) -> Option<String> {
    brain_route_receipt_is_fresh(app)
        .then(|| {
            app.brain_route_receipt
                .as_ref()
                .map(|receipt| receipt.label.clone())
        })
        .flatten()
}

/// Persistent pilot-console rail. These labels describe the actual Bag route
/// and the backend-owned effort; clicks are registered only while changing that
/// state is safe. During a turn the route is locked and labelled RUN.
///
/// Test-frozen entry: standalone callers snapshot the chrome themselves. The
/// live frame path shares `ui`'s one snapshot via [`render_agent_controls_in`].
#[cfg(test)]
pub(crate) fn render_agent_controls(frame: &mut Frame, app: &mut App, area: Rect) {
    let chrome = FrameChrome::compute(&app.bag);
    render_agent_controls_in(frame, app, area, &chrome);
}

pub(crate) fn render_agent_controls_in(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    chrome: &FrameChrome,
) {
    if area.width < 12 || area.height == 0 {
        return;
    }
    let busy = app.thinking.is_some() || app.bg_job.is_some();
    let route_just_set = brain_route_receipt_is_fresh(app);
    // The first rail rendered this frame is the menu anchor. On roomy layouts
    // that is the header toolbar, so clicking there opens the deck directly
    // beneath it; compact layouts fall back to the agent-bay rail.
    if app.agent_control_area.is_none() {
        app.agent_control_area = Some(area);
    }
    // A formation armed for this turn (or the session) *is* the route. Every
    // formation runs through the same bag slot, which is named `sota-moa`, so
    // showing the raw slot label reads as "it abandoned my roster and went back
    // to the default mixture" — the exact misread that makes an engaged Tag Team
    // look broken while its seats are in fact running.
    let engaged_formation = app
        .moa_one_shot
        .as_ref()
        .or(app.moa_session.as_ref())
        .map(|engagement| crate::formations::formation(engagement.formation).name);
    let route = engaged_formation
        .or(chrome.mode())
        .unwrap_or_else(|| app.bag.in_hand_label());
    let effort_selectable = chrome.effort_selectable();
    let route_inspectable = app.bag.has_route_choices();
    let fallback_armed = crate::club::fallback_armed();
    let (moa_scope, moa_formation) = moa_chip_identity(app);
    let model_tag = if busy {
        "RUN"
    } else if route_just_set {
        "SET"
    } else if fallback_armed {
        "PREF"
    } else {
        "MODEL"
    };
    let think_enabled = !busy && effort_selectable;
    let model_enabled = !busy && route_inspectable;
    let stacked = area.height >= 2 && area.width < 38;
    let include_think = stacked || area.width >= 24;
    let include_moa = !stacked && area.width >= 38;
    let cache_hit = app.agent_control_chips.as_ref().is_some_and(|cache| {
        cache.width == area.width
            && cache.height == area.height
            && cache.busy == busy
            && cache.route_just_set == route_just_set
            && cache.fallback_armed == fallback_armed
            && cache.effort_selectable == effort_selectable
            && cache.mode.as_deref() == chrome.mode()
            && cache.effort.as_deref() == chrome.effort()
            && cache.route == route
            && cache.has_route_choices == route_inspectable
            && cache.moa_scope == moa_scope
            && cache.moa_formation == moa_formation
    });
    if !cache_hit {
        let effort = match chrome.effort() {
            None => if label_has_moa(route) {
                "mixed"
            } else {
                "native"
            }
            .to_string(),
            Some(effort) if effort_selectable => effort.to_string(),
            Some(effort) => format!("{effort}·fixed"),
        };
        let think_text = if think_enabled {
            format!("[THINK:{effort} ▾]")
        } else {
            format!("[THINK:{effort}]")
        };
        let moa_label = app.moa_formation_chip_label();
        let moa_text = if busy {
            format!("[{moa_label}:EDIT ▾]")
        } else {
            format!("[{moa_label} ▾]")
        };
        let suffix_width = if stacked {
            0
        } else {
            usize::from(include_think) * (control_width(&think_text) + 1)
                + usize::from(include_moa) * (control_width(&moa_text) + 1)
        };
        let model_drop = if model_enabled { " ▾" } else { "" };
        let model_shell = control_width(model_tag) + control_width(model_drop) + 3;
        let model_value_width = (area.width as usize)
            .saturating_sub(suffix_width + model_shell)
            .max(1);
        let model_value = truncate_control_value(route, model_value_width);
        let model_text = format!("[{model_tag}:{model_value}{model_drop}]");
        app.agent_control_chips = Some(crate::app::AgentControlChipCache {
            width: area.width,
            height: area.height,
            busy,
            route_just_set,
            fallback_armed,
            effort_selectable,
            mode: chrome.mode().map(str::to_string),
            effort: chrome.effort().map(str::to_string),
            route: route.to_string(),
            has_route_choices: route_inspectable,
            moa_scope,
            moa_formation,
            stacked,
            include_think,
            include_moa,
            model_text,
            think_text,
            moa_text,
        });
    }
    let (stacked, include_think, include_moa, model_text, think_text, moa_text) = {
        let chips = app
            .agent_control_chips
            .as_ref()
            .expect("agent control chips filled");
        (
            chips.stacked,
            chips.include_think,
            chips.include_moa,
            chips.model_text.clone(),
            chips.think_text.clone(),
            chips.moa_text.clone(),
        )
    };

    let enabled_style = Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD);
    let receipt_style = Style::new()
        .fg(ratatui::style::Color::Black)
        .bg(HUD_PHOSPHOR)
        .add_modifier(Modifier::BOLD);
    let disabled_style = Style::new().fg(HUD_DIM);
    let mut first = Vec::new();
    let mut second = Vec::new();
    let mut x = area.x;
    let mut y = area.y;
    let model_width = control_width_u16(&model_text).min(area.width);

    first.push(Span::styled(
        model_text,
        if route_just_set && !busy {
            receipt_style
        } else if model_enabled {
            enabled_style
        } else {
            disabled_style
        },
    ));
    if model_enabled {
        app.agent_buttons.push((
            Rect {
                x,
                y,
                width: model_width,
                height: 1,
            },
            AgentButton::Model,
        ));
    }
    x = x.saturating_add(model_width);

    if include_think {
        let target = if stacked {
            x = area.x;
            y = area.y.saturating_add(1);
            &mut second
        } else {
            first.push(Span::raw(" "));
            x = x.saturating_add(1);
            &mut first
        };
        let think_width =
            control_width_u16(&think_text).min(area.x.saturating_add(area.width).saturating_sub(x));
        target.push(Span::styled(
            think_text,
            if think_enabled {
                enabled_style
            } else {
                disabled_style
            },
        ));
        if think_enabled {
            app.agent_buttons.push((
                Rect {
                    x,
                    y,
                    width: think_width,
                    height: 1,
                },
                AgentButton::ReasoningEffort,
            ));
        }
        x = x.saturating_add(think_width);
    }

    if include_moa && x < area.x + area.width {
        first.push(Span::raw(" "));
        x = x.saturating_add(1);
        let moa_style = if app.moa_deck.is_some() {
            Style::new()
                .fg(ratatui::style::Color::Black)
                .bg(HUD_PHOSPHOR)
        } else {
            enabled_style
        };
        let moa_width =
            control_width_u16(&moa_text).min(area.x.saturating_add(area.width).saturating_sub(x));
        first.push(Span::styled(moa_text, moa_style));
        app.agent_buttons.push((
            Rect {
                x,
                y: area.y,
                width: moa_width,
                height: 1,
            },
            AgentButton::MoaDeck,
        ));
    }

    let mut lines = vec![Line::from(first)];
    if stacked {
        lines.push(Line::from(second));
    }
    frame.render_widget(Paragraph::new(lines).style(dim_panel_style()), area);
}

fn moa_chip_identity(app: &App) -> (u8, u64) {
    if let Some(engagement) = app.moa_one_shot.as_ref() {
        return (1, engagement.formation.cache_key());
    }
    if let Some(engagement) = app.moa_session.as_ref() {
        return (2, engagement.formation.cache_key());
    }
    if let Some(deck) = app.moa_deck.as_ref() {
        let formation = deck.selected();
        if !formation.is_resting() {
            return (3, formation.id.cache_key());
        }
    }
    (0, 0)
}

fn render_agent_feedback_controls(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(route) = pending_feedback_route(app) else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let useful = "[+ useful]";
    let miss = "[- miss]";
    let useful_style = Style::new().fg(HUD_PHOSPHOR).add_modifier(Modifier::BOLD);
    let enabled_style = Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD);
    let disabled_style = Style::new().fg(HUD_DIM);
    let mut feedback = vec![
        Span::styled(useful, useful_style),
        Span::raw(" "),
        Span::styled(miss, enabled_style),
    ];
    let useful_width = control_width_u16(useful);
    let miss_width = control_width_u16(miss);
    let used = usize::from(useful_width) + 1 + usize::from(miss_width);
    if area.width as usize > used + 3 {
        let max_route = area.width as usize - used - 3;
        feedback.push(Span::styled(
            format!(" · {}", truncate_control_value(&route, max_route)),
            disabled_style,
        ));
    }
    app.agent_buttons.push((
        Rect::new(area.x, area.y, useful_width.min(area.width), 1),
        AgentButton::RateUseful,
    ));
    app.agent_buttons.push((
        Rect::new(
            area.x.saturating_add(useful_width).saturating_add(1),
            area.y,
            miss_width.min(area.width.saturating_sub(useful_width.saturating_add(1))),
            1,
        ),
        AgentButton::RateMiss,
    ));
    frame.render_widget(
        Paragraph::new(Line::from(feedback)).style(dim_panel_style()),
        area,
    );
}

fn render_agentviz_portal_card(frame: &mut Frame, app: &mut App, area: Rect) -> Rect {
    if !side_column_kitty_compose_allowed() {
        return area;
    }
    if area.width < 30 || area.height < 9 {
        return area;
    }
    let Some(presentation) = app.agentviz_portal.presentation() else {
        return area;
    };
    let portal_height = area.height.saturating_sub(3).min(8);
    if portal_height < 5 {
        return area;
    }
    let [portal_area, remainder] =
        Layout::vertical([Constraint::Length(portal_height), Constraint::Min(3)]).areas(area);
    let seats = if presentation.omitted_seats > 0 {
        format!(
            "{}+{} seats",
            presentation.active_seats, presentation.omitted_seats
        )
    } else if presentation.active_seats == 1 {
        "1 seat".to_string()
    } else {
        format!("{} seats", presentation.active_seats)
    };
    // Seat-return pips from the live fan-out: "3/6 back" beside the count.
    let seats = if presentation.returned_seats > 0 {
        format!(
            "{seats} · {}/{} back",
            presentation.returned_seats,
            presentation.active_seats + presentation.omitted_seats
        )
    } else {
        seats
    };
    let title = truncate_control_value(
        &format!(" WebGPU · {} · {seats} ", presentation.stage),
        portal_area.width.saturating_sub(2) as usize,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .style(panel_style())
        .border_style(Style::new().fg(HUD_DIM))
        .title(title);
    let inner = block.inner(portal_area);
    frame.render_widget(block, portal_area);

    let painted = presentation.frame.as_ref().is_some_and(|portal_frame| {
        app.viewer
            .render_agentviz_portal(frame, inner, portal_frame)
    });
    if !painted {
        frame.render_widget(
            Paragraph::new("rendering GPU portal…")
                .alignment(Alignment::Center)
                .style(dim_panel_style()),
            inner,
        );
    }
    remainder
}

/// Test-frozen entry — see [`render_agent_controls`] for the wrapper rule.
#[cfg(test)]
pub(crate) fn render_agent_bay(frame: &mut Frame, app: &mut App, area: Rect) {
    let chrome = FrameChrome::compute(&app.bag);
    render_agent_bay_in(frame, app, area, &chrome);
}

/// Keep the stream rectangular and full-width. The compact portrait owns only
/// a bottom band, never a column alongside long text. Tiny bays spend every
/// available cell on reasoning rather than squeezing it around an avatar.
///
/// The trace strip at the top of the bay is guaranteed this many rows at every
/// size (operator request 2026-09-16: the trace must be accessible at all
/// times, even when the canvas is compressed).
const MIN_TRACE_ROWS: u16 = 2;

fn agent_bay_flow_layout(area: Rect, stable_h: u16, with_portrait: bool) -> (Rect, Option<Rect>) {
    // Operator request 2026-09-16: the agent trace must be accessible at all
    // bay sizes. Even the smallest bays keep a guaranteed trace strip at the
    // top and stack the portrait vertically beneath it — no size class drops
    // the trace or the portrait entirely.
    if !with_portrait || area.width < 20 || area.height < 4 {
        return (area, None);
    }
    // Operator request: keep the reactive fraction-based sizing, but make it
    // (a) consistent frame to frame. The old sizing
    // keyed off the ever-changing body area (controls/feedback rows appear
    // and vanish), so the knight shrank and grew as lines rose. Fractions are
    // now taken against the bay's stable inner height (`stable_h`) so the
    // portrait holds one size for a given terminal size, while still scaling
    // reactively on resize. Operator request: pull the avatar back 25% from the
    // oversized caps; keep it pinned to the lower-right corner.
    let (portrait_w, portrait_h) = if area.width >= 30 && stable_h >= 12 {
        (
            (area.width * 9 / 16).clamp(11, 30),
            (stable_h * 27 / 64).clamp(4, 10),
        )
    } else {
        (
            (area.width / 4).clamp(8, 12),
            (area.height * 3 / 16).clamp(2, 4),
        )
    };
    // The portrait may never swallow the trace: cap it so the top strip keeps
    // MIN_TRACE_ROWS no matter how short the bay becomes.
    let portrait_h = portrait_h.min(area.height - MIN_TRACE_ROWS);
    let flow = Rect {
        height: area.height - portrait_h,
        ..area
    };
    let portrait = Rect::new(
        area.right() - portrait_w,
        area.bottom() - portrait_h,
        portrait_w,
        portrait_h,
    );
    (flow, Some(portrait))
}

pub(crate) fn render_agent_bay_in(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    chrome: &FrameChrome,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let profile = app.active_profile();
    let active = portrait_active(app);
    let _badge = profile_badge(profile);
    let _header_owns_card = app.header_card_area.is_some();
    let title = if app.pending_approval.is_some() {
        format!(" {} · Approval needed ", profile.name)
    } else if app.bg_job.is_some() {
        format!(" {} · Working ", profile.name)
    } else if app.thinking.is_some() {
        format!(" {} · Thinking ", profile.name)
    } else if !app.reasoning.is_empty() {
        format!(" {} · Previous turn ", profile.name)
    } else {
        format!(" {} ", profile.name)
    };
    let focused = app
        .module_host
        .focused()
        .is_some_and(|module| module.as_str() == "agent");
    let mut block = transparent_hud_block(title.as_str());
    if focused {
        block = block.border_style(HUD_BLUE_BORDER_STYLE);
    }
    // Six-state border tint: the portrait state paints the bay frame. Blocked
    // outranks the focus tint (a missed approval modal must read from across
    // the room); the other states tint only an unfocused bay so the focus
    // indicator stays legible. Idle keeps the default chrome.
    if portrait_states_enabled() {
        let state = portrait_state(app);
        if let Some(tint) = state.border_tint()
            && (state == agent_view::PortraitState::Blocked || !focused)
        {
            block = block.border_style(Style::new().fg(tint));
        }
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let header_owns_route_controls = app.agent_control_area.is_some();
    let base_controls_h = if header_owns_route_controls {
        0
    } else if inner.height >= 6 && (20..38).contains(&inner.width) {
        2
    } else {
        u16::from(inner.height >= 6 && inner.width >= 20)
    };
    let feedback_h =
        u16::from(inner.height >= 6 && inner.width >= 20 && pending_feedback_route(app).is_some());
    let sections = Layout::vertical([
        Constraint::Length(base_controls_h),
        Constraint::Length(feedback_h),
        Constraint::Min(2),
    ])
    .split(inner);
    if base_controls_h > 0 {
        render_agent_controls_in(frame, app, sections[0], chrome);
    }
    if feedback_h > 0 {
        render_agent_feedback_controls(frame, app, sections[1]);
    }
    let body_area = render_agentviz_portal_card(frame, app, sections[2]);

    // Provider-exposed text persists after a turn and does not depend on focus.
    // Identity stays below it so long lines can use the entire bay width.
    let show_reasoning_body = !app.reasoning.is_empty()
        || app.bg_job.is_some()
        || app.thinking.is_some()
        || app.header_card_area.is_some()
        || app.agent_info_area.is_some()
        || body_area.height < 6;
    let header_owns_portrait = app.header_portrait_area.is_some();
    if show_reasoning_body {
        let (flow_area, portrait_area) =
            agent_bay_flow_layout(body_area, inner.height, !header_owns_portrait);
        render_reasoning_body(frame, app, flow_area);
        if let Some(portrait_area) = portrait_area {
            app.bay_portrait_area = Some(portrait_area);
            let ready = render_terminal_agent_portrait(frame, app, portrait_area, profile, chrome);
            // Without image support the compact identity still occupies the
            // same bottom-right slot, never the reasoning hit rectangle.
            if !ready {
                frame.render_widget(
                    Paragraph::new(bay_profile_lines(
                        app,
                        profile,
                        &agent_profile_caption_label(app),
                        active,
                    ))
                    .style(dim_panel_style()),
                    portrait_area,
                );
            }
        }
        return;
    }

    let token_capacity = if body_area.height >= 13 {
        5
    } else if body_area.height >= 10 {
        3
    } else if body_area.height >= 9 {
        1
    } else {
        0
    };
    let token_lines = agent_token_lines(app, body_area.width, token_capacity);
    let (context_pressure, capability_line) = if body_area.height >= 8 {
        // The bay inherits the retired info header's capability line: ctx
        // first, no default output-budget noise, pre-truncated to fit.
        agent_route_capability_paint(app, body_area.width as usize)
    } else {
        (ContextPressure::Normal, None)
    };
    // One bounded pulse serves both the row budget and the paint below — the
    // metrics strip shares `body_area`'s width, so building the full pulse a
    // second time just to test presence was pure double work.
    let route_receipt = fresh_brain_route_receipt(app).map(|label| format!("route set · {label}"));
    let host_metrics = agent_host_metrics_line(app, active);
    let metrics_h = if body_area.height >= 6 {
        u16::from(host_metrics.is_some())
            + u16::from(route_receipt.is_some())
            + u16::from(capability_line.is_some())
            + token_lines.len() as u16
    } else {
        0
    };
    let (upper_area, portrait_area) =
        agent_bay_flow_layout(body_area, inner.height, !header_owns_portrait);
    let [notice_area, metrics_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(metrics_h)]).areas(upper_area);
    render_reasoning_body(frame, app, notice_area);
    let label = agent_profile_caption_label(app);
    if let Some(portrait_area) = portrait_area {
        app.bay_portrait_area = Some(portrait_area);
        let ready = render_terminal_agent_portrait(frame, app, portrait_area, profile, chrome);
        if !ready {
            frame.render_widget(
                Paragraph::new(bay_profile_lines(app, profile, &label, active))
                    .style(dim_panel_style()),
                portrait_area,
            );
        }
    }

    if metrics_h > 0 {
        let mut lines = Vec::new();
        if let Some(metrics) = host_metrics {
            lines.push(Line::from(Span::raw(metrics)));
        }
        if let Some(receipt) = route_receipt {
            lines.push(Line::from(Span::styled(
                truncate_control_value(&receipt, metrics_area.width as usize),
                Style::new().fg(HUD_PHOSPHOR).add_modifier(Modifier::BOLD),
            )));
        }
        if let Some(capability) = capability_line {
            let capability_style = match context_pressure {
                ContextPressure::Normal => Style::new().fg(HUD_BLUE),
                ContextPressure::Warning(_) => Style::new()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                ContextPressure::Critical(_) => Style::new()
                    .fg(ratatui::style::Color::Red)
                    .add_modifier(Modifier::BOLD),
            };
            lines.push(Line::from(Span::styled(
                truncate_control_value(&capability, metrics_area.width as usize),
                capability_style,
            )));
        }
        lines.extend(token_lines);
        frame.render_widget(Paragraph::new(lines).style(dim_panel_style()), metrics_area);
    }
}

/// Right-aligned bay tab/clock title. Comp / lean and backdrop-off skip
/// `bag.tabs()` and the string build; default still paints the route.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn agent_route_title_allowed() -> bool {
    crate::App::side_column_visuals_allowed()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn agent_route_title(
    app: &mut App,
    width: u16,
    left_title_chars: usize,
) -> Option<String> {
    if !agent_route_title_allowed() {
        return None;
    }
    let max_chars = (width as usize).saturating_sub(left_title_chars.saturating_add(4));
    let tabs = app.bag.tabs();
    let thinking = app.think_state().is_some();
    let treebeard = crate::harness::is_treebeard();
    if !thinking {
        let key = (std::sync::Arc::as_ptr(&tabs) as usize, max_chars, treebeard);
        if let Some((ptr, width, lane, title)) = &app.idle_route_title
            && (*ptr, *width, *lane) == key
        {
            return Some(title.clone());
        }
    }
    let (clock, load_pct) = match app.think_state() {
        Some((_progress, secs, _club)) => (
            Some(status_view::fmt_clock(secs)),
            Some(app.overwatch.snapshot.load_pct()),
        ),
        None => (None, None),
    };
    let mut title =
        status_view::agent_route_title(tabs.as_slice(), clock.as_deref(), load_pct, max_chars)?;
    // Phase 3 Treebeard strip: surface the active RLM lane on the agent bay
    // so operators see strategy-only mode without opening ENV docs.
    if treebeard {
        let bare = title.trim();
        let tagged = format!(" treebeard · {bare} ");
        if tagged.chars().count() <= max_chars.saturating_add(2) {
            title = tagged;
        } else if max_chars >= 12 {
            title = " treebeard ".into();
        }
    }
    if !thinking {
        app.idle_route_title = Some((
            std::sync::Arc::as_ptr(&tabs) as usize,
            max_chars,
            treebeard,
            title.clone(),
        ));
    }
    Some(title)
}

pub(crate) fn render_reasoning_body(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let (cancel_area, body) = if app.bg_job.is_some() && area.height >= 2 {
        let [cancel, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        (Some(cancel), body)
    } else {
        (None, area)
    };
    // The bar must never overwrite a final text cell or the fresh tail.
    let scroll_area = body;
    let body = Rect {
        width: body.width.saturating_sub(1).max(1),
        ..body
    };
    // This is the exact text viewport after the scrollbar gutter. Portrait
    // cells stay outside this rect so they cannot enter the copy selection.
    app.panes.push(mouse::PaneId::AgentBay, body);
    if let (Some(cancel_area), Some(job)) = (cancel_area, app.bg_job.as_ref()) {
        frame.render_widget(
            Paragraph::new(format!("{} · Esc cancels", job.operation()))
                .style(Style::new().fg(HUD_DIM)),
            cancel_area,
        );
    }
    let background_output = app
        .bg_job
        .as_ref()
        .and_then(app_control::BackgroundJob::live_output);
    let shown = app.reasoning_shown.min(app.reasoning.len());
    let visible = if app.bg_job.is_some() {
        background_output
            .as_deref()
            .unwrap_or("Waiting for process output…")
    } else if app.reasoning.is_empty() {
        ""
    } else {
        &app.reasoning[..shown]
    };
    // While streaming, the fresh tail aligns with the portrait's right edge
    // below the text viewport. Settled text wraps full-width above it; overflow
    // follows the bottom unless the reader has scrolled to an earlier anchor.
    let streaming = app.bg_job.is_none()
        && !app.reasoning.is_empty()
        && (app.thinking.is_some() || shown < app.reasoning.len());
    let (settled, fresh) = if streaming {
        maybe_speech_flow_split(visible, usize::from(body.width).min(24))
    } else {
        (visible, "")
    };
    let reasoning_style = if app.bg_job.is_none() && app.reasoning.is_empty() {
        Style::new().fg(HUD_DIM)
    } else {
        PHOSPHOR_BOLD_STYLE
    };
    // Wrap key: body width, streaming vs settled, `trim: false`, shown-range
    // into `app.reasoning`, thinking/bg_job presence, and exact settled content.
    const WRAP_TRIM: bool = false;
    let wrap_width = body.width.max(1);
    let viewport_height = usize::from(body.height);
    let previous_total = app
        .reasoning_wrap
        .lines
        .saturating_add(usize::from(app.reasoning_wrap.fresh_row));
    let previous_bottom = previous_total.saturating_sub(app.reasoning_wrap.viewport_height);
    let previous_y = previous_bottom.saturating_sub(app.reasoning_scroll);
    let anchor = (app.reasoning_scroll > 0)
        .then(|| app.reasoning_wrap.rows.get(previous_y).map(|row| row.start))
        .flatten();
    let mut replacement_rows = None;
    let settled_lines = app.reasoning_wrap.settled_lines(
        crate::app::ReasoningWrapKey {
            width: wrap_width,
            streaming,
            wrap_trim: WRAP_TRIM,
            shown,
            reasoning_len: app.reasoning.len(),
            live_thinking: app.thinking.is_some(),
            bg_job: app.bg_job.is_some(),
        },
        settled,
        || {
            let rows = reasoning_row_ranges(settled, wrap_width);
            let count = rows.len();
            replacement_rows = Some(rows);
            count
        },
    );
    if let Some(rows) = replacement_rows {
        app.reasoning_wrap.rows = rows;
    }
    let total = settled_lines.saturating_add(usize::from(!fresh.is_empty()));
    let bottom = total.saturating_sub(viewport_height);
    if let Some(anchor) = anchor {
        let row = app
            .reasoning_wrap
            .rows
            .partition_point(|row| row.start <= anchor)
            .saturating_sub(1);
        app.reasoning_scroll = bottom.saturating_sub(row);
    }
    app.reasoning_scroll = app.reasoning_scroll.min(bottom);
    let y = bottom - app.reasoning_scroll;
    app.reasoning_wrap.viewport_height = viewport_height;
    app.reasoning_wrap.fresh_row = !fresh.is_empty();
    let visible_rows = reasoning_visible_rows(
        settled,
        &app.reasoning_wrap.rows,
        y,
        viewport_height,
        wrap_width,
    );
    frame.render_widget(Paragraph::new(visible_rows).style(reasoning_style), body);
    if !fresh.is_empty() {
        let row = settled_lines.saturating_sub(y);
        if row < viewport_height {
            let fresh_area = Rect {
                x: body.x,
                y: body.y.saturating_add(row as u16),
                width: body.width,
                height: 1,
            };
            // The fresh tail ends above the bottom-right portrait.
            let align = ratatui::layout::Alignment::Right;
            frame.render_widget(
                Paragraph::new(fresh)
                    .alignment(align)
                    .style(Style::new().fg(HUD_PHOSPHOR).add_modifier(Modifier::ITALIC)),
                fresh_area,
            );
        }
    }
    if total > viewport_height && scroll_area.width > 1 {
        let mut sb = ScrollbarState::new(total)
            .viewport_content_length(body.height as usize)
            .position(y);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            scroll_area,
            &mut sb,
        );
    }
}

/// Decorative right-align of the last words. Comp / lean paints settled
/// think text; default still pours from the portrait edge.
pub(crate) fn speech_flow_allowed() -> bool {
    crate::comp_mode::ambient_stage_sim_allowed()
}

/// Comp / lean skip the split; default still hangs the tail at the portrait.
pub(crate) fn maybe_speech_flow_split(text: &str, max_fresh: usize) -> (&str, &str) {
    if speech_flow_allowed() {
        let (settled, fresh) = speech_flow_split(text, max_fresh);
        if reasoning_flow_suffix_fits(settled, fresh, max_fresh) {
            (settled, fresh)
        } else {
            (text, "")
        }
    } else {
        (text, "")
    }
}

/// Split revealed reasoning into (settled, fresh) for the speech-flow render:
/// `fresh` is the trailing whole-word run (at most `max_fresh` chars) still
/// "in flight" at the portrait edge; `settled` is everything already merged
/// into the wrapped paragraph. The very start of a stream is entirely fresh —
/// the first words appear at the portrait before any paragraph exists. A
/// whitespace-free window (one long token) splits mid-token rather than
/// holding the whole tail hostage.
pub(crate) fn speech_flow_split(text: &str, max_fresh: usize) -> (&str, &str) {
    if text.is_empty() {
        return ("", "");
    }
    let mut window_start = text.len();
    for (count, (idx, _)) in text.char_indices().rev().enumerate() {
        window_start = idx;
        if count + 1 >= max_fresh {
            break;
        }
    }
    if window_start == 0 {
        return ("", text.trim_end());
    }
    let window = &text[window_start..];
    // Fresh renders in a ONE-row right-aligned strip, so it must never carry a
    // newline: prefer the tail after the window's last explicit line break.
    let fresh_start = if let Some(nl) = window.rfind('\n') {
        window[nl..]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map(|(i, _)| window_start + nl + i)
            .unwrap_or(text.len())
    } else {
        match window.find(char::is_whitespace) {
            Some(ws) => window[ws..]
                .char_indices()
                .find(|(_, c)| !c.is_whitespace())
                .map(|(i, _)| window_start + ws + i)
                .unwrap_or(text.len()),
            None => window_start,
        }
    };
    (&text[..fresh_start], text[fresh_start..].trim_end())
}

#[allow(dead_code)]
pub(crate) fn render_reasoning_canvas(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let title = status_view::reasoning_title(app.active_profile().name);
    let title = Span::styled(title.as_ref(), PHOSPHOR_BOLD_STYLE);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(PHOSPHOR_STYLE)
        .title(title);
    let inner = block.inner(area);
    render_reasoning_body(frame, app, inner);
}

/// Pure strip label for Treebeard lane (testable). Living peer geomean is the
fn treebeard_header_strip_label() -> String {
    let hiq = crate::harness::last_root_hiq();
    let stats = crate::harness::session_stats();
    let peer = crate::harness::load_living_peer_snapshot();
    let p1 = crate::harness::load_living_peer_p1_us();
    let top_open = crate::harness::load_living_peer_top_open();
    let forge = crate::harness::load_forge_train_snap();
    let forge_frag = forge
        .as_ref()
        .map(crate::harness::forge_train_strip_fragment)
        .unwrap_or_default();
    type Key = (
        Option<crate::harness::LastRootHiq>,
        crate::harness::HandleStoreStats,
        Option<u64>,
        Option<u64>,
        Option<String>,
        Option<u64>,
        String,
    );
    let peer_bits = peer.as_ref().map(|(geo, _, _)| geo.to_bits());
    let p1_bits = p1.map(f64::to_bits);
    let top_open_us = top_open.as_ref().map(|(_, us)| us.to_bits());
    static CACHE: std::sync::Mutex<Option<(Key, String)>> = std::sync::Mutex::new(None);
    let stored_open_key = {
        let top_open_key = top_open.as_ref().map(|(key, _)| key.as_str());
        if let Ok(guard) = CACHE.lock()
            && let Some((cached, label)) = guard.as_ref()
            && cached.0 == hiq
            && cached.1 == stats
            && cached.2 == peer_bits
            && cached.3 == p1_bits
            && cached.4.as_deref() == top_open_key
            && cached.5 == top_open_us
            && cached.6 == forge_frag
        {
            return label.clone();
        }
        top_open_key.map(str::to_owned)
    };
    let label = treebeard_strip_label_with_open(hiq, stats, peer, p1, top_open, forge);
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((
            (
                hiq,
                stats,
                peer_bits,
                p1_bits,
                stored_open_key,
                top_open_us,
                forge_frag,
            ),
            label.clone(),
        ));
    }
    label
}

/// GPU MODE competition frontier from `popcorn-promote-peer`. Optional P1
/// shape µs keeps the open lever visible without stuffing bulk logs.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn treebeard_strip_label(
    hiq: Option<crate::harness::LastRootHiq>,
    stats: crate::harness::HandleStoreStats,
    peer: Option<(f64, String, Option<String>)>,
) -> String {
    treebeard_strip_label_with_p1(hiq, stats, peer, None)
}

/// Same as [`treebeard_strip_label`] with optional living P1 shape µs.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn treebeard_strip_label_with_p1(
    hiq: Option<crate::harness::LastRootHiq>,
    stats: crate::harness::HandleStoreStats,
    peer: Option<(f64, String, Option<String>)>,
    p1_us: Option<f64>,
) -> String {
    treebeard_strip_label_with_forge(hiq, stats, peer, p1_us, None)
}

/// Treebeard strip + optional free-train / last-cycle forge fragment.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn treebeard_strip_label_with_forge(
    hiq: Option<crate::harness::LastRootHiq>,
    stats: crate::harness::HandleStoreStats,
    peer: Option<(f64, String, Option<String>)>,
    p1_us: Option<f64>,
    forge: Option<crate::harness::ForgeTrainSnap>,
) -> String {
    treebeard_strip_label_with_open(hiq, stats, peer, p1_us, None, forge)
}

/// P1 board keys arrive as `512x640` or `512·640`. Match in place so the
/// strip builder does not allocate a lowered copy of the lever name.
fn treebeard_open_key_is_p1(key: &str) -> bool {
    let Some(rest) = key.strip_prefix("512") else {
        return false;
    };
    let rest = rest
        .strip_prefix('x')
        .or_else(|| rest.strip_prefix('X'))
        .or_else(|| rest.strip_prefix('·'));
    rest.is_some_and(|rest| rest.starts_with("640"))
}

fn treebeard_open_key_short(key: &str) -> &str {
    if key.starts_with("32768") {
        "32k"
    } else if key.starts_with("16384") {
        "16k"
    } else if key.starts_with("8192") {
        "8k"
    } else {
        key
    }
}

/// Full Treebeard strip: peer + P1 + top open lever (board µs) + forge.
///
/// Top open is suppressed when it is the P1 shape (already shown as P1).
pub(crate) fn treebeard_strip_label_with_open(
    hiq: Option<crate::harness::LastRootHiq>,
    stats: crate::harness::HandleStoreStats,
    peer: Option<(f64, String, Option<String>)>,
    p1_us: Option<f64>,
    top_open: Option<(String, f64)>,
    forge: Option<crate::harness::ForgeTrainSnap>,
) -> String {
    let mut base = if let Some(hiq) = hiq {
        format!(
            "treebeard · offload {:.0}% · hiq {:.2}",
            hiq.offload_ratio * 100.0,
            hiq.hiq_priority
        )
    } else if stats.entries > 0 || stats.puts > 0 {
        let kb = stats.total_bytes / 1024;
        format!(
            "treebeard · Hi/Q · hnd {} / {}k",
            stats.entries,
            kb.max(if stats.total_bytes > 0 { 1 } else { 0 })
        )
    } else {
        "treebeard · Hi/Q".into()
    };
    if let Some((geo, _name, _)) = peer
        && geo.is_finite()
        && geo > 0.0
    {
        base.push_str(&format!(" · peer {:.1}µs", geo));
    }
    if let Some(p1) = p1_us
        && p1.is_finite()
        && p1 > 0.0
    {
        base.push_str(&format!(" · P1 {:.0}µs", p1));
    }
    if let Some((ref key, us)) = top_open
        && us.is_finite()
        && us > 0.0
    {
        // P1 already shown — prefer the next geomean lever (usually huge).
        if !treebeard_open_key_is_p1(key) {
            // Compact: 32768x1 → 32k, 16384x1 → 16k, else key as-is.
            let short = treebeard_open_key_short(key);
            // Rank-1 board µs = PRIMARY attack (usually huge 32k). Label it
            // so strip matches forge curriculum / popcorn-attack-primary.
            let tag = if key.starts_with("32768") || short == "32k" {
                "PRIMARY"
            } else {
                "open"
            };
            // µs for mid shapes; ms for huge singles so strip stays short.
            if us >= 10_000.0 {
                base.push_str(&format!(" · {tag} {} {:.0}ms", short, us / 1000.0));
            } else {
                base.push_str(&format!(" · {tag} {} {:.0}µs", short, us));
            }
        }
    }
    if let Some(ref snap) = forge {
        let frag = crate::harness::forge_train_strip_fragment(snap);
        if !frag.is_empty() {
            base.push_str(" · ");
            base.push_str(&frag);
        }
    }
    base
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/draw__agent_panel_view__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/draw__agent_panel_view__reasoning_rows_tests.rs"]
mod reasoning_rows_tests;
