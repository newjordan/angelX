//! Records the rescues' tick for the Bend port (`delve/rules/home/rescues.bendx`).
use super::{Captive, resident};
use crate::rules::home::bend_cases::{Cases, Mix, keep, state, states, to};
use crate::rules::{Input, Run};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn view(run: &Run) -> Value {
    let mut v = keep(&state(run), &["players", "captive", "home", "cues", "sounds", "marks", "found", "tick", "at", "met", "notices"]);
    v["home"] = keep(&v["home"], &["residents"]);
    v
}

#[test]
#[ignore]
fn record_rescues() {
    let mut m = Mix(0x3e5c);
    let mut cage = Cases::new("rescues.Run.tick_cage");
    let mut help = Cases::new("rescues.Run.residents_help");
    let mut greet = Cases::new("rescues.Run.greet_residents");
    let mut pip = Cases::new("rescues.Run.pips_map");
    let mut who = Cases::new("rescues.resident");
    for id in ["mabel", "anselm", "pip", "maud", "nobody", ""] {
        who.add(json!({"id": id}), match resident(id) {
            Some(r) => json!({"id": r.id, "name": r.name, "pack": serde_json::to_value(r.pack).unwrap(), "gives": r.gives, "home_at": [r.home_at.0, r.home_at.1]}),
            None => json!(null),
        });
    }
    for (_, mut run) in states(3) {
        for _ in 0..4 {
            run.home.residents.clear();
            for r in ["mabel", "anselm", "pip", "maud"] {
                if m.chance(50) {
                    run.home.residents.insert(r.into());
                }
            }
            let ids: Vec<u32> = run.players.keys().copied().collect();
            for id in &ids {
                let hero = run.players.get_mut(id).unwrap();
                hero.hp = *m.pick(&[0, 40, 40]);
                hero.stone = m.chance(15);
                hero.winds = *m.pick(&[0u8, 1, 254, 255]);
                hero.buying = *m.pick(&[0, 3, 21, 22]);
                hero.hand = (0..m.below(5)).map(|_| "potion".to_string()).collect();
                if m.chance(40) {
                    let (x, y) = *m.pick(&[(9.0 * 2.0, 11.6 * 2.0), (3.9 * 2.0, 8.7 * 2.0), (15.4 * 2.0 + 1.0, 5.6), (32.0, 22.0), (60.0, 60.0)]);
                    hero.x = x;
                    hero.y = y;
                }
            }
            let first = run.players.values().next().unwrap().clone();
            run.captive = if m.chance(85) {
                Some(Captive { who: (*m.pick(&["mabel", "pip", "white-horse"])).into(), room: if m.chance(85) { run.at } else { run.at + 1 }, x: first.x + *m.pick(&[0.0, 1.0, 2.9, 3.1]), y: first.y, freed: if m.chance(10) { Some(3) } else { None } })
            } else {
                None
            };
            if let Some(room) = run.dungeon.rooms.get_mut(run.at) {
                room.cleared = m.chance(50);
            }
            run.met = *m.pick(&[0u8, 1, 3, 15]);
            if m.chance(25) {
                run.found = Some((run.tick.saturating_sub(10), 1, "THE MIRE KING HOLDS THE PEN.".into()));
            }
            let mut inputs: BTreeMap<u32, Input> = BTreeMap::new();
            for id in &ids {
                if m.chance(85) {
                    inputs.insert(*id, Input { interact: m.chance(80), ..Input::default() });
                }
            }
            let before = view(&run);
            let mut after = run.clone();
            after.tick_cage(&inputs);
            cage.add(json!({"run": before, "inputs": to(&inputs)}), view(&after));
            let mut after = run.clone();
            after.residents_help();
            help.add(json!({"run": before}), view(&after));
            let mut after = run.clone();
            after.greet_residents();
            greet.add(json!({"run": before}), view(&after));
        }
        let mut after = run.clone();
        after.home.residents.insert("pip".into());
        let b = keep(&state(&after), &["home", "dungeon"]);
        let mut b2 = b.clone();
        b2["home"] = keep(&b["home"], &["residents"]);
        after.pips_map();
        let mut o = keep(&state(&after), &["home", "dungeon"]);
        o["home"] = keep(&o["home"], &["residents"]);
        pip.add(json!({"run": b2}), o);
    }
    for c in [&cage, &help, &greet, &pip, &who] {
        c.save();
    }
}
