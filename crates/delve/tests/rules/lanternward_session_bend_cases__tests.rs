//! Case recorder for lanternward.rs and the pub(super) phases (Bend port,
//! games_a): Session::new/step/finale/valid/problem/repair, every phase of a
//! tick, the bot's keys, and the small public helpers.
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

/// One phase on `t`: record `{s, r}` before and `{s, ev}` after, when `qual`
/// and on the name's stride.
fn phase(name: &str, qual: bool, k: usize, t: &mut Session, r: &Rules, f: impl FnOnce(&mut Session, &mut Events)) {
    let rec_it = qual && take(name, k) && cases::count(name) < CAP;
    let before = if rec_it { Some(json!({"s": jv(&*t), "r": rj(r)})) } else { None };
    let mut ev = Events::default();
    f(t, &mut ev);
    if let Some(b) = before {
        rec(name, b, json!({"s": jv(&*t), "ev": evj(&ev)}));
    }
}

/// The step, mirrored phase by phase so each can be recorded.
fn mirror(t: &mut Session, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>, r: &Rules) {
    let mut ev = Events::default();
    t.elapsed = (t.elapsed + 1).min(LIMIT);
    let here = |id: &u32| heroes.get(id).is_some_and(|h| !h.away && !h.stone);
    for (&id, k) in t.seats.iter().filter(|(id, _)| !here(id)) {
        t.kept.insert(id, (k.gems, k.bombs));
    }
    t.seats.retain(|id, _| here(id));
    for (&id, _) in heroes.iter().filter(|(_, h)| !h.away && !h.stone) {
        if !t.seats.contains_key(&id) {
            let at = t.start_of(id);
            let mut k = Seat::new(id, t.elapsed, at);
            if let Some((gems, bombs)) = t.kept.remove(&id) {
                (k.gems, k.bombs) = (gems, bombs);
            }
            k.max_hp = START_HP + HEART * u32::from(t.inv.containers);
            k.hp = k.max_hp;
            if t.inv.lantern {
                k.flame = r.flame;
            }
            t.seats.insert(id, k);
        }
    }
    phase("lanternward.foes.Session.provision", true, 40, t, r, |t, _| t.provision());
    let ids: Vec<u32> = t.seats.keys().copied().collect();
    for id in ids {
        let inp = inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
        let name = "lanternward.kit.Session.step_seat";
        let rec_it = take(name, 4) && cases::count(name) < CAP;
        let before = if rec_it { Some(json!({"s": jv(&*t), "id": id, "inp": jv(&inp), "r": rj(r)})) } else { None };
        let mut e = Events::default();
        t.step_seat(id, inp, r, &mut e);
        if let Some(b) = before {
            rec(name, b, json!({"s": jv(&*t), "ev": evj(&e)}));
        }
    }
    let _ = &mut ev;
    phase("lanternward.stage.Session.tick_props", true, 6, t, r, |t, ev| t.tick_props(ev));
    phase("lanternward.stage.Session.unstick", true, 40, t, r, |t, _| t.unstick());
    phase("lanternward.foes.Session.step_foes", !t.foes.is_empty(), 5, t, r, |t, ev| t.step_foes(r, ev));
    phase("lanternward.boss.Session.step_boss", t.boss.is_some(), 7, t, r, |t, ev| t.step_boss(r, ev));
    phase("lanternward.blow.Session.step_bolts", !t.bolts.is_empty(), 4, t, r, |t, ev| t.step_bolts(r, ev));
    phase("lanternward.blow.Session.step_fuses", !t.fuses.is_empty(), 2, t, r, |t, ev| t.step_fuses(ev));
    phase("lanternward.stage.Session.pickups", true, 9, t, r, |t, ev| t.pickups(r, ev));
    phase("lanternward.stage.Session.revive_pass", t.seats.values().any(|k| k.act == Act::Down), 2, t, r, |t, ev| t.revive_pass(ev));
    phase("lanternward.stage.Session.tick_stage", true, 11, t, r, |t, ev| t.tick_stage(r, ev));
    phase("lanternward.stage.Session.tick_wipe", t.seats.values().all(|k| !k.up()), 1, t, r, |t, ev| t.tick_wipe(r, ev));
    phase("lanternward.stage.Session.age", true, 30, t, r, |t, _| t.age());
}

/// A session a bad save could hand over: the `which`th kind of damage.
fn damage(s: &mut Session, which: u64) {
    match which % 24 {
        0 => s.elapsed = LIMIT + 5,
        1 => s.room = 9,
        2 => {
            s.seats.insert(9, Seat::new(9, 0, (Q, Q)));
        }
        3 => {
            if let Some(k) = s.seats.values_mut().next() {
                k.hp = k.max_hp + 3;
            }
        }
        4 => {
            let p = s.props.first().cloned();
            if let Some(p) = p {
                for _ in 0..170 {
                    s.props.push(p.clone());
                }
            }
        }
        5 => {
            if let Some(p) = s.props.first_mut() {
                p.x = -5;
            }
        }
        6 => s.inv.small_keys = 20,
        7 => s.shake = 99,
        8 => s.exiting = 99,
        9 => s.paid = 1,
        10 => s.tries = 2_000_000,
        11 => s.banner = Some(Banner { item: Item::Map, seat: 1, t: 99 }),
        12 => s.done = Some(s.elapsed + 50),
        13 => s.cue = Some((-Q, 0, 10)),
        14 => {
            for id in 1..=8 {
                s.kept.insert(id, (1, 99));
            }
        }
        15 => s.stage = 9,
        16 => s.stage_t = LIMIT + 1,
        17 => {
            if let Some(k) = s.seats.values_mut().next() {
                k.x = 99 * TILE * Q;
                k.joined = s.elapsed + 4;
            }
        }
        18 => {
            let t = Foe::new(0, Fk::Snuffer, Q, Q, 1);
            for _ in 0..20 {
                s.foes.push(t.clone());
            }
        }
        19 => s.bolts.push(Bolt { kind: Bk::Iron, x: Q, y: Q, vx: 9 * Q, vy: 0, ttl: 5, owner: 0 }),
        20 => s.fuses.push(Fuse { x: Q, y: Q, t: 200, owner: 1 }),
        21 => s.inv.containers = 30,
        22 => s.flash = 80,
        _ => {
            s.room = 4;
            s.boss = Some(Warden::new(Q * 40, Q * 40, 1));
        }
    }
}

#[test]
#[ignore]
fn record_lanternward_session() {
    for (n, &(seats, seed, limit)) in TABLES.iter().enumerate() {
        let mut dmg = Lcg(seed + 11);
        drive(seats, seed, limit, 7, |f| {
            let (s, heroes, inputs, r) = (f.s, f.heroes, f.inputs, f.r);
            // The whole step.
            if take("lanternward.Session.step", 9) && cases::count("lanternward.Session.step") < CAP {
                let mut t = s.clone();
                let ev = t.step(inputs, heroes, r);
                rec(
                    "lanternward.Session.step",
                    json!({"s": jv(s), "inputs": jv(inputs), "heroes": hj(heroes), "r": rj(r)}),
                    json!({"s": jv(&t), "ev": evj(&ev)}),
                );
            }
            // Its phases, in order.
            let mut t = s.clone();
            mirror(&mut t, inputs, heroes, r);
            // The bot's keys, a knight at a time.
            for (&id, h) in heroes.iter() {
                if s.seats.contains_key(&id) && take("lanternward.bot.Session.bot_input", 3) && cases::count("lanternward.bot.Session.bot_input") < CAP {
                    rec(
                        "lanternward.bot.Session.bot_input",
                        json!({"s": jv(s), "id": id, "r": rj(r)}),
                        jv(&s.bot_input(id, h, r)),
                    );
                }
            }
            // Bounds, and the mending of a broken session.
            if take("lanternward.Session.finale", 40) {
                rec("lanternward.Session.finale", json!({"s": jv(s)}), json!(s.finale()));
            }
            if take("lanternward.Session.valid", 25) {
                rec("lanternward.Session.valid", json!({"s": jv(s)}), json!(s.valid()));
                rec("lanternward.Session.problem", json!({"s": jv(s)}), json!(s.problem()));
            }
            if take("lanternward.Session.damaged", 12) {
                let mut bad = s.clone();
                damage(&mut bad, dmg.next());
                rec("lanternward.Session.valid", json!({"s": jv(&bad)}), json!(bad.valid()));
                rec("lanternward.Session.problem", json!({"s": jv(&bad)}), json!(bad.problem()));
                let before = jv(&bad);
                bad.repair();
                rec("lanternward.Session.repair", json!({"s": before}), json!({"s": jv(&bad)}));
            }
            // Public helpers on the live state.
            small(f, n);
        });
    }
    // Session::new: a table, with and without absent players.
    for (i, &(seats, seed, _)) in TABLES.iter().enumerate() {
        let mut run = Run::lanternward_for_test();
        for id in 2..=seats {
            run.join_seat(id, &format!("Knight {id}"));
        }
        let mut heroes = run.players.clone();
        rec("lanternward.Session.new", json!({"players": hj(&heroes)}), jv(&Session::new(&heroes)));
        if let Some(h) = heroes.values_mut().next() {
            if i % 2 == 0 && seats > 1 {
                h.away = true;
            } else {
                h.stone = i % 3 == 0;
            }
        }
        rec("lanternward.Session.new", json!({"players": hj(&heroes)}), jv(&Session::new(&heroes)));
        let _ = seed;
    }
    for seats in 0..=8u32 {
        for tell in [40u32, 56, 70, 90] {
            let r = Rules { tell, ..Rules::default() };
            rec("lanternward.Rules.tell_for", json!({"r": rj(&r), "seats": seats}), json!(r.tell_for(seats)));
        }
    }
    for first in [true, false] {
        for gems in [0u32, 3, 20, 21, 99, 1000] {
            rec("lanternward.purse", json!({"first": first, "gems": gems}), json!(purse(first, gems)));
        }
    }
}

/// The public helpers, called on the live state.
fn small(f: &Frame, n: usize) {
    let (s, r) = (f.s, f.r);
    let mut g = Lcg(f.tick as u64 * 7919 + n as u64 + f.seed);
    let spots: Vec<(i32, i32)> = {
        let mut v: Vec<(i32, i32)> = s.seats.values().map(|k| (k.x, k.y)).collect();
        v.extend(s.props.iter().map(|p| (p.x, p.y)));
        v.extend(s.foes.iter().map(|q| (q.x, q.y)));
        v
    };
    if spots.is_empty() {
        return;
    }
    let (px, py) = spots[g.below(spots.len() as u64) as usize];
    let (x, y) = (px + g.below(9 * 256) as i32 - 4 * 256, py + g.below(9 * 256) as i32 - 4 * 256);
    let (qx, qy) = spots[g.below(spots.len() as u64) as usize];
    if take("lanternward.props.Session.clear", 3) {
        let rad = [5 * Q, 2 * Q, 4 * Q, 8 * Q][g.below(4) as usize];
        rec("lanternward.props.Session.clear", json!({"s": jv(s), "x": x, "y": y, "r": rad}), json!(s.clear(x, y, rad)));
        rec("lanternward.props.Session.blocked", json!({"s": jv(s), "x": x, "y": y}), json!(s.blocked(x, y)));
        rec("lanternward.props.Session.pit", json!({"s": jv(s), "x": x, "y": y}), json!(s.pit(x, y)));
    }
    if take("lanternward.props.Session.line_open", 3) {
        rec(
            "lanternward.props.Session.line_open",
            json!({"s": jv(s), "a": [px, py], "b": [qx, qy]}),
            json!(s.line_open((px, py), (qx, qy))),
        );
        rec("lanternward.props.Session.burning", json!({"s": jv(s), "x": x, "y": y}), json!(s.burning(x, y)));
    }
    if take("lanternward.props.Session.secret_open", 30) {
        rec("lanternward.props.Session.secret_open", json!({"s": jv(s)}), json!(s.secret_open()));
        rec("lanternward.boss.Session.boss_exposed", json!({"s": jv(s)}), json!(s.boss_exposed()));
        rec("lanternward.ctx.Session.beacon_ready", json!({"s": jv(s)}), json!(s.beacon_ready()));
        rec("lanternward.ctx.Session.burn_ticks", json!({"s": jv(s)}), json!(s.burn_ticks()));
        rec("lanternward.stage.Session.exit_open", json!({"s": jv(s)}), json!(s.exit_open()));
    }
    for (&id, k) in &s.seats {
        if take("lanternward.ctx.Session.context", 5) {
            let mut t = s.clone();
            t.seats.remove(&id);
            rec("lanternward.ctx.Session.context", json!({"s": jv(&t), "k": jv(k)}), jv(&t.context(k)));
            rec("lanternward.ctx.Session.carrying_boss_key", json!({"s": jv(&t), "k": jv(k)}), json!(t.carrying_boss_key(k)));
        }
        if take("lanternward.hint.line", 4) {
            let mut t = s.clone();
            t.seats.remove(&id);
            rec("lanternward.hint.line", json!({"s": jv(&t), "k": jv(k), "r": rj(r)}), jv(&hint::line(&t, k, r)));
            rec("lanternward.hint.hint", json!({"s": jv(&t), "k": jv(k), "r": rj(r)}), jv(&hint::hint(&t, k, r)));
            rec("lanternward.hint.lock_hint", json!({"s": jv(&t), "k": jv(k)}), jv(&hint::lock_hint(&t, k)));
            rec("lanternward.hint.context", json!({"k": jv(k)}), jv(&hint::context(k)));
        }
        if take("lanternward.kit.Session.lock_point", 4) {
            if let Some(l) = k.lock {
                rec("lanternward.kit.Session.lock_point", json!({"s": jv(s), "l": jv(&l)}), jv(&s.lock_point(l)));
            }
            rec("lanternward.foes.Session.nearest_knight", json!({"s": jv(s), "x": x, "y": y}), jv(&s.nearest_knight(x, y)));
        }
        if take("lanternward.stage.Session.start_of", 30) {
            rec("lanternward.stage.Session.start_of", json!({"s": jv(s), "id": id}), jv(&s.start_of(id)));
        }
    }
    if take("lanternward.foes.Session.nudge", 4) {
        let rad = [4 * Q, 5 * Q][g.below(2) as usize];
        rec("lanternward.foes.Session.nudge", json!({"s": jv(s), "x": x, "y": y, "r": rad}), jv(&s.nudge(x, y, rad)));
    }
    // Rolls, scatter, shatter, gates, fx, drops: on a copy each.
    if take("lanternward.props.Session.roll", 5) {
        let mut t = s.clone();
        let n = [1u32, 6, 100, 7][g.below(4) as usize];
        let v = t.roll(n);
        rec("lanternward.props.Session.roll", json!({"s": jv(s), "n": n}), json!({"s": jv(&t), "v": v}));
        for (name, kind) in [("Grass", Pk::Grass), ("Pot", Pk::Pot), ("Fissure", Pk::Fissure)] {
            let mut u = s.clone();
            u.scatter(kind, x, y);
            rec("lanternward.props.Session.scatter", json!({"s": jv(s), "kind": name, "x": x, "y": y}), json!({"s": jv(&u)}));
        }
        let mut u = s.clone();
        u.fx(Fxk::Spark, x, y, -3);
        rec("lanternward.props.Session.fx", json!({"s": jv(s), "kind": "Spark", "x": x, "y": y, "a": -3}), json!({"s": jv(&u)}));
        let mut u = s.clone();
        u.drop_at(Dk::Gem, x, y);
        rec("lanternward.props.Session.drop_at", json!({"s": jv(s), "kind": "Gem", "x": x, "y": y}), json!({"s": jv(&u)}));
        let mut u = s.clone();
        let mut ev = Events::default();
        u.solved(x, y, &mut ev);
        rec("lanternward.props.Session.solved", json!({"s": jv(s), "x": x, "y": y}), json!({"s": jv(&u), "ev": evj(&ev)}));
    }
    if take("lanternward.props.Session.shatter", 4) {
        if let Some(i) = s.props.iter().position(|p| matches!(p.kind, Pk::Pot | Pk::Grass)) {
            let mut u = s.clone();
            let mut ev = Events::default();
            u.shatter(i, &mut ev);
            rec("lanternward.props.Session.shatter", json!({"s": jv(s), "i": i}), json!({"s": jv(&u), "ev": evj(&ev)}));
        }
        for (id, open) in [(0u8, true), (1, true), (2, true), (1, false)] {
            let mut u = s.clone();
            let changed = u.set_gate(id, open);
            rec("lanternward.props.Session.set_gate", json!({"s": jv(s), "id": id, "open": open}), json!({"s": jv(&u), "changed": changed}));
        }
    }
    // Blows on a knight, and on a foe, from a nearby point.
    if take("lanternward.blow.Session.blow", 3) {
        for (&id, _) in s.seats.iter().take(2) {
            for (kind, name) in [(Hit::Melee, "Melee"), (Hit::Low, "Low"), (Hit::Bolt, "Bolt")] {
                let mut u = s.clone();
                let mut ev = Events::default();
                let from = (x, y);
                let dmg = 2 + g.below(4) as u32;
                let out = u.blow(id, dmg, from, kind, r, &mut ev);
                rec(
                    "lanternward.blow.Session.blow",
                    json!({"s": jv(s), "id": id, "dmg": dmg, "from": [from.0, from.1], "kind": name, "r": rj(r)}),
                    json!({"s": jv(&u), "ev": evj(&ev), "out": format!("{out:?}")}),
                );
            }
        }
        if !s.foes.is_empty() {
            let i = g.below(s.foes.len() as u64) as usize;
            let mut u = s.clone();
            let mut ev = Events::default();
            let hurt = u.hurt_foe(i, 3, (x, y), &mut ev);
            rec(
                "lanternward.blow.Session.hurt_foe",
                json!({"s": jv(s), "i": i, "dmg": 3, "from": [x, y]}),
                json!({"s": jv(&u), "ev": evj(&ev), "hurt": hurt}),
            );
        }
    }
    if s.boss.is_some() && take("lanternward.boss.Session.hurt_boss", 3) {
        let mut u = s.clone();
        let mut ev = Events::default();
        let hurt = u.hurt_boss(4, (x, y), 1, &mut ev);
        rec(
            "lanternward.boss.Session.hurt_boss",
            json!({"s": jv(s), "dmg": 4, "from": [x, y], "owner": 1}),
            json!({"s": jv(&u), "ev": evj(&ev), "hurt": hurt}),
        );
        let mut u = s.clone();
        let mut ev = Events::default();
        u.blast_boss((x, y), &mut ev);
        rec("lanternward.boss.Session.blast_boss", json!({"s": jv(s), "at": [x, y]}), json!({"s": jv(&u), "ev": evj(&ev)}));
        let mut u = s.clone();
        u.light_heart(1);
        rec("lanternward.boss.Session.light_heart", json!({"s": jv(s), "owner": 1}), json!({"s": jv(&u)}));
        let mut u = s.clone();
        u.reset_boss_phase();
        rec("lanternward.boss.Session.reset_boss_phase", json!({"s": jv(s)}), json!({"s": jv(&u)}));
    }
    // Retry and entering rooms.
    if take("lanternward.stage.Session.retry", 60) {
        let mut u = s.clone();
        let mut ev = Events::default();
        u.retry(r, &mut ev);
        rec("lanternward.stage.Session.retry", json!({"s": jv(s), "r": rj(r)}), json!({"s": jv(&u), "ev": evj(&ev)}));
        let mut u = s.clone();
        u.enter_room((s.room as usize + 1) % 5, r);
        rec(
            "lanternward.stage.Session.enter_room",
            json!({"s": jv(s), "room": (s.room as usize + 1) % 5, "r": rj(r)}),
            json!({"s": jv(&u)}),
        );
    }
    // Spawning, provisioning, granting.
    if take("lanternward.foes.Session.spawn_foe", 20) {
        let mut u = s.clone();
        u.spawn_foe(Fk::Turret, x, y, true);
        rec("lanternward.foes.Session.spawn_foe", json!({"s": jv(s), "kind": "Turret", "x": x, "y": y, "hooded": true}), json!({"s": jv(&u)}));
    }
    if take("lanternward.ctx.Session.grant", 20) {
        for (&id, k) in s.seats.iter().take(1) {
            for item in [Item::Map, Item::Lantern, Item::SmallKey, Item::BossKey, Item::Container, Item::Gems(25), Item::Bombs(3)] {
                let mut u = s.clone();
                let mut kk = u.seats.remove(&id).unwrap();
                let mut ev = Events::default();
                u.grant(id, &mut kk, item, r, &mut ev);
                let _ = k;
                rec(
                    "lanternward.ctx.Session.grant",
                    json!({"s": jv(s), "id": id, "item": jv(&item), "r": rj(r)}),
                    json!({"s": jv(&{ u.seats.insert(id, kk); u }), "ev": evj(&ev)}),
                );
            }
        }
    }
}
