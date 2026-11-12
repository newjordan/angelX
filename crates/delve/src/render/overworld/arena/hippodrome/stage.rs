//! The race on its own screen: the course's country under a camera that rides the viewer's own
//! team, the field with its dust and sparks and whips, the crowd that roars as the lead changes
//! hands, the line and its paper, and over them the plates, the calls and the count. A seat that
//! drives sees from its own team; a seat that only watches rides the leader.
use super::super::super::ink::{Img, bayer, hash};
use super::super::class_looks::{self, Look};
use super::super::juice;
use super::super::knights::Facing;
use super::{art, finish, fx, hud, moments, view::View, world};
use crate::rules::chariot::{Phase, heading_on, pos_on, tangent_on, show::Kind};
use crate::rules::juice::Weight;

/// A blow a notch lighter: what a rival's trouble does to your screen.
fn lighter(w: Weight) -> Weight {
    match w {
        Weight::Finale => Weight::Kill,
        Weight::Kill => Weight::Crushing,
        Weight::Crushing => Weight::Solid,
        _ => Weight::Graze,
    }
}

/// The world's shake for this seat: its own barges and wrecks in full, a rival's a notch lighter,
/// the first team home and the photograph for all. The picture keeps its phase while the room is
/// held, so a freeze still trembles.
pub(super) fn shake(v: &View) -> (i32, i32) {
    let now = v.now;
    let phase = u64::from(now) * 7 + u64::from(v.run.hitstop.left) * 3;
    let blows = v.race.show.fx.iter().filter_map(|f| {
        let mine = v.is_me(f.team) || v.is_me(f.other);
        let weight = match f.kind {
            Kind::Wreck | Kind::Barge => f.weight,
            Kind::Finish if f.n == 1 => Weight::Crushing,
            Kind::Photo => Weight::Crushing,
            _ => return None,
        };
        let seen = if mine || matches!(f.kind, Kind::Finish | Kind::Photo) { weight } else { lighter(lighter(weight)) };
        (seen > Weight::Graze).then_some((seen, f.age(now)))
    });
    juice::shake(blows, phase)
}

/// One team as it stands in this frame.
struct Pose {
    i: usize,
    at: (i32, i32),
    tangent: (f32, f32),
    view: art::View,
    flip: bool,
}

fn pose(v: &View, i: usize, cam: (i32, i32)) -> Pose {
    let (x, y) = v.map_px(i);
    let drawn = v.drawn(i);
    let (tx, ty) = tangent_on(v.course(), drawn);
    let h = heading_on(v.course(), drawn);
    // Head-on or from behind only where the road runs nearly straight down or up the screen.
    let view = if ty.abs() > tx.abs() * 1.6 { if ty > 0.0 { art::View::Head } else { art::View::Rear } } else { art::View::Side };
    let _ = h;
    Pose { i, at: (x as i32 - cam.0, y as i32 - cam.1), tangent: (tx, ty), view, flip: view == art::View::Side && tx < 0.0 }
}

/// Where the feet stand on a picture of `view`: the sprite's anchor.
fn anchor(view: art::View) -> (i32, i32) {
    match view {
        art::View::Side => (32, 35),
        art::View::Head => (20, 42),
        art::View::Rear => (20, 40),
    }
}

/// The dithered shadow under a team.
fn shadow(layer: &mut Img, (x, y): (i32, i32), view: art::View) {
    let (rx, ry) = if view == art::View::Side { (24.0, 4.5) } else { (15.0, 4.0) };
    for dy in -5..=5 {
        for dx in -26..=26 {
            let k = (dx * dx) as f32 / (rx * rx) + (dy * dy) as f32 / (ry * ry);
            if k <= 1.0 && bayer(x + dx, y + dy) < 0.78 - k * 0.3 {
                layer.put(x + dx, y + 2 + dy, 'k');
            }
        }
    }
}

/// A team's picture now: the sprite, dressed with the driver's class, struck white, squashed.
fn sprite_of(v: &View, p: &Pose) -> Img {
    let race = v.race;
    let r = &race.racers[p.i];
    let l = art::livery(&r.house);
    let frame = (v.drawn(p.i) / 5.0).max(0.0) as u32;
    let lashed = r.whipped && r.wrecked == 0 && race.phase == Phase::Racing;
    let mut im = art::team(l, r.team, frame, p.view, lashed);
    if p.flip {
        im = im.flip_h();
    }
    if let Some((seat, look)) = v.seat_of(r.team).and_then(|s| v.run.players.get(&s).and_then(Look::of).map(|l| (s, l))) {
        let facing = match p.view {
            art::View::Head => Facing::Down,
            art::View::Rear => Facing::Up,
            art::View::Side => Facing::Side,
        };
        class_looks::dress_driver(&mut im, art::driver_at(p.view), facing, p.flip, look, seat);
    }
    // Struck: the blow's own flash, then its squash and stretch.
    let now = v.now;
    let blow = race
        .show
        .fx
        .iter()
        .filter(|f| matches!(f.kind, Kind::Barge | Kind::Wreck) && (f.team == r.team || f.other == r.team) && f.weight >= Weight::Solid)
        .filter(|f| f.age(now) < 16)
        .max_by_key(|f| (f.weight, std::cmp::Reverse(f.age(now))));
    if let Some(f) = blow {
        let age = f.age(now);
        let loser = if f.kind == Kind::Barge { f.other == r.team } else { f.team == r.team };
        if loser && age < f.weight.flash() {
            im = juice::impact_frame(&im, 'H');
        } else if loser {
            im = juice::squash_stretch(&im, age, f.weight.squash()).0;
        }
    }
    im
}

/// The bunting over the line, strung between two posts.
fn gantry(layer: &mut Img, w: &world::World, cam: (i32, i32), now: u32, cloths: &[char]) {
    let (lx, ly, tx, ty) = w.line;
    let (nx, ny) = (-ty, tx);
    let ends: Vec<(f32, f32)> = [-1.0f32, 1.0].iter().map(|s| (lx + nx * 36.0 * s - cam.0 as f32, ly + ny * 36.0 * s - cam.1 as f32)).collect();
    let (a, b) = (ends[0], ends[1]);
    if (a.0 < -60.0 && b.0 < -60.0) || (a.0 > 450.0 && b.0 > 450.0) || (a.1 < -80.0 && b.1 < -80.0) || (a.1 > 300.0 && b.1 > 300.0) {
        return;
    }
    for &(x, y) in &[a, b] {
        layer.rect(x as i32 - 1, y as i32 - 26, 3, 27, 'B');
        layer.rect(x as i32 - 1, y as i32 - 26, 1, 27, 'R');
        layer.rect(x as i32 - 2, y as i32 - 28, 5, 3, 'O');
    }
    let n = 14;
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let sag = (t * std::f32::consts::PI).sin() * 6.0;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t - 26.0 + sag);
        layer.put(x as i32, y as i32, 'j');
        if k % 2 == 0 && k > 0 && k < n {
            let sway = ((now / 5 + k as u32) % 3) as i32;
            let cloth = cloths[k as usize % cloths.len()];
            for row in 0..5 {
                let half = (5 - row) / 2;
                layer.rect(x as i32 - half + if row > 1 { sway - 1 } else { 0 }, y as i32 + 1 + row, half * 2 + 1, 1, cloth);
            }
        }
    }
}

/// The gate's bar across the road at the line: down while the field is held, lifted at the off.
fn gate_bar(layer: &mut Img, w: &world::World, cam: (i32, i32), open: f32) {
    let (lx, ly, tx, ty) = w.line;
    let (nx, ny) = (-ty, tx);
    let reach = 28.0 * (1.0 - open);
    if reach < 1.0 {
        return;
    }
    let mut u = -reach;
    while u <= reach {
        for v in [10.0f32, 11.0] {
            let (x, y) = (lx + nx * u + tx * v - cam.0 as f32, ly + ny * u + ty * v - cam.1 as f32);
            layer.rect(x as i32, y as i32 - 5, 2, 2, if ((u + 40.0) / 5.0) as i32 % 2 == 0 { '7' } else { 'T' });
        }
        u += 1.0;
    }
}

/// Dropped iron waits where every driver can see it.
fn irons(layer: &mut Img, v: &View, cam: (i32, i32)) {
    for &(at, lane) in &v.race.traps {
        let (tx, ty) = pos_on(v.course(), at, lane);
        let (sx, sy) = ((tx * super::super::TILE as f32) as i32 - cam.0, (ty * super::super::TILE as f32) as i32 - cam.1);
        if sx < -6 || sy < -6 || sx > layer.w + 6 || sy > layer.h + 6 {
            continue;
        }
        for (dx, dy) in [(0, -3), (-3, 0), (3, 0), (0, 3), (-2, -2), (2, -2), (-2, 2), (2, 2)] {
            layer.put(sx + dx, sy + dy, 'k');
        }
        for (dx, dy) in [(0, -2), (-2, 0), (2, 0), (0, 2), (0, 0)] {
            layer.put(sx + dx, sy + dy, 'h');
        }
        if (v.now / 5 + sx as u32) % 4 == 0 {
            layer.put(sx, sy - 1, 'H');
        }
    }
}

/// How hot the racing is: the share of the field within a few lengths of another.
fn heat(v: &View) -> f32 {
    let rs = &v.race.racers;
    let mut close = 0;
    for i in 0..rs.len() {
        for j in i + 1..rs.len() {
            if (rs[i].dist - rs[j].dist).abs() < 36.0 && (rs[i].lane - rs[j].lane).abs() < 0.9 {
                close += 1;
            }
        }
    }
    (close as f32 / 3.0).min(1.0)
}

/// The race on `scene` (the world, under any shake) and `over` (the plates and words).
pub(super) fn frame(scene: &mut Img, over: &mut Img, run: &crate::rules::Run, w: i32, h: i32, focus: Option<u32>) {
    let Some(v) = View::new(run, focus, (w, h)) else { return };
    let race = v.race;
    if let Some(age) = finish::ended(&v).filter(|&a| a >= finish::OPENS) {
        let img = finish::scene(&v, age - finish::OPENS);
        scene.stamp(&img, 0, 0);
        return;
    }
    let course = v.course();
    let wd = world::world(race.track);
    let cam = v.camera();
    // The baked country is laid out with a margin of black paper round it.
    let wcam = (cam.0 + world::PAD, cam.1 + world::PAD);
    let now = v.now;
    let mut layer = world::window(wd, wcam.0, wcam.1, w, h);
    let cloths: Vec<char> = race.racers.iter().map(|r| art::livery(&r.house).cloth).collect();
    // The crowd: a roar runs round the stands from wherever the lead changed hands.
    let roar = race
        .show
        .fx
        .iter()
        .filter(|f| matches!(f.kind, Kind::Lead | Kind::Photo) || (f.kind == Kind::Finish && f.n == 1))
        .map(|f| (f.at[0].rem_euclid(course.lap), f.age(now)))
        .filter(|&(_, age)| age < 70)
        .min_by_key(|&(_, age)| age);
    let mood = world::Mood { roar, heat: heat(&v), lap: course.lap };
    world::draw_posts(&mut layer, wd, wcam, now, &cloths);
    world::draw_crowd(&mut layer, wd, wcam, now, &mood, &cloths);
    if race.phase == Phase::Gate || race.t < 12 {
        let open = if race.phase == Phase::Gate { 0.0 } else { race.t as f32 / 12.0 };
        gate_bar(&mut layer, wd, wcam, open);
    }
    irons(&mut layer, &v, cam);
    // Everyone in the picture, back to front: the dust first, so the field runs in front of it.
    let mut poses: Vec<Pose> = (0..race.racers.len()).map(|i| pose(&v, i, cam)).collect();
    poses.retain(|p| p.at.0 > -70 && p.at.0 < w + 70 && p.at.1 > -50 && p.at.1 < h + 70);
    poses.sort_by_key(|p| p.at.1);
    for p in &poses {
        let r = &race.racers[p.i];
        if race.phase == Phase::Racing || r.place.is_some() {
            let lash = if r.whipped && r.wrecked == 0 { 1.0 } else { 0.0 };
            let pace = if r.wrecked > 0 { r.speed.max(2.0) * 0.7 } else { r.speed };
            fx::plume(&mut layer, course, cam, v.drawn(p.i), r.lane, pace, lash, u32::from(r.team) * 31 + 7, now);
        }
    }
    for p in &poses {
        let r = &race.racers[p.i];
        let (_ax, ay) = anchor(p.view);
        let mut at = p.at;
        // A blown team wobbles; a locked one shudders.
        if r.wind < 0.25 && race.phase == Phase::Racing && r.place.is_none() {
            at.1 += ((now / 3) % 2) as i32 - 1;
        }
        if r.jolt > 6 || r.wrecked > 0 {
            at.0 += if (now / 2) % 2 == 0 { 1 } else { -1 };
        }
        shadow(&mut layer, at, p.view);
        let im = sprite_of(&v, p);
        // The feet stay where the base picture's did, whatever the squash made of its canvas.
        let h0 = if p.view == art::View::Side { 38 } else { 44 };
        let bottom = at.1 - ay + 1 + h0;
        layer.stamp(&im, at.0 - im.w / 2, bottom - im.h);
        // The marker over the viewer's own team, bobbing.
        if v.mine && p.i == v.me && race.phase != Phase::Done {
            let bob = ((now / 6) % 2) as i32;
            let (mx, my) = (at.0, at.1 - ay - 8 + bob);
            let ink = art::banner_ink(&r.house);
            layer.rect(mx - 3, my, 7, 1, 'k');
            layer.rect(mx - 2, my - 1, 5, 1, ink);
            layer.rect(mx - 1, my - 2, 3, 1, ink);
            layer.rect(mx, my - 3, 1, 1, ink);
        }
        // Sparks off a wheel that is working hard.
        let cornering = r.cornering(course);
        if race.phase == Phase::Racing && r.place.is_none() && r.speed > 5.0 && (r.wrecked > 0 || (cornering && r.speed > 6.5)) {
            let (bx, by) = (-p.tangent.0, -p.tangent.1);
            let wheel = (at.0 as f32 + bx * 18.0, at.1 as f32 - 4.0 + by * 6.0);
            fx::sparks(&mut layer, wheel, (bx, by), if r.wrecked > 0 { 3 } else { 2 }, u32::from(r.team) * 17 + 3, now);
        }
        // The whip's crack at the horses' heads.
        if let Some(age) = race.show.since(Kind::Whip, r.team, now).filter(|&a| a < 12) {
            let reach = if p.view == art::View::Side { 30.0 } else { 6.0 };
            let lift = if p.view == art::View::Side { 22.0 } else { 34.0 };
            let from = (at.0 as f32 - p.tangent.0 * 10.0, at.1 as f32 - lift);
            let to = (at.0 as f32 + p.tangent.0 * reach + p.tangent.0.signum() * 6.0, at.1 as f32 - lift - 4.0 + p.tangent.1 * 12.0);
            fx::crack(&mut layer, from, to, age, u32::from(r.team) * 13 + 1, now);
        }
    }
    // Barges and wrecks, where they happened.
    for f in race.show.fx.iter() {
        let age = f.age(now);
        let (tx, ty) = pos_on(course, f.at[0], f.at[1]);
        let at = (tx * super::super::TILE as f32 - cam.0 as f32, ty * super::super::TILE as f32 - cam.1 as f32 - 8.0);
        if at.0 < -40.0 || at.1 < -40.0 || at.0 > w as f32 + 40.0 || at.1 > h as f32 + 40.0 {
            continue;
        }
        let seed = hash(f.born as i32, i32::from(f.team), 0x41);
        match f.kind {
            Kind::Barge if age < 32 && f.weight >= Weight::Solid => fx::barge(&mut layer, at, age, seed, f.weight, now),
            Kind::Barge if age < 14 => fx::barge(&mut layer, at, age, seed, Weight::Graze, now),
            Kind::Wreck if age < 44 => fx::wreck(&mut layer, at, age, seed, f.weight, now),
            Kind::Iron if age < 8 => {
                let mut pen = super::super::pfx::Pen::new(&mut layer, 80);
                pen.shock((at.0, at.1 + 8.0), 2.0 + age as f32 * 1.5, 1.0, 0.6, super::super::pfx::ramps::SMOKE, age as f32 / 8.0);
            }
            _ => {}
        }
    }
    gantry(&mut layer, wd, wcam, now, &cloths);
    // The paper at the line for the first team home.
    if let Some(f) = race.show.fx.iter().find(|f| f.kind == Kind::Finish && f.n == 1) {
        let (lx, ly, ..) = wd.line;
        finish::paper(&mut layer, (lx as i32 - wcam.0, ly as i32 - wcam.1 - 20), f.age(now));
    }
    // Wind past the lens: the faster the viewer rides, the more of it, and a rush at the off.
    let me = &race.racers[v.me];
    let pace = if race.phase == Phase::Racing { me.speed / 11.0 } else { 0.0 };
    let surge = if race.phase == Phase::Racing && race.t < 26 { 1.0 - race.t as f32 / 26.0 } else { 0.0 };
    let dir = tangent_on(course, v.drawn(v.me));
    fx::wind(&mut layer, dir, pace.clamp(0.0, 1.0), surge, now);
    let (dx, dy) = shake(&v);
    scene.stamp(&layer, dx, dy);

    // Over it: the plates, the calls, the count, the iris.
    let plan = moments::plan_of(&v);
    hud::draw(over, &v, &plan);
    if let Some(b) = &plan.banner {
        moments::draw_banner(over, b);
    }
    moments::draw_start(over, &plan, &v);
    // The photograph's flash, for everyone.
    if race.show.fx.iter().any(|f| f.kind == Kind::Photo && f.age(now) < 2) {
        for y in 0..over.h {
            for x in 0..over.w {
                if bayer(x, y) < 0.16 {
                    over.put(x, y, 'v');
                }
            }
        }
    }
    if let Some(age) = finish::ended(&v) {
        finish::iris(over, age);
    }
}
