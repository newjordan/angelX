// Turnip Keepy-Uppy: kick a turnip in time with a beating phase.
// Tests cover initialization, kick timing windows, double-kick rejection,
// timeout, progress advancement, and serialization.
//
// Key mechanics (from pastimes.rs):
//   HZ = 30 ticks/sec; beat = elapsed / HZ; phase = elapsed % HZ
//   Kick succeeds when phase <= HZ / 5 (i.e. phase <= 6)
//   Hit radius: 2.2 tiles from Game::Turnip.start()
//   last_beat guard: one kick per beat maximum
//   Need: 8 successful kicks; Time: 20 * HZ = 600 ticks

/// Set up a run with the Turnip game active. Both players arrive at the
/// Fairground pad; player 1 is the starter. One empty step starts the game.
fn start_turnip_game() -> Run {
    use super::super::pastimes::Game;
    let mut run = at_the_gate();
    run.arrive(
        room_of(&run.dungeon, RoomKind::Fairground).unwrap(),
        Game::Turnip.start(),
    );
    run.step(&BTreeMap::new());
    assert!(
        run.pastimes.round.is_some(),
        "arriving on the pad starts the Turnip game"
    );
    assert_eq!(run.pastimes.round.as_ref().unwrap().left, Game::Turnip.time());
    run
}

#[test]
fn turnip_initializes_correctly() {
    use super::super::pastimes::Game;
    let run = start_turnip_game();

    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.game, Game::Turnip);
    assert_eq!(round.progress, 0, "no kicks yet");
    assert_eq!(round.mistakes, 0, "no mistakes yet");
    assert_eq!(round.last_beat, None, "no beat has been kicked yet");
    assert_eq!(round.tried, 0);
    assert_eq!(round.left, Game::Turnip.time(), "full clock at start");
    assert_eq!(round.game.need(), 8, "turnip needs 8 kicks to win");
    assert_eq!(round.game.time(), 20 * super::super::HZ, "turnip clock is 20 seconds");
    assert!(run.pastimes.heat.is_some(), "heat is set for the multi-player game");
}

#[test]
fn turnip_kick_within_window() {
    use super::super::pastimes::Game;
    let mut run = start_turnip_game();

    // Set round.left so that the next step lands at phase 0 of beat 0.
    // shown_left = Game::Turnip.time() => elapsed = 0, beat = 0, phase = 0.
    // Phase 0 <= HZ/5 (6) => gold window => success.
    run.pastimes.round.as_mut().unwrap().left = Game::Turnip.time();

    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));

    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.progress, 1, "kick in the gold window advances progress");
    assert_eq!(round.mistakes, 0, "clean kick, no mistake");
    assert_eq!(round.last_beat, Some(0), "beat 0 is recorded");
}

#[test]
fn turnip_kick_too_early() {
    use super::super::pastimes::Game;
    let mut run = start_turnip_game();

    // Phase 7: just past the gold window (HZ/5 = 6).
    // shown_left = 600 - 7 = 593 => elapsed = 7, beat = 0, phase = 7.
    run.pastimes.round.as_mut().unwrap().left = Game::Turnip.time() - 7;

    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));

    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.progress, 0, "early kick does not advance progress");
    assert_eq!(round.mistakes, 1, "early kick is a mistake");
    assert_eq!(round.last_beat, Some(0), "beat is still recorded");
}

#[test]
fn turnip_kick_too_late() {
    use super::super::pastimes::Game;
    let mut run = start_turnip_game();

    // Phase 29: the very last tick of beat 0 (HZ - 1 = 29).
    // shown_left = 600 - 29 = 571 => elapsed = 29, beat = 0, phase = 29.
    run.pastimes.round.as_mut().unwrap().left = Game::Turnip.time() - 29;

    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));

    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.progress, 0, "late kick does not advance progress");
    assert_eq!(round.mistakes, 1, "late kick is a mistake");
    assert_eq!(round.last_beat, Some(0), "beat is still recorded");
}

#[test]
fn turnip_double_kick_rejected() {
    use super::super::pastimes::Game;
    let mut run = start_turnip_game();

    // Set up phase 0 of beat 0 for a clean first kick.
    run.pastimes.round.as_mut().unwrap().left = Game::Turnip.time();

    // First kick: succeeds (phase 0, beat 0).
    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.progress, 1, "first kick succeeds");
    assert_eq!(round.last_beat, Some(0));

    // Release fire so the edge trigger can fire again.
    run.step(&BTreeMap::default());

    // Second kick: still beat 0 (elapsed = 2, phase = 2).
    // last_beat == Some(0) == Some(beat) => rejected by the guard.
    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.progress, 1, "second kick in same beat is rejected");
    assert_eq!(round.mistakes, 0, "rejected kick does not add a mistake");
}

#[test]
fn turnip_timeout_no_kicks() {
    let mut run = start_turnip_game();

    // Set both the lead round and every heat round to 1 tick remaining.
    run.pastimes.round.as_mut().unwrap().left = 1;
    for heat_round in run.pastimes.heats.values_mut() {
        heat_round.left = 1;
    }

    // One step expires every clock.
    run.step(&BTreeMap::default());

    // Player 1's round has been consumed by the timeout.
    assert!(
        run.pastimes.round.is_none(),
        "timeout with no kicks ends the lead round"
    );
    // No progress was made by anyone.
    let heat = run.pastimes.heat.as_ref();
    if let Some(h) = heat {
        for finish in h.done.values() {
            assert_eq!(finish.progress, 0, "no progress on timeout");
            assert!(finish.points.is_none(), "no points on timeout");
        }
    }
}

#[test]
fn turnip_progress_advances() {
    use super::super::pastimes::Game;
    let need = Game::Turnip.need(); // 8
    let mut run = start_turnip_game();

    // Deliver `need` successful kicks, one per beat, each at phase 0.
    for i in 0..u32::from(need) {
        // Set left so elapsed = i * HZ => beat = i, phase = 0.
        let left = Game::Turnip.time() - i * super::super::HZ;
        run.pastimes.round.as_mut().unwrap().left = left;

        // Kick at the gold window.
        run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));

        if i + 1 < u32::from(need) {
            let round = run.pastimes.round.as_ref().expect("round continues until need is met");
            assert_eq!(
                round.progress,
                (i + 1) as u8,
                "kick {}/{} advances progress",
                i + 1,
                need,
            );
            assert_eq!(round.mistakes, 0, "no mistakes on clean kicks");

            // Release fire so the next kick is a new edge.
            run.step(&BTreeMap::default());
        }
    }

    // After 8 clean kicks the game is won and the round is consumed.
    assert!(
        run.pastimes.round.is_none(),
        "reaching need completes the round"
    );
    let idx = Game::Turnip.index();
    assert_eq!(
        run.home.countryside.games.wins[idx], 1,
        "one win recorded"
    );
    assert!(
        run.home.countryside.games.best[idx] > 0,
        "a positive score was banked"
    );
}

#[test]
fn turnip_serialization_roundtrip() {
    use super::super::pastimes::Game;
    let run = start_turnip_game();

    // Record the live state.
    let original_pastimes = run.pastimes.clone();
    let original_score = run.score;

    // Serialize the entire run and restore it.
    let wire = serde_json::to_vec(&run).unwrap();
    let mut restored: Run = serde_json::from_slice(&wire).unwrap();
    assert!(restored.valid_snapshot(), "restored run passes validation");
    assert_eq!(
        restored.pastimes, original_pastimes,
        "pastimes survive a save/load cycle"
    );
    assert_eq!(restored.score, original_score);

    // The restored game must remain playable: a kick at phase 0 still works.
    restored.pastimes.round.as_mut().unwrap().left = Game::Turnip.time();
    restored.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
    let round = restored.pastimes.round.as_ref().unwrap();
    assert_eq!(round.progress, 1, "restored game accepts a kick");
    assert_eq!(round.last_beat, Some(0));

    // The wire copy (live mirror) must match the host.
    let mirror = restored.wire_copy();
    assert_eq!(mirror.pastimes, restored.pastimes);
}
