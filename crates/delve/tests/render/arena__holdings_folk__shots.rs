//! The village drawn: the ledger's folk in a steward's hall and at the
//! Court, a feast, a tantrum, a mourning, a seized workshop, a finished
//! masterwork, and the tavern's board.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_folk_shots -- --ignored`.

use super::*;
use crate::realm::Spoils;
use crate::rules::fey::{Masterwork, Seizure};
use crate::rules::holdings::{Built, Deed, Holding, Holdings, ROOM_LABOUR, WALLS};
use crate::rules::home::Home;
use crate::rules::occasions::{Broke, Cause, Dying, Occasion};
use crate::rules::villagers::{Trade, Villager};
use crate::rules::{Input, Pack, Run, RoomKind};
use std::collections::BTreeMap;

fn soul(name: &str, trade: Trade, mood: i8, skill: u8) -> Villager {
    Villager { name: name.into(), trade, mood, hunger: 0, skill, practiced: skill * 6, age: 30, seasons: 0 }
}

fn village() -> Holdings {
    let mut h = Holdings::default();
    h.stores = 22;
    let mut w = Holding {
        key: Deed::Floor(Pack::Crypt).key(),
        rooms: ["taproom", "smithy", "shrine", "stall", "barracks"]
            .iter()
            .map(|id| Built { id: id.to_string(), labour: ROOM_LABOUR })
            .collect(),
        steward: Some("vell".into()),
        walls: WALLS,
        stars: 3,
        ..Holding::default()
    };
    w.rooms.push(Built { id: "greathall".into(), labour: 3 });
    w.folk = vec![
        soul("Berta", Trade::Brewer, 3, 1),
        soul("Col", Trade::Smith, 1, 2),
        Villager { hunger: 1, ..soul("Goda", Trade::Acolyte, -2, 0) },
        soul("Isolde", Trade::Trader, 1, 0),
        soul("Harold", Trade::Guard, 1, 0),
        soul("Jory", Trade::Labourer, 1, 0),
        Villager { age: 7, ..soul("Mabel", Trade::Child, 2, 0) },
        Villager { age: 70, ..soul("Uda", Trade::Brewer, 1, 2) },
    ];
    h.owned.push(w);
    h
}

fn hall(h: Holdings, tick: u64, stand: (f32, f32)) -> Run {
    let home = Home { holdings: h, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    run.dungeon.pack = Pack::Crypt;
    run.descend_for_test();
    run.dare = None;
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Holding).unwrap();
    run.arrive(i, (12.0, 8.0));
    run.enemies.clear();
    run.tick = tick;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (stand.0 * 2.0, stand.1 * 2.0);
    hero.invulnerable = 0;
    run.step(&BTreeMap::new());
    run.found = None;
    run
}

fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, Some(1), true), 0, 0);
    cv
}

fn save(dir: &std::path::Path, name: &str, img: &Img, k: u32) {
    let rgba = img.rgba_scaled(k);
    image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
        .unwrap()
        .save(dir.join(format!("{name}.png")))
        .unwrap();
}

fn press(run: &mut Run, id: u32) {
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(id, Input { interact: true, ..Input::default() })]));
}

/// Every line the picture writes, for a hall as it stands.
fn said(run: &Run) -> Vec<String> {
    let (_, said) = crate::render::overworld::ink::said::read(|| whole(run));
    said.lines.into_iter().map(|l| l.text).collect()
}

#[test]
fn the_folk_drawn_in_the_hall_are_the_ledgers_and_the_hall_changes_with_what_it_is_in_the_middle_of() {
    let plain = whole(&hall(village(), 300, (12.0, 9.2)));
    let mut empty = village();
    empty.owned[0].folk.clear();
    assert_ne!(plain.content_hash(), whole(&hall(empty, 300, (12.0, 9.2))).content_hash(), "the village is drawn");
    // And moves: the same village a little later is somewhere else.
    assert_ne!(plain.content_hash(), whole(&hall(village(), 700, (12.0, 9.2))).content_hash());
    let mut seen = vec![plain.content_hash()];
    for occasion in [
        Occasion::Feast,
        Occasion::Tantrum { who: "Col".into(), cause: Cause::Hunger, broke: Broke::Course(1) },
        Occasion::Tantrum { who: "Col".into(), cause: Cause::NoBed, broke: Broke::Sack },
        Occasion::Mourning { who: "Nyle".into(), trade: Trade::Smith, dying: Dying::Years },
    ] {
        let mut h = village();
        h.owned[0].occasion = Some(occasion.clone());
        let hash = whole(&hall(h, 300, (12.0, 9.2))).content_hash();
        assert!(!seen.contains(&hash), "{occasion:?} looks like something else");
        seen.push(hash);
    }
    let mut h = village();
    h.owned[0].seized = Some(Seizure { who: "Col".into(), trade: Trade::Smith, days: 2 });
    assert!(!seen.contains(&whole(&hall(h, 300, (12.0, 9.2))).content_hash()), "a seized workshop is lit");
}

#[test]
fn the_hall_says_what_the_village_is_in_the_middle_of_and_only_that() {
    assert!(super::holdings_folk::banner(&village().owned[0]).is_none(), "an ordinary day needs no banner");
    let mut h = village();
    h.owned[0].occasion = Some(Occasion::Mourning { who: "Nyle".into(), trade: Trade::Smith, dying: Dying::Raiders });
    let lines = said(&hall(h, 300, (12.0, 9.2)));
    assert!(lines.iter().any(|l| l == "MOURNING NYLE, DEAD TO THE RAIDERS"), "{lines:?}");
    let mut h = village();
    h.owned[0].seized = Some(Seizure { who: "Col".into(), trade: Trade::Smith, days: 2 });
    let lines = said(&hall(h, 300, (12.0, 9.2)));
    assert!(lines.iter().any(|l| l == "COL HAS THE WORKSHOP: 2D LEFT"), "{lines:?}");
    assert!(lines.iter().any(|l| l == "COL HAS IT"), "the seized bay's tag says whose it is: {lines:?}");
    // Its plate promises nothing while it is the craftsman's.
    let at = crate::rules::holdings::bay(1).1;
    let mut run = hall(h_with_seized(), 300, (at.0 as f32 + 1.5, at.1 as f32 + 0.5));
    run.step(&BTreeMap::new());
    let lines = said(&run);
    assert!(lines.iter().any(|l| l == "SEIZED: COL AT THE WORK, 2D"), "{lines:?}");
    assert!(!lines.iter().any(|l| l.starts_with("HOLD E: ") && l.contains("MAIL")), "{lines:?}");
}

fn h_with_seized() -> Holdings {
    let mut h = village();
    h.owned[0].seized = Some(Seizure { who: "Col".into(), trade: Trade::Smith, days: 2 });
    h
}

#[test]
fn a_masterwork_board_reads_its_card_word_for_word_and_who_carries_it() {
    let mut h = village();
    h.owned[0].works.push(Masterwork { maker: "Col".into(), trade: Trade::Smith });
    // Only the maker about, so the knight's beside nobody else.
    h.owned[0].folk.retain(|f| f.name == "Col");
    let held = h.owned[0].clone();
    let curio = held.works[0].curio().unwrap();
    let at = crate::rules::strolls::strollers(&held, 400)[0];
    let mut run = hall(h, 400, (at.x, at.y + 1.1));
    let lines = said(&run);
    assert!(lines.iter().any(|l| *l == curio.name.to_uppercase()), "{lines:?}");
    assert!(lines.iter().any(|l| *l == format!("+ {}", curio.boon)), "{lines:?}");
    assert!(lines.iter().any(|l| *l == format!("- {}", curio.bane)), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("E: TAKE IT")), "{lines:?}");
    press(&mut run, 1);
    let lines = said(&run);
    assert!(lines.iter().any(|l| l == "YOU CARRY IT THIS RUN"), "{lines:?}");
    assert!(!lines.iter().any(|l| l.starts_with("E: TAKE IT")), "{lines:?}");
}

#[test]
fn a_hungry_souls_board_says_hungry_and_the_word_they_gave_is_up_a_moment() {
    let held = village().owned[0].clone();
    let goda = crate::rules::strolls::strollers(&held, 300)[2];
    let mut run = hall(village(), 300, (goda.x, goda.y + 1.1));
    let lines = said(&run);
    assert!(lines.iter().any(|l| l == "GODA · ACOLYTE") && lines.iter().any(|l| l == "HUNGRY"), "{lines:?}");
    press(&mut run, 1);
    let lines = said(&run);
    assert!(lines.iter().any(|l| l.starts_with("GODA THE ACOLYTE: HUNGRY. THE GRANARY IS LOW")), "{lines:?}");
}

#[test]
fn the_taverns_board_pins_a_paper_for_each_tiding() {
    let tavern = |n: usize| {
        let mut h = Holdings::default();
        for k in 0..n {
            h.tell(&format!("news {k}"));
        }
        let home = Home { holdings: h, ..Home::default() };
        let mut r = Run::at_home(41, 1, Some("Friend"), home, Spoils::default());
        r.walk_alehouse();
        r.floor_plates.clear();
        r.step(&BTreeMap::new());
        let mut cv = Img::black(768, 448);
        super::alehouse::figures(&mut cv, &r);
        cv.content_hash()
    };
    let hashes: Vec<u64> = (0..=4).map(tavern).collect();
    for i in 0..hashes.len() {
        for j in i + 1..hashes.len() {
            assert_ne!(hashes[i], hashes[j], "{i} papers vs {j}");
        }
    }
}

/// The Windmill with the bakery built and Berta keeping it, a knight at its ring.
fn bakery(stores: u32, unrest: u32) -> Run {
    let mut h = Holdings::default();
    h.stores = stores;
    h.unrest = unrest;
    let home = Home { holdings: h, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    run.walk_alehouse();
    run.home.countryside.built |= 1;
    run.home.countryside.keepers.insert("bakery".into(), "Berta".into());
    let i = crate::rules::world::room_of(&run.dungeon, RoomKind::Windmill).expect("the windmill");
    run.arrive(i, (6.0, 9.0));
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::new());
    run
}

#[test]
fn a_plots_keeper_is_named_over_their_head_and_greys_with_the_realms_larder() {
    let fed = bakery(12, 0);
    assert!(said(&fed).iter().any(|l| l == "BERTA"), "the keeper's name stands over them: {:?}", said(&fed));
    let (a, b) = (whole(&fed).content_hash(), whole(&bakery(0, 0)).content_hash());
    assert_ne!(a, b, "a bare granary greys the keeper");
    assert_ne!(a, whole(&bakery(12, crate::rules::holdings::RESTLESS)).content_hash(), "so does a restless realm");
    // Nobody built, nobody there.
    let mut empty = bakery(12, 0);
    empty.home.countryside.built = 0;
    assert!(!said(&empty).iter().any(|l| l == "BERTA"));
}

#[test]
#[ignore]
fn write_folk_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir).join("folk");
    std::fs::create_dir_all(&dir).unwrap();

    // An ordinary day: the ledger's folk at their benches, the labourer with
    // timber for the greathall, a child at play, an elder by the fire; a
    // hungry acolyte in grey.
    let run = hall(village(), 300, (12.0, 9.2));
    save(&dir, "a-hall-ordinary", &whole(&run), 2);
    let run = hall(village(), 650, (8.0, 8.4));
    save(&dir, "a2-hall-ordinary-later", &whole(&run), 2);

    // The feast.
    let mut h = village();
    h.owned[0].occasion = Some(Occasion::Feast);
    let run = hall(h, 500, (12.0, 9.4));
    save(&dir, "b-feast", &whole(&run), 2);

    // The tantrum at the forge.
    let mut h = village();
    h.owned[0].occasion = Some(Occasion::Tantrum { who: "Col".into(), cause: Cause::Hunger, broke: Broke::Course(1) });
    h.owned[0].rooms[1].labour = ROOM_LABOUR - 1;
    h.owned[0].folk[1].hunger = 1;
    let run = hall(h, 400, (7.5, 6.2));
    save(&dir, "c-tantrum", &whole(&run), 2);

    // The mourning.
    let mut h = village();
    h.owned[0].occasion = Some(Occasion::Mourning { who: "Nyle".into(), trade: Trade::Smith, dying: Dying::Years });
    let run = hall(h, 150, (12.0, 9.8));
    save(&dir, "d-mourning-a", &whole(&run), 2);
    let mut h = village();
    h.owned[0].occasion = Some(Occasion::Mourning { who: "Nyle".into(), trade: Trade::Smith, dying: Dying::Years });
    let run = hall(h, 300, (12.0, 9.8));
    save(&dir, "d-mourning-b", &whole(&run), 2);

    // The fey at the smithy.
    let mut h = village();
    h.owned[0].seized = Some(Seizure { who: "Col".into(), trade: Trade::Smith, days: 2 });
    let run = hall(h, 400, (7.4, 5.2));
    save(&dir, "e-fey-seized", &whole(&run), 2);

    // The finished masterwork, a knight beside its maker.
    let mut h = village();
    h.owned[0].works.push(Masterwork { maker: "Col".into(), trade: Trade::Smith });
    h.owned[0].works.push(Masterwork { maker: "Berta".into(), trade: Trade::Brewer });
    let held = h.owned[0].clone();
    let run = hall(h.clone(), 400, (8.5, 5.4));
    let at = crate::rules::strolls::strollers(&held, 400)[1];
    let mut run2 = hall(h.clone(), 400, (at.x, at.y + 1.1));
    save(&dir, "f-masterwork-board", &whole(&run2), 2);
    press(&mut run2, 1);
    save(&dir, "f2-masterwork-taken", &whole(&run2), 2);
    save(&dir, "f3-masterwork-taken-seat", &seat_scene(&run2, 768, 448, Some(1)).compose(), 2);
    let _ = run;

    // A soul spoken with.
    let at = crate::rules::strolls::strollers(&village().owned[0], 300)[2];
    let mut run = hall(village(), 300, (at.x, at.y + 1.1));
    save(&dir, "g-speak-board", &whole(&run), 2);
    press(&mut run, 1);
    let img = seat_scene(&run, 768, 448, Some(1)).compose();
    save(&dir, "g2-speak-seat", &img, 2);

    // The Court's cross-section with its folk.
    let court = |h: Holdings, tick: u64| {
        let home = Home { holdings: h, ..Home::default() };
        let mut run = Run::at_home(7, 1, None, home, Spoils::default());
        let i = crate::rules::world::room_of(&run.dungeon, RoomKind::LandOffice).expect("the Court");
        run.arrive(i, (12.0, 12.0));
        run.step(&BTreeMap::new());
        run.tick = tick;
        run
    };
    let run = court(village(), 300);
    save(&dir, "h-court", &whole(&run), 2);
    let mut h = village();
    h.owned[0].occasion = Some(Occasion::Feast);
    h.owned[0].seized = Some(Seizure { who: "Col".into(), trade: Trade::Smith, days: 1 });
    h.owned[0].works.push(Masterwork { maker: "Berta".into(), trade: Trade::Brewer });
    let run = court(h, 300);
    save(&dir, "h2-court-feast", &whole(&run), 2);

    // A plot's keeper at the bakery.
    save(&dir, "j-keeper-bakery", &whole(&bakery(12, 0)), 2);
    save(&dir, "j2-keeper-lean", &whole(&bakery(0, 0)), 2);

    // The Griffin's board with the folk's news pinned on it, and read.
    let tavern = |h: Holdings| {
        let home = Home { holdings: h, ..Home::default() };
        let mut r = Run::at_home(41, 1, Some("Friend"), home, Spoils::default());
        r.walk_alehouse();
        r.floor_plates.clear();
        r.step(&BTreeMap::new());
        r
    };
    let mut h = village();
    h.tell("The granary is full: The Crypt holds a feast, 3 sacks eaten");
    h.tell("Mabel the child has starved at The Crypt, mourned at The Crypt");
    h.tell("Col the smith is seized by a fey mood at The Crypt: the Smithy is theirs for 2 rent days, 4 sacks for the work");
    let mut run = tavern(h);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (3.9 * 2.0, 6.0 * 2.0);
    save(&dir, "i-tavern-board-pinned", &seat_scene(&run, 768, 448, Some(1)).compose(), 2);
    press(&mut run, 1);
    save(&dir, "i2-tavern-board-read", &seat_scene(&run, 768, 448, Some(1)).compose(), 2);
}
