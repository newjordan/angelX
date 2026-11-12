//! Brasswake's effects in the aisle: blows that spark and splinter, kicks that throw dust, slams
//! that crack the floor, casks that fall, break and slop, bread that sparkles, a gate that
//! opens in a shaft of light. Every one is a pure function of a mark's age on the run's tick
//! (`rules::brasswake::show`), so a friend's mirror and a guest draw the same pixels.
use super::super::super::ink::{Img, bayer, hash};
use super::super::juice;
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::hud::{BOTTOM, TOP, W};
use crate::motion::ease;
use crate::rules::brasswake::{
    Beat, Lane,
    show::{Kind, Mark},
};
use crate::rules::juice::Weight;

/// Ale: a pale head down to a dark amber.
pub(super) const ALE: Ramp = Ramp(['9', 'T', 't', 'O', 'o', 'r']);
/// Steam off a copper vat: bone to smoke.
pub(super) const STEAM: Ramp = Ramp(['H', 'i', 'h', 'J', 'G', 'j']);

/// What an effect needs to place itself on the screen.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub d: &'a Lane,
    pub now: u32,
    pub cam: i32,
}

impl Ctx<'_> {
    pub fn at(&self, x: i32, y: i32) -> (f32, f32) {
        ((x - self.cam) as f32, y as f32)
    }
}

fn seed(m: &Mark, extra: u32) -> u32 {
    m.born.wrapping_mul(0x9E37_79B1).wrapping_add(extra.wrapping_mul(0x85EB_CA77)) ^ 0x2c1b_3c6d
}

/// A dithered shadow under a body's feet.
pub(super) fn shadow(im: &mut Img, x: i32, y: i32, rx: i32, ry: i32, k: f32) {
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let (fx, fy) = (dx as f32 / rx as f32, dy as f32 / ry as f32);
            if fx * fx + fy * fy <= 1.0 && bayer(x + dx, y + dy) < k {
                im.put(x + dx, y + dy, 'k');
            }
        }
    }
}

// ─── under the bodies ────────────────────────────────────────────────────────

/// What lies on the floor: a puddle of ale where a cask broke, a scuff where a slam fell, the
/// light of a gate that fell.
pub(super) fn floor(im: &mut Img, c: Ctx) {
    for m in &c.d.show.marks {
        if m.wave != c.d.wave && !matches!(m.kind, Kind::Clear { .. }) {
            continue;
        }
        let age = m.age(c.now);
        let (x, y) = (m.x - c.cam, m.y);
        match m.kind {
            Kind::Smash { .. } => {
                let k = age as f32 / 48.0;
                let r = 12.0 + 8.0 * ease::out((age as f32 / 10.0).min(1.0));
                for dy in -8..=8i32 {
                    for dx in -22..=22i32 {
                        let (fx, fy) = (dx as f32 / r, dy as f32 / 7.0);
                        if fx * fx + fy * fy <= 1.0 && bayer(x + dx, y + dy) < 0.8 * (1.0 - k) {
                            let rim = fx * fx + fy * fy > 0.55;
                            im.put(x + dx, y + dy, if rim { 'p' } else { 'r' });
                            if !rim && (dx + dy).rem_euclid(5) == 0 {
                                im.put(x + dx, y + dy, 'o');
                            }
                        }
                    }
                }
            }
            Kind::Quake { boss, rush: false, .. } if age < 24 => {
                let r = if boss { 30.0 } else { 18.0 };
                let mut pen = Pen::new(im, 300);
                pen.shock((x as f32, y as f32 + 2.0), 6.0 + age as f32 * r / 12.0, 1.8, 0.4, ramps::SMOKE, age as f32 / 24.0);
            }
            Kind::Clear { .. } if age < 30 => {
                // A shaft of light down the gate that fell.
                let gx = (i32::from(m.wave) + 1) * 240 - c.cam;
                let k = age as f32 / 30.0;
                let half = (4.0 + 10.0 * ease::out((age as f32 / 8.0).min(1.0))) as i32;
                for yy in TOP..BOTTOM {
                    for dx in -half..=half {
                        let edge = dx.abs() as f32 / half.max(1) as f32;
                        if bayer(gx + dx, yy) < (1.0 - edge) * 0.55 * (1.0 - k) {
                            im.put(gx + dx, yy, if edge < 0.4 { '6' } else { 'O' });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// A foe's telegraph: the danger rectangle fills with dither as the wind-up runs out, and a
/// shrinking ring closes on the spot a slam will land; steam chevrons run the rush's length.
pub(super) fn telegraph(im: &mut Img, c: Ctx, tell: u32) {
    for e in &c.d.foes {
        if !matches!(e.beat, Beat::Tell) || e.hp == 0 {
            continue;
        }
        let z = e.danger;
        let (x, y) = (z.x - c.cam, z.y);
        let total = tell.saturating_sub(e.stage() as u32 * 3).max(23);
        let k = 1.0 - e.ticks as f32 / total as f32;
        let blink = e.ticks < 9 && (e.ticks / 2) % 2 == 0;
        let (fill, hot) = if e.rush { ('u', 'U') } else { ('p', 'R') };
        for yy in y..y + z.h {
            for xx in x..x + z.w {
                if bayer(xx, yy) < 0.1 + k * 0.42 + if blink { 0.2 } else { 0.0 } {
                    im.put(xx, yy, if (xx + yy + c.now as i32 / 3) % 7 == 0 { hot } else { fill });
                }
            }
        }
        if e.rush {
            // Chevrons that run toward the far end, faster as it nears.
            let step = 16;
            let slide = (c.now as i32 * 2) % step;
            let mut xx = if e.dir > 0 { x + 4 + slide } else { x + z.w - 4 - slide };
            while xx > x + 2 && xx < x + z.w - 2 {
                for t in 0..4 {
                    im.put(xx - e.dir * t, y + z.h / 2 - 4 + t, hot);
                    im.put(xx - e.dir * t, y + z.h / 2 + 4 - t, hot);
                }
                xx += e.dir * step;
            }
        } else {
            let cx = x + z.w / 2;
            let cy = y + z.h / 2;
            let r = (1.0 - k) * 30.0 + 6.0;
            let mut pen = Pen::new(im, 200);
            pen.ring((cx as f32, cy as f32), r, 1.3, 0.55, ramps::EMBER, 0.0);
        }
    }
}

/// Where a thrown cask will land, and the cask coming down on it: a shadow that tightens, the cask
/// falling out of the dark above the aisle, turning.
pub(super) fn incoming_floor(im: &mut Img, c: Ctx, tell_total: u32) {
    for k in &c.d.incoming {
        if k.tell == 0 {
            continue;
        }
        let z = k.danger;
        let (cx, cy) = (z.x - c.cam + z.w / 2, z.y + z.h / 2);
        let t = k.tell as f32 / tell_total.max(1) as f32;
        let r = 8.0 + 14.0 * t;
        shadow(im, cx, cy + 4, (r * 1.2) as i32, (r * 0.45) as i32 + 1, 0.7 - t * 0.3);
        let mut pen = Pen::new(im, 160);
        pen.ring((cx as f32, cy as f32 + 2.0), 10.0 + 22.0 * t, 1.2, 0.45, ALE, 0.0);
        // Whose cask it is: the thrower's colour on a pip over the mark.
        im.rect(cx - 3, z.y - 12, 6, 8, 'k');
        im.rect(cx - 2, z.y - 11, 4, 6, super::hud::pip(k.from));
    }
}

/// The height a falling cask is over its mark, `tell` ticks before it lands.
pub(super) fn fall_height(tell: u32, total: u32) -> i32 {
    let t = tell as f32 / total.max(1) as f32;
    (t * t * 190.0) as i32
}

// ─── over the bodies ─────────────────────────────────────────────────────────

fn splinters(pen: &mut Pen, at: (f32, f32), age: f32, sd: u32, n: u32, tick: u64) {
    let wood = Spray { gravity: 0.22, ..Spray::round(sd, n, at, (1.6, 4.2), (14.0, 26.0)) };
    wood.each(age, |m| pfx::mote(pen, &m, Shape::Shard, ramps::EARTH, 2, tick));
}

/// A blow landed: the spark and its splinters, dust at the heel on the heavy ones, stars and a
/// foam of dust on a kill, a row of blasts down a keeper that falls.
pub(super) fn blow(im: &mut Img, c: Ctx, m: &Mark, tick: u64) {
    let Kind::Blow { swing, boss, slain, foe, .. } = m.kind else { return };
    let age = m.age(c.now);
    let w = m.kind.weight();
    let lift = if boss { 40.0 } else { 26.0 };
    let (fx, fy) = c.at(m.x, m.y);
    let at = (fx - f32::from(m.dir) * 7.0, fy - lift);
    let sd = seed(m, u32::from(foe));
    juice::impact_burst(im, at, age, sd, w, tick);
    if age < 3 {
        // The instant of contact: a hard star, bigger for a heavier blow, that is gone in three ticks.
        let r = 3 + 2 * w as i32 + (2 - age as i32);
        let (cx, cy) = (at.0 as i32, at.1 as i32);
        for i in -r..=r {
            let fade = (i.abs() as f32 / r as f32) * 0.8 + age as f32 * 0.15;
            if bayer(i, age as i32) >= fade {
                im.put(cx + i, cy, 'T');
                im.put(cx, cy + i, 'T');
            }
            if i.abs() <= r * 2 / 3 {
                im.put(cx + i, cy + i, 'O');
                im.put(cx + i, cy - i, 'O');
            }
        }
        im.rect(cx - 1, cy - 1, 3, 3, 'c');
    }
    {
        let mut pen = Pen::new(im, 500);
        if w >= Weight::Solid {
            splinters(&mut pen, at, age as f32, sd ^ 0x5a, if swing == 2 { 12 } else { 6 }, tick);
        }
        if w >= Weight::Crushing && age < 18 {
            pen.shock((fx, fy + 2.0), 4.0 + age as f32 * 2.6, 1.6, 0.4, ramps::SMOKE, age as f32 / 18.0);
        }
        if swing == 2 && age < 9 {
            // The kick's drive: speed lines thrown the way it went, long ones across the aisle.
            for i in 0..7 {
                let off = hash(i, sd as i32, 3) as f32 % 34.0;
                let len = 9.0 + (hash(i, sd as i32, 5) % 40) as f32;
                pen.streak((at.0 + f32::from(m.dir) * (6.0 + age as f32 * 9.0), at.1 - 17.0 + off), (f32::from(m.dir), 0.0), len, ramps::BONE, age as f32 / 9.0);
            }
        }
        if slain {
            let stars = Spray { gravity: 0.03, drag: 0.06, ..Spray::round(sd ^ 0x31, 7, (at.0, at.1 - 6.0), (0.9, 2.6), (22.0, 34.0)) };
            stars.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Star, ramps::GOLD, 2, tick));
            let foam = Spray { gravity: -0.04, ..Spray::round(sd ^ 0xc3, 8, (fx, fy - 8.0), (0.4, 1.4), (16.0, 30.0)) };
            foam.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Smoke, ramps::BONE, 2, tick));
            let ale = Spray { gravity: 0.2, ..Spray::round(sd ^ 0x77, 12, at, (1.2, 3.4), (14.0, 24.0)) };
            ale.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Droplet, ALE, 1, tick));
        }
    }
    if boss && slain {
        // The room whites out for the length of the hold, then the blasts roll down the keeper.
        if age < 7 {
            for y in TOP..BOTTOM {
                for x in 0..W {
                    if bayer(x, y) < 0.42 * (1.0 - age as f32 / 7.0) {
                        im.put(x, y, 'H');
                    }
                }
            }
        }
        // A chain of blasts down the keeper's length, the vat roaring.
        for i in 0..9u32 {
            let start = i * 4;
            if age < start {
                continue;
            }
            let o = ((hash(i as i32, sd as i32, 7) % 60) as f32 - 30.0, (hash(i as i32, sd as i32, 9) % 56) as f32);
            juice::impact_burst(im, (fx + o.0, fy - 10.0 - o.1), age - start, sd ^ i, Weight::Kill, tick);
        }
        let mut pen = Pen::new(im, 600);
        pen.shock((fx, fy + 2.0), 6.0 + age as f32 * 5.0, 2.4, 0.4, ramps::GOLD, (age as f32 / 26.0).min(1.0));
        for i in 0..3 {
            let side = f32::from([-30i8, 0, 30][i]);
            for t in 0..14 {
                pen.wisp((fx + side, fy - 40.0), age as f32 - t as f32, (0.0, 30.0), 24.0, 56.0, 6.0, STEAM, sd ^ i as u32);
            }
        }
    }
}

/// The swing in the air: a streak in front of the fist for a jab, an arc for the cross, a low
/// sweep for the kick; it follows the knight.
pub(super) fn swoosh(im: &mut Img, c: Ctx, tick: u64) {
    let d = c.d;
    for m in &d.show.marks {
        let swing = match m.kind {
            Kind::Blow { swing, .. } | Kind::Whiff { swing } => swing,
            _ => continue,
        };
        let age = m.age(c.now);
        if age > 5 {
            continue;
        }
        let dir = f32::from(m.dir.max(-1).min(1));
        let dir = if dir == 0.0 { d.dir as f32 } else { dir };
        let (kx, ky) = c.at(d.x, d.y);
        let k = age as f32 / 5.0;
        let mut pen = Pen::new(im, 80);
        let (r, base, sweep): (f32, f32, f32) = match swing {
            0 => (22.0, 30.0, 0.0),
            1 => (26.0, 34.0, 1.4),
            _ => (30.0, 14.0, -1.5),
        };
        let steps = 7;
        for i in 0..steps {
            let a = if sweep == 0.0 { 0.0 } else { (i as f32 / steps as f32 - 0.5) * sweep + k * sweep * 0.4 };
            let reach = r * (0.55 + 0.45 * ease::out(k));
            let (px, py) = (kx + dir * a.cos() * reach, ky - base + a.sin() * reach * 0.6);
            if bayer(px as i32, py as i32) < 1.0 - k {
                pen.put(px.round() as i32, py.round() as i32, if swing == 2 { 'H' } else { 'i' });
                if swing == 0 {
                    pen.put((px - dir * (i as f32)).round() as i32, py.round() as i32, 'h');
                }
            }
        }
        let _ = tick;
    }
}

/// A knight struck: the spark off the body and a ring; held off: a bright tick of steel.
pub(super) fn hurt(im: &mut Img, c: Ctx, m: &Mark, tick: u64) {
    let Kind::Hurt { guarded, boss } = m.kind else { return };
    let age = m.age(c.now);
    let (kx, ky) = c.at(m.x, m.y);
    let sd = seed(m, 11);
    if guarded {
        let at = (kx + f32::from(m.dir) * 14.0, ky - 26.0);
        let mut pen = Pen::new(im, 160);
        let sparks = Spray { gravity: 0.1, ..Spray::round(sd, 10, at, (1.6, 3.6), (8.0, 14.0)) };
        sparks.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Spark, ramps::BONE, 2, tick));
        if age < 6 {
            pfx::rays(&mut pen, at, 6, 10.0, 1.0, 0.3, ramps::BONE, age as f32 / 6.0);
        }
        return;
    }
    let w = m.kind.weight();
    juice::impact_burst(im, (kx, ky - 26.0), age, sd, if boss { Weight::Kill } else { w }, tick);
    let mut pen = Pen::new(im, 200);
    let drops = Spray { gravity: 0.2, ..Spray::round(sd ^ 0x9, 6, (kx, ky - 30.0), (1.0, 2.6), (12.0, 20.0)) };
    drops.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Droplet, pfx::ramps::BLOOD, 1, tick));
}

/// A slam or a rush that came down: the floor cracks, dust and chips fly; the keeper's makes a
/// bigger crater and thrown stone.
pub(super) fn quake(im: &mut Img, c: Ctx, m: &Mark, tick: u64) {
    let Kind::Quake { boss, rush, hit } = m.kind else { return };
    let age = m.age(c.now);
    let (x, y) = c.at(m.x, m.y);
    let sd = seed(m, 5);
    let mut pen = Pen::new(im, 500);
    if rush {
        let steam = Spray { gravity: -0.05, ..Spray::round(sd, 14, (x, y - 6.0), (0.8, 2.8), (14.0, 26.0)) };
        steam.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Smoke, STEAM, 2, tick));
        return;
    }
    let (n, speed) = if boss { (22, (1.6, 4.4)) } else { (12, (1.2, 3.2)) };
    let chips = Spray { gravity: 0.22, squash: 0.8, ..Spray::round(sd, n, (x, y), speed, (14.0, 26.0)) };
    chips.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Shard, ramps::EARTH, 2, tick));
    let dust = Spray { gravity: -0.03, squash: 0.5, ..Spray::round(sd ^ 0xc3, 10, (x, y), (0.6, 2.0), (16.0, 30.0)) };
    dust.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Smoke, ramps::SMOKE, 3, tick));
    if age < 7 {
        pfx::rays(&mut pen, (x, y - 4.0), 10, if boss { 26.0 } else { 16.0 }, 0.5, 0.4, ramps::EMBER, age as f32 / 7.0);
    }
    let _ = hit;
}

/// A cask breaking on the aisle: staves and hoops thrown, ale slopped in sheets, foam, a ring.
pub(super) fn smash(im: &mut Img, c: Ctx, m: &Mark, tick: u64) {
    let Kind::Smash { hit, .. } = m.kind else { return };
    let age = m.age(c.now);
    let (x, y) = c.at(m.x, m.y);
    let sd = seed(m, 3);
    let mut pen = Pen::new(im, 700);
    let staves = Spray { gravity: 0.24, ..Spray::round(sd, 14, (x, y - 8.0), (1.8, 4.6), (16.0, 30.0)) };
    staves.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Shard, ramps::EARTH, 2, tick));
    let ale = Spray { gravity: 0.2, ..Spray::round(sd ^ 0x44, 24, (x, y - 8.0), (1.0, 4.2), (14.0, 28.0)) };
    ale.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Droplet, ALE, 1, tick));
    let foam = Spray { gravity: -0.03, ..Spray::round(sd ^ 0x99, 8, (x, y - 10.0), (0.4, 1.6), (18.0, 32.0)) };
    foam.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Smoke, ramps::BONE, 2, tick));
    if age < 14 {
        pen.shock((x, y), 4.0 + age as f32 * 3.4, 1.8, 0.42, ALE, age as f32 / 14.0);
        pfx::rays(&mut pen, (x, y - 8.0), 8, 18.0, 0.7, 0.2, ALE, age as f32 / 7.0);
    }
    if hit && age < 12 {
        pen.shock((x, y), 8.0 + age as f32 * 5.0, 2.2, 0.4, ramps::SMOKE, age as f32 / 12.0);
    }
}

/// Bread eaten: hearts rise off the knight, sparkles ring them.
pub(super) fn bread(im: &mut Img, c: Ctx, m: &Mark, tick: u64) {
    let age = m.age(c.now);
    let (x, y) = c.at(m.x, m.y);
    let sd = seed(m, 9);
    let mut pen = Pen::new(im, 160);
    let hearts = Spray { gravity: -0.05, drag: 0.08, ..Spray::round(sd, 6, (x, y - 30.0), (0.3, 1.2), (24.0, 38.0)) };
    hearts.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Heart, ramps::ROSE, 2, tick));
    let glints = Spray { gravity: -0.02, ..Spray::round(sd ^ 0x13, 8, (x, y - 24.0), (0.8, 2.0), (14.0, 22.0)) };
    glints.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Star, ramps::GOLD, 1, tick));
}

/// The knight lifted a cask: a pop of dust at the hands; it left them: a ring and long lines.
pub(super) fn heft(im: &mut Img, c: Ctx, m: &Mark, tick: u64) {
    let age = m.age(c.now);
    if age > 14 {
        return;
    }
    let (x, y) = c.at(m.x, m.y);
    let sd = seed(m, 2);
    let mut pen = Pen::new(im, 200);
    match m.kind {
        Kind::Grab => {
            pen.shock((x, y + 1.0), 3.0 + age as f32 * 2.0, 1.4, 0.4, ramps::SMOKE, age as f32 / 14.0);
            let dust = Spray { gravity: -0.03, squash: 0.5, ..Spray::round(sd, 8, (x, y), (0.6, 1.8), (10.0, 18.0)) };
            dust.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Smoke, ramps::SMOKE, 2, tick));
        }
        Kind::Throw => {
            let dir = f32::from(m.dir);
            pen.shock((x + dir * 12.0, y - 30.0), 3.0 + age as f32 * 3.0, 1.4, 1.0, ramps::BONE, age as f32 / 12.0);
            for i in 0..4 {
                let off = i as f32 * 4.0 - 6.0;
                pen.streak((x + dir * (14.0 + age as f32 * 6.0), y - 32.0 + off), (dir, 0.0), 12.0, ramps::BONE, age as f32 / 12.0);
            }
        }
        _ => {}
    }
}

/// A step: a puff of dust at the heel, rising and thinning.
pub(super) fn step(im: &mut Img, c: Ctx, m: &Mark, tick: u64) {
    let age = m.age(c.now);
    let (x, y) = c.at(m.x, m.y);
    let sd = seed(m, 1);
    let mut pen = Pen::new(im, 60);
    let dust = Spray { gravity: -0.03, squash: 0.4, ..Spray::round(sd, 4, (x - f32::from(m.dir) * 8.0, y - 1.0), (0.2, 0.9), (10.0, 14.0)) };
    dust.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Smoke, ramps::SMOKE, 1, tick));
}

/// A fall: the knight hits the floor with a ring and a cloud; a rise: stars wheel off.
pub(super) fn tumble(im: &mut Img, c: Ctx, m: &Mark, tick: u64) {
    let age = m.age(c.now);
    let (x, y) = c.at(m.x, m.y);
    let sd = seed(m, 4);
    let mut pen = Pen::new(im, 260);
    match m.kind {
        Kind::Fall if age < 40 => {
            pen.shock((x, y + 2.0), 4.0 + age as f32 * 2.4, 2.0, 0.4, ramps::SMOKE, (age as f32 / 20.0).min(1.0));
            // Stars wheel over the downed knight.
            for i in 0..4 {
                let a = age as f32 * 0.22 + i as f32 * 1.57;
                let (sx, sy) = (x + a.cos() * 11.0, y - 38.0 + a.sin() * 3.5);
                pen.light(sx.round() as i32, sy.round() as i32, '6');
                pen.light(sx.round() as i32 + 1, sy.round() as i32, '@');
            }
        }
        Kind::Rise if age < 18 => {
            let dust = Spray { gravity: -0.03, squash: 0.5, ..Spray::round(sd, 10, (x, y), (0.8, 2.4), (10.0, 18.0)) };
            dust.each(age as f32, |mt| pfx::mote(&mut pen, &mt, Shape::Smoke, ramps::SMOKE, 2, tick));
        }
        _ => {}
    }
}

/// A thrown cask in the air: it leaves the knight's hands high and spinning, and runs off the top
/// of the screen toward the next aisle.
pub(super) fn thrown(im: &mut Img, c: Ctx, cask: &Img, m: &Mark) {
    if m.kind != Kind::Throw {
        return;
    }
    let age = m.age(c.now);
    const FLIGHT: u32 = 16;
    if age >= FLIGHT {
        return;
    }
    let t = age as f32 / FLIGHT as f32;
    let (x, y) = c.at(m.x, m.y);
    let dir = f32::from(m.dir);
    let px = x + dir * (12.0 + 150.0 * ease::in_quad(t));
    let py = y - 34.0 - 70.0 * (1.0 - (1.0 - t * 1.3).powi(2)).max(0.0) - t * 90.0;
    let img = if (age / 2) % 2 == 0 { cask.clone() } else { cask.flip_h() };
    im.stamp(&img, px as i32 - img.w / 2, py as i32 - img.h / 2);
    // Its wake: ale flecks and a streak of air.
    let mut pen = Pen::new(im, 80);
    for i in 1..5 {
        let tt = (t - i as f32 * 0.05).max(0.0);
        let (tx, ty) = (x + dir * (12.0 + 150.0 * ease::in_quad(tt)), y - 34.0 - 70.0 * (1.0 - (1.0 - tt * 1.3).powi(2)).max(0.0) - tt * 90.0);
        pen.light(tx as i32, ty as i32, ['T', 't', 'O', 'o'][i as usize - 1]);
    }
}

/// A rim of red round the stage when the knight is down to its last hearts or a hit just landed.
pub(super) fn rim(im: &mut Img, c: Ctx, tick: u64) {
    let d = c.d;
    let hurt = d.show.marks.iter().filter(|m| matches!(m.kind, Kind::Hurt { guarded: false, .. })).map(|m| m.age(c.now)).min();
    let mut k = 0.0f32;
    if let Some(a) = hurt.filter(|&a| a < 12) {
        k = k.max(0.55 * (1.0 - a as f32 / 12.0));
    }
    if d.hp <= 2 && d.down == 0 {
        k = k.max(0.16 + 0.1 * ((tick / 5) % 2) as f32);
    }
    if k > 0.0 {
        juice::vignette(im, (0, TOP, W, BOTTOM - TOP), k, 10, 'p');
    }
}
