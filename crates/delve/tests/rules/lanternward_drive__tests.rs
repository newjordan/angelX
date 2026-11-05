//! Shared driver for the Lanternward case recorders (Bend port, games_a):
//! bots play a table from the door, a few with chaos (random hands, hurt
//! knights, bombs, a seat that stands up and sits down again), and every
//! sampled tick is handed to the recorder.
#![allow(dead_code)]
use crate::rules::lanternward::*;
use crate::rules::{Hero, Input, Run};
use serde::Serialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeMap;

pub struct Lcg(pub u64);
impl Lcg {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

pub fn jv<T: Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}
pub fn rj(r: &Rules) -> Value {
    json!({"lock": r.lock, "parry": r.parry, "combo": r.combo, "flame": r.flame, "tell": r.tell})
}
pub fn evj(ev: &Events) -> Value {
    json!({"sounds": ev.sounds, "stop": ev.stop.map(|s| format!("{s:?}")), "won": ev.won})
}
/// The two hero flags the rules read.
pub fn hj(heroes: &BTreeMap<u32, Hero>) -> Value {
    let mut m = serde_json::Map::new();
    for (id, h) in heroes {
        m.insert(id.to_string(), json!({"away": h.away, "stone": h.stone}));
    }
    Value::Object(m)
}

thread_local! {
    static COUNTS: RefCell<BTreeMap<String, usize>> = const { RefCell::new(BTreeMap::new()) };
}
/// True on every `k`th call for `name`: a stride through the qualifying frames.
pub fn take(name: &str, k: usize) -> bool {
    COUNTS.with(|c| {
        let mut c = c.borrow_mut();
        let n = c.entry(name.to_string()).or_insert(0);
        *n += 1;
        (*n - 1) % k.max(1) == 0
    })
}

pub struct Frame<'a> {
    pub s: &'a Session,
    pub heroes: &'a BTreeMap<u32, Hero>,
    pub inputs: &'a BTreeMap<u32, Input>,
    pub r: &'a Rules,
    pub tick: u32,
    pub seats: u32,
    pub seed: u64,
}

fn scramble(i: &mut Input, g: &mut Lcg) {
    let axis = |g: &mut Lcg| (g.below(3) as i8) - 1;
    match g.below(8) {
        0 => i.move_x = axis(g),
        1 => i.move_y = axis(g),
        2 => i.aim_x = axis(g),
        3 => i.aim_y = axis(g),
        4 => i.dash = !i.dash,
        5 => i.swing = !i.swing,
        6 => i.interact = !i.interact,
        _ => {
            i.fire = !i.fire;
            i.ult = !i.ult;
        }
    }
}

/// Drive `seats` bots for `limit` ticks, handing every `every`th tick (before
/// the step) to `f`. Odd seeds add chaos.
pub fn drive(seats: u32, seed: u64, limit: u32, every: u32, mut f: impl FnMut(&Frame)) {
    let mut run = Run::lanternward_for_test();
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    let mut heroes = run.players.clone();
    let mut s = Session::new(&heroes);
    let r = Rules::default();
    let mut g = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut gone: Option<(u32, u32)> = None;
    let mut done_at = None;
    for tick in 0..limit {
        let mut inputs: BTreeMap<u32, Input> = BTreeMap::new();
        for &id in s.seats.keys() {
            inputs.insert(id, s.bot_input(id, &heroes[&id], &r));
        }
        if seed % 2 == 1 {
            for i in inputs.values_mut() {
                if g.below(9) == 0 {
                    scramble(i, &mut g);
                }
            }
        }
        if tick % every == 0 {
            f(&Frame {
                s: &s,
                heroes: &heroes,
                inputs: &inputs,
                r: &r,
                tick,
                seats,
                seed,
            });
        }
        s.step(&inputs, &heroes, &r);
        if seed % 2 == 1 && tick % 97 == 50 && !s.seats.is_empty() {
            let ids: Vec<u32> = s.seats.keys().copied().collect();
            let id = ids[g.below(ids.len() as u64) as usize];
            match g.below(4) {
                0 => {
                    let k = s.seats.get_mut(&id).unwrap();
                    k.hp = (k.hp / 3).max(1);
                }
                1 => s.seats.get_mut(&id).unwrap().bombs = 3,
                2 => {
                    if gone.is_none() && ids.len() > 1 {
                        heroes.get_mut(&id).unwrap().away = true;
                        gone = Some((id, tick + 25));
                    }
                }
                _ => s.seats.get_mut(&id).unwrap().gems += 7,
            }
        }
        if let Some((id, back)) = gone
            && tick >= back
        {
            heroes.get_mut(&id).unwrap().away = false;
            gone = None;
        }
        if s.done.is_some() && done_at.is_none() {
            done_at = Some(tick);
        }
        if done_at.is_some_and(|t| tick > t + 200) {
            break;
        }
    }
}

/// The usual set of tables: (seats, seed, limit).
pub const TABLES: [(u32, u64, u32); 8] = [
    (1, 2, 9000),
    (2, 4, 9000),
    (4, 6, 9000),
    (6, 8, 9500),
    (1, 3, 6000),
    (3, 5, 9000),
    (5, 7, 9000),
    (2, 9, 9000),
];
