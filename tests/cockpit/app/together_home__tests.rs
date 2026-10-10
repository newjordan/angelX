use super::*;
use crate::drive::together_realm::{Spoil, Spoils};

fn spoils(items: &[(Spoil, u32)]) -> Spoils {
    let mut s = Spoils::default();
    for &(spoil, n) in items {
        s.add(spoil, n);
    }
    s
}

fn rich() -> Spoils {
    spoils(&[
        (Spoil::Gold, 5000),
        (Spoil::Ore, 50),
        (Spoil::Gem, 50),
        (Spoil::Wax, 50),
        (Spoil::Bone, 50),
        (Spoil::Ember, 50),
        (Spoil::Bond, 50),
        (Spoil::Scale, 5),
    ])
}

fn built(levels: &[(Station, u8)], deepest: u32) -> Home {
    let mut home = Home {
        deepest,
        ..Home::default()
    };
    for &(station, level) in levels {
        home.levels.insert(station, level);
    }
    home
}

fn hold(fire: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            fire,
            ..Input::default()
        },
    )])
}

/// Knight one standing on a station's plate.
fn on_plate(run: &mut Run, station: Station) {
    let spot = SPOTS.iter().find(|s| s.station == station).unwrap();
    let (c, r, w, h) = spot.plate;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
}

#[test]
fn a_delve_begins_in_the_undercroft() {
    let run = Run::at_home(7, 1, Some("Friend"), Home::default(), Spoils::default());
    assert!(run.at_home_now());
    assert_eq!(run.room().kind, RoomKind::Home);
    assert!(run.enemies.is_empty());
    assert_eq!(run.phase, Phase::Exploring);
    assert!(run.valid_snapshot());
    assert!(run.at_entrance(), "friends may join at home");
    assert!(run.cues.iter().any(|c| c == "home"));
    // Nobody arrives standing on the stair or a plate.
    for hero in run.players.values() {
        assert_eq!(home::plate_at(RoomKind::Home, hero.x, hero.y), None);
        let tile = run
            .room()
            .tile((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
        assert_eq!(tile, Tile::Floor);
    }
}

#[test]
fn holding_f_on_a_plate_orders_once_and_a_tap_orders_nothing() {
    let mut run = Run::at_home(7, 1, None, Home::default(), rich());
    on_plate(&mut run, Station::Forge);
    run.step(&hold(true));
    run.step(&hold(false));
    assert!(run.orders.is_empty(), "a tap in passing buys nothing");
    for _ in 0..home::BUY_HOLD + 20 {
        run.step(&hold(true));
    }
    assert_eq!(
        run.orders,
        vec![home::Order {
            knight: 1,
            station: Station::Forge
        }],
        "held: one order, and holding on buys no more"
    );
    run.orders.clear();
    run.step(&hold(false));
    for _ in 0..home::BUY_HOLD {
        run.step(&hold(true));
    }
    assert_eq!(run.orders.len(), 1, "let go and held again: another");
    // Off the plate, F is only a bow shot.
    run.orders.clear();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 20.0);
    for _ in 0..home::BUY_HOLD * 2 {
        run.step(&hold(true));
    }
    assert!(run.orders.is_empty());
}

#[test]
fn buying_pays_the_price_or_says_what_is_short() {
    let mut home = Home::default();
    let mut treasury = spoils(&[(Spoil::Gold, 160), (Spoil::Ore, 4)]);
    assert_eq!(home.buy(Station::Forge, &mut treasury), Ok(1));
    assert_eq!(treasury, spoils(&[(Spoil::Gold, 10)]));
    let short = home.buy(Station::Forge, &mut treasury).unwrap_err();
    assert!(short.contains("need"), "{short}");
    assert_eq!(home.level(Station::Forge), 1, "nothing paid, nothing built");
    assert_eq!(treasury, spoils(&[(Spoil::Gold, 10)]));
    // A landing needs the floor reached first.
    let mut treasury = rich();
    let locked = home.buy(Station::Map, &mut treasury).unwrap_err();
    assert!(locked.contains("floor 2"), "{locked}");
    assert_eq!(treasury, rich());
    home.deepest = 2;
    assert_eq!(home.buy(Station::Map, &mut treasury), Ok(1));
    assert_eq!(home.landings(), vec![1, 2]);
}

#[test]
fn every_ladder_is_affordable_from_real_runs_and_none_is_free() {
    for ladder in home::LADDERS.iter().filter(|l| l.station != Station::Coffer) {
        assert!(!ladder.rungs.is_empty());
        for (i, rung) in ladder.rungs.iter().enumerate() {
            assert!(!rung.price.is_empty(), "{} {i}", ladder.name);
            assert!(
                rung.price.iter().any(|&(s, n)| s == Spoil::Gold && n > 0),
                "{} {i} costs gold",
                ladder.name
            );
            assert!(
                rung.says.len() <= 30,
                "{} {i}: the ledger line fits",
                ladder.name
            );
        }
        // Nothing past the first rung is bought before the first floor.
        assert_eq!(ladder.rungs[0].needs.min(2), ladder.rungs[0].needs);
    }
}

#[test]
fn home_cards_follow_what_is_built() {
    let mut run = Run::at_home(
        7,
        1,
        None,
        built(&[(Station::Forge, 2), (Station::Hearth, 1)], 0),
        Spoils::default(),
    );
    let hero = &run.players[&1];
    assert!(hero.deck.contains(&"home-edge".to_string()));
    assert_eq!(hero.bonus.damage, 16);
    assert_eq!((hero.hp, hero.max_hp), (110, 110));
    assert_eq!(run.book.get("home-edge").unwrap().name, "Tobbin's Edge II");
    // A rung more: only the difference is added.
    run.rebuild_home(
        built(&[(Station::Forge, 3), (Station::Hearth, 2)], 0),
        Spoils::default(),
    );
    let hero = &run.players[&1];
    assert_eq!(hero.bonus.damage, 24);
    assert_eq!(hero.max_hp, 120);
    assert_eq!(
        hero.deck.iter().filter(|c| c.starts_with("home-")).count(),
        2,
        "one card a station"
    );
    // A knight of the company keeps the hearth's warmth on top of their own.
    let galahad = knights::knight("galahad").unwrap();
    run.outfit(1, galahad);
    assert_eq!(run.players[&1].max_hp, 130 + 20);
}

#[test]
fn a_friend_who_joins_at_home_is_dealt_its_cards() {
    let mut run = Run::at_home(
        7,
        1,
        None,
        built(&[(Station::Forge, 1)], 0),
        Spoils::default(),
    );
    run.join_seat(3, "Gawain");
    assert_eq!(run.players[&3].bonus.damage, 8);
}

#[test]
fn the_winding_stair_goes_down_to_the_landing_with_the_kit() {
    let home = Home {
        landing: 2,
        ..built(
            &[(Station::Rack, 2), (Station::Map, 1), (Station::Chapel, 1)],
            2,
        )
    };
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    run.begin_in(Pack::Crypt);
    let bombs = run.players[&1].bombs;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    run.step(&BTreeMap::new());
    assert!(run.at_home_now(), "the stair is taken on purpose: a moment on it");
    for _ in 0..home::DESCEND_HOLD {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.floor(), 2, "the landing Wren drew");
    assert_eq!(
        run.dungeon.pack,
        Pack::Cavern,
        "the other delve, below the Crypt"
    );
    let hero = &run.players[&1];
    assert!(hero.hand.contains(&"potion".to_string()));
    assert_eq!(hero.bombs, bombs + 1);
    assert_eq!(hero.winds, 1);
    assert!(
        run.bank.is_empty() && run.reclaimed.is_empty(),
        "home banks nothing"
    );
    assert!(run.valid_snapshot());
}

#[test]
fn the_stairs_mouth_turns_the_landing() {
    let mut run = Run::at_home(
        7,
        1,
        None,
        built(&[(Station::Map, 2)], 3),
        Spoils::default(),
    );
    let (c, r, w, _) = home::STAIR_MOUTH;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + 0.5) * TILE_UNITS;
    let mut seen = Vec::new();
    for _ in 0..4 {
        run.step(&hold(true));
        run.step(&hold(false));
        seen.push(run.home.landing());
    }
    assert_eq!(seen, vec![2, 3, 1, 2]);
    assert!(run.at_home_now(), "the mouth is not the stair");
}

#[test]
fn a_second_wind_lifts_a_fallen_knight_once() {
    let mut run = Run::new(7, 1, None);
    run.calm_for_test();
    let hero = run.players.get_mut(&1).unwrap();
    hero.winds = 1;
    hero.hp = 0;
    run.step(&BTreeMap::new());
    let hero = &run.players[&1];
    assert!(hero.hp > 0, "rose again");
    assert_eq!(hero.winds, 0);
    assert!(run.active(), "no wipe");
    assert!(run.cues.iter().any(|c| c == "second_wind"));
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = 0;
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped, "only once");
}

#[test]
fn keepers_greet_once_per_approach() {
    let mut run = Run::at_home(7, 1, None, Home::default(), Spoils::default());
    run.cues.clear();
    let (tx, ty) = home::keeper_at("tobbin").unwrap();
    let greets = |run: &Run| run.cues.iter().filter(|c| *c == "npc:tobbin").count();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (tx + 1.5, ty + 1.0);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(greets(&run), 1);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 20.0);
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (tx + 1.5, ty + 1.0);
    run.step(&BTreeMap::new());
    assert_eq!(greets(&run), 2);
}

#[test]
fn the_undercroft_survives_a_checkpoint_and_the_mirror() {
    let run = Run::at_home(
        7,
        1,
        Some("Friend"),
        built(&[(Station::Forge, 2), (Station::Rack, 1)], 2),
        rich(),
    );
    let copy: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(copy.valid_snapshot());
    assert_eq!(copy.home, run.home);
    assert_eq!(copy.treasury, run.treasury);
    assert_eq!(copy.players[&2].bonus.damage, 16);
}
