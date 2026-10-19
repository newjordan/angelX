//! Open floors on screen: a seat's view of the map, and the whole floor.

use super::*;
use crate::rules::{DIRS, Input};
use std::collections::BTreeMap;

fn party(seed: u64, depth: u32) -> Run {
    let mut run = Run::new(seed, 7, None);
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bors");
    run.open_floor_for_test(depth);
    run
}

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H)
}

/// The whole floor, a pixel a tile at `k`: floor, wall, block, hazard,
/// door, stairs; chambers not yet seen under fog when `fog`.
pub(crate) fn overview(run: &Run, fog: bool) -> Img {
    let open = run.dungeon.open.as_ref().unwrap();
    let map = &run.dungeon.rooms[open.map];
    let mut img = Img::black(map.cols as i32, map.rows as i32);
    for r in 0..map.rows as i32 {
        for c in 0..map.cols as i32 {
            let (x, y) = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
            let seen = !fog || open.chamber_at(x, y).is_some_and(|i| open.chambers[i].seen);
            let ink: Rgb = match map.tile(c, r) {
                Tile::Floor | Tile::Ledge => [120, 110, 96],
                Tile::Wall => {
                    let faces = DIRS.iter().any(|(dc, dr)| !matches!(map.tile(c + dc, r + dr), Tile::Wall));
                    if faces { [60, 56, 70] } else { [8, 8, 12] }
                }
                Tile::Block => [80, 70, 60],
                Tile::Hazard => [40, 70, 110],
                Tile::Bog => [52, 70, 40],
                Tile::Door => [200, 150, 60],
                Tile::Stairs => [230, 220, 120],
            };
            let ink = if seen { ink } else { [ink[0] / 5, ink[1] / 5, ink[2] / 5] };
            img.set(c, r, ink);
        }
    }
    for (i, ch) in open.chambers.iter().enumerate() {
        let tint: Rgb = if ch.sealed { [220, 60, 50] } else if i == usize::from(open.guardian) { [200, 120, 220] } else if ch.cleared { [90, 200, 120] } else { [150, 150, 170] };
        if fog && !ch.seen {
            continue;
        }
        let (x, y, w, h) = (i32::from(ch.rect.0), i32::from(ch.rect.1), i32::from(ch.rect.2), i32::from(ch.rect.3));
        for c in x..x + w {
            img.set(c, y, tint);
            img.set(c, y + h - 1, tint);
        }
        for r in y..y + h {
            img.set(x, r, tint);
            img.set(x + w - 1, r, tint);
        }
    }
    for e in run.enemies.iter().filter(|e| e.hp > 0) {
        let (c, r) = ((e.x / TILE_UNITS) as i32, (e.y / TILE_UNITS) as i32);
        if !fog || open.chamber_at(e.x, e.y).is_some_and(|i| open.chambers[i].seen) {
            img.set(c, r, if e.dormant { [140, 40, 40] } else { [255, 60, 60] });
        }
    }
    for h in run.players.values().filter(|h| !h.absent) {
        let (c, r) = ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32);
        for (dc, dr) in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)] {
            img.set(c + dc, r + dr, [120, 220, 255]);
        }
    }
    img
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_open_floor_shots -- --ignored`:
/// a party on an open floor: the arrival, a passage, a chamber's fight,
/// the guardian's sealed chamber, the floor under its fog, and the whole
/// floor's tiles.
#[test]
#[ignore]
fn write_open_floor_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let depth: u32 = std::env::var("OPEN_SHOT_DEPTH").ok().and_then(|d| d.parse().ok()).unwrap_or(2);
    let mut run = party(17, depth);
    let open = run.dungeon.open.clone().unwrap();
    save("open-floor-whole", &overview(&run, false), 4);
    save("open-arrival", &seat_scene(&run, w, h, Some(1)).compose(), 3);
    // A passage: the middle of the first link's way.
    let (a, b) = open.links[0];
    let (ax, ay) = open.chambers[usize::from(a)].centre();
    let (bx, by) = open.chambers[usize::from(b)].centre();
    let mid = run.open_spots(open.map, ((ax + bx) / 2.0, (ay + by) / 2.0), 3);
    for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = mid[k];
    }
    save("open-corridor", &seat_scene(&run, w, h, Some(1)).compose(), 3);
    // A fight in a chamber.
    let fight = (0..open.chambers.len()).find(|&i| open.chambers[i].kind == RoomKind::Fight && !open.chambers[i].cleared).unwrap();
    let (cx, cy) = open.chambers[fight].centre();
    let spots = run.open_spots(open.map, (cx, cy + 6.0), 3);
    for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = spots[k];
        hero.invulnerable = 100_000;
    }
    let aim = BTreeMap::from([(1, Input { aim_y: -1, fire: true, ..Default::default() }), (2, Input { aim_x: 1, fire: true, ..Default::default() })]);
    for _ in 0..70 {
        run.step(&aim);
    }
    save("open-chamber-fight", &seat_scene(&run, w, h, Some(1)).compose(), 3);
    // The guardian's chamber, shut: a knight a few steps in from a mouth,
    // the barred gate behind them.
    let g = usize::from(open.guardian);
    let gate = open.chambers[g].gates[0];
    let (x0, y0, w0, h0) = open.chambers[g].rect;
    let (gc, gr) = (f32::from(gate.0) + 0.5, f32::from(gate.1) + 0.5);
    let inward = if gate.1 == y0 { (0.0, 4.0) } else if gate.1 == y0 + h0 - 1 { (0.0, -4.0) } else if gate.0 == x0 { (4.0, 0.0) } else { (-4.0, 0.0) };
    let _ = w0;
    let inside = run.open_spots(open.map, ((gc + inward.0) * TILE_UNITS, (gr + inward.1) * TILE_UNITS), 1)[0];
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = inside;
    run.step(&BTreeMap::new());
    assert!(run.dungeon.open.as_ref().unwrap().chambers[g].sealed);
    save("open-guardian-sealed", &seat_scene(&run, w, h, Some(1)).compose(), 3);
    // And the guardian, met.
    for _ in 0..45 {
        run.step(&BTreeMap::new());
    }
    if let Some(boss) = run.enemies.iter().find(|e| e.is_boss() && e.hp > 0) {
        let near = run.open_spots(open.map, (boss.x, boss.y + 9.0), 3);
        for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
            (run.players.get_mut(&id).unwrap().x, run.players.get_mut(&id).unwrap().y) = near[k];
        }
        for _ in 0..20 {
            run.step(&aim);
        }
        save("open-guardian-fight", &seat_scene(&run, w, h, Some(1)).compose(), 3);
    }
    save("open-map-fog", &overview(&run, true), 4);
    for (party, depth) in [(1usize, 3u32), (2, 4), (4, 5), (1, 6)] {
        let mut rng = crate::rules::Rng::new(40 + party as u64 * 7 + u64::from(depth));
        let mut whole = Run::new(3, 3, None);
        whole.dungeon = crate::rules::layout::open_floor(depth, crate::rules::Pack::at(depth, crate::rules::Pack::Cavern), party, &mut rng);
        whole.at = whole.dungeon.open.as_ref().unwrap().map;
        whole.enemies.clear();
        save(&format!("open-floor-whole-party{party}-depth{depth}"), &overview(&whole, false), 4);
    }
}

/// `cargo test --release open_floor_frame_cost -- --ignored --nocapture`:
/// what drawing an open map costs: its scenery baked whole, and a seat's
/// frame once the scenery is kept, beside a party's great hall.
#[test]
#[ignore]
fn open_floor_frame_cost() {
    let time = |f: &mut dyn FnMut()| {
        let start = std::time::Instant::now();
        f();
        start.elapsed().as_secs_f64() * 1000.0
    };
    let (w, h) = seat();
    let mut run = party(17, 2);
    run.join_seat(4, "Ysolde");
    run.open_floor_for_test(5);
    let mut hall = Run::new(5, 40, Some("Friend"));
    hall.join_seat(3, "Wren");
    hall.join_seat(4, "Bors");
    let stairs = hall.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    hall.enter_for_test(stairs);
    for (name, r) in [("open map", &run), ("great hall", &hall)] {
        let (pw, ph) = room_px(r.room());
        let bake = time(&mut || {
            let _ = scenery(r);
        });
        // Kept scenery: the first frame bakes it, the next ones reuse it.
        let _ = seat_scene(r, w, h, Some(1)).compose();
        let n = 30;
        let frame = time(&mut || {
            for _ in 0..n {
                let _ = seat_scene(r, w, h, Some(1)).compose();
            }
        }) / f64::from(n);
        let flat = time(&mut || {
            for _ in 0..n {
                let _ = frame_for(r, NATIVE_W, NATIVE_H, Some(1));
            }
        }) / f64::from(n);
        println!("{name}: {pw}x{ph} px, scenery bake {bake:.1} ms, a seat's frame in parts {frame:.2} ms, as one picture {flat:.2} ms");
    }
}
