//! The ossuary's module seam: a brain that fails, or answers outside the
//! host's envelope, leaves the tick to the native code with the Run exactly
//! as the native code alone would have left it.
use super::*;

fn in_ossuary(seats: u32) -> Run {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.enemies.clear();
    run.dungeon.rooms.push(nooks::room(RoomKind::Ossuary, (12, 12), 3));
    let index = run.dungeon.rooms.len() - 1;
    run.nook = Some(Nook { room: index, ..Default::default() });
    run.enter_for_test(index);
    run.enemies.clear();
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    run
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

/// A stalled line, then a full one: the same script for both runs.
fn script(run: &mut Run, tick: u32) {
    let press = tick % 90 == 1;
    let input = Input { interact: press, ..Default::default() };
    run.step(&BTreeMap::from([(1, input)]));
}

fn drive(brain: Option<OssuaryBrain>) -> (Vec<String>, BrainTally) {
    let had = install_ossuary_brain(brain);
    assert!(had.is_none());
    reset_ossuary_brain_tally();
    let mut run = in_ossuary(2);
    put(&mut run, 1, domino_at(0));
    put(&mut run, 2, (2.0, 2.0));
    let mut states = vec![];
    for tick in 0..300 {
        script(&mut run, tick);
        states.push(serde_json::to_string(&run).unwrap());
    }
    let tally = ossuary_brain_tally();
    install_ossuary_brain(None);
    (states, tally)
}

#[test]
fn no_brain_means_the_native_tick_and_an_empty_tally() {
    assert!(!ossuary_brain_installed());
    let (_, tally) = drive(None);
    assert_eq!(tally, BrainTally::default());
}

#[test]
fn a_brain_that_errors_leaves_every_tick_to_the_native_code() {
    let (native, _) = drive(None);
    let (fallen_back, tally) = drive(Some(Rc::new(|_| Err("module unavailable".into()))));
    assert_eq!(native, fallen_back);
    assert!(tally.fell_back > 50 && tally.applied == 0, "{tally:?}");
    assert_eq!(tally.last_error.as_deref(), Some("module unavailable"));
}

#[test]
fn an_answer_outside_the_host_envelope_is_not_applied() {
    let (native, _) = drive(None);
    let lies: [fn(&OssuaryFrame) -> OssuaryStep; 5] = [
        // Raises a monster it never asked for.
        |f| OssuaryStep { state: OssuaryState { raised: 1, ..f.state.clone() }, effects: vec![] },
        // Grants coin to a knight who is not in the room.
        |f| OssuaryStep { state: f.state.clone(), effects: vec![OssuaryEffect::Grant { id: 5, gold: 8 }] },
        // A grant beyond the envelope.
        |f| OssuaryStep { state: f.state.clone(), effects: vec![OssuaryEffect::Grant { id: 1, gold: MAX_GRANT + 1 }] },
        // A line longer than the line.
        |f| OssuaryStep { state: OssuaryState { fallen: 43, ..f.state.clone() }, effects: vec![] },
        // More effects than a tick may carry.
        |f| OssuaryStep { state: f.state.clone(), effects: vec![OssuaryEffect::Sound { cue: OssuaryCue::Toll }; MAX_EFFECTS + 1] },
    ];
    for lie in lies {
        let (fallen_back, tally) = drive(Some(Rc::new(move |f| Ok(lie(f)))));
        assert_eq!(native, fallen_back);
        assert!(tally.fell_back > 50 && tally.applied == 0, "{tally:?}");
    }
}

#[test]
fn an_honest_answer_is_applied_in_order_and_counted() {
    // A brain that only ever says "clatter" and keeps the state: the world
    // hears it, the nook does not move.
    let _ = install_ossuary_brain(Some(Rc::new(|f| {
        Ok(OssuaryStep { state: f.state.clone(), effects: vec![OssuaryEffect::Sound { cue: OssuaryCue::Clatter }] })
    })));
    let mut run = in_ossuary(1);
    run.sounds.clear();
    run.step(&BTreeMap::new());
    assert!(run.sounds.contains(&"bones_clatter"));
    assert!(!run.nook.as_ref().unwrap().running);
    assert!(ossuary_brain_tally().applied >= 1);
    install_ossuary_brain(None);
}
