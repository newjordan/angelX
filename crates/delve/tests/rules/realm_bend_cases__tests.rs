//! Records `realm.rs` (`Spoil`, `Spoils`) for the Bend port (`delve/rules/world/realm.bx`).
use super::*;
use crate::rules::layout::bend_cases::{Cases, Mix, to};
use serde_json::json;

fn spoils(mix: &mut Mix) -> Spoils {
    let mut s = Spoils::default();
    for spoil in Spoil::ALL {
        if mix.below(3) != 0 {
            s.add(spoil, [0u32, 1, 2, 3, 7, 20, 150, 4_000_000_000][mix.below(8) as usize]);
        }
    }
    s
}

#[test]
#[ignore]
fn record_spoils() {
    let mut mix = Mix(0x5b01);
    let (mut add, mut merge, mut empty, mut half, mut covers, mut take) = (
        Cases::new("realm.Spoils.add"), Cases::new("realm.Spoils.merge"), Cases::new("realm.Spoils.is_empty"),
        Cases::new("realm.Spoils.half"), Cases::new("realm.Spoils.covers"), Cases::new("realm.Spoils.take"),
    );
    let (mut worth, mut label, mut short, mut word, mut info) = (
        Cases::new("realm.Spoils.worth"), Cases::new("realm.Spoils.label"), Cases::new("realm.Spoils.shortfall"),
        Cases::new("realm.Spoil.from_word"), Cases::new("realm.Spoil.info"),
    );
    for _ in 0..300 {
        let a = spoils(&mut mix);
        let b = spoils(&mut mix);
        let spoil = Spoil::ALL[mix.below(14) as usize];
        let n = [0u32, 1, 5, 4_294_967_295, 100][mix.below(5) as usize];
        let mut after = a.clone();
        after.add(spoil, n);
        add.add(json!({"spoils": to(&a), "spoil": to(&spoil), "n": n}), to(&after));
        let mut merged = a.clone();
        merged.merge(&b);
        merge.add(json!({"spoils": to(&a), "other": to(&b)}), to(&merged));
        empty.add(json!({"spoils": to(&a)}), json!(a.is_empty()));
        half.add(json!({"spoils": to(&a)}), to(&a.half()));
        covers.add(json!({"spoils": to(&a), "price": to(&b)}), json!(a.covers(&b)));
        let mut taken = a.clone();
        taken.take(&b);
        take.add(json!({"spoils": to(&a), "price": to(&b)}), to(&taken));
        worth.add(json!({"spoils": to(&a)}), json!(a.worth()));
        label.add(json!({"spoils": to(&a)}), json!(a.label()));
        short.add(json!({"spoils": to(&a), "price": to(&b)}), json!(a.shortfall(&b)));
    }
    for w in ["gold", "golds", "gems", "bond", "bonds", "hearthstones", "ss", "", "dragon", "scaless", "wax", "waxs", "ore"] {
        word.add(json!({"word": w}), json!(Spoil::from_word(w).map(|s| s.word())));
    }
    for s in Spoil::ALL {
        info.add(json!({"spoil": to(&s)}), json!({"source": s.source(), "worth": s.worth()}));
    }
    for c in [&add, &merge, &empty, &half, &covers, &take, &worth, &label, &short, &word, &info] {
        c.save();
    }
}
