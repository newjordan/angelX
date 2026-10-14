// The quay's ordered wake course must be playable on foot, not only by
// assigning a knight to every wake. Called by the established world/mirror test.
fn washtub_walking_regression() {
    use super::super::pastimes::{Game, WAKES};

    let game = Game::Washtub;
    let index = game.index();
    let empty = BTreeMap::new();
    let mut base = at_the_gate();
    base.join_seat(3, "Deputy Duck Inspector");
    base.players.get_mut(&1).unwrap().stone = true;
    let treasury = base.treasury.clone();
    for kind in [RoomKind::Forest, RoomKind::Fairground, RoomKind::Riverbank] {
        walk_ordinary_road(&mut base, kind, 2);
    }
    // Park the second friend clear of the course, then approach W from below.
    // All movement after the initial gate fixture is ordinary Run::step input.
    walk_pastime_to(&mut base, 3, (12.0, 11.0), false);
    walk_pastime_to(&mut base, 2, (12.0, 12.0), false);
    walk_pastime_to(&mut base, 2, (4.0, 12.0), false);
    assert!(base.pastimes.round.is_none());
    assert_eq!(base.treasury, treasury);
    let mut phases = std::collections::BTreeSet::new();
    let mut finished = None;

    // The wakes have an eight-second drift cycle. Start one walking round in
    // each second of it; no timer/progress/pad or hero coordinates are injected.
    for delay in (0..8 * HZ).step_by(HZ as usize) {
        let mut run = base.clone();
        for _ in 0..delay {
            run.step(&empty);
        }
        walk_pastime_to(&mut run, 2, game.start(), false);
        let round = run.pastimes.round.as_ref().expect("walking onto W starts the regatta");
        assert_eq!((round.game, round.progress), (game, 0));
        assert_eq!(run.home.countryside.games.plays[index], 1);
        let phase = run.tick % u64::from(8 * HZ);
        assert!(phases.insert(phase), "wake phases must be distinct");
        let began_at = run.tick - u64::from(game.time() - round.left);
        let score = run.score;

        // First visit wake four by walking north, then east. It is not the
        // required first ring: being at a future ring must not skip the course.
        walk_pastime_to(&mut run, 2, (4.0, 5.5), false);
        walk_pastime_to(&mut run, 2, WAKES[3], false);
        assert_eq!(run.pastimes.round.as_ref().unwrap().progress, 0);
        assert_eq!(run.score, score);

        for (i, target) in WAKES.into_iter().enumerate() {
            // Each knight paddles their own course; seat 2 started this one.
            let driver = 2;
            // The base of the visibly drifting ring remains within its reach;
            // walk there instead of teleporting to its instantaneous position.
            walk_pastime_to(&mut run, driver, target, false);
            if i + 1 < WAKES.len() {
                assert_eq!(run.pastimes.round.as_ref().unwrap().progress, (i + 1) as u8);
            } else {
                assert!(run.pastimes.round.is_none(), "all six walked wakes finish the round");
            }
            if i == 2 {
                let state = run.pastimes.clone();
                run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
                assert_eq!(run.pastimes, state, "checkpoint preserves the moving-course clock and order");
            }
            assert!(run.valid_snapshot());
            let mirror = run.wire_copy();
            assert_eq!(mirror.pastimes, run.pastimes);
            assert_eq!(mirror.home.countryside.games, run.home.countryside.games);
            assert_eq!(run.treasury, treasury);
        }
        let elapsed = run.tick - began_at;
        assert!(elapsed < u64::from(game.time()), "all walking must fit the 30-second course");
        let result = run.score - score;
        let records = &run.home.countryside.games;
        assert_eq!((records.plays[index], records.wins[index]), (1, 1));
        assert_eq!(u32::from(records.best[index]), result);
        assert!((25..=1000).contains(&result));
        for _ in 0..HZ {
            run.step(&empty);
        }
        assert!(run.pastimes.round.is_none());
        assert_eq!(run.home.countryside.games.wins[index], 1);
        assert_eq!(run.score, score + result, "finished regatta cannot pay twice");
        assert_eq!(run.treasury, treasury, "washtub practice must never mint treasury gold");
        println!("Washtub: drift phase {phase}/{}; statue host, alternating friends, future wake ignored, six wakes walked in {elapsed}/{} ticks, restored at wake 3, live mirror, team +{result}, treasury unchanged", 8 * HZ, game.time());
        finished = Some(run);
    }
    assert_eq!(phases.len(), 8);

    let mut run = finished.unwrap();
    // The other friend ended on wake six. Move them clear before the retry:
    // otherwise their perfectly legitimate co-op presence would finish it.
    walk_pastime_to(&mut run, 3, (12.0, 11.0), false);
    let score = run.score;
    // A real walking retry, then timeout with five rings completed. Wait away
    // from the sixth ring; neither its timer nor progress is rewritten.
    walk_pastime_to(&mut run, 2, (4.0, 12.0), false);
    walk_pastime_to(&mut run, 2, game.start(), false);
    assert_eq!(run.home.countryside.games.plays[index], 2);
    for (i, target) in WAKES.into_iter().take(5).enumerate() {
        walk_pastime_to(&mut run, 2, target, false);
        assert_eq!(run.pastimes.round.as_ref().unwrap().progress, (i + 1) as u8);
    }
    let state = run.pastimes.clone();
    run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert!(run.valid_snapshot());
    assert_eq!(run.pastimes, state);
    while run.pastimes.round.is_some() {
        run.step(&empty);
    }
    assert_eq!(run.score, score, "five wakes and a timeout must not pay");
    assert_eq!(run.home.countryside.games.wins[index], 1);
    // Entering the final ring after time cannot finish or pay the expired round.
    walk_pastime_to(&mut run, 2, WAKES[5], false);
    assert!(run.pastimes.round.is_none());
    assert_eq!(run.score, score);
    assert_eq!(run.treasury, treasury);

    // Leaving an unfinished third attempt cancels it but keeps the saved best.
    walk_pastime_to(&mut run, 2, (4.0, 12.0), false);
    walk_pastime_to(&mut run, 2, game.start(), false);
    assert_eq!(run.home.countryside.games.plays[index], 3);
    let records = run.home.countryside.games.clone();
    walk_ordinary_road(&mut run, RoomKind::Fairground, 2);
    assert!(run.pastimes.round.is_none());
    assert_eq!(run.home.countryside.games, records);
    assert_eq!(run.score, score);
    assert_eq!(run.treasury, treasury);
    println!("Washtub: walked off/back to retry; five-wake restore then timeout without payout; final ring after expiry ignored; ordinary road exit cancels next attempt and keeps records");
}
