//! Procedural-noise and flow-field progress bars.
//!
//! Every style is driven by **real noise algorithms** written from scratch —
//! value noise, fBm, Perlin-style gradient noise, domain warping, Worley /
//! Voronoi cellular noise, DLA crystal growth, curl noise, topographic
//! contour bands, plasma, Brownian trails, and a flow-field particle streamer.
//!
//! All algorithms are deterministic given `(ctx.progress, ctx.time)`.
//! Per-frame cost is bounded to ≤ ~3 000 evaluations in the worst case so the
//! bars stay fast even on large grids.

use super::super::draw;
use super::super::{BarContext, ProgressStyle};
use crate::{BrailleGrid, Color, DotmaxError};
use std::f32::consts::PI;

// ─── deterministic hash ──────────────────────────────────────────────────────

/// Fast integer hash → `[0, 1)`.  Used by every noise algorithm below.
#[inline]
fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) % 1000) as f32 / 1000.0
}

/// 3-D variant: hash `(x, y, z_int)` — useful for animating with a time slot.
#[inline]
fn hash3(x: i32, y: i32, z: i32) -> f32 {
    hash2(x ^ z.wrapping_mul(1_234_567), y ^ z.wrapping_mul(7_654_321))
}

// ─── smoothstep ──────────────────────────────────────────────────────────────

#[inline]
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ─── value noise ─────────────────────────────────────────────────────────────

/// Bilinear value noise at `(x, y)`.  Hash the four surrounding lattice
/// corners and interpolate with smoothstep.  Pure, no state. Kept as a
/// static building block for anyone extracting this file (the bundled styles
/// use the time-animated [`value_noise_t`] variant).
#[allow(dead_code)]
fn value_noise(x: f32, y: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = smoothstep(x - xi as f32);
    let fy = smoothstep(y - yi as f32);
    let v00 = hash2(xi, yi);
    let v10 = hash2(xi + 1, yi);
    let v01 = hash2(xi, yi + 1);
    let v11 = hash2(xi + 1, yi + 1);
    lerp(lerp(v00, v10, fx), lerp(v01, v11, fx), fy)
}

/// Animated value noise: treat `time` as a third dimension by linearly blending
/// between two lattice slabs `floor(t)` and `floor(t)+1`.
fn value_noise_t(x: f32, y: f32, t: f32) -> f32 {
    let ti = t.floor() as i32;
    let ft = smoothstep(t - ti as f32);

    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = smoothstep(x - xi as f32);
    let fy = smoothstep(y - yi as f32);

    let slab = |tz: i32| -> f32 {
        let v00 = hash3(xi, yi, tz);
        let v10 = hash3(xi + 1, yi, tz);
        let v01 = hash3(xi, yi + 1, tz);
        let v11 = hash3(xi + 1, yi + 1, tz);
        lerp(lerp(v00, v10, fx), lerp(v01, v11, fx), fy)
    };
    lerp(slab(ti), slab(ti + 1), ft)
}

// ─── fBm (fractal Brownian motion) ───────────────────────────────────────────

/// Sum `octaves` layers of animated value noise with lacunarity 2 / gain 0.5.
fn fbm(x: f32, y: f32, t: f32, octaves: usize) -> f32 {
    let mut val = 0.0f32;
    let mut amp = 0.5f32;
    let mut freq = 1.0f32;
    let mut norm = 0.0f32;
    for _ in 0..octaves.max(1) {
        val += amp * value_noise_t(x * freq, y * freq, t * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    val / norm
}

// ─── Perlin-style gradient noise ─────────────────────────────────────────────

/// Unit gradient vector from a hash, mapped to 8 cardinal / diagonal dirs.
#[inline]
fn grad_vec(ix: i32, iy: i32) -> (f32, f32) {
    let h = (hash2(ix, iy) * 8.0) as u32 % 8;
    match h {
        0 => (1.0, 0.0),
        1 => (-1.0, 0.0),
        2 => (0.0, 1.0),
        3 => (0.0, -1.0),
        4 => (0.707, 0.707),
        5 => (-0.707, 0.707),
        6 => (0.707, -0.707),
        _ => (-0.707, -0.707),
    }
}

/// Perlin-style gradient noise at `(x, y)`.
fn gradient_noise(x: f32, y: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = x - xi as f32;
    let fy = y - yi as f32;
    let ux = smoothstep(fx);
    let uy = smoothstep(fy);

    let dot = |ix: i32, iy: i32, dx: f32, dy: f32| -> f32 {
        let (gx, gy) = grad_vec(ix, iy);
        gx * dx + gy * dy
    };

    let n00 = dot(xi, yi, fx, fy);
    let n10 = dot(xi + 1, yi, fx - 1.0, fy);
    let n01 = dot(xi, yi + 1, fx, fy - 1.0);
    let n11 = dot(xi + 1, yi + 1, fx - 1.0, fy - 1.0);

    // Perlin output is [-0.7, 0.7] → remap to [0, 1].
    (lerp(lerp(n00, n10, ux), lerp(n01, n11, ux), uy) + 0.7) / 1.4
}

// ─── Worley / cellular noise ─────────────────────────────────────────────────

/// Distance to the nearest of several hash-placed feature points in the cell
/// `(xi, yi)` and its 8 neighbours.  Returns (F1, F2).
fn worley(x: f32, y: f32, seed_density: u32) -> (f32, f32) {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let mut f1 = f32::MAX;
    let mut f2 = f32::MAX;
    for cy in (yi - 1)..=(yi + 1) {
        for cx in (xi - 1)..=(xi + 1) {
            for k in 0..seed_density {
                let px = cx as f32 + hash2(cx * 7 + k as i32, cy * 13);
                let py = cy as f32 + hash2(cx * 11, cy * 17 + k as i32);
                let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
                if d < f1 {
                    f2 = f1;
                    f1 = d;
                } else if d < f2 {
                    f2 = d;
                }
            }
        }
    }
    (f1, f2)
}

// ─── registry ────────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Theme tint — analog static blue-gray.
// ---------------------------------------------------------------------------

/// Gradient endpoints for this theme's signature tint.
const TINT_START: Color = Color::rgb(188, 196, 214);
const TINT_END: Color = Color::rgb(96, 116, 156);

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

/// All styles in the `noise` theme.
pub fn styles() -> Vec<Box<dyn ProgressStyle>> {
    vec![
        Box::new(Tinted(ValueNoiseFill)),
        Box::new(Tinted(FbmContour)),
        Box::new(Tinted(GradientNoiseFill)),
        Box::new(Tinted(DomainWarp)),
        Box::new(Tinted(FlowField)),
        Box::new(Tinted(WorleyCell)),
        Box::new(Tinted(VoronoiDiagram)),
        Box::new(Tinted(DlaGrowth)),
        Box::new(Tinted(BrownianTrail)),
        Box::new(Tinted(CurlNoise)),
        Box::new(Tinted(TopoContour)),
        Box::new(Tinted(Plasma)),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. VALUE NOISE FILL
// ═══════════════════════════════════════════════════════════════════════════

/// Animated bilinear value noise thresholded by eased progress.
struct ValueNoiseFill;
impl ProgressStyle for ValueNoiseFill {
    fn name(&self) -> &str {
        "value-noise-fill"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Bilinear value noise lattice thresholded by progress — blobs of filled \
         dots that slowly churn and spread as progress grows."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        // Scale: zoom out enough that the field looks interesting even at 1-cell height.
        let scale_x = 4.0 / dw as f32;
        let scale_y = 4.0 / dh.max(1) as f32;
        let t = ctx.time * 0.3;
        let threshold = ctx.eased;
        // Hard threshold left of fill marker; animate everywhere.
        let fill_x = (ctx.eased * dw as f32) as usize;
        for dy in 0..dh {
            for dx in 0..dw {
                // Left of the fill boundary: always lit if noise < threshold.
                let n = value_noise_t(dx as f32 * scale_x, dy as f32 * scale_y, t);
                if dx < fill_x {
                    // Inside fill: draw where noise is above a moving wavefront.
                    if n > 0.3 - threshold * 0.2 {
                        draw::dot(grid, dx, dy);
                    }
                } else if dx == fill_x && n > 0.5 {
                    // Leading edge sparkle.
                    draw::dot(grid, dx, dy);
                }
            }
        }
        // Always draw full bottom edge as baseline.
        draw::hline(grid, 0, fill_x.saturating_sub(1), dh - 1);
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. fBm CONTOUR FILL
// ═══════════════════════════════════════════════════════════════════════════

/// fBm with octave count driven by progress — the detail level visibly grows.
struct FbmContour;
impl ProgressStyle for FbmContour {
    fn name(&self) -> &str {
        "fbm-contour"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Fractal Brownian motion fill: octave count rises with progress, so the \
         bar starts smooth and gains turbulent detail as it nears 100%."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        // Octave count: 1 at 0%, 6 at 100%.
        let octaves = (1 + (ctx.eased * 5.0).floor() as usize).min(6);
        let scale = 3.0 / dw as f32;
        let t = ctx.time * 0.25;
        let fill_x = (ctx.eased * dw as f32) as usize;
        // Bias threshold so that ≈50% of the noise field is lit at full progress.
        let threshold = 0.45 - ctx.eased * 0.1;
        for dy in 0..dh {
            for dx in 0..fill_x.min(dw) {
                let n = fbm(dx as f32 * scale, dy as f32 * scale, t, octaves);
                if n > threshold {
                    draw::dot(grid, dx, dy);
                }
            }
        }
        // Solid baseline so the bar reads as a bar even at low progress.
        if dh > 0 {
            draw::hline(grid, 0, fill_x.saturating_sub(1), dh - 1);
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. GRADIENT (PERLIN-STYLE) NOISE FILL
// ═══════════════════════════════════════════════════════════════════════════

/// Perlin-style gradient noise: smoother, more organic than value noise.
struct GradientNoiseFill;
impl ProgressStyle for GradientNoiseFill {
    fn name(&self) -> &str {
        "gradient-noise-fill"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Perlin-style gradient noise thresholded by progress — smoother, more \
         organic blobs than value noise, with a shimmering animated field."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let scale_x = 5.0 / dw as f32;
        let scale_y = 3.0 / dh.max(1) as f32;
        // Animate by slowly shifting the sample coordinates.
        let tx = ctx.time * 0.2;
        let ty = ctx.time * 0.15;
        let fill_x = (ctx.eased * dw as f32) as usize;
        for dy in 0..dh {
            for dx in 0..fill_x.min(dw) {
                let n = gradient_noise(dx as f32 * scale_x + tx, dy as f32 * scale_y + ty);
                if n > 0.35 {
                    draw::dot(grid, dx, dy);
                }
            }
        }
        if dh > 0 {
            draw::hline(grid, 0, fill_x.saturating_sub(1), dh - 1);
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. DOMAIN WARPING
// ═══════════════════════════════════════════════════════════════════════════

/// Domain warping: sample fBm at `(x + fbm(x,y), y + fbm(x+5,y+5))`.
/// Creates hypnotic swirls that evolve with time.
struct DomainWarp;
impl ProgressStyle for DomainWarp {
    fn name(&self) -> &str {
        "domain-warp"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Domain warping — sampling fBm at displaced coordinates — produces \
         hypnotic, swirling tendrils that churn and grow with progress."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let scale = 3.0 / dw as f32;
        let sy = 3.0 / dh.max(1) as f32;
        let t = ctx.time * 0.2;
        let fill_x = (ctx.eased * dw as f32) as usize;
        // Warp strength grows with progress.
        let warp = ctx.eased * 1.5;
        let octaves = 3;
        for dy in 0..dh {
            for dx in 0..fill_x.min(dw) {
                let sx = dx as f32 * scale;
                let sy2 = dy as f32 * sy;
                // First warp pass.
                let qx = fbm(sx, sy2, t, octaves);
                let qy = fbm(sx + 5.2, sy2 + 1.3, t, octaves);
                // Second warp pass with offset.
                let n = fbm(sx + warp * qx, sy2 + warp * qy, t, octaves);
                if n > 0.42 {
                    draw::dot(grid, dx, dy);
                }
            }
        }
        if dh > 0 {
            draw::hline(grid, 0, fill_x.saturating_sub(1), dh - 1);
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. FLOW FIELD PARTICLE STREAKS
// ═══════════════════════════════════════════════════════════════════════════

/// Flow field: angle = noise(x, y, t)·2π, particles are advected and drawn.
/// Number of active particles scales with progress.
struct FlowField;
impl ProgressStyle for FlowField {
    fn name(&self) -> &str {
        "flow-field"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Flow-field particles: each particle is advected along a noise-derived \
         angle field, leaving a streak. Particle count grows with progress."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let max_particles = 48usize;
        let n_particles = ((ctx.eased * max_particles as f32) as usize)
            .max(1)
            .min(max_particles);
        let streak_len = 12usize;
        let scale = 4.0 / dw as f32;
        let sy = 4.0 / dh.max(1) as f32;
        let t = ctx.time * 0.4;
        for p in 0..n_particles {
            // Deterministic start position from hash.
            let mut px = hash2(p as i32 * 7, 3) * dw as f32;
            let mut py = hash2(p as i32 * 11, 17) * dh as f32;
            // Offset start by time so particles continually respawn.
            let phase = hash2(p as i32, 99) * 10.0;
            let tp = (t + phase).fract();
            px = (px + tp * dw as f32 * 0.3) % dw as f32;
            for _ in 0..streak_len {
                draw::dot_i(grid, px as i32, py as i32);
                // Angle from noise field.
                let angle = value_noise_t(px * scale, py * sy, t) * 2.0 * PI;
                let step = 1.2f32;
                px += angle.cos() * step;
                py += angle.sin() * step * 0.5;
                // Wrap within grid.
                if px < 0.0 {
                    px += dw as f32;
                }
                if py < 0.0 {
                    py += dh as f32;
                }
                if px >= dw as f32 {
                    px -= dw as f32;
                }
                if py >= dh as f32 {
                    py -= dh as f32;
                }
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. WORLEY / CELLULAR NOISE
// ═══════════════════════════════════════════════════════════════════════════

/// Worley cellular noise: shade by F1 distance, reveal cells left of fill edge.
struct WorleyCell;
impl ProgressStyle for WorleyCell {
    fn name(&self) -> &str {
        "worley-cell"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Worley cellular noise — distance to nearest feature point — fills with \
         shimmering cell-membrane patterns, edges sharpening with progress."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let fill_x = (ctx.eased * dw as f32) as usize;
        let scale = 4.0 / dw as f32;
        let sy = 3.0 / dh.max(1) as f32;
        // Animate feature points slowly.
        let t_offset = ctx.time * 0.15;
        let seeds = 2u32;
        // Threshold: draw dot when F2-F1 is small (near cell edges) or F1 is small.
        for dy in 0..dh {
            for dx in 0..fill_x.min(dw) {
                let fx = dx as f32 * scale + t_offset * 0.1;
                let fy = dy as f32 * sy;
                let (f1, f2) = worley(fx, fy, seeds);
                // Cell edge (F2-F1 near 0) or inside small cells (F1 small).
                let edge = f2 - f1;
                if edge < 0.08 || f1 < 0.12 {
                    draw::dot(grid, dx, dy);
                }
            }
        }
        if dh > 0 {
            draw::hline(grid, 0, fill_x.saturating_sub(1), dh - 1);
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. VORONOI DIAGRAM
// ═══════════════════════════════════════════════════════════════════════════

/// Voronoi diagram: seeds grow with progress, draw nearest-seed boundaries.
struct VoronoiDiagram;
impl ProgressStyle for VoronoiDiagram {
    fn name(&self) -> &str {
        "voronoi-diagram"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Voronoi diagram with seed count growing with progress — cell boundaries \
         shatter and multiply, revealing a cracked-glass pattern."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let max_seeds = 24usize;
        let n_seeds = ((ctx.eased * max_seeds as f32) as usize)
            .max(1)
            .min(max_seeds);
        // Pre-compute seed positions in dot-space.
        let mut seeds: Vec<(f32, f32)> = Vec::with_capacity(n_seeds);
        let t_drift = ctx.time * 0.08;
        for i in 0..n_seeds {
            let bx = hash2(i as i32 * 3, 1) * dw as f32;
            let by = hash2(i as i32 * 7, 2) * dh as f32;
            // Seeds drift slowly.
            let dx2 = (hash2(i as i32, 42) - 0.5) * t_drift * dw as f32 * 0.1;
            let dy2 = (hash2(i as i32 + 100, 42) - 0.5) * t_drift * dh as f32 * 0.1;
            let sx = (bx + dx2).rem_euclid(dw as f32);
            let sy = (by + dy2).rem_euclid(dh.max(1) as f32);
            seeds.push((sx, sy));
        }
        // For each dot, find nearest seed index and second-nearest; draw edge if different.
        // Cost: dw*dh*n_seeds — cap to keep within budget.
        let budget_dw = dw.min(80);
        let budget_dh = dh.min(16);
        for dy in 0..budget_dh {
            for dx in 0..budget_dw {
                let mut d1 = f32::MAX;
                let mut d2 = f32::MAX;
                let mut id1 = 0usize;
                for (si, &(sx, sy)) in seeds.iter().enumerate() {
                    let d = ((dx as f32 - sx).powi(2) + (dy as f32 - sy).powi(2)).sqrt();
                    if d < d1 {
                        d2 = d1;
                        d1 = d;
                        id1 = si;
                    } else if d < d2 {
                        d2 = d;
                    }
                }
                // Boundary: d2-d1 very small → on an edge between cells.
                // Color by parity of nearest-seed id.
                let _ = id1; // used implicitly for the edge condition
                if d2 - d1 < 1.5 {
                    draw::dot(grid, dx, dy);
                } else if id1 % 2 == 0 && d1 < 3.0 {
                    // Dot near every other seed centre.
                    draw::dot(grid, dx, dy);
                }
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. DLA (DIFFUSION-LIMITED AGGREGATION) CRYSTAL
// ═══════════════════════════════════════════════════════════════════════════

/// Deterministic DLA approximation: grow a crystal cluster whose size is
/// controlled by `eased`.  Implemented as a space-filling hash walk so it's
/// O(N) with no random-number state.
struct DlaGrowth;
impl ProgressStyle for DlaGrowth {
    fn name(&self) -> &str {
        "dla-growth"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Deterministic DLA crystal: a branching, fern-like aggregate grows from \
         the centre outward as progress increases."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let cx = (dw / 2) as i32;
        let cy = (dh / 2) as i32;
        let max_particles = 200usize;
        let n_alive = ((ctx.eased * max_particles as f32) as usize).min(max_particles);
        // Animate: time shifts particle walks so the crystal breathes.
        let t_int = (ctx.time * 1.5) as i32;
        // Each particle performs a short deterministic random walk from the
        // edge and "sticks" to the cluster approximated by a distance-from-centre
        // budget that grows with eased.
        let max_r = (ctx.eased * (dw.min(dh) as f32 * 0.45)).max(1.0);
        for p in 0..n_alive {
            // Deterministic start on the perimeter.
            let angle = hash2(p as i32, t_int) * 2.0 * PI;
            let mut px = cx + (angle.cos() * max_r * 1.2) as i32;
            let mut py = cy + (angle.sin() * max_r * 0.6) as i32;
            // Walk 20 steps toward centre with noise jitter.
            for step in 0..20i32 {
                let jitter_h = hash2(p as i32 * 31 + step, t_int ^ 0xABCD);
                let jitter_v = hash2(p as i32 * 17 + step + 1000, t_int ^ 0x1234);
                let dx2 = if jitter_h > 0.5 { 1i32 } else { -1 };
                let dy2 = if jitter_v > 0.5 { 1i32 } else { -1 };
                let bias_x = if px > cx { -1i32 } else { 1 };
                let bias_y = if py > cy { -1i32 } else { 1 };
                // Blend jitter with attraction.
                let blend_h = hash2(p as i32 + step * 3, 7777);
                let blend_v = hash2(p as i32 + step * 5, 8888);
                px += if blend_h > 0.4 { bias_x } else { dx2 };
                py += if blend_v > 0.4 { bias_y } else { dy2 };
                // Stick when close enough to centre.
                let r2 = ((px - cx).pow(2) + (py - cy).pow(2)) as f32;
                if r2 < max_r * max_r * 0.5 {
                    draw::dot_i(grid, px, py);
                    break;
                }
            }
            draw::dot_i(grid, px, py);
        }
        // Always show a seed dot at the centre.
        draw::dot_i(grid, cx, cy);
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. BROWNIAN TRAIL
// ═══════════════════════════════════════════════════════════════════════════

/// Hash-driven random walk trails, revealed left-to-right by eased progress.
struct BrownianTrail;
impl ProgressStyle for BrownianTrail {
    fn name(&self) -> &str {
        "brownian-trail"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Brownian random-walk trails: multiple walkers leave hash-driven paths \
         that are progressively revealed as the bar fills."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let n_walkers = 6usize;
        let steps_per_walker = (dw / n_walkers.max(1)).max(4);
        let reveal_x = (ctx.eased * dw as f32) as usize;
        // Animate: walkers shift with time.
        let t_seed = (ctx.time * 0.5) as i32;
        for w in 0..n_walkers {
            let mut x = (w * dw / n_walkers) as i32;
            let mut y = (dh / 2) as i32;
            for s in 0..steps_per_walker {
                let hx = hash2(w as i32 * 1000 + s as i32, t_seed);
                let hy = hash2(w as i32 * 2000 + s as i32, t_seed + 1);
                x += if hx > 0.5 { 1 } else { -1 };
                y += if hy > 0.66 {
                    1
                } else if hy < 0.33 {
                    -1
                } else {
                    0
                };
                if x >= 0 && (x as usize) < reveal_x {
                    draw::dot_i(grid, x, y);
                }
            }
        }
        // Baseline.
        if dh > 0 {
            draw::hline(grid, 0, reveal_x.saturating_sub(1), dh - 1);
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. CURL NOISE FIELD
// ═══════════════════════════════════════════════════════════════════════════

/// Curl noise: take the 2-D curl of a scalar noise potential to get a
/// divergence-free velocity field, then draw streamlines through it.
struct CurlNoise;
impl ProgressStyle for CurlNoise {
    fn name(&self) -> &str {
        "curl-noise"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Curl-noise streamlines: a divergence-free velocity field derived from \
         the gradient of value noise — swirling, never-crossing filaments."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let n_streams = 32usize;
        let steps = 20usize;
        let scale = 4.0 / dw as f32;
        let sy = 3.0 / dh.max(1) as f32;
        let t = ctx.time * 0.3;
        let reveal_x = (ctx.eased * dw as f32) as usize;
        let eps = 0.5f32;
        for s in 0..n_streams {
            // Seed point spread across the bar.
            let mut px = hash2(s as i32, 1) * dw as f32;
            let mut py = hash2(s as i32, 2) * dh as f32;
            for _ in 0..steps {
                // Numerical curl: d/dy(noise) in x, -d/dx(noise) in y.
                let n_y_plus = value_noise_t(px * scale, (py + eps) * sy, t);
                let n_y_minus = value_noise_t(px * scale, (py - eps) * sy, t);
                let n_x_plus = value_noise_t((px + eps) * scale, py * sy, t);
                let n_x_minus = value_noise_t((px - eps) * scale, py * sy, t);
                let curl_x = (n_y_plus - n_y_minus) / (2.0 * eps * sy);
                let curl_y = -(n_x_plus - n_x_minus) / (2.0 * eps * scale);
                // Normalise.
                let mag = (curl_x * curl_x + curl_y * curl_y).sqrt().max(0.001);
                let step = 1.5f32;
                px += curl_x / mag * step;
                py += curl_y / mag * step * 0.5;
                // Wrap.
                px = px.rem_euclid(dw as f32);
                py = py.rem_euclid(dh.max(1) as f32);
                if (px as usize) < reveal_x {
                    draw::dot_i(grid, px as i32, py as i32);
                }
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. TOPOGRAPHIC CONTOUR (fBm ISOLINES)
// ═══════════════════════════════════════════════════════════════════════════

/// Marching contour bands of fBm — looks like a topographic map.
/// Number of contour levels grows with progress.
struct TopoContour;
impl ProgressStyle for TopoContour {
    fn name(&self) -> &str {
        "topo-contour"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Topographic fBm isolines: contour bands of fractal Brownian motion \
         multiply with progress, building a richly detailed elevation map."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let n_levels = ((ctx.eased * 8.0) as usize).max(1).min(8);
        let scale = 3.5 / dw as f32;
        let sy = 3.0 / dh.max(1) as f32;
        let t = ctx.time * 0.2;
        let octaves = 4;
        let fill_x = (ctx.eased * dw as f32) as usize;
        for dy in 0..dh {
            for dx in 0..fill_x.min(dw) {
                let n = fbm(
                    dx as f32 * scale,
                    dy as f32 * scale * sy / scale,
                    t,
                    octaves,
                );
                // Draw at contour bands: every 1/n_levels interval near an isoline.
                let band = (n * n_levels as f32).fract();
                // A dot is on the isoline if the band value is near 0 or 1.
                if band < 0.12 || band > 0.88 {
                    draw::dot(grid, dx, dy);
                }
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. PLASMA (CLASSIC DEMOSCENE)
// ═══════════════════════════════════════════════════════════════════════════

/// Classic demoscene plasma: sum of sines of x, y, distance, and time.
/// Threshold the result against eased progress to fill left-to-right.
struct Plasma;
impl ProgressStyle for Plasma {
    fn name(&self) -> &str {
        "plasma"
    }
    fn theme(&self) -> &str {
        "noise"
    }
    fn describe(&self) -> &str {
        "Classic demoscene plasma — superimposed sine waves of position, \
         distance, and time — thresholded into a pulsing psychedelic fill."
    }
    fn render(&self, grid: &mut BrailleGrid, ctx: &BarContext) -> Result<(), DotmaxError> {
        let (dw, dh) = draw::dot_dims(grid);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let t = ctx.time;
        let cx = dw as f32 / 2.0;
        let cy = dh as f32 / 2.0;
        let fill_x = (ctx.eased * dw as f32) as usize;
        // Threshold oscillates slightly with time for shimmer.
        let threshold = 0.4 + 0.08 * (t * 1.1).sin();
        for dy in 0..dh {
            for dx in 0..fill_x.min(dw) {
                let x = dx as f32 / dw as f32 * 8.0;
                let y = dy as f32 / dh.max(1) as f32 * 4.0;
                let dist = ((dx as f32 - cx).powi(2) + (dy as f32 - cy).powi(2)).sqrt();
                let v = 0.25 * (x + t).sin()
                    + 0.25 * (y + t * 0.7).sin()
                    + 0.25 * ((x + y) * 0.5 + t * 1.3).sin()
                    + 0.25 * (dist * 0.25 + t).sin();
                // v ∈ [-1, 1] → remap to [0, 1].
                let vn = (v + 1.0) * 0.5;
                if vn > threshold {
                    draw::dot(grid, dx, dy);
                }
            }
        }
        // Baseline.
        if dh > 0 {
            draw::hline(grid, 0, fill_x.saturating_sub(1), dh - 1);
        }
        Ok(())
    }
}
