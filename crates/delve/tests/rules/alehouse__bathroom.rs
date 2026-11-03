//! The washroom is ordinary geometry in the tavern, not a new activity.
use super::*;
use std::collections::{BTreeSet, VecDeque};

fn start() -> Run {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_alehouse();
    r
}

fn units((x, y): (f32, f32)) -> (f32, f32) {
    (x * TILE_UNITS, y * TILE_UNITS)
}

#[test]
fn bathroom_mirror_stand_is_reachable_from_arrival_without_a_travel_plate() {
    let r = start();
    let room = r.room();
    let from = (ARRIVAL.0.floor() as i32, ARRIVAL.1.floor() as i32);
    let to = (MIRROR_STAND.0.floor() as i32, MIRROR_STAND.1.floor() as i32);
    let mut todo = VecDeque::from([from]);
    let mut seen = BTreeSet::from([from]);
    while let Some((x, y)) = todo.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = (x + dx, y + dy);
            if room.tile(next.0, next.1) != Tile::Floor || seen.contains(&next) {
                continue;
            }
            let (ux, uy) = units((next.0 as f32 + 0.5, next.1 as f32 + 0.5));
            if r.settlement_walkable_for_test(ux, uy) {
                seen.insert(next);
                todo.push_back(next);
            }
        }
    }
    assert!(
        seen.contains(&to),
        "the mirror is connected to the hall on foot"
    );
    let (x, y) = units(MIRROR_STAND);
    assert!(r.settlement_walkable_for_test(x, y));
    assert!(r.valid_snapshot());
}

#[test]
fn bathroom_door_can_be_walked_both_ways_and_walls_stop_a_knight() {
    let mut r = start();
    let place = units((34.5, 5.5));
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = place;
    let step = |r: &mut Run, move_x: i8, move_y: i8, ticks| {
        for _ in 0..ticks {
            r.step(&BTreeMap::from([(
                1,
                Input {
                    move_x,
                    move_y,
                    ..Default::default()
                },
            )]));
        }
    };
    step(&mut r, 1, 0, 30);
    assert!(
        r.players[&1].x > 36.0 * TILE_UNITS,
        "walk through the open jamb"
    );
    assert_eq!(r.room().kind, RoomKind::Alehouse);
    step(&mut r, -1, 0, 30);
    assert!(
        r.players[&1].x < 35.0 * TILE_UNITS,
        "walk back into the hall"
    );

    // The same east wall immediately above the doorway is not passable.
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = units((34.5, 4.5));
    step(&mut r, 1, 0, 240);
    assert!(r.players[&1].x <= BATHROOM.0 as f32 * TILE_UNITS - HERO_RADIUS + 0.001);
    assert_eq!(r.room().kind, RoomKind::Alehouse);
}

#[test]
fn bathroom_fixture_collision_matches_the_small_enclosed_footprint() {
    let r = start();
    let room = r.room();
    let (c, row, w, h) = BATHROOM;
    for y in row..row + h {
        for x in c..c + w {
            let boundary = x == c || x == c + w - 1 || y == row || y == row + h - 1;
            let (dc, dr, dw, dh) = BATHROOM_DOOR;
            let door = (dc..dc + dw).contains(&x) && (dr..dr + dh).contains(&y);
            if boundary {
                assert_eq!(room.tile(x, y), if door { Tile::Floor } else { Tile::Wall });
            }
        }
    }
    for (c, row, w, h) in [BATHROOM_BASIN, BATHROOM_BUCKET] {
        for y in row..row + h {
            for x in c..c + w {
                assert_eq!(room.tile(x, y), Tile::Block);
                let (ux, uy) = units((x as f32 + 0.5, y as f32 + 0.5));
                assert!(!r.settlement_walkable_for_test(ux, uy));
            }
        }
    }
    for y in BATHROOM.3..HALL.1 as i32 {
        for x in BATHROOM.0..HALL.0 as i32 {
            assert_eq!(
                room.tile(x, y),
                Tile::Wall,
                "no accidental floor outside the annex"
            );
        }
    }
}

#[test]
fn bathroom_keeps_the_existing_lamp_weapons_doors_and_save_contract() {
    let r = start();
    let (x, y) = units(LAMP_STAND);
    assert_eq!(spot_near(x, y, r.tick), Some(Spot::Lamp));
    assert!(r.settlement_walkable_for_test(x, y));
    for i in 0..home_arms().len() {
        let (x, y) = units(arm_stand(i));
        assert_eq!(spot_near(x, y, r.tick), Some(Spot::Arm(i)));
        assert!(r.settlement_walkable_for_test(x, y));
    }
    for (c, row, w, h) in [FRONT_DOOR, BAZAAR_DOOR] {
        for y in row..row + h {
            for x in c..c + w {
                assert_eq!(r.room().tile(x, y), Tile::Stairs);
            }
        }
    }
    let decoded: Run = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert!(decoded.valid_snapshot());
    assert_eq!(
        serde_json::to_value(decoded.room()).unwrap(),
        serde_json::to_value(r.room()).unwrap()
    );
}
