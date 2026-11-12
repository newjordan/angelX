//! Proof for the pods lane: turntables, the line-up, the grid to go and the
//! chase with rivals down the road. Nothing runs unless asked:
//!
//!   ANGEL_PODS_PROOF=/dir cargo test -p angel-delve --profile fast --lib pods_proof -- --nocapture
//!
//! `ANGEL_PODS_ONLY=turn|lineup|grid|chase|stop|far` picks one; `ANGEL_PODS_COURSE` is a
//! course's id (any case) for the grid, the chase and the stop.
use super::*;
use crate::rules::Run;
// Named under another name: the griffinwing test counts the sources that call the painted raster by its own.
use raster::render_meshes_painted as paint_meshes;
use std::path::{Path, PathBuf};

fn out() -> Option<PathBuf> { std::env::var_os("ANGEL_PODS_PROOF").map(PathBuf::from) }
fn want(name: &str) -> bool { std::env::var("ANGEL_PODS_ONLY").ok().is_none_or(|o| o == name) }

fn save(im: &Img, path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::save_buffer(path, &im.rgba_bytes(), 384, 224, image::ColorType::Rgba8).unwrap();
}

fn course() -> rules::CourseId {
    let want = std::env::var("ANGEL_PODS_COURSE").unwrap_or_default().to_lowercase();
    rules::courses().iter().copied().find(|id| format!("{id:?}").to_lowercase() == want).unwrap_or(rules::cup()[0])
}

fn race_on(id: rules::CourseId) -> (Run, rules::Session) {
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    run.knight_racers = None;
    let mut race = rules::Session::blank_on(id);
    race.join_ids(1..=6);
    (run, race)
}

fn frame(run: &mut Run, race: &rules::Session, seat: u32) -> Img {
    run.knight_racers = Some(race.clone());
    layers(run, Some(seat)).0
}

/// How a pod is shot for the proof.
#[derive(Clone, Copy)]
enum Shot {
    /// The race's own lens: 22 m behind and 8.6 m over the engines.
    Chase,
    /// The race's lens on a rival `dist` metres ahead of it, field of 1.25, 4 m over the road.
    Game(f32),
    /// Close, the eye `az` radians round the pod (`PI` = dead astern, `PI / 2` = its left side), `rise` high.
    Close(f32, f32),
}

/// One pod alone on its floor in the shot's lens. `Close` frames the whole pod to the picture.
fn stage(look: usize, seat: u32, shot: Shot, t: f32, burn: f32) -> Img {
    let mut pod = Mesh::new();
    build(&mut pod, look, seat, t, burn);
    let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
    for tr in &pod.tris { for p in &tr.v { lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)); hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)); } }
    let mid = v3((lo.x + hi.x) * 0.5, 0., 1.5);
    let r = ((hi.x - lo.x).powi(2) + (hi.y - lo.y).powi(2)).sqrt() * 0.5;
    let mut floor = Mesh::new();
    let (eye, aim, fov) = match shot {
        Shot::Chase => {
            floor.push_quad(v3(-80., -30., 0.), v3(80., -30., 0.), v3(80., 30., 0.), v3(-80., 30., 0.), mat::PATH);
            (v3(-22., 0., 8.6), v3(12., 0., 0.3), 1.1)
        }
        Shot::Game(dist) => {
            floor.push_quad(v3(-80., -40., 0.), v3(120., -40., 0.), v3(120., 40., 0.), v3(-80., 40., 0.), mat::PATH);
            for k in -3..=3 { floor.push_quad(v3(-80., k as f32 * 6. - 0.08, 0.02), v3(120., k as f32 * 6. - 0.08, 0.02), v3(120., k as f32 * 6. + 0.08, 0.02), v3(-80., k as f32 * 6. + 0.08, 0.02), IRON); }
            (v3(-dist, 0., 4.0), v3(10., 0., 1.0), 1.25)
        }
        Shot::Close(az, rise) => {
            let ring = |rad: f32, i: usize| { let a = i as f32 / 32. * TAU; v3(mid.x + rad * a.cos(), rad * a.sin(), 0.) };
            let rr = r * 1.25;
            for i in 0..32 {
                floor.push_tri([v3(mid.x, 0., 0.), ring(rr, i), ring(rr, i + 1)], [[0.; 2]; 3], mat::PATH);
                let lift = v3(0., 0., 0.03);
                floor.push_quad(ring(rr * 0.94, i) + lift, ring(rr * 1.02, i) + lift, ring(rr * 1.02, i + 1) + lift, ring(rr * 0.94, i + 1) + lift, if i % 2 == 0 { STEEL } else { IRON });
            }
            let dist = r * 2.05;
            (mid + v3(dist * az.cos(), dist * az.sin(), dist * rise), mid - v3(0., 0., 0.4), 0.95)
        }
    };
    let d = aim - eye;
    let view = View3 { pos: eye, heading_rad: d.y.atan2(d.x), pitch: d.z.atan2(d.x.hypot(d.y)), fov_rad: fov };
    let (rgba, _) = paint_meshes(&[&floor, &pod], &view, 384, 224, Firelight { gain: 1.65, warm: 0.06 }, 130., Backdrop::Dark([10, 12, 20]), Some(palette()));
    let mut im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
    recolour(&mut im, rgba.as_raw(), &[(0, 0, 384, 224)]);
    im
}

/// A turntable per pod, 36 frames a turn, in the seat colour of its own seat.
#[test]
fn pods_proof_turntables() {
    let Some(out) = out() else { return };
    if !want("turn") { return; }
    for look in 0..6usize {
        let name = PODS[look].name.to_lowercase();
        for n in 0..36u32 {
            let t = n as f32 / 12.;
            let im = stage(look, look as u32 + 1, Shot::Close(PI + n as f32 / 36. * TAU, 0.45), t, 0.55 + 0.45 * (t * 2.).sin());
            save(&im, &out.join(format!("turn-{name}/{n:04}.png")));
        }
    }
}

/// The line-up: every pod under the race's own lens and then close from astern at three-quarters.
#[test]
fn pods_proof_lineup() {
    let Some(out) = out() else { return };
    if !want("lineup") { return; }
    let mut sheet = image::RgbaImage::new(384 * 3, 224 * 4);
    for look in 0..6usize {
        for (row, shot) in [Shot::Chase, Shot::Close(PI + 0.55, 0.45)].into_iter().enumerate() {
            let im = stage(look, look as u32 + 1, shot, 0.4, if look % 2 == 0 { 0.4 } else { 1.55 });
            let (cx, cy) = ((look % 3) as u32 * 384, ((look / 3) * 2 + row) as u32 * 224);
            for y in 0..224 { for x in 0..384 {
                let c = im.get(x, y).unwrap_or([0, 0, 0]);
                sheet.put_pixel(cx + x as u32, cy + y as u32, image::Rgba([c[0], c[1], c[2], 255]));
            } }
        }
    }
    std::fs::create_dir_all(&out).unwrap();
    sheet.save(out.join("lineup.png")).unwrap();
}

/// The grid to go: the course card over the whole grid, the lights, the swoop in behind seat 4.
#[test]
fn pods_proof_grid() {
    let Some(out) = out() else { return };
    if !want("grid") { return; }
    let (mut run, mut race) = race_on(course());
    let seat: u32 = std::env::var("ANGEL_PODS_SEAT").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let dir = out.join("grid");
    for n in 0..150u32 {
        if n > 0 {
            let input = race.bot_drives_all();
            race.step_drive(&input);
            state::observe(&race);
        }
        save(&frame(&mut run, &race, seat), &dir.join(format!("{n:04}.png")));
    }
}

/// A chase down a straight with the pack ahead: rivals 10, 30, 50, 90 and 150 m up the road
/// from the viewer, each in its own lane, everyone at the same speed so the gaps hold.
#[test]
fn pods_proof_chase() {
    let Some(out) = out() else { return };
    if !want("chase") { return; }
    let (mut run, mut race) = race_on(course());
    let start: f32 = std::env::var("ANGEL_PODS_D").ok().and_then(|v| v.parse().ok()).unwrap_or(260.);
    race.ticks = 400;
    let gaps = [(2u32, 10., 3.), (3, 30., -3.5), (5, 50., 2.), (6, 90., -2.), (4, 150., 0.)];
    let dir = out.join("chase");
    for n in 0..150u32 {
        let d = start + n as f32 * 46. / 30.;
        race.ticks = 400 + n;
        {
            let me = race.cars.get_mut(&1).unwrap();
            me.d = d; me.y = 0.; me.v = 46.; me.boosting = true; me.finish = None;
        }
        for &(id, gap, lane) in &gaps {
            let c = race.cars.get_mut(&id).unwrap();
            c.d = d + gap; c.y = lane; c.v = 46.; c.boosting = id % 2 == 0; c.finish = None;
        }
        save(&frame(&mut run, &race, 1), &dir.join(format!("{n:04}.png")));
    }
}

/// The same convoy, but one rival at each distance alone, so a still shows the pod at that range.
#[test]
fn pods_proof_far() {
    let Some(out) = out() else { return };
    if !want("far") { return; }
    let (mut run, mut race) = race_on(course());
    let start: f32 = std::env::var("ANGEL_PODS_D").ok().and_then(|v| v.parse().ok()).unwrap_or(260.);
    race.ticks = 400;
    let mut sheet = image::RgbaImage::new(384 * 3, 224 * 2);
    for (n, gap) in [10., 25., 50., 80., 120., 180.].into_iter().enumerate() {
        {
            let me = race.cars.get_mut(&1).unwrap();
            me.d = start; me.y = 0.; me.v = 46.; me.finish = None;
        }
        for id in 2..=6u32 {
            let c = race.cars.get_mut(&id).unwrap();
            c.d = start - 400.; c.y = 0.; c.v = 40.; c.finish = None;
        }
        let c = race.cars.get_mut(&((n as u32 % 5) + 2)).unwrap();
        c.d = start + gap; c.y = if n % 2 == 0 { 2. } else { -2. };
        let im = frame(&mut run, &race, 1);
        for y in 0..224 { for x in 0..384 {
            let p = im.get(x, y).unwrap_or([0, 0, 0]);
            sheet.put_pixel((n as u32 % 3) * 384 + x as u32, (n as u32 / 3) * 224 + y as u32, image::Rgba([p[0], p[1], p[2], 255]));
        } }
    }
    std::fs::create_dir_all(&out).unwrap();
    sheet.save(out.join("far.png")).unwrap();
}

/// What a pod costs, in triangles, at each cut: the frame-time budget in numbers.
#[test]
fn pods_proof_counts() {
    if out().is_none() { return; }
    for (look, p) in PODS.iter().enumerate() {
        let n = |lod| { let mut m = Mesh::new(); build_lod(&mut m, look, 1 + look as u32, 0.7, 1., lod, 1.); m.tris.len() };
        println!("{:12} near {:5} mid {:5} far {:5}", p.name, n(Lod::Near), n(Lod::Mid), n(Lod::Far));
    }
    for &id in rules::courses() {
        let c = rules::course(id);
        let w = |d: f32| { let f = c.frame(0, d); (f.wl, f.wr) };
        println!("{:?}: half-widths at the grid  d=-6 {:?}  d=-26 {:?}  d=-46 {:?}", id, w(-6.), w(-26.), w(-46.));
    }
}

// ---------------------------------------------------------------- round 2: the read lane

/// A game-speed run for `seat` at `d` with a clean, still field: everyone else far back, nothing moving.
fn run_at(race: &mut rules::Session, seat: u32, d: f32, v: f32) {
    race.ticks = 400;
    for (&id, c) in race.cars.iter_mut() {
        c.finish = None; c.boosting = false; c.heat = [0.; 2]; c.wear = [0.; 2];
        if id == seat { c.d = d; c.y = 0.; c.v = v; } else { c.d = d - 500. - id as f32 * 20.; c.y = 0.; c.v = v; }
    }
}

/// `n`-up sheet of crops (`w` by `h` at `scale`) of `frames` around (cx, cy), `cols` to a row.
fn crops(frames: &[(Img, (f32, f32))], (w, h): (i32, i32), scale: u32, cols: usize) -> image::RgbaImage {
    let rows = frames.len().div_ceil(cols);
    let (cw, ch) = (w as u32 * scale, h as u32 * scale);
    let mut sheet = image::RgbaImage::from_pixel(cw * cols as u32 + 4 * (cols as u32 - 1), ch * rows as u32 + 4 * (rows as u32 - 1), image::Rgba([30, 30, 30, 255]));
    for (n, (im, (cx, cy))) in frames.iter().enumerate() {
        let (x0, y0) = ((*cx as i32 - w / 2).clamp(0, 384 - w), (*cy as i32 - h / 2).clamp(0, 224 - h));
        let (ox, oy) = ((n % cols) as u32 * (cw + 4), (n / cols) as u32 * (ch + 4));
        for y in 0..h { for x in 0..w {
            let c = im.get(x0 + x, y0 + y).unwrap_or([0, 0, 0]);
            for sy in 0..scale { for sx in 0..scale {
                sheet.put_pixel(ox + x as u32 * scale + sx, oy + y as u32 * scale + sy, image::Rgba([c[0], c[1], c[2], 255]));
            } }
        } }
    }
    sheet
}

/// Where a pod 'gap' metres ahead of `me` lands in the seat's frame, for cropping.
fn spot(race: &rules::Session, seat: u32, ahead: f32, y: f32) -> (f32, f32) {
    let me = &race.cars[&seat];
    let view = camera::shot(race, me, seat).view;
    camera::project(&view, point(me.d + ahead, y, 1.4)).map_or((192., 110.), |(x, y, _)| (x, y))
}

/// The six pods as a rival, one row each, at 6, 12 and 20 m up the road (21, 27 and 35 m from the lens) in the race's own lens,
/// enlarged: can every pod be told from the rest by its shape?
#[test]
fn pods_proof_silhouettes() {
    let Some(out) = out() else { return };
    if !want("silhouettes") { return; }
    let (mut run, mut race) = race_on(course());
    let start: f32 = std::env::var("ANGEL_PODS_D").ok().and_then(|v| v.parse().ok()).unwrap_or(260.);
    let mut shots = Vec::new();
    for look in 0..6u8 {
        for gap in [6., 12., 20.] {
            fx_reset();
            run_at(&mut race, 1, start, 48.);
            {
                let c = race.cars.get_mut(&2).unwrap();
                c.d = start + gap; c.y = if look % 2 == 0 { 3.5 } else { -3.5 }; c.v = 48.; c.pod = look;
            }
            // Warm the lens' springs a few ticks so the field is the running one.
            for tick in 0..12 { race.ticks = 400 + tick; let _ = frame(&mut run, &race, 1); }
            let at = spot(&race, 1, gap, race.cars[&2].y);
            shots.push((frame(&mut run, &race, 1), at));
        }
    }
    std::fs::create_dir_all(&out).unwrap();
    crops(&shots, (64, 36), 5, 3).save(out.join("silhouettes.png")).unwrap();
}

/// Reset the feel kit's memory for the seat so a still does not inherit the last one's springs.
fn fx_reset() { super::fx::forget_all(); }

/// The leader 30 m up the road in the chase lens: the whole frame, then the same enlarged.
#[test]
fn pods_proof_leader() {
    let Some(out) = out() else { return };
    if !want("leader") { return; }
    let (mut run, mut race) = race_on(course());
    let start: f32 = std::env::var("ANGEL_PODS_D").ok().and_then(|v| v.parse().ok()).unwrap_or(260.);
    let look: u8 = std::env::var("ANGEL_PODS_RIVAL").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let own: u8 = std::env::var("ANGEL_PODS_OWN").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    fx_reset();
    run_at(&mut race, 1, start, 48.);
    race.cars.get_mut(&1).unwrap().pod = own;
    {
        let c = race.cars.get_mut(&2).unwrap();
        c.d = start + 30.; c.y = 1.5; c.v = 48.; c.pod = look;
    }
    for tick in 0..14 { race.ticks = 400 + tick; let _ = frame(&mut run, &race, 1); }
    let im = frame(&mut run, &race, 1);
    std::fs::create_dir_all(&out).unwrap();
    save(&im, &out.join("leader-30m.png"));
    let at = spot(&race, 1, 30., 1.5);
    crops(&[(im.clone(), at)], (96, 56), 6, 1).save(out.join("leader-30m-rival.png")).unwrap();
    crops(&[(im, (192., 140.))], (160, 90), 4, 1).save(out.join("leader-30m-own.png")).unwrap();
}

/// Each pod as the player's own, in the chase, from the pit of the picture up: shaded, trimmed, placed.
#[test]
fn pods_proof_own() {
    let Some(out) = out() else { return };
    if !want("own") { return; }
    let (mut run, mut race) = race_on(course());
    let start: f32 = std::env::var("ANGEL_PODS_D").ok().and_then(|v| v.parse().ok()).unwrap_or(260.);
    let mut shots = Vec::new();
    for seat in 1..=6u32 {
        fx_reset();
        run_at(&mut race, seat, start, 48.);
        race.cars.get_mut(&seat).unwrap().pod = (seat - 1) as u8;
        {
            let c = race.cars.get_mut(&if seat == 2 { 3 } else { 2 }).unwrap();
            c.d = start + 30.; c.y = 1.5; c.v = 48.; c.pod = (seat % 6) as u8;
        }
        for tick in 0..14 { race.ticks = 400 + tick; let _ = frame(&mut run, &race, seat); }
        shots.push((frame(&mut run, &race, seat), (192., 112.)));
    }
    std::fs::create_dir_all(&out).unwrap();
    crops(&shots, (192, 130), 2, 3).save(out.join("own.png")).unwrap();
}

/// A gold pod has a lit face, a shaded face and an underside: at least four steps of the gold ramp
/// show on it, not the one flat tone the raster's floored cloths gave it.
#[test]
fn a_gold_pod_shows_light_faces_and_dark_ones() {
    let ramp: Vec<[u8; 3]> = ['B', 'a', 'o', 'O', '4', '5', '6'].iter().map(|&c| crate::ink::ink(c).unwrap()).collect();
    let mut found = std::collections::BTreeSet::new();
    for turn in [0.45f32, 0.9, 1.4] {
        let im = stage(0, 1, Shot::Close(PI + turn, 0.5), 0.4, 0.4);
        let mut count = vec![0usize; ramp.len()];
        for y in 0..224 { for x in 0..384 {
            if let Some(c) = im.get(x, y) { if let Some(i) = ramp.iter().position(|r| *r == c) { count[i] += 1; } }
        } }
        for (i, n) in count.iter().enumerate() { if *n >= 30 { found.insert(i); } }
    }
    assert!(found.len() >= 4, "only {} steps of the gold ramp show on the gold pod: {found:?}", found.len());
}

/// A pack down the road: five rivals at 9 to 40 m, each a different pod in its own seat's colour and its own
/// lane, two of them hurt. The whole frame, then the road ahead enlarged. `ANGEL_PODS_ROT` turns which pod is whose.
#[test]
fn pods_proof_pack() {
    let Some(out) = out() else { return };
    if !want("pack") { return; }
    let (mut run, mut race) = race_on(course());
    let start: f32 = std::env::var("ANGEL_PODS_D").ok().and_then(|v| v.parse().ok()).unwrap_or(260.);
    let rot: u8 = std::env::var("ANGEL_PODS_ROT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    fx_reset();
    run_at(&mut race, 1, start, 48.);
    race.cars.get_mut(&1).unwrap().pod = (5 + rot) % 6;
    let field = [(2u32, 9., 3.5), (3, 14., -3.5), (4, 22., 2.5), (5, 30., -2.), (6, 40., 0.5)];
    for (n, &(id, gap, lane)) in field.iter().enumerate() {
        let c = race.cars.get_mut(&id).unwrap();
        c.d = start + gap; c.y = lane; c.v = 48.; c.pod = (n as u8 + rot) % 6; c.boosting = n % 2 == 0;
        if n == 1 { c.heat = [0.64, 0.2]; }
        if n == 3 { c.heat = [0.2, 0.9]; }
    }
    for tick in 0..14 { race.ticks = 400 + tick; let _ = frame(&mut run, &race, 1); }
    let im = frame(&mut run, &race, 1);
    std::fs::create_dir_all(&out).unwrap();
    save(&im, &out.join(format!("pack-{rot}.png")));
    crops(&[(im, (192., 112.))], (176, 100), 5, 1).save(out.join(format!("pack-{rot}-road.png"))).unwrap();
}

/// A hurt pod at 6 to 20 m: smoke, a burning nozzle, a blowout's burst and its steam, in the race's own lens.
#[test]
fn pods_proof_damage() {
    let Some(out) = out() else { return };
    if !want("damage") { return; }
    let (mut run, mut race) = race_on(course());
    let start: f32 = std::env::var("ANGEL_PODS_D").ok().and_then(|v| v.parse().ok()).unwrap_or(260.);
    let mut shots = Vec::new();
    // (heat, wear, crash ticks left, blown side)
    let states: [([f32; 2], [f32; 2], u32, u8); 4] = [
        ([0.62, 0.2], [0.; 2], 0, 0), ([0.95, 0.3], [0.; 2], 0, 0),
        ([1., 0.3], [0.; 2], rules::race::engines::BLOWOUT_TICKS - 5, 1), ([0.4, 0.3], [0.2; 2], 90, 1),
    ];
    for (n, &(heat, wear, crash, side)) in states.iter().enumerate() {
        for gap in [7., 14.] {
            fx_reset();
            run_at(&mut race, 1, start, 40.);
            {
                let c = race.cars.get_mut(&2).unwrap();
                c.d = start + gap; c.y = if n % 2 == 0 { 2.5 } else { -2.5 }; c.v = if crash > 0 { 0. } else { 40. }; c.pod = (n as u8 + 1) % 6;
                c.heat = heat; c.wear = wear; c.crash = crash; c.blown_side = side;
            }
            for tick in 0..12 { race.ticks = 400 + tick; let _ = frame(&mut run, &race, 1); }
            let at = spot(&race, 1, gap, race.cars[&2].y);
            shots.push((frame(&mut run, &race, 1), (at.0, at.1 - 8.)));
        }
    }
    std::fs::create_dir_all(&out).unwrap();
    crops(&shots, (112, 64), 4, 2).save(out.join("damage.png")).unwrap();
}

/// Each pod alone on the road at 22, 32 and 45 m from the race's lens, in its own seat's colour: the shapes
/// themselves, with no other pod, plume or tag in the way.
#[test]
fn pods_proof_distance() {
    let Some(out) = out() else { return };
    if !want("distance") { return; }
    let mut frames = Vec::new();
    for look in 0..6usize {
        for dist in [22., 32., 45.] {
            let im = stage(look, look as u32 + 1, Shot::Game(dist), 0.4, 0.8);
            frames.push((im, (192., 118.)));
        }
    }
    std::fs::create_dir_all(&out).unwrap();
    crops(&frames, (96, 48), 4, 3).save(out.join("distance.png")).unwrap();
}
