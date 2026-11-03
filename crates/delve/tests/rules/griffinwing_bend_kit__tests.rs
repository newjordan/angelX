//! Griffinwing's Bend-port recorder kit (feature `bend-cases-games-a-griffinwing`):
//! what every recorder file of the game shares. A case is `{fn, in, out}` in
//! `$DELVE_CASES/<fn>.jsonl`; `rec` spreads a function's cases over the
//! recorded runs and over each run's time.
use super::*;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap as Map;

#[path = "games_a_bend_cases__tests.rs"]
pub mod cases;

/// Cases kept of each function, and of each run, and the ticks between two of a
/// function's cases in one run.
pub const CAP: usize = 64;

#[derive(Default)]
struct Gate {
    run: usize,
    n: usize,
    last: Option<u32>,
}

thread_local! {
    static GATES: RefCell<Map<String, Gate>> = RefCell::new(Map::new());
    static NOW: RefCell<(usize, u32)> = const { RefCell::new((0, 0)) };
}

/// The recorder's clock: which run and which tick it is on.
pub fn at(run: usize, tick: u32) {
    NOW.with(|n| *n.borrow_mut() = (run, tick));
}

pub fn clock() -> (usize, u32) {
    NOW.with(|n| *n.borrow())
}

/// Whether to record a case of `name` now: a steady scatter over the runs and
/// over each run's time (about four cases a run).
pub fn want(name: &str) -> bool {
    if cases::count(name) >= CAP {
        return false;
    }
    let (run, tick) = clock();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h ^= (run as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= u64::from(tick).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 31;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 29;
    h % RATE == 0
}

/// Ticks between two scattered cases, on average.
pub const RATE: u64 = 1500;

/// Whether to record a case of an event (a state the caller has found): at most
/// `per_run` a run, `gap` ticks apart.
pub fn want_with(name: &str, per_run: usize, gap: u32) -> bool {
    if cases::count(name) >= CAP {
        return false;
    }
    let (run, tick) = clock();
    GATES.with(|g| {
        let mut g = g.borrow_mut();
        let e = g.entry(name.to_string()).or_default();
        if e.run != run {
            e.run = run;
            e.n = 0;
            e.last = None;
        }
        if e.n >= per_run || e.last.is_some_and(|l| tick < l + gap) {
            return false;
        }
        e.n += 1;
        e.last = Some(tick);
        true
    })
}

/// Append a case.
pub fn rec(name: &str, input: Value, out: Value) {
    cases::rec_cap(name, &input, &out, usize::MAX);
}

/// A session as the Bend port reads it: serde's shape with the sound and
/// leave queues the checkpoint carries.
pub fn sj(s: &Session) -> Value {
    let mut v = serde_json::to_value(s).unwrap();
    let o = v.as_object_mut().unwrap();
    o.insert("snd".into(), json!(s.snd));
    o.insert("leave".into(), json!(s.leave));
    v
}

/// Only some fields of a session (for functions that read just those).
pub fn sj_keys(s: &Session, keys: &[&str]) -> Value {
    let v = sj(s);
    let mut o = serde_json::Map::new();
    for k in keys {
        if let Some(x) = v.get(*k) {
            o.insert((*k).to_string(), x.clone());
        }
    }
    Value::Object(o)
}

pub fn j<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}

pub fn nums_j(n: &Nums) -> Value {
    json!({"charge": n.charge, "roll": n.roll, "window": n.window, "spur": n.spur, "tell": n.tell})
}

/// The one thing about a hero the game reads: whether it sits.
pub fn heroes_j(h: &BTreeMap<u32, Hero>) -> Value {
    let mut o = serde_json::Map::new();
    for (id, hero) in h {
        o.insert(id.to_string(), json!({"away": hero.away, "stone": hero.stone}));
    }
    Value::Object(o)
}

pub fn inputs_j(i: &BTreeMap<u32, Input>) -> Value {
    let mut o = serde_json::Map::new();
    for (id, x) in i {
        o.insert(id.to_string(), j(x));
    }
    Value::Object(o)
}

pub fn v3(t: (i32, i32, i32)) -> Value {
    json!([t.0, t.1, t.2])
}

/// A steady generator (xorshift64*), seeded from the run and the tick.
pub struct Rng(pub u64);

impl Rng {
    pub fn new(a: u64, b: u64) -> Rng {
        Rng(0x9E37_79B9_7F4A_7C15 ^ a.wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ b.wrapping_mul(0x94D0_49BB_1331_11EB) | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// `0..n`.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            (self.next() >> 11) % n
        }
    }
    /// `lo..=hi`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1) as u64) as i32
    }
    pub fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}
