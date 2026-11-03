//! Records `loot::Rng` for the Bend port (`delve/rules/combat/loot.bend`).
use super::bend_cases::{Cases, Mix};
use super::Rng;
use serde_json::json;

fn states() -> Vec<u64> {
    let mut mix = Mix(0x5eed);
    let mut out: Vec<u64> = vec![0, 1, 2, u64::MAX, 1 << 32, (1 << 32) - 1, 0x9e37_79b9_7f4a_7c15];
    out.extend((0..90).map(|_| mix.next()));
    out
}

#[test]
#[ignore]
fn record_rng() {
    let mut mix = Mix(11);
    let mut new = Cases::new("loot.Rng.new");
    let mut next = Cases::new("loot.Rng.next");
    let mut below = Cases::new("loot.Rng.below");
    let mut chance = Cases::new("loot.Rng.chance");
    let mut pick = Cases::new("loot.Rng.pick");
    for seed in states() {
        new.add(json!({"seed": seed}), json!(serde_json::to_value(Rng::new(seed)).unwrap()));
        let mut r = Rng::new(seed);
        let before = serde_json::to_value(&r).unwrap();
        let out = r.next();
        next.add(json!({"rng": before}), json!({"rng": serde_json::to_value(&r).unwrap(), "out": out}));
        for n in [0usize, 1, 2, 3, 7, 100, 1000, 65_536, 4_000_000_000] {
            let mut r = Rng::new(seed ^ n as u64);
            let before = serde_json::to_value(&r).unwrap();
            let v = r.below(n);
            below.add(json!({"rng": before, "n": n}), json!({"rng": serde_json::to_value(&r).unwrap(), "v": v}));
        }
        for percent in [0u32, 1, 15, 50, 99, 100, 120] {
            let mut r = Rng::new(seed.rotate_left(percent));
            let before = serde_json::to_value(&r).unwrap();
            let hit = r.chance(percent);
            chance.add(json!({"rng": before, "percent": percent}), json!({"rng": serde_json::to_value(&r).unwrap(), "hit": hit}));
        }
        let count = 1 + mix.below(8) as usize;
        let weights: Vec<u32> = (0..count).map(|_| mix.below(40) as u32).collect();
        let items: Vec<(u32, u32)> = weights.iter().enumerate().map(|(i, w)| (i as u32, *w)).collect();
        if weights.iter().sum::<u32>() > 0 {
            let mut r = Rng::new(seed);
            let before = serde_json::to_value(&r).unwrap();
            let got = r.pick(&items);
            pick.add(
                json!({"rng": before, "weights": weights}),
                json!({"rng": serde_json::to_value(&r).unwrap(), "index": got}),
            );
        }
    }
    for c in [&new, &next, &below, &chance, &pick] {
        c.save();
    }
}
