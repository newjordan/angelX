// Trencher Toss: launch a plate, steer the goose to catch it.
// Tests cover initialization, validation, plate flight, catching, and game completion.

/// A run standing on the Trencher pad in GooseGreen, round live.
fn trencher_run() -> Run {
    use super::super::pastimes::Game;
    let mut run = at_the_gate();
    let room = room_of(&run.dungeon, RoomKind::GooseGreen).unwrap();
    run.arrive(room, Game::Trencher.start());
    run.step(&BTreeMap::default());
    assert!(run.pastimes.round.as_ref().is_some_and(|r| r.game == Game::Trencher));
    run
}

/// Toss one plate, fly it to its final tick, park the goose `offset`
/// from its landing, and step the tick it comes down.
fn trencher_land_one(offset: i32) -> Run {
    let mut run = trencher_run();
    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
    while run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().flight.as_ref().unwrap().left > 1 {
        run.step(&BTreeMap::default());
    }
    let landing = run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().flight.as_ref().unwrap().landing;
    run.pastimes.round.as_mut().unwrap().trencher.as_mut().unwrap().goose = landing + offset;
    run.step(&BTreeMap::default());
    run
}

#[test]
fn trencher_initializes_with_owner_and_default_state() {
    use super::super::pastimes::Trencher;
    let t = Trencher::new(2);
    assert_eq!(t.owner, 2);
    assert_eq!(t.goose, 12 * 256);
    assert!(t.flight.is_none());
    assert_eq!(t.wait, 0);
    assert_eq!(t.celebrate, 0);
    assert_eq!(t.sulk, 0);
}

#[test]
fn trencher_plate_flies_after_launch() {
    let mut run = trencher_run();
    let round = run.pastimes.round.as_ref().unwrap();
    assert!(round.trencher.as_ref().unwrap().flight.is_none());

    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));

    let round = run.pastimes.round.as_ref().unwrap();
    let plate = round.trencher.as_ref().unwrap().flight.as_ref().unwrap();
    assert!(plate.left > 0, "plate should have flight time remaining");
}

#[test]
fn trencher_catch_advances_progress() {
    let run = trencher_land_one(0);
    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.progress, 1, "catch advances progress");
    assert_eq!(round.mistakes, 0, "catch doesn't add mistakes");
}

#[test]
fn trencher_miss_adds_mistake() {
    let run = trencher_land_one(500);
    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.mistakes, 1, "miss adds mistake");
    assert_eq!(round.progress, 0);
}

#[test]
fn trencher_three_mistakes_ends_game() {
    let mut run = trencher_run();
    for i in 0..3 {
        // Wait out the between-plates pause, then toss and whiff again.
        while run.pastimes.round.as_ref().is_some_and(|r| r.trencher.as_ref().unwrap().wait > 0) {
            run.step(&BTreeMap::default());
        }
        run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
        while run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().flight.as_ref().unwrap().left > 1 {
            run.step(&BTreeMap::default());
        }
        let landing = run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().flight.as_ref().unwrap().landing;
        run.pastimes.round.as_mut().unwrap().trencher.as_mut().unwrap().goose = landing + 500;
        run.step(&BTreeMap::default());
        if i < 2 {
            assert_eq!(run.pastimes.round.as_ref().unwrap().mistakes, i + 1, "mistake count increments");
        }
    }
    assert!(run.pastimes.round.is_none(), "three mistakes end the round");
}

#[test]
fn trencher_goose_steering_works() {
    let mut run = trencher_run();
    let initial_goose = run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().goose;

    run.step(&BTreeMap::from([(1, Input { move_x: -1, ..Default::default() })]));
    let left_goose = run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().goose;
    assert!(left_goose < initial_goose, "steering left moves goose left");

    run.step(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]));
    let right_goose = run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap().goose;
    assert!(right_goose > left_goose, "steering right moves goose right");
}

#[test]
fn trencher_valid_accepts_fresh_state() {
    let run = trencher_run();
    let round = run.pastimes.round.as_ref().unwrap();
    let t = round.trencher.as_ref().unwrap();
    assert!(t.valid(round), "fresh trencher should be valid");
}

#[test]
fn trencher_serialization_roundtrip() {
    use super::super::pastimes::Trencher;
    let t = Trencher::new(1);
    let serialized = serde_json::to_vec(&t).unwrap();
    let deserialized: Trencher = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(t, deserialized);
}

#[test]
fn trencher_catch_celebrates_then_settles() {
    let mut run = trencher_land_one(0);
    let t = run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap();
    assert!(t.celebrate > 0, "a catch starts the goose's hop");
    assert_eq!(t.sulk, 0, "a catch is no occasion to sulk");
    for _ in 0..super::super::pastimes::TRENCHER_FLIGHT {
        run.step(&BTreeMap::default());
    }
    let t = run.pastimes.round.as_ref().unwrap().trencher.as_ref().unwrap();
    assert_eq!(t.celebrate, 0, "the hop winds down on its own");
}

#[test]
fn trencher_far_miss_sulks_without_popup() {
    let run = trencher_land_one(2000);
    let round = run.pastimes.round.as_ref().unwrap();
    let t = round.trencher.as_ref().unwrap();
    assert!(t.sulk > 0, "a miss starts the droop");
    assert_eq!(t.celebrate, 0);
    assert_ne!(round.score_popup_text, "SO CLOSE!", "a mile-wide miss earns no gasp");
}

#[test]
fn trencher_near_miss_earns_the_gasp() {
    // Just outside the catch reach: still a mistake, but a dramatic one.
    let run = trencher_land_one(200);
    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.mistakes, 1, "a near miss still counts as a miss");
    assert_eq!(round.score_popup_text, "SO CLOSE!");
    assert!(round.trencher.as_ref().unwrap().sulk > 0);
}

#[test]
fn trencher_old_checkpoint_without_moods_still_reads() {
    use super::super::pastimes::Trencher;
    let old: Trencher = serde_json::from_slice(br#"{"owner":1,"goose":3072,"flight":null,"wait":0}"#).unwrap();
    assert_eq!(old.celebrate, 0);
    assert_eq!(old.sulk, 0);
}

#[test]
fn trencher_valid_rejects_impossible_moods() {
    let mut run = trencher_run();
    let round = run.pastimes.round.as_mut().unwrap();
    round.trencher.as_mut().unwrap().celebrate = u8::MAX;
    let round = run.pastimes.round.as_ref().unwrap();
    assert!(!round.trencher.as_ref().unwrap().valid(round), "an endless hop is not a legal state");
}
