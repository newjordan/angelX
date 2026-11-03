//! Case recorder for lanternward/blow.rs private steps (Bend port, games_a).
use super::*;
use serde_json::{Value, json};
#[path = "games_a_bend_cases__tests.rs"]
mod cases;
#[path = "lanternward_drive__tests.rs"]
mod drive;
use drive::*;

const CAP: usize = 70;

fn scase(name: &str, qual: bool, stride: usize, s: &Session, r: &Rules, args: Value, f: impl FnOnce(&mut Session, &mut Events) -> Value) {
    if !qual || !take(name, stride) || cases::count(name) >= CAP {
        return;
    }
    let mut t = s.clone();
    let input = json!({"s": jv(&t), "r": rj(r), "a": args});
    let mut ev = Events::default();
    let ret = f(&mut t, &mut ev);
    cases::rec_cap(name, &input, &json!({"s": jv(&t), "ev": evj(&ev), "ret": ret}), CAP);
}

#[test]
#[ignore]
fn record_lanternward_blow() {
    for &(seats, seed, limit) in TABLES.iter() {
        drive(seats, seed, limit, 3, |f| {
            let (s, r) = (f.s, f.r);
            for i in 0..s.bolts.len() {
                scase("lanternward.blow.Session.bolt_meets", s.bolts[i].ttl > 0, 3, s, r, json!({"i": i}), |t, ev| {
                    t.bolt_meets(i, r, ev);
                    Value::Null
                });
                scase("lanternward.blow.Session.bolt_ends", true, 4, s, r, json!({"i": i}), |t, ev| {
                    t.bolt_ends(i, ev);
                    Value::Null
                });
                for (&id, _) in s.seats.iter().take(1) {
                    for parried in [true, false] {
                        scase(
                            "lanternward.blow.Session.turn_bolt",
                            s.bolts[i].kind == Bk::Iron,
                            2,
                            s,
                            r,
                            json!({"i": i, "id": id, "parried": parried}),
                            |t, ev| {
                                t.turn_bolt(i, id, parried, ev);
                                Value::Null
                            },
                        );
                    }
                }
            }
            for i in 0..s.foes.len() {
                scase("lanternward.blow.Session.light_foe", true, 3, s, r, json!({"i": i}), |t, ev| {
                    t.light_foe(i, ev);
                    Value::Null
                });
            }
        });
    }
}
