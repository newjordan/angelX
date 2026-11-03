//! Griffinwing's combat recorder (the private parts of combat.rs), for the
//! Bend port. Called from the main recorder's sampling.
use super::super::bend_kit::{self as kit, j, rec, want, want_with, Rng};
use super::*;
use serde_json::{json, Value};

fn pick<'a, T>(v: &'a [T], rng: &mut Rng) -> Option<&'a T> {
    if v.is_empty() {
        None
    } else {
        Some(&v[rng.below(v.len() as u64) as usize])
    }
}

fn friendly(rng: &mut Rng) -> Arm {
    [Arm::Lance, Arm::Star, Arm::Ally, Arm::Turned][rng.below(4) as usize]
}

/// A bolt of this kind with a velocity that carries it through `to`.
fn bolt_through(shot: Arm, to: (i32, i32, i32), rng: &mut Rng) -> (Bolt, (i32, i32, i32)) {
    let v = (rng.range(40, 600), rng.range(-90, 90), rng.range(-90, 90));
    let b = Bolt {
        shot,
        owner: rng.below(11) as u8,
        s: to.0,
        x: to.1,
        z: to.2,
        vs: v.0,
        vx: v.1,
        vz: v.2,
        dmg: [10u16, 16, 36, 60, 25][rng.below(5) as usize],
        life: 1 + rng.below(80) as u16,
        aim: 0,
        src: 0,
        delay: 0,
        full: rng.chance(2),
    };
    (b, (to.0 - v.0, to.1 - v.1, to.2 - v.2))
}

pub fn sample(s: &Session, n: &Nums, rng: &mut Rng) {
    let ids: Vec<u32> = s.seats.keys().copied().collect();
    // ── spawning and the small readers ─────────────────────────────────────
    if want("griffinwing.combat.Session.spawn_foe") {
        for k in 0..4 {
            let mut c = s.clone();
            if k == 3 {
                while c.foes.len() < MAX_FOES {
                    let id = c.next_id();
                    c.foes.push(Foe { id, kind: Kind::Wisp, s: 0, x: 0, z: 0, vs: 0, vx: 0, vz: 0, hp: 1, full: 1, seats: 1, st: St::Fly, ticks: 0, fx: 0, fz: 0, on: 0, flash: 0, mode: 0, phase: 0 });
                }
            }
            let kind = [Kind::Wisp, Kind::Kite, Kind::Turret, Kind::Bomber, Kind::Lamp][rng.below(5) as usize];
            let st = [St::Fly, St::Hide, St::Rest, St::Hunt, St::Shut][rng.below(5) as usize];
            let at = (rng.range(0, 90_000), rng.range(-900, 900), rng.range(-400, 400));
            let vel = (rng.range(-30, 30), rng.range(-60, 60), rng.range(-20, 20));
            let (mode, phase) = (rng.below(4) as u8, rng.below(30) as u8);
            let before = kit::sj(&c);
            let r = c.spawn_foe(kind, at, vel, st, mode, phase);
            rec(
                "griffinwing.combat.Session.spawn_foe",
                json!({"s": before, "kind": j(&kind), "at": kit::v3(at), "vel": kit::v3(vel), "st": j(&st), "mode": mode, "phase": phase}),
                json!({"s": kit::sj(&c), "r": r}),
            );
        }
    }
    if want("griffinwing.combat.Session.hurt_craft") {
        for k in 0..5 {
            let mut c = s.clone();
            if let Some(&id) = pick(&ids, rng) {
                let seat = c.seats.get_mut(&id).unwrap();
                match k {
                    0 => seat.craft.hurt = 0,
                    1 => seat.craft.hurt = 12,
                    2 => seat.craft.down = 30,
                    3 => {
                        seat.craft.hurt = 0;
                        seat.craft.shield = 40;
                    }
                    _ => seat.craft.hurt = 0,
                }
            }
            let id = if k == 4 { 7 + rng.below(5) as u8 } else { pick(&ids, rng).map_or(1, |&i| i as u8) };
            let dmg = [10, 15, 25, 35, 20, 60][rng.below(6) as usize];
            let before = kit::sj(&c);
            let r = c.hurt_craft(id, dmg);
            rec("griffinwing.combat.Session.hurt_craft", json!({"s": before, "id": id, "dmg": dmg}), json!({"s": kit::sj(&c), "r": r}));
        }
    }
    if want("griffinwing.combat.Session.drain") {
        for _ in 0..4 {
            let mut c = s.clone();
            let id = if rng.chance(2) { pick(&ids, rng).map_or(1, |&i| i as u8) } else { 7 + rng.below(4) as u8 };
            if let Some(seat) = c.seats.get_mut(&u32::from(id)) {
                if rng.chance(3) {
                    seat.craft.down = 20;
                }
            }
            let amount = [2, 3, 5][rng.below(3) as usize];
            let before = kit::sj(&c);
            c.drain(id, amount);
            rec("griffinwing.combat.Session.drain", json!({"s": before, "id": id, "amount": amount}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.combat.Session.latched") {
        let mut c = s.clone();
        for f in c.foes.iter_mut().take(3) {
            if rng.chance(2) {
                f.st = St::Latched;
                f.on = 1 + rng.below(10) as u8;
            }
        }
        for id in 0..=10u8 {
            rec("griffinwing.combat.Session.latched", json!({"s": kit::sj_keys(&c, &["foes"]), "id": id}), json!(c.latched(id)));
        }
    }
    if want("griffinwing.combat.Session.victim") {
        let mut c = s.clone();
        for f in c.foes.iter_mut().take(4) {
            if rng.chance(2) {
                f.st = St::Latched;
                f.on = 1 + rng.below(10) as u8;
            }
        }
        if rng.chance(3) {
            for w in c.wing.iter_mut() {
                w.lost = true;
            }
        }
        for hunter in [0u16, 1, 2, 3, 7, 40] {
            let r = c.victim(hunter);
            rec("griffinwing.combat.Session.victim", json!({"s": kit::sj_keys(&c, &["stage", "t", "seats", "wing", "foes"]), "hunter": hunter}), json!(r));
        }
    }
    if want("griffinwing.combat.Session.target") {
        let mut c = s.clone();
        if rng.chance(3) {
            for seat in c.seats.values_mut() {
                if rng.chance(2) {
                    seat.craft.down = 10;
                }
            }
        }
        for turn in [0usize, 1, 2, 3, 5, 9, 14] {
            let r = c.target(turn);
            rec("griffinwing.combat.Session.target", json!({"s": kit::sj_keys(&c, &["seats"]), "turn": turn}), json!(r.map(|(x, z)| [x, z])));
        }
    }
    if want("griffinwing.combat.Session.plane_shot") {
        for _ in 0..6 {
            let o = (s.anchor() + rng.range(-100, 4000), rng.range(-700, 700), rng.range(-300, 600));
            let to = (rng.range(-500, 500), rng.range(-300, 300));
            let closing = [dm(4), dm(5), 0, 1, dm(9)][rng.below(5) as usize];
            let v = s.plane_shot(o, to, closing);
            rec(
                "griffinwing.combat.Session.plane_shot",
                json!({"s": kit::sj_keys(s, &["stage", "t"]), "o": kit::v3(o), "to": [to.0, to.1], "closing": closing}),
                kit::v3(v),
            );
        }
    }
    if want("griffinwing.combat.Session.marks") {
        for seats in 0..=7u8 {
            let f = Foe { id: 3, kind: Kind::Turret, s: 0, x: 0, z: 0, vs: 0, vx: 0, vz: 0, hp: 1, full: 1, seats: 1, st: St::Tell, ticks: 0, fx: rng.range(-700, 700), fz: rng.range(-300, 300), on: 0, flash: 0, mode: 0, phase: 0 };
            rec("griffinwing.combat.Session.marks", json!({"f": j(&f), "seats": seats}), json!(Session::marks(&f, seats).into_iter().map(|(x, z)| [x, z]).collect::<Vec<_>>()));
        }
    }
    if let Some(f) = pick(&s.foes, rng) {
        if want("griffinwing.combat.Session.fire_marks") {
            let mut f = f.clone();
            f.fx = rng.range(-700, 700);
            f.fz = rng.range(-300, 300);
            let mut c = s.clone();
            let before = kit::sj(&c);
            c.fire_marks(&f);
            rec("griffinwing.combat.Session.fire_marks", json!({"s": before, "f": j(&f)}), json!({"s": kit::sj(&c)}));
        }
    }
    // ── the foes' own turns: each state a foe can be in ───────────────────
    steps(s, n, rng);
    // ── hits ───────────────────────────────────────────────────────────────
    if !s.foes.is_empty() {
        if want("griffinwing.combat.Session.hit_foe") {
            for _ in 0..4 {
                let mut c = s.clone();
                let fi = rng.below(c.foes.len() as u64) as usize;
                if rng.chance(3) {
                    c.foes[fi].hp = 1 + rng.below(30) as u32;
                }
                if rng.chance(4) {
                    c.foes[fi].st = St::Latched;
                    c.foes[fi].on = 1 + rng.below(8) as u8;
                }
                let dmg = [6u32, 10, 16, 36, 60, 120][rng.below(6) as usize];
                let by = rng.below(11) as u8;
                let before = kit::sj(&c);
                let r = c.hit_foe(fi, dmg, by);
                rec("griffinwing.combat.Session.hit_foe", json!({"s": before, "fi": fi, "dmg": dmg, "by": by}), json!({"s": kit::sj(&c), "r": r}));
            }
        }
        if want("griffinwing.combat.Session.kill") {
            for _ in 0..4 {
                let mut c = s.clone();
                let fi = rng.below(c.foes.len() as u64) as usize;
                if rng.chance(2) {
                    c.foes[fi].st = St::Latched;
                    c.foes[fi].on = 1 + rng.below(8) as u8;
                }
                let by = if rng.chance(4) { 7 + rng.below(4) as u8 } else { pick(&ids, rng).map_or(1, |&i| i as u8) };
                if let Some(seat) = c.seats.get_mut(&u32::from(by)) {
                    seat.craft.chain = [0u16, 1, 2, 3, 4, 7, 40][rng.below(7) as usize];
                }
                let before = kit::sj(&c);
                c.kill(fi, by);
                rec("griffinwing.combat.Session.kill", json!({"s": before, "fi": fi, "by": by}), json!({"s": kit::sj(&c)}));
            }
        }
        if want("griffinwing.combat.Session.starfall") {
            let mut c = s.clone();
            let by = pick(&ids, rng).map_or(1, |&i| i as u8);
            let s_c = c.craft_s(&c.seats.values().next().map(|x| x.craft.clone()).unwrap_or_else(|| Craft::new(0, 0)));
            let before = kit::sj(&c);
            c.starfall(by, s_c, 0, 0);
            rec("griffinwing.combat.Session.starfall", json!({"s": before, "by": by, "s_c": s_c, "x": 0, "z": 0}), json!({"s": kit::sj(&c)}));
        }
    }
    if s.foes.iter().any(|f| f.kind == Kind::Lamp) && want("griffinwing.combat.Session.break_lamp") {
        let mut c = s.clone();
        let fi = c.foes.iter().position(|f| f.kind == Kind::Lamp).unwrap();
        let by = pick(&ids, rng).map_or(1, |&i| i as u8);
        let before = kit::sj(&c);
        c.break_lamp(fi, by);
        rec("griffinwing.combat.Session.break_lamp", json!({"s": before, "fi": fi, "by": by}), json!({"s": kit::sj(&c)}));
    }
    // ── bolts against foes and craft ──────────────────────────────────────
    bolts(s, n, rng);
    // ── dive-bombers meet craft ───────────────────────────────────────────
    if want("griffinwing.combat.Session.bombers_ram") {
        let mut c = s.clone();
        for &id in ids.iter().take(2) {
            let cr = c.seats[&id].craft.clone();
            let s_c = c.craft_s(&cr);
            let at = (s_c + rng.range(-40, 120), cr.x + rng.range(-90, 90), cr.z + rng.range(-90, 90));
            let vel = (-rng.range(20, 80), rng.range(-30, 30), rng.range(-30, 30));
            c.spawn_foe(Kind::Bomber, at, vel, St::Dive, 0, 0);
        }
        if rng.chance(2) {
            if let Some(seat) = c.seats.values_mut().next() {
                seat.craft.hurt = if rng.chance(2) { 0 } else { 20 };
            }
        }
        let before = kit::sj(&c);
        c.bombers_ram();
        rec("griffinwing.combat.Session.bombers_ram", json!({"s": before}), json!({"s": kit::sj(&c)}));
    }
}

/// Each foe in the session is stepped as `step_foes` does it (the foes taken
/// out), and the kites in their hunting and hanging states are stepped by
/// `hunt` and `hang` directly.
fn steps(s: &Session, n: &Nums, rng: &mut Rng) {
    if s.foes.is_empty() {
        return;
    }
    let anchor = s.anchor();
    let interesting = s.foes.iter().any(|f| matches!(f.st, St::Tell | St::Hunt | St::Latched | St::Dive | St::Hide | St::Rest | St::Flee));
    if (interesting && want_with("griffinwing.combat.Session.step_foe", 5, 30)) || want("griffinwing.combat.Session.step_foe") {
        let mut c = s.clone();
        let foes = std::mem::take(&mut c.foes);
        let mut c2 = c.clone();
        for mut f in foes {
            // a foe about to act: nearly out of ticks sometimes
            if rng.chance(4) {
                f.ticks = f.ticks.min(1 + rng.below(2) as u32);
            }
            if rng.chance(5) {
                f.flash = rng.below(6) as u8;
            }
            let before = kit::sj(&c2);
            let fb = f.clone();
            c2.step_foe(&mut f, anchor, n);
            if kit::cases::count("griffinwing.combat.Session.step_foe") < kit::CAP {
                rec(
                    "griffinwing.combat.Session.step_foe",
                    json!({"s": before, "f": j(&fb), "anchor": anchor, "n": kit::nums_j(n)}),
                    json!({"s": kit::sj(&c2), "f": j(&f)}),
                );
            }
        }
    }
    if let Some(f) = s.foes.iter().find(|f| f.st == St::Hunt) {
        if want_with("griffinwing.combat.Session.hunt", 5, 20) {
            let mut c = s.clone();
            c.foes.clear();
            let mut f = f.clone();
            let before = kit::sj(&c);
            let fb = f.clone();
            c.hunt(&mut f);
            rec("griffinwing.combat.Session.hunt", json!({"s": before, "f": j(&fb)}), json!({"s": kit::sj(&c), "f": j(&f)}));
        }
    }
    if let Some(f) = s.foes.iter().find(|f| f.st == St::Latched) {
        if want_with("griffinwing.combat.Session.hang", 5, 20) {
            let mut c = s.clone();
            c.foes.clear();
            let mut f = f.clone();
            if rng.chance(3) {
                f.ticks = 11;
            }
            let before = kit::sj(&c);
            let fb = f.clone();
            c.hang(&mut f);
            rec("griffinwing.combat.Session.hang", json!({"s": before, "f": j(&fb)}), json!({"s": kit::sj(&c), "f": j(&f)}));
        }
    }
    // a hunter placed near a craft or a rider, and a latched kite on one, to be
    // sure of the states
    if want_with("griffinwing.combat.Session.hunt", 4, 40) {
        let mut c = s.clone();
        c.foes.clear();
        let who = [1u8, 7, 8, 0, 3][rng.below(5) as usize];
        let at = c.craft_at(who).map_or((c.anchor() + 3000, 0, 0), |(a, b, d, _)| (a - dm(18) + rng.range(-200, 200), b + rng.range(-100, 100), d + dm(4) + rng.range(-100, 100)));
        let id = 5 + rng.below(30) as u16;
        let mut f = Foe { id, kind: Kind::Kite, s: at.0, x: at.1, z: at.2, vs: 0, vx: 0, vz: 0, hp: 20, full: 20, seats: 1, st: St::Hunt, ticks: 0, fx: 0, fz: 0, on: who, flash: 0, mode: 1, phase: 0 };
        if rng.chance(3) {
            f.on = 0;
        }
        let before = kit::sj(&c);
        let fb = f.clone();
        c.hunt(&mut f);
        rec("griffinwing.combat.Session.hunt", json!({"s": before, "f": j(&fb)}), json!({"s": kit::sj(&c), "f": j(&f)}));
        let mut c = s.clone();
        c.foes.clear();
        let who = [1u8, 2, 7, 8, 0, 3][rng.below(6) as usize];
        let mut f = Foe { id, kind: Kind::Kite, s: 0, x: 0, z: 0, vs: 0, vx: 0, vz: 0, hp: 20, full: 20, seats: 1, st: St::Latched, ticks: rng.below(30) as u32, fx: 0, fz: 0, on: who, flash: 0, mode: 1, phase: 0 };
        if let Some(seat) = c.seats.get_mut(&u32::from(who)) {
            seat.craft.roll = if rng.chance(3) { 9 } else { 0 };
        }
        let before = kit::sj(&c);
        let fb = f.clone();
        c.hang(&mut f);
        rec("griffinwing.combat.Session.hang", json!({"s": before, "f": j(&fb)}), json!({"s": kit::sj(&c), "f": j(&f)}));
    }
}

/// Friendly and hostile bolts: the ones in the session, and ones sent through
/// the bodies they may hit.
fn bolts(s: &Session, n: &Nums, rng: &mut Rng) {
    let mut dead0 = vec![false; s.bolts.len()];
    if want("griffinwing.combat.Session.friendly_bolt") {
        for _ in 0..4 {
            let mut c = s.clone();
            // aim through a foe, or through a spark in the air
            let (shot, to) = match pick(&c.foes, rng) {
                Some(f) if rng.chance(3) => (friendly(rng), (f.s + rng.range(-60, 60), f.x + rng.range(-100, 100), f.z + rng.range(-100, 100))),
                _ => match c.bolts.iter().find(|b| b.shot == Arm::Spark) {
                    Some(b) if rng.chance(2) => (Arm::Lance, (b.s + rng.range(-10, 10), b.x + rng.range(-40, 40), b.z + rng.range(-40, 40))),
                    _ => (friendly(rng), (c.anchor() + rng.range(0, 5000), rng.range(-600, 600), rng.range(-300, 300))),
                },
            };
            let (mut b, from) = bolt_through(shot, to, rng);
            if shot == Arm::Star {
                b.aim = pick(&c.foes, rng).map_or(0, |f| f.id);
            }
            dead0.iter_mut().for_each(|d| *d = rng.chance(8));
            let mut dead = dead0.clone();
            let before = kit::sj(&c);
            let r = c.friendly_bolt(&b, from, to, &mut dead);
            rec(
                "griffinwing.combat.Session.friendly_bolt",
                json!({"s": before, "b": j(&b), "from": kit::v3(from), "to": kit::v3(to), "dead": dead0}),
                json!({"s": kit::sj(&c), "r": r, "dead": dead}),
            );
        }
    }
    if !s.foes.is_empty() && want("griffinwing.combat.Session.land") {
        for _ in 0..4 {
            let mut c = s.clone();
            let i = match c.foes.iter().position(|f| f.kind == Kind::Lamp) {
                Some(l) if rng.chance(2) => l,
                _ => rng.below(c.foes.len() as u64) as usize,
            };
            if let Some(f) = c.foes.get_mut(i) {
                if f.kind == Kind::Lamp {
                    f.st = [St::Shut, St::Open, St::Shut, St::Broken][rng.below(4) as usize];
                }
            }
            let (b, _) = bolt_through(friendly(rng), (0, 0, 0), rng);
            let before = kit::sj(&c);
            c.land(i, &b);
            rec("griffinwing.combat.Session.land", json!({"s": before, "i": i, "b": j(&b)}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.combat.Session.hostile_bolt") {
        for _ in 0..4 {
            let mut c = s.clone();
            let shot = if rng.chance(2) { Arm::Gloam } else { Arm::Spark };
            // aim through a craft: sometimes rolling, sometimes just hurt
            let ids: Vec<u32> = c.seats.keys().copied().collect();
            let to = match pick(&ids, rng) {
                Some(&id) => {
                    let seat = c.seats.get_mut(&id).unwrap();
                    match rng.below(4) {
                        0 => {
                            seat.craft.roll = 1 + rng.below(25) as u32;
                            seat.craft.roll_len = 30;
                        }
                        1 => seat.craft.hurt = 5,
                        2 => seat.craft.hurt = 0,
                        _ => {}
                    }
                    let cr = seat.craft.clone();
                    (c.craft_s(&cr) + rng.range(-30, 30), cr.x + rng.range(-120, 120), cr.z + rng.range(-120, 120))
                }
                None => (c.anchor() + 500, 0, 0),
            };
            let (mut b, from) = bolt_through(shot, to, rng);
            b.owner = 0;
            b.src = pick(&c.foes, rng).map_or(0, |f| f.id);
            let bj0 = j(&b);
            let before = kit::sj(&c);
            let r = c.hostile_bolt(&mut b, from, to, n);
            rec(
                "griffinwing.combat.Session.hostile_bolt",
                json!({"s": before, "b": bj0, "from": kit::v3(from), "to": kit::v3(to), "n": kit::nums_j(n)}),
                json!({"s": kit::sj(&c), "b": j(&b), "r": r}),
            );
        }
    }
    if want("griffinwing.combat.Session.turn") {
        for _ in 0..4 {
            let mut c = s.clone();
            let at = (c.anchor() + rng.range(-100, 600), rng.range(-500, 500), rng.range(-300, 300));
            let (mut b, _) = bolt_through(if rng.chance(2) { Arm::Gloam } else { Arm::Spark }, at, rng);
            b.owner = 0;
            b.src = pick(&c.foes, rng).map_or(rng.below(5) as u16, |f| f.id);
            let by = rng.below(7) as u32;
            let bj0 = j(&b);
            let before = kit::sj(&c);
            c.turn(&mut b, by, at);
            rec(
                "griffinwing.combat.Session.turn",
                json!({"s": before, "b": bj0, "by": by, "at": kit::v3(at)}),
                json!({"s": kit::sj(&c), "b": j(&b)}),
            );
        }
    }
    let _ = Value::Null;
}
