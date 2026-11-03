//! Freedom wishes: the freed genie gives every knight one, it is spent on
//! the safe room or the weapons locker by walking to the place's wishing
//! spot and saying it, the words become one of the place's gifts or a
//! curio, and what is wished stays in the realm from delve to delve.
use super::*;
use crate::rules::home_settlement;
use crate::rules::host_commands::Journal;

fn press(run: &mut Run, id: u32) {
    run.step(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x, y);
}

/// A party of two standing in a floor-two Sanctuary, the window and its
/// signs gone.
fn sanctuary() -> Run {
    let mut run = Run::new(0, 1, Some("Matt"));
    run.descend_for_test();
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    run.window = None;
    run.floor_plates.clear();
    run
}

/// Knight `id` holds `n` freedom wishes, as the genie's thanks would leave
/// them.
fn holds(run: &mut Run, id: u32, n: u8) {
    run.players.get_mut(&id).unwrap().freedom_wishes = n;
    let name = run.players[&id].name.clone();
    run.home.wished.held.insert(name, n);
}

/// Knight `id` walks to the wishing spot, presses E and says `words`.
fn wish_at_spot(run: &mut Run, id: u32, words: &str) -> genie::Offer {
    let (_, at) = run.freedom_spot().expect("the wishing spot stands");
    put(run, id, (at.0 - 1.0, at.1 - 0.5));
    let rubbed = run.lamp.rubbed.get(&id).copied();
    press(run, id);
    assert!(run.players[&id].place_wish.is_some(), "E at the spot begins a freedom wish");
    assert_ne!(run.lamp.rubbed.get(&id).copied(), rubbed, "the wish line opens");
    run.offer_wish(id, words, None).unwrap();
    run.players[&id].offer.clone().expect("the scroll shows what the words became")
}

#[test]
fn freeing_the_genie_gives_every_knight_present_a_freedom_wish() {
    let mut run = Run::new(4, 2, Some("Matt"));
    run.join_seat(3, "Away");
    run.players.get_mut(&3).unwrap().absent = true;
    run.genie.freedom = genie::FREEDOM;
    let hero = run.players.get_mut(&1).unwrap();
    hero.great_wishes = 1;
    hero.wishes = 2;
    run.offer_wish(1, genie::FREE_WORDS, None).unwrap();
    run.choose(1, 0).unwrap();
    assert!(run.genie.free());
    assert_eq!(run.players[&1].freedom_wishes, 1);
    assert_eq!(run.players[&2].freedom_wishes, 1, "a friend standing there is thanked too");
    assert_eq!(run.players[&3].freedom_wishes, 0, "a seat nobody plays is not");
    assert_eq!(run.players[&1].wishes, 2, "no plain wish is touched");
    assert_eq!(run.home.wished.held.get("You"), Some(&1), "the realm keeps it by name");
    assert_eq!(run.home.wished.held.get("Matt"), Some(&1));
    assert!(run.cues.iter().any(|c| c == "freedom_wish"));
    assert!(run.valid_snapshot());
}

#[test]
fn the_words_read_as_one_of_the_places_gifts_or_a_curio() {
    use freedom::{Gift::*, Place::*, read};
    for (words, gift) in [
        ("a roaring hearth", Hearth),
        ("a warm fireplace with logs", Hearth),
        ("I wish for a hot tub", Spring),
        ("a steaming bath", Spring),
        ("a feast of roast boar and ale", Feast),
        ("bread and cheese", Feast),
        ("a war map of the deep", WarMap),
        ("a big soft featherbed", Featherbed),
        ("pillows!", Featherbed),
        ("a fire and a bath", Hearth),
        ("a hot bath full of steam", Spring),
        ("a golden statue of my cat", Curio),
        ("a piece of the moon", Curio),
    ] {
        assert_eq!(read(Safe, words), gift, "safe room: {words:?}");
    }
    for (words, gift) in [
        ("a whetstone", Whetstone),
        ("a sharpening wheel for every blade", Whetstone),
        ("a quiver of arrows", Fletching),
        ("a keg of black powder", PowderKeg),
        ("more bombs", PowderKeg),
        ("a suit of armour", ArmourStand),
        ("helmets for everyone", ArmourStand),
        ("a practice dummy", Dummy),
        ("a hot spring", Curio),
        ("a golden statue of my cat", Curio),
    ] {
        assert_eq!(read(Locker, words), gift, "weapons locker: {words:?}");
    }
    assert_eq!(read(Safe, "A ROARING HEARTH"), read(Safe, "a roaring hearth"), "case is no matter");
    assert_eq!(freedom::plaque_words("  a   very\tsoft  bed \n"), "a very soft bed");
    assert_eq!(freedom::plaque_words(&"x".repeat(200)).chars().count(), freedom::WORDS);
}

#[test]
fn a_freedom_wish_spent_in_the_safe_room_stands_there_and_gives_at_once() {
    let mut run = sanctuary();
    assert!(run.freedom_spot().is_none(), "no brazier until a freedom wish or an upgrade calls for it");
    holds(&mut run, 1, 1);
    let (wishes, great) = (run.players[&1].wishes, run.players[&1].great_wishes);
    let offer = wish_at_spot(&mut run, 1, "a feast of roast boar");
    assert_eq!(offer.choices.len(), 1);
    assert_eq!(offer.choices[0].wish.as_deref(), Some("place:feast"));
    assert_eq!(offer.choices[0].head, "A Feast Table");
    assert_eq!(offer.choices[0].gives, freedom::kind(freedom::Gift::Feast).unwrap().does);
    assert_eq!(run.players[&1].freedom_wishes, 1, "nothing is spent by asking");
    let hand = run.players[&1].hand.len();
    assert_eq!(run.choose(1, 0).unwrap(), "A Feast Table");
    let hero = &run.players[&1];
    assert_eq!(hero.freedom_wishes, 0);
    assert_eq!((hero.wishes, hero.great_wishes), (wishes, great), "a freedom wish spends no other");
    assert!(hero.offer.is_none() && hero.place_wish.is_none());
    assert_eq!(hero.hand.len(), (hand + 1).min(cards::HAND), "the feast's potion, at once");
    assert_eq!(
        run.home.wished.safe,
        [freedom::Upgrade { gift: freedom::Gift::Feast, words: "a feast of roast boar".into(), by: "You".into() }]
    );
    assert!(run.home.wished.held.is_empty(), "the realm's count is spent too");
    let landed = run.freedom.landed.clone().expect("its moment");
    assert_eq!((landed.place, landed.slot, landed.knight), (freedom::Place::Safe, 0, 1));
    assert!(run.freedom_spot().is_some(), "the brazier stays, now the room has a wish in it");
    for _ in 0..freedom::LANDING_TICKS + 2 {
        run.step(&BTreeMap::new());
    }
    assert!(run.freedom.landed.is_none(), "the moment ends");
    assert!(run.valid_snapshot());
}

#[test]
fn a_freedom_wish_spent_at_the_weapons_locker_goes_down_with_every_knight() {
    let mut run = Run::at_home(4, 2, Some("Matt"), Default::default(), Default::default());
    run.walk_alehouse();
    assert_eq!(run.room().kind, RoomKind::Alehouse);
    assert!(run.freedom_spot().is_none(), "the tavern is as it was");
    holds(&mut run, 1, 2);
    let offer = wish_at_spot(&mut run, 1, "a whetstone for every blade");
    assert_eq!(offer.choices[0].wish.as_deref(), Some("place:whetstone"));
    run.choose(1, 0).unwrap();
    assert_eq!(run.players[&1].freedom_wishes, 1);
    assert_eq!(run.home.wished.held.get("You"), Some(&1));
    assert_eq!(run.home.wished.locker[0].gift, freedom::Gift::Whetstone);
    let offer = wish_at_spot(&mut run, 1, "a keg of black powder");
    assert_eq!(offer.choices[0].wish.as_deref(), Some("place:powder_keg"));
    run.choose(1, 0).unwrap();
    assert_eq!(run.home.wished.locker.len(), 2);
    assert_eq!(run.freedom.landed.as_ref().map(|l| l.slot), Some(1));
    let bombs: Vec<u32> = run.players.values().map(|h| h.bombs).collect();
    run.descend_for_test();
    assert!(run.dungeon.depth >= 1);
    for (hero, before) in run.players.values().zip(bombs) {
        assert!(hero.boon.damage >= freedom::WHETSTONE, "the whetstone's edge, the first floor");
        assert!(hero.bombs > before.min(MAX_BOMBS - 1), "the keg's bomb");
    }
}

#[test]
fn the_safe_room_gives_its_gifts_once_a_floor() {
    let with = |gifts: &[freedom::Gift]| {
        let mut run = Run::new(0, 1, Some("Matt"));
        run.home.wished.safe = gifts
            .iter()
            .map(|&gift| freedom::Upgrade { gift, words: "wished".into(), by: "Matt".into() })
            .collect();
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp / 3;
            hero.hand.clear();
            hero.spell_cooldowns = [90; cards::SPELL_SLOTS];
        }
        run.descend_for_test();
        assert_eq!(run.room().kind, RoomKind::Sanctuary);
        run
    };
    use freedom::Gift::*;
    let plain = with(&[]);
    let gifted = with(&[Hearth, Spring, Feast, WarMap, Featherbed]);
    let (a, b) = (&plain.players[&1], &gifted.players[&1]);
    assert_eq!(b.hp, (a.hp + b.max_hp / freedom::HEARTH_SHARE).min(b.max_hp), "the hearth mends a fifth more");
    assert!(b.spell_cooldowns.iter().all(|&c| c == 0), "the spring readies every spell");
    assert!(a.spell_cooldowns.iter().any(|&c| c > 0));
    assert_eq!(b.hand.iter().filter(|c| *c == "potion").count(), 1, "the feast's potion");
    assert!(gifted.freedom.charted && gifted.freedom.rested && gifted.freedom.given);
    assert!(plain.freedom == Default::default(), "nothing wished, nothing written");
    // The war map: every room of the floor on the map.
    let charted = gifted.floor_map(Some(1)).nodes.len();
    let known = plain.floor_map(Some(1)).nodes.len();
    assert!(charted > known, "{charted} of {} charted, {known} known", gifted.dungeon.rooms.len());
    // Out of the door, rested: the featherbed's ward; back in, no second feast.
    let mut run = gifted;
    let door = (0..4).find(|&d| run.dungeon.neighbour(run.at, d).is_some()).unwrap();
    let next = run.dungeon.neighbour(run.at, door).unwrap();
    run.enter(next, Some((door + 2) % 4));
    assert!(run.players[&1].invulnerable >= freedom::RESTED_WARD - 1);
    assert!(!run.freedom.rested, "one ward a floor");
    run.enter(0, Some(door));
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    assert_eq!(run.players[&1].hand.iter().filter(|c| *c == "potion").count(), 1, "once a floor");
}

#[test]
fn a_gift_had_already_or_words_the_place_has_no_gift_for_stand_as_curios() {
    let mut run = sanctuary();
    holds(&mut run, 1, 3);
    let offer = wish_at_spot(&mut run, 1, "a roaring hearth");
    assert_eq!(offer.choices[0].wish.as_deref(), Some("place:hearth"));
    run.choose(1, 0).unwrap();
    let offer = wish_at_spot(&mut run, 1, "another hearth, a bigger one");
    assert_eq!(offer.choices[0].wish.as_deref(), Some("place:curio"), "the hearth burns already");
    run.choose(1, 0).unwrap();
    let offer = wish_at_spot(&mut run, 1, "a golden statue of my cat");
    assert_eq!(offer.choices[0].wish.as_deref(), Some("place:curio"));
    assert_eq!(run.choose(1, 0).unwrap(), "a golden statue of my cat", "a curio is named by its words");
    let gifts: Vec<_> = run.home.wished.safe.iter().map(|u| u.gift).collect();
    assert_eq!(gifts, [freedom::Gift::Hearth, freedom::Gift::Curio, freedom::Gift::Curio]);
    assert_eq!(run.home.wished.safe[2].name(), "a golden statue of my cat");
    assert_eq!(run.home.wished.safe[2].does(), freedom::CURIO_DOES);
}

#[test]
fn a_place_holds_six_wishes_and_a_full_one_keeps_the_freedom_wish() {
    let mut run = sanctuary();
    run.home.wished.safe = (0..freedom::MOST_PER_PLACE)
        .map(|i| freedom::Upgrade { gift: freedom::Gift::Curio, words: format!("curio {i}"), by: "Matt".into() })
        .collect();
    holds(&mut run, 1, 1);
    let (_, at) = run.freedom_spot().unwrap();
    put(&mut run, 1, (at.0 - 1.0, at.1));
    press(&mut run, 1);
    assert!(run.players[&1].place_wish.is_none(), "a full place begins no wish");
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("ALL THE WISHES"), "{said}");
    assert_eq!(run.players[&1].freedom_wishes, 1, "the freedom wish is kept");
    assert!(run.home.wished.valid());
    run.home.wished.safe.push(run.home.wished.safe[0].clone());
    assert!(!run.home.wished.valid(), "a seventh is never a valid realm");
}

#[test]
fn rolling_the_scroll_up_or_walking_out_keeps_the_freedom_wish() {
    let mut run = sanctuary();
    holds(&mut run, 1, 1);
    wish_at_spot(&mut run, 1, "a hot spring");
    assert!(run.drop_offer(1));
    let hero = &run.players[&1];
    assert!(hero.offer.is_none() && hero.place_wish.is_none());
    assert_eq!(hero.freedom_wishes, 1);
    assert!(run.home.wished.safe.is_empty());
    // Begun, then out of the door: the words are the knight's own again.
    wish_at_spot(&mut run, 1, "a hot spring");
    let door = (0..4).find(|&d| run.dungeon.neighbour(run.at, d).is_some()).unwrap();
    let next = run.dungeon.neighbour(run.at, door).unwrap();
    run.enter(next, Some((door + 2) % 4));
    run.step(&BTreeMap::new());
    assert!(run.players[&1].place_wish.is_none() && run.players[&1].offer.is_none());
    assert_eq!(run.players[&1].freedom_wishes, 1);
    // No freedom wish held: the spot says how to earn one.
    let mut run = sanctuary();
    run.genie.freed = Some((0, "Matt".into()));
    let (_, at) = run.freedom_spot().expect("the freed genie's brazier burns");
    put(&mut run, 2, (at.0 - 1.0, at.1));
    press(&mut run, 2);
    assert!(run.players[&2].place_wish.is_none());
    assert!(run.found.as_ref().unwrap().2.contains("SET THE GENIE FREE"));
}

#[test]
fn the_wish_line_opens_for_a_freedom_wish_without_a_plain_one() {
    let mut run = sanctuary();
    for hero in run.players.values_mut() {
        (hero.wishes, hero.great_wishes, hero.fairy_wishes) = (0, 0, 0);
    }
    holds(&mut run, 1, 1);
    assert!(!run.wish_line_opens(1));
    wish_at_spot(&mut run, 1, "a featherbed");
    assert!(run.wish_line_opens(1));
    assert!(run.special_wish_note(1).unwrap().contains("safe room"));
    run.choose(1, 0).unwrap();
    assert_eq!(run.home.wished.safe[0].gift, freedom::Gift::Featherbed);
}

#[test]
fn the_realm_keeps_the_places_and_the_wishes_held_and_hands_them_back() {
    let mut run = sanctuary();
    holds(&mut run, 2, 2);
    wish_at_spot(&mut run, 2, "a war map");
    run.choose(2, 0).unwrap();
    // The host's settlement carries it into the realm, as the privy wall.
    let mut journal = Journal::new("session-a".into(), run.raid_id);
    let claim = journal.claim_home(&mut run).unwrap().expect("the run has home news");
    assert_eq!(claim.transfer.view.wished, run.home.wished);
    let mut home = home::Home::default();
    let mut treasury = crate::realm::Spoils::default();
    let result = home_settlement::settle(
        &mut activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 },
        &claim.transfer,
        "Host",
        false,
    )
    .unwrap();
    assert!(result.changed);
    assert_eq!(result.home.wished.safe[0].gift, freedom::Gift::WarMap);
    assert_eq!(result.home.wished.held.get("Matt"), Some(&1));
    // A new delve from that realm: the war map in the safe room, and the
    // unspent freedom wish back in Matt's hands.
    let next = Run::at_home(5, 3, Some("Matt"), result.home.clone(), Default::default());
    assert_eq!(next.home.wished, result.home.wished);
    assert_eq!(next.players[&2].freedom_wishes, 1);
    assert_eq!(next.players[&1].freedom_wishes, 0);
    // And through a save, whole.
    let saved = serde_json::to_string(&result.home).unwrap();
    assert!(saved.contains("\"wished\""));
    let back: home::Home = serde_json::from_str(&saved).unwrap();
    assert_eq!(back.wished, result.home.wished);
    let saved = serde_json::to_value(&run).unwrap();
    let back: Run = serde_json::from_value(saved).unwrap();
    assert_eq!(back.home.wished, run.home.wished);
    assert_eq!(back.freedom.landed, run.freedom.landed);
    assert_eq!(back.players[&2].freedom_wishes, run.players[&2].freedom_wishes);
}

#[test]
fn an_old_save_without_freedom_wishes_loads_and_writes_unchanged() {
    // A run and a realm from before: none of the new fields are written...
    let run = Run::new(9, 4, Some("Matt"));
    let saved = serde_json::to_string(&run).unwrap();
    for field in ["\"freedom\"", "\"wished\"", "\"freedom_wishes\"", "\"place_wish\""] {
        assert!(!saved.contains(field), "{field} written while unused");
    }
    let home = serde_json::to_string(&home::Home::default()).unwrap();
    assert!(!home.contains("wished"));
    // ...and what was written before reads back and writes the same bytes.
    let back: Run = serde_json::from_str(&saved).unwrap();
    assert_eq!(serde_json::to_string(&back).unwrap(), saved);
    let back: home::Home = serde_json::from_str(&home).unwrap();
    assert_eq!(serde_json::to_string(&back).unwrap(), home);
    // A garbled realm subtree never costs the realm: it reads as nothing.
    let mut garbled: serde_json::Value = serde_json::from_str(&home).unwrap();
    garbled["wished"] = serde_json::json!({"safe": "not a list"});
    let back: home::Home = serde_json::from_value(garbled).unwrap();
    assert!(back.wished.is_empty());
    // A gift a later realm knows reads as a curio.
    let later: freedom::Upgrade =
        serde_json::from_value(serde_json::json!({"gift": "moon_gate", "words": "a moon gate", "by": "Matt"})).unwrap();
    assert_eq!(later.gift, freedom::Gift::Curio);
}

fn save(dir: &std::path::Path, run: &Run, name: &str) {
    let mut img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
    crate::render::genie::paint(&mut img, run, Some(1));
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_freedom_shots -- --ignored`:
/// the genie set free, the safe room's brazier and its prompt, a wish
/// landing, the safe room with its six wishes, the weapons locker before and
/// after, and a wish landing in it.
#[test]
#[ignore]
fn write_freedom_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir).join("freedom");
    std::fs::create_dir_all(&dir).unwrap();
    // The genie set free, a second in, and three.
    let mut free = sanctuary();
    free.genie.freedom = genie::FREEDOM;
    free.players.get_mut(&1).unwrap().great_wishes = 1;
    free.offer_wish(1, genie::FREE_WORDS, None).unwrap();
    free.choose(1, 0).unwrap();
    for (n, ticks) in [("a-freed-1", 12), ("a-freed-2", 40), ("a-freed-3", 60)] {
        for _ in 0..ticks {
            free.step(&BTreeMap::new());
        }
        save(&dir, &free, n);
    }
    for _ in 0..genie::FREED_TICKS {
        free.step(&BTreeMap::new());
    }
    // At the brazier, holding the freedom wish: its prompt.
    let (_, at) = free.freedom_spot().unwrap();
    put(&mut free, 1, (at.0 - 1.6, at.1 - 0.4));
    put(&mut free, 2, (at.0 - 9.0, at.1 - 3.0));
    for _ in 0..8 {
        free.step(&BTreeMap::new());
    }
    save(&dir, &free, "b-brazier-prompt");
    // The wish said: the scroll shows what it became.
    wish_at_spot(&mut free, 1, "a roaring hearth to warm us");
    for _ in 0..6 {
        free.step(&BTreeMap::new());
    }
    save(&dir, &free, "c-scroll");
    free.choose(1, 0).unwrap();
    for (n, ticks) in [("d-landing-1", 6), ("d-landing-2", 24), ("d-landing-3", 40)] {
        for _ in 0..ticks {
            free.step(&BTreeMap::new());
        }
        save(&dir, &free, n);
    }
    // The safe room with all six, a knight reading a plaque.
    let mut full = sanctuary();
    use freedom::Gift::*;
    full.home.wished.safe = [
        (Hearth, "a roaring hearth", "You"),
        (Spring, "a hot spring", "Matt"),
        (Feast, "a feast of roast boar", "You"),
        (WarMap, "a war map of the deep", "Matt"),
        (Featherbed, "a big soft featherbed", "You"),
        (Curio, "a golden statue of my cat", "Matt"),
    ]
    .into_iter()
    .map(|(gift, words, by)| freedom::Upgrade { gift, words: words.into(), by: by.into() })
    .collect();
    let at = freedom::slot_at(full.room(), 0);
    put(&mut full, 1, (at.0 + 0.5, at.1 - 3.6));
    put(&mut full, 2, (at.0 - 14.0, at.1 - 5.0));
    for _ in 0..30 {
        full.step(&BTreeMap::new());
    }
    save(&dir, &full, "e-safe-room-six");
    for _ in 0..7 {
        full.step(&BTreeMap::new());
    }
    save(&dir, &full, "e-safe-room-six-b");
    // The weapons locker: called up by a freedom wish, then filled.
    let mut tavern = Run::at_home(4, 2, Some("Matt"), Default::default(), Default::default());
    tavern.walk_alehouse();
    tavern.floor_plates.clear();
    holds(&mut tavern, 1, 1);
    let spot = (freedom::LOCKER_AT.0 * TILE_UNITS, freedom::LOCKER_AT.1 * TILE_UNITS);
    put(&mut tavern, 1, (spot.0 - 1.2, spot.1 + 1.6));
    put(&mut tavern, 2, (spot.0 - 9.0, spot.1));
    for _ in 0..40 {
        tavern.step(&BTreeMap::new());
    }
    save(&dir, &tavern, "f-locker-empty");
    wish_at_spot(&mut tavern, 1, "a whetstone for every blade");
    tavern.choose(1, 0).unwrap();
    for (n, ticks) in [("g-locker-landing-1", 8), ("g-locker-landing-2", 30)] {
        for _ in 0..ticks {
            tavern.step(&BTreeMap::new());
        }
        save(&dir, &tavern, n);
    }
    tavern.home.wished.locker = [
        (Whetstone, "a whetstone for every blade", "You"),
        (Fletching, "a quiver of fine arrows", "Matt"),
        (PowderKeg, "a keg of black powder", "You"),
        (ArmourStand, "a suit of shining armour", "Matt"),
        (Dummy, "a practice dummy", "You"),
        (Curio, "a stuffed owl", "Matt"),
    ]
    .into_iter()
    .map(|(gift, words, by)| freedom::Upgrade { gift, words: words.into(), by: by.into() })
    .collect();
    tavern.freedom.landed = None;
    for _ in 0..30 {
        tavern.step(&BTreeMap::new());
    }
    save(&dir, &tavern, "h-locker-six");
    for _ in 0..9 {
        tavern.step(&BTreeMap::new());
    }
    save(&dir, &tavern, "h-locker-six-b");
}
