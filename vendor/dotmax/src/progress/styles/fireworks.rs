//! Fireworks progress bars — launches, bursts, sparklers, grand finales.
//!
//! Progress reads as a show building: rockets go up one per ten percent,
//! bursts bloom wider, salvos thicken, and everything pays off at one
//! hundred percent. Colors are a night-show palette — gold, coral, cyan,
//! violet — around white-hot flashes. Deterministic in `(progress, time)`.

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

// ─── theme colors — night show ──────────────────────────────────────────────

/// Gold, the classic shell.
const FW_GOLD: Color = Color::rgb(255, 203, 94);
/// Coral red.
const FW_CORAL: Color = Color::rgb(240, 110, 90);
/// Electric cyan.
const FW_CYAN: Color = Color::rgb(92, 220, 240);
/// Violet.
const FW_VIOLET: Color = Color::rgb(176, 140, 250);
/// White-hot flash.
const FW_FLASH: Color = Color::rgb(255, 248, 231);

/// Pick a show color for particle index `i`.
fn fw_color(i: i32) -> Color {
    match (hash2(i, 999) * 4.0) as u32 {
        0 => FW_GOLD,
        1 => FW_CORAL,
        2 => FW_CYAN,
        _ => FW_VIOLET,
    }
}

/// All styles in the `fireworks` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Launch),
        Box::new(Peony),
        Box::new(Chrysanthemum),
        Box::new(Crackle),
        Box::new(RomanCandle),
        Box::new(SparklerFill),
        Box::new(Salvo),
        Box::new(Willow),
        Box::new(StrobePop),
        Box::new(GrandFinale),
    ]
}

/// Rockets launch one per ten percent, each leaving a star at apex.
struct Launch;
impl ProgressStyle for Launch {
    fn name(&self) -> &str {
        "launch"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "Rockets launching one per ten percent"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let count = 10usize;
        // Ground line for composition.
        for x in (0..w).step_by(2) {
            draw::dot(grid, x, h - 1);
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, Color::rgb(96, 92, 104));
        }
        let apex = 2usize;
        let span = (h - 1 - apex) as f32;
        for k in 0..count {
            let x = ((k as f32 + 0.5) / count as f32 * w as f32) as usize;
            if x >= w {
                continue;
            }
            let window = (ctx.eased * count as f32) - k as f32;
            if window >= 1.0 {
                // Arrived: a twinkling star at apex.
                if hash3(k as i32, 5, (ctx.time * 4.0) as i32) > 0.25 {
                    draw::dot(grid, x, apex);
                    draw::dot(grid, x.saturating_sub(1), apex + 1);
                    draw::dot(grid, (x + 1).min(w - 1), apex + 1);
                    let _ = grid.set_cell_color(x / 2, apex / 4, fw_color(k as i32));
                }
            } else if window > 0.0 {
                // In flight: rocket climbs with a short flame trail.
                let y = (h as f32 - 1.0 - window * span) as usize;
                draw::dot(grid, x, y);
                for t in 1..4usize {
                    let ty = y + t;
                    if ty < h && hash3(k as i32, t as i32, (ctx.time * 12.0) as i32) > 0.3 {
                        draw::dot(grid, x, ty);
                    }
                }
                let _ = grid.set_cell_color(x / 2, y / 4, FW_FLASH);
            }
        }
        Ok(())
    }
}

/// One great shell blooms from the center as progress grows.
struct Peony;
impl ProgressStyle for Peony {
    fn name(&self) -> &str {
        "peony"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "A shell blooming wider with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let max_r = w as f32 / 2.0 - 1.0;
        let r = ctx.eased * max_r;
        let rays = 18;
        for ray in 0..rays {
            let ang = ray as f32 * TAU / rays as f32 + ctx.time * TAU * 0.25 * 0.2;
            let color = fw_color(ray);
            // Dots along the ray, denser near the tip.
            let steps = (r / 2.0) as i32;
            for s in 0..=steps {
                let dist = r * (s as f32 / steps.max(1) as f32);
                if hash3(ray, s, 0) < 0.35 && s < steps - 1 {
                    continue;
                }
                let px = cx + ang.cos() * dist;
                let py = cy + ang.sin() * dist * 0.45; // squash to the bar
                draw::dot_i(grid, px as i32, py as i32);
                if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                    let c = if s == steps { FW_FLASH } else { color };
                    let _ = grid.set_cell_color(px as usize / 2, py as usize / 4, c);
                }
            }
        }
        // Twinkle at the core.
        if hash3(0, 0, (ctx.time * 6.0) as i32) > 0.4 {
            draw::dot(grid, cx as usize, cy as usize);
            let _ = grid.set_cell_color(cx as usize / 2, cy as usize / 4, FW_FLASH);
        }
        Ok(())
    }
}

/// A double shell: a dense core disc inside a ring of falling petals.
struct Chrysanthemum;
impl ProgressStyle for Chrysanthemum {
    fn name(&self) -> &str {
        "chrysanthemum"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "A double shell with drooping petals"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let cx = w as f32 / 2.0;
        let cy = h as f32 * 0.4;
        let max_r = w as f32 / 2.0 - 1.0;
        let r = ctx.eased * max_r;
        // Core disc: dithered fill.
        let core = r * 0.35;
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = (y as f32 - cy) * 2.2;
                if dx * dx + dy * dy <= core * core && hash2(x as i32, y as i32) < 0.6 {
                    draw::dot(grid, x, y);
                    let _ = grid.set_cell_color(x / 2, y / 4, FW_GOLD);
                }
            }
        }
        // Outer petals: ring points sagging with age.
        let petals = 22;
        for p in 0..petals {
            let ang = p as f32 * TAU / petals as f32;
            let sag = (r / max_r.max(1.0)).powi(2) * h as f32 * 0.3;
            let px = cx + ang.cos() * r;
            let py = cy + ang.sin() * r * 0.4 + sag;
            draw::dot_i(grid, px as i32, py as i32);
            // A short trailing spark above each petal.
            draw::dot_i(grid, px as i32, py as i32 - 2);
            if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                let _ = grid.set_cell_color(px as usize / 2, py as usize / 4, fw_color(p));
            }
        }
        Ok(())
    }
}

/// A steady bar that crackles with popping sparks as it fills.
struct Crackle;
impl ProgressStyle for Crackle {
    fn name(&self) -> &str {
        "crackle"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "A bar crackling with popping sparks"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        let mid = h / 2;
        // The base bar: a chunky four-row band.
        for x in 0..filled {
            for dy in 0..4usize {
                draw::dot(grid, x, (mid + dy).saturating_sub(2).min(h - 1));
            }
            let _ = grid.set_cell_color(x / 2, mid / 4, FW_GOLD);
        }
        // Crackle pops above and below, densest near the leading edge.
        let slot = (ctx.time * 12.0) as i32;
        for x in 0..filled {
            let near = 1.0 - (filled - x) as f32 / w.max(1) as f32 * 1.8;
            if near <= 0.0 {
                continue;
            }
            if hash3(x as i32, 1, slot) < near * 0.7 {
                let up = 1 + (hash3(x as i32, 2, slot) * (mid as f32 - 2.0)) as usize;
                let y = if hash3(x as i32, 3, slot) > 0.5 {
                    mid.saturating_sub(2 + up)
                } else {
                    (mid + 2 + up).min(h - 1)
                };
                draw::dot(grid, x, y);
                if hash3(x as i32, 4, slot) > 0.5 {
                    draw::dot(grid, (x + 1).min(w - 1), y);
                }
                let _ = grid.set_cell_color(x / 2, y / 4, FW_FLASH);
            }
        }
        Ok(())
    }
}

/// A candle at the left lobs shots that burst at the progress point.
struct RomanCandle;
impl ProgressStyle for RomanCandle {
    fn name(&self) -> &str {
        "roman-candle"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "Arcing shots bursting at the progress point"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        // The candle tube at the left edge.
        for y in (h * 2 / 3)..h {
            draw::dot(grid, 0, y);
            draw::dot(grid, 1.min(w - 1), y);
        }
        let _ = grid.set_cell_color(0, (h - 1) / 4, Color::rgb(120, 104, 86));
        let target = (ctx.eased * (w as f32 - 3.0)) + 2.0;
        // Milestone embers where earlier shots landed: little glow mounds.
        for k in 1..=10 {
            let mx = (k as f32 / 10.0 * (w as f32 - 3.0) + 2.0) as usize;
            if (mx as f32) < target && hash3(k, 8, (ctx.time * 4.0) as i32) > 0.25 {
                draw::dot(grid, mx, h - 1);
                draw::dot(grid, (mx + 1).min(w - 1), h - 1);
                draw::dot(grid, mx, h - 2);
                let _ = grid.set_cell_color(mx / 2, (h - 2) / 4, fw_color(k));
            }
        }
        // Target marker: a blinking tick where the next shot lands.
        if (ctx.time * 3.0) as i32 % 2 == 0 {
            let tx = (target as usize).min(w.saturating_sub(1));
            draw::vline(grid, tx, h.saturating_sub(4), h - 1);
            let _ = grid.set_cell_color(tx / 2, (h - 1) / 4, FW_FLASH);
        }
        // The shot in flight: a fat spark on a parabolic arc, with a trail.
        let phase = (ctx.time * 0.75).fract();
        let sx = 1.0 + phase * (target - 1.0);
        let ft = sx / w.max(1) as f32;
        let arc = (phase * (1.0 - phase)) * 4.0; // 0..1 peak mid-flight
        let sy = (h as f32 - 2.0) - arc * (h as f32 - 4.0) * (0.5 + ft * 0.5);
        for k in 0..4i32 {
            let tp = (phase - k as f32 * 0.03).max(0.0);
            let tx = 1.0 + tp * (target - 1.0);
            let tarc = (tp * (1.0 - tp)) * 4.0;
            let ty = (h as f32 - 2.0) - tarc * (h as f32 - 4.0) * (0.5 + ft * 0.5);
            draw::dot_i(grid, tx as i32, ty as i32);
        }
        draw::dot_i(grid, sx as i32, sy as i32 - 1);
        draw::dot_i(grid, sx as i32 + 1, sy as i32);
        if sx >= 0.0 && sy >= 0.0 && (sx as usize) < w && (sy as usize) < h {
            let _ = grid.set_cell_color(sx as usize / 2, sy as usize / 4, FW_FLASH);
        }
        // Burst bloom when the shot arrives.
        if phase > 0.9 {
            let bloom = (phase - 0.9) * 10.0;
            for ray in 0..8 {
                let ang = ray as f32 * TAU / 8.0;
                let px = target + ang.cos() * bloom * 5.0;
                let py = (h as f32 - 3.0) + ang.sin() * bloom * 3.0;
                draw::dot_i(grid, px as i32, py as i32);
                if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                    let _ = grid.set_cell_color(px as usize / 2, py as usize / 4, fw_color(ray));
                }
            }
        }
        Ok(())
    }
}

/// A sparkler point burns along a wire, spraying rays as it goes.
struct SparklerFill;
impl ProgressStyle for SparklerFill {
    fn name(&self) -> &str {
        "sparkler-fill"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "A sparkler burning along a wire"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let mid = h / 2;
        let head = (ctx.eased * w as f32) as usize;
        // Burnt wire behind: a thin steady line.
        for x in 0..head {
            draw::dot(grid, x, mid);
            let _ = grid.set_cell_color(x / 2, mid / 4, Color::rgb(150, 130, 110));
        }
        // Unburnt wire ahead: sparse dots.
        for x in (head..w).step_by(3) {
            draw::dot(grid, x, mid);
            let _ = grid.set_cell_color(x / 2, mid / 4, Color::rgb(90, 86, 96));
        }
        // The sparkler head: dense random short rays, re-rolled each frame.
        let slot = (ctx.time * 12.0) as i32;
        for s in 0..16i32 {
            let ang = hash3(s, 1, slot) * TAU;
            let len = 1.5 + hash3(s, 2, slot) * 5.0;
            for k in 0..len as i32 {
                let px = head as f32 + ang.cos() * k as f32;
                let py = mid as f32 + ang.sin() * k as f32 * 0.7;
                draw::dot_i(grid, px as i32, py as i32);
                if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                    let c = if k < 2 { FW_FLASH } else { FW_GOLD };
                    let _ = grid.set_cell_color(px as usize / 2, py as usize / 4, c);
                }
            }
        }
        Ok(())
    }
}

/// Small shells burst all along the filled stretch, thicker as it grows.
struct Salvo;
impl ProgressStyle for Salvo {
    fn name(&self) -> &str {
        "salvo"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "Shell bursts thickening along the fill"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        // Thin baseline keeps the reading unambiguous.
        for x in 0..filled {
            draw::dot(grid, x, h - 1);
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, FW_GOLD);
        }
        // Bursts: each has a home position and its own life cycle.
        let shells = 12;
        for s in 0..shells {
            let bx = (hash2(s, 51) * w as f32) as usize;
            if bx >= filled {
                continue;
            }
            let by = 2.0 + hash2(s, 52) * (h as f32 * 0.5);
            let age = (ctx.time * 0.5 + hash2(s, 53)).fract();
            let radius = age * 6.0;
            let fade = 1.0 - age;
            let points = 8;
            for p in 0..points {
                if hash3(s, p, 7) > fade + 0.3 {
                    continue;
                }
                let ang = p as f32 * TAU / points as f32 + hash2(s, 54) * TAU;
                let px = bx as f32 + ang.cos() * radius;
                let py = by + ang.sin() * radius * 0.6 + age * age * 3.0;
                draw::dot_i(grid, px as i32, py as i32);
                if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                    let c = if age < 0.15 {
                        FW_FLASH
                    } else {
                        fw_color(s * 31 + p)
                    };
                    let _ = grid.set_cell_color(px as usize / 2, py as usize / 4, c);
                }
            }
        }
        Ok(())
    }
}

/// One golden willow: long drooping trails that stretch with progress.
struct Willow;
impl ProgressStyle for Willow {
    fn name(&self) -> &str {
        "willow"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "Golden trails drooping wider with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let cx = w as f32 / 2.0;
        let cy = 1.5;
        let trails = 16;
        let reach = ctx.eased * (w as f32 / 2.0);
        for t in 0..trails {
            let spread = (t as f32 / (trails - 1).max(1) as f32) * 2.0 - 1.0; // -1..1
            let dir = spread * reach;
            let steps = (reach * 0.9) as i32;
            for s in 0..steps {
                let frac = s as f32 / steps.max(1) as f32;
                let px = cx + dir * frac;
                let py = cy + frac * frac * (h as f32 - 2.5) + (hash3(t, s, 3) - 0.5);
                // Trails shimmer out near the tips.
                if frac > 0.6 && hash3(t, s, (ctx.time * 6.0) as i32) < frac - 0.5 {
                    continue;
                }
                draw::dot_i(grid, px as i32, py as i32);
                if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                    let c = if frac < 0.2 { FW_FLASH } else { FW_GOLD };
                    let _ = grid.set_cell_color(px as usize / 2, py as usize / 4, c);
                }
            }
        }
        Ok(())
    }
}

/// Single-frame flashes pop across the fill, leaving afterglow rings.
struct StrobePop;
impl ProgressStyle for StrobePop {
    fn name(&self) -> &str {
        "strobe-pop"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "Strobe flashes with afterglow rings"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        // Baseline bar, two rows.
        for x in 0..filled {
            draw::dot(grid, x, h - 1);
            draw::dot(grid, x, h.saturating_sub(2));
            let _ = grid.set_cell_color(x / 2, (h - 1) / 4, FW_VIOLET);
        }
        // Pops: each strobe lives two slots — a bright cross, then a ring.
        let slot = (ctx.time * 6.0) as i32;
        let pops = 2 + (ctx.progress * 6.0) as i32;
        for p in 0..pops {
            let s = slot - p; // stagger pops across recent slots
            let px = (hash3(p, 1, s) * filled.max(1) as f32) as i32;
            let py = 1 + (hash3(p, 2, s) * (h as f32 - 6.0)) as i32;
            if px as usize >= filled {
                continue;
            }
            if p % 2 == 0 {
                // Flash: a bright cross.
                draw::dot_i(grid, px, py);
                draw::dot_i(grid, px - 1, py);
                draw::dot_i(grid, px + 1, py);
                draw::dot_i(grid, px - 2, py);
                draw::dot_i(grid, px + 2, py);
                draw::dot_i(grid, px, py - 1);
                draw::dot_i(grid, px, py + 1);
                if px >= 0 && py >= 0 {
                    let _ = grid.set_cell_color(px as usize / 2, py as usize / 4, FW_FLASH);
                }
            } else {
                // Afterglow: a denser ring.
                for r in 0..8 {
                    let ang = r as f32 * TAU / 8.0;
                    let rx = px + (ang.cos() * 3.5) as i32;
                    let ry = py + (ang.sin() * 2.2) as i32;
                    if hash3(p, r, s) > 0.25 {
                        draw::dot_i(grid, rx, ry);
                        if rx >= 0 && ry >= 0 && (rx as usize) < w && (ry as usize) < h {
                            let _ = grid.set_cell_color(
                                rx as usize / 2,
                                ry as usize / 4,
                                fw_color(p * 7 + r),
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// The whole show at once: a gauge below, a sky that fills with bursts.
struct GrandFinale;
impl ProgressStyle for GrandFinale {
    fn name(&self) -> &str {
        "grand-finale"
    }
    fn theme(&self) -> &str {
        "fireworks"
    }
    fn describe(&self) -> &str {
        "A sky filling with bursts toward the finale"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let (cw, ch) = grid.dimensions();
        // The gauge: a crisp eighth-block bar on the bottom cell row.
        if ch > 0 {
            draw::hbar(grid, ch - 1, ctx.eased);
            for cx in 0..cw {
                let _ = grid.set_cell_color(cx, ch - 1, FW_GOLD);
            }
        }
        // The sky above fills with simultaneous bursts as progress climbs.
        let sky_h = h.saturating_sub(4);
        if sky_h < 3 {
            return Ok(());
        }
        let bursts = 1 + (ctx.eased * 5.0) as i32;
        for b in 0..bursts {
            let age = (ctx.time * 0.5 + hash2(b, 61)).fract();
            let bx = (hash3(b, 62, (ctx.time * 0.5 + hash2(b, 61)) as i32) * w as f32) as f32;
            let by = 1.0 + hash2(b, 63) * (sky_h as f32 * 0.6);
            let radius = age * 7.0;
            let rays = 10;
            for r in 0..rays {
                let ang = r as f32 * TAU / rays as f32 + hash2(b, 64) * TAU;
                let px = bx + ang.cos() * radius;
                let py = by + ang.sin() * radius * 0.5 + age * age * 2.5;
                if py >= sky_h as f32 {
                    continue;
                }
                if hash3(b, r, 9) > age {
                    draw::dot_i(grid, px as i32, py as i32);
                    if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                        let c = if age < 0.12 {
                            FW_FLASH
                        } else {
                            fw_color(b * 13 + r)
                        };
                        let _ = grid.set_cell_color(px as usize / 2, py as usize / 4, c);
                    }
                }
            }
        }
        // The hundred-percent moment: a full-sky strobe.
        if ctx.progress > 0.97 && (ctx.time * 6.0) as i32 % 2 == 0 {
            for x in (0..w).step_by(2) {
                draw::dot(grid, x, 0);
                let _ = grid.set_cell_color(x / 2, 0, FW_FLASH);
            }
        }
        Ok(())
    }
}
