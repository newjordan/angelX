//! Space / cosmic progress bars — ten distinct styles from rocket launches
//! to black-hole accretion disks, all animated via `ctx.time` and driven by
//! `ctx.eased` for fill amount. Every style returns `"space"` from `theme()`.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Deterministic pseudo-random helper (no external crates).
// Returns a stable float in [0, 1) for star index `n`.
// ---------------------------------------------------------------------------
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

fn hash_f(n: u32) -> f32 {
    (hash(n) % 1000) as f32 / 1000.0
}

// ---------------------------------------------------------------------------
// Public registry
// ---------------------------------------------------------------------------

/// All styles in the `space` theme, in display order.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(RocketLaunch),
        Box::new(StarfieldWarp),
        Box::new(PlanetOrbit),
        Box::new(CometTail),
        Box::new(MoonPhase),
        Box::new(GalaxySpiral),
        Box::new(SatelliteDish),
        Box::new(SaturnRings),
        Box::new(BlackHole),
        Box::new(Constellation),
    ]
}

// ---------------------------------------------------------------------------
// 1 — Rocket launch
// ---------------------------------------------------------------------------

/// Rocket rises from left to right with eased body and a flickering exhaust plume.
struct RocketLaunch;
impl ProgressStyle for RocketLaunch {
    fn name(&self) -> &str {
        "rocket-launch"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Rocket advances with eased fill; exhaust flame flickers via time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Fuel trail — thin exhaust baseline
        let head = ((ctx.eased * w as f32) as usize).min(w.saturating_sub(1));
        let mid = h / 2;

        // Body: solid bar from 0..head
        draw::hline(grid, 0, head, mid);
        if h >= 3 {
            draw::hline(grid, 0, head, mid.saturating_sub(1));
        }

        // Tint the trail with palette gradient
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = if filled_cells <= 1 {
                0.0
            } else {
                cx as f32 / (filled_cells - 1) as f32
            };
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        // Rocket nose — vertical stripe at head position
        if head < w {
            draw::vline(
                grid,
                head,
                mid.saturating_sub(1),
                (mid + 1).min(h.saturating_sub(1)),
            );
        }

        // Exhaust plume behind the rocket: flicker via time + sine
        if head > 0 {
            let plume_len = (h / 2).max(1);
            for p in 0..plume_len {
                let flicker = (ctx.time * 18.0 + p as f32 * 1.3).sin();
                let offset = (flicker * 1.5) as i32;
                let px = head.saturating_sub(p + 1);
                draw::dot_i(grid, px as i32, (mid as i32) + offset);
                if p < plume_len / 2 {
                    draw::dot_i(grid, px as i32, (mid as i32) + offset + 1);
                    draw::dot_i(grid, px as i32, (mid as i32) + offset - 1);
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2 — Starfield warp
// ---------------------------------------------------------------------------

/// Stars streak outward from center, speed proportional to progress; density uses hash.
struct StarfieldWarp;
impl ProgressStyle for StarfieldWarp {
    fn name(&self) -> &str {
        "starfield-warp"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Stars streak from center outward; speed and streak length ramp with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as f32;
        let cy = (h / 2) as f32;
        let speed = 0.3 + ctx.eased * 2.5;
        let max_streak = (ctx.eased * 8.0 + 1.0) as usize;
        let num_stars: u32 = 64;

        for i in 0..num_stars {
            // Stable angle and base radius per star
            let angle = hash_f(i) * 2.0 * PI;
            let base_r = hash_f(i + 1000) * 0.5 + 0.1; // fraction of half-width
            let phase = hash_f(i + 2000); // time offset

            // Star travels outward; wrap via fract
            let r_frac = ((base_r + ctx.time * speed * 0.05 + phase).fract()).clamp(0.0, 1.0);
            let max_r = cx.min(cy);
            let r = r_frac * max_r;

            let sx = cx + angle.cos() * r;
            let sy = cy + angle.sin() * r * 0.6; // slightly squish vertically

            // Streak toward center (streak grows with progress + radius)
            let streak = ((r_frac * max_streak as f32) as usize).max(1);
            for s in 0..streak {
                let sr = (r - s as f32).max(0.0);
                let px = (cx + angle.cos() * sr) as i32;
                let py = (cy + angle.sin() * sr * 0.6) as i32;
                draw::dot_i(grid, px, py);
            }
            // Bright head
            draw::dot_i(grid, sx as i32, sy as i32);
        }

        // Tint with palette
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3 — Planet orbit
// ---------------------------------------------------------------------------

/// A planet traces an elliptical orbit; angle = eased * 2π with a trailing tail.
struct PlanetOrbit;
impl ProgressStyle for PlanetOrbit {
    fn name(&self) -> &str {
        "planet-orbit"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Planet travels an elliptical orbit; position = eased * 2π with orbital tail"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as f32;
        let cy = (h / 2) as f32;
        let rx = (w as f32 * 0.42).max(1.0);
        let ry = (h as f32 * 0.38).max(1.0);

        // Draw faint orbit ellipse (every 4th dot)
        let steps = 80usize;
        for s in 0..steps {
            let a = s as f32 / steps as f32 * 2.0 * PI;
            let ox = (cx + rx * a.cos()) as i32;
            let oy = (cy + ry * a.sin()) as i32;
            if s % 4 == 0 {
                draw::dot_i(grid, ox, oy);
            }
        }

        // Star / sun at center
        draw::dot_i(grid, cx as i32, cy as i32);
        draw::dot_i(grid, cx as i32 - 1, cy as i32);
        draw::dot_i(grid, cx as i32 + 1, cy as i32);
        draw::dot_i(grid, cx as i32, cy as i32 - 1);
        draw::dot_i(grid, cx as i32, cy as i32 + 1);

        // Planet angle driven by time for continuous animation, progress sets lap fraction
        let angle = ctx.time * 0.9 + ctx.eased * 2.0 * PI;

        // Trailing tail (10 ghost dots fading behind)
        let tail_len = 12usize;
        for t in 0..tail_len {
            let frac = (tail_len - t) as f32 / tail_len as f32;
            let ta = angle - (t as f32 * 0.15);
            let tx = (cx + rx * ta.cos()) as i32;
            let ty = (cy + ry * ta.sin()) as i32;
            // Sparser toward end of tail
            if hash(t as u32 * 7 + (ctx.time * 10.0) as u32) % 100 < (frac * 90.0) as u32 {
                draw::dot_i(grid, tx, ty);
            }
        }

        // Planet body (3-dot cluster)
        let px = (cx + rx * angle.cos()) as i32;
        let py = (cy + ry * angle.sin()) as i32;
        draw::dot_i(grid, px, py);
        draw::dot_i(grid, px + 1, py);
        draw::dot_i(grid, px, py + 1);
        draw::dot_i(grid, px + 1, py + 1);

        // Tint palette across cells
        let (cells_w, cells_h) = grid.dimensions();
        let color_start = ctx.palette.sample(0.0);
        let color_end = ctx.palette.sample(1.0);
        for cy_c in 0..cells_h {
            draw::tint_row(grid, cy_c, 0, cells_w / 2, color_start);
            draw::tint_row(
                grid,
                cy_c,
                cells_w / 2,
                cells_w.saturating_sub(1),
                color_end,
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4 — Comet with tail
// ---------------------------------------------------------------------------

/// Comet head at eased position; tail trails behind with decreasing dot density.
struct CometTail;
impl ProgressStyle for CometTail {
    fn name(&self) -> &str {
        "comet-tail"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Comet head at eased position; tail fades behind via decreasing dot density"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = h / 2;
        let head_x = ((ctx.eased * w as f32) as usize).min(w.saturating_sub(1));

        // Background stars (stable via hash)
        for i in 0u32..30 {
            let sx = (hash_f(i) * w as f32) as usize;
            let sy = (hash_f(i + 50) * h as f32) as usize;
            // Twinkle: occasionally skip
            if (hash(i + (ctx.time * 3.0) as u32) % 5) != 0 {
                draw::dot(grid, sx, sy);
            }
        }

        // Tail: comet path from 0 to head, density drops off from head toward origin
        let tail_len = head_x;
        for t in 0..tail_len {
            let dist_from_head = tail_len.saturating_sub(t);
            let frac = 1.0 - (dist_from_head as f32 / tail_len.max(1) as f32);
            // Probability of lighting a dot increases near the head
            let threshold = (frac * frac * 900.0) as u32;
            let roll = hash(
                (t as u32)
                    .wrapping_mul(31)
                    .wrapping_add((ctx.time * 5.0) as u32),
            ) % 1000;
            if roll < threshold {
                let spread = (frac * (h as f32 / 2.0)) as i32;
                let wobble = ((ctx.time * 4.0 + t as f32 * 0.2).sin() * spread as f32) as i32;
                draw::dot_i(grid, t as i32, mid as i32 + wobble);
                if spread > 1 {
                    draw::dot_i(grid, t as i32, mid as i32 + wobble - 1);
                }
            }
        }

        // Comet core — bright 3-dot head
        if head_x < w {
            draw::vline(
                grid,
                head_x,
                mid.saturating_sub(1),
                (mid + 1).min(h.saturating_sub(1)),
            );
            if head_x + 1 < w {
                draw::dot(grid, head_x + 1, mid);
            }
        }

        // Tint: warm orange→blue across the sweep
        let (cells_w, cells_h) = grid.dimensions();
        let head_cell = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..head_cell.min(cells_w) {
            let t = cx as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5 — Moon phase
// ---------------------------------------------------------------------------

/// Disc whose illuminated fraction equals eased; terminator sweeps like a moon phase.
struct MoonPhase;
impl ProgressStyle for MoonPhase {
    fn name(&self) -> &str {
        "moon-phase"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Circular disc illuminated by eased fraction — sweeps new→full moon"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let r = ((w.min(h) / 2).saturating_sub(1)).max(1) as i32;

        // Phase: 0 = new moon (none lit), 1 = full moon (all lit)
        // Terminator is a vertical line sweeping left→right across the disc.
        // At phase=0.5 it's at center (half-moon); phase=1 terminator is at +r (all lit).
        // Terminator x-offset: (eased * 2 - 1) * r  →  ranges -r..+r
        let term_x = ((ctx.eased * 2.0 - 1.0) * r as f32) as i32;

        for dy in -r..=r {
            // Half-chord at this row
            let dx_max_sq = r * r - dy * dy;
            if dx_max_sq < 0 {
                continue;
            }
            let dx_max = (dx_max_sq as f32).sqrt() as i32;
            for dx in -dx_max..=dx_max {
                // Lit side: right of terminator (dx >= term_x)
                if dx >= term_x {
                    draw::dot_i(grid, cx + dx, cy + dy);
                } else {
                    // Dark side: draw sparse dots for the disc outline only
                    if dx == -dx_max || dy == -r || dy == r {
                        draw::dot_i(grid, cx + dx, cy + dy);
                    }
                }
            }
        }

        // Tint lit portion with pale yellow→white from palette
        let (cells_w, cells_h) = grid.dimensions();
        let lit_cells = (ctx.eased * cells_w as f32) as usize;
        let start_cell = cells_w.saturating_sub(lit_cells);
        for cy_c in 0..cells_h {
            draw::tint_row(
                grid,
                cy_c,
                start_cell,
                cells_w.saturating_sub(1),
                ctx.palette.sample(0.85),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6 — Galaxy spiral
// ---------------------------------------------------------------------------

/// Logarithmic spiral arms; dots lit up to eased fraction, arms rotate with time.
struct GalaxySpiral;
impl ProgressStyle for GalaxySpiral {
    fn name(&self) -> &str {
        "galaxy-spiral"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Two-armed logarithmic galaxy spiral rotating in time; arms fill to eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as f32;
        let cy = (h / 2) as f32;
        let max_r = cx.min(cy * 1.4);

        // Logarithmic spiral: r = a * e^(b * theta)
        let a: f32 = 0.8;
        let b: f32 = 0.18;
        let num_arms = 2usize;
        let arm_steps = 120usize;
        let lit_steps = (ctx.eased * arm_steps as f32) as usize;
        let rot = ctx.time * 0.3; // slow rotation

        for arm in 0..num_arms {
            let arm_offset = arm as f32 * PI; // 180° apart
            for s in 0..lit_steps.min(arm_steps) {
                let theta = s as f32 / arm_steps as f32 * 4.0 * PI;
                let r = a * (b * theta).exp();
                if r > max_r {
                    break;
                }
                let angle = theta + arm_offset + rot;
                let px = (cx + r * angle.cos()) as i32;
                // Squish vertically to look more natural in wide terminals
                let py = (cy + r * angle.sin() * 0.55) as i32;
                draw::dot_i(grid, px, py);
                // Occasional scatter dot around arm
                if hash((s as u32).wrapping_add(arm as u32 * 500)) % 5 == 0 {
                    let scatter_a = angle + hash_f((s * 3 + arm * 999) as u32) * 0.4 - 0.2;
                    let scatter_r = r * (0.8 + hash_f((s * 7 + arm * 777) as u32) * 0.4);
                    let spx = (cx + scatter_r * scatter_a.cos()) as i32;
                    let spy = (cy + scatter_r * scatter_a.sin() * 0.55) as i32;
                    draw::dot_i(grid, spx, spy);
                }
            }
        }

        // Bright galactic core
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                draw::dot_i(grid, cx as i32 + dx, cy as i32 + dy);
            }
        }

        // Tint: deep blue→purple across all cells
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7 — Satellite dish receiving signal
// ---------------------------------------------------------------------------

/// Parabolic dish with concentric arcs pulsing outward via time; fill = eased.
struct SatelliteDish;
impl ProgressStyle for SatelliteDish {
    fn name(&self) -> &str {
        "satellite-dish"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Parabolic dish with signal arcs pulsing outward; strength driven by eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Dish: left-anchored parabola
        let dish_cx = (w / 4) as i32;
        let dish_cy = (h / 2) as i32;
        let dish_r = ((h / 2).saturating_sub(1)).max(2) as i32;

        // Draw parabolic bowl: x = cx - (dy^2 / dish_r)
        for dy in -dish_r..=dish_r {
            let dx = (dy * dy) / dish_r.max(1);
            let px = dish_cx - dx;
            let py = dish_cy + dy;
            draw::dot_i(grid, px, py);
        }
        // Stem
        draw::vline(
            grid,
            dish_cx as usize,
            (dish_cy + dish_r) as usize,
            (dish_cy + dish_r + 2).min(h.saturating_sub(1) as i32) as usize,
        );

        // Signal arcs emanating from focal point to the right
        let focal_x = dish_cx + dish_r / 2 + 1;
        let focal_y = dish_cy;
        let num_arcs = (ctx.eased * 5.0 + 1.0) as usize;
        let phase = ctx.time * 2.5;

        for arc_idx in 0..num_arcs.min(5) {
            // Each arc scrolls outward; wrap with fract
            let arc_phase = (phase + arc_idx as f32 * 0.7).fract();
            let arc_r = (arc_phase * (w as f32 * 0.55)) as i32;
            let arc_r = arc_r.max(1);

            // Draw a quarter-circle arc opening to the right
            let arc_steps = 20usize;
            for s in 0..arc_steps {
                let a = (s as f32 / arc_steps as f32 - 0.5) * PI; // -π/2 .. +π/2
                let px = focal_x + (arc_r as f32 * a.cos()) as i32;
                let py = focal_y + (arc_r as f32 * a.sin()) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Tint signal region with palette
        let (cells_w, cells_h) = grid.dimensions();
        let signal_cells = (ctx.eased * cells_w as f32) as usize;
        let dish_cell = cells_w / 4;
        for cx_c in dish_cell..signal_cells.min(cells_w) {
            let t = (cx_c - dish_cell) as f32 / (cells_w - dish_cell).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8 — Saturn rings filling
// ---------------------------------------------------------------------------

/// Planet disc surrounded by rings that fill in from eased; ring tilt animated.
struct SaturnRings;
impl ProgressStyle for SaturnRings {
    fn name(&self) -> &str {
        "saturn-rings"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Saturn disc with concentric rings filling from inner to outer as eased grows"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let planet_r = ((h / 2).saturating_sub(2)).max(1) as i32;

        // Planet body
        for dy in -planet_r..=planet_r {
            let dx_max = ((planet_r * planet_r - dy * dy) as f32).sqrt() as i32;
            for dx in -dx_max..=dx_max {
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Rings: ellipses with increasing radii, tilt oscillates via time
        let num_rings = 5usize;
        let tilt = 0.25 + 0.08 * (ctx.time * 0.4).sin(); // vertical compression
        let max_ring_r = (w / 2).saturating_sub(1) as i32;
        let rings_lit = (ctx.eased * num_rings as f32).ceil() as usize;

        for ring in 0..rings_lit.min(num_rings) {
            let frac = (ring + 1) as f32 / num_rings as f32;
            let ring_rx = (planet_r + 2 + (frac * (max_ring_r - planet_r - 2) as f32) as i32)
                .max(planet_r + 1);
            let ring_ry = (ring_rx as f32 * tilt) as i32;
            let ring_ry = ring_ry.max(1);

            // Partial ring: last ring fills to eased sub-fraction
            let lit_frac = if ring + 1 == rings_lit {
                let inner_frac = ctx.eased * num_rings as f32 - ring as f32;
                inner_frac.clamp(0.0, 1.0)
            } else {
                1.0
            };
            let ring_steps = 80usize;
            let lit_steps = (lit_frac * ring_steps as f32) as usize;

            for s in 0..lit_steps.min(ring_steps) {
                let a = s as f32 / ring_steps as f32 * 2.0 * PI;
                let px = cx + (ring_rx as f32 * a.cos()) as i32;
                let py = cy + (ring_ry as f32 * a.sin()) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Tint: rings in palette color
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9 — Black hole accretion disk
// ---------------------------------------------------------------------------

/// Event horizon with swirling accretion disk; disk rotates with time, fills with eased.
struct BlackHole;
impl ProgressStyle for BlackHole {
    fn name(&self) -> &str {
        "black-hole"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Black-hole event horizon with swirling accretion disk rotating via time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let horizon_r = ((h / 2).saturating_sub(2)).max(1) as i32;

        // Event horizon: solid black circle (we draw its outline only — interior stays empty)
        let eh_steps = 64usize;
        for s in 0..eh_steps {
            let a = s as f32 / eh_steps as f32 * 2.0 * PI;
            let px = cx + (horizon_r as f32 * a.cos()) as i32;
            let py = cy + (horizon_r as f32 * a.sin() * 0.55) as i32;
            draw::dot_i(grid, px, py);
        }

        // Accretion disk: multiple rings outside the horizon
        let disk_rings = 4usize;
        let disk_lit = (ctx.eased * disk_rings as f32).ceil() as usize;
        let rot = ctx.time * 1.2;

        for ring in 0..disk_lit.min(disk_rings) {
            let frac = (ring + 1) as f32 / disk_rings as f32;
            let ring_rx = horizon_r + 2 + (frac * (w as f32 * 0.3)) as i32;
            let ring_ry = (ring_rx as f32 * 0.3).max(1.0) as i32;

            let lit_frac = if ring + 1 == disk_lit {
                (ctx.eased * disk_rings as f32 - ring as f32).clamp(0.0, 1.0)
            } else {
                1.0
            };

            let disk_steps = 72usize;
            let lit_steps = (lit_frac * disk_steps as f32) as usize;
            for s in 0..lit_steps.min(disk_steps) {
                // Spiral: angle offset by ring index to create swirl effect
                let a = s as f32 / disk_steps as f32 * 2.0 * PI + rot + ring as f32 * 0.4;
                // Wavy radius: breathing effect
                let r_vary = 1.0 + 0.12 * (a * 3.0 + ctx.time * 2.0).sin();
                let px = cx + (ring_rx as f32 * r_vary * a.cos()) as i32;
                let py = cy + (ring_ry as f32 * r_vary * a.sin()) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Tint: hot orange/red near hole, fading outward
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            // Map to distance from center
            let dist = (cx_c as f32 - cells_w as f32 / 2.0).abs() / (cells_w as f32 / 2.0);
            let t = 1.0 - dist;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10 — Constellation
// ---------------------------------------------------------------------------

/// Fixed star positions; edges connect one by one as eased * N edges grow.
struct Constellation;
impl ProgressStyle for Constellation {
    fn name(&self) -> &str {
        "constellation"
    }
    fn theme(&self) -> &str {
        "space"
    }
    fn describe(&self) -> &str {
        "Star constellation: edges connect one by one as eased fraction grows; stars twinkle"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // 12 stable star positions (hash-seeded, unit fractions)
        const NUM_STARS: u32 = 12;
        let stars: Vec<(i32, i32)> = (0..NUM_STARS)
            .map(|i| {
                let sx = (hash_f(i) * (w as f32 - 2.0) + 1.0) as i32;
                let sy = (hash_f(i + 100) * (h as f32 - 2.0) + 1.0) as i32;
                (sx, sy)
            })
            .collect();

        // Adjacency: connect star i to star (i+1) % N and (i+3) % N for visual variety
        let edges: Vec<(usize, usize)> = (0..NUM_STARS as usize)
            .flat_map(|i| {
                vec![
                    (i, (i + 1) % NUM_STARS as usize),
                    (i, (i + 3) % NUM_STARS as usize),
                ]
            })
            .collect();

        let edges_lit = (ctx.eased * edges.len() as f32) as usize;

        // Draw edges that are revealed so far
        for (a, b) in edges.iter().take(edges_lit) {
            let (ax, ay) = stars[*a];
            let (bx, by) = stars[*b];
            // Bresenham-lite: step along the longer axis
            let dx = (bx - ax).abs();
            let dy = (by - ay).abs();
            let steps = dx.max(dy).max(1);
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let px = ax + ((bx - ax) as f32 * t) as i32;
                let py = ay + ((by - ay) as f32 * t) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Draw stars with twinkle: occasionally skip a dot based on time+index
        for (i, (sx, sy)) in stars.iter().enumerate() {
            let twinkle = (hash((i as u32).wrapping_add((ctx.time * 4.0) as u32)) % 6) != 0;
            if twinkle {
                draw::dot_i(grid, *sx, *sy);
                // Larger star cross
                draw::dot_i(grid, sx + 1, *sy);
                draw::dot_i(grid, sx - 1, *sy);
                draw::dot_i(grid, *sx, sy + 1);
                draw::dot_i(grid, *sx, sy - 1);
            }
        }

        // Tint revealed edges with gradient
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx_c in 0..filled_cells.min(cells_w) {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}
