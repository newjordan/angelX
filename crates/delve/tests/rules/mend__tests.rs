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
