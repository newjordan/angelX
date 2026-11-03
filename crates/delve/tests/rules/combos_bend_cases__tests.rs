//! Records the deck's bonus and the wish families for the Bend port
//! (`delve/rules/home/cards.bend`, `combos.bend`).
use super::{Family, families, held_families, unlocked};
use crate::rules::cards::{self, Bonus, Card};
use crate::rules::home::bend_cases::{Cases, Mix, to};
use crate::rules::phrasebook;
use serde_json::{Value, json};

fn index(set: &std::collections::BTreeSet<Family>) -> Value {
    json!(set.iter().map(|f| Family::ALL.iter().position(|g| g == f).unwrap()).collect::<Vec<_>>())
}

/// A deck: found cards (copies too), wishes up their ladders, odd hold cards.
fn deck(m: &mut Mix, pool: &[Card], wishes: &[Card]) -> Vec<Card> {
    let mut out = Vec::new();
    for _ in 0..m.below(8) {
        out.push(m.pick(pool).clone());
    }
    for _ in 0..m.below(9) {
        out.push(m.pick(wishes).clone());
    }
    if m.chance(30) {
        let c = out.get(m.below(out.len().max(1) as u64) as usize).cloned();
        out.extend(c);
        out.extend(out.last().cloned());
    }
    out
}

#[test]
#[ignore]
fn record_bonus() {
    let book = cards::Book::builtin();
    let pool: Vec<Card> = book
        .cards
        .iter()
        .filter(|c| matches!(c.kind, cards::Kind::Hold | cards::Kind::Take | cards::Kind::Play))
        .cloned()
        .collect();
    let mut wishes: Vec<Card> = Vec::new();
    for w in phrasebook::book().iter() {
        for t in 0..w.tiers.len() {
            if let Ok(card) = w.card(t) {
                wishes.push(card);
            }
        }
    }
    // Held cards of every word at awkward numbers, so every cap is reached.
    let mut loud: Vec<Card> = Vec::new();
    for (i, word) in ["damage 50", "rate 40", "speed 30", "pierce 2", "shots 3", "armor 2", "vamp 10", "volley 5", "volley 8", "bounce 3", "homing 3", "chain 3", "mend 5", "orbit 3", "burst 50", "fork 3", "fart 3", "size 3", "knock 3", "burn 3", "chill 3", "poison 3", "boomerang 1", "velocity 60"].iter().enumerate() {
        let text = format!("name Loud {i}\nkind hold\nrarity rare\ntext loud\n{word}\n");
        if let Ok(c) = cards::check(&format!("loud-{i}"), &text) {
            loud.push(c.card);
        }
    }
    let mut m = Mix(0xb0a5);
    let mut within = Cases::new("cards.Bonus.within");
    let mut hf = Cases::new("combos.held_families");
    let mut fam = Cases::new("combos.families");
    let mut unl = Cases::new("combos.unlocked");
    for round in 0..260 {
        let mut held = deck(&mut m, if round % 3 == 0 { &loud } else { &pool }, &wishes);
        if round % 4 == 1 {
            held.extend(loud.iter().filter(|_| m.chance(50)).cloned());
        }
        let floor = if m.chance(25) { None } else { Some(m.below(9) as u32) };
        let bonus = Bonus::within(held.iter(), floor);
        within.add(json!({"held": to(&held), "floor": floor}), to(&bonus));
        let set = held_families(held.iter());
        hf.add(json!({"held": to(&held)}), index(&set));
        if let Some(card) = held.first() {
            fam.add(json!({"card": to(card)}), index(&families(card)));
        }
        for word in ["homing", "chain", "burst", "pierce", "fork", "bounce", "orbit", "volley", "damage"] {
            unl.add(json!({"effect": word, "families": index(&set)}), json!(unlocked(word, &set)));
        }
    }
    eprintln!("bonus {} held_families {} families {} unlocked {}", within.len(), hf.len(), fam.len(), unl.len());
    within.save();
    hf.save();
    fam.save();
    unl.save();
}

#[test]
#[ignore]
fn record_caps() {
    let words = ["damage", "rate", "speed", "pierce", "shots", "armor", "vamp", "volley", "bounce", "homing", "chain", "mend", "orbit", "burst", "fork", "fart", "size", "knock", "burn", "chill", "poison", "boomerang", "velocity", "max_hp", "heal", "nova", "cast", "summon", "nothing", ""];
    let mut cap = Cases::new("cards.cap");
    let mut range = Cases::new("cards.range");
    let mut wish_cap = Cases::new("cards.wish_cap");
    let mut wish_most = Cases::new("cards.wish_most");
    for w in words {
        cap.add(json!({"word": w}), json!(cards::cap(w)));
        range.add(json!({"word": w}), json!(cards::range(w)));
        wish_cap.add(json!({"word": w}), json!(cards::wish_cap(w)));
        wish_most.add(json!({"word": w}), json!(cards::wish_most(w)));
    }
    cap.save();
    range.save();
    wish_cap.save();
    wish_most.save();
}
