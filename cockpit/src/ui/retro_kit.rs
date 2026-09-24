//! The cockpit's shared retro art direction — ordered dither, banded
//! palettes and value noise for every dotmax braille surface.
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
//! On top sits value noise for organic variation, shared by every surface.
//! Everything here is a pure function of its inputs — same frame in, same art
//! out — because the surfaces that consume it are cached, fixture-pinned, or
//! both. Firelight and dusk now belong to the pixel overworld's own light
//! pass (`stage/world_viz/overworld/light.rs`).

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

/// Night air: indigo depths up to pale moonlight.
pub(crate) const MOONLIT: Ramp = Ramp(&[
    (6, 8, 20),
    (24, 32, 64),
    (56, 72, 120),
    (120, 140, 190),
    (212, 222, 242),
]);

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

// ============================================================================
// Firelight
// ============================================================================

/// Test-only art-review support: rasterize a rendered ratatui `Text` to PNG —
/// braille dots as pixel blocks in their span ink, glyph cells as solid
/// blocks — at terminal proportions (cell ≈ 1:2), so any braille surface can
/// be eyeballed like the game art it is. Used by the ignored `*_gallery`
/// dump tests across the viz modules.
#[cfg(test)]
#[path = "../../../tests/cockpit/app/retro_kit__gallery.rs"]
pub(crate) mod gallery;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/retro_kit__tests.rs"]
mod tests;
