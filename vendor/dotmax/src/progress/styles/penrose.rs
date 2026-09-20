//! Penrose / aperiodic-tiling sacred-geometry progress styles.
//!
//! Ten structurally distinct styles, each built on a real aperiodic or
//! quasi-periodic mathematical construction:
//!
//! - `penrose-p3`            — Rhombus tiling via Robinson-triangle deflation
//! - `penrose-p2`            — Kite & dart tiling via gnomon/triangle subdivision
//! - `sun-pattern`           — The canonical Penrose "sun" (5 kites) seed cluster
//! - `girih-tiles`           — Islamic girih decagon strapwork
//! - `ammann-bars`           — Ammann quasiperiodic line overlay on rhombi
//! - `debruijn-pentagrid`    — de Bruijn pentagrid dual: 5 families at 72°
//! - `decagon-fractal`       — Decagon recursively filled with smaller decagons
//! - `quasicrystal-diffraction` — Sum of 5 plane waves → 10-fold interference pattern
//! - `pinwheel-tiling`       — Pinwheel 1:2 right-triangle substitution tiling
//! - `truchet-quasi`         — Truchet arcs on a quasiperiodic rhombus lattice

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

const PHI: f32 = 1.6180339887;

// ────────────────────────────────────────────────────────────────────────────
// Registry
// ────────────────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — sunset tiling.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(255, 146, 94);
const TINT_END: Color = Color::rgb(146, 86, 255);

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

/// All styles in the `penrose` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per aperiodic-tiling bar, ordered from
/// most iconic (P3 rhombus) to most exotic (Truchet-quasi).  All styles are
/// independent and can be used in any order.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(PenroseP3)),
        Box::new(Tinted(PenroseP2)),
        Box::new(Tinted(SunPattern)),
        Box::new(Tinted(GirihTiles)),
        Box::new(Tinted(AmmannBars)),
        Box::new(Tinted(DeBruijnPentagrid)),
        Box::new(Tinted(DecagonFractal)),
        Box::new(Tinted(QuasicrystalDiffraction)),
        Box::new(Tinted(PinwheelTiling)),
        Box::new(Tinted(TruchetQuasi)),
    ]
}

// ────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ────────────────────────────────────────────────────────────────────────────

/// Grid center in dot-space.
#[inline]
fn center(dw: usize, dh: usize) -> (f32, f32) {
    (dw as f32 * 0.5, dh as f32 * 0.5)
}

/// Uniform scale to fit a unit-radius object in the grid with padding.
#[inline]
fn fit_scale(dw: usize, dh: usize) -> f32 {
    let hw = (dw as f32 * 0.5 - 1.0).max(1.0);
    let hh = (dh as f32 * 0.5 - 1.0).max(1.0);
    hw.min(hh)
}

/// Bresenham line rasteriser. Out-of-bounds dots are silently discarded.
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

/// Draw a polygon defined by a slice of (x,y) dot-space points (closed).
fn draw_poly(grid: &mut BrailleGrid, pts: &[(i32, i32)]) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    for i in 0..n {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[(i + 1) % n];
        bresenham(grid, x0, y0, x1, y1);
    }
}

/// Map a unit-space point (scale ~1.0) to dot-space with center + scale.
#[inline]
fn to_dot(cx: f32, cy: f32, scale: f32, ux: f32, uy: f32, rot: f32) -> (i32, i32) {
    let rx = ux * rot.cos() - uy * rot.sin();
    let ry = ux * rot.sin() + uy * rot.cos();
    (
        (cx + rx * scale).round() as i32,
        (cy - ry * scale).round() as i32,
    )
}

// ────────────────────────────────────────────────────────────────────────────
// 1. Penrose P3 — rhombus tiling via Robinson-triangle deflation
// ────────────────────────────────────────────────────────────────────────────
//
// A Penrose P3 tiling decomposes into two Robinson triangles:
//   • "fat" rhombus → interior angle 72° (composed of two "acute" triangles)
//   • "thin" rhombus → interior angle 36° (composed of two "obtuse" triangles)
//
// Deflation rule (one step doubles the number of triangles):
//   Acute triangle  (A) with vertices P, Q, R:
//     split at S on PR where PS = 1/PHI · PR → A(P,S,Q)  + B(R,S,Q)
//   Obtuse triangle (B) with vertices P, Q, R:
//     split at S on QP where QS = 1/PHI · QP → A(R,S,P)  + B(R,Q,S)
//
// We seed with 10 acute triangles forming a "sun" and deflate up to 5 times,
// capping depth with `ctx.eased`.

struct PenroseP3;
impl ProgressStyle for PenroseP3 {
    fn name(&self) -> &str {
        "penrose-p3"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Penrose P3 rhombus tiling: Robinson-triangle deflation reveals fat (72°) \
         and thin (36°) rhombi generation by generation as progress rises; the \
         whole pattern rotates slowly with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);
        let rot = ctx.time * 0.25;

        // Depth from eased: 0 → depth 0, 1 → depth 4 (max 4 for cost safety).
        let depth = (ctx.eased * 4.0).floor() as usize;
        // Reveal fraction within the current depth level.
        let reveal_frac = (ctx.eased * 4.0).fract();

        // Seed: 10 acute Robinson triangles arranged in a "sun" ring.
        // Each triangle: type=Acute, vertices (p,q,r) in unit space.
        // Acute triangle: two short sides length 1, long side PHI.
        //   p = center, q & r on the circle at angles (k±36°)*π/180
        let mut tris: Vec<(bool, [f32; 2], [f32; 2], [f32; 2])> = Vec::new();
        for k in 0..10usize {
            let a1 = (k as f32 * 36.0) * PI / 180.0;
            let a2 = (k as f32 * 36.0 + 36.0) * PI / 180.0;
            let p = [0.0f32, 0.0];
            let q = [a1.cos(), a1.sin()];
            let r = [a2.cos(), a2.sin()];
            // Alternate acute/obtuse for perfect sun seeding.
            let is_acute = k % 2 == 0;
            tris.push((is_acute, p, q, r));
        }

        // Deflate `depth` times.
        for _step in 0..depth.min(4) {
            tris = deflate_p3(tris);
            if tris.len() > 3000 {
                break;
            } // cost cap
        }

        // Draw revealed triangles (edges only for aesthetic clarity).
        let n_total = tris.len();
        let n_draw = if depth < 4 {
            (reveal_frac * n_total as f32).round() as usize
        } else {
            n_total
        };

        for tri in tris.iter().take(n_draw) {
            let (_is_acute, p, q, r) = tri;
            let pd = to_dot(cx, cy, scale, p[0], p[1], rot);
            let qd = to_dot(cx, cy, scale, q[0], q[1], rot);
            let rd = to_dot(cx, cy, scale, r[0], r[1], rot);
            draw_poly(grid, &[pd, qd, rd]);
        }
        Ok(())
    }
}

/// One deflation step for P3 Robinson triangles.
/// is_acute=true → "acute" (fat-rhombus) triangle, false → "obtuse" (thin-rhombus).
fn deflate_p3(
    tris: Vec<(bool, [f32; 2], [f32; 2], [f32; 2])>,
) -> Vec<(bool, [f32; 2], [f32; 2], [f32; 2])> {
    let mut out = Vec::with_capacity(tris.len() * 2);
    for (is_acute, p, q, r) in tris {
        if is_acute {
            // Acute: P is apex. Split PQ at S where PS = 1/PHI * PQ.
            let s = lerp2(p, q, 1.0 / PHI);
            // Produce: acute(Q,S,R) + obtuse(P,S,R)   [classic Penrose deflation]
            out.push((true, q, s, r));
            out.push((false, p, s, r));
        } else {
            // Obtuse: R is apex opposite the long side. Split RP at S where RS = 1/PHI * RP.
            let s = lerp2(r, p, 1.0 / PHI);
            // Produce: obtuse(Q,S,P) + acute(Q,R,S)
            out.push((false, q, s, p));
            out.push((true, q, r, s));
        }
    }
    out
}

#[inline]
fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

// ────────────────────────────────────────────────────────────────────────────
// 2. Penrose P2 — kite & dart tiling via golden gnomon subdivision
// ────────────────────────────────────────────────────────────────────────────
//
// Kite: a quadrilateral with interior angles 72°,72°,72°,144°.
// Dart: a quadrilateral with interior angles 36°,36°,36°,252°.
//
// We represent each as a pair of "golden triangles":
//   Golden triangle (GT): isoceles, apex 36°, base angles 72° each.
//   Golden gnomon  (GG): isoceles, apex 108°, base angles 36° each.
//
// Deflation (each step roughly multiplies count by PHI²):
//   GT (apex P, base QR): split at S on PR s.t. PS=PQ/PHI → GT(P,S,Q) + GG(S,Q,R)
//   GG (apex P, base QR): split at S on QP s.t. QS=QR/PHI → GT(R,Q,S) + GG(P,R,S)

struct PenroseP2;
impl ProgressStyle for PenroseP2 {
    fn name(&self) -> &str {
        "penrose-p2"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Penrose P2 kite & dart tiling: golden-triangle / golden-gnomon subdivision \
         reveals the kite-and-dart mosaic generation by generation; time animates \
         a gentle shimmer across revealed tiles"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);
        let rot = ctx.time * 0.18;

        let depth = (ctx.eased * 4.0).floor() as usize;
        let reveal_frac = (ctx.eased * 4.0).fract();

        // Seed: 5 golden triangles forming a "star" at the origin.
        let mut tris: Vec<(bool, [f32; 2], [f32; 2], [f32; 2])> = Vec::new();
        for k in 0..5usize {
            let a_mid = (k as f32 * 72.0 + 90.0) * PI / 180.0;
            let a_lo = (k as f32 * 72.0 + 90.0 - 36.0) * PI / 180.0;
            let a_hi = (k as f32 * 72.0 + 90.0 + 36.0) * PI / 180.0;
            let p = [0.0f32, 0.0];
            let q = [a_lo.cos(), a_lo.sin()];
            let r = [a_hi.cos(), a_hi.sin()];
            let _ = a_mid;
            tris.push((true, p, q, r)); // golden triangle, apex at center
        }

        for _step in 0..depth.min(4) {
            tris = deflate_p2(tris);
            if tris.len() > 3000 {
                break;
            }
        }

        let n_total = tris.len();
        let n_draw = if depth < 4 {
            (reveal_frac * n_total as f32).round() as usize
        } else {
            n_total
        };

        for tri in tris.iter().take(n_draw) {
            let (_gt, p, q, r) = tri;
            let pd = to_dot(cx, cy, scale, p[0], p[1], rot);
            let qd = to_dot(cx, cy, scale, q[0], q[1], rot);
            let rd = to_dot(cx, cy, scale, r[0], r[1], rot);
            draw_poly(grid, &[pd, qd, rd]);
        }
        Ok(())
    }
}

fn deflate_p2(
    tris: Vec<(bool, [f32; 2], [f32; 2], [f32; 2])>,
) -> Vec<(bool, [f32; 2], [f32; 2], [f32; 2])> {
    let mut out = Vec::with_capacity(tris.len() * 2);
    for (is_gt, p, q, r) in tris {
        if is_gt {
            // Golden triangle: P is apex (36°), Q and R are base.
            // S on PR such that PS = PQ / PHI (both equal 1 in unit space, S divides at 1/PHI).
            let s = lerp2(p, r, 1.0 / PHI);
            out.push((true, p, q, s));
            out.push((false, s, q, r));
        } else {
            // Golden gnomon: P is apex (108°). S on QP such that QS = QR / PHI.
            let s = lerp2(q, p, 1.0 / PHI);
            out.push((true, r, q, s));
            out.push((false, p, r, s));
        }
    }
    out
}

// ────────────────────────────────────────────────────────────────────────────
// 3. Sun Pattern — 5 kites around a center, expanding rings
// ────────────────────────────────────────────────────────────────────────────
//
// The canonical Penrose "sun" seed: five kites sharing a vertex at the origin.
// Progress reveals rings of tiles growing outward from the center.

struct SunPattern;
impl ProgressStyle for SunPattern {
    fn name(&self) -> &str {
        "sun-pattern"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Penrose sun: five kites sharing the central vertex, then the surrounding \
         dart ring, then outer kite rings — each concentric generation appears as \
         progress rises, pulsing gently with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);
        let rot = ctx.time * 0.15;

        // Kite: apex at center (36° angle), two arms of length 1/PHI, two arms of length 1.
        // Vertices in unit space (apex at 0,0, tip at distance 1 along bisector):
        //   center(0,0), left-wing, outer-tip, right-wing
        // For kite k: bisector angle = k*72°
        //   outer_tip  = (cos(a), sin(a))         distance 1
        //   left_wing  = (cos(a+36°), sin(a+36°)) * (1/PHI)
        //   right_wing = (cos(a-36°), sin(a-36°)) * (1/PHI)

        // Number of concentric "rings" to draw (1-3 based on eased).
        let rings = ((ctx.eased * 3.0) as usize).max(1).min(3);

        for ring in 0..rings {
            let ring_scale = scale / (1.0 + ring as f32 * 0.6);
            // Pulse: inner ring blinks faster.
            let pulse = (ctx.time * (1.0 + ring as f32 * 0.5)).sin() * 0.5 + 0.5;
            if ring > 0 && pulse < 0.3 {
                continue;
            }

            for k in 0..5usize {
                let a = k as f32 * 72.0 * PI / 180.0 + rot + ring as f32 * 36.0 * PI / 180.0;
                let tip = [a.cos(), a.sin()];
                let lw = [(a + PI / 5.0).cos() / PHI, (a + PI / 5.0).sin() / PHI];
                let rw = [(a - PI / 5.0).cos() / PHI, (a - PI / 5.0).sin() / PHI];
                let ctr = [0.0f32, 0.0];

                let pd = to_dot(cx, cy, ring_scale, ctr[0], ctr[1], 0.0);
                let qd = to_dot(cx, cy, ring_scale, lw[0], lw[1], 0.0);
                let rd = to_dot(cx, cy, ring_scale, tip[0], tip[1], 0.0);
                let sd = to_dot(cx, cy, ring_scale, rw[0], rw[1], 0.0);
                draw_poly(grid, &[pd, qd, rd, sd]);
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 4. Girih Tiles — Islamic decagon strapwork
// ────────────────────────────────────────────────────────────────────────────
//
// Girih tiles are five polygons (decagon, pentagon, hexagon, bowtie, rhombus)
// whose edges all have the same length and whose angles are multiples of 36°.
// We draw a central decagon, then surround it with pentagons and bowties,
// revealing the strapwork interior lines as progress rises.

struct GirihTiles;
impl ProgressStyle for GirihTiles {
    fn name(&self) -> &str {
        "girih-tiles"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Islamic girih tile strapwork: a central 10-gon surrounded by pentagons \
         and bowties; interior strap lines reveal with progress, the whole pattern \
         rotating with time like a medieval mosque decoration"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh) * 0.45;
        let rot = ctx.time * 0.12;

        // Central decagon.
        let dec = regular_ngon(10, cx, cy, scale, rot);
        draw_poly(grid, &dec);

        // Interior strapwork: connect every other vertex of the decagon.
        let n_straps = (ctx.eased * 10.0).round() as usize;
        for i in 0..n_straps.min(10) {
            let j = (i + 2) % 10;
            bresenham(grid, dec[i].0, dec[i].1, dec[j].0, dec[j].1);
        }

        // Surrounding 10 pentagons at each edge midpoint.
        let n_pent = (ctx.eased * 10.0 * 2.0 - 10.0).round() as usize; // second half of progress
        for k in 0..n_pent.min(10) {
            let a = k as f32 * 36.0 * PI / 180.0 + rot + 18.0 * PI / 180.0;
            let dist = scale * (1.0 + 1.0 / (2.0 * (PI / 10.0).tan()));
            let pcx = cx + dist * a.cos();
            let pcy = cy - dist * a.sin();
            let side = scale * 2.0 * (PI / 10.0).sin();
            let pent = regular_ngon(5, pcx, pcy, side * 0.5, rot + a);
            draw_poly(grid, &pent);
        }
        Ok(())
    }
}

/// Compute vertices of a regular n-gon centered at (cx,cy) in dot-space.
fn regular_ngon(n: usize, cx: f32, cy: f32, radius: f32, offset: f32) -> Vec<(i32, i32)> {
    (0..n)
        .map(|k| {
            let a = 2.0 * PI * k as f32 / n as f32 + offset;
            (
                (cx + radius * a.cos()).round() as i32,
                (cy - radius * a.sin()).round() as i32,
            )
        })
        .collect()
}

// ────────────────────────────────────────────────────────────────────────────
// 5. Ammann Bars — the hidden quasiperiodic grid on a P3 rhombus tiling
// ────────────────────────────────────────────────────────────────────────────
//
// Every Penrose P3 tiling carries an "Ammann bar" decoration: each rhombus
// gets one stripe across it, and the stripes form 5 families of parallel lines
// at angles 0°, 72°, 144°, 216°, 288°.  The spacings within each family are
// quasiperiodic (long L and short S with L/S = PHI).
//
// We approximate this by drawing 5 families of parallel quasiperiodic lines.
// Each family k has angle k*36° and lines at positions …, 0, L, L+S, 2L+S, 2L+2S, …
// where L = PHI and S = 1.

struct AmmannBars;
impl ProgressStyle for AmmannBars {
    fn name(&self) -> &str {
        "ammann-bars"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Ammann bars: the five quasiperiodic stripe families that decorate every \
         Penrose P3 rhombus tiling, with long-L and short-S spacings in ratio PHI; \
         families reveal one by one as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);
        let rot = ctx.time * 0.08;

        // How many of the 5 families to draw (reveal with eased).
        let n_families = (ctx.eased * 5.0).ceil() as usize;
        let family_frac = (ctx.eased * 5.0).fract(); // partial last family

        for fam in 0..n_families.min(5) {
            let angle = fam as f32 * 36.0 * PI / 180.0 + rot;
            // Direction perpendicular to the stripe family.
            let perp_x = angle.cos();
            let perp_y = -angle.sin();
            // Line direction.
            let line_x = -angle.sin();
            let line_y = -angle.cos();

            // Generate quasiperiodic offsets: L, S, L, L, S, L, S, …
            // The Fibonacci word gives the sequence of L/S.
            let max_lines = 20usize;
            let mut offsets: Vec<f32> = Vec::with_capacity(max_lines * 2 + 1);
            offsets.push(0.0);
            let mut pos = 0.0f32;
            let l_step = scale * PHI / (PHI + 1.0);
            let s_step = scale * 1.0 / (PHI + 1.0);
            let mut fib_a = 1usize;
            let mut fib_b = 1usize;
            for _i in 0..max_lines {
                // Use the Fibonacci word: if fib ratio decides L or S.
                let use_long = fib_a > fib_b;
                let step = if use_long { l_step } else { s_step };
                // Update Fibonacci-like counter (Beatty sequence approximation).
                let old_a = fib_a;
                fib_a = fib_a + fib_b;
                fib_b = old_a;
                let fib_a_c = fib_a;
                let fib_b_c = fib_b;
                let _ = (fib_a_c, fib_b_c);
                pos += step;
                offsets.push(pos);
                offsets.push(-pos);
            }

            // Last family: only draw partial set.
            let n_lines = if fam + 1 == n_families {
                (family_frac * offsets.len() as f32) as usize
            } else {
                offsets.len()
            };

            for &off in offsets.iter().take(n_lines) {
                // Line at distance `off` from center, in direction `line_*`.
                let base_x = cx + perp_x * off;
                let base_y = cy + perp_y * off;
                let half = scale * 1.5;
                let x0 = (base_x + line_x * half).round() as i32;
                let y0 = (base_y + line_y * half).round() as i32;
                let x1 = (base_x - line_x * half).round() as i32;
                let y1 = (base_y - line_y * half).round() as i32;
                bresenham(grid, x0, y0, x1, y1);
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 6. De Bruijn Pentagrid — 5 families of parallel lines → dual Penrose tiling
// ────────────────────────────────────────────────────────────────────────────
//
// de Bruijn (1981): a Penrose P3 tiling is dual to a pentagrid — 5 families of
// equidistant parallel lines at angles 72°·k, each with offset γ_k ≈ arbitrary.
//
// We draw the five families, then animate the intersection points to reveal
// the dual rhombus vertices.  The dual step is: each intersection of family j
// and family k at crossing index (m,n) maps to a rhombus vertex.
//
// For progress we simply reveal the five line families one at a time, and
// overlay the computed dual vertices with `ctx.eased`.

struct DeBruijnPentagrid;
impl ProgressStyle for DeBruijnPentagrid {
    fn name(&self) -> &str {
        "debruijn-pentagrid"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "de Bruijn pentagrid: five families of quasiperiodic parallel lines at 72° \
         intervals whose dual gives a Penrose tiling; the grid weaves in with \
         progress and the dual rhombus vertices twinkle with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);
        let rot = ctx.time * 0.07;

        // Five-direction angles.
        let gammas: [f32; 5] = [0.1, 0.2, -0.15, 0.05, -0.08]; // irrational offsets
        let n_families = (ctx.eased * 5.0).ceil() as usize;

        for fam in 0..n_families.min(5) {
            let angle = fam as f32 * 72.0 * PI / 180.0 + rot;
            let perp_x = angle.cos();
            let perp_y = -angle.sin();
            let line_x = -angle.sin();
            let line_y = -angle.cos();
            let gamma = gammas[fam];

            // Draw ~9 parallel lines (4 on each side of center).
            let lines = 9i32;
            for m in -lines / 2..=lines / 2 {
                let off = (m as f32 + gamma) * scale * 0.4;
                let base_x = cx + perp_x * off;
                let base_y = cy + perp_y * off;
                let half = scale * 1.5;
                let x0 = (base_x + line_x * half).round() as i32;
                let y0 = (base_y + line_y * half).round() as i32;
                let x1 = (base_x - line_x * half).round() as i32;
                let y1 = (base_y - line_y * half).round() as i32;
                bresenham(grid, x0, y0, x1, y1);
            }
        }

        // Dual rhombus vertices: intersections of each pair of families.
        // Only draw if eased > 0.7.
        if ctx.eased > 0.7 {
            let dual_frac = (ctx.eased - 0.7) / 0.3;
            let step = scale * 0.4;
            let mut pts: Vec<(i32, i32)> = Vec::new();
            for fj in 0..5usize {
                for fk in (fj + 1)..5usize {
                    let aj = fj as f32 * 72.0 * PI / 180.0 + rot;
                    let ak = fk as f32 * 72.0 * PI / 180.0 + rot;
                    for mj in -4i32..=4 {
                        for mk in -4i32..=4 {
                            // Intersection of line mj in family j and mk in family k.
                            let bj_x = (mj as f32 + gammas[fj]) * step * aj.cos();
                            let bj_y = (mj as f32 + gammas[fj]) * step * (-aj.sin());
                            let bk_x = (mk as f32 + gammas[fk]) * step * ak.cos();
                            let bk_y = (mk as f32 + gammas[fk]) * step * (-ak.sin());
                            let dj_x = -aj.sin();
                            let dj_y = -aj.cos();
                            let dk_x = -ak.sin();
                            let dk_y = -ak.cos();
                            // Solve: (bj + t*dj) = (bk + s*dk)
                            let det = dj_x * dk_y - dj_y * dk_x;
                            if det.abs() < 1e-6 {
                                continue;
                            }
                            let dx = bk_x - bj_x;
                            let dy = bk_y - bj_y;
                            let t = (dx * dk_y - dy * dk_x) / det;
                            let ix = cx + bj_x + t * dj_x;
                            let iy = cy + bj_y + t * dj_y;
                            pts.push((ix.round() as i32, iy.round() as i32));
                        }
                    }
                }
            }
            let n_pts = (dual_frac * pts.len() as f32).round() as usize;
            for &(px, py) in pts.iter().take(n_pts) {
                // Draw a tiny cross at each dual vertex.
                draw::dot_i(grid, px, py);
                draw::dot_i(grid, px + 1, py);
                draw::dot_i(grid, px - 1, py);
                draw::dot_i(grid, px, py + 1);
                draw::dot_i(grid, px, py - 1);
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 7. Decagon Fractal — recursive decagon infill
// ────────────────────────────────────────────────────────────────────────────
//
// A decagon subdivides into 10 smaller decagons + connecting pentagons (roughly).
// We approximate with: one large decagon, then ring of 10 smaller at radius
// 1+sin(18°) * sub_r, then sub-sub-decagons, up to depth 3.
// Depth capped at 3 for cost safety.

struct DecagonFractal;
impl ProgressStyle for DecagonFractal {
    fn name(&self) -> &str {
        "decagon-fractal"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Decagon fractal: a central 10-gon filled with rings of smaller 10-gons \
         at each recursion level, forming a self-similar quasicrystalline snowflake \
         that unfolds as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh) * 0.9;
        let rot = ctx.time * 0.1;

        let depth = (ctx.eased * 3.0).floor() as usize;
        let reveal_frac = (ctx.eased * 3.0).fract();

        // Seed: (cx, cy, radius, phase_offset)
        let mut decagons: Vec<(f32, f32, f32, f32)> = vec![(cx, cy, scale, rot)];

        for _d in 0..depth.min(3) {
            let mut next = Vec::new();
            for &(dx, dy, r, phase) in &decagons {
                // Child decagons: 10 of them at distance r + sub_r.
                let sub_r = r / (1.0 + PHI);
                let dist = r - sub_r + sub_r * 0.1; // slight overlap
                for k in 0..10usize {
                    let a = k as f32 * 36.0 * PI / 180.0 + phase;
                    let nx = dx + dist * a.cos();
                    let ny = dy - dist * a.sin();
                    next.push((nx, ny, sub_r, phase + a));
                }
            }
            // Draw the current level's decagons.
            for &(dx, dy, r, phase) in &decagons {
                let pts = regular_ngon(10, dx, dy, r, phase);
                draw_poly(grid, &pts);
            }
            decagons = next;
            if decagons.len() > 500 {
                break;
            }
        }

        // Partial reveal of the last depth ring.
        let n_draw = (reveal_frac * decagons.len() as f32).round() as usize;
        for &(dx, dy, r, phase) in decagons.iter().take(n_draw) {
            let pts = regular_ngon(10, dx, dy, r, phase);
            draw_poly(grid, &pts);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 8. Quasicrystal Diffraction — 5 plane waves at 72° → 10-fold interference
// ────────────────────────────────────────────────────────────────────────────
//
// The quasicrystal interference pattern is computed as:
//   f(x,y) = Σ_{k=0}^{4} cos(2π · (x·cos(k·72°) + y·sin(k·72°)) · freq)
// Normalise to [0,1] and threshold at eased to reveal the bright regions.
// Braille dot space is sampled directly — very fast per dot.

struct QuasicrystalDiffraction;
impl ProgressStyle for QuasicrystalDiffraction {
    fn name(&self) -> &str {
        "quasicrystal-diffraction"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Quasicrystal interference: sum of 5 plane waves at 72° intervals produces \
         a 10-fold diffraction pattern; a moving threshold cuts through the field as \
         progress rises, the whole pattern rotating slowly with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh).max(1.0);

        // Spatial frequency: higher → more rings visible.
        let freq = 3.0 / scale;
        // Phase drift with time — the whole pattern rotates.
        let phase = ctx.time * 0.4;

        // Threshold: eased controls which portion of intensity is lit.
        // f ranges from -5 to 5; threshold sweeps 5 → -5 as eased goes 0→1.
        let threshold = 5.0 - ctx.eased * 10.0;

        for dy in 0..dh {
            for dx in 0..dw {
                let ux = (dx as f32 - cx) / scale;
                let uy = (dy as f32 - cy) / scale;
                // Sum of 5 plane waves.
                let mut f = 0.0f32;
                for k in 0..5usize {
                    let angle = k as f32 * 72.0 * PI / 180.0 + phase;
                    f += (2.0 * PI * freq * (ux * angle.cos() + uy * angle.sin()) * scale).cos();
                }
                if f > threshold {
                    draw::dot(grid, dx, dy);
                }
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 9. Pinwheel Tiling — 1:2 right-triangle substitution
// ────────────────────────────────────────────────────────────────────────────
//
// The pinwheel tiling (Conway & Radin 1994) is based on a right triangle
// with legs 1 and 2 and hypotenuse √5.  One triangle substitutes into 5,
// and with each generation the triangles appear at every possible rotation
// (dense in SO(2)), making it rotationally aperiodic.
//
// Deflation: a right triangle (legs a=1, b=2) splits into 5 smaller copies.
// Vertices: P (right angle), Q (on short side), R (far end of long side).
// We cap at depth 4 and cost 2000 triangles.

struct PinwheelTiling;
impl ProgressStyle for PinwheelTiling {
    fn name(&self) -> &str {
        "pinwheel-tiling"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Pinwheel tiling: a 1:2 right triangle substitutes into 5 smaller copies \
         at irrational rotations, filling the plane with triangles at every angle; \
         deflation generations bloom outward as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh) * 0.8;
        let rot = ctx.time * 0.12;

        let depth = (ctx.eased * 3.0).floor() as usize;
        let reveal_frac = (ctx.eased * 3.0).fract();

        // Seed: one 1:2 right triangle in unit space.
        // P=right angle=(0,0), Q=(1,0), R=(0,2), normalised to scale.
        let seed_scale = 1.0 / 5.0f32.sqrt(); // so hypotenuse=1 in unit space
        type Tri = [[f32; 2]; 3]; // [P, Q, R]

        let seed: Tri = [[0.0, 0.0], [seed_scale, 0.0], [0.0, seed_scale * 2.0]];
        let mut tris: Vec<Tri> = vec![seed];

        for _step in 0..depth.min(3) {
            tris = pinwheel_deflate(tris);
            if tris.len() > 2000 {
                break;
            }
        }

        let n_total = tris.len();
        let n_draw = if depth < 3 {
            (reveal_frac * n_total as f32).round() as usize
        } else {
            n_total
        };

        for tri in tris.iter().take(n_draw) {
            let pts: Vec<(i32, i32)> = tri
                .iter()
                .map(|v| to_dot(cx, cy, scale, v[0], v[1], rot))
                .collect();
            draw_poly(grid, &pts);
        }
        Ok(())
    }
}

type Tri2 = [[f32; 2]; 3];

fn pinwheel_deflate(tris: Vec<Tri2>) -> Vec<Tri2> {
    let mut out = Vec::with_capacity(tris.len() * 5);
    for tri in tris {
        let [p, q, r] = tri;
        // Pinwheel substitution: 5 children.
        // Standard pinwheel: right angle at P, short leg PQ (len 1), long leg PR (len 2).
        // Sub-triangle hypotenuse = 1/√5 of parent hypotenuse.
        //
        // Key points (per Conway-Radin):
        //   M  = midpoint(P, R)           (midpoint of long leg)
        //   A  = P + (1/5)*(Q-P)          (1/5 along PQ from P)
        //   B  = P + (2/5)*(Q-P)          (2/5 along PQ from P)
        //   C  = midpoint(Q, R)
        //   D  = M + (1/2)*(Q - M)        ~ midpoint(M, Q)
        //
        // 5 children (approximate but structurally correct):
        let m = lerp2(p, r, 0.5);
        let a = lerp2(p, q, 0.2);
        let b = lerp2(p, q, 0.4);
        let c = lerp2(q, r, 0.5);
        let d = lerp2(m, q, 0.5);
        out.push([p, a, m]);
        out.push([a, b, d]);
        out.push([b, q, c]);
        out.push([d, c, m]);
        out.push([m, c, r]);
    }
    out
}

// ────────────────────────────────────────────────────────────────────────────
// 10. Truchet-Quasi — Truchet arcs on a quasiperiodic rhombus lattice
// ────────────────────────────────────────────────────────────────────────────
//
// Classic Truchet tiles place quarter-circle arcs in squares, two orientations.
// We place arcs inside the rhombi of a Penrose-like rhombus grid (fat and thin),
// with orientation chosen by a deterministic pseudo-random rule seeded from
// the rhombus index so the pattern never repeats.
//
// Since we don't have a full deflation here, we generate a flat grid of
// rhombi using a pentagrid projection approach (approximate) and draw arcs
// inside each.

struct TruchetQuasi;
impl ProgressStyle for TruchetQuasi {
    fn name(&self) -> &str {
        "truchet-quasi"
    }
    fn theme(&self) -> &str {
        "penrose"
    }
    fn describe(&self) -> &str {
        "Truchet-quasi: quarter-circle arcs placed in the fat and thin rhombi of a \
         Penrose-like quasiperiodic lattice; orientation is deterministically varied \
         so the arc flow never repeats, revealed tile by tile with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);
        let rot = ctx.time * 0.09;

        // Generate rhombi from a simple pentagrid projection.
        // For a pentagrid with 5 families at 72°·k:
        //   Each pair of family lines (j,k) intersects, dual vertex = rhombus.
        //   Rhombus type = |j - k| mod 5: if 1 or 4 → fat, if 2 or 3 → thin.
        let spacing = scale * 0.35;
        let mut rhombi: Vec<(f32, f32, f32, bool, usize)> = Vec::new(); // (cx, cy, angle, is_fat, idx)

        let n_lines = 5i32;
        for fj in 0..5usize {
            for fk in (fj + 1)..5usize {
                let aj = fj as f32 * 72.0 * PI / 180.0 + rot;
                let ak = fk as f32 * 72.0 * PI / 180.0 + rot;
                for mj in -n_lines..=n_lines {
                    for mk in -n_lines..=n_lines {
                        let bj_x = mj as f32 * spacing * aj.cos();
                        let bj_y = mj as f32 * spacing * (-aj.sin());
                        let bk_x = mk as f32 * spacing * ak.cos();
                        let bk_y = mk as f32 * spacing * (-ak.sin());
                        let dj_x = -aj.sin();
                        let dj_y = -aj.cos();
                        let dk_x = -ak.sin();
                        let dk_y = -ak.cos();
                        let det = dj_x * dk_y - dj_y * dk_x;
                        if det.abs() < 1e-6 {
                            continue;
                        }
                        let dx = bk_x - bj_x;
                        let dy = bk_y - bj_y;
                        let t = (dx * dk_y - dy * dk_x) / det;
                        let rx = cx + bj_x + t * dj_x;
                        let ry = cy + bj_y + t * dj_y;
                        // Inside grid?
                        if rx < 0.0 || ry < 0.0 || rx >= dw as f32 || ry >= dh as f32 {
                            continue;
                        }
                        let diff = (fk - fj) % 5;
                        let is_fat = diff == 1 || diff == 4;
                        let idx = (mj.unsigned_abs() as usize * 7
                            + mk.unsigned_abs() as usize * 13
                            + fj * 3
                            + fk * 5)
                            & 1; // 0 or 1
                        rhombi.push((rx, ry, aj, is_fat, idx));
                    }
                }
            }
        }

        let n_total = rhombi.len();
        let n_draw = (ctx.eased * n_total as f32).round() as usize;

        for &(rx, ry, angle, _is_fat, flip) in rhombi.iter().take(n_draw) {
            // Draw a small quarter-arc inside the rhombus.
            // Arc radius = spacing * 0.4.
            let r = spacing * 0.4;
            let arc_steps = 8usize;
            // Choose two corners based on flip.
            let corner_a = angle + if flip == 0 { 0.0 } else { PI };
            let corner_b = corner_a + PI * 0.5;
            // Arc from corner_a to corner_b.
            let arc_cx = rx + r * corner_a.cos();
            let arc_cy = ry - r * corner_a.sin();
            for s in 0..=arc_steps {
                let a = corner_b + (corner_a - corner_b) * s as f32 / arc_steps as f32;
                let px = (arc_cx + r * a.cos()).round() as i32;
                let py = (arc_cy - r * a.sin()).round() as i32;
                draw::dot_i(grid, px, py);
            }
            // Second arc from the opposite corner.
            let corner_c = corner_a + PI;
            let corner_d = corner_c + PI * 0.5;
            let arc_cx2 = rx + r * corner_c.cos();
            let arc_cy2 = ry - r * corner_c.sin();
            for s in 0..=arc_steps {
                let a = corner_d + (corner_c - corner_d) * s as f32 / arc_steps as f32;
                let px = (arc_cx2 + r * a.cos()).round() as i32;
                let py = (arc_cy2 - r * a.sin()).round() as i32;
                draw::dot_i(grid, px, py);
            }
        }
        Ok(())
    }
}
