//! Animated border / frame progress styles.
//!
//! Every style in this theme draws progress as motion **around the perimeter**
//! of the grid region rather than as a horizontal fill. The twelve styles cover
//! fundamentally different visual strategies: sequential draw-on, oscillating
//! dashes, growing corner brackets, a chasing comet, nested frames, pulsing
//! glow, distinct dash patterns, chamfered corners, inward thickening, a ruler
//! with a marker, a window title strip, and two opposing runners.
//!
//! # Perimeter helpers
//!
//! All styles share a common concept: the perimeter is a 1-D sequence of dot
//! positions walked clockwise starting from the top-left corner. The helper
//! `perim_point(i, pw, ph)` converts a perimeter index to `(x, y)` in dot
//! space. `perim_len(pw, ph)` gives the total count.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ── Perimeter helpers ────────────────────────────────────────────────────────

/// Total perimeter length in dots for a rectangle of dot-size `(pw, ph)`.
/// Minimum 1 so we never divide by zero.
#[inline]
fn perim_len(pw: usize, ph: usize) -> usize {
    if pw == 0 || ph == 0 {
        return 1;
    }
    if pw == 1 && ph == 1 {
        return 1;
    }
    if pw == 1 {
        return ph;
    }
    if ph == 1 {
        return pw;
    }
    2 * (pw - 1) + 2 * (ph - 1)
}

/// Map a perimeter index `i` (0-based, clockwise from top-left) to `(x, y)`
/// in dot space. Wraps modulo the perimeter length.
#[inline]
fn perim_point(i: usize, pw: usize, ph: usize) -> (usize, usize) {
    if pw == 0 || ph == 0 {
        return (0, 0);
    }
    if pw == 1 && ph == 1 {
        return (0, 0);
    }
    if pw == 1 {
        let i = i % ph;
        return (0, i);
    }
    if ph == 1 {
        let i = i % pw;
        return (i, 0);
    }
    let p = perim_len(pw, ph);
    let i = i % p;
    // top edge: left→right
    if i < pw {
        return (i, 0);
    }
    let i = i - (pw - 1);
    // right edge: top→bottom (skip top-right corner already counted)
    if i < ph {
        return (pw - 1, i);
    }
    let i = i - (ph - 1);
    // bottom edge: right→left
    if i < pw {
        return (pw - 1 - i, ph - 1);
    }
    let i = i - (pw - 1);
    // left edge: bottom→top (skip corners already counted)
    let i = i % (ph - 1).max(1); // guard against degenerate ph
    (0, ph - 1 - i)
}

// ── Public registry ──────────────────────────────────────────────────────────

/// All styles in the `border` theme.
///
/// Returns 12 structurally distinct styles, each communicating progress or
/// time through perimeter motion, geometry, or density — never only colour.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(DrawOn),
        Box::new(MarchingAnts),
        Box::new(CornerBrackets),
        Box::new(Runner),
        Box::new(DoubleFrame),
        Box::new(NeonPulse),
        Box::new(DashedBorder),
        Box::new(DottedBorder),
        Box::new(RoundedBorder),
        Box::new(FillFrame),
        Box::new(TickedFrame),
        Box::new(TwoRunners),
    ]
}

// ── Style 1: DrawOn ──────────────────────────────────────────────────────────

/// The border draws itself clockwise from the top-left; `eased` sets how far
/// around the perimeter the stroke has reached. At 1.0 the frame is complete.
struct DrawOn;
impl ProgressStyle for DrawOn {
    fn name(&self) -> &str {
        "draw-on"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Border draws clockwise from top-left; perimeter = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        let p = perim_len(pw, ph);
        let lit = (ctx.eased * p as f32).round() as usize;
        for i in 0..lit.min(p) {
            let (x, y) = perim_point(i, pw, ph);
            draw::dot(grid, x, y);
        }
        // Tint the lit portion with the palette gradient.
        let (cw, ch) = grid.dimensions();
        if lit > 0 {
            for cy in 0..ch {
                let t = cy as f32 / ch.max(1) as f32;
                draw::tint_row(grid, cy, 0, cw.saturating_sub(1), ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Style 2: MarchingAnts ────────────────────────────────────────────────────

/// A dashed border whose dash pattern scrolls around the perimeter over time.
/// Dash length and gap are fixed; `time` drives the scroll offset.
struct MarchingAnts;
impl ProgressStyle for MarchingAnts {
    fn name(&self) -> &str {
        "marching-ants"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Dashes scroll clockwise around the frame; speed increases with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        let p = perim_len(pw, ph);
        let dash = 6usize;
        let gap = 4usize;
        let period = (dash + gap) as f32;
        // Speed scales up with progress so faster near completion.
        let speed = 8.0 + ctx.progress * 24.0;
        let offset = (ctx.time * speed) as usize;
        for i in 0..p {
            let phase = (i + offset) % (dash + gap);
            if phase < dash {
                let (x, y) = perim_point(i, pw, ph);
                draw::dot(grid, x, y);
            }
        }
        // Tint the whole border in start colour.
        let (cw, ch) = grid.dimensions();
        let c = ctx.palette.sample(0.3);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), c);
        }
        let _ = period; // suppress lint
        Ok(())
    }
}

// ── Style 3: CornerBrackets ──────────────────────────────────────────────────

/// Four L-shaped corner brackets grow inward along each edge proportionally
/// to `eased`. At 1.0 they meet in the middle of every edge.
struct CornerBrackets;
impl ProgressStyle for CornerBrackets {
    fn name(&self) -> &str {
        "corner-brackets"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Corner brackets extend toward midpoints of each edge as progress fills"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        if pw == 0 || ph == 0 {
            return Ok(());
        }
        let hreach = ((ctx.eased * (pw / 2) as f32).round() as usize).min(pw / 2);
        let vreach = ((ctx.eased * (ph / 2) as f32).round() as usize).min(ph / 2);
        let x1 = pw.saturating_sub(1);
        let y1 = ph.saturating_sub(1);
        // Top-left
        draw::hline(grid, 0, hreach.min(x1), 0);
        draw::vline(grid, 0, 0, vreach.min(y1));
        // Top-right
        draw::hline(grid, x1.saturating_sub(hreach), x1, 0);
        draw::vline(grid, x1, 0, vreach.min(y1));
        // Bottom-left
        draw::hline(grid, 0, hreach.min(x1), y1);
        draw::vline(grid, 0, y1.saturating_sub(vreach), y1);
        // Bottom-right
        draw::hline(grid, x1.saturating_sub(hreach), x1, y1);
        draw::vline(grid, x1, y1.saturating_sub(vreach), y1);
        let (cw, ch) = grid.dimensions();
        let c = ctx.palette.sample(ctx.eased);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), c);
        }
        Ok(())
    }
}

// ── Style 4: Runner ──────────────────────────────────────────────────────────

/// A single bright comet chases the perimeter with `time`; the tail fades
/// using dot density. A secondary dim outline shows the full frame.
struct Runner;
impl ProgressStyle for Runner {
    fn name(&self) -> &str {
        "runner"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Bright comet races around the full frame perimeter with a fading tail"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        let p = perim_len(pw, ph);
        // Dim outline — every other dot so it reads as a ghost frame.
        for i in (0..p).step_by(2) {
            let (x, y) = perim_point(i, pw, ph);
            draw::dot(grid, x, y);
        }
        // Comet head position driven by time.
        let speed = 0.5; // laps per second
        let head = ((ctx.time * speed).fract() * p as f32) as usize;
        let tail = (p / 6).max(3);
        for k in 0..tail {
            let idx = (head + p - k) % p;
            let (x, y) = perim_point(idx, pw, ph);
            // Tail thins: draw every dot near head, every other farther out.
            if k == 0 || k % (1 + k / (tail / 3).max(1)) == 0 {
                draw::dot(grid, x, y);
            }
        }
        // Tint comet with palette end colour.
        let (cw, ch) = grid.dimensions();
        let c = ctx.palette.sample(1.0);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), c);
        }
        Ok(())
    }
}

// ── Style 5: DoubleFrame ─────────────────────────────────────────────────────

/// Two nested rectangle outlines. The outer is always drawn; the inner
/// animates with `eased` — it begins as a tiny dot cluster and expands
/// outward until it sits 2 dots inside the outer frame.
struct DoubleFrame;
impl ProgressStyle for DoubleFrame {
    fn name(&self) -> &str {
        "double-frame"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Outer frame always present; inner frame expands from centre with eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        // Outer frame.
        draw::rect_outline(grid, 0, 0, pw, ph);
        // Inner frame: starts at centre, grows to 2 dots inside outer.
        let max_inset = 2usize;
        let inset = max_inset
            + (((1.0 - ctx.eased) * (pw.min(ph) / 2).saturating_sub(max_inset) as f32).round()
                as usize);
        let inset = inset.min(pw / 2).min(ph / 2);
        let ix0 = inset;
        let iy0 = inset;
        let iw = pw.saturating_sub(inset * 2);
        let ih = ph.saturating_sub(inset * 2);
        if iw >= 2 && ih >= 2 {
            draw::rect_outline(grid, ix0, iy0, iw, ih);
        }
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }
        Ok(())
    }
}

// ── Style 6: NeonPulse ───────────────────────────────────────────────────────

/// The frame pulses in apparent brightness by varying the dot density of a
/// two-pixel-wide border band. More progress → base brightness; time drives
/// the oscillation on top.
struct NeonPulse;
impl ProgressStyle for NeonPulse {
    fn name(&self) -> &str {
        "neon-pulse"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Frame border pulses in dot-density (glow simulation); brighter with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        // Pulse: base brightness from progress, oscillation from time.
        let osc = (ctx.time * 2.5 * PI).sin() * 0.2;
        let density = (ctx.eased * 0.7 + 0.3 + osc).clamp(0.0, 1.0);
        let p = perim_len(pw, ph);
        // Outer ring.
        for i in 0..p {
            let (x, y) = perim_point(i, pw, ph);
            // Probabilistic density: use i modulo to approximate fraction.
            let period = (1.0 / density.max(0.01)).round() as usize;
            let period = period.max(1);
            if i % period == 0 {
                draw::dot(grid, x, y);
            }
        }
        // Second ring (1 dot inset) with lower density for glow halo.
        let inner_density = (density * 0.55).clamp(0.0, 1.0);
        if pw >= 4 && ph >= 4 {
            let ip = perim_len(pw - 2, ph - 2);
            let period2 = ((1.0 / inner_density.max(0.01)).round() as usize).max(1);
            for i in 0..ip {
                if i % period2 == 0 {
                    let (x, y) = perim_point(i, pw - 2, ph - 2);
                    draw::dot(grid, x + 1, y + 1);
                }
            }
        }
        let (cw, ch) = grid.dimensions();
        let c = ctx.palette.sample(ctx.eased);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), c);
        }
        Ok(())
    }
}

// ── Style 7: DashedBorder ────────────────────────────────────────────────────

/// A static dashed frame (long dash, long gap – 3:2 ratio). `eased` controls
/// how many of the dashes are lit, walking clockwise — structurally a
/// draw-on of a dash pattern rather than a solid stroke.
struct DashedBorder;
impl ProgressStyle for DashedBorder {
    fn name(&self) -> &str {
        "dashed"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Dashed frame border; lit dashes accumulate clockwise with eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        let p = perim_len(pw, ph);
        let dash = 5usize;
        let gap = 3usize;
        let period = dash + gap;
        let lit_perimeter = (ctx.eased * p as f32).round() as usize;
        for i in 0..lit_perimeter.min(p) {
            if i % period < dash {
                let (x, y) = perim_point(i, pw, ph);
                draw::dot(grid, x, y);
            }
        }
        let (cw, ch) = grid.dimensions();
        let c = ctx.palette.sample(ctx.eased);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), c);
        }
        Ok(())
    }
}

// ── Style 8: DottedBorder ────────────────────────────────────────────────────

/// A dotted (1:2 ratio – single dot, double gap) frame. Progress fills
/// clockwise. Visually sparser and more delicate than `dashed`.
struct DottedBorder;
impl ProgressStyle for DottedBorder {
    fn name(&self) -> &str {
        "dotted"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Sparse dotted frame (1:2 on/off); lit dots accumulate with eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        let p = perim_len(pw, ph);
        let period = 3usize; // 1 dot, 2 gap
        let lit_perimeter = (ctx.eased * p as f32).round() as usize;
        for i in 0..lit_perimeter.min(p) {
            if i % period == 0 {
                let (x, y) = perim_point(i, pw, ph);
                draw::dot(grid, x, y);
            }
        }
        let (cw, ch) = grid.dimensions();
        let c = ctx.palette.sample(0.6);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), c);
        }
        Ok(())
    }
}

// ── Style 9: RoundedBorder ───────────────────────────────────────────────────

/// A frame with arc-approximated rounded corners: the four corner regions use
/// diagonal dot clusters to soften the right angles. The straight edges fill
/// with `eased` clockwise.
struct RoundedBorder;
impl ProgressStyle for RoundedBorder {
    fn name(&self) -> &str {
        "rounded"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Chamfered/rounded corners via arc dots; straight edges fill with eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        if pw == 0 || ph == 0 {
            return Ok(());
        }
        let x1 = pw.saturating_sub(1);
        let y1 = ph.saturating_sub(1);
        // Corner arc radius in dots (capped so it doesn't consume the whole edge).
        let r = 3usize.min(pw / 4).min(ph / 4).max(1);
        // ── Straight edges (filled with eased) ──────────────────────────────
        // Top & bottom edges between corners.
        let top_x0 = r;
        let top_x1 = x1.saturating_sub(r);
        let bot_x0 = r;
        let bot_x1 = x1.saturating_sub(r);
        // Edge lengths.
        let h_len = top_x1.saturating_sub(top_x0) + 1;
        let v_len = y1.saturating_sub(r).saturating_sub(r) + 1;
        let total_edge = 2 * h_len + 2 * v_len;
        let lit_edge = (ctx.eased * total_edge as f32).round() as usize;
        // Clockwise: top, right, bottom, left.
        let mut rem = lit_edge;
        // Top
        if rem > 0 && top_x0 <= top_x1 {
            let seg = rem.min(h_len);
            draw::hline(grid, top_x0, top_x0 + seg.saturating_sub(1), 0);
            rem = rem.saturating_sub(seg);
        }
        // Right
        if rem > 0 {
            let r_y0 = r;
            let r_y1 = y1.saturating_sub(r);
            let seg = rem.min(v_len);
            if r_y0 <= r_y1 {
                draw::vline(grid, x1, r_y0, r_y0 + seg.saturating_sub(1));
            }
            rem = rem.saturating_sub(seg);
        }
        // Bottom
        if rem > 0 && bot_x0 <= bot_x1 {
            let seg = rem.min(h_len);
            draw::hline(
                grid,
                bot_x1.saturating_sub(seg.saturating_sub(1)),
                bot_x1,
                y1,
            );
            rem = rem.saturating_sub(seg);
        }
        // Left
        if rem > 0 {
            let l_y0 = r;
            let l_y1 = y1.saturating_sub(r);
            let seg = rem.min(v_len);
            if l_y0 <= l_y1 {
                draw::vline(grid, 0, l_y1.saturating_sub(seg.saturating_sub(1)), l_y1);
            }
        }
        // ── Corner arcs (always drawn) ───────────────────────────────────────
        // Approximate a quarter-circle in braille dots for each corner.
        let steps = (r * 4).max(4);
        for s in 0..steps {
            let angle = (s as f32 / steps as f32) * PI / 2.0;
            let ax = (angle.cos() * r as f32).round() as usize;
            let ay = (angle.sin() * r as f32).round() as usize;
            // Top-left (quarter from 180°→90°): arc maps to (r-ax, r-ay).
            draw::dot(grid, r.saturating_sub(ax), r.saturating_sub(ay));
            // Top-right: (x1 - r + ax, r - ay)
            if x1 >= r.saturating_sub(ax) {
                draw::dot(
                    grid,
                    x1.saturating_sub(r).saturating_add(ax).min(x1),
                    r.saturating_sub(ay),
                );
            }
            // Bottom-left: (r - ax, y1 - r + ay)
            draw::dot(
                grid,
                r.saturating_sub(ax),
                y1.saturating_sub(r).saturating_add(ay).min(y1),
            );
            // Bottom-right: (x1 - r + ax, y1 - r + ay)
            draw::dot(
                grid,
                x1.saturating_sub(r).saturating_add(ax).min(x1),
                y1.saturating_sub(r).saturating_add(ay).min(y1),
            );
        }
        let (cw, ch) = grid.dimensions();
        let c = ctx.palette.sample(ctx.eased);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), c);
        }
        Ok(())
    }
}

// ── Style 10: FillFrame ──────────────────────────────────────────────────────

/// The frame thickens inward proportionally to `eased`. At 0 it is a hairline;
/// at 1.0 it fills the entire grid. The result is a frame-shaped progress meter.
struct FillFrame;
impl ProgressStyle for FillFrame {
    fn name(&self) -> &str {
        "fill-frame"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Frame thickens inward with eased — a border-shaped progress meter"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        if pw == 0 || ph == 0 {
            return Ok(());
        }
        // Maximum thickness is half the shorter dimension.
        let max_thick = (pw.min(ph) / 2).max(1);
        let thick = (ctx.eased * max_thick as f32).round() as usize;
        let thick = thick.max(1);
        // Draw concentric outlines from outermost inward to `thick`.
        for t in 0..thick {
            let x0 = t;
            let y0 = t;
            let w = pw.saturating_sub(t * 2);
            let h = ph.saturating_sub(t * 2);
            if w < 1 || h < 1 {
                break;
            }
            draw::rect_outline(grid, x0, y0, w, h);
        }
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }
        Ok(())
    }
}

// ── Style 11: TickedFrame ────────────────────────────────────────────────────

/// The top edge is a ruler with tick marks at every 10% of its length. A
/// traveling marker (solid vline pip) moves left-to-right with `eased`, and
/// the remaining three sides form a plain frame.
struct TickedFrame;
impl ProgressStyle for TickedFrame {
    fn name(&self) -> &str {
        "ticked-frame"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Ruler ticks along the top edge with a traveling marker; plain frame elsewhere"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        if pw == 0 || ph == 0 {
            return Ok(());
        }
        let x1 = pw.saturating_sub(1);
        let y1 = ph.saturating_sub(1);
        // Three sides (not top).
        draw::vline(grid, 0, 0, y1);
        draw::vline(grid, x1, 0, y1);
        draw::hline(grid, 0, x1, y1);
        // Top ruler line.
        draw::hline(grid, 0, x1, 0);
        // Tick marks: short ticks at 10% intervals, longer at 0/50/100.
        for i in 0..=10 {
            let x = (i as f32 / 10.0 * x1 as f32).round() as usize;
            let tick_h = if i == 0 || i == 5 || i == 10 {
                (ph / 3).max(1)
            } else {
                (ph / 6).max(1)
            };
            draw::vline(grid, x, 0, tick_h.min(y1));
        }
        // Traveling marker: a full-height vline at the progress position.
        let marker_x = (ctx.eased * x1 as f32).round() as usize;
        draw::vline(grid, marker_x.min(x1), 0, y1);
        // Tint marker column with end color, rest with start.
        let (cw, ch) = grid.dimensions();
        let base_c = ctx.palette.sample(0.2);
        let marker_c = ctx.palette.sample(1.0);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), base_c);
        }
        // Marker cell column (cell x = marker_x / 2).
        let marker_cx = marker_x / 2;
        for cy in 0..ch {
            draw::tint_row(grid, cy, marker_cx, marker_cx, marker_c);
        }
        Ok(())
    }
}

// ── Style 12: TwoRunners ─────────────────────────────────────────────────────

/// Two bright segments race in opposite directions around the perimeter.
/// They start at top-left, one going clockwise, one counter-clockwise. They
/// meet at the point corresponding to `eased * P/2`. A dim full outline
/// shows the frame.
struct TwoRunners;
impl ProgressStyle for TwoRunners {
    fn name(&self) -> &str {
        "two-runners"
    }
    fn theme(&self) -> &str {
        "border"
    }
    fn describe(&self) -> &str {
        "Two comet segments race opposite ways; they meet at progress * half-perimeter"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (pw, ph) = draw::dot_dims(grid);
        let p = perim_len(pw, ph);
        // Ghost outline — every 3rd dot.
        for i in (0..p).step_by(3) {
            let (x, y) = perim_point(i, pw, ph);
            draw::dot(grid, x, y);
        }
        // Meeting point: each runner travels eased * p/2 from the origin.
        let half = p / 2;
        let reach = (ctx.eased * half as f32).round() as usize;
        let tail = (half / 5).max(2);
        // Clockwise runner.
        for k in 0..reach.min(half) {
            let idx = k % p;
            let (x, y) = perim_point(idx, pw, ph);
            draw::dot(grid, x, y);
        }
        // Counter-clockwise runner (start at same origin, go backwards).
        for k in 0..reach.min(half) {
            let idx = (p - (k % p)) % p;
            let (x, y) = perim_point(idx, pw, ph);
            draw::dot(grid, x, y);
        }
        // Animate time-based comet shimmer on the leading edges.
        let head_cw = reach % p;
        let head_ccw = (p - reach % p) % p;
        let shimmer_tail = tail;
        for k in 0..shimmer_tail {
            let step = (ctx.time * 8.0) as usize;
            if (k + step) % 2 == 0 {
                let idx_cw = (head_cw + p - k) % p;
                let idx_ccw = (head_ccw + k) % p;
                let (x, y) = perim_point(idx_cw, pw, ph);
                draw::dot(grid, x, y);
                let (x2, y2) = perim_point(idx_ccw, pw, ph);
                draw::dot(grid, x2, y2);
            }
        }
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }
        Ok(())
    }
}
