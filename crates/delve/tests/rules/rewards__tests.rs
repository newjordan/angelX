//! The steps up and the road ahead: a level, a tier, a rung, a record and a
//! relic each cheered once; a knight's moments one after another; the
//! ledger of a delve; the realm's sky lit and its gift handed out; what is
//! next for a knight and for the party; and nothing new written for a run
//! or a realm that has none of it.
use super::*;
use crate::rules::{activity, home, home_settlement, host_commands::Journal};

fn steps(run: &Run) -> Vec<(u32, Kind)> {
    run.rewards.steps.iter().map(|s| (s.knight, s.kind.clone())).collect()
}

fn tick(run: &mut Run, n: u32) {
    for _ in 0..n {
        run.step(&BTreeMap::new());
    }
}

/// Two knights in a cleared fight room of the first floor, looked at once.
fn party() -> Run {
    let mut run = Run::new(3, 1, Some("Ada"));
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap();
    run.enter_for_test(room);
    run.clear_for_test();
    run.floor_plates.clear();
    // What the clearing fell for is left out: each test marks its own.
    run.marks.clear();
    tick(&mut run, 1);
    run.rewards.steps.clear();
    run.ledger.earned.clear();
    run
}

#[test]
fn a_level_reached_is_cheered_once_and_the_realm_settling_it_cheers_nothing() {
    let mut run = party();
    assert!(steps(&run).is_empty());
    run.marks.insert("xp".into(), talents::LEVELS[0]);
    tick(&mut run, 1);
    assert_eq!(steps(&run), [(1, Kind::Level { level: 2 }), (2, Kind::Level { level: 2 })]);
    tick(&mut run, 5);
    assert_eq!(run.rewards.steps.len(), 2, "once");
    // The realm takes the marks and hands its home back: the same level.
    let key = talents::knight_key(&run.players[&1]);
    run.marks.remove("xp");
    run.home.knights.insert(key, talents::Prowess { xp: talents::LEVELS[0], learned: Vec::new() });
    let ada = talents::knight_key(&run.players[&2]);
    run.home.knights.insert(ada, talents::Prowess { xp: talents::LEVELS[0], learned: Vec::new() });
    tick(&mut run, 1);
    assert_eq!(run.rewards.steps.len(), 2, "the settled level is the same level");
    let earned = &run.ledger.earned[&1];
    assert_eq!((earned.from_level, earned.level, earned.xp), (1, 2, talents::LEVELS[0]));
    // It plays its length and is let go.
    tick(&mut run, 90);
    assert!(run.rewards.steps.is_empty());
}

#[test]
fn a_tier_climbed_is_a_step_and_a_super_wish_is_one_step_of_two_tiers() {
    let mut run = party();
    let volley = phrasebook::get("volley").unwrap();
    run.players.get_mut(&1).unwrap().wishes = 3;
    run.wish(1, &volley).unwrap();
    assert!(steps(&run).is_empty(), "a new wish is the grant's own moment");
    run.wish(1, &volley).unwrap();
    assert_eq!(steps(&run), [(1, Kind::Tier { wish: volley.name.clone(), from: 0, tier: 1 })]);
    tick(&mut run, 80);
    assert!(steps(&run).is_empty());
    // A super wish: two steps of the ladder at once, one moment.
    let seeker = phrasebook::get("seeker").unwrap();
    run.wish(1, &seeker).unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.wishes, hero.great_wishes) = (0, 1);
    run.wish(1, &seeker).unwrap();
    assert_eq!(steps(&run), [(1, Kind::Tier { wish: seeker.name.clone(), from: 0, tier: 2 })]);
    let earned = &run.ledger.earned[&1];
    assert_eq!(earned.wishes, 5);
    assert_eq!(earned.best, Some((seeker.name.clone(), 2)));
}

#[test]
fn a_knights_moments_play_one_after_another() {
    let mut run = party();
    run.marks.insert("xp".into(), talents::LEVELS[0]);
    run.players.get_mut(&1).unwrap().wishes = 2;
    let volley = phrasebook::get("volley").unwrap();
    run.wish(1, &volley).unwrap();
    run.wish(1, &volley).unwrap();
    tick(&mut run, 1);
    let mine: Vec<&Step> = run.rewards.steps.iter().filter(|s| s.knight == 1).collect();
    assert_eq!(mine.len(), 2);
    assert!(mine[1].at >= mine[0].at + 30, "the level waits for the tier's front: {} then {}", mine[0].at, mine[1].at);
    assert!(run.rewards.steps.iter().find(|s| s.knight == 2).unwrap().at < mine[1].at, "a friend's plays at once");
}

#[test]
fn a_floor_deeper_than_the_realm_has_been_is_a_record_and_each_knight_climbs_a_rung() {
    let mut run = party();
    run.home.deepest = 2;
    tick(&mut run, 1);
    run.descend_for_test();
    tick(&mut run, 1);
    assert_eq!(run.floor(), 2);
    let got = steps(&run);
    assert!(!got.iter().any(|(_, k)| matches!(k, Kind::Record { .. })), "the realm has stood on floor 2");
    assert!(got.contains(&(1, Kind::Rung { px: 32 })) && got.contains(&(2, Kind::Rung { px: 32 })));
    tick(&mut run, 120);
    run.descend_for_test();
    tick(&mut run, 1);
    assert_eq!(steps(&run), [(0, Kind::Record { floor: 3 })]);
    assert!(run.ledger.record && run.ledger.deepest == 3);
    assert!(run.road().contains(&Road::Record { floor: 3 }));
    // A knight's own moment waits for the party's banner.
    run.descend_for_test();
    tick(&mut run, 1);
    let record = run.rewards.steps.iter().find(|s| s.knight == 0).unwrap().at;
    let rung = run.rewards.steps.iter().find(|s| s.kind == Kind::Rung { px: 64 }).unwrap().at;
    assert!(rung > record, "the rung after the record: {record} then {rung}");
}

#[test]
fn a_floor_waits_for_its_plate_to_clear() {
    let mut run = party();
    run.descend_for_test();
    run.begin_floor_plate(floor_plates::Kind::of(run.dungeon.pack));
    tick(&mut run, 1);
    assert!(run.rewards.steps.is_empty(), "under the plate nothing plays");
    run.floor_plates.clear();
    tick(&mut run, 1);
    assert_eq!(steps(&run), [(0, Kind::Record { floor: 2 }), (1, Kind::Rung { px: 32 }), (2, Kind::Rung { px: 32 })]);
}

#[test]
fn a_relic_taken_off_the_floor_is_a_step() {
    let mut run = party();
    let (x, y) = (run.players[&2].x, run.players[&2].y);
    run.drop_item("awakened-fork".into(), x + 0.3, y, None);
    for _ in 0..40 {
        tick(&mut run, 1);
        if !run.rewards.steps.is_empty() {
            break;
        }
    }
    let name = run.book.get("awakened-fork").unwrap().name.clone();
    assert!(steps(&run).iter().any(|s| *s == (2, Kind::Relic { name: name.clone() })), "{:?}", steps(&run));
    assert_eq!(run.ledger.earned[&2].relics, 1);
}

#[test]
fn the_sky_takes_a_star_for_every_wish_and_a_whole_constellation_leaves_a_gift() {
    let mut run = party();
    assert_eq!(run.home.sky.filling(), (0, 0, 10));
    let names = ["volley", "pierce", "seeker", "quick", "fleet"];
    for name in names {
        let wish = phrasebook::get(name).unwrap();
        for id in [1, 2] {
            run.players.get_mut(&id).unwrap().wishes = 1;
            run.wish(id, &wish).unwrap();
        }
    }
    assert_eq!(run.home.sky.stars, 10);
    assert_eq!(run.home.sky.filling(), (1, 0, 14), "THE LAMP is whole");
    assert_eq!(run.home.sky.lit_by, ["Ada"]);
    assert_eq!(run.home.sky.gift, GIFT);
    assert!(steps(&run).contains(&(0, Kind::Lit { index: 0, by: "Ada".into() })));
    assert_eq!(run.road().last(), Some(&Road::Sky { index: 1, placed: 0, size: 14, gift: true }));
    // The realm keeps it through the host's settlement, as the privy wall.
    let mut journal = Journal::new("session-a".into(), run.raid_id);
    let claim = journal.claim_home(&mut run).unwrap().expect("the run has home news");
    let (mut realm, mut treasury) = (home::Home::default(), crate::realm::Spoils::default());
    let result = home_settlement::settle(
        &mut activity::Realm { home: &mut realm, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 },
        &claim.transfer,
        "Host",
        false,
    )
    .unwrap();
    assert!(result.changed);
    assert_eq!(result.home.sky, run.home.sky);
    // The next delve from that realm: a wish more each, once.
    let mut next = Run::at_home(5, 3, Some("Ada"), result.home.clone(), Default::default());
    tick(&mut next, 1);
    next.descend_for_test();
    let before: Vec<u8> = next.players.values().map(|h| h.wishes).collect();
    tick(&mut next, 1);
    let after: Vec<u8> = next.players.values().map(|h| h.wishes).collect();
    assert_eq!(after, before.iter().map(|w| w + GIFT).collect::<Vec<_>>());
    assert_eq!(next.home.sky.gift, 0, "spent");
    assert!(steps(&next).contains(&(0, Kind::Gift { wishes: GIFT })));
    tick(&mut next, 3);
    assert_eq!(next.players.values().map(|h| h.wishes).collect::<Vec<_>>(), after, "once");
}

#[test]
fn constellations_grow_and_come_round_again() {
    assert_eq!((0..9).map(size).collect::<Vec<_>>(), [10, 14, 18, 22, 26, 30, 34, 38, 42]);
    assert_eq!(size(40), 48);
    assert_eq!(constellation(0), "THE LAMP");
    assert_eq!(constellation(8), "THE LAMP II");
    assert_eq!(constellation(17), "THE CROWN III");
    let sky = Sky { stars: 10 + 14 + 3, ..Sky::default() };
    assert_eq!(sky.filling(), (2, 3, 18));
    assert_eq!(sky.lit(), 2);
}

#[test]
fn what_is_next_for_a_knight() {
    let mut run = party();
    let key = talents::knight_key(&run.players[&1]);
    run.home.knights.insert(key.clone(), talents::Prowess { xp: 1700, learned: vec![0] });
    let ahead = run.ahead(1);
    assert_eq!(ahead[0], Next::Level { level: 3, xp: 100 });
    assert_eq!(ahead[1], Next::Rung { px: 32, at: Climb::Floor(2) });
    assert_eq!(ahead[0].line(), "LV 3 IN 100 XP");
    assert_eq!(ahead[1].line(), "32 PX AT FLOOR 2");
    // A lesson earned and not taken; the best wish's next tier.
    run.home.knights.insert(key, talents::Prowess { xp: 6000, learned: vec![0, 1] });
    run.ledger.earned.entry(1).or_default().best = Some(("Volley".into(), 2));
    let ahead = run.ahead(1);
    assert_eq!(ahead[0], Next::Level { level: 6, xp: 3000 });
    assert_eq!(ahead[1], Next::Lessons(2));
    assert_eq!(ahead.last().unwrap(), &Next::Tier { wish: "Volley".into(), tier: 3, floor: Some(2) });
    assert_eq!(ahead.last().unwrap().line(), "TIER IV AT FLOOR 2");
    // A wish at its top has no tier ahead.
    run.ledger.earned.get_mut(&1).unwrap().best = Some(("Volley".into(), 4));
    assert!(!run.ahead(1).iter().any(|n| matches!(n, Next::Tier { .. })));
    // The last level has no level ahead.
    let top = *talents::LEVELS.last().unwrap();
    let all = vec![0; talents::LESSONS.len()];
    run.home.knights.insert(talents::knight_key(&run.players[&1]), talents::Prowess { xp: top, learned: all });
    assert_eq!(run.ahead(1)[0], Next::TopLevel);
    // Every line fits the road ahead's column.
    for n in run.ahead(1) {
        assert!(n.line().len() <= 24, "{}", n.line());
    }
}

#[test]
fn what_lies_ahead_for_the_party() {
    let mut run = party();
    assert_eq!(run.road()[0], Road::DeepRoad { floor: DEEPEST });
    assert_eq!(run.road()[0].line(), "STAND ON FLOOR 6: THE DEEP ROAD OPENS");
    run.home.deepest = DEEPEST;
    let Road::Village { floor, town, .. } = run.road()[0].clone() else { panic!("{:?}", run.road()) };
    assert_eq!(floor, deep::CHAPTER);
    assert!(!town.is_empty());
    run.home.deep.deepest = 14;
    assert!(matches!(run.road()[0], Road::Village { floor: 20, .. }));
    run.home.sky.stars = 7;
    assert_eq!(run.road().last().unwrap().line(), "THE LAMP: 3 WISHES MORE LIGHT IT");
    run.home.sky.stars = 9;
    assert_eq!(run.road().last().unwrap().line(), "THE LAMP: 1 WISH MORE LIGHT IT");
    for r in run.road() {
        assert!(r.line().len() <= 62, "{}", r.line());
    }
}

#[test]
fn a_new_delve_starts_its_ledger_over() {
    let mut run = Run::at_home(5, 3, Some("Ada"), home::Home::default(), Default::default());
    tick(&mut run, 1);
    run.descend_for_test();
    tick(&mut run, 1);
    assert!(run.ledger.earned.contains_key(&1) && run.ledger.deepest == 1);
    run.marks.insert("xp".into(), 40);
    tick(&mut run, 1);
    assert_eq!(run.ledger.earned[&1].xp, 40);
    assert!(!run.ledger.record, "floor 1 is no record");
}

#[test]
fn a_save_picks_up_where_it_stands() {
    let mut run = party();
    run.marks.insert("xp".into(), 100);
    tick(&mut run, 1);
    let saved = serde_json::to_value(&run).unwrap();
    assert!(saved["ledger"]["earned"].is_object(), "the ledger is kept");
    assert!(saved["rewards"].get("seen").is_none(), "the watcher's eye is not");
    let mut back: Run = serde_json::from_value(saved).unwrap();
    assert_eq!(back.ledger.earned, run.ledger.earned);
    back.marks.insert("xp".into(), 150);
    tick(&mut back, 2);
    assert!(back.rewards.steps.is_empty(), "a save cheers nothing");
    assert_eq!(back.ledger.earned[&1].xp, 100, "what came before the save stays; the first look counts nothing");
    tick(&mut back, 1);
    back.marks.insert("xp".into(), 170);
    tick(&mut back, 1);
    assert_eq!(back.ledger.earned[&1].xp, 120);
    assert!(back.valid_snapshot());
}

#[test]
fn an_old_run_and_realm_write_nothing_new() {
    let run = Run::new(9, 4, Some("Matt"));
    let saved = serde_json::to_string(&run).unwrap();
    assert!(!saved.contains("\"rewards\""));
    let realm = serde_json::to_string(&home::Home::default()).unwrap();
    assert!(!realm.contains("\"sky\""));
    // A garbled sky never costs the realm.
    let mut garbled: serde_json::Value = serde_json::from_str(&realm).unwrap();
    garbled["sky"] = serde_json::json!({"stars": "many"});
    let back: home::Home = serde_json::from_value(garbled).unwrap();
    assert!(back.sky.is_empty());
}

#[test]
fn a_ledger_out_of_bounds_is_no_snapshot() {
    let mut run = party();
    assert!(run.valid_snapshot());
    run.rewards.steps = (0..=MOST_STEPS as u64).map(|at| Step { at, knight: 1, kind: Kind::Level { level: 2 } }).collect();
    assert!(!run.valid_snapshot());
    run.rewards.steps.clear();
    run.home.sky.gift = 9;
    assert!(!run.valid_snapshot());
}

#[test]
fn the_ledger_stays_on_the_host_until_the_run_ends_and_then_goes_to_every_seat() {
    let mut run = party();
    run.marks.insert("xp".into(), 50);
    tick(&mut run, 1);
    assert_eq!(run.ledger.earned[&1].xp, 50);
    let tree = crate::wire::publish(&run).unwrap();
    assert!(tree.field("ledger").is_none(), "it plays nothing on a seat");
    let seat = crate::wire::read(&tree).unwrap();
    assert!(seat.ledger_shown().quiet());
    // The run falls: its last results carry the ledger to every seat.
    for hero in run.players.values_mut() {
        hero.hp = 0;
        (hero.winds, hero.talisman) = (0, false);
    }
    tick(&mut run, 1);
    assert!(run.over());
    let seat = crate::wire::read(&crate::wire::publish(&run).unwrap()).unwrap();
    assert_eq!(seat.ledger_shown(), &run.ledger);
    assert_eq!(seat.ahead(1), run.ahead(1));
    assert_eq!(seat.road(), run.road());
}

#[test]
fn each_step_lands_with_its_sound_once() {
    let mut run = party();
    run.marks.insert("xp".into(), talents::LEVELS[0]);
    tick(&mut run, 1);
    let count = |run: &Run| run.sounds.iter().filter(|s| **s == "level_up").count();
    assert_eq!(count(&run), 2, "one for each knight: {:?}", run.sounds);
    // The host takes a tick's sounds as it plays them.
    run.sounds.clear();
    tick(&mut run, 3);
    assert_eq!(count(&run), 0, "once");
    // A step put off behind another sounds when its own moment comes.
    let mut run = party();
    run.marks.insert("xp".into(), talents::LEVELS[0]);
    run.players.get_mut(&1).unwrap().wishes = 2;
    let volley = phrasebook::get("volley").unwrap();
    run.wish(1, &volley).unwrap();
    run.wish(1, &volley).unwrap();
    run.sounds.clear();
    tick(&mut run, 1);
    let level = run.rewards.steps.iter().find(|s| s.knight == 1 && s.kind == Kind::Level { level: 2 }).unwrap().at;
    assert!(level > run.tick, "the level waits behind the tier");
    let mut heard = None;
    for _ in 0..60 {
        run.sounds.clear();
        tick(&mut run, 1);
        if run.sounds.contains(&"level_up") {
            heard = Some(run.tick);
        }
    }
    assert!(heard.is_some_and(|t| t >= level), "{heard:?} for a step at {level}");
    for kind in [Kind::Level { level: 2 }, Kind::Rung { px: 32 }, Kind::Record { floor: 2 }, Kind::Relic { name: String::new() }, Kind::Lit { index: 0, by: String::new() }, Kind::Gift { wishes: 1 }] {
        assert!(kind.sound().is_some_and(|s| SOUNDS.contains(&s)), "{kind:?}");
    }
}

#[test]
fn the_rung_is_the_resolution_ladders() {
    assert_eq!(RUNG_PIXELS.to_vec(), crate::rules::resolution::Rung::ALL.iter().map(|r| r.px()).collect::<Vec<_>>());
    crate::rules::resolution::set_rule(Some(crate::rules::resolution::Rule::Floor));
    let mut run = party();
    assert_eq!(rung_px(&run, 1), crate::rules::resolution::rung(&run, 1).px());
    assert_eq!(next_rung(&run, 1), Some((32, Climb::Floor(2))));
    // One rung for everyone: no climb, and nothing ahead on the ladder.
    crate::rules::resolution::set_rule(Some(crate::rules::resolution::Rule::Fixed(crate::rules::resolution::Rung::of(64).unwrap())));
    let mut run = party();
    run.descend_for_test();
    tick(&mut run, 1);
    assert!(!steps(&run).iter().any(|(_, k)| matches!(k, Kind::Rung { .. })), "a fixed rung never climbs");
    assert!(!run.ahead(1).iter().any(|n| matches!(n, Next::Rung { .. })));
    crate::rules::resolution::set_rule(None);
}
