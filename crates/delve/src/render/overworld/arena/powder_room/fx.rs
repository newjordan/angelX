//! What a blast does to a knight, in the room's pixels over the figures: a
//! caught knight whited where they stood, a burst of soot, a black ghost
//! of them flung home along an arc with a smoke trail, a ring where they
//! land; the soot that stays on them a while; and a red warning over any
//! knight standing in a lane about to burn. Every one is a pure function
//! of the show's ticks.

use super::super::super::ink::{Img, bayer, hash};
use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::{at, knights};
use super::{bits, boom, set};
use crate::motion::{self, ease, math};
use crate::rules::juice::Weight;
use crate::rules::powder::Powder;
use crate::rules::powder_show::{SOOT_LIFE, Soot};
use crate::rules::Run;

/// Ticks the ghost takes home.
const FLIGHT: u32 = 18;

/// Over the figures: sootings, the soot that stays, the lane warnings.
pub(super) fn over(cv: &mut Img, run: &Run, p: &Powder, focus: Option<u32>, view: (i32, i32)) {
    let now = run.tick as u32;
    for s in &p.show.soots {
        let age = now.saturating_sub(s.born);
        if age < SOOT_LIFE && (set::near(boom::centre(s.from), view, 60) || set::near(boom::centre(s.to), view, 60)) {
            sooting(cv, run, s, age);
        }
    }
    warnings(cv, run, p, focus, view, now);
}

/// A knight sooted at `s.from` and sent to `s.to`.
fn sooting(cv: &mut Img, run: &Run, s: &Soot, age: u32) {
    let who = u32::from(s.who);
    let Some(hero) = run.players.get(&who) else { return };
    let tick = run.tick;
    let (fx, fy) = boom::centre(s.from);
    let (tx, ty) = boom::centre(s.to);
    let seed = s.born ^ who.wrapping_mul(0x9E37);
    let sprite = knights::figure(hero, who);
    // Where they stood: white a moment, a burst, and the soot going up.
    if age < Weight::Kill.flash() + 1 {
        let ghost = juice::impact_frame(&sprite, 'H');
        cv.stamp(&ghost, fx - ghost.w / 2, fy + 7 - ghost.h);
    }
    juice::impact_burst(cv, (fx as f32, fy as f32 - 5.0), age, seed, Weight::Kill, tick);
    let mut pen = Pen::new(cv, 500);
    let a = age as f32;
    let cloud = Spray { gravity: -0.035, drag: 0.06, ..Spray::round(seed ^ 0x11, 12, (fx as f32, fy as f32 - 4.0), (0.5, 2.0), (16.0, 34.0)) };
    cloud.each(a, |m| pfx::mote(&mut pen, &m, Shape::Smoke, ramps::SMOKE, 2, tick));
    let flakes = Spray { gravity: 0.03, ..Spray::round(seed ^ 0x22, 9, (fx as f32, fy as f32 - 8.0), (0.4, 1.8), (18.0, 36.0)) };
    flakes.each(a, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramps::VOID, 1, tick));
    // The black ghost flung home along an arc, a smoke trail behind it.
    if (2..2 + FLIGHT).contains(&age) {
        let t = (age - 2) as f32 / FLIGHT as f32;
        let ghost = bits::sooted(&sprite);
        for j in (1..6).rev() {
            let tt = (t - j as f32 * 0.05).max(0.0);
            let (x, y) = motion::arc((fx as f32, fy as f32), (tx as f32, ty as f32), 36.0, ease::both(tt));
            if bayer(x as i32, y as i32) < 0.8 - j as f32 * 0.12 {
                cv.rect(x as i32 - 1, y as i32 - 5 - j, 3, 3, if j % 2 == 0 { 'g' } else { 'j' });
            }
        }
        let (x, y) = motion::arc((fx as f32, fy as f32), (tx as f32, ty as f32), 36.0, ease::both(t));
        cv.stamp(&ghost, x as i32 - ghost.w / 2, y as i32 + 7 - ghost.h);
    }
    // Home: a ring and dust where they land.
    if age >= FLIGHT {
        let l = (age - FLIGHT) as f32;
        let mut pen = Pen::new(cv, 300);
        if l < 14.0 {
            pen.ring((tx as f32, ty as f32 + 3.0), 4.0 + l * 1.5, 1.4, 0.6, ramps::SMOKE, l / 14.0);
        }
        let dust = Spray { gravity: 0.04, ..Spray::round(seed ^ 0x33, 8, (tx as f32, ty as f32 + 2.0), (0.4, 1.6), (8.0, 16.0)) };
        dust.each(l, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramps::SMOKE, 1, tick));
    }
    // Dizzy stars wheel over their head a moment after they land.
    if (FLIGHT..FLIGHT + 26).contains(&age) {
        let (hx, hy) = at(hero.x, hero.y);
        let t = (age - FLIGHT) as f32;
        let mut pen = Pen::new(cv, 120);
        for k in 0..3 {
            let ang = t * 0.34 + k as f32 * 2.09;
            pen.twinkle((hx as f32 + math::cos(ang) * 8.0, hy as f32 - 25.0 + math::sin(ang) * 2.5), 1.4, (t / 26.0 + k as f32 * 0.05).min(0.99), ramps::GOLD);
        }
    }
    // The soot that stays on them while they are safe: specks over their
    // own pixels, thinning out as the safe time ends.
    let (hx, hy) = at(hero.x, hero.y);
    if age >= FLIGHT - 2 {
        let k = 0.5 * (1.0 - (age - (FLIGHT - 2)) as f32 / (SOOT_LIFE - FLIGHT + 2) as f32).max(0.0);
        for y in hy - 22..hy + 6 {
            for x in hx - 9..hx + 10 {
                if cv.get(x, y).is_some() && bayer(x, y) < k && hash(x, y, 5) % 3 != 0 {
                    cv.put(x, y, if hash(x, y, 6) % 2 == 0 { 'k' } else { 'X' });
                }
            }
        }
    }
}

/// A red mark over each knight who stands in a lane that will burn within
/// a second: a triangle that bobs, faster as the fuse runs out.
fn warnings(cv: &mut Img, run: &Run, p: &Powder, focus: Option<u32>, view: (i32, i32), now: u32) {
    if p.done || p.titled() {
        return;
    }
    let sign = bits_warn();
    for (&id, hero) in run.players.iter().filter(|(id, h)| h.hp > 0 && !h.stone && !h.absent && p.broken.contains_key(id)) {
        let tile = ((hero.x / crate::rules::TILE_UNITS) as i32, (hero.y / crate::rules::TILE_UNITS) as i32);
        let Some(fuse) = p.kegs.iter().filter(|k| k.fuse < 42 && set::in_lane(run, p, k, tile)).map(|k| k.fuse).min() else {
            continue;
        };
        let (x, y) = at(hero.x, hero.y);
        if !set::near((x, y), view, 30) {
            continue;
        }
        let rate = if fuse < 16 { 2 } else { 4 };
        let bob = [0, -1, -2, -1][(now / rate % 4) as usize];
        if fuse < 16 && now / 2 % 2 == 0 && focus != Some(id) {
            continue;
        }
        cv.stamp(sign, x - sign.w / 2, y - 32 + bob);
    }
}

/// The warning sign: a red triangle with a mark in it, seven by seven.
fn bits_warn() -> &'static Img {
    static SIGN: std::sync::OnceLock<Img> = std::sync::OnceLock::new();
    SIGN.get_or_init(|| {
        Img::from_rows(&["...k...", "..k7k..", ".k777k.", ".k797k.", "k77977k", "k77777k", "kkkkkkk"])
    })
}
