//! The ambient map's frames, byte for byte.
//!
//! The overworld's contract is that the same [`Scene`] renders the same bytes. A resting window
//! redraws the map a few times a second as the ambient step moves, and the draw keeps what the steps
//! share (the sprites drawn once, the ground and the light's noise under a view) instead of drawing it
//! again. These frames are the proof it changes nothing: each was hashed from the drawing as it was
//! before anything was kept, over steps in order, views that move, and scenes that light and move
//! things, so a kept layer that went stale (a view that moved, wishes that changed, a lit place
//! that went dark) would show as a different hash.

use super::*;
use super::map::Place;

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Where the realm keeps water, lava and falls: the first tile of each kind in reading order, so the
/// frames below look at ground that moves with the step.
fn first_of(kinds: &[u8]) -> Option<(i32, i32)> {
    let realm = map::Realm::get();
    (0..MAP_H)
        .flat_map(|ty| (0..MAP_W).map(move |tx| (tx, ty)))
        .find(|&(tx, ty)| kinds.contains(&realm.at(tx, ty)))
}

fn centre((tx, ty): (i32, i32)) -> (f32, f32) {
    ((tx * TILE + TILE / 2) as f32, (ty * TILE + TILE / 2) as f32)
}

fn spots() -> Vec<(&'static str, (f32, f32))> {
    let stand = |place: Place| {
        let (tx, ty) = place.stand_world();
        centre((tx, ty))
    };
    let mut spots = vec![
        ("keep", stand(Place::Keep)),
        ("smithy", stand(Place::Smithy)),
        ("lists", stand(Place::Lists)),
        ("colosseum", stand(Place::Colosseum)),
        ("mines", stand(Place::Mines)),
        ("dragon", stand(Place::DragonKeep)),
    ];
    for (name, kinds) in [("water", &b"~"[..]), ("stream", b"w"), ("lava", b"LF")] {
        if let Some(tile) = first_of(kinds) {
            spots.push((name, centre(tile)));
        }
    }
    spots
}

/// One frame's hash: the scene drawn the way the window draws it, at one map pixel to a pixel.
fn hash_of(scene: &Scene, w: i32, h: i32) -> u64 {
    fnv(&frame_sized(scene, w, h).rgba_scaled(1))
}

/// The frames, in the order they are drawn: one scene's steps, the view then moving to the next
/// spot, again and again, as a window that is left running and then walked does.
fn frames() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut lit = Scene::resting();
    lit.tier = 8;
    lit.active = Some(Place::Smithy);
    lit.chapel_lit = true;
    lit.forge_hot = true;
    lit.cottages = [true, false, true];
    lit.weather = scene::Weather::Rain;
    let mut quiet = Scene::resting();
    quiet.tier = 3;
    let mut stormy = Scene::resting();
    stormy.tier = 6;
    stormy.active = Some(Place::Lists);
    stormy.quest = Some((2, 5));
    stormy.weather = scene::Weather::Storm;
    for (label, base) in [("quiet", quiet), ("lit", lit), ("stormy", stormy)] {
        for (name, (x, y)) in spots() {
            let mut scene = base.clone();
            scene.camera = (x, y);
            scene.knight.x = x;
            scene.knight.y = y;
            for tick in [0u32, 1, 2, 3, 4, 5, 11, 12] {
                scene.tick = tick;
                // Two sizes: the window's own, and a wider one a bigger window shows.
                for (w, h) in [(344, 224), (421, 253)] {
                    out.push((format!("{label}/{name}/t{tick}/{w}x{h}"), hash_of(&scene, w, h)));
                }
            }
        }
    }
    // The same scene again after the cached layers were built for other views: first in nothing
    // alike, then coming back to a view already drawn.
    let mut scene = Scene::resting();
    for tick in [7u32, 8, 7] {
        scene.tick = tick;
        out.push((format!("again/t{tick}"), hash_of(&scene, 344, 224)));
    }
    out
}

#[test]
fn the_ambient_frames_are_the_frames_the_map_always_drew() {
    let frames = frames();
    if std::env::var_os("ANGEL_PRINT_AMBIENT_FRAMES").is_some() {
        for (name, hash) in &frames {
            println!("    (\"{name}\", {hash:#018x}),");
        }
    }
    let golden: &[(&str, u64)] = GOLDEN;
    assert_eq!(frames.len(), golden.len(), "the set of frames changed: regenerate the table from the old drawing");
    for ((name, hash), (want_name, want)) in frames.iter().zip(golden) {
        assert_eq!(name, want_name);
        assert_eq!(*hash, *want, "frame {name} no longer draws the way it did");
    }
}

#[test]
fn a_view_drawn_again_after_other_views_is_the_same_frame() {
    // What is kept belongs to its view: draw A, then B, then A again, and each step of A must come
    // out as it did the first time.
    let spots = [(11.0 * TILE as f32, 12.0 * TILE as f32), (30.0 * TILE as f32, 25.0 * TILE as f32)];
    let mut scene = Scene::resting();
    let mut round = |scene: &mut Scene| {
        let mut hashes = Vec::new();
        for spot in spots {
            scene.camera = spot;
            for tick in 0..4 {
                scene.tick = tick;
                hashes.push(hash_of(scene, 344, 224));
            }
        }
        hashes
    };
    let first = round(&mut scene);
    let second = round(&mut scene);
    assert_eq!(first, second);
    // And the other spot's frames are not the first's: the key is the view.
    assert_ne!(first[0], first[4]);
}

#[test]
fn only_the_tiles_that_move_depend_on_the_ambient_step() {
    // The kept ground leaves every tile but the moving ones as it was drawn once: so no other
    // tile's marks may change with the step, whatever the ground is made of there.
    let realm = map::Realm::get();
    for ty in 0..MAP_H {
        for tx in 0..MAP_W {
            if ground::moves(realm.at(tx, ty)) {
                continue;
            }
            for ly in 0..TILE {
                for lx in 0..TILE {
                    let (wx, wy) = (tx * TILE + lx, ty * TILE + ly);
                    let at_rest = ground::mark(realm, wx, wy, 0);
                    for tick in [13u32, 40] {
                        assert_eq!(
                            ground::mark(realm, wx, wy, tick),
                            at_rest,
                            "tile {:?} at ({tx}, {ty}) moves with the step, but `ground::moves` says it does not",
                            realm.at(tx, ty) as char
                        );
                    }
                }
            }
        }
    }
}

/// `light::dusk` as it was written before the view's field was kept: every pixel works out the light on
/// it, the ambient's wander and its steps for itself.
fn dusk_as_it_was(cv: &mut Img, realm: &map::Realm, lights: &[light::Light], (ox, oy): (i32, i32), ambient: f32) {
    use super::ink::{BLACK, bayer, vnoise};
    let (w, h) = (cv.w as f32, cv.h as f32);
    let lights: Vec<light::Light> = lights
        .iter()
        .copied()
        .filter(|li| {
            li.x + li.r > ox as f32 && li.x - li.r < ox as f32 + w && li.y + li.r > oy as f32 && li.y - li.r < oy as f32 + h
        })
        .collect();
    for y in 0..cv.h {
        for x in 0..cv.w {
            let (wx, wy) = (ox + x, oy + y);
            let (px, py) = (wx as f32 + 0.5, wy as f32 + 0.5);
            let dapple = (vnoise(px / 48.0, py / 48.0, 77) - 0.5) * light::DAPPLE;
            let (local, fire) = light::light_at(&lights, px, py);
            let level = (ambient + dapple + local).min(1.0);
            match cv.get(x, y) {
                Some(c) if c != BLACK => {
                    let th = light::seam(wx, wy);
                    let c = if fire > 0.16 + th * 0.24 { light::warm(c) } else { c };
                    let v = ((1.0 - level) * light::STEPS).max(0.0);
                    let t = ((v.fract() - 0.5) * light::SEAM_EDGE + 0.5).clamp(0.0, 1.0);
                    let steps = (v.floor() + if t > th { 1.0 } else { 0.0 }) as usize;
                    cv.set(x, y, light::step_down(c, steps));
                }
                _ => {
                    let inside = wx >= 0 && wy >= 0 && wx < MAP_W * TILE && wy < MAP_H * TILE;
                    let p = (local - 0.22) * 0.55;
                    if inside && p > 0.0 && bayer(wx, wy) < p {
                        cv.put(x, y, ground::pool_ink(realm, wx, wy, fire > local * 0.5));
                    }
                }
            }
        }
    }
}

/// `light::vignette` as it was written before its table was kept.
fn vignette_as_it_was(f: &mut Img, top: i32) {
    use super::ink::bayer;
    let (w, h) = (f.w as f32, (f.h - top) as f32);
    for y in top..f.h {
        for x in 0..f.w {
            let nx = (x as f32 + 0.5 - w / 2.0) / (w / 2.0);
            let ny = (y as f32 - top as f32 + 0.5 - h / 2.0) / (h / 2.0);
            let r = (nx * nx + ny * ny).sqrt();
            let steps = ((r - 0.72).max(0.0) * 3.2 + bayer(x, y) * 0.999).floor() as usize;
            if steps > 0
                && let Some(c) = f.get(x, y)
            {
                f.set(x, y, light::step_down(c, steps));
            }
        }
    }
}

/// A picture of the realm's inks scattered over black paper, the same for the same seed.
fn scattered(w: i32, h: i32, seed: u32) -> Img {
    let inks: Vec<char> = "RoOtTHivVWzYySshgGuUMAaB@5w".chars().collect();
    let mut img = Img::black(w, h);
    for y in 0..h {
        for x in 0..w {
            // Runs of one ink, as sprites and the ground leave them, and black paper between.
            let cell = ink::hash(x / 5, y / 3, seed);
            if cell % 7 < 3 {
                continue;
            }
            img.put(x, y, inks[(ink::hash(x / 5, y / 3, seed + 1) as usize) % inks.len()]);
        }
    }
    img
}

#[test]
fn the_light_over_a_view_is_the_light_it_always_was() {
    let realm = map::Realm::get();
    // Views in the realm and half off it; lights that burn (fire) and lights that do not, in the view,
    // across its edge, and out of reach.
    for (ox, oy, w, h) in [(300, 260, 173, 121), (-40, -30, 90, 70), (MAP_W * TILE - 60, MAP_H * TILE - 50, 90, 80)] {
        let lights = [
            light::Light { x: (ox + 40) as f32, y: (oy + 30) as f32, r: 34.0, s: 0.55, fire: true },
            light::Light { x: (ox + 41) as f32, y: (oy + 32) as f32, r: 20.0, s: 0.3, fire: false },
            light::Light { x: (ox + w - 5) as f32, y: (oy + h / 2) as f32, r: 48.0, s: 0.75, fire: true },
            light::Light { x: (ox - 20) as f32, y: (oy + 10) as f32, r: 30.0, s: 0.36, fire: false },
            light::Light { x: (ox + w + 500) as f32, y: (oy + 500) as f32, r: 30.0, s: 0.5, fire: true },
        ];
        let field = light::DuskField::new(ox, oy, w, h);
        // The same field serves every ambient and every set of lights a window draws over this view.
        for ambient in [light::DUSK, 0.84, 1.0, 0.3, light::DUSK] {
            for lights in [&lights[..], &lights[..2], &lights[..0]] {
                let picture = scattered(w, h, (ox + oy).unsigned_abs());
                let mut was = picture.clone();
                dusk_as_it_was(&mut was, realm, lights, (ox, oy), ambient);
                let mut alone = picture.clone();
                light::dusk(&mut alone, realm, lights, (ox, oy), ambient);
                let mut kept = picture.clone();
                light::dusk_over(&mut kept, realm, lights, (ox, oy), ambient, Some(&field));
                assert!(was == alone, "dusk at {ox},{oy} ambient {ambient} without a kept field");
                assert!(was == kept, "dusk at {ox},{oy} ambient {ambient} with a kept field");
            }
        }
    }
}

#[test]
fn the_rim_of_a_view_is_the_rim_it_always_was() {
    for (w, h, top) in [(344, 224, 0), (421, 253, 0), (96, 80, 12), (7, 5, 0), (1, 1, 0)] {
        let picture = scattered(w, h, (w + h) as u32);
        let mut was = picture.clone();
        vignette_as_it_was(&mut was, top);
        for _ in 0..2 {
            let mut now = picture.clone();
            light::vignette(&mut now, top);
            assert!(was == now, "vignette {w}x{h} from row {top}");
        }
    }
}

/// Hashed from the drawing before the ambient layers were kept (`ANGEL_PRINT_AMBIENT_FRAMES=1` prints the
/// table the current drawing makes).
const GOLDEN: &[(&str, u64)] = &[
    ("quiet/keep/t0/344x224", 0x367268720a0f079a),
    ("quiet/keep/t0/421x253", 0xafb9300a9b1d5e99),
    ("quiet/keep/t1/344x224", 0x7f8fc16290071637),
    ("quiet/keep/t1/421x253", 0xfe9cbdb0bee6fa90),
    ("quiet/keep/t2/344x224", 0xed13f76e21c4a70d),
    ("quiet/keep/t2/421x253", 0x7d5cf412ae59f90d),
    ("quiet/keep/t3/344x224", 0xd509afe239224343),
    ("quiet/keep/t3/421x253", 0xb9f2fd3ff53a8ec3),
    ("quiet/keep/t4/344x224", 0x238799f8b6ca2b01),
    ("quiet/keep/t4/421x253", 0x7ec59978a1c687cd),
    ("quiet/keep/t5/344x224", 0x741acdbbd428672b),
    ("quiet/keep/t5/421x253", 0xec9161d2d027a272),
    ("quiet/keep/t11/344x224", 0xe248f9d0800ff006),
    ("quiet/keep/t11/421x253", 0xee15cea87f7b82ac),
    ("quiet/keep/t12/344x224", 0x49c1b0477603e6e3),
    ("quiet/keep/t12/421x253", 0x6d4512ebc0250771),
    ("quiet/smithy/t0/344x224", 0x83f04a5b5a7fc70a),
    ("quiet/smithy/t0/421x253", 0x709e360fd60edf93),
    ("quiet/smithy/t1/344x224", 0x9da0c81b3af938aa),
    ("quiet/smithy/t1/421x253", 0xee72b9f0832a4714),
    ("quiet/smithy/t2/344x224", 0xf236ce5956911063),
    ("quiet/smithy/t2/421x253", 0x78d60f13ac3dd9c4),
    ("quiet/smithy/t3/344x224", 0xe79b068e71cb7a22),
    ("quiet/smithy/t3/421x253", 0x152e2baf4c79b42d),
    ("quiet/smithy/t4/344x224", 0x53a76acd5b48a60f),
    ("quiet/smithy/t4/421x253", 0xe44f09fc903530c3),
    ("quiet/smithy/t5/344x224", 0xe5073ed3353a6160),
    ("quiet/smithy/t5/421x253", 0xca455fdd7bf006a4),
    ("quiet/smithy/t11/344x224", 0x47dea279fdb37cc4),
    ("quiet/smithy/t11/421x253", 0xa8391a20e9a3ac79),
    ("quiet/smithy/t12/344x224", 0x25617eeca5d33688),
    ("quiet/smithy/t12/421x253", 0xe3cc67512f964f53),
    ("quiet/lists/t0/344x224", 0x474d2aa10e1ebbc2),
    ("quiet/lists/t0/421x253", 0xa45d71e920f482a3),
    ("quiet/lists/t1/344x224", 0x474d2aa10e1ebbc2),
    ("quiet/lists/t1/421x253", 0xa45d71e920f482a3),
    ("quiet/lists/t2/344x224", 0x474d2aa10e1ebbc2),
    ("quiet/lists/t2/421x253", 0xa45d71e920f482a3),
    ("quiet/lists/t3/344x224", 0x474d2aa10e1ebbc2),
    ("quiet/lists/t3/421x253", 0xa45d71e920f482a3),
    ("quiet/lists/t4/344x224", 0x474d2aa10e1ebbc2),
    ("quiet/lists/t4/421x253", 0xa45d71e920f482a3),
    ("quiet/lists/t5/344x224", 0x474d2aa10e1ebbc2),
    ("quiet/lists/t5/421x253", 0xa45d71e920f482a3),
    ("quiet/lists/t11/344x224", 0x474d2aa10e1ebbc2),
    ("quiet/lists/t11/421x253", 0xa45d71e920f482a3),
    ("quiet/lists/t12/344x224", 0x474d2aa10e1ebbc2),
    ("quiet/lists/t12/421x253", 0xa45d71e920f482a3),
    ("quiet/colosseum/t0/344x224", 0xc2bf9788f8fb9305),
    ("quiet/colosseum/t0/421x253", 0x68ae7559ac2e6bb6),
    ("quiet/colosseum/t1/344x224", 0xc2bf9788f8fb9305),
    ("quiet/colosseum/t1/421x253", 0x68ae7559ac2e6bb6),
    ("quiet/colosseum/t2/344x224", 0xc2bf9788f8fb9305),
    ("quiet/colosseum/t2/421x253", 0x68ae7559ac2e6bb6),
    ("quiet/colosseum/t3/344x224", 0xc2bf9788f8fb9305),
    ("quiet/colosseum/t3/421x253", 0x68ae7559ac2e6bb6),
    ("quiet/colosseum/t4/344x224", 0xc2bf9788f8fb9305),
    ("quiet/colosseum/t4/421x253", 0x68ae7559ac2e6bb6),
    ("quiet/colosseum/t5/344x224", 0xc2bf9788f8fb9305),
    ("quiet/colosseum/t5/421x253", 0x68ae7559ac2e6bb6),
    ("quiet/colosseum/t11/344x224", 0xc2bf9788f8fb9305),
    ("quiet/colosseum/t11/421x253", 0x68ae7559ac2e6bb6),
    ("quiet/colosseum/t12/344x224", 0xc2bf9788f8fb9305),
    ("quiet/colosseum/t12/421x253", 0x68ae7559ac2e6bb6),
    ("quiet/mines/t0/344x224", 0x2229a08eb923ec1e),
    ("quiet/mines/t0/421x253", 0x9455927d9b0f9eab),
    ("quiet/mines/t1/344x224", 0x2229a08eb923ec1e),
    ("quiet/mines/t1/421x253", 0x9455927d9b0f9eab),
    ("quiet/mines/t2/344x224", 0x2229a08eb923ec1e),
    ("quiet/mines/t2/421x253", 0x9455927d9b0f9eab),
    ("quiet/mines/t3/344x224", 0x2229a08eb923ec1e),
    ("quiet/mines/t3/421x253", 0x9455927d9b0f9eab),
    ("quiet/mines/t4/344x224", 0x2229a08eb923ec1e),
    ("quiet/mines/t4/421x253", 0x9455927d9b0f9eab),
    ("quiet/mines/t5/344x224", 0x2229a08eb923ec1e),
    ("quiet/mines/t5/421x253", 0x9455927d9b0f9eab),
    ("quiet/mines/t11/344x224", 0x2229a08eb923ec1e),
    ("quiet/mines/t11/421x253", 0x9455927d9b0f9eab),
    ("quiet/mines/t12/344x224", 0x2229a08eb923ec1e),
    ("quiet/mines/t12/421x253", 0x9455927d9b0f9eab),
    ("quiet/dragon/t0/344x224", 0xab24f11091db0ae3),
    ("quiet/dragon/t0/421x253", 0x6f903bdeeee5ba62),
    ("quiet/dragon/t1/344x224", 0xab24f11091db0ae3),
    ("quiet/dragon/t1/421x253", 0x6f903bdeeee5ba62),
    ("quiet/dragon/t2/344x224", 0xab24f11091db0ae3),
    ("quiet/dragon/t2/421x253", 0x6f903bdeeee5ba62),
    ("quiet/dragon/t3/344x224", 0xab24f11091db0ae3),
    ("quiet/dragon/t3/421x253", 0x6f903bdeeee5ba62),
    ("quiet/dragon/t4/344x224", 0xab24f11091db0ae3),
    ("quiet/dragon/t4/421x253", 0x6f903bdeeee5ba62),
    ("quiet/dragon/t5/344x224", 0xab24f11091db0ae3),
    ("quiet/dragon/t5/421x253", 0x6f903bdeeee5ba62),
    ("quiet/dragon/t11/344x224", 0xab24f11091db0ae3),
    ("quiet/dragon/t11/421x253", 0x6f903bdeeee5ba62),
    ("quiet/dragon/t12/344x224", 0xab24f11091db0ae3),
    ("quiet/dragon/t12/421x253", 0x6f903bdeeee5ba62),
    ("quiet/water/t0/344x224", 0x1614d580e8867f82),
    ("quiet/water/t0/421x253", 0xb2ad885158a9dab3),
    ("quiet/water/t1/344x224", 0x1614d580e8867f82),
    ("quiet/water/t1/421x253", 0xb2ad885158a9dab3),
    ("quiet/water/t2/344x224", 0x2cb67ef2660cc8a6),
    ("quiet/water/t2/421x253", 0xfc4afdc15521fcde),
    ("quiet/water/t3/344x224", 0x2cb67ef2660cc8a6),
    ("quiet/water/t3/421x253", 0xfc4afdc15521fcde),
    ("quiet/water/t4/344x224", 0x59b7539d6b30e2aa),
    ("quiet/water/t4/421x253", 0x1ab6b9bfa5a87ed9),
    ("quiet/water/t5/344x224", 0x59b7539d6b30e2aa),
    ("quiet/water/t5/421x253", 0x1ab6b9bfa5a87ed9),
    ("quiet/water/t11/344x224", 0x3e1ead22a23ed7f0),
    ("quiet/water/t11/421x253", 0x7be392f851ebabf2),
    ("quiet/water/t12/344x224", 0x06fcedd12ef0fb3e),
    ("quiet/water/t12/421x253", 0xc9bc926e4d413056),
    ("quiet/stream/t0/344x224", 0xc4a09ce72aac3f7d),
    ("quiet/stream/t0/421x253", 0x5f2bc945b879790d),
    ("quiet/stream/t1/344x224", 0xc0178c2ecfe59f0a),
    ("quiet/stream/t1/421x253", 0xa82af3c1268b112a),
    ("quiet/stream/t2/344x224", 0x5d5c597391c1d152),
    ("quiet/stream/t2/421x253", 0x870cf68a98c7c19c),
    ("quiet/stream/t3/344x224", 0x0f4abd57ec0546b6),
    ("quiet/stream/t3/421x253", 0x86c2bdcae203d860),
    ("quiet/stream/t4/344x224", 0x86bb36ed4e730cf8),
    ("quiet/stream/t4/421x253", 0x1568aebffd9ec974),
    ("quiet/stream/t5/344x224", 0x062f0952e7d1578e),
    ("quiet/stream/t5/421x253", 0x24859e13cb192182),
    ("quiet/stream/t11/344x224", 0x0fbd79fc1bf488d8),
    ("quiet/stream/t11/421x253", 0x6eb7f4452eb635f6),
    ("quiet/stream/t12/344x224", 0x3ce8354130a0bfcf),
    ("quiet/stream/t12/421x253", 0x9dc90bc6e3b84d69),
    ("quiet/lava/t0/344x224", 0xe9d4200e3fdf155d),
    ("quiet/lava/t0/421x253", 0x2031523b88d02dfc),
    ("quiet/lava/t1/344x224", 0x530b1e807f6ded55),
    ("quiet/lava/t1/421x253", 0x7bf60610d314a623),
    ("quiet/lava/t2/344x224", 0x3313ec5756a4b480),
    ("quiet/lava/t2/421x253", 0xe9719a8748993858),
    ("quiet/lava/t3/344x224", 0x320218aae9e0c507),
    ("quiet/lava/t3/421x253", 0x56ba45631444c884),
    ("quiet/lava/t4/344x224", 0x7af6a75d7ef229b7),
    ("quiet/lava/t4/421x253", 0x2df028b67ceb1fac),
    ("quiet/lava/t5/344x224", 0xf0197ac075d06fc3),
    ("quiet/lava/t5/421x253", 0x94258c22cbc4070d),
    ("quiet/lava/t11/344x224", 0xa0e469af8d3927a5),
    ("quiet/lava/t11/421x253", 0x779a41bbd8192e74),
    ("quiet/lava/t12/344x224", 0xe2c0b7b7e4d33966),
    ("quiet/lava/t12/421x253", 0x36e068675095eabe),
    ("lit/keep/t0/344x224", 0xfd607ffa8f53688f),
    ("lit/keep/t0/421x253", 0x39828c9a290be07e),
    ("lit/keep/t1/344x224", 0x7e54e9f064241381),
    ("lit/keep/t1/421x253", 0xa86161c5fef520b1),
    ("lit/keep/t2/344x224", 0x364ce3224d98a4df),
    ("lit/keep/t2/421x253", 0xeefea45b4dd805f0),
    ("lit/keep/t3/344x224", 0x44b07ee5dbd37027),
    ("lit/keep/t3/421x253", 0x67a2dc8356c605d0),
    ("lit/keep/t4/344x224", 0x1382ec37cc3863c4),
    ("lit/keep/t4/421x253", 0x16d311cba5204a13),
    ("lit/keep/t5/344x224", 0xb7f2e45d93f62631),
    ("lit/keep/t5/421x253", 0x55ec4c5d1a51950c),
    ("lit/keep/t11/344x224", 0xc13a28aad1d82605),
    ("lit/keep/t11/421x253", 0xdf7939711ce115bc),
    ("lit/keep/t12/344x224", 0x6016243644f9e3aa),
    ("lit/keep/t12/421x253", 0x54de15515ca003d8),
    ("lit/smithy/t0/344x224", 0xe776341a0ed6dcc5),
    ("lit/smithy/t0/421x253", 0x5956671ab8a8e8fa),
    ("lit/smithy/t1/344x224", 0x4a349ef981364e23),
    ("lit/smithy/t1/421x253", 0xfd378457e4564ca5),
    ("lit/smithy/t2/344x224", 0x8d0e86d1a1ef6ca9),
    ("lit/smithy/t2/421x253", 0x11441476e1295f9b),
    ("lit/smithy/t3/344x224", 0x8438dbb3046779d4),
    ("lit/smithy/t3/421x253", 0x6fd761da30c466d7),
    ("lit/smithy/t4/344x224", 0xc0d3fa5251f22036),
    ("lit/smithy/t4/421x253", 0x5f45149f8df7fb8b),
    ("lit/smithy/t5/344x224", 0xc800261a040aa101),
    ("lit/smithy/t5/421x253", 0x8dccc4d0b8d2c6ee),
    ("lit/smithy/t11/344x224", 0x364a03c0f09e2fe0),
    ("lit/smithy/t11/421x253", 0x408f2d99b7c07cb6),
    ("lit/smithy/t12/344x224", 0xe34b728dfe5d1030),
    ("lit/smithy/t12/421x253", 0x657c881613d3cdfc),
    ("lit/lists/t0/344x224", 0x937e513ab45e8cb6),
    ("lit/lists/t0/421x253", 0x05751dbc1246b2cf),
    ("lit/lists/t1/344x224", 0x44bdcdb112563758),
    ("lit/lists/t1/421x253", 0x65f3a3bfd6c6cfd2),
    ("lit/lists/t2/344x224", 0xb8f4275a22855bc9),
    ("lit/lists/t2/421x253", 0x52323f229b5a412a),
    ("lit/lists/t3/344x224", 0xce0ad55eb4a137e2),
    ("lit/lists/t3/421x253", 0x7f82c36f4a372afe),
    ("lit/lists/t4/344x224", 0x5351d1d22bb4c394),
    ("lit/lists/t4/421x253", 0xf6d15ebac843d6d6),
    ("lit/lists/t5/344x224", 0x917a261eea6ae41e),
    ("lit/lists/t5/421x253", 0x6cc97cd91d1f97c9),
    ("lit/lists/t11/344x224", 0x6f8c7164fbacd839),
    ("lit/lists/t11/421x253", 0x89fd1c57614dee31),
    ("lit/lists/t12/344x224", 0xdf9f02b8b78c9fc2),
    ("lit/lists/t12/421x253", 0x0d13f057bbfc7665),
    ("lit/colosseum/t0/344x224", 0xea9c5d4e08b14895),
    ("lit/colosseum/t0/421x253", 0x8d5e5ebc688c45e2),
    ("lit/colosseum/t1/344x224", 0xa44226e95c84af8a),
    ("lit/colosseum/t1/421x253", 0x0c13542b23a9db2e),
    ("lit/colosseum/t2/344x224", 0xcd997c6407376328),
    ("lit/colosseum/t2/421x253", 0x9d3d86f9bf07969c),
    ("lit/colosseum/t3/344x224", 0xd074db48aa3ef0cc),
    ("lit/colosseum/t3/421x253", 0xe3bc5f60f4028235),
    ("lit/colosseum/t4/344x224", 0x2e1029c2206b99c5),
    ("lit/colosseum/t4/421x253", 0xd2b72b94a8047060),
    ("lit/colosseum/t5/344x224", 0x3c90925490ec6091),
    ("lit/colosseum/t5/421x253", 0x80f7536bbc9b8fb7),
    ("lit/colosseum/t11/344x224", 0xe953551ea6f67793),
    ("lit/colosseum/t11/421x253", 0x77be5a6df13f0223),
    ("lit/colosseum/t12/344x224", 0x5b18d1072970bb7d),
    ("lit/colosseum/t12/421x253", 0x66106e7d5e928c50),
    ("lit/mines/t0/344x224", 0x089d4dd3187164b1),
    ("lit/mines/t0/421x253", 0x280ea47a0b6c514e),
    ("lit/mines/t1/344x224", 0x1eb3223cf57e6d68),
    ("lit/mines/t1/421x253", 0xd9080b3ece2757d4),
    ("lit/mines/t2/344x224", 0xfad001ebc60e7a20),
    ("lit/mines/t2/421x253", 0x0d252fd27af292dc),
    ("lit/mines/t3/344x224", 0x7dc967df194453d5),
    ("lit/mines/t3/421x253", 0x7767b121d9a612e9),
    ("lit/mines/t4/344x224", 0xb4a7a6ffd3de1d71),
    ("lit/mines/t4/421x253", 0x6cee5886faa9e891),
    ("lit/mines/t5/344x224", 0xe229b182a2548c06),
    ("lit/mines/t5/421x253", 0x18600680d63e90e3),
    ("lit/mines/t11/344x224", 0x985a489f8227ec85),
    ("lit/mines/t11/421x253", 0xbc374feed84ea0bb),
    ("lit/mines/t12/344x224", 0x3b14869c7a1e01f4),
    ("lit/mines/t12/421x253", 0xab925ac1b019edf9),
    ("lit/dragon/t0/344x224", 0x9d8a6a9e14e7d8f5),
    ("lit/dragon/t0/421x253", 0x01a46319147cf9fc),
    ("lit/dragon/t1/344x224", 0xda62a3d6756f7a18),
    ("lit/dragon/t1/421x253", 0x8c57556f43e6574f),
    ("lit/dragon/t2/344x224", 0x288ca7ba2daef803),
    ("lit/dragon/t2/421x253", 0x7445f5f7c7f5a6f1),
    ("lit/dragon/t3/344x224", 0xf585a2d117a25c87),
    ("lit/dragon/t3/421x253", 0x8b5e530442d2b2fb),
    ("lit/dragon/t4/344x224", 0xf9e0ac93f7ef55d4),
    ("lit/dragon/t4/421x253", 0x3d77aa1dd6b44ebf),
    ("lit/dragon/t5/344x224", 0x1c3cedc173a5c08d),
    ("lit/dragon/t5/421x253", 0x062c7e92257b03f5),
    ("lit/dragon/t11/344x224", 0x16de8a8e5a0bc8cc),
    ("lit/dragon/t11/421x253", 0xaafa86837782ea20),
    ("lit/dragon/t12/344x224", 0xa6cdb360acc8c4bf),
    ("lit/dragon/t12/421x253", 0xf38eaace428803fe),
    ("lit/water/t0/344x224", 0xe171daa42a074d1d),
    ("lit/water/t0/421x253", 0x865b798011903847),
    ("lit/water/t1/344x224", 0x7ee43dac3365bc3e),
    ("lit/water/t1/421x253", 0xefaa3aab2aef920d),
    ("lit/water/t2/344x224", 0xa0309937a510eaf7),
    ("lit/water/t2/421x253", 0x1fdab2c570a87b39),
    ("lit/water/t3/344x224", 0x6918f009e30ac8c0),
    ("lit/water/t3/421x253", 0x13b662e697feb615),
    ("lit/water/t4/344x224", 0xe1a443d2a3ee7934),
    ("lit/water/t4/421x253", 0x4c5b8f9d205c79ec),
    ("lit/water/t5/344x224", 0xf97b0906b8474276),
    ("lit/water/t5/421x253", 0x0d38861b796cd861),
    ("lit/water/t11/344x224", 0xf325c5d8e8bfc20d),
    ("lit/water/t11/421x253", 0x459fec197d66b8ea),
    ("lit/water/t12/344x224", 0x3544ea6a1762319b),
    ("lit/water/t12/421x253", 0xee399d08c6539121),
    ("lit/stream/t0/344x224", 0x7f14ef4a4eb8aed3),
    ("lit/stream/t0/421x253", 0x1f85d0262a9b75bc),
    ("lit/stream/t1/344x224", 0x517f364370495df5),
    ("lit/stream/t1/421x253", 0x7c8688b5033feee0),
    ("lit/stream/t2/344x224", 0x10fb1bf2d9eba1e6),
    ("lit/stream/t2/421x253", 0x0bd00903d7eb0329),
    ("lit/stream/t3/344x224", 0x47236f6e1636e144),
    ("lit/stream/t3/421x253", 0x901e2a8fb901a152),
    ("lit/stream/t4/344x224", 0x8cb5332adc93ac68),
    ("lit/stream/t4/421x253", 0xfb078d1e1c37d208),
    ("lit/stream/t5/344x224", 0x08f1974616e451e6),
    ("lit/stream/t5/421x253", 0x71e55206f391ad8a),
    ("lit/stream/t11/344x224", 0xe1836880335c1409),
    ("lit/stream/t11/421x253", 0x11d8a54fb0410ccd),
    ("lit/stream/t12/344x224", 0x60dcb75c0e162b2b),
    ("lit/stream/t12/421x253", 0xbac78085b72dd217),
    ("lit/lava/t0/344x224", 0x532cd69bc20be8cf),
    ("lit/lava/t0/421x253", 0xe42fb8aab26afdcc),
    ("lit/lava/t1/344x224", 0xec68516dd7db6f11),
    ("lit/lava/t1/421x253", 0x7dad22021429c104),
    ("lit/lava/t2/344x224", 0x775ab981162ce007),
    ("lit/lava/t2/421x253", 0xc0f3a7cbdec1f854),
    ("lit/lava/t3/344x224", 0x3456ff98a66ca692),
    ("lit/lava/t3/421x253", 0xea1b3e5e1cf6f41f),
    ("lit/lava/t4/344x224", 0xdf84de9bde4b1cf9),
    ("lit/lava/t4/421x253", 0x397a54c9a8d35360),
    ("lit/lava/t5/344x224", 0x5410e131c168209c),
    ("lit/lava/t5/421x253", 0x5cdbe5b67bdcd70f),
    ("lit/lava/t11/344x224", 0xebe47b679014cc23),
    ("lit/lava/t11/421x253", 0x14199ff9a5d97fe1),
    ("lit/lava/t12/344x224", 0xa285fe2b80b17388),
    ("lit/lava/t12/421x253", 0x482a816e737ec094),
    ("stormy/keep/t0/344x224", 0xd2abad449c9f121c),
    ("stormy/keep/t0/421x253", 0x40eb8791435635dc),
    ("stormy/keep/t1/344x224", 0x95b4bc8219e8137d),
    ("stormy/keep/t1/421x253", 0xf0141440895f21bc),
    ("stormy/keep/t2/344x224", 0x9c510c6a29f882fb),
    ("stormy/keep/t2/421x253", 0x5ba9380147b52f1d),
    ("stormy/keep/t3/344x224", 0x1c25e1e577c85f81),
    ("stormy/keep/t3/421x253", 0x9a45ab601110aa83),
    ("stormy/keep/t4/344x224", 0x5b101726a15876d9),
    ("stormy/keep/t4/421x253", 0x0674a711067a4243),
    ("stormy/keep/t5/344x224", 0xd72b98203b5fcf13),
    ("stormy/keep/t5/421x253", 0x12296e25368beca3),
    ("stormy/keep/t11/344x224", 0xc91386d0a5e1dfb1),
    ("stormy/keep/t11/421x253", 0x14077acf0e726111),
    ("stormy/keep/t12/344x224", 0xe9ff4b8b8d8a0d9d),
    ("stormy/keep/t12/421x253", 0x6291c210409afdce),
    ("stormy/smithy/t0/344x224", 0xce0f6d7fd3872e66),
    ("stormy/smithy/t0/421x253", 0xf7bc5f59f41ff2a5),
    ("stormy/smithy/t1/344x224", 0xd603039a17650b7f),
    ("stormy/smithy/t1/421x253", 0xf93405b46bbdacca),
    ("stormy/smithy/t2/344x224", 0xee4fcc6ea60c9c7b),
    ("stormy/smithy/t2/421x253", 0x6189034219778a1c),
    ("stormy/smithy/t3/344x224", 0x875d82daeca28c49),
    ("stormy/smithy/t3/421x253", 0xb494f0233dd4587e),
    ("stormy/smithy/t4/344x224", 0xd8a1ab48f46e9acb),
    ("stormy/smithy/t4/421x253", 0xbe7d3a2030d81e1c),
    ("stormy/smithy/t5/344x224", 0x9c4e6b7f9f486f3f),
    ("stormy/smithy/t5/421x253", 0x35124abecbab7f27),
    ("stormy/smithy/t11/344x224", 0x2bda14e8d7f0931a),
    ("stormy/smithy/t11/421x253", 0xf6016dee7687402a),
    ("stormy/smithy/t12/344x224", 0x83c84f67fd347cec),
    ("stormy/smithy/t12/421x253", 0xc6d510a5f7e4474b),
    ("stormy/lists/t0/344x224", 0xdc200f2d25d70328),
    ("stormy/lists/t0/421x253", 0xa37e5a6ae814306b),
    ("stormy/lists/t1/344x224", 0xf93fd626d6094923),
    ("stormy/lists/t1/421x253", 0x9af28b6296f9bb23),
    ("stormy/lists/t2/344x224", 0x2dfecb5598f0366b),
    ("stormy/lists/t2/421x253", 0xd14ca0e61bbd16e2),
    ("stormy/lists/t3/344x224", 0xa53f98421e4d0ad4),
    ("stormy/lists/t3/421x253", 0x427a267ffff05372),
    ("stormy/lists/t4/344x224", 0x758d86898c9352aa),
    ("stormy/lists/t4/421x253", 0x3d54fbe8e99155c6),
    ("stormy/lists/t5/344x224", 0x2cd5a3d1d5b5c1dc),
    ("stormy/lists/t5/421x253", 0x46fa8add2a5dbd25),
    ("stormy/lists/t11/344x224", 0x5c278a0b96f5d5ba),
    ("stormy/lists/t11/421x253", 0xb9bd288f846fa2ac),
    ("stormy/lists/t12/344x224", 0x1f52c2ebc4b0a490),
    ("stormy/lists/t12/421x253", 0xd22269b1392d5441),
    ("stormy/colosseum/t0/344x224", 0xea9c5d4e08b14895),
    ("stormy/colosseum/t0/421x253", 0x8d5e5ebc688c45e2),
    ("stormy/colosseum/t1/344x224", 0xa44226e95c84af8a),
    ("stormy/colosseum/t1/421x253", 0x0c13542b23a9db2e),
    ("stormy/colosseum/t2/344x224", 0xcd997c6407376328),
    ("stormy/colosseum/t2/421x253", 0x9d3d86f9bf07969c),
    ("stormy/colosseum/t3/344x224", 0xd074db48aa3ef0cc),
    ("stormy/colosseum/t3/421x253", 0xe3bc5f60f4028235),
    ("stormy/colosseum/t4/344x224", 0x2e1029c2206b99c5),
    ("stormy/colosseum/t4/421x253", 0xd2b72b94a8047060),
    ("stormy/colosseum/t5/344x224", 0x3c90925490ec6091),
    ("stormy/colosseum/t5/421x253", 0x80f7536bbc9b8fb7),
    ("stormy/colosseum/t11/344x224", 0xe953551ea6f67793),
    ("stormy/colosseum/t11/421x253", 0x77be5a6df13f0223),
    ("stormy/colosseum/t12/344x224", 0x5b18d1072970bb7d),
    ("stormy/colosseum/t12/421x253", 0x66106e7d5e928c50),
    ("stormy/mines/t0/344x224", 0x089d4dd3187164b1),
    ("stormy/mines/t0/421x253", 0x280ea47a0b6c514e),
    ("stormy/mines/t1/344x224", 0x1eb3223cf57e6d68),
    ("stormy/mines/t1/421x253", 0xd9080b3ece2757d4),
    ("stormy/mines/t2/344x224", 0xfad001ebc60e7a20),
    ("stormy/mines/t2/421x253", 0x0d252fd27af292dc),
    ("stormy/mines/t3/344x224", 0x7dc967df194453d5),
    ("stormy/mines/t3/421x253", 0x7767b121d9a612e9),
    ("stormy/mines/t4/344x224", 0xb4a7a6ffd3de1d71),
    ("stormy/mines/t4/421x253", 0x6cee5886faa9e891),
    ("stormy/mines/t5/344x224", 0xe229b182a2548c06),
    ("stormy/mines/t5/421x253", 0x18600680d63e90e3),
    ("stormy/mines/t11/344x224", 0x985a489f8227ec85),
    ("stormy/mines/t11/421x253", 0xbc374feed84ea0bb),
    ("stormy/mines/t12/344x224", 0x3b14869c7a1e01f4),
    ("stormy/mines/t12/421x253", 0xab925ac1b019edf9),
    ("stormy/dragon/t0/344x224", 0x9d8a6a9e14e7d8f5),
    ("stormy/dragon/t0/421x253", 0x01a46319147cf9fc),
    ("stormy/dragon/t1/344x224", 0xda62a3d6756f7a18),
    ("stormy/dragon/t1/421x253", 0x8c57556f43e6574f),
    ("stormy/dragon/t2/344x224", 0x288ca7ba2daef803),
    ("stormy/dragon/t2/421x253", 0x7445f5f7c7f5a6f1),
    ("stormy/dragon/t3/344x224", 0xf585a2d117a25c87),
    ("stormy/dragon/t3/421x253", 0x8b5e530442d2b2fb),
    ("stormy/dragon/t4/344x224", 0xf9e0ac93f7ef55d4),
    ("stormy/dragon/t4/421x253", 0x3d77aa1dd6b44ebf),
    ("stormy/dragon/t5/344x224", 0x1c3cedc173a5c08d),
    ("stormy/dragon/t5/421x253", 0x062c7e92257b03f5),
    ("stormy/dragon/t11/344x224", 0x16de8a8e5a0bc8cc),
    ("stormy/dragon/t11/421x253", 0xaafa86837782ea20),
    ("stormy/dragon/t12/344x224", 0xa6cdb360acc8c4bf),
    ("stormy/dragon/t12/421x253", 0xf38eaace428803fe),
    ("stormy/water/t0/344x224", 0xe171daa42a074d1d),
    ("stormy/water/t0/421x253", 0x865b798011903847),
    ("stormy/water/t1/344x224", 0x7ee43dac3365bc3e),
    ("stormy/water/t1/421x253", 0xefaa3aab2aef920d),
    ("stormy/water/t2/344x224", 0xa0309937a510eaf7),
    ("stormy/water/t2/421x253", 0x1fdab2c570a87b39),
    ("stormy/water/t3/344x224", 0x6918f009e30ac8c0),
    ("stormy/water/t3/421x253", 0x13b662e697feb615),
    ("stormy/water/t4/344x224", 0xe1a443d2a3ee7934),
    ("stormy/water/t4/421x253", 0x4c5b8f9d205c79ec),
    ("stormy/water/t5/344x224", 0xf97b0906b8474276),
    ("stormy/water/t5/421x253", 0x0d38861b796cd861),
    ("stormy/water/t11/344x224", 0xf325c5d8e8bfc20d),
    ("stormy/water/t11/421x253", 0x459fec197d66b8ea),
    ("stormy/water/t12/344x224", 0x3544ea6a1762319b),
    ("stormy/water/t12/421x253", 0xee399d08c6539121),
    ("stormy/stream/t0/344x224", 0x7f14ef4a4eb8aed3),
    ("stormy/stream/t0/421x253", 0x1f85d0262a9b75bc),
    ("stormy/stream/t1/344x224", 0x517f364370495df5),
    ("stormy/stream/t1/421x253", 0x7c8688b5033feee0),
    ("stormy/stream/t2/344x224", 0x10fb1bf2d9eba1e6),
    ("stormy/stream/t2/421x253", 0x0bd00903d7eb0329),
    ("stormy/stream/t3/344x224", 0x47236f6e1636e144),
    ("stormy/stream/t3/421x253", 0x901e2a8fb901a152),
    ("stormy/stream/t4/344x224", 0x8cb5332adc93ac68),
    ("stormy/stream/t4/421x253", 0xfb078d1e1c37d208),
    ("stormy/stream/t5/344x224", 0x08f1974616e451e6),
    ("stormy/stream/t5/421x253", 0x71e55206f391ad8a),
    ("stormy/stream/t11/344x224", 0xe1836880335c1409),
    ("stormy/stream/t11/421x253", 0x11d8a54fb0410ccd),
    ("stormy/stream/t12/344x224", 0x60dcb75c0e162b2b),
    ("stormy/stream/t12/421x253", 0xbac78085b72dd217),
    ("stormy/lava/t0/344x224", 0x532cd69bc20be8cf),
    ("stormy/lava/t0/421x253", 0xe42fb8aab26afdcc),
    ("stormy/lava/t1/344x224", 0xec68516dd7db6f11),
    ("stormy/lava/t1/421x253", 0x7dad22021429c104),
    ("stormy/lava/t2/344x224", 0x775ab981162ce007),
    ("stormy/lava/t2/421x253", 0xc0f3a7cbdec1f854),
    ("stormy/lava/t3/344x224", 0x3456ff98a66ca692),
    ("stormy/lava/t3/421x253", 0xea1b3e5e1cf6f41f),
    ("stormy/lava/t4/344x224", 0xdf84de9bde4b1cf9),
    ("stormy/lava/t4/421x253", 0x397a54c9a8d35360),
    ("stormy/lava/t5/344x224", 0x5410e131c168209c),
    ("stormy/lava/t5/421x253", 0x5cdbe5b67bdcd70f),
    ("stormy/lava/t11/344x224", 0xebe47b679014cc23),
    ("stormy/lava/t11/421x253", 0x14199ff9a5d97fe1),
    ("stormy/lava/t12/344x224", 0xa285fe2b80b17388),
    ("stormy/lava/t12/421x253", 0x482a816e737ec094),
    ("again/t7", 0xde0f300fda63cd67),
    ("again/t8", 0x864696314917a3fc),
    ("again/t7", 0xde0f300fda63cd67),
];
