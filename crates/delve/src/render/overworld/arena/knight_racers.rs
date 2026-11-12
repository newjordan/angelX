//! Knight Racer: the example's armored engines and bent hell course, drawn
//! with Delve's shared 3D raster and per-seat chase camera.
#[path = "knight_racers_instruments.rs"]
mod instruments;
#[path = "knight_racers_world/mod.rs"]
mod worldkit;
use instruments::{instruments, pods, screens};
pub(super) use instruments::garage;
use super::super::ink::{Img, glyph};
use super::race3d::dots_with;
use super::roundtable::{self as shell, Look};
use crate::rules::{Run, knight_racers as rules};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, mat};
use crate::render::world3d::raster::{self, View3, Firelight, Backdrop};
use std::f32::consts::{PI, TAU};
const LOOK: Look = Look { name: rules::NAME, backdrop: 'F', edge: 'y', accent: 'T' };
// Hardcore palette mapped onto the game's semantic materials:
// black iron -> STONE_DARK, steel -> ARMOR, Excalibur Blue -> CRYSTAL / CLOTH_BLUE, exhaust -> FLAME / EMBER.
const IRON: u8 = mat::STONE_DARK;
const STEEL: u8 = mat::ARMOR;
const GUN: u8 = mat::HORSE_GREY; // the engines' gunmetal, darker than a knight's steel
const BLUE: u8 = mat::CRYSTAL;
const LAVA: u8 = mat::EMBER;
const LAVA_HOT: u8 = mat::FLAME;

fn tri(m: &mut Mesh, a: V3, b: V3, c: V3, k: u8) {
    m.push_tri([a, b, c], [[0.0; 2]; 3], k);
}

fn cuboid(m: &mut Mesh, lo: V3, hi: V3, k: u8) {
    let p = |x: bool, y: bool, z: bool| v3(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z });
    let (f, t) = (false, true);
    m.push_quad(p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t), k);
    m.push_quad(p(f, f, f), p(t, f, f), p(t, t, f), p(f, t, f), k);
    m.push_quad(p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t), k);
    m.push_quad(p(f, t, f), p(t, t, f), p(t, t, t), p(f, t, t), k);
    m.push_quad(p(f, f, f), p(f, t, f), p(f, t, t), p(f, f, t), k);
    m.push_quad(p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t), k);
}

/// n-gon prism along +x from x0 to x1, centred (y,z), radii r0 -> r1.
fn barrel(m: &mut Mesh, [x0, x1]: [f32; 2], [y, z]: [f32; 2], [r0, r1]: [f32; 2], n: usize, k: u8, caps: bool) {
    let ring = |x: f32, r: f32, i: usize| {
        let a = i as f32 / n as f32 * TAU + PI / n as f32;
        v3(x, y + r * a.cos(), z + r * a.sin())
    };
    for i in 0..n {
        let j = (i + 1) % n;
        m.push_quad(ring(x0, r0, i), ring(x0, r0, j), ring(x1, r1, j), ring(x1, r1, i), k);
        if caps {
            for (x, r) in [(x0, r0), (x1, r1)] {
                tri(m, v3(x, y, z), ring(x, r, i), ring(x, r, j), k);
            }
        }
    }
}

/// A pool of the racer's light on the floor under `c`: two nested ellipses, so
/// it falls off in steps (see `pool`, which dims whatever lies flat on the floor).
fn glow_pool(m: &mut Mesh, c: V3, [rx, ry]: [f32; 2], k: u8) {
    for (w, z) in [(1.0f32, 0.03), (0.62, 0.05)] {
        let at = |i: usize| c + v3(rx * w * (i as f32 / 12.0 * TAU).cos(), ry * w * (i as f32 / 12.0 * TAU).sin(), z);
        (0..12).for_each(|i| tri(m, v3(c.x, c.y, z), at(i), at(i + 1), k));
    }
}

/// Four-sided cone from a ring at `base` (radius r) to a tip.
fn spike(m: &mut Mesh, base: V3, tip: V3, r: f32, k: u8) {
    let ax = (tip - base).normalize();
    let u = ax.cross(if ax.z.abs() > 0.9 { v3(1.0, 0.0, 0.0) } else { V3::UP }).normalize();
    let w = ax.cross(u);
    let at = |i: usize| {
        let (s, c) = (i as f32 / 4.0 * TAU).sin_cos();
        base + (u * c + w * s) * r
    };
    for i in 0..4 {
        tri(m, at(i), at(i + 1), tip, k);
    }
}

fn skull(m: &mut Mesh, c: V3, s: f32, face: f32) {
    cuboid(m, c + v3(-s, -s, 0.0), c + v3(s, s, 1.6 * s), mat::BONE);
    cuboid(m, c + v3(-0.6 * s, -s, -0.6 * s), c + v3(0.6 * s, s, 0.0), mat::BONE); // jaw
    for dx in [-0.45f32, 0.45] {
        // eye sockets glow lava
        cuboid(m, c + v3((dx - 0.22) * s, 0.95 * face * s, 0.7 * s), c + v3((dx + 0.22) * s, 1.05 * face * s, 1.1 * s), LAVA);
    }
}

fn chain(m: &mut Mesh, a: V3, b: V3) {
    let n = ((b - a).length() / 0.5) as usize + 1;
    for i in 0..n {
        let (p0, p1) = (a.lerp(b, i as f32 / n as f32), a.lerp(b, (i as f32 + 0.8) / n as f32));
        let w = if i % 2 == 0 { v3(0.09, 0.0, 0.0) } else { v3(0.0, 0.09, 0.0) };
        m.push_quad(p0 - w, p1 - w, p1 + w, p0 + w, IRON);
    }
}

/// A wave's rate, nudged so it fits a whole number of times into `period`
/// (none: as it is). A track that loops (`play.rs`) closes on itself unseen.
fn rate(w: f32, period: f32) -> f32 {
    if period > 0.0 { (w * period / TAU).round().max(1.0) * TAU / period } else { w }
}

fn lap() -> f32 { worldkit::route::current().lap() }
const SCALE: f32 = 0.82;
fn pose(s: f32) -> (f32, f32, f32) { worldkit::route::current().pose(s) }
/// The world around the road, as seen from a car at `s`: built by `worldkit`.
fn world(s: f32) -> Mesh { worldkit::build(worldkit::route::current(), s) }

fn point(s: f32, y: f32, z: f32) -> V3 { worldkit::point(worldkit::route::current(), s, y, z) }
/// The race's painted materials: the pods' own, and the land's (see `worldkit::palette`).
fn race_palette() -> &'static raster::Painted { worldkit::palette::painted() }

pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let Some(s) = &run.knight_racers else { return (Img::black(384, 224), Img::new(384, 224)); };
    if let Some(screen) = screens(s, focus) { return (screen, Img::new(384, 224)); }
    worldkit::route::select(s.course, s.ticks);
    let Some((&seat, car)) = focus.and_then(|id| s.cars.get_key_value(&id)).or_else(|| s.cars.iter().next()) else {
        return (Img::black(384, 224), Img::new(384, 224));
    };
    let shot = instruments::shot(s, car, seat);
    let view = shot.view;
    // The lens leans into a bend for as long as this frame is drawn (the instruments' projections lean with it).
    let _lean = raster::roll(shot.roll);
    let mut movers = Mesh::new();
    for (&other, c) in &s.cars {
        pods::add(&mut movers, s, other, c, view.pos);
    }
    let track = world(car.d);
    let tunnel = s.course().stretch(car.d.rem_euclid(s.course().lap())) == rules::Stretch::Tunnel;
    let lighting = Firelight { gain: if tunnel { 1.30 } else { 1.65 }, warm: 0.06 };
    let look = worldkit::route::current().zone(car.d.rem_euclid(lap()) + 60.).0;
    let fog = worldkit::fog_for(look);
    let (rgba, depth) = raster::render_meshes_painted(&[&track, &movers], &view, 384, 224, lighting, fog, Backdrop::Haze(worldkit::haze(look)), Some(pods::palette()));
    let mut im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
    // Paint only untouched backdrop samples: the course/pods retain real
    // z-buffer silhouettes against the layered desert horizon.
    for y in 18..175 { for x in 0..384 {
        if !depth[(y * 384 + x) as usize].is_finite() {
            let ridge = 88. + 13. * (x as f32 * 0.025 + pose(car.d).2).sin() + 5. * (x as f32 * 0.073).sin();
            let rgb = if y as f32 > ridge { [100, 79, 69] }
                else if y as f32 > ridge - 13. { [136, 105, 82] }
                else { let f = y as f32 / 175.; [(62. + 65. * f) as u8, (100. + 47. * f) as u8, (139. + 21. * f) as u8] };
            im.set(x, y, rgb);
        }
    } }
    worldkit::dye(&mut im, rgba.as_raw());
    pods::recolour(&mut im, rgba.as_raw(), &pods::boxes(s, &view));
    instruments::feel(&mut im, s, car, seat, &view, &depth);
    let mut hud = Img::new(384, 224);
    instruments(&mut hud, s, car, seat, &view, &depth);
    for y in 0..224 { for x in 0..384 { if let Some(c) = hud.get(x, y) { im.set(x, y, c); } } }
    (im, Img::new(384, 224))
}
pub(super) fn kingdom(im: &mut Img, _run: &Run, origin: (i32, i32)) {
    shell::stub_kingdom(im, origin, rules::ENTRY, &LOOK);
}

#[cfg(test)]
mod podracer_gauntlet {
    use super::*;
    use crate::rules::Input;
    use std::collections::BTreeMap;

    #[test]
    fn local_track_geometry_stays_bounded_for_the_whole_lap() {
        let counts: Vec<_> = [0., 80., 320., 600., 850.].into_iter().map(|d| world(d).tris.len()).collect();
        let lo = *counts.iter().min().unwrap();
        let hi = *counts.iter().max().unwrap();
        // Sections differ (open dune vs cavern vault). A doubled window was the
        // old bug — pillars emitted from zero and growing by a lap. Three times
        // still catches that, and allows the cavern to be denser than the dune.
        assert!(hi < lo * 3, "local mesh grew around lap: {counts:?}");
        let mut run = Run::roundtable_for_test("knight_racers").unwrap();
        let car = run.knight_racers.as_mut().unwrap().cars.get_mut(&1).unwrap();
        for d in [0., 280., 340., 600., 780., 850.] {
            for lane in [-8., 0., 8.] {
                car.d = d; car.y = lane;
                let camera = instruments::chase(car, 120);
                let expected = point(d - instruments::camera::BACK, lane * instruments::camera::FOLLOW, instruments::camera::UP);
                assert!((camera.pos - expected).length() < 0.01,
                    "chase eye left the course at d={d}, lane={lane}");
            }
        }
    }

    #[test]
    fn rendered_six_seat_podracer_gauntlet() {
        let mut run = Run::roundtable_for_test("knight_racers").unwrap();
        // The proving circuit: six cars driven flat out with no steering get round it.
        let mut race = rules::Session::blank_on(rules::CourseId::Proving);
        race.join_ids(1..=6);
        run.knight_racers = None;
        let drive: BTreeMap<_, _> = (1..=6).map(|id| (id, Input { move_y: -1, dash: true, ..Input::default() })).collect();
        let out = std::env::var_os("ANGEL_PODRACER_SHOTS").map(std::path::PathBuf::from);
        if let Some(out) = &out { std::fs::create_dir_all(out).unwrap(); }
        if let Some(out) = &out {
            run.knight_racers = Some(race.clone());
            let (im, _) = layers(&run, Some(6));
            image::save_buffer(out.join("00-rear-grid.png"), &im.rgba_bytes(), 384, 224, image::ColorType::Rgba8).unwrap();
        }
        let mut previous = None;
        for (name, ticks) in [("01-grid", 0), ("02-go", 95), ("03-boost", 140), ("04-cooling", 210), ("05-bend", 420)] {
            while race.ticks < ticks { race.step(&drive); assert!(race.valid()); }
            run.knight_racers = Some(race.clone());
            let (im, _) = layers(&run, Some(1));
            let rgba = im.rgba_bytes();
            assert_eq!(rgba.len(), 384 * 224 * 4);
            // A non-empty course and pod below the header, not just passing HUD text.
            let lit = (25..170).flat_map(|y| (30..295).map(move |x| (x,y)))
                .filter(|&(x,y)| im.get(x,y).is_some_and(|c| c[0].max(c[1]).max(c[2]) > 40)).count();
            assert!(lit > 1000, "{name}: race view is blank/dark ({lit})");
            if let Some(prev) = &previous { assert_ne!(prev, &rgba, "frozen race frame at {name}"); }
            if let Some(out) = &out { image::save_buffer(out.join(format!("{name}.png")), &rgba, 384, 224, image::ColorType::Rgba8).unwrap(); }
            previous = Some(rgba);
        }
        for seat in 1..=6 {
            let (im, _) = layers(&run, Some(seat));
            assert_eq!(im.rgba_bytes().len(), 384 * 224 * 4);
        }
        let car = race.cars.get_mut(&1).unwrap();
        let wall = rules::wall_at(car.d);
        car.y = wall + 2.;
        car.vy = 13.;
        race.step(&[(1, Input { move_x: 1, ..Input::default() })].into());
        assert_eq!(race.cars[&1].impact, 12);
        run.knight_racers = Some(race.clone());
        if let Some(out) = &out { let (im, _) = layers(&run, Some(1)); image::save_buffer(out.join("06-wall.png"), &im.rgba_bytes(), 384, 224, image::ColorType::Rgba8).unwrap(); }
        // 2026-11-07: six pods held flat out with no lift now cook their engines and blow them (the racer's law), each costing a few seconds: they still finish, later.
        while !race.finished && race.ticks < 24000 { race.step(&drive); assert!(race.valid()); }
        assert!(race.finished, "six seats must finish the race, progress {:?}", race.cars.iter().map(|(id, c)| (id, c.d, c.finish)).collect::<Vec<_>>());
        run.knight_racers = Some(race.clone());
        if let Some(out) = &out { let (im, _) = layers(&run, Some(1)); image::save_buffer(out.join("07-results.png"), &im.rgba_bytes(), 384, 224, image::ColorType::Rgba8).unwrap(); }
        race.step(&[(1, Input { interact: true, ..Input::default() })].into());
        assert_eq!(race.ticks, 0);
        assert_eq!(race.cars.len(), 6);
        assert!(!race.finished);
    }
}
