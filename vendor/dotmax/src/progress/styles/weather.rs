//! Weather / meteorology progress bars.
//!
//! Twelve animated styles built entirely from `draw::` helpers. Each bar reads
//! `ctx.eased` for its fill/intensity amount and `ctx.time` for looping
//! animation, so they stay alive even when progress is held constant. Every
//! style targets a distinct atmospheric phenomenon — hurricane spirals, tornado
//! funnels, accumulating snowdrifts, hailstreaks, rainbow arcs, fog density,
//! barometer needles, windsock gusts, blizzard whiteout, cumulonimbus
//! convection, frost crystal propagation, and a mercury thermometer column.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ── registry ──────────────────────────────────────────────────────────────────

/// All styles in the `weather` theme.
///
/// Each style is a structurally distinct weather phenomenon — spiral arms,
/// funnel geometry, accumulation physics, optical arcs, density fields,
/// analog instruments, fluid flow, and crystal growth. Color is not the
/// distinguishing axis; the drawn geometry is.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Hurricane),
        Box::new(Tornado),
        Box::new(SnowAccumulation),
        Box::new(Hailstorm),
        Box::new(RainbowArc),
        Box::new(FogRollIn),
        Box::new(Barometer),
        Box::new(WindSock),
        Box::new(BlizzardWhiteout),
        Box::new(Cumulonimbus),
        Box::new(FrostCrystals),
        Box::new(Thermometer),
    ]
}

// ── deterministic hash ────────────────────────────────────────────────────────

/// Fast integer hash used for stable per-particle position seeds.
#[inline]
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

// ── 1. Hurricane ─────────────────────────────────────────────────────────────

/// Rotating spiral arms with a calm eye, intensity scales with `eased`.
struct Hurricane;
impl ProgressStyle for Hurricane {
    fn name(&self) -> &str {
        "hurricane"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Spiral arms rotate around a calm eye; intensity and radius grow with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        // Max radius shrinks slightly from the edge.
        let max_rx = (w / 2).saturating_sub(1) as f32;
        let max_ry = (h / 2).saturating_sub(1) as f32;
        let intensity = ctx.eased;

        // Eye: a small empty circle, radius grows slightly with progress.
        let eye_r = (1.0 + intensity * 2.0) as i32;

        // Four spiral arms, each offset by PI/2.
        let arms = 4usize;
        let arm_turns = 1.5_f32;
        let steps = 80usize;
        for arm in 0..arms {
            let arm_offset = arm as f32 * 2.0 * PI / arms as f32;
            for s in 0..steps {
                let t = s as f32 / steps as f32;
                let r_frac = t * intensity; // arm length scales with eased
                let angle = arm_offset + t * arm_turns * 2.0 * PI + ctx.time * 1.8;
                let rx = max_rx * r_frac;
                let ry = max_ry * r_frac;
                let px = cx + (angle.cos() * rx) as i32;
                let py = cy + (angle.sin() * ry) as i32;
                // Skip eye region.
                let dist2 = (px - cx) * (px - cx) + (py - cy) * (py - cy);
                if dist2 > eye_r * eye_r {
                    draw::dot_i(grid, px, py);
                }
            }
        }

        // Eye wall: ring at eye radius.
        let eye_steps = 32usize;
        for s in 0..eye_steps {
            let a = s as f32 / eye_steps as f32 * 2.0 * PI;
            let ex = cx + (a.cos() * eye_r as f32) as i32;
            let ey = cy + (a.sin() * eye_r as f32) as i32;
            draw::dot_i(grid, ex, ey);
        }

        // Color: palette across rows.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * intensity);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 2. Tornado ───────────────────────────────────────────────────────────────

/// Tapering funnel that widens toward the ground; debris vortices swirl.
struct Tornado;
impl ProgressStyle for Tornado {
    fn name(&self) -> &str {
        "tornado"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Tapering funnel widens toward the ground; debris particles orbit the vortex"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Funnel centre x drifts a little with time — the wobble.
        let base_cx = (w / 2) as f32;
        let wobble = (ctx.time * 1.3).sin() * (w as f32 * 0.06).max(1.0);
        let cx = (base_cx + wobble) as i32;

        // Funnel occupies `eased` fraction of height from the top.
        let funnel_bottom = (ctx.eased * h as f32) as usize;

        // At each row y, funnel half-width grows linearly: 1 at top, max at bottom.
        let max_hw = (w as f32 * 0.35).max(2.0);

        for y in 0..funnel_bottom.min(h) {
            let frac = if funnel_bottom <= 1 {
                1.0f32
            } else {
                y as f32 / (funnel_bottom - 1) as f32
            };
            let hw = (frac * max_hw) as i32;
            // Left and right edges of the funnel.
            draw::dot_i(grid, cx - hw, y as i32);
            draw::dot_i(grid, cx + hw, y as i32);
            // Ground touchdown: fill the bottom ring solid.
            if y + 1 >= funnel_bottom {
                for dx in -hw..=hw {
                    draw::dot_i(grid, cx + dx, y as i32);
                }
            }
        }

        // Debris: 8 particles orbiting at different radii and speeds.
        let debris_count = 8usize;
        for d in 0..debris_count {
            let h_val = hash(d as u32);
            let orbit_r_frac = 0.3 + (h_val & 0xFF) as f32 / 255.0 * 0.5; // 0.3..0.8
            let orbit_rx = max_hw * orbit_r_frac * 1.4;
            let orbit_ry = (h as f32 * 0.18).max(1.0) * orbit_r_frac;
            let h2 = hash(h_val);
            let orbit_y_frac = (h2 & 0xFF) as f32 / 255.0;
            let orbit_cy = (orbit_y_frac * funnel_bottom.max(1) as f32) as i32;
            let speed = 2.0 + (h_val >> 8 & 0xFF) as f32 / 255.0 * 4.0;
            let phase = d as f32 / debris_count as f32 * 2.0 * PI;
            let angle = ctx.time * speed + phase;
            let dx = (angle.cos() * orbit_rx) as i32 + cx;
            let dy = orbit_cy + (angle.sin() * orbit_ry) as i32;
            draw::dot_i(grid, dx, dy);
        }

        // Tint: palette from top (pale) to ground (dark).
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 3. Snow accumulation ─────────────────────────────────────────────────────

/// Flakes drift down; a drift pile rises from the bottom with `eased`.
struct SnowAccumulation;
impl ProgressStyle for SnowAccumulation {
    fn name(&self) -> &str {
        "snow-accumulation"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Snowflakes drift downward while a snowdrift pile rises from the bottom with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Drift pile: solid fill rising from the bottom.
        let drift_h = (ctx.eased * h as f32).round() as usize;
        let drift_top = h.saturating_sub(drift_h);
        if drift_h > 0 {
            // Curved top surface: sine bump.
            for x in 0..w {
                let bump =
                    ((x as f32 / w.max(1) as f32 * PI * 2.0).sin() * (h as f32 * 0.06)) as i32;
                let top = (drift_top as i32 - bump).max(0) as usize;
                draw::vline(grid, x, top, h.saturating_sub(1));
            }
        }

        // Falling flakes: 12 flakes, each with own horizontal lane & phase.
        let flake_count = 12usize;
        for f in 0..flake_count {
            let h_val = hash(f as u32);
            let lane_x = (h_val & 0xFFFF) as f32 / 65535.0 * (w.saturating_sub(1)) as f32;
            let speed = 0.6 + (h_val >> 16 & 0xFF) as f32 / 255.0 * 0.8;
            let phase = f as f32 / flake_count as f32;
            // Drift stops above the pile.
            let fall_space = drift_top.saturating_sub(1);
            if fall_space == 0 {
                continue;
            }
            let cycle = (ctx.time * speed + phase).fract();
            let fy = (cycle * fall_space as f32) as usize;
            // Horizontal sway.
            let sway = ((ctx.time * 1.2 + phase * 7.3).sin() * (w as f32 * 0.04)).round() as i32;
            let fx = (lane_x as i32 + sway).clamp(0, w.saturating_sub(1) as i32);
            draw::dot_i(grid, fx, fy as i32);
        }

        // Tint: cool blue for drift, lighter above.
        let (cw, ch) = grid.dimensions();
        let drift_top_cell = drift_top / 4;
        for cy_c in 0..ch {
            let t = if cy_c >= drift_top_cell {
                (cy_c - drift_top_cell) as f32 / ch.saturating_sub(1).max(1) as f32
            } else {
                0.0
            };
            let color = ctx.palette.sample(t * 0.6 + 0.4 * ctx.eased);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 4. Hailstorm ─────────────────────────────────────────────────────────────

/// Fast vertical streaks that hit the ground and bounce.
struct Hailstorm;
impl ProgressStyle for Hailstorm {
    fn name(&self) -> &str {
        "hailstorm"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Dense hailstones streak straight down and bounce on impact; density ramps with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Number of active stones grows with progress.
        let stone_count = (2.0 + ctx.eased * 20.0).round() as usize;

        for s in 0..stone_count {
            let h_val = hash(s as u32);
            // Horizontal position: fixed per stone.
            let sx = (h_val & 0xFFFF) as f32 / 65535.0 * (w.saturating_sub(1)) as f32;
            let speed = 1.8 + (h_val >> 16 & 0xFF) as f32 / 255.0 * 2.5;
            let phase = s as f32 / stone_count.max(1) as f32;

            // Vertical: linear fall + bounce near bottom.
            let cycle = (ctx.time * speed + phase).fract();
            // Bounce: reflect off the bottom — fold the cycle around 0.5.
            let bounce_t = if cycle < 0.85 {
                cycle / 0.85
            } else {
                // Short bounce-up.
                1.0 - (cycle - 0.85) / 0.15 * 0.25
            };
            let sy = (bounce_t * (h.saturating_sub(1)) as f32) as i32;
            let sx_i = sx as i32;

            // Stone: a short vertical streak (2-3 dots).
            draw::dot_i(grid, sx_i, sy);
            draw::dot_i(grid, sx_i, sy - 1);

            // Impact flash at the very bottom when stone lands.
            if bounce_t > 0.95 {
                let flash_w = 3i32;
                for dx in -flash_w..=flash_w {
                    draw::dot_i(grid, sx_i + dx, (h.saturating_sub(1)) as i32);
                }
            }
        }

        // Ground line.
        draw::hline(grid, 0, w.saturating_sub(1), h.saturating_sub(1));

        // Cold-grey tint.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(1.0 - t * 0.5);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 5. Rainbow arc ───────────────────────────────────────────────────────────

/// Concentric colour arcs drawn band by band as progress increases.
struct RainbowArc;
impl ProgressStyle for RainbowArc {
    fn name(&self) -> &str {
        "rainbow-arc"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Concentric rainbow arcs appear band by band from the horizon as progress grows"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Arc centred at the bottom-centre.
        let cx = (w / 2) as i32;
        let base_y = h as i32;

        // Seven bands of the rainbow; each appears when progress passes its threshold.
        let bands = 7usize;
        let max_ry = (h.saturating_sub(1)).max(1) as f32;
        let max_rx = (w / 2).saturating_sub(0) as f32;

        use crate::Color;
        let band_colors: [Color; 7] = [
            Color::rgb(148, 0, 211), // violet
            Color::rgb(75, 0, 130),  // indigo
            Color::rgb(0, 0, 255),   // blue
            Color::rgb(0, 128, 0),   // green
            Color::rgb(255, 255, 0), // yellow
            Color::rgb(255, 127, 0), // orange
            Color::rgb(255, 0, 0),   // red (outermost)
        ];

        for band in 0..bands {
            // Band `band` becomes visible when progress crosses its threshold.
            let threshold = band as f32 / bands as f32;
            if ctx.eased < threshold {
                continue;
            }

            // Partial reveal on the current outermost visible band.
            let band_frac = ((ctx.eased - threshold) * bands as f32).clamp(0.0, 1.0);

            // Radius fraction for this band: innermost (band 0) is smallest.
            let r_frac = (band + 1) as f32 / bands as f32;
            let rx = (max_rx * r_frac) as i32;
            let ry = (max_ry * r_frac) as i32;

            // Draw a semicircle arc (angles PI to 0, i.e. left horizon to right).
            let steps = (rx.max(ry) * 4).max(32) as usize;
            let arc_steps = (steps as f32 * band_frac).round() as usize;
            for s in 0..arc_steps {
                // Angle from PI (left) to 0 (right) so it draws left→right.
                let a = PI - s as f32 / steps.max(1) as f32 * PI;
                let px = cx + (a.cos() * rx as f32) as i32;
                let py = base_y - (a.sin() * ry as f32) as i32;
                draw::dot_i(grid, px, py);
            }

            // Apply band color.
            let color = band_colors[band];
            let (cw, ch) = grid.dimensions();
            for cy_c in 0..ch {
                draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
            }
        }

        // Re-tint with palette sampled at eased to blend the per-band colors.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(1.0 - t);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 6. Fog rolling in ────────────────────────────────────────────────────────

/// Shade-glyph density sweeps left-to-right; denser at the leading edge.
struct FogRollIn;
impl ProgressStyle for FogRollIn {
    fn name(&self) -> &str {
        "fog-roll-in"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Dense fog rolls in from the left; the leading edge thickens with a shade-glyph gradient"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        let fog_front = (ctx.eased * cw as f32) as usize;

        for cx in 0..cw {
            // How deeply into the fog are we? 0=clear, 1=deep fog.
            if cx >= fog_front {
                break;
            }
            let depth = (fog_front - cx) as f32 / fog_front.max(1) as f32;
            // Add a slow undulation from time.
            let wave = ((cx as f32 * 0.5 + ctx.time * 1.5).sin() * 0.12 + 1.0).clamp(0.5, 1.5);
            let density = (depth * wave).clamp(0.0, 1.0);

            // Map density [0..1] → shade level [0..4].
            let level = (density * 4.0).round() as usize;

            for cy in 0..ch {
                // Row-vary the fog slightly — ground-level is denser.
                let row_extra = cy as f32 / ch.saturating_sub(1).max(1) as f32 * 0.5;
                let row_level = ((density + row_extra).clamp(0.0, 1.0) * 4.0).round() as usize;
                draw::shade(grid, cx, cy, row_level.min(4));
                let _ = level; // used above for clarity
            }
        }

        // Tint the fog grey-blue via palette.
        let (cw2, ch2) = grid.dimensions();
        for cy_c in 0..ch2 {
            let color = ctx.palette.sample(ctx.eased * 0.5);
            draw::tint_row(grid, cy_c, 0, fog_front.min(cw2).saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 7. Barometer ─────────────────────────────────────────────────────────────

/// An analog pressure dial with a rotating needle driven by `eased`.
struct Barometer;
impl ProgressStyle for Barometer {
    fn name(&self) -> &str {
        "barometer"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Analog pressure gauge: a needle sweeps from STORMY to FAIR as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let max_r = (w.min(h * 2) / 2).saturating_sub(2) as i32;

        // Arc: semicircle from left (STORMY) to right (FAIR), opening downward.
        // Angles: PI (left) down to 0 (right) sweeping through 0..PI (top half).
        let arc_steps = 60usize;
        for s in 0..=arc_steps {
            let a = PI - s as f32 / arc_steps as f32 * PI;
            let px = cx + (a.cos() * max_r as f32) as i32;
            let py = cy - (a.sin() * max_r as f32) as i32; // minus: up
            draw::dot_i(grid, px, py);
        }

        // Tick marks at 0%, 25%, 50%, 75%, 100% of the arc.
        for tick in 0..=4 {
            let a = PI - tick as f32 / 4.0 * PI;
            let inner = (max_r - 3).max(1);
            let outer = max_r;
            for r in inner..=outer {
                let px = cx + (a.cos() * r as f32) as i32;
                let py = cy - (a.sin() * r as f32) as i32;
                draw::dot_i(grid, px, py);
            }
        }

        // Needle: points from centre toward the arc, angle driven by eased.
        // eased=0 → left (PI), eased=1 → right (0).
        let needle_angle = PI - ctx.eased * PI;
        // Slow pressure tremor.
        let tremor = (ctx.time * 6.0).sin() * 0.03;
        let needle_angle = needle_angle + tremor;
        let needle_len = (max_r - 2).max(1) as f32;
        let steps = needle_len.round() as usize;
        for s in 0..=steps {
            let r = s as f32;
            let px = cx + (needle_angle.cos() * r) as i32;
            let py = cy - (needle_angle.sin() * r) as i32;
            draw::dot_i(grid, px, py);
        }

        // Centre pivot.
        draw::dot_i(grid, cx, cy);
        draw::dot_i(grid, cx + 1, cy);

        // Tint: warm high-pressure orange at right, stormy blue at left.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let color = ctx.palette.sample(ctx.eased);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 8. Wind sock ─────────────────────────────────────────────────────────────

/// A conical sock extends right; gust streak lines sweep across behind it.
struct WindSock;
impl ProgressStyle for WindSock {
    fn name(&self) -> &str {
        "wind-sock"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "A windsock cone extends as gusts blow; streak lines visualise airspeed"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = (h / 2) as i32;
        // Sock extends from left pole to eased * width.
        let sock_len = (ctx.eased * (w.saturating_sub(2)) as f32).round() as usize;
        let pole_x = 0i32;

        // Pole: vertical line at x=0.
        draw::vline(grid, 0, 0, h.saturating_sub(1));

        // Horizontal attachment rod (top half of pole).
        let attach_y = (h / 4) as i32;
        draw::dot_i(grid, pole_x, attach_y);

        // Sock: tapered cone — wide at pole (opening_hw), narrow at tip (1).
        let opening_hw = (h as f32 * 0.38).max(1.0) as i32;
        for x in 0..sock_len.min(w.saturating_sub(1)) {
            let frac = if sock_len <= 1 {
                0.0
            } else {
                x as f32 / (sock_len - 1) as f32
            };
            // Wind flutter: opening oscillates slightly with time.
            let flutter = (ctx.time * 4.0 + x as f32 * 0.2).sin() * (opening_hw as f32 * 0.12);
            let hw = ((opening_hw as f32 * (1.0 - frac) + 1.0 + flutter) as i32).max(1);
            draw::dot_i(grid, pole_x + x as i32 + 1, mid - hw);
            draw::dot_i(grid, pole_x + x as i32 + 1, mid + hw);
        }
        // Tip dot.
        if sock_len > 0 {
            draw::dot_i(grid, pole_x + sock_len as i32, mid);
        }

        // Gust streaks: horizontal lines scrolling left-to-right.
        let gust_count = 5usize;
        for g in 0..gust_count {
            let h_val = hash(g as u32);
            let gust_y = (h_val & 0xFF) as f32 / 255.0 * h as f32;
            let speed = 1.2 + (h_val >> 8 & 0xFF) as f32 / 255.0 * 2.0;
            let phase = g as f32 / gust_count as f32;
            // Streaks scroll from left, wrap around.
            let gust_x_frac = (ctx.time * speed + phase).fract();
            let gust_x = (gust_x_frac * w as f32) as usize;
            let gust_len = (w / 6).max(2);
            let x0 = gust_x;
            let x1 = (gust_x + gust_len).min(w.saturating_sub(1));
            draw::hline(grid, x0, x1, gust_y as usize);
        }

        // Tint.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.saturating_sub(1).max(1) as f32;
            let color = ctx.palette.sample(t * ctx.eased + ctx.eased * 0.3);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 9. Blizzard whiteout ─────────────────────────────────────────────────────

/// Shade density ramps toward full whiteout as progress approaches 1.
struct BlizzardWhiteout;
impl ProgressStyle for BlizzardWhiteout {
    fn name(&self) -> &str {
        "blizzard-whiteout"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Visibility collapses into whiteout: shade density ramps to full as progress reaches 1"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Base whiteout shade level driven by eased.
        let base_density = ctx.eased;

        for cx in 0..cw {
            for cy_c in 0..ch {
                // Add spatial noise and time flicker to break the uniformity.
                let h_val = hash((cx as u32).wrapping_mul(31).wrapping_add(cy_c as u32 * 97));
                let spatial_noise = (h_val & 0xFF) as f32 / 255.0 * 0.3;
                let flicker = (ctx.time * 8.0 + cx as f32 * 0.7 + cy_c as f32 * 1.3).sin() * 0.08;
                let density =
                    (base_density + spatial_noise * base_density + flicker).clamp(0.0, 1.0);
                let level = (density * 4.0).round() as usize;
                draw::shade(grid, cx, cy_c, level.min(4));
            }
        }

        // Particles: bright dots that blow horizontally through the whiteout.
        let (w, h) = draw::dot_dims(grid);
        let particle_count = (4.0 + ctx.eased * 16.0).round() as usize;
        for p in 0..particle_count {
            let h_val = hash(p as u32 + 500);
            let row_frac = (h_val & 0xFFFF) as f32 / 65535.0;
            let py = (row_frac * h.saturating_sub(1) as f32) as usize;
            let speed = 1.0 + (h_val >> 16 & 0xFF) as f32 / 255.0 * 3.0;
            let phase = p as f32 / particle_count.max(1) as f32;
            let px = ((ctx.time * speed + phase).fract() * w as f32) as usize;
            draw::dot(grid, px.min(w.saturating_sub(1)), py);
        }

        // Tint: cold white-blue.
        for cy_c in 0..ch {
            let color = ctx.palette.sample(ctx.eased * 0.3);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 10. Cumulonimbus ─────────────────────────────────────────────────────────

/// Cloud builds vertically into a thunderhead, then rain falls beneath.
struct Cumulonimbus;
impl ProgressStyle for Cumulonimbus {
    fn name(&self) -> &str {
        "cumulonimbus"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "A cloud tower builds upward into a thunderhead, then rain streaks fall below"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // The bar is divided: top 60% = cloud building, bottom 40% = rain.
        let cloud_zone = (h as f32 * 0.6) as usize;
        let rain_zone_top = cloud_zone;

        // Cloud anvil: top half fills with irregular lobes as progress grows.
        // Each lobe is an ellipse centred along the width at a fixed height.
        let lobe_count = 5usize;
        let cloud_height = (ctx.eased * cloud_zone as f32).round() as usize;

        for lobe in 0..lobe_count {
            let lobe_cx = (lobe as f32 + 0.5) / lobe_count as f32 * w as f32;
            // Lobes vary in width.
            let h_val = hash(lobe as u32 + 200);
            let lobe_rx = (w as f32 / lobe_count as f32 * 0.6
                + (h_val & 0xFF) as f32 / 255.0 * w as f32 * 0.08)
                .max(2.0);
            let lobe_ry = cloud_height as f32 * (0.5 + (h_val >> 8 & 0xFF) as f32 / 255.0 * 0.5);
            let lobe_cy = (cloud_zone.saturating_sub(1)) as f32;

            // Animate lobe tops with a slow boil.
            let boil = (ctx.time * 0.8 + lobe as f32 * 1.4).sin() * cloud_height as f32 * 0.04;

            let steps = 40usize;
            for s in 0..=steps {
                let a = s as f32 / steps as f32 * 2.0 * PI;
                let ex = (lobe_cx + a.cos() * lobe_rx + boil).round() as i32;
                let ey = (lobe_cy - (a.sin().abs() * lobe_ry)).round() as i32;
                // Only the top hemisphere (a.sin() >= 0 draws the fluffy top).
                if a.sin() >= 0.0 {
                    draw::dot_i(grid, ex, ey);
                }
                // Fill interior.
                if ey >= 0 && ey < cloud_zone as i32 {
                    let ey_fill = ey;
                    draw::dot_i(grid, ex, ey_fill);
                    draw::dot_i(grid, ex, lobe_cy as i32);
                    // Vertical fill from ey to base.
                    if ey < lobe_cy as i32 {
                        draw::vline(
                            grid,
                            ex.max(0) as usize,
                            ey.max(0) as usize,
                            lobe_cy as usize,
                        );
                    }
                }
            }
        }

        // Rain streaks below cloud zone — appear only after cloud is 50% built.
        if ctx.eased > 0.5 {
            let rain_intensity = ((ctx.eased - 0.5) * 2.0).clamp(0.0, 1.0);
            let rain_count = (2.0 + rain_intensity * 14.0).round() as usize;
            let rain_h = h.saturating_sub(rain_zone_top);
            if rain_h > 0 {
                for r in 0..rain_count {
                    let h_val = hash(r as u32 + 400);
                    let rx = (h_val & 0xFFFF) as f32 / 65535.0 * (w.saturating_sub(1)) as f32;
                    let speed = 1.5 + (h_val >> 16 & 0xFF) as f32 / 255.0 * 2.0;
                    let phase = r as f32 / rain_count.max(1) as f32;
                    let cycle = (ctx.time * speed + phase).fract();
                    let ry = rain_zone_top + (cycle * rain_h as f32) as usize;
                    draw::dot(grid, rx as usize, ry.min(h.saturating_sub(1)));
                    draw::dot(grid, rx as usize, (ry + 1).min(h.saturating_sub(1)));
                }
            }
        }

        // Tint: dark grey cloud, light blue rain.
        let (cw, ch) = grid.dimensions();
        let cloud_top_cell = 0;
        let rain_cell = (rain_zone_top / 4).min(ch.saturating_sub(1));
        for cy_c in cloud_top_cell..rain_cell.min(ch) {
            let color = ctx.palette.sample(0.3 * ctx.eased);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }
        for cy_c in rain_cell..ch {
            let color = ctx.palette.sample(ctx.eased * 0.7 + 0.3);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 11. Frost crystals ────────────────────────────────────────────────────────

/// Dendritic frost spreads inward from all four edges.
struct FrostCrystals;
impl ProgressStyle for FrostCrystals {
    fn name(&self) -> &str {
        "frost-crystals"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Dendritic frost crystals propagate inward from the edges; branching depth grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Frost depth in dots from each edge.
        let max_depth = ((w.min(h) / 2).saturating_sub(1)) as f32;
        let depth = (ctx.eased * max_depth).round() as usize;

        if depth == 0 {
            return Ok(());
        }

        // Grow dendritic crystals from left, right, top, bottom edges.
        // Each edge has a set of roots spaced every 4 dots along the edge.
        // From each root, a main spine grows inward; then side branches sprout
        // every few dots, each in the perpendicular direction.
        let branch_spacing = 4usize.max(1);
        let branch_len_frac = 0.4_f32;

        // Left edge.
        let mut root = 0usize;
        while root < h {
            let h_val = hash(root as u32 + 1000);
            // Slight y variation.
            let ry = root;
            for d in 0..depth.min(w) {
                draw::dot(grid, d, ry.min(h.saturating_sub(1)));
                // Side branches perpendicular (up/down).
                if d > 0 && d % branch_spacing == 0 {
                    let blen =
                        (depth as f32 * branch_len_frac * (h_val & 0xFF) as f32 / 255.0) as usize;
                    for b in 1..blen.min(h / 2) {
                        draw::dot(grid, d, (ry + b).min(h.saturating_sub(1)));
                        if ry >= b {
                            draw::dot(grid, d, ry - b);
                        }
                    }
                }
            }
            root += branch_spacing + (h_val >> 8 & 3) as usize;
        }

        // Right edge.
        root = 0;
        while root < h {
            let h_val = hash(root as u32 + 2000);
            let ry = root;
            for d in 0..depth.min(w) {
                let x = w.saturating_sub(1 + d);
                draw::dot(grid, x, ry.min(h.saturating_sub(1)));
                if d > 0 && d % branch_spacing == 0 {
                    let blen =
                        (depth as f32 * branch_len_frac * (h_val & 0xFF) as f32 / 255.0) as usize;
                    for b in 1..blen.min(h / 2) {
                        draw::dot(grid, x, (ry + b).min(h.saturating_sub(1)));
                        if ry >= b {
                            draw::dot(grid, x, ry - b);
                        }
                    }
                }
            }
            root += branch_spacing + (h_val >> 8 & 3) as usize;
        }

        // Top edge.
        root = 0;
        while root < w {
            let h_val = hash(root as u32 + 3000);
            let rx = root;
            for d in 0..depth.min(h) {
                draw::dot(grid, rx.min(w.saturating_sub(1)), d);
                if d > 0 && d % branch_spacing == 0 {
                    let blen =
                        (depth as f32 * branch_len_frac * (h_val & 0xFF) as f32 / 255.0) as usize;
                    for b in 1..blen.min(w / 2) {
                        draw::dot(grid, (rx + b).min(w.saturating_sub(1)), d);
                        if rx >= b {
                            draw::dot(grid, rx - b, d);
                        }
                    }
                }
            }
            root += branch_spacing + (h_val >> 8 & 3) as usize;
        }

        // Bottom edge.
        root = 0;
        while root < w {
            let h_val = hash(root as u32 + 4000);
            let rx = root;
            for d in 0..depth.min(h) {
                let y = h.saturating_sub(1 + d);
                draw::dot(grid, rx.min(w.saturating_sub(1)), y);
                if d > 0 && d % branch_spacing == 0 {
                    let blen =
                        (depth as f32 * branch_len_frac * (h_val & 0xFF) as f32 / 255.0) as usize;
                    for b in 1..blen.min(w / 2) {
                        draw::dot(grid, (rx + b).min(w.saturating_sub(1)), y);
                        if rx >= b {
                            draw::dot(grid, rx - b, y);
                        }
                    }
                }
            }
            root += branch_spacing + (h_val >> 8 & 3) as usize;
        }

        // Ice-blue tint.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let color = ctx.palette.sample(0.2 + ctx.eased * 0.6);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ── 12. Thermometer ──────────────────────────────────────────────────────────

/// A mercury column rises in a vertical thermometer tube.
struct Thermometer;
impl ProgressStyle for Thermometer {
    fn name(&self) -> &str {
        "thermometer"
    }
    fn theme(&self) -> &str {
        "weather"
    }
    fn describe(&self) -> &str {
        "Mercury column rises in a thermometer tube; bulb glows at bottom, tick marks show scale"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Thermometer shaft: centred horizontally, tube width = 4 dots.
        let cx = (w / 2) as i32;
        let tube_w = 4i32; // inner width
        let tube_left = cx - tube_w / 2;
        let tube_right = cx + tube_w / 2;

        // Tube occupies top 85% of the height; bulb is the bottom 15%.
        let bulb_top_y = (h as f32 * 0.82) as usize;
        let tube_top = 1usize;

        // Tube outline.
        for y in tube_top..bulb_top_y {
            draw::dot_i(grid, tube_left - 1, y as i32);
            draw::dot_i(grid, tube_right + 1, y as i32);
        }
        draw::hline(
            grid,
            tube_left.max(0) as usize,
            (tube_right + 1) as usize,
            tube_top,
        );

        // Bulb: filled circle at the bottom.
        let bulb_cx = cx;
        let bulb_cy = (bulb_top_y + h.saturating_sub(1)) as i32 / 2;
        let bulb_r = ((h.saturating_sub(bulb_top_y)) / 2).max(2) as i32;
        for dy in -bulb_r..=bulb_r {
            for dx in -bulb_r..=bulb_r {
                if dx * dx + dy * dy <= bulb_r * bulb_r + bulb_r {
                    draw::dot_i(grid, bulb_cx + dx, bulb_cy + dy);
                }
            }
        }

        // Mercury column: fills the tube from the bulb upward.
        let tube_h = bulb_top_y.saturating_sub(tube_top);
        let mercury_h = (ctx.eased * tube_h as f32).round() as usize;
        let mercury_top = bulb_top_y.saturating_sub(mercury_h);
        if mercury_h > 0 {
            for y in mercury_top..bulb_top_y {
                for x in tube_left..=tube_right {
                    draw::dot_i(grid, x, y as i32);
                }
            }
        }

        // Tick marks on the right side of the tube, every 25%.
        let tick_positions = [0.0, 0.25, 0.5, 0.75, 1.0];
        for &tp in &tick_positions {
            let ty = bulb_top_y.saturating_sub((tp * tube_h as f32).round() as usize);
            let tick_len = if (tp * 4.0).round() as usize % 2 == 0 {
                4i32
            } else {
                2i32
            };
            for dx in 0..tick_len {
                draw::dot_i(grid, tube_right + 2 + dx, ty as i32);
            }
        }

        // Mercury tint: cool blue at empty → hot red at full via palette.
        let (cw, ch) = grid.dimensions();
        let mercury_top_cell = (mercury_top / 4).min(ch.saturating_sub(1));
        let bulb_top_cell = (bulb_top_y / 4).min(ch.saturating_sub(1));
        for cy_c in mercury_top_cell..=bulb_top_cell.min(ch.saturating_sub(1)) {
            let color = ctx.palette.sample(ctx.eased);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}
