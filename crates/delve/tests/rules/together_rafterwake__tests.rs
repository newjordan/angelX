use super::{BTreeMap, Input, Run, Spoil};
use crate::rules::{rafterwake::*, tune::Tune};
fn game(seats: u32) -> Run {
    let mut r = Run::rafterwake_for_test();
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
    assert!(r.rafterwake.is_some());
    // These tests are about the climb: the title and the 3-2-1 have their own tests below.
    r.rafterwake.as_mut().unwrap().go = 0;
    r
}
fn step(d: &mut Lane, i: Input, tick: u64) {
    d.step_at(i, &Tune::default(), tick, 11, tick as u32);
}
#[test]
fn rafterwake_same_seed_independent_contacts_and_camera() {
    let mut r = game(4);
    let seed = r.rafterwake.as_ref().unwrap().seed;
    for t in 0..70 {
        let d = &r.rafterwake.as_ref().unwrap().lanes[&1];
        let input = d.bot_input(t);
        r.step(&BTreeMap::from([(1, input)]));
    }
    let s = r.rafterwake.as_ref().unwrap();
    assert!(s.lanes[&1].height > 0);
    assert_eq!(s.lanes[&2].height, 0);
    assert_eq!(s.lanes[&2], s.lanes[&3]);
    assert!(s.lanes.values().all(|d| d.seed == seed));
    assert!(r.valid_snapshot());
}
#[test]
fn rafterwake_short_release_coyote_and_buffer_are_physical() {
    let mut full = Lane::new(41);
    let mut short = full.clone();
    let mut a = FLOOR;
    let mut b = FLOOR;
    for tick in 0..40 {
        step(
            &mut full,
            Input {
                fire: tick < 20,
                ..Default::default()
            },
            tick,
        );
        step(
            &mut short,
            Input {
                fire: tick < 2,
                ..Default::default()
            },
            tick,
        );
        a = a.min(full.y / Q);
        b = b.min(short.y / Q);
    }
    assert!(a + 12 < b, "full {a}, short {b}");
    let mut late = Lane::new(41);
    late.ground = None;
    late.coyote = 2;
    late.y -= Q;
    step(
        &mut late,
        Input {
            fire: true,
            ..Default::default()
        },
        0,
    );
    assert!(late.vy < 0);
    let mut early = Lane::new(41);
    early.ground = None;
    early.coyote = 0;
    early.y -= Q;
    early.vy = 256;
    step(
        &mut early,
        Input {
            fire: true,
            ..Default::default()
        },
        0,
    );
    assert!(early.ground.is_some());
    for tick in 1..5 {
        step(&mut early, Input::default(), tick);
    }
    assert!(early.vy < 0, "buffered press springs after contact");
}
#[test]
fn rafterwake_sampled_jump_and_climb_use_shared_allowance_edges() {
    let allowance = angel_net::cmd::sample_allowance(
        std::time::Duration::from_secs(60),
        crate::rules::HZ,
        1,
        false,
    );
    assert_eq!(allowance, 11);
    for delay in [allowance, allowance + 1] {
        let mut d = Lane::new(41);
        d.step_at(Input::default(), &Tune::default(), 100, allowance, 0);
        d.ground = None;
        d.coyote = 0;
        d.x = 70 * Q;
        d.y = 500 * Q;
        d.vy = 0;
        d.step_at(
            Input {
                fire: true,
                latch_tick: Some(100),
                ..Default::default()
            },
            &Tune::default(),
            100 + delay,
            allowance,
            delay as u32,
        );
        assert_eq!(d.vy < 0, delay == allowance);
    }
    for delay in [allowance, allowance + 1] {
        let mut d = Lane::new(41);
        d.x = 120 * Q;
        d.step_at(Input::default(), &Tune::default(), 100, allowance, 0);
        d.x = 170 * Q;
        d.y = 530 * Q;
        d.step_at(
            Input {
                move_y: -1,
                latch_tick: Some(100),
                ..Default::default()
            },
            &Tune::default(),
            100 + delay,
            allowance,
            delay as u32,
        );
        assert_eq!(d.climbing, delay == allowance);
    }
}
#[test]
fn rafterwake_lifts_carry_boots_and_ladders_have_end_contacts() {
    let mut d = Lane::new(41);
    let p = planks(41, 0)[2];
    d.x = (p.x + p.w / 2) * Q;
    d.y = p.y * Q;
    d.ground = Some(2);
    d.checkpoint = 2;
    let x = d.x;
    for tick in 1..12 {
        step(&mut d, Input::default(), tick);
    }
    assert_ne!(d.x, x);
    assert_eq!(d.y, p.y * Q);
    assert_eq!(d.ground, Some(2));
    let mut c = Lane::new(41);
    c.x = 120 * Q;
    for tick in 0..30 {
        step(
            &mut c,
            Input {
                move_y: -1,
                ..Default::default()
            },
            tick,
        );
    }
    assert!(c.checkpoint >= 1);
}
#[test]
fn rafterwake_boss_tells_mark_exact_danger_and_ramp_only_in_rage() {
    for hp in [6, 4, 2] {
        for sweep in [false, true] {
            let mut k = Keeper::new();
            k.hp = hp;
            k.attack = u32::from(sweep);
            k.ticks = 0;
            k.step(90);
            assert_eq!(k.beat, Beat::Tell);
            assert!(k.ticks >= 23);
            assert_eq!(k.sweep, sweep);
            let z = k.danger;
            assert!(z.contains(if sweep { 90 } else { z.x + 24 }, TOP));
            assert!(!z.contains(
                if sweep { 90 } else { z.x - 1 },
                if sweep { TOP - 25 } else { TOP }
            ));
            let ticks = k.ticks;
            for _ in 1..ticks {
                assert!(!k.step(90));
            }
            assert!(k.step(90));
            assert_eq!(k.beat, Beat::Attack);
        }
    }
}
#[test]
fn rafterwake_fall_restarts_under_a_second_and_never_loses_progress() {
    let mut d = Lane::new(41);
    d.height = 216;
    d.checkpoint = 6;
    d.y = 500 * Q;
    step(&mut d, Input::default(), 0);
    assert_eq!(d.falls, 1);
    assert!(d.down > 0);
    for tick in 1..28 {
        step(&mut d, Input::default(), tick);
    }
    assert_eq!(d.down, 0);
    assert_eq!(d.height, 216);
    assert!(d.valid());
}
#[test]
fn rafterwake_purse_once_ties_record_crown_and_one_tick_rematch() {
    for seats in [1, 2, 4] {
        let mut r = game(seats);
        let before = r
            .players
            .values()
            .map(|h| h.carried.get(Spoil::Gold))
            .sum::<u32>();
        let s = r.rafterwake.as_mut().unwrap();
        s.left = 1;
        for d in s.lanes.values_mut() {
            d.score = 1000;
            d.finish = Some(1800);
        }
        r.step(&BTreeMap::new());
        let after = r
            .players
            .values()
            .map(|h| h.carried.get(Spoil::Gold))
            .sum::<u32>();
        assert_eq!(after - before, 10 * seats);
        assert!(r.players.values().all(|h| h.laurel));
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
        assert_eq!(r.home.countryside.games.rafterwake.best, 1000);
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        assert_eq!(r.rafterwake.as_ref().unwrap().left, LENGTH);
    }
}
#[test]
fn rafterwake_bad_save_repairs_and_tiebreak_uses_same_metric() {
    let mut s = Session::new(1..=4, 41);
    s.lanes.get_mut(&1).unwrap().x = i32::MAX;
    assert!(!s.valid());
    s.repair();
    assert!(s.valid());
    s.lanes.get_mut(&1).unwrap().score = 100;
    s.lanes.get_mut(&2).unwrap().score = 100;
    s.lanes.get_mut(&1).unwrap().falls = 1;
    assert!(s.score(2) < s.score(1));
}
#[test]
fn rafterwake_bots_can_finish_and_idle_can_lose() {
    for seats in [1, 2, 4, 2, 4] {
        let mut r = game(seats);
        for t in 0..LENGTH {
            let s = r.rafterwake.as_ref().unwrap();
            if s.left == 0 {
                break;
            }
            let inputs = s
                .lanes
                .iter()
                .map(|(&id, d)| (id, d.bot_input(t)))
                .collect();
            r.step(&inputs);
        }
        let s = r.rafterwake.as_ref().unwrap();
        assert!(
            s.won,
            "{:?}",
            s.lanes
                .values()
                .map(|d| (d.checkpoint, d.x / Q, d.y / Q, d.keeper.hp, d.falls))
                .collect::<Vec<_>>()
        );
        assert!(r.valid_snapshot());
    }
    let mut r = game(2);
    for _ in 0..LENGTH {
        r.step(&BTreeMap::new());
    }
    assert!(!r.rafterwake.as_ref().unwrap().won);
}
#[test]
#[ignore = "writes native whole-tower and state review images"]
fn write_rafterwake_review() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    for n in [
        "entrance",
        "bottom",
        "middle",
        "climb",
        "jump",
        "fall",
        "land",
        "hit",
        "idle",
        "walk-1",
        "walk-2",
        "drop-tell",
        "drop",
        "sweep-tell",
        "sweep",
        "flinch",
        "damage-1",
        "damage-2",
        "defeat",
        "recover",
        "podium",
        "tower-0",
        "tower-144",
        "tower-288",
        "tower-408",
    ] {
        let mut r = Run::rafterwake_review_for_test(n);
        if n.starts_with("walk-") {
            r.rafterwake
                .as_mut()
                .unwrap()
                .lanes
                .get_mut(&1)
                .unwrap()
                .keeper
                .beat = Beat::Walk;
            r.tick = if n == "walk-1" { 0 } else { 7 };
        }
        let im = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1));
        image::save_buffer(
            dir.join(format!("{n}.png")),
            &im.rgb_bytes(),
            384,
            224,
            image::ColorType::Rgb8,
        )
        .unwrap();
    }
    for attack in ["drop", "sweep"] {
        for (stage, ticks) in [33, 24, 16, 7].into_iter().enumerate() {
            let mut r = Run::rafterwake_review_for_test(&format!("{attack}-tell"));
            r.rafterwake
                .as_mut()
                .unwrap()
                .lanes
                .get_mut(&1)
                .unwrap()
                .keeper
                .ticks = ticks;
            let im = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1));
            image::save_buffer(
                dir.join(format!("{attack}-tell-{stage}.png")),
                &im.rgb_bytes(),
                384,
                224,
                image::ColorType::Rgb8,
            )
            .unwrap();
        }
    }
    for (name, score) in [
        ("standings-first", 432),
        ("standings-third", 180),
        ("standings-fourth", 90),
        ("standings-tie", 288),
    ] {
        let mut r = Run::rafterwake_review_for_test("idle");
        r.rafterwake
            .as_mut()
            .unwrap()
            .lanes
            .get_mut(&1)
            .unwrap()
            .score = score;
        let im = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1));
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgb_bytes(),
            384,
            224,
            image::ColorType::Rgb8,
        )
        .unwrap();
    }
    for name in ["drop-overlap", "land-dust", "climb-dust"] {
        let mut r = Run::rafterwake_review_for_test(if name == "drop-overlap" {
            "drop"
        } else if name == "land-dust" {
            "land"
        } else {
            "climb"
        });
        let d = r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap();
        if name == "drop-overlap" {
            d.x = (d.keeper.x - 8) * Q;
        } else {
            d.effect = 5;
            d.effect_x = d.x / Q;
            d.effect_y = d.y / Q;
        }
        let im = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1));
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgb_bytes(),
            384,
            224,
            image::ColorType::Rgb8,
        )
        .unwrap();
    }
}

#[test]
#[ignore = "five bot rounds for each feel proposal, plus per-seat live captures"]
fn write_rafterwake_tuning() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut log =
        String::from("gravity,jump,run,coyote,buffer,seats,finish_ticks,height,falls,keeper_hp\n");
    for values in [
        (144, 1536, 512, 2, 3),
        (96, 1664, 768, 5, 6),
        (112, 1536, 640, 4, 5),
    ] {
        for seats in [1, 2, 4, 2, 4] {
            let mut r = game(seats);
            for (k, v) in [
                ("rafter_gravity", values.0),
                ("rafter_jump", values.1),
                ("rafter_run", values.2),
                ("rafter_coyote", values.3),
                ("rafter_buffer", values.4),
            ] {
                assert!(r.tune.set(k, f64::from(v)));
            }
            for t in 0..LENGTH {
                let s = r.rafterwake.as_ref().unwrap();
                if s.left == 0 {
                    break;
                }
                let inputs = s
                    .lanes
                    .iter()
                    .map(|(&id, d)| (id, d.bot_input(t)))
                    .collect();
                r.step(&inputs);
                if values.0 == 112 && seats == 4 && [120, 360, 480].contains(&t) {
                    for id in 1..=4 {
                        let im = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(id));
                        image::save_buffer(
                            dir.join(format!("live-{t}-seat-{id}.png")),
                            &im.rgb_bytes(),
                            384,
                            224,
                            image::ColorType::Rgb8,
                        )
                        .unwrap();
                    }
                }
            }
            let s = r.rafterwake.as_ref().unwrap();
            let d = &s.lanes[&1];
            log.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                values.0,
                values.1,
                values.2,
                values.3,
                values.4,
                seats,
                d.finish.unwrap_or(LENGTH),
                d.height,
                d.falls,
                d.keeper.hp
            ));
        }
    }
    std::fs::write(dir.join("feel-tuning.csv"), log).unwrap();
}

#[test]
fn rafterwake_bot_climbs_from_delayed_snapshots() {
    use std::collections::VecDeque;
    for seed in [0, 1, 2, 3, 41, 166] {
        for delay in [0, 1, 3, 6, 8, 11] {
            let mut d = Lane::new(seed);
            let mut seen = VecDeque::new();
            for tick in 0..LENGTH {
                seen.push_back((tick, d.clone()));
                if seen.len() > delay + 1 {
                    seen.pop_front();
                }
                let (sample, view) = seen.front().unwrap();
                let mut input = view.bot_input(*sample);
                if input.fire || input.move_y != 0 {
                    input.latch_tick = Some(u64::from(*sample));
                }
                d.step_at(input, &Tune::default(), u64::from(tick), 11, tick);
                if d.finish.is_some() {
                    break;
                }
            }
            assert!(
                d.finish.is_some(),
                "seed {seed}, delay {delay}: deck {}, x {}, y {}, hp {}, fire {}",
                d.checkpoint,
                d.x / Q,
                d.y / Q,
                d.keeper.hp,
                d.fire_key
            );
        }
    }
}

#[test]
fn rafterwake_podium_columns_end_at_their_headers() {
    use crate::render::overworld::{arena, ink::text_width};
    let r = Run::rafterwake_review_for_test("podium");
    let labels = arena::rafterwake_labels(&r, Some(1));
    for y in [64, 88, 112, 136] {
        let values = labels.iter().filter(|(x, yy, _)| *yy == y && *x >= 148)
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 3);
        for ((x, _, value), (start, header)) in values.into_iter().zip([
            (148, "PROGRESS"), (212, "FALLS"), (274, "FINISH")
        ]) {
            assert_eq!(x + text_width(value), start + text_width(header));
        }
    }
    for text in ["40G POT + CROWN", "MOST HEIGHT, FEWER FALLS, EARLIER FINISH"] {
        let (x, _, value) = labels.iter().find(|(_, _, v)| v == text).unwrap();
        assert!((2*x + text_width(value) - 384).abs() <= 1);
    }
}

/// Every knight waits out the title and the 3-2-1 together and starts on one tick.
#[test]
fn rafterwake_everyone_starts_together_after_the_count() {
    let mut r = game(4);
    r.rafterwake.as_mut().unwrap().go = INTRO;
    let run_right = Input { move_x: 1, fire: true, ..Default::default() };
    let keys: BTreeMap<u32, Input> = (1..=4).map(|id| (id, run_right)).collect();
    for n in 0..INTRO {
        let s = r.rafterwake.as_ref().unwrap();
        assert_eq!(s.go, INTRO - n);
        assert_eq!(s.left, LENGTH, "the clock waits for the count");
        assert!(s.lanes.values().all(|d| d.x == 42 * Q), "nobody moves before GO");
        r.step(&keys);
    }
    let s = r.rafterwake.as_ref().unwrap();
    assert_eq!(s.go, 0);
    assert_eq!(s.left, LENGTH);
    r.step(&keys);
    let s = r.rafterwake.as_ref().unwrap();
    assert_eq!(s.left, LENGTH - 1, "the first tick of the round");
    // Every seat started the same tick, so every seat moved the same way.
    assert!(s.lanes.values().all(|d| d.x == s.lanes[&1].x && d.x > 42 * Q));
    assert!(r.valid_snapshot());
}

/// A button held through the count is not a press the moment the round starts.
#[test]
fn rafterwake_a_press_held_through_the_count_is_not_a_hop() {
    let mut r = game(2);
    r.rafterwake.as_mut().unwrap().go = 6;
    let held = Input { fire: true, ..Default::default() };
    for _ in 0..8 {
        r.step(&BTreeMap::from([(1, held)]));
    }
    let d = &r.rafterwake.as_ref().unwrap().lanes[&1];
    assert_eq!(d.vy, 0);
    assert!(d.ground.is_some(), "still on the plank: the held button did not hop");
}

/// The crew bell rings as the crew's average deck passes 3, 6 and 9: once each, with high hops,
/// gold for the pot and a call the whole table hears.
#[test]
fn rafterwake_crew_bell_rings_at_three_six_nine_and_pays_the_pot() {
    let mut r = game(4);
    let base = 10 * 4;
    assert_eq!(r.rafterwake.as_ref().unwrap().pot(), base);
    for (n, deck) in [3usize, 6, 9].into_iter().enumerate() {
        for d in r.rafterwake.as_mut().unwrap().lanes.values_mut() {
            d.checkpoint = deck;
        }
        r.step(&BTreeMap::new());
        let s = r.rafterwake.as_ref().unwrap();
        assert_eq!(s.bells as usize, n + 1, "deck {deck}");
        assert_eq!(s.bell, BELL_LIFE - 0, "the bell's hops last their time");
        assert_eq!(s.pot(), base + BELL_GOLD * (n as u32 + 1));
        assert!(s.show.list.iter().any(|c| c.cry == show::Cry::Bell(n as u8 + 1)));
        // Another tick at the same decks rings nothing new.
        r.step(&BTreeMap::new());
        assert_eq!(r.rafterwake.as_ref().unwrap().bells as usize, n + 1);
    }
    assert!(r.valid_snapshot());
}

/// A knight who is not the whole crew's average does not ring the bell alone.
#[test]
fn rafterwake_one_fast_knight_does_not_ring_the_crew_bell() {
    let mut r = game(4);
    r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().checkpoint = 8;
    r.step(&BTreeMap::new());
    assert_eq!(r.rafterwake.as_ref().unwrap().bells, 0);
}

/// High hops: the same press clears a little more air while the bell rings.
#[test]
fn rafterwake_bell_hops_a_little_higher() {
    let jump = |boost: bool| {
        let mut d = Lane::new(41);
        let hop = Input { fire: true, ..Default::default() };
        d.step_boosted(hop, &Tune::default(), 0, 11, 0, boost);
        let mut top = 0;
        for t in 1..40 {
            d.step_boosted(hop, &Tune::default(), t, 11, t as u32, boost);
            top = top.max(FLOOR - d.y / Q);
        }
        top
    };
    assert!(jump(true) > jump(false), "{} vs {}", jump(true), jump(false));
}

/// The show's marks: a hop, a landing, a deck won and a keeper struck are written on the tick
/// they happen, and a heavy one holds the room's time.
#[test]
fn rafterwake_marks_name_what_happened_and_a_strike_holds_the_room() {
    use show::Kind;
    let mut d = Lane::new(41);
    let hop = Input { fire: true, ..Default::default() };
    let mut seen = Vec::new();
    for t in 0..60 {
        d.step_at(if t < 14 { hop } else { Input::default() }, &Tune::default(), t, 11, t as u32);
        seen.extend(d.show.marks.iter().filter(|m| m.born == t as u32).map(|m| m.kind));
    }
    assert!(seen.contains(&Kind::Hop), "{seen:?}");
    assert!(seen.iter().any(|k| matches!(k, Kind::Land { .. })), "{seen:?}");
    // The keeper struck from above: a mark, heavy, and the run's hit-stop is asked for.
    let mut r = game(2);
    let s = r.rafterwake.as_mut().unwrap();
    let d = s.lanes.get_mut(&1).unwrap();
    d.height = 400;
    d.checkpoint = 12;
    d.ground = None;
    d.x = d.keeper.x * Q;
    d.y = (TOP - 30) * Q;
    d.vy = -300;
    d.keeper.beat = Beat::Rest;
    d.keeper.ticks = 30;
    r.step(&BTreeMap::new());
    let s = r.rafterwake.as_ref().unwrap();
    let d = &s.lanes[&1];
    assert!(d.show.marks.iter().any(|m| matches!(m.kind, Kind::Strike { left: 5 })), "{:?}", d.show.marks);
    assert!(r.hitstop.left > 0 || r.hitstop.held, "a crushing hit holds the room");
}

/// The lead changes hands only with a margin, once, and is called to the table.
#[test]
fn rafterwake_lead_is_called_once_with_a_margin() {
    let mut c = show::Calls::default();
    c.standings(10, &[(1, 100), (2, 60)]);
    assert!(c.list.is_empty(), "the first leader is not a change");
    c.standings(20, &[(1, 100), (2, 110)]);
    assert!(c.list.is_empty(), "a margin under a deck is no lead");
    c.standings(30, &[(1, 100), (2, 140)]);
    assert_eq!(c.list.len(), 1);
    assert_eq!((c.list[0].cry, c.list[0].who), (show::Cry::Lead, 2));
    c.standings(31, &[(1, 100), (2, 150)]);
    assert_eq!(c.list.len(), 1, "no second call for the same leader");
    // A fall is called, then not again at once.
    c.fell(3, 40);
    c.fell(3, 50);
    assert_eq!(c.list.iter().filter(|c| c.cry == show::Cry::Fell).count(), 1);
    assert!(c.valid());
}

/// Snapshots written before the show have none of its fields and still load.
#[test]
fn rafterwake_old_snapshots_load_and_a_quiet_round_writes_nothing_new() {
    let r = game(2);
    let s = r.rafterwake.as_ref().unwrap().clone();
    let mut value = serde_json::to_value(&s).unwrap();
    for key in ["go", "bell", "bells", "ended", "was", "show"] {
        value.as_object_mut().unwrap().remove(key);
    }
    for lane in value["lanes"].as_object_mut().unwrap().values_mut() {
        lane.as_object_mut().unwrap().remove("show");
    }
    let old: Session = serde_json::from_value(value).unwrap();
    assert!(old.valid());
    assert_eq!(old.go, 0);
    let text = serde_json::to_string(&old).unwrap();
    for key in ["\"go\"", "\"bell\"", "\"bells\"", "\"ended\"", "\"was\""] {
        assert!(!text.contains(key), "{key} is written for a quiet round");
    }
}
