use super::*;
use crate::rules::claim_bosses::{self, Guardian};
use crate::rules::stockyard::{BLOCK_PACE, Stockyard};

/// A closed room on the Mines' first floor with a seam the party has won.
fn won() -> (Run, industry::Deposit) {
    let (mut run, d) = claim_bosses::tests::fixture(Guardian::Ironroot);
    run.home.industry.secure(&d.id);
    assert!(run.home.industry.claims[&d.id].secured);
    run.kingdom.ore_sentries.remove(&d.id);
    (run, d)
}
fn swinging() -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { swing: true, ..Default::default() })])
}
/// Hold the sword down until `n` swings have begun.
fn swing(run: &mut Run, n: u32) {
    let mut seen = 0;
    for _ in 0..n * 40 {
        run.step(&swinging());
        let h = &run.players[&1];
        if h.sword && h.swing == 8 {
            seen += 1;
            if seen == n {
                return;
            }
        }
    }
}
/// Every seat's keys up, so the next press is a fresh one.
fn release(run: &mut Run) {
    let up: BTreeMap<u32, Input> = run.players.keys().map(|&id| (id, Input::default())).collect();
    run.step(&up);
}
fn at(run: &mut Run, id: u32, x: f32, y: f32) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x, y);
}

#[test]
fn three_swings_at_a_won_face_part_a_block_onto_the_knight() {
    let (mut run, d) = won();
    at(&mut run, 1, d.x, d.y);
    swing(&mut run, 2);
    assert!(run.players[&1].block.is_none(), "two chips are not a block");
    assert_eq!(run.kingdom.stir.chips[&d.id], 2);
    swing(&mut run, 1);
    assert_eq!(run.block_of(1), Some("limestone"), "{:?}", run.players[&1].block);
    assert_eq!(run.kingdom.stir.cut[&d.id], 1);
    assert_eq!(run.kingdom.stir.chips[&d.id], 0);
    assert!(run.kingdom.stir.level >= stir::CUT);
    assert!(run.found_line().is_some_and(|l| l.contains("LIMESTONE BLOCK")), "{:?}", run.found_line());
}

#[test]
fn only_a_won_face_can_be_cut_and_only_by_a_knight_beside_it() {
    let (mut run, d) = won();
    run.home.industry.claims.get_mut(&d.id).unwrap().secured = false;
    at(&mut run, 1, d.x, d.y);
    swing(&mut run, 4);
    assert!(run.players[&1].block.is_none() && run.kingdom.stir.cut.is_empty(), "the guardian still holds it");
    run.home.industry.claims.get_mut(&d.id).unwrap().secured = true;
    run.kingdom.ore_sentries.remove(&d.id);
    at(&mut run, 1, d.x + 9.0, d.y);
    swing(&mut run, 4);
    assert!(run.players[&1].block.is_none(), "too far from the rock");
}

#[test]
fn a_block_is_heavy_and_a_full_pair_of_hands_cuts_no_more() {
    let (mut run, d) = won();
    let free = run.ground_pace_of(1).unwrap();
    at(&mut run, 1, d.x, d.y);
    swing(&mut run, 3);
    assert!(run.players[&1].block.is_some());
    let loaded = run.ground_pace_of(1).unwrap();
    assert!((loaded / free - BLOCK_PACE).abs() < 1e-4, "{free} -> {loaded}");
    swing(&mut run, 6);
    assert_eq!(run.kingdom.stir.cut[&d.id], 1, "hands full: no second block");
    assert!(run.found_line().is_some_and(|l| l.contains("HANDS FULL")) || run.kingdom.stir.chips[&d.id] <= 2);
}

#[test]
fn a_face_gives_a_dozen_blocks_and_is_worked_out() {
    let (mut run, d) = won();
    at(&mut run, 1, d.x, d.y);
    for n in 0..stir::FACE_BLOCKS {
        swing(&mut run, 3);
        assert!(run.players[&1].block.is_some(), "block {n}");
        run.players.get_mut(&1).unwrap().block = None;
    }
    assert_eq!(run.kingdom.stir.cut[&d.id], stir::FACE_BLOCKS);
    swing(&mut run, 4);
    assert!(run.players[&1].block.is_none(), "worked out for this expedition");
    assert!(run.kingdom.stir.valid());
    // A new expedition opens the face again.
    run.kingdom.stir.new_expedition();
    assert!(run.kingdom.stir.cut.is_empty());
}

#[test]
fn a_friend_takes_the_block_and_the_party_chains_it_along() {
    let (mut run, d) = won();
    run.join_seat(2, "Friend");
    at(&mut run, 1, d.x, d.y);
    at(&mut run, 2, d.x + 2.0, d.y + 1.0);
    run.players.get_mut(&1).unwrap().block = Some("limestone".into());
    // Too far to reach: nothing passes.
    at(&mut run, 2, d.x + 6.0, d.y);
    run.step(&BTreeMap::from([(2, Input { interact: true, ..Default::default() })]));
    assert!(run.players[&1].block.is_some() && run.players[&2].block.is_none());
    // Beside them, a fresh press of E takes it.
    at(&mut run, 2, d.x + 2.0, d.y + 1.0);
    release(&mut run);
    run.step(&BTreeMap::from([(2, Input { interact: true, ..Default::default() })]));
    assert!(run.players[&1].block.is_none(), "it left their back");
    assert_eq!(run.block_of(2), Some("limestone"));
    // And back again: a chain, not a one-way gift.
    release(&mut run);
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert_eq!(run.block_of(1), Some("limestone"));
    assert!(run.players[&2].block.is_none());
}

#[test]
fn the_block_is_set_down_at_the_stockyard_on_the_surface() {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    let (yx, yy) = quarrying::yard_spot();
    run.players.get_mut(&1).unwrap().block = Some("flint".into());
    at(&mut run, 1, yx + 20.0, yy);
    run.step(&BTreeMap::new());
    assert!(run.players[&1].block.is_some() && run.home.stockyard.is_empty(), "not at the yard yet");
    at(&mut run, 1, yx + 1.0, yy);
    run.step(&BTreeMap::new());
    assert!(run.players[&1].block.is_none());
    assert_eq!(run.home.stockyard.count("flint"), 1);
    assert_eq!(run.home.stockyard.hauled, 1);
    assert!(run.notices.iter().any(|n| n.contains("sets a block of flint")), "{:?}", run.notices);
    assert!(run.kingdom.stir.level >= stir::LIFT);
    assert!(run.valid_snapshot());
}

#[test]
fn a_full_yard_keeps_the_block_on_the_knights_back() {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    run.home.stockyard.stack("basalt", 16);
    let (yx, yy) = quarrying::yard_spot();
    run.players.get_mut(&1).unwrap().block = Some("flint".into());
    at(&mut run, 1, yx, yy);
    run.step(&BTreeMap::new());
    assert_eq!(run.block_of(1), Some("flint"), "nowhere to put it");
    assert!(run.found_line().is_some_and(|l| l.contains("STOCKYARD FULL")));
    assert_eq!(run.home.stockyard.held(), 16);
}

#[test]
fn a_fallen_knight_loses_the_block_and_the_stair_head_takes_one_going_down() {
    let (mut run, d) = won();
    at(&mut run, 1, d.x, d.y);
    run.players.get_mut(&1).unwrap().block = Some("limestone".into());
    run.players.get_mut(&1).unwrap().hp = 0;
    run.step(&BTreeMap::new());
    assert!(run.players[&1].block.is_none(), "dead men carry nothing");
    // Going down from home with one in hand: the porters stock it.
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    // (The Archive's quarry stone is `freestone` now: `slate` is the slater's roofing in the sim's catalogue.)
    run.players.get_mut(&1).unwrap().block = Some("freestone".into());
    run.stock_blocks_at_the_stair();
    assert!(run.players[&1].block.is_none());
    assert_eq!(run.home.stockyard.count("freestone"), 1);
}

#[test]
fn the_block_and_the_yard_survive_a_save_and_the_wire() {
    let (mut run, d) = won();
    at(&mut run, 1, d.x, d.y);
    swing(&mut run, 3);
    run.home.stockyard.stack("flint", 2);
    let back: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(back.valid_snapshot());
    assert_eq!(back.block_of(1), Some("limestone"));
    assert_eq!(back.home.stockyard, run.home.stockyard);
    assert_eq!(back.kingdom.stir, run.kingdom.stir);
    // A block of a stone that is not one is not a valid run.
    let mut bad = back.clone();
    bad.players.get_mut(&1).unwrap().block = Some("moon-cheese".into());
    assert!(!bad.valid_snapshot());
}

#[test]
fn yard_stacks_never_exceed_the_tier() {
    let mut y = Stockyard::default();
    for _ in 0..40 {
        y.stack("tufa", 1);
    }
    assert_eq!(y.held(), 16);
}
