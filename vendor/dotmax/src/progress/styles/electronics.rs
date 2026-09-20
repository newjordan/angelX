//! Electronics / signals themed progress bars for dotmax.
//!
//! Every style in this theme is grounded in a real electronics or signals
//! concept — not just a colour change. The visual vocabulary deliberately
//! maps to physical behaviour:
//!
//! - [`RcCharge`] — exponential V=V₀(1−e^(−t/RC)) capacitor fill
//! - [`Oscilloscope`] — graticule + live waveform sweep (sine/triangle/square)
//! - [`LogicGate`] — signal pulse propagating through AND→OR→XOR→NOT
//! - [`SevenSegment`] — numeric counter rendered with segment lines
//! - [`LedVuMeter`] — VU column meter with peak-hold dots
//! - [`BinaryBus`] — parallel bit-bus with bits shifting left→right
//! - [`SquareClock`] — clock signal with scrolling rising/falling edges
//! - [`PwmDuty`] — PWM pulse whose duty cycle widens with progress
//! - [`ResistorBands`] — colour-coded resistor bands revealing left→right
//! - [`SignalNoise`] — clean sine emerging from noise as SNR improves
//! - [`LissajousScope`] — Lissajous x=sin(at), y=sin(bt+δ) on a CRT grid

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// All styles in the `electronics` theme.
///
/// Returns 11 structurally distinct bars, each modelling a different concept
/// from analogue/digital electronics. Safe to render at any size from 1×1 up.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(RcCharge),
        Box::new(Oscilloscope),
        Box::new(LogicGate),
        Box::new(SevenSegment),
        Box::new(LedVuMeter),
        Box::new(BinaryBus),
        Box::new(SquareClock),
        Box::new(PwmDuty),
        Box::new(ResistorBands),
        Box::new(SignalNoise),
        Box::new(LissajousScope),
    ]
}

// ---------------------------------------------------------------------------
// 1. RC capacitor charging
//    V(t) = V₀ · (1 − e^(−eased / (1−eased+ε)))
//    The curve is drawn dot-by-dot from left to right; a capacitor symbol
//    (two vertical plates separated by a gap) appears at the right edge and
//    fills from bottom upward proportional to V.
// ---------------------------------------------------------------------------
struct RcCharge;
impl ProgressStyle for RcCharge {
    fn name(&self) -> &str {
        "rc-charge"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "RC capacitor charging: exponential V=V₀(1−e^(−t/RC)) curve draws out, capacitor fills"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Reserve the rightmost 4 dot-columns for the capacitor symbol.
        let cap_w = 4usize;
        let plot_w = w.saturating_sub(cap_w + 1);

        // Draw the exponential charging curve across plot_w columns.
        // We evaluate V at each x position relative to progress reaching that x.
        let mid_y = (h - 1) as i32; // baseline at the bottom
        let top_y = 0i32; // 100% charged = top dot row

        let mut prev_y: Option<i32> = None;
        for xi in 0..plot_w {
            // normalised x in [0, 1]
            let xn = if plot_w <= 1 {
                ctx.eased
            } else {
                xi as f32 / (plot_w - 1) as f32
            };
            // The curve is only drawn up to the current charge level (eased).
            let effective = xn.min(ctx.eased);
            // RC time constant: tau chosen so 5RC covers the bar.
            let tau = 0.2_f32;
            let v = 1.0 - (-effective / tau).exp();
            // v in [0, 1]; map to dot row (0 = top, h-1 = bottom)
            let dot_y = (mid_y - (v * h as f32 * 0.9) as i32).clamp(top_y, mid_y);
            draw::dot_i(grid, xi as i32, dot_y);
            // Connect consecutive dots vertically to avoid gaps.
            if let Some(py) = prev_y {
                let lo = py.min(dot_y);
                let hi = py.max(dot_y);
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dot_y);
        }

        // Draw baseline.
        draw::hline(grid, 0, plot_w.saturating_sub(1), h - 1);

        // Draw the capacitor symbol: two plates (vlines) with a gap between.
        if cap_w + 1 <= w {
            let plate_x1 = w.saturating_sub(cap_w);
            let plate_x2 = w.saturating_sub(cap_w - 2);
            let plate_top = h / 4;
            let plate_bot = h.saturating_sub(h / 4).max(plate_top);
            draw::vline(grid, plate_x1, plate_top, plate_bot);
            draw::vline(grid, plate_x2, plate_top, plate_bot);
            // Wire into the plates from the curve endpoint and from right.
            let wire_y = h / 2;
            draw::hline(grid, plot_w, plate_x1, wire_y);
            draw::hline(grid, plate_x2, w - 1, wire_y);
            // Fill the capacitor from bottom upward proportional to charge.
            let fill_h = (ctx.eased * (plate_bot - plate_top + 1) as f32).round() as usize;
            let fill_y0 = plate_bot.saturating_sub(fill_h.saturating_sub(1));
            if fill_h > 0 {
                for fy in fill_y0..=plate_bot {
                    draw::dot(grid, plate_x1 + 1, fy);
                }
            }
        }

        // Tint the charged region.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Oscilloscope
//    A graticule (light grid lines) with a live waveform trace.
//    Waveform shape cycles through sine → triangle → square as eased rises.
//    The trace sweeps rightward driven by time.
// ---------------------------------------------------------------------------
struct Oscilloscope;
impl ProgressStyle for Oscilloscope {
    fn name(&self) -> &str {
        "oscilloscope"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "CRT oscilloscope with graticule grid and waveform sweeping sine→triangle→square"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Draw graticule: sparse horizontal and vertical lines.
        let h_divs = 4usize.max(1);
        let v_divs = 8usize.max(1);
        for di in 0..=h_divs {
            let y = di * h / h_divs.max(1);
            let y = y.min(h - 1);
            // Dashed: every 4 dots draw 2, skip 2.
            for xi in (0..w).step_by(4) {
                draw::dot(grid, xi, y);
                if xi + 1 < w {
                    draw::dot(grid, xi + 1, y);
                }
            }
        }
        for di in 0..=v_divs {
            let x = di * w / v_divs.max(1);
            let x = x.min(w - 1);
            for yi in (0..h).step_by(4) {
                draw::dot(grid, x, yi);
                if yi + 1 < h {
                    draw::dot(grid, x, yi + 1);
                }
            }
        }

        // Choose waveform: sine (0..0.33), triangle (0.33..0.67), square (0.67..1).
        let shape = (ctx.eased * 3.0).floor() as usize; // 0,1,2
        let freq = 3.0_f32; // cycles across bar
        let phase = ctx.time * 2.0 * PI * 0.5;
        let amp = (h as f32 * 0.42).max(1.0);
        let mid = (h / 2) as i32;

        let mut prev_y: Option<i32> = None;
        for xi in 0..w {
            let theta = (xi as f32 / w as f32) * freq * 2.0 * PI + phase;
            let val: f32 = match shape {
                0 => theta.sin(),
                1 => {
                    // Triangle: 2/π · arcsin(sin(θ))
                    (2.0 / PI) * theta.sin().asin()
                }
                _ => {
                    // Square
                    if theta.sin() >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
            };
            let dy = (mid - (val * amp) as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, dy);
            if let Some(py) = prev_y {
                let lo = py.min(dy);
                let hi = py.max(dy);
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        // Tint: full-width green-ish glow (use palette, skew toward end colour).
        let (cw, ch) = grid.dimensions();
        for cx in 0..cw {
            let t = cx as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t * ctx.eased);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Logic-gate cascade
//    A signal pulse propagates through four gates: AND → OR → XOR → NOT.
//    Gates are drawn as text symbols; the signal wire lights up between them
//    as progress crosses each threshold (0.25, 0.5, 0.75, 1.0).
//    The active gate flickers at ~4 Hz to indicate processing.
// ---------------------------------------------------------------------------
struct LogicGate;
impl ProgressStyle for LogicGate {
    fn name(&self) -> &str {
        "logic-gate"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "Signal pulse cascades through AND→OR→XOR→NOT gates; active gate flickers"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, _h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Gate labels; we render them as glyphs.
        let gates = ["&", "|", "^", "!"];
        let n_gates = gates.len();
        // Each gate occupies (cw / (n_gates+1)) cells; wires fill the gaps.
        let spacing = (cw / (n_gates + 1)).max(1);

        // Wire row: middle cell row.
        let wire_row = ch / 2;

        // Draw the full wire as a horizontal run of dots.
        let wire_dot_y = wire_row * 4 + 1; // a dot near the middle of the cell row
        let wire_dot_y = wire_dot_y.min(w.saturating_sub(1)); // reuse h bound check via dot()
                                                              // Horizontal baseline wire across the whole grid dot-width.
        draw::hline(
            grid,
            0,
            w.saturating_sub(1),
            wire_dot_y.min({
                let (_, dh) = draw::dot_dims(grid);
                dh.saturating_sub(1)
            }),
        );

        // Thicken the powered stretch of wire so progress reads without
        // color, and race a pulse packet along it.
        let (_, dh) = draw::dot_dims(grid);
        let powered = (ctx.eased * w as f32) as usize;
        let wire_y = (wire_row * 4 + 1).min(dh.saturating_sub(2));
        for x in 0..powered.min(w) {
            draw::dot(grid, x, wire_y + 1);
        }
        if powered > 4 {
            let pulse = ((ctx.time * 0.75).fract() * powered as f32) as usize;
            for k in 0..4usize {
                if pulse >= k {
                    draw::dot(grid, pulse - k, wire_y.saturating_sub(1));
                }
            }
        }

        // Draw each gate symbol and light up wires up to the active gate.
        let lit_gates = (ctx.eased * n_gates as f32).floor() as usize;
        let flicker_on = (ctx.time * 8.0) as usize % 2 == 0;

        for (gi, &label) in gates.iter().enumerate() {
            let gate_cell_x = (gi + 1) * spacing;
            if gate_cell_x >= cw {
                break;
            }

            // Draw the gate glyph at wire_row.
            let ch_sym = label.chars().next().unwrap_or('?');
            draw::glyph(grid, gate_cell_x, wire_row, ch_sym);

            // Tint the wire cells leading up to this gate if lit.
            if gi < lit_gates {
                let wire_start = if gi == 0 { 0 } else { gi * spacing };
                let wire_end = gate_cell_x.saturating_sub(1);
                for cx in wire_start..=wire_end.min(cw.saturating_sub(1)) {
                    let col = ctx.palette.sample(gi as f32 / n_gates as f32);
                    for cy in 0..ch {
                        draw::tint_row(grid, cy, cx, cx, col);
                    }
                }
                // Tint the gate cell itself.
                let col = ctx.palette.sample((gi + 1) as f32 / n_gates as f32);
                for cy in 0..ch {
                    draw::tint_row(grid, cy, gate_cell_x, gate_cell_x, col);
                }
            } else if gi == lit_gates && flicker_on {
                // Active gate (being processed): flicker.
                let col = ctx.palette.sample(0.8);
                for cy in 0..ch {
                    draw::tint_row(grid, cy, gate_cell_x, gate_cell_x, col);
                }
            }
        }

        // Tint the wire after the last lit gate.
        if lit_gates >= n_gates {
            let last_gate_x = n_gates * spacing;
            let trail_start = last_gate_x + 1;
            for cx in trail_start..cw {
                let col = ctx.palette.sample(1.0);
                for cy in 0..ch {
                    draw::tint_row(grid, cy, cx, cx, col);
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. 7-segment counter
//    Counts from 0 up to a value proportional to eased (max 99 for 2 digits).
//    Each digit is drawn using segment lines in dot space.
//    Segments: top, top-left, top-right, middle, bot-left, bot-right, bottom.
// ---------------------------------------------------------------------------

/// Segment on/off table for digits 0–9.
/// Order: [top, tl, tr, mid, bl, br, bot]
const SEG7: [[bool; 7]; 10] = [
    [true, true, true, false, true, true, true],     // 0
    [false, false, true, false, false, true, false], // 1
    [true, false, true, true, true, false, true],    // 2
    [true, false, true, true, false, true, true],    // 3
    [false, true, true, true, false, true, false],   // 4
    [true, true, false, true, false, true, true],    // 5
    [true, true, false, true, true, true, true],     // 6
    [true, false, true, false, false, true, false],  // 7
    [true, true, true, true, true, true, true],      // 8
    [true, true, true, true, false, true, true],     // 9
];

/// Draw a single 7-segment digit in dot space at (ox, oy) with given w×h.
fn draw_seg7_digit(
    grid: &mut BrailleGrid,
    digit: usize,
    ox: usize,
    oy: usize,
    sw: usize,
    sh: usize,
) {
    let d = digit.min(9);
    let segs = SEG7[d];
    let mid_y = oy + sh / 2;
    let bot_y = oy + sh.saturating_sub(1);
    let right_x = ox + sw.saturating_sub(1);

    // top
    if segs[0] {
        draw::hline(grid, ox, right_x, oy);
    }
    // top-left
    if segs[1] {
        draw::vline(grid, ox, oy, mid_y);
    }
    // top-right
    if segs[2] {
        draw::vline(grid, right_x, oy, mid_y);
    }
    // middle
    if segs[3] {
        draw::hline(grid, ox, right_x, mid_y);
    }
    // bot-left
    if segs[4] {
        draw::vline(grid, ox, mid_y, bot_y);
    }
    // bot-right
    if segs[5] {
        draw::vline(grid, right_x, mid_y, bot_y);
    }
    // bottom
    if segs[6] {
        draw::hline(grid, ox, right_x, bot_y);
    }
}

struct SevenSegment;
impl ProgressStyle for SevenSegment {
    fn name(&self) -> &str {
        "seven-segment"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "7-segment numeric counter: digits drawn with segment lines, counting up with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Determine how many digits we can fit.
        // Each digit needs ~6 dot-columns wide and full height.
        let digit_w = (w / 4).max(3).min(12);
        let gap = 2usize;
        let n_digits = ((w + gap) / (digit_w + gap)).max(1).min(4);

        // Current count: 0..=10^n_digits - 1
        let max_val = 10usize.pow(n_digits as u32).saturating_sub(1);
        let count = (ctx.eased * max_val as f32).round() as usize;

        // Centre the digits horizontally.
        let total_w = n_digits * digit_w + (n_digits - 1) * gap;
        let ox = w.saturating_sub(total_w) / 2;

        for di in 0..n_digits {
            // Extract the di-th digit from count (leftmost = most significant).
            let place = 10usize.pow((n_digits - 1 - di) as u32);
            let digit = (count / place) % 10;
            let dx = ox + di * (digit_w + gap);
            if dx + digit_w <= w {
                draw_seg7_digit(grid, digit, dx, 0, digit_w, h);
            }
        }

        // Tint: gradient across the full width, brightness scales with eased.
        let (cw, ch) = grid.dimensions();
        for cx in 0..cw {
            let t = cx as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t * ctx.eased + ctx.eased * 0.5);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. LED VU meter
//    N vertical bar columns; each column's level is driven by a synthetic
//    audio signal: A_k = |sin(k·time · f_k)| where f_k varies per column.
//    The filled region (left columns) react fully; right columns are dim.
//    Peak-hold: a single dot at the highest recent level per column.
// ---------------------------------------------------------------------------
struct LedVuMeter;
impl ProgressStyle for LedVuMeter {
    fn name(&self) -> &str {
        "led-vu-meter"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "LED VU bar graph: columns pulse to synthetic audio levels; peak-hold dot per column"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        let n_cols = cw;
        let lit_cols = (ctx.eased * n_cols as f32).round() as usize;

        for col in 0..n_cols {
            // Synthetic audio level per column: sum of two sinusoids.
            let kf = (col + 1) as f32;
            let freq1 = 0.7 + kf * 0.3;
            let freq2 = 1.3 + kf * 0.17;
            let raw = ((kf * ctx.time * freq1).sin().abs()
                + (kf * 0.5 * ctx.time * freq2 + 1.0).sin().abs())
                * 0.5;
            let level = if col < lit_cols { raw } else { raw * 0.12 };

            // Convert level [0,1] to vblock eighths per cell row.
            // Columns fill from the bottom upward.
            let total_eighths = (level * ch as f32 * 8.0).round() as usize;
            let full_cells = total_eighths / 8;
            let rem = total_eighths % 8;

            // Draw full cells from bottom upward.
            for row in 0..full_cells.min(ch) {
                let cell_y = ch.saturating_sub(1).saturating_sub(row);
                draw::vblock(grid, col, cell_y, 8);
            }
            // Partial cell above the full cells.
            if rem > 0 && full_cells < ch {
                let cell_y = ch.saturating_sub(1).saturating_sub(full_cells);
                draw::vblock(grid, col, cell_y, rem);
            }

            // Peak-hold dot: place a single vblock=1 at the top of the level.
            let peak_eighths = total_eighths.saturating_add(4).min(ch * 8);
            let peak_row = ch.saturating_sub(1).saturating_sub(peak_eighths / 8);
            if peak_row < ch && full_cells > 0 {
                draw::vblock(grid, col, peak_row, 1);
            }

            // Tint the column.
            if col < lit_cols {
                let t = col as f32 / n_cols.max(1) as f32;
                let col_color = ctx.palette.sample(t);
                for cy in 0..ch {
                    draw::tint_row(grid, cy, col, col, col_color);
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Binary data bus
//    8 parallel horizontal bit lines. Bits shift left→right over time.
//    The number of "ones" in the current word = floor(eased × 8).
//    Each line carries a different bit position of a pseudo-random byte stream.
// ---------------------------------------------------------------------------
struct BinaryBus;
impl ProgressStyle for BinaryBus {
    fn name(&self) -> &str {
        "binary-bus"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "8-bit data bus: parallel bit lines shift bits left→right; word density = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Number of bus lines — cap at 8, fill h evenly.
        let n_lines = 8usize.min(h);
        let ones_per_word = (ctx.eased * n_lines as f32).round() as usize;

        // Bit cell width in dots (each bit takes bit_w dots).
        let bit_w = (w / 16).max(2);
        let n_cells = w / bit_w;

        // Scroll offset: bits move left at 4 cells/sec.
        let scroll = (ctx.time * 4.0) as usize;

        for line in 0..n_lines {
            // y position for this line: evenly spaced.
            let y = if n_lines <= 1 {
                h / 2
            } else {
                line * (h - 1) / (n_lines - 1)
            };
            let y = y.min(h - 1);

            // Draw each bit cell.
            for ci in 0..n_cells {
                // Determine the bit value via a simple LFSR-like hash of (ci+scroll, line).
                let slot = ci.wrapping_add(scroll);
                // Hash: mix slot and line to produce a pseudo-random bit.
                let hash = slot
                    .wrapping_mul(2654435761)
                    .wrapping_add(line.wrapping_mul(40503));
                // Which bit of hash to use: cycle through n_lines bits.
                let bit_pos = line % 8;
                let raw_bit = (hash >> bit_pos) & 1;
                // bit is "1" only if it is in the active ones budget AND raw says so.
                let bit = raw_bit == 1 && line < ones_per_word;

                let x0 = ci * bit_w;
                let x1 = (x0 + bit_w).saturating_sub(2).max(x0);

                if bit {
                    // HIGH: draw a line at top of cell.
                    draw::hline(grid, x0, x1, y.saturating_sub(1).max(0));
                    draw::hline(grid, x0, x1, y);
                } else {
                    // LOW: draw a line at bottom (just one dot row).
                    draw::hline(grid, x0, x1, (y + 1).min(h - 1));
                }

                // Rising/falling edge connectors (vertical transitions).
                // Peek at next bit.
                if ci + 1 < n_cells {
                    let next_slot = ci + 1 + scroll;
                    let next_hash = next_slot
                        .wrapping_mul(2654435761)
                        .wrapping_add(line.wrapping_mul(40503));
                    let next_raw = (next_hash >> (line % 8)) & 1;
                    let next_bit = next_raw == 1 && line < ones_per_word;
                    if bit != next_bit && x1 + 1 < w {
                        // Draw a vertical edge connector at x1.
                        let y_lo = y.saturating_sub(1);
                        let y_hi = (y + 1).min(h - 1);
                        draw::vline(grid, x1 + 1, y_lo, y_hi);
                    }
                }
            }
        }

        // Tint active lines.
        let (cw, ch) = grid.dimensions();
        for cx in 0..cw {
            let t = cx as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t * ctx.eased);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Square-wave clock signal
//    A digital clock trace scrolls from right to left.
//    Duty cycle is fixed at 50%; eased controls how many complete clock
//    cycles have passed (i.e. the fill level = cycles completed).
//    Rising and falling edges are sharp verticals.
// ---------------------------------------------------------------------------
struct SquareClock;
impl ProgressStyle for SquareClock {
    fn name(&self) -> &str {
        "square-clock"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "Digital clock signal scrolling left; rising/falling edges sharp; progress = cycles complete"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Clock period in dots; eased sets how many full cycles fit in the bar.
        let min_period = 8usize;
        let max_cycles = (w / min_period).max(1);
        let cycles = (ctx.eased * max_cycles as f32).ceil() as usize + 1;
        let period = (w / cycles.max(1)).max(4);

        // Scroll: the waveform shifts left over time.
        let scroll_dots = (ctx.time * 6.0) as usize % (period * 2).max(1);

        let hi_y = h / 4; // top rail (logic HIGH)
        let lo_y = h.saturating_sub(h / 4 + 1); // bottom rail (logic LOW)

        let lit_x = (ctx.eased * w as f32).round() as usize; // filled boundary

        let mut prev_level: Option<bool> = None;
        for xi in 0..w {
            // Phase position within the period, accounting for scroll.
            let phase = (xi + scroll_dots) % (period * 2).max(1);
            let high = phase < period;

            let y = if high { hi_y } else { lo_y };
            let y = y.min(h - 1);

            // Draw the horizontal rail dot.
            draw::dot(grid, xi, y);

            // Draw the vertical edge when the level changes.
            if let Some(prev) = prev_level {
                if prev != high {
                    draw::vline(grid, xi, hi_y, lo_y);
                }
            }

            prev_level = Some(high);

            // Dim dots beyond the progress boundary.
            if xi >= lit_x && xi < lit_x + 2 {
                // Draw a progress cursor marker.
                draw::vline(grid, xi, 0, h.saturating_sub(1));
            }
        }

        // Tint.
        let (cw, ch) = grid.dimensions();
        let filled_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled_cells.min(cw) {
            let col = ctx.palette.sample(cx as f32 / cw.max(1) as f32);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. PWM duty cycle
//    A repeating pulse whose duty cycle = eased (0%→100%).
//    Multiple pulses fill the bar; the on-time widens as progress increases.
//    Frequency stays constant (≈8 pulses across the bar).
// ---------------------------------------------------------------------------
struct PwmDuty;
impl ProgressStyle for PwmDuty {
    fn name(&self) -> &str {
        "pwm-duty"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "PWM signal: fixed-frequency pulses whose on-time duty cycle widens with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let n_pulses = 8usize;
        let period = (w / n_pulses).max(2);
        let on_time = (ctx.eased * period as f32).round() as usize;

        let hi_y = h / 5;
        let lo_y = h.saturating_sub(h / 5 + 1).min(h - 1);

        // Scroll slightly with time for visual feedback.
        let scroll = (ctx.time * 3.0) as usize % period.max(1);

        for xi in 0..w {
            let phase = (xi + scroll) % period.max(1);
            let is_high = phase < on_time;
            let y = if is_high { hi_y } else { lo_y };
            draw::dot(grid, xi, y.min(h - 1));

            // Vertical edge.
            if xi > 0 {
                let prev_phase = (xi + scroll - 1) % period.max(1);
                let prev_high = prev_phase < on_time;
                if is_high != prev_high {
                    draw::vline(grid, xi, hi_y, lo_y);
                }
            }
        }

        // Shading inside the ON region using shade glyphs for visual texture.
        let (cw, ch) = grid.dimensions();
        for col in 0..cw {
            let xi_mid = col * 2 + 1;
            let phase = (xi_mid + scroll) % (period * 2 / 1).max(2); // dot phase
            let cell_period = period / 2; // cells per period (each cell = 2 dots)
            let cell_on = (on_time / 2).max(if on_time > 0 { 1 } else { 0 });
            let cell_phase = (col + scroll / 2) % cell_period.max(1);
            if cell_phase < cell_on && cell_period > 0 {
                // Use shade to indicate ON time.
                let density = 2usize + (ctx.eased * 2.0) as usize;
                for cy in 0..ch {
                    draw::shade(grid, col, cy, density.min(4));
                }
                let col_color = ctx.palette.sample(col as f32 / cw.max(1) as f32);
                for cy in 0..ch {
                    draw::tint_row(grid, cy, col, col, col_color);
                }
            }
            let _ = phase; // suppress unused warning
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Resistor colour bands
//    A resistor body drawn with end caps. Colour bands reveal left→right
//    as progress increases. Each band is a distinct vertical stripe of
//    dots at a fixed width. The IEC resistor colour code:
//    0=black, 1=brown, 2=red, 3=orange, 4=yellow, 5=green, 6=blue,
//    7=violet, 8=grey, 9=white. Bands use palette lerp for terminal colour.
// ---------------------------------------------------------------------------

/// Band colour fractions within the start→end palette, indexed by digit 0–9.
const BAND_T: [f32; 10] = [
    0.0,  // 0 black   → start
    0.11, // 1 brown
    0.22, // 2 red
    0.33, // 3 orange
    0.44, // 4 yellow
    0.55, // 5 green
    0.66, // 6 blue
    0.77, // 7 violet
    0.88, // 8 grey
    1.0,  // 9 white   → end
];

struct ResistorBands;
impl ProgressStyle for ResistorBands {
    fn name(&self) -> &str {
        "resistor-bands"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "Resistor with IEC colour bands revealing left-to-right as progress increases"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        let (cw, ch) = grid.dimensions();
        if w == 0 || h == 0 || cw == 0 || ch == 0 {
            return Ok(());
        }

        // Resistor body: a filled horizontal rectangle in the middle 60% of height.
        let body_top = h / 5;
        let body_bot = h.saturating_sub(h / 5).max(body_top);

        // End caps / lead wires (thin horizontal lines at mid-height).
        let lead_y = (body_top + body_bot) / 2;
        let body_x0 = w / 6;
        let body_x1 = w.saturating_sub(w / 6).max(body_x0 + 1);
        // Left lead.
        draw::hline(grid, 0, body_x0, lead_y);
        // Right lead.
        draw::hline(grid, body_x1, w.saturating_sub(1), lead_y);
        // Body outline.
        draw::rect_outline(
            grid,
            body_x0,
            body_top,
            body_x1 - body_x0,
            body_bot - body_top + 1,
        );
        // Body fill (sparse, to show bands over it).
        for y in body_top + 1..body_bot {
            for x in body_x0 + 1..body_x1 {
                if (x + y) % 3 == 0 {
                    draw::dot(grid, x, y);
                }
            }
        }

        // Bands: 4 bands (3 value + 1 tolerance) within the body.
        let n_bands = 4usize;
        let body_len = body_x1.saturating_sub(body_x0 + 2); // inner pixels
        let band_spacing = body_len / (n_bands + 1);
        let band_w = (band_spacing / 2).max(1);

        let lit_bands = (ctx.eased * n_bands as f32).ceil() as usize;

        for bi in 0..n_bands {
            if bi >= lit_bands {
                break;
            }
            // Band centre x in dot space.
            let cx_dot = body_x0 + 1 + (bi + 1) * band_spacing;
            let bx0 = cx_dot.saturating_sub(band_w / 2);
            let bx1 = (bx0 + band_w).min(body_x1.saturating_sub(1));

            // Digit for this band: use a fixed sequence 4,7,2,5 (a representative value).
            let digits = [4usize, 7, 2, 5];
            let digit = digits[bi % digits.len()];
            let band_t = BAND_T[digit];

            // Draw band as solid vertical stripe.
            for bx in bx0..=bx1 {
                draw::vline(
                    grid,
                    bx,
                    body_top + 1,
                    body_bot.saturating_sub(1).max(body_top + 1),
                );
            }

            // Tint the band cells.
            let band_cell_x0 = bx0 / 2;
            let band_cell_x1 = (bx1 / 2 + 1).min(cw.saturating_sub(1));
            let col = ctx.palette.sample(band_t);
            for cy in 0..ch {
                draw::tint_row(grid, cy, band_cell_x0, band_cell_x1, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Signal through noise
//     A sinusoidal target signal mixed with Gaussian-like noise.
//     SNR = eased: at 0 the trace is pure noise; at 1 it is a clean sine.
//     Noise is generated from a deterministic hash of (x, time_bucket).
// ---------------------------------------------------------------------------

/// Fast deterministic noise in [-1, 1] from integer seed.
#[inline]
fn pseudo_noise(seed: u32) -> f32 {
    let h = seed
        .wrapping_mul(2246822519)
        .wrapping_add(seed.wrapping_mul(3266489917));
    let h = h ^ (h >> 13);
    let h = h.wrapping_mul(1274126177);
    let h = h ^ (h >> 16);
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

struct SignalNoise;
impl ProgressStyle for SignalNoise {
    fn name(&self) -> &str {
        "signal-noise"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "Sine signal rising from noise: SNR improves with progress until the clean wave emerges"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let snr = ctx.eased; // 0=all noise, 1=clean sine
        let noise_amp = 1.0 - snr * 0.95; // noise amplitude → 0 as SNR → 1
        let sig_amp = snr; // signal amplitude → 1

        let freq = 3.0_f32;
        let phase = ctx.time * PI * 0.6;

        // Quantise time into buckets (4 per second) so noise is stable between frames.
        let time_bucket = (ctx.time * 4.0) as u32;

        let mid = (h / 2) as i32;
        let half_h = (h as f32 * 0.45).max(1.0);

        let mut prev_y: Option<i32> = None;
        for xi in 0..w {
            let xn = xi as f32 / w as f32;
            let sine_val = (xn * freq * 2.0 * PI + phase).sin();
            let noise_val = pseudo_noise(xi as u32 ^ time_bucket.wrapping_mul(1013904223));
            let val = sig_amp * sine_val + noise_amp * noise_val;
            let dy = (mid - (val * half_h) as i32).clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, dy);
            if let Some(py) = prev_y {
                let lo = py.min(dy);
                let hi = py.max(dy);
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        // Baseline.
        draw::hline(grid, 0, w.saturating_sub(1), (h / 2).min(h - 1));

        // Tint: gradient that sharpens (more saturated) as SNR increases.
        let (cw, ch) = grid.dimensions();
        for cx in 0..cw {
            let t = cx as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t * 0.5 + snr * 0.5);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Lissajous on an oscilloscope screen
//     x = sin(a·τ + δ),  y = sin(b·τ)
//     a and b selected by eased (ratio table), δ drifts with time.
//     A faint CRT-style circular border frames the display.
// ---------------------------------------------------------------------------

/// (a, b) ratio pairs for Lissajous figures, indexed by eased × N_RATIOS.
const LISSAJOUS_RATIOS: [(f32, f32); 6] = [
    (1.0, 1.0), // circle / ellipse
    (2.0, 1.0), // figure-8 horizontal
    (1.0, 2.0), // figure-8 vertical
    (3.0, 2.0), // trefoil-like
    (3.0, 4.0), // 5-lobe
    (5.0, 4.0), // complex knot
];

struct LissajousScope;
impl ProgressStyle for LissajousScope {
    fn name(&self) -> &str {
        "lissajous-scope"
    }
    fn theme(&self) -> &str {
        "electronics"
    }
    fn describe(&self) -> &str {
        "Lissajous figure on a CRT scope: ratio unlocks with progress, phase drifts with time"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let n_ratios = LISSAJOUS_RATIOS.len();
        let idx = ((ctx.eased * n_ratios as f32) as usize).min(n_ratios - 1);
        let (a, b) = LISSAJOUS_RATIOS[idx];

        let delta = ctx.time * 0.4; // phase drift

        // Draw a faint circular CRT bezel (ellipse inscribed in the dot rect).
        let cx_f = (w as f32 - 1.0) / 2.0;
        let cy_f = (h as f32 - 1.0) / 2.0;
        let rx = cx_f * 0.97;
        let ry = cy_f * 0.97;
        let bezel_pts = (w + h) * 2;
        for i in 0..bezel_pts {
            let angle = (i as f32 / bezel_pts as f32) * 2.0 * PI;
            let px = (cx_f + rx * angle.cos()) as i32;
            let py = (cy_f + ry * angle.sin()) as i32;
            draw::dot_i(grid, px, py);
        }

        // Draw graticule cross-hairs (centre lines only).
        let mid_x = w / 2;
        let mid_y = h / 2;
        // Dashed hline.
        for x in (0..w).step_by(4) {
            draw::dot(grid, x.min(w - 1), mid_y);
            if x + 1 < w {
                draw::dot(grid, x + 1, mid_y);
            }
        }
        // Dashed vline.
        for y in (0..h).step_by(4) {
            draw::dot(grid, mid_x, y.min(h - 1));
            if y + 1 < h {
                draw::dot(grid, mid_x, y + 1);
            }
        }

        // Plot the Lissajous figure.
        let plot_rx = rx * 0.88;
        let plot_ry = ry * 0.88;
        let steps = (w * h).max(512);
        let period = 2.0 * PI;
        let mut prev: Option<(i32, i32)> = None;
        for si in 0..steps {
            let tau = (si as f32 / steps as f32) * period;
            let lx = plot_rx * (a * tau + delta).sin();
            let ly = plot_ry * (b * tau).sin();
            let px = (cx_f + lx) as i32;
            let py = (cy_f + ly) as i32;
            draw::dot_i(grid, px, py);
            if let Some((ox, oy)) = prev {
                let gap = (((px - ox).abs() + (py - oy).abs()) as usize).max(1);
                for s in 1..gap {
                    let f = s as f32 / gap as f32;
                    let ix = (ox as f32 + (px - ox) as f32 * f) as i32;
                    let iy = (oy as f32 + (py - oy) as f32 * f) as i32;
                    draw::dot_i(grid, ix, iy);
                }
            }
            prev = Some((px, py));
        }

        // Tint: full grid coloured by eased.
        let (cw, ch) = grid.dimensions();
        for cell_x in 0..cw {
            let t = cell_x as f32 / cw.max(1) as f32;
            let col = ctx.palette.sample(t * ctx.eased + (1.0 - ctx.eased) * 0.3);
            for cy in 0..ch {
                draw::tint_row(grid, cy, cell_x, cell_x, col);
            }
        }
        Ok(())
    }
}
