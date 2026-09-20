//! Dotmax loop-state visualization for the artifacts pane.
//!
//! The cockpit already owns the terminal and ratatui frame, so this module uses
//! dotmax as a pure braille framebuffer and returns `Text` for normal composition.

use crate::harness::{SubmissionSlotPhase, SubmissionSlotTelemetry};
use crate::loop_ctl::{EscalationTier, LoopState, LoopStatus, cycle_elapsed_secs};
use crate::harness::comp_packages::yukon::fleet::{YukonFleetState, YukonSubmissionPhase};
use dotmax::{
    BrailleGrid, Color as DotColor,
    primitives::{draw_circle_colored, draw_line_colored},
};
use ratatui::{
    style::{Color as TuiColor, Style},
    text::{Line, Span, Text},
};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_CELLS: u16 = 220;
const PHOSPHOR_DIM: DotColor = DotColor::rgb(14, 78, 35);
const PHOSPHOR: DotColor = DotColor::rgb(45, 205, 82);
const PHOSPHOR_HOT: DotColor = DotColor::rgb(104, 255, 139);
const WARNING_AMBER: DotColor = DotColor::rgb(255, 204, 63);
const ALERT_RED: DotColor = DotColor::rgb(255, 74, 66);
const TUI_PHOSPHOR_DIM: TuiColor = TuiColor::Rgb(28, 102, 49);
const TUI_PHOSPHOR: TuiColor = TuiColor::Rgb(45, 205, 82);
const TUI_PHOSPHOR_HOT: TuiColor = TuiColor::Rgb(104, 255, 139);
const TUI_WARNING_AMBER: TuiColor = TuiColor::Rgb(255, 204, 63);
const TUI_ALERT_RED: TuiColor = TuiColor::Rgb(255, 74, 66);
pub(crate) const HAMMERTIME_MIN_SIDE: u16 = 7;

pub(crate) fn visible(st: &LoopState) -> bool {
    matches!(
        st.status,
        LoopStatus::Baselining
            | LoopStatus::Running
            | LoopStatus::Verifying
            | LoopStatus::AwaitingApproval
            | LoopStatus::Paused
    )
}

/// True while the loop is active or paused with findings. Hammertime dances here.
pub(crate) fn hammertime_active(st: &LoopState) -> bool {
    matches!(
        st.status,
        LoopStatus::Baselining
            | LoopStatus::Running
            | LoopStatus::Verifying
            | LoopStatus::AwaitingApproval
            | LoopStatus::Paused
    ) || st.iteration > 0
}

/// Kitty/halfblock portrait asset for dancer #1 (facing camera), flip ~280ms.
pub(crate) fn hammertime_asset(time_secs: f32) -> &'static str {
    // Alternate the two dance frames so Kitty graphics visibly flip back and forth.
    if hammertime_flip_tick(time_secs).is_multiple_of(2) {
        "assets/loop/hammertime-a.png"
    } else {
        "assets/loop/hammertime-b.png"
    }
}

/// Opposite frame for portrait prefetch (keeps the flip warm in the protocol cache).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn hammertime_asset_other(time_secs: f32) -> &'static str {
    if hammertime_asset(time_secs).ends_with("hammertime-a.png") {
        "assets/loop/hammertime-b.png"
    } else {
        "assets/loop/hammertime-a.png"
    }
}

/// Dancer #2 — mirrored twin, phase-shifted so the pair doesn't flip in lockstep.
/// Uses the dedicated flip plate on even ticks and pose B on odd ticks.
pub(crate) fn hammertime_twin_asset(time_secs: f32) -> &'static str {
    // Half-period offset: when #1 is A, twin is mid-step, and vice versa.
    let t = time_secs.max(0.0) + 0.14;
    if hammertime_flip_tick(t).is_multiple_of(2) {
        "assets/loop/hammertime-a-flip.png"
    } else {
        "assets/loop/hammertime-b.png"
    }
}

/// Prefetch opposite for the twin dancer.
#[allow(dead_code)]
pub(crate) fn hammertime_twin_asset_other(time_secs: f32) -> &'static str {
    if hammertime_twin_asset(time_secs).ends_with("hammertime-a-flip.png") {
        "assets/loop/hammertime-b.png"
    } else {
        "assets/loop/hammertime-a-flip.png"
    }
}

fn hammertime_flip_tick(time_secs: f32) -> u64 {
    ((time_secs.max(0.0) * 1000.0) as u64 / 280) % 2
}

/// Side length (cells) for each dancer plate, scaled to the Stage area.
pub(crate) fn hammertime_plate_side(area_w: u16, area_h: u16) -> u16 {
    // Duo needs two plates + a gap; stay compact so the map stays readable.
    // The authored minimum is only a preference: a live loop can open the
    // Stage in a shorter default panel, and its paint rectangle must remain a
    // hard child of that panel.
    let available = area_w.min(area_h);
    (available / 4)
        .clamp(HAMMERTIME_MIN_SIDE, 14)
        .min(available)
}

/// Layout for the dual hammertime dancers along the Stage floor: left twin +
/// right lead, with a light sway/bob so they read as dancing *around* rather
/// than stuck in fixed corners. Positions are clamped inside `area`.
pub(crate) fn hammertime_duo_boxes(
    area: ratatui::layout::Rect,
    time_secs: f32,
) -> [ratatui::layout::Rect; 2] {
    use ratatui::layout::Rect;
    let side = hammertime_plate_side(area.width, area.height);
    let t = time_secs.max(0.0);
    // Sway + bob in cell units — small enough not to thrash the protocol cache
    // key every frame (cache keys on width/height, not x/y… wait, portrait keys
    // on area size + path. Position changes alone are fine).
    let sway_l = ((t * 1.7).sin() * 2.0).round() as i32;
    let bob_l = ((t * 2.3).cos() * 1.0).round() as i32;
    let sway_r = ((t * 1.7 + 1.1).sin() * 2.0).round() as i32;
    let bob_r = ((t * 2.3 + 0.9).cos() * 1.0).round() as i32;

    let clamp_box = |x: i32, y: i32| -> Rect {
        let max_x = area.x.saturating_add(area.width.saturating_sub(side)) as i32;
        let max_y = area.y.saturating_add(area.height.saturating_sub(side)) as i32;
        let x = x.clamp(area.x as i32, max_x.max(area.x as i32)) as u16;
        let y = y.clamp(area.y as i32, max_y.max(area.y as i32)) as u16;
        Rect::new(x, y, side, side)
    };

    // Left floor: mirrored twin. Right floor: video / lead.
    let left_x = area.x as i32 + 1 + sway_l;
    let right_x = area.x as i32 + area.width as i32 - side as i32 - 1 + sway_r;
    let floor_y = area.y as i32 + area.height as i32 - side as i32 - 1;
    [
        clamp_box(left_x, floor_y + bob_l),
        clamp_box(right_x, floor_y + bob_r),
    ]
}

/// Frame rate of the video mascot dance (matches `scripts/video-to-mascot-frames.py`).
const MASCOT_FPS: f32 = 8.0;
/// How long a just-passed frame stays queued behind the live prefetch horizon.
#[cfg_attr(not(test), allow(dead_code))]
const MASCOT_TRAIL: usize = 6;

/// One lazily-populated ping-pong cycle of the video dance: forward through the
/// clip, then backward — so the dancer rewinds and dances forward again with no
/// cut. Frames are RGBA plates keyed by `scripts/video-to-mascot-frames.py`.
///
/// Empty / missing strip → `None` and the Stage falls back to the cartoon lead.
fn mascot_frames() -> &'static [std::path::PathBuf] {
    use std::sync::OnceLock;
    static FRAMES: OnceLock<Vec<std::path::PathBuf>> = OnceLock::new();
    FRAMES.get_or_init(|| {
        let dir = crate::runtime_paths::cockpit_dir().join("assets/loop/mascot");
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|entry| entry.ok())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .filter(|name| name.starts_with("hammertime2-") && name.ends_with(".png"))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        // Guard: refuse a thin strip (old planet failure was 50+ dark frames;
        // a real dance needs more than a few plates).
        if names.len() < 8 {
            return Vec::new();
        }
        let n = names.len();
        let mut frames: Vec<std::path::PathBuf> =
            names.into_iter().map(|name| dir.join(name)).collect();
        // Ping-pong: 0..n-1 then n-2..1 so both ends appear once.
        for i in (1..n.saturating_sub(1)).rev() {
            frames.push(frames[i].clone());
        }
        frames
    })
}

/// Live video-dance frame for `time_secs`; `None` when no strip is installed.
pub(crate) fn mascot_frame(time_secs: f32) -> Option<&'static std::path::Path> {
    let frames = mascot_frames();
    if frames.is_empty() {
        return None;
    }
    let idx = ((time_secs.max(0.0) * MASCOT_FPS) as usize) % frames.len();
    Some(frames[idx].as_path())
}

/// Prefetch horizon around the live video frame (trail + short lookahead).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn mascot_warm_frames(time_secs: f32) -> Vec<&'static std::path::Path> {
    let frames = mascot_frames();
    let n = frames.len();
    if n == 0 {
        return Vec::new();
    }
    let idx = ((time_secs.max(0.0) * MASCOT_FPS) as usize) % n;
    (0..=MASCOT_TRAIL)
        .map(|off| frames[(idx + n - off) % n].as_path())
        .chain((1..=3usize).map(|ahead| frames[(idx + ahead) % n].as_path()))
        .collect()
}

pub(crate) fn title(st: &LoopState) -> String {
    let status = match st.status {
        LoopStatus::Idle => "idle",
        LoopStatus::Baselining => "baselining",
        LoopStatus::Running => "running",
        LoopStatus::Verifying => "verifying",
        LoopStatus::AwaitingApproval => "awaiting SOTA",
        LoopStatus::Paused => "paused",
        LoopStatus::Done => "done",
        LoopStatus::Stopped => "stopped",
        LoopStatus::Failed => "failed",
    };
    let tier = match st.tier {
        EscalationTier::Local => "local",
        EscalationTier::Swarm => "swarm",
        EscalationTier::Sota => "SOTA",
    };
    let iters = if st.max_iters > 0 {
        format!("{}/{}", st.iteration, st.max_iters)
    } else {
        format!("{}/inf", st.iteration)
    };
    match cycle_elapsed_secs(st) {
        Some(secs) => format!(" loop · {status} · iter {iters} · {tier} · cycle {secs}s "),
        None => format!(" loop · {status} · iter {iters} · {tier} "),
    }
}

pub(crate) fn render(
    st: &LoopState,
    slot: &SubmissionSlotTelemetry,
    fleet: &YukonFleetState,
    time: f32,
    width: u16,
    height: u16,
) -> Text<'static> {
    let w = width.min(MAX_CELLS) as usize;
    let h = height.min(MAX_CELLS) as usize;
    if w == 0 || h == 0 {
        return Text::default();
    }
    let competition = slot.phase != SubmissionSlotPhase::Dormant;
    let hud_rows = hud_row_count(w, h, competition);
    let scene_h = h.saturating_sub(hud_rows);
    let time = if time.is_finite() { time } else { 0.0 };
    let mut grid = match BrailleGrid::new(w, scene_h) {
        Ok(grid) => grid,
        Err(_) => return Text::default(),
    };
    grid.enable_color_support();

    let dot_w = grid.dot_width();
    let dot_h = grid.dot_height();
    draw_trench_run(&mut grid, dot_w, dot_h, time, st.iteration, slot);
    let mut text = grid_to_text(&grid);
    text.lines
        .extend(hud_lines(st, slot, fleet, time, w, hud_rows));
    text
}

fn hud_row_count(width: usize, height: usize, competition: bool) -> usize {
    if width < 16 || height < 3 {
        0
    } else if competition && width >= 38 && height >= 8 {
        // The wide HUD follows the flight-instrument concept: fire control,
        // fleet, then one aligned telemetry rail. Keeping it to three rows
        // gives the perspective trench more room than the legacy meter stack.
        3
    } else if !competition && width >= 38 && height >= 6 {
        2
    } else if competition && width >= 24 && height >= 13 {
        5
    } else if width >= 24 && height >= 11 {
        4
    } else if height >= 8 {
        3
    } else if height >= 6 {
        2
    } else {
        1
    }
}

fn hud_lines(
    st: &LoopState,
    slot: &SubmissionSlotTelemetry,
    fleet: &YukonFleetState,
    time: f32,
    width: usize,
    rows: usize,
) -> Vec<Line<'static>> {
    if rows == 0 {
        return Vec::new();
    }
    let mut lines = Vec::with_capacity(rows);
    let competition = slot.phase != SubmissionSlotPhase::Dormant;
    if competition {
        lines.push(submission_slot_line(slot, width));
        if rows >= 2 {
            lines.push(yukon_fleet_line(fleet, time, width));
        }
        if rows >= 3 {
            if width >= 38 {
                lines.push(telemetry_rail(st, width));
            } else {
                lines.push(research_progress_line(st, width));
            }
        }
        if rows >= 4 {
            lines.push(research_progress_line(st, width));
        }
    } else {
        lines.push(research_progress_line(st, width));
        if rows >= 2 {
            if width >= 38 {
                lines.push(telemetry_rail(st, width));
            } else {
                lines.push(budget_line(
                    "tok",
                    st.tokens_spent as u64,
                    st.token_budget as u64,
                    width,
                ));
            }
        }
        if rows >= 3 {
            lines.push(submission_slot_line(slot, width));
        }
    }
    lines
}

/// One scan-line of loop health, arranged like the three lower instruments in
/// the trench concept. Each value owns its semantic health color while labels
/// remain phosphor green, so amber/red still mean something instead of becoming
/// decoration.
fn telemetry_rail(st: &LoopState, width: usize) -> Line<'static> {
    let column_width = width / 3;
    let remainder = width % 3;
    let widths = [
        column_width + usize::from(remainder > 0),
        column_width + usize::from(remainder > 1),
        column_width,
    ];
    let elapsed = run_elapsed_secs(st);
    let time_value = if widths[2] >= 17 {
        format!(
            "{}/{}",
            duration_clock(elapsed),
            duration_cap(st.deadline_secs)
        )
    } else {
        format!("{}/{}", compact_count(elapsed), cap_label(st.deadline_secs))
    };

    let metrics = [
        (
            if widths[0] >= 12 { "TOK" } else { "T" },
            format!(
                "{}/{}",
                compact_count(st.tokens_spent as u64),
                cap_label(st.token_budget as u64)
            ),
            budget_style(remaining_fraction(
                st.tokens_spent as u64,
                st.token_budget as u64,
            )),
        ),
        (
            if widths[1] >= 12 { "ITER" } else { "I" },
            format!(
                "{}/{}",
                compact_count(st.iteration as u64),
                cap_label(st.max_iters as u64)
            ),
            budget_style(remaining_fraction(st.iteration as u64, st.max_iters as u64)),
        ),
        (
            if widths[2] >= 12 { "TIME" } else { "C" },
            time_value,
            budget_style(remaining_fraction(elapsed, st.deadline_secs)),
        ),
    ];

    let mut spans = Vec::with_capacity(6);
    for ((label, value, value_style), column_width) in metrics.into_iter().zip(widths) {
        let label = format!("{label} ");
        let label_width = label.chars().count().min(column_width);
        spans.push(Span::styled(
            label.chars().take(column_width).collect::<String>(),
            Style::new().fg(TUI_PHOSPHOR),
        ));
        spans.push(Span::styled(
            fit_pad_ascii(&value, column_width.saturating_sub(label_width)),
            value_style,
        ));
    }
    Line::from(spans)
}

fn yukon_fleet_line(fleet: &YukonFleetState, time: f32, width: usize) -> Line<'static> {
    let prefix = "YUKON ";
    let mut spans = vec![Span::styled(
        prefix.chars().take(width).collect::<String>(),
        Style::new().fg(TUI_PHOSPHOR),
    )];
    let mut used = prefix.chars().count().min(width);
    if used >= width {
        return Line::from(spans);
    }

    if fleet.entries().is_empty() {
        let (lamp, label, color) = if fleet.scanning() {
            let lamp = if pulse_on(time) { "[~] " } else { "[ ] " };
            (lamp, "SCANNING OPEN FLEET", TUI_WARNING_AMBER)
        } else if fleet.has_error() {
            ("[X] ", "OFFLINE", TUI_ALERT_RED)
        } else {
            ("[ ] ", "NO ACTIVE SUBMISSIONS", TUI_PHOSPHOR)
        };
        let lamp = lamp
            .chars()
            .take(width.saturating_sub(used))
            .collect::<String>();
        used += lamp.chars().count();
        spans.push(Span::styled(lamp, Style::new().fg(color)));
        spans.push(Span::styled(
            fit_pad_ascii(label, width.saturating_sub(used)),
            Style::new().fg(TUI_PHOSPHOR_HOT),
        ));
        return Line::from(spans);
    }

    if fleet.stale() || fleet.scanning() {
        let (lamp, color) = if fleet.stale() {
            ("! ", TUI_ALERT_RED)
        } else if pulse_on(time) {
            ("~ ", TUI_WARNING_AMBER)
        } else {
            ("  ", TUI_WARNING_AMBER)
        };
        spans.push(Span::styled(lamp, Style::new().fg(color)));
        used += lamp.chars().count();
    }

    let counts = fleet_counts(fleet);
    let summary = format!(
        " B{} Q{} V{} H{} X{}",
        fleet.benchmark_count(),
        counts.0,
        counts.1,
        counts.2,
        counts.3
    );
    let summary_width = summary.chars().count().min(width.saturating_sub(used));
    let dot_capacity = width.saturating_sub(used).saturating_sub(summary_width);
    let visible = if fleet.entries().len() > dot_capacity && dot_capacity > 0 {
        dot_capacity.saturating_sub(1)
    } else {
        fleet.entries().len().min(dot_capacity)
    };
    for entry in fleet.entries().iter().take(visible) {
        let (glyph, color) = fleet_glyph(entry.phase, time);
        spans.push(Span::styled(glyph, Style::new().fg(color)));
        used += 1;
    }
    if visible < fleet.entries().len() && used < width.saturating_sub(summary_width) {
        spans.push(Span::styled("…", Style::new().fg(TUI_PHOSPHOR)));
        used += 1;
    }
    let padding = width.saturating_sub(summary_width).saturating_sub(used);
    if padding > 0 {
        spans.push(Span::raw(" ".repeat(padding)));
    }
    spans.push(Span::styled(
        summary.chars().take(summary_width).collect::<String>(),
        Style::new().fg(TUI_PHOSPHOR_HOT),
    ));
    Line::from(spans)
}

fn fleet_counts(fleet: &YukonFleetState) -> (usize, usize, usize, usize) {
    fleet.entries().iter().fold(
        (0, 0, 0, 0),
        |(queued, running, accepted, rejected), entry| match entry.phase {
            YukonSubmissionPhase::Queued => (queued + 1, running, accepted, rejected),
            YukonSubmissionPhase::Running => (queued, running + 1, accepted, rejected),
            YukonSubmissionPhase::Accepted => (queued, running, accepted + 1, rejected),
            YukonSubmissionPhase::Rejected => (queued, running, accepted, rejected + 1),
            YukonSubmissionPhase::Unknown => (queued, running, accepted, rejected),
        },
    )
}

fn fleet_glyph(phase: YukonSubmissionPhase, time: f32) -> (&'static str, TuiColor) {
    match phase {
        YukonSubmissionPhase::Queued | YukonSubmissionPhase::Running => {
            (if pulse_on(time) { "●" } else { "·" }, TUI_WARNING_AMBER)
        }
        YukonSubmissionPhase::Accepted => ("●", TUI_PHOSPHOR_HOT),
        YukonSubmissionPhase::Rejected => ("×", TUI_ALERT_RED),
        YukonSubmissionPhase::Unknown => ("○", TUI_PHOSPHOR_DIM),
    }
}

fn pulse_on(time: f32) -> bool {
    ((time.max(0.0) * 2.0).floor() as u64).is_multiple_of(2)
}

fn submission_slot_line(slot: &SubmissionSlotTelemetry, width: usize) -> Line<'static> {
    let prefix = if width >= 30 {
        "TRENCH // SLOT "
    } else {
        "SLOT "
    };
    let prefix_len = prefix.chars().count().min(width);
    let status_width = width.saturating_sub(prefix_len);
    let (lamp, status, lamp_color) = submission_slot_label(slot);
    let lamp_len = lamp.chars().count().min(status_width);
    Line::from(vec![
        Span::styled(
            prefix.chars().take(width).collect::<String>(),
            Style::new().fg(TUI_PHOSPHOR),
        ),
        Span::styled(
            lamp.chars().take(status_width).collect::<String>(),
            Style::new().fg(lamp_color),
        ),
        Span::styled(
            fit_pad_ascii(&status, status_width.saturating_sub(lamp_len)),
            Style::new().fg(TUI_PHOSPHOR_HOT),
        ),
    ])
}

fn submission_slot_label(slot: &SubmissionSlotTelemetry) -> (&'static str, String, TuiColor) {
    match slot.phase {
        SubmissionSlotPhase::Dormant => ("[ ] ", "STANDBY".to_string(), TUI_PHOSPHOR_DIM),
        SubmissionSlotPhase::Empty => ("[ ] ", "NO SUBMISSION".to_string(), TUI_PHOSPHOR),
        SubmissionSlotPhase::InFlight => {
            let id = slot.id.as_deref().map(short_slot_id).unwrap_or("slot");
            ("[!] ", format!("PENDING · {id}"), TUI_WARNING_AMBER)
        }
        SubmissionSlotPhase::Accepted => {
            let score = slot
                .score
                .as_deref()
                .map(|score| format!(" · {score}"))
                .unwrap_or_default();
            ("[+] ", format!("ACCEPTED{score}"), TUI_PHOSPHOR_HOT)
        }
        SubmissionSlotPhase::Rejected => ("[X] ", "REJECTED".to_string(), TUI_ALERT_RED),
    }
}

fn short_slot_id(id: &str) -> &str {
    id.get(id.len().saturating_sub(8)..).unwrap_or(id)
}

fn research_progress_line(st: &LoopState, width: usize) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }
    let findings_n = st.findings.len();
    let dirs_n = st.directions_tried.len();
    let hyp_n = st.hypotheses.len();
    let task_summary = st.task.lines().next().unwrap_or("").trim();

    let frac = if st.max_iters > 0 {
        (st.iteration as f32 / st.max_iters as f32).clamp(0.0, 1.0)
    } else {
        (findings_n as f32 / (findings_n + st.stale_count + 1) as f32).clamp(0.0, 1.0)
    };

    let bar_w = if width >= 50 {
        12
    } else if width >= 34 {
        8
    } else {
        4
    };
    let filled = (frac * bar_w as f32).round() as usize;
    let bar = format!(
        "{}{}",
        "#".repeat(filled.min(bar_w)),
        "-".repeat(bar_w.saturating_sub(filled))
    );

    let prefix = "RES ";
    let stats = if width >= 56 {
        format!("[{bar}] {findings_n} find · {dirs_n} dir · {hyp_n} hyp")
    } else if width >= 36 {
        format!("[{bar}] F:{findings_n} D:{dirs_n} H:{hyp_n}")
    } else if width >= 20 {
        format!("[{bar}] F:{findings_n}")
    } else {
        format!("[{bar}]")
    };

    let prefix_chars: String = prefix.chars().take(width).collect();
    let prefix_len = prefix_chars.chars().count();
    let remaining_w = width.saturating_sub(prefix_len);

    let stats_chars: String = stats.chars().take(remaining_w).collect();
    let stats_len = stats_chars.chars().count();
    let remaining_w2 = remaining_w.saturating_sub(stats_len);

    let mut spans = vec![
        Span::styled(prefix_chars, Style::new().fg(TUI_PHOSPHOR)),
        Span::styled(stats_chars, Style::new().fg(TUI_PHOSPHOR_HOT)),
    ];

    if remaining_w2 >= 6 && !task_summary.is_empty() {
        spans.push(Span::styled(" | ", Style::new().fg(TUI_PHOSPHOR_DIM)));
        let task_room = remaining_w2 - 3;
        let task_fitted = fit_pad_ascii(task_summary, task_room);
        spans.push(Span::styled(
            task_fitted,
            Style::new().fg(TUI_WARNING_AMBER),
        ));
    } else if remaining_w2 > 0 {
        spans.push(Span::raw(" ".repeat(remaining_w2)));
    }

    Line::from(spans)
}

fn budget_line(label: &str, used: u64, cap: u64, width: usize) -> Line<'static> {
    let remaining = remaining_fraction(used, cap);
    let bar_w = if width >= 50 {
        18
    } else if width >= 34 {
        12
    } else {
        8
    };
    let text = format!(
        "{label:<4} [{}] {}/{}",
        health_bar(remaining, bar_w),
        compact_count(used),
        cap_label(cap)
    );
    Line::from(Span::styled(
        fit_pad_ascii(&text, width),
        budget_style(remaining),
    ))
}

fn health_bar(remaining: f32, width: usize) -> String {
    let width = width.max(1);
    let filled = (remaining.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!(
        "{}{}",
        "#".repeat(filled.min(width)),
        "-".repeat(width.saturating_sub(filled))
    )
}

fn budget_style(remaining: f32) -> Style {
    let color = if remaining <= 0.15 {
        TUI_ALERT_RED
    } else if remaining <= 0.35 {
        TUI_WARNING_AMBER
    } else {
        TUI_PHOSPHOR_HOT
    };
    Style::new().fg(color)
}

fn remaining_fraction(used: u64, cap: u64) -> f32 {
    if cap == 0 {
        1.0
    } else {
        1.0 - (used as f32 / cap.max(1) as f32).clamp(0.0, 1.0)
    }
}

fn cap_label(cap: u64) -> String {
    if cap == 0 {
        "inf".to_string()
    } else {
        compact_count(cap)
    }
}

fn duration_cap(seconds: u64) -> String {
    if seconds == 0 {
        "--:--".to_string()
    } else {
        duration_clock(seconds)
    }
}

fn duration_clock(seconds: u64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes < 100 {
        format!("{minutes:02}:{seconds:02}")
    } else {
        let hours = minutes / 60;
        let minutes = minutes % 60;
        format!("{hours}:{minutes:02}:{seconds:02}")
    }
}

fn compact_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0).replace(".0m", "m")
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0).replace(".0k", "k")
    } else {
        n.to_string()
    }
}

fn run_elapsed_secs(st: &LoopState) -> u64 {
    if st.started_ms == 0 {
        return 0;
    }
    now_ms().saturating_sub(st.started_ms) / 1000
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn fit_pad_ascii(text: &str, width: usize) -> String {
    let mut out = if text.chars().count() > width {
        if width <= 1 {
            "~".to_string()
        } else {
            let mut s = text.chars().take(width - 1).collect::<String>();
            s.push('~');
            s
        }
    } else {
        text.to_string()
    };
    let len = out.chars().count();
    if len < width {
        out.push_str(&" ".repeat(width - len));
    }
    out
}

/// Perspective wireframe valley. Every structural line is phosphor green;
/// yellow and red are reserved for submission state at the targeting reticle.
///
/// The geometry is deliberately a *trench*, not a generic starfield: a high
/// wall lip, sloping side grids, a recessed floor, and a stream of guidance
/// gates all collapse into the submission target. That target is the stable
/// visual anchor while the world moves toward the pilot.
fn draw_trench_run(
    grid: &mut BrailleGrid,
    dot_w: usize,
    dot_h: usize,
    time: f32,
    iteration: usize,
    slot: &SubmissionSlotTelemetry,
) {
    if dot_w < 4 || dot_h < 4 {
        return;
    }
    let max_x = dot_w.saturating_sub(1) as i32;
    let max_y = dot_h.saturating_sub(1) as i32;
    let horizon = ((dot_h as f32 * 0.27).round() as i32).clamp(1, max_y - 1);
    let sway = (time * 0.42 + iteration as f32 * 0.13).sin();
    let vanishing_x =
        (max_x / 2 + (sway * dot_w as f32 * 0.025).round() as i32).clamp(1, max_x - 1);
    let floor_left = ((dot_w as f32 * 0.27).round() as i32).clamp(1, max_x / 2);
    let floor_right = ((dot_w as f32 * 0.73).round() as i32).clamp(max_x / 2, max_x - 1);
    let lip_min = (horizon + 1).min(max_y);
    let lip_max = (max_y - 1).max(lip_min);
    let near_lip_y = (max_y - (dot_h as f32 * 0.42).round() as i32).clamp(lip_min, lip_max);
    let wall_mid_y = (near_lip_y + max_y) / 2;
    let reticle_clearance = (max_x.min(max_y) / 16).clamp(2, 5) + 5;

    // Longitudinal rails describe the wall lips, wall planes, floor seams, and
    // attack lane. All of them converge on the same targeting solution.
    let center = (floor_left + floor_right) / 2;
    let near_rails = [
        (0, near_lip_y, PHOSPHOR),
        (floor_left / 2, wall_mid_y, PHOSPHOR_DIM),
        (floor_left, max_y, PHOSPHOR),
        ((floor_left + center) / 2, max_y, PHOSPHOR_DIM),
        (center, max_y, PHOSPHOR),
        ((center + floor_right) / 2, max_y, PHOSPHOR_DIM),
        (floor_right, max_y, PHOSPHOR),
        ((floor_right + max_x) / 2, wall_mid_y, PHOSPHOR_DIM),
        (max_x, near_lip_y, PHOSPHOR),
    ];
    for (near_x, near_y, ink) in near_rails {
        let start_x = lerp_i(vanishing_x, near_x, 0.16);
        let start_y = lerp_i(horizon, near_y, 0.16);
        let _ = draw_line_colored(grid, start_x, start_y, near_x, near_y, ink, Some(1));
    }

    // Cross-sections accelerate toward the camera. Unlike a flat horizon grid,
    // each section climbs both walls before crossing the recessed floor.
    let flow = (time * 0.72 + iteration as f32 * 0.19).fract();
    for band in 0..14 {
        let depth = ((band as f32 + flow) / 14.0).fract();
        let perspective = depth.powf(2.15);
        if perspective < 0.012 {
            continue;
        }
        let floor_y = lerp_i(horizon, max_y, perspective);
        if floor_y <= horizon + reticle_clearance {
            continue;
        }
        let wall_top_y = lerp_i(horizon, near_lip_y, perspective);
        let outer_left = lerp_i(vanishing_x, 0, perspective);
        let inner_left = lerp_i(vanishing_x, floor_left, perspective);
        let inner_right = lerp_i(vanishing_x, floor_right, perspective);
        let outer_right = lerp_i(vanishing_x, max_x, perspective);
        let ink = if perspective > 0.58 {
            PHOSPHOR
        } else {
            PHOSPHOR_DIM
        };
        let _ = draw_line_colored(
            grid,
            outer_left,
            wall_top_y,
            inner_left,
            floor_y,
            ink,
            Some(1),
        );
        let _ = draw_line_colored(
            grid,
            inner_left,
            floor_y,
            inner_right,
            floor_y,
            ink,
            Some(1),
        );
        let _ = draw_line_colored(
            grid,
            inner_right,
            floor_y,
            outer_right,
            wall_top_y,
            ink,
            Some(1),
        );
    }

    draw_guidance_gates(
        grid,
        vanishing_x,
        horizon,
        floor_left,
        floor_right,
        max_y,
        time + iteration as f32 * 0.09,
        slot,
    );

    // A restrained canopy/nose silhouette makes the camera read as a pilot's
    // seat without reducing the live grid to decorative scenery.
    let cockpit_rise = (dot_h as i32 / 9).clamp(2, 6);
    let nose_half = (dot_w as i32 / 18).clamp(2, 7);
    let nose_top_y = (max_y - cockpit_rise).max(horizon + 1);
    let _ = draw_line_colored(
        grid,
        0,
        max_y,
        floor_left / 2,
        wall_mid_y,
        PHOSPHOR,
        Some(1),
    );
    let _ = draw_line_colored(
        grid,
        max_x,
        max_y,
        max_x - floor_left / 2,
        wall_mid_y,
        PHOSPHOR,
        Some(1),
    );
    let _ = draw_line_colored(
        grid,
        center - nose_half,
        max_y,
        center - nose_half / 2,
        nose_top_y,
        PHOSPHOR_DIM,
        Some(1),
    );
    let _ = draw_line_colored(
        grid,
        center - nose_half / 2,
        nose_top_y,
        center + nose_half / 2,
        nose_top_y,
        PHOSPHOR_DIM,
        Some(1),
    );
    let _ = draw_line_colored(
        grid,
        center + nose_half / 2,
        nose_top_y,
        center + nose_half,
        max_y,
        PHOSPHOR_DIM,
        Some(1),
    );

    draw_submission_target(grid, vanishing_x, horizon, max_x, max_y, time, slot);
}

#[allow(clippy::too_many_arguments)]
fn draw_guidance_gates(
    grid: &mut BrailleGrid,
    x: i32,
    horizon: i32,
    floor_left: i32,
    floor_right: i32,
    max_y: i32,
    time: f32,
    slot: &SubmissionSlotTelemetry,
) {
    let flow = (time * 0.58).fract();
    let near_half_width = ((floor_right - floor_left) as f32 * 0.18).max(2.0);
    let reticle_clearance = (max_y / 16).clamp(2, 5) + 5;
    for gate in 0..6 {
        let depth = ((gate as f32 + flow) / 6.0).fract();
        let perspective = depth.powf(1.85);
        if perspective < 0.055 {
            continue;
        }
        let y = lerp_i(horizon, max_y, perspective);
        if y <= horizon + reticle_clearance {
            continue;
        }
        let half_width = (near_half_width * perspective).round().max(2.0) as i32;
        let half_height =
            (((max_y - horizon) as f32 * 0.055 * perspective).round() as i32).clamp(1, 4);
        let cap = (half_width / 2).clamp(1, 3);
        let left = x - half_width;
        let right = x + half_width;
        let ink = if slot.phase == SubmissionSlotPhase::Accepted && perspective > 0.42 {
            PHOSPHOR_HOT
        } else if perspective > 0.62 {
            PHOSPHOR
        } else {
            PHOSPHOR_DIM
        };
        let _ = draw_line_colored(
            grid,
            left,
            y - half_height,
            left,
            y + half_height,
            ink,
            Some(1),
        );
        let _ = draw_line_colored(
            grid,
            left,
            y - half_height,
            left + cap,
            y - half_height,
            ink,
            Some(1),
        );
        let _ = draw_line_colored(
            grid,
            left,
            y + half_height,
            left + cap,
            y + half_height,
            ink,
            Some(1),
        );
        let _ = draw_line_colored(
            grid,
            right,
            y - half_height,
            right,
            y + half_height,
            ink,
            Some(1),
        );
        let _ = draw_line_colored(
            grid,
            right - cap,
            y - half_height,
            right,
            y - half_height,
            ink,
            Some(1),
        );
        let _ = draw_line_colored(
            grid,
            right - cap,
            y + half_height,
            right,
            y + half_height,
            ink,
            Some(1),
        );
    }
}

fn draw_submission_target(
    grid: &mut BrailleGrid,
    x: i32,
    y: i32,
    max_x: i32,
    max_y: i32,
    time: f32,
    slot: &SubmissionSlotTelemetry,
) {
    let ink = submission_target_color(slot);
    let radius = ((max_x.min(max_y) / 16).clamp(2, 5)) as u32;
    let outer_radius = radius + 3;
    let _ = draw_circle_colored(grid, x, y, outer_radius, PHOSPHOR_DIM, false);
    let _ = draw_circle_colored(grid, x, y, radius, ink, false);

    let bracket = outer_radius as i32 + 3;
    draw_lock_brackets(grid, x, y, bracket, ink);

    // A sweep belongs only to acquisition. Once a submission is away, the
    // projectile supplies the motion and amber takes over the target state.
    if matches!(slot.phase, SubmissionSlotPhase::Empty) {
        let angle = time * 1.9;
        let sweep = outer_radius as f32;
        let sweep_x = x + (angle.cos() * sweep).round() as i32;
        let sweep_y = y + (angle.sin() * sweep * 0.55).round() as i32;
        let _ = draw_line_colored(grid, x, y, sweep_x, sweep_y, PHOSPHOR_DIM, Some(1));
    }

    match slot.phase {
        SubmissionSlotPhase::InFlight => {
            let travel = (time * 1.8).fract().powf(1.35);
            let shot_x = lerp_i(max_x / 2, x, travel);
            let shot_y = lerp_i(max_y, y, travel);
            let tail = (travel - 0.16).max(0.0);
            let tail_x = lerp_i(max_x / 2, x, tail);
            let tail_y = lerp_i(max_y, y, tail);
            let _ = draw_line_colored(grid, tail_x, tail_y, shot_x, shot_y, WARNING_AMBER, Some(1));
            let _ = draw_circle_colored(grid, shot_x, shot_y, 1, WARNING_AMBER, true);
        }
        SubmissionSlotPhase::Accepted => {
            let burst = 3 + ((time * 2.0).fract() * 4.0).round() as i32;
            let _ = draw_circle_colored(grid, x, y, burst as u32, PHOSPHOR_HOT, false);
            for (dx, dy) in [(-burst, 0), (burst, 0), (0, -burst), (0, burst)] {
                let _ = draw_line_colored(grid, x, y, x + dx, y + dy, ink, Some(1));
            }
        }
        SubmissionSlotPhase::Rejected => {
            let reject = outer_radius as i32;
            let _ = draw_line_colored(
                grid,
                x - reject,
                y - reject / 2,
                x + reject,
                y + reject / 2,
                ALERT_RED,
                Some(1),
            );
            let _ = draw_line_colored(
                grid,
                x - reject,
                y + reject / 2,
                x + reject,
                y - reject / 2,
                ALERT_RED,
                Some(1),
            );
        }
        SubmissionSlotPhase::Dormant | SubmissionSlotPhase::Empty => {}
    }
}

fn draw_lock_brackets(grid: &mut BrailleGrid, x: i32, y: i32, extent: i32, ink: DotColor) {
    let tick = (extent / 3).clamp(1, 3);
    for side_x in [-1, 1] {
        for side_y in [-1, 1] {
            let corner_x = x + side_x * extent;
            let corner_y = y + side_y * extent;
            let _ = draw_line_colored(
                grid,
                corner_x,
                corner_y,
                corner_x - side_x * tick,
                corner_y,
                ink,
                Some(1),
            );
            let _ = draw_line_colored(
                grid,
                corner_x,
                corner_y,
                corner_x,
                corner_y - side_y * tick,
                ink,
                Some(1),
            );
        }
    }
}

fn submission_target_color(slot: &SubmissionSlotTelemetry) -> DotColor {
    match slot.phase {
        SubmissionSlotPhase::Dormant => PHOSPHOR_DIM,
        SubmissionSlotPhase::Empty => PHOSPHOR,
        SubmissionSlotPhase::InFlight => WARNING_AMBER,
        SubmissionSlotPhase::Accepted => PHOSPHOR_HOT,
        SubmissionSlotPhase::Rejected => ALERT_RED,
    }
}

fn lerp_i(a: i32, b: i32, t: f32) -> i32 {
    (a as f32 + (b - a) as f32 * t.clamp(0.0, 1.0)).round() as i32
}

fn grid_to_text(grid: &BrailleGrid) -> Text<'static> {
    Text::from(crate::term::art::grid_lines(grid))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/loop_viz__tests.rs"]
mod tests;
