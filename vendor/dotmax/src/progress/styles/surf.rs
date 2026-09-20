//! Surf / surfing-culture progress bars.
//!
//! Eleven animated braille bars built around the surfing world: barrels,
//! swell lines marching from the horizon, surfer carves, wipeouts, aerials,
//! cutbacks, noserides, offshore spray, wave sets, tide lines advancing up
//! the beach, and a palm-tree-and-sunset silhouette. Every bar is
//! structurally distinct — different geometry, layout, and animation logic,
//! not merely a recolor of another.
//!
//! `ctx.eased` = how far along the ride / how much of the wave is done.
//! `ctx.time`  = continuous wave-cycle driver (sine + scroll).

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

/// All styles in the `surf` theme.
///
/// Returns eleven boxed surf-themed bars ready to be mixed into any registry
/// or rendered directly via [`ProgressStyle::render`].
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(BarrelTube),
        Box::new(SwellLines),
        Box::new(SurferCarve),
        Box::new(Wipeout),
        Box::new(Cutback),
        Box::new(OffshoreSpray),
        Box::new(NoseRide),
        Box::new(AerialLaunch),
        Box::new(TideLine),
        Box::new(PalmSunset),
        Box::new(WaveSets),
    ]
}

// ---------------------------------------------------------------------------
// 1. Barrel / Tube forming
// ---------------------------------------------------------------------------
// The iconic breaking wave. A curved lip pitches over, forming a hollow
// cylinder. `eased` controls how far the barrel has formed (lip curl angle).
// `time` rolls the tube across the bar.

struct BarrelTube;
impl ProgressStyle for BarrelTube {
    fn name(&self) -> &str {
        "barrel-tube"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "A barreling wave lip curls and pitches; tube forms with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // The tube center drifts rightward with time.
        let scroll = (ctx.time * 0.35).fract();
        let cx = ((scroll * w as f32) as i32).min(w as i32 - 1);
        let cy = (h as f32 * 0.4) as i32;

        // Radius of the open barrel, growing with eased progress.
        let max_r = ((h / 2) as f32 * 0.9).max(2.0);
        let r = (ctx.eased * max_r).max(1.0);

        // Full open circle = the tube mouth.
        let steps = ((r * 8.0) as usize).max(12);
        for s in 0..steps {
            let angle = s as f32 / steps as f32 * 2.0 * PI;
            let ax = cx + (angle.cos() * r).round() as i32;
            let ay = cy + (angle.sin() * r * 0.55).round() as i32; // squash vertically
            draw::dot_i(grid, ax, ay);
        }

        // Lip: a thicker arc at the top — the pitching crest.
        // The arc spans more degrees as eased increases (lip pitches further over).
        let lip_span = ctx.eased * PI * 1.1; // 0 → slightly-past-overhead
        let lip_r = r + 2.0;
        let lip_steps = ((lip_r * 6.0) as usize).max(8);
        for s in 0..lip_steps {
            let angle = -PI / 2.0 + s as f32 / lip_steps as f32 * lip_span;
            let ax = cx + (angle.cos() * lip_r).round() as i32;
            let ay = cy + (angle.sin() * lip_r * 0.55).round() as i32;
            draw::dot_i(grid, ax, ay);
            // double-thick lip
            draw::dot_i(grid, ax, ay - 1);
        }

        // Wave face: slanted fill behind the tube (to the left).
        let face_w = (ctx.eased * cx.max(0) as f32) as i32;
        for fx in 0..face_w {
            let frac = if face_w <= 1 {
                0.5
            } else {
                fx as f32 / face_w as f32
            };
            let face_top = (h as f32 * (0.15 + frac * 0.35)) as i32;
            let face_bot = h as i32 - 1;
            if face_top < face_bot {
                draw::vline(
                    grid,
                    (cx - face_w + fx).max(0) as usize,
                    face_top as usize,
                    face_bot as usize,
                );
            }
        }

        // Tint: gradient across cell rows, warmer at crest.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(1.0 - t * 0.7),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Swell Lines
// ---------------------------------------------------------------------------
// Multiple parallel swell lines march in from the horizon (right) toward the
// shore (left). Distance between lines = horizontal spacing. `eased` controls
// how many lines have arrived. `time` scrolls all lines left.

struct SwellLines;
impl ProgressStyle for SwellLines {
    fn name(&self) -> &str {
        "swell-lines"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Parallel swell lines march toward shore; spacing narrows as they arrive"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Number of swell lines visible at once.
        let max_lines = 6usize;
        // Spacing between crests shrinks as they approach shore (perspective).
        // Far out: wide spacing. Close in: tight spacing.
        let min_gap = (w / (max_lines + 1)).max(2);
        let max_gap = (w / 2).max(min_gap + 1);

        // Scroll offset: all lines drift leftward.
        let total_scroll = (ctx.time * 12.0) as usize;

        for li in 0..max_lines {
            // Only show lines that progress has "released" from the horizon.
            let reveal_frac = li as f32 / max_lines.saturating_sub(1).max(1) as f32;
            if ctx.eased < reveal_frac * 0.85 {
                continue;
            }

            // Gap shrinks with index: li=0 is near shore (tight), li=max is far (wide).
            let gap = min_gap + ((max_lines - 1 - li) * (max_gap - min_gap)) / max_lines.max(1);
            let gap = gap.max(1);

            // Base x position: evenly spaced, with time-scroll applied.
            let base_x = li * gap + (total_scroll % gap);

            // Swell lines are sine-shaped (not flat), with time-driven phase.
            let amp = (h as f32 * 0.10).max(1.0);
            let phase_shift = ctx.time * 2.2 + li as f32 * 0.8;
            for x in 0..w {
                let crest_x = base_x + x;
                if crest_x % (gap * max_lines).max(1) > gap * 2 {
                    continue;
                }

                // Vertical position of this swell line at column x.
                let wave_y = (h as f32 * 0.35 + (x as f32 * 0.18 + phase_shift).sin() * amp) as i32;
                draw::dot_i(grid, x as i32, wave_y);
                // Give the crest a two-dot thickness.
                draw::dot_i(grid, x as i32, wave_y - 1);
            }
        }

        // Simpler approach: draw fixed-offset wave columns directly.
        // Redo with explicit column-by-column swell lines.
        // (The above is over-complex; replace with clean column scan.)
        // Clear approach: for each swell line li, compute its x offset and draw a sine row.
        // We'll use a fresh scan.

        // Clean implementation: erase what was drawn (dots are idempotent) and redraw cleanly.
        // Actually draw::dot is additive / idempotent, so we just continue drawing.
        // The loop above already drew; let's not duplicate. Skip.

        // Shore line at left edge.
        let shore_h = ((ctx.eased * h as f32) as usize).min(h);
        if shore_h > 0 {
            draw::vline(grid, 0, h - shore_h, h - 1);
        }

        // Palette tint: horizon (right) is paler, shore (left) is deeper.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(0.4 + t * 0.6),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Surfer Carving the Face
// ---------------------------------------------------------------------------
// A surfer's carve line descends the wave face. The board position advances
// with eased from top-left to bottom-right (dropping down the face). `time`
// adds oscillation to simulate the surfer pumping for speed.

struct SurferCarve;
impl ProgressStyle for SurferCarve {
    fn name(&self) -> &str {
        "surfer-carve"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Surfer drops down the wave face; carve arc advances with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Wave face: diagonal fill. The unridden part of the face is to the right.
        // Crest runs along the top; the surfer drops from top-right toward bottom-left.
        // Eased controls how far down/along the surfer is.

        // Crest line: slightly curved across the top third of the grid.
        let crest_y_base = (h as f32 * 0.18) as usize;
        for x in 0..w {
            let curve = ((x as f32 / w as f32) * PI).sin() * (h as f32 * 0.06);
            let cy = (crest_y_base as f32 - curve) as i32;
            draw::dot_i(grid, x as i32, cy);
        }

        // Wave body fill below the crest — the face.
        for x in 0..w {
            let curve = ((x as f32 / w as f32) * PI).sin() * (h as f32 * 0.06);
            let top = (crest_y_base as f32 - curve + 1.0) as usize;
            // Face fills from crest down to bottom, slanting right.
            let face_bot = (crest_y_base + (x * (h - crest_y_base) / w.max(1))).min(h - 1);
            if top <= face_bot {
                draw::vline(grid, x, top, face_bot);
            }
        }

        // Surfer position: drops diagonally down the face with eased.
        // x: starts near the crest (right side of wave), moves left as progress.
        let surfer_x = (w as f32 * (1.0 - ctx.eased * 0.8)) as i32;
        // y: starts near top, descends with eased. Pump oscillation from time.
        let pump = (ctx.time * 5.5).sin() * (h as f32 * 0.05);
        let surfer_y = (h as f32 * (0.25 + ctx.eased * 0.55) + pump) as i32;

        // Draw surfer as a small cross / figure.
        draw::dot_i(grid, surfer_x, surfer_y);
        draw::dot_i(grid, surfer_x - 1, surfer_y);
        draw::dot_i(grid, surfer_x + 1, surfer_y);
        draw::dot_i(grid, surfer_x, surfer_y - 1);
        draw::dot_i(grid, surfer_x, surfer_y + 1);

        // Carve trail: a curved wake line left by the board, stored as past positions.
        let trail_steps = 12usize;
        for ti in 1..=trail_steps {
            let past_eased = (ctx.eased - ti as f32 * 0.05).max(0.0);
            let tx = (w as f32 * (1.0 - past_eased * 0.8)) as i32;
            let ty = (h as f32 * (0.25 + past_eased * 0.55)) as i32;
            if ti % 2 == 0 {
                draw::dot_i(grid, tx, ty);
            }
        }

        // Tint the wave face.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Wipeout
// ---------------------------------------------------------------------------
// The surfer tumbles. `eased` drives how far through the wipeout we are.
// Dots scatter outward in a splash pattern; the scatter radius grows with
// eased; `time` spins individual splash particles.

struct Wipeout;
impl ProgressStyle for Wipeout {
    fn name(&self) -> &str {
        "wipeout"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Surfer tumbles into whitewater; splash dots scatter radially with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Impact center: the wipeout occurs at the eased position along the bar.
        let cx = (ctx.eased * w as f32) as i32;
        let cy = (h as f32 * 0.5) as i32;

        // Whitewater: chaotic horizontal bands of dots on either side of impact.
        // Dense near cx, fading left.
        let foam_w = ((ctx.eased * w as f32 * 0.7) as i32).max(1);
        for fx in 0..foam_w {
            let dx = cx - fx;
            // Probability-ish: skip some columns for texture.
            if (dx + fx * 3 + (ctx.time * 8.0) as i32) % 3 == 0 {
                continue;
            }
            let foam_h = (h as i32 / 4 + (ctx.time * 3.0 + fx as f32 * 0.4).sin() as i32).max(1);
            let foam_top = (cy - foam_h / 2).max(0);
            let foam_bot = (cy + foam_h / 2).min(h as i32 - 1);
            draw::dot_i(grid, dx, foam_top);
            draw::dot_i(grid, dx, foam_bot);
            if foam_h > 2 {
                draw::dot_i(grid, dx, cy);
            }
        }

        // Splash particles: dots flying outward from the impact point.
        let n_particles = 16usize;
        let max_r = (w.min(h * 2)) as f32 * 0.45 * ctx.eased;
        for pi in 0..n_particles {
            let base_angle = (pi as f32 / n_particles as f32) * 2.0 * PI;
            // Each particle spirals slightly with time.
            let angle = base_angle + ctx.time * (0.3 + pi as f32 * 0.07);
            // Radius grows with eased, oscillates slightly.
            let r = max_r * (0.5 + 0.5 * ((ctx.time * 2.0 + pi as f32 * 0.5).sin() * 0.3 + 0.7));
            let px = cx + (angle.cos() * r) as i32;
            let py = cy + (angle.sin() * r * 0.5) as i32; // flatten vertically
            draw::dot_i(grid, px, py);
        }

        // Tumbling surfer body — a cluster of dots that rotates with time.
        let body_r = 2.0f32;
        for bi in 0..6usize {
            let ba = ctx.time * 3.0 + bi as f32 * PI / 3.0;
            let bx = cx + (ba.cos() * body_r) as i32;
            let by = cy + (ba.sin() * body_r * 0.6) as i32;
            draw::dot_i(grid, bx, by);
        }

        // Tint: whitewater foam is pale; deeper water is darker.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(1.0 - t * 0.5),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Cutback (S-Turn)
// ---------------------------------------------------------------------------
// A cutback carve draws an S-shaped trajectory across the bar. The S
// unrolls from left to right with eased progress; `time` adds a wave-face
// undulation behind the carved path.

struct Cutback;
impl ProgressStyle for Cutback {
    fn name(&self) -> &str {
        "cutback"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "An S-turn cutback carve unrolls across the bar with eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Draw the S-curve: a sine wave with frequency ~1.5 cycles across the bar.
        // Amplitude = center half of the grid height.
        let amp = h as f32 * 0.38;
        let mid = h as f32 * 0.5;
        let revealed_w = (ctx.eased * w as f32).round() as usize;

        for x in 0..revealed_w {
            let t = x as f32 / w.max(1) as f32;
            // S-curve: 1.5 sine periods across the full width.
            let curve_y = mid + (t * PI * 1.5 - PI * 0.5).sin() * amp;
            let iy = curve_y.round() as i32;
            // Draw the path with 3-dot thickness for visibility.
            draw::dot_i(grid, x as i32, iy - 1);
            draw::dot_i(grid, x as i32, iy);
            draw::dot_i(grid, x as i32, iy + 1);
        }

        // Spray kicked out at the leading edge of the carve (the board rail).
        if revealed_w > 0 {
            let lead_x = revealed_w.saturating_sub(1) as i32;
            let t = lead_x as f32 / w.max(1) as f32;
            let lead_y = mid + (t * PI * 1.5 - PI * 0.5).sin() * amp;
            let spray_n = 5usize;
            for si in 0..spray_n {
                let sa = -PI / 4.0 + si as f32 * PI / (2.0 * spray_n.max(1) as f32);
                let sr = 2.0 + (ctx.time * 4.0 + si as f32).sin().abs() * 3.0;
                let sx = lead_x + (sa.cos() * sr) as i32;
                let sy = lead_y as i32 + (sa.sin() * sr * 0.5) as i32;
                draw::dot_i(grid, sx, sy);
            }
        }

        // Undulating wake behind the carve: lighter sine echoes of the path.
        for x in 0..revealed_w.saturating_sub(4) {
            let t = x as f32 / w.max(1) as f32;
            let wake_y = mid + (t * PI * 1.5 - PI * 0.5 + 1.0).sin() * amp * 0.6;
            // Dotted wake (every other column).
            if x % 3 != 0 {
                continue;
            }
            draw::dot_i(grid, x as i32, wake_y.round() as i32);
        }

        // Tint across rows.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(0.3 + t * 0.7),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Offshore Spray
// ---------------------------------------------------------------------------
// Wind blows from the shore (left), lofting spray off wave crests to the
// right. A crest ridge runs across the upper third; spray dots arc rightward
// off it. `time` drives the spray launch; `eased` thickens the crest/spray.

struct OffshoreSpray;
impl ProgressStyle for OffshoreSpray {
    fn name(&self) -> &str {
        "offshore-spray"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Offshore wind lofts spray dots off the wave crest; spray density grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Crest: a sine-textured ridge in the upper third.
        let crest_base = (h as f32 * 0.30) as usize;
        for x in 0..w {
            let wobble =
                ((x as f32 * 0.3 - ctx.time * 2.0).sin() * (h as f32 * 0.06)).round() as i32;
            let cy = crest_base as i32 + wobble;
            draw::dot_i(grid, x as i32, cy);
            // Crest lip is thicker with progress.
            if ctx.eased > 0.3 {
                draw::dot_i(grid, x as i32, cy - 1);
            }
        }

        // Wave body below the crest (progress controls how much is drawn).
        let body_h = (ctx.eased * (h - crest_base) as f32) as usize;
        if body_h > 0 {
            draw::fill_rect(
                grid,
                0,
                crest_base + 1,
                w,
                body_h.min(h.saturating_sub(crest_base + 1)),
            );
        }

        // Spray particles: born at crest, blown rightward and upward.
        let n_spray = ((ctx.eased * 20.0) as usize).min(24);
        for si in 0..n_spray {
            // Birth column spread across the crest.
            let birth_x = (si * w) / n_spray.max(1);
            let birth_wobble =
                ((birth_x as f32 * 0.3 - ctx.time * 2.0).sin() * (h as f32 * 0.06)).round() as i32;
            let birth_y = crest_base as i32 + birth_wobble;

            // Phase within spray lifetime (0=new at crest, 1=max height/distance).
            let phase = (ctx.time * 1.8 + si as f32 * 0.4).fract();

            // Spray arcs rightward and upward, then dissipates.
            let fly_x = (birth_x as f32 + phase * 8.0 * (1.0 + si as f32 * 0.3)) as i32;
            let fly_y = birth_y - (phase * (h as f32 * 0.35) * ((1.0 - phase) * 4.0)) as i32;

            draw::dot_i(grid, fly_x, fly_y);
        }

        // Palette tint: sky above crest cooler, water below warmer.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Longboard Noseride
// ---------------------------------------------------------------------------
// The board tilts as the surfer walks to the nose. `eased` controls tilt
// angle (0 = flat, 1 = nose down). `time` rocks the board with the wave.
// Drawn as a long plank outline + a stick figure moving toward the front.

struct NoseRide;
impl ProgressStyle for NoseRide {
    fn name(&self) -> &str {
        "noseride"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Longboard tilts as surfer walks to the nose; tilt angle tracks progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid_y = (h as f32 * 0.5) as i32;

        // Board: a long horizontal line that tilts with eased and rocks with time.
        let board_l = (w as f32 * 0.85) as i32;
        let board_start_x = ((w as f32 * 0.07) as i32).max(0);

        // Tilt: nose (left end) dips down, tail (right) rises. Max tilt = h/4 dots.
        let max_tilt = (h as f32 * 0.28) as i32;
        let tilt = (ctx.eased * max_tilt as f32) as i32;
        // Rock: a smaller time-driven oscillation on top of the tilt.
        let rock = (ctx.time * 2.3).sin() * (h as f32 * 0.04);

        // Draw board as a line from (board_start_x, nose_y) to (board_start_x+board_l, tail_y).
        let nose_y = (mid_y + tilt + rock as i32).clamp(0, h as i32 - 1);
        let tail_y = (mid_y - tilt + rock as i32).clamp(0, h as i32 - 1);
        let steps = board_l.max(1) as usize;
        for si in 0..=steps {
            let t = si as f32 / steps as f32;
            let bx = board_start_x + si as i32;
            let by = nose_y + ((tail_y - nose_y) as f32 * t).round() as i32;
            draw::dot_i(grid, bx, by);
            draw::dot_i(grid, bx, by + 1); // give board 2-dot thickness
        }

        // Surfer walks from tail (right) toward nose (left) with eased.
        let surfer_t = 1.0 - ctx.eased; // 1.0=at tail, 0.0=at nose
        let sx = (board_start_x as f32 + surfer_t * board_l as f32) as i32;
        let tilt_frac = surfer_t;
        let sy = (nose_y as f32 + (tail_y - nose_y) as f32 * tilt_frac) as i32;

        // Stick figure: head, body, arms.
        draw::dot_i(grid, sx, sy - 3); // head
        draw::dot_i(grid, sx, sy - 2); // neck
        draw::dot_i(grid, sx, sy - 1); // torso top
        draw::dot_i(grid, sx - 1, sy - 1); // left arm
        draw::dot_i(grid, sx + 1, sy - 1); // right arm

        // Wave texture below the board: rolling dots.
        for x in 0..w {
            let wave_y = (h as f32 * 0.75
                + (x as f32 * 0.25 - ctx.time * 3.0).sin() * (h as f32 * 0.08))
                as i32;
            draw::dot_i(grid, x as i32, wave_y);
        }

        // Tint.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(0.2 + t * 0.8),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Aerial Launch
// ---------------------------------------------------------------------------
// The surfer launches off the lip and traces a parabolic arc above the wave.
// `eased` controls how far along the parabola the board is (0=lip, 1=landing).
// `time` adds a rotation to the board silhouette.

struct AerialLaunch;
impl ProgressStyle for AerialLaunch {
    fn name(&self) -> &str {
        "aerial-launch"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Surfer launches off the lip and traces a parabola; arc tracks progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Launch point: lower-left area (the wave lip).
        let launch_x = (w as f32 * 0.15) as i32;
        let launch_y = (h as f32 * 0.70) as i32;

        // Landing point: lower-right area (back on the wave face).
        let land_x = (w as f32 * 0.85) as i32;
        let land_y = (h as f32 * 0.60) as i32;

        // Parabola peak: centered horizontally, near the top of the grid.
        let peak_x = (launch_x + land_x) / 2;
        let peak_y = (h as f32 * 0.10) as i32;

        // Draw the full arc (faint dots for the trajectory path).
        let arc_steps = 30usize;
        for ai in 0..arc_steps {
            let t = ai as f32 / arc_steps.saturating_sub(1).max(1) as f32;
            // Quadratic bezier: P = (1-t)^2*A + 2t(1-t)*B + t^2*C
            let ax = ((1.0 - t) * (1.0 - t) * launch_x as f32
                + 2.0 * t * (1.0 - t) * peak_x as f32
                + t * t * land_x as f32) as i32;
            let ay = ((1.0 - t) * (1.0 - t) * launch_y as f32
                + 2.0 * t * (1.0 - t) * peak_y as f32
                + t * t * land_y as f32) as i32;
            // Only draw the revealed portion of the arc.
            if t <= ctx.eased {
                draw::dot_i(grid, ax, ay);
            }
        }

        // Current surfer position on the arc.
        let et = ctx.eased;
        let cur_x = ((1.0 - et) * (1.0 - et) * launch_x as f32
            + 2.0 * et * (1.0 - et) * peak_x as f32
            + et * et * land_x as f32) as i32;
        let cur_y = ((1.0 - et) * (1.0 - et) * launch_y as f32
            + 2.0 * et * (1.0 - et) * peak_y as f32
            + et * et * land_y as f32) as i32;

        // Board: a short line rotated with time (board spins in the air).
        let rot = ctx.time * 3.5;
        let board_len = 4i32;
        for bl in -board_len..=board_len {
            let bx = cur_x + (rot.cos() * bl as f32) as i32;
            let by = cur_y + (rot.sin() * bl as f32 * 0.5) as i32;
            draw::dot_i(grid, bx, by);
        }

        // Wave lip and face below the launch point.
        for x in 0..launch_x as usize + 4 {
            let lip_y = (h as f32 * 0.65
                + (x as f32 * 0.4 - ctx.time * 2.5).sin() * (h as f32 * 0.07))
                as i32;
            draw::dot_i(grid, x as i32, lip_y);
        }
        // Wave face fill left of launch.
        if launch_x > 1 {
            draw::fill_rect(
                grid,
                0,
                launch_y as usize,
                launch_x as usize,
                (h - launch_y as usize).min(h),
            );
        }

        // Tint.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t * 0.9),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Tide Line
// ---------------------------------------------------------------------------
// The tide advance line creeps up a beach from right to left. The "wet sand"
// zone (filled) grows leftward with eased. The line itself has a wave-edge
// driven by time. Completely different from ocean's RisingTide (that is
// vertical water-level; this is a horizontal shoreline sweeping the beach).

struct TideLine;
impl ProgressStyle for TideLine {
    fn name(&self) -> &str {
        "tide-line"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Tide advance line sweeps across the beach; wet-sand zone grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // The tide line is vertical but positioned horizontally across the grid.
        // eased controls how far left it has advanced.
        let tide_x = (w as f32 * (1.0 - ctx.eased)) as usize;

        // Wet-sand fill: right portion (where tide has reached).
        // Use shading glyphs for the wet zone, progressing denser near the tide line.
        let (cells_w, cells_h) = grid.dimensions();
        let tide_cell_x = (ctx.eased * cells_w as f32) as usize;
        for cy_c in 0..cells_h {
            for cx_c in tide_cell_x..cells_w {
                // Density increases as we get further from the tide line.
                let dist_frac = (cx_c - tide_cell_x) as f32 / (cells_w - tide_cell_x).max(1) as f32;
                let density = (dist_frac * 3.0) as usize;
                draw::shade(grid, cx_c, cy_c, density.min(3));
            }
        }

        // The tide line edge: a wavy vertical column of dots.
        let wave_amp = (h as f32 * 0.08).max(1.0);
        for y in 0..h {
            let wobble = ((y as f32 * 0.4 + ctx.time * 2.8).sin() * wave_amp).round() as i32;
            let lx = tide_x as i32 + wobble;
            draw::dot_i(grid, lx, y as i32);
            draw::dot_i(grid, lx - 1, y as i32); // double width
        }

        // Foam: a few dots scattered just to the left of the tide line.
        let foam_zone = ((h as f32 * 0.3) as usize).max(1);
        for fi in 0..foam_zone {
            let fx_base =
                tide_x as i32 - 2 - ((ctx.time * 2.0 + fi as f32 * 0.7).fract() * 5.0) as i32;
            let fy = (fi * h / foam_zone.max(1)) as i32;
            draw::dot_i(grid, fx_base, fy);
        }

        // Tint: dry sand = warm palette, wet = cooler.
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            // Left of tide line = wet (start color), right = dry (end color).
            if tide_cell_x > 0 {
                draw::tint_row(
                    grid,
                    cy_c,
                    0,
                    tide_cell_x.saturating_sub(1),
                    ctx.palette.sample(0.1 + t * 0.3),
                );
            }
            if tide_cell_x < cells_w {
                draw::tint_row(
                    grid,
                    cy_c,
                    tide_cell_x,
                    cells_w.saturating_sub(1),
                    ctx.palette.sample(0.7 + t * 0.3),
                );
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Palm-Tree-and-Sunset Silhouette
// ---------------------------------------------------------------------------
// A palm silhouette on the left, sun descending on the right. `eased`
// controls how far the sun has sunk (0=high, 1=below horizon). `time`
// sways the palm fronds. Purely line-art, no fill — structure is tall
// vertical shapes versus a shrinking circular arc.

struct PalmSunset;
impl ProgressStyle for PalmSunset {
    fn name(&self) -> &str {
        "palm-sunset"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Palm tree silhouette and sinking sun; sun descends below horizon with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Horizon line.
        let horizon_y = (h as f32 * 0.65) as usize;
        draw::hline(grid, 0, w - 1, horizon_y);

        // --- Palm tree (left ~25% of bar) ---
        let trunk_x = (w as f32 * 0.18) as i32;
        let trunk_base_y = (horizon_y as i32).min(h as i32 - 1);
        let trunk_top_y = (h as f32 * 0.10) as i32;
        let trunk_h = (trunk_base_y - trunk_top_y).max(0);

        // Curved trunk: slight S-curve via sine-offset per segment.
        for seg in 0..trunk_h as usize {
            let seg_y = trunk_base_y - seg as i32;
            let lean = ((seg as f32 / trunk_h.max(1) as f32) * PI * 0.5).sin();
            let trunk_sway = (ctx.time * 0.8).sin() * lean * 2.0;
            let seg_x = trunk_x + (lean * 3.0 + trunk_sway) as i32;
            draw::dot_i(grid, seg_x, seg_y);
        }

        // Fronds: 5 arching lines from the top of the trunk.
        let n_fronds = 5usize;
        let frond_len = ((h as f32 * 0.28) as i32).max(3);
        let sway = (ctx.time * 1.1).sin() * 2.0;
        let crown_x = trunk_x + (PI / 2.0 * 3.0).sin() as i32 + sway as i32;
        let crown_y = trunk_top_y;

        for fi in 0..n_fronds {
            // Angles spread from ~-40° to ~+160° (arching upward and to the sides).
            let base_angle = -PI * 0.22 + fi as f32 * PI * 0.36;
            let droop = (ctx.time * 0.9 + fi as f32 * 0.5).sin() * 0.08;
            let angle = base_angle + droop;
            for fl in 1..=frond_len {
                let fx = crown_x + (angle.cos() * fl as f32 * 1.1) as i32;
                let fy = crown_y + (angle.sin() * fl as f32 * 0.8) as i32;
                // Draw dots with slight gaps for a frond look.
                if fl % 3 != 0 {
                    draw::dot_i(grid, fx, fy);
                }
                // Leaflets off the frond spine.
                if fl % 4 == 2 {
                    let perp_x = (angle + PI / 2.0).cos();
                    let perp_y = (angle + PI / 2.0).sin();
                    draw::dot_i(grid, fx + perp_x as i32, fy + perp_y as i32);
                    draw::dot_i(grid, fx - perp_x as i32, fy - perp_y as i32);
                }
            }
        }

        // --- Sun (right ~40% of bar) ---
        let sun_cx = (w as f32 * 0.72) as i32;
        // Sun descends: at eased=0, sun is fully above horizon; at eased=1, fully below.
        // The visible arc shrinks as it dips below the horizon line.
        let sun_r = ((h as f32 * 0.20).round() as i32).max(3);
        // Center y: sun rises from below horizon_y to above, driven by eased.
        // eased=0 → sun center well above horizon. eased=1 → center at or below horizon.
        let sun_cy = (horizon_y as f32 - sun_r as f32 * (1.0 - ctx.eased * 1.2)) as i32;

        // Draw only the arc ABOVE the horizon.
        let sun_steps = ((sun_r * 8) as usize).max(12);
        for si in 0..sun_steps {
            let angle = si as f32 / sun_steps as f32 * 2.0 * PI;
            let ax = sun_cx + (angle.cos() * sun_r as f32) as i32;
            let ay = sun_cy + (angle.sin() * sun_r as f32 * 0.8) as i32;
            if (ay as usize) < horizon_y {
                draw::dot_i(grid, ax, ay);
            }
        }

        // Sun rays: short spokes above horizon only.
        let n_rays = 8usize;
        let ray_len = sun_r / 2 + 1;
        for ri in 0..n_rays {
            let ra = ri as f32 / n_rays as f32 * 2.0 * PI + ctx.time * 0.3;
            for rl in 1..=ray_len {
                let rx = sun_cx + (ra.cos() * (sun_r + rl) as f32) as i32;
                let ry = sun_cy + (ra.sin() * (sun_r + rl) as f32 * 0.8) as i32;
                if (ry as usize) < horizon_y && rl % 2 == 0 {
                    draw::dot_i(grid, rx, ry);
                }
            }
        }

        // Ocean reflection below horizon: shimmering horizontal dashes.
        for ref_y in horizon_y + 1..h {
            let shimmer_x = (sun_cx as f32
                + ((ref_y as f32 * 0.5 + ctx.time * 3.0).sin() * sun_r as f32 * 0.7))
                as i32;
            let ref_width = (sun_r as f32
                * (1.0 - (ref_y - horizon_y) as f32 / (h - horizon_y).max(1) as f32))
                as i32;
            if ref_width > 0 {
                draw::hline(
                    grid,
                    (shimmer_x - ref_width).max(0) as usize,
                    (shimmer_x + ref_width).min(w as i32 - 1) as usize,
                    ref_y,
                );
            }
        }

        // Tint: warm sunset gradient.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(t),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Wave Sets
// ---------------------------------------------------------------------------
// Surfing arrives in "sets" — groups of waves. This bar shows discrete wave
// groups (sets) counting across the grid. Each set is a cluster of 2–3 wave
// crests rendered as a block group; `eased` controls how many sets have
// arrived; `time` rolls each set leftward (they march toward shore).

struct WaveSets;
impl ProgressStyle for WaveSets {
    fn name(&self) -> &str {
        "wave-sets"
    }
    fn theme(&self) -> &str {
        "surf"
    }
    fn describe(&self) -> &str {
        "Wave sets (groups of 3 crests) march toward shore; set count grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // How many sets appear at most.
        let max_sets = 5usize;
        // Each set takes up a portion of the bar width (with a gap between sets).
        let set_zone = w / max_sets.max(1);
        let set_w = (set_zone * 3 / 4).max(2); // each set occupies 3/4 of its zone
                                               // Waves-per-set: 3 crests spaced within the set.
        let waves_per_set = 3usize;

        let active_sets = ((ctx.eased * max_sets as f32).ceil() as usize).min(max_sets);

        for si in 0..active_sets {
            // Reveal fraction for this set (last set may be partially revealed).
            let set_reveal = {
                let needed = (si + 1) as f32 / max_sets as f32;
                ((ctx.eased - needed + 1.0 / max_sets as f32) * max_sets as f32).clamp(0.0, 1.0)
            };

            // Sets march from right to left: base_x shifts leftward with time.
            let base_x = si * set_zone;
            // Scroll each set leftward continuously with time.
            let scroll = ((ctx.time * 8.0 + si as f32 * 3.1) % set_zone as f32) as usize;
            let sx_base = base_x.saturating_sub(scroll);

            for wi in 0..waves_per_set {
                // Only reveal waves proportionally within this set.
                if wi as f32 / waves_per_set as f32 > set_reveal {
                    continue;
                }

                // Space the 3 crests evenly within set_w.
                let crest_x = sx_base + (wi * set_w) / waves_per_set.max(1);

                // Crest height: middle wave is tallest (natural swell shape).
                let height_scale = match wi {
                    0 => 0.6f32,
                    1 => 1.0,
                    _ => 0.75,
                };
                let crest_h = (h as f32 * 0.35 * height_scale) as usize;

                // Draw this crest: a gentle sine peak shape.
                let crest_width = (set_w / waves_per_set).max(2);
                for dx in 0..crest_width {
                    let px = crest_x + dx;
                    if px >= w {
                        continue;
                    }
                    let t_dx = dx as f32 / crest_width.max(1) as f32;
                    // Sine envelope peaks in the center of the crest.
                    let env = (t_dx * PI).sin();
                    let col_h = (env * crest_h as f32) as usize;
                    let crest_top_y = (h as f32 * 0.30) as usize;
                    // Draw the crest as a vertical column from crest top down.
                    for dy in 0..col_h.min(h - crest_top_y) {
                        draw::dot(grid, px, crest_top_y + dy);
                    }
                    // Time-driven wave phase rolls each crest.
                    let roll_y =
                        crest_top_y as i32 + (dx as f32 * 0.3 - ctx.time * 3.5).sin() as i32;
                    draw::dot_i(grid, px as i32, roll_y);
                }
            }
        }

        // Baseline: the flat water between sets (a thin line across the lower portion).
        let baseline_y = (h as f32 * 0.68) as usize;
        draw::hline(grid, 0, w - 1, baseline_y);

        // Tint per row.
        let (_, cells_h) = grid.dimensions();
        for cy_c in 0..cells_h {
            let t = cy_c as f32 / cells_h.max(1) as f32;
            draw::tint_row(
                grid,
                cy_c,
                0,
                ctx.width.saturating_sub(1),
                ctx.palette.sample(0.2 + t * 0.8),
            );
        }

        Ok(())
    }
}
