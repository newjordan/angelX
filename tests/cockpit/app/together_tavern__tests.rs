use super::*;
use crate::drive::together_realm::Spoils;
use crate::drive::together_shooter::home::{Home, Station};

fn index_of(run: &Run, kind: RoomKind) -> usize {
    run.dungeon
        .rooms
        .iter()
        .position(|r| r.kind == kind)
        .unwrap_or_else(|| panic!("a {kind:?} at home"))
}

fn centre((c, r, w, h): (i32, i32, i32, i32)) -> (f32, f32) {
    (
        (c as f32 + w as f32 / 2.0) * TILE_UNITS,
        (r as f32 + h as f32 / 2.0) * TILE_UNITS,
    )
}

fn hold_f(run: &mut Run, ticks: u32) {
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..ticks {
        run.step(&fire);
    }
}

fn dug() -> Home {
    let mut home = Home {
        deepest: 1,
        ..Home::default()
    };
    home.levels.insert(Station::Wing, 1);
    home
}

#[test]
fn the_trophy_halls_west_wall_waits_for_tobbins_crew() {
    let home = Home {
        deepest: 1,
        ..Home::default()
    };
    let mut run = Run::at_home(3, 1, None, home, Spoils::default());
    assert!(!run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Tavern));
    let hall = index_of(&run, RoomKind::Trophies);
    assert!(!run.dungeon.rooms[hall].doors[3], "rubble, not a doorway");
    run.enter_for_test(hall);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = centre(WING_PLATE);
    hold_f(&mut run, home::BUY_HOLD + 1);
    assert!(
        run.orders.iter().any(|o| o.station == Station::Wing),
        "the plate orders the dig"
    );
}

#[test]
fn the_dug_wing_opens_the_trophy_hall_onto_the_tavern() {
    let mut run = Run::at_home(3, 1, None, Home::default(), Spoils::default());
    run.rebuild_home(dug(), Spoils::default());
    let hall = index_of(&run, RoomKind::Trophies);
    let tavern = index_of(&run, RoomKind::Tavern);
    assert_eq!(run.dungeon.neighbour(hall, 3), Some(tavern));
    assert_eq!(
        run.dungeon.neighbour(tavern, 1),
        Some(hall),
        "doors pair up"
    );
    assert!(run.cues.iter().any(|c| c == "dug"));
    // Dug once: a second rebuild changes nothing.
    let rooms = run.dungeon.rooms.len();
    run.rebuild_home(dug(), Spoils::default());
    assert_eq!(run.dungeon.rooms.len(), rooms);
    // A realm that dug it long ago starts with it.
    let run = Run::at_home(3, 1, None, dug(), Spoils::default());
    assert!(run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Tavern));
    assert!(!run.cues.iter().any(|c| c == "dug"));
}

#[test]
fn maud_keeps_the_bar_once_she_is_home_and_the_tap_is_dry_until_then() {
    let at_the_bar = |maud: bool| {
        let mut home = dug();
        if maud {
            home.residents.insert("maud".into());
        }
        let mut run = Run::at_home(3, 1, None, home, Spoils::default());
        run.enter_for_test(index_of(&run, RoomKind::Tavern));
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (
            MAUD_AT.0 * TILE_UNITS,
            (BAR.1 + 1) as f32 * TILE_UNITS + 2.0,
        );
        let heard = run.cues.len();
        run.step(&BTreeMap::new());
        run.cues[heard..].to_vec()
    };
    assert!(at_the_bar(true).iter().any(|c| c == "bar:maud"));
    assert!(at_the_bar(false).iter().any(|c| c == "tavern_dry"));
}

#[test]
fn the_siege_perilous_finds_most_knights_wanting() {
    let sit = |feats: usize| {
        let mut home = dug();
        for k in 0..feats {
            home.feats.insert(format!("deed{k}"));
        }
        let mut run = Run::at_home(3, 1, None, home, Spoils::default());
        run.enter_for_test(index_of(&run, RoomKind::Tavern));
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = centre(SIEGE);
        let hp = hero.hp;
        for _ in 0..HZ + 2 {
            run.step(&BTreeMap::new());
        }
        (run, hp)
    };
    let (run, hp) = sit(3);
    assert!(run.cues.iter().any(|c| c == "siege_wanting"));
    let hero = &run.players[&1];
    assert_eq!(hero.hp, hp - 5, "a sting");
    assert!(hero.x > centre(SIEGE).0 + 1.0, "and off the chair");
    let (run, _) = sit(SIEGE_WORTHY);
    assert!(run.cues.iter().any(|c| c == "siege_worthy"));
    assert!(run.feats.contains(&"siege_perilous"));
}

#[test]
fn the_rumour_board_forecasts_supported_politics_not_a_universal_guardian() {
    for seed in 0..16 {
        let mut run = Run::at_home(seed, 1, None, Home::default(), Spoils::default());
        let rumour = run.rumours()[0].clone();
        let pack = run.dungeon.pack;
        assert!(rumour.contains(&pack.name().to_uppercase()));
        assert!(rumour.contains(if pack == Pack::Cavern {
            "ANY CHIEF"
        } else {
            "ALL GATE HOUSES"
        }));
        run.begin_in(pack);
        run.descend_for_test();
        let g = run.boss_gates.clone().unwrap();
        let actual = run.rumours()[0].clone();
        for leader in g.leaders {
            assert!(actual.contains(&run.bosses[usize::from(leader.boss)].name));
        }
    }
}

#[test]
fn maud_pours_at_her_taps_and_only_once_she_is_home() {
    let at_the_stout = |maud: bool| {
        let mut home = dug();
        if maud {
            home.residents.insert("maud".into());
        }
        let mut run = Run::at_home(3, 1, None, home, Spoils::default());
        run.enter_for_test(index_of(&run, RoomKind::Tavern));
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = centre(TAPS[1]);
        hold_f(&mut run, home::BUY_HOLD + 1);
        run.orders.iter().map(|o| o.station).collect::<Vec<_>>()
    };
    assert_eq!(at_the_stout(true), [Station::TapB]);
    assert!(at_the_stout(false).is_empty(), "a dry tap pours nothing");
}

#[test]
fn a_round_is_drunk_at_the_top_of_the_stair() {
    let down_with = |round: Option<&str>| {
        let mut home = dug();
        home.round = round.map(String::from);
        let mut run = Run::at_home(3, 1, None, home, Spoils::default());
        run.descend_for_test();
        run
    };
    let dry = down_with(None);
    let hero = &dry.players[&1];
    let (damage, winds) = (hero.bonus.damage, hero.winds);
    let stout = down_with(Some("stout"));
    assert_eq!(stout.players[&1].bonus.damage, damage + STOUT);
    assert!(stout.players[&1].deck.iter().any(|c| c == "home-round"));
    assert!(stout.cues.iter().any(|c| c == "round_drunk:stout"));
    assert_eq!(
        stout.marks.get("round_drunk"),
        Some(&1),
        "the cockpit clears the tab"
    );
    assert_eq!(stout.home.round, None);
    assert_eq!(stout.round.as_deref(), Some("stout"));
    let ale = down_with(Some("ale"));
    assert_eq!(ale.players[&1].winds, winds + 1);
    let reserve = down_with(Some("reserve"));
    assert!(reserve.audience >= dry.audience + 300);
    assert!(!dry.marks.contains_key("round_drunk"));
}

#[test]
fn dinadan_takes_a_request_at_his_stage() {
    let mut run = Run::at_home(3, 1, None, dug(), Spoils::default());
    run.enter_for_test(index_of(&run, RoomKind::Tavern));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = centre(SONG_PLATES[0]);
    hold_f(&mut run, home::BUY_HOLD + 1);
    assert_eq!(
        run.orders.iter().map(|o| o.station).collect::<Vec<_>>(),
        [Station::SongA],
        "no resident needed: he sings for anyone"
    );
}

/// A delve of `pair` knights with `song` asked for, down the stair.
fn singing(song: &str, pair: bool) -> Run {
    let mut home = dug();
    home.song = Some(song.into());
    let mut run = Run::at_home(3, 1, pair.then_some("Friend"), home, Spoils::default());
    run.descend_for_test();
    run
}

#[test]
fn a_song_goes_down_the_stair_and_is_heard_by_knights_together() {
    let mut run = singing("haste", false);
    assert_eq!(run.song.as_deref(), Some("haste"));
    assert_eq!(run.home.song, None);
    assert_eq!(
        run.marks.get("song_sung"),
        Some(&1),
        "the cockpit clears the tab"
    );
    assert!(run.cues.iter().any(|c| c == "song:haste"));
    run.step(&BTreeMap::new());
    assert!(run.players[&1].singing, "alone, a knight always hears it");
    let mut run = singing("haste", true);
    for (id, x) in [(1, 10.0), (2, 14.0)] {
        run.players.get_mut(&id).unwrap().x = x;
    }
    run.step(&BTreeMap::new());
    assert!(
        run.players.values().all(|h| h.singing),
        "together, both hear it"
    );
    run.players.get_mut(&2).unwrap().x = 10.0 + SONG_REACH + 8.0;
    run.step(&BTreeMap::new());
    assert!(
        run.players.values().all(|h| !h.singing),
        "apart, neither does"
    );
}

#[test]
fn the_iron_hymn_guards_and_the_red_ballad_mends_on_kills() {
    let mut run = singing("iron", false);
    run.step(&BTreeMap::new());
    let base = run.players[&1].bonus.armor.min(MAX_ARMOR);
    assert_eq!(run.players[&1].armor, base + SONG_ARMOR);
    let mut run = singing("red", false);
    run.calm_for_test();
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = 40;
    let vamp = hero.bonus.vamp;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    let slain = run.enemies.len() as u32;
    run.step(&BTreeMap::new());
    assert!(slain > 0);
    assert!(
        run.players[&1].hp >= 40 + (vamp + SONG_VAMP) * slain,
        "{} after {slain} kills",
        run.players[&1].hp
    );
}

#[test]
fn the_lay_of_haste_quickens_a_knight_who_hears_it() {
    let walk = |song: Option<&str>| {
        let mut run = match song {
            Some(song) => singing(song, false),
            None => {
                let mut run = Run::at_home(3, 1, None, dug(), Spoils::default());
                run.descend_for_test();
                run
            }
        };
        run.calm_for_test();
        run.step(&BTreeMap::new());
        let x = run.players[&1].x;
        for _ in 0..HZ / 3 {
            run.step(&BTreeMap::from([(
                1,
                Input {
                    move_x: 1,
                    ..Default::default()
                },
            )]));
        }
        run.players[&1].x - x
    };
    let (plain, haste) = (walk(None), walk(Some("haste")));
    assert!(haste > plain * 1.1, "haste {haste} vs {plain}");
}
