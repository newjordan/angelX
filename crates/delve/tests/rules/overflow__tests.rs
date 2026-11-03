//! Each overflow the damaged-save fuzz used to hit, proved on its own.
//! These call the site directly (no mend), so a saturating fix that is
//! removed fails here even if restore would have clamped the value.

use super::*;

#[test]
fn a_heavy_hit_check_survives_a_huge_max_hp() {
    let mut run = Run::new(1, 1, None);
    let hero = run.players.get_mut(&1).unwrap();
    hero.max_hp = u32::MAX;
    hero.hp = u32::MAX;
    run.step(&BTreeMap::new());
    assert!(!run.over());
}

#[test]
fn a_critter_timer_at_the_top_does_not_wrap() {
    let mut run = Run::new(1, 1, None);
    run.hold(Hold::Open);
    let id = run.spawn_body(EnemyKind::Biter, 12.0, 12.0).expect("a biter");
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().timer = u32::MAX;
    run.step(&BTreeMap::new());
    let foe = run.enemies.iter().find(|e| e.id == id).unwrap();
    assert_eq!(foe.timer, u32::MAX);
}

#[test]
fn a_foe_age_at_the_top_does_not_wrap() {
    let mut run = Run::new(1, 1, None);
    run.hold(Hold::Open);
    let id = run.spawn_body(EnemyKind::Slime, 12.0, 12.0).expect("a foe");
    {
        let foe = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        foe.age = u32::MAX - 1;
        foe.dormant = false;
        foe.hp = 10;
    }
    run.step(&BTreeMap::new());
    let age = run.enemies.iter().find(|e| e.id == id).unwrap().age;
    assert_eq!(age, u32::MAX, "the hunt ticks age");
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().age = u32::MAX;
    run.step(&BTreeMap::new());
    assert_eq!(run.enemies.iter().find(|e| e.id == id).unwrap().age, u32::MAX);
}

#[test]
fn a_spin_longer_than_its_turn_does_not_underflow() {
    let mut run = Run::new(1, 1, None);
    let hero = run.players.get_mut(&1).unwrap();
    hero.spin = u32::MAX;
    let _ = hero.own_keys(&Input::default(), false, true);
    assert_eq!(hero.spin, u32::MAX - 1);
}

#[test]
fn a_full_shot_tally_does_not_overflow() {
    let mut run = Run::new(1, 1, None);
    run.floor_tally.line(1).shots = u32::MAX;
    let mut inputs = BTreeMap::new();
    inputs.insert(1, Input { fire: true, aim_x: 1, ..Default::default() });
    run.step(&inputs);
    assert_eq!(run.floor_tally.line(1).shots, u32::MAX);
    assert!(run.sounds.iter().any(|s| s.starts_with("shot_")), "{:?}", run.sounds);
}

#[test]
fn a_full_fought_tally_does_not_overflow() {
    let mut tally = floor_stats::Tally::default();
    tally.line(1).fought = u32::MAX;
    tally.count(1, 0, true);
    assert_eq!(tally.line(1).fought, u32::MAX);
}
