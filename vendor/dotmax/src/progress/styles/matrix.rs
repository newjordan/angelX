//! Digital-rain progress bars — cascading code, phosphor decay, boot logs.
//!
//! The signature dotmax theme: everything is green-on-black terminal light.
//! Styles range from classic falling-glyph rain that settles into a solid
//! bar, to CRT raster scans, packet streams, and a self-typing boot log.
//! All motion is deterministic in `(ctx.progress, ctx.time)`.

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

/// 3-D variant: hash `(x, y, z_int)` — useful for animating with a time slot.
#[inline]
fn hash3(x: i32, y: i32, z: i32) -> f32 {
    hash2(x ^ z.wrapping_mul(1_234_567), y ^ z.wrapping_mul(7_654_321))
}

// ─── theme tint — phosphor green ────────────────────────────────────────────

/// Deep phosphor green, the unlit end of the ramp.
const TINT_DARK: Color = Color::rgb(14, 92, 55);
/// Bright terminal green, the lit end of the ramp.
const TINT_BRIGHT: Color = Color::rgb(110, 231, 160);
/// White-hot head color for leading edges.
const TINT_HOT: Color = Color::rgb(214, 255, 226);

/// Sample the dark→bright green ramp at `t` in `0.0..=1.0`.
fn sample_tint(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
    Color::rgb(
        lerp(TINT_DARK.r, TINT_BRIGHT.r),
        lerp(TINT_DARK.g, TINT_BRIGHT.g),
        lerp(TINT_DARK.b, TINT_BRIGHT.b),
    )
}

/// Applies the phosphor-green tint to every cell the inner style drew, with a
/// per-cell shimmer so the field of glyphs never sits still.
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
        let slot = (ctx.time * 3.0) as i32;
        for y in 0..h {
            for x in 0..w {
                let ch = grid.get_char(x, y);
                if ch != '\u{2800}' && ch != ' ' {
                    let ramp = 0.25 + 0.45 * (x as f32 / w.max(1) as f32);
                    let shimmer = 0.3 * hash3(x as i32, y as i32, slot);
                    let _ = grid.set_cell_color(x, y, sample_tint(ramp + shimmer));
                }
            }
        }
        Ok(())
    }
}

/// All styles in the `matrix` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(RainFill),
        Box::new(Tinted(Decode)),
        Box::new(Trace),
        Box::new(Tinted(CascadeWipe)),
        Box::new(Tinted(CodeColumn)),
        Box::new(PhosphorTrail),
        Box::new(Downlink),
        Box::new(PacketBurst),
        Box::new(GreenNoiseFill),
        Box::new(TerminalBoot),
    ]
}

/// Falling code streams settle left-to-right into a solid bar.
struct RainFill;
impl ProgressStyle for RainFill {
    fn name(&self) -> &str {
        "rain-fill"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Digital rain settling into a solid bar"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let filled = (ctx.eased * w as f32).round() as usize;
        // Settled region: solid columns with a ragged, hash-torn top edge.
        for x in 0..filled {
            let torn = (hash2(x as i32, 7) * 2.5) as usize;
            for y in torn..h {
                draw::dot(grid, x, y);
            }
            let t = 0.35 + 0.45 * (x as f32 / w.max(1) as f32);
            draw::tint_row(grid, 0, x / 2, x / 2, sample_tint(t));
            for cy in 1..grid.dimensions().1 {
                draw::tint_row(grid, cy, x / 2, x / 2, sample_tint(t));
            }
        }
        // Active region: streams of rain with bright heads and fading tails.
        let cycle = (h * 2) as f32;
        for x in filled..w {
            let speed = 6.0 + hash2(x as i32, 1) * 8.0;
            // 0.25-multiple rates keep the 4 s capture loop seamless.
            let speed = (speed * 4.0).round() / 4.0;
            let head = ((ctx.time * speed + hash2(x as i32, 2) * cycle) % cycle) as i32;
            let tail = 3 + (hash2(x as i32, 3) * 4.0) as i32;
            for k in 0..=tail {
                let y = head - k;
                if y >= 0 && (y as usize) < h {
                    draw::dot(grid, x, y as usize);
                    let bright = if k == 0 {
                        TINT_HOT
                    } else {
                        sample_tint(0.8 - 0.2 * k as f32)
                    };
                    let _ = grid.set_cell_color(x / 2, y as usize / 4, bright);
                }
            }
        }
        Ok(())
    }
}

/// Scrambled code characters resolve into solid blocks at the leading edge.
struct Decode;
impl ProgressStyle for Decode {
    fn name(&self) -> &str {
        "decode"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Scrambled glyphs resolving into a solid bar"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        const CODE: [char; 12] = ['0', '1', '<', '>', '/', '#', '$', '+', '=', '*', '?', ';'];
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        let slot = (ctx.time * 10.0) as i32;
        for y in 0..ch {
            for x in 0..cw {
                if x + 2 < filled {
                    draw::glyph(grid, x, y, '█');
                } else if x < filled + 4 {
                    // Decode band: glyphs flip fast, density fades rightward.
                    let fade = 1.0 - (x as f32 - filled as f32 + 2.0) / 6.0;
                    if hash3(x as i32, y as i32, slot / 3) < fade {
                        let pick = (hash3(x as i32, y as i32, slot) * CODE.len() as f32) as usize;
                        draw::glyph(grid, x, y, CODE[pick.min(CODE.len() - 1)]);
                    }
                } else if hash3(x as i32, y as i32, slot / 4) < 0.05 {
                    // Sparse idle static far right of the edge.
                    draw::dot(grid, x * 2, y * 4 + 2);
                }
            }
        }
        Ok(())
    }
}

/// Light packets race along a phosphor track toward the leading edge.
struct Trace;
impl ProgressStyle for Trace {
    fn name(&self) -> &str {
        "trace"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Packets racing along a phosphor track"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let mid = h / 2;
        let filled = (ctx.eased * w as f32).round() as usize;
        // Dim dotted track across the whole width.
        for x in (0..w).step_by(3) {
            draw::dot(grid, x, mid);
            let _ = grid.set_cell_color(x / 2, mid / 4, TINT_DARK);
        }
        // Completed run: a solid double line.
        for x in 0..filled {
            draw::dot(grid, x, mid.saturating_sub(1));
            draw::dot(grid, x, mid);
            let t = 0.3 + 0.3 * (x as f32 / w.max(1) as f32);
            let _ = grid.set_cell_color(x / 2, mid / 4, sample_tint(t));
        }
        // Packets: bright dashes streaming through the completed run.
        if filled > 2 {
            for i in 0..3 {
                let phase = (ctx.time * 0.5 + i as f32 / 3.0).fract();
                let head = (phase * filled as f32) as usize;
                for k in 0..6 {
                    if head >= k {
                        let x = head - k;
                        draw::dot(grid, x, mid.saturating_sub(1));
                        draw::dot(grid, x, mid);
                        let c = if k < 2 {
                            TINT_HOT
                        } else {
                            sample_tint(0.9 - 0.15 * k as f32)
                        };
                        let _ = grid.set_cell_color(x / 2, mid / 4, c);
                    }
                }
            }
        }
        // Blinking terminal tick at the leading edge.
        if (ctx.time * 2.0) as i32 % 2 == 0 {
            let x = filled.min(w.saturating_sub(1));
            draw::vline(grid, x, mid.saturating_sub(3), (mid + 3).min(h - 1));
            let _ = grid.set_cell_color(x / 2, mid / 4, TINT_HOT);
        }
        Ok(())
    }
}

/// A glyph waterfall wipes across, dissolving from solid to mist.
struct CascadeWipe;
impl ProgressStyle for CascadeWipe {
    fn name(&self) -> &str {
        "cascade-wipe"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Waterfall wipe dissolving from solid to mist"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        let front = ctx.eased * (cw as f32 + 6.0);
        for y in 0..ch {
            for x in 0..cw {
                let rag = hash2(x as i32, y as i32) * 3.0;
                let drip = if hash2(x as i32, 40) < 0.25 {
                    y as f32 * 0.8
                } else {
                    0.0
                };
                let shimmer = 0.4 * (ctx.time * TAU * 0.5 + x as f32 * 0.7).sin();
                let depth = front - x as f32 - rag + drip + shimmer;
                let level = if depth >= 3.0 {
                    4
                } else if depth >= 2.0 {
                    3
                } else if depth >= 1.0 {
                    2
                } else if depth >= 0.0 {
                    1
                } else {
                    0
                };
                if level > 0 {
                    draw::shade(grid, x, y, level);
                }
            }
        }
        Ok(())
    }
}

/// Lines of code type themselves out, one blinking cursor at a time.
struct CodeColumn;
impl ProgressStyle for CodeColumn {
    fn name(&self) -> &str {
        "code-column"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Code lines typing in with a blinking cursor"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        let total = cw * ch;
        let typed = (ctx.eased * total as f32).round() as usize;
        for y in 0..ch {
            for x in 0..cw {
                let idx = y * cw + x;
                if idx >= typed {
                    continue;
                }
                // Pseudo source line: hash-chosen indent, chunks, and gaps.
                let indent = (hash2(y as i32, 0) * 5.0) as usize;
                if x < indent {
                    continue;
                }
                let chunk = hash2((x / 4) as i32, y as i32 + 31);
                if chunk < 0.72 && hash2(x as i32, y as i32 + 17) < 0.9 {
                    draw::glyph(grid, x, y, if chunk < 0.2 { '▓' } else { '█' });
                }
            }
        }
        // Blinking cursor at the caret.
        if typed < total && (ctx.time * 2.5) as i32 % 2 == 0 {
            draw::glyph(grid, typed % cw, typed / cw, '▌');
        }
        Ok(())
    }
}

/// A CRT raster scan lights the bar dot by dot, glow decaying behind it.
struct PhosphorTrail;
impl ProgressStyle for PhosphorTrail {
    fn name(&self) -> &str {
        "phosphor-trail"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "CRT raster scan with decaying phosphor glow"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let total = w * h;
        let lit = (ctx.eased * total as f32).round() as usize;
        let full_rows = lit / w.max(1);
        let partial = lit % w.max(1);
        for y in 0..full_rows.min(h) {
            draw::hline(grid, 0, w - 1, y);
        }
        if full_rows < h {
            for x in 0..partial {
                draw::dot(grid, x, full_rows);
            }
            // Scan-head flare.
            draw::vline(
                grid,
                partial.min(w.saturating_sub(1)),
                full_rows.saturating_sub(1),
                (full_rows + 1).min(h - 1),
            );
        }
        // Phosphor decay: brightness falls with raster distance behind the head.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            for cx in 0..cw {
                if grid.get_char(cx, cy) == '\u{2800}' {
                    continue;
                }
                let cell_order = (cy * 4 + 2) * w + cx * 2;
                let behind = lit.saturating_sub(cell_order) as f32 / total.max(1) as f32;
                let flicker = 0.08 * hash3(cx as i32, cy as i32, (ctx.time * 6.0) as i32);
                let _ = grid.set_cell_color(cx, cy, sample_tint(1.0 - behind * 1.4 + flicker));
            }
        }
        // White-hot head cell.
        if full_rows < h {
            let _ = grid.set_cell_color(
                (partial.min(w.saturating_sub(1))) / 2,
                full_rows / 4,
                TINT_HOT,
            );
        }
        Ok(())
    }
}

/// Data packets fall from above into a pool that rises with progress.
struct Downlink;
impl ProgressStyle for Downlink {
    fn name(&self) -> &str {
        "downlink"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Falling packets filling a rising pool"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let pool = (ctx.eased * h as f32).round() as usize;
        let surface = h.saturating_sub(pool);
        // Pool body with a rippling surface line.
        for y in surface..h {
            for x in 0..w {
                let ripple = (x as f32 * 0.5 + ctx.time * TAU * 0.25).sin();
                if y == surface && ripple < -0.2 {
                    continue;
                }
                draw::dot(grid, x, y);
                let depth = (y - surface) as f32 / pool.max(1) as f32;
                let _ = grid.set_cell_color(x / 2, y / 4, sample_tint(0.75 - depth * 0.5));
            }
        }
        // Falling packets above the surface.
        if surface > 1 {
            for x in 0..w {
                if hash2(x as i32, 3) > 0.45 {
                    continue;
                }
                let speed = 4.0 + (hash2(x as i32, 4) * 8.0 * 4.0).round() / 4.0;
                let y = ((ctx.time * speed + hash2(x as i32, 5) * surface as f32) % surface as f32)
                    as usize;
                for k in 0..3usize {
                    if y >= k {
                        draw::dot(grid, x, y - k);
                    }
                }
                let c = if y + 2 >= surface {
                    TINT_HOT
                } else {
                    sample_tint(0.9)
                };
                let _ = grid.set_cell_color(x / 2, y / 4, c);
            }
        }
        Ok(())
    }
}

/// Packets stream home through staggered lanes and stack into the bar.
struct PacketBurst;
impl ProgressStyle for PacketBurst {
    fn name(&self) -> &str {
        "packet-burst"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Lane packets arriving into a growing stack"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let lanes = (h / 4).max(1);
        let filled = (ctx.eased * w as f32).round() as usize;
        for lane in 0..lanes {
            let y0 = lane * 4 + 1;
            let rag = (hash2(lane as i32, 9) * 4.0) as usize;
            let lane_fill = filled.saturating_sub(rag);
            // The stacked, completed run.
            for x in 0..lane_fill {
                draw::dot(grid, x, y0);
                draw::dot(grid, x, y0 + 1);
                let t = 0.3 + 0.4 * (x as f32 / w.max(1) as f32);
                let _ = grid.set_cell_color(x / 2, y0 / 4, sample_tint(t));
            }
            // Inbound packet: travels right-to-left into the stack.
            if lane_fill + 4 < w {
                let rate = 0.25 + 0.25 * (lane % 3) as f32;
                let span = (w - lane_fill) as f32;
                let pos = w as f32 - (ctx.time * rate + hash2(lane as i32, 11)).fract() * span;
                let px = pos as usize;
                for k in 0..5 {
                    let x = px + k;
                    if x < w {
                        draw::dot(grid, x, y0);
                        draw::dot(grid, x, y0 + 1);
                        let _ = grid.set_cell_color(x / 2, y0 / 4, TINT_HOT);
                    }
                }
                // Arrival flash at the stack edge.
                if pos < lane_fill as f32 + 4.0 {
                    draw::vline(grid, lane_fill, y0.saturating_sub(1), (y0 + 2).min(h - 1));
                    let _ = grid.set_cell_color(lane_fill / 2, y0 / 4, TINT_HOT);
                }
            }
        }
        Ok(())
    }
}

/// Dithered static condenses into solid signal behind a fizzing edge.
struct GreenNoiseFill;
impl ProgressStyle for GreenNoiseFill {
    fn name(&self) -> &str {
        "green-noise-fill"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Static condensing into solid signal"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        grid.enable_color_support();
        let edge = ctx.eased * w as f32;
        let band = 12.0;
        let slot = (ctx.time * 8.0) as i32;
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32;
                let lit = if fx < edge - band {
                    // Solid signal with drifting pinprick holes.
                    hash3(x as i32, y as i32, (ctx.time * 3.0) as i32) < 0.96
                } else if fx < edge {
                    let t = (edge - fx) / band;
                    hash3(x as i32, y as i32, slot) < 0.05 + 0.9 * t
                } else {
                    hash3(x as i32, y as i32, slot) < 0.04
                };
                if lit {
                    draw::dot(grid, x, y);
                    let c = if fx >= edge - band && fx < edge {
                        sample_tint(0.85)
                    } else if fx < edge {
                        sample_tint(0.3 + 0.35 * (fx / w.max(1) as f32))
                    } else {
                        TINT_DARK
                    };
                    let _ = grid.set_cell_color(x / 2, y / 4, c);
                }
            }
        }
        Ok(())
    }
}

/// A terminal boot log types itself in, percent readout on the last line.
struct TerminalBoot;
impl ProgressStyle for TerminalBoot {
    fn name(&self) -> &str {
        "terminal-boot"
    }
    fn theme(&self) -> &str {
        "matrix"
    }
    fn describe(&self) -> &str {
        "Boot log typing in with a live percent readout"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        const TEXT: [char; 4] = ['▄', '▀', '█', '▐'];
        let (cw, ch) = grid.dimensions();
        grid.enable_color_support();
        let total = cw * ch;
        let typed = (ctx.eased * total as f32).round() as usize;
        for y in 0..ch {
            for x in 0..cw {
                if y * cw + x >= typed {
                    continue;
                }
                if x == 0 {
                    draw::glyph(grid, x, y, '>');
                    let _ = grid.set_cell_color(x, y, TINT_BRIGHT);
                    continue;
                }
                // Word-like chunks separated by hash-chosen gaps.
                let line_len = 3 + (hash2(y as i32, 1) * (cw as f32 * 0.85)) as usize;
                if x > line_len || hash2((x / 3) as i32, y as i32 + 5) < 0.25 {
                    continue;
                }
                let pick = (hash2(x as i32, y as i32) * TEXT.len() as f32) as usize;
                draw::glyph(grid, x, y, TEXT[pick.min(TEXT.len() - 1)]);
                let shimmer = 0.3 * hash3(x as i32, y as i32, (ctx.time * 2.0) as i32);
                let _ = grid.set_cell_color(x, y, sample_tint(0.45 + shimmer));
            }
        }
        // Blinking cursor at the caret.
        if typed < total && (ctx.time * 2.5) as i32 % 2 == 0 {
            draw::glyph(grid, typed % cw, typed / cw, '▌');
            let _ = grid.set_cell_color(typed % cw, typed / cw, TINT_HOT);
        }
        // Percent readout, bottom-right, in real text.
        if let Some(label) = &ctx.label {
            let chars: Vec<char> = label.chars().collect();
            if cw > chars.len() + 1 && ch > 0 {
                let x0 = cw - chars.len() - 1;
                for (i, c) in chars.iter().enumerate() {
                    draw::glyph(grid, x0 + i, ch - 1, *c);
                    let _ = grid.set_cell_color(x0 + i, ch - 1, TINT_HOT);
                }
            }
        }
        Ok(())
    }
}
