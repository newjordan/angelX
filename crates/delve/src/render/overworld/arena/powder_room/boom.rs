//! The blasts: what a keg going off looks like, in the room's pixels. Every
//! one is a pure function of the blast's age in ticks (`rules::powder_show`
//! keeps what happened), laid under the knights (`under`: the floor's light,
//! the shock rings, the burnt stone's embers, the debris's shadows) or over
//! them (`over`: the white flash, the fireball and its rising smoke, the
//! flames down the arms, the crates coming apart, sparks and embers).
//!
//! A blast has a body and an echo. The body: a flash that whites the whole
//! cross for two ticks, a ball of fire that swells and rises as a column of
//! smoke, a flame standing on each tile of each arm, a ring that runs out
//! across the floor. The echo is what is left: crate planks and iron
//! bouncing across the stone, embers drifting up, soot on the floor
//! (`set::scorch`), a pall hanging a while.
//!
//! A keg that another keg set off waits its turn in the picture (`LINK`
//! ticks a link), so a chain is seen running along the floor.

use super::super::super::ink::{Img, bayer, hash, vnoise};
use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps, unit};
use super::super::TILE;
use super::{bits, walls};
use crate::motion::{ease, math};
use crate::rules::juice::Weight;
use crate::rules::powder_show::{Blast, LINK};

/// The hot colours, white to red to dark: a heat from 1 down to 0 picks one,
/// the ordered dither mixing neighbours so it never bands.
const HEAT: [char; 8] = ['9', '6', '@', '4', 'R', '7', 'p', '8'];

/// The colour of `heat` (1 white-hot, 0 gone) at pixel `(x, y)`, or `None`
/// where it has cooled to nothing.
fn hot(heat: f32, x: i32, y: i32) -> Option<char> {
    if heat <= 0.05 {
        return None;
    }
    let k = (1.0 - heat.min(1.0)) * 7.0;
    let (i, f) = (k.floor() as usize, k.fract());
    // Flat bands, as the arcade drew them: only the last third of a band
    // mixes into the next, so the fire has edges.
    let i = if f > 0.66 && bayer(x, y) < (f - 0.66) / 0.34 { i + 1 } else { i };
    HEAT.get(i.min(7)).copied()
}

/// Whether a blast's picture reaches `view` (room pixels), with `pad`
/// pixels of slack.
fn reaches(b: &Blast, view: (i32, i32), pad: i32) -> bool {
    let (cx, cy) = centre(b.at);
    let far = (i32::from(*b.arms.iter().max().unwrap_or(&0)) + 1) * TILE;
    cx + far + pad >= view.0 && cx - far - pad < view.0 + 384 && cy + far + pad >= view.1 && cy - far - pad < view.1 + 224
}

/// A tile's middle, in room pixels.
pub(super) fn centre(t: (i32, i32)) -> (i32, i32) {
    (t.0 * TILE + TILE / 2, t.1 * TILE + TILE / 2)
}

/// A blast's local age: ticks since its link in the chain went (none while
/// it waits).
pub(super) fn local(b: &Blast, now: u32) -> Option<u32> {
    now.checked_sub(b.born + u32::from(b.depth) * LINK).filter(|_| b.age(now) >= u32::from(b.depth) * LINK)
}

/// How big a blast is drawn: by how it weighed, and how deep a chain.
fn size(b: &Blast) -> f32 {
    // The store going up at the end: many small blasts, not big ones.
    if b.who == 0 {
        return 0.62;
    }
    let w = match b.weight {
        Weight::Graze => 0.82,
        Weight::Solid => 0.94,
        Weight::Crushing => 1.05,
        Weight::Kill => 1.18,
        Weight::Finale => 1.45,
    };
    w * (1.0 + 0.06 * f32::from(b.depth.min(5)))
}

/// Under the knights: each blast's light on the floor and its shock rings.
pub(super) fn under(cv: &mut Img, shows: &[Blast], now: u32, view: (i32, i32)) {
    for b in shows {
        let Some(a) = local(b, now) else { continue };
        if !reaches(b, view, 80) || a >= 40 {
            continue;
        }
        let (cx, cy) = centre(b.at);
        let (fx, fy) = (cx as f32, cy as f32);
        let af = a as f32;
        let big = size(b);
        let mut pen = Pen::new(cv, 1400);
        // The instant of the blast: a bloom of cream light round it, a
        // dither that never covers a figure.
        if a < 3 && b.depth == 0 {
            pen.glow((fx, fy), 58.0 * big, 0.85, ramps::HOLY, [0.75, 0.42, 0.2][a as usize]);
        }
        // The light the fire throws on the floor and the walls.
        if a < 26 {
            let power = 0.85 * (1.0 - af / 26.0) * big.min(1.3);
            // A chain's links light their own patch; the root lights the room.
            let reach = if b.depth == 0 { 54.0 } else { 30.0 };
            pen.glow((fx, fy), reach * big, 0.8, ramps::EMBER, power);
        }
        // The shock: a dark wake in a bright edge running out, then a pale
        // swell behind it; the arms' tiles are lit hotter under their flames.
        // One wave a chain, and a heavy link's own: a ring for every keg
        // would be a target.
        let leads = b.depth == 0 || b.depth % 3 == 0 || b.weight >= Weight::Finale;
        if a < 13 && leads {
            pen.shock((fx, fy + 2.0), 6.0 + af * 6.8 * big, 2.6, 0.62, ramps::EMBER, af / 13.0);
        }
        if (2..20).contains(&a) && leads {
            pen.ring((fx, fy + 2.0), 3.0 + (af - 2.0) * 4.6 * big, 1.6, 0.62, ramps::SMOKE, (af - 2.0) / 18.0);
        }
        for t in b.tiles() {
            let (tx, ty) = centre(t);
            if a < 16 {
                let k = 1.0 - af / 16.0;
                for dy in -6..=6i32 {
                    for dx in -7..=7i32 {
                        let d = (dx * dx) as f32 / 49.0 + (dy * dy) as f32 / 36.0;
                        if d < 1.0 && bayer(tx + dx, ty + dy) < (1.0 - d) * 0.9 * k {
                            let ink = if d < 0.35 && a < 8 { '6' } else if d < 0.7 { '@' } else { 'R' };
                            pen.light(tx + dx, ty + dy, ink);
                        }
                    }
                }
            }
        }
    }
}

/// Over the knights: everything with height.
pub(super) fn over(cv: &mut Img, shows: &[Blast], now: u32, view: (i32, i32), tick: u64) {
    // Far blasts first, so a near one is drawn in front of them.
    let mut order: Vec<&Blast> = shows.iter().collect();
    order.sort_by_key(|b| (b.at.1, b.at.0));
    for b in order {
        let Some(a) = local(b, now) else { continue };
        if !reaches(b, view, 90) {
            continue;
        }
        let seed = (b.at.0 as u32).wrapping_mul(31) ^ (b.at.1 as u32).wrapping_mul(131) ^ b.born;
        if a < 3 {
            flash(cv, b, a);
        }
        crates_pop(cv, b, a);
        // Flames stand down each arm, the far end last.
        for d in 0..4 {
            for i in 1..=i32::from(b.arms[d]) {
                let t = b.arm_tile(d, i);
                let arrive = (i as u32).saturating_sub(1);
                if a >= arrive {
                    let end = i == i32::from(b.arms[d]);
                    flame(cv, centre(t), (a - arrive) as f32, size(b) * if end { 0.85 } else { 1.0 }, seed ^ (d as u32 * 7 + i as u32));
                }
            }
        }
        flame(cv, centre(b.at), a as f32, size(b) * 1.1, seed);
        if b.who != 0 {
            stem(cv, b, a, seed);
        }
        fireball(cv, b, a, seed);
        debris(cv, b, a, seed, tick);
        smoke(cv, b, a, seed);
        sparks(cv, b, a, seed, tick);
        if a < 24 && (b.weight >= Weight::Finale || (b.weight >= Weight::Kill && b.depth == 0)) {
            rim(cv, b, a);
        }
    }
}

/// The first two ticks: the whole cross white, edges first to go.
fn flash(cv: &mut Img, b: &Blast, a: u32) {
    for t in b.tiles() {
        let (cx, cy) = centre(t);
        let (x0, y0) = (cx - 8, cy - 8);
        for py in 0..TILE {
            for px in 0..TILE {
                let edge = px == 0 || py == 0 || px == TILE - 1 || py == TILE - 1;
                let (x, y) = (x0 + px, y0 + py);
                if a == 0 {
                    cv.put(x, y, if edge { '6' } else { '9' });
                } else if a == 1 && (!edge || bayer(x, y) < 0.5) && bayer(x + 2, y) < 0.8 {
                    cv.put(x, y, if bayer(x, y) < 0.4 { '9' } else { '6' });
                } else if a == 2 && bayer(x, y) < 0.3 && !edge {
                    cv.put(x, y, '@');
                }
            }
        }
    }
}

/// The crates the blast broke, in the instant they break: whited, swelling,
/// then gone to planks.
fn crates_pop(cv: &mut Img, b: &Blast, a: u32) {
    if a >= 4 {
        return;
    }
    for &(tx, ty) in &b.crates {
        let img = walls::crate_img(walls::crate_kind(tx, ty));
        let lit = juice::impact_frame(img, if a < 2 { 'H' } else { '9' });
        let (cx, cy) = centre((tx, ty));
        let k = 1.0 + 0.12 * a as f32;
        let big = super::super::enlarge(&lit, k);
        cv.stamp(&big, cx - big.w / 2, cy - big.h / 2 - a as i32);
    }
}

/// How high and how hot a flame stands `a` ticks after it caught: up in three
/// ticks, full while the fire is dangerous, then guttering out.
fn envelope(a: f32) -> f32 {
    if a < 0.0 {
        0.0
    } else if a < 3.0 {
        ease::out(a / 3.0)
    } else if a < 10.0 {
        1.0
    } else {
        (1.0 - (a - 10.0) / 7.0).max(0.0)
    }
}

/// A flame standing on tile `(cx, cy)`: tongues licking up from a pool of
/// fire, white at the root, going to red at the tips, the tips dithered away.
pub(super) fn flame(cv: &mut Img, (cx, cy): (i32, i32), a: f32, size: f32, seed: u32) {
    let env = envelope(a);
    if env <= 0.02 {
        return;
    }
    let base = cy + 6;
    let hmax = 25.0 * size * env;
    // The pool on the floor: a squashed ellipse of fire under it.
    for dy in -3..=3i32 {
        for dx in -8..=8i32 {
            let d = (dx * dx) as f32 / 64.0 + (dy * dy) as f32 / 9.0;
            if d < 1.0 {
                let (x, y) = (cx + dx, base + dy);
                if let Some(ink) = hot(0.55 + 0.4 * (1.0 - d) * env - (a / 40.0), x, y) {
                    cv.put(x, y, ink);
                }
            }
        }
    }
    for dx in -7..=7i32 {
        let col = (1.0 - (dx.abs() as f32 / 7.8).powf(3.0)).max(0.0);
        // Each column licks on its own beat, two noises over each other: a
        // slow swell of the whole tongue, a quick flutter of its tip.
        let n = vnoise(dx as f32 * 0.5 + a * 0.2, a * 0.7, seed);
        let m = vnoise(dx as f32 * 1.4 - a * 0.6, a * 1.9, seed ^ 0x7);
        let h = hmax * col * (0.5 + 0.65 * n) * (0.82 + 0.36 * m);
        for dy in 0..h.ceil() as i32 {
            let t = dy as f32 / h.max(1.0);
            if t > 0.78 && bayer(cx + dx, base - dy) < (t - 0.78) * 4.2 {
                continue;
            }
            let heat = (1.05 - t * 0.9) * (0.5 + 0.5 * col) + 0.08 * n - a * 0.01;
            if let Some(ink) = hot(heat, cx + dx, base - dy) {
                cv.put(cx + dx, base - dy, ink);
            }
        }
    }
}

/// The ball of fire over the keg: a cauliflower of seven overlapping
/// lobes that swells in six ticks, then rises and cools through red into
/// the smoke it turns to, its lobes turning as it boils.
fn fireball(cv: &mut Img, b: &Blast, a: u32, seed: u32) {
    if a >= 30 {
        return;
    }
    let af = a as f32;
    let big = size(b) * (0.9 + 0.3 * unit(b.at.0, b.at.1, seed)) * 1.35;
    let r = if a < 6 { (4.0 + 11.0 * ease::out(af / 6.0)) * big } else { 15.0 * big * (1.0 - ((af - 6.0) / 24.0).powf(1.25)).max(0.0) };
    if r < 1.5 {
        return;
    }
    let (cx, cy) = centre(b.at);
    let rise = af * 0.55 + if a >= 6 { (af - 6.0) * 0.25 } else { 0.0 };
    let (fx, fy) = (cx as f32, cy as f32 - 4.0 - rise);
    // The lobes: a heart and six round it, each turning its own way.
    let lobes = if b.who == 0 { 4 } else { 7 };
    let mut balls = [(fx, fy, r * 0.8); 7];
    for (i, ball) in balls.iter_mut().enumerate().skip(1).take(lobes - 1) {
        let u = |k: u32| unit(i as i32, seed as i32, k);
        let ang = u(1) * std::f32::consts::TAU + af * 0.05 * if i % 2 == 0 { 1.0 } else { -1.0 };
        let dist = r * (0.36 + 0.36 * u(2)) * (0.85 + 0.15 * math::sin(af * 0.3 + i as f32));
        *ball = (fx + math::cos(ang) * dist * 1.12, fy + math::sin(ang) * dist * 0.88 - r * 0.12 * u(3), r * (0.42 + 0.3 * u(4)));
    }
    let cool = 1.2 - af / 30.0;
    let beat = (a / 3) as i32;
    for y in (fy - r * 1.3).floor() as i32..=(fy + r * 1.3).ceil() as i32 {
        for x in (fx - r * 1.4).floor() as i32..=(fx + r * 1.4).ceil() as i32 {
            let mut least = 1.0f32;
            for &(bx, by, br) in &balls[..lobes] {
                let (ox, oy) = (x as f32 + 0.5 - bx, y as f32 + 0.5 - by);
                if ox.abs() >= br || oy.abs() >= br {
                    continue;
                }
                let (dx, dy) = (ox / br, oy / (br * 0.95));
                least = least.min(dx * dx + dy * dy);
            }
            if least >= 1.0 {
                continue;
            }
            let best = 1.0 - least.sqrt();
            // Hot at the heart, a rolling pattern in it (chunks that change
            // every few ticks); lit from above.
            let swirl = 0.14 * ((hash(x >> 1, (y >> 1) + beat, seed ^ 0x55) & 0xff) as f32 / 255.0);
            let lit = -((y as f32 - fy) / r) * 0.16;
            let heat = best.powf(0.8) * 1.12 * cool + swirl + lit;
            match hot(heat, x, y) {
                Some(ink) => cv.put(x, y, ink),
                None => {
                    // Cooled: the ball goes to smoke at its edge.
                    if af > 8.0 && bayer(x, y) < 0.6 * (1.0 - (af - 8.0) / 22.0).max(0.0) {
                        cv.put(x, y, if best > 0.2 { 'X' } else { 'g' });
                    }
                }
            }
        }
    }
}

/// The stalk under the fireball: a column of dark smoke from the floor up
/// to the ball as it rises, so the blast is a mushroom and not a bubble.
fn stem(cv: &mut Img, b: &Blast, a: u32, seed: u32) {
    if !(5..34).contains(&a) {
        return;
    }
    let af = a as f32;
    let (cx, cy) = centre(b.at);
    let rise = af * 0.55 + (af - 6.0).max(0.0) * 0.25;
    let top = cy as f32 - 4.0 - rise;
    let k = 1.0 - (af - 5.0) / 29.0;
    let big = size(b);
    let mut y = top;
    while y < cy as f32 + 2.0 {
        let t = (y - top) / (cy as f32 + 2.0 - top).max(1.0);
        let w = (2.0 + 4.0 * (1.0 - (t - 0.5).abs() * 1.4).max(0.0)) * big * k.max(0.3);
        let sway = vnoise(y * 0.15, af * 0.1, seed ^ 0x3) * 3.0 - 1.5;
        for dx in -(w as i32)..=(w as i32) {
            let (x, yy) = (cx + dx + sway as i32, y as i32);
            if bayer(x, yy) < 0.75 * k + 0.1 && cv.get(x, yy).is_none() {
                let lit = if dx < 0 { 'G' } else { 'j' };
                cv.put(x, yy, if dx.abs() as f32 > w - 1.0 { 'g' } else { lit });
            }
        }
        y += 1.0;
    }
}

/// Pieces of a crate thrown out of a blast, each its own arc with the floor
/// to bounce on: planks, chunks, splinters and bent iron, their shadows
/// small on the stone. Back to rest by the end of their life, blinking out.
fn debris(cv: &mut Img, b: &Blast, a: u32, seed: u32, tick: u64) {
    const LIFE: u32 = 52;
    if a >= LIFE {
        return;
    }
    let (bx, by) = centre(b.at);
    for (j, &(tx, ty)) in b.crates.iter().enumerate() {
        let (cx, cy) = centre((tx, ty));
        let away = ((cx - bx) as f32, (cy - by) as f32);
        let len = away.0.hypot(away.1).max(1.0);
        let dir = (away.0 / len, away.1 / len);
        for i in 0..if b.who == 0 { 9u32 } else { 16 } {
            let s = seed ^ (j as u32 * 977) ^ (i * 131);
            let u = |k: u32| unit(i as i32, s as i32, k);
            let speed = 1.4 + 3.0 * u(1);
            let mut vx = dir.0 * speed * (0.6 + 0.8 * u(2)) + (u(3) - 0.5) * 2.6;
            let mut vy = dir.1 * speed * 0.7 * (0.6 + 0.8 * u(4)) + (u(5) - 0.5) * 1.8;
            let mut vz = 3.0 + 4.0 * u(6);
            let (mut x, mut y, mut z) = (cx as f32, cy as f32, 5.0f32);
            let mut still = false;
            for _ in 0..a {
                if still {
                    break;
                }
                x += vx;
                y += vy;
                vz -= 0.36;
                z += vz;
                vx *= 0.97;
                vy *= 0.97;
                if z <= 0.0 {
                    z = 0.0;
                    vz = -vz * 0.4;
                    vx *= 0.6;
                    vy *= 0.6;
                    if vz < 0.7 {
                        still = true;
                    }
                }
            }
            if a > LIFE - 12 && bayer(i as i32 * 3, a as i32) < (a - (LIFE - 12)) as f32 / 12.0 {
                continue;
            }
            let (px, py) = (x.round() as i32, y.round() as i32);
            if z > 1.0 {
                cv.put(px, py + 1, 'k');
            }
            let spin = (tick as u32 / 2 + i) % 4;
            let top = py - z.round() as i32;
            // Pieces of one crate are the crate's own browns, with iron.
            match i % 6 {
                0 | 1 => {
                    // A plank, lit on its top.
                    let (w, h) = if spin % 2 == 0 { (5, 2) } else { (2, 5) };
                    cv.rect(px, top, w, h, if i % 2 == 0 { 'o' } else { 'r' });
                    cv.rect(px, top, if w > h { w } else { 1 }, if w > h { 1 } else { h }, 'T');
                    cv.put(px + w - 1, top + h - 1, 'p');
                }
                2 | 3 => {
                    // A chunk of lid or plank end.
                    cv.rect(px, top, 3, 3, 'r');
                    cv.rect(px, top, 3, 1, 'O');
                    cv.put(px + 2, top + 2, 'p');
                    cv.put(px, top + 1, 'o');
                }
                4 => {
                    // Bent iron, bright where it catches the fire.
                    cv.rect(px, top, 2, 2, 'J');
                    cv.put(px + i32::from(spin < 2), top + i32::from(spin >= 2), 'h');
                    cv.put(px + 1, top + 1, 'g');
                }
                _ => {
                    // A splinter, and a spark of it burning.
                    let (dx, dy) = [(1, -1), (1, 0), (1, 1), (0, 1)][spin as usize];
                    cv.put(px, top, 'T');
                    cv.put(px + dx, top + dy, 'o');
                    cv.put(px + dx * 2, top + dy * 2, 'O');
                    if a < 14 {
                        cv.put(px - dx, top - dy, '6');
                    }
                }
            }
        }
    }
}

/// The pall: columns of black-powder smoke standing off the centre and every
/// arm tile, billowing, drifting off down the room's wind, going pale. Never
/// drawn over a figure, so a knight is never lost in it. Greys that read on
/// the dark stone: the smoke is the lightest thing a blast leaves.
fn smoke(cv: &mut Img, b: &Blast, a: u32, seed: u32) {
    if a < 3 {
        return;
    }
    let mut sources = vec![(centre(b.at), if b.who == 0 { 2u32 } else { 6 })];
    for d in 0..4 {
        for i in 1..=i32::from(b.arms[d]) {
            sources.push((centre(b.arm_tile(d, i)), 3));
        }
    }
    for (si, &((sx, sy), puffs)) in sources.iter().enumerate() {
        for k in 0..puffs {
            let u = |n: u32| unit(si as i32 * 13 + k as i32, seed as i32, n);
            let start = 3.0 + k as f32 * 3.5 + u(1) * 4.0;
            let life = 56.0 + 34.0 * u(2);
            let t = a as f32 - start;
            if t < 0.0 || t >= life {
                continue;
            }
            let p = t / life;
            let wind = 0.12 + 0.07 * u(3);
            let (x, y) = (
                sx as f32 + (u(4) - 0.5) * 8.0 + t * wind + math::sin(t * 0.1 + u(5) * 6.0) * 2.2,
                sy as f32 - 6.0 - t * 0.36 - t * t * 0.004,
            );
            let r = (4.0 + 8.0 * p.powf(0.6)) * if si == 0 { 1.3 } else { 1.0 };
            let density = 1.0 * (1.0 - p).powf(1.05) * (if si == 0 { 1.0 } else { 0.85 });
            puff(cv, (x, y), r, density, p, seed ^ (si as u32 * 31 + k), true);
        }
    }
}

/// One puff: a dithered disc, grey with a lit upper left and a shaded
/// underside; only on bare paper when `protect`.
pub(super) fn puff(cv: &mut Img, (cx, cy): (f32, f32), r: f32, density: f32, age: f32, seed: u32, protect: bool) {
    // The lobed edge: noise at the corners of a four by four grid over the
    // puff, blended across it.
    let mut grid = [[0.0f32; 4]; 4];
    for (j, row) in grid.iter_mut().enumerate() {
        for (i, v) in row.iter_mut().enumerate() {
            *v = 0.7 + 0.5 * ((hash(i as i32, j as i32, seed) & 0xff) as f32 / 255.0);
        }
    }
    for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
        for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / r, (y as f32 + 0.5 - cy) / r);
            let d2 = dx * dx + dy * dy;
            if d2 >= 1.7 {
                continue;
            }
            let (gx, gy) = (((dx + 1.0) * 1.5).clamp(0.0, 2.999), ((dy + 1.0) * 1.5).clamp(0.0, 2.999));
            let (i, j, fx, fy) = (gx as usize, gy as usize, gx.fract(), gy.fract());
            let top = grid[j][i] + (grid[j][i + 1] - grid[j][i]) * fx;
            let bot = grid[j + 1][i] + (grid[j + 1][i + 1] - grid[j + 1][i]) * fx;
            let lobe = top + (bot - top) * fy;
            let d = d2.sqrt();
            if d >= lobe {
                continue;
            }
            // Solid in its body, its edge broken by the dither, the whole of
            // it thinning as it ages.
            let o = density * (1.7 - 1.3 * (d / lobe).powf(1.6));
            if bayer(x, y) >= o || (protect && cv.get(x, y).is_some()) {
                continue;
            }
            // Lit from the upper left, shaded below; paler as it ages.
            let lit = -(dx + dy) * 0.5;
            let tone = 0.45 + lit * 0.5 + age * 0.35;
            let ink = if tone < 0.3 {
                'g'
            } else if tone < 0.5 {
                'j'
            } else if tone < 0.7 {
                'G'
            } else if tone < 0.9 {
                'J'
            } else {
                'h'
            };
            cv.put(x, y, ink);
        }
    }
}

/// Sparks thrown from the centre and each arm's end, and embers that rise
/// and last.
fn sparks(cv: &mut Img, b: &Blast, a: u32, seed: u32, tick: u64) {
    if a >= 70 {
        return;
    }
    let (cx, cy) = centre(b.at);
    let big = size(b);
    let mut pen = Pen::new(cv, 700);
    let af = a as f32;
    let n = if b.who == 0 { 7 } else { (22.0 * big) as u32 };
    let burst = Spray { gravity: 0.1, drag: 0.07, ..Spray::round(seed ^ 0x51, n, (cx as f32, cy as f32 - 3.0), (1.6 * big, 4.8 * big), (9.0, 22.0)) };
    burst.each(af, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 2, tick));
    for d in 0..4 {
        let len = i32::from(b.arms[d]);
        if len == 0 {
            continue;
        }
        let end = centre(b.arm_tile(d, len));
        let jets = Spray { gravity: 0.1, drag: 0.08, ..Spray::round(seed ^ (0x77 + d as u32), 7, (end.0 as f32, end.1 as f32 - 3.0), (0.8, 3.0), (8.0, 16.0)) };
        jets.each(af, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::EMBER, 1, tick));
    }
    // Embers climb out of the smoke and last.
    let embers = Spray { gravity: -0.012, drag: 0.03, stagger: 1.5, ..Spray::round(seed ^ 0x9, if b.who == 0 { 3 } else { (14.0 * big) as u32 }, (cx as f32, cy as f32 - 6.0), (0.3, 1.5), (30.0, 64.0)) };
    embers.each(af, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::EMBER, 1, tick));
}

/// A heavy blast kicks a ring of dithered light round the whole view's
/// edge for a moment: the room feels it. Drawn from the blast's own height,
/// so it is a mark of weight rather than a second flash.
fn rim(cv: &mut Img, b: &Blast, a: u32) {
    let (cx, cy) = centre(b.at);
    let big = if b.weight >= Weight::Finale { 1.5 } else { 1.0 };
    let r = 10.0 + a as f32 * 16.0 * big;
    let mut pen = Pen::new(cv, 3200);
    pen.ring((cx as f32, cy as f32 + 2.0), r, 1.6, 0.62, ramps::EMBER, a as f32 / 24.0);
}

/// A keg that a blast has set going and that waits its turn (the chain's
/// links): drawn white-hot where it stands, shaking, the picture's own
/// reading of "next".
pub(super) fn primed(cv: &mut Img, b: &Blast, now: u32) {
    let wait = u32::from(b.depth) * LINK;
    let age = b.age(now);
    if b.depth == 0 || age >= wait {
        return;
    }
    let (cx, cy) = centre(b.at);
    let shake = if age % 2 == 0 { 1 } else { 0 };
    let keg = bits::keg(if age % 2 == 0 { 3 } else { 2 });
    cv.stamp(keg, cx - 6 + shake, cy - 7);
}
