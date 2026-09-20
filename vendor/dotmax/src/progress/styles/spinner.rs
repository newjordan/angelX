//! Indeterminate spinner / busy-indicator styles.
//!
//! Every spinner here is **time-driven**: the animation runs purely from
//! `ctx.time` (seconds elapsed) and looks alive at any fixed progress value.
//! `ctx.progress` is used only as a subtle modifier (e.g. spin-rate scaling)
//! where noted. All spinners are centered in whatever grid they are given and
//! are safe at 1×1 cells.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ─── registry ────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — cool spinner cyan. Applied to styles that draw monochrome.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(84, 222, 255);
const TINT_END: Color = Color::rgb(128, 144, 255);

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

/// All styles in the `spinner` theme.
///
/// Returns 14 structurally distinct spinners in a canonical order. Each entry
/// is a heap-allocated [`ProgressStyle`] suitable for direct use with
/// [`crate::progress::render_lines`] or a [`crate::TerminalRenderer`].
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(BrailleSpinner)),
        Box::new(DotRing),
        Box::new(Tinted(ArcSweep)),
        Box::new(Tinted(DualArc)),
        Box::new(Bounce),
        Box::new(Pulse),
        Box::new(Orbit),
        Box::new(Tinted(ClockHand)),
        Box::new(Tinted(Radar)),
        Box::new(Ellipsis),
        Box::new(GrowingArc),
        Box::new(SquareRunner),
        Box::new(Tinted(SpinnerBars)),
        Box::new(HourglassFlip),
    ]
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Return the dot-space center of the grid as `(cx, cy)` floats.
#[inline]
fn dot_center(grid: &BrailleGrid) -> (f32, f32) {
    let (dw, dh) = draw::dot_dims(grid);
    (dw as f32 * 0.5 - 0.5, dh as f32 * 0.5 - 0.5)
}

/// Draw a single dot from a polar angle `theta` and radius `r`, centered.
#[inline]
fn dot_polar(grid: &mut BrailleGrid, cx: f32, cy: f32, r: f32, theta: f32) {
    let x = cx + r * theta.cos();
    let y = cy + r * theta.sin();
    draw::dot_i(grid, x.round() as i32, y.round() as i32);
}

/// Draw an arc from `theta_start` to `theta_end` with `steps` sample points.
fn draw_arc(
    grid: &mut BrailleGrid,
    cx: f32,
    cy: f32,
    r: f32,
    theta_start: f32,
    theta_end: f32,
    steps: u32,
) {
    if steps == 0 {
        return;
    }
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let theta = theta_start + t * (theta_end - theta_start);
        dot_polar(grid, cx, cy, r, theta);
    }
}

/// Draw a radial line from `(cx,cy)` toward angle `theta`, length `len` dots.
fn draw_radial(grid: &mut BrailleGrid, cx: f32, cy: f32, len: f32, theta: f32) {
    let steps = (len.ceil() as u32).max(1);
    for i in 0..=steps {
        let r = i as f32 * len / steps as f32;
        dot_polar(grid, cx, cy, r, theta);
    }
}

// ─── 1. BrailleSpinner ───────────────────────────────────────────────────────

/// Classic 8-frame braille spinner glyph cycling at the grid center.
struct BrailleSpinner;

const BRAILLE_FRAMES: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];

impl ProgressStyle for BrailleSpinner {
    fn name(&self) -> &str {
        "braille-spin"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Classic ⠋⠙⠹⠸⠼⠴⠦⠧ single-cell braille glyph cycling at the center"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        // Speed subtly scales with progress (faster when more progress).
        let rate = 10.0 + ctx.progress * 6.0;
        let frame = (ctx.time * rate) as usize % BRAILLE_FRAMES.len();
        let cx = cw / 2;
        let cy = ch / 2;
        draw::glyph(grid, cx, cy, BRAILLE_FRAMES[frame]);
        Ok(())
    }
}

// ─── 2. DotRing ──────────────────────────────────────────────────────────────

/// N dots on a circle; one bright head rotates, leaving a dimming comet tail.
struct DotRing;

impl ProgressStyle for DotRing {
    fn name(&self) -> &str {
        "dot-ring"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Rotating comet head on a dot ring — bright lead, fading tail"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        let r = (dw.min(dh) as f32 * 0.35).max(1.0);
        let n: usize = 12;
        let tail_len: usize = 5;

        // Head angle advances clockwise.
        let head_angle = ctx.time * 2.5 * 2.0 * PI;
        let head_idx = (head_angle / (2.0 * PI) * n as f32) as usize % n;

        for i in 0..n {
            // Is this dot within tail_len steps behind the head?
            let behind = (head_idx + n - i) % n;
            if behind < tail_len {
                // tail_len=0 is head (full), tail_len-1 is faintest
                let theta = 2.0 * PI * i as f32 / n as f32;
                // Draw the dot using dot_polar — head always drawn, tail drawn
                // with decreasing density (we just draw every dot; shade varies
                // by skipping for far tail members).
                // Simplest: draw head, draw first half of tail as full dots.
                if behind == 0 || behind < tail_len / 2 + 1 {
                    dot_polar(grid, cx, cy, r, theta);
                } else {
                    // Outer tail: only draw if even index (creates spacing = dimming)
                    if i % 2 == 0 {
                        dot_polar(grid, cx, cy, r, theta);
                    }
                }
            }
            // Non-tail dots: draw a faint ring placeholder using every 3rd dot
            else if i % 3 == 0 {
                let theta = 2.0 * PI * i as f32 / n as f32;
                dot_polar(grid, cx, cy, r, theta);
            }
        }

        // Tint the head bright if color available.
        let head_theta = 2.0 * PI * head_idx as f32 / n as f32;
        let hx = (cx + r * head_theta.cos()).round() as i32;
        let hy = (cy + r * head_theta.sin()).round() as i32;
        if hx >= 0 && hy >= 0 {
            let cell_x = (hx as usize) / 2;
            let cell_y = (hy as usize) / 4;
            let color = ctx.palette.sample(1.0);
            let (cw, ch) = grid.dimensions();
            if cell_x < cw && cell_y < ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }
        Ok(())
    }
}

// ─── 3. ArcSweep ─────────────────────────────────────────────────────────────

/// A quarter-arc rotating around the center — single sweeping arc.
struct ArcSweep;

impl ProgressStyle for ArcSweep {
    fn name(&self) -> &str {
        "arc-sweep"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "A quarter-arc sweeping clockwise around the center"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        let r = (dw.min(dh) as f32 * 0.38).max(1.0);
        let arc_span = PI * 0.5; // quarter turn
        let head = ctx.time * 2.0 * PI * 0.8;
        let steps = ((r * arc_span).ceil() as u32).max(2).min(32);
        draw_arc(grid, cx, cy, r, head, head + arc_span, steps);
        Ok(())
    }
}

// ─── 4. DualArc ──────────────────────────────────────────────────────────────

/// Two counter-rotating half-arcs on the same circle.
struct DualArc;

impl ProgressStyle for DualArc {
    fn name(&self) -> &str {
        "dual-arc"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Two half-arcs spinning in opposite directions on the same ring"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        let r = (dw.min(dh) as f32 * 0.38).max(1.0);
        let arc_span = PI * 0.55;
        let t = ctx.time * 2.0 * PI * 0.7;
        let steps = ((r * arc_span).ceil() as u32).max(2).min(32);
        // Forward arc.
        draw_arc(grid, cx, cy, r, t, t + arc_span, steps);
        // Counter-rotating arc (offset by PI).
        draw_arc(grid, cx, cy, r, -t + PI, -t + PI + arc_span, steps);
        Ok(())
    }
}

// ─── 5. Bounce ───────────────────────────────────────────────────────────────

/// A dot bouncing left-right (Cylon/Knight Rider scanner) along the center row.
struct Bounce;

impl ProgressStyle for Bounce {
    fn name(&self) -> &str {
        "bounce"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Knight Rider: a dot bouncing left-right along the center with a fading trail"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cy = dh / 2;
        // Sine oscillates -1..1, map to 0..dw-1.
        let phase = (ctx.time * PI * 0.9).sin(); // -1..1
        let head_x = ((phase * 0.5 + 0.5) * (dw.saturating_sub(1)) as f32).round() as usize;
        // Trail: draw 4 dots fading toward head.
        let trail_len: i32 = 5;
        let direction_sign: i32 = if phase >= 0.0 { 1 } else { -1 };
        for i in 0..trail_len {
            let tx = head_x as i32 - direction_sign * i;
            if i == 0 {
                draw::dot_i(grid, tx, cy as i32);
            } else if i < trail_len / 2 + 1 {
                draw::dot_i(grid, tx, cy as i32);
            } else if i % 2 == 0 {
                draw::dot_i(grid, tx, cy as i32);
            }
        }
        // Tint head cell.
        let cell_x = head_x / 2;
        let cell_y = cy / 4;
        let (cw, ch) = grid.dimensions();
        if cell_x < cw && cell_y < ch {
            let color = ctx.palette.sample(head_x as f32 / dw.max(1) as f32);
            draw::tint_row(grid, cell_y, cell_x, cell_x, color);
        }
        Ok(())
    }
}

// ─── 6. Pulse ────────────────────────────────────────────────────────────────

/// A circle that expands and contracts (breathing) with time.
struct Pulse;

impl ProgressStyle for Pulse {
    fn name(&self) -> &str {
        "pulse"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Breathing circle that expands and contracts with a sine rhythm"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        let max_r = (dw.min(dh) as f32 * 0.40).max(1.0);
        // Breathe: r oscillates between 30% and 100% of max_r.
        let breath = (ctx.time * PI * 0.7).sin() * 0.5 + 0.5; // 0..1
        let r = max_r * (0.3 + breath * 0.7);
        let steps = ((2.0 * PI * r).ceil() as u32).max(4).min(64);
        draw_arc(grid, cx, cy, r, 0.0, 2.0 * PI, steps);
        // Tint proportionally to pulse phase.
        let (cw, ch) = grid.dimensions();
        let mid_cy = ch / 2;
        if ch > 0 && cw > 0 {
            let color = ctx.palette.sample(breath);
            draw::tint_row(grid, mid_cy, 0, cw.saturating_sub(1), color);
        }
        Ok(())
    }
}

// ─── 7. Orbit ────────────────────────────────────────────────────────────────

/// A small moon orbiting a fixed center dot.
struct Orbit;

impl ProgressStyle for Orbit {
    fn name(&self) -> &str {
        "orbit"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Moon orbiting a stationary center planet"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        // Center dot (planet).
        draw::dot_i(grid, cx.round() as i32, cy.round() as i32);

        let r = (dw.min(dh) as f32 * 0.35).max(1.5);
        let theta = ctx.time * 2.0 * PI * 0.6;
        // Moon.
        dot_polar(grid, cx, cy, r, theta);
        // Short trail behind moon.
        for i in 1..3usize {
            let trail_theta = theta - i as f32 * (PI / 8.0);
            if i == 1 {
                dot_polar(grid, cx, cy, r, trail_theta);
            }
        }
        // Tint moon cell.
        let mx = (cx + r * theta.cos()).round() as i32;
        let my = (cy + r * theta.sin()).round() as i32;
        if mx >= 0 && my >= 0 {
            let cell_x = (mx as usize) / 2;
            let cell_y = (my as usize) / 4;
            let (cw, ch) = grid.dimensions();
            if cell_x < cw && cell_y < ch {
                let color = ctx.palette.sample(0.8);
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }
        Ok(())
    }
}

// ─── 8. ClockHand ────────────────────────────────────────────────────────────

/// A single radial line sweeping like a clock hand.
struct ClockHand;

impl ProgressStyle for ClockHand {
    fn name(&self) -> &str {
        "clock-hand"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Clock-hand: a single radial spoke sweeping 360° with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        let len = (dw.min(dh) as f32 * 0.40).max(1.0);
        // Clockwise from 12 o'clock (−π/2).
        let theta = ctx.time * 2.0 * PI * 0.5 - PI / 2.0;
        draw_radial(grid, cx, cy, len, theta);
        // Center hub dot.
        draw::dot_i(grid, cx.round() as i32, cy.round() as i32);
        Ok(())
    }
}

// ─── 9. Radar ────────────────────────────────────────────────────────────────

/// Sweeping radar line leaving a fading wedge behind it.
struct Radar;

impl ProgressStyle for Radar {
    fn name(&self) -> &str {
        "radar"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Radar sweep: rotating spoke with a fading wedge afterglow"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        let r = (dw.min(dh) as f32 * 0.40).max(1.0);
        let head = ctx.time * 2.0 * PI * 0.55;

        // Fading wedge: 4 ghost spokes behind the head.
        for ghost in 1..=4usize {
            let fade_angle = head - ghost as f32 * (PI / 10.0);
            // Only draw every-other dot on ghosts to imply fading.
            let steps = ((r * 0.8).ceil() as u32).max(1).min(20);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let gr = t * r * 0.85;
                // Skip interior dots for farthest ghosts.
                if ghost > 2 && i % 2 != 0 {
                    continue;
                }
                let gx = cx + gr * fade_angle.cos();
                let gy = cy + gr * fade_angle.sin();
                draw::dot_i(grid, gx.round() as i32, gy.round() as i32);
            }
        }

        // Solid head spoke.
        let steps = (r.ceil() as u32).max(1).min(24);
        draw_radial(grid, cx, cy, r, head);
        let _ = steps; // used above

        // Outer ring (full circle, sparse).
        let n = 24u32;
        for i in 0..n {
            if i % 4 == 0 {
                let theta = 2.0 * PI * i as f32 / n as f32;
                dot_polar(grid, cx, cy, r, theta);
            }
        }

        // Center dot.
        draw::dot_i(grid, cx.round() as i32, cy.round() as i32);
        Ok(())
    }
}

// ─── 10. Ellipsis ────────────────────────────────────────────────────────────

/// Three dots that light in sequence (· ·· ···) — the "Loading..." cycle.
struct Ellipsis;

impl ProgressStyle for Ellipsis {
    fn name(&self) -> &str {
        "ellipsis"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Three-dot ellipsis cycling · ·· ··· — the classic Loading... indicator"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }
        // How many dots are lit: 0, 1, 2, 3, repeating every ~1.2 s.
        let phase = (ctx.time / 1.2).fract(); // 0..1
        let lit = (phase * 4.0) as usize; // 0,1,2,3

        let dot_count = 3usize;
        // Center the three glyph cells.
        let start_x = cw.saturating_sub(dot_count) / 2;
        let mid_y = ch / 2;

        // Braille ellipsis frames for each slot.
        const DARK: char = '⠂'; // faint placeholder dot
        const BRIGHT: char = '⣿'; // full braille block

        for i in 0..dot_count {
            let cx = (start_x + i).min(cw.saturating_sub(1));
            if i < lit.min(dot_count) {
                draw::glyph(grid, cx, mid_y, BRIGHT);
                let color = ctx.palette.sample(i as f32 / dot_count as f32);
                draw::tint_row(grid, mid_y, cx, cx, color);
            } else {
                draw::glyph(grid, cx, mid_y, DARK);
            }
        }
        Ok(())
    }
}

// ─── 11. GrowingArc ──────────────────────────────────────────────────────────

/// An arc that grows then shrinks as it rotates (Material / Android spinner).
struct GrowingArc;

impl ProgressStyle for GrowingArc {
    fn name(&self) -> &str {
        "growing-arc"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Material-style arc that grows then shrinks as it rotates around the ring"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        let r = (dw.min(dh) as f32 * 0.38).max(1.0);

        // Period = 2 s: first half arc grows 10°→270°, second half it shrinks.
        let cycle = (ctx.time * 0.5).fract(); // 0..1 per 2 s
        let arc_frac = if cycle < 0.5 {
            // growing: 0 → full
            cycle * 2.0
        } else {
            // shrinking: full → 0
            1.0 - (cycle - 0.5) * 2.0
        };
        let arc_span = (PI / 18.0) + arc_frac * (PI * 1.4); // 10° to ~252°

        // Head rotates continuously.
        let head = ctx.time * 2.0 * PI * 0.4;
        let steps = ((r * arc_span).ceil() as u32).max(2).min(48);
        draw_arc(grid, cx, cy, r, head, head + arc_span, steps);

        // Tint the head end.
        let tip_theta = head + arc_span;
        let tx = (cx + r * tip_theta.cos()).round() as i32;
        let ty = (cy + r * tip_theta.sin()).round() as i32;
        if tx >= 0 && ty >= 0 {
            let cell_x = (tx as usize) / 2;
            let cell_y = (ty as usize) / 4;
            let (cw, ch) = grid.dimensions();
            if cell_x < cw && cell_y < ch {
                let color = ctx.palette.sample(1.0);
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }
        Ok(())
    }
}

// ─── 12. SquareRunner ────────────────────────────────────────────────────────

/// A dot running clockwise around the perimeter of a centered square.
struct SquareRunner;

impl ProgressStyle for SquareRunner {
    fn name(&self) -> &str {
        "square-runner"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "A dot chasing around the perimeter of a centered square — with outline"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Square side in dots (fit inside grid).
        let side = dw.min(dh).saturating_sub(2).max(1);
        let half = side / 2;
        let (cx, cy) = dot_center(grid);
        let x0 = (cx - half as f32).round().max(0.0) as usize;
        let y0 = (cy - half as f32).round().max(0.0) as usize;

        // Draw outline.
        draw::rect_outline(grid, x0, y0, side, side);

        // Perimeter length.
        let perim = (side.saturating_sub(1)) * 4;
        if perim == 0 {
            return Ok(());
        }

        // Position along perimeter 0..perim.
        let pos = (ctx.time * (perim as f32) * 0.5).rem_euclid(perim as f32) as usize;

        // Map pos to (x, y) — clockwise: top, right, bottom (reversed), left (reversed).
        let seg = side.saturating_sub(1).max(1);
        let (rx, ry) = if pos < seg {
            // top edge left→right
            (x0 + pos, y0)
        } else if pos < 2 * seg {
            // right edge top→bottom
            (x0 + side.saturating_sub(1), y0 + (pos - seg))
        } else if pos < 3 * seg {
            // bottom edge right→left
            (
                x0 + side.saturating_sub(1).saturating_sub(pos - 2 * seg),
                y0 + side.saturating_sub(1),
            )
        } else {
            // left edge bottom→top
            (
                x0,
                y0 + side.saturating_sub(1).saturating_sub(pos - 3 * seg),
            )
        };

        // Draw runner dot (on top of outline).
        draw::dot(grid, rx, ry);
        // Tint runner cell.
        let cell_x = rx / 2;
        let cell_y = ry / 4;
        let (cw, ch) = grid.dimensions();
        if cell_x < cw && cell_y < ch {
            let t = pos as f32 / perim.max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cell_y, cell_x, cell_x, color);
        }
        Ok(())
    }
}

// ─── 13. SpinnerBars ─────────────────────────────────────────────────────────

/// A ring of short radial spokes fading in sequence — the classic throbber.
struct SpinnerBars;

impl ProgressStyle for SpinnerBars {
    fn name(&self) -> &str {
        "spinner-bars"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Ring of 8 radial spokes fading in sequence — the classic macOS throbber"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = dot_center(grid);
        let outer_r = (dw.min(dh) as f32 * 0.40).max(2.0);
        let inner_r = (outer_r * 0.45).max(1.0);
        let n: usize = 8;

        // Which spoke is the bright head.
        let head_idx = (ctx.time * n as f32 * 1.5).rem_euclid(n as f32) as usize % n;

        for i in 0..n {
            let theta = 2.0 * PI * i as f32 / n as f32 - PI / 2.0; // from 12 o'clock
                                                                   // Distance behind head (0 = head, n-1 = farthest).
            let behind = (head_idx + n - i) % n;
            // Only draw spokes within the "lit" arc (0..n/2+1).
            if behind > n / 2 {
                continue; // dark half — skip spoke
            }
            // Spokes closer to head are longer; farther ones are shorter.
            let len_frac = 1.0 - behind as f32 / (n / 2 + 1) as f32;
            let spoke_outer = inner_r + (outer_r - inner_r) * len_frac;

            // Draw spoke as dots from inner_r to spoke_outer.
            let steps = ((spoke_outer - inner_r).ceil() as u32).max(1).min(8);
            for s in 0..=steps {
                let r = inner_r + (spoke_outer - inner_r) * s as f32 / steps as f32;
                dot_polar(grid, cx, cy, r, theta);
            }
        }
        Ok(())
    }
}

// ─── 14. HourglassFlip ───────────────────────────────────────────────────────

/// An hourglass glyph that flips between ⧗ and ⧖ every ~0.8 s.
struct HourglassFlip;

const HOURGLASS_FRAMES: [char; 4] = ['⧗', '⧗', '⧖', '⧖'];

impl ProgressStyle for HourglassFlip {
    fn name(&self) -> &str {
        "hourglass-flip"
    }
    fn theme(&self) -> &str {
        "spinner"
    }
    fn describe(&self) -> &str {
        "Hourglass glyph flipping ⧗↔⧖ with a braille-dot shimmer around it"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        let frame = (ctx.time * 1.25) as usize % HOURGLASS_FRAMES.len();
        let cx = cw / 2;
        let cy = ch / 2;

        // Place hourglass glyph at center.
        draw::glyph(grid, cx, cy, HOURGLASS_FRAMES[frame]);

        // Tint center cell.
        let color = ctx.palette.sample(if frame < 2 { 0.2 } else { 0.8 });
        draw::tint_row(grid, cy, cx, cx, color);

        // Braille shimmer: rotating single-dot around the glyph cell (dot space).
        let (dw, dh) = draw::dot_dims(grid);
        let dcx = dw as f32 * 0.5;
        let dcy = dh as f32 * 0.5;
        let r = 2.5_f32.max(1.0);
        let theta = ctx.time * 2.0 * PI * 1.2;
        let sx = (dcx + r * theta.cos()).round() as i32;
        let sy = (dcy + r * theta.sin()).round() as i32;
        // Only draw shimmer dot if it doesn't land on the center glyph cell.
        let shimmer_cell_x = (sx.max(0) as usize) / 2;
        let shimmer_cell_y = (sy.max(0) as usize) / 4;
        if shimmer_cell_x != cx || shimmer_cell_y != cy {
            draw::dot_i(grid, sx, sy);
        }

        Ok(())
    }
}
