use super::*;

fn fights(floor: &layout::Floor) -> Vec<&layout::Room> {
    floor.rooms.iter().filter(|r| matches!(r.kind, RoomKind::Fight | RoomKind::Stairs)).collect()
}

#[test]
fn a_lone_knights_floor_is_laid_out_as_it_always_was() {
    for depth in 1..=6 {
        for pack in [Pack::Crypt, Pack::Cavern] {
            let alone = layout::floor(depth, pack, &mut Rng::new(41 + u64::from(depth)));
            let one = layout::floor_party(depth, pack, 1, &mut Rng::new(41 + u64::from(depth)));
            let prints = |f: &layout::Floor| f.rooms.iter().map(|r| r.fingerprint()).collect::<Vec<_>>();
            assert_eq!(prints(&alone), prints(&one), "depth {depth} {pack:?}");
        }
    }
}

#[test]
fn fight_rooms_grow_with_the_party() {
    for party in 1..=5 {
        let (cols, rows) = layout::party_room(party);
        for depth in 1..=6 {
            let floor = layout::floor_party(depth, Pack::Crypt, party, &mut Rng::new(7 * u64::from(depth)));
            for room in fights(&floor) {
                assert!(room.cols >= cols && room.rows >= rows, "party {party} depth {depth}: {}x{}", room.cols, room.rows);
                assert!(room.valid_snapshot(), "party {party} depth {depth}");
            }
        }
    }
    let area = |p| {
        let (c, r) = layout::party_room(p);
        c * r
    };
    assert!(area(1) < area(2) && area(2) < area(3) && area(3) < area(4));
    assert_eq!(area(4), 4 * area(1));
}

#[test]
fn the_stairs_lay_the_next_floor_out_for_the_knights_seated() {
    let mut run = Run::new(5, 1, None);
    run.join_seat(2, "Gawain");
    run.join_seat(3, "Lynette");
    // The host's empty seat is no knight of the party.
    run.set_absent(1, true);
    assert_eq!(run.party_size(), 2);
    run.join_seat(4, "Percival");
    assert_eq!(run.party_size(), 3);
    run.descend_for_test();
    let (cols, rows) = layout::party_room(3);
    let rooms = fights(&run.dungeon);
    assert!(!rooms.is_empty());
    for room in rooms {
        assert!(room.cols >= cols && room.rows >= rows, "{}x{}", room.cols, room.rows);
    }
}

fn walk(hero: &mut Hero, move_x: i8, move_y: i8, aim_x: i8, aim_y: i8) {
    let input = Input { move_x, move_y, aim_x, aim_y, ..Input::default() };
    hero.own_keys(&input, false, true);
}

#[test]
fn a_knight_walking_one_way_with_no_aim_turns_to_face_it_after_half_a_second() {
    let run = Run::new(5, 1, None);
    let mut hero = run.players.values().next().unwrap().clone();
    walk(&mut hero, 0, 0, 0, -1);
    assert_eq!((hero.aim_x, hero.aim_y), (0.0, -1.0));
    for _ in 0..TURN_TO_WALK {
        walk(&mut hero, 1, 0, 0, 0);
        assert_eq!((hero.aim_x, hero.aim_y), (0.0, -1.0), "too soon");
    }
    walk(&mut hero, 1, 0, 0, 0);
    assert_eq!((hero.aim_x, hero.aim_y), (1.0, 0.0));
    // Turning the walk starts the half second again.
    walk(&mut hero, 0, 1, 0, 0);
    assert_eq!((hero.aim_x, hero.aim_y), (1.0, 0.0));
    for _ in 0..TURN_TO_WALK {
        walk(&mut hero, 0, 1, 0, 0);
    }
    assert_eq!((hero.aim_x, hero.aim_y), (0.0, 1.0));
}

#[test]
fn a_held_aim_always_wins_over_the_walk() {
    let run = Run::new(5, 1, None);
    let mut hero = run.players.values().next().unwrap().clone();
    for _ in 0..3 * TURN_TO_WALK {
        walk(&mut hero, 1, 0, -1, 0);
    }
    assert_eq!((hero.aim_x, hero.aim_y), (-1.0, 0.0));
    // Stopping, then walking again, waits the half second anew.
    walk(&mut hero, 0, 0, 0, 0);
    for _ in 0..TURN_TO_WALK {
        walk(&mut hero, 1, 0, 0, 0);
    }
    assert_eq!((hero.aim_x, hero.aim_y), (-1.0, 0.0));
}

/// A fight room with its two knights standing at `(x, y)` in it, nothing
/// else about.
fn knights_at(spot: impl Fn(&Room) -> (f32, f32)) -> Run {
    let mut run = Run::new(31, 1, Some("Friend"));
    run.descend_for_test();
    let room = (0..run.dungeon.rooms.len()).find(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight).unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    let (x, y) = spot(run.room());
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (x, y);
    }
    run
}

fn nearest_knight(run: &Run, (x, y): (f32, f32)) -> f32 {
    run.players.values().map(|h| (h.x - x).hypot(h.y - y)).fold(f32::MAX, f32::min)
}

#[test]
fn a_boss_never_rises_on_top_of_a_party_that_came_in_at_the_head_of_the_room() {
    let bosses = knights_at(|room| (room.width() / 2.0, 3.0)).bosses.len();
    assert!(bosses > 0);
    for index in 0..bosses {
        let mut run = knights_at(|room| (room.width() / 2.0, 3.0));
        run.spawn_boss(index);
        let boss = run.enemies.last().unwrap();
        assert!(nearest_knight(&run, (boss.x, boss.y)) >= BOSS_CLEARANCE - 2.0 * TILE_UNITS, "boss {index}");
    }
    let mut run = knights_at(|room| (room.width() / 2.0, 3.0));
    run.spawn(EnemyKind::Dragon);
    let dragon = run.enemies.last().unwrap();
    assert!(nearest_knight(&run, (dragon.x, dragon.y)) >= BOSS_CLEARANCE);
}

#[test]
fn a_party_coming_in_from_below_finds_the_boss_at_the_head_as_ever() {
    let mut run = knights_at(|room| (room.width() / 2.0, room.height() - 3.0));
    run.spawn(EnemyKind::Dragon);
    let dragon = run.enemies.last().unwrap();
    assert_eq!((dragon.x, dragon.y), (run.room().width() / 2.0, 7.0));
}
