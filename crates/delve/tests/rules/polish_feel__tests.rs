use super::*;
use crate::rules::feel::testing::quiet;
use std::collections::BTreeMap;

fn phase_change() -> Run {
    let mut run = quiet();
    run.spawn_boss_at_for_test(0, 24.0, 9.0);
    run.enemies[0].age = super::super::TELEGRAPH + 1;
    let (definition, _) = bosses::check("phase-test",
        "name Test\nwhere crypt\nphases flower sweep every=40 volleys=2\nart\nkkkk\nkkkk\n").unwrap();
    run.bosses[0].phases = definition.phases;
    for _ in 0..100 {
        run.step(&BTreeMap::new());
        if run.spectacle.phase.is_some() { return run; }
    }
    panic!("the authored volley chain must change phase");
}

#[test]
fn guardian_pattern_feedback_is_once_deterministic_and_returns_to_rest() {
    let mut run = phase_change();
    let same = phase_change();
    assert_eq!(serde_json::to_value(&run).unwrap(), serde_json::to_value(&same).unwrap());
    let mark = run.spectacle.phase.unwrap();
    assert!(!mark.rage);
    assert_eq!(mark.born, run.tick);
    assert_eq!(run.sounds.iter().filter(|&&s| s == "boss_roar").count(), 1);
    run.finish_hitstop();
    assert!(run.hitstop.left > 0);
    let e = &run.enemies[0];
    let unchanged = vec![(e.id, e.raged, e.frozen, run.boss_phase(e))];
    let hp = run.players.values().map(|h| (h.hp, h.max_hp)).collect();
    run.tally_wounds(hp, unchanged);
    assert_eq!(run.sounds.iter().filter(|&&s| s == "boss_roar").count(), 1);
    run.enemies.clear();
    for _ in 0..PHASE_MARK + 20 { run.step(&BTreeMap::new()); }
    assert!(run.spectacle.phase.is_none());
}

#[test]
fn phase_mark_old_saves_and_new_wire_round_trip() {
    let run = phase_change();
    let mut value = serde_json::to_value(&run).unwrap();
    value["spectacle"].as_object_mut().unwrap().remove("phase");
    let old: Run = serde_json::from_value(value).unwrap();
    assert!(old.spectacle.phase.is_none());
    let mut publisher = crate::wire::Publisher::default();
    let tree = publisher.publish(&run).unwrap();
    let got = crate::wire::read(&tree).unwrap();
    assert_eq!(got.spectacle.phase, run.spectacle.phase);
    let mut changed = run.clone();
    changed.at = (run.at + 1) % run.dungeon.rooms.len();
    let seen = changed.seen();
    changed.watch(seen);
    assert!(changed.spectacle.phase.is_none());
}

#[test]
fn rage_uses_its_existing_sound_and_a_guardians_fall_clears_the_phase_sign() {
    let mut run = quiet();
    run.spawn_boss_at_for_test(0, 24.0, 9.0);
    run.enemies[0].age = super::super::TELEGRAPH + 1;
    run.enemies[0].hp = run.enemies[0].max_hp / 10;
    run.bosses[0].rage = 0.5;
    run.step(&BTreeMap::new());
    assert!(run.spectacle.phase.unwrap().rage);
    assert_eq!(run.sounds.iter().filter(|&&s| s == "boss_rage").count(), 1);
    assert!(!run.sounds.contains(&"boss_roar"), "no second roar on rage");
    run.hitstop = Default::default();
    run.enemies[0].hp = 0;
    run.step(&BTreeMap::new());
    assert!(!run.spectacle.corpses.is_empty());
    assert!(run.spectacle.phase.is_none(), "no stale rage label over the corpse");
    assert!(run.sounds.contains(&"boss_death"));
}

#[test]
fn phase_marks_reject_nonfinite_and_out_of_room_snapshot_data() {
    let mut run = phase_change();
    assert!(run.valid_snapshot());
    run.spectacle.phase.as_mut().unwrap().x = f32::NAN;
    assert!(!run.valid_snapshot());
    run.spectacle.phase.as_mut().unwrap().x = 24.0;
    run.spectacle.phase.as_mut().unwrap().room = run.dungeon.rooms.len();
    assert!(!run.valid_snapshot());
}
