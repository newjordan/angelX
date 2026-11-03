//! Integer geometry the picture reads: a square root and a sine table. The
//! rules' own arithmetic lives in `cockpit/live/lightgallop-rules`; nothing
//! here touches a float, so a drawing agrees byte for byte on every host.
//!
//! A *turn* is 1/256 of a circle, and a *brad* is 1/1024 of one. Screen y
//! points down, so an angle of 64 turns points at the ground.

/// Fixed point: 256 sub-pixels to a native pixel, as the stage games use.
pub const Q: i32 = 256;
/// Unit vectors are kept in 12 bits: 4096 is 1.0.
pub const ONE: i32 = 4096;

/// A quarter of a sine wave, 65 steps of 90/64 degrees, in 12 bits.
const SIN: [i16; 65] = [
    0, 101, 201, 301, 401, 501, 601, 700, 799, 897, 995, 1092, 1189, 1285, 1380, 1474, 1567, 1660,
    1751, 1842, 1931, 2019, 2106, 2191, 2276, 2359, 2440, 2520, 2598, 2675, 2751, 2824, 2896, 2967,
    3035, 3102, 3166, 3229, 3290, 3349, 3406, 3461, 3513, 3564, 3612, 3659, 3703, 3745, 3784, 3822,
    3857, 3889, 3920, 3948, 3973, 3996, 4017, 4036, 4052, 4065, 4076, 4085, 4091, 4095, 4096,
];

/// The integer square root: the largest r with r*r <= n.
pub fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// The sine of `turns` (1/256 of a circle), in 12 bits.
pub fn sin(turns: i32) -> i32 {
    let a = turns.rem_euclid(256);
    let (quarter, k) = (a / 64, a % 64);
    let up = i32::from(SIN[k as usize]);
    let down = i32::from(SIN[(64 - k) as usize]);
    match quarter {
        0 => up,
        1 => down,
        2 => -up,
        _ => -down,
    }
}

/// The cosine of `turns`, in 12 bits.
pub fn cos(turns: i32) -> i32 {
    sin(turns + 64)
}

/// The sine of `brads` (1/1024 of a circle), in 12 bits, between the table's
/// steps.
pub fn sin_b(brads: i32) -> i32 {
    let a = brads.rem_euclid(1024);
    let (t, f) = (a / 4, a % 4);
    sin(t) + (sin(t + 1) - sin(t)) * f / 4
}

/// The cosine of `brads`, in 12 bits.
pub fn cos_b(brads: i32) -> i32 {
    sin_b(brads + 256)
}
