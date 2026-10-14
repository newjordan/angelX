//! The way on: a cleared room's doorway toward the floor's next aim.
use super::*;

fn floor() -> Run {
    let mut run = Run::new(41, 1, None);
    run.enemies.clear();
    run
}

#[test]
fn a_cleared_room_points_one_step_closer_to_its_aim() {
    let mut run = floor();
    let (dir, _) = run.way_on().expect("a cleared start room points on");
    let next = run.dungeon.neighbour(run.at, dir).expect("through an open doorway");
    // Following it room by room arrives, never circling.
    let mut room = next;
    let mut steps = 0;
    while steps < 40 {
        run.enter_for_test(room);
        run.enemies.clear();
        let Some((dir, _)) = run.way_on() else {
            break;
        };
        room = run.dungeon.neighbour(room, dir).unwrap();
        steps += 1;
    }
    assert!(steps < 40, "the way on arrives");
}

#[test]
fn no_way_on_while_the_room_still_fights_or_up_in_the_world() {
    let mut run = Run::new(41, 1, None);
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(fight);
    if run.foes_left() {
        assert!(run.way_on().is_none(), "barred doors are not a way on");
    }
    let home = Run::at_home(41, 1, None, Default::default(), Default::default());
    assert!(home.way_on().is_none());
}

#[test]
fn a_new_party_in_the_undercroft_is_pointed_down_the_stair() {
    let mut home = Run::at_home(41, 1, None, Default::default(), Default::default());
    assert_eq!(home.home_way().map(|w| w.1), Some("Down the stair"));
    home.home.deepest = 1;
    assert!(home.home_way().is_none(), "once they have been down, no nagging");
}
