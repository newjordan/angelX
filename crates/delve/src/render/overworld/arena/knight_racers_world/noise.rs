//! Integer-hash value noise: the same land on every machine, no tables.

pub fn hash(ix: i32, iy: i32, seed: u32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(0x9E37_79B1) ^ (iy as u32).wrapping_mul(0x85EB_CA6B) ^ seed.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFF_FFFF) as f32 / 0x100_0000 as f32
}

fn smooth(t: f32) -> f32 { t * t * (3. - 2. * t) }

/// Smooth value noise in `0..1`.
pub fn value(x: f32, y: f32, seed: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let (tx, ty) = (smooth(x - fx), smooth(y - fy));
    let a = hash(ix, iy, seed);
    let b = hash(ix + 1, iy, seed);
    let c = hash(ix, iy + 1, seed);
    let d = hash(ix + 1, iy + 1, seed);
    (a + (b - a) * tx) + ((c + (d - c) * tx) - (a + (b - a) * tx)) * ty
}

/// Fractal noise in `0..1`, `octaves` deep.
pub fn fbm(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0., 0.5, 0., 1.);
    for o in 0..octaves {
        sum += amp * value(x * f, y * f, seed.wrapping_add(o * 101));
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}
