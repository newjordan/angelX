//! Headless proof: chase frames of a bot-driven lap through the real `layers`,
//! and wide establishing shots through the same raster. Writes PNGs when
//! `ANGEL_PODRACER_WORLD_SHOTS` names a directory; otherwise does nothing.
use super::super::*;
use std::path::PathBuf;

fn out_dir() -> Option<PathBuf> {
    let d = std::env::var_os("ANGEL_PODRACER_WORLD_SHOTS").map(PathBuf::from)?;
    std::fs::create_dir_all(&d).unwrap();
    Some(d)
}

pub(super) fn save(dir: &std::path::Path, name: &str, im: &Img) {
    image::save_buffer(dir.join(name), &im.rgba_bytes(), 384, 224, image::ColorType::Rgba8).unwrap();
}

/// The six pods of a session, posed as `layers` poses them (no lens close
/// enough to shrink one: these cameras are wide).
pub(super) fn pack(s: &rules::Session) -> Mesh {
    let mut movers = Mesh::new();
    for (&other, c) in &s.cars {
        pods::add(&mut movers, s, other, c, v3(0., 0., 1.0e5));
    }
    movers
}

/// One frame from any camera, through the same raster and dots as the race.
pub(super) fn view_frame(s: &rules::Session, at: f32, view: &View3) -> Img {
    let track = super::gather_view(super::route::current(), at, s.ticks, view.pos, view.heading_rad, 3.0, &mut Default::default());
    let movers = pack(s);
    let look = super::route::current().zone(at.rem_euclid(super::route::current().lap()) + 60.).0;
    let (rgba, _) = raster::render_meshes_painted(&[&track, &movers], view, 384, 224, Firelight { gain: 1.65, warm: 0.06 }, super::fog_for(look), Backdrop::Haze(super::haze(look)), Some(race_palette()));
    let mut im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
    super::dye(&mut im, rgba.as_raw());
    im
}

pub(super) fn aim(from: V3, to: V3, fov: f32) -> View3 {
    let d = to - from;
    View3 { pos: from, heading_rad: d.y.atan2(d.x), pitch: d.z.atan2(d.x.hypot(d.y)), fov_rad: fov }
}

pub(super) fn course_id() -> rules::CourseId {
    match std::env::var("ANGEL_PODRACER_COURSE").ok().as_deref() {
        Some("1" | "emberfall") => rules::CourseId::Emberfall,
        Some("2" | "glassway") => rules::CourseId::Glassway,
        Some("3" | "moatwalk") => rules::CourseId::Moatwalk,
        _ => rules::CourseId::Proving,
    }
}

#[test]
fn world_shots() {
    let Some(dir) = out_dir() else { return };
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let id = course_id();
    let mut race = rules::Session::blank_on(id);
    race.join_ids(1..=6);
    super::route::select(id, 0);
    let lap = rules::course(id).lap();
    let every: u32 = std::env::var("ANGEL_PODRACER_EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let laps: f32 = std::env::var("ANGEL_PODRACER_LAPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    // Establishing shots: (s of the leader, eye's lateral, eye's height, look-ahead along the road, height looked at).
    let wide: Vec<(f32, f32, f32, f32, f32)> = (0..6).map(|i| {
        let s = lap * (0.03 + 0.165 * i as f32);
        match i % 3 {
            0 => (s, 34., 52., 380., 18.),
            1 => (s, -70., 70., 140., 0.),
            _ => (s, 20., 30., 300., 24.),
        }
    }).collect();
    let mut frame = 0;
    let mut wide_done = vec![false; wide.len()];
    let mut ticks = 0;
    loop {
        race.step_drive(&race.bot_drives_all());
        ticks += 1;
        let lead = race.cars.values().map(|c| c.d).fold(0., f32::max);
        let d1 = race.cars[&1].d;
        run.knight_racers = Some(race.clone());
        if ticks % every == 0 && race.ticks > 90 {
            let (im, _) = layers(&run, Some(1));
            save(&dir, &format!("chase_{frame:04}.png"), &im);
            frame += 1;
        }
        for (i, &(s, lat, up, ahead, look)) in wide.iter().enumerate() {
            if !wide_done[i] && d1 >= s {
                wide_done[i] = true;
                super::route::select(id, race.ticks);
                let mut eye = point(s - 70., lat, up);
                // Never inside the hill: at least fifteen metres over the land under the eye.
                eye.z = eye.z.max(super::route::current().ground(eye.x, eye.y) + 15.);
                let view = aim(eye, point(s + ahead, 0., look), 1.3);
                save(&dir, &format!("wide_{i}.png"), &view_frame(&race, s, &view));
            }
        }
        if lead >= lap * laps || race.ticks > 24000 { break; }
    }
    eprintln!("wrote {frame} chase frames, lap {lap}");
}

/// A course with one piece on it, for looking at pieces one at a time.
struct Gallery<'a> { base: &'a dyn super::route::Route, piece: super::route::Piece, key: u32 }

impl super::route::Route for Gallery<'_> {
    fn key(&self) -> u32 { self.key }
    fn lap(&self) -> f32 { self.base.lap() }
    fn pose(&self, s: f32) -> (f32, f32, f32) { self.base.pose(s) }
    fn height(&self, s: f32) -> f32 { self.base.height(s) }
    fn width(&self, s: f32) -> (f32, f32) { self.base.width(s) }
    fn zone(&self, s: f32) -> (super::route::Look, &'static str) { self.base.zone(s) }
    fn sink(&self, s: f32) -> f32 { self.base.sink(s) }
    fn ground(&self, x: f32, y: f32) -> f32 { self.base.ground(x, y) }
    fn pieces(&self) -> &[super::route::Piece] { std::slice::from_ref(&self.piece) }
    fn forks(&self) -> &[super::route::ForkInfo] { &[] }
    fn frame(&self, line: u8, s: f32) -> super::route::RoadFrame { self.base.frame(line, s) }
    fn hazards(&self) -> Vec<super::route::Hazard> { Vec::new() }
    fn strips(&self) -> Vec<super::route::Strip> { Vec::new() }
    fn gaps(&self) -> Vec<super::route::Gap> { if self.piece.kind == super::route::Kind::ChasmJump { vec![super::route::Gap { s0: self.piece.s0, s1: self.piece.s1, ramp: 0.2 }] } else { Vec::new() } }
    fn landmarks(&self) -> Vec<f32> { Vec::new() }
    fn curve(&self, s: f32) -> f32 { self.base.curve(s) }
    fn zones(&self) -> Vec<(f32, f32, super::route::Look, &'static str)> { self.base.zones() }
}

fn render_route(r: &dyn super::route::Route, s: f32, view: &View3) -> Img { render_route_at(r, s, 0, view) }

/// The world alone (no pods, no HUD) through `view`, as the race draws it, at race tick `ticks`.
fn render_route_at(r: &dyn super::route::Route, s: f32, ticks: u32, view: &View3) -> Img {
    let track = super::gather_view(r, s, ticks, view.pos, view.heading_rad, 3.0, &mut Default::default());
    let look = r.zone(s + 60.).0;
    let (rgba, _) = raster::render_meshes_painted(&[&track], view, 384, 224, Firelight { gain: 1.65, warm: 0.06 }, super::fog_for(look), Backdrop::Haze(super::haze(look)), Some(race_palette()));
    let mut im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
    super::dye(&mut im, rgba.as_raw());
    im
}

/// The road and what lies beside it through the chase lens's own height and reach, with no pod and no
/// HUD in the way, at the distances `ANGEL_PODRACER_AT` lists (comma separated; a spread of the lap
/// by default), at race tick `ANGEL_PODRACER_TICKS` (300). A second lens stands high, beside the road.
#[test]
fn road_look() {
    let Some(dir) = out_dir() else { return };
    let id = course_id();
    let name = format!("{id:?}").to_lowercase();
    let ticks: u32 = std::env::var("ANGEL_PODRACER_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
    super::route::select(id, ticks);
    let r = super::route::on(id);
    let at: Vec<f32> = match std::env::var("ANGEL_PODRACER_AT") {
        Ok(v) => v.split(',').filter_map(|x| x.parse().ok()).collect(),
        Err(_) => (0..12).map(|i| r.lap() * (0.02 + 0.08 * i as f32)).collect(),
    };
    for s in at {
        let eye = point_on(r, s - 14., 0., 3.4);
        let view = aim(eye, point_on(r, s + 26., 0., 1.4), 1.25);
        save(&dir, &format!("{name}_road_{s:04.0}.png"), &render_route_at(r, s, ticks, &view));
        let high = aim(point_on(r, s - 30., 20., 14.), point_on(r, s + 30., 0., 0.), 1.3);
        save(&dir, &format!("{name}_road_{s:04.0}_high.png"), &render_route_at(r, s, ticks, &high));
    }
}

#[test]
fn piece_gallery() {
    use super::route::{Kind, Look, Piece, Route};
    let Some(dir) = out_dir() else { return };
    let id = course_id();
    super::route::select(id, 0);
    let base = super::route::on(id);
    let kinds = [Kind::Bridge, Kind::Viaduct, Kind::Tunnel, Kind::Arch, Kind::Grandstand, Kind::CastleGate, Kind::Foundry, Kind::Waterfall, Kind::CrystalVault,
        Kind::ChasmJump, Kind::Drawbridge, Kind::LavaFall, Kind::Statues, Kind::Lanterns, Kind::Pylons, Kind::Gantry, Kind::Rampart, Kind::Spires];
    let only = std::env::var("ANGEL_PODRACER_ONLY").ok();
    for (n, kind) in kinds.into_iter().enumerate() {
        if only.as_ref().is_some_and(|o| !format!("{kind:?}").eq_ignore_ascii_case(o)) { continue; }
        let (s0, len) = (120., match kind { Kind::Arch => 10., Kind::CastleGate => 24., Kind::ChasmJump => 40., Kind::Gantry => 30., _ => 90. });
        let side = match kind { Kind::Grandstand | Kind::Foundry | Kind::Waterfall | Kind::LavaFall | Kind::Statues | Kind::Rampart | Kind::Spires | Kind::Pylons | Kind::Lanterns => 1., _ => 0. };
        let g = Gallery { base, piece: Piece { kind, s0, s1: s0 + len, side, seed: 5 }, key: 900 + n as u32 };
        let _ = Look::Dune;
        let mid = s0 + len * 0.5;
        let views = [
            ("road", aim(point_on(&g, s0 - 36., 0., 7.), point_on(&g, mid, 0., 4.), 1.1)),
            ("aerial", aim(point_on(&g, s0 - 40., 26., 30.), point_on(&g, mid, 0., 3.), 1.0)),
            ("side", aim(point_on(&g, mid, -34., 12.), point_on(&g, mid, 0., 5.), 1.0)),
        ];
        for (name, view) in views {
            let im = render_route(&g, mid - 30., &view);
            save(&dir, &format!("piece_{kind:?}_{name}.png"), &im);
        }
        let _ = g.lap();
    }
}

fn point_on(r: &dyn super::route::Route, s: f32, lat: f32, up: f32) -> V3 { super::point(r, s, lat, up) }

/// The chase view at the start of every named stretch of the course, and a
/// quarter of the way into it: where the places change.
#[test]
fn zone_shots() {
    let Some(dir) = out_dir() else { return };
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let id = course_id();
    let course = rules::course(id);
    let mut race = rules::Session::blank_on(id);
    race.join_ids(1..=6);
    race.ticks = 400;
    for (n, z) in course.zones().iter().enumerate() {
        for (k, at) in [z.s0 + 45., z.s0 + (z.s1 - z.s0) * 0.5].into_iter().enumerate() {
            for (slot, car) in race.cars.values_mut().enumerate() {
                car.d = at - slot as f32 * 8.;
                car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
                car.v = 40.;
            }
            run.knight_racers = Some(race.clone());
            let (im, _) = layers(&run, Some(1));
            save(&dir, &format!("zone_{n:02}_{k}.png"), &im);
        }
    }
}

/// What the eye sees, by material, from the establishing views: a way to find
/// which surface a stray patch of colour is.
#[test]
fn material_census() {
    if std::env::var_os("ANGEL_PODRACER_CENSUS").is_none() { return; }
    let id = course_id();
    super::route::select(id, 0);
    let r = super::route::on(id);
    for (n, f) in [0.03f32, 0.2, 0.36, 0.53].into_iter().enumerate() {
        let s = r.lap() * f;
        let mut eye = point(s - 70., 34., 52.);
        eye.z = eye.z.max(r.ground(eye.x, eye.y) + 15.);
        let view = aim(eye, point(s + 300., 0., 18.), 1.3);
        let mesh = super::gather_view(r, s, 0, view.pos, view.heading_rad, 3.0, &mut Default::default());
        let seen = raster::visible_materials(&mesh, &view, 192, 112);
        let mut count = [0usize; 64];
        for m in seen { count[m as usize % 64] += 1; }
        let top: Vec<_> = { let mut v: Vec<_> = count.iter().enumerate().filter(|(_, c)| **c > 0).collect(); v.sort_by_key(|(_, c)| std::cmp::Reverse(**c)); v.into_iter().take(6).map(|(m, c)| format!("{m}:{c}")).collect() };
        eprintln!("view {n} s={s:.0}: {top:?}");
    }
}

/// The named stretches of each lap, what they sink to, the gaps in the road and every set piece:
/// where to look. Prints when `ANGEL_PODRACER_TALLY` is set.
#[test]
fn zone_list() {
    if std::env::var_os("ANGEL_PODRACER_TALLY").is_none() { return; }
    for &id in rules::courses() {
        let r = super::route::on(id);
        eprintln!("ZONES {id:?} lap {:.0}", r.lap());
        for (s0, s1, look, name) in r.zones() { eprintln!("ZONE   {name:24} {s0:6.0}..{s1:6.0} {look:?} sink {:.0}", r.sink((s0 + s1) * 0.5)); }
        for g in r.gaps() { eprintln!("ZONE   gap {:.0}..{:.0}", g.s0, g.s1); }
        for p in r.pieces() { eprintln!("ZONE   piece {:?} {:.0}..{:.0} side {}", p.kind, p.s0, p.s1, p.side); }
    }
}

/// The sky alone (the dome, the stars, the moon and what stands on the horizon), through the chase lens
/// at each distance `ANGEL_PODRACER_AT` names: where a night's own light and shapes are judged.
#[test]
fn sky_look() {
    let Some(dir) = out_dir() else { return };
    let id = course_id();
    let name = format!("{id:?}").to_lowercase();
    let ticks: u32 = std::env::var("ANGEL_PODRACER_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
    super::route::select(id, ticks);
    let r = super::route::on(id);
    let at: Vec<f32> = std::env::var("ANGEL_PODRACER_AT").map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_else(|_| (0..8).map(|i| r.lap() * 0.1 * i as f32 + 100.).collect());
    for s in at {
        let eye = point_on(r, s - 14., 0., 3.4);
        let view = aim(eye, point_on(r, s + 26., 0., 1.4), 1.25);
        let mut tris = Vec::new();
        super::sky::dome(&mut tris, r, eye, view.heading_rad, r.zone(s + 60.).0, ticks);
        let n = tris.len();
        let mesh = Mesh { tris };
        let look = r.zone(s + 60.).0;
        let (rgba, _) = raster::render_meshes_painted(&[&mesh], &view, 384, 224, Firelight { gain: 1.65, warm: 0.06 }, super::fog_for(look), Backdrop::Haze(super::haze(look)), Some(race_palette()));
        let mut im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
        super::dye(&mut im, rgba.as_raw());
        save(&dir, &format!("{name}_sky_{s:04.0}.png"), &im);
        eprintln!("sky at {s:.0}: {n} triangles, heading {:.2}", view.heading_rad);
    }
}
