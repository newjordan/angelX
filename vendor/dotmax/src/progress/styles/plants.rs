//! Plants / flora progress bars — living, growing, breathing.
//!
//! Ten structurally distinct styles, each driven by `ctx.eased` for growth
//! stage and `ctx.time` for sway / breeze animation. Every style uses a
//! completely different rendering algorithm: no two share the same structural
//! approach, differing only in palette.
//!
//! | Name              | Mechanism                                              |
//! |-------------------|--------------------------------------------------------|
//! | `seedling`        | Discrete seed→sprout→stem→leaf→flower growth stages   |
//! | `fern-fiddlehead` | Logarithmic spiral unrolling (fiddlehead uncurling)    |
//! | `bamboo-shoot`    | Vertical segments shooting up one by one               |
//! | `ivy-trellis`     | Vine climbing a grid trellis with curling tendrils     |
//! | `cactus-arms`     | Trunk grows then arms branch out at thresholds         |
//! | `sunflower-seeds` | Disc fills with golden-angle phyllotaxis seed dots     |
//! | `mushroom-cap`    | Cap rises and expands as stalk grows upward            |
//! | `succulent`       | Radial rosette: leaves open from centre outward        |
//! | `root-system`     | Fractal root branches growing downward                 |
//! | `bonsai`          | Trunk → recursive branches → dot canopy               |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ── registry ─────────────────────────────────────────────────────────────────

/// All styles in the `plants` theme.
///
/// Returns ten structurally distinct plant-growth progress bars, each
/// mapping `ctx.eased` to visible growth and `ctx.time` to living motion.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Seedling),
        Box::new(FernFiddlehead),
        Box::new(BambooShoot),
        Box::new(IvyTrellis),
        Box::new(CactusArms),
        Box::new(SunflowerSeeds),
        Box::new(MushroomCap),
        Box::new(Succulent),
        Box::new(RootSystem),
        Box::new(Bonsai),
    ]
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Integer sine — keeps per-bar code terse.
#[inline]
fn isin(angle: f32, amplitude: f32) -> i32 {
    (angle.sin() * amplitude).round() as i32
}

// ── 1. Seedling — discrete growth stages ─────────────────────────────────────
//
// eased maps to five visible stages:
//   0.00–0.20  seed dot (underground bulge)
//   0.20–0.40  sprout curl emerging from soil
//   0.40–0.60  single stem rising
//   0.60–0.80  two side leaves appear
//   0.80–1.00  flower petals open at the tip

struct Seedling;
impl ProgressStyle for Seedling {
    fn name(&self) -> &str {
        "seedling"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "Five discrete growth stages: seed → sprout → stem → leaves → flower, driven by progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let stage = ctx.eased;
        let cx = (w / 2) as i32;
        let ground = h.saturating_sub(2) as i32;
        // Sway all above-ground parts with a gentle breeze.
        let sway = isin(ctx.time * 1.4, (w as f32 * 0.04).max(1.0));

        // Stage 1: seed — a small oval underground.
        if stage >= 0.0 {
            // Seed sits at ground level.
            draw::dot_i(grid, cx, ground);
            draw::dot_i(grid, cx + 1, ground);
            draw::dot_i(grid, cx, ground + 1);
            draw::dot_i(grid, cx + 1, ground + 1);
        }

        // Stage 2: sprout hook emerging above ground (eased 0.20+).
        if stage >= 0.20 {
            // Hook: a short upward line that curves over.
            let sprout_h = ((stage - 0.20) / 0.20 * h as f32 * 0.25).round() as i32;
            for dy in 0..=sprout_h {
                draw::dot_i(grid, cx + sway / 2, ground - dy);
            }
            // Curved tip — the cotyledon loop.
            draw::dot_i(grid, cx + sway / 2 + 1, ground - sprout_h);
            draw::dot_i(grid, cx + sway / 2 + 1, ground - sprout_h + 1);
        }

        // Stage 3: full stem (eased 0.40+).
        if stage >= 0.40 {
            let stem_h = ((stage - 0.40) / 0.20 * h as f32 * 0.50).round() as i32;
            let stem_h = stem_h.min(ground);
            for dy in 0..=stem_h {
                let lean = isin(ctx.time * 1.4 + dy as f32 * 0.2, (w as f32 * 0.04).max(1.0));
                draw::dot_i(grid, cx + lean, ground - dy);
            }
        }

        // Stage 4: two leaves at mid-stem (eased 0.60+).
        if stage >= 0.60 {
            let stem_h = (h as f32 * 0.50).round() as i32;
            let leaf_y = ground - stem_h / 2;
            let leaf_reach = ((stage - 0.60) / 0.20 * w as f32 * 0.20).round() as i32;
            for lx in 1..=leaf_reach {
                // Left leaf curves up-left, right leaf up-right.
                let curve = (lx as f32 / leaf_reach.max(1) as f32 * PI * 0.5).sin();
                let dy = -(curve * 2.0).round() as i32;
                draw::dot_i(grid, cx + sway - lx, leaf_y + dy);
                draw::dot_i(grid, cx + sway + lx, leaf_y + dy);
            }
        }

        // Stage 5: flower petals (eased 0.80+).
        if stage >= 0.80 {
            let stem_h = (h as f32 * 0.50).round() as i32;
            let tip_y = ground - stem_h;
            let petals = 6usize;
            let petal_len = ((stage - 0.80) / 0.20 * (w as f32 * 0.12).max(2.0)).max(0.0);
            for p in 0..petals {
                let angle = (p as f32 / petals as f32) * 2.0 * PI + ctx.time * 0.4;
                let steps = petal_len.round() as usize;
                for s in 1..=steps {
                    let r = s as f32;
                    draw::dot_i(
                        grid,
                        cx + sway + (angle.cos() * r).round() as i32,
                        tip_y + (angle.sin() * r).round() as i32,
                    );
                }
            }
            // Centre of flower.
            draw::dot_i(grid, cx + sway, tip_y);
        }

        // Tint: soil brown at bottom, green in the middle, gold at tip.
        let (cw, ch) = grid.dimensions();
        use crate::Color;
        for cy in 0..ch {
            let t = 1.0 - cy as f32 / ch.saturating_sub(1).max(1) as f32; // 0 = top, 1 = bottom
            let color = if t > 0.85 {
                Color::rgb(101, 67, 33) // soil
            } else {
                ctx.palette.sample(1.0 - t)
            };
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 2. Fern Fiddlehead — logarithmic spiral unrolling ────────────────────────
//
// A fiddlehead starts as a tight coil (eased=0) and progressively unrolls
// into an open frond (eased=1). The spiral uses polar coordinates.
// ctx.time adds a gentle oscillation so the frond breathes.

struct FernFiddlehead;
impl ProgressStyle for FernFiddlehead {
    fn name(&self) -> &str {
        "fern-fiddlehead"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "A fern fiddlehead unrolls from a tight coil into an open frond as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;

        // Logarithmic spiral: r = a * e^(b * theta).
        // At eased=0 draw only the tight coil; at eased=1 draw full frond.
        let a = 1.0f32;
        let b = 0.22f32;

        // Total angular span of the frond: 0 (coil only) → 4*PI (full unroll).
        let max_turns = 4.0 * PI;
        let unroll = ctx.eased * max_turns;

        // Coil starts wound tightly (small theta) and unwinds.
        // We draw from theta = 0 (innermost) outward.
        let max_r = (w.min(h) / 2).saturating_sub(1) as f32;
        let scale = max_r / (a * (b * max_turns).exp());

        // Breathe: the coil rotates slightly with time.
        let breath_offset = (ctx.time * 0.5).sin() * 0.15;

        let steps = 200usize;
        for s in 0..=steps {
            let theta = s as f32 / steps as f32 * unroll;
            if theta > unroll {
                break;
            }
            let r = a * (b * theta).exp() * scale;
            // Rotate so fiddlehead starts pointing down (PI/2) and uncurls upward.
            let angle = theta + PI / 2.0 + breath_offset;
            let px = cx + (r * angle.cos()).round() as i32;
            let py = cy + (r * angle.sin()).round() as i32;
            draw::dot_i(grid, px, py);

            // Pinnae (side leaflets) every half-turn on the outer half of the spiral.
            if theta > max_turns / 2.0 && s % 20 == 0 {
                let pinnae_len = (r * 0.4).max(1.0);
                let pinnae_angle = angle + PI / 2.0;
                for pd in 1..=(pinnae_len.round() as i32) {
                    draw::dot_i(
                        grid,
                        px + (pinnae_angle.cos() * pd as f32).round() as i32,
                        py + (pinnae_angle.sin() * pd as f32).round() as i32,
                    );
                    draw::dot_i(
                        grid,
                        px - (pinnae_angle.cos() * pd as f32).round() as i32,
                        py - (pinnae_angle.sin() * pd as f32).round() as i32,
                    );
                }
            }
        }

        // Tint: deep green shifting to pale frond tip.
        let (cw, ch) = grid.dimensions();
        for row in 0..ch {
            let t = row as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased);
            draw::tint_row(grid, row, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 3. Bamboo Shoot — vertical segments shooting up ──────────────────────────
//
// Each segment is a cell-glyph column block. Segments appear one by one from
// the bottom as eased advances. Each joint has a horizontal line (node).
// ctx.time causes segments to sway slightly left-right as a column.

struct BambooShoot;
impl ProgressStyle for BambooShoot {
    fn name(&self) -> &str {
        "bamboo-shoot"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "Bamboo culm grows segment by segment upward; joints appear at each node as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let (cw, ch) = grid.dimensions();
        // Bamboo column sits in the centre third.
        let stem_cx = cw / 2;
        let seg_height_cells = 2usize.max(1); // each segment is 2 cell rows tall
        let max_segs = (ch / seg_height_cells).max(1);
        let segs_grown = (ctx.eased * max_segs as f32).round() as usize;

        // Sway: the culm leans left/right.
        let sway_cells = (isin(ctx.time * 1.0, (cw as f32 * 0.06).max(1.0))) as i32;

        for seg in 0..segs_grown.min(max_segs) {
            // Segments grow from bottom up.
            let base_cell_y = ch.saturating_sub(1 + seg * seg_height_cells);
            let sx = (stem_cx as i32 + sway_cells).clamp(0, cw.saturating_sub(1) as i32) as usize;

            // Draw segment body using vblock glyphs (full column in both cell rows).
            for dy in 0..seg_height_cells {
                let cy = base_cell_y.saturating_sub(dy);
                draw::vblock(grid, sx.min(cw.saturating_sub(1)), cy, 8);
                // Thin border dots on both sides (in dot space).
                let dx = sx * 2;
                let dy_dot = cy * 4;
                draw::vline(grid, dx.saturating_sub(1), dy_dot, dy_dot + 3);
                draw::vline(grid, (dx + 2).min(w.saturating_sub(1)), dy_dot, dy_dot + 3);
            }

            // Node (joint) line at the base of each segment.
            let node_y = base_cell_y;
            let node_dot_y = node_y * 4 + 3;
            let left = sx.saturating_sub(1) * 2;
            let right = (sx + 2).min(w / 2) * 2;
            draw::hline(grid, left, right, node_dot_y.min(h.saturating_sub(1)));

            // Leaf pair at every other node (alternating sides).
            if seg % 2 == 0 && segs_grown > 1 {
                let leaf_y = node_y as i32;
                let leaf_x_base = sx as i32 * 2;
                let side = if seg % 4 == 0 { 1i32 } else { -1i32 };
                // Three-dot leaf sweeping out.
                for ld in 1..=3i32 {
                    draw::dot_i(grid, leaf_x_base + side * ld, (leaf_y * 4) as i32 - ld);
                }
            }
        }

        // Gradient tint: deep green at base, yellow-green at tip.
        for cy in 0..ch {
            let t = 1.0 - cy as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 4. Ivy Trellis — vine climbing a grid trellis with tendrils ──────────────
//
// The trellis is a fixed dot-grid. The vine advances along the trellis
// left→right (progress). At intervals, curling tendrils spiral off the vine.
// ctx.time animates the tendril curl continuously.

struct IvyTrellis;
impl ProgressStyle for IvyTrellis {
    fn name(&self) -> &str {
        "ivy-trellis"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "Ivy climbs a dot-grid trellis left-to-right; curling tendrils spiral off the vine"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Draw trellis: vertical posts every 8 dots, two horizontal rails.
        let post_spacing = 8usize.max(1);
        let rail1 = h / 4;
        let rail2 = 3 * h / 4;
        // Top rail.
        draw::hline(grid, 0, w.saturating_sub(1), rail1);
        // Bottom rail.
        draw::hline(grid, 0, w.saturating_sub(1), rail2);
        // Vertical posts.
        let posts = w / post_spacing;
        for p in 0..=posts {
            let px = p * post_spacing;
            draw::vline(grid, px.min(w.saturating_sub(1)), rail1, rail2);
        }

        // Vine: travels along the bottom rail, then climbs up each post, then along top rail.
        // The vine path is parameterized in [0,1] → (x,y) dot coordinates.
        // Phase 0–0.5: bottom rail left→right
        // Phase 0.5–1.0: top rail left→right (climbs posts along the way)
        let reach = ctx.eased;
        let vine_end_dot = (reach * w as f32) as usize;

        // Bottom rail vine.
        let bottom_reach = (reach * 2.0).min(1.0);
        let brd = (bottom_reach * w as f32) as usize;
        for x in 0..brd.min(w) {
            let sway = isin(x as f32 * 0.5 + ctx.time * 1.2, 1.0);
            draw::dot_i(grid, x as i32, rail2 as i32 + sway);
        }

        // Top rail vine only appears in second half of progress.
        if reach > 0.5 {
            let top_reach = (reach - 0.5) * 2.0;
            let trd = (top_reach * w as f32) as usize;
            for x in 0..trd.min(w) {
                let sway = isin(x as f32 * 0.5 + ctx.time * 1.2, 1.0);
                draw::dot_i(grid, x as i32, rail1 as i32 + sway);
            }
        }

        // Tendrils: small clockwise spirals hanging off the bottom vine.
        let tendril_spacing = 12usize.max(1);
        let tendril_count = vine_end_dot / tendril_spacing;
        for t_idx in 0..tendril_count {
            let tx = (t_idx * tendril_spacing + tendril_spacing / 2).min(w.saturating_sub(1));
            // Tendril curls below the rail.
            let curl_turns = 1.5f32;
            let tendril_r = (h as f32 * 0.08).max(2.0);
            let phase_offset = ctx.time * 1.5 + t_idx as f32 * 0.8;
            let curl_steps = 24usize;
            for s in 0..=curl_steps {
                let theta = s as f32 / curl_steps as f32 * curl_turns * 2.0 * PI + phase_offset;
                let r = tendril_r * (1.0 - s as f32 / curl_steps.max(1) as f32 * 0.7);
                let px = tx as i32 + (theta.cos() * r).round() as i32;
                let py = rail2 as i32 + 2 + (theta.sin() * r).round() as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Tint the filled region.
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

// ── 5. Cactus Arms — trunk grows then lateral arms branch out ────────────────
//
// The trunk (a solid vline) grows from the bottom up as eased → 0.5.
// At eased = 0.5, left arm appears; at eased = 0.75, right arm.
// At eased = 1.0, arms grow spines (small perpendicular dots).
// ctx.time gives a slow pulse (very slight trunk width change).

struct CactusArms;
impl ProgressStyle for CactusArms {
    fn name(&self) -> &str {
        "cactus-arms"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "A cactus trunk rises then sprouts a left arm, right arm, and finally spines at full growth"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let base_y = h.saturating_sub(1) as i32;

        // Trunk grows from base upward — first 50% of eased.
        let trunk_frac = (ctx.eased / 0.5).min(1.0);
        let trunk_h = (trunk_frac * h as f32 * 0.80).round() as i32;
        let trunk_top = base_y - trunk_h;

        // Draw trunk as 2-wide column.
        for dy in 0..=trunk_h {
            let y = base_y - dy;
            draw::dot_i(grid, cx, y);
            draw::dot_i(grid, cx + 1, y);
        }

        // Slow pulse width (purely cosmetic, time-driven).
        let pulse = ((ctx.time * 0.8).sin() * 0.5 + 0.5) > 0.5;
        if pulse && trunk_h > 4 {
            for dy in 2..=trunk_h - 2 {
                draw::dot_i(grid, cx - 1, base_y - dy);
            }
        }

        // Left arm appears at eased 0.50–0.75.
        if ctx.eased >= 0.50 {
            let arm_frac = ((ctx.eased - 0.50) / 0.25).min(1.0);
            let arm_attach_y = trunk_top + trunk_h / 3;
            let arm_w = (arm_frac * w as f32 * 0.25).round() as i32;
            // Horizontal run out, then up.
            for dx in 0..=arm_w {
                draw::dot_i(grid, cx - dx, arm_attach_y);
            }
            let arm_up = (arm_frac * h as f32 * 0.15).round() as i32;
            for dy in 0..=arm_up {
                draw::dot_i(grid, cx - arm_w, arm_attach_y - dy);
            }
        }

        // Right arm appears at eased 0.75–1.00.
        if ctx.eased >= 0.75 {
            let arm_frac = ((ctx.eased - 0.75) / 0.25).min(1.0);
            let arm_attach_y = trunk_top + trunk_h / 5;
            let arm_w = (arm_frac * w as f32 * 0.22).round() as i32;
            for dx in 0..=arm_w {
                draw::dot_i(grid, cx + 1 + dx, arm_attach_y);
            }
            let arm_up = (arm_frac * h as f32 * 0.12).round() as i32;
            for dy in 0..=arm_up {
                draw::dot_i(grid, cx + 1 + arm_w, arm_attach_y - dy);
            }
        }

        // Spines: small perpendicular dots along trunk at full growth.
        if ctx.eased >= 0.95 {
            let spine_spacing = 4i32;
            let mut y = base_y - spine_spacing;
            while y > trunk_top {
                // Alternating left/right spines.
                let side = if (y / spine_spacing) % 2 == 0 {
                    3i32
                } else {
                    -3i32
                };
                draw::dot_i(grid, cx + side, y);
                draw::dot_i(grid, cx + side / 2, y - 1);
                y -= spine_spacing;
            }
        }

        // Cactus green tint.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(1.0 - t * 0.5);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 6. Sunflower Seeds — golden-angle phyllotaxis ────────────────────────────
//
// The disc is filled with seeds placed at golden-angle increments.
// eased controls how many seeds are visible (from centre outward).
// ctx.time causes the whole disc to rotate slowly.

struct SunflowerSeeds;
impl ProgressStyle for SunflowerSeeds {
    fn name(&self) -> &str {
        "sunflower-seeds"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "Sunflower disc fills with golden-angle phyllotaxis seeds radiating from the centre"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let max_r = (w.min(h) / 2).saturating_sub(1) as f32;

        // Total seeds scales with area; use a fixed cap for fast rendering.
        let max_seeds = 200usize;
        let visible = (ctx.eased * max_seeds as f32).round() as usize;

        // Golden angle in radians.
        let golden_angle = PI * (3.0 - 5.0_f32.sqrt());
        let rotation_offset = ctx.time * 0.2;

        for n in 0..visible.min(max_seeds) {
            let r = max_r * (n as f32 / max_seeds as f32).sqrt();
            let theta = n as f32 * golden_angle + rotation_offset;
            let sx = cx + (r * theta.cos()).round() as i32;
            let sy = cy + (r * theta.sin()).round() as i32;
            draw::dot_i(grid, sx, sy);
            // Slightly thicker seeds in the outer half.
            if r > max_r * 0.5 {
                draw::dot_i(grid, sx + 1, sy);
            }
        }

        // Outer ring (flower receptacle edge).
        if ctx.eased > 0.1 {
            let rim_r = (max_r * ctx.eased.sqrt()).round() as i32;
            let rim_r = rim_r.max(1);
            let steps = (2.0 * PI * rim_r as f32).round() as usize + 4;
            for s in 0..steps {
                let angle = s as f32 / steps as f32 * 2.0 * PI;
                draw::dot_i(
                    grid,
                    cx + (rim_r as f32 * angle.cos()).round() as i32,
                    cy + (rim_r as f32 * angle.sin()).round() as i32,
                );
            }
        }

        // Warm yellow-brown centre tint.
        let (cw, ch) = grid.dimensions();
        for row in 0..ch {
            let t = row as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased);
            draw::tint_row(grid, row, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 7. Mushroom Cap — stalk rises, cap expands ───────────────────────────────
//
// eased 0–0.5: stalk grows upward from the ground (vline, 2-wide).
// eased 0.5–1.0: dome cap arc widens from a dot to a full hemisphere.
// ctx.time causes subtle cap wobble and spore dots fall below the cap.

struct MushroomCap;
impl ProgressStyle for MushroomCap {
    fn name(&self) -> &str {
        "mushroom-cap"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "A mushroom stalk rises then its dome cap expands; spore dots drift downward"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let ground = h.saturating_sub(1) as i32;

        // Stalk grows from ground up during first 50%.
        let stalk_frac = (ctx.eased / 0.5).min(1.0);
        let stalk_h = (stalk_frac * h as f32 * 0.55).round() as i32;
        for dy in 0..=stalk_h {
            let y = ground - dy;
            draw::dot_i(grid, cx, y);
            draw::dot_i(grid, cx + 1, y);
        }

        // Stalk top position.
        let stalk_top = ground - stalk_h;

        // Cap: dome arc (upper semi-ellipse) centred above the stalk.
        if ctx.eased >= 0.5 {
            let cap_frac = ((ctx.eased - 0.5) / 0.5).min(1.0);
            // Wobble with time.
            let wobble = (ctx.time * 2.3).sin() * cap_frac * 1.0;
            let cap_rx = ((cap_frac * w as f32 * 0.40) + wobble).max(1.0) as i32;
            let cap_ry = (cap_frac * h as f32 * 0.35).max(1.0) as i32;
            let cap_cy = stalk_top;

            // Draw filled upper semi-ellipse.
            for dy in 0..=cap_ry {
                // Horizontal half-width at this row (ellipse formula).
                let row_w = if cap_ry == 0 {
                    0
                } else {
                    (cap_rx as f32 * (1.0 - (dy as f32 / cap_ry as f32).powi(2)).sqrt()).round()
                        as i32
                };
                let y = cap_cy - dy;
                for dx in -row_w..=row_w {
                    draw::dot_i(grid, cx + dx, y);
                }
            }

            // Spots on the cap (Amanita-style) — fixed polar positions.
            let spots = [(0.3f32, 0.4f32), (-0.35, 0.55), (0.55, 0.65), (-0.1, 0.75)];
            for &(sx_frac, sy_frac) in &spots {
                let sx = cx + (sx_frac * cap_rx as f32).round() as i32;
                let sy = cap_cy - (sy_frac * cap_ry as f32).round() as i32;
                draw::dot_i(grid, sx, sy);
                draw::dot_i(grid, sx + 1, sy);
            }

            // Spore fall: dots below the cap edge drift downward.
            let spore_count = 5usize;
            for s in 0..spore_count {
                let phase = s as f32 / spore_count as f32;
                let spore_x = cx + ((phase - 0.5) * cap_rx as f32 * 1.8).round() as i32;
                let drop = ((ctx.time * 0.8 + phase).fract() * (ground - stalk_top) as f32) as i32;
                let spore_y = stalk_top + drop;
                draw::dot_i(grid, spore_x, spore_y);
            }
        }

        // Tint: earthy warm for stalk, red/orange cap above.
        let (cw, ch) = grid.dimensions();
        let stalk_cell = (stalk_top.max(0) as usize / 4).min(ch.saturating_sub(1));
        for cy in 0..ch {
            let t = if cy < stalk_cell {
                ctx.palette.sample(0.9)
            } else {
                ctx.palette
                    .sample(0.3 + cy as f32 / ch.saturating_sub(1).max(1) as f32 * 0.6)
            };
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), t);
        }

        Ok(())
    }
}

// ── 8. Succulent Rosette — radial leaves open from centre ────────────────────
//
// eased controls how many leaves are visible AND their opening angle.
// Leaves are rendered as filled wedge arcs growing outward from the centre.
// ctx.time rotates the whole rosette slowly (succulents track the sun).

struct Succulent;
impl ProgressStyle for Succulent {
    fn name(&self) -> &str {
        "succulent-rosette"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "A succulent rosette opens radially; leaves widen and extend as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let max_r = (w.min(h) / 2).saturating_sub(1) as f32;

        let total_leaves = 12usize;
        // Leaves appear in order as progress increases.
        let leaves_open = (ctx.eased * total_leaves as f32).ceil() as usize;

        // Rotate slowly with time.
        let rotation = ctx.time * 0.15;

        for leaf in 0..leaves_open.min(total_leaves) {
            let base_angle = (leaf as f32 / total_leaves as f32) * 2.0 * PI + rotation;

            // This leaf's opening fraction (newest leaf partially open, others full).
            let leaf_frac = if leaf + 1 < leaves_open {
                1.0f32
            } else {
                let base = leaf as f32 / total_leaves as f32;
                ((ctx.eased - base) * total_leaves as f32).clamp(0.0, 1.0)
            };

            let leaf_r = (leaf_frac * max_r * 0.85).max(0.0);
            // Half-angular width of each leaf (wedge).
            let half_w = leaf_frac * PI / (total_leaves as f32) * 2.5;

            // Fill the wedge arc with dot samples.
            let arc_steps = (leaf_r * 4.0).round() as usize + 4;
            let r_steps = (leaf_r * 0.5).round() as usize + 2;
            for rs in 1..=r_steps {
                let r = leaf_r * rs as f32 / r_steps as f32;
                for a in 0..=arc_steps {
                    let angle_off = (a as f32 / arc_steps as f32 - 0.5) * 2.0 * half_w;
                    let angle = base_angle + angle_off;
                    let px = cx + (r * angle.cos()).round() as i32;
                    let py = cy + (r * angle.sin()).round() as i32;
                    draw::dot_i(grid, px, py);
                }
            }

            // Midrib line (leaf vein).
            let vein_steps = (leaf_r * 0.9).round() as usize;
            for s in 0..=vein_steps {
                let r = leaf_r * s as f32 / vein_steps.max(1) as f32;
                draw::dot_i(
                    grid,
                    cx + (base_angle.cos() * r).round() as i32,
                    cy + (base_angle.sin() * r).round() as i32,
                );
            }
        }

        // Centre dome.
        draw::dot_i(grid, cx, cy);
        draw::dot_i(grid, cx + 1, cy);
        draw::dot_i(grid, cx, cy + 1);

        // Tint concentric rings.
        let (cw, ch) = grid.dimensions();
        for row in 0..ch {
            let t = row as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, row, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 9. Root System — fractal branching downward ───────────────────────────────
//
// Grows roots downward from a horizontal taproot line at the top.
// eased controls how deep / how many branch generations appear.
// ctx.time causes slight lateral tremor (root seeking moisture).

struct RootSystem;
impl ProgressStyle for RootSystem {
    fn name(&self) -> &str {
        "root-system"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "Fractal root branches grow downward from a taproot; depth and density increase with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Taproot horizontal line at top.
        let troot_y = 1usize;
        draw::hline(grid, 0, w.saturating_sub(1), troot_y);

        // Number of branch generations shown.
        let max_depth = 5usize;
        let depth = (ctx.eased * max_depth as f32).ceil() as usize;

        // Recursive branch drawer (iterative via stack to avoid recursion limits).
        // Stack: (x, y, angle, length, generation)
        let mut stack: Vec<(i32, i32, f32, f32, usize)> = Vec::new();

        // Seed with 3 primary roots evenly spaced.
        let num_primaries = 3usize;
        for p in 0..num_primaries {
            let px = (p as f32 / (num_primaries - 1).max(1) as f32 * (w.saturating_sub(1)) as f32)
                as i32;
            stack.push((px, troot_y as i32, PI / 2.0, h as f32 * 0.35, 0));
        }

        while let Some((x0, y0, angle, length, gen)) = stack.pop() {
            if gen >= depth {
                continue;
            }
            if length < 1.5 {
                continue;
            }

            let tremor = (ctx.time * 2.0 + gen as f32 * 1.3 + x0 as f32 * 0.1).sin() * 0.08;
            let actual_angle = angle + tremor;

            let x1 = x0 + (actual_angle.cos() * length).round() as i32;
            let y1 = y0 + (actual_angle.sin() * length).round() as i32;

            // Draw this segment.
            let steps = length.round() as usize + 1;
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let bx = x0 + ((x1 - x0) as f32 * t).round() as i32;
                let by = y0 + ((y1 - y0) as f32 * t).round() as i32;
                draw::dot_i(grid, bx, by);
            }

            // Branch into two children, spread by ~30 degrees.
            let child_len = length * 0.62;
            let spread = PI / 6.0;
            stack.push((x1, y1, actual_angle - spread, child_len, gen + 1));
            stack.push((x1, y1, actual_angle + spread, child_len, gen + 1));
        }

        // Earthy tint: dark at top roots, lighter at fine root tips below.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 10. Bonsai — trunk → recursive branches → dot canopy ─────────────────────
//
// eased 0–0.40: trunk rises from bottom, slightly tapered.
// eased 0.40–0.70: two main branches fork from near the top.
// eased 0.70–0.90: secondary branches fork again.
// eased 0.90–1.00: canopy dots fill in around the branch tips.
// ctx.time: gentle breeze sways the canopy.

struct Bonsai;
impl ProgressStyle for Bonsai {
    fn name(&self) -> &str {
        "bonsai"
    }
    fn theme(&self) -> &str {
        "plants"
    }
    fn describe(&self) -> &str {
        "A bonsai forms: trunk rises, branches fork in tiers, then canopy foliage fills in"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let ground = h.saturating_sub(1) as i32;

        // ── Trunk ──────────────────────────────────────────────────────────
        let trunk_frac = (ctx.eased / 0.40).min(1.0);
        let trunk_h = (trunk_frac * h as f32 * 0.55).round() as i32;
        for dy in 0..=trunk_h {
            let y = ground - dy;
            // Taper: 3 wide at base, 1 wide at top.
            let width = if dy < trunk_h / 3 {
                3i32
            } else if dy < 2 * trunk_h / 3 {
                2i32
            } else {
                1i32
            };
            for dx in 0..width {
                draw::dot_i(grid, cx - width / 2 + dx, y);
            }
        }

        let fork_y = ground - trunk_h; // where branches begin

        // ── Primary branches ───────────────────────────────────────────────
        if ctx.eased >= 0.40 {
            let br_frac = ((ctx.eased - 0.40) / 0.30).min(1.0);
            let br_len = (br_frac * w as f32 * 0.28).round() as i32;
            let br_rise = (br_frac * h as f32 * 0.20).round() as i32;
            // Left branch.
            for s in 0..=br_len {
                let t = s as f32 / br_len.max(1) as f32;
                let bx = cx - s;
                let by = fork_y - (t * br_rise as f32).round() as i32;
                draw::dot_i(grid, bx, by);
            }
            // Right branch.
            for s in 0..=br_len {
                let t = s as f32 / br_len.max(1) as f32;
                let bx = cx + s;
                let by = fork_y - (t * br_rise as f32 * 0.8).round() as i32;
                draw::dot_i(grid, bx, by);
            }

            // ── Secondary branches ─────────────────────────────────────────
            if ctx.eased >= 0.70 {
                let sb_frac = ((ctx.eased - 0.70) / 0.20).min(1.0);
                let sb_len = (sb_frac * w as f32 * 0.15).round() as i32;
                let sb_rise = (sb_frac * h as f32 * 0.12).round() as i32;
                // Tips of primary branches.
                let left_tip_x = cx - br_len;
                let left_tip_y = fork_y - br_rise;
                let right_tip_x = cx + br_len;
                let right_tip_y = fork_y - (br_rise as f32 * 0.8).round() as i32;

                for s in 0..=sb_len {
                    let t = s as f32 / sb_len.max(1) as f32;
                    // From left tip: branch further left and up.
                    draw::dot_i(
                        grid,
                        left_tip_x - s,
                        left_tip_y - (t * sb_rise as f32).round() as i32,
                    );
                    // From left tip: branch up-right.
                    draw::dot_i(
                        grid,
                        left_tip_x + s / 2,
                        left_tip_y - (t * sb_rise as f32 * 1.2).round() as i32,
                    );
                    // From right tip: branch right and up.
                    draw::dot_i(
                        grid,
                        right_tip_x + s,
                        right_tip_y - (t * sb_rise as f32).round() as i32,
                    );
                    // From right tip: branch up-left.
                    draw::dot_i(
                        grid,
                        right_tip_x - s / 2,
                        right_tip_y - (t * sb_rise as f32 * 1.1).round() as i32,
                    );
                }

                // ── Canopy foliage dots ────────────────────────────────────
                if ctx.eased >= 0.90 {
                    let canopy_frac = ((ctx.eased - 0.90) / 0.10).min(1.0);
                    // Scatter foliage dots around branch tips using time-driven noise.
                    let dot_count = (canopy_frac * 40.0).round() as usize;
                    let canopy_cx = cx;
                    let canopy_cy = (fork_y - br_rise - sb_rise).max(0);
                    let canopy_rx = (br_len + sb_len).max(1) as f32;
                    let canopy_ry = (br_rise + sb_rise).max(1) as f32;

                    for d in 0..dot_count {
                        // Deterministic scatter via golden angle + time sway.
                        let theta = d as f32 * 2.399 + ctx.time * 0.5; // 2.399 ≈ golden angle
                        let r_frac = (d as f32 / dot_count.max(1) as f32).sqrt();
                        let sway = (ctx.time * 1.3 + d as f32 * 0.3).sin() * 1.5;
                        let dx = (theta.cos() * canopy_rx * r_frac + sway).round() as i32;
                        let dy = (theta.sin() * canopy_ry * r_frac).round() as i32;
                        draw::dot_i(grid, canopy_cx + dx, canopy_cy + dy);
                    }
                }
            }
        }

        // Soil line at very bottom.
        draw::hline(grid, 0, w.saturating_sub(1), ground as usize);

        // Tint: brown trunk base → green foliage top.
        let (cw, ch) = grid.dimensions();
        let trunk_cell_top = (fork_y.max(0) as usize / 4).min(ch.saturating_sub(1));
        for cy in 0..ch {
            let color = if cy >= trunk_cell_top {
                ctx.palette.sample(0.85) // trunk: warm brown-ish
            } else {
                ctx.palette
                    .sample(cy as f32 / trunk_cell_top.max(1) as f32 * 0.7)
            };
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}
