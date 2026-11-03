//! Records the spell ladders' Hero side for the Bend port
//! (`delve/rules/home/spell_upgrades.bendx`).
use crate::rules::home::bend_cases::{Cases, Mix, keep, state, states};
use crate::rules::cards;
use serde_json::json;

#[test]
#[ignore]
fn record_spell_ladders() {
    let mut m = Mix(0x7e11);
    let ids = ["raise-ghoul", "barrow-warden", "bone-totem", "cabbage-storm", "hedge-maze", "soul-rip", "wish-ember-1", "nothing"];
    let mut tier = Cases::new("spell_upgrades.Hero.spell_tier");
    let mut power = Cases::new("spell_upgrades.Hero.spell_power");
    let mut step = Cases::new("spell_upgrades.Hero.spell_step");
    let mut wait = Cases::new("spell_upgrades.Hero.spell_wait");
    let mut name = Cases::new("spell_upgrades.Hero.spell_name");
    for (_, mut run) in states(6) {
        let id = *run.players.keys().next().unwrap();
        for round in 0..8 {
            let hero = run.players.get_mut(&id).unwrap();
            hero.spell_tiers.clear();
            for spell in ids {
                if m.chance(50) {
                    hero.spell_tiers.insert(spell.into(), m.below(4) as u8);
                }
            }
            let hero = run.players[&id].clone();
            let h = keep(&state(&run)["players"][id.to_string()], &["spell_tiers"]);
            let spell = *m.pick(&ids);
            tier.add(json!({"h": h, "id": spell}), json!(hero.spell_tier(spell)));
            let base = *m.pick(&[0u32, 1, 2, 7, 14, 40, 90, 120, 121, 500]);
            power.add(json!({"h": h, "id": spell, "base": base}), json!(hero.spell_power(spell, base)));
            let s = hero.spell_step(spell);
            step.add(
                json!({"h": h, "id": spell}),
                json!({"warden_hp": s.warden_hp, "rip_reach": s.rip_reach, "hammer_burst": s.hammer_burst, "slam_reach": s.slam_reach, "fury": s.fury, "rot": s.rot, "rot_reach": s.rot_reach, "haul_reach": s.haul_reach, "quick_reach": s.quick_reach, "quick_life": s.quick_life, "elixir_reach": s.elixir_reach, "cabbage_extra": s.cabbage_extra, "imposture": s.imposture}),
            );
            let card = run.book.cards[m.below(run.book.cards.len() as u64) as usize].clone();
            let mut card = card;
            if m.chance(30) {
                card.cooldown = m.below(130) as u32;
            }
            if card.kind != cards::Kind::Spell {
                card.kind = cards::Kind::Spell;
            }
            wait.add(json!({"h": h, "card": serde_json::to_value(&card).unwrap()}), json!(hero.spell_wait(&card)));
            name.add(json!({"h": h, "card": serde_json::to_value(&card).unwrap()}), json!(hero.spell_name(&card)));
            let _ = round;
        }
    }
    for c in [&tier, &power, &step, &wait, &name] {
        c.save();
    }
}
