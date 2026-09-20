//! NES / Nintendo-classics themed progress bars.
//!
//! Each style is mechanically distinct — geometry, algorithm, motion, and
//! symbol choice all differ. Colour alone is never the only differentiator.
//!
//! Styles:
//! - `mario-run`      — Mario runs rightward; at 100% the flagpole drops.
//! - `zelda-hearts`   — Heart-container row fills one half-heart at a time.
//! - `metroid-etanks` — Energy-tank segments charge one at a time.
//! - `tetris-well`    — A well fills upward with a settling tetromino stack.
//! - `duck-hunt`      — Ducks arc across the sky; scored ducks show progress.
//! - `excitebike`     — A bike races right with a turbo-heat gauge on row 2.
//! - `punchout-stars` — A stamina bar that depletes then refills as star-power.
//! - `contra-spread`  — Spread-shot bullets fan out rightward.
//! - `megaman-weapon` — Segmented Mega Man weapon-energy bar charges top-down.
//! - `iceclimber-up`  — Platforms ascend; climber rises with `eased`.
//! - `donkey-barrel`  — Barrels roll down girders; Mario climbs a ladder.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Theme tint — cartridge red into NES blue. Applied to styles that draw monochrome.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(232, 64, 52);
const TINT_END: Color = Color::rgb(72, 124, 255);

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

/// All styles in the `nintendo` theme.
///
/// Returns one boxed implementor per NES game mechanic.  Every style is
/// a stateless unit struct — no heap allocation beyond the `Box`.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(MarioRun)),
        Box::new(Tinted(ZeldaHearts)),
        Box::new(Tinted(MetroidETanks)),
        Box::new(Tinted(TetrisWell)),
        Box::new(Tinted(DuckHunt)),
        Box::new(Excitebike),
        Box::new(PunchOutStars),
        Box::new(Tinted(ContraSpread)),
        Box::new(MegaManWeapon),
        Box::new(Tinted(IceClimberUp)),
        Box::new(Tinted(DonkeyBarrel)),
    ]
}

// ─── 1. Mario Run ────────────────────────────────────────────────────────────

/// Mario runs rightward with animated legs; at 100% a flagpole appears on the
/// right and Mario "slides" (collapses to a dot at the base).
struct MarioRun;
impl ProgressStyle for MarioRun {
    fn name(&self) -> &str {
        "mario-run"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Mario sprints right in braille dots; flagpole drops at 100%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Ground line across the bottom.
        draw::hline(grid, 0, dw.saturating_sub(1), dh.saturating_sub(1));

        let mario_x =
            ((ctx.eased * (dw.saturating_sub(4)) as f32) as usize).min(dw.saturating_sub(1));
        let ground_y = dh.saturating_sub(1);

        if ctx.progress >= 0.999 {
            // Flagpole on the far right.
            let pole_x = dw.saturating_sub(2);
            draw::vline(grid, pole_x, 0, ground_y);
            // Flag (small filled square near top).
            if dh >= 3 {
                draw::fill_rect(grid, pole_x + 1, 0, 1, (dh / 3).max(1));
            }
            // Mario at the base of the pole (just a dot cluster).
            draw::dot(grid, pole_x, ground_y);
            if pole_x > 0 {
                draw::dot(grid, pole_x.saturating_sub(1), ground_y);
            }
        } else {
            // Mario body: 2-wide, 3-tall (scaled by dh).
            let body_h = ((dh as f32 * 0.6) as usize).max(1);
            let body_y = ground_y.saturating_sub(body_h);
            draw::fill_rect(
                grid,
                mario_x,
                body_y,
                2.min(dw.saturating_sub(mario_x)),
                body_h,
            );

            // Animated legs — alternate two dot patterns driven by time.
            let step = ((ctx.time * 8.0) as usize) % 2;
            if dh >= 2 {
                let lx = mario_x + step;
                draw::dot(grid, lx.min(dw.saturating_sub(1)), ground_y);
                if mario_x + 1 < dw {
                    draw::dot(grid, (mario_x + 1).saturating_sub(step), ground_y);
                }
            }
        }

        // Coin trail to the left of Mario (shows how far he's come).
        let coins = (ctx.eased * (mario_x / 4).max(1) as f32) as usize;
        for i in 0..coins {
            let cx = i * 4;
            if cx + 1 < mario_x && cx < dw {
                draw::dot(grid, cx, ground_y.saturating_sub(dh / 3));
            }
        }

        Ok(())
    }
}

// ─── 2. Zelda Hearts ─────────────────────────────────────────────────────────

/// Zelda HUD heart containers: full hearts are dense-filled, empty hearts are
/// outlines, and partial is a half-full heart.  Progress maps to half-hearts.
struct ZeldaHearts;
impl ProgressStyle for ZeldaHearts {
    fn name(&self) -> &str {
        "zelda-hearts"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Zelda heart-container row — fills one half-heart at a time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Pack as many 3-cell-wide hearts as fit, with a 1-cell gap.
        let heart_cell_w = 3usize;
        let gap = 1usize;
        let slot_w = heart_cell_w + gap;
        let n_hearts = (cw / slot_w).max(1);
        let half_hearts_total = n_hearts * 2;
        let half_filled = (ctx.eased * half_hearts_total as f32).round() as usize;

        let row = ch / 2; // draw in vertical middle

        for i in 0..n_hearts {
            let x0 = i * slot_w;
            let full_halves = (half_filled).saturating_sub(i * 2);
            // full_halves for this heart: 0 = empty, 1 = half, 2 = full
            let state = full_halves.min(2);

            // Heart outline (3 dots wide, 2 dots tall in dot-space):
            //  cols x0*2, x0*2+1, x0*2+2, x0*2+3, x0*2+4, x0*2+5
            let dx = x0 * 2;
            let (dw, dh) = draw::dot_dims(grid);
            let dy = (row * 4).min(dh.saturating_sub(3));
            // Two bumps on top row.
            if dx + 1 < dw && dy < dh {
                draw::dot(grid, dx + 1, dy);
            }
            if dx + 4 < dw && dy < dh {
                draw::dot(grid, dx + 4, dy);
            }
            // Wider middle: outline edges only, so the fill state reads.
            if dy + 1 < dh {
                if dx < dw {
                    draw::dot(grid, dx, dy + 1);
                }
                if dx + 5 < dw {
                    draw::dot(grid, dx + 5, dy + 1);
                }
            }
            // Taper bottom: outline edges only.
            if dy + 2 < dh {
                if dx + 1 < dw {
                    draw::dot(grid, dx + 1, dy + 2);
                }
                if dx + 4 < dw {
                    draw::dot(grid, dx + 4, dy + 2);
                }
            }
            if dx + 2 < dw && dy + 3 < dh {
                draw::dot(grid, dx + 2, dy + 3);
            }
            if dx + 3 < dw && dy + 3 < dh {
                draw::dot(grid, dx + 3, dy + 3);
            }

            // Fill interior based on state.
            if state == 2 {
                // Full: fill centre of the heart.
                for xx in (dx + 1)..(dx + 5).min(dw) {
                    if dy + 1 < dh {
                        draw::dot(grid, xx, dy + 1);
                    }
                }
                for xx in (dx + 1)..(dx + 5).min(dw) {
                    if dy + 2 < dh {
                        draw::dot(grid, xx, dy + 2);
                    }
                }
            } else if state == 1 {
                // Half: fill left side only.
                for xx in (dx + 1)..(dx + 3).min(dw) {
                    if dy + 1 < dh {
                        draw::dot(grid, xx, dy + 1);
                    }
                    if dy + 2 < dh {
                        draw::dot(grid, xx, dy + 2);
                    }
                }
            }
            // state == 0: outline only (already drawn above)
        }

        Ok(())
    }
}

// ─── 3. Metroid Energy Tanks ──────────────────────────────────────────────────

/// Metroid HUD: a row of rectangular energy-tank segments that charge one at a
/// time.  Each tank is a bordered rectangle; the active one shows a partial fill.
struct MetroidETanks;
impl ProgressStyle for MetroidETanks {
    fn name(&self) -> &str {
        "metroid-etanks"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Metroid energy-tank segments — each tank charges then the next activates"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let tank_dot_w = ((dw / 6).max(3)).min(dw);
        let gap_dots = 1usize;
        let n_tanks = (dw / (tank_dot_w + gap_dots)).max(1);

        let filled_f = ctx.eased * n_tanks as f32;
        let full_tanks = filled_f as usize;
        let partial_f = filled_f.fract();

        for i in 0..n_tanks {
            let x0 = i * (tank_dot_w + gap_dots);
            if x0 >= dw {
                break;
            }
            let actual_w = tank_dot_w.min(dw.saturating_sub(x0));
            // Border.
            draw::rect_outline(grid, x0, 0, actual_w, dh);
            // Fill.
            if i < full_tanks {
                // Fully charged: fill interior.
                let iw = actual_w.saturating_sub(2);
                let ih = dh.saturating_sub(2);
                if iw > 0 && ih > 0 {
                    draw::fill_rect(grid, x0 + 1, 1, iw, ih);
                }
            } else if i == full_tanks && partial_f > 0.001 {
                // Partially charged: fill bottom portion of interior.
                let iw = actual_w.saturating_sub(2);
                let ih = dh.saturating_sub(2);
                if iw > 0 && ih > 0 {
                    let charge_h = ((partial_f * ih as f32).round() as usize).max(1).min(ih);
                    let y0 = ih + 1 - charge_h; // fills bottom-up
                    draw::fill_rect(grid, x0 + 1, y0, iw, charge_h);
                }
            }
        }

        Ok(())
    }
}

// ─── 4. Tetris Well ──────────────────────────────────────────────────────────

/// A vertical well (left and right walls, open top) fills from the bottom as
/// tetrominoes settle.  The stack height is driven by `eased`; the topmost
/// layer shows the current "falling" piece as a blinking row.
struct TetrisWell;
impl ProgressStyle for TetrisWell {
    fn name(&self) -> &str {
        "tetris-well"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Tetris well fills upward with a settling block stack"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Well walls.
        draw::vline(grid, 0, 0, dh.saturating_sub(1));
        draw::vline(grid, dw.saturating_sub(1), 0, dh.saturating_sub(1));
        // Floor.
        draw::hline(grid, 0, dw.saturating_sub(1), dh.saturating_sub(1));

        let inner_w = dw.saturating_sub(2);
        let inner_h = dh.saturating_sub(1);
        if inner_w == 0 || inner_h == 0 {
            return Ok(());
        }

        // Stack: fills from the bottom.
        let stack_dots = (ctx.eased * inner_h as f32) as usize;
        let stack_dots = stack_dots.min(inner_h);
        let stack_y0 = inner_h.saturating_sub(stack_dots);

        // Draw stacked rows with slight texture (every 4 dots a gap line).
        for y in stack_y0..inner_h {
            let row_mod = (inner_h - 1 - y) % 4;
            if row_mod == 3 {
                // "Mortar" gap — sparse dots.
                for x in (1..inner_w + 1).step_by(3) {
                    draw::dot(grid, x, y);
                }
            } else {
                draw::hline(grid, 1, inner_w, y);
            }
        }

        // Falling piece: a blinking 4-wide row just above the stack.
        if stack_dots < inner_h {
            let piece_y = stack_y0.saturating_sub(1);
            let blink = ((ctx.time * 4.0) as usize) % 2 == 0;
            if blink && piece_y < inner_h {
                // Randomise piece shape by time bucket (cycles through shapes).
                let shape = ((ctx.time * 0.5) as usize) % 5;
                let pw = (inner_w.min(8)).max(1);
                let px0 = 1 + (inner_w.saturating_sub(pw)) / 2;
                match shape {
                    0 => draw::hline(grid, px0, (px0 + 3).min(dw.saturating_sub(1)), piece_y), // I
                    1 => {
                        // L
                        draw::hline(grid, px0, (px0 + 2).min(dw.saturating_sub(1)), piece_y);
                        if piece_y >= 1 {
                            draw::dot(grid, px0 + 2, piece_y.saturating_sub(1));
                        }
                    }
                    2 => {
                        // S
                        draw::hline(grid, px0 + 1, (px0 + 3).min(dw.saturating_sub(1)), piece_y);
                        if piece_y >= 1 {
                            draw::hline(
                                grid,
                                px0,
                                (px0 + 2).min(dw.saturating_sub(1)),
                                piece_y.saturating_sub(1),
                            );
                        }
                    }
                    3 => {
                        // T
                        draw::hline(grid, px0, (px0 + 2).min(dw.saturating_sub(1)), piece_y);
                        if piece_y >= 1 {
                            draw::dot(grid, px0 + 1, piece_y.saturating_sub(1));
                        }
                    }
                    _ => {
                        // O (square)
                        draw::hline(grid, px0, (px0 + 1).min(dw.saturating_sub(1)), piece_y);
                        if piece_y >= 1 {
                            draw::hline(
                                grid,
                                px0,
                                (px0 + 1).min(dw.saturating_sub(1)),
                                piece_y.saturating_sub(1),
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

// ─── 5. Duck Hunt ─────────────────────────────────────────────────────────────

/// Ducks arc across the sky using sinusoidal paths; the number of "shot" ducks
/// equals `round(progress * total_ducks)`.  Downed ducks fall to the ground.
struct DuckHunt;
impl ProgressStyle for DuckHunt {
    fn name(&self) -> &str {
        "duck-hunt"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Ducks arc across the sky; progress = ducks shot, complete ducks fall"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Ground.
        draw::hline(grid, 0, dw.saturating_sub(1), dh.saturating_sub(1));

        let n_ducks = 5usize;
        let shot_count = (ctx.eased * n_ducks as f32).round() as usize;

        for i in 0..n_ducks {
            let phase = i as f32 * 0.7 + ctx.time * 0.8;
            let t_norm = (phase * 0.4).fract(); // 0..1 left-to-right

            if i < shot_count {
                // Shot duck: falls toward the bottom.
                let fall = ((ctx.time - (i as f32 * 0.3)).max(0.0) * 12.0) as i32;
                let dx = (t_norm * dw as f32) as i32;
                let ground = dh as i32 - 2;
                let dy = (fall).min(ground);
                // Tumbling duck (just dots in a small cluster).
                draw::dot_i(grid, dx, dy);
                draw::dot_i(grid, dx + 1, dy);
                draw::dot_i(grid, dx, dy + 1);
            } else {
                // Live duck: sinusoidal arc across sky.
                let dx = (t_norm * dw as f32) as i32;
                let dy = ((PI * t_norm).sin() * (dh.saturating_sub(3)) as f32 * 0.6 + 1.0) as i32;
                // 2×2 body.
                draw::dot_i(grid, dx, dy);
                draw::dot_i(grid, dx + 1, dy);
                draw::dot_i(grid, dx, dy - 1);
                // Wing flap (alternate row).
                let flap = (((ctx.time * 6.0 + i as f32) as i32) % 2) as i32;
                draw::dot_i(grid, dx + 2, dy - flap);
            }
        }

        Ok(())
    }
}

// ─── 6. Excitebike ───────────────────────────────────────────────────────────

/// The bike position on row 1 tracks `eased` (progress = distance covered).
/// Row 2 is a TURBO HEAT gauge: it oscillates — if it overheats the bike icon
/// stutters (shows a "flame" glyph and the bar pulses red via tinting).
struct Excitebike;
impl ProgressStyle for Excitebike {
    fn name(&self) -> &str {
        "excitebike"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Bike races right; row 2 is a turbo-heat gauge that must not overheat"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // ── Track (ground line) ──
        draw::hline(grid, 0, dw.saturating_sub(1), dh.saturating_sub(1));

        // ── Bike position ──
        let bike_x =
            ((ctx.eased * (dw.saturating_sub(4)) as f32) as usize).min(dw.saturating_sub(1));
        let wheel_y = dh.saturating_sub(2);

        // Front and rear wheels (single dots).
        draw::dot(grid, bike_x, wheel_y);
        if bike_x >= 3 {
            draw::dot(grid, bike_x.saturating_sub(3), wheel_y);
        }

        // Frame (diagonal line from rear-wheel to handlebars).
        for i in 0..3usize {
            draw::dot_i(grid, bike_x as i32 - i as i32, wheel_y as i32 - i as i32);
        }
        // Rider head.
        if wheel_y >= 3 {
            draw::dot(grid, bike_x, wheel_y.saturating_sub(3));
        }

        // ── Turbo heat gauge on row 2 (cell row 1 if height > 1) ──
        if ch >= 2 && cw > 0 {
            // Heat oscillates with time; at high progress it climbs.
            let heat = (ctx.time * 1.2).sin() * 0.3 + ctx.eased * 0.7;
            let heat = heat.clamp(0.0, 1.0);
            let overheat = heat > 0.85;

            // Use hbar in the second-to-last cell row for the gauge.
            let gauge_row = ch.saturating_sub(2).min(ch.saturating_sub(1));
            // Draw gauge fill via glyph calls on the heat-gauge row.
            let heat_cells = (heat * cw as f32) as usize;
            for cx in 0..heat_cells.min(cw) {
                let shade = if overheat { 4usize } else { 2 };
                draw::shade(grid, cx, gauge_row, shade);
            }

            // Tint gauge row: red if overheating, yellow otherwise.
            if overheat {
                let hot_color = crate::Color::rgb(255, 40, 0);
                draw::tint_row(grid, gauge_row, 0, cw.saturating_sub(1), hot_color);
            } else {
                let warm_color = crate::Color::rgb(255, 200, 0);
                draw::tint_row(
                    grid,
                    gauge_row,
                    0,
                    heat_cells.min(cw.saturating_sub(1)),
                    warm_color,
                );
            }
        }

        Ok(())
    }
}

// ─── 7. Punch-Out Stars ──────────────────────────────────────────────────────

/// A segmented stamina bar that depletes left-to-right as `progress` rises,
/// then at 75%+ refills from right-to-left as a "star-power" burst (bright
/// glyphs).  Uses `vblock` for segment columns.
struct PunchOutStars;
impl ProgressStyle for PunchOutStars {
    fn name(&self) -> &str {
        "punchout-stars"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Punch-Out stamina bar depletes then surges back as star-power at 75%"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Stamina: depletes until 75%, then star-power refills.
        let star_power = ctx.progress > 0.75;
        let bar_frac = if star_power {
            // Refill from 0 → 1 over the range 0.75..1.0.
            (ctx.progress - 0.75) / 0.25
        } else {
            // Deplete from 1 → 0 over the range 0..0.75.
            1.0 - ctx.progress / 0.75
        };
        let bar_frac = bar_frac.clamp(0.0, 1.0);

        let filled_cells = (bar_frac * cw as f32).round() as usize;

        for cx in 0..cw {
            for cy in 0..ch {
                let level = if cx < filled_cells { 8usize } else { 0 };
                if level > 0 {
                    if star_power {
                        // Star-power: alternate between full and heavy-shade.
                        let pulse = ((ctx.time * 8.0 + cx as f32 * 0.5) as usize) % 2;
                        draw::shade(grid, cx, cy, if pulse == 0 { 4 } else { 3 });
                    } else {
                        draw::vblock(grid, cx, cy, level);
                    }
                } else {
                    draw::shade(grid, cx, cy, 1);
                }
            }
        }

        // Tint: yellow stamina, white star-power.
        let color = if star_power {
            crate::Color::rgb(255, 255, 180)
        } else {
            crate::Color::rgb(255, 220, 0)
        };
        for cy in 0..ch {
            draw::tint_row(grid, cy, 0, filled_cells.min(cw).saturating_sub(1), color);
        }

        Ok(())
    }
}

// ─── 8. Contra Spread ────────────────────────────────────────────────────────

/// Contra spread-shot: multiple bullets fan out from the left edge in a
/// symmetric arc.  Progress controls how far the fan has travelled rightward.
/// Each bullet trail is a dotted line along its angle.
struct ContraSpread;
impl ProgressStyle for ContraSpread {
    fn name(&self) -> &str {
        "contra-spread"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Contra spread-shot bullets fan out rightward; progress = bullet travel"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx_f = 0.0f32; // fire from left edge
        let cy_f = (dh as f32 - 1.0) / 2.0; // vertical centre
        let n_rays = 5usize;
        let reach = (ctx.eased * dw as f32) as usize;

        // Also draw a muzzle flash at the origin.
        let flash = ((ctx.time * 12.0) as usize) % 2 == 0;
        if flash {
            draw::dot(grid, 0, cy_f as usize);
        }

        for i in 0..n_rays {
            // Angles: 0° (straight), ±22.5°, ±45°.
            let angle_idx = i as i32 - (n_rays as i32 / 2);
            let angle = angle_idx as f32 * PI / 8.0; // 22.5° steps
            let dx_f = angle.cos();
            let dy_f = angle.sin();

            // Draw the bullet trail up to reach dots.
            let steps = reach.min(dw);
            for s in (0..steps).step_by(2) {
                let bx = (cx_f + s as f32 * dx_f) as i32;
                let by = (cy_f + s as f32 * dy_f) as i32;
                draw::dot_i(grid, bx, by);
            }
            // Bullet head at the tip.
            if reach > 0 {
                let bx = (cx_f + reach as f32 * dx_f) as i32;
                let by = (cy_f + reach as f32 * dy_f) as i32;
                draw::dot_i(grid, bx, by);
                draw::dot_i(grid, bx + 1, by);
            }
        }

        Ok(())
    }
}

// ─── 9. Mega Man Weapon ───────────────────────────────────────────────────────

/// Mega Man weapon-energy: a tall narrow bar divided into discrete segments
/// (like the in-game weapon panel).  Segments light up from the bottom; the
/// active segment shows sub-segment precision via `vblock`.
struct MegaManWeapon;
impl ProgressStyle for MegaManWeapon {
    fn name(&self) -> &str {
        "megaman-weapon"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Mega Man weapon-energy bar — segmented column charges from bottom up"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Number of segments = cell height; each segment is one cell row.
        let n_segs = ch;
        let seg_f = ctx.eased * n_segs as f32;
        let full_segs = seg_f as usize;
        let partial = seg_f.fract();

        // The weapon bar occupies columns 0..=1 (or full width if narrow).
        let bar_w = cw.min(2);

        for seg in 0..n_segs {
            // Segments fill bottom-up: seg 0 = bottom (ch-1 in cell coords).
            let cell_y = n_segs.saturating_sub(1).saturating_sub(seg);

            if seg < full_segs {
                // Full segment: solid fill.
                for cx in 0..bar_w {
                    draw::glyph(grid, cx, cell_y, '█');
                }
            } else if seg == full_segs && partial > 0.001 {
                // Partial segment using vblock precision.
                let level = (partial * 8.0).round() as usize;
                for cx in 0..bar_w {
                    draw::vblock(grid, cx, cell_y, level);
                }
            } else {
                // Empty segment — light border using shade 1.
                for cx in 0..bar_w {
                    draw::shade(grid, cx, cell_y, 1);
                }
            }
        }

        // Remainder columns: empty track.
        for cx in bar_w..cw {
            for cy in 0..ch {
                draw::shade(grid, cx, cy, 0);
            }
        }

        // Tint the filled column.
        let bar_color = ctx.palette.sample(ctx.eased);
        for seg in 0..full_segs.min(ch) {
            let cell_y = ch.saturating_sub(1).saturating_sub(seg);
            draw::tint_row(grid, cell_y, 0, bar_w.saturating_sub(1), bar_color);
        }

        Ok(())
    }
}

// ─── 10. Ice Climber Up ───────────────────────────────────────────────────────

/// Platforms scroll upward: `eased` determines how high the climber has risen.
/// The bottom of the grid shows platforms drawn as horizontal dot-lines at
/// staggered heights; the climber is a small dot cluster that moves up with
/// `eased`.
struct IceClimberUp;
impl ProgressStyle for IceClimberUp {
    fn name(&self) -> &str {
        "iceclimber-up"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "Ice Climber ascends upward platform by platform driven by eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Platforms: spaced every dh/4 dots, alternating left-right alignment,
        // scrolling upward with time so the world moves past the climber.
        let scroll_offset = (ctx.time * 2.0) as usize % (dh.max(1));
        let plat_spacing = (dh / 4).max(2);
        let plat_count = (dh / plat_spacing) + 2;

        for p in 0..plat_count {
            let base_y = p * plat_spacing;
            let y = (base_y + scroll_offset) % (dh + plat_spacing);
            if y >= dh {
                continue;
            }

            let is_left = p % 2 == 0;
            let pw = dw * 3 / 4;
            let px0 = if is_left { 0 } else { dw.saturating_sub(pw) };
            draw::hline(
                grid,
                px0,
                (px0 + pw).saturating_sub(1).min(dw.saturating_sub(1)),
                y,
            );
            // Small snow pillar at platform edges.
            if y + 1 < dh {
                draw::dot(grid, px0, y + 1);
                let right = (px0 + pw).saturating_sub(1).min(dw.saturating_sub(1));
                draw::dot(grid, right, y + 1);
            }
        }

        // Climber: rises from bottom (eased=0) to top (eased=1).
        let climber_y = (dh.saturating_sub(3) as f32 * (1.0 - ctx.eased)) as usize;
        let climber_x = dw / 2;
        // Head.
        draw::dot(grid, climber_x, climber_y);
        draw::dot(grid, climber_x + 1, climber_y);
        // Body.
        if climber_y + 1 < dh {
            draw::dot(grid, climber_x, climber_y + 1);
            draw::dot(grid, climber_x + 1, climber_y + 1);
        }
        // Legs alternate with time.
        if climber_y + 2 < dh {
            let leg_step = ((ctx.time * 6.0) as usize) % 2;
            draw::dot(grid, climber_x + leg_step, climber_y + 2);
        }

        Ok(())
    }
}

// ─── 11. Donkey Kong Barrel ───────────────────────────────────────────────────

/// Donkey Kong: girders drawn as horizontal dot-lines at staggered rows; barrels
/// are circles that roll down the girders from right to left driven by time.
/// Mario is a small dot cluster at the bottom-left that climbs a ladder (moves
/// up) as `eased` increases.
struct DonkeyBarrel;
impl ProgressStyle for DonkeyBarrel {
    fn name(&self) -> &str {
        "donkey-barrel"
    }
    fn theme(&self) -> &str {
        "nintendo"
    }
    fn describe(&self) -> &str {
        "DK barrels roll down girders; Mario climbs a ladder as progress rises"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // ── Girders: horizontal bands ──
        let n_girders = ((dh / 3).max(1)).min(4);
        let girder_gap = dh / (n_girders + 1).max(1);

        for g in 0..n_girders {
            let gy = dh.saturating_sub(1).saturating_sub((g + 1) * girder_gap);
            draw::hline(grid, 0, dw.saturating_sub(1), gy);
        }

        // ── Ladder: narrow vertical line at the far left ──
        draw::vline(grid, 1, 0, dh.saturating_sub(1));
        // Ladder rungs.
        let rung_spacing = 3usize;
        let mut ry = 0usize;
        while ry < dh {
            draw::dot(grid, 0, ry);
            draw::dot(grid, 2, ry);
            ry += rung_spacing;
        }

        // ── Barrels: roll along each girder, one per girder ──
        for g in 0..n_girders {
            let gy = dh.saturating_sub(1).saturating_sub((g + 1) * girder_gap);
            // Each barrel on a different girder has offset phase.
            let phase = g as f32 * 0.4 + ctx.time * 0.7;
            let barrel_t = 1.0 - (phase * 0.3).fract(); // travels right→left
            let bx = (barrel_t * (dw.saturating_sub(3)) as f32) as usize;

            // Barrel is a small circle: 3 dots wide.
            if bx < dw && gy >= 1 {
                draw::dot(grid, bx, gy.saturating_sub(1)); // top
                draw::dot(grid, bx.saturating_sub(1), gy); // err: was gy but actually the girder row
                                                           // Place barrel one row above girder so it sits on top.
                let barrel_y = gy.saturating_sub(1);
                draw::dot(grid, bx, barrel_y);
                if bx + 1 < dw {
                    draw::dot(grid, bx + 1, barrel_y);
                }
                // Rolling indicator: alternating side dot.
                let roll = ((ctx.time * 5.0 + g as f32) as usize) % 2;
                let side_dot = if roll == 0 {
                    bx.saturating_sub(1)
                } else {
                    (bx + 2).min(dw.saturating_sub(1))
                };
                if barrel_y >= 1 {
                    draw::dot(grid, side_dot, barrel_y);
                }
            }
        }

        // ── Mario: climbs the ladder from bottom to top ──
        let mario_y = (dh.saturating_sub(3) as f32 * (1.0 - ctx.eased)) as usize;
        // Mario is at x=1 (on the ladder).
        draw::dot(grid, 1, mario_y);
        draw::dot(grid, 2, mario_y);
        if mario_y + 1 < dh {
            draw::dot(grid, 1, mario_y + 1);
            draw::dot(grid, 2, mario_y + 1);
        }

        Ok(())
    }
}
