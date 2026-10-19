use super::*;
use crate::rules::{Phase, RoomKind, home, world};

#[test]
fn together_floor_plates_arrive_after_the_real_descent_and_expire() {
    for first in [Pack::Crypt, Pack::Cavern] {
        let mut run = Run::at_home(
            17,
            1,
            Some("Friend"),
            Default::default(),
            Default::default(),
        );
        run.dungeon.pack = first;
        run.descend();
        for depth in 1..=6 {
            let p = run.floor_plate(Some(1)).unwrap();
            assert_eq!(
                (p.kind, p.depth, p.age),
                (Kind::of(Pack::at(depth, first)), depth, 0)
            );
            assert_eq!(run.floor_plate(Some(2)), Some(p));
            assert!(matches!(run.phase, Phase::Exploring | Phase::Fighting));
            if depth < 6 {
                run.clear_prerequisites_for_test();
                if let Some(g) = &mut run.boss_gates {
                    g.guardian_defeated = true;
                    run.dungeon.rooms[g.guardian].cleared = true;
                }
                run.descend();
            }
        }
        // The floor behind's results, then the title.
        let plate = run.floor_plate(Some(1)).unwrap();
        assert_eq!(plate.length(), crate::rules::floor_stats::SHOWN + DURATION);
        for _ in 0..plate.length() {
            run.step(&BTreeMap::new());
        }
        assert!(run.floor_plates.is_empty());
    }
}

#[test]
fn together_floor_plates_skip_is_seat_local_and_the_shared_sim_keeps_running() {
    let mut run = Run::new(17, 1, Some("Friend"));
    run.begin_floor_plate(Kind::of(run.dungeon.pack));
    let x = run.players[&2].x;
    let tick = run.tick;
    run.step(&BTreeMap::from([
        (
            1,
            Input {
                skip_plate: true,
                ..Default::default()
            },
        ),
        (
            2,
            Input {
                move_x: 1,
                ..Default::default()
            },
        ),
    ]));
    assert!(run.floor_plate(Some(1)).is_none());
    assert_eq!(run.floor_plate(Some(2)).unwrap().age, 1);
    assert_eq!(run.tick, tick + 1);
    assert!(run.players[&2].x > x);
    assert!(!run.skip_floor_plate(1));
    run.mode = super::super::fortune::Mode::Turbo;
    for _ in 1..DURATION {
        run.step(&BTreeMap::new());
    }
    assert!(
        run.floor_plates.is_empty(),
        "Doubletime still has 54 display steps"
    );
}

#[test]
fn together_floor_plates_gate_hold_and_wrens_landing_use_arrival_depth() {
    let mut run = Run::at_home(
        17,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    run.come_to_the_gate();
    let entrance = world::ENTRANCES
        .iter()
        .find(|e| e.room == RoomKind::Gate && matches!(e.to, world::Dest::Room(RoomKind::Home, _)))
        .unwrap();
    let centre = world::entrance_centre(entrance);
    // The whole party stands on the plate: it moves them all together.
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = centre;
    }
    run.stairs_held = false;
    for _ in 1..home::DESCEND_HOLD {
        run.step(&BTreeMap::new());
        assert!(run.floor_plates.is_empty(), "the hold is not an arrival");
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.floor_plate(Some(1)).unwrap().kind, Kind::Gate);
    assert_eq!(run.floor(), 0);
    // The map table has earned floor 4 and chosen that landing.
    run.home.deepest = 4;
    run.home.landing = 4;
    run.home.levels.insert(home::Station::Map, 3);
    run.descend();
    assert_eq!(run.floor_plate(Some(1)).unwrap().kind, Kind::Archive);
    assert_eq!(run.floor_plate(Some(1)).unwrap().depth, 4);
    let encoded = serde_json::to_vec(&run).unwrap();
    let copy: Run = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(run.floor_plates, copy.floor_plates);
}

#[test]
fn together_floor_plates_stairs_and_mine_shaft_trigger_after_arrival() {
    let mut run = Run::at_home(17, 1, None, Default::default(), Default::default());
    run.come_to_the_gate();
    let shaft = world::ENTRANCES
        .iter()
        .find(|e| matches!(e.to, world::Dest::Delve(Pack::Cavern)))
        .unwrap();
    run.take_entrance(shaft);
    assert_eq!(run.floor_plate(Some(1)).unwrap().kind, Kind::Mines);
    run.clear_prerequisites_for_test();
    let stairs = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    if let Some(g) = &mut run.boss_gates {
        g.guardian_defeated = true;
        run.dungeon.rooms[g.guardian].cleared = true;
    }
    run.enter_for_test(stairs);
    run.phase = Phase::Exploring;
    run.dungeon.rooms[stairs].cleared = true;
    run.enemies.clear();
    run.calm_for_test();
    let room = run.room();
    let (c, r) = (0..room.rows as i32)
        .flat_map(|r| (0..room.cols as i32).map(move |c| (c, r)))
        .find(|&(c, r)| room.tile(c, r) == super::super::Tile::Stairs)
        .unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((c as f32 + 0.5) * 2.0, (r as f32 + 0.5) * 2.0);
    run.stairs_held = false;
    run.step(&BTreeMap::new());
    assert_eq!(run.floor(), 2);
    assert_eq!(run.floor_plate(Some(1)).unwrap().kind, Kind::Crypt);
    assert_eq!(run.floor_plate(Some(1)).unwrap().age, 0);
}

#[test]
fn together_floor_plates_live_mirror_carries_animation_and_dismissal() {
    let mut host = Run::new(17, 1, Some("Friend"));
    host.begin_floor_plate(Kind::of(host.dungeon.pack));
    host.step(&BTreeMap::new());
    let mut guest = host.wire_copy();
    assert_eq!(guest.floor_plate(Some(2)).unwrap().age, 1);
    host.step(&BTreeMap::from([(
        2,
        Input {
            skip_plate: true,
            ..Default::default()
        },
    )]));
    guest = host.wire_copy();
    assert!(guest.floor_plate(Some(2)).is_none());
    assert!(guest.floor_plate(Some(1)).is_some());
}

#[test]
fn together_floor_plates_discard_on_a_wipe_or_victory() {
    for phase in [Phase::Won, Phase::Wiped] {
        let mut run = Run::new(17, 1, None);
        run.begin_floor_plate(Kind::of(run.dungeon.pack));
        run.phase = phase;
        run.step(&BTreeMap::new());
        assert!(run.floor_plates.is_empty());
    }
}

#[test]
fn together_floor_plate_skip_wins_over_a_vote_and_gameplay_in_the_same_input() {
    use crate::rules::votes::{Action, Command};
    let mut run = Run::new(17, 1, Some("Friend"));
    run.vote_command(
        2,
        Command {
            serial: 1,
            action: Action::Toggle,
        },
    );
    run.begin_floor_plate(Kind::of(run.dungeon.pack));
    let before = run.players[&2].clone();
    run.step(&BTreeMap::from([(
        2,
        Input {
            skip_plate: true,
            vote: Some(Command {
                serial: 2,
                action: Action::Rate(5),
            }),
            move_x: 1,
            fire: true,
            ..Default::default()
        },
    )]));
    assert!(run.floor_plate(Some(2)).is_none());
    assert!(run.floor_plate(Some(1)).is_some());
    assert!(run.playtest.open(2));
    assert_eq!(run.playtest.seats[&2].serial, 1);
    assert!(run.playtest.records.is_empty());
    assert_eq!((run.players[&2].x, run.players[&2].y), (before.x, before.y));
    run.step(&BTreeMap::from([(
        2,
        Input {
            vote: Some(Command {
                serial: 2,
                action: Action::Rate(5),
            }),
            ..Default::default()
        },
    )]));
    assert_eq!(
        run.playtest.records["Friend"]["controls"],
        crate::rules::votes::Rating::Stars(5)
    );
}
