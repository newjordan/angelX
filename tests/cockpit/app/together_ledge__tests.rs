use super::ledge::FOOT;
use super::*;

fn index_of(run: &Run, kind: RoomKind) -> usize {
    run.dungeon
        .rooms
        .iter()
        .position(|r| r.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} room"))
}

fn slay_all(run: &mut Run) {
    run.clear_for_test();
}

fn press(dir_y: i8, dir_x: i8) -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            move_x: dir_x,
            move_y: dir_y,
            ..Default::default()
        },
    )])
}

fn idle() -> BTreeMap<u32, Input> {
    BTreeMap::new()
}

/// A solo run standing in floor 1's side-on hall, its monsters gone.
fn hall() -> Run {
    let mut run = Run::new(0, 1, None);
    let at = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Ledge)
        .expect("floor 1 has a side-on hall");
    run.enter(at, Some(3));
    run.calm_for_test();
    run.enemies.clear();
    run
}

fn ground_y() -> f32 {
    // The hall's ground is row 11's top.
    11.0 * TILE_UNITS - FOOT
}

#[test]
fn floor_one_hangs_one_side_on_hall_off_a_fight_room() {
    for seed in 0..40 {
        let run = Run::new(seed, 1, None);
        let floor = &run.dungeon;
        let halls: Vec<usize> = (0..floor.rooms.len())
            .filter(|&i| floor.rooms[i].kind == RoomKind::Ledge)
            .collect();
        assert_eq!(halls.len(), 1, "seed {seed}");
        let hall = &floor.rooms[halls[0]];
        let back = hall.way_back().unwrap();
        assert_eq!(hall.doors.iter().filter(|&&d| d).count(), 1, "a dead end");
        let parent = floor.neighbour(halls[0], back).unwrap();
        assert_eq!(floor.rooms[parent].kind, RoomKind::Fight);
        assert_eq!(floor.neighbour(parent, (back + 2) % 4), Some(halls[0]));
        assert!(floor.rooms[parent].doors[(back + 2) % 4]);
        assert!(hall.valid_snapshot());
    }
    let mut run = Run::new(0, 1, None);
    run.descend_for_test();
    assert!(run.dungeon.rooms.iter().all(|r| r.kind != RoomKind::Ledge));
}

#[test]
fn the_hall_turns_the_party_side_on_at_its_west_door() {
    let mut run = Run::new(0, 1, Some("Matt"));
    let at = index_of(&run, RoomKind::Ledge);
    run.enter(at, Some(1));
    assert!(run.side_on());
    assert_eq!(run.phase, Phase::Fighting, "its monsters wait");
    for hero in run.players.values() {
        assert!(hero.x < 8.0 && hero.grounded);
        assert_eq!(hero.y, ground_y());
    }
    // Walkers stand on something; fliers fly.
    for enemy in &run.enemies {
        if !enemy.kind.flies() {
            let feet = enemy.y + FOOT;
            let below = run
                .room()
                .tile((enemy.x / TILE_UNITS) as i32, (feet / TILE_UNITS) as i32);
            assert!(matches!(below, Tile::Block | Tile::Ledge), "{enemy:?}");
        }
    }
}

#[test]
fn knights_fall_jump_and_jump_again_in_the_air() {
    let mut run = hall();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (6.0, 8.0);
    hero.grounded = false;
    for _ in 0..HZ {
        run.step(&idle());
    }
    assert_eq!(run.players[&1].y, ground_y(), "gravity sets them down");
    assert!(run.players[&1].grounded);

    // One jump: a press, not a hold.
    let peak = |run: &mut Run, double: bool| {
        let mut top = f32::MAX;
        run.step(&press(-1, 0));
        for i in 0..HZ * 2 {
            let again = double && i == 8;
            run.step(&if again { press(-1, 0) } else { idle() });
            if i == 7 {
                // Let go between the two presses.
                run.step(&idle());
            }
            top = top.min(run.players[&1].y);
        }
        top
    };
    let single = peak(&mut run, false);
    assert!(ground_y() - single > 3.0, "a jump clears a step ({single})");
    assert_eq!(run.players[&1].y, ground_y(), "and comes back down");
    let held = {
        let mut top = f32::MAX;
        for _ in 0..HZ * 2 {
            run.step(&press(-1, 0));
            top = top.min(run.players[&1].y);
        }
        top
    };
    assert!((held - single).abs() < 0.5, "holding up jumps once");
    run.step(&idle());
    let double = peak(&mut run, true);
    assert!(
        ground_y() - double > (ground_y() - single) * 1.5,
        "the air jump climbs higher: {double} vs {single}"
    );
}

#[test]
fn planks_hold_from_above_and_drop_you_through_with_down() {
    let mut run = hall();
    // The first plank: row 7, columns 11–15.
    let plank_top = 7.0 * TILE_UNITS;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (13.0 * TILE_UNITS, plank_top - FOOT - 3.0);
    for _ in 0..HZ {
        run.step(&idle());
    }
    assert_eq!(run.players[&1].y, plank_top - FOOT, "it holds from above");
    run.step(&press(1, 0));
    for _ in 0..HZ {
        run.step(&idle());
    }
    assert_eq!(run.players[&1].y, ground_y(), "down drops through it");
    // And from below, a double jump passes up through it and stands on it.
    run.step(&press(-1, 0));
    for i in 0..HZ * 2 {
        run.step(&if i == 9 { press(-1, 0) } else { idle() });
    }
    assert_eq!(
        run.players[&1].y,
        plank_top - FOOT,
        "up through, then on top"
    );
}

#[test]
fn a_pit_hurts_and_puts_the_knight_back_on_firm_ground() {
    let mut run = hall();
    // The first pit: columns 9–11 of the ground.
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let hp = hero.hp;
    for _ in 0..HZ * 2 {
        run.step(&press(0, 1));
        if run.players[&1].hp < hp {
            break;
        }
    }
    let hero = &run.players[&1];
    assert_eq!(hero.hp, hp - 20, "the pit bites once");
    assert_eq!(hero.y, ground_y());
    assert!(hero.x < 9.0 * TILE_UNITS, "back on this side: {}", hero.x);
    // A running jump from its lip clears it.
    let mut run = hall();
    let start_hp = run.players[&1].hp;
    let mut jumped = false;
    for _ in 0..HZ * 2 {
        let lip = run.players[&1].x >= 8.3 * TILE_UNITS;
        run.step(&press(if lip && !jumped { -1 } else { 0 }, 1));
        jumped |= lip;
    }
    assert_eq!(run.players[&1].hp, start_hp, "no fall");
    assert!(run.players[&1].x > 13.0 * TILE_UNITS);
}

#[test]
fn the_rolls_run_flat_and_the_knight_faces_the_way_they_run() {
    let mut run = hall();
    run.step(&press(0, -1));
    assert_eq!(run.players[&1].aim_x, -1.0);
    run.step(&press(0, 1));
    assert_eq!((run.players[&1].aim_x, run.players[&1].aim_y), (1.0, 0.0));
    let y = run.players[&1].y;
    run.step(&BTreeMap::from([(
        1,
        Input {
            dash: true,
            ..Default::default()
        },
    )]));
    let x = run.players[&1].x;
    for _ in 0..5 {
        run.step(&idle());
        assert_eq!(run.players[&1].y, y, "flat along the ground");
    }
    assert!(run.players[&1].x > x + 3.0, "{x} -> {}", run.players[&1].x);
}

#[test]
fn out_the_west_door_is_back_where_you_came_in() {
    let mut run = Run::new(0, 1, None);
    let at = index_of(&run, RoomKind::Ledge);
    let back = run.dungeon.rooms[at].way_back().unwrap();
    let parent = run.dungeon.neighbour(at, back).unwrap();
    run.dungeon.rooms[parent].cleared = true;
    run.enter(parent, Some(2));
    assert!(!run.side_on());
    // Through the parent's banner door: side-on.
    let toward = (back + 2) % 4;
    run.enter(at, Some(back));
    assert!(run.side_on());
    slay_all(&mut run);
    assert_eq!(run.phase, Phase::Exploring);
    for _ in 0..HZ * 2 {
        run.step(&press(0, -1));
        if !run.side_on() {
            break;
        }
    }
    assert_eq!(run.at, parent, "home through the one doorway");
    let hero = &run.players[&1];
    let (w, h) = (run.room().width(), run.room().height());
    let near = match toward {
        0 => hero.y < 5.0,
        1 => hero.x > w - 5.0,
        2 => hero.y > h - 5.0,
        _ => hero.x < 5.0,
    };
    assert!(
        near,
        "at the door toward the hall: {toward} ({}, {})",
        hero.x, hero.y
    );
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_ledge_shots -- --ignored`
#[test]
#[ignore]
fn write_ledge_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = Run::new(0, 1, Some("Matt"));
    let at = index_of(&run, RoomKind::Ledge);
    let back = run.dungeon.rooms[at].way_back().unwrap();
    let parent = run.dungeon.neighbour(at, back).unwrap();
    run.dungeon.rooms[parent].cleared = true;
    run.enter(parent, Some(2));
    save(&run, "ledge_door");
    run.enter(at, Some(back));
    for _ in 0..TELEGRAPH + 10 {
        run.step(&idle());
    }
    save(&run, "ledge_enter");
    // Matt jumps toward the first plank; the host fires up and right.
    for i in 0..14 {
        run.step(&BTreeMap::from([
            (
                1,
                Input {
                    aim_x: 1,
                    aim_y: -1,
                    ..Default::default()
                },
            ),
            (
                2,
                Input {
                    move_x: 1,
                    move_y: if i == 0 || i == 9 { -1 } else { 0 },
                    ..Default::default()
                },
            ),
        ]));
    }
    save(&run, "ledge_jump");
    for _ in 0..80 {
        run.step(&BTreeMap::from([(
            1,
            Input {
                move_x: 1,
                ..Default::default()
            },
        )]));
    }
    save(&run, "ledge_run");
    // The far end, where the walkers wait.
    for (i, hero) in run.players.values_mut().enumerate() {
        (hero.x, hero.y) = (70.0 + i as f32 * 3.0, 11.0 * TILE_UNITS - FOOT);
        hero.hp = hero.max_hp;
    }
    for _ in 0..20 {
        run.step(&idle());
    }
    save(&run, "ledge_far");
}
