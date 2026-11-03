//! Case recorder for lanternward/kit.rs (Bend port, games_a): the knight's
//! private steps, called on real bot states with the keys the step would pass.
use super::*;
use serde_json::{Value, json};
#[path = "games_a_bend_cases__tests.rs"]
mod cases;
#[path = "lanternward_drive__tests.rs"]
mod drive;
use drive::*;

const CAP: usize = 70;

/// Record a call that works on the session (the knight taken out of it), the
/// knight and the events.
fn kcase(
    name: &str,
    qual: bool,
    stride: usize,
    t: &Session,
    k: &Seat,
    r: &Rules,
    args: Value,
    f: impl FnOnce(&mut Session, &mut Seat, &mut Events) -> Value,
) {
    if !qual || !take(name, stride) || cases::count(name) >= CAP {
        return;
    }
    let (mut t2, mut k2) = (t.clone(), k.clone());
    let input = json!({"s": jv(&t2), "k": jv(&k2), "r": rj(r), "a": args});
    let mut ev = Events::default();
    let ret = f(&mut t2, &mut k2, &mut ev);
    cases::rec_cap(name, &input, &json!({"s": jv(&t2), "k": jv(&k2), "ev": evj(&ev), "ret": ret}), CAP);
}

fn rec(name: &str, input: Value, out: Value) {
    cases::rec_cap(name, &input, &out, CAP);
}

#[test]
#[ignore]
fn record_lanternward_kit() {
    for (n, &(seats, seed, limit)) in TABLES.iter().enumerate() {
        let mut g = Lcg(seed + 31 + n as u64);
        drive(seats, seed, limit, 5, |f| {
            let (s, r) = (f.s, f.r);
            let ids: Vec<u32> = s.seats.keys().copied().collect();
            for id in ids {
                let Some(inp) = f.inputs.get(&id).copied() else { continue };
                let mut t = s.clone();
                let k = t.seats.remove(&id).unwrap();
                let mask = buttons(&inp);
                let press = mask & !k.held;
                let id_ = id;
                // The keys and the seat itself.
                if take("lanternward.kit.buttons", 7) {
                    rec("lanternward.kit.buttons", json!({"i": jv(&inp)}), json!(mask));
                }
                if take("lanternward.kit.Seat.valid", 6) {
                    let p = plan(usize::from(s.room.min(4)));
                    let (w, h) = (p.w() * TILE * Q, p.h() * TILE * Q);
                    let mut bad = k.clone();
                    match g.below(14) {
                        0 => bad.hp = bad.max_hp + 1,
                        1 => bad.x = w + 5,
                        2 => bad.z = 20 * Q,
                        3 => bad.face = (3 * Q, 0),
                        4 => bad.flame = 301,
                        5 => bad.n = 4,
                        6 => bad.charge = 256,
                        7 => bad.mv = (9 * Q, 0),
                        8 => bad.read = Some((1, 200)),
                        9 => bad.cam = (-3, 0),
                        10 => bad.joined = s.elapsed + 1,
                        11 => bad.bombs = 11,
                        12 => bad.pusht = 17,
                        _ => {}
                    }
                    for kk in [&k, &bad] {
                        rec(
                            "lanternward.kit.Seat.valid",
                            json!({"k": jv(kk), "id": id, "elapsed": s.elapsed, "w": w, "h": h}),
                            json!(kk.valid(id, s.elapsed, w, h)),
                        );
                    }
                }
                if take("lanternward.kit.Seat.new", 80) {
                    let at = t.start_of(id);
                    rec(
                        "lanternward.kit.Seat.new",
                        json!({"id": id, "joined": s.elapsed, "at": [at.0, at.1]}),
                        jv(&Seat::new(id, s.elapsed, at)),
                    );
                }
                // Lantern, upkeep, targets and the lock.
                kcase("lanternward.kit.Session.refill", true, 8, &t, &k, r, json!({}), |t, k, _| {
                    t.refill(k, r);
                    Value::Null
                });
                kcase("lanternward.kit.Session.upkeep", true, 8, &t, &k, r, json!({}), |t, k, ev| {
                    t.upkeep(k, ev);
                    Value::Null
                });
                kcase("lanternward.kit.Session.camera", true, 6, &t, &k, r, json!({}), |t, k, _| {
                    t.camera(k, r);
                    Value::Null
                });
                if take("lanternward.kit.Session.targets", 6) {
                    rec("lanternward.kit.Session.targets", json!({"s": jv(&t)}), jv(&t.targets()));
                }
                kcase("lanternward.kit.Session.pick_target", true, 5, &t, &k, r, json!({}), |t, k, _| {
                    jv(&t.pick_target(k, r, None))
                });
                for dir in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
                    kcase(
                        "lanternward.kit.Session.cycle_target",
                        k.lock.is_some(),
                        6,
                        &t,
                        &k,
                        r,
                        json!({"dir": [dir.0, dir.1]}),
                        |t, k, _| jv(&t.cycle_target(k, r, dir)),
                    );
                }
                if let Some(l) = k.lock {
                    kcase("lanternward.kit.Session.lock_holds", true, 5, &t, &k, r, json!({"l": jv(&l)}), |t, k, _| {
                        json!(t.lock_holds(k, l, r))
                    });
                }
                if let Some((l, _, _, _)) = t.targets().first().copied() {
                    kcase("lanternward.kit.Session.take_lock", true, 5, &t, &k, r, json!({"l": jv(&l)}), |t, k, ev| {
                        t.take_lock(k, l, ev);
                        Value::Null
                    });
                }
                kcase("lanternward.kit.Session.lock_on", k.act == Act::Free, 4, &t, &k, r, json!({"id": id, "mask": mask, "press": press, "inp": jv(&inp)}), |t, k, ev| {
                    t.lock_on(id_, k, mask, press, &inp, r, ev);
                    Value::Null
                });
                kcase("lanternward.kit.Session.face_and_move", k.act == Act::Free, 4, &t, &k, r, json!({"inp": jv(&inp)}), |t, k, _| {
                    t.face_and_move(k, &inp, r);
                    Value::Null
                });
                // A shove, and a lean on a block.
                let (dx, dy) = (g.below(2400) as i32 - 1200, g.below(2400) as i32 - 1200);
                kcase("lanternward.kit.Session.shove", true, 6, &t, &k, r, json!({"dx": dx, "dy": dy}), |t, k, _| {
                    t.shove(k, dx, dy);
                    Value::Null
                });
                let (sx, sy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][g.below(4) as usize];
                kcase(
                    "lanternward.kit.Session.lean",
                    t.props.iter().any(|p| p.kind == Pk::Block),
                    2,
                    &t,
                    &k,
                    r,
                    json!({"sx": sx, "sy": sy}),
                    |t, k, _| {
                        t.lean(k, sx, sy);
                        Value::Null
                    },
                );
                if let Some((tx, ty)) = k.lock.and_then(|l| t.lock_point(l)) {
                    let walking = (i32::from(inp.move_x), i32::from(inp.move_y));
                    let v = unit(walking.0, walking.1);
                    let (vx, vy) = (v.0 * WALK / Q, v.1 * WALK / Q);
                    kcase("lanternward.kit.Session.orbit", true, 3, &t, &k, r, json!({"t": [tx, ty], "v": [vx, vy]}), |t, k, _| {
                        t.orbit(k, (tx, ty), (vx, vy));
                        Value::Null
                    });
                }
                // The acts, each in its own act.
                kcase("lanternward.kit.Session.falling", k.act == Act::Fall, 1, &t, &k, r, json!({}), |t, k, _| {
                    t.falling(k);
                    Value::Null
                });
                kcase("lanternward.kit.Session.hurting", k.act == Act::Hurt, 1, &t, &k, r, json!({}), |t, k, _| {
                    t.hurting(k);
                    Value::Null
                });
                let a = json!({"id": id, "mask": mask, "press": press, "inp": jv(&inp)});
                kcase("lanternward.kit.Session.free", k.act == Act::Free, 4, &t, &k, r, a.clone(), |t, k, ev| {
                    t.free(id_, k, mask, press, &inp, r, ev);
                    Value::Null
                });
                kcase(
                    "lanternward.kit.Session.swinging",
                    matches!(k.act, Act::Slash | Act::Lunge | Act::Spin),
                    2,
                    &t,
                    &k,
                    r,
                    a.clone(),
                    |t, k, ev| {
                        t.swinging(id_, k, mask, press, &inp, r, ev);
                        Value::Null
                    },
                );
                kcase("lanternward.kit.Session.rolling", k.act == Act::Roll, 1, &t, &k, r, json!({}), |t, k, _| {
                    t.rolling(k);
                    Value::Null
                });
                kcase("lanternward.kit.Session.hopping", k.act == Act::Hop, 1, &t, &k, r, a.clone(), |t, k, ev| {
                    t.hopping(id_, k, press, r, ev);
                    Value::Null
                });
                kcase("lanternward.kit.Session.plunging", k.act == Act::Plunge, 1, &t, &k, r, json!({"id": id}), |t, k, ev| {
                    t.plunging(id_, k, r, ev);
                    Value::Null
                });
                kcase("lanternward.kit.Session.lantern", true, 4, &t, &k, r, a.clone(), |t, k, ev| {
                    t.lantern(id_, k, mask, press, r, ev);
                    Value::Null
                });
                kcase("lanternward.kit.Session.verbs", k.act != Act::Down && k.act != Act::Item, 3, &t, &k, r, a.clone(), |t, k, ev| {
                    t.verbs(id_, k, mask, press, &inp, r, ev);
                    Value::Null
                });
                kcase("lanternward.kit.Session.begin_dodge", k.act == Act::Free, 3, &t, &k, r, json!({"inp": jv(&inp)}), |t, k, ev| {
                    t.begin_dodge(k, &inp, ev);
                    Value::Null
                });
                for step in 0u8..3 {
                    kcase("lanternward.kit.Session.begin_slash", k.act == Act::Free, 6, &t, &k, r, json!({"step": step}), |t, k, ev| {
                        t.begin_slash(k, step, r, ev);
                        Value::Null
                    });
                }
                kcase("lanternward.kit.Session.begin_lunge", k.act == Act::Free, 6, &t, &k, r, json!({}), |t, k, ev| {
                    t.begin_lunge(k, ev);
                    Value::Null
                });
                kcase("lanternward.kit.Session.begin_spin", k.act == Act::Free, 6, &t, &k, r, json!({}), |t, k, ev| {
                    t.begin_spin(k, ev);
                    Value::Null
                });
                if take("lanternward.kit.Session.recovering", 5) {
                    rec("lanternward.kit.Session.recovering", json!({"k": jv(&k)}), json!(t.recovering(&k)));
                    rec("lanternward.kit.Seat.flaring", json!({"k": jv(&k)}), json!(k.flaring()));
                    rec("lanternward.kit.Seat.up", json!({"k": jv(&k)}), json!(k.up()));
                }
                // A swing's arc, and a lunge's line: the three arcs the sword has.
                for (reach, cos, dmg, heavy) in [(18, 66, 2u32, false), (22, -44, 4, true), (22, -Q, 3, true), (18, -Q, 4, true)] {
                    kcase(
                        "lanternward.kit.Session.sweep",
                        k.act != Act::Down,
                        3,
                        &t,
                        &k,
                        r,
                        json!({"id": id, "reach": reach, "cos": cos, "dmg": dmg, "heavy": heavy}),
                        |t, k, ev| json!(t.sweep(id_, k, reach, cos, dmg, heavy, ev)),
                    );
                }
                kcase("lanternward.kit.Session.lunge_hit", k.act != Act::Down, 3, &t, &k, r, json!({"id": id}), |t, k, ev| {
                    json!(t.lunge_hit(id_, k, ev))
                });
            }
        });
    }
}
