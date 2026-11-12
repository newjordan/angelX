//! The upper world on screen (`crate::world::plan`): the tiny castle, a
//! camp, the empty hillside, the realm's whole map, and a knight walking.
//! `ANGEL_UPPER_SHOTS=<dir> ANGEL_REALM_PLAN=tabula cargo test -p angel-delve
//! --lib write_upper_world_shots -- --ignored` (and `=full` for the old realm's
//! overview to set beside it).

use super::*;
use crate::rules::{Input, Run};
use std::collections::BTreeMap;

fn dir() -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_UPPER_SHOTS")?);
    std::fs::create_dir_all(&dir).unwrap();
    Some(dir)
}

/// A picture at its own size and nearest-neighbour scaled up.
fn save(dir: &std::path::Path, name: &str, img: &Img, scale: u32) {
    image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
        .unwrap()
        .save(dir.join(format!("{name}-x1.png")))
        .unwrap();
    image::RgbaImage::from_raw(img.w as u32 * scale, img.h as u32 * scale, img.rgba_scaled(scale))
        .unwrap()
        .save(dir.join(format!("{name}-x{scale}.png")))
        .unwrap();
}

/// A lone knight standing at realm tile `(x, y)`, a moment settled, the camera
/// `up` units (half tiles) above where it would sit (the dead zone lets it rest
/// there), so what stands behind the knight is in the frame.
fn at_framed(tile: (f32, f32), up: f32) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some((tile.0 as i32, tile.1 as i32)));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((tile.0 + 0.5) * 2.0, (tile.1 + 0.7) * 2.0);
    hero.camera = Some((hero.x, hero.y - 1.0 - up));
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    run
}

fn at(tile: (f32, f32)) -> Run {
    at_framed(tile, 0.0)
}

/// Walk knight 1 toward each waypoint (realm tiles) in turn, a frame every
/// `every` ticks into `<dir>/<name>/NNNN.png` at native size.
fn walk(dir: &std::path::Path, name: &str, mut run: Run, waypoints: &[(f32, f32)], every: usize) {
    let frames = dir.join(name);
    std::fs::create_dir_all(&frames).unwrap();
    let mut n = 0;
    let mut tick = 0usize;
    let shoot = |run: &Run, n: &mut usize| {
        let img = frame_for(run, 384, 224, Some(1));
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(frames.join(format!("{:04}.png", *n)))
            .unwrap();
        *n += 1;
    };
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    shoot(&run, &mut n);
    for &(wx, wy) in waypoints {
        let (gx, gy) = ((wx + 0.5) * 2.0, (wy + 0.7) * 2.0);
        for _ in 0..4000 {
            let hero = &run.players[&1];
            let (dx, dy) = (gx - hero.x, gy - hero.y);
            if dx.hypot(dy) < 0.5 {
                break;
            }
            let sign = |d: f32| if d > 0.35 { 1 } else if d < -0.35 { -1 } else { 0 };
            let input = Input { move_x: sign(dx), move_y: sign(dy), ..Default::default() };
            run.step(&BTreeMap::from([(1, input)]));
            tick += 1;
            if tick % every == 0 {
                shoot(&run, &mut n);
            }
        }
    }
    for _ in 0..24 {
        run.step(&BTreeMap::new());
        tick += 1;
        if tick % every == 0 {
            shoot(&run, &mut n);
        }
    }
    assert!(n > 20, "a walk of {n} frames");
}

/// The whole map, averaged down by `k`, as one picture.
fn overview(k: i32) -> Img {
    let atlas = crate::render::overworld::kingdom::atlas(&[]);
    let (w, h) = (atlas.w / k, atlas.h / k);
    // The realm is drawn on black paper: lifted so the eye finds it.
    let lift = |v: u32| (v * 7 / 2).min(255) as u8;
    let mut out = Img::black(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 3];
            for sy in 0..k {
                for sx in 0..k {
                    let c = atlas.get(x * k + sx, y * k + sy).unwrap_or([0, 0, 0]);
                    for i in 0..3 {
                        sum[i] += u32::from(c[i]);
                    }
                }
            }
            let n = (k * k) as u32;
            out.set(x, y, [lift(sum[0] / n), lift(sum[1] / n), lift(sum[2] / n)]);
        }
    }
    out
}

#[test]
#[ignore]
fn write_upper_world_shots() {
    let Some(dir) = dir() else {
        return;
    };
    let plan = if crate::world::plan::tabula() { "tabula" } else { "full" };
    let map = overview(4);
    image::RgbaImage::from_raw(map.w as u32, map.h as u32, map.rgba_bytes())
        .unwrap()
        .save(dir.join(format!("overview-{plan}.png")))
        .unwrap();
    if !crate::world::plan::tabula() {
        return;
    }
    use crate::world::plan::tabula as t;
    // The tiny castle, whole: the knight at the spawn before it.
    let run = at_framed((t::KEEP_STAND.0 as f32, t::KEEP_STAND.1 as f32 + 1.0), 4.0);
    save(&dir, "keep-begun", &frame_for(&run, 384, 224, Some(1)), 3);
    // A camp, house 0's (Sol, the north), and the knight at its fire.
    let c0 = t::camp_stand(t::CAMPS[0]);
    let run = at_framed((c0.0 as f32 + 5.0, c0.1 as f32 + 2.0), 4.0);
    save(&dir, "camp-sol", &frame_for(&run, 384, 224, Some(1)), 3);
    // Another, in the south east.
    let c6 = t::camp_stand(t::CAMPS[6]);
    let run = at_framed((c6.0 as f32 + 5.0, c6.1 as f32 + 2.0), 4.0);
    save(&dir, "camp-6", &frame_for(&run, 384, 224, Some(1)), 3);
    // Every camp, one frame each (a contact sheet is cut from them afterwards).
    std::fs::create_dir_all(dir.join("camps")).unwrap();
    for (i, camp) in t::CAMPS.iter().enumerate() {
        let stand = t::camp_stand(*camp);
        let run = at_framed((stand.0 as f32 + 5.0, stand.1 as f32 + 2.0), 4.0);
        let img = frame_for(&run, 384, 224, Some(1));
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join("camps").join(format!("{i:02}.png")))
            .unwrap();
    }
    // The empty hillside between the Keep and a camp, wide (twice the view).
    let run = at((100.0, 76.0));
    save(&dir, "hillside-wide", &frame_for(&run, 768, 448, Some(1)), 2);
    save(&dir, "hillside", &frame_for(&run, 384, 224, Some(1)), 3);
    // The Delve's door.
    let (dx, dy) = t::delve_stand();
    let run = at((dx as f32, dy as f32 + 1.5));
    save(&dir, "delve-door", &frame_for(&run, 384, 224, Some(1)), 3);
    // The walk: spawn, past the castle's west flank, out to the north camp.
    let spawn = at((t::KEEP_STAND.0 as f32, t::KEEP_STAND.1 as f32 + 1.0));
    walk(
        &dir,
        "walk-camp",
        spawn,
        &[(122.0, 89.0), (121.0, 78.0), (121.0, 66.0), (129.0, 62.0), (c0.0 as f32, c0.1 as f32 + 1.0)],
        3,
    );
    // And the walk to the Delve door.
    let spawn = at((t::KEEP_STAND.0 as f32, t::KEEP_STAND.1 as f32 + 1.0));
    walk(&dir, "walk-delve", spawn, &[(138.0, 90.0), (dx as f32, dy as f32 + 0.5)], 3);
    world3d_proof(&dir);
}

/// The realm's 3D sets for the Keep's chunk and a camp's, parsed as a page
/// parses them, written as text, and drawn from above (a plan view, each
/// shape's footprint painted low to high, lighter the higher): no GPU, just
/// what the sets hold and where.
fn world3d_proof(dir: &std::path::Path) {
    use crate::world3d::{kingdom as k3, realm};
    use crate::world::plan::tabula as t;
    let chunk_of = |(tx, ty): (i32, i32)| k3::index(tx.div_euclid(24), ty.div_euclid(16)).unwrap();
    for (name, tile) in [("keep", t::KEEP_STAND), ("camp-sol", t::camp_stand(t::CAMPS[0])), ("camp-6", t::camp_stand(t::CAMPS[6]))] {
        let index = chunk_of(tile);
        let text = realm::set_text(index);
        std::fs::write(dir.join(format!("world3d-{name}.set")), text.as_bytes()).unwrap();
        let chunk = k3::Chunk::new(index, text.clone()).expect("the chunk's set parses");
        let (cx, cz) = k3::cell(index);
        let scale = 4.0;
        let (w, h) = ((24.0 * 16.0 * scale / 2.0) as i32, (16.0 * 16.0 * scale / 2.0) as i32);
        let mut im = Img::black(w, h);
        let mut shapes: Vec<_> = chunk.set.shapes.iter().collect();
        shapes.sort_by(|a, b| (a.at[1] + a.size[1]).total_cmp(&(b.at[1] + b.size[1])));
        let (ox, oz) = (cx as f32 * 24.0 * 16.0, cz as f32 * 16.0 * 16.0);
        for s in shapes {
            let tone = match s.kind.name() {
                "stone" | "rock" | "slate" | "plaster" => [150.0, 165.0, 175.0],
                "wood" | "trunk" | "thatch" | "roof" => [150.0, 105.0, 60.0],
                "cloth" => [190.0, 180.0, 130.0],
                "fire" | "window" => [250.0, 170.0, 60.0],
                "door" | "metal" => [20.0, 20.0, 24.0],
                "foliage" => [60.0, 110.0, 60.0],
                _ => [90.0, 110.0, 90.0],
            };
            let lift = 0.5 + 0.5 * ((s.at[1] + s.size[1]) / 90.0).clamp(0.0, 1.0);
            let colour = [(tone[0] * lift) as u8, (tone[1] * lift) as u8, (tone[2] * lift) as u8];
            let (sn, cs) = s.turn.to_radians().sin_cos();
            let (hx, hz) = (s.size[0] / 2.0, s.size[2] / 2.0);
            let reach = hx.hypot(hz);
            let to_px = |x: f32, z: f32| (((x - ox) * scale / 2.0) as i32, ((z - oz) * scale / 2.0) as i32);
            let (x0, z0) = to_px(s.at[0] - reach, s.at[2] - reach);
            let (x1, z1) = to_px(s.at[0] + reach, s.at[2] + reach);
            for py in z0.max(0)..=z1.min(h - 1) {
                for px in x0.max(0)..=x1.min(w - 1) {
                    let (wx, wz) = (ox + px as f32 * 2.0 / scale - s.at[0], oz + py as f32 * 2.0 / scale - s.at[2]);
                    let (lx, lz) = (wx * cs - wz * sn, wx * sn + wz * cs);
                    if lx.abs() <= hx && lz.abs() <= hz {
                        im.set(px, py, colour);
                    }
                }
            }
        }
        image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("world3d-{name}-plan.png")))
            .unwrap();
    }
}
