// Case generator for the numerics group (test tooling, not product code).
//
//   ~/.rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/rustc -O gen_cases.rs -o gen_cases
//   DELVE_CASES=<dir> ./gen_cases [per-function count] [seed]
//   (ONLY=RS.f_sin,RS.f_cos limits the functions written)
//
// Writes `{"fn":"W.add","in":[..],"out":..}` JSONL, one file per function.
// Floats are printed as the exact f64 of the f32 (so they read back exactly);
// NaN and the infinities are the strings "nan", "inf", "-inf".
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / 16777216.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.unit() * (hi - lo)
    }
}

// An input float. NaN keeps its sign (copysign reads it) as "nan" / "-nan".
fn jf(x: f32) -> String {
    if x.is_nan() {
        (if x.is_sign_negative() { "\"-nan\"" } else { "\"nan\"" }).into()
    } else if x == f32::INFINITY {
        "\"inf\"".into()
    } else if x == f32::NEG_INFINITY {
        "\"-inf\"".into()
    } else {
        format!("{:e}", x as f64)
    }
}

struct Out {
    dir: String,
    only: Option<Vec<String>>,
    files: HashMap<String, BufWriter<File>>,
}
impl Out {
    fn emit(&mut self, name: &str, inp: &[String], out: String) {
        if let Some(only) = &self.only {
            if !only.iter().any(|n| n == name) {
                return;
            }
        }
        let dir = self.dir.clone();
        let w = self.files.entry(name.to_string()).or_insert_with(|| {
            BufWriter::new(File::create(format!("{}/{}.jsonl", dir, name)).unwrap())
        });
        // outputs compare NaN by class only
        let out = if out == "\"-nan\"" { "\"nan\"".to_string() } else { out };
        writeln!(w, "{{\"fn\":\"{}\",\"in\":[{}],\"out\":{}}}", name, inp.join(","), out).unwrap();
    }
}

fn special_f32() -> Vec<f32> {
    let mut v = vec![
        0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 3.5, 0.49999997, -0.49999997, 0.50000006,
        8388607.5, 8388608.0, 16777216.0, 4294967296.0, 4294967040.0, 2147483648.0, -2147483648.0,
        2147483520.0, -2147483904.0, 18446744073709551616.0, 1.8446743e19, 1e-45, -1e-45, 1.1754942e-38,
        1.1754944e-38, f32::MAX, f32::MIN, f32::MIN_POSITIVE, f32::EPSILON, f32::INFINITY, f32::NEG_INFINITY,
        f32::NAN, 3.1415927, -3.1415927, 1.5707964, 6.2831855, 0.1, 0.2, 0.3, 10.0, 100.0, 1000.0, 1e10, 1e-10,
        1e20, 1e30, 1e38, 3e38, 2e38, 1e-20, 1e-30,
    ];
    for k in -40..=40 {
        v.push(k as f32 * 0.5);
    }
    v
}

// A float from a mix of strategies: raw bits, ranges, small integers and halves, specials.
fn fany(r: &mut Rng, sp: &[f32]) -> f32 {
    match r.below(10) {
        0 | 1 => loop {
            let f = f32::from_bits((r.next() >> 32) as u32);
            if f.is_finite() {
                return f;
            }
        },
        2 => r.range(-10.0, 10.0),
        3 => r.range(-1000.0, 1000.0),
        4 => (r.below(200) as f32 - 100.0) / 2.0,
        5 => sp[r.below(sp.len() as u64) as usize],
        6 => r.range(-1.0, 1.0),
        7 => r.range(-100000.0, 100000.0),
        8 => {
            // near an integer or half, a few ulps off
            let base = (r.below(2000) as f32 - 1000.0) / 2.0;
            f32::from_bits((base.to_bits() as i64 + r.below(7) as i64 - 3) as u32)
        }
        _ => {
            let e = r.below(60) as i32 - 30;
            r.range(-1.0, 1.0) * 2f32.powi(e)
        }
    }
}

fn ffin(r: &mut Rng, sp: &[f32]) -> f32 {
    loop {
        let f = fany(r, sp);
        if f.is_finite() {
            return f;
        }
    }
}

fn u64any(r: &mut Rng) -> u64 {
    match r.below(9) {
        0 => r.below(100),
        1 => r.below(1 << 20),
        2 => r.next() >> 32,
        3 => r.next(),
        4 => [0u64, 1, 2, 3, u64::MAX, u64::MAX - 1, 1 << 32, (1 << 32) - 1, (1 << 32) + 1, 1 << 63, (1 << 63) - 1, 0xFFFF_FFFF_0000_0000, 0x0000_0001_FFFF_FFFF][r.below(13) as usize],
        5 => 1u64 << r.below(64),
        6 => (1u64 << r.below(64)).wrapping_sub(1),
        7 => r.next() >> r.below(64),
        _ => r.next() & (u64::MAX << r.below(64)),
    }
}

fn u32any(r: &mut Rng) -> u32 {
    match r.below(7) {
        0 => r.below(100) as u32,
        1 => r.below(70000) as u32,
        2 => (r.next() >> 32) as u32,
        3 => [0u32, 1, 2, 3, u32::MAX, u32::MAX - 1, 1 << 31, (1 << 31) - 1, 65535, 65536][r.below(10) as usize],
        4 => 1u32 << r.below(32),
        5 => (1u32 << r.below(32)).wrapping_sub(1),
        _ => (r.next() >> 32) as u32 >> r.below(32),
    }
}

fn i32any(r: &mut Rng) -> i32 {
    let u = u32any(r) as i32;
    if r.below(2) == 0 { u } else { u.wrapping_neg() }
}

fn i32small(r: &mut Rng) -> i32 {
    r.below(2001) as i32 - 1000
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4000);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0x9E3779B97F4A7C15);
    let dir = std::env::var("DELVE_CASES").expect("set DELVE_CASES to the case output directory");
    std::fs::create_dir_all(&dir).unwrap();
    // ONLY=RS.f_sin,RS.f_cos limits the output to those functions (for bulk runs).
    let only = std::env::var("ONLY").ok().map(|v| v.split(',').map(|x| x.to_string()).collect());
    let mut o = Out { dir, only, files: HashMap::new() };
    let mut r = Rng(seed | 1);
    let sp = special_f32();
    let s = |x: u64| x.to_string();
    let si = |x: i32| x.to_string();
    let su = |x: u32| x.to_string();
    let sb = |x: bool| (if x { "true" } else { "false" }).to_string();

    // ---- W (u64) ----
    for _ in 0..n {
        let (a, b) = (u64any(&mut r), u64any(&mut r));
        let nsh = match r.below(3) { 0 => r.below(64) as u32, 1 => r.below(130) as u32, _ => u32any(&mut r) };
        let d = match r.below(3) { 0 => r.below(20) as u32 + 1, 1 => u32any(&mut r).max(1), _ => (r.below(100000) as u32).max(1) };
        let ab = [s(a), s(b)];
        o.emit("W.add", &ab, s(a.wrapping_add(b)));
        o.emit("W.sub", &ab, s(a.wrapping_sub(b)));
        o.emit("W.mul", &ab, s(a.wrapping_mul(b)));
        o.emit("W.xor", &ab, s(a ^ b));
        o.emit("W.and", &ab, s(a & b));
        o.emit("W.or", &ab, s(a | b));
        o.emit("W.not", &[s(a)], s(!a));
        o.emit("W.neg", &[s(a)], s(a.wrapping_neg()));
        o.emit("W.shl", &[s(a), su(nsh)], s(a.wrapping_shl(nsh)));
        o.emit("W.shr", &[s(a), su(nsh)], s(a.wrapping_shr(nsh)));
        o.emit("W.rotl", &[s(a), su(nsh)], s(a.rotate_left(nsh)));
        o.emit("W.rotr", &[s(a), su(nsh)], s(a.rotate_right(nsh)));
        o.emit("W.eq", &ab, sb(a == b));
        o.emit("W.ne", &ab, sb(a != b));
        o.emit("W.lt", &ab, sb(a < b));
        o.emit("W.le", &ab, sb(a <= b));
        o.emit("W.gt", &ab, sb(a > b));
        o.emit("W.ge", &ab, sb(a >= b));
        o.emit("W.is_zero", &[s(a)], sb(a == 0));
        o.emit("W.min", &ab, s(a.min(b)));
        o.emit("W.max", &ab, s(a.max(b)));
        o.emit("W.sat_add", &ab, s(a.saturating_add(b)));
        o.emit("W.sat_sub", &ab, s(a.saturating_sub(b)));
        o.emit("W.abs_diff", &ab, s(a.abs_diff(b)));
        if b != 0 {
            o.emit("W.div", &ab, s(a / b));
            o.emit("W.rem", &ab, s(a % b));
        }
        o.emit("W.div_u32", &[s(a), su(d)], s(a / d as u64));
        o.emit("W.rem_u32", &[s(a), su(d)], su((a % d as u64) as u32));
        o.emit("W.mul_u32", &[s(a), su(d)], s(a.wrapping_mul(d as u64)));
        o.emit("W.add_u32", &[s(a), su(d)], s(a.wrapping_add(d as u64)));
        o.emit("W.sub_u32", &[s(a), su(d)], s(a.wrapping_sub(d as u64)));
        o.emit("W.lo32", &[s(a)], su(a as u32));
        o.emit("W.hi32", &[s(a)], su((a >> 32) as u32));
        o.emit("W.of_u32", &[su(d)], s(d as u64));
        o.emit("W.to_j", &[s(a)], s(a));
        o.emit("W.count_ones", &[s(a)], su(a.count_ones()));
        o.emit("W.leading_zeros", &[s(a)], su(a.leading_zeros()));
        o.emit("W.trailing_zeros", &[s(a)], su(a.trailing_zeros()));
        o.emit("W.as_f32", &[s(a)], jf(a as f32));
    }
    // u64 -> f32 rounding edge cases (ties at the 24-bit boundary)
    for e in 24..64u32 {
        for k in 0..16u64 {
            let base = (1u64 << e) | (k << (e - 24).saturating_sub(4));
            for d in [0u64, 1, (1 << (e - 24)) / 2 - 1, (1 << (e - 24)) / 2, (1 << (e - 24)) / 2 + 1, (1 << (e - 24)) - 1] {
                let a = base.wrapping_add(d);
                o.emit("W.as_f32", &[s(a)], jf(a as f32));
            }
        }
    }

    // ---- more u64 and i64 (carried as u64 bit patterns) ----
    for _ in 0..n {
        let (a, b) = (u64any(&mut r), u64any(&mut r));
        let e = r.below(40) as u32;
        let bsmall = (r.below(1000) + 1) as u64;
        let b2 = if r.below(2) == 0 { b } else { bsmall };
        let ab = [s(a), s(b)];
        o.emit("W.is_multiple_of", &[s(a), s(b2)], sb(if b2 == 0 { a == 0 } else { a % b2 == 0 }));
        o.emit("W.isqrt", &[s(a)], s(a.isqrt()));
        o.emit("W.pow", &[s(a), su(e)], s(a.wrapping_pow(e)));
        o.emit("W.sat_mul", &ab, s(a.saturating_mul(b)));
        let (ia, ib) = (a as i64, b as i64);
        let nsh = r.below(70) as u32;
        o.emit("W.s_abs", &[s(a)], s(ia.wrapping_abs() as u64));
        o.emit("W.s_lt", &ab, sb(ia < ib));
        o.emit("W.s_le", &ab, sb(ia <= ib));
        o.emit("W.s_gt", &ab, sb(ia > ib));
        o.emit("W.s_ge", &ab, sb(ia >= ib));
        o.emit("W.s_min", &ab, s(ia.min(ib) as u64));
        o.emit("W.s_max", &ab, s(ia.max(ib) as u64));
        o.emit("W.s_signum", &[s(a)], s(ia.signum() as u64));
        o.emit("W.s_sar", &[s(a), su(nsh)], s(ia.wrapping_shr(nsh) as u64));
        o.emit("W.s_as_f32", &[s(a)], jf(ia as f32));
        let ib2 = if r.below(2) == 0 { ib } else { (b2 as i64) * if r.below(2) == 0 { 1 } else { -1 } };
        if ib2 != 0 && !(ia == i64::MIN && ib2 == -1) {
            let bb = [s(a), s(ib2 as u64)];
            o.emit("W.s_div", &bb, s((ia / ib2) as u64));
            o.emit("W.s_rem", &bb, s((ia % ib2) as u64));
            o.emit("W.s_div_euclid", &bb, s(ia.div_euclid(ib2) as u64));
            o.emit("W.s_rem_euclid", &bb, s(ia.rem_euclid(ib2) as u64));
        }
        let (lo, hi) = if ib <= (u64any(&mut r) as i64) { (ib, u64any(&mut r) as i64) } else { (ib, ib) };
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        o.emit("W.s_clamp", &[s(a), s(lo as u64), s(hi as u64)], s(ia.clamp(lo, hi) as u64));
        let i32v = i32any(&mut r);
        o.emit("W.s_of_i32", &[si(i32v)], s(i32v as i64 as u64));
    }
    // ---- i32 / u32 ----
    for _ in 0..n {
        let (a, b) = if r.below(3) == 0 { (i32small(&mut r), i32small(&mut r)) } else { (i32any(&mut r), i32any(&mut r)) };
        let c = i32any(&mut r);
        let (ua, ub) = (u32any(&mut r), u32any(&mut r));
        let nsh = r.below(40) as u32;
        let ab = [si(a), si(b)];
        o.emit("RS.i_add", &ab, si(a.wrapping_add(b)));
        o.emit("RS.i_sub", &ab, si(a.wrapping_sub(b)));
        o.emit("RS.i_mul", &ab, si(a.wrapping_mul(b)));
        o.emit("RS.i_neg", &[si(a)], si(a.wrapping_neg()));
        o.emit("RS.i_abs", &[si(a)], si(a.wrapping_abs()));
        o.emit("RS.i_unsigned_abs", &[si(a)], su(a.unsigned_abs()));
        o.emit("RS.i_signum", &[si(a)], si(a.signum()));
        if b != 0 && !(a == i32::MIN && b == -1) {
            o.emit("RS.i_div", &ab, si(a / b));
            o.emit("RS.i_rem", &ab, si(a % b));
            o.emit("RS.i_div_euclid", &ab, si(a.div_euclid(b)));
            o.emit("RS.i_rem_euclid", &ab, si(a.rem_euclid(b)));
        } else if b != 0 && a == i32::MIN && b == -1 {
            // wrapping_rem_euclid is 0, quotient wraps
            o.emit("RS.i_rem_euclid", &ab, si(a.wrapping_rem_euclid(b)));
        }
        o.emit("RS.i_lt", &ab, sb(a < b));
        o.emit("RS.i_le", &ab, sb(a <= b));
        o.emit("RS.i_gt", &ab, sb(a > b));
        o.emit("RS.i_ge", &ab, sb(a >= b));
        o.emit("RS.i_eq", &ab, sb(a == b));
        o.emit("RS.i_ne", &ab, sb(a != b));
        o.emit("RS.i_min", &ab, si(a.min(b)));
        o.emit("RS.i_max", &ab, si(a.max(b)));
        let (lo, hi) = if b <= c { (b, c) } else { (c, b) };
        o.emit("RS.i_clamp", &[si(a), si(lo), si(hi)], si(a.clamp(lo, hi)));
        o.emit("RS.i_abs_diff", &ab, su(a.abs_diff(b)));
        o.emit("RS.i_shr", &[si(a), su(nsh)], si(a.wrapping_shr(nsh)));
        o.emit("RS.i_shl", &[si(a), su(nsh)], si(a.wrapping_shl(nsh)));
        o.emit("RS.i_sat_add", &ab, si(a.saturating_add(b)));
        o.emit("RS.i_sat_sub", &ab, si(a.saturating_sub(b)));
        o.emit("RS.i_pow", &[si(a), su(nsh % 12)], si(a.wrapping_pow(nsh % 12)));
        o.emit("RS.as_u8", &[su(ua)], su(ua as u8 as u32));
        o.emit("RS.as_u16", &[su(ua)], su(ua as u16 as u32));
        o.emit("RS.as_i8", &[su(ua)], si(ua as i8 as i32));
        o.emit("RS.as_i16", &[su(ua)], si(ua as i16 as i32));
        let uab = [su(ua), su(ub)];
        o.emit("RS.u_sat_add", &uab, su(ua.saturating_add(ub)));
        o.emit("RS.u_sat_sub", &uab, su(ua.saturating_sub(ub)));
        o.emit("RS.u_sat_mul", &uab, su(ua.saturating_mul(ub)));
        o.emit("RS.u_min", &uab, su(ua.min(ub)));
        o.emit("RS.u_max", &uab, su(ua.max(ub)));
        o.emit("RS.u_abs_diff", &uab, su(ua.abs_diff(ub)));
        let (ulo, uhi) = if ub <= ua { (ub, ua) } else { (ua, ub) };
        let x = u32any(&mut r);
        o.emit("RS.u_clamp", &[su(x), su(ulo), su(uhi)], su(x.clamp(ulo, uhi)));
        o.emit("RS.u_shl", &[su(ua), su(nsh)], su(ua.wrapping_shl(nsh)));
        o.emit("RS.u_shr", &[su(ua), su(nsh)], su(ua.wrapping_shr(nsh)));
        o.emit("RS.u_rotl", &[su(ua), su(nsh)], su(ua.rotate_left(nsh)));
        o.emit("RS.u_rotr", &[su(ua), su(nsh)], su(ua.rotate_right(nsh)));
        o.emit("RS.u_count_ones", &[su(ua)], su(ua.count_ones()));
        o.emit("RS.u_leading_zeros", &[su(ua)], su(ua.leading_zeros()));
        o.emit("RS.u_trailing_zeros", &[su(ua)], su(ua.trailing_zeros()));
        o.emit("RS.u_isqrt", &[su(ua)], su(ua.isqrt()));
        o.emit("RS.u_pow", &[su(ua), su(nsh % 40)], su(ua.wrapping_pow(nsh % 40)));
        o.emit("RS.u_as_f32", &[su(ua)], jf(ua as f32));
        o.emit("RS.i_as_f32", &[si(a)], jf(a as f32));
    }

    // ---- f32 ----
    for _ in 0..n {
        let (x, y) = (fany(&mut r, &sp), fany(&mut r, &sp));
        let z = fany(&mut r, &sp);
        let xy = [jf(x), jf(y)];
        o.emit("RS.f_abs", &[jf(x)], jf(x.abs()));
        o.emit("RS.f_neg", &[jf(x)], jf(-x));
        o.emit("RS.f_signum", &[jf(x)], jf(x.signum()));
        o.emit("RS.f_copysign", &xy, jf(x.copysign(y)));
        o.emit("RS.f_round", &[jf(x)], jf(x.round()));
        o.emit("RS.f_fract", &[jf(x)], jf(x.fract()));
        o.emit("RS.f_min", &xy, jf(x.min(y)));
        o.emit("RS.f_max", &xy, jf(x.max(y)));
        if !(x.is_nan() || y.is_nan() || z.is_nan()) {
            let (lo, hi) = if y <= z { (y, z) } else { (z, y) };
            o.emit("RS.f_clamp", &[jf(x), jf(lo), jf(hi)], jf(x.clamp(lo, hi)));
        } else if !(y.is_nan() || z.is_nan()) {
            let (lo, hi) = if y <= z { (y, z) } else { (z, y) };
            o.emit("RS.f_clamp", &[jf(x), jf(lo), jf(hi)], jf(x.clamp(lo, hi)));
        }
        o.emit("RS.f_rem_euclid", &xy, jf(x.rem_euclid(y)));
        o.emit("RS.f_div_euclid", &xy, jf(x.div_euclid(y)));
        o.emit("RS.f_to_degrees", &[jf(x)], jf(x.to_degrees()));
        o.emit("RS.f_to_radians", &[jf(x)], jf(x.to_radians()));
        o.emit("RS.f_as_u32", &[jf(x)], su(x as u32));
        o.emit("RS.f_as_u8", &[jf(x)], su(x as u8 as u32));
        o.emit("RS.f_as_u16", &[jf(x)], su(x as u16 as u32));
        o.emit("RS.f_as_i8", &[jf(x)], si(x as i8 as i32));
        o.emit("RS.f_as_i16", &[jf(x)], si(x as i16 as i32));
        let sm = r.range(-300.0, 300.0);
        o.emit("RS.f_as_u8", &[jf(sm)], su(sm as u8 as u32));
        o.emit("RS.f_as_i8", &[jf(sm)], si(sm as i8 as i32));
        let md = r.range(-70000.0, 70000.0);
        o.emit("RS.f_as_u16", &[jf(md)], su(md as u16 as u32));
        o.emit("RS.f_as_i16", &[jf(md)], si(md as i16 as i32));
        o.emit("RS.f_as_i32", &[jf(x)], si(x as i32));
        if (x as u64) < (1u64 << 32) || x.is_nan() || x < 0.0 {
            o.emit("RS.f_as_usize", &[jf(x)], s(x as usize as u64));
        }
        o.emit("RS.f_as_u64", &[jf(x)], s(x as u64));
        o.emit("RS.f_hypot", &xy, jf(x.hypot(y)));
        o.emit("RS.f_sqrt", &[jf(x)], jf(x.sqrt()));
        o.emit("RS.f_floor", &[jf(x)], jf(x.floor()));
        o.emit("RS.f_ceil", &[jf(x)], jf(x.ceil()));
        o.emit("RS.f_trunc", &[jf(x)], jf(x.trunc()));
        o.emit("RS.f_tan", &[jf(x)], jf(x.tan()));
        o.emit("RS.f_atan", &[jf(x)], jf(x.atan()));
        o.emit("RS.f_asin", &[jf(x)], jf(x.asin()));
        o.emit("RS.f_acos", &[jf(x)], jf(x.acos()));
        o.emit("RS.f_atan2", &xy, jf(x.atan2(y)));
        o.emit("RS.f_sin", &[jf(x)], jf(x.sin()));
        o.emit("RS.f_cos", &[jf(x)], jf(x.cos()));
        o.emit("RS.f_exp", &[jf(x)], jf(x.exp()));
        o.emit("RS.f_ln", &[jf(x)], jf(x.ln()));
        // powi: small and large exponents, both signs
        let e = match r.below(4) { 0 => i32small(&mut r) % 12, 1 => i32small(&mut r), 2 => i32any(&mut r) % 100, _ => i32any(&mut r) };
        o.emit("RS.f_powi", &[jf(x), si(e)], jf(x.powi(e)));
        let b = match r.below(3) { 0 => r.range(0.0, 10.0), 1 => r.range(0.0, 2.0), _ => ffin(&mut r, &sp).abs() };
        let p = match r.below(3) { 0 => r.range(-4.0, 4.0), 1 => (r.below(9) as f32 - 4.0) / 2.0, _ => ffin(&mut r, &sp) };
        o.emit("RS.f_powf", &[jf(b), jf(p)], jf(b.powf(p)));
        o.emit("RS.f_powf", &xy, jf(x.powf(y)));
        // sin / cos on game-sized angles
        let a = match r.below(3) { 0 => r.range(-7.0, 7.0), 1 => r.range(-130.0, 130.0), _ => r.range(-2000.0, 2000.0) };
        o.emit("RS.f_sin", &[jf(a)], jf(a.sin()));
        o.emit("RS.f_cos", &[jf(a)], jf(a.cos()));
        let ex = r.range(-30.0, 30.0);
        o.emit("RS.f_exp", &[jf(ex)], jf(ex.exp()));
        let lx = match r.below(3) { 0 => r.range(0.0, 10.0), 1 => r.range(0.0, 1000.0), _ => f32::from_bits(((r.next() >> 33) as u32) & 0x7fff_ffff) };
        o.emit("RS.f_ln", &[jf(lx)], jf(lx.ln()));
        // two-argument functions on game-sized values
        let (gy, gx) = (r.range(-300.0, 300.0), r.range(-300.0, 300.0));
        o.emit("RS.f_hypot", &[jf(gy), jf(gx)], jf(gy.hypot(gx)));
        o.emit("RS.f_atan2", &[jf(gy), jf(gx)], jf(gy.atan2(gx)));
        let (hy, hx) = (r.range(-3.0, 3.0), r.range(-3.0, 3.0));
        o.emit("RS.f_hypot", &[jf(hy), jf(hx)], jf(hy.hypot(hx)));
        o.emit("RS.f_atan2", &[jf(hy), jf(hx)], jf(hy.atan2(hx)));
        o.emit("RS.f_round", &[jf(gy)], jf(gy.round()));
        o.emit("RS.f_round", &[jf(hy)], jf(hy.round()));
    }
    // exp / ln / powf near their limits and special inputs
    for _ in 0..n {
        let x = match r.below(4) { 0 => r.range(-110.0, -80.0), 1 => r.range(80.0, 90.0), 2 => r.range(-104.5, -102.5), _ => r.range(88.0, 89.0) };
        o.emit("RS.f_exp", &[jf(x)], jf(x.exp()));
        let sub = f32::from_bits((r.next() >> 41) as u32 & 0x7fffff);
        o.emit("RS.f_ln", &[jf(sub)], jf(sub.ln()));
        let nx = -r.range(0.0, 50.0);
        o.emit("RS.f_ln", &[jf(nx)], jf(nx.ln()));
        let (bx, by) = match r.below(6) {
            0 => (2.0f32, r.range(127.0, 129.0)),
            1 => (r.range(1.5, 2.5), r.range(120.0, 135.0)),
            2 => (-(r.range(0.0, 8.0)), (r.below(15) as f32) - 7.0),
            3 => (-(r.range(0.0, 8.0)), r.range(-4.0, 4.0)),
            4 => (f32::from_bits((r.next() >> 41) as u32 & 0x7fffff), r.range(-2.0, 2.0)),
            _ => (r.range(0.0, 3.0), r.range(-150.0, -100.0)),
        };
        o.emit("RS.f_powf", &[jf(bx), jf(by)], jf(bx.powf(by)));
        let ex = 127.0 + r.unit() * 2.0;
        o.emit("RS.f_powf", &[jf(2.0), jf(ex)], jf(2.0f32.powf(ex)));
    }
    // atan2 with a subnormal result: exact ties between two floats
    for _ in 0..n {
        let yb = f32::from_bits(((r.next() >> 41) as u32 & 0x7fffff) | if r.below(4) == 0 { 0x00800000 } else { 0 });
        let xe = f32::from_bits(0x3f800000 + ((r.below(30) as u32) << 23)) * if r.below(2) == 0 { 1.0 } else { 3.0 };
        let (yy, xx) = (yb * if r.below(2) == 0 { 1.0 } else { -1.0 }, xe * if r.below(4) == 0 { -1.0 } else { 1.0 });
        o.emit("RS.f_atan2", &[jf(yy), jf(xx)], jf(yy.atan2(xx)));
        let y2 = f32::from_bits(r.next() as u32 & 0x007f_ffff);
        let x2 = f32::from_bits(0x4e80_0000 + ((r.next() >> 41) as u32 & 0x03ff_ffff));
        o.emit("RS.f_atan2", &[jf(y2), jf(x2)], jf(y2.atan2(x2)));
    }
    // sin / cos right at multiples of pi/2 and pi/4, where the reduction is hardest
    for _ in 0..n {
        let k = match r.below(3) { 0 => r.below(64), 1 => r.below(100000), _ => r.below(1 << 30) } as f64;
        let q = if r.below(2) == 0 { std::f64::consts::FRAC_PI_2 } else { std::f64::consts::FRAC_PI_4 };
        let base = (k * q) as f32 * if r.below(2) == 0 { 1.0 } else { -1.0 };
        let x = f32::from_bits((base.to_bits() as i64 + r.below(9) as i64 - 4) as u32);
        if x.is_finite() {
            o.emit("RS.f_sin", &[jf(x)], jf(x.sin()));
            o.emit("RS.f_cos", &[jf(x)], jf(x.cos()));
        }
        let big = f32::from_bits(0x4200_0000 + (r.next() >> 41) as u32 % 0x2b00_0000);
        o.emit("RS.f_sin", &[jf(big)], jf(big.sin()));
        o.emit("RS.f_cos", &[jf(-big)], jf((-big).cos()));
    }
    // half-way values for round, and all edge pairs for min/max/hypot/atan2/rem_euclid
    for k in -300..=300 {
        for d in [0.0f32, 0.25, 0.5, 0.75] {
            let x = k as f32 + d;
            o.emit("RS.f_round", &[jf(x)], jf(x.round()));
            o.emit("RS.f_round", &[jf(-x)], jf(-x.round()));
        }
    }
    for e in 0..40 {
        let x = 2f32.powi(e) + 0.5;
        o.emit("RS.f_round", &[jf(x)], jf(x.round()));
        let x = f32::from_bits(0x4b000000u32.wrapping_add(e as u32));
        o.emit("RS.f_round", &[jf(x)], jf(x.round()));
    }
    for &x in &sp {
        for &y in &sp {
            let xy = [jf(x), jf(y)];
            o.emit("RS.f_hypot", &xy, jf(x.hypot(y)));
            o.emit("RS.f_atan2", &xy, jf(x.atan2(y)));
            o.emit("RS.f_min", &xy, jf(x.min(y)));
            o.emit("RS.f_max", &xy, jf(x.max(y)));
            o.emit("RS.f_rem_euclid", &xy, jf(x.rem_euclid(y)));
            o.emit("RS.f_powf", &xy, jf(x.powf(y)));
            o.emit("RS.f_copysign", &xy, jf(x.copysign(y)));
        }
    }
    // hypot where the squares nearly cancel the double rounding, and subnormals
    for _ in 0..n {
        let e1 = r.below(120) as i32 - 100;
        let e2 = e1 - r.below(40) as i32;
        let x = (1.0 + r.unit()) * 2f32.powi(e1);
        let y = (1.0 + r.unit()) * 2f32.powi(e2);
        o.emit("RS.f_hypot", &[jf(x), jf(y)], jf(x.hypot(y)));
        let sx = f32::from_bits((r.next() >> 41) as u32);
        let sy = f32::from_bits((r.next() >> 41) as u32);
        o.emit("RS.f_hypot", &[jf(sx), jf(sy)], jf(sx.hypot(sy)));
        let bx = f32::from_bits(0x7f00_0000 + (r.next() >> 41) as u32 % 0x7fffff);
        let by = f32::from_bits(0x7f00_0000 + (r.next() >> 41) as u32 % 0x7fffff);
        o.emit("RS.f_hypot", &[jf(bx), jf(by)], jf(bx.hypot(by)));
    }
    // ---- more u32, checked ops, total_cmp ----
    let so = |x: Option<u32>| match x { Some(v) => v.to_string(), None => "null".to_string() };
    let sow = |x: Option<u64>| match x { Some(v) => v.to_string(), None => "null".to_string() };
    for _ in 0..n {
        let (ua, ub) = (u32any(&mut r), u32any(&mut r).max(1));
        let ub2 = if r.below(2) == 0 { ub } else { (r.below(64) + 1) as u32 };
        let (ib, ic) = (i32any(&mut r), if r.below(2) == 0 { i32small(&mut r) } else { i32any(&mut r) });
        o.emit("RS.u_div_ceil", &[su(ua), su(ub2)], su(ua.div_ceil(ub2)));
        o.emit("RS.u_is_multiple_of", &[su(ua), su(ub2)], sb(ua.is_multiple_of(ub2)));
        o.emit("RS.u_is_multiple_of", &[su(ua - ua % ub2), su(ub2)], sb(true));
        o.emit("RS.u_sat_add_signed", &[su(ua), si(ic)], su(ua.saturating_add_signed(ic)));
        let _ = ib;
        o.emit("RS.u_checked_add", &[su(ua), su(ub)], so(ua.checked_add(ub)));
        o.emit("RS.u_checked_sub", &[su(ua), su(ub)], so(ua.checked_sub(ub)));
        o.emit("RS.u_checked_mul", &[su(ua), su(ub)], so(ua.checked_mul(ub)));
        let (wa, wb) = (u64any(&mut r), u64any(&mut r));
        let wb1 = wb.max(1);
        o.emit("W.div_ceil", &[s(wa), s(wb1)], s(wa.div_ceil(wb1)));
        o.emit("W.checked_add", &[s(wa), s(wb)], sow(wa.checked_add(wb)));
        o.emit("W.checked_sub", &[s(wa), s(wb)], sow(wa.checked_sub(wb)));
        o.emit("W.checked_mul", &[s(wa), s(wb)], sow(wa.checked_mul(wb)));
        let (fx, fy) = (fany(&mut r, &sp), fany(&mut r, &sp));
        let ord = |c: std::cmp::Ordering| match c { std::cmp::Ordering::Less => "-1", std::cmp::Ordering::Equal => "0", std::cmp::Ordering::Greater => "1" }.to_string();
        o.emit("RS.f_total_cmp", &[jf(fx), jf(fy)], ord(fx.total_cmp(&fy)));
        o.emit("RS.f_total_lt", &[jf(fx), jf(fy)], sb(fx.total_cmp(&fy).is_lt()));
        o.emit("RS.f_total_le", &[jf(fx), jf(fy)], sb(fx.total_cmp(&fy).is_le()));
    }
    for &x in &sp {
        for &y in &sp {
            o.emit("RS.f_total_cmp", &[jf(x), jf(y)], (match x.total_cmp(&y) { std::cmp::Ordering::Less => "-1", std::cmp::Ordering::Equal => "0", std::cmp::Ordering::Greater => "1" }).to_string());
        }
    }
    // ---- soft double (u64 bit patterns) ----
    let dbits = |x: f64| s(x.to_bits());
    let rd = |r: &mut Rng| -> f64 {
        // a normal double: random sign, exponent in a wide but safe range, any mantissa
        let e = match r.below(4) { 0 => r.below(60) as i64 - 30, 1 => r.below(200) as i64 - 100, _ => r.below(500) as i64 - 250 };
        let mant = match r.below(6) { 0 => 0, 1 => (1u64 << 52) - 1, 2 => 1u64 << r.below(52), _ => r.next() & ((1u64 << 52) - 1) };
        let sign = r.below(2);
        f64::from_bits((sign << 63) | (((e + 1023) as u64) << 52) | mant)
    };
    for _ in 0..n {
        let (a, b, c) = (rd(&mut r), rd(&mut r), rd(&mut r));
        let nz = |x: f64| if x == 0.0 { 0.0f64 } else { x };
        o.emit("RS.d_mul", &[dbits(a), dbits(b)], dbits(nz(a * b)));
        o.emit("RS.d_add", &[dbits(a), dbits(c)], dbits(nz(a + c)));
        o.emit("RS.d_sub", &[dbits(a), dbits(c)], dbits(nz(a - c)));
        o.emit("RS.d_fma", &[dbits(a), dbits(b), dbits(c)], dbits(nz(a.mul_add(b, c))));
        // addend close to -(a*b): cancellation
        let p = a * b;
        let near = f64::from_bits((p.to_bits() as i64 + r.below(9) as i64 - 4) as u64);
        let c2 = -near;
        o.emit("RS.d_fma", &[dbits(a), dbits(b), dbits(c2)], dbits(nz(a.mul_add(b, c2))));
        // addend of about the same size, or a few bits off
        let c3 = f64::from_bits(((p.to_bits() as i64) + (r.below(200) as i64 - 100) * (1i64 << r.below(52))) as u64) * if r.below(2) == 0 { 1.0 } else { -1.0 };
        if c3.is_finite() && c3 != 0.0 {
            o.emit("RS.d_fma", &[dbits(a), dbits(b), dbits(c3)], dbits(nz(a.mul_add(b, c3))));
        }
        // far apart exponents
        let c4 = f64::from_bits(c.to_bits() ^ 0).copysign(a) * 2f64.powi(r.below(160) as i32 - 80);
        if c4.is_finite() && c4 != 0.0 && c4.abs() > 1e-250 && c4.abs() < 1e250 {
            o.emit("RS.d_fma", &[dbits(a), dbits(b), dbits(c4)], dbits(nz(a.mul_add(b, c4))));
        }
        let f = fany(&mut r, &sp);
        if f.is_finite() && f != 0.0 {
            o.emit("RS.d_of_f32", &[jf(f)], dbits(f as f64));
        }
        // double -> f32 across the normal, subnormal and overflow ranges
        let e = r.below(340) as i32 - 190;
        let dv = (1.0 + (r.next() >> 11) as f64 / (1u64 << 53) as f64) * 2f64.powi(e) * if r.below(2) == 0 { 1.0 } else { -1.0 };
        o.emit("RS.d_to_f32", &[dbits(dv)], jf(dv as f32));
        // a double that is nearly a tie between two f32
        let fb = f32::from_bits((r.next() >> 33) as u32 & 0x7f7fffff) as f64;
        let ulp = f32::from_bits(((fb as f32).to_bits() + 1) & 0x7fffffff) as f64 - fb;
        let tie = fb + ulp / 2.0;
        let t = f64::from_bits((tie.to_bits() as i64 + r.below(5) as i64 - 2) as u64);
        o.emit("RS.d_to_f32", &[dbits(t)], jf(t as f32));
        let i = i32any(&mut r);
        o.emit("RS.d_of_i32", &[si(i)], dbits(i as f64));
        let i64v = match r.below(3) { 0 => (r.next() as i64) >> (r.below(63) as u32 + 1), 1 => (r.next() >> 2) as i64 - (1i64 << 61), _ => (r.next() as i64) >> 3 };
        let i64v = i64v.clamp(-(1i64 << 62), 1i64 << 62);
        o.emit("RS.d_of_i64", &[s(i64v as u64)], dbits(i64v as f64));
        let tv = (r.range(-1.0, 1.0) as f64) * 2f64.powi(r.below(31) as i32);
        o.emit("RS.d_trunc_i32", &[dbits(tv)], si(tv as i32));
    }
    // u32 -> f32 cast ties
    for _ in 0..n {
        let k = (r.next() >> 33) as u32 | 0x8000_0000;
        let u = k >> r.below(8) as u32;
        o.emit("RS.u_as_f32", &[su(u)], jf(u as f32));
        let t = ((1u32 << 24) + r.below(1 << 20) as u32) << r.below(8) as u32;
        let t2 = t | (1u32 << (t.trailing_zeros().saturating_sub(1)).min(31));
        o.emit("RS.u_as_f32", &[su(t2)], jf(t2 as f32));
    }
    for w in o.files.values_mut() {
        w.flush().unwrap();
    }
    let mut names: Vec<_> = o.files.keys().cloned().collect();
    names.sort();
    eprintln!("wrote {} files to {}", names.len(), o.dir);
}
