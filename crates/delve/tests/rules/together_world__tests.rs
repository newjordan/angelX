use super::*;
use crate::rules::home::{DESCEND_HOLD, Home};

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
    // Foot objectives, all ordinary door lanes and a four-friend arrival fan
    // must be reachable in each authored surface room, not just on the map.
    use super::super::{countryside, pastimes};
    for &(kind, _) in &ROOMS {
        let room = &run.dungeon.rooms[room_of(&run.dungeon, kind).unwrap()];
        if !countryside::PLACES.contains(&kind)
            && !countryside::WEST_PLACES.contains(&kind)
            && !countryside::EAST_PLACES.contains(&kind)
            && !countryside::NORTH_PLACES.contains(&kind)
            && !super::super::coastal::PLACES.contains(&kind)
            && !super::super::woodland::PLACES.contains(&kind)
            && !super::super::wetland::PLACES.contains(&kind)
            && !super::super::aerie::PLACES.contains(&kind)
            && !super::super::mummers::PLACES.contains(&kind)
            && !super::super::windcountry::PLACES.contains(&kind)
        {
            continue;
        }
        let mut reached = vec![false; room.cols * room.rows];
        let mut todo = vec![(11, 6)];
        while let Some((x, y)) = todo.pop() {
            if x < 0 || y < 0 || x >= room.cols as i32 || y >= room.rows as i32 {
                continue;
            }
            let k = y as usize * room.cols + x as usize;
            if reached[k]
                || !walkable(
                    room,
                    (x as f32 + 0.5) * TILE_UNITS,
                    (y as f32 + 0.5) * TILE_UNITS,
                )
            {
                continue;
            }
            reached[k] = true;
            todo.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
        }
        let mut objectives: Vec<_> = pastimes::Game::ALL
            .into_iter()
            .filter(|g| g.room() == kind)
            .map(|g| g.start())
            .collect();
        objectives.extend(
            countryside::PLOTS
                .iter()
                .filter(|p| p.room == kind)
                .map(|p| p.at),
        );
        objectives.extend(
            countryside::RELICS
                .iter()
                .filter(|p| p.room == kind)
                .map(|p| p.at),
        );
        objectives.extend(
            countryside::LAMPS
                .iter()
                .filter(|l| l.room == kind)
                .map(|l| l.at),
        );
        objectives.extend(
            countryside::SURVEY
                .iter()
                .filter(|(room, _)| *room == kind)
                .map(|(_, at)| *at),
        );
        objectives.extend(
            countryside::PICNIC
                .iter()
                .filter(|(room, _)| *room == kind)
                .map(|(_, at)| *at),
        );
        objectives.extend(
            countryside::FORECAST
                .iter()
                .filter(|(room, _)| *room == kind)
                .map(|(_, at)| *at),
        );
        objectives.extend(
            super::super::coastal::ERRAND
                .iter()
                .filter(|(room, _)| *room == kind)
                .map(|(_, at)| *at),
        );
        objectives.extend(super::super::wetland::ERRAND.iter().filter(|(room,_)| *room==kind).map(|(_,at)| *at));
        if kind == RoomKind::Crag {
            objectives.extend(
                pastimes::SLED_GATES
                    .iter()
                    .map(|&(x, y)| (x as f32, y as f32)),
            );
        }
        if kind == RoomKind::Causeway {
            objectives.extend(
                pastimes::HURDLES
                    .iter()
                    .flat_map(|&x| [6.0, 7.0, 8.0].map(|y| (x, y))),
            );
        }
        if kind == RoomKind::Village {
            objectives.extend(pastimes::MAILBOXES);
        }
        if kind == RoomKind::Fairground {
            objectives.extend(pastimes::FLAGS);
        }
        if kind == RoomKind::Riverbank {
            for tick in 0..HZ * 8 {
                for i in 0..6 {
                    objectives.push(pastimes::wake(i, u64::from(tick)));
                }
            }
        }
        objectives.extend(
            super::super::woodland::ERRAND
                .iter()
                .filter(|(room, _)| *room == kind)
                .map(|(_, at)| *at),
        );
        objectives.extend(super::super::aerie::DOCUMENTS.iter().filter(|(k,_,_)| *k == kind).map(|(_,at,_)| *at));
        if kind == RoomKind::Aviary { objectives.push(super::super::aerie::CLERK); }
        if kind == RoomKind::GooseGreen {objectives.push(super::super::windcountry::CLOTH);}
        if kind == RoomKind::Kitewright {objectives.push(super::super::windcountry::MAKER);}
        if kind == RoomKind::SignalHill {objectives.extend(super::super::windcountry::FLAGS);}

        for (x, y) in objectives {
            assert!(
                reached[y as usize * room.cols + x as usize],
                "{kind:?}: unreachable objective {x},{y}"
            );
            assert!(
                Grid {
                    room,
                    barred: false
                }
                .clear(x * TILE_UNITS, y * TILE_UNITS, HERO_RADIUS, Mover::Hero)
            );
        }
        for dir in 0..4 {
            if !room.doors[dir] {
                continue;
            }
            for offset in [-3.6f32, -1.2, 1.2, 3.6] {
                let (x, y) = match dir {
                    0 => (room.width() / 2.0 + offset, 3.3),
                    1 => (room.width() - 3.3, room.height() / 2.0 + offset),
                    2 => (room.width() / 2.0 + offset, room.height() - 3.3),
                    _ => (3.3, room.height() / 2.0 + offset),
                };
                assert!(
                    Grid {
                        room,
                        barred: false
                    }
                    .clear(x, y, HERO_RADIUS, Mover::Hero),
                    "{kind:?} dir {dir} arrival {x},{y}"
                );
            }
        }
    }

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
        if let Dest::Room(kind, (x, y)) = e.to {
            let dest = &run.dungeon.rooms[room_of(&run.dungeon, kind).unwrap()];
            for n in 0..4 {
                // Match arrive()'s true four-knight fan, 2.4 arena units apart.
                let (hx, hy) = ((x - 1.8 + n as f32 * 1.2) * TILE_UNITS, y * TILE_UNITS);
                assert!(walkable(dest, hx, hy), "{} arrival {n}", e.id);
                assert!(
                    entrance_at(
                        kind,
                        (hx / TILE_UNITS).floor() as i32,
                        (hy / TILE_UNITS).floor() as i32
                    )
                    .is_none(),
                    "{} arrival bounces",
                    e.id
                );
            }
        }
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
    let run = at_the_gate();
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
    let forest = run
        .dungeon
        .neighbour(gate, 2)
        .expect("south road reaches the forest");
    assert_eq!(run.dungeon.rooms[forest].kind, RoomKind::Forest);
    assert!(run.dungeon.rooms[gate].doors[2]);
    assert!(
        run.dungeon.rooms[forest].doors[0],
        "forest road returns to the gate"
    );
    ordinary_road_journey_regression();
}

#[test]
fn a_mirror_of_the_world_follows_the_host_through_the_gate() {
    shield_surfing_accepts_only_the_published_gold_wave();
    shield_surfing_walks_in_and_clears_each_last_gold_frame();
    washtub_walking_regression();
    let mut host = at_the_gate();
    take(&mut host, entrance("gate"));
    let mut friend = host.wire_copy();
    assert_eq!(friend.room().kind, RoomKind::Home);

    // Both forest games use the same host state and live projection as travel.
    use super::super::forest;
    let clearing = room_of(&host.dungeon, RoomKind::Forest).unwrap();
    host.arrive(clearing, forest::LANTERN);
    host.tick_forest();
    assert!(host.forest.hunt_left > 0);
    host.join("Friend");
    let guest = 2;
    for i in 0..forest::FLY_COUNT {
        let (x, y) = forest::firefly(i, host.tick);
        assert!(walkable(host.room(), x, y), "firefly {i} is reachable");
        let h = host
            .players
            .get_mut(&if i % 2 == 0 { 1 } else { guest })
            .unwrap();
        (h.x, h.y) = (x, y);
        host.tick_forest();
    }
    assert_eq!(host.forest.best, 8);
    assert_eq!(host.forest.hunts, 1);
    assert_eq!(host.forest.hunt_left, 0);
    let score = host.score;
    host.tick_forest();
    assert_eq!(host.score, score, "a finished round pays only once");
    friend = host.wire_copy();
    assert_eq!(friend.room().kind, RoomKind::Forest);
    assert_eq!(friend.forest, host.forest);

    let place = |run: &mut Run, id, (x, y): (f32, f32)| {
        let h = run.players.get_mut(&id).unwrap();
        (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
    };
    place(&mut host, 1, forest::MEMORY);
    host.tick_forest();
    assert!(host.forest.singing);
    while host.forest.show_left > 0 {
        host.tick_forest();
    }
    // Wrong notes end a round without a reward, but allow a retry.
    let wrong = (host.forest.melody(0) + 1) % forest::RUNES.len();
    place(&mut host, 1, forest::RUNES[wrong]);
    host.tick_forest();
    assert!(!host.forest.singing);
    assert_eq!(host.score, score);
    place(&mut host, 1, forest::MEMORY);
    host.tick_forest();
    while host.forest.show_left > 0 {
        host.tick_forest();
    }
    for note in 0..forest::SONG_LEN {
        let pad = forest::RUNES[host.forest.melody(note)];
        place(&mut host, 1, pad);
        host.tick_forest();
        assert_eq!(host.forest.note, note + 1);
    }
    assert!(!host.forest.singing);
    assert_eq!(host.forest.songs, 1);
    assert_eq!(host.score, score + 100);
    friend = host.wire_copy();
    assert_eq!(friend.forest, host.forest);
    let mut saved: Run = serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    assert!(saved.valid_snapshot());
    assert_eq!(saved.forest, host.forest);
    place(&mut saved, 1, forest::LANTERN);
    saved.tick_forest();
    assert!(saved.forest.hunt_left > 0);
    saved.forest.hunt_left = 1;
    saved.tick_forest();
    assert_eq!(saved.forest.hunt_left, 0, "the hunt times out");
    saved.tick_forest();
    assert_eq!(
        saved.forest.hunt_left, 0,
        "standing still cannot restart it"
    );
    saved.arrive(
        room_of(&saved.dungeon, RoomKind::Gate).unwrap(),
        (12.0, 9.5),
    );
    assert!(!saved.forest.singing);
    assert_eq!(saved.forest.hunt_left, 0);
    assert_eq!(saved.forest.songs, 1, "leaving keeps the bests");

    // The new arcade rides the same host/mirror/save contract as the clearing.
    use super::super::{countryside, pastimes};
    use pastimes::Game;
    let place = |run: &mut Run, id, (x, y): (f32, f32)| {
        let h = run.players.get_mut(&id).unwrap();
        (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
    };
    let fire = |id| {
        BTreeMap::from([(
            id,
            Input {
                fire: true,
                ..Input::default()
            },
        )])
    };
    // Plots, lamps and the folk answer the interact key, not the fire key.
    let meet = |id| {
        BTreeMap::from([(
            id,
            Input {
                interact: true,
                ..Input::default()
            },
        )])
    };
    let row = room_of(&host.dungeon, RoomKind::Village).unwrap();
    host.arrive(row, Game::Broadsheet.start());
    place(&mut host, 2, (21.0, 9.0));
    host.tick_pastimes(&BTreeMap::new());
    assert!(host.pastimes.round.is_some());
    friend = host.wire_copy();
    assert_eq!(friend.pastimes, host.pastimes);
    let mask = host.pastimes.round.as_ref().unwrap().recipients;
    let score = host.score;
    for (_, pad) in pastimes::MAILBOXES
        .into_iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
    {
        // Each paper knight walks their own round; the starter's is this one.
        let id = 1;
        place(&mut host, id, pad);
        host.tick_pastimes(&BTreeMap::new());
        host.tick_pastimes(&fire(id));
        host.tick_pastimes(&fire(id)); // held F does not deliver a second sheet
    }
    assert!(host.pastimes.round.is_none());
    assert_eq!(host.home.countryside.games.wins[0], 1);
    assert!(host.score > score);
    let after = host.score;
    host.tick_pastimes(&fire(1));
    assert_eq!(host.score, after);
    friend = host.wire_copy();
    assert_eq!(friend.home.countryside.games, host.home.countryside.games);
    let mut checkpoint: Run = serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    assert!(checkpoint.valid_snapshot());
    assert_eq!(
        checkpoint.home.countryside.games,
        host.home.countryside.games
    );

    // Both walk/roll courses require the ordered target, not an out-of-order flag.
    for game in [Game::Barrel, Game::Washtub] {
        checkpoint.arrive(
            room_of(&checkpoint.dungeon, game.room()).unwrap(),
            game.start(),
        );
        place(&mut checkpoint, 2, (21.0, 11.5));
        checkpoint.tick_pastimes(&BTreeMap::new());
        let out_of_order = if game == Game::Barrel {
            pastimes::FLAGS[3]
        } else {
            pastimes::wake(3, checkpoint.tick)
        };
        place(&mut checkpoint, 1, out_of_order);
        checkpoint.tick_pastimes(&BTreeMap::new());
        assert_eq!(checkpoint.pastimes.round.as_ref().unwrap().progress, 0);
        for i in 0..6 {
            let target = if game == Game::Barrel {
                pastimes::FLAGS[i]
            } else {
                pastimes::wake(i, checkpoint.tick)
            };
            place(&mut checkpoint, 1, target);
            checkpoint.tick_pastimes(&BTreeMap::new());
        }
        assert!(checkpoint.pastimes.round.is_none());
        assert_eq!(checkpoint.home.countryside.games.wins[game.index()], 1);
        assert!(checkpoint.home.countryside.games.best[game.index()] > 0);
    }
    // Turnip presses are edge-triggered, quantized to a beat, and shared.
    checkpoint.arrive(
        room_of(&checkpoint.dungeon, RoomKind::Fairground).unwrap(),
        Game::Turnip.start(),
    );
    place(&mut checkpoint, 2, (21.0, 11.5));
    checkpoint.tick_pastimes(&BTreeMap::new());
    checkpoint.tick_pastimes(&fire(1));
    assert_eq!(checkpoint.pastimes.round.as_ref().unwrap().progress, 1);
    for _ in 0..HZ * 2 {
        checkpoint.tick_pastimes(&fire(1));
    }
    assert_eq!(
        checkpoint.pastimes.round.as_ref().unwrap().progress,
        1,
        "holding F cannot juggle"
    );
    for n in 1..8 {
        // Walk time forward to a distinct landing window, release then tap.
        let elapsed = Game::Turnip.time() - checkpoint.pastimes.round.as_ref().unwrap().left;
        let next = (elapsed / HZ + 1) * HZ;
        for _ in elapsed..next {
            checkpoint.tick_pastimes(&BTreeMap::new());
        }
        checkpoint.tick_pastimes(&fire(1));
        if n < 7 {
            assert_eq!(checkpoint.pastimes.round.as_ref().unwrap().progress, n + 1);
        }
    }
    assert!(checkpoint.pastimes.round.is_none());
    assert_eq!(
        checkpoint.home.countryside.games.wins[Game::Turnip.index()],
        1
    );
    checkpoint.arrive(row, Game::Broadsheet.start());
    place(&mut checkpoint, 2, (21.0, 11.5));
    checkpoint.tick_pastimes(&BTreeMap::new());
    let valid = checkpoint.clone();
    let wire = serde_json::to_vec(&valid).unwrap();
    let mut restored: Run = serde_json::from_slice(&wire).unwrap();
    assert!(restored.valid_snapshot());
    assert_eq!(restored.pastimes, valid.pastimes);
    let round = restored.pastimes.round.as_mut().unwrap();
    round.left = 1;
    let i = round.recipients.trailing_zeros() as usize;
    round.tried = round.recipients & !(1 << i);
    round.progress = 2; // This delivery would actually win without the expiry guard.
    place(&mut restored, 1, pastimes::MAILBOXES[i]);
    let score = restored.score;
    restored.tick_pastimes(&fire(1));
    assert!(
        restored.pastimes.round.is_none(),
        "expiry takes precedence over delivery"
    );
    assert_eq!(restored.score, score);
    restored.arrive(row, Game::Broadsheet.start());
    restored.tick_pastimes(&BTreeMap::new());
    restored.players.get_mut(&1).unwrap().stone = true;
    restored.players.get_mut(&2).unwrap().hp = 0;
    restored.tick_pastimes(&fire(1));
    assert!(
        restored.pastimes.round.is_none(),
        "no dead or stone contributors"
    );
    checkpoint.arrive(
        room_of(&checkpoint.dungeon, RoomKind::Gate).unwrap(),
        (12.0, 9.5),
    );
    assert!(checkpoint.pastimes.round.is_none());
    assert_eq!(
        checkpoint.home.countryside.games.wins[0], 1,
        "exit keeps records"
    );

    western_circuit_regression(checkpoint.clone());
    eastern_adventure_regression(checkpoint.clone());
    northern_adventure_regression(checkpoint.clone());
    coastal_adventure_regression(checkpoint.clone());
    woodland_adventure_regression(checkpoint.clone());
    wetland_adventure_regression(checkpoint.clone());
    aerie_adventure_regression(checkpoint.clone());
    mummers_adventure_regression(checkpoint.clone());
    hippodrome_adventure_regression(checkpoint.clone());
    windcountry_adventure_regression(checkpoint.clone());
    original_turnip_cue_regression();
    broadsheet_walking_regression();
    // The realm, not a practice round, owns the paid works and one-time ramble.
    let mut country = countryside::Country::default();
    let mut treasury = crate::realm::Spoils::default();
    let marks = BTreeMap::from_iter(
        (0..8)
            .map(|i| (format!("country:visit:{i}"), 1))
            .chain((0..3).map(|i| (format!("country:relic:{i}"), 1))),
    );
    assert!(countryside::settle(&mut country, &mut treasury, &marks).0);
    let paid = treasury.clone();
    assert!(!countryside::settle(&mut country, &mut treasury, &marks).0);
    assert_eq!(treasury, paid);
    let request = BTreeMap::from([("country:build:inn".into(), 1)]);
    assert!(
        !countryside::settle(&mut country, &mut treasury, &request).0,
        "insufficient materials spend nothing"
    );
    assert_eq!(treasury, paid);
    treasury.add(Spoil::Bone, 4);
    treasury.add(Spoil::Wax, 3);
    assert!(countryside::settle(&mut country, &mut treasury, &request).0);
    let paid = treasury.clone();
    assert!(!countryside::settle(&mut country, &mut treasury, &request).0);
    assert_eq!(treasury, paid);
    // An idle knight must not monopolize a plot while a friend holds E.
    let plot = &countryside::PLOTS[0];
    checkpoint.arrive(room_of(&checkpoint.dungeon, plot.room).unwrap(), plot.at);
    place(&mut checkpoint, 1, plot.at);
    place(&mut checkpoint, 2, plot.at);
    for _ in 0..super::super::home::BUY_HOLD {
        checkpoint.tick_countryside(&meet(2));
    }
    assert_eq!(checkpoint.marks.get("country:build:bakery"), Some(&2));
    checkpoint.marks.remove("country:build:bakery");
    for _ in 0..super::super::home::BUY_HOLD * 2 {
        checkpoint.tick_countryside(&meet(2));
    }
    assert!(
        !checkpoint.marks.contains_key("country:build:bakery"),
        "continued hold is latched"
    );
    checkpoint.tick_countryside(&BTreeMap::new());
    for _ in 0..super::super::home::BUY_HOLD - 1 {
        checkpoint.tick_countryside(&meet(2));
    }
    place(&mut checkpoint, 2, (12.0, 7.0));
    checkpoint.tick_countryside(&meet(2));
    place(&mut checkpoint, 2, plot.at);
    checkpoint.tick_countryside(&meet(2));
    assert!(
        !checkpoint.marks.contains_key("country:build:bakery"),
        "leaving resets the hold"
    );
    assert_eq!(checkpoint.build_hold, Some((0, 2, 1)));

    // Extra places must not replace missing core visits or revoke old rewards.
    let legacy: Home = serde_json::from_value(serde_json::json!({"countryside": {
        "visited": 63, "relics": 7, "built": 7,
        "ramble_paid": true, "relics_paid": true
    }}))
    .unwrap();
    assert!(legacy.countryside.valid());
    assert_eq!(legacy.countryside.built, 7);
    assert_eq!(legacy.countryside.lamps, 0);
    let mut skipped = countryside::Country {
        visited: 254,
        ..Default::default()
    };
    let mut purse = treasury.clone();
    let before = purse.clone();
    countryside::settle(&mut skipped, &mut purse, &request);
    assert!(!skipped.has(2));
    assert_eq!(purse, before);

    checkpoint.home.countryside = country;
    let meadow = room_of(&checkpoint.dungeon, RoomKind::Meadow).unwrap();
    checkpoint.arrive(meadow, (12.0, 9.0));
    assert!(open(entrance("meadow-coach"), &checkpoint.home));
    take(&mut checkpoint, entrance("meadow-coach"));
    assert_eq!(checkpoint.room().kind, RoomKind::Gate);

    // Trace the production beacon/bell -> lantern marks -> save settlement ->
    // ferry purchase path, including co-op and replayed transaction marks.
    use countryside::LAMPS;
    checkpoint.treasury.add(Spoil::Gold, 1000);
    checkpoint.treasury.add(Spoil::Bone, 20);
    checkpoint.treasury.add(Spoil::Ore, 20);
    checkpoint.home.countryside.built |= 2;
    let lamp = &LAMPS[0];
    checkpoint.arrive(room_of(&checkpoint.dungeon, lamp.room).unwrap(), lamp.at);
    checkpoint.marks.clear();
    checkpoint.home.countryside.relics &= !2;
    checkpoint.tick_countryside(&meet(1));
    assert!(!checkpoint.marks.contains_key("country:lamp:0"));
    checkpoint.home.countryside.relics |= 2;
    checkpoint.home.countryside.built &= !2;
    checkpoint.tick_countryside(&meet(1));
    assert!(!checkpoint.marks.contains_key("country:lamp:0"));
    checkpoint.home.countryside.built |= 2;
    let mut unfounded = countryside::Country::default();
    let mut empty = crate::realm::Spoils::default();
    countryside::settle(
        &mut unfounded,
        &mut empty,
        &BTreeMap::from([("country:lamp:0".into(), 1)]),
    );
    assert_eq!(unfounded.lamps, 0);

    let before = checkpoint.treasury.clone();
    countryside::settle(
        &mut checkpoint.home.countryside,
        &mut checkpoint.treasury,
        &BTreeMap::from([("country:build:ferry".into(), 1)]),
    );
    assert!(!checkpoint.home.countryside.has(3));
    assert_eq!(checkpoint.treasury, before, "locked ferry does not charge");
    for (i, lamp) in LAMPS.iter().enumerate() {
        checkpoint.marks.clear();
        checkpoint.arrive(room_of(&checkpoint.dungeon, lamp.room).unwrap(), lamp.at);
        checkpoint.tick_countryside(&BTreeMap::new());
        assert!(!checkpoint.marks.contains_key(&format!("country:lamp:{i}")));
        for hero in checkpoint.players.values_mut() {
            (hero.x, hero.y) = (lamp.at.0 * TILE_UNITS, lamp.at.1 * TILE_UNITS);
        }
        let both = BTreeMap::from([
            (
                1,
                Input {
                    interact: true,
                    ..Input::default()
                },
            ),
            (
                2,
                Input {
                    interact: true,
                    ..Input::default()
                },
            ),
        ]);
        checkpoint.tick_countryside(&both);
        assert_eq!(checkpoint.marks.get(&format!("country:lamp:{i}")), Some(&1));
        let before = checkpoint.treasury.clone();
        let marks = checkpoint.marks.clone();
        countryside::settle(
            &mut checkpoint.home.countryside,
            &mut checkpoint.treasury,
            &marks,
        );
        assert_eq!(checkpoint.home.countryside.lamps, (1 << (i + 1)) - 1);
        assert_eq!(
            checkpoint.treasury.get(Spoil::Gold),
            before.get(Spoil::Gold) + if i == 2 { 75 } else { 0 }
        );
        assert_eq!(
            checkpoint.treasury.get(Spoil::Wax),
            before.get(Spoil::Wax) + if i == 2 { 4 } else { 0 }
        );
        let paid = checkpoint.treasury.clone();
        countryside::settle(
            &mut checkpoint.home.countryside,
            &mut checkpoint.treasury,
            &marks,
        );
        checkpoint.tick_countryside(&both);
        assert_eq!(
            checkpoint.treasury, paid,
            "held E / retry cannot repay lights"
        );
        assert!(checkpoint.home.countryside.valid());
    }
    let restored: Home =
        serde_json::from_slice(&serde_json::to_vec(&checkpoint.home).unwrap()).unwrap();
    assert_eq!(restored.countryside.lamps, 7);
    assert!(restored.countryside.lamps_paid);
    assert_eq!(
        restored.countryside.visited, 255,
        "all eight places survive settlement"
    );
    checkpoint.arrive(
        room_of(&checkpoint.dungeon, RoomKind::Orchard).unwrap(),
        (12.0, 7.0),
    );
    friend = checkpoint.wire_copy();
    assert_eq!(friend.room().kind, RoomKind::Orchard);
    assert_eq!(friend.home.countryside, checkpoint.home.countryside);

    let ferry = &countryside::PLOTS[3];
    checkpoint.marks.clear();
    checkpoint.arrive(room_of(&checkpoint.dungeon, ferry.room).unwrap(), ferry.at);
    for _ in 0..super::super::home::BUY_HOLD {
        checkpoint.tick_countryside(&meet(1));
    }
    assert!(checkpoint.marks.contains_key("country:build:ferry"));
    let marks = checkpoint.marks.clone();
    countryside::settle(
        &mut checkpoint.home.countryside,
        &mut checkpoint.treasury,
        &marks,
    );
    assert!(checkpoint.home.countryside.has(3));
    let paid = checkpoint.treasury.clone();
    countryside::settle(
        &mut checkpoint.home.countryside,
        &mut checkpoint.treasury,
        &marks,
    );
    assert_eq!(checkpoint.treasury, paid, "ferry office is bought once");
    assert!(checkpoint.valid_snapshot());

    for (id, bit) in [
        ("gate-coach", 4),
        ("meadow-coach", 4),
        ("marsh-ferry", 8),
        ("quay-ferry", 8),
    ] {
        let e = entrance(id);
        let Dest::Room(target, _) = e.to else {
            unreachable!()
        };
        checkpoint.arrive(room_of(&checkpoint.dungeon, e.room).unwrap(), (12.0, 7.0));
        checkpoint.home.countryside.built &= !bit;
        for hero in checkpoint.players.values_mut() {
            (hero.x, hero.y) = entrance_centre(e);
        }
        for _ in 0..DESCEND_HOLD + 2 {
            checkpoint.step(&BTreeMap::new());
        }
        assert_eq!(checkpoint.room().kind, e.room, "{id} locked");
        checkpoint.home.countryside.built |= bit;
        assert!(open(e, &checkpoint.home));
        for _ in 0..DESCEND_HOLD + 2 {
            checkpoint.step(&BTreeMap::new());
        }
        assert_eq!(checkpoint.room().kind, target, "{id} arrived");
        for hero in checkpoint.players.values() {
            assert!(walkable(checkpoint.room(), hero.x, hero.y));
        }
        for _ in 0..DESCEND_HOLD * 2 {
            checkpoint.step(&BTreeMap::new());
        }
        assert_eq!(checkpoint.room().kind, target, "{id} cannot bounce");
    }
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
    let gate = room_of(&run.dungeon, RoomKind::Gate).unwrap();
    let (c, r, _, _) = super::super::countryside::COACH;
    run.dungeon.rooms[gate].set(c as usize, r as usize, Tile::Floor);
    assert!(
        raise_world(&mut run.dungeon),
        "migrate an existing travel plate"
    );
    assert_eq!(run.dungeon.rooms[gate].tile(c, r), Tile::Stairs);
    assert!(!raise_world(&mut run.dungeon), "migration is idempotent");
    // Publishing an old topology before migration must not retain the same
    // whole-mirror key. A stale live frame is rejected until a whole arrives.
    run.dungeon
        .rooms
        .retain(|r| !matches!(r.kind, RoomKind::Causeway | RoomKind::Priory));
    assert!(raise_world(&mut run.dungeon));
    let mine = room_of(&run.dungeon, RoomKind::MineHead).unwrap();
    for y in 1..4 {for x in 11..14 {run.dungeon.rooms[mine].set(x,y,Tile::Block);}}
    assert!(raise_world(&mut run.dungeon), "old mine spoil pile clears for the mummers road");
    for y in 1..4 {for x in 11..14 {
        let expected=if entrance_at(RoomKind::MineHead,x,y).is_some(){Tile::Stairs}else{Tile::Floor};
        assert_eq!(run.dungeon.rooms[mine].tile(x,y),expected);
    }}
    assert!(!raise_world(&mut run.dungeon));
    let stale: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(stale.valid_snapshot());
    run.arrive(
        room_of(&run.dungeon, RoomKind::Causeway).unwrap(),
        (3.0, 7.0),
    );
    assert_eq!(run.wire_copy().room().kind, RoomKind::Causeway);
}

#[test]
fn merchant_camps_are_seeded_bounded_safe_and_do_not_rewrite_the_estate() {
    use super::super::merchant::{CART_AT, PLATES, camp};
    let base = super::super::layout::undercroft(Pack::Crypt);
    let estate = serde_json::to_value(&base).unwrap();
    let mut locations = std::collections::BTreeSet::new();
    let mut catalogs = std::collections::BTreeSet::new();
    for seed in 0..48 {
        let mut floor = base.clone();
        raise_world(&mut floor);
        let before = serde_json::to_value(&floor).unwrap();
        let shop = camp(seed, 1, &floor).unwrap();
        assert_eq!(shop, camp(seed, 1, &floor).unwrap());
        assert!((2..=3).contains(&shop.offers.len()));
        let room = &floor.rooms[shop.room];
        assert!(matches!(room.kind, RoomKind::Gate | RoomKind::MineHead));
        assert!(room.cleared);
        locations.insert(format!("{:?}", room.kind));
        catalogs.insert(format!("{:?}", shop.offers));
        for &(c, r) in &PLATES {
            assert_eq!(room.tile(c, r), Tile::Floor);
            assert!(entrance_at(room.kind, c, r).is_none());
        }
        assert_eq!(room.tile(CART_AT.0 as i32, CART_AT.1 as i32), Tile::Floor);
        assert_eq!(
            serde_json::to_value(&floor).unwrap(),
            before,
            "camp is data, not geometry"
        );
        assert_eq!(
            serde_json::to_value(below_the_world(&floor)).unwrap(),
            estate
        );
    }
    assert_eq!(locations.len(), 2, "seeded surface variation");
    assert!(catalogs.len() > 3, "seeded prices and offers");
    assert!(camp(7, 1, &base).is_none(), "no camp in an unraised estate");
    // Every authored surface trail is reciprocal, and legacy occupied cells
    // remain untouched rather than being silently excavated for a new road.
    let mut raised = base.clone();
    raise_world(&mut raised);
    for &(a, b) in TRAILS {
        let ai = room_of(&raised, a).unwrap();
        let bi = room_of(&raised, b).unwrap();
        let ac = raised.rooms[ai].cell;
        let bc = raised.rooms[bi].cell;
        let dir = layout::DIRS
            .iter()
            .position(|&(x, y)| (ac.0 + x, ac.1 + y) == bc)
            .unwrap();
        assert!(raised.rooms[ai].doors[dir] && raised.rooms[bi].doors[(dir + 2) % 4]);
    }
    let mut occupied = base.clone();
    let blocker = Room::solid_rock((1, -7));
    let snapshot = serde_json::to_value(&blocker).unwrap();
    occupied.rooms.push(blocker);
    assert!(raise_world(&mut occupied));
    assert!(room_of(&occupied, RoomKind::Village).is_none());
    assert_eq!(serde_json::to_value(&occupied.rooms[4]).unwrap(), snapshot);
    let stables = room_of(&occupied, RoomKind::Stables).unwrap();
    assert!(!occupied.rooms[stables].doors[2]);
    assert!(!raise_world(&mut occupied));
}

#[test]
fn merchant_seed_stream_leaves_combat_rng_alone_and_never_uses_combat_rooms() {
    use super::super::merchant::camp;
    for seed in 0..12 {
        let mut run = Run::new(seed, 1, None);
        let mut untouched = run.clone();
        for _ in 0..40 {
            let _ = run.merchant_camp();
        }
        assert_eq!(
            run.rng.next(),
            untouched.rng.next(),
            "merchant has a private RNG stream"
        );
        let fight = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight)
            .unwrap();
        run.dungeon.rooms[fight].cleared = true;
        let shop = run.merchant_camp().unwrap();
        assert_eq!(run.dungeon.rooms[shop.room].kind, RoomKind::Treasure);
        assert_ne!(shop.room, fight);
        run.dungeon.rooms.retain(|r| r.kind != RoomKind::Treasure);
        assert!(
            run.merchant_camp().is_none(),
            "no fallback to cleared combat rooms"
        );
        run.descend_for_test();
        let deep = camp(seed, 1, &run.dungeon).unwrap();
        assert!(matches!(
            run.dungeon.rooms[deep.room].kind,
            RoomKind::Treasure | RoomKind::Sanctuary
        ));
        run.dungeon.rooms.retain(|r| r.kind != RoomKind::Treasure);
        let sanctuary = run
            .merchant_camp()
            .expect("deep initial sanctuary fallback");
        assert_eq!(sanctuary.room, 0);
        assert_eq!(run.dungeon.rooms[0].kind, RoomKind::Sanctuary);
        assert_eq!(run.merchant_camp(), Some(sanctuary));
    }
}

// Exercised by the pre-existing host/mirror/world regression, not a new
// standalone verifier with invented score assignments.
fn western_circuit_regression(mut run: Run) {
    use super::super::{countryside, pastimes};
    use crate::realm::Spoil;
    use pastimes::Game;
    let place = |run: &mut Run, id, at: (f32, f32)| {
        let h = run.players.get_mut(&id).unwrap();
        (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
    };
    let empty = BTreeMap::new();
    let throw = BTreeMap::from([
        (
            1,
            Input {
                aim_x: -1,
                ..Input::default()
            },
        ),
        (
            2,
            Input {
                fire: true,
                aim_x: 1,
                ..Input::default()
            },
        ),
    ]);
    let downs = room_of(&run.dungeon, RoomKind::Downs).unwrap();
    run.arrive(downs, Game::Cheese.start());
    place(&mut run, 1, (12.0, 11.0));
    place(&mut run, 2, Game::Cheese.start());
    run.tick_pastimes(&empty);
    assert_eq!(run.home.countryside.games.cheese.plays, 1);
    let target = pastimes::cheese_target(run.pastimes.round.as_ref().unwrap());
    place(&mut run, 2, target);
    run.tick_pastimes(&throw);
    assert_eq!(
        run.pastimes.round.as_ref().unwrap().progress,
        0,
        "feet at target never score"
    );
    assert_eq!(
        run.pastimes.round.as_ref().unwrap().shots,
        0,
        "throw only from tee"
    );
    run.tick_pastimes(&empty);
    place(&mut run, 2, Game::Cheese.start());
    run.tick_pastimes(&throw);
    assert!(
        run.pastimes
            .round
            .as_ref()
            .unwrap()
            .cheese
            .as_ref()
            .unwrap()
            .dx
            > 0,
        "friend's own aim"
    );
    let restored: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(restored.valid_snapshot());
    assert_eq!(restored.pastimes, run.pastimes);
    let guest = run.wire_copy();
    assert_eq!(guest.pastimes, run.pastimes);
    for _ in 0..80 {
        run.tick_pastimes(&throw);
    }
    assert_eq!(
        run.pastimes.round.as_ref().unwrap().shots,
        1,
        "holding F cannot auto-fire"
    );
    for _ in 0..650 {
        let Some(round) = run.pastimes.round.as_ref() else {
            break;
        };
        if round.cheese.is_none() {
            let mut future = round.clone();
            let travel = ((pastimes::cheese_target(round).0 - 4.0) * 256.0 / 108.0).ceil() as u32;
            future.left = future.left.saturating_sub(travel + 1);
            let target = pastimes::cheese_target(&future);
            if (4.85..7.15).contains(&target.1) {
                place(&mut run, 2, (4.0, target.1));
                run.tick_pastimes(&empty);
                run.tick_pastimes(&throw);
            } else {
                run.tick_pastimes(&empty);
            }
        } else {
            run.tick_pastimes(&empty);
        }
        assert!(run.pastimes.valid());
    }
    assert!(run.pastimes.round.is_none(), "real throws finish a course");
    assert_eq!(run.home.countryside.games.cheese.wins, 1);
    assert!(run.home.countryside.games.cheese.best > 0);
    assert_eq!(
        run.home.countryside.games.wins[0], 1,
        "old score arrays unchanged"
    );
    let records = run.home.countryside.games.clone();
    let mut saved = pastimes::Records::default();
    assert!(saved.merge(&records));
    assert!(!saved.merge(&records), "retry is idempotent");
    assert_eq!(saved, records);
    let old: countryside::Country = serde_json::from_value(serde_json::json!({
        "visited":63,"built":7,"ramble_paid":true,
        "games":{"best":[100,200,300,400],"wins":[1,1,1,1],"plays":[2,2,2,2]}
    }))
    .unwrap();
    assert!(old.valid());
    assert_eq!(old.games.best, [100, 200, 300, 400]);
    assert_eq!(old.games.cheese.plays, 0);
    assert_eq!(old.west_visited, 0);
    assert_eq!(old.survey, 0);
    let mut bad = restored.pastimes.clone();
    bad.round.as_mut().unwrap().cheese.as_mut().unwrap().dx = i32::MAX;
    assert!(!bad.valid());
    bad = restored.pastimes.clone();
    bad.round.as_mut().unwrap().shots = 0;
    assert!(!bad.valid());
    run.arrive(downs, Game::Cheese.start());
    place(&mut run, 1, (12.0, 11.0));
    place(&mut run, 2, Game::Cheese.start());
    run.tick_pastimes(&empty);
    run.tick_pastimes(&BTreeMap::from([(
        2,
        Input {
            fire: true,
            aim_x: 2,
            ..Input::default()
        },
    )]));
    assert_eq!(run.pastimes.round.as_ref().unwrap().shots, 0);
    for _ in 0..12 {
        run.tick_pastimes(&empty);
        run.tick_pastimes(&BTreeMap::from([(
            2,
            Input {
                fire: true,
                aim_x: -1,
                ..Input::default()
            },
        )]));
        for _ in 0..10 {
            run.tick_pastimes(&empty);
        }
    }
    assert!(run.pastimes.round.is_none(), "12 missed wheels end a round");
    assert_eq!(run.home.countryside.games.cheese.wins, 1);
    run.arrive(downs, Game::Cheese.start());
    place(&mut run, 1, (12.0, 11.0));
    place(&mut run, 2, Game::Cheese.start());
    run.tick_pastimes(&empty);
    run.pastimes.round.as_mut().unwrap().left = 1;
    run.tick_pastimes(&throw);
    assert!(run.pastimes.round.is_none(), "timeout precedes launching");

    // Survey uses real co-op E inputs; marks settle only in the host's save.
    let fire = BTreeMap::from([(
        2,
        Input {
            interact: true,
            ..Input::default()
        },
    )]);
    let request = BTreeMap::from([("country:build:lift".into(), 2)]);
    let mut country = old;
    let mut purse = crate::realm::Spoils::default();
    purse.add(Spoil::Gold, 200);
    purse.add(Spoil::Bone, 10);
    purse.add(Spoil::Ore, 5);
    let before = purse.clone();
    assert!(!countryside::settle(&mut country, &mut purse, &request).0);
    assert_eq!(purse, before);
    run.home.countryside = country.clone();
    run.marks.clear();
    assert!(!open(entrance("windmill-lift"), &run.home));
    run.join("Third");
    run.join("Fourth");
    for (i, &(kind, at)) in countryside::SURVEY.iter().enumerate() {
        run.arrive(room_of(&run.dungeon, kind).unwrap(), (12.0, 6.0));
        place(&mut run, 2, at);
        run.tick_countryside(&empty);
        assert!(!run.marks.contains_key(&format!("country:survey:{i}")));
        run.players.get_mut(&2).unwrap().privy = 1;
        run.tick_countryside(&fire);
        assert!(!run.marks.contains_key(&format!("country:survey:{i}")));
        run.players.get_mut(&2).unwrap().privy = 0;
        run.tick_countryside(&BTreeMap::from([(
            2,
            Input {
                interact: true,
                aim_x: 2,
                ..Input::default()
            },
        )]));
        assert!(!run.marks.contains_key(&format!("country:survey:{i}")));
        run.tick_countryside(&fire);
        assert_eq!(run.marks.get(&format!("country:survey:{i}")), Some(&1));
        assert!(countryside::settle(&mut country, &mut purse, &run.marks).0);
        run.home.countryside = country.clone();
        run.marks.clear();
        if i < 2 {
            assert!(!country.survey_paid);
        }
    }
    assert!(country.survey_paid);
    assert_eq!(country.west_visited, 3);
    assert_eq!(
        country.visited, 63,
        "west visits don't alter old milestones"
    );
    assert_eq!(purse.get(Spoil::Gold), before.get(Spoil::Gold) + 45);
    assert_eq!(purse.get(Spoil::Ore), before.get(Spoil::Ore) + 2);
    let replay = BTreeMap::from_iter((0..3).map(|i| (format!("country:survey:{i}"), 1)));
    let paid = purse.clone();
    assert!(!countryside::settle(&mut country, &mut purse, &replay).0);
    assert_eq!(purse, paid);
    assert!(countryside::settle(&mut country, &mut purse, &request).0);
    assert!(country.has(4));
    assert_eq!(purse.get(Spoil::Gold), paid.get(Spoil::Gold) - 70);
    assert_eq!(purse.get(Spoil::Bone), paid.get(Spoil::Bone) - 3);
    let paid = purse.clone();
    assert!(!countryside::settle(&mut country, &mut purse, &request).0);
    assert_eq!(purse, paid);
    run.home.countryside = country;
    place(&mut run, 2, countryside::PLOTS[4].at);
    for _ in 0..super::super::home::BUY_HOLD {
        run.tick_countryside(&fire);
    }
    assert_eq!(
        run.marks.get("country:build:lift"),
        None,
        "a plot already built takes no new order"
    );
    take(&mut run, entrance("cliffs-lift"));
    assert_eq!(run.room().kind, RoomKind::Windmill);
    for h in run.players.values() {
        assert!(walkable(run.room(), h.x, h.y));
    }
    take(&mut run, entrance("windmill-lift"));
    assert_eq!(run.room().kind, RoomKind::Cliffs);
    for h in run.players.values() {
        assert!(walkable(run.room(), h.x, h.y));
    }
    let restored: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(restored.valid_snapshot());
    assert!(open(entrance("cliffs-lift"), &restored.home));
    let guest = run.wire_copy();
    assert_eq!(guest.home.countryside, run.home.countryside);
}

include!("together_world__east.rs");
include!("together_world__north.rs");

include!("together_world__coast.rs");

include!("together_world__woodland.rs");

include!("together_world__wetland.rs");

include!("together_world__aerie.rs");

include!("together_world__mummers.rs");

include!("together_world__hippodrome.rs");

include!("together_world__windcountry.rs");

include!("together_world__roads.rs");
include!("together_world__broadsheet.rs");
include!("together_world__surf_cue.rs");
include!("together_world__washtub.rs");

// Exercises the original festival through the same live-input entry as travel.
// In particular, a tap must be judged against the cue in the published round,
// not the phase that will be drawn after that tap's tick has completed.
fn original_turnip_cue_regression() {
    use super::super::pastimes::Game;
    let empty = BTreeMap::new();
    let fire = |ids: &[u32]| {
        ids.iter()
            .map(|&id| {
                (
                    id,
                    Input {
                        fire: true,
                        ..Input::default()
                    },
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    let fresh = || {
        let mut run = at_the_gate();
        run.arrive(
            room_of(&run.dungeon, RoomKind::Fairground).unwrap(),
            Game::Turnip.start(),
        );
        // A friend can keep playing while the host is a warded statue.
        run.players.get_mut(&1).unwrap().stone = true;
        let (x, y) = Game::Turnip.start();
        let friend = run.players.get_mut(&2).unwrap();
        (friend.x, friend.y) = (x * TILE_UNITS, y * TILE_UNITS);
        run.step(&empty);
        assert_eq!(
            run.pastimes.round.as_ref().unwrap().left,
            Game::Turnip.time()
        );
        run
    };

    // All displayed phases, including the last gold frame and the red frame
    // immediately before the next beat. No injected round clock is used.
    for phase in 0..HZ {
        let mut run = fresh();
        for _ in 0..phase {
            run.step(&empty);
        }
        let wire = serde_json::to_vec(&run).unwrap();
        let mut restored: Run = serde_json::from_slice(&wire).unwrap();
        assert!(restored.valid_snapshot());
        let treasury = restored.treasury.clone();
        restored.step(&fire(&[2]));
        let round = restored.pastimes.round.as_ref().unwrap();
        let gold = phase <= HZ / 5;
        assert_eq!(
            (round.progress, round.mistakes),
            (u8::from(gold), u8::from(!gold)),
            "displayed Turnip phase {phase} must judge the friend's restored tap"
        );
        assert_eq!(
            round.last_beat,
            Some(0),
            "phase {phase} belongs to the published beat"
        );
        let counts = (round.progress, round.mistakes);
        let mirror = restored.wire_copy();
        assert_eq!(mirror.pastimes, restored.pastimes);
        // Holding F through another beat must not turn into another tap.
        for _ in 0..HZ + 1 {
            restored.step(&fire(&[2]));
        }
        let round = restored.pastimes.round.as_ref().unwrap();
        assert_eq!((round.progress, round.mistakes), counts);
        assert_eq!(restored.treasury, treasury);
    }

    let mut run = fresh();
    run.join_seat(3, "Turnip correspondent");
    let (x, y) = Game::Turnip.start();
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (x * TILE_UNITS, y * TILE_UNITS);
    let treasury = run.treasury.clone();
    let score = run.score;
    let together = fire(&[2, 3]);
    for beat in 0..u32::from(Game::Turnip.need()) {
        let landing = beat * HZ + HZ / 5;
        while Game::Turnip.time() - run.pastimes.round.as_ref().unwrap().left < landing {
            run.step(&empty);
        }
        if beat == 3 {
            run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
            assert!(run.valid_snapshot());
        }
        run.step(&together);
        if beat + 1 < u32::from(Game::Turnip.need()) {
            let round = run.pastimes.round.as_ref().unwrap();
            assert_eq!((round.progress, round.mistakes), ((beat + 1) as u8, 0));
            // Simultaneous friends and continued F each yield only one kick.
            for _ in 0..3 {
                run.step(&together);
            }
            assert_eq!(
                run.pastimes.round.as_ref().unwrap().progress,
                (beat + 1) as u8
            );
        }
    }
    assert!(run.pastimes.round.is_none());
    let index = Game::Turnip.index();
    assert_eq!(
        (
            run.home.countryside.games.plays[index],
            run.home.countryside.games.wins[index]
        ),
        (1, 1)
    );
    assert!((25..=1000).contains(&run.home.countryside.games.best[index]));
    assert!(run.score > score);
    let paid_score = run.score;
    for _ in 0..HZ {
        run.step(&together);
    }
    assert!(
        run.pastimes.round.is_none(),
        "standing on T cannot automatically restart"
    );
    assert_eq!(run.score, paid_score);
    assert_eq!(run.treasury, treasury, "practice wins never mint gold");

    // Walk off and back to retry with ordinary controls, rather than clearing
    // the pad latch or teleporting a round into existence.
    let moving = |direction| {
        BTreeMap::from_iter([2, 3].map(|id| {
            (
                id,
                Input {
                    move_x: direction,
                    ..Input::default()
                },
            )
        }))
    };
    for _ in 0..HZ {
        run.step(&moving(-1));
    }
    assert!(run.pastimes.round.is_none());
    for _ in 0..HZ {
        run.step(&moving(1));
        if run.pastimes.round.is_some() {
            break;
        }
    }
    assert!(
        run.pastimes.round.is_some(),
        "ordinary walking must rearm T"
    );
    assert_eq!(run.home.countryside.games.plays[index], 2);
    while run.pastimes.round.as_ref().unwrap().left > 1 {
        run.step(&empty);
    }
    run.step(&together);
    assert!(
        run.pastimes.round.is_none(),
        "expiry precedes even a last-tick tap"
    );
    assert_eq!(run.home.countryside.games.wins[index], 1);
    assert_eq!(run.score, paid_score);
    assert_eq!(run.treasury, treasury);

    let mut run = fresh();
    run.players.get_mut(&2).unwrap().stone = true;
    run.step(&fire(&[2]));
    assert!(
        run.pastimes.round.is_none(),
        "no eligible contributors ends the round"
    );
}

#[test]
fn holding_e_on_a_plot_already_built_files_no_order() {
    use crate::rules::{Input, countryside, home::BUY_HOLD};
    let fire = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    let plot = &countryside::PLOTS[0];
    for built in [false, true] {
        let mut run = at_the_gate();
        if built {
            run.home.countryside.built |= 1;
        }
        run.arrive(room_of(&run.dungeon, plot.room).unwrap(), plot.at);
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (plot.at.0 * TILE_UNITS, plot.at.1 * TILE_UNITS);
        run.players.remove(&2);
        for _ in 0..BUY_HOLD + 2 {
            run.step(&fire);
        }
        let order = format!("country:build:{}", plot.id);
        assert_eq!(run.marks.contains_key(&order), !built, "built: {built}");
    }
}

#[test]
fn one_bad_countryside_field_is_mended_and_the_rest_kept() {
    use super::super::countryside::{self, Country};
    // A stage past its end, and a field of the wrong kind: each goes alone.
    let home: Home = serde_json::from_value(serde_json::json!({"countryside": {
        "visited": 63, "relics": 7, "built": 7, "ramble_paid": true, "relics_paid": true,
        "signal": 9, "lamps": "lots", "survey": 5
    }}))
    .unwrap();
    let country = &home.countryside;
    assert!(country.valid());
    assert_eq!((country.visited, country.relics, country.built), (63, 7, 7));
    assert!(country.ramble_paid && country.relics_paid);
    assert_eq!(country.survey, 5, "a good field stays as it was");
    assert_eq!(country.signal, 0, "the bad stage starts again");
    assert_eq!(country.lamps, 0, "the unreadable field is left at its default");
    // Built and paid progress vouches for the job behind it.
    let built: Country = countryside::load(serde_json::json!({
        "built": 1 << 4, "survey": 3, "harbour_paid": true, "harbour": 1, "minutes": 5
    }));
    assert!(built.valid());
    assert!(built.has(4));
    assert_eq!(built.survey, 7);
    assert_eq!(built.harbour, 3);
    assert_eq!((built.post, built.minutes), (1, 5));
    // Whatever the numbers, a load always comes back playable.
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut roll = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let fields = serde_json::to_value(Country::default()).unwrap();
    for _ in 0..400 {
        let mut save = serde_json::Map::new();
        for (key, value) in fields.as_object().unwrap() {
            let wild = match value {
                serde_json::Value::Bool(_) => serde_json::json!(roll(2) == 0),
                serde_json::Value::Number(_) => {
                    let top = if roll(3) == 0 { 300 } else { 9 };
                    serde_json::json!(roll(top))
                }
                _ => continue,
            };
            if roll(3) == 0 {
                save.insert(key.clone(), wild);
            }
        }
        let save = serde_json::Value::Object(save);
        assert!(countryside::load(save.clone()).valid(), "{save}");
        // Mending alone makes it whole: never the fresh start.
        if let Ok(mut country) = serde_json::from_value::<Country>(save.clone()) {
            country.repair();
            assert!(country.valid(), "{save}");
        }
    }
}

/// Humphrey the Risen is retired: however far the festival goes, no zombie
/// claws up above ground, and one left standing in an older save is cleared.
#[test]
fn no_zombies_rise_above_ground() {
    let mut run = at_the_gate();
    let fair = room_of(&run.dungeon, RoomKind::Fairground).unwrap();
    run.arrive(fair, (12.0, 7.0));
    run.home.countryside.games.plays[pastimes::Game::Barrel.index() as usize] = 9;
    run.home.countryside.games.wins[pastimes::Game::Barrel.index() as usize] = 3;
    for _ in 0..(super::super::risen::FOLLOW as usize + 4 * HZ as usize) {
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies.iter().all(|e| e.kind != EnemyKind::Risen), "nothing rises");
    let id = run.spawn_at(EnemyKind::Risen, 20.0, 10.0).expect("an old save's zombie");
    assert!(run.enemies.iter().any(|e| e.id == id));
    run.step(&BTreeMap::new());
    assert!(run.enemies.iter().all(|e| e.kind != EnemyKind::Risen), "cleared at once");
    assert!(run.valid_snapshot());
}

#[test]
fn a_running_arcade_round_blocks_surveys_lamps_and_plot_orders() {
    use super::super::{countryside, pastimes::Game};
    let mut run = at_the_gate();
    run.arrive(room_of(&run.dungeon, Game::Cheese.room()).unwrap(), Game::Cheese.start());
    run.tick_pastimes(&BTreeMap::new());
    assert!(run.pastimes.round.is_some());
    let round = run.pastimes.round.clone();
    let interact = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    let mut check = |kind, (x, y): (f32, f32), mark: &str| {
        run.arrive(room_of(&run.dungeon, kind).unwrap(), (x, y));
        run.pastimes.round = round.clone();
        run.marks.clear();
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
        for _ in 0..super::super::home::BUY_HOLD + 1 {
            run.tick_countryside(&interact);
        }
        assert!(!run.marks.contains_key(mark), "{mark} during the round");
        run.pastimes.round = None;
        run.home.countryside.relics = 7;
        run.home.countryside.built = 2;
        for _ in 0..super::super::home::BUY_HOLD + 1 {
            run.tick_countryside(&interact);
        }
        assert!(run.marks.contains_key(mark), "{mark} after the round");
    };
    let (kind, at) = countryside::SURVEY[1];
    check(kind, at, "country:survey:1");
    let lamp = &countryside::LAMPS[0];
    check(lamp.room, lamp.at, "country:lamp:0");
    let plot = &countryside::PLOTS[0];
    check(plot.room, plot.at, "country:build:bakery");
}
