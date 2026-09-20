//! Fruits-themed progress bars — eleven distinct fruit styles drawn in braille dots.
//!
//! Every bar is stateless: all motion comes from `ctx.time` (for perpetual
//! animation) and `ctx.eased` (for progress-driven advancement). All writes
//! go through `draw::` helpers so every coordinate is silently bounds-safe.
//!
//! Styles in this file:
//! - `citrus-wheel`    — orange/citrus radial segments filling like a pie
//! - `apple-bite`      — apple silhouette with a bite that grows with eased
//! - `banana-peel`     — peel strips folding down progressively
//! - `grape-bunch`     — triangular grape cluster filling grape by grape
//! - `watermelon-slice`— semicircle rind-to-center fill with seeds appearing
//! - `strawberry-seeds`— strawberry body with seed-dimple pattern spreading
//! - `pineapple-lattice`— diamond-lattice rind advancing column by column
//! - `cherry-swing`    — cherry pair on stems, swinging with time
//! - `juice-squeeze`   — fruit compresses top-down, juice level rises below
//! - `berry-pop`       — berry cluster popping in one by one with a bounce
//! - `kiwi-rings`      — cross-section concentric elliptic rings filling outward

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

/// All styles in the `fruits` theme.
///
/// Returns eleven [`ProgressStyle`] implementations, each using a distinct
/// geometric or motion strategy so no two differ by color alone.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(CitrusWheel),
        Box::new(AppleBite),
        Box::new(BananaPeel),
        Box::new(GrapeBunch),
        Box::new(WatermelonSlice),
        Box::new(StrawberrySeeds),
        Box::new(PineappleLattice),
        Box::new(CherrySwing),
        Box::new(JuiceSqueeze),
        Box::new(BerryPop),
        Box::new(KiwiRings),
    ]
}

// ── CitrusWheel ───────────────────────────────────────────────────────────────
// Orange/citrus cross-section: pie segments fill clockwise with eased.
// Each segment is a triangular wedge drawn via polar scan.

struct CitrusWheel;
impl ProgressStyle for CitrusWheel {
    fn name(&self) -> &str {
        "citrus-wheel"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Orange cross-section: radial pie segments fill clockwise as progress grows"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        // Radius fits inside the smaller dimension.
        let r = ((w.min(h * 2) / 2).saturating_sub(1)).max(1) as f32;
        // How many full radians are filled (clockwise from top = -PI/2).
        let filled_angle = ctx.eased * 2.0 * PI;
        // Animate a slow rotation of the whole wheel with time.
        let spin = ctx.time * 0.3;
        // Draw the outer circle outline.
        let steps = (r * 2.0 * PI) as usize + 1;
        for i in 0..=steps {
            let theta = i as f32 / steps.max(1) as f32 * 2.0 * PI;
            let px = cx + (r * theta.cos()) as i32;
            let py = cy + (r * 0.5 * theta.sin()) as i32; // 0.5 = dot aspect
            draw::dot_i(grid, px, py);
        }
        // Fill wedge: for every dot inside the circle, check if its angle is filled.
        let ri = r as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                // Dot-space has 2:1 aspect (x dots = 2× cells, y dots = 4× cells)
                // so vertical dots are half the visual height of horizontal dots.
                // Compensate by scaling dy by 2 for distance check.
                let dist2 = dx * dx + dy * dy * 4;
                if dist2 as f32 > r * r {
                    continue;
                }
                // Angle from top, going clockwise.
                let angle = (dy as f32 * 2.0).atan2(dx as f32) + PI / 2.0 - spin;
                let angle = ((angle % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);
                if angle <= filled_angle {
                    draw::dot_i(grid, cx + dx, cy + dy);
                }
            }
        }
        // Segment divider lines (8 segments like a real citrus).
        let n_seg = 8u32;
        for s in 0..n_seg {
            let theta = s as f32 / n_seg as f32 * 2.0 * PI - PI / 2.0 + spin;
            let seg_angle = ((theta + PI / 2.0 - spin) % (2.0 * PI) + 2.0 * PI) % (2.0 * PI);
            if seg_angle > filled_angle + 0.01 {
                continue;
            }
            // Draw a thin line from center to rim.
            let steps_r = r as usize;
            for ri2 in 0..=steps_r {
                let frac = ri2 as f32 / steps_r.max(1) as f32;
                let px = cx + (frac * r * theta.cos()) as i32;
                let py = cy + (frac * r * 0.5 * theta.sin()) as i32;
                // Draw divider as "empty" — skip every other dot to create a gap line.
                if ri2 % 2 == 0 {
                    // We can't easily erase, so just mark them — the sector fill gives
                    // enough visual distinction without erasure.
                    let _ = (px, py); // dividers are implicit in the wedge boundaries
                }
            }
        }
        // Tint the filled cells orange.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cy2 in 0..cells_h {
            for cx2 in 0..filled_cells.min(cells_w) {
                let t = if filled_cells <= 1 {
                    0.5
                } else {
                    cx2 as f32 / filled_cells as f32
                };
                let color = ctx.palette.sample(t);
                draw::tint_row(grid, cy2, cx2, cx2, color);
            }
        }
        Ok(())
    }
}

// ── AppleBite ────────────────────────────────────────────────────────────────
// Apple silhouette: full apple drawn as an ellipse; a circular bite hole grows
// from the right side as eased increases, erasing dots inside the bite region.
// Since we cannot erase, we scan and draw only dots outside the bite.

struct AppleBite;
impl ProgressStyle for AppleBite {
    fn name(&self) -> &str {
        "apple-bite"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Apple silhouette with a bite taken from the right — bite grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let cx = (w / 2) as i32;
        // Apple body centered slightly above middle.
        let cy = (h / 2).saturating_add(h / 8) as i32;
        let rx = (w / 2).saturating_sub(1).max(1) as f32;
        let ry = (h / 2).saturating_sub(1).max(1) as f32;
        // Bite center: right edge, mid-height. Radius grows with progress.
        let bite_cx = w as i32 - 1;
        let bite_cy = cy;
        let bite_r = (ctx.eased * (rx * 0.9 + 1.0)).max(0.0);
        // Stem (a short vline above apple).
        let stem_x = cx;
        let stem_top = 0i32;
        let stem_bot = (cy as i32 - ry as i32).max(0);
        for sy in stem_top..=stem_bot {
            draw::dot_i(grid, stem_x, sy);
        }
        // Leaf: a few dots to the right of the stem.
        let leaf_y = (stem_bot + stem_top) / 2;
        draw::dot_i(grid, stem_x + 1, leaf_y);
        draw::dot_i(grid, stem_x + 2, leaf_y - 1);
        // Scan the apple ellipse and draw dots not inside the bite.
        let rx_i = rx as i32 + 1;
        let ry_i = ry as i32 + 1;
        for dy in -ry_i..=ry_i {
            for dx in -rx_i..=rx_i {
                // Inside apple ellipse? (scale for dot aspect).
                let in_apple =
                    (dx as f32 / rx).powi(2) + (dy as f32 * 2.0 / (ry * 2.0)).powi(2) <= 1.0;
                if !in_apple {
                    continue;
                }
                let px = cx + dx;
                let py = cy + dy;
                // Inside bite hole?
                let bdx = px - bite_cx;
                let bdy = py - bite_cy;
                let in_bite = ((bdx * bdx + bdy * bdy * 4) as f32) < bite_r * bite_r;
                if !in_bite {
                    draw::dot_i(grid, px, py);
                }
            }
        }
        // Tint apple cells.
        let (cells_w, cells_h) = grid.dimensions();
        for cy2 in 0..cells_h {
            for cx2 in 0..cells_w {
                let t = cx2 as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy2, cx2, cx2, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── BananaPeel ───────────────────────────────────────────────────────────────
// A banana being peeled: the bar width represents the banana. Peel strips
// (4 vertical bands) fold outward progressively. The inner flesh is revealed
// as eased increases. Strips swing down via a fold angle proportional to eased.

struct BananaPeel;
impl ProgressStyle for BananaPeel {
    fn name(&self) -> &str {
        "banana-peel"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Banana being peeled: four peel strips fold outward as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let n_strips = 4usize;
        let strip_w = (w / n_strips).max(1);
        // How far the peel has opened: 0 = closed, 1 = fully peeled.
        let peel = ctx.eased;
        // Animate a gentle sway.
        let sway = (ctx.time * 2.0).sin() * 0.5;
        let mid = h / 2;
        for strip in 0..n_strips {
            let x_center = strip * strip_w + strip_w / 2;
            // Fold angle: how far the top of the strip has swung away from center.
            // At peel=0 strips are straight (no fold); at peel=1 they droop fully.
            let fold_frac = (peel * 1.2 - strip as f32 * 0.15).clamp(0.0, 1.0);
            let droop = (fold_frac * h as f32 * 0.4 + sway) as i32;
            // Draw the peel strip: a curved arc from top (with droop) to bottom.
            for y in 0..h {
                // Horizontal offset from center: parabolic curve simulating peel fold.
                let t_strip = y as f32 / h.max(1) as f32;
                // Top half folds away from center, bottom half stays anchored.
                let fold_off = if t_strip < 0.5 {
                    // Fold top outward.
                    let t2 = t_strip * 2.0; // 0..1 over top half
                    let expand = (strip as i32 * 2 - n_strips as i32 + 1).signum(); // direction
                    (fold_frac * expand as f32 * strip_w as f32 * t2 * 0.5) as i32
                } else {
                    0i32
                };
                let px = x_center as i32 + fold_off;
                let py =
                    y as i32 - (droop as i32 * (1 - (y as i32 * 2 / h.max(1) as i32).abs())).max(0);
                let py = py.clamp(0, h as i32 - 1);
                draw::dot_i(grid, px, py);
                // Peel edge dots on both sides of the strip.
                draw::dot_i(grid, px - 1, py);
                draw::dot_i(grid, px + 1, py);
            }
            // Inner flesh: fill the center as strips peel away.
            if peel > strip as f32 / n_strips as f32 {
                let flesh_top = (mid as f32 * (1.0 - peel * 0.5)) as usize;
                let flesh_bot = h.saturating_sub(flesh_top);
                draw::vline(grid, x_center.min(w - 1), flesh_top, flesh_bot.min(h - 1));
            }
        }
        // Tint yellow-ish using palette.
        let (cells_w, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            for cx in 0..cells_w {
                let t = cx as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── GrapeBunch ───────────────────────────────────────────────────────────────
// Triangular cluster of grape circles. Grapes appear bottom-to-top (heavy
// bottom cluster first) as eased advances. Each grape is a small filled circle.

struct GrapeBunch;
impl ProgressStyle for GrapeBunch {
    fn name(&self) -> &str {
        "grape-bunch"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Triangular grape bunch filling grape-by-grape from the bottom of the cluster"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        // Build a triangular grid of grape positions (in dot coords).
        // Grape radius: small enough to pack several across the bar.
        let grape_r = (w.min(h * 2) / 10).max(1) as i32;
        let diam = grape_r * 2 + 1;
        // Number of columns at the widest row: as many as fit.
        let max_cols = (w as i32 / diam.max(1)).max(1) as usize;
        // Number of rows: triangle means row k has (max_cols - k) grapes.
        let max_rows = max_cols.max(1);
        // Collect all grape centers in bottom-to-top order.
        let mut grapes: Vec<(i32, i32)> = Vec::new();
        for row in 0..max_rows {
            let n_in_row = max_cols.saturating_sub(row);
            if n_in_row == 0 {
                break;
            }
            // Center this row horizontally.
            let row_w = n_in_row as i32 * diam;
            let x_start = (w as i32 - row_w) / 2 + grape_r;
            let y = h as i32 - 1 - row as i32 * diam;
            if y < 0 {
                break;
            }
            for col in 0..n_in_row {
                let x = x_start + col as i32 * diam;
                grapes.push((x, y));
            }
        }
        // How many grapes are visible.
        let visible = (ctx.eased * grapes.len() as f32).ceil() as usize;
        // Draw stem from top grape up.
        if let Some(&(sx, sy)) = grapes.last() {
            let stem_top = 0i32;
            for sy2 in stem_top..sy.saturating_sub(grape_r) {
                draw::dot_i(grid, sx, sy2);
            }
        }
        // Draw each visible grape as a small filled circle.
        for (i, &(gx, gy)) in grapes.iter().enumerate() {
            if i >= visible {
                break;
            }
            // Fill a circle of radius grape_r centered at (gx, gy).
            for dy in -grape_r..=grape_r {
                for dx in -grape_r..=grape_r {
                    // Dot-aspect: vertical dots are narrower visually.
                    if dx * dx + dy * dy * 4 <= grape_r * grape_r * 4 {
                        draw::dot_i(grid, gx + dx, gy + dy);
                    }
                }
            }
            // Highlight: one bright dot near top-left of each grape.
            draw::dot_i(grid, gx - grape_r / 2, gy.saturating_sub(grape_r / 2 + 1));
        }
        // Tint visible grapes.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx in 0..filled_cells.min(cells_w) {
                let t = cx as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── WatermelonSlice ───────────────────────────────────────────────────────────
// A semicircular watermelon slice: rind drawn at top, flesh fills from the flat
// bottom edge upward with eased. Black seed dots appear as flesh fills in.

struct WatermelonSlice;
impl ProgressStyle for WatermelonSlice {
    fn name(&self) -> &str {
        "watermelon-slice"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Watermelon slice: flesh fills from flat bottom up; seeds appear as it ripens"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let base = (h - 1) as i32;
        let cx = (w / 2) as i32;
        let r = (w / 2).saturating_sub(1).max(1) as f32;
        // Flat bottom edge (the cut face).
        draw::hline(grid, 0, w - 1, h - 1);
        // Outer rind arc (upper semicircle).
        let arc_steps = (r * PI) as usize + 1;
        for i in 0..=arc_steps {
            let theta = i as f32 / arc_steps.max(1) as f32 * PI; // 0..PI (upper half)
            let px = cx + (r * theta.cos()) as i32;
            let py = base - (r * 0.5 * theta.sin()) as i32;
            draw::dot_i(grid, px, py);
            // Rind thickness: inner rind ring.
            let inner_r = r * 0.88;
            let ipx = cx + (inner_r * theta.cos()) as i32;
            let ipy = base - (inner_r * 0.5 * theta.sin()) as i32;
            draw::dot_i(grid, ipx, ipy);
        }
        // Fill flesh from base upward.
        let fill_h = (ctx.eased * (h as f32 - 1.0)).round() as i32;
        for dy in 0..fill_h {
            let y = base - dy;
            if y < 0 {
                break;
            }
            // Half-width of the arc at this height.
            let norm = dy as f32 / (h.max(1) as f32 - 1.0);
            let half_w = (r * (1.0 - (1.0 - norm * 2.0).powi(2)).sqrt() * 0.95) as i32;
            draw::hline(
                grid,
                (cx - half_w).max(0) as usize,
                (cx + half_w).min(w as i32 - 1) as usize,
                y as usize,
            );
        }
        // Seeds: fixed positions, appear once the fill reaches their row.
        // Seed positions as (fraction_from_center_x, fraction_from_base_y).
        let seeds: &[(f32, f32)] = &[
            (-0.35, 0.25),
            (0.0, 0.2),
            (0.35, 0.25),
            (-0.55, 0.45),
            (-0.18, 0.50),
            (0.18, 0.50),
            (0.55, 0.45),
            (-0.38, 0.68),
            (0.38, 0.68),
        ];
        for &(sx_frac, sy_frac) in seeds {
            let sx = cx + (sx_frac * r * 0.9) as i32;
            let sy = base - (sy_frac * (h as f32 - 1.0)) as i32;
            // Seed is visible only if the fill has reached its row.
            let seed_fill_needed = sy_frac;
            if ctx.eased >= seed_fill_needed {
                draw::dot_i(grid, sx, sy);
                draw::dot_i(grid, sx + 1, sy);
                draw::dot_i(grid, sx, sy - 1);
            }
        }
        // Tint filled portion.
        let (cells_w, cells_h) = grid.dimensions();
        let fill_cells = (ctx.eased * cells_h as f32) as usize;
        for cy in (cells_h.saturating_sub(fill_cells))..cells_h {
            for cx2 in 0..cells_w {
                let t = cx2 as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy, cx2, cx2, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── StrawberrySeeds ───────────────────────────────────────────────────────────
// A strawberry outline: heart-like shape fills from bottom (tip) upward.
// Seed dimples (tiny dots scattered on the surface) appear as the fill rises.

struct StrawberrySeeds;
impl ProgressStyle for StrawberrySeeds {
    fn name(&self) -> &str {
        "strawberry-seeds"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Strawberry body fills tip-to-top; seed dimples appear across the surface"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let cx = (w / 2) as i32;
        let tip_y = (h - 1) as i32;
        let top_y = 0i32;
        // Strawberry shape: an inverted teardrop. Width at each row:
        //   - narrow at the tip (bottom), widens going up, then slightly narrows at top for calyx.
        let body_h = h as i32;
        let max_hw = (w / 2).saturating_sub(1).max(1) as f32;
        // Fill from tip upward — fill_rows is how many rows are filled.
        let fill_rows = (ctx.eased * body_h as f32).round() as i32;
        for dy in 0..body_h {
            let y = tip_y - dy;
            if y < top_y {
                break;
            }
            // Normalized position: 0 at tip, 1 at top.
            let t = dy as f32 / body_h.max(1) as f32;
            // Width profile: 0 at tip, grows to max at 60%, slight indent at top.
            let hw = if t < 0.6 {
                max_hw * (t / 0.6).sqrt()
            } else {
                max_hw * (1.0 - (t - 0.6) / 0.4 * 0.15)
            };
            let hw_i = hw.round() as i32;
            // Draw outline.
            draw::dot_i(grid, cx - hw_i, y);
            draw::dot_i(grid, cx + hw_i, y);
            // Fill if within progress.
            if dy < fill_rows {
                for dx in -hw_i..=hw_i {
                    draw::dot_i(grid, cx + dx, y);
                }
            }
        }
        // Calyx (leaves) at the top: a few dots above the fruit.
        draw::dot_i(grid, cx, top_y - 1);
        draw::dot_i(grid, cx - 2, top_y - 1);
        draw::dot_i(grid, cx + 2, top_y - 1);
        draw::dot_i(grid, cx - 1, top_y - 2);
        draw::dot_i(grid, cx + 1, top_y - 2);
        // Seeds: scattered positions as (t_across, t_up_from_tip).
        let seeds: &[(f32, f32)] = &[
            (-0.3, 0.2),
            (0.3, 0.2),
            (0.0, 0.3),
            (-0.5, 0.4),
            (0.5, 0.4),
            (-0.2, 0.5),
            (0.2, 0.5),
            (-0.45, 0.65),
            (0.45, 0.65),
            (0.0, 0.6),
            (-0.3, 0.75),
            (0.3, 0.75),
            (0.0, 0.85),
        ];
        for &(sx_frac, sy_frac) in seeds {
            let t = sy_frac;
            // Width at this row.
            let hw_at = if t < 0.6 {
                max_hw * (t / 0.6).sqrt()
            } else {
                max_hw * (1.0 - (t - 0.6) / 0.4 * 0.15)
            };
            let sx = cx + (sx_frac * hw_at) as i32;
            let sy = tip_y - (sy_frac * body_h as f32) as i32;
            // Seed is visible once the fill has reached its row.
            if ctx.eased >= sy_frac && sy >= top_y {
                draw::dot_i(grid, sx, sy);
            }
        }
        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        let fill_cells = (ctx.eased * cells_h as f32) as usize;
        for cy in (cells_h.saturating_sub(fill_cells))..cells_h {
            for cx2 in 0..cells_w {
                let t = cx2 as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy, cx2, cx2, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── PineappleLattice ──────────────────────────────────────────────────────────
// Pineapple rind: a diamond lattice pattern fills the bar column by column.
// Each cell of the lattice is a small diamond of dots. The pattern sweeps left
// to right with eased.

struct PineappleLattice;
impl ProgressStyle for PineappleLattice {
    fn name(&self) -> &str {
        "pineapple-lattice"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Pineapple diamond-lattice rind filling column by column across the bar"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        // Diamond cell size: spacing between lattice nodes.
        let cell_x = 4usize;
        let cell_y = 3usize;
        // How many dot columns are revealed.
        let filled_w = (ctx.eased * w as f32).round() as usize;
        // Time-driven shimmer: the whole pattern slowly drifts.
        let drift_y = ((ctx.time * 1.5) as usize) % cell_y.max(1);
        // Draw diamond lattice nodes and connecting lines.
        // Lattice is a grid of points at (col*cell_x, row*cell_y + offset_for_odd_cols).
        let cols = w / cell_x + 2;
        let rows = h / cell_y + 2;
        for col in 0..cols {
            let cx2 = col * cell_x;
            if cx2 >= filled_w {
                break;
            }
            for row in 0..rows {
                // Alternate rows are offset by cell_y/2 for each column to make diamonds.
                let y_off = if col % 2 == 0 { 0 } else { cell_y / 2 };
                let ry = (row * cell_y + y_off + drift_y) % (h + cell_y);
                if ry >= h {
                    continue;
                }
                // Diamond node: a small cross.
                draw::dot(grid, cx2.min(w - 1), ry);
                if cx2 + 1 < filled_w {
                    draw::dot(grid, (cx2 + 1).min(w - 1), ry);
                }
                // Connect to adjacent nodes to form diamond edges.
                // Right neighbor (same row, next column).
                if cx2 + cell_x < filled_w {
                    let next_y_off = if (col + 1) % 2 == 0 { 0 } else { cell_y / 2 };
                    // Draw a diagonal line bridging the two nodes.
                    let nx = cx2 + cell_x;
                    let ny = (row * cell_y + next_y_off + drift_y) % (h + cell_y);
                    if ny < h {
                        let mid_x = (cx2 + nx) / 2;
                        let mid_y = if ry < ny {
                            (ry + ny) / 2
                        } else {
                            (ny + ry) / 2
                        };
                        draw::dot(grid, mid_x.min(w - 1), mid_y.min(h - 1));
                    }
                }
            }
        }
        // Crown at the top: a few upward spikes above the bar.
        let crown_cols = (w / 6).max(1);
        for s in 0..crown_cols {
            let sx = s * 6 + 3;
            if sx >= w {
                break;
            }
            draw::dot_i(grid, sx as i32, -1); // above bar, ignored safely
            draw::dot(grid, sx.min(w - 1), 0);
        }
        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx_col in 0..filled_cells.min(cells_w) {
                let t = cx_col as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy, cx_col, cx_col, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── CherrySwing ───────────────────────────────────────────────────────────────
// Two cherries hanging on a Y-shaped stem. The pair swings as a pendulum
// driven by ctx.time. Progress fills the cherries — each is a small filled
// circle that fills from empty to solid as eased crosses 0.5 (one cherry) and
// 1.0 (both cherries), with the bar background showing stem trail.

struct CherrySwing;
impl ProgressStyle for CherrySwing {
    fn name(&self) -> &str {
        "cherry-swing"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Cherry pair on a Y-stem swinging as a pendulum; each fruit fills with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        // Pendulum swing: amplitude decreases as progress completes (settling).
        let settle = 1.0 - ctx.eased * 0.7;
        let swing = (ctx.time * 2.5).sin() * settle * (h as f32 * 0.2);
        // The pair is centered horizontally; at progress=0 they're near the top,
        // at progress=1 they're pulled down (ripe and hanging heavy).
        let cx = (w / 2) as i32;
        let stem_top_y = 0i32;
        let stem_fork_y = (h as f32 * 0.35) as i32;
        // Cherry centers below fork.
        let cherry_r = (h.min(w / 2) / 5).max(1) as i32;
        let cherry_y = stem_fork_y + cherry_r * 2 + (swing.abs() as i32).min(h as i32 / 4);
        let cherry_left_x = cx - cherry_r * 2 - 1;
        let cherry_right_x = cx + cherry_r * 2 + 1;
        // Horizontal shift from pendulum swing.
        let sx = swing as i32;
        // Draw the main stem from top to fork.
        for y in stem_top_y..=stem_fork_y {
            draw::dot_i(grid, cx + sx * y / (stem_fork_y.max(1)), y);
        }
        // Left and right sub-stems from fork to each cherry.
        let fork_x = cx + sx;
        let clx = cherry_left_x + sx;
        let crx = cherry_right_x + sx;
        let cy2 = cherry_y;
        // Left sub-stem.
        for t in 0..=8 {
            let t_f = t as f32 / 8.0;
            let px = fork_x + ((clx - fork_x) as f32 * t_f) as i32;
            let py = stem_fork_y + ((cy2 - cherry_r - stem_fork_y) as f32 * t_f) as i32;
            draw::dot_i(grid, px, py);
        }
        // Right sub-stem.
        for t in 0..=8 {
            let t_f = t as f32 / 8.0;
            let px = fork_x + ((crx - fork_x) as f32 * t_f) as i32;
            let py = stem_fork_y + ((cy2 - cherry_r - stem_fork_y) as f32 * t_f) as i32;
            draw::dot_i(grid, px, py);
        }
        // Draw the cherry circles. First cherry filled at eased >= 0.5, second at 1.0.
        let left_fill = (ctx.eased * 2.0).clamp(0.0, 1.0);
        let right_fill = ((ctx.eased - 0.5) * 2.0).clamp(0.0, 1.0);
        for dr in -cherry_r..=cherry_r {
            for dc in -cherry_r..=cherry_r {
                let dist2 = dr * dr * 4 + dc * dc;
                if dist2 > cherry_r * cherry_r * 4 {
                    continue;
                }
                // Outline always.
                let on_edge = dist2 > (cherry_r - 1) * (cherry_r - 1) * 4;
                // Left cherry.
                let in_left_fill = (dr as f32 / cherry_r as f32 + 1.0) / 2.0 <= left_fill;
                if on_edge || in_left_fill {
                    draw::dot_i(grid, clx + dc, cy2 + dr);
                }
                // Right cherry.
                let in_right_fill = (dr as f32 / cherry_r as f32 + 1.0) / 2.0 <= right_fill;
                if on_edge || in_right_fill {
                    draw::dot_i(grid, crx + dc, cy2 + dr);
                }
            }
        }
        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_t in 0..cells_h {
            for cx_t in 0..cells_w {
                let t = ctx.eased;
                draw::tint_row(grid, cy_t, cx_t, cx_t, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── JuiceSqueeze ──────────────────────────────────────────────────────────────
// A citrus fruit in the top half compresses vertically (squish) as progress
// rises. The juice level in a glass below rises to match. The squeeze is
// animated with a gentle pulsing vibration from ctx.time.

struct JuiceSqueeze;
impl ProgressStyle for JuiceSqueeze {
    fn name(&self) -> &str {
        "juice-squeeze"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Fruit compresses top-down while juice level rises in a glass below"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let cx = (w / 2) as i32;
        // Divide bar: top half = fruit, bottom half = glass.
        let split_y = (h / 2) as i32;
        // Fruit: an ellipse that gets compressed vertically with progress.
        // At eased=0 it's a full circle; at eased=1 it's very flat.
        let pulse = (ctx.time * 8.0).sin() * 0.05 * ctx.eased;
        let fruit_rx = (w / 4).max(1) as f32;
        let fruit_ry_full = (split_y as f32 * 0.45).max(1.0);
        let squeeze = (ctx.eased + pulse).clamp(0.0, 1.0);
        let fruit_ry = fruit_ry_full * (1.0 - squeeze * 0.75).max(0.1);
        let fruit_cy = (split_y as f32 * 0.5) as i32;
        // Draw fruit ellipse outline.
        let arc_steps = (fruit_rx * 2.0 * PI) as usize + 1;
        for i in 0..=arc_steps {
            let theta = i as f32 / arc_steps.max(1) as f32 * 2.0 * PI;
            let px = cx + (fruit_rx * theta.cos()) as i32;
            let py = fruit_cy + (fruit_ry * theta.sin() * 0.5) as i32;
            draw::dot_i(grid, px, py);
        }
        // Fill the fruit.
        let rx_i = fruit_rx as i32 + 1;
        let ry_i = (fruit_ry * 0.5) as i32 + 1;
        for dy in -ry_i..=ry_i {
            for dx in -rx_i..=rx_i {
                if (dx as f32 / fruit_rx).powi(2) + (dy as f32 / (fruit_ry * 0.5).max(0.1)).powi(2)
                    <= 1.0
                {
                    draw::dot_i(grid, cx + dx, fruit_cy + dy);
                }
            }
        }
        // Drops falling from fruit to glass (time-animated).
        let drop_t = (ctx.time * 3.0).fract();
        if ctx.eased > 0.05 {
            let drop_y = (fruit_cy + ry_i + 1) as f32
                + drop_t * (split_y as f32 - fruit_cy as f32 - ry_i as f32 - 1.0);
            draw::dot_i(grid, cx, drop_y as i32);
            draw::dot_i(grid, cx, drop_y as i32 + 1);
        }
        // Glass outline: a trapezoid — slightly wider at top.
        let glass_top_y = split_y + 1;
        let glass_bot_y = h as i32 - 1;
        let glass_h = (glass_bot_y - glass_top_y).max(1);
        let glass_top_hw = (w as i32 / 3).max(1);
        let glass_bot_hw = (glass_top_hw as f32 * 0.8) as i32;
        // Left and right walls (angled inward).
        for y in glass_top_y..=glass_bot_y {
            let t_g = (y - glass_top_y) as f32 / glass_h as f32;
            let hw = glass_top_hw - ((glass_top_hw - glass_bot_hw) as f32 * t_g) as i32;
            draw::dot_i(grid, cx - hw, y);
            draw::dot_i(grid, cx + hw, y);
        }
        // Bottom of glass.
        draw::hline(
            grid,
            (cx - glass_bot_hw).max(0) as usize,
            (cx + glass_bot_hw).min(w as i32 - 1) as usize,
            glass_bot_y as usize,
        );
        // Juice fill inside glass, rising with progress.
        let juice_h = (ctx.eased * glass_h as f32).round() as i32;
        for y in (glass_bot_y - juice_h).max(glass_top_y)..=glass_bot_y {
            let t_g = (y - glass_top_y) as f32 / glass_h as f32;
            let hw =
                (glass_top_hw - ((glass_top_hw - glass_bot_hw) as f32 * t_g) as i32 - 1).max(0);
            draw::hline(
                grid,
                (cx - hw).max(0) as usize,
                (cx + hw).min(w as i32 - 1) as usize,
                y as usize,
            );
        }
        // Juice surface ripple (time-animated).
        let juice_top_y = (glass_bot_y - juice_h).max(glass_top_y);
        if juice_h > 0 {
            let ripple = (ctx.time * 5.0).sin() * 1.5;
            let t_g = (juice_top_y - glass_top_y) as f32 / glass_h as f32;
            let hw =
                (glass_top_hw - ((glass_top_hw - glass_bot_hw) as f32 * t_g) as i32 - 1).max(0);
            draw::dot_i(grid, cx + ripple as i32, juice_top_y - 1);
            let _ = hw; // suppress unused warning — hw used for draw call above
        }
        // Tint.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_t in 0..cells_h {
            let half = cells_h / 2;
            for cx_t in 0..cells_w {
                let t = if cy_t >= half {
                    ctx.eased
                } else {
                    cx_t as f32 / cells_w.max(1) as f32
                };
                draw::tint_row(grid, cy_t, cx_t, cx_t, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── BerryPop ──────────────────────────────────────────────────────────────────
// A cluster of berries pops in one by one. Each berry (small filled circle)
// appears with a brief outward "pop" bloom that is animated with ctx.time.
// The positions are spread across the bar area in a scattered pattern.

struct BerryPop;
impl ProgressStyle for BerryPop {
    fn name(&self) -> &str {
        "berry-pop"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Berry cluster appearing one by one — each fruit pops in with a radial bloom"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        // Pre-computed scattered berry positions as (x_frac, y_frac) in [0,1].
        // Laid out to avoid overlap for small sizes.
        let positions: &[(f32, f32)] = &[
            (0.08, 0.5),
            (0.18, 0.25),
            (0.18, 0.75),
            (0.30, 0.5),
            (0.40, 0.2),
            (0.40, 0.8),
            (0.50, 0.5),
            (0.60, 0.3),
            (0.60, 0.7),
            (0.72, 0.5),
            (0.82, 0.25),
            (0.82, 0.75),
            (0.92, 0.5),
        ];
        let n_berries = positions.len();
        let berry_r = (w.min(h * 2) / (n_berries + 2)).max(1).min(3) as i32;
        let visible = (ctx.eased * n_berries as f32).ceil() as usize;
        // Each berry's progress fraction threshold.
        for (i, &(xf, yf)) in positions.iter().enumerate() {
            if i >= visible {
                break;
            }
            let bx = (xf * (w.saturating_sub(1)) as f32) as i32;
            let by = (yf * (h.saturating_sub(1)) as f32) as i32;
            // Pop animation: how long ago this berry appeared (in seconds worth of progress).
            let berry_threshold = i as f32 / n_berries as f32;
            let age = (ctx.eased - berry_threshold) * n_berries as f32; // 0..1 within its own slot
                                                                        // Map age to a pop bloom radius (blooms quickly, then settles).
            let bloom_r = if age < 0.3 {
                (berry_r as f32 + (0.3 - age) / 0.3 * berry_r as f32 * 1.5) as i32
            } else {
                berry_r
            };
            // Draw bloom ring.
            if bloom_r > berry_r {
                let bloom_steps = (bloom_r as f32 * 2.0 * PI) as usize + 1;
                for b in 0..bloom_steps {
                    let theta = b as f32 / bloom_steps.max(1) as f32 * 2.0 * PI;
                    let px = bx + (bloom_r as f32 * theta.cos()) as i32;
                    let py = by + (bloom_r as f32 * 0.5 * theta.sin()) as i32;
                    draw::dot_i(grid, px, py);
                }
            }
            // Draw solid berry.
            for dy in -berry_r..=berry_r {
                for dx in -berry_r..=berry_r {
                    if dx * dx + dy * dy * 4 <= berry_r * berry_r * 4 {
                        draw::dot_i(grid, bx + dx, by + dy);
                    }
                }
            }
            // Highlight dot (top-left of berry).
            draw::dot_i(grid, bx - berry_r / 2, by.saturating_sub(berry_r / 2 + 1));
            // Stem nub above each berry.
            draw::dot_i(grid, bx, by - berry_r - 1);
        }
        // Tint visible portion.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx2 in 0..filled_cells.min(cells_w) {
                let t = cx2 as f32 / cells_w.max(1) as f32;
                draw::tint_row(grid, cy, cx2, cx2, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── KiwiRings ────────────────────────────────────────────────────────────────
// Kiwi/lemon cross-section: concentric elliptic rings fill outward from center
// as eased increases. The center is solid (seed zone), then each subsequent
// ring appears until the outer rind ring is reached. Time animates a slow pulse.

struct KiwiRings;
impl ProgressStyle for KiwiRings {
    fn name(&self) -> &str {
        "kiwi-rings"
    }
    fn theme(&self) -> &str {
        "fruits"
    }
    fn describe(&self) -> &str {
        "Kiwi cross-section: concentric elliptic rings fill outward from center to rind"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        // Outer radius fits inside the bar.
        let max_r = (w.min(h * 2) / 2).saturating_sub(1).max(1) as f32;
        let n_rings = 6usize;
        // Pulse: rings gently breathe with time.
        let pulse = 1.0 + (ctx.time * 2.0).sin() * 0.03;
        // Number of rings drawn grows with eased (innermost first).
        let rings_visible = (ctx.eased * n_rings as f32).ceil() as usize;
        // Draw rings from outer to inner, only those within eased.
        for ring in 0..rings_visible.min(n_rings) {
            // Ring index from inside (0 = innermost/center).
            let r_frac = (ring + 1) as f32 / n_rings as f32;
            let r = max_r * r_frac * pulse;
            // Draw ellipse ring.
            let steps = (r * 2.0 * PI) as usize + 4;
            for i in 0..steps {
                let theta = i as f32 / steps.max(1) as f32 * 2.0 * PI;
                let px = cx + (r * theta.cos()) as i32;
                let py = cy + (r * 0.5 * theta.sin()) as i32;
                draw::dot_i(grid, px, py);
            }
            // Fill interior of ring (only for innermost ring — the seed center).
            if ring == 0 {
                let ri = r as i32;
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        if (dx as f32 / r).powi(2) + (dy as f32 * 2.0 / r).powi(2) <= 1.0 {
                            draw::dot_i(grid, cx + dx, cy + dy);
                        }
                    }
                }
            }
        }
        // Seed dots in the center zone (appear at eased > 0.1).
        if ctx.eased > 0.1 {
            let seed_offsets: &[(i32, i32)] = &[
                (0, -1),
                (1, 0),
                (0, 1),
                (-1, 0),
                (2, -1),
                (-2, -1),
                (2, 1),
                (-2, 1),
            ];
            for &(dx, dy) in seed_offsets {
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }
        // Outer rind (always drawn for context).
        {
            let r = max_r;
            let steps = (r * 2.0 * PI) as usize + 4;
            for i in 0..steps {
                let theta = i as f32 / steps.max(1) as f32 * 2.0 * PI;
                let px = cx + (r * theta.cos()) as i32;
                let py = cy + (r * 0.5 * theta.sin()) as i32;
                draw::dot_i(grid, px, py);
                // Double rind line.
                let r2 = max_r + 1.0;
                let px2 = cx + (r2 * theta.cos()) as i32;
                let py2 = cy + (r2 * 0.5 * theta.sin()) as i32;
                draw::dot_i(grid, px2, py2);
            }
        }
        // Tint: color by ring depth.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_t in 0..cells_h {
            for cx_t in 0..cells_w {
                // t = distance from center of grid, normalized.
                let dx = (cx_t as f32 + 0.5) - cells_w as f32 / 2.0;
                let dy = (cy_t as f32 + 0.5) - cells_h as f32 / 2.0;
                let dist = (dx * dx + dy * dy * 4.0).sqrt();
                let max_dist = ((cells_w as f32 / 2.0).powi(2) + (cells_h as f32).powi(2)).sqrt();
                let t = (dist / max_dist.max(1.0)).clamp(0.0, 1.0) * ctx.eased;
                draw::tint_row(grid, cy_t, cx_t, cx_t, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}
