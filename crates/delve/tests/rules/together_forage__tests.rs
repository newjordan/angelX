//! The badger's forage: three mushrooms at once, a Boot that kicks, and the
//! knight who picked the most made Deputy Stirrer.
use super::*;
use crate::rules::{home::Home, world};

fn taken() -> Run {
    let mut run = Run::at_home(31, 1, None, Home::default(), Spoils::default());
    run.join_seat(2, "Two");
    run.marks.insert(QUEST_MARKS[0].into(), 1);
    run
}

fn press(run: &mut Run, id: u32, i: usize) {
    let (kind, at) = ERRAND[i];
    if run.room().kind != kind {
        run.arrive(world::room_of(&run.dungeon, kind).unwrap(), (12.0, 7.0));
    }
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
    run.tick += 1;
    run.tick_woodland(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
}

#[test]
fn the_boot_kicks_the_first_hand_and_shoves_its_knight_off() {
    let mut run = taken();
    press(&mut run, 1, 1);
    let f = run.forage.as_ref().unwrap();
    assert!(f.kicked && f.carried[0] == 0);
    let at = ERRAND[1].1;
    let h = &run.players[&1];
    assert!((h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) > 1.0, "shoved off");
    assert!(run.found.as_ref().unwrap().2.contains("KICKED"));
}

#[test]
fn the_cook_wants_all_three_and_the_best_picker_stirs() {
    let mut run = taken();
    press(&mut run, 1, 1);
    press(&mut run, 2, 1);
    press(&mut run, 2, 2);
    press(&mut run, 1, 3);
    assert_eq!(run.forage.as_ref().unwrap().carried, [2, 2, 1]);
    // Two picks for knight two, one for knight one.
    run.forage.as_mut().unwrap().carried[2] = 0;
    run.found = None;
    press(&mut run, 1, 4);
    assert_eq!(stage(&run.home.countryside, &run.marks), 1, "only two in hand");
    assert!(run.found.as_ref().unwrap().2.contains("THREE"));
    press(&mut run, 1, 3);
    press(&mut run, 1, 4);
    assert_eq!(stage(&run.home.countryside, &run.marks), 5);
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    assert!(run.found.as_ref().unwrap().2.contains("TWO IS DEPUTY STIRRER"));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_forage_shots -- --ignored`
#[test]
#[ignore]
fn write_forage_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("forage");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = taken();
    run.arrive(world::room_of(&run.dungeon, RoomKind::HedgeMaze).unwrap(), (12.0, 7.0));
    run.step(&BTreeMap::new());
    save(&run, "a-maze");
    press(&mut run, 2, 2);
    press(&mut run, 1, 1);
    run.step(&BTreeMap::new());
    save(&run, "b-kicked-hat-carried");
    press(&mut run, 1, 1);
    press(&mut run, 2, 3);
    run.arrive(world::room_of(&run.dungeon, RoomKind::Bothy).unwrap(), ERRAND[4].1);
    run.step(&BTreeMap::new());
    save(&run, "c-home-with-three");
}
