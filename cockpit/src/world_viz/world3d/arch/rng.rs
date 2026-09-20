//! Deterministic seeded hashing for the architecture generators.
//!
//! There is no ambient randomness anywhere in this subtree: every varying
//! number is a pure function of an explicit `seed: u64` pushed through the
//! splitmix64 multiply/xor-shift avalanche below. That keeps the world3d
//! determinism law (docs/plans/world3d-spec.md) — same seed, byte-identical
//! triangle stream, forever.

/// splitmix64 finalizer: the multiply / xor-shift avalanche.
pub(crate) const fn mix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Derive an independent child seed so two builders fed the same parent seed
/// do not march in lockstep.
pub(crate) const fn sub_seed(seed: u64, tag: u64) -> u64 {
    mix64(seed ^ mix64(tag.wrapping_mul(0x2545_F491_4F6C_DD1D)))
}

/// Map a hash to `[0, 1)`. Uses the top 24 bits so the f32 conversion is exact.
pub(crate) fn unit_from(h: u64) -> f32 {
    ((h >> 40) as f32) * (1.0 / 16_777_216.0)
}

/// Hash a 2D integer lattice point to `[0, 1)` — the terrain noise source.
pub(crate) fn lattice(seed: u64, ix: i32, iy: i32) -> f32 {
    let a = (ix as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let b = (iy as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    unit_from(mix64(seed ^ a ^ b.rotate_left(17)))
}

/// Smooth-interpolated value noise over the unit lattice; result in `[0, 1]`.
pub(crate) fn value_noise(seed: u64, x: f32, y: f32) -> f32 {
    let xf = x.floor();
    let yf = y.floor();
    let (ix, iy) = (xf as i32, yf as i32);
    let fx = x - xf;
    let fy = y - yf;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let a = lattice(seed, ix, iy);
    let b = lattice(seed, ix + 1, iy);
    let c = lattice(seed, ix, iy + 1);
    let d = lattice(seed, ix + 1, iy + 1);
    let lo = a + (b - a) * sx;
    let hi = c + (d - c) * sx;
    lo + (hi - lo) * sy
}

/// Two-octave version — one broad swell plus a finer wrinkle. Still `[0, 1]`.
pub(crate) fn fbm2(seed: u64, x: f32, y: f32) -> f32 {
    let a = value_noise(seed, x, y);
    let b = value_noise(seed ^ 0x5BF0_3635, x * 2.7 + 11.3, y * 2.7 - 7.1);
    0.72 * a + 0.28 * b
}

/// Tiny deterministic stream RNG. Not cryptographic, not statistically fancy —
/// just a repeatable jitter source for facade rhythm and prop scatter.
#[derive(Clone, Debug)]
pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng {
            state: mix64(seed ^ 0xA076_1D64_78BD_642F),
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix64(self.state)
    }

    /// `[0, 1)`
    pub(crate) fn unit(&mut self) -> f32 {
        unit_from(self.next_u64())
    }

    pub(crate) fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    /// Symmetric jitter in `[-amount, amount]`.
    pub(crate) fn jitter(&mut self, amount: f32) -> f32 {
        self.range(-amount, amount)
    }

    pub(crate) fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    /// Uniform integer in `[0, n)`; `0` when `n == 0`.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Uniform integer in `[lo, hi]` (inclusive both ends).
    pub(crate) fn int_in(&mut self, lo: usize, hi: usize) -> usize {
        if hi <= lo {
            lo
        } else {
            lo + self.below(hi - lo + 1)
        }
    }
}
