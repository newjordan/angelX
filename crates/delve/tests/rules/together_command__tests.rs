use super::*;
use crate::rules::votes::{Action, Command as Vote};
use crate::rules::{HZ, Run};
use angel_net::cmd::{Command, SeatInputs};
use std::collections::BTreeMap;

fn bytes(cmd: &DelveCmd) -> Vec<u8> {
    let mut out = Vec::new();
    cmd.encode(&mut out);
    out
}

/// Every field of an input set to something, for the round trips.
fn busy() -> Input {
    Input {
        leave_activity: true,
        activity_menu: true,
        vote: Some(Vote { serial: 1_760_000_000_123, action: Action::Rate(4) }),
        move_x: -1,
        move_y: 1,
        aim_x: 1,
        aim_y: -1,
        fire: true,
        dash: true,
        bomb: true,
        swing: true,
        play: 4,
        cast: 3,
        vigil: true,
        ult: true,
        take: true,
        interact: true,
        skill: 5,
        skip_plate: true,
        again: true,
    }
}

#[test]
fn a_command_reads_back_as_it_was_written() {
    let walking = DelveCmd {
        held: Input { move_x: 1, move_y: -1, fire: true, ..Input::default() },
        pressed: Input { fire: true, ..Input::default() },
    };
    let cases = [
        DelveCmd::default(),
        walking,
        DelveCmd { held: busy(), pressed: busy() },
        DelveCmd { held: Input::default(), pressed: Input { cast: 2, ..Input::default() } },
    ];
    for cmd in cases {
        let b = bytes(&cmd);
        assert!(b.len() <= MAX_BYTES, "{} bytes", b.len());
        assert_eq!(DelveCmd::decode(&b), Some(cmd));
    }
    // Walking and firing is four bytes; standing still is too.
    assert_eq!(bytes(&walking).len(), 4);
    assert_eq!(bytes(&DelveCmd::default()).len(), 4);
    for action in [
        Action::Toggle,
        Action::Close,
        Action::Up,
        Action::Down,
        Action::Left,
        Action::Right,
        Action::Clear,
        Action::Untried,
        Action::Rate(1),
        Action::Rate(5),
    ] {
        let cmd = DelveCmd {
            held: Input::default(),
            pressed: Input { vote: Some(Vote { serial: 7, action }), ..Input::default() },
        };
        assert_eq!(DelveCmd::decode(&bytes(&cmd)), Some(cmd), "{action:?}");
    }
}

#[test]
fn malformed_bytes_are_refused() {
    let good = bytes(&DelveCmd { held: busy(), pressed: busy() });
    // Cut short anywhere, or with a byte left over.
    for n in 0..good.len() {
        assert_eq!(DelveCmd::decode(&good[..n]), None, "cut at {n}");
    }
    let mut long = good.clone();
    long.push(0);
    assert_eq!(DelveCmd::decode(&long), None);
    // An axis code 3 is no axis.
    assert_eq!(DelveCmd::decode(&[0b0000_0011, 0, 0x55, 0]), None);
    assert_eq!(DelveCmd::decode(&[0x55, 0, 0b1100_0000, 0]), None);
    // A flag past the vote's.
    assert_eq!(DelveCmd::decode(&[0x55, 0x80, 0x80, 0x01, 0x55, 0]), None);
    // A slots varint claimed and zero, or too wide.
    assert_eq!(DelveCmd::decode(&[0x55, 0x80, 0x20, 0, 0x55, 0]), None);
    assert_eq!(DelveCmd::decode(&[0x55, 0x80, 0x20, 0x80, 0x04, 0x55, 0]), None);
    // A card slot past the hand (5), a spell past the slots (5), a skill
    // past the skills (6): read, then refused by `Input::valid`.
    for slots in [5u64, 5 << 3, 6 << 6] {
        let mut b = vec![0x55, 0x80, 0x20];
        put_varint(&mut b, slots);
        b.extend_from_slice(&[0x55, 0]);
        assert_eq!(DelveCmd::decode(&b), None, "slots {slots:#x}");
    }
    // A vote with no action, an unknown action, a rating of 0 or 6.
    for action in [None, Some(8u8), Some(14), Some(255)] {
        let mut b = vec![0x55, 0x80, 0x40, 9];
        b.extend(action);
        b.extend_from_slice(&[0x55, 0]);
        assert_eq!(DelveCmd::decode(&b), None, "action {action:?}");
    }
    // Too long to be a command at all.
    assert_eq!(DelveCmd::decode(&[0x55; MAX_BYTES + 1]), None);
    // Garbage never panics.
    let mut seed = 0x9e37_79b9u32;
    for _ in 0..20_000 {
        let n = (seed % 24) as usize;
        let body: Vec<u8> = (0..n)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed as u8
            })
            .collect();
        if let Some(cmd) = DelveCmd::decode(&body) {
            assert!(cmd.held.valid() && cmd.pressed.valid());
            assert_eq!(DelveCmd::decode(&bytes(&cmd)), Some(cmd));
        }
    }
}

#[test]
fn the_step_sees_held_with_pressed_laid_over_it() {
    let cmd = DelveCmd {
        held: Input { move_x: 1, aim_y: -1, cast: 1, ..Input::default() },
        pressed: Input { fire: true, aim_y: 1, cast: 3, skill: 2, ..Input::default() },
    };
    let input = cmd.input();
    assert_eq!((input.move_x, input.aim_y, input.fire), (1, 1, true));
    assert_eq!((input.cast, input.skill), (3, 2), "a pressed slot wins over a held one");
}

#[test]
fn a_repeated_tick_keeps_what_is_held_and_drops_presses_and_one_shots() {
    let cmd = DelveCmd { held: busy(), pressed: busy() };
    let repeat = cmd.held_only();
    assert_eq!(repeat.pressed, Input::default());
    let held = repeat.input();
    assert!(held.vote.is_none() && !held.leave_activity && !held.activity_menu && !held.skip_plate);
    assert!(held.fire && held.dash && held.ult && held.interact);
    assert_eq!((held.move_x, held.aim_x, held.play, held.cast, held.skill), (-1, 1, 4, 3, 5));
    assert_eq!(DelveCmd::neutral().input(), Input::default());
}

#[test]
fn a_pile_up_folds_into_the_next_command_keeping_its_presses() {
    let vote = Some(Vote { serial: 3, action: Action::Toggle });
    let older = DelveCmd {
        held: Input { move_x: -1, ..Input::default() },
        pressed: Input { bomb: true, cast: 2, vote, ..Input::default() },
    };
    let mut newer = DelveCmd {
        held: Input { move_x: 1, ..Input::default() },
        pressed: Input { fire: true, ..Input::default() },
    };
    newer.absorb(&older);
    let input = newer.input();
    assert_eq!(input.move_x, 1, "the newer command's held keys");
    assert!(input.bomb && input.fire);
    assert_eq!((input.cast, input.vote), (2, vote));
    // The seat's queue folds the same way: the tap is not lost.
    let mut seat = SeatInputs::default();
    seat.push(1, older);
    seat.push(2, DelveCmd::neutral());
    seat.push(3, DelveCmd::neutral());
    seat.push(4, DelveCmd::neutral());
    let (taken, ack) = seat.take();
    assert_eq!(ack, 2);
    assert!(taken.input().bomb);
}

/// A tap of F shorter than a tick (down and up between two samples) is in
/// that tick's `pressed` alone, and the knight fires on that tick.
#[test]
fn a_tap_shorter_than_a_tick_fires() {
    let mut run = Run::new(7, 1, None);
    run.enter_for_test(0);
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    let shots = |run: &Run| run.projectiles.iter().filter(|p| !p.hostile).count();
    let before = shots(&run);
    let tap = DelveCmd {
        held: Input::default(),
        pressed: Input { fire: true, ..Input::default() },
    };
    let decoded = DelveCmd::decode(&bytes(&tap)).expect("reads");
    run.step(&BTreeMap::from([(1, decoded.input())]));
    assert!(shots(&run) > before, "the tap fired");
    // The repeat of that tick, had the next command been late, does not.
    let mut quiet = run.clone();
    let before = shots(&quiet);
    for _ in 0..HZ {
        quiet.step(&BTreeMap::from([(1, decoded.held_only().input())]));
    }
    assert!(shots(&quiet) <= before, "a starved tick repeats no press");
}
