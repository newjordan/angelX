//! Sacred-geometry / parametric-curve progress bars.
//!
//! Every style maps `ctx.eased` onto the swept parameter range of a classical
//! curve so the shape **draws itself** as progress rises, while `ctx.time`
//! drives continuous rotation / animation.  Eleven distinct styles, each
//! implementing the real mathematical equations in dot space.
//!
//! Curve catalogue:
//! - `rose`          — Rhodonea / rose curve r = cos(k·θ)
//! - `spirograph`    — Hypotrochoid spirograph
//! - `epitrochoid`   — Epicycloid variant spirograph
//! - `superformula`  — Gielis superformula with animated exponents
//! - `phyllotaxis`   — Sunflower / golden-angle spiral
//! - `cardioid`      — String-art cardioid (multiplication table on a circle)
//! - `astroid`       — Astroid (4-cusp hypocycloid)
//! - `lemniscate`    — Lemniscate of Bernoulli (figure-eight)
//! - `maurer-rose`   — Maurer rose (chord-connected rose points)
//! - `mystic-rose`   — Mystic rose (all chords of an n-gon)
//! - `fermat-spiral` — Fermat's / parabolic spiral revealed by progress

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ────────────────────────────────────────────────────────────────────────────
// Registry
// ────────────────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — cyan into violet.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(70, 200, 255);
const TINT_END: Color = Color::rgb(164, 96, 255);

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

/// All styles in the `geometry` theme.
///
/// Returns one `Box<dyn ProgressStyle>` per parametric-curve bar.  The vector
/// is ordered from simplest (rose) to most elaborate (mystic rose), but all
/// styles are independent and can be used in any order.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(Rose)),
        Box::new(Tinted(Spirograph)),
        Box::new(Tinted(Epitrochoid)),
        Box::new(Tinted(Superformula)),
        Box::new(Tinted(Phyllotaxis)),
        Box::new(Tinted(Cardioid)),
        Box::new(Tinted(Astroid)),
        Box::new(Tinted(Lemniscate)),
        Box::new(Tinted(MaurerRose)),
        Box::new(Tinted(MysticRose)),
        Box::new(Tinted(FermatSpiral)),
    ]
}

// ────────────────────────────────────────────────────────────────────────────
// Shared geometry helpers
// ────────────────────────────────────────────────────────────────────────────

/// Convert polar (r, theta) + grid center to dot-space (x, y), scale by
/// `scale` dots.  Returns signed ints so `draw::dot_i` can bounds-clip them.
#[inline]
fn polar_to_dot(cx: f32, cy: f32, r: f32, theta: f32, scale: f32) -> (i32, i32) {
    let x = cx + r * theta.cos() * scale;
    let y = cy - r * theta.sin() * scale; // y-axis flipped in screen space
    (x.round() as i32, y.round() as i32)
}

/// Compute a sensible uniform scale so a unit-radius curve fits the grid with
/// a small margin.  Uses the minimum half-dimension minus 1 dot of padding.
#[inline]
fn fit_scale(dw: usize, dh: usize) -> f32 {
    let hw = (dw as f32 / 2.0 - 1.0).max(1.0);
    let hh = (dh as f32 / 2.0 - 1.0).max(1.0);
    hw.min(hh)
}

/// Grid center in dot-space (floating-point).
#[inline]
fn center(dw: usize, dh: usize) -> (f32, f32) {
    (dw as f32 / 2.0, dh as f32 / 2.0)
}

// ────────────────────────────────────────────────────────────────────────────
// 1. Rose curve (Rhodonea) — r = cos(k·θ)
// ────────────────────────────────────────────────────────────────────────────

struct Rose;
impl ProgressStyle for Rose {
    fn name(&self) -> &str {
        "rose"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Rhodonea rose r=cos(k·θ): petals unfurl from the center as progress rises, \
         the whole bloom slowly rotating with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        // k=5 gives 5 petals (odd k → k petals, even k → 2k petals).
        let k: f32 = 5.0;
        // Full rose requires θ ∈ [0, π] for odd k; sweep up to eased·π.
        let theta_max = ctx.eased * PI;
        // Rotate the whole rose slowly with time.
        let rot = ctx.time * 0.3;

        let steps = (512.0 * ctx.eased).max(4.0) as usize;
        for i in 0..=steps {
            let theta = i as f32 / steps as f32 * theta_max;
            let r = (k * theta).cos();
            let (x, y) = polar_to_dot(cx, cy, r, theta + rot, scale);
            draw::dot_i(grid, x, y);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 2. Spirograph / hypotrochoid
// ────────────────────────────────────────────────────────────────────────────
//
//   x = (R − r)·cos θ + d·cos((R−r)/r · θ)
//   y = (R − r)·sin θ − d·sin((R−r)/r · θ)
//
// R=5, r=3, d=5 gives the classic "petals inside a ring" pattern.

struct Spirograph;
impl ProgressStyle for Spirograph {
    fn name(&self) -> &str {
        "spirograph"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Hypotrochoid spirograph: the inner-wheel trace of a circle rolling inside \
         a larger circle, drawing an intricate looping flower as progress sweeps θ"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        let big_r: f32 = 5.0;
        let small_r: f32 = 3.0;
        let d: f32 = 5.0;
        let ratio = (big_r - small_r) / small_r;
        // Full period requires lcm(R,r)/r full rotations of θ.
        let theta_max = ctx.eased * 2.0 * PI * small_r; // ≈ 6π for (5,3)
        let rot = ctx.time * 0.2;

        let steps = (800.0 * ctx.eased).max(4.0) as usize;
        for i in 0..=steps {
            let theta = i as f32 / steps as f32 * theta_max;
            let xf = (big_r - small_r) * theta.cos() + d * (ratio * theta).cos();
            let yf = (big_r - small_r) * theta.sin() - d * (ratio * theta).sin();
            // Normalise by the bounding radius so it fills the grid.
            let norm = big_r + d;
            let sx = cx + (xf / norm) * scale * (rot.cos());
            let sy = cy - (yf / norm) * scale;
            // Apply rotation in dot-space.
            let dx = xf / norm * scale;
            let dy = yf / norm * scale;
            let rx = dx * rot.cos() - dy * rot.sin();
            let ry = dx * rot.sin() + dy * rot.cos();
            let px = (cx + rx).round() as i32;
            let py = (cy - ry).round() as i32;
            let _ = (sx, sy); // suppress unused warning
            draw::dot_i(grid, px, py);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 3. Epitrochoid (epicycloid variant)
// ────────────────────────────────────────────────────────────────────────────
//
//   x = (R + r)·cos θ − d·cos((R+r)/r · θ)
//   y = (R + r)·sin θ − d·sin((R+r)/r · θ)

struct Epitrochoid;
impl ProgressStyle for Epitrochoid {
    fn name(&self) -> &str {
        "epitrochoid"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Epitrochoid: a small circle rolling *outside* a fixed circle traces \
         a multi-lobed crown; progress reveals the full pattern chord by chord"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        // R=3, r=1, d=2.5 → 3-lobed limaçon-like shape.
        let big_r: f32 = 3.0;
        let small_r: f32 = 1.0;
        let d: f32 = 2.5;
        let ratio = (big_r + small_r) / small_r;
        let theta_max = ctx.eased * 2.0 * PI;
        let rot = ctx.time * 0.25;

        let steps = (600.0 * ctx.eased).max(4.0) as usize;
        let norm = big_r + small_r + d;
        for i in 0..=steps {
            let theta = i as f32 / steps as f32 * theta_max;
            let xf = (big_r + small_r) * theta.cos() - d * (ratio * theta).cos();
            let yf = (big_r + small_r) * theta.sin() - d * (ratio * theta).sin();
            let dx = xf / norm * scale;
            let dy = yf / norm * scale;
            let rx = dx * rot.cos() - dy * rot.sin();
            let ry = dx * rot.sin() + dy * rot.cos();
            let px = (cx + rx).round() as i32;
            let py = (cy - ry).round() as i32;
            draw::dot_i(grid, px, py);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 4. Gielis Superformula
// ────────────────────────────────────────────────────────────────────────────
//
//   r(φ) = [ |cos(m·φ/4)/a|^n2 + |sin(m·φ/4)/b|^n3 ]^(−1/n1)
//
// Animate n1 with time to morph between circle, star, and amoeba shapes.

struct Superformula;
impl ProgressStyle for Superformula {
    fn name(&self) -> &str {
        "superformula"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Gielis superformula: a single equation spanning circles, stars, flowers, \
         and alien blobs — n-exponents morph continuously with time as the curve \
         draws itself with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        let m: f32 = 6.0; // symmetry order
        let a: f32 = 1.0;
        let b: f32 = 1.0;
        // Animate the exponents to morph the shape.
        let n1 = 1.5 + 2.5 * (ctx.time * 0.4).sin().abs();
        let n2 = 2.0 + 1.5 * (ctx.time * 0.31).cos();
        let n3 = 2.0 + 1.5 * (ctx.time * 0.27).sin();
        let rot = ctx.time * 0.18;

        let theta_max = ctx.eased * 2.0 * PI;
        let steps = (512.0 * ctx.eased).max(4.0) as usize;

        // Track max radius over the full sweep for normalization.
        // We pre-compute with the current n-values.
        let mut r_max: f32 = 0.001;
        for i in 0..=256 {
            let phi = i as f32 / 256.0 * 2.0 * PI;
            let t1 = ((m * phi / 4.0).cos() / a).abs().powf(n2);
            let t2 = ((m * phi / 4.0).sin() / b).abs().powf(n3);
            let sum = t1 + t2;
            if sum > 1e-6 {
                let r = sum.powf(-1.0 / n1);
                if r > r_max {
                    r_max = r;
                }
            }
        }

        for i in 0..=steps {
            let phi = i as f32 / steps as f32 * theta_max;
            let t1 = ((m * phi / 4.0).cos() / a).abs().powf(n2);
            let t2 = ((m * phi / 4.0).sin() / b).abs().powf(n3);
            let sum = t1 + t2;
            if sum < 1e-6 {
                continue;
            }
            let r = sum.powf(-1.0 / n1) / r_max;
            let dx = r * phi.cos() * scale;
            let dy = r * phi.sin() * scale;
            let rx = dx * rot.cos() - dy * rot.sin();
            let ry = dx * rot.sin() + dy * rot.cos();
            let px = (cx + rx).round() as i32;
            let py = (cy - ry).round() as i32;
            draw::dot_i(grid, px, py);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 5. Phyllotaxis / sunflower (golden-angle spiral)
// ────────────────────────────────────────────────────────────────────────────
//
//   angle_n = n · 137.5°
//   radius_n = c · √n
//
// Plot the first eased·N points; N grows so the spiral fills as progress rises.

struct Phyllotaxis;
impl ProgressStyle for Phyllotaxis {
    fn name(&self) -> &str {
        "phyllotaxis"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Sunflower phyllotaxis: seeds appear at the golden angle 137.5°, \
         radius ∝ √n, producing the mesmerising Fibonacci spiral pattern of \
         real sunflowers and pinecones"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        let golden_angle: f32 = 2.0 * PI * (1.0 - 1.0 / 1.618_033_9); // ≈ 137.508°
        let n_max: usize = 400;
        let n_plot = (ctx.eased * n_max as f32).round() as usize;
        // c chosen so the outermost seed lands near the grid edge.
        let c = scale / (n_max as f32).sqrt();
        let rot = ctx.time * 0.15;

        for n in 0..n_plot {
            let angle = n as f32 * golden_angle + rot;
            let r = c * (n as f32).sqrt();
            let px = (cx + r * angle.cos()).round() as i32;
            let py = (cy - r * angle.sin()).round() as i32;
            draw::dot_i(grid, px, py);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 6. Cardioid via string-art (multiplication table on a circle)
// ────────────────────────────────────────────────────────────────────────────
//
//   Place N points evenly around a unit circle.
//   Draw chord from point i to point (2·i mod N).
//   The envelope of these chords is the cardioid r = a(1 − cos θ).

struct Cardioid;
impl ProgressStyle for Cardioid {
    fn name(&self) -> &str {
        "cardioid"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Cardioid string-art: connecting point i to (2·i mod N) on a circle \
         traces the cardioid r=a(1−cosθ) as an emergent envelope — chords appear \
         one by one as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh) * 0.9;

        let n_total: usize = 180; // total points on the circle
        let n_chords = (ctx.eased * n_total as f32).round() as usize;
        let rot = ctx.time * 0.2;

        for i in 0..n_chords {
            let j = (2 * i) % n_total;
            let a_i = 2.0 * PI * i as f32 / n_total as f32 + rot;
            let a_j = 2.0 * PI * j as f32 / n_total as f32 + rot;

            let x0 = (cx + a_i.cos() * scale).round() as i32;
            let y0 = (cy - a_i.sin() * scale).round() as i32;
            let x1 = (cx + a_j.cos() * scale).round() as i32;
            let y1 = (cy - a_j.sin() * scale).round() as i32;

            // Bresenham line between the two circle points.
            bresenham(grid, x0, y0, x1, y1);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 7. Astroid (4-cusp hypocycloid)
// ────────────────────────────────────────────────────────────────────────────
//
//   x = R·cos³θ,  y = R·sin³θ   (hypocycloid with k=4, so r = R/4)

struct Astroid;
impl ProgressStyle for Astroid {
    fn name(&self) -> &str {
        "astroid"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Astroid (4-cusp hypocycloid): x=R·cos³θ, y=R·sin³θ — a star-shaped \
         curve with four sharp cusps that sweeps closed as progress completes a \
         full 2π revolution"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        let theta_max = ctx.eased * 2.0 * PI;
        let rot = ctx.time * 0.22;
        let steps = (512.0 * ctx.eased).max(4.0) as usize;

        for i in 0..=steps {
            let theta = i as f32 / steps as f32 * theta_max;
            let xf = theta.cos().powi(3);
            let yf = theta.sin().powi(3);
            let dx = xf * scale;
            let dy = yf * scale;
            let rx = dx * rot.cos() - dy * rot.sin();
            let ry = dx * rot.sin() + dy * rot.cos();
            let px = (cx + rx).round() as i32;
            let py = (cy - ry).round() as i32;
            draw::dot_i(grid, px, py);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 8. Lemniscate of Bernoulli — r² = a²·cos(2θ)
// ────────────────────────────────────────────────────────────────────────────
//
//   Cartesian form: (x²+y²)² = a²(x²−y²)
//   Parametric:     x = a·cos(t)/(1+sin²t),  y = a·sin(t)·cos(t)/(1+sin²t)

struct Lemniscate;
impl ProgressStyle for Lemniscate {
    fn name(&self) -> &str {
        "lemniscate"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Lemniscate of Bernoulli: the ∞ figure-eight (x²+y²)²=a²(x²−y²), \
         traced with the rational parametric form — both lobes materialise \
         symmetrically as progress sweeps 0→2π"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        let theta_max = ctx.eased * 2.0 * PI;
        let rot = ctx.time * 0.18;
        let steps = (512.0 * ctx.eased).max(4.0) as usize;

        for i in 0..=steps {
            let t = i as f32 / steps as f32 * theta_max;
            let denom = 1.0 + t.sin().powi(2);
            if denom.abs() < 1e-6 {
                continue;
            }
            // Rational parametric lemniscate (a = 1).
            let xf = t.cos() / denom;
            let yf = t.sin() * t.cos() / denom;
            let dx = xf * scale;
            let dy = yf * scale;
            let rx = dx * rot.cos() - dy * rot.sin();
            let ry = dx * rot.sin() + dy * rot.cos();
            let px = (cx + rx).round() as i32;
            let py = (cy - ry).round() as i32;
            draw::dot_i(grid, px, py);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 9. Maurer Rose
// ────────────────────────────────────────────────────────────────────────────
//
//   The rose curve r = cos(k·θ) evaluated at evenly spaced integer multiples
//   of d degrees, connected with straight chords.  d=71°, k=5 is classic.

struct MaurerRose;
impl ProgressStyle for MaurerRose {
    fn name(&self) -> &str {
        "maurer-rose"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Maurer rose: rose curve points at d-degree integer steps connected by \
         straight chords, producing a densely interlaced star web — d=71°, k=5 \
         gives the canonical intricate design"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        let k: f32 = 5.0;
        let d_deg: f32 = 71.0; // step in degrees
        let d_rad = d_deg * PI / 180.0;
        let n_total: usize = 361; // one full revolution in d-degree steps
        let n_chords = (ctx.eased * n_total as f32).round() as usize;
        let rot = ctx.time * 0.15;

        // Compute successive chord endpoints.
        let point = |n: usize| -> (i32, i32) {
            let theta = n as f32 * d_rad + rot;
            let r = (k * theta).cos();
            let x = (cx + r * theta.cos() * scale).round() as i32;
            let y = (cy - r * theta.sin() * scale).round() as i32;
            (x, y)
        };

        for n in 0..n_chords.saturating_sub(1) {
            let (x0, y0) = point(n);
            let (x1, y1) = point(n + 1);
            bresenham(grid, x0, y0, x1, y1);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 10. Mystic Rose — all chords of a regular n-gon
// ────────────────────────────────────────────────────────────────────────────
//
//   Place N points equally spaced on a circle and draw every chord.
//   Total chords = N·(N−1)/2.  Progress reveals them in order.

struct MysticRose;
impl ProgressStyle for MysticRose {
    fn name(&self) -> &str {
        "mystic-rose"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Mystic rose: every pair of vertices in a regular 24-gon connected by a \
         chord — 276 chords in total, appearing progressively to build a densely \
         woven stained-glass wheel"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        let n: usize = 24; // 24-gon → 276 chords
        let total_chords = n * (n - 1) / 2;
        let chords_to_draw = (ctx.eased * total_chords as f32).round() as usize;
        let rot = ctx.time * 0.12;

        let vertex = |i: usize| -> (i32, i32) {
            let angle = 2.0 * PI * i as f32 / n as f32 + rot;
            let px = (cx + angle.cos() * scale).round() as i32;
            let py = (cy - angle.sin() * scale).round() as i32;
            (px, py)
        };

        let mut drawn = 0usize;
        'outer: for i in 0..n {
            for j in (i + 1)..n {
                if drawn >= chords_to_draw {
                    break 'outer;
                }
                let (x0, y0) = vertex(i);
                let (x1, y1) = vertex(j);
                bresenham(grid, x0, y0, x1, y1);
                drawn += 1;
            }
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 11. Fermat's Spiral (parabolic spiral)
// ────────────────────────────────────────────────────────────────────────────
//
//   r = ±a·√θ,  θ ∈ [0, eased·θ_max]
//
// Both arms (±r) give the symmetric double-armed spiral.

struct FermatSpiral;
impl ProgressStyle for FermatSpiral {
    fn name(&self) -> &str {
        "fermat-spiral"
    }
    fn theme(&self) -> &str {
        "geometry"
    }
    fn describe(&self) -> &str {
        "Fermat's (parabolic) spiral r=±a√θ: both symmetric arms uncoil from the \
         origin as progress sweeps outward, the pair slowly rotating with time to \
         reveal a balanced double helix in braille dots"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cx, cy) = center(dw, dh);
        let scale = fit_scale(dw, dh);

        // How many full revolutions to uncoil (6π ≈ 3 turns).
        let theta_max = ctx.eased * 6.0 * PI;
        let rot = ctx.time * 0.17;
        // Normalise so the outermost point lands near the grid edge.
        // At θ_max the radius = a·√θ_max; we want that ≈ 1.0 in unit coords.
        let a = 1.0 / (6.0 * PI).sqrt(); // ensures r ≤ 1 at full sweep

        let steps = (600.0 * ctx.eased).max(4.0) as usize;
        for i in 0..=steps {
            let theta = i as f32 / steps as f32 * theta_max;
            let r = a * theta.sqrt();
            // Positive arm.
            let angle = theta + rot;
            let px = (cx + r * angle.cos() * scale).round() as i32;
            let py = (cy - r * angle.sin() * scale).round() as i32;
            draw::dot_i(grid, px, py);
            // Negative arm (π offset = opposite side).
            let qx = (cx - r * angle.cos() * scale).round() as i32;
            let qy = (cy + r * angle.sin() * scale).round() as i32;
            draw::dot_i(grid, qx, qy);
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Internal: integer Bresenham line rasteriser
// ────────────────────────────────────────────────────────────────────────────

/// Draw a straight line between two signed dot-space points using Bresenham's
/// algorithm.  Out-of-bounds dots are silently discarded by `draw::dot_i`.
fn bresenham(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    // Bound the number of steps to avoid infinite loops on degenerate input.
    let max_steps = (dx.abs() + dy.abs() + 2) as usize;
    let mut steps = 0usize;

    loop {
        draw::dot_i(grid, x0, y0);
        if x0 == x1 && y0 == y1 {
            break;
        }
        steps += 1;
        if steps > max_steps {
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
