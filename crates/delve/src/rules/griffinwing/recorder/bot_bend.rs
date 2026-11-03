//! Griffinwing's bot recorder (the private helpers of bot.rs), for the Bend
//! port. Called from the main recorder's sampling.
use super::super::bend_kit::{self as kit, j, rec, want, Rng};
use super::*;
use serde_json::json;

pub fn noise_cases() {
    let mut rng = Rng::new(9, 9);
    for k in 0..80u32 {
        let id = 1 + rng.below(6) as u32;
        let t = rng.below(9000) as u32 / [1u32, 14, 45, 60, 90][(k % 5) as usize];
        let salt = [1u32, 2, 3, 5, 9][(k % 5) as usize];
        let span = [dm(15), dm(10), 2, 1, 2][(k % 5) as usize];
        rec("griffinwing.bot.noise", json!({"id": id, "t": t, "salt": salt, "span": span}), json!(noise(id, t, salt, span)));
    }
}

pub fn sample(s: &Session, n: &Nums, rng: &mut Rng) {
    let ids: Vec<u32> = s.seats.keys().copied().collect();
    if ids.is_empty() {
        return;
    }
    if want("griffinwing.bot.Session.reflector") {
        for id in 1..=6u32 {
            rec("griffinwing.bot.Session.reflector", json!({"s": kit::sj_keys(s, &["seats"]), "id": id}), json!(s.reflector(id)));
        }
    }
    let id = ids[rng.below(ids.len() as u64) as usize];
    let craft = s.seats[&id].craft.clone();
    let s_c = s.craft_s(&craft);
    if want("griffinwing.bot.Session.threat") {
        let mut c = s.clone();
        // a few hostile bolts sent at the craft's plane
        for _ in 0..rng.below(4) {
            let to = (s_c + rng.range(40, 2200), craft.x + rng.range(-300, 300), craft.z + rng.range(-200, 200));
            let v = (PACE - rng.range(20, 90), rng.range(-30, 30), rng.range(-30, 30));
            if c.bolts.len() < MAX_BOLTS {
                c.bolts.push(Bolt {
                    shot: if rng.chance(2) { Arm::Gloam } else { Arm::Spark },
                    owner: 0,
                    s: to.0,
                    x: to.1,
                    z: to.2,
                    vs: v.0,
                    vx: v.1,
                    vz: v.2,
                    dmg: 25,
                    life: 100,
                    aim: 0,
                    src: 0,
                    delay: 0,
                    full: false,
                });
            }
        }
        let r = c.threat(&craft, s_c);
        rec(
            "griffinwing.bot.Session.threat",
            json!({"s": kit::sj_keys(&c, &["bolts"]), "c": j(&craft), "s_c": s_c}),
            json!(r.map(|(t, x, z, g)| json!([t, x, z, g]))),
        );
    }
    if want("griffinwing.bot.Session.room_from") {
        for _ in 0..6 {
            let mut c = craft.clone();
            c.x = rng.range(-n.wx(), n.wx());
            c.z = rng.range(-n.wz(), n.wz());
            let (ox, oz) = (c.x + rng.range(-200, 200), c.z + rng.range(-150, 150));
            let r = Session::room_from(&c, ox, oz, n);
            rec("griffinwing.bot.Session.room_from", json!({"c": j(&c), "ox": ox, "oz": oz, "n": kit::nums_j(n)}), json!([r.0, r.1]));
        }
    }
    if want("griffinwing.bot.Session.footprint_away") {
        for _ in 0..3 {
            let mut c = s.clone();
            // a tell near the craft
            let kind = [Kind::Turret, Kind::Bomber, Kind::Wisp, Kind::Kite][rng.below(4) as usize];
            let id2 = c.next_id();
            c.foes.push(Foe {
                id: id2,
                kind,
                s: s_c + 3000,
                x: 0,
                z: 0,
                vs: 0,
                vx: 0,
                vz: 0,
                hp: 20,
                full: 20,
                seats: 1,
                st: if rng.chance(5) { St::Fly } else { St::Tell },
                ticks: 20,
                fx: craft.x + rng.range(-300, 300) - if rng.chance(2) { 0 } else { dm(8) },
                fz: craft.z + rng.range(-120, 120),
                on: 0,
                flash: 0,
                mode: 0,
                phase: 0,
            });
            let r = c.footprint_away(&craft, n);
            rec(
                "griffinwing.bot.Session.footprint_away",
                json!({"s": kit::sj_keys(&c, &["seats", "foes"]), "c": j(&craft), "n": kit::nums_j(n)}),
                json!(r.map(|(x, z)| [x, z])),
            );
        }
    }
    if want("griffinwing.bot.Session.boss_corridor") {
        for _ in 0..4 {
            let mut c = s.clone();
            if c.boss.is_none() {
                c.stage = 2;
                c.rise();
            }
            if let Some(b) = &mut c.boss {
                b.st = [Wyrm::Tell, Wyrm::Coil, Wyrm::Roar, Wyrm::Idle, Wyrm::Hurt][rng.below(5) as usize];
                b.atk = rng.below(3) as u8;
                b.gap = rng.range(-300, 300);
                b.col = craft.x + rng.range(-250, 250);
            }
            let r = c.boss_corridor(&craft, n);
            rec(
                "griffinwing.bot.Session.boss_corridor",
                json!({"s": kit::sj_keys(&c, &["boss"]), "c": j(&craft), "n": kit::nums_j(n)}),
                json!(r.map(|(x, z)| [x, z])),
            );
        }
    }
    if want("griffinwing.bot.Session.intercept") {
        for _ in 0..3 {
            let mut c = s.clone();
            if c.boss.is_none() {
                c.stage = 2;
                c.foes.clear();
                c.rise();
            }
            let lit = c.boss.as_ref().map_or(0, |b| b.lamps[usize::from(b.phase.min(2))]);
            if rng.chance(4) {
                for f in c.foes.iter_mut().filter(|f| f.id == lit) {
                    f.st = St::Open;
                }
            }
            for _ in 0..rng.below(4) {
                if c.bolts.len() < MAX_BOLTS {
                    c.bolts.push(Bolt {
                        shot: if rng.chance(5) { Arm::Spark } else { Arm::Gloam },
                        owner: 0,
                        s: s_c + rng.range(-100, 2200),
                        x: rng.range(-400, 400),
                        z: rng.range(-200, 300),
                        vs: PACE - rng.range(10, 90),
                        vx: rng.range(-30, 30),
                        vz: rng.range(-30, 30),
                        dmg: 25,
                        life: 200,
                        aim: 0,
                        src: if rng.chance(5) { lit + 1 } else { lit },
                        delay: 0,
                        full: false,
                    });
                }
            }
            let r = c.intercept(s_c);
            rec(
                "griffinwing.bot.Session.intercept",
                json!({"s": kit::sj_keys(&c, &["boss", "foes", "bolts"]), "s_c": s_c}),
                json!(r.map(|(t, x, z)| [t as i64, i64::from(x), i64::from(z)])),
            );
        }
    }
}
