//! Stress: do the glass fast paths transfer to random assets, or are they
//! overfit to the 7 handpicked symbols in the head-to-head?
//!
//! Deterministic (xorshift64*, fixed seed, no external deps). Uses the REAL
//! in-tree code where it is public (`angel_gpu::FxHasher`, `angel_glass::Layer`)
//! and a byte-exact mirror of `backend::symbol_width` (pub(crate), so the
//! example re-pins it against `unicode-width` on every random symbol).
//!
//! cargo run --offline -p angel-glass --example stress-random --release

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::hint::black_box;
use std::time::Instant;

use angel_glass::gpu::FxHasher;
use unicode_width::UnicodeWidthStr;

// ---- deterministic RNG (xorshift64*) ----
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.below((hi - lo + 1) as u64) as u32)
    }
}

// ---- byte-exact mirror of backend::symbol_width (pinned below) ----
#[inline]
fn fast_width(symbol: &str) -> usize {
    let b = symbol.as_bytes();
    if (b.len() == 1 && b[0] >= 0x20)
        || (b.len() == 2 && matches!(b[0], 0xC3..=0xCB | 0xCE | 0xCF | 0xD0 | 0xD1 | 0xD3..=0xD5 | 0xDA))
        || (b.len() == 3
            && b[0] == 0xe2
            && (matches!(b[1], 0x94 | 0x95 | 0xa0..=0xa3) || (b[1] == 0x96 && b[2] < 0xa0)))
    {
        1
    } else if b.len() == 3 {
        fast_wide3(b)
    } else if b.len() == 4 && b[0] == 0xF0 && matches!(b[1], 0x97 | 0xA0..=0xAE | 0xB0..=0xBE) {
        2
    } else {
        UnicodeWidthStr::width(symbol)
    }
}
#[inline]
fn fast_wide3(b: &[u8], ) -> usize {
    let b0 = b[0];
    if matches!(b0, 0xE4..=0xE9) {
        return 2;
    }
    let cp = (u32::from(b0 & 0x0F) << 12) | (u32::from(b[1] & 0x3F) << 6) | u32::from(b[2] & 0x3F);
    match b0 {
        0xE1 if (0x1100..=0x115F).contains(&cp) => 2,
        0xE2 if (0x2000..=0x200A).contains(&cp) || (0x2010..=0x2029).contains(&cp) => 1,
        0xE3
            if (0x3000..=0x3029).contains(&cp)
                || (0x3030..=0x303E).contains(&cp)
                || (0x3041..=0x3096).contains(&cp)
                || (0x309B..=0x309E).contains(&cp)
                || (0x30A0..=0x30FF).contains(&cp)
                || (0x3105..=0x312E).contains(&cp)
                || (0x3131..=0x3163).contains(&cp)
                || (0x3165..=0x318E).contains(&cp)
                || (0x3190..=0x31E4).contains(&cp)
                || (0x3200..=0x321E).contains(&cp)
                || (0x3220..=0x3247).contains(&cp)
                || (0x3250..=0x32FF).contains(&cp)
                || (0x3400..=0x3FFF).contains(&cp) =>
        {
            2
        }
        0xEA if (0xA000..=0xA48C).contains(&cp)
            || (0xA490..=0xA4C6).contains(&cp)
            || (0xAC00..=0xAFFF).contains(&cp) =>
        {
            2
        }
        0xEB | 0xEC => 2,
        0xED if (0xD000..=0xD7A3).contains(&cp) => 2,
        0xEF if (0xFF01..=0xFF60).contains(&cp) || (0xFFE0..=0xFFE6).contains(&cp) => 2,
        0xEF if (0xFF61..=0xFF9D).contains(&cp) || (0xFFA1..=0xFFDF).contains(&cp) => 1,
        _ => UnicodeWidthStr::width(std::str::from_utf8(b).unwrap_or("")),
    }
}

fn sip_hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
fn fx_hash(s: &str) -> u64 {
    let mut h = FxHasher::default();
    s.hash(&mut h);
    h.finish()
}

fn gen_symbol(rng: &mut Rng) -> String {
    match rng.below(100) {
        0..=29 => (rng.range(0x20, 0x7E) as u8 as char).to_string(), // ASCII
        30..=39 => char::from_u32(rng.range(0x2500, 0x28FF)).unwrap().to_string(), // box/block/braille
        40..=54 => {
            // 2-byte incl. adversarial fallthrough (combining, Hebrew, C2, D2, D6-D9, DB)
            let pools = [
                (0xC0u32, 0xFFu32),
                (0x100, 0x2FF),
                (0x370, 0x3FF),
                (0x400, 0x52F),
                (0x300, 0x36F),
                (0x590, 0x5FF),
                (0x600, 0x6FF),
            ];
            let (lo, hi) = pools[rng.below(pools.len() as u64) as usize];
            char::from_u32(rng.range(lo, hi)).unwrap().to_string()
        }
        55..=69 => {
            let pools = [(0x800u32, 0xFFFFu32)];
            let (lo, hi) = pools[0];
            // bias half into E4..=E9 fast block, half anywhere
            let cp = if rng.below(2) == 0 {
                rng.range(0x4000, 0x9FFF)
            } else {
                rng.range(lo, hi)
            };
            match char::from_u32(cp) {
                Some(c) => c.to_string(),
                None => "�".to_string(),
            }
        }
        70..=84 => {
            // supplementary: half in F0 97/A0-AE/B0-BE fast blocks
            let cp = if rng.below(2) == 0 {
                let blocks = [(0x17000u32, 0x187FFu32), (0x20000, 0x2EBEF), (0x30000, 0x323AF)];
                let (lo, hi) = blocks[rng.below(3) as usize];
                rng.range(lo, hi)
            } else {
                rng.range(0x10000, 0x10FFFF)
            };
            match char::from_u32(cp) {
                Some(c) => c.to_string(),
                None => "🦀".to_string(),
            }
        }
        85..=94 => {
            // multi-char: combining, pairs, emoji ZWJ-ish
            let a = char::from_u32(rng.range(0x20, 0x7E)).unwrap();
            let b = char::from_u32(rng.range(0x300, 0x36F)).unwrap_or('́');
            format!("{a}{b}")
        }
        _ => {
            let n = 2 + rng.below(3) as usize;
            (0..n)
                .map(|_| char::from_u32(rng.range(0x20, 0x9FFF)).unwrap_or('x'))
                .collect()
        }
    }
}

fn bench_ns<F: Fn()>(iters: u64, mut f: F) -> f64 {
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn main() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    const N: usize = 20_000;
    let mut corpus: Vec<String> = Vec::with_capacity(N);
    for _ in 0..N {
        corpus.push(gen_symbol(&mut rng));
    }
    // class mix report
    let ascii = corpus.iter().filter(|s| s.len() == 1 && s.as_bytes()[0] < 0x80).count();
    let two = corpus.iter().filter(|s| s.as_bytes().len() == 2).count();
    let three = corpus.iter().filter(|s| s.as_bytes().len() == 3).count();
    let four = corpus.iter().filter(|s| s.as_bytes().len() == 4).count();
    let multi = corpus.iter().filter(|s| s.chars().count() > 1).count();
    println!("CORPUS n={N} ascii1={ascii} len2={two} len3={three} len4={four} multichar={multi}");

    // 1) hash: correctness (determinism) + collisions + speed on RANDOM corpus
    let mut fx_set = HashSet::new();
    let mut collisions = 0u32;
    let mut seen: HashMap<u64, &str> = HashMap::new();
    for s in &corpus {
        assert_eq!(fx_hash(s), fx_hash(s), "fx nondeterministic");
        let h = fx_hash(s);
        if !fx_set.insert(h) && seen.get(&h).map(|p| *p != s.as_str()).unwrap_or(true) {
            // could be a true duplicate symbol hashing same (fine) or a collision
            if seen.get(&h).map(|p| *p != s.as_str()).unwrap_or(false) {
                collisions += 1;
            }
        }
        seen.entry(h).or_insert(s.as_str());
    }
    println!("HASH collisions on {N} random symbols: {collisions} (duplicates share hash, expected ~0 true collisions)");

    // time: iterate corpus, accumulate to defeat dead-code elimination
    let reps = 30u64;
    let t_sip = {
        let t = Instant::now();
        let mut acc = 0u64;
        for _ in 0..reps {
            for s in &corpus {
                acc ^= sip_hash(black_box(s));
            }
        }
        black_box(acc);
        t.elapsed().as_nanos() as f64 / (reps as f64 * N as f64)
    };
    let t_fx = {
        let t = Instant::now();
        let mut acc = 0u64;
        for _ in 0..reps {
            for s in &corpus {
                acc ^= fx_hash(black_box(s));
            }
        }
        black_box(acc);
        t.elapsed().as_nanos() as f64 / (reps as f64 * N as f64)
    };
    println!("HASH random-corpus sip={:.1}ns fx={:.1}ns speedup={:.2}x", t_sip, t_fx, t_sip / t_fx.max(0.01));

    // per-class hash speedup (is Fx only fast on 1-byte?)
    for (name, kind) in [("ascii", 0u8), ("len2", 1u8), ("len3", 2u8), ("len4", 3u8)] {
        let sub: Vec<&String> = corpus
            .iter()
            .filter(|s| match kind {
                0 => s.len() == 1,
                1 => s.as_bytes().len() == 2,
                2 => s.as_bytes().len() == 3,
                _ => s.as_bytes().len() == 4,
            })
            .take(2000)
            .collect();
        if sub.is_empty() {
            continue;
        }
        let r = 100u64;
        let a = {
            let t = Instant::now();
            let mut x = 0u64;
            for _ in 0..r {
                for s in &sub {
                    x ^= sip_hash(black_box(*s));
                }
            }
            black_box(x);
            t.elapsed().as_nanos() as f64 / (r as f64 * sub.len() as f64)
        };
        let b = {
            let t = Instant::now();
            let mut x = 0u64;
            for _ in 0..r {
                for s in &sub {
                    x ^= fx_hash(black_box(*s));
                }
            }
            black_box(x);
            t.elapsed().as_nanos() as f64 / (r as f64 * sub.len() as f64)
        };
        println!("HASH class {name} n={} sip={:.1}ns fx={:.1}ns speedup={:.2}x", sub.len(), a, b, a / b.max(0.01));
    }

    // 2) width: agreement on EVERY random symbol + fast-hit rate + timing
    let mut mism = 0u32;
    let mut fast_hits = 0usize;
    for s in &corpus {
        let b = s.as_bytes();
        let hit = (b.len() == 1 && b[0] >= 0x20)
            || (b.len() == 2 && matches!(b[0], 0xC3..=0xCB | 0xCE | 0xCF | 0xD0 | 0xD1 | 0xD3..=0xD5 | 0xDA))
            || (b.len() == 3 && b[0] == 0xe2
                && (matches!(b[1], 0x94 | 0x95 | 0xa0..=0xa3) || (b[1] == 0x96 && b[2] < 0xa0)))
            || b.len() == 3
            || (b.len() == 4 && b[0] == 0xF0 && matches!(b[1], 0x97 | 0xA0..=0xAE | 0xB0..=0xBE));
        if hit {
            fast_hits += 1;
        }
        if fast_width(s) != UnicodeWidthStr::width(s.as_str()) {
            mism += 1;
            if mism < 5 {
                println!("WIDTH MISMATCH {s:?} fast={} table={}", fast_width(s), UnicodeWidthStr::width(s.as_str()));
            }
        }
    }
    println!(
        "WIDTH agreement on {N} random symbols: {} mismatches, fast-path covers {fast_hits}/{N} ({:.1}%)",
        mism,
        100.0 * fast_hits as f64 / N as f64
    );
    assert_eq!(mism, 0, "fast path must agree with table on random assets");
    let wr = 50u64;
    let t_fast = {
        let t = Instant::now();
        let mut acc = 0usize;
        for _ in 0..wr {
            for s in &corpus {
                acc += fast_width(black_box(s));
            }
        }
        black_box(acc);
        t.elapsed().as_nanos() as f64 / (wr as f64 * N as f64)
    };
    let t_tab = {
        let t = Instant::now();
        let mut acc = 0usize;
        for _ in 0..wr {
            for s in &corpus {
                acc += UnicodeWidthStr::width(black_box(s.as_str()));
            }
        }
        black_box(acc);
        t.elapsed().as_nanos() as f64 / (wr as f64 * N as f64)
    };
    println!("WIDTH random-corpus fast={:.1}ns table={:.1}ns speedup={:.2}x", t_fast, t_tab, t_tab / t_fast.max(0.01));

    // ascii-only sub-bench (the fast path's home turf) vs CJK-only (wide path)
    let ascii_c: Vec<&String> = corpus.iter().filter(|s| s.len() == 1 && s.as_bytes()[0] >= 0x20).take(3000).collect();
    let cjk_c: Vec<&String> = corpus
        .iter()
        .filter(|s| {
            let b = s.as_bytes();
            b.len() == 3 && b[0] >= 0xE4 && b[0] <= 0xE9
        })
        .take(3000)
        .collect();
    for (name, sub) in [("ascii-only", ascii_c), ("cjk-E4-E9-only", cjk_c)] {
        if sub.is_empty() {
            continue;
        }
        let r = 100u64;
        let a = {
            let t = Instant::now();
            let mut x = 0usize;
            for _ in 0..r {
                for s in &sub {
                    x += fast_width(black_box(*s));
                }
            }
            black_box(x);
            t.elapsed().as_nanos() as f64 / (r as f64 * sub.len() as f64)
        };
        let b = {
            let t = Instant::now();
            let mut x = 0usize;
            for _ in 0..r {
                for s in &sub {
                    x += UnicodeWidthStr::width(black_box(s.as_str()));
                }
            }
            black_box(x);
            t.elapsed().as_nanos() as f64 / (r as f64 * sub.len() as f64)
        };
        println!("WIDTH {name} n={} fast={:.1}ns table={:.1}ns speedup={:.2}x", sub.len(), a, b, b / a.max(0.01));
    }

    // 3) layer sort fast path on RANDOM z-patterns (uses real Layer type shape: z ordering)
    // We bench the predicate + partition_point vs collect+sort on z-vectors.
    println!("SORT random-pattern check-vs-sort (ns, 100k iters):");
    let mut rng2 = Rng(0xDEAD_BEEF_CAFE_1234);
    for n in [2usize, 4, 8, 16, 64] {
        for (pname, mk) in [
            ("sorted", 0u8),
            ("reversed", 1u8),
            ("random", 2u8),
            ("one-swap", 3u8),
        ] {
            let base: Vec<i32> = (0..n as i32).collect();
            let zs: Vec<i32> = match mk {
                0 => base.clone(),
                1 => base.iter().rev().cloned().collect(),
                2 => {
                    let mut v = base.clone();
                    for i in (1..v.len()).rev() {
                        let j = (rng2.next() as usize) % (i + 1);
                        v.swap(i, j);
                    }
                    v
                }
                _ => {
                    let mut v = base.clone();
                    if n >= 2 {
                        let i = (rng2.next() as usize) % n;
                        let j = (rng2.next() as usize) % n;
                        v.swap(i, j);
                    }
                    v
                }
            };
            let it = 100_000u64;
            let chk = bench_ns(it, || {
                black_box(zs.windows(2).all(|w| w[0] <= w[1]));
            });
            let srt = bench_ns(it, || {
                let mut v: Vec<i32> = zs.iter().cloned().collect();
                v.sort();
                black_box(v);
            });
            // combined: check then sort-if-needed (what the renderer pays on unsorted)
            let both = bench_ns(it, || {
                if zs.windows(2).all(|w| w[0] <= w[1]) {
                    black_box(zs.partition_point(|x| *x < 0));
                } else {
                    let mut v: Vec<i32> = zs.iter().cloned().collect();
                    v.sort();
                    black_box(v);
                }
            });
            println!("  n={n:2} {pname:8} check={chk:.1}ns sort={srt:.1}ns check+branch={both:.1}ns overhead-vs-sort={:.2}x", both / srt.max(0.01));
        }
    }
    // touch the real Layer type so the bench stays tied to it
    let l = angel_glass::Layer {
        id: 1,
        z: -1,
        placement: angel_glass::Placement::Pixels([0.0, 0.0, 10.0, 10.0]),
        opacity: 1.0,
        kind: angel_glass::LayerKind::Solid([1.0, 1.0, 1.0, 1.0]),
        angle: 0.0,
        height: 0.0,
        breathe: 0.0,
        additive: false,
        flicker: 0.0,
        stand: angel_glass::Stand::default(),
    };
    black_box(l.z);

    // 4) bg-run key + LUT on random data
    {
        let n = 200_000u64;
        let t1 = bench_ns(n, || {
            let mut m: HashMap<String, u8> = HashMap::new();
            let k = format!("bg{}", black_box(0u8));
            m.insert(k, 1);
            black_box(m.len());
        });
        let t2 = bench_ns(n, || {
            let mut m: HashMap<(u8, u8, u8), u8> = HashMap::new();
            m.insert((12, 10, 8), 1);
            black_box(m.len());
        });
        println!("BGRUN stringkey={:.1}ns rawkey={:.1}ns speedup={:.2}x", t1, t2, t1 / t2.max(0.01));
        // random colours, not just (12,10,8)
        let mut rr = Rng(99);
        let cols: Vec<(u8, u8, u8)> = (0..4096).map(|_| (rr.below(256) as u8, rr.below(256) as u8, rr.below(256) as u8)).collect();
        let t3 = {
            let t = Instant::now();
            let mut m: HashMap<(u8, u8, u8), u8> = HashMap::new();
            for _ in 0..50 {
                for c in &cols {
                    m.insert(*c, 1);
                    black_box(m.len());
                }
            }
            t.elapsed().as_nanos() as f64 / (50.0 * cols.len() as f64)
        };
        println!("BGRUN random-4096-colours rawkey={:.1}ns/insert", t3);
    }
    {
        let n = 500_000u64;
        let lut = [9u8; 256];
        let mut rr = Rng(7);
        let idx: Vec<u8> = (0..1024).map(|_| rr.below(256) as u8).collect();
        let d = {
            let t = Instant::now();
            let mut k = 0usize;
            for _ in 0..n {
                let i = idx[k % idx.len()];
                black_box((i as u16 * 255 / 15) as u8);
                k += 1;
            }
            t.elapsed().as_nanos() as f64 / n as f64
        };
        let l = {
            let t = Instant::now();
            let mut k = 0usize;
            for _ in 0..n {
                black_box(lut[idx[k % idx.len()] as usize]);
                k += 1;
            }
            t.elapsed().as_nanos() as f64 / n as f64
        };
        println!("LUT random-idx div={:.1}ns lut={:.1}ns speedup={:.2}x", d, l, d / l.max(0.01));
    }
    println!("STRESS done: all speeds above are on RANDOM assets, seed fixed. WIDTH mismatches must be 0.");
}
