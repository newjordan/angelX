//! The research plotline: the loop's discoveries as one rising Dotmax contour,
//! drawn like the startup intro. A white pen line runs from the low left and
//! the dotted wizard stands at the frontier. Each admitted finding or recorded
//! measurement lifts the line; a flat shelf is the loop still studying. Time
//! runs in machine-speed offsets from the run's start (`+12s`, `+4m07s`,
//! `+1h02m`), not wall-clock ages. On first sight the whole line is traced in
//! and the wizard walks on; a later discovery draws its rise in while the
//! wizard climbs it.

use crate::app::startup_intro::{
    INTRO_INK, draw_dot_line, draw_wizard_sized, paint_dot, smoothstep,
};
use crate::drive::loop_ctl::LoopState;
use crate::ui::term::art::{ColoredBrailleCell, ColoredBrailleImage};
use crate::ui::viz::lifecycle_viz::MotionMode;
use std::time::Instant;

/// The pen's left origin and the frontier (where the wizard stands), as
/// fractions of the canvas width; the base and summit heights as fractions of
/// its height. The same framing as the intro's ascent.
const X_ORIGIN: f32 = 0.06;
const X_FRONTIER: f32 = 0.84;
const X_GROUND_END: f32 = 0.92;
const Y_BASE: f32 = 0.89;
const Y_SUMMIT: f32 = 0.43;
/// A discovery's rise takes this share of the width before its point, so the
/// line climbs like the intro's slopes while the shelves stay flat.
const RISE_WIDTH: f32 = 0.018;
/// The line scale never tops out below this many discoveries, so one early
/// finding reads as a first step rather than the summit.
const MIN_SCALE: usize = 4;
/// Seconds to trace the whole line on first sight, and to draw a new rise.
const TRACE_SECS: f32 = 2.0;
const RISE_SECS: f32 = 1.2;
/// The wizard's walk onto the frontier once the first trace is done.
const WALK_SECS: f32 = 2.0;
/// The wizard's height as a share of the canvas: he fits the headroom above
/// the summit shelf.
const WIZARD_HEIGHT: f32 = 0.34;

/// One discovery on the run's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Discovery {
    pub(crate) offset_ms: u64,
    /// A measurement or submission rather than a finding: marked on the line.
    pub(crate) measured: bool,
}

/// The run's discoveries in time order: admitted findings by their stamp,
/// measured candidates and submissions by their record time. Findings
/// admitted before stamping existed have no time and are left out.
pub(crate) fn discoveries(st: &LoopState) -> Vec<Discovery> {
    let start = st.started_ms;
    let offset = |at_ms: u64| at_ms.saturating_sub(start);
    let mut out: Vec<Discovery> = st
        .finding_stamps
        .iter()
        .filter(|stamp| stamp.at_ms > 0)
        .map(|stamp| Discovery {
            offset_ms: offset(stamp.at_ms),
            measured: false,
        })
        .collect();
    let recorded = st
        .measured_candidates_log
        .iter()
        .map(|row| row.utc.as_str())
        .chain(st.submissions_log.iter().map(|row| row.utc.as_str()));
    for utc in recorded {
        if let Ok(secs) = utc.parse::<u64>() {
            out.push(Discovery {
                offset_ms: offset(secs.saturating_mul(1000)),
                measured: true,
            });
        }
    }
    out.sort_by_key(|discovery| discovery.offset_ms);
    out
}

/// `+12s`, `+4m07s`, `+1h02m`: an offset from the run's start.
pub(crate) fn offset_label(ms: u64) -> String {
    let secs = ms / 1000;
    match secs {
        0..=59 => format!("+{secs}s"),
        60..=3_599 => format!("+{}m{:02}s", secs / 60, secs % 60),
        _ => format!("+{}h{:02}m", secs / 3_600, (secs % 3_600) / 60),
    }
}

/// How much of the line is drawn in, and where the wizard is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reveal {
    /// Discoveries already drawn in full; later ones draw with `progress`.
    /// `None` traces the whole line from the origin (first sight).
    pub(crate) settled: Option<usize>,
    /// 0..=1 through the current trace or rise.
    pub(crate) progress: f32,
    /// `Some` while the wizard walks on from the right (0..=1).
    pub(crate) walk_in: Option<f32>,
    /// Idle pose (0..8): the wind in the cloak and hat.
    pub(crate) idle_pose: u8,
}

impl Reveal {
    pub(crate) fn still(total: usize) -> Self {
        Self {
            settled: Some(total),
            progress: 1.0,
            walk_in: None,
            idle_pose: 0,
        }
    }
}

/// Per-view motion: what the viewer has seen of which run.
#[derive(Debug, Default)]
pub(crate) struct PlotMotion {
    run: String,
    shown: usize,
    from: usize,
    began: Option<Instant>,
    first_sight: bool,
}

impl PlotMotion {
    /// Advance to `total` discoveries of run `run` and say what to draw.
    pub(crate) fn observe(
        &mut self,
        run: &str,
        total: usize,
        now: Instant,
        motion: MotionMode,
    ) -> Reveal {
        if motion != MotionMode::Full {
            *self = Self {
                run: run.to_string(),
                shown: total,
                from: total,
                began: None,
                first_sight: false,
            };
            return Reveal::still(total);
        }
        if self.run != run {
            // A run seen for the first time is traced in whole, as the intro is.
            *self = Self {
                run: run.to_string(),
                shown: total,
                from: 0,
                began: Some(now),
                first_sight: true,
            };
        } else if total != self.shown {
            self.from = self.shown.min(total);
            self.shown = total;
            self.began = Some(now);
            self.first_sight = false;
        }
        let Some(began) = self.began else {
            return Reveal {
                idle_pose: wind_pose(now),
                ..Reveal::still(total)
            };
        };
        let secs = now.saturating_duration_since(began).as_secs_f32();
        let (trace, walk) = if self.first_sight {
            (TRACE_SECS, WALK_SECS)
        } else {
            (RISE_SECS, 0.0)
        };
        let progress = smoothstep(secs / trace);
        let walk_in = (self.first_sight && secs < trace + walk)
            .then(|| smoothstep(((secs - trace) / walk).max(0.0)));
        if secs >= trace + walk {
            self.began = None;
            self.from = total;
            self.first_sight = false;
        }
        Reveal {
            settled: (!self.first_sight).then_some(self.from),
            progress,
            walk_in,
            idle_pose: wind_pose(now),
        }
    }
}

/// The idle wind cycle, four poses a second off the process clock.
fn wind_pose(now: Instant) -> u8 {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let epoch = *EPOCH.get_or_init(Instant::now);
    ((now.saturating_duration_since(epoch).as_millis() / 250) % 8) as u8
}

/// Compose the plotline on a `columns` × `rows` Braille canvas (each cell is
/// 2 × 4 dots). `elapsed_ms` is the frontier's offset from the run's start.
pub(crate) fn compose(
    discoveries: &[Discovery],
    elapsed_ms: u64,
    columns: usize,
    rows: usize,
    reveal: Reveal,
) -> ColoredBrailleImage {
    let mut image = ColoredBrailleImage {
        width: columns,
        height: rows,
        cells: vec![ColoredBrailleCell::default(); columns * rows],
    };
    if columns < 4 || rows < 2 {
        return image;
    }
    let dot_w = (columns * 2) as f32;
    let dot_h = (rows * 4) as f32;
    let point = |x: f32, y: f32| {
        (
            (x * (dot_w - 1.0)).round() as i32,
            (y * (dot_h - 1.0)).round() as i32,
        )
    };
    let span = elapsed_ms
        .max(discoveries.last().map_or(0, |d| d.offset_ms))
        .max(1) as f32;
    let scale = discoveries.len().max(MIN_SCALE) as f32;
    let x_at = |offset_ms: u64| X_ORIGIN + (offset_ms as f32 / span) * (X_FRONTIER - X_ORIGIN);
    let y_at = |count: usize| Y_BASE - (count as f32 / scale) * (Y_BASE - Y_SUMMIT);

    // The contour: flat shelves, and a short climb into each discovery.
    // `owner[i]` is how many discoveries are complete at vertex i.
    let mut vertices = vec![(X_ORIGIN, Y_BASE)];
    let mut owner = vec![0usize];
    let mut marks = Vec::new();
    for (index, discovery) in discoveries.iter().enumerate() {
        let x = x_at(discovery.offset_ms);
        let (last_x, _) = *vertices.last().unwrap();
        let foot = (x - RISE_WIDTH).max(last_x);
        vertices.push((foot, y_at(index)));
        owner.push(index);
        vertices.push((x, y_at(index + 1)));
        owner.push(index + 1);
        if discovery.measured {
            marks.push((index + 1, (x, y_at(index + 1))));
        }
    }
    let total = discoveries.len();
    vertices.push((X_GROUND_END, y_at(total)));
    owner.push(total);

    // Settled segments are drawn whole; the rest trace in with `progress`,
    // measured along their length as the intro's pen is.
    let segments: Vec<((f32, f32), (f32, f32), usize)> = vertices
        .windows(2)
        .zip(owner.windows(2))
        .map(|(pair, owners)| (pair[0], pair[1], owners[1]))
        .collect();
    let length = |a: (f32, f32), b: (f32, f32)| ((b.0 - a.0) * dot_w).hypot((b.1 - a.1) * dot_h);
    let is_settled = |owner: usize| reveal.settled.is_some_and(|settled| owner <= settled);
    let pending: f32 = segments
        .iter()
        .filter(|(_, _, owner)| !is_settled(*owner))
        .map(|(a, b, _)| length(*a, *b))
        .sum();
    let mut budget = pending * reveal.progress.clamp(0.0, 1.0);
    let mut frontier = (X_ORIGIN, Y_BASE);
    // Discoveries whose point the pen has reached: their marks show.
    let mut reached = 0;
    for (a, b, owner) in &segments {
        if is_settled(*owner) {
            draw_dot_line(&mut image, point(a.0, a.1), point(b.0, b.1), INTRO_INK, 255);
            frontier = *b;
            reached = reached.max(*owner);
            continue;
        }
        if budget <= 0.0 {
            break;
        }
        let whole = length(*a, *b);
        let portion = if whole > 0.0 {
            (budget / whole).min(1.0)
        } else {
            1.0
        };
        let end = (a.0 + (b.0 - a.0) * portion, a.1 + (b.1 - a.1) * portion);
        draw_dot_line(
            &mut image,
            point(a.0, a.1),
            point(end.0, end.1),
            INTRO_INK,
            255,
        );
        frontier = end;
        if portion >= 1.0 {
            reached = reached.max(*owner);
        }
        budget -= whole;
    }

    // A measurement or submission wears a small open ring where it landed.
    for (owner, (x, y)) in marks {
        if owner > reached {
            continue;
        }
        let (cx, cy) = point(x, y);
        for (dx, dy) in [
            (0, -2),
            (1, -1),
            (2, 0),
            (1, 1),
            (0, 2),
            (-1, 1),
            (-2, 0),
            (-1, -1),
        ] {
            paint_dot(&mut image, cx + dx, cy + dy - 3, INTRO_INK, 255);
        }
    }

    // The wizard stands at the frontier on the current shelf; while a rise
    // draws in he climbs with the pen, and on first sight he walks on from
    // the right once the trace is done.
    let traced = reveal.progress >= 1.0 || reveal.settled == Some(total);
    let ground_y = if traced { y_at(total) } else { frontier.1 };
    let wizard_x = match reveal.walk_in {
        Some(walk) if reveal.progress >= 1.0 => Some(1.24 + (X_FRONTIER - 1.24) * walk),
        Some(_) => None,
        None => Some(X_FRONTIER),
    };
    if let Some(x) = wizard_x {
        let (wx, wy) = point(x, ground_y);
        let walking = reveal.walk_in.is_some_and(|walk| walk < 1.0) || !traced;
        // The intro's gait: a new pose every third of its 24 walking ticks.
        let pose = match reveal.walk_in {
            Some(walk) if walk < 1.0 => ((walk * 24.0) as u8 / 3) % 8,
            _ if !traced => ((reveal.progress * 24.0) as u8 / 3) % 8,
            _ => reveal.idle_pose,
        };
        // Sized to the headroom above the summit, as tall as the chart allows.
        let height = (dot_h * WIZARD_HEIGHT).max(12.0);
        draw_wizard_sized(&mut image, wx, wy - 1, height, pose, !walking, 255);
    }
    image
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/research_plot__tests.rs"]
mod tests;
