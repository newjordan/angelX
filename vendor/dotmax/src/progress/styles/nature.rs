//! Nature / weather / seasons progress bars.
//!
//! Ten animated styles built entirely from `draw::` helpers. Every bar reads
//! `ctx.eased` for its fill/growth amount and `ctx.time` for looping
//! animation, so they look alive even when progress is held constant.
//! Palette tinting is applied via `draw::tint_row`.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ── registry ─────────────────────────────────────────────────────────────────

/// All styles in the `nature` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(GrowingVine),
        Box::new(GrassBlade),
        Box::new(Sunrise),
        Box::new(RainGauge),
        Box::new(TreeRings),
        Box::new(MountainSnow),
        Box::new(FourSeasons),
        Box::new(FlowerBloom),
        Box::new(FallingLeaves),
        Box::new(LightningBolt),
    ]
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Integer sine rounded to i32 — keeps per-bar code terse.
#[inline]
fn isin(angle: f32, amplitude: f32) -> i32 {
    (angle.sin() * amplitude).round() as i32
}

// ── 1. Growing vine ──────────────────────────────────────────────────────────

struct GrowingVine;
impl ProgressStyle for GrowingVine {
    fn name(&self) -> &str {
        "growing-vine"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "A vine climbs left-to-right; leaf crosses sprout at every 10% milestone"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = (h / 2) as i32;
        let reach = (ctx.eased * w as f32).round() as usize;

        // Sine-swaying stem.
        for x in 0..reach {
            let sway = isin(x as f32 * 0.3 + ctx.time * 2.0, (h as f32 * 0.2).max(1.0));
            draw::dot_i(grid, x as i32, mid + sway);
        }

        // Leaf crosses at each completed 10% threshold.
        for step in 1..=10usize {
            let threshold = step as f32 * 0.1;
            if ctx.eased < threshold {
                break;
            }
            let lx = (threshold * w as f32).round() as i32;
            let sway = isin(lx as f32 * 0.3 + ctx.time * 2.0, (h as f32 * 0.2).max(1.0));
            let ly = mid + sway;
            // Small cross: horizontal + vertical arms of length 2.
            for dx in -2i32..=2 {
                draw::dot_i(grid, lx + dx, ly);
            }
            for dy in -2i32..=2 {
                draw::dot_i(grid, lx, ly + dy);
            }
        }

        // Tint the filled span green.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        let color = ctx.palette.sample(ctx.eased);
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, filled_cells.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 2. Grass blades ──────────────────────────────────────────────────────────

struct GrassBlade;
impl ProgressStyle for GrassBlade {
    fn name(&self) -> &str {
        "grass-blades"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "Individual grass blades grow taller with progress and sway in the wind"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let blade_spacing = 3usize.max(1);
        let base_y = h.saturating_sub(1);
        let max_growth = h.saturating_sub(1);

        let blade_count = w / blade_spacing;
        for b in 0..blade_count {
            let bx = b * blade_spacing + blade_spacing / 2;
            // Each blade grows to a slightly different height (pseudo-random via hash).
            let variety = ((b as f32 * 7.3).sin() * 0.5 + 0.5) * 0.4 + 0.6; // 0.6..1.0
            let height = (ctx.eased * max_growth as f32 * variety).round() as usize;
            if height == 0 {
                continue;
            }

            // Sway: low-frequency sine, each blade phase-shifted by position.
            let sway = isin(ctx.time * 1.8 + bx as f32 * 0.5, (h as f32 * 0.12).max(1.0));
            let tip_x = (bx as i32 + sway).max(0) as usize;

            let top_y = base_y.saturating_sub(height);
            // Stem.
            for y in top_y..=base_y {
                let frac = (base_y - y) as f32 / height.max(1) as f32;
                let sx = (bx as f32 + sway as f32 * frac).round() as usize;
                draw::dot(grid, sx.min(w.saturating_sub(1)), y);
            }
            // Tip dot — slightly to the side.
            draw::dot(grid, tip_x.min(w.saturating_sub(1)), top_y);
        }

        // Gradient tint.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let t = if cw <= 1 {
                0.0
            } else {
                cx as f32 / (cw - 1) as f32
            };
            let color = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ── 3. Sunrise ───────────────────────────────────────────────────────────────

struct Sunrise;
impl ProgressStyle for Sunrise {
    fn name(&self) -> &str {
        "sunrise"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "A sun disc rises along an arc; the horizon glows as it climbs"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Horizon line at the bottom row.
        draw::hline(grid, 0, w.saturating_sub(1), h.saturating_sub(1));

        // Sun arc: progress 0 = right horizon, progress 1 = zenith/left horizon apex.
        // We map eased in [0, 1] → angle in [0, PI], where:
        //   angle=0   → cos=1,  sin=0 → sun at right horizon
        //   angle=PI/2 → cos=0, sin=1 → sun at zenith (top centre)
        //   angle=PI  → cos=-1, sin=0 → sun at left horizon
        // To keep the sun HIGH at full progress we clamp the arc to [0, PI/2]
        // (dawn to noon) so t=1 means the sun is at its peak, not setting again.
        let cx = (w / 2) as i32;
        let base_y = h.saturating_sub(2) as i32;
        let arc_rx = (w as f32 * 0.38) as i32;
        let arc_ry = h.saturating_sub(2).max(1) as i32;

        // Map progress 0→1 to angle PI→0 (right horizon rises to zenith and beyond).
        // Clamp to [0, PI] so at t=1 the sun rests at the top-left.
        let angle = (1.0 - ctx.eased) * PI;
        let sun_x = cx + (arc_rx as f32 * angle.cos()) as i32;
        // sin is always ≥ 0 for angle in [0, PI], so sun is always above or at horizon.
        let sun_y = base_y - (arc_ry as f32 * angle.sin()) as i32;

        // Draw sun as a small filled disc (radius 2 in dot-space).
        let r = 2i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r + 1 {
                    draw::dot_i(grid, sun_x + dx, sun_y + dy);
                }
            }
        }

        // Rays: 8 short lines radiating from sun center, animated with slow rotation.
        let ray_len = 4i32;
        for k in 0..8 {
            let ray_angle = (k as f32 / 8.0) * 2.0 * PI + ctx.time * 0.4;
            let rx = (ray_angle.cos() * (r + ray_len) as f32) as i32;
            let ry = (ray_angle.sin() * (r + ray_len) as f32) as i32;
            draw::dot_i(grid, sun_x + rx, sun_y + ry);
            draw::dot_i(grid, sun_x + rx / 2, sun_y + ry / 2);
        }

        // Sky tint: palette from bottom to top across all cells.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let sky_t = 1.0 - cy as f32 / ch.saturating_sub(1).max(1) as f32;
            let tint_t = ctx.eased * sky_t;
            let color = ctx.palette.sample(tint_t);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 4. Rain gauge ────────────────────────────────────────────────────────────

struct RainGauge;
impl ProgressStyle for RainGauge {
    fn name(&self) -> &str {
        "rain-gauge"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "Raindrops fall from a cloud; the water level rises with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Gauge outline.
        draw::rect_outline(grid, 0, 0, w, h);

        // Water fill from the bottom up.
        let fill_h = (ctx.eased * (h.saturating_sub(2)) as f32).round() as usize;
        let water_top = h.saturating_sub(1 + fill_h);
        if fill_h > 0 {
            draw::fill_rect(grid, 1, water_top, w.saturating_sub(2).max(1), fill_h);
        }

        // Animated raindrops — 5 drops cycling at different rates.
        let unfilled_h = water_top;
        if unfilled_h > 1 {
            for d in 0..5usize {
                let phase = d as f32 / 5.0;
                let drop_cycle = (ctx.time * 1.5 + phase).fract();
                let drop_y = (drop_cycle * unfilled_h as f32) as usize;
                let drop_x =
                    (((d as f32 * 1.618 + 0.5) % 1.0) * (w.saturating_sub(2)) as f32) as usize + 1;
                // Draw as a 1-dot pip.
                draw::dot(
                    grid,
                    drop_x.min(w.saturating_sub(2)),
                    drop_y.min(unfilled_h.saturating_sub(1)),
                );
            }
        }

        // Tint water blue via palette.
        let (cw, ch) = grid.dimensions();
        let water_top_cell = (water_top / 4).min(ch.saturating_sub(1));
        for cy in water_top_cell..ch {
            let t = (cy - water_top_cell) as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 5. Tree rings ────────────────────────────────────────────────────────────

struct TreeRings;
impl ProgressStyle for TreeRings {
    fn name(&self) -> &str {
        "tree-rings"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "Concentric growth rings expand outward from the center as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let max_rings = 6usize;
        let max_rx = (w / 2).saturating_sub(1) as f32;
        let max_ry = (h / 2).saturating_sub(1) as f32;

        let rings_visible = (ctx.eased * max_rings as f32).ceil() as usize;

        for ring in 1..=rings_visible.min(max_rings) {
            // Partial reveal on the outermost ring.
            let ring_frac = if ring < rings_visible {
                1.0f32
            } else {
                let base = (ring - 1) as f32 / max_rings as f32;
                let span = 1.0 / max_rings as f32;
                ((ctx.eased - base) / span).clamp(0.0, 1.0)
            };

            let rx = (max_rx * ring as f32 / max_rings as f32) as i32;
            let ry = (max_ry * ring as f32 / max_rings as f32) as i32;
            let rx = rx.max(1);
            let ry = ry.max(1);

            // Parametric ellipse, draw fraction based on ring_frac.
            let steps = 120usize;
            let arc_steps = (steps as f32 * ring_frac).round() as usize;
            for s in 0..arc_steps {
                let a = s as f32 / steps as f32 * 2.0 * PI;
                let ex = cx + (rx as f32 * a.cos()).round() as i32;
                let ey = cy + (ry as f32 * a.sin()).round() as i32;
                draw::dot_i(grid, ex, ey);
            }
        }

        // Center dot, always present.
        draw::dot_i(grid, cx, cy);

        // Radial tint.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 6. Mountain snow ─────────────────────────────────────────────────────────

struct MountainSnow;
impl ProgressStyle for MountainSnow {
    fn name(&self) -> &str {
        "mountain-snow"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "A mountain silhouette fills with snow creeping down from the peaks"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Mountain profile: triangle with roughened ridgeline.
        let peak_x = w / 2;
        let base_y = h.saturating_sub(1);

        // Render silhouette: for each x, calculate mountain height.
        let mountain_h = |x: usize| -> usize {
            let dist = if x <= peak_x { peak_x - x } else { x - peak_x };
            let slope = 1.0 - dist as f32 / peak_x.max(1) as f32;
            // Add ridge roughness.
            let roughness = ((x as f32 * 0.4).sin() * 0.08 + (x as f32 * 0.7).cos() * 0.05) * slope;
            let mh = (slope + roughness).clamp(0.0, 1.0);
            (mh * h as f32).round() as usize
        };

        // Draw mountain outline and fill.
        for x in 0..w {
            let mh = mountain_h(x);
            if mh == 0 {
                continue;
            }
            let top_y = base_y.saturating_sub(mh);
            draw::vline(grid, x, top_y, base_y);
        }

        // Snow: fill from peak downward by eased fraction.
        for x in 0..w {
            let mh = mountain_h(x);
            if mh == 0 {
                continue;
            }
            let snow_h = (ctx.eased * mh as f32).round() as usize;
            if snow_h == 0 {
                continue;
            }
            // Snowline starts at the top and descends.
            let top_y = base_y.saturating_sub(mh);
            let snow_bottom = (top_y + snow_h).min(base_y);
            // Overdraw snow with solid fill (same dots, but tinted below).
            draw::vline(grid, x, top_y, snow_bottom);
        }

        // Snow-cap tint via palette (cool blue-white at top, warm at base).
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let height_frac = 1.0 - cy as f32 / ch.saturating_sub(1).max(1) as f32;
            let t = height_frac * ctx.eased;
            let color = ctx.palette.sample(1.0 - t);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 7. Four seasons ──────────────────────────────────────────────────────────

struct FourSeasons;
impl ProgressStyle for FourSeasons {
    fn name(&self) -> &str {
        "four-seasons"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "The fill sweeps through spring, summer, autumn, and winter colour bands"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let filled = (ctx.eased * w as f32).round() as usize;

        // Solid fill dots first.
        draw::fill_rect(grid, 0, 0, filled, h);

        // Season tinting: each quarter of the fill is a distinct season colour.
        // Spring: green (0–0.25), Summer: gold (0.25–0.5),
        // Autumn: burnt orange (0.5–0.75), Winter: icy blue (0.75–1.0).
        use crate::Color;
        let season_colors: [(Color, Color); 4] = [
            (Color::rgb(34, 139, 34), Color::rgb(144, 238, 144)), // spring
            (Color::rgb(218, 165, 32), Color::rgb(255, 215, 0)),  // summer
            (Color::rgb(160, 82, 45), Color::rgb(205, 133, 63)),  // autumn
            (Color::rgb(70, 130, 180), Color::rgb(173, 216, 230)), // winter
        ];

        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let frac = cx as f32 / cw.saturating_sub(1).max(1) as f32;
            // Which season are we in?
            let season_idx = ((frac * 4.0) as usize).min(3);
            let season_t = (frac * 4.0).fract();
            // Animate a subtle brightness pulse within each season.
            let pulse = (ctx.time * 1.2 + frac * 2.0 * PI).sin() * 0.08 + 0.92; // 0.84..1.0
            let (s_col, e_col) = season_colors[season_idx];
            let r = super::super::lerp(s_col.r as f32, e_col.r as f32, season_t) * pulse;
            let g = super::super::lerp(s_col.g as f32, e_col.g as f32, season_t) * pulse;
            let b = super::super::lerp(s_col.b as f32, e_col.b as f32, season_t) * pulse;
            let color = Color::rgb(
                r.clamp(0.0, 255.0) as u8,
                g.clamp(0.0, 255.0) as u8,
                b.clamp(0.0, 255.0) as u8,
            );
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        // Track outline.
        draw::hline(grid, 0, w.saturating_sub(1), 0);
        draw::hline(grid, 0, w.saturating_sub(1), h.saturating_sub(1));

        Ok(())
    }
}

// ── 8. Flower bloom ──────────────────────────────────────────────────────────

struct FlowerBloom;
impl ProgressStyle for FlowerBloom {
    fn name(&self) -> &str {
        "flower-bloom"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "Petals extend radially from the centre as the flower blooms with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let petals = 8usize;
        // Max petal length — limited by whichever dimension is smaller.
        let max_len = (w.min(h) / 2).saturating_sub(1) as f32;

        for p in 0..petals {
            let base_angle = (p as f32 / petals as f32) * 2.0 * PI;
            // Petals slowly rotate with time.
            let angle = base_angle + ctx.time * 0.3;
            let petal_len = (ctx.eased * max_len).max(0.0);

            // Draw petal as a line from centre outward.
            let steps = petal_len.round() as usize;
            for s in 0..=steps {
                let r = s as f32;
                let px = cx + (angle.cos() * r).round() as i32;
                let py = cy + (angle.sin() * r).round() as i32;
                draw::dot_i(grid, px, py);
            }

            // Small bulge at the petal tip (two side dots).
            if petal_len >= 2.0 {
                let side_angle = angle + PI / 2.0;
                let tip_x = cx + (angle.cos() * petal_len).round() as i32;
                let tip_y = cy + (angle.sin() * petal_len).round() as i32;
                draw::dot_i(
                    grid,
                    tip_x + side_angle.cos().round() as i32,
                    tip_y + side_angle.sin().round() as i32,
                );
                draw::dot_i(
                    grid,
                    tip_x - side_angle.cos().round() as i32,
                    tip_y - side_angle.sin().round() as i32,
                );
            }
        }

        // Centre dot always present.
        draw::dot_i(grid, cx, cy);
        draw::dot_i(grid, cx + 1, cy);
        draw::dot_i(grid, cx, cy + 1);

        // Tint radially: warm centre → cool petals.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 9. Falling leaves ────────────────────────────────────────────────────────

struct FallingLeaves;
impl ProgressStyle for FallingLeaves {
    fn name(&self) -> &str {
        "falling-leaves"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "Autumn leaves drift downward with parabolic + sine sway, density grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Number of leaves grows with progress (min 2, max 12).
        let leaf_count = (2.0 + ctx.eased * 10.0).round() as usize;

        for leaf in 0..leaf_count {
            // Each leaf has a fixed "lane" x and an individual fall phase.
            let phase = leaf as f32 / leaf_count as f32;
            let cycle = (ctx.time * 0.7 + phase).fract();

            // Horizontal position: sine drift across the width.
            let origin_x = (phase * (w.saturating_sub(4)) as f32) as i32 + 2;
            let sway = isin(
                cycle * 2.0 * PI * 1.5 + phase * 3.0,
                (w as f32 * 0.07).max(2.0),
            );
            let lx = (origin_x + sway).clamp(0, w.saturating_sub(1) as i32);

            // Vertical: linear fall, wraps.
            let ly = (cycle * h as f32) as i32;

            // Leaf shape: a small cross (2-wide).
            draw::dot_i(grid, lx, ly);
            draw::dot_i(grid, lx + 1, ly);
            draw::dot_i(grid, lx, ly + 1);
            draw::dot_i(grid, lx - 1, ly);
        }

        // Autumn gradient tint via palette.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased + (1.0 - ctx.eased) * 0.5);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 10. Lightning bolt ───────────────────────────────────────────────────────

struct LightningBolt;
impl ProgressStyle for LightningBolt {
    fn name(&self) -> &str {
        "lightning-bolt"
    }
    fn theme(&self) -> &str {
        "nature"
    }
    fn describe(&self) -> &str {
        "A jagged lightning bolt zigzags across the bar; intensity pulses with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // How far the bolt extends is controlled by progress.
        let reach = (ctx.eased * w as f32).round() as usize;
        if reach == 0 {
            return Ok(());
        }

        // Brightness pulse: modulates the bolt's drawn thickness.
        let pulse = (ctx.time * 4.0 * PI).sin() * 0.5 + 0.5; // 0..1
        let thickness = if pulse > 0.6 { 2i32 } else { 1i32 };

        // Bolt path: zigzag with fixed "joints" spaced every w/5 dots.
        let segments = 5usize;
        let seg_w = (reach / segments.max(1)).max(1);

        // Seed the zigzag using time (slow drift, so it flickers).
        let mut prev_y = (h / 2) as i32;

        for seg in 0..segments {
            let x0 = (seg * seg_w).min(reach.saturating_sub(1));
            let x1 = ((seg + 1) * seg_w).min(reach.saturating_sub(1));
            if x1 <= x0 {
                break;
            }

            // Next joint y chosen via time-seeded sine (different per segment).
            let angle = ctx.time * 3.0 + seg as f32 * 1.9;
            let next_y = (h as f32 / 2.0 + angle.sin() * h as f32 * 0.38) as i32;

            // Draw a straight line between joints via DDA.
            let dx = (x1 - x0) as i32;
            let dy = next_y - prev_y;
            let steps = dx.max(dy.abs()).max(1);
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let bx = x0 as i32 + (t * dx as f32).round() as i32;
                let by = prev_y + (t * dy as f32).round() as i32;
                for tk in -thickness / 2..=thickness / 2 {
                    draw::dot_i(grid, bx, by + tk);
                }
            }

            prev_y = next_y;
        }

        // Electric tint: palette sampled at progress, brighter at the tip.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let t = if filled_cells <= 1 {
                1.0
            } else {
                cx as f32 / (filled_cells - 1) as f32
            };
            let color = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}
