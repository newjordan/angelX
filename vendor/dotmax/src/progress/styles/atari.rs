//! ATARI 2600 / early-arcade themed progress bars.
//!
//! Each style evokes a specific game's mechanic: Pong's bouncing rally,
//! Breakout's brick demolition, Asteroids' vector-wireframe explosions,
//! Missile Command's interceptor arcs, Centipede's segmented crawl,
//! Adventure's hero walk, Pitfall's vine pendulum, Combat's tank shells,
//! Kaboom's falling bombs, Yars' Revenge shield erosion, and Lunar Lander's
//! vector descent. Visual form is as distinct as the mechanics: braille dot
//! arcs, block-glyph bricks, line-art polygons, discrete segments, shade
//! walls, and smooth hbar fills all appear exactly once.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Theme tint — warm CRT phosphor orange.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(228, 84, 38);
const TINT_END: Color = Color::rgb(255, 184, 28);

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

/// All styles in the `atari` theme.
///
/// Returns eleven structurally distinct bars, each referencing a different
/// Atari-era game mechanic — from Pong's paddle rally to Lunar Lander's
/// vector descent. No two styles share the same geometry or algorithm.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(Pong)),
        Box::new(Tinted(Breakout)),
        Box::new(Tinted(Asteroids)),
        Box::new(Tinted(MissileCommand)),
        Box::new(Tinted(Centipede)),
        Box::new(Tinted(Adventure)),
        Box::new(Tinted(Pitfall)),
        Box::new(Tinted(Combat)),
        Box::new(Tinted(Kaboom)),
        Box::new(Tinted(YarsRevenge)),
        Box::new(Tinted(LunarLander)),
    ]
}

// ---------------------------------------------------------------------------
// 1. Pong — two paddles rally a ball; rally count fills left→right with score
// ---------------------------------------------------------------------------
struct Pong;
impl ProgressStyle for Pong {
    fn name(&self) -> &str {
        "pong"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Pong: paddles rally a ball across the screen; progress = score"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Score bar — a filled strip whose width encodes progress.
        let score_w = (ctx.eased * w as f32) as usize;
        let bar_y = h.saturating_sub(1);
        draw::hline(grid, 0, score_w.min(w.saturating_sub(1)), bar_y);

        // Centre net: dotted vertical line at mid-width.
        let net_x = w / 2;
        let mut y = 0;
        while y < h {
            draw::dot(grid, net_x, y);
            y += 3;
        }

        // Ball: bounces across width using time, and vertically using sine.
        let ball_period = 2.0_f32;
        let ball_phase = (ctx.time / ball_period).fract();
        // Ping-pong: go 0→1→0
        let ping_pong = if ball_phase < 0.5 {
            ball_phase * 2.0
        } else {
            (1.0 - ball_phase) * 2.0
        };
        let bx = (ping_pong * w.saturating_sub(1) as f32) as usize;
        let by = ((((ctx.time * 1.3).sin() + 1.0) * 0.5) * h.saturating_sub(2) as f32) as usize;
        draw::dot(grid, bx, by);

        // Left paddle: tracks ball vertically on left edge.
        let pad_h = (h / 3).max(1);
        let pad_top_left = by.saturating_sub(pad_h / 2).min(h.saturating_sub(pad_h));
        draw::vline(
            grid,
            0,
            pad_top_left,
            (pad_top_left + pad_h).min(h).saturating_sub(1),
        );

        // Right paddle: fixed mid-height (the "computer" side).
        let pad_top_right = (h / 2).saturating_sub(pad_h / 2);
        draw::vline(
            grid,
            w.saturating_sub(1),
            pad_top_right,
            (pad_top_right + pad_h).min(h).saturating_sub(1),
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Breakout — a ball smashes a brick wall; bricks cleared = eased progress
// ---------------------------------------------------------------------------
struct Breakout;
impl ProgressStyle for Breakout {
    fn name(&self) -> &str {
        "breakout"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Breakout: a ball demolishes brick rows; cleared bricks = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }
        let (dw, dh) = draw::dot_dims(grid);

        // Brick rows occupy the top half of cells.
        let brick_rows = ((ch / 2).max(1)).min(ch.saturating_sub(2).max(1));
        let total_bricks = cw * brick_rows;
        let cleared = (ctx.eased * total_bricks as f32) as usize;

        // Draw remaining bricks using shade glyph (solid █ for intact brick).
        let mut count = 0usize;
        'outer: for row in 0..brick_rows {
            for col in 0..cw {
                if count < cleared {
                    // Brick cleared — leave blank.
                } else {
                    draw::shade(grid, col, row, 4); // '█'
                }
                count += 1;
                if count > total_bricks {
                    break 'outer;
                }
            }
        }

        // Paddle at bottom row, centred with width=cw/4.
        let pad_w = (cw / 4).max(1);
        let pad_x = (cw / 2).saturating_sub(pad_w / 2);
        let pad_y = ch.saturating_sub(1);
        for px in pad_x..(pad_x + pad_w).min(cw) {
            draw::glyph(grid, px, pad_y, '▬');
        }

        // Ball: bounces horizontally; sine drives vertical within lower area.
        let period = 1.8_f32;
        let phase = (ctx.time / period).fract();
        let ping = if phase < 0.5 {
            phase * 2.0
        } else {
            (1.0 - phase) * 2.0
        };
        let bx = (ping * dw.saturating_sub(1) as f32) as usize;
        let by_min = brick_rows * 4;
        let by_range = dh.saturating_sub(by_min + 1).max(1);
        let by = by_min + (((ctx.time * 2.1).sin() + 1.0) * 0.5 * by_range as f32) as usize;
        draw::dot(grid, bx, by.min(dh.saturating_sub(1)));

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Asteroids — wireframe vector polygons shatter as progress rises
// ---------------------------------------------------------------------------
struct Asteroids;
impl ProgressStyle for Asteroids {
    fn name(&self) -> &str {
        "asteroids"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Asteroids: vector-wireframe rocks shatter as progress rises; a ship remains"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Draw a vector polygon (n-gon) centred at (cx, cy) with given radius.
        let draw_ngon = |grid: &mut BrailleGrid, cx: i32, cy: i32, r: i32, n: usize, rot: f32| {
            if n < 2 {
                return;
            }
            for i in 0..n {
                let a0 = rot + 2.0 * PI * i as f32 / n as f32;
                let a1 = rot + 2.0 * PI * (i + 1) as f32 / n as f32;
                let x0 = cx + (r as f32 * a0.cos()) as i32;
                let y0 = cy + (r as f32 * a0.sin()) as i32;
                let x1 = cx + (r as f32 * a1.cos()) as i32;
                let y1 = cy + (r as f32 * a1.sin()) as i32;
                // Bresenham line via dot_i.
                let mut sx = x0;
                let mut sy = y0;
                let dx = (x1 - x0).abs();
                let dy = (y1 - y0).abs();
                let step_x: i32 = if x1 > x0 { 1 } else { -1 };
                let step_y: i32 = if y1 > y0 { 1 } else { -1 };
                let mut err = dx - dy;
                for _ in 0..(dx + dy + 1).min(256) {
                    draw::dot_i(grid, sx, sy);
                    if sx == x1 && sy == y1 {
                        break;
                    }
                    let e2 = 2 * err;
                    if e2 > -dy {
                        err -= dy;
                        sx += step_x;
                    }
                    if e2 < dx {
                        err += dx;
                        sy += step_y;
                    }
                }
            }
        };

        // Asteroid field: 4 asteroids at fixed logical positions.
        // As eased rises, radius shrinks (they shatter to nothing).
        let asteroid_data: [(f32, f32, f32); 4] = [
            (0.15, 0.3, 0.0),
            (0.4, 0.7, 0.5),
            (0.65, 0.25, 1.1),
            (0.85, 0.6, 0.8),
        ];
        let shrink = 1.0 - ctx.eased; // fully gone when eased=1
        for (i, &(fx, fy, rot_off)) in asteroid_data.iter().enumerate() {
            let cx = (fx * w as f32) as i32;
            let cy = (fy * h as f32) as i32;
            let base_r = ((h as f32 * 0.25) as i32).max(2);
            let r = ((base_r as f32 * shrink) as i32).max(0);
            if r < 1 {
                continue;
            }
            let rot = rot_off + ctx.time * (0.4 + i as f32 * 0.15);
            let sides = if i % 2 == 0 { 6 } else { 5 };
            draw_ngon(grid, cx, cy, r, sides, rot);
        }

        // Player ship: small triangle near bottom-centre.
        let ship_cx = (w / 2) as i32;
        let ship_cy = (h * 3 / 4) as i32;
        let ship_r = ((h as f32 * 0.12).max(2.0)) as i32;
        draw_ngon(grid, ship_cx, ship_cy, ship_r, 3, -PI / 2.0);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Missile Command — arcing interceptor parabolas rise; count = eased
// ---------------------------------------------------------------------------
struct MissileCommand;
impl ProgressStyle for MissileCommand {
    fn name(&self) -> &str {
        "missile-command"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Missile Command: interceptor arcs rise to meet incoming threats; arcs = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Ground line at the bottom.
        draw::hline(grid, 0, w.saturating_sub(1), h.saturating_sub(1));

        // Draw a parabolic arc from (x0,base) up to apex (mx, top), back down to (x1,base).
        let draw_arc =
            |grid: &mut BrailleGrid, x0: i32, x1: i32, base: i32, top: i32, fill: f32| {
                let steps = (x1 - x0).abs().max(1).min(w as i32);
                let drawn_steps = (fill * steps as f32) as i32;
                for i in 0..drawn_steps.min(steps) {
                    let t = i as f32 / steps as f32;
                    let x = x0 + ((x1 - x0) as f32 * t) as i32;
                    // Parabola: y = base - (base-top)*4*t*(1-t)
                    let arc = 4.0 * t * (1.0 - t);
                    let y = base - ((base - top) as f32 * arc) as i32;
                    draw::dot_i(grid, x, y);
                }
            };

        // Six interceptor arcs at fixed x-positions, launched at staggered progress thresholds.
        let arc_defs: [(f32, f32, f32); 6] = [
            (0.1, 0.35, 0.0),
            (0.2, 0.55, 0.15),
            (0.3, 0.7, 0.3),
            (0.5, 0.85, 0.45),
            (0.65, 0.9, 0.6),
            (0.8, 0.95, 0.75),
        ];
        let base_y = h.saturating_sub(2) as i32;
        let apex_y = (h / 4) as i32;
        for &(x_frac, x_end_frac, threshold) in &arc_defs {
            if ctx.eased < threshold {
                break;
            }
            let local_fill = ((ctx.eased - threshold) / 0.25).min(1.0);
            let x0 = (x_frac * w as f32) as i32;
            let x1 = (x_end_frac * w as f32) as i32;
            draw_arc(grid, x0, x1, base_y, apex_y, local_fill);
        }

        // Incoming threat dots: a few dots falling from the top, animated by time.
        let threat_xs = [w / 5, w / 2, 3 * w / 4];
        for (i, &tx) in threat_xs.iter().enumerate() {
            let phase = ((ctx.time * 0.7 + i as f32 * 0.4) % 2.0) as f32;
            let ty = (phase * h as f32 * 0.5) as usize;
            draw::dot(grid, tx, ty.min(h.saturating_sub(1)));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Centipede — segmented centipede winds down; segments cleared = progress
// ---------------------------------------------------------------------------
struct Centipede;
impl ProgressStyle for Centipede {
    fn name(&self) -> &str {
        "centipede"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Centipede: a segmented worm winds through the field; cleared segments = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Centipede winds in a boustrophedon (serpentine) pattern across cells.
        // Each cell = one segment. Total segments = cw * ch.
        let total_seg = cw * ch;
        let cleared = (ctx.eased * total_seg as f32) as usize;

        // Draw the remaining (uncleared) segments as shade blocks.
        // Segments are laid out: row 0 left→right, row 1 right→left, etc.
        let head_seg = cleared; // the head is at the cleared boundary.
        for seg in cleared..total_seg {
            let row = seg / cw;
            let col_idx = seg % cw;
            let col = if row % 2 == 0 {
                col_idx
            } else {
                cw.saturating_sub(1).saturating_sub(col_idx)
            };
            let col = col.min(cw.saturating_sub(1));
            let row = row.min(ch.saturating_sub(1));
            // Segments: body = dense shade, head = full block.
            if seg == head_seg && head_seg < total_seg {
                draw::shade(grid, col, row, 4); // head = '█'
            } else {
                draw::shade(grid, col, row, 2); // body = '▒'
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Adventure — dot hero walks a corridor toward a goal; distance = eased
// ---------------------------------------------------------------------------
struct Adventure;
impl ProgressStyle for Adventure {
    fn name(&self) -> &str {
        "adventure"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Adventure: a square hero traverses a corridor toward a goal; distance = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Corridor walls: top and bottom horizontal lines.
        draw::hline(grid, 0, w.saturating_sub(1), 0);
        draw::hline(grid, 0, w.saturating_sub(1), h.saturating_sub(1));

        // Floor trail — a dotted base line to show the hero's path.
        let mid_y = h / 2;
        for x in (0..w).step_by(3) {
            draw::dot(grid, x, mid_y);
        }

        // Goal: a cross / chalice shape at the right end.
        let gx = w.saturating_sub(2) as i32;
        let gy = mid_y as i32;
        draw::dot_i(grid, gx, gy);
        draw::dot_i(grid, gx - 1, gy);
        draw::dot_i(grid, gx + 1, gy);
        draw::dot_i(grid, gx, gy - 1);
        draw::dot_i(grid, gx, gy + 1);

        // Hero: a small filled square (2×2 dots) advancing with progress.
        let hero_x = (ctx.eased * w.saturating_sub(4) as f32) as usize;
        let hero_y = mid_y.saturating_sub(1);
        draw::fill_rect(grid, hero_x, hero_y, 2, 2.min(h.saturating_sub(hero_y)));

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Pitfall — Harry swings a vine (pendulum arc) over pits; screens = eased
// ---------------------------------------------------------------------------
struct Pitfall;
impl ProgressStyle for Pitfall {
    fn name(&self) -> &str {
        "pitfall"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Pitfall: Harry swings a vine pendulum over pits; screens advanced = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Ground line.
        draw::hline(grid, 0, w.saturating_sub(1), h.saturating_sub(1));

        // Tree/anchor at left edge, centred vertically in top third.
        let anchor_x = (w / 5) as i32;
        let anchor_y = (h / 4) as i32;
        draw::vline(grid, anchor_x as usize, 0, anchor_y as usize);

        // Progress bar: ground changes from pits (gaps) to solid path.
        // Left portion (eased) = solid ground, right = pits.
        let solid_end = (ctx.eased * w as f32) as usize;
        // Draw pits as two-dot-wide gaps in the remaining ground area.
        let ground_y = h.saturating_sub(1);
        draw::hline(grid, 0, solid_end.min(w.saturating_sub(1)), ground_y);
        // Right side: pitted — place single dots every 4 to show pit edges.
        let mut px = solid_end;
        while px < w {
            draw::dot(grid, px, ground_y);
            if px + 3 < w {
                px += 4;
            } else {
                break;
            }
        }

        // Vine: line from anchor to Harry.
        let vine_len = (h as f32 * 0.55).max(2.0);
        // Pendulum angle: oscillates with time.
        let angle = (ctx.time * 2.5).sin() * (PI / 4.0);
        let vine_dx = (vine_len * angle.sin()) as i32;
        let vine_dy = (vine_len * angle.cos()) as i32;
        let harry_x = anchor_x + vine_dx;
        let harry_y = anchor_y + vine_dy;
        // Draw vine as a Bresenham line.
        let mut lx = anchor_x;
        let mut ly = anchor_y;
        let dx = (harry_x - anchor_x).abs();
        let dy = (harry_y - anchor_y).abs();
        let step_x: i32 = if harry_x > anchor_x { 1 } else { -1 };
        let step_y: i32 = if harry_y > anchor_y { 1 } else { -1 };
        let mut err = dx - dy;
        for _ in 0..(dx + dy + 1).min(256) {
            draw::dot_i(grid, lx, ly);
            if lx == harry_x && ly == harry_y {
                break;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                lx += step_x;
            }
            if e2 < dx {
                err += dx;
                ly += step_y;
            }
        }
        // Harry: 2×2 block at vine end.
        draw::fill_rect(grid, harry_x.max(0) as usize, harry_y.max(0) as usize, 2, 2);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Combat — two tanks; shells fly; hits fill a score meter
// ---------------------------------------------------------------------------
struct Combat;
impl ProgressStyle for Combat {
    fn name(&self) -> &str {
        "combat"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Combat: two tanks exchange shells; hits scored = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }
        let (dw, dh) = draw::dot_dims(grid);

        // Score bar at the top row using hbar (smooth eighth-block fill).
        draw::hbar(grid, 0, ctx.eased);

        if ch < 2 {
            return Ok(());
        }

        // Tanks: left tank at ~15% width, right tank at ~85%.
        // Each tank is a shade glyph: '▓' for body.
        let left_col = (cw / 8).min(cw.saturating_sub(1));
        let right_col = cw
            .saturating_sub(cw / 8 + 1)
            .max(left_col + 1)
            .min(cw.saturating_sub(1));
        let tank_row = ch / 2;
        draw::shade(grid, left_col, tank_row, 3); // left tank
        draw::shade(grid, right_col, tank_row, 3); // right tank

        // Shells: multiple projectiles travel between tanks.
        // Each shell animates with a phase offset.
        let shell_count = 3usize;
        for i in 0..shell_count {
            let phase = ((ctx.time * 1.2 + i as f32 * 0.33) % 1.0) as f32;
            // Alternate left→right and right→left.
            let (fx, tx) = if i % 2 == 0 {
                (left_col as f32 * 2.0 + 2.0, right_col as f32 * 2.0 - 1.0)
            } else {
                (right_col as f32 * 2.0 - 1.0, left_col as f32 * 2.0 + 2.0)
            };
            let sx = (fx + (tx - fx) * phase) as usize;
            let sy = tank_row * 4 + 1; // mid-cell dot row
            draw::dot(
                grid,
                sx.min(dw.saturating_sub(1)),
                sy.min(dh.saturating_sub(1)),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Kaboom — bombs fall; a bucket catches them; catches = eased
// ---------------------------------------------------------------------------
struct Kaboom;
impl ProgressStyle for Kaboom {
    fn name(&self) -> &str {
        "kaboom"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Kaboom: bombs drop from a mad bomber; bucket catches = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }
        let (dw, dh) = draw::dot_dims(grid);

        // Mad bomber at top-centre.
        let bomber_cx = cw / 2;
        draw::shade(grid, bomber_cx.min(cw.saturating_sub(1)), 0, 3);

        // Bucket at the bottom, following a sine sweep.
        let bucket_col =
            ((((ctx.time * 1.5).sin() + 1.0) * 0.5) * (cw.saturating_sub(2)) as f32) as usize;
        let bucket_row = ch.saturating_sub(1);
        draw::glyph(grid, bucket_col.min(cw.saturating_sub(1)), bucket_row, '▂');

        // Falling bombs: staggered phases.
        let bomb_count = 4usize;
        for i in 0..bomb_count {
            let phase = ((ctx.time * 0.8 + i as f32 * 0.25) % 1.0) as f32;
            // Bombs fan out from bomber position.
            let bx =
                ((bomber_cx as f32 + (i as f32 - bomb_count as f32 / 2.0) * 2.0) * 2.0) as usize;
            let by = (phase * dh as f32) as usize;
            draw::dot(
                grid,
                bx.min(dw.saturating_sub(1)),
                by.min(dh.saturating_sub(1)),
            );
        }

        // Progress: a catch-score strip on the right edge (vertical fill).
        let score_h = (ctx.eased * dh as f32) as usize;
        for sy in (dh.saturating_sub(score_h))..dh {
            draw::dot(grid, dw.saturating_sub(1), sy);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Yars' Revenge — a shield wall erodes cell by cell from left to right
// ---------------------------------------------------------------------------
struct YarsRevenge;
impl ProgressStyle for YarsRevenge {
    fn name(&self) -> &str {
        "yars-revenge"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Yars' Revenge: the Qotile shield wall erodes cell by cell; erosion = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }
        let (dw, _dh) = draw::dot_dims(grid);

        // The shield wall: a column of shade blocks occupying the left 1/3 of cells.
        let wall_cols = ((cw / 3).max(1)).min(cw);
        let total_blocks = wall_cols * ch;
        let eroded = (ctx.eased * total_blocks as f32) as usize;

        // Blocks erode column by column left-to-right (like the original game).
        let eroded_cols = eroded / ch.max(1);
        let eroded_partial = eroded % ch.max(1);

        for col in 0..wall_cols {
            for row in 0..ch {
                let block_idx = col * ch + row;
                if block_idx < eroded {
                    // Eroded: leave blank.
                } else if col == eroded_cols && row < eroded_partial {
                    // Partially eroded column.
                } else {
                    // Intact: dense shade block.
                    draw::shade(
                        grid,
                        col.min(cw.saturating_sub(1)),
                        row.min(ch.saturating_sub(1)),
                        3,
                    );
                }
            }
        }

        // Yar (the fly): a dot hero moving horizontally, animated by time.
        let yar_x = (((ctx.time * 3.0).sin() + 1.0) * 0.5 * (wall_cols + 2) as f32) as usize;
        let yar_y = (ch / 2) * 4; // mid-cell in dot space
        draw::dot(grid, yar_x.min(dw.saturating_sub(1)), yar_y);
        draw::dot(
            grid,
            yar_x.min(dw.saturating_sub(1)),
            yar_y.saturating_sub(1),
        );

        // Qotile (the enemy): a vertical stripe on the right edge.
        let q_col = cw.saturating_sub(1);
        let q_row = (((ctx.time * 0.5).sin() + 1.0) * 0.5 * ch.saturating_sub(1) as f32) as usize;
        draw::shade(grid, q_col, q_row.min(ch.saturating_sub(1)), 4);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Lunar Lander — vector lander descends; fuel/altitude gauge on the side
// ---------------------------------------------------------------------------
struct LunarLander;
impl ProgressStyle for LunarLander {
    fn name(&self) -> &str {
        "lunar-lander"
    }
    fn theme(&self) -> &str {
        "atari"
    }
    fn describe(&self) -> &str {
        "Lunar Lander: a vector wireframe lander descends; altitude/fuel = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Altitude gauge: vertical bar on the far right (2 dots wide).
        let gauge_x = w.saturating_sub(2);
        let gauge_h = h;
        let fuel_h = (ctx.eased * gauge_h as f32) as usize;
        // Empty gauge outline.
        draw::vline(grid, gauge_x, 0, gauge_h.saturating_sub(1));
        // Fuel fill from bottom.
        for gy in (gauge_h.saturating_sub(fuel_h))..gauge_h {
            draw::dot(grid, gauge_x + 1, gy);
        }

        // Lunar surface: irregular terrain at the bottom.
        let surface_y = h.saturating_sub(2);
        draw::hline(grid, 0, gauge_x.saturating_sub(1), surface_y);
        // Jagged peaks.
        let peak_xs = [w / 6, w / 3, w / 2, 2 * w / 3];
        for &px in &peak_xs {
            if px < gauge_x {
                draw::vline(grid, px, surface_y.saturating_sub(2), surface_y);
            }
        }

        // Lander: descends from top to surface as eased rises.
        // Wireframe: body (rectangle) + legs (two diagonal lines) + thruster dot.
        let lander_col_centre = (w / 2) as i32;
        let descent_range = surface_y.saturating_sub(6);
        let lander_y = (ctx.eased * descent_range as f32) as i32;

        let bw: i32 = (w as i32 / 8).max(2).min(5);
        let bh: i32 = 2.max((h as i32 / 8).min(3));

        // Body rectangle.
        let bx0 = lander_col_centre - bw / 2;
        let bx1 = lander_col_centre + bw / 2;
        let by0 = lander_y;
        let by1 = lander_y + bh;
        // Top and bottom of body.
        for x in bx0..=bx1 {
            draw::dot_i(grid, x, by0);
            draw::dot_i(grid, x, by1);
        }
        // Sides.
        for y in by0..=by1 {
            draw::dot_i(grid, bx0, y);
            draw::dot_i(grid, bx1, y);
        }
        // Landing legs: diagonal lines down from body corners.
        let leg_len: i32 = bh.max(2);
        draw::dot_i(grid, bx0 - leg_len, by1 + leg_len);
        draw::dot_i(grid, bx0 - leg_len + 1, by1 + leg_len - 1);
        draw::dot_i(grid, bx0 - leg_len + 2, by1 + leg_len - 2);
        draw::dot_i(grid, bx1 + leg_len, by1 + leg_len);
        draw::dot_i(grid, bx1 + leg_len - 1, by1 + leg_len - 1);
        draw::dot_i(grid, bx1 + leg_len - 2, by1 + leg_len - 2);

        // Thruster exhaust: a pulsing dot below the body (only when descending).
        if ctx.eased < 0.95 {
            let pulse = ((ctx.time * 8.0).sin() > 0.0) as i32;
            draw::dot_i(grid, lander_col_centre, by1 + 1 + pulse);
        }

        Ok(())
    }
}
