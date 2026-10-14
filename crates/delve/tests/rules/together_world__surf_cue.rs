// Inputs arrive after a world snapshot was painted: a gold wave is permission
// to brace, not a prediction of the next simulation tick.
fn shield_surfing_accepts_only_the_published_gold_wave() {
    use super::super::pastimes::{CREST_WINDOW, Surf, WAVE_LEANS, WAVE_TICKS};
    for phase in 0..WAVE_TICKS {
        let mut surf = Surf::new(1);
        surf.phase = phase;
        surf.lean = WAVE_LEANS[0];
        let cleared = surf.step(Input::default(), true, 0);
        let accepted = surf.braced || cleared == Some(true);
        assert_eq!(
            accepted,
            phase >= WAVE_TICKS - CREST_WINDOW,
            "tap against published phase {phase}/{WAVE_TICKS}"
        );
    }
}

// Run through the established host/mirror regression entry point. All walking
// and six-wave input comes from Run::step; no hero/round coordinates are injected.
fn shield_surfing_walks_in_and_clears_each_last_gold_frame() {
    use super::super::pastimes::{Game, WAVE_LEANS, WAVE_TICKS};

    let mut run = at_the_gate();
    run.join_seat(3, "Unhelpful Lifeguard");
    run.players.get_mut(&1).unwrap().stone = true;
    let treasury = run.treasury.clone();
    for kind in [
        RoomKind::Forest,
        RoomKind::Village,
        RoomKind::Cliffs,
        RoomKind::Timberyard,
        RoomKind::Millpond,
    ] {
        walk_ordinary_road(&mut run, kind, 2);
    }
    walk_pastime_to(&mut run, 2, Game::Surf.start(), false);
    let surf = run.pastimes.round.as_ref().unwrap().surf.as_ref().unwrap();
    assert_eq!(surf.owner, 2, "the living friend owns the board, not the statue");
    let mut cleared = 0;
    let mut restored = false;
    let score = run.score;
    let mut result = 0;
    for _ in 0..Game::Surf.time() {
        let Some(round) = run.pastimes.round.as_ref() else { break };
        let wave = round.progress;
        let surf = round.surf.as_ref().unwrap();
        let phase = surf.phase;
        let lean = surf.lean;
        let left = round.left;
        assert_eq!(surf.owner, 2);
        if wave == 2 && phase == WAVE_TICKS - 2 {
            let state = run.pastimes.clone();
            run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
            assert!(run.valid_snapshot());
            assert_eq!(run.pastimes, state, "restore preserves the published crest and key latches");
            restored = true;
        }
        // Brace on the LAST published gold frame, not the beginning of the
        // window used by the old full-course check. Hold across wave one's
        // boundary; the lifeguard's held F/opposite lean must not steal control.
        let last_gold = phase == WAVE_TICKS - 1;
        let held = wave == 1 && phase < 5;
        run.step(&BTreeMap::from([
            (2, Input {
                aim_x: (WAVE_LEANS[usize::from(wave)] - lean).signum() as i8,
                fire: last_gold || held,
                ..Default::default()
            }),
            (3, Input { aim_x: -1, fire: true, ..Default::default() }),
        ]));
        assert!(run.valid_snapshot());
        let mirror = run.wire_copy();
        assert_eq!(mirror.pastimes, run.pastimes);
        assert_eq!(mirror.home.countryside.games, run.home.countryside.games);
        assert_eq!(run.treasury, treasury);
        if last_gold {
            cleared += 1;
            println!("Shield Surf: friend 2 clears wave {} on published gold phase {phase}/{WAVE_TICKS}", wave + 1);
            if wave + 1 == Game::Surf.need() {
                result = 450 + (left - 1) / HZ * 5;
                assert!(run.pastimes.round.is_none());
            } else {
                let next = run.pastimes.round.as_ref().unwrap();
                assert_eq!(next.progress, wave + 1);
                assert_eq!(next.surf.as_ref().unwrap().phase, 0);
            }
        }
    }
    assert_eq!(cleared, 6);
    assert!(restored);
    assert!(run.pastimes.round.is_none());
    let records = run.home.countryside.games.surf.clone();
    assert_eq!((records.plays, records.wins), (1, 1));
    assert_eq!(u32::from(records.best), result);
    assert_eq!(run.score, score + result);
    for _ in 0..HZ {
        run.step(&BTreeMap::from([(2, Input { fire: true, ..Default::default() })]));
    }
    assert_eq!(run.home.countryside.games.surf, records);
    assert_eq!(run.score, score + result, "holding F after the finish must not pay twice");
    assert_eq!(run.treasury, treasury, "surf practice does not create treasury gold");
    // Retrying requires a real step off and back onto U, not a pad reset.
    walk_pastime_to(&mut run, 2, (3.0, 11.0), false);
    walk_pastime_to(&mut run, 2, Game::Surf.start(), false);
    assert_eq!(run.home.countryside.games.surf.plays, 2);
    let score = run.score;
    let mut last_unbraced = false;
    for _ in 0..WAVE_TICKS {
        let Some(round) = run.pastimes.round.as_ref() else { break };
        let surf = round.surf.as_ref().unwrap();
        let lean = surf.lean;
        if surf.phase == WAVE_TICKS - 1 {
            assert!((lean - WAVE_LEANS[0]).abs() <= 16, "wipeout must not be caused by poor balance");
            assert!(!surf.braced, "a held key must not become a fresh gold-frame tap");
            last_unbraced = true;
        }
        run.step(&BTreeMap::from([(2, Input {
            aim_x: (WAVE_LEANS[0] - lean).signum() as i8,
            fire: true,
            ..Default::default()
        })]));
    }
    assert!(last_unbraced, "the balanced retry must reach its last gold frame");
    assert!(run.pastimes.round.is_none(), "held F cannot brace at a later crest");
    assert_eq!(run.home.countryside.games.surf.wins, 1);
    assert_eq!(run.score, score);
    assert_eq!(run.treasury, treasury);
    println!("Shield Surf: walked Gate -> Millpond, statue host, six last-gold taps, restored before crest, live mirror, held-key wipeout/retry, practice +{result}, treasury unchanged");
}
