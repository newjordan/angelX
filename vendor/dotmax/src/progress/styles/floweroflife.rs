//! Flower-of-Life sacred geometry progress styles.
//!
//! Eleven structurally distinct styles drawn from the Flower-of-Life family:
//! overlapping-circle constructions on a hexagonal lattice, their intersection
//! graphs (Metatron's Cube), the Kabbalistic Tree of Life, a 64-tetrahedron
//! triangular projection, and a time-rotating torus-flower. Every style
//! reveals itself as `ctx.eased` rises from 0 → 1 and breathes or rotates
//! via `ctx.time`.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ── shared geometry helpers ────────────────────────────────────────────────

/// Plot a circle (parametric) in dot-space.
/// `cx,cy` are the centre in dots (f32); `r` is the radius in dots.
/// Steps are bounded to avoid overdraw on tiny grids.
fn plot_circle(grid: &mut BrailleGrid, cx: f32, cy: f32, r: f32) {
    if r < 0.5 {
        draw::dot_i(grid, cx.round() as i32, cy.round() as i32);
        return;
    }
    // Circumference ≈ 2πr; sample at ≥2 dots per step so we don't miss any dot.
    let steps = ((2.0 * PI * r).ceil() as usize * 2).max(8).min(2048);
    for i in 0..steps {
        let angle = 2.0 * PI * i as f32 / steps as f32;
        let px = cx + r * angle.cos();
        let py = cy + r * angle.sin();
        draw::dot_i(grid, px.round() as i32, py.round() as i32);
    }
}

/// Draw a line between two dot-space points using Bresenham-style stepping.
fn plot_line(grid: &mut BrailleGrid, x0: f32, y0: f32, x1: f32, y1: f32) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = (dx.abs().max(dy.abs()).ceil() as usize).max(1).min(4096);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let px = x0 + dx * t;
        let py = y0 + dy * t;
        draw::dot_i(grid, px.round() as i32, py.round() as i32);
    }
}

/// Hex-lattice unit vectors (6 directions, 60° apart).
fn hex_offset(ring_angle_index: usize, radius: f32) -> (f32, f32) {
    let a = ring_angle_index as f32 * PI / 3.0;
    (radius * a.cos(), radius * a.sin())
}

/// The 7 Seed-of-Life circle centres: centre + 6 around it at distance `r`.
/// Returned in order: [centre, 0°, 60°, 120°, 180°, 240°, 300°].
fn seed_centres(cx: f32, cy: f32, r: f32) -> [(f32, f32); 7] {
    let mut out = [(0f32, 0f32); 7];
    out[0] = (cx, cy);
    for i in 0..6 {
        let (dx, dy) = hex_offset(i, r);
        out[i + 1] = (cx + dx, cy + dy);
    }
    out
}

/// All 19 Flower-of-Life centres: Seed (7) + outer ring (12).
fn flower_centres(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = Vec::with_capacity(19);
    // Inner 7
    for c in seed_centres(cx, cy, r) {
        v.push(c);
    }
    // Outer 12: two rings of 6 each at √3·r and 2·r, offset 30°
    for i in 0..6 {
        let a = i as f32 * PI / 3.0 + PI / 6.0;
        let d = 3f32.sqrt() * r;
        v.push((cx + d * a.cos(), cy + d * a.sin()));
    }
    for i in 0..6 {
        let (dx, dy) = hex_offset(i, 2.0 * r);
        v.push((cx + dx, cy + dy));
    }
    v
}

/// 13 Fruit-of-Life centres: centre + 6 at r + 6 at 2r (same hex axes).
fn fruit_centres(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = Vec::with_capacity(13);
    v.push((cx, cy));
    for i in 0..6 {
        let (dx, dy) = hex_offset(i, r);
        v.push((cx + dx, cy + dy));
    }
    for i in 0..6 {
        let (dx, dy) = hex_offset(i, 2.0 * r);
        v.push((cx + dx, cy + dy));
    }
    v
}

// ── All styles ─────────────────────────────────────────────────────────────

/// All styles in the `floweroflife` theme.
///
/// Returns eleven structurally distinct sacred-geometry progress styles,
/// each revealing itself as `eased` rises and animating via `time`.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(VesicaPiscis),
        Box::new(SeedOfLife),
        Box::new(FlowerOfLife),
        Box::new(EggOfLife),
        Box::new(FruitOfLife),
        Box::new(MetatronsCube),
        Box::new(GermOfLife),
        Box::new(TripodOfLife),
        Box::new(TreeOfLife),
        Box::new(TetraGrid),
        Box::new(TorusFlower),
    ]
}

// ── 1. Vesica Piscis ───────────────────────────────────────────────────────

/// Two overlapping circles; the shared lens (vesica) forms as `eased` rises.
struct VesicaPiscis;
impl ProgressStyle for VesicaPiscis {
    fn name(&self) -> &str {
        "vesica-piscis"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "Two circles overlap to reveal the sacred lens; arc segments appear with eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let r = (cx.min(cy) * 0.45).max(1.0);

        // Separation between the two circle centres, grows with eased.
        // At eased=0: coincident. At eased=1: separated by r (classic vesica).
        let sep = r * ctx.eased;
        let lx = cx - sep / 2.0;
        let rx = cx + sep / 2.0;

        // Draw the two circles, breathing via time.
        let breathe = 1.0 + 0.04 * (ctx.time * 1.2).sin();
        let rr = r * breathe;
        plot_circle(grid, lx, cy, rr);
        if ctx.eased > 0.05 {
            plot_circle(grid, rx, cy, rr);
        }

        // Color: tint the left half one shade, right half another.
        let (cw, ch) = grid.dimensions();
        let mid_cell = cw / 2;
        for cy_cell in 0..ch {
            if mid_cell > 0 {
                draw::tint_row(
                    grid,
                    cy_cell,
                    0,
                    mid_cell.saturating_sub(1),
                    ctx.palette.start,
                );
                draw::tint_row(
                    grid,
                    cy_cell,
                    mid_cell,
                    cw.saturating_sub(1),
                    ctx.palette.end,
                );
            }
        }
        Ok(())
    }
}

// ── 2. Seed of Life ────────────────────────────────────────────────────────

/// Central circle + 6 petals revealed one at a time; rotates with time.
struct SeedOfLife;
impl ProgressStyle for SeedOfLife {
    fn name(&self) -> &str {
        "seed-of-life"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "Seven circles of the Seed of Life appear petal-by-petal as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let r = (cx.min(cy) * 0.38).max(1.0);

        // Rotation offset from time.
        let rot = ctx.time * 0.25;
        // How many of the 7 circles to reveal (0 → 7).
        let reveal = (ctx.eased * 7.0).ceil() as usize;

        let centres = seed_centres(cx, cy, r);
        for (i, &(px, py)) in centres.iter().enumerate().take(reveal.min(7)) {
            // Rotate each petal centre around cx,cy.
            let (ox, oy) = (px - cx, py - cy);
            let rx_c = ox * rot.cos() - oy * rot.sin() + cx;
            let ry_c = ox * rot.sin() + oy * rot.cos() + cy;
            plot_circle(grid, rx_c, ry_c, r);

            // Tint by petal index.
            let t = i as f32 / 6.0;
            let color = ctx.palette.sample(t);
            let (cw, ch) = grid.dimensions();
            let cell_x = (rx_c / 2.0).round() as usize;
            let cell_y = (ry_c / 4.0).round() as usize;
            if cell_x < cw && cell_y < ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }
        Ok(())
    }
}

// ── 3. Flower of Life ─────────────────────────────────────────────────────

/// Full 19-circle pattern with boundary ring; circles added as eased rises.
struct FlowerOfLife;
impl ProgressStyle for FlowerOfLife {
    fn name(&self) -> &str {
        "flower-of-life"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "All 19 overlapping circles of the Flower of Life bloom with progress, bounded by an outer ring"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        // Fit: 19 circles span ≈3r radius from centre; outer ring at 2r.
        let r = (cx.min(cy) / 3.2).max(1.0);

        let centres = flower_centres(cx, cy, r);
        let total = centres.len(); // 19
        let reveal = (ctx.eased * total as f32).ceil() as usize;

        for (i, &(px, py)) in centres.iter().enumerate().take(reveal.min(total)) {
            plot_circle(grid, px, py, r);
            let t = i as f32 / (total - 1).max(1) as f32;
            let color = ctx.palette.sample(t);
            let (cw, ch) = grid.dimensions();
            let cell_x = (px / 2.0).round() as usize;
            let cell_y = (py / 4.0).round() as usize;
            if cell_x < cw && cell_y < ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }

        // Outer boundary circle at 2r.
        if ctx.eased >= 0.95 {
            plot_circle(grid, cx, cy, 2.0 * r);
        }
        Ok(())
    }
}

// ── 4. Egg of Life ────────────────────────────────────────────────────────

/// 7 non-overlapping circles at alternate Flower-of-Life positions; pulses with time.
struct EggOfLife;
impl ProgressStyle for EggOfLife {
    fn name(&self) -> &str {
        "egg-of-life"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "Seven separated circles in egg-of-life formation; radius pulses with time as progress fills"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        // Egg of life: 7 circles placed at seed positions but non-overlapping (spacing=2r).
        let r = (cx.min(cy) * 0.22).max(1.0);
        let spacing = r * 2.2;

        let reveal = (ctx.eased * 7.0).ceil() as usize;
        let pulse = 1.0 + 0.06 * (ctx.time * 1.8).sin();

        let mut centres = [(0f32, 0f32); 7];
        centres[0] = (cx, cy);
        for i in 0..6 {
            let (dx, dy) = hex_offset(i, spacing);
            centres[i + 1] = (cx + dx, cy + dy);
        }

        for (i, &(px, py)) in centres.iter().enumerate().take(reveal.min(7)) {
            plot_circle(grid, px, py, r * pulse);
            // Fill inner dot for egg effect.
            draw::dot_i(grid, px.round() as i32, py.round() as i32);
            let t = i as f32 / 6.0;
            let color = ctx.palette.sample(t);
            let (cw, ch) = grid.dimensions();
            let cell_x = (px / 2.0).round() as usize;
            let cell_y = (py / 4.0).round() as usize;
            if cell_x < cw && cell_y < ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }
        Ok(())
    }
}

// ── 5. Fruit of Life ──────────────────────────────────────────────────────

/// 13-circle pattern (the Fruit of Life); circles connected by lines as eased rises.
struct FruitOfLife;
impl ProgressStyle for FruitOfLife {
    fn name(&self) -> &str {
        "fruit-of-life"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "13 circles of the Fruit of Life; connecting lines drawn between all centres with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let r = (cx.min(cy) / 2.5).max(1.0);
        let cr = (r * 0.18).max(1.0);

        let centres = fruit_centres(cx, cy, r);
        let total = centres.len(); // 13

        // Phase 1 (eased 0→0.5): reveal circles one by one.
        // Phase 2 (eased 0.5→1): draw connecting lines between all centres.
        let circle_frac = (ctx.eased * 2.0).min(1.0);
        let line_frac = ((ctx.eased - 0.5) * 2.0).max(0.0).min(1.0);

        let reveal_circles = (circle_frac * total as f32).ceil() as usize;
        for (i, &(px, py)) in centres.iter().enumerate().take(reveal_circles.min(total)) {
            plot_circle(grid, px, py, cr);
            let t = i as f32 / (total - 1).max(1) as f32;
            draw::tint_row(
                grid,
                (py / 4.0) as usize,
                (px / 2.0) as usize,
                (px / 2.0) as usize,
                ctx.palette.sample(t),
            );
        }

        // Connecting lines (all pairs = 78 edges).
        let all_pairs: Vec<(usize, usize)> = (0..total)
            .flat_map(|a| (a + 1..total).map(move |b| (a, b)))
            .collect();
        let reveal_lines = (line_frac * all_pairs.len() as f32).round() as usize;
        for &(a, b) in all_pairs.iter().take(reveal_lines) {
            let (ax, ay) = centres[a];
            let (bx, by) = centres[b];
            plot_line(grid, ax, ay, bx, by);
        }
        Ok(())
    }
}

// ── 6. Metatron's Cube ────────────────────────────────────────────────────

/// 13 Fruit-of-Life centres; ALL straight lines between them drawn with eased.
struct MetatronsCube;
impl ProgressStyle for MetatronsCube {
    fn name(&self) -> &str {
        "metatrons-cube"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "Metatron's Cube: 13 nodes with all 78 connecting edges revealed progressively"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let r = (cx.min(cy) / 2.4).max(1.0);
        // Slow rotation via time.
        let rot = ctx.time * 0.15;

        let raw = fruit_centres(cx, cy, r);
        // Rotate all centres around cx,cy.
        let centres: Vec<(f32, f32)> = raw
            .iter()
            .map(|&(px, py)| {
                let ox = px - cx;
                let oy = py - cy;
                (
                    ox * rot.cos() - oy * rot.sin() + cx,
                    ox * rot.sin() + oy * rot.cos() + cy,
                )
            })
            .collect();

        let total = centres.len();
        let all_pairs: Vec<(usize, usize)> = (0..total)
            .flat_map(|a| (a + 1..total).map(move |b| (a, b)))
            .collect();
        let reveal = (ctx.eased * all_pairs.len() as f32).round() as usize;

        for &(a, b) in all_pairs.iter().take(reveal) {
            let (ax, ay) = centres[a];
            let (bx, by) = centres[b];
            let t = a as f32 / (total - 1).max(1) as f32;
            let _ = t; // color applied per row via tint below
            plot_line(grid, ax, ay, bx, by);
        }

        // Draw node dots on top.
        let cr = 1.5f32.max(r * 0.12);
        for &(px, py) in &centres {
            plot_circle(grid, px, py, cr);
        }

        // Gradient tint across all columns.
        let (cw, ch) = grid.dimensions();
        for cy_cell in 0..ch {
            for cx_cell in 0..cw {
                let t = cx_cell as f32 / cw.max(1) as f32;
                draw::tint_row(grid, cy_cell, cx_cell, cx_cell, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── 7. Germ of Life ───────────────────────────────────────────────────────

/// First 7 circles of the Flower of Life with overlapping arcs shaded by eased.
/// Distinct from Seed-of-Life: draws ONLY the interior arcs (intersection petals),
/// not full circles — revealing the petal "germ" geometry.
struct GermOfLife;
impl ProgressStyle for GermOfLife {
    fn name(&self) -> &str {
        "germ-of-life"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "Germ of Life: interior arc-petals of 7 overlapping circles fill in as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let r = (cx.min(cy) * 0.38).max(1.0);
        let centres = seed_centres(cx, cy, r);

        // For each pair of adjacent outer circles (6 pairs), draw ONLY the arc
        // segment of circle i that lies within circle i+1. We approximate this
        // by plotting only angle ranges where the point is inside the neighbour.
        let reveal_petals = (ctx.eased * 6.0).ceil() as usize;

        // Draw the central circle fully.
        plot_circle(grid, cx, cy, r);

        for petal in 0..reveal_petals.min(6) {
            let (ox1, oy1) = (centres[petal + 1].0, centres[petal + 1].1);
            // Next petal (wrapping).
            let next = ((petal + 1) % 6) + 1;
            let (ox2, oy2) = (centres[next].0, centres[next].1);

            // Arc of circle at ox1,oy1 that passes through cx,cy region.
            let steps = 256usize;
            for i in 0..steps {
                let angle = 2.0 * PI * i as f32 / steps as f32;
                let px = ox1 + r * angle.cos();
                let py = oy1 + r * angle.sin();
                // Only draw if inside the central circle.
                let dc = (px - cx).hypot(py - cy);
                if dc <= r + 0.5 {
                    draw::dot_i(grid, px.round() as i32, py.round() as i32);
                }
            }

            // Also arc of ox2 inside ox1.
            for i in 0..steps {
                let angle = 2.0 * PI * i as f32 / steps as f32;
                let px = ox2 + r * angle.cos();
                let py = oy2 + r * angle.sin();
                let d1 = (px - ox1).hypot(py - oy1);
                if d1 <= r + 0.5 {
                    draw::dot_i(grid, px.round() as i32, py.round() as i32);
                }
            }

            let t = petal as f32 / 5.0;
            let color = ctx.palette.sample(t);
            let (cw, ch) = grid.dimensions();
            let pcx = (ox1 / 2.0).round() as usize;
            let pcy = (oy1 / 4.0).round() as usize;
            if pcx < cw && pcy < ch {
                draw::tint_row(grid, pcy, pcx, pcx, color);
            }
        }
        Ok(())
    }
}

// ── 8. Tripod of Life ─────────────────────────────────────────────────────

/// Three-fold symmetry: 3 interlocking circles + 3-arm spoke structure.
/// Structurally distinct: uses 3-fold (not 6-fold) symmetry and builds a
/// branching tripod geometry inside the overlapping region.
struct TripodOfLife;
impl ProgressStyle for TripodOfLife {
    fn name(&self) -> &str {
        "tripod-of-life"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "Tripod of Life: three interlocking circles with a central 3-arm spoke revealed by eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let r = (cx.min(cy) * 0.42).max(1.0);

        let rot = ctx.time * 0.3;

        // 3 circle centres at 120° intervals.
        let mut circle_centres = [(0f32, 0f32); 3];
        for i in 0..3 {
            let a = rot + i as f32 * 2.0 * PI / 3.0;
            circle_centres[i] = (cx + r * 0.5 * a.cos(), cy + r * 0.5 * a.sin());
        }

        // Reveal circles (phase 1: eased 0→0.5), then spoke arms (phase 2: 0.5→1).
        let circle_frac = (ctx.eased * 2.0).min(1.0);
        let arm_frac = ((ctx.eased - 0.5) * 2.0).max(0.0).min(1.0);

        let reveal_circles = (circle_frac * 3.0).ceil() as usize;
        for i in 0..reveal_circles.min(3) {
            let (px, py) = circle_centres[i];
            plot_circle(grid, px, py, r);
            let color = ctx.palette.sample(i as f32 / 2.0);
            let (cw, ch) = grid.dimensions();
            let cell_x = (px / 2.0).round() as usize;
            let cell_y = (py / 4.0).round() as usize;
            if cell_x < cw && cell_y < ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }

        // Spoke arms from centre outward.
        for i in 0..3 {
            let a = rot + i as f32 * 2.0 * PI / 3.0;
            let arm_r = r * arm_frac;
            let ex = cx + arm_r * a.cos();
            let ey = cy + arm_r * a.sin();
            plot_line(grid, cx, cy, ex, ey);
        }

        // Central dot.
        if ctx.eased > 0.1 {
            draw::dot_i(grid, cx.round() as i32, cy.round() as i32);
        }
        Ok(())
    }
}

// ── 9. Tree of Life ───────────────────────────────────────────────────────

/// Kabbalistic Tree of Life: 10 sephirot nodes + 22 connecting paths.
/// Paths drawn progressively with eased; nodes always visible.
struct TreeOfLife;
impl ProgressStyle for TreeOfLife {
    fn name(&self) -> &str {
        "tree-of-life"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "Kabbalistic Tree of Life: 10 sephirot nodes with 22 paths revealed as eased rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;

        // Scale so the tree fits: spans 6 units tall, 4 wide.
        let sx = (dw as f32 / 4.5).min(dh as f32 / 6.5).max(1.0);
        let sy = sx;

        // Sephirot positions (normalized, y increasing downward).
        // Classical layout:  1=Kether(top), 10=Malkuth(bottom).
        let nodes: [(f32, f32); 10] = [
            (0.0, -3.0),  // 0 Kether
            (-1.0, -2.0), // 1 Chokmah
            (1.0, -2.0),  // 2 Binah
            (-1.0, -1.0), // 3 Chesed
            (1.0, -1.0),  // 4 Geburah
            (0.0, 0.0),   // 5 Tiphareth
            (-1.0, 1.0),  // 6 Netzach
            (1.0, 1.0),   // 7 Hod
            (0.0, 2.0),   // 8 Yesod
            (0.0, 3.0),   // 9 Malkuth
        ];

        // Convert to dot-space.
        let dot_nodes: Vec<(f32, f32)> = nodes
            .iter()
            .map(|&(x, y)| (cx + x * sx, cy + y * sy))
            .collect();

        // 22 paths of the Tree of Life (traditional Sefer Yetzirah connections).
        let paths: [(usize, usize); 22] = [
            (0, 1),
            (0, 2),
            (0, 5), // Kether connections
            (1, 2),
            (1, 3),
            (1, 5), // Chokmah
            (2, 4),
            (2, 5), // Binah
            (3, 4),
            (3, 5),
            (3, 6), // Chesed
            (4, 5),
            (4, 7), // Geburah
            (5, 6),
            (5, 7),
            (5, 8), // Tiphareth
            (6, 7),
            (6, 8), // Netzach
            (7, 8), // Hod
            (8, 9), // Yesod-Malkuth
            (1, 4),
            (2, 3), // cross-paths (Paroketh)
        ];

        let reveal = (ctx.eased * paths.len() as f32).round() as usize;
        for (i, &(a, b)) in paths.iter().enumerate().take(reveal) {
            let (ax, ay) = dot_nodes[a];
            let (bx, by) = dot_nodes[b];
            plot_line(grid, ax, ay, bx, by);
            let t = i as f32 / (paths.len() - 1).max(1) as f32;
            let _ = t;
        }

        // Draw sephirot nodes as small circles.
        let nr = (sx * 0.25).max(1.0);
        for (i, &(px, py)) in dot_nodes.iter().enumerate() {
            plot_circle(grid, px, py, nr);
            let t = i as f32 / 9.0;
            let color = ctx.palette.sample(t);
            let (cw, ch) = grid.dimensions();
            let cell_x = (px / 2.0).round() as usize;
            let cell_y = (py / 4.0).round() as usize;
            if cell_x < cw && cell_y < ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }
        Ok(())
    }
}

// ── 10. 64-Tetrahedron Grid ───────────────────────────────────────────────

/// 2D projection of the 64-tetrahedron grid as a triangular lattice.
/// Structurally different: pure triangular grid, no circles.
struct TetraGrid;
impl ProgressStyle for TetraGrid {
    fn name(&self) -> &str {
        "64-tetra-grid"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "64-tetrahedron grid: equilateral triangular lattice revealed by progress, sweeping in from left"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;

        // Triangular lattice basis vectors (equilateral triangles).
        // a = (1, 0), b = (0.5, √3/2), scaled to fit.
        let cell_size = (dw.min(dh) as f32 / 8.0).max(2.0);
        let ax = cell_size;
        let ay = 0.0f32;
        let bx = cell_size * 0.5;
        let by = cell_size * (3f32.sqrt() / 2.0);

        // Generate lattice points in a grid range.
        let range = 6i32;
        let mut points: Vec<(f32, f32)> = Vec::new();
        for i in -range..=range {
            for j in -range..=range {
                let px = cx + i as f32 * ax + j as f32 * bx;
                let py = cy + i as f32 * ay + j as f32 * by;
                // Only keep if within dot bounds.
                if px >= 0.0 && px < dw as f32 && py >= 0.0 && py < dh as f32 {
                    points.push((px, py));
                }
            }
        }

        // Reveal point by point from left to right (x-sorted).
        points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let reveal = (ctx.eased * points.len() as f32).round() as usize;

        // Draw the triangular edges between adjacent lattice points.
        // For each revealed point, draw edges to its right-neighbours.
        for &(px, py) in points.iter().take(reveal) {
            // The six neighbour offsets in triangular lattice.
            let neighbors: [(f32, f32); 6] = [
                (ax, ay),
                (-ax, -ay),
                (bx, by),
                (-bx, -by),
                (ax - bx, ay - by),
                (-(ax - bx), -(ay - by)),
            ];
            draw::dot_i(grid, px.round() as i32, py.round() as i32);
            for (dx, dy) in neighbors {
                let nx = px + dx;
                let ny = py + dy;
                if nx >= 0.0 && nx < dw as f32 && ny >= 0.0 && ny < dh as f32 {
                    // Only draw if neighbour is also revealed.
                    let idx = points.partition_point(|&(qx, _)| qx < nx - 0.5);
                    let in_revealed = points[..idx.min(reveal)]
                        .iter()
                        .any(|&(qx, qy)| (qx - nx).abs() < 1.0 && (qy - ny).abs() < 1.0);
                    if in_revealed {
                        plot_line(grid, px, py, nx, ny);
                    }
                }
            }
        }

        // Color gradient left-to-right.
        let (cw, ch) = grid.dimensions();
        for cy_cell in 0..ch {
            for cx_cell in 0..cw {
                let t = cx_cell as f32 / cw.max(1) as f32;
                draw::tint_row(grid, cy_cell, cx_cell, cx_cell, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ── 11. Torus Flower ──────────────────────────────────────────────────────

/// Flower-of-Life circles warped onto a toroidal projection, rotating with time.
/// Structurally distinct: each circle centre is displaced by a sinusoidal
/// torus-warp offset derived from its lattice angle, creating a rolling,
/// depth-suggesting motion entirely absent from the flat flower.
struct TorusFlower;
impl ProgressStyle for TorusFlower {
    fn name(&self) -> &str {
        "torus-flower"
    }
    fn theme(&self) -> &str {
        "floweroflife"
    }
    fn describe(&self) -> &str {
        "Flower-of-Life circles warped onto a torus surface; the whole pattern rolls with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let r = (cx.min(cy) / 3.2).max(1.0);

        // Torus parameters: major radius R (from centre to tube centre),
        // minor radius ru (tube radius).
        let big_r = cx.min(cy) * 0.55;
        let small_r = cx.min(cy) * 0.2;

        // Time drives toroidal rotation angle phi.
        let phi = ctx.time * 0.5;

        let raw = flower_centres(cx, cy, r);
        let total = raw.len();
        let reveal = (ctx.eased * total as f32).ceil() as usize;

        for (i, &(px, py)) in raw.iter().enumerate().take(reveal.min(total)) {
            // Map flat (px,py) → polar angle θ relative to centre.
            let theta = (py - cy).atan2(px - cx);
            let dist_frac = (px - cx).hypot(py - cy) / (2.0 * r).max(1.0);

            // Torus warp: displace each centre by a depth offset derived from
            // mapping theta → torus surface.
            let torus_x_offset = big_r
                * (theta + phi).cos()
                * (1.0 - small_r / big_r * (dist_frac * PI * 2.0 + phi).cos());
            let torus_y_offset = small_r * (dist_frac * PI * 2.0 + phi).sin();

            // Blend the flat position with the torus-warped position.
            let warp = 0.3;
            let wpx = (1.0 - warp) * px + warp * (cx + torus_x_offset * (r / big_r.max(1.0)));
            let wpy = (1.0 - warp) * py + warp * (cy + torus_y_offset * 2.0);

            // Scale circle radius by apparent depth (simulating perspective).
            let depth = 1.0 + 0.25 * (dist_frac * PI * 2.0 + phi).cos();
            let cr = (r * depth).max(1.0);

            plot_circle(grid, wpx, wpy, cr);

            let t = i as f32 / (total - 1).max(1) as f32;
            let color = ctx.palette.sample(t);
            let (cw, ch) = grid.dimensions();
            let cell_x = (wpx / 2.0).round() as usize;
            let cell_y = (wpy / 4.0).round() as usize;
            if cell_x < cw && cell_y < ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, color);
            }
        }
        Ok(())
    }
}
