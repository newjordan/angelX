//! Game Boy / handheld LCD progress bars.
//!
//! Eleven styles evoking the washed-out dot-matrix LCD panels of classic
//! handheld gaming: Pokémon HP, Nokia Snake, Tetris GB, Dr. Mario, Kirby,
//! Tamagotchi, Game & Watch, Link's Awakening hearts, Mole Whack, Wario
//! treasure, and LCD Pinball.
//!
//! Every style leans into the `░▒▓` shade ramp to simulate the ghosting and
//! segment-dimming of a genuine reflective-LCD panel. Animation is a pure
//! function of `ctx.time`; fill extent is `ctx.eased`. All writes go through
//! `draw::` helpers — out-of-bounds coordinates are silently discarded.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

/// All styles in the `gameboy` theme.
///
/// Returns one boxed implementor per style, in display order. Each struct is a
/// zero-size private unit type — no heap allocation beyond the `Box` itself.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(PokemonHp),
        Box::new(NokiaSnake),
        Box::new(TetrisGb),
        Box::new(DrMario),
        Box::new(KirbyInhale),
        Box::new(Tamagotchi),
        Box::new(GameWatch),
        Box::new(HeartContainers),
        Box::new(MoleWhack),
        Box::new(WarioTreasure),
        Box::new(LcdPinball),
    ]
}

// ─── 1. Pokémon HP Bar ───────────────────────────────────────────────────────

/// HP bar drains/fills with a Poké Ball that wobbles mid-capture, settling at 100%.
struct PokemonHp;
impl ProgressStyle for PokemonHp {
    fn name(&self) -> &str {
        "pokemon-hp"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Pokémon-style HP bar with a Poké Ball that wobbles during capture and settles at full"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (_w, _h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();

        // ── LCD panel background (shade 1 = ░) ──────────────────────────────
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── HP label "HP" in the leftmost 2 cells (shade 4 = █) ─────────────
        // Represented as thin vertical bars to mimic dot-matrix lettering.
        if cw >= 2 {
            draw::shade(grid, 0, 0, 4);
            if ch > 1 {
                draw::shade(grid, 0, 1, 4);
            }
        }

        // ── HP track — the green segmented bar ──────────────────────────────
        let bar_start_cell = if cw > 3 { 2 } else { 0 };
        let bar_w = cw.saturating_sub(bar_start_cell + 1).max(1);
        // Background of HP track (shade 2 = ▒)
        for cx in bar_start_cell..bar_start_cell + bar_w {
            for cy in 0..ch {
                draw::shade(grid, cx, cy, 2);
            }
        }
        // Filled HP (shade 4)
        let filled = (ctx.eased * bar_w as f32).round() as usize;
        for cx in bar_start_cell..bar_start_cell + filled.min(bar_w) {
            for cy in 0..ch {
                draw::shade(grid, cx, cy, 4);
            }
        }

        // ── Poké Ball in the last cell ───────────────────────────────────────
        if cw > 0 {
            let ball_cx = cw.saturating_sub(1);
            // Wobble angle — dampens to zero as progress → 1.0
            let settle = 1.0 - ctx.eased;
            let wobble = (ctx.time * 8.0).sin() * settle * 0.5;
            // Express wobble as a shade level: rocking between ▒ and █
            let shade_lvl = if wobble.abs() > 0.25 { 3usize } else { 4 };
            for cy in 0..ch {
                draw::shade(grid, ball_cx, cy, shade_lvl);
            }
            // Central equator line on the ball — always shade 2
            if ch >= 2 {
                draw::shade(grid, ball_cx, ch / 2, 2);
            }
        }

        // ── Palette tint — green HP, red when low ───────────────────────────
        let health_t = ctx.eased; // 0=red, 1=green via palette
        let filled_cells = bar_start_cell + filled.min(bar_w);
        for cy in 0..ch {
            if filled_cells > bar_start_cell {
                draw::tint_row(
                    grid,
                    cy,
                    bar_start_cell,
                    filled_cells.saturating_sub(1),
                    ctx.palette.sample(health_t),
                );
            }
        }
        Ok(())
    }
}

// ─── 2. Nokia Snake ──────────────────────────────────────────────────────────

/// Snake grows segment by segment as progress rises, eating pellets on a dim LCD grid.
struct NokiaSnake;
impl ProgressStyle for NokiaSnake {
    fn name(&self) -> &str {
        "nokia-snake"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Nokia-style snake grows from left; segments = progress, pellets sparkle with time"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();

        // ── Snake body in dot space ──────────────────────────────────────────
        // Snake travels in a boustrophedon (row-alternating) path.
        let snake_dots = (ctx.eased * (w * h) as f32) as usize;
        let mid_y = h / 2;

        for seg in 0..snake_dots.min(w * h) {
            // Boustrophedon: even rows go right, odd rows go left.
            let row = seg / w;
            let col_in_row = seg % w;
            let x = if row % 2 == 0 {
                col_in_row
            } else {
                w.saturating_sub(1 + col_in_row)
            };
            let y = row;
            draw::dot(grid, x, y.min(h.saturating_sub(1)));
        }

        // ── Pellets: 2-dot squares at regular intervals, dim if eaten ────────
        let pellet_spacing = (w / 5).max(2);
        let pellet_count = w / pellet_spacing;
        let eaten = (ctx.eased * pellet_count as f32) as usize;
        for p in 0..pellet_count {
            let px = p * pellet_spacing + pellet_spacing / 2;
            if p >= eaten {
                // Uneaten pellet — blink with time
                let blink = ((ctx.time * 3.0 + p as f32 * 0.7).sin() > 0.0) as usize;
                let shade = 2 + blink; // ▒ or ▓
                draw::dot(grid, px.min(w.saturating_sub(1)), mid_y);
                draw::dot(grid, px.saturating_sub(1), mid_y);
                let _ = shade; // shade already expressed via dot presence/absence
            }
            // Eaten pellets leave no trace (already set by snake body or absent)
        }

        // ── Head: 3×3 block at the snake tip ─────────────────────────────────
        if snake_dots > 0 {
            let head_seg = snake_dots.saturating_sub(1);
            let row = head_seg / w;
            let col_in_row = head_seg % w;
            let hx = if row % 2 == 0 {
                col_in_row
            } else {
                w.saturating_sub(1 + col_in_row)
            };
            let hy = row.min(h.saturating_sub(1));
            for dy in 0..2i32 {
                for dx in 0..2i32 {
                    draw::dot_i(grid, hx as i32 + dx, hy as i32 + dy);
                }
            }
        }

        // ── Dim LCD background: only cells the snake left blank ─────────────
        // (a shade glyph overrides braille dots, so it must come last)
        for cy in 0..ch {
            for cx in 0..cw {
                if grid.get_char(cx, cy) == '\u{2800}' {
                    draw::shade(grid, cx, cy, 1);
                }
            }
        }

        // ── Palette tint on filled region ────────────────────────────────────
        let filled_cells = (ctx.eased * cw as f32) as usize;
        for cy in 0..ch {
            if filled_cells > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    filled_cells.saturating_sub(1),
                    ctx.palette.sample(ctx.eased),
                );
            }
        }
        Ok(())
    }
}

// ─── 3. Tetris GB ────────────────────────────────────────────────────────────

/// Shaded blocks stack in a well from the bottom; height = progress.
struct TetrisGb;
impl ProgressStyle for TetrisGb {
    fn name(&self) -> &str {
        "tetris-gb"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Shaded Tetris blocks stack in a GB-style well; block density = progress"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();

        // ── Well background (shade 1) ─────────────────────────────────────────
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── Stacked blocks — each cell gets a shade based on depth ────────────
        // Blocks fill from the bottom. Stack height = eased fraction of rows.
        let stack_rows = (ctx.eased * ch as f32).round() as usize;
        let base_row = ch.saturating_sub(stack_rows);

        // Tetromino columns vary phase slightly for the ragged skyline.
        let col_w = 2usize.max(1); // 2 cells per "block column"
        let block_cols = (cw / col_w).max(1);

        for bc in 0..block_cols {
            // Each block column has a small phase offset in its height.
            let phase = (bc as f32 / block_cols as f32) * PI;
            let col_extra = (phase.sin() * 1.0).round() as i32;
            let col_base = (base_row as i32 - col_extra).max(0) as usize;

            for row in col_base..ch {
                // Shade deepens toward the bottom — deeper = denser.
                let depth = ch.saturating_sub(row);
                let shade_lvl = match depth {
                    d if d >= stack_rows => 4,
                    d if d * 4 >= stack_rows * 3 => 3,
                    d if d * 2 >= stack_rows => 2,
                    _ => 1,
                };
                for col_off in 0..col_w {
                    let cx = bc * col_w + col_off;
                    if cx < cw {
                        draw::shade(grid, cx, row, shade_lvl);
                    }
                }
            }

            // Block dividers: a shade-1 gap row every 2 rows from the bottom.
            // (Makes the stack look like individual pieces.)
            let mut div_row = ch.saturating_sub(2);
            while div_row > col_base {
                for col_off in 0..col_w {
                    let cx = bc * col_w + col_off;
                    if cx < cw && div_row < ch {
                        draw::shade(grid, cx, div_row, 1);
                    }
                }
                div_row = div_row.saturating_sub(2);
            }
        }

        // ── Currently falling piece — a single 2×2 block that drops with time ─
        let fall_period = 0.8f32;
        let fall_t = (ctx.time % fall_period) / fall_period;
        let fall_row = (fall_t * base_row.max(1) as f32) as usize;
        let piece_col = ((ctx.time * 0.3) as usize % block_cols.max(1)) * col_w;
        for col_off in 0..col_w {
            let cx = piece_col + col_off;
            if cx < cw && fall_row < base_row {
                draw::shade(grid, cx, fall_row, 4);
            }
        }

        // ── Palette tint on filled stack ─────────────────────────────────────
        for cy in base_row..ch {
            let t = (cy.saturating_sub(base_row)) as f32 / stack_rows.max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), color);
        }
        Ok(())
    }
}

// ─── 4. Dr. Mario ────────────────────────────────────────────────────────────

/// Pills appear and stack; each cleared row is progress.
struct DrMario;
impl ProgressStyle for DrMario {
    fn name(&self) -> &str {
        "dr-mario"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Dr. Mario pills stack in a bottle; rows cleared = progress; cleared rows flash"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();

        // ── Bottle background (shade 1) ───────────────────────────────────────
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── Bottle walls (shade 4 left + right column) ────────────────────────
        for cy in 0..ch {
            draw::shade(grid, 0, cy, 4);
            if cw > 1 {
                draw::shade(grid, cw.saturating_sub(1), cy, 4);
            }
        }

        // Usable interior width.
        let inner_cw = cw.saturating_sub(2).max(1);
        let inner_start = 1usize;

        // ── Pills fill from the bottom, alternating two-cell pill pairs ────────
        let pill_cells = (ctx.eased * (inner_cw * ch) as f32) as usize;
        let pill_rows_filled = (pill_cells / inner_cw.max(1)).min(ch);

        let base_row = ch.saturating_sub(pill_rows_filled);

        for row in base_row..ch {
            let is_cleared = row % 3 == 0; // every 3rd row is a "cleared" row
                                           // Cleared rows flash with time.
            let flash = is_cleared && ((ctx.time * 6.0) as usize % 2 == 0);
            let shade_lvl: usize = if flash { 1 } else { 3 };

            // Pills: alternating pairs of shade 3 / shade 4 within the row.
            let mut cx = inner_start;
            let mut pill_idx = 0usize;
            while cx < inner_start + inner_cw {
                let lvl = if pill_idx % 2 == 0 {
                    shade_lvl
                } else {
                    shade_lvl.saturating_sub(1).max(2)
                };
                draw::shade(grid, cx, row, lvl);
                if cx + 1 < inner_start + inner_cw {
                    draw::shade(grid, cx + 1, row, lvl.min(4));
                }
                cx += 2;
                pill_idx += 1;
            }
        }

        // ── Falling pill at the top ────────────────────────────────────────────
        let drop_t = (ctx.time * 1.5).fract();
        let drop_row = (drop_t * base_row.max(1) as f32) as usize;
        let drop_col = inner_start + ((ctx.time * 0.5) as usize % inner_cw.max(1));
        if drop_row < base_row && drop_col < inner_start + inner_cw {
            draw::shade(grid, drop_col, drop_row, 4);
            if drop_col + 1 < inner_start + inner_cw {
                draw::shade(grid, drop_col + 1, drop_row, 3);
            }
        }

        // ── Palette tint on pill rows ─────────────────────────────────────────
        for row in base_row..ch {
            let t = (row.saturating_sub(base_row)) as f32 / pill_rows_filled.max(1) as f32;
            let color = ctx.palette.sample(t);
            draw::tint_row(
                grid,
                row,
                inner_start,
                inner_start + inner_cw.saturating_sub(1),
                color,
            );
        }
        Ok(())
    }
}

// ─── 5. Kirby Inhale ─────────────────────────────────────────────────────────

/// Kirby "inhale" meter — cheeks puff with eased, stars inhaled with time.
struct KirbyInhale;
impl ProgressStyle for KirbyInhale {
    fn name(&self) -> &str {
        "kirby-inhale"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Kirby puffs up as the meter fills; stars streak toward his mouth with time"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();

        // ── Dim LCD background ────────────────────────────────────────────────
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── Inhale power bar across the bottom rows ───────────────────────────
        // A meter of shade glyphs showing current inhale strength.
        let meter_row = ch.saturating_sub(1);
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..cw {
            let lvl = if cx < filled_cells { 4 } else { 2 };
            draw::shade(grid, cx, meter_row, lvl);
        }

        // ── Kirby body in dot space — circle that expands with eased ──────────
        // Kirby is in the right third of the bar.
        let kirby_cx = (w * 3 / 4) as i32;
        let kirby_cy = (h / 2) as i32;
        // Base radius + puff proportional to eased.
        let min_r = (h / 4).max(2) as i32;
        let max_r = (h / 2).max(3) as i32;
        let r = min_r + ((ctx.eased * (max_r - min_r) as f32).round() as i32);

        // Draw Kirby's body circle.
        let steps = (2.0 * PI * r as f32 * 1.5) as usize + 8;
        for s in 0..steps {
            let a = s as f32 / steps as f32 * 2.0 * PI;
            let dx = (a.cos() * r as f32).round() as i32;
            let dy = (a.sin() * r as f32 * 0.7).round() as i32; // slightly squished
            draw::dot_i(grid, kirby_cx + dx, kirby_cy + dy);
        }
        // Filled interior dots.
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx * 100 + dy * dy * 100 * 100 / 49 <= r * r * 100 {
                    draw::dot_i(grid, kirby_cx + dx, kirby_cy + dy);
                }
            }
        }

        // Eyes — two dots above center.
        let eye_y = kirby_cy - r / 3;
        draw::dot_i(grid, kirby_cx - r / 3, eye_y);
        draw::dot_i(grid, kirby_cx + r / 3, eye_y);

        // Mouth — a small open "O" when inhaling (progress < 1.0).
        if ctx.eased < 0.95 {
            let mouth_y = kirby_cy + r / 3;
            draw::dot_i(grid, kirby_cx - 1, mouth_y);
            draw::dot_i(grid, kirby_cx + 1, mouth_y);
            draw::dot_i(grid, kirby_cx, mouth_y + 1);
        }

        // ── Inhaled stars streaking toward Kirby ──────────────────────────────
        let star_count = 5usize;
        for i in 0..star_count {
            let phase = i as f32 / star_count as f32;
            let t = (ctx.time * 1.5 + phase).fract(); // 0→1 travel
                                                      // Stars come from the left.
            let sx = ((1.0 - t) * kirby_cx as f32) as i32;
            let sy = kirby_cy + ((phase * 2.0 - 1.0) * (h / 4) as f32) as i32;
            draw::dot_i(grid, sx, sy);
            draw::dot_i(grid, sx + 1, sy);
        }

        // ── Palette tint — bluer when empty, fuller when puffed ──────────────
        let body_cell_x = (kirby_cx as usize / 2).saturating_sub(r as usize / 2);
        let body_cell_w = (r as usize).min(cw.saturating_sub(body_cell_x));
        for cy in 0..ch.saturating_sub(1) {
            if body_cell_w > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    body_cell_x,
                    body_cell_x + body_cell_w.saturating_sub(1),
                    ctx.palette.sample(ctx.eased),
                );
            }
            // Star trail region tinted at lower intensity.
            if body_cell_x > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    body_cell_x.saturating_sub(1),
                    ctx.palette.sample(ctx.eased * 0.4),
                );
            }
        }
        Ok(())
    }
}

// ─── 6. Tamagotchi ───────────────────────────────────────────────────────────

/// Tamagotchi pet hunger/growth meter; pet animates (toddles) with time.
struct Tamagotchi;
impl ProgressStyle for Tamagotchi {
    fn name(&self) -> &str {
        "tamagotchi"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Tamagotchi hunger/growth bar; the pixel-art pet toddles and blinks with time"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, _h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();

        // ── LCD background (shade 1) ───────────────────────────────────────────
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── Hunger bar: shade segments across the top row ─────────────────────
        let bar_cells = cw.saturating_sub(4).max(1); // leave right side for pet
        let bar_filled = (ctx.eased * bar_cells as f32).round() as usize;
        for cx in 0..bar_cells {
            let lvl = if cx < bar_filled { 4usize } else { 2 };
            draw::shade(grid, cx, 0, lvl);
        }
        // Meter ticks: shade-1 every 2 cells.
        let mut tick = 2usize;
        while tick < bar_cells {
            draw::shade(grid, tick, 0, 1);
            tick += 3;
        }

        // ── Pet sprite in dot space (right 8 dots of the bar) ─────────────────
        // Pet is a simple 5×7 pixel-art egg-blob.
        let pet_x = (w.saturating_sub(10)) as i32;
        let pet_y = 1i32;
        // Walk animation: bobble up/down with time.
        let walk_frame = ((ctx.time * 4.0) as usize) % 2;
        let bob = if walk_frame == 0 { 0i32 } else { 1i32 };

        // Body: oval.
        let bx = pet_x;
        let by = pet_y + bob;
        // 5×5 body (filled dots).
        for dy in 0..5i32 {
            for dx in 0..5i32 {
                // Rounded corners: skip (0,0),(4,0),(0,4),(4,4)
                if (dx == 0 || dx == 4) && (dy == 0 || dy == 4) {
                    continue;
                }
                draw::dot_i(grid, bx + dx, by + dy);
            }
        }
        // Eyes: two gaps in the body at row 1.
        // (draw inverse: no-op — the eye is absence of dots. Represent with shade.)
        // Feet: two dots at the bottom.
        draw::dot_i(grid, bx + 1, by + 5);
        draw::dot_i(grid, bx + 3, by + 5);

        // Blink: at blink moment erase the eye-row dots.
        let blink = (ctx.time * 2.5).fract() > 0.85;
        if !blink {
            // Eyes open: leave body dots as-is, mark eye positions with a shade glyph.
            // (We can't erase dots, so we use shade glyphs in cell space.)
            let eye_cell_x = (bx as usize) / 2;
            let eye_cell_y = (by as usize) / 4;
            if eye_cell_x + 1 < cw && eye_cell_y < ch {
                // Use shade-2 to simulate lighter "white of eye" in those cells.
                draw::shade(grid, eye_cell_x, eye_cell_y, 2);
                if eye_cell_x + 2 < cw {
                    draw::shade(grid, eye_cell_x + 2, eye_cell_y, 2);
                }
            }
        }

        // Growth: pet cell shaded by hunger/growth level.
        let pet_shade = match ctx.eased {
            e if e >= 0.8 => 4,
            e if e >= 0.5 => 3,
            e if e >= 0.2 => 2,
            _ => 1,
        };
        let pet_cell_x = (pet_x.max(0) as usize) / 2;
        let pet_cell_w = 3usize;
        for cy in 1..ch {
            for px in 0..pet_cell_w {
                let cx = pet_cell_x + px;
                if cx < cw {
                    draw::shade(grid, cx, cy, pet_shade.min(4));
                }
            }
        }

        // ── Palette tint ──────────────────────────────────────────────────────
        for cy in 0..ch {
            if bar_filled > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    bar_filled.min(cw).saturating_sub(1),
                    ctx.palette.sample(ctx.eased),
                );
            }
        }
        Ok(())
    }
}

// ─── 7. Game & Watch ─────────────────────────────────────────────────────────

/// Flat LCD segment figure juggles; ball position cycles through discrete segment states.
struct GameWatch;
impl ProgressStyle for GameWatch {
    fn name(&self) -> &str {
        "game-watch"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Game & Watch segment LCD: a juggler tosses balls; active count = progress"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();

        // ── LCD panel — ALL cells start at shade 1 (the "ghosted" segment look) ─
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── Juggler figure: fixed LCD segments in a 3-column region ──────────
        // Figures are discrete: head, body, hands, feet.
        // Position figure in the left quarter.
        let fig_col = cw / 4;

        // Head (shade 4 at top).
        if fig_col < cw {
            draw::shade(grid, fig_col, 0, 4);
        }
        // Body (shade 3 for 1..ch-2).
        for cy in 1..ch.saturating_sub(1) {
            if fig_col < cw {
                draw::shade(grid, fig_col, cy, 3);
            }
        }
        // Arms: the frame toggles which arm is raised.
        let arm_frame = ((ctx.time * 3.0) as usize) % 2;
        if arm_frame == 0 {
            // Left arm up.
            if fig_col > 0 {
                draw::shade(grid, fig_col.saturating_sub(1), 0, 4);
            }
            if fig_col + 1 < cw && ch > 1 {
                draw::shade(grid, fig_col + 1, 1, 3);
            }
        } else {
            // Right arm up.
            if fig_col + 1 < cw {
                draw::shade(grid, fig_col + 1, 0, 4);
            }
            if fig_col > 0 && ch > 1 {
                draw::shade(grid, fig_col.saturating_sub(1), 1, 3);
            }
        }
        // Feet.
        if ch > 1 {
            if fig_col > 0 {
                draw::shade(grid, fig_col.saturating_sub(1), ch.saturating_sub(1), 3);
            }
            if fig_col + 1 < cw {
                draw::shade(grid, fig_col + 1, ch.saturating_sub(1), 3);
            }
        }

        // ── Balls: discrete arc positions cycling with time ───────────────────
        // Number of active balls = progress * max_balls.
        let max_balls = ((cw.saturating_sub(fig_col + 2)) / 3).max(1);
        let active_balls = (ctx.eased * max_balls as f32).round() as usize;

        // Arc has 4 discrete positions (game & watch style).
        let arc_positions: [(i32, i32); 4] = [
            (0, 0),  // hand level
            (1, -1), // going up
            (2, -2), // apex
            (3, -1), // coming down
        ];

        for b in 0..active_balls.min(max_balls) {
            let phase_offset = b as f32 / active_balls.max(1) as f32;
            let arc_idx = ((ctx.time * 2.0 + phase_offset * 4.0) as usize) % 4;
            let (arc_dx, arc_dy) = arc_positions[arc_idx];
            let ball_col =
                (fig_col as i32 + fig_col as i32 / 2 + arc_dx + b as i32 * 2).max(0) as usize;
            let ball_row = (ch as i32 / 2 + arc_dy).clamp(0, ch as i32 - 1) as usize;
            if ball_col < cw {
                draw::shade(grid, ball_col, ball_row, 4);
            }
        }

        // ── Score track: shade segments across the right portion ──────────────
        let score_start = cw * 3 / 4;
        let score_w = cw.saturating_sub(score_start);
        for cx in score_start..score_start + score_w {
            if cx < cw {
                let seg_active =
                    cx.saturating_sub(score_start) < active_balls * score_w / max_balls.max(1);
                draw::shade(grid, cx, ch / 2, if seg_active { 4 } else { 2 });
            }
        }

        // ── Palette tint on active regions ────────────────────────────────────
        for cy in 0..ch {
            let active_w = (ctx.eased * cw as f32) as usize;
            if active_w > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    active_w.saturating_sub(1),
                    ctx.palette.sample(ctx.eased),
                );
            }
        }
        Ok(())
    }
}

// ─── 8. Link's Awakening Hearts ──────────────────────────────────────────────

/// Heart containers fill one at a time, like Link's Awakening health display.
struct HeartContainers;
impl ProgressStyle for HeartContainers {
    fn name(&self) -> &str {
        "hearts"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Link's Awakening heart containers: each fills one at a time; last pulses"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();

        // ── LCD background ────────────────────────────────────────────────────
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── Hearts: each heart = 2 cells wide, displayed in the center rows ───
        let heart_w = 2usize; // cells per heart
        let max_hearts = (cw / heart_w).max(1);
        let full_hearts = (ctx.eased * max_hearts as f32) as usize;
        let partial_frac = (ctx.eased * max_hearts as f32).fract();

        // Pulse animation on the leading (currently-filling) heart.
        let pulse = (ctx.time * 6.0).sin() * 0.5 + 0.5; // 0..1

        for h_idx in 0..max_hearts {
            let cx_start = h_idx * heart_w;
            let heart_shade = if h_idx < full_hearts {
                // Fully filled heart.
                4usize
            } else if h_idx == full_hearts {
                // Partially filled — shade by partial fraction + pulse.
                let lvl = (partial_frac * 3.0 + pulse * 0.5) as usize;
                (lvl + 1).min(4)
            } else {
                // Empty heart container.
                2
            };

            // Each heart: top row is 2 bumps (shade 4 at edges, 3 in middle),
            // bottom row is a V-shape (shade 4).
            // We use cell rows from top to bottom.
            let top_row = ch / 4;
            let bot_row = ch / 2;

            // Top bumps.
            for cy in top_row..top_row + 1 {
                if cy < ch {
                    for off in 0..heart_w {
                        let cx = cx_start + off;
                        if cx < cw {
                            draw::shade(grid, cx, cy, heart_shade);
                        }
                    }
                }
            }
            // Bottom V.
            for cy in bot_row..bot_row + 1 {
                if cy < ch {
                    // Middle cell shade (V tip).
                    let mid_cx = cx_start + heart_w / 2;
                    if mid_cx < cw {
                        draw::shade(grid, mid_cx, cy, heart_shade);
                    }
                    // Flanks slightly dimmer.
                    if cx_start < cw {
                        draw::shade(grid, cx_start, cy, heart_shade.saturating_sub(1).max(1));
                    }
                    if cx_start + heart_w.saturating_sub(1) < cw {
                        draw::shade(
                            grid,
                            cx_start + heart_w.saturating_sub(1),
                            cy,
                            heart_shade.saturating_sub(1).max(1),
                        );
                    }
                }
            }
            // Body fill between rows.
            if bot_row > top_row + 1 {
                for cy in top_row + 1..bot_row {
                    for off in 0..heart_w {
                        let cx = cx_start + off;
                        if cx < cw && cy < ch {
                            draw::shade(grid, cx, cy, heart_shade.saturating_sub(1).max(1));
                        }
                    }
                }
            }
        }

        // ── Palette tint: filled hearts ───────────────────────────────────────
        let filled_cells = (full_hearts * heart_w).min(cw);
        for cy in 0..ch {
            if filled_cells > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    filled_cells.saturating_sub(1),
                    ctx.palette.sample(ctx.eased),
                );
            }
        }
        Ok(())
    }
}

// ─── 9. Mole Whack ───────────────────────────────────────────────────────────

/// Moles pop up in parabolic arcs; bonked moles = progress.
struct MoleWhack;
impl ProgressStyle for MoleWhack {
    fn name(&self) -> &str {
        "mole-whack"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Whack-a-Mole: moles pop from holes in parabolic arcs; bonked count = progress"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();

        // ── Ground line ───────────────────────────────────────────────────────
        let ground_y = h.saturating_sub(3);
        draw::hline(grid, 0, w.saturating_sub(1), ground_y);
        draw::hline(grid, 0, w.saturating_sub(1), ground_y + 1);

        // ── Holes: shade-2 ovals at fixed x positions below the ground ────────
        let hole_count = (cw / 4).max(1);
        let hole_spacing = cw / hole_count;

        for h_idx in 0..hole_count {
            let hole_cx = h_idx * hole_spacing + hole_spacing / 2;
            if hole_cx < cw && ch > 0 {
                draw::shade(grid, hole_cx, ch.saturating_sub(1), 2);
                if hole_cx + 1 < cw {
                    draw::shade(grid, hole_cx + 1, ch.saturating_sub(1), 2);
                }
            }
        }

        // ── Moles: bonked_count rise from their holes ─────────────────────────
        let bonked = (ctx.eased * hole_count as f32) as usize;

        for h_idx in 0..hole_count {
            let hole_cx_cell = h_idx * hole_spacing + hole_spacing / 2;
            let hole_x = hole_cx_cell * 2;

            let is_bonked = h_idx < bonked;

            if is_bonked {
                // Bonked mole — stays peeking at the rim as a shade-3 stub.
                let mole_y = ground_y.saturating_sub(1);
                draw::dot(grid, hole_x.min(w.saturating_sub(1)), mole_y);
                draw::dot(grid, (hole_x + 1).min(w.saturating_sub(1)), mole_y);
                // Stars above (bonk effect, animated with time).
                let star_anim = ((ctx.time * 5.0 + h_idx as f32) as usize) % 3;
                if star_anim < 2 && mole_y >= 2 {
                    draw::dot(
                        grid,
                        hole_x.min(w.saturating_sub(1)),
                        mole_y.saturating_sub(2),
                    );
                    draw::dot(
                        grid,
                        (hole_x + 2).min(w.saturating_sub(1)),
                        mole_y.saturating_sub(2),
                    );
                }
            } else {
                // Un-bonked mole — parabolic pop animation.
                let phase = h_idx as f32 * 0.7;
                let t = (ctx.time * 1.8 + phase).fract(); // 0→1
                                                          // Parabola: up on first half, down on second.
                let arc = 1.0 - (t * 2.0 - 1.0).powi(2);
                let mole_dot_y = (ground_y as f32 - arc * (ground_y as f32 * 0.8)).round() as usize;

                // Mole body: 2×3 block.
                for dy in 0..3usize {
                    for dx in 0..2usize {
                        let mx = (hole_x + dx).min(w.saturating_sub(1));
                        let my = mole_dot_y + dy;
                        if my < ground_y {
                            draw::dot(grid, mx, my);
                        }
                    }
                }
                // Mole eyes (2 dots).
                if mole_dot_y + 1 < ground_y {
                    draw::dot(grid, hole_x.min(w.saturating_sub(1)), mole_dot_y);
                    draw::dot(grid, (hole_x + 1).min(w.saturating_sub(1)), mole_dot_y);
                }
            }
        }

        // ── Dim LCD background: only cells left blank by the scene ───────────
        // (a shade glyph overrides braille dots, so it must come last)
        for cy in 0..ch {
            for cx in 0..cw {
                if grid.get_char(cx, cy) == '\u{2800}' {
                    draw::shade(grid, cx, cy, 1);
                }
            }
        }

        // ── Palette tint ──────────────────────────────────────────────────────
        for cy in 0..ch {
            let bonked_cells = (bonked * cw / hole_count.max(1)).min(cw);
            if bonked_cells > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    bonked_cells.saturating_sub(1),
                    ctx.palette.sample(ctx.eased),
                );
            }
        }
        Ok(())
    }
}

// ─── 10. Wario Treasure ──────────────────────────────────────────────────────

/// Coin counter / treasure meter — coins flip and stack as progress rises.
struct WarioTreasure;
impl ProgressStyle for WarioTreasure {
    fn name(&self) -> &str {
        "wario-treasure"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "Wario coin meter: coins flip and land in a chest; treasure fraction = progress"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();

        // ── LCD background ────────────────────────────────────────────────────
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── Chest / treasure box — right half of the bar ──────────────────────
        // Chest outline in cell space.
        let chest_start = cw / 2;
        let chest_w = cw.saturating_sub(chest_start).max(1);
        // Lid (top row).
        for cx in chest_start..chest_start + chest_w {
            if cx < cw {
                draw::shade(grid, cx, 0, 4);
            }
        }
        // Sides.
        for cy in 1..ch {
            if chest_start < cw {
                draw::shade(grid, chest_start, cy, 4);
            }
            if chest_start + chest_w.saturating_sub(1) < cw {
                draw::shade(grid, chest_start + chest_w.saturating_sub(1), cy, 4);
            }
        }
        // Bottom.
        if ch > 0 {
            for cx in chest_start..chest_start + chest_w {
                if cx < cw {
                    draw::shade(grid, cx, ch.saturating_sub(1), 4);
                }
            }
        }

        // Chest fill: shade 3/4 from the bottom up proportional to eased.
        let fill_rows = (ctx.eased * (ch.saturating_sub(2)) as f32).round() as usize;
        let fill_start = ch.saturating_sub(1 + fill_rows).max(1);
        for cy in fill_start..ch.saturating_sub(1) {
            for cx in chest_start + 1..chest_start + chest_w.saturating_sub(1) {
                if cx < cw && cy < ch {
                    let t = (cy.saturating_sub(fill_start)) as f32 / fill_rows.max(1) as f32;
                    let shade_lvl = if t < 0.5 { 3usize } else { 4 };
                    draw::shade(grid, cx, cy, shade_lvl);
                }
            }
        }

        // ── Coins: stacked shade circles in the left half ─────────────────────
        let coin_area_w = chest_start.max(1);
        let total_coin_cells = coin_area_w * ch;
        let filled_coins = (ctx.eased * total_coin_cells as f32) as usize;

        // Fill coins column by column, bottom-up.
        let cols = coin_area_w;
        let rows = ch;
        for idx in 0..filled_coins.min(total_coin_cells) {
            let col = idx % cols;
            let row_from_bottom = idx / cols;
            let row = rows.saturating_sub(1 + row_from_bottom);
            if col < cw && row < ch {
                draw::shade(grid, col, row, 3);
            }
        }

        // ── Flying coin animation ─────────────────────────────────────────────
        // A coin arcs from left pile to the chest.
        let coin_t = (ctx.time * 1.2).fract();
        let coin_x = (coin_t * (w.saturating_sub(2)) as f32) as i32;
        let arc_h = (h / 2) as f32;
        let coin_y = (arc_h * (1.0 - (coin_t * PI).sin())).round() as i32;
        draw::dot_i(grid, coin_x, coin_y);
        draw::dot_i(grid, coin_x + 1, coin_y);
        // Coin flip: squeeze horizontally at 0/1 of arc, full at peak.
        let flip = (ctx.time * 6.0).sin();
        if flip.abs() > 0.3 {
            draw::dot_i(grid, coin_x, coin_y + 1);
            draw::dot_i(grid, coin_x + 1, coin_y + 1);
        }

        // ── Palette tint ──────────────────────────────────────────────────────
        // Gold on coins, green/blue on chest fill.
        let coin_cells = (ctx.eased * coin_area_w as f32) as usize;
        for cy in 0..ch {
            if coin_cells > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    coin_cells.min(cw).saturating_sub(1),
                    ctx.palette.sample(0.9),
                );
            }
            if fill_rows > 0 && fill_start <= cy && cy < ch.saturating_sub(1) {
                if chest_start < cw {
                    draw::tint_row(
                        grid,
                        cy,
                        chest_start + 1,
                        (chest_start + chest_w.saturating_sub(2)).min(cw.saturating_sub(1)),
                        ctx.palette.sample(ctx.eased),
                    );
                }
            }
        }
        Ok(())
    }
}

// ─── 11. LCD Pinball ─────────────────────────────────────────────────────────

/// LCD segment pinball: flippers, bumpers, and a ball; lit bumpers = progress.
struct LcdPinball;
impl ProgressStyle for LcdPinball {
    fn name(&self) -> &str {
        "lcd-pinball"
    }
    fn theme(&self) -> &str {
        "gameboy"
    }
    fn describe(&self) -> &str {
        "LCD pinball: segment flippers and bumpers; lit bumpers = progress, ball bounces with time"
    }

    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();

        // ── Pinball table background ───────────────────────────────────────────
        for cy in 0..ch {
            for cx in 0..cw {
                draw::shade(grid, cx, cy, 1);
            }
        }

        // ── Side walls ────────────────────────────────────────────────────────
        draw::vline(grid, 0, 0, h.saturating_sub(1));
        draw::vline(grid, w.saturating_sub(1), 0, h.saturating_sub(1));

        // ── Bumpers: shade ovals in a row near the top ────────────────────────
        let bumper_count = (cw / 3).max(1).min(5);
        let bumper_spacing = cw / (bumper_count + 1);
        let lit_bumpers = (ctx.eased * bumper_count as f32).round() as usize;

        for b in 0..bumper_count {
            let bx = (b + 1) * bumper_spacing;
            let by = ch / 3;
            if bx < cw && by < ch {
                let shade_lvl = if b < lit_bumpers { 4usize } else { 2 };
                draw::shade(grid, bx, by, shade_lvl);
                // Bumper halo.
                if bx + 1 < cw {
                    draw::shade(grid, bx + 1, by, (shade_lvl.saturating_sub(1)).max(1));
                }
                if bx > 0 {
                    draw::shade(
                        grid,
                        bx.saturating_sub(1),
                        by,
                        (shade_lvl.saturating_sub(1)).max(1),
                    );
                }
                if by > 0 {
                    draw::shade(
                        grid,
                        bx,
                        by.saturating_sub(1),
                        (shade_lvl.saturating_sub(1)).max(1),
                    );
                }
                if by + 1 < ch {
                    draw::shade(grid, bx, by + 1, (shade_lvl.saturating_sub(1)).max(1));
                }
            }
        }

        // ── Flippers at the bottom ────────────────────────────────────────────
        // Two flipper segments that angle up/down with time.
        let flip_row = ch.saturating_sub(2);
        let mid_cell = cw / 2;
        let flipper_len = (cw / 4).max(1);

        // Left flipper — angles based on time.
        let left_raised = (ctx.time * 3.0).sin() > 0.3;
        let left_shade = if left_raised { 4usize } else { 3 };
        for i in 0..flipper_len {
            let cx = mid_cell.saturating_sub(flipper_len).saturating_sub(1) + i;
            let cy = if left_raised && i < flipper_len / 2 {
                flip_row.saturating_sub(1)
            } else {
                flip_row
            };
            if cx < cw && cy < ch {
                draw::shade(grid, cx, cy, left_shade);
            }
        }

        // Right flipper.
        let right_raised = !left_raised;
        let right_shade = if right_raised { 4usize } else { 3 };
        for i in 0..flipper_len {
            let cx = mid_cell + 1 + i;
            let cy = if right_raised && i >= flipper_len / 2 {
                flip_row.saturating_sub(1)
            } else {
                flip_row
            };
            if cx < cw && cy < ch {
                draw::shade(grid, cx, cy, right_shade);
            }
        }

        // ── Ball trajectory ────────────────────────────────────────────────────
        // Ball bounces in an elliptical path around the playfield.
        let ball_t = ctx.time * 1.5;
        let ball_cx = w / 2;
        let ball_cy_center = h / 2;
        let rx = (w.saturating_sub(4) / 2) as f32;
        let ry = (h.saturating_sub(4) / 2).max(1) as f32;
        let bx = (ball_cx as f32 + ball_t.cos() * rx).round() as i32;
        let by = (ball_cy_center as f32 + (ball_t * 0.7).sin() * ry).round() as i32;
        // 2×2 ball.
        draw::dot_i(grid, bx, by);
        draw::dot_i(grid, bx + 1, by);
        draw::dot_i(grid, bx, by + 1);
        draw::dot_i(grid, bx + 1, by + 1);

        // ── Palette tint on lit bumper region ─────────────────────────────────
        for cy in 0..ch {
            let active_w = (ctx.eased * cw as f32) as usize;
            if active_w > 0 {
                draw::tint_row(
                    grid,
                    cy,
                    0,
                    active_w.saturating_sub(1),
                    ctx.palette.sample(ctx.eased),
                );
            }
        }
        Ok(())
    }
}
