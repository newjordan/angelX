//! The gauntlets: the axe bridge, the hammer row, the boulder run and the
//! saw rail; their timing, their blows, the gorge, the bots that cross
//! them, a friend's mirror, and old saves.

use super::*;
use std::collections::BTreeMap;

/// A run standing in a straight passage of floor `depth` (north to south
/// when `upright`), nothing in it, the knight warded off, the passage as
/// the floor built it with no gauntlet laid yet.
fn passage(upright: bool, depth: u32) -> Run {
    let want = if upright { [true, false, true, false] } else { [false, true, false, true] };
    for seed in 0..400u64 {
        let mut run = Run::new(seed, 3, None);
        run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, depth, Pack::at(depth, Pack::Crypt), &mut run.rng);
        let Some(i) = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Hall && r.doors == want) else {
            continue;
        };
        let pristine = run.dungeon.rooms[i].clone();
        run.enter_for_test(i);
        run.dungeon.rooms[i] = pristine;
        run.calm_for_test();
        run.enemies.clear();
        run.works.gauntlets.clear();
        run.works.perils.clear();
        run.window = None;
        return run;
    }
    panic!("no straight passage on any floor tried");
}

/// The same passage with its bridge and axes laid.
fn bridged(upright: bool, depth: u32) -> Run {
    let mut run = passage(upright, depth);
    let tier = (depth.min(3) - 1) as usize;
    run.lay_bridge(upright, tier, &mut Rng::new(7));
    assert!(matches!(run.works.gauntlets[0], Gauntlet::Bridge { .. }));
    run
}

fn bridge_of(run: &Run) -> (f32, f32, f32, bool) {
    run.works
        .gauntlets
        .iter()
        .find_map(|g| match *g {
            Gauntlet::Bridge { from, to, line, upright } => Some((from, to, line, upright)),
            _ => None,
        })
        .unwrap()
}

fn axes(run: &Run) -> Vec<Gauntlet> {
    run.works.gauntlets.iter().copied().filter(|g| matches!(g, Gauntlet::Axe { .. })).collect()
}

/// A phase that puts a cycle of `period` at `beat` on the run's next tick.
fn phase_for(run: &Run, period: u32, beat: u32) -> u32 {
    let next = (run.tick + 1) % u64::from(period);
    ((u64::from(beat) + u64::from(period) - next) % u64::from(period)) as u32
}

fn place(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = (x, y);
    hero.invulnerable = 0;
    hero.hp = hero.max_hp;
    hero.footing = None;
}

fn foe(run: &mut Run, kind: EnemyKind, x: f32, y: f32) -> u32 {
    run.spawn_at_for_test(kind, x, y);
    let e = run.enemies.last_mut().unwrap();
    e.age = TELEGRAPH;
    e.hp = 10_000;
    e.max_hp = 10_000;
    e.seen_hp = 10_000;
    e.frozen = 10_000;
    e.id
}

fn idle(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

/// The beat of a depth-one swing at which the blade, heading out, passes
/// the edge of the bridge (a knight standing there is struck on this beat).
const EDGE_BEAT: u32 = 7;

/// One axe alone over the bridge's middle, at `beat` of its swing next tick.
fn one_axe(run: &mut Run, beat: u32) -> Gauntlet {
    let (from, to, line, upright) = bridge_of(run);
    let period = AXE_PERIOD[0];
    let mid = (from + to) / 2.0;
    let (x, y) = if upright { (line, mid) } else { (mid, line) };
    let axe = Gauntlet::Axe { x, y, upright, period, phase: phase_for(run, period, beat) };
    run.works.gauntlets.retain(|g| matches!(g, Gauntlet::Bridge { .. }));
    run.works.gauntlets.push(axe);
    axe
}

// ─── the axe bridge ──────────────────────────────────────────────────────────

#[test]
fn a_bridged_passage_is_a_gorge_but_for_its_bridge_and_its_landings_and_stays_whole() {
    for upright in [false, true] {
        let run = bridged(upright, 2);
        let room = run.room();
        let (from, to, line, _) = bridge_of(&run);
        assert!(layout::connected(room), "doorway to doorway, over the bridge");
        let pits = room.tiles_for_test().iter().filter(|&&t| t == Tile::Hazard).count();
        assert!(pits > 60, "a real gorge: {pits} tiles");
        // The bridge is firm all the way; a step off it is the gorge.
        let mut along = from + 0.5;
        while along < to {
            let on = if upright { (line, along) } else { (along, line) };
            let off = if upright { (line + BRIDGE_HALF + 0.6, along) } else { (along, line + BRIDGE_HALF + 0.6) };
            let tile = |(x, y): (f32, f32)| room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
            assert_eq!(tile(on), Tile::Floor, "upright {upright} at {along}");
            assert_eq!(tile(off), Tile::Hazard, "upright {upright} at {along}");
            along += 1.0;
        }
        // Each doorway keeps firm ground in front of it.
        for d in 0..4 {
            if room.doors[d] {
                let ((x, y), (ix, iy)) = hazards::doorway(room, d);
                for k in 0..3 {
                    let (px, py) = (x + ix * k as f32, y + iy * k as f32);
                    let tile = room.tile((px / TILE_UNITS) as i32, (py / TILE_UNITS) as i32);
                    assert!(tile.ground() || tile == Tile::Door, "landing by doorway {d}");
                }
            }
        }
        assert!((2..=5).contains(&axes(&run).len()));
    }
}

#[test]
fn an_axe_is_high_at_the_ends_of_its_swing_and_comes_down_only_over_the_bridge() {
    let run = bridged(false, 1);
    let axe = axes(&run)[0];
    let Gauntlet::Axe { y, period, .. } = axe else { unreachable!() };
    let mut lows = 0;
    for t in 0..u64::from(period) * 2 {
        let (_, by) = axe.blade(t);
        if axe.low(t) {
            lows += 1;
            assert!((by - y).abs() <= BRIDGE_HALF + 0.2, "down among the knights only over the bridge");
            assert!(axe.warning(t).is_none());
        } else {
            assert!((by - y).abs() >= BRIDGE_HALF - 0.3, "high over the gorge at the ends");
        }
    }
    // Twice a cycle, a little under half the time each.
    assert!(lows > period as usize / 4 && lows < period as usize, "{lows}");
    // The shadow warns for a good half second before every descent.
    for t in 1..u64::from(period) * 2 {
        if axe.low(t) && !axe.low(t - 1) {
            let warned = (1..=15).all(|k| t >= k && axe.warning(t - k).is_some());
            assert!(warned, "the shadow shows before tick {t}");
        }
    }
}

#[test]
fn an_axe_throws_a_knight_the_way_it_swings_and_the_ward_carries_them_through_the_next_pass() {
    let mut run = bridged(false, 1);
    let axe = one_axe(&mut run, 0);
    let Gauntlet::Axe { x, y, .. } = axe else { unreachable!() };
    // The blade comes down from the north (swing -1 to 0 at beat 0... it
    // crosses the middle heading south at beat 0).
    place(&mut run, 1, (x, y));
    let hp = run.players[&1].hp;
    run.step(&BTreeMap::new());
    let hero = &run.players[&1];
    assert!(hero.hp < hp, "struck");
    let heading = axe.heading(run.tick);
    assert!(((hero.y - y) - heading * AXE_KNOCK).abs() < 0.3, "thrown {} along the swing", hero.y - y);
    assert!((hero.x - x).abs() < 0.01, "not along the bridge");
    assert!(run.room().tile((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32) == Tile::Floor, "kept to the middle, still on the bridge");
    // The blade swings back through while the ward holds: nothing more.
    let after = run.players[&1].hp;
    let ward = run.players[&1].invulnerable;
    assert!(ward > 0);
    idle(&mut run, ward.saturating_sub(1));
    assert_eq!(run.players[&1].hp, after, "the ward carries them through");
}

#[test]
fn a_knight_struck_at_the_edge_goes_over_and_comes_back_on_the_footing_they_set_out_from() {
    let mut run = bridged(false, 1);
    let (from, _, line, _) = bridge_of(&run);
    let axe = one_axe(&mut run, 0);
    let Gauntlet::Axe { x, y, .. } = axe else { unreachable!() };
    // They set out from the west landing...
    place(&mut run, 1, (from - 2.0, line));
    run.step(&BTreeMap::new());
    let footing = run.players[&1].footing.expect("firm ground remembered");
    // ...and stand at the bridge's southern edge as the blade comes down
    // heading south, reaching them this tick.
    let axe = one_axe(&mut run, EDGE_BEAT);
    let heading = axe.heading(run.tick + 1);
    assert_eq!(heading, 1.0);
    let edge = y + heading * (BRIDGE_HALF - HERO_RADIUS - 0.05);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (x, edge, 0);
    let hp = hero.hp;
    run.step(&BTreeMap::new());
    let hero = &run.players[&1];
    assert_eq!((hero.x, hero.y), footing, "back where they set out from");
    let fall = hero.max_hp * FALL_SHARE / 100;
    assert!(hp - hero.hp >= fall, "the blow and the fall: {} lost", hp - hero.hp);
    assert!(hp - hero.hp < hero.max_hp * 9 / 10, "punished, never one-shot");
    assert!(hero.invulnerable >= HZ - 1, "a moment's grace on the ledge");
    assert!(run.cues.iter().any(|c| c == "gorge_fall"));
}

#[test]
fn a_fall_with_no_footing_lands_at_the_bridges_nearer_end_and_a_fallen_knight_stays_reachable() {
    let mut run = bridged(true, 1);
    // Two knights, so one falling is not the party's end.
    run.join_seat(2, "Wren");
    let (from, to, line, _) = bridge_of(&run);
    let axe = one_axe(&mut run, EDGE_BEAT);
    let Gauntlet::Axe { x, y, .. } = axe else { unreachable!() };
    let heading = axe.heading(run.tick + 1);
    place(&mut run, 1, (x + heading * (BRIDGE_HALF - HERO_RADIUS - 0.05), y));
    // Nearly spent: the blow takes what is left, and the body lies on the
    // bridge, not in the gorge.
    run.players.get_mut(&1).unwrap().hp = 3;
    run.step(&BTreeMap::new());
    let hero = &run.players[&1];
    assert_eq!(hero.hp, 0);
    assert!(run.room().tile((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32).ground(), "a body where a friend can reach it");
    // Another knight, with no footing kept, goes over and lands at an end.
    let axe = one_axe(&mut run, EDGE_BEAT);
    let heading = axe.heading(run.tick + 1);
    place(&mut run, 2, (x + heading * (BRIDGE_HALF - HERO_RADIUS - 0.05), y));
    run.players.get_mut(&2).unwrap().footing = None;
    // Skip the footing kept this tick: they stand on the span.
    run.step(&BTreeMap::new());
    let hero = &run.players[&2];
    assert!(hero.hp > 0);
    assert!((hero.x - line).abs() < 0.01);
    assert!((hero.y - (from - 1.5)).abs() < 0.01 || (hero.y - (to + 1.5)).abs() < 0.01, "at an end: {}", hero.y);
}

#[test]
fn an_axe_sweeps_a_monster_clean_off_the_bridge_into_the_gorge() {
    let mut run = bridged(false, 1);
    let axe = one_axe(&mut run, 0);
    let Gauntlet::Axe { x, y, .. } = axe else { unreachable!() };
    place(&mut run, 1, (4.0, 14.0));
    let id = foe(&mut run, EnemyKind::Skeleton, x, y);
    run.step(&BTreeMap::new());
    assert!(run.enemies.iter().all(|e| e.id != id || e.hp == 0), "over the edge");
    assert!(run.feats.contains(&"mind_the_gap"));
    // A guardian is too big to sweep: it stands, and takes the cut.
    let mut run = bridged(false, 1);
    let axe = one_axe(&mut run, 0);
    let Gauntlet::Axe { x, y, .. } = axe else { unreachable!() };
    let id = foe(&mut run, EnemyKind::PitTyrant, x, y);
    run.step(&BTreeMap::new());
    let e = run.enemies.iter().find(|e| e.id == id).unwrap();
    assert!(e.hp > 0 && e.hp < 10_000);
    assert_eq!((e.x, e.y), (x, y));
}

// ─── the hammer row ──────────────────────────────────────────────────────────

fn hammered(upright: bool) -> Run {
    let mut run = passage(upright, 3);
    run.lay_hammers(&mut Rng::new(3));
    assert!(run.works.gauntlets.iter().all(|g| matches!(g, Gauntlet::Hammer { .. })));
    assert!(!run.works.gauntlets.is_empty());
    run
}

#[test]
fn hammers_stand_across_the_whole_passage_in_gates_that_fall_one_after_another() {
    for upright in [false, true] {
        let run = hammered(upright);
        let room = run.room();
        let mut gates: BTreeMap<u32, Vec<(f32, f32)>> = BTreeMap::new();
        for g in &run.works.gauntlets {
            let Gauntlet::Hammer { x, y, phase, .. } = *g else { unreachable!() };
            let along = if upright { y } else { x };
            gates.entry(along.to_bits()).or_default().push((x, y));
            // Its footprint is all passage floor.
            for (dx, dy) in [(-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
                assert!(room.tile(((x + dx) / TILE_UNITS) as i32, ((y + dy) / TILE_UNITS) as i32).ground());
            }
        }
        assert!(gates.len() >= 2, "more than one gate (upright {upright})");
        // A gate covers the passage's width: no way round its hammers.
        for hammers in gates.values() {
            let span = hammers.len() as f32 * 2.0 * HAMMER_HALF;
            let legs = run.hall_legs();
            let width = legs.iter().map(|(_, lanes, _)| lanes.len()).max().unwrap() as f32 * TILE_UNITS;
            assert_eq!(span, width);
        }
    }
}

#[test]
fn a_hammer_shudders_before_it_falls_and_throws_whoever_stands_under_it_clear() {
    let mut run = hammered(false);
    let Gauntlet::Hammer { x, y, upright, .. } = run.works.gauntlets[0] else { unreachable!() };
    let phase = phase_for(&run, HAMMER_PERIOD, HAMMER_WARN);
    run.works.gauntlets = vec![Gauntlet::Hammer { x, y, upright, phase }];
    place(&mut run, 1, (x + 0.6, y));
    let id = foe(&mut run, EnemyKind::Skeleton, x - 0.6, y);
    let hp = run.players[&1].hp;
    let mut warned = 0;
    for _ in HAMMER_WARN..HAMMER_SLAM {
        run.step(&BTreeMap::new());
        place(&mut run, 1, (x + 0.6, y));
        run.players.get_mut(&1).unwrap().hp = hp;
        warned += usize::from(run.works.gauntlets[0].warning(run.tick).is_some());
    }
    assert!(warned as u32 >= (HAMMER_SLAM - HAMMER_WARN) - 2, "it shudders for most of a second first");
    assert!(warned as f32 / HZ as f32 >= 0.5);
    assert_eq!(run.players[&1].hp, hp, "the warning hurts nothing");
    run.step(&BTreeMap::new());
    assert_eq!(run.works.gauntlets[0].beat(run.tick), HAMMER_SLAM);
    let hero = &run.players[&1];
    assert!(hero.hp < hp, "struck");
    assert!(hero.x - x >= HAMMER_HALF + HERO_RADIUS, "thrown clear of the footprint: {}", hero.x - x);
    let e = run.enemies.iter().find(|e| e.id == id).unwrap();
    assert!(e.hp < 10_000, "a monster too");
    assert!(x - e.x >= HAMMER_HALF, "and out it goes, the other way");
    // While it lies on the floor, nobody walks into it.
    let walk = BTreeMap::from([(1, Input { move_x: -1, ..Default::default() })]);
    for _ in 0..3 {
        run.step(&walk);
        assert!(run.players[&1].x - x >= HAMMER_HALF + HERO_RADIUS - 0.01);
    }
}

// ─── the boulder run ─────────────────────────────────────────────────────────

/// A cleared fight room with bare floor and nothing in it.
fn hall_room() -> Run {
    let mut run = Run::new(4, 1, None);
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.open_room_for_test(room);
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    run.works.perils.clear();
    run.works.gauntlets.clear();
    run
}

#[test]
fn a_boulder_rumbles_then_rolls_its_lane_and_throws_knights_and_monsters_aside() {
    let mut run = hall_room();
    assert!(run.lay_boulder(1, &mut Rng::new(5)));
    let Gauntlet::Boulder { from, to, y, period, .. } = run.works.gauntlets[0] else { unreachable!() };
    let phase = phase_for(&run, period, 0);
    let boulder = Gauntlet::Boulder { from, to, y, period, phase };
    run.works.gauntlets = vec![boulder];
    let mid = (from + to) / 2.0;
    place(&mut run, 1, (mid, y + 0.4));
    let id = foe(&mut run, EnemyKind::Skeleton, (from + mid) / 2.0, y - 0.3);
    let hp = run.players[&1].hp;
    // The rumble: no boulder yet, and the lane says so.
    for _ in 0..BOULDER_WARN {
        run.step(&BTreeMap::new());
        assert!(boulder.boulder(run.tick).is_none());
        assert!(boulder.warning(run.tick).is_some() || boulder.beat(run.tick) == BOULDER_WARN);
    }
    assert_eq!(run.players[&1].hp, hp);
    let roll = ((to - from).abs() / BOULDER_SPEED) as u32;
    let mut was = from;
    for _ in 0..roll {
        run.step(&BTreeMap::new());
        if let Some((bx, _)) = boulder.boulder(run.tick) {
            assert!((bx - from).abs() >= (was - from).abs(), "it rolls one way");
            was = bx;
        }
    }
    let hero = &run.players[&1];
    assert!(hero.hp < hp, "run down");
    assert!((hero.y - y).abs() >= BOULDER_RADIUS + HERO_RADIUS, "and thrown out of the lane: {}", hero.y - y);
    let e = run.enemies.iter().find(|e| e.id == id).unwrap();
    assert!(e.hp < 10_000, "monsters too");
    assert!((e.y - y).abs() >= BOULDER_RADIUS, "thrown aside");
    // It bursts on the far stone, and the next one waits for its rumble.
    idle(&mut run, 4);
    assert!(boulder.boulder(run.tick).is_none());
}

#[test]
fn a_boulder_lane_comes_out_of_solid_wall_and_never_passes_a_doorway() {
    let mut laid = 0;
    for seed in 0..60u64 {
        let mut run = Run::new(seed, 2, None);
        run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, 2, Pack::Cavern, &mut run.rng);
        for at in 0..run.dungeon.rooms.len() {
            if run.dungeon.rooms[at].kind != RoomKind::Fight {
                continue;
            }
            run.at = at;
            run.works.gauntlets.clear();
            if !run.lay_boulder(1, &mut Rng::new(seed ^ at as u64)) {
                continue;
            }
            laid += 1;
            let Gauntlet::Boulder { from, to, y, .. } = run.works.gauntlets[0] else { unreachable!() };
            let room = run.room();
            let wall = if from < to { 0 } else { room.cols as i32 - 1 };
            let row = (y / TILE_UNITS) as i32;
            assert_eq!(room.tile(wall, row - 1), Tile::Wall);
            assert_eq!(room.tile(wall, row), Tile::Wall);
            let mut x = from.min(to);
            while x <= from.max(to) {
                for dy in [-1.5f32, 1.5] {
                    assert!(room.tile((x / TILE_UNITS) as i32, ((y + dy) / TILE_UNITS) as i32).ground(), "an open lane");
                }
                x += 1.0;
            }
            for d in 0..4 {
                if room.doors[d] {
                    let ((dx, dy), _) = hazards::doorway(room, d);
                    let near = if dx < from.min(to) { (dx - from.min(to)).hypot(dy - y) } else if dx > from.max(to) { (dx - from.max(to)).hypot(dy - y) } else { (dy - y).abs() };
                    assert!(near > 5.0, "clear of doorway {d}");
                }
            }
        }
    }
    assert!(laid > 20, "lanes are found: {laid}");
}

// ─── the saw rail ────────────────────────────────────────────────────────────

#[test]
fn a_saw_revs_at_each_end_runs_its_rail_and_bites_what_it_meets() {
    let mut run = hall_room();
    assert!(run.lay_saw(0, &mut Rng::new(9)));
    let saw = run.works.gauntlets[0];
    let Gauntlet::Saw { a, b, speed, .. } = saw else { unreachable!() };
    // The whole cycle: rev at a, run to b, rev at b, run back.
    let cycle = 2 * (((b.0 - a.0).hypot(b.1 - a.1) / speed).ceil() as u32 + SAW_REST);
    let saw = Gauntlet::Saw { a, b, speed, phase: phase_for(&run, cycle, 0) };
    run.works.gauntlets = vec![saw];
    let mut revs = 0;
    for k in 0..u64::from(cycle) {
        let (p, rest) = saw.saw_at(run.tick + 1 + k);
        assert!(to_segment(p, a, b) < 0.01, "on its rail");
        revs += usize::from(rest.is_some());
    }
    assert_eq!(revs, 2 * SAW_REST as usize, "it revs at both ends");
    assert!(SAW_REST as f32 / HZ as f32 >= 0.5, "a half-second rev before it sets off");
    // A knight standing on the rail's middle is bitten and thrown off it.
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    place(&mut run, 1, mid);
    let hp = run.players[&1].hp;
    let id = foe(&mut run, EnemyKind::Skeleton, a.0 + (b.0 - a.0) * 0.8, a.1 + (b.1 - a.1) * 0.8);
    for _ in 0..cycle {
        run.step(&BTreeMap::new());
    }
    let hero = &run.players[&1];
    assert!(hero.hp < hp, "bitten");
    assert!(to_segment((hero.x, hero.y), a, b) >= SAW_KNOCK - 0.05, "thrown off the rail");
    assert!(run.enemies.iter().find(|e| e.id == id).unwrap().hp < 10_000, "monsters too");
}

// ─── where they are laid ─────────────────────────────────────────────────────

#[test]
fn gauntlets_come_by_depth_and_pack_never_touch_the_dice_and_leave_passages_whole() {
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    let mut halls = (0, 0);
    let mut rooms = (0, 0);
    for depth in 1..=6u32 {
        for seed in 0..30u64 {
            let mut run = Run::new(seed, 9, None);
            run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, depth, Pack::at(depth, Pack::Crypt), &mut run.rng);
            for at in 0..run.dungeon.rooms.len() {
                let kind = run.dungeon.rooms[at].kind;
                if !matches!(kind, RoomKind::Hall | RoomKind::Fight) {
                    continue;
                }
                let rng_before = serde_json::to_string(&run.rng).unwrap();
                run.at = at;
                run.traps.clear();
                run.works.perils.clear();
                run.arm_gauntlets();
                assert_eq!(serde_json::to_string(&run.rng).unwrap(), rng_before, "the run's dice are never drawn");
                let laid = !run.works.gauntlets.is_empty();
                if kind == RoomKind::Hall {
                    halls = (halls.0 + 1, halls.1 + usize::from(laid));
                } else {
                    rooms = (rooms.0 + 1, rooms.1 + usize::from(laid));
                }
                assert!(layout::connected(run.room()), "every doorway still reaches every other");
                for g in &run.works.gauntlets {
                    assert!(g.valid());
                    let name = match g {
                        Gauntlet::Bridge { .. } => "bridge",
                        Gauntlet::Axe { .. } => "axe",
                        Gauntlet::Hammer { .. } => "hammer",
                        Gauntlet::Boulder { .. } => "boulder",
                        Gauntlet::Saw { .. } => "saw",
                    };
                    *kinds.entry(name).or_default() += 1;
                    if kind == RoomKind::Fight {
                        assert!(matches!(g, Gauntlet::Boulder { .. } | Gauntlet::Saw { .. }));
                    }
                }
                // Laying it twice lays the same: a mirror and a return agree.
                let first = run.works.gauntlets.clone();
                let tiles = run.room().tiles_for_test().to_vec();
                run.arm_gauntlets();
                assert_eq!(run.works.gauntlets, first);
                assert_eq!(run.room().tiles_for_test(), &tiles[..]);
            }
        }
    }
    eprintln!("halls {halls:?} rooms {rooms:?} kinds {kinds:?}");
    for k in ["bridge", "axe", "hammer", "boulder", "saw"] {
        assert!(kinds.get(k).copied().unwrap_or(0) > 0, "{k} appears");
    }
    assert!(halls.1 * 2 > halls.0, "most passages are gauntlets: {halls:?}");
    assert!(rooms.1 * 8 > rooms.0 && rooms.1 * 2 < rooms.0, "some fighting rooms: {rooms:?}");
}

#[test]
fn no_gauntlet_on_an_open_floor_in_the_swamp_or_in_a_follys_passage() {
    let mut run = Run::new(5, 5, None);
    run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, 2, Pack::Swamp, &mut run.rng);
    for at in 0..run.dungeon.rooms.len() {
        run.at = at;
        run.arm_gauntlets();
        assert!(run.works.gauntlets.is_empty());
    }
    let mut run = Run::new(5, 5, None);
    run.open_floor_for_test(2);
    run.arm_gauntlets();
    assert!(run.works.gauntlets.is_empty());
    // A passage a game is played in (the folly, the gantry, a race) keeps to it.
    for upright in [false, true] {
        let run = passage(upright, 2);
        for game in 0..3 {
            let mut run = run.clone();
            let at = Some(run.at);
            match game {
                0 => run.folly_room = at,
                1 => run.gantry_room = at,
                _ => run.race3d_room = at,
            }
            run.arm_gauntlets();
            assert!(run.works.gauntlets.is_empty());
        }
    }
    // And a room without one is written as it always was.
    let run = passage(false, 2);
    assert!(!serde_json::to_string(&run).unwrap().contains("gauntlets"));
}

// ─── crossing ────────────────────────────────────────────────────────────────

/// Walk knight `id` from one end of the passage to the other: along it,
/// keeping to its middle line. With `wait`, hold back while the next
/// danger ahead is down or warning. True if they reach the far landing
/// within `secs`; and how many falls on the way.
fn cross(run: &mut Run, id: u32, upright: bool, line: f32, goal: f32, wait: bool, secs: u32) -> (bool, u32) {
    let mut falls = 0;
    for _ in 0..secs * HZ {
        let hero = &run.players[&id];
        if hero.hp == 0 {
            return (false, falls);
        }
        let (along, across) = if upright { (hero.y, hero.x) } else { (hero.x, hero.y) };
        if (along - goal).abs() < 1.0 {
            return (true, falls);
        }
        let ahead = (goal - along).signum();
        let mut go = ahead as i8;
        if wait {
            // The next danger within a few steps ahead: wait it out.
            let tick = run.tick + 1;
            let threat = run.works.gauntlets.iter().any(|g| {
                let (gx, gy) = g.at();
                let ga = if upright { gy } else { gx };
                let d = (ga - along) * ahead;
                // Wait at the edge of its reach, never inside it: once in,
                // keep going.
                let (reach, ahead_ticks) = match g {
                    Gauntlet::Axe { .. } => (AXE_EDGE + HERO_RADIUS + 0.2, 11),
                    Gauntlet::Hammer { .. } => (HAMMER_HALF + HERO_RADIUS + 0.2, 15),
                    _ => return false,
                };
                let near = d > reach && d < reach + 1.0;
                let hot = (0..=ahead_ticks).any(|k| g.low(tick + k) || g.down(tick + k));
                near && hot
            });
            if threat {
                go = 0;
            }
        }
        let steer = if (across - line).abs() > 0.25 { (line - across).signum() as i8 } else { 0 };
        let input = if upright {
            Input { move_x: steer, move_y: go, ..Default::default() }
        } else {
            Input { move_x: go, move_y: steer, ..Default::default() }
        };
        let before = run.cues.len();
        run.step(&BTreeMap::from([(id, input)]));
        falls += run.cues[before.min(run.cues.len())..].iter().filter(|c| *c == "gorge_fall").count() as u32;
    }
    (false, falls)
}

#[test]
fn a_bot_that_walks_straight_over_crosses_every_bridge_from_full_health_with_the_ward() {
    for upright in [false, true] {
        for depth in [1u32, 2, 3] {
            for seed in 0..4u64 {
                let mut run = bridged(upright, depth);
                let tier = (depth.min(3) - 1) as usize;
                run.works.gauntlets.clear();
                run.lay_bridge(upright, tier, &mut Rng::new(seed * 31 + 1));
                let (from, to, line, _) = bridge_of(&run);
                for (start, goal) in [(from - 2.0, to + 2.0), (to + 2.0, from - 2.0)] {
                    let at = if upright { (line, start) } else { (start, line) };
                    place(&mut run, 1, at);
                    let (made, falls) = cross(&mut run, 1, upright, line, goal, false, 30);
                    assert!(made, "upright {upright} depth {depth} seed {seed}: crossed ({falls} falls)");
                    assert!(run.players[&1].hp > 0);
                }
            }
        }
    }
}

#[test]
fn a_bot_that_waits_for_its_moment_crosses_untouched_at_any_depth() {
    for upright in [false, true] {
        for depth in [1u32, 3, 6] {
            let mut run = bridged(upright, depth);
            let (from, to, line, _) = bridge_of(&run);
            let at = if upright { (line, from - 2.0) } else { (from - 2.0, line) };
            place(&mut run, 1, at);
            let hp = run.players[&1].hp;
            let (made, falls) = cross(&mut run, 1, upright, line, to + 2.0, true, 40);
            assert!(made && falls == 0, "upright {upright} depth {depth}");
            assert_eq!(run.players[&1].hp, hp, "timed right, never touched (upright {upright} depth {depth})");
        }
    }
    // The hammer row too.
    for upright in [false, true] {
        let mut run = hammered(upright);
        let legs = run.hall_legs();
        let (_, lanes, _) = legs.iter().max_by_key(|l| l.2.len()).unwrap().clone();
        let line = (lanes[lanes.len() / 2 - 1] as f32 + 1.0) * TILE_UNITS;
        let long = if upright { run.room().height() } else { run.room().width() };
        let at = if upright { (line, 2.5) } else { (2.5, line) };
        place(&mut run, 1, at);
        let hp = run.players[&1].hp;
        let (made, _) = cross(&mut run, 1, upright, line, long - 2.5, true, 40);
        assert!(made, "through the hammers (upright {upright})");
        assert_eq!(run.players[&1].hp, hp, "with the wave, untouched (upright {upright})");
    }
}

#[test]
fn a_bot_that_walks_straight_through_the_hammers_makes_it_too() {
    for upright in [false, true] {
        for seed in 0..4u64 {
            let mut run = passage(upright, 3);
            run.lay_hammers(&mut Rng::new(seed + 11));
            let legs = run.hall_legs();
            let (_, lanes, _) = legs.iter().max_by_key(|l| l.2.len()).unwrap().clone();
            let line = (lanes[lanes.len() / 2 - 1] as f32 + 1.0) * TILE_UNITS;
            let long = if upright { run.room().height() } else { run.room().width() };
            let at = if upright { (line, 2.5) } else { (2.5, line) };
            place(&mut run, 1, at);
            let (made, _) = cross(&mut run, 1, upright, line, long - 2.5, false, 30);
            assert!(made, "upright {upright} seed {seed}");
            assert!(run.players[&1].hp > 0);
        }
    }
}

// ─── the wire ────────────────────────────────────────────────────────────────

#[test]
fn a_friends_mirror_sees_every_blade_boulder_and_hammer_where_the_host_does() {
    let mut run = bridged(false, 2);
    place(&mut run, 1, (4.0, 14.0));
    let mut room = hall_room();
    room.lay_boulder(1, &mut Rng::new(5));
    room.lay_saw(1, &mut Rng::new(6));
    for host in [&mut run, &mut room] {
        for _ in 0..90 {
            host.step(&BTreeMap::new());
            let mirror = host.wire_copy();
            assert_eq!(mirror.works.gauntlets, host.works.gauntlets);
            assert_eq!(mirror.room().tiles_for_test(), host.room().tiles_for_test(), "the gorge rides the wire");
            for (g, m) in host.works.gauntlets.iter().zip(&mirror.works.gauntlets) {
                assert_eq!(g.blade(host.tick), m.blade(mirror.tick));
                assert_eq!(g.boulder(host.tick), m.boulder(mirror.tick));
                assert_eq!(g.saw_at(host.tick), m.saw_at(mirror.tick));
                assert_eq!(g.drop(host.tick), m.drop(mirror.tick));
            }
        }
    }
}

#[test]
fn the_same_run_plays_its_gauntlets_out_the_same_twice() {
    let play = || {
        let mut run = bridged(true, 3);
        let (from, to, line, _) = bridge_of(&run);
        place(&mut run, 1, (line, from - 2.0));
        let _ = cross(&mut run, 1, true, line, to + 2.0, false, 10);
        let h = &run.players[&1];
        (run.tick, h.x.to_bits(), h.y.to_bits(), h.hp)
    };
    assert_eq!(play(), play());
}

#[test]
fn a_save_from_before_gauntlets_loads_and_lays_its_passages_on_entry() {
    let run = bridged(false, 1);
    let mut save = serde_json::to_value(&run).unwrap();
    save["works"].as_object_mut().unwrap().remove("gauntlets");
    let back: Run = serde_json::from_value(save).unwrap();
    assert!(back.works.gauntlets.is_empty());
    let works: works::Works = serde_json::from_str(r#"{"perils": []}"#).unwrap();
    assert!(works.gauntlets.is_empty() && works.valid());
    // A mirrored list that is not finite is refused.
    let mut bad = run.works.clone();
    bad.gauntlets.push(Gauntlet::Axe { x: f32::NAN, y: 0.0, upright: false, period: 60, phase: 0 });
    assert!(!bad.valid());
}
