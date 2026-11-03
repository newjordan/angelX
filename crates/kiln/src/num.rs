//! Deterministic arithmetic: the few curves kiln needs (sine and cosine,
//! the exponential, the logarithm, powers) in plain `f32` additions and
//! multiplications, so a recipe draws the same pixels on every machine and
//! in the browser, whatever libm the platform has. Square roots are the
//! platform's: IEEE rounds them exactly.

use std::f32::consts::{LN_2, PI, TAU};

/// Sine and cosine of `deg` degrees.
pub fn sin_cos_deg(deg: f32) -> (f32, f32) {
    sin_cos(deg * (PI / 180.0))
}

/// Sine and cosine of `x` radians, to about 1e-7.
pub fn sin_cos(x: f32) -> (f32, f32) {
    if !x.is_finite() {
        return (0.0, 1.0);
    }
    // Into [-pi, pi], then fold into [-pi/2, pi/2] where the series is short.
    let x = x - (x / TAU).round() * TAU;
    let (r, flip) = if x > PI / 2.0 {
        (PI - x, true)
    } else if x < -PI / 2.0 {
        (-PI - x, true)
    } else {
        (x, false)
    };
    let r2 = r * r;
    let s = r * (1.0 - r2 / 6.0 * (1.0 - r2 / 20.0 * (1.0 - r2 / 42.0 * (1.0 - r2 / 72.0 * (1.0 - r2 / 110.0)))));
    let c = 1.0 - r2 / 2.0 * (1.0 - r2 / 12.0 * (1.0 - r2 / 30.0 * (1.0 - r2 / 56.0 * (1.0 - r2 / 90.0))));
    (s, if flip { -c } else { c })
}

/// `e` to the `x`, to a few ulps; held to f32's range.
pub fn exp(x: f32) -> f32 {
    if x.is_nan() {
        return x;
    }
    let x = x.clamp(-87.0, 88.0);
    let k = (x / LN_2).round();
    let r = x - k * LN_2;
    // |r| <= ln 2 / 2: eight terms.
    let mut term = 1.0f32;
    let mut sum = 1.0f32;
    for n in 1..=9 {
        term *= r / n as f32;
        sum += term;
    }
    sum * f32::from_bits(((k as i32 + 127).clamp(1, 254) as u32) << 23)
}

/// The natural logarithm of `x`; minus infinity at 0 and below.
pub fn ln(x: f32) -> f32 {
    if x <= 0.0 || x.is_nan() {
        return f32::NEG_INFINITY;
    }
    if x.is_infinite() {
        return x;
    }
    // x = m * 2^e with m in [sqrt(1/2), sqrt(2)).
    let bits = x.to_bits();
    let mut e = ((bits >> 23) & 0xff) as i32 - 127;
    let mut m = f32::from_bits((bits & 0x007f_ffff) | 0x3f80_0000);
    if e == -127 {
        // Subnormal: scale up first.
        return ln(x * 8_388_608.0) - 23.0 * LN_2;
    }
    if m > std::f32::consts::SQRT_2 {
        m /= 2.0;
        e += 1;
    }
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let series = 2.0 * s * (1.0 + s2 * (1.0 / 3.0 + s2 * (1.0 / 5.0 + s2 * (1.0 / 7.0 + s2 * (1.0 / 9.0)))));
    e as f32 * LN_2 + series
}

/// `x` to the `y`, for `x >= 0`; 0 to any positive power is 0.
pub fn pow(x: f32, y: f32) -> f32 {
    if x <= 0.0 {
        return if y == 0.0 { 1.0 } else { 0.0 };
    }
    exp(y * ln(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curves_match_the_platform_closely() {
        for i in -720..=720 {
            let d = i as f32 * 0.75;
            let (s, c) = sin_cos_deg(d);
            let r = d.to_radians();
            assert!((s - r.sin()).abs() < 2e-6, "sin {d}");
            assert!((c - r.cos()).abs() < 2e-6, "cos {d}");
        }
        for i in -200..=200 {
            let x = i as f32 * 0.1;
            assert!((exp(x) - x.exp()).abs() <= x.exp() * 2e-6, "exp {x}");
        }
        for i in 1..=400 {
            let x = i as f32 * 0.037;
            assert!((ln(x) - x.ln()).abs() < 2e-6, "ln {x}");
            assert!((pow(x, 1.2) - x.powf(1.2)).abs() <= x.powf(1.2) * 4e-6, "pow {x}");
        }
        assert_eq!(pow(0.0, 1.2), 0.0);
    }
}
