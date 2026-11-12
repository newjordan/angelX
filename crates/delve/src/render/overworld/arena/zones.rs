//! Area spells, drawn: the waves, bursts, cones, lines, fields, pillars and
//! novas a wish casts (`rules::zones`).
//!
//! Every shape is drawn along a guide: a wave's front is an arc swept about
//! the knight's aim, a nova's a circle, a cone's two edges, a line's the
//! crack walked segment by segment, a burst's and a field's a ring with
//! spokes, a pillar's a ring closing on its mark, lightning a jagged run
//! between two foes. The pieces are placed on the guide (ice spikes up
//! through the floor as the front passes them, flame tongues, stone, shafts
//! of light, a storm's bolts) and the guide itself can be drawn in its own
//! layer (`guides`) to check the paths before judging the picture.
//!
//! Frost's spikes and shards are the pixel-effects library's crystal and
//! shatter (a leaning faceted spike, white at its tip and lit down its
//! ridge, white for the first frames it breaks the floor), recut in the
//! realm's inks, ordered dither and no glow; fire's tongues, the storm's
//! forked bolts, the holy column and the poison's bubbling cloud are its
//! flame, lightning, pillar and cloud likewise.
//!
//! A pure function of the run's zones: the same pixels on the host, a
//! mirror and a browser guest.

use super::super::ink::{Img, bayer, hash};
use super::pfx::{self, Mote, Pen, Ramp, Shape, Spray, ramps, unit};
use super::{UNIT, works};
use crate::rules::cards::{Element, Form};
use crate::rules::zones::{self, ARC_TICKS, BURST_WAIT, LINE_STEP, LINE_WAIT, PILLAR_WAIT, SHARD_TICKS, Zone};
use crate::rules::{Enemy, Run};
use std::f32::consts::{PI, TAU};

/// The most pixels the passes may write a frame, under and over.
const UNDER_BUDGET: u32 = 9000;
const OVER_BUDGET: u32 = 14000;

thread_local! {
    static GUIDES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Draw the guides every zone is drawn along, in a layer of their own, for
/// the shots that check the paths. Off unless asked.
pub fn guides(on: bool) {
    GUIDES.with(|g| g.set(on));
}

#[cfg(test)]
thread_local! {
    /// For the bench only: the parts of the passes left out (1 cloud, 2 arcs, 4 the pool's life, 8 blows).
    pub(super) static SKIP: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn skip(part: u32) -> bool {
    #[cfg(test)]
    {
        SKIP.with(std::cell::Cell::get) & part != 0
    }
    #[cfg(not(test))]
    {
        let _ = part;
        false
    }
}

fn guides_on() -> bool {
    GUIDES.with(std::cell::Cell::get)
}

/// A zone as the picture sees it.
struct V<'a> {
    z: &'a Zone,
    ramp: Ramp,
    age: f32,
    tick: u64,
    seed: u32,
    /// Where it stands, in pixels, and its aim.
    o: (f32, f32),
    d: (f32, f32),
    /// The picture its wish painted for it (`draw shot`), stamped along the
    /// guide where the element's own pieces would stand.
    sprite: Option<Img>,
}

/// Flame: cream-white heart, yellow, amber, then the reds the edge and the
/// tip go out in.
const FLAME: Ramp = Ramp(['9', '6', '@', '7', 'p', '8']);

fn ramp_of(element: Element) -> Ramp {
    match element {
        Element::Fire => FLAME,
        Element::Frost => ramps::FROST,
        Element::Storm => ramps::STORM,
        Element::Poison => ramps::POISON,
        Element::Holy => ramps::HOLY,
        Element::Arcane => ramps::ARCANE,
        Element::Earth => ramps::EARTH,
    }
}

fn view<'a>(run: &Run, z: &'a Zone) -> V<'a> {
    let seed = z.spell.bytes().fold(z.owner.wrapping_mul(0x9e37_79b9), |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let sprite = run
        .book
        .get(&z.spell)
        .and_then(|card| card.looks.shot.as_ref())
        .map(|sprite| super::knights::sprite_img(sprite, run.tick))
        .filter(|im| im.w > 0 && im.h > 0);
    V { z, ramp: ramp_of(z.area.element), age: z.age as f32, tick: run.tick, seed, o: (z.x * UNIT, z.y * UNIT), d: (z.dx, z.dy), sprite }
}

impl V<'_> {
    fn reach(&self) -> f32 {
        self.z.reach() * UNIT
    }
    fn size(&self) -> f32 {
        1.0 + 0.22 * (f32::from(self.z.area.size) - 1.0)
    }
    fn angle(&self) -> f32 {
        self.d.1.atan2(self.d.0)
    }
    /// How much of the zone's end has come: 0 until its last `n` ticks.
    fn sinking(&self, n: f32) -> f32 {
        ((self.age - (self.z.life as f32 - n)) / n).clamp(0.0, 1.0)
    }
    fn u(&self, i: u32, salt: u32) -> f32 {
        unit(i as i32, self.seed as i32, salt)
    }
}

/// A glow as the pen's own, without its power function: a bright core, a
/// halo ramping down the inks, its edge dithered away to bare paper.
fn soft(pen: &mut Pen, (cx, cy): (f32, f32), r: f32, squash: f32, ramp: Ramp, power: f32) {
    let power = if pen.calm { power * 0.55 } else { power };
    if power <= 0.02 || r < 0.5 {
        return;
    }
    let ry = (r * squash).max(0.5);
    let (ir, iry) = (1.0 / r, 1.0 / ry);
    for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
        let dy = (y as f32 + 0.5 - cy) * iry;
        for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
            let dx = (x as f32 + 0.5 - cx) * ir;
            let d2 = dx * dx + dy * dy;
            if d2 >= 1.0 {
                continue;
            }
            let e = 1.0 - d2.sqrt();
            let i = power * e * e * (0.55 + 0.45 * e);
            if i < 0.07 || (i < 0.32 && bayer(x, y) > (i - 0.07) / 0.25) {
                continue;
            }
            if i < 0.8 && pen.has(x, y) {
                continue;
            }
            pen.light(x, y, ramp.dithered(1.0 - i.min(1.0), x, y));
        }
    }
}

fn out(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    1.0 - (1.0 - k) * (1.0 - k)
}

/// Overshoots a little and settles: a spike breaking the floor.
fn pop(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    let c = 2.2;
    1.0 + (c + 1.0) * (k - 1.0).powi(3) + c * (k - 1.0).powi(2)
}

// ─── the passes ──────────────────────────────────────────────────────────────

/// Under the figures: the marks left on the floor, the pools, cracks, frost
/// and scorch, and what each zone's telegraph draws on the ground.
pub(super) fn under(cv: &mut Img, run: &Run) {
    if run.zones.is_empty() {
        return;
    }
    let mut pen = Pen::new(cv, UNDER_BUDGET);
    for z in &run.zones.zones {
        let v = view(run, z);
        floor(&mut pen, &v);
    }
    for s in &run.zones.shards {
        let k = f32::from(s.age) / f32::from(SHARD_TICKS);
        let c = (s.x * UNIT, s.y * UNIT);
        decal(&mut pen, Element::Frost, c, 15.0, (1.0 - k).powi(2) * 0.9, u32::from(s.seed), 0);
    }
}

/// Over the figures: the zones' bodies, the frozen in their ice, the arcs,
/// the shatters and the blows landing.
pub(super) fn over(cv: &mut Img, run: &Run) {
    if run.zones.is_empty() {
        return;
    }
    let mut pen = Pen::new(cv, OVER_BUDGET);
    for z in &run.zones.zones {
        let v = view(run, z);
        body(&mut pen, run, &v);
    }
    for r in &run.zones.rime {
        if let Some(e) = run.enemies.iter().find(|e| e.id == r.foe && e.hp > 0) {
            casing(&mut pen, run, e);
        }
    }
    for a in run.zones.arcs.iter().rev().take(14) {
        if !skip(2) {
            arc(&mut pen, a);
        }
    }
    for s in &run.zones.shards {
        shatter(&mut pen, run, s);
    }
    for z in &run.zones.zones {
        if !skip(8) {
            blows(&mut pen, run, &view(run, z));
        }
    }
    if guides_on() {
        for z in &run.zones.zones {
            guide_layer(&mut pen, &view(run, z));
        }
        for s in &run.zones.shards {
            let at = (s.x * UNIT, s.y * UNIT - 8.0);
            let seed = u32::from(s.seed).wrapping_mul(0x85eb);
            let n = 7 + seed % 4;
            for i in 0..n {
                for step in 0..16 {
                    let (p, _) = shard_guide(seed, i, n, step as f32 * 1.5, at);
                    pen.put(p.0.round() as i32, p.1.round() as i32, '+');
                }
            }
        }
    }
}

// ─── the floor ───────────────────────────────────────────────────────────────

fn floor(pen: &mut Pen, v: &V) {
    let z = v.z;
    let reach = v.reach();
    let age = v.age;
    match z.area.form {
        Form::Burst => {
            if age < BURST_WAIT as f32 {
                telegraph(pen, v, v.o, reach, age / BURST_WAIT as f32);
            } else {
                let grow = out((age - BURST_WAIT as f32) / 5.0);
                let fade = 1.0 - v.sinking(26.0);
                decal(pen, z.area.element, v.o, reach * 0.95 * grow, fade, v.seed, v.tick as u32);
            }
        }
        Form::Field => {
            let grow = out(age / 8.0);
            let fade = 1.0 - v.sinking(26.0);
            decal(pen, z.area.element, v.o, reach * grow, fade, v.seed, v.tick as u32);
        }
        Form::Pillar => {
            if age < PILLAR_WAIT as f32 {
                telegraph(pen, v, v.o, reach, age / PILLAR_WAIT as f32);
            } else {
                let k = (age - PILLAR_WAIT as f32) / 16.0;
                decal(pen, z.area.element, v.o, reach * 1.1, (1.0 - k).clamp(0.0, 1.0), v.seed, 0);
            }
        }
        Form::Line => crack(pen, v),
        Form::Wave | Form::Nova => {
            // What the front left behind it: the element's mark across the
            // swept ground, fading.
            let (ticks, half) = if z.area.form == Form::Wave { (20, 0.95) } else { (16, PI) };
            let r = zones::front(z.age, ticks, z.reach()) * UNIT;
            let fade = (1.0 - v.sinking(14.0)) * 0.7;
            sector_decal(pen, v, r, half, fade);
        }
        Form::Cone => {}
    }
}

/// The element's mark on a disc of floor: scorch and embers, frost, charge,
/// a puddle, a rune circle, a vortex, cracks. `fade` 0 to 1; `phase` slow
/// ticks for what flickers.
fn decal(pen: &mut Pen, element: Element, c: (f32, f32), r: f32, fade: f32, seed: u32, phase: u32) {
    if r < 2.0 || fade <= 0.02 {
        return;
    }
    if element == Element::Fire {
        lava_pool(pen, c, r, fade, seed, phase);
        return;
    }
    let ramp = ramp_of(element);
    let (cx, cy) = c;
    let r2 = r + 2.0;
    // In blocks of two by two, a crust and a scorch being clumps and not grit.
    let (x0, y0) = (((cx - r2).floor() as i32) & !1, ((cy - r2).floor() as i32) & !1);
    for y in (y0..=(cy + r2).ceil() as i32).step_by(2) {
        for x in (x0..=(cx + r2).ceil() as i32).step_by(2) {
            // The pixels it draws are chosen first (no element draws more
            // than 85 in the hundred of a pixel), and its distance only for
            // those.
            let n = unit(x, y, seed ^ 0x51);
            if n > fade * 0.85 {
                continue;
            }
            let (dx, dy) = (x as f32 + 1.0 - cx, y as f32 + 1.0 - cy);
            let d2 = (dx * dx + dy * dy) / (r * r);
            if d2 > 1.45 {
                continue;
            }
            let d = d2.sqrt();
            // A ragged edge, not a clean disc.
            let edge = 1.0 + (unit(x / 3, y / 3, seed) - 0.5) * 0.35;
            if d > edge {
                continue;
            }
            let body = fade * (1.0 - d * d * 0.7);
            match element {
                Element::Fire => {
                    // A scorch, and cinders glowing in it.
                    if n < body * 0.55 {
                        block(pen, x, y, if unit(x, y, seed ^ phase.wrapping_mul(7)) < 0.07 * body { ramp.step(2) } else { 'k' });
                    } else if d > 0.8 && n < body * 0.7 {
                        block(pen, x, y, ramp.step(3));
                    }
                }
                Element::Frost => {
                    if n < body * 0.8 {
                        let k = if d > 0.85 { 0.15 } else { 0.38 + d * 0.3 };
                        block(pen, x, y, if n < 0.05 * body { 'w' } else { ramp.dithered(k, x, y) });
                    }
                }
                Element::Storm => {
                    if n < body * 0.02 {
                        block(pen, x, y, ramp.step(4));
                    }
                }
                Element::Poison => {
                    if n < body * 0.7 {
                        block(pen, x, y, ramp.dithered(0.45 + d * 0.4 + (unit(x / 2, y / 2, seed ^ (phase / 6)) - 0.5) * 0.2, x, y));
                    }
                }
                Element::Holy => {
                    if n < body * 0.22 {
                        block(pen, x, y, ramp.dithered(0.2 + d * 0.4, x, y));
                    }
                }
                Element::Arcane => {
                    // A swirl: spiral arms turning, a void in the middle.
                    let a = dy.atan2(dx) + d * 5.0 - (phase / 6) as f32 * 0.4;
                    let arm = (a * 1.5).sin();
                    if d < 0.2 {
                        if n < body {
                            block(pen, x, y, 'K');
                        }
                    } else if arm > 0.55 && n < body * 0.75 {
                        block(pen, x, y, ramp.dithered(0.3 + d * 0.6, x, y));
                    }
                }
                Element::Earth => {
                    if n < body * 0.28 {
                        block(pen, x, y, if n < 0.05 * body { ramp.step(1) } else { ramp.step(4) });
                    }
                }
            }
        }
    }
    // The ring that bounds it, drawn in the element's own light.
    let k = 1.0 - fade;
    if matches!(element, Element::Holy | Element::Arcane) {
        pen.ring(c, r * 0.98, 0.9, 1.0, ramp, k);
    }
    if element == Element::Holy || element == Element::Arcane {
        let rot = (phase / 6) as f32 * 0.35;
        pen.glyph(c, r * 0.8, 1.0, rot, 1.0, if element == Element::Holy { 6 } else { 5 }, ramp.cooler(1), k, seed);
    }
}

/// A block of two by two pixels of `ch`, its top left at `(x, y)`.
fn block(pen: &mut Pen, x: i32, y: i32, ch: char) {
    pen.put(x, y, ch);
    pen.put(x + 1, y, ch);
    pen.put(x, y + 1, ch);
    pen.put(x + 1, y + 1, ch);
}

/// The box a sector of radius `r`, aimed `a0` and `half` wide to each side,
/// fits in: its tip, its two ends and any quarter-turn its arc passes.
fn sector_box(c: (f32, f32), a0: f32, half: f32, r: f32) -> (i32, i32, i32, i32) {
    let (mut x0, mut y0, mut x1, mut y1) = (c.0, c.1, c.0, c.1);
    let mut add = |a: f32| {
        let (px, py) = (c.0 + a.cos() * r, c.1 + a.sin() * r);
        (x0, x1, y0, y1) = (x0.min(px), x1.max(px), y0.min(py), y1.max(py));
    };
    add(a0 - half);
    add(a0 + half);
    for k in 0..4 {
        let a = k as f32 * std::f32::consts::FRAC_PI_2;
        let da = (a - a0 + 3.0 * PI).rem_euclid(TAU) - PI;
        if da.abs() <= half {
            add(a);
        }
    }
    (x0.floor() as i32 - 1, y0.floor() as i32 - 1, x1.ceil() as i32 + 1, y1.ceil() as i32 + 1)
}

/// A lava pool: a dark crust, ragged at its edge, cracked by seams that
/// glow and pulse as heat runs out along them from the middle. The seams are
/// walked on a guide: spokes out of the centre, each turning a little at every
/// step and throwing a short branch now and then.
fn lava_pool(pen: &mut Pen, c: (f32, f32), r: f32, fade: f32, seed: u32, tick: u32) {
    let ramp = FLAME;
    let (cx, cy) = c;
    let r2 = r + 2.0;
    let (x0, y0) = (((cx - r2).floor() as i32) & !1, ((cy - r2).floor() as i32) & !1);
    for y in (y0..=(cy + r2).ceil() as i32).step_by(2) {
        for x in (x0..=(cx + r2).ceil() as i32).step_by(2) {
            let n = unit(x, y, seed ^ 0x51);
            if n > fade * 0.92 {
                continue;
            }
            let (dx, dy) = (x as f32 + 1.0 - cx, y as f32 + 1.0 - cy);
            let d2 = (dx * dx + dy * dy) / (r * r);
            if d2 > 1.45 {
                continue;
            }
            let d = d2.sqrt();
            let edge = 1.0 + (unit(x / 3, y / 3, seed) - 0.5) * 0.35;
            if d > edge {
                continue;
            }
            // Crust: black stone, a brown fleck, the rim scorched red.
            let ch = if d > 0.84 { ramp.step(4) } else if n < 0.1 { ramp.step(5) } else if n < 0.28 { 'K' } else { 'k' };
            block(pen, x, y, ch);
        }
    }
    let spokes = 7;
    let heat = tick as f32;
    for a in 0..spokes {
        let mut ang = a as f32 / spokes as f32 * TAU + (unit(a, seed as i32, 3) - 0.5) * 0.7;
        let mut p = c;
        for s in 0..((r / 3.0) as i32).max(2) {
            ang += (unit(a * 31 + s, seed as i32, 7) - 0.5) * 0.9;
            let q = (p.0 + ang.cos() * 3.0, p.1 + ang.sin() * 3.0);
            if (q.0 - cx).hypot(q.1 - cy) > r * 0.9 {
                break;
            }
            // Heat running out along the seam, each seam on its own beat.
            let beat = 0.5 + 0.5 * (heat * 0.17 - s as f32 * 0.6 + a as f32 * 1.7).sin();
            let k = 0.08 + (1.0 - beat) * 0.55 + s as f32 * 0.025;
            let steps = 4;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let (px, py) = ((p.0 + (q.0 - p.0) * t).round() as i32, (p.1 + (q.1 - p.1) * t).round() as i32);
                if bayer(px, py) < (1.0 - fade) {
                    continue;
                }
                pen.put(px, py, ramp.dithered(k, px, py));
                if beat > 0.5 {
                    // A hot seam is two pixels wide and bleeds red either side.
                    pen.put(px + 1, py, ramp.dithered(k + 0.2, px + 1, py));
                    pen.light(px, py + 1, ramp.step(3));
                    pen.light(px - 1, py, ramp.step(4));
                }
            }
            // A short branch off every third step.
            if s % 3 == 1 && unit(a * 17 + s, seed as i32, 9) < 0.7 {
                let side = if unit(a * 13 + s, seed as i32, 4) < 0.5 { 1.0 } else { -1.0 };
                let b = (q.0 + (ang + side * 1.1).cos() * 3.5, q.1 + (ang + side * 1.1).sin() * 3.5);
                if (b.0 - cx).hypot(b.1 - cy) < r * 0.88 {
                    for i in 0..=3 {
                        let t = i as f32 / 3.0;
                        let (px, py) = ((q.0 + (b.0 - q.0) * t).round() as i32, (q.1 + (b.1 - q.1) * t).round() as i32);
                        pen.light(px, py, ramp.dithered(0.35 + (1.0 - beat) * 0.4 + t * 0.15, px, py));
                    }
                }
            }
            p = q;
        }
    }
    // The heart of it: one slow molten eye.
    let eye = 0.5 + 0.5 * (heat * 0.11).sin();
    soft(pen, c, r * 0.22, 1.0, ramp, 0.5 * fade * (0.5 + 0.5 * eye));
}

/// A sector of the floor marked as the front passed over it.
fn sector_decal(pen: &mut Pen, v: &V, r: f32, half: f32, fade: f32) {
    if r < 3.0 || fade <= 0.02 {
        return;
    }
    let a0 = v.angle();
    let ramp = v.ramp;
    let (cx, cy) = v.o;
    let element = v.z.area.element;
    let (ux, uy) = v.d;
    let cos_half = half.cos();
    let (x0, y0, x1, y1) = sector_box(v.o, a0, half, r);
    for y in (y0 & !1..=y1).step_by(2) {
        for x in (x0 & !1..=x1).step_by(2) {
            // The random pick first: most pixels end here.
            let n = unit(x, y, v.seed ^ 0x77);
            if n >= fade * 0.45 {
                continue;
            }
            let (dx, dy) = (x as f32 + 1.0 - cx, y as f32 + 1.0 - cy);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r || d2 < 36.0 {
                continue;
            }
            let d = d2.sqrt();
            let cos_a = (dx * ux + dy * uy) / d;
            if cos_a < cos_half {
                continue;
            }
            let da = cos_a.clamp(-1.0, 1.0).acos();
            let q = d / r;
            // Thickest just behind the front, thinning toward the knight.
            let body = fade * (0.25 + 0.75 * q) * (1.0 - da / half.max(0.1) * 0.5);
            if n >= body * 0.45 {
                continue;
            }
            let ch = match element {
                Element::Fire => {
                    if n < body * 0.08 { ramp.step(2) } else { 'k' }
                }
                Element::Frost => {
                    if n < body * 0.04 { 'w' } else { ramp.dithered(0.35 + (1.0 - q) * 0.3, x, y) }
                }
                Element::Storm => {
                    if n < body * 0.07 { ramp.step(0) } else { continue }
                }
                Element::Poison => ramp.dithered(0.5 + (1.0 - q) * 0.3, x, y),
                Element::Holy => ramp.dithered(0.25 + (1.0 - q) * 0.4, x, y),
                Element::Arcane => ramp.dithered(0.5 + (1.0 - q) * 0.4, x, y),
                Element::Earth => ramp.step(4),
            };
            block(pen, x, y, ch);
        }
    }
}

/// A burst's or pillar's warning: a ring closing on its mark with spokes,
/// a rune opening in it, the colour building at its heart.
fn telegraph(pen: &mut Pen, v: &V, c: (f32, f32), r: f32, k: f32) {
    let ramp = v.ramp;
    let close = 1.0 + (1.0 - out(k)) * 0.7;
    pen.ring(c, r * close, 0.9, 1.0, ramp, 0.55 - 0.4 * k);
    pen.glyph(c, r * 0.85, 1.0, v.age * 0.12, out(k * 1.3), 5, ramp, 0.25 - 0.2 * k, v.seed);
    // The radial guide: spokes from the middle out to the ring.
    for s in 0..6 {
        let a = v.age * 0.05 + s as f32 / 6.0 * TAU;
        let (ux, uy) = (a.cos(), a.sin());
        let (from, to) = (r * 0.2, r * close * 0.95);
        pen.line((c.0 + ux * from, c.1 + uy * from), (c.0 + ux * to * k.max(0.25), c.1 + uy * to * k.max(0.25)), ramp.step(2));
    }
    soft(pen, c, r * 0.5 * k.max(0.2), 1.0, ramp, 0.25 + 0.5 * k);
}

// ─── the bodies ──────────────────────────────────────────────────────────────

fn body(pen: &mut Pen, run: &Run, v: &V) {
    match v.z.area.form {
        Form::Wave => wave(pen, v),
        Form::Nova => nova(pen, v),
        Form::Burst => burst(pen, v),
        Form::Cone => cone(pen, v),
        Form::Line => line(pen, v),
        Form::Field => field(pen, run, v),
        Form::Pillar => pillar(pen, v),
    }
}

/// One thing that comes up through the floor on a guide: where, how big,
/// the zone age it broke the floor at.
struct Site {
    at: (f32, f32),
    h: f32,
    t0: f32,
    i: u32,
}

/// Sites placed on the guide of a front: rings at fractions of its reach,
/// each ring's pieces spread across the sweep, each rising as the front
/// passes it.
fn front_sites(v: &V, half: f32, ticks: f32, rings: &[(f32, u32)]) -> Vec<Site> {
    let a0 = v.angle();
    let reach = v.reach();
    let mut sites = Vec::new();
    let mut i = 0;
    for &(frac, count) in rings {
        for j in 0..count {
            let jitter = (v.u(i, 3) - 0.5) * 0.7;
            let ang = if half >= PI - 0.01 {
                a0 + (j as f32 + 0.5 + jitter) / count as f32 * TAU
            } else {
                a0 + ((j as f32 + 0.5 + jitter) / count as f32 - 0.5) * 2.0 * half * 0.94
            };
            let r = reach * (frac + (v.u(i, 4) - 0.5) * 0.05).clamp(0.05, 1.0);
            // The tick the front's run has got as far as `r`.
            let k = 1.0 - (1.0 - (r / reach).min(0.999)).sqrt();
            let h = (15.0 - 7.0 * frac) * (0.8 + 0.4 * v.u(i, 5)) * v.size();
            sites.push(Site { at: (v.o.0 + ang.cos() * r, v.o.1 + ang.sin() * r), h, t0: k * ticks + v.u(i, 6) * 1.2, i });
            i += 1;
        }
    }
    sites.sort_by(|a, b| a.at.1.total_cmp(&b.at.1));
    sites
}

fn wave(pen: &mut Pen, v: &V) {
    let (ticks, half) = (20.0, 0.95);
    let reach = v.reach();
    let r = zones::front(v.z.age, 20, v.z.reach()) * UNIT;
    let sink = v.sinking(10.0);
    let sites = front_sites(v, half, ticks, &[(0.14, 2), (0.28, 3), (0.44, 4), (0.6, 4), (0.76, 5), (0.92, 5)]);
    for s in &sites {
        let since = v.age - s.t0;
        if since >= 0.0 {
            erupt(pen, v, s.at, s.h, since, sink, s.i);
        }
    }
    // The front itself: an arc swept about the aim, thick and hot at its
    // leading edge, trailing away behind it.
    if v.age <= ticks + 6.0 {
        let fade = ((v.age - ticks) / 6.0).clamp(0.0, 1.0);
        front_arc(pen, v, r, half, fade, reach);
    }
}

/// The leading edge of a front along its guide: points on the arc, each
/// trailing a streak back toward the caster, brightest where the sweep is
/// middle and thinning to its ends.
fn front_arc(pen: &mut Pen, v: &V, r: f32, half: f32, fade: f32, reach: f32) {
    if r < 4.0 {
        return;
    }
    let a0 = v.angle();
    let full = half >= PI - 0.01;
    let n = ((2.0 * half * r) / 1.4).ceil().clamp(8.0, 220.0) as i32;
    let ramp = v.ramp;
    let trail = 7.0 + 8.0 * (1.0 - r / reach).max(0.0);
    // The leading edge itself, a hard bright line along the guide.
    for s in 0..=n {
        let q = s as f32 / n as f32;
        let ang = if full { a0 + q * TAU } else { a0 - half + 2.0 * half * q };
        let e = if full { 1.0 } else { (1.0 - (2.0 * q - 1.0).abs().powi(3)).max(0.0) };
        if e > 0.12 && fade < 0.7 {
            let (px, py) = ((v.o.0 + ang.cos() * (r + 1.0)).round() as i32, (v.o.1 + ang.sin() * (r + 1.0)).round() as i32);
            pen.light(px, py, ramp.step(0));
            if e > 0.5 {
                pen.light(px - ang.cos().round() as i32, py - ang.sin().round() as i32, ramp.step(1));
            }
        }
    }
    for s in 0..=n {
        let q = s as f32 / n as f32;
        let ang = if full { a0 + q * TAU } else { a0 - half + 2.0 * half * q };
        let e = if full { 1.0 } else { (1.0 - (2.0 * q - 1.0).abs().powi(3)).max(0.0) };
        let (ux, uy) = (ang.cos(), ang.sin());
        // The wobble that keeps it a living edge and not a drawn one.
        let wob = (unit(s, v.seed as i32, 0x31) - 0.5) * 2.2;
        let rr = r + wob;
        let run_len = trail * e * (1.0 - fade);
        let steps = run_len.ceil().max(1.0) as i32;
        for t in 0..=steps {
            let tt = t as f32 / steps as f32;
            let (px, py) = ((v.o.0 + ux * (rr - t as f32)).round() as i32, (v.o.1 + uy * (rr - t as f32)).round() as i32);
            if t > 1 && bayer(px, py) < tt * 0.75 {
                continue;
            }
            pen.light(px, py, ramp.dithered(0.05 + tt * 0.85 + fade * 0.3, px, py));
        }
        if s % 5 == 0 && e > 0.3 && fade < 0.6 {
            let m = Mote {
                i: s as u32,
                at: (v.o.0 + ux * (rr + 1.0), v.o.1 + uy * (rr + 1.0) - (unit(s, v.tick as i32 / 2, v.seed ^ 9) * 4.0)),
                vel: (ux * 0.6, uy * 0.6 - 0.8),
                k: fade,
                seed: hash(s, v.seed as i32, 0x5a),
            };
            pfx::mote(pen, &m, mote_of(v.z.area.element), ramp, 1, v.tick);
        }
    }
}

fn mote_of(element: Element) -> Shape {
    match element {
        Element::Fire => Shape::Ember,
        Element::Frost => Shape::Flake,
        Element::Storm => Shape::Spark,
        Element::Poison => Shape::Bubble,
        Element::Holy => Shape::Star,
        Element::Arcane => Shape::Dot,
        Element::Earth => Shape::Shard,
    }
}

fn nova(pen: &mut Pen, v: &V) {
    let ticks = 16.0;
    let reach = v.reach();
    let r = zones::front(v.z.age, 16, v.z.reach()) * UNIT;
    let sink = v.sinking(12.0);
    let sites = front_sites(v, PI, ticks, &[(0.22, 4), (0.42, 7), (0.62, 9), (0.8, 12), (0.94, 14)]);
    for s in &sites {
        let since = v.age - s.t0;
        if since >= 0.0 {
            erupt(pen, v, s.at, s.h * 0.9, since, sink, s.i);
        }
    }
    if v.age <= ticks + 6.0 {
        let fade = ((v.age - ticks) / 6.0).clamp(0.0, 1.0);
        front_arc(pen, v, r, PI, fade, reach);
        pen.shock(v.o, r, 1.6, 1.0, v.ramp, fade);
    }
    // The flash at the heart as it lets go.
    if v.age < 7.0 {
        soft(pen, v.o, 20.0 * (1.0 - v.age / 7.0) + 6.0, 1.0, v.ramp, 0.9 * (1.0 - v.age / 7.0));
    }
}

fn burst(pen: &mut Pen, v: &V) {
    let reach = v.reach();
    let w = BURST_WAIT as f32;
    if v.age < w {
        // The charge: the colour gathering at the mark, motes drawn in off
        // the ring along the spokes.
        let k = v.age / w;
        soft(pen, v.o, reach * 0.35 * k + 3.0, 1.0, v.ramp, 0.4 + 0.5 * k);
        for i in 0..10u32 {
            let a = v.u(i, 11) * TAU;
            let rr = reach * (1.2 - 1.1 * ((v.age + v.u(i, 12) * 3.0) / (w + 3.0)).min(1.0));
            let (px, py) = (v.o.0 + a.cos() * rr, v.o.1 + a.sin() * rr);
            pen.light(px.round() as i32, py.round() as i32, v.ramp.step(1));
            pen.light((px - a.cos()).round() as i32, (py - a.sin()).round() as i32, v.ramp.step(3));
        }
        return;
    }
    let since = v.age - w;
    let sink = v.sinking(24.0);
    // The eruption: a great piece at the mark, a ring of lesser round it,
    // each rising as the shock runs out over them.
    let tall = 24.0 * v.size();
    erupt(pen, v, v.o, tall, since, sink, 0);
    for i in 0..9u32 {
        let a = i as f32 / 9.0 * TAU + v.u(i, 2) * 0.5;
        let rr = reach * (0.5 + 0.35 * v.u(i, 3));
        let at = (v.o.0 + a.cos() * rr, v.o.1 + a.sin() * rr);
        let t0 = rr / reach * 3.0;
        if since >= t0 {
            erupt(pen, v, at, tall * (0.42 + 0.3 * v.u(i, 4)), since - t0, sink, i + 1);
        }
    }
    if since < 14.0 {
        let k = since / 14.0;
        pen.shock(v.o, reach * (0.25 + 0.95 * out(k)), 1.8, 1.0, v.ramp, k);
        soft(pen, v.o, reach * 0.9 * (1.0 - k) + 6.0, 1.0, v.ramp, 0.9 * (1.0 - k) * (1.0 - k));
    }
    pool_life(pen, v, v.o, reach * 0.9, since, sink);
    // What the eruption throws.
    let spray = Spray { angle: -PI / 2.0, spread: 2.4, gravity: 0.22, ..Spray::round(v.seed ^ 0x1234, 28, (v.o.0, v.o.1 - 4.0), (1.4, 3.8), (14.0, 24.0)) };
    spray.each(since, |m| pfx::mote(pen, &m, mote_of(v.z.area.element), v.ramp, 2, v.tick));
}

fn field(pen: &mut Pen, run: &Run, v: &V) {
    let reach = v.reach();
    let grow = out(v.age / 8.0);
    let sink = v.sinking(26.0);
    if !skip(4) {
        pool_life(pen, v, v.o, reach * grow, v.age, sink);
    }
    if v.z.area.element == Element::Storm && !skip(1) {
        // The cloud is lit from inside for the beat a bolt drops.
        let lit = run.zones.arcs.iter().any(|a| a.sky && a.age < 3);
        cloud(pen, v, v.o, reach * grow, 1.0 - sink, lit);
    }
}

/// What lingers in a pool or field: the element alive on its floor.
fn pool_life(pen: &mut Pen, v: &V, c: (f32, f32), r: f32, age: f32, sink: f32) {
    if r < 3.0 {
        return;
    }
    let ramp = v.ramp;
    let fade = 1.0 - sink;
    let t = v.tick;
    let n = ((r / 3.2) as u32).clamp(6, 16);
    // Back to front, so nearer pieces cover farther.
    let mut spots: Vec<(f32, f32, u32)> = (0..n)
        .map(|i| {
            let a = v.u(i, 21) * TAU + age * 0.01 * (v.u(i, 22) - 0.5);
            let rr = r * v.u(i, 23).sqrt() * 0.92;
            (c.0 + a.cos() * rr, c.1 + a.sin() * rr, i)
        })
        .collect();
    spots.sort_by(|a, b| a.1.total_cmp(&b.1));
    if let Some(img) = &v.sprite {
        for &(x, y, i) in spots.iter().take(9) {
            let beat = out((age - v.u(i, 28) * 6.0) / 5.0) * fade;
            stamp(pen, img, (x, y), (9.0 + 8.0 * v.u(i, 29)) * beat, i % 2 == 0);
        }
    }
    match v.z.area.element {
        Element::Fire => {
            // A few flames standing on the crust, each its own height, rate
            // and phase; where there is no flame there is only glowing seam.
            for &(x, y, i) in spots.iter().take(((n / 2) as usize).max(5)) {
                let beat = 0.45 + 0.55 * (t as f32 * (0.2 + 0.3 * v.u(i, 30)) + i as f32 * 2.3).sin().abs();
                let h = (4.0 + 15.0 * v.u(i, 24)) * beat * fade;
                flame(pen, (x, y), h, 4.0 + 6.0 * v.u(i, 25), t, v.seed.wrapping_add(i.wrapping_mul(0x2f1b)), ramp);
            }
            // Embers rising off the pool in three streams, offset in time.
            for j in 0..3u32 {
                let at = (c.0 + (v.u(j, 33) - 0.5) * r, c.1 + (v.u(j, 34) - 0.5) * r * 0.6);
                let embers = Spray { angle: -PI / 2.0, spread: 1.4, gravity: -0.012, stagger: 1.6, ..Spray::round(v.seed ^ 0xe3 ^ j, 9, at, (0.3, 0.9), (16.0, 30.0)) };
                embers.each((age + j as f32 * 11.0) % 40.0, |m| pfx::mote(pen, &m, Shape::Ember, ramp, 1, t));
            }
        }
        Element::Frost => {
            // Frost standing in the pool: small spikes through it, flakes
            // falling, glints on the crust.
            for &(x, y, i) in spots.iter().take(8) {
                let h = (6.0 + 5.0 * v.u(i, 26)) * out((age - 2.0) / 5.0) * fade;
                spike(pen, (x, y), h, h * 0.45, (v.u(i, 27) - 0.5) * h * 0.5, ramp, false);
            }
            for i in 0..14u32 {
                let x = c.0 + (v.u(i, 31) - 0.5) * 2.0 * r;
                let fall = ((age * 0.9 + v.u(i, 32) * 40.0) % 36.0) / 36.0;
                let m = Mote { i, at: (x + (fall * 6.0).sin() * 2.0, c.1 - 34.0 + fall * 38.0), vel: (0.2, 1.0), k: fall, seed: hash(i as i32, v.seed as i32, 3) };
                pfx::mote(pen, &m, Shape::Flake, ramp, 1, t);
            }
            for i in 0..4u32 {
                let a = v.u(i, 41) * TAU;
                let rr = r * v.u(i, 42).sqrt();
                pen.twinkle((c.0 + a.cos() * rr, c.1 + a.sin() * rr), 3.0, ((age * 0.08 + v.u(i, 43) * 3.0) % 1.0).min(0.999), ramp);
            }
        }
        Element::Storm => {
            // Rain falling out of the cloud in streaks, a ground arc or two
            // between points of the pool.
            for i in 0..10u32 {
                let x = c.0 + (v.u(i, 91) - 0.5) * 2.0 * r * 0.9;
                let ph = ((age * 1.7 + v.u(i, 92) * 30.0) % 22.0) / 22.0;
                let y = c.1 - 50.0 + ph * (48.0 + (v.u(i, 93) - 0.5) * 10.0) + (v.u(i, 94) - 0.5) * r * 0.4;
                pen.streak((x, y), (0.14, 1.0), 5.0, ramp, 0.12);
            }
            for i in 0..2u32 {
                let k = (t as u32 / 3 + i * 7) as i32;
                let (a, b) = (unit(k, v.seed as i32, 51) * TAU, unit(k, v.seed as i32, 52) * TAU);
                let (ra, rb) = (r * unit(k, v.seed as i32, 53).sqrt(), r * unit(k, v.seed as i32, 54).sqrt());
                let p = (c.0 + a.cos() * ra, c.1 + a.sin() * ra);
                let q = (c.0 + b.cos() * rb, c.1 + b.sin() * rb);
                if (t as u32 + i) % 3 != 0 {
                    pen.arc(p, q, hash(k, i as i32, v.seed), 2.5, ramp, 0.6 + 0.3 * sink);
                }
            }
        }
        Element::Poison => {
            // A cloud: puffs wandering over the pool, bubbles rising and
            // popping through it.
            for &(x, y, i) in &spots {
                let sway = (age * 0.07 + i as f32).sin() * 3.0;
                let rise = 4.0 + 6.0 * ((age * 0.03 + v.u(i, 61)) % 1.0);
                soft(pen, (x + sway, y - rise), 7.0 + 6.0 * v.u(i, 62), 0.8, ramp.cooler(1), (0.33 + 0.2 * v.u(i, 63)) * fade);
            }
            for i in 0..12u32 {
                let a = v.u(i, 64) * TAU;
                let rr = r * v.u(i, 65).sqrt() * 0.9;
                let life = ((age * 0.6 + v.u(i, 66) * 30.0) % 30.0) / 30.0;
                let m = Mote { i, at: (c.0 + a.cos() * rr, c.1 + a.sin() * rr - life * 14.0), vel: (0.0, -0.5), k: life, seed: hash(i as i32, v.seed as i32, 7) };
                pfx::mote(pen, &m, Shape::Bubble, ramp, if i % 3 == 0 { 2 } else { 1 }, t);
            }
        }
        Element::Holy => {
            // Light rising off the ground: motes drawn up, a dome of rays.
            for i in 0..16u32 {
                let a = v.u(i, 71) * TAU;
                let rr = r * v.u(i, 72).sqrt();
                let life = ((age * 0.7 + v.u(i, 73) * 36.0) % 36.0) / 36.0;
                let m = Mote { i, at: (c.0 + a.cos() * rr, c.1 + a.sin() * rr - life * 26.0), vel: (0.0, -1.0), k: life, seed: hash(i as i32, v.seed as i32, 8) };
                pfx::mote(pen, &m, Shape::Star, ramp, 1, t);
            }
            pfx::rays(pen, c, 10, r * 0.9, 1.0, age * 0.02, ramp, 0.55 + 0.2 * sink);
            soft(pen, c, r * 0.8, 1.0, ramp, 0.35 * fade);
        }
        Element::Arcane => {
            // Streams of motes wheeling in on the swirl, the void drinking.
            for arm in 0..3u32 {
                for j in 0..9u32 {
                    let k = (age * 0.045 + j as f32 / 9.0 + arm as f32 / 3.0) % 1.0;
                    let rr = r * (1.0 - k);
                    let a = arm as f32 / 3.0 * TAU + k * 5.5 + age * 0.03;
                    let p = (c.0 + a.cos() * rr, c.1 + a.sin() * rr);
                    pen.light(p.0.round() as i32, p.1.round() as i32, ramp.at(0.1 + 0.7 * k));
                }
            }
            soft(pen, c, r * 0.35, 1.0, ramp.cooler(2), 0.5 * fade);
        }
        Element::Earth => {
            for &(x, y, i) in spots.iter().take(10) {
                let h = (5.0 + 8.0 * v.u(i, 81)) * out((age - 1.0) / 4.0) * fade;
                spike(pen, (x, y), h, h * 0.7, (v.u(i, 82) - 0.5) * h * 0.8, ramp, false);
            }
            let sp = Spray { angle: -PI / 2.0, spread: 2.6, gravity: 0.3, ..Spray::round(v.seed ^ 0xea, 14, (c.0, c.1), (0.8, 2.2), (8.0, 14.0)) };
            sp.each(age % 30.0, |m| pfx::mote(pen, &m, Shape::Shard, ramp, 1, t));
        }
    }
}

/// The storm cloud's lobes, on their guide: three great masses riding a slow
/// loop about the field's middle, high over it, and two crown puffs rising off
/// the top, each swelling and shrinking as it churns. `(x, y, half width, half
/// height)` in pixels.
fn lobes(v: &V, c: (f32, f32), r: f32) -> [(f32, f32, f32, f32); 5] {
    let top = c.1 - 54.0;
    std::array::from_fn(|i| {
        let churn = 1.0 + 0.07 * (v.age * 0.045 + i as f32 * 2.1).sin();
        if i < 3 {
            let ph = i as f32 / 3.0 * TAU + 0.6 + v.age * 0.012;
            let rad = r * 0.4 * churn;
            (c.0 + ph.cos() * r * 0.5, top + (ph * 1.7).sin() * r * 0.07 + (i as f32 - 1.0).abs() * r * 0.05, rad * 1.25, rad * 0.8)
        } else {
            // A crown puff over the first and the third mass.
            let (px, py, prx, pry) = {
                let ph = (i as f32 - 3.0) * 2.0 / 3.0 * TAU + 0.6 + v.age * 0.012;
                let rad = r * 0.4;
                (c.0 + ph.cos() * r * 0.5, top + (ph * 1.7).sin() * r * 0.07 + (((i - 3) * 2) as f32 - 1.0).abs() * r * 0.05, rad * 1.25, rad * 0.8)
            };
            let rad = r * 0.24 * churn;
            (px - prx * 0.1, py - pry * 0.85, rad * 1.15, rad * 0.75)
        }
    })
}

/// The storm's cloud: a thunderhead of three billowing masses in the dark
/// inks, ordered-dither shaded, black underneath and a paler rim on top,
/// their edges churning slowly; lit through from inside when a bolt drops.
fn cloud(pen: &mut Pen, v: &V, c: (f32, f32), r: f32, fade: f32, lit: bool) {
    // Rim to underside: stone-grey rim, mid, shadow, soot.
    const BANK: Ramp = Ramp(['J', 'G', 'j', 'g', 'K', 'k']);
    let masses = lobes(v, c, r);
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &(x, y, rx, ry) in &masses {
        (x0, x1, y0, y1) = (x0.min(x - rx), x1.max(x + rx), y0.min(y - ry), y1.max(y + ry));
    }
    let drift = (v.age * 0.05) as i32;
    // Blocks of two by two, dithered at that size: a thunderhead is a few big
    // soft masses, and a quarter the work.
    let (bx0, by0) = ((x0.floor() as i32 - 2) & !1, (y0.floor() as i32 - 2) & !1);
    for y in (by0..=y1.ceil() as i32 + 1).step_by(2) {
        for x in (bx0..=x1.ceil() as i32 + 1).step_by(2) {
            let fx = x as f32 + 1.0;
            let fy = y as f32 + 1.0;
            // The mass this pixel is deepest in.
            let mut best = (f32::MAX, 0.0f32, 0.0f32);
            for &(mx, my, rx, ry) in &masses {
                let (dx, dy) = ((fx - mx) / rx, (fy - my) / ry);
                let e = dx * dx + dy * dy;
                if e < best.0 {
                    best = (e, dy, dx);
                }
            }
            if best.0 > 1.4 {
                continue;
            }
            // Edges that billow: noise that slides across them as it ages.
            let bulge = (unit((x + drift) / 4, y / 4, v.seed ^ 0x99) - 0.5) * 0.5;
            let e = best.0 + bulge * 0.7;
            if e > 1.0 {
                continue;
            }
            if fade < 1.0 && unit(x, y, v.seed ^ 0x5d) > fade {
                continue;
            }
            // Top lit, bottom dark; the deeper in, the darker; the rim paler.
            let down = (best.1 + 1.0) * 0.5;
            let mut k = 0.16 + down * 0.7 + e * 0.1 - (if best.1 < -0.3 && e > 0.5 { 0.26 } else { 0.0 });
            let mut ch = BANK.dithered(k.clamp(0.0, 1.0), x >> 1, y >> 1);
            if lit && e < 0.55 {
                // Lit from inside: the core goes cold white-blue, the rim stays dark.
                k = (0.25 + e * 1.1).clamp(0.0, 1.0);
                ch = v.ramp.dithered(k, x >> 1, y >> 1);
            }
            block(pen, x, y, ch);
        }
    }
}

fn cone(pen: &mut Pen, v: &V) {
    let reach = v.reach();
    let a0 = v.angle();
    let half: f32 = 0.72;
    let age = v.age;
    // A jet comes up fast, holds and thins at the end.
    let live = out(age / 5.0) * (1.0 - v.sinking(8.0));
    if live <= 0.02 {
        return;
    }
    let element = v.z.area.element;
    let ramp = v.ramp;
    // From the hand, a little out along the aim.
    let o = (v.o.0 + v.d.0 * 5.0, v.o.1 + v.d.1 * 5.0 - 4.0);
    // The fill: a cone of the element, dithered, hot at the nozzle, thin
    // and ragged at its far edge; its flicker re-rolls every other tick.
    let tick2 = (v.tick / 2) as i32;
    let r = reach * live;
    let (ux, uy) = v.d;
    let (x0, y0, x1, y1) = sector_box(o, a0, half, r);
    // Pairs of pixels across: a jet's fill is soft, and half the work.
    for y in y0..=y1 {
        for x in ((x0 & !1)..=x1).step_by(2) {
            let n = unit(x, y, (tick2 as u32).wrapping_mul(0x9e37) ^ v.seed);
            let (dx, dy) = (x as f32 + 1.0 - o.0, y as f32 + 0.5 - o.1);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r || d2 < 4.0 {
                continue;
            }
            let d = d2.sqrt();
            let q = d / r;
            // Dense at the nozzle: the far, thin parts are thrown out before
            // the angle is ever asked.
            if n > (1.0 - q).powf(0.5) * 1.15 + 0.1 {
                continue;
            }
            let cos_a = (dx * ux + dy * uy) / d;
            let spread = half * (0.35 + 0.65 * q);
            if cos_a < spread.cos() {
                continue;
            }
            let across = cos_a.clamp(-1.0, 1.0).acos() / spread;
            let dens = (1.0 - q).powf(0.5) * (1.0 - across * across * 0.65) * 1.05 + 0.1;
            if n > dens {
                continue;
            }
            let k = (across * 0.62 + q * 0.42 + (n - 0.5) * 0.14 - 0.04).clamp(0.0, 1.0);
            let ch = ramp.dithered(k, x, y);
            pen.light(x, y, ch);
            pen.light(x + 1, y, ch);
        }
    }
    // Pieces the jet carries along its guide, born at the nozzle and
    // grown with the distance they have run.
    let n = 18u32;
    for i in 0..n {
        let phase = (age * 0.06 + i as f32 / n as f32 + v.u(i, 1) * 0.15) % 1.0;
        let d = r * phase;
        let lateral = (v.u(i, 2) - 0.5) * 2.0 * half * 0.8 * d;
        let (ux, uy) = (v.d.0, v.d.1);
        let p = (o.0 + ux * d - uy * lateral, o.1 + uy * d + ux * lateral);
        if let Some(img) = &v.sprite {
            if i % 2 == 0 {
                stamp(pen, img, (p.0, p.1 + 4.0), 6.0 + 9.0 * phase, i % 4 == 0);
            }
            continue;
        }
        match element {
            Element::Fire => flame(pen, p, 4.0 + 8.0 * phase, 3.0 + 6.0 * phase, v.tick, v.seed ^ i, ramp),
            Element::Frost => {
                let m = Mote { i, at: p, vel: (ux, uy), k: phase, seed: hash(i as i32, v.seed as i32, 2) };
                pfx::mote(pen, &m, if i % 3 == 0 { Shape::Shard } else { Shape::Flake }, ramp, 1 + u8::from(phase > 0.5), v.tick);
            }
            Element::Storm => {
                if i % 3 == 0 && (v.tick as u32 + i) % 2 == 0 {
                    pen.arc(o, p, hash(tick2, i as i32, v.seed), 3.0 + phase * 3.0, ramp, 0.5 + 0.4 * phase);
                }
            }
            Element::Poison => {
                soft(pen, p, 4.0 + 9.0 * phase, 0.85, ramp.cooler(1), 0.5 * (1.0 - phase * 0.5));
                if i % 4 == 0 {
                    let m = Mote { i, at: p, vel: (ux, uy - 0.3), k: phase, seed: hash(i as i32, v.seed as i32, 4) };
                    pfx::mote(pen, &m, Shape::Bubble, ramp, 1, v.tick);
                }
            }
            Element::Holy => {
                let m = Mote { i, at: p, vel: (ux, uy), k: phase, seed: hash(i as i32, v.seed as i32, 5) };
                pfx::mote(pen, &m, Shape::Star, ramp, 1, v.tick);
            }
            Element::Arcane => {
                let a = phase * 9.0 + i as f32;
                let q = (p.0 + a.cos() * 2.5, p.1 + a.sin() * 2.5);
                pen.light(q.0.round() as i32, q.1.round() as i32, ramp.at(0.1 + 0.7 * phase));
            }
            Element::Earth => {
                let m = Mote { i, at: p, vel: (ux * 2.0, uy * 2.0), k: phase, seed: hash(i as i32, v.seed as i32, 6) };
                pfx::mote(pen, &m, Shape::Shard, ramp, 1 + u8::from(phase > 0.6), v.tick);
            }
        }
    }
    // The nozzle: a flare where it leaves the hand.
    soft(pen, o, 6.0 * live, 1.0, ramp, 0.8 * live);
}

/// The crack a line leaves, walked from the knight out to its head: a
/// jagged dark seam lit down its middle by what the element is.
fn crack(pen: &mut Pen, v: &V) {
    let n = zones::segments(v.z.area);
    let head = ((v.age - LINE_WAIT as f32) / LINE_STEP as f32).floor() as i32;
    if head < 0 {
        // The telegraph: a faint seam drawn out ahead.
        let k = (v.age / LINE_WAIT as f32).clamp(0.0, 1.0);
        let end = (v.o.0 + v.d.0 * v.reach() * 0.4 * k, v.o.1 + v.d.1 * v.reach() * 0.4 * k);
        pen.line(v.o, end, v.ramp.step(3));
        return;
    }
    let shown = (head + 1).min(n as i32);
    let fade = 1.0 - v.sinking(18.0);
    let mut prev = v.o;
    for i in 0..shown {
        let p = zones::segment_at(v.z, i as u32);
        let mut p = (p.0 * UNIT, p.1 * UNIT);
        // A seam wanders: sideways by a hashed step.
        let off = (unit(i, v.seed as i32, 0x61) - 0.5) * 4.0;
        p = (p.0 - v.d.1 * off, p.1 + v.d.0 * off);
        let steps = ((p.0 - prev.0).abs().max((p.1 - prev.1).abs())).ceil().max(1.0) as i32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let (x, y) = (prev.0 + (p.0 - prev.0) * t, prev.1 + (p.1 - prev.1) * t);
            let (px, py) = (x.round() as i32, y.round() as i32);
            if bayer(px, py) < (1.0 - fade) {
                continue;
            }
            // Dark seam three wide, a lit core one wide.
            for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                pen.put(px + dx, py + dy, 'k');
            }
            let glow = (v.age - LINE_WAIT as f32 - i as f32 * LINE_STEP as f32).max(0.0);
            let hot = (1.0 - glow / 24.0).clamp(0.0, 1.0);
            pen.light(px, py, v.ramp.dithered(0.1 + (1.0 - hot) * 0.6, px, py));
        }
        prev = p;
    }
}

fn line(pen: &mut Pen, v: &V) {
    let n = zones::segments(v.z.area);
    let sink = v.sinking(14.0);
    // Pieces up through the seam as the line reaches each segment, drawn
    // far to near.
    let mut order: Vec<u32> = (0..n).collect();
    order.sort_by(|&a, &b| zones::segment_at(v.z, a).1.total_cmp(&zones::segment_at(v.z, b).1));
    for i in order {
        let t0 = (LINE_WAIT + LINE_STEP * i) as f32;
        if v.age < t0 {
            continue;
        }
        let p = zones::segment_at(v.z, i);
        let at = (p.0 * UNIT, p.1 * UNIT);
        let h = (13.0 + 6.0 * v.u(i, 7)) * v.size() * (0.8 + 0.3 * (i as f32 / n as f32));
        erupt(pen, v, at, h, v.age - t0, sink, i);
    }
    // The head of the line: a flare running ahead of the eruptions.
    let head = ((v.age - LINE_WAIT as f32) / LINE_STEP as f32).floor();
    if head >= 0.0 && (head as u32) < n {
        let p = zones::segment_at(v.z, head as u32 + 1);
        soft(pen, (p.0 * UNIT, p.1 * UNIT), 8.0, 1.0, v.ramp, 0.7);
    }
}

fn pillar(pen: &mut Pen, v: &V) {
    let reach = v.reach();
    let wait = PILLAR_WAIT as f32;
    let element = v.z.area.element;
    let ramp = v.ramp;
    if v.age < wait {
        // The warning: a faint column's two edges rising out of the mark.
        let k = v.age / wait;
        let top = v.o.1 - 90.0;
        for side in [-1.0f32, 1.0] {
            let x = (v.o.0 + side * reach * 0.7).round() as i32;
            for y in (top as i32..v.o.1 as i32).step_by(2) {
                if bayer(x, y) < 0.4 * k {
                    pen.light(x, y, ramp.step(3));
                }
            }
        }
        return;
    }
    let since = v.age - wait;
    let k = (since / 16.0).clamp(0.0, 1.0);
    let top = v.o.1 - 130.0;
    // The column: wide at the strike, narrowing as it spends itself.
    let w = reach * 1.5 * (1.0 - k).powf(0.6) + 2.0;
    // A storm's column is the bolt itself: the glow about it is thin.
    let w = if element == Element::Storm { w * 0.4 } else { w };
    let fall = out(since / 2.5);
    let head = top + (v.o.1 - top) * fall;
    for y in top.floor() as i32..=v.o.1.ceil() as i32 {
        if y as f32 > head {
            continue;
        }
        let u = (y as f32 - top) / (v.o.1 - top);
        let half = w * (0.55 + 0.45 * u);
        for x in (v.o.0 - half).floor() as i32..=(v.o.0 + half).ceil() as i32 {
            let dx = ((x as f32 + 0.5 - v.o.0).abs() / half).clamp(0.0, 1.0);
            let n = unit(x, y / 5, v.seed ^ (v.tick as u32 / 2));
            if dx > 0.78 && n > 1.65 * (1.0 - dx) {
                continue;
            }
            if k > 0.5 && bayer(x, y) < (k - 0.5) * 1.6 {
                continue;
            }
            // Hot white down the axis, the colour toward the edge, streaks
            // running the length of it, thinning to the sky.
            if bayer(x, y) < (1.0 - u) * 0.55 || (element == Element::Storm && bayer(x, y) < 0.45) {
                continue;
            }
            let streak = (unit(x, 3, v.seed) - 0.5) * 0.3;
            let col = if dx < 0.22 { 0.0 } else { dx * 0.8 + k * 0.2 + streak + (1.0 - u) * 0.25 };
            pen.light(x, y, ramp.dithered(col.clamp(0.0, 1.0), x, y));
        }
    }
    match element {
        Element::Storm => {
            // The bolt that is the column: a jagged run down the middle,
            // forked.
            let t2 = v.tick as i32 / 2;
            let seed = hash(t2, 7, v.seed);
            for f in -1..=1 {
                pen.arc((v.o.0 + f as f32, top), (v.o.0 + f as f32, v.o.1), seed, 6.0, ramp.cooler(usize::from(f != 0)), 0.15 + k * 0.6);
            }
            if since < 6.0 {
                soft(pen, (v.o.0, v.o.1 - 30.0), 34.0, 0.6, ramp, 0.4 * (1.0 - since / 6.0));
            }
        }
        Element::Frost => {
            // An icicle as long as the column, point down, shattering as it lands.
            if since < 4.0 {
                spike(pen, (v.o.0, head), reach * 3.0, reach * 1.1, 0.0, ramp, since < 2.0);
            }
        }
        Element::Earth => {
            if since < 4.0 {
                soft(pen, (v.o.0, head - 6.0), reach * 0.9, 1.0, ramp, 0.9);
            }
        }
        _ => {}
    }
    // The landing: a ring running out, a flash, what it throws.
    if since >= 2.0 {
        let s = since - 2.0;
        let kk = (s / 12.0).clamp(0.0, 1.0);
        pen.shock(v.o, reach * (0.4 + 1.3 * out(kk)), 1.8, 1.0, ramp, kk);
        soft(pen, v.o, reach * (1.2 - kk * 0.8) + 4.0, 1.0, ramp, 0.9 * (1.0 - kk));
        let spray = Spray { angle: -PI / 2.0, spread: 2.8, gravity: 0.18, ..Spray::round(v.seed ^ 0xb11a, 24, (v.o.0, v.o.1 - 2.0), (1.2, 3.4), (12.0, 22.0)) };
        spray.each(s, |m| pfx::mote(pen, &m, mote_of(element), ramp, 2, v.tick));
    }
}

// ─── what comes up through the floor ─────────────────────────────────────────

/// One piece of the element breaking the floor at `base`, `since` ticks
/// after it began, `h` pixels tall at its height, sinking away by `sink`.
fn erupt(pen: &mut Pen, v: &V, base: (f32, f32), h: f32, since: f32, sink: f32, i: u32) {
    if sink >= 1.0 || h < 2.0 {
        return;
    }
    let grow = pop(since / 5.0) * (1.0 - sink);
    let h = h * grow;
    if h < 1.5 {
        return;
    }
    let flash = since < 2.0;
    let ramp = v.ramp;
    let lean = (v.u(i, 91) - 0.5) * h * 0.55;
    // A wish that painted its own look: that picture stands up through the
    // floor where the element's piece would, a flash and a few motes of the
    // element at its foot.
    if let Some(img) = &v.sprite {
        stamp(pen, img, base, h * 1.15, v.u(i, 95) < 0.5);
        if flash {
            soft(pen, (base.0, base.1 - h * 0.3), h * 0.5, 1.0, ramp, 0.8);
        }
        let dust = Spray { angle: -PI / 2.0, spread: 2.4, gravity: 0.12, ..Spray::round(v.seed ^ i.wrapping_mul(53), 6, base, (0.5, 1.6), (6.0, 11.0)) };
        dust.each(since, |m| pfx::mote(pen, &m, mote_of(v.z.area.element), ramp, 1, v.tick));
        return;
    }
    match v.z.area.element {
        Element::Frost => {
            spike(pen, base, h, h * 0.42, lean, ramp, flash);
            // Two lesser ones leaning off it.
            spike(pen, (base.0 - h * 0.2, base.1 + 1.0), h * 0.5, h * 0.26, -h * 0.28, ramp, flash);
            spike(pen, (base.0 + h * 0.2, base.1 + 1.0), h * 0.42, h * 0.24, h * 0.3, ramp, flash);
        }
        Element::Earth => {
            spike(pen, base, h, h * 0.55, lean * 1.3, ramp, flash);
            spike(pen, (base.0 + h * 0.22, base.1 + 1.0), h * 0.5, h * 0.38, h * 0.3, ramp, flash);
            if since < 10.0 {
                let dust = Spray { angle: -PI / 2.0, spread: 2.6, gravity: 0.05, ..Spray::round(v.seed ^ i.wrapping_mul(77), 7, base, (0.5, 1.6), (6.0, 11.0)) };
                dust.each(since, |m| pfx::mote(pen, &m, Shape::Smoke, ramps::SMOKE, 1, v.tick));
            }
        }
        Element::Fire => {
            let beat = 0.8 + 0.28 * (v.tick as f32 * (0.22 + 0.2 * v.u(i, 96)) + i as f32 * 1.7).sin();
            let breadth = 0.5 + 0.35 * v.u(i, 97);
            flame(pen, base, h * (0.7 + 0.5 * v.u(i, 98)) * beat, h * breadth, v.tick, v.seed.wrapping_add(i.wrapping_mul(0x2f1b)), ramp);
            if flash {
                soft(pen, (base.0, base.1 - h * 0.3), h * 0.55, 1.0, ramp, 0.9);
            }
        }
        Element::Storm => {
            // A bolt standing up out of the ground, jagged, a spark at its foot.
            let fade = (since / 9.0).clamp(0.0, 1.0);
            if since < 9.0 {
                pen.arc((base.0 + lean * 0.2, base.1 - h * 1.5), base, hash(v.tick as i32 / 2, i as i32, v.seed), 3.0, ramp, fade * 0.8);
            }
            soft(pen, base, 5.0 + h * 0.2, 1.0, ramp, 0.6 * (1.0 - fade));
        }
        Element::Poison => {
            // A geyser: a plume of bubbles and a swelling puff.
            let rise = Spray { angle: -PI / 2.0, spread: 0.6, gravity: 0.03, ..Spray::round(v.seed ^ i.wrapping_mul(13), 9, base, (0.8, 1.9), (10.0, 18.0)) };
            rise.each(since, |m| pfx::mote(pen, &m, Shape::Bubble, ramp, 1, v.tick));
            soft(pen, (base.0, base.1 - h * 0.3), h * 0.45, 0.9, ramp.cooler(1), 0.5 * (1.0 - since / 24.0).clamp(0.0, 1.0));
        }
        Element::Holy => {
            // A shaft of light, thin and tall, with a twinkle at its tip.
            let w = (h * 0.16).max(1.5);
            let top = base.1 - h * 1.5;
            for y in top.floor() as i32..=base.1.ceil() as i32 {
                let u = (y as f32 - top) / (base.1 - top);
                let half = w * (0.5 + 0.5 * u);
                for x in (base.0 - half).floor() as i32..=(base.0 + half).ceil() as i32 {
                    let dx = ((x as f32 + 0.5 - base.0).abs() / half.max(0.5)).min(1.0);
                    if dx > 0.7 && bayer(x, y) < 0.5 {
                        continue;
                    }
                    pen.light(x, y, ramp.dithered(0.05 + dx * 0.5 + (1.0 - u) * 0.2, x, y));
                }
            }
            pen.twinkle((base.0, top), 4.0, (since / 14.0).clamp(0.0, 0.99), ramp);
        }
        Element::Arcane => {
            // A violet shard hanging over a glowing seat.
            let w = h * 0.3;
            spike(pen, (base.0, base.1 - 3.0), h, w, lean * 0.3, ramp, flash);
            soft(pen, base, 4.0 + h * 0.2, 1.0, ramp.cooler(1), 0.5);
        }
    }
}

/// A painted look stood on `base`, `h` pixels tall (its aspect kept).
fn stamp(pen: &mut Pen, img: &Img, base: (f32, f32), h: f32, flip: bool) {
    let k = (h / img.h.max(1) as f32).clamp(0.25, 2.2);
    let scaled = super::resized(img, k);
    let scaled = if flip { scaled.flip_h() } else { scaled };
    pen.cv.stamp(&scaled, base.0.round() as i32 - scaled.w / 2, base.1.round() as i32 - scaled.h + 2);
}

/// A leaning faceted spike, ported from the pixel-effects crystal: dark
/// outline, a lit ridge, bright left facet and shaded right, white at the
/// tip; `flash` paints all of it white (the first frames it breaks).
fn spike(pen: &mut Pen, base: (f32, f32), h: f32, w: f32, lean: f32, ramp: Ramp, flash: bool) {
    if h < 1.5 {
        return;
    }
    let (bx, by) = base;
    let w = w.max(2.0);
    let ridge0 = bx - w * 0.14;
    let top = (by - h).floor() as i32;
    // The tip's outline, one pixel over it.
    pen.put((bx + lean).round() as i32, top - 1, ramp.step(5));
    for py in top..by.ceil() as i32 {
        let u = (by - (py as f32 + 0.5)) / h;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let c = bx + lean * u;
        let hw = (w / 2.0) * (1.0 - u) + 0.35;
        let (lo, hi) = ((c - hw - 0.5).ceil() as i32, (c + hw - 0.5).floor() as i32);
        if lo > hi {
            continue;
        }
        pen.put(lo - 1, py, ramp.step(5));
        pen.put(hi + 1, py, ramp.step(5));
        let rx = ridge0 + (bx + lean - ridge0) * u;
        for px in lo..=hi {
            if flash || u > 0.9 {
                pen.put(px, py, ramp.step(0));
                continue;
            }
            let fx = px as f32 + 0.5;
            let ch = if (fx - rx).abs() < 0.55 {
                ramp.step(1)
            } else if fx < rx {
                if fx - (c - hw) < 1.0 {
                    ramp.step(1)
                } else if (fx - (c - hw)) / (rx - (c - hw)).max(1.0) > 0.55 {
                    ramp.step(2)
                } else {
                    ramp.step(3)
                }
            } else if c + hw - fx < 1.2 {
                ramp.step(4)
            } else if (fx - rx) / (c + hw - rx).max(1.0) > 0.5 {
                ramp.step(3)
            } else {
                ramp.step(2)
            };
            pen.put(px, py, ch);
        }
    }
}

/// A flame, of one of three silhouettes by its seed: one tall tongue with a
/// sidekick, a twin pair of unequal height, or a wide bed of three. Each is
/// cream-white at its heart, amber then red toward its edge, and goes out in
/// a dither at the tip.
fn flame(pen: &mut Pen, base: (f32, f32), h: f32, w: f32, tick: u64, seed: u32, ramp: Ramp) {
    if h < 2.0 {
        return;
    }
    if h < 8.0 {
        tongue(pen, base, h, w, tick, seed, ramp, 0.0);
        return;
    }
    let kind = seed.wrapping_mul(0x9e37_79b9) >> 29;
    let mut t = |dx: f32, hk: f32, wk: f32, ph: f32, n: u32| tongue(pen, (base.0 + dx * w, base.1), h * hk, w * wk, tick, seed.wrapping_add(n * 0x51), ramp, ph);
    match kind % 3 {
        0 => {
            t(0.0, 1.0, 0.5, 0.0, 0);
            t(0.38, 0.4, 0.3, 2.6, 1);
        }
        1 => {
            t(-0.2, 0.72, 0.46, 1.3, 0);
            t(0.2, 1.0, 0.5, 0.0, 1);
        }
        _ => {
            t(-0.34, 0.55, 0.4, 2.1, 0);
            t(0.0, 0.95, 0.5, 0.0, 1);
            t(0.34, 0.7, 0.42, 4.0, 2);
        }
    }
}

fn tongue(pen: &mut Pen, base: (f32, f32), h: f32, w: f32, tick: u64, seed: u32, ramp: Ramp, phase: f32) {
    let t = tick as f32;
    // Each tongue sways at a rate of its own.
    let rate = 0.38 + 0.22 * unit(1, seed as i32, 5);
    let top = (base.1 - h).floor() as i32;
    for py in top..=base.1.round() as i32 {
        let u = ((base.1 - py as f32) / h).clamp(0.0, 1.0);
        let sway = (t * rate + phase + u * 4.2 + seed as f32 * 0.3).sin() * w * 0.5 * u.powf(1.3);
        let flick = 0.8 + 0.35 * unit(py, (tick / 2) as i32, seed);
        // A belly low down, narrowing to a point.
        let half = w * 0.5 * (1.0 - u.powf(1.5)).max(0.0).powf(0.8) * flick;
        let cx = base.0 + sway;
        for px in (cx - half - 1.0).floor() as i32..=(cx + half + 1.0).ceil() as i32 {
            let dx = (px as f32 + 0.5 - cx).abs();
            if dx > half {
                continue;
            }
            if u > 0.6 && bayer(px, py) < (u - 0.6) * 1.8 {
                continue;
            }
            let inner = dx / half.max(0.5);
            // Noise in the colour, so the body is not banded.
            let k = (0.1 + u * 0.55 + inner * 0.3 + (unit(px, py, seed) - 0.5) * 0.18).clamp(0.0, 1.0);
            pen.put(px, py, ramp.dithered(k, px, py));
        }
    }
}

// ─── what is struck ──────────────────────────────────────────────────────────

/// A frozen foe in its ice: a faceted shell about the figure, white
/// glints crossing it, spikes at its feet.
fn casing(pen: &mut Pen, run: &Run, e: &Enemy) {
    let (x, y, h) = works::foe_box(run, e);
    let (x, y, h) = (x as f32, y as f32, (h as f32).max(10.0));
    let w = ((e.radius() * 2.0 * UNIT).clamp(8.0, 40.0)) * 0.62;
    let ramp = ramps::FROST;
    let seed = e.id.wrapping_mul(0x9e37);
    let top = y - h;
    for py in top.floor() as i32 - 1..=y.ceil() as i32 + 1 {
        let u = ((y - py as f32) / h).clamp(0.0, 1.0);
        // A rounded block, narrowing at the top.
        let half = w * (0.72 + 0.28 * (1.0 - u * u).sqrt());
        for px in (x - half).floor() as i32..=(x + half).ceil() as i32 {
            let dx = ((px as f32 + 0.5 - x).abs() / half).clamp(0.0, 1.0);
            let edge = dx > 0.82 || py as f32 <= top + 1.0 || py as f32 >= y - 1.0;
            if edge {
                pen.light(px, py, if (px as f32 + 0.5) < x { ramp.step(1) } else { ramp.step(3) });
            } else if bayer(px, py) < 0.22 {
                pen.light(px, py, if (px as f32 + 0.5) < x { ramp.step(2) } else { ramp.step(4) });
            }
        }
    }
    // The glint crossing the block on the diagonal.
    let g = (((run.tick as f32) * 0.09 + unit(0, seed as i32, 1) * 3.0) % 1.8) - 0.4;
    if (0.0..=1.0).contains(&g) {
        let gy = y - h * g;
        for k in -2i32..=2 {
            pen.light((x - w * 0.4 + g * w * 0.8 + k as f32).round() as i32, (gy - k as f32).round() as i32, ramp.step(0));
        }
    }
    spike(pen, (x - w * 0.9, y + 1.0), h * 0.35, w * 0.35, -w * 0.3, ramp, false);
    spike(pen, (x + w * 0.9, y + 1.0), h * 0.3, w * 0.3, w * 0.3, ramp, false);
}

/// The ice shards a shatter throws: where each flies, on its radial guide.
fn shard_guide(seed: u32, i: u32, n: u32, age: f32, at: (f32, f32)) -> ((f32, f32), f32) {
    let a = (i as f32 + unit(i as i32, seed as i32, 1) * 0.5) / n as f32 * TAU + unit(i as i32, seed as i32, 2) * 0.35;
    let speed = 2.8 + 2.4 * unit(i as i32, seed as i32, 3);
    // Out along the guide, slowing; a little fall once it is up.
    let run = speed * (1.0 - crate::motion::math::exp(-0.13 * age)) / 0.13;
    ((at.0 + a.cos() * run, at.1 + a.sin() * run * 0.9 + 0.035 * age * age), a)
}

/// A shatter: the casing flashes white-blue and bursts into six to ten
/// angular shards that fly out along radial guides, tumbling and fading, a
/// small puff of frost where it stood.
fn shatter(pen: &mut Pen, run: &Run, s: &zones::Shard) {
    let age = f32::from(s.age);
    let k = age / f32::from(SHARD_TICKS);
    let at = (s.x * UNIT, s.y * UNIT - 8.0);
    let ramp = ramps::FROST;
    let seed = u32::from(s.seed).wrapping_mul(0x85eb);
    // The casing, one last time, lit white through.
    if age < 3.0 {
        for dy in -10i32..=9 {
            let half = (7.0 - (dy as f32).abs() * 0.3) as i32;
            for dx in -half..=half {
                let (x, y) = (at.0.round() as i32 + dx, at.1.round() as i32 + dy);
                if bayer(x, y) >= age * 0.3 {
                    pen.light(x, y, if dx.abs() >= half - 1 { ramp.step(1) } else { ramp.step(0) });
                }
            }
        }
    }
    // The puff.
    if age < 10.0 {
        soft(pen, (at.0, at.1 + 3.0), 6.0 + age * 1.3, 0.8, ramp.cooler(1), 0.6 * (1.0 - age / 10.0));
    }
    let n = 7 + seed % 4;
    for i in 0..n {
        let (p, a) = shard_guide(seed, i, n, age, at);
        let tumble = a + age * (0.28 + 0.3 * unit(i as i32, seed as i32, 4)) * if i % 2 == 0 { 1.0 } else { -1.0 };
        let size = 5.0 + 4.5 * unit(i as i32, seed as i32, 5);
        let fade = ((k - 0.3) / 0.7).clamp(0.0, 1.0);
        ice_shard(pen, p, tumble, size, fade, ramp);
    }
    // Flakes hanging after them.
    let flakes = Spray { angle: -PI / 2.0, spread: TAU * 0.9, gravity: 0.04, ..Spray::round(seed ^ 0x77, 8, at, (0.5, 1.6), (12.0, 20.0)) };
    flakes.each(age, |m| pfx::mote(pen, &m, Shape::Flake, ramp, 1, run.tick));
}

/// One angular shard: a long kite, lit on one side and shaded on the other,
/// white at its point, dissolving by ordered dither as it fades.
fn ice_shard(pen: &mut Pen, at: (f32, f32), spin: f32, size: f32, fade: f32, ramp: Ramp) {
    let tip = (at.0 + spin.cos() * size, at.1 + spin.sin() * size);
    let (la, ra) = (spin + 2.5, spin - 2.5);
    let (l, r) = ((at.0 + la.cos() * size * 0.55, at.1 + la.sin() * size * 0.55), (at.0 + ra.cos() * size * 0.55, at.1 + ra.sin() * size * 0.55));
    let tail = (at.0 - spin.cos() * size * 0.45, at.1 - spin.sin() * size * 0.45);
    let (x0, x1) = ((tip.0.min(l.0).min(r.0).min(tail.0)).floor() as i32, (tip.0.max(l.0).max(r.0).max(tail.0)).ceil() as i32);
    let (y0, y1) = ((tip.1.min(l.1).min(r.1).min(tail.1)).floor() as i32, (tip.1.max(l.1).max(r.1).max(tail.1)).ceil() as i32);
    let (ax, ay) = (spin.cos(), spin.sin());
    for y in y0..=y1 {
        for x in x0..=x1 {
            if fade > 0.0 && bayer(x, y) < fade {
                continue;
            }
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Inside the kite: on the inner side of each of its four edges.
            let side = |a: (f32, f32), b: (f32, f32)| (b.0 - a.0) * (py - a.1) - (b.1 - a.1) * (px - a.0);
            let (s1, s2, s3, s4) = (side(tip, l), side(l, tail), side(tail, r), side(r, tip));
            let inside = (s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0 && s4 >= 0.0) || (s1 <= 0.0 && s2 <= 0.0 && s3 <= 0.0 && s4 <= 0.0);
            if !inside {
                continue;
            }
            // Which side of its spine, and how near the point.
            let across = -(px - at.0) * ay + (py - at.1) * ax;
            let along = (px - at.0) * ax + (py - at.1) * ay;
            let ch = if along > size * 0.6 {
                ramp.step(0)
            } else if across.abs() < 0.5 {
                ramp.step(1)
            } else if across < 0.0 {
                ramp.step(2)
            } else {
                ramp.step(4)
            };
            pen.put(x, y, ch);
        }
    }
}

/// A bolt of lightning between two foes, or out of the sky onto one.
fn arc(pen: &mut Pen, a: &zones::Arc) {
    let age = f32::from(a.age);
    let k = age / f32::from(ARC_TICKS);
    let to = (a.to.0 * UNIT, a.to.1 * UNIT - 6.0);
    let seed = u32::from(a.seed);
    let from = if a.sky {
        (to.0 + (unit(0, seed as i32, 3) - 0.5) * 12.0, to.1 - 66.0)
    } else {
        (a.from.0 * UNIT, a.from.1 * UNIT - 6.0)
    };
    // The flicker: the same bolt twice in its first beats, then fading.
    let ramp = ramps::STORM;
    let s2 = if a.age < 3 { seed } else { seed ^ (u32::from(a.age) / 2) };
    let jag = if a.sky { 7.0 } else { 3.5 };
    pen.arc(from, to, s2, jag, ramp, k * 0.8);
    if a.sky {
        // Three wide: the same path again either side, the flank cooler.
        pen.arc((from.0 + 1.0, from.1), (to.0 + 1.0, to.1), s2, jag, ramp.cooler(2), 0.3 + k * 0.6);
    } else if a.age < 3 {
        pen.arc((from.0 + 1.0, from.1), (to.0 + 1.0, to.1), s2 ^ 5, jag, ramp, 0.1);
    }
    soft(pen, (to.0, to.1 + 5.0), (if a.sky { 11.0 } else { 7.0 }) * (1.0 - k) + 3.0, 1.0, ramp, 0.95 * (1.0 - k));

}

/// The blows a zone has just landed: each struck foe flashes, in the
/// element's colour, a spark ring and spray.
fn blows(pen: &mut Pen, run: &Run, v: &V) {
    let ramp = v.ramp;
    for &(id, born) in v.z.hit.iter().rev().take(24) {
        let since = v.z.age.saturating_sub(born);
        if since > 6 {
            continue;
        }
        let Some(e) = run.enemies.iter().find(|e| e.id == id) else { continue };
        let (x, y, h) = works::foe_box(run, e);
        let at = (x as f32, y as f32 - h as f32 * 0.5);
        if since < 3 {
            soft(pen, at, 7.0 - since as f32 * 1.6, 1.0, ramp, 0.85 - since as f32 * 0.2);
        }
        let seed = id.wrapping_mul(0x85eb);
        let spray = Spray { angle: -PI / 2.0, spread: TAU * 0.8, gravity: 0.12, ..Spray::round(seed, 8, at, (1.2, 3.2), (6.0, 10.0)) };
        spray.each(since as f32, |m| pfx::mote(pen, &m, Shape::Spark, ramp, 1, run.tick));
    }
}

// ─── the guides ──────────────────────────────────────────────────────────────

/// The paths each zone is drawn along, in bright pink (`guides`), so the
/// path can be checked against the picture and then turned off.
fn guide_layer(pen: &mut Pen, v: &V) {
    let z = v.z;
    let dot = |pen: &mut Pen, p: (f32, f32)| {
        pen.put(p.0.round() as i32, p.1.round() as i32, '+');
    };
    let arc_pts = |pen: &mut Pen, c: (f32, f32), r: f32, a0: f32, half: f32| {
        let n = ((2.0 * half * r) / 1.5).ceil().max(8.0) as i32;
        for s in 0..=n {
            let a = a0 - half + 2.0 * half * s as f32 / n as f32;
            dot(pen, (c.0 + a.cos() * r, c.1 + a.sin() * r));
        }
    };
    dot(pen, v.o);
    for k in 0..6 {
        dot(pen, (v.o.0 + v.d.0 * k as f32, v.o.1 + v.d.1 * k as f32));
    }
    match z.area.form {
        Form::Wave => {
            arc_pts(pen, v.o, zones::front(z.age, 20, z.reach()) * UNIT, v.angle(), 0.95);
            arc_pts(pen, v.o, v.reach(), v.angle(), 0.95);
            // The sector's two edges.
            for side in [-1.0f32, 1.0] {
                let a = v.angle() + side * 0.95;
                for s in 0..28 {
                    let d = v.reach() * s as f32 / 28.0;
                    dot(pen, (v.o.0 + a.cos() * d, v.o.1 + a.sin() * d));
                }
            }
        }
        Form::Nova => arc_pts(pen, v.o, zones::front(z.age, 16, z.reach()) * UNIT, 0.0, PI),
        Form::Field if z.area.element == Element::Storm => {
            arc_pts(pen, v.o, v.reach(), 0.0, PI);
            // The thunderhead's lobes on their drift: each ellipse's middle, and its outline.
            for (x, y, rx, ry) in lobes(v, v.o, v.reach()) {
                dot(pen, (x, y));
                for s in 0..24 {
                    let a = s as f32 / 24.0 * TAU;
                    dot(pen, (x + a.cos() * rx, y + a.sin() * ry));
                }
            }
        }
        Form::Burst | Form::Field | Form::Pillar => arc_pts(pen, v.o, v.reach(), 0.0, PI),
        Form::Cone => {
            for side in [-1.0f32, 1.0] {
                let a = v.angle() + side * 0.72;
                for s in 0..24 {
                    let d = v.reach() * s as f32 / 24.0;
                    dot(pen, (v.o.0 + a.cos() * d, v.o.1 + a.sin() * d));
                }
            }
        }
        Form::Line => {
            for i in 0..zones::segments(z.area) {
                let p = zones::segment_at(z, i);
                dot(pen, (p.0 * UNIT, p.1 * UNIT));
                arc_pts(pen, (p.0 * UNIT, p.1 * UNIT), 1.7 * UNIT, 0.0, PI);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__zones__shots.rs"]
mod shots;
