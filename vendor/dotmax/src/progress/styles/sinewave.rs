//! Sine-wave progress bars for dotmax — a full family spanning smooth↔blocky.
//!
//! Every style in this theme is driven by sine-wave mathematics, but each
//! exploits a *structurally distinct* form of sinusoidal behavior:
//!
//! | Name | Waveform / Structure |
//! |------|----------------------|
//! | `sw-traveling` | y = A·sin(kx − ωt) scrolling; area below filled |
//! | `sw-sine-scroll` | demoscene bobbing markers on a rippling baseline |
//! | `sw-am` | amplitude-modulated: envelope·carrier, A(x)·sin(ωx) |
//! | `sw-chirp` | frequency chirp/sweep: freq rises across x, eased sets max |
//! | `sw-wave-packet` | Gaussian-windowed sinusoid traveling with time |
//! | `sw-harmonics` | N-harmonic superposition; N = 1 + eased·8 |
//! | `sw-barber-pole` | phase-gradient columns → diagonal stripes scrolling in time |
//! | `sw-area-fill` | region under |sin| fills up to eased (smooth braille) |
//! | `sw-blocky-eq` | same sine rendered as vblock columns (blocky counterpart) |
//! | `sw-rectified` | |sin| hard-clipped to threshold — rectified ripple |
//! | `sw-damped` | e^(−γx)·sin(ωx) ring-down from left edge |
//! | `sw-standing` | 2A·sin(kx)·cos(ωt) with nodes fixed in space |
//! | `sw-density` | dot density per column ∝ |sin| — dithered shading texture |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// All styles in the `sinewave` theme.
///
/// Returns 13 structurally distinct sine-wave bars. Each style exploits a
/// different mathematical form: traveling waves, AM envelopes, chirps,
/// Gaussian packets, harmonic superposition, phase gradients, area fills,
/// blocky quantization, rectification, ring-down damping, standing waves,
/// and density textures. Color does not substitute for structural variety.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Traveling),
        Box::new(SineScroll),
        Box::new(AmplitudeMod),
        Box::new(FreqChirp),
        Box::new(WavePacket),
        Box::new(Harmonics),
        Box::new(BarberPole),
        Box::new(AreaFill),
        Box::new(BlockyEq),
        Box::new(Rectified),
        Box::new(Damped),
        Box::new(StandingEnvelope),
        Box::new(Density),
    ]
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Draw a connected curve: for each x column compute y, connect vertically to
/// prior column to prevent single-dot gaps.
#[inline]
fn draw_curve<F>(grid: &mut BrailleGrid, w: usize, h: usize, f: F)
where
    F: Fn(usize) -> i32,
{
    let mut prev: Option<i32> = None;
    for xi in 0..w {
        let dy = f(xi).clamp(0, h as i32 - 1);
        draw::dot_i(grid, xi as i32, dy);
        if let Some(py) = prev {
            let lo = py.min(dy);
            let hi = py.max(dy);
            for yy in lo..=hi {
                draw::dot_i(grid, xi as i32, yy);
            }
        }
        prev = Some(dy);
    }
}

/// Tint the filled-cells region with a gradient.
#[inline]
fn tint_filled(grid: &mut BrailleGrid, ctx: &BarContext) {
    let (cw, ch) = grid.dimensions();
    let filled = (ctx.eased * cw as f32).round() as usize;
    for cx in 0..filled.min(cw) {
        let t = if filled <= 1 {
            0.5
        } else {
            cx as f32 / (filled - 1) as f32
        };
        let col = ctx.palette.sample(t);
        for cy in 0..ch {
            draw::tint_row(grid, cy, cx, cx, col);
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Traveling wave: y = A·sin(kx − ωt)
//    The wave scrolls rightward as time increases.
//    Progress controls how much of the bar is "filled" below the wave.
// ---------------------------------------------------------------------------
struct Traveling;
impl ProgressStyle for Traveling {
    fn name(&self) -> &str {
        "sw-traveling"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Traveling wave y=A·sin(kx−ωt): scrolls rightward; progress fills the swept area below"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let amp = h as f32 * 0.38;
        let k = 2.0 * PI * 3.0 / w as f32; // 3 full cycles across the bar
        let omega = 2.0 * PI * 1.2; // rad/s
        let mid = (h / 2) as i32;
        let fill_x = (ctx.eased * w as f32).round() as usize; // rightward fill boundary

        // Filled region: solid dots from baseline to the wave surface
        for xi in 0..fill_x.min(w) {
            let theta = k * xi as f32 - omega * ctx.time;
            let wave_y = (mid - (amp * theta.sin()) as i32).clamp(0, h as i32 - 1);
            // Fill from wave surface down to bottom
            let top = wave_y.min(mid).max(0) as usize;
            let bot = wave_y.max(mid).min(h as i32 - 1) as usize;
            for y in top..=bot {
                draw::dot(grid, xi, y);
            }
        }

        // Unfilled region: just the wave outline
        draw_curve(grid, w, h, |xi| {
            let theta = k * xi as f32 - omega * ctx.time;
            (mid - (amp * theta.sin()) as i32).clamp(0, h as i32 - 1)
        });

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 2. Sine-scroll (demoscene): a row of markers bob on a sine baseline.
//    The baseline itself ripples — sine of a sine. Markers are vertical
//    strokes at regular x positions, their y driven by sin(x + t).
// ---------------------------------------------------------------------------
struct SineScroll;
impl ProgressStyle for SineScroll {
    fn name(&self) -> &str {
        "sw-sine-scroll"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Demoscene sine-scroller: markers bob on a rippling baseline — classic 8-bit demo effect"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let amp = (h as f32 * 0.30).max(1.0);
        let mid = h as f32 / 2.0;
        let speed = 3.0_f32;
        let k1 = 2.0 * PI * 2.5 / w as f32; // baseline ripple freq
        let k2 = 2.0 * PI * 5.0 / w as f32; // marker bobbing freq (faster)

        // Rippling baseline
        draw_curve(grid, w, h, |xi| {
            let base_y = mid + amp * 0.4 * (k1 * xi as f32 - speed * ctx.time).sin();
            base_y as i32
        });

        // Marker positions: evenly spaced, limited by progress
        let n_markers = ((w / 4).max(1)).min(w);
        let markers_shown = (ctx.eased * n_markers as f32).ceil() as usize;
        for m in 0..markers_shown.min(n_markers) {
            let xi = (m * w / n_markers.max(1)).min(w - 1);
            let bob_y = mid + amp * (k2 * xi as f32 - speed * 1.3 * ctx.time).sin();
            let bob_y = bob_y.clamp(0.0, (h - 1) as f32) as usize;
            // Vertical stroke marker (3 dots tall)
            let top = bob_y.saturating_sub(1);
            let bot = (bob_y + 1).min(h - 1);
            draw::vline(grid, xi, top, bot);
            // Horizontal crossbar
            if xi > 0 {
                draw::dot(grid, xi - 1, bob_y);
            }
            if xi + 1 < w {
                draw::dot(grid, xi + 1, bob_y);
            }
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Amplitude modulation: A(x) · sin(ωx + φt)
//    Envelope A(x) is a sine arch over [0, eased·L] — trapezoidal AM shape.
//    Carrier frequency is constant; the envelope reveals with progress.
// ---------------------------------------------------------------------------
struct AmplitudeMod;
impl ProgressStyle for AmplitudeMod {
    fn name(&self) -> &str {
        "sw-am"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Amplitude modulation: arch-shaped envelope × carrier sine; envelope width grows with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h / 2) as i32;
        let max_amp = (h as f32 * 0.44).max(1.0);
        let omega = 2.0 * PI * 6.0 / w as f32; // carrier: 6 cycles
        let phi = ctx.time * 2.0 * PI * 0.8; // slow phase drift
                                             // Envelope boundary: progress sets how far the AM arch extends
        let env_end = ctx.eased * w as f32;

        draw_curve(grid, w, h, |xi| {
            let xf = xi as f32;
            // Envelope: sin arch over [0, env_end], 0 outside
            let env = if xf <= env_end && env_end > 0.0 {
                (PI * xf / env_end).sin().max(0.0)
            } else {
                0.0
            };
            let carrier = (omega * xf + phi).sin();
            let val = env * carrier;
            (mid - (max_amp * val) as i32).clamp(0, h as i32 - 1)
        });

        // Draw envelope outline (upper and lower silhouette)
        for xi in 0..w {
            let xf = xi as f32;
            let env = if xf <= env_end && env_end > 0.0 {
                (PI * xf / env_end).sin().max(0.0)
            } else {
                0.0
            };
            let e = (max_amp * env) as i32;
            draw::dot_i(grid, xi as i32, (mid - e).clamp(0, h as i32 - 1));
            draw::dot_i(grid, xi as i32, (mid + e).clamp(0, h as i32 - 1));
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Frequency chirp: sin(k(x)·x)  where k(x) = k_min + (k_max−k_min)·x/L
//    Frequency increases linearly from left to right. eased sets k_max.
//    Time shifts the instantaneous phase: chirp "sweeps" as a whole.
// ---------------------------------------------------------------------------
struct FreqChirp;
impl ProgressStyle for FreqChirp {
    fn name(&self) -> &str {
        "sw-chirp"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Frequency chirp: low→high sweep, compression visible from left; progress raises max frequency"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.44).max(1.0);
        let k_min = 2.0 * PI * 1.0 / w as f32; // 1 cycle at left
        let k_max = 2.0 * PI * (1.0 + ctx.eased * 9.0) / w as f32; // up to 10 cycles at right
        let phase = ctx.time * 2.0 * PI * 0.5; // global phase offset

        draw_curve(grid, w, h, |xi| {
            let xf = xi as f32;
            // Instantaneous phase: integral of k(x') dx' from 0 to x = k_min·x + (k_max−k_min)·x²/(2L)
            let inst_phase =
                k_min * xf + (k_max - k_min) * xf * xf / (2.0 * w.max(1) as f32) + phase;
            let val = inst_phase.sin();
            (mid - (amp * val) as i32).clamp(0, h as i32 - 1)
        });

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Wave packet: Gaussian-windowed sinusoid.
//    A(x) = exp(−((x − x₀)/σ)²) · sin(k·(x − x₀) + φ)
//    x₀ = eased × L (packet centre), σ = L/5, φ = ωt (carrier oscillates).
//    Distinct from waves::WavePacket: here the Gaussian envelope is drawn as
//    solid fill under the packet, not just a traced curve.
// ---------------------------------------------------------------------------
struct WavePacket;
impl ProgressStyle for WavePacket {
    fn name(&self) -> &str {
        "sw-wave-packet"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Gaussian wave packet: solid filled envelope travels with progress; carrier fringes oscillate"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w as f32;
        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.44).max(1.0);
        let x0 = ctx.eased * wf;
        let sigma = (wf / 5.0).max(1.0);
        let k = 10.0 * PI / wf;
        let phi = ctx.time * 3.0 * PI;

        // Solid fill: between midline and wave surface, weighted by envelope
        for xi in 0..w {
            let xf = xi as f32;
            let dx = xf - x0;
            let env = (-(dx * dx) / (2.0 * sigma * sigma)).exp();
            if env < 0.01 {
                continue;
            }
            let wave_y = (mid - (amp * env * (k * dx + phi).sin()) as i32).clamp(0, h as i32 - 1);
            let top = wave_y.min(mid).max(0) as usize;
            let bot = wave_y.max(mid).min(h as i32 - 1) as usize;
            for y in top..=bot {
                draw::dot(grid, xi, y);
            }
        }

        // Gaussian envelope outline
        for xi in 0..w {
            let xf = xi as f32;
            let dx = xf - x0;
            let env = (-(dx * dx) / (2.0 * sigma * sigma)).exp();
            let ey = (amp * env) as i32;
            draw::dot_i(grid, xi as i32, (mid - ey).clamp(0, h as i32 - 1));
            draw::dot_i(grid, xi as i32, (mid + ey).clamp(0, h as i32 - 1));
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Harmonic superposition: y = Σ_{n=1}^{N} sin(n·θ) / n
//    N = 1 + floor(eased·8), so progress "adds" harmonics one by one.
//    Time provides a common phase offset so the whole composite wave scrolls.
//    At N=1 it's a clean sine; at N=9 it approximates a sawtooth-ish shape.
// ---------------------------------------------------------------------------
struct Harmonics;
impl ProgressStyle for Harmonics {
    fn name(&self) -> &str {
        "sw-harmonics"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Harmonic superposition: each harmonic unlocks with progress, morphing sine toward a complex wave"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.44).max(1.0);
        let n_harm = (1 + (ctx.eased * 8.0).floor() as usize).min(9);
        let phase_off = ctx.time * 2.0 * PI * 0.4; // scroll speed

        // Normalization: max theoretical amplitude = Σ 1/n
        let norm: f32 = (1..=n_harm).map(|n| 1.0 / n as f32).sum::<f32>().max(1.0);

        draw_curve(grid, w, h, |xi| {
            let theta = (xi as f32 / w.max(1) as f32) * 2.0 * PI * 3.0 + phase_off;
            let val: f32 = (1..=n_harm)
                .map(|n| (n as f32 * theta).sin() / n as f32)
                .sum::<f32>()
                / norm;
            (mid - (amp * val) as i32).clamp(0, h as i32 - 1)
        });

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Phase-gradient barber-pole: each column x has phase φ(x) = 2π·x/L·M
//    producing M diagonal stripe repeats. Time offsets all phases → stripes
//    scroll diagonally. Cells are lit if sin(φ(x) − ωt + 2π·y/h·M) > thresh.
// ---------------------------------------------------------------------------
struct BarberPole;
impl ProgressStyle for BarberPole {
    fn name(&self) -> &str {
        "sw-barber-pole"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Phase-gradient barber-pole: diagonal stripes scroll as time flows — each column a shifted sine"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let wf = w.max(1) as f32;
        let hf = h.max(1) as f32;
        let stripes = 4.0_f32; // number of diagonal stripe repeats
        let omega = 2.0 * PI * 1.5; // scroll speed
        let thresh = 0.3_f32;
        // Fill gate: only dots in [0, fill_x) are lit
        let fill_x = (ctx.eased * wf).round() as usize;

        for xi in 0..fill_x.min(w) {
            for yi in 0..h {
                let xn = xi as f32 / wf;
                let yn = yi as f32 / hf;
                // Phase gradient across x + y gives diagonal stripes
                let phase = 2.0 * PI * stripes * (xn + yn) - omega * ctx.time;
                if phase.sin() > thresh {
                    draw::dot(grid, xi, yi);
                }
            }
        }

        // Unfilled region: dim ghost stripes (only column outline)
        for xi in fill_x.min(w)..w {
            for yi in 0..h {
                let xn = xi as f32 / wf;
                let yn = yi as f32 / hf;
                let phase = 2.0 * PI * stripes * (xn + yn) - omega * ctx.time;
                // Only light the bright peak in unfilled area
                if phase.sin() > 0.85 {
                    draw::dot(grid, xi, yi);
                }
            }
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Sine area fill (smooth braille): the region 0 ≤ y ≤ |sin(kx)| · h · eased
//    is filled with braille dots. Pure smooth rendering — no block glyphs.
//    Time animates the phase so the filled ripple rolls.
// ---------------------------------------------------------------------------
struct AreaFill;
impl ProgressStyle for AreaFill {
    fn name(&self) -> &str {
        "sw-area-fill"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Sine area fill: |sin| landscape fills smoothly with braille dots up to eased height"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let hf = h as f32;
        let k = 2.0 * PI * 4.0 / w.max(1) as f32; // 4 bumps across bar
        let phase = ctx.time * 2.0 * PI * 0.6;

        for xi in 0..w {
            let xf = xi as f32;
            let raw = (k * xf + phase).sin().abs(); // ∈ [0, 1]
                                                    // scale by eased to fill progressively
            let fill_h = (raw * ctx.eased * hf).round() as usize;
            let fill_h = fill_h.min(h);
            let y0 = h.saturating_sub(fill_h);
            for y in y0..h {
                draw::dot(grid, xi, y);
            }
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Blocky equalizer (explicit blocky counterpart of AreaFill):
//    The same |sin| amplitude computed per CELL column, then quantized to
//    block-eighths via draw::vblock — character-cell rendering only, no dots.
// ---------------------------------------------------------------------------
struct BlockyEq;
impl ProgressStyle for BlockyEq {
    fn name(&self) -> &str {
        "sw-blocky-eq"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Blocky sine equalizer: |sin| quantized to ▁▂▃▄▅▆▇█ columns — character-cell only, zero dots"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }
        let k = 2.0 * PI * 4.0 / cw.max(1) as f32;
        let phase = ctx.time * 2.0 * PI * 0.6;
        let fill_cells = (ctx.eased * cw as f32).round() as usize;

        for cx in 0..cw {
            let xf = cx as f32;
            let amp = if cx < fill_cells {
                (k * xf + phase).sin().abs() // ∈ [0, 1]
            } else {
                // Ghost: dim un-filled columns
                (k * xf + phase).sin().abs() * 0.25
            };
            // Map to eighths per cell row
            let total_eighths = (amp * (ch * 8) as f32).round() as usize;
            let full_rows = total_eighths / 8;
            let rem_eighths = total_eighths % 8;

            // Fill from bottom up: full rows + partial top row
            for row_from_bot in 0..full_rows.min(ch) {
                let cy = ch.saturating_sub(1 + row_from_bot);
                draw::vblock(grid, cx, cy, 8);
            }
            if rem_eighths > 0 {
                let cy = ch.saturating_sub(1 + full_rows.min(ch));
                if full_rows < ch {
                    draw::vblock(grid, cx, cy, rem_eighths);
                }
            }
        }

        // Tint filled cells
        let (_, ch2) = grid.dimensions();
        for cx in 0..fill_cells.min(cw) {
            let t = if fill_cells <= 1 {
                0.5
            } else {
                cx as f32 / (fill_cells - 1) as f32
            };
            let col = ctx.palette.sample(t);
            for cy in 0..ch2 {
                draw::tint_row(grid, cy, cx, cx, col);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Rectified / clipped sine: y = clip(|sin(kx − ωt)|, threshold)
//     The waveform is always ≥ 0 (rectified); a hard top-clip flattens peaks.
//     Structurally distinct: no zero-crossings, flat tops, scalloped pattern.
//     eased sets the clip ceiling: low eased → shaved flat; high → full humps.
// ---------------------------------------------------------------------------
struct Rectified;
impl ProgressStyle for Rectified {
    fn name(&self) -> &str {
        "sw-rectified"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Rectified & clipped sine: |sin| with a hard ceiling — scalloped humps, no zero-crossings"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let hf = h as f32;
        let k = 2.0 * PI * 5.0 / w.max(1) as f32; // 5 humps
        let omega = 2.0 * PI * 1.1;
        // Clip ceiling: rises with eased (more of the hump is shown)
        let ceiling = (0.2 + ctx.eased * 0.8).min(1.0);

        for xi in 0..w {
            let xf = xi as f32;
            // Rectified and clipped in [0, ceiling]
            let val = (k * xf - omega * ctx.time).sin().abs().min(ceiling);
            let fill_h = (val / ceiling.max(0.001) * hf).round() as usize;
            let fill_h = fill_h.min(h);
            let y0 = h.saturating_sub(fill_h);
            for y in y0..h {
                draw::dot(grid, xi, y);
            }
        }

        // Draw the clip line across the top as a visible ceiling
        let clip_y = (hf * (1.0 - ceiling)).round() as usize;
        if clip_y < h {
            draw::hline(grid, 0, w.saturating_sub(1), clip_y);
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Damped oscillation: y = e^(−γ·x) · sin(ω·x + φt)
//     Ring-down from the left edge. eased controls decay rate γ:
//     low eased → slow decay (many visible oscillations); high → fast ring-down.
//     Time provides a phase shift so the carrier ripples continuously.
// ---------------------------------------------------------------------------
struct Damped;
impl ProgressStyle for Damped {
    fn name(&self) -> &str {
        "sw-damped"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Damped oscillation e^(−γx)·sin(ωx): ring-down from left; decay rate rises with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.44).max(1.0);
        let gamma = 0.5 + ctx.eased * 4.0; // decay rate: 0.5..4.5
        let omega = 2.0 * PI * 6.0 / w.max(1) as f32; // 6 cycles at zero decay
        let phi = ctx.time * 2.0 * PI * 0.7; // carrier phase drift

        draw_curve(grid, w, h, |xi| {
            let xn = xi as f32 / w.max(1) as f32; // [0, 1]
            let env = (-gamma * xn).exp();
            let val = env * (omega * xi as f32 + phi).sin();
            (mid - (amp * val) as i32).clamp(0, h as i32 - 1)
        });

        // Draw the decaying envelope outline
        for xi in 0..w {
            let xn = xi as f32 / w.max(1) as f32;
            let env = (-gamma * xn).exp();
            let ey = (amp * env) as i32;
            draw::dot_i(grid, xi as i32, (mid - ey).clamp(0, h as i32 - 1));
            draw::dot_i(grid, xi as i32, (mid + ey).clamp(0, h as i32 - 1));
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 12. Standing wave envelope: 2A·sin(kx)·cos(ωt)
//     Nodes are fixed in space; antinodes breathe in time.
//     eased selects mode n = 1 + floor(eased·5); each mode adds one more node.
//     Fills the area between the upper and lower envelope with dots.
// ---------------------------------------------------------------------------
struct StandingEnvelope;
impl ProgressStyle for StandingEnvelope {
    fn name(&self) -> &str {
        "sw-standing"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Standing wave: fixed-node antinodes breathe with time; mode count rises with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let mid = (h / 2) as i32;
        let amp = (h as f32 * 0.44).max(1.0);
        let mode = (1 + (ctx.eased * 5.0).floor() as usize).min(6);
        let k = mode as f32 * PI / w.max(1) as f32;
        let omega = 2.5 * PI;
        let breath = (omega * ctx.time).cos(); // ∈ [-1, 1]

        // Fill between upper and lower envelope
        for xi in 0..w {
            let spatial = (k * xi as f32).sin(); // spatial factor
            let upper_y = (mid - (amp * spatial * breath.abs()) as i32).clamp(0, h as i32 - 1);
            let lower_y = (mid + (amp * spatial * breath.abs()) as i32).clamp(0, h as i32 - 1);
            let top = upper_y.min(lower_y) as usize;
            let bot = upper_y.max(lower_y) as usize;
            for y in top..=bot {
                draw::dot(grid, xi, y);
            }
        }

        // Draw the traveling wave itself (sign preserved for direction)
        draw_curve(grid, w, h, |xi| {
            let val = 2.0 * (k * xi as f32).sin() * breath;
            (mid - (amp * val) as i32).clamp(0, h as i32 - 1)
        });

        // Mark nodes with tiny vertical ticks
        for n in 0..=mode {
            let node_x = (n as f32 / mode as f32 * w.max(1) as f32) as usize;
            if node_x < w {
                let tick = (h / 8).max(1);
                draw::vline(
                    grid,
                    node_x,
                    (mid as usize).saturating_sub(tick),
                    ((mid as usize) + tick).min(h - 1),
                );
            }
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 13. Sine-density texture: dot density in each column ∝ |sin(kx − ωt)|.
//     Each column gets a random-looking dither pattern with density set by the
//     sine value. Uses a row-hash to scatter dots vertically — no block glyphs.
//     Progress gates which columns participate: unfilled cols are near-empty.
// ---------------------------------------------------------------------------
struct Density;
impl ProgressStyle for Density {
    fn name(&self) -> &str {
        "sw-density"
    }
    fn theme(&self) -> &str {
        "sinewave"
    }
    fn describe(&self) -> &str {
        "Sine-density texture: dot density per column ∝ |sin| — dithered shading, no curves drawn"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let k = 2.0 * PI * 5.0 / w.max(1) as f32;
        let omega = 2.0 * PI * 0.9;
        let fill_x = (ctx.eased * w as f32).round() as usize;

        for xi in 0..w {
            let density = if xi < fill_x {
                (k * xi as f32 - omega * ctx.time).sin().abs() // ∈ [0, 1]
            } else {
                // Dim ghost in unfilled region
                (k * xi as f32 - omega * ctx.time).sin().abs() * 0.15
            };
            let lit_rows = (density * h as f32).round() as usize;
            // Scatter dots using a deterministic hash: row is lit if
            // (xi * 7 + yi * 13) mod prime < lit_rows * prime / h
            // This avoids vertical run artifacts.
            let prime = 31usize;
            let thresh = lit_rows * prime / h.max(1);
            for yi in 0..h {
                let hash = (xi.wrapping_mul(7).wrapping_add(yi.wrapping_mul(13))) % prime;
                if hash < thresh {
                    draw::dot(grid, xi, yi);
                }
            }
        }

        tint_filled(grid, ctx);
        Ok(())
    }
}
