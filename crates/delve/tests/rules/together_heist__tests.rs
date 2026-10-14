//! The goose heist: grab the sail, outrun the goose, honk to help a friend.
use super::*;
use crate::rules::{home::Home, world};

fn on_the_green() -> Run {
    let mut run = Run::at_home(31, 1, None, Home::default(), Spoils::default());
    run.join_seat(2, "Two");
    run.marks.insert(QUEST_MARKS[0].into(), 1);
    run.arrive(world::room_of(&run.dungeon, RoomKind::GooseGreen).unwrap(), (12.0, 7.0));
    run
}

fn press(run: &mut Run, id: u32) {
    run.tick += 1;
    run.tick_windcountry(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
    run.tick += 1;
    run.tick_windcountry(&BTreeMap::new());
}

fn put(run: &mut Run, id: u32, at: (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = at;
    h.invulnerable = 0;
}

#[test]
fn the_goose_chases_the_thief_and_takes_the_sail_back() {
    let mut run = on_the_green();
    put(&mut run, 1, (CLOTH.0 * TILE_UNITS, CLOTH.1 * TILE_UNITS + 1.0));
    press(&mut run, 1);
    assert_eq!(run.heist.as_ref().unwrap().carrier, 1);
    // The thief stands still: the goose arrives and pecks.
    for _ in 0..HZ * 2 {
        run.tick += 1;
        run.tick_windcountry(&BTreeMap::new());
    }
    assert_eq!(run.heist.as_ref().unwrap().carrier, 0, "taken back");
    assert!(run.players[&1].invulnerable > 0, "pecked");
    assert_eq!(stage(&run.home.countryside, &run.marks), 1);
    assert!(run.valid_snapshot());
}

#[test]
fn a_friends_honk_draws_it_off_the_thief() {
    let mut run = on_the_green();
    put(&mut run, 1, (CLOTH.0 * TILE_UNITS, CLOTH.1 * TILE_UNITS + 1.0));
    press(&mut run, 1);
    let g = run.heist.as_ref().unwrap().goose;
    put(&mut run, 2, (g.0 - 2.0, g.1 + 2.0));
    press(&mut run, 2);
    assert_eq!(run.heist.as_ref().unwrap().honked.map(|h| h.0), Some(2));
    // It turns on the honker, not the thief.
    for _ in 0..HZ {
        run.tick += 1;
        run.tick_windcountry(&BTreeMap::new());
    }
    assert_eq!(run.heist.as_ref().unwrap().carrier, 1, "the thief keeps it");
    assert!(run.found.as_ref().unwrap().2.contains("PECKED") || run.heist.as_ref().unwrap().honked.is_some());
}

#[test]
fn off_the_green_with_the_sail_wins_it_and_the_crown() {
    let mut run = on_the_green();
    put(&mut run, 2, (CLOTH.0 * TILE_UNITS, CLOTH.1 * TILE_UNITS + 1.0));
    press(&mut run, 2);
    let friend = run.wire_copy();
    assert_eq!(friend.heist, run.heist);
    run.arrive(world::room_of(&run.dungeon, RoomKind::Kitewright).unwrap(), (12.0, 7.0));
    run.tick_windcountry(&BTreeMap::new());
    assert_eq!(stage(&run.home.countryside, &run.marks), 2);
    assert!(run.heist.is_none());
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    assert!(run.found.as_ref().unwrap().2.contains("TWO GOT THE SAIL AWAY"));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_heist_shots -- --ignored`
#[test]
#[ignore]
fn write_heist_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("heist");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = on_the_green();
    put(&mut run, 1, (CLOTH.0 * TILE_UNITS - 6.0, CLOTH.1 * TILE_UNITS + 4.0));
    run.step(&BTreeMap::new());
    save(&run, "a-guarded");
    put(&mut run, 1, (CLOTH.0 * TILE_UNITS, CLOTH.1 * TILE_UNITS + 1.0));
    press(&mut run, 1);
    let west = BTreeMap::from([(1, Input { move_x: -1, move_y: 1, ..Default::default() })]);
    run.step(&west);
    save(&run, "b-startled");
    for _ in 0..HZ * 5 / 4 {
        run.step(&west);
    }
    save(&run, "c-chased");
}
