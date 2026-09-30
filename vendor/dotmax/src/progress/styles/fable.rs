//! Fable progress bars — quiet, storybook motion in indigo, gold and moonlight.
//!
//! Twelve small scenes, each with its own mechanic: ink blooming in water,
//! lanterns in procession, moths spiralling to a flame, a constellation being
//! woven, fireflies over a meadow, a quill stroke, paper folding open, a
//! phosphor tide, meshing clockwork, a murmuration, a sideways hourglass and
//! a river carrying light. Every scene is deterministic in `(progress, time)`
//! and loops seamlessly at 4 s (all rates are multiples of 0.25 Hz).

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

/// 3-D variant for time-slotted flicker.
#[inline]
fn hash3(x: i32, y: i32, z: i32) -> f32 {
    hash2(x ^ z.wrapping_mul(1_234_567), y ^ z.wrapping_mul(7_654_321))
}

// ─── palette ────────────────────────────────────────────────────────────────

/// Deep indigo — the night the stories happen in.
const F_NIGHT: Color = Color::rgb(58, 72, 150);
/// Dusk indigo for unlit structure.
const F_DUSK: Color = Color::rgb(68, 74, 118);
/// Lantern gold.
const F_GOLD: Color = Color::rgb(240, 192, 96);
/// Ember orange for flames.
const F_EMBER: Color = Color::rgb(255, 122, 69);
/// Moonlight, near-white with a blue cast.
const F_MOON: Color = Color::rgb(238, 243, 255);
/// Sage green for living things.
const F_SAGE: Color = Color::rgb(127, 208, 160);
/// Phosphor green — the terminal's own colour.
const F_PHOSPHOR: Color = Color::rgb(110, 231, 160);
/// Ink blue-black for wet ink.
const F_INK: Color = Color::rgb(40, 48, 96);
/// Paper cream.
const F_PAPER: Color = Color::rgb(236, 226, 198);

/// Blend two colors at `t` in `0.0..=1.0`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let l = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
    Color::rgb(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b))
}

/// Tint the cell containing dot `(x, y)`, ignoring out-of-range.
fn tint_dot(grid: &mut BrailleGrid, x: i32, y: i32, c: Color) {
    if x < 0 || y < 0 {
        return;
    }
    let (w, h) = grid.dimensions();
    let (cx, cy) = ((x / 2) as usize, (y / 4) as usize);
    if cx < w && cy < h {
        let _ = grid.set_cell_color(cx, cy, c);
    }
}

/// Set a dot and tint its cell in one go.
fn ink(grid: &mut BrailleGrid, x: i32, y: i32, c: Color) {
    draw::dot_i(grid, x, y);
    tint_dot(grid, x, y, c);
}

/// Wash every cell with `c` (the unlit background tone).
fn wash(grid: &mut BrailleGrid, c: Color) {
    let (w, h) = grid.dimensions();
    for cy in 0..h {
        draw::tint_row(grid, cy, 0, w.saturating_sub(1), c);
    }
}

/// Dotted line between two points (every `step`-th dot), tinted.
fn line(grid: &mut BrailleGrid, x0: f32, y0: f32, x1: f32, y1: f32, step: usize, c: Color) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as usize).max(1);
    for i in (0..=n).step_by(step.max(1)) {
        let t = i as f32 / n as f32;
        let x = (x0 + (x1 - x0) * t).round() as i32;
        let y = (y0 + (y1 - y0) * t).round() as i32;
        ink(grid, x, y, c);
    }
}

/// Filled ellipse disc.
fn disc(grid: &mut BrailleGrid, cx: f32, cy: f32, rx: f32, ry: f32, c: Color) {
    if rx < 0.5 || ry < 0.5 {
        ink(grid, cx.round() as i32, cy.round() as i32, c);
        return;
    }
    let y0 = (cy - ry).floor() as i32;
    let y1 = (cy + ry).ceil() as i32;
    for y in y0..=y1 {
        let dy = (y as f32 - cy) / ry;
        if dy.abs() > 1.0 {
            continue;
        }
        let half = rx * (1.0 - dy * dy).sqrt();
        let x0 = (cx - half).round() as i32;
        let x1 = (cx + half).round() as i32;
        for x in x0..=x1 {
            ink(grid, x, y, c);
        }
    }
}

/// Ellipse outline traced by angle.
fn ring(grid: &mut BrailleGrid, cx: f32, cy: f32, rx: f32, ry: f32, c: Color) {
    let n = ((rx + ry) * 2.5).ceil().max(6.0) as usize;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        ink(
            grid,
            (cx + rx * a.cos()).round() as i32,
            (cy + ry * a.sin()).round() as i32,
            c,
        );
    }
}

/// All styles in the `fable` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(PhosphorTide),
        Box::new(InkBloom),
        Box::new(LanternProcession),
        Box::new(MothToFlame),
        Box::new(ConstellationWeave),
        Box::new(Fireflies),
        Box::new(QuillStroke),
        Box::new(PaperFold),
        Box::new(Clockwork),
        Box::new(Murmuration),
        Box::new(Hourglass),
        Box::new(RiverOfLight),
    ]
}

// ─── phosphor-tide ──────────────────────────────────────────────────────────

/// A CRT-green fill whose leading edge burns bright and whose body carries a
/// rolling scan wave; the phosphor behind the edge decays toward dark.
struct PhosphorTide;
impl ProgressStyle for PhosphorTide {
    fn name(&self) -> &str {
        "phosphor-tide"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "Phosphor fill with a burning edge and a rolling scan wave"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, mix(F_PHOSPHOR, Color::rgb(0, 0, 0), 0.8));
        let filled = (ctx.eased * wi as f32).round() as i32;
        let slot = (ctx.time * 12.0) as i32;
        // Track: a faint dotted rail so 0% still reads as a bar.
        for x in (0..wi).step_by(3) {
            ink(grid, x, hi / 2, mix(F_PHOSPHOR, Color::rgb(0, 0, 0), 0.6));
        }
        for x in 0..filled {
            // Scan wave: a sine ripple travelling right at 0.5 Hz.
            let phase = x as f32 * 0.35 - ctx.time * TAU * 0.5;
            let wave = phase.sin();
            let thick = (hi as f32 * (0.35 + 0.25 * wave)).round() as i32;
            let top = (hi / 2 - thick / 2).max(0);
            let bot = (top + thick).min(hi - 1);
            // Phosphor persistence: bright at the edge, dim well behind it.
            let behind = (filled - x) as f32 / wi as f32;
            let flicker = if hash3(x / 4, 0, slot) < 0.06 {
                0.35
            } else {
                0.0
            };
            let c = mix(
                F_PHOSPHOR,
                Color::rgb(6, 40, 20),
                (behind * 1.6 + flicker).min(0.85),
            );
            for y in top..=bot {
                // Interior dither so the wave shape stays visible in mono.
                if (x + y) % 2 == 0 || x > filled - 5 {
                    ink(grid, x, y, c);
                }
            }
        }
        // Burning leading edge, full height, moon-white core.
        if filled > 0 && filled <= wi {
            for dx in -1..=1 {
                let x = filled - 1 + dx;
                for y in 0..hi {
                    if dx == 0 || y % 2 == 0 {
                        ink(grid, x, y, if dx == 0 { F_MOON } else { F_PHOSPHOR });
                    }
                }
            }
        }
        // Retrace spark sweeping the whole track once per second.
        let spark = ((ctx.time * 1.0).fract() * wi as f32) as i32;
        ink(grid, spark, hi - 1, F_MOON);
        Ok(())
    }
}

// ─── ink-bloom ──────────────────────────────────────────────────────────────

/// Drops of ink land along the bar as progress rises and bloom outward in
/// water; the rings breathe with time until the blooms merge into a fill.
struct InkBloom;
impl ProgressStyle for InkBloom {
    fn name(&self) -> &str {
        "ink-bloom"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "Ink drops blooming in water until they merge"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wf, hf) = (w as f32, h as f32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        // Water line along the bottom.
        for x in (0..w as i32).step_by(2) {
            ink(grid, x, h as i32 - 1, F_NIGHT);
        }
        let drops = 9;
        let lit = ctx.eased * drops as f32;
        for i in 0..drops {
            let age = (lit - i as f32).clamp(0.0, 1.0);
            if age <= 0.0 {
                continue;
            }
            let cx = (i as f32 + 0.5) / drops as f32 * wf;
            let cy = hf * (0.35 + 0.35 * hash2(i, 7));
            // Bloom radius grows quickly then settles; capped so blooms overlap.
            let grow = 1.0 - (1.0 - age) * (1.0 - age);
            let rx = grow * (wf / drops as f32 * 0.62 + 1.0);
            let ry = grow * (hf * 0.4);
            disc(grid, cx, cy, rx, ry, mix(F_INK, F_NIGHT, 1.0 - age * 0.6));
            // Breathing ring on the surface, out of phase per drop.
            let breathe = 1.0 + 0.25 * (ctx.time * TAU * 0.5 + i as f32).sin();
            ring(
                grid,
                cx,
                cy,
                rx * breathe + 1.0,
                ry * breathe + 0.5,
                mix(F_MOON, F_NIGHT, 0.4),
            );
            // The newest drop still shows its falling dot above the surface.
            if age < 0.5 {
                let fall = (cy * (age * 2.0)).round() as i32;
                ink(grid, cx.round() as i32, fall, F_MOON);
            }
        }
        Ok(())
    }
}

// ─── lantern-procession ─────────────────────────────────────────────────────

/// Paper lanterns join a procession one by one, bobbing on their strings,
/// each flame flickering; the path beneath lights up as far as they've come.
struct LanternProcession;
impl ProgressStyle for LanternProcession {
    fn name(&self) -> &str {
        "lantern-procession"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "Paper lanterns joining a procession along a lit path"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        let slot = (ctx.time * 8.0) as i32;
        // The path: a dotted ground line lit gold as far as the procession.
        let lit_x = (ctx.eased * wi as f32).round() as i32;
        for x in (0..wi).step_by(2) {
            ink(grid, x, hi - 1, if x < lit_x { F_GOLD } else { F_NIGHT });
        }
        let lanes = 9;
        let count = (ctx.eased * lanes as f32).ceil() as i32;
        for i in 0..count.min(lanes) {
            let cx = ((i as f32 + 0.5) / lanes as f32 * wi as f32).round() as i32;
            // Bob on the string; the newest lantern rises into place.
            let arrive = (ctx.eased * lanes as f32 - i as f32).clamp(0.0, 1.0);
            let bob = (ctx.time * TAU * 0.25 + i as f32 * 0.9).sin() * 2.5;
            let cy = (hi as f32 * (0.45 + 0.4 * (1.0 - arrive)) + bob).round() as i32;
            let (rx, ry) = (2.5, 3.0);
            // Paper body: warm shell, dark inside except the flame.
            disc(
                grid,
                cx as f32,
                cy as f32,
                rx,
                ry,
                mix(F_GOLD, F_EMBER, 0.35),
            );
            // Cap and tassel.
            ink(grid, cx, cy - 4, F_NIGHT);
            ink(grid, cx, cy + 4, F_EMBER);
            ink(grid, cx, cy + 5, F_EMBER);
            // Flame flicker: the core pulses gold/white and the heat shimmer
            // above the cap jumps between two heights every slot.
            let flick = hash3(i, 1, slot);
            tint_dot(grid, cx, cy, if flick > 0.5 { F_MOON } else { F_GOLD });
            let shimmer = if flick > 0.5 { 6 } else { 5 };
            ink(
                grid,
                cx + if (slot + i) % 2 == 0 { 1 } else { -1 },
                cy - shimmer,
                F_EMBER,
            );
            // Soft glow halo on neighbouring cells, brighter when settled.
            if arrive > 0.6 {
                tint_dot(grid, cx - 3, cy, mix(F_GOLD, F_DUSK, 0.5));
                tint_dot(grid, cx + 3, cy, mix(F_GOLD, F_DUSK, 0.5));
            }
        }
        Ok(())
    }
}

// ─── moth-to-flame ──────────────────────────────────────────────────────────

/// A candle flame carries the progress head; moths spiral inward toward it,
/// wings beating, while the wick's trail smoulders behind.
struct MothToFlame;
impl ProgressStyle for MothToFlame {
    fn name(&self) -> &str {
        "moth-to-flame"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "Moths spiralling into the flame at the progress head"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        let slot = (ctx.time * 12.0) as i32;
        let head = 3 + (ctx.eased * (wi - 7).max(0) as f32).round() as i32;
        let base = hi - 2;
        // Burnt wick behind the flame; unburnt wick ahead, sparse.
        for x in 0..wi {
            if x < head {
                if x % 2 == 0 {
                    ink(grid, x, base, mix(F_EMBER, F_INK, 0.55));
                }
            } else if x % 4 == 0 {
                ink(grid, x, base, F_NIGHT);
            }
        }
        // Flame: a teardrop whose tip wavers with time.
        let lean = (ctx.time * TAU * 1.0).sin() * 1.2 + (hash3(0, 0, slot) - 0.5);
        let height = (hi - 3).max(2);
        for dy in 0..height {
            let t = dy as f32 / height as f32; // 0 at tip, 1 at base
            let half = (t * 2.2).round() as i32;
            let x0 = head + (lean * (1.0 - t)).round() as i32;
            let y = base - height + dy;
            for dx in -half..=half {
                let c = if half > 0 && dx.abs() == half {
                    F_EMBER
                } else if t > 0.6 {
                    F_MOON
                } else {
                    F_GOLD
                };
                ink(grid, x0 + dx, y, c);
            }
        }
        // Moths: each on its own slow spiral, a 3-dot body with beating wings.
        let moths = 6;
        for i in 0..moths {
            let phase = (ctx.time * 0.25 + i as f32 / moths as f32).fract();
            let radius = 3.0 + 18.0 * (1.0 - phase);
            let ang = ctx.time * TAU * 0.5 + i as f32 * 1.1;
            let mx = head as f32 + radius * ang.cos();
            let my = (base as f32 - height as f32 * 0.5) + radius * 0.35 * ang.sin();
            let (x, y) = (mx.round() as i32, my.round() as i32);
            if x < 0 || x >= wi || y < 0 || y >= hi {
                continue;
            }
            let c = mix(F_MOON, F_GOLD, 1.0 - phase);
            ink(grid, x, y, c);
            let flap = (slot + i) % 2 == 0;
            ink(grid, x - 1, if flap { y - 1 } else { y }, c);
            ink(grid, x + 1, if flap { y - 1 } else { y }, c);
        }
        Ok(())
    }
}

// ─── constellation-weave ────────────────────────────────────────────────────

/// Stars twinkle across the night; threads are woven between them one by
/// one as progress rises, the newest thread glowing gold.
struct ConstellationWeave;
impl ProgressStyle for ConstellationWeave {
    fn name(&self) -> &str {
        "constellation-weave"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "Threads woven star to star, the newest one gold"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wf, hf) = (w as f32, h as f32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        let slot = (ctx.time * 4.0) as i32;
        let crawl = (ctx.time * 6.0) as i32;
        let stars = 14;
        let pos = |i: i32| -> (f32, f32) {
            (
                (i as f32 + 0.3 + 0.4 * hash2(i, 3)) / stars as f32 * wf,
                1.0 + hash2(i, 5) * (hf - 3.0).max(0.0),
            )
        };
        // Edge list: a running chain plus long cross-threads for the weave.
        let mut edges = Vec::with_capacity(stars as usize * 2);
        for i in 0..stars - 1 {
            edges.push((i, i + 1));
            if i + 3 < stars && i % 2 == 0 {
                edges.push((i, i + 3));
            }
        }
        let lit = ctx.eased * edges.len() as f32;
        for (k, &(a, b)) in edges.iter().enumerate() {
            let t = (lit - k as f32).clamp(0.0, 1.0);
            if t <= 0.0 {
                continue;
            }
            let (x0, y0) = pos(a);
            let (x1, y1) = pos(b);
            // Partially drawn thread grows from a toward b.
            let (xe, ye) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            let newest = (k as f32) > lit - 1.5;
            let c = if newest { F_GOLD } else { F_NIGHT };
            // Thread dots crawl from star to star so the weave shimmers.
            let n = ((xe - x0).abs().max((ye - y0).abs()).ceil() as i32).max(1);
            for i in 0..=n {
                if (i + crawl).rem_euclid(3) != 0 {
                    continue;
                }
                let f = i as f32 / n as f32;
                ink(
                    grid,
                    (x0 + (xe - x0) * f).round() as i32,
                    (y0 + (ye - y0) * f).round() as i32,
                    c,
                );
            }
        }
        // Stars on top, twinkling: a bright cross when their slot is hot.
        for i in 0..stars {
            let (x, y) = pos(i);
            let (xi, yi) = (x.round() as i32, y.round() as i32);
            let tw = hash3(i, 9, slot);
            ink(grid, xi, yi, F_MOON);
            if tw > 0.55 {
                ink(grid, xi - 1, yi, F_MOON);
                ink(grid, xi + 1, yi, F_MOON);
                ink(grid, xi, yi - 1, F_MOON);
                ink(grid, xi, yi + 1, F_MOON);
            }
        }
        Ok(())
    }
}

// ─── fireflies ──────────────────────────────────────────────────────────────

/// Fireflies rise over a meadow, blinking on their own rhythms and drifting
/// on the breeze; the swarm thickens from left to right with progress.
struct Fireflies;
impl ProgressStyle for Fireflies {
    fn name(&self) -> &str {
        "fireflies"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "A blinking swarm of fireflies thickening over a meadow"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        // Meadow: grass blades along the bottom row, swaying.
        let sway = (ctx.time * TAU * 0.25).sin();
        for x in (0..wi).step_by(3) {
            let tall = 1 + ((hash2(x, 2) * 2.0) as i32);
            for dy in 0..=tall {
                let lean = ((sway * dy as f32) * 0.6).round() as i32;
                ink(grid, x + lean, hi - 1 - dy, mix(F_SAGE, F_INK, 0.5));
            }
        }
        // Fireflies: base position by hash, drift by slow sines, blink by slot.
        let total = 48;
        let slot = (ctx.time * 4.0) as i32;
        for i in 0..total {
            let bx = hash2(i, 11) * wi as f32;
            // Swarm reveals left to right; a few strays are always out.
            let reveal = ctx.eased * wi as f32 + 6.0;
            if bx > reveal && hash2(i, 13) > 0.08 {
                continue;
            }
            let by = 1.0 + hash2(i, 17) * (hi as f32 - 4.0).max(1.0);
            let dx = 3.0 * (ctx.time * TAU * 0.25 + hash2(i, 19) * TAU).sin();
            let dy = 1.5 * (ctx.time * TAU * 0.5 + hash2(i, 23) * TAU).cos();
            let (x, y) = ((bx + dx).round() as i32, (by + dy).round() as i32);
            // Each fly has its own blink duty; hot slots glow gold-white.
            let duty = 0.35 + 0.4 * hash2(i, 29);
            let on = hash3(i, 31, slot) < duty;
            if !on {
                continue;
            }
            let bright = hash3(i, 37, slot) > 0.6;
            let c = if bright {
                F_MOON
            } else {
                mix(F_GOLD, F_SAGE, 0.35)
            };
            ink(grid, x, y, c);
            if bright {
                tint_dot(grid, x - 2, y, mix(F_GOLD, F_DUSK, 0.6));
                tint_dot(grid, x + 2, y, mix(F_GOLD, F_DUSK, 0.6));
            }
        }
        Ok(())
    }
}

// ─── quill-stroke ───────────────────────────────────────────────────────────

/// A calligraphic stroke thickens and thins as the nib travels; the nib
/// trembles faintly and ink beads beneath it while the hand rests.
struct QuillStroke;
impl ProgressStyle for QuillStroke {
    fn name(&self) -> &str {
        "quill-stroke"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "A calligraphic ink stroke drawn by a trembling nib"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, mix(F_PAPER, F_DUSK, 0.55));
        // Ruled guide, faint.
        for x in (0..wi).step_by(4) {
            ink(grid, x, hi - 1, mix(F_PAPER, F_DUSK, 0.7));
        }
        let head = (ctx.eased * wi as f32).round() as i32;
        let mid = hi as f32 * 0.5;
        let amp = (hi as f32 * 0.28).max(1.0);
        let tremble = (ctx.time * TAU * 2.0).sin() * 0.45;
        for x in 0..head.min(wi) {
            let k = x as f32 * 0.16;
            let y = mid + amp * k.sin() + tremble;
            // Pressure: thick on the downstrokes, hairline on the ups.
            let pressure = 0.5 + 0.5 * k.cos();
            let thick = (1.0 + pressure * (hi as f32 * 0.35)).round() as i32;
            let y0 = (y - thick as f32 / 2.0).round() as i32;
            for dy in 0..thick.max(1) {
                let edge = dy == 0 || dy == thick - 1;
                ink(
                    grid,
                    x,
                    y0 + dy,
                    if edge {
                        mix(F_INK, F_NIGHT, 0.5)
                    } else {
                        F_INK
                    },
                );
            }
        }
        // The nib: a small angled tip ahead of the stroke, gold.
        if head < wi {
            let k = head as f32 * 0.16;
            let y = (mid + amp * k.sin() + tremble).round() as i32;
            ink(grid, head, y, F_GOLD);
            ink(grid, head + 1, y - 1, F_GOLD);
            ink(grid, head + 2, y - 2, F_MOON);
            // Ink bead swelling under the resting nib on a 1 s cycle.
            let bead = ((ctx.time * 1.0).fract() * 3.0) as i32;
            for dy in 1..=bead {
                ink(grid, head, y + dy, F_INK);
            }
        }
        Ok(())
    }
}

// ─── paper-fold ─────────────────────────────────────────────────────────────

/// An accordion of folded paper opens panel by panel; each new panel swings
/// out from its crease, and the whole sheet breathes gently in a draught.
struct PaperFold;
impl ProgressStyle for PaperFold {
    fn name(&self) -> &str {
        "paper-fold"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "Folded paper opening panel by panel"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        let panels = 11;
        let panel_w = (wi as f32 / panels as f32).max(1.0);
        let open = ctx.eased * panels as f32;
        let breathe = (ctx.time * TAU * 0.25).sin();
        let grain = (ctx.time * 6.0) as i32;
        let mut x_cursor = 0.0f32;
        for i in 0..panels {
            let t = (open - i as f32).clamp(0.0, 1.0);
            if t <= 0.0 {
                break;
            }
            // Width swings out from the crease as the panel opens.
            let width = panel_w * (0.15 + 0.85 * t);
            let x0 = x_cursor;
            let x1 = x_cursor + width;
            x_cursor = x1;
            // Alternate panels lean opposite ways; the draught rocks them.
            let lean = if i % 2 == 0 { 1.0 } else { -1.0 } * (1.0 - t) * 3.0
                + breathe * if i % 2 == 0 { 1.6 } else { -1.6 };
            let top = 1.0 + lean.max(0.0);
            let bot = hi as f32 - 2.0 + lean.min(0.0);
            let shade = if i % 2 == 0 {
                F_PAPER
            } else {
                mix(F_PAPER, F_DUSK, 0.45)
            };
            for x in (x0.round() as i32)..(x1.round() as i32).min(wi) {
                let f = (x as f32 - x0) / width.max(0.5);
                let yt = (top + (bot - top) * 0.0 + lean * f * 0.5).round() as i32;
                let yb = (bot - lean * f * 0.5).round() as i32;
                for y in yt..=yb.min(hi - 1) {
                    // Paper texture: mostly solid with a faint grain.
                    if (x * 7 + y * 3 + grain) % 5 != 0 || x == x0.round() as i32 {
                        ink(grid, x, y, shade);
                    }
                }
            }
            // Crease line, dark, on the panel's left edge.
            let cx = x0.round() as i32;
            for y in 0..hi {
                if y % 2 == 0 {
                    ink(grid, cx, y, F_INK);
                }
            }
        }
        Ok(())
    }
}

// ─── clockwork ──────────────────────────────────────────────────────────────

/// Gears assemble left to right as progress rises and mesh into a train,
/// each turning against its neighbour; the last gear carries the pointer.
struct Clockwork;
impl ProgressStyle for Clockwork {
    fn name(&self) -> &str {
        "clockwork"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "A gear train assembling and meshing as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        let cy = hi as f32 * 0.5;
        let ry = (hi as f32 * 0.5 - 1.5).max(1.5);
        let rx = ry * 2.0;
        let gears = ((wi as f32 / (rx * 2.0 + 3.0)).floor() as i32).clamp(1, 8);
        let spacing = wi as f32 / gears as f32;
        let assembled = ctx.eased * gears as f32;
        for i in 0..gears {
            let t = (assembled - i as f32).clamp(0.0, 1.0);
            if t <= 0.0 {
                break;
            }
            let cx = (i as f32 + 0.5) * spacing;
            // Slide in from just above the frame, then turn. Alternate
            // directions so neighbours mesh.
            let cyi = cy - (1.0 - t) * (hi as f32 * 0.45);
            let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
            let ang = dir * ctx.time * TAU * 0.25 + i as f32 * 0.3;
            let grx = rx * (0.8 + 0.2 * t);
            let gry = ry * (0.8 + 0.2 * t);
            let c = if t < 1.0 { F_MOON } else { F_GOLD };
            ring(grid, cx, cyi, grx, gry, c);
            // Teeth: 8 radial nubs.
            for k in 0..8 {
                let a = ang + k as f32 / 8.0 * TAU;
                let x = (cx + (grx + 1.5) * a.cos()).round() as i32;
                let y = (cyi + (gry + 1.0) * a.sin()).round() as i32;
                ink(grid, x, y, c);
            }
            // Spokes and hub.
            for k in 0..3 {
                let a = ang + k as f32 / 3.0 * TAU;
                line(
                    grid,
                    cx,
                    cyi,
                    cx + grx * 0.7 * a.cos(),
                    cyi + gry * 0.7 * a.sin(),
                    1,
                    mix(c, F_DUSK, 0.35),
                );
            }
            ink(grid, cx.round() as i32, cyi.round() as i32, F_EMBER);
        }
        // Escapement tick along the base: a dot stepping 4 Hz.
        let tick = ((ctx.time * 4.0) as i32).rem_euclid(wi.max(1));
        ink(grid, tick, hi - 1, F_MOON);
        Ok(())
    }
}

// ─── murmuration ────────────────────────────────────────────────────────────

/// A flock of starlings folds and stretches as one cloud, drifting across
/// the bar with progress; each bird follows its own blend of slow sines.
struct Murmuration;
impl ProgressStyle for Murmuration {
    fn name(&self) -> &str {
        "murmuration"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "A starling murmuration folding across the sky"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        // Horizon and the ground the flock has crossed.
        let crossed = (ctx.eased * wi as f32).round() as i32;
        for x in (0..wi).step_by(2) {
            ink(grid, x, hi - 1, if x < crossed { F_NIGHT } else { F_DUSK });
        }
        let birds = 70;
        let span = (wi as f32 - 16.0).max(1.0);
        let cx = 8.0 + ctx.eased * span;
        let cy = hi as f32 * 0.42;
        // The cloud itself stretches and folds on 0.25 / 0.5 Hz beats.
        let stretch = 1.0 + 0.45 * (ctx.time * TAU * 0.25).sin();
        let fold = 0.6 * (ctx.time * TAU * 0.5).sin();
        for i in 0..birds {
            let p1 = hash2(i, 41) * TAU;
            let p2 = hash2(i, 43) * TAU;
            let r = hash2(i, 47);
            let ox = (12.0 * r * (ctx.time * TAU * 0.25 + p1).sin()
                + 5.0 * (ctx.time * TAU * 0.5 + p2).cos())
                * stretch;
            let oy = 3.5 * r * (ctx.time * TAU * 0.5 + p2).sin()
                + fold * ox * 0.15
                + 1.5 * (ctx.time * TAU * 0.25 + p1).cos();
            let (x, y) = ((cx + ox).round() as i32, (cy + oy).round() as i32);
            if y < 0 || y >= hi - 1 {
                continue;
            }
            // Dense core reads dark, the fringe catches the moon.
            let c = if r < 0.6 {
                F_INK
            } else {
                mix(F_MOON, F_NIGHT, 0.4)
            };
            ink(grid, x, y, c);
        }
        Ok(())
    }
}

// ─── hourglass ──────────────────────────────────────────────────────────────

/// An hourglass laid on its side: sand drains from the left bulb through a
/// pulsing neck and settles in the right one as progress rises.
struct Hourglass;
impl ProgressStyle for Hourglass {
    fn name(&self) -> &str {
        "hourglass"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "Sand draining through a sideways hourglass"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        let mid = wi / 2;
        let cy = hi as f32 * 0.5;
        let half_h = (hi as f32 * 0.5 - 0.5).max(0.5);
        // Glass: two bulbs meeting at the neck, drawn as tapered outlines.
        for x in 0..wi {
            let d = (x - mid).abs() as f32 / mid.max(1) as f32; // 0 at neck, 1 at ends
            let r = 1.0 + (half_h - 1.0) * (d * 1.4).min(1.0);
            let (yt, yb) = ((cy - r).round() as i32, (cy + r).round() as i32);
            if x % 2 == 0 || d < 0.2 {
                ink(grid, x, yt, mix(F_MOON, F_DUSK, 0.45));
                ink(grid, x, yb, mix(F_MOON, F_DUSK, 0.45));
            }
        }
        // Sand: the left bulb empties from the neck outward... no — from the
        // far end inward, since sand nearest the neck flows first. The right
        // bulb fills from its far end back toward the neck.
        let left_remaining = 1.0 - ctx.eased;
        let left_edge = (mid as f32 * (1.0 - left_remaining)).round() as i32;
        let right_edge = mid + (mid as f32 * (1.0 - ctx.eased)).round() as i32;
        let slot = (ctx.time * 12.0) as i32;
        for x in 0..wi {
            let d = (x - mid).abs() as f32 / mid.max(1) as f32;
            let r = 1.0 + (half_h - 1.0) * (d * 1.4).min(1.0);
            let in_left = x < mid && x >= left_edge && x < mid - 1;
            let in_right = x > mid && x >= right_edge;
            if !(in_left || in_right) {
                continue;
            }
            let (yt, yb) = ((cy - r).round() as i32 + 1, (cy + r).round() as i32 - 1);
            for y in yt..=yb {
                // Grain: settled sand is solid, the surface facing the neck
                // sifts a little.
                let surface = if in_left {
                    x == left_edge
                } else {
                    x == right_edge
                };
                if surface && hash3(x, y, slot) < 0.4 {
                    continue;
                }
                ink(grid, x, y, if surface { F_MOON } else { F_GOLD });
            }
        }
        // The stream through the neck: grains stepping right at 12 Hz.
        if ctx.eased > 0.0 && ctx.eased < 1.0 {
            for k in 0..4 {
                let x = mid - 1 + ((slot + k * 2) % 6) - 1;
                ink(grid, x, cy.round() as i32, F_MOON);
            }
        }
        Ok(())
    }
}

// ─── river-of-light ─────────────────────────────────────────────────────────

/// A river meanders across the bar; packets of light drift downstream and
/// the water itself brightens from the source as progress rises.
struct RiverOfLight;
impl ProgressStyle for RiverOfLight {
    fn name(&self) -> &str {
        "river-of-light"
    }
    fn theme(&self) -> &str {
        "fable"
    }
    fn describe(&self) -> &str {
        "A meandering river carrying packets of light downstream"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (wi, hi) = (w as i32, h as i32);
        grid.enable_color_support();
        wash(grid, F_DUSK);
        let mid = hi as f32 * 0.5;
        let amp = (hi as f32 * 0.25).max(0.5);
        let lit = (ctx.eased * wi as f32).round() as i32;
        let flow = ctx.time * 12.0; // dots per second downstream
        for x in 0..wi {
            let y = mid + amp * (x as f32 * 0.14).sin();
            let (yi, half) = (y.round() as i32, 1);
            let bright = x < lit;
            // Banks: dotted, sage where the river is lit.
            if x % 2 == 0 {
                let bank = if bright { F_SAGE } else { F_NIGHT };
                ink(grid, x, yi - half - 2, bank);
                ink(grid, x, yi + half + 2, bank);
            }
            // Water: lit water is a dense weave, dark water a sparse trickle.
            for dy in -half..=half {
                if (bright && (x + dy) % 2 == 0) || (x + dy) % 3 == 0 {
                    ink(
                        grid,
                        x,
                        yi + dy,
                        if bright {
                            mix(F_SAGE, F_NIGHT, 0.55)
                        } else {
                            F_INK
                        },
                    );
                }
            }
            // Light packets: 5-dot pulses every 16 dots, sliding downstream,
            // only where the water is lit; a lead packet rides the front.
            let phase = ((x as f32 - flow).rem_euclid(16.0)) as i32;
            if bright && phase < 5 {
                let c = if phase == 2 { F_MOON } else { F_GOLD };
                for dy in -half..=half {
                    ink(grid, x, yi + dy, c);
                }
                if phase == 2 {
                    ink(grid, x, yi - half - 1, F_MOON);
                }
            }
        }
        // Source spring at the left, sparkling.
        let slot = (ctx.time * 6.0) as i32;
        if hash3(0, 0, slot) > 0.3 {
            ink(grid, 0, mid.round() as i32 - 1, F_MOON);
        }
        Ok(())
    }
}
