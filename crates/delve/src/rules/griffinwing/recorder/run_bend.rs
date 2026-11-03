//! Griffinwing's Run-hook recorder, for the Bend port: the door, the tick, the
//! finish, and the shell's ADVENTURE and ACTIVITY entries. A case's `run` is the
//! checkpoint's run object cut down to the fields the game reads and writes.
use super::bend_kit::{self as kit, j, rec, want, want_with, Rng};
use super::*;
use serde_json::{json, Map, Value};

/// The run as the hooks see it.
fn tj(r: &Run) -> Value {
    let full = crate::rules::runtime_state::capture(r).unwrap().run;
    let mut o = Map::new();
    for k in ["griffinwing", "marks", "found", "cues", "sounds", "tick", "forest"] {
        if let Some(v) = full.get(k) {
            o.insert(k.into(), v.clone());
        }
    }
    let mut ps = Map::new();
    for (id, h) in full["players"].as_object().unwrap() {
        let mut m = Map::new();
        for k in ["away", "stone", "carried", "laurel"] {
            if let Some(v) = h.get(k) {
                m.insert(k.into(), v.clone());
            }
        }
        ps.insert(id.clone(), Value::Object(m));
    }
    o.insert("players".into(), Value::Object(ps));
    o.insert("exits".into(), json!({"outside": full["exits"]["outside"]}));
    o.insert("tune".into(), json!({"roundtable": full["tune"]["roundtable"]}));
    let mut home = Map::new();
    if let Some(v) = full["home"].get("roundtable") {
        home.insert("roundtable".into(), v.clone());
    }
    let mut games = Map::new();
    if let Some(v) = full["home"]["countryside"]["games"].get("roundtable") {
        games.insert("roundtable".into(), v.clone());
    }
    home.insert("countryside".into(), json!({"games": Value::Object(games)}));
    o.insert("home".into(), Value::Object(home));
    let kind = full["dungeon"]["rooms"][full["at"].as_u64().unwrap() as usize]["kind"].clone();
    o.insert("at".into(), json!(0));
    o.insert("dungeon".into(), json!({"rooms": [{"kind": kind}]}));
    Value::Object(o)
}

fn table(seats: u32) -> Run {
    let mut r = Run::griffinwing_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

fn outside(r: &mut Run, id: u32) {
    r.exits.outside.insert(id, exits::Entry { at: 0, party: BTreeMap::new() });
}

#[test]
#[ignore = "records the Bend port's cases"]
fn record_griffinwing_run_bend_cases() {
    for (ri, seats) in [1u32, 2, 3, 4, 6, 2, 5].into_iter().enumerate() {
        let ri = 100 + ri;
        let mut rng = Rng::new(ri as u64, 7);
        let mut r = table(seats);
        let mut settled_seen = 0;
        for tick in 0..14_000u32 {
            kit::at(ri, tick);
            let inputs = r.roundtable_inputs();
            let (mode, mode_t, finished) = {
                let s = r.griffinwing.as_ref().unwrap();
                (s.mode, s.mode_t, s.finished())
            };
            let quiet = !(finished && mode_t > TALLY_HOLD - 2);
            // the tick, on states the hooks pass through (never a tick that
            // lets a knight leave: that is another group's door)
            let sample = want_with("griffinwing.Run.tick_griffinwing", 4, 1500) || (finished && mode_t < 5 && settled_seen < 8);
            if sample && quiet && inputs.values().all(|i| !(finished && i.interact)) {
                let mut c = r.clone();
                if rng.chance(3) && seats > 1 {
                    outside(&mut c, 2);
                }
                if finished && rng.chance(2) {
                    c.griffinwing.as_mut().unwrap().first = rng.chance(2);
                }
                let before = tj(&c);
                c.tick_griffinwing(&inputs);
                rec("griffinwing.Run.tick_griffinwing", json!({"run": before, "inputs": kit::inputs_j(&inputs)}), json!({"run": tj(&c)}));
                if finished {
                    settled_seen += 1;
                }
            }
            if want_with("griffinwing.Run.griffinwing_showing", 3, 2000) {
                rec("griffinwing.Run.griffinwing_showing", json!({"run": tj(&r)}), json!(r.griffinwing_showing()));
                rec("griffinwing.ACTIVITY.on", json!({"run": tj(&r)}), json!((ACTIVITY.on)(&r)));
                let mut c = r.clone();
                let before = tj(&c);
                (ACTIVITY.end)(&mut c);
                rec("griffinwing.ACTIVITY.end", json!({"run": before}), json!({"run": tj(&c)}));
                let mut c = r.clone();
                let before = tj(&c);
                (ADVENTURE.clear)(&mut c);
                rec("griffinwing.ADVENTURE.clear", json!({"run": before}), json!({"run": tj(&c)}));
                rec("griffinwing.ADVENTURE.valid", json!({"run": tj(&r)}), json!((ADVENTURE.valid)(&r)));
                rec("griffinwing.ADVENTURE.seated", json!({"run": tj(&r)}), json!((ADVENTURE.seated)(&r)));
                for id in 0..=7u32 {
                    rec("griffinwing.ADVENTURE.bot", json!({"run": tj(&r), "id": id}), j(&(ADVENTURE.bot)(&r, id)));
                }
            }
            if want_with("griffinwing.ACTIVITY.ride", 3, 2000) {
                let mut c = r.clone();
                let before = tj(&c);
                let out = (ACTIVITY.ride.unwrap())(&mut c, &inputs);
                rec(
                    "griffinwing.ACTIVITY.ride",
                    json!({"run": before, "inputs": kit::inputs_j(&inputs)}),
                    json!({"run": tj(&c), "inputs": out.map(|m| kit::inputs_j(&m))}),
                );
            }
            // the settle on the finished session, many ways
            if finished && mode_t == 1 {
                settles(&r, &mut rng);
            }
            r.tick_griffinwing(&inputs);
            let _ = mode;
            if r.griffinwing.as_ref().is_some_and(|s| s.finished() && s.mode_t > 20) {
                break;
            }
        }
        doors(&r, seats, &mut rng);
    }
    for name in [
        "griffinwing.Run.griffinwing_showing",
        "griffinwing.Run.start_griffinwing",
        "griffinwing.Run.tick_griffinwing",
        "griffinwing.Run.settle_griffinwing",
        "griffinwing.ADVENTURE.valid",
        "griffinwing.ADVENTURE.clear",
        "griffinwing.ADVENTURE.seated",
        "griffinwing.ADVENTURE.bot",
        "griffinwing.ACTIVITY.on",
        "griffinwing.ACTIVITY.ride",
        "griffinwing.ACTIVITY.end",
        "griffinwing.settle",
    ] {
        eprintln!("{name}: {}", kit::cases::count(name));
    }
}

/// The finish paid out, with the finished session changed this way and that.
fn settles(r: &Run, rng: &mut Rng) {
    for k in 0..8 {
        kit::at(300 + k, 0);
        let mut c = r.clone();
        let mut s = c.griffinwing.take().unwrap();
        match k % 8 {
            1 => s.medal = Some(Medal::Gold),
            2 => s.medal = Some(Medal::Silver),
            3 => s.medal = None,
            4 => s.settled = true,
            5 => s.paid = bit(1),
            6 => {
                for seat in s.seats.values_mut() {
                    seat.joined = s.done.unwrap_or(0) + 50;
                }
            }
            7 => s.first = false,
            _ => {}
        }
        if rng.chance(3) {
            c.home.roundtable[SLOT] = rng.below(4) as u32;
        }
        if rng.chance(3) {
            let rec0 = &mut c.home.countryside.games.roundtable[SLOT];
            rec0.plays = 1 + rng.below(4) as u32;
            rec0.wins = rng.below(2) as u32;
            rec0.best = rng.below(1000) as u16;
        }
        let before = json!({"run": tj(&c), "s": kit::sj(&s)});
        c.settle_griffinwing(&mut s);
        rec("griffinwing.Run.settle_griffinwing", before, json!({"run": tj(&c), "s": kit::sj(&s)}));
    }
    // the realm settle
    for k in 0..6 {
        let mut home = r.home.clone();
        home.roundtable[SLOT] = [0, 1, 2, 3, 0, 1][k];
        let mut marks = BTreeMap::new();
        if k % 3 != 2 {
            marks.insert(CAMPAIGN_MARK.to_string(), 1);
        }
        if k % 2 == 0 {
            marks.insert(GOLD_MARK.to_string(), 1);
        }
        let hv = |h: &home::Home| {
            let v = serde_json::to_value(h).unwrap();
            match v.get("roundtable") {
                Some(rt) => json!({"home": {"roundtable": rt}}),
                None => json!({"home": {}}),
            }
        };
        let before = json!({"realm": hv(&home), "marks": marks});
        let mut treasury = Spoils::default();
        let reclaimed = BTreeMap::new();
        let mut realm = activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &reclaimed, raids_won: 0 };
        let out = (ACTIVITY.settle.unwrap())(&mut realm, &marks);
        let after = json!({"realm": hv(&home), "settled": {"changed": out.changed, "said": out.said}});
        rec("griffinwing.settle", before, after);
    }
}

/// The door: the party let in, from homes in each state.
fn doors(r: &Run, seats: u32, rng: &mut Rng) {
    kit::at(400 + seats as usize, 0);
    for k in 0..10 {
        let mut c = table(1 + (k + seats) % 6);
        c.griffinwing = None;
        c.home.roundtable[SLOT] = [0, 1, 2, 3][k as usize % 4];
        if k % 3 == 0 {
            c.forest.hunt_left = 40;
            c.forest.caught = 2;
            c.forest.singing = true;
            c.forest.show_left = 12;
            c.forest.note = 3;
            c.forest.set_held_for_griffinwing_cases(5);
        }
        if k % 4 == 1 {
            let rec0 = &mut c.home.countryside.games.roundtable[SLOT];
            rec0.plays = 4;
            rec0.wins = 2;
            rec0.best = 700;
        }
        if k % 5 == 2 {
            if let Some(h) = c.players.values_mut().next() {
                h.away = true;
            }
        }
        c.tick = rng.below(100_000);
        let before = tj(&c);
        c.start_griffinwing();
        rec("griffinwing.Run.start_griffinwing", json!({"run": before}), json!({"run": tj(&c)}));
        // not in the forest: it does not start
        let mut d = c.clone();
        d.griffinwing = None;
        let _ = &mut d;
    }
    let _ = r;
    let _ = want;
}
