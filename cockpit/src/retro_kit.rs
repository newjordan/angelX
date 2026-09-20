//! The cockpit's shared retro art direction — ordered dither, banded warm
//! palettes, and procedural firelight for every dotmax braille surface.
//!
//! The braille matrix is treated the way OG-era tile hardware was: one ink
//! per terminal cell (the "attribute"), 2×4 dots per cell for texture. All
//! shading therefore happens in exactly two registers, and this module owns
//! both:
//!
//! - **dot density** — Bayer 8×8 ordered dither, the signature texture of the
//!   era. A luminance field in `[0,1]` becomes dots via [`dither`], so
//!   gradients read as woven patterns instead of hard bands.
//! - **banded color ramps** — limited palettes sampled through [`Ramp`],
//!   deliberately quantized so color steps land like 16-color scene art
//!   rather than smooth 24-bit gradients.
//!
//! On top sit the ambient ingredients every surface shares: value noise and
//! fbm for organic variation, a stateless flame field for firelight, and
//! radial glow/flicker helpers for lanterns and forges. Everything here is a
//! pure function of its inputs — same frame in, same art out — because the
//! surfaces that consume it are cached, fixture-pinned, or both.

#![cfg_attr(not(test), allow(dead_code))]

use dotmax::Color as DotColor;

/// Classic 8×8 Bayer threshold matrix (values 0–63), the same one dotmax's
/// image pipeline uses. Kept here at dot granularity so procedural fields can
/// dither without allocating a `GrayImage` per frame.
pub(crate) const BAYER_8X8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Ordered-dither gate: is the dot at `(x, y)` lit for luminance `v` ∈ [0,1]?
///
/// `v = 0.0` never lights, `v = 1.0` always lights, and everything between
/// falls into the Bayer weave. Screen-space coordinates keep the pattern
/// stable while content moves beneath it — the period-correct feel.
#[inline]
pub(crate) fn dither(x: usize, y: usize, v: f32) -> bool {
    let threshold = (f32::from(BAYER_8X8[y & 7][x & 7]) + 0.5) / 64.0;
    v > threshold
}

// ============================================================================
// Banded palettes
// ============================================================================

/// A palette ramp: dark → bright stops, linearly blended, optionally banded.
///
/// Ramps are the "attribute" half of the art direction: a surface computes a
/// single brightness per cell and lets the ramp pick the ink, so every
/// surface drawing from the same ramp reads as one lighting scheme.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ramp(pub(crate) &'static [(u8, u8, u8)]);

impl Ramp {
    /// Smoothly sample the ramp at `t` ∈ [0,1].
    pub(crate) fn sample(&self, t: f32) -> DotColor {
        let stops = self.0;
        debug_assert!(stops.len() >= 2, "a ramp needs at least two stops");
        let t = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
        let i = (t as usize).min(stops.len() - 2);
        let f = t - i as f32;
        let (r0, g0, b0) = stops[i];
        let (r1, g1, b1) = stops[i + 1];
        let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * f) as u8;
        DotColor::rgb(mix(r0, r1), mix(g0, g1), mix(b0, b1))
    }

    /// Sample with `bands` deliberate steps — the 16-color-era look where a
    /// sky is four blues, not four hundred.
    pub(crate) fn banded(&self, t: f32, bands: u8) -> DotColor {
        let bands = bands.max(2);
        let q = (t.clamp(0.0, 1.0) * f32::from(bands)).floor() / f32::from(bands - 1);
        self.sample(q.min(1.0))
    }
}

/// Firelight: near-black coal → deep red → orange → amber → candle-white.
pub(crate) const EMBER: Ramp = Ramp(&[
    (12, 4, 4),
    (66, 10, 14),
    (140, 34, 18),
    (219, 86, 24),
    (255, 158, 44),
    (255, 224, 140),
    (255, 250, 220),
]);

/// Torch/lantern spill: umber shadows warming to gold. Gentler than EMBER —
/// this is light *landing on* things, not the fire itself.
pub(crate) const TORCH: Ramp = Ramp(&[
    (26, 16, 12),
    (74, 44, 24),
    (146, 92, 40),
    (222, 160, 72),
    (255, 214, 138),
]);

/// Night air: indigo depths up to pale moonlight.
pub(crate) const MOONLIT: Ramp = Ramp(&[
    (6, 8, 20),
    (24, 32, 64),
    (56, 72, 120),
    (120, 140, 190),
    (212, 222, 242),
]);

/// A dusk sky, horizon-warm: indigo → violet → rose → amber afterglow.
pub(crate) const DUSK: Ramp = Ramp(&[
    (18, 10, 38),
    (58, 24, 72),
    (128, 42, 84),
    (206, 90, 78),
    (247, 166, 90),
    (255, 216, 150),
]);

/// Blend two inks: `t = 0` is all `a`, `t = 1` all `b`. The lantern-light
/// primitive — warm a terrain cell toward TORCH without leaving its biome.
#[inline]
#[allow(dead_code)]
pub(crate) fn mix(a: DotColor, b: DotColor, t: f32) -> DotColor {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t) as u8;
    DotColor::rgb(ch(a.r, b.r), ch(a.g, b.g), ch(a.b, b.b))
}

// ============================================================================
// Noise — deterministic, allocation-free
// ============================================================================

/// Stateless integer hash → [0,1). Same mixing family as dotmax's jittered
/// dither, so ambient noise across surfaces shares a texture.
#[inline]
pub(crate) fn hash01(x: i32, y: i32, seed: u64) -> f32 {
    let mut h = seed
        .wrapping_add((x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    h ^= h >> 30;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 27;
    h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 31;
    ((h >> 40) as f32) / ((1u64 << 24) as f32)
}

/// Smooth value noise at `(x, y)` — bilinear blend of lattice hashes with a
/// smoothstep fade. Range [0,1].
pub(crate) fn value_noise(x: f32, y: f32, seed: u64) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let (fx, fy) = (x - xi, y - yi);
    let fade = |t: f32| t * t * (3.0 - 2.0 * t);
    let (ux, uy) = (fade(fx), fade(fy));
    let (xi, yi) = (xi as i32, yi as i32);
    let n00 = hash01(xi, yi, seed);
    let n10 = hash01(xi + 1, yi, seed);
    let n01 = hash01(xi, yi + 1, seed);
    let n11 = hash01(xi + 1, yi + 1, seed);
    let top = n00 + (n10 - n00) * ux;
    let bot = n01 + (n11 - n01) * ux;
    top + (bot - top) * uy
}

/// Fractal brownian motion: `octaves` layers of value noise, each half the
/// amplitude and double the frequency. Normalized to [0,1].
pub(crate) fn fbm(x: f32, y: f32, octaves: u32, seed: u64) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for octave in 0..octaves.max(1) {
        sum += value_noise(x * freq, y * freq, seed.wrapping_add(u64::from(octave))) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

// ============================================================================
// Firelight
// ============================================================================

/// Heat of a stateless flame at normalized coordinates: `u` ∈ [-1,1] across
/// the flame's width, `v` ∈ [0,1] from base to tip, `t` in seconds.
///
/// A teardrop envelope (wide at the base, tapering to the tip, swaying with
/// time) is eaten into by upward-scrolling fbm turbulence — the classic
/// demoscene fire read, but reproducible per frame so cached surfaces and
/// tests stay deterministic. Returns heat in [0,1]: ≥0.85 is the white core,
/// mid-range the orange body, low values the ember fringe.
pub(crate) fn flame_heat(u: f32, v: f32, t: f32, seed: u64) -> f32 {
    let v = v.clamp(0.0, 1.0);
    // The tongue sways as it rises; higher = more sway.
    let sway = (t * 2.3 + v * 3.4).sin() * 0.28 * v;
    let u = u - sway;
    // Width tapers from base to tip; a soft shoulder keeps the base round.
    let width = (1.0 - v).mul_add(0.85, 0.15);
    let core = 1.0 - (u / width).powi(2);
    if core <= 0.0 {
        return 0.0;
    }
    // Turbulence scrolls downward through sample space => flame licks upward.
    let lick = fbm(u * 2.4, v * 3.2 - t * 2.1, 3, seed);
    let heat = core * (1.0 - v * 0.55) - lick * v * 0.65;
    heat.clamp(0.0, 1.0)
}

/// Radial glow falloff: 1 at the source, 0 at `radius`, eased so the pool of
/// light has a bright heart and a long soft skirt.
#[inline]
pub(crate) fn glow(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let d = (distance / radius).clamp(0.0, 1.0);
    (1.0 - d) * (1.0 - d)
}

/// Lantern flicker: a slow noise wander in [0.82, 1.12], time-keyed so every
/// light with a distinct `seed` breathes on its own rhythm.
#[inline]
pub(crate) fn flicker(t: f32, seed: u64) -> f32 {
    0.82 + value_noise(t * 3.1, 0.5, seed) * 0.30
}

/// Test-only art-review support: rasterize a rendered ratatui `Text` to PNG —
/// braille dots as pixel blocks in their span ink, glyph cells as solid
/// blocks — at terminal proportions (cell ≈ 1:2), so any braille surface can
/// be eyeballed like the game art it is. Used by the ignored `*_gallery`
/// dump tests across the viz modules.
#[cfg(test)]
pub(crate) mod gallery {
    use ratatui::style::Color;
    use ratatui::text::Text;

    pub(crate) fn out_dir() -> std::path::PathBuf {
        let dir = std::env::var("ANGEL0_GALLERY_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("angel0-gallery"));
        std::fs::create_dir_all(&dir).expect("gallery dir");
        dir
    }

    pub(crate) fn rasterize(text: &Text<'_>, path: &std::path::Path) {
        let rows = text.lines.len() as u32;
        let cols = text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.chars().count())
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0) as u32;
        // Cell = 10×20 px (terminal ~1:2); braille dot = 5×5 px block.
        let mut img = image::RgbImage::from_pixel(cols * 10, rows * 20, image::Rgb([10, 10, 14]));
        for (row, line) in text.lines.iter().enumerate() {
            let mut col = 0usize;
            for span in &line.spans {
                let ink = match span.style.fg {
                    Some(Color::Rgb(r, g, b)) => image::Rgb([r, g, b]),
                    _ => image::Rgb([198, 206, 215]),
                };
                for ch in span.content.chars() {
                    let (x0, y0) = (col as u32 * 10, row as u32 * 20);
                    if ('\u{2800}'..='\u{28FF}').contains(&ch) {
                        let bits = ch as u32 - 0x2800;
                        for (bit, dcol, drow) in [
                            (0u32, 0u32, 0u32),
                            (1, 0, 1),
                            (2, 0, 2),
                            (3, 1, 0),
                            (4, 1, 1),
                            (5, 1, 2),
                            (6, 0, 3),
                            (7, 1, 3),
                        ] {
                            if bits & (1 << bit) != 0 {
                                for py in 0..4 {
                                    for px in 0..4 {
                                        img.put_pixel(x0 + dcol * 5 + px, y0 + drow * 5 + py, ink);
                                    }
                                }
                            }
                        }
                    } else if ch != ' ' {
                        // Glyph overlays: a solid block so signs read in review.
                        for py in 2..18 {
                            for px in 1..9 {
                                img.put_pixel(x0 + px, y0 + py, ink);
                            }
                        }
                    }
                    col += 1;
                }
            }
        }
        img.save(path).expect("save png");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dither_extremes_and_coverage() {
        for y in 0..8 {
            for x in 0..8 {
                assert!(!dither(x, y, 0.0), "zero luminance must never light");
                assert!(dither(x, y, 1.0), "full luminance must always light");
            }
        }
        // Mid grey lights about half the matrix — the whole point of Bayer.
        let lit = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .filter(|&(x, y)| dither(x, y, 0.5))
            .count();
        assert!((24..=40).contains(&lit), "50% grey lit {lit}/64 dots");
    }

    #[test]
    fn ramps_run_dark_to_bright() {
        for ramp in [EMBER, TORCH, MOONLIT, DUSK] {
            let lo = ramp.sample(0.0);
            let hi = ramp.sample(1.0);
            let luma = |c: DotColor| u32::from(c.r) + u32::from(c.g) + u32::from(c.b);
            assert!(
                luma(lo) < luma(hi),
                "ramp must brighten from t=0 to t=1: {lo:?} vs {hi:?}"
            );
            // Out-of-range samples clamp instead of panicking.
            assert_eq!(ramp.sample(-1.0), lo);
            assert_eq!(ramp.sample(2.0), hi);
        }
    }

    #[test]
    fn banded_sampling_quantizes() {
        let mut distinct = std::collections::BTreeSet::new();
        for i in 0..=100 {
            let c = EMBER.banded(i as f32 / 100.0, 4);
            distinct.insert((c.r, c.g, c.b));
        }
        assert!(
            distinct.len() <= 5,
            "4 bands should yield at most 5 inks, got {}",
            distinct.len()
        );
    }

    #[test]
    fn noise_is_deterministic_and_in_range() {
        for i in 0..64 {
            let x = i as f32 * 0.37;
            let y = i as f32 * 0.61;
            let a = value_noise(x, y, 7);
            let b = value_noise(x, y, 7);
            assert_eq!(a, b, "same input must give same noise");
            assert!((0.0..=1.0).contains(&a));
            let f = fbm(x, y, 3, 7);
            assert!((0.0..=1.0).contains(&f));
        }
        // Different seeds decorrelate.
        assert_ne!(value_noise(3.3, 4.4, 1), value_noise(3.3, 4.4, 2));
    }

    #[test]
    fn flame_has_hot_base_cool_tip_and_bounds() {
        let base = flame_heat(0.0, 0.05, 1.0, 3);
        let tip = flame_heat(0.0, 0.95, 1.0, 3);
        assert!(
            base > tip,
            "flame base ({base}) must run hotter than tip ({tip})"
        );
        assert!(base > 0.5, "centre of the base should glow: {base}");
        for step in 0..200 {
            let u = (step % 20) as f32 / 10.0 - 1.0;
            let v = (step / 20) as f32 / 10.0;
            let h = flame_heat(u, v, step as f32 * 0.13, 9);
            assert!((0.0..=1.0).contains(&h));
        }
        // Outside the envelope there is no heat.
        assert_eq!(flame_heat(1.8, 0.5, 0.0, 3), 0.0);
    }

    #[test]
    fn glow_and_flicker_stay_bounded() {
        assert!(glow(0.0, 5.0) > 0.99);
        assert_eq!(glow(5.0, 5.0), 0.0);
        assert_eq!(glow(9.0, 5.0), 0.0);
        assert_eq!(glow(1.0, 0.0), 0.0);
        for i in 0..100 {
            let f = flicker(i as f32 * 0.21, 11);
            assert!((0.8..=1.15).contains(&f), "flicker out of band: {f}");
        }
    }
}
