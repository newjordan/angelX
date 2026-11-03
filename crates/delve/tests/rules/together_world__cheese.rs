// Cheese Chuck: throw cheese wheels at a moving basket target.
// Tests cover initialization, velocity integration, wall collisions,
// basket hits, misses, game completion, serialization, and validation.

/// Start a Cheese game with both players near the start so the heat
/// survives (if both are far from the game, the heat ends immediately).
fn start_cheese_game() -> Run {
    use super::super::pastimes::Game;
    let mut run = at_the_gate();
    run.arrive(
        room_of(&run.dungeon, Game::Cheese.room()).unwrap(),
        Game::Cheese.start(),
    );
    run.tick_pastimes(&BTreeMap::new());
    assert!(run.pastimes.round.is_some(), "cheese game should have started");
    run
}

#[test]
fn cheese_initializes_with_shots() {
    use super::super::pastimes::Game;
    let run = start_cheese_game();
    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.game, Game::Cheese);
    assert_eq!(round.shots, 0, "no shots taken before first throw");
    assert!(round.cheese.is_none(), "no cheese in flight initially");
    assert_eq!(round.progress, 0);
    assert_eq!(round.mistakes, 0);
    // The game clock starts at full time; the first decrement happens on
    // the next tick after the round is created.
    assert!(round.left <= Game::Cheese.time());
    assert_eq!(Game::Cheese.need(), 5, "need 5 hits to win");
}

#[test]
fn cheese_velocity_integration() {
    use super::super::pastimes::Cheese;
    let mut run = start_cheese_game();

    // Inject a cheese with known velocity in a clear area.
    {
        let round = run.pastimes.round.as_mut().unwrap();
        round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
            x: 10 * 256,
            y: 7 * 256,
            dx: 50,
            dy: -30,
            left: 2 * HZ,
            splat: 0,
            splat_x: 0,
            splat_y: 0,
        });
        round.shots = 1;
        round.progress = 0;
        round.mistakes = 0;
    }

    let (x_before, y_before, left_before) = {
        let c = run.pastimes.round.as_ref().unwrap().cheese.as_ref().unwrap();
        (c.x, c.y, c.left)
    };

    run.step(&BTreeMap::default());

    let c = run.pastimes.round.as_ref().unwrap().cheese.as_ref().unwrap();
    assert_eq!(c.x, x_before + 50, "x advances by dx each tick");
    assert_eq!(c.y, y_before - 30, "y advances by dy each tick");
    assert_eq!(c.left, left_before - 1, "flight timer counts down");
}

#[test]
fn cheese_wall_collision() {
    use super::super::pastimes::Cheese;
    let mut run = start_cheese_game();

    // Place cheese near the right edge with strong rightward velocity.
    // It takes a few ticks to reach the boundary; step until it is
    // removed (either by hitting a wall tile or going out of bounds).
    {
        let round = run.pastimes.round.as_mut().unwrap();
        round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
            x: 20 * 256,
            y: 7 * 256,
            dx: 108,
            dy: 0,
            left: 2 * HZ,
            splat: 0,
            splat_x: 0,
            splat_y: 0,
        });
        round.shots = 1;
        round.progress = 0;
        round.mistakes = 0;
    }

    for _ in 0..20 {
        if run.pastimes.round.as_ref().unwrap().cheese.is_none() {
            break;
        }
        run.step(&BTreeMap::default());
    }

    let round = run.pastimes.round.as_ref().unwrap();
    assert!(round.cheese.is_none(), "cheese removed after wall collision or OOB");
    assert_eq!(round.mistakes, 1, "wall collision counts as mistake");
    assert_eq!(round.progress, 0, "wall hit does not score");
}

#[test]
fn cheese_basket_hit() {
    use super::super::pastimes::Cheese;
    let mut run = start_cheese_game();

    // Compute where the basket will be when the cheese is processed.
    // The game loop decrements round.left before cheese processing, so
    // we compute the target with the post-decrement age.
    let (cheese_x, cheese_y, dx, dy) = {
        let round = run.pastimes.round.as_mut().unwrap();
        round.left -= 1;
        let target = super::super::pastimes::cheese_target(round);
        round.left += 1;
        let dx = 80i32;
        let dy = 20i32;
        (
            (target.0 * 256.0).round() as i32 - dx,
            (target.1 * 256.0).round() as i32 - dy,
            dx,
            dy,
        )
    };

    {
        let round = run.pastimes.round.as_mut().unwrap();
        round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
            x: cheese_x,
            y: cheese_y,
            dx,
            dy,
            left: 2 * HZ,
            splat: 0,
            splat_x: 0,
            splat_y: 0,
        });
        round.shots = 1;
        round.progress = 0;
        round.mistakes = 0;
    }

    run.step(&BTreeMap::default());

    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.progress, 1, "basket hit advances progress");
    assert_eq!(round.mistakes, 0, "basket hit does not add mistakes");
    assert!(round.cheese.is_none(), "cheese consumed after basket hit");
}

#[test]
fn cheese_basket_miss() {
    use super::super::pastimes::Cheese;
    let mut run = start_cheese_game();

    // Send cheese hard to the left, away from the basket (which sits
    // around x=15-19). It takes a few ticks to leave the room; step
    // until the cheese is removed by the validity check.
    {
        let round = run.pastimes.round.as_mut().unwrap();
        round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
            x: 3 * 256,
            y: 6 * 256,
            dx: -108,
            dy: 0,
            left: 2 * HZ,
            splat: 0,
            splat_x: 0,
            splat_y: 0,
        });
        round.shots = 1;
        round.progress = 0;
        round.mistakes = 0;
    }

    for _ in 0..20 {
        if run.pastimes.round.as_ref().unwrap().cheese.is_none() {
            break;
        }
        run.step(&BTreeMap::default());
    }

    let round = run.pastimes.round.as_ref().unwrap();
    assert!(round.cheese.is_none(), "missed cheese is removed");
    assert_eq!(round.mistakes, 1, "missing the basket counts as mistake");
    assert_eq!(round.progress, 0, "miss does not advance progress");
}

#[test]
fn cheese_all_shots_used() {
    let mut run = start_cheese_game();

    // Set up a state where all 12 shots have been used (5 hits + 7 misses)
    // and no cheese is in flight. The game must end.
    {
        let round = run.pastimes.round.as_mut().unwrap();
        round.shots = 12;
        round.progress = 5;
        round.mistakes = 7;
        round.cheese = None;
    }

    run.step(&BTreeMap::default());
    assert!(
        run.pastimes.round.is_none(),
        "game ends after all 12 shots with no cheese in flight"
    );
}

#[test]
fn cheese_serialization_roundtrip() {
    use super::super::pastimes::Cheese;

    // Roundtrip the Cheese struct alone.
    let c = Cheese {
                    prev_x: 0,
                    prev_y: 0,
        x: 1234,
        y: -567,
        dx: 108,
        dy: -50,
        left: 42,
        splat: 0,
        splat_x: 0,
        splat_y: 0,
    };
    let serialized = serde_json::to_vec(&c).unwrap();
    let deserialized: Cheese = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(c, deserialized);

    // Roundtrip through a full Run with an active cheese round.
    let mut run = start_cheese_game();
    {
        let round = run.pastimes.round.as_mut().unwrap();
        round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
            x: 5 * 256,
            y: 6 * 256,
            dx: 90,
            dy: -10,
            left: 30,
            splat: 0,
            splat_x: 0,
            splat_y: 0,
        });
        round.shots = 3;
        round.progress = 1;
        round.mistakes = 1;
    }
    let wire = serde_json::to_vec(&run).unwrap();
    let restored: Run = serde_json::from_slice(&wire).unwrap();
    assert!(restored.valid_snapshot());
    assert_eq!(restored.pastimes, run.pastimes);
}

#[test]
fn cheese_valid_rejects_bad_state() {
    use super::super::pastimes::Cheese;

    // Start a valid cheese game, then corrupt one field at a time.
    let fresh = |field: &str| {
        let mut run = start_cheese_game();
        let round = run.pastimes.round.as_mut().unwrap();
        round.shots = 2;
        round.progress = 1;
        round.mistakes = 0;
        match field {
            "neg_x" => {
                round.cheese = Some(Cheese { prev_x: 0, prev_y: 0, x: -1, y: 5 * 256, dx: 50, dy: 0, left: 30, splat: 0, splat_x: 0, splat_y: 0 });
            }
            "over_x" => {
                round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
                    x: 24 * 256,
                    y: 5 * 256,
                    dx: 50,
                    dy: 0,
                    left: 30,
                    splat: 0,
                    splat_x: 0,
                    splat_y: 0,
                });
            }
            "neg_y" => {
                round.cheese = Some(Cheese { prev_x: 0, prev_y: 0, x: 5 * 256, y: -10, dx: 50, dy: 0, left: 30, splat: 0, splat_x: 0, splat_y: 0 });
            }
            "zero_vel" => {
                round.cheese = Some(Cheese { prev_x: 0, prev_y: 0, x: 5 * 256, y: 5 * 256, dx: 0, dy: 0, left: 30, splat: 0, splat_x: 0, splat_y: 0 });
            }
            "zero_left" => {
                round.cheese = Some(Cheese { prev_x: 0, prev_y: 0, x: 5 * 256, y: 5 * 256, dx: 50, dy: 0, left: 0, splat: 0, splat_x: 0, splat_y: 0 });
            }
            "over_left" => {
                round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
                    x: 5 * 256,
                    y: 5 * 256,
                    dx: 50,
                    dy: 0,
                    left: 2 * HZ + 1,
                    splat: 0,
                    splat_x: 0,
                    splat_y: 0,
                });
            }
            "over_dx" => {
                round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
                    x: 5 * 256,
                    y: 5 * 256,
                    dx: 109,
                    dy: 0,
                    left: 30,
                    splat: 0,
                    splat_x: 0,
                    splat_y: 0,
                });
            }
            "bad_accounting" => {
                // progress(2) + mistakes(1) + cheese(1) = 4 != shots(5)
                round.shots = 5;
                round.progress = 2;
                round.mistakes = 1;
                round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
                    x: 5 * 256,
                    y: 5 * 256,
                    dx: 50,
                    dy: 0,
                    left: 30,
                    splat: 0,
                    splat_x: 0,
                    splat_y: 0,
                });
            }
            _ => unreachable!(),
        }
        run
    };

    assert!(!fresh("neg_x").pastimes.valid(), "negative x rejected");
    assert!(!fresh("over_x").pastimes.valid(), "x beyond room rejected");
    assert!(!fresh("neg_y").pastimes.valid(), "negative y rejected");
    assert!(!fresh("zero_vel").pastimes.valid(), "zero velocity rejected");
    assert!(!fresh("zero_left").pastimes.valid(), "zero left rejected");
    assert!(!fresh("over_left").pastimes.valid(), "left beyond 2*HZ rejected");
    assert!(!fresh("over_dx").pastimes.valid(), "dx beyond 108 rejected");
    assert!(
        !fresh("bad_accounting").pastimes.valid(),
        "inconsistent shot accounting rejected"
    );

    // A valid cheese state passes.
    let mut run = start_cheese_game();
    {
        let round = run.pastimes.round.as_mut().unwrap();
        round.shots = 2;
        round.progress = 1;
        round.mistakes = 0;
        round.cheese = Some(Cheese {
                    prev_x: 0,
                    prev_y: 0,
            x: 5 * 256,
            y: 5 * 256,
            dx: 50,
            dy: -30,
            left: 30,
            splat: 0,
            splat_x: 0,
            splat_y: 0,
        });
    }
    assert!(run.pastimes.valid(), "valid cheese state accepted");
}
