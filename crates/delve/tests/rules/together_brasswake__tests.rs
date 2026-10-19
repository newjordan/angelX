use super::{BTreeMap, Input, Run, Spoil};
use crate::rules::{brasswake::*, tune::Tune};
fn game(seats: u32) -> Run {
    let mut r = Run::brasswake_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    if seats == 1 {
        r.leave(2);
    }
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(r.brasswake.is_some());
    r
}
fn close() -> Lane {
    let mut d = Lane::new(41);
    d.x = 150;
    d.y = 150;
    d.foes[0].x = 180;
    d.foes[0].y = 150;
    d.foes[1].x = 240;
    d.foes[1].y = 174;
    d
}
#[test]
fn brasswake_combo_has_two_punches_a_kick_and_local_freeze() {
    let mut d = close();
    let t = Tune::default();
    let mut now = 0;
    for n in 0..3 {
        d.step_at(
            Input {
                fire: true,
                ..Default::default()
            },
            &t,
            now,
            0,
            now as u32,
        );
        assert_eq!(d.pose, 3 + n);
        assert!(d.freeze > 0);
        assert_eq!(d.chain, u32::from(n) + 1);
        d.foes[0].x = d.x + 30;
        for _ in 0..t.brass_combo {
            now += 1;
            d.step_at(Input::default(), &t, now, 0, now as u32);
        }
        now += 1;
    }
    assert_eq!(d.foes[0].hp, 3);
    assert_eq!(d.combo, 0);
    assert!(d.valid());
}

#[test]
fn brasswake_held_back_ack_delay_cannot_buy_an_older_hit() {
    let allowance = angel_net::cmd::sample_allowance(std::time::Duration::from_secs(2), crate::rules::HZ, 1, false);
    assert_eq!(allowance, 11);
    for delay in [allowance, allowance + 1] {
        let mut d = close();
        let t = Tune::default();
        d.step_at(Input::default(), &t, 100, allowance, 0);
        d.step_at(Input { fire: true, latch_tick: Some(100), ..Default::default() }, &t, 100 + delay, allowance, delay as u32);
        assert_eq!(d.foes[0].hp, if delay == allowance { 8 } else { 10 });
    }
}
#[test]
fn brasswake_same_seed_independent_lanes_and_camera() {
    let mut r = game(4);
    let seed = r.brasswake.as_ref().unwrap().seed;
    for d in r.brasswake.as_ref().unwrap().lanes.values() {
        assert_eq!(d.seed, seed);
    }
    let before = r.brasswake.as_ref().unwrap().lanes[&2].x;
    for _ in 0..40 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                move_x: 1,
                ..Default::default()
            },
        )]));
    }
    let s = r.brasswake.as_ref().unwrap();
    assert!(s.lanes[&1].x > before);
    assert_eq!(s.lanes[&2].x, before);
    assert!(s.lanes[&1].camera > s.lanes[&2].camera);
    assert_eq!(s.lanes[&3].foes, s.lanes[&4].foes);
    assert!(r.valid_snapshot());
}
#[test]
fn brasswake_tells_are_at_least_three_quarters_and_exact_danger_lands() {
    let t = Tune::default();
    for hp in [32, 20, 8] {
        for rush in [false, true] {
            let mut d = close();
            d.foes = vec![Foe::new(180, 150, true), Foe::new(240, 174, false)];
            d.foes[0].hp = hp;
            d.foes[0].attack = if rush { 0 } else { 1 };
            d.foes[0].beat = Beat::Walk;
            d.step_at(Input::default(), &t, 0, 0, 0);
            let e = &d.foes[0];
            assert_eq!(e.beat, Beat::Tell);
            assert!(e.ticks >= 23);
            let z = e.danger;
            let ticks = e.ticks;
            d.x = z.x + z.w / 2;
            d.y = (z.y + z.h / 2).clamp(126, 174);
            for tick in 1..=ticks {
                d.step_at(Input::default(), &t, u64::from(tick), 0, tick);
            }
            assert_eq!(d.hp, 5, "inside mark gets hit");
            let mut safe = close();
            safe.foes[0] = d.foes[0].clone();
            safe.foes[0].beat = Beat::Tell;
            safe.foes[0].ticks = 1;
            safe.y = if z.y > 130 { 126 } else { 174 };
            safe.step_at(Input::default(), &t, 0, 0, 0);
            assert_eq!(safe.hp, 6, "outside mark is safe");
        }
    }
}
#[test]
fn brasswake_grab_throw_marks_only_the_next_rival_lane() {
    let mut r = game(4);
    let s = r.brasswake.as_mut().unwrap();
    for d in s.lanes.values_mut() {
        d.x = 82;
        d.key = false;
    }
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(r.brasswake.as_ref().unwrap().lanes[&1].holding);
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    let s = r.brasswake.as_ref().unwrap();
    assert!(!s.lanes[&1].holding);
    assert_eq!(s.lanes[&2].incoming.len(), 1);
    assert_eq!(s.lanes[&2].incoming[0].tell, TELL);
    assert!(s.lanes[&3].incoming.is_empty());
    assert!(s.lanes[&4].incoming.is_empty());
}
#[test]
fn brasswake_sampled_punch_uses_the_seen_position_and_latency_edges() {
    let t = Tune::default();
    let mut d = close();
    d.step_at(Input::default(), &t, 100, 8, 0);
    d.x = 40;
    d.foes[0].x = 220;
    d.step_at(
        Input {
            fire: true,
            latch_tick: Some(100),
            ..Default::default()
        },
        &t,
        108,
        8,
        8,
    );
    assert_eq!(d.foes[0].hp, 8);
    for stamp in [99, 109] {
        let mut d = close();
        d.step_at(Input::default(), &t, 100, 8, 0);
        d.step_at(
            Input {
                fire: true,
                latch_tick: Some(stamp),
                ..Default::default()
            },
            &t,
            108,
            8,
            8,
        );
        assert_eq!(d.foes[0].hp, 10);
    }
    let mut held = close();
    for tick in 0..40 {
        held.step_at(
            Input {
                fire: true,
                ..Default::default()
            },
            &t,
            tick,
            0,
            tick as u32,
        );
    }
    assert_eq!(held.score, 10, "hold is one punch");
}
#[test]
fn brasswake_down_stands_up_under_a_second_guard_and_bread_work() {
    let t = Tune::default();
    let mut d = close();
    d.hp = 1;
    d.foes[0].beat = Beat::Tell;
    d.foes[0].ticks = 1;
    d.foes[0].danger = Danger {
        x: 130,
        y: 138,
        w: 48,
        h: 28,
    };
    d.step_at(Input::default(), &t, 0, 0, 0);
    assert_eq!(d.down, 18);
    assert_eq!(d.falls, 1);
    for tick in 1..30 {
        d.step_at(Input::default(), &t, tick, 0, tick as u32);
    }
    assert_eq!(d.down, 0);
    assert_eq!(d.hp, 6);
    let mut g = close();
    g.key = false;
    g.foes[0].beat = Beat::Tell;
    g.foes[0].ticks = 1;
    g.foes[0].danger = Danger {
        x: 130,
        y: 138,
        w: 48,
        h: 28,
    };
    g.step_at(
        Input {
            interact: true,
            ..Default::default()
        },
        &t,
        0,
        0,
        0,
    );
    assert_eq!(g.hp, 6);
    g.hp = 3;
    g.freeze = 0;
    g.bread = Some(g.x);
    g.step_at(Input::default(), &t, 1, 0, 1);
    assert_eq!(g.hp, 5);
    assert_eq!(g.score, 25);
}
#[test]
fn brasswake_three_gates_then_the_boss_keep_encounter_indices_separate() {
    let t = Tune::default();
    let mut d = Lane::new(41);
    for wave in 0..4 {
        assert_eq!(d.wave, wave);
        for e in &mut d.foes {
            e.hp = 0;
            e.beat = Beat::Defeat;
        }
        d.x = (i32::from(wave) + 1) * 240 - 40;
        d.step_at(Input::default(), &t, u64::from(wave), 0, u32::from(wave));
        if wave < 3 {
            assert_eq!(d.wave, wave + 1);
            assert!(d.foes.iter().all(|e| e.hp > 0));
        } else {
            assert_eq!(d.finish, Some(3));
        }
    }
}
#[test]
fn brasswake_purse_ties_records_and_one_tick_rematch() {
    for seats in [1, 2, 4] {
        let mut r = game(seats);
        let before = r
            .players
            .values()
            .map(|h| h.carried.get(Spoil::Gold))
            .sum::<u32>();
        let s = r.brasswake.as_mut().unwrap();
        s.left = 1;
        for d in s.lanes.values_mut() {
            d.score = 200;
            d.finish = Some(1800);
        }
        r.step(&BTreeMap::new());
        let s = r.brasswake.as_ref().unwrap();
        assert_eq!(
            s.lanes.keys().map(|&id| s.payout(id)).sum::<u32>(),
            10 * seats
        );
        assert!(r.players.values().all(|h| h.laurel));
        let after = r
            .players
            .values()
            .map(|h| h.carried.get(Spoil::Gold))
            .sum::<u32>();
        assert_eq!(after - before, 10 * seats);
        for _ in 0..3 {
            r.step(&BTreeMap::new());
        }
        assert_eq!(
            r.players
                .values()
                .map(|h| h.carried.get(Spoil::Gold))
                .sum::<u32>(),
            after
        );
        assert_eq!(r.home.countryside.games.brasswake.wins, 1);
        assert_eq!(r.home.countryside.games.brasswake.best, 40);
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        assert_eq!(r.brasswake.as_ref().unwrap().left, LENGTH);
        assert_eq!(r.home.countryside.games.brasswake.plays, 2);
    }
}
#[test]
fn brasswake_rank_uses_work_then_falls_then_finish_and_splits_spare_gold() {
    let mut s = Session::new(1..=3, 41);
    s.left = 0;
    s.order = vec![1, 2, 3];
    for d in s.lanes.values_mut() {
        d.score = 100;
        d.finish = Some(1800);
    }
    s.lanes.get_mut(&3).unwrap().falls = 1;
    assert_eq!(s.place(1), 1);
    assert_eq!(s.place(3), 3);
    assert_eq!(s.payout(1), 15);
    s.lanes.get_mut(&1).unwrap().finish = Some(1799);
    assert_eq!(s.payout(1), 30);
    s.lanes.get_mut(&1).unwrap().score = 90;
    assert_eq!(s.place(2), 1);
}
#[test]
fn brasswake_invalid_state_repair_and_wire_omit_only_host_history() {
    let mut s = Session::new(1..=4, 41);
    s.lanes.get_mut(&2).unwrap().x = -100;
    assert!(!s.valid());
    s.repair();
    assert!(s.valid());
    let mut r = game(4);
    r.step(&BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            ..Default::default()
        },
    )]));
    let wire = serde_json::to_string(&r).unwrap();
    assert!(!wire.contains("history"));
    let loaded: Run = serde_json::from_str(&wire).unwrap();
    assert!(loaded.valid_snapshot());
    assert_eq!(
        loaded.brasswake.as_ref().unwrap().lanes[&1].x,
        r.brasswake.as_ref().unwrap().lanes[&1].x
    );
}
#[test]
fn brasswake_scripted_table_can_clear_and_idle_can_lose() {
    let mut r = game(4);
    for tick in 0..LENGTH {
        let s = r.brasswake.as_ref().unwrap();
        if s.left == 0 {
            break;
        }
        let inputs = s
            .lanes
            .iter()
            .map(|(&id, d)| (id, d.bot_input(tick)))
            .collect();
        r.step(&inputs);
        assert!(r.brasswake.as_ref().unwrap().valid(), "tick {tick}");
    }
    let s = r.brasswake.as_ref().unwrap();
    assert!(
        s.lanes.values().all(|d| d.finish.is_some()),
        "{:?}",
        s.lanes
            .iter()
            .map(|(id, d)| (
                id,
                d.wave,
                d.x,
                d.score,
                d.falls,
                d.foes.iter().map(|e| e.hp).collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
    assert!(
        r.valid_snapshot(),
        "a completed best stays in the shared ledger range"
    );
    let mut idle = game(2);
    for _ in 0..LENGTH {
        idle.step(&BTreeMap::new());
    }
    assert_eq!(idle.brasswake.as_ref().unwrap().left, 0);
    assert!(!idle.brasswake.as_ref().unwrap().won);
}
#[test]
#[ignore = "writes native room review and fifteen complete bot tuning rounds"]
fn write_brasswake_review() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let save = |n: &str, r: &Run, id| {
        let im = crate::render::overworld::arena::frame_for(r, 384, 224, Some(id));
        image::save_buffer(
            dir.join(format!("{n}.png")),
            &im.rgb_bytes(),
            384,
            224,
            image::ColorType::Rgb8,
        )
        .unwrap();
    };
    for n in [
        "entrance",
        "overlap",
        "standings",
        "idle",
        "slam-tell",
        "slam",
        "rush-tell",
        "rush",
        "flinch",
        "damage-1",
        "damage-2",
        "defeat",
        "throw",
        "down",
        "guard",
        "kick",
        "wave-1",
        "wave-2",
        "wave-3",
        "podium",
    ] {
        save(n, &Run::brasswake_review_for_test(n), 1);
    }
    let source = Run::brasswake_review_for_test("idle");
    let (base, _) =
        crate::render::overworld::arena::art_v2::terminal_layers(&source, Some(1)).unwrap();
    image::save_buffer(
        dir.join("cell-source.png"),
        &base.rgb_bytes(),
        384,
        224,
        image::ColorType::Rgb8,
    )
    .unwrap();
    let mut log = String::from("stride,combo,tell,shove,stop,seats,winners,ticks,work,falls\n");
    for values in [(1, 20, 36, 8, 4), (2, 16, 30, 10, 3), (3, 12, 27, 12, 2)] {
        for seats in [1, 2, 4, 2, 4] {
            let mut r = game(seats);
            r.tune.brass_stride = values.0;
            r.tune.brass_combo = values.1;
            r.tune.brass_tell = values.2;
            r.tune.brass_shove = values.3;
            r.tune.brass_stop = values.4;
            for tick in 0..LENGTH {
                let s = r.brasswake.as_ref().unwrap();
                if s.left == 0 {
                    break;
                }
                let inputs = s
                    .lanes
                    .iter()
                    .map(|(&id, d)| (id, d.bot_input(tick)))
                    .collect();
                r.step(&inputs);
                if values.0 == 2 && seats == 4 && tick == 600 {
                    for id in 1..=4 {
                        save(&format!("live-seat-{id}"), &r, id);
                    }
                }
            }
            let s = r.brasswake.as_ref().unwrap();
            let d = &s.lanes[&s.order[0]];
            log.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                values.0,
                values.1,
                values.2,
                values.3,
                values.4,
                seats,
                s.lanes.values().filter(|d| d.finish.is_some()).count(),
                d.finish.unwrap_or(LENGTH),
                d.score,
                d.falls
            ));
        }
    }
    std::fs::write(dir.join("bot-tuning.csv"), log).unwrap();
}

#[test]
fn brasswake_five_knobs_change_their_named_feel_without_weakening_foes() {
    let t = Tune::default();
    for (name, value) in [
        ("brass_stride", 4.0),
        ("brass_combo", 30.0),
        ("brass_tell", 60.0),
        ("brass_shove", 18.0),
        ("brass_stop", 8.0),
    ] {
        let mut moved = t;
        assert!(moved.set(name, value));
        assert!(moved.valid());
        let mut a = close();
        let mut b = a.clone();
        let input = if name == "brass_stride" {
            Input {
                move_x: 1,
                ..Default::default()
            }
        } else {
            Input {
                fire: name != "brass_tell",
                ..Default::default()
            }
        };
        a.step_at(input, &t, 100, 0, 0);
        b.step_at(input, &moved, 100, 0, 0);
        match name {
            "brass_stride" => assert_ne!(a.x, b.x),
            "brass_combo" => assert_ne!(a.cooldown, b.cooldown),
            "brass_tell" => assert_ne!(a.foes[0].ticks, b.foes[0].ticks),
            "brass_shove" => assert_ne!(a.foes[0].x, b.foes[0].x),
            _ => assert_ne!(a.freeze, b.freeze),
        }
        assert_eq!(a.foes[0].hp, b.foes[0].hp);
    }
}

#[test]
fn brasswake_a_late_seat_gets_its_own_aisle_and_bodies_stay_inside_gates() {
    let mut r = game(1);
    r.join_seat(3, "Lyra");
    r.step(&BTreeMap::from([(
        3,
        Input {
            move_x: 1,
            ..Default::default()
        },
    )]));
    let s = r.brasswake.as_ref().unwrap();
    assert!(s.lanes[&3].x > s.lanes[&1].x);
    assert_eq!(s.lanes[&3].seed, s.seed);
    let t = Tune::default();
    let mut d = Lane::new(41);
    for tick in 0..120 {
        d.step_at(
            Input {
                move_x: -1,
                ..Default::default()
            },
            &t,
            tick,
            0,
            tick as u32,
        );
    }
    assert_eq!(d.x, 24);
    d.x = 239;
    for e in &mut d.foes {
        e.x = 239;
        e.beat = Beat::Rest;
        e.ticks = 60;
    }
    d.fire_key = false;
    d.step_at(
        Input {
            fire: true,
            ..Default::default()
        },
        &t,
        121,
        0,
        121,
    );
    assert!(d.foes[0].x <= 216);
}
