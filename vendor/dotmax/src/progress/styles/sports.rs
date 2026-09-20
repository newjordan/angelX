//! Sports-themed progress bars — eleven distinct athletic spectacles in braille dots.
//!
//! Every bar is stateless: all motion comes from `ctx.time` (perpetual animation)
//! and `ctx.eased` / `ctx.progress` (progress-driven advancement). All writes go
//! through `draw::` helpers so every pixel is silently bounds-safe.
//!
//! Styles in this file:
//! - `sprint-100m`      — runner advances right, legs cycle, finish tape snaps at 100%
//! - `basketball-arc`   — ball traces a parabola into a hoop, swish flash at 100%
//! - `soccer-goal`      — ball curves into a net, net bulges on score
//! - `swimming-laps`    — swimmer bobs across a lane, lap counter via vblocks
//! - `archery`          — bow draws back with eased, arrow flies to a bullseye
//! - `bowling`          — ball rolls down lane, pins topple progressively at end
//! - `darts`            — dart flies toward concentric scoring rings of a dartboard
//! - `high-jump`        — athlete arcs over a bar that rises with progress
//! - `weightlifting`    — barbell overhead, plate stack height = eased
//! - `tennis-rally`     — ball bounces between baselines, rally count as hbar fills
//! - `cycling-peloton`  — wheels spin, riders packed tight, gap eaten by progress

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Theme tint — court orange into jersey red. Applied to styles that draw monochrome.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(255, 152, 56);
const TINT_END: Color = Color::rgb(255, 84, 84);

/// Sample the theme gradient at `t` in `0.0..=1.0`.
fn sample_tint(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
    Color::rgb(
        lerp(TINT_START.r, TINT_END.r),
        lerp(TINT_START.g, TINT_END.g),
        lerp(TINT_START.b, TINT_END.b),
    )
}

/// Applies the theme's signature gradient to every cell the inner style drew,
/// drifting slowly with `time`. Styles stay monochrome-safe underneath: drop
/// the wrapper in [`styles`] for uncolored output.
struct Tinted<S>(S);

impl<S: ProgressStyle> ProgressStyle for Tinted<S> {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn theme(&self) -> &str {
        self.0.theme()
    }
    fn describe(&self) -> &str {
        self.0.describe()
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        self.0.render(grid, ctx)?;
        grid.enable_color_support();
        let (w, h) = grid.dimensions();
        for y in 0..h {
            for x in 0..w {
                let ch = grid.get_char(x, y);
                if ch != '\u{2800}' && ch != ' ' {
                    let t = (x as f32 / w.max(1) as f32 + ctx.time * 0.05).fract();
                    let tri = 1.0 - (2.0 * t - 1.0).abs();
                    let _ = grid.set_cell_color(x, y, sample_tint(tri));
                }
            }
        }
        Ok(())
    }
}

/// All styles in the `sports` theme.
///
/// Returns one boxed [`ProgressStyle`] per sport variant, ready to be mixed
/// into a gallery or driven individually.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Sprint100m),
        Box::new(Tinted(BasketballArc)),
        Box::new(SoccerGoal),
        Box::new(Tinted(SwimmingLaps)),
        Box::new(Tinted(Archery)),
        Box::new(Bowling),
        Box::new(Tinted(Darts)),
        Box::new(Tinted(HighJump)),
        Box::new(Tinted(Weightlifting)),
        Box::new(TennisRally),
        Box::new(CyclingPeloton),
    ]
}

// ── Sprint 100m ───────────────────────────────────────────────────────────────

struct Sprint100m;
impl ProgressStyle for Sprint100m {
    fn name(&self) -> &str {
        "sprint-100m"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "100m sprinter advances right with cycling legs; finish-line tape snaps at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = h.saturating_sub(1);
        let mid = h / 2;

        // Ground line.
        draw::hline(grid, 0, w.saturating_sub(1), base);

        // Finish line — a vertical column of dots at the right edge.
        let finish_x = w.saturating_sub(2);
        draw::vline(grid, finish_x, 0, base);

        // Runner horizontal position driven by progress.
        let runner_x = (ctx.eased * finish_x.saturating_sub(4) as f32) as usize;
        let runner_x = runner_x.min(finish_x.saturating_sub(4));

        // Leg cycle phase from time.
        let phase = ctx.time * 8.0;
        // Forward leg (right).
        let fwd = (phase.sin() * 1.5).round() as i32;
        // Back leg (left) is opposite phase.
        let bk = (-(phase.sin()) * 1.5).round() as i32;

        // Body: torso leaning forward.
        let torso_bot = base as i32 - 1;
        let torso_top = torso_bot - (h as i32 / 3).max(1);
        draw::dot_i(grid, runner_x as i32 + 1, torso_bot);
        draw::dot_i(grid, runner_x as i32 + 1, torso_bot - 1);
        draw::dot_i(grid, runner_x as i32 + 1, torso_top);
        // Head.
        draw::dot_i(grid, runner_x as i32 + 2, torso_top - 1);

        // Arms (opposite to legs).
        let arm_phase = -phase;
        let arm_fwd = (arm_phase.sin() * 1.2).round() as i32;
        draw::dot_i(grid, runner_x as i32 + 2, torso_bot - 1 + arm_fwd);
        draw::dot_i(grid, runner_x as i32, torso_bot - 1 - arm_fwd);

        // Legs.
        draw::dot_i(grid, runner_x as i32 + 2, torso_bot + fwd);
        draw::dot_i(grid, runner_x as i32, torso_bot + bk);
        // Feet.
        draw::dot_i(grid, runner_x as i32 + 3, base as i32 + fwd.min(0));
        draw::dot_i(grid, runner_x as i32 - 1, base as i32 + bk.min(0));

        // Tape break flash when at 100%.
        if ctx.progress >= 0.999 {
            // Horizontal tape across mid of bar at the finish line.
            draw::hline(grid, finish_x.saturating_sub(1), w.saturating_sub(1), mid);
            // Torn fragments: a few scattered dots to the right.
            for k in 0..3usize {
                let fx = finish_x + k * 2;
                let fy = (mid as i32 + (k as i32 % 3) - 1).max(0) as usize;
                draw::dot(
                    grid,
                    fx.min(w.saturating_sub(1)),
                    fy.min(h.saturating_sub(1)),
                );
            }
        }

        // Tint the swept lane.
        let (cw, ch) = grid.dimensions();
        let swept = (ctx.eased * cw as f32) as usize;
        for cy in 0..ch {
            for cx in 0..swept.min(cw) {
                let t = if swept <= 1 {
                    0.0
                } else {
                    cx as f32 / (swept - 1) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Basketball Arc ────────────────────────────────────────────────────────────

struct BasketballArc;
impl ProgressStyle for BasketballArc {
    fn name(&self) -> &str {
        "basketball-arc"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Basketball traces a parabolic arc toward a hoop; swish flash lights up at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = h.saturating_sub(1);

        // Hoop: two dots at top-right, a small horizontal bar.
        let hoop_x = w.saturating_sub(3);
        let hoop_y = h / 4;
        draw::hline(grid, hoop_x, w.saturating_sub(1), hoop_y);
        // Net: vertical lines below hoop.
        let net_h = (h / 4).max(1);
        for nx in [hoop_x, w.saturating_sub(1)] {
            draw::vline(
                grid,
                nx.min(w.saturating_sub(1)),
                hoop_y,
                (hoop_y + net_h).min(base),
            );
        }
        // Net cross-strands (every 2 dots).
        for ny in (hoop_y..=(hoop_y + net_h).min(base)).step_by(2) {
            draw::hline(grid, hoop_x, w.saturating_sub(1), ny.min(base));
        }

        // Ball trajectory: parabola from bottom-left to hoop.
        // t drives the shot from 0 (ball in hand) → 1 (in hoop).
        // We animate t using progress; time adds a spin bobble at rest.
        let t = ctx.eased;

        // Parametric: x linear, y parabolic (peak at mid-arc).
        let bx = (t * hoop_x as f32) as i32;
        // Parabola that starts at base and ends at hoop_y.
        // y(t) = base - t*base + peak * 4t(1-t), where peak brings it above hoop.
        let start_y = base as f32;
        let end_y = hoop_y as f32;
        let peak_lift = (h as f32 * 0.6).min((base - hoop_y) as f32 + h as f32 * 0.4);
        let linear_y = start_y + t * (end_y - start_y);
        let arc_y = linear_y - peak_lift * 4.0 * t * (1.0 - t);
        let by = arc_y.round().clamp(0.0, base as f32) as i32;

        // Ball (2-dot body, slightly round via 4 dots).
        draw::dot_i(grid, bx, by);
        draw::dot_i(grid, bx + 1, by);
        draw::dot_i(grid, bx, by + 1);
        draw::dot_i(grid, bx + 1, by + 1);

        // Swish flash at 100%: fill net bright.
        if ctx.progress >= 0.999 {
            for ny in hoop_y..=(hoop_y + net_h).min(base) {
                draw::hline(grid, hoop_x, w.saturating_sub(1), ny);
            }
        }

        // Faint arc trail (dotted line of the parabola).
        for step in 0..20usize {
            let pt = step as f32 / 20.0;
            if pt >= t {
                break;
            }
            let tx = (pt * hoop_x as f32) as i32;
            let ty_lin = start_y + pt * (end_y - start_y);
            let ty = (ty_lin - peak_lift * 4.0 * pt * (1.0 - pt))
                .round()
                .clamp(0.0, base as f32) as i32;
            if step % 2 == 0 {
                draw::dot_i(grid, tx, ty);
            }
        }

        Ok(())
    }
}

// ── Soccer Goal ───────────────────────────────────────────────────────────────

struct SoccerGoal;
impl ProgressStyle for SoccerGoal {
    fn name(&self) -> &str {
        "soccer-goal"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Soccer ball curves into a goal net; net bulges on score"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = h.saturating_sub(1);

        // Goal frame: right side of bar.
        let goal_left = w.saturating_sub((w / 5).max(3));
        let goal_top = 0usize;
        // Crossbar.
        draw::hline(grid, goal_left, w.saturating_sub(1), goal_top);
        // Posts.
        draw::vline(grid, goal_left, goal_top, base);
        draw::vline(grid, w.saturating_sub(1), goal_top, base);

        // Net: diagonal grid inside the goal.
        let net_bulge: i32 = if ctx.progress >= 0.999 {
            ((ctx.time * 10.0).sin() * 1.5).round() as i32
        } else {
            0
        };
        for ny in (goal_top..=base).step_by(2) {
            for nx in (goal_left..w).step_by(3) {
                let bx = nx as i32 + if ny % 4 == 0 { net_bulge } else { -net_bulge };
                draw::dot_i(grid, bx, ny as i32);
            }
        }

        // Ball curve: starts bottom-left, curves into goal mouth.
        let t = ctx.eased;
        // Horizontal: 0 → goal_left.
        let bx = (t * goal_left as f32) as i32;
        // Vertical: starts at mid, curves up then down to goal mid.
        let start_y = (h as f32 * 0.7) as i32;
        let end_y = (h / 2) as i32;
        let curve_peak = ((h as f32 * 0.25) * (PI * t).sin()) as i32;
        let by =
            (start_y + (t * (end_y - start_y) as f32) as i32 - curve_peak).clamp(0, base as i32);

        // Ball: pentagon-ish 3x3 pattern.
        draw::dot_i(grid, bx, by);
        draw::dot_i(grid, bx + 1, by);
        draw::dot_i(grid, bx, by + 1);
        draw::dot_i(grid, bx + 1, by + 1);
        // Pentagons: alternating black dots on ball.
        if (ctx.time * 5.0) as usize % 2 == 0 {
            draw::dot_i(grid, bx + 1, by - 1);
        } else {
            draw::dot_i(grid, bx - 1, by + 1);
        }

        // Field: grass line at base.
        for gx in (0..w).step_by(4) {
            draw::dot(grid, gx.min(w.saturating_sub(1)), base);
            if gx + 1 < w {
                draw::dot(grid, gx + 1, base.saturating_sub(1));
            }
        }

        // Tint the arc path.
        let (cw, ch) = grid.dimensions();
        let swept = (ctx.eased * cw as f32) as usize;
        for cy in 0..ch {
            for cx in 0..swept.min(cw) {
                let t2 = if swept <= 1 {
                    0.0
                } else {
                    cx as f32 / (swept - 1) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t2));
            }
        }
        Ok(())
    }
}

// ── Swimming Laps ─────────────────────────────────────────────────────────────

struct SwimmingLaps;
impl ProgressStyle for SwimmingLaps {
    fn name(&self) -> &str {
        "swimming-laps"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Swimmer bobs across a lane; vblock lap counter fills on the right as laps complete"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cw, ch) = grid.dimensions();
        let mid = h / 2;

        // Lane boundaries: top and bottom dots.
        draw::hline(grid, 0, w.saturating_sub(1), 0);
        draw::hline(grid, 0, w.saturating_sub(1), h.saturating_sub(1));

        // Lane rope: dashed middle.
        let rope_y = mid;
        for rx in (0..w).step_by(3) {
            draw::dot(grid, rx.min(w.saturating_sub(1)), rope_y);
        }

        // Swimmer position: goes left-to-right then right-to-left on lap 2.
        // Number of laps = 4 total. Each lap = 0.25 of progress.
        let n_laps = 4usize;
        let lap_frac_f = ctx.eased * n_laps as f32;
        let current_lap = (lap_frac_f as usize).min(n_laps.saturating_sub(1));
        let within_lap = lap_frac_f.fract();

        // Alternate direction each lap.
        let going_right = current_lap % 2 == 0;
        let swim_x = if going_right {
            (within_lap * w as f32) as usize
        } else {
            w.saturating_sub(1)
                .saturating_sub((within_lap * w as f32) as usize)
        };
        let swim_x = swim_x.min(w.saturating_sub(3));

        // Vertical bob from time + small correction for which half of lane.
        let bob_amp = ((h / 4).max(1)) as f32 * 0.5;
        let lane_y = if going_right {
            mid.saturating_sub(2)
        } else {
            (mid + 2).min(h.saturating_sub(2))
        };
        let bob = (ctx.time * 6.0).sin() * bob_amp;
        let sy = (lane_y as f32 + bob)
            .round()
            .clamp(1.0, h.saturating_sub(2) as f32) as usize;

        // Swimmer body.
        draw::dot(grid, swim_x, sy);
        draw::dot(grid, swim_x + 1, sy);
        // Head.
        let head_off: i32 = if going_right { 2 } else { -1 };
        draw::dot_i(grid, swim_x as i32 + head_off, sy as i32 - 1);

        // Arm stroke — alternating.
        let stroke = (ctx.time * 4.0).sin();
        let arm_x = if going_right {
            swim_x as i32 + 3
        } else {
            swim_x as i32 - 2
        };
        draw::dot_i(grid, arm_x, sy as i32 + (stroke * 1.5).round() as i32);
        // Kick: tiny tail dots.
        let kick_x = if going_right {
            swim_x as i32 - 1
        } else {
            swim_x as i32 + 2
        };
        let kick_off = (stroke * -1.0).round() as i32;
        draw::dot_i(grid, kick_x, sy as i32 + kick_off);
        draw::dot_i(grid, kick_x, sy as i32 + kick_off + 1);

        // Lap counter: vblocks on right columns, one per completed lap.
        let laps_done = current_lap.min(n_laps);
        let counter_cells = (n_laps).min(cw);
        let start_cell = cw.saturating_sub(counter_cells);
        for i in 0..counter_cells {
            let filled = if i < laps_done { 8 } else { 0 };
            for cy in 0..ch {
                draw::vblock(grid, start_cell + i, cy, filled);
            }
        }

        // Water ripples trailing the swimmer.
        let ripple_dir: i32 = if going_right { -1 } else { 1 };
        for k in 1..4usize {
            let rx = swim_x as i32 + ripple_dir * (k as i32 * 2);
            if rx >= 0 && (rx as usize) < w {
                let ry_off = (k as f32 * 0.7 * (ctx.time * 3.0 + k as f32).sin()) as i32;
                draw::dot_i(grid, rx, sy as i32 + ry_off);
            }
        }

        Ok(())
    }
}

// ── Archery ───────────────────────────────────────────────────────────────────

struct Archery;
impl ProgressStyle for Archery {
    fn name(&self) -> &str {
        "archery"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Bow draws back with eased progress; arrow flies across to a multi-ring bullseye"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = h / 2;

        // Bullseye target: concentric rings on the right.
        let target_cx = w.saturating_sub(3) as i32;
        let target_cy = mid as i32;
        let max_r = ((h / 2).min(3)).max(1) as i32;
        for r in (1..=max_r).rev() {
            // Approximate circle with 8 cardinal dots.
            for &(dx, dy) in &[
                (r, 0),
                (-r, 0),
                (0, r),
                (0, -r),
                (r, r),
                (-r, r),
                (r, -r),
                (-r, -r),
            ] {
                draw::dot_i(grid, target_cx + dx, target_cy + dy);
            }
        }
        // Bull: center dot.
        draw::dot_i(grid, target_cx, target_cy);

        // Bow on the left side.
        let bow_x = 2i32;
        let bow_h = (h * 3 / 4).max(2) as i32;
        let bow_top = (mid as i32) - bow_h / 2;
        let bow_bot = bow_top + bow_h;
        // Bow limbs: arced using a few dots.
        for step in 0..=bow_h {
            let by = bow_top + step;
            // Arc bulge to the left (pulling back).
            let pull = ctx.eased; // 0 = no pull, 1 = full draw
            let arc = (PI * step as f32 / bow_h as f32).sin();
            let bx = bow_x - (arc * 2.0 * pull).round() as i32;
            draw::dot_i(grid, bx, by);
        }
        // Bowstring: straight line from top to bottom of bow.
        let string_x = bow_x + 1 - (ctx.eased * 1.5).round() as i32;
        draw::vline(
            grid,
            string_x.max(0) as usize,
            bow_top.max(0) as usize,
            bow_bot.min(h as i32 - 1) as usize,
        );

        // Arrow: travels from string to target when progress > 0.
        // Before release (eased < 0.8) the arrow is nocked and held back.
        // After release (eased >= 0.8) the arrow flies to the target.
        let t = ctx.eased;
        let arrow_head_x = if t < 0.8 {
            // Nocked: arrow tip just ahead of string.
            string_x + 1
        } else {
            // Flying: interpolate from nock to target.
            let flight_t = (t - 0.8) / 0.2;
            let nock_x = string_x + 1;
            (nock_x as f32 + flight_t * (target_cx - nock_x as i32) as f32).round() as i32
        };

        // Arrow shaft (horizontal dots from string to head).
        let shaft_start = (string_x + 1).max(0);
        let shaft_end = arrow_head_x.max(shaft_start);
        for ax in shaft_start..=shaft_end.min(target_cx) {
            draw::dot_i(grid, ax, mid as i32);
        }
        // Arrow head: small right-pointing tip.
        draw::dot_i(grid, arrow_head_x, mid as i32 - 1);
        draw::dot_i(grid, arrow_head_x + 1, mid as i32);
        draw::dot_i(grid, arrow_head_x, mid as i32 + 1);
        // Fletching: two dots at the tail.
        let tail_x = shaft_start - 1;
        draw::dot_i(grid, tail_x, mid as i32 - 1);
        draw::dot_i(grid, tail_x, mid as i32 + 1);

        Ok(())
    }
}

// ── Bowling ───────────────────────────────────────────────────────────────────

struct Bowling;
impl ProgressStyle for Bowling {
    fn name(&self) -> &str {
        "bowling"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Ball rolls down lane toward pins; pins scatter progressively at the end"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = h.saturating_sub(1);
        let mid = h / 2;

        // Lane: two guide lines.
        let lane_top = mid.saturating_sub(1);
        let lane_bot = (mid + 1).min(base);
        let guide_step = (w / 8).max(1);
        for gx in (0..w).step_by(guide_step) {
            draw::dot(grid, gx.min(w.saturating_sub(1)), lane_top);
            draw::dot(grid, gx.min(w.saturating_sub(1)), lane_bot);
        }

        // Pin rack at the right end: triangle of 10 pins in rows 4-3-2-1.
        let pin_area_x = w.saturating_sub((w / 6).max(4));
        // Knock-down threshold: pins fall when progress > 0.8, staggered.
        let knock_t = ((ctx.eased - 0.8) / 0.2).clamp(0.0, 1.0);
        let rows = [4usize, 3, 2, 1];
        let mut pin_idx = 0usize;
        let total_pins = 10usize;
        for (row, &count) in rows.iter().enumerate() {
            for col in 0..count {
                let px = pin_area_x + col * 2 + row;
                let px = px.min(w.saturating_sub(1));
                let pin_frac = pin_idx as f32 / total_pins as f32;
                let knocked = knock_t > pin_frac;
                if knocked {
                    // Pin lying down: dot to the right.
                    let scatter = ((ctx.time * 3.0 + pin_idx as f32).sin() * 1.5).round() as i32;
                    draw::dot_i(grid, px as i32 + 1 + scatter, base as i32);
                } else {
                    // Pin standing: vertical stack of 2.
                    draw::dot(grid, px, mid);
                    draw::dot(grid, px, mid.saturating_sub(1));
                }
                pin_idx += 1;
            }
        }

        // Ball: rolls from left to pin area.
        let ball_x = (ctx.eased * pin_area_x as f32) as usize;
        let ball_x = ball_x.min(pin_area_x.saturating_sub(2));
        // Slight vertical wobble on roll.
        let wobble = ((ctx.time * 12.0).sin() * 0.4).round() as i32;
        let by = (mid as i32 + wobble).clamp(0, base as i32) as usize;
        // Ball (2x2 dots).
        draw::dot(grid, ball_x, by);
        draw::dot(grid, (ball_x + 1).min(w.saturating_sub(1)), by);
        draw::dot(grid, ball_x, (by + 1).min(base));
        draw::dot(
            grid,
            (ball_x + 1).min(w.saturating_sub(1)),
            (by + 1).min(base),
        );
        // Spin dot.
        let spin_angle = ctx.time * 10.0;
        let sdx = spin_angle.cos().round() as i32;
        let sdy = spin_angle.sin().round() as i32;
        draw::dot_i(grid, ball_x as i32 + sdx, by as i32 + sdy);

        // Color: lane gets warmer toward pins.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32) as usize;
        for cy in 0..ch {
            for cx in 0..filled_cells.min(cw) {
                let t = if filled_cells <= 1 {
                    0.0
                } else {
                    cx as f32 / (filled_cells - 1) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Darts ─────────────────────────────────────────────────────────────────────

struct Darts;
impl ProgressStyle for Darts {
    fn name(&self) -> &str {
        "darts"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Dart flies toward concentric scoring rings; board glows as the dart closes in"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = h / 2;

        // Dartboard: concentric dot-rings on the right, centered.
        let board_cx = w.saturating_sub(2) as i32;
        let board_cy = mid as i32;
        let n_rings = ((h / 2).min(4)).max(1);
        for r in 1..=n_rings {
            let radius = r as i32;
            // Draw partial ellipse (wider than tall) using parametric dots.
            let steps = (radius * 8).max(8) as usize;
            for s in 0..steps {
                let angle = 2.0 * PI * s as f32 / steps as f32;
                let ex = (board_cx + (angle.cos() * radius as f32 * 1.5).round() as i32).max(0);
                let ey = (board_cy + (angle.sin() * radius as f32 * 0.75).round() as i32).max(0);
                draw::dot_i(grid, ex, ey);
            }
        }
        // Bull.
        draw::dot_i(grid, board_cx, board_cy);

        // Dart: travels horizontally from the left.
        let dart_x = (ctx.eased * board_cx as f32).round() as i32;
        let dart_y = board_cy;

        // Slight arc drop: gravity dip over the flight.
        let drop = if dart_x < board_cx {
            let flight_t = dart_x as f32 / board_cx.max(1) as f32;
            (flight_t * (1.0 - flight_t) * 2.0 * h as f32 * 0.2).round() as i32
        } else {
            0
        };
        let dart_y_dropped = (dart_y + drop).clamp(0, h as i32 - 1);

        // Dart body: tip then shaft then flights.
        // Tip.
        draw::dot_i(grid, dart_x, dart_y_dropped);
        // Barrel (2 dots back).
        draw::dot_i(grid, dart_x - 1, dart_y_dropped);
        draw::dot_i(grid, dart_x - 2, dart_y_dropped);
        // Flights: angled fork.
        draw::dot_i(grid, dart_x - 3, dart_y_dropped - 1);
        draw::dot_i(grid, dart_x - 3, dart_y_dropped + 1);
        draw::dot_i(grid, dart_x - 4, dart_y_dropped - 2);
        draw::dot_i(grid, dart_x - 4, dart_y_dropped + 2);

        // Board glow: as dart gets close, fill innermost ring column.
        if ctx.eased > 0.8 {
            let glow_t = (ctx.eased - 0.8) / 0.2;
            let glow_r = (glow_t * n_rings as f32).round() as i32;
            for gr in 1..=glow_r.min(n_rings as i32) {
                draw::dot_i(grid, board_cx, board_cy - gr);
                draw::dot_i(grid, board_cx, board_cy + gr);
                draw::dot_i(grid, board_cx - gr, board_cy);
                draw::dot_i(grid, board_cx + gr, board_cy);
            }
        }

        Ok(())
    }
}

// ── High Jump ─────────────────────────────────────────────────────────────────

struct HighJump;
impl ProgressStyle for HighJump {
    fn name(&self) -> &str {
        "high-jump"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Athlete arcs over a bar that rises with progress; backflop pose at peak"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = h.saturating_sub(1);

        // High-jump mat: fill at the right.
        let mat_x = w.saturating_sub((w / 5).max(2));
        draw::hline(grid, mat_x, w.saturating_sub(1), base);

        // Bar uprights: two vertical posts.
        let post_h = (ctx.eased * base as f32).round() as usize;
        let post_h = post_h.max(1);
        let lpost = mat_x.saturating_sub(2).min(w.saturating_sub(1));
        let rpost = (mat_x + 1).min(w.saturating_sub(1));
        draw::vline(grid, lpost, base.saturating_sub(post_h), base);
        draw::vline(grid, rpost, base.saturating_sub(post_h), base);
        // Crossbar at the top of the posts.
        let bar_y = base.saturating_sub(post_h);
        draw::hline(grid, lpost, rpost, bar_y);

        // Athlete: progresses from approach run to arc over bar.
        // phase: 0 = running in, 0.5 = peak arc, 1 = landing.
        let t = ctx.eased;
        // Athlete x: runs toward post then curves over and lands on mat.
        let approach_x = (t * lpost as f32) as usize;
        // Arc: highest at t=0.5, using a parabola shifted by the bar height.
        let arc_frac = 4.0 * t * (1.0 - t); // 0→1→0
        let arc_lift = (arc_frac * post_h as f32).round() as usize;
        let ath_base = base.saturating_sub(arc_lift);

        let ax = approach_x.min(w.saturating_sub(3));

        // Body shape changes with arc phase.
        if t < 0.4 {
            // Running: upright torso.
            draw::dot(grid, ax, ath_base);
            draw::dot(grid, ax + 1, ath_base);
            draw::dot(grid, ax, ath_base.saturating_sub(1));
            // Head.
            draw::dot_i(grid, ax as i32 + 1, ath_base as i32 - 2);
            // Legs.
            let leg_phase = (ctx.time * 8.0).sin();
            draw::dot_i(grid, ax as i32 + 1, ath_base as i32 + 1);
            draw::dot_i(
                grid,
                ax as i32 - 1 + (leg_phase * 1.0).round() as i32,
                ath_base as i32 + 1,
            );
        } else {
            // Arcing: horizontal body (Fosbury flop).
            let bod_y = ath_base;
            draw::dot(grid, ax, bod_y);
            draw::dot(grid, ax + 1, bod_y);
            draw::dot(grid, ax + 2, bod_y);
            // Arch the back: middle dot slightly higher.
            draw::dot_i(grid, ax as i32 + 1, bod_y as i32 - 1);
            // Head tilted back.
            draw::dot_i(grid, ax as i32 - 1, bod_y as i32 - 1);
            // Legs up.
            draw::dot_i(grid, ax as i32 + 3, bod_y as i32 - 1);
            draw::dot_i(grid, ax as i32 + 4, bod_y as i32 - 2);
        }

        // Approach runway: ground to the left.
        draw::hline(grid, 0, lpost.saturating_sub(1), base);

        Ok(())
    }
}

// ── Weightlifting ─────────────────────────────────────────────────────────────

struct Weightlifting;
impl ProgressStyle for Weightlifting {
    fn name(&self) -> &str {
        "weightlifting"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Barbell lifted overhead; plate stack height and arm angle track eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = h.saturating_sub(1);
        let cx = w / 2;

        // Lift progress: 0 = bar on floor, 1 = fully overhead.
        // Use eased so the lift has inertia.
        let lift = ctx.eased;
        // Barbell y position: from near base to top.
        let bar_range = (base.saturating_sub(2)) as f32;
        let bar_y = base
            .saturating_sub(2)
            .saturating_sub((lift * bar_range) as usize);
        let bar_y = bar_y.min(base.saturating_sub(1));

        // Barbell shaft.
        let shaft_half = (w / 4).max(2);
        draw::hline(
            grid,
            cx.saturating_sub(shaft_half),
            (cx + shaft_half).min(w.saturating_sub(1)),
            bar_y,
        );

        // Plates: stack at each end proportional to progress.
        let max_plates = 4usize;
        let plates_on = (lift * max_plates as f32).ceil() as usize;
        for p in 0..plates_on.min(max_plates) {
            let plate_off = p + 1;
            // Left plate.
            let lpx = cx.saturating_sub(shaft_half).saturating_sub(plate_off);
            draw::vline(grid, lpx, bar_y.saturating_sub(1), (bar_y + 1).min(base));
            // Right plate.
            let rpx = (cx + shaft_half + plate_off).min(w.saturating_sub(1));
            draw::vline(grid, rpx, bar_y.saturating_sub(1), (bar_y + 1).min(base));
        }
        // Outer collar dots.
        let lcollar = cx.saturating_sub(shaft_half + plates_on + 1);
        let rcollar = (cx + shaft_half + plates_on + 1).min(w.saturating_sub(1));
        draw::dot(grid, lcollar, bar_y);
        draw::dot(grid, rcollar, bar_y);

        // Athlete: body adapts to lift phase.
        // Legs always at base, torso/arms angle with lift.
        let torso_top = bar_y.saturating_sub(1);
        let hip_y = (base.saturating_sub(1)).min(base);

        // Legs: two dots each side.
        let leg_spread = (w / 10).max(1);
        draw::vline(grid, cx.saturating_sub(leg_spread), hip_y, base);
        draw::vline(
            grid,
            (cx + leg_spread).min(w.saturating_sub(1)),
            hip_y,
            base,
        );

        // Torso.
        draw::vline(grid, cx, torso_top, hip_y);

        // Arms: angle from hips up to bar.
        // Simplified: diagonal from shoulder to bar ends.
        let shoulder_y = torso_top.saturating_sub(1);
        let lshoulder_x = cx.saturating_sub(1);
        let rshoulder_x = (cx + 1).min(w.saturating_sub(1));

        // Left arm: shoulder → left end of bar.
        let arm_steps = 3usize;
        for step in 0..=arm_steps {
            let at = step as f32 / arm_steps as f32;
            let ax = (lshoulder_x as f32 + at * (lcollar as i32 - lshoulder_x as i32) as f32)
                .round() as i32;
            let ay =
                (shoulder_y as f32 + at * (bar_y as i32 - shoulder_y as i32) as f32).round() as i32;
            draw::dot_i(grid, ax, ay);
        }
        // Right arm.
        for step in 0..=arm_steps {
            let at = step as f32 / arm_steps as f32;
            let ax = (rshoulder_x as f32 + at * (rcollar as i32 - rshoulder_x as i32) as f32)
                .round() as i32;
            let ay =
                (shoulder_y as f32 + at * (bar_y as i32 - shoulder_y as i32) as f32).round() as i32;
            draw::dot_i(grid, ax, ay);
        }

        // Head.
        draw::dot_i(grid, cx as i32, torso_top as i32 - 2);
        draw::dot_i(grid, cx as i32, torso_top as i32 - 1);

        // Strain shimmer at peak.
        if ctx.progress >= 0.9 {
            let shake = ((ctx.time * 20.0).sin() * 0.5).round() as i32;
            draw::dot_i(grid, cx as i32 + shake, bar_y as i32 - 1);
        }

        Ok(())
    }
}

// ── Tennis Rally ──────────────────────────────────────────────────────────────

struct TennisRally;
impl ProgressStyle for TennisRally {
    fn name(&self) -> &str {
        "tennis-rally"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Ball bounces between baselines; a smooth hbar fill tracks rally progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cw, ch) = grid.dimensions();

        // Bottom row: hbar fill showing overall progress.
        if ch > 0 {
            draw::hbar(grid, ch.saturating_sub(1), ctx.eased);
        }

        // Court area: rows above the hbar.
        let court_rows = ch.saturating_sub(1);
        if court_rows == 0 {
            return Ok(());
        }
        let court_h = court_rows * 4;
        let court_base = court_h.saturating_sub(1);

        // Court lines: baselines and net.
        draw::hline(grid, 0, w.saturating_sub(1), court_base);
        draw::hline(grid, 0, w.saturating_sub(1), 0);
        let net_x = w / 2;
        draw::vline(grid, net_x, 0, court_base);

        // Service line markers.
        let sl = w / 4;
        let sr = w * 3 / 4;
        for sy in (0..court_base).step_by(2) {
            draw::dot(grid, sl.min(w.saturating_sub(1)), sy);
            draw::dot(grid, sr.min(w.saturating_sub(1)), sy);
        }

        // Ball: bounces back and forth driven by time.
        // Horizontal: sinusoidal across width (rally speed increases with progress).
        let rally_speed = 1.5 + ctx.progress * 3.0;
        let bx_raw = ((ctx.time * rally_speed).sin() * 0.5 + 0.5) * (w - 1) as f32;
        let bx = bx_raw.round() as usize;
        let bx = bx.min(w.saturating_sub(1));

        // Vertical: parabolic bounce — abs(sin) gives repeated parabola shape.
        let bounce_freq = rally_speed * 2.0;
        let bounce = ((ctx.time * bounce_freq).sin()).abs();
        let by_raw = court_base as f32 - bounce * (court_base as f32 * 0.7);
        let by = by_raw.round().clamp(0.0, court_base as f32) as usize;

        // Ball dot.
        draw::dot(grid, bx, by);
        // Second dot for visibility (right or below).
        if bx + 1 < w {
            draw::dot(grid, bx + 1, by);
        }

        // Ball shadow on the court floor.
        draw::dot(grid, bx, court_base);

        // Players: simple stick figures at each end.
        // Left player (server side).
        let lp_x = 1usize;
        let lp_mid = court_h / 2;
        draw::dot(grid, lp_x, lp_mid.saturating_sub(1));
        draw::dot(grid, lp_x, lp_mid);
        draw::dot(grid, lp_x, (lp_mid + 1).min(court_base));
        // Right player.
        let rp_x = w.saturating_sub(2);
        draw::dot(grid, rp_x, lp_mid.saturating_sub(1));
        draw::dot(grid, rp_x, lp_mid);
        draw::dot(grid, rp_x, (lp_mid + 1).min(court_base));

        // Tint court.
        for cy in 0..court_rows {
            for cx in 0..cw {
                let t = if cw <= 1 {
                    0.5
                } else {
                    cx as f32 / (cw - 1) as f32
                };
                // Subtle tint — muted by applying only partially.
                let color = ctx.palette.sample(t);
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ── Cycling Peloton ───────────────────────────────────────────────────────────

struct CyclingPeloton;
impl ProgressStyle for CyclingPeloton {
    fn name(&self) -> &str {
        "cycling-peloton"
    }
    fn theme(&self) -> &str {
        "sports"
    }
    fn describe(&self) -> &str {
        "Tightly-packed peloton advances; each bike's wheels spin with time via dot rotation"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = h.saturating_sub(1);
        let rider_h = (h / 2).max(2);
        let body_y = base.saturating_sub(rider_h);

        // Road: baseline.
        draw::hline(grid, 0, w.saturating_sub(1), base);
        // Road markings.
        let center_y = (base + body_y) / 2;
        for rx in (0..w).step_by(5) {
            draw::dot(grid, rx.min(w.saturating_sub(1)), center_y);
        }

        // Pack of riders: progress dictates how far the leading edge is.
        let lead_x = (ctx.eased * w as f32) as usize;
        let lead_x = lead_x.min(w.saturating_sub(1));

        // Number of riders: scales with bar width, minimum 2.
        let n_riders = (w / 10).max(2).min(8);
        // Pack depth behind leader.
        let pack_depth = (n_riders as f32 * 5.0) as usize;

        // Wheel spin angle (radians) driven by time.
        let spin = ctx.time * 8.0;
        let wheel_r = ((h / 4).max(1)) as f32;

        for i in 0..n_riders {
            // Space riders behind the leader.
            let offset = i * (pack_depth / n_riders.max(1));
            let rx = lead_x.saturating_sub(offset);
            if rx == 0 && i > 0 {
                continue;
            }
            let rx = rx.min(w.saturating_sub(3));

            // Each rider: two wheels + frame + body.
            let front_wheel_x = (rx + 2).min(w.saturating_sub(1)) as i32;
            let rear_wheel_x = rx as i32;
            let wheel_y = (base - 1) as i32;

            // Wheel spokes (2 per wheel, cross-shaped, rotating).
            for spoke in 0..2usize {
                let angle = spin + spoke as f32 * PI;
                let sdx = (angle.cos() * wheel_r * 0.8).round() as i32;
                let sdy = (angle.sin() * wheel_r * 0.5).round() as i32;
                // Front wheel.
                draw::dot_i(grid, front_wheel_x + sdx, wheel_y + sdy);
                draw::dot_i(grid, front_wheel_x - sdx, wheel_y - sdy);
                // Rear wheel.
                draw::dot_i(grid, rear_wheel_x + sdx, wheel_y + sdy);
                draw::dot_i(grid, rear_wheel_x - sdx, wheel_y - sdy);
            }
            // Wheel rims (hub dots).
            draw::dot_i(grid, front_wheel_x, wheel_y);
            draw::dot_i(grid, rear_wheel_x, wheel_y);

            // Frame: diagonal from rear wheel to front wheel top.
            draw::dot_i(grid, rear_wheel_x + 1, wheel_y - 1);
            draw::dot_i(grid, front_wheel_x - 1, wheel_y - 1);

            // Rider body.
            let seat_x = rear_wheel_x + 1;
            let seat_y = wheel_y - 2;
            draw::dot_i(grid, seat_x, seat_y);
            // Torso leans forward.
            draw::dot_i(grid, seat_x + 1, seat_y - 1);
            draw::dot_i(grid, seat_x + 1, seat_y - 2);
            // Head.
            draw::dot_i(grid, front_wheel_x, seat_y - 2);
        }

        // Gradient tint of the swept portion.
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32) as usize;
        for cy in 0..ch {
            for cx in 0..filled.min(cw) {
                let t = if filled <= 1 {
                    0.0
                } else {
                    cx as f32 / (filled - 1) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}
