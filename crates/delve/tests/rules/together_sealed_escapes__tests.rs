//! A sealed gate holds every knight mover, not only a walk.
//!
//! The knight stands just inside a barred door. Each mover is applied
//! outward. They must not finish outside the chamber or inside a wall.

use super::super::*;
use super::super::gauntlets::throw;
use super::super::items::blink;
use super::super::shoulders::{Flight, THROW_TICKS};
use std::collections::BTreeMap;

const COLS: usize = 12;
const ROWS: usize = 7;
const GATE: usize = 6;
const ROW: usize = 3;

fn door_x() -> f32 {
    GATE as f32 * TILE_UNITS
}

fn lane_y() -> f32 {
    (ROW as f32 + 0.5) * TILE_UNITS
}

/// Just inside the gate, facing out.
fn inside() -> (f32, f32) {
    (door_x() - HERO_RADIUS - 0.06, lane_y())
}

/// Clear floor on the far side of the gate.
fn outside() -> (f32, f32) {
    ((GATE as f32 + 1.5) * TILE_UNITS, lane_y())
}

fn gate_room() -> Room {
    let mut tiles = vec![Tile::Wall; COLS * ROWS];
    for c in 1..COLS - 1 {
        tiles[ROW * COLS + c] = Tile::Floor;
    }
    tiles[ROW * COLS + GATE] = Tile::Door;
    let mut room = Room::kingdom(tiles, COLS, ROWS);
    room.kind = RoomKind::Fight;
    room
}

fn gate_run() -> Run {
    let mut run = Run::new(1, 0, None);
    run.dungeon.depth = 2;
    run.phase = Phase::Fighting;
    run.dungeon.rooms[run.at] = gate_room();
    let (x, y) = inside();
    let mut hero = Hero::new("A", x, y);
    hero.invulnerable = 0;
    run.players.insert(1, hero);
    // A living foe bars the door. Stood well inside, out of the movers' way.
    run.spawn_body(EnemyKind::Slime, 4.0, lane_y());
    assert!(run.barred(), "the gate should be sealed");
    run
}

fn held(run: &Run, id: u32) {
    let h = &run.players[&id];
    let grid = Grid { room: run.room(), barred: true };
    let solid = |x: f32, y: f32| matches!(grid.tile_at(x, y), Tile::Wall | Tile::Block | Tile::Door);
    assert!(!solid(h.x, h.y), "centre in stone at {}, {}", h.x, h.y);
    let r = HERO_RADIUS * 0.7;
    assert!(!solid(h.x + r, h.y) && !solid(h.x - r, h.y), "body in stone at {}, {}", h.x, h.y);
    assert!(h.x + HERO_RADIUS < door_x() + 0.04, "left the chamber at {}", h.x);
}

fn input_out() -> BTreeMap<u32, Input> {
    let mut inputs = BTreeMap::new();
    let mut input = Input::default();
    input.move_x = 1;
    inputs.insert(1, input);
    inputs
}

#[test]
fn a_dash_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let inputs = input_out();
    for _ in 0..40 {
        let hero = run.players.get_mut(&1).unwrap();
        hero.dash_ticks = 20;
        hero.dash_x = 1.0;
        hero.dash_y = 0.0;
        run.knights_turn(&inputs);
    }
    held(&run, 1);
}

#[test]
fn a_blink_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    {
        let hero = run.players.get_mut(&1).unwrap();
        hero.aim_x = 1.0;
        hero.aim_y = 0.0;
        hero.blink = 8;
    }
    let room = &run.dungeon.rooms[run.at];
    let grid = Grid { room, barred: true };
    blink(run.players.get_mut(&1).unwrap(), &grid);
    held(&run, 1);
}

#[test]
fn a_ram_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    let mut deeds = foes::Deeds::default();
    deeds.rams.push((x - 0.2, y, 2.0, 1, (1.0, 0.0)));
    run.apply_deeds(deeds);
    held(&run, 1);
}

#[test]
fn a_mole_erupt_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    let mut doings = critters::Doings::default();
    doings.erupts.push((x - 0.3, y));
    run.apply_critters(doings);
    held(&run, 1);
}

#[test]
fn a_shield_bash_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    let mut doings = critters::Doings::default();
    doings.bashes.push(((x - 0.4, y), (1.0, 0.0)));
    run.apply_critters(doings);
    held(&run, 1);
}

#[test]
fn a_pit_slam_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    run.tick_slams(vec![(x - 0.3, y)]);
    held(&run, 1);
}

#[test]
fn a_yard_thwack_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    let id = run.spawn_body(EnemyKind::Dummy, x - 1.6, y).unwrap();
    let dummy = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
    dummy.hp = dummy.max_hp.saturating_sub(yard::THWACK_AT as u32 + 1);
    dummy.dir.0 = yard::THWACK_AT - 1.0;
    dummy.stage = 0;
    run.tick_yard(&BTreeMap::new());
    held(&run, 1);
}

#[test]
fn a_ravage_toss_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    let mut cast = tide::Ravage::cast((x - tide::RING_GAP, y), 1, 1.0, 8.0, 1, 0);
    cast.age = tide::WARN.saturating_sub(1);
    run.tick_ravage(vec![cast]);
    held(&run, 1);
}

#[test]
fn a_charge_toss_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    let centre = (x - 1.0, y);
    run.signature_deeds(Vec::new(), Vec::new(), vec![(centre, centre, 30.0, 0, (1.0, 0.0), 1)]);
    held(&run, 1);
}

#[test]
fn a_gauntlet_throw_stops_at_a_sealed_gate() {
    let run = gate_run();
    let from = inside();
    let landed = throw(run.room(), from, (1.0, 0.0), 8.0, HERO_RADIUS);
    let mut run = run;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = landed;
    held(&run, 1);
}

#[test]
fn a_shoulder_throw_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let from = inside();
    let to = outside();
    run.shoulders.flights.push(Flight {
        knight: 1,
        by: 1,
        from,
        to,
        age: 0,
        ticks: THROW_TICKS,
        at: run.at,
        depth: run.dungeon.depth,
    });
    for _ in 0..8 {
        run.shoulders_begin(&BTreeMap::new());
    }
    held(&run, 1);
}

#[test]
fn a_shoulder_slam_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let from = inside();
    run.shoulders.flights.push(Flight {
        knight: 1,
        by: 1,
        from,
        to: outside(),
        age: THROW_TICKS.saturating_sub(1),
        ticks: THROW_TICKS,
        at: run.at,
        depth: run.dungeon.depth,
    });
    run.shoulders_begin(&BTreeMap::new());
    held(&run, 1);
}

#[test]
fn a_well_pull_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let foe = run.enemies.iter_mut().next().unwrap();
    foe.kind = EnemyKind::Whirlgrave;
    let (ox, oy) = outside();
    run.wells.push(foes::Well { x: ox, y: oy, left: foes::WELL_SPAN });
    for _ in 0..80 {
        run.tick_wells();
    }
    held(&run, 1);
}

#[test]
fn a_hole_pull_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let owner = run.enemies[0].id;
    let hp = run.enemies[0].hp;
    let (ox, oy) = outside();
    run.holes.push(hollow::Hole {
        x: ox,
        y: oy,
        warn: 0,
        left: 40,
        owner,
        owner_hp: hp,
    });
    for _ in 0..40 {
        run.tick_holes(Vec::new());
    }
    held(&run, 1);
}

#[test]
fn a_lure_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (ox, oy) = outside();
    for _ in 0..80 {
        let mut doings = horrors::Doings::default();
        doings.lures.push((ox, oy));
        run.apply_horrors(doings);
    }
    held(&run, 1);
}

#[test]
fn a_hook_reel_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    let (ox, oy) = outside();
    let foe = run.enemies.iter_mut().next().unwrap();
    foe.x = ox;
    foe.y = oy;
    let by = foe.id;
    run.hooks.push(hunters::Hook {
        x,
        y,
        dir: (1.0, 0.0),
        out: 1.0,
        back: true,
        by,
        caught: Some(1),
    });
    for _ in 0..30 {
        run.tick_hunters();
    }
    held(&run, 1);
}

#[test]
fn a_bladewind_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (ox, oy) = outside();
    let foe = run.enemies.iter_mut().next().unwrap();
    foe.x = ox;
    foe.y = oy;
    let hero = run.players.get_mut(&1).unwrap();
    hero.slashes = 1;
    hero.slash_wait = 0;
    run.tick_ults();
    held(&run, 1);
}

#[test]
fn a_possession_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (x, y) = inside();
    let mut fallen = Hero::new("B", x, y);
    fallen.hp = 0;
    fallen.fell = 1;
    run.players.insert(2, fallen);
    run.tick = skulls::SKULL_SETTLE + 2;
    let hero = run.players.get_mut(&1).unwrap();
    hero.taking = true;
    hero.carrying_skull = Some(2);
    let body = run.enemies.iter_mut().next().unwrap();
    body.x = door_x() + 0.8;
    body.y = lane_y();
    body.hp = 1;
    body.age = TELEGRAPH + 1;
    run.tick_skulls();
    held(&run, 2);
}

/// Rising in a body is the fight's respawn. A corpse in the gate is not a way out.
#[test]
fn a_respawn_stops_at_a_sealed_gate() {
    a_possession_stops_at_a_sealed_gate();
}

#[test]
fn riding_shoulders_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let (ox, oy) = outside();
    let mut carrier = Hero::new("C", ox, oy);
    carrier.invulnerable = 0;
    run.players.insert(3, carrier);
    run.shoulders.stacks.insert(3, vec![1]);
    let room = &run.dungeon.rooms[run.at];
    let grid = Grid { room, barred: true };
    run.shoulders.seat_riders(&grid, &mut run.players);
    held(&run, 1);
}

#[test]
fn setting_down_stops_at_a_sealed_gate() {
    let mut run = gate_run();
    let hero = run.players.get_mut(&1).unwrap();
    hero.aim_x = 1.0;
    hero.aim_y = 0.0;
    run.shoulders.stacks.insert(1, vec![2]);
    let mut rider = Hero::new("B", inside().0, inside().1);
    rider.invulnerable = 0;
    run.players.insert(2, rider);
    run.set_down(1, &[2]);
    held(&run, 2);
}

#[test]
fn open_spots_do_not_place_through_a_sealed_gate() {
    let run = gate_run();
    let spots = run.open_spots(run.at, inside(), 6);
    let grid = Grid { room: run.room(), barred: true };
    for (x, y) in spots {
        assert!(x + HERO_RADIUS < door_x() + 0.04, "spot outside at {x}, {y}");
        assert!(
            !matches!(grid.tile_at(x, y), Tile::Wall | Tile::Block | Tile::Door),
            "spot in stone at {x}, {y}"
        );
    }
}

#[test]
fn settling_off_a_pit_stops_at_a_sealed_gate() {
    let mut run = Run::new(1, 0, None);
    run.dungeon.depth = 2;
    run.phase = Phase::Fighting;
    let mut tiles = vec![Tile::Wall; COLS * ROWS];
    for c in 1..COLS - 1 {
        tiles[ROW * COLS + c] = Tile::Floor;
    }
    tiles[ROW * COLS + GATE] = Tile::Door;
    let (x, _) = inside();
    let col = (x / TILE_UNITS) as usize;
    tiles[ROW * COLS + col] = Tile::Hazard;
    let mut room = Room::kingdom(tiles, COLS, ROWS);
    room.kind = RoomKind::Fight;
    run.dungeon.rooms[run.at] = room;
    let (x, y) = inside();
    let mut hero = Hero::new("A", x, y);
    hero.invulnerable = 0;
    run.players.insert(1, hero);
    run.spawn_body(EnemyKind::Slime, 4.0, lane_y());
    run.settle_off_pits();
    held(&run, 1);
}

#[test]
fn a_boss_door_hold_stops_at_a_sealed_gate() {
    let mut run = Run::new(1, 0, None);
    run.dungeon.depth = 2;
    run.phase = Phase::Fighting;
    let mut tiles = vec![Tile::Wall; COLS * ROWS];
    for c in 0..COLS - 1 {
        tiles[ROW * COLS + c] = Tile::Floor;
    }
    tiles[ROW * COLS + 1] = Tile::Door;
    let mut room = Room::kingdom(tiles, COLS, ROWS);
    room.kind = RoomKind::Fight;
    run.dungeon.rooms[run.at] = room;
    let mut hero = Hero::new("A", 0.7, lane_y());
    hero.invulnerable = 0;
    run.players.insert(1, hero);
    run.spawn_body(EnemyKind::Slime, 8.0, lane_y());
    assert!(run.barred());
    let _ = run.boss_door_holds(1, 0);
    let h = &run.players[&1];
    let grid = Grid { room: run.room(), barred: true };
    assert!(
        !matches!(grid.tile_at(h.x, h.y), Tile::Wall | Tile::Block | Tile::Door),
        "held into stone at {}, {}",
        h.x,
        h.y
    );
    assert!(h.x + HERO_RADIUS < 2.0 + 0.04, "pushed through the gate at {}", h.x);
}
