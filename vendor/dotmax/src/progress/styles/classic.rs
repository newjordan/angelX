//! Classic / professional progress bars — the reference theme.
//!
//! These six styles demonstrate every technique the themed bars build on:
//! solid fill, gradient tinting, discrete blocks, a scrolling indeterminate
//! shimmer, an eased "rocket" fill, and a ticked percentage track. Copy any
//! one of these as a starting point for a new bar.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Theme tint — the default cyan into violet. Applied to styles that draw monochrome.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(0, 200, 255);
const TINT_END: Color = Color::rgb(120, 80, 255);

/// Sample the theme gradient at `t` in `0.0..=1.0`.
fn sample_tint(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
    Color::rgb(
        lerp(TINT_START.r, TINT_END.r),
        lerp(TINT_START.g, TINT_END.g),
        lerp(TINT_START.b, TINT_END.b),
    )
}

/// Applies the theme's signature gradient to every cell the inner style drew,
/// drifting slowly with `time`. Styles stay monochrome-safe underneath: drop
/// the wrapper in [`styles`] for uncolored output.
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
                    let t = (x as f32 / w.max(1) as f32 + ctx.time * 0.05).fract();
                    let tri = 1.0 - (2.0 * t - 1.0).abs();
                    let _ = grid.set_cell_color(x, y, sample_tint(tri));
                }
            }
        }
        Ok(())
    }
}

/// All styles in the `classic` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(SolidFill)),
        Box::new(Gradient),
        Box::new(Tinted(Blocks)),
        Box::new(Tinted(Shimmer)),
        Box::new(Tinted(Rocket)),
        Box::new(Tinted(TickedTrack)),
    ]
}

/// A plain filled bar with a one-dot border.
struct SolidFill;
impl ProgressStyle for SolidFill {
    fn name(&self) -> &str {
        "solid"
    }
    fn theme(&self) -> &str {
        "classic"
    }
    fn describe(&self) -> &str {
        "Bordered solid fill"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        draw::rect_outline(grid, 0, 0, w, h);
        if w > 4 && h > 4 {
            let inner = w - 4;
            let filled = (ctx.eased * inner as f32).round() as usize;
            draw::fill_rect(grid, 2, 2, filled, h - 4);
        }
        Ok(())
    }
}

/// A solid fill tinted with the palette's start→end gradient.
struct Gradient;
impl ProgressStyle for Gradient {
    fn name(&self) -> &str {
        "gradient"
    }
    fn theme(&self) -> &str {
        "classic"
    }
    fn describe(&self) -> &str {
        "Gradient-tinted fill"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let filled = (ctx.eased * w as f32).round() as usize;
        draw::fill_rect(grid, 0, 0, filled, h);
        // Tint per cell-column across the filled span.
        let (cells_w, _) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32).round() as usize;
        for cx in 0..filled_cells {
            let t = if filled_cells <= 1 {
                0.0
            } else {
                cx as f32 / (filled_cells - 1) as f32
            };
            let color = ctx.palette.sample(t);
            let (_, cells_h) = grid.dimensions();
            draw::tint_row(grid, cells_h / 2, cx, cx, color);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}

/// Discrete blocks with a one-dot gap, like a segmented LED meter.
struct Blocks;
impl ProgressStyle for Blocks {
    fn name(&self) -> &str {
        "blocks"
    }
    fn theme(&self) -> &str {
        "classic"
    }
    fn describe(&self) -> &str {
        "Segmented block meter"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let seg_count = 16usize;
        let seg_w = (w / seg_count).max(1);
        let gap = 1;
        let lit = (ctx.eased * seg_count as f32).round() as usize;
        for s in 0..lit.min(seg_count) {
            let x0 = s * seg_w;
            let bw = seg_w.saturating_sub(gap).max(1);
            draw::fill_rect(grid, x0, 1, bw, h.saturating_sub(2).max(1));
        }
        Ok(())
    }
}

/// An indeterminate scrolling shimmer (uses `time`, ignores `progress`).
struct Shimmer;
impl ProgressStyle for Shimmer {
    fn name(&self) -> &str {
        "shimmer"
    }
    fn theme(&self) -> &str {
        "classic"
    }
    fn describe(&self) -> &str {
        "Indeterminate scrolling shimmer"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let band = (w / 4).max(3);
        let travel = w + band;
        let head = ((ctx.time * 0.6).fract() * travel as f32) as i32 - band as i32;
        for i in 0..band as i32 {
            let x = head + i;
            // Triangular intensity: fuller in the middle of the band.
            let edge = ((i as f32 / band as f32) * PI).sin();
            let col_h = (edge * h as f32).round() as usize;
            let y0 = (h - col_h) / 2;
            for y in y0..y0 + col_h {
                draw::dot_i(grid, x, y as i32);
            }
        }
        Ok(())
    }
}

/// An eased fill with a glowing leading edge — uses `ctx.eased` for snap.
struct Rocket;
impl ProgressStyle for Rocket {
    fn name(&self) -> &str {
        "rocket"
    }
    fn theme(&self) -> &str {
        "classic"
    }
    fn describe(&self) -> &str {
        "Eased fill with a bright leading edge"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let head = (ctx.eased * w as f32).round() as usize;
        let mid = h / 2;
        // Body: a thick center line that thins toward the start.
        for x in 0..head {
            let frac = if head <= 1 {
                1.0
            } else {
                x as f32 / head as f32
            };
            let thick = (frac * (h as f32 / 2.0)).round() as usize;
            draw::vline(grid, x, mid.saturating_sub(thick), (mid + thick).min(h - 1));
        }
        // Leading edge flare.
        if head < w {
            draw::vline(grid, head, 0, h - 1);
        }
        Ok(())
    }
}

/// A thin baseline with tick marks every 10% and a fill underline.
struct TickedTrack;
impl ProgressStyle for TickedTrack {
    fn name(&self) -> &str {
        "ticked"
    }
    fn theme(&self) -> &str {
        "classic"
    }
    fn describe(&self) -> &str {
        "Ruler track with 10% ticks"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let base = h - 1;
        draw::hline(grid, 0, w - 1, base);
        for i in 0..=10 {
            let x = (i as f32 / 10.0 * (w - 1) as f32).round() as usize;
            let tick = if i % 5 == 0 { h / 2 } else { h / 4 };
            draw::vline(grid, x, base.saturating_sub(tick), base);
        }
        let filled = (ctx.eased * w as f32).round() as usize;
        draw::hline(grid, 0, filled.min(w - 1), base.saturating_sub(1));
        draw::hline(grid, 0, filled.min(w - 1), base);
        Ok(())
    }
}
