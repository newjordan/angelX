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
fn bot(run: &Run, id: u32, goal: (f32, f32), path: &mut Vec<(i32, i32)>) -> Input {
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
