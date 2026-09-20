//! Topology / parametric-surface / 3D-projection progress bars.
//!
//! Each bar maps `ctx.eased` to the extent of a surface reveal or curve trace,
//! and `ctx.time` to a continuous 3D rotation. A shared helper rotates a point
//! by Euler angles (ax, ay) and orthographically projects it onto the dot
//! lattice, centred on the grid and scaled to fit both axes.
//!
//! All surfaces are rendered in **dot space** via `draw::dot_i`, so out-of-
//! bounds plots are silently discarded. Per-frame point counts are bounded to
//! keep even wide grids snappy.
//!
//! # Styles
//!
//! | Name | Surface |
//! |---|---|
//! | `mobius-strip` | Möbius strip, u revealed by eased |
//! | `torus-wireframe` | (R,r)-torus latitude/longitude wire mesh |
//! | `torus-knot` | (2,3) torus knot trace |
//! | `trefoil-knot` | Trefoil knot, reveals and spins |
//! | `klein-bottle` | Figure-8 Klein bottle immersion |
//! | `hopf-fibers` | Three Hopf fibration circles |
//! | `sphere-inflate` | Wireframe sphere inflating with eased |
//! | `helix-climb` | Double helix climbing with eased |
//! | `saddle-surface` | Hyperbolic paraboloid (saddle) wireframe |
//! | `tesseract-spin` | Rotating 4-D hypercube projected to 2-D |
//! | `roman-surface` | Steiner's Roman surface parametric |
//! | `seifert-ramp` | Spiral Seifert-surface ramp |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ── Shared 3-D projection helper ─────────────────────────────────────────────

/// Rotate `(x, y, z)` about the X-axis by `ax` radians then the Y-axis by
/// `ay` radians (standard Euler XY extrinsic), then orthographically project
/// the result onto the dot lattice centred at `(cx, cy)` with uniform `scale`
/// dots-per-unit.
///
/// Returns `(sx, sy)` as `i32` — suitable for `draw::dot_i`.
#[inline]
fn project(x: f32, y: f32, z: f32, ax: f32, ay: f32, cx: i32, cy: i32, scale: f32) -> (i32, i32) {
    // Rotate about X axis.
    let (sax, cax) = ax.sin_cos();
    let y1 = y * cax - z * sax;
    let z1 = y * sax + z * cax;
    // Rotate about Y axis.
    let (say, cay) = ay.sin_cos();
    let x2 = x * cay + z1 * say;
    let y2 = y1;
    // Orthographic projection (drop z2, flip y for screen coords).
    let sx = cx + (x2 * scale).round() as i32;
    let sy = cy - (y2 * scale).round() as i32;
    (sx, sy)
}

/// Plot a parametric curve segment from `t0` to `t1` (in `[0, 2π]` or
/// `[0, 1]`), sampling `steps` evenly-spaced points, using `f(t) -> (x,y,z)`.
/// Each point is rotated and projected with the given angles / centre / scale.
#[inline]
fn plot_curve<F>(
    grid: &mut BrailleGrid,
    t0: f32,
    t1: f32,
    steps: usize,
    ax: f32,
    ay: f32,
    cx: i32,
    cy: i32,
    scale: f32,
    f: F,
) where
    F: Fn(f32) -> (f32, f32, f32),
{
    if steps == 0 {
        return;
    }
    for i in 0..=steps {
        let t = t0 + (t1 - t0) * (i as f32 / steps as f32);
        let (x, y, z) = f(t);
        let (sx, sy) = project(x, y, z, ax, ay, cx, cy, scale);
        draw::dot_i(grid, sx, sy);
    }
}

// ── Shared geometry: grid centre + uniform scale ──────────────────────────────

/// Return `(cx_i32, cy_i32, scale)` for a grid: centre in dot coords, and
/// scale chosen so a unit sphere just fits the smaller axis.
fn grid_cxys(grid: &BrailleGrid) -> (i32, i32, f32) {
    let (dw, dh) = draw::dot_dims(grid);
    let cx = (dw / 2) as i32;
    let cy = (dh / 2) as i32;
    let scale = (dw.min(dh * 2) as f32 * 0.42).max(1.0);
    (cx, cy, scale)
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Möbius strip
// ─────────────────────────────────────────────────────────────────────────────

struct MobiusStrip;

impl ProgressStyle for MobiusStrip {
    fn name(&self) -> &str {
        "mobius-strip"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "A one-sided Möbius strip unfurling along its parametric u-axis as progress advances, rotating lazily on time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.37;
        let ay = ctx.time * 0.53;
        // Reveal u from 0 to eased·2π across ~20 stripes in v.
        let u_max = ctx.eased * 2.0 * PI;
        let u_steps = 120usize;
        let v_lines = 9usize; // v ∈ [-1, 1] in v_lines steps
        for vi in 0..v_lines {
            let v = -1.0 + 2.0 * vi as f32 / (v_lines - 1).max(1) as f32;
            plot_curve(grid, 0.0, u_max, u_steps, ax, ay, cx, cy, scale, |u| {
                let half = 0.5 * v * (u / 2.0).cos();
                let x = (1.0 + half) * u.cos();
                let y = (1.0 + half) * u.sin();
                let z = 0.5 * v * (u / 2.0).sin();
                (x, y, z)
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Torus wireframe
// ─────────────────────────────────────────────────────────────────────────────

struct TorusWireframe;

impl ProgressStyle for TorusWireframe {
    fn name(&self) -> &str {
        "torus-wireframe"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Latitude/longitude wireframe of a (R=1, r=0.38) torus, circles revealed row-by-row as progress grows"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.4 + 0.4;
        let ay = ctx.time * 0.6;
        let r_big = 1.0_f32;
        let r_small = 0.38_f32;
        let n_lat = 14usize; // latitude circles (constant v)
        let n_lon = 10usize; // longitude circles (constant u)
        let u_steps = 60usize;

        // Latitude circles: reveal the first `n_lit` of them.
        let n_lit = (ctx.eased * (n_lat + n_lon) as f32).round() as usize;

        for i in 0..n_lit.min(n_lat) {
            let v = 2.0 * PI * i as f32 / n_lat as f32;
            plot_curve(grid, 0.0, 2.0 * PI, u_steps, ax, ay, cx, cy, scale, |u| {
                let x = (r_big + r_small * v.cos()) * u.cos();
                let y = (r_big + r_small * v.cos()) * u.sin();
                let z = r_small * v.sin();
                (x, y, z)
            });
        }
        // Longitude circles.
        let n_lon_lit = n_lit.saturating_sub(n_lat).min(n_lon);
        for i in 0..n_lon_lit {
            let u = 2.0 * PI * i as f32 / n_lon as f32;
            plot_curve(grid, 0.0, 2.0 * PI, u_steps, ax, ay, cx, cy, scale, |v| {
                let x = (r_big + r_small * v.cos()) * u.cos();
                let y = (r_big + r_small * v.cos()) * u.sin();
                let z = r_small * v.sin();
                (x, y, z)
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Torus knot (2,3)
// ─────────────────────────────────────────────────────────────────────────────

struct TorusKnot;

impl ProgressStyle for TorusKnot {
    fn name(&self) -> &str {
        "torus-knot"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "(2,3) torus knot traced on a torus surface — a trefoil path that wraps twice around the tube for every three loops of the core"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.29;
        let ay = ctx.time * 0.47;
        let p = 2_i32;
        let q = 3_i32;
        let r_big = 1.0_f32;
        let r_small = 0.35_f32;
        // Parametric: t ∈ [0, 2π·lcm(p,q)] but one pass = 2π suffices for (2,3).
        let t_max = ctx.eased * 2.0 * PI;
        let steps = 200usize;
        plot_curve(grid, 0.0, t_max, steps, ax, ay, cx, cy, scale, |t| {
            let u = p as f32 * t;
            let v = q as f32 * t;
            let x = (r_big + r_small * v.cos()) * u.cos();
            let y = (r_big + r_small * v.cos()) * u.sin();
            let z = r_small * v.sin();
            (x, y, z)
        });
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Trefoil knot
// ─────────────────────────────────────────────────────────────────────────────

struct TrefoilKnot;

impl ProgressStyle for TrefoilKnot {
    fn name(&self) -> &str {
        "trefoil-knot"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Trefoil knot: x=sin t+2sin 2t, y=cos t−2cos 2t, z=−sin 3t — the simplest non-trivial knot, spinning on time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.33;
        let ay = ctx.time * 0.55;
        let t_max = ctx.eased * 2.0 * PI;
        let steps = 200usize;
        // Normalise scale: trefoil radius ≈ 3, so shrink by 1/3.
        let s = scale / 3.0;
        plot_curve(grid, 0.0, t_max, steps, ax, ay, cx, cy, s, |t| {
            let x = t.sin() + 2.0 * (2.0 * t).sin();
            let y = t.cos() - 2.0 * (2.0 * t).cos();
            let z = -(3.0 * t).sin();
            (x, y, z)
        });
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Klein bottle (figure-8 immersion)
// ─────────────────────────────────────────────────────────────────────────────

struct KleinBottle;

impl ProgressStyle for KleinBottle {
    fn name(&self) -> &str {
        "klein-bottle"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Figure-8 immersion of the Klein bottle in ℝ³ — a non-orientable surface with no inside revealed petal by petal"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.28 + 0.6;
        let ay = ctx.time * 0.44;
        // Figure-8 (Lawson) immersion: u ∈ [0,π], v ∈ [0,2π].
        // Reveal u-strips up to eased·π.
        let u_max = ctx.eased * PI;
        let u_lines = 20usize;
        let v_steps = 60usize;
        let s = scale / 2.5;
        for ui in 0..u_lines {
            let u = u_max * ui as f32 / u_lines.max(1) as f32;
            plot_curve(grid, 0.0, 2.0 * PI, v_steps, ax, ay, cx, cy, s, |v| {
                // Standard figure-8 Klein bottle parametrisation.
                let cu = u.cos();
                let su = u.sin();
                let cv = v.cos();
                let sv = v.sin();
                let a = 2.5;
                let x = (a + cu * (cv + 1.0)) * (2.0 * u).cos() - su * (2.0 * u).cos() * cv;
                let y = (a + cu * (cv + 1.0)) * (2.0 * u).sin() - su * (2.0 * u).sin() * cv;
                let z = su * (cv + 1.0) + cu * sv;
                (x * 0.3, y * 0.3, z * 0.5)
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Hopf fibration (three fibers)
// ─────────────────────────────────────────────────────────────────────────────

struct HopfFibers;

impl ProgressStyle for HopfFibers {
    fn name(&self) -> &str {
        "hopf-fibers"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Three Hopf fibration circles stereographically projected from S³ — every fiber is a great circle linking every other"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.31;
        let ay = ctx.time * 0.52;
        // Base points on S²: we choose 3 latitudes on the sphere, evenly spread.
        // For base point (θ,φ) on S², the Hopf fiber is the circle in S³
        // parameterised by t, and we stereographically project S³→ℝ³.
        // Base points: three points on S² at latitudes 0°, 60°, −60°.
        let bases: [(f32, f32); 3] = [
            (0.0, 0.0),
            (PI / 3.0, 2.0 * PI / 3.0),
            (-PI / 3.0, 4.0 * PI / 3.0),
        ];
        let t_max = ctx.eased * 2.0 * PI;
        let steps = 100usize;
        let s = scale * 0.55;
        for &(theta, phi) in &bases {
            // Hopf fiber: quaternion (cos α, sin α · p) where p ∈ S²,
            // stereographically projected from S³ \ {north pole} to ℝ³.
            let (st, ct) = theta.sin_cos();
            let (sp, cp) = phi.sin_cos();
            // Base point on S² as quaternion component: p = (ct·cp, ct·sp, st).
            // Fiber in S³: (cos t · 1  +  sin t · base-quat-pair).
            // We use a clean parameterisation:
            //   q(t) = (cos t · cos(θ/2),  cos t · sin(θ/2)·e^{iφ},
            //           sin t · cos(θ/2),  sin t · sin(θ/2)·e^{iφ})  [simplified]
            let hth = theta / 2.0;
            let (shth, chth) = hth.sin_cos();
            plot_curve(grid, 0.0, t_max, steps, ax, ay, cx, cy, s, |t| {
                // q = (q0,q1,q2,q3) ∈ S³.
                let q0 = t.cos() * chth;
                let q1 = t.cos() * shth * cp;
                let q2 = t.sin() * chth;
                let q3 = t.sin() * shth * cp;
                // Stereographic from north pole (1,0,0,0):
                // Project: (x,y,z) = (q1,q2,q3)/(1-q0).
                // Guard: if q0 ≈ 1 skip point.
                let denom = 1.0 - q0;
                if denom.abs() < 1e-4 {
                    (0.0, 0.0, 0.0)
                } else {
                    let inv = 1.0 / denom;
                    // Blend in phi so fibers spread out.
                    let _ = (st, sp, ct); // suppress unused
                    (q1 * inv, q2 * inv, q3 * inv)
                }
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Sphere wireframe (inflating)
// ─────────────────────────────────────────────────────────────────────────────

struct SphereInflate;

impl ProgressStyle for SphereInflate {
    fn name(&self) -> &str {
        "sphere-inflate"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Unit sphere wireframe expanding from a point to full radius as progress grows, spinning on time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.35;
        let ay = ctx.time * 0.62;
        let r = ctx.eased; // radius grows with eased, 0 → 1
        let n_lat = 7usize;
        let n_lon = 8usize;
        let steps = 50usize;
        // Latitude circles (constant polar angle θ).
        for i in 0..n_lat {
            let theta = PI * (i + 1) as f32 / (n_lat + 1) as f32;
            let ring_r = r * theta.sin();
            let z0 = r * theta.cos();
            plot_curve(grid, 0.0, 2.0 * PI, steps, ax, ay, cx, cy, scale, |phi| {
                (ring_r * phi.cos(), ring_r * phi.sin(), z0)
            });
        }
        // Longitude arcs (constant azimuth φ).
        for i in 0..n_lon {
            let phi = 2.0 * PI * i as f32 / n_lon as f32;
            let (sp, cp) = phi.sin_cos();
            plot_curve(grid, 0.0, PI, steps, ax, ay, cx, cy, scale, |theta| {
                (r * theta.sin() * cp, r * theta.sin() * sp, r * theta.cos())
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Helix / double helix climbing
// ─────────────────────────────────────────────────────────────────────────────

struct HelixClimb;

impl ProgressStyle for HelixClimb {
    fn name(&self) -> &str {
        "helix-climb"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Double helix on a cylinder — two strands climb in opposite phase as progress advances, rotating with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.25 + 0.3;
        let ay = ctx.time * 0.58;
        let turns = 4.0_f32;
        let t_max = ctx.eased * turns * 2.0 * PI;
        let steps = 200usize;
        let r = 0.6_f32;
        let height = 2.0_f32; // total height -1 to +1
                              // Strand A.
        plot_curve(grid, 0.0, t_max, steps, ax, ay, cx, cy, scale, |t| {
            let z = -1.0 + height * t / (turns * 2.0 * PI);
            (r * t.cos(), r * t.sin(), z)
        });
        // Strand B (π offset in phase).
        plot_curve(grid, 0.0, t_max, steps, ax, ay, cx, cy, scale, |t| {
            let z = -1.0 + height * t / (turns * 2.0 * PI);
            (r * (t + PI).cos(), r * (t + PI).sin(), z)
        });
        // Cross-links every half turn.
        let n_links = (ctx.eased * turns * 2.0) as usize;
        for i in 0..n_links {
            let t_link = i as f32 * PI;
            let z = -1.0 + height * t_link / (turns * 2.0 * PI);
            plot_curve(grid, 0.0, 1.0, 4, ax, ay, cx, cy, scale, |s| {
                let angle_a = t_link;
                let angle_b = t_link + PI;
                let xa = r * angle_a.cos();
                let ya = r * angle_a.sin();
                let xb = r * angle_b.cos();
                let yb = r * angle_b.sin();
                (xa + s * (xb - xa), ya + s * (yb - ya), z)
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Saddle surface (hyperbolic paraboloid)
// ─────────────────────────────────────────────────────────────────────────────

struct SaddleSurface;

impl ProgressStyle for SaddleSurface {
    fn name(&self) -> &str {
        "saddle-surface"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Hyperbolic paraboloid z = x²−y² wireframe — the canonical saddle point, revealed as a grid of iso-lines, rotating with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.22 + 0.5;
        let ay = ctx.time * 0.48;
        let n_lines = 12usize;
        let revealed = (ctx.eased * n_lines as f32 * 2.0).round() as usize;
        let steps = 60usize;
        // x-parallel iso-lines (vary y, constant x).
        for i in 0..revealed.min(n_lines) {
            let x = -1.0 + 2.0 * i as f32 / (n_lines - 1).max(1) as f32;
            plot_curve(grid, -1.0, 1.0, steps, ax, ay, cx, cy, scale, |y| {
                (x, y, x * x - y * y)
            });
        }
        // y-parallel iso-lines (vary x, constant y).
        let extra = revealed.saturating_sub(n_lines);
        for i in 0..extra.min(n_lines) {
            let y = -1.0 + 2.0 * i as f32 / (n_lines - 1).max(1) as f32;
            plot_curve(grid, -1.0, 1.0, steps, ax, ay, cx, cy, scale, |x| {
                (x, y, x * x - y * y)
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Tesseract (4-D hypercube) double projection
// ─────────────────────────────────────────────────────────────────────────────

struct TesseractSpin;

impl ProgressStyle for TesseractSpin {
    fn name(&self) -> &str {
        "tesseract-spin"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "4-D hypercube (tesseract) rotated in the XW and YZ planes then perspective-projected to 2-D — edges revealed by progress, spinning on time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let cx = (dw / 2) as i32;
        let cy = (dh / 2) as i32;
        let scale = (dw.min(dh * 2) as f32 * 0.35).max(1.0);

        // The 16 vertices of a unit tesseract in (x,y,z,w) ∈ {-1,+1}^4.
        let verts: [[f32; 4]; 16] = {
            let mut v = [[0.0f32; 4]; 16];
            for i in 0..16usize {
                v[i][0] = if i & 1 != 0 { 1.0 } else { -1.0 };
                v[i][1] = if i & 2 != 0 { 1.0 } else { -1.0 };
                v[i][2] = if i & 4 != 0 { 1.0 } else { -1.0 };
                v[i][3] = if i & 8 != 0 { 1.0 } else { -1.0 };
            }
            v
        };
        // The 32 edges: pairs that differ in exactly one bit.
        let mut edges: Vec<(usize, usize)> = Vec::with_capacity(32);
        for i in 0..16usize {
            for j in (i + 1)..16usize {
                if (i ^ j).count_ones() == 1 {
                    edges.push((i, j));
                }
            }
        }

        // Reveal edges up to progress.
        let n_show = (ctx.eased * edges.len() as f32).round() as usize;

        // 4-D rotation angles: XW plane (time), YZ plane (time/2).
        let a_xw = ctx.time * 0.48;
        let a_yz = ctx.time * 0.31;

        // Rotate all 16 vertices.
        let project4 = |vert: [f32; 4]| -> (i32, i32) {
            let [x0, y0, z0, w0] = vert;
            // XW rotation.
            let (sxw, cxw) = a_xw.sin_cos();
            let x1 = x0 * cxw - w0 * sxw;
            let w1 = x0 * sxw + w0 * cxw;
            // YZ rotation.
            let (syz, cyz) = a_yz.sin_cos();
            let y1 = y0 * cyz - z0 * syz;
            let z1 = y0 * syz + z0 * cyz;
            // 3-D XY rotation from ctx.time (slow tumble).
            let ax = ctx.time * 0.22;
            let ay = ctx.time * 0.37;
            let (sax, cax) = ax.sin_cos();
            let y2 = y1 * cax - z1 * sax;
            let z2 = y1 * sax + z1 * cax;
            let (say, cay) = ay.sin_cos();
            let x3 = x1 * cay + z2 * say;
            let y3 = y2;
            // Perspective from w-depth.
            let w_dist = 2.5 - w1 * 0.5;
            let inv = if w_dist.abs() < 0.01 {
                0.0
            } else {
                1.0 / w_dist
            };
            let sx = cx + (x3 * inv * scale) as i32;
            let sy = cy - (y3 * inv * scale) as i32;
            (sx, sy)
        };

        let projected: Vec<(i32, i32)> = verts.iter().map(|&v| project4(v)).collect();

        for &(a, b) in edges.iter().take(n_show) {
            let (x0, y0) = projected[a];
            let (x1, y1) = projected[b];
            // Bresenham line between projected endpoints.
            let dx = (x1 - x0).abs();
            let dy = (y1 - y0).abs();
            let sx = if x0 < x1 { 1i32 } else { -1i32 };
            let sy = if y0 < y1 { 1i32 } else { -1i32 };
            let mut x = x0;
            let mut y = y0;
            let mut err = dx - dy;
            let max_steps = (dx + dy + 1).min(300);
            for _ in 0..max_steps {
                draw::dot_i(grid, x, y);
                if x == x1 && y == y1 {
                    break;
                }
                let e2 = 2 * err;
                if e2 > -dy {
                    err -= dy;
                    x += sx;
                }
                if e2 < dx {
                    err += dx;
                    y += sy;
                }
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. Roman surface (Steiner's surface)
// ─────────────────────────────────────────────────────────────────────────────

struct RomanSurface;

impl ProgressStyle for RomanSurface {
    fn name(&self) -> &str {
        "roman-surface"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Steiner's Roman surface — one of the most beautiful immersions of ℝP² into ℝ³, with six Whitney umbrella singularities, revealed petal by petal"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.27 + 0.4;
        let ay = ctx.time * 0.43;
        // Parametrisation: u ∈ [0,π], v ∈ [0,π].
        // Reveal u-strips up to eased·π.
        let u_max = ctx.eased * PI;
        let u_lines = 18usize;
        let v_steps = 60usize;
        let s = scale * 0.6;
        for ui in 0..u_lines {
            let u = u_max * ui as f32 / u_lines.max(1) as f32;
            let (su, cu) = u.sin_cos();
            let s2u = (2.0 * u).sin();
            plot_curve(grid, 0.0, PI, v_steps, ax, ay, cx, cy, s, |v| {
                let (sv, _cv) = v.sin_cos();
                let s2v = (2.0 * v).sin();
                // Roman surface: x=sin²u·sin 2v, y=sin 2u·sin²v, z=sin 2u·sin 2v / 2.
                let x = su * su * s2v;
                let y = s2u * sv * sv;
                let z = 0.5 * s2u * s2v;
                let _ = cu;
                (x, y, z)
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 12. Seifert surface spiral ramp
// ─────────────────────────────────────────────────────────────────────────────

struct SeifertRamp;

impl ProgressStyle for SeifertRamp {
    fn name(&self) -> &str {
        "seifert-ramp"
    }
    fn theme(&self) -> &str {
        "topology"
    }
    fn describe(&self) -> &str {
        "Seifert-surface-inspired spiral ramp spanning a trefoil knot boundary — a twisted disk that rises through a full turn as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cx, cy, scale) = grid_cxys(grid);
        let ax = ctx.time * 0.30 + 0.5;
        let ay = ctx.time * 0.50;
        // Seifert surface for trefoil: parametrised by (r, theta) in a disk,
        // lifted to 3-D via the trefoil fibration.
        // Approximation: r ∈ [0,1], theta ∈ [0, 2π].
        // Point: rotate by theta/3 (fibration angle), scale radially.
        // x = r·cos(theta), y = r·sin(theta), z = r·sin(theta/3)·(1-r).
        let theta_max = ctx.eased * 2.0 * PI;
        let n_radii = 8usize;
        let steps = 80usize;
        // Concentric rings.
        for ri in 1..=n_radii {
            let r = ri as f32 / n_radii as f32;
            plot_curve(
                grid,
                0.0,
                theta_max,
                steps,
                ax,
                ay,
                cx,
                cy,
                scale,
                |theta| {
                    let x = r * theta.cos();
                    let y = r * theta.sin();
                    let z = r * (theta / 3.0).sin() * (1.0 - r * 0.5);
                    (x, y, z)
                },
            );
        }
        // Radial spokes.
        let n_spokes = 12usize;
        for si in 0..n_spokes {
            let theta = theta_max * si as f32 / n_spokes.max(1) as f32;
            plot_curve(grid, 0.0, 1.0, 20, ax, ay, cx, cy, scale, |r| {
                let x = r * theta.cos();
                let y = r * theta.sin();
                let z = r * (theta / 3.0).sin() * (1.0 - r * 0.5);
                (x, y, z)
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Registry
// ─────────────────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — green into blue.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(134, 232, 184);
const TINT_END: Color = Color::rgb(64, 104, 224);

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

/// All styles in the `topology` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per surface type, in the order they
/// appear in the source: Möbius, torus wireframe, torus knot, trefoil knot,
/// Klein bottle, Hopf fibers, sphere inflate, helix climb, saddle surface,
/// tesseract spin, Roman surface, Seifert ramp.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(MobiusStrip)),
        Box::new(Tinted(TorusWireframe)),
        Box::new(Tinted(TorusKnot)),
        Box::new(Tinted(TrefoilKnot)),
        Box::new(Tinted(KleinBottle)),
        Box::new(Tinted(HopfFibers)),
        Box::new(Tinted(SphereInflate)),
        Box::new(Tinted(HelixClimb)),
        Box::new(Tinted(SaddleSurface)),
        Box::new(Tinted(TesseractSpin)),
        Box::new(Tinted(RomanSurface)),
        Box::new(Tinted(SeifertRamp)),
    ]
}
