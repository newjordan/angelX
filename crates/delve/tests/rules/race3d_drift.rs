use super::*;

fn field() -> Run {
    let mut run = (0..80)
        .find_map(|seed| {
            let mut r = Run::new(seed, 1, None);
            for _ in 0..6 {
                r.descend_for_test();
                if Course::of_floor(r.dungeon.depth) == Course::Touge && r.race3d_room.is_some() {
                    return Some(r);
                }
            }
            None
        })
        .unwrap();
    for id in 2..=4 {
        run.join_seat(id, &format!("Racer {id}"));
    }
    for h in run.players.values_mut() {
        h.carried.add(Spoil::Gold, 100);
    }
    run.enemies.clear();
    run.enter_for_test(run.race3d_room.unwrap());
    run.race3d.as_mut().unwrap().t = TITLE;
    run
}
fn step_bots(run: &mut Run) {
    let r = run.race3d.as_ref().unwrap();
    let inputs = r.riders.keys().map(|&id| (id, drift_bot(r, id))).collect();
    run.step(&inputs);
}

#[test]
fn drift_grip_charge_release_and_spin_out() {
    let run = field();
    let race = run.race3d.as_ref().unwrap();
    let mut grip = Rider::new(20.0, 0.0, 1);
    grip.vs = 14.0;
    let mut slide = grip.clone();
    let mut noise = Noise::default();
    for _ in 0..12 {
        drive(
            race,
            &mut grip,
            Input {
                move_x: 1,
                move_y: -1,
                ..Input::default()
            },
            &mut noise,
        );
        drive(
            race,
            &mut slide,
            Input {
                move_x: 1,
                move_y: -1,
                dash: true,
                ..Input::default()
            },
            &mut noise,
        );
    }
    assert!(slide.drift.angle().abs() > grip.drift.angle().abs() * 2.0);
    assert!(slide.drift.charge > 0);
    let edge = DRIFT_WIDTH * 0.5 - 0.65;
    slide.x = edge - 0.001;
    slide.drift.travel = 0.8;
    drive(
        race,
        &mut slide,
        Input {
            move_x: 1,
            dash: true,
            ..Input::default()
        },
        &mut noise,
    );
    assert_eq!(slide.wreck, Wreck::SpinOut);
    assert_eq!(slide.drift.charge, 0);
    for _ in 0..SPIN_TICKS {
        drive(race, &mut slide, Input::default(), &mut noise);
    }
    assert!(slide.racing());
    slide.drift.heading = 1.4;
    slide.drift.travel = 0.0;
    slide.drift.sliding = true;
    slide.vs = 14.0;
    drive(
        race,
        &mut slide,
        Input {
            move_x: 1,
            dash: true,
            ..Input::default()
        },
        &mut noise,
    );
    assert_eq!(
        slide.wreck,
        Wreck::SpinOut,
        "excess angle spins without a wall"
    );
}

#[test]
fn all_three_boosts_have_exact_thresholds_sizes_and_durations() {
    let run = field();
    let race = run.race3d.as_ref().unwrap();
    for (i, &threshold) in CHARGE_TICKS.iter().enumerate() {
        let mut r = Rider::new(20.0, 0.0, 1);
        r.vs = 12.0;
        r.drift.charge = threshold - 1;
        assert_eq!(r.drift.stage(), i);
        r.drift.charge = threshold;
        assert_eq!(r.drift.stage(), i + 1);
        r.drift.sliding = true;
        r.drift.corner = 123;
        drive(
            race,
            &mut r,
            Input {
                move_y: -1,
                ..Input::default()
            },
            &mut Noise::default(),
        );
        assert_eq!(r.drift.boost, BOOST_TICKS[i] - 1);
        assert!((r.vs - (12.0 + BOOST_SPEED[i] + DRIFT_ACCEL * DT)).abs() < 0.001);
        assert_eq!(r.drift.last_corner, 123);
        assert_eq!(r.drift.score, 123);
        assert_eq!(r.drift.charge, 0);
    }
}

#[test]
fn touge_is_closed_and_three_laps_rank_total_progress() {
    let t = track(Course::Touge);
    let a = t.world(0.0, 0.0, 0.0);
    let b = t.world(t.len() - 0.001, 0.0, 0.0);
    assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.01));
    let mut run = field();
    let race = run.race3d.as_mut().unwrap();
    race.riders.get_mut(&1).unwrap().s = t.finish + 10.0;
    race.riders.get_mut(&2).unwrap().s = t.finish - 1.0;
    assert_eq!(race.lap(1), 2);
    assert_eq!(race.place(1), 1);
    let finish = race.finish_distance();
    for r in race.riders.values_mut() {
        r.s = finish - 0.01;
        r.vs = 14.0;
    }
    step_bots(&mut run);
    let race = run.race3d.as_ref().unwrap();
    assert!(race.done && race.riders.values().all(|r| r.finish.is_some()));
    assert_eq!(race.order(), vec![1, 2, 3, 4], "same tick ties use seat id");
    let gold = run.players[&1].carried.get(Spoil::Gold);
    step_bots(&mut run);
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), gold, "pays once");
}

#[test]
fn four_drift_bots_complete_and_replay_byte_for_byte() {
    let mut a = field();
    let mut b = a.clone();
    let mut stages = [0u32; 3];
    let mut first_laps = [None; 4];
    loop {
        step_bots(&mut a);
        step_bots(&mut b);
        let race = a.race3d.as_ref().unwrap();
        for (&id, r) in &race.riders {
            if r.s >= track(Course::Touge).finish && first_laps[id as usize - 1].is_none() {
                first_laps[id as usize - 1] = Some((race.t - TITLE) / HZ);
            }
            if r.drift.stage() > 0 {
                stages[r.drift.stage() - 1] += 1;
            }
        }
        assert!(
            a.valid_snapshot(),
            "tick {} race valid {}: {:?}",
            a.tick,
            race.valid(),
            race
        );
        assert_eq!(
            serde_json::to_vec(&a.race3d).unwrap(),
            serde_json::to_vec(&b.race3d).unwrap()
        );
        if race.done {
            break;
        }
    }
    let race = a.race3d.as_ref().unwrap();
    eprintln!(
        "Touge laps {first_laps:?}, stages {stages:?}, finishes {:?}",
        race.riders
            .iter()
            .map(|(id, r)| (
                id,
                r.finish.map(|t| (t - TITLE) / HZ),
                r.crashes,
                r.drift.score
            ))
            .collect::<Vec<_>>()
    );
    assert!(
        race.riders.values().all(|r| r.finish.is_some()),
        "bots finish before the race clock"
    );
    assert!(first_laps.iter().all(|t| (60..=90).contains(&t.unwrap())));
    assert!(stages.iter().all(|&n| n > 0));
    assert!(
        race.riders
            .values()
            .all(|r| r.drift.score > 0 && r.crashes < 12)
    );
}

#[test]
fn leaving_the_hall_releases_every_seat_and_closes_the_race() {
    let mut run = field();
    run.floor_plates.clear();
    let hall = run.at;
    for id in 1..=4 {
        assert!(run.can_leave_activity(id));
        assert!(run.leave_activity(id));
    }
    assert_ne!(run.at, hall);
    assert!(!run.race3d_showing());
    assert!(run.valid_snapshot());
}

#[test]
fn throttle_brake_and_bad_drift_snapshot_are_checked() {
    let run = field();
    let race = run.race3d.as_ref().unwrap();
    let mut r = Rider::new(20.0, 0.0, 1);
    let mut noise = Noise::default();
    for _ in 0..HZ {
        drive(
            race,
            &mut r,
            Input {
                move_y: -1,
                ..Input::default()
            },
            &mut noise,
        );
    }
    assert!(r.vs > 5.0);
    for _ in 0..HZ {
        drive(
            race,
            &mut r,
            Input {
                move_y: 1,
                ..Input::default()
            },
            &mut noise,
        );
    }
    assert_eq!(r.vs, 0.0);
    let mut bad = race.clone();
    bad.riders.get_mut(&1).unwrap().drift.travel = f32::NAN;
    assert!(!bad.valid());
}

#[test]
fn hippodrome_entry_ticks_the_shared_racer_and_disconnected_seats_depart() {
    let mut run = Run::at_home(7, 1, None, home::Home::default(), Spoils::default());
    for id in 2..=4 {
        run.join_seat(id, &format!("Racer {id}"));
    }
    let room = world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap();
    run.arrive(room, (12.0, 11.0));
    run.floor_plates.clear();
    run.board_drift();
    assert_eq!(run.race3d.as_ref().unwrap().riders.len(), 4);
    run.race3d.as_mut().unwrap().t = TITLE;
    for _ in 0..HZ {
        step_bots(&mut run);
    }
    assert!(
        run.race3d
            .as_ref()
            .unwrap()
            .riders
            .values()
            .all(|r| r.s > 1.0)
    );
    run.leave(4);
    assert!(!run.race3d.as_ref().unwrap().riders.contains_key(&4));
    assert!(run.valid_snapshot());
}

#[test]
fn space_and_e_at_the_coach_board_before_a_dodge_leaves_the_plate() {
    let mut run = Run::at_home(7, 1, None, home::Home::default(), Spoils::default());
    run.join_seat(2, "Two");
    let room = world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap();
    let (c, r, _, _) = hippodrome::COACH;
    run.arrive(room, (c as f32 + 0.5, r as f32 + 0.5));
    // Arrival spreads the party sideways; the driver walks onto the plate.
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + 0.5) * TILE_UNITS;
    hero.y = (r as f32 + 0.5) * TILE_UNITS;
    run.floor_plates.clear();
    let pos = (run.players[&1].x, run.players[&1].y);
    run.step(&BTreeMap::from([(
        1,
        Input {
            dash: true,
            interact: true,
            ..Input::default()
        },
    )]));
    assert_eq!(run.race3d.as_ref().unwrap().course, Course::Touge);
    assert_eq!(run.race3d.as_ref().unwrap().riders.len(), 2);
    assert!(run.chariot.is_none());
    assert_eq!((run.players[&1].x, run.players[&1].y), pos);
}
