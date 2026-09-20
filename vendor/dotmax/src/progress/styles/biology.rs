//! Biology-themed progress bars for dotmax.
//!
//! Twelve animated braille bars each rooted in a distinct biological concept:
//! DNA replication, cell division, neural firing, logistic growth, cardiac
//! rhythm, protein folding, photosynthesis, phylogeny, hemodynamics, enzyme
//! kinetics, viral spread, and ion-channel pumping. Every bar is structurally
//! different — color alone is never the only distinguishing feature.
//!
//! ## Styles
//!
//! | Name                  | Concept                                              |
//! |-----------------------|------------------------------------------------------|
//! | `dna-helix`           | Double helix with rotating strands and base-pair rungs |
//! | `mitosis`             | Single cell pinches and divides into two             |
//! | `action-potential`    | PQRST-like spike travelling along an axon            |
//! | `logistic-growth`     | S-curve fill with carrying-capacity ceiling          |
//! | `ecg-heartbeat`       | Scrolling ECG waveform; beats counted = progress     |
//! | `protein-folding`     | Chain collapses from extended to compact blob        |
//! | `photosynthesis`      | Sun rays hit leaf; sugar store fills                 |
//! | `phylogenetic-tree`   | Bifurcating tree grows outward with eased            |
//! | `blood-flow`          | Red cells stream through a vessel with pulse         |
//! | `enzyme-kinetics`     | Substrate binds active site; product count = eased   |
//! | `virus-spread`        | Infection sweeps a tissue grid cell by cell          |
//! | `ion-channels`        | Ion pumps cycle; electrochemical gradient builds     |

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, DotmaxError};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// All styles in the `biology` theme.
///
/// Returns 12 boxed bars, each embodying a structurally distinct biological
/// process. Safe to render from a 1×1 cell grid up to 80×8 or larger; every
/// function uses saturating arithmetic and `draw::dot_i` for bounds safety.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(DnaHelix),
        Box::new(Mitosis),
        Box::new(ActionPotential),
        Box::new(LogisticGrowth),
        Box::new(EcgHeartbeat),
        Box::new(ProteinFolding),
        Box::new(Photosynthesis),
        Box::new(PhylogeneticTree),
        Box::new(BloodFlow),
        Box::new(EnzymeKinetics),
        Box::new(VirusSpread),
        Box::new(IonChannels),
    ]
}

// ---------------------------------------------------------------------------
// Shared helper: Bresenham-style line between two signed dot coordinates
// ---------------------------------------------------------------------------
fn line_i(grid: &mut BrailleGrid, x0: i32, y0: i32, x1: i32, y1: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let steps = dx.max(dy).max(1);
    for i in 0..=steps {
        let x = x0 + (x1 - x0) * i / steps;
        let y = y0 + (y1 - y0) * i / steps;
        draw::dot_i(grid, x, y);
    }
}

// ---------------------------------------------------------------------------
// 1. DNA Double Helix
//    Two sine strands π out of phase; horizontal base-pair rungs connect them.
//    Rotates via time (simulates 3-D twist); unzipping revealed by eased.
// ---------------------------------------------------------------------------
struct DnaHelix;
impl ProgressStyle for DnaHelix {
    fn name(&self) -> &str {
        "dna-helix"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "DNA double helix: two counter-rotating sine strands with base-pair rungs; unzips with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = (h / 2) as f32;
        let amp = (h as f32 * 0.42).max(1.0);
        // One full helix period spans the bar; time advances phase for 3-D rotation feel.
        let phase_offset = ctx.time * 1.4;
        // How far the helix is "unzipped" — eased controls it left-to-right.
        let unzip_x = (ctx.eased * w as f32) as usize;

        let mut prev_a: Option<i32> = None;
        let mut prev_b: Option<i32> = None;

        for xi in 0..w {
            let xf = xi as f32 / w as f32;
            let angle = xf * 4.0 * PI + phase_offset;

            let ya = (mid + amp * angle.sin()) as i32;
            let yb = (mid + amp * (angle + PI).sin()) as i32;

            // Draw strand A (continuous curve)
            draw::dot_i(grid, xi as i32, ya);
            if let Some(py) = prev_a {
                let (lo, hi) = (py.min(ya), py.max(ya));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_a = Some(ya);

            // Draw strand B (phase-flipped)
            draw::dot_i(grid, xi as i32, yb);
            if let Some(py) = prev_b {
                let (lo, hi) = (py.min(yb), py.max(yb));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_b = Some(yb);

            // Base-pair rungs every ~8 dots, only in the zipped region
            if xi >= unzip_x && xi % 8 == 0 {
                let rung_lo = ya.min(yb).max(0) as usize;
                let rung_hi = ya.max(yb).min(h as i32 - 1) as usize;
                draw::vline(grid, xi, rung_lo, rung_hi);
            }
        }

        // Tint: filled region gets palette gradient
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
// 2. Mitosis
//    A single circle starts centered. As eased→1 it pinches at the equator
//    and separates into two daughter cells. The cleavage furrow deepens
//    proportionally with eased.
// ---------------------------------------------------------------------------
struct Mitosis;
impl ProgressStyle for Mitosis {
    fn name(&self) -> &str {
        "mitosis"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Cell mitosis: a circle pinches at its equator and divides into two daughter cells"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid_y = (h / 2) as f32;
        let base_r = ((w.min(h) as f32) * 0.30).max(2.0);

        // eased 0→0.5: shrink horizontally (pinch), 0.5→1: split apart
        let e = ctx.eased;
        let pinch = (e * 2.0).min(1.0); // 0→1 over first half
        let split = ((e - 0.5) * 2.0).max(0.0); // 0→1 over second half

        // Horizontal squeeze factor: cell flattens at equator
        let h_scale = 1.0 - pinch * 0.45;
        // Vertical elongation before split
        let v_scale = 1.0 + pinch * 0.2;
        // Separation of the two daughter nuclei
        let sep = split * base_r * 1.8;

        let draw_cell = |grid: &mut BrailleGrid, cx: f32, cy: f32, rx: f32, ry: f32| {
            let steps = ((rx + ry) as usize * 8).max(16);
            for s in 0..=steps {
                let angle = s as f32 / steps as f32 * 2.0 * PI;
                let px = (cx + rx * angle.cos()) as i32;
                let py = (cy + ry * angle.sin()) as i32;
                draw::dot_i(grid, px, py);
            }
        };

        if split < 0.01 {
            // Single cell, possibly pinching
            let rx = base_r * h_scale;
            let ry = base_r * v_scale;
            let cx = (w / 2) as f32;
            draw_cell(grid, cx, mid_y, rx, ry);
            // Cleavage furrow: two horizontal dents closing in
            let furrow_depth = (pinch * ry * 0.5) as i32;
            let fx = cx as i32;
            for d in 0..furrow_depth {
                draw::dot_i(grid, fx - d, mid_y as i32);
                draw::dot_i(grid, fx + d, mid_y as i32);
            }
        } else {
            // Two daughter cells separating
            let r = base_r * 0.8;
            let cx1 = (w as f32 / 2.0) - sep;
            let cx2 = (w as f32 / 2.0) + sep;
            draw_cell(grid, cx1, mid_y, r, r);
            draw_cell(grid, cx2, mid_y, r, r);
            // Nucleus dot in each
            draw::dot_i(grid, cx1 as i32, mid_y as i32);
            draw::dot_i(grid, cx2 as i32, mid_y as i32);
        }

        // Tint across the whole grid keyed by progress
        let (cw, ch) = grid.dimensions();
        for cy in 0..ch {
            let col = ctx.palette.sample(ctx.eased);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), col);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Neuron Action Potential
//    An axon runs as a horizontal baseline. A travelling spike (−70→+40→hyperpolarise)
//    scrolls left via time; spike count accumulated = eased.
// ---------------------------------------------------------------------------
struct ActionPotential;
impl ProgressStyle for ActionPotential {
    fn name(&self) -> &str {
        "action-potential"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Neuron action potential: a travelling voltage spike scrolls along the axon; spikes fired = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let baseline_y = (h as f32 * 0.72) as usize; // resting potential sits low
        let spike_amp = (h as f32 * 0.70).max(1.0);

        // Axon baseline
        draw::hline(grid, 0, w.saturating_sub(1), baseline_y);

        // Soma bump on the left
        let soma_r = (h as f32 * 0.25).max(1.0) as i32;
        for dy in -soma_r..=soma_r {
            let dist = (dy.abs() as f32 / soma_r as f32).powi(2);
            let dx = ((1.0 - dist).max(0.0).sqrt() * soma_r as f32) as i32;
            for ddx in -dx..=dx {
                draw::dot_i(grid, ddx, baseline_y as i32 + dy);
            }
        }

        // Wave shape: resting(−70)=0, depol(+40)=+1, repol(0)=0, hyperpol(−90)=−0.2, return=0
        // Parameterised over local phase [0, 1)
        let spike_shape = |phase: f32| -> f32 {
            if phase < 0.15 {
                // Rising depolarisation
                (phase / 0.15) * 1.0
            } else if phase < 0.30 {
                // Falling repolarisation
                1.0 - ((phase - 0.15) / 0.15) * 1.2
            } else if phase < 0.45 {
                // Hyperpolarisation undershoot
                -0.2 * (1.0 - ((phase - 0.30) / 0.15))
            } else {
                0.0
            }
        };

        // Spike repetition period in seconds: fewer spikes at low progress
        let spike_period = 1.2_f32 - ctx.eased * 0.7; // 1.2s → 0.5s at full
        let spike_period = spike_period.max(0.4);

        // How wide a single spike is in dots
        let spike_w = (w as f32 * 0.22).max(6.0);

        // Render spikes scrolling left; phase offset driven by time
        let scroll = (ctx.time / spike_period).fract(); // 0..1 scroll within period
                                                        // Draw up to 5 spike instances tiled across the bar
        for pass in 0..5i32 {
            // Position of this spike's leading edge (in dots from right, scrolling left)
            let spike_center = w as f32 - (scroll + pass as f32) * (w as f32 * 0.5);

            for xi in 0..w {
                let xf = xi as f32;
                let dist = xf - spike_center;
                if dist.abs() < spike_w {
                    let local_phase = (dist / spike_w * 0.5 + 0.5).clamp(0.0, 1.0);
                    let v = spike_shape(local_phase);
                    let dy = (v * spike_amp) as i32;
                    let sy = baseline_y as i32 - dy;
                    draw::dot_i(grid, xi as i32, sy.clamp(0, h as i32 - 1));
                }
            }
        }

        // Filled region (spikes fired) tinted with palette
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 4. Logistic Population Growth
//    N(t) = K / (1 + exp(−r·(t − t_mid)))  S-curve fills with eased.
//    A horizontal carrying-capacity ceiling is drawn at the top.
// ---------------------------------------------------------------------------
struct LogisticGrowth;
impl ProgressStyle for LogisticGrowth {
    fn name(&self) -> &str {
        "logistic-growth"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Logistic population growth: S-curve fills the bar with carrying-capacity ceiling"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Carrying-capacity line (dashed): 2 dots from top
        let k_y = 2usize;
        for x in (0..w).step_by(4) {
            draw::hline(grid, x, (x + 2).min(w - 1), k_y);
        }

        // Logistic curve: x axis = progress (0..w), y axis = population
        // r controls steepness; midpoint at x=half
        let r = 8.0_f32; // steepness
                         // eased shifts the midpoint of the visible S-curve along x
        let midpoint = (1.0 - ctx.eased) * w as f32; // at eased=1 midpoint exits left

        let mut prev_pop_y: Option<i32> = None;
        for xi in 0..w {
            let xf = xi as f32;
            let t = (xf - midpoint) * r / w as f32;
            let n = 1.0 / (1.0 + (-t).exp()); // 0..1
                                              // Map to dot rows: n=1 → k_y+1, n=0 → h−1
            let pop_y = (k_y + 1 + ((1.0 - n) * (h - k_y - 2) as f32) as usize).min(h - 1);
            let py = pop_y as i32;

            draw::dot_i(grid, xi as i32, py);
            if let Some(prev) = prev_pop_y {
                let (lo, hi) = (prev.min(py), prev.max(py));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_pop_y = Some(py);

            // Shade column under the curve
            for y in (pop_y + 1)..h {
                // Dotted fill — every other row only to keep it readable
                if y % 2 == xi % 2 {
                    draw::dot(grid, xi, y);
                }
            }
        }

        // Population label ticks at 25%, 50%, 75%
        for &frac in &[0.25_f32, 0.5, 0.75] {
            let ty = (k_y + 1 + ((1.0 - frac) * (h - k_y - 2) as f32) as usize).min(h - 1);
            draw::hline(grid, 0, 3.min(w - 1), ty);
        }

        // Tint: gradient across the filled portion
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. ECG / Heartbeat
//    PQRST waveform scrolls leftward driven by time.
//    Number of beats since start = floor(eased × max_beats) — shown as
//    filled-fraction of the baseline.
// ---------------------------------------------------------------------------
struct EcgHeartbeat;
impl ProgressStyle for EcgHeartbeat {
    fn name(&self) -> &str {
        "ecg-heartbeat"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Scrolling ECG / PQRST heartbeat waveform; beats counted = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let baseline_y = (h as f32 * 0.65) as usize;
        draw::hline(grid, 0, w.saturating_sub(1), baseline_y);

        // PQRST shape: parameterised over [0,1) within one beat period
        // Returns vertical offset relative to baseline (positive = up)
        let pqrst = |phase: f32| -> f32 {
            // P wave: small bump 0.05..0.15
            let p = if (0.05..0.20).contains(&phase) {
                ((phase - 0.05) / 0.075 * PI).sin() * 0.15
            } else {
                0.0
            };
            // Q: slight dip 0.25..0.30
            let q = if (0.25..0.30).contains(&phase) {
                -((phase - 0.25) / 0.05 * PI).sin() * 0.12
            } else {
                0.0
            };
            // R: tall spike 0.30..0.40
            let r = if (0.30..0.40).contains(&phase) {
                ((phase - 0.30) / 0.05 * PI).sin() * 1.0
            } else {
                0.0
            };
            // S: dip immediately after 0.40..0.46
            let s = if (0.40..0.46).contains(&phase) {
                -((phase - 0.40) / 0.03 * PI).sin() * 0.25
            } else {
                0.0
            };
            // T: broad bump 0.50..0.70
            let t = if (0.50..0.70).contains(&phase) {
                ((phase - 0.50) / 0.10 * PI).sin() * 0.30
            } else {
                0.0
            };
            p + q + r + s + t
        };

        // Heart rate: ~60 BPM at low progress, faster (up to 120 BPM) at full
        let bpm = 60.0 + ctx.eased * 60.0;
        let period = 60.0 / bpm; // seconds per beat
                                 // Time scroll position
        let scroll_phase = (ctx.time / period) % 1.0;

        let amp = (h as f32 * 0.55).max(1.0);
        let mut prev_y: Option<i32> = None;

        for xi in 0..w {
            // Each dot column maps to a phase that scrolls left
            let phase = ((xi as f32 / w as f32) + scroll_phase) % 1.0;
            let v = pqrst(phase);
            let dy = baseline_y as i32 - (v * amp) as i32;
            let dy = dy.clamp(0, h as i32 - 1);
            draw::dot_i(grid, xi as i32, dy);
            if let Some(py) = prev_y {
                let (lo, hi) = (py.min(dy), py.max(dy));
                for yy in lo..=hi {
                    draw::dot_i(grid, xi as i32, yy);
                }
            }
            prev_y = Some(dy);
        }

        // Tint: filled fraction of the bar = beats counted
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 6. Protein Folding
//    An amino-acid chain (polyline) starts extended across the bar at eased=0
//    and collapses into a compact, looping blob as eased→1. Individual
//    residues are dots; the backbone is a continuous polyline.
// ---------------------------------------------------------------------------
struct ProteinFolding;
impl ProgressStyle for ProteinFolding {
    fn name(&self) -> &str {
        "protein-folding"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Protein folding: amino-acid chain collapses from extended strand to a compact folded blob"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let n_residues = (w / 4).max(4).min(32);
        let mid_y = (h / 2) as f32;
        // Folded target: a compact spiral / blob centered in the bar
        let blob_r = (w.min(h) as f32 * 0.30).max(2.0);
        let cx = (w / 2) as f32;

        // Extended: residues evenly spaced across the bar at mid height
        // Folded: residues placed on a spiral centered at cx, mid_y
        // Interpolate with eased
        let e = ctx.eased;
        // Animate the folded state gently rotating with time
        let fold_rot = ctx.time * 0.4;

        let mut prev_x: Option<i32> = None;
        let mut prev_y: Option<i32> = None;

        for i in 0..n_residues {
            let frac = if n_residues <= 1 {
                0.5
            } else {
                i as f32 / (n_residues - 1) as f32
            };

            // Extended position: spread across bar
            let ex = frac * (w - 1) as f32;
            let ey = mid_y + (i as f32 * 1.3).sin() * (h as f32 * 0.05);

            // Folded position: tightly wound helix
            let spiral_angle = frac * 4.0 * PI + fold_rot;
            let spiral_r = blob_r * (0.3 + frac * 0.7);
            let fx = cx + spiral_r * spiral_angle.cos();
            let fy = mid_y + spiral_r * 0.55 * spiral_angle.sin();

            // Interpolate
            let px = (ex + (fx - ex) * e) as i32;
            let py = (ey + (fy - ey) * e) as i32;

            // Draw residue
            draw::dot_i(grid, px, py);

            // Connect backbone
            if let (Some(lx), Some(ly)) = (prev_x, prev_y) {
                line_i(grid, lx, ly, px, py);
            }
            prev_x = Some(px);
            prev_y = Some(py);
        }

        // Tint the blob region when folded
        let (cw, ch) = grid.dimensions();
        let blob_cells = (e * cw as f32).round() as usize;
        for cx2 in 0..blob_cells.min(cw) {
            let t = cx2 as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx2, cx2, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 7. Photosynthesis
//    Sun (arc) on the left emits rays that hit a leaf silhouette in the centre.
//    A glucose "store" rectangle on the right fills as eased increases.
// ---------------------------------------------------------------------------
struct Photosynthesis;
impl ProgressStyle for Photosynthesis {
    fn name(&self) -> &str {
        "photosynthesis"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Photosynthesis: sun rays hit a leaf; glucose store fills with eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid_y = (h / 2) as i32;

        // --- Sun (left 1/5): partial circle arc ---
        let sun_cx = (w as f32 * 0.08) as i32;
        let sun_r = (h as f32 * 0.30).max(2.0) as i32;
        let sun_steps = 24usize;
        for s in 0..sun_steps {
            let angle = s as f32 / sun_steps as f32 * PI * 1.2 - PI * 0.1; // right-facing arc
            let ax = sun_cx + (angle.cos() * sun_r as f32) as i32;
            let ay = mid_y + (angle.sin() * sun_r as f32) as i32;
            draw::dot_i(grid, ax, ay);
        }

        // --- Animated light rays ---
        let ray_count = 5usize;
        let leaf_cx = (w as f32 * 0.50) as i32;
        let ray_phase = ctx.time * 1.8;
        for ri in 0..ray_count {
            let angle_spread = PI * 0.55;
            let angle =
                -angle_spread / 2.0 + (ri as f32 / (ray_count - 1).max(1) as f32) * angle_spread;
            // Ray pulses along its length with time
            let pulse = (ray_phase + ri as f32 * 0.7).sin() * 0.5 + 0.5;
            let ray_len = ((leaf_cx - sun_cx) as f32 * (0.6 + pulse * 0.4)) as i32;
            let rx_end = sun_cx + (angle.cos() * ray_len as f32) as i32;
            let ry_end = mid_y + (angle.sin() * ray_len as f32) as i32;
            let rx_start = sun_cx + (angle.cos() * sun_r as f32) as i32;
            let ry_start = mid_y + (angle.sin() * sun_r as f32) as i32;
            line_i(grid, rx_start, ry_start, rx_end, ry_end);
        }

        // --- Leaf (centre): simple leaf silhouette (two arcs) ---
        let leaf_h = (h as f32 * 0.70).max(2.0);
        let leaf_w = (w as f32 * 0.12).max(2.0);
        let leaf_steps = 20usize;
        for s in 0..=leaf_steps {
            let t = s as f32 / leaf_steps as f32;
            let angle = t * PI;
            // Upper arc
            let lx = leaf_cx + (leaf_w * (angle - PI / 2.0).cos()) as i32;
            let ly = mid_y - (leaf_h / 2.0 * angle.sin()) as i32;
            draw::dot_i(grid, lx, ly);
            // Lower arc (mirror)
            draw::dot_i(grid, lx, mid_y + (mid_y - ly));
        }
        // Midrib
        draw::vline(
            grid,
            leaf_cx as usize,
            (mid_y - (leaf_h / 2.0) as i32).max(0) as usize,
            (mid_y + (leaf_h / 2.0) as i32).min(h as i32 - 1) as usize,
        );

        // --- Glucose store (right 1/5): fills with eased ---
        let store_x = (w as f32 * 0.80) as usize;
        let store_w = (w as f32 * 0.15).max(2.0) as usize;
        let store_h = (h as f32 * 0.70).max(2.0) as usize;
        let store_y = (h - store_h) / 2;
        draw::rect_outline(grid, store_x, store_y, store_w, store_h);
        let fill_h = (ctx.eased * store_h as f32) as usize;
        let fill_y = store_y + store_h.saturating_sub(fill_h);
        if fill_h > 0 && store_w > 2 {
            draw::fill_rect(grid, store_x + 1, fill_y, store_w.saturating_sub(2), fill_h);
        }

        // Tint
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 8. Phylogenetic Tree
//    Branches bifurcate outward from a root on the left; depth / branch count
//    driven by eased. Branches splay up and down. Time animates tip flutter.
// ---------------------------------------------------------------------------
struct PhylogeneticTree;
impl ProgressStyle for PhylogeneticTree {
    fn name(&self) -> &str {
        "phylogenetic-tree"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Phylogenetic tree: branches bifurcate outward; depth unlocks with eased progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Max depth controlled by eased: 0→1 = 1..=5 levels
        let max_depth = 1 + (ctx.eased * 4.0).floor() as usize; // 1..=5

        // Recursively draw branches using a stack (avoid actual recursion)
        // Each entry: (x, y, angle_rad, length, depth)
        let root_x = 2i32;
        let root_y = (h / 2) as i32;
        let root_len = (w as f32 * 0.24).max(4.0);

        struct Branch {
            x: i32,
            y: i32,
            angle: f32,
            len: f32,
            depth: usize,
        }
        let mut stack: Vec<Branch> = vec![Branch {
            x: root_x,
            y: root_y,
            angle: 0.0,
            len: root_len,
            depth: 0,
        }];

        // Gentle tip flutter via time
        let flutter_amp = 0.05_f32;
        let flutter = (ctx.time * 2.5).sin() * flutter_amp;

        while let Some(b) = stack.pop() {
            if b.depth > max_depth {
                continue;
            }
            // Squash the vertical component so deep branches stay on the
            // short canvas instead of shooting off the top and bottom.
            let end_x = b.x + (b.angle.cos() * b.len) as i32;
            let end_y = b.y + (b.angle.sin() * b.len * 0.35) as i32;
            line_i(grid, b.x, b.y, end_x, end_y);

            if b.depth < max_depth {
                let new_len = b.len * 0.62;
                if new_len < 1.5 {
                    continue;
                }
                let spread = PI * 0.38 + flutter * (b.depth as f32 + 1.0);
                stack.push(Branch {
                    x: end_x,
                    y: end_y,
                    angle: b.angle - spread,
                    len: new_len,
                    depth: b.depth + 1,
                });
                stack.push(Branch {
                    x: end_x,
                    y: end_y,
                    angle: b.angle + spread,
                    len: new_len,
                    depth: b.depth + 1,
                });
            }
        }

        // Root dot
        draw::dot_i(grid, root_x, root_y);

        // Tint gradient left→right
        let (cw, ch) = grid.dimensions();
        for cx in 0..cw {
            let t = cx as f32 / cw.max(1) as f32;
            if t <= ctx.eased {
                let col = ctx.palette.sample(t);
                for cy in 0..ch {
                    draw::tint_row(grid, cy, cx, cx, col);
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 9. Blood Flow
//    A vessel (two parallel lines) runs horizontally. Red blood cell dots
//    stream along it at pulsatile speed (time-modulated). Filled fraction of
//    the vessel = eased (vessel "perfused").
// ---------------------------------------------------------------------------
struct BloodFlow;
impl ProgressStyle for BloodFlow {
    fn name(&self) -> &str {
        "blood-flow"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Blood flow: red cells stream through a vessel at pulsatile speed; perfused fraction = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = h / 2;
        let vessel_r = (h as f32 * 0.25).max(1.0) as usize;

        // Vessel walls (two hlines)
        let top_wall = mid.saturating_sub(vessel_r);
        let bot_wall = (mid + vessel_r).min(h - 1);
        draw::hline(grid, 0, w.saturating_sub(1), top_wall);
        draw::hline(grid, 0, w.saturating_sub(1), bot_wall);

        // Perfused segment fills from left with eased
        let perfused_w = (ctx.eased * w as f32) as usize;

        // Pulsatile flow speed: heartbeat modulates velocity
        // Systole: 0→0.3 of cardiac cycle; diastole: rest
        let cardiac_freq = 1.1_f32; // Hz
        let cardiac_phase = (ctx.time * cardiac_freq).fract();
        let pulse_v = if cardiac_phase < 0.3 {
            // Systolic peak
            1.0 + 2.0 * (cardiac_phase / 0.15 * PI).sin().powi(2)
        } else {
            // Diastolic trickle
            0.4 + 0.1 * (cardiac_phase * 4.0 * PI).sin()
        };

        // Red blood cells: flattened ovals (2-dot wide blobs)
        let n_cells = 12usize;
        for ci in 0..n_cells {
            // Base position spread evenly; scroll driven by pulse_v and time
            let base_x = (ci as f32 / n_cells as f32) * w as f32;
            let scroll = (ctx.time * pulse_v * 8.0 + base_x) % w as f32;
            let cx = scroll as i32;
            let cy = mid as i32;

            // Only in perfused region
            if (scroll as usize) < perfused_w {
                // Biconcave disc: 3 dots horizontally, 1 center dot dimmer
                draw::dot_i(grid, cx - 1, cy);
                draw::dot_i(grid, cx, cy);
                draw::dot_i(grid, cx + 1, cy);
                // Slight vertical extent
                draw::dot_i(grid, cx, cy - 1);
                draw::dot_i(grid, cx, cy + 1);
            }
        }

        // Tint perfused section
        let (cw, ch) = grid.dimensions();
        let perfused_cells = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..perfused_cells.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 10. Enzyme Kinetics (Michaelis-Menten)
//     An active-site pocket (U-shape) on the left; substrates approach from
//     the right, snap in, and exit as products. Product count = eased.
//     Rate = Vmax·S / (Km + S); S decreases as eased rises.
// ---------------------------------------------------------------------------
struct EnzymeKinetics;
impl ProgressStyle for EnzymeKinetics {
    fn name(&self) -> &str {
        "enzyme-kinetics"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Michaelis-Menten kinetics: substrates bind the active site; products counted = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        let mid = (h / 2) as i32;
        let enzyme_cx = (w as f32 * 0.20) as i32;
        let pocket_r = (h as f32 * 0.30).max(2.0) as i32;

        // Enzyme body: U-shaped pocket (lower semicircle)
        let steps = 24usize;
        for s in 0..=steps {
            let angle = s as f32 / steps as f32 * PI; // 0..π bottom half
            let ax = enzyme_cx + (angle.cos() * pocket_r as f32) as i32;
            let ay = mid + (angle.sin() * pocket_r as f32) as i32;
            draw::dot_i(grid, ax, ay);
        }
        // Left arm
        draw::vline(
            grid,
            (enzyme_cx - pocket_r).max(0) as usize,
            (mid - pocket_r).max(0) as usize,
            mid as usize,
        );
        // Right arm
        draw::vline(
            grid,
            (enzyme_cx + pocket_r).min(w as i32 - 1) as usize,
            (mid - pocket_r).max(0) as usize,
            mid as usize,
        );

        // Michaelis-Menten rate: at eased=0 S is high, rate is high; at eased=1 S exhausted
        let s_conc = 1.0 - ctx.eased; // substrate concentration decreases
        let km = 0.3_f32;
        let v_max = 1.0_f32;
        let rate = v_max * s_conc / (km + s_conc); // 0..1

        // Substrate molecules approaching from the right, speed driven by rate
        let n_substrates = 4usize;
        for si in 0..n_substrates {
            let phase = (ctx.time * rate * 1.5 + si as f32 * 0.25) % 1.0;
            // Travel from right edge to pocket
            let sx = (w as f32 - (enzyme_cx + pocket_r + 2) as f32) * (1.0 - phase)
                + (enzyme_cx + pocket_r + 2) as f32;
            let sy = mid - 1 + (si % 2) as i32 * 2;
            if phase < 0.85 {
                // Substrate in transit: small square
                draw::dot_i(grid, sx as i32, sy);
                draw::dot_i(grid, sx as i32 + 1, sy);
                draw::dot_i(grid, sx as i32, sy + 1);
                draw::dot_i(grid, sx as i32 + 1, sy + 1);
            }
        }

        // Product molecules exiting to the right (below the axis)
        let n_products = (ctx.eased * 6.0) as usize;
        for pi in 0..n_products {
            let px = enzyme_cx + pocket_r + 2 + (pi as i32 * 5);
            let py = mid + pocket_r / 2;
            draw::dot_i(grid, px, py);
            draw::dot_i(grid, px + 1, py);
        }

        // Rate curve below the enzyme (mini graph)
        let graph_y_base = (h as f32 * 0.88) as usize;
        let graph_h = (h as f32 * 0.10).max(1.0) as usize;
        for xi in 0..w {
            let s = 1.0 - xi as f32 / w as f32; // S decreases left→right
            let v = v_max * s / (km + s);
            let bar_h = (v * graph_h as f32) as usize;
            for y in graph_y_base.saturating_sub(bar_h)..graph_y_base {
                draw::dot(grid, xi, y);
            }
        }
        // Mark Km on the graph
        let km_x = ((1.0 - km) * w as f32) as usize;
        if km_x < w {
            draw::dot(grid, km_x, graph_y_base);
        }

        // Tint product side
        let (cw, ch) = grid.dimensions();
        let filled = (ctx.eased * cw as f32).round() as usize;
        for cx in 0..filled.min(cw) {
            let t = cx as f32 / cw.max(1) as f32;
            for cy in 0..ch {
                draw::tint_row(grid, cy, cx, cx, ctx.palette.sample(t));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 11. Virus Spread
//    A tissue grid of cells (shade glyphs for cell-sized elements, dots for
//    infection front). Infection spreads outward from upper-left; infected
//    fraction = eased. Time animates a shimmer on the wavefront.
// ---------------------------------------------------------------------------
struct VirusSpread;
impl ProgressStyle for VirusSpread {
    fn name(&self) -> &str {
        "virus-spread"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Viral spread: infection sweeps a tissue grid cell by cell; infected fraction = progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (cw, ch) = grid.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(());
        }

        // Total tissue cells; infected fraction = eased
        let _total = cw * ch;

        // Infection spreads in a diagonal wave from top-left
        // Cell order: sorted by (cx + cy) ascending (Manhattan distance from origin)
        // We can approximate by iterating and checking if (cx+cy) / (cw+ch-2) <= eased

        for cy in 0..ch {
            for cx in 0..cw {
                // Normalized Manhattan distance from TL corner
                let dist = (cx + cy) as f32 / ((cw + ch).saturating_sub(2).max(1)) as f32;
                // Wavefront shimmer: cells near the boundary pulse
                let at_front = (dist - ctx.eased).abs() < 0.08;
                let shimmer = (ctx.time * 6.0 + dist * 4.0).sin() * 0.5 + 0.5;

                if dist <= ctx.eased {
                    // Infected: dense shade
                    let density = if at_front && shimmer > 0.5 { 3usize } else { 4 };
                    draw::shade(grid, cx, cy, density);
                    // Color
                    let col = ctx.palette.sample(dist);
                    draw::tint_row(grid, cy, cx, cx, col);
                } else if dist <= ctx.eased + 0.06 {
                    // Wavefront: lighter shade
                    draw::shade(grid, cx, cy, 2);
                }
                // Uninfected cells: leave blank (empty)
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 12. Ion Channels (Cell Membrane)
//    Two parallel lines represent the lipid bilayer. Ion channels (vertical
//    slits) pump ions (dots) through; the electrochemical gradient builds as
//    eased rises. Inside vs. outside ion density difference = progress.
// ---------------------------------------------------------------------------
struct IonChannels;
impl ProgressStyle for IonChannels {
    fn name(&self) -> &str {
        "ion-channels"
    }
    fn theme(&self) -> &str {
        "biology"
    }
    fn describe(&self) -> &str {
        "Cell membrane ion channels: pumps drive ions through the bilayer; gradient builds with progress"
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(grid);
        if w == 0 || h == 0 {
            return Ok(());
        }

        // Bilayer: two horizontal lines at 1/3 and 2/3 height
        let outer_y = (h as f32 * 0.35) as usize;
        let inner_y = (h as f32 * 0.65) as usize;
        draw::hline(grid, 0, w.saturating_sub(1), outer_y);
        draw::hline(grid, 0, w.saturating_sub(1), inner_y);

        // Ion channels: vertical gaps in the membrane with a gate indicator
        let n_channels = ((w / 10).max(1)).min(8);
        for ci in 0..n_channels {
            let ch_x = (ci * w / n_channels + w / (n_channels * 2).max(1)).min(w - 1);
            // Gate open fraction oscillates with time and ci phase
            let gate_phase = ctx.time * 2.0 + ci as f32 * 0.9;
            let open = (gate_phase.sin() * 0.5 + 0.5) * ctx.eased;
            // Clear membrane at channel (draw gap as empty — draw dots on either side)
            let gap_half = 2usize;
            // Left edge of channel
            if ch_x >= gap_half + 1 {
                draw::dot(grid, ch_x - gap_half - 1, outer_y);
                draw::dot(grid, ch_x - gap_half - 1, inner_y);
            }
            // Right edge
            if ch_x + gap_half + 1 < w {
                draw::dot(grid, ch_x + gap_half + 1, outer_y);
                draw::dot(grid, ch_x + gap_half + 1, inner_y);
            }
            // Gate: vertical bar at half-open position
            let gate_y_outer = outer_y as i32;
            let gate_y_inner = inner_y as i32;
            let gate_midpoint = gate_y_outer + ((gate_y_inner - gate_y_outer) as f32 * 0.5) as i32;
            let gate_len = ((gate_y_inner - gate_y_outer) as f32 * (1.0 - open) * 0.4) as i32;
            draw::dot_i(grid, ch_x as i32, gate_midpoint - gate_len / 2);
            draw::dot_i(grid, ch_x as i32, gate_midpoint + gate_len / 2);

            // Ions moving through the channel
            let ion_progress = (ctx.time * 1.5 + ci as f32 * 0.6).fract();
            let ion_y = (outer_y as f32 + ion_progress * (inner_y - outer_y) as f32) as i32;
            if open > 0.2 {
                draw::dot_i(grid, ch_x as i32, ion_y);
                draw::dot_i(grid, ch_x as i32 - 1, ion_y);
            }
        }

        // Outside ions (above outer membrane): sparse, few at start
        let outside_density = 1.0 - ctx.eased; // high outside before pumping
        let n_outside = (outside_density * w as f32 * 0.3) as usize;
        for oi in 0..n_outside {
            let ox = (oi * w / n_outside.max(1)) as i32;
            let oy_base = outer_y.saturating_sub(1) as i32;
            let oy = oy_base
                - ((ctx.time * 1.3 + oi as f32 * 0.5).sin() * (outer_y as f32 * 0.5)).abs() as i32;
            draw::dot_i(grid, ox, oy.max(0));
        }

        // Inside ions (below inner membrane): accumulate with eased
        let inside_density = ctx.eased;
        let n_inside = (inside_density * w as f32 * 0.3) as usize;
        for ii in 0..n_inside {
            let ix = (ii * w / n_inside.max(1)) as i32;
            let iy_base = (inner_y + 1).min(h - 1) as i32;
            let iy = iy_base
                + ((ctx.time * 1.1 + ii as f32 * 0.6).sin()
                    * (h.saturating_sub(inner_y) as f32 * 0.4))
                    .abs() as i32;
            draw::dot_i(grid, ix, iy.min(h as i32 - 1));
        }

        // Tint: inside region (below inner membrane) gets gradient
        let (cw, ch_cells) = grid.dimensions();
        let inner_cell_y = inner_y / 4;
        for cy in inner_cell_y.min(ch_cells.saturating_sub(1))..ch_cells {
            let t = ctx.eased;
            let col = ctx.palette.sample(t);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), col);
        }
        // Outside region gets complementary tint
        for cy in 0..outer_y / 4 {
            let col = ctx.palette.sample(1.0 - ctx.eased);
            draw::tint_row(grid, cy, 0, cw.saturating_sub(1), col);
        }

        Ok(())
    }
}
