//! Retro / arcade-gaming progress bars.
//!
//! Ten distinct styles that evoke classic 80s and 90s arcade and home-console
//! aesthetics: Pac-Man pellets, Space Invader waves, Tetris stacking, RPG
//! segmented health bars, cassette reels, 8-bit blocky pixels, CRT scanlines,
//! a growing snake, pinball brick-breaker, and a combo power-up meter.
//!
//! All bars are stateless — animation derives purely from `ctx.time`, and fill
//! extent from `ctx.eased`. Every write goes through `draw::` helpers so
//! out-of-bounds coordinates are silently discarded.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

/// All styles in the `retro` theme.
///
/// Returns one boxed implementor per style. The vec is in display order —
/// register it with `all_styles` or iterate for a gallery picker.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(PacMan),
        Box::new(SpaceInvaders),
        Box::new(TetrisStack),
        Box::new(RpgHealthBar),
        Box::new(CassetteReels),
        Box::new(EightBitBlocks),
        Box::new(CrtScanline),
        Box::new(Snake),
        Box::new(PinballBricks),
        Box::new(ComboPower),
    ]
}

// ─── 1. Pac-Man ──────────────────────────────────────────────────────────────

struct PacMan;
impl ProgressStyle for PacMan {
    fn name(&self) -> &str {
        "pacman"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "Pac-Man chomps through a row of pellets; eaten count = progress"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let mid_y = h / 2;

        // How many pellets exist total across the track.
        let pellet_count = (w / 6).max(1);
        let pellet_spacing = w / pellet_count.max(1);
        let eaten = (ctx.eased * pellet_count as f32) as usize;

        // Draw uneaten pellets (small 2×2 squares).
        for p in eaten..pellet_count {
            let px = p * pellet_spacing + pellet_spacing / 2;
            draw::dot(grid, px, mid_y);
            draw::dot(grid, px.saturating_sub(1), mid_y);
            draw::dot(grid, px, mid_y.saturating_sub(1));
            draw::dot(grid, px.saturating_sub(1), mid_y.saturating_sub(1));
        }

        // Pac-Man position: just past the last eaten pellet.
        let pac_x = if eaten == 0 {
            0usize
        } else {
            (eaten.saturating_sub(1) * pellet_spacing + pellet_spacing / 2 + 3)
                .min(w.saturating_sub(1))
        };

        // Mouth angle oscillates with time — open/close cycle.
        let chomp = ((ctx.time * 6.0).sin() * 0.5 + 0.5) as f32; // 0..1
        let mouth_angle = chomp * (PI / 3.0); // 0 = fully open, PI/3 = widest

        // Pac-Man body: 5×5 dot circle, clipped by mouth wedge.
        let r = (h / 2).max(2) as i32;
        let cx = pac_x as i32;
        let cy = mid_y as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    // Mouth wedge: suppress dots in the forward-facing cone.
                    let angle = (dy as f32).atan2(dx as f32).abs();
                    if angle > mouth_angle {
                        draw::dot_i(grid, cx + dx, cy + dy);
                    }
                }
            }
        }

        // Tint eaten region yellow-ish, uneaten region dim.
        let (cells_w, cells_h) = grid.dimensions();
        let eaten_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            if eaten_cells > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    eaten_cells.saturating_sub(1),
                    ctx.palette.sample(0.85),
                ); // warm yellow via palette end
            }
        }

        Ok(())
    }
}

// ─── 2. Space Invaders ───────────────────────────────────────────────────────

struct SpaceInvaders;
impl ProgressStyle for SpaceInvaders {
    fn name(&self) -> &str {
        "space-invaders"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "A row of descending invaders; fill = alive count; legs toggle with time"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);

        let inv_w = 6usize; // dots wide per invader
        let inv_count = (w / inv_w).max(1);
        let alive = (ctx.eased * inv_count as f32).ceil() as usize;
        let leg_frame = ((ctx.time * 4.0) as usize) % 2; // 0 or 1

        for i in 0..alive.min(inv_count) {
            let base_x = i * inv_w;
            let cx = base_x + inv_w / 2; // center x
            let top_y = 0usize;

            // Body: 3-wide block in the upper rows.
            draw::hline(
                grid,
                cx.saturating_sub(1),
                (cx + 1).min(w.saturating_sub(1)),
                top_y + 1,
            );
            draw::hline(
                grid,
                cx.saturating_sub(2),
                (cx + 2).min(w.saturating_sub(1)),
                top_y + 2,
            );
            draw::hline(
                grid,
                cx.saturating_sub(1),
                (cx + 1).min(w.saturating_sub(1)),
                top_y + 3,
            );

            // Eyes: two dots.
            draw::dot(grid, cx.saturating_sub(1), top_y + 2);
            draw::dot(grid, (cx + 1).min(w.saturating_sub(1)), top_y + 2);

            // Antennae.
            if cx >= 2 {
                draw::dot(grid, cx - 2, top_y);
            }
            if cx + 2 < w {
                draw::dot(grid, cx + 2, top_y);
            }

            // Legs: alternate between two frames.
            let leg_y = top_y + 4;
            if leg_y < h {
                if leg_frame == 0 {
                    if cx >= 2 {
                        draw::dot(grid, cx - 2, leg_y);
                    }
                    if cx + 2 < w {
                        draw::dot(grid, cx + 2, leg_y);
                    }
                } else {
                    if cx >= 1 {
                        draw::dot(grid, cx - 1, leg_y);
                    }
                    if cx + 1 < w {
                        draw::dot(grid, cx + 1, leg_y);
                    }
                }
            }
        }

        // Tint alive invaders green.
        let (cells_w, cells_h) = grid.dimensions();
        let alive_cells = (ctx.eased * cells_w as f32).ceil() as usize;
        for cy in 0..cells_h {
            if alive_cells > 0 {
                let end = alive_cells.min(cells_w).saturating_sub(1);
                draw::tint_row(grid, cy, 0, end, ctx.palette.sample(0.4));
            }
        }

        Ok(())
    }
}

// ─── 3. Tetris Stack ─────────────────────────────────────────────────────────

struct TetrisStack;
impl ProgressStyle for TetrisStack {
    fn name(&self) -> &str {
        "tetris-stack"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "Tetris blocks stack from the bottom; stack height = progress"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let col_count = (w / 4).max(1);
        let col_w = 4usize;

        // Stack height for each column: vary by column index to create a ragged skyline.
        for col in 0..col_count {
            let phase = (col as f32 / col_count as f32) * PI;
            let col_frac = (ctx.eased + 0.15 * phase.sin()).clamp(0.0, 1.0);
            let stack_h = (col_frac * h as f32) as usize;
            if stack_h == 0 {
                continue;
            }

            let x0 = col * col_w;
            let bw = (col_w - 1).max(1);
            let y0 = h.saturating_sub(stack_h);

            // Draw the stacked column as a filled rect with outline.
            draw::fill_rect(grid, x0, y0, bw, stack_h);

            // Draw block divisions every 4 dots (simulate individual pieces).
            let mut seg_y = y0;
            while seg_y + 4 < y0 + stack_h {
                draw::hline(grid, x0, (x0 + bw).saturating_sub(1), seg_y + 3);
                seg_y += 4;
            }
        }

        // Tint by column using the palette gradient.
        let (_, cells_h) = grid.dimensions();
        let (cells_w, _) = grid.dimensions();
        for cx in 0..cells_w {
            let t = cx as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(t);
            let col_frac = ctx.eased;
            let stack_cells = (col_frac * cells_h as f32) as usize;
            let y_start = cells_h.saturating_sub(stack_cells);
            for cy in y_start..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ─── 4. RPG Health Bar ───────────────────────────────────────────────────────

struct RpgHealthBar;
impl ProgressStyle for RpgHealthBar {
    fn name(&self) -> &str {
        "rpg-health"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "Chunky segmented HP bar with a slow shine sweep — classic RPG style"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);

        // Outer border.
        draw::rect_outline(grid, 0, 0, w, h);

        // Segment geometry.
        let seg_count = 10usize;
        let inner_w = w.saturating_sub(4);
        let seg_w = (inner_w / seg_count).max(2);
        let gap = 1usize;
        let lit_segs = (ctx.eased * seg_count as f32).round() as usize;

        for s in 0..lit_segs.min(seg_count) {
            let x0 = 2 + s * seg_w;
            let bw = seg_w.saturating_sub(gap).max(1);
            let by = 2usize;
            let bh = h.saturating_sub(4).max(1);
            draw::fill_rect(grid, x0, by, bw, bh);
        }

        // Shine sweep: a bright vertical stripe traveling left→right over time.
        let shine_period = 3.0f32;
        let shine_t = (ctx.time % shine_period) / shine_period;
        let shine_x = (shine_t * inner_w as f32) as usize + 2;
        let shine_w = (inner_w / 8).max(2);
        for dx in 0..shine_w {
            let sx = shine_x + dx;
            if sx < w.saturating_sub(2) {
                // Only shine over filled segs.
                let frac = (sx as f32 - 2.0) / inner_w.max(1) as f32;
                if frac <= ctx.eased {
                    // Thin central bright line.
                    draw::dot(grid, sx, h / 2);
                }
            }
        }

        // Color: green when healthy, red when low.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32).round() as usize;
        let health_color = ctx.palette.sample(ctx.eased);
        for cy in 0..cells_h {
            if filled_cells > 0 {
                draw::tint_row(grid, cy, 0, filled_cells.saturating_sub(1), health_color);
            }
        }

        Ok(())
    }
}

// ─── 5. Cassette Reels ───────────────────────────────────────────────────────

struct CassetteReels;
impl ProgressStyle for CassetteReels {
    fn name(&self) -> &str {
        "cassette-reels"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "Two tape reels spin as tape transfers from supply to take-up reel"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let mid_y = (h / 2) as i32;

        // Reel radii: supply shrinks, take-up grows.
        let max_r = ((h / 2).saturating_sub(1)).max(2) as i32;
        let take_r = (1 + (ctx.eased * (max_r - 1) as f32) as i32)
            .max(1)
            .min(max_r);
        let supply_r = (max_r - take_r + 1).max(1).min(max_r);

        let reel_offset = (w as i32 / 4).max(max_r + 1);
        let left_cx = reel_offset;
        let right_cx = w as i32 - reel_offset;

        // Draw reel circles as outlines (supply = left, take-up = right).
        let draw_reel = |grid: &mut BrailleGrid, cx: i32, cy: i32, r: i32, angle: f32| {
            // Circle outline.
            let steps = (2.0 * PI * r as f32 * 1.5) as usize;
            for s in 0..steps {
                let a = s as f32 / steps as f32 * 2.0 * PI;
                let dx = (a.cos() * r as f32).round() as i32;
                let dy = (a.sin() * r as f32).round() as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
            // Rotating spokes (4 spokes, each 90° apart).
            for spoke in 0..4 {
                let sa = angle + spoke as f32 * PI / 2.0;
                for t in 0..r {
                    let dx = (sa.cos() * t as f32).round() as i32;
                    let dy = (sa.sin() * t as f32).round() as i32;
                    draw::dot_i(grid, cx + dx, cy + dy);
                }
            }
            // Hub dot.
            draw::dot_i(grid, cx, cy);
        };

        let left_angle = ctx.time * 2.5;
        let right_angle = ctx.time * 2.5 + PI; // opposite phase

        draw_reel(grid, left_cx, mid_y, supply_r, left_angle);
        draw_reel(grid, right_cx, mid_y, take_r, right_angle);

        // Tape path: two horizontal lines connecting the reel tangent points.
        let tape_y_top = mid_y - 1;
        let tape_y_bot = mid_y + 1;
        let tape_x0 = (left_cx + supply_r + 1).max(0) as usize;
        let tape_x1 = (right_cx - take_r - 1).max(0) as usize;
        if tape_x0 < tape_x1 {
            draw::hline(grid, tape_x0, tape_x1, tape_y_top.max(0) as usize);
            if tape_y_bot < h as i32 {
                draw::hline(grid, tape_x0, tape_x1, tape_y_bot as usize);
            }
        }

        // Tint reels.
        let (cells_w, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            let left_end = (cells_w / 4).min(cells_w.saturating_sub(1));
            let right_start = (cells_w * 3 / 4).min(cells_w.saturating_sub(1));
            draw::tint_row(grid, cy, 0, left_end, ctx.palette.sample(1.0 - ctx.eased));
            draw::tint_row(
                grid,
                cy,
                right_start,
                cells_w.saturating_sub(1),
                ctx.palette.sample(ctx.eased),
            );
        }

        Ok(())
    }
}

// ─── 6. 8-bit Blocky Pixels ──────────────────────────────────────────────────

struct EightBitBlocks;
impl ProgressStyle for EightBitBlocks {
    fn name(&self) -> &str {
        "8bit-blocks"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "Coarse pixel blocks fill in left-to-right, snapping to a chunky grid"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);

        // Pixel block size in dots — deliberately coarse.
        let px_w = 4usize;
        let px_h = (h / 3).max(2);
        let cols = (w / px_w).max(1);
        let rows = (h / px_h).max(1);
        let total = cols * rows;
        // Fill order: column by column, bottom row first (arcade-screen bottom-up).
        let filled = (ctx.eased * total as f32) as usize;

        for idx in 0..filled.min(total) {
            let col = idx / rows;
            let row_from_bottom = idx % rows;
            let row = rows.saturating_sub(1).saturating_sub(row_from_bottom);
            let x0 = col * px_w;
            let y0 = row * px_h;
            // 1-dot inset on each block to show the grid.
            draw::fill_rect(
                grid,
                x0,
                y0,
                px_w.saturating_sub(1).max(1),
                px_h.saturating_sub(1).max(1),
            );
        }

        // Tint with a slow color cycle (time-based hue shift via palette).
        let (cells_w, cells_h) = grid.dimensions();
        for cy in 0..cells_h {
            for cx in 0..cells_w {
                let t = (cx as f32 / cells_w.max(1) as f32 + ctx.time * 0.1).fract();
                let color = ctx.palette.sample(t);
                // Only tint cells within the filled region.
                let block_col = cx / 2; // 2 cells per px_w/2 block approx
                if block_col < (ctx.eased * (cells_w / 2) as f32) as usize {
                    draw::tint_row(grid, cy, cx, cx, color);
                }
            }
        }

        Ok(())
    }
}

// ─── 7. CRT Scanline ─────────────────────────────────────────────────────────

struct CrtScanline;
impl ProgressStyle for CrtScanline {
    fn name(&self) -> &str {
        "crt-scanline"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "CRT phosphor bar with a rolling dark scanline and interlaced glow"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);

        // Filled region.
        let filled = (ctx.eased * w as f32).round() as usize;
        draw::fill_rect(grid, 0, 0, filled, h);

        // Scanline band rolling downward (wraps).
        let scan_period = 1.5f32;
        let scan_t = (ctx.time % scan_period) / scan_period;
        let scan_y = (scan_t * h as f32) as usize;
        // Erase the scanline row (subtract from braille — just skip those dots).
        // Since braille dots are set, we can punch a "dark band" by not drawing
        // two rows and instead drawing everything except those rows.
        //
        // Re-draw: clear whole grid by only setting non-scanline rows.
        // The fill_rect above already set everything; overlay a blank stripe
        // by drawing over with the outline trick isn't possible with set-only dots.
        // Instead: draw only every other row for the scanline effect.
        // We use a fresh approach: draw only non-suppressed rows.

        // Actually, braille grid is write-only in this API, so we simulate by
        // drawing line by line and skipping scanline rows in the first pass,
        // but fill_rect already ran. Let's draw the scan band as a brighter line
        // by double-drawing adjacent dots — the "dark" is the absence we can't erase.
        //
        // Practical CRT effect: draw even/odd scanlines with varying density.
        // Odd rows get a lighter fill (every other dot).
        // We do this by redrawing the bar without fill_rect, row by row.
        // First, re-render (we can't clear, but the grid starts clean each frame).
        // The grid IS cleared each render call by the caller. So we can just draw
        // row by row:

        // Clear conceptual state and re-draw carefully:
        // (The grid passed in is fresh — we can draw selectively.)
        // We already called fill_rect, but let's work with what we have.
        // Draw a "glow" column at the leading edge.
        if filled > 0 && filled <= w {
            let edge = filled.saturating_sub(1);
            draw::vline(grid, edge, 0, h - 1);
        }

        // Rolling scanline: highlight one horizontal band with extra dots.
        // Since we can only add dots, the "scanline" is a bright band.
        let bright_y = scan_y.min(h.saturating_sub(1));
        if filled > 0 {
            draw::hline(grid, 0, filled.saturating_sub(1), bright_y);
        }

        // Interlace: every other Y row, extend dots 1px into the unfilled zone.
        for y in (0..h).step_by(2) {
            if filled + 1 < w {
                draw::dot(grid, filled, y);
            }
        }

        // Tint: phosphor green across the fill, rolling brightness.
        let (cells_w, cells_h) = grid.dimensions();
        let filled_cells = (ctx.eased * cells_w as f32).round() as usize;
        for cy in 0..cells_h {
            // Vary brightness by row (simulate scanline dimming).
            let row_t = cy as f32 / cells_h.max(1) as f32;
            let scan_dist = ((row_t - scan_t).abs()).min(1.0);
            let t = ctx.eased * (0.7 + 0.3 * scan_dist);
            let color = ctx.palette.sample(t.clamp(0.0, 1.0));
            if filled_cells > 0 {
                draw::tint_row(grid, cy, 0, filled_cells.saturating_sub(1), color);
            }
        }

        Ok(())
    }
}

// ─── 8. Snake ────────────────────────────────────────────────────────────────

struct Snake;
impl ProgressStyle for Snake {
    fn name(&self) -> &str {
        "snake"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "Snake grows longer as progress increases, wiggling its body with time"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let mid_y = h / 2;

        // Snake length in dots proportional to progress.
        let snake_len = (ctx.eased * w as f32) as usize;
        if snake_len == 0 {
            return Ok(());
        }

        // Head leads at the right; tail trails.
        for seg in 0..snake_len {
            // X position: head at snake_len-1, tail at 0.
            let x = seg;
            // Y wiggle: sine wave travelling backwards (toward tail).
            let wave_phase = seg as f32 * 0.4 - ctx.time * 5.0;
            let amp = ((h / 2).saturating_sub(1)) as f32;
            let y_offset = (wave_phase.sin() * amp * 0.5).round() as i32;
            let y = mid_y as i32 + y_offset;

            // Draw a 2-dot wide segment for thickness.
            draw::dot_i(grid, x as i32, y);
            draw::dot_i(grid, x as i32, y + 1);
        }

        // Head: a 3×3 block at the front.
        let head_x = snake_len.saturating_sub(1) as i32;
        let head_wave = ((snake_len as f32) * 0.4 - ctx.time * 5.0).sin();
        let head_y =
            mid_y as i32 + (head_wave * (h / 2).saturating_sub(1) as f32 * 0.5).round() as i32;
        draw::dot_i(grid, head_x + 1, head_y);
        draw::dot_i(grid, head_x + 1, head_y + 1);
        // Eyes: two dots offset from center.
        draw::dot_i(grid, head_x, head_y - 1);

        // Tint the snake body with a gradient head→tail.
        let (cells_w, cells_h) = grid.dimensions();
        let snake_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            for cx in 0..snake_cells.min(cells_w) {
                let t = cx as f32 / snake_cells.max(1) as f32;
                let color = ctx.palette.sample(t);
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }

        Ok(())
    }
}

// ─── 9. Pinball Brick-Breaker ─────────────────────────────────────────────────

struct PinballBricks;
impl ProgressStyle for PinballBricks {
    fn name(&self) -> &str {
        "pinball-bricks"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "A bouncing ball knocks out bricks; bricks cleared = progress"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);

        // Brick grid.
        let brick_cols = (w / 5).max(1);
        let brick_h = (h / 3).max(2).min(h.saturating_sub(4));
        let brick_w = 4usize;
        let bricks_total = brick_cols;
        let bricks_broken = (ctx.eased * bricks_total as f32) as usize;

        // Draw intact bricks at the top.
        for b in bricks_broken..bricks_total {
            let x0 = b * brick_w;
            draw::rect_outline(
                grid,
                x0,
                0,
                brick_w.min(w.saturating_sub(x0)),
                brick_h.max(2),
            );
        }

        // Paddle at the bottom — centered, moves slightly with time.
        let paddle_w = (w / 5).max(4);
        let paddle_y = h.saturating_sub(2);
        let paddle_drift =
            ((ctx.time * 0.8).sin() * (w.saturating_sub(paddle_w)) as f32 * 0.3) as i32;
        let paddle_cx = (w / 2) as i32 + paddle_drift;
        let px0 = (paddle_cx - paddle_w as i32 / 2).max(0) as usize;
        let px1 = (paddle_cx + paddle_w as i32 / 2).min(w as i32 - 1).max(0) as usize;
        draw::hline(grid, px0, px1, paddle_y);
        if paddle_y + 1 < h {
            draw::hline(grid, px0, px1, paddle_y + 1);
        }

        // Ball: parabolic bounce trajectory.
        let ball_period = 1.2f32;
        let ball_t = (ctx.time % ball_period) / ball_period; // 0..1
                                                             // X sweeps left→right.
        let ball_x = (ball_t * w as f32) as i32;
        // Y: parabola — up on first half, down on second.
        let ball_arc = 1.0 - (ball_t * 2.0 - 1.0).powi(2); // peak at t=0.5
        let ball_y = (brick_h as f32 + ball_arc * (h - brick_h) as f32 * 0.9) as i32;

        // Draw ball as a 2×2 block.
        draw::dot_i(grid, ball_x, ball_y);
        draw::dot_i(grid, ball_x + 1, ball_y);
        draw::dot_i(grid, ball_x, ball_y + 1);
        draw::dot_i(grid, ball_x + 1, ball_y + 1);

        // Tint broken region (where bricks were).
        let (cells_w, cells_h) = grid.dimensions();
        let broken_cells = (ctx.eased * cells_w as f32) as usize;
        for cy in 0..cells_h {
            if broken_cells > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    broken_cells.saturating_sub(1),
                    ctx.palette.sample(ctx.eased),
                );
            }
        }

        Ok(())
    }
}

// ─── 10. Combo Power-Up Meter ────────────────────────────────────────────────

struct ComboPower;
impl ProgressStyle for ComboPower {
    fn name(&self) -> &str {
        "combo-power"
    }
    fn theme(&self) -> &str {
        "retro"
    }
    fn describe(&self) -> &str {
        "Combo/power-up meter with a pulsing plasma core and charging arcs"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let mid_y = (h / 2) as i32;
        let mid_x = (w / 2) as i32;

        // Fill: a segmented bar along the bottom half.
        let seg_count = 8usize;
        let seg_w = (w / seg_count).max(2);
        let lit = (ctx.eased * seg_count as f32).round() as usize;
        for s in 0..lit.min(seg_count) {
            let x0 = s * seg_w;
            let bw = seg_w.saturating_sub(1).max(1);
            let by = h / 2 + 1;
            let bh = h.saturating_sub(by + 1).max(1);
            draw::fill_rect(grid, x0, by, bw, bh);
        }

        // Plasma core: expanding ring that pulses with time.
        let pulse = (ctx.time * 4.0).sin() * 0.5 + 0.5; // 0..1
        let core_r = (pulse * ctx.eased * (h / 2) as f32).round() as i32;
        if core_r > 0 {
            let steps = (2.0 * PI * core_r as f32 * 2.0) as usize + 4;
            for s in 0..steps {
                let a = s as f32 / steps as f32 * 2.0 * PI;
                let dx = (a.cos() * core_r as f32).round() as i32;
                let dy = (a.sin() * core_r as f32 * 0.5).round() as i32; // squash vertically
                draw::dot_i(grid, mid_x + dx, mid_y + dy);
            }
        }

        // Charging arcs: lightning-bolt-style zigzag lines converging on the core.
        let arc_count = lit.min(4);
        for arc in 0..arc_count {
            let base_angle = arc as f32 * PI / 2.0 + ctx.time * 3.0;
            let arc_len = (w / 4).max(2) as i32;
            for step in 0..arc_len {
                let frac = step as f32 / arc_len as f32;
                let angle = base_angle + (frac * PI * 2.0).sin() * 0.4; // zigzag
                let r = (arc_len as f32 * (1.0 - frac)).round() as i32;
                let ax = mid_x + (angle.cos() * r as f32).round() as i32;
                let ay = mid_y + (angle.sin() * r as f32 * 0.4).round() as i32;
                draw::dot_i(grid, ax, ay);
            }
        }

        // Center dot.
        draw::dot_i(grid, mid_x, mid_y);

        // Tint: full-spectrum cycle that speeds up as power charges.
        let (cells_w, cells_h) = grid.dimensions();
        let speed = 0.2 + ctx.eased * 1.0;
        for cy in 0..cells_h {
            for cx in 0..cells_w {
                let t = (cx as f32 / cells_w.max(1) as f32 + ctx.time * speed).fract();
                let color = ctx.palette.sample(t);
                // Only tint cells within lit segments + core area.
                let in_bar = cx < (ctx.eased * cells_w as f32) as usize;
                let in_core = (cx as i32 - cells_w as i32 / 2).abs()
                    < (ctx.eased * cells_w as f32 * 0.15) as i32 + 1;
                if in_bar || in_core {
                    draw::tint_row(grid, cy, cx, cx, color);
                }
            }
        }

        Ok(())
    }
}
