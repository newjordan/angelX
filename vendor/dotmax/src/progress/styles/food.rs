//! Food / kitchen progress bars — a full set of animated, braille-rendered
//! loading styles themed around cooking, eating, and the kitchen.
//!
//! Every bar is stateless: it is a pure function of `(ctx.eased, ctx.time)`.
//! Bubbles rise, steam drifts, conveyor plates slide, and popcorn pops — all
//! driven by `ctx.time` so the bars stay alive at any fixed progress value.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Deterministic hash — gives pseudo-random f32 in [0, 1) from any u32 seed.
// ---------------------------------------------------------------------------
#[inline]
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

#[inline]
fn hashf(n: u32) -> f32 {
    (hash(n) % 1_000) as f32 / 1_000.0
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

/// Beer glass filling with a foam head and rising bubbles.
struct BeerGlass;
impl ProgressStyle for BeerGlass {
    fn name(&self) -> &str {
        "beer-glass"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "Beer glass fills with amber liquid; foam head and rising bubbles animate over time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 4 {
            return Ok(());
        }

        // Glass outline — tapered slightly: wider at top, narrower at base.
        let left = 1usize;
        let right = w.saturating_sub(2);
        // Left and right walls
        draw::vline(grid, left, 0, h - 1);
        draw::vline(grid, right, 0, h - 1);
        // Base
        draw::hline(grid, left, right, h - 1);

        // Liquid level from the bottom, driven by eased progress.
        let liquid_h = (ctx.eased * (h - 2) as f32).round() as usize;
        let liquid_top = h.saturating_sub(1).saturating_sub(liquid_h);
        if liquid_h > 0 {
            draw::fill_rect(
                grid,
                left + 1,
                liquid_top,
                right.saturating_sub(left + 1).max(1),
                liquid_h,
            );
        }

        // Foam head: 1–2 dot rows above the liquid.
        let foam_rows = ((ctx.eased * 2.0).round() as usize).min(2);
        for fr in 0..foam_rows {
            let fy = liquid_top.saturating_sub(fr + 1);
            // Bumpy foam: dots spaced every 2.
            let mut fx = left + 1;
            while fx < right {
                draw::dot(grid, fx, fy);
                fx += 2;
            }
        }

        // Bubbles: small dots that rise from the bottom of the liquid.
        if liquid_h > 1 {
            let bubble_count = 4usize;
            for i in 0..bubble_count {
                let bx = left
                    + 1
                    + (hashf(i as u32) * (right.saturating_sub(left + 2)).max(1) as f32) as usize;
                // Period varies per bubble so they don't all move in sync.
                let period = 1.5 + hashf(i as u32 + 100) * 2.0;
                let phase = hashf(i as u32 + 200);
                let t = (ctx.time / period + phase).fract();
                // Only show bubble while it's inside the liquid column.
                let by_raw = liquid_top + ((1.0 - t) * liquid_h as f32) as usize;
                if by_raw < h.saturating_sub(1) {
                    draw::dot(grid, bx.min(right.saturating_sub(1)), by_raw);
                }
            }
        }

        // Palette tint — amber/gold over the liquid rows.
        let (cw, ch) = grid.dimensions();
        let liq_cell_top = liquid_top / 4;
        for cy in liq_cell_top..ch {
            let t = if liquid_h == 0 {
                0.5
            } else {
                (cy.saturating_sub(liq_cell_top)) as f32 / ch.max(1) as f32
            };
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Coffee cup filling with steam wisps drifting upward.
struct CoffeePour;
impl ProgressStyle for CoffeePour {
    fn name(&self) -> &str {
        "coffee-pour"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "Coffee cup fills with dark brew; sinusoidal steam wisps drift upward as it warms"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 6 || h < 4 {
            return Ok(());
        }

        // Cup outline.
        let left = w / 6;
        let right = w.saturating_sub(w / 6 + 1);
        draw::hline(grid, left, right, h - 1); // base
        draw::vline(grid, left, h / 3, h - 1); // left wall
        draw::vline(grid, right, h / 3, h - 1); // right wall
        draw::hline(grid, left, right, h / 3); // rim

        // Handle: a small arc on the right side.
        let hx = right + 1;
        let hmid = (h / 3 + h - 1) / 2;
        draw::dot(grid, hx.min(w - 1), hmid.saturating_sub(1));
        draw::dot(grid, hx.min(w - 1), hmid);
        draw::dot(grid, hx.min(w - 1), hmid + 1);

        // Coffee fill from bottom of cup up.
        let cup_inner_h = h.saturating_sub(h / 3 + 2);
        let fill_h = (ctx.eased * cup_inner_h as f32).round() as usize;
        if fill_h > 0 {
            let fill_top = h.saturating_sub(1).saturating_sub(fill_h);
            draw::fill_rect(
                grid,
                left + 1,
                fill_top,
                right.saturating_sub(left + 1).max(1),
                fill_h,
            );
        }

        // Steam wisps above the cup: sine waves drifting up with time.
        let wisp_count = 3usize;
        let steam_amount = ctx.eased;
        for i in 0..wisp_count {
            if steam_amount < (i as f32 * 0.3) {
                continue;
            }
            let base_x =
                left + 1 + i * ((right.saturating_sub(left + 1)).max(3) / (wisp_count).max(1));
            let phase = (i as f32) * 2.0 * PI / wisp_count as f32;
            let rise_speed = 0.8 + i as f32 * 0.3;
            for dot_y in 0..h / 3 {
                // Phase advances with time to make the wisp drift upward.
                let t = dot_y as f32 / (h / 3).max(1) as f32;
                let drift_t = (ctx.time * rise_speed + t * 3.0 + phase).sin() * 2.0;
                let sx = base_x as i32 + drift_t as i32;
                // Fade out near top (only draw some dots for wispiness).
                let density = ((1.0 - t) * 3.0) as usize;
                if dot_y % (density.max(1)) == 0 {
                    draw::dot_i(grid, sx, dot_y as i32);
                }
            }
        }

        // Palette tint over the fill area.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let color = ctx.palette.sample(cy as f32 / ch.max(1) as f32);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Pizza being eaten: slices disappear as progress rises.
struct PizzaSlices;
impl ProgressStyle for PizzaSlices {
    fn name(&self) -> &str {
        "pizza-slices"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "Pizza loses slices as progress climbs — radial wedges removed from 8-slice pie"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 4 {
            return Ok(());
        }

        let cx = w / 2;
        let cy = h / 2;
        let r = (w.min(h) / 2).saturating_sub(1).max(1);

        // How many slices remain (out of 8). At progress=0 → 8 slices, at 1 → 0.
        let total_slices = 8usize;
        let eaten = (ctx.eased * total_slices as f32).round() as usize;
        let remaining = total_slices.saturating_sub(eaten);

        // Draw filled wedges for remaining slices.
        let slice_angle = 2.0 * PI / total_slices as f32;
        // Rotate so we start at the top.
        let start_offset = -PI / 2.0;

        for s in 0..remaining {
            let a0 = start_offset + s as f32 * slice_angle;
            let a1 = a0 + slice_angle;
            // Rasterise the wedge by scanning all dots in bounding box.
            for dy in 0..h {
                for dx in 0..w {
                    let fx = dx as f32 - cx as f32;
                    let fy = dy as f32 - cy as f32;
                    let dist = (fx * fx + fy * fy).sqrt();
                    if dist > r as f32 {
                        continue;
                    }
                    let mut angle = fy.atan2(fx);
                    // Normalise angle into [a0, a1] range.
                    while angle < a0 {
                        angle += 2.0 * PI;
                    }
                    if angle <= a1 {
                        draw::dot(grid, dx, dy);
                    }
                }
            }
        }

        // Crust ring.
        for dot_y in 0..h {
            for dot_x in 0..w {
                let fx = dot_x as f32 - cx as f32;
                let fy = dot_y as f32 - cy as f32;
                let dist = (fx * fx + fy * fy).sqrt();
                if dist >= r as f32 && dist <= r as f32 + 1.0 {
                    draw::dot(grid, dot_x, dot_y);
                }
            }
        }

        // Palette tint.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let color = ctx.palette.sample(cy_c as f32 / ch.max(1) as f32);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Boiling pot: liquid heat-fills from bottom, bubbles pop at the surface.
struct BoilingPot;
impl ProgressStyle for BoilingPot {
    fn name(&self) -> &str {
        "boiling-pot"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "Pot fills with boiling liquid; bubbles pop vigorously at the surface driven by time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 4 {
            return Ok(());
        }

        // Pot outline: rounded bottom, straight walls, rim with handles.
        let left = 2usize;
        let right = w.saturating_sub(3);
        let rim = 1usize;
        let base = h - 1;

        // Pot walls.
        draw::vline(grid, left, rim, base);
        draw::vline(grid, right, rim, base);
        // Pot base.
        draw::hline(grid, left + 1, right.saturating_sub(1), base);
        // Rim.
        draw::hline(grid, left, right, rim);
        // Handles.
        draw::dot(grid, left.saturating_sub(1), rim);
        draw::dot(grid, left.saturating_sub(1), rim + 1);
        draw::dot(grid, right + 1, rim);
        draw::dot(grid, right + 1, rim + 1);

        // Fill: liquid from base upward.
        let inner_w = right.saturating_sub(left + 1).max(1);
        let inner_h = base.saturating_sub(rim + 1).max(1);
        let fill_h = (ctx.eased * inner_h as f32).round() as usize;
        if fill_h > 0 {
            let fill_top = base.saturating_sub(fill_h);
            draw::fill_rect(grid, left + 1, fill_top, inner_w, fill_h);

            // Bubbles at the surface: time-driven pop timing.
            let surface_y = fill_top;
            let bubble_count = 6usize;
            for i in 0..bubble_count {
                let bx_frac = hashf(i as u32 + 77);
                let bx = left + 1 + (bx_frac * (inner_w.saturating_sub(1)) as f32) as usize;
                // Each bubble pops on its own period.
                let period = 0.4 + hashf(i as u32 + 200) * 0.8;
                let t = (ctx.time / period + hashf(i as u32 + 300)).fract();
                // Pop arc: rises a few dots above the surface then disappears.
                if t < 0.5 {
                    let rise = (t * 2.0 * PI).sin(); // 0..0 parabola via sine half-wave
                    let bubble_y = surface_y as i32 - (rise * 3.0) as i32;
                    draw::dot_i(grid, bx as i32, bubble_y);
                    // Small splat ring at apex.
                    if t > 0.3 && t < 0.45 {
                        draw::dot_i(grid, bx as i32 - 1, bubble_y);
                        draw::dot_i(grid, bx as i32 + 1, bubble_y);
                    }
                }
            }
        }

        // Palette tint.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let color = ctx.palette.sample(cy as f32 / ch.max(1) as f32);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Sushi conveyor: plates slide right, count served = eased.
struct SushiConveyor;
impl ProgressStyle for SushiConveyor {
    fn name(&self) -> &str {
        "sushi-conveyor"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "Sushi plates slide along a conveyor belt; plates served scales with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 6 || h < 3 {
            return Ok(());
        }

        // Two belt rails.
        let rail_top = 0usize;
        let rail_bot = h.saturating_sub(1);
        draw::hline(grid, 0, w - 1, rail_top);
        draw::hline(grid, 0, w - 1, rail_bot);

        // Belt notches (static texture).
        let notch_gap = 4usize;
        let mut nx = 0;
        while nx < w {
            draw::dot(grid, nx, rail_top + 1);
            draw::dot(grid, nx, rail_bot.saturating_sub(1));
            nx += notch_gap;
        }

        // Total plates to show on the belt at full progress.
        let max_plates = (w / 10).max(2);
        let served = (ctx.eased * max_plates as f32).round() as usize;

        // Plate width/height in dots.
        let pw = 6usize;
        let ph = h.saturating_sub(2).max(2);
        let plate_gap = (w / max_plates.max(1)).max(pw + 2);
        let belt_speed = 8.0_f32; // dots per second

        for i in 0..max_plates {
            // Shift each plate with time so they slide right.
            let base_x = i * plate_gap;
            let scroll = (ctx.time * belt_speed) as usize % (w + pw);
            let plate_x = (base_x + scroll) % (w + pw);

            // Only draw plates that have been "served" (within eased count).
            if i >= served {
                continue;
            }

            let px0 = plate_x.min(w.saturating_sub(1));
            // Plate oval: fill a rounded rect.
            let ph_inner = ph.saturating_sub(2).max(1);
            draw::fill_rect(grid, px0, 1 + 1, pw.min(w.saturating_sub(px0)), ph_inner);
            // Plate border top/bottom.
            draw::hline(grid, px0, (px0 + pw).min(w - 1), 1);
            draw::hline(grid, px0, (px0 + pw).min(w - 1), 1 + ph_inner + 1);

            // Nigiri topping: a small bump in the center of the plate.
            let top_x = px0 + pw / 2;
            if top_x < w {
                draw::dot(grid, top_x, 2);
                draw::dot(grid, top_x, 3);
            }
        }

        // Palette tint on belt rows.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = cy as f32 / ch.max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Popcorn kernels launching on parabolic arcs, filling a box.
struct PopcornPopping;
impl ProgressStyle for PopcornPopping {
    fn name(&self) -> &str {
        "popcorn-popping"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "Kernels pop on random parabolic arcs; the box fills with fluffy popcorn as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 4 {
            return Ok(());
        }

        // Box outline.
        draw::rect_outline(grid, 0, 0, w, h);

        // Settled popcorn pile: fill from the bottom up, scaled by eased.
        let pile_h = (ctx.eased * (h - 2) as f32).round() as usize;
        if pile_h > 0 {
            // Wavy top edge with small sine bumps.
            let pile_top = h.saturating_sub(1).saturating_sub(pile_h);
            draw::fill_rect(
                grid,
                1,
                pile_top + 1,
                w.saturating_sub(2).max(1),
                pile_h.saturating_sub(1),
            );
            // Wavy surface: alternate dots on top row.
            for px in (1..w.saturating_sub(1)).step_by(2) {
                let wave = ((px as f32 * 0.7 + ctx.time * 0.5).sin() * 1.0) as i32;
                draw::dot_i(grid, px as i32, pile_top as i32 + wave);
            }
        }

        // Airborne popcorn: each kernel follows a parabolic arc.
        let kernel_count = 8usize;
        for i in 0..kernel_count {
            // Each kernel has a random launch time offset.
            let offset = hashf(i as u32) * 2.0;
            let period = 0.6 + hashf(i as u32 + 50) * 0.7;
            let t = ((ctx.time * 1.2 + offset) / period).fract();

            // Only in the air during rising phase (t < 0.7).
            if t > 0.7 {
                continue;
            }
            if ctx.eased < hashf(i as u32 + 10) * 0.9 {
                continue;
            } // threshold before launch

            // Horizontal: random x near center spread.
            let lx = 2 + (hashf(i as u32 + 20) * (w.saturating_sub(4)) as f32) as usize;
            // Vertical: parabola — peaks in the middle of the phase.
            let up = -((t / 0.35 - 1.0).powi(2) - 1.0); // 0→1→0 arc
            let ky = (h as f32 - 2.0) * (1.0 - up * 0.85) - 1.0;
            let ky = ky as i32;

            draw::dot_i(grid, lx as i32, ky);
            draw::dot_i(grid, lx as i32 + 1, ky);
            draw::dot_i(grid, lx as i32, ky - 1);
        }

        // Palette tint.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let color = ctx.palette.sample(cy as f32 / ch.max(1) as f32);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Egg hourglass: sand empties from top chamber, fills bottom chamber.
struct EggTimer;
impl ProgressStyle for EggTimer {
    fn name(&self) -> &str {
        "egg-timer"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "Egg-timer hourglass drains sand from the top into the bottom as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 6 {
            return Ok(());
        }

        let mid_y = h / 2;
        let cx = w / 2;

        // Outer hourglass silhouette — two triangles meeting at the waist.
        for dy in 0..mid_y {
            let spread = ((1.0 - dy as f32 / mid_y as f32) * (cx as f32 - 1.0)).round() as usize;
            let x0 = cx.saturating_sub(spread);
            let x1 = (cx + spread).min(w - 1);
            draw::dot(grid, x0, dy);
            draw::dot(grid, x1, dy);
        }
        for dy in mid_y..h {
            let spread =
                (((dy - mid_y) as f32 / mid_y.max(1) as f32) * (cx as f32 - 1.0)).round() as usize;
            let x0 = cx.saturating_sub(spread);
            let x1 = (cx + spread).min(w - 1);
            draw::dot(grid, x0, dy);
            draw::dot(grid, x1, dy);
        }
        // Waist pinch (single dot at center for each side).
        draw::dot(grid, cx.saturating_sub(1), mid_y);
        draw::dot(grid, (cx + 1).min(w - 1), mid_y);

        // Top sand (draining): fills from top downward, decreases as progress rises.
        let top_fill = ((1.0 - ctx.eased) * (mid_y as f32 - 1.0)).round() as usize;
        for row in 0..top_fill {
            let spread =
                ((1.0 - row as f32 / mid_y.max(1) as f32) * (cx as f32 - 2.0)).round() as usize;
            let x0 = cx.saturating_sub(spread);
            let x1 = (cx + spread).min(w - 1);
            if x1 > x0 {
                draw::hline(grid, x0 + 1, x1.saturating_sub(1), row);
            }
        }

        // Bottom sand (accumulating): fills from bottom upward as progress rises.
        let bot_fill = (ctx.eased * (mid_y as f32 - 1.0)).round() as usize;
        for row in 0..bot_fill {
            let abs_row = h.saturating_sub(1).saturating_sub(row);
            let spread_t = row as f32 / mid_y.max(1) as f32;
            let spread = (spread_t * (cx as f32 - 2.0)).round() as usize;
            let x0 = cx.saturating_sub(spread);
            let x1 = (cx + spread).min(w - 1);
            if x1 > x0 {
                draw::hline(grid, x0 + 1, x1.saturating_sub(1), abs_row);
            }
        }

        // Falling sand particle: a dot that travels from waist to mid when in-flight.
        if ctx.eased > 0.0 && ctx.eased < 1.0 {
            let fall_t = (ctx.time * 3.0).fract();
            let particle_y = mid_y + (fall_t * mid_y as f32) as usize;
            draw::dot(grid, cx, particle_y.min(h - 1));
        }

        // Palette tint.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let color = ctx.palette.sample(cy as f32 / ch.max(1) as f32);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Pancake stack growing one pancake at a time.
struct PancakeStack;
impl ProgressStyle for PancakeStack {
    fn name(&self) -> &str {
        "pancake-stack"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "A stack of pancakes grows taller with each progress increment; syrup drips animate"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 3 {
            return Ok(());
        }

        let max_cakes = 6usize;
        let cake_h = (h / max_cakes).max(2);
        let count = (ctx.eased * max_cakes as f32).ceil() as usize;

        for i in 0..count.min(max_cakes) {
            let y0 = h.saturating_sub((i + 1) * cake_h);
            let y1 = h.saturating_sub(i * cake_h + 1);

            // Pancake ellipse: taller in center, tapers on sides.
            for dy in y0..=y1 {
                let t = if y1 <= y0 {
                    0.5
                } else {
                    (dy - y0) as f32 / (y1 - y0).max(1) as f32
                };
                let rel = 1.0 - (t * 2.0 - 1.0).powi(2); // 0→1→0 bulge
                let half = ((w as f32 / 2.0) * (0.6 + rel * 0.4)).round() as usize;
                let cx = w / 2;
                let px0 = cx.saturating_sub(half);
                let px1 = (cx + half).min(w - 1);
                draw::hline(grid, px0, px1, dy);
            }

            // Syrup drip: wiggly vertical line on right side, time-animated.
            let drip_x = w * 3 / 4;
            let drip_start = y0;
            let drip_end = y1 + 1;
            for dy in drip_start..drip_end {
                let sine = (ctx.time * 4.0 + dy as f32 * 0.5).sin() * 1.5;
                let dx = drip_x as i32 + sine as i32;
                draw::dot_i(grid, dx, dy as i32);
            }
        }

        // Top animation: a last pancake floating down if mid-progress.
        let frac = (ctx.eased * max_cakes as f32).fract();
        if frac > 0.0 && frac < 1.0 && count > 0 {
            let stack_top = h.saturating_sub(count * cake_h);
            let fall_offset = ((1.0 - frac) * (cake_h as f32 * 2.0)) as usize;
            let landing_y = stack_top.saturating_sub(fall_offset.min(h));
            let half = w / 3;
            let cx = w / 2;
            draw::hline(
                grid,
                cx.saturating_sub(half),
                (cx + half).min(w - 1),
                landing_y,
            );
        }

        // Palette tint.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let color = ctx.palette.sample(cy as f32 / ch.max(1) as f32);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Candy jar filling with jellybeans.
struct CandyJar;
impl ProgressStyle for CandyJar {
    fn name(&self) -> &str {
        "candy-jar"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "A glass candy jar fills with jelly beans whose colors shift across the palette"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 4 {
            return Ok(());
        }

        // Jar outline: wide body, narrow lid.
        let left = 1usize;
        let right = w.saturating_sub(2);
        let lid_h = (h / 5).max(1);
        let body_h = h.saturating_sub(lid_h);

        // Lid.
        draw::hline(grid, left + 1, right.saturating_sub(1), 0);
        draw::hline(grid, left, right, lid_h);
        draw::vline(grid, left + 1, 0, lid_h);
        draw::vline(grid, right.saturating_sub(1), 0, lid_h);

        // Jar body.
        draw::vline(grid, left, lid_h, h - 1);
        draw::vline(grid, right, lid_h, h - 1);
        draw::hline(grid, left, right, h - 1);

        // Fill beans: stack from the bottom.
        let inner_h = body_h.saturating_sub(2).max(1);
        let _inner_w = right.saturating_sub(left + 1).max(1);
        let fill_h = (ctx.eased * inner_h as f32).round() as usize;

        if fill_h > 0 {
            let fill_top = h.saturating_sub(1).saturating_sub(fill_h);
            // Individual beans: 2-dot oval shapes in a pseudo-random grid.
            let bean_rows = fill_h / 3;
            for row in 0..bean_rows {
                let by = fill_top + row * 3;
                let row_offset = if row % 2 == 0 { 0usize } else { 2 };
                let mut bx = left + 1 + row_offset;
                let mut bi = 0u32;
                while bx + 2 < right {
                    // Each bean is a 2-wide dot pair.
                    let jitter = (hashf(bi + row as u32 * 37) * 1.0) as i32;
                    draw::dot_i(grid, bx as i32, by as i32 + jitter);
                    draw::dot_i(grid, bx as i32 + 1, by as i32 + jitter);
                    draw::dot_i(grid, bx as i32, by as i32 + jitter + 1);
                    bx += 4;
                    bi += 1;
                }
            }
        }

        // Palette tint for color variety on the beans.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let t = 1.0 - cy as f32 / ch.max(1) as f32; // invert so colors change row by row
            let color = ctx.palette.sample((t + ctx.time * 0.05).fract());
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Noodle being slurped: a wiggly spaghetti strand shortens from right to left.
struct NoodleSlurp;
impl ProgressStyle for NoodleSlurp {
    fn name(&self) -> &str {
        "noodle-slurp"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "A spaghetti strand wiggles and shortens from the right as it is slurped up"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 2 {
            return Ok(());
        }

        let mid_y = h / 2;
        // Remaining noodle length shrinks as eased rises.
        let noodle_len = ((1.0 - ctx.eased) * w as f32).round() as usize;

        if noodle_len > 0 {
            // Draw noodle from left edge to noodle_len with time-driven sine wiggle.
            let strand_count = 3usize;
            for s in 0..strand_count {
                let y_base = mid_y.saturating_sub(s);
                for px in 0..noodle_len.min(w) {
                    // Phase and frequency per strand for variety.
                    let freq = 0.4 + s as f32 * 0.15;
                    let speed = 3.0 + s as f32 * 1.5;
                    let phase = s as f32 * PI / strand_count as f32;
                    let wave = ((px as f32 * freq + ctx.time * speed + phase).sin()
                        * (h as f32 * 0.2)) as i32;
                    draw::dot_i(grid, px as i32, y_base as i32 + wave);
                }
            }

            // "Mouth" at the leading edge: a small vertical bite marker.
            let mouth_x = noodle_len.min(w.saturating_sub(1));
            draw::vline(
                grid,
                mouth_x,
                mid_y.saturating_sub(1),
                (mid_y + 1).min(h - 1),
            );
        }

        // Sauce splatter near origin: dots scattered around x=0 with time jitter.
        for i in 0..4u32 {
            let period = 0.5 + hashf(i + 500) * 0.5;
            let t = (ctx.time / period + hashf(i + 600)).fract();
            if t < 0.15 {
                let sx = (hashf(i + 700) * 5.0) as i32;
                let sy = mid_y as i32 + (hashf(i + 800) * 4.0) as i32 - 2;
                draw::dot_i(grid, sx, sy);
            }
        }

        // Palette tint.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let color = ctx.palette.sample(cy as f32 / ch.max(1) as f32);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Toast browning in a toaster: color darkens from golden to dark brown via palette.
struct ToastBrowning;
impl ProgressStyle for ToastBrowning {
    fn name(&self) -> &str {
        "toast-browning"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "Toast darkens from pale gold to deep brown inside a toaster; the toast pops up at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 4 {
            return Ok(());
        }

        // Toaster body: outer rectangle.
        draw::rect_outline(grid, 0, h / 3, w, h.saturating_sub(h / 3));

        // Slots in the toaster top.
        let slot_w = (w / 3).max(2);
        let slot_x1 = w / 4;
        let slot_x2 = w * 3 / 4;
        draw::hline(
            grid,
            slot_x1,
            slot_x1 + slot_w.min(w.saturating_sub(slot_x1)),
            h / 3,
        );
        draw::hline(
            grid,
            slot_x2.saturating_sub(slot_w / 2),
            slot_x2 + slot_w / 2,
            h / 3,
        );

        // Toast rectangles: two slices popping up.
        let pop_h = if ctx.eased >= 0.99 {
            // Full pop: toast rises above the toaster.
            h / 3 + 1
        } else {
            // Toast is inside: visible height = eased * slot depth.
            let slot_depth = h / 3;
            (ctx.eased * slot_depth as f32).round() as usize
        };

        // Left slice.
        let tx1 = slot_x1 + 1;
        let tw1 = slot_w.saturating_sub(2).max(1);
        if pop_h > 0 {
            let toast_top = h / 3 + 1;
            let toast_bot = (toast_top + tw1).min(h - 1);
            // Draw toast shape: rectangle.
            draw::fill_rect(
                grid,
                tx1,
                toast_top.saturating_sub(pop_h),
                tw1,
                pop_h.min(toast_bot.saturating_sub(toast_top) + 1),
            );
        }

        // Right slice (mirrored).
        let tx2 = slot_x2.saturating_sub(slot_w / 2) + 1;
        if pop_h > 0 {
            let toast_top = h / 3 + 1;
            draw::fill_rect(
                grid,
                tx2,
                toast_top.saturating_sub(pop_h),
                slot_w.saturating_sub(2).max(1),
                pop_h,
            );
        }

        // Palette tint darkens as progress increases — lighter at start, darker at end.
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            // Color the toast darker based on progress.
            let brown_t = ctx.eased;
            let color = ctx.palette.sample(brown_t);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// Pie chart / progress: radial sweep fills a circular pie from 0→2π.
struct PieSweep;
impl ProgressStyle for PieSweep {
    fn name(&self) -> &str {
        "pie-sweep"
    }
    fn theme(&self) -> &str {
        "food"
    }
    fn describe(&self) -> &str {
        "A pie dish fills clockwise from 0° to 360°; a pulsing crust ring surrounds the fill"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w < 4 || h < 4 {
            return Ok(());
        }

        let cx = w / 2;
        let cy = h / 2;
        let r = (w.min(h) / 2).saturating_sub(1).max(1);

        let filled_angle = ctx.eased * 2.0 * PI;

        for dy in 0..h {
            for dx in 0..w {
                let fx = dx as f32 - cx as f32;
                let fy = dy as f32 - cy as f32;
                let dist = (fx * fx + fy * fy).sqrt();
                if dist > r as f32 {
                    continue;
                }

                // Angle measured clockwise from top (12-o'clock).
                let mut angle = fy.atan2(fx) + PI / 2.0;
                if angle < 0.0 {
                    angle += 2.0 * PI;
                }

                if angle <= filled_angle {
                    draw::dot(grid, dx, dy);
                }
            }
        }

        // Crust: pulsing ring that shimmers with time.
        let pulse = 0.5 + 0.5 * (ctx.time * 2.0 * PI * 0.5).sin();
        let crust_r = r as f32 + 0.5 + pulse * 0.5;
        for dy in 0..h {
            for dx in 0..w {
                let fx = dx as f32 - cx as f32;
                let fy = dy as f32 - cy as f32;
                let dist = (fx * fx + fy * fy).sqrt();
                if (dist - crust_r).abs() < 1.0 {
                    draw::dot(grid, dx, dy);
                }
            }
        }

        // Palette tint radiating from center.
        let (cw, ch) = grid.dimensions();
        for cy_c in 0..ch {
            let t = cy_c as f32 / ch.max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy_c, 0, cw.saturating_sub(1), color);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// All styles in the `food` theme.
///
/// Returns one boxed [`ProgressStyle`] per food/kitchen bar. Call this to get
/// the full set for display in a gallery or picker.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(BeerGlass),
        Box::new(CoffeePour),
        Box::new(PizzaSlices),
        Box::new(BoilingPot),
        Box::new(SushiConveyor),
        Box::new(PopcornPopping),
        Box::new(EggTimer),
        Box::new(PancakeStack),
        Box::new(CandyJar),
        Box::new(NoodleSlurp),
        Box::new(ToastBrowning),
        Box::new(PieSweep),
    ]
}
