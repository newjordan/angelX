//! Case recorder for lanternward/stage.rs private steps (Bend port, games_a).
use super::*;
use serde_json::{Value, json};
#[path = "games_a_bend_cases__tests.rs"]
mod cases;
#[path = "lanternward_drive__tests.rs"]
mod drive;
use drive::*;

const CAP: usize = 70;

fn scase(
    name: &str,
    qual: bool,
    stride: usize,
    s: &Session,
    r: &Rules,
    args: Value,
    f: impl FnOnce(&mut Session, &mut Events) -> Value,
) {
    if !qual || !take(name, stride) || cases::count(name) >= CAP {
        return;
    }
    let mut t = s.clone();
    let input = json!({"s": jv(&t), "r": rj(r), "a": args});
    let mut ev = Events::default();
    let ret = f(&mut t, &mut ev);
    cases::rec_cap(
        name,
        &input,
        &json!({"s": jv(&t), "ev": evj(&ev), "ret": ret}),
        CAP,
    );
}

#[test]
#[ignore]
fn record_lanternward_stage() {
    for &(seats, seed, limit) in TABLES.iter() {
        drive(seats, seed, limit, 3, |f| {
            let (s, r) = (f.s, f.r);
            let room = usize::from(s.room);
            scase(
                "lanternward.stage.Session.stage_lamp_hall",
                s.room == 1,
                3,
                s,
                r,
                json!({}),
                |t, ev| {
                    t.stage_lamp_hall(ev);
                    Value::Null
                },
            );
            scase(
                "lanternward.stage.Session.stage_gallery",
                s.room == 2,
                3,
                s,
                r,
                json!({}),
                |t, ev| {
                    t.stage_gallery(ev);
                    Value::Null
                },
            );
            scase(
                "lanternward.stage.Session.stage_vault",
                s.room == 3,
                3,
                s,
                r,
                json!({}),
                |t, ev| {
                    t.stage_vault(ev);
                    Value::Null
                },
            );
            scase(
                "lanternward.stage.Session.open_exit",
                true,
                40,
                s,
                r,
                json!({}),
                |t, ev| {
                    t.open_exit(ev);
                    Value::Null
                },
            );
            scase(
                "lanternward.stage.Session.provision_room",
                true,
                40,
                s,
                r,
                json!({"room": room}),
                |t, _| {
                    t.provision_room(room);
                    Value::Null
                },
            );
            scase(
                "lanternward.stage.Session.spawn_wave",
                true,
                40,
                s,
                r,
                json!({}),
                |t, _| {
                    t.spawn_wave();
                    Value::Null
                },
            );
            for kind in [Pk::Brazier, Pk::Plate, Pk::Crystal, Pk::Chest] {
                scase(
                    "lanternward.stage.Session.all_lit",
                    true,
                    20,
                    s,
                    r,
                    json!({"kind": jv(&kind)}),
                    |t, _| json!(t.all_lit(kind)),
                );
            }
            scase(
                "lanternward.stage.Session.foes_down",
                true,
                10,
                s,
                r,
                json!({}),
                |t, _| json!(t.foes_down()),
            );
            for (kind, id) in [
                (Pk::Chest, 0u8),
                (Pk::Chest, 1),
                (Pk::Gate, 1),
                (Pk::Plate, 1),
                (Pk::Great, 0),
            ] {
                scase(
                    "lanternward.stage.Session.prop_of",
                    true,
                    12,
                    s,
                    r,
                    json!({"kind": jv(&kind), "id": id}),
                    |t, _| jv(&t.prop_of(kind, id)),
                );
            }
        });
    }
}
