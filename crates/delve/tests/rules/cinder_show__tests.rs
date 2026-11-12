//! Cinder Stack's show: marks written where pots pop, pairs land, slag falls and the kiln cracks, the
//! calls the table hears, the hold a crack asks of the room, and the time to beat a rematch carries.
use super::{BTreeMap, Input, Run};
use crate::rules::cinder::*;
use crate::rules::cinder_show::{Calls, Cells, Cry, Kind, Mark, Show};
use crate::rules::juice::Weight;
use crate::rules::tune::Tune;

fn drop_key() -> Input {
    Input { interact: true, ..Default::default() }
}

/// A board that chains `links` times when its first pair is dropped in the first column.
fn staged(links: u8) -> Board {
    let cols: [&[u8]; 6] = if links >= 3 {
        [&[1, 1], &[2, 3], &[2, 3], &[3], &[2, 1], &[3, 2]]
    } else {
        [&[1, 1], &[2], &[2], &[3], &[2, 1], &[3, 2]]
    };
    let mut b = Board::seeded(5);
    b.cells = [[0; COLS]; ROWS];
    for (x, col) in cols.iter().enumerate() {
        for (i, &c) in col.iter().enumerate() {
            b.cells[ROWS - 1 - i][x] = c;
        }
    }
    b.piece = 0;
    b.x = 0;
    b.y = 1;
    b.turn = 0;
    b.drop_key = false;
    b
}

/// Step a board with nothing pressed until it is still, returning every mark it made, oldest first.
fn settle(b: &mut Board, tune: &Tune, made: &mut Vec<Mark>) {
    for _ in 0..120 {
        b.step(Input::default(), tune);
        for m in &b.show.marks {
            if !made.contains(m) {
                made.push(*m);
            }
        }
        // The cascade ends when the next pair comes.
        if b.flash == 0 && b.piece > 0 {
            break;
        }
    }
}

#[test]
fn a_dropped_pair_and_a_three_link_chain_are_marked_link_by_link() {
    let tune = Tune::default();
    let mut b = staged(3);
    b.step(drop_key(), &tune);
    let mut made: Vec<Mark> = b.show.marks.clone();
    settle(&mut b, &tune, &mut made);
    let kinds: Vec<_> = made.iter().map(|m| m.kind).collect();
    assert!(matches!(kinds[0], Kind::Land { hard: true, a: (0, 7), b: (0, 6), .. }), "the drop is marked first: {:?}", kinds[0]);
    let pops: Vec<(u8, u8)> = kinds
        .iter()
        .filter_map(|k| match k {
            Kind::Pop { chain, gain, .. } => Some((*chain, *gain)),
            _ => None,
        })
        .collect();
    assert_eq!(pops, vec![(1, 3), (2, 6), (3, 9)], "three links, each taking its link's number of cracks");
    assert_eq!(b.cracks, 18);
    // Twelve cracks were crossed on the third link: one stage, once.
    let stages: Vec<u8> = kinds.iter().filter_map(|k| if let Kind::Stage { n } = k { Some(*n) } else { None }).collect();
    assert_eq!(stages, vec![1]);
    // Every pop names exactly the pots that went, by colour.
    let Kind::Pop { by, .. } = kinds.iter().find(|k| matches!(k, Kind::Pop { chain: 1, .. })).copied().unwrap() else { unreachable!() };
    assert_eq!(by[0].count(), 3, "three blue pots");
    assert!(by[0].has(0, 7) && by[0].has(0, 8) && by[0].has(0, 9));
    assert_eq!(by[1].count() + by[2].count() + by[3].count(), 0);
    // The marks are in the board's own ticks, and old ones go.
    assert!(made.iter().all(|m| m.born <= b.clock));
    assert!(b.valid());
    for _ in 0..70 {
        b.step(Input::default(), &tune);
    }
    assert!(b.show.marks.iter().all(|m| m.age(b.clock) < m.kind.life()), "old marks were let go");
}

#[test]
fn weights_grow_with_the_chain_and_the_last_crack_is_the_finale() {
    let by = [Cells::default(); 4];
    let w = |chain, gain| Kind::Pop { by, chain, gain }.weight();
    assert_eq!(w(1, 3), Weight::Graze);
    assert_eq!(w(1, 6), Weight::Solid);
    assert_eq!(w(2, 6), Weight::Solid);
    assert_eq!(w(3, 9), Weight::Crushing);
    assert_eq!(w(2, 14), Weight::Crushing);
    assert_eq!(w(4, 12), Weight::Kill);
    assert_eq!(Kind::Stage { n: 1 }.weight(), Weight::Kill);
    assert_eq!(Kind::Stage { n: 3 }.weight(), Weight::Finale);
    assert_eq!(Kind::Slag { col: 0, row: 0, hit: true, near: true }.weight(), Weight::Crushing);
    assert_eq!(Kind::Slag { col: 0, row: 0, hit: false, near: false }.weight(), Weight::Solid);
    // The last of the kiln: thirty-four and a pop of three splits it.
    let mut b = staged(2);
    b.cracks = 34;
    b.step(drop_key(), &Tune::default());
    let mut made = b.show.marks.clone();
    settle(&mut b, &Tune::default(), &mut made);
    assert_eq!(b.cracks, CRACKS, "capped at the goal");
    assert!(made.iter().any(|m| m.kind == Kind::Stage { n: 3 }), "the finale is marked");
}

#[test]
fn slag_is_marked_where_it_lands_and_into_the_pair_it_is_a_cough() {
    let tune = Tune::default();
    let mut b = Board::seeded(5);
    b.column = 2;
    b.tell = 1;
    b.windup = 27;
    b.x = 2;
    b.step(Input::default(), &tune);
    let slag = b.show.marks.iter().find_map(|m| if let Kind::Slag { col, row, hit, near } = m.kind { Some((col, row, hit, near)) } else { None });
    assert_eq!(slag, Some((2, 7, false, true)), "slag settles on the stack, in the pair's own column");
    // Slag dropped into a pair that cannot move is a cough.
    let mut b = Board::seeded(5);
    for y in 2..ROWS {
        b.cells[y][3] = 1 + (y % 2) as u8;
    }
    b.cells[1][3] = 0;
    b.cells[0][3] = 0;
    b.x = 3;
    b.y = 1;
    b.column = 3;
    b.tell = 1;
    let before = b.misses;
    b.step(Input::default(), &tune);
    assert!(b.show.marks.iter().any(|m| matches!(m.kind, Kind::Slag { hit: true, .. })));
    assert!(b.show.marks.iter().any(|m| m.kind == Kind::Cough));
    assert_eq!(b.misses, before + 1);
}

#[test]
fn the_table_hears_a_lead_a_big_chain_a_stage_and_match_point_once_each() {
    let mut c = Calls::default();
    let by = [Cells::default(); 4];
    c.standings(10, &[(1, 3), (2, 2)]);
    assert!(c.list.is_empty(), "three cracks is not yet a lead");
    c.standings(20, &[(1, 7), (2, 2)]);
    assert_eq!(c.list.len(), 1);
    assert_eq!((c.list[0].cry, c.list[0].who), (Cry::Lead, 1));
    c.standings(21, &[(1, 8), (2, 2)]);
    assert_eq!(c.list.len(), 1, "the lead is called once");
    c.standings(30, &[(1, 8), (2, 9)]);
    assert_eq!((c.list[1].cry, c.list[1].who), (Cry::Lead, 2), "and again when it changes hands");
    c.standings(40, &[(1, 30), (2, 31)]);
    c.standings(41, &[(1, 30), (2, 31)]);
    assert_eq!(c.list.iter().filter(|c| c.cry == Cry::Point).count(), 2, "match point once a knight");
    let two = Mark { kind: Kind::Pop { by, chain: 2, gain: 6 }, born: 50 };
    let five = Mark { kind: Kind::Pop { by, chain: 5, gain: 15 }, born: 50 };
    let stage = Mark { kind: Kind::Stage { n: 2 }, born: 50 };
    let last = Mark { kind: Kind::Stage { n: 3 }, born: 50 };
    c.report(3, [two, five, stage, last].iter(), 50);
    let heard: Vec<Cry> = c.list.iter().filter(|c| c.who == 3).map(|c| c.cry).collect();
    assert_eq!(heard, vec![Cry::Chain(5), Cry::Stage(2)], "a chain of two stays on its own board; the last crack is the session's to call");
    assert!(c.valid());
    for call in &c.list {
        assert!(call.cry.rank() <= 9);
    }
    // They are let go.
    c.tick(50 + 200);
    assert!(c.list.is_empty());
    // Two knights close to the end and three cracks apart: the race is called once.
    let mut race = Calls::default();
    race.standings(5, &[(1, 26), (2, 20)]);
    assert!(!race.list.iter().any(|c| c.cry == Cry::Close), "not yet near the end");
    race.standings(6, &[(1, 28), (2, 24)]);
    assert!(!race.list.iter().any(|c| c.cry == Cry::Close), "four apart is not close");
    race.standings(7, &[(1, 28), (2, 25)]);
    race.standings(8, &[(1, 29), (2, 28)]);
    assert_eq!(race.list.iter().filter(|c| c.cry == Cry::Close).count(), 1);
    assert_eq!(race.list.iter().find(|c| c.cry == Cry::Close).map(|c| c.who), Some(1));
    // A lone knight has no lead to take.
    let mut solo = Calls::default();
    solo.standings(5, &[(1, 20)]);
    assert!(solo.list.is_empty());
}

#[test]
fn a_whole_round_keeps_its_show_valid_and_exact_on_the_wire_and_a_crack_holds_the_room() {
    let mut run = Run::cinder_for_test();
    for id in 2..=4 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.step(&BTreeMap::from([(1, drop_key())]));
    let mut held = 0;
    let mut stage_seen = false;
    for t in 0..LENGTH {
        let Some(s) = run.cinder.as_ref() else { break };
        if s.left == 0 {
            break;
        }
        let keys = s.boards.iter().filter(|(_, b)| b.clock % 3 == 0).map(|(&id, b)| (id, b.bot_input())).collect();
        let before = run.cinder.as_ref().unwrap().left;
        run.step(&keys);
        let s = run.cinder.as_ref().unwrap();
        if s.left == before && s.left > 0 {
            held += 1;
        }
        stage_seen |= s.boards.values().any(|b| b.show.marks.iter().any(|m| matches!(m.kind, Kind::Stage { .. })));
        assert!(s.valid(), "tick {t}");
        if t % 17 == 0 {
            assert_eq!(run.wire_copy().cinder, run.cinder, "the show rides the wire at tick {t}");
        }
    }
    assert!(stage_seen, "a bot cracked the kiln a stage");
    assert!(held >= 3, "the room held still for a crack: {held} held ticks");
    let s = run.cinder.as_ref().unwrap();
    assert!(s.won && s.left == 0);
    assert!(s.done > 0, "the round noted when it ended");
    assert!(s.calls.list.iter().any(|c| c.cry == Cry::Cracked && c.who == s.order[0]));
}

#[test]
fn the_quickest_crack_of_a_sitting_is_carried_into_the_rematch() {
    let mut run = Run::cinder_for_test();
    run.join_seat(2, "Seat 2");
    run.step(&BTreeMap::from([(1, drop_key())]));
    assert_eq!(run.cinder.as_ref().unwrap().to_beat, 0, "a first round has nothing to beat");
    for _ in 0..LENGTH {
        let s = run.cinder.as_ref().unwrap();
        if s.left == 0 {
            break;
        }
        let keys = s.boards.iter().map(|(&id, b)| (id, b.bot_input())).collect();
        run.step(&keys);
    }
    let s = run.cinder.as_ref().unwrap();
    assert!(s.won);
    let time = s.boards[&s.order[0]].crack_tick;
    assert!(time > 0 && s.quickest() == time);
    for _ in 0..60 {
        run.step(&BTreeMap::new());
    }
    run.step(&BTreeMap::from([(1, drop_key())]));
    let again = run.cinder.as_ref().unwrap();
    assert_eq!(again.left, LENGTH, "a new round");
    assert_eq!(again.to_beat, time, "and the time to beat");
    assert!(again.calls.quiet() && again.boards.values().all(|b| b.show.quiet()), "with a fresh show");
}

#[test]
fn a_broken_show_is_repaired_and_marks_are_capped() {
    let mut s = Session::new(1..=2, 9);
    let b = s.boards.get_mut(&1).unwrap();
    for k in 0..40 {
        b.show.mark(Kind::Cough, k);
    }
    assert!(b.show.marks.len() <= 12, "the board keeps a dozen marks at most");
    assert!(b.valid());
    b.show = Show { marks: vec![Mark { kind: Kind::Stage { n: 9 }, born: 0 }] };
    assert!(!s.valid());
    s.repair();
    assert!(s.valid(), "repair put the board back");
}
