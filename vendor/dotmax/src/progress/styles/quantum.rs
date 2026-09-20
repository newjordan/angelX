//! Quantum-mechanics progress bars for dotmax.
//!
//! Ten styles, each grounded in a distinct quantum phenomenon.  Every bar is a
//! pure function of `(ctx.progress, ctx.eased, ctx.time)` — no mutable state.
//!
//! ## Styles (10 total)
//!
//! | Name | Phenomenon | Key idea |
//! |------|-----------|---------|
//! | `qubit-superposition` | Qubit |0⟩↔|1⟩ | state oscillates, collapses to |1⟩ at 100% |
//! | `bloch-sphere` | Bloch sphere | wireframe sphere, state vector precesses with time |
//! | `wavefunction-collapse` | Measurement | probability cloud sharpens to spike as eased→1 |
//! | `quantum-tunneling` | Tunneling | wave packet partially transmits through a barrier |
//! | `harmonic-oscillator` | QHO eigenstates | |ψₙ|² density ladder, n from eased |
//! | `particle-in-box` | PIB standing modes | sin²(nπx/L) mode n from eased |
//! | `spin-precession` | Spin precession | arrow coning on a circle, tilt from eased |
//! | `energy-levels` | Orbital filling | Bohr-like levels filling with electrons |
//! | `decoherence` | Decoherence | sharp fringes wash out as eased increases |
//! | `quantum-walk` | Quantum random walk | spreading binomial-like distribution |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// All styles in the `quantum` theme.
///
/// Returns 10 bars, each modelling a distinct quantum-mechanical phenomenon.
/// Safe to render at every size from 1×1 to 80×8 or beyond.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(QubitSuperposition),
        Box::new(BlochSphere),
        Box::new(WavefunctionCollapse),
        Box::new(QuantumTunneling),
        Box::new(HarmonicOscillator),
        Box::new(ParticleInBox),
        Box::new(SpinPrecession),
        Box::new(EnergyLevels),
        Box::new(Decoherence),
        Box::new(QuantumWalk),
    ]
}

// ---------------------------------------------------------------------------
// Helper: integer-step Bresenham line in dot space
// ---------------------------------------------------------------------------
#[inline]
fn line(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let steps = dx.max(dy).max(1);
    for i in 0..=steps {
        let px = x0 + (x1 - x0) * i / steps;
        let py = y0 + (y1 - y0) * i / steps;
        draw::dot_i(grid, px, py);
    }
}

// ---------------------------------------------------------------------------
// 1. Qubit Superposition
//    A qubit lives in state α|0⟩ + β|1⟩.
//    We render it as two columns (|0⟩ left, |1⟩ right) whose heights are
//    |α|² and |β|² respectively.
//    At rest, |α|² = cos²(θ/2), |β|² = sin²(θ/2).
//    θ oscillates with time (quantum precession): θ(t) = π·sin(ω·t).
//    Progress collapses the qubit: at eased=1, |β|²=1 (|1⟩ measured).
//    The "collapse" is a smooth interpolation: at eased < 1 the bar
//    oscillates; as eased→1 the oscillation damps and β²→1.
//    Progress bar: two tall vblock columns side-by-side, rest of width is
//    a dim track glyph; the relative heights encode the state amplitudes.
// ---------------------------------------------------------------------------
struct QubitSuperposition;
impl ProgressStyle for QubitSuperposition {
    fn name(&self) -> &str {
        "qubit-superposition"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Qubit |0⟩↔|1⟩ superposition: state oscillates, collapses to |1⟩ as progress reaches 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Oscillation angle θ(t) with collapse damping
        let omega = 2.5_f32;
        let osc = (omega * ctx.time).sin(); // -1..1
                                            // At eased=0 it oscillates freely; at eased=1 it is frozen to |1⟩
        let theta = PI * osc * (1.0 - ctx.eased); // collapses to 0 oscillation

        // State amplitudes (Born rule)
        let beta_sq = (theta / 2.0).sin().powi(2) * (1.0 - ctx.eased) + ctx.eased; // → 1.0 at completion
        let alpha_sq = (1.0 - beta_sq).max(0.0);

        // Tint: palette across the bar keyed by |β|²
        let (dw, dh) = draw::dot_dims(grid);
        let _ = (dw, dh);
        for cy in 0..ch {
            for cx in 0..cw {
                let t = cx as f32 / cw.max(1) as f32;
                let col = ctx.palette.sample(t * beta_sq);
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }

        // Two prominent columns: left = |0⟩, right = |1⟩
        // Each column is 2 cells wide; centred in the bar.
        let col_w = (cw / 2).max(1);
        let gap = col_w.min(2);
        let left_x = cw / 4;
        let right_x = (3 * cw / 4).min(cw.saturating_sub(1));

        // Convert amplitudes → vblock levels (0..8)
        let alpha_lv = (alpha_sq * 8.0).round() as usize;
        let beta_lv = (beta_sq * 8.0).round() as usize;

        // Draw from the bottom cell upward; top rows = full, bottom rows = empty
        for cy in 0..ch {
            let row_from_bottom = ch - 1 - cy;
            // alpha column (|0⟩)
            let alpha_rows = (alpha_lv * ch / 8).min(ch);
            if row_from_bottom < alpha_rows {
                for dx in 0..gap.max(1) {
                    if left_x + dx < cw {
                        draw::vblock(grid, left_x + dx, cy, 8);
                    }
                }
            }
            // beta column (|1⟩)
            let beta_rows = (beta_lv * ch / 8).min(ch);
            if row_from_bottom < beta_rows {
                for dx in 0..gap.max(1) {
                    if right_x + dx < cw {
                        draw::vblock(grid, right_x + dx, cy, 8);
                    }
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Bloch Sphere
//    A state vector |ψ⟩ = cos(θ/2)|0⟩ + e^{iφ}sin(θ/2)|1⟩ lives on the
//    unit sphere.  We render the sphere as three dot ellipses (equator +
//    two great-circle arcs) and draw the state vector as a Bresenham line
//    from the origin to its tip.
//    φ(t) = 2π·t (azimuthal precession driven by time).
//    θ = π·eased (polar angle: 0=north pole|0⟩ → π=south pole|1⟩).
// ---------------------------------------------------------------------------
struct BlochSphere;
impl ProgressStyle for BlochSphere {
    fn name(&self) -> &str {
        "bloch-sphere"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Bloch sphere: wireframe sphere + state vector precessing with time, polar angle from progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx = (dw / 2) as i32;
        let cy = (dh / 2) as i32;
        // Sphere radii — fit inside the dot canvas
        let rx = (dw / 2).saturating_sub(1) as i32; // horizontal radius
        let ry = (dh / 2).saturating_sub(1) as i32; // vertical radius

        // Draw equator (horizontal ellipse)
        let eq_steps = (rx * 2 + 8).max(8) as usize * 2;
        let mut prev: Option<(i32, i32)> = None;
        for s in 0..=eq_steps {
            let a = s as f32 / eq_steps as f32 * 2.0 * PI;
            // Project onto the inclined ellipse (perspective: y compressed)
            let px = cx + (rx as f32 * a.cos()) as i32;
            let py = cy + (ry as f32 * 0.35 * a.sin()) as i32; // flat equator
            draw::dot_i(grid, px, py);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, px, py);
            }
            prev = Some((px, py));
        }

        // Draw vertical great circle (meridian)
        let me_steps = (ry * 2 + 8).max(8) as usize * 2;
        let mut prev: Option<(i32, i32)> = None;
        for s in 0..=me_steps {
            let a = s as f32 / me_steps as f32 * 2.0 * PI;
            let px = cx + (rx as f32 * 0.35 * a.sin()) as i32; // slim meridian
            let py = cy + (ry as f32 * a.cos()) as i32;
            draw::dot_i(grid, px, py);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, px, py);
            }
            prev = Some((px, py));
        }

        // Draw outer circle (full sphere silhouette)
        let sil_steps = ((rx + ry) * 2 + 16).max(16) as usize;
        let mut prev: Option<(i32, i32)> = None;
        for s in 0..=sil_steps {
            let a = s as f32 / sil_steps as f32 * 2.0 * PI;
            let px = cx + (rx as f32 * a.cos()) as i32;
            let py = cy + (ry as f32 * a.sin()) as i32;
            draw::dot_i(grid, px, py);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, px, py);
            }
            prev = Some((px, py));
        }

        // State vector tip in 3D: (sin θ cos φ, sin θ sin φ, cos θ)
        let theta = PI * ctx.eased;
        let phi = 2.0 * PI * ctx.time * 0.5;
        // Project 3D → 2D dot coords (simple orthographic + aspect)
        let vx = theta.sin() * phi.cos(); // -1..1
        let vz = theta.cos(); // -1..1
        let tip_x = cx + (vx * rx as f32 * 0.85) as i32;
        let tip_y = cy - (vz * ry as f32 * 0.85) as i32; // screen y flipped
        line(grid, cx, cy, tip_x, tip_y);
        // Arrow head (small cross at tip)
        draw::dot_i(grid, tip_x, tip_y);
        draw::dot_i(grid, tip_x + 1, tip_y);
        draw::dot_i(grid, tip_x - 1, tip_y);
        draw::dot_i(grid, tip_x, tip_y + 1);
        draw::dot_i(grid, tip_x, tip_y - 1);

        // North/south poles
        draw::dot_i(grid, cx, cy - ry);
        draw::dot_i(grid, cx, cy + ry);

        // Tint filled half of bar
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cell_x in 0..filled_cells.min(cw) {
            let t = cell_x as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t);
            for cell_y in 0..ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Wavefunction Collapse
//    Before measurement: |ψ(x)|² is a broad Gaussian centred at mid-bar.
//    After measurement (eased→1): it narrows to a sharp delta spike.
//    Width σ(p) = σ_max · (1 − eased) + σ_min
//    We render the probability density as a column-height bar chart using
//    vblock glyphs — the shape is structural, not just a color wash.
// ---------------------------------------------------------------------------
struct WavefunctionCollapse;
impl ProgressStyle for WavefunctionCollapse {
    fn name(&self) -> &str {
        "wavefunction-collapse"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Wavefunction collapse: |ψ(x)|² spreads as a broad Gaussian then sharpens to a spike at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }
        let cwf = cw as f32;

        // Width of the Gaussian: broad at eased=0, narrow at eased=1
        let sigma_max = cwf * 0.35;
        let sigma_min = cwf.max(4.0) * 0.025;
        let sigma = sigma_max * (1.0 - ctx.eased) + sigma_min;

        // Measurement position: oscillates before collapse, locks at centre
        let center_norm = 0.5_f32 + 0.15 * (ctx.time * 1.8).sin() * (1.0 - ctx.eased);
        let mean = center_norm * cwf;

        // Scan columns, compute Gaussian height, draw vblock + tint
        for cx in 0..cw {
            let xf = cx as f32 + 0.5;
            let exponent = -0.5 * ((xf - mean) / sigma.max(0.1)).powi(2);
            let amplitude = exponent.exp().clamp(0.0, 1.0);

            // Use vblock in each cell of this column (top-down fill)
            for cy in 0..ch {
                let row_norm = 1.0 - cy as f32 / ch as f32; // 0 at bottom, 1 at top
                if amplitude >= row_norm {
                    draw::vblock(grid, cx, cy, 8);
                } else {
                    // partial block for the transition row
                    let partial = ((amplitude - (row_norm - 1.0 / ch as f32)) * ch as f32 * 8.0)
                        .clamp(0.0, 8.0) as usize;
                    if partial > 0 {
                        draw::vblock(grid, cx, cy, partial);
                    }
                }
            }

            // Tint by palette
            let t = cx as f32 / cwf;
            let col = ctx.palette.sample(t * amplitude + ctx.eased * (1.0 - t));
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Quantum Tunneling
//    A Gaussian wave packet travels rightward and hits a rectangular barrier
//    centred at 60% of the bar width.  The transmitted fraction = eased
//    (classically forbidden for small eased; quantum tunneling allows it).
//    Left of barrier: incident + reflected standing-wave pattern.
//    Right of barrier: transmitted wave of amplitude ~ sqrt(eased).
//    Both regions rendered as a sinusoidal waveform in dot space.
// ---------------------------------------------------------------------------
struct QuantumTunneling;
impl ProgressStyle for QuantumTunneling {
    fn name(&self) -> &str {
        "quantum-tunneling"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Quantum tunneling: wave packet hits a barrier, transmitted amplitude grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let dwf = dw as f32;
        let dhf = dh as f32;

        let mid_y = (dhf / 2.0) as i32;
        // Barrier position and width
        let barrier_x0 = (dwf * 0.55) as usize;
        let barrier_x1 = (dwf * 0.65) as usize;
        let barrier_h = (dhf * 0.7).max(2.0) as usize;
        let bar_top = ((dhf - barrier_h as f32) / 2.0) as usize;

        // Draw barrier outline
        for y in bar_top..(bar_top + barrier_h).min(dh) {
            draw::dot(grid, barrier_x0.min(dw.saturating_sub(1)), y);
            draw::dot(grid, barrier_x1.min(dw.saturating_sub(1)), y);
        }
        draw::hline(
            grid,
            barrier_x0,
            barrier_x1.min(dw.saturating_sub(1)),
            bar_top,
        );
        draw::hline(
            grid,
            barrier_x0,
            barrier_x1.min(dw.saturating_sub(1)),
            (bar_top + barrier_h)
                .saturating_sub(1)
                .min(dh.saturating_sub(1)),
        );

        // Evanescent fill inside barrier (dots spacing ∝ tunneling decay)
        let decay = (1.0 - ctx.eased).max(0.01);
        for xi in barrier_x0..=barrier_x1.min(dw.saturating_sub(1)) {
            let d = xi.saturating_sub(barrier_x0) as f32;
            let envelope =
                (-decay * 2.5 * d / (barrier_x1.saturating_sub(barrier_x0) as f32).max(1.0)).exp();
            if (d as usize % (2.max((1.0 / envelope.max(0.1)) as usize))) == 0 {
                draw::dot(grid, xi, mid_y as usize);
            }
        }

        // Incident + reflected wave (left region, standing wave)
        let k = 4.0 * PI / dwf; // wavenumber
        let omega = k * 2.0; // angular frequency
        let refl_amp = (1.0 - ctx.eased).sqrt();
        let inc_amp = 1.0_f32;
        let wave_amp = (dhf * 0.35).max(1.0);

        let mut prev: Option<(i32, i32)> = None;
        for xi in 0..barrier_x0 {
            let xf = xi as f32;
            let incident = inc_amp * (k * xf - omega * ctx.time).sin();
            let reflected = refl_amp * (k * xf + omega * ctx.time).sin();
            let total = (incident + reflected) * wave_amp;
            let py = (mid_y as f32 - total) as i32;
            let py = py.clamp(0, dh as i32 - 1);
            draw::dot_i(grid, xi as i32, py);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, xi as i32, py);
            }
            prev = Some((xi as i32, py));
        }

        // Transmitted wave (right region)
        let trans_amp = ctx.eased.sqrt();
        let mut prev: Option<(i32, i32)> = None;
        for xi in (barrier_x1 + 1)..dw {
            let xf = xi as f32;
            let transmitted = trans_amp * (k * xf - omega * ctx.time).sin() * wave_amp;
            let py = (mid_y as f32 - transmitted) as i32;
            let py = py.clamp(0, dh as i32 - 1);
            draw::dot_i(grid, xi as i32, py);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, xi as i32, py);
            }
            prev = Some((xi as i32, py));
        }

        // Axis
        draw::hline(grid, 0, dw.saturating_sub(1), mid_y as usize);

        // Tint: transmitted side keyed by eased
        let (cw, ch) = grid.dimensions();
        let split_cell = barrier_x1 / 2;
        for cx in split_cell.min(cw)..cw {
            let t = cx as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t * ctx.eased);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Quantum Harmonic Oscillator Eigenstates
//    |ψₙ(x)|² for the n-th energy eigenstate of a 1D QHO.
//    ψₙ(x) ∝ Hₙ(x)·exp(−x²/2) where Hₙ are Hermite polynomials.
//    Energy level n = floor(eased · N_max) — bar literally climbs the
//    ladder rung by rung.  We compute Hermite up to n≤6 via recurrence.
//    The probability density is rendered as a dot-height waveform.
// ---------------------------------------------------------------------------
struct HarmonicOscillator;
impl ProgressStyle for HarmonicOscillator {
    fn name(&self) -> &str {
        "harmonic-oscillator"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "QHO eigenstate: |ψₙ|² density plotted — energy level n steps up the ladder with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let dwf = dw as f32;
        let dhf = dh as f32;

        // Energy level n (0..6)
        let n_max = 6_usize;
        let n = (ctx.eased * (n_max + 1) as f32).floor() as usize;
        let n = n.min(n_max);

        // x range over which ψₙ is significant: ±x_max
        let x_max = 3.5_f32 + n as f32 * 0.4;

        // Hermite polynomial Hₙ(x) via recurrence: H₀=1, H₁=2x, Hₙ=2x·Hₙ₋₁−2(n-1)Hₙ₋₂
        let hermite = |x: f32| -> f32 {
            if n == 0 {
                return 1.0;
            }
            let mut h_prev2 = 1.0_f32;
            let mut h_prev1 = 2.0 * x;
            if n == 1 {
                return h_prev1;
            }
            let mut h = 0.0_f32;
            for k in 2..=(n as i32) {
                h = 2.0 * x * h_prev1 - 2.0 * (k - 1) as f32 * h_prev2;
                h_prev2 = h_prev1;
                h_prev1 = h;
            }
            h
        };

        // Normalise: sample the peak density
        let mut peak = 0.0_f32;
        let norm_samples = 64_usize;
        for i in 0..norm_samples {
            let xf = -x_max + 2.0 * x_max * i as f32 / norm_samples as f32;
            let hn = hermite(xf);
            let psi = hn * (-xf * xf / 2.0).exp();
            let dens = psi * psi;
            if dens > peak {
                peak = dens;
            }
        }
        let peak = peak.max(1e-9);

        // Animated phase shimmer on top
        let phase = ctx.time * 0.8;

        // Render: for each dot column compute density → dot height
        let mut prev: Option<(i32, i32)> = None;
        for xi in 0..dw {
            let xf = -x_max + 2.0 * x_max * xi as f32 / dwf;
            let hn = hermite(xf);
            let psi = hn * (-xf * xf / 2.0).exp();
            let density = (psi * psi / peak).clamp(0.0, 1.0);
            // Add a mild shimmer based on phase (re-part of wavefunction, not probability)
            let shimmer = 1.0 + 0.08 * (2.0 * xf + phase).cos();
            let display_h = (density * shimmer.clamp(0.0, 1.1) * dhf).clamp(0.0, dhf);

            let bot = dh as i32 - 1;
            let top = (bot as f32 - display_h) as i32;
            let top = top.clamp(0, bot);
            let mid_dot = (top + bot) / 2;
            draw::dot_i(grid, xi as i32, mid_dot);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, xi as i32, mid_dot);
            }
            prev = Some((xi as i32, mid_dot));
        }

        // Horizontal baseline
        draw::hline(grid, 0, dw.saturating_sub(1), dh.saturating_sub(1));

        // Tint by energy level fraction
        let (cw, ch) = grid.dimensions();
        for cx in 0..cw {
            let t = cx as f32 / cw.max(1) as f32;
            let level_t = n as f32 / n_max as f32;
            let col = ctx.palette.sample(t * 0.5 + level_t * 0.5);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Particle in a Box — standing modes
//    ψₙ(x) = sqrt(2/L)·sin(nπx/L),  n = 1,2,...
//    |ψₙ(x)|² = (2/L)·sin²(nπx/L) — a standing wave with n antinodes.
//    n is driven by eased: n = max(1, round(eased · N_max)).
//    The box walls are drawn on left and right; density as dot waveform.
// ---------------------------------------------------------------------------
struct ParticleInBox;
impl ProgressStyle for ParticleInBox {
    fn name(&self) -> &str {
        "particle-in-box"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Particle in a box: sin²(nπx/L) standing mode — mode n steps up with progress, walls visible"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let dhf = dh as f32;

        // Mode number n (1-based)
        let n_max = 7_usize;
        let n = (ctx.eased * n_max as f32).ceil() as usize;
        let n = n.clamp(1, n_max);

        // Box walls
        draw::vline(grid, 0, 0, dh.saturating_sub(1));
        draw::vline(grid, dw.saturating_sub(1), 0, dh.saturating_sub(1));

        // Interior x range (excluding walls, 1 dot each side)
        let x_start = 1_usize;
        let x_end = dw.saturating_sub(2);
        let box_w = x_end.saturating_sub(x_start).max(1);

        // Time-dependent phase (animated but density is |ψ|² so purely real)
        let phase = ctx.time * 1.2;

        let mut prev: Option<(i32, i32)> = None;
        for xi in x_start..=x_end {
            let xrel = (xi - x_start) as f32 / box_w as f32; // 0..1
            let density = (n as f32 * PI * xrel).sin().powi(2).clamp(0.0, 1.0);
            // Mild animated shimmer (Re part oscillates, density = time-averaged)
            let shimmer = 0.85 + 0.15 * (n as f32 * PI * xrel - phase * n as f32).sin().abs();
            let display_h = (density * shimmer * (dhf - 2.0)).clamp(0.0, dhf - 2.0);

            // Draw the wavefunction peak dot
            let base = dh.saturating_sub(2) as i32;
            let peak_dot = (base as f32 - display_h) as i32;
            let peak_dot = peak_dot.clamp(0, base);
            draw::dot_i(grid, xi as i32, peak_dot);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, xi as i32, peak_dot);
            }
            prev = Some((xi as i32, peak_dot));
        }

        // Floor of the box
        draw::hline(grid, 0, dw.saturating_sub(1), dh.saturating_sub(1));

        // Tint: colour maps the density (hot = high probability)
        let (cw, ch) = grid.dimensions();
        let x_start_c = 0_usize;
        let x_end_c = cw;
        for cx in x_start_c..x_end_c {
            let xrel = cx as f32 / cw.max(1) as f32;
            let density = (n as f32 * PI * xrel).sin().powi(2).clamp(0.0, 1.0);
            let col = ctx.palette.sample(density);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Spin Precession
//    A spin-½ particle in a magnetic field B precesses at the Larmor frequency.
//    The spin vector traces a cone: tip draws a circle at polar angle θ.
//    θ = π·eased/2 (equatorial plane = |+⟩ equally mixed, pole = pure|↑⟩).
//    The precession cone is drawn as an ellipse; the spin arrow from origin
//    to current tip is a Bresenham line; the cone base circle is dashed.
// ---------------------------------------------------------------------------
struct SpinPrecession;
impl ProgressStyle for SpinPrecession {
    fn name(&self) -> &str {
        "spin-precession"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Spin precession: Larmor cone — polar tilt from progress, azimuth rotates with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let dwf = dw as f32;
        let dhf = dh as f32;

        let origin_x = (dwf / 2.0) as i32;
        let origin_y = (dhf * 0.55) as i32; // slightly below centre

        // Spin vector length
        let len = (dhf * 0.4 + dwf * 0.15).min(dhf * 0.7).max(2.0);

        // Polar angle (cone half-angle) from eased; azimuth precesses with time
        let theta = PI / 2.0 * ctx.eased; // 0 (pole) → π/2 (equator)
        let phi = 2.0 * PI * ctx.time * 0.7; // Larmor precession

        // Spin vector tip in dot-space (orthographic projection)
        let tip_x = origin_x + (len * theta.sin() * phi.cos()) as i32;
        // y: up in physics = up on screen, correct since origin is mid
        let tip_y = origin_y - (len * theta.cos()) as i32;

        // Draw vertical z-axis (field direction)
        let z_top = (origin_y as f32 - len * 1.1) as i32;
        let z_bot = (origin_y as f32 + len * 0.3) as i32;
        for y in z_top.max(0)..=z_bot.min(dh as i32 - 1) {
            if y % 2 == 0 {
                draw::dot_i(grid, origin_x, y);
            }
        }

        // Cone outline: draw two generating lines (opposite azimuths)
        let tip_x2 = origin_x - (len * theta.sin() * phi.cos()) as i32;
        let tip_y2 = tip_y; // same height, opposite side
        line(grid, origin_x, origin_y, tip_x, tip_y);
        line(grid, origin_x, origin_y, tip_x2, tip_y2);

        // Precession circle: the ellipse at height tip_y
        let circle_rx = (len * theta.sin()).abs() as i32;
        let circle_ry = (circle_rx as f32 * 0.35) as i32; // foreshortened
        let circ_steps = (circle_rx * 2 + 8).max(8) as usize;
        let mut prev: Option<(i32, i32)> = None;
        for s in 0..=circ_steps {
            let a = s as f32 / circ_steps as f32 * 2.0 * PI;
            let px = origin_x + (circle_rx as f32 * a.cos()) as i32;
            let py = tip_y + (circle_ry as f32 * a.sin()) as i32;
            // Dashed: every other pair of steps
            if (s / 2) % 2 == 0 {
                draw::dot_i(grid, px, py);
                if let Some((lx, ly)) = prev {
                    line(grid, lx, ly, px, py);
                }
            }
            prev = Some((px, py));
        }

        // Spin arrow and tip
        line(grid, origin_x, origin_y, tip_x, tip_y);
        draw::dot_i(grid, tip_x, tip_y);
        draw::dot_i(grid, tip_x + 1, tip_y);
        draw::dot_i(grid, tip_x, tip_y + 1);

        // Origin dot
        draw::dot_i(grid, origin_x, origin_y);

        // Tint: radially from origin
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Energy Level Diagram with Electron Filling
//    N_levels horizontal lines (orbitals) stacked from bottom to top.
//    Progress fills them with electrons (Aufbau principle).
//    Each electron is a small filled circle (3-dot cross) on its level.
//    Levels are drawn as horizontal line segments; filled ones are tinted.
//    eased determines how many electrons have been placed.
// ---------------------------------------------------------------------------
struct EnergyLevels;
impl ProgressStyle for EnergyLevels {
    fn name(&self) -> &str {
        "energy-levels"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Energy level diagram: Aufbau filling — electrons populate orbitals from bottom as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let dwf = dw as f32;
        let dhf = dh as f32;

        // Number of levels — scale with height
        let n_levels = ((dh / 3).max(2)).min(8) as usize;
        // Each level holds 2 electrons (spin up/down)
        let n_electrons_max = n_levels * 2;
        let n_filled = (ctx.eased * n_electrons_max as f32).round() as usize;

        // Level y positions (equal spacing from bottom to top)
        // Level 0 is the lowest energy (bottom of display)
        for lv in 0..n_levels {
            let y_frac = 1.0 - (lv as f32 + 0.5) / n_levels as f32;
            let y_dot = (y_frac * (dhf - 2.0) + 1.0) as usize;
            let y_dot = y_dot.min(dh.saturating_sub(1));

            // Draw the level line (horizontal bar across 40% of width, centred)
            let lw = (dwf * 0.4).max(4.0) as usize;
            let lx0 = ((dwf - lw as f32) / 2.0) as usize;
            let lx1 = (lx0 + lw).min(dw.saturating_sub(1));
            draw::hline(grid, lx0, lx1, y_dot);

            // Draw electrons (up to 2 per level)
            let elec_base = lv * 2; // first electron index for this level
            for spin in 0..2_usize {
                let elec_idx = elec_base + spin;
                if elec_idx >= n_filled {
                    break;
                }
                // Place electron on left or right of level line
                let ex = if spin == 0 {
                    lx0.saturating_sub(3)
                } else {
                    (lx1 + 3).min(dw.saturating_sub(1))
                };
                // Cross-shaped electron marker
                draw::dot_i(grid, ex as i32, y_dot as i32);
                draw::dot_i(grid, ex as i32 - 1, y_dot as i32);
                draw::dot_i(grid, ex as i32 + 1, y_dot as i32);
                draw::dot_i(grid, ex as i32, y_dot as i32 - 1);
                draw::dot_i(grid, ex as i32, y_dot as i32 + 1);
            }

            // Energy axis tick on the left
            draw::dot(grid, 0, y_dot);
            draw::dot(grid, 1, y_dot);
        }

        // Vertical energy axis
        draw::vline(grid, 0, 0, dh.saturating_sub(1));

        // Tint: filled levels are brighter
        let (cw, ch) = grid.dimensions();
        let filled_levels = (n_filled + 1) / 2; // levels with at least one electron
        for lv in 0..filled_levels.min(n_levels) {
            let y_frac = 1.0 - (lv as f32 + 0.5) / n_levels as f32;
            let cy = (y_frac * (ch as f32 - 1.0)) as usize;
            let cy = cy.min(ch.saturating_sub(1));
            let t = lv as f32 / n_levels.max(1) as f32;
            let col = ctx.palette.sample(1.0 - t); // bottom levels warm, top cool
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), col);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Decoherence
//    An interference pattern starts crisp and washes out as the system
//    interacts with the environment (eased → 1 = fully decohered).
//    Fringe visibility V = 1 − eased.
//    Pattern: I(x) = (1/2)[1 + V·cos(k·x + phase(t))]
//    Rendered as a dot height waveform modulated by V.
//    At eased=0: vivid fringes; at eased=1: flat uniform intensity.
// ---------------------------------------------------------------------------
struct Decoherence;
impl ProgressStyle for Decoherence {
    fn name(&self) -> &str {
        "decoherence"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Decoherence: sharp interference fringes wash out to flat noise as entanglement grows"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let dwf = dw as f32;
        let dhf = dh as f32;

        // Fringe visibility: 1 = fully coherent, 0 = fully decohered
        let visibility = 1.0 - ctx.eased;
        // Wave vector: ~6 fringes across the bar
        let k = 6.0 * 2.0 * PI / dwf.max(1.0);
        let phase = ctx.time * 1.5;

        let mut prev: Option<(i32, i32)> = None;
        for xi in 0..dw {
            let xf = xi as f32;
            // Intensity: central uniform term + fringe term
            let intensity = 0.5 * (1.0 + visibility * (k * xf + phase).cos());
            let intensity = intensity.clamp(0.0, 1.0);
            let display_h = (intensity * (dhf - 1.0)).clamp(0.0, dhf - 1.0);
            let py = (dhf - 1.0 - display_h) as i32;
            let py = py.clamp(0, dh as i32 - 1);
            draw::dot_i(grid, xi as i32, py);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, xi as i32, py);
            }
            prev = Some((xi as i32, py));
        }

        // Zero-line (fully decohered reference)
        let zero_y = (dhf * 0.5) as usize;
        for xi in (0..dw).step_by(4) {
            draw::dot(grid, xi, zero_y.min(dh.saturating_sub(1)));
        }

        // Tint: cool when coherent, warm when decohered
        let (cw, ch) = grid.dimensions();
        for cx in 0..cw {
            let t = 1.0 - visibility; // 0=coherent(start color), 1=decohered(end color)
            let col = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Quantum Random Walk
//     A coin-flipped quantum walk spreads as a binomial distribution.
//     After t steps the standard deviation σ ∝ t (not √t as for classical).
//     We visualise the probability distribution over positions x ∈ [-W, W]
//     after N_steps steps, where N_steps = round(eased · N_max).
//     Distribution: approximate QW distribution (bimodal near ±σ),
//     computed from a simplified analytic envelope.
//     Rendered as a column-height bar chart (vblock glyphs).
// ---------------------------------------------------------------------------
struct QuantumWalk;
impl ProgressStyle for QuantumWalk {
    fn name(&self) -> &str {
        "quantum-walk"
    }
    fn theme(&self) -> &str {
        "quantum"
    }
    fn describe(&self) -> &str {
        "Quantum random walk: bimodal spreading distribution — spreads linearly (not √n) with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }
        let cwf = cw as f32;
        let chf = ch as f32;

        // Number of walk steps (spread increases with steps)
        let n_max = 40_usize;
        let n_steps = (ctx.eased * n_max as f32).round() as usize;
        let n_steps = n_steps.max(1);

        // QW distribution: bimodal with peaks near ±n/√2, suppressed centre
        // Approximate analytic envelope: P(x) ∝ 1/√(n² - x²) for |x| < n (arcsine-like)
        // We normalise and render.
        let nf = n_steps as f32;
        let sigma = nf / (2.0_f32).sqrt(); // RMS spread

        // Animated drift in peak position (Hadamard walk has slight asymmetry — animates)
        let drift_phase = ctx.time * 0.4;

        let mut densities = vec![0.0_f32; cw];
        let mut max_d = 0.0_f32;
        for cx in 0..cw {
            // Map column to position x ∈ [-nf, nf]
            let x = (cx as f32 / cwf - 0.5) * 2.0 * nf;
            // QW envelope: arcsine law ≈ (n² - x²)^(-1/2), zero outside
            let arg = nf * nf - x * x;
            let envelope = if arg > 0.0 { 1.0 / arg.sqrt() } else { 0.0 };
            // Animated interference ripple inside the distribution
            let ripple = 1.0 + 0.25 * (PI * x / sigma.max(0.1) + drift_phase).cos();
            let d = envelope * ripple.max(0.0);
            densities[cx] = d;
            if d > max_d {
                max_d = d;
            }
        }
        let max_d = max_d.max(1e-9);

        for cx in 0..cw {
            let norm = (densities[cx] / max_d).clamp(0.0, 1.0);
            // vblock per cell row
            for cy in 0..ch {
                let row_thresh = 1.0 - cy as f32 / chf;
                if norm >= row_thresh {
                    draw::vblock(grid, cx, cy, 8);
                } else {
                    let partial =
                        ((norm - (row_thresh - 1.0 / chf)) * chf * 8.0).clamp(0.0, 8.0) as usize;
                    if partial > 0 {
                        draw::vblock(grid, cx, cy, partial);
                    }
                }
            }

            // Tint: peaks in vivid colour
            let t = cx as f32 / cwf;
            let col = ctx.palette.sample(t * norm + (1.0 - norm) * 0.2);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }

        // Mark the walk origin (centre) with a baseline tick
        let centre_cell = cw / 2;
        draw::vblock(
            grid,
            centre_cell.min(cw.saturating_sub(1)),
            ch.saturating_sub(1),
            1,
        );

        Ok(())
    }
}
