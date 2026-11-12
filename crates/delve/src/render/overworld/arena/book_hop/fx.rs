//! Book Hop's moments in the room's own pixels, over the knights, each a
//! pure function of an age in ticks (`rules::hop_show` keeps what
//! happened): a landing's thud and paper dust, a knight sinking, ringed
//! and bubbled; a crossing's gold and bookmarks over the far bank and the
//! knight dropping back onto the near one; the archive flaring when the
//! table has all crossed; an arrow over each knight and, for the seat
//! looking, a flag where a book is carrying them into the wall.

use super::super::super::ink::{Img, bayer, hash};
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::super::{TILE, at, juice, knights};
use super::bits;
use crate::motion::ease;
use crate::rules::hop::{self, Hop};
use crate::rules::hop_show::{CROSS_LIFE, LAND_LIFE, LIT_LIFE, SINK_LIFE, Sink};
use crate::rules::{Run, TILE_UNITS};

/// The water thrown up: pale at the top, the river's own at the foot.
const INK: Ramp = Ramp(['V', 'U', 'z', 'u', 'S', 'x']);
/// Paper dust off a book: cream down to its shadow.
const PAPER: Ramp = Ramp(['c', '$', 'i', 'h', 'J', 'G']);
/// A bookmark's silk.
const SILK: Ramp = Ramp(['7', 'R', 'p', 'B', 'b', 'n']);

/// How long the knight takes to go under, in ticks.
const GOING: u32 = 16;

/// The knight sunk, going under: their figure (white for two ticks, then
/// itself) with its feet taken by the water line a row at a time.
fn going_under(cv: &mut Img, run: &Run, s: &Sink, age: u32) {
    let who = u32::from(s.who);
    let Some(hero) = run.players.get(&who) else { return };
    if age >= GOING {
        return;
    }
    let mut fig = knights::figure(hero, who);
    if hero.aim_x < -0.3 {
        fig = fig.flip_h();
    }
    if age < 2 {
        fig = juice::impact_frame(&fig, 'H');
    }
    let under = (ease::in_quad(age as f32 / GOING as f32) * fig.h as f32) as i32;
    let show = fig.h - under;
    if show <= 0 {
        return;
    }
    let (x, y) = s.at;
    // The upper part of the figure, its foot on the water line; ink dithered
    // over its lowest rows, as the river climbs it.
    for sy in 0..show {
        for sx in 0..fig.w {
            let Some(c) = fig.get(sx, sy) else { continue };
            let wet = show - sy <= 3 && bayer(x + sx, y + sy) < 0.55;
            cv.set(x - fig.w / 2 + sx, y + 1 - show + sy, if wet { super::super::super::ink::ink('u').unwrap_or(c) } else { c });
        }
    }
}

/// A knight sunk: the ring of water thrown up, rings on the river, bubbles
/// rising, and for a wall the dust off the stone.
fn sunk(cv: &mut Img, run: &Run, s: &Sink, now: u32, tick: u64) {
    let age = now.saturating_sub(s.born);
    if age >= SINK_LIFE {
        return;
    }
    let (x, y) = (s.at.0 as f32, s.at.1 as f32);
    going_under(cv, run, s, age);
    let seed = hash(s.at.0, s.at.1, u32::from(s.who)) & 0xffff;
    let mut pen = Pen::new(cv, 500);
    // The crown of water.
    let crown = Spray { angle: -std::f32::consts::FRAC_PI_2, spread: 1.9, gravity: 0.17, drag: 0.04, ..Spray::round(seed, 30, (x, y - 1.0), (1.6, 4.4), (14.0, 28.0)) };
    crown.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Droplet, INK, 2, tick));
    // The plume: a column of water standing up out of the hole, then falling in.
    if age < 12 {
        let up = (crate::motion::math::sin(age as f32 / 12.0 * std::f32::consts::PI) * 17.0) as i32;
        for dy in 0..up {
            let half = 1 + (dy < 4) as i32;
            for dx in -half..=half {
                let ink = if dx.abs() == half { 'u' } else if dy > up - 3 { 'V' } else { 'U' };
                if dy + 2 >= up || bayer(x as i32 + dx, y as i32 - dy) < 0.85 {
                    pen.put(x as i32 + dx, y as i32 - dy, ink);
                }
            }
        }
    }
    // Rings on the river, four in turn.
    for j in 0..4u32 {
        if age >= j * 4 && age - j * 4 < 26 {
            let a = (age - j * 4) as f32;
            pen.ring((x, y + 1.0), 3.0 + a * 1.15, 1.1, 0.42, INK, a / 26.0);
        }
    }
    // Bubbles, once the water has closed over.
    if age >= 6 {
        let bubbles = Spray { angle: -std::f32::consts::FRAC_PI_2, spread: 0.7, gravity: -0.004, drag: 0.03, stagger: 4.0, ..Spray::round(seed ^ 0x5a, 5, (x, y), (0.15, 0.45), (22.0, 36.0)) };
        bubbles.each((age - 6) as f32, |m| pfx::mote(&mut pen, &m, Shape::Bubble, INK, 1, tick));
    }
    if s.wall {
        // A book against the stone: grit and a spark.
        let side = if s.at.0 < 192 { -1.0 } else { 1.0 };
        let grit = Spray { angle: if side < 0.0 { 0.0 } else { std::f32::consts::PI }, spread: 1.4, gravity: 0.06, ..Spray::round(seed ^ 0x77, 10, (x + side * 4.0, y - 4.0), (0.8, 2.2), (10.0, 20.0)) };
        grit.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::SMOKE, 1, tick));
        if age < 6 {
            pfx::rays(&mut pen, (x, y - 4.0), 6, 10.0, 0.8, 0.3, ramps::GOLD, age as f32 / 6.0);
        }
    }
}

/// A knight set down: a ring on the book (or the stone), a puff of paper.
fn landed(cv: &mut Img, l: &crate::rules::hop_show::Land, now: u32, tick: u64) {
    let age = now.saturating_sub(l.born);
    if age >= LAND_LIFE {
        return;
    }
    let (x, y) = (l.at.0 as f32, l.at.1 as f32);
    let mut pen = Pen::new(cv, 120);
    let a = age as f32;
    pen.ring((x, y + 2.0), 2.0 + a * 0.55, 0.9, 0.4, if l.lane == 255 { ramps::SMOKE } else { INK }, a / LAND_LIFE as f32);
    let dust = Spray { gravity: 0.03, squash: 0.5, ..Spray::round(hash(l.at.0, l.at.1, 3) & 0xff, 5, (x, y + 1.0), (0.5, 1.3), (8.0, 16.0)) };
    dust.each(a, |m| pfx::mote(&mut pen, &m, Shape::Dot, if l.lane == 255 { ramps::SMOKE } else { PAPER }, 1, tick));
    // On the very edge of the book: a gold flick, a near thing.
    if l.edge && age < 14 {
        let tings = Spray { gravity: 0.03, drag: 0.1, ..Spray::round(hash(l.at.0, l.at.1, 5) & 0xff, 6, (x, y - 5.0), (0.7, 1.6), (9.0, 15.0)) };
        tings.each(a, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, tick));
    }
}

/// A knight over the far bank: gold and bookmarks thrown up, a ring of light
/// on the stone, a column of sparkle; and the knight dropping back in on the
/// near bank, a ring and a fall of paper.
fn crossed(cv: &mut Img, c: &crate::rules::hop_show::Cross, now: u32, rows: i32, tick: u64) {
    let age = now.saturating_sub(c.born);
    if age >= CROSS_LIFE {
        return;
    }
    let (x, y) = (c.at.0 as f32, (hop::far_bank(rows) * TILE + 12) as f32);
    let a = age as f32;
    let big = c.clean >= 3;
    let seed = hash(c.at.0, c.n as i32, u32::from(c.who)) & 0xffff;
    let mut pen = Pen::new(cv, 700);
    if age < 30 {
        let n = if big { 48 } else { 32 };
        let up = Spray { gravity: 0.06, drag: 0.05, ..Spray::round(seed, n, (x, y - 4.0), (1.0, 4.0), (16.0, 32.0)) };
        up.each(a, |m| pfx::mote(&mut pen, &m, if m.i % 3 == 0 { Shape::Star } else { Shape::Coin }, ramps::GOLD, 1 + u8::from(big), tick));
        let ribbons = Spray { gravity: 0.03, drag: 0.06, stagger: 1.0, ..Spray::round(seed ^ 0x33, 9, (x, y - 6.0), (0.6, 2.0), (22.0, 40.0)) };
        ribbons.each(a, |m| pfx::mote(&mut pen, &m, Shape::Petal, SILK, 1, tick));
        pen.shock((x, y), 4.0 + a * 3.2, 1.8, 0.45, ramps::GOLD, a / 22.0);
        if big {
            pen.shock((x, y), 2.0 + a * 5.0, 1.5, 0.45, ramps::EMBER, a / 18.0);
            if age < 8 {
                pfx::rays(&mut pen, (x, y - 8.0), 10, 26.0, 0.8, 0.2, ramps::GOLD, a / 8.0);
            }
        }
    }
    // The column of sparkle where they stood, rising a moment.
    if age < 10 {
        for dy in 0..22i32 {
            let fall = 1.0 - dy as f32 / 22.0;
            for dx in -2..=2i32 {
                if bayer(x as i32 + dx, y as i32 - dy) < fall * (1.0 - a / 10.0) * 0.8 {
                    cv.put(x as i32 + dx, y as i32 - 4 - dy, if dx == 0 { 'w' } else { 'T' });
                }
            }
        }
    }
    // Back on the near bank: they drop in on a ring.
    if (5..26).contains(&age) {
        let ny = (hop::near_bank(rows) * TILE + 12) as f32;
        let b = (age - 5) as f32;
        let mut pen = Pen::new(cv, 300);
        pen.ring((x, ny), 3.0 + b * 0.8, 1.0, 0.4, ramps::HOLY, b / 21.0);
        let paper = Spray { angle: std::f32::consts::FRAC_PI_2, spread: 0.6, gravity: 0.05, drag: 0.08, stagger: 1.0, ..Spray::round(seed ^ 0x12, 8, (x, ny - 26.0), (0.4, 1.2), (10.0, 18.0)) };
        paper.each(b, |m| pfx::mote(&mut pen, &m, Shape::Flake, PAPER, 1, tick));
    }
}

/// The archive lit: a sweep of gold along the far bank, motes off every
/// lantern and feathers off the owls.
fn flare(cv: &mut Img, h: &Hop, cols: i32, now: u32, tick: u64) {
    let Some(f) = h.show.flared else { return };
    let age = now.saturating_sub(f);
    if age >= LIT_LIFE {
        return;
    }
    let w = cols * TILE;
    let a = age as f32;
    // The sweep: a dithered band of gilt crossing the bank in a third of a second.
    if age < 18 {
        let sx = (ease::out(a / 18.0) * (w + 40) as f32) as i32 - 20;
        for y in 14..34 {
            for dx in -18..=18i32 {
                let fall = 1.0 - dx.abs() as f32 / 19.0;
                if bayer(sx + dx, y) < fall * 0.8 {
                    cv.put(sx + dx, y, if dx.abs() < 3 { 'T' } else { 'o' });
                }
            }
        }
    }
    let mut pen = Pen::new(cv, 500);
    let n = super::set::lantern_count(h);
    for k in 0..n {
        let (lx, ly) = super::set::lantern_at(cols, k, n);
        let sparks = Spray { angle: std::f32::consts::FRAC_PI_2, spread: 2.6, gravity: 0.04, drag: 0.05, ..Spray::round(31 + k as u32, 12, (lx as f32, ly as f32 - 2.0), (0.6, 2.2), (18.0, 34.0)) };
        sparks.each(a, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, tick));
    }
    let feathers = Spray { gravity: 0.012, drag: 0.03, stagger: 2.0, ..Spray::round(77, 22, (w as f32 / 2.0, 8.0), (0.5, 3.4), (40.0, 70.0)) };
    feathers.each(a, |m| pfx::mote(&mut pen, &m, Shape::Petal, ramps::EARTH, 1, tick));
}

/// An arrow over every knight, in their own ink; the seat looking bobs more.
fn arrows(cv: &mut Img, run: &Run, h: &Hop, focus: Option<u32>, tick: u32) {
    for &id in h.crossings.keys() {
        let Some(hero) = run.players.get(&id).filter(|p| p.hp > 0 && !p.stone) else { continue };
        let (x, y) = at(hero.x, hero.y);
        let mine = focus == Some(id);
        let bob = if mine {
            [0, -1, -2, -1][(tick / 4 % 4) as usize]
        } else {
            [0, 0, -1, -1][(tick / 6 % 4) as usize]
        };
        let arrow = bits::arrow(knights::seat_mark(id));
        // Over the head: the figure stands about twenty pixels tall.
        let ay = (y - 33 + bob).max(1);
        cv.stamp(&arrow, x - arrow.w / 2, ay);
        // A run of clean crossings burns beside the arrow.
        let run_of = h.show.run_of(id);
        if run_of >= 2 {
            let fire = bits::flame(tick / 3 + id);
            cv.stamp(fire, x + arrow.w / 2, ay - 1);
            if run_of >= 4 {
                cv.stamp(bits::flame(tick / 3 + id + 1), x - arrow.w / 2 - fire.w, ay - 1);
            }
        }
    }
}

/// A flag on the wall a book is carrying the seat looking into, flashing
/// as it nears: a bar of red dither the lane's height and an arrow.
fn warning(cv: &mut Img, run: &Run, h: &Hop, focus: Option<u32>, tick: u32) {
    let Some(me) = focus else { return };
    let Some(hero) = run.players.get(&me).filter(|p| p.hp > 0 && !p.stone) else { return };
    if h.titled() || h.done {
        return;
    }
    let (cols, rows) = (run.room().cols as i32, run.room().rows as i32);
    let row = (hero.y / TILE_UNITS) as i32;
    let Some(l) = hop::lane(rows, row) else { return };
    let width = cols as f32 * TILE_UNITS;
    let t = h.t.saturating_sub(crate::rules::folly::TITLE);
    if !hop::afloat_with(l, t, width, hero.x, hop::EDGE_GRACE) {
        return;
    }
    let dir = if l % 2 == 0 { 1.0 } else { -1.0 };
    let (px, _) = at(hero.x, hero.y);
    let wall = if dir > 0.0 { (cols - 1) * TILE } else { TILE };
    let d = ((wall - px) as f32 * dir).max(0.0);
    let reach = 60.0;
    if d >= reach {
        return;
    }
    let k = 1.0 - d / reach;
    let on = d > 30.0 || tick / 3 % 2 == 0;
    if !on {
        return;
    }
    let y0 = row * TILE;
    let x0 = if dir > 0.0 { wall - 3 } else { wall };
    for y in y0..y0 + TILE {
        for x in x0..x0 + 3 {
            if bayer(x, y) < 0.35 + k * 0.6 {
                cv.put(x, y, if k > 0.6 { 'R' } else { '7' });
            }
        }
    }
    // A bang mark on the stone beside it.
    let bang = bit_bang();
    cv.stamp(&bang, if dir > 0.0 { wall - 8 } else { wall + 4 }, y0 + 4);
}

fn bit_bang() -> Img {
    let mut im = Img::from_rows(&["..c..", "..c..", "..c..", "..c..", ".....", "..c.."]);
    for y in 0..im.h {
        for x in 0..im.w {
            if im.get(x, y).is_some() {
                im.put(x, y, 'R');
            }
        }
    }
    im.outline_outside('k');
    im
}

/// What the hop does over the knights, in the room's pixels.
pub(super) fn over(cv: &mut Img, run: &Run, h: &Hop, focus: Option<u32>) {
    let now = run.tick as u32;
    let rows = run.room().rows as i32;
    let cols = run.room().cols as i32;
    for s in &h.show.sinks {
        sunk(cv, run, s, now, run.tick);
    }
    for l in &h.show.lands {
        landed(cv, l, now, run.tick);
    }
    for c in &h.show.crosses {
        crossed(cv, c, now, rows, run.tick);
    }
    flare(cv, h, cols, now, run.tick);
    if !h.done {
        warning(cv, run, h, focus, now);
        arrows(cv, run, h, focus, now);
    }
}
