//! Everyone plays at once: a game started in a room gives every knight
//! there a round of their own, each sees their own, and the first home wins.
use super::*;

fn fair() -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    for id in 2..=3 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let room = world::room_of(&run.dungeon, RoomKind::Fairground).unwrap();
    run.arrive(room, (12.0, 11.0));
    run
}

fn place(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

#[test]
fn every_knight_in_the_room_plays_their_own_round_and_the_first_home_wins() {
    let mut run = fair();
    run.tick_pastimes(&BTreeMap::new());
    assert!(run.pastimes.heat.is_none());
    place(&mut run, 1, Game::Barrel.start());
    run.tick_pastimes(&BTreeMap::new());
    let heat = run.pastimes.heat.clone().expect("a heat begins");
    assert_eq!(heat.seats, vec![1, 2, 3], "everyone in the room is in");
    assert_eq!(run.pastimes.round.as_ref().unwrap().game, Game::Barrel);
    assert_eq!(run.pastimes.heats.len(), 2);
    // Each knight's screen shows their own round.
    for id in [2, 3] {
        let view = run.view_for_seat(id).unwrap();
        assert_eq!(view.pastimes.round.as_ref(), run.pastimes.heats.get(&id));
    }
    assert!(run.valid_snapshot());
    // Seat two rides the course; seat one's round never moves for it.
    for (i, &flag) in FLAGS.iter().enumerate() {
        place(&mut run, 2, flag);
        run.tick_pastimes(&BTreeMap::new());
        if i + 1 < FLAGS.len() {
            assert_eq!(run.pastimes.heats[&2].progress, (i + 1) as u8);
            assert_eq!(run.pastimes.round.as_ref().unwrap().progress, 0);
            assert_eq!(run.pastimes.heat.as_ref().unwrap().standing[&2], (i + 1) as u8);
        }
    }
    assert!(run.pastimes.heat.is_none() && run.pastimes.round.is_none());
    assert!(run.pastimes.heats.is_empty(), "first home ends everyone's round");
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("SEAT 2 WINS"), "{said}");
    assert!(run.players[&2].laurel, "the winner wears the laurel");
    assert!(run.players[&1].booby.is_some() && run.players[&3].booby.is_some());
    assert!(run.valid_snapshot());
}

#[test]
fn a_knight_alone_plays_as_before() {
    let mut run = fair();
    run.players.retain(|&id, _| id == 1);
    place(&mut run, 1, Game::Barrel.start());
    run.tick_pastimes(&BTreeMap::new());
    assert!(run.pastimes.heats.is_empty());
    for &flag in &FLAGS {
        place(&mut run, 1, flag);
        run.tick_pastimes(&BTreeMap::new());
    }
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("complete!"), "{said}");
    assert!(!run.players[&1].laurel, "no crown without a rival");
}

/// Plates of a heat from each knight's own screen, and the crowned and
/// booby-prized knights after: `ANGEL_ARENA_SHOTS=<dir> cargo test
/// write_heat_shots -- --ignored`.
#[test]
#[ignore]
fn write_heat_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("heats");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, seat: u32, name: &str| {
        let view = run.view_for_seat(seat);
        let img = crate::render::overworld::arena::frame_for(
            view.as_ref().unwrap_or(run),
            768,
            448,
            Some(seat),
        );
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    for (game, steps) in [(Game::Barrel, 3usize), (Game::DuckShoot, 0), (Game::Halfpipe, 0)] {
        let mut run = fair();
        if game.room() != RoomKind::Fairground {
            let room = world::room_of(&run.dungeon, game.room()).unwrap();
            run.arrive(room, (12.0, 11.0));
        }
        place(&mut run, 1, (12.0, 11.0));
        run.tick_pastimes(&BTreeMap::new());
        place(&mut run, 1, game.start());
        place(&mut run, 2, (8.0, 9.0));
        place(&mut run, 3, (16.0, 9.0));
        run.tick_pastimes(&BTreeMap::new());
        for &flag in FLAGS.iter().take(steps) {
            place(&mut run, 2, flag);
            run.tick_pastimes(&BTreeMap::new());
        }
        for _ in 0..10 {
            run.step(&BTreeMap::new());
        }
        for seat in 1..=3 {
            save(&run, seat, &format!("{game:?}-seat{seat}"));
        }
    }
    let mut run = fair();
    run.tick_pastimes(&BTreeMap::new());
    place(&mut run, 1, Game::Barrel.start());
    run.tick_pastimes(&BTreeMap::new());
    for &flag in &FLAGS {
        place(&mut run, 2, flag);
        run.tick_pastimes(&BTreeMap::new());
    }
    place(&mut run, 1, (10.0, 9.0));
    place(&mut run, 2, (12.0, 9.0));
    place(&mut run, 3, (14.0, 9.0));
    run.players.get_mut(&3).unwrap().booby = Some(Game::Pot);
    for _ in 0..5 {
        run.step(&BTreeMap::new());
    }
    save(&run, 1, "after-crowned");
}

#[test]
fn the_firefly_grove_pays_its_two_purses_once() {
    use crate::realm::{Spoil, Spoils};
    use crate::rules::countryside::{self, Country, GROVE_MARKS};
    let mut country = Country::default();
    let mut treasury = Spoils::default();
    let marks: BTreeMap<String, u32> = GROVE_MARKS.iter().map(|m| (m.to_string(), 1)).collect();
    assert!(GROVE_MARKS.iter().all(|m| countryside::intent_key(m)));
    let (changed, news) = countryside::settle(&mut country, &mut treasury, &marks);
    assert!(changed && news.len() == 2, "{news:?}");
    assert_eq!(treasury.get(Spoil::Gold), 80);
    let (_, again) = countryside::settle(&mut country, &mut treasury, &marks);
    assert!(again.is_empty(), "never twice");
    assert_eq!(treasury.get(Spoil::Gold), 80);
}

#[test]
fn a_far_duck_counts_double() {
    let mut run = fair();
    run.players.retain(|&id, _| id == 1);
    place(&mut run, 1, Game::DuckShoot.start());
    run.tick_pastimes(&BTreeMap::new());
    let round = run.pastimes.round.as_mut().unwrap();
    let range = round.duckshoot.as_mut().unwrap();
    // Aim dead on where the far rail's duck slides to next, and fire.
    let duck = &range.ducks[2];
    range.ax = duckshoot::DuckShoot::screen_x(duck.x + duck.vx, 2) as i16;
    range.ay = duckshoot::RAIL_SY[2] as i16;
    let fire = BTreeMap::from([(1, Input { fire: true, ..Default::default() })]);
    run.tick_pastimes(&fire);
    assert_eq!(run.pastimes.round.as_ref().unwrap().progress, 2);
}

#[test]
fn the_crown_brings_a_bomb_down_and_the_booby_prize_takes_a_blow() {
    let mut run = fair();
    run.players.get_mut(&1).unwrap().laurel = true;
    run.players.get_mut(&2).unwrap().booby = Some(Game::Pot);
    let bombs = (run.players[&1].bombs, run.players[&2].bombs);
    run.kit_out_for_test();
    assert_eq!(run.players[&1].bombs, (bombs.0 + 1).min(MAX_BOMBS));
    assert_eq!(run.players[&2].bombs, bombs.1, "no crown, no bomb");
    let hero = run.players.get_mut(&2).unwrap();
    let hp = hero.hp;
    hero.invulnerable = 0;
    hero.hurt(30);
    assert_eq!(hero.hp, hp, "the chamber pot took it");
    assert!(hero.booby.is_none(), "and flew off");
    hero.invulnerable = 0;
    hero.hurt(30);
    assert!(hero.hp < hp, "the next blow lands");
}

#[test]
fn a_games_first_win_pays_the_realm_once() {
    use crate::realm::{Spoil, Spoils};
    use crate::rules::countryside::{self, Country, FIRST_WIN_GOLD};
    let mut run = fair();
    run.players.retain(|&id, _| id == 1);
    place(&mut run, 1, Game::Barrel.start());
    run.tick_pastimes(&BTreeMap::new());
    for &flag in &FLAGS {
        place(&mut run, 1, flag);
        run.tick_pastimes(&BTreeMap::new());
    }
    let mark = countryside::game_mark(Game::Barrel.index());
    assert!(run.marks.contains_key(&mark), "the win is marked");
    assert!(countryside::intent_key(&mark));
    let (mut country, mut treasury) = (Country::default(), Spoils::default());
    countryside::settle_games(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury.get(Spoil::Gold), FIRST_WIN_GOLD);
    countryside::settle_games(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury.get(Spoil::Gold), FIRST_WIN_GOLD, "only once");
}

#[test]
fn the_golden_acorn_crowns_its_finder_and_pays_once() {
    use crate::realm::Spoils;
    use crate::rules::{countryside, woodland as wood};
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    let room = world::room_of(&run.dungeon, RoomKind::HedgeMaze).unwrap();
    run.arrive(room, wood::ACORN);
    assert_eq!(run.room().tile(wood::ACORN.0 as i32, wood::ACORN.1 as i32), Tile::Floor);
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert!(run.marks.contains_key(wood::ACORN_MARK));
    assert!(run.players[&1].laurel);
    let (mut country, mut treasury) = (countryside::Country::default(), Spoils::default());
    countryside::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury.get(Spoil::Gold), wood::ACORN_GOLD);
    countryside::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury.get(Spoil::Gold), wood::ACORN_GOLD, "once");
}
