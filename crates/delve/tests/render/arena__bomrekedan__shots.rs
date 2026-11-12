//! Bomrekedan drawn flat: every level paints, through the real seat scene,
//! and its kept drawing is not redrawn every torch step.
use super::*;
use crate::rules::world::{self, bomrekedan};
use crate::rules::{RoomKind, Run};

fn scene(r: &Run) -> Img {
    super::super::seat_scene(
        r,
        super::super::NATIVE_W,
        super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H,
        Some(1),
    )
    .compose()
}

fn level_run(kind: RoomKind, at: (f32, f32)) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
    r.step(&Default::default());
    r
}

/// Where a party arrives on each level: by a stair down to it, or on the
/// top level from the kingdom.
fn arrival(kind: RoomKind) -> (f32, f32) {
    bomrekedan::DOORS
        .iter()
        .find_map(|e| match e.to {
            world::Dest::Room(k, at) if k == kind => Some(at),
            _ => None,
        })
        .unwrap_or(bomrekedan::ARRIVAL)
}

#[test]
fn every_level_draws_and_keeps_its_drawing() {
    for def in &bomrekedan::LEVELS {
        let r = level_run(def.kind, arrival(def.kind));
        assert_eq!(super::super::scenery::torch_step(&r), 0, "{:?} is drawn once", def.kind);
        let im = scenery(&r);
        assert_eq!((im.w, im.h), (r.room().cols as i32 * T, r.room().rows as i32 * T));
        let lit = (0..im.h).step_by(7).flat_map(|y| (0..im.w).step_by(7).map(move |x| (x, y)));
        assert!(lit.filter(|&(x, y)| im.get(x, y).is_some_and(|c| c != [0, 0, 0])).count() > 1000, "{:?} draws", def.kind);
        let shot = scene(&r);
        assert!((0..shot.h).any(|y| (0..shot.w).any(|x| shot.get(x, y).is_some_and(|c| c != [0, 0, 0]))), "{:?} shows", def.kind);
    }
}

/// Every DRESS key the import writes has art of its own, and every tile
/// that holds something is drawn otherwise than its bare ground.
#[test]
fn every_dressed_tile_is_drawn_with_its_piece() {
    for def in &bomrekedan::LEVELS {
        let r = level_run(def.kind, arrival(def.kind));
        let level = bomrekedan::level(def.kind).unwrap();
        let im = scenery(&r);
        // Each tile's bare ground, drawn alone over a canvas of its own
        // (`ground` paints every pixel of its tile).
        let mut cv = Img::black(im.w, im.h);
        let mut keys = std::collections::BTreeMap::<u8, (u32, u32)>::new();
        for row in 0..r.room().rows as i32 {
            for col in 0..r.room().cols as i32 {
                let ch = level.dress_at(col, row);
                if ch == b' ' || !matches!(r.room().tile(col, row), Tile::Floor | Tile::Block) {
                    continue;
                }
                assert!(dressed(ch), "{}: DRESS key {:?} at {col},{row} has no art", def.slug, ch as char);
                ground(&mut cv, level, col * T, row * T, col, row);
                let (x0, y0) = (col * T, row * T);
                let differs = (0..T).flat_map(|y| (0..T).map(move |x| (x0 + x, y0 + y))).filter(|&(x, y)| im.get(x, y) != cv.get(x, y)).count();
                let e = keys.entry(ch).or_default();
                e.0 += 1;
                e.1 += (differs >= 3) as u32;
            }
        }
        for (&ch, &(n, drawn)) in &keys {
            // A stockpile is goods strewn on some of its tiles, and its
            // corner marks on all; a windmill is one tower over its three by
            // three, its corners left open; anything else stands on every
            // tile.
            let least = match ch {
                b'p' => n,
                b'm' => n / 2,
                _ => n * 9 / 10,
            };
            assert!(drawn >= least, "{}: {:?} drawn on {drawn} of {n} tiles", def.slug, ch as char);
        }
        eprintln!("{}: {}", def.slug, keys.iter().map(|(k, (n, _))| format!("{}={n}", *k as char)).collect::<Vec<_>>().join(" "));
    }
}

/// A level's drawing is baked whole once, so its bake must stay quick: a
/// debug build's budget, the slowest level printed.
#[test]
fn a_level_bakes_well_inside_a_second() {
    let mut slowest = (std::time::Duration::ZERO, "");
    for def in &bomrekedan::LEVELS {
        let r = level_run(def.kind, arrival(def.kind));
        // The kit's shared sprites are built once, outside the timing (and
        // the surface water's banks, which `material` keeps by their bounds:
        // the water's own cost is held down by
        // `the_surfaces_water_is_laid_over_its_bounds_alone`).
        let _ = scenery(&r);
        let t = std::time::Instant::now();
        let im = scenery(&r);
        let took = t.elapsed();
        assert!(im.w > 0);
        eprintln!("{}: baked in {:?}", def.slug, took);
        if took > slowest.0 {
            slowest = (took, def.slug);
        }
    }
    eprintln!("slowest bake: {} in {:?}", slowest.1, slowest.0);
    assert!(slowest.0 < std::time::Duration::from_millis(1500), "{} baked in {:?}", slowest.1, slowest.0);
}

/// The surface's water is laid over its own bounds, not the whole level,
/// and is the same water pixel for pixel.
#[test]
fn the_surfaces_water_is_laid_over_its_bounds_alone() {
    let r = level_run(bomrekedan::TOP, bomrekedan::ARRIVAL);
    let room = r.room();
    let pond = wet_bounds(room).expect("the Gate has water");
    let (w, h) = (room.cols as i32 * T, room.rows as i32 * T);
    assert!(pond.2 * pond.3 * 4 < w * h, "the water's bounds {pond:?} are a fraction of the level");
    let wet = |x: i32, y: i32| room.tile(x.div_euclid(T), y.div_euclid(T)) == Tile::Hazard;
    let (mut whole, mut bounded) = (Img::new(w, h), Img::new(w, h));
    mat::water_field(&mut whole, (0, 0, w, h), &wet, 61, 0);
    mat::water_field(&mut bounded, pond, &wet, 61, 0);
    let differ = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| whole.get(x, y) != bounded.get(x, y)).count();
    assert_eq!(differ, 0, "the bounded water differs on {differ} pixels");
    assert!(wet_bounds(&bomrekedan::room(RoomKind::BomGreatHall, (0, 0))).is_none());
}

/// The fort's people stand in the frame, drawn over the kept level each
/// frame and moving on their rounds; the drawing under them stays the
/// same.
#[test]
fn the_people_are_drawn_each_frame_over_the_kept_level() {
    let kind = RoomKind::BomGreatHall;
    let here = bomrekedan::people::on(kind);
    assert!(!here.is_empty());
    let mut r = level_run(kind, arrival(kind));
    let figures_at = |r: &Run| {
        let mut cv = Img::new(r.room().cols as i32 * T, r.room().rows as i32 * T);
        figures(&mut cv, r);
        (0..cv.h).flat_map(|y| (0..cv.w).map(move |x| (x, y))).filter(|&(x, y)| cv.get(x, y).is_some()).count()
    };
    let drawn = figures_at(&r);
    assert!(drawn > here.len() * 40, "{} people, {drawn} pixels", here.len());
    // Someone of the hall walks within a few seconds.
    let start: Vec<_> = here.iter().map(|p| p.at(r.tick)).collect();
    for _ in 0..crate::rules::HZ * 12 {
        r.tick += 1;
    }
    let moved = here.iter().zip(&start).filter(|(p, s)| p.at(r.tick) != **s).count();
    assert!(moved > 0, "nobody moved");
    assert!(figures_at(&r) > 0);
    // Their moving is no reason to bake the level again.
    assert_eq!(super::super::scenery::torch_step(&r), 0, "the level is not redrawn for them");
}

/// Opt-in PNGs for review: each level whole, at one pixel a pixel.
#[test]
#[ignore = "exports Bomrekedan's levels for visual review"]
fn bomrekedan_export_pngs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/bomrekedan-shots");
    std::fs::create_dir_all(&dir).unwrap();
    for def in &bomrekedan::LEVELS {
        let r = level_run(def.kind, arrival(def.kind));
        let mut im = scenery(&r);
        let mut cv = Img::new(im.w, im.h);
        figures(&mut cv, &r);
        im.stamp(&cv, 0, 0);
        image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_scaled(1))
            .unwrap()
            .save(dir.join(format!("{}.png", def.slug)))
            .unwrap();
    }
    for slug in ["the-gate", "the-workshops", "the-forges", "the-great-hall", "the-temples"] {
        let def = bomrekedan::LEVELS.iter().find(|d| d.slug == slug).unwrap();
        let r = level_run(def.kind, arrival(def.kind));
        let shot = scene(&r);
        image::RgbaImage::from_raw(shot.w as u32, shot.h as u32, shot.rgba_scaled(1))
            .unwrap()
            .save(dir.join(format!("seat-{slug}.png")))
            .unwrap();
    }
}

#[test]
fn an_underground_workshop_loses_its_roof_and_lamps_pool() {
    let roofed = super::kit::workshop(super::Trade::Smith, true);
    let mut open = roofed.clone();
    super::open_the_roof(&mut open, true);
    let changed = (0..15.min(open.h)).flat_map(|y| (0..open.w).map(move |x| (x, y))).filter(|&(x, y)| roofed.get(x, y) != open.get(x, y)).count();
    assert!(changed > 40, "the roof still covers the shop ({changed} pixels)");

    let level = bomrekedan::level(RoomKind::BomWorkshops).unwrap();
    let stair = bomrekedan::DOORS.iter().find(|e| e.room == RoomKind::BomWorkshops).unwrap().at;
    let near = super::shade_at(level, stair.0 + 1, stair.1 + 1);
    let dim = (0..level.size.0 as i32)
        .step_by(3)
        .flat_map(|c| (0..level.size.1 as i32).step_by(3).map(move |r| (c, r)))
        .map(|(c, r)| super::shade_at(level, c, r))
        .max()
        .unwrap();
    assert!(near <= 2, "a stair is a lamp ({near})");
    assert!(dim >= 6, "a hall has a dim reach ({dim})");
    let (fc, fr) = (0..level.size.0 as i32)
        .step_by(2)
        .flat_map(|c| (0..level.size.1 as i32).step_by(2).map(move |r| (c, r)))
        .find(|&(c, r)| super::shade_at(level, c, r) >= 6)
        .expect("a dim tile");
    let warm = {
        let mut cv = Img::black(T, T);
        ground(&mut cv, level, 0, 0, stair.0, stair.1);
        cv.get(4, 8)
    };
    let cool = {
        let mut cv = Img::black(T, T);
        ground(&mut cv, level, 0, 0, fc, fr);
        cv.get(4, 8)
    };
    assert_ne!(warm, cool, "a lamp warms the flags beside it");
}
