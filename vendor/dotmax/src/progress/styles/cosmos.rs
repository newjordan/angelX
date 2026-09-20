//! Cosmos / deep-space-phenomena progress bars.
//!
//! Ten structurally distinct styles, each modelling a different astrophysical
//! phenomenon: supernova shockwave, pulsar lighthouse, nebula condensation,
//! big-bang expansion, solar flare arc, aurora curtains, meteor shower,
//! total eclipse corona, gravitational-lens Einstein ring, cosmic-web
//! filaments, redshift wavefronts, and a quasar relativistic jet.
//!
//! All bars return `"cosmos"` from `theme()`. `ctx.eased` drives the
//! intensity / progress of the phenomenon; `ctx.time` drives animation.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Deterministic pseudo-random helpers — no external crates.
// ---------------------------------------------------------------------------

/// Cheap integer hash (Knuth multiplicative + avalanche).
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

/// Float in [0, 1) from index `n`.
fn hash_f(n: u32) -> f32 {
    (hash(n) % 10_000) as f32 / 10_000.0
}

// ---------------------------------------------------------------------------
// Public registry
// ---------------------------------------------------------------------------

/// All styles in the `cosmos` theme, in display order.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Supernova),
        Box::new(Pulsar),
        Box::new(NebulaClouds),
        Box::new(BigBang),
        Box::new(SolarFlare),
        Box::new(AuroraCurtains),
        Box::new(MeteorShower),
        Box::new(TotalEclipse),
        Box::new(GravitationalLens),
        Box::new(CosmicWeb),
        Box::new(Redshift),
        Box::new(QuasarJet),
    ]
}

// ---------------------------------------------------------------------------
// 1 — Supernova
// ---------------------------------------------------------------------------
// Structural idea: an expanding shell of dots whose radius = eased * max_r.
// The shell itself is a thin ring (drawn at radius ± 1). A dense core shrinks
// as the shell grows, and a sparse debris halo scatters outside the shell.
// No other style here uses a growing circular shell as its primary element.

/// Supernova: shockwave shell radiates outward; radius tracks eased, debris halos outside.
struct Supernova;
impl ProgressStyle for Supernova {
    fn name(&self) -> &str {
        "supernova"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Supernova shockwave shell explodes outward; radius = eased * max; debris halos outside"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let max_r = cx.min(cy * 2).max(1) as f32;
        let shell_r = (ctx.eased * max_r) as i32;

        // Collapsing stellar core — solid disc that shrinks as shell expands.
        let core_r = ((1.0 - ctx.eased) * max_r * 0.25) as i32;
        for dy in -core_r..=core_r {
            let dx_max = ((core_r * core_r - dy * dy).max(0) as f32).sqrt() as i32;
            for dx in -dx_max..=dx_max {
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Shell ring — draw dots at radius shell_r ± 1.
        if shell_r > 0 {
            let steps = (2.0 * PI * shell_r as f32 * 1.5) as usize;
            let steps = steps.max(8);
            for s in 0..steps {
                let a = s as f32 / steps as f32 * 2.0 * PI;
                for dr in [-1i32, 0, 1] {
                    let r = (shell_r + dr).max(0);
                    let px = cx + (r as f32 * a.cos()) as i32;
                    let py = cy + (r as f32 * a.sin() * 0.5) as i32;
                    // Occasional gap for ring texture
                    if hash((s as u32).wrapping_add(dr.unsigned_abs() * 997)) % 5 != 0 {
                        draw::dot_i(grid, px, py);
                    }
                }
            }
        }

        // Debris: sparse dots between core and shell, animated via time.
        let debris_count = 40u32;
        for i in 0..debris_count {
            let angle = hash_f(i) * 2.0 * PI;
            let r_frac = hash_f(i + 1000);
            let r = (r_frac * shell_r as f32 * 0.9) as i32;
            if r <= core_r {
                continue;
            }
            // Drift outward at speed proportional to position
            let drift = (ctx.time * 0.4 * r_frac) as i32;
            let r_d = (r + drift).min(shell_r);
            let px = cx + (r_d as f32 * angle.cos()) as i32;
            let py = cy + (r_d as f32 * angle.sin() * 0.5) as i32;
            if hash(i.wrapping_add((ctx.time * 3.0) as u32)) % 3 != 0 {
                draw::dot_i(grid, px, py);
            }
        }

        // Tint: hot white core → violet shell
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let dist = (cx_c as f32 - cells_w as f32 / 2.0).abs() / (cells_w as f32 / 2.0).max(1.0);
            let t = 1.0 - dist;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2 — Pulsar
// ---------------------------------------------------------------------------
// Structural idea: two opposed lighthouse beams sweeping around a central dot
// at angular speed driven by time; beam count (blips) accumulates via eased.
// No other style here uses sweeping angular beams as the fill mechanism.

/// Pulsar: two lighthouse beams sweep around a magnetar; blip count = eased * max.
struct Pulsar;
impl ProgressStyle for Pulsar {
    fn name(&self) -> &str {
        "pulsar"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Pulsar lighthouse beams sweep via time; blip tally counts up to eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let max_r = cx.min(cy * 2).max(2) as f32;

        // Spin rate accelerates with eased (faster pulsar = more progress).
        let spin_rate = 2.0 + ctx.eased * 6.0;
        let beam_angle = ctx.time * spin_rate;

        // Two opposed beams (180° apart).
        for beam in 0u32..2 {
            let base_a = beam_angle + beam as f32 * PI;
            // Each beam has angular half-width that narrows as progress grows
            // (pulsar emission cone sharpens at higher spin).
            let half_width = 0.18 - ctx.eased * 0.08;
            let half_width = half_width.max(0.04);

            let beam_steps = (max_r * 1.5) as usize;
            let beam_steps = beam_steps.max(4);
            for s in 0..beam_steps {
                let r = s as f32;
                if r > max_r {
                    break;
                }
                // Fan: sweep a few angles within half_width
                let fan_n = 5usize;
                for f in 0..fan_n {
                    let da =
                        (f as f32 / fan_n.saturating_sub(1).max(1) as f32 - 0.5) * half_width * 2.0;
                    let a = base_a + da;
                    let px = cx + (r * a.cos()) as i32;
                    let py = cy + (r * a.sin() * 0.5) as i32;
                    // Beam fades with distance
                    let fade_thresh = (1.0 - r / max_r) * 3.0;
                    if hash(
                        (s as u32)
                            .wrapping_mul(7)
                            .wrapping_add(f as u32 * 13)
                            .wrapping_add(beam * 997),
                    ) % 4
                        < (fade_thresh * 3.5) as u32 + 1
                    {
                        draw::dot_i(grid, px, py);
                    }
                }
            }
        }

        // Neutron-star core: dense 3×3 cluster
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Blip tally bar: small accumulator along the bottom row of dots.
        // Each blip = one lit dot; max blips = width.
        let max_blips = w.saturating_sub(2);
        let blips = (ctx.eased * max_blips as f32) as usize;
        for b in 0..blips.min(max_blips) {
            draw::dot(grid, b + 1, h.saturating_sub(1));
        }

        // Tint beams with palette
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy_c, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3 — Nebula cloud condensing
// ---------------------------------------------------------------------------
// Structural idea: the entire canvas is subdivided into shaded cells; cell
// shade level grows with eased (gas condenses into denser material).
// Pure shade-glyph approach — no dot-drawing at all in this style.

/// Nebula: gas cloud condenses across the canvas; cell shade = eased density.
struct NebulaClouds;
impl ProgressStyle for NebulaClouds {
    fn name(&self) -> &str {
        "nebula-clouds"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Nebula gas cloud condenses via shade glyphs; density = eased; ripples with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cells_w, cells_h) = grid.dimensions();
        if cells_w == 0 || cells_h == 0 {
            return Ok(());
        }

        // Each cell gets a noise-like density offset from hash + time ripple.
        for cy in 0..cells_h {
            for cx in 0..cells_w {
                let idx = (cy * cells_w + cx) as u32;

                // Static spatial noise [0,1).
                let spatial = hash_f(idx);

                // Time ripple: slow outward pulse from centre.
                let dcx = cx as f32 - cells_w as f32 / 2.0;
                let dcy = (cy as f32 - cells_h as f32 / 2.0) * 2.0;
                let dist = (dcx * dcx + dcy * dcy).sqrt();
                let ripple = (dist * 0.4 - ctx.time * 0.7).sin() * 0.15;

                // Combined density: eased base + spatial variation + ripple.
                let density = (ctx.eased + spatial * 0.35 - 0.175 + ripple).clamp(0.0, 1.0);

                // Map density → shade level 0..4.
                let level = (density * 4.0) as usize;
                draw::shade(grid, cx, cy, level);
            }
        }

        // Tint with palette
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4 — Big Bang expansion
// ---------------------------------------------------------------------------
// Structural idea: all dots radiate OUTWARD from a single origin point as
// eased increases. At progress=0 everything is at the singularity; at
// progress=1 dots fill the whole canvas. Radial particle positions scale
// linearly with eased, animated with a slow drift via time.

/// Big Bang: particles radiate from singularity; position = eased × final_coords.
struct BigBang;
impl ProgressStyle for BigBang {
    fn name(&self) -> &str {
        "big-bang"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Big-bang singularity explodes: all particles radiate outward, position scales with eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;

        // Singularity flash at t=0: bright core when eased is small.
        if ctx.eased < 0.05 {
            draw::dot_i(grid, cx as i32, cy as i32);
            return Ok(());
        }

        let particle_count = 120u32;
        for i in 0..particle_count {
            // Each particle has a stable random target position on the canvas edge.
            let target_angle = hash_f(i) * 2.0 * PI;
            // Target distance: between half-way and the edge.
            let target_r_frac = 0.5 + hash_f(i + 500) * 0.5;
            let max_half = cx.min(cy * 1.8);
            let target_r = target_r_frac * max_half;

            // Current position = origin + eased * (target - origin)
            let r = ctx.eased * target_r;

            // Slow thermal drift via time.
            let drift_a = target_angle + (ctx.time * 0.05 * hash_f(i + 200) * 2.0 - 0.05);
            let px = cx + r * drift_a.cos();
            let py = cy + r * drift_a.sin() * 0.55;

            // Draw particle; brighter near leading edge.
            draw::dot_i(grid, px as i32, py as i32);
            if hash(i.wrapping_add(17)) % 4 == 0 {
                draw::dot_i(grid, px as i32 + 1, py as i32);
            }
        }

        // Tint: hot centre → cool edge
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let dist = (cx_c as f32 - cells_w as f32 / 2.0).abs() / (cells_w as f32 / 2.0).max(1.0);
            let t = 1.0 - dist * 0.9;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5 — Solar flare arc
// ---------------------------------------------------------------------------
// Structural idea: a parabolic arc of dots loops OFF a stellar limb.
// The arc rises and falls (parametric in angle 0→π), its peak height driven
// by eased; time animates a slow rotation of the arc base on the stellar disc.
// No other style draws a single parametric arc off a limb.

/// Solar flare: a looping arc erupts off the stellar limb; height = eased.
struct SolarFlare;
impl ProgressStyle for SolarFlare {
    fn name(&self) -> &str {
        "solar-flare"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Solar flare arc loops off a stellar limb; peak height = eased; base rotates with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as f32;
        let cy = (h / 2) as f32;
        let star_r = (cy * 0.6).max(1.0);

        // Stellar disc.
        let disc_steps = 64usize;
        for s in 0..disc_steps {
            let a = s as f32 / disc_steps as f32 * 2.0 * PI;
            for dr in 0..=(star_r as i32) {
                let frac = dr as f32 / star_r;
                if hash(
                    (s as u32)
                        .wrapping_mul(13)
                        .wrapping_add(dr.unsigned_abs() * 7),
                ) % 10
                    < (frac * 9.0 + 1.0) as u32
                {
                    let px = cx + dr as f32 * a.cos();
                    let py = cy + dr as f32 * a.sin() * 0.55;
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Flare arc: parametric, base rotates with time.
        let base_angle = ctx.time * 0.4; // slow rotation
        let arc_half_span = 0.4f32; // angular half-width of the base on the limb (radians)

        // Arc from base_angle - arc_half_span → base_angle + arc_half_span via apex.
        // Apex is above the limb, height = eased * extra.
        let apex_r = star_r + ctx.eased * (cy * 1.2).max(1.0);

        let arc_steps = 50usize;
        for s in 0..arc_steps {
            let t = s as f32 / arc_steps.saturating_sub(1).max(1) as f32;
            // Parametric: angle sweeps from left base to right base;
            // radius peaks at apex_r at t=0.5, equals star_r at t=0 and t=1.
            let angle = (base_angle - arc_half_span) + t * 2.0 * arc_half_span;
            let r = star_r + (apex_r - star_r) * (PI * t).sin();

            let px = cx + r * angle.cos();
            let py = cy + r * angle.sin() * 0.55;
            draw::dot_i(grid, px as i32, py as i32);

            // Glow: an extra dot offset slightly outward
            if s % 3 == 0 {
                let px2 = cx + (r + 1.5) * angle.cos();
                let py2 = cy + (r + 1.5) * angle.sin() * 0.55;
                draw::dot_i(grid, px2 as i32, py2 as i32);
            }
        }

        // Tint: star warm orange, flare hot white
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6 — Aurora curtains
// ---------------------------------------------------------------------------
// Structural idea: vertical columns of dots whose height is modulated by a
// sine-wave, creating "curtains" that ripple horizontally via time.
// eased controls how many columns are lit (curtain extent).
// The ripple phase varies per column — distinct from any horizontal bar style.

/// Aurora: vertical curtain columns ripple via sine; extent = eased columns.
struct AuroraCurtains;
impl ProgressStyle for AuroraCurtains {
    fn name(&self) -> &str {
        "aurora-curtains"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Aurora curtains: vertical sine-sheet columns ripple via time; extent = eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let lit_cols = ((ctx.eased * w as f32) as usize).min(w);

        for x in 0..lit_cols {
            // Each column has a phase offset based on position.
            let col_phase = x as f32 * 0.3;
            // Height envelope: sine wave scrolling over time.
            let envelope = 0.5 + 0.5 * (ctx.time * 1.8 + col_phase).sin();
            // Secondary ripple for width texture.
            let secondary = 0.2 * (ctx.time * 3.1 + col_phase * 1.7).sin();
            let col_h_frac = (envelope + secondary).clamp(0.1, 1.0);
            let col_h = (col_h_frac * h as f32) as usize;

            // Curtain hangs from the top.
            let y0 = 0usize;
            let y1 = col_h.min(h).saturating_sub(1);
            for y in y0..=y1 {
                // Density fades toward the bottom of each curtain.
                let fade = 1.0 - (y as f32 / col_h.max(1) as f32);
                if hash(
                    (x as u32)
                        .wrapping_mul(17)
                        .wrapping_add(y as u32 * 31)
                        .wrapping_add((ctx.time * 8.0) as u32),
                ) % 8
                    < (fade * 7.5 + 0.5) as u32
                {
                    draw::dot(grid, x, y);
                }
            }
        }

        // Tint: green → blue → violet across width (aurora spectrum).
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7 — Meteor shower
// ---------------------------------------------------------------------------
// Structural idea: diagonal streaks rain from top-right to bottom-left.
// Active meteor count = eased * max_meteors; each meteor has a stable entry
// point on the top/right edge and trails behind it with decreasing dot density.
// Direction and density-from-count are structurally unlike all other styles.

/// Meteor shower: diagonal streaks rain across the canvas; count = eased * max.
struct MeteorShower;
impl ProgressStyle for MeteorShower {
    fn name(&self) -> &str {
        "meteor-shower"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Meteor shower: diagonal streaks rain top-right → bottom-left; count scales with eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let max_meteors = 20u32;
        let active = (ctx.eased * max_meteors as f32) as u32;

        for m in 0..active.min(max_meteors) {
            // Stable entry x along top edge + right edge combined.
            let entry_frac = hash_f(m);
            let speed = 0.6 + hash_f(m + 300) * 1.2;
            let trail_len = (4.0 + hash_f(m + 600) * 8.0) as usize;

            // Entry x (spread across full width plus some from right edge).
            let entry_x = (entry_frac * (w + h) as f32) as i32 - h as i32;
            // Head position: travels diagonally at rate (speed, speed/2).
            let travel = (ctx.time * speed * 8.0) as i32;
            // Wrap travel so shower continues indefinitely.
            let period = (w + h) as i32;
            let travel = travel % period.max(1);

            let head_x = entry_x - travel;
            let head_y = travel / 2;

            // Trail dots from head backward along the streak direction (+1, -0.5 per dot).
            for t in 0..trail_len {
                let tx = head_x + t as i32;
                let ty = head_y - t as i32 / 2;
                // Density drops with distance from head.
                let frac = 1.0 - t as f32 / trail_len as f32;
                if hash(
                    m.wrapping_mul(31)
                        .wrapping_add(t as u32 * 7)
                        .wrapping_add((ctx.time * 4.0) as u32),
                ) % 10
                    < (frac * frac * 9.5 + 0.5) as u32
                {
                    draw::dot_i(grid, tx, ty);
                }
            }
            // Bright head: 2-dot cluster.
            draw::dot_i(grid, head_x, head_y);
            draw::dot_i(grid, head_x - 1, head_y);
        }

        // Background star field (stable, sparse).
        for i in 0u32..20 {
            let sx = (hash_f(i + 9000) * w as f32) as i32;
            let sy = (hash_f(i + 9100) * h as f32) as i32;
            if hash(i.wrapping_add((ctx.time * 2.0) as u32)) % 4 != 0 {
                draw::dot_i(grid, sx, sy);
            }
        }

        // Tint: cool blue at top, warmer orange at bottom (heat from entry).
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = 1.0 - cy_c as f32 / cells_h.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy_c, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8 — Total eclipse
// ---------------------------------------------------------------------------
// Structural idea: TWO discs — a star (bright, fixed left-of-center) and
// an occluder moon (moving right-to-left across the star as eased goes 0→1).
// At mid-eclipse a corona ring of dots surrounds the star's limb where the
// moon occluder covers it. No other style has two interacting geometric bodies.

/// Total eclipse: moon disc occluder crosses star limb; corona ring appears at totality.
struct TotalEclipse;
impl ProgressStyle for TotalEclipse {
    fn name(&self) -> &str {
        "total-eclipse"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Moon occluder crosses star limb from right; corona ring bursts at mid-eclipse"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let star_cx = (w / 2) as i32;
        let star_cy = (h / 2) as i32;
        let star_r = ((h / 2).saturating_sub(1).max(2)) as i32;

        // Moon starts well to the right, crosses the star, exits to the left.
        // At eased=0.5 the centres align (totality).
        let travel = (w as f32 * 1.4) as i32;
        let moon_cx = star_cx + travel / 2 - (ctx.eased * travel as f32) as i32;
        let moon_cy = star_cy;
        let moon_r = (star_r as f32 * 0.92) as i32;

        // Draw stellar disc (skip pixels covered by moon).
        let steps = 72usize;
        for s in 0..steps {
            let a = s as f32 / steps as f32 * 2.0 * PI;
            for dr in 0..=star_r {
                let px = star_cx + (dr as f32 * a.cos()) as i32;
                let py = star_cy + (dr as f32 * a.sin() * 0.5) as i32;
                // Check if this dot is inside the moon disc.
                let ddx = px - moon_cx;
                let ddy = (py - moon_cy) * 2; // undo vertical squeeze
                let in_moon = ddx * ddx + ddy * ddy <= moon_r * moon_r;
                if !in_moon {
                    draw::dot_i(grid, px, py);
                }
            }
        }

        // Moon disc (always drawn, covers star).
        for s in 0..steps {
            let a = s as f32 / steps as f32 * 2.0 * PI;
            for dr in 0..=moon_r {
                let px = moon_cx + (dr as f32 * a.cos()) as i32;
                let py = moon_cy + (dr as f32 * a.sin() * 0.5) as i32;
                // We draw only the outline to leave the moon "dark" (no dots inside).
                if dr == moon_r || dr == moon_r - 1 {
                    draw::dot_i(grid, px, py);
                }
            }
        }

        // Corona ring: visible only near totality (eased ≈ 0.5).
        let totality_closeness = 1.0 - (ctx.eased * 2.0 - 1.0).abs(); // peaks at 0.5
        let corona_steps = 48usize;
        let corona_r = star_r + 2 + (ctx.time * 0.3).sin() as i32;
        let corona_r = corona_r.max(star_r + 1);
        for s in 0..corona_steps {
            let a = s as f32 / corona_steps as f32 * 2.0 * PI;
            let px = star_cx + (corona_r as f32 * a.cos()) as i32;
            let py = star_cy + (corona_r as f32 * a.sin() * 0.5) as i32;
            // Only appear when near totality; flicker via time.
            let threshold = (totality_closeness * 10.0) as u32;
            if hash((s as u32).wrapping_add((ctx.time * 5.0) as u32)) % 10 < threshold {
                draw::dot_i(grid, px, py);
            }
        }

        // Tint palette
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9 — Gravitational lensing
// ---------------------------------------------------------------------------
// Structural idea: dots from a "background" grid are displaced radially
// outward from a central lens mass, creating an Einstein-ring arc pattern.
// The ring radius grows with eased. Pure dot-displacement geometry — unlike
// any other style which has no concept of optical warping.

/// Gravitational lens: background light bends around a mass; Einstein ring at eased radius.
struct GravitationalLens;
impl ProgressStyle for GravitationalLens {
    fn name(&self) -> &str {
        "grav-lens"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Einstein ring arc: background dots deflect around a central mass; ring radius = eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as f32;
        let cy = (h / 2) as f32;
        let max_r = cx.min(cy * 1.8);
        // Einstein ring radius grows with eased.
        let ring_r = ctx.eased * max_r * 0.85;

        // Draw the Einstein ring arc (full ring at 100%, partial arc otherwise).
        let ring_steps = 80usize;
        let arc_frac = ctx.eased.min(1.0);
        let lit_steps = (arc_frac * ring_steps as f32) as usize;
        for s in 0..lit_steps.min(ring_steps) {
            let a = s as f32 / ring_steps as f32 * 2.0 * PI;
            // Ring thickness: 2 dots.
            for dr in [-1i32, 0, 1] {
                let r = (ring_r as i32 + dr).max(0) as f32;
                let px = cx + r * a.cos();
                let py = cy + r * a.sin() * 0.5;
                if hash((s as u32).wrapping_add(dr.unsigned_abs() * 199)) % 3 != 0 {
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Lensing smear arcs — shorter arcs between ring and lens mass showing
        // deflected background sources.
        let source_count = 6u32;
        for src in 0..source_count {
            let src_angle = hash_f(src) * 2.0 * PI;
            let src_r = max_r * (0.5 + hash_f(src + 100) * 0.45);
            // Source position (outside ring).
            let _sx = cx + src_r * src_angle.cos();
            let _sy = cy + src_r * src_angle.sin() * 0.5;

            // Arc of deflected image: a short curve at ring_r, on the far side.
            let arc_center_a = src_angle + PI; // opposite side from source
            let arc_half = 0.25;
            let arc_n = 12usize;
            for k in 0..arc_n {
                let t = k as f32 / arc_n.saturating_sub(1).max(1) as f32;
                let a = (arc_center_a - arc_half) + t * 2.0 * arc_half;
                let r = ring_r * (0.9 + 0.15 * (PI * t).sin());
                let px = cx + r * a.cos();
                let py = cy + r * a.sin() * 0.5;
                if ctx.eased > hash_f(src + 200) * 0.5 {
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }

        // Central lens mass: compact dot cluster.
        for dy in -1i32..=1 {
            draw::dot_i(grid, cx as i32, cy as i32 + dy);
        }
        draw::dot_i(grid, cx as i32 - 1, cy as i32);
        draw::dot_i(grid, cx as i32 + 1, cy as i32);

        // Tint
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let dist = (cx_c as f32 - cells_w as f32 / 2.0).abs() / (cells_w as f32 / 2.0).max(1.0);
            let t = 1.0 - dist * 0.6;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10 — Cosmic web
// ---------------------------------------------------------------------------
// Structural idea: a set of stable node positions connected by line filaments.
// Filaments appear one by one as eased grows (like galaxy filaments forming).
// Nodes pulse in brightness via time. Structurally: graph edges, not rings,
// not shells, not columns, not sweeping beams.

/// Cosmic web: galaxy nodes connected by filaments that appear as eased grows.
struct CosmicWeb;
impl ProgressStyle for CosmicWeb {
    fn name(&self) -> &str {
        "cosmic-web"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Cosmic web: node galaxies connected by filaments appearing as eased grows; nodes pulse"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        const NUM_NODES: u32 = 10;
        // Stable node positions (hash-seeded, small margin from edges).
        let nodes: Vec<(i32, i32)> = (0..NUM_NODES)
            .map(|i| {
                let nx = (hash_f(i) * (w.saturating_sub(4)) as f32 + 2.0) as i32;
                let ny = (hash_f(i + 200) * (h.saturating_sub(2)) as f32 + 1.0) as i32;
                (nx, ny)
            })
            .collect();

        // Filaments: connect each node to its two nearest by index
        // (i → i+1, i → i+2 wrapping) — gives a sparse web structure.
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for i in 0..NUM_NODES as usize {
            edges.push((i, (i + 1) % NUM_NODES as usize));
            edges.push((i, (i + 2) % NUM_NODES as usize));
            // One cross-brace to make it feel 3-D.
            edges.push((i, (i + NUM_NODES as usize / 2) % NUM_NODES as usize));
        }
        // Remove exact duplicates (normalise so a < b).
        edges.iter_mut().for_each(|(a, b)| {
            if *a > *b {
                std::mem::swap(a, b);
            }
        });
        edges.sort_unstable();
        edges.dedup();

        let lit_edges = (ctx.eased * edges.len() as f32) as usize;

        // Draw revealed filaments.
        for &(a, b) in edges.iter().take(lit_edges) {
            let (ax, ay) = nodes[a];
            let (bx, by) = nodes[b];
            let dx = (bx - ax).abs();
            let dy = (by - ay).abs();
            let steps = dx.max(dy).max(1);
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let px = ax + ((bx - ax) as f32 * t) as i32;
                let py = ay + ((by - ay) as f32 * t) as i32;
                // Filament has gaps — sparse, like gas strands.
                if hash(
                    (s as u32)
                        .wrapping_mul(a as u32 * 13 + b as u32 * 7 + 1)
                        .wrapping_add((ctx.time * 2.0) as u32),
                ) % 4
                    != 0
                {
                    draw::dot_i(grid, px, py);
                }
            }
        }

        // Node dots — bright cluster, pulse via time.
        for (i, &(nx, ny)) in nodes.iter().enumerate() {
            let pulse = (ctx.time * 1.5 + i as f32 * 0.9).sin() * 0.5 + 0.5;
            // Cross cluster; size modulated by pulse.
            let size = if pulse > 0.5 { 2i32 } else { 1i32 };
            for dy in -size..=size {
                for dx in -size..=size {
                    if dx.abs() + dy.abs() <= size {
                        draw::dot_i(grid, nx + dx, ny + dy);
                    }
                }
            }
        }

        // Tint with palette across width.
        let (cells_w, cells_h) = grid.dimensions();
        for cx_c in 0..cells_w {
            let t = cx_c as f32 / cells_w.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy_c in 0..cells_h {
                draw::tint_row(grid, cy_c, cx_c, cx_c, color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11 — Redshift wavefronts
// ---------------------------------------------------------------------------
// Structural idea: concentric horizontal wavefronts (hlines) expanding from a
// central source. Their spacing grows with eased (wavelength stretches = redshift).
// No circles — purely horizontal bands whose spacing is the progress indicator.

/// Redshift: wavefronts stretch horizontally from centre; spacing = eased wavelength.
struct Redshift;
impl ProgressStyle for Redshift {
    fn name(&self) -> &str {
        "redshift"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Redshift wavefronts: horizontal bands from source; spacing stretches with eased"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Source at vertical centre.
        let cy = (h / 2) as f32;

        // Wavelength stretches: at eased=0 waves are packed (short λ, blue);
        // at eased=1 they are widely spaced (long λ, red).
        let min_lambda = 2.0f32;
        let max_lambda = (h as f32 * 0.45).max(min_lambda + 0.5);
        let lambda = min_lambda + ctx.eased * (max_lambda - min_lambda);

        // Emit wave fronts from cy; they scroll outward via time.
        // Offset = time * wave_speed mod lambda.
        let wave_speed = 3.0f32;
        let phase_offset = (ctx.time * wave_speed) % lambda.max(0.01);

        // Draw wavefronts above and below the source.
        let mut y_up = cy - phase_offset;
        let mut y_dn = cy + phase_offset;

        let max_waves = 20usize;
        for _wv in 0..max_waves {
            // Draw a horizontal line at this y position.
            let iy_up = y_up.round() as i32;
            let iy_dn = y_dn.round() as i32;
            if iy_up >= 0 {
                // Partial width: wavefront amplitude decays with distance from source.
                let dist_frac = (cy - y_up) / cy.max(1.0);
                let width_frac = (1.0 - dist_frac * 0.4).clamp(0.1, 1.0);
                let x_margin = ((1.0 - width_frac) * w as f32 * 0.5) as usize;
                draw::hline(
                    grid,
                    x_margin,
                    w.saturating_sub(x_margin + 1),
                    iy_up as usize,
                );
            }
            if iy_dn < h as i32 && iy_dn != iy_up {
                let dist_frac = (y_dn - cy) / (h as f32 - cy).max(1.0);
                let width_frac = (1.0 - dist_frac * 0.4).clamp(0.1, 1.0);
                let x_margin = ((1.0 - width_frac) * w as f32 * 0.5) as usize;
                draw::hline(
                    grid,
                    x_margin,
                    w.saturating_sub(x_margin + 1),
                    iy_dn as usize,
                );
            }

            y_up -= lambda;
            y_dn += lambda;

            if y_up < 0.0 && y_dn >= h as f32 {
                break;
            }
        }

        // Source dot at centre.
        draw::dot_i(grid, (w / 2) as i32, cy as i32);

        // Tint: blue at eased=0, red at eased=1 (redshift colour shift).
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = ctx.eased; // fixed tint driven by eased, not position
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy_c, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 12 — Quasar jet
// ---------------------------------------------------------------------------
// Structural idea: two opposed straight-line relativistic jets fire along the
// vertical axis from a central nucleus; jet length = eased * half-height.
// Knots (bright blobs) travel along the jet at speed driven by time.
// A small accretion torus ring is drawn perpendicular to the jets.
// The structural element is a VLINE-based bidirectional jet — unique.

/// Quasar jet: twin relativistic jets fire along vertical axis; length = eased.
struct QuasarJet;
impl ProgressStyle for QuasarJet {
    fn name(&self) -> &str {
        "quasar-jet"
    }
    fn theme(&self) -> &str {
        "cosmos"
    }
    fn describe(&self) -> &str {
        "Quasar twin jets fire vertically from nucleus; length = eased; knots travel via time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;

        // Jet length grows with eased.
        let jet_len = (ctx.eased * cy as f32) as i32;

        // Draw primary jet spine (3 dots wide for visibility).
        for jet_dir in [-1i32, 1i32] {
            for offset in [-1i32, 0, 1] {
                let col = (cx + offset).max(0);
                let y_end = cy + jet_dir * jet_len;
                let y0 = cy.min(y_end).max(0) as usize;
                let y1 = cy.max(y_end).min(h as i32 - 1) as usize;
                draw::vline(grid, col as usize, y0, y1);
            }
        }

        // Bright knots travelling outward along each jet.
        let knot_count = 4usize;
        let knot_speed = 5.0f32;
        for jet_dir in [-1i32, 1i32] {
            for k in 0..knot_count {
                let phase = k as f32 / knot_count as f32;
                let knot_pos = ((ctx.time * knot_speed + phase * jet_len as f32)
                    % jet_len.max(1) as f32) as i32;
                let ky = cy + jet_dir * knot_pos;
                // Cross-shaped bright knot.
                for dy in -1i32..=1 {
                    for dx in -2i32..=2 {
                        if dx.abs() + dy.abs() <= 2 {
                            draw::dot_i(grid, cx + dx, ky + dy);
                        }
                    }
                }
            }
        }

        // Accretion torus: small horizontal ellipse around the nucleus.
        let torus_rx = (w / 6).max(2) as i32;
        let torus_ry = 1i32;
        let torus_steps = 32usize;
        for s in 0..torus_steps {
            let a = s as f32 / torus_steps as f32 * 2.0 * PI;
            let px = cx + (torus_rx as f32 * a.cos()) as i32;
            let py = cy + (torus_ry as f32 * a.sin()) as i32;
            draw::dot_i(grid, px, py);
        }

        // Nucleus.
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Tint: jet colour from palette top→bottom.
        let (cells_w, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(1.0 - t);
            draw::tint_row(grid, cy_c, 0, cells_w.saturating_sub(1), color);
        }

        Ok(())
    }
}
