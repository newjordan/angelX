use super::{BTreeMap, HZ, Input, Run, Spoil};
use crate::rules::{cinder::*, tune::Tune};

fn game(n: u32) -> Run {
    let mut run = Run::cinder_for_test();
    if n == 1 {
        run.leave(2);
    }
    for id in 2..=n {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(run.cinder.is_some());
    run
}
#[test]
fn cinder_every_seat_moves_its_own_pair_and_wire_preserves_it() {
    let mut run = game(4);
    let start = run.cinder.as_ref().unwrap();
    assert!(start.boards.values().all(|b| b.seed == start.seed));
    run.step(&BTreeMap::from([
        (
            2,
            Input {
                move_x: -1,
                ..Default::default()
            },
        ),
        (
            3,
            Input {
                fire: true,
                ..Default::default()
            },
        ),
        (
            4,
            Input {
                interact: true,
                ..Default::default()
            },
        ),
    ]));
    let s = run.cinder.as_ref().unwrap();
    assert_eq!(s.boards.len(), 4);
    assert_eq!(s.boards[&1].x, 2);
    assert_eq!(s.boards[&2].x, 1);
    assert_eq!(s.boards[&3].turn, 1);
    // Starting E is latched, so release it before the next drop.
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(
        4,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert_ne!(
        run.cinder.as_ref().unwrap().boards[&4].cells,
        run.cinder.as_ref().unwrap().boards[&1].cells
    );
    assert_eq!(run.wire_copy().cinder, run.cinder);
    assert!(run.valid_snapshot());
    assert_ne!(
        crate::render::overworld::arena::frame_for(&run, 384, 224, Some(1)),
        crate::render::overworld::arena::frame_for(&run, 384, 224, Some(2))
    );
}
#[test]
fn cinder_scripted_match_wins_each_round_pays_once_and_crown_carries_down() {
    for seats in [1, 2, 4, 2, 4] {
        let mut attempt = game(seats);
        for _ in 0..LENGTH {
            let s = attempt.cinder.as_ref().unwrap();
            if s.left == 0 {
                break;
            }
            let input = s.boards[&seats].bot_input();
            attempt.step(&BTreeMap::from([(seats, input)]));
        }
        assert!(
            attempt.cinder.as_ref().unwrap().won,
            "{seats} seats can complete the loop"
        );
    }
    let mut run = game(2);
    for _ in 0..LENGTH {
        let s = run.cinder.as_ref().unwrap();
        if s.left == 0 {
            break;
        }
        let input = s.boards[&2].bot_input();
        run.step(&BTreeMap::from([(2, input)]));
        assert!(run.cinder.as_ref().unwrap().valid());
    }
    let s = run.cinder.as_ref().unwrap();
    assert!(s.won, "scripted solver can crack the spirit");
    assert_eq!(s.order[0], 2);
    assert!(run.players[&2].laurel);
    assert!(!run.players[&1].laurel);
    let gold = run.players[&2].carried.get(Spoil::Gold);
    assert_eq!(gold, GOLD_PER_SEAT * 2);
    assert_eq!(run.home.countryside.games.cinder.wins, 1);
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&2].carried.get(Spoil::Gold), gold);
    run.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert_eq!(run.cinder.as_ref().unwrap().left, LENGTH, "one-tick replay");
    run.cinder
        .as_mut()
        .unwrap()
        .boards
        .get_mut(&2)
        .unwrap()
        .cracks = CRACKS;
    run.step(&BTreeMap::new());
    assert_eq!(
        run.players[&2].carried.get(Spoil::Gold),
        gold + GOLD_PER_SEAT * 2,
        "a new clear pays a new purse"
    );
    let bombs = run.players[&2].bombs;
    run.kit_out_for_test();
    assert_eq!(
        run.players[&2].bombs,
        (bombs + 1).min(crate::rules::MAX_BOMBS)
    );
    let mut records = Default::default();
    assert!(crate::rules::pastimes::Records::merge(
        &mut records,
        &run.home.countryside.games
    ));
    assert_eq!(records.cinder.best, run.home.countryside.games.cinder.best);
}
#[test]
fn cinder_shared_round_seed_time_tiebreak_and_departures_are_safe() {
    let mut run = game(4);
    let s = run.cinder.as_ref().unwrap();
    let seed = s.seed;
    let cells = s.boards[&1].cells;
    assert!(
        s.boards
            .values()
            .all(|b| b.seed == seed && b.cells == cells)
    );
    assert!((1..16).any(|n| Board::seeded(n).cells != Board::seeded(n + 1).cells));
    for (&id, b) in &mut run.cinder.as_mut().unwrap().boards {
        b.cracks = CRACKS;
        b.crack_tick = if id == 1 { 90 } else { 60 };
    }
    run.step(&BTreeMap::new());
    let s = run.cinder.as_ref().unwrap();
    assert_eq!(s.order[0], 2, "earlier equal score wins the tiebreak");
    assert_eq!(s.place(1), 4);
    assert_eq!(s.place(2), 1);
    assert_eq!(s.place(3), 1);
    assert!(!run.players[&1].laurel);
    for id in 2..=4 {
        assert!(
            run.players[&id].laurel,
            "exact ties visibly share the crown"
        );
        assert_eq!(
            run.players[&id].carried.get(Spoil::Gold),
            GOLD_PER_SEAT * 4 / 3 + u32::from(id == 2)
        );
    }
    assert_eq!(
        (1..=4)
            .map(|id| run.players[&id].carried.get(Spoil::Gold))
            .sum::<u32>(),
        GOLD_PER_SEAT * 4
    );
    assert_eq!(s.payout(1), 0);
    assert_eq!(s.payout(2), 14);
    assert_eq!(s.payout(3), 13);
    assert_eq!(s.payout(4), 13);
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    let s = run.cinder.as_ref().unwrap();
    assert_ne!(s.seed, seed);
    assert!(s.boards.values().all(|b| b.cells == s.boards[&1].cells));
    let mut departed = game(2);
    departed
        .cinder
        .as_mut()
        .unwrap()
        .boards
        .get_mut(&2)
        .unwrap()
        .cracks = CRACKS;
    departed.leave(2);
    departed.step(&BTreeMap::new());
    assert_eq!(departed.cinder.as_ref().unwrap().boards.len(), 1);
    assert!(departed.cinder.as_ref().unwrap().left > 0);
    let mut broken = Board::default();
    broken.direction = i8::MIN;
    assert!(!broken.valid());
}

#[test]
fn cinder_last_clear_clamps_at_goal_and_only_finishers_have_a_time() {
    let mut run = game(1);
    let gold = run.players[&1].carried.get(Spoil::Gold);
    let b = run.cinder.as_mut().unwrap().boards.get_mut(&1).unwrap();
    b.cracks = CRACKS - 1;
    b.crack_tick = 9;
    assert_eq!(b.finish_tick(), None);
    b.cells = [[0; COLS]; ROWS];
    b.cells[9] = [1, 1, 1, 0, 0, 0];
    b.x = 5;
    b.drop_key = false;
    run.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    let s = run.cinder.as_ref().unwrap();
    let b = &s.boards[&1];
    assert_eq!(b.cracks, CRACKS);
    assert_eq!(b.finish_tick(), Some(1));
    assert!(s.won);
    assert!(s.valid());
    assert_eq!(run.home.countryside.games.cinder.best, CRACKS);
    assert_eq!(
        run.players[&1].carried.get(Spoil::Gold),
        gold + GOLD_PER_SEAT
    );
    let mut invalid = b.clone();
    invalid.cracks = CRACKS + 1;
    assert!(!invalid.valid());
    let mut unfinished = Board::default();
    unfinished.crack_tick = 120;
    assert_eq!(unfinished.finish_tick(), None);
    unfinished.cracks = 24;
    assert_eq!(unfinished.finish_tick(), None);
}

#[test]
fn cinder_idle_loses_and_broken_save_repairs() {
    let mut run = game(1);
    for _ in 0..LENGTH {
        run.step(&BTreeMap::new());
    }
    let s = run.cinder.as_ref().unwrap();
    assert!(!s.won);
    assert_eq!(s.left, 0);
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), 0);
    let mut bad = s.clone();
    let seed = bad.seed;
    bad.boards.get_mut(&1).unwrap().crack_tick = LENGTH + 1;
    assert!(!bad.valid());
    bad.boards.get_mut(&1).unwrap().x = 99;
    assert!(!bad.valid());
    bad.repair();
    assert!(bad.valid());
    assert_eq!(bad.boards[&1].seed, seed);
    bad.boards.clear();
    bad.repair();
    assert!(bad.valid());
}
#[test]
fn cinder_overflow_restarts_within_half_a_second() {
    let mut b = Board::default();
    b.seed = 73;
    let starting = Board::seeded(73).cells;
    b.cells = [[4; COLS]; ROWS];
    // An attack into a completely full column triggers a cough, not an index error.
    b.tell = 1;
    b.step(Input::default(), &Tune::default());
    assert_eq!(b.retry, RETRY);
    for _ in 0..RETRY {
        b.step(Input::default(), &Tune::default());
    }
    assert_eq!(b.retry, 0);
    assert_eq!(
        b.cells, starting,
        "retry retains this round’s shared layout"
    );
    assert!(RETRY < HZ);
    assert!(b.valid());
}
#[test]
fn cinder_slag_telegraph_is_exact_and_never_shorter_than_three_quarters() {
    let tune = Tune::default();
    let mut b = Board::default();
    b.attack = tune.cinder_attack - 1;
    b.step(Input::default(), &tune);
    assert_eq!(b.tell, tune.cinder_tell);
    let before = b.cells;
    assert_eq!(b.crack_stage(), 0);
    for (cracks, stage) in [(12, 1), (24, 2), (36, 3)] {
        let mut v = b.clone();
        v.cracks = cracks;
        assert_eq!(v.crack_stage(), stage);
    }
    let col = b.column as usize;
    for _ in 0..tune.cinder_tell - 1 {
        b.step(Input::default(), &tune);
        assert_eq!(b.cells, before);
    }
    b.step(Input::default(), &tune);
    assert_eq!(b.cells[ROWS - 3][col], 4);
    assert_eq!(b.impact, 12);
    for _ in 0..12 {
        b.step(Input::default(), &tune);
    }
    assert_eq!(b.impact, 0);
    assert_eq!(b.impact_row as usize, ROWS - 3);
    assert_eq!(b.cells.iter().flatten().filter(|&&c| c == 4).count(), 1);
    assert!(tune.cinder_tell >= 23);
    let mut tuned = tune;
    tuned.set("cinder_tell", 1.0);
    assert_eq!(tuned.cinder_tell, 23);
}
#[test]
fn cinder_all_five_knobs_change_their_part_of_play() {
    let original = Tune::default();
    assert_eq!(
        (
            original.cinder_fall,
            original.cinder_slide,
            original.cinder_lock,
            original.cinder_tell,
            original.cinder_attack
        ),
        (20, 5, 10, 27, 165)
    );
    // Each knob exercises the verb or rival, with deterministic input.
    for name in [
        "cinder_fall",
        "cinder_slide",
        "cinder_lock",
        "cinder_tell",
        "cinder_attack",
    ] {
        let base = Tune::default();
        let mut moved = base;
        let k = base.knobs().into_iter().find(|k| k.name == name).unwrap();
        moved.set(name, k.max);
        let play = |tune: &Tune| {
            let mut b = Board::default();
            for tick in 0..340 {
                let i = Input {
                    move_x: if tick < 80 { -1 } else { 1 },
                    ..Default::default()
                };
                b.step(i, tune);
            }
            b
        };
        assert_ne!(play(&base), play(&moved), "{name}");
    }
}
#[test]
fn cinder_matches_break_adjacent_slag_and_form_cascades() {
    let mut b = Board::default();
    b.cells = [[0; COLS]; ROWS];
    b.cells[9] = [1, 1, 1, 4, 0, 0];
    b.cells[8] = [2, 1, 1, 0, 0, 0];
    b.cells[7] = [0, 1, 1, 0, 0, 0];
    b.cells[6] = [0, 2, 1, 0, 0, 0];
    b.cells[5][2] = 2;
    b.x = 5;
    b.y = 1;
    b.turn = 0;
    b.piece = 0;
    b.drop_key = false;
    b.step(
        Input {
            interact: true,
            ..Default::default()
        },
        &Tune::default(),
    );
    assert_eq!(
        b.flash, 8,
        "clear drives both flinch frames and the pot burst"
    );
    assert!(b.pop[9][3]);
    assert!(b.cracks >= 3);
    for _ in 0..8 {
        b.step(Input::default(), &Tune::default());
    }
    assert_eq!(b.cells[9][3], 0, "matched neighbours break slag");
    assert_eq!(b.chain, 2, "staggered blue pots fall into the second clear");
    assert_eq!(b.flash, 8);
    assert_eq!(b.cracks, 14, "eight reds plus three blues at chain two");
}
#[test]
#[ignore]
fn write_cinder_review() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let entrance =
        crate::render::overworld::arena::frame_for(&Run::cinder_for_test(), 768, 448, Some(1));
    image::save_buffer(
        dir.join("entrance.png"),
        &entrance.rgb_bytes(),
        768,
        448,
        image::ColorType::Rgb8,
    )
    .unwrap();
    let mut run = game(4);
    for tick in 0..LENGTH {
        let s = run.cinder.as_ref().unwrap();
        let inputs = s
            .boards
            .iter()
            .map(|(&id, b)| {
                let mut i = if id == 1 {
                    Input::default()
                } else {
                    b.bot_input()
                };
                if tick % 18 > 1 {
                    i.interact = false;
                }
                (id, i)
            })
            .collect();
        run.step(&inputs);
        if [1, 190, 195, 260].contains(&tick) || run.cinder.as_ref().unwrap().left == 0 {
            let name = if run.cinder.as_ref().unwrap().left == 0 {
                "podium".to_owned()
            } else {
                format!("tick-{tick:04}")
            };
            for id in [1, 2] {
                let frame = crate::render::overworld::arena::frame_for(&run, 768, 448, Some(id));
                image::save_buffer(
                    dir.join(format!("{name}-seat-{id}.png")),
                    &frame.rgb_bytes(),
                    768,
                    448,
                    image::ColorType::Rgb8,
                )
                .unwrap();
            }
            if run.cinder.as_ref().unwrap().left == 0 {
                break;
            }
        }
    }
    let dir = dir.join("moments");
    std::fs::create_dir_all(&dir).unwrap();
    let mut staged = game(4);
    let scenes = [
        ("cracks-00", 0, 0, 0, 0, 0),
        ("cracks-12", 12, 0, 0, 0, 0),
        ("cracks-24", 24, 0, 0, 0, 0),
        ("cracks-36", 36, 0, 0, 0, 0),
        ("flinch-1", 0, 8, 0, 0, 0),
        ("flinch-2", 0, 4, 0, 0, 0),
        ("hit-chain-2", 24, 8, 0, 0, 2),
        ("hit-chain-3", 24, 4, 0, 0, 3),
        ("windup-1", 24, 0, 27, 0, 0),
        ("windup-2", 24, 0, 13, 0, 0),
        ("impact-1", 24, 0, 0, 12, 0),
        ("impact-2", 24, 0, 0, 9, 0),
        ("impact-3", 24, 0, 0, 6, 0),
        ("impact-4", 24, 0, 0, 3, 0),
        ("flight-1", 24, 8, 0, 0, 0),
        ("flight-2", 24, 7, 0, 0, 0),
        ("flight-3", 24, 6, 0, 0, 0),
        ("flight-4", 24, 5, 0, 0, 0),
        ("flight-5", 24, 4, 0, 0, 0),
        ("flight-6", 24, 3, 0, 0, 0),
        ("flight-7", 24, 2, 0, 0, 0),
        ("flight-8", 24, 1, 0, 0, 0),
        ("rival-stacks", 12, 0, 0, 0, 0),
    ];
    for (name, cracks, flash, tell, impact, chain) in scenes {
        let b = staged.cinder.as_mut().unwrap().boards.get_mut(&1).unwrap();
        *b = Board::seeded(41);
        b.cracks = cracks;
        b.flash = flash;
        b.tell = tell;
        b.windup = 27;
        b.impact = impact;
        b.impact_row = 7;
        b.column = 3;
        b.chain = chain;
        b.pop = [[false; COLS]; ROWS];
        if flash > 0 {
            for x in 0..3 {
                b.cells[9][x] = 1;
                b.pop[9][x] = true;
            }
        }
        if name == "rival-stacks" {
            for (id, heights) in [
                (2, [2, 6, 3, 8, 5, 10]),
                (3, [8, 2, 5, 3, 7, 4]),
                (4, [1, 5, 9, 4, 6, 2]),
            ] {
                let b = staged.cinder.as_mut().unwrap().boards.get_mut(&id).unwrap();
                b.cells = [[0; COLS]; ROWS];
                for (x, height) in heights.into_iter().enumerate() {
                    for y in ROWS - height..ROWS {
                        b.cells[y][x] = 1;
                    }
                }
            }
        }
        let frame = crate::render::overworld::arena::frame_for(&staged, 384, 224, Some(1));
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &frame.rgb_bytes(),
            384,
            224,
            image::ColorType::Rgb8,
        )
        .unwrap();
    }
}
