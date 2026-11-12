//! How a blow, a lock and a parry feel on Lanternward's screen. The rules
//! (Bend) keep only whole-number counters (a foe's flinch, a knight's freeze,
//! the effects' own ages); everything here is a pure function of those ages,
//! so the host, a friend's mirror and a browser guest draw the same pixels.
//! The brackets close on a take, the blade leaves a crescent, a struck body
//! stands white and squashes, a parry rings, a kill bursts.
use super::super::super::ink::{Img, bayer, hash};
use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::scene::{View, polar};
use crate::rules::{
    juice::Weight,
    lanternward::{self as lw, Act, Fk, Fs, Fxk, Lock, Seat, world::{Q, angle_of}},
};

/// How hard the blow that struck a foe weighed, from what the foe and the
/// knights about it are doing (the rules' own counters, nothing kept).
pub(super) fn foe_weight_for(s: &lw::Session, f: &lw::Foe) -> Weight {
    if f.st == Fs::Dying {
        return Weight::Kill;
    }
    let near = |k: &Seat| {
        let (dx, dy) = ((k.x - f.x) / Q, (k.y - f.y) / Q);
        dx * dx + dy * dy < 46 * 46
    };
    let heavy = s.seats.values().any(|k| {
        near(k)
            && match k.act {
                Act::Slash => k.n >= 2,
                Act::Spin | Act::Plunge | Act::Lunge => true,
                _ => false,
            }
    });
    if heavy || f.open {
        Weight::Crushing
    } else if f.hp * 3 <= f.full {
        Weight::Solid
    } else {
        Weight::Solid
    }
}

/// A blow's weight on the Warden, by what the blade did.
pub(super) fn boss_weight_for(s: &lw::Session, w: &lw::Warden) -> Weight {
    if w.hp == 0 {
        return Weight::Finale;
    }
    let heavy = s.seats.values().any(|k| match k.act {
        Act::Slash => k.n >= 2,
        Act::Spin | Act::Plunge | Act::Lunge => true,
        _ => false,
    });
    if heavy || w.st == lw::Ws::Stagger { Weight::Crushing } else { Weight::Solid }
}

/// The ticks since a flinch began (0 on the blow), if one is on.
pub(super) fn hit_age(flinch: u8) -> Option<u32> {
    (flinch > 0).then(|| u32::from(lw::FLINCH_TICKS.saturating_sub(flinch)))
}

// ----- lock-on --------------------------------------------------------------------

/// Four brackets close on what a knight has locked, a wick-spark floats over
/// it, and a thin dotted rope runs from the knight to it in the knight's own
/// view. A friend's lock is two small brackets in the friend's colour.
pub(super) fn lock_on(im: &mut Img, v: &View) {
    for (&id, k) in &v.s.seats {
        let Some(l) = k.lock else { continue };
        let Some((tx, ty)) = v.s.lock_point(l) else { continue };
        let (x, y) = (v.x(tx), v.y(ty));
        let (open, held, top) = match l {
            Lock::Boss => (30, 17, -17),
            Lock::Foe(fid) => match v.s.foes.iter().find(|f| f.id == fid).map(|f| f.kind) {
                Some(Fk::Turret) => (22, 12, -14),
                _ => (20, 10, -12),
            },
            Lock::Prop(_) => (18, 9, -13),
        };
        let age = k.lock_t.min(255);
        let ink = if id == v.id { '6' } else { super::super::knights::seat_cloth(id) };
        // The brackets close in six ticks, overshoot a pixel and settle.
        let close = (age as f32 / lw::SNAP_TICKS as f32).min(1.0);
        let eased = 1.0 - (1.0 - close) * (1.0 - close);
        let mut r = open as f32 + (held - open) as f32 * eased;
        if age >= lw::SNAP_TICKS && age < lw::SNAP_TICKS + 4 {
            r -= 1.0;
        }
        // Then a slow breath.
        r += [0.0, 0.0, 1.0, 1.0][(v.tick / 5 % 4) as usize] * f32::from(age >= lw::SNAP_TICKS);
        let r = r.round() as i32;
        let bright = if age < 3 { 'w' } else { ink };
        let arm = if l == Lock::Boss { 5 } else { 3 };
        let me = id == v.id;
        for (sx, sy) in [(-1i32, -1i32), (1, -1), (-1, 1), (1, 1)] {
            if !me && (sx + sy).abs() != 2 {
                continue;
            }
            let (cx, cy) = (x + sx * r, y + sy * (r * 4 / 5));
            for i in 0..=arm {
                // the shadow first, so a bracket reads on any floor
                im.put(cx - sx * i + 1, cy + 1, 'k');
                im.put(cx + 1, cy - sy * i + 1, 'k');
            }
            for i in 0..=arm {
                im.put(cx - sx * i, cy, bright);
                im.put(cx, cy - sy * i, bright);
            }
        }
        if me {
            // The take: four ticks fly in from the corners.
            if age < 5 {
                let d = (open + 6 - age as i32 * 4).max(r + 2);
                for (sx, sy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    im.put(x + sx * d, y + sy * (d * 4 / 5), 'w');
                    im.put(x + sx * (d + 2), y + sy * ((d + 2) * 4 / 5), '6');
                }
            }
            // The wick-spark over it.
            let bob = [0, -1, -2, -1][(v.tick / 3 % 4) as usize];
            super::bank::slot_mid(im, "mark", "idle", v.tick / 6, x, y + top + bob - (r - held).max(0) / 2);
            // A rope of light from the knight's lantern to the lock.
            if let Some(me_k) = v.s.seats.get(&id) {
                rope(im, (v.x(me_k.x), v.y(me_k.y) - 4), (x, y), r + 3, v.tick);
            }
        }
    }
}

/// A dotted rope between two points, a dot in three, drifting toward the
/// target; it ends short of the brackets and starts clear of the knight.
fn rope(im: &mut Img, a: (i32, i32), b: (i32, i32), stop: i32, tick: u32) {
    let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
    let len = dx.hypot(dy);
    if len < 30.0 + stop as f32 {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let from = 12;
    let to = len as i32 - stop;
    for d in from..to {
        if (d - (tick as i32 / 2)).rem_euclid(5) == 0 {
            let (x, y) = ((a.0 as f32 + ux * d as f32).round() as i32, (a.1 as f32 + uy * d as f32).round() as i32);
            let near = (d as f32 / len * 2.0).min(1.0);
            if bayer(x, y) < 0.35 + near * 0.55 {
                im.put(x, y, '4');
            }
        }
    }
}

// ----- the blade ------------------------------------------------------------------

/// The sword's crescent, the spin's ring, the lunge's streak and the plunge's
/// blade: bright at the head, thinning and cooling behind it.
pub(super) fn swing_arc(im: &mut Img, k: &Seat, x: i32, y: i32) {
    let face = k.face;
    let base = angle_of(face.0, face.1);
    match k.act {
        Act::Slash => {
            let n = usize::from(k.n.min(2));
            let (wind, active) = ([2, 2, 4][n], [3, 3, 4][n]);
            if k.t as i32 > wind - 1 && k.t as i32 <= wind + active + 3 {
                let reach = [20, 20, 25][n];
                let thick = [5, 5, 9][n];
                let sweep = if n == 1 { -1 } else { 1 };
                let span = if n == 2 { 160 } else { 118 };
                let progress = ((k.t as i32 - wind).max(0) * 100 / active).min(135);
                let steps = 34;
                for st in 0..steps {
                    let f = st * 100 / steps;
                    let behind = progress - f;
                    if !(0..=75).contains(&behind) {
                        continue;
                    }
                    let a = base + sweep * (f - 50) * span / 100;
                    let t = (thick * (100 - behind * 125 / 100) / 100).max(1);
                    for r in (reach - t)..=reach {
                        let (px, py) = polar(x, y, a, r, 90);
                        // A dithered tail: the far end of the trail is stipple.
                        let tail = behind > 48 && bayer(px, py) < (behind - 48) as f32 / 40.0;
                        if tail {
                            continue;
                        }
                        let ink = if behind < 12 {
                            'w'
                        } else if behind < 26 {
                            'W'
                        } else if behind < 46 {
                            'V'
                        } else {
                            'U'
                        };
                        im.put(px, py, ink);
                    }
                    // The heavy cut burns gold along its outer edge.
                    if n == 2 && behind < 36 {
                        let (px, py) = polar(x, y, a, reach + 1, 90);
                        im.put(px, py, if behind < 14 { '6' } else { '4' });
                    }
                }
                // A flick of sparks at the head of the arc.
                if progress < 130 && n == 2 {
                    let a = base + sweep * (progress.min(100) - 50) * span / 100;
                    let (px, py) = polar(x, y, a, reach + 3, 90);
                    im.put(px, py, 'w');
                    im.put(px + 1, py - 1, '6');
                }
            }
        }
        Act::Spin => {
            let r = (8 + k.t as i32 * 2).min(24);
            for a in 0..56 {
                let deg = a * 360 / 56 + k.t as i32 * 58;
                let lead = (a + k.t as i32 * 5) % 14;
                let w = if lead < 4 { 3 } else if lead < 8 { 2 } else { 1 };
                for rr in (r - w)..=r {
                    let (px, py) = polar(x, y, deg, rr, 90);
                    let ink = if lead < 3 { 'w' } else if lead < 7 { 'V' } else { 'U' };
                    if lead < 11 || bayer(px, py) < 0.5 {
                        im.put(px, py, ink);
                    }
                }
            }
        }
        Act::Lunge if (lw::LUNGE_WIND..lw::LUNGE_WIND + lw::LUNGE_ACTIVE).contains(&k.t) => {
            let len = 26;
            let (ux, uy) = (face.0 as f32 / Q as f32, face.1 as f32 / Q as f32);
            // a streak behind the point, then the blade
            for d in 4..len {
                let (px, py) = ((x as f32 + ux * d as f32).round() as i32, (y as f32 + uy * d as f32).round() as i32);
                im.put(px, py, if d > len - 7 { 'w' } else { 'W' });
                if d > len - 11 {
                    let (sx, sy) = (-(uy * 1.0).round() as i32, (ux * 1.0).round() as i32);
                    im.put(px + sx, py + sy, 'V');
                }
            }
            let (tx, ty) = ((x as f32 + ux * (len + 2) as f32).round() as i32, (y as f32 + uy * (len + 2) as f32).round() as i32);
            im.put(tx, ty, 'w');
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                im.put(tx + dx, ty + dy, '6');
            }
        }
        Act::Plunge => {
            im.line(x, y - 18, x, y + 2, 'W');
            im.line(x - 1, y - 14, x - 1, y, 'V');
            im.line(x + 1, y - 14, x + 1, y, 'V');
            im.put(x, y + 3, 'w');
        }
        _ => {}
    }
}

// ----- effects --------------------------------------------------------------------

/// Every effect the rules left in the room, drawn from its age.
pub(super) fn effects(im: &mut Img, v: &View) {
    // The words thrown up this frame, so two parries side by side never overprint.
    let mut words: Vec<(i32, i32, i32, i32)> = Vec::new();
    for (i, fx) in v.s.fx.iter().enumerate() {
        let (x, y) = (v.x(fx.x), v.y(fx.y));
        if x < -40 || y < -40 || x > 424 || y > 264 {
            continue;
        }
        let t = u32::from(fx.t);
        let seed = hash(fx.x / Q, fx.y / Q, i as u32 + 3);
        let at = (x as f32, y as f32);
        let tick = u64::from(v.tick);
        match fx.kind {
            Fxk::Puff => {
                let mut pen = Pen::new(im, 260);
                let s = Spray { gravity: -0.05, ..Spray::round(seed, 8, (at.0, at.1 - 2.0), (0.3, 0.9), (10.0, 16.0)) };
                s.each(t as f32, |m| pfx::mote(&mut pen, &m, Shape::Smoke, ramps::SMOKE, 2, tick));
            }
            Fxk::Light => {
                let mut pen = Pen::new(im, 200);
                let s = Spray { gravity: -0.09, ..Spray::round(seed, 9, (at.0, at.1 - 4.0), (0.3, 1.2), (14.0, 22.0)) };
                s.each(t as f32, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::GOLD, 1, tick));
                if t < 5 {
                    let mut pen = Pen::new(im, 80);
                    pen.twinkle((at.0, at.1 - 8.0 - t as f32), 4.0 - t as f32 * 0.5, t as f32 * 0.6, ramps::GOLD);
                }
            }
            Fxk::Spark => {
                juice::impact_burst(im, (at.0, at.1 - 4.0), t, seed, Weight::Solid, tick);
            }
            Fxk::Ring => {
                let mut pen = Pen::new(im, 240);
                pen.shock((at.0, at.1), 4.0 + t as f32 * 2.2 + f32::from(fx.a) * 0.25, 1.3, 0.55, ramps::FROST, t as f32 / 12.0);
            }
            Fxk::Parry => parry(im, v, &mut words, at, t, seed, tick),
            Fxk::Tick => {
                let mut pen = Pen::new(im, 120);
                pen.shock((at.0, at.1 - 6.0), 3.0 + t as f32 * 1.6, 1.0, 0.7, ramps::GOLD, t as f32 / 10.0);
            }
            Fxk::Dust => {
                let mut pen = Pen::new(im, 200);
                let s = Spray { gravity: -0.02, squash: 0.5, ..Spray::round(seed, 8, (at.0, at.1 + 1.0), (0.5, 1.4), (8.0, 14.0)) };
                s.each(t as f32, |m| pfx::mote(&mut pen, &m, Shape::Smoke, ramps::EARTH.cooler(1), 1, tick));
            }
            Fxk::Solved => {
                let big = fx.a > 0;
                let k = t as f32 / 22.0;
                let mut pen = Pen::new(im, 500);
                pfx::rays(&mut pen, (at.0, at.1 - 2.0), if big { 14 } else { 10 }, if big { 34.0 } else { 22.0 }, 0.9, 0.2, ramps::GOLD, k);
                pen.shock((at.0, at.1), 5.0 + t as f32 * if big { 3.0 } else { 2.0 }, 1.4, 0.6, ramps::HOLY, k);
                let s = Spray { gravity: 0.1, ..Spray::round(seed, if big { 24 } else { 14 }, (at.0, at.1 - 4.0), (1.0, 2.8), (14.0, 24.0)) };
                s.each(t as f32, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, tick));
            }
            Fxk::Blast => {
                let mut pen = Pen::new(im, 900);
                let k = t as f32 / 22.0;
                if t < 4 {
                    pen.glow(at, 18.0 - t as f32 * 2.0, 0.8, ramps::FIRE, 1.0);
                }
                pen.shock(at, 4.0 + t as f32 * 3.0, 2.4, 0.8, ramps::FIRE, k);
                let debris = Spray { gravity: 0.2, ..Spray::round(seed, 16, (at.0, at.1 - 3.0), (1.4, 3.8), (12.0, 22.0)) };
                debris.each(t as f32, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::EMBER, 1, tick));
                let smoke = Spray { gravity: -0.05, ..Spray::round(seed ^ 0x71, 8, (at.0, at.1 - 4.0), (0.4, 1.4), (16.0, 28.0)) };
                smoke.each(t as f32, |m| pfx::mote(&mut pen, &m, Shape::Smoke, ramps::SMOKE, 2, tick));
            }
            Fxk::Wave => {
                let mut pen = Pen::new(im, 500);
                let r = 6.0 + t as f32 * 5.0;
                pen.shock(at, r, 2.6, 0.62, ramps::EARTH, t as f32 / 20.0);
                pen.shock(at, (r - 5.0).max(2.0), 1.2, 0.62, ramps::SMOKE, t as f32 / 16.0);
                let s = Spray { gravity: 0.12, squash: 0.5, ..Spray::round(seed, 12, (at.0, at.1), (1.5, 3.2), (8.0, 14.0)) };
                s.each(t as f32, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::EARTH, 1, tick));
            }
        }
    }
}

/// A parry: a white flash, a ring and rays, a chime of sparks, and the word
/// rising off it.
fn parry(im: &mut Img, v: &View, words: &mut Vec<(i32, i32, i32, i32)>, at: (f32, f32), t: u32, seed: u32, tick: u64) {
    let k = t as f32 / 16.0;
    let mut pen = Pen::new(im, 700);
    if t < 3 {
        let r = 9 - t as i32 * 2;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    pen.put(at.0 as i32 + dx, at.1 as i32 + dy, if dx * dx + dy * dy < r * r / 2 { '9' } else { 'c' });
                }
            }
        }
    }
    pfx::rays(&mut pen, at, 12, 24.0, 0.9, seed as f32 * 0.1, ramps::FROST, k);
    pen.shock(at, 3.0 + t as f32 * 2.6, 1.6, 0.8, ramps::FROST, k);
    pen.shock(at, 2.0 + t as f32 * 1.5, 1.0, 0.8, ramps::HOLY, k * 1.2);
    let s = Spray { gravity: 0.1, ..Spray::round(seed, 14, at, (1.4, 3.2), (10.0, 18.0)) };
    s.each(t as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::HOLY, 2, tick));
    drop(pen);
    if t < 22 {
        pop_word(im, v, words, "PARRY", (at.0 as i32, at.1 as i32 - 10), t);
    }
}

/// A word that rises off a spot, kept clear of the top bar, the bars and
/// hints at the foot, and whatever card is up: it is not drawn where it would
/// meet other words.
fn pop_word(im: &mut Img, v: &View, words: &mut Vec<(i32, i32, i32, i32)>, word: &str, at: (i32, i32), age: u32) {
    let at = (at.0, at.1.clamp(66, 172));
    let hop = (crate::motion::ease::out((age as f32 / 7.0).min(1.0)) * 12.0) as i32 + (age as i32 * 3 / 5);
    let (w, h) = (super::super::super::ink::text_width(word) + 4, 11);
    let rect = (at.0 - w / 2, at.1 - hop - h, w, h);
    let meets = |a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)| a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
    if rect.0 < 2 || rect.0 + rect.2 > 382 || words.iter().any(|&o| meets(rect, o)) || super::moments::card_box(v).is_some_and(|c| meets(rect, c)) {
        return;
    }
    words.push(rect);
    super::hud::claim("pop", rect);
    juice::pop_number(im, word, at, age, 22, 1, 'w');
}

// ----- small marks ----------------------------------------------------------------

/// Stars wheeling round a dazed head, `n` of them.
pub(super) fn stars(im: &mut Img, x: i32, y: i32, tick: u32, n: i32, r: i32) {
    for a in 0..n {
        let deg = a * 360 / n + tick as i32 * 14;
        let (px, py) = polar(x, y, deg, r, 40);
        im.put(px, py, '6');
        im.put(px - 1, py, '4');
        im.put(px + 1, py, '4');
        im.put(px, py - 1, 'w');
        im.put(px, py + 1, '4');
    }
}

/// A blinking "!" over a foe about to strike.
pub(super) fn exclaim(im: &mut Img, x: i32, y: i32, tick: u32) {
    if (tick / 2).is_multiple_of(2) {
        return;
    }
    im.rect(x - 2, y - 1, 5, 11, 'k');
    im.rect(x - 1, y, 3, 6, '7');
    im.rect(x, y, 1, 5, 'c');
    im.rect(x - 1, y + 7, 3, 2, '7');
}

/// A small lantern over a hooded snuffer: the light that would burn its hood off.
pub(super) fn lamp_icon(im: &mut Img, x: i32, y: i32, tick: u32) {
    im.rect(x - 3, y - 2, 7, 10, 'k');
    im.rect(x - 1, y - 1, 3, 1, 'g');
    im.rect(x - 2, y, 5, 5, 'g');
    let lit = (tick / 6).is_multiple_of(2);
    im.rect(x - 1, y + 1, 3, 3, if lit { '6' } else { 'R' });
    im.put(x, y + 2, if lit { 'w' } else { '5' });
    im.rect(x - 1, y + 5, 3, 1, 'X');
}

/// The turret's barrel, laid over its lid toward the footprint.
pub(super) fn barrel(im: &mut Img, x: i32, y: i32, face: (i32, i32), hot: bool, recoil: i32) {
    let (ux, uy) = (face.0 as f32 / Q as f32, face.1 as f32 / Q as f32);
    let len = 11 - recoil;
    for d in 2..=len {
        let (px, py) = ((x as f32 + ux * d as f32).round() as i32, (y as f32 + uy * d as f32).round() as i32);
        let (sx, sy) = ((-uy).round() as i32, ux.round() as i32);
        im.put(px + sx + 1, py + sy + 1, 'k');
        im.put(px - sx, py - sy, 'k');
        im.put(px + sx, py + sy, 'k');
        im.put(px, py, 'g');
        im.put(px - sx, py - sy, 'j');
    }
    let (mx, my) = ((x as f32 + ux * len as f32).round() as i32, (y as f32 + uy * len as f32).round() as i32);
    im.rect(mx - 1, my - 1, 3, 3, 'k');
    im.put(mx, my, if hot { 'w' } else { 'R' });
}

/// A friend's flare kindles yours: while one knight flares and another with a
/// lantern stands within its reach, motes of light drift from the flare to
/// the friend, and a small ring warms about them both.
pub(super) fn kindle(im: &mut Img, v: &View) {
    if !v.s.inv.lantern {
        return;
    }
    for (&a, ka) in &v.s.seats {
        if !ka.flaring() {
            continue;
        }
        for (&b, kb) in &v.s.seats {
            if a == b || kb.act == Act::Down {
                continue;
            }
            let (dx, dy) = ((kb.x - ka.x) / Q, (kb.y - ka.y) / Q);
            if dx * dx + dy * dy > 28 * 28 {
                continue;
            }
            let (ax, ay, bx, by) = (v.x(ka.x), v.y(ka.y) - 8, v.x(kb.x), v.y(kb.y) - 8);
            for m in 0..5 {
                let t = ((v.tick as i32 * 2 + m * 12) % 60) as f32 / 60.0;
                let (x, y) = (ax as f32 + (bx - ax) as f32 * t, ay as f32 + (by - ay) as f32 * t - (t * (1.0 - t) * 14.0));
                im.put(x.round() as i32, y.round() as i32, if m % 2 == 0 { '6' } else { 'w' });
                im.put(x.round() as i32, y.round() as i32 + 1, '4');
            }
            super::scene::ring_px(im, bx, by + 6, 10 + (v.tick as i32 / 3 % 3), '4', v.tick);
        }
    }
}
