//! Records the model-job contracts for the Bend port (delve/rules/home):
//! `delve.card-paint`, `delve.brew`, `delve.reforge` and `delve.wish`, each
//! over requests built from the delve's own book and tampered copies, with
//! the answer the rules worker gives. Requests that need the looks, the
//! forge or the phrasebook ports go to their own files.
use crate::rules::home::bend_cases::{Cases, Mix, to, worker};
use crate::rules::{
    Run, brew_jobs, card_paint, cards, cauldron, genie::Asked, home, host_commands::Journal,
    knights::Part, phrasebook, reforge_jobs, wish_jobs,
};
use serde_json::{Value, json};

fn plain(card: &cards::Card) -> bool {
    !matches!(card.arm, Some(cards::Arm::Forged(_))) && card.looks.is_empty() && card.ult.is_none()
}

fn art_rows(m: &mut Mix) -> Vec<String> {
    let inks = ['2', '3', '4', '5', 'h', 'H', 'F', 'D', '.'];
    let n = 3 + m.below(10) as usize;
    (0..n)
        .map(|_| (0..(2 + m.below(14) as usize)).map(|_| *m.pick(&inks)).collect())
        .collect()
}

fn variant(m: &mut Mix, current: &cards::Card, pool: &[cards::Card]) -> cards::Card {
    let mut authored = current.clone();
    match m.below(9) {
        0 => {}
        1 => authored.art = pool[m.below(pool.len() as u64) as usize].art.clone(),
        2 => authored.by = "A painter".into(),
        3 => authored.text.push_str(" Changed."),
        4 => authored.name = format!("{} II", authored.name),
        5 => authored.drop += 1,
        6 => authored.art = art_rows(m),
        7 => authored.kind = cards::Kind::Take,
        _ => {
            authored.art = pool[m.below(pool.len() as u64) as usize].art.clone();
            authored.by = "Painter".into();
        }
    }
    authored
}

#[test]
#[ignore]
fn record_card_paint() {
    let pool: Vec<cards::Card> = cards::Book::builtin().cards.clone();
    let simple: Vec<cards::Card> = pool.iter().filter(|c| plain(c)).cloned().collect();
    let mut m = Mix(0x9a17);
    let mut main = Cases::new("compute.delve.card_paint");
    let mut forged = Cases::new("compute.delve.card_paint.forged");
    for round in 0..160 {
        let n = 1 + m.below(4) as usize;
        let mut cands = Vec::new();
        let mut all_plain = true;
        for _ in 0..n {
            let source = if m.chance(85) { &simple } else { &pool };
            let current = m.pick(source).clone();
            if cands.iter().any(|c: &card_paint::Candidate| c.current.id == current.id) {
                continue;
            }
            all_plain &= plain(&current);
            let mut authored = variant(&mut m, &current, &simple);
            if m.chance(8) {
                authored.id.push('x');
            }
            let mut current = current;
            if m.chance(30) {
                current.by = "Original maker".into();
            }
            cands.push(card_paint::Candidate { current, authored });
        }
        let request = card_paint::Request { candidates: cands };
        let mut input = to(&request);
        if round % 9 == 4 {
            // tampered captures
            match m.below(5) {
                0 => input["candidates"][0]["current"]["id"] = json!("Bad_Id"),
                1 => input["extra"] = json!(1),
                2 => {
                    let first = input["candidates"][0].clone();
                    input["candidates"].as_array_mut().unwrap().push(first);
                }
                3 => input["candidates"][0]["authored"]["name"] = json!("n".repeat(1100)),
                _ => input["candidates"][0]["authored"]["art"] = json!(vec!["2"; 17]),
            }
        }
        let out = worker("card_paint", &input);
        if all_plain { main.add(input, out) } else { forged.add(input, out) }
    }
    eprintln!("card_paint main {} forged {}", main.len(), forged.len());
    main.save();
    forged.save();
}

// ---------------------------------------------------------------------------

const NAMES: &[&str] = &[
    "Frostbriar Orchard", "Thistle Storm", "Cabbage Storm", "Hedge Maze", "Potent Cabbage Storm II",
    "Wildfire", "Moon Pie", "Ash and Ember", "The Imposture", "Grave Chime", "  Spaced Name ", "Q",
    "A Very Long Name That Will Be Cut Down", "Hearth Song",
];
const EFFECTS: &[&[&str]] = &[
    &["cabbages 3", "hedge 1", "ward 1"],
    &["nova 40", "bombs 1"],
    &["cast ring", "ward 2"],
    &["cast bolt"],
    &["heal 30"],
    &["summon 2 swarm"],
    &["summon 3 guard", "cast bolt"],
    &["cabbages 8", "nova 120"],
    &["imposture 1", "cast ring"],
    &["hedge 1"],
    &["cabbages 5"],
    &["nova 25", "heal 20"],
    &["summon 1 turret", "ward 1"],
    &["ward 5", "cast meteor", "bombs 3"],
];
const ARTS: &[&str] = &["..fDDf..\n.fDEEDf.\nfDE44EDf\nfDE44EDf\n.fDEEDf.\n..fDDf..", "22\n22", "...\n...", "2h\nh2\n2h"];

fn spell_text(m: &mut Mix) -> String {
    let name = *m.pick(NAMES);
    let effects = *m.pick(EFFECTS);
    let kind = if m.chance(90) { "spell" } else { "hold" };
    let mut out = format!("name {name}\nkind {kind}\nrarity {}\ntext {}\n", m.pick(&["common", "rare", "relic"]), m.pick(&["Thorns and a cold wind.", "x", "Burns, and hums while it does."]));
    if m.chance(80) {
        out.push_str(&format!("cooldown {}\n", m.pick(&[1, 4, 8, 14, 20, 40, 90, 150])));
    }
    for e in effects {
        out.push_str(e);
        out.push('\n');
    }
    if m.chance(25) {
        out.push_str("look ember star\n");
    }
    if m.chance(85) {
        out.push_str("art\n");
        out.push_str(*m.pick(ARTS));
        out.push('\n');
    }
    out
}

const SPELL_INPUTS: &[&str] = &[
    "cabbage-storm", "hedge-maze", "quickfire", "elixir", "groundslam", "rot", "soul-rip", "storm-hammer",
    "the-imposture", "bone-totem", "raise-ghoul", "miasma-flask", "chain-haul", "brute-force",
];

fn brew_claim(m: &mut Mix) -> (Run, brew_jobs::Claim) {
    let raid = 1 + m.below(60);
    let mut run = Run::at_home(m.below(20), raid, Some("Guest"), home::Home::default(), Default::default());
    run.descend_for_test();
    let a = cauldron::Essence::ALL[m.below(6) as usize];
    let b = cauldron::Essence::ALL[m.below(6) as usize];
    let tier = cauldron::Tier::ALL[m.below(3) as usize];
    let placeholder = cauldron::brewed_card(cauldron::recipe(a, b), tier);
    let knight = 1 + m.below(2) as u32;
    let ask = cauldron::Brewing {
        knight,
        raid: run.raid_id,
        depth: run.dungeon.depth,
        cards: [(*m.pick(SPELL_INPUTS)).into(), (*m.pick(SPELL_INPUTS)).into()],
        placeholder: placeholder.id.clone(),
    };
    let hero = run.players.get_mut(&knight).unwrap();
    hero.spells[0] = Some(placeholder.id.clone());
    hero.bar[0] = Some(cards::BarSlot::Spell(placeholder.id.clone()));
    run.book.insert(placeholder);
    run.brewing.push(ask);
    let mut journal = Journal::new(format!("brew-session-{}", m.below(5)), raid);
    let claim = journal.claim_brew(&mut run).unwrap().unwrap();
    (run, claim)
}

#[test]
#[ignore]
fn record_brew() {
    let mut m = Mix(0xb4e3);
    let mut main = Cases::new("compute.delve.brew");
    let mut looks = Cases::new("compute.delve.brew.looks");
    for round in 0..170 {
        let (run, claim) = brew_claim(&mut m);
        let taken = cauldron::taken(&run.book);
        // The model's answer: a checked card, now and then the very input.
        let card = if m.chance(12) {
            let mut c = claim.work.inputs[m.below(2) as usize].clone();
            if m.chance(50) {
                c.name = format!("{} II", c.name);
            }
            c
        } else {
            let text = spell_text(&mut m);
            match cards::check(&claim.work.result_id, &text) {
                Ok(checked) => checked.card,
                Err(_) => continue,
            }
        };
        let request = brew_jobs::PolicyRequest { work: claim.work.clone(), card, taken };
        let mut input = to(&request);
        if round % 11 == 5 {
            match m.below(6) {
                0 => input["work"]["result_id"] = json!("bad id"),
                1 => input["work"]["player_name"] = json!("p".repeat(1100)),
                2 => input["work"]["ask"]["depth"] = json!(0),
                3 => input["work"]["inputs"][0]["id"] = json!("other"),
                4 => input["extra"] = json!(true),
                _ => input["taken"] = json!((0..4100).map(|i| json!({"id": format!("t{i}"), "name": "n", "mix": []})).collect::<Vec<_>>()),
            }
        }
        let out = worker("brew", &input);
        let sprites = !request.work.inputs.iter().all(plain) || !plain(&request.card);
        if sprites { looks.add(input, out) } else { main.add(input, out) }
    }
    eprintln!("brew main {} looks {}", main.len(), looks.len());
    main.save();
    looks.save();
}

// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn record_reforge() {
    let mut m = Mix(0x4ef0);
    let mut main = Cases::new("compute.delve.reforge");
    let mut forged = Cases::new("compute.delve.reforge.forged");
    for round in 0..160 {
        let mut run = Run::new(m.below(30), 1 + m.below(60), Some("Guest"));
        run.descend_for_test();
        let part = if m.chance(50) { Part::Defense } else { Part::Offense };
        let player = 1 + m.below(2) as u32;
        let mut journal = Journal::new(format!("reforge-session-{}", m.below(7)), run.raid_id);
        let words = *m.pick(&["a silver shield", "a bow that sings", "  ", "x", "heavier"]);
        let Ok(claim) = journal.claim_reforge(&mut run, player, part, words.into()) else { continue };
        let want = if part == Part::Defense { cards::Kind::Guard } else { cards::Kind::Arm };
        let pool: Vec<&cards::Card> = run.book.cards.iter().filter(|c| c.kind == want || m.chance(10)).collect();
        let mut card = (*pool[m.below(pool.len() as u64) as usize]).clone();
        card.id = if m.chance(90) { claim.work.result_id.clone() } else { "other-id".into() };
        if m.chance(60) {
            card.name = format!("{} {}", m.pick(&["Silver", "Old", "Singing"]), m.pick(&["Shield", "Bow", "Aegis"]));
        }
        if m.chance(10) {
            card.text = "t".repeat(60);
        }
        let checked = cards::check(&card.id, &card.definition_file());
        if m.chance(85) {
            if let Ok(c) = checked {
                card = c.card;
            }
        }
        let request = reforge_jobs::PolicyRequest { work: claim.work.clone(), card };
        let mut input = to(&request);
        if round % 10 == 3 {
            match m.below(6) {
                0 => input["work"]["result_id"] = json!("nope"),
                1 => input["work"]["depth"] = json!(0),
                2 => input["work"]["words"] = json!("  "),
                3 => input["work"]["player"] = json!(7),
                4 => input["extra"] = json!(1),
                _ => input["work"]["part"] = json!(if part == Part::Defense { "offense" } else { "defense" }),
            }
        }
        let out = worker("reforge", &input);
        let sprites = !plain(&request.card) || request.work.original.as_ref().is_some_and(|c| !plain(c));
        if sprites { forged.add(input, out) } else { main.add(input, out) }
    }
    eprintln!("reforge main {} forged {}", main.len(), forged.len());
    main.save();
    forged.save();
}

// ---------------------------------------------------------------------------

const WISHES: &[&str] = &[
    "wish captured-grace | Captured Grace\nsay golden grace\ntier damage 10 :: Grace\n",
    "wish iron-hide | Iron Hide\nsay iron hide\nsay skin of iron\ntier armor 1 :: Tough\ntier armor 2; max_hp 10 :: Tougher\n",
    "wish quick-feet | Quick Feet\nkind hold\nsay quick feet\ntier speed 10 :: Fast\ntier speed 20 :: Faster\ntier speed 30 :: Fastest\n",
    "wish ember-lash | Ember Lash\nkind spell\nsay ember lash\ntier cast bolt; cooldown 8 :: A lash of ember\n",
    "wish x | Bad\nsay x\ntier unknownword 5 :: nothing\n",
    "wish empty-ladder | Empty\nsay empty\n",
];

#[test]
#[ignore]
fn record_wish() {
    let mut m = Mix(0x31f5);
    let mut cases = Cases::new("compute.delve.wish");
    let book = phrasebook::book();
    for round in 0..160 {
        let wish = if m.chance(60) {
            m.pick(&book).clone()
        } else {
            let text = *m.pick(WISHES);
            match phrasebook::read_checked(text).0.into_iter().next() {
                Some(w) => w,
                None => phrasebook::read(text).into_iter().next().unwrap_or_else(|| book[0].clone()),
            }
        };
        let ask = Asked {
            knight: 1 + m.below(3) as u32,
            words: (*m.pick(&["golden grace", "a bow", "faster", "  ", "ö"])).into(),
            steps: 1 + m.below(3) as u8,
            great: m.chance(30),
            fills: m.chance(50),
            making: None,
            target: crate::rules::dead_wishes::Target::You,
        };
        let work = wish_jobs::Work {
            ask,
            maker: (*m.pick(&["Original maker", "", "Ada"])).into(),
            binding: format!("wish-session-{}-{:x}-{:x}", m.below(9), m.below(50), m.below(9)),
        };
        let mut input = to(&wish_jobs::PolicyRequest { work, wish });
        if round % 9 == 2 {
            match m.below(6) {
                0 => input["work"]["binding"] = json!("nope"),
                1 => input["work"]["ask"]["steps"] = json!(0),
                2 => input["work"]["ask"]["knight"] = json!(0),
                3 => input["wish"]["id"] = json!("Bad_Id"),
                4 => input["extra"] = json!(1),
                _ => input["wish"]["name"] = json!(" "),
            }
        }
        let out = worker("wish", &input);
        cases.add(input, out);
    }
    eprintln!("wish {}", cases.len());
    cases.save();
    let _: Value = json!(null);
}
