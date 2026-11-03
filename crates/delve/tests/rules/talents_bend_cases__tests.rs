//! Records Sir Ector's lessons for the Bend port (`delve/rules/home/talents.bendx`).
use super::{Prowess, knight_key, lectern_at};
use crate::rules::home::bend_cases::{Cases, Mix, keep, state, states, to};
use crate::rules::Input;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn view(run: &crate::rules::Run) -> Value {
    let mut v = keep(&state(run), &["players", "orders", "greeted", "cues", "home"]);
    v["home"] = keep(&v["home"], &["knights"]);
    v
}

#[test]
#[ignore]
fn record_talents() {
    let mut m = Mix(0x7a1e);
    let mut learn = Cases::new("home.Home.learn");
    let mut prowess = Cases::new("home.Home.prowess");
    let mut level = Cases::new("talents.Prowess.level");
    let mut waiting = Cases::new("talents.Prowess.waiting");
    let mut key = Cases::new("talents.knight_key");
    let mut lectern = Cases::new("talents.lectern_at");
    let mut lessons = Cases::new("talents.Run.tick_lessons");
    for _ in 0..120 {
        let p = Prowess { xp: *m.pick(&[0, 100, 599, 600, 1800, 3000, 3600, 6000, 9000]), learned: (0..m.below(5)).map(|_| m.below(2) as u8).collect() };
        level.add(json!({"p": to(&p)}), json!(p.level()));
        waiting.add(json!({"p": to(&p)}), json!(p.waiting()));
        let who = *m.pick(&["ada", "bors", "gravecaller"]);
        let mut home = crate::rules::home::Home::default();
        if m.chance(80) {
            home.knights.insert(who.to_string(), p.clone());
        }
        let hm = json!({"knights": to(&home.knights)});
        prowess.add(json!({"hm": hm, "who": who}), to(&home.prowess(who)));
        let side = *m.pick(&[0u8, 1, 2]);
        let r = home.learn(who, side).map(|t| json!({"name": t.name, "says": t.says, "effect": t.effect, "does": t.does}));
        let r = match r {
            Ok(t) => json!({"Ok": t}),
            Err(e) => json!({"Err": e}),
        };
        learn.add(json!({"hm": hm, "who": who, "side": side}), json!({"hm": {"knights": to(&home.knights)}, "r": r}));
        let (x, y) = (((m.below(2000) as f32) / 100.0) - 2.0, ((m.below(3000) as f32) / 100.0) - 2.0);
        lectern.add(json!({"x": x, "y": y}), json!(lectern_at(x, y).map_or(9, |s| s)));
    }
    for (_, mut run) in states(3) {
        for round in 0..4 {
            let ids: Vec<u32> = run.players.keys().copied().collect();
            for (n, id) in ids.iter().enumerate() {
                let hero = run.players.get_mut(id).unwrap();
                hero.knight = if m.chance(30) { Some("ada".into()) } else { None };
                hero.name = (*m.pick(&["Ada", "BORS", "Cai"])).into();
                if m.chance(70) {
                    let (x, y) = *m.pick(&[(5.0, 23.0), (7.0, 23.0), (13.0, 23.0), (10.0, 20.0), (30.0, 40.0), (4.0, 22.0), (8.0, 22.9)]);
                    hero.x = x;
                    hero.y = y;
                }
                hero.hp = *m.pick(&[0, 30, 30, 30]);
                hero.stone = m.chance(15);
                hero.buying = *m.pick(&[0, 5, 20, 21, 22]);
                hero.buy_spent = m.chance(25);
                if n == 0 {
                    key.add(json!({"h": keep(&state(&run)["players"][id.to_string()], &["knight", "name"])}), json!(knight_key(&run.players[id])));
                }
            }
            run.greeted = *m.pick(&[0u8, 16, 17, 255]);
            if m.chance(50) {
                run.home.knights.insert("ada".into(), Prowess { xp: 700, learned: vec![] });
            }
            let mut inputs: BTreeMap<u32, Input> = BTreeMap::new();
            for id in &ids {
                if m.chance(80) {
                    inputs.insert(*id, Input { interact: m.chance(70), ..Input::default() });
                }
            }
            let before = view(&run);
            let mut after = run.clone();
            after.tick_lessons(&inputs);
            lessons.add(json!({"run": before, "inputs": to(&inputs)}), view(&after));
            let _ = round;
        }
    }
    eprintln!("talents {} {} {} {} {} {} {}", learn.len(), prowess.len(), level.len(), waiting.len(), key.len(), lectern.len(), lessons.len());
    for c in [&learn, &prowess, &level, &waiting, &key, &lectern, &lessons] {
        c.save();
    }
}

#[test]
#[ignore]
fn record_yard() {
    let mut m = Mix(0x91a2);
    let mut price = Cases::new("yard.stall_price");
    let mut plate = Cases::new("yard.stall_plate");
    for card in crate::rules::cards::Book::builtin().cards.iter() {
        price.add(json!({"card": json!({"rarity": to(&card.rarity)})}), json!(crate::rules::yard::stall_price(card)));
    }
    for _ in 0..120 {
        let (x, y) = (((m.below(5000) as f32) / 100.0) - 1.0, ((m.below(3000) as f32) / 100.0) + 15.0);
        plate.add(json!({"x": x, "y": y}), json!(crate::rules::yard::stall_plate(x, y)));
    }
    for (x, y) in [(34.0, 23.0), (38.0, 22.0), (42.0, 22.0), (36.0, 22.0), (33.9, 22.0), (43.9, 23.9)] {
        plate.add(json!({"x": x, "y": y}), json!(crate::rules::yard::stall_plate(x, y)));
    }
    price.save();
    plate.save();
}
