use super::*;

/// Cinderjaw in a bare stairs room, with boulders at the tiles `blocks`.
fn arena(blocks: &[(usize, usize)]) -> Run {
    let mut run = Run::new(17, 1, Some("Friend"));
    run.dungeon = crawl_floor(2, Pack::Cavern, 17);
    let at = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(at);
    let room = &mut run.dungeon.rooms[at];
    for y in 1..room.rows - 1 {
        for x in 1..room.cols - 1 {
            room.set_for_test(x, y, Tile::Floor);
        }
    }
    for &(x, y) in blocks {
        room.set_for_test(x, y, Tile::Block);
    }
    run.enemies.clear();
    run.projectiles.clear();
    run.calm_for_test();
    run.works.blazes.clear();
    let i = run.bosses.iter().position(|b| b.id == furnace::ID).unwrap();
    run.spawn_boss_at_for_test(i, 24.0, 14.0);
    for h in run.players.values_mut() {
        (h.x, h.y) = (12.0, 20.0);
        h.invulnerable = 10000;
    }
    run.phase = Phase::Fighting;
    run
}

fn block(run: &Run, x: i32, y: i32) -> bool {
    run.dungeon.rooms[run.at].tile(x, y) == Tile::Block
}

#[test]
fn rubble_breaks_the_nearest_blocks_in_reach_and_sends_the_room_anew() {
    // (12, 7) holds the point (24, 14); two beside it, one far, a wall of seven.
    let wall: Vec<_> = (2..9).map(|y| (16, y)).collect();
    let mut blocks = vec![(11, 7), (13, 7), (20, 7)];
    blocks.extend(&wall);
    let mut run = arena(&blocks);
    let revision = run.dungeon.world_revision;
    run.sounds.clear();
    assert_eq!(run.shatter((24.0, 14.0), 3.4), 2);
    assert!(!block(&run, 11, 7) && !block(&run, 13, 7));
    assert!(block(&run, 20, 7), "out of reach");
    assert_eq!(run.dungeon.world_revision, revision.wrapping_add(1));
    assert!(run.sounds.contains(&"crack_open"));
    assert!(run.feel.puffs.len() >= 4);

    // Never more than SHATTER_MOST, nearest first.
    assert_eq!(run.shatter((33.0, 10.0), 20.0), SHATTER_MOST);
    assert!(block(&run, 20, 7), "the farthest stands");
    // Nothing in reach: nothing sent.
    let revision = run.dungeon.world_revision;
    assert_eq!(run.shatter((4.0, 4.0), 0.5), 0);
    assert_eq!(run.dungeon.world_revision, revision);
}

#[test]
fn rubble_never_breaks_a_rooms_edge_walls_doors_or_stairs() {
    let mut run = arena(&[]);
    let room = &run.dungeon.rooms[run.at];
    let (cols, rows) = (room.cols, room.rows);
    let before: Vec<Tile> = (0..rows)
        .flat_map(|y| (0..cols).map(move |x| (x, y)))
        .map(|(x, y)| run.dungeon.rooms[run.at].tile(x as i32, y as i32))
        .collect();
    run.dungeon.rooms[run.at].set_for_test(0, 5, Tile::Block);
    assert_eq!(run.shatter((1.0, 11.0), 60.0), 0);
    assert!(block(&run, 0, 5), "the edge stands");
    run.dungeon.rooms[run.at].set_for_test(0, 5, before[5 * cols]);
    let after: Vec<Tile> = (0..rows)
        .flat_map(|y| (0..cols).map(move |x| (x, y)))
        .map(|(x, y)| run.dungeon.rooms[run.at].tile(x as i32, y as i32))
        .collect();
    assert_eq!(before, after);
}

#[test]
fn rubble_cinderjaws_stoked_landing_breaks_the_stone_beside_it_once() {
    let mut run = arena(&[(13, 7), (11, 8)]);
    let leap = run.bosses[run.enemies[0].boss.unwrap() as usize]
        .windups
        .iter()
        .position(|u| u.kind == bosses::Windup::Leap)
        .unwrap() as u8;
    let e = &mut run.enemies[0];
    (e.stage, e.timer, e.signature.windup) = (signatures::DAZED, 0, leap);
    e.seen_hp = e.hp;
    run.tend_furnace();
    assert!(block(&run, 13, 7), "the opening is not stoked");

    let e = &mut run.enemies[0];
    e.hp = e.max_hp / 2;
    e.seen_hp = e.hp;
    run.tend_furnace();
    assert!(!block(&run, 13, 7) && !block(&run, 11, 8));
    let revision = run.dungeon.world_revision;
    // Held in time on its landing, it breaks nothing more.
    run.dungeon.rooms[run.at].set_for_test(12, 8, Tile::Block);
    run.tend_furnace();
    assert!(block(&run, 12, 8), "in reach, but the landing is spent");
    assert_eq!(run.dungeon.world_revision, revision);

    // The fight plays on over the rubble.
    for _ in 0..90 {
        run.step(&Default::default());
    }
    assert!(run.enemies.iter().any(|e| e.kind == EnemyKind::Boss));
}

/// Boulders round Cinderjaw before and after a stoked landing, drawn the
/// way every seat draws them, to `RUBBLE_SHOTS` (required).
/// `cargo test rubble_shots -- --ignored`
#[test]
#[ignore]
fn rubble_shots() {
    let dir = std::path::PathBuf::from(
        std::env::var("RUBBLE_SHOTS").expect("set RUBBLE_SHOTS to the output directory"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = arena(&[(9, 6), (10, 6), (14, 6), (15, 6), (9, 9), (10, 9), (14, 9), (15, 9)]);
    let leap = run.bosses[run.enemies[0].boss.unwrap() as usize]
        .windups
        .iter()
        .position(|u| u.kind == bosses::Windup::Leap)
        .unwrap() as u8;
    save(&run, "1-before");
    let e = &mut run.enemies[0];
    (e.stage, e.timer, e.signature.windup) = (signatures::DAZED, 0, leap);
    e.hp = e.max_hp / 2;
    e.seen_hp = e.hp;
    run.tend_furnace();
    save(&run, "2-landed");
    for _ in 0..12 {
        run.step(&Default::default());
    }
    save(&run, "3-after");
}

#[test]
fn rubble_a_breaking_charge_runs_into_a_pillar_and_through_it() {
    let mut run = arena(&[(18, 7), (18, 8)]);
    let tusk = bosses::checked("iron-tusk-chief").unwrap();
    let charge = tusk.windups.iter().position(|u| u.kind == bosses::Windup::Charge).unwrap();
    assert!(tusk.windups[charge].breaks);
    run.bosses.push(tusk);
    let index = run.bosses.len() - 1;
    run.enemies.clear();
    run.spawn_boss_at_for_test(index, 28.0, 15.0);
    let e = &mut run.enemies[0];
    (e.stage, e.timer, e.signature.windup, e.dir) = (signatures::DASH, 0, charge as u8, (1.0, 0.0));
    e.signature.rails = vec![signatures::Rail { a: (28.0, 15.0), b: (46.0, 15.0) }];
    let mut shatters = Vec::new();
    for _ in 0..45 {
        let grid = Grid {
            room: &run.dungeon.rooms[run.at],
            barred: true,
        };
        let e = &mut run.enemies[0];
        let mut deeds = foes::Deeds::default();
        signatures::wind(e, &run.bosses[index], &[(12.0, 20.0)], &grid, 17, &mut deeds);
        shatters.extend(deeds.shatters);
        if run.enemies[0].stage != signatures::DASH {
            break;
        }
    }
    assert_eq!(shatters.len(), 1, "stopped by the pillar once");
    run.apply_deeds(foes::Deeds { shatters, ..Default::default() });
    assert!(!block(&run, 18, 7) && !block(&run, 18, 8), "through it");
}

#[test]
fn rubble_breaks_is_a_windup_word_and_off_unless_written() {
    let cinderjaw = bosses::checked(furnace::ID).unwrap();
    assert!(cinderjaw.windups.iter().all(|u| !u.breaks), "its own rule breaks its stone");
    let sovereign = bosses::checked("hellforge-slag-sovereign").unwrap();
    assert!(sovereign.windups.iter().any(|u| u.kind == bosses::Windup::Slam && u.breaks));
    let plain = serde_json::to_string(&cinderjaw.windups[0]).unwrap();
    assert!(!plain.contains("breaks"), "old saves and streams keep their shape: {plain}");
}
