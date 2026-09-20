//! Aurora borealis progress bars — drifting light curtains and polar skies.
//!
//! Layered sine curtains, shimmer veils, and slow ribbons of light in a
//! teal → emerald → violet palette. Progress reads as light sweeping in
//! (left-to-right, bottom-up, or center-out) while `time` keeps every
//! curtain breathing. All motion is deterministic in `(progress, time)`.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::{PI, TAU};

// ─── deterministic hash ─────────────────────────────────────────────────────

/// Fast integer hash → `[0, 1)`.
#[inline]
fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) % 1000) as f32 / 1000.0
}

/// 3-D variant: hash `(x, y, z_int)` for time-slotted flicker.
#[inline]
fn hash3(x: i32, y: i32, z: i32) -> f32 {
    hash2(x ^ z.wrapping_mul(1_234_567), y ^ z.wrapping_mul(7_654_321))
}

// ─── theme tint — polar light ───────────────────────────────────────────────

/// Deep polar green at the dark end.
const AURORA_DEEP: Color = Color::rgb(14, 116, 86);
/// Bright glacial teal at the core of the lights.
const AURORA_TEAL: Color = Color::rgb(76, 227, 196);
/// High-altitude violet fringe.
const AURORA_VIOLET: Color = Color::rgb(167, 139, 250);

/// Sample the deep-green → teal → violet ramp at `t` in `0.0..=1.0`.
fn sample_tint(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8, k: f32| (f32::from(a) + (f32::from(b) - f32::from(a)) * k) as u8;
    if t < 0.55 {
        let k = t / 0.55;
        Color::rgb(
            lerp(AURORA_DEEP.r, AURORA_TEAL.r, k),
            lerp(AURORA_DEEP.g, AURORA_TEAL.g, k),
            lerp(AURORA_DEEP.b, AURORA_TEAL.b, k),
        )
    } else {
        let k = (t - 0.55) / 0.45;
        Color::rgb(
            lerp(AURORA_TEAL.r, AURORA_VIOLET.r, k),
            lerp(AURORA_TEAL.g, AURORA_VIOLET.g, k),
            lerp(AURORA_TEAL.b, AURORA_VIOLET.b, k),
        )
    }
}

/// Applies the polar-light ramp to every cell the inner style drew, hue
/// drifting slowly along the bar the way curtains wander across a sky.
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
                    let drift =
                        (x as f32 / w.max(1) as f32 * TAU * 0.5 + ctx.time * TAU * 0.25).sin();
                    let vert = 1.0 - y as f32 / h.max(1) as f32;
                    let _ = grid.set_cell_color(x, y, sample_tint(0.3 + 0.3 * drift + 0.3 * vert));
                }
            }
        }
        Ok(())
    }
}

/// All styles in the `aurora` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Curtain),
        Box::new(RibbonFlow),
        Box::new(BorealisFill),
        Box::new(Tinted(ShimmerVeil)),
        Box::new(PolarArc),
        Box::new(SolarWind),
        Box::new(IonStorm),
        Box::new(Zenith),
        Box::new(Corona),
        Box::new(NightSkyFill),
    ]
}

/// Curtains of light hang from the top edge, revealed left to right.
struct Curtain;
impl ProgressStyle for Curtain {
    fn name(&self) -> &str {
        "curtain"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "Hanging light curtains revealed across the sky"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        for x in 0..filled {
            let wave = (x as f32 * 0.18 + ctx.time * TAU * 0.25).sin();
            let flicker = 0.25 * hash3(x as i32, 0, (ctx.time * 4.0) as i32);
            let len = ((0.55 + 0.3 * wave + flicker) * h as f32) as usize;
            for y in 0..len.min(h) {
                draw::dot(grid, x, y);
            }
            let hue = 0.25 + 0.35 * wave + 0.25 * flicker;
            for cy in 0..grid.dimensions().1 {
                let fade = 1.0 - cy as f32 * 0.18;
                draw::tint_row(grid, cy, x / 2, x / 2, sample_tint(hue * fade + 0.15));
            }
        }
        Ok(())
    }
}

/// A thick ribbon of light weaves through the bar as it extends.
/// Layered aurora ribbon revealed from left to right.
pub struct RibbonFlow;
impl ProgressStyle for RibbonFlow {
    fn name(&self) -> &str {
        "ribbon-flow"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "A weaving ribbon of polar light"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        let mid = h as f32 / 2.0;
        let amp = (h as f32 / 2.0 - 2.5).max(0.5);
        for x in 0..filled {
            let phase = x as f32 * 0.11 + ctx.time * TAU * 0.25;
            let center = mid + amp * phase.sin() * (0.8 + 0.2 * (phase * 0.5).cos());
            let half = 2.0 + 0.9 * (x as f32 * 0.23 - ctx.time * TAU * 0.25).sin();
            let y0 = (center - half).max(0.0) as usize;
            let y1 = ((center + half) as usize).min(h.saturating_sub(1));
            for y in y0..=y1 {
                draw::dot(grid, x, y);
                let off = ((y as f32 - center) / half.max(0.1)).abs();
                let _ = grid.set_cell_color(x / 2, y / 4, sample_tint(0.55 - 0.35 * off + 0.3));
            }
            // Stray sparkles shed above and below the ribbon.
            if hash3(x as i32, 9, (ctx.time * 4.0) as i32) < 0.08 {
                let sy = (hash2(x as i32, 13) * h as f32) as usize;
                draw::dot(grid, x, sy.min(h - 1));
                let _ = grid.set_cell_color(x / 2, sy.min(h - 1) / 4, sample_tint(0.95));
            }
        }
        Ok(())
    }
}

/// Layered translucent light bands build a dense aurora fill.
struct BorealisFill;
impl ProgressStyle for BorealisFill {
    fn name(&self) -> &str {
        "borealis-fill"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "Layered light bands stacking into a dense fill"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        grid.enable_color_support();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for y in 0..ch {
            for x in 0..filled {
                let fx = x as f32;
                let fy = y as f32;
                let a = (fx * 0.35 + ctx.time * TAU * 0.25).sin();
                let b = (fx * 0.13 - fy * 0.9 + ctx.time * TAU * 0.5).sin();
                let c = (fx * 0.07 + fy * 1.3 + ctx.time * TAU * 0.25).cos();
                let intensity = (0.5 + 0.2 * a + 0.2 * b + 0.15 * c).clamp(0.0, 1.0);
                let level = 1 + (intensity * 3.2) as usize;
                draw::shade(grid, x, y, level.min(4));
                let _ = grid.set_cell_color(x, y, sample_tint(0.15 + 0.75 * intensity));
            }
        }
        Ok(())
    }
}

/// Thin rays flicker like a veil of light; density is the progress.
struct ShimmerVeil;
impl ProgressStyle for ShimmerVeil {
    fn name(&self) -> &str {
        "shimmer-veil"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "A flickering veil of thin light rays"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let filled = (ctx.eased * w as f32).round() as usize;
        let slot = (ctx.time * 6.0) as i32;
        for x in 0..filled {
            // Each ray blinks in and out on its own cadence.
            if hash3(x as i32, 0, slot / 2) < 0.3 {
                continue;
            }
            let top = (hash3(x as i32, 1, slot / 3) * h as f32 * 0.4) as usize;
            let len = 2 + (hash3(x as i32, 2, slot / 2) * h as f32 * 0.7) as usize;
            draw::vline(grid, x, top, (top + len).min(h.saturating_sub(1)));
        }
        // A steady baseline keeps the progress readable through the flicker.
        for x in 0..filled {
            draw::dot(grid, x, h - 1);
        }
        Ok(())
    }
}

/// A polar arc lights up along its length, a glow pulse racing it.
struct PolarArc;
impl ProgressStyle for PolarArc {
    fn name(&self) -> &str {
        "polar-arc"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "An arc of light with a racing glow pulse"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        let arc_y = |x: usize| -> usize {
            let t = x as f32 / w.max(1) as f32;
            let rise = (t * PI).sin() * (h as f32 - 3.0);
            (h as f32 - 1.5 - rise).max(0.0) as usize
        };
        // Twinkling stars beneath the arc.
        let slot = (ctx.time * 2.0) as i32;
        for x in 0..w {
            if hash3(x as i32, 21, slot) < 0.05 {
                let y = arc_y(x) + 2 + (hash2(x as i32, 22) * 4.0) as usize;
                if y < h {
                    draw::dot(grid, x, y);
                    let _ = grid.set_cell_color(x / 2, y / 4, Color::rgb(120, 130, 170));
                }
            }
        }
        // The lit portion of the arc, two dots thick.
        for x in 0..filled {
            let y = arc_y(x);
            draw::dot(grid, x, y);
            draw::dot(grid, x, (y + 1).min(h - 1));
            let t = x as f32 / w.max(1) as f32;
            let _ = grid.set_cell_color(x / 2, y / 4, sample_tint(0.3 + 0.5 * t));
        }
        // Glow pulse racing along the lit arc.
        if filled > 3 {
            let pos = ((ctx.time * 0.5).fract() * filled as f32) as usize;
            for k in 0..5usize {
                if pos >= k {
                    let x = pos - k;
                    let y = arc_y(x);
                    draw::dot(grid, x, y.saturating_sub(1));
                    let _ = grid.set_cell_color(x / 2, y / 4, sample_tint(0.95));
                }
            }
        }
        Ok(())
    }
}

/// Streaks of charged wind flow over a rising baseline fill.
struct SolarWind;
impl ProgressStyle for SolarWind {
    fn name(&self) -> &str {
        "solar-wind"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "Charged streaks over a rising baseline"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        // Baseline fill: bottom two dot rows carry the progress.
        for x in 0..filled {
            draw::dot(grid, x, h - 1);
            draw::dot(grid, x, h.saturating_sub(2));
            let t = x as f32 / w.max(1) as f32;
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, sample_tint(0.25 + 0.4 * t));
        }
        // Wind streaks race across the open sky above.
        let sky = h.saturating_sub(3);
        if sky > 0 {
            for row in 0..sky {
                if hash2(row as i32, 31) > 0.6 {
                    continue;
                }
                let rate = 0.5 + ((hash2(row as i32, 32) * 4.0).round()) * 0.25;
                let head = ((ctx.time * rate + hash2(row as i32, 33)).fract() * (w as f32 + 14.0))
                    as i32
                    - 7;
                let tail = 4 + (hash2(row as i32, 34) * 6.0) as i32;
                for k in 0..tail {
                    let x = head - k;
                    if x >= 0 && (x as usize) < w {
                        draw::dot(grid, x as usize, row);
                        let fade = 1.0 - k as f32 / tail as f32;
                        let _ = grid.set_cell_color(
                            x as usize / 2,
                            row / 4,
                            sample_tint(0.4 + 0.55 * fade),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

/// A crackling storm front advances, lightning at the leading edge.
struct IonStorm;
impl ProgressStyle for IonStorm {
    fn name(&self) -> &str {
        "ion-storm"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "A crackling front with edge lightning"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let edge = ctx.eased * w as f32;
        let slot = (ctx.time * 8.0) as i32;
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32;
                let d = edge - fx;
                let lit = if d > 6.0 {
                    true
                } else if d > -6.0 {
                    // Storm band: heavy flicker on both sides of the front.
                    hash3(x as i32, y as i32, slot) < 0.5 + d * 0.07
                } else {
                    false
                };
                if lit {
                    draw::dot(grid, x, y);
                    let hue = if d.abs() <= 6.0 {
                        0.85
                    } else {
                        0.2 + 0.35 * (fx / w.max(1) as f32)
                    };
                    let _ = grid.set_cell_color(x / 2, y / 4, sample_tint(hue));
                }
            }
        }
        // Occasional lightning stroke at the front.
        if hash2(slot, 77) < 0.35 {
            let x = (edge as usize + (hash2(slot, 78) * 6.0) as usize).min(w.saturating_sub(1));
            draw::vline(grid, x, 0, h - 1);
            let _ = grid.set_cell_color(x / 2, 0, Color::rgb(240, 244, 255));
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, Color::rgb(240, 244, 255));
        }
        Ok(())
    }
}

/// Light rises from the horizon; the sky fills bottom-up.
struct Zenith;
impl ProgressStyle for Zenith {
    fn name(&self) -> &str {
        "zenith"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "Light rising from the horizon, filling upward"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let rise = ctx.eased * h as f32;
        for x in 0..w {
            let wave = 1.5 * (x as f32 * 0.15 + ctx.time * TAU * 0.25).sin();
            let top = (h as f32 - rise + wave).max(0.0) as usize;
            for y in top..h {
                draw::dot(grid, x, y);
                let depth = (y.saturating_sub(top)) as f32 / h.max(1) as f32;
                let _ = grid.set_cell_color(x / 2, y / 4, sample_tint(0.8 - depth * 0.6));
            }
            // Faint rays escaping upward from the surface.
            if top > 1 && hash3(x as i32, 3, (ctx.time * 4.0) as i32) < 0.12 {
                draw::dot(grid, x, top.saturating_sub(2));
            }
        }
        Ok(())
    }
}

/// A band of light grows outward from the center, breathing as it goes.
struct Corona;
impl ProgressStyle for Corona {
    fn name(&self) -> &str {
        "corona"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "A breathing band growing from the center"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let half = ctx.eased * w as f32 / 2.0;
        let cx = w as f32 / 2.0;
        let mid = h as f32 / 2.0;
        let breathe = 0.75 + 0.25 * (ctx.time * TAU * 0.25).sin();
        for x in 0..w {
            let d = (x as f32 - cx).abs();
            if d > half {
                continue;
            }
            let falloff = 1.0 - d / half.max(0.5);
            let thick = (mid - 0.5) * breathe * (0.35 + 0.65 * falloff);
            let y0 = (mid - thick).max(0.0) as usize;
            let y1 = ((mid + thick) as usize).min(h.saturating_sub(1));
            for y in y0..=y1 {
                draw::dot(grid, x, y);
            }
            let _ =
                grid.set_cell_color(x / 2, (mid as usize) / 4, sample_tint(0.25 + 0.6 * falloff));
            // Shimmer rays at both advancing fronts.
            if half - d < 3.0 && hash3(x as i32, 5, (ctx.time * 6.0) as i32) < 0.5 {
                draw::vline(grid, x, y0.saturating_sub(2), y0);
                draw::vline(grid, x, y1, (y1 + 2).min(h - 1));
            }
        }
        Ok(())
    }
}

/// Dawn sweeps across a twinkling star field, absorbing it into light.
struct NightSkyFill;
impl ProgressStyle for NightSkyFill {
    fn name(&self) -> &str {
        "night-sky-fill"
    }
    fn theme(&self) -> &str {
        "aurora"
    }
    fn describe(&self) -> &str {
        "Dawn sweeping over a twinkling star field"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let edge = ctx.eased * w as f32;
        let slot = (ctx.time * 2.0) as i32;
        // Star field to the right of the dawn line.
        for y in 0..h {
            for x in (edge as usize)..w {
                if hash2(x as i32, y as i32) < 0.035 && hash3(x as i32, y as i32, slot) > 0.3 {
                    draw::dot(grid, x, y);
                    let _ = grid.set_cell_color(x / 2, y / 4, Color::rgb(126, 138, 176));
                }
            }
        }
        // The dawn band: solid light with undulating top and bottom edges.
        for x in 0..(edge as usize).min(w) {
            let top_wave = 1.5 + 1.5 * (x as f32 * 0.12 + ctx.time * TAU * 0.25).sin();
            let bot_wave = 1.5 + 1.5 * (x as f32 * 0.17 - ctx.time * TAU * 0.25).cos();
            let y0 = top_wave.max(0.0) as usize;
            let y1 = (h as f32 - 1.0 - bot_wave).max(0.0) as usize;
            for y in y0..=y1.min(h - 1) {
                draw::dot(grid, x, y);
                let t = x as f32 / w.max(1) as f32;
                let _ = grid.set_cell_color(x / 2, y / 4, sample_tint(0.25 + 0.55 * t));
            }
        }
        Ok(())
    }
}
