//! The Powder Room: crates, kegs and a cross of fire.
use super::*;

fn third_floor() -> Run {
    let mut run = (0..60u64)
        .map(|seed| {
            let mut r = Run::new(seed, 1, None);
            r.descend_for_test();
            r.descend_for_test();
            r
        })
        .find(|r| r.dungeon.depth % 3 == 0)
        .expect("a third floor");
    run.join_seat(2, "Two");
    run.enemies.clear();
    run
}

fn bomb(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { bomb: true, ..Default::default() })])
}

#[test]
fn the_third_floors_folly_is_the_powder_room() {
    let mut run = third_floor();
    run.pick_folly();
    let index = run.folly_room.expect("a hall");
    run.enter_for_test(index);
    let p = run.powder.as_ref().expect("the powder room");
    assert!(p.crates.len() > 20);
    assert_eq!(p.corners.len(), 2, "a corner each");
    assert!(run.barred() && run.valid_snapshot());
}

#[test]
fn a_keg_breaks_the_crates_in_its_cross_and_soots_a_knight_in_it() {
    let mut run = third_floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    run.powder.as_mut().unwrap().t = folly::TITLE;
    // Knight one by a crate: drop a keg, step away, wait for it.
    let cols = run.room().cols as i32;
    let (cx, cy) = {
        let p = run.powder.as_ref().unwrap();
        let i = *p.crates.iter().next().unwrap();
        (i as i32 % cols, i as i32 / cols)
    };
    let beside = [(cx - 1, cy), (cx + 1, cy), (cx, cy - 1), (cx, cy + 1)]
        .into_iter()
        .find(|&(x, y)| run.room().tile(x, y) == Tile::Floor)
        .unwrap_or((cx - 1, cy));
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = ((beside.0 as f32 + 0.5) * TILE_UNITS, (beside.1 as f32 + 0.5) * TILE_UNITS);
        h.invulnerable = 0;
    }
    // Make the tile walkable for the test, in case the crate's neighbour was a crate.
    let at = run.at;
    run.dungeon.rooms[at].set(beside.0 as usize, beside.1 as usize, Tile::Floor);
    run.step(&bomb(1));
    assert_eq!(run.powder.as_ref().unwrap().kegs.len(), 1);
    for _ in 0..powder::FUSE + 2 {
        run.step(&BTreeMap::new());
    }
    let p = run.powder.as_ref().unwrap();
    assert!(p.broken[&1] >= 1 || run.players[&1].invulnerable > 0, "a crate broken, or the knight sooted");
    assert!(run.valid_snapshot());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_powder_shots -- --ignored`
#[test]
#[ignore]
fn write_powder_shots() {
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
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    run.step(&BTreeMap::new());
    save(&run, "p1-title");
    run.powder.as_mut().unwrap().t = folly::TITLE;
    run.step(&bomb(1));
    run.step(&BTreeMap::new());
    run.step(&bomb(2));
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    save(&run, "p2-kegs");
    for _ in 0..powder::FUSE - 28 {
        run.step(&BTreeMap::new());
    }
    save(&run, "p3-blast");
}
