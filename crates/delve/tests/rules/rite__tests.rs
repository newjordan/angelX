use super::*;
use crate::rules::creation::{self, Creation, Role, Spell, WeaponCreation, WeaponKind, wish};
use crate::rules::home::DESCEND_HOLD;
use crate::rules::made;

/// A realm's first session as the game begins it with the rite on: the
/// host `name`, kitted from nothing, in the Undercroft.
pub(crate) fn realm_of(name: &str) -> Run {
    realm_with(name, home::Home::default())
}

pub(crate) fn realm_with(name: &str, home: home::Home) -> Run {
    let mut run = Run::at_home(7, 1, None, home, Default::default());
    run.naked_start = true;
    run.rite = true;
    run.rename_host(name);
    run
}

/// What the genie makes of `words`, as the Creator hands it over to be brought to life.
pub(crate) fn wished(words: &str) -> Made {
    match wish::wish_to_spec(words, &[]).make {
        wish::Make::Creature(c) => Made::Creature(c),
        wish::Make::Weapon(w) => Made::Weapon(w),
    }
}

/// A creature of `role` that fits its budget.
pub(crate) fn creature(role: Role, name: &str) -> Made {
    let mut c = Creation { role, name: name.into(), ..Creation::default() };
    if role == Role::Boss {
        c.equip(creation::Slot::Armour, Some("royal-mail")).unwrap();
    }
    Made::Creature(c)
}

/// A rite kept by `knight` as the realm keeps one, whatever seat they sat in.
pub(crate) fn rite_by(knight: &str, made: Made) -> Rite {
    Rite { knight: knight.into(), kind: made.kind(), words: made.summary(), card: String::new(), name: made.name().into(), day: 1, made: Some(made) }
}

pub(crate) fn blade(name: &str) -> Made {
    Made::Weapon(WeaponCreation { name: name.into(), ..WeaponCreation::of_kind(WeaponKind::Blade) })
}

/// The party stands on the Winding Stair long enough to take it.
fn stand_on_the_stair(run: &mut Run) {
    run.idle.clear();
    run.step(&BTreeMap::new());
    let room = run.room();
    let (c, r) = (0..room.rows as i32)
        .flat_map(|r| (0..room.cols as i32).map(move |c| (c, r)))
        .find(|&(c, r)| room.tile(c, r) == Tile::Stairs && world::entrance_at(room.kind, c, r).is_none())
        .expect("the Winding Stair");
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
    }
    for _ in 0..DESCEND_HOLD + 2 {
        run.idle.clear();
        run.step(&BTreeMap::new());
    }
}

#[test]
fn a_new_knight_is_invited_to_the_creator_and_nothing_is_held_for_it() {
    let mut run = realm_of("Ada");
    assert!(run.owes_rite(1), "a name the realm has never seen owes a rite");
    // The bar goes on as ever: no scroll opens, no wish is handed, nothing is asked.
    let hero = &run.players[&1];
    assert!(hero.offer.is_none() && hero.wishes == 0 && run.lamp.rubbed.is_empty());
    // And no stair is shut: the party goes down with the rite still owed.
    stand_on_the_stair(&mut run);
    assert!(run.dungeon.depth >= 1, "the stairs were taken");
    assert!(run.owes_rite(1), "still owed, and never in the way");
    // A newcomer joining a running party is invited the same way and holds no one.
    run.join_seat(2, "Bors");
    assert!(run.owes_rite(2) && run.players[&2].offer.is_none());
}

#[test]
fn a_returning_knight_with_a_rite_on_record_is_not_invited() {
    let mut run = realm_of("Ada");
    run.contribute(1, blade("Oathmark")).unwrap();
    let home = run.home.clone();
    // The next session: Ada again (any case of her name), and Bors new.
    let mut next = realm_with("ada", home);
    assert!(!next.owes_rite(1), "the realm has her rite");
    next.join_seat(2, "Bors");
    assert!(next.owes_rite(2), "a new name owes one");
    // A friend taking Ada's seat under a name with a rite owes nothing.
    next.join_seat(2, "Ada");
    next.step(&BTreeMap::new());
    assert!(!next.owes_rite(2));
}

#[test]
fn a_knight_the_realm_already_knows_came_before_the_rite_and_is_not_asked() {
    let mut home = home::Home::default();
    home.knights.insert("ada".into(), talents::Prowess { xp: 40, ..Default::default() });
    let mut run = realm_with("Ada", home);
    assert!(!run.owes_rite(1), "a returning player is not new");
    run.join_seat(2, "Bors");
    assert!(run.owes_rite(2), "a name the realm has never seen owes one");
    let mut kept = run.home.clone();
    kept.wardrobe.knights.insert("Cai".into(), gear::Kept::equipped());
    let mut next = realm_with("Ada", kept);
    next.join_seat(3, "Cai");
    assert!(!next.owes_rite(3), "kept gear is a record too");
}

#[test]
fn a_bare_run_and_the_off_switch_ask_nobody() {
    let mut run = Run::new(1, 1, None);
    run.rename_host("Ada");
    assert!(!run.owes_rite(1), "bare runs keep it off");
    let mut run = realm_of("Bors");
    run.rite = false;
    assert!(!run.owes_rite(1), "the switch is the run's own");
}

#[test]
fn every_role_is_open_and_each_makes_its_own_kind_in_the_record() {
    let mut run = realm_of("Ada");
    let roles = [
        (blade("Oathmark"), Kind::Weapon),
        (creature(Role::Minion, "Pip"), Kind::Minion),
        (creature(Role::Monster, "Gruk"), Kind::Monster),
        (creature(Role::Boss, "Old Hollow"), Kind::Boss),
        (creature(Role::Hero, "Dame Wren"), Kind::Hero),
        (wished("a barman who stands behind the bar"), Kind::Folk),
    ];
    for (made, kind) in roles {
        assert_eq!(made.kind(), kind);
        let line = run.contribute(1, made).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert!(line.contains("for the realm"), "{line}");
    }
    let kinds: Vec<Kind> = run.home.rites.kept.iter().map(|r| r.kind).collect();
    assert_eq!(kinds, [Kind::Weapon, Kind::Minion, Kind::Monster, Kind::Boss, Kind::Hero, Kind::Folk]);
    assert!(run.home.rites.kept.iter().all(|r| r.knight == "Ada" && r.made.is_some()), "credited to the maker");
    assert!(!run.owes_rite(1), "the first one is the rite");
    assert!(run.home.rites.valid());
    // A seventh is more than one knight may add.
    assert!(run.contribute(1, blade("Again")).is_err());
}

#[test]
fn a_thing_over_its_budget_or_made_by_nobody_is_refused() {
    let mut run = realm_of("Ada");
    let mut c = Creation { role: Role::Minion, name: "Greedy".into(), ..Creation::default() };
    c.equip(creation::Slot::Armour, Some("royal-mail")).unwrap();
    c.spells.push(Spell::default());
    let err = run.contribute(1, Made::Creature(c)).unwrap_err();
    assert!(err.contains("over budget"), "{err}");
    assert!(run.contribute(9, blade("Ghost")).is_err(), "no such knight");
    let weapon_role = Made::Creature(Creation { role: Role::Weapon, ..Creation::default() });
    assert!(run.contribute(1, weapon_role).is_err(), "a weapon is not a creature");
    assert!(run.home.rites.is_empty() && run.owes_rite(1), "nothing was kept, the rite is still owed");
}

#[test]
fn a_maker_is_the_knights_own_name_whatever_the_recipe_says() {
    let mut run = realm_of("Ada");
    let Made::Creature(mut c) = creature(Role::Minion, "Pip") else { unreachable!() };
    c.maker = "Somebody Else".into();
    run.contribute(1, Made::Creature(c)).unwrap();
    assert_eq!(run.home.rites.kept[0].knight, "Ada");
    assert_eq!(run.home.rites.kept[0].creature().unwrap().maker, "Ada");
}

#[test]
fn a_folk_wish_makes_a_world_npc_with_a_post_a_habit_and_a_line() {
    let Made::Creature(c) = wished("a barman who wanders the courtyard and says mind the step") else { panic!("a creature") };
    assert_eq!(c.role, Role::Folk);
    assert_eq!((c.calling.post, c.calling.habit), (creation::Post::Courtyard, creation::Habit::Wanders));
    assert_eq!(c.calling.line, "Mind the step.");
    assert!(c.validate().is_ok() && c.spells.is_empty());
    let Made::Creature(plain) = wished("a blacksmith") else { panic!("a creature") };
    assert_eq!((plain.role, plain.calling.post), (Role::Folk, creation::Post::Griffin));
    assert!(plain.calling.line.contains("steel"), "an occupation has its own line: {}", plain.calling.line);
    // Wishing for a sword after still makes a weapon.
    assert!(matches!(wished("a flaming sword"), Made::Weapon(_)));
}

#[test]
fn a_folk_stands_in_the_griffin_and_says_its_line_on_e() {
    let mut run = realm_of("Ada");
    run.contribute(1, wished("a barman who stands behind the bar and says the ale is cold")).unwrap();
    assert_eq!(run.home.rites.kept[0].creature().unwrap().calling.post, creation::Post::Griffin);
    let here = made::placed(&run, RoomKind::Alehouse, run.tick);
    assert_eq!(here.len(), 1);
    assert!(made::placed(&run, RoomKind::Gate, run.tick).is_empty(), "not in the courtyard");
    assert!(made::placed(&run, RoomKind::Home, run.tick).is_empty(), "nowhere else");
    // A knight beside it who presses E hears its line; one far away does not.
    let alehouse = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Alehouse).expect("the Griffin");
    run.enter(alehouse, None);
    let at = here[0].at;
    let e = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    let put = |run: &mut Run, dx: f32, dy: f32| {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = ((at.0 + dx) * TILE_UNITS, (at.1 + dy) * TILE_UNITS);
    };
    put(&mut run, 8.0, 8.0);
    run.step(&e);
    assert!(run.made.speaking(0, run.tick).is_none(), "too far to hear");
    put(&mut run, 1.0, 0.0);
    run.step(&e);
    assert!(run.made.speaking(0, run.tick).is_some(), "E beside it makes it speak");
    assert_eq!(run.made_line(0).as_deref(), Some("The ale is cold."));
}

#[test]
fn a_wanderer_paces_and_the_world_shows_only_the_newest_of_each_place() {
    let mut run = realm_of("Ada");
    run.contribute(1, wished("a pilgrim who wanders the courtyard")).unwrap();
    let (a, b) = (made::placed(&run, RoomKind::Gate, 0), made::placed(&run, RoomKind::Gate, 40));
    assert!(a[0].walking && a[0].at != b[0].at, "it moves with the tick");
    // Many folk of the Griffin: the newest few show, the record keeps every one.
    let mut run = realm_of("Ada");
    for n in 0..rite::MOST_FOLK + 2 {
        assert!(run.home.rites.record(rite_by(&format!("Maker{n}"), wished("a barman"))));
    }
    assert_eq!(made::placed(&run, RoomKind::Alehouse, 0).len(), rite::MOST_FOLK);
    assert_eq!(run.home.rites.kept.len(), rite::MOST_FOLK + 2);
    let shown: Vec<usize> = made::placed(&run, RoomKind::Alehouse, 0).iter().map(|p| p.rite).collect();
    assert_eq!(shown, [2, 3, 4, 5], "the oldest roll off the display");
}

#[test]
fn a_hero_walks_the_courtyard() {
    let mut run = realm_of("Ada");
    run.contribute(1, creature(Role::Hero, "Dame Wren")).unwrap();
    let here = made::placed(&run, RoomKind::Gate, 0);
    assert_eq!((here.len(), here[0].kind, here[0].walking), (1, Kind::Hero, true));
    assert_eq!(run.made_line(0).as_deref(), Some("Dame Wren of the realm. Made by Ada."));
}

#[test]
fn a_weapon_hangs_on_the_wall_and_lies_in_the_loot_under_its_makers_name() {
    let mut run = realm_of("Ada");
    run.contribute(1, blade("Oathmark")).unwrap();
    let card = run.book.get(&rite::loot_id(0)).expect("its loot copy");
    assert_eq!((card.by.as_str(), card.name.as_str()), ("Ada", "Oathmark"));
    assert!(card.drop > 0 && card.chest > 0);
    // Five pegs on the wall: more weapons take the newest places.
    for n in 0..rite::MOST_WEAPONS {
        assert!(run.home.rites.record(rite_by(&format!("Maker{n}"), blade(&format!("Blade{n}")))));
    }
    run.stock_rites();
    let shown = run.home.rites.newest(Kind::Weapon, rite::MOST_WEAPONS);
    assert_eq!(shown.len(), rite::MOST_WEAPONS);
    assert_eq!(shown[0].0, 1, "the oldest rolled off the wall");
    assert_eq!(run.home.rites.kept.len(), rite::MOST_WEAPONS + 1, "never off the record");
}

#[test]
fn a_minion_turns_up_on_a_delve_floor_as_the_partys_friend_credited_to_its_maker() {
    let mut run = Run::new(3, 3, None);
    run.home.rites.record(Rite {
        knight: "Ada".into(),
        kind: Kind::Minion,
        words: "a little friend".into(),
        card: String::new(),
        name: "Pip".into(),
        day: 1,
        made: Some(creature(Role::Minion, "Pip")),
    });
    assert!(run.hireling.is_none());
    run.enter_for_test(1);
    let friend = run.hireling.as_ref().expect("it turns up");
    let tag = friend.made.as_ref().expect("a made friend");
    assert_eq!((tag.by.as_str(), tag.name.as_str()), ("Ada", "Pip"));
    assert!(friend.hp > 0 && friend.summoner.is_some());
    // It joins once a floor, never twice over.
    let first = friend.made.clone();
    run.enter_for_test(2);
    assert_eq!(run.hireling.as_ref().unwrap().made, first);
}

#[test]
fn a_monster_and_a_boss_join_the_foes_with_their_makers_names_and_bounded_health() {
    let mut run = Run::new(5, 5, None);
    for (role, name) in [(Role::Monster, "Gruk"), (Role::Boss, "Old Hollow")] {
        run.home.rites.record(Rite {
            knight: "Ada".into(),
            kind: Kind::of_role(role),
            words: String::new(),
            card: String::new(),
            name: name.into(),
            day: 1,
            made: Some(creature(role, name)),
        });
    }
    run.dungeon.depth = 3;
    let mut met = Vec::new();
    for index in 1..run.dungeon.rooms.len().min(9) {
        run.enter_for_test(index);
        met.extend(run.enemies.iter().filter_map(|e| e.made.as_ref().map(|t| (t.name.clone(), t.by.clone(), e.hp, e.max_hp))));
    }
    assert!(!met.is_empty(), "a made foe is met on the floor");
    assert!(met.len() <= rite::MOST_FOES, "bounded: {met:?}");
    let cap = 900 * run.party_len().max(1) as u32;
    for (_, by, hp, max) in &met {
        assert_eq!(by, "Ada");
        assert!(*hp > 0 && *max <= cap, "bounded health");
    }
}

#[test]
fn nothing_changes_for_a_realm_with_no_makings() {
    let mut run = Run::new(5, 5, None);
    run.enter_for_test(1);
    assert!(run.hireling.is_none() && run.enemies.iter().all(|e| e.made.is_none()) && run.made.quiet());
}

#[test]
fn the_makings_state_crosses_to_the_rules_worker_and_back() {
    let mut run = realm_of("Ada");
    run.contribute(1, wished("a barman who says hello")).unwrap();
    run.join_seat(2, "Bors");
    run.contribute(2, blade("Oathmark")).unwrap();
    run.made.said.insert(1, (3, 0));
    let live = runtime_state::capture(&run).unwrap();
    let back = runtime_state::restore_value(&live.clone().into_value()).expect("the worker takes it");
    assert_eq!(runtime_state::capture(&back).unwrap().run, live.run);
    assert_eq!(back.home.rites, run.home.rites);
    assert_eq!(back.made, run.made);
    assert!(back.rite && !back.owes_rite(1) && !back.owes_rite(2));
    assert!(back.book.get(&rite::loot_id(1)).is_some());
    // The step the worker takes is the same step.
    let mut stepped = back;
    stepped.step(&BTreeMap::new());
    let again = runtime_state::restore_value(&runtime_state::capture(&stepped).unwrap().into_value()).expect("and back");
    assert_eq!(again.home.rites.kept.len(), 2);
    // A thing brought to life there is kept here: the command crosses with it.
    let mut host = again;
    host.join_seat(3, "Cai");
    host.contribute(3, creature(Role::Minion, "Pip")).unwrap();
    let worker = runtime_state::restore_value(&runtime_state::capture(&host).unwrap().into_value()).unwrap();
    assert_eq!(worker.home.rites.kept.len(), 3);
}

#[test]
fn a_contribute_command_is_checked_and_kept_like_any_other_host_command() {
    use crate::rules::host_commands::Command as GameCommand;
    let made = blade("Oathmark");
    let bad = Made::Creature(Creation { role: Role::Weapon, ..Creation::default() });
    assert!(GameCommand::Contribute { player: 1, made: made.clone() }.validate().is_ok(), "a sealed weapon is a command");
    assert!(GameCommand::Contribute { player: 1, made: bad }.validate().is_err(), "a malformed one never leaves the seat");
    let json = serde_json::to_string(&GameCommand::Contribute { player: 1, made }).unwrap();
    assert!(serde_json::from_str::<GameCommand>(&json).is_ok(), "and it crosses the engine's codec");
}

#[test]
fn a_joined_guest_sees_the_makings() {
    use crate::wire::{Guest, Publisher, Shots, publish, read};
    let mut run = realm_of("Ada");
    run.contribute(1, wished("a barman who stands behind the bar")).unwrap();
    run.join_seat(2, "Bors");
    let seen = read(&publish(&run).unwrap()).expect("the tree reads back");
    assert!(seen.owes_rite(2) && !seen.owes_rite(1));
    assert_eq!(seen.home.rites.kept.len(), 1);
    assert_eq!(seen.home.rites.kept[0].knight, "Ada");
    assert_eq!(made::placed(&seen, RoomKind::Alehouse, 0).len(), 1, "the guest sees the barman stand");
    // And a guest kept up by patches keeps up through a bringing to life.
    let mut publisher = Publisher::default();
    let mut guest = Guest::default();
    let first = publisher.publish(&run).unwrap();
    guest.read(&first, None, &Shots::Rebuild).unwrap();
    run.contribute(2, blade("Oathmark")).unwrap();
    run.step(&BTreeMap::new());
    let next = publisher.publish(&run).unwrap();
    let seen = read(&next).unwrap();
    assert!(!seen.owes_rite(2));
    assert_eq!(seen.home.rites.kept.len(), 2);
    assert_eq!(seen.home.rites.kept[1].weapon().map(|w| w.name.as_str()), Some("Oathmark"));
    assert_eq!(seen.book.get(&rite::loot_id(1)).map(|c| c.by.as_str()), Some("Bors"));
}

#[test]
fn the_realm_settles_the_makings_into_its_record_and_keeps_them_across_a_delivery() {
    let mut run = realm_of("Ada");
    run.contribute(1, blade("Oathmark")).unwrap();
    let transfer = home_settlement::HomeTransfer::capture_and_reserve(&mut run).unwrap();
    let mut home = home::Home::default();
    let mut treasury = crate::realm::Spoils::default();
    let reclaimed = BTreeMap::new();
    let result = home_settlement::settle(
        &mut activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &reclaimed, raids_won: 0 },
        &transfer,
        "host",
        false,
    )
    .unwrap();
    assert!(result.changed);
    assert_eq!(home.rites, run.home.rites, "the realm keeps the making");
    // A second party adds its own to the realm's, never replacing the first.
    let mut other = realm_with("Bors", home.clone());
    other.contribute(1, creature(Role::Minion, "Pip")).unwrap();
    let transfer = home_settlement::HomeTransfer::capture_and_reserve(&mut other).unwrap();
    home_settlement::settle(
        &mut activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &reclaimed, raids_won: 0 },
        &transfer,
        "host",
        false,
    )
    .unwrap();
    let names: Vec<&str> = home.rites.kept.iter().map(|r| r.knight.as_str()).collect();
    assert_eq!(names, ["Ada", "Bors"]);
}

#[test]
fn an_old_save_without_the_field_loads_and_a_bad_one_resets_only_the_record() {
    let mut json = serde_json::to_value(&home::Home::default()).unwrap();
    assert!(json.get("rites").is_none(), "an empty record is left out: saves are as they were");
    let home: home::Home = serde_json::from_value(json.clone()).unwrap();
    assert!(home.rites.is_empty());
    json["rites"] = serde_json::json!({"kept": "not a list"});
    let home: home::Home = serde_json::from_value(json.clone()).unwrap();
    assert!(home.rites.is_empty(), "a bad subtree never costs the realm");
    // A rite made at the wish scroll this morning: a card and no recipe, two kinds only.
    json["rites"] = serde_json::json!({"kept": [{"knight": "Ada", "kind": "minion", "card": "wish-plain-hound-1"}]});
    let home: home::Home = serde_json::from_value(json).unwrap();
    assert_eq!(home.rites.kept[0].words, "");
    assert!(home.rites.has("ada") && home.rites.valid() && home.rites.kept[0].made.is_none());
    // A made thing that is not of the kind it is kept as cannot sit in a save.
    let mut bad = home.rites.clone();
    bad.kept[0].made = Some(creature(Role::Boss, "Wrong kind"));
    assert!(!bad.valid(), "a recipe must be of the kind it is kept as");
    // A run saved before the rite has no flag and owes nothing; one saved owing reads back owing.
    let run = realm_of("Ada");
    let saved: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(saved.rite && saved.owes_rite(1) && saved.valid_snapshot());
    let mut old = serde_json::to_value(&run).unwrap();
    old.as_object_mut().unwrap().remove("rite");
    old["players"]["1"].as_object_mut().unwrap().remove("rite_owed");
    let old: Run = serde_json::from_value(old).unwrap();
    assert!(!old.rite && !old.owes_rite(1) && old.made.quiet());
}
