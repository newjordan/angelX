//! World-cultures / ornament-themed progress bars.
//!
//! Ten structurally distinct styles, each drawing itself as `ctx.eased` rises
//! and animating via `ctx.time`.  Every style encodes a real cultural ornament
//! pattern in braille dot-space:
//!
//! - `mandala`        — radial symmetry: petals and concentric rings unfurl
//! - `celtic-knot`    — interlaced over/under woven bands fill the bar
//! - `aztec-fret`     — stepped-fret meander tiles from left to right
//! - `islamic-star`   — 8-fold star tessellation tiles revealed by progress
//! - `greek-key`      — meander border scrolls in from the left
//! - `seigaiha`       — Japanese overlapping wave-scale arcs fill row by row
//! - `totem-pole`     — carved segments stack upward as progress rises
//! - `runes`          — Norse rune glyphs carved in one by one
//! - `paisley-swirl`  — henna swirl spirals unfurl from multiple seeds
//! - `kente-weave`    — kente / tartan warp-and-weft interlacing

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ── optional easing import ────────────────────────────────────────────────────
use super::super::{ease, Easing};

// ─────────────────────────────────────────────────────────────────────────────
// Public registry
// ─────────────────────────────────────────────────────────────────────────────

/// All styles in the `cultures` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per ornament style, in display order.
/// Every style is structurally distinct — variety is in shape, not colour.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Mandala),
        Box::new(CelticKnot),
        Box::new(AztecFret),
        Box::new(IslamicStar),
        Box::new(GreekKey),
        Box::new(Seigaiha),
        Box::new(TotemPole),
        Box::new(Runes),
        Box::new(PaisleySwirl),
        Box::new(KenteWeave),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Integer Bresenham line rasteriser. Step-bounded; `draw::dot_i` clips OOB.
fn bres(grid: &mut BrailleGrid, mut x0: i32, mut y0: i32, x1: i32, y1: i32) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let max_steps = (dx - dy + 2).unsigned_abs() as usize;
    let mut steps = 0usize;
    loop {
        draw::dot_i(grid, x0, y0);
        if x0 == x1 && y0 == y1 {
            break;
        }
        steps += 1;
        if steps > max_steps + 2 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Draw a circle arc (dots) from angle `a0` to `a1` (radians) around `(cx,cy)`
/// with dot-space radius `r`.  Step count is proportional to arc length.
fn arc(grid: &mut BrailleGrid, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
    if r < 0.5 {
        return;
    }
    let arc_len = (a1 - a0).abs() * r;
    let steps = (arc_len.ceil() as usize + 2).max(4);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let a = a0 + t * (a1 - a0);
        let px = (cx + r * a.cos()).round() as i32;
        let py = (cy + r * a.sin()).round() as i32;
        draw::dot_i(grid, px, py);
    }
}

/// Tint every cell in the grid with a horizontal palette gradient.
fn palette_tint(grid: &mut BrailleGrid, ctx: &BarContext) {
    let (cw, ch) = grid.dimensions();
    for cy in 0..ch {
        for cx in 0..cw {
            let t = cx as f32 / cw.max(1) as f32;
            draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Mandala — radial symmetry: concentric rings + petal spokes unfurl
// ─────────────────────────────────────────────────────────────────────────────

struct Mandala;
impl ProgressStyle for Mandala {
    fn name(&self) -> &str {
        "mandala"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Radial mandala: concentric rings and petal spokes bloom outward as progress \
         rises; the whole mandala rotates slowly with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let max_r = (dw.min(dh * 2) as f32 / 2.0 - 1.0).max(1.0);
        let rot = ctx.time * 0.25;

        // How many rings and petals are revealed.
        let n_rings = 4usize;
        let n_petals = 8usize;

        for ring in 0..n_rings {
            let ring_frac = (ring + 1) as f32 / n_rings as f32;
            // Each ring appears when eased > its threshold.
            if ctx.eased < ring_frac * 0.6 {
                continue;
            }
            let r = ring_frac * max_r;
            // Full ring circle.
            arc(grid, cx, cy, r, 0.0, 2.0 * PI);
        }

        // Petal spokes — Bézier-approximated with two symmetric arc pairs.
        let petal_reveal = (ctx.eased * 3.0 - 0.5).clamp(0.0, 1.0);
        for p in 0..n_petals {
            let base_angle = p as f32 * 2.0 * PI / n_petals as f32 + rot;
            let tip_r = max_r * petal_reveal;
            // Draw spoke line.
            let x1 = (cx + tip_r * base_angle.cos()).round() as i32;
            let y1 = (cy + tip_r * base_angle.sin()).round() as i32;
            bres(grid, cx as i32, cy as i32, x1, y1);
            // Petal arc: small bulge to either side.
            if tip_r > 3.0 {
                let bulge_r = tip_r * 0.35;
                let mid_r = tip_r * 0.55;
                let mid_x = cx + mid_r * base_angle.cos();
                let mid_y = cy + mid_r * base_angle.sin();
                let perp = base_angle + PI / 2.0;
                let side_x = mid_x + bulge_r * perp.cos();
                let side_y = mid_y + bulge_r * perp.sin();
                arc(
                    grid,
                    side_x,
                    side_y,
                    bulge_r,
                    base_angle + PI,
                    base_angle + 2.0 * PI,
                );
                let side2_x = mid_x - bulge_r * perp.cos();
                let side2_y = mid_y - bulge_r * perp.sin();
                arc(grid, side2_x, side2_y, bulge_r, base_angle, base_angle + PI);
            }
        }

        // Radial tint from center outward.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Celtic Knot — interlaced over/under woven bands
// ─────────────────────────────────────────────────────────────────────────────

struct CelticKnot;
impl ProgressStyle for CelticKnot {
    fn name(&self) -> &str {
        "celtic-knot"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Celtic interlace: two sinusoidal bands weave over and under each other; \
         the knot expands rightward as progress rises, undulating with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let filled_w = (ctx.eased * dw as f32).round() as usize;
        if filled_w == 0 {
            return Ok(());
        }

        let cy = dh as f32 / 2.0;
        // Amplitude of each strand — scale to half the grid height minus border.
        let amp = (dh as f32 / 2.0 - 1.0).max(0.5);
        // Wave period: 1 full cycle per ~20 dots.
        let period = (dw as f32 / 3.0).max(8.0);
        let phase = ctx.time * 1.5;
        // Band thickness in dots (1 or 2).
        let thick = (dh / 8).max(1);

        for x in 0..filled_w.min(dw) {
            let t = x as f32 / period;
            // Two strands: 180° apart, giving the over/under illusion via
            // a gap wherever they cross.
            let y_a = cy + amp * (2.0 * PI * t + phase).sin();
            let y_b = cy + amp * (2.0 * PI * t + phase + PI).sin();

            // Detect crossing region — suppress one strand near crossings.
            let dist = (y_a - y_b).abs();
            let crossing = dist < amp * 0.5;

            for dy in 0..thick {
                // Strand A (always drawn).
                let ya = y_a as i32 + dy as i32;
                draw::dot_i(grid, x as i32, ya);
                draw::dot_i(grid, x as i32, ya - 1);
                // Strand B suppressed near crossing for the "under" effect.
                if !crossing || (x / (thick + 1)) % 2 == 0 {
                    let yb = y_b as i32 + dy as i32;
                    draw::dot_i(grid, x as i32, yb);
                    draw::dot_i(grid, x as i32, yb - 1);
                }
            }
        }

        // Tint: gradient across the bar.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cy_c in 0..ch {
            let hi = filled_cells.saturating_sub(1).min(cw.saturating_sub(1));
            if filled_cells > 0 {
                draw::tint_row(grid, cy_c, 0, hi, ctx.palette.sample(0.6));
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Aztec Fret — stepped-T meander pattern tiles left to right
// ─────────────────────────────────────────────────────────────────────────────

struct AztecFret;
impl ProgressStyle for AztecFret {
    fn name(&self) -> &str {
        "aztec-fret"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Aztec/Maya stepped-fret meander: interlocking T-shaped spirals tile the \
         bar from left to right as progress rises."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Each fret tile is `tile_w` dots wide and fills the full height.
        let tile_w = ((dw / 6).max(3)).min(dw);
        let filled_w = (ctx.eased * dw as f32).round() as usize;
        if filled_w == 0 {
            return Ok(());
        }

        // Outer border line.
        draw::hline(grid, 0, filled_w.min(dw).saturating_sub(1), 0);
        draw::hline(
            grid,
            0,
            filled_w.min(dw).saturating_sub(1),
            dh.saturating_sub(1),
        );

        let n_tiles = (filled_w + tile_w - 1) / tile_w;

        for tile in 0..n_tiles {
            let x0 = tile * tile_w;
            let x1 = (x0 + tile_w).min(filled_w).min(dw);
            if x0 >= x1 {
                continue;
            }

            // Even tiles: stepped fret going right-then-down.
            // Odd tiles: mirror image going left-then-up.
            let flip = tile % 2 == 1;

            // Vertical stem at the start of the tile.
            let stem_x = if flip { x1.saturating_sub(1) } else { x0 };
            draw::vline(
                grid,
                stem_x.min(dw.saturating_sub(1)),
                0,
                dh.saturating_sub(1),
            );

            // Stepped rungs: 3 horizontal bars at equal y-intervals.
            let n_steps = 3usize;
            for s in 1..=n_steps {
                let y = s * dh / (n_steps + 1);
                let rung_x0 = if flip {
                    stem_x.saturating_sub(tile_w / 2)
                } else {
                    stem_x
                };
                let rung_x1 = if flip { stem_x } else { stem_x + tile_w / 2 };
                let rx0 = rung_x0.min(dw.saturating_sub(1));
                let rx1 = rung_x1.min(dw.saturating_sub(1));
                draw::hline(grid, rx0, rx1, y.min(dh.saturating_sub(1)));
                // Short vertical drop at the rung end.
                let drop_x = if flip { rx0 } else { rx1 };
                let drop_top = y.saturating_sub(dh / (n_steps * 2 + 1) + 1);
                draw::vline(grid, drop_x, drop_top, y.min(dh.saturating_sub(1)));
            }
        }

        palette_tint(grid, ctx);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Islamic Star — 8-fold star tessellation
// ─────────────────────────────────────────────────────────────────────────────

struct IslamicStar;
impl ProgressStyle for IslamicStar {
    fn name(&self) -> &str {
        "islamic-star"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Islamic geometric: 8-fold star tiles tesselate the bar, each star's points \
         appearing as progress rises; the grid shimmers in slow rotation with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Tile the grid with 8-pointed stars (octagram).
        // Each star is centred on a regular grid, radius scales to cell size.
        let star_r = ((dh as f32 / 2.0).min(dw as f32 / 2.0) - 1.0).max(2.0);
        let cell_x = (star_r * 2.2).max(4.0);
        let cell_y = (star_r * 2.2).max(4.0);

        let n_cols = ((dw as f32 / cell_x).ceil() as usize + 1).max(1);
        let n_rows = ((dh as f32 / cell_y).ceil() as usize + 1).max(1);

        // Animation: slow rotation of star orientation.
        let rot = ctx.time * 0.18;

        // Draw each star up to the progress-gated total.
        let total_stars = n_cols * n_rows;
        let reveal = (ctx.eased * total_stars as f32).ceil() as usize;

        for idx in 0..reveal.min(total_stars) {
            let row = idx / n_cols;
            let col = idx % n_cols;

            // Hex-offset: odd rows shifted by half a cell.
            let off_x = if row % 2 == 1 { cell_x / 2.0 } else { 0.0 };
            let cx = col as f32 * cell_x + star_r + off_x;
            let cy = row as f32 * cell_y + star_r;

            // Draw an 8-pointed star: outer 8 points + inner 8 points.
            let n_pts = 8usize;
            let inner_r = star_r * 0.42;
            let mut outer: Vec<(i32, i32)> = Vec::with_capacity(n_pts);
            let mut inner: Vec<(i32, i32)> = Vec::with_capacity(n_pts);

            for i in 0..n_pts {
                let angle = i as f32 * 2.0 * PI / n_pts as f32 + rot;
                let half_angle = angle + PI / n_pts as f32;
                outer.push((
                    (cx + star_r * angle.cos()).round() as i32,
                    (cy + star_r * angle.sin()).round() as i32,
                ));
                inner.push((
                    (cx + inner_r * half_angle.cos()).round() as i32,
                    (cy + inner_r * half_angle.sin()).round() as i32,
                ));
            }

            // Connect outer[i] → inner[i] → outer[i+1] forming the star outline.
            for i in 0..n_pts {
                let next = (i + 1) % n_pts;
                bres(grid, outer[i].0, outer[i].1, inner[i].0, inner[i].1);
                bres(grid, inner[i].0, inner[i].1, outer[next].0, outer[next].1);
            }
        }

        palette_tint(grid, ctx);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Greek Key — meander border scrolling from the left
// ─────────────────────────────────────────────────────────────────────────────

struct GreekKey;
impl ProgressStyle for GreekKey {
    fn name(&self) -> &str {
        "greek-key"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Greek key / meander border: interlocking rectangular spirals scroll in \
         from the left edge as progress rises, one complete meander unit at a time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // One meander unit is a rectangular spiral: right, down, left, up, right.
        // Unit width in dots.
        let unit_w = ((dh * 2).max(6)).min(dw / 2 + 1);
        let unit_h = dh.saturating_sub(2).max(2);
        let step = (unit_w / 4).max(1);

        let filled_w = (ctx.eased * dw as f32).round() as usize;
        if filled_w == 0 {
            return Ok(());
        }

        let n_units = (filled_w + unit_w - 1) / unit_w;

        for u in 0..n_units {
            let x0 = u * unit_w;
            if x0 >= dw {
                break;
            }
            let x1 = (x0 + unit_w).min(filled_w).min(dw);
            let y0 = 0usize;
            let y1 = unit_h.min(dh.saturating_sub(1));

            // Outer border of the unit.
            draw::hline(grid, x0, x1.saturating_sub(1), y0);
            draw::hline(grid, x0, x1.saturating_sub(1), y1);
            draw::vline(grid, x0, y0, y1);
            draw::vline(grid, x1.saturating_sub(1).min(dw.saturating_sub(1)), y0, y1);

            // Inner key: a hook shape entering from the top.
            if step >= 1 && unit_w >= 4 && unit_h >= 4 {
                let ix = (x0 + step).min(dw.saturating_sub(1));
                let iy0 = y0 + step;
                let iy1 = y1.saturating_sub(step);
                // Vertical down segment.
                draw::vline(grid, ix, iy0, iy1);
                // Horizontal right at the bottom of the inner key.
                let inner_right = (ix + step + step)
                    .min(x1.saturating_sub(1))
                    .min(dw.saturating_sub(1));
                draw::hline(grid, ix, inner_right, iy1);
                // Short vertical up.
                let rise = (iy1.saturating_sub(step)).max(iy0);
                draw::vline(grid, inner_right, rise, iy1);
            }
        }

        palette_tint(grid, ctx);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Seigaiha — Japanese overlapping wave scales filling row by row
// ─────────────────────────────────────────────────────────────────────────────

struct Seigaiha;
impl ProgressStyle for Seigaiha {
    fn name(&self) -> &str {
        "seigaiha"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Japanese seigaiha (blue ocean waves): overlapping semicircular scales tile \
         the bar bottom-up as progress rises; time ripples a subtle phase shift."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Each scale is a semicircle.  Scales tile in a brick offset pattern.
        let scale_r = (dh as f32 / 2.0).max(2.0).min(dw as f32 / 2.0);
        let scale_w = (scale_r * 2.0) as usize;
        let scale_h = scale_r as usize + 1;

        // Reveal from bottom to top.
        let eased_e = ease(Easing::CubicInOut, ctx.eased);
        let revealed_h = (eased_e * dh as f32).round() as usize;
        let y_threshold = dh.saturating_sub(revealed_h);

        // Phase animation — gentle horizontal drift.
        let phase_offset = (ctx.time * 0.4).sin() * scale_r * 0.1;

        let n_rows = (dh / scale_h.max(1) + 2).max(1);
        let n_cols = (dw / scale_w.max(1) + 2).max(1);

        for row in 0..n_rows {
            let cy = (dh as i32 - row as i32 * scale_h as i32) as f32;
            if cy < -(scale_r) {
                break;
            }

            for col in 0..n_cols {
                // Brick offset: odd rows shift by half a scale width.
                let offset = if row % 2 == 1 {
                    scale_w as f32 / 2.0
                } else {
                    0.0
                };
                let cx = col as f32 * scale_w as f32 + offset + phase_offset;

                // Draw semicircle (upper half only — flat edge down).
                let r = scale_r;
                let steps = (PI * r).ceil() as usize + 4;
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    let angle = PI + t * PI; // from π to 2π (top half of unit circle)
                    let px = (cx + r * angle.cos()).round() as i32;
                    let py = (cy + r * angle.sin()).round() as i32;
                    if py >= 0 && (py as usize) >= y_threshold {
                        draw::dot_i(grid, px, py);
                    }
                }
                // Flat baseline of the scale.
                let py_base = cy.round() as i32;
                let x_left = (cx - r).round() as i32;
                let x_right = (cx + r).round() as i32;
                if py_base >= 0 && (py_base as usize) >= y_threshold {
                    for px in x_left..=x_right {
                        draw::dot_i(grid, px, py_base);
                    }
                }
            }
        }

        palette_tint(grid, ctx);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Totem Pole — carved segments stacking upward with progress
// ─────────────────────────────────────────────────────────────────────────────

struct TotemPole;
impl ProgressStyle for TotemPole {
    fn name(&self) -> &str {
        "totem-pole"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Totem pole: vertically stacked carved segments (eyes, beak, wings) build \
         upward cell by cell as progress rises; columns shimmer with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Each totem column is `seg_w` dots wide; they stand side by side.
        let seg_w = (dw / 5).max(2).min(dw);
        let n_cols = (dw / seg_w).max(1);

        // How many dot rows are revealed (growing from the bottom).
        let revealed = (ctx.eased * dh as f32).round() as usize;
        let y0_visible = dh.saturating_sub(revealed);

        for col in 0..n_cols {
            let x0 = col * seg_w;
            let x1 = (x0 + seg_w).min(dw);
            let cx = (x0 + x1) / 2;
            let cw_col = x1.saturating_sub(x0);

            // Outer column border.
            draw::vline(grid, x0, y0_visible, dh.saturating_sub(1));
            draw::vline(
                grid,
                x1.saturating_sub(1).min(dw.saturating_sub(1)),
                y0_visible,
                dh.saturating_sub(1),
            );

            // Segment height: divide the column into 4 face segments.
            let n_segs = 4usize;
            let seg_h = (dh / n_segs).max(1);

            for seg in 0..n_segs {
                let sy0 = seg * seg_h;
                let sy1 = ((seg + 1) * seg_h).min(dh);
                if sy0 < y0_visible {
                    continue;
                }
                let scy = (sy0 + sy1) / 2;

                // Each segment gets a different carved motif.
                match seg % 4 {
                    0 => {
                        // Eyes: two dots side by side.
                        let eye_y = scy.min(dh.saturating_sub(1));
                        let eye_lx = cx.saturating_sub(cw_col / 4).min(dw.saturating_sub(1));
                        let eye_rx = (cx + cw_col / 4).min(dw.saturating_sub(1));
                        draw::dot(grid, eye_lx, eye_y);
                        draw::dot(grid, eye_rx, eye_y);
                        // Eyebrow lines.
                        let brow_y = scy.saturating_sub(1).min(dh.saturating_sub(1));
                        draw::hline(
                            grid,
                            x0,
                            x1.saturating_sub(1).min(dw.saturating_sub(1)),
                            brow_y,
                        );
                    }
                    1 => {
                        // Beak: V-shape pointing downward.
                        let beak_top_y = sy0.max(y0_visible);
                        let beak_bot_y = scy.min(dh.saturating_sub(1));
                        bres(
                            grid,
                            x0 as i32,
                            beak_top_y as i32,
                            cx as i32,
                            beak_bot_y as i32,
                        );
                        bres(
                            grid,
                            x1.saturating_sub(1) as i32,
                            beak_top_y as i32,
                            cx as i32,
                            beak_bot_y as i32,
                        );
                    }
                    2 => {
                        // Wings: horizontal bars spreading from centre.
                        let wing_y = scy.min(dh.saturating_sub(1));
                        draw::hline(
                            grid,
                            x0,
                            x1.saturating_sub(1).min(dw.saturating_sub(1)),
                            wing_y,
                        );
                        // Wing tips — vlines up and down.
                        let tip_h = (seg_h / 3).max(1);
                        draw::vline(
                            grid,
                            x0,
                            wing_y.saturating_sub(tip_h),
                            (wing_y + tip_h).min(dh.saturating_sub(1)),
                        );
                        draw::vline(
                            grid,
                            x1.saturating_sub(1).min(dw.saturating_sub(1)),
                            wing_y.saturating_sub(tip_h),
                            (wing_y + tip_h).min(dh.saturating_sub(1)),
                        );
                    }
                    _ => {
                        // Base plaque: full width filled rectangle.
                        let plaque_y = sy1.saturating_sub(2).max(sy0).min(dh.saturating_sub(1));
                        draw::hline(
                            grid,
                            x0,
                            x1.saturating_sub(1).min(dw.saturating_sub(1)),
                            plaque_y,
                        );
                        draw::hline(
                            grid,
                            x0,
                            x1.saturating_sub(1).min(dw.saturating_sub(1)),
                            sy1.saturating_sub(1).min(dh.saturating_sub(1)),
                        );
                    }
                }

                // Segment divider.
                draw::hline(
                    grid,
                    x0,
                    x1.saturating_sub(1).min(dw.saturating_sub(1)),
                    sy0.min(dh.saturating_sub(1)),
                );
            }
        }

        palette_tint(grid, ctx);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Runes — Norse rune glyphs carved in one by one via draw::glyph
// ─────────────────────────────────────────────────────────────────────────────

struct Runes;
impl ProgressStyle for Runes {
    fn name(&self) -> &str {
        "runes"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Norse elder futhark runes: glyphs are carved into stone cells one by one \
         as progress rises; a flicker-shimmer via shade density pulses with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Elder Futhark runes (Unicode block U+16A0...).
        let runes: &[char] = &[
            'ᚠ', 'ᚢ', 'ᚦ', 'ᚨ', 'ᚱ', 'ᚲ', 'ᚷ', 'ᚹ', 'ᚺ', 'ᚾ', 'ᛁ', 'ᛃ', 'ᛇ', 'ᛈ', 'ᛉ', 'ᛊ', 'ᛏ',
            'ᛒ', 'ᛖ', 'ᛗ', 'ᛚ', 'ᛜ', 'ᛞ', 'ᛟ',
        ];

        let total_cells = cw * ch;
        let revealed = (ctx.eased * total_cells as f32).round() as usize;

        // Time-driven flicker: a shimmer wave travels left to right.
        let shimmer_x = ((ctx.time * 0.7).fract() * cw as f32) as usize;

        for idx in 0..revealed.min(total_cells) {
            let cx = idx % cw;
            let cy = idx / cw;
            let rune = runes[idx % runes.len()];

            // Near the shimmer wave: show a lighter shade character instead,
            // giving the impression of light reflecting off carved stone.
            let dist = (cx as i32 - shimmer_x as i32).unsigned_abs() as usize;
            if dist <= 1 {
                draw::shade(grid, cx, cy, 2); // ▒ — lit stone
            } else {
                draw::glyph(grid, cx, cy, rune);
            }
        }

        // Unfilled cells: bare stone background (light shade).
        for idx in revealed..total_cells {
            let cx = idx % cw;
            let cy = idx / cw;
            draw::shade(grid, cx, cy, 1); // ░
        }

        // Tint by column.
        for cy in 0..ch {
            for cx_c in 0..cw {
                let t = cx_c as f32 / cw.max(1) as f32;
                draw::tint_row(grid, cy, cx_c, cx_c, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Paisley Swirl — henna spirals unfurling from multiple seeds
// ─────────────────────────────────────────────────────────────────────────────

struct PaisleySwirl;
impl ProgressStyle for PaisleySwirl {
    fn name(&self) -> &str {
        "paisley-swirl"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Henna / paisley: teardrop spiral seeds unfurl across the bar as progress \
         rises; each swirl grows its own tight logarithmic coil, animated with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Number of paisley seeds tiling the bar.
        let n_seeds: usize = ((dw / (dh.max(1) * 2)).max(1)).min(12);
        let seed_w = dw / n_seeds.max(1);

        let eased_e = ease(Easing::QuadOut, ctx.eased);
        let max_turns = 2.5f32; // spiral coil turns at full progress.

        for s in 0..n_seeds {
            // Seed centre.
            let cx = s as f32 * seed_w as f32 + seed_w as f32 / 2.0;
            let cy = dh as f32 / 2.0;
            // Orientation alternates + time drift.
            let base_angle = if s % 2 == 0 { 0.0f32 } else { PI } + ctx.time * 0.3;

            // Max radius of this swirl — limited by seed cell height.
            let max_r = (dh as f32 / 2.0 - 1.0).max(1.0).min(seed_w as f32 / 2.0);

            // Teardrop outline: draw a small circle offset from the spiral tip.
            let tip_r = max_r * 0.25 * eased_e;
            let tip_cx = cx + (max_r * 0.6 * eased_e) * base_angle.cos();
            let tip_cy = cy + (max_r * 0.6 * eased_e) * base_angle.sin();
            if tip_r >= 1.0 {
                arc(grid, tip_cx, tip_cy, tip_r, 0.0, 2.0 * PI);
            }

            // Logarithmic spiral: r = a * e^(b*theta).
            // We parameterise so r goes from 0 to max_r over max_turns*2π.
            let theta_max = eased_e * max_turns * 2.0 * PI;
            let steps = (theta_max * max_r).ceil() as usize + 4;
            let a = max_r / (max_turns * 2.0 * PI).exp();
            let b = 1.0f32;

            for i in 0..=steps {
                let t = i as f32 / steps.max(1) as f32;
                let theta = t * theta_max;
                let r = a * (b * theta).exp();
                if r > max_r {
                    break;
                }
                let angle = theta + base_angle;
                let px = (cx + r * angle.cos()).round() as i32;
                let py = (cy + r * angle.sin()).round() as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Radial tint from the grid centre.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Kente Weave — warp-and-weft interlacing strips
// ─────────────────────────────────────────────────────────────────────────────

struct KenteWeave;
impl ProgressStyle for KenteWeave {
    fn name(&self) -> &str {
        "kente-weave"
    }
    fn theme(&self) -> &str {
        "cultures"
    }
    fn describe(&self) -> &str {
        "Kente / tartan weave: vertical warp strips and horizontal weft strips \
         interlace in a floating-weave pattern; strips appear as progress rises and \
         the crossing pattern is animated by time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Strip width in dots (both warp and weft).
        let strip = ((dh / 4).max(1)).min(6).min(dw);
        let n_warp = (dw / (strip * 2).max(1)).max(1);
        let n_weft = (dh / (strip * 2).max(1)).max(1);

        let filled_w = (ctx.eased * dw as f32).round() as usize;
        let filled_h = (ctx.eased * dh as f32).round() as usize;

        // Time-driven phase shifts which strips appear to float over or under.
        let phase = (ctx.time * 0.5) as usize;

        // Draw warp (vertical) strips across the progress-filled width.
        for warp in 0..n_warp {
            let x0 = warp * strip * 2;
            if x0 >= filled_w {
                break;
            }
            let x1 = (x0 + strip).min(filled_w).min(dw);
            // Draw this vertical strip for its full height.
            for x in x0..x1 {
                for y in 0..dh {
                    // Leave gaps where weft floats over (cross-check with weft stripe).
                    let weft_idx = y / (strip * 2);
                    let in_weft = (y % (strip * 2)) < strip;
                    // Alternate which crosses on top using the phase.
                    let warp_on_top = (warp + weft_idx + phase) % 2 == 0;
                    if !in_weft || warp_on_top {
                        draw::dot(grid, x, y);
                    }
                }
            }
        }

        // Draw weft (horizontal) strips across the progress-filled height.
        for weft in 0..n_weft {
            let y0 = weft * strip * 2;
            if y0 >= filled_h {
                break;
            }
            let y1 = (y0 + strip).min(filled_h).min(dh);
            for y in y0..y1 {
                for x in 0..dw {
                    let warp_idx = x / (strip * 2);
                    let in_warp = (x % (strip * 2)) < strip;
                    let warp_on_top = (warp_idx + weft + phase) % 2 == 0;
                    // Weft draws only where it floats over warp, or in the gap.
                    if !in_warp || !warp_on_top {
                        draw::dot(grid, x, y);
                    }
                }
            }
        }

        // Tint warp and weft rows with contrasting palette samples.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }
        Ok(())
    }
}
