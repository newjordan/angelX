//! The crab heist on the Strand: a chase for the whole party.
use super::*;
use crate::rules::{home::Home, world};

/// The party on the Strand, the keeper's errand taken, far from the crab.
fn on_the_strand() -> Run {
    let mut run = Run::at_home(13, 7, None, Home::default(), Spoils::default());
    run.join_seat(2, "Two");
    run.marks.insert(QUEST_MARKS[0].into(), 1);
    run.arrive(world::room_of(&run.dungeon, RoomKind::Strand).unwrap(), (6.0, 7.0));
    run.tick = 0;
    run
}

fn still() -> BTreeMap<u32, Input> {
    BTreeMap::new()
}

#[test]
fn the_crab_runs_from_the_nearest_knight_and_keeps_to_the_sand() {
    let mut run = on_the_strand();
    run.tick_coastal(&still());
    let start = run.crab.clone().expect("the thief is out");
    for _ in 0..HZ {
        run.tick += 1;
        run.tick_coastal(&still());
    }
    let crab = run.crab.clone().unwrap();
    // The knights are west of it: it went east.
    assert!(crab.x > start.x, "{start:?} -> {crab:?}");
    let tile = run.room().tile((crab.x / TILE_UNITS) as i32, (crab.y / TILE_UNITS) as i32);
    assert_eq!(tile, Tile::Floor, "never into the water");
    assert!(run.valid_snapshot());
}

#[test]
fn at_high_tide_it_digs_in_and_cannot_be_grabbed() {
    let mut run = on_the_strand();
    run.tick = u64::from(9 * HZ);
    run.tick_coastal(&still());
    let crab = run.crab.clone().unwrap();
    assert!(crab.dug);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (crab.x, crab.y);
    run.tick_coastal(&still());
    assert_eq!(stage(&run.home.countryside, &run.marks), 1, "dug in");
    assert_eq!(run.crab.as_ref().map(|c| (c.x, c.y)), Some((crab.x, crab.y)), "it holds still");
}

#[test]
fn whoever_grabs_it_gets_the_oil_and_the_crown_and_a_friend_sees_it() {
    let mut run = on_the_strand();
    run.tick_coastal(&still());
    let friend = run.wire_copy();
    assert_eq!(friend.crab, run.crab);
    let crab = run.crab.clone().unwrap();
    let h = run.players.get_mut(&2).unwrap();
    (h.x, h.y) = (crab.x + 0.5, crab.y);
    run.tick_coastal(&still());
    assert_eq!(stage(&run.home.countryside, &run.marks), 2);
    assert!(run.crab.is_none());
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    assert!(run.found.as_ref().unwrap().2.contains("TWO GOT THE OIL"));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_crab_shots -- --ignored`
#[test]
#[ignore]
fn write_crab_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("crab");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = on_the_strand();
    // Both knights chase it east.
    let east = BTreeMap::from([
        (1, Input { move_x: 1, ..Default::default() }),
        (2, Input { move_x: 1, ..Default::default() }),
    ]);
    for _ in 0..HZ * 2 {
        run.step(&east);
    }
    save(&run, "a-chase");
    while low_tide(run.tick + 1) {
        run.step(&still());
    }
    for _ in 0..HZ {
        run.step(&still());
    }
    save(&run, "b-dug");
}
