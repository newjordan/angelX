use super::*;
use crate::drive::together_shooter::home::{DESCEND_HOLD, Home};

fn at_the_gate() -> Run {
    let mut run = Run::at_home(7, 1, Some("Friend"), Home::default(), Default::default());
    run.come_to_the_gate();
    run
}

/// Knight one (and any friend) stands on `e`'s stair long enough to take it.
/// A step first where they stand: arriving on stairs, one steps off before
/// they work.
fn take(run: &mut Run, e: &Entrance) {
    run.step(&BTreeMap::new());
    let (x, y) = entrance_centre(e);
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (x, y);
    }
    for _ in 0..DESCEND_HOLD + 2 {
        run.step(&BTreeMap::new());
    }
}

fn entrance(id: &str) -> &'static Entrance {
    ENTRANCES.iter().find(|e| e.id == id).unwrap()
}

fn walkable(room: &Room, x: f32, y: f32) -> bool {
    matches!(
        room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32),
        Tile::Floor | Tile::Door | Tile::Stairs
    )
}

#[test]
fn a_delve_begins_in_the_world_at_the_gate() {
    let run = at_the_gate();
    assert!(run.at_home_now());
    assert_eq!(run.room().kind, RoomKind::Gate);
    assert!(run.enemies.is_empty());
    for hero in run.players.values() {
        assert!(
            walkable(run.room(), hero.x, hero.y),
            "{} {}",
            hero.x,
            hero.y
        );
    }
    assert!(run.cues.iter().any(|c| c == "the_gate"));
}

#[test]
fn the_world_rises_once_on_the_home_floor_and_never_below() {
    let mut run = Run::at_home(7, 1, None, Home::default(), Default::default());
    let rooms = run.dungeon.rooms.len();
    assert!(!raise_world(&mut run.dungeon), "at_home already raised it");
    for (kind, cell) in ROOMS {
        let i = room_of(&run.dungeon, kind).unwrap();
        assert_eq!(run.dungeon.rooms[i].cell, cell);
        assert!(run.dungeon.rooms[i].valid_snapshot());
    }
    // Cells are unique across the whole floor.
    let mut cells: Vec<_> = run.dungeon.rooms.iter().map(|r| r.cell).collect();
    cells.sort_unstable();
    cells.dedup();
    assert_eq!(cells.len(), rooms);
    // A loop settlement digs within six cells of the cellar: never this far.
    let cellar = run.dungeon.rooms[0].cell;
    for (_, (x, y)) in ROOMS {
        assert!((x - cellar.0).abs() > 6 || (y - cellar.1).abs() > 6);
    }
    run.descend_for_test();
    assert!(!raise_world(&mut run.dungeon));
    assert!(run.dungeon.rooms.iter().all(|r| !r.kind.in_world()));
}

#[test]
fn every_entrance_stands_on_stairs_reachable_from_its_rooms_doors() {
    let run = at_the_gate();
    for e in &ENTRANCES {
        let i = room_of(&run.dungeon, e.room).unwrap();
        let room = &run.dungeon.rooms[i];
        let (c, r, w, h) = e.at;
        for row in r..r + h {
            for col in c..c + w {
                assert_eq!(room.tile(col, row), Tile::Stairs, "{} at {col},{row}", e.id);
            }
        }
        // Flood the walkable tiles from the room's first open door (or the
        // arrival in the gate) and find the entrance in it.
        // A room with no doorways (the King's Hall) is entered by an
        // entrance: flood from where it arrives instead.
        let arrival = ENTRANCES.iter().find_map(|o| match o.to {
            Dest::Room(kind, (x, y)) if kind == e.room => Some((x as i32, y as i32)),
            _ => None,
        });
        let start = (0..4)
            .find(|&d| room.doors[d])
            .map(|d| [(11, 0), (23, 6), (11, 13), (0, 6)][d])
            .or(arrival)
            .unwrap();
        let mut seen = vec![false; room.cols * room.rows];
        let mut todo = vec![start];
        while let Some((x, y)) = todo.pop() {
            if x < 0 || y < 0 || x >= room.cols as i32 || y >= room.rows as i32 {
                continue;
            }
            let k = y as usize * room.cols + x as usize;
            if seen[k] || !matches!(room.tile(x, y), Tile::Floor | Tile::Door | Tile::Stairs) {
                continue;
            }
            seen[k] = true;
            todo.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
        assert!(
            seen[r as usize * room.cols + c as usize],
            "{} unreachable",
            e.id
        );
    }
}

#[test]
fn the_gates_stair_goes_down_to_the_undercroft_and_its_stair_comes_back_up() {
    let mut run = at_the_gate();
    take(&mut run, entrance("gate"));
    assert_eq!(run.room().kind, RoomKind::Home);
    assert_eq!(run.dungeon.depth, 0, "the Undercroft is still home");
    for hero in run.players.values() {
        assert!(walkable(run.room(), hero.x, hero.y));
    }
    // Arriving beside the stair up does not bounce the party back up.
    for _ in 0..DESCEND_HOLD * 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.room().kind, RoomKind::Home);
    take(&mut run, entrance("cellar"));
    assert_eq!(run.room().kind, RoomKind::Gate);
}

#[test]
fn a_moment_on_the_stair_is_needed_and_a_step_off_cancels_it() {
    let mut run = at_the_gate();
    run.step(&BTreeMap::new());
    let (x, y) = entrance_centre(entrance("gate"));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x, y);
    for _ in 0..DESCEND_HOLD / 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.room().kind, RoomKind::Gate);
    assert!(run.descending > 0);
    let hero = run.players.get_mut(&1).unwrap();
    hero.y += 4.0;
    run.step(&BTreeMap::new());
    assert_eq!(run.descending, 0);
}

#[test]
fn the_winding_stair_in_the_undercroft_still_goes_down() {
    let mut run = at_the_gate();
    take(&mut run, entrance("gate"));
    run.step(&BTreeMap::new());
    let (c, r, w, h) = home::STAIRWELL;
    let (x, y) = (
        (c as f32 + w as f32 / 2.0) * TILE_UNITS,
        (r as f32 + h as f32 / 2.0) * TILE_UNITS,
    );
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (x, y);
    }
    for _ in 0..DESCEND_HOLD + 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.dungeon.depth, 1);
}

#[test]
fn the_mines_shaft_goes_straight_down_into_the_mines() {
    let mut run = at_the_gate();
    run.dungeon.pack = Pack::Crypt;
    let head = room_of(&run.dungeon, RoomKind::MineHead).unwrap();
    run.arrive(head, (12.0, 9.0));
    take(&mut run, entrance("shaft"));
    assert_eq!(run.dungeon.depth, 1);
    assert_eq!(run.dungeon.pack, Pack::Cavern);
    assert!(run.cues.iter().any(|c| c == "run_start"));
    assert!(run.cues.iter().any(|c| c == "entrance:shaft"));
}

#[test]
fn the_worlds_doors_join_its_rooms() {
    let mut run = at_the_gate();
    let gate = run.at;
    for (dir, kind) in [
        (3usize, RoomKind::Stables),
        (1, RoomKind::Lists),
        (0, RoomKind::MineHead),
    ] {
        let next = run.dungeon.neighbour(gate, dir).unwrap();
        assert_eq!(run.dungeon.rooms[next].kind, kind);
        assert!(
            run.dungeon.rooms[next].doors[(dir + 2) % 4],
            "{kind:?} opens back"
        );
    }
    assert!(
        !run.dungeon.rooms[gate].doors[2],
        "the realm's road is not a room"
    );
}

#[test]
fn a_mirror_of_the_world_follows_the_host_through_the_gate() {
    let mut host = at_the_gate();
    let wire = serde_json::to_vec(&host).unwrap();
    let mut friend: Run = serde_json::from_slice(&wire).unwrap();
    take(&mut host, entrance("gate"));
    assert!(friend.apply_live(host.live()));
    assert_eq!(friend.room().kind, RoomKind::Home);
}

#[test]
fn an_older_checkpoint_gets_its_world_when_home_is_rebuilt() {
    let mut run = Run::at_home(7, 1, None, Home::default(), Default::default());
    run.dungeon.rooms.retain(|r| !r.kind.in_world());
    let (c, r, _, _) = CELLAR_STAIR;
    run.dungeon.rooms[0].set(c as usize, r as usize, Tile::Wall);
    run.rebuild_home(Home::default(), Default::default());
    assert!(room_of(&run.dungeon, RoomKind::Gate).is_some());
    assert_eq!(run.dungeon.rooms[0].tile(c, r), Tile::Stairs);
}
