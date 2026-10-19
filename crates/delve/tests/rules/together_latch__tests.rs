use super::{BTreeMap, HZ, Input, Run, Spoil};
use crate::rules::{latch::*, tune::Tune};
fn game(n: u32) -> Run {
    let mut r = Run::latch_for_test();
    if n == 1 {
        r.leave(2);
    }
    for id in 2..=n {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(r.latch.is_some());
    r
}
fn tell(ticks: u32, height: i8) -> Duel {
    let mut d = Duel::new(41);
    d.beat = Beat::Tell;
    d.ticks = ticks;
    d.height = height;
    d
}
#[test]
fn latch_counter_window_has_exact_edges_and_needs_height_and_a_fresh_press() {
    let t = Tune::default();
    for remaining in [1, t.latch_window] {
        for height in [-1, 1] {
            let mut d = tell(remaining, height);
            assert_eq!(
                d.step(
                    Input {
                        fire: true,
                        move_y: height,
                        ..Default::default()
                    },
                    &t
                ),
                Some("clang")
            );
            assert_eq!(d.counters, 1);
            assert_eq!(d.freeze, t.latch_stop);
            assert_eq!(d.error, remaining - 1);
            let frozen = d.ticks;
            for _ in 0..t.latch_stop {
                d.step(Input::default(), &t);
                assert_eq!(d.ticks, frozen);
            }
            assert!(d.valid());
        }
    }
    for (remaining, guard) in [(t.latch_window + 1, -1), (1, 1)] {
        let mut d = tell(remaining, -1);
        d.step(
            Input {
                fire: true,
                move_y: guard,
                ..Default::default()
            },
            &t,
        );
        assert_eq!(d.beat, Beat::Miss);
        assert_eq!(d.counters, 0);
        assert_eq!(d.misses, 1);
        for _ in 0..t.latch_recover {
            d.step(Input::default(), &t);
        }
        assert_eq!(d.beat, Beat::Tell);
        assert!(t.latch_recover < HZ);
    }
    let mut d = tell(t.latch_window + 1, -1);
    d.fire_key = true;
    for _ in 0..t.latch_window + 1 {
        d.step(
            Input {
                fire: true,
                ..Default::default()
            },
            &t,
        );
    }
    assert_eq!(d.beat, Beat::Miss);
    assert_eq!(d.counters, 0, "holding F cannot catch a new window");
}
#[test]
fn latch_every_seat_has_an_independent_guard_counter_and_wire_state() {
    let mut r = game(4);
    let s = r.latch.as_mut().unwrap();
    let seed = s.seed;
    for d in s.duels.values_mut() {
        *d = tell(1, -1);
        d.seed = seed;
    }
    r.step(&BTreeMap::from([
        (
            1,
            Input {
                fire: true,
                move_y: -1,
                ..Default::default()
            },
        ),
        (
            2,
            Input {
                fire: true,
                move_y: 1,
                ..Default::default()
            },
        ),
        (
            3,
            Input {
                fire: true,
                move_y: -1,
                ..Default::default()
            },
        ),
    ]));
    let s = r.latch.as_ref().unwrap();
    assert_eq!(s.duels[&1].counters, 1);
    assert_eq!(s.duels[&2].misses, 1);
    assert_eq!(s.duels[&3].counters, 1);
    assert_eq!(s.duels[&4].misses, 1);
    assert_eq!(r.wire_copy().latch, r.latch);
    assert!(r.valid_snapshot());
    assert_ne!(
        crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1)),
        crate::render::overworld::arena::frame_for(&r, 384, 224, Some(2))
    );
}
#[test]
fn latch_five_rounds_seat_counts_pay_once_rematch_and_crown_carries_down() {
    for seats in [1, 2, 4, 2, 4] {
        let mut r = game(seats);
        for _ in 0..LENGTH {
            let s = r.latch.as_ref().unwrap();
            if s.left == 0 {
                break;
            }
            let inputs = s
                .duels
                .iter()
                .map(|(&id, d)| {
                    (
                        id,
                        if id == seats {
                            d.bot_input(1)
                        } else {
                            Input::default()
                        },
                    )
                })
                .collect();
            r.step(&inputs);
            assert!(r.latch.as_ref().unwrap().valid());
        }
        let s = r.latch.as_ref().unwrap();
        assert!(s.won);
        assert_eq!(s.order[0], seats);
        assert!(s.left == 0);
        let gold = r.players[&seats].carried.get(Spoil::Gold);
        assert_eq!(gold, GOLD_PER_SEAT * seats);
        assert!(r.players[&seats].laurel);
        assert_eq!(r.home.countryside.games.latch.wins, 1);
        for _ in 0..40 {
            r.step(&BTreeMap::new());
        }
        assert_eq!(r.players[&seats].carried.get(Spoil::Gold), gold);
        let seed = r.latch.as_ref().unwrap().seed;
        r.step(&BTreeMap::from([(
            seats,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        assert_eq!(r.latch.as_ref().unwrap().left, LENGTH);
        assert_ne!(r.latch.as_ref().unwrap().seed, seed);
        let bombs = r.players[&seats].bombs;
        r.kit_out_for_test();
        assert_eq!(
            r.players[&seats].bombs,
            (bombs + 1).min(crate::rules::MAX_BOMBS)
        );
        let mut records = Default::default();
        assert!(crate::rules::pastimes::Records::merge(
            &mut records,
            &r.home.countryside.games
        ));
        assert_eq!(records.latch.plays, 2);
    }
}
#[test]
fn latch_same_seed_same_deck_includes_both_attacks_and_full_windups() {
    let t = Tune::default();
    let mut phrase_orders = Vec::new();
    for seed in 1..=8 {
        let mut a = Duel::new(seed);
        let mut b = a.clone();
        let mut moves = Vec::new();
        for _ in 0..240 {
            let before = a.clone();
            a.step(Input::default(), &t);
            b.step(Input::default(), &t);
            assert_eq!(a, b);
            if a.beat == Beat::Tell && before.beat != Beat::Tell {
                moves.push(a.height);
                assert_eq!(a.ticks, t.latch_tell);
                assert!(a.ticks >= 23);
            }
        }
        assert!(moves.contains(&-1) && moves.contains(&1));
        phrase_orders.push(moves);
    }
    assert!(phrase_orders.windows(2).any(|p| p[0] != p[1]));
    let mut tuned = t;
    tuned.set("latch_tell", 1.0);
    assert_eq!(tuned.latch_tell, 23);
}
#[test]
fn latch_visible_tiebreak_exact_ties_share_the_whole_purse() {
    let mut r = game(4);
    let s = r.latch.as_mut().unwrap();
    for (&id, d) in &mut s.duels {
        d.counters = 3;
        d.beat = Beat::Defeat;
        d.misses = if id == 1 { 1 } else { 0 };
        d.error = if id == 4 { 1 } else { 0 };
    }
    s.finish = Some(1);
    r.step(&BTreeMap::new());
    let s = r.latch.as_ref().unwrap();
    assert_eq!(s.order, vec![2, 3, 4, 1]);
    assert_eq!(s.place(2), 1);
    assert_eq!(s.place(3), 1);
    assert_eq!(s.place(4), 3);
    assert_eq!(s.payout(2), 20);
    assert_eq!(s.payout(3), 20);
    assert_eq!(s.payout(4), 0);
    let mut r = game(4);
    let s = r.latch.as_mut().unwrap();
    s.left = 1;
    for (&id, d) in &mut s.duels {
        d.beat = Beat::Rest;
        d.ticks = 24;
        d.counters = if id == 1 { 1 } else { 2 };
    }
    r.step(&BTreeMap::new());
    let s = r.latch.as_ref().unwrap();
    assert!(!s.won);
    assert_eq!((s.payout(2), s.payout(3), s.payout(4)), (14, 13, 13));
    assert_eq!(s.duels.keys().map(|&id| s.payout(id)).sum::<u32>(), 40);
    assert!(r.players[&2].laurel && r.players[&3].laurel && r.players[&4].laurel);
}
#[test]
fn latch_invalid_save_repairs_departures_and_leaving_clear_the_activity() {
    let mut r = game(2);
    let mut broken = r.latch.clone().unwrap();
    broken.duels.get_mut(&1).unwrap().ticks = 999;
    assert!(!broken.valid());
    broken.repair();
    assert!(broken.valid());
    broken.duels.clear();
    broken.repair();
    assert!(broken.valid());
    r.leave(2);
    r.step(&BTreeMap::new());
    assert_eq!(r.latch.as_ref().unwrap().duels.len(), 1);
    r.arrive(
        r.dungeon
            .rooms
            .iter()
            .position(|r| r.kind == crate::rules::RoomKind::Market)
            .unwrap(),
        (12., 10.),
    );
    r.step(&BTreeMap::new());
    assert!(r.latch.is_none());
}
#[test]
fn latch_each_feel_knob_changes_its_own_beat() {
    let base = Tune::default();
    for name in [
        "latch_tell",
        "latch_window",
        "latch_rest",
        "latch_recover",
        "latch_stop",
    ] {
        let mut moved = base;
        let knob = base.knobs().into_iter().find(|k| k.name == name).unwrap();
        moved.set(name, knob.max);
        let play = |t: &Tune| {
            let mut d = Duel::new(1);
            let mut result = Vec::new();
            for tick in 0..240 {
                let input = Input {
                    move_y: d.height,
                    fire: d.beat == Beat::Tell
                        && d.ticks == if name == "latch_window" { 8 } else { 1 },
                    ..Default::default()
                };
                d.step(input, t);
                result.push((tick, d.beat, d.ticks, d.freeze, d.counters));
            }
            result
        };
        // The rest/tell/window/recover knobs all affect this scripted early player.
        if name != "latch_stop" {
            assert_ne!(play(&base), play(&moved), "{name}");
        } else {
            let mut a = tell(1, -1);
            let mut b = a.clone();
            a.step(
                Input {
                    fire: true,
                    ..Default::default()
                },
                &base,
            );
            b.step(
                Input {
                    fire: true,
                    ..Default::default()
                },
                &moved,
            );
            assert_ne!(a.freeze, b.freeze);
        }
    }
}
#[test]
#[ignore = "writes 1x review frames and bot tuning ledger"]
fn write_latch_review() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, r: &Run, id| {
        let im = crate::render::overworld::arena::frame_for(r, 384, 224, Some(id));
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgb_bytes(),
            384,
            224,
            image::ColorType::Rgb8,
        )
        .unwrap();
    };
    save("entrance", &Run::latch_for_test(), 1);
    let mut r = game(4);
    for (name, beat, height, ticks, counters, chain, freeze) in [
        ("idle", Beat::Rest, -1, 24, 0, 0, 0),
        ("high-tell", Beat::Tell, -1, 30, 0, 0, 0),
        ("high-window", Beat::Tell, -1, 7, 0, 0, 0),
        ("low-tell", Beat::Tell, 1, 30, 0, 0, 0),
        ("low-window", Beat::Tell, 1, 7, 0, 0, 0),
        ("high-miss", Beat::Miss, -1, 18, 0, 0, 0),
        ("low-miss", Beat::Miss, 1, 18, 0, 0, 0),
        ("perfect", Beat::Clean, -1, 18, 1, 1, 4),
        ("chain", Beat::Clean, 1, 10, 2, 2, 0),
        ("damage-1", Beat::Rest, -1, 24, 1, 1, 0),
        ("damage-2", Beat::Rest, 1, 24, 2, 2, 0),
        ("defeat", Beat::Defeat, -1, 0, 3, 3, 0),
    ] {
        let d = r.latch.as_mut().unwrap().duels.get_mut(&1).unwrap();
        *d = Duel::new(d.seed);
        d.beat = beat;
        d.height = height;
        d.guard = height;
        d.ticks = ticks;
        d.counters = counters;
        d.chain = chain;
        d.freeze = freeze;
        save(name, &r, 1);
    }
    for (name,ticks,freeze) in [("thrust-brace",18,4),("thrust-contact",18,2),("thrust-recover",5,0)] {
        let d = r.latch.as_mut().unwrap().duels.get_mut(&1).unwrap();
        d.beat=Beat::Clean; d.height=-1; d.guard=-1; d.counters=1; d.chain=1;
        d.ticks=ticks; d.freeze=freeze;
        save(name,&r,1);
    }
    let identity = r.latch.as_ref().unwrap().duels[&1].clone();
    for d in r.latch.as_mut().unwrap().duels.values_mut() { *d = identity.clone(); }
    for id in 1..=4 { save(&format!("identity-{id}"),&r,id); }
    let mut ledger =
        String::from("tell,window,rest,recover,stop,seats,finish_ticks,misses,timing\n");
    for values in [(30, 5, 18, 15, 3), (36, 7, 24, 18, 4), (45, 10, 30, 24, 6)] {
        for seats in [1, 2, 4, 2, 4] {
            let mut r = game(seats);
            r.tune.latch_tell = values.0;
            r.tune.latch_window = values.1;
            r.tune.latch_rest = values.2;
            r.tune.latch_recover = values.3;
            r.tune.latch_stop = values.4;
            let mut elapsed = 0;
            for tick in 0..LENGTH {
                let s = r.latch.as_ref().unwrap();
                if s.left == 0 {
                    break;
                }
                let input = s
                    .duels
                    .iter()
                    .map(|(&id, d)| (id, d.bot_input(1 + id % 3)))
                    .collect();
                r.step(&input);
                elapsed += 1;
                if values.0 == 36 && seats == 4 && tick == 45 {
                    save("live-seat-1", &r, 1);
                    save("live-seat-2", &r, 2);
                }
            }
            let s = r.latch.as_ref().unwrap();
            let d = &s.duels[&s.order[0]];
            ledger.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                values.0, values.1, values.2, values.3, values.4, seats, elapsed, d.misses, d.error
            ));
            if values.0 == 36 && seats == 4 {
                save("podium", &r, 1);
            }
        }
    }
    let mut r = game(4);
    for (id, name) in [(2, "Bot 1"), (3, "Bot 2"), (4, "Bot 3")] {
        r.players.get_mut(&id).unwrap().name = name.into();
    }
    let s = r.latch.as_mut().unwrap();
    s.order = vec![4, 2, 3, 1];
    s.left = 0;
    s.won = true;
    for (&id, d) in &mut s.duels {
        d.counters = match id { 4 => 3, 2 => 2, _ => 0 };
        d.error = if id == 4 { 2 } else { 0 };
        d.misses = u32::from(id == 1);
    }
    save("podium-zero", &r, 1);
    std::fs::write(dir.join("bot-tuning.csv"), ledger).unwrap();
}

fn sample_counter(r: &mut Run, remaining: u32) -> Input {
    loop {
        let d = &r.latch.as_ref().unwrap().duels[&1];
        if d.beat == Beat::Tell && d.ticks == remaining {
            return Input { fire: true, move_y: d.height, latch_tick: Some(r.tick), ..Default::default() };
        }
        r.step(&BTreeMap::new());
    }
}

#[test]
fn latch_delayed_counter_uses_its_sample_tick_and_restores_the_chain() {
    let mut remote = game(2);
    remote.latch_input_age.insert(1, 10);
    let input = sample_counter(&mut remote, 2);
    let d = remote.latch.as_mut().unwrap().duels.get_mut(&1).unwrap();
    d.counters = 1;
    d.chain = 1;
    let mut local = remote.clone();
    local.step(&BTreeMap::from([(1, input)]));
    for _ in 0..10 { remote.step(&BTreeMap::new()); }
    assert_eq!(remote.latch.as_ref().unwrap().duels[&1].beat, Beat::Miss);
    remote.step(&BTreeMap::from([(1, input)]));
    let a = &local.latch.as_ref().unwrap().duels[&1];
    let b = &remote.latch.as_ref().unwrap().duels[&1];
    assert_eq!((b.counters,b.misses,b.error,b.chain), (a.counters,a.misses,a.error,a.chain));
    assert_eq!((b.counters,b.misses,b.error,b.chain), (2,0,1,2));
    assert_eq!(b.freeze, STOP);
    assert!(remote.valid_snapshot());
    // A resent press cannot buy a second counter, including after recovery.
    for _ in 0..STOP + RECOVER + REST { remote.step(&BTreeMap::new()); }
    remote.step(&BTreeMap::from([(1, input)]));
    assert_eq!(remote.latch.as_ref().unwrap().duels[&1].counters, 2);
}

#[test]
fn latch_lag_does_not_forgive_early_late_or_wrong_height_inputs() {
    for (remaining, offset, wrong) in [(WINDOW+1,0,false),(1,1,false),(2,0,true)] {
        let mut r = game(2);
        r.latch_input_age.insert(1, 12);
        let mut input = sample_counter(&mut r, remaining);
        input.latch_tick = input.latch_tick.map(|t| t + offset);
        if wrong { input.move_y *= -1; }
        for _ in 0..remaining + 3 { r.step(&BTreeMap::new()); }
        r.step(&BTreeMap::from([(1,input)]));
        let d = &r.latch.as_ref().unwrap().duels[&1];
        assert_eq!((d.counters,d.misses), (0,1));
        assert_eq!(d.beat, Beat::Miss);
    }
    // Transit cannot turn an early sample into a hit merely because it arrives
    // while the host itself is inside the counter window.
    let mut early = game(2);
    early.latch_input_age.insert(1, 2);
    let input = sample_counter(&mut early, WINDOW + 1);
    for _ in 0..2 { early.step(&BTreeMap::new()); }
    assert!(early.latch.as_ref().unwrap().duels[&1].ticks <= WINDOW);
    early.step(&BTreeMap::from([(1,input)]));
    assert_eq!(early.latch.as_ref().unwrap().duels[&1].counters,0);
    assert_eq!(early.latch.as_ref().unwrap().duels[&1].beat,Beat::Miss);
    // An early attempt already consumed this tell; it cannot be rewritten later.
    let mut r = game(2);
    let input = sample_counter(&mut r, WINDOW + 1);
    r.step(&BTreeMap::from([(1,input)]));
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(1,Input { latch_tick: input.latch_tick.map(|t| t+2), ..input })]));
    assert_eq!(r.latch.as_ref().unwrap().duels[&1].counters, 0);
}

#[test]
fn latch_same_winning_tick_can_arrive_during_the_existing_contact_hold() {
    let mut r = game(2);
    r.latch_input_age.insert(2, 10);
    let input = sample_counter(&mut r, 2);
    for d in r.latch.as_mut().unwrap().duels.values_mut() { d.counters=2; d.chain=2; }
    r.step(&BTreeMap::from([(1,input)]));
    // The common input tick remains fixed if the live recovery knob moves.
    let winning = r.latch.as_ref().unwrap().finish_at;
    r.tune.latch_recover = 9;
    assert_eq!(r.latch.as_ref().unwrap().finish_at,winning);
    for _ in 0..8 { r.step(&BTreeMap::new()); }
    r.step(&BTreeMap::from([(2,input)]));
    assert_eq!(r.latch.as_ref().unwrap().duels[&2].counters, 3);
    for _ in 0..STOP + RECOVER { r.step(&BTreeMap::new()); }
    let s = r.latch.as_ref().unwrap();
    assert_eq!(s.score(1),s.score(2));
    assert_eq!((s.payout(1),s.payout(2)),(10,10));
    assert_eq!(timing(6), "+200MS");
    assert_eq!(timing(1), "+33MS");
}


#[test]
fn latch_sample_age_rejects_a_spoofed_old_perfect_tick_and_future_tick() {
    for (delay, future) in [(4, false), (0, true)] {
        let mut r = game(2);
        r.latch_input_age.insert(1, 3);
        let mut input = sample_counter(&mut r, 1);
        for _ in 0..delay { r.step(&BTreeMap::new()); }
        if future { input.latch_tick = Some(r.tick + 1); }
        r.step(&BTreeMap::from([(1, input)]));
        let d = &r.latch.as_ref().unwrap().duels[&1];
        assert_eq!(d.counters, 0);
        assert_eq!(d.error, 0);
        assert_eq!(d.misses, 1);
    }
    // Both allowance boundaries are inclusive, without promoting a stale tick
    // to the current perfect tick or forgiving an expired sample.
    let mut r = game(2);
    r.latch_input_age.insert(1, 3);
    let input = sample_counter(&mut r, 2);
    for _ in 0..3 { r.step(&BTreeMap::new()); }
    r.step(&BTreeMap::from([(1, input)]));
    assert_eq!(r.latch.as_ref().unwrap().duels[&1].counters, 1);
    assert_eq!(r.latch.as_ref().unwrap().duels[&1].error, 1);
}

#[test]
fn latch_held_back_ack_delay_cannot_buy_an_older_counter() {
    let allowance = angel_net::cmd::sample_allowance(std::time::Duration::from_secs(2), HZ, 1, false);
    assert_eq!(allowance, 11);
    for delay in [allowance, allowance + 1] {
        let mut r = game(2);
        r.latch_input_age.insert(1, allowance);
        let input = sample_counter(&mut r, 2);
        for _ in 0..delay { r.step(&BTreeMap::new()); }
        r.step(&BTreeMap::from([(1, input)]));
        let d = &r.latch.as_ref().unwrap().duels[&1];
        assert_eq!(d.counters, u8::from(delay == allowance));
        assert_eq!(d.misses, u32::from(delay > allowance));
    }
}

#[test]
fn latch_miss_rollback_keeps_records_and_the_paid_pot_consistent() {
    let mut r = game(2);
    r.latch_input_age.insert(2, 10);
    let input = sample_counter(&mut r, 2);
    for d in r.latch.as_mut().unwrap().duels.values_mut() { d.counters = 2; d.chain = 2; }
    r.latch_input_age.insert(1, 3);
    for _ in 0..3 { r.step(&BTreeMap::new()); }
    assert_eq!(r.latch.as_ref().unwrap().duels[&2].beat, Beat::Miss);
    r.step(&BTreeMap::from([(1, input)]));
    for _ in 0..4 { r.step(&BTreeMap::new()); }
    r.step(&BTreeMap::from([(2, input)]));
    let s = r.latch.as_ref().unwrap();
    assert_eq!(s.score(1), s.score(2));
    assert_eq!((s.duels[&2].counters,s.duels[&2].misses,s.duels[&2].error,s.duels[&2].chain), (3,0,1,3));
    assert_eq!(r.home.countryside.games.latch.wins, 0);
    for _ in 0..STOP + RECOVER { r.step(&BTreeMap::new()); }
    let s = r.latch.as_ref().unwrap();
    assert_eq!((s.payout(1),s.payout(2)), (10,10));
    assert_eq!(r.players.values().map(|h| h.carried.get(Spoil::Gold)).sum::<u32>(), 20);
    assert_eq!((r.home.countryside.games.latch.plays,r.home.countryside.games.latch.wins,r.home.countryside.games.latch.best), (1,1,3));
    for _ in 0..40 { r.step(&BTreeMap::new()); }
    assert_eq!(r.players.values().map(|h| h.carried.get(Spoil::Gold)).sum::<u32>(), 20);
    assert_eq!(r.home.countryside.games.latch.wins, 1);
}

#[test]
fn latch_late_bot_misses_and_window_edge_bot_counters() {
    let mut late = game(1);
    let mut edge = game(1);
    for _ in 0..LENGTH {
        let d = &late.latch.as_ref().unwrap().duels[&1];
        let input = Input { latch_tick: Some(late.tick), ..d.late_bot_input(RECOVER) };
        late.step(&BTreeMap::from([(1,input)]));
        if edge.latch.as_ref().unwrap().left > 0 {
            let d = &edge.latch.as_ref().unwrap().duels[&1];
            edge.step(&BTreeMap::from([(1,d.bot_input(WINDOW))]));
        }
    }
    let a = &late.latch.as_ref().unwrap().duels[&1];
    let b = &edge.latch.as_ref().unwrap().duels[&1];
    assert_eq!(a.counters, 0); assert!(a.misses > 0);
    assert_eq!((b.counters,b.misses,b.error), (3,0,3*(WINDOW-1)));
}
