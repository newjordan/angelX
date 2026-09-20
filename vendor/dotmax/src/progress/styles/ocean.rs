//! Ocean / aquatic progress bars.
//!
//! Ten animated braille bars inspired by the sea: tides, waves, bubbles,
//! fish, sonar, jellyfish, coral, seaweed, ripple interference, and a
//! deep-ocean depth gauge. Every bar uses `ctx.time` for continuous animation
//! and `ctx.eased` for fill/advance driven by progress.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

/// All styles in the `ocean` theme.
///
/// Returns ten boxed ocean-themed bars ready to be mixed into any registry
/// or rendered directly via [`ProgressStyle::render`].
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(RisingTide),
        Box::new(BubblesRising),
        Box::new(FishSwim),
        Box::new(WaveCrest),
        Box::new(SonarPing),
        Box::new(DepthGauge),
        Box::new(JellyfishPulse),
        Box::new(CoralReef),
        Box::new(Seaweed),
        Box::new(RippleInterference),
    ]
}

// ---------------------------------------------------------------------------
// 1. Rising Tide
// ---------------------------------------------------------------------------
// Water level climbs with eased progress; the surface is a live sine wave.

struct RisingTide;
impl ProgressStyle for RisingTide {
    fn name(&self) -> &str {
        "rising-tide"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "Water level rises with eased progress; animated sine-wave surface"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Water level: 0 = empty (bottom), h = full (top). eased controls it.
        let water_h = (ctx.eased * h as f32).round() as usize;
        let water_top = h.saturating_sub(water_h); // dot-y of the surface line

        // Fill body below the surface.
        if water_h > 1 {
            draw::fill_rect(grid, 0, water_top + 1, w, h.saturating_sub(water_top + 1));
        }

        // Animated surface wave — one row of sine-displaced dots.
        let amp = (h as f32 * 0.08).max(1.0);
        for x in 0..w {
            let phase = ctx.time * 2.5 + x as f32 * 0.35;
            let dy = (phase.sin() * amp).round() as i32;
            let sy = water_top as i32 + dy;
            draw::dot_i(grid, x as i32, sy);
        }

        // Tint: deep ocean blue at bottom, sea-foam cyan at surface.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let frac = 1.0 - cy as f32 / cells_h.max(1) as f32; // 0 at top, 1 at bottom
            let color = ctx.palette.sample(frac);
            let dot_y_of_cell = cy * 4;
            if dot_y_of_cell >= water_top {
                draw::tint_row(grid, cy, 0, ctx.width.saturating_sub(1), color);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Bubbles Rising
// ---------------------------------------------------------------------------
// Bubbles float upward with sinusoidal lateral wobble; spawn density ∝ progress.

struct BubblesRising;
impl ProgressStyle for BubblesRising {
    fn name(&self) -> &str {
        "bubbles-rising"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "Bubbles float upward with wobble; spawn density scales with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Number of bubbles tied to progress (minimum 1 while progress > 0).
        let n_bubbles =
            ((ctx.eased * 14.0).round() as usize).max(if ctx.progress > 0.0 { 1 } else { 0 });

        for i in 0..n_bubbles {
            // Each bubble has a fixed column origin spread across the width.
            let base_x = if n_bubbles == 1 {
                w / 2
            } else {
                (i * w) / n_bubbles + w / (n_bubbles * 2).max(1)
            };
            // Independent speed and phase per bubble via prime-ish offsets.
            let speed = 0.5 + (i as f32 * 0.17) % 0.8;
            let phase_offset = i as f32 * 1.3;
            // y travels 0 (top) to h (bottom) and wraps.
            let travel = (ctx.time * speed + phase_offset) % 1.0;
            let y = h.saturating_sub(1) - (travel * h as f32) as usize;
            // Lateral sine wobble.
            let wobble = ((ctx.time * 1.8 + i as f32 * 0.9).sin() * 2.0).round() as i32;
            let bx = base_x as i32 + wobble;
            // Draw a tiny 2×2 bubble circle (four corners).
            draw::dot_i(grid, bx, y as i32);
            draw::dot_i(grid, bx + 1, y as i32);
            draw::dot_i(grid, bx, y as i32 + 1);
            draw::dot_i(grid, bx + 1, y as i32 + 1);
        }

        // Tint the whole grid with a soft deep-ocean gradient.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.max(1) as f32;
            let color = ctx.palette.sample(1.0 - t * 0.6);
            draw::tint_row(grid, cy, 0, ctx.width.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Fish Swimming
// ---------------------------------------------------------------------------
// A fish advances with eased progress; its tail flicks with time.

struct FishSwim;
impl ProgressStyle for FishSwim {
    fn name(&self) -> &str {
        "fish-swim"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "A fish swims across with a flickering tail driven by time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = (h / 2) as i32;
        // Fish nose position.
        let head_x = (ctx.eased * w as f32).round() as i32;
        // Body length proportional to grid width, capped.
        let body_len = ((w as f32 * 0.25) as i32).max(4).min(w as i32 / 2);

        // Body: an ellipse-ish blob of dots (tall center, narrow ends).
        for bx in 0..body_len {
            let frac = bx as f32 / body_len.max(1) as f32;
            // Elliptical half-height: peaks at 0.5, tapers to 0 at ends.
            let half_h = ((frac * PI).sin() * (h as f32 * 0.35)).round() as i32;
            let dx = head_x - bx;
            draw::vline(
                grid,
                dx.max(0) as usize,
                (mid - half_h).max(0) as usize,
                (mid + half_h).min(h as i32 - 1) as usize,
            );
        }

        // Eye: single dot near the nose.
        draw::dot_i(grid, head_x - 1, mid - 1);

        // Tail: two angled lines that flick with time.
        let flick = (ctx.time * 6.0).sin() * (h as f32 * 0.25);
        let tail_x = head_x - body_len;
        let tail_tip_up = (mid - flick.round() as i32).clamp(0, h as i32 - 1);
        let tail_tip_dn = (mid + flick.round() as i32).clamp(0, h as i32 - 1);
        draw::vline(
            grid,
            tail_x.max(0) as usize,
            tail_tip_up as usize,
            mid as usize,
        );
        draw::vline(
            grid,
            tail_x.max(0) as usize,
            mid as usize,
            tail_tip_dn as usize,
        );

        // Wake bubbles trailing behind.
        if tail_x > 2 {
            for i in 0..3usize {
                let wt = (ctx.time * 3.0 + i as f32 * 1.1).fract();
                let wx = tail_x - 2 - (wt * 4.0) as i32;
                let wy = mid + ((ctx.time * 4.0 + i as f32).sin() * 1.5).round() as i32;
                draw::dot_i(grid, wx, wy);
            }
        }

        // Tint with palette across cell rows.
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
// 4. Wave Crest
// ---------------------------------------------------------------------------
// A tall sine wave rides across the grid; its amplitude and position track
// eased progress, with additional time-driven ripple.

struct WaveCrest;
impl ProgressStyle for WaveCrest {
    fn name(&self) -> &str {
        "wave-crest"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "A rolling sine-wave crest advances with progress; fill trails behind"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = h as f32 / 2.0;
        let amp = h as f32 * 0.4;
        // The crest center (in dot-x) tracks eased progress.
        let crest_center = (ctx.eased * w as f32) as i32;

        for x in 0..w {
            // Phase: combination of crest position and time scroll.
            let phase = (x as f32 - crest_center as f32) * 0.28 - ctx.time * 2.8;
            let envelope = {
                // Gaussian-ish envelope peaking at the crest, fading in front.
                let dist = (x as f32 - crest_center as f32) / w as f32 * 4.0;
                (-dist * dist).exp()
            };
            let wave_y = mid + phase.sin() * amp * envelope;
            let iy = wave_y.round() as i32;

            // Draw a vertical smear around the wave height for thickness.
            for dy in -1i32..=1 {
                draw::dot_i(grid, x as i32, iy + dy);
            }

            // Fill water below the wave for x < crest.
            if x < crest_center.max(0) as usize {
                let floor = (wave_y.round() as usize).min(h - 1);
                draw::vline(grid, x, floor, h - 1);
            }
        }

        // Palette tint: columns behind crest get deeper hue.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let filled_cx = (ctx.eased * ctx.width as f32) as usize;
            if filled_cx > 0 {
                let t = cy as f32 / cells_h.max(1) as f32;
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    filled_cx.saturating_sub(1),
                    ctx.palette.sample(t),
                );
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Sonar Ping
// ---------------------------------------------------------------------------
// Concentric arcs expand from the left edge; arc count tracks eased progress.

struct SonarPing;
impl ProgressStyle for SonarPing {
    fn name(&self) -> &str {
        "sonar-ping"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "Expanding sonar arcs; ring count scales with eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = 0i32;
        let cy = (h / 2) as i32;
        // Number of active rings — progress controls how many appear.
        let max_rings = 6usize;
        let active_rings = ((ctx.eased * max_rings as f32).round() as usize).max(1);

        for ring_idx in 0..active_rings {
            // Each ring has an independent oscillating radius driven by time.
            let speed = 1.5 + ring_idx as f32 * 0.4;
            let phase = ctx.time * speed + ring_idx as f32 * 0.9;
            let radius = ((phase % (2.0 * PI)) / (2.0 * PI) * w as f32).round() as i32;

            // Draw a quarter-circle arc (right-facing semicircle from the origin).
            let steps = (radius * 3).max(8) as usize;
            for s in 0..steps {
                let angle = (s as f32 / steps as f32) * PI - PI / 2.0;
                let ax = cx + (angle.cos() * radius as f32).round() as i32;
                let ay = cy + (angle.sin() * radius as f32).round() as i32;
                draw::dot_i(grid, ax, ay);
            }
        }

        // Origin blip.
        draw::dot_i(grid, cx, cy);
        draw::dot_i(grid, cx + 1, cy);

        // Palette tint across rows.
        let (_, cells_h) = grid.dimensions();
        for cy_cell in 0..cells_h {
            let t = cy_cell as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_cell,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t * 0.7),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Depth Gauge
// ---------------------------------------------------------------------------
// A vertical depth gauge on the left fills downward with eased progress;
// tick marks label depth levels.

struct DepthGauge;
impl ProgressStyle for DepthGauge {
    fn name(&self) -> &str {
        "depth-gauge"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "Submarine depth gauge: fills downward with depth markers and tint"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Left vertical spine.
        draw::vline(grid, 1, 0, h - 1);

        // Fill downward: submarine goes deeper with progress.
        let filled_h = (ctx.eased * h as f32).round() as usize;
        draw::fill_rect(grid, 2, 0, (w / 3).max(1), filled_h);

        // Tick marks every ~20% down the spine.
        for i in 0..=5usize {
            let tick_y = (i as f32 / 5.0 * (h - 1) as f32).round() as usize;
            let tick_len = if i % 5 == 0 { 5usize } else { 3 };
            draw::hline(grid, 0, tick_len.min(w - 1), tick_y);
        }

        // Animated pressure bubbles racing up the right side.
        for i in 0..4usize {
            let bspeed = 0.6 + i as f32 * 0.15;
            let bt = (ctx.time * bspeed + i as f32 * 0.7) % 1.0;
            let by = h.saturating_sub(1) - (bt * h as f32) as usize;
            let bx = (w as i32 - 3) + (((ctx.time * 2.0 + i as f32).sin()) * 1.5) as i32;
            draw::dot_i(grid, bx, by as i32);
            draw::dot_i(grid, bx + 1, by as i32);
        }

        // Depth-gradient tint: lighter near surface, darkening with depth.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, ctx.width.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Jellyfish Pulse
// ---------------------------------------------------------------------------
// A jellyfish bell expands and contracts via time; it drifts upward with
// eased progress. Trailing tentacles sway with sine.

struct JellyfishPulse;
impl ProgressStyle for JellyfishPulse {
    fn name(&self) -> &str {
        "jellyfish-pulse"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "A pulsing jellyfish bell drifts upward; tentacles sway in the current"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        // Bell center y: starts near bottom, rises with progress.
        let bell_y = (h as f32 * (1.0 - ctx.eased * 0.85)).round() as i32;
        // Bell radius pulses with time.
        let base_r = (w.min(h) as f32 * 0.22).max(3.0);
        let pulse = 1.0 + 0.25 * (ctx.time * 3.5).sin();
        let r = (base_r * pulse).round() as i32;
        let r_half = (r as f32 * 0.55).round() as i32;

        // Draw the dome (upper semicircle arc).
        let steps = (r * 6).max(12) as usize;
        for s in 0..=steps {
            let angle = s as f32 / steps as f32 * PI; // 0..=π (top half)
            let ax = cx + (angle.cos() * r as f32).round() as i32;
            let ay = bell_y - (angle.sin() * r_half as f32).round() as i32;
            draw::dot_i(grid, ax, ay);
        }
        // Flat base of bell.
        draw::hline(
            grid,
            (cx - r).max(0) as usize,
            (cx + r).min(w as i32 - 1) as usize,
            bell_y as usize,
        );

        // Interior pulsing fill (inner arc a bit smaller).
        let r2 = (r as f32 * 0.6).round() as i32;
        for s in 0..=steps {
            let angle = s as f32 / steps as f32 * PI;
            let ax = cx + (angle.cos() * r2 as f32).round() as i32;
            let ay = bell_y - (angle.sin() * (r_half as f32 * 0.6)).round() as i32;
            draw::dot_i(grid, ax, ay);
        }

        // Tentacles hanging below the bell base.
        let n_tent = 5usize;
        for t_idx in 0..n_tent {
            let tx_base = cx - r + (t_idx as i32 * (r * 2) / n_tent.max(1) as i32);
            let sway_amp = 2.0f32;
            let sway_freq = 1.8 + t_idx as f32 * 0.3;
            let tent_len = (h as f32 * 0.35).round() as i32;
            for seg in 0..tent_len {
                let sway =
                    (ctx.time * sway_freq + seg as f32 * 0.3 + t_idx as f32).sin() * sway_amp;
                let tx = tx_base + sway.round() as i32;
                let ty = bell_y + 1 + seg;
                draw::dot_i(grid, tx, ty);
            }
        }

        // Soft bioluminescence tint — shifting through the palette with time.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = ((cy as f32 / cells_h.max(1) as f32) + ctx.time * 0.05) % 1.0;
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
// 8. Coral Reef
// ---------------------------------------------------------------------------
// Branching coral grows upward from the seafloor; branch count ∝ progress.
// Fronds sway gently with time.

struct CoralReef;
impl ProgressStyle for CoralReef {
    fn name(&self) -> &str {
        "coral-reef"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "Coral branches grow from the seafloor; sway and growth track progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Seafloor baseline.
        draw::hline(grid, 0, w - 1, h - 1);

        // Number of coral stalks from 1 up to ~10 based on progress.
        let n_stalks = ((ctx.eased * 10.0).round() as usize).max(1);

        for s in 0..n_stalks {
            let sx = (s * w) / n_stalks + w / (n_stalks * 2).max(1);
            // Each stalk height proportional to progress and staggered.
            let height_frac = ctx.eased * (0.5 + (s as f32 * 0.23) % 0.5);
            let stalk_h = (height_frac * h as f32 * 0.85).round() as usize;
            let stalk_base = h - 1;
            let stalk_top = stalk_base.saturating_sub(stalk_h);

            // Sway: gentle lateral sine on the whole stalk.
            let sway_phase = ctx.time * 1.2 + s as f32 * 0.8;

            for seg in 0..stalk_h {
                let seg_y = stalk_base - seg;
                let sway =
                    ((sway_phase + seg as f32 * 0.15).sin() * seg as f32 * 0.06).round() as i32;
                draw::dot_i(grid, sx as i32 + sway, seg_y as i32);
            }

            // Branch fronds at 1/3 and 2/3 of stalk height.
            for &branch_frac in &[0.33f32, 0.66] {
                let bseg = (stalk_h as f32 * branch_frac).round() as usize;
                if bseg == 0 {
                    continue;
                }
                let by = stalk_base.saturating_sub(bseg);
                let bsway =
                    ((sway_phase + bseg as f32 * 0.15).sin() * bseg as f32 * 0.06).round() as i32;
                let bx = sx as i32 + bsway;
                let branch_len = (stalk_h as f32 * 0.25).round() as i32;
                // Left and right branches.
                for bl in 1..=branch_len {
                    draw::dot_i(grid, bx - bl, by as i32 - bl / 2);
                    draw::dot_i(grid, bx + bl, by as i32 - bl / 2);
                }
            }

            // Tip dot.
            let sway_tip =
                ((sway_phase + stalk_h as f32 * 0.15).sin() * stalk_h as f32 * 0.06).round() as i32;
            draw::dot_i(grid, sx as i32 + sway_tip, stalk_top as i32);
        }

        // Warm reef tint.
        let (_, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let t = 1.0 - cy as f32 / cells_h.max(1) as f32; // deeper = more saturated
            draw::tint_row(
                grid,
                cy,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t * 0.8),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Seaweed
// ---------------------------------------------------------------------------
// Vertical fronds grow from the seafloor, swaying with time; filled width
// tracks eased progress.

struct Seaweed;
impl ProgressStyle for Seaweed {
    fn name(&self) -> &str {
        "seaweed"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "Seaweed fronds grow and sway; column fill tracks eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Number of fronds filling from left.
        let filled_w = (ctx.eased * w as f32).round() as usize;
        let n_fronds = (filled_w / 2).max(if ctx.progress > 0.0 { 1 } else { 0 });

        for fi in 0..n_fronds {
            let fx = (fi * filled_w) / n_fronds.max(1);
            let sway_freq = 1.0 + (fi as f32 * 0.31) % 0.8;
            let height_frac = 0.5 + (fi as f32 * 0.19) % 0.5;
            let frond_h = (height_frac * h as f32).round() as usize;

            for seg in 0..frond_h {
                let seg_y = h - 1 - seg;
                // Sway increases toward tip.
                let sway_amp = seg as f32 / frond_h.max(1) as f32 * 3.0;
                let sway = ((ctx.time * sway_freq + fi as f32 * 0.7 + seg as f32 * 0.2).sin()
                    * sway_amp)
                    .round() as i32;
                draw::dot_i(grid, fx as i32 + sway, seg_y as i32);

                // Alternating side leaflets every 4 segments.
                if seg % 4 == 2 {
                    let leaf_side: i32 = if seg % 8 < 4 { 1 } else { -1 };
                    draw::dot_i(grid, fx as i32 + sway + leaf_side, seg_y as i32 - 1);
                }
            }
        }

        // Seafloor.
        draw::hline(grid, 0, filled_w.min(w - 1), h - 1);

        // Tint only the filled portion.
        let (_, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * ctx.width as f32).round() as usize;
        for cy in 0..cells_h {
            let t = cy as f32 / cells_h.max(1) as f32;
            if filled_cells > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    filled_cells.saturating_sub(1),
                    ctx.palette.sample(0.3 + t * 0.5),
                );
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Ripple Interference
// ---------------------------------------------------------------------------
// Two sinusoidal wave sources produce an interference pattern; source
// separation tracks eased progress.

struct RippleInterference;
impl ProgressStyle for RippleInterference {
    fn name(&self) -> &str {
        "ripple-interference"
    }
    fn theme(&self) -> &str {
        "ocean"
    }
    fn describe(&self) -> &str {
        "Two point sources create animated ripple interference patterns"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Source positions: start centered, spread apart with progress.
        let cx = (w / 2) as f32;
        let cy = (h / 2) as f32;
        let spread = ctx.eased * (w as f32 * 0.35);
        let s1x = cx - spread;
        let s2x = cx + spread;

        for py in 0..h {
            for px in 0..w {
                let pxf = px as f32;
                let pyf = py as f32;
                // Distance from each source.
                let d1 = ((pxf - s1x).powi(2) + (pyf - cy).powi(2)).sqrt();
                let d2 = ((pxf - s2x).powi(2) + (pyf - cy).powi(2)).sqrt();
                // Superposition of two circular waves.
                let wave = (d1 * 0.6 - ctx.time * 4.0).sin() + (d2 * 0.6 - ctx.time * 4.0).sin();
                // Dot when combined amplitude exceeds threshold.
                if wave > 1.2 {
                    draw::dot(grid, px, py);
                }
            }
        }

        // Tint smoothly across both axes.
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
