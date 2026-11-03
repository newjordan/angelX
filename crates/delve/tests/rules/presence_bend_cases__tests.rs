//! Records presence.rs and fellowship.rs for the Bend port
//! (`delve/rules/home/presence.bendx`, `fellowship.bend`) over states of the
//! replay corpus.
use super::Hold;
use crate::rules::fellowship;
use crate::rules::home::bend_cases::{Cases, Mix, keep, state, states, to};
use crate::rules::{Input, Run};
use serde_json::{Value, json};

/// The parts of a Run these rules read and write, as the state tree holds them.
fn view(run: &Run) -> Value {
    keep(&state(run), &["tick", "players", "idle", "held", "fellowship", "seat_sounds"])
}

fn pick_seat(run: &Run, m: &mut Mix) -> u32 {
    let ids: Vec<u32> = run.players.keys().copied().collect();
    ids[m.below(ids.len() as u64) as usize]
}

/// Dress a state so the paths a fresh corpus state never takes are taken.
fn stir(run: &mut Run, m: &mut Mix) {
    for hero in run.players.values_mut() {
        hero.away = m.chance(25);
        hero.stone = m.chance(30);
        hero.idle = m.chance(30);
        hero.vigil = m.chance(20);
        hero.absent = m.chance(10);
        hero.invulnerable = m.below(60) as u32;
        hero.shielding = m.chance(30);
        hero.walling = m.chance(30);
        if m.chance(15) {
            hero.hp = 0;
        }
    }
}

#[test]
#[ignore]
fn record_presence() {
    let mut m = Mix(0x9e5e);
    let mut set_away = Cases::new("presence.Run.set_away");
    let mut absent = Cases::new("presence.Run.set_absent");
    let mut rest = Cases::new("presence.Run.rest_until_moved");
    let mut ward = Cases::new("presence.Run.ward_while_held");
    let mut hold = Cases::new("presence.Run.hold");
    let mut wake = Cases::new("presence.Run.wake_on_first_move");
    for (_, mut run) in states(4) {
        stir(&mut run, &mut m);
        for _ in 0..3 {
            let id = if m.chance(5) { 99 } else { pick_seat(&run, &mut m) };
            let away = m.chance(50);
            let before = view(&run);
            let mut r = run.clone();
            r.set_away(id, away);
            set_away.add(json!({"run": before, "id": id, "away": away}), view(&r));
            let absent_now = m.chance(50);
            let mut r = run.clone();
            r.set_absent(id, absent_now);
            absent.add(json!({"run": view(&run), "id": id, "absent": absent_now}), view(&r));
        }
        let mut r = run.clone();
        r.rest_until_moved();
        rest.add(json!({"run": view(&run)}), view(&r));
        let h = [Hold::Open, Hold::Arriving, Hold::Empty][m.below(3) as usize];
        let word = ["open", "arriving", "empty"][[Hold::Open, Hold::Arriving, Hold::Empty].iter().position(|x| *x == h).unwrap()];
        let mut r = run.clone();
        r.hold(h);
        hold.add(json!({"run": view(&run), "hold": word}), view(&r));
        let mut held = run.clone();
        held.hold(h);
        let before = view(&held);
        held.ward_while_held();
        ward.add(json!({"run": before}), view(&held));
        let id = pick_seat(&run, &mut m);
        let mut hero = run.players[&id].clone();
        let input = Input {
            move_x: *m.pick(&[0, 0, 1, -1]),
            move_y: *m.pick(&[0, 0, 1, -1]),
            fire: m.chance(15),
            dash: m.chance(10),
            swing: m.chance(10),
            interact: m.chance(10),
            ..Input::default()
        };
        let before = to(&hero);
        Run::wake_on_first_move(&mut hero, &input);
        wake.add(json!({"hero": before, "input": to(&input)}), to(&hero));
    }
    eprintln!("presence {} {} {} {} {} {}", set_away.len(), absent.len(), rest.len(), ward.len(), hold.len(), wake.len());
    for c in [&set_away, &absent, &rest, &ward, &hold, &wake] {
        c.save();
    }
}

#[test]
#[ignore]
fn record_fellowship() {
    let mut m = Mix(0xfe11);
    let mut call = Cases::new("fellowship.Run.call_fellowship");
    let mut clear = Cases::new("fellowship.Run.clear");
    let mut wait = Cases::new("fellowship.Run.wait");
    let mut line = Cases::new("fellowship.Run.fellowship_line");
    let mut counts = Cases::new("fellowship.Run.counts_in_fellowship");
    for (_, mut run) in states(6) {
        stir(&mut run, &mut m);
        for hero in run.players.values_mut() {
            hero.away = m.chance(15);
            hero.absent = m.chance(8);
            hero.stone = m.chance(15);
        }
        let ids: Vec<u32> = run.players.keys().copied().collect();
        // Some of the party wait in ways, some already called and rung.
        let mut waiting = Vec::new();
        for &id in &ids {
            if m.chance(60) {
                waiting.push((id, *m.pick(&[0u8, 1, 2, 4, 4, 5])));
            }
        }
        let mut r = run.clone();
        r.tick = 200 + m.below(1000);
        r.fellowship.clear();
        r.fellowship.wait(waiting.iter().copied());
        if m.chance(40) {
            for &id in &ids {
                if m.chance(50) {
                    r.fellowship.rung.insert(id, r.tick - m.below(300).min(r.tick));
                }
            }
            r.fellowship.called = ids.iter().copied().filter(|_| m.chance(50)).collect();
        }
        let before = view(&r);
        let mut after = r.clone();
        after.call_fellowship();
        call.add(json!({"run": before}), view(&after));
        let mut c = r.clone();
        c.fellowship.clear();
        clear.add(json!({"run": view(&r)}), view(&c));
        let mut w = run.clone();
        w.fellowship.clear();
        let mut ways: Vec<(u32, u8)> = Vec::new();
        for &i in &ids {
            if m.chance(50) {
                ways.push((i, m.below(6) as u8));
            }
        }
        let b = view(&w);
        w.fellowship.wait(ways.iter().copied());
        wait.add(json!({"run": b, "ways": ways.iter().map(|&(i, w)| json!([i, w])).collect::<Vec<_>>()}), view(&w));
        let focus: Option<u32> = if m.chance(20) { None } else { Some(ids[m.below(ids.len() as u64) as usize]) };
        line.add(json!({"run": view(&r), "focus": focus}), json!(r.fellowship_line(focus)));
        for &id in ids.iter().take(2) {
            counts.add(json!({"run": view(&r), "id": id}), json!(r.counts_in_fellowship(id)));
        }
    }
    eprintln!("fellowship {} {} {} {} {}", call.len(), clear.len(), wait.len(), line.len(), counts.len());
    for c in [&call, &clear, &wait, &line, &counts] {
        c.save();
    }
    let _: Value = json!(null);
}
