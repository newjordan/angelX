//! The living cave in play: knights go in by a mouth, dig, blast, slide and
//! come out by the lamp; a guest reads the same cave; stone goes home.

use super::*;
use crate::render::overworld::arena::{frame_for, party, seat_frame};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;
use std::path::Path;

fn seat_at(host: &Run, who: u32) -> (i32, i32) {
    let seat = &host.caves.seats[&who];
    (i32::from(seat.x), i32::from(seat.y))
}

fn place(host: &mut Run, who: u32, at: (i32, i32)) {
    let seat = host.caves.seats.get_mut(&who).expect("seat");
    seat.x = at.0 as i16;
    seat.y = at.1 as i16;
    seat.cool = 0;
    seat.coast = 0;
    seat.step_dx = 0;
    seat.step_dy = 0;
}

/// The nearest open cell, by walking, that has a cell `wanted` beside it:
/// where to stand, which way to face, and that cell.
fn stand_beside(
    host: &Run,
    from: (i32, i32),
    wanted: impl Fn(&world::Cell) -> bool,
) -> Option<((i32, i32), (i32, i32), (i32, i32))> {
    stand_facing(host, from, wanted, |_, _| true)
}

/// `stand_beside`, where the stand and the way faced also suit `site`.
fn stand_facing(
    host: &Run,
    from: (i32, i32),
    wanted: impl Fn(&world::Cell) -> bool,
    site: impl Fn((i32, i32), (i32, i32)) -> bool,
) -> Option<((i32, i32), (i32, i32), (i32, i32))> {
    let cave = host.caves.view();
    let exit = exit_cell(host.caves.seed);
    let mut q = std::collections::VecDeque::from([from]);
    let mut seen = std::collections::HashSet::from([from]);
    while let Some((x, y)) = q.pop_front() {
        for (dx, dy) in [(0, -1), (1, 0), (-1, 0), (0, 1)] {
            let n = (x + dx, y + dy);
            if cave.at(n.0, n.1).is_some_and(|c| wanted(c)) && site((x, y), (dx, dy)) {
                return Some(((x, y), (dx, dy), n));
            }
            if n != exit && !seen.contains(&n) && cave.at(n.0, n.1).is_some_and(|c| c.open()) {
                seen.insert(n);
                q.push_back(n);
            }
        }
    }
    None
}

/// The cells to walk from `from` to `to`, not counting `from`; never over the lamp unless it is `to`.
fn path(host: &Run, from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let cave = host.caves.view();
    let exit = exit_cell(host.caves.seed);
    let mut q = std::collections::VecDeque::from([from]);
    let mut prev = std::collections::HashMap::from([(from, from)]);
    while let Some(at) = q.pop_front() {
        if at == to {
            break;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (at.0 + dx, at.1 + dy);
            let open = cave.at(n.0, n.1).is_some_and(|c| c.open());
            if prev.contains_key(&n) || !open || (n == exit && n != to) {
                continue;
            }
            prev.insert(n, at);
            q.push_back(n);
        }
    }
    let mut out = Vec::new();
    let mut at = to;
    while at != from {
        out.push(at);
        at = *prev.get(&at).expect("a way through");
    }
    out.reverse();
    out
}

/// Walk `who` to `to` on real inputs, a cell every `MOVE_EVERY` ticks. `each` sees the run after every tick.
fn walk(host: &mut Run, who: u32, to: (i32, i32), each: &mut dyn FnMut(&Run)) {
    for next in path(host, seat_at(host, who), to) {
        let at = seat_at(host, who);
        let step = Input { move_x: (next.0 - at.0) as i8, move_y: (next.1 - at.1) as i8, ..Input::default() };
        for _ in 0..=MOVE_EVERY {
            host.step(&BTreeMap::from([(who, step)]));
            each(host);
            if !host.caves.inside(who) || seat_at(host, who) == next {
                break;
            }
        }
        if !host.caves.inside(who) {
            return;
        }
        assert_eq!(seat_at(host, who), next, "a step did not land");
    }
}

fn rest(host: &mut Run, ticks: u32, each: &mut dyn FnMut(&Run)) {
    for _ in 0..ticks {
        host.step(&BTreeMap::new());
        each(host);
    }
}

fn first_floor_at_the_mouth(seed: u64) -> Run {
    let mut host = Run::new(seed, 1, Some("Ash"));
    host.calm_for_test();
    let start = host.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Start).expect("a start room");
    host.at = start;
    host.stamp_cave_mouth();
    let (col, row, _, _) = FLOOR_MOUTH.at;
    for hero in host.players.values_mut() {
        hero.x = (col as f32 + 0.5) * TILE_UNITS;
        hero.y = (row as f32 + 0.5) * TILE_UNITS;
    }
    host
}

#[test]
fn two_knights_share_one_cave_and_stone_comes_home() {
    let mut host = Run::new(7, 1, Some("Ash"));
    let bare = crate::wire::publish(&host).expect("bare publish");
    let bare_bytes = format!("{bare:?}").len();
    host.enter_cave(true);
    assert!(host.caves.inside(1) && host.caves.inside(2));
    let (stand, aim, _) = stand_beside(&host, seat_at(&host, 1), |c| c.wall() && c.loose()).expect("loose rock");
    place(&mut host, 1, stand);
    let dig = Input { fire: true, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() };
    let mut inputs = BTreeMap::from([(1, dig), (2, Input::default())]);
    host.tick = 1;
    host.tick_caves(&inputs);
    assert!(!host.caves.changes.is_empty(), "dig wrote a change");
    let fp = host.caves.fingerprint();
    let (stand, aim, _) = stand_beside(&host, seat_at(&host, 2), |c| c.wall() && c.hard()).expect("hard rock");
    place(&mut host, 2, stand);
    inputs.insert(2, Input { cast: 1, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() });
    host.tick_caves(&inputs);
    assert_ne!(fp, host.caves.fingerprint(), "blast changed the shared cave");
    let wire = serde_json::to_vec(&host.caves).expect("cave field");
    let guest: Field = serde_json::from_slice(&wire).expect("late guest");
    assert_eq!(guest.fingerprint(), host.caves.fingerprint());
    let published = crate::wire::publish(&host).expect("publish");
    let guest_run = crate::wire::read(&published).expect("guest reads the cave");
    assert_eq!(guest_run.caves.fingerprint(), host.caves.fingerprint(), "decoded cave");
    assert_eq!(guest_run.caves.bursts, host.caves.bursts, "the guest draws the same rock breaking");
    assert_eq!(guest_run.caves.seats, host.caves.seats, "the guest draws the same knights");
    assert!(!guest_run.caves.changes.is_empty(), "decoded hole");
    let full_bytes = format!("{published:?}").len();
    assert!(full_bytes > bare_bytes, "cave adds snapshot bytes");
    assert!(wire.len() < 2_000_000, "cave field stays inside a wire chunk");
    eprintln!("snapshot bytes bare={bare_bytes} with_cave={full_bytes} field={}", wire.len());
    let exit = exit_cell(host.caves.seed);
    place(&mut host, 1, exit);
    place(&mut host, 2, exit);
    let before = host.home.stockyard.hauled;
    host.tick_caves(&inputs);
    assert!(!host.caves.inside(1) && !host.caves.inside(2));
    assert!(host.home.stockyard.hauled > before, "stone tally rose");
}

#[test]
fn one_knight_enters_the_mouth_alone() {
    let mut host = first_floor_at_the_mouth(11);
    assert_eq!(host.dungeon.depth, 1, "a new run starts on the first floor");
    if let Some(hero) = host.players.get_mut(&2) {
        hero.x += 2.0 * TILE_UNITS;
        hero.y += 2.0 * TILE_UNITS;
    }
    let hold = Input { interact: true, ..Input::default() };
    for _ in 0..40 {
        host.step(&BTreeMap::from([(1, hold), (2, Input::default())]));
        if host.caves.inside(1) {
            break;
        }
    }
    assert!(host.caves.inside(1), "the knight on the mouth never went in");
    assert!(!host.caves.inside(2), "the knight off the mouth was pulled in");
}

#[test]
fn the_party_arrives_apart_and_not_on_the_lamp() {
    for seed in [1u64, 7, 11, 99] {
        let mut host = Run::new(seed, 1, Some("Ash"));
        host.enter_cave(false);
        let (a, b) = (seat_at(&host, 1), seat_at(&host, 2));
        assert_ne!(a, b, "seed {seed}: two knights on one cell");
        assert_eq!((a.0 - b.0).abs() + (a.1 - b.1).abs(), 2, "seed {seed}: not two cells apart");
        let exit = exit_cell(host.caves.seed);
        assert!(a != exit && b != exit, "seed {seed}: a knight arrived on the lamp");
        assert!(host.caves.inside(1) && host.caves.inside(2));
    }
}

#[test]
fn a_held_shovel_cracks_loose_rock_twice_then_breaks_it() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_cave(false);
    let (stand, aim, wall) = stand_beside(&host, seat_at(&host, 1), |c| c.wall() && c.loose()).expect("loose rock");
    place(&mut host, 1, stand);
    let dig = Input { fire: true, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() };
    let mut swings = Vec::new();
    let mut hit_ticks = Vec::new();
    for _ in 0..3 * u32::from(SWING_EVERY) {
        let before = host.caves.bursts.len();
        host.step(&BTreeMap::from([(1, dig)]));
        if host.caves.bursts.len() > before {
            hit_ticks.push(host.tick);
            let cell = host.caves.cell(wall.0, wall.1).expect("cell");
            swings.push((cell.wall(), cell.swings));
        }
    }
    assert_eq!(swings, [(true, 2), (true, 1), (false, world::SHOVEL_SWINGS)], "two cracks, then open");
    assert_eq!(hit_ticks[1] - hit_ticks[0], u64::from(SWING_EVERY), "a held key is one hit a swing");
    let kinds: Vec<u8> = host.caves.bursts.iter().map(|b| b.kind).collect();
    assert_eq!(kinds, [BURST_CHIP, BURST_CHIP, BURST_CRUMBLE]);
    assert!(host.caves.bursts.iter().all(|b| (i32::from(b.x), i32::from(b.y)) == wall));
    assert_eq!(host.caves.seats[&1].pack().total(), 1, "one stone for the broken cell");
    let open = host.caves.cell(wall.0, wall.1).expect("cell");
    assert!(open.open() && open.rubble(), "the broken cell is rubble to walk on");
}

#[test]
fn hard_rock_rings_under_the_shovel_and_stands() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_cave(false);
    let (stand, aim, wall) =
        stand_beside(&host, seat_at(&host, 1), |c| c.wall() && c.hard() && !c.loose()).expect("hard rock");
    place(&mut host, 1, stand);
    let dig = Input { fire: true, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() };
    for _ in 0..4 * u32::from(SWING_EVERY) {
        host.step(&BTreeMap::from([(1, dig)]));
    }
    assert!(host.caves.cell(wall.0, wall.1).is_some_and(|c| c.wall() && c.hard()), "the shovel opened hard rock");
    assert!(!host.caves.bursts.is_empty() && host.caves.bursts.iter().all(|b| b.kind == BURST_CHIP));
    assert_eq!(host.caves.seats[&1].pack().total(), 0);
}

#[test]
fn a_blast_breaks_hard_rock_and_its_bursts_die_away() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_cave(false);
    let (stand, aim, wall) =
        stand_beside(&host, seat_at(&host, 1), |c| c.wall() && c.hard() && !c.loose()).expect("hard rock");
    place(&mut host, 1, stand);
    let cast = Input { cast: 1, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() };
    host.step(&BTreeMap::from([(1, cast)]));
    let tick = host.tick as u32 - 1;
    assert!(host.caves.cell(wall.0, wall.1).is_some_and(|c| !c.wall()), "the blast left the rock");
    assert_eq!(host.caves.seats[&1].magic, world::MAGIC_MAX - world::BLAST_COST);
    let bursts = host.caves.bursts.clone();
    assert_eq!(bursts.iter().filter(|b| b.kind == BURST_BLAST).count(), 1);
    let blast = bursts.iter().find(|b| b.kind == BURST_BLAST).unwrap();
    assert_eq!((i32::from(blast.x), i32::from(blast.y), blast.at), (wall.0, wall.1, tick));
    for crumble in bursts.iter().filter(|b| b.kind == BURST_CRUMBLE) {
        let here = (i32::from(crumble.x), i32::from(crumble.y));
        assert!(host.caves.cell(here.0, here.1).is_some_and(|c| !c.wall()), "a crumble where no rock broke");
        assert_eq!(crumble.at, if here == wall { tick } else { tick + 2 }, "the outer breaks follow the middle");
    }
    assert!(bursts.iter().any(|b| b.kind == BURST_CRUMBLE && (i32::from(b.x), i32::from(b.y)) == wall));
    // Held, the cast does not go off again until the knight has rested.
    host.step(&BTreeMap::from([(1, cast)]));
    assert_eq!(host.caves.seats[&1].magic, world::MAGIC_MAX - world::BLAST_COST, "a second blast the next tick");
    rest(&mut host, BURST_KEPT + 4, &mut |_| {});
    assert!(host.caves.bursts.is_empty(), "old bursts are dropped");
    let quiet = serde_json::to_string(&host.caves).expect("cave field");
    assert!(!quiet.contains("bursts"), "a quiet cave sends no bursts");
}

#[test]
fn a_cast_opens_hard_rock_in_front_of_the_knight() {
    let mut host = Run::new(7, 1, Some("Ash"));
    host.enter_cave(true);
    assert_eq!(host.caves.seats[&1].magic, world::MAGIC_MAX);
    host.tick = 1;
    for n in 0..3 {
        let (stand, aim, wall) =
            stand_beside(&host, seat_at(&host, 1), |c| c.wall() && c.hard()).expect("hard rock beside an open cell");
        place(&mut host, 1, stand);
        let cast = Input { cast: 1, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() };
        host.tick_caves(&BTreeMap::from([(1, cast)]));
        assert!(host.caves.cell(wall.0, wall.1).is_some_and(|c| !c.wall()), "cast {n} left the hard rock sealed");
        // Past the rest after a blast, on ticks the meter does not refill on.
        host.tick += 33;
    }
    assert_eq!(host.caves.seats[&1].magic, 0, "three blasts empty the meter");
    host.tick = 104;
    host.tick_caves(&BTreeMap::from([(1, Input::default())]));
    assert_eq!(host.caves.seats[&1].magic, 1, "the meter refills one point every 8 ticks");
}

#[test]
fn a_second_seats_inputs_dig_walk_blast_and_pay_home() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_cave(false);
    let (stand, aim, _) = stand_beside(&host, seat_at(&host, 2), |c| c.wall() && c.loose()).expect("loose rock");
    place(&mut host, 2, stand);
    let before = host.home.stockyard.hauled;
    let dig = Input { fire: true, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() };
    for _ in 0..3 * u32::from(SWING_EVERY) {
        host.step(&BTreeMap::from([(2, dig)]));
    }
    assert!(host.caves.seats[&2].pack().total() > 0, "the guest's shovel dug no stone");
    let was = seat_at(&host, 2);
    let back = Input { move_x: -aim.0 as i8, move_y: -aim.1 as i8, ..Input::default() };
    for _ in 0..10 {
        host.step(&BTreeMap::from([(2, back)]));
    }
    assert_ne!(was, seat_at(&host, 2), "the guest did not walk");
    let exit = exit_cell(host.caves.seed);
    place(&mut host, 2, exit);
    host.step(&BTreeMap::new());
    assert!(!host.caves.inside(2), "the guest did not leave");
    assert!(host.home.stockyard.hauled > before, "the guest's stone was not paid home");
}

#[test]
fn a_knight_in_the_cave_takes_no_delve_damage() {
    let mut host = Run::new(3, 1, Some("Ash"));
    let hp = host.players[&1].hp;
    host.enter_cave(true);
    assert!(host.caves.inside(1));
    for _ in 0..40 {
        host.step(&BTreeMap::new());
    }
    assert_eq!(host.players[&1].hp, hp);
    assert!(host.caves.inside(1));
}

#[test]
fn a_knight_crosses_a_cell_a_pixel_a_tick() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_cave(false);
    let from = seat_at(&host, 1);
    let east = Input { move_x: 1, ..Input::default() };
    host.step(&BTreeMap::from([(1, east)]));
    let seat = host.caves.seats[&1];
    assert_eq!(seat_at(&host, 1), (from.0 + 1, from.1));
    assert_eq!(seat.stride(), (-i32::from(MOVE_EVERY), 0), "the step starts a whole cell back");
    assert!(seat.stepping() && !seat.gliding());
    let mut last = seat.stride().0;
    for _ in 1..MOVE_EVERY {
        host.step(&BTreeMap::new());
        let now = host.caves.seats[&1].stride().0;
        assert_eq!(now, last + 1, "a pixel a tick");
        last = now;
    }
    host.step(&BTreeMap::new());
    assert_eq!(host.caves.seats[&1].stride(), (0, 0));
    assert!(!host.caves.seats[&1].stepping());
}

#[test]
fn plain_boots_slide_on_ice_and_grip_boots_do_not() {
    for (boots, slides) in [(0u8, true), (1, false)] {
        let mut host = Run::new(11, 1, Some("Ash"));
        host.enter_cave(false);
        let start = ice_run(&host).expect("a run of ice");
        place(&mut host, 1, start);
        host.caves.seats.get_mut(&1).unwrap().footwear = boots;
        let east = Input { move_x: 1, ..Input::default() };
        for _ in 0..2 * u32::from(MOVE_EVERY) {
            host.step(&BTreeMap::from([(1, east)]));
        }
        let let_go = seat_at(&host, 1);
        let mut glided = false;
        for _ in 0..8 * u32::from(MOVE_EVERY) {
            host.step(&BTreeMap::new());
            glided |= host.caves.seats[&1].gliding();
        }
        let carried = seat_at(&host, 1).0 - let_go.0;
        if slides {
            assert!(carried >= 3 && glided, "plain boots were carried {carried} cells");
            assert!(host.caves.seats[&1].slides > 0);
        } else {
            assert_eq!(carried, 0, "grip boots slid");
            assert!(!glided);
        }
    }
}

/// A cell of ice with eight more open ice cells east of it, the row clear of
/// any wall's face: four open cells to its north all the way along.
fn ice_run(host: &Run) -> Option<(i32, i32)> {
    let cave = host.caves.view();
    let ice = |x: i32, y: i32| cave.at(x, y).is_some_and(|c| c.open() && c.ice() && c.fluid == 0);
    (6..world::MAP_H - 2)
        .flat_map(|y| (2..world::MAP_W - 12).map(move |x| (x, y)))
        .find(|&(x, y)| (0..9).all(|d| (0..5).all(|up| ice(x + d, y - up))))
}

fn open_flood(cave: &world::Cave, sx: i32, sy: i32) -> usize {
    let mut seen = std::collections::HashSet::from([(sx, sy)]);
    let mut q = vec![(sx, sy)];
    while let Some((x, y)) = q.pop() {
        for n in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if cave.at(n.0, n.1).is_some_and(|c| c.open()) && seen.insert(n) {
                q.push(n);
            }
        }
    }
    seen.len()
}

/// The fewest loose cells to dig from `(sx, sy)` to open ground in `region`, up to two.
fn digs_into(cave: &world::Cave, sx: i32, sy: i32, region: u8) -> Option<u8> {
    let w = world::MAP_W;
    let h = world::MAP_H;
    let mut best = vec![3u8; (w * h) as usize];
    let mut q = std::collections::VecDeque::new();
    best[(sy * w + sx) as usize] = 0;
    q.push_back((sx, sy));
    let mut found = None;
    while let Some((x, y)) = q.pop_front() {
        let d = best[(y * w + x) as usize];
        if world::region_of(x, y).0 == region && cave.at(x, y).is_some_and(|c| c.open()) {
            found = Some(found.map_or(d, |old: u8| old.min(d)));
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            let Some(cell) = cave.at(nx, ny) else { continue };
            let cost = if cell.open() {
                0
            } else if cell.wall() && !cell.hard() {
                1
            } else {
                continue;
            };
            let nd = d.saturating_add(cost);
            let i = (ny * w + nx) as usize;
            if nd > 2 || nd >= best[i] {
                continue;
            }
            best[i] = nd;
            if cost == 0 {
                q.push_front((nx, ny));
            } else {
                q.push_back((nx, ny));
            }
        }
    }
    found
}

#[test]
fn arrival_is_in_the_open_cavern() {
    for seed in [1u32, 2, 3, 7, 99] {
        let cave = world::Cave::generate(seed);
        let (sx, sy) = spawn_cell(seed);
        assert!(cave.at(sx, sy).is_some_and(|c| c.open()), "seed {seed} arrival sealed");
        assert!(world::region_of(sx, sy).0 == 0, "seed {seed} arrival left the mine");
        assert!(
            sx >= 12 && sy >= 12 && sx < world::MAP_W - 12 && sy < world::MAP_H - 12,
            "seed {seed} arrival is near a map edge"
        );
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for d in 1..=3 {
                assert!(
                    cave.at(sx + dx * d, sy + dy * d).is_some_and(|c| c.open()),
                    "seed {seed} arrival is not three cells deep"
                );
            }
        }
        let flood = open_flood(&cave, sx, sy);
        assert!(flood >= 400, "seed {seed} walkable {flood} < 400");
        let (ex, ey) = exit_cell(seed);
        assert_eq!((ex - sx).abs() + (ey - sy).abs(), 2, "seed {seed} lamp not two cells away");
        assert!(against_wall(&cave, ex, ey), "seed {seed} lamp not against a wall");
    }
}

#[test]
fn every_region_is_reachable_from_arrival() {
    for seed in [1u32, 2, 3, 7, 99] {
        let cave = world::Cave::generate(seed);
        let (sx, sy) = spawn_cell(seed);
        for region in 0..4 {
            let digs = digs_into(&cave, sx, sy, region);
            assert!(digs.is_some_and(|n| n <= 2), "seed {seed} region {region} needs {digs:?} soft digs");
        }
    }
}

#[test]
fn loose_rock_lines_the_caverns_and_hard_rock_lies_behind_it() {
    for seed in [1u32, 7, 11, 99] {
        let cave = world::Cave::generate(seed);
        let mut edge = [0u32; 2];
        let mut deep_loose = 0;
        for y in 1..world::MAP_H - 1 {
            for x in 1..world::MAP_W - 1 {
                let Some(cell) = cave.at(x, y) else { continue };
                if !cell.wall() || cell.brick() {
                    continue;
                }
                let near = (-2..=2i32).any(|dy| {
                    (-2..=2i32).any(|dx| dx.abs() + dy.abs() <= 2 && cave.at(x + dx, y + dy).is_some_and(|c| c.open()))
                });
                assert!(cell.loose() != cell.hard(), "seed {seed} ({x}, {y}) is neither loose nor hard, or both");
                if near {
                    edge[usize::from(cell.loose())] += 1;
                } else if cell.loose() && !world::in_rect(x, y, 12, 33, 16, 37) {
                    deep_loose += 1;
                }
            }
        }
        assert_eq!(deep_loose, 0, "seed {seed}: loose rock deep in the mass");
        let share = edge[1] * 100 / (edge[0] + edge[1]);
        assert!((25..=55).contains(&share), "seed {seed}: {share}% of the caverns' edge is loose");
    }
}

#[test]
fn a_walk_goes_in_digs_blasts_and_leaves_by_the_lamp() {
    play(None);
}

#[test]
#[ignore = "writes the cave's stills, frames and measured costs to ANGEL_CAVE_STILLS"]
fn write_cave_stills() {
    let dir = std::path::PathBuf::from(std::env::var("ANGEL_CAVE_STILLS").expect("ANGEL_CAVE_STILLS"));
    std::fs::create_dir_all(&dir).unwrap();
    play(Some(&dir));
}

/// A seat's whole screen, as a player has it: the party's band, the view, the HUD's band.
fn screen(host: &Run, who: u32) -> crate::ink::Img {
    let (w, h) = party::seat_size();
    seat_frame(host, w as i32, h as i32, Some(who))
}

fn save(dir: &Path, name: &str, im: &crate::ink::Img) {
    image::RgbImage::from_raw(im.w as u32, im.h as u32, im.rgb_bytes())
        .expect("a frame")
        .save(dir.join(name))
        .expect("a still written");
}

/// Every tick's screen for seat 1, appended raw, with where each stretch starts.
struct Film {
    frames: std::fs::File,
    count: u32,
    marks: String,
}

/// The film, if one is being made. Shared by the steps below.
struct Reel(std::cell::RefCell<Option<Film>>);

impl Reel {
    fn shot(&self, host: &Run) {
        use std::io::Write;
        if let Some(film) = self.0.borrow_mut().as_mut() {
            film.frames.write_all(&screen(host, 1).rgb_bytes()).expect("a frame written");
            film.count += 1;
        }
    }

    fn mark(&self, name: &str) {
        if let Some(film) = self.0.borrow_mut().as_mut() {
            film.marks.push_str(&format!("{name} {}\n", film.count));
        }
    }
}

/// Two knights go in by the first floor's mouth; one digs loose rock, blasts
/// hard rock, slides on the frost's ice and leaves by the lamp. With `out`,
/// every tick's screen and the measured costs are written there.
fn play(out: Option<&Path>) {
    let reel = Reel(std::cell::RefCell::new(out.map(|dir| Film {
        frames: std::fs::File::create(dir.join("frames.rgb")).expect("frames file"),
        count: 0,
        marks: String::new(),
    })));
    let mut each = |host: &Run| reel.shot(host);
    let mut costs = String::new();
    let mut host = first_floor_at_the_mouth(11);
    let hold = Input { interact: true, ..Input::default() };
    reel.mark("room");
    for _ in 0..60 {
        // The draw thread, still outside: this warms the arrival.
        let outside = frame_for(&host, 384, 224, Some(1));
        assert_eq!((outside.w, outside.h), (384, 224));
        reel.shot(&host);
        host.step(&BTreeMap::from([(1, hold), (2, hold)]));
        if host.caves.inside(1) && host.caves.inside(2) {
            break;
        }
    }
    assert!(host.caves.inside(1) && host.caves.inside(2), "the party never went in");
    let cold = std::time::Instant::now();
    let first = frame_for(&host, 384, 224, Some(1));
    costs.push_str(&format!("cold_first_frame_inside_us {}\n", cold.elapsed().as_micros()));
    assert!(first.get(192, 100).is_some(), "the first frame inside is not drawn");
    if let Some(dir) = out {
        save(dir, "enter.png", &screen(&host, 1));
        save(dir, "enter-guest.png", &screen(&host, 2));
    }
    reel.mark("enter");
    rest(&mut host, 20, &mut each);

    // Loose rock, faced from the south and clear of the lamp: three hits of a held shovel.
    let exit = exit_cell(host.caves.seed);
    let clear = |at: (i32, i32)| (at.0 - exit.0).abs() + (at.1 - exit.1).abs() >= 7;
    let (stand, aim, wall) =
        stand_facing(&host, seat_at(&host, 1), |c| c.wall() && c.loose(), |at, aim| aim == (0, -1) && clear(at))
            .expect("loose rock");
    walk(&mut host, 1, stand, &mut each);
    let dig = Input { fire: true, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() };
    reel.mark("dig");
    let mut stages = vec![host.caves.cell(wall.0, wall.1).unwrap().swings];
    for _ in 0..3 * u32::from(SWING_EVERY) {
        host.step(&BTreeMap::from([(1, dig)]));
        reel.shot(&host);
        let cell = host.caves.cell(wall.0, wall.1).unwrap();
        if cell.wall() && stages.last() != Some(&cell.swings) {
            stages.push(cell.swings);
        }
    }
    assert_eq!(stages, [3, 2, 1], "the wall cracked in two stages before it broke");
    assert!(host.caves.cell(wall.0, wall.1).is_some_and(|c| c.open()), "the loose wall did not open");
    assert_eq!(host.caves.seats[&1].pack().total(), 1);
    rest(&mut host, 50, &mut each);
    walk(&mut host, 1, wall, &mut each);
    assert_eq!(seat_at(&host, 1), wall, "the knight walks through the hole");

    // The same from the side and from above, for the film.
    for (name, way) in [("dig-east", (1, 0)), ("dig-south", (0, 1))] {
        let Some((stand, aim, wall)) =
            stand_facing(&host, seat_at(&host, 1), |c| c.wall() && c.loose(), |at, aim| aim == way && clear(at))
        else {
            continue;
        };
        walk(&mut host, 1, stand, &mut each);
        reel.mark(name);
        let dig = Input { fire: true, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() };
        for _ in 0..3 * u32::from(SWING_EVERY) {
            host.step(&BTreeMap::from([(1, dig)]));
            reel.shot(&host);
        }
        assert!(host.caves.cell(wall.0, wall.1).is_some_and(|c| c.open()), "{name}: the loose wall did not open");
        rest(&mut host, 45, &mut each);
    }

    // Pacing by the arrival with nothing else going on: what a frame costs mid-stride.
    reel.mark("pace");
    let spawn = spawn_cell(host.caves.seed);
    walk(&mut host, 1, spawn, &mut each);
    let east = Input { move_x: 1, ..Input::default() };
    let west = Input { move_x: -1, ..Input::default() };
    let mut walked = Vec::new();
    let mut strides = 0;
    for n in 0..60u32 {
        host.step(&BTreeMap::from([(1, if (n / 24) % 2 == 0 { east } else { west })]));
        strides += u32::from(host.caves.seats[&1].stepping());
        let t0 = std::time::Instant::now();
        let _ = frame_for(&host, 384, 224, Some(1));
        walked.push(t0.elapsed().as_micros());
        reel.shot(&host);
    }
    assert!(strides >= 50, "the knight stood for most of the pacing ({strides} of 60 ticks mid-stride)");
    walked.sort_unstable();
    costs.push_str(&format!(
        "mid_walk_frame_us mean {} median {} worst {}\n",
        walked.iter().sum::<u128>() / 60,
        walked[30],
        walked[59]
    ));

    // What the cave costs the host's tick with two knights in it, one walking.
    let mut probe = host.clone();
    let t0 = std::time::Instant::now();
    for n in 0..60u32 {
        probe.tick_caves(&BTreeMap::from([(1, if (n / 24) % 2 == 0 { east } else { west })]));
        probe.tick += 1;
    }
    costs.push_str(&format!("tick_caves_us mean {}\n", t0.elapsed().as_micros() / 60));

    // Hard rock, faced from the south: one blast.
    let (stand, aim, wall) = stand_facing(
        &host,
        seat_at(&host, 1),
        |c| c.wall() && c.hard() && !c.loose(),
        |at, aim| aim == (0, -1) && clear(at),
    )
    .expect("hard rock");
    walk(&mut host, 1, stand, &mut each);
    rest(&mut host, 10, &mut each);
    let before = host.caves.seats[&1].magic;
    reel.mark("blast");
    host.step(&BTreeMap::from([(1, Input { cast: 1, aim_x: aim.0 as i8, aim_y: aim.1 as i8, ..Input::default() })]));
    assert!(host.caves.cell(wall.0, wall.1).is_some_and(|c| !c.wall()), "the blast left the rock");
    assert_eq!(host.caves.seats[&1].magic, before - world::BLAST_COST, "the blast spent the meter");
    assert!(host.caves.inside(1));
    let mut blasting = Vec::new();
    for _ in 0..70 {
        let t0 = std::time::Instant::now();
        let _ = frame_for(&host, 384, 224, Some(1));
        blasting.push(t0.elapsed().as_micros());
        reel.shot(&host);
        host.step(&BTreeMap::new());
    }
    blasting.sort_unstable();
    costs.push_str(&format!(
        "blast_frame_us mean {} median {} worst {}\n",
        blasting.iter().sum::<u128>() / 70,
        blasting[35],
        blasting[69]
    ));

    // The frost: a slide in plain boots.
    let ice = ice_run(&host).expect("a run of ice");
    place(&mut host, 1, ice);
    reel.mark("ice");
    rest(&mut host, 10, &mut each);
    for _ in 0..2 * u32::from(MOVE_EVERY) {
        host.step(&BTreeMap::from([(1, east)]));
        reel.shot(&host);
    }
    let let_go = seat_at(&host, 1);
    for n in 0..7 * u32::from(MOVE_EVERY) {
        host.step(&BTreeMap::new());
        reel.shot(&host);
        if n == 12 {
            assert!(host.caves.seats[&1].gliding(), "not sliding a moment after letting go");
            if let Some(dir) = out {
                save(dir, "ice.png", &screen(&host, 1));
            }
        }
    }
    assert!(seat_at(&host, 1).0 - let_go.0 >= 3, "plain boots did not slide");

    // Back at the arrival, and out by the lamp.
    let spawn = spawn_cell(host.caves.seed);
    place(&mut host, 1, spawn);
    reel.mark("leave");
    rest(&mut host, 12, &mut each);
    let before = host.home.stockyard.hauled;
    let carried = u32::from(host.caves.seats[&1].pack().total());
    assert!(carried > 0);
    if let Some(dir) = out {
        save(dir, "leave-lamp.png", &screen(&host, 1));
    }
    walk(&mut host, 1, exit, &mut each);
    assert!(!host.caves.inside(1), "the lamp did not lead out");
    assert_eq!(host.home.stockyard.hauled, before + carried, "every stone carried went to the stockyard");
    assert!(host.caves.inside(2), "the knight who stayed was taken out too");
    rest(&mut host, 10, &mut each);
    if let Some(dir) = out {
        save(dir, "leave-room.png", &screen(&host, 1));
        let film = reel.0.borrow_mut().take().expect("film");
        std::fs::write(dir.join("marks.txt"), format!("{}end {}\n", film.marks, film.count)).unwrap();
        std::fs::write(dir.join("costs.txt"), &costs).unwrap();
    }
    eprint!("{costs}");
}

fn reaches(host: &Run, from: (i32, i32), to: (i32, i32)) -> bool {
    let cave = host.caves.view();
    let mut q = std::collections::VecDeque::from([from]);
    let mut seen = std::collections::HashSet::from([from]);
    while let Some(at) = q.pop_front() {
        if at == to {
            return true;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (at.0 + dx, at.1 + dy);
            if seen.contains(&n) || !cave.at(n.0, n.1).is_some_and(|c| c.open()) {
                continue;
            }
            seen.insert(n);
            q.push_back(n);
        }
    }
    false
}

fn nearest_ice(cave: &world::Cave, at: (i32, i32)) -> i32 {
    let mut best = 999;
    for y in 0..world::MAP_H {
        for x in 0..world::MAP_W {
            if cave.at(x, y).is_some_and(|c| c.open() && c.ice()) {
                best = best.min((x - at.0).abs() + (y - at.1).abs());
            }
        }
    }
    best
}

#[test]
fn grip_and_blades_lie_on_reachable_floor_for_five_seeds() {
    for seed in [1u32, 2, 7, 11, 99] {
        let cave = world::Cave::generate(seed);
        let grip = grip_cell(seed);
        let blades = blades_cell(seed);
        assert_ne!(grip, blades, "seed {seed} put both pairs on one cell");
        let g = cave.at(grip.0, grip.1).expect("grip cell");
        let b = cave.at(blades.0, blades.1).expect("blades cell");
        assert!(g.open() && !g.ice(), "seed {seed} grip {grip:?} is not open floor before the ice");
        assert!(b.open() && world::frost_cell(blades.0, blades.1), "seed {seed} blades {blades:?} are not in the frost");
        let mut host = Run::new(u64::from(seed), 1, Some("Ash"));
        host.calm_for_test();
        host.enter_one(1, false);
        assert!(reaches(&host, seat_at(&host, 1), grip), "seed {seed} grip {grip:?} needs a dig");
        assert!(reaches(&host, seat_at(&host, 1), blades), "seed {seed} blades {blades:?} needs a dig");
        assert!(nearest_ice(&cave, grip) <= 8, "seed {seed} grip is far from the first ice");
        assert!(world::region_of(blades.0, blades.1).0 == 3);
    }
}

#[test]
fn walking_onto_boots_names_them_and_a_guest_sees_who() {
    let mut host = Run::new(1, 1, Some("Ash"));
    host.calm_for_test();
    let mut guest = host.players[&1].clone();
    guest.name = "Bram".into();
    host.players.insert(2, guest);
    host.enter_one(1, false);
    host.enter_one(2, false);
    let grip = grip_cell(host.caves.seed);
    let blades = blades_cell(host.caves.seed);
    walk(&mut host, 1, grip, &mut |_| {});
    assert_eq!(host.caves.grip_by, 1);
    assert_eq!(boots_of(host.caves.seats[&1].footwear), world::Footwear::Grip);
    walk(&mut host, 1, blades, &mut |_| {});
    assert_eq!(host.caves.blades_by, 1);
    assert_eq!(boots_of(host.caves.seats[&1].footwear), world::Footwear::Blades);
    let wire = serde_json::to_string(&host.caves).unwrap();
    let seen: Field = serde_json::from_str(&wire).unwrap();
    assert_eq!(seen.grip_by, 1, "a guest does not see who took the grip boots");
    assert_eq!(seen.blades_by, 1, "a guest does not see who took the blades");
}

#[test]
fn a_shovel_on_open_ice_chips_and_does_not_melt() {
    let mut host = Run::new(1, 1, Some("Ash"));
    host.calm_for_test();
    host.enter_one(1, false);
    let cave = world::Cave::generate(host.caves.seed);
    let mut ice = None;
    let mut stand = None;
    for y in 0..world::MAP_H {
        for x in 0..world::MAP_W {
            if !cave.at(x, y).is_some_and(|c| c.ice() && c.open() && !c.wall()) {
                continue;
            }
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let n = (x + dx, y + dy);
                if cave.at(n.0, n.1).is_some_and(|c| c.open() && !c.wall()) {
                    ice = Some((x, y));
                    stand = Some(n);
                    break;
                }
            }
            if ice.is_some() {
                break;
            }
        }
        if ice.is_some() {
            break;
        }
    }
    let ice = ice.expect("an ice floor");
    let stand = stand.expect("a place to stand beside the ice");
    place(&mut host, 1, stand);
    let (dx, dy) = (ice.0 - stand.0, ice.1 - stand.1);
    host.sounds.clear();
    host.step(&BTreeMap::from([(1, Input { fire: true, aim_x: dx as i8, aim_y: dy as i8, ..Input::default() })]));
    let after = host.caves.cell(ice.0, ice.1).expect("ice cell");
    assert!(after.ice(), "a shovel melted the ice floor");
    assert!(host.caves.bursts.iter().any(|b| b.kind == BURST_CHIP), "no ice chips");
    assert!(host.sounds.iter().any(|s| *s == "rock_land"), "no dull chip sound");
    host.players.get_mut(&1).unwrap().spells[0] = Some("fire".into());
    for _ in 0..12 {
        host.step(&BTreeMap::new());
    }
    host.step(&BTreeMap::from([(1, Input { cast: 1, aim_x: dx as i8, aim_y: dy as i8, ..Input::default() })]));
    let melted = host.caves.cell(ice.0, ice.1).expect("ice cell");
    assert!(!melted.ice() || melted.thin(), "fire on ice no longer melts it");
}

fn stand_on(host: &mut Run, id: u32, col: i32, row: i32) {
    let hero = host.players.get_mut(&id).unwrap();
    hero.x = col as f32 * TILE_UNITS + TILE_UNITS * 0.5;
    hero.y = row as f32 * TILE_UNITS + TILE_UNITS * 0.5;
    hero.away = false;
    hero.absent = false;
    hero.stone = false;
}

fn add_guest(host: &mut Run, id: u32, name: &str) {
    let mut guest = host.players[&1].clone();
    guest.name = name.into();
    host.players.insert(id, guest);
}

/// Pictures and costs for the cave fix. Not part of the ordinary suite.
#[test]
#[ignore = "writes the cf2 proof stills; run with --ignored"]
fn write_cf2_proof() {
    let dir_buf = std::env::temp_dir().join("delve-proof/cf2b");
    let dir = dir_buf.as_path();
    std::fs::create_dir_all(dir).unwrap();
    let mut delve = Run::new(1, 1, Some("Ash"));
    delve.calm_for_test();
    add_guest(&mut delve, 2, "Bram");
    stand_on(&mut delve, 1, 8, 8);
    let (mx, my, _, _) = FLOOR_MOUTH.at;
    // Beside the arch, near enough for the label, not standing in the opening.
    stand_on(&mut delve, 2, mx + 1, my + 1);
    save(dir, "mouth-delve-full.png", &screen(&delve, 2));
    crop_still(dir, "mouth-delve-full.png", "mouth-delve.png", 0, 150, 180, 130);

    let mut yard = Run::at_home(
        1,
        1,
        Some("Ash"),
        crate::rules::home::Home::default(),
        Default::default(),
    );
    yard.calm_for_test();
    yard.come_to_the_gate();
    assert_eq!(yard.room().kind, crate::rules::RoomKind::Gate, "yard shot left the kingdom yard");
    add_guest(&mut yard, 2, "Bram");
    let (yx, yy, _, _) = YARD_MOUTH.at;
    stand_on(&mut yard, 1, 10, 10);
    stand_on(&mut yard, 2, yx + 2, yy + 1);
    save(dir, "mouth-yard-full.png", &screen(&yard, 2));
    crop_still(dir, "mouth-yard-full.png", "mouth-yard.png", 0, 40, 240, 100);

    let mut boots = Run::new(1, 1, Some("Ash"));
    boots.calm_for_test();
    boots.enter_one(1, false);
    let grip = grip_cell(boots.caves.seed);
    let blades = blades_cell(boots.caves.seed);
    // Untaken, in the lamp, the knight three cells off so the pair is in plain
    // view. Tick 4 keeps the glint on.
    let grip_stand = stand_off(&boots, grip);
    place(&mut boots, 1, grip_stand);
    boots.tick = 4;
    let grip_screen = screen(&boots, 1);
    save(dir, "boots-grip-1x.png", &grip_screen);
    let blades_stand = stand_off(&boots, blades);
    place(&mut boots, 1, blades_stand);
    boots.tick = 4;
    let blades_screen = screen(&boots, 1);
    save(dir, "boots-blades-1x.png", &blades_screen);
    place(&mut boots, 1, (grip.0 + 1, grip.1));
    walk_onto(&mut boots, 1, grip);
    assert_eq!(boots.caves.grip_by, 1, "grip boots were not taken");
    assert_eq!(boots_of(boots.caves.seats[&1].footwear), world::Footwear::Grip);
    let hud_grip = screen(&boots, 1);
    save(dir, "boots-hud-grip.png", &hud_grip);
    place(&mut boots, 1, (blades.0 + 1, blades.1));
    walk_onto(&mut boots, 1, blades);
    assert_eq!(boots.caves.blades_by, 1, "blade boots were not taken");
    assert_eq!(boots_of(boots.caves.seats[&1].footwear), world::Footwear::Blades);
    let hud_blades = screen(&boots, 1);
    save(dir, "boots-hud-blades.png", &hud_blades);
    write_boots_sheet(dir, &grip_screen, (grip, grip_stand), &blades_screen, (blades, blades_stand), &hud_grip, &hud_blades);

    let mut ice = Run::new(1, 1, Some("Ash"));
    ice.calm_for_test();
    ice.enter_one(1, false);
    let cave = world::Cave::generate(ice.caves.seed);
    let mut hole = None;
    let mut stand = None;
    for y in 1..world::MAP_H - 1 {
        for x in 1..world::MAP_W - 1 {
            if !cave.at(x, y).is_some_and(|c| c.open() && c.ice() && !c.wall()) {
                continue;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (x + dx, y + dy);
                if cave.at(n.0, n.1).is_some_and(|c| c.open() && !c.ice()) {
                    hole = Some((x, y));
                    stand = Some(n);
                    break;
                }
            }
            if hole.is_some() {
                break;
            }
        }
        if hole.is_some() {
            break;
        }
    }
    let hole = hole.expect("ice");
    place(&mut ice, 1, stand.expect("stand"));
    let (dx, dy) = (hole.0 - stand.unwrap().0, hole.1 - stand.unwrap().1);
    ice.step(&BTreeMap::from([(1, Input { fire: true, aim_x: dx as i8, aim_y: dy as i8, ..Input::default() })]));
    save(dir, "ice-shovel.png", &screen(&ice, 1));

    let mut costs = String::new();
    let names = ["mine", "hollow", "foundry", "frost"];
    let kinds = ["rock-chip", "rock-crumble", "rock-blast"];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/cave_fx");
    for (region, name) in names.iter().enumerate() {
        let t = std::time::Instant::now();
        for kind in kinds {
            let bytes = std::fs::read(root.join(format!("{kind}-{name}.png"))).expect("sheet");
            let _ = image::load_from_memory(&bytes).expect("decode");
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        costs.push_str(&format!("region {region} sheet decode {ms:.3} ms\n"));
    }
    let mut inside = Run::new(1, 1, Some("Ash"));
    inside.calm_for_test();
    inside.enter_one(1, false);
    let t = std::time::Instant::now();
    let _ = screen(&inside, 1);
    costs.push_str(&format!("cold first frame inside {:.3} ms\n", t.elapsed().as_secs_f64() * 1000.0));
    for (region, at) in [(0, (20, 40)), (1, (20, 70)), (2, (90, 40)), (3, (90, 8))] {
        place(&mut inside, 1, at);
        let t = std::time::Instant::now();
        let _ = screen(&inside, 1);
        costs.push_str(&format!("after region {region} first frame {:.3} ms\n", t.elapsed().as_secs_f64() * 1000.0));
    }
    let mut sum = 0.0;
    for step in 0..60 {
        inside.step(&BTreeMap::from([(1, Input { move_x: 1, ..Input::default() })]));
        let t = std::time::Instant::now();
        let _ = screen(&inside, 1);
        sum += t.elapsed().as_secs_f64() * 1000.0;
        if step == 0 {
            costs.push_str(&format!("first mid-walk frame {:.3} ms\n", sum));
        }
    }
    costs.push_str(&format!("mean of 60 mid-walk frames {:.3} ms\n", sum / 60.0));
    let mut quiet = Run::new(1, 1, Some("Ash"));
    quiet.calm_for_test();
    quiet.enter_one(1, false);
    quiet.enter_one(2, false);
    let mut pub_ = crate::wire::Publisher::default();
    let mut last = pub_.publish(&quiet).expect("wire");
    let mut quiet_bytes = 0usize;
    let mut quiet_n = 0usize;
    for _ in 0..8 {
        quiet.step(&BTreeMap::new());
        let tree = pub_.publish(&quiet).expect("wire");
        let prev = crate::wire::seat_tree(&last, crate::wire::seat_of(2));
        let now = crate::wire::seat_tree(&tree, crate::wire::seat_of(2));
        if let Some(patch) = angel_net::doc::delta::diff(&prev, &now) {
            let mut out = Vec::new();
            angel_net::doc::wire::Writer::default().patch(&mut out, &patch);
            quiet_bytes += out.len();
            quiet_n += 1;
        }
        last = tree;
    }
    let per = if quiet_n == 0 { 0 } else { quiet_bytes / quiet_n };
    costs.push_str(&format!("quiet guest bytes {per}\n"));
    std::fs::write(dir.join("costs.txt"), &costs).unwrap();
    eprint!("{costs}");
}

fn crop_still(dir: &Path, src: &str, dst: &str, x: u32, y: u32, w: u32, h: u32) {
    let im = image::open(dir.join(src)).expect("still").to_rgb8();
    image::imageops::crop_imm(&im, x, y, w, h)
        .to_image()
        .save(dir.join(dst))
        .expect("cropped still");
}

/// Each untaken pair at 1x and 3x, then the HUD after each pair is taken.
fn write_boots_sheet(
    dir: &Path,
    grip: &crate::ink::Img,
    grip_cell: ((i32, i32), (i32, i32)),
    blades: &crate::ink::Img,
    blades_cell: ((i32, i32), (i32, i32)),
    hud_grip: &crate::ink::Img,
    hud_blades: &crate::ink::Img,
) {
    let grip_1 = crop_img(grip, boot_crop(grip_cell));
    let blades_1 = crop_img(blades, boot_crop(blades_cell));
    let grip_3 = scale3(&grip_1);
    let blades_3 = scale3(&blades_1);
    let hud_g = scale2(&crop_img(hud_grip, pack_crop()));
    let hud_b = scale2(&crop_img(hud_blades, pack_crop()));
    sheet(dir, "boots.png", &[&grip_1, &grip_3, &blades_1, &blades_3, &hud_g, &hud_b]);
}

/// A window on the floor around a pair. The proof stands one cell east of it.
/// An open cell three away from `cell`, for a still where the knight does not
/// stand over the pair.
fn stand_off(host: &Run, cell: (i32, i32)) -> (i32, i32) {
    let cave = host.caves.view();
    [(3, 0), (-3, 0), (0, 3), (0, -3), (2, 2), (-2, 2), (2, -2), (-2, -2), (2, 0), (-2, 0)]
        .into_iter()
        .map(|(dx, dy)| (cell.0 + dx, cell.1 + dy))
        .find(|&(x, y)| cave.at(x, y).is_some_and(|c| c.open()))
        .unwrap_or((cell.0 + 1, cell.1))
}

fn boot_crop((cell, stand): ((i32, i32), (i32, i32))) -> (i32, i32, i32, i32) {
    let feet = (stand.0 * world::CELL + world::CELL / 2, stand.1 * world::CELL + world::CELL / 2);
    let plate_w = world::MAP_W * world::CELL;
    let plate_h = world::MAP_H * world::CELL;
    let cam_x = (feet.0 - world::NATIVE_W / 2).clamp(0, plate_w - world::NATIVE_W);
    let cam_y = (feet.1 - world::NATIVE_H / 2).clamp(0, plate_h - world::NATIVE_H);
    let sx = cell.0 * world::CELL + world::CELL / 2 - cam_x - 3;
    let sy = cell.1 * world::CELL + world::CELL / 2 - cam_y - 5;
    (sx - 20, party::BAND_H + sy - 16, 56, 48)
}

fn crop_img(im: &crate::ink::Img, at: (i32, i32, i32, i32)) -> image::RgbImage {
    let (mut x0, mut y0, w, h) = at;
    x0 = x0.clamp(0, im.w - w);
    y0 = y0.clamp(0, im.h - h);
    let mut out = image::RgbImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let c = im.get(x0 + x, y0 + y).unwrap_or([8, 7, 6]);
            out.put_pixel(x as u32, y as u32, image::Rgb(c));
        }
    }
    out
}

fn walk_onto(host: &mut Run, who: u32, at: (i32, i32)) {
    let from = seat_at(host, who);
    let step = Input {
        move_x: (at.0 - from.0) as i8,
        move_y: (at.1 - from.1) as i8,
        ..Input::default()
    };
    for _ in 0..=MOVE_EVERY {
        host.step(&BTreeMap::from([(who, step)]));
        if seat_at(host, who) == at {
            break;
        }
    }
    assert_eq!(seat_at(host, who), at, "did not step onto the pair");
}

/// The cave's own corner, where a taken pair is named after it is gone from the floor.
fn pack_crop() -> (i32, i32, i32, i32) {
    (0, party::BAND_H + world::NATIVE_H - 16, 240, 16)
}

fn scale3(im: &image::RgbImage) -> image::RgbImage {
    image::imageops::resize(im, im.width() * 3, im.height() * 3, image::imageops::FilterType::Nearest)
}

fn scale2(im: &image::RgbImage) -> image::RgbImage {
    image::imageops::resize(im, im.width() * 2, im.height() * 2, image::imageops::FilterType::Nearest)
}

fn sheet(dir: &Path, name: &str, imgs: &[&image::RgbImage]) {
    let gap = 4u32;
    let w = imgs.iter().map(|im| im.width()).sum::<u32>() + gap * (imgs.len().saturating_sub(1) as u32);
    let h = imgs.iter().map(|im| im.height()).max().unwrap_or(0);
    let mut out = image::RgbImage::from_pixel(w, h, image::Rgb([12, 10, 8]));
    let mut x0 = 0u32;
    for im in imgs {
        let y0 = (h - im.height()) / 2;
        for (x, y, pix) in im.enumerate_pixels() {
            out.put_pixel(x0 + x, y0 + y, *pix);
        }
        x0 += im.width() + gap;
    }
    out.save(dir.join(name)).expect("boot sheet");
}

fn stitch(dir: &Path, name: &str, parts: &[&str]) {
    let imgs: Vec<image::RgbImage> = parts
        .iter()
        .map(|part| image::open(dir.join(part)).expect("panel").to_rgb8())
        .collect();
    let w: u32 = imgs.iter().map(|im| im.width()).sum();
    let h = imgs[0].height();
    let mut out = image::RgbImage::new(w, h);
    let mut x0 = 0u32;
    for im in &imgs {
        for (x, y, pix) in im.enumerate_pixels() {
            out.put_pixel(x0 + x, y, *pix);
        }
        x0 += im.width();
    }
    out.save(dir.join(name)).expect("stitched still");
}
