//! What the table does when something happens: the fireflies in the dark,
//! the lit lamps' breathing, the rings and sparks off a bumper, the comet
//! behind the ball, the kick of a flipper, the keeper's petals and fall, the
//! drain's sting, the bell's rim and the jackpot's coins. Every one is a pure
//! function of the ball's own ticks or of the show's ages, so the host, a
//! friend's mirror and a browser guest draw the same pixels.

use super::super::super::ink::{Img, bayer, hash, ink};
use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::{OX, OY, field_x};
use crate::motion::math::{cos, sin};
use crate::rules::juice::Weight;
use crate::rules::mossbell::{BUMPERS, Ball, Q};
use crate::rules::mossbell_show::MossShow;

/// The playfield's black window on the screen: left, top, width, height.
pub(super) const WINDOW: (i32, i32, i32, i32) = (OX + 9, OY + 6, 236, 131);

/// A point of the table (physics pixels) as a pixel of the screen.
pub(super) fn at(x: i32, y: i32) -> (f32, f32) {
    ((OX + field_x(x)) as f32, (OY + y) as f32)
}

/// Where the keeper stands on the screen, and where the ball meets it.
pub(super) const KEEPER: (f32, f32) = (192.0, 73.0);

/// Fireflies drifting in the dark of the table, blinking.
pub(super) fn ambient(w: &mut Img, tick: u64) {
    let (x0, y0, ww, wh) = WINDOW;
    for k in 0..9 {
        let h = hash(k, 5, 0x42);
        let t = tick as f32 * 0.021 + k as f32 * 1.7;
        let x = x0 + 8 + (h % (ww as u32 - 16)) as i32 + (sin(t * 1.3 + k as f32) * 13.0) as i32;
        let y = y0 + 10 + ((h >> 9) % (wh as u32 - 40)) as i32 + (cos(t * 0.9 + k as f32 * 2.0) * 8.0) as i32;
        let blink = (tick as u32 / 7 + h % 11) % 11;
        if blink < 5 {
            w.put(x, y, if blink % 2 == 0 { 'Y' } else { 'y' });
            if blink == 2 {
                w.put(x - 1, y, 'l');
                w.put(x + 1, y, 'l');
            }
        }
    }
}

/// Light falling through the conservatory glass: three slow diagonal shafts of dither on
/// the black of the table, swaying, thinning as they go down.
pub(super) fn shafts(w: &mut Img, tick: u64) {
    let (x0, y0, ww, wh) = WINDOW;
    let shade = ink('x').unwrap();
    for beam in 0..3 {
        let sway = sin(tick as f32 * 0.012 + beam as f32 * 2.1) * 16.0;
        for row in 0..wh {
            let y = y0 + row;
            let down = row as f32 / wh as f32;
            let cx = x0 as f32 + 34.0 + beam as f32 * 84.0 + row as f32 * 0.5 + sway;
            let half = 8.0 + row as f32 * 0.06;
            let k = 0.34 * (1.0 - down * 0.75);
            let (lo, hi) = (((cx - half) as i32).max(x0), ((cx + half) as i32).min(x0 + ww - 1));
            // Only the dither's lowest steps can pass under `k`: walk those columns alone.
            for phase in 0..4 {
                if bayer(phase, y) >= k {
                    continue;
                }
                let mut x = lo + (phase - lo).rem_euclid(4);
                while x <= hi {
                    let edge = 1.0 - ((x as f32 - cx).abs() / half);
                    if bayer(x, y) < k * edge {
                        w.set(x, y, shade);
                    }
                    x += 4;
                }
            }
        }
    }
}

/// A lit lamp breathes: a dithered ring just outside its bumper, swelling
/// and fading, under the sprite.
pub(super) fn lamps(w: &mut Img, b: &Ball, tick: u64) {
    let (hot, warm) = (ink('t').unwrap(), ink('o').unwrap());
    for (i, &(bx, by, _)) in BUMPERS.iter().enumerate() {
        if b.lit & (1 << i) == 0 {
            continue;
        }
        let (cx, cy) = at(bx, by);
        let (cx, cy) = (cx as i32, cy as i32);
        let pulse = 0.5 + 0.5 * sin(tick as f32 * 0.2 + i as f32 * 2.0);
        let (r0, r1) = (14.5f32, 18.0 + pulse * 2.0);
        let k = 0.55 - pulse * 0.2;
        // Each row's two stretches of the ring band, and nothing else.
        for dy in -(r1 as i32)..=r1 as i32 {
            let dy2 = (dy * dy) as f32;
            let outer = (r1 * r1 - dy2).max(0.0).sqrt();
            let inner = (r0 * r0 - dy2).max(0.0).sqrt();
            for side in [-1, 1] {
                let (from, to) = (inner.ceil() as i32, outer as i32);
                for off in from..=to {
                    let dx = side * off;
                    let d = ((dx * dx) as f32 + dy2).sqrt();
                    if d < r0 || d >= r1 {
                        continue;
                    }
                    let (x, y) = (cx + dx, cy + dy);
                    if bayer(x, y) < k * (1.0 - (d - r0) / (r1 - r0)) {
                        w.set(x, y, if d < r0 + 1.5 { hot } else { warm });
                    }
                }
            }
        }
    }
}

/// A bumper struck: a shock ring leaving it and a spray of sparks.
pub(super) fn bumper_hits(w: &mut Img, b: &Ball, tick: u64) {
    for (i, &(bx, by, _)) in BUMPERS.iter().enumerate() {
        if b.flash[i] == 0 {
            continue;
        }
        let age = 12 - u32::from(b.flash[i]);
        let c = at(bx, by);
        let mut pen = Pen::new(w, 1200);
        let seed = hash(i as i32, b.score as i32, 1);
        pen.shock(c, 9.0 + age as f32 * 3.2, 2.2, 1.0, ramps::GOLD, age as f32 / 12.0);
        let petals = Spray { gravity: 0.05, ..Spray::round(seed ^ 0x77, 9, c, (0.9, 2.4), (10.0, 20.0)) };
        petals.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Petal, ramps::NATURE, 1, tick));
        juice::impact_burst(w, c, age, seed, Weight::Solid, tick);
    }
}

/// What the last contact was.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Contact {
    Flipper,
    Keeper,
    Drain,
    Bumper,
    Jackpot,
}

pub(super) fn contact_kind(b: &Ball) -> Contact {
    let (x, y) = b.impact;
    if (x, y) == (80, 134) {
        Contact::Drain
    } else if (x, y) == (80, 63) {
        Contact::Jackpot
    } else if (x, y) == (80, 33) {
        Contact::Keeper
    } else if BUMPERS.iter().any(|&(bx, by, _)| (bx, by) == (x, y)) {
        Contact::Bumper
    } else {
        Contact::Flipper
    }
}

/// A flipper catches the ball: sparks off the paddle and a puff of dust.
pub(super) fn catch(w: &mut Img, b: &Ball, tick: u64) {
    if b.burst == 0 || contact_kind(b) != Contact::Flipper {
        return;
    }
    let age = 12 - u32::from(b.burst.min(12));
    let c = at(b.impact.0, b.impact.1 + 3);
    let seed = hash(b.impact.0, b.impact.1, 7);
    juice::impact_burst(w, c, age, seed, Weight::Solid, tick);
    let mut pen = Pen::new(w, 200);
    let dust = Spray { gravity: -0.02, squash: 0.5, ..Spray::round(seed ^ 9, 8, (c.0, c.1 + 3.0), (0.5, 1.6), (10.0, 18.0)) };
    dust.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Smoke, ramps::SMOKE, 1, tick));
}

/// A flipper swung: a swoosh through its arc and a glint at the tip, for the
/// first ticks of the swing.
pub(super) fn swoosh(w: &mut Img, b: &Ball, tick: u64) {
    for (swing, pivot, dir) in [(b.left, (37, 119), 1.0f32), (b.right, (123, 119), -1.0)] {
        if swing < 3 {
            continue;
        }
        let age = 5 - swing;
        let (px, py) = at(pivot.0, pivot.1);
        let reach = 53.0;
        // From the rest angle, a little below level, up to the raised one.
        let (from, to) = (0.21f32, -0.15f32);
        let mut pen = Pen::new(w, 160);
        for step in 0..=8 {
            let t = step as f32 / 8.0;
            if t > 0.25 * (age as f32 + 2.0) {
                break;
            }
            let a = from + (to - from) * t;
            let (x, y) = (px + dir * cos(a) * reach, py + sin(a) * reach - 6.0);
            if step % 2 == 0 || age == 0 {
                pen.light(x as i32, y as i32, if t > 0.7 { 'H' } else { 'T' });
            }
        }
        if age < 2 {
            let (tx, ty) = (px + dir * reach, py - 14.0);
            pen.twinkle((tx, ty), 2.0, age as f32 / 3.0, ramps::GOLD);
            let _ = tick;
        }
    }
}

/// The ball's comet: a tail of embers behind it, longer the faster it goes.
pub(super) fn comet(w: &mut Img, b: &Ball, tick: u64) {
    let speed = (b.vx.abs() + b.vy.abs()) / 4;
    if speed < 120 {
        return;
    }
    let mut pts: Vec<(f32, f32)> = Vec::with_capacity(5);
    let (hx, hy) = at(b.x / Q, b.y / Q);
    pts.push((hx, hy));
    for &(tx, ty) in &b.trail {
        pts.push(at(tx, ty));
    }
    let mut pen = Pen::new(w, 220);
    let hot = speed > 900;
    for (n, pair) in pts.windows(2).enumerate() {
        let (a, c) = (pair[0], pair[1]);
        let steps = (a.0 - c.0).abs().max((a.1 - c.1).abs()).ceil().max(1.0) as i32;
        if steps > 20 {
            continue;
        }
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let (x, y) = (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t);
            let k = (n as f32 + t) / 4.0;
            if bayer(x as i32, y as i32) < k * 0.8 {
                continue;
            }
            let ramp = if hot { ramps::GOLD } else { ramps::EMBER };
            pen.light(x as i32, y as i32, ramp.dithered(0.25 + k * 0.7, x as i32, y as i32));
        }
    }
    // A glint on the marble's shoulder, twice a second.
    if tick / 8 % 3 == 0 {
        pen.light(hx as i32 - 2, hy as i32 - 2, 'H');
    }
}

/// The keeper's petals and sparks, and its fall: one burst per blow of this
/// seat's still on the books.
pub(super) fn keeper(w: &mut Img, show: &MossShow, me: u32, now: u32, tick: u64) {
    for blow in show.juice.blows.iter().filter(|bl| bl.owner == me && bl.weight >= Weight::Solid) {
        let age = blow.age(now);
        let seed = blow.born.wrapping_mul(2_654_435_761);
        let (n, speed) = match blow.weight {
            Weight::Solid => (12, (1.0, 3.0)),
            Weight::Crushing => (18, (1.2, 3.6)),
            _ => (28, (1.4, 4.4)),
        };
        let mut pen = Pen::new(w, 700);
        let petals = Spray { gravity: 0.1, drag: 0.08, ..Spray::round(seed, n, KEEPER, speed, (16.0, 34.0)) };
        petals.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Petal, ramps::NATURE, 1, tick));
        if blow.weight == Weight::Kill {
            juice::death_burst(w, KEEPER, age, seed, tick);
        } else if blow.weight != Weight::Finale {
            // Where the marble met it.
            juice::impact_burst(w, (KEEPER.0, KEEPER.1 + 8.0), age, seed, blow.weight, tick);
        }
    }
}

/// The jackpot: a fountain of coins from the keeper's crown and fireworks
/// over the table, for the seat that took it.
pub(super) fn jackpot(w: &mut Img, age: u32, tick: u64) {
    let (x0, y0, ww, _) = WINDOW;
    let mut pen = Pen::new(w, 1400);
    for (seed, spread, speed) in [(0x6a, 2.4, (2.4, 5.4)), (0x6b, 1.5, (3.4, 6.2))] {
        let coins = Spray {
            angle: -std::f32::consts::FRAC_PI_2,
            spread,
            gravity: 0.2,
            drag: 0.02,
            stagger: 0.7,
            ..Spray::round(seed, 44, (KEEPER.0, KEEPER.1 - 6.0), speed, (34.0, 62.0))
        };
        coins.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Coin, ramps::GOLD, 1, tick));
    }
    // Paper and gold over the whole table once the first shots have broken.
    if (8..90).contains(&age) {
        juice::confetti(w, WINDOW, age, 5, 70);
    }
    let shots = [
        (x0 + ww / 6, 76.0, ramps::GOLD),
        (x0 + ww * 5 / 6, 70.0, ramps::HOLY),
        (x0 + ww / 3, 58.0, ramps::EMBER),
        (x0 + ww * 2 / 3, 62.0, ramps::GOLD),
        (x0 + ww / 2, 52.0, ramps::HOLY),
    ];
    for (j, (fx, top, ramp)) in shots.into_iter().enumerate() {
        let t = age as i32 - 4 - j as i32 * 7;
        if t >= 0 {
            juice::firework(w, fx as f32, (y0 + 118) as f32, top, t as u32, 11 + j as u32 * 5, ramp, tick);
        }
    }
}

/// A rim of one ink round the field, `k` (0 to 1) strong: the border bands only.
pub(super) fn rim(im: &mut Img, k: f32, depth: i32, ink_char: char) {
    if k <= 0.0 {
        return;
    }
    let Some(c) = ink(ink_char) else { return };
    let (bx, by, bw, bh) = WINDOW;
    for y in by..by + bh {
        let ey = (y - by).min(by + bh - 1 - y);
        let across = ey < depth;
        let mut x = bx;
        while x < bx + bw {
            if !across && x == bx + depth {
                x = bx + bw - depth;
                continue;
            }
            let edge = (x - bx).min(bx + bw - 1 - x).min(ey);
            if edge < depth {
                let near = 1.0 - edge as f32 / depth as f32;
                if bayer(x, y) < near * near * k {
                    im.set(x, y, c);
                }
            }
            x += 1;
        }
    }
}

/// The drain: a ring and embers at the pit under the paddles.
pub(super) fn drain(w: &mut Img, b: &Ball, tick: u64) {
    if b.save < 2 || b.drains == 0 {
        return;
    }
    let age = 12 - u32::from(b.save);
    let c = at(80, 133);
    let mut pen = Pen::new(w, 700);
    pen.shock(c, 6.0 + age as f32 * 3.4, 1.8, 0.4, ramps::BLOOD, age as f32 / 12.0);
    let embers = Spray { gravity: -0.05, ..Spray::round(hash(b.drains as i32, 3, 5), 10, c, (0.8, 2.6), (10.0, 20.0)) };
    embers.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::EMBER, 1, tick));
}
