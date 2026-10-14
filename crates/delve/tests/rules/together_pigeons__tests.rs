//! Pigeon post: bonk the pigeon, take its paper, and dodge the poop.
use super::*;
use crate::rules::{home::Home, world};

fn in_the_belfry() -> Run {
    let mut run = Run::at_home(31, 1, None, Home::default(), Spoils::default());
    run.join_seat(2, "Two");
    run.marks.insert(QUEST_MARKS[0].into(), 1);
    run.arrive(world::room_of(&run.dungeon, RoomKind::Belfry).unwrap(), (12.0, 7.0));
    run
}

#[test]
fn the_pigeon_flies_a_loop_over_the_room() {
    let run = in_the_belfry();
    let room = run.room();
    let spots: Vec<(f32, f32)> = (0..LAP as u64).step_by(HZ as usize).map(|t| pigeon_at(room, 0, t)).collect();
    assert!(spots.iter().all(|&(x, y)| x > 0.0 && y > 0.0 && x < room.width() && y < room.height()));
    let xs: Vec<f32> = spots.iter().map(|s| s.0).collect();
    let span = xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
    assert!(span > room.width() / 2.0, "it ranges across the room");
}

#[test]
fn a_real_arrow_bonks_it_and_the_paper_drops() {
    let mut run = in_the_belfry();
    run.kit_out_for_test();
    // Knight one stands below the pigeon's path and looses arrows up at it.
    for _ in 0..LAP * 2 {
        let (x, y) = pigeon_at(run.room(), 0, run.tick + 6);
        let h = &run.players[&1];
        let (dx, dy) = (x - h.x, y - h.y);
        let len = dx.hypot(dy).max(0.01);
        let aim = BTreeMap::from([(
            1,
            Input {
                fire: true,
                // Eight ways, as a stick or the keys aim.
                aim_x: if dx.abs() > 0.4 * len { dx.signum() as i8 } else { 0 },
                aim_y: if dy.abs() > 0.4 * len { dy.signum() as i8 } else { 0 },
                ..Default::default()
            },
        )]);
        run.step(&aim);
        if run.pigeons.as_ref().is_some_and(|p| p.dropped.is_some()) {
            break;
        }
    }
    let p = run.pigeons.as_ref().expect("the chase is on");
    assert!(p.dropped.is_some(), "an arrow found the pigeon");
    assert_eq!(p.bonks.get(&1), Some(&1));
    assert!(run.valid_snapshot());
}

#[test]
fn a_bare_fist_bonks_it_as_it_swoops_past() {
    let mut run = in_the_belfry();
    let (x, y) = pigeon_at(run.room(), 0, run.tick + 1);
    let h = run.players.get_mut(&2).unwrap();
    (h.x, h.y) = (x + 1.0, y);
    h.swing = 3;
    run.tick += 1;
    run.tick_aerie(&BTreeMap::new());
    assert_eq!(run.pigeons.as_ref().unwrap().bonks.get(&2), Some(&1));
    assert!(run.pigeons.as_ref().unwrap().dropped.is_some());
}

#[test]
fn it_poops_on_whoever_stands_beneath() {
    let mut run = in_the_belfry();
    run.tick = u64::from(POOP_EVERY) * 4 - 1;
    let (x, y) = pigeon_at(run.room(), 0, run.tick + 1);
    let h = run.players.get_mut(&2).unwrap();
    (h.x, h.y) = (x, y);
    run.tick += 1;
    run.tick_aerie(&BTreeMap::new());
    assert_eq!(run.pigeons.as_ref().unwrap().poop.get(&2), Some(&1));
    assert!(run.found.as_ref().unwrap().2.contains("SPLAT"));
}

#[test]
fn the_best_bonker_is_crowned_when_the_minutes_go_in() {
    let mut run = in_the_belfry();
    let take = |id| BTreeMap::from([(id, Input { interact: true, ..Default::default() })]);
    for (i, by) in [(0usize, 2u32), (1, 2), (2, 1)] {
        let kind = DOCUMENTS[i].0;
        run.arrive(world::room_of(&run.dungeon, kind).unwrap(), (12.0, 7.0));
        let (x, y) = run.bonk_pigeon_for_test(by).unwrap();
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x, y);
        run.tick_aerie(&take(1));
        assert_ne!(progress(&run.home.countryside, &run.marks).1 & (1 << i), 0);
    }
    run.arrive(world::room_of(&run.dungeon, RoomKind::Aviary).unwrap(), CLERK);
    run.tick_aerie(&take(1));
    assert_eq!(progress(&run.home.countryside, &run.marks).0, 2);
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    assert!(run.found.as_ref().unwrap().2.contains("TWO BONKED BEST"));
    assert!(run.pigeons.is_none());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_pigeon_shots -- --ignored`
#[test]
#[ignore]
fn write_pigeon_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("pigeons");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = in_the_belfry();
    run.arrive(world::room_of(&run.dungeon, RoomKind::Aqueduct).unwrap(), (12.0, 7.0));
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    run.pigeons.get_or_insert_default().poop.insert(2, 3);
    save(&run, "a-flying");
    let (x, y) = run.bonk_pigeon_for_test(1).unwrap();
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (x - 3.0, y + 1.0);
    run.step(&BTreeMap::new());
    save(&run, "b-dropped");
}
