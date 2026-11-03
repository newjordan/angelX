//! Records `delve.scroll` for the Bend port (`delve/rules/home/scrolls.bend`):
//! requests captured from varied Runs (and tampered copies) with the answer
//! the rules worker gives.
use crate::rules::home::bend_cases::{Cases, Mix, to, worker};
use crate::rules::{Run, genie, home, identity, phrasebook, scrolls};
use serde_json::{Value, json};

const WORDS: &[&str] = &[
    "golden grace", "a sword of ice", "upgrade cabbage storm", "Upgrade Hedge Maze", "upgrade   nothing here",
    "I wish the genie were free", "free the genie", "let him go", "unshackle my genie friend",
    "make my ghouls bright pink", "my dead army hits harder", "armor of the old kings", "faster feet",
    "  spaced out  ", "wish für ångström", "a bird", "an ogre of iron", "a bow", "a gun", "the great power",
    "x", "bright blade", "mend me", "up", "UPGRADE ", "upgrade",
];

fn world(m: &mut Mix, naked: bool) -> Run {
    let mut run = if naked {
        let mut run = Run::at_home(7, 1 + m.below(40), None, home::Home::default(), Default::default());
        run.naked_start = true;
        run.identity = true;
        run.rename_host("Ada");
        for id in 2..=1 + m.below(3) as u32 {
            run.join_seat(id, ["Bors", "Cai", "Dag", "Ena", "Fen"][id as usize - 2]);
        }
        run
    } else {
        Run::new(m.below(1000), 1 + m.below(50), Some("Guest"))
    };
    run.descend_for_test();
    run.seed = m.next();
    run.tick = m.below(100_000) + if m.chance(10) { 1 << 33 } else { 0 };
    run.genie = genie::Genie {
        freedom: *m.pick(&[0, 10, 60, 99, 100, 130]),
        sealed: m.below(40) as u32,
        last: None,
        freed: m.chance(10).then(|| (m.below(5000), "Ada".to_string())),
    };
    for hero in run.players.values_mut() {
        hero.wishes = *m.pick(&[0, 1, 1, 2, 3, 5]);
        hero.great_wishes = *m.pick(&[0, 0, 1, 1, 2]);
        hero.max_hp = *m.pick(&[1, 2, 20, 90, 100, 180, 300]);
        hero.hp = *m.pick(&[0, 1, hero.max_hp / 2, hero.max_hp, hero.max_hp]).min(&hero.max_hp);
        if m.chance(25) {
            hero.knight = Some("gravecaller".into());
        }
    }
    run
}

fn dress(run: &mut Run, m: &mut Mix) {
    let book = phrasebook::book();
    for _ in 0..m.below(4) {
        let wish = m.pick(&book).clone();
        let steps = 1 + m.below(3);
        for _ in 0..steps {
            let _ = run.grant(1, &wish);
        }
    }
    let hero = run.players.get_mut(&1).unwrap();
    let spells = ["cabbage-storm", "hedge-maze", "firebolt"];
    for (slot, id) in spells.iter().enumerate() {
        if m.chance(50) && run.book.get(id).is_some() {
            hero.spells[slot] = Some((*id).into());
            if m.chance(50) {
                hero.spell_tiers.insert((*id).into(), m.below(3) as u8);
            }
        }
    }
}

fn intents(m: &mut Mix) -> Vec<scrolls::Intent> {
    let book = phrasebook::book();
    let mut out = vec![
        scrolls::Intent::Learn { words: m.pick(WORDS).to_string() },
        scrolls::Intent::Learn { words: m.pick(WORDS).to_string() },
        scrolls::Intent::Known { wish: m.pick(&book).id.clone() },
        scrolls::Intent::Known { wish: m.pick(&book).id.clone() },
    ];
    let w = m.pick(&book).clone();
    out.push(scrolls::Intent::Local { words: m.pick(WORDS).to_string(), known: Some(w.id.clone()) });
    out.push(scrolls::Intent::Local { words: w.name.clone(), known: None });
    out.push(scrolls::Intent::Learn { words: w.name });
    out
}

fn tamper(m: &mut Mix, req: &Value) -> Value {
    let mut v = req.clone();
    match m.below(18) {
        0 => v["facts"]["player"] = json!(9),
        1 => v["facts"]["player"] = json!(0),
        2 => v["said"] = json!("x".repeat(4097)),
        3 => v["heard"] = json!("y".repeat(4100)),
        4 => v["facts"]["deck"] = json!(vec!["card"; 1025]),
        5 => v["facts"]["deck"] = json!(["z".repeat(129)]),
        6 => v["facts"]["spells"] = json!([{"id":"a","name":"A","tier":3}]),
        7 => v["facts"]["spells"] = json!(vec![json!({"id":"a","name":"A","tier":1}); 5]),
        8 => v["whims"] = json!((0..513).map(|i| json!({"id": format!("w{i}"), "name": "W", "tiers": ["t"], "learned": false, "answers": null})).collect::<Vec<_>>()),
        9 => {
            let first = v["whims"].as_array().and_then(|a| a.first().cloned());
            if let Some(f) = first {
                v["whims"].as_array_mut().unwrap().push(f);
            }
        }
        10 => {
            if let Some(w) = v["whims"].as_array_mut().and_then(|a| a.first_mut()) {
                w["learned"] = json!(true);
            }
        }
        11 => {
            if let Some(w) = v["whims"].as_array_mut().and_then(|a| a.first_mut()) {
                w["id"] = json!("Bad_Id");
            }
        }
        12 => v["known"] = json!({"id": "", "name": "n", "tiers": ["t"], "learned": false, "answers": null}),
        13 => v["known"] = json!({"id": "ok", "name": "   ", "tiers": ["t"], "learned": false, "answers": null}),
        14 => v["known"] = json!({"id": "ok", "name": "n", "tiers": [], "learned": false, "answers": null}),
        15 => v["facts"]["previous"] = json!({"said": "s", "great": false, "choices": [], "line": "", "opened": 1}),
        16 => v["extra"] = json!(1),
        _ => {
            v.as_object_mut().unwrap().remove("facts");
        }
    }
    v
}

#[test]
#[ignore]
fn record_scroll() {
    let mut cases = Cases::new("compute.delve.scroll");
    let mut m = Mix(0x5c20_11);
    let mut kinds = [0usize; 3];
    for n in 0..180 {
        let mut run = world(&mut m, n % 4 == 3);
        if n % 4 != 3 || m.chance(50) {
            dress(&mut run, &mut m);
        }
        for intent in intents(&mut m) {
            let Ok(capture) = scrolls::Capture::new(&run, 1, &intent) else { continue };
            let input = to(&capture.request);
            let out = worker("scroll", &input);
            kinds[usize::from(out.get("Err").is_some())] += 1;
            if n % 6 == 0 {
                let bad = tamper(&mut m, &input);
                cases.add(bad.clone(), worker("scroll", &bad));
            }
            cases.add(input, out);
        }
    }
    // The scroll's own error answers and decisions both appear.
    eprintln!("scroll cases {} (ok {} err {})", cases.len(), kinds[0], kinds[1]);
    let _ = identity::Shape::ALL;
    cases.save();
}
