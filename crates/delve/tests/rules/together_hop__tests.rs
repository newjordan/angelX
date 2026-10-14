//! Book Hop: lanes of drifting books, the ink, and the far bank.
use super::*;

fn sixth_floor() -> Run {
    let mut run = (0..80u64)
        .find_map(|seed| {
            let mut r = Run::new(seed, 1, None);
            for _ in 0..5 {
                r.descend_for_test();
            }
            (r.dungeon.depth % 6 == 0).then_some(r)
        })
        .expect("a sixth floor");
    run.join_seat(2, "Two");
    run.enemies.clear();
    run
}

fn into_the_archive() -> Run {
    let mut run = sixth_floor();
    run.pick_folly();
    run.enter_for_test(run.folly_room.expect("a hall"));
    run
}

#[test]
fn the_sixth_floors_folly_is_book_hop() {
    let run = into_the_archive();
    let h = run.hop.as_ref().expect("book hop");
    assert_eq!(h.crossings.len(), 2);
    assert!(run.powder.is_none());
    assert!(run.barred() && run.valid_snapshot());
    let rows = run.room().rows as i32;
    for hero in run.players.values() {
        assert_eq!((hero.y / TILE_UNITS) as i32, near_bank(rows), "on the near bank");
    }
}

#[test]
fn a_book_carries_its_rider_and_the_ink_sends_you_back() {
    let mut run = into_the_archive();
    run.hop.as_mut().unwrap().t = folly::TITLE;
    let (cols, rows) = (run.room().cols as i32, run.room().rows as i32);
    let width = cols as f32 * TILE_UNITS;
    let row = landing(rows) + 1;
    let l = lane(rows, row).unwrap();
    let t = 1;
    // Stand on the middle of a book well inside the walls.
    let (lefts, len) = books(l, t, width);
    let bx = lefts
        .iter()
        .map(|&b| b + len / 2.0)
        .find(|&x| x > 4.0 * TILE_UNITS && x < (cols as f32 - 4.0) * TILE_UNITS)
        .expect("a book mid-hall");
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (bx, (row as f32 + 0.5) * TILE_UNITS);
        h.invulnerable = 0;
    }
    run.tick_book_hop();
    let h = &run.players[&1];
    assert!((h.x - bx).abs() > 0.05, "carried along");
    assert_eq!((h.y / TILE_UNITS) as i32, row, "still afloat");
    // Now into the ink between books.
    let gap = lefts.iter().map(|&b| b + len + 1.0).find(|&x| x > 3.0 * TILE_UNITS && x < (cols as f32 - 3.0) * TILE_UNITS && !afloat(l, t + 1, width, x)).unwrap();
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (gap, (row as f32 + 0.5) * TILE_UNITS);
        h.invulnerable = 0;
    }
    run.tick_book_hop();
    assert_eq!((run.players[&1].y / TILE_UNITS) as i32, near_bank(rows), "sank back to the bank");
    assert_eq!(run.hop.as_ref().unwrap().splashes.len(), 1);
}

#[test]
fn the_far_bank_scores_and_the_minute_crowns_the_best() {
    let mut run = into_the_archive();
    run.hop.as_mut().unwrap().t = folly::TITLE;
    let rows = run.room().rows as i32;
    for _ in 0..2 {
        let h = run.players.get_mut(&2).unwrap();
        h.y = (far_bank(rows) as f32 + 0.5) * TILE_UNITS;
        run.tick_book_hop();
    }
    assert_eq!(run.hop.as_ref().unwrap().crossings[&2], 2);
    run.hop.as_mut().unwrap().t = folly::TITLE + HOP_TIME - 1;
    let gold = run.players[&2].carried.get(Spoil::Gold);
    run.tick_book_hop();
    let h = run.hop.as_ref().unwrap();
    assert!(h.done && run.room().cleared);
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    assert!(run.players[&2].carried.get(Spoil::Gold) >= gold + 2);
    assert!(run.found.as_ref().unwrap().2.contains("TWO CROSSED THE MOST"));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_hop_shots -- --ignored`
#[test]
#[ignore]
fn write_hop_shots() {
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
    let mut run = into_the_archive();
    run.step(&BTreeMap::new());
    save(&run, "h1-title");
    run.hop.as_mut().unwrap().t = folly::TITLE;
    let up = BTreeMap::from([
        (1, Input { move_y: -1, ..Default::default() }),
        (2, Input { move_y: -1, move_x: 1, ..Default::default() }),
    ]);
    for _ in 0..HZ / 2 {
        run.step(&up);
    }
    save(&run, "h2-hopping");
}
