//! The resolution ladder's rung rule: sixteen at the run's start, a rung a
//! stretch of floors (or levels) after, the top in the Deep; the knob read
//! as the environment says it.
use super::*;
use crate::rules::talents::Prowess;

fn run_on(depth: u32) -> Run {
    let mut run = Run::new(7, 7, Some("Friend"));
    run.dungeon.depth = depth;
    run
}

#[test]
fn a_run_starts_at_sixteen_and_climbs_by_floor() {
    set_rule(None);
    let id = *run_on(1).players.keys().next().unwrap();
    let rungs: Vec<u32> = (1..=9).map(|d| rung_by(&run_on(d), id, Rule::Floor).px()).collect();
    assert_eq!(rungs, [16, 32, 32, 64, 64, 64, 128, 128, 128]);
    assert_eq!(rung_by(&run_on(0), id, Rule::Floor), Rung::BASE, "home is the game's grain");
}

#[test]
fn the_deep_counts_its_own_floors() {
    let mut run = run_on(1);
    let id = *run.players.keys().next().unwrap();
    run.deep_floor_for_test(12);
    assert_eq!(run.floor(), 12);
    assert_eq!(rung_by(&run, id, Rule::Floor).px(), 128);
}

#[test]
fn by_level_a_knight_climbs_as_the_realm_keeps_them() {
    let mut run = run_on(1);
    let id = *run.players.keys().next().unwrap();
    // The realm keeps prowess under the knight's key (their company knight, else the
    // lowercased name), not under the name as typed.
    run.players.get_mut(&id).unwrap().name = "Ada".into();
    let key = crate::rules::talents::knight_key(&run.players[&id]);
    assert_eq!(rung_by(&run, id, Rule::Level), Rung::BASE, "a knight the realm has not met is level 1");
    for (xp, px) in [(0, 16), (600, 32), (1800, 64), (3600, 64), (6000, 128)] {
        run.home.knights.insert(key.clone(), Prowess { xp, learned: Vec::new() });
        assert_eq!(rung_by(&run, id, Rule::Level).px(), px, "xp {xp}");
    }
}

#[test]
fn the_knob_and_what_lies_ahead() {
    assert_eq!(parse_rule(Some("floor")), Some(Rule::Floor));
    assert_eq!(parse_rule(Some("level")), Some(Rule::Level));
    assert_eq!(parse_rule(Some("64")), Some(Rule::Fixed(Rung::of(64).unwrap())));
    assert_eq!(parse_rule(Some("128px")), Some(Rule::Fixed(Rung::of(128).unwrap())));
    assert_eq!(parse_rule(Some("48")), None, "not a rung");
    assert_eq!(parse_rule(None), None);
    let id = *run_on(1).players.keys().next().unwrap();
    set_rule(Some(Rule::Floor));
    assert_eq!(next(&run_on(1), id), Some((Rung::of(32).unwrap(), Climb::Floor(2))));
    assert_eq!(next(&run_on(3), id), Some((Rung::of(64).unwrap(), Climb::Floor(4))));
    assert_eq!(next(&run_on(8), id), None, "the top");
    set_rule(Some(Rule::Fixed(Rung::of(64).unwrap())));
    assert_eq!(rung(&run_on(1), id).px(), 64);
    assert_eq!(next(&run_on(1), id), None);
    set_rule(None);
    assert_eq!(Rung::of(32).unwrap().scale(), 2);
    assert_eq!(Rung::of(128).unwrap().scale(), 8);
    assert_eq!(Rung::BASE.next(), Rung::of(32));
    assert_eq!(Rung::of(128).unwrap().next(), None);
}
