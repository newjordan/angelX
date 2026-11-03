//! Chance without rolling: every share an effect needs is hashed from
//! numbers it already holds (a mote's index, the recipe's seed, a frame),
//! so a frame renders alone, in any order, the same on every machine.

/// Murmur3's finaliser: every bit of `h` stirred into every other.
fn stir(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

/// A well-mixed hash of two numbers and a salt. Each goes in through its
/// own round, so neighbouring indices under any salt come out apart (one
/// round over the three together left some salts bunched).
pub fn hash(a: i32, b: i32, s: u32) -> u32 {
    let h = stir(s.wrapping_mul(0x9e37_79b9) ^ 0x2545_f491);
    let h = stir(h ^ (a as u32).wrapping_mul(0x27d4_eb2d));
    stir(h ^ (b as u32).wrapping_mul(0x1656_67b1))
}

/// A share in `0..1` hashed from `(a, b, s)`.
pub fn unit(a: i32, b: i32, s: u32) -> f32 {
    (hash(a, b, s) >> 16) as f32 / 65536.0
}

/// Smooth value noise in `0..=1`.
pub fn noise(x: f32, y: f32, s: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let g = |ix: i32, iy: i32| (hash(ix, iy, s) & 0xffff) as f32 / 65535.0;
    let (ix, iy) = (x0 as i32, y0 as i32);
    let top = g(ix, iy) + (g(ix + 1, iy) - g(ix, iy)) * sx;
    let bot = g(ix, iy + 1) + (g(ix + 1, iy + 1) - g(ix, iy + 1)) * sx;
    top + (bot - top) * sy
}

const BAYER: [[f32; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];

/// The 4x4 ordered-dither threshold in `(0, 1)` at a picture's pixel, so a
/// pattern is keyed on the picture and never swims as an effect moves.
pub fn bayer(x: i32, y: i32) -> f32 {
    (BAYER[(y & 3) as usize][(x & 3) as usize] + 0.5) / 16.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chance_is_the_same_every_time_and_in_range() {
        assert_eq!(hash(3, 4, 5), hash(3, 4, 5));
        assert_ne!(hash(3, 4, 5), hash(4, 3, 5));
        for i in 0..200 {
            let u = unit(i, 7, 1);
            assert!((0.0..1.0).contains(&u));
            let n = noise(i as f32 * 0.37, 1.3, 2);
            assert!((0.0..=1.0).contains(&n));
        }
        assert!(bayer(0, 0) < bayer(1, 1));
    }

    #[test]
    fn neighbours_under_any_salt_come_out_spread() {
        for seed in [0u32, 7, 91, 0x9e37_79b9, hash(6, 0, 91)] {
            for salt in 0..16 {
                let v: Vec<f32> = (0..48).map(|i| unit(i, seed as i32, salt)).collect();
                let (lo, hi) = (v.iter().cloned().fold(1.0, f32::min), v.iter().cloned().fold(0.0, f32::max));
                let mean = v.iter().sum::<f32>() / v.len() as f32;
                assert!(lo < 0.15 && hi > 0.85 && (0.35..0.65).contains(&mean), "seed {seed} salt {salt}: {v:?}");
            }
        }
    }
}
