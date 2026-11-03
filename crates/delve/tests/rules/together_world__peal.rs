// Change Ringing: pull the bell rope exactly as the swing turns gold.
// Tests cover the published-cue timing, a full winning peal, clangers and
// misses ending it, validation, and a two-knight heat.

/// A run standing on the Peal pad in the Belfry, round live.
fn peal_run() -> Run {
    use super::super::pastimes::Game;
    let mut run = at_the_gate();
    // A lone ringer: without this the gate's friend is seated too and
    // every finish is judged as a heat instead of a solo round.
    run.players.retain(|&id, _| id == 1);
    let room = room_of(&run.dungeon, RoomKind::Belfry).unwrap();
    run.arrive(room, Game::Peal.start());
    run.step(&BTreeMap::default());
    assert!(run.pastimes.round.as_ref().is_some_and(|r| r.game == Game::Peal));
    run
}

#[test]
fn peal_accepts_only_the_published_gold() {
    use super::super::pastimes::{PEAL_WINDOW, Peal};
    for struck in [0u8, 6, 12] {
        for phase in 0..super::super::pastimes::peal_interval(struck) {
            let mut peal = Peal::new(1);
            peal.struck = struck;
            peal.phase = phase;
            let span = peal.span();
            let closed = peal.step(true);
            let rung = peal.rung || closed == Some(true);
            assert_eq!(
                rung,
                phase + PEAL_WINDOW >= span,
                "tap against published phase {phase}/{span} of strike {struck}"
            );
            match closed {
                // Still swinging: an early tap has spoiled this strike.
                None => assert_eq!(peal.spoiled, !rung, "strike {struck} at phase {phase}"),
                // Closed on this very tick: the verdict is the result.
                Some(hit) => assert_eq!(hit, rung, "strike {struck} at phase {phase}"),
            }
        }
    }
}

#[test]
fn change_ringing_rings_a_full_peal_and_pays_once() {
    use super::super::pastimes::Game;
    let mut run = peal_run();
    let treasury = run.treasury.clone();
    let score = run.score;
    let mut fired = false;
    for _ in 0..Game::Peal.time() {
        let Some(round) = run.pastimes.round.as_ref() else { break };
        let peal = round.peal.as_ref().unwrap();
        // Pull on the published gold, releasing between pulls so each is
        // a fresh tap, exactly as a ringer at the rope would.
        let pull = peal.in_window() && !fired;
        fired = pull;
        run.step(&BTreeMap::from([(1, Input { fire: pull, ..Default::default() })]));
        assert!(run.valid_snapshot());
    }
    assert!(run.pastimes.round.is_none(), "the peal ends");
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("CHANGE RINGING complete!"), "{said}");
    let records = run.home.countryside.games.peal.clone();
    assert_eq!((records.plays, records.wins), (1, 1));
    assert!(records.best > 0);
    assert_eq!(run.score, score + u32::from(records.best));
    assert_eq!(run.treasury, treasury, "practice does not create treasury gold");
    for _ in 0..HZ {
        run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
    }
    assert_eq!(run.home.countryside.games.peal, records, "the win pays once");
    assert_eq!(run.score, score + u32::from(records.best));
}

#[test]
fn change_ringing_clangers_fell_the_peal() {
    use super::super::pastimes::Game;
    let mut run = peal_run();
    let mut fired = false;
    for _ in 0..Game::Peal.time() {
        let Some(round) = run.pastimes.round.as_ref() else { break };
        let peal = round.peal.as_ref().unwrap();
        // Pull at the very start of every swing: always a clanger.
        let pull = peal.phase == 0 && !fired;
        fired = pull;
        run.step(&BTreeMap::from([(1, Input { fire: pull, ..Default::default() })]));
    }
    assert!(run.pastimes.round.is_none(), "four clangers end the peal");
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("fell apart"), "{said}");
    let records = &run.home.countryside.games.peal;
    assert_eq!((records.plays, records.wins), (1, 0));
}

#[test]
fn change_ringing_missed_strikes_fell_the_peal() {
    use super::super::pastimes::Game;
    let mut run = peal_run();
    for _ in 0..Game::Peal.time() {
        if run.pastimes.round.is_none() {
            break;
        }
        run.step(&BTreeMap::default());
        assert!(run.valid_snapshot());
    }
    assert!(run.pastimes.round.is_none(), "four missed strikes end the peal");
    assert_eq!(run.home.countryside.games.peal.wins, 0);
}

#[test]
fn peal_valid_rejects_a_broken_peal_and_records_repair() {
    let mut run = peal_run();
    {
        let round = run.pastimes.round.as_ref().unwrap();
        assert!(round.peal.as_ref().unwrap().valid(round), "a fresh peal is valid");
    }
    let round = run.pastimes.round.as_mut().unwrap();
    round.peal.as_mut().unwrap().rung = true;
    round.peal.as_mut().unwrap().spoiled = true;
    let round = run.pastimes.round.as_ref().unwrap();
    assert!(!round.peal.as_ref().unwrap().valid(round), "rung and spoiled cannot both hold");

    let mut run = peal_run();
    let round = run.pastimes.round.as_mut().unwrap();
    round.peal.as_mut().unwrap().phase = round.peal.as_ref().unwrap().span();
    let round = run.pastimes.round.as_ref().unwrap();
    assert!(!round.peal.as_ref().unwrap().valid(round), "a phase past the beat never closed");

    let mut records = super::super::pastimes::Records::default();
    records.peal.best = 5000;
    records.peal.wins = 3;
    assert!(!records.valid());
    records.repair();
    assert!(records.valid());
    assert_eq!(records.peal.best, 1000);
    assert_eq!(records.peal.plays, 3, "plays rise to meet wins");
}

/// The frames, to look at:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_peal_shots -- --ignored`
#[test]
#[ignore]
fn write_peal_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("peal");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |r: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(r, 768, 448, Some(1));
        image::RgbaImage::from_raw(768, 448, img.rgba_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    // The belfry at rest: both games' boards and pads.
    let mut run = at_the_gate();
    run.players.retain(|&id, _| id == 1);
    let room = room_of(&run.dungeon, RoomKind::Belfry).unwrap();
    run.arrive(room, (12.0, 11.0));
    run.step(&BTreeMap::default());
    save(&run, "1-belfry-idle.png");
    // A swing coming round, then the gold, then the pull.
    let mut run = peal_run();
    while run.pastimes.round.as_ref().unwrap().peal.as_ref().unwrap().phase < 10 {
        run.step(&BTreeMap::default());
    }
    save(&run, "2-swing.png");
    while !run.pastimes.round.as_ref().unwrap().peal.as_ref().unwrap().in_window() {
        run.step(&BTreeMap::default());
    }
    save(&run, "3-gold.png");
    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
    save(&run, "4-rung.png");
    // A clanger: pull far too soon on the next swing.
    while run.pastimes.round.as_ref().unwrap().peal.as_ref().unwrap().struck == 0 {
        run.step(&BTreeMap::default());
    }
    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
    save(&run, "5-clanger.png");
}

#[test]
fn two_knights_ring_their_own_peals_and_the_first_home_is_crowned() {
    use super::super::pastimes::Game;
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.join_seat(2, "Seat 2");
    let room = room_of(&run.dungeon, RoomKind::Belfry).unwrap();
    run.arrive(room, (12.0, 11.0));
    run.tick_pastimes(&BTreeMap::new());
    assert!(run.pastimes.heat.is_none());
    for id in [1, 2] {
        let h = run.players.get_mut(&id).unwrap();
        (h.x, h.y) = (Game::Peal.start().0 * TILE_UNITS, Game::Peal.start().1 * TILE_UNITS);
    }
    run.tick_pastimes(&BTreeMap::new());
    let heat = run.pastimes.heat.clone().expect("a heat begins");
    assert_eq!(heat.seats, vec![1, 2], "both knights in the belfry ring");
    assert_eq!(run.pastimes.heats.len(), 1);
    // Seat two rings on its own published gold; seat one never pulls.
    // Seat one's peal falls apart on misses while seat two rings on:
    // each seat's outcome is its own.
    let mut fired = false;
    for _ in 0..Game::Peal.time() * 2 {
        if run.pastimes.heat.is_none() {
            break;
        }
        let pull = run
            .pastimes
            .heats
            .get(&2)
            .and_then(|r| r.peal.as_ref())
            .is_some_and(|p| p.in_window())
            && !fired;
        fired = pull;
        run.tick_pastimes(&BTreeMap::from([(2, Input { fire: pull, ..Default::default() })]));
        assert!(run.valid_snapshot());
    }
    assert!(run.pastimes.heat.is_none() && run.pastimes.round.is_none());
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("WINS THE CHANGE RINGING"), "{said}");
    assert!(run.players[&2].laurel, "the ringer home first wears the laurel");
    assert_eq!(run.players[&1].booby, Some(Game::Peal), "the other wears the bell");
    assert_eq!(run.home.countryside.games.peal.wins, 1);
}
