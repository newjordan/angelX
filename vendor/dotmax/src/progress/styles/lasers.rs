//! Laser / light-beam progress bars — eleven structurally distinct beam styles.
//!
//! Every style is stateless: all animation comes from `ctx.time` and all
//! charge / fill comes from `ctx.eased`. Lines are the medium — beams, sweeps,
//! fans, grids, and reflections. Color tints are additive; structure drives the
//! variety.
//!
//! Styles in this module:
//! 1. `charge-and-fire`     — core charges, then a beam lances the full width
//! 2. `scanning-line`       — a vertical sweep beam travels back and forth
//! 3. `security-grid`       — criss-crossing beams; tripped beams flicker
//! 4. `prism-dispersion`    — one beam fans into a spectrum of angled lines
//! 5. `laser-light-show`    — Lissajous beams crossing, time-animated
//! 6. `range-finder`        — rotating sweep with a target lock at eased position
//! 7. `mirror-bounce`       — beam reflects off walls; path length = eased
//! 8. `fiber-pulse`         — light pulses travel along curved fiber lines
//! 9. `plasma-bolt`         — jagged lightning-like beam jittered by time
//! 10. `particle-accelerator` — dots racing along a track, speed = eased
//! 11. `disco-fan`          — radial beams sweeping from a corner origin

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ─── deterministic hash helpers ─────────────────────────────────────────────

#[inline]
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

#[inline]
fn hashf(n: u32) -> f32 {
    (hash(n) % 1000) as f32 / 1000.0
}

// ─── step-bounded Bresenham line ─────────────────────────────────────────────

/// Draw a straight line from `(x0,y0)` to `(x1,y1)` in dot-space using
/// integer Bresenham. At most `max_steps` pixels are emitted so that a 1×1
/// grid can never loop a million times. `draw::dot_i` ignores OOB writes.
fn beam_line(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32, max_steps: usize) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut cx = x0;
    let mut cy = y0;
    let steps = (dx + dy + 2) as usize;
    let limit = steps.min(max_steps);
    for _ in 0..limit {
        draw::dot_i(grid, cx, cy);
        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = err * 2;
        if e2 > -dy {
            err -= dy;
            cx += sx;
        }
        if e2 < dx {
            err += dx;
            cy += sy;
        }
    }
}

// ─── public registry ─────────────────────────────────────────────────────────

/// All styles in the `lasers` theme.
///
/// Returns eleven laser / light-beam bars, each with a structurally distinct
/// beam topology. Progress (`ctx.eased`) drives charge, fill, or target
/// position; time (`ctx.time`) drives sweep, animation, and flicker.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(ChargeAndFire),
        Box::new(ScanningLine),
        Box::new(SecurityGrid),
        Box::new(PrismDispersion),
        Box::new(LaserLightShow),
        Box::new(RangeFinder),
        Box::new(MirrorBounce),
        Box::new(FiberPulse),
        Box::new(PlasmaBolt),
        Box::new(ParticleAccelerator),
        Box::new(DiscoFan),
    ]
}

// ─── 1. Charge and fire ──────────────────────────────────────────────────────

/// Core charges visibly as eased grows; at 100% a full-width beam fires.
struct ChargeAndFire;
impl ProgressStyle for ChargeAndFire {
    fn name(&self) -> &str {
        "charge-and-fire"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Core charges with eased; at full power a beam lances across the entire bar"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let mid = h / 2;
        let core_x = w / 6;

        // Phase: 0..0.85 = charging, 0.85..1.0 = fired
        let fired = ctx.eased >= 0.85;
        let charge = (ctx.eased / 0.85).clamp(0.0, 1.0);

        if fired {
            // Full horizontal beam across entire bar.
            for y in mid.saturating_sub(1)..=(mid + 1).min(h.saturating_sub(1)) {
                draw::hline(grid, 0, w.saturating_sub(1), y);
            }
            // Bright centre stripe.
            draw::hline(grid, 0, w.saturating_sub(1), mid);

            // Tint: intense palette end color across all cells.
            let color = ctx.palette.sample(1.0);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, 0, cells_w.saturating_sub(1), color);
            }
        } else {
            // Charging core: concentric rings growing outward from core_x.
            let rings = ((charge * 6.0) as usize).max(1).min(6);
            for r in 0..rings {
                let radius = r + 1;
                // Horizontal arms.
                for dr in 0..radius {
                    draw::dot_i(grid, core_x as i32 + dr as i32, mid as i32);
                    if core_x >= dr {
                        draw::dot_i(grid, core_x as i32 - dr as i32, mid as i32);
                    }
                }
                // Vertical arms.
                for dv in 0..radius {
                    draw::dot_i(grid, core_x as i32, mid as i32 + dv as i32);
                    draw::dot_i(grid, core_x as i32, mid as i32 - dv as i32);
                }
            }

            // Pulsing ring: flicker via time.
            let pulse_r = (charge * 4.0) as i32;
            if pulse_r > 0 {
                let steps = 32usize;
                for s in 0..steps {
                    let angle = s as f32 / steps as f32 * 2.0 * PI + ctx.time * 4.0;
                    // Squish vertically (braille dots are taller than wide).
                    let px = core_x as i32 + (angle.cos() * pulse_r as f32 * 1.5) as i32;
                    let py = mid as i32 + (angle.sin() * pulse_r as f32 * 0.7) as i32;
                    draw::dot_i(grid, px, py);
                }
            }

            // Charge beam lead: partial horizontal line toward the right.
            let beam_reach = ((charge * w as f32) as usize).min(w.saturating_sub(1));
            if beam_reach > core_x {
                draw::hline(grid, core_x, beam_reach, mid);
            }

            // Tint proportional to charge.
            let filled_cells = (charge * cells_w as f32) as usize;
            for cx in 0..filled_cells.min(cells_w) {
                let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
                let color = ctx.palette.sample(t);
                for cy in 0..cells_h {
                    draw::tint_row(grid, cy, cx, cx, color);
                }
            }
        }

        Ok(())
    }
}

// ─── 2. Scanning line ────────────────────────────────────────────────────────

/// A bright vertical beam sweeps left↔right; the swept region fills with eased.
struct ScanningLine;
impl ProgressStyle for ScanningLine {
    fn name(&self) -> &str {
        "scanning-line"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Vertical beam sweeps back and forth; swept fraction fills to eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Sweep: triangle wave in time.
        let period = 3.0f32;
        let t_norm = (ctx.time % period) / period; // 0..1
        let sweep_frac = if t_norm < 0.5 {
            t_norm * 2.0
        } else {
            2.0 - t_norm * 2.0
        };
        let sweep_x = (sweep_frac * w as f32) as usize;
        let sweep_x = sweep_x.min(w.saturating_sub(1));

        // Filled region up to eased progress: sparse horizontal lines.
        let filled = ((ctx.eased * w as f32) as usize).min(w);
        let mid = h / 2;
        // Three horizontal rails.
        draw::hline(
            grid,
            0,
            filled.saturating_sub(1),
            mid.saturating_sub(1).min(h - 1),
        );
        draw::hline(grid, 0, filled.saturating_sub(1), mid);
        draw::hline(
            grid,
            0,
            filled.saturating_sub(1),
            (mid + 1).min(h.saturating_sub(1)),
        );

        // Sweep beam: full-height vertical line with a narrow bright core.
        draw::vline(grid, sweep_x, 0, h.saturating_sub(1));
        // One-dot wings on each side for width.
        if sweep_x > 0 {
            draw::vline(grid, sweep_x - 1, h / 4, h * 3 / 4);
        }
        if sweep_x + 1 < w {
            draw::vline(grid, sweep_x + 1, h / 4, h * 3 / 4);
        }

        // Tint: filled region gets gradient; beam cell gets bright end.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        let beam_cell = (sweep_x / 2).min(cells_w.saturating_sub(1));
        let bright = ctx.palette.sample(1.0);
        for cy in 0..cells_h {
            draw::tint_row(grid, cy, beam_cell, beam_cell, bright);
        }

        Ok(())
    }
}

// ─── 3. Security grid ────────────────────────────────────────────────────────

/// Horizontal + vertical beams form a grid; beams tripped by the sweep flicker.
struct SecurityGrid;
impl ProgressStyle for SecurityGrid {
    fn name(&self) -> &str {
        "security-grid"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Criss-crossing security beams; tripped beams flicker dangerously via time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Grid spacing: number of beams scales with eased.
        let h_beams = ((ctx.eased * 5.0 + 1.0) as usize).min(8).max(1);
        let v_beams = ((ctx.eased * 3.0 + 1.0) as usize).min(6).max(1);

        // Horizontal beams.
        for i in 0..h_beams {
            let y = if h_beams <= 1 {
                h / 2
            } else {
                i * h.saturating_sub(1) / (h_beams - 1)
            };
            // Trip: a "tripwire" sweeper crosses at ctx.time.
            let trip_x = ((ctx.time * 0.4).fract() * w as f32) as usize;
            let tripped = trip_x < w / 2; // left half crossed = tripped
            let flicker_on = (ctx.time * 12.0 + i as f32 * 1.7).sin() > 0.0;

            if tripped && flicker_on {
                // Flicker: draw the beam in two broken halves.
                if trip_x > 0 {
                    draw::hline(grid, 0, trip_x.saturating_sub(1), y);
                }
                if trip_x + 2 < w {
                    draw::hline(grid, trip_x + 2, w.saturating_sub(1), y);
                }
            } else {
                draw::hline(grid, 0, w.saturating_sub(1), y);
            }
        }

        // Vertical beams.
        for j in 0..v_beams {
            let x = if v_beams <= 1 {
                w / 2
            } else {
                j * w.saturating_sub(1) / (v_beams - 1)
            };
            let on = (ctx.time * 8.0 + j as f32 * 2.3).cos() > -0.3;
            if on {
                draw::vline(grid, x, 0, h.saturating_sub(1));
            }
        }

        // Tint: map each cell column to palette gradient.
        for cx in 0..cells_w {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ─── 4. Prism dispersion ────────────────────────────────────────────────────

/// One incoming beam from the left fans into multiple angled beams via a prism.
struct PrismDispersion;
impl ProgressStyle for PrismDispersion {
    fn name(&self) -> &str {
        "prism-dispersion"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "One white beam enters a prism and fans into a spectrum of angled beams"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let prism_x = (w / 4) as i32;
        let mid = (h / 2) as i32;

        // Input beam: horizontal from left to prism.
        beam_line(grid, 0, mid, prism_x, mid, w + h);

        // Prism triangle outline.
        let tri_h = (h / 3).max(1) as i32;
        beam_line(
            grid,
            prism_x,
            mid - tri_h,
            prism_x + tri_h,
            mid + tri_h,
            w + h,
        );
        beam_line(grid, prism_x, mid - tri_h, prism_x, mid + tri_h, w + h);
        beam_line(
            grid,
            prism_x,
            mid + tri_h,
            prism_x + tri_h,
            mid + tri_h,
            w + h,
        );

        // Fanned output beams: spread angle increases with eased.
        let n_beams = ((ctx.eased * 7.0 + 1.0) as usize).max(1).min(8);
        let fan_origin_x = prism_x + tri_h;
        let fan_origin_y = mid;
        let spread = (ctx.eased * PI * 0.7).max(0.05);

        for b in 0..n_beams {
            // Angle: fan from -spread/2 to +spread/2.
            let angle = if n_beams == 1 {
                0.0f32
            } else {
                -spread / 2.0 + b as f32 / (n_beams - 1) as f32 * spread
            };

            // Length of each beam: reaches the right edge.
            let remain_w = (w as i32 - fan_origin_x).max(1);
            let end_x = fan_origin_x + remain_w;
            let end_y = fan_origin_y + (angle.tan() * remain_w as f32) as i32;

            beam_line(grid, fan_origin_x, fan_origin_y, end_x, end_y, (w + h) * 2);

            // Tint each beam with a palette sample.
            let t = b as f32 / n_beams.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            // Approximate the cell columns this beam passes through.
            let cx0 = (fan_origin_x / 2).max(0) as usize;
            let cx1 = (end_x / 2).clamp(0, cells_w as i32 - 1) as usize;
            for cy in 0..cells_h {
                draw::tint_row(
                    grid,
                    cy,
                    cx0.min(cx1),
                    cx0.max(cx1).min(cells_w.saturating_sub(1)),
                    color,
                );
            }
        }

        Ok(())
    }
}

// ─── 5. Laser light show ────────────────────────────────────────────────────

/// Multiple Lissajous beams cross; each traces a 1-D scan at different frequencies.
struct LaserLightShow;
impl ProgressStyle for LaserLightShow {
    fn name(&self) -> &str {
        "laser-light-show"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Multiple Lissajous sweep beams crossing at different frequencies; count = eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Number of beams active = eased * max.
        let n_active = ((ctx.eased * 6.0 + 1.0) as usize).min(7);

        // Each beam is a Lissajous-style scan: x scans uniformly, y = A*sin(f*x + phase).
        let freq_pairs: [(f32, f32); 7] = [
            (1.0, 2.0),
            (2.0, 3.0),
            (3.0, 4.0),
            (3.0, 5.0),
            (5.0, 6.0),
            (4.0, 7.0),
            (7.0, 8.0),
        ];

        let amp = (h as f32 / 2.0 - 1.0).max(0.5);
        let cy_mid = h as f32 / 2.0;

        for b in 0..n_active {
            let (fx, fy) = freq_pairs[b % freq_pairs.len()];
            let phase = hashf(b as u32 * 17) * 2.0 * PI + ctx.time * (0.5 + b as f32 * 0.15);

            // Sample beam as a sequence of dots along x.
            let steps = w * 2;
            for s in 0..=steps {
                let frac = s as f32 / steps as f32;
                let px = (frac * w as f32) as i32;
                let theta_x = frac * fx * PI * 2.0;
                let theta_y = frac * fy * PI * 2.0 + phase;
                let py = (cy_mid + amp * theta_y.sin() * (theta_x.cos() * 0.3 + 0.7)) as i32;
                draw::dot_i(grid, px, py);
            }

            // Tint with palette.
            let t = b as f32 / n_active.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, 0, cells_w.saturating_sub(1), color);
            }
        }

        Ok(())
    }
}

// ─── 6. Range finder ────────────────────────────────────────────────────────

/// A rotating radial sweep beam; target lock reticle at the eased radius.
struct RangeFinder;
impl ProgressStyle for RangeFinder {
    fn name(&self) -> &str {
        "range-finder"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Rotating sweep beam radiates from center; target lock reticle at eased radius"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let max_r = (w.min(h * 2) / 2) as f32;

        // Radar-style range rings (3 concentric).
        for ring in 1..=3usize {
            let ring_r = (ring as f32 / 3.0 * max_r) as i32;
            if ring_r == 0 {
                continue;
            }
            let steps = (ring_r * 8).max(16) as usize;
            for s in 0..steps {
                let angle = s as f32 / steps as f32 * 2.0 * PI;
                // Squish vertically: braille cells are ~2× taller than wide.
                let px = cx + (angle.cos() * ring_r as f32) as i32;
                let py = cy + (angle.sin() * ring_r as f32 * 0.5) as i32;
                // Sparse: only every 3rd dot.
                if s % 3 == 0 {
                    draw::dot_i(grid, px, py);
                }
            }
        }

        // Rotating sweep beam.
        let sweep_angle = ctx.time * 1.8; // ~1 revolution per 3.5 s
        let sweep_dx = sweep_angle.cos();
        let sweep_dy = sweep_angle.sin() * 0.5;
        let beam_end_x = cx + (sweep_dx * max_r) as i32;
        let beam_end_y = cy + (sweep_dy * max_r) as i32;
        beam_line(grid, cx, cy, beam_end_x, beam_end_y, w + h);

        // Ghost trail (slightly behind sweep).
        for ghost in 1..=4usize {
            let ga = sweep_angle - ghost as f32 * 0.12;
            let gx = cx + (ga.cos() * max_r) as i32;
            let gy = cy + (ga.sin() * 0.5 * max_r) as i32;
            // Step only every 2nd pixel.
            let dx = (gx - cx).abs();
            let dy = (gy - cy).abs();
            let steps = dx.max(dy).max(1) as usize;
            for s in (0..steps).step_by(2) {
                let t = s as f32 / steps as f32;
                let px = cx + ((gx - cx) as f32 * t) as i32;
                let py = cy + ((gy - cy) as f32 * t) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Target lock: cross-hair at eased radius along the sweep beam.
        let lock_r = ctx.eased * max_r;
        let lock_x = cx + (sweep_angle.cos() * lock_r) as i32;
        let lock_y = cy + (sweep_angle.sin() * 0.5 * lock_r) as i32;
        // Reticle: 4-dot cross.
        for d in 0..3i32 {
            draw::dot_i(grid, lock_x + d, lock_y);
            draw::dot_i(grid, lock_x - d, lock_y);
            draw::dot_i(grid, lock_x, lock_y + d);
            draw::dot_i(grid, lock_x, lock_y - d);
        }

        // Tint sweep sector with bright color.
        let color = ctx.palette.sample(0.8);
        let dim = ctx.palette.sample(0.2);
        for cx_c in 0..cells_w {
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, dim);
            }
        }
        let beam_cell = (beam_end_x / 2).clamp(0, cells_w as i32 - 1) as usize;
        let center_cell = (cx / 2).clamp(0, cells_w as i32 - 1) as usize;
        for cy_c in 0..cells_h {
            let lo = center_cell.min(beam_cell);
            let hi = center_cell.max(beam_cell);
            draw::tint_row(grid, cy_c, lo, hi, color);
        }

        Ok(())
    }
}

// ─── 7. Mirror bounce ───────────────────────────────────────────────────────

/// A beam enters from the left and bounces off walls; path length = eased.
struct MirrorBounce;
impl ProgressStyle for MirrorBounce {
    fn name(&self) -> &str {
        "mirror-bounce"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Beam enters left and reflects off top/bottom walls; total path length = eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Beam starts at left-middle, travels at a diagonal.
        // Angle modulated by time so the bounce pattern shifts.
        let angle_base = PI / 4.0 + (ctx.time * 0.2).sin() * PI / 8.0;
        // Float accumulators: an i32 position re-truncated each step would
        // never accumulate sub-dot motion and the beam would stand still.
        let mut fx = 0.0f32;
        let mut fy = h as f32 / 2.0;
        let dx = angle_base.cos();
        let mut dy = angle_base.sin();

        // Total dots to emit = eased * (w + several bounces worth).
        let total_dots = ((ctx.eased * (w * 6) as f32) as usize).max(1);
        let step = 1.0f32;

        for _ in 0..total_dots {
            fx += dx * step;
            fy += dy * step;

            // Reflect off top/bottom walls.
            if fy < 0.0 {
                fy = -fy;
                dy = -dy;
            } else if fy >= h as f32 {
                fy = 2.0 * h as f32 - fy - 2.0;
                dy = -dy;
            }
            // Wrap x so long beams keep bouncing across the bar.
            if fx >= w as f32 {
                fx -= w as f32;
            }

            let (bx, by) = (fx as i32, fy as i32);
            draw::dot_i(grid, bx, by);

            // Mark bounce point with a triple dot when direction reverses.
            if (dy > 0.0 && by <= 1) || (dy < 0.0 && by >= h as i32 - 2) {
                draw::dot_i(grid, bx - 1, by);
                draw::dot_i(grid, bx + 1, by);
            }
        }

        // Tint: gradient left to right.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ─── 8. Fiber pulse ─────────────────────────────────────────────────────────

/// Light pulses travel down multiple sinusoidal fiber lines; speed = eased.
struct FiberPulse;
impl ProgressStyle for FiberPulse {
    fn name(&self) -> &str {
        "fiber-pulse"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Pulses race down curved fiber-optic lines; pulse speed and fiber count = eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let n_fibers = ((ctx.eased * 6.0 + 1.0) as usize).max(1).min(7);
        let pulse_speed = 0.3 + ctx.eased * 2.5;

        for f in 0..n_fibers {
            // Each fiber is a sine wave with unique amplitude and frequency.
            let fiber_amp = (h as f32 / (n_fibers as f32 * 1.2 + 1.0)).max(0.5);
            let freq = 1.5 + f as f32 * 0.4;
            let phase_off = f as f32 * PI * 0.6;
            // Vertical offset so fibers don't all overlap.
            let v_off = if n_fibers == 1 {
                h as f32 / 2.0
            } else {
                (f as f32 + 0.5) * h as f32 / n_fibers as f32
            };

            // Draw the fiber path.
            for px in 0..w {
                let x_frac = px as f32 / w.saturating_sub(1).max(1) as f32;
                let py = v_off + fiber_amp * (x_frac * freq * 2.0 * PI + phase_off).sin();
                draw::dot_i(grid, px as i32, py as i32);
            }

            // Pulse position: travels left→right, wraps.
            let pulse_phase = (ctx.time * pulse_speed + f as f32 * 0.37).fract();
            let pulse_x = (pulse_phase * w as f32) as usize;

            // Draw a 7-dot wide bright pulse on the fiber at pulse_x.
            let pulse_half = 4usize;
            for dp in 0..=pulse_half * 2 {
                let ppx = pulse_x.saturating_sub(pulse_half) + dp;
                if ppx >= w {
                    break;
                }
                let x_frac = ppx as f32 / w.saturating_sub(1).max(1) as f32;
                let py = v_off + fiber_amp * (x_frac * freq * 2.0 * PI + phase_off).sin();
                // Intensity falls off from centre of pulse.
                let dist = (dp as i32 - pulse_half as i32).abs();
                if dist <= 2 {
                    // Core dot.
                    draw::dot_i(grid, ppx as i32, py as i32);
                    draw::dot_i(grid, ppx as i32, py as i32 - 1);
                    draw::dot_i(grid, ppx as i32, py as i32 + 1);
                } else {
                    draw::dot_i(grid, ppx as i32, py as i32);
                }
            }

            // Tint per fiber.
            let t = f as f32 / n_fibers.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, 0, cells_w.saturating_sub(1), color);
            }
        }

        Ok(())
    }
}

// ─── 9. Plasma bolt ─────────────────────────────────────────────────────────

/// A jagged lightning beam from left to right; jitter driven by time.
struct PlasmaBolt;
impl ProgressStyle for PlasmaBolt {
    fn name(&self) -> &str {
        "plasma-bolt"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Jagged plasma lightning beam jitters frame-by-frame via time; length = eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let bolt_len = ((ctx.eased * w as f32) as usize).max(1).min(w);
        let mid = h / 2;
        let jitter_epoch = (ctx.time * 20.0) as u32; // changes 20×/sec

        // Main bolt: walk left-to-right, each column's y is jittered.
        let mut prev_y = mid as i32;
        let max_jitter = ((h / 2) as i32).max(1);

        for px in 0..bolt_len {
            // Jitter: unique per (x, epoch).
            let jitter_raw = hashf(px as u32 * 7 + jitter_epoch * 31);
            let jitter = ((jitter_raw * 2.0 - 1.0) * max_jitter as f32) as i32;
            let mut cy = mid as i32 + jitter;
            cy = cy.clamp(0, h as i32 - 1);

            // Draw vertical segment between prev_y and cy (ensures continuity).
            let y_lo = prev_y.min(cy).clamp(0, h as i32 - 1) as usize;
            let y_hi = prev_y.max(cy).clamp(0, h as i32 - 1) as usize;
            draw::vline(grid, px, y_lo, y_hi);

            prev_y = cy;
        }

        // Secondary thinner bolt slightly offset in time.
        let jitter_epoch2 = jitter_epoch.wrapping_add(7);
        let mut prev_y2 = mid as i32;
        for px in 0..bolt_len {
            let jitter_raw = hashf(px as u32 * 13 + jitter_epoch2 * 41);
            let jitter = ((jitter_raw * 2.0 - 1.0) * (max_jitter / 2) as f32) as i32;
            let mut cy = mid as i32 + jitter;
            cy = cy.clamp(0, h as i32 - 1);
            draw::dot_i(grid, px as i32, cy);
            // Thinner: no vline, just the dot.
            prev_y2 = cy;
        }
        let _ = prev_y2;

        // Tint the bolt region.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ─── 10. Particle accelerator ───────────────────────────────────────────────

/// Dots race along a track; spacing and speed scale with eased.
struct ParticleAccelerator;
impl ProgressStyle for ParticleAccelerator {
    fn name(&self) -> &str {
        "particle-accelerator"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Charged particles accelerate along a beam track; speed and density = eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let mid = h / 2;

        // Track: double-rail lines.
        let rail_above = mid.saturating_sub(1);
        let rail_below = (mid + 1).min(h.saturating_sub(1));
        draw::hline(grid, 0, w.saturating_sub(1), rail_above);
        draw::hline(grid, 0, w.saturating_sub(1), rail_below);

        // Cross-ties every few dots.
        let tie_spacing = 5usize;
        let mut tx = 0;
        while tx < w {
            draw::vline(grid, tx, rail_above, rail_below);
            tx += tie_spacing;
        }

        // Particles: n_particles travel left→right at speed = eased.
        let speed = 0.4 + ctx.eased * 4.0;
        let n_particles = ((ctx.eased * 8.0 + 1.0) as usize).max(1).min(10);
        let particle_gap = w / n_particles.max(1);

        for p in 0..n_particles {
            let phase_off = p as f32 / n_particles as f32;
            // Position: wraps continuously.
            let pos = ((ctx.time * speed + phase_off).fract() * w as f32) as usize;
            let pos = pos.min(w.saturating_sub(1));

            // Particle core: 3-dot cluster on the centreline.
            draw::dot_i(grid, pos as i32, mid as i32);
            draw::dot_i(grid, pos as i32 + 1, mid as i32);
            if pos > 0 {
                draw::dot_i(grid, pos as i32 - 1, mid as i32);
            }

            // Wake: decreasing density behind the particle.
            let wake_len = (particle_gap / 2).max(2).min(w);
            for w_step in 1..wake_len {
                // Probability falls off with distance.
                if w_step * 3 < wake_len * 2 {
                    let wx = if pos >= w_step { pos - w_step } else { 0 };
                    draw::dot_i(grid, wx as i32, mid as i32);
                }
            }
        }

        // Tint: speed gradient.
        for cx in 0..cells_w {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased + (1.0 - ctx.eased) * 0.1);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ─── 11. Disco fan ──────────────────────────────────────────────────────────

/// Radial beams sweep from a corner origin; sweep angle grows with eased + time.
struct DiscoFan;
impl ProgressStyle for DiscoFan {
    fn name(&self) -> &str {
        "disco-fan"
    }
    fn theme(&self) -> &str {
        "lasers"
    }
    fn describe(&self) -> &str {
        "Radial fan of beams sweeps from a corner; beam count and arc = eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Fan origin: bottom-left corner.
        let ox = 0i32;
        let oy = h as i32 - 1;

        // Number of beams.
        let n_beams = ((ctx.eased * 9.0 + 1.0) as usize).max(1).min(10);

        // The fan sweeps continuously via time, occupying an arc that grows with eased.
        let arc = ctx.eased * PI * 0.9 + 0.1; // arc in radians, 0.1..~π·0.9
        let sweep_center = PI / 2.0 * (0.4 + 0.6 * ctx.eased) - (ctx.time * 0.6).sin() * arc * 0.3; // centre oscillates

        for b in 0..n_beams {
            let angle_frac = if n_beams == 1 {
                0.5f32
            } else {
                b as f32 / (n_beams - 1) as f32
            };
            let angle = sweep_center - arc / 2.0 + angle_frac * arc;

            // Beam extends to the far edge of the grid.
            let beam_len = ((w as f32).hypot(h as f32) as i32 + 2).max(2);
            let end_x = ox + (angle.cos() * beam_len as f32) as i32;
            let end_y = oy - (angle.sin() * beam_len as f32) as i32;

            beam_line(grid, ox, oy, end_x, end_y, (w + h) * 2);

            // Tint each beam column.
            let t = b as f32 / n_beams.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            // Paint a stripe along the beam.
            let x_end = end_x.clamp(0, cells_w as i32 - 1) as usize;
            let x_start = 0usize;
            let (lo, hi) = if x_start <= x_end {
                (x_start, x_end)
            } else {
                (x_end, x_start)
            };
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, lo, hi.min(cells_w.saturating_sub(1)), color);
            }
        }

        // Bright origin point.
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                draw::dot_i(grid, ox + dx, oy + dy);
            }
        }

        Ok(())
    }
}
