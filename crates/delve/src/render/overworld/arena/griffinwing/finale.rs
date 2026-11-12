//! The way home as one moment, the beacon its hero: the procession of gates
//! and hearth rings is drawn as strings of lights on a hair of a ring that
//! fade as they near and as the beacon takes the flame (so no great gold
//! hoop fills the picture), and when the wing's comets have landed the light
//! floods the canyon from the beacon back toward the eye, a warm dither
//! lifted over the rock, strongest far down the gorge, a little on the near
//! walls, and left on as a warm cast over the tally.
use super::cam::Cam;
use super::layout::{H, W};
use super::moments::LIGHT_AT;
use super::set;
use super::super::pfx::ramps;
use crate::render::overworld::ink::{Img, bayer, nearest_plain};
use crate::rules::griffinwing::trig::Det;
use crate::rules::griffinwing::{Mode, Session};
use std::cell::RefCell;
use std::collections::HashMap;

/// Whether the flight is in its victory: the way home or the tally.
pub(super) fn on(s: &Session) -> bool {
    matches!(s.mode, Mode::Victory | Mode::Tally)
}

/// How many ticks the beacon has been lit (none before it takes the flame).
pub(super) fn lit_age(s: &Session) -> Option<u32> {
    match s.mode {
        Mode::Victory if s.mode_t >= LIGHT_AT => Some(s.mode_t - LIGHT_AT),
        // The tally keeps the sky lit.
        Mode::Tally => Some(20 + s.mode_t),
        _ => None,
    }
}

fn smooth(lo: f32, hi: f32, v: f32) -> f32 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ─── the light that floods the canyon ──────────────────────────────────────

thread_local! {
    /// The warmed version of each ink the scene is made of, at two strengths.
    static WARM: RefCell<HashMap<([u8; 3], u8), [u8; 3]>> = RefCell::new(HashMap::new());
}

/// `c` lifted toward gold, snapped to the realm's inks. Level 1 is half the
/// way, level 2 nearly all of it.
fn warmed(c: [u8; 3], level: u8) -> [u8; 3] {
    WARM.with(|cache| {
        *cache.borrow_mut().entry((c, level)).or_insert_with(|| {
            let lum = f32::from(c[0]) * 0.3 + f32::from(c[1]) * 0.5 + f32::from(c[2]) * 0.2;
            let want = [lum * 1.5 + 78.0, lum * 1.15 + 46.0, lum * 0.5 + 10.0];
            let k = if level == 1 { 0.55 } else { 0.9 };
            nearest_plain(std::array::from_fn(|i| {
                let from = f32::from(c[i]);
                from + (want[i] - from) * k
            }))
        })
    })
}

/// The light over the canyon, as the scene's own dots (the sky and the pieces
/// stood over it are left alone): a front runs from the beacon back down the
/// gorge to the eye in about nine ticks, then the cast stays.
pub(super) fn flood(im: &mut Img, depth: &[f32], cam: &Cam, s: &Session) {
    let Some(age) = lit_age(s) else { return };
    let Some(su) = set::beacon_at(s) else { return };
    let (fx, fy, far) = set::project(cam, su, 0.0, 11.0).map_or((W as f32 * 0.5, H as f32 * 0.4, 70.0), |(x, y, d)| (x, y, d.max(20.0)));
    let a = age as f32;
    // Up at once, a held peak, then a steady warm cast.
    let k = if a < 5.0 {
        a / 5.0
    } else if a < 20.0 {
        1.0 - 0.45 * (a - 5.0) / 15.0
    } else {
        0.4
    };
    let front = far * 1.1 - 7.5 * a;
    for y in 0..H {
        for x in 0..W {
            let d = depth[(y * W + x) as usize];
            if !d.is_finite() {
                continue;
            }
            let reach = smooth(0.0, 12.0, d - front);
            if reach <= 0.0 {
                continue;
            }
            let near = (d / far).clamp(0.0, 1.0);
            let r = ((x as f32 - fx).powi(2) + (y as f32 - fy).powi(2)).sqrt() / W as f32;
            // Strongest round the beacon and far down the gorge, a little on the near walls.
            let st = (k * reach * (0.05 + 0.3 * near * near + 0.85 * (1.0 - r * 2.4).max(0.0))).clamp(0.0, 0.9);
            let b = bayer(x, y);
            if b >= st {
                continue;
            }
            if let Some(c) = im.get(x, y) {
                im.set(x, y, warmed(c, if b < st * 0.55 { 2 } else { 1 }));
            }
        }
    }
}

// ─── the procession's rings, thin ──────────────────────────────────────────

/// A gate or hearth ring as a hair of a ring strung with lights.
pub(super) struct Ghost {
    pub at: (f32, f32),
    /// Its radius across and up, in dots.
    pub r: (f32, f32),
    pub depth: f32,
    pub hearth: bool,
}

/// Draw `g` over `im` where `depth` leaves it showing, `tick` the flight's.
/// It is faint while far, strongest in the middle of the run and gone by the
/// time it would fill the picture; once the beacon is lit it dims, and it
/// keeps clear of the flame.
pub(super) fn ghost(im: &mut Img, depth: &[f32], g: &Ghost, s: &Session, flame: Option<(f32, f32)>) {
    let d = g.depth;
    let mut alpha = smooth(8.0, 20.0, d) * (1.0 - 0.7 * smooth(60.0, 110.0, d));
    if let Some(age) = lit_age(s) {
        alpha *= 1.0 - 0.55 * smooth(0.0, 12.0, age as f32);
    }
    if let Some((fx, fy)) = flame {
        let dist = ((g.at.0 - fx).powi(2) + (g.at.1 - fy).powi(2)).sqrt();
        alpha *= (dist / (W as f32 * 0.25)).clamp(0.3, 1.0);
    }
    if alpha < 0.05 {
        return;
    }
    let (rx, ry) = g.r;
    let n = ((rx.max(ry) * 6.28).ceil() as i32).clamp(24, 420);
    let (st, ct) = (std::f32::consts::TAU / n as f32).dsin_cos();
    let (mut sn, mut cs) = (0.0f32, 1.0f32);
    let hair = ramps::GOLD.at(0.35);
    let lit = |im: &mut Img, x: i32, y: i32, ink: char, a: f32| {
        if x < 0 || y < 0 || x >= W || y >= H {
            return;
        }
        if depth[(y * W + x) as usize] > d - 0.25 && bayer(x, y) < a {
            im.put(x, y, ink);
        }
    };
    for _ in 0..n {
        let (x, y) = ((g.at.0 + rx * cs).round() as i32, (g.at.1 - ry * sn).round() as i32);
        // The top of the ring catches more light than the underside.
        lit(im, x, y, if sn > 0.0 { hair } else { ramps::GOLD.at(0.55) }, alpha * 0.85);
        (sn, cs) = (sn * ct + cs * st, cs * ct - sn * st);
    }
    // The lights on it, turning slowly.
    let beads = if g.hearth { 12 } else { 8 };
    let turn = s.elapsed as f32 * 0.025;
    let size = ((rx.max(ry) / 16.0).round() as i32).clamp(1, 3);
    for i in 0..beads {
        let (bs, bc) = (turn + i as f32 / beads as f32 * std::f32::consts::TAU).dsin_cos();
        let (x, y) = ((g.at.0 + rx * bc).round() as i32, (g.at.1 - ry * bs).round() as i32);
        let ink = if g.hearth { ramps::EMBER.at(0.1) } else { ramps::GOLD.at(0.0) };
        for dy in 0..size {
            for dx in 0..size {
                let core = if size > 1 && dx == 0 && dy == 0 { ramps::GOLD.at(0.0) } else { ink };
                lit(im, x + dx - size / 2, y + dy - size / 2, core, (alpha * 1.1).min(1.0));
            }
        }
    }
}
