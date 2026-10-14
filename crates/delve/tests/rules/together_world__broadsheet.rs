// The Paperboy homage must be winnable by walking, not just by placing a
// knight at each mailbox. Extend the existing world/mirror test entry.
fn walk_pastime_to(run: &mut Run, driver: u32, target: (f32, f32), held: bool) {
    use std::collections::VecDeque;

    let old_at = run.at;
    let room = run.room();
    assert!(room.kind.in_world(), "walking helper stays in the upper world");
    let hero = &run.players[&driver];
    let start = ((hero.x / TILE_UNITS) as usize, (hero.y / TILE_UNITS) as usize);
    let goal = (target.0 as usize, target.1 as usize);
    let cell = |(x, y): (usize, usize)| y * room.cols + x;
    let centre = |k: usize| {
        (
            (k % room.cols) as f32 * TILE_UNITS + TILE_UNITS / 2.0,
            (k / room.cols) as f32 * TILE_UNITS + TILE_UNITS / 2.0,
        )
    };
    let grid = Grid { room, barred: false };
    let mut parents = vec![None; room.cols * room.rows];
    parents[cell(start)] = Some(cell(start));
    let mut queue = VecDeque::from([start]);
    while let Some((x, y)) = queue.pop_front() {
        let here = cell((x, y));
        if (x, y) == goal {
            break;
        }
        for (dx, dy) in DIRS {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx <= 0 || ny <= 0 || nx >= room.cols as i32 - 1 || ny >= room.rows as i32 - 1 {
                continue;
            }
            let candidate = cell((nx as usize, ny as usize));
            let (cx, cy) = centre(candidate);
            let (hx, hy) = centre(here);
            if parents[candidate].is_none()
                && room.tile(nx, ny) == Tile::Floor
                && grid.clear(cx, cy, HERO_RADIUS, Mover::Hero)
                && grid.clear((cx + hx) / 2.0, (cy + hy) / 2.0, HERO_RADIUS, Mover::Hero)
            {
                parents[candidate] = Some(here);
                queue.push_back((nx as usize, ny as usize));
            }
        }
    }
    assert!(parents[cell(goal)].is_some(), "no ordinary walking path to pastime target {target:?}");
    let mut path = vec![cell(goal)];
    while *path.last().unwrap() != cell(start) {
        path.push(parents[*path.last().unwrap()].unwrap());
    }
    let mut targets: Vec<_> = path.into_iter().rev().map(centre).collect();
    targets.push((target.0 * TILE_UNITS, target.1 * TILE_UNITS));
    for (tx, ty) in targets {
        let mut reached = false;
        for _ in 0..2 * HZ {
            let hero = &run.players[&driver];
            let axis = |delta: f32| {
                if delta.abs() <= HERO_SPEED * DT * 0.6 {
                    0
                } else if delta < 0.0 {
                    -1
                } else {
                    1
                }
            };
            let (move_x, move_y) = (axis(tx - hero.x), axis(ty - hero.y));
            if move_x == 0 && move_y == 0 {
                reached = true;
                break;
            }
            let mut inputs = BTreeMap::from_iter([2, 3].map(|id| {
                (id, Input { fire: held, ..Default::default() })
            }));
            inputs.insert(driver, Input { move_x, move_y, fire: held, ..Default::default() });
            run.step(&inputs);
            assert_eq!(run.at, old_at, "walking to a pastime must not exit its room");
            assert!(run.settlement_walkable_for_test(run.players[&driver].x, run.players[&driver].y));
        }
        assert!(reached, "knight {driver} stuck walking to {tx},{ty}");
    }
}

fn broadsheet_walking_regression() {
    use super::super::pastimes::{Game, MAILBOXES};

    let empty = BTreeMap::new();
    let fire = BTreeMap::from_iter([2, 3].map(|id| {
        (id, Input { fire: true, ..Default::default() })
    }));
    let mut run = at_the_gate();
    run.join_seat(3, "Deputy Editor");
    run.players.get_mut(&1).unwrap().stone = true;
    // No arrive() or coordinate assignments after the initial gate fixture.
    walk_ordinary_road(&mut run, RoomKind::Forest, 2);
    walk_ordinary_road(&mut run, RoomKind::Village, 3);
    let treasury = run.treasury.clone();
    let mut masks = std::collections::BTreeSet::new();
    let index = Game::Broadsheet.index();

    for rotation in 0..5 {
        walk_pastime_to(&mut run, 2, Game::Broadsheet.start(), false);
        let round = run.pastimes.round.as_ref().expect("walking onto P starts delivery");
        assert_eq!(round.game, Game::Broadsheet);
        let mask = [0, 2, 3].into_iter().fold(0u8, |m, i| m | (1 << ((i + rotation) % 5)));
        assert_eq!(round.recipients, mask);
        assert!(masks.insert(mask), "each round must rotate its subscriber flags");
        assert_eq!(run.home.countryside.games.plays[index], (rotation + 1) as u32);
        let subscribers: Vec<_> = (0..5).filter(|i| mask & (1 << i) != 0).collect();
        let wrong: Vec<_> = (0..5).filter(|i| mask & (1 << i) == 0).collect();
        let score = run.score;
        let start_left = round.left;

        // Two simultaneous couriers at one non-subscriber count only one
        // attempted delivery, even if they keep tapping or hold the key.
        for id in [2, 3] {
            walk_pastime_to(&mut run, id, MAILBOXES[wrong[0]], false);
        }
        let mistakes = run.pastimes.round.as_ref().unwrap().mistakes;
        run.step(&fire);
        let round = run.pastimes.round.as_ref().unwrap();
        assert_eq!((round.progress, round.tried), (0, 1 << wrong[0]));
        assert_eq!(round.mistakes, mistakes + 1);
        for _ in 0..3 {
            run.step(&fire);
        }
        run.step(&empty);
        run.step(&fire);
        assert_eq!(run.pastimes.round.as_ref().unwrap().mistakes, mistakes + 1);

        // Walking to another mailbox with F still down does not throw there.
        walk_pastime_to(&mut run, 2, MAILBOXES[wrong[1]], true);
        assert_eq!(run.pastimes.round.as_ref().unwrap().tried, 1 << wrong[0]);
        let mistakes = run.pastimes.round.as_ref().unwrap().mistakes;
        run.step(&empty);
        run.step(&fire);
        let round = run.pastimes.round.as_ref().unwrap();
        assert_eq!((round.progress, round.tried), (0, (1 << wrong[0]) | (1 << wrong[1])));
        assert_eq!(round.mistakes, mistakes + 1);

        walk_pastime_to(&mut run, 2, MAILBOXES[subscribers[0]], false);
        run.step(&fire);
        assert_eq!(run.pastimes.round.as_ref().unwrap().progress, 1);
        // Checkpoint while both keys are held: restore must retain the exact
        // subscriber/tried masks, clock, goose cooldown and per-seat key latch.
        let pastimes = run.pastimes.clone();
        run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
        assert!(run.valid_snapshot());
        assert_eq!(run.pastimes, pastimes);
        let mirror = run.wire_copy();
        assert_eq!(mirror.pastimes, run.pastimes);
        assert_eq!(mirror.home.countryside.games, run.home.countryside.games);
        run.step(&fire);
        assert_eq!(run.pastimes.round.as_ref().unwrap().progress, 1);

        // Each paper knight walks their own round: the starter walks on to
        // the next subscriber, and a held key there does not throw.
        let tried = run.pastimes.round.as_ref().unwrap().tried;
        walk_pastime_to(&mut run, 2, MAILBOXES[subscribers[1]], true);
        assert_eq!(run.pastimes.round.as_ref().unwrap().tried, tried);
        run.step(&empty);
        run.step(&fire);
        assert_eq!(run.pastimes.round.as_ref().unwrap().progress, 2);
        walk_pastime_to(&mut run, 2, MAILBOXES[subscribers[2]], false);
        let round = run.pastimes.round.as_ref().unwrap();
        assert_eq!(round.progress, 2);
        assert!(round.left > 1, "all walking deliveries must fit the 35-second clock");
        let elapsed = Game::Broadsheet.time() - round.left + 1;
        let result = (225 + (round.left - 1) / HZ * 5)
            .saturating_sub(u32::from(round.mistakes) * 25)
            .clamp(25, 1000);
        run.step(&fire);
        assert!(run.pastimes.round.is_none());
        let records = &run.home.countryside.games;
        assert_eq!(records.wins[index], (rotation + 1) as u32);
        assert!(u32::from(records.best[index]) >= result);
        assert_eq!(run.score, score + result);
        assert_eq!(run.treasury, treasury, "paper-round practice never mints treasury gold");
        let mirror = run.wire_copy();
        assert_eq!(mirror.home.countryside.games, run.home.countryside.games);
        let score = run.score;
        for _ in 0..HZ {
            run.step(&fire);
        }
        assert!(run.pastimes.round.is_none());
        assert_eq!(run.score, score, "completed delivery cannot pay twice");
        println!("Broadsheet rotation {rotation}: mask={mask:05b}, walking round {elapsed}/{} ticks (start left {start_left}), team +{result}, treasury unchanged", Game::Broadsheet.time());
    }
    assert_eq!(masks.len(), 5);

    // A sixth walking retry restores two actual deliveries, then times out on
    // a genuine final-mailbox tap. No injected timer, progress or pad state.
    walk_pastime_to(&mut run, 2, Game::Broadsheet.start(), false);
    let mask = run.pastimes.round.as_ref().unwrap().recipients;
    let subscribers: Vec<_> = (0..5).filter(|i| mask & (1 << i) != 0).collect();
    for &i in &subscribers[..2] {
        walk_pastime_to(&mut run, 2, MAILBOXES[i], false);
        run.step(&fire);
    }
    walk_pastime_to(&mut run, 2, MAILBOXES[subscribers[2]], false);
    assert_eq!(run.pastimes.round.as_ref().unwrap().progress, 2);
    run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(run.valid_snapshot());
    while run.pastimes.round.as_ref().unwrap().left > 1 {
        run.step(&empty);
    }
    let score = run.score;
    run.step(&fire);
    assert!(run.pastimes.round.is_none(), "expiry must precede the last delivery");
    assert_eq!(run.home.countryside.games.plays[index], 6);
    assert_eq!(run.home.countryside.games.wins[index], 5);
    assert_eq!(run.score, score);
    assert_eq!(run.treasury, treasury);
    println!("Broadsheet sixth walking retry: two delivered, restored, last-tick tap expired without payout");
}
