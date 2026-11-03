use super::*;
use crate::rules::exits::Entry;
use std::collections::BTreeMap;

#[test]
fn a_sound_run_is_left_as_it_is() {
    let mut run = Run::new(7, 1, None);
    let before = format!("{:?}", (&run.exits, &run.kingdom.return_to, run.at));
    assert!(run.mend().is_empty());
    assert_eq!(before, format!("{:?}", (&run.exits, &run.kingdom.return_to, run.at)));
    assert!(crate::wire::drawable(&run) && run.valid_snapshot());
}

#[test]
fn what_froze_every_page_is_put_right_and_the_run_reads_again() {
    let mut run = Run::new(7, 1, None);
    let rooms = run.dungeon.rooms.len();
    // The race's way back with knights past its edge (the play box,
    // 2026-10-17), a way back to a room that is not there, a knight who is
    // nowhere, and one waiting outside who has left.
    run.exits.entry = Some(Entry { at: run.at, party: BTreeMap::from([(1, (-40.0, 9999.0))]) });
    run.exits.outside.insert(3, Entry { at: rooms + 5, party: BTreeMap::new() });
    run.players.get_mut(&1).unwrap().x = f32::NAN;
    assert!(!crate::wire::drawable(&run) && !run.valid_snapshot());

    let mended = run.mend();
    assert!(mended.contains(&"exits") && mended.contains(&"knight"), "{mended:?}");
    assert!(crate::wire::drawable(&run), "every page draws it again");
    assert!(run.valid_snapshot(), "the save comes back");
    let (x, y) = run.exits.entry.as_ref().unwrap().party[&1];
    let room = run.room();
    assert!((0.0..=room.width()).contains(&x) && (0.0..=room.height()).contains(&y));
    assert!(run.exits.outside.is_empty());
    assert!(run.mend().is_empty(), "once is enough");
}

#[test]
fn a_kingdom_way_back_off_the_map_comes_back_onto_it() {
    let mut run = Run::new(7, 1, None);
    run.kingdom.return_to = Some(crate::rules::kingdom::Return {
        x: -3.0,
        y: f32::INFINITY,
        name: "the gate".into(),
        root: run.room().kind,
        party: BTreeMap::from([(1, (1.0e9, 4.0))]),
    });
    assert!(!run.kingdom.valid());
    assert_eq!(run.mend(), vec!["kingdom"]);
    assert!(run.kingdom.valid() && run.kingdom.return_to.is_some());
    assert!(crate::wire::drawable(&run) && run.valid_snapshot());
}

#[test]
fn a_foe_far_outside_the_world_is_pulled_back() {
    let mut run = Run::new(1, 1, None);
    let id = run.spawn_body(EnemyKind::Slime, 10.0, 10.0).expect("a foe");
    let foe = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
    foe.x = 1.0e20;
    foe.y = -1.0e9;
    assert!(run.mend().contains(&"place"));
    let foe = run.enemies.iter().find(|e| e.id == id).unwrap();
    assert!(foe.x.is_finite() && foe.x.abs() <= 8192.0);
    assert!(foe.y.is_finite() && foe.y.abs() <= 8192.0);
    let _ = run.step(&BTreeMap::new());
}

#[test]
fn a_huge_hp_is_capped_before_it_can_reach_play() {
    let mut run = Run::new(1, 1, None);
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = 4_000_000_000;
    hero.max_hp = u32::MAX;
    hero.boon.plus = u32::MAX;
    assert!(run.mend().contains(&"health"));
    let hero = run.players.get(&1).unwrap();
    assert!(hero.max_hp <= 100_000);
    assert!(hero.hp <= hero.max_hp);
    assert!(hero.boon.plus <= 1_000);
    let _ = run.step(&BTreeMap::new());
}

#[test]
fn a_censer_past_any_pulse_is_cut_back() {
    let mut run = Run::new(1, 1, None);
    run.players.get_mut(&1).unwrap().censer = u32::MAX;
    assert!(run.mend().contains(&"health"));
    assert!(run.players.get(&1).unwrap().censer <= 1_000);
    let _ = run.step(&BTreeMap::new());
}

#[test]
fn a_huge_mend_does_not_overflow_its_worth() {
    let got = stakes::landed(u32::MAX, stakes::Mend::Party, stakes::Stakes::Fight);
    assert!(got >= 1);
    assert!(got < u32::MAX);
}

#[test]
fn a_spin_past_its_length_is_cut_back() {
    let mut run = Run::new(1, 1, None);
    run.players.get_mut(&1).unwrap().spin = u32::MAX;
    assert!(run.mend().contains(&"spin"));
    assert!(run.players.get(&1).unwrap().spin <= spin::TICKS);
}
