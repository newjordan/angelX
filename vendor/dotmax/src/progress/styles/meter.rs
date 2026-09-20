//! Radial meter / gauge / dial progress styles.
//!
//! Every style in this theme maps `ctx.eased` onto a needle angle or a swept
//! arc rather than a linear fill, giving each gauge a structural identity that
//! no palette swap can replicate.  Twenty structurally distinct dials:
//!
//! - `speedometer`      — 270° arc + tick marks + redline zone + needle
//! - `ring-progress`    — thin full-circle ring that fills clockwise from 12 o'clock
//! - `half-gauge`       — 180° semicircle fuel/temperature gauge with needle
//! - `tachometer`       — overshooting + settling needle (time wobble)
//! - `donut`            — thick filled pie wedge that grows with eased
//! - `signal-arc`       — concentric arc bands, VU-style, lighting up by tier
//! - `compass`          — cardinal tick marks + rotating bearing needle
//! - `vu-needle`        — analog VU bounce around the eased level
//! - `segmented-ring`   — discrete ring segments lighting up one by one
//! - `dual-gauge`       — two concentric arcs filling at different speeds
//! - `clock-face`       — hour + minute hands set by progress
//! - `pressure-dial`    — danger arc zone + damped settling needle
//! - `altimeter`        — aviation two-needle dial: fast 10× needle + slow needle
//! - `radar-scope`      — range rings + rotating sweep; contact blips appear with progress
//! - `attitude-horizon` — artificial horizon: pitch climbs with eased, roll sways with time
//! - `bubble-level`     — full-width spirit level; bubble drifts to centre as eased → 1
//! - `edgewise-meter`   — full-width panel meter: horizontal scale + slanted pointer
//! - `dial-bank`        — three mini-dials filling in sequence across the width
//! - `galvanometer`     — top-pivot hanging needle over a bottom arc scale
//! - `anemometer`       — spinning wind-cup rotor; rotation speed tracks eased

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ────────────────────────────────────────────────────────────────────────────
// Registry
// ────────────────────────────────────────────────────────────────────────────

/// All styles in the `meter` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per radial-gauge bar.  Every style
/// auto-fits the dial radius to `min(dot_width/2, dot_height/2)` so it renders
/// correctly even on 1-cell-tall grids (radius simply collapses to ≥1).
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Speedometer),
        Box::new(RingProgress),
        Box::new(HalfGauge),
        Box::new(Tachometer),
        Box::new(Donut),
        Box::new(SignalArc),
        Box::new(Compass),
        Box::new(VuNeedle),
        Box::new(SegmentedRing),
        Box::new(DualGauge),
        Box::new(ClockFace),
        Box::new(PressureDial),
        Box::new(Altimeter),
        Box::new(RadarScope),
        Box::new(AttitudeHorizon),
        Box::new(BubbleLevel),
        Box::new(EdgewiseMeter),
        Box::new(DialBank),
        Box::new(Galvanometer),
        Box::new(Anemometer),
    ]
}

// ────────────────────────────────────────────────────────────────────────────
// Low-level drawing helpers
// ────────────────────────────────────────────────────────────────────────────

/// Integer Bresenham line in dot-space.  Step-bounded; OOB dots silently
/// discarded by `draw::dot_i`.
fn bresenham(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let max_steps = (dx.abs() + dy.abs() + 2) as usize;
    for _ in 0..=max_steps {
        draw::dot_i(grid, x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Draw an arc of dots: centre `(cx,cy)`, radius `r` dots, from angle `a0` to
/// `a1` (radians, screen coords: y-axis points down so `sin` is inverted).
/// The arc is sampled at `steps` evenly-spaced angles; step count is clamped
/// so it is always ≥ 4 and never overruns.
fn arc(grid: &mut BrailleGrid, cx: i32, cy: i32, r: i32, a0: f32, a1: f32, steps: usize) {
    let steps = steps.max(4);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let angle = a0 + t * (a1 - a0);
        let x = cx + (r as f32 * angle.cos()).round() as i32;
        let y = cy - (r as f32 * angle.sin()).round() as i32; // flip y
        draw::dot_i(grid, x, y);
    }
}

/// Draw a thick arc by rendering concentric arcs from radius `r_inner` to
/// `r_outer` inclusive (in dot units).
fn thick_arc(
    grid: &mut BrailleGrid,
    cx: i32,
    cy: i32,
    r_inner: i32,
    r_outer: i32,
    a0: f32,
    a1: f32,
) {
    let r0 = r_inner.max(1);
    let r1 = r_outer.max(r0);
    for r in r0..=r1 {
        let steps = ((r as f32 * (a1 - a0).abs() * 2.0).round() as usize).max(4);
        arc(grid, cx, cy, r, a0, a1, steps);
    }
}

/// Compute centre and best-fit radius for a dial that must stay inside the
/// dot grid of `(dw, dh)`.  Returns `(cx, cy, radius)` all in dot units.
/// Radius is at least 1.
fn dial_fit(dw: usize, dh: usize) -> (i32, i32, i32) {
    let cx = (dw / 2) as i32;
    let cy = (dh / 2) as i32;
    // leave 1-dot margin on each side
    let r = (cx.min(cy) - 1).max(1);
    (cx, cy, r)
}

// ────────────────────────────────────────────────────────────────────────────
// 1. Speedometer — 270° arc, tick marks, redline, line needle
// ────────────────────────────────────────────────────────────────────────────

struct Speedometer;
impl ProgressStyle for Speedometer {
    fn name(&self) -> &str {
        "speedometer"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Classic 270° arc speedometer: tick marks at every 10%, a redline zone \
         above 80%, and a line needle pointing to the eased progress value"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // The dial spans 270°, from 225° clockwise to -45° (i.e. -315°…-45° in
        // standard screen angles where 0=right, positive=counter-clockwise).
        // We use standard math angles (CCW positive, y flipped in draw).
        // Start = bottom-left (225°), sweep CCW to bottom-right (-45° = 315°).
        let a_start = 225_f32.to_radians(); // left of bottom; sweep ends at -45° (315°)
        let span = -(270_f32.to_radians()); // clockwise = negative in math angle

        // Draw outer arc (track).
        let arc_steps = ((r as f32 * 2.0 * PI * 0.75).round() as usize).max(4);
        arc(grid, cx, cy, r, a_start, a_start + span, arc_steps);

        // Tick marks: 11 ticks (0%, 10%, …, 100%).
        for i in 0..=10 {
            let frac = i as f32 / 10.0;
            let angle = a_start + frac * span;
            let tick_len = if i % 5 == 0 {
                (r / 4).max(1)
            } else {
                (r / 6).max(1)
            };
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Redline arc: 80–100% of span.
        let r_inner = (r - (r / 4).max(1)).max(1);
        let a_red0 = a_start + 0.8 * span;
        let a_red1 = a_start + span;
        let red_steps = ((r as f32 * (a_red1 - a_red0).abs()).round() as usize).max(4);
        for rr in r_inner..=r {
            arc(grid, cx, cy, rr, a_red0, a_red1, red_steps);
        }

        // Needle: from centre toward rim at the eased angle.
        let needle_angle = a_start + ctx.eased.clamp(0.0, 1.0) * span;
        let nx = cx + (r as f32 * needle_angle.cos()).round() as i32;
        let ny = cy - (r as f32 * needle_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, nx, ny);

        // Colour: tint rim with palette.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = cx_c as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 2. Ring progress — thin full-circle ring filling clockwise from 12 o'clock
// ────────────────────────────────────────────────────────────────────────────

struct RingProgress;
impl ProgressStyle for RingProgress {
    fn name(&self) -> &str {
        "ring-progress"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Thin full-circle ring that fills clockwise from 12 o'clock; \
         the filled arc length = eased × 2π, the unfilled portion stays dim"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // 12 o'clock = 90° in standard math angle (y-up); since we flip y,
        // screen-up = math-up, so 12 o'clock is angle 90° = π/2.
        let a_top = PI / 2.0;
        let full_steps = ((r as f32 * 2.0 * PI).round() as usize).max(4);

        // Unfilled track (dim) — always draw the whole ring first.
        // We use a sparser dot pattern for the track by sampling every other step.
        for i in (0..=full_steps).step_by(2) {
            let t = i as f32 / full_steps as f32;
            let angle = a_top - t * 2.0 * PI; // clockwise: subtract
            let x = cx + (r as f32 * angle.cos()).round() as i32;
            let y = cy - (r as f32 * angle.sin()).round() as i32;
            draw::dot_i(grid, x, y);
        }

        // Filled arc — dense dots over eased fraction.
        let filled_steps = ((r as f32 * 2.0 * PI * ctx.eased.clamp(0.0, 1.0)).round() as usize)
            .max(if ctx.eased > 0.0 { 1 } else { 0 });
        for i in 0..=filled_steps {
            let t = if filled_steps == 0 {
                0.0
            } else {
                i as f32 / filled_steps as f32 * ctx.eased
            };
            let angle = a_top - t * 2.0 * PI;
            let x = cx + (r as f32 * angle.cos()).round() as i32;
            let y = cy - (r as f32 * angle.sin()).round() as i32;
            draw::dot_i(grid, x, y);
            // Also fill the ring with one dot inside for slightly thicker look.
            if r > 2 {
                let xi = cx + ((r - 1) as f32 * angle.cos()).round() as i32;
                let yi = cy - ((r - 1) as f32 * angle.sin()).round() as i32;
                draw::dot_i(grid, xi, yi);
            }
        }

        // Gradient tint.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = ctx.eased;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 3. Half-gauge — 180° semicircle (fuel / temperature) with needle
// ────────────────────────────────────────────────────────────────────────────

struct HalfGauge;
impl ProgressStyle for HalfGauge {
    fn name(&self) -> &str {
        "half-gauge"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "180° semicircle gauge (fuel/temperature style): arc spans left→right \
         across the top of the cell, a needle swings from empty (left) to full (right)"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // Semicircle: from 180° (left) CCW to 0° (right) in standard math coords.
        // In screen coords (y down/flipped), this draws the arc at the TOP of the grid.
        let a_left = PI; // 180°
        let a_right = 0.0_f32; // 0°

        // Outer arc.
        let arc_steps = ((r as f32 * PI).round() as usize).max(4);
        arc(grid, cx, cy, r, a_left, a_right, arc_steps);

        // Tick marks at 0, 25, 50, 75, 100% positions.
        for i in 0..=4 {
            let frac = i as f32 / 4.0;
            let angle = a_left + frac * (a_right - a_left);
            let tick_len = if i % 2 == 0 {
                (r / 4).max(1)
            } else {
                (r / 6).max(1)
            };
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Baseline: horizontal line from left arc end to right arc end.
        let lx = cx - r;
        let rx = cx + r;
        if lx >= 0 {
            draw::hline(
                grid,
                lx as usize,
                rx.min(dw as i32 - 1).max(0) as usize,
                cy as usize,
            );
        }

        // Needle: pivots from bottom centre (cx, cy), sweeps up into semicircle.
        let needle_angle = a_left + ctx.eased.clamp(0.0, 1.0) * (a_right - a_left);
        let needle_r = (r - 1).max(1);
        let nx = cx + (needle_r as f32 * needle_angle.cos()).round() as i32;
        let ny = cy - (needle_r as f32 * needle_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, nx, ny);

        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = cx_c as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 4. Tachometer — like speedometer, but needle overshoots and settles
// ────────────────────────────────────────────────────────────────────────────

struct Tachometer;
impl ProgressStyle for Tachometer {
    fn name(&self) -> &str {
        "tachometer"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Tachometer: 270° rev-counter dial with a needle that overshoots the \
         target and damps back — the settle wobble is driven by ctx.time so \
         the needle visibly hunts to the eased value"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        let a_start = 225_f32.to_radians();
        let span = -(270_f32.to_radians());

        // Arc track.
        let arc_steps = ((r as f32 * 2.0 * PI * 0.75).round() as usize).max(4);
        arc(grid, cx, cy, r, a_start, a_start + span, arc_steps);

        // Denser tick marks: every 5% (21 ticks).
        for i in 0..=20 {
            let frac = i as f32 / 20.0;
            let angle = a_start + frac * span;
            let tick_len = if i % 4 == 0 {
                (r / 3).max(1)
            } else {
                (r / 7).max(1)
            };
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Overshoot + damped settle wobble.
        // Model: needle = eased + A·sin(ω·t)·exp(-λ·t)
        // A = 0.12 of full span, ω = 8 rad/s, λ = 1.5 (dies in ~2 s).
        let wobble = 0.12 * (8.0 * ctx.time).sin() * (-1.5 * ctx.time).exp();
        let effective = (ctx.eased + wobble).clamp(0.0, 1.0);
        let needle_angle = a_start + effective * span;
        let nx = cx + (r as f32 * needle_angle.cos()).round() as i32;
        let ny = cy - (r as f32 * needle_angle.sin()).round() as i32;
        // Draw needle with a small back-fin for drama.
        bresenham(grid, cx, cy, nx, ny);
        let back_angle = needle_angle + PI;
        let bx = cx + ((r / 6).max(1) as f32 * back_angle.cos()).round() as i32;
        let by = cy - ((r / 6).max(1) as f32 * back_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, bx, by);

        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = cx_c as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 5. Donut — thick filled pie wedge growing clockwise from 12 o'clock
// ────────────────────────────────────────────────────────────────────────────

struct Donut;
impl ProgressStyle for Donut {
    fn name(&self) -> &str {
        "donut"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Thick donut / pie: a filled wedge between an inner and outer radius \
         sweeps clockwise from 12 o'clock as eased grows — the hole stays empty, \
         the wedge fills with braille dots"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // Donut ring: outer = r, inner = r * 0.45.
        let r_inner = ((r as f32 * 0.45).round() as i32).max(1);

        let a_top = PI / 2.0;
        let sweep = ctx.eased.clamp(0.0, 1.0) * 2.0 * PI;
        let a_end = a_top - sweep; // clockwise = subtract

        // Outer arc (always draw full rim).
        let rim_steps = ((r as f32 * 2.0 * PI).round() as usize).max(4);
        arc(grid, cx, cy, r, a_top, a_top - 2.0 * PI, rim_steps);

        // Inner arc (hole boundary).
        let hole_steps = ((r_inner as f32 * 2.0 * PI).round() as usize).max(4);
        arc(grid, cx, cy, r_inner, a_top, a_top - 2.0 * PI, hole_steps);

        // Filled wedge for the filled portion.
        if sweep > 0.001 {
            thick_arc(grid, cx, cy, r_inner, r, a_top, a_end);
        }

        // Tint the whole grid with the eased colour.
        let color = ctx.palette.sample(ctx.eased);
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 6. Signal-arc — concentric arc bands lighting up by tier (cell-signal style)
// ────────────────────────────────────────────────────────────────────────────

struct SignalArc;
impl ProgressStyle for SignalArc {
    fn name(&self) -> &str {
        "signal-arc"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Concentric arc bands like a cell-signal or Wi-Fi icon: each tier is a \
         thicker arc; bands light up from innermost outward as eased rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r_max) = dial_fit(dw, dh);

        // Signal arcs span 90° centred on the top (90° in math = 12 o'clock).
        let a_left = 135_f32.to_radians();
        let a_right = 45_f32.to_radians();

        // Number of tiers depends on radius; at least 1.
        let n_tiers = ((r_max / 3).max(1) as usize).min(5);
        let lit = (ctx.eased.clamp(0.0, 1.0) * n_tiers as f32).ceil() as usize;

        // Base pivot dot.
        draw::dot_i(grid, cx, cy);

        for tier in 0..n_tiers {
            if tier >= lit {
                break;
            }
            let r_inner = (tier as i32 * (r_max / n_tiers as i32).max(1)) + 1;
            let r_outer = ((tier as i32 + 1) * (r_max / n_tiers as i32).max(1)).min(r_max);
            if r_inner > r_outer {
                continue;
            }
            thick_arc(grid, cx, cy, r_inner, r_outer, a_left, a_right);
        }

        // Tint by tier.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = cx_c as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 7. Compass — cardinal tick marks + rotating bearing needle
// ────────────────────────────────────────────────────────────────────────────

struct Compass;
impl ProgressStyle for Compass {
    fn name(&self) -> &str {
        "compass"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Compass rose: a full circle with N/E/S/W tick marks and a bearing \
         needle that points to eased × 360° — progress literally steers the heading"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // Full circle.
        let steps = ((r as f32 * 2.0 * PI).round() as usize).max(4);
        arc(grid, cx, cy, r, 0.0, 2.0 * PI, steps);

        // Cardinal ticks: N=90°, E=0°, S=270°, W=180°.
        let cardinals = [
            PI / 2.0,  // N (up)
            0.0_f32,   // E (right)
            -PI / 2.0, // S (down)
            PI,        // W (left)
        ];
        for (i, &angle) in cardinals.iter().enumerate() {
            let tick_len = if i % 2 == 0 {
                (r / 3).max(1) // N, S longer
            } else {
                (r / 4).max(1)
            };
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Intercardinal ticks (NE, SE, SW, NW): shorter.
        for i in 0..4 {
            let angle = PI / 4.0 + i as f32 * PI / 2.0;
            let tick_len = (r / 6).max(1);
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Bearing needle: 0 = N (90°), clockwise → subtract.
        // eased = 0 → pointing north; eased = 1 → full 360° back to north.
        let bearing = PI / 2.0 - ctx.eased.clamp(0.0, 1.0) * 2.0 * PI;
        let needle_r = (r - (r / 5).max(1)).max(1);
        let nx = cx + (needle_r as f32 * bearing.cos()).round() as i32;
        let ny = cy - (needle_r as f32 * bearing.sin()).round() as i32;
        bresenham(grid, cx, cy, nx, ny);

        // Counter-needle (tail, half length, opposite direction).
        let tail_r = (r / 4).max(1);
        let tx = cx + (tail_r as f32 * (bearing + PI).cos()).round() as i32;
        let ty = cy - (tail_r as f32 * (bearing + PI).sin()).round() as i32;
        bresenham(grid, cx, cy, tx, ty);

        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let color = ctx.palette.sample(ctx.eased);
            draw::tint_row(grid, cy_c, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 8. VU-needle — analog VU meter that bounces around the eased level
// ────────────────────────────────────────────────────────────────────────────

struct VuNeedle;
impl ProgressStyle for VuNeedle {
    fn name(&self) -> &str {
        "vu-needle"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Analog VU-meter needle: the needle bounces rhythmically around the \
         eased level driven by ctx.time, mimicking the momentum of a real \
         moving-coil meter in a recording studio"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // VU meter spans 60° left to 60° right of vertical (12 o'clock).
        // −30° to +30° from North = 120° to 60° in standard math.
        let a_left = (90.0 + 60.0_f32).to_radians();
        let a_right = (90.0 - 60.0_f32).to_radians();
        let span = a_right - a_left; // negative (clockwise)

        // Arc.
        let arc_steps = ((r as f32 * (a_left - a_right)).round() as usize).max(4);
        arc(grid, cx, cy, r, a_left, a_right, arc_steps);

        // Ticks: 7 marks at 0, 10, 20, 30 (centre), 40, 50, 60% of span.
        for i in 0..=6 {
            let frac = i as f32 / 6.0;
            let angle = a_left + frac * span;
            let tick_len = if i == 3 {
                (r / 3).max(1)
            } else {
                (r / 5).max(1)
            };
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Bounce: sum of harmonics simulating ballistic meter momentum.
        // Faster bounce at higher levels (heavier signal).
        let freq = 3.0 + ctx.eased * 4.0;
        let bounce = 0.08 * (freq * ctx.time).sin() + 0.04 * (freq * 1.7 * ctx.time + 0.3).sin();
        let effective = (ctx.eased + bounce).clamp(0.0, 1.0);

        let needle_angle = a_left + effective * span;
        let nx = cx + (r as f32 * needle_angle.cos()).round() as i32;
        let ny = cy - (r as f32 * needle_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, nx, ny);

        // Tint by reading level.
        let color = ctx.palette.sample(ctx.eased);
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            draw::tint_row(grid, cy_c, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 9. Segmented-ring — discrete ring segments lighting up one by one
// ────────────────────────────────────────────────────────────────────────────

struct SegmentedRing;
impl ProgressStyle for SegmentedRing {
    fn name(&self) -> &str {
        "segmented-ring"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Discrete LED-style segments arranged around a full ring: each is a \
         short arc of dots separated by a small gap; segments light up clockwise \
         from 12 o'clock one by one as eased rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        let n_segs: usize = 24;
        let gap_frac = 0.12_f32; // fraction of each slot that is a gap
        let lit = (ctx.eased.clamp(0.0, 1.0) * n_segs as f32).round() as usize;

        for seg in 0..n_segs {
            let seg_start = seg as f32 / n_segs as f32;
            let seg_end = (seg + 1) as f32 / n_segs as f32;
            let inner_start = seg_start + gap_frac / n_segs as f32;
            let inner_end = seg_end - gap_frac / n_segs as f32;

            // Angles: clockwise from top → subtract from π/2.
            let a0 = PI / 2.0 - inner_start * 2.0 * PI;
            let a1 = PI / 2.0 - inner_end * 2.0 * PI;

            let seg_steps = ((r as f32 * (a0 - a1).abs()).round() as usize).max(2);

            if seg < lit {
                // Lit: draw thick arc (inner + outer ring).
                let r_in = (r - 2).max(1);
                thick_arc(grid, cx, cy, r_in, r, a0, a1);
            } else {
                // Unlit: single-dot thin arc outline only.
                arc(grid, cx, cy, r, a0, a1, seg_steps);
            }
        }

        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = lit as f32 / n_segs as f32;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 10. Dual-gauge — two concentric arcs (outer: eased, inner: time-oscillating)
// ────────────────────────────────────────────────────────────────────────────

struct DualGauge;
impl ProgressStyle for DualGauge {
    fn name(&self) -> &str {
        "dual-gauge"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Two concentric arc gauges: the outer ring tracks eased progress while \
         the inner ring oscillates with time, showing a secondary live reading \
         (e.g. instantaneous vs. average) in the same braille dial"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r_outer) = dial_fit(dw, dh);

        // Inner radius leaves a gap for visual separation.
        let r_inner = ((r_outer as f32 * 0.55).round() as i32).max(2);

        // Both use the same angular convention: clockwise from top (12 o'clock).
        let a_top = PI / 2.0;

        // Outer ring: outer progress (eased), left-to-right track arc.
        let outer_sweep = ctx.eased.clamp(0.0, 1.0) * 2.0 * PI;
        let outer_track_steps = ((r_outer as f32 * 2.0 * PI).round() as usize).max(4);
        // Dim track.
        for i in (0..=outer_track_steps).step_by(3) {
            let t = i as f32 / outer_track_steps as f32;
            let angle = a_top - t * 2.0 * PI;
            let x = cx + (r_outer as f32 * angle.cos()).round() as i32;
            let y = cy - (r_outer as f32 * angle.sin()).round() as i32;
            draw::dot_i(grid, x, y);
        }
        // Bright fill.
        if outer_sweep > 0.001 {
            thick_arc(
                grid,
                cx,
                cy,
                r_outer - 1,
                r_outer,
                a_top,
                a_top - outer_sweep,
            );
        }

        // Inner ring: secondary oscillating reading.
        let secondary = 0.5 + 0.5 * (ctx.time * 1.3).sin(); // 0..1 pulsing
        let inner_sweep = secondary * 2.0 * PI;
        let inner_track_steps = ((r_inner as f32 * 2.0 * PI).round() as usize).max(4);
        // Dim track.
        for i in (0..=inner_track_steps).step_by(3) {
            let t = i as f32 / inner_track_steps as f32;
            let angle = a_top - t * 2.0 * PI;
            let x = cx + (r_inner as f32 * angle.cos()).round() as i32;
            let y = cy - (r_inner as f32 * angle.sin()).round() as i32;
            draw::dot_i(grid, x, y);
        }
        // Bright fill.
        if inner_sweep > 0.001 {
            thick_arc(
                grid,
                cx,
                cy,
                r_inner - 1,
                r_inner,
                a_top,
                a_top - inner_sweep,
            );
        }

        // Tint outer.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = ctx.eased;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 11. Clock-face — hour + minute hands set by progress
// ────────────────────────────────────────────────────────────────────────────

struct ClockFace;
impl ProgressStyle for ClockFace {
    fn name(&self) -> &str {
        "clock-face"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Analog clock face: progress maps the full 12-hour cycle — hour hand \
         completes one revolution at 100%, minute hand moves 12× faster, \
         giving a two-hand clock whose time equals eased × 12:00"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // Full circle rim.
        let rim_steps = ((r as f32 * 2.0 * PI).round() as usize).max(4);
        arc(grid, cx, cy, r, 0.0, 2.0 * PI, rim_steps);

        // 12 hour-tick marks.
        for h in 0..12 {
            let angle = PI / 2.0 - h as f32 * 2.0 * PI / 12.0;
            let tick_len = if h % 3 == 0 {
                (r / 4).max(1)
            } else {
                (r / 7).max(1)
            };
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Hour hand: one full revolution = eased 0→1.
        let hour_angle = PI / 2.0 - ctx.eased.clamp(0.0, 1.0) * 2.0 * PI;
        let hour_r = ((r as f32 * 0.6).round() as i32).max(1);
        let hx = cx + (hour_r as f32 * hour_angle.cos()).round() as i32;
        let hy = cy - (hour_r as f32 * hour_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, hx, hy);

        // Minute hand: 12× as fast → completes 12 revolutions at eased=1.
        let minute_angle = PI / 2.0 - ctx.eased.clamp(0.0, 1.0) * 12.0 * 2.0 * PI;
        let minute_r = ((r as f32 * 0.85).round() as i32).max(1);
        let mx = cx + (minute_r as f32 * minute_angle.cos()).round() as i32;
        let my = cy - (minute_r as f32 * minute_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, mx, my);

        // Centre dot.
        draw::dot_i(grid, cx, cy);

        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = cx_c as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 12. Pressure-dial — danger arc zone + damped settling needle
// ────────────────────────────────────────────────────────────────────────────

struct PressureDial;
impl ProgressStyle for PressureDial {
    fn name(&self) -> &str {
        "pressure-dial"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Pressure gauge with a hatched danger arc beyond 75% and a needle that \
         overdamps into place — slow creep at low pressure, urgent quiver at high; \
         danger zone fills with a dense dot pattern when eased exceeds the threshold"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // 240° dial (like speedometer but stops short on the left).
        // From 210° down to -30° clockwise.
        let a_start = 210_f32.to_radians();
        let span = -(240_f32.to_radians());

        // Track arc.
        let arc_steps = ((r as f32 * 2.0 * PI * (240.0 / 360.0)).round() as usize).max(4);
        arc(grid, cx, cy, r, a_start, a_start + span, arc_steps);

        // Tick marks every 20% (6 ticks).
        for i in 0..=5 {
            let frac = i as f32 / 5.0;
            let angle = a_start + frac * span;
            let tick_len = if i % 5 == 0 {
                (r / 3).max(1)
            } else {
                (r / 6).max(1)
            };
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Danger arc: 75%–100% of span — draw a thick hatched band.
        let danger_threshold = 0.75_f32;
        let a_danger0 = a_start + danger_threshold * span;
        let a_danger1 = a_start + span;
        let r_in_danger = (r - (r / 4).max(1)).max(1);
        let danger_steps = ((r as f32 * (a_danger0 - a_danger1).abs()).round() as usize).max(4);
        // Draw outer danger arc.
        arc(grid, cx, cy, r, a_danger0, a_danger1, danger_steps);
        // Inner danger arc.
        arc(
            grid,
            cx,
            cy,
            r_in_danger,
            a_danger0,
            a_danger1,
            danger_steps,
        );
        // Radial hatch lines inside the danger zone.
        let n_hatch = 4usize.max((r / 5) as usize);
        for h in 0..=n_hatch {
            let frac = h as f32 / n_hatch as f32;
            let angle = a_danger0 + frac * (a_danger1 - a_danger0);
            let x0 = cx + (r_in_danger as f32 * angle.cos()).round() as i32;
            let y0 = cy - (r_in_danger as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // If in danger zone, fill with dense dots (pressure really is high).
        if ctx.eased > danger_threshold {
            let extra_frac = (ctx.eased - danger_threshold) / (1.0 - danger_threshold);
            let a_fill_end = a_danger0 + extra_frac * (a_danger1 - a_danger0);
            let fill_steps =
                ((r as f32 * (a_danger0 - a_fill_end).abs() * 2.0).round() as usize).max(2);
            thick_arc(grid, cx, cy, r_in_danger, r, a_danger0, a_fill_end);
            let _ = fill_steps;
        }

        // Needle with overdamped settle (no oscillation, just slow exponential
        // approach — this contrasts with the tachometer's underdamped wobble).
        // Model: effective = eased − (eased − prev_display) · exp(-3·t)
        // Since we're stateless, simulate with: effective = eased·(1−exp(-3t)) + 0·exp(-3t).
        // At t=0 needle starts at 0 and exponentially reaches eased.
        let damp = (-2.0 * ctx.time).exp();
        let effective = ctx.eased * (1.0 - damp);
        let needle_angle = a_start + effective.clamp(0.0, 1.0) * span;
        let nx = cx + (r as f32 * needle_angle.cos()).round() as i32;
        let ny = cy - (r as f32 * needle_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, nx, ny);

        // At high pressure, add quiver: small fast oscillation.
        if ctx.eased > 0.6 {
            let quiver_amp = (ctx.eased - 0.6) / 0.4 * 0.03;
            let quiver = quiver_amp * (20.0 * ctx.time).sin();
            let q_angle = needle_angle + quiver;
            let qx = cx + (r as f32 * q_angle.cos()).round() as i32;
            let qy = cy - (r as f32 * q_angle.sin()).round() as i32;
            bresenham(grid, cx, cy, qx, qy);
        }

        // Tint: green at low pressure, through orange, to red at danger.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = ctx.eased;
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 13. Altimeter — aviation two-needle dial (fast 10× needle + slow needle)
// ────────────────────────────────────────────────────────────────────────────

struct Altimeter;
impl ProgressStyle for Altimeter {
    fn name(&self) -> &str {
        "altimeter"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Aviation altimeter: ten rim ticks like the 0–9 altitude scale, a long \
         needle spinning 10 revolutions over the full range (hundreds) and a \
         short needle making a single revolution (thousands)"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // Bezel.
        let rim_steps = ((r as f32 * 2.0 * PI).round() as usize).max(4);
        arc(grid, cx, cy, r, 0.0, 2.0 * PI, rim_steps);

        // Ten numeral ticks (0–9), clockwise from 12 o'clock.
        for i in 0..10 {
            let angle = PI / 2.0 - i as f32 * 2.0 * PI / 10.0;
            let tick_len = (r / 5).max(1);
            let x0 = cx + ((r - tick_len) as f32 * angle.cos()).round() as i32;
            let y0 = cy - ((r - tick_len) as f32 * angle.sin()).round() as i32;
            let x1 = cx + (r as f32 * angle.cos()).round() as i32;
            let y1 = cy - (r as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        let eased = ctx.eased.clamp(0.0, 1.0);

        // Long needle: 10 revolutions over the range (hundreds of feet).
        let fast_angle = PI / 2.0 - eased * 10.0 * 2.0 * PI;
        let fast_r = (r - 1).max(1);
        let fx = cx + (fast_r as f32 * fast_angle.cos()).round() as i32;
        let fy = cy - (fast_r as f32 * fast_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, fx, fy);

        // Short needle: one revolution (thousands), with a wide arrow head.
        let slow_angle = PI / 2.0 - eased * 2.0 * PI;
        let slow_r = ((r as f32 * 0.55).round() as i32).max(1);
        let sx = cx + (slow_r as f32 * slow_angle.cos()).round() as i32;
        let sy = cy - (slow_r as f32 * slow_angle.sin()).round() as i32;
        bresenham(grid, cx, cy, sx, sy);
        // Arrow head: two short barbs angled back from the tip.
        for barb in [-0.5_f32, 0.5_f32] {
            let ba = slow_angle + PI + barb;
            let bx = sx + (2.0 * ba.cos()).round() as i32;
            let by = sy - (2.0 * ba.sin()).round() as i32;
            bresenham(grid, sx, sy, bx, by);
        }

        draw::dot_i(grid, cx, cy);

        // Tint sweeps with the fast needle so the dial visibly churns.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = ((eased * 10.0).fract() + cx_c as f32 / cells_w.max(1) as f32 * 0.3)
                    .clamp(0.0, 1.0);
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 14. Radar-scope — range rings + rotating sweep, blips appear with progress
// ────────────────────────────────────────────────────────────────────────────

struct RadarScope;
impl ProgressStyle for RadarScope {
    fn name(&self) -> &str {
        "radar-scope"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Radar scope: concentric range rings with a sweep beam rotating on \
         ctx.time; contact blips at pseudo-random bearings appear one by one \
         as eased rises — at 100% the screen is full of traffic"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // Range rings: outer rim plus one or two inner rings (sparse dots).
        let rim_steps = ((r as f32 * 2.0 * PI).round() as usize).max(4);
        arc(grid, cx, cy, r, 0.0, 2.0 * PI, rim_steps);
        for ring in 1..3 {
            let rr = (r * ring / 3).max(1);
            let steps = ((rr as f32 * 2.0 * PI).round() as usize).max(4);
            for i in (0..=steps).step_by(3) {
                let a = i as f32 / steps as f32 * 2.0 * PI;
                let x = cx + (rr as f32 * a.cos()).round() as i32;
                let y = cy - (rr as f32 * a.sin()).round() as i32;
                draw::dot_i(grid, x, y);
            }
        }

        // Sweep beam: rotates clockwise on time, with a short trailing fade
        // (two extra beams at decreasing radius behind the leader).
        let sweep = PI / 2.0 - ctx.time * 1.8;
        for (lag, len_frac) in [(0.0_f32, 1.0_f32), (0.12, 0.8), (0.24, 0.55)] {
            let a = sweep + lag; // trailing = counter-clockwise behind
            let br = ((r as f32 * len_frac).round() as i32).max(1);
            let bx = cx + (br as f32 * a.cos()).round() as i32;
            let by = cy - (br as f32 * a.sin()).round() as i32;
            bresenham(grid, cx, cy, bx, by);
        }

        // Contact blips: fixed pseudo-random bearings/ranges (pure hash of the
        // blip index — stateless and frame-stable). Revealed count = eased × N.
        let n_blips = 9usize;
        let shown = (ctx.eased.clamp(0.0, 1.0) * n_blips as f32).round() as usize;
        for i in 0..shown.min(n_blips) {
            let h = (i as u32).wrapping_mul(2654435761).wrapping_add(40503);
            let bearing = (h % 360) as f32 / 360.0 * 2.0 * PI;
            let range = 0.3 + ((h >> 9) % 64) as f32 / 64.0 * 0.6;
            let br = (r as f32 * range).round() as i32;
            let bx = cx + (br as f32 * bearing.cos()).round() as i32;
            let by = cy - (br as f32 * bearing.sin()).round() as i32;
            // 2×2 dot blip so contacts read at small radii.
            for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                draw::dot_i(grid, bx + ox, by + oy);
            }
        }

        // Phosphor tint: brightest at the sweep, fading around the dial.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = (ctx.eased * 0.7
                    + 0.3 * ((ctx.time * 1.8 + cx_c as f32 * 0.2).sin() * 0.5 + 0.5))
                    .clamp(0.0, 1.0);
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 15. Attitude-horizon — artificial horizon: pitch from eased, roll from time
// ────────────────────────────────────────────────────────────────────────────

struct AttitudeHorizon;
impl ProgressStyle for AttitudeHorizon {
    fn name(&self) -> &str {
        "attitude-horizon"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Artificial horizon (attitude indicator): the horizon line climbs the \
         bezel as eased rises — ground hatching shrinks away below — while a \
         gentle time-driven roll banks the whole horizon; fixed wings mark centre"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);

        // Bezel.
        let rim_steps = ((r as f32 * 2.0 * PI).round() as usize).max(4);
        arc(grid, cx, cy, r, 0.0, 2.0 * PI, rim_steps);

        // Horizon: vertical offset from pitch (eased), slope from roll (time).
        // eased = 0 → horizon near the top (mostly ground: climb ahead);
        // eased = 1 → horizon near the bottom (all sky: cruising altitude).
        let pitch = (ctx.eased.clamp(0.0, 1.0) - 0.5) * 2.0; // -1..1
        let h_off = (pitch * r as f32 * 0.7).round() as i32;
        let roll = 0.35 * (ctx.time * 0.9).sin();
        let slope = roll.tan();

        for dx in -r..=r {
            let fy = h_off as f32 + dx as f32 * slope;
            let y = cy + fy.round() as i32;
            let x = cx + dx;
            // Clip to the bezel circle.
            if dx * dx + (y - cy) * (y - cy) <= r * r {
                draw::dot_i(grid, x, y);
                // Ground hatching: sparse dots below the horizon line.
                let mut gy = y + 2;
                while (gy - cy) * (gy - cy) + dx * dx <= r * r {
                    if (dx + gy) % 3 == 0 {
                        draw::dot_i(grid, x, gy);
                    }
                    gy += 2;
                }
            }
        }

        // Fixed aircraft symbol: stub wings + centre dot (never moves).
        let wing = (r / 3).max(2);
        bresenham(grid, cx - wing, cy, cx - 1, cy);
        bresenham(grid, cx + 1, cy, cx + wing, cy);
        draw::dot_i(grid, cx, cy);

        // Tint: sky colour above, ground colour below, split at the horizon row.
        let (cells_w, cells_h) = grid.dimensions();
        let horizon_cell = (((cy + h_off).max(0) as usize) / 4).min(cells_h.saturating_sub(1));
        for cy_c in 0..cells_h {
            let t = if cy_c <= horizon_cell { 0.85 } else { 0.15 };
            draw::tint_row(
                grid,
                cy_c,
                0,
                cells_w.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 16. Bubble-level — full-width spirit level, bubble drifts to centre
// ────────────────────────────────────────────────────────────────────────────

struct BubbleLevel;
impl ProgressStyle for BubbleLevel {
    fn name(&self) -> &str {
        "bubble-level"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Spirit level spanning the full width: a gently curved vial with centre \
         marks; the bubble starts at the far end and drifts toward dead centre \
         as eased approaches 1 — level achieved at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let cy = (dh / 2) as i32;
        let margin = 2i32;
        let left = margin;
        let right = (dw as i32 - 1 - margin).max(left + 4);
        let span = (right - left) as f32;
        let cx = (left + right) / 2;

        // Vial: two horizontal walls bowed upward (real vials curve so the
        // bubble seeks the high point — the middle).
        let bow = ((dh as f32) * 0.18).max(1.0);
        let half_gap = ((dh as f32) * 0.16).max(1.0) as i32;
        for x in left..=right {
            let u = (x - left) as f32 / span * 2.0 - 1.0; // -1..1
            let lift = (bow * (1.0 - u * u)).round() as i32;
            draw::dot_i(grid, x, cy - half_gap - lift);
            draw::dot_i(grid, x, cy + half_gap - lift);
        }
        // End caps.
        for x in [left, right] {
            let u = (x - left) as f32 / span * 2.0 - 1.0;
            let lift = (bow * (1.0 - u * u)).round() as i32;
            bresenham(grid, x, cy - half_gap - lift, x, cy + half_gap - lift);
        }

        // Centre marks: the "level" gate the bubble must land between.
        let gate = (span * 0.06).max(2.0) as i32;
        for gx in [cx - gate, cx + gate] {
            bresenham(
                grid,
                gx,
                cy - half_gap - bow as i32 - 1,
                gx,
                cy + half_gap - bow as i32 + 1,
            );
        }

        // Bubble: travels from the left end to centre as eased → 1, with a
        // small time wobble that dies out as it homes in.
        let eased = ctx.eased.clamp(0.0, 1.0);
        let wobble = (1.0 - eased) * 0.05 * (ctx.time * 5.0).sin();
        let pos = (eased * 0.5 + wobble).clamp(0.0, 0.5); // 0 = left end, 0.5 = centre
        let bx = left + (pos * 2.0 * (cx - left) as f32).round() as i32;
        let u = (bx - left) as f32 / span * 2.0 - 1.0;
        let lift = (bow * (1.0 - u * u)).round() as i32;
        let bw = gate.max(2);
        // Filled oval bubble.
        for ox in -bw..=bw {
            let h = ((1.0 - (ox as f32 / bw as f32).powi(2)).max(0.0).sqrt()
                * (half_gap as f32 - 1.0).max(1.0))
            .round() as i32;
            bresenham(grid, bx + ox, cy - lift - h, bx + ox, cy - lift + h);
        }

        // Tint: cool along the vial, hot at the bubble.
        let (cells_w, cells_h) = grid.dimensions();
        let bubble_cell = (bx.max(0) as usize / 2).min(cells_w.saturating_sub(1));
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let d = (cx_c as f32 - bubble_cell as f32).abs() / cells_w.max(1) as f32;
                let t = (1.0 - d * 3.0).clamp(0.0, 1.0);
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 17. Edgewise-meter — full-width panel meter: horizontal scale + pointer
// ────────────────────────────────────────────────────────────────────────────

struct EdgewiseMeter;
impl ProgressStyle for EdgewiseMeter {
    fn name(&self) -> &str {
        "edgewise-meter"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Edgewise panel meter spanning the full width: a horizontal scale with \
         ticks every 10% and a hatched red zone past 85%; a slanted needle pivots \
         from below the panel to the eased position, like rack-mount audio gear"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let margin = 2i32;
        let left = margin;
        let right = (dw as i32 - 1 - margin).max(left + 4);
        let span = (right - left) as f32;
        let scale_y = (dh as f32 * 0.25).round() as i32;
        // Pivot sits half a grid-height below the bottom edge so the needle slants.
        let pivot = ((left + right) / 2, dh as i32 + dh as i32 / 2);

        // Scale line.
        bresenham(grid, left, scale_y, right, scale_y);

        // Ticks every 10%, taller every 50%.
        for i in 0..=10 {
            let x = left + (i as f32 / 10.0 * span).round() as i32;
            let len = if i % 5 == 0 { 4 } else { 2 };
            bresenham(grid, x, scale_y - len, x, scale_y);
        }

        // Red zone: hatch above the scale from 85% to 100%.
        let rz = left + (0.85 * span).round() as i32;
        let mut x = rz;
        while x <= right {
            bresenham(grid, x, scale_y - 3, x.min(right), scale_y - 1);
            x += 3;
        }

        // Needle: pivots from far below the panel so it slants realistically;
        // a touch of VU-style bounce keeps it alive.
        let bounce = 0.03 * (ctx.time * 4.0).sin();
        let level = (ctx.eased + bounce).clamp(0.0, 1.0);
        let tip_x = left + (level * span).round() as i32;
        let (px, py) = (pivot.0, pivot.1);
        // Clip the needle to the visible grid: walk from the tip toward the
        // pivot but stop at the bottom edge.
        let t_bottom = ((dh as i32 - 1 - scale_y) as f32 / (py - scale_y) as f32).clamp(0.0, 1.0);
        let end_x = tip_x as f32 + (px - tip_x) as f32 * t_bottom;
        let end_y = scale_y as f32 + (py - scale_y) as f32 * t_bottom;
        bresenham(
            grid,
            tip_x,
            scale_y,
            end_x.round() as i32,
            end_y.round() as i32,
        );
        // Pointer head.
        for (ox, oy) in [(0, -1), (-1, 0), (1, 0)] {
            draw::dot_i(grid, tip_x + ox, scale_y + oy);
        }

        // Tint: gradient sweeps along the scale up to the needle, dim past it.
        let (cells_w, cells_h) = grid.dimensions();
        let lit_cells = ((level * cells_w as f32).round() as usize).min(cells_w);
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = if cx_c <= lit_cells {
                    cx_c as f32 / cells_w.max(1) as f32
                } else {
                    0.05
                };
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 18. Dial-bank — three mini-dials filling in sequence across the width
// ────────────────────────────────────────────────────────────────────────────

struct DialBank;
impl ProgressStyle for DialBank {
    fn name(&self) -> &str {
        "dial-bank"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "A bank of three mini speedometers side by side: progress cascades — \
         the first dial sweeps 0–33%, the second 33–66%, the third 66–100% — \
         like a rack of instruments coming alive one by one"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let n = 3usize;
        let eased = ctx.eased.clamp(0.0, 1.0);

        let cy = (dh / 2) as i32;
        let r = ((dh as i32 / 2) - 1)
            .max(1)
            .min(dw as i32 / (2 * n as i32) - 1)
            .max(1);

        let a_start = 225_f32.to_radians();
        let span = -(270_f32.to_radians());

        for d in 0..n {
            let cx = (dw as f32 * (2 * d + 1) as f32 / (2 * n) as f32).round() as i32;
            // This dial's share of progress: cascades left to right.
            let local = (eased * n as f32 - d as f32).clamp(0.0, 1.0);

            // Arc track.
            let steps = ((r as f32 * 2.0 * PI * 0.75).round() as usize).max(4);
            arc(grid, cx, cy, r, a_start, a_start + span, steps);

            // 5 ticks per dial.
            for i in 0..=4 {
                let frac = i as f32 / 4.0;
                let angle = a_start + frac * span;
                let tick = (r / 4).max(1);
                let x0 = cx + ((r - tick) as f32 * angle.cos()).round() as i32;
                let y0 = cy - ((r - tick) as f32 * angle.sin()).round() as i32;
                let x1 = cx + (r as f32 * angle.cos()).round() as i32;
                let y1 = cy - (r as f32 * angle.sin()).round() as i32;
                bresenham(grid, x0, y0, x1, y1);
            }

            // Needle; idle dials rest at zero with a faint jitter when "next up".
            let next_up = local <= 0.0 && (eased * n as f32) > d as f32 - 0.15;
            let jitter = if next_up {
                0.02 * (ctx.time * 9.0 + d as f32).sin().max(0.0)
            } else {
                0.0
            };
            let needle = a_start + (local + jitter).clamp(0.0, 1.0) * span;
            let nx = cx + (r as f32 * needle.cos()).round() as i32;
            let ny = cy - (r as f32 * needle.sin()).round() as i32;
            bresenham(grid, cx, cy, nx, ny);
        }

        // Tint each dial's column band by its own fill level.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let d = (cx_c * n / cells_w.max(1)).min(n - 1);
                let local = (eased * n as f32 - d as f32).clamp(0.0, 1.0);
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(local));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 19. Galvanometer — top-pivot hanging needle over a bottom arc scale
// ────────────────────────────────────────────────────────────────────────────

struct Galvanometer;
impl ProgressStyle for Galvanometer {
    fn name(&self) -> &str {
        "galvanometer"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Laboratory galvanometer: the needle hangs from a pivot at the TOP of \
         the instrument and swings across an arc scale at the bottom — the \
         inverted geometry of a mirror galvanometer, with a slow critically-damped \
         settle toward the eased deflection"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let eased = ctx.eased.clamp(0.0, 1.0);

        // Pivot at top centre; scale arc near the bottom.
        let px = (dw / 2) as i32;
        let py = 1i32;
        let r = (dh as i32 - 3).max(2).min(dw as i32 / 2 - 1).max(2);

        // Hanging needle sweeps ±55° around straight-down (-π/2 in math angle).
        let max_defl = 55_f32.to_radians();
        let a_left = -PI / 2.0 - max_defl;
        let a_right = -PI / 2.0 + max_defl;

        // Scale arc (centred on the pivot, so the needle tip rides along it).
        let steps = ((r as f32 * 2.0 * max_defl).round() as usize).max(4);
        arc(grid, px, py, r, a_left, a_right, steps);

        // Scale ticks every 12.5% — longer at 0, 50, 100%.
        for i in 0..=8 {
            let frac = i as f32 / 8.0;
            let angle = a_left + frac * (a_right - a_left);
            let tick = if i % 4 == 0 { 3 } else { 1 };
            let x0 = px + ((r - tick) as f32 * angle.cos()).round() as i32;
            let y0 = py - ((r - tick) as f32 * angle.sin()).round() as i32;
            let x1 = px + ((r + 1) as f32 * angle.cos()).round() as i32;
            let y1 = py - ((r + 1) as f32 * angle.sin()).round() as i32;
            bresenham(grid, x0, y0, x1, y1);
        }

        // Critically-damped settle: approaches eased without overshoot,
        // plus a faint residual tremor that fades with time.
        let settle = 1.0 - (-2.5 * ctx.time).exp() * (1.0 + 2.5 * ctx.time);
        let tremor = 0.015 * (ctx.time * 11.0).sin() * (-0.8 * ctx.time).exp();
        let defl = (eased * settle + tremor).clamp(0.0, 1.0);
        let a = a_left + defl * (a_right - a_left);
        let nx = px + (r as f32 * a.cos()).round() as i32;
        let ny = py - (r as f32 * a.sin()).round() as i32;
        bresenham(grid, px, py, nx, ny);

        // Pivot mount: small bracket at the top.
        bresenham(grid, px - 2, py, px + 2, py);
        draw::dot_i(grid, px, py + 1);

        // Mirror-galvo light spot: a bright 2×2 blob where the needle meets
        // the scale, like the reflected beam dot on a lab bench.
        for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            draw::dot_i(grid, nx + ox, ny + oy);
        }

        // Tint: deflection colour at the spot, cool elsewhere.
        let (cells_w, cells_h) = grid.dimensions();
        let spot_cell = (nx.max(0) as usize / 2).min(cells_w.saturating_sub(1));
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let d = (cx_c as f32 - spot_cell as f32).abs() / cells_w.max(1) as f32;
                let t = (eased * (1.0 - d * 2.0)).clamp(0.05, 1.0);
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 20. Anemometer — spinning wind-cup rotor, speed tracks eased
// ────────────────────────────────────────────────────────────────────────────

struct Anemometer;
impl ProgressStyle for Anemometer {
    fn name(&self) -> &str {
        "anemometer"
    }
    fn theme(&self) -> &str {
        "meter"
    }
    fn describe(&self) -> &str {
        "Anemometer: a three-cup wind rotor whose spin rate tracks eased — \
         barely turning at 0%, a blur at 100% — ringed by a dotted bearing \
         race; the cups are small circles at the arm tips"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy, r) = dial_fit(dw, dh);
        let eased = ctx.eased.clamp(0.0, 1.0);

        // Bearing race: sparse dotted outer ring.
        let steps = ((r as f32 * 2.0 * PI).round() as usize).max(4);
        for i in (0..=steps).step_by(3) {
            let a = i as f32 / steps as f32 * 2.0 * PI;
            let x = cx + (r as f32 * a.cos()).round() as i32;
            let y = cy - (r as f32 * a.sin()).round() as i32;
            draw::dot_i(grid, x, y);
        }

        // Rotor: angle integrates a speed proportional to eased.
        let speed = 0.4 + eased * 9.0;
        let base = ctx.time * speed;
        let arm_r = (r - 2).max(1);
        let cup_r = (r / 4).max(1);

        for k in 0..3 {
            let a = base + k as f32 * 2.0 * PI / 3.0;
            let ax = cx + (arm_r as f32 * a.cos()).round() as i32;
            let ay = cy - (arm_r as f32 * a.sin()).round() as i32;
            bresenham(grid, cx, cy, ax, ay);
            // Cup: small circle at the arm tip.
            let cup_steps = ((cup_r as f32 * 2.0 * PI).round() as usize).max(4);
            arc(grid, ax, ay, cup_r, 0.0, 2.0 * PI, cup_steps);
        }

        // Hub.
        draw::dot_i(grid, cx, cy);

        // Tint: hue rotates with the rotor so the spin reads in colour too.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            for cx_c in 0..cells_w {
                let t = ((base * 0.15 + cx_c as f32 / cells_w.max(1) as f32).fract()).abs();
                draw::tint_row(grid, cy_c, cx_c, cx_c, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}
