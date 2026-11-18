//! Integer geometry for the road: a square root, a sine table, an arctangent
//! table. Nothing here touches a float, so the same ticks replay byte for
//! byte on every host (native, wasm, the browser seats).
//!
//! Angles in the road's code come in two units. A *turn* is 1/256 of a
//! circle and is how the builders place arcs; a *brad* is 1/1024 of a circle
//! and is how a rider's lean is kept. Screen y points down, so an angle of 64
//! turns points at the ground.

pub const Q: i32 = 256;
pub const ONE: i32 = 4096;

const SIN: [i16; 65] = [
    0, 101, 201, 301, 401, 501, 601, 700, 799, 897, 995, 1092, 1189, 1285, 1380, 1474, 1567, 1660,
    1751, 1842, 1931, 2019, 2106, 2191, 2276, 2359, 2440, 2520, 2598, 2675, 2751, 2824, 2896, 2967,
    3035, 3102, 3166, 3229, 3290, 3349, 3406, 3461, 3513, 3564, 3612, 3659, 3703, 3745, 3784, 3822,
    3857, 3889, 3920, 3948, 3973, 3996, 4017, 4036, 4052, 4065, 4076, 4085, 4091, 4095, 4096,
];

const ATAN: [i16; 65] = [
    0, 3, 5, 8, 10, 13, 15, 18, 20, 23, 25, 28, 30, 33, 35, 38, 40, 42, 45, 47, 49, 52, 54, 56, 58,
    61, 63, 65, 67, 69, 71, 74, 76, 78, 80, 82, 84, 85, 87, 89, 91, 93, 95, 96, 98, 100, 102, 103,
    105, 106, 108, 110, 111, 113, 114, 116, 117, 119, 120, 121, 123, 124, 125, 127, 128,
];

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

pub fn cos(turns: i32) -> i32 {
    sin(turns + 64)
}

pub fn sin_b(brads: i32) -> i32 {
    let a = brads.rem_euclid(1024);
    let (t, f) = (a / 4, a % 4);
    sin(t) + (sin(t + 1) - sin(t)) * f / 4
}

pub fn cos_b(brads: i32) -> i32 {
    sin_b(brads + 256)
}

pub fn unit(dx: i64, dy: i64) -> (i32, i32) {
    let len = isqrt((dx * dx + dy * dy) as u64) as i64;
    if len == 0 {
        return (ONE, 0);
    }
    ((dx * i64::from(ONE) / len) as i32, (dy * i64::from(ONE) / len) as i32)
}

pub fn atan2(y: i32, x: i32) -> i32 {
    if x == 0 && y == 0 {
        return 0;
    }
    let (ax, ay) = (i64::from(x.abs()), i64::from(y.abs()));
    let octant = |small: i64, large: i64| -> i32 {
        let r = small * 4096 / large;
        let (k, f) = ((r / 64) as usize, (r % 64) as i32);
        let (a, b) = (i32::from(ATAN[k.min(64)]), i32::from(ATAN[(k + 1).min(64)]));
        a + (b - a) * f / 64
    };
    let first = if ay <= ax { octant(ay, ax) } else { 256 - octant(ax, ay) };
    let a = match (x >= 0, y >= 0) {
        (true, true) => first,
        (false, true) => 512 - first,
        (false, false) => 512 + first,
        (true, false) => 1024 - first,
    };
    a.rem_euclid(1024)
}

pub fn turn_toward(from: i32, to: i32, ease: i32, slew: i32) -> i32 {
    let gap = (to - from + 512).rem_euclid(1024) - 512;
    let step = (gap / ease.max(1)).clamp(-slew, slew);
    let step = if gap != 0 && step == 0 { gap.signum() } else { step };
    (from + step).rem_euclid(1024)
}

pub fn smooth(t: i32) -> i32 {
    let t = i64::from(t.clamp(0, ONE));
    let one = i64::from(ONE);
    ((t * t * (3 * one - 2 * t)) / (one * one)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_square_root_is_exact_on_squares_and_floors_between() {
        for n in [0u64, 1, 2, 3, 4, 15, 16, 17, 99, 100, 65535, 65536, 1 << 40] {
            let r = isqrt(n);
            assert!(r * r <= n && (r + 1) * (r + 1) > n, "{n}");
        }
    }

    #[test]
    fn the_circle_closes_and_keeps_its_length() {
        for a in 0..256 {
            let (s, c) = (sin(a), cos(a));
            let len = isqrt((s * s + c * c) as u64) as i32;
            assert!((len - ONE).abs() <= 24, "turn {a}: {len}");
        }
        assert_eq!((sin(0), sin(64), sin(128), sin(192)), (0, 4096, 0, -4096));
        assert_eq!(cos(0), 4096);
        assert_eq!(sin(-64), -4096);
    }

    #[test]
    fn the_arctangent_walks_the_compass() {
        assert_eq!(atan2(0, 1), 0);
        assert_eq!(atan2(1, 0), 256);
        assert_eq!(atan2(0, -1), 512);
        assert_eq!(atan2(-1, 0), 768);
        assert!((atan2(1, 1) - 128).abs() <= 1);
        assert!((atan2(-5, -5) - 640).abs() <= 1);
        for a in (0..1024).step_by(7) {
            let (s, c) = (sin(a / 4), cos(a / 4));
            let back = atan2(s, c);
            let gap = (back - a / 4 * 4 + 512).rem_euclid(1024) - 512;
            assert!(gap.abs() <= 3, "brad {a}: {back}");
        }
    }

    #[test]
    fn turning_takes_the_short_way_and_settles() {
        assert_eq!(turn_toward(1000, 10, 1, 100), 10);
        assert_eq!(turn_toward(1000, 10, 1, 20), 1020);
        let mut a = 10;
        for _ in 0..40 {
            a = turn_toward(a, 1000, 3, 30);
        }
        assert_eq!(a, 1000);
    }
}
