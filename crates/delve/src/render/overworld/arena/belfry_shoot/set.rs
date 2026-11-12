//! The belfry's moving parts, in the room's pixels, under the knights: the
//! great bell on its frame (swaying, and rocked by every toll), the
//! lanterns' flames and the warm pools they throw, dust drifting up the
//! moon's shafts and the bats roosting on the rafter. The still stone is
//! `walls`; these are the things that make the room breathe.

use super::super::pfx::{Pen, ramps};
use super::super::super::ink::{Img, bayer, hash};
use super::super::TILE;
use super::{bits, walls};
use crate::motion;
use crate::rules::Run;
use crate::rules::folly::BatShoot;

/// Where the great bell hangs: the middle of the north wall, its pivot just
/// under the rafter.
pub(super) fn bell_pivot(run: &Run) -> (i32, i32) {
    walls::plan_here(run).bell
}

/// How far the bell is swung at `now`, radians: a breath of sway while it
/// waits, and each toll sets it rocking, ringing down over a second.
pub(super) fn swing(shoot: &BatShoot, now: u32) -> f32 {
    let idle = 0.035 * motion::math::sin(now as f32 * 0.042);
    let toll = shoot
        .show
        .tolls
        .iter()
        .map(|&t| {
            let age = now.saturating_sub(t) as f32;
            0.5 * motion::math::exp(-age / 17.0) * motion::math::cos(age * 0.62)
        })
        .fold(0.0f32, |a, b| if b.abs() > a.abs() { b } else { a });
    // The last five seconds: the bell is touched each second, a clock's tick.
    let tick = if !shoot.done && !shoot.titled() && shoot.left() <= 5 {
        let rem = crate::rules::folly::TITLE + crate::rules::folly::BAT_TIME - shoot.t.min(crate::rules::folly::TITLE + crate::rules::folly::BAT_TIME);
        let age = (crate::rules::HZ - 1 - rem.saturating_sub(1) % crate::rules::HZ) as f32;
        0.2 * motion::math::exp(-age / 9.0) * motion::math::cos(age * 0.7) * if shoot.left() % 2 == 0 { 1.0 } else { -1.0 }
    } else {
        0.0
    };
    idle + toll + tick
}

/// The sprite turned `turn` radians about `pivot` (its own pixels),
/// nearest-neighbour, so it stays pixels.
pub(super) fn turned(sprite: &Img, pivot: (f32, f32), turn: f32) -> Img {
    if turn.abs() < 0.012 {
        return sprite.clone();
    }
    let (s, c) = (motion::math::sin(-turn), motion::math::cos(-turn));
    // The canvas keeps room for the swing.
    let (cw, ch) = (sprite.w + 24, sprite.h + 10);
    let (ox, oy) = (12.0, 0.0);
    let mut out = Img::new(cw, ch);
    for y in 0..ch {
        for x in 0..cw {
            let (dx, dy) = (x as f32 - ox - pivot.0, y as f32 - oy - pivot.1);
            let (sx, sy) = (dx * c - dy * s + pivot.0, dx * s + dy * c + pivot.1);
            if let Some(p) = sprite.get(sx.round() as i32, sy.round() as i32) {
                out.set(x, y, p);
            }
        }
    }
    out
}

/// The bell's frame and the bell, swung, over the floor at the north wall.
fn great_bell(cv: &mut Img, run: &Run, shoot: &BatShoot, now: u32, view: (i32, i32)) {
    let (cx, py) = bell_pivot(run);
    if !super::near((cx, py + 20), view, 70) {
        return;
    }
    let reach = 25;
    // The frame: two oak uprights with iron feet, the crossbeam over them.
    for side in [-1, 1] {
        let x = cx + side * reach - 2;
        cv.rect(x, py - 2, 5, 46, 'B');
        cv.rect(x + if side < 0 { 0 } else { 3 }, py - 2, 2, 46, if side < 0 { 'r' } else { 'p' });
        cv.rect(x - 1, py + 41, 7, 3, 'g');
        cv.rect(x - 1, py + 41, 7, 1, 'J');
        // A diagonal brace up to the beam.
        for k in 0..10 {
            cv.put(x + side * -(k / 2) + if side < 0 { 5 } else { -1 }, py + 10 + k, 'P');
        }
    }
    cv.rect(cx - reach - 3, py - 5, reach * 2 + 7, 5, 'B');
    cv.rect(cx - reach - 3, py - 5, reach * 2 + 7, 1, 'o');
    cv.rect(cx - reach - 3, py - 1, reach * 2 + 7, 1, 'p');
    for bx in [cx - reach, cx + reach - 3] {
        cv.rect(bx, py - 5, 3, 5, 'g');
        cv.put(bx + 1, py - 4, 'J');
    }
    // The bell-pull: a rope from the crossbeam, its knot and tassel swinging
    // a little behind the bell.
    let turn_now = swing(shoot, now);
    let rope = cx + reach - 8;
    for k in 0..32 {
        let sway = (turn_now * k as f32 * 0.55) as i32;
        cv.put(rope + sway, py + k, if k % 4 < 2 { 'o' } else { 'P' });
        if k == 22 {
            cv.rect(rope + sway - 1, py + k, 3, 2, 'O');
        }
    }
    let tail = (turn_now * 32.0 * 0.55) as i32;
    cv.rect(rope + tail - 1, py + 32, 3, 4, 'T');
    cv.put(rope + tail, py + 36, 'o');
    // The bell, turned about its yoke.
    let sprite = bits::great();
    let pivot = (sprite.w as f32 / 2.0, 1.0);
    let turn = swing(shoot, now);
    let img = turned(sprite, pivot, turn);
    let (ox, oy) = if turn.abs() < 0.012 { (0, 0) } else { (12, 0) };
    cv.stamp(&img, cx - sprite.w / 2 - ox, py - 1 - oy);
    // Its shadow, a dithered oval on the boards, sliding with the swing.
    let sh = (turn * 40.0) as i32;
    for dy in -3..=3i32 {
        for dx in -13..=13i32 {
            let d = (dx * dx) as f32 / 169.0 + (dy * dy) as f32 / 9.0;
            if d < 1.0 && bayer(cx + dx + sh, py + 52 + dy) < (1.0 - d) * 0.4 && cv.get(cx + dx + sh, py + 52 + dy).is_none() {
                cv.put(cx + dx + sh, py + 52 + dy, 'k');
            }
        }
    }
    // A toll's ringing: gold rings thrown out of the bell's mouth, and the
    // air shimmering round it.
    for &t in &shoot.show.tolls {
        let age = now.saturating_sub(t);
        if age < 26 {
            let mouth = (cx as f32 + turn * 30.0, (py + 30) as f32);
            let mut pen = Pen::new(cv, 500);
            let k = age as f32 / 26.0;
            pen.shock(mouth, 8.0 + age as f32 * 3.4, 1.6, 0.7, ramps::GOLD, k);
            if age > 4 {
                pen.shock(mouth, 4.0 + (age - 4) as f32 * 2.8, 1.2, 0.7, ramps::EMBER, (age - 4) as f32 / 22.0);
            }
        }
    }
}

/// Lanterns on the north walls between the slits: an iron lantern on its
/// bracket, its flame flickering, and a warm pool on the boards under it.
fn lanterns(cv: &mut Img, run: &Run, now: u32, view: (i32, i32)) {
    let plan = walls::plan_here(run);
    for &(x, base) in plan.lanterns.iter().filter(|&&(x, base)| super::near((x, base + 10), view, 36)) {
        let y = base - 3;
        let c = x / TILE;
        // The bracket, the lantern's iron and its glass.
        cv.rect(x - 1, y - 7, 3, 1, 'g');
        cv.rect(x - 3, y - 6, 7, 1, 'j');
        cv.rect(x - 3, y - 5, 7, 6, 'g');
        cv.rect(x - 2, y - 4, 5, 4, 'Z');
        let flicker = hash(c, (now / 3) as i32, 11);
        cv.stamp(bits::flame(now / 4 + c as u32), x - 1, y - 5 + i32::from(flicker % 5 == 0));
        // The pool on the boards: a warm dither, fainter than the torches'.
        let k = 0.8 + 0.2 * (flicker % 7) as f32 / 7.0;
        for dy in 0..22i32 {
            for dx in -18..=18i32 {
                let d = (dx * dx) as f32 / 324.0 + (dy * dy) as f32 / 484.0;
                if d < 1.0 && bayer(x + dx, y + 6 + dy) < (1.0 - d) * 0.16 * k && cv.get(x + dx, y + 6 + dy).is_none() {
                    cv.put(x + dx, y + 6 + dy, if d < 0.2 { 'I' } else { 'Z' });
                }
            }
        }
    }
}

/// Dust turning in the moon's shafts, and the bats on the rafter.
fn air(cv: &mut Img, run: &Run, shoot: &BatShoot, now: u32, view: (i32, i32)) {
    let plan = walls::plan_here(run);
    for (n, &(sx, base)) in plan.slits.iter().enumerate().filter(|&(_, &(sx, base))| super::near((sx + 50, base + 70), view, 100)) {
        for k in 0..7u32 {
            let life = 150u32;
            let t = (now / 2 + k * 23 + n as u32 * 31) % life;
            let fall = t as f32;
            let sway = motion::math::sin(fall * 0.09 + k as f32 * 2.0) * 3.5;
            let x = sx as f32 + fall * 0.62 + sway + (hash(n as i32, k as i32, 3) % 7) as f32 - 3.0;
            let y = base as f32 + fall;
            if t < 6 || t > life - 20 {
                continue;
            }
            let ink = if t % 5 < 2 { 'V' } else { 'U' };
            if bayer(x as i32, y as i32) < 0.8 && cv.get(x as i32, y as i32).is_none() {
                cv.put(x as i32, y as i32, ink);
            }
        }
    }
    // The roost: a folded bat now and then along the rafter, shuffling a
    // moment in turn; when a flock drops they all stir.
    let stir = shoot.show.swoops.iter().any(|d| now.saturating_sub(d.born) < 10);
    for &(x, y) in plan.roosts.iter().filter(|&&p| super::near(p, view, 12)) {
        let shuffle = stir || (now / 5 + hash(x, 1, 5) % 90) % 90 < 2;
        let roost = bits::roost(u32::from(shuffle) * (now / 3));
        cv.stamp(roost, x, y);
    }
}

/// Everything of the set that moves, over the scenery and under the figures.
pub(super) fn under(cv: &mut Img, run: &Run, shoot: &BatShoot, view: (i32, i32)) {
    let now = run.tick as u32;
    lanterns(cv, run, now, view);
    air(cv, run, shoot, now, view);
    great_bell(cv, run, shoot, now, view);
}
