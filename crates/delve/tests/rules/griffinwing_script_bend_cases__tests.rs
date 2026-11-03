//! Griffinwing's script recorder (the private parts of script.rs), for the
//! Bend port. Called from the main recorder's sampling.
use super::super::bend_kit::{self as kit, j, rec, want, Rng};
use super::*;
use serde_json::{json, Value};

pub fn act_j(a: &Act) -> Value {
    match *a {
        Act::Wisps { n, plus, shape, x, z, mode } => json!({"k": "Wisps", "n": n, "plus": plus, "shape": shape, "x": x, "z": z, "mode": mode}),
        Act::Convoy { n, plus, from, z } => json!({"k": "Convoy", "n": n, "plus": plus, "from": from, "z": z}),
        Act::Hunter { side } => json!({"k": "Hunter", "side": side}),
        Act::Turret { x, z } => json!({"k": "Turret", "x": x, "z": z}),
        Act::Ridge { x, bombers, plus } => json!({"k": "Ridge", "x": x, "bombers": bombers, "plus": plus}),
        Act::Gate { rel, x, z, bonus } => json!({"k": "Gate", "rel": rel, "x": x, "z": z, "bonus": bonus}),
        Act::Ring { rel, x, z } => json!({"k": "Ring", "rel": rel, "x": x, "z": z}),
        Act::Arch { rel, x, z, w, h } => json!({"k": "Arch", "rel": rel, "x": x, "z": z, "w": w, "h": h}),
        Act::Shutter { rel, phase } => json!({"k": "Shutter", "rel": rel, "phase": phase}),
        Act::Say(line) => json!({"k": "Say", "line": j(&line)}),
        Act::Beast => json!({"k": "Beast"}),
    }
}

fn every_act() -> Vec<Act> {
    (0..3u8).flat_map(|st| script(st).iter().map(|e| e.act).collect::<Vec<_>>()).collect()
}

/// The tables themselves, once.
pub fn tables() {
    for stage in 0..=3u8 {
        let evs: Vec<Value> = script(stage).iter().map(|e| json!({"at": e.at, "act": act_j(&e.act)})).collect();
        rec("griffinwing.script.script", json!(stage), json!(evs));
    }
    for stage in 0..3u8 {
        for cp in [0u32, 1, 30, 31, 100, 109, 110, 111, 330, 331, 480, 481, 700, 1010, 1011, 1500, 1700, 2000, 2300, 5000] {
            rec("griffinwing.script.resume", json!({"stage": stage, "cp": cp}), json!(resume(stage, cp)));
        }
    }
    for k in 0..10 {
        for count in 1..=8 {
            if k < count {
                rec("griffinwing.script.Session.spread", json!({"k": k, "count": count}), json!(Session::spread(k, count)));
            }
        }
    }
}

pub fn sample(s: &Session, _n: &Nums, rng: &mut Rng) {
    if want("griffinwing.script.Session.act") {
        let acts = every_act();
        for _ in 0..6 {
            let mut c = s.clone();
            if rng.chance(3) {
                while c.props.len() < MAX_PROPS - 1 {
                    c.props.push(Prop { thing: Thing::Ring, s: 0, x: 0, z: 0, w: 0, h: 0, len: 0, taken: 0, phase: 0 });
                }
            }
            c.stage = rng.below(3) as u8;
            c.next = rng.below(60) as u16;
            let a = if rng.chance(3) {
                // an act of a made-up shape
                match rng.below(5) {
                    0 => Act::Wisps { n: 1 + rng.below(6) as u8, plus: rng.below(3) as u8, shape: rng.below(5) as u8, x: rng.range(-30, 30) as i16, z: rng.range(0, 40) as i16, mode: [0u8, 1, 3][rng.below(3) as usize] },
                    1 => Act::Convoy { n: 1 + rng.below(6) as u8, plus: rng.below(3) as u8, from: [-1i8, 1][rng.below(2) as usize], z: rng.range(0, 40) as i16 },
                    2 => Act::Ridge { x: rng.range(-90, 90) as i16, bombers: rng.below(4) as u8, plus: rng.below(3) as u8 },
                    3 => Act::Gate { rel: rng.range(10_000, 40_000), x: rng.range(-30, 30) as i16, z: rng.range(0, 40) as i16, bonus: rng.chance(2) },
                    _ => Act::Hunter { side: [-1i8, 1][rng.below(2) as usize] },
                }
            } else {
                acts[rng.below(acts.len() as u64) as usize]
            };
            let before = kit::sj(&c);
            c.act(a);
            rec("griffinwing.script.Session.act", json!({"s": before, "act": act_j(&a)}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.script.Session.victory_gates") {
        let mut c = s.clone();
        if rng.chance(3) {
            while c.props.len() < MAX_PROPS - 3 {
                c.props.push(Prop { thing: Thing::Gate, s: 0, x: 0, z: 0, w: 0, h: 0, len: 0, taken: 0, phase: 0 });
            }
        }
        let before = kit::sj(&c);
        c.victory_gates();
        rec("griffinwing.script.Session.victory_gates", json!({"s": before}), json!({"s": kit::sj(&c)}));
    }
}
