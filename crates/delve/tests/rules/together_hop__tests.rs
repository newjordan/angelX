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

#[test]
fn the_hall_is_one_screen_whatever_the_party() {
    let mut run = (0..80u64)
        .find_map(|seed| {
            let mut r = Run::new(seed, 1, None);
            for id in 2..=4 {
                r.join_seat(id, &format!("Knight {id}"));
            }
            for _ in 0..5 {
                r.descend_for_test();
            }
            (r.dungeon.depth % 6 == 0).then_some(r)
        })
        .expect("a sixth floor");
    run.enemies.clear();
    run.pick_folly();
    run.enter_for_test(run.folly_room.expect("a hall"));
    assert!(run.hop.is_some());
    assert_eq!((run.room().cols, run.room().rows), (layout::COLS, layout::ROWS));
    assert!(run.valid_snapshot());
    let xs: Vec<f32> = run.players.values().map(|h| h.x).collect();
    assert!(xs.windows(2).all(|w| w[1] - w[0] >= 1.9 * TILE_UNITS), "the party stands apart: {xs:?}");
}

#[test]
fn the_river_is_the_halls_only_hazard() {
    // Whatever the seed lays in a hall, nothing is laid over the river: the
    // room is armed after the folly starts, and leaves it alone.
    for seed in 0..40u64 {
        let mut run = Run::new(seed, 1, None);
        run.join_seat(2, "Two");
        for _ in 0..5 {
            run.descend_for_test();
        }
        if run.dungeon.depth % 6 != 0 {
            continue;
        }
        run.enemies.clear();
        run.pick_folly();
        let Some(hall) = run.folly_room else { continue };
        run.enter_for_test(hall);
        assert!(run.hop.is_some());
        assert!(run.traps.is_empty() && run.rocks.is_empty(), "seed {seed}: no spikes or vents under the books");
        // And one laid before it starts is swept off.
        run.traps.push(Trap::Spikes { col: 9, row: 9, phase: 0 });
        let at = run.at;
        run.start_book_hop(at);
        assert!(run.traps.is_empty(), "seed {seed}: swept off");
    }
}

#[test]
fn a_sink_past_the_landing_goes_back_to_the_landing_only() {
    let mut run = into_the_archive();
    run.hop.as_mut().unwrap().t = folly::TITLE;
    let (cols, rows) = (run.room().cols as i32, run.room().rows as i32);
    let width = cols as f32 * TILE_UNITS;
    let row = landing(rows) - 1;
    let l = lane(rows, row).unwrap();
    let gap = (3 * cols as usize..)
        .map(|k| k as f32 * 0.5)
        .find(|&x| x > 3.0 * TILE_UNITS && x < (cols as f32 - 3.0) * TILE_UNITS && !afloat_with(l, 2, width, x, EDGE_GRACE + 0.3))
        .expect("water");
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (gap, (row as f32 + 0.5) * TILE_UNITS);
        h.invulnerable = 0;
    }
    run.tick_book_hop();
    assert_eq!((run.players[&1].y / TILE_UNITS) as i32, landing(rows), "back to the landing, not the bank");
    let show = &run.hop.as_ref().unwrap().show;
    assert_eq!(show.sinks.len(), 1);
    assert_eq!(show.sinks[0].lost, (landing(rows) - row) as u8);
}

#[test]
fn a_toe_over_a_books_edge_is_still_afloat() {
    let (cols, _) = (24, 14);
    let width = cols as f32 * TILE_UNITS;
    let (lefts, len) = books(0, 0, width);
    let b = lefts[1];
    assert!(!afloat(0, 0, width, b + len + 0.2));
    assert!(afloat_with(0, 0, width, b + len + 0.2, EDGE_GRACE));
    assert!(!afloat_with(0, 0, width, b + len + 1.0, EDGE_GRACE));
}

#[test]
fn nobody_leaves_the_bank_under_the_title_card() {
    let mut run = into_the_archive();
    let rows = run.room().rows as i32;
    {
        let h = run.players.get_mut(&1).unwrap();
        h.y = (landing(rows) as f32 + 0.5) * TILE_UNITS;
    }
    run.tick_book_hop();
    assert_eq!((run.players[&1].y / TILE_UNITS) as i32, near_bank(rows));
}

#[test]
fn the_whole_table_crossing_lights_the_archive_and_pays_the_table() {
    let mut run = into_the_archive();
    run.hop.as_mut().unwrap().t = folly::TITLE;
    let rows = run.room().rows as i32;
    for id in [1u32, 2] {
        run.players.get_mut(&id).unwrap().y = (far_bank(rows) as f32 + 0.5) * TILE_UNITS;
    }
    run.tick_book_hop();
    let h = run.hop.as_ref().unwrap();
    assert_eq!(h.show.waves, 1, "both seats crossed in the one tick");
    run.hop.as_mut().unwrap().t = folly::TITLE + HOP_TIME - 1;
    let gold: Vec<u32> = [1u32, 2].iter().map(|id| run.players[id].carried.get(Spoil::Gold)).collect();
    run.tick_book_hop();
    for (k, id) in [1u32, 2].iter().enumerate() {
        assert!(run.players[id].carried.get(Spoil::Gold) >= gold[k] + 1 + hop_show::ARCHIVE_GOLD, "seat {id} is paid the table's gold");
    }
    let rec = &run.home.countryside.games.hop;
    assert_eq!((rec.best, rec.plays, rec.wins), (1, 1, 1));
    assert_eq!(run.hop.as_ref().unwrap().show.ended.unwrap().waves, 1);
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
