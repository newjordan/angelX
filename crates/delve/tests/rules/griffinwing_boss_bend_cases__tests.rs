//! Griffinwing's Wyrm recorder (the private parts of boss.rs), for the Bend
//! port. Called from the main recorder's sampling.
use super::super::bend_kit::{self as kit, j, rec, want, Rng};
use super::*;
use serde_json::json;

/// A session with the Wyrm up, in the state `k` asks for.
fn with_wyrm(s: &Session, rng: &mut Rng) -> Session {
    let mut c = s.clone();
    if c.boss.is_none() {
        c.stage = 2;
        c.t = rail::BOSS_AT + 20 + rng.below(100) as u32;
        c.foes.clear();
        c.rise();
    }
    c
}

pub fn sample(s: &Session, n: &Nums, rng: &mut Rng) {
    if want("griffinwing.boss.Session.rise") {
        for k in 0..3 {
            let mut c = s.clone();
            c.stage = 2;
            c.t = rail::BOSS_AT + rng.below(40) as u32;
            if k == 1 {
                c.rise();
                if let Some(b) = &mut c.boss {
                    b.st = Wyrm::Idle;
                    b.marks.push((5, 6));
                    b.phase = 1;
                }
            }
            if k == 2 {
                while c.foes.len() < MAX_FOES {
                    let id = c.next_id();
                    c.foes.push(Foe { id, kind: Kind::Wisp, s: 0, x: 0, z: 0, vs: 0, vx: 0, vz: 0, hp: 1, full: 1, seats: 1, st: St::Fly, ticks: 0, fx: 0, fz: 0, on: 0, flash: 0, mode: 0, phase: 0 });
                }
            }
            let before = kit::sj(&c);
            c.rise();
            rec("griffinwing.boss.Session.rise", json!({"s": before}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.boss.Session.rest") {
        for phase in 0..3u8 {
            let c = s.clone();
            rec("griffinwing.boss.Session.rest", json!({"s": kit::sj_keys(&c, &["seats"]), "phase": phase}), json!(c.rest(phase)));
        }
    }
    if want("griffinwing.boss.Session.begin_tell") {
        for _ in 0..4 {
            let mut c = with_wyrm(s, rng);
            let mut b = c.boss.take().unwrap();
            b.atk = rng.below(3) as u8;
            b.seq = rng.below(7) as u8;
            b.phase = rng.below(3) as u8;
            if rng.chance(5) {
                for seat in c.seats.values_mut() {
                    seat.craft.down = 10;
                }
            }
            let nn = Nums { tell: rng.below(100) as u32 + 10, window: 40 + rng.below(50) as u32, ..*n };
            let before = kit::sj(&c);
            let bj0 = j(&b);
            c.begin_tell(&mut b, &nn);
            rec(
                "griffinwing.boss.Session.begin_tell",
                json!({"s": before, "b": bj0, "n": kit::nums_j(&nn)}),
                json!({"s": kit::sj(&c), "b": j(&b)}),
            );
        }
    }
    if want("griffinwing.boss.Session.spit") {
        for _ in 0..3 {
            let mut c = with_wyrm(s, rng);
            let mut b = c.boss.take().unwrap();
            b.phase = rng.below(3) as u8;
            b.x = rng.range(-500, 500);
            b.marks = (0..rng.below(6)).map(|_| (rng.range(-600, 600), rng.range(-300, 300))).collect();
            let before = kit::sj(&c);
            let bj0 = j(&b);
            c.spit(&b);
            rec("griffinwing.boss.Session.spit", json!({"s": before, "b": bj0}), json!({"s": kit::sj(&c)}));
        }
    }
    // the Wyrm's own tick, on a Wyrm put in each state
    if want("griffinwing.boss.Session.step_boss") {
        for _ in 0..4 {
            let mut c = with_wyrm(s, rng);
            if let Some(b) = &mut c.boss {
                b.st = [Wyrm::Rise, Wyrm::Idle, Wyrm::Tell, Wyrm::Coil, Wyrm::Roar, Wyrm::Hurt, Wyrm::Dying][rng.below(7) as usize];
                b.ticks = rng.below(3) as u32 + if rng.chance(3) { 12 } else { 0 };
                b.phase = rng.below(3) as u8;
                b.atk = rng.below(3) as u8;
                b.gap = rng.range(-300, 300);
                b.col = rng.range(-300, 300);
                b.marks = (0..rng.below(4)).map(|_| (rng.range(-600, 600), rng.range(-300, 300))).collect();
                if rng.chance(3) {
                    // the lit lamp is out
                    let lit = b.lamps[usize::from(b.phase.min(2))];
                    for f in c.foes.iter_mut().filter(|f| f.id == lit) {
                        f.st = St::Broken;
                        f.hp = 0;
                    }
                }
            }
            let before = kit::sj(&c);
            c.step_boss(n);
            rec("griffinwing.boss.Session.step_boss", json!({"s": before, "n": kit::nums_j(n)}), json!({"s": kit::sj(&c)}));
        }
    }
}
