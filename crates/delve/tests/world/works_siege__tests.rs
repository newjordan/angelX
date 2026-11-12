//! The siege rows: what each stone stands on, derived from the stones, set beside what
//! Blocks' own export says of the same stones, and the one-pass rule the game plays.

use super::*;
use crate::world::works_siege::Rests;
use std::collections::BTreeSet;

/// The first 2,500 stones of a Blocks great tower (v2 export), with its own `rests`.
const BLOCKS: &str = include_str!("../fixtures/blocks-sol-siege.json");

fn blocks_castle() -> Castle {
    // The baker reads v1; a v2 file is v1 and more.
    Castle::from_json("sol", &BLOCKS.replacen("angel-castle/v2", "angel-castle/v1", 1)).expect("the fixture bakes")
}

#[test]
fn rows_derived_from_the_stones_agree_with_blocks_own_rows() {
    let castle = blocks_castle();
    let theirs = Rests::of_json(BLOCKS).expect("the export's own rows");
    let ours = Rests::of(&castle);
    assert_eq!((castle.stones.len(), theirs.len(), ours.len()), (2500, 2500, 2500));
    // The same holders, row for row, for most; and where they differ, never by the ground.
    let (mut same, mut ground_off) = (0, 0);
    for i in 0..2500u32 {
        let (a, b) = (ours.row(i), theirs.row(i));
        if a == b {
            same += 1;
        }
        if (a.first() == Some(&0)) != (b.first() == Some(&0)) {
            ground_off += 1;
        }
    }
    let hanging = (0..2500u32).filter(|&i| ours.row(i).is_empty()).count();
    eprintln!("rows identical: {same}/2500; ground disagreements: {ground_off}; hanging: {hanging} (Blocks: 0)");
    assert!(same >= 2500 * 80 / 100, "{same} of 2500 rows are Blocks' own");
    assert!(hanging <= 2500 / 20, "{hanging} stones rest on nothing");
    assert!(ground_off <= 25, "{ground_off} stones disagree on whether they sit on the ground");
}

#[test]
fn losing_a_stone_brings_down_the_same_stones_by_either_set_of_rows() {
    let castle = blocks_castle();
    let theirs = Rests::of_json(BLOCKS).unwrap();
    let ours = Rests::of(&castle);
    // Lose a stone from the top of the first lift's laid courses, as a raid does (a handful,
    // spread through the stones laid in the lift in hand), and compare what falls.
    let (mut agree, mut total) = (0, 0);
    for lost in (900..1100).step_by(7) {
        let gone = BTreeSet::from([lost]);
        let a: BTreeSet<u32> = ours.falls(0, 1200, &gone).into_iter().collect();
        let b: BTreeSet<u32> = theirs.falls(0, 1200, &gone).into_iter().collect();
        total += 1;
        let jaccard = if a.is_empty() && b.is_empty() { 1.0 } else { a.intersection(&b).count() as f32 / a.union(&b).count() as f32 };
        if jaccard >= 0.6 {
            agree += 1;
        }
    }
    eprintln!("falls that agree: {agree}/{total}");
    assert!(agree * 4 >= total * 3, "{agree}/{total}");
}

#[test]
fn the_one_pass_rule_is_blocks_majority_rule() {
    // A stone on the ground stands always; one on two stones needs both; one on three needs two.
    let rests = Rests::of_json(r#"{"rests":[[0],[0],[1,2],[1,2,3],[1],[]]}"#).unwrap();
    let none = BTreeSet::new();
    assert!(rests.falls(0, 6, &none).is_empty(), "nothing lost, nothing falls");
    // Losing stone 1: stone 2 rests on [1,2] back = stones 1 and 0 (one lost of two): falls.
    // Stone 3 rests on [1,2,3] back = stones 2, 1, 0: two of three would stand but 2 fell, so 1 of 3: falls.
    // Stone 4 rests on stone 3: falls. Stone 5 rests on nothing (falsework): stands.
    assert_eq!(rests.falls(0, 6, &BTreeSet::from([1])), vec![2, 3, 4]);
    assert_eq!(rests.held_by(1, 0, 6, &none), vec![2, 3, 4]);
    // Stones before `from` are finished and stand.
    assert!(rests.falls(2, 6, &BTreeSet::from([4])).is_empty());
    // A stone lost on its own is not "fallen": it is the loss.
    assert!(!rests.falls(0, 6, &BTreeSet::from([0])).contains(&0));
}

#[test]
fn the_keeps_rows_are_derived_once_and_they_are_sane() {
    let castle = castle(KEEP).expect("the great tower");
    let rows = rests(KEEP).expect("its rows");
    assert_eq!(rows.len(), castle.stones.len());
    // The footings sit on the ground, and every stone but a few held by falsework has a holder.
    let grounded = (0..castle.len()).filter(|&i| rows.row(i).first() == Some(&0)).count();
    let hanging = (0..castle.len()).filter(|&i| rows.row(i).is_empty()).count();
    assert!(grounded > 100, "{grounded} stones on the ground");
    assert!(hanging * 20 < castle.stones.len(), "{hanging} stones hang on nothing");
    // Every holder is earlier in lay order.
    for i in 0..castle.len() {
        assert!(rows.row(i).iter().all(|&d| d == 0 || u32::from(d) <= i), "stone {i}");
    }
    // The same plan twice is the same rows.
    assert!(std::ptr::eq(rows, rests(KEEP).unwrap()));
    assert!(rests("no-such-plan").is_none());
}
