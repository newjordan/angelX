//! Fractal-themed progress bars — hard mathematics rendered in braille dots.
//!
//! Each style implements a distinct fractal or iterated-function-system formula,
//! mapping `ctx.eased` to a reveal / iteration parameter and `ctx.time` to
//! continuous animation (zoom, rotation, c-parameter drift). All coordinate
//! arithmetic is done in `f32`/`i32` and committed via `draw::dot_i` so that
//! negative values from fractal math never cause panics at small grid sizes.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Deterministic hash (no external crates)
// ---------------------------------------------------------------------------

#[inline]
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

// ---------------------------------------------------------------------------
// Public registry
// ---------------------------------------------------------------------------

/// All styles in the `fractal` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per style, in display order. Each
/// style maps `ctx.eased` to a mathematical reveal parameter (iteration depth,
/// point count, recursion level) and `ctx.time` to continuous animation.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(MandelbrotEscape),
        Box::new(JuliaSet),
        Box::new(SierpinskiTriangle),
        Box::new(KochCurve),
        Box::new(BarnsleyFern),
        Box::new(DragonCurve),
        Box::new(SierpinskiCarpet),
        Box::new(BurningShip),
        Box::new(PythagorasTree),
        Box::new(NewtonFractal),
        Box::new(CantorDust),
        Box::new(LyapunovBar),
    ]
}

// ---------------------------------------------------------------------------
// 1. Mandelbrot escape-time
// ---------------------------------------------------------------------------

struct MandelbrotEscape;
impl ProgressStyle for MandelbrotEscape {
    fn name(&self) -> &str {
        "mandelbrot-escape"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Mandelbrot set rendered as braille: escape-time threshold rises with progress, \
         viewport pans and zooms with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Max iterations scales with eased progress (2..=48).
        let max_iter = (2.0 + ctx.eased * 46.0) as u32;

        // Animate: gentle zoom and pan into the Seahorse Valley area.
        let zoom = 2.5 / (1.0 + ctx.time * 0.05);
        let cx_center = -0.7269 - ctx.time * 0.002;
        let cy_center = 0.1889;

        let aspect = dw as f32 / dh as f32;

        for py in 0..dh {
            for px in 0..dw {
                // Map dot to complex plane.
                let cr = cx_center + (px as f32 / dw as f32 - 0.5) * zoom * aspect;
                let ci = cy_center + (py as f32 / dh as f32 - 0.5) * zoom;

                let mut zr = 0.0f32;
                let mut zi = 0.0f32;
                let mut escaped = false;
                for _ in 0..max_iter {
                    let zr2 = zr * zr;
                    let zi2 = zi * zi;
                    if zr2 + zi2 > 4.0 {
                        escaped = true;
                        break;
                    }
                    let new_zr = zr2 - zi2 + cr;
                    zi = 2.0 * zr * zi + ci;
                    zr = new_zr;
                }

                // Points that escape → lit; interior stays dark (classic look).
                if escaped {
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Tint the filled portion by progress.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32) as usize;
        for cy in 0..ch {
            for cx in 0..filled_cells.min(cw) {
                let t = if cw <= 1 {
                    0.5
                } else {
                    cx as f32 / (cw - 1) as f32
                };
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Julia set  z → z² + c,  c = 0.7885·e^(i·θ)
// ---------------------------------------------------------------------------

struct JuliaSet;
impl ProgressStyle for JuliaSet {
    fn name(&self) -> &str {
        "julia-set"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Julia set with c=0.7885·e^(i·θ), θ animated by time; resolution and iteration \
         depth grow with progress."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let theta = ctx.time * 0.4;
        let cr = 0.7885 * theta.cos();
        let ci = 0.7885 * theta.sin();

        let max_iter = (4.0 + ctx.eased * 44.0) as u32;
        let scale = 1.5;

        for py in 0..dh {
            for px in 0..dw {
                let mut zr = (px as f32 / dw as f32 - 0.5) * scale * 2.0;
                let mut zi = (py as f32 / dh as f32 - 0.5) * scale * 2.0;

                let mut iters = 0u32;
                while iters < max_iter && zr * zr + zi * zi <= 4.0 {
                    let new_zr = zr * zr - zi * zi + cr;
                    zi = 2.0 * zr * zi + ci;
                    zr = new_zr;
                    iters += 1;
                }

                if iters < max_iter {
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Palette tint: columns cycle through hue based on progress.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            for cx in 0..cw {
                let t = (cx as f32 / cw as f32 + ctx.time * 0.1).fract();
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Sierpinski triangle — chaos-game IFS
// ---------------------------------------------------------------------------

struct SierpinskiTriangle;
impl ProgressStyle for SierpinskiTriangle {
    fn name(&self) -> &str {
        "sierpinski-triangle"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Sierpinski triangle via the chaos game (random midpoint IFS); point count \
         grows with progress, triangle rotates with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let n_points = (10.0 + ctx.eased * 2990.0) as u32;

        // Three vertices of the triangle, rotated by time.
        let rot = ctx.time * 0.3;
        let vertices: [(f32, f32); 3] = [
            (0.5 + 0.45 * (rot).cos(), 0.5 + 0.45 * (rot).sin()),
            (
                0.5 + 0.45 * (rot + 2.094).cos(),
                0.5 + 0.45 * (rot + 2.094).sin(),
            ),
            (
                0.5 + 0.45 * (rot + 4.189).cos(),
                0.5 + 0.45 * (rot + 4.189).sin(),
            ),
        ];

        // Seed — iterate once without drawing to warm up.
        let mut px = 0.5f32;
        let mut py = 0.5f32;
        for i in 0..n_points + 20 {
            let v = (hash(i.wrapping_mul(1_000_003)) % 3) as usize;
            px = (px + vertices[v].0) * 0.5;
            py = (py + vertices[v].1) * 0.5;
            if i >= 20 {
                let dx = (px * dw as f32) as i32;
                let dy = (py * dh as f32) as i32;
                draw::dot_i(grid, dx, dy);
            }
        }

        // Tint by row.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Koch snowflake curve — L-system recursion
// ---------------------------------------------------------------------------

struct KochCurve;
impl ProgressStyle for KochCurve {
    fn name(&self) -> &str {
        "koch-curve"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Koch snowflake drawn as a dot polyline; L-system recursion depth grows \
         with progress (0..=4), rotated and scaled by time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let depth = (ctx.eased * 4.0).floor() as u32;

        // Collect Koch curve segment endpoints via recursive subdivision.
        fn koch_points(ax: f32, ay: f32, bx: f32, by: f32, depth: u32, pts: &mut Vec<(f32, f32)>) {
            if depth == 0 {
                pts.push((bx, by));
                return;
            }
            let dx = bx - ax;
            let dy = by - ay;
            // p1 = 1/3 of the way
            let p1x = ax + dx / 3.0;
            let p1y = ay + dy / 3.0;
            // p3 = 2/3 of the way
            let p3x = ax + 2.0 * dx / 3.0;
            let p3y = ay + 2.0 * dy / 3.0;
            // p2 = peak of the outward triangle (60° rotation of the 1/3 segment)
            let p2x = p1x + (dx / 3.0) * 0.5 - (dy / 3.0) * 0.866;
            let p2y = p1y + (dy / 3.0) * 0.5 + (dx / 3.0) * 0.866;
            koch_points(ax, ay, p1x, p1y, depth - 1, pts);
            koch_points(p1x, p1y, p2x, p2y, depth - 1, pts);
            koch_points(p2x, p2y, p3x, p3y, depth - 1, pts);
            koch_points(p3x, p3y, bx, by, depth - 1, pts);
        }

        // Three sides of a snowflake, centred in the dot grid.
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let r = (dw.min(dh * 2) as f32 * 0.42).max(1.0);
        let rot = ctx.time * 0.2;

        let tri: Vec<(f32, f32)> = (0..3)
            .map(|i| {
                let a = rot + i as f32 * 2.0 * PI / 3.0 - PI / 2.0;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();

        for side in 0..3 {
            let (ax, ay) = tri[side];
            let (bx, by) = tri[(side + 1) % 3];
            let mut pts = vec![(ax, ay)];
            koch_points(ax, ay, bx, by, depth, &mut pts);

            // Draw line segments between consecutive points.
            for w in pts.windows(2) {
                let (x0, y0) = w[0];
                let (x1, y1) = w[1];
                // Bresenham-style plot via parametric steps.
                let steps = ((x1 - x0).abs() + (y1 - y0).abs()).ceil() as u32 + 1;
                let steps = steps.max(1);
                for s in 0..=steps {
                    let t = s as f32 / steps as f32;
                    let px = x0 + t * (x1 - x0);
                    let py = y0 + t * (y1 - y0);
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Tint.
        let (cw, ch) = grid.dimensions();
        for cy_cell in 0..ch {
            let t = cy_cell as f32 / ch.max(1) as f32;
            draw::tint_row(
                grid,
                cy_cell,
                0,
                cw.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Barnsley fern — IFS with 4 affine maps
// ---------------------------------------------------------------------------

struct BarnsleyFern;
impl ProgressStyle for BarnsleyFern {
    fn name(&self) -> &str {
        "barnsley-fern"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Barnsley fern IFS (4 affine maps, exact original coefficients); point count \
         and detail scale with progress; sways gently with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let n_pts = (20.0 + ctx.eased * 2980.0) as u32;

        // Classic Barnsley fern IFS coefficients:
        //  f1: stem        probability 0.01
        //  f2: large leaf  probability 0.85
        //  f3: left leaflet  probability 0.07
        //  f4: right leaflet probability 0.07
        // Each: [a, b, c, d, e, f] for x' = ax+by+e, y' = cx+dy+f
        let maps: [[f32; 6]; 4] = [
            [0.0, 0.0, 0.0, 0.16, 0.0, 0.0],      // stem
            [0.85, 0.04, -0.04, 0.85, 0.0, 1.6],  // large leaf
            [0.20, -0.26, 0.23, 0.22, 0.0, 1.6],  // left leaflet
            [-0.15, 0.28, 0.26, 0.24, 0.0, 0.44], // right leaflet
        ];
        // Cumulative probability thresholds (×1000 for integer hash).
        let thresholds = [10u32, 860, 930, 1000];

        // Gentle sway: perturb e coefficient of map 2 and 3.
        let sway = (ctx.time * 0.5).sin() * 0.04;

        let mut x = 0.0f32;
        let mut y = 0.0f32;

        for i in 0..n_pts + 20 {
            let r = hash(i.wrapping_mul(999_983)) % 1000;
            let map_idx = if r < thresholds[0] {
                0
            } else if r < thresholds[1] {
                1
            } else if r < thresholds[2] {
                2
            } else {
                3
            };

            let m = maps[map_idx];
            let new_x = m[0] * x + m[1] * y + m[4] + if map_idx >= 2 { sway } else { 0.0 };
            let new_y = m[2] * x + m[3] * y + m[5];
            x = new_x;
            y = new_y;

            if i >= 20 {
                // Fern lives in x ∈ [-2.182, 2.6558], y ∈ [0, 9.9983].
                let px = ((x + 2.182) / 4.8378 * dw as f32) as i32;
                let py = ((1.0 - y / 9.9983) * dh as f32) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Green-ish tint via palette.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                cw.saturating_sub(1),
                ctx.palette.sample(1.0 - t),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Dragon curve — paper-folding L-system
// ---------------------------------------------------------------------------

struct DragonCurve;
impl ProgressStyle for DragonCurve {
    fn name(&self) -> &str {
        "dragon-curve"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Dragon curve drawn by paper-folding construction; reveal length grows with \
         progress (up to 12 folds), rotates with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Number of folds determines curve length = 2^n segments.
        let max_folds: u32 = 12;
        let folds = (ctx.eased * max_folds as f32).floor() as u32;
        let folds = folds.min(max_folds);
        let n_segs = 1u32 << folds; // 2^folds

        // Generate turn sequence: for segment k, turn = fold_direction(k).
        // The k-th turn of the dragon curve (0-indexed):
        // T(k) = bit at position (highest power of 2 dividing k+1) shifted odd/even.
        // Equivalently: let k' = (k+1). Turn right if ((k' >> trailing_zeros(k')) >> 1) & 1 == 0.
        fn dragon_turn(k: u32) -> bool {
            // k is 1-indexed segment.
            let tz = k.trailing_zeros();
            ((k >> tz) >> 1) & 1 == 0
        }

        // Walk the curve.
        // Direction: 0=right, 1=up, 2=left, 3=down.
        let rot_offset = (ctx.time * 0.25) as i32; // integer quarter-turns
        let start_dir: i32 = rot_offset.rem_euclid(4);

        // Collect all segment start points to find bounding box for centering.
        let step = 1i32;
        let dx = [step, 0, -step, 0];
        let dy = [0, -step, 0, step];

        let mut cx_i: i32 = 0;
        let mut cy_i: i32 = 0;
        let mut dir: i32 = start_dir;
        let mut pts: Vec<(i32, i32)> = Vec::with_capacity((n_segs + 1) as usize);
        pts.push((cx_i, cy_i));

        for k in 1..=n_segs {
            cx_i += dx[dir as usize];
            cy_i += dy[dir as usize];
            pts.push((cx_i, cy_i));
            if k < n_segs {
                let turn = dragon_turn(k);
                dir = if turn {
                    (dir + 1).rem_euclid(4)
                } else {
                    (dir + 3).rem_euclid(4)
                };
            }
        }

        // Bounding box for centering.
        let min_x = pts.iter().map(|p| p.0).min().unwrap_or(0);
        let max_x = pts.iter().map(|p| p.0).max().unwrap_or(0);
        let min_y = pts.iter().map(|p| p.1).min().unwrap_or(0);
        let max_y = pts.iter().map(|p| p.1).max().unwrap_or(0);
        let span_x = (max_x - min_x).max(1);
        let span_y = (max_y - min_y).max(1);

        for p in &pts {
            let px = (p.0 - min_x) as f32 / span_x as f32 * (dw as f32 - 1.0);
            let py = (p.1 - min_y) as f32 / span_y as f32 * (dh as f32 - 1.0);
            draw::dot_i(grid, px as i32, py as i32);
        }

        // Tint horizontally.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Sierpinski carpet — recursive square removal
// ---------------------------------------------------------------------------

struct SierpinskiCarpet;
impl ProgressStyle for SierpinskiCarpet {
    fn name(&self) -> &str {
        "sierpinski-carpet"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Sierpinski carpet: each dot is tested via the 9-ary digit rule (x&y in base 3 \
         for any digit == 1 → hole); depth driven by progress."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Depth controls which power-of-3 we map to.
        // At depth d we check d digits in base 3.
        let max_depth = (ctx.eased * 5.0).ceil() as u32;
        let max_depth = max_depth.max(1);

        // Scale: use a 3^max_depth × 3^max_depth logical grid.
        let pow3 = 3u32.pow(max_depth);

        // Animate: slow drift in mapping origin.
        let off_x = (ctx.time * 0.4).sin() * 0.08;
        let off_y = (ctx.time * 0.3).cos() * 0.08;

        for py in 0..dh {
            for px in 0..dw {
                // Map dot to carpet coordinates in [0, pow3).
                let fx = ((px as f32 / dw as f32 + off_x).rem_euclid(1.0) * pow3 as f32) as u32;
                let fy = ((py as f32 / dh as f32 + off_y).rem_euclid(1.0) * pow3 as f32) as u32;

                // Sierpinski carpet rule: if any base-3 digit of x AND y both == 1 → hole.
                let mut in_hole = false;
                let mut rx = fx;
                let mut ry = fy;
                for _ in 0..max_depth {
                    if rx % 3 == 1 && ry % 3 == 1 {
                        in_hole = true;
                        break;
                    }
                    rx /= 3;
                    ry /= 3;
                }

                if !in_hole {
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Tint.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32) as usize;
        for cy in 0..ch {
            for cx in 0..filled_cells.min(cw) {
                let t = cx as f32 / cw.max(1) as f32;
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Burning Ship fractal  — |Re(z)|, |Im(z)| before squaring
// ---------------------------------------------------------------------------

struct BurningShip;
impl ProgressStyle for BurningShip {
    fn name(&self) -> &str {
        "burning-ship"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Burning Ship fractal (z → (|Re z|+i|Im z|)² + c); its jagged, ship-like \
         silhouette burns across the bar as progress rises."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let max_iter = (3.0 + ctx.eased * 45.0) as u32;

        // Classic Burning Ship viewport: Re ∈ [-2.5, 1.5], Im ∈ [-2, 0.5].
        // Animate by slowly drifting.
        let drift = ctx.time * 0.015;
        let re_min = -2.5 + drift.sin() * 0.2;
        let re_max = 1.5 + drift.cos() * 0.1;
        let im_min = -2.0 + (drift * 0.7).sin() * 0.15;
        let im_max = 0.5 + (drift * 0.5).cos() * 0.1;

        for py in 0..dh {
            for px in 0..dw {
                let cr = re_min + (px as f32 / dw as f32) * (re_max - re_min);
                let ci = im_min + (py as f32 / dh as f32) * (im_max - im_min);

                let mut zr = 0.0f32;
                let mut zi = 0.0f32;
                let mut escaped = false;
                for _ in 0..max_iter {
                    let zr2 = zr * zr;
                    let zi2 = zi * zi;
                    if zr2 + zi2 > 4.0 {
                        escaped = true;
                        break;
                    }
                    // Key difference: absolute values before squaring.
                    let new_zr = zr2 - zi2 + cr;
                    zi = 2.0 * zr.abs() * zi.abs() + ci;
                    zr = new_zr;
                }

                if escaped {
                    draw::dot_i(grid, px as i32, py as i32);
                }
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

// ---------------------------------------------------------------------------
// 9. Pythagoras tree — recursive binary squares
// ---------------------------------------------------------------------------

struct PythagorasTree;
impl ProgressStyle for PythagorasTree {
    fn name(&self) -> &str {
        "pythagoras-tree"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Pythagoras tree: a square sprouts two smaller squares at a time-animated \
         split angle; branch count grows with progress (depth 0..=7)."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let max_depth = (ctx.eased * 7.0).floor() as u32;
        // Animate split angle α between 20° and 70°.
        let alpha = 0.349 + (ctx.time * 0.2).sin().abs() * 0.524; // 20°..50° in radians

        // Draw a square given its bottom-left corner vector and direction.
        // We represent the square by two corners of its base edge: (x1,y1)-(x2,y2),
        // growing upward (away from the viewer, i.e. toward smaller y in screen coords).
        fn draw_square(grid: &mut BrailleGrid, x1: f32, y1: f32, x2: f32, y2: f32) {
            // Edge vector.
            let ex = x2 - x1;
            let ey = y2 - y1;
            // Perpendicular pointing up on screen (y grows downward): (ey, -ex).
            let x3 = x2 + ey;
            let y3 = y2 - ex;
            let x4 = x1 + ey;
            let y4 = y1 - ex;

            // Draw 4 edges via parametric steps.
            let corners = [(x1, y1), (x2, y2), (x3, y3), (x4, y4), (x1, y1)];
            for w in corners.windows(2) {
                let (ax, ay) = w[0];
                let (bx, by) = w[1];
                let steps = ((bx - ax).abs() + (by - ay).abs()).ceil() as i32 + 1;
                let steps = steps.max(1);
                for s in 0..=steps {
                    let t = s as f32 / steps as f32;
                    let px = ax + t * (bx - ax);
                    let py = ay + t * (by - ay);
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Recursively build the tree.
        fn tree(
            grid: &mut BrailleGrid,
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
            alpha: f32,
            depth: u32,
        ) {
            if depth == 0 {
                return;
            }
            draw_square(grid, x1, y1, x2, y2);

            // Top edge of the square (same screen-up perpendicular as above).
            let ex = x2 - x1;
            let ey = y2 - y1;
            let tx1 = x1 + ey;
            let ty1 = y1 - ex;
            let tx2 = x2 + ey;
            let ty2 = y2 - ex;

            // Split point on the top edge at angle alpha.
            let ca = alpha.cos();
            let sa = alpha.sin();
            let ex2 = tx2 - tx1;
            let ey2 = ty2 - ty1;
            let len = (ex2 * ex2 + ey2 * ey2).sqrt().max(1e-6);
            let ux = ex2 / len;
            let uy = ey2 / len;
            // Left branch base: tx1 to apex, rotated by -alpha so the apex
            // bulges above the top edge (screen-up) instead of folding down.
            let left_size = len * ca;
            let apex_x = tx1 + ux * left_size * ca + uy * left_size * sa;
            let apex_y = ty1 + uy * left_size * ca - ux * left_size * sa;

            // Left child square.
            tree(grid, tx1, ty1, apex_x, apex_y, alpha, depth - 1);
            // Right child square.
            tree(grid, apex_x, apex_y, tx2, ty2, alpha, depth - 1);
        }

        // Root square: centred at the bottom, capped by grid height so the
        // crown (roughly 3x the root side) still fits short bar grids.
        let sq_w = (dw as f32 * 0.28).min(dh as f32 * 0.34).max(2.0);
        let base_y = dh as f32 - 1.0;
        let base_x = dw as f32 / 2.0 - sq_w / 2.0;
        tree(
            grid,
            base_x,
            base_y,
            base_x + sq_w,
            base_y,
            alpha,
            max_depth + 1,
        );

        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = 1.0 - cy as f32 / ch.max(1) as f32; // brighter at top (crown)
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Newton fractal — basins of attraction for z³ - 1 = 0
// ---------------------------------------------------------------------------

struct NewtonFractal;
impl ProgressStyle for NewtonFractal {
    fn name(&self) -> &str {
        "newton-fractal"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Newton fractal for z³−1=0: three root basins painted with palette colors; \
         iteration count threshold rises with progress, domain rotates with time."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let max_iter = (2.0 + ctx.eased * 30.0) as u32;
        let tol2 = 1e-4f32;

        // Three cube roots of unity.
        let roots = [(1.0f32, 0.0f32), (-0.5, 0.866_025_4), (-0.5, -0.866_025_4)];

        let scale = 2.0f32 / (1.0 + ctx.time * 0.04);
        let theta = ctx.time * 0.15;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        for py in 0..dh {
            for px in 0..dw {
                // Map to complex plane, then rotate.
                let raw_r = (px as f32 / dw as f32 - 0.5) * scale * 2.0;
                let raw_i = (py as f32 / dh as f32 - 0.5) * scale * 2.0;
                let mut zr = raw_r * cos_t - raw_i * sin_t;
                let mut zi = raw_r * sin_t + raw_i * cos_t;

                // Newton iteration: z ← z - f(z)/f'(z) = z - (z³-1)/(3z²)
                let mut root_id = 0usize;
                let mut converged = false;
                for _ in 0..max_iter {
                    let zr2 = zr * zr;
                    let zi2 = zi * zi;
                    // z³ = z² * z
                    let z3r = (zr2 - zi2) * zr - 2.0 * zr * zi * zi;
                    let z3i = (zr2 - zi2) * zi + 2.0 * zr * zi * zr;
                    // 3z²
                    let d3r = 3.0 * (zr2 - zi2);
                    let d3i = 6.0 * zr * zi;
                    let d_mag2 = d3r * d3r + d3i * d3i;
                    if d_mag2 < 1e-10 {
                        break;
                    }
                    // (z³-1) / (3z²): numerator real and imag.
                    let nr = z3r - 1.0;
                    let ni = z3i;
                    // Complex division (nr + i·ni) / (d3r + i·d3i).
                    let qr = (nr * d3r + ni * d3i) / d_mag2;
                    let qi = (ni * d3r - nr * d3i) / d_mag2;
                    zr -= qr;
                    zi -= qi;

                    // Check convergence to each root.
                    for (rid, &(rr, ri)) in roots.iter().enumerate() {
                        let dr = zr - rr;
                        let di = zi - ri;
                        if dr * dr + di * di < tol2 {
                            root_id = rid;
                            converged = true;
                            break;
                        }
                    }
                    if converged {
                        break;
                    }
                }

                if converged {
                    // Color by which root basin we landed in.
                    let t = root_id as f32 / 2.0;
                    let (cw, ch) = grid.dimensions();
                    let cell_x = (px / 2).min(cw.saturating_sub(1));
                    let cell_y = (py / 4).min(ch.saturating_sub(1));
                    draw::dot_i(grid, px as i32, py as i32);
                    draw::tint_row(grid, cell_y, cell_x, cell_x, ctx.palette.sample(t));
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Cantor dust — iterated middle-third removal cascade
// ---------------------------------------------------------------------------

struct CantorDust;
impl ProgressStyle for CantorDust {
    fn name(&self) -> &str {
        "cantor-dust"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Cantor dust: 2-D middle-third removal across rows; each row is a deeper \
         level of the Cantor set, revealing from top to bottom with progress."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // How many rows to reveal.
        let revealed_rows = ((ctx.eased * dh as f32).ceil() as usize).min(dh);

        for py in 0..revealed_rows {
            // Row depth: maps row to a Cantor iteration level (0..=7).
            let level = (py as f32 / dh as f32 * 7.0).floor() as u32;
            let pow3 = 3u32.pow(level.min(7));

            // Animate: slow horizontal drift.
            let drift = (ctx.time * 0.3 + py as f32 * 0.05).sin() * 0.1;

            for px in 0..dw {
                // Map x to [0, pow3) with drift.
                let fx = ((px as f32 / dw as f32 + drift).rem_euclid(1.0) * pow3 as f32) as u32;
                // Cantor: point is IN the set iff no base-3 digit == 1.
                let mut in_cantor = true;
                let mut rx = fx;
                for _ in 0..level {
                    if rx % 3 == 1 {
                        in_cantor = false;
                        break;
                    }
                    rx /= 3;
                }
                if in_cantor {
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Tint rows.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.max(1) as f32;
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), ctx.palette.sample(t));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 12. Lyapunov bar — stability exponent rendered as fill intensity
// ---------------------------------------------------------------------------

struct LyapunovBar;
impl ProgressStyle for LyapunovBar {
    fn name(&self) -> &str {
        "lyapunov-bar"
    }
    fn theme(&self) -> &str {
        "fractal"
    }
    fn describe(&self) -> &str {
        "Lyapunov exponent landscape for the logistic map sequence AABB…; each column \
         is a parameter r swept across [2.5, 4.0], lit where the exponent is negative \
         (stable). Progress raises the iteration count; time scrolls the AB sequence phase."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let n_iter = (8.0 + ctx.eased * 92.0) as u32;
        let warmup = 20u32;

        // Sequence AABB (phase scrolled by time).
        // We use a 4-symbol repeating sequence; time shifts which symbol we start on.
        let phase = (ctx.time * 0.5) as u32;

        for px in 0..dw {
            // Map column to parameter a ∈ [2.5, 4.0] and b ∈ [2.5, 4.0] (diagonal sweep).
            let t_col = px as f32 / dw as f32;
            let ra = 2.5 + t_col * 1.5;
            let rb = 2.5 + (1.0 - t_col) * 1.5;

            // Compute Lyapunov exponent.
            let mut x = 0.5f32;
            let mut lam = 0.0f32;
            let seq_len = 4u32;

            // Warmup.
            for i in 0..warmup {
                let r = if (i + phase) % seq_len < 2 { ra } else { rb };
                x = r * x * (1.0 - x);
            }
            // Measure.
            for i in 0..n_iter {
                let r = if (i + phase) % seq_len < 2 { ra } else { rb };
                x = r * x * (1.0 - x);
                let deriv = (r * (1.0 - 2.0 * x)).abs().max(1e-10);
                lam += deriv.ln();
            }
            lam /= n_iter as f32;

            // Negative exponent → stable (chaos-free) → draw column.
            // Positive exponent → chaotic → height proportional to |λ|.
            let col_fill_frac = if lam < 0.0 {
                1.0f32 // fully lit: stable region
            } else {
                (1.0 - (lam / 2.0).min(1.0)).max(0.0) // partial: chaotic
            };

            let col_h = (col_fill_frac * dh as f32).round() as usize;
            let y0 = dh.saturating_sub(col_h);
            for py in y0..dh {
                draw::dot_i(grid, px as i32, py as i32);
            }
        }

        // Tint: palette maps column to hue.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            for cx in 0..cw {
                let t = cx as f32 / cw.max(1) as f32;
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }

        Ok(())
    }
}
