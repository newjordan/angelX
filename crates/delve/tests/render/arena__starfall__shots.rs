//! Starfall at game scale: `ANGEL_ARENA_SHOTS=<dir> cargo test
//! write_starfall_shots -- --ignored`. Frames of a star crossing a cleared
//! room, its banner, its dive and landing, the shard lying and fading, a
//! knight taking it, and a star wish landing, each as a numbered PPM for
//! the strips and the GIFs.
use crate::rules::{Run, RoomKind, TILE_UNITS, phrasebook};
use std::collections::BTreeMap;

fn save(dir: &std::path::Path, run: &Run, name: &str) {
    let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

/// Three knights standing about a cleared fight room.
fn room(seed: u64) -> Run {
    let mut run = Run::new(seed, 1, Some("Matt"));
    run.join_seat(3, "Ada");
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.clear_for_test();
    run.floor_plates.clear();
    for (id, (x, y)) in [(1u32, (10.0f32, 9.0f32)), (2, (13.5, 8.2)), (3, (8.0, 7.2))] {
        if let Some(h) = run.players.get_mut(&id) {
            (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
            h.invulnerable = 0;
        }
    }
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    run
}

fn hold(run: &mut Run) {
    for (id, (x, y)) in [(1u32, (10.0f32, 9.0f32)), (2, (13.5, 8.2)), (3, (8.0, 7.2))] {
        if let Some(h) = run.players.get_mut(&id) {
            (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
        }
    }
}

#[test]
#[ignore]
fn write_starfall_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir).join("starfall");
    std::fs::create_dir_all(&dir).unwrap();
    // The star crossing, its dive and its landing, frame by frame.
    let mut run = room(7);
    run.star_falls_for_test();
    let start = run.tick;
    let fall = dir.join("fall");
    std::fs::create_dir_all(&fall).unwrap();
    for f in 0..200u64 {
        if f % 2 == 0 {
            save(&fall, &run, &format!("{:04}", run.tick - start));
        }
        run.step(&BTreeMap::new());
        hold(&mut run);
    }
    // The shard lying, late in its life, and fading.
    let shard = dir.join("shard");
    std::fs::create_dir_all(&shard).unwrap();
    let mut late = run.clone();
    for _ in 0..crate::rules::starfall::SHARD_LIFE - 200 {
        late.step(&BTreeMap::new());
        hold(&mut late);
    }
    for f in 0..110u64 {
        if f % 3 == 0 {
            save(&shard, &late, &format!("{:04}", f));
        }
        late.step(&BTreeMap::new());
        hold(&mut late);
    }
    // A knight takes it.
    let catch = dir.join("catch");
    std::fs::create_dir_all(&catch).unwrap();
    let land = run.starfall.star.as_ref().unwrap().land;
    let from = (run.players[&2].x, run.players[&2].y);
    for f in 0..90u64 {
        // Knight 2 walks to it, and on.
        let t = (f as f32 / 14.0).min(1.0);
        if let Some(h) = run.players.get_mut(&2) {
            (h.x, h.y) = (from.0 + (land.0 - from.0) * t, from.1 + (land.1 - from.1) * t);
        }
        if f % 2 == 0 {
            save(&catch, &run, &format!("{:04}", f));
        }
        run.step(&BTreeMap::new());
    }
    // A star wish landing on knight 1.
    let wish = dir.join("wish");
    std::fs::create_dir_all(&wish).unwrap();
    let mut w = room(7);
    w.star_falls_for_test();
    for _ in 0..30 {
        w.step(&BTreeMap::new());
        hold(&mut w);
    }
    save(&wish, &w, "banner-held");
    let volley = phrasebook::get("volley").unwrap();
    w.offer_wish(1, &volley.name, Some(&volley)).unwrap();
    for f in 0..24u64 {
        w.step(&BTreeMap::new());
        hold(&mut w);
        if f % 4 == 3 {
            // The scroll as the seat's client paints it over the room.
            let mut img = crate::render::overworld::arena::frame_for(&w, 768, 448, Some(1));
            crate::render::genie::paint(&mut img, &w, Some(1));
            let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
            out.extend(img.rgb_bytes());
            std::fs::write(wish.join(format!("scroll-{f:02}.ppm")), out).unwrap();
        }
    }
    w.choose(1, 0).unwrap();
    for f in 0..80u64 {
        if f % 2 == 0 {
            save(&wish, &w, &format!("{:04}", f));
        }
        w.step(&BTreeMap::new());
        hold(&mut w);
    }
}
