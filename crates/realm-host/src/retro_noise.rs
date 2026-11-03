//! Deterministic, allocation-free value noise for the realm's weather and
//! water. The cockpit's `ui::retro_kit` re-exports these two beside its
//! dither and palette ramps, so every surface shares one texture.

/// Stateless integer hash → [0,1). Same mixing family as dotmax's jittered
/// dither, so ambient noise across surfaces shares a texture.
#[inline]
pub fn hash01(x: i32, y: i32, seed: u64) -> f32 {
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
#[inline]
pub fn value_noise(x: f32, y: f32, seed: u64) -> f32 {
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
