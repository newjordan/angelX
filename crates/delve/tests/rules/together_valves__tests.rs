//! Brother Bucket's sluice: three sprung valves, all open at once.
use super::*;
use crate::rules::{home::Home, world};

fn at_the_sluice() -> Run {
    let mut run = Run::at_home(31, 1, None, Home::default(), Spoils::default());
    run.join_seat(2, "Two");
    run.marks.insert(QUEST_MARKS[0].into(), 1);
    run.arrive(world::room_of(&run.dungeon, RoomKind::Sluice).unwrap(), (12.0, 7.0));
    run
}

fn turn(run: &mut Run, id: u32, valve: usize) {
    let at = ERRAND[1 + valve].1;
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
    run.tick += 1;
    run.tick_wetland(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
}

fn wait(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.tick += 1;
        run.tick_wetland(&BTreeMap::new());
    }
}

#[test]
fn a_valve_springs_shut_and_one_open_alone_drains_nothing() {
    let mut run = at_the_sluice();
    turn(&mut run, 1, 0);
    assert_eq!(run.valves.unwrap()[0], VALVE_OPEN);
    wait(&mut run, VALVE_OPEN);
    assert_eq!(run.valves.unwrap()[0], 0, "sprang shut");
    assert!(run.found.as_ref().unwrap().2.contains("SPROING"));
    assert_eq!(stage(&run.home.countryside, &run.marks), 1);
}

#[test]
fn too_slow_alone_but_two_knights_drain_it() {
    let mut run = at_the_sluice();
    turn(&mut run, 1, 0);
    wait(&mut run, VALVE_OPEN / 2);
    turn(&mut run, 1, 1);
    wait(&mut run, VALVE_OPEN / 2 + 1);
    turn(&mut run, 1, 2);
    assert_eq!(stage(&run.home.countryside, &run.marks), 1, "the fish shut first");
    // Together: one holds the fish, the other runs boot and duck.
    turn(&mut run, 2, 0);
    turn(&mut run, 1, 1);
    assert_eq!(stage(&run.home.countryside, &run.marks), 4);
    assert!(run.valves.is_none());
    assert!(run.found.as_ref().unwrap().2.contains("DRAINS"));
}

#[test]
fn whoever_fishes_out_the_bell_wears_the_crown() {
    let mut run = at_the_sluice();
    for v in 0..3 {
        turn(&mut run, 1, v);
    }
    let (kind, at) = ERRAND[4];
    run.arrive(world::room_of(&run.dungeon, kind).unwrap(), (12.0, 7.0));
    let h = run.players.get_mut(&2).unwrap();
    (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
    run.tick_wetland(&BTreeMap::from([(2, Input { interact: true, ..Default::default() })]));
    assert_eq!(stage(&run.home.countryside, &run.marks), 5);
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_valve_shots -- --ignored`
#[test]
#[ignore]
fn write_valve_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("valves");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = at_the_sluice();
    run.step(&BTreeMap::new());
    save(&run, "a-shut");
    turn(&mut run, 1, 0);
    wait(&mut run, VALVE_OPEN - HZ);
    turn(&mut run, 2, 2);
    wait(&mut run, HZ / 2);
    save(&run, "b-two-open");
}
