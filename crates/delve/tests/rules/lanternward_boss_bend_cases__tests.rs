//! Case recorder for lanternward/boss.rs private steps (Bend port, games_a).
use super::*;
use serde_json::{Value, json};
#[path = "games_a_bend_cases__tests.rs"]
mod cases;
#[path = "lanternward_drive__tests.rs"]
mod drive;
use drive::*;

const CAP: usize = 70;

fn rec(name: &str, input: Value, out: Value) {
    cases::rec_cap(name, &input, &out, CAP);
}

#[test]
#[ignore]
fn record_lanternward_boss() {
    for seats in 0..=7u32 {
        let w = Warden::new(5 * TILE * Q, 6 * TILE * Q, seats);
        rec("lanternward.boss.Warden.new", json!({"x": w.x, "y": w.y, "seats": seats}), jv(&w));
    }
    for &(seats, seed, limit) in TABLES.iter() {
        drive(seats, seed, limit, 3, |f| {
            let (s, r) = (f.s, f.r);
            let Some(w) = &s.boss else { return };
            let n = s.seats.len() as u32;
            if take("lanternward.boss.Warden.exposed", 3) {
                rec("lanternward.boss.Warden.valid", json!({"w": jv(w)}), json!(w.valid()));
                rec("lanternward.boss.Warden.need", json!({"w": jv(w), "seats": n}), json!(w.need(n)));
                rec(
                    "lanternward.boss.Warden.exposed",
                    json!({"w": jv(w), "now": s.elapsed, "seats": n}),
                    json!(w.exposed(s.elapsed, n)),
                );
                rec("lanternward.boss.Warden.heart", json!({"w": jv(w)}), jv(&w.heart()));
            }
            // The private steps, with the Warden taken out as step_boss does.
            let mut t = s.clone();
            let w = t.boss.take().unwrap();
            if let Some((_, tx, ty, _)) = t.nearest_knight(w.x, w.y)
                && take("lanternward.boss.Session.begin_attack", 3)
            {
                let (mut t2, mut w2) = (t.clone(), w.clone());
                t2.begin_attack(&mut w2, (tx, ty), r);
                rec(
                    "lanternward.boss.Session.begin_attack",
                    json!({"s": jv(&t), "w": jv(&w), "at": [tx, ty], "r": rj(r)}),
                    json!({"s": jv(&t2), "w": jv(&w2)}),
                );
            }
            for (line, dmg, kind, name) in [
                (Danger::Capsule { ax: w.x, ay: w.y, bx: w.x + 90 * Q, by: w.y, r: 6 * Q }, BEAM_DAMAGE, Hit::Beam, "Beam"),
                (Danger::Ring { x: w.x, y: w.y, r: 30 * Q, w: 6 * Q }, SLAM_DAMAGE, Hit::Low, "Low"),
                (w.danger, BEAM_DAMAGE, Hit::Beam, "Beam"),
            ] {
                if take("lanternward.boss.Session.warden_hits", 2) {
                    let (mut t2, mut w2) = (t.clone(), w.clone());
                    let mut ev = Events::default();
                    let parried = t2.warden_hits(&mut w2, line, dmg, kind, r, &mut ev);
                    rec(
                        "lanternward.boss.Session.warden_hits",
                        json!({"s": jv(&t), "w": jv(&w), "line": jv(&line), "dmg": dmg, "kind": name, "r": rj(r)}),
                        json!({"s": jv(&t2), "w": jv(&w2), "ev": evj(&ev), "parried": parried}),
                    );
                }
            }
            if take("lanternward.boss.Session.summon", 2) {
                let mut t2 = t.clone();
                let mut ev = Events::default();
                t2.summon(&w, &mut ev);
                rec("lanternward.boss.Session.summon", json!({"s": jv(&t), "w": jv(&w)}), json!({"s": jv(&t2)}));
            }
            if take("lanternward.boss.Session.prop_at", 3) {
                for p in t.props.iter().filter(|p| p.kind == Pk::Fissure).take(2) {
                    rec(
                        "lanternward.boss.Session.prop_at",
                        json!({"s": jv(&t), "kind": "Fissure", "tx": p.tx, "ty": p.ty}),
                        jv(&t.prop_at(Pk::Fissure, i32::from(p.tx), i32::from(p.ty))),
                    );
                }
            }
            let mut t2 = s.clone();
            let mut ev = Events::default();
            t2.stage_warden(r, &mut ev);
            if take("lanternward.boss.Session.stage_warden", 4) {
                rec("lanternward.boss.Session.stage_warden", json!({"s": jv(s), "r": rj(r)}), json!({"s": jv(&t2), "ev": evj(&ev)}));
            }
        });
    }
}
