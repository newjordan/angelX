//! The Sozzled Griffin: every delve begins in it, the party together; each
//! knight of the company sits there by the generic name of their calling,
//! and E makes you that knight; the keeper, the fence and the toys answer
//! E; the doors lead to the bazaar and out to the kingdom; an old save
//! still loads.
use super::*;
use crate::rules::{knights, world};

/// A party of `seats` in the tavern, as the cockpit begins a delve.
fn start(seats: u32) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    for id in 3..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.players.retain(|&id, _| id <= seats.max(1));
    r.walk_alehouse();
    r.floor_plates.clear();
    r
}

fn place(r: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = r.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn press(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

/// E pressed once at `at` (a fresh press: E is let go first).
fn tap(r: &mut Run, id: u32, at: (f32, f32)) {
    place(r, id, at);
    r.step(&BTreeMap::new());
    r.step(&press(id));
    r.step(&BTreeMap::new());
}

/// E held at `at` until the hold has bought.
fn hold(r: &mut Run, id: u32, at: (f32, f32)) {
    place(r, id, at);
    r.step(&BTreeMap::new());
    for _ in 0..crate::rules::home::BUY_HOLD + 3 {
        r.step(&press(id));
    }
    r.step(&BTreeMap::new());
}

/// The aisle before a patron: where a knight walks up to them.
fn before(p: &alehouse::Patron) -> (f32, f32) {
    (p.at.0, p.at.1 - 1.0)
}

fn said(r: &Run, id: u32) -> String {
    r.alehouse.said.get(&id).map(|s| s.text.clone()).unwrap_or_default()
}

fn gold(r: &Run, id: u32) -> u32 {
    r.players[&id].carried.get(Spoil::Gold)
}

/// The party stands on plate `at` until it has gone through it.
fn take(r: &mut Run, at: (i32, i32, i32, i32)) {
    let from = r.room().kind;
    r.step(&BTreeMap::new());
    let ids: Vec<u32> = r.players.keys().copied().collect();
    for id in ids {
        place(r, id, (at.0 as f32 + at.2 as f32 / 2.0, at.1 as f32 + at.3 as f32 / 2.0));
    }
    for _ in 0..4 * HZ {
        r.step(&BTreeMap::new());
        if r.room().kind != from {
            break;
        }
    }
    for _ in 0..3 {
        r.step(&BTreeMap::new());
    }
}

#[test]
fn a_delve_begins_in_the_tavern_with_the_whole_party_together() {
    let r = start(4);
    assert_eq!(r.room().kind, RoomKind::Alehouse);
    assert_eq!((r.room().cols, r.room().rows), alehouse::HALL, "a great hall");
    assert!(r.valid_snapshot());
    let spots: Vec<(f32, f32)> = r.players.values().map(|h| (h.x, h.y)).collect();
    assert_eq!(spots.len(), 4);
    for &(x, y) in &spots {
        assert!(r.room().tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor, "standing on open floor");
    }
    let near = spots.iter().all(|&(x, y)| spots.iter().all(|&(a, b)| (x - a).hypot(y - b) < 14.0));
    assert!(near, "together, side by side");
    assert!(r.cues.iter().any(|c| c == "alehouse"));
    assert!(r.kingdom.return_to.is_some(), "the way back is the kingdom's gate");
}

#[test]
fn every_knight_is_handed_a_purse_once_and_a_late_friend_too() {
    let mut r = start(2);
    r.step(&BTreeMap::new());
    let first = gold(&r, 1);
    assert_eq!(first, alehouse::PURSE);
    assert_eq!(gold(&r, 2), alehouse::PURSE);
    for _ in 0..30 {
        r.step(&BTreeMap::new());
    }
    assert_eq!(gold(&r, 1), first, "once");
    r.join_seat(3, "Late");
    r.step(&BTreeMap::new());
    assert_eq!(gold(&r, 3), alehouse::PURSE, "a friend who joins is funded too");
}

#[test]
fn the_company_sits_in_the_tavern_by_calling_and_never_by_name() {
    // Every kit of the company has exactly one seat.
    for kit in knights::COMPANY {
        assert_eq!(alehouse::PATRONS.iter().filter(|p| p.kit == kit.id).count(), 1, "{}", kit.id);
    }
    assert_eq!(alehouse::PATRONS.len(), knights::COMPANY.len());
    for p in &alehouse::PATRONS {
        // A class kit is shown by its class's own name.
        if let Some(class) = classes::CLASSES.iter().find(|c| c.kit == p.kit) {
            assert_eq!(p.label, class.name);
        }
        // No personal names: not a knight's, not a title, not "the X".
        let knight = knights::knight(p.kit).unwrap();
        for text in [p.label, p.pitch] {
            let lower = text.to_lowercase();
            for banned in [knight.name, knight.title, "sir ", "dame "] {
                assert!(!lower.contains(&banned.to_lowercase()), "{text}: {banned}");
            }
            assert!(crate::rules::cards::not_for_work(text).is_none());
        }
        assert!(!p.pitch.is_empty() && p.pitch.len() < 80, "one line of pitch");
        assert!(p.label.chars().next().unwrap().is_uppercase() && !p.label.contains(' '));
    }
    let labels: std::collections::BTreeSet<_> = alehouse::PATRONS.iter().map(|p| p.label).collect();
    assert_eq!(labels.len(), alehouse::PATRONS.len(), "no two alike");
    // Every patron has an open aisle in front of them, a knight's length
    // off, and no two share a seat.
    // The recruits sit in the muster tent and are seated there
    // (`muster_tent`); here, the company in the hall.
    let r = start(1);
    for (i, p) in alehouse::PATRONS.iter().enumerate().filter(|(i, _)| alehouse::patron_room(*i) == RoomKind::Alehouse) {
        let page = (0..COMPANY_PAGES).find(|&page| patron_visible(page, i)).unwrap();
        let (x, y) = p.at;
        assert_eq!(r.room().tile(x as i32, (y - 1.0) as i32), Tile::Floor, "{} has an aisle to walk up", p.label);
        assert!(matches!(alehouse::spot_near_on_page(x * TILE_UNITS, (y - 1.0) * TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(_))));
    }
    for (i, p) in alehouse::PATRONS.iter().enumerate() {
        for (j, q) in alehouse::PATRONS.iter().enumerate().skip(i + 1) {
            if alehouse::patron_room(i) != alehouse::patron_room(j) {
                continue;
            }
            assert!((p.at.0 - q.at.0).hypot(p.at.1 - q.at.1) > 1.2, "{} and {} sit apart", p.label, q.label);
        }
    }
}

#[test]
fn e_at_a_patron_makes_you_that_knight() {
    let mut r = start(2);
    r.step(&BTreeMap::new());
    // The company in the hall; the recruits are picked in the muster tent
    // (`muster_tent`).
    for (i, p) in alehouse::PATRONS.iter().enumerate().filter(|(i, _)| alehouse::patron_room(*i) == RoomKind::Alehouse) {
        // Visit each shared company page, then swap at its visible bench.
        r.alehouse.company_page = (0..COMPANY_PAGES).find(|&page| patron_visible(page, i)).unwrap();
        tap(&mut r, 1, before(p));
        let h = &r.players[&1];
        assert_eq!(h.knight.as_deref(), Some(p.kit), "{} is picked", p.label);
        assert_eq!(h.arm.as_deref(), Some(knights::knight(p.kit).unwrap().arm), "{} keeps its original arm", p.label);
        assert_eq!(h.hp, h.max_hp, "a fresh knight");
        let want = knights::knight(p.kit).unwrap();
        assert_eq!(h.max_hp, want.max_hp + h.home_hp);
        assert_eq!(h.bombs, want.bombs);
        assert!(said(&r, 1).contains(&p.label.to_uppercase()), "{}", said(&r, 1));
        // Seat 2 cannot take the same one: the first to the table keeps them.
        if r.players[&2].knight.as_deref() != Some(p.kit) {
            tap(&mut r, 2, before(p));
            assert_ne!(r.players[&2].knight.as_deref(), Some(p.kit), "{i}: one knight, one player");
            assert!(said(&r, 2).contains("ADVENTURING WITH A FRIEND"), "{}", said(&r, 2));
        }
    }
}

#[test]
fn a_class_kit_takes_its_class_and_its_spells_and_swapping_clears_them() {
    let mut r = start(1);
    let at = |kit: &str| before(alehouse::PATRONS.iter().find(|p| p.kit == kit).unwrap());
    r.alehouse.company_page = 1; // Mage, Healer and Knight share this company page.
    tap(&mut r, 1, at("scryglass"));
    let h = &r.players[&1];
    assert_eq!(h.class.as_deref(), Some("Mage"));
    assert!(crate::rules::weave::weaves(h), "the Mage weaves its spells");
    assert_eq!(h.hand, vec!["thunder-scroll".to_string(), "rime-whistle".to_string()]);
    tap(&mut r, 1, at("loop"));
    let h = &r.players[&1];
    assert_eq!(h.class.as_deref(), Some("Healer"));
    assert!(!crate::rules::weave::weaves(h) && h.weave.is_empty(), "the old class's weave goes");
    assert_eq!(h.hand, vec!["censer".to_string(), "choir-crumb".to_string(), "holy-water".to_string()]);
    // A kit with no class has none.
    tap(&mut r, 1, at("composer"));
    assert_eq!(r.players[&1].class, None);
    assert!(r.valid_snapshot());
    // Swapping gives the old knight back to the table.
    assert!(r.patron_taken(alehouse::PATRONS.iter().position(|p| p.kit == "loop").unwrap(), 2).is_none());
}

#[test]
fn a_shape_does_not_pick_under_the_identity_start() {
    let mut r = start(1);
    r.identity = true;
    r.unmake(1);
    let kit = r.players[&1].knight.clone();
    let archer = alehouse::PATRONS.iter().find(|p| p.kit == "percival").unwrap();
    tap(&mut r, 1, before(archer));
    assert_eq!(r.players[&1].knight, kit, "the table does not make a shape");
    assert!(r.players[&1].shape.is_some());
    assert!(said(&r, 1).contains("WISHES ITSELF"), "{}", said(&r, 1));
}

#[test]
fn the_keeper_sells_a_potion_a_bomb_and_a_second_wind_for_the_purse() {
    let mut r = start(1);
    r.step(&BTreeMap::new());
    r.players.get_mut(&1).unwrap().hand.clear();
    r.players.get_mut(&1).unwrap().bombs = 0;
    let before = gold(&r, 1);
    hold(&mut r, 1, alehouse::TAPS[0]);
    assert_eq!(r.players[&1].hand, vec!["potion".to_string()]);
    assert_eq!(gold(&r, 1), before - alehouse::POURS[0].price);
    hold(&mut r, 1, alehouse::TAPS[1]);
    assert_eq!(r.players[&1].bombs, 1);
    let winds = r.players[&1].winds;
    hold(&mut r, 1, alehouse::TAPS[2]);
    assert_eq!(r.players[&1].winds, winds + 1);
    // A brief tap buys nothing.
    let spent = gold(&r, 1);
    place(&mut r, 1, alehouse::TAPS[0]);
    r.step(&BTreeMap::new());
    r.step(&press(1));
    r.step(&BTreeMap::new());
    assert_eq!(gold(&r, 1), spent, "a stray press buys nothing");
    // And a pauper is told so.
    r.players.get_mut(&1).unwrap().hand.clear();
    r.players.get_mut(&1).unwrap().spend_gold(10_000);
    hold(&mut r, 1, alehouse::TAPS[0]);
    assert!(said(&r, 1).contains("MORE GOLD"), "{}", said(&r, 1));
    assert!(r.players[&1].hand.is_empty());
}

#[test]
fn the_toys_answer_e_and_some_hand_over_a_small_real_thing() {
    // Every toy says something, every time, even when tired of you.
    let mut r = start(1);
    r.step(&BTreeMap::new());
    for &(toy, at) in alehouse::TOYS.iter().chain([(alehouse::Toy::Hearth, alehouse::WARM_AT)].iter()) {
        for _ in 0..4 {
            r.alehouse.said.clear();
            r.step(&BTreeMap::new());
            tap(&mut r, 1, at);
            let line = said(&r, 1);
            assert!(!line.is_empty(), "{toy:?} says nothing");
            assert!(crate::rules::cards::not_for_work(&line).is_none(), "{line}");
        }
    }
    // Across seats, the toys give: some seat finds a coin, a bomb, a wind.
    let mut found = (false, false, false);
    for seed in 0..40u64 {
        let mut r = Run::at_home(seed, seed + 3, None, Default::default(), Default::default());
        r.walk_alehouse();
        r.step(&BTreeMap::new());
        let (g, b, w) = (gold(&r, 1), r.players[&1].bombs, r.players[&1].winds);
        r.players.get_mut(&1).unwrap().bombs = 0;
        for _ in 0..3 {
            tap(&mut r, 1, alehouse::TOYS[0].1);
            r.step(&BTreeMap::new());
        }
        for _ in 0..3 {
            tap(&mut r, 1, alehouse::TOYS[1].1);
        }
        found.0 |= gold(&r, 1) > g;
        found.1 |= r.players[&1].bombs > 0;
        found.2 |= r.players[&1].winds > w;
    }
    assert!(found.0 && found.1 && found.2, "{found:?}");
}

#[test]
fn the_toys_are_deterministic_and_tire_of_a_knight() {
    let play = || {
        let mut r = start(2);
        r.step(&BTreeMap::new());
        for _ in 0..3 {
            tap(&mut r, 1, alehouse::TOYS[1].1);
            tap(&mut r, 2, alehouse::TOYS[0].1);
        }
        serde_json::to_string(&r).unwrap()
    };
    assert_eq!(play(), play(), "the same inputs, the same hall");
    // Darts: three throws, then the cat has them.
    let mut r = start(1);
    r.step(&BTreeMap::new());
    for _ in 0..5 {
        tap(&mut r, 1, alehouse::TOYS[1].1);
    }
    assert_eq!(r.alehouse.tries[&1].darts, 3);
    assert!(said(&r, 1).contains("NO MORE"));
}

#[test]
fn arm_wrestling_stakes_ten_gold_and_pays_twenty_five_to_a_winner() {
    let mut won = false;
    for seed in 0..30u64 {
        let mut r = Run::at_home(seed, 9, None, Default::default(), Default::default());
        r.walk_alehouse();
        r.step(&BTreeMap::new());
        let before = gold(&r, 1);
        tap(&mut r, 1, alehouse::TOYS[2].1);
        let after = gold(&r, 1);
        assert!(after == before - 10 || after == before + 15, "{before} -> {after}");
        won |= after > before;
    }
    assert!(won);
    let mut r = start(1);
    r.step(&BTreeMap::new());
    r.players.get_mut(&1).unwrap().spend_gold(10_000);
    tap(&mut r, 1, alehouse::TOYS[2].1);
    assert!(said(&r, 1).contains("TEN GOLD"));
}

#[test]
fn the_cat_walks_the_hall_on_the_clock() {
    let a = alehouse::cat_at(0);
    let b = alehouse::cat_at(150);
    let c = alehouse::cat_at(75);
    assert!((c.0 - (a.0 + b.0) / 2.0).abs() < 0.05 && (c.1 - (a.1 + b.1) / 2.0).abs() < 0.05);
    let r = start(1);
    for t in (0..1200).step_by(7) {
        let (x, y) = alehouse::cat_at(t);
        assert_eq!(r.room().tile(x as i32, y as i32), Tile::Floor, "the cat at {t} is on the floor");
    }
    assert_eq!(alehouse::cat_at(0), alehouse::cat_at(8 * 150), "she comes round");
}

#[test]
fn the_bazaar_door_leads_to_the_bazaar_and_back_and_the_front_door_to_the_kingdom() {
    let mut r = start(2);
    take(&mut r, alehouse::BAZAAR_DOOR);
    assert_eq!(r.room().kind, RoomKind::Bazaar);
    assert!(r.valid_snapshot());
    for h in r.players.values() {
        let (x, y) = (h.x / TILE_UNITS, h.y / TILE_UNITS);
        assert_eq!(r.room().tile(x as i32, y as i32), Tile::Floor, "arrived on floor at {x},{y}");
    }
    take(&mut r, bazaar::TAVERN_DOOR);
    assert_eq!(r.room().kind, RoomKind::Alehouse, "back in the tavern");
    for h in r.players.values() {
        let (x, y) = (h.x / TILE_UNITS, h.y / TILE_UNITS);
        assert_eq!(r.room().tile(x as i32, y as i32), Tile::Floor, "arrived on floor at {x},{y}");
    }
    // The front door: the kingdom, at the Delve's gate, as every delve began.
    let gate = r.kingdom.return_to.as_ref().map(|b| (b.x, b.y)).unwrap();
    take(&mut r, alehouse::FRONT_DOOR);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    let h = &r.players[&1];
    assert!((h.x - gate.0).hypot(h.y - gate.1) < 6.0, "at the Delve's gate");
    assert!(r.valid_snapshot());
}

#[test]
fn the_bazaars_other_door_also_leads_out_and_the_leave_key_goes_to_the_kingdom() {
    let mut r = start(2);
    take(&mut r, alehouse::BAZAAR_DOOR);
    take(&mut r, bazaar::GATE);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    let mut r = start(1);
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(1, Input { leave_activity: true, ..Default::default() })]));
    r.step(&BTreeMap::new());
    assert_eq!(r.room().kind, RoomKind::Kingdom, "the leave key walks out of the tavern");
}

#[test]
fn the_green_house_in_the_courtyard_is_the_way_back_in() {
    let mut r = start(2);
    let gate = r.dungeon.rooms.iter().position(|room| room.kind == RoomKind::Gate).unwrap();
    r.kingdom.return_to = None;
    r.arrive(gate, world::COURTYARD_ARRIVAL);
    take(&mut r, alehouse::COURTYARD_DOOR);
    assert_eq!(r.room().kind, RoomKind::Alehouse, "into the tavern");
    // With no visit to go back to, the front door comes out before it.
    take(&mut r, alehouse::FRONT_DOOR);
    assert_eq!(r.room().kind, RoomKind::Gate);
    let h = &r.players[&1];
    let (fx, fy) = alehouse::COURTYARD_FRONT;
    assert!((h.x / TILE_UNITS - fx).hypot(h.y / TILE_UNITS - fy) < 3.0, "before the green house");
    assert!(r.valid_snapshot());
}

#[test]
fn an_old_save_without_the_tavern_loads_and_the_world_raises_it() {
    let r = start(2);
    let mut value = serde_json::to_value(&r).unwrap();
    // A run saved before the tavern: no tavern state, no pockets, no tavern rooms,
    // standing in the kingdom.
    let obj = value.as_object_mut().unwrap();
    obj.remove("alehouse");
    for hero in obj["players"].as_object_mut().unwrap().values_mut() {
        hero.as_object_mut().unwrap().remove("pockets");
    }
    let rooms = obj["dungeon"]["rooms"].as_array().unwrap().clone();
    let kept: Vec<_> = rooms
        .iter()
        .filter(|room| !matches!(room["kind"].as_str(), Some("alehouse" | "bazaar")))
        .cloned()
        .collect();
    assert_eq!(kept.len() + 2, rooms.len());
    obj["dungeon"]["rooms"] = serde_json::Value::Array(kept);
    obj["at"] = serde_json::json!(0);
    obj.remove("kingdom");
    let mut old: Run = serde_json::from_value(value).expect("an old save loads");
    assert!(old.alehouse.is_default());
    assert!(old.players.values().all(|h| h.pockets.items.is_empty()));
    assert!(world::room_of(&old.dungeon, RoomKind::Alehouse).is_none());
    assert!(old.valid_snapshot());
    // The world is raised around it as the party walks, tavern and all.
    assert!(world::raise_world(&mut old.dungeon));
    assert!(world::room_of(&old.dungeon, RoomKind::Alehouse).is_some());
    assert!(world::room_of(&old.dungeon, RoomKind::Bazaar).is_some());
    assert!(old.valid_snapshot());
    // And the new fields leave the wire when nothing is in them.
    let fresh = serde_json::to_value(Run::at_home(7, 1, None, Default::default(), Default::default())).unwrap();
    assert!(fresh.get("alehouse").is_none());
    assert!(fresh["players"]["1"].get("pockets").is_none());
}

#[test]
fn a_run_with_the_tavern_round_trips_and_steps_alike_on_both_sides() {
    let mut r = start(3);
    r.step(&BTreeMap::new());
    tap(&mut r, 1, before(&alehouse::PATRONS[3]));
    let json = serde_json::to_string(&r).unwrap();
    let mut back: Run = serde_json::from_str(&json).unwrap();
    assert!(back.valid_snapshot());
    for _ in 0..20 {
        r.step(&BTreeMap::new());
        back.step(&BTreeMap::new());
    }
    assert_eq!(serde_json::to_string(&r).unwrap(), serde_json::to_string(&back).unwrap());
}

#[test]
fn the_tavern_never_starts_a_fight_and_holds_no_foes() {
    let mut r = start(2);
    for _ in 0..300 {
        r.step(&BTreeMap::new());
    }
    assert!(r.enemies.is_empty());
    assert!(r.players.values().all(|h| h.hp == h.max_hp));
    assert!(r.active());
}

#[test]
fn home_lamp_grants_three_once_and_opens_the_existing_wish_line() {
    let mut r = start(2);
    r.step(&BTreeMap::new());
    let before = r.players[&1].wishes;
    hold(&mut r, 1, LAMP_STAND);
    assert_eq!(r.players[&1].wishes, before + 3);
    assert!(r.lamp.rubbed.contains_key(&1), "the client opens its existing wish input");
    assert!(!r.lamp.rubbed.contains_key(&2), "each knight claims separately");
    hold(&mut r, 1, LAMP_STAND);
    assert_eq!(r.players[&1].wishes, before + 3, "rubbing again is not another grant");
    hold(&mut r, 2, LAMP_STAND);
    assert!(r.lamp.rubbed.contains_key(&2));
    assert!(r.valid_snapshot());
}

#[test]
fn home_lamp_claim_survives_patron_repick_spending_and_save() {
    let mut r = start(1);
    hold(&mut r, 1, LAMP_STAND);
    r.pick_patron(1, 0);
    r.players.get_mut(&1).unwrap().wishes = 0;
    let mut r: Run = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    r.alehouse.company_page = 0;
    r.pick_patron(1, 3);
    hold(&mut r, 1, LAMP_STAND);
    assert_eq!(r.players[&1].wishes, 0, "a new patron cannot reclaim spent lamp wishes");
    assert!(said(&r, 1).contains("NO WISHES"));
    assert!(r.valid_snapshot());
}

#[test]
fn home_lamp_respects_wish_caps_and_old_house_defaults() {
    let mut r = start(1);
    r.players.get_mut(&1).unwrap().wishes = crate::rules::overclass::MOST_WISHES - 1;
    hold(&mut r, 1, LAMP_STAND);
    assert_eq!(r.players[&1].wishes, crate::rules::overclass::MOST_WISHES);
    let old: House = serde_json::from_str("{}").unwrap();
    assert_eq!(old, House::default());
    let invalid: House = serde_json::from_value(serde_json::json!({"lamp_claimed":[0,999]})).unwrap();
    assert!(!invalid.valid(), "claim state is bounded and seat ids validated");
}

#[test]
fn home_lamp_wishes_use_both_known_grants_and_the_existing_learner_queue() {
    let mut r = start(1);
    hold(&mut r, 1, LAMP_STAND);
    let wish = crate::rules::phrasebook::get("volley").unwrap();
    r.offer_wish(1, "volley", Some(&wish)).unwrap();
    r.choose(1, 0).unwrap();
    assert_eq!(r.players[&1].wishes, 2);
    assert!(r.players[&1].deck.contains(&wish.card_id(0)));
    r.offer_wish(1, "a moon made of marmalade", None).unwrap();
    r.choose(1, 0).unwrap();
    assert_eq!(r.players[&1].wishes, 1);
    assert_eq!(r.asked.last().unwrap().words, "a moon made of marmalade");
    hold(&mut r, 1, LAMP_STAND);
    assert_eq!(r.players[&1].wishes, 1, "a wish does not reset the lamp claim");
    assert!(r.valid_snapshot());
}

#[test]
fn home_lamp_needs_a_live_present_knight_in_reach_and_resets_only_for_a_new_run() {
    for state in [0, 1, 2] {
        let mut r = start(1);
        let h = r.players.get_mut(&1).unwrap();
        match state {
            0 => h.hp = 0,
            1 => h.absent = true,
            _ => h.stone = true,
        }
        hold(&mut r, 1, LAMP_STAND);
        assert!(r.alehouse.lamp_claimed.is_empty());
    }
    let mut r = start(1);
    tap(&mut r, 1, ARRIVAL);
    assert!(r.alehouse.lamp_claimed.is_empty());
    hold(&mut r, 1, LAMP_STAND);
    assert!(r.alehouse.lamp_claimed.contains(&1));
    assert!(start(1).alehouse.lamp_claimed.is_empty());
}

#[test]
fn home_arms_are_plain_stock_without_relic_or_private_exceptions() {
    use crate::rules::cards::{Kind, Rarity};
    let ranged = ["bow", "crossbow", "handgonne"];
    let private: Vec<_> = PATRONS.iter().map(|p| knights::knight(p.kit).unwrap().arm)
        .filter(|id| !ranged.contains(id)).collect();
    let mut forbidden = Vec::new();
    for card in home_arms() {
        if card.rarity == Rarity::Relic {
            forbidden.push(format!("{}: relic", card.id));
        }
        if private.contains(&card.id.as_str()) {
            forbidden.push(format!("{}: champion/hero-private", card.id));
        }
        assert_eq!(card.kind, Kind::Arm);
        assert!(card.arm.is_some());
        assert!(!card.art.is_empty(), "{} uses the builtin card-art pipeline", card.id);
    }
    assert!(forbidden.is_empty(), "no wall exceptions: {forbidden:?}");
    for card in home_arms().iter().filter(|c| !ranged.contains(&c.id.as_str())) {
        assert_eq!(card.rarity, Rarity::Common, "{} must be genuine common stock", card.id);
    }
}

#[test]
fn home_arms_stock_stats_are_plain_and_weaker_than_the_relic_forms() {
    use crate::rules::cards::{Arm, Book, Rarity};
    let book = Book::builtin();
    for (stock, relic, staff) in [
        ("stock-blade", "cleaver", false),
        ("stock-staff", "sigil-staff", true),
        ("stock-maul", "great-maul", false),
    ] {
        let card = book.get(stock).expect("registered builtin stock arm");
        assert_eq!(card.rarity, Rarity::Common);
        assert!(card.effects.is_empty());
        assert!(card.ult.is_none());
        assert_eq!((card.drop, card.chest), (0, 0), "wall stock does not alter dungeon loot");
        let Some(Arm::Forged(plain)) = &card.arm else { panic!("stock has checked plain runes") };
        let Some(Arm::Forged(champion)) = &book.get(relic).unwrap().arm else { panic!("relic runes") };
        assert!(plain.throw.is_none() && plain.spread.is_none());
        if staff {
            assert!(plain.melee.is_none());
            let (p, c) = (plain.bolt.unwrap(), champion.bolt.unwrap());
            assert_eq!((p.damage, p.speed, p.every, p.range), (8, 36.0, 15, 24.0));
            assert!(p.damage < c.damage && p.range < c.range && p.every >= c.every);
        } else {
            assert!(plain.bolt.is_none());
            let (p, c) = (plain.melee.unwrap(), champion.melee.unwrap());
            let expected = if stock == "stock-blade" { (18, 2.4, 100.0, 18) } else { (26, 2.8, 120.0, 26) };
            assert_eq!((p.damage, p.reach, p.arc, p.every), expected);
            assert!(p.damage < c.damage && p.reach < c.reach && p.arc < c.arc && p.every > c.every);
        }
    }
}

#[test]
fn home_arms_are_exactly_six_basic_wall_choices_and_all_are_reachable() {
    let actual: Vec<_> = home_arms().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(actual, ["bow", "crossbow", "handgonne", "stock-blade", "stock-staff", "stock-maul"]);
    let r = start(1);
    for i in 0..home_arms().len() {
        let at = arm_stand(i);
        assert_eq!(spot_near(at.0 * TILE_UNITS, at.1 * TILE_UNITS, r.tick), Some(Spot::Arm(i)));
        assert_eq!(r.room().tile(at.0 as i32, at.1 as i32), Tile::Floor, "every arm is reachable");
        assert!(at.1 >= 17.0, "weapons belong on a wall, not across the table aisle");
    }
}

#[test]
fn home_arms_hold_equips_every_stock_arm_for_free_without_changing_the_kit() {
    let mut r = start(1);
    r.alehouse.company_page = 0;
    r.pick_patron(1, 3);
    let before = r.players[&1].clone();
    r.step(&BTreeMap::new());
    r.players.get_mut(&1).unwrap().carried = Default::default();
    for (i, card) in home_arms().iter().enumerate() {
        hold(&mut r, 1, arm_stand(i));
        let h = &r.players[&1];
        assert_eq!(h.arm.as_deref(), Some(card.id.as_str()), "{} equips", card.id);
        assert_eq!(h.knight, before.knight);
        assert_eq!(h.class, before.class);
        assert_eq!(h.hand, before.hand);
        assert_eq!(h.spells, before.spells);
        assert_eq!(h.wishes, before.wishes);
        assert_eq!(gold(&r, 1), 0);
        assert_eq!(h.deck, before.deck, "no free deck bonus or pickup temper farming");
    }
    assert!(r.valid_snapshot());
}

#[test]
fn home_arms_tapping_or_changing_target_does_not_equip_and_holding_does_not_temper() {
    let mut r = start(1);
    r.alehouse.company_page = 0;
    r.pick_patron(1, 3);
    let old = r.players[&1].arm.clone();
    let i = home_arms().iter().position(|c| c.id == "bow").unwrap();
    tap(&mut r, 1, arm_stand(i));
    assert_eq!(r.players[&1].arm, old, "a tap is not a hold");
    place(&mut r, 1, arm_stand(i));
    for _ in 0..crate::rules::home::BUY_HOLD - 1 { r.step(&press(1)); }
    let j = home_arms().iter().position(|c| c.id == "crossbow").unwrap();
    place(&mut r, 1, arm_stand(j));
    r.step(&press(1));
    assert_eq!(r.players[&1].arm, old, "each arm needs its own full hold");
    hold(&mut r, 1, arm_stand(i));
    for _ in 0..3 * crate::rules::home::BUY_HOLD { r.step(&press(1)); }
    assert_eq!(r.players[&1].temper, 0, "free arms never temper");
    assert_eq!(r.players[&1].arm.as_deref(), Some("bow"));
}

#[test]
fn home_arms_old_defaults_and_bounded_hold_state_round_trip_mid_hold() {
    let old: House = serde_json::from_str("{}").unwrap();
    assert!(old.arm_holds.is_empty());
    assert!(old.valid());
    for invalid in [serde_json::json!({"arm_holds":{"0":0}}), serde_json::json!({"arm_holds":{"1":999}})] {
        let house: House = serde_json::from_value(invalid).unwrap();
        assert!(!house.valid());
    }
    let mut r = start(1);
    r.alehouse.company_page = 0;
    r.pick_patron(1, 3);
    place(&mut r, 1, arm_stand(0));
    for _ in 0..crate::rules::home::BUY_HOLD / 2 { r.step(&press(1)); }
    let mut saved: Run = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    for _ in 0..crate::rules::home::BUY_HOLD {
        r.step(&press(1));
        saved.step(&press(1));
    }
    assert_eq!(r.players[&1].arm.as_deref(), Some(home_arms()[0].id.as_str()));
    assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::to_value(&saved).unwrap());
    assert!(r.valid_snapshot());
}

#[test]
fn home_arms_have_independent_holds_and_no_shared_stock_or_invalid_claims() {
    let mut r = start(2);
    let slot = home_arms().iter().position(|c| c.id == "stock-maul").unwrap();
    let other = home_arms().iter().position(|c| c.id == "stock-blade").unwrap();
    place(&mut r, 1, arm_stand(slot));
    place(&mut r, 2, arm_stand(other));
    let both = BTreeMap::from([(1, Input { interact: true, ..Default::default() }), (2, Input { interact: true, ..Default::default() })]);
    for _ in 0..crate::rules::home::BUY_HOLD + 2 { r.step(&both); }
    assert_eq!(r.players[&1].arm.as_deref(), Some("stock-maul"));
    assert_eq!(r.players[&2].arm.as_deref(), Some("stock-blade"));
    hold(&mut r, 2, arm_stand(slot));
    assert_eq!(r.players[&2].arm.as_deref(), Some("stock-maul"), "stock never runs out");
    for state in [0, 1, 2, 3] {
        let mut r = start(1);
        let old = r.players[&1].arm.clone();
        let h = r.players.get_mut(&1).unwrap();
        match state { 0 => h.hp = 0, 1 => h.absent = true, 2 => h.stone = true, _ => {} }
        hold(&mut r, 1, if state == 3 { ARRIVAL } else { arm_stand(slot) });
        assert_eq!(r.players[&1].arm, old, "a live present knight must reach the stand");
    }
}

#[test]
fn every_new_hero_has_an_open_selectable_patron_in_the_hall() {
    for kit in heroes::KITS {
        let i = PATRONS.iter().position(|p| p.kit == kit).expect("new hero has a tavern patron");
        let mut r = start(1);
        assert_eq!(r.room().tile(PATRONS[i].at.0.floor() as i32, PATRONS[i].at.1.floor() as i32), Tile::Floor);
        r.alehouse.company_page = (0..COMPANY_PAGES).find(|&page| patron_visible(page, i)).unwrap();
        tap(&mut r, 1, PATRONS[i].at);
        let h = &r.players[&1];
        assert_eq!(h.knight.as_deref(), Some(kit));
        assert_eq!(h.spells[0].as_deref(), Some(knights::knight(kit).unwrap().spells[0]));
    }
}

#[test]
fn home_lamp_requires_a_full_rub_not_a_tap() {
    let mut r = start(1);
    tap(&mut r, 1, LAMP_STAND);
    assert_eq!(r.players[&1].wishes, 0, "a tap does not complete a lamp rub");
    assert!(!r.alehouse.lamp_claimed.contains(&1));
    hold(&mut r, 1, LAMP_STAND);
    assert_eq!(r.players[&1].wishes, LAMP_WISHES);
    assert!(r.alehouse.lamp_claimed.contains(&1));
}

#[test]
fn the_whole_company_sits_in_the_hall_and_every_seat_selects_its_kit() {
    let mut r = start(1);
    let mut seen = std::collections::BTreeSet::new();
    for (i, p) in PATRONS.iter().enumerate() {
        assert!(patron_visible(r.alehouse.company_page, i), "{} sits in the hall", p.kit);
        // The recruits are picked in the muster tent (`muster_tent`).
        if alehouse::patron_room(i) != RoomKind::Alehouse {
            seen.insert(p.kit);
            continue;
        }
        assert_eq!(alehouse::spot_near_in(alehouse::patron_room(i), p.at.0 * TILE_UNITS, p.at.1 * TILE_UNITS, r.tick, r.alehouse.company_page), Some(Spot::Patron(i)));
        tap(&mut r, 1, before(p));
        assert_eq!(r.players[&1].knight.as_deref(), Some(p.kit));
        seen.insert(p.kit);
    }
    assert_eq!(seen.len(), knights::COMPANY.len());
}

#[test]
fn the_notice_board_reads_a_note_and_leaves_the_company_in_the_hall() {
    let mut r = start(2);
    r.pick_patron(1, 0);
    let at = TOYS.iter().find(|(toy, _)| *toy == Toy::Board).unwrap().1;
    hold(&mut r, 2, at);
    assert_eq!(r.alehouse.company_page, 0, "the board turns no page");
    assert!((0..PATRONS.len()).all(|i| patron_visible(r.alehouse.company_page, i)));
    r.pick_patron(1, 1);
    assert_eq!(r.players[&1].knight.as_deref(), Some(PATRONS[1].kit), "every patron can be picked");
    let old = r.players[&1].knight.clone();
    r.pick_patron(1, usize::MAX);
    assert_eq!(r.players[&1].knight, old, "invalid patron indices are ignored");
    // A save from the paged hall still loads, and its page stays bounded.
    let saved = serde_json::to_value(&r.alehouse).unwrap();
    let mut legacy = saved.clone();
    legacy.as_object_mut().unwrap().remove("company_page");
    assert_eq!(serde_json::from_value::<House>(legacy).unwrap().company_page, 0);
    let mut paged: House = serde_json::from_value(saved).unwrap();
    paged.company_page = COMPANY_PAGES - 1;
    assert!(paged.valid());
    paged.company_page = COMPANY_PAGES;
    assert!(!paged.valid(), "replicated page is bounded");
}

#[test]
fn wall_walk_routes_and_simultaneous_six_seat_holds_reach_every_peg() {
    let mut r = start(6);
    // A floor-only flood from arrival proves the pegs are connected, not
    // merely six isolated cells marked Floor.
    let mut reached = std::collections::BTreeSet::new();
    let mut pending = vec![(ARRIVAL.0 as i32, ARRIVAL.1 as i32)];
    while let Some((x, y)) = pending.pop() {
        if !reached.insert((x, y)) { continue; }
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
            let next = (x + dx, y + dy);
            if next.0 >= 0 && next.1 >= 0 && next.0 < HALL.0 as i32 && next.1 < HALL.1 as i32
                && r.room().tile(next.0, next.1) == Tile::Floor && !reached.contains(&next)
            { pending.push(next); }
        }
    }
    for slot in 0..BASIC_ARMS.len() {
        let at = arm_stand(slot);
        assert!(reached.contains(&(at.0 as i32, at.1 as i32)), "no walk route to peg {slot}");
        place(&mut r, slot as u32 + 1, at);
    }
    r.step(&BTreeMap::new());
    let inputs = (1..=6).map(|id| (id, Input { interact: true, ..Default::default() })).collect();
    for _ in 0..BUY_HOLD + 2 { r.step(&inputs); }
    for slot in 0..BASIC_ARMS.len() {
        assert_eq!(r.players[&(slot as u32 + 1)].arm.as_deref(), Some(BASIC_ARMS[slot]));
    }
}

#[test]
fn unused_company_page_preserves_old_serialized_house_and_all_pages_round_trip() {
    let old = serde_json::json!({"funded":0,"tries":{},"said":{},"held":[]});
    assert_eq!(serde_json::to_value(House::default()).unwrap(), old);
    assert_eq!(serde_json::from_value::<House>(old).unwrap(), House::default());
    for page in 0..COMPANY_PAGES {
        let house = House { company_page: page, ..Default::default() };
        let saved = serde_json::to_value(&house).unwrap();
        assert_eq!(saved.get("company_page").is_some(), page != 0);
        assert_eq!(serde_json::from_value::<House>(saved).unwrap(), house);
    }
}

#[test]
fn old_twenty_bay_mid_hold_restores_and_cancels_without_equipping() {
    let mut run = start(1);
    place(&mut run, 1, (7.0 + 19.0 * 1.25, 12.0));
    run.alehouse.arm_holds.insert(1, 19);
    run.players.get_mut(&1).unwrap().buying = crate::rules::home::BUY_HOLD - 1;
    let old_arm = run.players[&1].arm.clone();
    let mut restored: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(restored.valid_snapshot(), "old aisle slot 19 must not make a sound checkpoint invalid");
    restored.step(&press(1));
    assert!(restored.alehouse.arm_holds.is_empty());
    assert_eq!(restored.players[&1].arm, old_arm, "obsolete hold must never grant an unrelated wall arm");
    assert!(restored.valid_snapshot());
}
