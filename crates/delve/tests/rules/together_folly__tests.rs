//! The Wax Maze: a folly room where everyone eats wax and flees wraiths.
use super::*;

fn floor() -> Run {
    let mut run = Run::new(41, 1, None);
    run.join_seat(2, "Two");
    run.enemies.clear();
    run
}

#[test]
fn every_floor_below_has_a_folly_room_and_walking_in_starts_the_maze() {
    let mut run = floor();
    run.pick_folly();
    let index = run.folly_room.expect("a folly room on floor one");
    assert_eq!(run.dungeon.rooms[index].kind, RoomKind::Hall);
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 50);
    }
    run.enter_for_test(index);
    let maze = run.folly.as_ref().expect("the maze begins");
    assert!(maze.wax.len() > 40 && maze.wraiths.len() == 4);
    assert!(run.enemies.is_empty(), "no monsters in a folly");
    assert!(run.barred(), "the doors hold while it runs");
    assert_eq!(maze.ante.values().sum::<u32>(), 10, "each knight antes a tenth");
    assert!(run.valid_snapshot());
}

#[test]
fn the_most_wax_takes_the_pot_and_the_crown() {
    let mut run = floor();
    run.pick_folly();
    let index = run.folly_room.unwrap();
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 50);
    }
    run.enter_for_test(index);
    // Past the title card; then knight one walks the top corridor.
    for _ in 0..folly::TITLE {
        run.step(&BTreeMap::new());
    }
    let cols = run.room().cols as i32;
    for x in 2..cols - 2 {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = ((x as f32 + 0.5) * TILE_UNITS, 1.5 * TILE_UNITS);
        h.invulnerable = 1000;
        run.step(&BTreeMap::new());
    }
    let eaten = run.folly.as_ref().unwrap().eaten[&1];
    assert!(eaten >= 10, "wax eaten walking: {eaten}");
    // Burn the candle down.
    run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1;
    run.step(&BTreeMap::new());
    let maze = run.folly.as_ref().unwrap();
    assert!(maze.done);
    assert!(!run.barred(), "the doors open after the podium");
    assert!(run.players[&1].laurel, "most wax wears the crown");
    assert!(run.players[&1].carried.get(Spoil::Gold) > run.players[&2].carried.get(Spoil::Gold));
    assert!(run.found.as_ref().unwrap().2.contains("ATE THE MOST WAX"));
    assert!(run.valid_snapshot());
}

#[test]
fn a_wraith_spills_wax_and_a_pale_one_is_caught() {
    let mut run = floor();
    run.players.retain(|&id, _| id == 1);
    run.pick_folly();
    let index = run.folly_room.unwrap();
    run.enter_for_test(index);
    run.folly.as_mut().unwrap().t = folly::TITLE;
    run.folly.as_mut().unwrap().eaten.insert(1, 12);
    let (wx, wy) = {
        let maze = run.folly.as_mut().unwrap();
        let w = &mut maze.wraiths[0];
        w.rest = 0;
        let p = w.pos(false);
        p
    };
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (wx * TILE_UNITS, wy * TILE_UNITS);
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.folly.as_ref().unwrap().eaten[&1], 12 - folly::SPILL, "a touch spills wax");
    // Pale, the same touch catches the wraith instead.
    run.folly.as_mut().unwrap().spooked.clear();
    run.folly.as_mut().unwrap().pale = folly::PALE;
    let (wx, wy) = run.folly.as_ref().unwrap().wraiths[0].pos(true);
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (wx * TILE_UNITS, wy * TILE_UNITS);
    }
    let bone = run.players[&1].carried.get(Spoil::Bone);
    run.step(&BTreeMap::new());
    assert!(run.players[&1].carried.get(Spoil::Bone) > bone, "a caught wraith banks a bone");
    assert!(run.folly.as_ref().unwrap().wraiths[0].rest > 0, "and rests in the middle");
}


/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_folly_shots -- --ignored`
#[test]
#[ignore]
fn write_folly_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("folly");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    run.step(&BTreeMap::new());
    save(&run, "1-title");
    for _ in 0..folly::TITLE + 3 * HZ {
        run.step(&BTreeMap::new());
    }
    let cols = run.room().cols as i32;
    for x in 2..cols / 2 {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = ((x as f32 + 0.5) * TILE_UNITS, 1.5 * TILE_UNITS);
        h.invulnerable = 1000;
        run.step(&BTreeMap::new());
    }
    save(&run, "2-play");
    run.folly.as_mut().unwrap().pale = folly::PALE;
    run.step(&BTreeMap::new());
    save(&run, "3-pale");
    run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1;
    run.step(&BTreeMap::new());
    save(&run, "4-done");
}

#[test]
fn a_friends_mirror_sees_the_maze_and_its_wraiths() {
    let mut run = floor();
    run.pick_folly();
    let mut mirror = run.clone();
    run.enter_for_test(run.folly_room.unwrap());
    for _ in 0..folly::TITLE + 10 {
        run.step(&BTreeMap::new());
    }
    // The maze changed the room's stones: the mirror takes the whole floor.
    assert_ne!(mirror.dungeon.world_revision, run.dungeon.world_revision);
    mirror = run.clone();
    for _ in 0..20 {
        run.step(&BTreeMap::new());
        mirror = run.wire_copy();
    }
    assert_eq!(mirror.folly, run.folly, "wax and wraiths come with every tick");
}

fn even_floor() -> Run {
    let mut run = Run::new(41, 1, None);
    run.descend_for_test();
    run.join_seat(2, "Two");
    run.enemies.clear();
    run
}

#[test]
fn the_even_floors_folly_is_the_belfry_shoot() {
    let mut run = even_floor();
    assert_eq!(run.dungeon.depth % 2, 0);
    run.pick_folly();
    let index = run.folly_room.expect("a hall on floor two");
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 40);
    }
    run.enter_for_test(index);
    assert!(run.bats.is_some() && run.folly.is_none());
    assert!(run.barred());
    for _ in 0..folly::TITLE + folly::BAT_EVERY + 1 {
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies.iter().any(|e| e.kind == EnemyKind::Bat), "the bats wake");
    // Knight one's harm is credited; burn the clock down.
    run.credit_bats(&[(1, 250)]);
    run.bats.as_mut().unwrap().t = folly::TITLE + folly::BAT_TIME - 1;
    run.step(&BTreeMap::new());
    let shoot = run.bats.as_ref().unwrap();
    assert!(shoot.done);
    assert!(!run.enemies.iter().any(|e| e.kind == EnemyKind::Bat));
    assert!(run.players[&1].laurel);
    assert!(run.found.as_ref().unwrap().2.contains("SHOT THE MOST BATS"));
    assert!(!run.barred());
    assert!(run.valid_snapshot());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_bats_shots -- --ignored`
#[test]
#[ignore]
fn write_bats_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("folly");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = even_floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    run.step(&BTreeMap::new());
    save(&run, "b1-title");
    for _ in 0..folly::TITLE + 5 * HZ {
        run.step(&BTreeMap::new());
        for h in run.players.values_mut() {
            h.invulnerable = 100;
        }
    }
    run.credit_bats(&[(1, 180), (2, 90)]);
    save(&run, "b2-play");
}
