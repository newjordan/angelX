//! Sine, cosine and the rest from basic arithmetic alone, so that the canyon,
//! the camera and the sky come out the same bit for bit in the browser's wasm
//! and on the host (the platform's own `sinf` need not agree with another's
//! in the last place, and a frame's hash would then differ). Everything is
//! done in `f64` with the operations IEEE fixes exactly (add, subtract,
//! multiply, divide, square root); the result is rounded to `f32` once.
const HALF_PI_HI: f64 = 1.570_796_326_794_896_6;
const HALF_PI_LO: f64 = 6.123_233_995_736_766e-17;

fn sin_cos_f64(x: f64) -> (f64, f64) {
    let k = (x / HALF_PI_HI).round();
    let r = (x - k * HALF_PI_HI) - k * HALF_PI_LO;
    let r2 = r * r;
    let sin = r
        * (1.0
            + r2 * (-1.0 / 6.0
                + r2 * (1.0 / 120.0
                    + r2 * (-1.0 / 5_040.0
                        + r2 * (1.0 / 362_880.0
                            + r2 * (-1.0 / 39_916_800.0
                                + r2 * (1.0 / 6_227_020_800.0
                                    + r2 * (-1.0 / 1_307_674_368_000.0))))))));
    let cos = 1.0
        + r2 * (-0.5
            + r2 * (1.0 / 24.0
                + r2 * (-1.0 / 720.0
                    + r2 * (1.0 / 40_320.0
                        + r2 * (-1.0 / 3_628_800.0
                            + r2 * (1.0 / 479_001_600.0 + r2 * (-1.0 / 87_178_291_200.0)))))));
    match (k as i64).rem_euclid(4) {
        0 => (sin, cos),
        1 => (cos, -sin),
        2 => (-sin, -cos),
        _ => (-cos, sin),
    }
}

fn atan_f64(x: f64) -> f64 {
    let (neg, x) = (x < 0.0, x.abs());
    let inv = x > 1.0;
    let mut t = if inv { 1.0 / x } else { x };
    // Halve the angle twice: atan t = 2 atan (t / (1 + sqrt (1 + t t))).
    for _ in 0..2 {
        t /= 1.0 + (1.0 + t * t).sqrt();
    }
    let t2 = t * t;
    let series = t
        * (1.0
            + t2 * (-1.0 / 3.0
                + t2 * (1.0 / 5.0
                    + t2 * (-1.0 / 7.0
                        + t2 * (1.0 / 9.0
                            + t2 * (-1.0 / 11.0 + t2 * (1.0 / 13.0 + t2 * (-1.0 / 15.0))))))));
    let mut a = 4.0 * series;
    if inv {
        a = HALF_PI_HI - a;
    }
    if neg { -a } else { a }
}

fn atan2_f64(y: f64, x: f64) -> f64 {
    const PI: f64 = 2.0 * HALF_PI_HI;
    if x == 0.0 {
        return if y > 0.0 {
            HALF_PI_HI
        } else if y < 0.0 {
            -HALF_PI_HI
        } else {
            0.0
        };
    }
    let a = atan_f64(y / x);
    if x > 0.0 {
        a
    } else if y >= 0.0 {
        a + PI
    } else {
        a - PI
    }
}

/// The deterministic versions, as methods on `f32` under their own names.
pub trait Det {
    fn dsin(self) -> f32;
    fn dcos(self) -> f32;
    fn dsin_cos(self) -> (f32, f32);
    fn dtan(self) -> f32;
    fn datan(self) -> f32;
    fn datan2(self, x: f32) -> f32;
}

impl Det for f32 {
    fn dsin(self) -> f32 {
        sin_cos_f64(f64::from(self)).0 as f32
    }
    fn dcos(self) -> f32 {
        sin_cos_f64(f64::from(self)).1 as f32
    }
    fn dsin_cos(self) -> (f32, f32) {
        let (s, c) = sin_cos_f64(f64::from(self));
        (s as f32, c as f32)
    }
    fn dtan(self) -> f32 {
        let (s, c) = sin_cos_f64(f64::from(self));
        (s / c) as f32
    }
    fn datan(self) -> f32 {
        atan_f64(f64::from(self)) as f32
    }
    fn datan2(self, x: f32) -> f32 {
        atan2_f64(f64::from(self), f64::from(x)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arithmetic_trig_agrees_with_the_platforms_to_a_few_ulps_over_the_whole_circle() {
        let mut worst = 0.0f32;
        for k in -2000..2000 {
            let x = k as f32 * 0.0097;
            let (s, c) = x.dsin_cos();
            worst = worst.max((s - x.sin()).abs()).max((c - x.cos()).abs());
            assert!((s * s + c * c - 1.0).abs() < 1e-6);
        }
        for k in -300..300 {
            let (y, x) = (k as f32 * 0.037, 1.3 - k as f32 * 0.011);
            worst = worst.max((y.datan2(x) - y.atan2(x)).abs());
            worst = worst.max((y.datan() - y.atan()).abs());
        }
        assert!(worst < 2e-6, "{worst}");
        assert!((0.7f32.dtan() - 0.7f32.tan()).abs() < 1e-6);
    }
}
