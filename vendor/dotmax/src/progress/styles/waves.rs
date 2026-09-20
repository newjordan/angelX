//! Fourier / wave-synthesis progress bars for dotmax.
//!
//! Every bar in this theme is driven by real harmonic mathematics — no faked
//! wiggle, no cheap sine trickery. `ctx.eased` controls how many harmonics are
//! revealed, what amplitude is reached, or how far along a series we sit.
//! `ctx.time` advances the phase so every bar animates even at a fixed progress
//! value.
//!
//! ## Styles (11 total)
//!
//! | Name | Math |
//! |------|------|
//! | `fourier-square` | Gibbs-ringing square via `Σ sin((2k-1)x)/(2k-1)` |
//! | `fourier-sawtooth` | Sawtooth via `Σ (-1)^(k+1) sin(kx)/k` |
//! | `fourier-triangle` | Triangle via `Σ (-1)^k sin((2k+1)x)/(2k+1)²` |
//! | `epicycle` | Epicycle chain, vectors + traced path |
//! | `lissajous` | Lissajous figure, δ swept by `eased` |
//! | `standing-wave` | Standing wave, mode = 1+floor(eased·6) |
//! | `interference` | Two-source interference, constructive/destructive bands |
//! | `chladni` | Chladni nodal lines `cos(nπx)cos(mπy)−cos(mπx)cos(nπy)=0` |
//! | `beat-frequency` | Beat envelope `sin(f1·x)+sin(f2·x)`, Δf driven by eased |
//! | `wave-packet` | Gaussian-modulated travelling sinusoid |
//! | `spectrum` | Synthetic Fourier spectrum that fills with eased |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// All styles in the `waves` theme.
///
/// Returns 11 distinct bars, each implementing real harmonic mathematics and
/// safe to render from a 1×1 cell grid up to 80×8 or larger.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(FourierSquare),
        Box::new(FourierSawtooth),
        Box::new(FourierTriangle),
        Box::new(Epicycle),
        Box::new(Lissajous),
        Box::new(StandingWave),
        Box::new(Interference),
        Box::new(Chladni),
        Box::new(BeatFrequency),
        Box::new(WavePacket),
        Box::new(Spectrum),
    ]
}

// ---------------------------------------------------------------------------
// Helper: clamp a float y-coordinate into dot rows [0, h)
// ---------------------------------------------------------------------------
#[inline]
fn y_to_dot(norm: f32, h: usize) -> i32 {
    // norm in [-1, 1] → dot row in [0, h)
    let row = ((1.0 - (norm * 0.5 + 0.5)) * h as f32) as i32;
    row.clamp(0, h as i32 - 1)
}

// ---------------------------------------------------------------------------
// 1. Fourier square wave
//    y = (4/π) Σ_{k=1}^{N} sin((2k-1)·θ) / (2k-1)
//    N = 1 + floor(eased · 12)
//    Watch the Gibbs ear appear at N≥3 and sharpen toward ~9% overshoot.
// ---------------------------------------------------------------------------
struct FourierSquare;
impl ProgressStyle for FourierSquare {
    fn name(&self) -> &str {
        "fourier-square"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Fourier square-wave synthesis: Gibbs ringing grows visible as harmonics unlock with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let harmonics = 1 + (ctx.eased * 12.0).floor() as usize;
        let phase = ctx.time * 2.0 * PI * 0.3; // slow rightward travel

        // Draw a thin baseline
        let base = h / 2;
        draw::hline(grid, 0, w.saturating_sub(1), base);

        let mut prev_y: Option<i32> = None;
        for xi in 0..w {
            let theta = (xi as f32 / w as f32) * 4.0 * PI + phase;
            let mut val: f32 = 0.0;
            for k in 1..=harmonics {
                let n = (2 * k - 1) as f32;
                val += (n * theta).sin() / n;
            }
            val *= 4.0 / PI;
            // val ∈ roughly [-1.18, 1.18] due to Gibbs — clamp gently for vis
            let val_n = val.clamp(-1.0, 1.0);
            let dy = y_to_dot(val_n, h);
            draw::dot_i(grid, xi as i32, dy);
            // Connect consecutive dots vertically to avoid gaps
            if let Some(py) = prev_y {
                let lo = py.min(dy);
                let hi = py.max(dy);
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        // Tint: colour ramps from start to end across the filled region
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let t = if filled_cells <= 1 {
                0.5
            } else {
                cx as f32 / (filled_cells - 1) as f32
            };
            let col = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Fourier sawtooth
//    y = (2/π) Σ_{k=1}^{N} (-1)^{k+1} sin(k·θ) / k
// ---------------------------------------------------------------------------
struct FourierSawtooth;
impl ProgressStyle for FourierSawtooth {
    fn name(&self) -> &str {
        "fourier-sawtooth"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Fourier sawtooth synthesis: each harmonic adds a finer diagonal ramp until the teeth appear"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let harmonics = 1 + (ctx.eased * 11.0).floor() as usize;
        let phase = ctx.time * 2.0 * PI * 0.25;

        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.45).max(1.0);

        let mut prev_y: Option<i32> = None;
        for xi in 0..w {
            let theta = (xi as f32 / w as f32) * 4.0 * PI + phase;
            let mut val: f32 = 0.0;
            for k in 1..=harmonics {
                let kf = k as f32;
                let sign = if k % 2 == 1 { 1.0_f32 } else { -1.0_f32 };
                val += sign * (kf * theta).sin() / kf;
            }
            val *= 2.0 / PI;
            let dy = (mid - (val * amp) as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, dy);
            if let Some(py) = prev_y {
                let (lo, hi) = (py.min(dy), py.max(dy));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        // Subtle leading-edge colour
        let (cw, ch) = grid.dimensions();
        let head_cell = (ctx.eased * cw as f32).round() as usize;
        for cy in 0..ch {
            let col = ctx.palette.sample(ctx.eased);
            draw::tint_row(grid, cy, 0, head_cell.min(cw.saturating_sub(1)), col);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Fourier triangle wave
//    y = (8/π²) Σ_{k=0}^{N} (-1)^k sin((2k+1)·θ) / (2k+1)²
// ---------------------------------------------------------------------------
struct FourierTriangle;
impl ProgressStyle for FourierTriangle {
    fn name(&self) -> &str {
        "fourier-triangle"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Fourier triangle-wave: smoother convergence than square/sawtooth — peaks sharpen gradually"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let harmonics = 1 + (ctx.eased * 10.0).floor() as usize;
        let phase = ctx.time * 2.0 * PI * 0.2;

        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.44).max(1.0);

        let mut prev_y: Option<i32> = None;
        for xi in 0..w {
            let theta = (xi as f32 / w as f32) * 4.0 * PI + phase;
            let mut val: f32 = 0.0;
            for k in 0..harmonics {
                let kf = k as f32;
                let n = 2.0 * kf + 1.0;
                let sign = if k % 2 == 0 { 1.0_f32 } else { -1.0_f32 };
                val += sign * (n * theta).sin() / (n * n);
            }
            val *= 8.0 / (PI * PI);
            let dy = (mid - (val * amp) as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, dy);
            if let Some(py) = prev_y {
                let (lo, hi) = (py.min(dy), py.max(dy));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw as f32;
            let col = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Epicycle Fourier drawing
//    Chain of N rotating vectors (epicycles); each vector k has radius 1/k and
//    angular velocity k·ω. Their tip traces a path that converges to a square
//    wave as N grows. eased controls N = 1..=8, time drives ω.
// ---------------------------------------------------------------------------
struct Epicycle;
impl ProgressStyle for Epicycle {
    fn name(&self) -> &str {
        "epicycle"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Epicycle chain: N rotating vectors sum to trace a square-wave path; watch the circles spin"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let n_vec = 1 + (ctx.eased * 7.0).floor() as usize; // 1..=8
        let omega = ctx.time * 1.8; // base angular speed (rad/s)

        // Origin at left-centre; epicycles stack rightward conceptually but
        // we render the tip path across the full width.
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let r_base = (h as f32 * 0.35).max(1.0);

        // Draw the path traced by the tip across many phase samples
        let steps = w * 2;
        let mut prev: Option<(i32, i32)> = None;
        for si in 0..steps {
            let t = si as f32 / steps as f32; // [0, 1)
            let phase_offset = t * 2.0 * PI;
            let mut tip_x = cx;
            let mut tip_y = cy;
            for k in 1..=n_vec {
                let kf = k as f32;
                let radius = r_base / kf;
                let angle = (2 * k - 1) as f32 * (omega + phase_offset);
                tip_x += radius * angle.cos();
                tip_y += radius * angle.sin();
            }
            // Map tip_x across bar width (re-map from cx±r_base to [0,w))
            let px = ((tip_x / w as f32) * w as f32) as i32;
            let py = tip_y as i32;
            draw::dot_i(grid, px, py);
            if let Some((lx, ly)) = prev {
                // Connect with Bresenham-style horizontal smear
                let dx = (px - lx).abs();
                if dx > 0 {
                    for step in 0..=dx {
                        let ix = lx + (px - lx) * step / dx.max(1);
                        let iy = ly + (py - ly) * step / dx.max(1);
                        draw::dot_i(grid, ix, iy);
                    }
                }
                draw::dot_i(grid, px, py);
            }
            prev = Some((px, py));
        }

        // Draw the outermost circle outline (largest epicycle) at current time
        let angle0 = omega;
        let r0 = r_base;
        let circle_pts = 48usize;
        for i in 0..circle_pts {
            let a = angle0 + (i as f32 / circle_pts as f32) * 2.0 * PI;
            let px = (cx + r0 * a.cos()) as i32;
            let py = (cy + r0 * a.sin()) as i32;
            draw::dot_i(grid, px, py);
        }

        // Draw the arm from centre to tip at current phase (static snapshot)
        {
            let mut tip_x = cx;
            let mut tip_y = cy;
            for k in 1..=n_vec {
                let kf = k as f32;
                let radius = r_base / kf;
                let angle = (2 * k - 1) as f32 * omega;
                let nx = tip_x + radius * angle.cos();
                let ny = tip_y + radius * angle.sin();
                // Draw arm segment
                let steps2 = (radius.max(1.0) as usize).max(2);
                for s in 0..=steps2 {
                    let frac = s as f32 / steps2 as f32;
                    let ax = (tip_x + (nx - tip_x) * frac) as i32;
                    let ay = (tip_y + (ny - tip_y) * frac) as i32;
                    draw::dot_i(grid, ax, ay);
                }
                tip_x = nx;
                tip_y = ny;
            }
        }

        // Colour the bar by eased
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx2 in 0..filled.min(cw) {
            let t = cx2 as f32 / cw as f32;
            for cy2 in 0..ch {
                draw::tint_row(grid, cy2, cx2, cx2, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Lissajous figure
//    x = sin(a·τ + δ),  y = sin(b·τ)
//    a=3, b=2 (3:2 ratio);  δ = eased·2π so phase sweep reveals the knot.
//    τ animated by time.
// ---------------------------------------------------------------------------
struct Lissajous;
impl ProgressStyle for Lissajous {
    fn name(&self) -> &str {
        "lissajous"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Lissajous figure 3:2 — phase delta sweeps with progress, time rotates the knot in place"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let cx = (w as f32 - 1.0) / 2.0;
        let cy = (h as f32 - 1.0) / 2.0;
        let rx = cx * 0.92;
        let ry = cy * 0.92;

        let a = 3.0_f32;
        let b = 2.0_f32;
        let delta = ctx.eased * 2.0 * PI;
        let tau_off = ctx.time * 0.5; // slow rotation of the whole figure

        let steps = (w * h * 2).max(256);
        let period = 2.0 * PI; // one full Lissajous period in τ
        let mut prev: Option<(i32, i32)> = None;
        for si in 0..steps {
            let tau = (si as f32 / steps as f32) * period + tau_off;
            let lx = rx * (a * tau + delta).sin();
            let ly = ry * (b * tau).sin();
            let px = (cx + lx) as i32;
            let py = (cy + ly) as i32;
            draw::dot_i(grid, px, py);
            if let Some((ox, oy)) = prev {
                // Fill gaps with an interpolating walk
                let steps2 = (((px - ox).abs() + (py - oy).abs()) as usize).max(1);
                for s in 1..steps2 {
                    let f = s as f32 / steps2 as f32;
                    let ix = (ox as f32 + (px - ox) as f32 * f) as i32;
                    let iy = (oy as f32 + (py - oy) as f32 * f) as i32;
                    draw::dot_i(grid, ix, iy);
                }
            }
            prev = Some((px, py));
        }

        // Tint: gradient across the figure, keyed by eased
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx2 in 0..filled.min(cw) {
            let t = cx2 as f32 / cw as f32;
            for cy2 in 0..ch {
                draw::tint_row(grid, cy2, cx2, cx2, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Standing wave
//    y(x, t) = 2A · sin(k·x) · cos(ω·t)
//    mode number n = 1 + floor(eased·6), so k = n·π/L
//    Nodes are fixed; antinodes breathe with cos(ωt).
// ---------------------------------------------------------------------------
struct StandingWave;
impl ProgressStyle for StandingWave {
    fn name(&self) -> &str {
        "standing-wave"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Standing wave: fixed nodes, breathing antinodes — mode unlocks as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mode = 1 + (ctx.eased * 6.0).floor() as usize; // 1..=7
        let k = mode as f32 * PI / w as f32; // wavenumber
        let omega = 2.5 * PI; // angular freq (vis only)
        let amp = h as f32 * 0.40;
        let mid = (h / 2) as i32;

        // Draw baseline
        draw::hline(grid, 0, w.saturating_sub(1), mid as usize);

        let mut prev_y: Option<i32> = None;
        for xi in 0..w {
            let xf = xi as f32;
            let val = 2.0 * amp * (k * xf).sin() * (omega * ctx.time).cos();
            let dy = (mid - val as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, dy);
            if let Some(py) = prev_y {
                let (lo, hi) = (py.min(dy), py.max(dy));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        // Mark node positions with tiny vertical strokes
        for n in 0..=mode {
            let node_x = (n as f32 / mode as f32 * w as f32) as usize;
            if node_x < w {
                let tick = (h / 8).max(1);
                let y0 = mid as usize;
                draw::vline(
                    grid,
                    node_x,
                    y0.saturating_sub(tick),
                    (y0 + tick).min(h - 1),
                );
            }
        }

        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Two-source interference
//    Two point sources at x=0 and x=L produce circular waves.
//    Intensity ∝ (sin(k·r1 − ω·t) + sin(k·r2 − ω·t))²
//    Lit if intensity > threshold → constructive/destructive bands.
// ---------------------------------------------------------------------------
struct Interference;
impl ProgressStyle for Interference {
    fn name(&self) -> &str {
        "interference"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Two-source interference: constructive/destructive fringe bands sweep with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        // Sources: left-centre and right-centre, separation driven by eased
        let sep = (ctx.eased * wf * 0.6 + wf * 0.1).min(wf - 1.0);
        let src1_x = (wf / 2.0 - sep / 2.0).max(0.0);
        let src2_x = (wf / 2.0 + sep / 2.0).min(wf - 1.0);
        let src_y = hf / 2.0;

        let lambda = wf / 4.0; // wavelength in dots
        let k = 2.0 * PI / lambda.max(1.0);
        let omega = 2.0 * PI * 1.2;
        let threshold = 0.5_f32;

        for yi in 0..h {
            let yf = yi as f32;
            for xi in 0..w {
                let xf = xi as f32;
                let r1 = ((xf - src1_x).powi(2) + (yf - src_y).powi(2)).sqrt();
                let r2 = ((xf - src2_x).powi(2) + (yf - src_y).powi(2)).sqrt();
                let s1 = (k * r1 - omega * ctx.time).sin();
                let s2 = (k * r2 - omega * ctx.time).sin();
                let intensity = (s1 + s2) * 0.5; // ∈ [-1, 1]
                if intensity.abs() > threshold {
                    draw::dot(grid, xi, yi);
                }
            }
        }

        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Chladni plate pattern
//    Nodal lines defined by: cos(n·π·x)·cos(m·π·y) − cos(m·π·x)·cos(n·π·y) ≈ 0
//    (n, m) chosen by eased: eased selects among (1,2),(1,3),(2,3),(2,5),(3,4).
//    Points near the nodal lines are lit.
// ---------------------------------------------------------------------------
struct Chladni;
impl ProgressStyle for Chladni {
    fn name(&self) -> &str {
        "chladni"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Chladni nodal lines: sand settles on vibration nodes — pattern changes with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Mode table: (n, m) pairs
        let modes: [(f32, f32); 5] = [(1.0, 2.0), (1.0, 3.0), (2.0, 3.0), (2.0, 5.0), (3.0, 4.0)];
        let idx = (ctx.eased * 4.999) as usize;
        let (n, m) = modes[idx.min(4)];

        // Animate: slow rotation of effective n/m by blending adjacent modes
        let blend = (ctx.time * 0.15).sin() * 0.08;
        let n = n + blend;
        let m = m - blend;

        let threshold = 0.25_f32;

        for yi in 0..h {
            let yf = yi as f32 / h as f32; // [0,1]
            for xi in 0..w {
                let xf = xi as f32 / w as f32; // [0,1]
                let val = (n * PI * xf).cos() * (m * PI * yf).cos()
                    - (m * PI * xf).cos() * (n * PI * yf).cos();
                if val.abs() < threshold {
                    draw::dot(grid, xi, yi);
                }
            }
        }

        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Beat frequency
//    y(x) = sin(f1·x) + sin(f2·x)
//    f1 = base freq, f2 = f1 + Δf where Δf = eased · 0.2
//    Envelope: |2·cos(Δf·x/2)| sculpts the familiar beat "lumps".
//    time shifts both sinusoids together (travelling beats).
// ---------------------------------------------------------------------------
struct BeatFrequency;
impl ProgressStyle for BeatFrequency {
    fn name(&self) -> &str {
        "beat-frequency"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Beat frequency: two close sinusoids interfere to create a slowly pulsing envelope"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.44).max(1.0);

        let f1 = 8.0_f32; // cycles across bar
        let df = ctx.eased * 2.0 + 0.05; // beat frequency (eased → 0..2.05 extra cycles)
        let f2 = f1 + df;
        let phase_shift = ctx.time * 2.0 * PI * 0.4;

        let mut prev_y: Option<i32> = None;
        for xi in 0..w {
            let xn = xi as f32 / w as f32; // [0, 1]
            let theta = xn * 2.0 * PI + phase_shift;
            let val = (f1 * theta).sin() + (f2 * theta).sin();
            // val ∈ [-2, 2] nominally
            let val_n = val * 0.5; // normalise to [-1, 1]
            let dy = (mid - (val_n * amp) as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, dy);
            if let Some(py) = prev_y {
                let (lo, hi) = (py.min(dy), py.max(dy));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        // Draw the envelope: ±2·|cos(Δf·θ/2)|
        for xi in 0..w {
            let xn = xi as f32 / w as f32;
            let theta = xn * 2.0 * PI + phase_shift;
            let env = 2.0 * ((df * theta / 2.0).cos()).abs() * 0.5 * amp;
            let top_y = (mid - env as i32).clamp(0, h as i32 - 1);
            let bot_y = (mid + env as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, top_y);
            draw::dot_i(grid, xi as i32, bot_y);
        }

        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Wave packet
//     A Gaussian-modulated sinusoid: y = A·exp(-((x-x0)/σ)²) · sin(k·(x-x0)+φ)
//     x0 = progress × L (packet position), σ = L/6, k = 12π/L.
//     time provides the carrier phase φ = ω·t so the inner fringes oscillate.
// ---------------------------------------------------------------------------
struct WavePacket;
impl ProgressStyle for WavePacket {
    fn name(&self) -> &str {
        "wave-packet"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Gaussian wave packet: a quantum-style probability envelope travels across the bar"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.44).max(1.0);

        let x0 = ctx.eased * wf;
        let sigma = (wf / 5.0).max(1.0);
        let k = 12.0 * PI / wf;
        let phi = ctx.time * 4.0 * PI; // carrier phase

        let mut prev_y: Option<i32> = None;
        for xi in 0..w {
            let xf = xi as f32;
            let dx = xf - x0;
            let envelope = (-(dx * dx) / (2.0 * sigma * sigma)).exp();
            let carrier = (k * dx + phi).sin();
            let val = envelope * carrier;
            let dy = (mid - (val * amp) as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, dy);
            if let Some(py) = prev_y {
                let (lo, hi) = (py.min(dy), py.max(dy));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        // Draw the Gaussian envelope outline above and below mid
        let mut prev_env: Option<(i32, i32)> = None;
        for xi in 0..w {
            let xf = xi as f32;
            let dx = xf - x0;
            let envelope = (-(dx * dx) / (2.0 * sigma * sigma)).exp();
            let ey = (envelope * amp) as i32;
            let top = (mid - ey).clamp(0, h as i32 - 1);
            let bot = (mid + ey).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, top);
            draw::dot_i(grid, xi as i32, bot);
            if let Some((pt, pb)) = prev_env {
                // Connect envelope curve
                let (lt, ht) = (pt.min(top), pt.max(top));
                let (lb, hb) = (pb.min(bot), pb.max(bot));
                for yy in lt..=ht {
                    draw::dot_i(grid, xi as i32, yy);
                }
                for yy in lb..=hb {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_env = Some((top, bot));
        }

        let (cw, ch) = grid.dimensions();
        // Tint concentrated around the packet centre
        let centre_cell = (ctx.eased * cw as f32).round() as usize;
        let sigma_cells = (cw / 5).max(1);
        for cx in 0..cw {
            let dist = if cx > centre_cell {
                (cx - centre_cell) as f32
            } else {
                (centre_cell - cx) as f32
            };
            let env_c = (-(dist * dist) / (2.0 * sigma_cells as f32 * sigma_cells as f32)).exp();
            if env_c > 0.05 {
                let col = ctx.palette.sample(cx as f32 / cw as f32);
                for cy in 0..ch {
                    draw::tint_row(grid, cy, cx, cx, col);
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Spectrum / equalizer bars
//     Synthetic Fourier-domain spectrum: N frequency bins, each amplitude
//     = A_k · (progress fill) + noise driven by sin(k·time).
//     eased controls how many bins are "filled" (left-to-right reveal).
//     Animated by time so each column pulses at its own frequency.
// ---------------------------------------------------------------------------
struct Spectrum;
impl ProgressStyle for Spectrum {
    fn name(&self) -> &str {
        "spectrum"
    }
    fn theme(&self) -> &str {
        "waves"
    }
    fn describe(&self) -> &str {
        "Fourier spectrum equalizer: frequency bins light up left-to-right as progress fills them"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let n_bins = w;
        // How many bins are "active" — left n_filled are in the revealed region
        let n_filled = (ctx.eased * n_bins as f32).round() as usize;

        // Draw baseline
        if h >= 1 {
            draw::hline(grid, 0, w.saturating_sub(1), h - 1);
        }

        for bin in 0..n_bins {
            // Spectral envelope: 1/f-like roll-off from DC with a few harmonics louder
            let bin_f = (bin + 1) as f32;
            let spectral = (1.0 / bin_f.sqrt())
                * (1.0 + 0.4 * (bin_f * 0.7).sin())
                * (1.0 + 0.2 * (bin_f * 1.3 + ctx.time * 2.5).sin().abs());

            // Amplitude: spectral shape × whether this bin is "filled"
            let amplitude = if bin < n_filled {
                spectral
            } else {
                spectral * 0.08
            };
            let bar_h = (amplitude * h as f32 * 0.85).round() as usize;
            let bar_h = bar_h.clamp(0, h);

            let y0 = h.saturating_sub(bar_h);
            for y in y0..h {
                draw::dot(grid, bin, y);
            }
        }

        // Colour: gradient across filled bins
        let (cw, ch) = grid.dimensions();
        // Map dot-bins to cells (each cell = 2 dot-columns)
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let t = cx as f32 / cw as f32;
            let col = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}
