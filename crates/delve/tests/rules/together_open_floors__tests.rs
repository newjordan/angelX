//! Open floors (`docs/OPEN-FLOORS.md`): one map a floor, its chambers
//! joined by passages, its companies waiting where they live.

use super::*;
use crate::rules::layout::{OPEN_MAX_COLS, OPEN_MAX_ROWS, open_floor};
use std::collections::VecDeque;

/// A party of `knights` playing (the host's own seat absent when
/// `absent_host`), on open floor `depth`.
fn open_run(seed: u64, knights: u32, absent_host: bool, depth: u32) -> Run {
    let mut run = Run::new(seed, 7, None);
    let names = ["Wren", "Bors", "Ysolde"];
    let friends = if absent_host { knights } else { knights.saturating_sub(1) };
    for (k, id) in (2..2 + friends).enumerate() {
        run.join_seat(id, names[k % 3]);
    }
    if absent_host {
        run.set_absent(1, true);
    }
    run.open_floor_for_test(depth);
    run
}

fn open(run: &Run) -> &layout::Open {
    run.dungeon.open.as_ref().expect("an open floor")
}

fn map(run: &Run) -> &Room {
    &run.dungeon.rooms[open(run).map]
}

/// The tiles a knight can stand on, reached from `from` on foot.
fn reach(room: &Room, from: (f32, f32)) -> Vec<bool> {
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Door | Tile::Stairs);
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let start = ((from.0 / TILE_UNITS) as i32, (from.1 / TILE_UNITS) as i32);
    let mut seen = vec![false; (cols * rows) as usize];
    seen[(start.1 * cols + start.0) as usize] = true;
    let mut queue = VecDeque::from([start]);
    while let Some((c, r)) = queue.pop_front() {
        for (dc, dr) in DIRS {
            let (nc, nr) = (c + dc, r + dr);
            if walk(room.tile(nc, nr)) && !seen[(nr * cols + nc) as usize] {
                seen[(nr * cols + nc) as usize] = true;
                queue.push_back((nc, nr));
            }
        }
    }
    seen
}

#[test]
fn every_chamber_and_floor_tile_is_reachable_from_the_arrival() {
    for seed in 0..12u64 {
        for depth in 1..=DEEPEST {
            for party in 1..=4usize {
                let mut rng = Rng::new(seed * 977 + u64::from(depth) * 31 + party as u64);
                let floor = open_floor(depth, Pack::at(depth, Pack::Crypt), party, &mut rng);
                let open = floor.open.as_ref().unwrap();
                assert!(open.valid(&floor.rooms), "seed {seed} depth {depth} party {party}");
                let map = &floor.rooms[open.map];
                assert!(map.cols <= OPEN_MAX_COLS && map.rows <= OPEN_MAX_ROWS);
                assert!(map.great(), "an open map is more than a screen");
                let seen = reach(map, open.arrival);
                for r in 0..map.rows {
                    for c in 0..map.cols {
                        let t = map.tile(c as i32, r as i32);
                        if matches!(t, Tile::Floor | Tile::Stairs | Tile::Door) {
                            assert!(seen[r * map.cols + c], "seed {seed} depth {depth} party {party}: tile {c},{r} ({t:?}) cut off");
                        }
                    }
                }
                for (i, ch) in open.chambers.iter().enumerate() {
                    let (x, y, w, h) = ch.rect;
                    let reached = (y + 1..y + h - 1).any(|r| (x + 1..x + w - 1).any(|c| seen[usize::from(r) * map.cols + usize::from(c)]));
                    assert!(reached, "chamber {i} cannot be walked to");
                }
                // Every door off the map is walked to, and leads back.
                for door in &open.doors {
                    if floor.secret.is_some_and(|s| s.vault == door.room && !s.found) {
                        continue;
                    }
                    for (c, r) in door.tiles() {
                        assert_eq!(map.tile(c, r), Tile::Door);
                    }
                    let back = (usize::from(door.dir) + 2) % 4;
                    assert_eq!(floor.neighbour(door.room, back), Some(open.map), "{:?}", floor.rooms[door.room].kind);
                }
                // The guardian's chamber keeps its stairs (but the dragon's
                // and the last floor's).
                let g = &open.chambers[usize::from(open.guardian)];
                if depth < FLOORS || (FLOORS < depth && depth < DEEPEST) {
                    assert_eq!(g.kind, RoomKind::Stairs);
                    let (x, y, w, h) = g.rect;
                    assert!((y..y + h).any(|r| (x..x + w).any(|c| map.tile(i32::from(c), i32::from(r)) == Tile::Stairs)));
                    assert!(!g.gates.is_empty(), "the guardian's mouths can shut");
                }
                if depth == FLOORS {
                    assert_eq!(g.kind, RoomKind::Lair);
                }
                if depth >= DEEPEST {
                    assert!(floor.rooms.iter().any(|r| r.kind == RoomKind::Threshold));
                }
                if depth >= 2 {
                    assert_eq!(floor.rooms[0].kind, RoomKind::Sanctuary, "the stairs come down into a Sanctuary");
                }
            }
        }
    }
}

#[test]
fn chambers_never_overlap_and_passages_leave_their_walls_standing() {
    for seed in 0..20u64 {
        let mut rng = Rng::new(seed);
        let floor = open_floor(2 + (seed % 4) as u32, Pack::Cavern, 1 + (seed % 4) as usize, &mut rng);
        let open = floor.open.as_ref().unwrap();
        let map = &floor.rooms[open.map];
        for (i, a) in open.chambers.iter().enumerate() {
            for b in &open.chambers[i + 1..] {
                let apart = a.rect.0 + a.rect.2 <= b.rect.0
                    || b.rect.0 + b.rect.2 <= a.rect.0
                    || a.rect.1 + a.rect.3 <= b.rect.1
                    || b.rect.1 + b.rect.3 <= a.rect.1;
                assert!(apart, "seed {seed}: {:?} and {:?}", a.rect, b.rect);
            }
            // A chamber's wall is stone but for its mouths and doors.
            let (x, y, w, h) = (i32::from(a.rect.0), i32::from(a.rect.1), i32::from(a.rect.2), i32::from(a.rect.3));
            let ring = (x..x + w).flat_map(|c| [(c, y), (c, y + h - 1)]).chain((y..y + h).flat_map(|r| [(x, r), (x + w - 1, r)]));
            let gaps = ring.filter(|&(c, r)| map.tile(c, r) != Tile::Wall).count();
            let links = open.links.iter().filter(|&&(p, q)| usize::from(p) == i || usize::from(q) == i).count();
            let doors = open.doors.iter().filter(|d| usize::from(d.chamber) == i).count();
            assert!(gaps <= links * 4 + doors * 2, "seed {seed} chamber {i}: {gaps} gaps for {links} passages, {doors} doors");
        }
        // Rock between chambers reads as wall, never as a pillar.
        assert!(map.tiles_for_test().iter().filter(|&&t| t == Tile::Wall).count() > map.cols * map.rows / 3);
    }
}

#[test]
fn the_companies_wait_dormant_until_a_knight_comes_and_stay_leashed() {
    let mut run = open_run(3, 2, false, 2);
    let o = open(&run).clone();
    assert!(o.populated);
    assert!(!run.enemies.is_empty() && run.enemies.len() <= depths::OPEN_FOES);
    assert!(run.enemies.iter().all(|e| e.dormant && e.home.is_some()));
    assert_eq!(run.phase, Phase::Exploring);
    assert!(!run.barred(), "nothing bars a door on an open map");
    // Standing at the arrival, nothing moves and nothing shoots.
    let before: Vec<(f32, f32)> = run.enemies.iter().map(|e| (e.x, e.y)).collect();
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    let after: Vec<(f32, f32)> = run.enemies.iter().map(|e| (e.x, e.y)).collect();
    assert_eq!(before, after, "a dormant company never moves");
    assert!(run.projectiles.iter().all(|p| !p.hostile), "nor shoots");
    assert!(run.enemies.iter().all(|e| e.dormant), "nor wakes for nobody");
    // A knight walks into a fight chamber: its company wakes; the others sleep on.
    let fight = (0..o.chambers.len()).find(|&i| o.chambers[i].kind == RoomKind::Fight && !o.chambers[i].cleared).unwrap();
    let (cx, cy) = o.chambers[fight].centre();
    let spot = run.open_spots(o.map, (cx, cy + 3.0), 1)[0];
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = spot;
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    run.step(&BTreeMap::new());
    assert!(run.enemies.iter().filter(|e| e.home == Some(fight as u8)).all(|e| !e.dormant), "the chamber's company wakes");
    assert!(run.enemies.iter().filter(|e| e.home != Some(fight as u8) && !o.chambers[usize::from(e.home.unwrap())].holds(cx, cy, -40.0)).all(|e| e.dormant));
    assert!(run.fighting(), "a fight is on");
    // The knight runs back to the arrival: the company never follows past its leash.
    let (x0, y0, x1, y1) = o.chambers[fight].bounds();
    for t in 0..300 {
        (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = o.arrival;
        run.step(&BTreeMap::new());
        for e in run.enemies.iter().filter(|e| e.home == Some(fight as u8)) {
            let l = depths::LEASH + 1.0;
            assert!(e.x >= x0 - l && e.x <= x1 + l && e.y >= y0 - l && e.y <= y1 + l, "tick {t}: {:?} at {},{} off its leash", e.kind, e.x, e.y);
        }
    }
}

#[test]
fn a_woken_company_wakes_no_more_than_the_cap() {
    let mut run = open_run(5, 4, true, 4);
    assert!(run.enemies.len() <= depths::OPEN_FOES);
    let homes: Vec<u8> = (0..open(&run).chambers.len() as u8).collect();
    for e in &mut run.enemies {
        e.wits.foe = 2;
    }
    let _ = homes;
    run.step(&BTreeMap::new());
    let awake = run.enemies.iter().filter(|e| !e.dormant).count();
    assert!(awake <= depths::AWAKE_CAP, "{awake} awake");
}

#[test]
fn the_guardians_chamber_seals_behind_the_party_and_opens_when_it_falls() {
    let mut run = open_run(9, 3, false, 2);
    let o = open(&run).clone();
    let g = usize::from(o.guardian);
    let ch = &o.chambers[g];
    assert!(ch.guarded() && !ch.sealed);
    assert!(run.enemies.iter().any(|e| e.home == Some(g as u8) && e.is_boss()), "the guardian waits in its chamber");
    let (cx, cy) = ch.centre();
    let inside = run.open_spots(o.map, (cx, cy + 4.0), 1)[0];
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = inside;
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run.step(&BTreeMap::new());
    let sealed = &open(&run).chambers[g];
    assert!(sealed.sealed, "the chamber shuts with its guardian alive");
    assert!(run.barred(), "its gates are barred");
    for (c, r) in &sealed.gates {
        assert_eq!(map(&run).tile(i32::from(*c), i32::from(*r)), Tile::Door);
    }
    assert!(run.players.values().all(|h| sealed.holds(h.x, h.y, 0.0)), "the party is drawn in together");
    assert!(run.open_guarded(), "the stairs wait");
    // Its company falls: the gates open, the chamber pays.
    for e in run.enemies.iter_mut().filter(|e| e.home == Some(g as u8)) {
        e.hp = 0;
    }
    run.step(&BTreeMap::new());
    let done = &open(&run).chambers[g];
    assert!(done.cleared && !done.sealed);
    assert!(!run.barred() && !run.open_guarded());
}

/// Each of the guardian chamber's gates with the way in from it: the tile,
/// and the unit step from its wall toward the chamber's middle.
fn gates_inward(ch: &layout::Chamber) -> Vec<((u16, u16), (f32, f32))> {
    let (x, y, w, h) = ch.rect;
    ch.gates
        .iter()
        .map(|&(c, r)| {
            let inward = if r == y {
                (0.0, 1.0)
            } else if r == y + h - 1 {
                (0.0, -1.0)
            } else if c == x {
                (1.0, 0.0)
            } else {
                (-1.0, 0.0)
            };
            ((c, r), inward)
        })
        .collect()
}

#[test]
fn a_knight_stepping_in_at_a_gate_draws_the_party_in_not_out() {
    // The first knight in stands just past the gate; the rest wait in the
    // passage behind it. The seal used to set them down round that knight,
    // the passage too, and shut the gate between the party.
    let mut cases = 0;
    for seed in 0..6u64 {
        for depth in 1..=3 {
            let base = open_run(seed, 3, false, depth);
            let o = open(&base).clone();
            let g = usize::from(o.guardian);
            for ((c, r), (ix, iy)) in gates_inward(&o.chambers[g]) {
                let mut run = base.clone();
                let (gx, gy) = ((f32::from(c) + 0.5) * TILE_UNITS, (f32::from(r) + 0.5) * TILE_UNITS);
                let behind = [(gx - ix * 2.0, gy - iy * 2.0), (gx - ix * 4.0, gy - iy * 4.0)];
                let stands = |(x, y): (f32, f32)| map(&run).tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor;
                if !behind.iter().all(|&at| stands(at)) {
                    continue;
                }
                let spots = [(gx + ix * 2.05, gy + iy * 2.05), behind[0], behind[1]];
                for (hero, at) in run.players.values_mut().zip(spots) {
                    (hero.x, hero.y) = at;
                    hero.invulnerable = 100_000;
                }
                run.step(&BTreeMap::new());
                let ch = &open(&run).chambers[g];
                assert!(ch.sealed, "seed {seed} depth {depth} gate {c},{r}: the first knight in seals it");
                for hero in run.players.values() {
                    assert!(
                        ch.holds(hero.x, hero.y, 0.0),
                        "seed {seed} depth {depth} gate {c},{r}: {} left outside at {:.1},{:.1}",
                        hero.name,
                        hero.x,
                        hero.y
                    );
                }
                cases += 1;
            }
        }
    }
    assert!(cases >= 12, "only {cases} gates tried");
}

#[test]
fn a_shut_chamber_nobody_stands_in_opens_again() {
    let mut run = open_run(9, 2, false, 2);
    let o = open(&run).clone();
    let g = usize::from(o.guardian);
    let (cx, cy) = o.chambers[g].centre();
    let inside = run.open_spots(o.map, (cx, cy + 4.0), 1)[0];
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = inside;
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run.step(&BTreeMap::new());
    assert!(open(&run).chambers[g].sealed);
    // Knocked out through a gate, the whole party: the guardian's door
    // must not shut it out of the floor for good.
    let ((c, r), (ix, iy)) = gates_inward(&o.chambers[g])[0];
    let out = ((f32::from(c) + 0.5 - ix * 2.0) * TILE_UNITS, (f32::from(r) + 0.5 - iy * 2.0) * TILE_UNITS);
    let outside = run.open_spots(o.map, out, 2);
    for (hero, at) in run.players.values_mut().zip(outside) {
        (hero.x, hero.y) = at;
    }
    assert!(run.players.values().all(|h| !o.chambers[g].holds(h.x, h.y, 0.0)));
    run.step(&BTreeMap::new());
    assert!(!open(&run).chambers[g].sealed && !run.barred(), "the gates open for the party outside");
    // And shut again behind the next knight in, the party with them.
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = inside;
    run.step(&BTreeMap::new());
    let ch = &open(&run).chambers[g];
    assert!(ch.sealed && run.barred());
    assert!(run.players.values().all(|h| ch.holds(h.x, h.y, 0.0)), "the party is drawn in together");
}

#[test]
fn a_chamber_clear_pays_once() {
    let mut run = open_run(12, 2, false, 1);
    let o = open(&run).clone();
    let fight = (0..o.chambers.len()).find(|&i| o.chambers[i].kind == RoomKind::Fight && !o.chambers[i].cleared).unwrap();
    let (cx, cy) = o.chambers[fight].centre();
    let spot = run.open_spots(o.map, (cx, cy), 1)[0];
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = spot;
    run.step(&BTreeMap::new());
    run.cues.clear();
    let bonds = |run: &Run| run.players.values().map(|h| h.carried.0.get(&Spoil::Bond).copied().unwrap_or(0)).sum::<u32>();
    let mark = barony::clear_mark(run.dungeon.pack);
    let (marks, bonded) = (run.marks.get(&mark).copied().unwrap_or(0), bonds(&run));
    for e in run.enemies.iter_mut().filter(|e| e.home == Some(fight as u8)) {
        e.hp = 0;
    }
    let mut clears = 0;
    for _ in 0..60 {
        run.step(&BTreeMap::new());
        clears += run.cues.iter().filter(|c| *c == "room_clear" || *c == "flawless").count();
        run.cues.clear();
    }
    assert_eq!(clears, 1, "one clear said");
    assert!(open(&run).chambers[fight].cleared);
    assert_eq!(run.marks.get(&mark).copied().unwrap_or(0), marks + 1, "one hall freed for the King");
    assert_eq!(bonds(&run), bonded + 1, "one bond for two knights standing");
    assert!(run.window.is_none(), "a plain chamber opens no wish window: only the floor's boss and its well do");
}

#[test]
fn leaving_the_map_for_a_side_room_and_back_keeps_its_companies() {
    let mut run = open_run(4, 1, false, 2);
    let o = open(&run).clone();
    let door = *o.doors.iter().find(|d| run.dungeon.rooms[d.room].kind == RoomKind::Treasure).expect("a treasure vault");
    let before: Vec<(u32, f32, f32, u32)> = run.enemies.iter().map(|e| (e.id, e.x, e.y, e.hp)).collect();
    let (c, r) = door.tiles()[0];
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
    run.step(&BTreeMap::new());
    assert_eq!(run.at, door.room, "the door leads into its room");
    assert_eq!(run.room().kind, RoomKind::Treasure);
    assert!(run.enemies.is_empty());
    // Out its one door again: back on the map, by that door.
    let back = (usize::from(door.dir) + 2) % 4;
    assert_eq!(run.dungeon.neighbour(run.at, back), Some(o.map));
    run.enter(o.map, Some(usize::from(door.dir)));
    assert_eq!(run.at, o.map);
    let hero = &run.players[&1];
    let (lx, ly) = door.landing();
    assert!((hero.x - lx).hypot(hero.y - ly) < 8.0, "the knight comes out where they went in");
    let after: Vec<(u32, f32, f32, u32)> = run.enemies.iter().map(|e| (e.id, e.x, e.y, e.hp)).collect();
    assert_eq!(before, after, "the map's companies are as they were left");
}

#[test]
fn the_stairs_lead_down_to_another_open_floor() {
    let mut run = open_run(21, 1, false, 1);
    let o = open(&run).clone();
    let g = usize::from(o.guardian);
    let stairs = {
        let (x, y, w, h) = o.chambers[g].rect;
        let m = map(&run);
        (y..y + h)
            .flat_map(|r| (x..x + w).map(move |c| (c, r)))
            .find(|&(c, r)| m.tile(i32::from(c), i32::from(r)) == Tile::Stairs)
            .unwrap()
    };
    let on = ((f32::from(stairs.0) + 0.5) * TILE_UNITS, (f32::from(stairs.1) + 0.5) * TILE_UNITS);
    // While the guardian lives the stairs go nowhere.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = on;
    hero.invulnerable = 100_000;
    for _ in 0..5 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.floor(), 1);
    for e in run.enemies.iter_mut().filter(|e| e.home == Some(g as u8)) {
        e.hp = 0;
    }
    run.step_sim_for_test(&BTreeMap::new());
    // Off the stairs, then on again.
    let (cx, cy) = o.chambers[g].centre();
    let off = run.open_spots(o.map, (cx, cy - 6.0), 1)[0];
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = off;
    run.step_sim_for_test(&BTreeMap::new());
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = on;
    run.step_sim_for_test(&BTreeMap::new());
    assert_eq!(run.floor(), 2, "down the stairs");
    assert!(run.dungeon.open.is_some(), "into another open floor");
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    assert!(run.valid_snapshot());
}

#[test]
fn a_wheel_spin_or_the_hosts_switch_lays_the_delve_open() {
    let mut run = Run::new(2, 3, None);
    assert!(!run.wants_open());
    run.mode = fortune::Mode::OpenFloors;
    assert!(run.wants_open());
    assert!(fortune::Mode::WHEEL.contains(&fortune::Mode::OpenFloors));
    assert_eq!(fortune::Mode::OpenFloors.rung(), 0);
    run.descend_for_test();
    assert!(run.dungeon.open.is_some(), "the wheel's wedge opens the next floor");
    let mut room_game = Run::new(2, 3, None);
    room_game.descend_for_test();
    assert!(room_game.dungeon.open.is_none(), "without it, the room game");
}

#[test]
fn an_old_save_loads_and_an_open_one_round_trips() {
    // A save from before open floors: none of their fields.
    let run = Run::new(8, 4, Some("Friend"));
    let mut save = serde_json::to_value(&run).unwrap();
    let fields = save.as_object_mut().unwrap();
    fields.remove("open_floors");
    fields.remove("parked");
    assert!(fields["dungeon"].get("open").is_none());
    let back: Run = serde_json::from_value(save).unwrap();
    assert!(back.valid_snapshot());
    assert!(!back.open_floors && back.dungeon.open.is_none());
    // An open floor mid-play saves and loads as it was.
    let mut open_one = open_run(8, 2, false, 3);
    for _ in 0..30 {
        open_one.step(&BTreeMap::new());
    }
    assert!(open_one.valid_snapshot());
    let text = serde_json::to_string(&open_one).unwrap();
    // Within what the cockpit reads back as a checkpoint (2 MiB).
    assert!(text.len() < 1024 * 1024, "an open floor's save is {} bytes", text.len());
    let again: Run = serde_json::from_str(&text).unwrap();
    assert!(again.valid_snapshot());
    assert_eq!(again.dungeon.open, open_one.dungeon.open);
    assert_eq!(again.enemies.len(), open_one.enemies.len());
    assert!(again.enemies.iter().zip(&open_one.enemies).all(|(a, b)| a.dormant == b.dormant && a.home == b.home));
    // And the wire's tree reads back drawable.
    let tree = crate::wire::publish(&open_one).unwrap();
    let client = crate::wire::read(&tree).unwrap();
    assert!(crate::wire::drawable(&client));
    assert_eq!(client.dungeon.open, open_one.dungeon.open);
}

/// What a bot does for knight `id` on an open map: shoot what is awake
/// and near, else walk the way to `goal`.
pub(crate) fn bot(run: &Run, id: u32, goal: (f32, f32), path: &mut Vec<(i32, i32)>) -> Input {
    let hero = &run.players[&id];
    if hero.hp == 0 || hero.stone {
        return Input::default();
    }
    let mut input = Input::default();
    let grid_room = run.room();
    // In sight: nothing but floor between.
    let sight = |x: f32, y: f32| {
        let d = (x - hero.x).hypot(y - hero.y);
        let n = (d / 0.5).ceil().max(1.0) as i32;
        (1..n).all(|k| {
            let t = k as f32 / n as f32;
            let (px, py) = (hero.x + (x - hero.x) * t, hero.y + (y - hero.y) * t);
            !matches!(grid_room.tile((px / TILE_UNITS) as i32, (py / TILE_UNITS) as i32), Tile::Wall | Tile::Block)
        })
    };
    let near = run
        .enemies
        .iter()
        .filter(|e| e.hp > 0 && !e.dormant && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH)
        .map(|e| (e, (e.x - hero.x).hypot(e.y - hero.y)))
        .filter(|&(e, d)| d < 22.0 && sight(e.x, e.y))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if near.is_some() {
        // A fight: as the balance harness's patient player fights it.
        return crate::rules::balance_sim_tests::bot_input(run, id);
    }
    let here = ((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
    if path.first() != Some(&here) || run.tick.is_multiple_of(45) {
        *path = depths::open_path(grid_room, (hero.x, hero.y), goal, run.barred()).unwrap_or_default();
    }
    while path.len() > 1 && path[0] == here {
        path.remove(0);
    }
    if let Some(&(c, r)) = path.first() {
        // Toward the next tile's middle, each way on its own, so a knight
        // off the line is brought back onto it and never hangs on a corner.
        let axis = |d: f32| if d > 0.25 { 1 } else if d < -0.25 { -1 } else { 0 };
        let (dx, dy) = ((c as f32 + 0.5) * TILE_UNITS - hero.x, (r as f32 + 0.5) * TILE_UNITS - hero.y);
        (input.move_x, input.move_y) = (axis(dx), axis(dy));
    }
    input
}

/// What a bot party's walk through an open floor came to.
#[derive(Debug, Default)]
struct Walk {
    ticks: u64,
    /// Chambers whose companies the party put down, of how many.
    cleared: usize,
    chambers: usize,
    /// Health the knights lost, in percent of one knight's.
    taken: u64,
    sealed: bool,
}

/// Bots play a whole open floor: from the arrival through the chambers
/// to the guardian, and down its stairs. The floor is under test, not the
/// bots' play: what they lose is counted and given back, so a walk always
/// reaches its end or shows where the floor stops it.
fn finish_a_floor(seed: u64, knights: u32, absent_host: bool, depth: u32) -> Walk {
    let mut run = open_run(seed, knights, absent_host, depth);
    assert_eq!(run.party_len() as u32, knights);
    let ids: Vec<u32> = run.players.iter().filter(|(_, h)| !h.absent).map(|(&id, _)| id).collect();
    let mut paths: BTreeMap<u32, Vec<(i32, i32)>> = BTreeMap::new();
    let start = run.floor();
    let mut walk = Walk { chambers: open(&run).chambers.len(), ..Walk::default() };
    let mut on_stairs_at: Option<u64> = None;
    while run.floor() == start && walk.ticks < 30 * 60 * u64::from(HZ) {
        assert_ne!(run.phase, Phase::Wiped, "seed {seed} knights {knights}: the party fell at tick {}", walk.ticks);
        let Some(o) = run.dungeon.open.clone() else { break };
        // Back onto the map from any side room.
        if run.at != o.map {
            let door = o.doors.iter().find(|d| d.room == run.at).copied();
            run.enter(o.map, door.map(|d| usize::from(d.dir)));
            continue;
        }
        walk.sealed |= o.chambers[usize::from(o.guardian)].sealed;
        let g = &o.chambers[usize::from(o.guardian)];
        let goal = if g.cleared {
            let (x, y, w, h) = g.rect;
            let m = map(&run);
            let s = (y..y + h)
                .flat_map(|r| (x..x + w).map(move |c| (c, r)))
                .find(|&(c, r)| m.tile(i32::from(c), i32::from(r)) == Tile::Stairs)
                .unwrap();
            // Step off and back on, should a stair be held.
            let t = walk.ticks;
            let wobble = on_stairs_at.is_some_and(|at| t - at > 60 && (t / 30).is_multiple_of(2));
            ((f32::from(s.0) + 0.5) * TILE_UNITS, (f32::from(s.1) + if wobble { -3.5 } else { 0.5 }) * TILE_UNITS)
        } else {
            g.centre()
        };
        if g.cleared && on_stairs_at.is_none() {
            on_stairs_at = Some(walk.ticks);
        }
        let inputs: BTreeMap<u32, Input> = ids
            .iter()
            .map(|&id| (id, bot(&run, id, goal, paths.entry(id).or_default())))
            .collect();
        let before: u32 = ids.iter().map(|id| run.players[id].hp).sum();
        run.step_sim_for_test(&inputs);
        let after: u32 = ids.iter().map(|id| run.players.get(id).map_or(0, |h| h.hp)).sum();
        walk.taken += u64::from(before.saturating_sub(after));
        for id in &ids {
            if let Some(hero) = run.players.get_mut(id) {
                hero.hp = hero.max_hp;
            }
        }
        if std::env::var_os("OPEN_SIM_LOG").is_some() && walk.ticks % 300 == 0 {
            let o = run.dungeon.open.as_ref().unwrap();
            let awake = run.enemies.iter().filter(|e| e.hp > 0 && !e.dormant).count();
            let at: Vec<(i32, i32)> = ids.iter().map(|id| (run.players[id].x as i32, run.players[id].y as i32)).collect();
            let cleared = o.chambers.iter().filter(|c| c.cleared).count();
            eprintln!("t{} awake{awake} left{} cleared{cleared}/{} at{at:?} goal{goal:?}", walk.ticks, run.enemies.iter().filter(|e| e.hp > 0).count(), o.chambers.len());
        }
        walk.ticks += 1;
    }
    let max = run.players.values().map(|h| h.max_hp).max().unwrap_or(100).max(1);
    walk.taken = walk.taken * 100 / u64::from(max);
    walk.cleared = run.dungeon.open.as_ref().map_or(walk.chambers, |o| if run.floor() > start { walk.chambers } else { o.chambers.iter().filter(|c| c.cleared).count() });
    assert!(run.floor() > start, "seed {seed} knights {knights} depth {depth}: not down after {} ticks ({} foes left)", walk.ticks, run.enemies.iter().filter(|e| e.hp > 0).count());
    assert!(walk.sealed, "the guardian's chamber shut on the way");
    walk
}

/// The first floor's breather (`Tune::clear_rest`): a chamber's company put
/// down mends every knight standing a share of their health; deeper, and
/// with the knob at 0, it is the game as it was.
#[test]
fn a_chamber_won_on_the_first_floor_mends_every_knight_standing() {
    let mended = |depth: u32, rest: Option<u32>| {
        let mut run = open_run(41, 2, true, depth);
        if let Some(rest) = rest {
            run.tune.clear_rest = rest;
        }
        let i = open(&run).chambers.iter().position(|c| c.kind == RoomKind::Fight).expect("a fight chamber");
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp / 5;
        }
        let before: Vec<(u32, u32)> = run.players.values().filter(|h| !h.absent).map(|h| (h.hp, h.max_hp)).collect();
        run.chamber_clear(i);
        let after: Vec<u32> = run.players.values().filter(|h| !h.absent).map(|h| h.hp).collect();
        before.iter().zip(after).map(|(&(was, most), now)| (now - was, most)).collect::<Vec<_>>()
    };
    for (gain, most) in mended(1, None) {
        assert_eq!(gain, most * crate::rules::tune::CLEAR_REST / 100, "the first floor's breather");
    }
    assert!(mended(2, None).iter().all(|&(gain, _)| gain == 0), "floor 2 keeps its old stakes");
    assert!(mended(1, Some(0)).iter().all(|&(gain, _)| gain == 0), "the knob at 0 is the game as it was");
}

#[test]
fn a_party_of_two_to_four_with_the_host_away_finishes_an_open_floor() {
    // Four seats: with the host's own absent, three knights at most play.
    for (seed, knights, absent, depth) in [(31u64, 2u32, true, 1u32), (32, 3, true, 1), (33, 4, false, 2)] {
        let walk = finish_a_floor(seed, knights, absent, depth);
        eprintln!("{knights} knights, floor {depth}: down in {:.0} s, {}% of a knight's health taken", walk.ticks as f32 / HZ as f32, walk.taken);
    }
}


/// `cargo test --release open_floor_tick_and_wire_cost -- --ignored --nocapture`:
/// a tick of a full open floor (four knights, the floor's whole company,
/// as many awake as may be, all of them fighting) and what the wire sends
/// a tick.
#[test]
#[ignore]
fn open_floor_tick_and_wire_cost() {
    use angel_net::doc::delta::diff;
    use angel_net::doc::wire::Writer;
    let mut run = open_run(77, 4, false, 5);
    let o = open(&run).clone();
    println!("map {}x{}, {} chambers, {} foes raised", map(&run).cols, map(&run).rows, o.chambers.len(), run.enemies.len());
    // Every knight in a different chamber, unhurt, firing.
    let fights: Vec<usize> = (0..o.chambers.len()).filter(|&i| o.chambers[i].kind == RoomKind::Fight).collect();
    for (k, id) in [1u32, 2, 3, 4].into_iter().enumerate() {
        let (cx, cy) = o.chambers[fights[k % fights.len()]].centre();
        let spot = run.open_spots(o.map, (cx, cy), 1)[0];
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = spot;
    }
    for e in &mut run.enemies {
        e.wits.foe = 1;
    }
    let mut publisher = crate::wire::Publisher::default();
    let mut writer = Writer::default();
    let first = publisher.publish(&run).unwrap();
    let mut whole = Vec::new();
    writer.value(&mut whole, &first);
    let mut before = first;
    let (mut tick_ns, mut publish_ns, mut bytes, mut most) = (0u128, 0u128, 0usize, 0usize);
    let mut awake_most = 0;
    let n = 300;
    for t in 0..n {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
            hero.invulnerable = 5;
        }
        let inputs: BTreeMap<u32, Input> = [1u32, 2, 3, 4]
            .into_iter()
            .map(|id| (id, Input { fire: true, aim_x: [1, 0, -1, 0][(t / 20 + id as usize) % 4], aim_y: [0, 1, 0, -1][(t / 20 + id as usize) % 4], ..Input::default() }))
            .collect();
        let start = std::time::Instant::now();
        run.step(&inputs);
        tick_ns += start.elapsed().as_nanos();
        awake_most = awake_most.max(run.enemies.iter().filter(|e| e.hp > 0 && !e.dormant).count());
        let start = std::time::Instant::now();
        let tree = publisher.publish(&run).unwrap();
        let patch = diff(&before, &tree);
        let mut out = Vec::new();
        if let Some(p) = &patch {
            writer.patch(&mut out, p);
        }
        publish_ns += start.elapsed().as_nanos();
        bytes += out.len();
        most = most.max(out.len());
        before = tree;
    }
    println!(
        "tick {:.3} ms avg; publish+diff {:.3} ms avg; wire {} B/tick avg, {} B most; first snapshot {} B; awake at most {}; shots {}",
        tick_ns as f64 / n as f64 / 1e6,
        publish_ns as f64 / n as f64 / 1e6,
        bytes / n,
        most,
        whole.len(),
        awake_most,
        run.projectiles.len()
    );
}

/// A plain arrow. Hostile ones answer the knights; the rest hunt the company.
fn air_shot(n: usize, x: f32, y: f32) -> Projectile {
    let ang = n as f32 * 0.37;
    let speed = 6.0 + (n % 5) as f32;
    Projectile {
        x,
        y,
        vx: ang.cos() * speed,
        vy: ang.sin() * speed,
        hostile: n % 3 == 0,
        kind: Shot::Arrow,
        look: None,
        damage: 1,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 8 * HZ,
    }
}

/// Top the air back up to `MAX_PROJECTILES` from the floor's open spots.
fn fill_air(run: &mut Run, spots: &[(f32, f32)]) {
    if spots.is_empty() {
        return;
    }
    let (w, h) = (map(run).width(), map(run).height());
    let have = run.projectiles.len();
    for n in have..MAX_PROJECTILES {
        let (sx, sy) = spots[n % spots.len()];
        run.projectiles.push(air_shot(n, sx.clamp(1.0, w - 1.0), sy.clamp(1.0, h - 1.0)));
    }
}

/// Hold the measured load: six living knights, every company awake, 384 shots.
fn hold_host_load(run: &mut Run, hp: &[u32], spots: &[(f32, f32)]) {
    for (e, full) in run.enemies.iter_mut().zip(hp.iter()) {
        e.dormant = false;
        if e.hp < *full {
            e.hp = *full;
        }
        if e.age < TELEGRAPH {
            e.age = TELEGRAPH;
        }
    }
    for hero in run.players.values_mut() {
        hero.hp = hero.max_hp.max(1);
        hero.invulnerable = 1_000_000;
    }
    fill_air(run, spots);
}

/// `cargo test --profile fast --lib open_floor_host_tick_cost -- --ignored --nocapture`:
/// `Run::step` on the largest open floor (depth 6, six knights, every
/// company awake, 384 shots in the air) over a few thousand ticks.
/// Prints mean and p95 per tick.
#[test]
#[ignore]
fn open_floor_host_tick_cost() {
    let ticks: usize = std::env::var("HOST_TICK_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2_000);
    const KNIGHTS: [u32; 6] = [1, 2, 3, 4, 5, 6];
    let mut run = open_run(77, 6, false, 6);
    assert_eq!(run.players.len(), 6, "six knights");
    assert_eq!(run.floor(), 6, "the largest open floor");
    let o = open(&run).clone();
    let fights: Vec<usize> = (0..o.chambers.len()).filter(|&i| o.chambers[i].kind == RoomKind::Fight).collect();
    assert!(!fights.is_empty(), "a fight chamber");
    for (k, id) in KNIGHTS.into_iter().enumerate() {
        let (cx, cy) = o.chambers[fights[k % fights.len()]].centre();
        let spot = run.open_spots(o.map, (cx, cy), 1)[0];
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = spot;
    }
    let mut hp = Vec::with_capacity(run.enemies.len());
    for e in &mut run.enemies {
        e.dormant = false;
        e.wits.foe = 1;
        if e.age < TELEGRAPH {
            e.age = TELEGRAPH;
        }
        // One tick of the full air must not empty the company.
        e.hp = e.hp.max(200_000);
        hp.push(e.hp);
    }
    if let Some(open) = run.dungeon.open.as_mut() {
        for ch in &mut open.chambers {
            if !ch.roster.is_empty() {
                ch.stirred = true;
                ch.cleared = false;
            }
        }
    }
    let awake = run.enemies.iter().filter(|e| e.hp > 0 && !e.dormant).count();
    assert_eq!(awake, run.enemies.len(), "every company awake");
    let spots = run.open_spots(o.map, o.arrival, 64);
    assert!(!spots.is_empty(), "open ground for the shots");
    fill_air(&mut run, &spots);
    assert_eq!(run.projectiles.len(), 384, "384 shots in the air");
    let inputs: BTreeMap<u32, Input> = KNIGHTS
        .into_iter()
        .map(|id| (id, Input { fire: true, aim_x: 1, aim_y: 0, ..Input::default() }))
        .collect();
    for _ in 0..32 {
        hold_host_load(&mut run, &hp, &spots);
        run.step(&inputs);
    }
    let mut samples = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        hold_host_load(&mut run, &hp, &spots);
        let start = std::time::Instant::now();
        run.step(&inputs);
        samples.push(start.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let sum: u128 = samples.iter().copied().sum();
    let mean_us = sum as f64 / ticks as f64 / 1_000.0;
    let p95_us = samples[(ticks * 95 + 99) / 100 - 1] as f64 / 1_000.0;
    println!(
        "host tick mean {mean_us:.3} us  p95 {p95_us:.3} us  over {ticks} ticks; foes {awake} shots 384 depth 6 knights 6"
    );
}

/// `cargo test --profile fast --lib open_floor_step_allocations -- --ignored --nocapture`:
/// allocations and bytes asked per `Run::step` on a depth-6 open floor
/// with six knights, every company awake and a few hundred shots.
#[test]
#[ignore]
fn open_floor_step_allocations() {
    const KNIGHTS: [u32; 6] = [1, 2, 3, 4, 5, 6];
    const TICKS: u32 = 40;
    let mut run = open_run(77, 6, false, 6);
    assert_eq!(run.players.len(), 6, "six knights");
    assert_eq!(run.floor(), 6, "the largest open floor");
    let o = open(&run).clone();
    let fights: Vec<usize> = (0..o.chambers.len()).filter(|&i| o.chambers[i].kind == RoomKind::Fight).collect();
    assert!(!fights.is_empty(), "a fight chamber");
    for (k, id) in KNIGHTS.into_iter().enumerate() {
        let (cx, cy) = o.chambers[fights[k % fights.len()]].centre();
        let spot = run.open_spots(o.map, (cx, cy), 1)[0];
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = spot;
    }
    let mut hp = Vec::with_capacity(run.enemies.len());
    for e in &mut run.enemies {
        e.dormant = false;
        e.wits.foe = 1;
        if e.age < TELEGRAPH {
            e.age = TELEGRAPH;
        }
        e.hp = e.hp.max(200_000);
        hp.push(e.hp);
    }
    if let Some(open) = run.dungeon.open.as_mut() {
        for ch in &mut open.chambers {
            if !ch.roster.is_empty() {
                ch.stirred = true;
                ch.cleared = false;
            }
        }
    }
    let awake = run.enemies.iter().filter(|e| e.hp > 0 && !e.dormant).count();
    assert!(awake > 0, "awake foes");
    let spots = run.open_spots(o.map, o.arrival, 64);
    fill_air(&mut run, &spots);
    assert!(run.projectiles.len() >= 300, "a few hundred shots");
    let inputs: BTreeMap<u32, Input> = KNIGHTS
        .into_iter()
        .map(|id| (id, Input { fire: true, aim_x: 1, aim_y: 0, ..Input::default() }))
        .collect();
    for _ in 0..32 {
        hold_host_load(&mut run, &hp, &spots);
        run.step(&inputs);
    }
    crate::test_alloc::reset_census();
    let mut asked = 0u64;
    let mut count = 0u64;
    for _ in 0..TICKS {
        hold_host_load(&mut run, &hp, &spots);
        crate::test_alloc::census(true);
        let (_, a0, c0) = crate::test_alloc::now();
        run.step(&inputs);
        let (_, a1, c1) = crate::test_alloc::now();
        crate::test_alloc::census(false);
        asked += a1 - a0;
        count += c1 - c0;
    }
    let allocs = count;
    let bytes = asked;
    let ticks = f64::from(TICKS);
    println!(
        "open-floor step allocations: {:.1} allocs/tick, {:.1} bytes/tick over {TICKS} ticks; foes {awake} shots {} depth 6 knights 6",
        allocs as f64 / ticks,
        bytes as f64 / ticks,
        run.projectiles.len()
    );
    let (buckets, large) = crate::test_alloc::census_report();
    for (lo, count, asked) in buckets {
        println!(
            "  bucket >= {lo:>8}  {:>8.1}/tick  {:>10.1} B/tick",
            count as f64 / ticks,
            asked as f64 / ticks
        );
    }
    for (size, count, asked) in large.iter().take(16) {
        println!(
            "  size {size:>8}  {:>8.1}/tick  {:>10.1} B/tick",
            *count as f64 / ticks,
            *asked as f64 / ticks
        );
    }
}

#[test]
fn a_knight_walking_out_of_a_side_room_stays_out_until_walking_back_in() {
    // The way back from a side room kept where the knight stood as they
    // went in: on the door tile itself, so walking out of the Pit or the
    // Wanted hall put them back on the door and it took them straight in
    // again ("fix the room walkouts we bounce back on accident",
    // 2026-10-19).
    let (mut run, door) = (1..32u64)
        .find_map(|seed| {
            let run = open_run(seed, 1, false, 2);
            let door = *open(&run).doors.iter().find(|d| run.dungeon.rooms[d.room].kind == RoomKind::Pit)?;
            Some((run, door))
        })
        .expect("a Pit off some open map");
    let o = open(&run).clone();
    let walk = |dir: usize| {
        let (dx, dy) = DIRS[dir];
        BTreeMap::from([(1, Input { move_x: dx as i8, move_y: dy as i8, ..Input::default() })])
    };
    // Up to the door and through it, as a player walks.
    let (lx, ly) = door.landing();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (lx, ly);
    for _ in 0..HZ * 2 {
        if run.at == door.room {
            break;
        }
        run.step(&walk(usize::from(door.dir)));
    }
    assert_eq!(run.at, door.room, "the door leads into the Pit");
    run.clear_for_test();
    // Out its doorway again, the way back held.
    let back = (usize::from(door.dir) + 2) % 4;
    for _ in 0..HZ * 4 {
        if run.at == o.map {
            break;
        }
        run.step(&walk(back));
    }
    assert_eq!(run.at, o.map, "the knight walks out onto the map");
    let hero = &run.players[&1];
    let tile = map(&run).tile((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
    assert_ne!(tile, Tile::Door, "out by the door, not on it");
    // Left alone, or still walking away, the knight stays out.
    for _ in 0..HZ * 2 {
        run.step(&BTreeMap::new());
    }
    for _ in 0..HZ / 2 {
        run.step(&walk(back));
    }
    assert_eq!(run.at, o.map, "no bounce back into the Pit");
    // Walking back to the door on purpose still goes in.
    for _ in 0..HZ * 4 {
        if run.at == door.room {
            break;
        }
        run.step(&walk(usize::from(door.dir)));
    }
    assert_eq!(run.at, door.room, "the door still takes a knight who walks into it");
}

#[test]
fn holding_west_out_of_a_side_on_hall_by_a_west_door_does_not_walk_back_in() {
    // A side-on hall is left west, by its one low doorway, whichever wall
    // of its chamber its door is in. Out by a west door the knight still
    // held west, and walked onto the door and back in.
    let (mut run, door) = (1..48u64)
        .find_map(|seed| {
            let run = open_run(seed, 1, false, 2);
            let door = *open(&run).doors.iter().find(|d| {
                d.dir == 3
                    && matches!(run.dungeon.rooms[d.room].kind, RoomKind::Ledge | RoomKind::Wanted)
                    && !run.dungeon.secret.is_some_and(|s| s.vault == d.room && !s.found)
            })?;
            Some((run, door))
        })
        .expect("a side-on hall behind a west door");
    let o = open(&run).clone();
    let west = BTreeMap::from([(1, Input { move_x: -1, ..Input::default() })]);
    let (lx, ly) = door.landing();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (lx, ly);
    for _ in 0..HZ * 2 {
        if run.at == door.room {
            break;
        }
        run.step(&west);
    }
    assert_eq!(run.at, door.room, "the west door leads into the hall");
    run.clear_for_test();
    for _ in 0..HZ * 6 {
        if run.at == o.map {
            break;
        }
        run.step(&west);
    }
    assert_eq!(run.at, o.map, "west out of the hall onto the map");
    for _ in 0..HZ {
        run.step(&west);
        assert_eq!(run.at, o.map, "west held through the doorway walks nowhere back");
    }
    run.step(&BTreeMap::new());
    for _ in 0..HZ * 2 {
        if run.at == door.room {
            break;
        }
        run.step(&west);
    }
    assert_eq!(run.at, door.room, "a fresh press walks back in");
}

/// How long a company may keep the party before the sweep puts it down,
/// so a slow fight is not mistaken for a floor that will not end.
const SOFTLOCK_FIGHT: u64 = 5 * HZ as u64;
/// No new tile, and no cleared chamber, for this long: the run is sitting.
const SOFTLOCK_STALL: u64 = 12 * HZ as u64;
/// A floor that has not reached its stairs by here is not going to.
const SOFTLOCK_BUDGET: u64 = 8 * 60 * HZ as u64;

/// Keep one uncleared chamber until it is cleared, so a knight on the
/// boundary of two does not turn around every tick and stand still.
fn softlock_aim(run: &Run, aim: &mut Option<u8>) -> Option<u8> {
    if let Some(i) = *aim {
        let still = run.dungeon.open.as_ref().is_some_and(|o| {
            o.chambers.get(usize::from(i)).is_some_and(|c| !c.cleared && c.kind != RoomKind::Start)
        });
        if still {
            return *aim;
        }
    }
    *aim = softlock_goal_home(run);
    *aim
}

fn softlock_goal_at(run: &Run, home: Option<u8>) -> Option<(f32, f32)> {
    let o = run.dungeon.open.as_ref()?;
    if let Some(i) = home {
        return o.chambers.get(usize::from(i)).map(|c| c.centre());
    }
    softlock_goal(run)
}

fn softlock_goal_home(run: &Run) -> Option<u8> {
    let o = run.dungeon.open.as_ref()?;
    if run.at != o.map {
        return None;
    }
    let hero = run.players.values().find(|h| h.hp > 0 && !h.stone && !h.absent)?;
    let here = (hero.x, hero.y);
    o.chambers
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.cleared && c.kind != RoomKind::Start)
        .min_by(|(_, a), (_, b)| {
            let da = (a.centre().0 - here.0).hypot(a.centre().1 - here.1);
            let db = (b.centre().0 - here.0).hypot(b.centre().1 - here.1);
            da.total_cmp(&db).then(a.dist.cmp(&b.dist))
        })
        .map(|(i, _)| i as u8)
}
/// Where the sweep is trying to stand: the nearest uncleared chamber, or
/// the stairs once the guardian's company is down.
fn softlock_goal(run: &Run) -> Option<(f32, f32)> {
    let o = run.dungeon.open.as_ref()?;
    let hero = run.players.values().find(|h| h.hp > 0 && !h.stone && !h.absent)?;
    let here = (hero.x, hero.y);
    let next = o
        .chambers
        .iter()
        .filter(|c| !c.cleared && c.kind != RoomKind::Start)
        .min_by(|a, b| {
            let da = (a.centre().0 - here.0).hypot(a.centre().1 - here.1);
            let db = (b.centre().0 - here.0).hypot(b.centre().1 - here.1);
            da.total_cmp(&db).then(a.dist.cmp(&b.dist))
        });
    if let Some(ch) = next {
        return Some(ch.centre());
    }
    let g = &o.chambers[usize::from(o.guardian)];
    let (x, y, w, h) = g.rect;
    let m = &run.dungeon.rooms[o.map];
    (y..y + h)
        .flat_map(|r| (x..x + w).map(move |c| (c, r)))
        .find(|&(c, r)| m.tile(i32::from(c), i32::from(r)) == Tile::Stairs)
        .map(|(c, r)| ((f32::from(c) + 0.5) * TILE_UNITS, (f32::from(r) + 0.5) * TILE_UNITS))
        .or(Some(g.centre()))
}

fn softlock_on_stairs(run: &Run) -> bool {
    let Some(o) = run.dungeon.open.as_ref() else {
        return false;
    };
    if run.at != o.map {
        return false;
    }
    let g = &o.chambers[usize::from(o.guardian)];
    if g.guarded() && !g.cleared {
        return false;
    }
    let m = &run.dungeon.rooms[o.map];
    run.players.values().any(|h| {
        h.hp > 0
            && !h.absent
            && m.tile((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32) == Tile::Stairs
    })
}

/// The floor's way on: down the stairs, onto them, or home. The last floor
/// lays no stairs; clearing its chambers is its end.
fn softlock_done(run: &Run, start: u32, depth: u32) -> bool {
    if run.floor() > start || run.phase == Phase::Won || softlock_on_stairs(run) {
        return true;
    }
    if depth >= DEEPEST {
        let Some(o) = run.dungeon.open.as_ref() else {
            return false;
        };
        return o.chambers.iter().filter(|c| c.kind != RoomKind::Start).all(|c| c.cleared);
    }
    false
}

fn seal_holds_a_living_knight(run: &Run) -> Result<(), String> {
    let Some(o) = run.dungeon.open.as_ref() else {
        return Ok(());
    };
    let g = &o.chambers[usize::from(o.guardian)];
    if !g.sealed {
        return Ok(());
    }
    let inside = run.players.values().any(|h| h.hp > 0 && g.holds(h.x, h.y, 0.0));
    if inside {
        return Ok(());
    }
    Err(format!(
        "guardian sealed with no living knight inside at tick {} phase {:?}",
        run.tick, run.phase
    ))
}

fn lead_tile(run: &Run) -> (i32, i32) {
    run.players
        .values()
        .find(|h| h.hp > 0 && !h.stone && !h.absent)
        .map(|h| ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32))
        .unwrap_or((0, 0))
}

/// A seat that can still walk, so a parked knight is never the only one.
fn park_a_seat(run: &mut Run) {
    let ids: Vec<u32> = run.players.iter().filter(|(_, h)| h.hp > 0 && !h.absent).map(|(&id, _)| id).collect();
    let walkers: Vec<u32> = ids.iter().copied().filter(|&id| !run.away(id)).collect();
    if walkers.len() < 2 {
        return;
    }
    let id = *walkers.iter().max().unwrap();
    run.set_away(id, true);
}

fn way_out(run: &Run) -> (i8, i8) {
    let Some(map_room) = run.dungeon.open.as_ref().map(|o| o.map) else {
        return (0, 0);
    };
    let dir = (0..4).find(|&d| run.dungeon.neighbour(run.at, d) == Some(map_room));
    dir.map(|d| {
        let (dx, dy) = DIRS[d];
        (dx as i8, dy as i8)
    })
    .unwrap_or((0, 0))
}

/// Foes the party has been trying to clear for too long go down, so the sweep
/// measures the floor and not how long a company takes to fall. Killing them
/// does not clear the chamber: a knight still has to walk in.
fn put_down_lingering(run: &mut Run, t: u64, fight_at: &mut Option<(u64, Option<u8>)>, goal_home: Option<u8>) {
    let started = match *fight_at {
        Some((at, h)) if h == goal_home => at,
        _ => {
            *fight_at = Some((t, goal_home));
            return;
        }
    };
    if t - started < SOFTLOCK_FIGHT {
        return;
    }
    let hero = run.players.values().find(|h| h.hp > 0 && !h.stone && !h.absent).map(|h| (h.x, h.y));
    for e in &mut run.enemies {
        if e.hp == 0 {
            continue;
        }
        let aimed = goal_home.is_some_and(|h| e.home == Some(h));
        let near = hero.is_some_and(|(x, y)| !e.dormant && (e.x - x).hypot(e.y - y) < 24.0);
        if aimed || near {
            e.hp = 0;
        }
    }
    *fight_at = None;
}

/// One seeded floor, played by the sweep's bots. `Err` is a softlock.
fn walk_open_floor(seed: u64, knights: u32, absent_host: bool, seat_away: bool, depth: u32) -> Result<(), String> {
    let mut run = open_run(seed, knights, absent_host, depth);
    if seat_away {
        park_a_seat(&mut run);
    }
    let ids: Vec<u32> = run.players.keys().copied().collect();
    if !ids.iter().any(|&id| run.players[&id].hp > 0 && !run.players[&id].absent && !run.away(id)) {
        return Err("no knight left to walk".into());
    }
    let mut paths: BTreeMap<u32, Vec<(i32, i32)>> = BTreeMap::new();
    let start = run.floor();
    let mut stall_tile = lead_tile(&run);
    let mut stall_at = 0u64;
    let mut cleared = open(&run).chambers.iter().filter(|c| c.cleared).count();
    let mut phase = run.phase;
    let mut phase_at = 0u64;
    let mut fight_at: Option<(u64, Option<u8>)> = None;
    let mut aim: Option<u8> = None;
    for t in 0..SOFTLOCK_BUDGET {
        if softlock_done(&run, start, depth) {
            return Ok(());
        }
        seal_holds_a_living_knight(&run)?;
        if run.phase == Phase::Wiped {
            return Err(format!("wiped at tick {t}"));
        }
        if run.phase != phase {
            phase = run.phase;
            phase_at = t;
        } else if run.phase != Phase::Exploring && t - phase_at > SOFTLOCK_STALL {
            return Err(format!("sat in {:?} for {} ticks at {t}", run.phase, t - phase_at));
        }
        let goal_home = softlock_aim(&run, &mut aim);
        put_down_lingering(&mut run, t, &mut fight_at, goal_home);
        let on_map = run.dungeon.open.as_ref().is_some_and(|o| run.at == o.map);
        let inputs: BTreeMap<u32, Input> = if on_map {
            let Some(goal) = softlock_goal_at(&run, goal_home) else {
                return Err(format!("no chamber and no stairs at tick {t}"));
            };
            ids.iter()
                .map(|&id| {
                    let path = paths.entry(id).or_default();
                    path.clear();
                    let mut input = bot(&run, id, goal, path);
                    // A key held out of a doorway walks nowhere until it is
                    // let go. A bot that never releases it stands forever.
                    if t > stall_at + 8 && (t - stall_at).is_multiple_of(12) {
                        input.move_x = 0;
                        input.move_y = 0;
                    }
                    (id, input)
                })
                .collect()
        } else {
            let (mx, my) = way_out(&run);
            ids.iter()
                .map(|&id| {
                    let hero = &run.players[&id];
                    let input = if hero.hp == 0 || hero.stone || run.away(id) {
                        Input::default()
                    } else {
                        Input { move_x: mx, move_y: my, ..Input::default() }
                    };
                    (id, input)
                })
                .collect()
        };
        for hero in run.players.values_mut() {
            if hero.hp > 0 {
                hero.hp = hero.max_hp;
            }
        }
        run.step_sim_for_test(&inputs);
        seal_holds_a_living_knight(&run)?;
        let now_cleared = run.dungeon.open.as_ref().map_or(cleared, |o| o.chambers.iter().filter(|c| c.cleared).count());
        let tile = lead_tile(&run);
        if tile != stall_tile || now_cleared != cleared || run.floor() != start {
            stall_tile = tile;
            stall_at = t;
            cleared = now_cleared;
        } else if t - stall_at > SOFTLOCK_STALL {
            let foes = run.enemies.iter().filter(|e| e.hp > 0).count();
            return Err(format!(
                "no move for {} ticks at {tile:?} phase {:?} room {} foes {foes} cleared {cleared}",
                t - stall_at,
                run.phase,
                run.at
            ));
        }
    }
    let foes = run.enemies.iter().filter(|e| e.hp > 0).count();
    let (cleared, chambers) = run
        .dungeon
        .open
        .as_ref()
        .map(|o| (o.chambers.iter().filter(|c| c.cleared).count(), o.chambers.len()))
        .unwrap_or((0, 0));
    Err(format!(
        "not at the stairs after {SOFTLOCK_BUDGET} ticks, floor {} phase {:?} cleared {cleared}/{chambers} foes {foes}",
        run.floor(),
        run.phase
    ))
}

/// Seeded open floors with simple bots: walk `open_path` to the nearest
/// uncleared chamber, fire at the nearest foe, and put down a company that
/// will not fall. A softlock is a sealed guardian with nobody living inside,
/// a phase that never ends, or a floor that never reaches its stairs.
///
/// `OPEN_SOFTLOCK_SEEDS` overrides the seed count (default 40).
#[test]
#[ignore]
fn seeded_open_floors_do_not_softlock() {
    let seeds: u64 = std::env::var("OPEN_SOFTLOCK_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(40);
    let only = std::env::var("OPEN_SOFTLOCK_ONLY").ok().and_then(|s| s.parse::<u64>().ok());
    let only_depth = std::env::var("OPEN_SOFTLOCK_DEPTH").ok().and_then(|s| s.parse::<u32>().ok());
    for seed in 0..seeds {
        if only.is_some_and(|n| n != seed) {
            continue;
        }
        for depth in 1..=6u32 {
            if only_depth.is_some_and(|d| d != depth) {
                continue;
            }
            let knights = 1 + ((seed as u32 + depth) % 6);
            let absent_host = seed % 5 == 0 && knights > 1;
            let seat_away = seed % 4 == 1 && knights > 1 && !(absent_host && knights < 3);
            if let Err(why) = walk_open_floor(seed, knights, absent_host, seat_away, depth) {
                panic!("seed {seed} depth {depth} knights {knights} absent {absent_host} away {seat_away}: {why}");
            }
        }
    }
}

/// A deepest open floor with a full party round-trips through the compact
/// tile run, and a save that still lists tile names loads to the same cells.
#[test]
fn save_round_trips_and_old_tile_lists_still_load() {
    let mut run = Run::new(11, 7, None);
    for id in 2..=MAX_PLAYERS {
        run.join_seat(id, "Knight");
    }
    run.open_floor_for_test(DEEPEST);
    assert_eq!(run.players.len(), MAX_PLAYERS as usize);
    assert_eq!(run.dungeon.depth, DEEPEST);
    let map = run.dungeon.open.as_ref().unwrap().map;
    let cells = run.dungeon.rooms[map].tiles_for_test().to_vec();
    assert!(cells.len() > 10_000, "the open map is the bulk of the save");

    let compact = serde_json::to_vec(&run).unwrap();
    let back: Run = serde_json::from_slice(&compact).unwrap();
    assert_eq!(back.dungeon.rooms[map].tiles_for_test(), cells.as_slice());
    assert_eq!(serde_json::to_vec(&back).unwrap(), compact, "the run round-trips exactly");

    let old = crate::rules::layout::expand_saved_tiles(&String::from_utf8(compact.clone()).unwrap());
    // The compact run spares at least four bytes a cell. (Measured per cell,
    // not as half the save: the run's own book now outweighs its map since
    // the 10-30 catalogue grew to two hundred more cards.)
    assert!(old.len() > compact.len() + cells.len() * 4, "old {} compact {} cells {}", old.len(), compact.len(), cells.len());
    let from_old: Run = serde_json::from_str(&old).unwrap();
    assert_eq!(from_old.dungeon.rooms[map].tiles_for_test(), cells.as_slice());
    assert_eq!(serde_json::to_vec(&from_old).unwrap(), compact);

    let mut saved: serde_json::Value = serde_json::from_slice(&compact).unwrap();
    compact_save(&mut saved);
    let smaller = serde_json::to_vec(&saved).unwrap();
    assert!(smaller.len() < compact.len(), "defaults left out: {} -> {}", compact.len(), smaller.len());
    let from_small: Run = serde_json::from_slice(&smaller).unwrap();
    assert_eq!(from_small.dungeon.rooms[map].tiles_for_test(), cells.as_slice());
    assert_eq!(serde_json::to_vec(&from_small).unwrap(), compact, "a compact save loads the same run");
    let enemies_old = serde_json::to_vec(&serde_json::from_slice::<serde_json::Value>(&compact).unwrap()["enemies"]).unwrap().len();
    let enemies_new = serde_json::to_vec(&saved["enemies"]).unwrap().len();
    assert!(enemies_new * 2 < enemies_old, "foe defaults {enemies_old} -> {enemies_new}");
    let value: serde_json::Value = serde_json::from_slice(&compact).unwrap();
    let run_tiles = &value["dungeon"]["rooms"][map]["tiles"];
    assert!(run_tiles.is_string(), "the open map is one run, not a list of names");
    assert!(run_tiles.as_str().unwrap().len() < 8_000);

    // A save written before the run: names, not glyphs. Every cell kind is here.
    let raw = r#"{"cell":[1,2],"kind":"fight","doors":[true,false,false,true],"cols":8,"rows":1,"tiles":["wall","floor","block","hazard","door","stairs","ledge","bog"],"visited":true,"cleared":false,"roster":[],"items":[],"chest":null}"#;
    let room: Room = serde_json::from_str(raw).unwrap();
    assert_eq!(
        room.tiles_for_test(),
        &[
            Tile::Wall,
            Tile::Floor,
            Tile::Block,
            Tile::Hazard,
            Tile::Door,
            Tile::Stairs,
            Tile::Ledge,
            Tile::Bog,
        ]
    );
    let spelled = serde_json::to_string(&room).unwrap();
    assert!(spelled.contains("\"tiles\":\"#.B~D>=o\""), "{spelled}");
    let again: Room = serde_json::from_str(&spelled).unwrap();
    assert_eq!(again.tiles_for_test(), room.tiles_for_test());
    assert_eq!(serde_json::to_string(&again).unwrap(), spelled);
}
