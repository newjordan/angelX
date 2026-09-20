//! Physics / mechanics progress bars for dotmax.
//!
//! Every bar in this theme is driven by real physical equations — no faked
//! motion. `ctx.eased` is the canonical "how far through" parameter; `ctx.time`
//! advances animation so bars live independently of progress.
//!
//! ## Styles (11 total)
//!
//! | Name | Physics | Key equation |
//! |------|---------|-------------|
//! | `projectile` | Projectile motion | y = v·sinθ·t − ½g·t²,  x = v·cosθ·t |
//! | `pendulum` | Simple pendulum SHM | θ(t) = θ₀·cos(√(g/L)·t) |
//! | `mass-spring` | Mass-spring SHM | x = A·cos(ωt),  spring coil drawn |
//! | `newtons-cradle` | Newton's cradle | elastic collision, momentum swap |
//! | `orbital` | Kepler orbit | ellipse, equal-area sweeping |
//! | `terminal-velocity` | Drag / terminal vel. | v = v_t·(1 − e^(−t/τ)) |
//! | `elastic-collision` | 1D elastic collision | two carts exchange momentum |
//! | `gravity-well` | 1/r potential well | funnel lines, spiral in-fall |
//! | `light-cone` | Special relativity | 45° worldlines, light cone |
//! | `double-slit` | Wave-particle duality | fringe pattern accumulates |
//! | `doppler` | Doppler effect | compressed / expanded wavefronts |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// All styles in the `physics` theme.
///
/// Returns 11 bars each implementing a distinct branch of classical or modern
/// physics, safe to render from 1×1 cells to 80×8 or larger.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Projectile),
        Box::new(Pendulum),
        Box::new(MassSpring),
        Box::new(NewtonsCradle),
        Box::new(Orbital),
        Box::new(TerminalVelocity),
        Box::new(ElasticCollision),
        Box::new(GravityWell),
        Box::new(LightCone),
        Box::new(DoubleSlit),
        Box::new(Doppler),
    ]
}

// ---------------------------------------------------------------------------
// Helper: draw a line between two dot-space points (Bresenham)
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
// 1. Projectile motion
//    x = v·cosθ·τ,  y = v·sinθ·τ − ½g·τ²
//    eased sweeps τ from 0 → T_land (time of flight = 2v·sinθ/g).
//    The full parabolic arc draws out dot by dot; a filled baseline tracks range.
// ---------------------------------------------------------------------------
struct Projectile;
impl ProgressStyle for Projectile {
    fn name(&self) -> &str {
        "projectile"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Projectile motion: parabolic arc y=v·sinθ·t−½g·t² draws out as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        // Physics params (arbitrary units scaled to dot space)
        let theta: f32 = PI / 4.0; // 45° launch angle
        let g: f32 = 1.0; // gravitational acc (normalised)
        let v: f32 = 1.0; // launch speed (normalised)
        let t_flight = 2.0 * v * theta.sin() / g; // total flight time
        let x_range = v * theta.cos() * t_flight; // total horizontal range

        // Number of arc-steps visible = eased fraction of flight
        let arc_steps = ((w * 4).max(32)) as usize;
        let t_now = ctx.eased * t_flight; // current simulation time

        // Draw the ground baseline
        let ground = h.saturating_sub(1);
        draw::hline(grid, 0, w.saturating_sub(1), ground);

        // Draw the arc up to t_now
        let mut prev: Option<(i32, i32)> = None;
        for step in 0..=arc_steps {
            let frac = step as f32 / arc_steps as f32;
            let t = frac * t_now;
            // x and y in physics units → dot space
            let px = (v * theta.cos() * t / x_range.max(1e-6) * wf) as i32;
            // y is up in physics, down in screen, origin at ground
            let py_phys = v * theta.sin() * t - 0.5 * g * t * t;
            let max_y = 0.5 * v * v * theta.sin() * theta.sin() / g;
            let py = (ground as f32 - py_phys / max_y.max(1e-6) * (hf - 2.0)) as i32;
            let py = py.clamp(0, h as i32 - 1);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, px, py);
            }
            draw::dot_i(grid, px, py);
            prev = Some((px, py));
        }

        // Filled range bar at ground: how far the projectile has travelled
        let fill_x = (ctx.eased * wf) as usize;
        draw::hline(grid, 0, fill_x.min(w.saturating_sub(1)), ground);

        // Mark launch point
        draw::dot_i(grid, 0, ground as i32);

        // Tint the filled arc region
        let (cw, ch) = grid.dimensions();
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

// ---------------------------------------------------------------------------
// 2. Pendulum (simple SHM)
//    θ(t) = θ₀·cos(ω·t)   where ω = √(g/L)
//    eased sets θ₀ (max swing amplitude, 0 → π/3).
//    time drives ω·t: the bob swings; its trail is drawn.
//    Pivot is at top-centre; rod and bob rendered each frame.
// ---------------------------------------------------------------------------
struct Pendulum;
impl ProgressStyle for Pendulum {
    fn name(&self) -> &str {
        "pendulum"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Pendulum SHM: θ(t)=θ₀·cos(√(g/L)·t) — amplitude grows with progress, bob swings with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        // Pivot at top-centre
        let pivot_x = (wf / 2.0) as i32;
        let pivot_y = 0_i32;

        // Rod length spans ~80% of height
        let rod_len = (hf * 0.80).max(2.0);

        // Amplitude and angular frequency
        let theta0 = ctx.eased * (PI / 3.0); // max swing 0..60°
        let omega = (9.8_f32 / rod_len).sqrt() * 4.0; // visual speed-up

        // Current angle (oscillates with time)
        let theta = theta0 * (omega * ctx.time).cos();

        // Bob position
        let bob_x = pivot_x + (rod_len * theta.sin()) as i32;
        let bob_y = pivot_y + (rod_len * theta.cos()) as i32;

        // Draw rod
        line(grid, pivot_x, pivot_y, bob_x, bob_y);

        // Draw pivot dot
        draw::dot_i(grid, pivot_x, pivot_y);

        // Draw bob (small filled square 2×2 dots)
        for dy in -1_i32..=1 {
            for dx in -1_i32..=1 {
                draw::dot_i(grid, bob_x + dx, bob_y + dy);
            }
        }

        // Draw the arc the bob traces: positions at many theta values
        let arc_steps = w.max(2);
        let mut prev: Option<(i32, i32)> = None;
        for s in 0..=arc_steps {
            let th = if arc_steps == 0 {
                0.0
            } else {
                theta0 * ((s as f32 / arc_steps as f32) * 2.0 - 1.0)
            };
            let ax = pivot_x + (rod_len * th.sin()) as i32;
            let ay = pivot_y + (rod_len * th.cos()) as i32;
            draw::dot_i(grid, ax, ay);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, ax, ay);
            }
            prev = Some((ax, ay));
        }

        // Draw horizontal equilibrium line at rod bottom
        let eq_y = (pivot_y + rod_len as i32).min(h as i32 - 1);
        draw::hline(grid, 0, w.saturating_sub(1), eq_y as usize);

        // Tint: colour spread across the swing width
        let (cw, ch) = grid.dimensions();
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

// ---------------------------------------------------------------------------
// 3. Mass-spring SHM
//    x(t) = A·cos(ωt + φ)   where ω = √(k/m)
//    eased sets A (amplitude, 0 → half-bar).  time drives ωt.
//    The spring coil is drawn as a zigzag between the wall and the mass.
// ---------------------------------------------------------------------------
struct MassSpring;
impl ProgressStyle for MassSpring {
    fn name(&self) -> &str {
        "mass-spring"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Mass-spring SHM: x=A·cos(ωt) — coil compresses/extends, amplitude grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        let omega = 3.0_f32; // angular frequency (visual)
        let amp_max = wf * 0.35;
        let amp = ctx.eased * amp_max;
        let equilibrium_x = wf * 0.55; // equilibrium position (dot space)
        let mass_x = equilibrium_x + amp * (omega * ctx.time).cos();
        let clamp_lo = 2.0_f32;
        let clamp_hi = (wf - 3.0).max(clamp_lo);
        let mass_x = mass_x.clamp(clamp_lo, clamp_hi);
        let mid_y = (hf / 2.0) as i32;

        // Wall on the left (2 dots wide)
        draw::vline(grid, 0, 0, h.saturating_sub(1));
        draw::vline(grid, 1, 0, h.saturating_sub(1));

        // Spring coil: zigzag from x=2 to mass_x-3
        let spring_start = 2_i32;
        let spring_end = (mass_x as i32 - 3).max(spring_start + 1);
        let spring_len = (spring_end - spring_start).max(1);
        let coil_count = 6_i32;
        let half_amp = (hf * 0.25).max(1.0) as i32;

        for c in 0..coil_count {
            let x0 = spring_start + spring_len * c / coil_count;
            let x1 = spring_start + spring_len * (c + 1) / coil_count;
            let y0 = mid_y + if c % 2 == 0 { half_amp } else { -half_amp };
            let y1 = mid_y + if c % 2 == 0 { -half_amp } else { half_amp };
            line(grid, x0, y0, x1, y1);
        }

        // Mass block (3×3 dot rectangle)
        let mx = mass_x as i32;
        for dy in -2_i32..=2 {
            for dx in 0_i32..=3 {
                draw::dot_i(grid, mx + dx, mid_y + dy);
            }
        }

        // Equilibrium marker (dashed vertical line at equilibrium_x)
        let eq_x = equilibrium_x as usize;
        for y in (0..h).step_by(2) {
            draw::dot(grid, eq_x.min(w.saturating_sub(1)), y);
        }

        // Tint: left of mass = compressed, right = extended
        let (cw, ch) = grid.dimensions();
        let mass_cell = (mass_x / 2.0) as usize;
        let eq_cell = (equilibrium_x / 2.0) as usize;
        let (lo, hi) = if mass_cell < eq_cell {
            (mass_cell, eq_cell)
        } else {
            (eq_cell, mass_cell)
        };
        for cx in lo..hi.min(cw) {
            let t = cx as f32 / cw as f32;
            let col = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        // Also tint filled portion from left by eased
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let t = cx as f32 / cw as f32;
            let col = ctx.palette.sample(t * 0.5); // blend, don't double-tint same colour
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Newton's cradle
//    5 balls on strings; only the end balls swing (elastic collision).
//    Left ball swings in with angle θ_L(t), right swings out θ_R(t).
//    Period T = 2π√(L/g); eased controls swing amplitude θ₀.
//    Momentum is conserved: when left ball hits, right ball leaves at same speed.
// ---------------------------------------------------------------------------
struct NewtonsCradle;
impl ProgressStyle for NewtonsCradle {
    fn name(&self) -> &str {
        "newtons-cradle"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Newton's cradle: elastic momentum transfer — end balls trade arcs every half-period"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        let n_balls: usize = 5;
        let ball_r = (wf / (n_balls as f32 * 3.0)).max(1.0) as i32;
        let spacing = wf / (n_balls as f32 + 1.0);
        let rod_top = (hf * 0.05) as i32;
        let rod_len = (hf * 0.65).max(4.0);
        // Equilibrium bob y (at rest)
        let bob_y_eq = rod_top + rod_len as i32;

        // Angular frequency from pendulum physics (visual scale)
        let omega = 3.5_f32;
        let theta0 = ctx.eased * (PI / 5.0); // max swing angle

        // Phase of the cradle: cos(ω·t) drives the END balls
        let phase = (omega * ctx.time).cos();
        // When phase > 0: left ball is out, right ball is at rest
        // When phase < 0: right ball is out, left ball is at rest
        let left_theta = if phase >= 0.0 { theta0 * phase } else { 0.0 };
        let right_theta = if phase < 0.0 { -theta0 * phase } else { 0.0 };

        // Draw horizontal support bar
        let bar_y = rod_top;
        draw::hline(grid, 0, w.saturating_sub(1), bar_y as usize);

        // Draw each ball
        for i in 0..n_balls {
            let pivot_x = (spacing * (i as f32 + 1.0)) as i32;
            let theta = if i == 0 {
                left_theta // left end swings
            } else if i == n_balls - 1 {
                -right_theta // right end swings (opposite direction)
            } else {
                0.0 // middle balls stationary
            };

            let bob_x = pivot_x + (rod_len * theta.sin()) as i32;
            let bob_y = rod_top + (rod_len * theta.cos()) as i32;

            // Rod
            line(grid, pivot_x, rod_top, bob_x, bob_y);

            // Bob (circle approximated by outline)
            for step in 0..16_i32 {
                let a = step as f32 / 16.0 * 2.0 * PI;
                let bx = bob_x + (ball_r as f32 * a.cos()) as i32;
                let by = bob_y + (ball_r as f32 * a.sin()) as i32;
                draw::dot_i(grid, bx, by);
            }
            // Filled centre of bob
            draw::dot_i(grid, bob_x, bob_y);
        }

        // Ground reference line
        let ground = (bob_y_eq + ball_r + 1).min(h as i32 - 1);
        draw::hline(grid, 0, w.saturating_sub(1), ground as usize);

        // Tint: gradient across the width keyed by eased
        let (cw, ch) = grid.dimensions();
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

// ---------------------------------------------------------------------------
// 5. Orbital (Kepler's laws)
//    Elliptical orbit: x = a·cos(E), y = b·sin(E)
//    where a = semi-major, b = a·√(1−e²), eccentricity e = 0.6.
//    Eccentric anomaly E(t) solved via Kepler's equation: E − e·sin(E) = M
//    Mean anomaly M = 2π·(time mod T)/T.
//    eased draws fraction of the orbit; planet sweeps equal areas in equal times.
// ---------------------------------------------------------------------------
struct Orbital;
impl ProgressStyle for Orbital {
    fn name(&self) -> &str {
        "orbital"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Kepler orbit: elliptical path, equal-area sweeping — planet position from mean anomaly"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        let cx = wf / 2.0;
        let cy = hf / 2.0;
        let a = (wf / 2.0 - 2.0).max(2.0); // semi-major axis in dots
        let ecc = 0.60_f32; // eccentricity
        let b = a * (1.0 - ecc * ecc).sqrt(); // semi-minor axis
        let focus_offset = a * ecc; // star is at focus

        // Draw the full ellipse outline
        let orbit_steps = (w * 4).max(64);
        let mut prev: Option<(i32, i32)> = None;
        for s in 0..=orbit_steps {
            let e_ang = s as f32 / orbit_steps as f32 * 2.0 * PI;
            let px = (cx - focus_offset + a * e_ang.cos()) as i32;
            let py = (cy + b * e_ang.sin()) as i32;
            draw::dot_i(grid, px, py);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, px, py);
            }
            prev = Some((px, py));
        }

        // Star at focus (small cross)
        let star_x = (cx - focus_offset) as i32;
        let star_y = cy as i32;
        draw::dot_i(grid, star_x, star_y);
        draw::dot_i(grid, star_x - 1, star_y);
        draw::dot_i(grid, star_x + 1, star_y);
        draw::dot_i(grid, star_x, star_y - 1);
        draw::dot_i(grid, star_x, star_y + 1);

        // Kepler equation: E − e·sin(E) = M;  solve by Newton-Raphson
        let period = 6.0_f32; // visual period seconds
        let m_anim = 2.0 * PI * (ctx.time % period.max(0.001)) / period.max(0.001);
        let e_anim = {
            let mut e = m_anim;
            for _ in 0..8 {
                e = e - (e - ecc * e.sin() - m_anim) / (1.0 - ecc * e.cos());
            }
            e
        };

        // Planet position from eccentric anomaly (animated)
        let planet_x = (cx - focus_offset + a * e_anim.cos()) as i32;
        let planet_y = (cy + b * e_anim.sin()) as i32;

        // Draw line from focus to planet (radius vector)
        line(grid, star_x, star_y, planet_x, planet_y);

        // Planet dot (3×3)
        for dy in -1_i32..=1 {
            for dx in -1_i32..=1 {
                draw::dot_i(grid, planet_x + dx, planet_y + dy);
            }
        }

        // Shade filled arc from eased (the orbit fraction completed)
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cell_x in 0..filled_cells.min(cw) {
            let t = cell_x as f32 / cw as f32;
            let col = ctx.palette.sample(t);
            for cell_y in 0..ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Terminal velocity
//    An object falling under gravity with drag: F = mg − k·v
//    Solution: v(t) = v_t·(1 − e^(−t/τ))  where v_t = mg/k, τ = m/k
//    Position:  y(t) = v_t·(t − τ·(1 − e^(−t/τ)))
//    eased controls normalised time (0 → T_max = 4τ).
//    Bar fills from the top as the object speeds up, then plateaus.
//    The velocity curve y=v(t) is plotted as a line graph.
// ---------------------------------------------------------------------------
struct TerminalVelocity;
impl ProgressStyle for TerminalVelocity {
    fn name(&self) -> &str {
        "terminal-velocity"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Terminal velocity: v=v_t·(1−e^(−t/τ)) — speed curve fills as object approaches drag limit"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let _hf = h as f32;

        // Physics: τ = 1 (normalised), v_t = 1
        let tau = 1.0_f32;
        let t_max = 4.0 * tau; // x-axis spans 0..4τ

        // Draw axes
        let ax_x = 1_usize; // y-axis (left edge)
        let ax_y = h.saturating_sub(2); // x-axis (bottom)
        draw::vline(grid, ax_x, 0, ax_y);
        draw::hline(grid, ax_x, w.saturating_sub(1), ax_y);

        // Draw the asymptote v = v_t (top of the graph area)
        let top_y = 1_usize;
        for xi in (ax_x..w).step_by(3) {
            if xi + 1 < w {
                draw::dot(grid, xi, top_y);
            }
        }

        // Draw v(t) curve up to eased fraction of t_max
        let t_now = ctx.eased * t_max;
        let graph_h = (ax_y as f32 - top_y as f32).max(1.0);
        let graph_w = (w - ax_x - 1) as f32;

        let mut prev: Option<(i32, i32)> = None;
        let steps = w.max(2);
        for s in 0..=steps {
            let frac = s as f32 / steps as f32;
            let t = frac * t_now;
            let v = 1.0 - (-t / tau).exp(); // v/v_t ∈ [0,1)
            let px = (ax_x as f32 + frac * graph_w * (t_now / t_max)) as i32;
            let py = (ax_y as f32 - v * graph_h) as i32;
            let py = py.clamp(top_y as i32, ax_y as i32);
            draw::dot_i(grid, px, py);
            if let Some((lx, ly)) = prev {
                line(grid, lx, ly, px, py);
            }
            prev = Some((px, py));
        }

        // Falling object: a small square at vertical position y(t) on the right
        let t_now_anim = (ctx.time % t_max.max(0.001)) * 0.8;
        let y_pos_norm = (t_now_anim / tau - (1.0 - (-t_now_anim / tau).exp()))
            / (t_max / tau - 1.0 + (-t_max / tau).exp()).max(1e-6);
        let obj_x = (wf * 0.92) as i32;
        let obj_y = (y_pos_norm.clamp(0.0, 1.0) * (ax_y as f32 - 2.0) + 1.0) as i32;
        for dy in 0_i32..=2 {
            for dx in -1_i32..=1 {
                draw::dot_i(grid, obj_x + dx, obj_y + dy);
            }
        }

        // Tint
        let (cw, ch) = grid.dimensions();
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

// ---------------------------------------------------------------------------
// 7. Elastic collision (1D)
//    Two equal-mass carts on a frictionless track.
//    Before collision: cart A moves right at v, cart B is stationary.
//    After: A stops, B moves right at v.  (Elastic: Δp = 0, ΔKE = 0)
//    eased divides the bar into three phases:
//       [0, 0.4)  → approach
//       [0.4, 0.6) → collision (overlap + rebound flash)
//       [0.6, 1.0] → separation
//    time provides subtle vibration at the collision instant.
// ---------------------------------------------------------------------------
struct ElasticCollision;
impl ProgressStyle for ElasticCollision {
    fn name(&self) -> &str {
        "elastic-collision"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "1D elastic collision: cart A→B, momentum swaps — three phases: approach, impact, separation"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        let mid_y = (hf / 2.0) as i32;
        let cart_w = (wf / 8.0).max(3.0) as i32;
        let cart_h = (hf / 3.0).max(2.0) as i32;
        let track_y = (mid_y + cart_h / 2 + 1).min(h as i32 - 1);

        // Track
        draw::hline(grid, 0, w.saturating_sub(1), track_y as usize);

        // Phase logic
        let phase = ctx.eased;
        let center_x = (wf / 2.0) as i32;

        let (ax, bx) = if phase < 0.4 {
            // Approach: A moves from left, B is stationary at center+cart_w/2
            let a_frac = phase / 0.4;
            let a_x = (a_frac * (wf * 0.4)) as i32;
            let b_x = center_x;
            (a_x, b_x)
        } else if phase < 0.6 {
            // Collision: both at center (slight overlap, vibrate)
            let vib = ((ctx.time * 20.0).sin() * 2.0) as i32;
            (center_x + vib, center_x + vib)
        } else {
            // Separation: A stops at center, B moves right
            let b_frac = (phase - 0.6) / 0.4;
            let a_x = center_x;
            let b_x = center_x + (b_frac * wf * 0.4) as i32;
            (a_x, b_x)
        };

        // Draw cart A (left cart)
        let a_left = ax;
        let a_top = mid_y - cart_h / 2;
        for dy in 0..cart_h {
            for dx in 0..cart_w {
                draw::dot_i(grid, a_left + dx, a_top + dy);
            }
        }

        // Hollow interior of A
        if cart_w > 2 && cart_h > 2 {
            for dy in 1..cart_h - 1 {
                for dx in 1..cart_w - 1 {
                    // don't draw inside — use outline only
                    let _ = (a_left + dx, a_top + dy); // already filled above; we want solid
                }
            }
        }

        // Draw cart B (right cart)
        let b_left = bx;
        let b_top = mid_y - cart_h / 2;
        for dy in 0..cart_h {
            draw::dot_i(grid, b_left, b_top + dy);
            draw::dot_i(grid, b_left + cart_w - 1, b_top + dy);
        }
        for dx in 0..cart_w {
            draw::dot_i(grid, b_left + dx, b_top);
            draw::dot_i(grid, b_left + dx, b_top + cart_h - 1);
        }

        // Velocity arrows: draw rightward arrows above carts
        let arrow_y = (a_top - 2).max(0);
        // Cart A arrow (before collision it moves; after it stops)
        if phase < 0.4 {
            let tail = a_left;
            let head = (a_left + cart_w + 2).min(w as i32 - 1);
            line(grid, tail, arrow_y, head, arrow_y);
            draw::dot_i(grid, head - 1, arrow_y - 1);
            draw::dot_i(grid, head - 1, arrow_y + 1);
        }
        // Cart B arrow (after collision it moves)
        if phase > 0.6 {
            let tail = b_left + cart_w;
            let head = (b_left + cart_w + 4).min(w as i32 - 1);
            line(grid, tail, arrow_y, head, arrow_y);
            draw::dot_i(grid, head - 1, arrow_y - 1);
            draw::dot_i(grid, head - 1, arrow_y + 1);
        }

        // Tint the bar by progress
        let (cw, ch) = grid.dimensions();
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

// ---------------------------------------------------------------------------
// 8. Gravity well (1/r potential)
//    Funnel lines: at radius r from centre, dot-row = y_max·(1 − r_min/r)
//    Plotted as a V-shaped funnel (side profile of a potential well).
//    A test particle spirals inward: r decreases with eased; azimuth from time.
// ---------------------------------------------------------------------------
struct GravityWell;
impl ProgressStyle for GravityWell {
    fn name(&self) -> &str {
        "gravity-well"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Gravity well: 1/r potential funnel in dot-lines, test particle spirals inward with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        // The funnel is drawn as a side-profile: V shape opening upward.
        // At horizontal distance d from centre, depth ∝ 1/d (capped).
        let cx = (wf / 2.0) as i32;
        let top = 0_i32;

        // Draw left and right funnel arms as curved lines y = k/x from centre
        let depth_max = (hf * 0.85).max(2.0);
        let k = depth_max * (wf / 2.0 - 2.0); // curvature constant

        let mut prev_l: Option<(i32, i32)> = None;
        let mut prev_r: Option<(i32, i32)> = None;

        for xi in 1..=(w / 2).max(1) {
            let xf = xi as f32;
            let depth = (k / xf).min(depth_max);
            let py = (top as f32 + hf - 1.0 - depth) as i32;
            let py = py.clamp(top, (h as i32) - 1);

            let left_x = cx - xi as i32;
            let right_x = cx + xi as i32;

            draw::dot_i(grid, right_x, py);
            draw::dot_i(grid, left_x, py);

            if let Some((lx, ly)) = prev_r {
                line(grid, lx, ly, right_x, py);
            }
            if let Some((lx, ly)) = prev_l {
                line(grid, lx, ly, left_x, py);
            }
            prev_r = Some((right_x, py));
            prev_l = Some((left_x, py));
        }

        // Well bottom marker
        let well_bottom = (top as f32 + hf - 1.0) as i32;
        draw::dot_i(grid, cx - 1, well_bottom);
        draw::dot_i(grid, cx, well_bottom);
        draw::dot_i(grid, cx + 1, well_bottom);

        // Spiraling particle: r decreases from max to 0 as eased→1
        let r_max = wf / 2.0 - 2.0;
        let r_now = r_max * (1.0 - ctx.eased);
        let azimuth = ctx.time * 4.0 * PI; // winds around

        // Particle traces a spiral (draw last quarter turn)
        let spiral_steps = 32_usize;
        let mut prev_sp: Option<(i32, i32)> = None;
        for s in 0..spiral_steps {
            let frac = s as f32 / spiral_steps as f32;
            let angle = azimuth - frac * PI * 0.5; // last half-turn
            let r_s = r_now + frac * r_max * 0.25;
            let depth = (k / r_s.max(1.0)).min(depth_max);
            let py_s = (top as f32 + hf - 1.0 - depth) as i32;
            let px_s = cx + (r_s * angle.cos()) as i32;
            let py_s = py_s.clamp(top, h as i32 - 1);
            draw::dot_i(grid, px_s, py_s);
            if let Some((lx, ly)) = prev_sp {
                line(grid, lx, ly, px_s, py_s);
            }
            prev_sp = Some((px_s, py_s));
        }

        // Particle at current position
        let px_now = cx + (r_now * azimuth.cos()) as i32;
        let depth_now = (k / r_now.max(1.0)).min(depth_max);
        let py_now = (top as f32 + hf - 1.0 - depth_now) as i32;
        let py_now = py_now.clamp(top, h as i32 - 1);
        for dy in -1_i32..=1 {
            for dx in -1_i32..=1 {
                draw::dot_i(grid, px_now + dx, py_now + dy);
            }
        }

        // Tint
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cell_x in 0..filled_cells.min(cw) {
            let t = cell_x as f32 / cw as f32;
            let col = ctx.palette.sample(t);
            for cell_y in 0..ch {
                draw::tint_row(grid, cell_y, cell_x, cell_x, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Relativity light cone
//    Spacetime diagram: x-axis = space, y-axis = time (up).
//    Light cone: two 45° lines from the origin event (centre-bottom).
//    Past cone: 45° lines going down.  Future cone going up.
//    A particle worldline: a curved timelike path from bottom to top.
//    eased controls how far up the worldline is drawn.
// ---------------------------------------------------------------------------
struct LightCone;
impl ProgressStyle for LightCone {
    fn name(&self) -> &str {
        "light-cone"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Spacetime light cone: 45° null worldlines + timelike particle path drawn by progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        // Origin event at centre-bottom
        let origin_x = (wf / 2.0) as i32;
        let origin_y = (hf - 2.0) as i32;

        // Future light cone: two 45° lines upward from origin
        // x = origin_x ± Δt  (slope = ±1 in dot-space, scaled to aspect)
        // Dot aspect: 1 cell = 2 dots wide, 4 dots tall → squash x by 2
        let scale_x = 1_i32; // how many x-dots per y-dot of light travel
        for step in 0..h as i32 {
            let dt = step;
            let right_x = origin_x + dt * scale_x;
            let left_x = origin_x - dt * scale_x;
            let cone_y = origin_y - dt;
            if cone_y >= 0 {
                draw::dot_i(grid, right_x, cone_y);
                draw::dot_i(grid, left_x, cone_y);
            }
        }

        // Past light cone: two 45° lines downward (below origin)
        // We clamp — only visible if origin is not at very bottom
        for step in 1..4_i32 {
            let dt = step;
            let right_x = origin_x + dt * scale_x;
            let left_x = origin_x - dt * scale_x;
            let cone_y = origin_y + dt;
            draw::dot_i(grid, right_x, cone_y);
            draw::dot_i(grid, left_x, cone_y);
        }

        // Draw axis labels as dot-marks
        // x-axis (space): horizontal at origin_y
        draw::hline(grid, 0, w.saturating_sub(1), origin_y as usize);
        // t-axis (time): vertical at origin_x
        draw::vline(grid, origin_x as usize, 0, h.saturating_sub(1));

        // Particle worldline: a slightly curved timelike path
        // x(τ) = origin_x + A·sin(ω_p·τ),  y(τ) = origin_y − τ  (moving through time)
        // τ from 0 to eased·(h-2).  Animated lateral drift via ctx.time.
        let amp_p = (wf / 6.0).max(1.0);
        let omega_p = 2.0 * PI / (hf.max(1.0));
        let tau_max = (ctx.eased * (h as f32 - 2.0)) as i32;
        let drift = ctx.time * 0.3; // slow lateral wiggle

        let mut prev_wp: Option<(i32, i32)> = None;
        for tau in 0..=tau_max {
            let tauf = tau as f32;
            let px = (origin_x as f32 + amp_p * (omega_p * tauf + drift).sin()) as i32;
            let py = origin_y as i32 - tau;
            if py >= 0 {
                draw::dot_i(grid, px, py);
                if let Some((lx, ly)) = prev_wp {
                    line(grid, lx, ly, px, py);
                }
                prev_wp = Some((px, py));
            }
        }

        // Particle tip marker
        if let Some((lx, ly)) = prev_wp {
            for dy in -1_i32..=1 {
                for dx in -1_i32..=1 {
                    draw::dot_i(grid, lx + dx, ly + dy);
                }
            }
        }

        // Tint
        let (cw, ch) = grid.dimensions();
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

// ---------------------------------------------------------------------------
// 10. Double-slit interference (wave-particle duality)
//     Two slits at y = h/3 and y = 2h/3.  For each screen position x,
//     intensity I(x) ∝ cos²(π·d·sin(θ)/λ) where sin(θ) ≈ (x−x_screen)/L.
//     eased controls how many particles have accumulated: fringe density
//     scales with eased.  time animates which particles are "in flight".
// ---------------------------------------------------------------------------
struct DoubleSlit;
impl ProgressStyle for DoubleSlit {
    fn name(&self) -> &str {
        "double-slit"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Double-slit interference: fringe pattern accumulates as particle count grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        // Barrier with two slits lives at x = 35% of width
        let barrier_x = (wf * 0.30) as usize;
        // Screen on the right
        let screen_x = (wf * 0.95) as usize;

        // Draw barrier (full vline with gaps for slits)
        let slit1_y = (hf * 0.33) as usize;
        let slit2_y = (hf * 0.67) as usize;
        let slit_h = (hf * 0.08).max(1.0) as usize;
        for y in 0..h {
            // Is this y inside a slit?
            let in_slit1 = y >= slit1_y && y < slit1_y + slit_h;
            let in_slit2 = y >= slit2_y && y < slit2_y + slit_h;
            if !in_slit1 && !in_slit2 {
                draw::dot(grid, barrier_x.min(w.saturating_sub(1)), y);
            }
        }

        // Interference pattern on screen
        // d = slit separation, λ = wavelength (both in dot units)
        let slit_sep = (slit2_y as f32 - slit1_y as f32 + slit_h as f32).max(1.0);
        let lambda = slit_sep * 0.6; // wavelength
        let dist_l = (screen_x as f32 - barrier_x as f32).max(1.0);

        // The fringe pattern: I(y) = cos²(π·d·(y−y_c)/(λ·L))
        let y_c = hf / 2.0;

        for yi in 0..h {
            let yf = yi as f32;
            let sin_t = (yf - y_c) / (dist_l * dist_l + (yf - y_c) * (yf - y_c)).sqrt();
            let phase = PI * slit_sep * sin_t / lambda;
            let intensity = phase.cos() * phase.cos(); // I ∈ [0, 1]
                                                       // Only draw if we have enough "particles" accumulated (ctx.eased)
                                                       // and intensity is above threshold scaled by 1-eased
            let threshold = (1.0 - ctx.eased) * 0.98;
            if intensity > threshold {
                // Thickness proportional to intensity
                let sx = screen_x.min(w.saturating_sub(1));
                draw::dot(grid, sx, yi);
                if intensity > 0.7 && sx > 0 {
                    draw::dot(grid, sx.saturating_sub(1), yi);
                }
            }
        }

        // Animate a particle in flight: travels from source (left) to screen
        let flight = (ctx.time * 0.7).fract();
        let part_x = (flight * screen_x as f32) as i32;
        // Particle starts from one of the slits, quantum-mechanically both
        let slit_cy = hf / 2.0;
        let part_y = slit_cy as i32;
        draw::dot_i(grid, part_x, part_y);
        draw::dot_i(grid, part_x, part_y - 1);
        draw::dot_i(grid, part_x, part_y + 1);

        // Source dot on far left
        draw::vline(grid, 0, h / 3, 2 * h / 3);

        // Tint: fringe region on the right
        let (cw, ch) = grid.dimensions();
        let screen_cell = screen_x / 2;
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let t = cx as f32 / cw as f32;
            let col = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        let _ = screen_cell;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Doppler effect
//     A source moves rightward at speed v_s < c_sound.
//     Wavefronts are circles emitted at intervals T = λ/c.
//     The source position when the nth wavefront was emitted:
//       x_n = v_s · n · T  (moves right over time)
//     Wavefront n at current time has radius r_n = c · (t − n·T).
//     eased → source speed (0 = stationary, 1 = Mach 0.9).
//     time advances so the wavefronts expand visually.
// ---------------------------------------------------------------------------
struct Doppler;
impl ProgressStyle for Doppler {
    fn name(&self) -> &str {
        "doppler"
    }
    fn theme(&self) -> &str {
        "physics"
    }
    fn describe(&self) -> &str {
        "Doppler effect: expanding wavefronts compress ahead/stretch behind a moving source"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let hf = h as f32;

        let c_sound = wf / 3.0; // wave speed in dots/sec
        let v_src = ctx.eased * c_sound * 0.85; // source speed (0 → Mach 0.85)
        let lambda = c_sound * 0.15; // rest wavelength
        let period = lambda / c_sound; // emission period
        let n_waves = 7_usize; // number of wavefronts shown

        // Source current x-position (travels from left, wraps)
        let loop_dur = (n_waves as f32 * period).max(0.001);
        let t_phase = ctx.time % loop_dur;
        let src_x = v_src * t_phase;
        let src_y = hf / 2.0;

        // Draw each wavefront as a circle (dot approximation)
        for n in 0..n_waves {
            let nf = n as f32;
            let t_emit = nf * period; // when this front was emitted
            let age = t_phase - t_emit;
            if age <= 0.0 {
                continue;
            }
            let radius = c_sound * age; // wavefront radius in dots
            let emit_x = v_src * t_emit; // source was here at emission

            if radius < 0.5 {
                continue;
            }

            // Draw circle outline (dot approximation)
            let circ_steps = ((radius * PI * 2.0) as usize + 8).max(8).min(128);
            let mut prev_c: Option<(i32, i32)> = None;
            for s in 0..=circ_steps {
                let angle = s as f32 / circ_steps as f32 * 2.0 * PI;
                let cx = (emit_x + radius * angle.cos()) as i32;
                let cy = (src_y + radius * angle.sin()) as i32;
                draw::dot_i(grid, cx, cy);
                if let Some((lx, ly)) = prev_c {
                    if (cx - lx).abs() + (cy - ly).abs() > 2 {
                        line(grid, lx, ly, cx, cy);
                    }
                }
                prev_c = Some((cx, cy));
            }
        }

        // Draw source (small moving dot)
        let src_ix = src_x as i32;
        let src_iy = src_y as i32;
        for dy in -1_i32..=1 {
            for dx in -1_i32..=1 {
                draw::dot_i(grid, src_ix + dx, src_iy + dy);
            }
        }

        // Direction arrow above source
        let arr_y = (src_iy - 3).max(0);
        let arr_len = (v_src / c_sound * 6.0 + 1.0) as i32;
        line(grid, src_ix, arr_y, src_ix + arr_len, arr_y);
        draw::dot_i(grid, src_ix + arr_len - 1, arr_y - 1);
        draw::dot_i(grid, src_ix + arr_len - 1, arr_y + 1);

        // Tint
        let (cw, ch) = grid.dimensions();
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
