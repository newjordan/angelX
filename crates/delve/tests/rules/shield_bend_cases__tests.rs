//! Records shield.rs and the spell ladders for the Bend port
//! (`delve/rules/home/shield.bendx`, `spell_upgrades.bendx`).
use super::{Wear, entry, meets, turns};
use crate::rules::home::bend_cases::{Cases, Mix, keep, state, states};
use crate::rules::{Hero, Run, cards};
use serde_json::{Value, json};

fn seat(run: &Run, m: &mut Mix) -> u32 {
    let ids: Vec<u32> = run.players.keys().copied().collect();
    ids[m.below(ids.len() as u64) as usize]
}

fn hero_json(run: &Run, id: u32, keys: &[&str]) -> Value {
    keep(&state(run)["players"][id.to_string()], keys)
}

fn float(m: &mut Mix, span: f32) -> f32 {
    if m.chance(8) {
        return 0.0;
    }
    ((m.below(20001) as f32 / 10000.0) - 1.0) * span
}

fn unit(m: &mut Mix) -> (f32, f32) {
    let a = float(m, 3.2);
    (a.cos(), a.sin())
}

#[test]
#[ignore]
fn record_shield() {
    let mut m = Mix(0x5a1e1d);
    let mut turns_c = Cases::new("shield.turns");
    let mut entry_c = Cases::new("shield.entry");
    let mut meets_c = Cases::new("shield.meets");
    let mut turned = Cases::new("shield.Hero.shield_turned");
    let mut rest = Cases::new("shield.Wear.rest");
    let mut left = Cases::new("shield.Wear.left");
    let mut back = Cases::new("shield.Wear.back");
    let mut fresh = Cases::new("shield.Wear.fresh");
    for (_, run) in states(5) {
        for round in 0..6 {
            let mut run = run.clone();
            let id = seat(&run, &mut m);
            {
                let hero = run.players.get_mut(&id).unwrap();
                hero.shielding = m.chance(70);
                let (ax, ay) = unit(&mut m);
                hero.aim_x = ax;
                hero.aim_y = ay;
                hero.max_hp = *m.pick(&[1, 20, 50, 110, 300]);
                hero.guard = if m.chance(70) { cards::Guard::Shield } else { cards::Guard::Roll };
                hero.shield_wear = Wear {
                    worn: if m.chance(50) { m.below(4000) as u32 } else { 0 },
                    calm: if m.chance(30) { m.below(40) as u32 } else { 0 },
                    down: if m.chance(25) { m.below(40) as u32 } else { 0 },
                    turned: m.below(500) as u32,
                };
            }
            let keys = ["shielding", "aim_x", "aim_y", "max_hp", "guard", "shield_wear"];
            let base = hero_json(&run, id, &keys);
            let hero: Hero = run.players[&id].clone();
            // turns / entry / meets
            let (dx, dy) = (float(&mut m, 3.0), float(&mut m, 3.0));
            turns_c.add(json!({"h": base, "dx": dx, "dy": dy}), json!(turns(&hero, (dx, dy))));
            let (a, b, c) = ((float(&mut m, 4.0), float(&mut m, 4.0)), (float(&mut m, 4.0), float(&mut m, 4.0)), (float(&mut m, 2.0), float(&mut m, 2.0)));
            let r = float(&mut m, 1.5).abs();
            entry_c.add(json!({"ax": a.0, "ay": a.1, "bx": b.0, "by": b.1, "cx": c.0, "cy": c.1, "r": r}), json!(entry(a, b, c, r)));
            let body = 0.58 + float(&mut m, 0.3).abs();
            let met = meets(&hero, c, a, b, body).map(|k| json!({"t": k.t, "shield": k.shield, "at": [k.at.0, k.at.1]}));
            meets_c.add(json!({"h": base, "cx": c.0, "cy": c.1, "ax": a.0, "ay": a.1, "bx": b.0, "by": b.1, "body": body}), json!(met));
            // wear
            let damage = *m.pick(&[0, 1, 5, 12, 30, 80, 400]);
            let mut after = run.clone();
            let knocked = after.players.get_mut(&id).unwrap().shield_turned(damage);
            turned.add(json!({"h": base, "damage": damage}), json!({"h": hero_json(&after, id, &keys), "knocked": knocked}));
            let mut after = run.clone();
            let max_hp = after.players[&id].max_hp;
            after.players.get_mut(&id).unwrap().shield_wear.rest(max_hp);
            rest.add(json!({"h": base}), hero_json(&after, id, &keys));
            let w = hero.shield_wear;
            left.add(json!({"h": base}), json!(w.left(hero.max_hp)));
            back.add(json!({"h": base}), json!(w.back()));
            fresh.add(json!({"h": base}), json!(w.fresh()));
            let _ = round;
        }
    }
    eprintln!("shield {} {} {} {} {}", turns_c.len(), entry_c.len(), meets_c.len(), turned.len(), rest.len());
    for c in [&turns_c, &entry_c, &meets_c, &turned, &rest, &left, &back, &fresh] {
        c.save();
    }
}
