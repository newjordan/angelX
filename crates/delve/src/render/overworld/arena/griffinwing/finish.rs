//! The way home and the tally: when the Wyrm falls every craft in the wing
//! sends a comet of gold to the beacon at once, the beacon takes the light
//! with a burst, rings and a beam, and fireworks go up over the canyon; the
//! tally is held in a fall of paper with the party's best and a crown on the
//! knight who flew best.
use super::super::juice;
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::fx;
use super::hud::Ctx;
use super::layout::{H, W};
use super::moments::LIGHT_AT;
use crate::render::overworld::ink::{Img, bayer, hash};
use crate::rules::griffinwing::{self as gw, Mode, Medal, Session};

/// Where the beacon's flame is on the picture, and how big a unit is there.
fn flame(c: &Ctx) -> Option<(f32, f32, f32)> {
    c.at(c.s.home + gw::dm(1000), 0, gw::dm(110))
}

/// Every craft that flies, a knight's own marked: where it is on the picture.
fn wing(c: &Ctx) -> Vec<(f32, f32)> {
    let s = c.s;
    let mut out = Vec::new();
    for (&id, seat) in &s.seats {
        if let Some((x, y, _)) = fx::craft_at(c, &seat.craft, id == c.seat) {
            out.push((x, y));
        }
    }
    for w in s.wing.iter().filter(|w| !w.lost) {
        if let Some((x, y, _)) = fx::craft_at(c, &w.craft, false) {
            out.push((x, y));
        }
    }
    out
}

/// The guide a comet rides, from where its craft is to the flame: a straight
/// run bowed up and swung out to a side in an arc, each craft's own, `u`
/// (0 to 1) how far along.
fn comet_guide(from: (f32, f32), to: (f32, f32), i: usize, u: f32) -> (f32, f32) {
    let bow = -34.0 * 4.0 * (1.0 + (i % 3) as f32 * 0.3);
    // They come in from both sides, each swung out to its own width, so the
    // wing is seen to gather on the beacon.
    let swing = if i % 2 == 0 { -1.0 } else { 1.0 } * (40.0 + 18.0 * ((i / 2) % 3) as f32) * 4.0;
    let arc = u * (1.0 - u);
    (from.0 + (to.0 - from.0) * u + swing * arc, from.1 + (to.1 - from.1) * u + bow * arc)
}

/// How far along its guide comet `i` is `t` (0 to 1) into the flight: eased,
/// each a little behind the one before so they land together.
fn comet_u(t: f32, i: usize) -> f32 {
    let lag = (i as f32 * 0.04).min(0.2);
    crate::motion::ease::both(((t - lag) / (1.0 - lag)).clamp(0.0, 1.0))
}

/// How many ticks the wing's comets fly to the beacon before it takes the
/// flame.
pub(super) const COMET_TICKS: u32 = 32;

/// The comets, the ignition, the beam and the fireworks of the way home.
pub(super) fn world(im: &mut Img, c: &Ctx) {
    let s = c.s;
    if !matches!(s.mode, Mode::Victory | Mode::Tally) {
        return;
    }
    let tick = u64::from(s.elapsed);
    let now = if s.mode == Mode::Victory {
        s.mode_t
    } else {
        // The tally keeps the sky lit.
        LIGHT_AT + 20 + s.mode_t
    };
    if let Some((fx_, fy, d)) = flame(c) {
        let k = (c.tall(1.0, d) / 4.0).clamp(0.7, 3.2);
        // The wing lights it: a gold comet from every craft, ending as the
        // beacon takes the flame.
        let from = LIGHT_AT - COMET_TICKS;
        if s.mode == Mode::Victory && (from..LIGHT_AT).contains(&now) {
            let t = (now - from) as f32 / COMET_TICKS as f32;
            let mut pen = Pen::new(im, 900);
            let mut guides: Vec<Vec<(f32, f32)>> = Vec::new();
            for (i, (x, y)) in wing(c).into_iter().enumerate() {
                // Each rides its own guide arc into the flame; they arrive
                // together.
                let (from_, to) = ((x, y), (fx_, fy));
                let at_ = |t: f32| {
                    let u = comet_u(t, i);
                    let (qx, qy) = comet_guide(from_, to, i, u);
                    (qx, qy, u)
                };
                let (px, py, u) = at_(t);
                // A tail of embers down the arc it has flown, thinning as it goes.
                for j in 1..=12u32 {
                    let (qx, qy, _) = at_((t - j as f32 * 0.022).max(0.0));
                    let k = j as f32 / 13.0;
                    if bayer(qx as i32, qy as i32) > 1.0 - k * 0.8 {
                        continue;
                    }
                    pen.put(qx as i32, qy as i32, ramps::GOLD.at(0.1 + k * 0.7));
                    if j < 7 {
                        pen.put(qx as i32 + 1, qy as i32, ramps::GOLD.at(0.3 + k * 0.5));
                        pen.put(qx as i32, qy as i32 + 1, ramps::GOLD.at(0.4 + k * 0.5));
                    }
                }
                // The head leans along the guide's own direction.
                let (nx, ny, _) = at_(t + 0.02);
                pen.streak((px, py), (nx - px, ny - py), 12.0 * (1.0 - u * 0.4), ramps::GOLD, u * 0.5);
                // The head: a bright core in a gold ring, twinkling.
                pen.put(px as i32, py as i32, '9');
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (1, 1), (-1, -1)] {
                    pen.put(px as i32 + dx, py as i32 + dy, '6');
                }
                for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                    pen.put(px as i32 + dx, py as i32 + dy, '5');
                }
                pen.twinkle((px, py), 4.0, (t * 3.0 + i as f32 * 0.3).fract(), ramps::GOLD);
                if super::guide::on() {
                    guides.push((0..=24).map(|k| comet_guide(from_, to, i, k as f32 / 24.0)).collect());
                }
            }
            drop(pen);
            for g in &guides {
                super::guide::dotted(im, g, '%');
            }
        }
        if now >= LIGHT_AT {
            let age = now - LIGHT_AT;
            let a = age as f32;
            let at = (fx_, fy);
            let mut pen = Pen::new(im, 1200);
            if age < 5 {
                let r = ((5 - age) as f32 * 3.0 * k) as i32;
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy <= r * r && bayer(at.0 as i32 + dx, at.1 as i32 + dy) < 0.8 {
                            pen.put(at.0 as i32 + dx, at.1 as i32 + dy, if age < 2 { 'w' } else { '9' });
                        }
                    }
                }
            }
            if age < 16 {
                pfx::rays(&mut pen, at, 14, 34.0 * k, 1.0, 0.0, ramps::GOLD, a / 16.0);
            }
            // Rings of light leaving it, one a half second, three at most.
            for ring in 0..3u32 {
                let start = ring * 14;
                if age >= start && age - start < 40 {
                    let t = (age - start) as f32;
                    pen.shock(at, (4.0 + t * 3.2) * k, 1.8, 0.8, ramps::GOLD, t / 40.0);
                }
            }
            // A beam up into the dusk, dithered thin at its edge.
            if age < 70 {
                let fade = (1.0 - a / 70.0).max(0.0);
                let half = (2.0 + 3.0 * k * fade) as i32;
                for y in 0..(fy as i32 - 4).max(0) {
                    let rise = (fy - y as f32) / fy.max(1.0);
                    for dx in -half..=half {
                        let edge = dx.abs() as f32 / half.max(1) as f32;
                        if bayer(at.0 as i32 + dx, y) < (1.0 - edge) * fade * (0.9 - rise * 0.4) {
                            pen.light(at.0 as i32 + dx, y, if edge < 0.4 { '9' } else { '6' });
                        }
                    }
                }
            }
            let motes = Spray {
                gravity: -0.05,
                ..Spray::round(0x7e, 28, at, (0.4 * k, 2.2 * k), (24.0, 60.0))
            };
            motes.each(a, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::GOLD, 1, tick));
        }
    }
    // Fireworks over the canyon: a shell a few ticks apart, over the sky.
    if now >= LIGHT_AT + 6 {
        const RAMPS: [Ramp; 3] = [ramps::GOLD, ramps::FROST, ramps::EMBER];
        for i in 0..7u32 {
            let start = LIGHT_AT + 6 + i * 11;
            if now < start || now - start >= 64 {
                continue;
            }
            let u = |salt: u32| (hash(i as i32, salt as i32, 0x42) & 0xffff) as f32 / 65536.0;
            // The middle is the beacon's and the banner's, and behind the tally's
            // panel is no place for one: the shells go up in the margins.
            let x = if s.mode == Mode::Tally {
                if i % 2 == 0 { 14.0 + u(1) * 18.0 } else { W as f32 - 32.0 + u(1) * 18.0 }
            } else if i % 2 == 0 {
                24.0 + u(1) * 70.0
            } else {
                W as f32 - 94.0 + u(1) * 70.0
            };
            let top = 26.0 + u(2) * 48.0;
            juice::firework(im, x, H as f32 * 0.52, top, now - start, 0x51 + i, RAMPS[(i % 3) as usize], tick);
        }
    }
    if s.mode == Mode::Tally {
        // Paper falls through the margins of the tally.
        juice::confetti(im, (0, 0, 40, H), s.mode_t, 11, 18);
        juice::confetti(im, (W - 40, 0, 40, H), s.mode_t, 12, 18);
        juice::confetti(im, (40, 0, W - 80, 24), s.mode_t, 13, 14);
    }
}

/// The crown on the knight who flew best, nine by four.
pub(super) fn crown(im: &mut Img, x: i32, y: i32) {
    let rows = ["5...5...5", "555.5.555", "555555555", ".5555555."];
    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if ch != '.' {
                im.put(x + dx as i32, y + dy as i32, if dy == 3 { '4' } else { '6' });
            }
        }
    }
}

/// Who flew best: the most points, the lower seat on a tie. Only when two or
/// more knights flew.
pub(super) fn mvp(s: &Session) -> Option<u32> {
    if s.seats.len() < 2 {
        return None;
    }
    s.seats
        .iter()
        .max_by_key(|(id, seat)| (seat.craft.pts, u32::MAX - **id))
        .map(|(&id, _)| id)
}

/// The party's best line: the share of what the flight offered that it took,
/// and whether this run beat the one before.
pub(super) fn best_line(s: &Session) -> (bool, u32, u32) {
    let now = s.permille();
    let prior = u32::from(s.show.prior);
    (s.show.known && now > prior, now / 10, prior / 10)
}

/// A glint across the gold medal, once in a while.
pub(super) fn glint(im: &mut Img, at: (i32, i32), s: &Session) {
    if s.medal != Some(Medal::Gold) {
        return;
    }
    let phase = (s.mode_t % 50) as f32 / 50.0;
    let mut pen = Pen::new(im, 80);
    pen.twinkle((at.0 as f32 - 6.0, at.1 as f32 - 7.0), 4.0, phase, ramps::HOLY);
}
