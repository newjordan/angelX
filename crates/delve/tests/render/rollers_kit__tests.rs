//! The rollerway's flat art: every piece drawn every way, and what moves, moves.
use super::*;

fn lit(im: &Img) -> usize {
    (0..im.h).flat_map(|y| (0..im.w).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y).is_some()).count()
}

#[test]
fn every_piece_draws_every_way_at_the_size_of_a_tile() {
    for piece in Piece::ALL {
        for dir in 0..4 {
            for (moving, crewed) in [(false, false), (true, true)] {
                let im = piece_flat(piece, dir, moving, crewed, 3);
                assert_eq!(im.w, TILE, "{piece:?} {dir}");
                assert!(im.h >= TILE && im.h <= TILE + RISE + 4, "{piece:?} {dir}: {}", im.h);
                assert!(lit(&im) > 40, "{piece:?} {dir} is nearly empty: {}", lit(&im));
            }
        }
    }
}

#[test]
fn the_ways_read_differently_and_the_pieces_read_differently() {
    for piece in Piece::ALL {
        let ways: Vec<_> = (0..4).map(|d| piece_flat(piece, d, false, true, 0)).collect();
        for a in 0..4 {
            for b in a + 1..4 {
                assert!(*ways[a] != *ways[b], "{piece:?} faces {a} and {b} alike");
            }
        }
    }
    let kinds: Vec<_> = Piece::ALL.iter().map(|p| piece_flat(*p, 1, false, true, 0)).collect();
    for a in 0..kinds.len() {
        for b in a + 1..kinds.len() {
            assert!(*kinds[a] != *kinds[b], "{:?} and {:?} are drawn alike", Piece::ALL[a], Piece::ALL[b]);
        }
    }
}

#[test]
fn the_rollers_turn_and_the_swape_swings_and_a_still_one_does_not() {
    for piece in [Piece::Roller, Piece::Trestle, Piece::Chain] {
        let beats: Vec<_> = (0..FRAMES).map(|f| piece_flat(piece, 1, true, true, f)).collect();
        assert!(beats.iter().any(|b| **b != *beats[0]), "{piece:?} does not move when it works");
        assert!(*piece_flat(piece, 1, false, true, 0) != *beats[1], "{piece:?} looks the same moving and still");
    }
    let swing: Vec<_> = (0..FRAMES).map(|f| swape_flat(1, State::Working, f)).collect();
    assert!(swing.iter().any(|s| **s != *swing[0]), "a working swape does not swing");
    assert_eq!(*swape_flat(1, State::Working, FRAMES), *swing[0], "and its round loops");
    let idle = swape_flat(1, State::Idle, 0);
    assert!(*idle != *swape_flat(1, State::Starved(Load::Planks), 0), "starved reads differently from idle");
    assert!(*idle != *swape_flat(1, State::BackedUp(Load::Planks), 0), "backed up reads differently from idle");
    for dir in 1..4 {
        assert!(*swape_flat(0, State::Idle, 0) != *swape_flat(dir, State::Idle, 0), "a swape faces its way");
    }
    // Raised in steps.
    let steps: Vec<usize> = (0..4).map(|s| lit(&swape_flat(1, State::Building(s), 0))).collect();
    assert!(steps.windows(2).all(|w| w[0] != w[1]), "{steps:?}");
}

#[test]
fn a_good_on_the_line_is_in_its_own_colours_and_a_jam_blinks() {
    let looks: Vec<_> = [Load::Stone, Load::Planks, Load::Lime, Load::Bread, Load::Iron].iter().map(|l| good_flat(*l)).collect();
    for a in 0..looks.len() {
        for b in a + 1..looks.len() {
            assert!(*looks[a] != *looks[b]);
        }
        assert_eq!((looks[a].w, looks[a].h), (6, 4));
    }
    assert!(*jam_flat(0) != *jam_flat(4), "the mark blinks");
    assert_eq!(*jam_flat(0), *jam_flat(8), "and loops");
}

#[test]
fn a_slot_runs_with_the_way_the_tile_faces() {
    // East: from the west edge to the east, mid-height.
    let xs: Vec<f32> = (0..SLOTS).map(|i| slot_at(1, i, 0.0).0).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]) && xs[0] > 0.0 && xs[SLOTS - 1] < TILE as f32);
    assert!((slot_at(1, 0, 0.0).1 - 8.0).abs() < 0.01);
    // West runs the other way, south down, north up.
    assert!(slot_at(3, 0, 0.0).0 > slot_at(3, SLOTS - 1, 0.0).0);
    assert!(slot_at(2, 0, 0.0).1 < slot_at(2, SLOTS - 1, 0.0).1);
    assert!(slot_at(0, 0, 0.0).1 > slot_at(0, SLOTS - 1, 0.0).1);
    // The drawing lags the sim: with a slot taken off it is that much further back.
    assert!(slot_at(1, 1, 1.0).0 < slot_at(1, 1, 0.0).0);
}
