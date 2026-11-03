//! A minion's finisher, drawn (`rules::gore`): the body kicked back and
//! pinned, knocked skidding, beheaded, cut in half, burst into chunks or
//! splattered, with the spray, the chunks and the stains it leaves.
//!
//! Everything here is a pure function of a `Gore` record and the run's
//! tick: the body's flight, every drop of the spray and every chunk are
//! worked out each frame from the record's seed, so a seat, a page and the
//! window draw the same pixels from the same run and nothing is kept
//! between frames. Two passes, like the rest of the fight's effects: what
//! lies on the floor or hangs on the wall goes under the figures (`under`),
//! what is in the air over them (`over`). Each pass spends at most `MOTES`
//! motes on spray, chunks and stains; the bodies themselves are a few
//! sprites. Chunky pixel gore in the palette's dark reds and ashes: no
//! stark white, and the colours of a thing come from what it was made of
//! (`rules::gore::body`): blood, bones and dust, ichor, embers and ash,
//! shards, splinters.

use super::super::ink::{Img, Rgb, bayer, hash, ink, shade};
use super::{UNIT, sprites, stature, vfx};
use crate::rules::gore::{BODY, Body, Cut, Gore, HOLD, LIFE, Pin, body};
use crate::rules::{EnemyKind, Run, Shot};
use std::cell::RefCell;
use std::f32::consts::{FRAC_PI_2, PI};
use std::rc::Rc;

/// The most motes one pass spends, all the finishers together.
const MOTES: u32 = 420;
/// Ticks of fade at the end of a body's, and a stain's, life.
const FADE: u64 = 36;

std::thread_local! {
    /// A kind's standing picture, at the size it stands (by tenths of the
    /// kind's own radius), made once.
    static BODIES: RefCell<Vec<(EnemyKind, u8, Rc<Img>)>> = const { RefCell::new(Vec::new()) };
}

fn sprite(kind: EnemyKind, r: f32) -> Rc<Img> {
    let k = (r / kind.radius()).clamp(1.0, 3.0);
    let key = (k * 10.0).round() as u8;
    if let Some(found) = BODIES.with(|b| b.borrow().iter().find(|e| e.0 == kind && e.1 == key).map(|e| e.2.clone())) {
        return found;
    }
    let base = sprites::enemy(kind, 0);
    let im = if kind == EnemyKind::Slime {
        // A slime is drawn as big as it is.
        stature::monster(kind, base, r / sprites::SLIME_REACH)
    } else {
        stature::monster(kind, base, k)
    };
    let im = Rc::new(im);
    BODIES.with(|b| b.borrow_mut().push((kind, key, im.clone())));
    im
}

/// The mote budget of a pass.
struct Pen<'a> {
    cv: &'a mut Img,
    left: u32,
}

impl Pen<'_> {
    fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if self.left > 0 {
            self.left -= 1;
            self.cv.set(x, y, c);
        }
    }
    fn put(&mut self, x: i32, y: i32, ch: char) {
        if let Some(c) = ink(ch) {
            self.set(x, y, c);
        }
    }
}

fn rgb(ch: char) -> Rgb {
    ink(ch).unwrap_or([0, 0, 0])
}

/// What the blood, or what stands for it, looks like: wet and bright, wet,
/// dry, and the darkest of a pool's heart.
#[derive(Clone, Copy)]
struct Pal {
    fresh: Rgb,
    wet: Rgb,
    dry: Rgb,
    dark: Rgb,
    /// Dust and bone, not a wet thing: less of it, and no pool.
    dust: bool,
}

fn pal(kind: EnemyKind, stuff: Body) -> Pal {
    match stuff {
        Body::Flesh => Pal { fresh: rgb('7'), wet: rgb('7'), dry: rgb('8'), dark: rgb('Z'), dust: false },
        Body::Bone => Pal { fresh: rgb('h'), wet: rgb('J'), dry: rgb('G'), dark: rgb('j'), dust: true },
        Body::Ash => Pal { fresh: rgb('@'), wet: rgb('7'), dry: rgb('X'), dark: rgb('k'), dust: false },
        Body::Wood => Pal { fresh: rgb('O'), wet: rgb('o'), dry: rgb('R'), dark: rgb('B'), dust: false },
        Body::Shard => {
            let c = vfx::colours_of(kind);
            Pal { fresh: c[0], wet: c[1], dry: shade(c[1], 0.7), dark: shade(c[2], 0.6), dust: false }
        }
        Body::Ichor => {
            // A slime comes apart in its own colours; the rest in a
            // sickly green.
            if matches!(kind, EnemyKind::Slime | EnemyKind::Spitter) {
                let c = vfx::colours_of(kind);
                Pal { fresh: c[0], wet: c[1], dry: shade(c[1], 0.65), dark: shade(c[2], 0.55), dust: false }
            } else {
                Pal { fresh: rgb('M'), wet: rgb('l'), dry: rgb('F'), dark: rgb('D'), dust: false }
            }
        }
    }
}

impl Pal {
    /// The colour of a drop `age` ticks after it landed.
    fn at(&self, age: f32) -> Rgb {
        if age < 8.0 {
            self.wet
        } else if age < 70.0 {
            self.dry
        } else {
            self.dark
        }
    }
}

fn h01(a: i32, b: i32, s: u32) -> f32 {
    (hash(a, b, s) % 1024) as f32 / 1024.0
}

use crate::motion::ease::{in_quad as ease_in, out_quad as ease_out};

/// How a stain thins in its last ticks: a dither threshold, 0 whole to 1
/// gone.
fn thin(age: u64, life: u64) -> f32 {
    let start = life.saturating_sub(FADE);
    if age <= start { 0.0 } else { ((age - start) as f32 / FADE as f32).min(1.0) }
}

/// A height over the floor: `vz` up, `g` pulling down, one bounce at a
/// third of the speed. `(height, rested)`.
fn arc(vz: f32, g: f32, tau: f32) -> (f32, bool) {
    if tau <= 0.0 {
        return (0.0, false);
    }
    let t1 = 2.0 * vz / g;
    if tau < t1 {
        return (vz * tau - 0.5 * g * tau * tau, false);
    }
    let v2 = vz * 0.33;
    let t2 = 2.0 * v2 / g;
    let tau2 = tau - t1;
    if tau2 < t2 {
        return (v2 * tau2 - 0.5 * g * tau2 * tau2, false);
    }
    (0.0, true)
}

/// The room-pixel point of a body's feet.
fn feet(g: &Gore) -> (f32, f32) {
    (g.x * UNIT, g.y * UNIT)
}

/// A drawn body: `pivot` (a point of the sprite, in its pixels) lands at
/// `at` in the room, the sprite turned `angle` about it, `dim` of its light,
/// only the pixels `keep` picks (in the sprite's own pixels, `(x, y)` from
/// its top-left), a cut face coloured where `face` says so, `thin` of it
/// dithered away as it fades.
struct Pose<'a> {
    at: (f32, f32),
    pivot: (f32, f32),
    angle: f32,
    dim: f32,
    keep: &'a dyn Fn(i32, i32) -> bool,
    face: &'a dyn Fn(i32, i32) -> Option<Rgb>,
    /// How squashed (1 whole), and wider by.
    squash: f32,
    wide: f32,
    thin: f32,
}

fn whole(_: i32, _: i32) -> bool {
    true
}
fn bare(_: i32, _: i32) -> Option<Rgb> {
    None
}

/// The sprite standing with its feet at `feet`, turning about its middle.
fn upright<'a>(im: &Img, feet: (f32, f32)) -> Pose<'a> {
    let (w, h) = (im.w as f32, im.h as f32);
    Pose {
        at: (feet.0, feet.1 - h / 2.0),
        pivot: (w / 2.0, h / 2.0),
        angle: 0.0,
        dim: 1.0,
        keep: &whole,
        face: &bare,
        squash: 1.0,
        wide: 1.0,
        thin: 0.0,
    }
}

/// The sprite with its feet at `feet`, turning about them (a body tipping
/// over).
fn tipping<'a>(im: &Img, feet: (f32, f32)) -> Pose<'a> {
    Pose { at: feet, pivot: (im.w as f32 / 2.0, im.h as f32 - 1.0), ..upright(im, feet) }
}

/// `upright`, lifted and turned and dimmed.
fn posed<'a>(im: &Img, feet: (f32, f32), lift: f32, angle: f32, dim: f32) -> Pose<'a> {
    let mut pose = upright(im, feet);
    pose.at.1 -= lift;
    pose.angle = angle;
    pose.dim = dim;
    pose
}

/// Draw `im` as `pose` says: each pixel of the result looks back into the
/// sprite, so a turned body has no holes.
fn blit(cv: &mut Img, im: &Img, pose: &Pose) {
    let (w, h) = (im.w as f32, im.h as f32);
    let reach = (w.hypot(h) * pose.wide.max(1.0)).ceil() as i32 + 2;
    let (s, c) = pose.angle.sin_cos();
    let (cxi, cyi) = (pose.at.0.round() as i32, pose.at.1.round() as i32);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (rx, ry) = (dx as f32, dy as f32);
            // Back into the sprite: undo the turn, then the squash.
            let (ux, uy) = (rx * c + ry * s, -rx * s + ry * c);
            let (sx, sy) = (ux / pose.wide + pose.pivot.0, uy / pose.squash + pose.pivot.1);
            let (ix, iy) = (sx.floor() as i32, sy.floor() as i32);
            if ix < 0 || iy < 0 || ix >= im.w || iy >= im.h {
                continue;
            }
            let Some(col) = im.get(ix, iy) else { continue };
            if !(pose.keep)(ix, iy) {
                continue;
            }
            let (px, py) = (cxi + dx, cyi + dy);
            if pose.thin > 0.0 && bayer(px, py) < pose.thin {
                continue;
            }
            let col = (pose.face)(ix, iy).unwrap_or_else(|| if pose.dim < 1.0 { shade(col, pose.dim) } else { col });
            cv.set(px, py, col);
        }
    }
}

/// The ground under a body: a soft dark ellipse, as the figures have.
fn shadow(cv: &mut Img, x: f32, y: f32, w: f32, thin: f32) {
    let rx = (w * 0.42).max(3.0);
    let ry = (rx * 0.38).max(1.5);
    for dy in -(ry.ceil() as i32)..=(ry.ceil() as i32) {
        for dx in -(rx.ceil() as i32)..=(rx.ceil() as i32) {
            if (dx as f32 / rx).powi(2) + (dy as f32 / ry).powi(2) > 1.0 {
                continue;
            }
            let (px, py) = (x.round() as i32 + dx, y.round() as i32 + dy);
            if bayer(px, py) < 0.45 + thin {
                continue;
            }
            cv.set(px, py, rgb('k'));
        }
    }
}

/// A spray of drops thrown from `o` (room pixels, at `height` over the
/// floor), `t` ticks ago: each its own speed, angle and lift from the seed,
/// in the air as a bright mote with a trail while fast, then lying where it
/// came down as a stain that dries. `dir` is the way it is thrown and
/// `spread` how wide, in radians.
#[allow(clippy::too_many_arguments)]
fn spray(
    pen: &mut Pen,
    seed: i32,
    salt: u32,
    o: (f32, f32),
    height: f32,
    dir: (f32, f32),
    spread: f32,
    count: i32,
    speed: (f32, f32),
    lift: (f32, f32),
    t: f32,
    pal: &Pal,
    stain_until: f32,
) {
    if t < 0.0 {
        return;
    }
    let base = dir.1.atan2(dir.0);
    // Chunky: more drops, thrown harder, than the call asks.
    let count = if pal.dust { count * 4 / 5 } else { count * 8 / 5 };
    for j in 0..count {
        let r = |s: u32| h01(j, seed, salt.wrapping_add(s));
        let a = base + (r(1) - 0.5) * spread;
        let sp = (speed.0 + (speed.1 - speed.0) * r(2)) * 1.2;
        let vz = lift.0 + (lift.1 - lift.0) * r(3);
        let delay = r(4) * 2.5;
        let tau = t - delay;
        if tau < 0.0 {
            continue;
        }
        let g = 0.5;
        // Height over the floor: thrown from `height`, it lands when it
        // comes down to the floor.
        let fall = (vz + (vz * vz + 2.0 * g * height).sqrt()) / g;
        let air = tau.min(fall);
        // Out along the ground, slowing as the air drags it.
        let run = sp * (air - 0.03 * air * air).max(0.0);
        let (gx, gy) = (o.0 + a.cos() * run, o.1 + a.sin() * run * 0.7);
        // A big drop is two pixels square, as the game's chunks are.
        let big = r(5) < 0.5;
        if tau < fall {
            if stain_until > 0.0 {
                continue;
            }
            let z = height + vz * tau - 0.5 * g * tau * tau;
            let (px, py) = (gx.round() as i32, (gy - z).round() as i32);
            pen.set(px, py, pal.fresh);
            if big {
                pen.set(px + 1, py, pal.wet);
                pen.set(px, py + 1, pal.wet);
                pen.set(px + 1, py + 1, pal.dry);
            }
            // A fast one leaves a pixel of trail.
            if sp > 1.6 && tau < 5.0 {
                let back = 1.0;
                pen.set((px as f32 - a.cos() * back).round() as i32, (py as f32 - a.sin() * back).round() as i32, pal.wet);
            }
        } else if stain_until > 0.0 {
            // Landed: a stain, a splash of pixels if it was a big drop.
            let age = tau - fall;
            let (px, py) = (gx.round() as i32, gy.round() as i32);
            let c = pal.at(age);
            pen.set(px, py, c);
            if big {
                pen.set(px + 1, py, c);
                pen.set(px, py + 1, shade(c, 0.8));
                if r(6) < 0.5 {
                    pen.set(px - 1, py, c);
                }
            }
        }
    }
}

/// A pool where it fell: a ragged blot growing for a few ticks, dithered at
/// its edge, darker at its heart.
fn pool(pen: &mut Pen, seed: i32, centre: (f32, f32), radius: f32, t: f32, pal: &Pal, thin_by: f32, flat: f32) {
    let radius = if pal.dust { radius * 0.35 } else { radius };
    if t < 0.0 || radius < 1.0 {
        return;
    }
    let grow = ease_out(t / 22.0);
    let rad = radius * 1.25 * (0.35 + 0.65 * grow);
    let (cx, cy) = (centre.0.round() as i32, centre.1.round() as i32);
    let rr = rad.ceil() as i32 + 1;
    for dy in -rr..=rr {
        for dx in -rr..=rr {
            // Ragged: the edge wobbles with the angle.
            let ang = (dy as f32).atan2(dx as f32);
            let wob = 1.0 + 0.28 * (h01((ang * 3.0).floor() as i32, seed, 11) - 0.5) * 2.0;
            let d = ((dx as f32).powi(2) + (dy as f32 / flat).powi(2)).sqrt() / (rad * wob);
            if d > 1.0 {
                continue;
            }
            let (px, py) = (cx + dx, cy + dy);
            if d > 0.78 && bayer(px, py) < 0.5 {
                continue;
            }
            if thin_by > 0.0 && bayer(px, py) < thin_by {
                continue;
            }
            let c = if d < 0.45 { pal.dark } else if t < 10.0 { pal.wet } else { pal.dry };
            pen.set(px, py, c);
        }
    }
}

/// A bone, three pixels long, lying at an angle.
fn bone(pen: &mut Pen, x: i32, y: i32, a: f32, c: Rgb) {
    let (dx, dy) = (a.cos().round() as i32, a.sin().round() as i32);
    pen.set(x, y, c);
    pen.set(x + dx, y + dy, c);
    pen.set(x - dx, y - dy, shade(c, 0.8));
}

/// Sources of the pieces a body flies apart in: sampled from its own sprite
/// at seeded spots (opaque ones, tried a few times).
fn pixel_of(im: &Img, seed: i32, j: i32) -> Rgb {
    for k in 0..8 {
        let x = (h01(j, seed, 31 + k) * im.w as f32) as i32;
        let y = (h01(j, seed, 61 + k) * im.h as f32) as i32;
        if let Some(c) = im.get(x, y) {
            return c;
        }
    }
    rgb('G')
}

/// One finisher's whole picture for the two passes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Under,
    Over,
}

/// Which way the body tumbles: with the blow, to the right or the left.
fn lean(g: &Gore) -> f32 {
    if g.dx >= 0.0 { 1.0 } else { -1.0 }
}

/// Every finisher of the run in the pass: the oldest first.
fn paint(cv: &mut Img, run: &Run, pass: Pass) {
    let mut pen = Pen { cv, left: MOTES };
    // Newest first, so a frame that runs out of motes loses the old stains.
    for g in run.gore.iter().rev() {
        if body(g.kind).is_none() {
            continue;
        }
        let age = g.age(run.tick);
        draw(&mut pen, g, age, pass, run);
    }
}

/// What lies on the floor and hangs on the wall, under the figures.
pub(super) fn under(cv: &mut Img, run: &Run) {
    if !run.gore.is_empty() {
        paint(cv, run, Pass::Under);
    }
}

/// What is in the air, over the figures.
pub(super) fn over(cv: &mut Img, run: &Run) {
    if !run.gore.is_empty() {
        paint(cv, run, Pass::Over);
    }
}

/// Whether the death at `(x, y)` is a finisher's, so the ordinary death
/// (a flash, then pieces) stands down.
pub(super) fn owns(run: &Run, x: f32, y: f32) -> bool {
    run.gore.iter().any(|g| g.cut != Cut::Pop && body(g.kind).is_some() && g.marks(x, y))
}

fn draw(pen: &mut Pen, g: &Gore, age: u64, pass: Pass, run: &Run) {
    let Some(stuff) = body(g.kind) else { return };
    let p = pal(g.kind, stuff);
    let im = sprite(g.kind, g.r);
    let seed = (g.id as i32).wrapping_mul(7919) ^ (g.born as i32);
    let t = age as f32 - HOLD as f32;
    let o = feet(g);
    let d = (g.dx, g.dy);
    let thin_stain = thin(age, LIFE);
    // A body that lies or hangs only so long.
    let body_thin = if age < BODY { 0.0 } else { thin(age, BODY + FADE) };
    let body_gone = age >= BODY + FADE;
    let torso = (o.0, o.1 - im.h as f32 * 0.5);
    // The held frame: the monster as it stood, with the blow's flash.
    if age < HOLD {
        if pass == Pass::Over {
            blit(pen.cv, &im, &upright(&im, o));
            flash(pen, g, torso, d, age);
        }
        return;
    }
    match g.cut {
        Cut::Kick => kick(pen, g, &im, &p, stuff, seed, t, age, pass, run, body_thin, body_gone, thin_stain),
        Cut::Punch => punch(pen, g, &im, &p, stuff, seed, t, age, pass, body_thin, body_gone, thin_stain),
        Cut::Behead => behead(pen, g, &im, &p, stuff, seed, t, age, pass, body_thin, body_gone, thin_stain),
        Cut::Halve if g.slant.is_some() => sever(pen, g, &im, &p, stuff, seed, t, pass, body_thin, body_gone, thin_stain),
        Cut::Halve => halve(pen, g, &im, &p, stuff, seed, t, age, pass, body_thin, body_gone, thin_stain),
        Cut::Burst => burst(pen, g, &im, &p, stuff, seed, t, age, pass, thin_stain),
        Cut::Splat => splat(pen, g, &im, &p, seed, t, age, pass, thin_stain),
        Cut::Pop => {
            if pass == Pass::Over {
                spray(pen, seed, 3, torso, 3.0, d, 2.4, 9, (0.8, 2.2), (1.0, 2.6), t, &p, 0.0);
            } else {
                spray(pen, seed, 3, torso, 3.0, d, 2.4, 9, (0.8, 2.2), (1.0, 2.6), t, &p, f32::MAX);
                pool(pen, seed, (o.0, o.1 - 1.0), g.r * UNIT * 0.7, t, &p, thin_stain, 0.5);
            }
        }
    }
}

/// The blow's flash on the held frame: a star where it struck, a ring and,
/// for a blade, the slash across: in the signal inks' cream, never white.
fn flash(pen: &mut Pen, g: &Gore, at: (f32, f32), d: (f32, f32), age: u64) {
    let k = 3 - age.min(3) as i32;
    let (cx, cy) = ((at.0 - d.0 * g.r * UNIT * 0.5).round() as i32, (at.1 - d.1 * g.r * UNIT * 0.4).round() as i32);
    pen.put(cx, cy, 'c');
    pen.put(cx + 1, cy, 'c');
    for i in 1..=k + 3 {
        for (sx, sy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            pen.put(cx + sx * i, cy + sy * i, if i <= 2 { 'c' } else if i <= 4 { '$' } else { '@' });
        }
    }
    for (sx, sy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
        for i in 1..=k + 1 {
            pen.put(cx + sx * i, cy + sy * i, if i == 1 { 'c' } else { '$' });
        }
    }
    // A ring of sparks opening out of it.
    let rr = 3.0 + (3 - k) as f32 * 3.0;
    for i in 0..10 {
        let a = i as f32 / 10.0 * std::f32::consts::TAU;
        pen.put((cx as f32 + a.cos() * rr).round() as i32, (cy as f32 + a.sin() * rr * 0.8).round() as i32, '@');
    }
    if let (Cut::Halve, Some(slant)) = (g.cut, g.slant) {
        // The blade's line itself, along the cut it leaves: long, bright
        // at its heart, and gone with the held frame.
        let a = f32::from(slant).to_radians();
        let (lx, ly) = (a.cos(), a.sin());
        let (mx, my) = (at.0, at.1 + g.r * UNIT * 0.1);
        let reach = 11 + k * 2;
        for i in -reach..=reach {
            let f = i as f32;
            let ink = if i.abs() < reach / 3 { 'w' } else if i.abs() < reach * 2 / 3 { 'c' } else { 'V' };
            pen.put((mx + lx * f).round() as i32, (my + ly * f).round() as i32, ink);
        }
    } else if matches!(g.cut, Cut::Behead | Cut::Halve) {
        // The blade's slash, across the blow.
        let (px, py) = (-d.1, d.0);
        let tilt = (f32::from(g.var) - 1.0) * 0.5;
        let (c, sn) = (tilt.cos(), tilt.sin());
        let (px, py) = (px * c - py * sn, px * sn + py * c);
        for i in -9i32..=9 {
            let f = i as f32;
            let ink = if i.abs() < 4 { 'c' } else if i.abs() < 7 { '$' } else { 'V' };
            pen.put((cx as f32 + px * f).round() as i32, (cy as f32 + py * f * 0.8).round() as i32, ink);
        }
    }
}

/// A fresh arrow, bolt or blade standing out of a body at `at`, its tail
/// toward `tail` (a unit vector), `len` pixels long.
fn shaft(pen: &mut Pen, kind: Shot, at: (f32, f32), tail: (f32, f32), len: i32) {
    let inks: &[char] = match kind {
        Shot::Bolt => &['V', 'R', 'R', 'R', 'b', 'b', 'b', 'b'],
        Shot::Blade => &['V', 'v', 'v', 'U', 'o', 'o', 'o', 'o'],
        _ => &['J', 'o', 'o', 'o', 'o', 'T', 'o', 'T'],
    };
    for k in 0..len.min(inks.len() as i32) {
        let k = k as f32;
        pen.put((at.0 + tail.0 * k).round() as i32, (at.1 + tail.1 * k).round() as i32, inks[k as usize]);
    }
}

/// The knight's boot swinging through to a monster's ribs at `at`, along
/// `d`, `t` ticks after it landed: the boot itself, and the arc it swept.
fn boot(pen: &mut Pen, at: (f32, f32), d: (f32, f32), t: f32) {
    let base = d.1.atan2(d.0);
    // The arc: a crescent behind the foot, thinning.
    for i in -6i32..=6 {
        let a = base + i as f32 * 0.16;
        let r = 8.0 - t * 1.2;
        let (x, y) = (at.0 - d.0 * 6.0 + a.cos() * r, at.1 - d.1 * 6.0 * 0.5 + a.sin() * r * 0.8);
        let ink = if i.abs() > 4 { 'U' } else if t < 2.0 { 'c' } else { 'V' };
        if t < 3.0 || i % 2 == 0 {
            pen.put(x.round() as i32, y.round() as i32, ink);
        }
    }
    // The boot: a dark toe-cap and heel, a lighter sole, driving in.
    let reach = 5.0 - t * 1.4;
    let (bx, by) = (at.0 - d.0 * reach, at.1 - d.1 * reach * 0.5);
    for (dx, dy, ink) in [(0, 0, 'h'), (1, 0, 'h'), (0, 1, 'G'), (1, 1, 'J'), (-1, 0, 'J'), (-1, 1, 'G'), (2, 0, 'i'), (2, 1, 'J')] {
        pen.put(bx.round() as i32 + dx, by.round() as i32 + dy, ink);
    }
}

/// The streak of the killshot, from the knight to where it strikes.
fn streak(pen: &mut Pen, from: (f32, f32), to: (f32, f32), head: f32, kind: Shot) {
    let len = (to.0 - from.0).hypot(to.1 - from.1).max(1.0);
    let (ux, uy) = ((to.0 - from.0) / len, (to.1 - from.1) / len);
    let tip = head * len;
    for k in 0..9 {
        let back = k as f32 * 1.0;
        let along = tip - back;
        if along < 0.0 || along > len {
            continue;
        }
        let ink = match (k, kind) {
            (0, _) => 'c',
            (1..=2, _) => '$',
            (_, Shot::Bolt) => 'V',
            _ => 'U',
        };
        pen.put((from.0 + ux * along).round() as i32, (from.1 + uy * along).round() as i32, ink);
    }
}

/// Stone chips and sparks where a pinned body hit the wall.
fn thunk(pen: &mut Pen, at: (f32, f32), d: (f32, f32), seed: i32, t: f32) {
    if !(0.0..7.0).contains(&t) {
        return;
    }
    for j in 0..7 {
        let a = d.1.atan2(d.0) + std::f32::consts::PI + (h01(j, seed, 201) - 0.5) * 2.6;
        let sp = 1.2 + 2.0 * h01(j, seed, 202);
        let run = sp * t;
        let (x, y) = (at.0 + a.cos() * run, at.1 + a.sin() * run + 0.12 * t * t * 1.5);
        let ink = ['h', 'J', 'c', 'G', 'O'][(j % 5) as usize];
        pen.put(x.round() as i32, y.round() as i32, ink);
    }
    if t < 2.0 {
        for (sx, sy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            pen.put(at.0.round() as i32 + sx, at.1.round() as i32 + sy, 'c');
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn kick(
    pen: &mut Pen,
    g: &Gore,
    im: &Img,
    p: &Pal,
    stuff: Body,
    seed: i32,
    t: f32,
    age: u64,
    pass: Pass,
    _run: &Run,
    body_thin: f32,
    body_gone: bool,
    thin_stain: f32,
) {
    let o = feet(g);
    let to = (g.to_x * UNIT, g.to_y * UNIT);
    let d = (g.dx, g.dy);
    let sgn = lean(g);
    let span = f32::from(g.span.max(1));
    let u = (t / span).clamp(0.0, 1.0);
    let s = ease_out(u);
    let travel = (to.0 - o.0).hypot(to.1 - o.1);
    let h = im.h as f32;
    let flying = t < span;
    // The knight's own feet.
    let from = (g.from_x * UNIT, g.from_y * UNIT - 6.0);
    if flying {
        if pass == Pass::Under {
            // What it spilled on the way, along the floor under its flight.
            let n = 10;
            for j in 0..n {
                let f = (j as f32 + 0.5) / n as f32;
                if f > s {
                    break;
                }
                let (x, y) = (o.0 + (to.0 - o.0) * f, o.1 + (to.1 - o.1) * f + (h01(j, seed, 321) - 0.5) * 4.0 - 1.0);
                pen.set(x.round() as i32, y.round() as i32, if t < 8.0 { p.wet } else { p.dry });
                if j % 2 == 0 {
                    pen.set(x.round() as i32 + 1, y.round() as i32, p.dry);
                }
            }
            spray(pen, seed, 43, (o.0, o.1 - h * 0.4), 3.0, d, 2.0, 7, (1.0, 2.6), (0.8, 2.2), t, p, f32::MAX);
            return;
        }
        let at = (o.0 + (to.0 - o.0) * s, o.1 + (to.1 - o.1) * s);
        let apex = (3.0 + travel * 0.14) * (1.0 + 0.3 * f32::from(g.var));
        let lift = (PI * u).sin() * apex;
        let angle = match g.pin {
            Pin::Wall => sgn * (PI * u).sin() * (0.9 + 0.3 * f32::from(g.var)),
            _ => sgn * s * (FRAC_PI_2 + 0.35 * f32::from(g.var)),
        };
        // Its shadow stays on the ground it flies over.
        shadow(pen.cv, at.0, at.1, im.w as f32 * 0.8, 0.0);
        // Afterimages along the flight: it is going fast.
        for (gi, (back, thin_by, dim)) in [(3.4f32, 0.72f32, 0.4f32), (1.7, 0.5, 0.6)].into_iter().enumerate() {
            let tt = t - back;
            if tt <= 0.0 {
                continue;
            }
            let (gu, gs) = ((tt / span).clamp(0.0, 1.0), ease_out((tt / span).clamp(0.0, 1.0)));
            let gat = (o.0 + (to.0 - o.0) * gs, o.1 + (to.1 - o.1) * gs);
            let glift = (PI * gu).sin() * apex;
            let gangle = match g.pin {
                Pin::Wall => sgn * (PI * gu).sin() * (0.9 + 0.3 * f32::from(g.var)),
                _ => sgn * gs * (FRAC_PI_2 + 0.35 * f32::from(g.var)),
            };
            let _ = gi;
            blit(pen.cv, im, &Pose { thin: thin_by, ..posed(im, gat, glift, gangle, dim) });
        }
        blit(pen.cv, im, &posed(im, at, lift, angle, 0.92));
        // The boot: a swing through, dust and a star where it struck.
        if t < 4.0 {
            boot(pen, (o.0 - d.0 * g.r * UNIT * 0.9, o.1 - h * 0.35), d, t);
        }
        if t < 3.0 {
            let c = (o.0 - d.0 * g.r * UNIT * 0.9, o.1 - h * 0.35 - d.1 * g.r * UNIT * 0.4);
            for j in 0..5 {
                let a = d.1.atan2(d.0) + PI + (h01(j, seed, 301) - 0.5) * 2.2;
                let r = 1.5 + t * 1.6 + h01(j, seed, 302) * 2.0;
                pen.put((c.0 + a.cos() * r).round() as i32, (c.1 + a.sin() * r * 0.8).round() as i32, ['G', 'j', 'J'][(j % 3) as usize]);
            }
        }
        // The killshot: it takes the body in the air, a third of the way.
        let hit_at = 0.34;
        let chest = (at.0, at.1 - lift - h * if g.var == 2 { 0.7 } else { 0.45 });
        if u < hit_at + 0.06 && u > 0.0 {
            let head = ((u / hit_at) * 1.0).min(1.0);
            streak(pen, from, chest, head, g.shaft);
        }
        if u >= hit_at {
            // The shaft rides in the body, tail toward the knight.
            shaft(pen, g.shaft, chest, (-d.0, -d.1), 7);
            // What the shot opened flies along behind it.
            let since = (u - hit_at) * span;
            spray(pen, seed, 41, chest, 2.0, (-d.0, -d.1), 1.8, 10, (0.7, 2.0), (0.4, 1.6), since, p, 0.0);
        }
        // The wound's first burst, at the kick.
        spray(pen, seed, 43, (o.0, o.1 - h * 0.4), 3.0, d, 2.0, 7, (1.0, 2.6), (0.8, 2.2), t, p, 0.0);
        return;
    }
    // Landed.
    let land = t - span;
    let chest_h = h * if g.var == 2 { 0.7 } else { 0.45 };
    match g.pin {
        Pin::Wall => {
            if pass == Pass::Under {
                // Stains thrown at the stone, running down it as it hangs.
                let wallp = (to.0 + d.0 * g.r * UNIT * 0.55, to.1 + d.1 * g.r * UNIT * 0.55);
                spray(pen, seed, 47, (to.0, to.1 - chest_h), 4.0, (-d.0, -d.1), 3.6, 14, (0.6, 2.4), (0.5, 2.2), land + 6.0, p, f32::MAX);
                // A splat on the stone around the shaft.
                for j in 0..12 {
                    let a = h01(j, seed, 311) * 2.0 * PI;
                    let r = 2.0 + 5.0 * h01(j, seed, 312);
                    let (x, y) = (wallp.0 + a.cos() * r, wallp.1 - chest_h + a.sin() * r * 0.8);
                    if thin_stain == 0.0 || bayer(x as i32, y as i32) >= thin_stain {
                        pen.set(x.round() as i32, y.round() as i32, if land < 6.0 { p.wet } else { p.dry });
                    }
                }
                // Drips run down the stone and gather at its foot.
                let run = (land * 0.5).min(h * 0.55);
                for j in 0..3 {
                    let x = (to.0 + (j as f32 - 1.0) * 3.0).round() as i32;
                    for k in 0..(run as i32 - j) {
                        pen.set(x, (to.1 - chest_h + 2.0 + k as f32).round() as i32, p.dry);
                    }
                }
                pool(pen, seed, (to.0, to.1 + 1.0), g.r * UNIT * 0.9, land + 4.0, p, thin_stain.max(body_thin * 0.0), 0.4);
            }
            if pass == Pass::Under && !body_gone {
                // It hangs: lifted off the floor, slumping a little more as
                // the seconds go.
                let slump = (land / 60.0).min(1.0);
                let tilt = sgn * (0.06 + 0.12 * slump);
                shadow(pen.cv, to.0, to.1 + 1.0, im.w as f32 * 0.7, 0.2);
                blit(pen.cv, im, &Pose { thin: body_thin, ..posed(im, to, 3.0 - slump * 2.0, tilt, 0.8) });
                if body_thin == 0.0 || bayer(to.0 as i32, to.1 as i32) >= body_thin {
                    shaft(pen, g.shaft, (to.0, to.1 - chest_h + 2.0 - 2.0 * slump), (-d.0, -d.1), 8);
                }
            }
            if pass == Pass::Over {
                spray(pen, seed, 47, (to.0, to.1 - chest_h), 4.0, (-d.0, -d.1), 3.6, 14, (0.6, 2.4), (0.5, 2.2), land + 6.0, p, 0.0);
                thunk(pen, (to.0 + d.0 * g.r * UNIT * 0.6, to.1 - chest_h), d, seed, land);
                if matches!(stuff, Body::Bone) {
                    scatter_bones(pen, g, seed, land, (to.0, to.1 - chest_h), p, false);
                }
            }
        }
        _ => {
            if pass == Pass::Over {
                spray(pen, seed, 53, (to.0, to.1 - 3.0), 5.0, d, 3.0, 10, (0.5, 2.0), (0.5, 2.0), land + 6.0, p, 0.0);
                return;
            }
            // Pinned to the floor: lying, the shaft standing up out of it.
            let lie = (to.0, to.1);
            pool(pen, seed, (lie.0, lie.1 + 1.0), g.r * UNIT * 0.95, land + 3.0, p, thin_stain, 0.5);
            spray(pen, seed, 53, (lie.0, lie.1 - 3.0), 5.0, d, 3.0, 10, (0.5, 2.0), (0.5, 2.0), land + 6.0, p, f32::MAX);
            if !body_gone {
                shadow(pen.cv, lie.0, lie.1 + 1.0, im.w as f32 * 0.9, 0.2);
                let angle = sgn * (FRAC_PI_2 + 0.35 * f32::from(g.var));
                blit(pen.cv, im, &Pose { thin: body_thin, ..posed(im, (lie.0, lie.1 + 2.0), 0.0, angle, 0.8) });
                if body_thin == 0.0 || bayer(lie.0 as i32, lie.1 as i32) >= body_thin {
                    // Standing in it, upright, with a flicker as it settles.
                    let wob = if land < 4.0 { ((land * 3.0).sin() * 1.0).round() } else { 0.0 };
                    shaft(pen, g.shaft, (lie.0 + wob, lie.1 - 1.0), (0.0, -1.0), 8);
                }
            }
        }
    }
    let _ = (stuff, age);
}

#[allow(clippy::too_many_arguments)]
fn punch(
    pen: &mut Pen,
    g: &Gore,
    im: &Img,
    p: &Pal,
    stuff: Body,
    seed: i32,
    t: f32,
    _age: u64,
    pass: Pass,
    body_thin: f32,
    body_gone: bool,
    thin_stain: f32,
) {
    let o = feet(g);
    let to = (g.to_x * UNIT, g.to_y * UNIT);
    let d = (g.dx, g.dy);
    let sgn = lean(g);
    let span = f32::from(g.span.max(1));
    let u = (t / span).clamp(0.0, 1.0);
    let s = ease_out(u);
    let h = im.h as f32;
    let at = (o.0 + (to.0 - o.0) * s, o.1 + (to.1 - o.1) * s);
    // The turn: upright to lying as it skids; a variant hops.
    let hop = if g.var == 2 { (PI * u).sin() * 4.0 } else { (PI * u).sin() * 1.5 };
    let angle = sgn * ease_in(u * 1.1).min(1.0) * (FRAC_PI_2 + 0.15 * f32::from(g.var));
    let flying = t < span;
    // The shot's spray, thrown on along its line.
    let wound = (o.0 - d.0 * g.r * UNIT * 0.4, o.1 - h * 0.45);
    if pass == Pass::Under {
        // The skid's smear along the floor.
        let n = (travel_of(o, at) * 0.8) as i32;
        for k in 0..n {
            let f = k as f32 / n.max(1) as f32;
            let x = o.0 + (at.0 - o.0) * f;
            let y = o.1 + (at.1 - o.1) * f - 1.0;
            if thin_stain == 0.0 || bayer(x as i32, y as i32) >= thin_stain {
                pen.set(x.round() as i32, y.round() as i32, if t < 10.0 { p.wet } else { p.dry });
                if k % 3 == 0 {
                    pen.set(x.round() as i32, y.round() as i32 + 1, p.dark);
                }
            }
        }
        pool(pen, seed, (at.0, at.1 - 1.0), g.r * UNIT * 0.65, t - span * 0.6, p, thin_stain, 0.5);
    }
    let out = if matches!(stuff, Body::Bone) { 0.0 } else { 1.0 };
    // The shot's spray, thrown on along its line, and what it took out of
    // the far side.
    let exit = (o.0 + d.0 * g.r * UNIT * 0.5, o.1 - h * 0.45);
    let stain = if pass == Pass::Over { 0.0 } else { f32::MAX };
    spray(pen, seed, 61, wound, 3.0, d, 1.5, (9.0 + 4.0 * out) as i32, (2.2, 4.6), (0.4, 2.2), t, p, stain);
    spray(pen, seed, 67, exit, 3.0, d, 1.0, 8, (2.6, 5.0), (0.2, 1.6), t - 1.0, p, stain);
    if matches!(stuff, Body::Bone) && pass == Pass::Over {
        scatter_bones(pen, g, seed, t, at, p, false);
    }
    if flying && pass == Pass::Over {
        // Afterimages: it is going fast.
        for (back, thin_by, dim) in [(3.0f32, 0.72f32, 0.4f32), (1.5, 0.5, 0.6)] {
            let tt = t - back;
            if tt <= 0.0 {
                continue;
            }
            let gu = (tt / span).clamp(0.0, 1.0);
            let gs = ease_out(gu);
            let gat = (o.0 + (to.0 - o.0) * gs, o.1 + (to.1 - o.1) * gs);
            let gangle = sgn * ease_in(gu * 1.1).min(1.0) * (FRAC_PI_2 + 0.15 * f32::from(g.var));
            blit(pen.cv, im, &Pose { thin: thin_by, ..posed(im, gat, 0.0, gangle, dim) });
        }
    }
    if (flying && pass == Pass::Over) || (!flying && pass == Pass::Under && !body_gone) {
        if !flying {
            shadow(pen.cv, at.0, at.1 + 1.0, im.w as f32 * 0.9, 0.2);
        }
        let dim = if flying { 0.95 } else { 0.8 };
        let feet = (at.0, at.1 + if flying { 0.0 } else { 2.0 });
        blit(pen.cv, im, &Pose { thin: body_thin, ..posed(im, feet, if flying { hop } else { 0.0 }, angle, dim) });
    }
}

fn travel_of(a: (f32, f32), b: (f32, f32)) -> f32 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

/// Bones thrown out of a skeleton and settling: a skull and a few long
/// bones and ribs, with a puff of dust where it came apart.
fn scatter_bones(pen: &mut Pen, g: &Gore, seed: i32, t: f32, at: (f32, f32), p: &Pal, lying: bool) {
    let n = 9;
    for j in 0..n {
        let r = |s: u32| h01(j, seed, 401 + s);
        let a = r(1) * 2.0 * PI;
        let sp = 0.9 + 2.2 * r(2);
        let vz = 1.8 + 2.4 * r(3);
        let tau = t - r(4) * 2.0;
        if tau < 0.0 {
            continue;
        }
        let (z, rest) = arc(vz, 0.5, tau);
        let run = sp * tau.min(2.0 * vz / 0.5 * 1.25);
        let (x, y) = (at.0 + a.cos() * run, at.1 + a.sin() * run * 0.6 - z - 4.0 * if rest { 0.0 } else { 0.0 });
        let c = if rest || lying { shade(p.fresh, 0.8) } else { p.fresh };
        bone(pen, x.round() as i32, y.round() as i32, a + tau * 0.4 * if rest { 0.0 } else { 1.0 }, c);
    }
    // The dust.
    if t < 9.0 {
        for j in 0..12 {
            let a = h01(j, seed, 451) * 2.0 * PI;
            let r = 1.0 + t * 0.7 + 3.0 * h01(j, seed, 452);
            let x = at.0 + a.cos() * r;
            let y = at.1 + a.sin() * r * 0.6 - t * 0.2;
            let ink = if t < 4.0 { 'J' } else if t < 7.0 { 'G' } else { 'j' };
            if bayer(x as i32, y as i32) > t / 12.0 {
                pen.put(x.round() as i32, y.round() as i32, ink);
            }
        }
    }
    let _ = g;
}

#[allow(clippy::too_many_arguments)]
fn behead(
    pen: &mut Pen,
    g: &Gore,
    im: &Img,
    p: &Pal,
    stuff: Body,
    seed: i32,
    t: f32,
    _age: u64,
    pass: Pass,
    body_thin: f32,
    body_gone: bool,
    thin_stain: f32,
) {
    let o = feet(g);
    let to = (g.to_x * UNIT, g.to_y * UNIT);
    let d = (g.dx, g.dy);
    let sgn = lean(g);
    let span = f32::from(g.span.max(1));
    let h = im.h as f32;
    let (bx0, by0, bx1, by1) = head_box(g.kind, im.w, im.h);
    let inside = move |x: i32, y: i32| x >= bx0 && x < bx1 && y >= by0 && y < by1;
    let head_keep = inside;
    let trunk_keep = move |x: i32, y: i32| !inside(x, y);
    let fresh = p.fresh;
    // The trunk's cut: the pixels touching where the head was.
    let neck_face = move |x: i32, y: i32| {
        let near = x >= bx0 - 1 && x <= bx1 && y >= by0 - 1 && y <= by1;
        (near && !inside(x, y)).then_some(fresh)
    };
    let cut = by1;
    let head_mid = ((bx0 + bx1) as f32 / 2.0, (by0 + by1) as f32 / 2.0);
    let neck_top = (o.0 + (head_mid.0 - im.w as f32 / 2.0), o.1 - h + by1 as f32);
    // The trunk stands a moment, wobbling, then falls along the blow.
    let fall_start = if g.var == 2 { 9.0 } else { 6.0 };
    let fall_u = ((t - fall_start) / (span - fall_start).max(1.0)).clamp(0.0, 1.0);
    let fallen_to = if g.var == 2 { -sgn } else { sgn } * (FRAC_PI_2 + 0.12);
    let trunk_angle = if t < fall_start { (t * 2.4).sin() * 0.05 } else { ease_in(fall_u) * fallen_to };
    let down = fall_u >= 1.0;
    // The head: out along the blow in an arc, a bounce, then at rest.
    let hu = (t / span).clamp(0.0, 1.0);
    let hs = ease_out(hu * 1.1);
    // The head pops high off the neck; a finisher's flies the highest.
    let apex = 10.0 + 4.0 * f32::from(g.var) + if g.span >= 18 { 4.0 } else { 0.0 };
    let hz = if hu < 1.0 { 4.0 * apex * hu * (1.0 - hu) } else { 0.0 };
    let start = (o.0 + (head_mid.0 - im.w as f32 / 2.0), o.1 - h + head_mid.1);
    let end = (to.0, to.1 - 3.0);
    let centre = (start.0 + (end.0 - start.0) * hs, start.1 + (end.1 - start.1) * hs - hz);
    let spin = sgn * hs * (2.4 + f32::from(g.var) * 0.8) * PI;
    let head_rest = t >= span;
    let head_pose = |at: (f32, f32), angle: f32, dim: f32, thin: f32| Pose {
        at,
        pivot: head_mid,
        angle,
        dim,
        keep: &head_keep,
        thin,
        wide: 1.3,
        squash: 1.3,
        ..upright(im, o)
    };
    if pass == Pass::Over {
        // The neck's pulses, up and a little along the blow.
        for pulse in 0..4 {
            let since = t - pulse as f32 * 5.0;
            if (0.0..14.0).contains(&since) {
                spray(pen, seed, 71 + pulse as u32 * 7, neck_top, 0.0, (d.0 * 0.3, -1.0), 0.9, 7, (0.8, 1.9), (2.4, 4.0), since, p, 0.0);
            }
        }
        if t < 5.0 {
            spray(pen, seed, 83, neck_top, 0.0, (d.0, -0.5), 1.6, 8, (1.0, 2.6), (1.4, 3.2), t, p, 0.0);
        }
        if !head_rest {
            blit(pen.cv, im, &head_pose(centre, spin, 0.95, 0.0));
            // Drops flung off it as it spins.
            spray(pen, seed, 91, centre, 0.0, (d.0, 0.0), 2.6, 4, (0.4, 1.4), (0.3, 1.2), t, p, 0.0);
        }
        if !down && !body_gone {
            blit(pen.cv, im, &Pose { keep: &trunk_keep, face: &neck_face, ..posed_tip(im, o, trunk_angle, 0.92) });
        }
        if matches!(stuff, Body::Bone) && t > fall_start {
            scatter_bones(pen, g, seed, t - fall_start, (o.0, o.1 - 4.0), p, false);
        }
    } else {
        // Stains from the spray; the pool under the neck grows.
        for pulse in 0..4 {
            let since = t - pulse as f32 * 5.0;
            if since >= 0.0 {
                spray(pen, seed, 71 + pulse as u32 * 7, neck_top, 0.0, (d.0 * 0.3, -1.0), 0.9, 7, (0.8, 1.9), (2.4, 4.0), since, p, f32::MAX);
            }
        }
        spray(pen, seed, 83, neck_top, 0.0, (d.0, -0.5), 1.6, 8, (1.0, 2.6), (1.4, 3.2), t, p, f32::MAX);
        pool(pen, seed, (o.0 + d.0 * 2.0, o.1), g.r * UNIT * 0.85, t - 4.0, p, thin_stain, 0.5);
        if head_rest {
            pool(pen, seed, end, g.r * UNIT * 0.35, t - span, p, thin_stain, 0.5);
            if !body_gone {
                shadow(pen.cv, end.0, end.1 + 2.0, 9.0, 0.2);
                blit(pen.cv, im, &head_pose(end, spin + 0.3 * sgn, 0.8, body_thin));
            }
        }
        if down && !body_gone {
            shadow(pen.cv, o.0, o.1 + 1.0, im.w as f32 * 0.9, 0.2);
            blit(
                pen.cv,
                im,
                &Pose { keep: &trunk_keep, face: &neck_face, thin: body_thin, ..posed_tip(im, (o.0, o.1 + 2.0), trunk_angle, 0.8) },
            );
        }
        if matches!(stuff, Body::Bone) && down {
            scatter_bones(pen, g, seed, t - fall_start, (o.0, o.1 - 4.0), p, true);
        }
    }
}

/// The box of a monster's head in its sprite, `(x0, y0, x1, y1)`, the far
/// corner not in it: read off the art (`print_sprites`). A body not listed
/// has its head in its top two fifths.
fn head_box(kind: EnemyKind, w: i32, h: i32) -> (i32, i32, i32, i32) {
    use EnemyKind as K;
    match kind {
        K::Skeleton | K::Necromancer | K::Imp => (0, 0, w, 5),
        K::Hob => (0, 0, 9, 6),
        K::Hexer => (0, 0, 8, 6),
        K::Goblin => (7, 0, 16, 7),
        _ => (0, 0, w, ((h as f32 * 0.4).round() as i32).max(3)),
    }
}

/// `tipping`, turned and dimmed.
fn posed_tip<'a>(im: &Img, feet: (f32, f32), angle: f32, dim: f32) -> Pose<'a> {
    Pose { angle, dim, ..tipping(im, feet) }
}

#[allow(clippy::too_many_arguments)]
fn halve(
    pen: &mut Pen,
    g: &Gore,
    im: &Img,
    p: &Pal,
    stuff: Body,
    seed: i32,
    t: f32,
    _age: u64,
    pass: Pass,
    body_thin: f32,
    body_gone: bool,
    thin_stain: f32,
) {
    let o = feet(g);
    let d = (g.dx, g.dy);
    let sgn = lean(g);
    let span = f32::from(g.span.max(1));
    let (w, h) = (im.w as f32, im.h as f32);
    let u = (t / span).clamp(0.0, 1.0);
    let s = ease_out(u);
    // The cut runs across the blow, tilted by the variant; the halves fall
    // apart along the blow.
    let tilt = (f32::from(g.var) - 1.0) * 0.6;
    let (c, sn) = (tilt.cos(), tilt.sin());
    let (nx, ny) = (d.0 * c - d.1 * sn, d.0 * sn + d.1 * c);
    // Seen from above, a blow straight up or down parts the halves left and
    // right.
    let (nx, ny) = if nx.abs() < 0.3 { (sgn * 0.9, ny * 0.5) } else { (nx, ny * 0.7) };
    let nl = nx.hypot(ny).max(0.01);
    let (nx, ny) = (nx / nl, ny / nl);
    let mid = (w / 2.0, h / 2.0);
    let far = move |x: i32, y: i32| (x as f32 + 0.5 - mid.0) * nx + (y as f32 + 0.5 - mid.1) * ny >= 0.0;
    let near = move |x: i32, y: i32| !far(x, y);
    let fresh = p.fresh;
    let cut_face = move |x: i32, y: i32| -> Option<Rgb> {
        let off = (x as f32 + 0.5 - mid.0) * nx + (y as f32 + 0.5 - mid.1) * ny;
        (off.abs() < 1.1).then_some(fresh)
    };
    let gap = (3.0 + g.r * UNIT * 0.55) * s;
    let sink = (s * 3.0).min(3.0);
    let pieces = [
        (true, sgn * s * 0.55, (nx * gap, ny * gap * 0.6 + sink * 0.3)),
        (false, -sgn * s * 0.75, (-nx * gap * 0.7, -ny * gap * 0.4 + sink)),
    ];
    let down = t >= span;
    let lift = if down { 0.0 } else { (PI * u).sin() * 3.0 };
    let half = |far_side: bool, angle: f32, shift: (f32, f32), dim: f32, thin: f32, rest: f32| {
        let mut pose = posed(im, (o.0 + shift.0, o.1 + shift.1 + rest), lift, angle, dim);
        pose.keep = if far_side { &far } else { &near };
        pose.face = &cut_face;
        pose.thin = thin;
        pose
    };
    if pass == Pass::Over && !down {
        // The spray along the cut, both ways, and a gout from the middle.
        for (k, dir) in [(0u32, (nx, ny)), (1u32, (-nx, -ny))] {
            spray(pen, seed, 111 + k * 9, (o.0, o.1 - h * 0.5), 2.0, dir, 1.7, 8, (1.0, 2.8), (0.8, 2.8), t, p, 0.0);
        }
        spray(pen, seed, 131, (o.0, o.1 - h * 0.5), 2.0, (0.0, -1.0), 1.2, 6, (0.4, 1.4), (2.0, 3.8), t, p, 0.0);
        for (far_side, angle, shift) in pieces {
            blit(pen.cv, im, &half(far_side, angle, shift, 0.94, 0.0, 0.0));
        }
        if matches!(stuff, Body::Bone) {
            scatter_bones(pen, g, seed, t, (o.0, o.1 - h * 0.4), p, false);
        }
    }
    if pass == Pass::Under {
        spray(pen, seed, 111, (o.0, o.1 - h * 0.5), 2.0, (nx, ny), 1.7, 8, (1.0, 2.8), (0.8, 2.8), t, p, f32::MAX);
        spray(pen, seed, 120, (o.0, o.1 - h * 0.5), 2.0, (-nx, -ny), 1.7, 8, (1.0, 2.8), (0.8, 2.8), t, p, f32::MAX);
        pool(pen, seed, (o.0, o.1), g.r * UNIT * 1.0, t - 3.0, p, thin_stain, 0.5);
        if down && !body_gone {
            shadow(pen.cv, o.0, o.1 + 1.0, w * 1.2, 0.2);
            for (far_side, angle, shift) in pieces {
                blit(pen.cv, im, &half(far_side, angle, shift, 0.8, body_thin, 2.0));
            }
        }
        if matches!(stuff, Body::Bone) && down {
            scatter_bones(pen, g, seed, t - 6.0, (o.0, o.1 - h * 0.4), p, true);
        }
    }
}

/// A body a knight's blade went through on a slope (`Gore::slant`): the
/// cut shows, the upper piece slides off down it and tumbles to the floor
/// beside, and the lower stands a moment, gouting, before it folds the other
/// way. Both lie in a spreading pool until they fade.
#[allow(clippy::too_many_arguments)]
fn sever(
    pen: &mut Pen,
    g: &Gore,
    im: &Img,
    p: &Pal,
    stuff: Body,
    seed: i32,
    t: f32,
    pass: Pass,
    body_thin: f32,
    body_gone: bool,
    thin_stain: f32,
) {
    let o = feet(g);
    let (w, h) = (im.w as f32, im.h as f32);
    let slant = f32::from(g.slant.unwrap_or(0)).to_radians();
    // Down the cut, and the side of it that is up: the upper piece's.
    let (lx, ly) = if slant.sin() < 0.0 { (-slant.cos(), -slant.sin()) } else { (slant.cos(), slant.sin()) };
    let (nx, ny) = if lx >= 0.0 { (ly, -lx) } else { (-ly, lx) };
    let fall = if lx >= 0.0 { 1.0 } else { -1.0 };
    // A level cut goes through the waist; a steep one down the middle.
    let mid = (w / 2.0, h * if slant.abs() > 0.8 { 0.5 } else { 0.48 });
    let off = move |x: i32, y: i32| (x as f32 + 0.5 - mid.0) * nx + (y as f32 + 0.5 - mid.1) * ny;
    let upper = move |x: i32, y: i32| off(x, y) > 0.0;
    let lower = move |x: i32, y: i32| off(x, y) <= 0.0;
    let fresh = p.fresh;
    let wet = p.wet;
    let face = move |x: i32, y: i32| -> Option<Rgb> {
        let d = off(x, y).abs();
        if d < 0.8 { Some(fresh) } else if d < 1.6 { Some(wet) } else { None }
    };
    // Where the upper piece's middle sits in the sprite: on the cut, a
    // third of the way into its side.
    let depth = if slant.abs() > 0.8 { w * 0.28 } else { h * 0.26 };
    let centre = (mid.0 + nx * depth, mid.1 + ny * depth);
    let span = f32::from(g.span.max(8));
    // The slide: a pixel or two while the cut shows, then away down it,
    // turning over, to the floor beside the legs.
    let start = (o.0 + centre.0 - w / 2.0, o.1 - h + centre.1);
    let rest = (o.0 + lx * (w * 0.55 + 4.0) + fall * 2.0, o.1 - 2.5);
    let shown = 4.0;
    // The blade throws the piece up off the cut a pixel as it slides, so a
    // line of dark shows between the two at once.
    let (at, angle, down) = if t < shown {
        let k = ease_out(t / shown) * 3.0;
        ((start.0 + lx * k + nx, start.1 + ly * k + ny), 0.0, false)
    } else {
        let u = ((t - shown) / (span - shown)).clamp(0.0, 1.0);
        let slide = 3.0 + 6.0 * ease_in((u * 2.0).min(1.0));
        let along = (start.0 + lx * slide + nx, start.1 + ly * slide + ny);
        let k = ease_in(u);
        let at = (along.0 + (rest.0 - along.0) * k, along.1 + (rest.1 - along.1) * k * k);
        (at, fall * (FRAC_PI_2 + 0.15) * k, u >= 1.0)
    };
    // A small bounce where it lands.
    let after = t - span;
    let bounce = if (0.0..5.0).contains(&after) { (PI * after / 5.0).sin() * 1.6 } else { 0.0 };
    let piece = |at: (f32, f32), angle: f32, dim: f32, thin: f32| Pose {
        at,
        pivot: centre,
        angle,
        dim,
        keep: &upper,
        face: &face,
        thin,
        ..upright(im, o)
    };
    // The legs stand while it slides, sag, then fold back the other way.
    let fold_at = shown + 6.0;
    let fold_u = ((t - fold_at) / 9.0).clamp(0.0, 1.0);
    let legs_angle = if t < fold_at { (t * 2.0).sin() * 0.04 } else { -fall * ease_in(fold_u) * (FRAC_PI_2 + 0.1) };
    let legs_down = fold_u >= 1.0;
    let sag = if t < fold_at { 0.0 } else { (fold_u * 2.0).min(1.0) * 1.5 };
    let legs = |angle: f32, dim: f32, thin: f32, drop: f32| Pose {
        keep: &lower,
        face: &face,
        thin,
        ..posed_tip(im, (o.0, o.1 + drop), angle, dim)
    };
    // Where the cut on the standing legs is, in the room.
    let wound = (o.0 + mid.0 - w / 2.0, o.1 - h + mid.1 + sag);
    if pass == Pass::Over {
        // The cut opening: a spray both ways along it, then a gout up out
        // of the legs in pulses while they stand.
        if t < 6.0 {
            for (k, dir) in [(0u32, (lx, ly)), (1u32, (-lx, -ly))] {
                spray(pen, seed, 141 + k * 9, wound, 1.0, dir, 1.2, 7, (1.0, 2.6), (0.8, 2.4), t, p, 0.0);
            }
        }
        for pulse in 0..3 {
            let since = t - shown - pulse as f32 * 4.0;
            if (0.0..12.0).contains(&since) && !legs_down {
                spray(pen, seed, 161 + pulse as u32 * 7, wound, 0.0, (nx * 0.4 + lx * 0.2, -1.0), 0.8, 6, (0.6, 1.6), (2.6, 4.2), since, p, 0.0);
            }
        }
        if !legs_down && !body_gone {
            blit(pen.cv, im, &legs(legs_angle, 0.92, 0.0, 0.0));
        }
        if !down && !body_gone {
            blit(pen.cv, im, &piece(at, angle, 0.95, 0.0));
            // Drops shaken off the piece as it goes.
            if t >= shown {
                spray(pen, seed, 181, at, 0.0, (lx, ly), 1.4, 3, (0.3, 1.0), (0.2, 1.0), t - shown, p, 0.0);
            }
        }
        if matches!(stuff, Body::Bone) && t > shown {
            scatter_bones(pen, g, seed, t - shown, (wound.0, wound.1 + 2.0), p, false);
        }
    } else {
        for (k, dir) in [(0u32, (lx, ly)), (1u32, (-lx, -ly))] {
            spray(pen, seed, 141 + k * 9, wound, 1.0, dir, 1.2, 7, (1.0, 2.6), (0.8, 2.4), t, p, f32::MAX);
        }
        for pulse in 0..3 {
            let since = t - shown - pulse as f32 * 4.0;
            if since >= 0.0 {
                spray(pen, seed, 161 + pulse as u32 * 7, wound, 0.0, (nx * 0.4 + lx * 0.2, -1.0), 0.8, 6, (0.6, 1.6), (2.6, 4.2), since, p, f32::MAX);
            }
        }
        pool(pen, seed, (o.0 - fall * 1.5, o.1), g.r * UNIT * 0.9, t - 4.0, p, thin_stain, 0.5);
        if down {
            pool(pen, seed ^ 0x55, rest, g.r * UNIT * 0.55, after, p, thin_stain, 0.5);
            if !body_gone {
                shadow(pen.cv, rest.0, rest.1 + 2.0, w * 0.7, 0.2);
                blit(pen.cv, im, &piece((rest.0, rest.1 - bounce), fall * (FRAC_PI_2 + 0.15), 0.8, body_thin));
            }
        }
        if legs_down && !body_gone {
            shadow(pen.cv, o.0, o.1 + 1.0, w * 0.9, 0.2);
            blit(pen.cv, im, &legs(legs_angle, 0.8, body_thin, 1.0));
        }
        if matches!(stuff, Body::Bone) && legs_down {
            scatter_bones(pen, g, seed, t - shown, (wound.0, wound.1 + 2.0), p, true);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn burst(
    pen: &mut Pen,
    g: &Gore,
    im: &Img,
    p: &Pal,
    stuff: Body,
    seed: i32,
    t: f32,
    _age: u64,
    pass: Pass,
    thin_stain: f32,
) {
    let o = feet(g);
    let d = (g.dx, g.dy);
    let h = im.h as f32;
    let c = (o.0, o.1 - h * 0.45);
    let r = g.r * UNIT;
    // The mist: a ring of dark opening out of it, a few ticks.
    if pass == Pass::Over && t < 8.0 {
        let rr = r * 0.6 + t * 2.2;
        let n = (rr * 2.5) as i32;
        for i in 0..n {
            let a = i as f32 / n as f32 * 2.0 * PI;
            let (x, y) = (c.0 + a.cos() * rr, c.1 + a.sin() * rr * 0.75);
            if bayer(x as i32, y as i32) > t / 9.0 {
                let ink = match stuff {
                    Body::Ash => ['6', '@', '7', 'X'][(t as usize / 2).min(3)],
                    Body::Bone => ['J', 'G', 'j', 'g'][(t as usize / 2).min(3)],
                    Body::Shard => ['V', 'v', 'U', 'u'][(t as usize / 2).min(3)],
                    Body::Wood => ['O', 'o', 'R', 'B'][(t as usize / 2).min(3)],
                    _ => ['7', '8', '8', 'Z'][(t as usize / 2).min(3)],
                };
                pen.put(x.round() as i32, y.round() as i32, ink);
            }
        }
    }
    // The chunks: its own pixels, thrown out, bounced once, at rest.
    let chunks = 9 + (r / 3.0) as i32;
    for j in 0..chunks {
        let rj = |s: u32| h01(j, seed, 501 + s);
        // Mostly the way the blow went.
        let cone = if g.var == 1 { 1.5 } else { 3.4 };
        let a = d.1.atan2(d.0) + (rj(1) - 0.5) * cone * if j % 3 == 0 { 1.9 } else { 1.0 };
        let sp = (1.4 + 3.0 * rj(2)) * if g.var == 2 { 0.7 } else { 1.0 };
        let vz = (2.0 + 3.6 * rj(3)) * if g.var == 2 { 1.7 } else { 1.0 };
        let tau = t - rj(4) * 1.5;
        if tau < 0.0 {
            continue;
        }
        let (z, rest) = arc(vz, 0.55, tau);
        let move_for = tau.min(2.0 * vz / 0.55 * 1.2);
        let ground = (c.0 + a.cos() * sp * move_for, o.1 - 2.0 + a.sin() * sp * move_for * 0.55);
        let lift = z + if rest { 0.0 } else { h * 0.35 * (1.0 - (tau / 4.0).min(1.0)) };
        let (px, py) = (ground.0.round() as i32, (ground.1 - lift).round() as i32);
        let col = pixel_of(im, seed, j);
        let col = if matches!(stuff, Body::Ash) { shade(col, 0.45) } else if rest { shade(col, 0.75) } else { col };
        if rest {
            if pass == Pass::Under && (thin_stain == 0.0 || bayer(px, py) >= thin_stain) {
                pen.set(px, py, col);
                if j % 3 == 0 {
                    pen.set(px + 1, py, col);
                    pen.set(px, py + 1, shade(col, 0.6));
                }
                if j % 4 == 0 {
                    pen.set(px - 1, py, p.dry);
                }
            }
        } else if pass == Pass::Over {
            pen.set(px, py, col);
            if j % 3 == 0 {
                pen.set(px + 1, py, col);
                pen.set(px, py + 1, shade(col, 0.6));
                pen.set(px + 1, py + 1, shade(col, 0.6));
            }
        }
    }
    let wet_spray = !matches!(stuff, Body::Bone | Body::Shard);
    if wet_spray {
        let stain = if pass == Pass::Under { f32::MAX } else { 0.0 };
        spray(pen, seed, 141, c, 3.0, d, 6.28, 16, (0.8, 3.0), (0.8, 3.4), t, p, stain);
    }
    if pass == Pass::Under {
        if matches!(stuff, Body::Bone) {
            scatter_bones(pen, g, seed, t, c, p, true);
        }
        let pool_k = if matches!(stuff, Body::Shard | Body::Bone) { 0.0 } else { 1.0 };
        pool(pen, seed, (o.0, o.1 - 1.0), r * 1.05 * pool_k, t - 2.0, p, thin_stain, 0.5);
        // Scorched ground where something burned.
        if matches!(stuff, Body::Ash) {
            pool(pen, seed, (o.0, o.1 - 1.0), r * 0.9, t - 1.0, p, thin_stain, 0.5);
        }
    } else if matches!(stuff, Body::Bone) {
        scatter_bones(pen, g, seed, t, c, p, false);
    }
}

#[allow(clippy::too_many_arguments)]
fn splat(pen: &mut Pen, g: &Gore, im: &Img, p: &Pal, seed: i32, t: f32, _age: u64, pass: Pass, thin_stain: f32) {
    let o = feet(g);
    let d = (g.dx, g.dy);
    let h = im.h as f32;
    let r = g.r * UNIT;
    let u = (t / 4.0).clamp(0.0, 1.0);
    // It squashes flat and wide, then it is a puddle.
    if pass == Pass::Over && t < 5.0 {
        let mut pose = tipping(im, o);
        pose.squash = 1.0 - 0.7 * ease_out(u);
        pose.wide = 1.0 + 0.55 * ease_out(u);
        pose.dim = 0.95;
        pose.thin = if t > 3.0 { 0.5 } else { 0.0 };
        blit(pen.cv, im, &pose);
    }
    let stain = if pass == Pass::Under { f32::MAX } else { 0.0 };
    // A ring of drops thrown out low and fast, and a few high.
    spray(pen, seed, 151, (o.0, o.1 - h * 0.3), 2.0, d, 6.28, if g.var == 2 { 20 } else { 14 }, (1.0, 3.2), (0.4, 2.8), t, p, stain);
    if pass == Pass::Under {
        let flat = if g.var == 1 { 0.32 } else { 0.55 };
        let big = if g.var == 1 { 1.6 } else { 1.25 };
        pool(pen, seed, (o.0, o.1), r * big, t - 1.0, p, thin_stain, flat);
        // Bubbles and a lit rim on the fresh puddle.
        if t < 90.0 {
            for j in 0..5 {
                let a = h01(j, seed, 161) * 2.0 * PI;
                let rr = r * (0.2 + 0.7 * h01(j, seed, 162));
                let (x, y) = (o.0 + a.cos() * rr, o.1 + a.sin() * rr * 0.5);
                if (t as i32 + j * 7) % 18 < 9 && (thin_stain == 0.0 || bayer(x as i32, y as i32) >= thin_stain) {
                    pen.set(x.round() as i32, y.round() as i32, p.fresh);
                }
            }
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn a_ballistic_arc_lands_and_rests() {
        let (z, rest) = arc(3.0, 0.5, 0.5);
        assert!(z > 0.0 && !rest);
        let (z, rest) = arc(3.0, 0.5, 40.0);
        assert_eq!((z, rest), (0.0, true));
    }

    #[test]
    fn a_stain_thins_only_at_the_end_of_its_life() {
        assert_eq!(thin(0, LIFE), 0.0);
        assert_eq!(thin(LIFE - FADE, LIFE), 0.0);
        assert!(thin(LIFE - 1, LIFE) > 0.9);
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__gore__tests.rs"]
mod tests;
