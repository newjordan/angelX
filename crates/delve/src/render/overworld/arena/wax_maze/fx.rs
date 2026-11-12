//! The maze's moments in the room's own pixels: a drop popping as it is
//! eaten, the holy water's rings, a hot run's trail and tag, the flash and
//! the spilt wax of a touch, the burst of a wraith caught. Each is a pure
//! function of an age in ticks (`rules::wax_show` keeps what happened), laid
//! under the knights (`under`) or over them (`over`).

use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::{Img, at, knights};
use super::bits;
use crate::rules::folly::WaxMaze;
use crate::rules::juice::Weight;
use crate::rules::wax_show::{self, CATCH_LIFE, MUNCH_GAP, POP_LIFE, SPLASH_LIFE, TOUCH_LIFE};
use crate::rules::{Hero, Run};

/// A tile's middle in room pixels.
pub(super) fn tile_centre(tile: u16, cols: i32) -> (i32, i32) {
    let (x, y) = (i32::from(tile) % cols, i32::from(tile) / cols);
    (x * super::super::TILE + super::super::TILE / 2, y * super::super::TILE + super::super::TILE / 2)
}

/// The floor's moments, under the knights: a drop's pop and the holy
/// water's rings.
pub(super) fn under(cv: &mut Img, run: &Run, maze: &WaxMaze) {
    let (now, cols) = (run.tick as u32, run.room().cols as i32);
    for p in &maze.show.pops {
        let age = now.saturating_sub(p.born);
        if age >= POP_LIFE {
            continue;
        }
        let (x, y) = tile_centre(p.tile, cols);
        pop(cv, (x as f32, y as f32), age, u32::from(p.tile) ^ (u32::from(p.who) << 12), run.tick);
    }
    for s in &maze.show.splashes {
        let age = now.saturating_sub(s.born);
        if age >= SPLASH_LIFE {
            continue;
        }
        let (x, y) = tile_centre(s.tile, cols);
        splash(cv, (x as f32, y as f32), age, u32::from(s.tile), run.tick);
    }
}

/// A drop eaten: a small ring and crumbs thrown off it.
fn pop(cv: &mut Img, at: (f32, f32), age: u32, seed: u32, tick: u64) {
    let mut pen = Pen::new(cv, 120);
    if age < 6 {
        pen.ring(at, 2.0 + age as f32 * 1.5, 1.0, 1.0, ramps::HOLY, age as f32 / 6.0);
    }
    let crumbs = Spray::round(seed, 5, at, (0.5, 1.4), (5.0, 9.0));
    crumbs.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramps::HOLY, 1, tick));
}

/// The holy water taken: rays at the flask, then two rings going out across
/// the lanes and drops thrown up and falling back.
fn splash(cv: &mut Img, at: (f32, f32), age: u32, seed: u32, tick: u64) {
    let a = age as f32;
    let mut pen = Pen::new(cv, 2600);
    let fade = a / SPLASH_LIFE as f32;
    pen.ring(at, 4.0 + a * 3.0, 2.2, 1.0, ramps::FROST, fade);
    if age >= 5 {
        pen.ring(at, (a - 5.0) * 2.3, 1.6, 1.0, ramps::WATER, (a - 5.0) / (SPLASH_LIFE - 5) as f32);
    }
    if age < 9 {
        pfx::rays(&mut pen, at, 12, 30.0, 1.0, seed as f32 * 0.37, ramps::FROST, a / 9.0);
    }
    let spray = Spray { gravity: 0.1, ..Spray::round(seed, 14, at, (1.0, 2.8), (16.0, 30.0)) };
    spray.each(a, |m| pfx::mote(&mut pen, &m, Shape::Droplet, ramps::WATER, 1, tick));
}

/// A chevron in a seat's ink over a knight's head, outlined black.
fn chevron(cv: &mut Img, x: i32, y: i32, ink: char) {
    for dx in -2..=2 {
        cv.put(x + dx, y - 1, 'k');
    }
    for (dy, half) in [(0, 3), (1, 2), (2, 1), (3, 0)] {
        for dx in -half..=half {
            cv.put(x + dx, y + dy, 'k');
        }
    }
    for (dy, half) in [(0, 2), (1, 1), (2, 0)] {
        for dx in -half..=half {
            cv.put(x + dx, y + dy, ink);
        }
    }
}

/// A hot run's wake behind a knight: a dithered streak of their ink that
/// grows with the run.
fn wake(cv: &mut Img, (x, y): (i32, i32), way: (i32, i32), run: u16, ink: char) {
    let len = 4 + i32::from(run.min(30)) / 2;
    for j in 4..len {
        let fade = (j - 4) as f32 / (len - 4).max(1) as f32;
        for side in [0, 1] {
            let (px, py) = (x - way.0 * j + way.1 * side, y - 4 - way.1 * j + way.0 * side);
            if super::super::super::ink::bayer(px, py) >= 1.0 - fade * 0.9 {
                continue;
            }
            cv.put(px, py, if j % 3 == 0 { 'T' } else { ink });
        }
    }
}

/// What stands over the knights: each knight's pointer, hot runs and their
/// tags, a touch's flash and spilt wax, a wraith caught.
pub(super) fn over(cv: &mut Img, run: &Run, maze: &WaxMaze, live: bool) {
    let (now, tick) = (run.tick as u32, run.tick);
    for (&id, hero) in run.players.iter().filter(|(_, h)| live && h.hp > 0 && !h.stone && !h.absent) {
        let (x, y) = at(hero.x, hero.y);
        let ink = knights::seat_mark(id);
        let n = maze.show.run_of(id, now);
        if n >= 3
            && let Some(&(wx, wy)) = maze.ways.get(&id)
        {
            wake(cv, (x, y), (i32::from(wx), i32::from(wy)), n, ink);
        }
        let bob = [0, 1, 2, 1][(tick / 5 % 4) as usize];
        chevron(cv, x, y - 25 - bob, ink);
        if n >= 4 {
            combo_tag(cv, maze, id, (x, y - 29 - bob), n, now, ink);
        }
    }
    for t in &maze.show.touches {
        let age = now.saturating_sub(t.born);
        if age < TOUCH_LIFE {
            touched(cv, run, t, age);
        }
    }
    for c in &maze.show.catches {
        let age = now.saturating_sub(c.born);
        if age < CATCH_LIFE {
            caught(cv, c, age, tick);
        }
    }
}

/// A run's size over the knight, their ink, and a fuse running down to the
/// moment it breaks.
fn combo_tag(cv: &mut Img, maze: &WaxMaze, id: u32, (x, y): (i32, i32), n: u16, now: u32, ink: char) {
    let (last, _) = maze.show.combo.get(&id).copied().unwrap_or((0, 0));
    let hot = n >= 25;
    let bite = now.saturating_sub(last) < 2;
    let scale = if hot || bite && n % 5 == 0 { 2 } else { 1 };
    let tag = juice::big_text_img(&format!("x{n}"), scale, if hot { '9' } else { 'T' }, 'k');
    cv.stamp(&tag, x - tag.w / 2, y - tag.h);
    let left = (MUNCH_GAP - now.saturating_sub(last).min(MUNCH_GAP)) as i32 * 12 / MUNCH_GAP as i32;
    for dx in 0..left {
        cv.put(x - 6 + dx, y + 1, ink);
    }
}

/// A knight touched: where they stood a white ghost holds a moment and the
/// wax they spilt arcs away, the bite's sparks and the wax lost as a number;
/// where they were sent back to, a ring.
fn touched(cv: &mut Img, run: &Run, t: &wax_show::Touch, age: u32) {
    let at16 = |p: (i32, i32)| (p.0 * super::super::TILE / 16, p.1 * super::super::TILE / 16);
    let (x, y) = at16(t.at);
    let who = u32::from(t.who);
    let weight = Weight::Crushing;
    if age < weight.flash() + 1
        && let Some(hero) = run.players.get(&who)
    {
        let ghost = ghost_of(hero, who);
        // A knight's feet are on the tile's lower half.
        cv.stamp(&ghost, x - ghost.w / 2, y + 7 - ghost.h);
    }
    juice::impact_burst(cv, (x as f32, (y - 4) as f32), age, u32::from(t.who) * 31 + t.born, weight, run.tick);
    let sprite = bits::spill();
    for k in 0..u32::from(t.lost.min(8)) {
        let h = super::super::super::ink::hash(k as i32, t.born as i32, 33);
        let ang = (h % 360) as f32 * std::f32::consts::PI / 180.0;
        let speed = 1.6 + (h / 360 % 10) as f32 * 0.22;
        let a = age as f32;
        // Thrown out with a drag, hopping up and back down to the floor.
        let slide = 1.0 - crate::motion::math::exp(-0.11 * a);
        let reach = speed / 0.11;
        let hop = 9.0 * crate::motion::math::sin((a / 22.0).min(1.0) * std::f32::consts::PI);
        let (px, py) = (x as f32 + ang.cos() * reach * slide, y as f32 + ang.sin() * reach * slide * 0.8 - 4.0 - hop);
        if age < 30 || super::super::super::ink::bayer(px as i32, py as i32) > (age - 30) as f32 / 22.0 {
            cv.stamp(sprite, px as i32 - 1, py as i32 - 2);
        }
    }
    if t.lost > 0 {
        juice::pop_number(cv, &format!("-{}", t.lost), (x, y - 14), age, 34, 2, '7');
    }
    if let Some(hero) = run.players.get(&who) {
        let (hx, hy) = at(hero.x, hero.y);
        let mut pen = Pen::new(cv, 300);
        if age < 14 {
            pen.ring((hx as f32, hy as f32 - 4.0), 3.0 + age as f32 * 1.4, 1.2, 1.0, ramps::HOLY, age as f32 / 14.0);
        }
        // Sent back, where they now stand: what it cost, over their head.
        if t.lost > 0 && (hx - x).abs() + (hy - y).abs() > 40 {
            juice::pop_number(cv, &format!("-{}", t.lost), (hx, hy - 26), age.saturating_sub(2), 34, 2, '7');
        }
    }
}

/// A knight as one bright silhouette, for the instant of a touch.
fn ghost_of(hero: &Hero, id: u32) -> Img {
    juice::impact_frame(&knights::figure(hero, id), 'H')
}

/// A pale wraith caught: a cold ring and sparks where it stood, its crown's
/// gold thrown up, smoke and embers, and what it was worth rising off it.
fn caught(cv: &mut Img, c: &wax_show::Catch, age: u32, tick: u64) {
    let (x, y) = (c.at.0 * super::super::TILE / 16, c.at.1 * super::super::TILE / 16);
    let weight = wax_show::catch_weight(c.chain);
    let seed = u32::from(c.king) * 17 + c.born;
    let a = age as f32;
    let at = (x as f32, y as f32 - 2.0);
    juice::impact_burst(cv, at, age, seed, weight, tick);
    if age < 20 {
        juice::death_burst(cv, at, age, seed ^ 0x77, tick);
    }
    let mut pen = Pen::new(cv, 1500);
    let big = 1.0 + f32::from(c.chain.min(4)) * 0.25;
    pen.ring(at, 3.0 + a * 2.6 * big, 2.0, 1.0, ramps::FROST, a / 30.0);
    if age >= 3 {
        pen.ring(at, (a - 3.0) * 1.8 * big, 1.4, 1.0, ramps::WATER, (a - 3.0) / 26.0);
    }
    let cold = Spray { gravity: 0.05, ..Spray::round(seed ^ 0x3, 26, at, (1.4, 3.6 * big), (10.0, 20.0)) };
    cold.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::FROST, 2, tick));
    let gold = Spray { gravity: 0.13, angle: -std::f32::consts::FRAC_PI_2, spread: 1.4, ..Spray::round(seed ^ 0x9, 10, at, (1.8, 3.8), (14.0, 26.0)) };
    gold.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 2, tick));
    juice::pop_number(cv, &format!("+{}", c.worth), (x, y - 10), age, 40, if c.chain >= 3 { 3 } else { 2 }, '@');
}
