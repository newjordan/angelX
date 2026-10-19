// Extend the established road regression with real movement, not arrive() at
// each destination. Planning never treats an entrance/shortcut as a footpath.
fn walk_ordinary_road(run: &mut Run, destination: RoomKind, driver: u32) {
    use std::collections::VecDeque;

    let origin = run.room().kind;
    let start_tick = run.tick;
    let old_at = run.at;
    let room = run.room();
    let next = room_of(&run.dungeon, destination).unwrap();
    let dir = DIRS.iter().position(|&(dx, dy)| {
        (room.cell.0 + dx, room.cell.1 + dy) == run.dungeon.rooms[next].cell
    }).expect("authored road joins adjacent rooms");
    assert_eq!(run.dungeon.neighbour(old_at, dir), Some(next));
    let hero = &run.players[&driver];
    let start = (hero.x as usize / 2, hero.y as usize / 2);
    let goal = match dir {
        0 => (room.cols / 2 - 1, 0),
        1 => (room.cols - 1, room.rows / 2 - 1),
        2 => (room.cols / 2 - 1, room.rows - 1),
        _ => (0, room.rows / 2 - 1),
    };
    let cell = |(x, y): (usize, usize)| y * room.cols + x;
    let centre = |k: usize| (
        (k % room.cols) as f32 * TILE_UNITS + TILE_UNITS / 2.0,
        (k / room.cols) as f32 * TILE_UNITS + TILE_UNITS / 2.0,
    );
    let grid = Grid { room, barred: false };
    let mut parents = vec![None; room.cols * room.rows];
    parents[cell(start)] = Some(cell(start));
    let mut queue = VecDeque::from([start]);
    while let Some((x, y)) = queue.pop_front() {
        let here = cell((x, y));
        if (x, y) == goal { break; }
        for (dx, dy) in DIRS {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= room.cols as i32 || ny >= room.rows as i32 {
                continue;
            }
            let candidate = cell((nx as usize, ny as usize));
            let (cx, cy) = centre(candidate);
            let (hx, hy) = centre(here);
            if parents[candidate].is_none()
                && matches!(room.tile(nx, ny), Tile::Floor | Tile::Door)
                && grid.clear(cx, cy, HERO_RADIUS, Mover::Hero)
                && grid.clear((cx + hx) / 2.0, (cy + hy) / 2.0, HERO_RADIUS, Mover::Hero)
            {
                parents[candidate] = Some(here);
                queue.push_back((nx as usize, ny as usize));
            }
        }
    }
    assert!(parents[cell(goal)].is_some(), "{origin:?} -> {destination:?}: no unpaid footpath from knight {driver}");
    let mut path = vec![cell(goal)];
    while *path.last().unwrap() != cell(start) {
        path.push(parents[*path.last().unwrap()].unwrap());
    }
    let targets: Vec<_> = path.into_iter().rev().map(centre).collect();
    // Every coordinate update below comes from the production input step.
    for (tx, ty) in targets {
        let mut reached = false;
        for _ in 0..2 * HZ {
            let h = &run.players[&driver];
            let (dx, dy) = (tx - h.x, ty - h.y);
            let tolerance = HERO_SPEED * DT * 0.6;
            let axis = |delta: f32| if delta.abs() <= tolerance { 0 } else if delta < 0.0 { -1 } else { 1 };
            let (move_x, move_y) = (axis(dx), axis(dy));
            if move_x == 0 && move_y == 0 { reached = true; break; }
            run.step(&BTreeMap::from([(driver, Input { move_x, move_y, ..Default::default() })]));
            assert_eq!(run.at, old_at, "unexpected exit while walking {origin:?} -> {destination:?}");
            assert_eq!(run.dungeon.depth, 0, "walking must not enter a mine shaft");
        }
        assert!(reached, "{origin:?} -> {destination:?}: knight {driver} stuck before {tx},{ty}");
    }
    let (dx, dy) = DIRS[dir];
    // A doorway takes the whole party (`fellowship`): the friends have walked
    // up beside the driver, and all of them go out through the road's mouth.
    let (gx, gy) = (run.players[&driver].x, run.players[&driver].y);
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (gx, gy);
    }
    let inputs: BTreeMap<u32, Input> = run.players.keys().map(|&id| (id, Input {
        move_x: dx as i8, move_y: dy as i8, ..Default::default()
    })).collect();
    for _ in 0..2 * HZ {
        if run.at != old_at { break; }
        run.step(&inputs);
    }
    assert_eq!(run.room().kind, destination, "{origin:?}: actual input did not cross road {dir}");
    assert_eq!(run.dungeon.depth, 0);
    assert!(!run.barred(), "surface roads must not start combat");
    for (&id, h) in &run.players {
        assert!(run.settlement_walkable_for_test(h.x, h.y), "{destination:?}: knight {id} obstructed in arrival fan {dir} at {},{}", h.x, h.y);
    }
    assert!(run.tick > start_tick);
}

fn walk_road_network(run: &mut Run, used: &mut [bool], crossings: &mut usize) {
    let origin = run.room().kind;
    for (edge, &(a, b)) in TRAILS.iter().enumerate() {
        let destination = if a == origin { b } else if b == origin { a } else { continue; };
        if used[edge] { continue; }
        used[edge] = true;
        walk_ordinary_road(run, destination, (*crossings % 4 + 1) as u32);
        *crossings += 1;
        walk_road_network(run, used, crossings);
        walk_ordinary_road(run, origin, (*crossings % 4 + 1) as u32);
        *crossings += 1;
    }
}

fn ordinary_road_journey_regression() {
    for migrated in [false, true] {
        let mut run = Run::at_home(7, 1, None, Home::default(), Default::default());
        if migrated {
            // Pre-expansion topology: retain the original six surface rooms,
            // their former obstacles, and all of the settlement below them.
            let original: Vec<_> = ROOMS[..6].iter().map(|&(kind, _)| kind).collect();
            run.dungeon.rooms.retain(|r| !r.kind.in_world() || original.contains(&r.kind));
            let cells: Vec<_> = run.dungeon.rooms.iter().map(|r| r.cell).collect();
            for room in &mut run.dungeon.rooms {
                if !room.kind.in_world() { continue; }
                for (dir, &(dx, dy)) in DIRS.iter().enumerate() {
                    if !cells.contains(&(room.cell.0 + dx, room.cell.1 + dy)) {
                        room.doors[dir] = false;
                        // Close only old boundary doors, not room scenery.
                        for y in 0..room.rows { for x in 0..room.cols {
                            if (x == 0 || y == 0 || x == room.cols - 1 || y == room.rows - 1)
                                && room.tile(x as i32, y as i32) == Tile::Door
                                && match dir { 0 => y == 0, 1 => x == room.cols - 1, 2 => y == room.rows - 1, _ => x == 0 }
                            { room.set(x, y, Tile::Wall); }
                        }}
                    }
                }
                let (xs, ys) = match room.kind {
                    RoomKind::Stables => (9..15, 1..5),
                    RoomKind::MineHead => (9..15, 1..4),
                    RoomKind::Lists => (9..15, 1..3),
                    _ => continue,
                };
                for y in ys { for x in xs.clone() {
                    if room.tile(x as i32, y as i32) != Tile::Stairs {
                        room.set(x, y, Tile::Block);
                    }
                } }
            }
            let cellar = serde_json::to_value(below_the_world(&run.dungeon)).unwrap();
            run.dungeon = serde_json::from_slice(&serde_json::to_vec(&run.dungeon).unwrap()).unwrap();
            assert!(raise_world(&mut run.dungeon));
            assert_eq!(serde_json::to_value(below_the_world(&run.dungeon)).unwrap(), cellar, "world migration must preserve the player's settlement");
            assert!(!raise_world(&mut run.dungeon), "road migration is idempotent");
        }
        for entrance in &ENTRANCES {
            let room = &run.dungeon.rooms[room_of(&run.dungeon, entrance.room).unwrap()];
            let (x, y, w, h) = entrance.at;
            for row in y..y + h { for col in x..x + w {
                assert_eq!(room.tile(col, row), Tile::Stairs, "{}: road clearance must preserve stairs", entrance.id);
            } }
        }
        run.come_to_the_gate();
        run.join_seat(2, "Sir Boots");
        run.join_seat(3, "Lady Sandwich");
        run.join_seat(4, "Goose Inspector");
        let treasury = run.treasury.clone();
        assert!(treasury.is_empty());
        assert_eq!(run.home.countryside.built, 0);
        let mut used = vec![false; TRAILS.len()];
        let mut crossings = 0;
        walk_road_network(&mut run, &mut used, &mut crossings);
        assert!(used.iter().all(|&edge| edge), "every authored free road is connected to the gate");
        assert_eq!(crossings, 2 * TRAILS.len());
        assert_eq!(run.room().kind, RoomKind::Gate);
        for &(kind, _) in &ROOMS {
            if kind == RoomKind::KingsHall { continue; } // paid great door, not a free ordinary road
            assert!(run.dungeon.rooms[room_of(&run.dungeon, kind).unwrap()].visited, "{kind:?}: unvisited surface place");
        }
        assert_eq!(run.treasury, treasury, "ordinary walking never charges or mints gold");
        assert_eq!(run.home.countryside.built, 0, "walking never buys a shortcut");
        assert!(run.valid_snapshot());
        let saved: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
        assert!(saved.valid_snapshot());
        assert_eq!(saved.room().kind, RoomKind::Gate);
        println!("unpaid road journey: migrated={migrated}, places={}, crossings={crossings}, ticks={}, four clear arrivals per crossing", ROOMS.len() - 1, run.tick);
    }
}
