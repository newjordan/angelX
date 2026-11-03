//! Griffinwing's props recorder (the private take and scrape of things.rs),
//! for the Bend port. Called from the main recorder's sampling.
use super::super::bend_kit::{self as kit, j, rec, want, Rng};
use super::*;
use serde_json::json;

fn prop(rng: &mut Rng, s_at: i32, x: i32, z: i32) -> Prop {
    let thing = [Thing::Gate, Thing::Ring, Thing::Arch, Thing::Ridge, Thing::Shutter][rng.below(5) as usize];
    Prop {
        thing,
        s: s_at,
        x,
        z,
        w: [dm(20), dm(22), dm(24), dm(30), 0][rng.below(5) as usize],
        h: [0, dm(15), dm(55)][rng.below(3) as usize],
        len: [0, dm(30), dm(60)][rng.below(3) as usize],
        taken: 0,
        phase: if rng.chance(3) { 1 } else { rng.below(150) as u16 },
    }
}

pub fn sample(s: &Session, n: &Nums, rng: &mut Rng) {
    let ids: Vec<u32> = s.seats.keys().copied().collect();
    if ids.is_empty() {
        return;
    }
    if want("griffinwing.things.Session.take") {
        for _ in 0..4 {
            let mut c = s.clone();
            let id = ids[rng.below(ids.len() as u64) as usize];
            let x = rng.range(-300, 300);
            let z = rng.range(-200, 200);
            let mut p = prop(rng, c.anchor() + 300, x, z);
            p.thing = if rng.chance(2) { Thing::Gate } else { Thing::Ring };
            let boosting = rng.chance(2);
            if let Some(seat) = c.seats.get_mut(&id) {
                seat.craft.shield = [100, 80, 40, 60][rng.below(4) as usize];
            }
            let before = kit::sj(&c);
            c.take(id, &p, boosting);
            rec("griffinwing.things.Session.take", json!({"s": before, "id": id, "p": j(&p), "boosting": boosting}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.things.Session.scrape") {
        for _ in 0..3 {
            let mut c = s.clone();
            let id = ids[rng.below(ids.len() as u64) as usize];
            if let Some(seat) = c.seats.get_mut(&id) {
                seat.craft.hurt = if rng.chance(2) { 0 } else { 9 };
                seat.craft.down = if rng.chance(6) { 9 } else { 0 };
            }
            let (at, x, z) = (c.anchor() + rng.range(-100, 200), rng.range(-300, 300), rng.range(-200, 200));
            let before = kit::sj(&c);
            c.scrape(id, at, x, z);
            rec("griffinwing.things.Session.scrape", json!({"s": before, "id": id, "at": at, "x": x, "z": z}), json!({"s": kit::sj(&c)}));
        }
    }
    // step_props on a prop set across the formation's path
    if want("griffinwing.things.Session.step_props") {
        for _ in 0..3 {
            let mut c = s.clone();
            c.props.clear();
            for &id in ids.iter().take(3) {
                let cr = c.seats[&id].craft.clone();
                let s_c = c.craft_s(&cr);
                let at = s_c - rng.range(0, 90) + rng.range(-20, 20);
            let x = cr.x + rng.range(-120, 120);
            let z = cr.z + rng.range(-120, 120);
            let p = prop(rng, at, x, z);
                c.props.push(p);
            }
            if rng.chance(2) {
                c.props.push(Prop { thing: Thing::Ring, s: c.anchor() - dm(400), x: 0, z: 0, w: dm(22), h: 0, len: 0, taken: 0, phase: 0 });
            }
            for seat in c.seats.values_mut() {
                seat.craft.hurt = if rng.chance(2) { 0 } else { 5 };
                seat.craft.boost = rng.chance(3);
                seat.craft.ds = if rng.chance(2) { PACE } else { PACE + 60 };
            }
            let before = kit::sj(&c);
            c.step_props(n);
            rec("griffinwing.things.Session.step_props", json!({"s": before, "n": kit::nums_j(n)}), json!({"s": kit::sj(&c)}));
        }
    }
}
