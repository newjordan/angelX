//! Griffinwing's blows and bursts: a muzzle's flash, a struck foe in one
//! white frame, a foe going up in sparks, debris and smoke, a lamp breaking,
//! the Wyrm's death, a barrel roll's streaks, a kite's tether and the
//! starbursts of a volley running home. All of it is read off the session's
//! records (`rules::griffinwing::show`) and its flares by their age, scaled
//! by how far off the thing is, in the realm's inks and an ordered dither,
//! with no glow that dims a figure.
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::hud::Ctx;
use crate::render::overworld::ink::{Img, bayer, hash};
use crate::rules::griffinwing::show::{Fx, What};
use crate::rules::griffinwing::{self as gw, Arm, Craft, Q, St};
use crate::rules::juice::Weight;

use super::layout::{H, W};

/// The cold light of the Gloam's own things, and the warm of what answers.
const COLD: Ramp = ramps::STORM;

/// How big a thing is at `depth`: dots a unit tall, in fifths, kept so a
/// far pop is still seen and a near one never fills the screen.
fn scale(c: &Ctx, depth: f32) -> f32 {
    (c.tall(1.0, depth) / 4.0).clamp(0.8, 2.8)
}

fn seed_of(f: &Fx) -> u32 {
    hash(f.s >> 4, f.x >> 3 ^ f.z, u32::from(f.n) ^ f.born)
}

/// Where a craft is on the picture: a knight's own sits low and near, where
/// the reticle's line starts.
pub(super) fn craft_at(c: &Ctx, craft: &Craft, own: bool) -> Option<(f32, f32, f32)> {
    let here = c.s.craft_s(craft);
    c.at(if own { here + gw::dm(8) } else { here }, craft.x, craft.z)
}

/// The craft `id` (a seat's or a wingman's), if it flies.
fn craft_of<'a>(c: &Ctx<'a>, id: u8) -> Option<&'a Craft> {
    if id <= 6 {
        c.s.seats.get(&u32::from(id)).map(|s| &s.craft)
    } else {
        c.s.wing.iter().find(|w| w.id == id).map(|w| &w.craft)
    }
}

// ─── the world's effects ────────────────────────────────────────────────────

/// A puff of smoke: a dithered disc that swells and thins as `m` ages,
/// laid over the scene (the kit's smoke goes under bare paper only, and the
/// canyon has none).
fn puff(pen: &mut Pen, m: &pfx::Mote, ramp: Ramp, size: f32) {
    let k = m.k.clamp(0.0, 1.0);
    let r = (1.2 + k * (1.5 + size)).max(1.0);
    let fill = 0.25 + 0.75 * (1.0 - k).powf(0.9);
    let (x, y) = (m.at.0.round() as i32, m.at.1.round() as i32);
    let ri = r.ceil() as i32;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            if (dx * dx + dy * dy) as f32 <= r * r && bayer(x + dx, y + dy) < fill {
                pen.put(x + dx, y + dy, ramp.at(0.15 + k * 0.7));
            }
        }
    }
}

/// A chunk of debris: a square of `size` dots in the ramp's body ink with a
/// bright edge, tumbling down.
fn chunk(pen: &mut Pen, m: &pfx::Mote, ramp: Ramp, size: i32) {
    let (x, y) = (m.at.0.round() as i32, m.at.1.round() as i32);
    let body = ramp.at(0.25 + m.k * 0.5);
    for dy in 0..size {
        for dx in 0..size {
            pen.put(x + dx, y + dy, body);
        }
    }
    pen.put(x, y, ramp.step(0));
}

/// A fireball: a dithered disc that swells fast out of a white core, burns
/// down `ramp` and thins away through the dither as it cools, like the
/// explosion of a sprite sheet.
fn fireball(pen: &mut Pen, at: (f32, f32), age: u32, rmax: f32, ramp: Ramp) {
    const LIFE: u32 = 18;
    if age >= LIFE {
        return;
    }
    let a = age as f32;
    let swell = crate::motion::ease::out((a / 7.0).min(1.0));
    let shrink = if age > 10 { 1.0 - (a - 10.0) / 16.0 } else { 1.0 };
    let r = (rmax * (0.25 + 0.75 * swell) * shrink).max(1.0);
    let ri = r.ceil() as i32 + 1;
    let burn = a / LIFE as f32;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (px, py) = (at.0 as i32 + dx, at.1 as i32 + dy);
            let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
            if d + (bayer(px, py) - 0.5) * 0.35 > 1.0 {
                continue;
            }
            if age > 9 && bayer(px + 3, py + 1) < (a - 9.0) / 9.0 {
                continue;
            }
            pen.put(px, py, ramp.at((d * 0.75 + burn * 0.7).clamp(0.0, 0.999)));
        }
    }
}

/// A flash, a ring, sparks, debris and smoke at `at`, `age` ticks after a foe
/// of `kind` went up under a blow of `weight`; `k` is the size at its depth.
#[allow(clippy::too_many_arguments)]
pub(super) fn burst(
    im: &mut Img,
    at: (f32, f32),
    age: u32,
    k: f32,
    kind: u16,
    weight: Weight,
    seed: u32,
    tick: u64,
) {
    let a = age as f32;
    let w = weight as i32 as f32;
    // A snuffer is cold light; a tower and a bomber are stone and flame.
    let (hot, spark, chunks) = match kind {
        0 => (COLD, COLD, ramps::BONE),
        1 => (ramps::EMBER, COLD, ramps::BONE),
        2 => (ramps::FIRE, ramps::EMBER, ramps::EARTH),
        3 => (ramps::FIRE, ramps::EMBER, ramps::BONE),
        _ => (ramps::GOLD, COLD, ramps::BONE),
    };
    let mut pen = Pen::new(im, 7000);
    // The fireball, and a white core in its first two ticks.
    let cold = kind <= 1;
    let rmax = if cold { (4.5 + w * 2.4) * k } else { (7.5 + w * 3.6) * k };
    fireball(&mut pen, at, age, rmax, if cold { COLD } else { ramps::FIRE });
    if age < 2 {
        let r = (rmax * 0.55) as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    pen.put(at.0 as i32 + dx, at.1 as i32 + dy, 'w');
                }
            }
        }
    }
    if age < 9 {
        pfx::rays(&mut pen, at, 10, (6.0 + w * 2.6) * k, 1.0, seed as f32 * 0.37, hot, a / 9.0);
    }
    let big = weight >= Weight::Crushing;
    if age < 20 && (big || age < 12) {
        pen.shock((at.0, at.1 + 2.0 * k), (2.0 + a * (1.3 + w * 0.3)) * k, 1.5, 0.75, ramps::SMOKE, a / 20.0);
    }
    let n = 9 + weight as u32 * 4;
    let sparks = Spray {
        gravity: 0.12 * k,
        ..Spray::round(seed, n, at, (0.9 * k, (2.6 + w * 0.3) * k), (9.0, 18.0 + w * 2.0))
    };
    sparks.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, spark, 2, tick));
    let debris = Spray {
        gravity: 0.2 * k,
        ..Spray::round(seed ^ 0x6d, 4 + weight as u32 * 3, at, (0.9 * k, 2.8 * k), (12.0, 26.0))
    };
    let size = if k > 1.5 { 3 } else { 2 };
    debris.each(a, |m| chunk(&mut pen, &m, chunks, size));
    let smoke = Spray {
        gravity: -0.05 * k,
        ..Spray::round(seed ^ 0x13, 4 + weight as u32 * 2, at, (0.3 * k, 1.2 * k), (14.0, 32.0))
    };
    smoke.each(a, |m| puff(&mut pen, &m, ramps::SMOKE, 2.5 * k));
}

/// A bolt landing: a white spark and a few thrown.
fn hit_spark(im: &mut Img, at: (f32, f32), age: u32, k: f32, seed: u32, tick: u64) {
    let mut pen = Pen::new(im, 200);
    if age < 3 {
        let r = ((3 - age) as f32 * 1.0 * k).ceil() as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    pen.put(at.0 as i32 + dx, at.1 as i32 + dy, if age == 0 { 'w' } else { '6' });
                }
            }
        }
    }
    let sparks = Spray {
        gravity: 0.1 * k,
        ..Spray::round(seed, 7, at, (0.8 * k, 2.0 * k), (6.0, 12.0))
    };
    sparks.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 1, tick));
}

/// A lamp breaking: its cold glass goes in a ring of shards and two gold
/// shocks, a white flash, rays, and the sparks left drifting up: the biggest
/// burst of the beat but the Wyrm's own.
fn lamp_burst(im: &mut Img, at: (f32, f32), age: u32, k: f32, seed: u32, tick: u64) {
    let a = age as f32;
    let mut pen = Pen::new(im, 9000);
    fireball(&mut pen, at, age, 15.0 * k, ramps::GOLD);
    if age < 5 {
        let r = ((5 - age) as f32 * 3.4 * k).round() as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let d2 = dx * dx + dy * dy;
                if d2 <= r * r && (d2 < r * r / 2 || bayer(at.0 as i32 + dx, at.1 as i32 + dy) < 0.55) {
                    pen.put(at.0 as i32 + dx, at.1 as i32 + dy, if d2 < r * r / 3 { 'w' } else { '3' });
                }
            }
        }
    }
    if age < 14 {
        pfx::rays(&mut pen, at, 14, 24.0 * k, 1.0, seed as f32, ramps::GOLD, a / 14.0);
    }
    if age < 30 {
        pen.shock((at.0, at.1), (3.0 + a * 2.8) * k, 1.8, 0.8, ramps::GOLD, a / 30.0);
        if age > 3 {
            pen.shock((at.0, at.1), (1.0 + (a - 3.0) * 1.8) * k, 1.4, 0.8, COLD, (a - 3.0) / 26.0);
        }
    }
    let shards = Spray {
        gravity: 0.22 * k,
        ..Spray::round(seed, 24, at, (1.1 * k, 3.8 * k), (16.0, 34.0))
    };
    shards.each(a, |m| chunk(&mut pen, &m, ramps::FROST, if k > 1.8 { 3 } else { 2 }));
    let embers = Spray {
        gravity: -0.03 * k,
        ..Spray::round(seed ^ 0x5a, 16, at, (0.5 * k, 2.2 * k), (18.0, 36.0))
    };
    embers.each(a, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::GOLD, 1, tick));
    let smoke = Spray {
        gravity: -0.05 * k,
        ..Spray::round(seed ^ 0x13, 8, at, (0.3 * k, 1.1 * k), (20.0, 40.0))
    };
    smoke.each(a, |m| puff(&mut pen, &m, ramps::SMOKE, 3.0 * k));
}

/// A lamp that is out smoulders: smoke curling up out of its socket and a
/// lick of flame, so the part that broke stays broken.
fn smoulder(im: &mut Img, at: (f32, f32), k: f32, tick: u32) {
    let mut pen = Pen::new(im, 400);
    for e in 0..6u32 {
        let age = (tick + e * 7) % 42;
        let t = age as f32 / 42.0;
        let sway = ((tick as f32 + e as f32 * 5.0) * 0.12).sin() * 3.0 * k * t;
        let m = pfx::Mote {
            i: e,
            at: (at.0 + sway, at.1 - 26.0 * k * t),
            vel: (0.0, 0.0),
            k: t,
            seed: e,
        };
        puff(&mut pen, &m, ramps::SMOKE, 2.5 * k);
    }
    // The flame in the socket.
    let flick = (tick / 2 + 3) % 3;
    for (dx, dy, ch) in [(0, 0, '6'), (-1, 1, 'R'), (1, 1, 'R'), (0, -1 - flick as i32 / 2, '@')] {
        pen.put(at.0 as i32 + dx, at.1 as i32 + dy, ch);
    }
}

/// The Wyrm's death: a white flash, then a blast at each part of it as it
/// lets go and falls (`wyrm.rs` says where and when), big and in the picture's
/// own measure (the Wyrm is the size of the canyon), the last the skull's.
fn wyrm_death(im: &mut Img, c: &Ctx, f: &Fx, age: u32, tick: u64) {
    if age < 5 {
        // The room goes white and comes back through a dither.
        let hold = (0.75 - age as f32 * 0.16).max(0.0);
        for y in 0..H {
            for x in 0..W {
                if bayer(x, y) < hold {
                    im.put(x, y, if age < 2 { 'w' } else { '9' });
                }
            }
        }
    }
    // The head, as it is held ahead of the wing (it keeps its pace until it
    // is done dying): the fall's own record is the crown lamp's place.
    let held = gw::PACE * age.min(60) as i32;
    let (hs, hx, hz) = (f.s + gw::dm(40) + held, f.x, f.z - gw::dm(42));
    let parts = super::wyrm::break_points();
    // The skull's is the heaviest (it stands eighth in the list).
    let skull = 7;
    for (i, (start, off)) in parts.into_iter().enumerate() {
        if age < start || age - start >= 40 {
            continue;
        }
        let place = c.at(hs + off[0], hx + off[1], hz + off[2]);
        let (x, y) = place.map_or((W as f32 / 2.0, 90.0), |(x, y, _)| (x, y));
        burst(
            im,
            (x.clamp(10.0, W as f32 - 10.0), y.clamp(30.0, H as f32 - 50.0)),
            age - start,
            2.0 + (i % 3) as f32 * 0.35,
            3,
            if i == skull { Weight::Finale } else { Weight::Kill },
            hash(i as i32, f.born as i32, 0x31),
            tick,
        );
    }
}

/// Everything the records and the flares say happened in the world: pops,
/// lamps, the Wyrm, a rescue, a landed bolt, a muzzle.
pub(super) fn world(im: &mut Img, c: &Ctx) {
    let now = c.s.elapsed;
    let tick = u64::from(now);
    for f in &c.s.show.fx {
        let age = f.age(now);
        match f.what {
            What::Pop if age < 36 => {
                if let Some((x, y, d)) = c.at(f.s, f.x, f.z) {
                    let kind = f.n & 15;
                    burst(im, (x, y), age, scale(c, d), kind, f.weight, seed_of(f), tick);
                }
            }
            What::LampOut if age < 40 => {
                if let Some((x, y, d)) = c.at(f.s, f.x, f.z) {
                    lamp_burst(im, (x, y), age, (scale(c, d) * 1.8).clamp(1.8, 3.4), seed_of(f), tick);
                }
            }
            What::Fall if age < 34 => {
                // A craft going down trails fire and smoke where it fell.
                let own = u32::from(f.seat) == c.seat;
                let here = (f.s, f.x, f.z - age as i32 * gw::dm(1) / 3);
                if let Some((x, y, d)) = c.at(here.0, here.1, here.2) {
                    let k = if own { 1.2 } else { scale(c, d) };
                    burst(im, (x, y + age as f32 * 0.8), age, k, 3, f.weight, seed_of(f), tick);
                }
            }
            What::WyrmDown if age < 100 => wyrm_death(im, c, f, age, tick),
            What::Rescue if age < 34 => rescue(im, c, f, age, tick),
            What::Muzzle if age < 3 => muzzle(im, c, f, age),
            _ => {}
        }
    }
    for lamp in c.s.foes.iter().filter(|f| f.kind == gw::Kind::Lamp && f.st == St::Broken) {
        if let Some((x, y, d)) = c.at(lamp.s, lamp.x, lamp.z) {
            smoulder(im, (x, y), (scale(c, d) * 1.2).clamp(1.0, 2.4), now);
        }
    }
    for fl in &c.s.fx {
        if fl.what != gw::Burst::Hit {
            continue;
        }
        let age = (14 - i32::from(fl.ticks)).max(0) as u32;
        if let Some((x, y, d)) = c.at(fl.s, fl.x, fl.z) {
            let seed = hash(fl.s >> 4, fl.x >> 3, fl.z as u32);
            hit_spark(im, (x, y), age, scale(c, d), seed, tick);
        }
    }
}

/// A wingman saved: a gold ring round its craft that swells and fades, with
/// rays, and stars rising off it.
fn rescue(im: &mut Img, c: &Ctx, f: &Fx, age: u32, tick: u64) {
    let Some(craft) = craft_of(c, f.n as u8) else {
        return;
    };
    let own = u32::from(f.n) == c.seat;
    let Some((x, y, d)) = craft_at(c, craft, own) else {
        return;
    };
    let k = scale(c, d).max(0.9);
    let a = age as f32;
    let mut pen = Pen::new(im, 600);
    pen.shock((x, y), (6.0 + a * 1.5) * k, 1.5, 0.8, ramps::GOLD, a / 34.0);
    if age < 10 {
        pfx::rays(&mut pen, (x, y), 10, 16.0 * k, 1.0, 0.2, ramps::GOLD, a / 10.0);
    }
    let stars = Spray {
        gravity: -0.05,
        ..Spray::round(seed_of(f), 10, (x, y), (0.4 * k, 1.5 * k), (16.0, 30.0))
    };
    stars.each(a, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, tick));
}

/// Two small flashes at a craft's guns, two ticks.
fn muzzle(im: &mut Img, c: &Ctx, f: &Fx, age: u32) {
    let id = u32::from(f.seat);
    let craft = if id == 0 {
        None
    } else {
        c.s.seats.get(&id).map(|s| &s.craft)
    };
    let Some(craft) = craft else {
        return;
    };
    let own = id == c.seat;
    let Some((x, y, d)) = craft_at(c, craft, own) else {
        return;
    };
    let k = if own { 1.4 } else { scale(c, d).min(1.2) };
    let (x, y) = (x.round() as i32, y.round() as i32);
    let off = (4.0 * k) as i32;
    for side in [-off, off] {
        let r = (3 - age as i32).max(1) * i32::from(k >= 0.9) + 1;
        for d in 0..=r {
            let ink = if d == 0 { 'w' } else { '6' };
            im.put(x + side + d, y, ink);
            im.put(x + side - d, y, ink);
            im.put(x + side, y - d, ink);
            im.put(x + side, y + d / 2, '4');
        }
    }
}

/// A foe the last blow struck, in one white frame: the pixels of the scene
/// that belong to it (those at its own depth), white for two ticks and then
/// through a thinning dither.
pub(super) fn strikes(im: &mut Img, c: &Ctx) {
    if c.depth.is_empty() {
        return;
    }
    for f in c.s.foes.iter().filter(|f| f.flash > 0 && f.alive()) {
        let Some((x, y, d)) = c.at(f.s, f.x, f.z) else {
            continue;
        };
        let r = (c.tall(f.radius() as f32 / Q as f32 * 2.0, d) / 2.0 * 1.25).clamp(3.0, 40.0);
        let reach = f.radius() as f32 / Q as f32 * 1.6;
        let hold = match f.flash {
            4.. => 1.1,
            3 => 0.8,
            2 => 0.5,
            _ => 0.25,
        };
        let (x0, y0) = ((x - r).floor() as i32, (y - r).floor() as i32);
        let (x1, y1) = ((x + r).ceil() as i32, (y + r).ceil() as i32);
        for py in y0.max(0)..=y1.min(H - 1) {
            for px in x0.max(0)..=x1.min(W - 1) {
                let dd = c.depth[(py * W + px) as usize];
                if (dd - d).abs() > reach || bayer(px, py) >= hold {
                    continue;
                }
                im.put(px, py, if f.flash >= 3 { 'w' } else { 'W' });
            }
        }
    }
}

// ─── the craft's own ────────────────────────────────────────────────────────

/// A barrel roll: a pair of spiral streaks wound round the craft, longer as
/// the roll goes, and at its start long dodge lines flung the way it left.
pub(super) fn rolls(im: &mut Img, c: &Ctx) {
    let s = c.s;
    let one = |im: &mut Img, craft: &Craft, own: bool, tick: u64| {
        if craft.roll == 0 || craft.down > 0 {
            return;
        }
        let Some((x, y, d)) = craft_at(c, craft, own) else {
            return;
        };
        let into = craft.roll_len - craft.roll;
        let r = (c.tall(5.2, d) / 2.0).clamp(10.0, 44.0);
        let way = f32::from(craft.dir);
        let mut pen = Pen::new(im, 700);
        // Three arms of the spiral, each a ribbon that trails behind its head.
        for arm in 0..3 {
            let head = into as f32 * 0.55 * way + arm as f32 * std::f32::consts::TAU / 3.0;
            let tail = (6.0 + into.min(10) as f32 * 0.9) * 0.1;
            let steps = 14;
            for j in 0..steps {
                let t = j as f32 / steps as f32;
                let a = head - way * t * tail * 4.0;
                let rr = r * (1.05 + 0.18 * t);
                let (px, py) = (x + a.cos() * rr, y + a.sin() * rr * 0.8);
                let ch = COLD.at(0.05 + t * 0.9);
                pen.light(px.round() as i32, py.round() as i32, ch);
                pen.light(px.round() as i32 + 1, py.round() as i32, ch);
            }
        }
        // The dodge: long lines flung out the way the roll left, fast to go.
        if into < 9 {
            let fade = 1.0 - into as f32 / 9.0;
            for j in 0..7 {
                let dy = (j as f32 - 3.0) * (r * 0.28);
                let len = (22.0 + (j * 7 % 5) as f32 * 9.0) * fade * if own { 1.5 } else { 1.0 };
                let (x0, y0) = (x - way * r * 0.6, y + dy);
                pen.streak((x0, y0), (-way, 0.0), len, ramps::FROST, into as f32 / 12.0);
            }
        }
        let _ = tick;
    };
    let tick = u64::from(s.elapsed);
    for (&id, seat) in &s.seats {
        one(im, &seat.craft, id == c.seat, tick);
    }
    for w in &s.wing {
        one(im, &w.craft, false, tick);
    }
}

/// A kite on a rider's tail: a red tether between them, thick and beating,
/// so a knight sees at once who to save and from what.
pub(super) fn tethers(im: &mut Img, c: &Ctx) {
    let s = c.s;
    for f in s.foes.iter().filter(|f| f.st == St::Latched) {
        let Some(victim) = craft_of(c, f.on) else {
            continue;
        };
        let own = u32::from(f.on) == c.seat;
        let (Some(a), Some(b)) = (c.at(f.s, f.x, f.z), craft_at(c, victim, own)) else {
            continue;
        };
        let beat = (s.elapsed / 3) % 2 == 0;
        let n = 14;
        for k in 0..=n {
            let t = k as f32 / n as f32;
            let (px, py) = (
                (a.0 + (b.0 - a.0) * t).round() as i32,
                (a.1 + (b.1 - a.1) * t).round() as i32,
            );
            if (k + (s.elapsed as i32 / 2)) % 2 == 0 {
                im.put(px, py, if beat { '7' } else { 'R' });
                im.put(px + 1, py, if beat { 'R' } else { '8' });
            }
        }
    }
}

/// A starburst loosed from a volley runs home: a faint line from each to the
/// foe it hunts, and a hot bracket on that foe while any is on its way, so a
/// release of six reads as six lines closing on six marks.
pub(super) fn volley(im: &mut Img, c: &Ctx) {
    let s = c.s;
    let tick = s.elapsed;
    let mut marked: Vec<u16> = Vec::new();
    for b in s.bolts.iter().filter(|b| b.shot == Arm::Star && b.aim != 0) {
        let Some(foe) = s.foes.iter().find(|f| f.id == b.aim && f.alive()) else {
            continue;
        };
        let (Some(a), Some(t)) = (c.at(b.s, b.x, b.z), c.at(foe.s, foe.x, foe.z)) else {
            continue;
        };
        let mine = u32::from(b.owner) == c.seat;
        let n = 16;
        for k in 0..n {
            if (k + tick as i32 / 2) % 3 != 0 {
                continue;
            }
            let u = k as f32 / n as f32;
            let (px, py) = (
                (a.0 + (t.0 - a.0) * u).round() as i32,
                (a.1 + (t.1 - a.1) * u).round() as i32,
            );
            if c.clear(px, py, a.2.min(t.2)) {
                im.put(px, py, if mine { '6' } else { 'z' });
            }
        }
        if mine && !marked.contains(&foe.id) {
            marked.push(foe.id);
            let half = (c.tall(foe.radius() as f32 / Q as f32 * 2.9, t.2) / 2.0).clamp(6.0, 24.0) as i32;
            let (cx, cy) = (t.0.round() as i32, t.1.round() as i32);
            let grow = i32::from(tick % 4 < 2);
            let ink = if tick % 6 < 3 { '7' } else { 'w' };
            for (sx, sy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let (ex, ey) = (cx + sx * (half + grow), cy + sy * (half + grow));
                for d in 0..4 {
                    im.put(ex - sx * d, ey, ink);
                    im.put(ex, ey - sy * d, ink);
                }
            }
        }
    }
}

/// Dust and light off the canyon for the Wyrm's tells, drawn as a dithered
/// rim: the picture warns before the footprints land.
pub(super) fn tell_rim(im: &mut Img, c: &Ctx) {
    use gw::Wyrm;
    let Some(b) = &c.s.boss else { return };
    let (k, urgent) = match b.st {
        Wyrm::Tell => {
            let beat = if b.ticks < 20 { 4 } else { 8 };
            let pulse = (c.s.elapsed / beat) % 2 == 0;
            (0.2 + if pulse { 0.14 } else { 0.0 }, b.ticks < 20)
        }
        Wyrm::Roar => (0.22, false),
        _ => return,
    };
    super::super::juice::vignette(im, (0, 0, W, H), k, 14, '8');
    // The last half second of a tell: the rim goes red.
    if urgent {
        super::super::juice::vignette(im, (0, 0, W, H), k * 0.9, 8, '7');
    }
}
