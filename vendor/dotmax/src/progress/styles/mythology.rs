//! Mythology-themed progress bars — twelve animated braille styles drawn from
//! Greek, Norse, and world mythology.
//!
//! Each bar is structurally unique: distinct geometry, motion, and symbol
//! approach. Color alone is never the only differentiator. Every bar animates
//! with `ctx.time` and advances with `ctx.eased` progress.
//!
//! Styles:
//! 1. **phoenix-rising**   — wings spread + ascending flame column flicker
//! 2. **dragon-flight**    — dragon body sweeps left→right breathing a fire arc
//! 3. **hydra-heads**      — each 10% unlocks a new sinusoidal neck + head
//! 4. **kraken-depths**    — tentacles rise from a baseline with sine undulation
//! 5. **pegasus-gallop**   — horse body gallops across + beating wing arcs
//! 6. **medusa-gaze**      — radial snake-hair strands writhe outward from centre
//! 7. **minotaur-labyrinth** — rectangular maze traced inward as progress advances
//! 8. **icarus-sun**       — figure rises toward a sun, feathers scatter at 100%
//! 9. **cerberus-watch**   — three dog-heads bob in a row, jaw opening with time
//! 10. **valkyrie-ride**   — lance-wielding rider sweeps across with spear trails
//! 11. **yggdrasil-grows** — world-tree trunk + spreading branches + root system
//! 12. **ouroboros**       — snake traces a circle that closes as progress → 1

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

/// All styles in the `mythology` theme.
///
/// Returns twelve boxed mythology-themed bars. Each is structurally distinct
/// (geometry, motion, and symbol); color alone never separates them.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(PhoenixRising),
        Box::new(DragonFlight),
        Box::new(HydraHeads),
        Box::new(KrakenDepths),
        Box::new(PegasusGallop),
        Box::new(MedusaGaze),
        Box::new(MinotaurLabyrinth),
        Box::new(IcarusSun),
        Box::new(CerberusWatch),
        Box::new(ValkyriRide),
        Box::new(YggdrasilGrows),
        Box::new(Ouroboros),
    ]
}

// ---------------------------------------------------------------------------
// 1. Phoenix Rising
// ---------------------------------------------------------------------------
// A central flame column grows with progress. Two wing arcs spread outward,
// whose span scales with eased progress. Flame tips flicker with time via
// sine-perturbed dot heights.

struct PhoenixRising;
impl ProgressStyle for PhoenixRising {
    fn name(&self) -> &str {
        "phoenix-rising"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Phoenix ascends: flame column grows with progress, wings spread wide and flicker with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        // Flame column height driven by eased progress.
        let flame_h = ((ctx.eased * h as f32 * 0.85).round() as usize).max(1);
        let base_y = (h - 1) as i32;

        // Draw central flame: each dot column flickers via per-x sine offset.
        for seg in 0..flame_h {
            let sy = base_y - seg as i32;
            // Flicker narrows the flame at the tip.
            let tip_frac = seg as f32 / flame_h.max(1) as f32;
            let half_w = ((1.0 - tip_frac) * 2.0 + 1.0).round() as i32;
            let flicker = ((ctx.time * 8.0 + seg as f32 * 0.5).sin() * 1.5).round() as i32;
            for dx in -half_w..=half_w {
                draw::dot_i(grid, cx + dx + flicker / 2, sy);
            }
        }

        // Wing arcs: sweep outward proportional to eased progress.
        let wing_span = ((ctx.eased * (w as f32 * 0.45)).round() as i32).max(1);
        let wing_beat = ((ctx.time * 4.0).sin() * h as f32 * 0.12).round() as i32;
        let wing_root_y = base_y - (flame_h as i32 / 3);

        // Left wing: descending arc from root to tip.
        let steps = (wing_span * 3).max(8) as usize;
        for s in 0..=steps {
            let t = s as f32 / steps.max(1) as f32;
            let wx = cx - (t * wing_span as f32).round() as i32;
            // Arc: starts level, curves down then up at tip.
            let wy =
                wing_root_y - ((t * PI * 0.8).sin() * h as f32 * 0.3).round() as i32 + wing_beat;
            draw::dot_i(grid, wx, wy);
            // Wing membrane — fill one row down.
            draw::dot_i(grid, wx, wy + 1);
        }

        // Right wing: mirror.
        for s in 0..=steps {
            let t = s as f32 / steps.max(1) as f32;
            let wx = cx + (t * wing_span as f32).round() as i32;
            let wy =
                wing_root_y - ((t * PI * 0.8).sin() * h as f32 * 0.3).round() as i32 + wing_beat;
            draw::dot_i(grid, wx, wy);
            draw::dot_i(grid, wx, wy + 1);
        }

        // Palette tint: deep ember at bottom, bright gold at top.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = 1.0 - cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Dragon Flight
// ---------------------------------------------------------------------------
// A serpentine dragon body (sine-oscillating spine) sweeps left→right as
// eased progress advances. Its mouth emits a diverging arc of fire dots
// ahead of the head. Wings beat with time.

struct DragonFlight;
impl ProgressStyle for DragonFlight {
    fn name(&self) -> &str {
        "dragon-flight"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Dragon flies left-to-right with a serpentine body and arc of fire breath"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid_y = (h / 2) as i32;
        // Head x tracks eased progress.
        let head_x = (ctx.eased * w as f32).round() as i32;
        // Dragon body length scales with grid width.
        let body_len = ((w as f32 * 0.35) as i32).max(4).min(w as i32 - 1);

        // Serpentine body: each segment offset by sine of position + time.
        for seg in 0..body_len {
            let bx = head_x - seg;
            if bx < 0 {
                break;
            }
            let wave =
                ((seg as f32 * 0.28 + ctx.time * 3.0).sin() * h as f32 * 0.25).round() as i32;
            let by = mid_y + wave;
            draw::dot_i(grid, bx, by);
            // Thickness: two dots tall on the body center.
            draw::dot_i(grid, bx, by + 1);
        }

        // Head: a 3×3 blob.
        draw::dot_i(grid, head_x, mid_y - 1);
        draw::dot_i(grid, head_x, mid_y);
        draw::dot_i(grid, head_x, mid_y + 1);
        draw::dot_i(grid, head_x + 1, mid_y);

        // Wings: two arcs above and below the body that beat.
        let wing_beat = ((ctx.time * 5.0).sin() * h as f32 * 0.2).round() as i32;
        let wing_w = (body_len / 2).max(2);
        for dx in 0..wing_w {
            let t = dx as f32 / wing_w.max(1) as f32;
            let wing_y_up = mid_y - (h as f32 * 0.2 * (t * PI).sin()).round() as i32 - wing_beat;
            let wing_y_dn = mid_y + (h as f32 * 0.2 * (t * PI).sin()).round() as i32 + wing_beat;
            let wx = head_x - (body_len / 4) - dx;
            draw::dot_i(grid, wx, wing_y_up);
            draw::dot_i(grid, wx, wing_y_dn);
        }

        // Fire breath: diverging arc of dots ahead of head.
        let fire_len = ((w as i32 - head_x).min(w as i32 / 3)).max(0);
        for fi in 1..=fire_len {
            let spread = (fi as f32 * 0.4).round() as i32;
            let flicker = ((ctx.time * 9.0 + fi as f32 * 0.7).sin() * 1.5).round() as i32;
            for ds in -spread..=spread {
                // Only draw the outer shell of the cone, plus flicker.
                if ds.abs() >= spread - 1 || (fi + flicker.abs() as i32) % 3 == 0 {
                    draw::dot_i(grid, head_x + fi, mid_y + ds + flicker);
                }
            }
        }

        // Palette: fiery gradient across columns.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Hydra Heads
// ---------------------------------------------------------------------------
// Each 10% of progress reveals a new neck+head growing from a shared base.
// Every neck follows a sinusoidal path; heads bob with time independently.

struct HydraHeads;
impl ProgressStyle for HydraHeads {
    fn name(&self) -> &str {
        "hydra-heads"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Hydra: each 10% progress grows a new writhing neck and snapping head"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let base_y = (h - 1) as i32;
        // One head per 10%.
        let n_heads = ((ctx.eased * 10.0).ceil() as usize).max(1).min(10);

        // Body base: thick horizontal bar at the bottom.
        let body_w = (w as f32 * 0.4).round() as usize;
        let body_x0 = w / 2 - body_w / 2;
        draw::hline(grid, body_x0, body_x0 + body_w, base_y as usize);
        draw::hline(
            grid,
            body_x0,
            body_x0 + body_w,
            (base_y - 1).max(0) as usize,
        );

        // Spread heads evenly across body width.
        for i in 0..n_heads {
            // Base x of this neck.
            let neck_base_x = if n_heads == 1 {
                w / 2
            } else {
                body_x0 + (i * body_w) / (n_heads - 1).max(1)
            };

            let neck_h = (h as f32 * 0.65).round() as usize;
            // Each head bobs at a different frequency.
            let bob_freq = 1.5 + i as f32 * 0.4;
            let bob = ((ctx.time * bob_freq + i as f32 * 1.1).sin() * 2.0).round() as i32;

            // Neck: sine-curved spine from base upward.
            for seg in 0..neck_h {
                let seg_y = base_y - seg as i32;
                let sway_amp = seg as f32 / neck_h.max(1) as f32 * 3.5;
                let sway = ((ctx.time * 1.8 + i as f32 * 0.9 + seg as f32 * 0.3).sin() * sway_amp)
                    .round() as i32;
                draw::dot_i(grid, neck_base_x as i32 + sway, seg_y);
            }

            // Head: oval 3×2 at the neck tip.
            let sway_tip = ((ctx.time * 1.8 + i as f32 * 0.9 + neck_h as f32 * 0.3).sin() * 3.5)
                .round() as i32;
            let hx = neck_base_x as i32 + sway_tip;
            let hy = base_y - neck_h as i32 + bob;
            for dy in -1i32..=1 {
                for dx in -2i32..=2 {
                    if dx.abs() + dy.abs() * 2 <= 3 {
                        draw::dot_i(grid, hx + dx, hy + dy);
                    }
                }
            }
            // Jaw: jabs open and shut with time.
            let jaw = ((ctx.time * bob_freq * 1.3 + i as f32).sin() * 1.5 + 1.5).round() as i32;
            draw::dot_i(grid, hx + 2, hy + 1 + jaw);
            draw::dot_i(grid, hx + 3, hy + 1 + jaw);
        }

        // Palette: hellish gradient bottom to top.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = 1.0 - cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Kraken Depths
// ---------------------------------------------------------------------------
// A seafloor baseline spans the bottom. Tentacles rise from it; their count
// and height track eased progress. Each tentacle is a sine-undulating column
// of dots, with a sucker dot on alternating sides.

struct KrakenDepths;
impl ProgressStyle for KrakenDepths {
    fn name(&self) -> &str {
        "kraken-depths"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Kraken tentacles rise from the abyss, sine-undulating as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let floor_y = (h - 1) as i32;
        // Seafloor.
        draw::hline(grid, 0, w - 1, floor_y as usize);

        let n_tent = ((ctx.eased * 8.0).ceil() as usize).max(1).min(8);
        let tent_spacing = (w / n_tent.max(1)).max(1);

        for i in 0..n_tent {
            let tx_base = (i * tent_spacing + tent_spacing / 2) as i32;
            // Height: full tentacles plus a partially grown newest one.
            let full_frac = if n_tent == 1 {
                ctx.eased
            } else {
                let step = 1.0 / n_tent as f32;
                let head_frac = (ctx.eased - (i as f32 * step)) / step;
                head_frac.clamp(0.0, 1.0)
            };
            let tent_h = (full_frac * h as f32 * 0.85).round() as usize;

            let freq = 1.4 + i as f32 * 0.25;
            for seg in 0..tent_h {
                let seg_y = floor_y - seg as i32;
                let phase = ctx.time * freq + i as f32 * 1.2 + seg as f32 * 0.22;
                let sway = (phase.sin() * (seg as f32 * 0.08).min(4.0)).round() as i32;
                draw::dot_i(grid, tx_base + sway, seg_y);
                // Sucker nodes every 5 dots, alternating sides.
                if seg % 5 == 2 {
                    let sucker_side: i32 = if seg % 10 < 5 { 2 } else { -2 };
                    draw::dot_i(grid, tx_base + sway + sucker_side, seg_y);
                }
            }
            // Curled tip.
            if tent_h > 0 {
                let tip_sway = ((ctx.time * freq + i as f32 * 1.2 + tent_h as f32 * 0.22).sin()
                    * (tent_h as f32 * 0.08).min(4.0))
                .round() as i32;
                let tip_y = floor_y - tent_h as i32;
                let curl = ((ctx.time * 2.0 + i as f32 * 0.5).sin() * 2.0).round() as i32;
                draw::dot_i(grid, tx_base + tip_sway + curl, tip_y - 1);
                draw::dot_i(grid, tx_base + tip_sway + curl + 1, tip_y - 2);
            }
        }

        // Deep-ocean tint: darkest at bottom.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Pegasus Gallop
// ---------------------------------------------------------------------------
// A horse body (oval) advances left→right driven by eased progress. Four
// legs cycle through a gallop pattern with time. Two wing arcs above beat
// independently with a different frequency.

struct PegasusGallop;
impl ProgressStyle for PegasusGallop {
    fn name(&self) -> &str {
        "pegasus-gallop"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Pegasus gallops across the bar: legs cycle through a gait, wings beat overhead"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid_y = (h / 2) as i32;
        let cx = (ctx.eased * w as f32).round() as i32;

        // Body: horizontal ellipse.
        let body_rx = ((w as f32 * 0.12) as i32).max(2);
        let body_ry = ((h as f32 * 0.2) as i32).max(1);
        let body_steps = (body_rx * 8).max(12) as usize;
        for s in 0..=body_steps {
            let angle = s as f32 / body_steps as f32 * 2.0 * PI;
            let bx = cx + (angle.cos() * body_rx as f32).round() as i32;
            let by = mid_y + (angle.sin() * body_ry as f32).round() as i32;
            draw::dot_i(grid, bx, by);
        }
        // Fill body centre.
        draw::dot_i(grid, cx, mid_y);

        // Head & neck: small oval in front of body.
        let head_x = cx + body_rx + 2;
        let head_ry = (body_ry as f32 * 0.7).round() as i32;
        for s in 0..16usize {
            let angle = s as f32 / 16.0 * 2.0 * PI;
            let hx = head_x + (angle.cos() * 2.0).round() as i32;
            let hy = mid_y - body_ry + (angle.sin() * head_ry as f32).round() as i32;
            draw::dot_i(grid, hx, hy);
        }
        // Mane flick.
        let mane = ((ctx.time * 7.0).sin() * 1.5).round() as i32;
        draw::dot_i(grid, head_x - 1, mid_y - body_ry - 2 + mane);

        // Four legs cycling through gallop: each leg has a phase offset.
        let leg_phases = [0.0f32, PI, PI * 0.5, PI * 1.5];
        let leg_offsets_x: [i32; 4] = [-body_rx + 1, -body_rx + 3, body_rx - 3, body_rx - 1];
        for (idx, (&loff, &phase)) in leg_offsets_x.iter().zip(leg_phases.iter()).enumerate() {
            let leg_x = cx + loff;
            let leg_cycle = (ctx.time * 6.0 + phase).sin();
            let knee_y = mid_y + body_ry + 2;
            let foot_y = mid_y + body_ry + (3.0 + leg_cycle * 2.5).round() as i32;
            let knee_x = leg_x + (leg_cycle * 1.5).round() as i32;
            // Upper leg.
            draw::dot_i(grid, leg_x, mid_y + body_ry);
            draw::dot_i(grid, knee_x, knee_y);
            // Lower leg.
            let foot_swing = ((ctx.time * 6.0 + phase + 0.4).sin() * 1.5).round() as i32;
            draw::dot_i(grid, knee_x + foot_swing, foot_y);
            // Hoof.
            draw::dot_i(grid, knee_x + foot_swing, foot_y + 1);
            draw::dot_i(grid, knee_x + foot_swing + 1, foot_y + 1);
            let _ = idx;
        }

        // Wings: two arcs above body, beating at a different frequency.
        let wing_beat = ((ctx.time * 3.5).sin() * h as f32 * 0.18).round() as i32;
        let wing_span = ((w as f32 * 0.25) as i32).max(2);
        for dx in 0..=wing_span {
            let t = dx as f32 / wing_span.max(1) as f32;
            let arc_y = mid_y
                - body_ry
                - (t * PI).sin() as i32 * ((h as f32 * 0.25).round() as i32)
                - wing_beat;
            draw::dot_i(grid, cx - dx, arc_y);
            draw::dot_i(grid, cx + dx, arc_y);
        }

        // Sky gradient tint.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(1.0 - t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Medusa Gaze
// ---------------------------------------------------------------------------
// A central head glyph at the progress point. From it, radial snake-hair
// strands extend outward, each following its own sinusoidal path. Strand
// count and length grow with eased progress.

struct MedusaGaze;
impl ProgressStyle for MedusaGaze {
    fn name(&self) -> &str {
        "medusa-gaze"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Medusa: radial writhing snake-hair strands grow outward from a central head"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Head centre: x tracks eased progress.
        let cx = ((ctx.eased * 0.85 + 0.075) * w as f32).round() as i32;
        let cy = (h / 2) as i32;

        // Snake-hair: n_strands radial lines with sinusoidal wobble.
        let n_strands = 8usize;
        let max_len = ((h.min(w)) as f32 * 0.42).round() as i32;
        let strand_len = ((ctx.eased * max_len as f32).round() as i32).max(1);

        for s in 0..n_strands {
            let base_angle = s as f32 / n_strands as f32 * 2.0 * PI;
            let phase_off = s as f32 * 0.7;
            let freq = 2.2 + s as f32 * 0.15;

            for seg in 1..=strand_len {
                // Sinusoidal deviation perpendicular to radial direction.
                let wobble =
                    ((ctx.time * freq + seg as f32 * 0.35 + phase_off).sin() * seg as f32 * 0.18)
                        .min(3.0);
                let perp_angle = base_angle + PI / 2.0;
                let sx =
                    cx + (base_angle.cos() * seg as f32 + perp_angle.cos() * wobble).round() as i32;
                let sy =
                    cy + (base_angle.sin() * seg as f32 + perp_angle.sin() * wobble).round() as i32;
                draw::dot_i(grid, sx, sy);
            }

            // Snake head at tip: two extra dots.
            if strand_len > 1 {
                let end_wobble = ((ctx.time * freq + strand_len as f32 * 0.35 + phase_off).sin()
                    * strand_len as f32
                    * 0.18)
                    .min(3.0);
                let perp_angle = base_angle + PI / 2.0;
                let hx = cx
                    + (base_angle.cos() * (strand_len + 1) as f32 + perp_angle.cos() * end_wobble)
                        .round() as i32;
                let hy = cy
                    + (base_angle.sin() * (strand_len + 1) as f32 + perp_angle.sin() * end_wobble)
                        .round() as i32;
                draw::dot_i(grid, hx, hy);
                draw::dot_i(
                    grid,
                    hx + base_angle.cos().round() as i32,
                    hy + base_angle.sin().round() as i32,
                );
            }
        }

        // Central head oval.
        for s in 0..12usize {
            let angle = s as f32 / 12.0 * 2.0 * PI;
            draw::dot_i(
                grid,
                cx + (angle.cos() * 2.0).round() as i32,
                cy + (angle.sin() * 1.5).round() as i32,
            );
        }
        // Eye glow pulses with time.
        let eye_pulse = ((ctx.time * 4.0).sin() * 0.5 + 0.5) as i32;
        draw::dot_i(grid, cx - 1, cy - eye_pulse);
        draw::dot_i(grid, cx + 1, cy - eye_pulse);

        // Venomous green gradient tint.
        let (_, cells_h) = grid.dimensions();
        for cy_cell in 0..cells_h {
            let t = cy_cell as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_cell,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Minotaur Labyrinth
// ---------------------------------------------------------------------------
// Concentric rectangular outlines are drawn from the outside in; the number
// of visible rings grows with eased progress. A Minotaur dot pulses in the
// centre, animated with time. This is pure geometry — no flowing shapes.

struct MinotaurLabyrinth;
impl ProgressStyle for MinotaurLabyrinth {
    fn name(&self) -> &str {
        "minotaur-labyrinth"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Labyrinth rings drawn inward ring-by-ring as progress advances; Minotaur pulses at the core"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Max rings that fit given smallest dimension.
        let max_rings = (h.min(w) / 4).max(1);
        let active_rings = ((ctx.eased * max_rings as f32).ceil() as usize).min(max_rings);

        for ring in 0..active_rings {
            let margin = ring * 2;
            let rx = margin;
            let ry = margin;
            let rw = w.saturating_sub(margin * 2);
            let rh = h.saturating_sub(margin * 2);
            if rw < 2 || rh < 2 {
                break;
            }
            draw::rect_outline(grid, rx, ry, rw, rh);

            // Gap in each ring would require clearing individual dots, which the API
            // does not expose; the concentric outlines convey the labyrinth structure.
        }

        // Minotaur pulse at centre.
        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let pulse = ((ctx.time * 3.0).sin() * 1.5).round() as i32;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx.abs() + dy.abs() <= 1 + pulse.abs() as i32 / 2 {
                    draw::dot_i(grid, cx + dx, cy + dy);
                }
            }
        }
        // "Horns".
        draw::dot_i(grid, cx - 2, cy - 2 - pulse.abs());
        draw::dot_i(grid, cx + 2, cy - 2 - pulse.abs());
        draw::dot_i(grid, cx - 3, cy - 3 - pulse.abs());
        draw::dot_i(grid, cx + 3, cy - 3 - pulse.abs());

        // Stone-cold tint: muted across rows.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t * 0.6 + 0.1),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Icarus Sun
// ---------------------------------------------------------------------------
// A figure (dot cluster) rises toward a radiant sun at the top-right corner.
// The figure's altitude tracks eased progress. When progress ≥ 0.9 the wings
// begin shedding feathers (scattered dots falling in arcs below the figure).

struct IcarusSun;
impl ProgressStyle for IcarusSun {
    fn name(&self) -> &str {
        "icarus-sun"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Icarus climbs toward the blazing sun; feathers scatter and fall as he nears 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Sun: radiating dot star at top-right corner.
        let sun_x = (w as i32) - 4;
        let sun_y = 3i32;
        let sun_r = ((h.min(w) as f32 * 0.18).round() as i32).max(2);
        // Sun disc.
        let disc_steps = (sun_r * 8).max(16) as usize;
        for s in 0..=disc_steps {
            let angle = s as f32 / disc_steps as f32 * 2.0 * PI;
            draw::dot_i(
                grid,
                sun_x + (angle.cos() * sun_r as f32).round() as i32,
                sun_y + (angle.sin() * (sun_r as f32 * 0.6)).round() as i32,
            );
        }
        // Sun rays: 8 spokes radiating outward, pulsing with time.
        let ray_len = ((sun_r as f32 * 1.6) as i32).max(2);
        let ray_pulse = ((ctx.time * 2.0).sin() * 1.5).round() as i32;
        for r in 0..8usize {
            let angle = r as f32 / 8.0 * 2.0 * PI + ctx.time * 0.3;
            for d in sun_r + 1..=sun_r + ray_len + ray_pulse {
                draw::dot_i(
                    grid,
                    sun_x + (angle.cos() * d as f32).round() as i32,
                    sun_y + (angle.sin() * d as f32 * 0.6).round() as i32,
                );
            }
        }

        // Icarus figure: rises from bottom-left toward the sun with progress.
        let fig_x = (ctx.eased * (w as f32 - 8.0) + 2.0).round() as i32;
        let fig_y = (h as f32 - ctx.eased * (h as f32 - 4.0) - 2.0).round() as i32;

        // Body.
        draw::dot_i(grid, fig_x, fig_y);
        draw::dot_i(grid, fig_x, fig_y + 1);
        draw::dot_i(grid, fig_x, fig_y + 2);
        // Head.
        draw::dot_i(grid, fig_x, fig_y - 1);
        draw::dot_i(grid, fig_x + 1, fig_y - 1);
        // Wings: two arcs that beat.
        let wing_beat = ((ctx.time * 4.5).sin() * h as f32 * 0.15).round() as i32;
        let wing_span = ((w as f32 * 0.15) as i32).max(3);
        for dx in 1..=wing_span {
            let t = dx as f32 / wing_span.max(1) as f32;
            let wy = fig_y + 1 - (t * PI * 0.6).sin() as i32 * 3 - wing_beat;
            draw::dot_i(grid, fig_x - dx, wy);
            draw::dot_i(grid, fig_x + dx + 1, wy);
        }

        // Feather fall: when progress ≥ 0.9, shed animated feathers below.
        if ctx.progress >= 0.9 {
            let melt = (ctx.progress - 0.9) / 0.1;
            let n_feathers = (melt * 12.0).round() as usize;
            for fi in 0..n_feathers {
                let fall_phase = (ctx.time * 1.8 + fi as f32 * 0.6) % 1.0;
                let fx = fig_x - wing_span + fi as i32 % (wing_span * 2 + 1).max(1);
                let fy = fig_y + 3 + (fall_phase * h as f32 * 0.7).round() as i32;
                let drift = ((ctx.time * 0.8 + fi as f32 * 1.1).sin() * 3.0).round() as i32;
                draw::dot_i(grid, fx + drift, fy);
                draw::dot_i(grid, fx + drift + 1, fy + 1);
            }
        }

        // Solar gradient tint.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = 1.0 - cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Cerberus Watch
// ---------------------------------------------------------------------------
// Three dog-heads bob across the progress bar in a row; each head is drawn
// as an oval outline with two eye dots and an animated jaw gap. Head spacing
// tracks progress (heads spread out as progress advances).

struct CerberusWatch;
impl ProgressStyle for CerberusWatch {
    fn name(&self) -> &str {
        "cerberus-watch"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Cerberus: three animated dog-heads spread across the bar, jaws barking with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid_y = (h / 2) as i32;
        // The three heads spread from a cluster on the left to evenly spaced as progress → 1.
        let base_cx = (w / 2) as i32;
        // Spread: at progress=0 all three overlap at centre; at 1 they span 80% of w.
        let spread = (ctx.eased * w as f32 * 0.4).round() as i32;
        let head_xs: [i32; 3] = [base_cx - spread, base_cx, base_cx + spread];
        let bob_freqs = [2.1f32, 2.7, 1.9];

        for (i, &hx) in head_xs.iter().enumerate() {
            let bob = ((ctx.time * bob_freqs[i] + i as f32 * 1.2).sin() * (h as f32 * 0.08)).round()
                as i32;
            let hy = mid_y + bob;

            // Head oval.
            let head_rx = ((w as f32 * 0.06).round() as i32).max(2).min(w as i32 / 4);
            let head_ry = ((h as f32 * 0.22).round() as i32).max(1);
            let oval_steps = (head_rx * 6 + head_ry * 3).max(10) as usize;
            for s in 0..oval_steps {
                let angle = s as f32 / oval_steps as f32 * 2.0 * PI;
                let ox = hx + (angle.cos() * head_rx as f32).round() as i32;
                let oy = hy + (angle.sin() * head_ry as f32).round() as i32;
                draw::dot_i(grid, ox, oy);
            }

            // Eyes.
            draw::dot_i(grid, hx - 1, hy - head_ry / 2);
            draw::dot_i(grid, hx + 1, hy - head_ry / 2);

            // Jaw: lower part of the oval opens based on a bark cycle.
            let bark = ((ctx.time * bob_freqs[i] * 1.5 + i as f32 * 0.8).sin()
                * (head_ry as f32 * 0.4))
                .round() as i32;
            if bark > 0 {
                // Upper jaw outline stays; lower jaw drops.
                for dx in -head_rx..=head_rx {
                    draw::dot_i(grid, hx + dx, hy + head_ry + bark);
                }
                // Tongue dot in the gap.
                draw::dot_i(grid, hx, hy + head_ry + bark / 2);
            }

            // Ears: two points above head.
            draw::dot_i(grid, hx - head_rx + 1, hy - head_ry - 2);
            draw::dot_i(grid, hx + head_rx - 1, hy - head_ry - 2);
        }

        // Body connecting the three heads: a wide thick bar below.
        let body_y = mid_y + (h as f32 * 0.3).round() as i32;
        draw::hline(
            grid,
            (base_cx - spread - 2).max(0) as usize,
            (base_cx + spread + 2).min(w as i32 - 1) as usize,
            body_y.clamp(0, h as i32 - 1) as usize,
        );
        draw::hline(
            grid,
            (base_cx - spread - 2).max(0) as usize,
            (base_cx + spread + 2).min(w as i32 - 1) as usize,
            (body_y + 1).clamp(0, h as i32 - 1) as usize,
        );

        // Underworld tint.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Valkyrie Ride
// ---------------------------------------------------------------------------
// A horse+rider silhouette sweeps left→right (eased progress). A spear
// extends forward from the rider; the horse gallops (leg cycle with time).
// A trail of braille dots smears behind. Wings on the helmet beat with time.

struct ValkyriRide;
impl ProgressStyle for ValkyriRide {
    fn name(&self) -> &str {
        "valkyrie-ride"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Valkyrie charges across on horseback: spear extended, trail of glory behind"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid_y = (h / 2) as i32;
        let cx = (ctx.eased * w as f32).round() as i32;

        // Trail of fading dots behind the rider.
        let trail_len = ((w as f32 * 0.3) as i32).max(4);
        for t in 1..=trail_len {
            let tx = cx - t;
            let ty = mid_y + ((ctx.time * 4.0 + t as f32 * 0.3).sin() * 1.0).round() as i32;
            // Thinning trail: draw every other dot further back.
            if t % 2 == 0 || t <= trail_len / 3 {
                draw::dot_i(grid, tx, ty);
            }
        }

        // Horse body: horizontal ellipse.
        let body_rx = ((w as f32 * 0.1) as i32).max(2);
        let body_ry = ((h as f32 * 0.18) as i32).max(1);
        let steps = (body_rx * 6 + body_ry * 3).max(10) as usize;
        for s in 0..=steps {
            let angle = s as f32 / steps as f32 * 2.0 * PI;
            let bx = cx + (angle.cos() * body_rx as f32).round() as i32;
            let by = mid_y + (angle.sin() * body_ry as f32).round() as i32;
            draw::dot_i(grid, bx, by);
        }

        // Horse head & neck.
        let neck_x = cx + body_rx;
        let neck_y = mid_y - body_ry;
        draw::dot_i(grid, neck_x, neck_y);
        draw::dot_i(grid, neck_x + 1, neck_y - 1);
        draw::dot_i(grid, neck_x + 2, neck_y - 2);
        // Mane.
        let mane = ((ctx.time * 8.0).sin() * 1.5).round() as i32;
        draw::dot_i(grid, neck_x + 1, neck_y - 3 + mane);

        // Rider torso: a vertical dot column above body.
        let rider_x = cx;
        for dy in 0..=(body_ry + 2) {
            draw::dot_i(grid, rider_x, mid_y - dy);
        }
        // Rider head.
        draw::dot_i(grid, rider_x, mid_y - body_ry - 3);
        draw::dot_i(grid, rider_x + 1, mid_y - body_ry - 3);
        // Helmet wings: two small arcs.
        let helm_y = mid_y - body_ry - 4;
        let wing_beat = ((ctx.time * 5.0).sin() * 1.5).round() as i32;
        draw::dot_i(grid, rider_x - 2, helm_y - wing_beat.abs());
        draw::dot_i(grid, rider_x - 3, helm_y - 1 - wing_beat.abs());
        draw::dot_i(grid, rider_x + 3, helm_y - wing_beat.abs());
        draw::dot_i(grid, rider_x + 4, helm_y - 1 - wing_beat.abs());

        // Spear: long diagonal line extending forward and slightly upward.
        let spear_len = ((w as f32 * 0.25) as i32).max(4);
        for d in 1..=spear_len {
            let spear_x = cx + body_rx + d + 2;
            let spear_y = mid_y - body_ry - 1 - d / 4;
            draw::dot_i(grid, spear_x, spear_y);
        }
        // Spear tip flare.
        let tip_x = cx + body_rx + spear_len + 2;
        let tip_y = mid_y - body_ry - 1 - spear_len / 4;
        draw::dot_i(grid, tip_x + 1, tip_y - 1);
        draw::dot_i(grid, tip_x + 1, tip_y + 1);

        // Four galloping legs.
        let leg_phases = [0.0f32, PI, PI * 0.5, PI * 1.5];
        let leg_xs: [i32; 4] = [
            cx - body_rx + 1,
            cx - body_rx + 3,
            cx + body_rx - 3,
            cx + body_rx - 1,
        ];
        for (lx, phase) in leg_xs.iter().zip(leg_phases.iter()) {
            let cycle = (ctx.time * 7.0 + phase).sin();
            let knee_y = mid_y + body_ry + 2;
            let foot_y = knee_y + (2.0 + cycle * 2.0).round() as i32;
            let foot_x = lx + (cycle * 1.5).round() as i32;
            draw::dot_i(grid, *lx, mid_y + body_ry);
            draw::dot_i(grid, foot_x, knee_y);
            draw::dot_i(grid, foot_x, foot_y);
        }

        // Nordic sky tint.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = 1.0 - cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t * 0.8 + 0.1),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Yggdrasil Grows
// ---------------------------------------------------------------------------
// A vertical trunk grows upward from the bottom with eased progress.
// Branches fork left and right at several heights, extending proportionally.
// Roots mirror downward. Leaves shimmer with time on the branches.
// This is pure tree geometry, not a traversal or wave.

struct YggdrasilGrows;
impl ProgressStyle for YggdrasilGrows {
    fn name(&self) -> &str {
        "yggdrasil-grows"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Yggdrasil world-tree grows trunk, branches, and roots as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let ground_y = (h * 2 / 3) as i32; // ground line at 2/3 height

        // Trunk: grows upward from ground with progress.
        let trunk_reach = (h as i32 / 3).max(1);
        let trunk_h = (ctx.eased * trunk_reach as f32).round() as i32;
        for dy in 0..trunk_h {
            let trunk_y = ground_y - dy;
            // Trunk is 3 dots wide.
            draw::dot_i(grid, cx - 1, trunk_y);
            draw::dot_i(grid, cx, trunk_y);
            draw::dot_i(grid, cx + 1, trunk_y);
        }

        // Root system: grows downward.
        let root_depth = (h as i32 - ground_y - 1).max(1);
        let root_h = (ctx.eased * root_depth as f32).round() as i32;
        let root_count = 3usize;
        for ri in 0..root_count {
            let root_angle = -PI / 4.0 + (ri as f32 / (root_count - 1).max(1) as f32) * PI / 2.0;
            let sway_freq = 0.8 + ri as f32 * 0.3;
            for seg in 0..root_h {
                let t = seg as f32 / root_depth.max(1) as f32;
                let spread = (t * w as f32 * 0.15).round() as i32;
                let sway = ((ctx.time * sway_freq + ri as f32 * 1.1 + seg as f32 * 0.1).sin() * 0.8)
                    .round() as i32;
                let rx = cx + root_angle.sin() as i32 * spread + sway;
                let ry = ground_y + 1 + seg;
                draw::dot_i(grid, rx, ry);
            }
        }

        // Branches: three tiers, each extending proportionally with progress.
        let branch_tiers = [
            (0.3f32, 0.55f32, w as f32 * 0.35),
            (0.55, 0.75, w as f32 * 0.27),
            (0.75, 1.0, w as f32 * 0.18),
        ];
        for (tier_idx, &(lo_prog, hi_prog, max_branch_w)) in branch_tiers.iter().enumerate() {
            // How far this tier has grown (0..1 within its own progress window).
            let tier_frac = ((ctx.eased - lo_prog) / (hi_prog - lo_prog)).clamp(0.0, 1.0);
            if tier_frac <= 0.0 {
                continue;
            }

            // Branch root y on the trunk.
            let branch_root_y = ground_y
                - (tier_idx as f32 / branch_tiers.len() as f32 * trunk_reach as f32).round() as i32
                - 1;
            let branch_len = (tier_frac * max_branch_w).round() as i32;

            // Left branch: angle upward-left.
            for b in 0..=branch_len {
                let t = b as f32 / max_branch_w.max(1.0);
                let leaf_sway =
                    ((ctx.time * 1.5 + tier_idx as f32 + b as f32 * 0.2).sin() * t * 1.5).round()
                        as i32;
                let bx = cx - b;
                let by = branch_root_y - (t * h as f32 * 0.15).round() as i32 + leaf_sway;
                draw::dot_i(grid, bx, by);
                // Leaf cluster near the tip.
                if b > branch_len - 3 {
                    draw::dot_i(grid, bx - 1, by - 1 + leaf_sway);
                    draw::dot_i(grid, bx, by - 2 + leaf_sway);
                }
            }
            // Right branch: mirror.
            for b in 0..=branch_len {
                let t = b as f32 / max_branch_w.max(1.0);
                let leaf_sway =
                    ((ctx.time * 1.5 + tier_idx as f32 + b as f32 * 0.2).sin() * t * 1.5).round()
                        as i32;
                let bx = cx + b;
                let by = branch_root_y - (t * h as f32 * 0.15).round() as i32 + leaf_sway;
                draw::dot_i(grid, bx, by);
                if b > branch_len - 3 {
                    draw::dot_i(grid, bx + 1, by - 1 + leaf_sway);
                    draw::dot_i(grid, bx, by - 2 + leaf_sway);
                }
            }
        }

        // Nature gradient tint: root-brown at bottom, sky-blue at top.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 12. Ouroboros
// ---------------------------------------------------------------------------
// A snake traces a circular arc (the arc angle = eased × 2π). At progress=1
// the circle is complete (snake biting its tail). The snake head and body
// are drawn as dots along the arc; the tail end tapers. The whole circle
// slowly rotates with time for animation even when progress is static.

struct Ouroboros;
impl ProgressStyle for Ouroboros {
    fn name(&self) -> &str {
        "ouroboros"
    }
    fn theme(&self) -> &str {
        "mythology"
    }
    fn describe(&self) -> &str {
        "Ouroboros: a snake traces a circle that closes completely as progress reaches 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        // Radius fits within the smaller dimension.
        let rx = ((w / 2).saturating_sub(3).max(2)) as f32;
        let ry = ((h / 2).saturating_sub(2).max(1)) as f32;

        // Arc angle: 0 at start, 2π at complete. Plus slow rotation from time.
        let arc = ctx.eased * 2.0 * PI;
        let rot = ctx.time * 0.4; // slow rotation for constant animation

        // Body thickness scales slightly at head.
        let body_steps = ((arc / (2.0 * PI)) * 300.0).round() as usize + 1;
        let body_steps = body_steps.max(2);

        for s in 0..=body_steps {
            let t = s as f32 / body_steps.max(1) as f32;
            let angle = rot + t * arc;
            let sx = cx + (angle.cos() * rx).round() as i32;
            let sy = cy + (angle.sin() * ry).round() as i32;
            draw::dot_i(grid, sx, sy);

            // Head: extra thick dot cluster at the leading end.
            if s == body_steps {
                draw::dot_i(grid, sx + 1, sy);
                draw::dot_i(grid, sx, sy + 1);
                draw::dot_i(grid, sx - 1, sy);
                // Eye: one dot slightly inside the circle.
                let eye_x = cx + ((angle - 0.2).cos() * (rx - 2.0)).round() as i32;
                let eye_y = cy + ((angle - 0.2).sin() * (ry - 1.0)).round() as i32;
                draw::dot_i(grid, eye_x, eye_y);
            }

            // Tail taper: shed dots near the start.
            if s == 0 && arc < 2.0 * PI * 0.98 {
                // Tail tip: single dot only (no extra).
            } else if s < 3 {
                // Build up the tail with 2-dot thickness for most of the arc.
                draw::dot_i(grid, sx + 1, sy);
            }
        }

        // Scales: small perpendicular ticks along the body every ~20 steps.
        let scale_interval = 20usize;
        for s in (0..=body_steps).step_by(scale_interval.max(1)) {
            let t = s as f32 / body_steps.max(1) as f32;
            let angle = rot + t * arc;
            let sx = cx + (angle.cos() * rx).round() as i32;
            let sy = cy + (angle.sin() * ry).round() as i32;
            // Perpendicular direction: tangent rotated 90°.
            let perp_x = (-angle.sin()).signum() as i32;
            let perp_y = (angle.cos()).signum() as i32;
            draw::dot_i(grid, sx + perp_x, sy + perp_y);
        }

        // Mystic tint: shifts through the palette with time to show constant motion.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = ((cy_c as f32 / cells_h.max(1) as f32) + ctx.time * 0.04) % 1.0;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }
        Ok(())
    }
}
