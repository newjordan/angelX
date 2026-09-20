//! Golden-ratio sacred-geometry progress bars.
//!
//! Every style is built from φ (the golden ratio) and its consequences:
//! Fibonacci tiling, the logarithmic spiral, pentagonal symmetry, the golden
//! gnomon, Kepler's triangle, and the 137.5° golden angle. All shapes are
//! constructed with exact φ-based mathematics and revealed by `ctx.eased`
//! while `ctx.time` drives continuous rotation or animation.
//!
//! Style catalogue:
//! - `golden-spiral`        — quarter-circle arcs in Fibonacci squares forming the logarithmic spiral
//! - `golden-rectangle`     — recursive whirling-rectangles φ-subdivision
//! - `fibonacci-squares`    — outward tiling of Fibonacci-sized squares
//! - `pentagram`            — nested pentagram-in-pentagon revealing φ self-similarity
//! - `pentagon-nest`        — concentric pentagons scaled by 1/φ², rotating
//! - `golden-gnomon`        — 36-72-72 golden triangle recursive subdivision
//! - `phi-phyllotaxis-pent` — five-fold seed arrangement at golden angle
//! - `nautilus`             — chambered logarithmic spiral r=a·φ^(θ·2/π) with cross-walls
//! - `golden-angle-rays`    — rays at successive 137.5° increments, length growing
//! - `dodecagram`           — 12-pointed φ-star (string-art on a 12-gon)
//! - `kepler-triangle`      — right triangle with sides 1, √φ, φ (recursive fan)

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

/// The golden ratio φ = (1 + √5) / 2.
const PHI: f32 = 1.618_033_9;

// ─────────────────────────────────────────────────────────────────────────────
// Registry
// ─────────────────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — burnished gold.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(255, 204, 92);
const TINT_END: Color = Color::rgb(196, 120, 28);

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

/// All styles in the `goldenratio` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per golden-ratio / φ-geometry bar.
/// Styles are ordered from the most recognisable (golden spiral) to the most
/// exotic (Kepler triangle fan), but are fully independent.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(GoldenSpiral)),
        Box::new(Tinted(GoldenRectangle)),
        Box::new(Tinted(FibonacciSquares)),
        Box::new(Tinted(Pentagram)),
        Box::new(Tinted(PentagonNest)),
        Box::new(Tinted(GoldenGnomon)),
        Box::new(Tinted(PhiPhyllotaxisPent)),
        Box::new(Tinted(Nautilus)),
        Box::new(Tinted(GoldenAngleRays)),
        Box::new(Tinted(Dodecagram)),
        Box::new(Tinted(KeplerTriangle)),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Grid center in dot-space (floating-point).
#[inline]
fn center(dw: usize, dh: usize) -> (f32, f32) {
    (dw as f32 / 2.0, dh as f32 / 2.0)
}

/// Uniform scale so a unit-radius figure fits the grid with a small margin.
#[inline]
fn fit_scale(dw: usize, dh: usize) -> f32 {
    let hw = (dw as f32 / 2.0 - 1.0).max(1.0);
    let hh = (dh as f32 / 2.0 - 1.0).max(1.0);
    hw.min(hh)
}

/// Integer Bresenham line between two signed dot-space points.
/// Out-of-bounds dots are silently discarded by `draw::dot_i`.
fn bresenham(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let max_steps = (dx.abs() + dy.abs() + 2) as usize;
    let mut steps = 0usize;
    loop {
        draw::dot_i(grid, x, y);
        if x == x1 && y == y1 {
            break;
        }
        steps += 1;
        if steps > max_steps {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Draw a circular arc from angle `a0` to `a1` (radians), radius `r` dots,
/// centered at dot-space `(cx, cy)`.  Step count is proportional to arc length.
fn arc(grid: &mut BrailleGrid, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
    if r < 0.5 {
        return;
    }
    let arc_len = (a1 - a0).abs() * r;
    let steps = (arc_len * 2.0).round() as usize;
    let steps = steps.max(2);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let angle = a0 + t * (a1 - a0);
        let px = (cx + r * angle.cos()).round() as i32;
        let py = (cy - r * angle.sin()).round() as i32;
        draw::dot_i(grid, px, py);
    }
}

/// Draw a regular N-gon in dot-space, centered at (cx, cy), radius r dots,
/// base rotation `rot` radians. Returns the vertex coordinates.
fn ngon_vertices(n: usize, cx: f32, cy: f32, r: f32, rot: f32) -> Vec<(i32, i32)> {
    (0..n)
        .map(|i| {
            let angle = rot + 2.0 * PI * i as f32 / n as f32;
            (
                (cx + r * angle.cos()).round() as i32,
                (cy - r * angle.sin()).round() as i32,
            )
        })
        .collect()
}

/// Draw the outline of a regular N-gon.
fn ngon_outline(grid: &mut BrailleGrid, n: usize, cx: f32, cy: f32, r: f32, rot: f32) {
    let verts = ngon_vertices(n, cx, cy, r, rot);
    for i in 0..n {
        let (x0, y0) = verts[i];
        let (x1, y1) = verts[(i + 1) % n];
        bresenham(grid, x0, y0, x1, y1);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Golden Spiral — quarter-circle arcs inscribed in Fibonacci squares
// ─────────────────────────────────────────────────────────────────────────────
//
// Each Fibonacci square hosts a quarter-circle arc whose radius equals the
// square's side length.  The squares tile outward in the canonical F1,F1,F2,
// F3,F5,F8… pattern; `ctx.eased` gates how many squares are revealed.

struct GoldenSpiral;
impl ProgressStyle for GoldenSpiral {
    fn name(&self) -> &str {
        "golden-spiral"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Fibonacci-square tiling with quarter-circle arcs forming the golden logarithmic \
         spiral; squares and arcs revealed one step per eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let scale = fit_scale(dw, dh);

        // Pre-compute up to 9 Fibonacci numbers (normalised so the two seeds = 1).
        // We store the relative sizes: 1,1,2,3,5,8,13,21,34.
        // The total rectangle after N squares has dimensions F[N] × F[N+1].
        // We normalise by the largest F so the whole thing fits in `scale` dots.
        let fibs: [f32; 9] = [1.0, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 21.0, 34.0];
        let n_total: usize = 9;
        let n_show = ((ctx.eased * n_total as f32).round() as usize).min(n_total);

        // The bounding rectangle after n_total squares has long side = fibs[n_total-1].
        let long = fibs[n_total - 1];
        // Dot-space unit = scale / long * 2 (×2 for comfortable fit).
        let unit = (scale * 1.8 / long).max(0.5);

        // We build the tiling by tracking the current "pivot corner" in
        // normalised Fibonacci units, then convert to dot-space.
        // The spiral grows: right, up, left, down, right, ... starting from
        // the two unit squares sharing the top-left corner.
        //
        // Pivot meaning: the arc sweeps from the corner diagonally opposite
        // the previous square. We track in (u, v) normalised coords, then
        // map to dot-space centered on the grid.

        // Center of the first 1×1 square in normalised units (left square).
        // We'll accumulate the pivot in normalised space, offset to center later.
        // Row,col offsets in normalised Fibonacci units relative to origin.

        // Direction cycle: right, up, left, down.
        // After placing square of side s in direction d, arc sweeps from the
        // far corner of the previous square.
        //
        // Track the "arc pivot" (normalised) and the arc start angle.
        // We start at origin, facing right.
        // Direction cycle: right, down, left, up (norm-space: y positive = down).
        let dirs: [(f32, f32); 4] = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
        // Arc start angles — one quarter-circle sweep per square, clockwise.
        let arc_starts: [f32; 4] = [PI, 3.0 * PI / 2.0, 0.0, PI / 2.0];

        let mut dir_idx = 0usize;
        // Pivot = arc center in normalised units (starts at origin = grid center).
        let mut px_n: f32 = 0.0;
        let mut py_n: f32 = 0.0;
        let (dcx, dcy) = center(dw, dh);

        for i in 0..n_show {
            let side = fibs[i];
            let a_start = arc_starts[dir_idx % 4];
            let a_end = a_start - PI / 2.0; // quarter circle, clockwise in screen

            // Convert normalised pivot to dot-space.
            let arc_cx = dcx + px_n * unit;
            let arc_cy = dcy + py_n * unit; // norm y positive = down = dot-space y positive

            // Draw the outline of the square.
            let (ddx, ddy) = dirs[dir_idx % 4];
            // The square occupies from (px_n, py_n) extending in the direction
            // perpendicular to current movement.  Just draw the arc; suppress
            // the square outline for cleaner look (arc alone reads well).
            // But on first reveal draw the square too.
            {
                // Square corners relative to pivot in norm units depend on dir.
                // For direction 0 (right): square is to the right, corners at
                // (px_n, py_n), (px_n+s, py_n), (px_n+s, py_n-s), (px_n, py_n-s).
                // For simplicity, derive from dir and perp.
                let perp = match dir_idx % 4 {
                    0 => (0.0, -1.0), // right-moving: square extends up (norm)
                    1 => (1.0, 0.0),  // down-moving:  square extends right
                    2 => (0.0, 1.0),  // left-moving:  square extends down
                    _ => (-1.0, 0.0), // up-moving:    square extends left
                };
                let s = side;
                let c0 = (px_n, py_n);
                let c1 = (px_n + ddx * s, py_n + ddy * s);
                let c2 = (px_n + ddx * s + perp.0 * s, py_n + ddy * s + perp.1 * s);
                let c3 = (px_n + perp.0 * s, py_n + perp.1 * s);
                let corners = [c0, c1, c2, c3];
                for k in 0..4 {
                    let (ax, ay) = corners[k];
                    let (bx, by) = corners[(k + 1) % 4];
                    let p0x = (dcx + ax * unit).round() as i32;
                    let p0y = (dcy + ay * unit).round() as i32;
                    let p1x = (dcx + bx * unit).round() as i32;
                    let p1y = (dcy + by * unit).round() as i32;
                    bresenham(grid, p0x, p0y, p1x, p1y);
                }
            }

            // Draw the arc: radius = side * unit dots.
            arc(grid, arc_cx, arc_cy, side * unit, a_start, a_end);

            // Advance pivot.
            let (adx, ady) = match dir_idx % 4 {
                0 => (side, 0.0),  // moved right → pivot goes right by side
                1 => (0.0, side),  // moved down  → pivot goes down by side
                2 => (-side, 0.0), // moved left  → pivot goes left by side
                _ => (0.0, -side), // moved up    → pivot goes up by side
            };
            // Arc pivot for the NEXT step is at the far corner of the square just drawn.
            // That corner is pivot + current_dir*side (the arc center was at start pivot).
            px_n += adx;
            py_n += ady;
            dir_idx += 1;
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Golden Rectangle — recursive whirling-rectangles φ-subdivision
// ─────────────────────────────────────────────────────────────────────────────
//
// Start with a φ-rectangle. Cut a square from the long side → leaves a smaller
// φ-rectangle. Recurse. The squares + remaining rectangles are the "whirling
// rectangles". `ctx.eased` controls recursion depth.

struct GoldenRectangle;
impl ProgressStyle for GoldenRectangle {
    fn name(&self) -> &str {
        "golden-rectangle"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Recursive φ-rectangle subdivision into a square + smaller φ-rectangle — \
         the whirling-rectangles diagram, with each subdivision revealed as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);

        // Fit the initial φ-rectangle (width = PHI, height = 1) into the grid.
        // Scale so width fits within dw, height within dh.
        let scale_w = (dw as f32 - 2.0) / PHI;
        let scale_h = (dh as f32 - 2.0) / 1.0;
        let unit = scale_w.min(scale_h).max(1.0);

        // Initial rectangle corners in dot-space (top-left origin).
        let rect_w = PHI * unit;
        let rect_h = 1.0 * unit;
        let ox = ((dw as f32 - rect_w) / 2.0).max(0.0);
        let oy = ((dh as f32 - rect_h) / 2.0).max(0.0);

        let max_depth: usize = 10;
        let depth = ((ctx.eased * max_depth as f32).round() as usize)
            .min(max_depth)
            .max(1);

        // Recursive subdivision: (x0, y0, w, h, horizontal_cut)
        // horizontal_cut=true means we cut a square from the left side (width = h).
        let rects: Vec<(f32, f32, f32, f32, bool)> = vec![(ox, oy, rect_w, rect_h, true)];
        let mut drawn = 0usize;

        // Draw outer rectangle always.
        {
            let (x0, y0, w, h, _) = rects[0];
            draw::rect_outline(
                grid,
                x0.round() as usize,
                y0.round() as usize,
                w.round().max(1.0) as usize,
                h.round().max(1.0) as usize,
            );
        }
        drawn += 1;

        let mut next_rects: Vec<(f32, f32, f32, f32, bool)> = Vec::new();
        let mut current = rects.clone();

        for _d in 0..depth.saturating_sub(1) {
            next_rects.clear();
            for &(x0, y0, w, h, horiz) in &current {
                if horiz {
                    // Cut a square of side h from the left.
                    let sq = h;
                    if sq >= 1.0 && w - sq >= 1.0 {
                        // Draw the dividing line between square and remainder.
                        let lx = (x0 + sq).round() as usize;
                        let ly0 = y0.round() as usize;
                        let ly1 = (y0 + h - 1.0).round() as usize;
                        if lx < dw {
                            draw::vline(
                                grid,
                                lx,
                                ly0.min(dh.saturating_sub(1)),
                                ly1.min(dh.saturating_sub(1)),
                            );
                        }
                        // The remaining rectangle is vertical (h > w).
                        next_rects.push((x0 + sq, y0, w - sq, h, false));
                        drawn += 1;
                    }
                } else {
                    // Cut a square of side w from the top.
                    let sq = w;
                    if sq >= 1.0 && h - sq >= 1.0 {
                        let ly = (y0 + sq).round() as usize;
                        let lx0 = x0.round() as usize;
                        let lx1 = (x0 + w - 1.0).round() as usize;
                        if ly < dh {
                            draw::hline(
                                grid,
                                lx0.min(dw.saturating_sub(1)),
                                lx1.min(dw.saturating_sub(1)),
                                ly,
                            );
                        }
                        next_rects.push((x0, y0 + sq, w, h - sq, true));
                        drawn += 1;
                    }
                }
                if drawn >= depth * 2 {
                    break;
                }
            }
            if next_rects.is_empty() {
                break;
            }
            current = next_rects.clone();
        }
        let _ = drawn;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Fibonacci Squares — outward tiling of F₁,F₁,F₂,F₃,F₅,F₈,…
// ─────────────────────────────────────────────────────────────────────────────
//
// Draws only the square outlines (not the arcs) so the tiling geometry is the
// hero.  Each new square appears as `ctx.eased` steps forward.  The tiling is
// centered and scaled to fit any grid.

struct FibonacciSquares;
impl ProgressStyle for FibonacciSquares {
    fn name(&self) -> &str {
        "fibonacci-squares"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Outward Fibonacci-square tiling (1,1,2,3,5,8,13,21,34…): each square \
         outline appears as progress advances, building the canonical φ-rectangle mosaic"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (dcx, dcy) = center(dw, dh);

        let fibs: [f32; 10] = [1.0, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 21.0, 34.0, 55.0];
        let n_total: usize = 10;
        let n_show = ((ctx.eased * n_total as f32).round() as usize).min(n_total);

        // Scale: the long side of the 10-square rectangle = 55 units.
        let long = fibs[n_total - 1];
        let unit = ((dw.min(dh) as f32 - 2.0) / long * 1.6).max(0.5);

        // Pivot tracking: same as golden-spiral but only rectangles.
        let dirs: [(f32, f32); 4] = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
        let perps: [(f32, f32); 4] = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];

        let mut px_n: f32 = 0.0;
        let mut py_n: f32 = 0.0;
        let mut dir_idx = 0usize;

        for i in 0..n_show {
            let side = fibs[i];
            let (ddx, ddy) = dirs[dir_idx % 4];
            let (ppx, ppy) = perps[dir_idx % 4];
            let s = side;
            // Four corners in norm units.
            let c0 = (px_n, py_n);
            let c1 = (px_n + ddx * s, py_n + ddy * s);
            let c2 = (px_n + ddx * s + ppx * s, py_n + ddy * s + ppy * s);
            let c3 = (px_n + ppx * s, py_n + ppy * s);
            let corners = [c0, c1, c2, c3];
            for k in 0..4 {
                let (ax, ay) = corners[k];
                let (bx, by) = corners[(k + 1) % 4];
                let p0x = (dcx + ax * unit).round() as i32;
                let p0y = (dcy + ay * unit).round() as i32;
                let p1x = (dcx + bx * unit).round() as i32;
                let p1y = (dcy + by * unit).round() as i32;
                bresenham(grid, p0x, p0y, p1x, p1y);
            }
            // Advance pivot.
            let step = match dir_idx % 4 {
                0 => (side, 0.0),
                1 => (0.0, side),
                2 => (-side, 0.0),
                _ => (0.0, -side),
            };
            px_n += step.0;
            py_n += step.1;
            dir_idx += 1;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Pentagram — nested pentagram-in-pentagon (the infinite pentagram)
// ─────────────────────────────────────────────────────────────────────────────
//
// Each diagonal of a regular pentagon is φ times the side.  Inside any
// pentagram lives a smaller regular pentagon (scaled by 1/φ²), which in turn
// holds another pentagram. `ctx.eased` controls nesting depth; `ctx.time`
// rotates the stack.

struct Pentagram;
impl ProgressStyle for Pentagram {
    fn name(&self) -> &str {
        "pentagram"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Nested pentagram-in-pentagon: each star's inner pentagon contains a smaller \
         pentagram scaled by 1/φ², revealing infinite φ self-similarity as progress deepens"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let base_r = fit_scale(dw, dh) * 0.95;

        let max_depth: usize = 6;
        let depth = ((ctx.eased * max_depth as f32).ceil() as usize)
            .min(max_depth)
            .max(1);
        let rot0 = ctx.time * 0.2 - PI / 2.0; // point upward, rotate with time

        // Draw from outermost to innermost.
        let inner_scale = 1.0 / (PHI * PHI); // each level shrinks by 1/φ²

        let mut r = base_r;
        let mut rot = rot0;

        for _ in 0..depth {
            if r < 1.0 {
                break;
            }
            // Draw pentagram: connect vertices 0-2-4-1-3-0.
            let verts = ngon_vertices(5, cx, cy, r, rot);
            let star_order = [0usize, 2, 4, 1, 3, 0];
            for k in 0..5 {
                let (x0, y0) = verts[star_order[k]];
                let (x1, y1) = verts[star_order[k + 1]];
                bresenham(grid, x0, y0, x1, y1);
            }
            // Draw the enclosing pentagon.
            ngon_outline(grid, 5, cx, cy, r, rot);

            r *= inner_scale;
            rot += PI / 5.0; // each inner pentagon is rotated by 36°
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Pentagon Nest — concentric pentagons each scaled by 1/φ², rotating
// ─────────────────────────────────────────────────────────────────────────────
//
// Purely the concentric pentagons without the star chords.  Each ring slowly
// counter-rotates against `ctx.time`, and the nest grows as `ctx.eased` rises.

struct PentagonNest;
impl ProgressStyle for PentagonNest {
    fn name(&self) -> &str {
        "pentagon-nest"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Concentric pentagons each scaled 1/φ² smaller, counter-rotating with time — \
         a hypnotic tunnel of five-fold symmetry"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let base_r = fit_scale(dw, dh) * 0.95;

        let max_rings: usize = 8;
        let rings = ((ctx.eased * max_rings as f32).ceil() as usize)
            .min(max_rings)
            .max(1);
        let inner_scale = 1.0 / (PHI * PHI);

        let mut r = base_r;
        for i in 0..rings {
            if r < 1.0 {
                break;
            }
            // Alternate rotation direction per ring.
            let sign = if i % 2 == 0 { 1.0_f32 } else { -1.0_f32 };
            let rot = ctx.time * 0.15 * sign - PI / 2.0 + i as f32 * 0.1;
            ngon_outline(grid, 5, cx, cy, r, rot);
            r *= inner_scale;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Golden Gnomon — 36-72-72 golden triangle recursive subdivision
// ─────────────────────────────────────────────────────────────────────────────
//
// The golden gnomon is the 36-72-72 isoceles triangle where the ratio of the
// long side to the short side equals φ. Subdividing the base triangle with a
// bisector from the apex yields a smaller golden gnomon + a golden triangle
// (36-36-108). Revealed with `ctx.eased` depth.

struct GoldenGnomon;
impl ProgressStyle for GoldenGnomon {
    fn name(&self) -> &str {
        "golden-gnomon"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "36-72-72 golden gnomon recursively bisected into smaller golden triangles — \
         the geometric basis of Penrose tilings, depth revealed with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh) * 0.9;
        let rot0 = ctx.time * 0.18 - PI / 2.0;

        let max_depth: usize = 5;
        let depth = ((ctx.eased * max_depth as f32).ceil() as usize)
            .min(max_depth)
            .max(1);

        // Golden gnomon (36-72-72): apex angle 36°, two base angles 72°.
        // Side ratios: two equal long sides = 1, short base = 1/φ.
        // Place apex at center, base below.
        let half_base_angle = 72.0_f32.to_radians(); // half of the full apex angle
        let apex_angle = 36.0_f32.to_radians();

        // Apex at center, long sides of length `scale`.
        // The two base vertices are at angle ±(90° - 72°) = ±18° from vertical.
        // More precisely: apex at top, each long side extends at angle ±(90 - 72) = ±18° from down.
        let _ = half_base_angle;
        let _ = apex_angle;

        // Triangle vertices (unit long side = 1):
        // Apex: (0, 0)
        // Base left: (sin(-36°), cos(36°)) * scale ... actually use exact computation:
        // With apex at origin, long side = 1, base vertices at ±sin(36°), cos(36°).
        let sin36 = 36.0_f32.to_radians().sin();
        let cos36 = 36.0_f32.to_radians().cos();

        // We'll represent each triangle as three vertices in dot-space and
        // recursively subdivide.  Triangle type: G = gnomon (36-72-72),
        // T = golden triangle (36-36-108).
        // For display, just draw the triangles at each depth level.
        // Stack: Vec<(A, B, C, is_gnomon)> where A is apex of gnomon or vertex of golden-tri.

        type Pt = (f32, f32);
        fn rot_pt(p: Pt, angle: f32) -> Pt {
            (
                p.0 * angle.cos() - p.1 * angle.sin(),
                p.0 * angle.sin() + p.1 * angle.cos(),
            )
        }
        fn lerp_pt(a: Pt, b: Pt, t: f32) -> Pt {
            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
        }
        fn draw_tri(grid: &mut BrailleGrid, cx: f32, cy: f32, a: Pt, b: Pt, c: Pt) {
            let to_dot = |p: Pt| ((cx + p.0).round() as i32, (cy - p.1).round() as i32);
            let (ax, ay) = to_dot(a);
            let (bx, by) = to_dot(b);
            let (ccx, ccy) = to_dot(c);
            bresenham(grid, ax, ay, bx, by);
            bresenham(grid, bx, by, ccx, ccy);
            bresenham(grid, ccx, ccy, ax, ay);
        }

        // Initial gnomon: apex at center top, base at bottom.
        // Long side = scale; base = scale / PHI.
        let apex: Pt = (0.0, cos36 * scale);
        let base_l: Pt = (-sin36 * scale, 0.0);
        let base_r: Pt = (sin36 * scale, 0.0);

        // Rotate by rot0.
        let apex = rot_pt(apex, rot0);
        let base_l = rot_pt(base_l, rot0);
        let base_r = rot_pt(base_r, rot0);

        // is_gnomon = true: 36-72-72 gnomon; false: golden triangle (36-36-108).
        // Subdivision:
        //   Gnomon (A=apex 36°, B, C base): place P on AB at AP = BC = 1/PHI * AB.
        //     → gnomon(P, A, C) [rotated/reflected] + golden-tri(B, P, C).
        //   Golden-tri (A=apex 108°, B, C base): place P on AB at AP = 1/PHI.
        //     → gnomon(C, P, A) + golden-tri(B, P, C).

        let mut tris: Vec<(Pt, Pt, Pt, bool)> = vec![(apex, base_l, base_r, true)];

        for _d in 0..depth.saturating_sub(1) {
            let mut next: Vec<(Pt, Pt, Pt, bool)> = Vec::new();
            for &(a, b, c, is_gnomon) in &tris {
                if is_gnomon {
                    // Gnomon subdivision: P on AB s.t. AP = 1/PHI * |AB|.
                    let p = lerp_pt(a, b, 1.0 / PHI);
                    next.push((p, a, c, true)); // smaller gnomon
                    next.push((b, p, c, false)); // golden triangle
                } else {
                    // Golden-tri: P on AB s.t. AP = 1/PHI * |AB|.
                    let p = lerp_pt(a, b, 1.0 / PHI);
                    next.push((c, p, a, true)); // gnomon
                    next.push((b, p, c, false)); // smaller golden-tri
                }
            }
            // Draw all triangles at this depth.
            for &(a, b, c, _) in &next {
                draw_tri(grid, cx, cy, a, b, c);
            }
            tris = next;
            if tris.len() > 128 {
                break;
            } // cap for degenerate/large grids
        }

        // Always draw the outermost triangle.
        draw_tri(grid, cx, cy, apex, base_l, base_r);

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. φ-Phyllotaxis (pentagonal) — five-fold seed arrangement
// ─────────────────────────────────────────────────────────────────────────────
//
// Standard phyllotaxis but with a modular constraint: only seeds whose index
// is congruent to 0, 1, 2, 3, 4 (mod 5) are drawn in five distinct angular
// sectors separated by 72°.  This produces five spiral arms and five-fold
// symmetry, unlike the usual all-seeds sunflower.

struct PhiPhyllotaxisPent;
impl ProgressStyle for PhiPhyllotaxisPent {
    fn name(&self) -> &str {
        "phi-phyllotaxis-pent"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Five-fold phyllotaxis: seeds placed at the golden angle but grouped into five \
         symmetric spiral arms, creating pentagonal lattice symmetry with φ spacing"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        // Golden angle in radians.
        let golden_angle = 2.0 * PI / (PHI * PHI); // ≈ 137.508°
        let n_max: usize = 300;
        let n_plot = ((ctx.eased * n_max as f32).round() as usize).min(n_max);
        let c = scale / (n_max as f32).sqrt();
        let rot = ctx.time * 0.12;

        // Five sectors: seed n belongs to sector (n % 5).
        // Draw seeds in each sector at their natural positions.
        // Connect adjacent seeds within each arm with short lines for extra
        // visual texture (unlike geometry.rs plain phyllotaxis which just plots dots).
        let mut sector_prev: [Option<(i32, i32)>; 5] = [None; 5];

        for n in 0..n_plot {
            let angle = n as f32 * golden_angle + rot;
            let r = c * (n as f32).sqrt();
            let px = (cx + r * angle.cos()).round() as i32;
            let py = (cy - r * angle.sin()).round() as i32;
            let sector = n % 5;
            // Connect to previous point in the same sector with a short chord.
            if let Some((lx, ly)) = sector_prev[sector] {
                bresenham(grid, lx, ly, px, py);
            } else {
                draw::dot_i(grid, px, py);
            }
            sector_prev[sector] = Some((px, py));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Nautilus — chambered logarithmic spiral with cross-walls
// ─────────────────────────────────────────────────────────────────────────────
//
// True φ-logarithmic spiral r = a·φ^(θ·2/π) (one φ-factor per quarter turn).
// Cross-walls ("septa") divide the spiral into chambers every π/2 radians,
// drawn as radial line segments from the inner to the outer wall.

struct Nautilus;
impl ProgressStyle for Nautilus {
    fn name(&self) -> &str {
        "nautilus"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "φ-logarithmic nautilus spiral r=a·φ^(θ·2/π) with chambered septa every \
         quarter turn — the classic cephalopod shell built from pure golden-ratio growth"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        // The 0.55 vertical squash below means height limits the shell at
        // dh/2/0.55, not dh/2 — use the larger budget so the shell fills.
        let scale = (dh as f32 / 2.0 / 0.55 - 1.0)
            .min(dw as f32 / 2.0 - 1.0)
            .max(2.0);

        // Spiral: r(θ) = φ^(k·(θ−θ_total)) — three turns, exponent compressed
        // so the innermost whorl is still ~a tenth of the outer radius.
        // (The raw φ^(θ·2/π) over four turns spans a 2000:1 radius range and
        // renders sub-dot for three of the four turns.)
        let theta_total = 6.0 * PI;
        let theta_max = ctx.eased * theta_total;
        let rot = ctx.time * 0.15;
        let k = 0.75 / PI;

        let steps = ((theta_max / (2.0 * PI) * 140.0).round() as usize).max(2);

        for i in 0..=steps {
            let theta = i as f32 / steps as f32 * theta_max;
            let r = PHI.powf(k * (theta - theta_total)) * scale;
            let angle = theta + rot;
            let px = (cx + r * angle.cos()).round() as i32;
            let py = (cy - r * angle.sin() * 0.55).round() as i32;
            draw::dot_i(grid, px, py);
            draw::dot_i(grid, px + 1, py);
        }

        // Draw septa (cross-walls) every π/2 radians, from inner to outer wall.
        let n_septa = (theta_max / (PI / 2.0)).floor() as usize;
        for s in 0..=n_septa {
            let theta_wall = s as f32 * PI / 2.0;
            if theta_wall > theta_max {
                break;
            }
            // Inner radius: one turn back (θ - 2π), or 0 if s < 4.
            let r_outer = PHI.powf(k * (theta_wall - theta_total)) * scale;
            let theta_inner = theta_wall - 2.0 * PI;
            let r_inner = if theta_inner > 0.0 {
                PHI.powf(k * (theta_inner - theta_total)) * scale
            } else {
                0.0
            };
            let angle = theta_wall + rot;
            let x_outer = (cx + r_outer * angle.cos()).round() as i32;
            let y_outer = (cy - r_outer * angle.sin() * 0.55).round() as i32;
            let x_inner = (cx + r_inner * angle.cos()).round() as i32;
            let y_inner = (cy - r_inner * angle.sin() * 0.55).round() as i32;
            bresenham(grid, x_inner, y_inner, x_outer, y_outer);
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Golden-Angle Rays — successive rays at 137.5° increments
// ─────────────────────────────────────────────────────────────────────────────
//
// Rays emanate from the center at successive multiples of the golden angle
// (137.508° ≈ 2π/φ²). Each ray is longer than the last by factor φ^(1/n),
// so the whole pattern never repeats and fills space evenly.

struct GoldenAngleRays;
impl ProgressStyle for GoldenAngleRays {
    fn name(&self) -> &str {
        "golden-angle-rays"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Rays at successive 137.5° golden-angle increments from the center, each \
         growing longer — the irrational spacing that prevents clustering and produces \
         optimal coverage of the disk"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        let golden_angle = 2.0 * PI / (PHI * PHI); // ≈ 137.508°
        let n_max: usize = 55; // 55 = F₁₀, natural Fibonacci count
        let n_rays = ((ctx.eased * n_max as f32).round() as usize).min(n_max);
        let rot = ctx.time * 0.1;

        for i in 0..n_rays {
            let angle = i as f32 * golden_angle + rot;
            // Ray length grows with index, bounded by scale.
            let t = (i + 1) as f32 / n_max as f32;
            let len = scale * t; // linear growth for clarity
            let x_end = (cx + len * angle.cos()).round() as i32;
            let y_end = (cy - len * angle.sin()).round() as i32;
            let cx_i = cx.round() as i32;
            let cy_i = cy.round() as i32;
            bresenham(grid, cx_i, cy_i, x_end, y_end);
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Dodecagram — 12-pointed φ-star via string-art on a 12-gon
// ─────────────────────────────────────────────────────────────────────────────
//
// Connect vertex i to vertex (i + 5) mod 12 on a regular 12-gon.  This
// produces a {12/5} dodecagram.  The ratio of chord to side of the 12-gon
// contains φ.  Chords appear one by one with progress.

struct Dodecagram;
impl ProgressStyle for Dodecagram {
    fn name(&self) -> &str {
        "dodecagram"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Dodecagram {12/5}: connecting every 5th vertex of a regular 12-gon produces \
         a twelve-pointed φ-star; chords materialise progressively while the star rotates"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh) * 0.92;

        let n: usize = 12;
        let step: usize = 5; // {12/5} star polygon
        let rot = ctx.time * 0.14;
        let n_chords = ((ctx.eased * n as f32).round() as usize).min(n);

        // Draw the outer 12-gon outline always.
        ngon_outline(grid, n, cx, cy, scale, rot);

        // Draw the star chords.
        let verts = ngon_vertices(n, cx, cy, scale, rot);
        for i in 0..n_chords {
            let j = (i + step) % n;
            let (x0, y0) = verts[i];
            let (x1, y1) = verts[j];
            bresenham(grid, x0, y0, x1, y1);
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. Kepler Triangle — right triangle with sides 1 : √φ : φ
// ─────────────────────────────────────────────────────────────────────────────
//
// The Kepler triangle has the unique property that its sides form a geometric
// progression 1, √φ, φ, so the hypotenuse-to-leg ratio embeds φ.
// We render a fan of Kepler triangles tiled around the center, each rotated by
// the golden angle, revealing more triangles as progress rises.

struct KeplerTriangle;
impl ProgressStyle for KeplerTriangle {
    fn name(&self) -> &str {
        "kepler-triangle"
    }
    fn theme(&self) -> &str {
        "goldenratio"
    }
    fn describe(&self) -> &str {
        "Kepler triangle fan: right triangles with sides 1:√φ:φ tiled around the \
         center, each rotated by the golden angle — where Pythagoras meets φ"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        // Kepler triangle sides in proportion: a=1, b=√φ, c=φ.
        // We scale so the hypotenuse (φ) = scale.
        let a = scale / PHI; // short leg
        let b = scale / PHI.sqrt(); // long leg (= √φ * a/1 * scale/φ simplified)
                                    // Verify: a² + b² = (scale/PHI)² + (scale/PHI.sqrt())²
                                    //       = scale²(1/φ² + 1/φ) = scale²((1+φ)/φ²) = scale²(φ²/φ²) = scale² ✓

        let golden_angle = 2.0 * PI / (PHI * PHI);
        let n_max: usize = 34; // F₉
        let n_tris = ((ctx.eased * n_max as f32).round() as usize).min(n_max);
        let rot_base = ctx.time * 0.1;

        for i in 0..n_tris {
            let rot = i as f32 * golden_angle + rot_base;
            // Right-angle vertex at center.
            // Leg a along rot direction, leg b perpendicular (rot + π/2).
            let tip_a = (cx + a * rot.cos(), cy - a * rot.sin());
            let tip_b = (
                cx + b * (rot + PI / 2.0).cos(),
                cy - b * (rot + PI / 2.0).sin(),
            );
            let origin = (cx.round() as i32, cy.round() as i32);
            let (ax, ay) = (tip_a.0.round() as i32, tip_a.1.round() as i32);
            let (bx, by) = (tip_b.0.round() as i32, tip_b.1.round() as i32);
            // Draw the three sides of the Kepler triangle.
            bresenham(grid, origin.0, origin.1, ax, ay); // short leg
            bresenham(grid, origin.0, origin.1, bx, by); // long leg
            bresenham(grid, ax, ay, bx, by); // hypotenuse
        }
        Ok(())
    }
}
