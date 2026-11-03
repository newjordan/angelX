use super::*;
use crate::rules::floor_plates::games_b_bend_cases_tests as cases;
use serde_json::json;

fn rnd(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn rec(name: &str, input: serde_json::Value, ret: impl serde::Serialize) {
    cases::record(name, input, json!({"ret": ret}));
}

/// The integer maths of the ride.
#[test]
#[ignore]
fn record_lightgallop_math() {
    for t in (-300..600).step_by(3) {
        rec("lightgallop.math.sin", json!({"turns": t}), math::sin(t));
        rec("lightgallop.math.cos", json!({"turns": t}), math::cos(t));
    }
    for b in (-2100..3100).step_by(7) {
        rec("lightgallop.math.sin_b", json!({"brads": b}), math::sin_b(b));
        rec("lightgallop.math.cos_b", json!({"brads": b}), math::cos_b(b));
    }
    let mut st = 0x1234_5678_9abc_def1u64;
    let mut pairs: Vec<(i32, i32)> = vec![(0, 1), (1, 0), (0, -1), (-1, 0), (1, 1), (-5, -5), (3000, 7), (-4096, 4095), (0, 0)];
    for _ in 0..500 {
        let r = rnd(&mut st);
        pairs.push((((r % 8001) as i32) - 4000, (((r >> 20) % 8001) as i32) - 4000));
    }
    for &(a, b) in &pairs {
        rec("lightgallop.math.atan2", json!({"y": a, "x": b}), math::atan2(a, b));
        rec("lightgallop.math.unit", json!({"dx": a, "dy": b}), math::unit(i64::from(a), i64::from(b)));
    }
    for _ in 0..400 {
        let r = rnd(&mut st);
        let (from, to) = ((r % 1024) as i32, ((r >> 12) % 1024) as i32);
        let (ease, slew) = (((r >> 24) % 9) as i32, ((r >> 30) % 70) as i32 + 1);
        rec("lightgallop.math.turn_toward", json!({"from": from, "to": to, "ease": ease, "slew": slew}), math::turn_toward(from, to, ease, slew));
    }
    for t in (-100..4300).step_by(11) {
        rec("lightgallop.math.smooth", json!({"t": t}), math::smooth(t));
    }
    for n in [0u64, 1, 2, 3, 4, 15, 16, 17, 99, 100, 65535, 65536, 1 << 40, (1 << 62) + 12345, (1 << 63) - 1] {
        rec("lightgallop.math.isqrt", json!({"n": n}), math::isqrt(n));
    }
    for _ in 0..300 {
        let n = rnd(&mut st) >> (rnd(&mut st) % 40);
        rec("lightgallop.math.isqrt", json!({"n": n}), math::isqrt(n));
    }
}
