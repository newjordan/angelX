//! Music-themed progress bars for dotmax.
//!
//! Every style in this theme derives its visual structure from a distinct
//! musical object or phenomenon — not just a colour change.
//!
//! - [`MusicalStaff`]     — 5-line staff with a treble clef; notes appear L→R
//! - [`PianoKeyboard`]    — white/black keys; keys depress as a melody plays
//! - [`Metronome`]        — pendulum arm swinging; tempo driven by eased
//! - [`VinylRecord`]      — spinning disc with grooves and an inward tonearm
//! - [`DrumKit`]          — radial impact rings pulsing on each beat
//! - [`SoundWaveform`]    — centered oscillating amplitude trace
//! - [`CassetteReels`]    — two reels turning; tape transfers as progress grows
//! - [`ConductorBaton`]   — baton tracing a 4/4 conducting pattern
//! - [`SheetScroll`]      — sheet music scrolling left; bar-lines and note heads
//! - [`TuningFork`]       — fork tines vibrating with decaying amplitude
//! - [`VolumeKnob`]       — rotating potentiometer knob with a sweep arc
//! - [`BeatPulse`]        — concentric rings expanding on each beat

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Theme tint — stage magenta into cyan. Applied to styles that draw monochrome.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(255, 104, 196);
const TINT_END: Color = Color::rgb(84, 204, 255);

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

/// All styles in the `music` theme.
///
/// Returns 12 structurally distinct bars, each modelling a different concept
/// from the world of music — notation, instruments, rhythm, and acoustics.
/// Safe to render at any size from 1×1 upward.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(MusicalStaff)),
        Box::new(PianoKeyboard),
        Box::new(Tinted(Metronome)),
        Box::new(Tinted(VinylRecord)),
        Box::new(Tinted(DrumKit)),
        Box::new(Tinted(SoundWaveform)),
        Box::new(Tinted(CassetteReels)),
        Box::new(Tinted(ConductorBaton)),
        Box::new(Tinted(SheetScroll)),
        Box::new(Tinted(TuningFork)),
        Box::new(Tinted(VolumeKnob)),
        Box::new(BeatPulse),
    ]
}

// ---------------------------------------------------------------------------
// 1. Musical Staff
//    Five horizontal staff lines span the full width.  A treble-clef glyph
//    sits in cell 0.  As eased grows, note glyphs (♪ / ♫) appear left-to-right
//    on alternating lines; ledger dots fill the remainder.
// ---------------------------------------------------------------------------
struct MusicalStaff;
impl ProgressStyle for MusicalStaff {
    fn name(&self) -> &str {
        "musical-staff"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "5-line musical staff with treble clef; notes materialize left-to-right with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Draw 5 staff lines (or fewer if height is too small).
        let line_count = 5usize.min(dh);
        let spacing = dh / (line_count + 1);
        let mut staff_ys = Vec::with_capacity(line_count);
        for i in 0..line_count {
            let y = spacing * (i + 1);
            if y < dh {
                draw::hline(grid, 0, dw - 1, y);
                staff_ys.push(y);
            }
        }

        // Treble clef in cell column 0 (only if height allows a glyph).
        if ch >= 1 && cw >= 1 {
            draw::glyph(grid, 0, ch / 2, '𝄞');
        }

        // Notes: place them left-to-right across columns 1..cw.
        let note_cols = cw.saturating_sub(1);
        if note_cols == 0 || staff_ys.is_empty() {
            return Ok(());
        }
        let notes_shown = (ctx.eased * note_cols as f32).round() as usize;

        let note_glyphs = ['♪', '♫', '♩', '♪', '♫'];
        for i in 0..notes_shown.min(note_cols) {
            let cx = i + 1;
            if cx >= cw {
                break;
            }
            // Pick which staff line this note sits on.
            let line_idx = i % staff_ys.len();
            let dot_y = staff_ys[line_idx];
            // Glyphs occupy a full cell — pick the cell row for this dot_y.
            let cy = (dot_y / 4).min(ch.saturating_sub(1));
            let g = note_glyphs[i % note_glyphs.len()];
            draw::glyph(grid, cx, cy, g);
        }

        // Animate: a cursor dot walks right at current position.
        let cursor_x = (ctx.eased * dw as f32) as usize;
        if !staff_ys.is_empty() {
            let t = (ctx.time * 4.0).sin() * 0.5 + 0.5;
            let line_idx = (t * staff_ys.len() as f32) as usize;
            let cy = staff_ys[line_idx.min(staff_ys.len() - 1)];
            draw::dot(grid, cursor_x.min(dw - 1), cy);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Piano Keyboard
//    White and black keys fill the width.  As time advances a "melody" index
//    cycles through notes; the key at the melody position depresses (shown as
//    a filled cell).  Progress lights filled keys cumulatively from the left.
// ---------------------------------------------------------------------------
struct PianoKeyboard;
impl ProgressStyle for PianoKeyboard {
    fn name(&self) -> &str {
        "piano-keyboard"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Piano keys across the full width; black keys overlay white; melody note depresses as time ticks"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // A piano octave has 7 white keys and 5 black keys (pattern W W B W W W B …).
        // We map each cell column to a key in the repeating octave pattern.
        // Black-key positions within an octave (0-indexed white keys): after 1,2,4,5,6
        // i.e. the black key appears between white keys 1-2, 2-3, 4-5, 5-6, 6-7.
        // Simplified: black keys at columns that map to octave positions 1,2,4,5,6 (mod 7).
        let black_offsets: [usize; 5] = [1, 2, 4, 5, 6]; // which white-key positions have a black above

        let filled_cols = (ctx.eased * cw as f32).round() as usize;

        // Beat-driven depressed key.
        let beat_period = 0.5_f32; // 120 BPM
        let beat_phase = (ctx.time / beat_period).floor() as usize;
        let melody = [0usize, 2, 4, 5, 7, 9, 11, 9, 7, 5, 4, 2];
        let melody_note = melody[beat_phase % melody.len()];

        for cx in 0..cw {
            let octave_pos = cx % 7;
            let is_black = black_offsets.contains(&octave_pos);
            let is_filled = cx < filled_cols;
            // Highlight the currently "played" key.
            let note_col = melody_note % 12;
            let white_equiv = [0usize, 2, 4, 5, 7, 9, 11]; // maps white index → semitone
            let is_playing = white_equiv[octave_pos] == note_col && !is_black;

            if is_black {
                // Black key: upper half filled; draw as a block in the top cell rows.
                let black_h = (dh / 2).max(1);
                draw::fill_rect(grid, cx * 2, 0, 2, black_h);
            } else if is_playing {
                // Depressed white key: filled solid.
                draw::fill_rect(grid, cx * 2, 0, 2, dh);
            } else if is_filled {
                // Filled (played so far): shade mark at bottom.
                let mark_y = dh.saturating_sub(2);
                draw::hline(grid, cx * 2, cx * 2 + 1, mark_y);
                draw::hline(grid, cx * 2, cx * 2 + 1, mark_y + 1);
            } else {
                // Unfilled white key: just vertical dividing line on left edge.
                draw::vline(grid, cx * 2, 0, dh - 1);
            }
        }

        // Tint filled cells.
        let _ = ch;
        for cy in 0..ch {
            draw::tint_row(
                grid,
                cy,
                0,
                filled_cols.min(cw).saturating_sub(1),
                ctx.palette.sample(ctx.eased),
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Metronome
//    A triangular pendulum arm pivots from a central pivot point near the top.
//    Its swing arc and tempo are driven by eased (more progress = faster beats).
//    The arm is drawn as a dot-traced line at an angle animated by time.
// ---------------------------------------------------------------------------
struct Metronome;
impl ProgressStyle for Metronome {
    fn name(&self) -> &str {
        "metronome"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Pendulum metronome arm swinging; beat tempo rises with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Pivot near top-centre.
        let px = (dw / 2) as i32;
        let py = (dh / 6) as i32;

        // Arm length: most of the height.
        let arm_len = (dh.saturating_sub(2)) as f32;

        // Tempo: slow at 0 progress (40 BPM ≈ 1.5 s/beat), fast at 1.0 (208 BPM ≈ 0.29 s/beat).
        let period = 1.5 - ctx.eased * 1.2; // seconds for a half-swing
        let period = period.max(0.15);

        // Swing angle: max ±30° at rest, narrower at top speed (visual choice).
        let max_angle = PI / 6.0; // 30 degrees
        let angle = (ctx.time / period * PI).sin() * max_angle;

        // Draw arm from pivot down at angle.
        let steps = arm_len.ceil() as usize;
        for s in 0..=steps {
            let frac = s as f32 / steps.max(1) as f32;
            let dx = (angle.sin() * frac * arm_len).round() as i32;
            let dy = (angle.cos() * frac * arm_len).round() as i32;
            draw::dot_i(grid, px + dx, py + dy);
        }

        // Bob (weight) at the tip — a small filled diamond shape.
        let tip_x = px + (angle.sin() * arm_len).round() as i32;
        let tip_y = py + (angle.cos() * arm_len).round() as i32;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx.abs() + dy.abs() <= 1 {
                    draw::dot_i(grid, tip_x + dx, tip_y + dy);
                }
            }
        }

        // Tick mark: flash a dot near the top at each extreme.
        let at_extreme = angle.abs() > max_angle * 0.85;
        if at_extreme {
            draw::dot_i(
                grid,
                px + (angle.signum() as i32) * ((dw as i32 / 4).min(6)),
                py,
            );
        }

        // Progress indicator: shade the base of the metronome.
        let filled_w = (ctx.eased * dw as f32).round() as usize;
        let base_y = (dh - 1) as usize;
        draw::hline(grid, 0, filled_w.min(dw - 1), base_y);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Vinyl Record
//    A spinning disc fills most of the height.  Concentric rings represent
//    grooves.  A straight tonearm enters from the right edge and tracks inward
//    as progress grows (from outer to inner groove).
// ---------------------------------------------------------------------------
struct VinylRecord;
impl ProgressStyle for VinylRecord {
    fn name(&self) -> &str {
        "vinyl-record"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Spinning vinyl disc with groove rings; tonearm tracks inward with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Centre the disc — it's circular so use the smaller of width/height.
        let cx = (dw / 2) as i32;
        let cy = (dh / 2) as i32;
        let max_r = ((dh / 2).min(dw / 2)) as i32;
        if max_r == 0 {
            return Ok(());
        }

        // Draw concentric groove rings.
        let groove_count = (max_r / 2).max(1) as usize;
        for g in 0..groove_count {
            let r = (g + 1) as i32 * (max_r / groove_count.max(1) as i32).max(1);
            if r > max_r {
                break;
            }
            // Approximate a circle with dot points.
            let steps = (2.0 * PI * r as f32).ceil() as usize;
            let steps = steps.max(4);
            for s in 0..steps {
                let theta = 2.0 * PI * s as f32 / steps as f32 + ctx.time * 2.0;
                let dx = (r as f32 * theta.cos()).round() as i32;
                let dy = (r as f32 * theta.sin()).round() as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Label hole in the centre (solid small disc, non-spinning).
        let hole_r = (max_r / 5).max(1);
        for dy in -hole_r..=hole_r {
            for dx in -hole_r..=hole_r {
                if dx * dx + dy * dy <= hole_r * hole_r {
                    draw::dot_i(grid, cx + dx, cy + dy);
                }
            }
        }

        // Tonearm: a diagonal line from the right edge angling toward the disc.
        // Arm pivot is at the far right, midway up.
        let arm_pivot_x = dw as i32 - 1;
        let arm_pivot_y = 0i32;
        // Track position: outer groove when progress=0, inner when progress=1.
        let track_r = max_r - (ctx.eased * (max_r - hole_r - 1) as f32).round() as i32;
        let arm_angle = -PI / 4.0; // ~45° from vertical
        let arm_tip_x = cx + (track_r as f32 * arm_angle.sin()).round() as i32;
        let arm_tip_y = cy - (track_r as f32 * arm_angle.cos()).round() as i32;

        // Draw the arm as a straight line from pivot to tip.
        let dx_total = arm_tip_x - arm_pivot_x;
        let dy_total = arm_tip_y - arm_pivot_y;
        let steps = (dx_total.abs().max(dy_total.abs())).max(1) as usize;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let ax = arm_pivot_x + (dx_total as f32 * t).round() as i32;
            let ay = arm_pivot_y + (dy_total as f32 * t).round() as i32;
            draw::dot_i(grid, ax, ay);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Drum Kit
//    On each beat a radial burst of impact rings expands from the centre.
//    Progress determines how many drum "hits" have occurred (cumulative rings
//    remain as ghost dots); current beat ring animates outward via time.
// ---------------------------------------------------------------------------
struct DrumKit;
impl ProgressStyle for DrumKit {
    fn name(&self) -> &str {
        "drum-kit"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Radial impact rings exploding from centre on each beat; ghost rings accumulate with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx = (dw / 2) as i32;
        let cy = (dh / 2) as i32;
        let max_r = ((dh / 2).min(dw / 2)) as i32;
        if max_r == 0 {
            return Ok(());
        }

        // Beat period 0.5 s (120 BPM quarter notes).
        let beat_period = 0.5_f32;
        let beat_phase = ctx.time / beat_period;
        let beat_frac = beat_phase.fract(); // 0..1 within current beat

        // Ghost rings from past hits — one ghost per progress segment.
        let ghost_count = (ctx.eased * max_r as f32).round() as usize;
        for g in 0..ghost_count.min(max_r as usize) {
            let r = (g + 1) as i32;
            // Draw sparse ghost ring (every-other dot).
            let steps = (2.0 * PI * r as f32).ceil() as usize;
            for s in (0..steps).step_by(2) {
                let theta = 2.0 * PI * s as f32 / steps.max(1) as f32;
                let dx = (r as f32 * theta.cos()).round() as i32;
                let dy = (r as f32 * theta.sin()).round() as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Live expanding ring for the current beat.
        let live_r = (beat_frac * max_r as f32).round() as i32;
        if live_r > 0 {
            let steps = (2.0 * PI * live_r as f32).ceil() as usize;
            for s in 0..steps {
                let theta = 2.0 * PI * s as f32 / steps.max(1) as f32;
                let dx = (live_r as f32 * theta.cos()).round() as i32;
                let dy = (live_r as f32 * theta.sin()).round() as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Drumstick dot: a brief flash at the centre at beat onset.
        if beat_frac < 0.1 {
            draw::dot_i(grid, cx, cy);
            draw::dot_i(grid, cx + 1, cy);
            draw::dot_i(grid, cx - 1, cy);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Sound Waveform
//    A continuous amplitude trace oscillates around the vertical centre.
//    Amplitude envelope is `eased` (silent at 0, full-scale at 1).
//    The waveform morphs from sine → sawtooth as time advances.
// ---------------------------------------------------------------------------
struct SoundWaveform;
impl ProgressStyle for SoundWaveform {
    fn name(&self) -> &str {
        "sound-waveform"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Centered oscillating waveform; amplitude grows with progress, morph sine→saw with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cy = (dh / 2) as i32;
        let half_h = (dh / 2) as f32;
        let amp = ctx.eased * (half_h - 1.0).max(0.0);

        // Morph parameter: slowly shifts between sine and sawtooth.
        let morph = ((ctx.time * 0.1).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        // Frequency: ~3 cycles across the bar.
        let freq = 3.0_f32;

        let prev_y: Option<i32> = None;
        let _ = prev_y;

        let mut last_y: Option<i32> = None;
        for xi in 0..dw {
            let xn = xi as f32 / dw.max(1) as f32;
            let phase = xn * freq * 2.0 * PI - ctx.time * 2.0;
            // Sine component.
            let sine = phase.sin();
            // Sawtooth component: 2*(phase/(2π) - floor(phase/(2π)+0.5)).
            let saw_phase = phase / (2.0 * PI);
            let saw = 2.0 * (saw_phase - (saw_phase + 0.5).floor());
            let sample = sine * (1.0 - morph) + saw * morph;
            let dot_y = cy - (sample * amp).round() as i32;

            // Connect previous and current with a vertical run.
            if let Some(ly) = last_y {
                let (lo, hi) = if ly <= dot_y {
                    (ly, dot_y)
                } else {
                    (dot_y, ly)
                };
                for y in lo..=hi {
                    draw::dot_i(grid, xi as i32, y);
                }
            } else {
                draw::dot_i(grid, xi as i32, dot_y);
            }
            last_y = Some(dot_y);
        }

        // Centre-line when amplitude is zero.
        if ctx.eased < 0.01 {
            draw::hline(grid, 0, dw - 1, cy as usize);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Cassette Reels
//    Two circular reels; as progress grows the left (supply) reel shrinks and
//    the right (take-up) reel grows.  Both spin at the same angular velocity.
//    A capstan and pinch-roller gap sit between them.
// ---------------------------------------------------------------------------
struct CassetteReels;
impl ProgressStyle for CassetteReels {
    fn name(&self) -> &str {
        "cassette-reels"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Two cassette reels: supply shrinks, take-up grows, both spin as tape transfers"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cy = (dh / 2) as i32;
        let max_r = ((dh / 2).max(1) - 1) as i32;
        let hub_r = (max_r / 3).max(1);

        // Two reel centres.
        let left_cx = (dw / 4) as i32;
        let right_cx = (dw * 3 / 4) as i32;

        // Supply (left) reel shrinks from max_r → hub_r as eased → 1.
        let supply_r = max_r - ((ctx.eased * (max_r - hub_r) as f32).round() as i32);
        let supply_r = supply_r.clamp(hub_r, max_r);
        // Take-up (right) reel grows from hub_r → max_r as eased → 1.
        let takeup_r = hub_r + ((ctx.eased * (max_r - hub_r) as f32).round() as i32);
        let takeup_r = takeup_r.clamp(hub_r, max_r);

        let spin = ctx.time * 3.0; // radians/second

        // Draw a reel: outer ring + hub + spokes.
        let draw_reel =
            |grid: &mut BrailleGrid, rcx: i32, rcy: i32, outer: i32, hub: i32, angle: f32| {
                // Outer ring.
                let steps = (2.0 * PI * outer as f32).ceil() as usize;
                for s in 0..steps {
                    let theta = 2.0 * PI * s as f32 / steps.max(1) as f32;
                    let dx = (outer as f32 * theta.cos()).round() as i32;
                    let dy = (outer as f32 * theta.sin()).round() as i32;
                    draw::dot_i(grid, rcx + dx, rcy + dy);
                }
                // Hub (solid small circle).
                for dy in -hub..=hub {
                    for dx in -hub..=hub {
                        if dx * dx + dy * dy <= hub * hub {
                            draw::dot_i(grid, rcx + dx, rcy + dy);
                        }
                    }
                }
                // 3 spokes.
                for spoke in 0..3 {
                    let theta = angle + spoke as f32 * 2.0 * PI / 3.0;
                    let len = outer - hub;
                    let spoke_steps = len.max(1) as usize;
                    for s in 1..=spoke_steps {
                        let frac = s as f32 / spoke_steps as f32;
                        let r = hub as f32 + frac * len as f32;
                        let dx = (r * theta.cos()).round() as i32;
                        let dy = (r * theta.sin()).round() as i32;
                        draw::dot_i(grid, rcx + dx, rcy + dy);
                    }
                }
            };

        draw_reel(grid, left_cx, cy, supply_r, hub_r, spin);
        draw_reel(grid, right_cx, cy, takeup_r, hub_r, -spin);

        // Tape guide: two horizontal dots between the reels at the top of reel.
        let tape_y = cy - max_r;
        let gap_x0 = (left_cx + supply_r + 1).min(dw as i32 - 1);
        let gap_x1 = (right_cx - takeup_r - 1).max(0);
        if gap_x0 <= gap_x1 {
            draw::hline(
                grid,
                gap_x0 as usize,
                gap_x1 as usize,
                tape_y.max(0) as usize,
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Conductor's Baton
//    The baton tip traces a stylised 4/4 conducting pattern:
//    beat 1 → down, beat 2 → left, beat 3 → right, beat 4 → up.
//    Progress determines how far through a full measure we are (0=start, 1=end).
// ---------------------------------------------------------------------------
struct ConductorBaton;
impl ProgressStyle for ConductorBaton {
    fn name(&self) -> &str {
        "conductor-baton"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Baton tracing a 4/4 conducting pattern; beat positions illuminate with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // 4/4 pattern beat waypoints (normalised 0..1 for both axes).
        // Standard 4/4: 1=centre-top, 2=bottom, 3=right, 4=left.
        let waypoints: [(f32, f32); 5] = [
            (0.5, 0.0),  // beat 1: top centre
            (0.5, 1.0),  // beat 2: bottom
            (0.75, 0.5), // beat 3: right mid
            (0.25, 0.5), // beat 4: left mid
            (0.5, 0.0),  // back to beat 1
        ];

        // Current position along the 4-beat pattern, driven by time.
        let beat_period = 0.6_f32; // seconds per beat
        let measure_time = (ctx.time / (beat_period * 4.0)).fract() * 4.0; // 0..4
        let beat_idx = measure_time as usize;
        let beat_frac = measure_time.fract();

        let (ax, ay) = waypoints[beat_idx.min(3)];
        let (bx, by) = waypoints[(beat_idx + 1).min(4)];
        // Smooth interpolation using sine ease.
        let t = (beat_frac * PI / 2.0).sin().powi(2);
        let tip_xn = ax + (bx - ax) * t;
        let tip_yn = ay + (by - ay) * t;

        let tip_x = (tip_xn * (dw - 1) as f32).round() as i32;
        let tip_y = (tip_yn * (dh - 1) as f32).round() as i32;

        // Baton handle: fixed at top-right corner.
        let handle_x = (dw - 1) as i32;
        let handle_y = 0i32;

        // Draw baton shaft from handle to tip.
        let dx = tip_x - handle_x;
        let dy = tip_y - handle_y;
        let steps = (dx.abs().max(dy.abs())).max(1) as usize;
        for s in 0..=steps {
            let ft = s as f32 / steps as f32;
            let px = handle_x + (dx as f32 * ft).round() as i32;
            let py = handle_y + (dy as f32 * ft).round() as i32;
            draw::dot_i(grid, px, py);
        }

        // Mark the beat waypoints as reference dots.
        for &(wx, wy) in &waypoints[..4] {
            let wpx = (wx * (dw - 1) as f32).round() as i32;
            let wpy = (wy * (dh - 1) as f32).round() as i32;
            draw::dot_i(grid, wpx, wpy);
        }

        // Progress indicator: a horizontal fill at the very bottom.
        let filled_w = (ctx.eased * dw as f32).round() as usize;
        draw::hline(
            grid,
            0,
            filled_w.min(dw - 1),
            (dh - 1).min(dh.saturating_sub(1)),
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Sheet Music Scroll
//    A virtual sheet of music scrolls left.  Vertical bar-lines appear at
//    regular cell intervals; note-head dots sit on and between cell rows.
//    Progress determines scroll offset; time keeps it animated.
// ---------------------------------------------------------------------------
struct SheetScroll;
impl ProgressStyle for SheetScroll {
    fn name(&self) -> &str {
        "sheet-scroll"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Sheet music scrolling left; bar-lines and note heads reveal with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Scroll offset in dot-columns, driven by time and progress.
        let scroll_speed = 2.0_f32; // dots per second
        let scroll_offset = (ctx.time * scroll_speed + ctx.eased * dw as f32) as usize;

        // Five staff lines (dot rows, evenly spaced).
        let line_count = 5usize.min(dh);
        let spacing = (dh / (line_count + 1)).max(1);
        let mut staff_ys = Vec::with_capacity(line_count);
        for i in 0..line_count {
            let y = spacing * (i + 1);
            if y < dh {
                draw::hline(grid, 0, dw - 1, y);
                staff_ys.push(y);
            }
        }

        // Bar lines every 8 cells.
        let bar_period = 16usize; // dot-columns per bar
        for xi in 0..dw {
            let world_x = xi + scroll_offset;
            if world_x % bar_period == 0 {
                draw::vline(grid, xi, 0, dh - 1);
            }
        }

        // Note heads: place dots at pre-defined positions in a repeating pattern.
        let note_pattern: &[(usize, usize)] = &[
            (2, 0),
            (6, 1),
            (10, 2),
            (14, 3),
            (18, 4),
            (22, 1),
            (26, 0),
            (30, 2),
            (34, 3),
            (38, 4),
            (3, 0),
            (7, 2),
            (11, 4),
            (15, 1),
            (19, 3),
        ];
        for &(world_col, line_idx) in note_pattern {
            if staff_ys.is_empty() {
                break;
            }
            let line = line_idx % staff_ys.len();
            let dot_y = staff_ys[line];
            // Compute screen x from world position.
            let world_x = world_col;
            // tile the pattern with the period of note_pattern.
            let period_dots = 48usize;
            // find visible column: world_x + k*period_dots - scroll_offset in [0, dw)
            let adjusted_offset = scroll_offset % period_dots;
            let visible_x = if world_x >= adjusted_offset {
                world_x - adjusted_offset
            } else {
                world_x + period_dots - adjusted_offset
            };
            if visible_x < dw {
                // Note head: small filled oval (3 dots wide, 1 tall).
                draw::dot(grid, visible_x, dot_y);
                if visible_x + 1 < dw {
                    draw::dot(grid, visible_x + 1, dot_y);
                }
                if visible_x > 0 {
                    draw::dot(grid, visible_x - 1, dot_y);
                }
                // Stem: upward from note head.
                let stem_top = dot_y.saturating_sub(4);
                for sy in stem_top..dot_y {
                    draw::dot(grid, visible_x + 1, sy);
                }
            }
        }

        // Progress cursor: a bright vertical line at the eased position.
        let cursor_x = (ctx.eased * (dw - 1) as f32).round() as usize;
        draw::vline(grid, cursor_x.min(dw - 1), 0, dh - 1);

        // Glyph note markers in cell columns.
        let _ = cw;
        let _ = ch;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Tuning Fork
//     Two parallel tines vibrate with decaying amplitude.  As progress nears 1
//     the vibration dampens to stillness (perfectly in tune).  Tine tips
//     draw the oscillating dot positions.
// ---------------------------------------------------------------------------
struct TuningFork;
impl ProgressStyle for TuningFork {
    fn name(&self) -> &str {
        "tuning-fork"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Tuning fork tines vibrating; amplitude decays as pitch locks in with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        // Fork body: a vertical handle in the centre-bottom.
        let cx = (dw / 2) as i32;
        let handle_top = (dh * 2 / 3) as i32;
        let handle_bot = (dh - 1) as i32;
        draw::vline(grid, cx as usize, handle_top as usize, handle_bot as usize);

        // Tine separation at the base.
        let tine_base_sep = ((dw as i32) / 6).max(1);
        let tine_len = (handle_top - 1).max(0) as usize;

        // Vibration amplitude: fades to 0 as eased → 1.
        let raw_amp = 1.0 - ctx.eased;
        let _freq = 440.0_f32; // visually representative; 440 Hz A4 reference pitch
        let vis_freq = 4.0; // oscillations per second at screen speed
        let amp = raw_amp * tine_base_sep as f32;

        // Draw left and right tines.
        for row in 0..tine_len {
            // Vibration phase varies along the tine (standing wave).
            let frac = row as f32 / tine_len.max(1) as f32;
            let phase = ctx.time * vis_freq * 2.0 * PI;
            let vibration = (phase + frac * PI).sin() * amp * frac; // antinode at tip
            let vibration = vibration.round() as i32;

            let dy = (handle_top as usize - 1 - row) as i32;
            let lx = cx - tine_base_sep + vibration;
            let rx = cx + tine_base_sep - vibration;
            draw::dot_i(grid, lx, dy);
            draw::dot_i(grid, rx, dy);
        }

        // Tine tips: extra thick dot at the very top.
        let tip_y = 0i32;
        let tip_phase = ctx.time * vis_freq * 2.0 * PI;
        let tip_vib = ((tip_phase + PI).sin() * amp).round() as i32;
        draw::dot_i(grid, cx - tine_base_sep + tip_vib - 1, tip_y);
        draw::dot_i(grid, cx - tine_base_sep + tip_vib, tip_y);
        draw::dot_i(grid, cx + tine_base_sep - tip_vib, tip_y);
        draw::dot_i(grid, cx + tine_base_sep - tip_vib + 1, tip_y);

        // Frequency label via ♩ glyph when close to tuned.
        let (_, ch) = grid.dimensions();
        let (cw, _) = grid.dimensions();
        if ctx.eased > 0.9 && cw > 0 && ch > 0 {
            draw::glyph(grid, 0, ch / 2, '♩');
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Volume Knob
//    A circular potentiometer knob drawn with a sweep arc from min to max.
//    The filled arc represents the current volume (eased).  A notch line
//    shows the knob pointer, rotating from 7 o'clock (0%) to 5 o'clock (100%).
// ---------------------------------------------------------------------------
struct VolumeKnob;
impl ProgressStyle for VolumeKnob {
    fn name(&self) -> &str {
        "volume-knob"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Potentiometer knob with rotating pointer; sweep arc fills from 7 o'clock to 5 o'clock"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx = (dw / 2) as i32;
        let cy = (dh / 2) as i32;
        let r = ((dh / 2).min(dw / 2)) as i32;
        let r = (r - 1).max(1);

        // Knob range: 7 o'clock = 225° from top = 225° in standard math angle.
        // We use angles measured clockwise from top (12 o'clock = 0).
        // 7 o'clock = 210° CW from top; 5 o'clock = 150° CW from top (going the long way).
        // Represent as start_angle=210°, sweep=300° total.
        let start_deg = 210.0_f32;
        let sweep_deg = 300.0_f32;

        // Draw the full arc outline (the "track").
        let arc_steps = 64usize;
        for s in 0..=arc_steps {
            let t = s as f32 / arc_steps as f32;
            let deg = start_deg + t * sweep_deg;
            let rad = deg * PI / 180.0;
            // Clockwise from top: x = sin(rad), y = -cos(rad).
            let dx = (r as f32 * rad.sin()).round() as i32;
            let dy = (-(r as f32 * rad.cos())).round() as i32;
            draw::dot_i(grid, cx + dx, cy + dy);
        }

        // Draw the filled arc (progress).
        let filled_steps = (ctx.eased * arc_steps as f32).round() as usize;
        let inner_r = (r * 2 / 3).max(1);
        for s in 0..=filled_steps.min(arc_steps) {
            let t = s as f32 / arc_steps as f32;
            let deg = start_deg + t * sweep_deg;
            let rad = deg * PI / 180.0;
            for ri in inner_r..=r {
                let dx = (ri as f32 * rad.sin()).round() as i32;
                let dy = (-(ri as f32 * rad.cos())).round() as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Knob body circle (inner).
        for dy in -inner_r..=inner_r {
            for dx in -inner_r..=inner_r {
                let d2 = dx * dx + dy * dy;
                if d2 <= inner_r * inner_r && d2 >= (inner_r / 2) * (inner_r / 2) {
                    draw::dot_i(grid, cx + dx, cy + dy);
                }
            }
        }

        // Pointer line from centre to rim at current angle.
        let pointer_deg = start_deg + ctx.eased * sweep_deg;
        let pointer_rad = pointer_deg * PI / 180.0;
        let steps = r as usize;
        for s in 0..=steps {
            let frac = s as f32 / steps.max(1) as f32;
            let pdx = (r as f32 * pointer_rad.sin() * frac).round() as i32;
            let pdy = (-(r as f32 * pointer_rad.cos()) * frac).round() as i32;
            draw::dot_i(grid, cx + pdx, cy + pdy);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 12. Beat Pulse
//    Concentric circles expand outward from the centre on each beat.
//    Multiple rings are in flight simultaneously, each at a different expansion
//    phase.  Progress determines the overall brightness / ring count.
// ---------------------------------------------------------------------------
struct BeatPulse;
impl ProgressStyle for BeatPulse {
    fn name(&self) -> &str {
        "beat-pulse"
    }
    fn theme(&self) -> &str {
        "music"
    }
    fn describe(&self) -> &str {
        "Concentric rings expanding outward on each beat; ring density grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }

        let cx = (dw / 2) as i32;
        let cy = (dh / 2) as i32;
        let max_r = ((dh / 2).min(dw / 2)) as i32;
        if max_r == 0 {
            return Ok(());
        }

        // Number of simultaneous rings scales with progress.
        let ring_count = (1.0 + ctx.eased * 3.0).round() as usize;
        // Ring period: 0.5 s per beat.
        let beat_period = 0.5_f32;

        for ring in 0..ring_count {
            // Stagger rings evenly across one beat period.
            let offset = ring as f32 / ring_count as f32;
            let phase = ((ctx.time / beat_period) + offset).fract(); // 0..1
            let r = (phase * max_r as f32).round() as i32;
            if r <= 0 {
                continue;
            }

            // Fade out as ring expands (bright at centre, dim at edge).
            let fade = 1.0 - phase;

            // Draw the ring circle.
            let steps = (2.0 * PI * r as f32).ceil() as usize;
            let steps = steps.max(4);
            for s in 0..steps {
                // Skip some dots for fainter rings.
                let skip = if fade > 0.5 { 1 } else { 2 };
                if s % skip != 0 {
                    continue;
                }
                let theta = 2.0 * PI * s as f32 / steps as f32;
                let dx = (r as f32 * theta.cos()).round() as i32;
                let dy = (r as f32 * theta.sin()).round() as i32;
                draw::dot_i(grid, cx + dx, cy + dy);
            }
        }

        // Centre bright dot on the beat onset.
        let beat_phase = (ctx.time / beat_period).fract();
        if beat_phase < 0.1 {
            draw::dot_i(grid, cx, cy);
        }

        // Tint filled cells proportional to progress.
        let (_, ch) = grid.dimensions();
        let (cw, _) = grid.dimensions();
        for cy_cell in 0..ch {
            draw::tint_row(
                grid,
                cy_cell,
                0,
                cw.saturating_sub(1),
                ctx.palette.sample(ctx.eased),
            );
        }

        Ok(())
    }
}
