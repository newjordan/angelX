use super::{BTreeMap, Input, Run, Spoil};
use crate::rules::{pennon::*, tune::Tune};
fn game(seats: u32) -> Run {
    let mut r = Run::pennon_for_test();
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
    assert!(r.pennon.is_some());
    r
}
fn step(d: &mut Lane, i: Input, tick: u64) {
    d.step_at(i, &Tune::default(), tick, 11, tick as u32);
}
#[test]
fn pennon_every_seat_owns_contacts_shots_and_camera() {
    let mut r = game(4);
    for t in 0..100 {
        let i = r.pennon.as_ref().unwrap().lanes[&1].bot_input(t);
        r.step(&BTreeMap::from([(1, i)]));
    }
    let s = r.pennon.as_ref().unwrap();
    assert!(s.lanes[&1].height > 0);
    assert_eq!(s.lanes[&2].height, 0);
    assert_eq!(s.lanes[&2], s.lanes[&3]);
    assert!(s.lanes.values().all(|d| d.seed == s.seed));
    assert!(r.valid_snapshot());
}
#[test]
fn pennon_short_release_coyote_buffer_and_gap_contacts() {
    let mut full = Lane::new(41);
    let mut short = full.clone();
    let (mut a, mut b) = (FLOOR, FLOOR);
    for tick in 0..35 {
        step(
            &mut full,
            Input {
                interact: tick < 24,
                ..Default::default()
            },
            tick,
        );
        step(
            &mut short,
            Input {
                interact: tick < 2,
                ..Default::default()
            },
            tick,
        );
        a = a.min(full.y / Q);
        b = b.min(short.y / Q);
    }
    assert!(a < b - 16, "held jump must be physically higher: {a} {b}");
    let mut late = Lane::new(41);
    late.ground = None;
    late.coyote = 1;
    late.y -= Q;
    step(
        &mut late,
        Input {
            interact: true,
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
            interact: true,
            ..Default::default()
        },
        0,
    );
    assert!(early.ground.is_some());
    for t in 1..5 {
        step(&mut early, Input::default(), t);
    }
    assert!(early.vy < 0);
    let ps = ramparts(41);
    assert!(
        crate::rules::rafterwake::landing(
            (FLOOR - 1) * Q,
            (FLOOR + 1) * Q,
            ps[0].x + ps[0].w + 20,
            &ps
        )
        .is_none()
    );
}
#[test]
fn pennon_sampled_shot_and_jump_include_allowance_edge_and_reject_expiry() {
    let allowance = angel_net::cmd::sample_allowance(
        std::time::Duration::from_secs(2),
        crate::rules::HZ,
        1,
        false,
    );
    assert_eq!(allowance, 11);
    for age in [11, 12] {
        let mut jump = Lane::new(41);
        let end = ramparts(41)[0].x + ramparts(41)[0].w;
        jump.x = (end - 2) * Q;
        step(&mut jump, Input::default(), 100);
        jump.ground = None;
        jump.coyote = 0;
        jump.x = (end + 20) * Q;
        jump.y += 2 * Q;
        step(
            &mut jump,
            Input {
                interact: true,
                latch_tick: Some(100),
                ..Default::default()
            },
            100 + age,
        );
        assert_eq!(jump.vy < 0, age == 11);
        let mut shot = Lane::new(41);
        shot.x = 70 * Q;
        step(&mut shot, Input::default(), 100);
        shot.x = 200 * Q;
        step(
            &mut shot,
            Input {
                fire: true,
                latch_tick: Some(100),
                ..Default::default()
            },
            100 + age,
        );
        assert_eq!(
            shot.foes[0].hp < 3,
            age == 11,
            "swept sampled muzzle should hit the body seen at the input tick"
        );
    }
}
#[test]
fn pennon_sparse_aimed_shot_has_locked_telegraph_and_duck_contact() {
    let mut f = Foe::new(158, false);
    f.ticks = 1;
    assert!(!f.step(70, FLOOR));
    let aim = f.aim;
    assert_eq!(f.beat, Beat::Tell);
    assert!(f.ticks >= 23);
    while f.beat == Beat::Tell {
        f.step(110, FLOOR - 30);
        assert_eq!(f.aim, aim);
    }
    assert_eq!(f.beat, Beat::Attack);
    assert!(segment_hits(
        (100 * Q, 82 * Q),
        (140 * Q, 82 * Q),
        (118, FLOOR - 37, 16, 37)
    ));
    assert!(!segment_hits(
        (100 * Q, 82 * Q),
        (140 * Q, 82 * Q),
        (118, FLOOR - 19, 16, 19)
    ));
    assert!(segment_hits(
        (70 * Q, 60 * Q),
        (160 * Q, 110 * Q),
        (120, 80, 16, 20)
    ));
}
#[test]
fn pennon_captain_tells_are_exact_distinct_and_phase_ramped() {
    for hp in [18, 12, 6] {
        let mut f = Foe::new(1130, true);
        f.hp = hp;
        f.ticks = 1;
        f.step(1000, FLOOR);
        let area = f.danger;
        let ticks = f.ticks;
        assert!(ticks >= 23);
        for _ in 0..ticks - 1 {
            assert!(!f.step(1080, FLOOR));
            assert_eq!(f.danger, area);
        }
        assert!(f.step(1080, FLOOR));
        assert_eq!(f.beat, Beat::Attack);
        assert!(!f.hook);
        assert_eq!(area.w, 52);
        f.beat = Beat::Walk;
        f.ticks = 1;
        f.step(1000, FLOOR);
        assert!(f.hook);
        assert_eq!(f.danger.h, 23);
        assert_ne!(f.danger, area);
    }
}
#[test]
fn pennon_fall_recovers_under_a_second_and_preserves_work() {
    let mut d = Lane::new(41);
    d.height = 400;
    d.checkpoint = 1;
    d.ground = None;
    d.y = (FLOOR + 72) * Q;
    step(&mut d, Input::default(), 0);
    assert_eq!(d.falls, 1);
    for t in 1..=22 {
        step(&mut d, Input::default(), t);
    }
    assert_eq!(d.down, 0);
    assert_eq!(d.ground, Some(1));
    assert!(d.height >= 400);
}
#[test]
fn pennon_purse_once_tie_record_buff_and_one_tick_rematch() {
    for seats in [1, 2, 4] {
        let mut r = game(seats);
        let before = r
            .players
            .values()
            .map(|h| h.carried.get(Spoil::Gold))
            .sum::<u32>();
        let s = r.pennon.as_mut().unwrap();
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
        assert_eq!(r.home.countryside.games.pennon.best, 1000);
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        assert_eq!(r.pennon.as_ref().unwrap().left, LENGTH);
    }
}
#[test]
fn pennon_save_repair_and_live_rank_use_podium_metric() {
    let mut s = Session::new(1..=4, 41);
    s.lanes.get_mut(&1).unwrap().score = 500;
    s.lanes.get_mut(&2).unwrap().score = 500;
    s.lanes.get_mut(&1).unwrap().falls = 1;
    assert_eq!(s.place(2), 1);
    assert_eq!(s.place(1), 2);
    s.lanes.get_mut(&2).unwrap().score = 1001;
    assert!(!s.valid());
    s.repair();
    assert!(s.valid());
    let json = serde_json::to_string(&s).unwrap();
    assert!(!json.contains("history"));
    let back: Session = serde_json::from_str(&json).unwrap();
    assert!(back.valid());
}
#[test]
fn pennon_bots_finish_seeded_stage_and_idle_loses() {
    for seed in [7, 41, 299] {
        let mut d = Lane::new(seed);
        for tick in 0..LENGTH {
            let input = d.bot_input(tick);
            step(&mut d, input, tick as u64);
            assert!(d.valid(), "{seed}/{tick}: {d:?}");
            if d.finish.is_some() {
                break;
            }
        }
        assert!(d.finish.is_some(), "{seed}: {d:?}");
    }
    let mut r = game(2);
    for _ in 0..LENGTH {
        r.step(&BTreeMap::new());
    }
    assert!(!r.pennon.as_ref().unwrap().won);
}
#[test]
#[ignore = "writes native Pennon Rush review scenes"]
fn write_pennon_review() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    for name in [
        "entrance",
        "start",
        "sentry",
        "idle",
        "jump",
        "fall",
        "land",
        "hit",
        "duck",
        "aim-up",
        "aim-down",
        "shoot",
        "drop-tell",
        "drop",
        "hook-tell",
        "hook",
        "flinch",
        "damage-1",
        "damage-2",
        "defeat",
        "recover",
        "podium",
        "rampart-0",
        "rampart-254",
        "rampart-508",
        "rampart-762",
        "rampart-946",
    ] {
        let r = Run::pennon_review_for_test(name);
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
    for kind in ["drop", "hook", "flinch", "land"] {
        for n in 0..4 {
            let r = Run::pennon_fx_review_for_test(kind, n);
            let im = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1));
            image::save_buffer(
                dir.join(format!("fx-{kind}-{n}.png")),
                &im.rgb_bytes(),
                384,
                224,
                image::ColorType::Rgb8,
            )
            .unwrap();
        }
    }
    for attack in ["drop", "hook"] {
        for (n, ticks) in [33, 24, 16, 7].into_iter().enumerate() {
            let mut r = Run::pennon_review_for_test(&format!("{attack}-tell"));
            r.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap().foes[3].ticks = ticks;
            let im = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1));
            image::save_buffer(
                dir.join(format!("{attack}-tell-{n}.png")),
                &im.rgb_bytes(),
                384,
                224,
                image::ColorType::Rgb8,
            )
            .unwrap();
        }
    }
}
#[test]
fn pennon_bot_handles_delayed_snapshots() {
    use std::collections::VecDeque;
    for seed in [0, 1, 2, 41] {
        for delay in [0, 3, 6, 11] {
            let mut d = Lane::new(seed);
            let mut seen = VecDeque::new();
            for tick in 0..LENGTH {
                seen.push_back((tick, d.clone()));
                if seen.len() > delay + 1 {
                    seen.pop_front();
                }
                let (sample, view) = seen.front().unwrap();
                let mut input = view.bot_input(*sample);
                if input.fire || input.interact {
                    input.latch_tick = Some(u64::from(*sample));
                }
                d.step_at(input, &Tune::default(), u64::from(tick), 11, tick);
                if d.finish.is_some() {
                    break;
                }
            }
            assert!(
                d.finish.is_some(),
                "seed {seed}, delay {delay}: x {}, falls {}, hp {:?}",
                d.x / Q,
                d.falls,
                d.foes.iter().map(|f| f.hp).collect::<Vec<_>>()
            );
        }
    }
}
#[test]
#[ignore = "five bot rounds per feel proposal and per-seat native live captures"]
fn write_pennon_tuning() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut log = String::from(
        "gravity,jump,run,coyote,buffer,seats,finish_ticks,progress,falls,captain_hp\n",
    );
    for values in [
        (144, 1536, 512, 2, 3),
        (96, 1664, 768, 5, 6),
        (112, 1536, 640, 4, 5),
    ] {
        for seats in [1, 2, 4, 2, 4] {
            let mut r = game(seats);
            for (k, v) in [
                ("pennon_gravity", values.0),
                ("pennon_jump", values.1),
                ("pennon_run", values.2),
                ("pennon_coyote", values.3),
                ("pennon_buffer", values.4),
            ] {
                assert!(r.tune.set(k, f64::from(v)));
            }
            for t in 0..LENGTH {
                let s = r.pennon.as_ref().unwrap();
                if s.left == 0 {
                    break;
                }
                let inputs = s
                    .lanes
                    .iter()
                    .map(|(&id, d)| (id, d.bot_input(t)))
                    .collect();
                r.step(&inputs);
                if values.0 == 112 && seats == 4 && [120, 360, 600].contains(&t) {
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
            let d = &r.pennon.as_ref().unwrap().lanes[&1];
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
                d.foes[3].hp
            ));
        }
    }
    std::fs::write(dir.join("feel-tuning.csv"), log).unwrap();
}
#[test]
fn pennon_awakened_tell_keeps_time_through_retreat_and_distant_foes_ignore_shots() {
    let mut d = Lane::new(41);
    for f in &mut d.foes[..3] {
        f.hp = 0;
        f.beat = Beat::Defeat;
    }
    d.x = 975 * Q;
    d.ground = Some(3);
    d.foes[3].ticks = 1;
    step(&mut d, Input::default(), 0);
    let area = d.foes[3].danger;
    assert_eq!(d.foes[3].beat, Beat::Tell);
    d.x = 900 * Q;
    for tick in 1..=45 {
        step(&mut d, Input::default(), tick);
    }
    assert_eq!(d.foes[3].beat, Beat::Rest);
    assert_eq!(d.foes[3].danger, area);
    let mut far = Lane::new(41);
    for f in &mut far.foes[..3] {
        f.hp = 0;
        f.beat = Beat::Defeat;
    }
    far.x = 600 * Q;
    far.shots.push(Shot {
        x: 1130 * Q,
        y: 82 * Q,
        vx: 0,
        vy: 0,
        enemy: false,
        life: 10,
    });
    step(&mut far, Input::default(), 0);
    assert_eq!(far.foes[3].hp, 18);
}
#[test]
fn pennon_aim_has_eight_integer_directions_and_can_strafe() {
    for (dx, dy) in [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        let mut d = Lane::new(41);
        let start = d.x;
        step(
            &mut d,
            Input {
                move_x: -1,
                aim_x: dx,
                aim_y: dy,
                fire: true,
                ..Default::default()
            },
            0,
        );
        let shot = d.shots.first().unwrap();
        assert_eq!(
            (shot.vx.signum(), shot.vy.signum()),
            (i32::from(dx), i32::from(dy))
        );
        assert!(d.x < start);
    }
}

#[test]
fn pennon_running_fire_keeps_the_leg_stride() {
    let mut d = Lane::new(41);
    step(
        &mut d,
        Input {
            move_x: 1,
            fire: true,
            ..Default::default()
        },
        0,
    );
    assert!([1, 2].contains(&d.pose));
    step(
        &mut d,
        Input {
            fire: true,
            ..Default::default()
        },
        1,
    );
    assert_eq!(d.pose, 11);
}
