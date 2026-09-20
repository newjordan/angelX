//! Tech / cyberpunk progress bars — digital rain, glitch, neon, and signal.
//!
//! Every style in this module is stateless: all animation derives from
//! `ctx.time` and `ctx.eased` with no mutable state. The deterministic
//! `hash` helper produces pseudo-random values for sparkle and glitch
//! effects without any external dependencies.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ─── deterministic hash (no external crates) ────────────────────────────────

#[inline]
fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x.wrapping_mul(2_246_822_519)
}

/// Map hash output to [0.0, 1.0).
#[inline]
fn hashf(n: u32) -> f32 {
    (hash(n) % 1000) as f32 / 1000.0
}

/// All styles in the `tech` theme.
///
/// Returns 11 distinct cyberpunk-themed progress bar implementations, each
/// stateless and animatable via `ctx.time`.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(MatrixRain),
        Box::new(NeonScanline),
        Box::new(DataPackets),
        Box::new(GlitchBar),
        Box::new(TerminalTyper),
        Box::new(HexFill),
        Box::new(SignalBars),
        Box::new(DownloadStream),
        Box::new(BinaryCounter),
        Box::new(Heartbeat),
        Box::new(CircuitTrace),
    ]
}

// ─── 1. Matrix digital rain ──────────────────────────────────────────────────

/// Matrix-style columns of falling dots whose density rises with progress.
struct MatrixRain;
impl ProgressStyle for MatrixRain {
    fn name(&self) -> &str {
        "matrix-rain"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Matrix digital rain: column density rises with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // How many columns are "active" depends on eased progress.
        let active_cols = ((ctx.eased * w as f32) as usize).min(w);

        for col in 0..active_cols {
            // Each column gets a unique phase offset from the hash.
            let phase = hashf(col as u32 * 7 + 1);
            let speed = 0.4 + 0.6 * hashf(col as u32 * 13 + 3);
            // Fall position: wraps top-to-bottom.
            let fall_t = (ctx.time * speed + phase).fract();
            let head = (fall_t * h as f32) as usize;

            // Draw a "raindrop" — head is bright, tail fades.
            let tail_len = (h / 3).max(2);
            for i in 0..tail_len {
                let y = if head >= i { head - i } else { h + head - i };
                if y < h {
                    draw::dot(grid, col, y);
                }
            }

            // Tint with palette: head bright end, tail dim start.
            let cell_x = col / 2;
            if cell_x < cells_w {
                let t = col as f32 / active_cols.max(1) as f32;
                let color = ctx.palette.sample(t);
                for cy in 0..cells_h {
                    draw::tint_row(grid, cy, cell_x, cell_x, color);
                }
            }
        }
        Ok(())
    }
}

// ─── 2. Neon scanline sweep ──────────────────────────────────────────────────

/// A neon vertical bar sweeps right; eased progress determines how far it
/// travels. The swept region glows with the palette gradient.
struct NeonScanline;
impl ProgressStyle for NeonScanline {
    fn name(&self) -> &str {
        "neon-scanline"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Neon vertical scanline with palette glow, eased to progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let head = ((ctx.eased * w as f32) as usize).min(w.saturating_sub(1));

        // Filled track up to head.
        // Draw a center baseline and a thick beam at the head.
        let mid = h / 2;
        draw::hline(grid, 0, head, mid);

        // Scanline beam: full height at head, half-height one behind, etc.
        let beam_w = (w / 20).max(1);
        for offset in 0..beam_w {
            if head < offset {
                break;
            }
            let x = head - offset;
            let reach = (h as f32 * (1.0 - offset as f32 / beam_w as f32)) as usize;
            let y0 = mid.saturating_sub(reach / 2);
            let y1 = (mid + reach / 2).min(h.saturating_sub(1));
            draw::vline(grid, x, y0, y1);
        }

        // Gradient tint from start to head.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = if filled_cells <= 1 {
                0.0
            } else {
                cx as f32 / (filled_cells - 1) as f32
            };
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}

// ─── 3. Data packets ─────────────────────────────────────────────────────────

/// Discrete data "packets" slide left→right; the number flowing increases with
/// progress. Uses cubic-out easing on each packet's internal travel.
struct DataPackets;
impl ProgressStyle for DataPackets {
    fn name(&self) -> &str {
        "data-packets"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Discrete data packets stream right; flow rate scales with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let packet_w = (w / 8).max(2);
        let gap = (w / 12).max(1);
        let stride = packet_w + gap;
        let n_packets = (w / stride).max(1);

        // How many packets are visible is gated by progress.
        let active = (ctx.eased * n_packets as f32).ceil() as usize;

        for p in 0..active.min(n_packets) {
            let phase = p as f32 / n_packets as f32;
            // Each packet loops: position travels 0..w in its own time slot.
            let t_raw = (ctx.time * 0.5 + phase).fract();
            // Cubic-out easing for a "snap then slow" feel.
            let t_eased = 1.0 - (1.0 - t_raw).powi(3);
            let x0 = (t_eased * (w + packet_w) as f32) as i32 - packet_w as i32;

            let row_h = (h / 2).max(1);
            let y0 = (h - row_h) / 2;
            for dy in 0..row_h {
                for dx in 0..packet_w as i32 {
                    draw::dot_i(grid, x0 + dx, (y0 + dy) as i32);
                }
            }

            // Tint the packet's cell span.
            let t_color = p as f32 / n_packets.max(1) as f32;
            let color = ctx.palette.sample(t_color);
            let cx0 = (x0 / 2).max(0) as usize;
            let cx1 = ((x0 + packet_w as i32) / 2).clamp(0, cells_w as i32 - 1) as usize;
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx0, cx1, color);
            }
        }
        Ok(())
    }
}

// ─── 4. Glitch bar ───────────────────────────────────────────────────────────

/// A solid fill that occasionally "glitches": horizontal strips are displaced
/// by random amounts derived from the hash function and snapped by time.
struct GlitchBar;
impl ProgressStyle for GlitchBar {
    fn name(&self) -> &str {
        "glitch"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Solid fill with periodic horizontal glitch displacements"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let filled = ((ctx.eased * w as f32) as usize).min(w);

        // Glitch "epoch" changes ~3 times per second; drives which rows glitch.
        let epoch = (ctx.time * 3.0) as u32;

        for y in 0..h {
            // Decide if this scanline glitches this epoch.
            let row_hash = hash(y as u32 * 31 + epoch * 997);
            let glitch_prob = hashf(row_hash);
            let shift: i32 = if glitch_prob < 0.08 {
                // Displace by up to ±1/8 of width.
                let mag = (hashf(row_hash ^ 0xDEAD) * (w as f32 / 8.0)) as i32;
                if row_hash & 1 == 0 {
                    mag
                } else {
                    -mag
                }
            } else {
                0
            };

            for x in 0..filled {
                draw::dot_i(grid, x as i32 + shift, y as i32);
            }
        }

        // Gradient tint on the filled region.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = cx as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}

// ─── 5. Terminal typer ───────────────────────────────────────────────────────

/// A cursor "types" across the bar, leaving a trail of filled dots; the cursor
/// blinks at 2 Hz using `ctx.time`. Progress controls how far it has typed.
struct TerminalTyper;
impl ProgressStyle for TerminalTyper {
    fn name(&self) -> &str {
        "terminal-typer"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Cursor types across the bar; cursor blinks, trail is filled"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let head = ((ctx.eased * w as f32) as usize).min(w.saturating_sub(1));

        // Trail: filled region before cursor.
        if head > 0 {
            draw::fill_rect(grid, 0, 0, head, h);
        }

        // Blinking cursor block: blinks at 2 Hz.
        let blink_on = (ctx.time * 2.0).fract() > 0.5;
        if blink_on && head < w {
            // Cursor is a full-height vertical block.
            draw::vline(grid, head, 0, h.saturating_sub(1));
            // Add one to the right for a fat cursor.
            if head + 1 < w {
                draw::vline(grid, head + 1, 0, h.saturating_sub(1));
            }
        }

        // Tint: trail in palette gradient, cursor in bright end color.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = cx as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}

// ─── 6. Hex / binary block fill ──────────────────────────────────────────────

/// The bar is divided into fixed-width "hex cells". Each cell toggles on in
/// sequence as eased progress advances, giving a quantised, digital feel.
struct HexFill;
impl ProgressStyle for HexFill {
    fn name(&self) -> &str {
        "hex-fill"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Hex-cell blocks toggle on sequentially as progress advances"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let n_cells = 16usize.min(w / 2);
        let cell_w = w / n_cells.max(1);
        let lit = (ctx.eased * n_cells as f32) as usize;

        for i in 0..n_cells {
            let x0 = i * cell_w;
            // Gap of 1 dot between cells.
            let bw = cell_w.saturating_sub(1).max(1);
            if i < lit {
                // Fully lit cell.
                draw::fill_rect(grid, x0, 0, bw, h);
            } else if i == lit {
                // Partially lit cell — animates in with a sine flicker.
                let flicker = ((ctx.time * 8.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                let bh = (flicker * h as f32) as usize;
                let y0 = h.saturating_sub(bh);
                draw::fill_rect(grid, x0, y0, bw, bh);
            } else {
                // Unlit: just outline.
                draw::rect_outline(grid, x0, 0, bw.max(2), h.max(2));
            }

            // Tint lit cells.
            if i <= lit {
                let t = i as f32 / n_cells.max(1) as f32;
                let color = ctx.palette.sample(t);
                let cx0 = (x0 / 2).min(cells_w.saturating_sub(1));
                let cx1 = ((x0 + bw) / 2).min(cells_w.saturating_sub(1));
                for cy in 0..cells_h {
                    draw::tint_row(grid, cy, cx0, cx1, color);
                }
            }
        }
        Ok(())
    }
}

// ─── 7. Signal / WiFi bars ───────────────────────────────────────────────────

/// Rising signal bars whose heights step up with progress, like a WiFi
/// strength indicator. Bars pulse slightly via a sine wave.
struct SignalBars;
impl ProgressStyle for SignalBars {
    fn name(&self) -> &str {
        "signal-bars"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Rising WiFi-style signal bars that fill with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let n_bars = 8usize;
        let bar_w = (w / n_bars / 2).max(1);
        let gap = (w / n_bars).saturating_sub(bar_w).max(1);
        let stride = bar_w + gap;

        let lit_count = (ctx.eased * n_bars as f32).round() as usize;

        for i in 0..n_bars {
            let x0 = i * stride;
            if x0 >= w {
                break;
            }

            // Each bar's target height: taller bars toward the right.
            let base_frac = (i + 1) as f32 / n_bars as f32;
            // Pulse: active bars breathe slightly.
            let pulse = if i < lit_count {
                1.0 + 0.05 * (ctx.time * 2.0 * PI + i as f32 * 0.5).sin()
            } else {
                1.0
            };
            let bar_h = (base_frac * h as f32 * pulse).round() as usize;
            let bar_h = bar_h.min(h);
            let y0 = h.saturating_sub(bar_h);

            if i < lit_count {
                draw::fill_rect(grid, x0, y0, bar_w, bar_h);
                let t = i as f32 / n_bars.max(1) as f32;
                let color = ctx.palette.sample(t);
                let cx = (x0 / 2).min(cells_w.saturating_sub(1));
                for cy in 0..cells_h {
                    draw::tint_row(grid, cy, cx, cx, color);
                }
            } else {
                // Dim outline for inactive bars.
                if bar_h >= 2 && bar_w >= 1 {
                    draw::rect_outline(grid, x0, y0, bar_w.max(2), bar_h.max(2));
                }
            }
        }
        Ok(())
    }
}

// ─── 8. Download stream ──────────────────────────────────────────────────────

/// A moving "buffer window" slides over a track of dots. The track density
/// behind the window matches eased progress; the window itself scrolls.
struct DownloadStream;
impl ProgressStyle for DownloadStream {
    fn name(&self) -> &str {
        "download-stream"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Moving buffer window slides over a download track"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let filled = ((ctx.eased * w as f32) as usize).min(w);

        // Background track: sparse dots every 3 columns in the filled zone.
        let mid = h / 2;
        for x in 0..filled {
            if x % 3 == 0 {
                draw::dot(grid, x, mid);
            }
        }

        // Buffer window: a bright solid block that scrolls through filled zone.
        let win_w = (w / 6).max(2);
        if filled > win_w {
            let travel = filled - win_w;
            let win_x = ((ctx.time * 0.8).fract() * travel as f32) as usize;
            let win_x = win_x.min(travel);
            let win_h = h;
            draw::fill_rect(grid, win_x, 0, win_w, win_h);
        }

        // Track baseline.
        draw::hline(grid, 0, w.saturating_sub(1), h.saturating_sub(1));

        // Tint the filled region.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = cx as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}

// ─── 9. Binary counter ───────────────────────────────────────────────────────

/// Progress is rendered as a binary number — each bit column toggles on/off as
/// it would in a rising counter, giving a flickering digital readout effect.
struct BinaryCounter;
impl ProgressStyle for BinaryCounter {
    fn name(&self) -> &str {
        "binary-counter"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Progress displayed as a live binary counter in dot columns"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        // Map progress to an integer value in [0, 2^n_bits).
        let n_bits = (w / 4).clamp(4, 16);
        let max_val = (1u32 << n_bits).saturating_sub(1);
        let val = (ctx.eased * max_val as f32).round() as u32;

        let bit_w = w / n_bits;
        let bit_w = bit_w.max(1);

        for bit in 0..n_bits {
            // MSB on the left.
            let bit_idx = n_bits - 1 - bit;
            let on = (val >> bit_idx) & 1 == 1;
            let x0 = bit * bit_w;
            let bw = bit_w.saturating_sub(1).max(1);

            if on {
                // Lit bit: full column.
                draw::fill_rect(grid, x0, 0, bw, h);
                // Tint.
                let t = bit as f32 / n_bits.max(1) as f32;
                let color = ctx.palette.sample(t);
                let cx = (x0 / 2).min(cells_w.saturating_sub(1));
                for cy in 0..cells_h {
                    draw::tint_row(grid, cy, cx, cx, color);
                }
            } else {
                // Unlit bit: just the bottom dot.
                draw::dot(grid, x0, h.saturating_sub(1));
            }
        }

        // Time-based flicker on the LSB to suggest counting.
        let lsb_x = (n_bits - 1) * bit_w;
        if (ctx.time * 8.0).fract() > 0.5 {
            draw::vline(grid, lsb_x, 0, h.saturating_sub(1));
        }
        Ok(())
    }
}

// ─── 10. Heartbeat / EKG ────────────────────────────────────────────────────

/// An EKG trace advances with progress. A sharp spike pulses once per second
/// driven by `ctx.time`; the baseline has filled up to `ctx.eased`.
struct Heartbeat;
impl ProgressStyle for Heartbeat {
    fn name(&self) -> &str {
        "heartbeat"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "EKG heartbeat line pulses in real time; trace advances with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let filled = ((ctx.eased * w as f32) as usize).min(w);
        let base = (h * 2 / 3).min(h.saturating_sub(1));

        // Draw baseline up to the filled point.
        draw::hline(grid, 0, filled.saturating_sub(1), base);

        // EKG spike: a repeating sharp bump that scrolls at ctx.time.
        // Spike width in dots.
        let spike_w = (w / 6).max(4);
        // Phase: spike repeats every 1.0 seconds.
        let phase = ctx.time.fract();
        // Spike head position (leading edge of the filled region, scrolling).
        let spike_center = if filled > spike_w {
            let travel = filled - spike_w;
            let scroll = (phase * travel as f32) as usize;
            scroll + spike_w / 2
        } else {
            filled / 2
        };

        // Draw the spike shape: /\_ with sharp peak.
        let peak_h = (h as f32 * 0.85) as usize;
        for dx in 0..spike_w {
            let x = spike_center.saturating_sub(spike_w / 2) + dx;
            if x >= w {
                break;
            }
            // Normalised position within spike [0,1].
            let t = dx as f32 / spike_w.max(1) as f32;
            let y_offset: i32 = if t < 0.25 {
                // Rising: base → peak.
                let rise = t / 0.25;
                -((rise * peak_h as f32) as i32)
            } else if t < 0.5 {
                // Falling from peak: peak → below base (the "S" dip).
                let fall = (t - 0.25) / 0.25;
                -(((1.0 - fall) * peak_h as f32) as i32)
            } else if t < 0.65 {
                // Small negative dip.
                let dip = (t - 0.5) / 0.15;
                (dip * h as f32 * 0.15) as i32
            } else {
                0
            };
            let y = (base as i32 + y_offset).clamp(0, h as i32 - 1);
            draw::dot(grid, x, y as usize);
        }

        // Tint.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = cx as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}

// ─── 11. Circuit trace ───────────────────────────────────────────────────────

/// A circuit-board trace routes across the bar: horizontal runs punctuated by
/// 90-degree turns and junction dots. The lit portion grows with eased progress
/// and "current" pulses along the trace driven by `ctx.time`.
struct CircuitTrace;
impl ProgressStyle for CircuitTrace {
    fn name(&self) -> &str {
        "circuit-trace"
    }
    fn theme(&self) -> &str {
        "tech"
    }
    fn describe(&self) -> &str {
        "Circuit board trace with routing turns, lit by progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (cells_w, cells_h) = grid.dimensions();

        let filled = ((ctx.eased * w as f32) as usize).min(w);

        // Build a simple repeating trace: horizontal run, then a vertical
        // "via" jog, then continue. Segment length varies by column using hash.
        let seg_len = (w / 8).max(4);
        let mut x = 0usize;
        let mut y = h / 2;
        let mut up = true; // next jog direction.

        while x < filled {
            // Horizontal run.
            let run = seg_len + (hash(x as u32 * 3 + 17) % seg_len as u32) as usize;
            let run_end = (x + run).min(filled);
            draw::hline(grid, x, run_end.saturating_sub(1), y);
            x = run_end;
            if x >= filled {
                break;
            }

            // Vertical jog (via).
            let jog = (h / 3).max(1);
            let (y0, y1) = if up {
                let new_y = y.saturating_sub(jog);
                (new_y, y)
            } else {
                let new_y = (y + jog).min(h.saturating_sub(1));
                (y, new_y)
            };
            draw::vline(grid, x.saturating_sub(1), y0, y1);
            // Junction dot (pad).
            draw::dot(grid, x.saturating_sub(1), y0);
            draw::dot(grid, x.saturating_sub(1), y1);
            y = if up {
                y.saturating_sub(jog)
            } else {
                (y + jog).min(h.saturating_sub(1))
            };
            up = !up;
        }

        // "Current pulse": a bright dot riding the trace, driven by time.
        let pulse_x = ((ctx.time * 0.7).fract() * filled as f32) as usize;
        let pulse_x = pulse_x.min(filled.saturating_sub(1));
        // Draw a 3-dot flare around the pulse.
        for dx in 0..3usize {
            let px = pulse_x.saturating_sub(1) + dx;
            if px < w {
                draw::dot(grid, px, h / 2);
            }
        }

        // Tint the lit region.
        let filled_cells = (ctx.eased * cells_w as f32) as usize;
        for cx in 0..filled_cells.min(cells_w) {
            let t = cx as f32 / cells_w.max(1) as f32;
            let color = ctx.palette.sample(t);
            for cy in 0..cells_h {
                draw::tint_row(grid, cy, cx, cx, color);
            }
        }
        Ok(())
    }
}
