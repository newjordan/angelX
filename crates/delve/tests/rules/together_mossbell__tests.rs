use super::{BTreeMap, HZ, Input, Run, Spoil};
use crate::rules::{mossbell::*, tune::Tune};
fn game(n: u32) -> Run {
    let mut r = Run::mossbell_for_test();
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
    assert!(r.mossbell.is_some());
    r
}
#[test]
fn mossbell_four_substeps_keep_balls_bounded_and_same_seed_is_exact() {
    let t = Tune::default();
    for seed in 1..=12 {
        let mut a = Table::new(seed);
        let mut b = a.clone();
        for age in 0..LENGTH {
            let input = if seed % 3 == 0 {
                Input::default()
            } else {
                a.bot_input()
            };
            a.step(input, &t, age, u64::from(age), 0);
            b.step(input, &t, age, u64::from(age), 0);
            assert_eq!(a.ball, b.ball);
            assert!(a.valid(), "seed {seed} age {age}: {:?}", a.ball);
        }
        assert!(a.ball.drains > 0 || a.ball.score > 0);
    }
}
#[test]
fn mossbell_paddles_lift_a_falling_ball_and_nudge_is_a_fresh_press() {
    let t = Tune::default();
    let mut b = Ball::new();
    b.save = 0;
    b.x = 58 * Q;
    b.y = 116 * Q;
    b.vy = 500;
    b.step(
        Stroke {
            left: true,
            ..Default::default()
        },
        &t,
        0,
        1,
        0,
    );
    assert!(b.vy < 0, "{:?}", b);
    assert_eq!(b.freeze, 2);
    let mut a = Table::new(41);
    a.ball.save = 0;
    a.ball.x = 30 * Q;
    a.ball.y = 90 * Q;
    a.step(
        Input {
            move_y: -1,
            ..Default::default()
        },
        &t,
        0,
        0,
        0,
    );
    let vx = a.ball.vx;
    a.step(
        Input {
            move_y: -1,
            ..Default::default()
        },
        &t,
        1,
        1,
        0,
    );
    assert_eq!(a.ball.vx, vx, "held nudge cannot accumulate");
}
#[test]
fn mossbell_contacts_score_damage_and_jackpot_requires_all_three_shapes() {
    let t = Tune::default();
    let mut b = Ball::new();
    b.save = 0;
    b.x = 80 * Q;
    b.y = 49 * Q;
    b.vy = -1200;
    b.step(Stroke::default(), &t, 0, 1, 0);
    assert_eq!(b.hp, 5);
    assert_eq!(b.score, 100);
    assert_eq!(b.beat, Beat::Flinch);
    let mut b = Ball::new();
    b.save = 0;
    b.x = 44 * Q;
    b.y = 72 * Q;
    b.vy = -800;
    b.step(Stroke::default(), &t, 0, 1, 0);
    assert_eq!(b.lit, 1);
    assert_eq!(b.score, 25);
    assert_eq!(b.chain, 1);
    b.hp = 0;
    b.lit = 3;
    b.step(Stroke::default(), &t, 1, 1, 1);
    assert_eq!(b.jackpot_at, None);
    b.lit = 7;
    b.step(Stroke::default(), &t, 2, 1, 2);
    assert_eq!(b.jackpot_at, Some(2));
    b.step(Stroke::default(), &t, 3, 1, 3);
    assert_eq!(b.jackpot_at, Some(2));
}
#[test]
fn mossbell_network_stroke_replays_the_sample_tick_including_hits() {
    let t = Tune::default();
    let mut ontime = Table::new(41);
    ontime.ball.save = 0;
    ontime.ball.x = 58 * Q;
    ontime.ball.y = 116 * Q;
    ontime.ball.vy = 500;
    let mut delayed = ontime.clone();
    let stroke = Input {
        fire: true,
        latch_tick: Some(0),
        ..Default::default()
    };
    for tick in 0..8 {
        ontime.step(
            if tick == 0 { stroke } else { Input::default() },
            &t,
            tick,
            tick as u64,
            10,
        );
        delayed.step(
            if tick == 5 { stroke } else { Input::default() },
            &t,
            tick,
            tick as u64,
            10,
        );
    }
    assert_eq!(
        ontime.ball, delayed.ball,
        "packet arrival cannot move the flipper contact"
    );
    let before = delayed.ball.clone();
    delayed.step(
        Input {
            latch_tick: Some(900),
            fire: true,
            ..Default::default()
        },
        &t,
        8,
        8,
        10,
    );
    assert!(delayed.ball.valid());
    assert_eq!(before.score, delayed.ball.score);
}

#[test]
fn mossbell_held_back_ack_delay_cannot_replay_an_older_stroke() {
    let allowance = angel_net::cmd::sample_allowance(std::time::Duration::from_secs(2), HZ, 1, false);
    assert_eq!(allowance, 11);
    for delay in [allowance, allowance + 1] {
        let mut r = game(2);
        r.latch_input_age.insert(1, allowance);
        let sample = r.tick;
        let b = &mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;
        b.save = 0; b.x = 58 * Q; b.y = 116 * Q; b.vy = 500;
        for _ in 0..delay { r.step(&BTreeMap::new()); }
        let mut control = r.clone();
        control.step(&BTreeMap::new());
        r.step(&BTreeMap::from([(1, Input { fire: true, latch_tick: Some(sample), ..Default::default() })]));
        let actual = &r.mossbell.as_ref().unwrap().tables[&1].ball;
        let without = &control.mossbell.as_ref().unwrap().tables[&1].ball;
        assert_eq!(actual == without, delay > allowance);
    }
}
#[test]
fn mossbell_race_uses_earliest_sample_and_shares_same_tick_jackpots() {
    let mut s = Session::new(1..=4, 41);
    s.tables.get_mut(&4).unwrap().ball.jackpot_at = Some(200);
    s.race();
    assert_eq!(s.jackpot, vec![4]);
    s.tables.get_mut(&2).unwrap().ball.jackpot_at = Some(199);
    s.race();
    assert_eq!(s.jackpot, vec![2]);
    s.tables.get_mut(&3).unwrap().ball.jackpot_at = Some(199);
    s.race();
    assert_eq!(s.jackpot, vec![2, 3]);
    assert_eq!(s.points(2), 500);
}
#[test]
fn mossbell_every_seat_plays_purse_once_tiebreak_rematch_and_crown() {
    let mut r = game(4);
    let seed = r.mossbell.as_ref().unwrap().seed;
    r.step(&BTreeMap::from([(
        2,
        Input {
            fire: true,
            ..Default::default()
        },
    )]));
    assert_ne!(
        r.mossbell.as_ref().unwrap().tables[&1].ball,
        r.mossbell.as_ref().unwrap().tables[&2].ball
    );
    assert_eq!(
        r.wire_copy().mossbell.unwrap().tables[&2].ball,
        r.mossbell.as_ref().unwrap().tables[&2].ball
    );
    assert!(r.valid_snapshot());
    let s = r.mossbell.as_mut().unwrap();
    s.left = 1;
    for (&id, t) in &mut s.tables {
        t.ball.save = 12;
        t.ball.score = 1000;
        t.ball.drains = if id == 1 { 2 } else { 1 };
    }
    r.step(&BTreeMap::new());
    let s = r.mossbell.as_ref().unwrap();
    assert_eq!(s.order, vec![2, 3, 4, 1]);
    assert_eq!(s.payout(2), 14);
    assert_eq!(s.payout(3), 13);
    assert_eq!(s.payout(4), 13);
    assert!(r.players[&2].laurel);
    assert!(!r.players[&1].laurel);
    let gold = r.players[&2].carried.get(Spoil::Gold);
    r.step(&BTreeMap::new());
    assert_eq!(r.players[&2].carried.get(Spoil::Gold), gold);
    r.step(&BTreeMap::from([(
        3,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert_eq!(r.mossbell.as_ref().unwrap().left, LENGTH);
    assert_ne!(r.mossbell.as_ref().unwrap().seed, seed);
    assert_eq!(r.home.countryside.games.mossbell.plays, 2);
    let bombs = r.players[&2].bombs;
    r.kit_out_for_test();
    assert_eq!(
        r.players[&2].bombs,
        (bombs + 1).min(crate::rules::MAX_BOMBS)
    );
}
#[test]
fn mossbell_drain_returns_under_a_second_and_attacks_telegraph() {
    let t = Tune::default();
    let mut b = Ball::new();
    b.save = 0;
    b.x = 80 * Q;
    b.y = H * Q;
    b.vy = 1200;
    b.step(Stroke::default(), &t, 0, 1, 0);
    assert_eq!(b.drains, 1);
    assert_eq!(b.save, 12);
    assert_eq!(b.chain, 0);
    for i in 1..=12 {
        b.step(Stroke::default(), &t, i, 1, i.into());
    }
    assert_eq!(b.save, 0);
    assert!(b.vy < 0);
    assert!(12 < HZ);
    let mut seen = vec![];
    let mut b = Ball::new();
    for tick in 0..600 {
        let prev = b.beat;
        b.step(Stroke::default(), &t, tick, 41, tick.into());
        if b.beat != prev && matches!(b.beat, Beat::RootTell | Beat::CrownTell) {
            assert!(b.beat_left >= 23);
            seen.push(b.beat);
        }
    }
    assert!(seen.contains(&Beat::RootTell) && seen.contains(&Beat::CrownTell));
}
#[test]
fn mossbell_invalid_state_repairs_and_departure_clears_activity() {
    let mut s = Session::new(1..=4, 41);
    s.tables.get_mut(&2).unwrap().ball.x = -1;
    assert!(!s.valid());
    s.repair();
    assert!(s.valid());
    let mut r = game(2);
    let home = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == super::RoomKind::Home)
        .unwrap();
    r.arrive(home, (5., 5.));
    assert!(r.mossbell.is_none());
}
#[test]
#[ignore = "writes native table states and five bot rounds per tune"]
fn write_mossbell_review() {
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
    save("entrance", &Run::mossbell_for_test(), 1);
    let mut r = game(4);
    for (name, beat, hp) in [
        ("idle", Beat::Idle, 6),
        ("root-tell", Beat::RootTell, 6),
        ("root-attack", Beat::Root, 6),
        ("crown-tell", Beat::CrownTell, 6),
        ("crown-attack", Beat::Crown, 6),
        ("flinch", Beat::Flinch, 5),
        ("damage-1", Beat::Idle, 4),
        ("damage-2", Beat::Idle, 2),
        ("defeat", Beat::Defeat, 0),
    ] {
        let b = &mut r
            .mossbell
            .as_mut()
            .unwrap()
            .tables
            .get_mut(&1)
            .unwrap()
            .ball;
        *b = Ball::new();
        b.beat = beat;
        b.beat_left = 24;
        b.hp = hp;
        b.score = 1375;
        b.save = 0;
        b.x = 66 * Q;
        b.y = 95 * Q;
        b.trail = [(64, 89), (63, 84), (64, 78), (65, 74)];
        save(name, &r, 1);
    }
    for (name, chain, burst, freeze) in [
        ("chain", 3, 8, 0),
        ("flipper", 0, 10, 2),
        ("jackpot", 3, 15, 1),
    ] {
        let b = &mut r
            .mossbell
            .as_mut()
            .unwrap()
            .tables
            .get_mut(&1)
            .unwrap()
            .ball;
        b.hp = if name == "jackpot" { 0 } else { 4 };
        b.beat = if name == "jackpot" {
            Beat::Defeat
        } else {
            Beat::Idle
        };
        b.chain = chain;
        b.lit = if chain == 3 { 7 } else { 0 };
        b.flash = [8; 3];
        b.burst = burst;
        b.freeze = freeze;
        b.left = 4;
        b.right = 4;
        b.impact = (80, 98);
        if name == "jackpot" {
            b.jackpot_at = Some(r.tick);
            b.impact = (80, 63);
        }
        r.mossbell.as_mut().unwrap().race();
        save(name, &r, 1);
    }
    for (prefix,beat,total) in [("root-stage",Beat::RootTell,36),("crown-stage",Beat::CrownTell,36),
        ("root-rage",Beat::RootTell,30),("crown-rage",Beat::CrownTell,30)] {
        for stage in 0..4 {
            let b=&mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;
            *b=Ball::new(); b.beat=beat; b.beat_left=total*(4-stage)/4;
            b.hp=if prefix.ends_with("rage") {2} else {6}; b.save=0; b.score=1375;
            r.mossbell.as_mut().unwrap().race();
            save(&format!("{prefix}-{stage}"),&r,1);
        }
    }
    for (name, attack) in [("root-right",0),("root-left",1)] {
        let b=&mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;
        *b=Ball::new();b.beat=Beat::RootTell;b.beat_left=1;b.attack=attack;
        save(name,&r,1);
    }
    let mut ledger =
        String::from("flipper,gravity,bounce,nudge,launch,seats,score,drains,boss_hp,jackpot\n");
    for values in [
        (1000, 22, 75, 120, 1300),
        (1250, 28, 85, 180, 1450),
        (1500, 36, 90, 240, 1650),
    ] {
        for seats in [1, 2, 4, 2, 4] {
            let mut r = game(seats);
            r.tune.moss_flipper = values.0;
            r.tune.moss_gravity = values.1;
            r.tune.moss_bounce = values.2;
            r.tune.moss_nudge = values.3;
            r.tune.moss_launch = values.4;
            for tick in 0..LENGTH {
                let inputs = r
                    .mossbell
                    .as_ref()
                    .unwrap()
                    .tables
                    .iter()
                    .map(|(&id, t)| {
                        (
                            id,
                            if id == 1 && seats > 1 && tick % 90 < 20 {
                                Input::default()
                            } else {
                                t.bot_input()
                            },
                        )
                    })
                    .collect();
                r.step(&inputs);
                if values.0 == 1250 && seats == 4 && tick == 200 {
                    save("live-seat-1", &r, 1);
                    save("live-seat-2", &r, 2);
                }
            }
            let s = r.mossbell.as_ref().unwrap();
            let b = &s.tables[&s.order[0]].ball;
            ledger.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                values.0,
                values.1,
                values.2,
                values.3,
                values.4,
                seats,
                b.score,
                b.drains,
                b.hp,
                b.jackpot_at.is_some()
            ));
            if values.0 == 1250 && seats == 4 {
                save("podium", &r, 1);
            }
        }
    }
    std::fs::write(dir.join("bot-tuning.csv"), ledger).unwrap();
}

#[test]
fn mossbell_host_replays_the_visible_tick_and_all_bot_rounds_can_pay() {
    let mut a = game(2);
    a.latch_input_age.insert(1, 10);
    let mut b = a.clone();
    let tick = a.tick;
    for r in [&mut a, &mut b] {
        let ball = &mut r
            .mossbell
            .as_mut()
            .unwrap()
            .tables
            .get_mut(&1)
            .unwrap()
            .ball;
        ball.save = 0;
        ball.x = 58 * Q;
        ball.y = 116 * Q;
        ball.vy = 500;
    }
    let input = Input {
        fire: true,
        latch_tick: Some(tick),
        ..Default::default()
    };
    for n in 0..8 {
        a.step(&BTreeMap::from([(
            1,
            if n == 0 { input } else { Input::default() },
        )]));
        b.step(&BTreeMap::from([(
            1,
            if n == 5 { input } else { Input::default() },
        )]));
    }
    assert_eq!(
        a.mossbell.as_ref().unwrap().tables[&1].ball,
        b.mossbell.as_ref().unwrap().tables[&1].ball
    );
    for seats in [1, 2, 4, 2, 4] {
        let mut r = game(seats);
        for _ in 0..LENGTH {
            let inputs = r
                .mossbell
                .as_ref()
                .unwrap()
                .tables
                .iter()
                .map(|(&id, t)| (id, t.bot_input()))
                .collect();
            r.step(&inputs);
        }
        let s = r.mossbell.as_ref().unwrap();
        assert_eq!(s.left, 0);
        assert!(!s.jackpot.is_empty());
        assert_eq!(s.payout(s.order[0]) * seats, GOLD_PER_SEAT * seats);
        assert!(s.tables.values().all(|t| t.ball.hp == 0));
        assert!(r.valid_snapshot());
    }
}

#[test]
fn mossbell_each_live_knob_changes_its_physical_action() {
    for name in [
        "moss_flipper",
        "moss_gravity",
        "moss_bounce",
        "moss_nudge",
        "moss_launch",
    ] {
        let plain = Tune::default();
        let mut moved = plain;
        let knob = plain.knobs().into_iter().find(|k| k.name == name).unwrap();
        assert!(moved.set(name, knob.max));
        assert!(moved.valid());
        let mut a = Ball::new();
        if name != "moss_launch" {
            a.save = 0;
            a.x = 58 * Q;
            a.y = 116 * Q;
            a.vy = 500;
        }
        if name == "moss_nudge" {
            a.x = 30 * Q;
            a.y = 90 * Q;
            a.vy = 0;
        }
        if name == "moss_bounce" {
            a.x = 44 * Q;
            a.y = 72 * Q;
            a.vy = -800;
        }
        let mut b = a.clone();
        let stroke = Stroke {
            left: true,
            right: true,
            nudge: 1,
            launch: true,
        };
        a.step(stroke, &plain, 0, 41, 0);
        b.step(stroke, &moved, 0, 41, 0);
        assert_ne!(a, b, "{name}");
    }
}
