//! Griffinwing's flight recorder (the private tap, acquire, release and
//! push_bolt), for the Bend port. Called from the main recorder's sampling.
use super::super::bend_kit::{self as kit, Rng, j, rec, want, want_with};
use super::*;
use serde_json::json;

fn some_craft(s: &Session, rng: &mut Rng) -> Option<(u32, Craft)> {
    let ids: Vec<u32> = s.seats.keys().copied().collect();
    if ids.is_empty() {
        return None;
    }
    let id = ids[rng.below(ids.len() as u64) as usize];
    Some((id, s.seats[&id].craft.clone()))
}

/// A reticle that points at foe `f` from craft `c`.
fn aim_at(c: &mut Craft, s_c: i32, f: &Foe, jitter: i32) {
    let ds = (f.s - s_c).max(1);
    c.rx = ((i64::from(f.x - c.x + jitter) * i64::from(FAR) / i64::from(ds)) as i32)
        .clamp(-RX_MAX, RX_MAX);
    c.rz = ((i64::from(f.z - c.z) * i64::from(FAR) / i64::from(ds)) as i32).clamp(-RZ_MAX, RZ_MAX);
}

pub fn sample(s: &Session, n: &Nums, rng: &mut Rng) {
    if want("griffinwing.flight.Session.push_bolt") {
        for k in 0..3 {
            let mut c = s.clone();
            if k == 2 {
                while c.bolts.len() < MAX_BOLTS {
                    c.bolts.push(Bolt {
                        shot: Arm::Lance,
                        owner: 1,
                        s: 0,
                        x: 0,
                        z: 0,
                        vs: 0,
                        vx: 0,
                        vz: 0,
                        dmg: 10,
                        life: 5,
                        aim: 0,
                        src: 0,
                        delay: 0,
                        full: false,
                    });
                }
            }
            let b = Bolt {
                shot: [
                    Arm::Lance,
                    Arm::Star,
                    Arm::Ally,
                    Arm::Spark,
                    Arm::Gloam,
                    Arm::Turned,
                ][rng.below(6) as usize],
                owner: rng.below(11) as u8,
                s: rng.range(-500, 90_000),
                x: rng.range(-900, 900),
                z: rng.range(-900, 900),
                vs: rng.range(-300, 600),
                vx: rng.range(-90, 90),
                vz: rng.range(-90, 90),
                dmg: rng.below(60) as u16,
                life: rng.below(300) as u16,
                aim: rng.below(30) as u16,
                src: rng.below(30) as u16,
                delay: rng.below(12) as u8,
                full: rng.chance(2),
            };
            let before = kit::sj(&c);
            c.push_bolt(b.clone());
            rec(
                "griffinwing.flight.Session.push_bolt",
                json!({"s": before, "b": j(&b)}),
                json!({"s": kit::sj(&c)}),
            );
        }
    }
    let Some((id, craft)) = some_craft(s, rng) else {
        return;
    };
    let s_c = s.craft_s(&craft);
    if want("griffinwing.flight.Session.tap") {
        for _ in 0..2 {
            let mut c = craft.clone();
            c.rx = rng.range(-RX_MAX, RX_MAX);
            c.rz = rng.range(-RZ_MAX, RZ_MAX);
            c.x = rng.range(-400, 400);
            c.z = rng.range(-300, 300);
            let mut t = s.clone();
            let before = kit::sj(&t);
            t.tap(id as u8, &c, s_c);
            rec(
                "griffinwing.flight.Session.tap",
                json!({"s": before, "owner": id, "c": j(&c), "s_c": s_c}),
                json!({"s": kit::sj(&t)}),
            );
        }
    }
    let live = !s.foes.is_empty();
    if live && want("griffinwing.flight.Session.acquire") {
        for k in 0..4 {
            let mut c = craft.clone();
            let f = &s.foes[rng.below(s.foes.len() as u64) as usize];
            aim_at(&mut c, s_c, f, rng.range(-60, 60));
            c.held = 9 + rng.below(20) as u32;
            c.charge = rng.below(40) as u32;
            c.dwell = rng.below(10) as u32;
            if k % 2 == 1 {
                // some locks held already (some stale, some good)
                c.locks.clear();
                for g in s.foes.iter().take(rng.below(4) as usize) {
                    c.locks.push(Lock {
                        foe: g.id,
                        n: 1 + rng.below(2) as u8,
                    });
                }
                if rng.chance(3) {
                    c.locks.push(Lock { foe: 59_999, n: 1 });
                }
            }
            let mut t = s.clone();
            let before = kit::sj(&t);
            let mut cc = c.clone();
            t.acquire(&mut cc, s_c);
            rec(
                "griffinwing.flight.Session.acquire",
                json!({"s": before, "c": j(&c), "s_c": s_c}),
                json!({"s": kit::sj(&t), "c": j(&cc)}),
            );
        }
    }
    if live && want("griffinwing.flight.Session.release") {
        for _ in 0..3 {
            let mut c = craft.clone();
            c.locks.clear();
            for g in s.foes.iter().take(1 + rng.below(4) as usize) {
                c.locks.push(Lock {
                    foe: g.id,
                    n: 1 + (rng.below(3) as u8),
                });
            }
            while c.locks.iter().map(|l| usize::from(l.n)).sum::<usize>() > MAX_LOCKS {
                c.locks.pop();
            }
            c.charge = rng.below(100) as u32;
            let mut t = s.clone();
            let before = kit::sj(&t);
            let mut cc = c.clone();
            t.release(id as u8, &mut cc, s_c, n);
            rec(
                "griffinwing.flight.Session.release",
                json!({"s": before, "owner": id, "c": j(&c), "s_c": s_c, "n": kit::nums_j(n)}),
                json!({"s": kit::sj(&t), "c": j(&cc)}),
            );
        }
    }
    let _ = want_with;
}
