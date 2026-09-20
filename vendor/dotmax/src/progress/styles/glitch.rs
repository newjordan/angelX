//! Glitch progress bars — torn signals, RGB fringes, static, sync locks.
//!
//! Progress reads as *signal integrity*: bars heal their dropouts, static
//! condenses into clean fill, and rolling pictures lock as the number
//! climbs. Fringe colors are the classic chromatic-aberration pair —
//! magenta and cyan around a white core. Deterministic in `(progress, time)`.

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

// ─── theme colors — chromatic aberration ────────────────────────────────────

/// The clean white signal.
const SIGNAL: Color = Color::rgb(232, 236, 242);
/// Magenta fringe (left/low offset copy).
const FRINGE_MAGENTA: Color = Color::rgb(255, 63, 216);
/// Cyan fringe (right/high offset copy).
const FRINGE_CYAN: Color = Color::rgb(57, 230, 255);
/// Dim scan-noise gray.
const NOISE_GRAY: Color = Color::rgb(110, 116, 128);

/// Applies glitch coloring to every cell the inner style drew: a white core
/// with hash-timed magenta/cyan pops, like a failing video cable.
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
        let slot = (ctx.time * 6.0) as i32;
        for y in 0..h {
            for x in 0..w {
                let ch = grid.get_char(x, y);
                if ch != '\u{2800}' && ch != ' ' {
                    let roll = hash3(x as i32, y as i32, slot);
                    let color = if roll > 0.92 {
                        FRINGE_MAGENTA
                    } else if roll > 0.84 {
                        FRINGE_CYAN
                    } else {
                        SIGNAL
                    };
                    let _ = grid.set_cell_color(x, y, color);
                }
            }
        }
        Ok(())
    }
}

/// All styles in the `glitch` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(SignalRestore)),
        Box::new(Tinted(Datamosh)),
        Box::new(RgbSplit),
        Box::new(Tinted(StaticSweep)),
        Box::new(Tinted(Dropout)),
        Box::new(ScanlineRoll),
        Box::new(Tinted(Bitcrush)),
        Box::new(Hexfade),
        Box::new(Tinted(TearFill)),
        Box::new(SyncLock),
    ]
}

/// A bar whose dropout holes heal as the signal is restored.
struct SignalRestore;
impl ProgressStyle for SignalRestore {
    fn name(&self) -> &str {
        "signal-restore"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "Dropout holes healing as signal returns"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let filled = (ctx.eased * w as f32).round() as usize;
        let holes = 1.0 - ctx.progress;
        let slot = (ctx.time * 10.0) as i32;
        for y in 0..h {
            for x in 0..filled {
                // Holes drift each slot and shrink as progress rises.
                if hash3((x / 3) as i32, (y / 2) as i32, slot) < holes * 0.55 {
                    continue;
                }
                draw::dot(grid, x, y);
            }
        }
        // Faint static beyond the fill.
        for y in 0..h {
            for x in filled..w {
                if hash3(x as i32, y as i32, slot) < 0.03 {
                    draw::dot(grid, x, y);
                }
            }
        }
        Ok(())
    }
}

/// Row bands shear sideways like a moshed keyframe.
struct Datamosh;
impl ProgressStyle for Datamosh {
    fn name(&self) -> &str {
        "datamosh"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "Row bands shearing like moshed video"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let filled = ctx.eased * w as f32;
        let slot = (ctx.time * 4.0) as i32;
        for y in 0..h {
            let band = (y / 3) as i32;
            let mut shear = (hash3(band, 0, slot) - 0.5) * 6.0;
            // Occasionally a band tears hard.
            if hash3(band, 1, slot) > 0.85 {
                shear *= 3.0;
            }
            let end = (filled + shear).max(0.0) as usize;
            for x in 0..end.min(w) {
                draw::dot(grid, x, y);
            }
        }
        Ok(())
    }
}

/// The bar splits into offset magenta/cyan copies around a white core.
struct RgbSplit;
impl ProgressStyle for RgbSplit {
    fn name(&self) -> &str {
        "rgb-split"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "Chromatic-aberration fringes around a white bar"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        let jit = 1 + (1.0 + 1.5 * (ctx.time * TAU * 0.75).sin()).max(0.0) as usize;
        let y0 = h / 4;
        let y1 = h.saturating_sub(h / 4);
        // Magenta copy: shifted left/up.
        for y in y0.saturating_sub(1)..y1.saturating_sub(1) {
            for x in 0..filled.saturating_sub(jit) {
                draw::dot(grid, x, y);
                let _ = grid.set_cell_color(x / 2, y / 4, FRINGE_MAGENTA);
            }
        }
        // Cyan copy: shifted right/down.
        for y in (y0 + 1)..(y1 + 1).min(h) {
            for x in jit..(filled + jit).min(w) {
                draw::dot(grid, x, y);
                let _ = grid.set_cell_color(x / 2, y / 4, FRINGE_CYAN);
            }
        }
        // White core drawn last so overlap reads clean.
        for y in y0..y1 {
            for x in 0..filled {
                draw::dot(grid, x, y);
                let _ = grid.set_cell_color(x / 2, y / 4, SIGNAL);
            }
        }
        Ok(())
    }
}

/// A band of television static sweeps ahead of the clean fill.
struct StaticSweep;
impl ProgressStyle for StaticSweep {
    fn name(&self) -> &str {
        "static-sweep"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "A static band sweeping ahead of clean fill"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let edge = ctx.eased * w as f32;
        let band = 10.0;
        let slot = (ctx.time * 12.0) as i32;
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32;
                if fx < edge {
                    draw::dot(grid, x, y);
                } else if fx < edge + band {
                    // Fresh white noise re-rolled every frame.
                    let falloff = 1.0 - (fx - edge) / band;
                    if hash3(x as i32, y as i32, slot) < 0.55 * falloff {
                        draw::dot(grid, x, y);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Whole scan bands black out for a beat, ghosting a shifted copy.
struct Dropout;
impl ProgressStyle for Dropout {
    fn name(&self) -> &str {
        "dropout"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "Scan bands blinking out and ghosting back"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let filled = (ctx.eased * w as f32).round() as usize;
        let slot = (ctx.time * 6.0) as i32;
        for y in 0..h {
            let band = (y / 2) as i32;
            let out = hash3(band, 40, slot) < 0.12;
            if out {
                // Ghost: a dim shifted stub where the band went missing.
                let shift = ((hash3(band, 41, slot) - 0.5) * 8.0) as i32;
                for x in (0..filled).step_by(2) {
                    draw::dot_i(grid, x as i32 + shift, y as i32);
                }
            } else {
                for x in 0..filled {
                    draw::dot(grid, x, y);
                }
            }
        }
        Ok(())
    }
}

/// A bright scanline rolls down the bar, bending dots as it passes.
struct ScanlineRoll;
impl ProgressStyle for ScanlineRoll {
    fn name(&self) -> &str {
        "scanline-roll"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "A rolling scanline warping the fill"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        let scan = ((ctx.time * 0.5).fract() * h as f32) as usize;
        for y in 0..h {
            let near = (y as i32 - scan as i32).abs();
            let warp = if near <= 1 { 2 } else { 0 };
            for x in 0..filled {
                draw::dot(grid, (x + warp).min(w.saturating_sub(1)), y);
            }
            // Interlace shading: alternate cell rows dimmer.
            let color = if near <= 1 {
                SIGNAL
            } else if (y / 2) % 2 == 0 {
                Color::rgb(200, 205, 214)
            } else {
                NOISE_GRAY
            };
            for cx in 0..(filled / 2 + 1).min(grid.dimensions().0) {
                let _ = grid.set_cell_color(cx, y / 4, color);
            }
        }
        Ok(())
    }
}

/// The fill edge snaps to chunky blocks that pop and flicker in.
struct Bitcrush;
impl ProgressStyle for Bitcrush {
    fn name(&self) -> &str {
        "bitcrush"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "A fill quantized into popping blocks"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        let chunk = 3usize;
        let cols = (cw / chunk).max(1);
        let lit = ctx.eased * cols as f32;
        let slot = (ctx.time * 8.0) as i32;
        for col in 0..cols {
            let fc = col as f32;
            let state = if fc + 1.0 <= lit {
                2 // solid
            } else if fc < lit {
                1 // popping in
            } else {
                0
            };
            if state == 0 {
                continue;
            }
            for y in 0..ch {
                for k in 0..chunk {
                    let x = col * chunk + k;
                    if x >= cw {
                        break;
                    }
                    if state == 2 {
                        draw::glyph(grid, x, y, '█');
                    } else if hash3(x as i32, y as i32, slot) < 0.6 {
                        draw::glyph(grid, x, y, '▓');
                    }
                }
            }
        }
        Ok(())
    }
}

/// A hex readout brightens into place as the dump completes.
struct Hexfade;
impl ProgressStyle for Hexfade {
    fn name(&self) -> &str {
        "hexfade"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "A hex dump resolving byte by byte"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        const HEX: [char; 16] = [
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
        ];
        let (cw, ch) = grid.dimensions();
        grid.enable_color_support();
        let total = cw * ch;
        let solved = (ctx.eased * total as f32).round() as usize;
        let slot = (ctx.time * 10.0) as i32;
        for y in 0..ch {
            for x in 0..cw {
                let idx = y * cw + x;
                if (x + 1) % 3 == 0 {
                    continue; // byte spacing
                }
                if idx < solved {
                    let pick = (hash2(x as i32, y as i32) * 16.0) as usize;
                    draw::glyph(grid, x, y, HEX[pick.min(15)]);
                    let _ = grid.set_cell_color(x, y, SIGNAL);
                } else if idx < solved + cw / 2 {
                    // The churn zone: digits still flipping.
                    let pick = (hash3(x as i32, y as i32, slot) * 16.0) as usize;
                    draw::glyph(grid, x, y, HEX[pick.min(15)]);
                    let _ = grid.set_cell_color(x, y, FRINGE_CYAN);
                } else {
                    draw::glyph(grid, x, y, '·');
                    let _ = grid.set_cell_color(x, y, NOISE_GRAY);
                }
            }
        }
        Ok(())
    }
}

/// The fill edge is a jagged, flickering tear.
struct TearFill;
impl ProgressStyle for TearFill {
    fn name(&self) -> &str {
        "tear-fill"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "A fill with a jagged flickering tear edge"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let filled = ctx.eased * w as f32;
        let slot = (ctx.time * 8.0) as i32;
        for y in 0..h {
            let jag = (hash3(y as i32, 0, slot) - 0.5) * 8.0;
            let end = (filled + jag).max(0.0) as usize;
            for x in 0..end.min(w) {
                draw::dot(grid, x, y);
            }
            // A stray shard past the tear.
            if hash3(y as i32, 1, slot) > 0.8 {
                let sx = (filled + jag + 4.0).max(0.0) as usize;
                for k in 0..3 {
                    draw::dot(grid, (sx + k).min(w.saturating_sub(1)), y);
                }
            }
        }
        Ok(())
    }
}

/// A rolling, noisy picture locks steady as sync is regained.
/// Rolling signal picture that settles as progress approaches lock.
pub struct SyncLock;
impl ProgressStyle for SyncLock {
    fn name(&self) -> &str {
        "sync-lock"
    }
    fn theme(&self) -> &str {
        "glitch"
    }
    fn describe(&self) -> &str {
        "A rolling picture locking into sync"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let chaos = (1.0 - ctx.progress).powf(1.5);
        // Vertical hold: the whole picture rolls until sync locks.
        let roll = (chaos * (ctx.time * 1.0).fract() * h as f32) as usize;
        let bar_h = (h / 3).max(1);
        let y_top = h / 3;
        let slot = (ctx.time * 12.0) as i32;
        for y in 0..h {
            let src_y = (y + roll) % h;
            let in_bar = src_y >= y_top && src_y < y_top + bar_h;
            for x in 0..w {
                let noise = hash3(x as i32, y as i32, slot) < chaos * 0.35;
                if in_bar != noise {
                    draw::dot(grid, x, y);
                    let color = if noise { NOISE_GRAY } else { SIGNAL };
                    let _ = grid.set_cell_color(x / 2, y / 4, color);
                }
            }
        }
        // Percent readout locks in with the signal.
        if let Some(label) = &ctx.label {
            let (cw, ch_cells) = grid.dimensions();
            let chars: Vec<char> = label.chars().collect();
            if ctx.progress > 0.5 && cw > chars.len() + 1 && ch_cells > 0 {
                let x0 = (cw - chars.len()) / 2;
                let y0 = ch_cells / 2;
                for (i, c) in chars.iter().enumerate() {
                    draw::glyph(grid, x0 + i, y0, *c);
                    let _ = grid.set_cell_color(x0 + i, y0, FRINGE_CYAN);
                }
            }
        }
        Ok(())
    }
}
