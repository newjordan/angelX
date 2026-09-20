//! Fire progress bars — spreading flames, rising embers, white-hot heads.
//!
//! Everything burns on a char-red → orange → gold → white heat ramp.
//! Progress reads as fire spreading along the bar (or candles lighting,
//! or a torch cutting through) while `time` keeps the flames flickering.
//! All motion is deterministic in `(progress, time)`.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::TAU;

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

// ─── theme tint — heat ramp ─────────────────────────────────────────────────

/// Charred deep red at the cold end.
const HEAT_CHAR: Color = Color::rgb(143, 29, 18);
/// Flame orange in the body.
const HEAT_ORANGE: Color = Color::rgb(255, 122, 47);
/// Molten gold near the core.
const HEAT_GOLD: Color = Color::rgb(255, 200, 74);
/// White-hot center.
const HEAT_WHITE: Color = Color::rgb(255, 243, 214);

/// Sample the char → orange → gold → white heat ramp at `t` in `0.0..=1.0`.
fn sample_tint(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8, k: f32| (f32::from(a) + (f32::from(b) - f32::from(a)) * k) as u8;
    let (from, to, k) = if t < 0.45 {
        (HEAT_CHAR, HEAT_ORANGE, t / 0.45)
    } else if t < 0.8 {
        (HEAT_ORANGE, HEAT_GOLD, (t - 0.45) / 0.35)
    } else {
        (HEAT_GOLD, HEAT_WHITE, (t - 0.8) / 0.2)
    };
    Color::rgb(
        lerp(from.r, to.r, k),
        lerp(from.g, to.g, k),
        lerp(from.b, to.b, k),
    )
}

/// Applies the heat ramp to every cell the inner style drew: hotter toward
/// the bottom (where the fire lives) with a fast per-cell flicker.
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
        let slot = (ctx.time * 8.0) as i32;
        for y in 0..h {
            for x in 0..w {
                let ch = grid.get_char(x, y);
                if ch != '\u{2800}' && ch != ' ' {
                    let depth = (y as f32 + 0.5) / h.max(1) as f32;
                    let flicker = 0.2 * hash3(x as i32, y as i32, slot);
                    let _ = grid.set_cell_color(x, y, sample_tint(0.25 + 0.5 * depth + flicker));
                }
            }
        }
        Ok(())
    }
}

/// All styles in the `inferno` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Wildfire),
        Box::new(FlameFill),
        Box::new(EmberRise),
        Box::new(Blowtorch),
        Box::new(Candle),
        Box::new(Tinted(Flashover)),
        Box::new(Charline),
        Box::new(Tinted(Backdraft)),
        Box::new(SolarFlare),
        Box::new(Phoenix),
    ]
}

/// Fire spreads left to right: ember bed behind, flames at the front.
struct Wildfire;
impl ProgressStyle for Wildfire {
    fn name(&self) -> &str {
        "wildfire"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "A fire line spreading across the bar"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let front = (ctx.eased * w as f32).round() as usize;
        let slot = (ctx.time * 8.0) as i32;
        // Unburnt fuel ahead: a thin ground line.
        for x in front..w {
            draw::dot(grid, x, h - 1);
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, Color::rgb(92, 74, 52));
        }
        // Ember bed behind the front: lower half, glowing unevenly.
        for x in 0..front {
            let bed = h / 2 + (hash2(x as i32, 3) * 2.0) as usize;
            for y in bed.min(h - 1)..h {
                draw::dot(grid, x, y);
            }
            let glow = 0.25 + 0.25 * hash3(x as i32, 0, slot / 2);
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, sample_tint(glow));
        }
        // Burning zone at the front: tall flame tongues licking upward.
        let zone = 12usize;
        for k in 0..zone {
            let x = front.saturating_sub(zone) + k;
            if x >= w {
                break;
            }
            let near = k as f32 / zone as f32;
            let flick = hash3(x as i32, 1, slot);
            let tongue = ((0.45 + 0.55 * near) * (0.65 + 0.35 * flick) * h as f32) as usize;
            for y in h.saturating_sub(tongue.max(2))..h {
                draw::dot(grid, x, y);
            }
            // Sparks leaping off the tallest tongues.
            if flick > 0.6 {
                let sy = h.saturating_sub(tongue + 2);
                draw::dot(grid, x, sy);
            }
            let heat = 0.5 + 0.5 * near;
            for cy in 0..grid.dimensions().1 {
                draw::tint_row(grid, cy, x / 2, x / 2, sample_tint(heat - cy as f32 * 0.12));
            }
        }
        Ok(())
    }
}

/// The whole filled region is living flame, tips flickering.
struct FlameFill;
impl ProgressStyle for FlameFill {
    fn name(&self) -> &str {
        "flame-fill"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "A bar of living flame"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        let slot = (ctx.time * 8.0) as i32;
        for x in 0..filled {
            let sway = (x as f32 * 0.4 + ctx.time * TAU * 0.5).sin();
            let flick = hash3(x as i32, 0, slot);
            let height = ((0.55 + 0.2 * sway + 0.25 * flick) * h as f32) as usize;
            let top = h.saturating_sub(height.max(2));
            for y in top..h {
                draw::dot(grid, x, y);
            }
            // Heat rises: white at the base, red at the tips.
            let (_, ch_cells) = grid.dimensions();
            for cy in 0..ch_cells {
                let frac = 1.0 - cy as f32 / ch_cells.max(1) as f32;
                draw::tint_row(grid, cy, x / 2, x / 2, sample_tint(1.0 - frac * 0.75));
            }
        }
        Ok(())
    }
}

/// A glowing bed extends along the bar; embers rise and drift above it.
struct EmberRise;
impl ProgressStyle for EmberRise {
    fn name(&self) -> &str {
        "ember-rise"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "Embers drifting up from a glowing bed"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        // The bed: two glowing dot rows.
        for x in 0..filled {
            draw::dot(grid, x, h - 1);
            draw::dot(grid, x, h.saturating_sub(2));
            let glow = 0.35 + 0.3 * hash3(x as i32, 0, (ctx.time * 6.0) as i32);
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, sample_tint(glow));
        }
        // Embers rise from the lit bed, drifting sideways as they climb.
        let head = h.saturating_sub(2) as f32;
        if head > 1.0 {
            for e in 0..(w / 3) {
                let ex = (hash2(e as i32, 5) * filled.max(1) as f32) as usize;
                if ex >= filled {
                    continue;
                }
                let rate = 0.25 + ((hash2(e as i32, 6) * 3.0).round()) * 0.25;
                let climb = (ctx.time * rate + hash2(e as i32, 7)).fract();
                let y = head - climb * head;
                let drift = ((climb * TAU).sin() * 2.5) as i32;
                let px = ex as i32 + drift;
                draw::dot_i(grid, px, y as i32);
                if px >= 0 {
                    let heat = 1.0 - climb;
                    let _ = grid.set_cell_color(
                        px as usize / 2,
                        y as usize / 4,
                        sample_tint(0.4 + 0.6 * heat),
                    );
                }
            }
        }
        Ok(())
    }
}

/// A torch head cuts rightward, leaving a cooling molten line.
struct Blowtorch;
impl ProgressStyle for Blowtorch {
    fn name(&self) -> &str {
        "blowtorch"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "A torch cutting a molten line"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let head = (ctx.eased * w as f32) as usize;
        let mid = h / 2;
        // The cooling cut: brightness decays with distance behind the head.
        for x in 0..head.min(w) {
            draw::dot(grid, x, mid);
            draw::dot(grid, x, mid.saturating_sub(1));
            let cool = (head - x) as f32 / w.max(1) as f32;
            let _ = grid.set_cell_color(x / 2, mid / 4, sample_tint(1.0 - cool * 1.6));
        }
        // The torch cone: a bright wedge of dots at the head.
        let slot = (ctx.time * 12.0) as i32;
        for dx in 0..5i32 {
            let spread = dx;
            for dy in -spread..=spread {
                let x = head as i32 + dx;
                let y = mid as i32 + dy;
                if hash3(x, y, slot) < 0.75 {
                    draw::dot_i(grid, x, y);
                }
            }
        }
        if head < w {
            let _ = grid.set_cell_color(head / 2, mid / 4, HEAT_WHITE);
            let _ = grid.set_cell_color(((head + 4).min(w - 1)) / 2, mid / 4, HEAT_GOLD);
        }
        // Sparks spraying off the cut point.
        for s in 0..6i32 {
            if hash3(s, 9, slot) < 0.5 {
                let sx = head as i32 + (hash3(s, 10, slot) * 8.0) as i32 - 2;
                let sy = mid as i32 + (hash3(s, 11, slot) * 8.0) as i32 - 4;
                draw::dot_i(grid, sx, sy);
            }
        }
        Ok(())
    }
}

/// Ten candles light up one by one, flames swaying.
struct Candle;
impl ProgressStyle for Candle {
    fn name(&self) -> &str {
        "candle"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "Candles lighting one per ten percent"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let count = 10usize;
        let lit = (ctx.progress * count as f32).round() as usize;
        for c in 0..count {
            let x = ((c as f32 + 0.5) / count as f32 * w as f32) as usize;
            if x >= w {
                continue;
            }
            // Candle body: a short two-wide column from the floor.
            let body_top = h.saturating_sub(h / 3);
            for y in body_top..h {
                draw::dot(grid, x, y);
                draw::dot(grid, (x + 1).min(w - 1), y);
            }
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, Color::rgb(120, 104, 86));
            // Flame above lit candles, swaying on its own phase.
            if c < lit {
                let sway = ((ctx.time * TAU * 0.5 + c as f32 * 1.3).sin() * 1.5) as i32;
                let fx = x as i32 + sway;
                let fy = body_top as i32;
                draw::dot_i(grid, fx, fy - 2);
                draw::dot_i(grid, fx, fy - 3);
                draw::dot_i(grid, fx + 1, fy - 2);
                draw::dot_i(grid, fx + 1, fy - 3);
                draw::dot_i(grid, fx, fy - 4);
                draw::dot_i(grid, fx, fy - 5);
                if fx >= 0 {
                    let _ = grid.set_cell_color(
                        fx as usize / 2,
                        (fy as usize).saturating_sub(3) / 4,
                        HEAT_GOLD,
                    );
                    let _ = grid.set_cell_color(
                        fx as usize / 2,
                        (fy as usize).saturating_sub(1) / 4,
                        HEAT_WHITE,
                    );
                }
            }
        }
        Ok(())
    }
}

/// The whole bar heats through shades until it rolls with fire.
struct Flashover;
impl ProgressStyle for Flashover {
    fn name(&self) -> &str {
        "flashover"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "Rising heat shading into full flashover"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        for y in 0..ch {
            for x in 0..cw {
                let fx = x as f32 / cw.max(1) as f32;
                let roll = 0.15 * (fx * TAU + ctx.time * TAU * 0.5).sin();
                let heat = ctx.eased * 1.25 - fx * 0.35 + roll;
                let level = (heat.clamp(0.0, 1.0) * 4.4) as usize;
                if level > 0 {
                    draw::shade(grid, x, y, level.min(4));
                }
            }
        }
        Ok(())
    }
}

/// A fuse burns along the middle: sparks at the front, smoke behind.
struct Charline;
impl ProgressStyle for Charline {
    fn name(&self) -> &str {
        "charline"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "A burning fuse with sparks and smoke"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let burn = (ctx.eased * w as f32) as usize;
        let mid = h / 2;
        // Unburnt fuse ahead.
        for x in burn..w {
            draw::dot(grid, x, mid);
            let _ = grid.set_cell_color(x / 2, mid / 4, Color::rgb(122, 104, 76));
        }
        // Char line behind: the consumed fuse stays as a glowing broken line,
        // so the bar keeps reading at high progress.
        for x in 0..burn {
            if hash2(x as i32, 12) < 0.8 {
                draw::dot(grid, x, mid);
                draw::dot(grid, x, (mid + 1).min(h - 1));
                let glow = 0.1 + 0.25 * hash3(x as i32, 13, (ctx.time * 4.0) as i32);
                let _ = grid.set_cell_color(x / 2, mid / 4, sample_tint(glow));
            }
        }
        // Spark cluster at the burn point.
        let slot = (ctx.time * 12.0) as i32;
        for s in 0..10i32 {
            if hash3(s, 1, slot) < 0.6 {
                let sx = burn as i32 + (hash3(s, 2, slot) * 7.0) as i32 - 3;
                let sy = mid as i32 + (hash3(s, 3, slot) * 7.0) as i32 - 3;
                draw::dot_i(grid, sx, sy);
                if sx >= 0 && sy >= 0 {
                    let _ = grid.set_cell_color(sx as usize / 2, sy as usize / 4, HEAT_GOLD);
                }
            }
        }
        if burn < w {
            let _ = grid.set_cell_color(burn / 2, mid / 4, HEAT_WHITE);
        }
        // Smoke wisps rising over the burnt stretch.
        for x in (0..burn).step_by(4) {
            let age = (burn - x) as f32 / w.max(1) as f32;
            let lift = (age * 8.0 + (ctx.time * TAU * 0.25 + x as f32 * 0.4).sin() * 1.5) as usize;
            let y = mid.saturating_sub(1 + lift.min(mid));
            if hash2(x as i32, 8) < 0.6 {
                draw::dot(grid, x, y);
                let gray = (150.0 - 90.0 * age) as u8;
                let _ = grid.set_cell_color(x / 2, y / 4, Color::rgb(gray, gray, gray));
            }
        }
        Ok(())
    }
}

/// Flame surges past the fill line and retracts, breathing like a backdraft.
struct Backdraft;
impl ProgressStyle for Backdraft {
    fn name(&self) -> &str {
        "backdraft"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "A fill that surges and retracts like a backdraft"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let base = ctx.eased * w as f32;
        // The surge: a breathing overshoot ahead of the true fill.
        let surge = (0.5 + 0.5 * (ctx.time * TAU * 0.5).sin()) * w as f32 * 0.08;
        let slot = (ctx.time * 10.0) as i32;
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32;
                if fx < base {
                    draw::dot(grid, x, y);
                } else if fx < base + surge {
                    // Surge zone: ragged, flickering flame.
                    let chance = 1.0 - (fx - base) / surge.max(0.5);
                    if hash3(x as i32, y as i32, slot) < chance * 0.8 {
                        draw::dot(grid, x, y);
                    }
                }
            }
        }
        Ok(())
    }
}

/// A blazing orb rides the bar, corona spikes wheeling around it.
struct SolarFlare;
impl ProgressStyle for SolarFlare {
    fn name(&self) -> &str {
        "solar-flare"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "A blazing orb with a plasma trail"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let mid = h as f32 / 2.0;
        let cx = ctx.eased * (w as f32 - 4.0) + 2.0;
        // Plasma trail back to the start.
        for x in 0..(cx as usize) {
            let fade = 1.0 - (cx - x as f32) / w.max(1) as f32 * 1.6;
            if fade > 0.0 {
                draw::dot(grid, x, mid as usize);
                if fade > 0.45 {
                    draw::dot(grid, x, (mid as usize).saturating_sub(1));
                    draw::dot(grid, x, (mid as usize + 1).min(h - 1));
                }
                let _ = grid.set_cell_color(x / 2, mid as usize / 4, sample_tint(fade.max(0.05)));
            }
        }
        // The orb: a filled disc.
        let r = (h as f32 / 3.2).max(1.5);
        let (x0, x1) = ((cx - r) as i32, (cx + r) as i32);
        for x in x0..=x1 {
            for y in 0..h as i32 {
                let dx = x as f32 - cx;
                let dy = y as f32 - mid;
                if dx * dx + dy * dy <= r * r {
                    draw::dot_i(grid, x, y);
                }
            }
        }
        if cx >= 0.0 {
            let _ = grid.set_cell_color(cx as usize / 2, mid as usize / 4, HEAT_WHITE);
        }
        // Corona spikes wheeling with time.
        for s in 0..6 {
            let ang = ctx.time * TAU * 0.25 + s as f32 * TAU / 6.0;
            let sx = cx + ang.cos() * (r + 2.0);
            let sy = mid + ang.sin() * (r + 1.0);
            draw::dot_i(grid, sx as i32, sy as i32);
            if sx >= 0.0 && sy >= 0.0 && (sx as usize) < w && (sy as usize) < h {
                let _ = grid.set_cell_color(sx as usize / 2, sy as usize / 4, HEAT_GOLD);
            }
        }
        Ok(())
    }
}

/// Feathered wing layers build the bar; at full it bursts into flame.
struct Phoenix;
impl ProgressStyle for Phoenix {
    fn name(&self) -> &str {
        "phoenix"
    }
    fn theme(&self) -> &str {
        "inferno"
    }
    fn describe(&self) -> &str {
        "Feathered fire that bursts at one hundred percent"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        // Feathered body: overlapping arcs rising toward the leading edge.
        for x in 0..filled {
            let t = x as f32 / w.max(1) as f32;
            let feather = (x as f32 * 0.5 + ctx.time * TAU * 0.25).sin() * 1.5;
            let height = (h as f32 * (0.35 + 0.4 * t) + feather) as usize;
            for y in h.saturating_sub(height.max(1))..h {
                if (y + x / 6) % 4 != 3 {
                    draw::dot(grid, x, y);
                }
            }
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, sample_tint(0.3 + 0.6 * t));
        }
        // The rebirth burst at (nearly) full.
        if ctx.progress > 0.94 {
            let strength = (ctx.progress - 0.94) / 0.06;
            let cx = filled.min(w.saturating_sub(1)) as f32;
            let cy = h as f32 / 2.0;
            for ray in 0..14 {
                let ang = ray as f32 * TAU / 14.0 + ctx.time * TAU * 0.25;
                let len = strength * h as f32 * (0.6 + 0.4 * hash2(ray, 3));
                for k in 0..len as i32 {
                    let px = cx + ang.cos() * k as f32 * 1.6;
                    let py = cy + ang.sin() * k as f32 * 0.8;
                    draw::dot_i(grid, px as i32, py as i32);
                    if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                        let _ = grid.set_cell_color(
                            px as usize / 2,
                            py as usize / 4,
                            sample_tint(1.0 - k as f32 / len.max(1.0)),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
