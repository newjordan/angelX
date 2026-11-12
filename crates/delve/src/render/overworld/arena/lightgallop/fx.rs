//! The weather of speed and the moments of the road, drawn in the world:
//! wind that streams along the way a rider goes, hoof dust left on the road,
//! after-images at pace, the lance-lock's reticle, and every record the
//! session keeps (`rules::lightgallop::show`): a spark taken, a foe popped, a
//! blow, a loop closed, a lantern lit, the warden down.
//!
//! Everything is a pure function of the session and its tick count: a
//! rider's past is read back off the road it stands on (`feet_at`), never
//! kept. The realm's inks, an ordered dither, no glow that dims a figure.
use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::{CLOTH, View, anim_at, painted, stamp_scaled, stamp_turned};
use crate::rules::lightgallop::{math::Q, terrain::Kind as Road};
use super::{Img, bayer, hash};
use crate::rules::juice::Weight;
use crate::rules::lightgallop::{
    Session,
    level::level,
    rider::Rider,
    show::Kind,
    world::{foe_at, foe_body},
};

/// Where a rider's feet were `ago` ticks back, 1/256 pixel: along the road it
/// stands on at the pace it keeps, or along its flight.
pub(super) fn feet_at(r: &Rider, ago: i32) -> (i32, i32) {
    match r.ground {
        Some(g) => {
            let path = &level().terrain.paths[g.path as usize];
            let s = (g.s - r.gsp * ago).clamp(0, path.len());
            let p = path.sample(s);
            (p.x, p.y)
        }
        None => (r.x - r.vx * ago, r.y - r.vy * ago),
    }
}

/// A disc of radius `rad` about `(cx, cy)` in `ink`, through an ordered
/// dither: `k` 0 is nothing, 1 is solid.
pub(super) fn disc(im: &mut Img, cx: i32, cy: i32, rad: i32, ink: char, k: f32) {
    for dy in -rad..=rad {
        for dx in -rad..=rad {
            if dx * dx + dy * dy <= rad * rad + rad / 2 && bayer(cx + dx, cy + dy) < k {
                im.put(cx + dx, cy + dy, ink);
            }
        }
    }
}

/// The way a rider is going, a 12-bit unit vector on the screen.
fn heading(r: &Rider) -> (i32, i32) {
    match r.ground {
        Some(g) => {
            let t = level().terrain.paths[g.path as usize].sample(g.s);
            let sg = r.gsp.signum();
            (t.tx * sg, t.ty * sg)
        }
        None => {
            let m = i64::from(r.speed().max(1));
            ((i64::from(r.vx) * 4096 / m) as i32, (i64::from(r.vy) * 4096 / m) as i32)
        }
    }
}

/// Wind streaming past the viewer's own rider along the way it goes (down a
/// slope, round a loop), more of it and longer the faster it rides. `front`
/// is the short, bright few that cross in front of the horse.
pub(super) fn lines(im: &mut Img, s: &Session, v: &View, seat: u32, r: &Rider, front: bool, keep_clear: (i32, i32)) {
    let sp = r.speed();
    if r.down > 0 || sp < 800 {
        return;
    }
    let k = ((sp - 800) as f32 / 1300.0).clamp(0.0, 1.0);
    let (ux, uy) = heading(r);
    // How steeply it runs downhill: the wind leans into a drop.
    let slope = (uy * ux.signum()).max(0) as f32 / 4096.0;
    let n = if front { 3 + (7.0 * k) as i32 } else { 5 + (15.0 * k + slope * 8.0) as i32 };
    let per_tick = (i64::from(sp) * i64::from(v.zoom) / 65536) as i32;
    let len0 = if front { 12 } else { 18 } + (k * 26.0) as i32 + (slope * 14.0) as i32;
    let (cx, cy) = (192, 126);
    let (px, py) = (-uy, ux);
    for i in 0..n {
        let h = hash(i, seat as i32 + 31 * i32::from(front), 0x4c) as i32;
        let lane = h.rem_euclid(260) - 130;
        let span = 520;
        let rate = 2 + per_tick * if front { 5 } else { 3 } / 2;
        let along = span / 2 - (h.rem_euclid(span) + s.elapsed as i32 * rate).rem_euclid(span);
        let len = len0 + (h >> 8).rem_euclid(12);
        let head = (cx + (ux * along + px * lane) / 4096, cy + (uy * along + py * lane) / 4096);
        for j in len / 2..len {
            let (x, y) = (head.0 + ux * j / 4096, head.1 + uy * j / 4096);
            if !(31..207).contains(&y) || (x - keep_clear.0).abs() < 16 && (y - keep_clear.1).abs() < 16 {
                continue;
            }
            // A solid head that thins to a dither at the tail.
            let fade = 2.0 * (1.0 - j as f32 / len as f32);
            if bayer(x, y) < fade * (0.7 + 0.3 * k) {
                let ink = match (front, j < len / 3) {
                    (true, true) => 'V',
                    (true, false) => 'v',
                    (false, true) => 'v',
                    (false, false) => 'u',
                };
                im.put(x, y, ink);
            }
        }
        // The head is solid: one run of the line primitive, kept off the heads-up display and the horse.
        let end = (head.0 + ux * (len / 2) / 4096, head.1 + uy * (len / 2) / 4096);
        let near = |p: (i32, i32)| (p.0 - keep_clear.0).abs() < 16 && (p.1 - keep_clear.1).abs() < 16;
        if (31..207).contains(&head.1) && (31..207).contains(&end.1) && !near(head) && !near(end) {
            im.line(head.0, head.1, end.0, end.1, if front { 'V' } else { 'v' });
        }
    }
}

/// Hoof dust thrown up along the road a rider has just galloped, bigger and
/// longer with pace; a skid throws it pale and heavy.
pub(super) fn dust(im: &mut Img, s: &Session, v: &View, seat: u32, r: &Rider) {
    if r.down > 0 || !r.grounded() || r.speed() < 500 {
        return;
    }
    let k = ((r.speed() - 500) as f32 / 1400.0).clamp(0.2, 1.0);
    let n = 3 + (7.0 * k) as i32 + if r.skid { 4 } else { 0 };
    for j in 0..n {
        let (fx, fy) = feet_at(r, 1 + j * 2);
        let (sx, sy) = (v.qx(fx), v.qy(fy));
        let t = j as f32 / n as f32;
        let h = hash(j, s.elapsed as i32 / 2, seat + 9) as i32;
        let rad = (1.0 + t * 2.4 * k * v.zoom as f32 / 256.0 + if r.skid { 1.0 } else { 0.0 }) as i32;
        let (x, y) = (sx - r.dir * (j / 2) + h.rem_euclid(3) - 1, sy - j / 2 - h.rem_euclid(3) - rad / 2);
        let ink = if r.skid { 'o' } else if j < 3 { 'r' } else { 'P' };
        disc(im, x, y, rad, ink, 0.75 - 0.6 * t);
    }
}

/// After-images at pace: the horse's own shape left hanging in the seat's
/// colour where it was a few ticks ago, thinned through the dither.
pub(super) fn ghosts(im: &mut Img, v: &View, seat: u32, r: &Rider, sprite: &Img, ang: i32, flip: bool) {
    let sp = r.speed();
    if r.down > 0 || sp < 1700 || r.roll || r.ball {
        return;
    }
    let t = &level().terrain;
    let c = r.centre(t);
    let (off_x, off_y) = (c.0 - r.x, c.1 - r.y);
    let ink = CLOTH[(seat.max(1) as usize - 1) % 6][2];
    let hot = if sp >= 2200 { 0.1 } else { 0.0 };
    // The horse's opaque pixels as offsets from its centre: straight off the sprite while it
    // runs near level, through the turn only when it leans (a loop, a steep slope).
    let mut offs: Vec<(i32, i32)> = Vec::new();
    if ang.abs() < 60 || (1024 - ang).abs() < 60 {
        for y in 0..sprite.h {
            for x in 0..sprite.w {
                if sprite.get(x, y).is_some() {
                    let dx = if flip { 23 - x } else { x - 24 };
                    offs.push((dx * v.zoom / 256, (y - 24) * v.zoom / 256));
                }
            }
        }
    } else {
        let mut shape = Img::new(72, 72);
        stamp_turned(&mut shape, sprite, (36, 36), (24, 24), ang, v.zoom, flip);
        for y in 0..72 {
            for x in 0..72 {
                if shape.get(x, y).is_some() {
                    offs.push((x - 36, y - 36));
                }
            }
        }
    }
    for (ago, k) in [(4, 0.36 + hot), (8, 0.2 + hot)] {
        let (fx, fy) = feet_at(r, ago);
        let (sx, sy) = (v.qx(fx + off_x), v.qy(fy + off_y));
        for &(dx, dy) in &offs {
            if bayer(sx + dx, sy + dy) < k {
                im.put(sx + dx, sy + dy, ink);
            }
        }
    }
}

/// The lance-lock's reticle: four brackets that close on the foe it is after,
/// and a thin dithered line of light from the horse to it.
pub(super) fn lock_on(im: &mut Img, s: &Session, v: &View, r: &Rider, from: (i32, i32)) {
    let Some(lock) = r.lock else { return };
    let lvl = level();
    let Some(spot) = lvl.foes.get(usize::from(lock.target)) else { return };
    let (fx, fy, _) = foe_at(lvl, spot, s.elapsed);
    let (_, fh) = foe_body(spot.kind);
    let (tx, ty) = (v.sx(fx), v.sy(fy - fh / 2));
    let steps = ((tx - from.0).abs().max((ty - from.1).abs())).max(1);
    for i in (0..steps).step_by(2) {
        let (x, y) = (from.0 + (tx - from.0) * i / steps, from.1 + (ty - from.1) * i / steps);
        if bayer(x, y) < 0.7 {
            im.put(x, y, 'T');
        }
    }
    let reach = 8 + i32::from(lock.ticks) / 2;
    for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        for o in 0..4 {
            im.put(tx + dx * reach - dx * o, ty + dy * reach, 'k');
            im.put(tx + dx * reach, ty + dy * reach - dy * o, 'k');
        }
        for o in 0..3 {
            im.put(tx + dx * reach - dx * o, ty + dy * reach, '6');
            im.put(tx + dx * reach, ty + dy * reach - dy * o, '6');
        }
    }
}

/// A rider on a loop: the ring it rides drawn as a thin dithered band of
/// light, and a comet of gold along the path just ridden.
pub(super) fn loop_light(im: &mut Img, s: &Session, v: &View, seat: u32, r: &Rider) {
    let Some(g) = r.ground else { return };
    let path = &level().terrain.paths[g.path as usize];
    if !r.looping || r.down > 0 || path.kind != Road::Ring {
        return;
    }
    // The ring: every few pixels of its length, a dim point and, now and again, a bright one.
    let len = path.len();
    let mut at = 0;
    while at <= len {
        let p = path.sample(at);
        let (x, y) = (v.qx(p.x), v.qy(p.y));
        if (0..v.w).contains(&x) && (0..v.h).contains(&y) && bayer(x, y) < 0.6 {
            let pulse = ((at / (6 * Q)) as u32 + s.elapsed / 2 + seat) % 9 == 0;
            im.put(x, y - 1, if pulse { 'T' } else { 'U' });
        }
        at += 3 * Q;
    }
    // The comet: where the horse has just been, a gold tail thinning away.
    for j in 0..14 {
        let (fx, fy) = feet_at(r, j * 2);
        let (x, y) = (v.qx(fx), v.qy(fy) - 10);
        let k = 1.0 - j as f32 / 14.0;
        disc(im, x, y, 1 + (k * 2.5) as i32, if j < 3 { 'T' } else if j < 7 { '5' } else { '4' }, 0.35 + 0.6 * k);
    }
}

/// A lantern hung in the air: a chain, a cap, a glass with a flame in it.
pub(super) fn lantern(im: &mut Img, x: i32, y: i32, flick: u32) {
    for dy in 1..=6 {
        im.put(x, y - dy, 'j');
    }
    im.rect(x - 3, y, 7, 2, 'k');
    im.rect(x - 3, y + 2, 7, 8, 'g');
    im.rect(x - 2, y + 3, 5, 6, if flick % 6 < 3 { '4' } else { '5' });
    im.rect(x - 1, y + 4, 3, 4, '6');
    im.rect(x, y + 5, 1, 2, '9');
    im.rect(x - 3, y + 10, 7, 1, 'k');
}

/// How far the whole picture leans round a loop, in brads: it tilts into the
/// turn as the horse climbs and rights itself as it comes out.
pub(super) fn tilt_of(s: &Session, me: u32) -> i32 {
    if crate::render::viewer::calm() {
        return 0;
    }
    let Some(seat) = s.seats.get(&me) else { return 0 };
    let dir = seat.rider.dir;
    let lean = |k: f32| (juice_ease(k) * 16.0) as i32 * dir;
    if let Some(&since) = s.show.looping.get(&me) {
        return lean((s.elapsed.saturating_sub(since) as f32 / 10.0).min(1.0));
    }
    s.show.fx.iter().rev().find(|f| u32::from(f.seat) == me && f.kind == Kind::LoopOut).map_or(0, |f| {
        let age = f.age(s.elapsed);
        if age < 12 { lean(1.0 - age as f32 / 12.0) } else { 0 }
    })
}

fn juice_ease(k: f32) -> f32 {
    k * k * (3.0 - 2.0 * k)
}

/// The whole picture turned `brads` about its middle and drawn a little
/// larger, so no corner shows: nearest sample, integer maths.
pub(super) fn tilted(im: &Img, brads: i32) -> Img {
    let mut out = Img::black(im.w, im.h);
    stamp_turned(&mut out, im, (im.w / 2, im.h / 2), (im.w / 2, im.h / 2), brads, 304, false);
    out
}

/// How long each record shows in the world.
fn life(kind: Kind) -> u32 {
    match kind {
        Kind::Spark => 18,
        Kind::Pop => 30,
        Kind::Hit => 34,
        Kind::Fall => 44,
        Kind::Warden => 26,
        Kind::WardenDown => 80,
        Kind::Lantern => 50,
        Kind::Bell => 80,
        Kind::LoopIn => 14,
        Kind::LoopOut => 40,
        Kind::Land => 16,
        Kind::Hop => 10,
        Kind::Spur => 12,
        Kind::Whoosh => 14,
        Kind::Spring | Kind::Pad => 16,
        Kind::Chain => 26,
        Kind::Crate => 20,
        Kind::Finish => 40,
    }
}

/// Every record the session keeps, drawn where it happened.
pub(super) fn events(im: &mut Img, s: &Session, v: &View, me: u32) {
    let tick = u64::from(s.elapsed);
    // A run of sparks is a crowd: the newest few burst, the rest only tick the counter.
    let mut sparks = 0;
    for f in s.show.fx.iter().rev() {
        let age = f.age(s.elapsed);
        if age >= life(f.kind) {
            continue;
        }
        let (sx, sy) = (v.sx(f.x), v.sy(f.y));
        if !(-60..v.w + 60).contains(&sx) || !(-60..v.h + 60).contains(&sy) {
            continue;
        }
        let a = age as f32;
        let at = (sx as f32, sy as f32);
        let seed = hash(f.x, f.y, f.born);
        match f.kind {
            Kind::Spark => {
                sparks += 1;
                if sparks > 10 || age >= 14 {
                    continue;
                }
                let mut pen = Pen::new(im, 260);
                if age < 4 {
                    pfx::rays(&mut pen, at, 6, 7.0 + a, 1.0, seed as f32 * 0.31, ramps::GOLD, a / 4.0);
                }
                let spray = Spray { gravity: 0.06, ..Spray::round(seed, 5, at, (0.8, 2.2), (8.0, 14.0)) };
                spray.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 1, tick));
                if age < 7 {
                    pen.shock(at, 2.0 + a * 1.9, 1.2, 1.0, ramps::GOLD, a / 7.0);
                }
            }
            Kind::Pop => {
                // The snuffer's cold puff, with the warm spark of light it gives back.
                let id = if f.n == 1 { "lightgallop.floater" } else { "lightgallop.snuffer" };
                if painted(id) && age < 12 {
                    let (anim, i) = anim_at(id, "pop", age);
                    stamp_scaled(im, &anim.frames[i], sx, sy + 14, v.zoom);
                }
                let mut pen = Pen::new(im, 700);
                if age < 7 {
                    pfx::rays(&mut pen, at, 8, 12.0, 1.0, seed as f32 * 0.37, ramps::GOLD, a / 7.0);
                }
                let spray = Spray { gravity: 0.14, ..Spray::round(seed, 12, at, (1.2, 3.2), (9.0, 18.0)) };
                spray.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::EMBER, 2, tick));
                let rise = Spray { gravity: -0.05, ..Spray::round(seed ^ 0x77, 5, at, (0.2, 0.9), (16.0, 28.0)) };
                rise.each(a, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::GOLD, 1, tick));
                if age < 14 {
                    pen.shock((at.0, at.1 + 6.0), 3.0 + a * 2.2, 1.4, 0.5, ramps::SMOKE, a / 14.0);
                }
            }
            Kind::Hit | Kind::Fall => {
                juice::impact_burst(im, at, age, seed, f.weight, tick);
                if f.kind == Kind::Fall {
                    let mut pen = Pen::new(im, 500);
                    let smoke = Spray { gravity: -0.05, ..Spray::round(seed ^ 0xc3, 7, at, (0.3, 1.2), (20.0, 40.0)) };
                    smoke.each(a, |m| pfx::mote(&mut pen, &m, Shape::Smoke, ramps::SMOKE, 2, tick));
                }
                if f.kind == Kind::Fall && age >= 3 {
                    // The lantern comes down for the fallen rider.
                    let drop = (28.0 * (1.0 - juice_ease((a / 18.0).min(1.0)))) as i32;
                    for k in 1..4 {
                        if bayer(sx + k, sy) < 0.5 {
                            im.put(sx + k - 2, sy - 30 - drop - k, 'v');
                        }
                    }
                    lantern(im, sx, sy - 24 - drop, s.elapsed);
                    disc(im, sx, sy - 18 - drop, 14, '4', 0.12);
                }
                if f.n > 0 && f.seat as u32 == me {
                    juice::pop_number(im, &format!("-{}", f.n), (sx, sy - 14), age, 30, 1, 'c');
                }
            }
            Kind::Warden | Kind::WardenDown => {
                juice::impact_burst(im, at, age, seed, f.weight, tick);
                if f.kind == Kind::WardenDown {
                    juice::death_burst(im, at, age, seed ^ 5, tick);
                    // Fireworks over the gate for the pack.
                    for (j, (x, top, ramp)) in [(70.0, 56.0, ramps::GOLD), (192.0, 40.0, ramps::EMBER), (314.0, 62.0, ramps::BONE)].into_iter().enumerate() {
                        let t = age as i32 - 10 - j as i32 * 13;
                        if t >= 0 {
                            juice::firework(im, x, 130.0, top, t as u32, seed.wrapping_add(j as u32 * 31), ramp, tick);
                        }
                    }
                }
            }
            Kind::Lantern => {
                let mut pen = Pen::new(im, 600);
                if age < 10 {
                    pfx::rays(&mut pen, at, 10, 14.0, 1.0, seed as f32 * 0.21, ramps::GOLD, a / 10.0);
                }
                let embers = Spray { gravity: -0.035, ..Spray::round(seed, 14, at, (0.3, 1.6), (22.0, 48.0)) };
                embers.each(a, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::GOLD, 1, tick));
                if age < 24 {
                    pen.shock(at, 3.0 + a * 1.6, 1.4, 1.0, ramps::GOLD, a / 24.0);
                }
            }
            Kind::LoopIn => {
                let mut pen = Pen::new(im, 300);
                pen.shock(at, 4.0 + a * 2.2, 1.4, 1.0, ramps::FROST, a / 14.0);
            }
            Kind::LoopOut => {
                let mut pen = Pen::new(im, 700);
                pen.shock(at, 4.0 + a * 3.2, 1.8, 1.0, ramps::GOLD, a / 40.0 * 1.2);
                pen.shock(at, 2.0 + a * 2.0, 1.2, 1.0, ramps::FROST, a / 30.0);
                let spray = Spray { gravity: 0.04, ..Spray::round(seed, 16, at, (1.4, 3.6), (14.0, 26.0)) };
                spray.each(a, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, tick));
            }
            Kind::Land | Kind::Hop => {
                // A ring of dust rolling out either side of the hooves.
                let reach = if f.kind == Kind::Land { 5 + i32::from(f.n).min(14) } else { 4 };
                let t = a / life(f.kind) as f32;
                for side in [-1, 1] {
                    for j in 0..3 {
                        let x = sx + side * (2 + (a * reach as f32 / 6.0) as i32 + j * 3);
                        let y = sy - 1 - (a as i32 / 3) - j;
                        disc(im, x, y, 1 + (3.0 * t) as i32 + (j == 0) as i32, if j == 0 { 't' } else { 'O' }, 0.9 - 0.75 * t);
                    }
                }
            }
            Kind::Spur => {
                let mut pen = Pen::new(im, 300);
                pen.shock(at, 3.0 + a * 2.4, 1.6, 1.0, ramps::GOLD, a / 12.0);
                pfx::rays(&mut pen, at, 6, 10.0, 1.0, 0.0, ramps::EMBER, a / 8.0);
            }
            Kind::Whoosh => {
                // The wind breaks over the picture: long strokes sweeping the way the horse goes.
                if f.seat as u32 != me {
                    continue;
                }
                let Some(seat) = s.seats.get(&me) else { continue };
                let (ux, uy) = heading(&seat.rider);
                let (px, py) = (-uy, ux);
                for i in 0..6 {
                    let lane = (i - 3) * 34 + (hash(i, 1, 9) as i32).rem_euclid(14);
                    let along = 300 - age as i32 * 36 - i * 18;
                    let len = 70 + i * 6;
                    for j in 0..len {
                        let (x, y) = (192 + (ux * (along - j) + px * lane) / 4096, 124 + (uy * (along - j) + py * lane) / 4096);
                        if y > 31 && y < 207 && bayer(x, y) < 0.8 * (1.0 - j as f32 / len as f32) * (1.0 - a / 14.0) {
                            im.put(x, y, if j < len / 4 { 'V' } else { 'v' });
                        }
                    }
                }
            }
            Kind::Spring => {
                let mut pen = Pen::new(im, 400);
                let straw = Spray { gravity: 0.12, angle: -1.57, spread: 1.6, ..Spray::round(seed, 12, (at.0, at.1 - 8.0), (1.6, 3.6), (10.0, 16.0)) };
                straw.each(a, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::GOLD, 1, tick));
                pen.shock((at.0, at.1), 3.0 + a * 1.8, 1.4, 0.5, ramps::GOLD, a / 16.0);
            }
            Kind::Pad => {
                let mut pen = Pen::new(im, 300);
                for j in 0..5 {
                    pen.streak((at.0 - 8.0 + j as f32 * 4.0 + a * 2.0, at.1 - 3.0 - (j % 2) as f32 * 2.0), (1.0, 0.0), 8.0, ramps::GOLD, a / 16.0);
                }
            }
            Kind::Chain => {
                if f.seat as u32 == me {
                    juice::pop_number(im, &format!("x{}", f.n), (sx + 10, sy - 18), age, 26, 1 + i32::from(f.n >= 4), if f.n >= 4 { '6' } else { 'T' });
                }
            }
            Kind::Crate => {
                let mut pen = Pen::new(im, 400);
                let ramp = match f.n {
                    0 => ramps::FROST,
                    2 => ramps::EMBER,
                    _ => ramps::GOLD,
                };
                let shards = Spray { gravity: 0.2, ..Spray::round(seed, 12, at, (1.4, 3.4), (10.0, 18.0)) };
                shards.each(a, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::EARTH, 1, tick));
                pen.shock(at, 3.0 + a * 1.7, 1.4, 1.0, ramp, a / 20.0);
            }
            Kind::Finish => {
                juice::confetti(im, (sx - 40, sy - 70, 80, 70), s.elapsed.wrapping_sub(f.born), seed, 18);
            }
            Kind::Bell => {
                let mut pen = Pen::new(im, 700);
                if age < 12 {
                    pfx::rays(&mut pen, at, 12, 22.0, 1.0, 0.0, ramps::GOLD, a / 12.0);
                }
                for k in 0..3 {
                    let t = a - k as f32 * 8.0;
                    if t > 0.0 {
                        pen.shock(at, 3.0 + t * 2.6, 1.6, 1.0, ramps::GOLD, t / 40.0);
                    }
                }
            }
        }
    }
}

/// The pack on the road: a crown over whoever leads it, and an arrow at the
/// edge of the picture for each knight off the side of it.
pub(super) fn pack(im: &mut Img, s: &Session, v: &View, me: u32) {
    let t = &level().terrain;
    let lead = s.seats.iter().filter(|(_, st)| st.rider.down == 0 && st.rider.finish.is_none()).max_by_key(|(_, st)| st.rider.x).map(|(&id, _)| id);
    for (&id, st) in &s.seats {
        let c = st.rider.centre(t);
        let (sx, sy) = (v.qx(c.0), v.qy(c.1));
        if s.seats.len() > 1 && Some(id) == lead && (4..v.w - 4).contains(&sx) && st.rider.down == 0 {
            // A little crown, bobbing, over the leader's plume.
            let (x, y) = (sx - 4, sy - 33 + (s.elapsed as i32 / 6) % 2);
            im.rect(x, y + 3, 9, 3, 'k');
            im.rect(x + 1, y + 4, 7, 1, '5');
            for dx in [0, 4, 8] {
                im.rect(x + dx, y, 1, 3, 'k');
                im.put(x + dx, y + 1, '6');
            }
        }
        // In a leader's wake the pack is tied by a thin line of gold dashes: the tow.
        if st.rider.draft && st.rider.down == 0 {
            let ahead = s
                .seats
                .iter()
                .filter(|&(&o, l)| o != id && l.rider.down == 0 && l.rider.x > st.rider.x && l.rider.x - st.rider.x < 220 * crate::rules::lightgallop::math::Q)
                .min_by_key(|(_, l)| l.rider.x);
            if let Some((_, lead)) = ahead {
                let lc = lead.rider.centre(t);
                let (lx, ly) = (v.qx(lc.0), v.qy(lc.1));
                let steps = (lx - sx).abs().max((ly - sy).abs()).max(1);
                for i in (6..steps - 6).step_by(1) {
                    if (i / 3 + s.elapsed as i32 / 2) % 3 == 0 {
                        im.put(sx + (lx - sx) * i / steps, sy - 6 + (ly - sy) * i / steps, 'O');
                    }
                }
            }
        }
        if id == me || (-6..v.w + 6).contains(&sx) {
            continue;
        }
        // Off to a side: an arrow in the knight's colour on that edge.
        let left = sx < 0;
        let y = sy.clamp(44, 196);
        let ink = CLOTH[(id.max(1) as usize - 1) % 6][2];
        let x0 = if left { 2 } else { v.w - 9 };
        for row in 0..7 {
            let w = 4 - (row - 3i32).abs();
            for col in 0..w.max(0) {
                let x = if left { x0 + col } else { x0 + 6 - col };
                im.put(x, y - 3 + row, ink);
            }
            let ex = if left { x0 + w.max(0) } else { x0 + 6 - w.max(0) };
            im.put(ex, y - 3 + row, 'k');
        }
        im.text(if left { x0 + 7 } else { x0 - 7 }, y - 3, &id.to_string(), ink);
    }
}

/// The world layer's shake from the records that jolt it: any knight's
/// weighty blow on this screen's own rider, the warden's falls for all.
pub(super) fn jolt(s: &Session, me: u32) -> (i32, i32) {
    if crate::render::viewer::calm() {
        return (0, 0);
    }
    let blows = s.show.fx.iter().filter_map(|f| {
        let mine = f.seat as u32 == me;
        let shared = matches!(f.kind, Kind::WardenDown | Kind::Bell | Kind::Warden);
        let heavy = matches!(f.kind, Kind::Hit | Kind::Fall | Kind::Warden | Kind::WardenDown | Kind::Bell);
        let age = f.age(s.elapsed);
        (heavy && (mine || shared) && f.weight >= Weight::Solid && age < f.weight.shake_ticks()).then_some((f.weight, age))
    });
    juice::shake(blows, u64::from(s.elapsed))
}
