//! The Wax Maze: a folly room where everyone eats wax and flees wraiths.
use super::*;

fn floor() -> Run {
    let mut run = Run::new(41, 1, None);
    run.join_seat(2, "Two");
    run.enemies.clear();
    run
}

#[test]
fn every_floor_below_has_a_folly_room_and_walking_in_starts_the_maze() {
    let mut run = floor();
    run.pick_folly();
    let index = run.folly_room.expect("a folly room on floor one");
    assert_eq!(run.dungeon.rooms[index].kind, RoomKind::Hall);
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 50);
    }
    run.enter_for_test(index);
    let maze = run.folly.as_ref().expect("the maze begins");
    assert!(maze.wax.len() > 40 && maze.wraiths.len() == 4);
    assert!(run.enemies.is_empty(), "no monsters in a folly");
    assert!(run.barred(), "the doors hold while it runs");
    assert_eq!(maze.ante.values().sum::<u32>(), 10, "each knight antes a tenth");
    assert!(run.valid_snapshot());
}

#[test]
fn the_most_wax_takes_the_pot_and_the_crown() {
    let mut run = floor();
    run.pick_folly();
    let index = run.folly_room.unwrap();
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 50);
    }
    run.enter_for_test(index);
    // Past the title card; then knight one walks the top corridor.
    for _ in 0..folly::TITLE {
        run.step(&BTreeMap::new());
    }
    let cols = run.room().cols as i32;
    for x in 2..cols - 2 {
        // Left alone by the wraiths for the walk.
        run.folly.as_mut().unwrap().spooked.insert(1, u32::MAX);
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = ((x as f32 + 0.5) * TILE_UNITS, 1.5 * TILE_UNITS);
        run.step(&BTreeMap::new());
    }
    let eaten = run.folly.as_ref().unwrap().eaten[&1];
    assert!(eaten >= 10, "wax eaten walking: {eaten}");
    // Burn the candle down.
    run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1;
    run.step(&BTreeMap::new());
    let maze = run.folly.as_ref().unwrap();
    assert!(maze.done);
    assert!(!run.barred(), "the doors open after the podium");
    assert!(run.players[&1].laurel, "most wax wears the crown");
    assert!(run.players[&1].carried.get(Spoil::Gold) > run.players[&2].carried.get(Spoil::Gold));
    assert!(run.found.as_ref().unwrap().2.contains("ATE THE MOST WAX"));
    assert!(run.valid_snapshot());
}

/// A maze under way past its title card, the wraiths all in their crypt
/// for a long rest, and knight `id` on the middle of lane tile `at`.
fn quiet_maze(run: &mut Run) {
    run.pick_folly();
    let index = run.folly_room.unwrap();
    run.enter_for_test(index);
    let maze = run.folly.as_mut().unwrap();
    maze.t = folly::TITLE;
    for w in &mut maze.wraiths {
        w.rest = folly::REST;
    }
}

fn mid(t: i32) -> f32 {
    (t as f32 + 0.5) * TILE_UNITS
}

fn lane(run: &Run, (x, y): (i32, i32)) -> bool {
    run.room().tile(x, y) == Tile::Floor
}

/// Wraith `k` set roaming on lane tile `at`, going nowhere yet.
fn loose(run: &mut Run, k: usize, at: (i32, i32)) {
    let w = &mut run.folly.as_mut().unwrap().wraiths[k];
    (w.at, w.to, w.came, w.step, w.rest, w.span) = (at, at, at, 0, 0, folly::WRAITH_STEP);
}

#[test]
fn the_maze_is_sound_in_every_party_room() {
    for knights in 1..=4usize {
        let (cols, rows) = layout::party_room(knights);
        let (cols, rows) = (cols as i32, rows as i32);
        for seed in 0..12u64 {
            let (open, crypt) = folly::lay_maze(cols, rows, seed * 7919 + knights as u64);
            let at = |x: i32, y: i32| (0..cols).contains(&x) && (0..rows).contains(&y) && open[(y * cols + x) as usize];
            let lanes: Vec<(i32, i32)> = (0..rows).flat_map(|y| (0..cols).map(move |x| (x, y))).filter(|&(x, y)| at(x, y)).collect();
            // The ring inside the walls, where every doorway comes in.
            for x in 1..cols - 1 {
                assert!(at(x, 1) && at(x, rows - 2), "{cols}x{rows} ring row at {x}");
            }
            for y in 1..rows - 1 {
                assert!(at(1, y) && at(cols - 2, y), "{cols}x{rows} ring column at {y}");
            }
            for &(x, y) in &lanes {
                assert!((1..cols - 1).contains(&x) && (1..rows - 1).contains(&y), "lanes inside the walls");
                let ways = [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().filter(|(dx, dy)| at(x + dx, y + dy)).count();
                assert!(ways >= 2, "{cols}x{rows} seed {seed}: a dead end at {x},{y}");
                // One tile wide: never a two by two of open floor.
                assert!(!(at(x + 1, y) && at(x, y + 1) && at(x + 1, y + 1)), "{cols}x{rows}: a wide patch at {x},{y}");
                // The same left and right.
                assert!(at(cols - 1 - x, y), "{cols}x{rows}: not mirrored at {x},{y}");
            }
            // All one maze.
            let mut seen = BTreeSet::from([lanes[0]]);
            let mut queue = vec![lanes[0]];
            while let Some((x, y)) = queue.pop() {
                for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                    if at(x + dx, y + dy) && seen.insert((x + dx, y + dy)) {
                        queue.push((x + dx, y + dy));
                    }
                }
            }
            assert_eq!(seen.len(), lanes.len(), "{cols}x{rows} seed {seed}: every lane reachable");
            // The crypt: stone in the middle, lanes all round it.
            let (cx, cy, cw, ch) = crypt;
            assert!(cw >= 2 && ch >= 2);
            assert_eq!(cx * 2 + cw, cols, "the crypt sits in the middle");
            for x in cx..cx + cw {
                assert!(at(x, cy - 1) && at(x, cy + ch) && (cy..cy + ch).all(|y| !at(x, y)));
            }
            for y in cy..cy + ch {
                assert!(at(cx - 1, y) && at(cx + cw, y));
            }
        }
    }
}

#[test]
fn the_maze_grows_with_the_party_and_its_wax_is_all_within_reach() {
    for knights in 1..=4u32 {
        let mut run = Run::new(41, 1, None);
        for id in 2..=knights {
            run.join_seat(id, "Friend");
        }
        run.enemies.clear();
        run.pick_folly();
        let index = run.folly_room.unwrap();
        run.enter_for_test(index);
        let maze = run.folly.clone().unwrap();
        let (cols, rows) = layout::party_room(knights as usize);
        assert_eq!((run.room().cols, run.room().rows), (cols, rows), "{knights} knights");
        let cols = cols as i32;
        let tiles: Vec<(i32, i32)> = maze.wax.iter().chain(maze.holy.iter().copied()).map(|i| (i as i32 % cols, i as i32 / cols)).collect();
        assert_eq!(maze.holy.len(), 4);
        for hero in run.players.values() {
            let from = ((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
            assert!(lane(&run, from), "{knights} knights: each comes in on a lane");
            let mut seen = BTreeSet::from([from]);
            let mut queue = vec![from];
            while let Some((x, y)) = queue.pop() {
                for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                    if lane(&run, (x + dx, y + dy)) && seen.insert((x + dx, y + dy)) {
                        queue.push((x + dx, y + dy));
                    }
                }
            }
            assert!(tiles.iter().all(|t| seen.contains(t)), "{knights} knights: all the wax within reach");
        }
        assert!(run.valid_snapshot());
    }
}

#[test]
fn a_knight_glides_round_corners_pressed_early_or_late() {
    let mut run = floor();
    run.players.retain(|&id, _| id == 1);
    quiet_maze(&mut run);
    // A crossing with an opening up, two lane tiles to its west with stone
    // above them.
    let (cols, rows) = (run.room().cols as i32, run.room().rows as i32);
    let (cx, cy) = (2..cols - 2)
        .flat_map(|x| (2..rows - 1).map(move |y| (x, y)))
        .find(|&(x, y)| {
            lane(&run, (x, y))
                && lane(&run, (x, y - 1))
                && lane(&run, (x - 1, y))
                && lane(&run, (x - 2, y))
                && lane(&run, (x - 3, y))
                && !lane(&run, (x - 1, y - 1))
                && !lane(&run, (x - 2, y - 1))
                && !lane(&run, (x - 3, y - 1))
        })
        .expect("a crossing to turn at");
    let key = |x: i8, y: i8| BTreeMap::from([(1, Input { move_x: x, move_y: y, ..Default::default() })]);
    let place = |run: &mut Run, x: f32| {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x, mid(cy));
        run.folly.as_mut().unwrap().ways.clear();
    };
    // Up pressed early: on east until the opening, then up it, never
    // stopping against the stone.
    place(&mut run, mid(cx - 3));
    run.step(&key(1, 0));
    let mut was = (run.players[&1].x, run.players[&1].y);
    for _ in 0..40 {
        run.step(&key(0, -1));
        let h = &run.players[&1];
        assert_ne!((h.x, h.y), was, "stopped against the stone");
        was = (h.x, h.y);
        if h.y < mid(cy) - 1.0 {
            break;
        }
    }
    let h = &run.players[&1];
    assert!((h.x - mid(cx)).abs() < 0.01 && h.y < mid(cy) - 1.0, "turned up at the opening: {},{}", h.x, h.y);
    // Pressed a little late, past the opening's middle: still taken.
    place(&mut run, mid(cx) + 0.7);
    run.step(&key(0, -1));
    run.step(&key(0, -1));
    run.step(&key(0, -1));
    let h = &run.players[&1];
    assert!(h.y < mid(cy) - 0.5 && (h.x - mid(cx)).abs() < 0.3, "a late turn is cut round the corner: {},{}", h.x, h.y);
    // Standing under stone, up does nothing; a lane's middle is kept.
    place(&mut run, mid(cx - 2));
    run.step(&key(0, -1));
    let h = &run.players[&1];
    assert_eq!((h.x, h.y), (mid(cx - 2), mid(cy)), "no way up there");
    // A wall ahead stops a knight in the middle of the last tile.
    let east = (cx..cols).take_while(|&x| lane(&run, (x, cy))).last().unwrap();
    place(&mut run, mid(east - 1));
    for _ in 0..20 {
        run.step(&key(1, 0));
    }
    assert_eq!(run.players[&1].x, mid(east), "stopped in the middle of the last tile");
    // Facing the way they go.
    assert!(run.players[&1].aim_x > 0.9);
    assert!(run.valid_snapshot());
}

#[test]
fn nobody_moves_under_the_title_card() {
    let mut run = floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    let before = (run.players[&1].x, run.players[&1].y);
    let right = BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]);
    for _ in 0..folly::TITLE - 2 {
        run.step(&right);
    }
    assert_eq!((run.players[&1].x, run.players[&1].y), before);
    for _ in 0..10 {
        run.step(&right);
    }
    assert!(run.players[&1].x > before.0, "and off they go");
}

#[test]
fn each_temper_hunts_its_own_way() {
    use folly::{Temper, aim};
    let size = (34, 20);
    let knight = ((10, 9), (1, 0));
    // The Hound: the knight's own tile.
    assert_eq!(aim(Temper::Hound, (20, 9), size, Some(knight), None), (10, 9));
    // The Cutter: ahead of them.
    assert_eq!(aim(Temper::Cutter, (20, 9), size, Some(knight), None), (10 + folly::CUT_AHEAD, 9));
    // The Flanker: from the far side of the knight, the Hound's line doubled.
    let flank = aim(Temper::Flanker, (3, 3), size, Some(knight), Some(((10, 15), knight)));
    assert_eq!(flank, (2 * (10 + folly::FLANK_AHEAD) - 10, 2 * 9 - 15));
    assert!(flank.1 < 9, "comes at them from above when the Hound is below");
    // The Drifter: hunts from afar, goes off to its corner when near.
    assert_eq!(aim(Temper::Drifter, (30, 18), size, Some(knight), None), (10, 9));
    assert_eq!(aim(Temper::Drifter, (12, 10), size, Some(knight), None), (-2, 21));
    // Keeping to their corners, four different ones.
    let corners: BTreeSet<(i32, i32)> = Temper::ALL.iter().map(|&t| aim(t, (5, 5), size, None, None)).collect();
    assert_eq!(corners.len(), 4);
}

#[test]
fn the_wraiths_keep_their_lanes_and_their_distance() {
    // Whole mazes played by the harness's thumbs, every party size.
    use super::sim::{Thumb, play};
    for (knights, thumbs) in [
        (1, &[Thumb::Careless][..]),
        (2, &[Thumb::Human, Thumb::Rowdy]),
        (3, &[Thumb::Bot, Thumb::Idle, Thumb::Careless]),
        (4, &[Thumb::Careless, Thumb::Human, Thumb::Rowdy, Thumb::Bot]),
    ] {
        let r = play(knights, 41, thumbs, |_, _| {});
        let line = r.line(knights, thumbs);
        assert_eq!((r.unreachable_wax, r.unreachable_floor), (0, 0), "{line}");
        assert_eq!(r.knight_in_stone, 0, "{line}");
        assert_eq!(r.wraith_in_stone, 0, "{line}");
        assert_eq!(r.wraith_stuck, 0, "{line}");
        assert_eq!(r.wraith_stacked, 0, "{line}");
        assert!(r.snags.is_empty(), "{line}");
        assert_eq!(r.fallen, 0, "{line}");
        assert!(r.ended_at.is_some() && r.found.contains("ATE THE MOST WAX"), "{line}");
    }
}

#[test]
fn a_wraith_spills_wax_and_a_pale_one_is_caught() {
    let mut run = floor();
    run.players.retain(|&id, _| id == 1);
    quiet_maze(&mut run);
    run.folly.as_mut().unwrap().eaten.insert(1, 12);
    let start = run.folly.as_ref().unwrap().starts[&1];
    // A calm wraith on the knight, out on the ring (its wax eaten).
    let spot = (3, 1);
    let cols = run.room().cols as i32;
    run.folly.as_mut().unwrap().wax.remove(&((spot.1 * cols + spot.0) as u16));
    loose(&mut run, 0, spot);
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (mid(spot.0), mid(spot.1));
    }
    run.step(&BTreeMap::new());
    let maze = run.folly.as_ref().unwrap();
    assert_eq!(maze.eaten[&1], 12 - folly::SPILL, "a touch spills wax");
    let h = &run.players[&1];
    assert_eq!(((h.x * 16.0) as i32, (h.y * 16.0) as i32), start, "and sends them back where they came in");
    // Left alone there: a wraith on them now touches nothing.
    let here = ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32);
    run.folly.as_mut().unwrap().wax.remove(&((here.1 * cols + here.0) as u16));
    loose(&mut run, 1, here);
    for _ in 0..folly::SPOOKED - 2 {
        run.step(&BTreeMap::new());
        let w = &mut run.folly.as_mut().unwrap().wraiths[1];
        (w.at, w.to, w.step) = (here, here, 0);
    }
    assert_eq!(run.folly.as_ref().unwrap().eaten[&1], 12 - folly::SPILL, "no touch while spooked");
    // Pale, the same touch catches the wraith instead.
    run.folly.as_mut().unwrap().spooked.clear();
    {
        let maze = run.folly.as_mut().unwrap();
        maze.pale = folly::PALE;
        maze.wraiths[1].pale = true;
    }
    let bone = run.players[&1].carried.get(Spoil::Bone);
    run.step(&BTreeMap::new());
    let maze = run.folly.as_ref().unwrap();
    assert!(run.players[&1].carried.get(Spoil::Bone) > bone, "a caught wraith banks a bone");
    assert_eq!(maze.eaten[&1], 12 - folly::SPILL + folly::CATCH[0]);
    assert!(maze.wraiths[1].eyes, "and its eyes go home");
}

#[test]
fn a_touch_is_where_their_shapes_meet() {
    let mut run = floor();
    run.players.retain(|&id, _| id == 1);
    quiet_maze(&mut run);
    // On the ring: a wraith on one tile, the knight a tile along, then half.
    loose(&mut run, 0, (6, 1));
    for (x, touched) in [(mid(7), false), (mid(6) + 1.6, false), (mid(6) + 1.0, true)] {
        run.folly.as_mut().unwrap().spooked.clear();
        let w = &mut run.folly.as_mut().unwrap().wraiths[0];
        (w.at, w.to, w.step) = ((6, 1), (6, 1), 0);
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x, mid(1));
        run.step(&BTreeMap::new());
        assert_eq!(run.folly.as_ref().unwrap().spooked.contains_key(&1), touched, "a knight {} units off", x - mid(6));
    }
    // Every lane tile a wraith can stand on is a whole tile or more from
    // any lane tile it has no lane to: there is no touching through stone.
    let (cols, rows) = (run.room().cols as i32, run.room().rows as i32);
    for y in 0..rows {
        for x in 0..cols {
            if lane(&run, (x, y)) && lane(&run, (x + 1, y + 1)) {
                assert!(lane(&run, (x + 1, y)) || lane(&run, (x, y + 1)), "lanes meet diagonally at {x},{y}");
            }
        }
    }
}

#[test]
fn holy_water_pales_them_and_a_caught_one_goes_home_and_comes_back_calm() {
    let mut run = floor();
    run.players.retain(|&id, _| id == 1);
    quiet_maze(&mut run);
    let cols = run.room().cols as i32;
    // Wraith 0 roaming a lane going east.
    loose(&mut run, 0, (5, 1));
    run.folly.as_mut().unwrap().wraiths[0].to = (6, 1);
    // The knight drinks the holy water in the top-left corner.
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (mid(1), mid(1));
    }
    run.step_sim_for_test(&BTreeMap::new());
    let maze = run.folly.as_ref().unwrap();
    assert!(!maze.holy.contains(&((cols + 1) as u16)), "drunk");
    assert_eq!(maze.pale, folly::PALE - 0, "the pale time begins");
    assert!(maze.wraiths.iter().all(|w| w.pale), "every wraith turns pale");
    assert_eq!(maze.wraiths[0].way(), (-1, 0), "and those abroad turn tail");
    assert!(!maze.warning());
    // Its warning flashes in the last of it, then it is over.
    run.folly.as_mut().unwrap().pale = folly::PALE_WARN;
    run.step_sim_for_test(&BTreeMap::new());
    assert!(run.folly.as_ref().unwrap().warning());
    // Caught: eyes race home, rest, and come out calm though the water
    // still runs.
    run.folly.as_mut().unwrap().pale = folly::PALE;
    let (wx, wy) = run.folly.as_ref().unwrap().wraiths[0].pos();
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (wx * TILE_UNITS, wy * TILE_UNITS);
    }
    run.step_sim_for_test(&BTreeMap::new());
    assert!(run.folly.as_ref().unwrap().wraiths[0].eyes);
    let home = run.folly.as_ref().unwrap().wraiths[0].home;
    let mut ticks = 0;
    while run.folly.as_ref().unwrap().wraiths[0].rest == 0 {
        // Keep the knight out of the way, and the water running.
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (mid(cols - 2), mid(1));
        run.folly.as_mut().unwrap().pale = folly::PALE;
        run.step_sim_for_test(&BTreeMap::new());
        ticks += 1;
        assert!(ticks < 5 * HZ, "home within a few seconds");
    }
    let w = &run.folly.as_ref().unwrap().wraiths[0];
    assert_eq!(w.at, home);
    assert!(!w.eyes && !w.pale);
    // It rests, then climbs out a step at a time (a step or two more when a
    // tick of the climb falls on the knight's first stir).
    for _ in 0..folly::REST + 4 * folly::LEAVE_STEP {
        run.folly.as_mut().unwrap().pale = folly::PALE;
        run.step_sim_for_test(&BTreeMap::new());
        let maze = run.folly.as_ref().unwrap();
        if maze.wraiths[0].roaming() && !maze.in_crypt(maze.wraiths[0].at) {
            break;
        }
    }
    let maze = run.folly.as_ref().unwrap();
    let w = &maze.wraiths[0];
    assert!(w.roaming() && !maze.in_crypt(w.at) && !maze.pale_now(w), "out again, and calm: {w:?}");
}

#[test]
fn the_maze_ends_when_the_wax_is_gone() {
    let mut run = floor();
    run.players.retain(|&id, _| id == 1);
    quiet_maze(&mut run);
    let cols = run.room().cols as i32;
    let last = (2 * cols + 1) as u16;
    {
        let maze = run.folly.as_mut().unwrap();
        maze.wax = [last].into_iter().collect();
    }
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (mid(1), mid(2));
    }
    run.step(&BTreeMap::new());
    let maze = run.folly.as_ref().unwrap();
    assert!(maze.done && maze.left() > 0, "done before the candle");
    assert!(!run.barred());
    assert!(run.found.as_ref().unwrap().2.contains("ATE THE MOST WAX"));
}

#[test]
fn the_absent_host_and_a_fallen_knight_sit_it_out() {
    let mut run = Run::new(41, 1, None);
    run.join_seat(2, "Two");
    run.join_seat(3, "Three");
    run.enemies.clear();
    run.set_absent(1, true);
    run.players.get_mut(&3).unwrap().hp = 0;
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 50);
    }
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    let maze = run.folly.as_ref().unwrap();
    assert_eq!(maze.ante.keys().copied().collect::<Vec<_>>(), vec![2], "only the knight playing antes");
    assert_eq!(maze.eaten.keys().copied().collect::<Vec<_>>(), vec![2]);
    assert!(!maze.starts.contains_key(&1), "the absent seat has no place");
    assert_eq!((run.room().cols, run.room().rows), layout::party_room(1), "a maze for the one knight playing");
    let start = maze.starts[&2];
    assert_eq!(maze.door, start);
    // Burn the candle down: the one knight playing takes the pot.
    run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1;
    run.step(&BTreeMap::new());
    assert!(run.folly.as_ref().unwrap().done);
    assert!(run.found.as_ref().unwrap().2.starts_with("TWO"));
    assert!(!run.players[&3].laurel && !run.players[&1].laurel);
    assert!(run.valid_snapshot());
}

#[test]
fn an_away_knight_is_left_alone() {
    let mut run = floor();
    quiet_maze(&mut run);
    run.set_away(2, true);
    loose(&mut run, 0, (3, 1));
    {
        let h = run.players.get_mut(&2).unwrap();
        (h.x, h.y) = (mid(3), mid(1));
    }
    let eaten = run.folly.as_ref().unwrap().eaten.get(&2).copied();
    run.step(&BTreeMap::new());
    let maze = run.folly.as_ref().unwrap();
    assert!(!maze.spooked.contains_key(&2), "a stone knight is no wraith's");
    assert_eq!(maze.eaten.get(&2).copied(), eaten);
}

#[test]
fn a_maze_left_and_come_back_to_plays_on() {
    let mut run = floor();
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 50);
    }
    run.pick_folly();
    let index = run.folly_room.unwrap();
    run.enter_for_test(index);
    for _ in 0..folly::TITLE + HZ {
        run.step(&BTreeMap::new());
    }
    let gold = run.players[&1].carried.get(Spoil::Gold);
    let before = run.folly.clone().unwrap();
    // Out of the room by any way at all: the maze waits as it stands.
    let other = (0..run.dungeon.rooms.len()).find(|&i| i != index).unwrap();
    run.enter(other, Some(0));
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.folly.as_ref().unwrap().t, before.t, "the candle waits");
    assert!(run.valid_snapshot());
    // Back in: it plays on, with no new ante and no company.
    run.enter(index, Some(2));
    assert!(run.barred(), "its doors hold again");
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    assert!(run.folly.as_ref().unwrap().t > before.t);
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), gold, "no second ante");
    assert!(run.enemies.is_empty(), "no company raised in the maze");
    assert!(run.valid_snapshot());
}

#[test]
fn a_saved_maze_plays_on_as_it_would_have() {
    let mut run = floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    let right = BTreeMap::from([(1, Input { move_x: 1, ..Default::default() }), (2, Input { move_y: -1, ..Default::default() })]);
    for _ in 0..folly::TITLE + 3 * HZ {
        run.step(&right);
    }
    let saved = serde_json::to_string(&run).unwrap();
    let mut back: Run = serde_json::from_str(&saved).unwrap();
    assert!(back.valid_snapshot());
    for _ in 0..3 * HZ {
        run.step(&right);
        back.step(&right);
    }
    assert_eq!(back.folly, run.folly, "the restored maze plays the same");
    assert_eq!(back.players[&1].x, run.players[&1].x);
}

#[test]
fn the_maze_waits_while_the_room_holds() {
    let mut run = floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    for _ in 0..folly::TITLE + HZ {
        run.step(&BTreeMap::new());
    }
    let before = run.folly.clone().unwrap();
    run.hold(presence::Hold::Arriving);
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.folly.as_ref().unwrap(), &before, "the candle and the wraiths wait for a friend's screen");
    run.hold(presence::Hold::Open);
    run.step(&BTreeMap::new());
    assert_eq!(run.folly.as_ref().unwrap().t, before.t + 1);
}

#[test]
fn the_skills_bar_keeps_off_the_maze_until_the_podium() {
    use crate::render::overworld::arena;
    let mut run = floor();
    quiet_maze(&mut run);
    // The bar's plates in the seat's HUD band, under the view.
    let plates = |run: &Run, focus: Option<u32>| {
        let hud = arena::hud_band(run, focus);
        (8..arena::HUD_H - 8).flat_map(|y| (6..150).map(move |x| (x, y))).filter(|&(x, y)| hud.get(x, y).is_some()).count()
    };
    for focus in [Some(1), Some(2)] {
        assert_eq!(plates(&run, focus), 0, "no skills bar while the maze runs");
    }
    run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1;
    run.step(&BTreeMap::new());
    assert!(plates(&run, Some(1)) > 300, "the podium called, the bar is back");
}

#[test]
fn a_drop_eaten_is_one_word_to_a_friends_mirror() {
    use angel_net::doc::delta::{Patch, diff};
    fn field<'a>(patch: &'a Patch, name: &str) -> Option<&'a Patch> {
        match patch {
            Patch::Struct { set, .. } => set.iter().find_map(|(n, p)| if n == name { Some(p) } else { field(p, name) }),
            Patch::Some(inner) | Patch::Variant(inner) => field(inner, name),
            _ => None,
        }
    }
    let mut run = floor();
    run.players.retain(|&id, _| id == 1);
    quiet_maze(&mut run);
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (mid(3), mid(1));
    }
    run.step(&BTreeMap::new());
    let before = crate::wire::publish(&run).unwrap();
    let wax = run.folly.as_ref().unwrap().wax.len();
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (mid(4), mid(1));
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.folly.as_ref().unwrap().wax.len(), wax - 1);
    let after = crate::wire::publish(&run).unwrap();
    let patch = diff(&before, &after).unwrap();
    match field(&patch, "wax") {
        Some(Patch::Seq { items, .. }) => assert_eq!(items.len(), 1, "one word changed"),
        other => panic!("the wax went as {other:?}"),
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_folly_shots -- --ignored`
#[test]
#[ignore]
fn write_folly_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("folly");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    run.step(&BTreeMap::new());
    save(&run, "1-title");
    for _ in 0..folly::TITLE + 3 * HZ {
        run.step(&BTreeMap::new());
    }
    let cols = run.room().cols as i32;
    for x in 2..cols / 2 {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = ((x as f32 + 0.5) * TILE_UNITS, 1.5 * TILE_UNITS);
        h.invulnerable = 1000;
        run.step(&BTreeMap::new());
    }
    save(&run, "2-play");
    run.folly.as_mut().unwrap().pale = folly::PALE;
    run.step(&BTreeMap::new());
    save(&run, "3-pale");
    run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1;
    run.step(&BTreeMap::new());
    save(&run, "4-done");
}

#[test]
fn a_friends_mirror_sees_the_maze_and_its_wraiths() {
    let mut run = floor();
    run.pick_folly();
    let mut mirror = run.clone();
    run.enter_for_test(run.folly_room.unwrap());
    for _ in 0..folly::TITLE + 10 {
        run.step(&BTreeMap::new());
    }
    // The maze changed the room's stones: the mirror takes the whole floor.
    assert_ne!(mirror.dungeon.world_revision, run.dungeon.world_revision);
    mirror = run.clone();
    for _ in 0..20 {
        run.step(&BTreeMap::new());
        mirror = run.wire_copy();
    }
    assert_eq!(mirror.folly, run.folly, "wax and wraiths come with every tick");
}

fn even_floor() -> Run {
    let mut run = Run::new(41, 1, None);
    run.descend_for_test();
    run.join_seat(2, "Two");
    run.enemies.clear();
    run
}

#[test]
fn the_even_floors_folly_is_the_belfry_shoot() {
    let mut run = even_floor();
    assert_eq!(run.dungeon.depth % 2, 0);
    run.pick_folly();
    let index = run.folly_room.expect("a hall on floor two");
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 40);
    }
    run.enter_for_test(index);
    assert!(run.bats.is_some() && run.folly.is_none());
    assert!(run.barred());
    for _ in 0..folly::TITLE + folly::BAT_EVERY + 1 {
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies.iter().any(|e| e.kind == EnemyKind::Bat), "the bats wake");
    // Knight one's harm is credited; burn the clock down.
    run.credit_bats(&[(1, 250)]);
    run.bats.as_mut().unwrap().t = folly::TITLE + folly::BAT_TIME - 1;
    run.step(&BTreeMap::new());
    let shoot = run.bats.as_ref().unwrap();
    assert!(shoot.done);
    assert!(!run.enemies.iter().any(|e| e.kind == EnemyKind::Bat));
    assert!(run.players[&1].laurel);
    assert!(run.found.as_ref().unwrap().2.contains("SHOT THE MOST BATS"));
    assert!(!run.barred());
    assert!(run.valid_snapshot());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_bats_shots -- --ignored`
#[test]
#[ignore]
fn write_bats_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("folly");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = even_floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    run.step(&BTreeMap::new());
    save(&run, "b1-title");
    for _ in 0..folly::TITLE + 5 * HZ {
        run.step(&BTreeMap::new());
        for h in run.players.values_mut() {
            h.invulnerable = 100;
        }
    }
    run.credit_bats(&[(1, 180), (2, 90)]);
    save(&run, "b2-play");
}
