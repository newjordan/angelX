//! Case recorder for lanternward/foes.rs private steps (Bend port, games_a).
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
fn record_lanternward_foes() {
    for &(seats, seed, limit) in TABLES.iter() {
        drive(seats, seed, limit, 3, |f| {
            let (s, r) = (f.s, f.r);
            let n = s.seats.len() as u32;
            for i in 0..s.foes.len() {
                let fo = &s.foes[i];
                let target = s.nearest_knight(fo.x, fo.y);
                if fo.st == Fs::Dying || !take("lanternward.foes.step_foe", 2) {
                    continue;
                }
                match fo.kind {
                    Fk::Snuffer => {
                        let mut g = fo.clone();
                        let mut strikes: Vec<(usize, Danger, u32)> = Vec::new();
                        step_snuffer(&mut g, target, i, &mut strikes);
                        rec(
                            "lanternward.foes.step_snuffer",
                            json!({"f": jv(fo), "target": jv(&target), "index": i}),
                            json!({"f": jv(&g), "strikes": jv(&strikes)}),
                        );
                    }
                    Fk::Turret => {
                        let mut g = fo.clone();
                        let mut shots: Vec<Bolt> = Vec::new();
                        let mut ev = Events::default();
                        step_turret(&mut g, target, r, n, &mut shots, &mut ev);
                        rec(
                            "lanternward.foes.step_turret",
                            json!({"f": jv(fo), "target": jv(&target), "r": rj(r), "seats": n}),
                            json!({"f": jv(&g), "shots": jv(&shots), "ev": evj(&ev)}),
                        );
                    }
                }
                if fo.danger != Danger::None && take("lanternward.foes.Session.strike_with", 2) {
                    let mut t = s.clone();
                    let mut ev = Events::default();
                    t.strike_with(i, fo.danger, 2, r, &mut ev);
                    rec(
                        "lanternward.foes.Session.strike_with",
                        json!({"s": jv(s), "i": i, "danger": jv(&fo.danger), "dmg": 2, "r": rj(r)}),
                        json!({"s": jv(&t), "ev": evj(&ev)}),
                    );
                }
                if take("lanternward.foes.Danger.hits", 2) {
                    let (x, y) = s.seats.values().next().map_or((0, 0), |k| (k.x, k.y));
                    rec(
                        "lanternward.foes.Danger.hits",
                        json!({"d": jv(&fo.danger), "x": x, "y": y, "rad": 5 * Q}),
                        json!(fo.danger.hits(x, y, 5 * Q)),
                    );
                }
                if take("lanternward.foes.Foe.tell", 5) {
                    rec("lanternward.foes.Foe.tell", json!({"f": jv(fo), "r": rj(r), "seats": n}), json!(fo.tell(r, n)));
                    rec("lanternward.foes.Foe.vulnerable", json!({"f": jv(fo)}), json!(fo.vulnerable()));
                }
            }
            if let Some(w) = &s.boss {
                let (x, y) = s.seats.values().next().map_or((0, 0), |k| (k.x, k.y));
                for d in [w.danger, Danger::Fan { x: w.x, y: w.y, deg: 45, half: 56, r: 150 * Q }, Danger::Ring { x: w.x, y: w.y, r: 40 * Q, w: 6 * Q }] {
                    if take("lanternward.foes.Danger.hits", 3) {
                        rec("lanternward.foes.Danger.hits", json!({"d": jv(&d), "x": x, "y": y, "rad": 5 * Q}), json!(d.hits(x, y, 5 * Q)));
                    }
                }
            }
        });
    }
    // Foes of every kind and size.
    for seats in 0..=7u32 {
        for (id, kind) in [(0u8, Fk::Snuffer), (3, Fk::Turret)] {
            rec(
                "lanternward.foes.Foe.new",
                json!({"id": id, "kind": jv(&kind), "x": 3 * Q, "y": 5 * Q, "seats": seats}),
                jv(&Foe::new(id, kind, 3 * Q, 5 * Q, seats)),
            );
        }
    }
}
