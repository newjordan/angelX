//! Animals-themed progress bars — ten distinct creatures drawn in braille dots.
//!
//! Every bar is stateless: all motion comes from `ctx.time` (for perpetual
//! animation) and `ctx.eased` (for progress-driven advancement). The bars use
//! only the `draw::` helpers so all writes are silently bounds-safe.
//!
//! Styles in this file:
//! - `caterpillar`  — segmented body that undulates across the bar
//! - `snail`        — shell advancing over a slime trail
//! - `inchworm`     — bunching and stretching segments via eased spacing
//! - `fish-school`  — sine-bobbing dots swimming in formation
//! - `snake`        — sine-wave lateral slither advancing with progress
//! - `rabbit-hops`  — parabolic jump arcs across discrete hop chunks
//! - `paw-prints`   — alternating offset prints appearing one by one
//! - `bird-flock`   — V-formation sweeping across the bar
//! - `turtle`       — dome shell that fills from the bottom up
//! - `ant-march`    — ants carrying the load in single file

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Theme tint — meadow green into honey. Applied to styles that draw monochrome.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(124, 204, 92);
const TINT_END: Color = Color::rgb(240, 184, 64);

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

/// All styles in the `animals` theme.
///
/// Returns one boxed [`ProgressStyle`] per animal variant, ready to be mixed
/// into a gallery or driven individually.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(Caterpillar)),
        Box::new(Tinted(Snail)),
        Box::new(Tinted(Inchworm)),
        Box::new(FishSchool),
        Box::new(Tinted(Snake)),
        Box::new(Tinted(RabbitHops)),
        Box::new(PawPrints),
        Box::new(BirdFlock),
        Box::new(Tinted(Turtle)),
        Box::new(AntMarch),
    ]
}

// ── Caterpillar ──────────────────────────────────────────────────────────────

struct Caterpillar;
impl ProgressStyle for Caterpillar {
    fn name(&self) -> &str {
        "caterpillar"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "Segmented caterpillar body undulating across the bar as it advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = h / 2;
        // How far the head has advanced (in dots).
        let head_x = (ctx.eased * w as f32) as usize;
        let seg_spacing = 4usize;
        // Phase offset scrolls with time for continuous crawl motion.
        let phase_offset = ctx.time * 6.0;
        // Draw body segments from tail to head.
        let mut x = 0usize;
        while x < head_x {
            // Sine-wave vertical displacement — body undulates.
            let wave = ((x as f32 * 0.4 + phase_offset) * 1.0).sin();
            let amp = ((h / 2).saturating_sub(1).max(1)) as f32 * 0.6;
            let y = (mid as f32 + wave * amp).round() as usize;
            let y = y.min(h - 1);
            // Segment body dot (slightly wider near center).
            draw::dot(grid, x, y);
            if x + 1 < head_x {
                draw::dot(grid, x + 1, y);
            }
            // Every seg_spacing dots, draw a leg nub above and below.
            if (x / seg_spacing) % 2 == 0 {
                if y + 1 < h {
                    draw::dot(grid, x, y + 1);
                }
                if y >= 1 {
                    draw::dot(grid, x, y - 1);
                }
            }
            x += seg_spacing;
        }
        // Head: two dots at the leading edge with antennae.
        if head_x > 0 {
            let hx = head_x.min(w - 1);
            let hy = mid.min(h - 1);
            draw::dot(grid, hx, hy);
            // Antenna: two dots diagonally up.
            draw::dot_i(grid, hx as i32 + 1, hy as i32 - 1);
            draw::dot_i(grid, hx as i32 + 1, hy as i32 - 2);
            draw::dot_i(grid, hx as i32 + 2, hy as i32 - 1);
        }
        Ok(())
    }
}

// ── Snail ─────────────────────────────────────────────────────────────────────

struct Snail;
impl ProgressStyle for Snail {
    fn name(&self) -> &str {
        "snail"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "Snail leaving a slime trail — shell advances, trail fills behind it"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = (h - 1).min(h.saturating_sub(1));
        let head_x = (ctx.eased * w as f32) as usize;
        // Slime trail: a single dot line along the base behind the snail.
        if head_x > 0 {
            draw::hline(grid, 0, head_x.saturating_sub(1).min(w - 1), base);
            // Slime shimmer — occasional dots one row above the trail.
            let shimmer_period = 8usize;
            let mut sx = 0usize;
            while sx < head_x.saturating_sub(2) {
                draw::dot(grid, sx, base.saturating_sub(1));
                sx += shimmer_period;
            }
        }
        // Shell: a small dome at head_x.
        let sx = head_x.min(w.saturating_sub(1));
        let shell_h = (h / 2).max(2);
        let shell_w = (shell_h).min(w.saturating_sub(sx));
        // Dome outline: arc of dots.
        for i in 0..=shell_w {
            let t = if shell_w == 0 {
                0.5
            } else {
                i as f32 / shell_w as f32
            };
            let arc_y = (base as f32 - (1.0 - (t * PI).sin() * (shell_h as f32 - 1.0)).max(0.0))
                .round() as usize;
            let arc_y = arc_y.max(base.saturating_sub(shell_h));
            draw::dot(grid, (sx + i).min(w - 1), arc_y.min(h - 1));
        }
        // Shell spiral (time-animated bob).
        let bob = ((ctx.time * 3.0).sin() * 0.4).round() as i32;
        let inner_x = sx as i32 + shell_w as i32 / 2;
        let inner_y = base as i32 - shell_h as i32 / 2 + bob;
        draw::dot_i(grid, inner_x, inner_y);
        // Head: eyestalk.
        let eye_x = sx as i32 + shell_w as i32 + 1;
        draw::dot_i(grid, eye_x, base as i32);
        draw::dot_i(grid, eye_x, base as i32 - 1);
        draw::dot_i(grid, eye_x + 1, base as i32 - 2);
        Ok(())
    }
}

// ── Inchworm ─────────────────────────────────────────────────────────────────

struct Inchworm;
impl ProgressStyle for Inchworm {
    fn name(&self) -> &str {
        "inchworm"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "Inchworm bunching and stretching — eased segment spacing gives organic motion"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = h / 2;
        // The inchworm body spans from 0 to head_x, split into N segments.
        let head_x = (ctx.eased * w as f32) as usize;
        let n_segs = 6usize;
        // Bunch/stretch phase: oscillates with time. 0 = fully bunched, 1 = stretched.
        let stretch_phase = ((ctx.time * 2.0).sin() * 0.5 + 0.5) as f32;
        // Map segment index → x position using an eased distribution.
        for seg in 0..=n_segs {
            let raw_t = if n_segs == 0 {
                0.5
            } else {
                seg as f32 / n_segs as f32
            };
            // Interpolate between bunched (quadratic cluster at head) and stretched (linear).
            let bunched = raw_t * raw_t; // cluster toward head
            let t = bunched * (1.0 - stretch_phase) + raw_t * stretch_phase;
            let sx = (t * head_x as f32) as usize;
            let sx = sx.min(w - 1);
            // Arch height: tallest in the middle of the body.
            let arch = (PI * raw_t).sin();
            let arch_h = (arch * (h / 2) as f32).round() as usize;
            let sy = mid.saturating_sub(arch_h).min(h - 1);
            draw::dot(grid, sx, sy);
            // Connect adjacent segments with a line.
            if seg > 0 {
                let prev_t_raw = (seg - 1) as f32 / n_segs as f32;
                let prev_bunched = prev_t_raw * prev_t_raw;
                let prev_t = prev_bunched * (1.0 - stretch_phase) + prev_t_raw * stretch_phase;
                let prev_x = (prev_t * head_x as f32) as usize;
                let prev_arch = (PI * prev_t_raw).sin();
                let prev_arch_h = (prev_arch * (h / 2) as f32).round() as usize;
                let prev_y = mid.saturating_sub(prev_arch_h).min(h - 1);
                // Draw midpoint bridging dot.
                let bx = (prev_x + sx) / 2;
                let by = (prev_y + sy) / 2;
                draw::dot(grid, bx.min(w - 1), by.min(h - 1));
            }
        }
        // Head features.
        if head_x > 0 {
            let hx = head_x.min(w.saturating_sub(1));
            draw::dot_i(grid, hx as i32 + 1, mid as i32);
            draw::dot_i(grid, hx as i32 + 1, mid as i32 - 1);
        }
        Ok(())
    }
}

// ── Fish School ───────────────────────────────────────────────────────────────

struct FishSchool;
impl ProgressStyle for FishSchool {
    fn name(&self) -> &str {
        "fish-school"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "School of fish bobbing in sine-wave formation, swimming with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let front_x = (ctx.eased * w as f32) as usize;
        // Number of fish scales with bar width.
        let n_fish = (w / 8).max(2).min(10);
        let amp = (h / 2).saturating_sub(1).max(1) as f32 * 0.8;
        let mid = h as f32 / 2.0;
        for i in 0..n_fish {
            // Each fish is offset behind the leader proportionally.
            let lag = i as f32 / n_fish as f32;
            // Fish x position: lead fish at front_x, others trail behind.
            let fish_x = (front_x as f32 * (1.0 - lag * 0.35)) as usize;
            let fish_x = fish_x.min(w.saturating_sub(2));
            // Vertical bobbing: each fish has a phase offset.
            let phase = i as f32 * (PI * 2.0 / n_fish as f32);
            let bob_y = mid + amp * (ctx.time * 4.0 + phase).sin();
            let fy = (bob_y.round() as usize).min(h - 1);
            // Fish body: two dots.
            draw::dot(grid, fish_x, fy);
            if fish_x + 1 < w {
                draw::dot(grid, fish_x + 1, fy);
            }
            // Fish tail: a dot forked behind.
            if fish_x >= 1 {
                draw::dot(grid, fish_x - 1, fy.saturating_sub(1).min(h - 1));
                draw::dot(grid, fish_x - 1, (fy + 1).min(h - 1));
            }
        }
        // Color: gradient across the school.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx in 0..filled_cells.min(cells_w) {
                let t = if filled_cells <= 1 {
                    0.5
                } else {
                    cx as f32 / filled_cells as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── Snake ─────────────────────────────────────────────────────────────────────

struct Snake;
impl ProgressStyle for Snake {
    fn name(&self) -> &str {
        "snake"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "Snake slithering — sine-wave lateral offset along a horizontal advance"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = h as f32 / 2.0;
        let amp = (h as f32 / 2.0 - 1.0).max(0.5);
        let head_x = (ctx.eased * w as f32) as usize;
        // Phase scrolls with time for continuous slither.
        let phase = ctx.time * 5.0;
        // Draw snake body from tail to head.
        for x in 0..head_x.min(w) {
            // Lateral sine offset; frequency increases toward the head (tighter wriggle).
            let freq = 0.25 + (x as f32 / w.max(1) as f32) * 0.25;
            let y = mid + amp * (x as f32 * freq + phase).sin();
            let y = (y.round() as usize).min(h - 1);
            draw::dot(grid, x, y);
            // Slight body thickness — extra dot perpendicular.
            if x % 3 == 0 {
                let y2 = (y + 1).min(h - 1);
                draw::dot(grid, x, y2);
            }
        }
        // Head: tongue flick (time-based).
        if head_x > 0 {
            let hx = head_x.min(w - 1);
            let hy_f = mid + amp * (hx as f32 * 0.5 + phase).sin();
            let hy = (hy_f.round() as usize).min(h - 1);
            // Tongue: two dots above the head at a tongue-flick interval.
            let tongue = ((ctx.time * 3.0).sin() > 0.4) as usize;
            if tongue > 0 {
                draw::dot_i(grid, hx as i32 + 1, hy as i32 - 1);
                draw::dot_i(grid, hx as i32 + 2, hy as i32 - 2);
                draw::dot_i(grid, hx as i32 + 2, hy as i32);
            } else {
                draw::dot_i(grid, hx as i32 + 1, hy as i32);
            }
            // Eye.
            draw::dot(grid, hx, hy.saturating_sub(1));
        }
        Ok(())
    }
}

// ── Rabbit Hops ───────────────────────────────────────────────────────────────

struct RabbitHops;
impl ProgressStyle for RabbitHops {
    fn name(&self) -> &str {
        "rabbit-hops"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "Rabbit hopping across in parabolic arcs — each hop a discrete progress chunk"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let n_hops = 5usize;
        let hop_w = w / n_hops.max(1);
        let base = h - 1;
        let peak = (h * 2 / 3).max(1);
        // Determine which hop the rabbit is in.
        let total_progress = ctx.eased;
        let hop_index_f = total_progress * n_hops as f32;
        let current_hop = (hop_index_f as usize).min(n_hops - 1);
        // Within-hop fraction (0=take-off, 1=landing).
        let hop_frac = hop_index_f.fract();
        // Draw ground dots for completed hops.
        for h_idx in 0..current_hop {
            let gx = h_idx * hop_w + hop_w / 2;
            draw::dot(grid, gx.min(w - 1), base);
        }
        // Draw the rabbit at its current arc position.
        let hop_start_x = current_hop * hop_w;
        let hop_end_x = (hop_start_x + hop_w).min(w);
        // Animate within the hop using time-driven oscillation when hop_frac is stalled.
        let t = if ctx.progress >= 1.0 {
            hop_frac
        } else {
            hop_frac
        };
        // Parabolic arc: y = -4h * t * (t - 1) gives peak at t=0.5.
        let arc = -4.0 * peak as f32 * t * (t - 1.0);
        let rx = hop_start_x + (hop_frac * (hop_end_x - hop_start_x) as f32) as usize;
        let ry_raw = base as f32 - arc;
        let ry = (ry_raw.round() as usize).min(h - 1);
        let rx = rx.min(w.saturating_sub(2));
        // Rabbit body (two dots).
        draw::dot(grid, rx, ry);
        if rx + 1 < w {
            draw::dot(grid, rx + 1, ry);
        }
        // Ears (two dots above body).
        draw::dot_i(grid, rx as i32, ry as i32 - 1);
        draw::dot_i(grid, rx as i32 + 1, ry as i32 - 1);
        draw::dot_i(grid, rx as i32, ry as i32 - 2);
        draw::dot_i(grid, rx as i32 + 2, ry as i32 - 2);
        // Shadow on ground under rabbit.
        draw::dot(grid, rx.min(w - 1), base);
        Ok(())
    }
}

// ── Paw Prints ────────────────────────────────────────────────────────────────

struct PawPrints;
impl ProgressStyle for PawPrints {
    fn name(&self) -> &str {
        "paw-prints"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "Paw prints appearing one by one, alternating left/right offset across the bar"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let n_prints = 8usize;
        let visible = (ctx.eased * n_prints as f32).ceil() as usize;
        let mid = h / 2;
        let step_w = w / n_prints.max(1);
        let offset = (h / 4).max(1);
        for i in 0..visible.min(n_prints) {
            let px = i * step_w + step_w / 2;
            let px = px.min(w.saturating_sub(2));
            // Alternate left/right offset.
            let (py, small_off) = if i % 2 == 0 {
                (mid.saturating_sub(offset), 1usize)
            } else {
                ((mid + offset).min(h.saturating_sub(2)), 0usize)
            };
            // Pad: a 2×2 cluster.
            draw::dot(grid, px, py);
            draw::dot(grid, (px + 1).min(w - 1), py);
            draw::dot(grid, px, (py + 1).min(h - 1));
            draw::dot(grid, (px + 1).min(w - 1), (py + 1).min(h - 1));
            // Three toe dots above the pad.
            let toe_y = py.saturating_sub(1);
            draw::dot(grid, px.saturating_sub(1), toe_y);
            draw::dot(grid, px, toe_y.saturating_sub(small_off));
            draw::dot(grid, (px + 1).min(w - 1), toe_y);
            // Tint completed prints with palette color.
            let t = if n_prints <= 1 {
                1.0
            } else {
                i as f32 / (n_prints - 1) as f32
            };
            let color = ctx.palette.sample(t);
            let (cells_w, cells_h) = grid.dimensions();
            let cell_x = (px / 2).min(cells_w.saturating_sub(1));
            for cy in 0..cells_h {
                draw::tint_row(
                    grid,
                    cy,
                    cell_x,
                    (cell_x + 1).min(cells_w.saturating_sub(1)),
                    color,
                );
            }
        }
        Ok(())
    }
}

// ── Bird Flock ────────────────────────────────────────────────────────────────

struct BirdFlock;
impl ProgressStyle for BirdFlock {
    fn name(&self) -> &str {
        "bird-flock"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "V-formation bird flock sweeping across the bar, wings flapping with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = h / 2;
        let leader_x = (ctx.eased * w as f32) as usize;
        // Wing flap: oscillate between up and down positions.
        let flap = (ctx.time * 5.0).sin();
        let wing_spread = ((h / 4).max(1)) as f32;
        // Birds in a V: index 0 = leader, positive/negative index = left/right wing.
        let n_side = 3usize; // birds on each side of the V
        let v_step_x = 3usize; // horizontal spacing between birds in the V
        let v_step_y = 1usize; // vertical depth per step back in the V
                               // Draw leader.
        let lx = leader_x.min(w.saturating_sub(1));
        let ly = mid.min(h - 1);
        draw_bird(grid, lx, ly, flap, wing_spread, w, h);
        // Draw left and right wings.
        for side in 0..n_side {
            let offset_x = (side + 1) * v_step_x;
            let offset_y = (side + 1) * v_step_y;
            // Phase lag per bird — trailing birds flap slightly later.
            let lag = side as f32 * 0.4;
            let flap_bird = (ctx.time * 5.0 - lag).sin();
            // Left wing bird (above mid).
            let lx_left = leader_x.saturating_sub(offset_x);
            let ly_left = mid.saturating_sub(offset_y).min(h - 1);
            draw_bird(
                grid,
                lx_left.min(w.saturating_sub(1)),
                ly_left,
                flap_bird,
                wing_spread * 0.7,
                w,
                h,
            );
            // Right wing bird (below mid).
            let lx_right = leader_x.saturating_sub(offset_x);
            let ly_right = (mid + offset_y).min(h - 1);
            draw_bird(
                grid,
                lx_right.min(w.saturating_sub(1)),
                ly_right,
                flap_bird,
                wing_spread * 0.7,
                w,
                h,
            );
        }
        // Gradient tint on the swept region.
        let (cells_w, cells_h) = grid.dimensions();
        let swept = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx in 0..swept.min(cells_w) {
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

/// Draw a single bird glyph at (bx, by) with a wing flap value in [-1, 1].
fn draw_bird(
    grid: &mut BrailleGrid,
    bx: usize,
    by: usize,
    flap: f32,
    wing_spread: f32,
    _w: usize,
    h: usize,
) {
    let wing_h = (flap * wing_spread * 0.5).round() as i32;
    // Body dot.
    draw::dot_i(grid, bx as i32, by as i32);
    // Left wing.
    draw::dot_i(grid, bx as i32 - 1, by as i32 + wing_h);
    draw::dot_i(
        grid,
        bx as i32 - 2,
        by as i32 + wing_h.abs().min((h / 2) as i32),
    );
    // Right wing.
    draw::dot_i(grid, bx as i32 + 1, by as i32 + wing_h);
    draw::dot_i(
        grid,
        bx as i32 + 2,
        by as i32 + wing_h.abs().min((h / 2) as i32),
    );
}

// ── Turtle ────────────────────────────────────────────────────────────────────

struct Turtle;
impl ProgressStyle for Turtle {
    fn name(&self) -> &str {
        "turtle"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "Turtle shell dome that fills from bottom up as progress increases"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        // Shell dome: centered, width = 80% of bar, height = full bar.
        let shell_w = (w * 4 / 5).max(4).min(w);
        let x0 = w.saturating_sub(shell_w) / 2;
        let base = h - 1;
        // Draw dome outline using semi-ellipse.
        for xi in 0..=shell_w {
            let t = if shell_w == 0 {
                0.5
            } else {
                xi as f32 / shell_w as f32
            };
            // Ellipse: y = h * sqrt(1 - (2t-1)^2).
            let norm = 2.0 * t - 1.0;
            let ellipse_y = (h as f32 * (1.0 - norm * norm).sqrt()).min(h as f32 - 1.0);
            let top_y = base.saturating_sub(ellipse_y.round() as usize);
            draw::dot(grid, (x0 + xi).min(w - 1), top_y.min(h - 1));
        }
        // Bottom base line.
        draw::hline(grid, x0, (x0 + shell_w).min(w - 1), base);
        // Fill from base up to progress fraction of dome height.
        let fill_height = (ctx.eased * h as f32).round() as usize;
        for xi in 0..=shell_w {
            let t = if shell_w == 0 {
                0.5
            } else {
                xi as f32 / shell_w as f32
            };
            let norm = 2.0 * t - 1.0;
            let dome_height = (h as f32 * (1.0 - norm * norm).sqrt()).round() as usize;
            let col_fill = fill_height.min(dome_height);
            if col_fill > 0 {
                let y_top = base.saturating_sub(col_fill);
                draw::vline(grid, (x0 + xi).min(w - 1), y_top, base);
            }
        }
        // Shell pattern: diamond grid overlay (scute pattern), animated bobble.
        let bob = ((ctx.time * 1.5).sin() * 0.5) as i32;
        for row in (0..h).step_by(3) {
            for col in (x0..x0 + shell_w).step_by(4) {
                draw::dot_i(grid, col as i32, row as i32 + bob);
            }
        }
        // Head and tail (only partially visible, emerge at sides).
        draw::dot_i(grid, x0 as i32 - 1, base as i32);
        draw::dot_i(grid, x0 as i32 - 2, base as i32);
        draw::dot_i(grid, (x0 + shell_w) as i32 + 1, base as i32);
        // Legs.
        draw::dot_i(grid, x0 as i32 + 1, base as i32 + 1);
        draw::dot_i(grid, (x0 + shell_w) as i32 - 1, base as i32 + 1);
        Ok(())
    }
}

// ── Ant March ─────────────────────────────────────────────────────────────────

struct AntMarch;
impl ProgressStyle for AntMarch {
    fn name(&self) -> &str {
        "ant-march"
    }
    fn theme(&self) -> &str {
        "animals"
    }
    fn describe(&self) -> &str {
        "Ants marching in single file, carrying the load — legs animate with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let n_ants = (w / 7).max(1).min(8);
        let base = (h - 1).min(h.saturating_sub(1));
        let head_x = (ctx.eased * w as f32) as usize;
        // Ant spacing: packed in the filled region.
        let spacing = if n_ants <= 1 {
            head_x.max(1)
        } else {
            head_x / n_ants.max(1)
        };
        // Leg animation: alternating sets swing with time.
        let leg_phase = ctx.time * 8.0;
        for i in 0..n_ants {
            let ant_x = if spacing == 0 { 0 } else { i * spacing };
            if ant_x >= head_x.max(1) {
                break;
            }
            let ant_x = ant_x.min(w.saturating_sub(3));
            // Alternate leg sets: even ants phase-A, odd ants phase-B.
            let ant_leg_phase = leg_phase + i as f32 * PI / n_ants as f32;
            let leg_up = (ant_leg_phase.sin() > 0.0) as usize;
            // Ant: head (1 dot), thorax (1 dot), abdomen (1 dot).
            // Head.
            draw::dot(grid, (ant_x + 2).min(w - 1), base.saturating_sub(2));
            // Antennae — flicker with leg_up.
            draw::dot_i(grid, ant_x as i32 + 1, base as i32 - 3);
            draw::dot_i(grid, ant_x as i32 + 3, base as i32 - 3 + leg_up as i32);
            // Thorax.
            draw::dot(grid, (ant_x + 2).min(w - 1), base.saturating_sub(1));
            // Abdomen.
            draw::dot(grid, (ant_x + 2).min(w - 1), base);
            // Legs (3 pairs): alternate up/down with phase.
            for leg in 0..3usize {
                let leg_y_off = if (leg + leg_up) % 2 == 0 { 0i32 } else { 1i32 };
                // Left leg.
                draw::dot_i(
                    grid,
                    ant_x as i32 + 1,
                    base as i32 - leg as i32 + leg_y_off - 1,
                );
                // Right leg.
                draw::dot_i(
                    grid,
                    ant_x as i32 + 3,
                    base as i32 - leg as i32 + leg_y_off - 1,
                );
            }
        }
        // Gradient tint on the ant column.
        let (cells_w, cells_h) = grid.dimensions();
        let ant_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx in 0..ant_cells.min(cells_w) {
                let t = if ant_cells <= 1 {
                    0.0
                } else {
                    cx as f32 / (ant_cells - 1) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}
