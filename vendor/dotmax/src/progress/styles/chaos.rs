//! Chaos / strange-attractor progress bars.
//!
//! Each style implements a real dynamical system from the literature, iterating
//! from a fixed seed each frame (stateless) so the bar is a pure function of
//! `(progress, time)`. Attractor orbits are mapped into dot-space coordinates
//! and revealed progressively via `ctx.eased`. Parameter animation uses
//! `ctx.time` to drive slow rotations, phase-shifts, and bifurcations.
//!
//! Systems implemented:
//! - **Lorenz** — the canonical butterfly attractor (σ=10, ρ=28, β=8/3).
//! - **Rössler** — a single spiral band (a=0.2, b=0.2, c=5.7).
//! - **Clifford** — algebraic iterated-function system with animated params.
//! - **De Jong** — sinusoidal IFS, slowly morphing.
//! - **Hénon map** — folded parabola (a=1.4, b=0.3).
//! - **Logistic bifurcation** — bifurcation diagram swept across the bar.
//! - **Double pendulum** — Lagrangian chaotic trace.
//! - **Standard map** — area-preserving twist map on the torus.
//! - **Gingerbreadman map** — triangular symmetry cellular automaton-like.
//! - **Duffing oscillator** — phase portrait of the forced anharmonic well.
//! - **Tinkerbell map** — complex-parameter fractal boundary.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Deterministic hash helper for seeding
// ---------------------------------------------------------------------------

#[inline(always)]
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Theme tint — hot pink into violet.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(255, 84, 130);
const TINT_END: Color = Color::rgb(128, 44, 212);

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

/// All styles in the `chaos` theme.
///
/// Returns 11 dynamical-system progress bars. Every style is stateless and
/// renders from a fixed seed each frame; no mutable state is held.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(LorenzAttractor)),
        Box::new(Tinted(RosslerAttractor)),
        Box::new(Tinted(CliffordAttractor)),
        Box::new(Tinted(DeJongAttractor)),
        Box::new(Tinted(HenonMap)),
        Box::new(Tinted(LogisticBifurcation)),
        Box::new(Tinted(DoublePendulum)),
        Box::new(Tinted(StandardMap)),
        Box::new(Tinted(Gingerbreadman)),
        Box::new(Tinted(DuffingOscillator)),
        Box::new(Tinted(TinkerbellMap)),
    ]
}

// ---------------------------------------------------------------------------
// 1. Lorenz Attractor
//    dx/dt = σ(y − x)        σ = 10
//    dy/dt = x(ρ − z) − y    ρ = 28
//    dz/dt = xy − βz         β = 8/3
//
//    Progress reveals orbit length. Time rotates the (x,z) projection.
// ---------------------------------------------------------------------------

struct LorenzAttractor;

impl ProgressStyle for LorenzAttractor {
    fn name(&self) -> &str {
        "lorenz"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Lorenz butterfly attractor: σ=10 ρ=28 β=8/3 — orbit revealed by progress, time rotates projection"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let sigma: f32 = 10.0;
        let rho: f32 = 28.0;
        let beta: f32 = 8.0 / 3.0;
        let dt: f32 = 0.01;

        // Total steps budget; eased controls how many we draw.
        let n_total: usize = 2800;
        let n_draw = (ctx.eased * n_total as f32).max(1.0) as usize;

        // Fixed seed — attractor min/max known analytically.
        let x_range = (-20.0_f32, 20.0_f32);
        let z_range = (0.0_f32, 50.0_f32);

        // Slow rotation of the projection plane with time.
        let angle = ctx.time * 0.3;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let mut x: f32 = 0.1;
        let mut y: f32 = 0.0;
        let mut z: f32 = 0.0;

        // Burn-in to get onto the attractor.
        for _ in 0..300 {
            let dx = sigma * (y - x);
            let dy = x * (rho - z) - y;
            let dz = x * y - beta * z;
            x += dx * dt;
            y += dy * dt;
            z += dz * dt;
        }

        for _ in 0..n_draw.min(n_total) {
            let dx = sigma * (y - x);
            let dy = x * (rho - z) - y;
            let dz = x * y - beta * z;
            x += dx * dt;
            y += dy * dt;
            z += dz * dt;

            // Project: rotate in (x,y) plane then use (proj_x, z) for screen.
            let proj_x = x * cos_a - y * sin_a;
            let norm_x = (proj_x - x_range.0) / (x_range.1 - x_range.0);
            let norm_z = (z - z_range.0) / (z_range.1 - z_range.0);

            let px = (norm_x.clamp(0.0, 1.0) * (dw - 1) as f32) as i32;
            let py = ((1.0 - norm_z.clamp(0.0, 1.0)) * (dh - 1) as f32) as i32;
            draw::dot_i(grid, px, py);
        }

        // Progress-bar underline: fill the bottom dot row to show eased.
        let filled = (ctx.eased * dw as f32) as usize;
        draw::hline(
            grid,
            0,
            filled.saturating_sub(1).min(dw.saturating_sub(1)),
            dh - 1,
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Rössler Attractor
//    dx/dt = −y − z
//    dy/dt = x + ay          a = 0.2
//    dz/dt = b + z(x − c)   b = 0.2, c = 5.7
//
//    Progress reveals orbit length. Time phase-shifts the (x,y) projection.
// ---------------------------------------------------------------------------

struct RosslerAttractor;

impl ProgressStyle for RosslerAttractor {
    fn name(&self) -> &str {
        "rossler"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Rössler spiral band: a=0.2 b=0.2 c=5.7 — single spiralling lobe revealed by progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let a: f32 = 0.2;
        let b: f32 = 0.2;
        let c: f32 = 5.7;
        let dt: f32 = 0.025;

        let n_total = 2400usize;
        let n_draw = (ctx.eased * n_total as f32).max(1.0) as usize;

        // Known attractor x,y range ≈ [-11, 11] × [-11, 11].
        let xy_min = -13.0_f32;
        let xy_max = 13.0_f32;

        let angle = ctx.time * 0.2;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let mut x: f32 = 1.0;
        let mut y: f32 = 0.0;
        let mut z: f32 = 0.0;

        // Burn-in.
        for _ in 0..200 {
            let dx = -y - z;
            let dy = x + a * y;
            let dz = b + z * (x - c);
            x += dx * dt;
            y += dy * dt;
            z += dz * dt;
        }

        for _ in 0..n_draw.min(n_total) {
            let dx = -y - z;
            let dy = x + a * y;
            let dz = b + z * (x - c);
            x += dx * dt;
            y += dy * dt;
            z += dz * dt;

            // Rotate in (x,y).
            let rx = x * cos_a - y * sin_a;
            let ry = x * sin_a + y * cos_a;

            let norm_x = (rx - xy_min) / (xy_max - xy_min);
            let norm_y = (ry - xy_min) / (xy_max - xy_min);

            let px = (norm_x.clamp(0.0, 1.0) * (dw - 1) as f32) as i32;
            let py = ((1.0 - norm_y.clamp(0.0, 1.0)) * (dh - 1) as f32) as i32;
            draw::dot_i(grid, px, py);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Clifford Attractor (IFS)
//    x_{n+1} = sin(a·y) + c·cos(a·x)
//    y_{n+1} = sin(b·x) + d·cos(b·y)
//
//    a,b animated with time; c,d fixed. 3000 iterations, eased fraction shown.
// ---------------------------------------------------------------------------

struct CliffordAttractor;

impl ProgressStyle for CliffordAttractor {
    fn name(&self) -> &str {
        "clifford"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Clifford IFS: sin/cos parameter orbit — params a,b slowly drift with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Animate a and b in ranges that keep the attractor bounded and chaotic.
        let a = -1.7 + 0.4 * (ctx.time * 0.17).sin();
        let b = 1.8 + 0.3 * (ctx.time * 0.13).cos();
        let c = -1.9_f32;
        let d = -0.4_f32;

        let n_total = 3000usize;
        let n_draw = (ctx.eased * n_total as f32).max(1.0) as usize;

        // Clifford attractor bounded by approx. ±2.
        let xy_min = -2.5_f32;
        let xy_max = 2.5_f32;
        let range = xy_max - xy_min;

        let mut x: f32 = 0.1;
        let mut y: f32 = 0.0;

        for _ in 0..n_draw.min(n_total) {
            let xn = (a * y).sin() + c * (a * x).cos();
            let yn = (b * x).sin() + d * (b * y).cos();
            x = xn;
            y = yn;

            let norm_x = (x - xy_min) / range;
            let norm_y = (y - xy_min) / range;
            let px = (norm_x.clamp(0.0, 1.0) * (dw - 1) as f32) as i32;
            let py = ((1.0 - norm_y.clamp(0.0, 1.0)) * (dh - 1) as f32) as i32;
            draw::dot_i(grid, px, py);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. De Jong Attractor (IFS)
//    x_{n+1} = sin(a·y) − cos(b·x)
//    y_{n+1} = sin(c·x) − cos(d·y)
//
//    All four params animated slowly with time.
// ---------------------------------------------------------------------------

struct DeJongAttractor;

impl ProgressStyle for DeJongAttractor {
    fn name(&self) -> &str {
        "de-jong"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "De Jong IFS: sin−cos iterated map — all four params drift with time for morphing forms"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let t = ctx.time;
        let a = -2.0 + 0.5 * (t * 0.11).sin();
        let b = -2.0 + 0.4 * (t * 0.07).cos();
        let c = 1.2 + 0.5 * (t * 0.09).sin();
        let d = 2.0 + 0.3 * (t * 0.13).cos();

        let n_total = 3000usize;
        let n_draw = (ctx.eased * n_total as f32).max(1.0) as usize;

        let xy_min = -2.5_f32;
        let xy_max = 2.5_f32;
        let range = xy_max - xy_min;

        let mut x: f32 = 0.1;
        let mut y: f32 = 0.1;

        for _ in 0..n_draw.min(n_total) {
            let xn = (a * y).sin() - (b * x).cos();
            let yn = (c * x).sin() - (d * y).cos();
            x = xn;
            y = yn;

            let norm_x = (x - xy_min) / range;
            let norm_y = (y - xy_min) / range;
            let px = (norm_x.clamp(0.0, 1.0) * (dw - 1) as f32) as i32;
            let py = ((1.0 - norm_y.clamp(0.0, 1.0)) * (dh - 1) as f32) as i32;
            draw::dot_i(grid, px, py);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Hénon Map
//    x_{n+1} = 1 − a·x² + y    a = 1.4
//    y_{n+1} = b·x              b = 0.3
//
//    Classic banana-shaped strange attractor. Progress = steps drawn.
//    Time shifts the plotting window to reveal hidden structure.
// ---------------------------------------------------------------------------

struct HenonMap;

impl ProgressStyle for HenonMap {
    fn name(&self) -> &str {
        "henon"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Hénon map: a=1.4 b=0.3 — banana strange attractor self-similar at every scale"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let a: f32 = 1.4;
        let b: f32 = 0.3;

        let n_total = 2800usize;
        let n_draw = (ctx.eased * n_total as f32).max(1.0) as usize;

        // Known attractor bounds: x ∈ [−1.33, 1.33], y ∈ [−0.43, 0.43].
        // Add margin: ±1.5, ±0.5.
        let x_min = -1.5_f32;
        let x_max = 1.5_f32;
        let y_min = -0.5_f32;
        let y_max = 0.5_f32;

        let mut x: f32 = 0.1;
        let mut y: f32 = 0.1;

        // Small burn-in.
        for _ in 0..50 {
            let xn = 1.0 - a * x * x + y;
            let yn = b * x;
            x = xn;
            y = yn;
        }

        for _ in 0..n_draw.min(n_total) {
            let xn = 1.0 - a * x * x + y;
            let yn = b * x;
            x = xn;
            y = yn;

            // Bail out if the orbit escaped.
            if x.abs() > 10.0 || y.abs() > 10.0 {
                break;
            }

            let norm_x = (x - x_min) / (x_max - x_min);
            let norm_y = (y - y_min) / (y_max - y_min);
            let px = (norm_x.clamp(0.0, 1.0) * (dw - 1) as f32) as i32;
            let py = ((1.0 - norm_y.clamp(0.0, 1.0)) * (dh - 1) as f32) as i32;
            draw::dot_i(grid, px, py);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Logistic Map Bifurcation Diagram
//    x_{n+1} = r · x · (1 − x),   x₀ = 0.5
//
//    The x-axis is the bar: r swept from 2.8 to 4.0 left→right.
//    ctx.eased gates how far right r has been revealed.
//    At each r column we iterate 300 steps (discard) + 80 (plot).
//    The y-axis is x (attractor values). Time shifts x₀ slightly.
//
//    This is the most gorgeous bar — the entire period-doubling cascade
//    is drawn column by column as progress advances.
// ---------------------------------------------------------------------------

struct LogisticBifurcation;

impl ProgressStyle for LogisticBifurcation {
    fn name(&self) -> &str {
        "logistic-bifurcation"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Logistic map bifurcation diagram r∈[2.8,4.0] — period-doubling route to chaos column by column"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let r_min: f32 = 2.8;
        let r_max: f32 = 4.0;

        // How many dot-columns to reveal (controlled by eased progress).
        let reveal_cols = (ctx.eased * dw as f32).round() as usize;

        // x₀ drifts very slowly with time (barely visible — keeps it alive).
        let x0 = 0.5 + 0.01 * (ctx.time * 0.05).sin();

        for col in 0..reveal_cols.min(dw) {
            let r = r_min + (col as f32 / dw.saturating_sub(1).max(1) as f32) * (r_max - r_min);

            let mut x = x0.clamp(0.01, 0.99);

            // Discard transient.
            for _ in 0..300 {
                x = r * x * (1.0 - x);
            }

            // Plot attractor.
            for _ in 0..80 {
                x = r * x * (1.0 - x);
                // x is in [0,1]; map to dot row.
                let row = ((1.0 - x) * (dh - 1) as f32).round() as usize;
                if row < dh {
                    draw::dot(grid, col, row);
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Double Pendulum Trace
//    Lagrangian equations (equal masses m=1, equal lengths l=1, g=9.81):
//
//    θ̈₁ = [−g(2m)sinθ₁ − mg·sin(θ₁−2θ₂) − 2sin(θ₁−θ₂)·m(θ̇₂²l+θ̇₁²l·cos(θ₁−θ₂))]
//           / [l(2m − m·cos(2θ₁−2θ₂))]
//
//    θ̈₂ = [2sin(θ₁−θ₂)(θ̇₁²l·2m + g·2m·cosθ₁ + θ̇₂²l·m·cos(θ₁−θ₂))]
//           / [l(2m − m·cos(2θ₁−2θ₂))]
//
//    Integrated with RK4 for accuracy. Progress reveals the chaotic tip trace.
// ---------------------------------------------------------------------------

struct DoublePendulum;

impl ProgressStyle for DoublePendulum {
    fn name(&self) -> &str {
        "double-pendulum"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Double pendulum chaotic trace — RK4 Lagrangian integration, tip path revealed by progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let g: f32 = 9.81;
        let dt: f32 = 0.02;

        let n_total: usize = 2000;
        let n_draw = (ctx.eased * n_total as f32).max(1.0) as usize;

        // State: [θ₁, ω₁, θ₂, ω₂]
        let mut th1: f32 = PI / 2.0 + 0.05 * (ctx.time * 0.001).sin();
        let mut om1: f32 = 0.0;
        let mut th2: f32 = PI + 0.03;
        let mut om2: f32 = 0.0;

        // The tip of the second rod (length 1+1=2 from pivot) traces a
        // path in Cartesian space. Scale so it fits in ±2 units → dot grid.
        let scale = 2.0_f32; // max extent (full length of both rods)

        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;

        let deriv = |th1: f32, om1: f32, th2: f32, om2: f32| -> (f32, f32, f32, f32) {
            let delta = th2 - th1;
            let denom = 2.0 - (2.0 * delta).cos(); // (2m - m·cos(2θ₁−2θ₂)) / m
            let denom = if denom.abs() < 1e-6 {
                1e-6_f32.copysign(denom)
            } else {
                denom
            };

            let dom1 = (-g * 2.0 * th1.sin()
                - g * delta.cos() * 2.0 * th2.sin() // approx — simplified form
                - 2.0 * delta.sin() * (om2 * om2 + om1 * om1 * delta.cos()))
                / denom;

            let dom2 = (2.0
                * delta.sin()
                * (om1 * om1 * 2.0 + g * 2.0 * th1.cos() + om2 * om2 * delta.cos()))
                / denom;

            (om1, dom1, om2, dom2)
        };

        for i in 0..n_draw.min(n_total) {
            // RK4 step.
            let (k1a, k1b, k1c, k1d) = deriv(th1, om1, th2, om2);
            let (k2a, k2b, k2c, k2d) = deriv(
                th1 + k1a * dt / 2.0,
                om1 + k1b * dt / 2.0,
                th2 + k1c * dt / 2.0,
                om2 + k1d * dt / 2.0,
            );
            let (k3a, k3b, k3c, k3d) = deriv(
                th1 + k2a * dt / 2.0,
                om1 + k2b * dt / 2.0,
                th2 + k2c * dt / 2.0,
                om2 + k2d * dt / 2.0,
            );
            let (k4a, k4b, k4c, k4d) = deriv(
                th1 + k3a * dt,
                om1 + k3b * dt,
                th2 + k3c * dt,
                om2 + k3d * dt,
            );

            th1 += dt / 6.0 * (k1a + 2.0 * k2a + 2.0 * k3a + k4a);
            om1 += dt / 6.0 * (k1b + 2.0 * k2b + 2.0 * k3b + k4b);
            th2 += dt / 6.0 * (k1c + 2.0 * k2c + 2.0 * k3c + k4c);
            om2 += dt / 6.0 * (k1d + 2.0 * k2d + 2.0 * k3d + k4d);

            // Tip position (second bob).
            let tip_x = th1.sin() + th2.sin();
            let tip_y = -(th1.cos() + th2.cos()); // y positive downward

            // Plot every step.
            let _ = i; // suppress unused warning
            let px = (cx + tip_x / scale * cx) as i32;
            let py = (cy + tip_y / scale * cy) as i32;
            draw::dot_i(grid, px, py);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Standard Map (Chirikov–Taylor map)
//    p_{n+1} = p_n + K·sin(θ_n)   (mod 2π)
//    θ_{n+1} = θ_n + p_{n+1}      (mod 2π)
//
//    K = stochasticity parameter. Progress ramps K from 0 → 4π.
//    Multiple orbits launched from a grid of initial conditions.
//    Time slowly shifts the phase-space viewport.
// ---------------------------------------------------------------------------

struct StandardMap;

impl ProgressStyle for StandardMap {
    fn name(&self) -> &str {
        "standard-map"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Chirikov–Taylor standard map — K grows with progress, KAM tori shatter into chaos"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // K = stochasticity. K=0 → all circles. K≥4 → fully chaotic.
        let k = ctx.eased * 4.0 * PI;

        // Launch orbits from a small grid of initial conditions.
        let n_seeds = 8usize;
        let n_iter = 200usize;

        for si in 0..n_seeds {
            for sj in 0..n_seeds {
                let mut theta = 2.0 * PI * si as f32 / n_seeds as f32;
                let mut p = 2.0 * PI * sj as f32 / n_seeds as f32;

                // Small time-based phase shift.
                theta += ctx.time * 0.04;

                for _ in 0..n_iter {
                    p = (p + k * theta.sin()).rem_euclid(2.0 * PI);
                    theta = (theta + p).rem_euclid(2.0 * PI);

                    let norm_x = theta / (2.0 * PI);
                    let norm_y = p / (2.0 * PI);
                    let px = (norm_x * (dw - 1) as f32) as i32;
                    let py = ((1.0 - norm_y) * (dh - 1) as f32) as i32;
                    draw::dot_i(grid, px, py);
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Gingerbreadman Map
//    x_{n+1} = 1 − y + |x|
//    y_{n+1} = x
//
//    Triangular symmetric strange attractor. Progress seeds more points;
//    time slowly rotates the view to reveal self-similar structure.
// ---------------------------------------------------------------------------

struct Gingerbreadman;

impl ProgressStyle for Gingerbreadman {
    fn name(&self) -> &str {
        "gingerbreadman"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Gingerbreadman map: x←1−y+|x|, y←x — fractal triangular symmetry, orbits seeded by progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let n_seeds_max = 12usize;
        let n_seeds = (ctx.eased * n_seeds_max as f32).max(1.0) as usize;
        let n_iter = 200usize;

        // Typical orbit extent ≈ ±100 for various seeds; normalize.
        let extent = 120.0_f32;

        let angle = ctx.time * 0.1;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;

        for si in 0..n_seeds.min(n_seeds_max) {
            // Deterministic seed positions.
            let h1 = hash(si as u32 * 17 + 3);
            let h2 = hash(si as u32 * 31 + 7);
            let mut x = (h1 % 40) as f32 - 20.0;
            let mut y = (h2 % 40) as f32 - 20.0;

            for _ in 0..n_iter {
                let xn = 1.0 - y + x.abs();
                let yn = x;
                x = xn;
                y = yn;

                // Rotate.
                let rx = x * cos_a - y * sin_a;
                let ry = x * sin_a + y * cos_a;

                let px = (cx + rx / extent * cx) as i32;
                let py = (cy + ry / extent * cy) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Duffing Oscillator Phase Portrait
//     ẋ = y
//     ẏ = −δy + αx − βx³ + γcos(ωt_sim)
//
//     δ=0.2, α=1.0, β=1.0, γ=0.3, ω=1.2 (periodic driving).
//     Plot (x, y) = (position, velocity) phase portrait.
//     Progress controls how many trajectory steps are drawn.
//     ctx.time offsets the driving phase, ctx.eased gates steps.
// ---------------------------------------------------------------------------

struct DuffingOscillator;

impl ProgressStyle for DuffingOscillator {
    fn name(&self) -> &str {
        "duffing"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Duffing oscillator phase portrait: ẏ=−δy+αx−βx³+γcos(ωt) — chaotic fractal basin boundary"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let delta: f32 = 0.2;
        let alpha: f32 = 1.0;
        let beta: f32 = 1.0;
        let gamma: f32 = 0.3;
        let omega: f32 = 1.2;
        let dt: f32 = 0.02;

        let n_total: usize = 2500;
        let n_draw = (ctx.eased * n_total as f32).max(1.0) as usize;

        // Phase portrait fits in roughly x∈[−1.5,1.5], y∈[−1.5,1.5].
        let x_range = 2.0_f32;
        let y_range = 2.0_f32;

        let mut x: f32 = 1.0;
        let mut y: f32 = 0.0;
        // Driving phase starts offset by ctx.time.
        let mut t_sim: f32 = ctx.time * 0.5;

        for _ in 0..n_draw.min(n_total) {
            let force = gamma * (omega * t_sim).cos();
            let ax = y;
            let ay = -delta * y + alpha * x - beta * x * x * x + force;

            x += ax * dt;
            y += ay * dt;
            t_sim += dt;

            let norm_x = (x + x_range) / (2.0 * x_range);
            let norm_y = (y + y_range) / (2.0 * y_range);
            let px = (norm_x.clamp(0.0, 1.0) * (dw - 1) as f32) as i32;
            let py = ((1.0 - norm_y.clamp(0.0, 1.0)) * (dh - 1) as f32) as i32;
            draw::dot_i(grid, px, py);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Tinkerbell Map
//     x_{n+1} = x² − y² + a·x + b·y
//     y_{n+1} = 2xy + c·x + d·y
//
//     a=0.9, b=−0.6013, c=2.0, d=0.5
//     Named for the fractal boundary of its basin of attraction.
//     Progress controls orbit length; time animates parameter a slightly.
// ---------------------------------------------------------------------------

struct TinkerbellMap;

impl ProgressStyle for TinkerbellMap {
    fn name(&self) -> &str {
        "tinkerbell"
    }
    fn theme(&self) -> &str {
        "chaos"
    }
    fn describe(&self) -> &str {
        "Tinkerbell map: a=0.9 b=−0.6013 c=2.0 d=0.5 — fractal basin boundary fairy-dust scatter"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let a: f32 = 0.9 + 0.05 * (ctx.time * 0.08).sin();
        let b: f32 = -0.6013;
        let c: f32 = 2.0;
        let d: f32 = 0.5;

        let n_total = 3000usize;
        let n_draw = (ctx.eased * n_total as f32).max(1.0) as usize;

        // Known attractor extent: x ∈ [−0.7, 0.6], y ∈ [−0.7, 0.4].
        // Use ±1.5 margin to be safe.
        let x_min = -1.5_f32;
        let x_max = 1.5_f32;
        let y_min = -1.5_f32;
        let y_max = 1.5_f32;

        let mut x: f32 = -0.72;
        let mut y: f32 = -0.64;

        for _ in 0..n_draw.min(n_total) {
            let xn = x * x - y * y + a * x + b * y;
            let yn = 2.0 * x * y + c * x + d * y;
            x = xn;
            y = yn;

            // Escape detection.
            if x.abs() > 5.0 || y.abs() > 5.0 {
                break;
            }

            let norm_x = (x - x_min) / (x_max - x_min);
            let norm_y = (y - y_min) / (y_max - y_min);
            let px = (norm_x.clamp(0.0, 1.0) * (dw - 1) as f32) as i32;
            let py = ((1.0 - norm_y.clamp(0.0, 1.0)) * (dh - 1) as f32) as i32;
            draw::dot_i(grid, px, py);
        }

        Ok(())
    }
}
