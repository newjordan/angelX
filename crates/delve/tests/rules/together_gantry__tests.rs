//! The Keg Gantry: everyone climbs, kegs roll down, the summit wins.
use super::*;

fn third_floor() -> Run {
    let mut run = (0..60u64)
        .map(|seed| Run::new(seed, 1, None))
        .find(|r| r.dungeon.rooms.iter().any(|room| room.kind == RoomKind::Ledge && room.cols == 48))
        .expect("a floor with a side-on hall");
    run.join_seat(2, "Two");
    run.enemies.clear();
    run
}

#[test]
fn the_side_on_hall_is_the_gantry() {
    let mut run = third_floor();
    run.pick_gantry();
    let index = run.gantry_room.expect("the gantry hall");
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 30);
    }
    run.enter_for_test(index);
    let g = run.gantry.as_ref().expect("the climb begins");
    assert_eq!(g.ante.values().sum::<u32>(), 6);
    assert!(run.side_on() && run.barred() && run.enemies.is_empty());
    // Kegs come down.
    for _ in 0..folly::TITLE + gantry::KEG_EVERY * 3 {
        run.step(&BTreeMap::new());
        for h in run.players.values_mut() {
            h.invulnerable = 100;
        }
    }
    assert!(!run.gantry.as_ref().unwrap().kegs.is_empty(), "the ogre rolls kegs");
    assert!(run.valid_snapshot());
}

#[test]
fn the_first_to_the_top_girder_takes_the_pot_and_the_crown() {
    let mut run = third_floor();
    run.pick_gantry();
    let index = run.gantry_room.unwrap();
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 30);
    }
    run.enter_for_test(index);
    run.gantry.as_mut().unwrap().t = folly::TITLE;
    // Knight two stands on the top girder.
    {
        let h = run.players.get_mut(&2).unwrap();
        h.x = 20.5 * TILE_UNITS;
        // Standing on the top girder: feet on its upper face.
        h.y = gantry::TOP_ROW as f32 * TILE_UNITS - ledge::FOOT;
        h.grounded = true;
    }
    for _ in 0..10 {
        run.step(&BTreeMap::new());
        if run.gantry.as_ref().unwrap().done {
            break;
        }
    }
    let g = run.gantry.as_ref().unwrap();
    assert!(g.done, "the summit ends the climb");
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    assert!(run.players[&2].carried.get(Spoil::Gold) > run.players[&1].carried.get(Spoil::Gold));
    assert!(run.found.as_ref().unwrap().2.contains("TOPPED THE GANTRY"));
    assert!(!run.barred());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_gantry_shots -- --ignored`
#[test]
#[ignore]
fn write_gantry_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("folly");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = third_floor();
    run.pick_gantry();
    run.enter_for_test(run.gantry_room.unwrap());
    run.step(&BTreeMap::new());
    save(&run, "g1-title");
    for _ in 0..folly::TITLE + gantry::KEG_EVERY * 5 {
        run.step(&BTreeMap::new());
        for h in run.players.values_mut() {
            h.invulnerable = 100;
        }
    }
    save(&run, "g2-kegs");
}

#[test]
fn a_knight_can_climb_the_scaffold_by_jumping() {
    let mut run = third_floor();
    run.players.retain(|&id, _| id == 1);
    run.pick_gantry();
    run.enter_for_test(run.gantry_room.unwrap());
    run.gantry.as_mut().unwrap().t = folly::TITLE;
    run.gantry.as_mut().unwrap().kegs.clear();
    let mut best = 0;
    for t in 0..HZ * 20 {
        // Hop: up pressed for a few ticks, then let go, a little eastward.
        let up = t % 20 < 6 || (t % 20 >= 10 && t % 20 < 14);
        let input = Input { move_y: if up { -1 } else { 0 }, move_x: 1, ..Default::default() };
        run.step(&BTreeMap::from([(1, input)]));
        run.players.get_mut(&1).unwrap().invulnerable = 100;
        if let Some(g) = run.gantry.as_mut() {
            g.kegs.clear();
            best = best.max(g.height.get(&1).copied().unwrap_or(0));
            if g.done {
                break;
            }
        }
    }
    assert_eq!(best, 3, "jumping climbs to the summit");
}
