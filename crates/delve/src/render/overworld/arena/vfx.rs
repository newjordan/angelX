//! The fight's sparks and smoke, drawn over the figures and under the shots
//! in flight, so no burst ever hides a monster's shot.
//!
//! - **A shield's block** (`rules::shield`, `feel::Block`): the face lit
//!   across the shot's line, a star where it struck, shards in the shot's
//!   own colours sprayed back off the face to fly and fall, a ring opening
//!   off it; a heavier shot throws more, further, for longer. The shield
//!   itself flashes and is knocked back a pixel or two (`knights::Struck`).
//! - **A hit** (`feel::Impact`): a hot star on the struck side and sparks
//!   thrown back out of it, a few through, falling (a volley striking
//!   together throws one spray); a shot a bomb popped bursts where it hung.
//! - **A death** (`feel::Death`): the held flash, then the monster in
//!   pieces of its own colours thrown up and out to land and lie a moment,
//!   a ring of dust on the floor and smoke going up.
//! - **A burst and a bomb** (`Run::blasts`, a knight's `bomb_cooldown`): a
//!   fireball from a white-hot pin to embers, a shock ring, cinders that
//!   fall and smoke after; the bomb's wave through the room with a dark
//!   ring behind its bright edge, its fire behind the knight who threw it.
//! - **A muzzle** (`feel::Puff`): a star along the aim, side spikes and a
//!   spark or two; a handgonne's smoke as small lit puffs that thin.
//! - **A roll's dust** (`feel::Puff::dust`): kicked up behind as it starts,
//!   left along it, thrown out both ways where it lands; under the figures.
//! - **A find** (`feel::Glint`): a star where it lay, a ring on the floor
//!   and motes rising, in its rarity's colours; a rare card on the floor
//!   twinkles now and then.
//!
//! Every effect here keys on what the run holds (`Run::feel`), so a
//! friend's mirror, a browser and the window draw it alike, and on nothing
//! but the run: the same run, the same pixels. Each of a frame's two passes
//! (behind the figures, and over them under the shots) draws at most
//! `PARTICLES` motes: a knight's guard first, then the newest effects.

use super::super::ink::{BLACK, Img, Rgb, bayer, hash, shade};
use super::fx;
use super::knights::Struck;
use super::{UNIT, sprites};
use crate::rules::feel::{
    Block, DEATH_HOLD, DEATH_TICKS, DUST_TICKS, FLASH_TICKS, GLINT_TICKS, IMPACT_TICKS, KICK, LAND, SMOKE_TICKS,
};
use crate::rules::{BLAST_TICKS, BOMB_REARM, EnemyKind, Run, Shot};
use std::f32::consts::{PI, TAU};

/// The most motes one pass of a room's frame draws, all its effects
/// together.
pub(super) const PARTICLES: u32 = 480;

/// What is left of a frame's motes.
pub(super) struct Budget(u32);

impl Budget {
    pub(super) fn new() -> Self {
        Budget(PARTICLES)
    }

    /// One more mote, if the frame has room for it.
    fn take(&mut self) -> bool {
        let room = self.0 > 0;
        self.0 = self.0.saturating_sub(1);
        room
    }

    /// Motes drawn so far.
    pub(super) fn spent(&self) -> u32 {
        PARTICLES - self.0
    }
}

/// A mote put through the budget.
fn put(cv: &mut Img, budget: &mut Budget, x: i32, y: i32, ink: char) {
    if budget.take() {
        cv.put(x, y, ink);
    }
}

/// A mote of a colour (a monster's own) put through the budget.
fn set(cv: &mut Img, budget: &mut Budget, x: i32, y: i32, c: Rgb) {
    if budget.take() {
        cv.set(x, y, c);
    }
}

/// A shot's colours as it breaks: hottest, its core, its rim, cooling, and
/// the dark it dies to; whether its pieces are solid (chips that drop and
/// do not trail) or a gas (motes that drift up).
#[derive(Clone, Copy)]
struct Ramp {
    inks: [char; 5],
    solid: bool,
    gas: bool,
}

fn ramp(kind: Shot) -> Ramp {
    let (inks, solid, gas) = match kind {
        Shot::Ember | Shot::Flame | Shot::Sacred | Shot::Ball => (['9', '6', '@', 'R', 'P'], false, false),
        Shot::Missile => (['c', '@', '7', 'R', 'g'], false, false),
        Shot::Ore => (['T', 'O', 'R', 'r', 'b'], true, false),
        Shot::Bone => (['H', 'i', 'h', 'J', 'g'], true, false),
        Shot::Page => (['c', '$', 'V', 'U', 'S'], true, false),
        Shot::Frost => (['W', 'V', 'z', 'Q', 'q'], false, false),
        Shot::Hex | Shot::Spore => (['Y', 'Y', 'C', 'm', 'e'], false, true),
        Shot::Wisp | Shot::Orb | Shot::Eye => (['3', '3', '2', 'q', 'S'], false, false),
        Shot::Arrow | Shot::Bolt | Shot::Blade => (['W', 'V', 'v', 'U', 'u'], false, false),
    };
    Ramp { inks, solid, gas }
}

impl Ramp {
    /// The ink `k` of the way (0 to 1) through a piece's life.
    fn at(&self, k: f32) -> char {
        let i = if k < 0.2 {
            1
        } else if k < 0.45 {
            2
        } else if k < 0.75 {
            3
        } else {
            4
        };
        self.inks[i]
    }
}

/// A seeded share in `0..1`.
fn unit_hash(a: i32, b: i32, s: u32) -> f32 {
    (hash(a, b, s) % 1024) as f32 / 1024.0
}

/// One piece of a spray, thrown from `o` at `speed` pixels a tick along
/// `angle`, slowing, falling `fall` pixels a tick per tick, gone after
/// `life` ticks: where it is `age` ticks in.
#[derive(Clone, Copy)]
struct Piece {
    o: (f32, f32),
    angle: f32,
    speed: f32,
    fall: f32,
    life: f32,
}

impl Piece {
    fn at(&self, age: f32) -> (f32, f32) {
        let run = self.speed * (age - 0.045 * age * age).max(0.0);
        (
            self.o.0 + self.angle.cos() * run,
            self.o.1 + self.angle.sin() * run + 0.5 * self.fall * age * age,
        )
    }
}

/// Draw `piece` at `age`: its head, and while it is fast a pixel of trail
/// behind it; in `ramp`'s colours as it cools.
fn piece(cv: &mut Img, budget: &mut Budget, p: Piece, age: f32, ramp: Ramp, first: char) {
    if age >= p.life {
        return;
    }
    let k = age / p.life;
    let (x, y) = p.at(age);
    let head = if age < 1.0 { first } else { ramp.at(k) };
    if !ramp.solid && age >= 1.0 && p.speed > 1.1 && k < 0.6 {
        let (tx, ty) = p.at(age - 0.6);
        put(cv, budget, tx.round() as i32, ty.round() as i32, ramp.at((k + 0.3).min(0.99)));
    }
    put(cv, budget, x.round() as i32, y.round() as i32, head);
    if ramp.solid && p.life > 7.0 && k < 0.5 {
        // A chip big enough to tumble: two pixels.
        put(cv, budget, x.round() as i32 + 1, y.round() as i32, ramp.inks[4]);
    }
}

/// How high off the floor a top-down burst at a knight's shield is drawn,
/// in pixels: a shot is painted at its floor point and the shield stands
/// over it, so its burst rises to meet the shield as it is drawn: a
/// little for one held out to the side or away, more for one held toward
/// the viewer, which covers the knight's front above their feet.
fn shield_lift(run: &Run, b: &Block) -> f32 {
    if run.side_on() {
        return 0.0;
    }
    let toward = outward(b).1.max(0.0);
    3.0 + 5.0 * toward
}

/// The way a block's face looks out at the struck point, as it did when
/// struck; back up the shot's line when it does not say.
fn outward(b: &Block) -> (f32, f32) {
    if b.nx == 0.0 && b.ny == 0.0 { (-b.dx, -b.dy) } else { (b.nx, b.ny) }
}

/// A shield held away from the viewer is behind its knight: what strikes
/// it is drawn under the figures, the knight before it.
fn behind(run: &Run, b: &Block) -> bool {
    !run.side_on() && outward(b).1 < -0.5
}

/// The fight's bursts, over the figures and under the shots: newest and
/// nearest a knight's own guard first, should a frame run out of motes.
pub(super) fn under_shots(cv: &mut Img, run: &Run) {
    let mut budget = Budget::new();
    blocks(cv, run, &mut budget, false);
    impacts(cv, run, &mut budget);
    muzzles(cv, run, &mut budget);
    deaths(cv, run, &mut budget);
    blasts(cv, run, &mut budget);
    glints(cv, run, &mut budget);
    twinkles(cv, run, &mut budget);
}

/// What happens behind the figures: a block on a shield held away, a
/// knight's own bomb, and the dust and smoke on the floor.
pub(super) fn under_figures(cv: &mut Img, run: &Run) {
    let mut budget = Budget::new();
    blocks(cv, run, &mut budget, true);
    bombs(cv, run, &mut budget);
    dust(cv, run, &mut budget);
    smoke(cv, run, &mut budget);
}

/// Knight `id`'s raised shield as the newest block on it leaves it: the
/// knock, and the flash.
pub(super) fn struck(run: &Run, id: u32) -> Struck {
    let Some(b) = run.feel.blocks.iter().rev().find(|b| b.knight == id && b.age() <= 2) else {
        return Struck::default();
    };
    let hero = run.players.get(&id);
    let age = b.age();
    // Back along the shot, a pixel; a heavy one two at first.
    let far = if b.weight >= 3 && age == 0 { 2.0 } else if age <= 1 { 1.0 } else { 0.0 };
    // Drawn facing right; a knight facing left is the picture turned over.
    let flip = if hero.is_some_and(|h| h.aim_x < -0.3) { -1.0 } else { 1.0 };
    Struck {
        dx: (b.dx * far * flip).round() as i32,
        dy: (b.dy * far).round() as i32,
        flash: 2u8.saturating_sub(age),
    }
}

/// Every shot a raised shield stopped, newest first: those behind their
/// knight, or the rest.
fn blocks(cv: &mut Img, run: &Run, budget: &mut Budget, back: bool) {
    for b in run.feel.blocks.iter().rev().filter(|b| behind(run, b) == back) {
        block(cv, b, shield_lift(run, b), budget);
    }
}

/// A chunky mote: two pixels square, as a shot's own pixels are drawn.
fn chunk(cv: &mut Img, budget: &mut Budget, x: i32, y: i32, ink: char) {
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        put(cv, budget, x + dx, y + dy, ink);
    }
}

/// One shot stopped on a shield's face.
fn block(cv: &mut Img, b: &Block, lift: f32, budget: &mut Budget) {
    let age = b.age();
    let heavy = b.weight >= 3;
    let w = f32::from(b.weight);
    let ramp = ramp(b.kind);
    let (px, py) = (b.x * UNIT, b.y * UNIT - lift);
    let (ix, iy) = (px.floor() as i32, py.floor() as i32);
    let (nx, ny) = outward(b);
    let normal = ny.atan2(nx);
    let (ax, ay) = (-ny, nx);
    let seed = hash(ix, iy, b.knight * 31 + b.kind as u32);
    let [hot, core, rim, cool, _] = ramp.inks;
    let at = |along: f32, out: f32| ((px + ax * along + nx * out).round() as i32, (py + ay * along + ny * out).round() as i32);
    // The tick it lands: a star of two-pixel points round a hot middle,
    // its long points along the face, a short one back up the shot's line.
    if age == 0 {
        fx::glow(cv, ix, iy, glow_hue(b.kind));
        let long = 1.5 + w;
        for (along, out, reach) in [(1.0, 0.0, long), (-1.0, 0.0, long), (0.0, 1.0, 1.0 + w * 0.5)] {
            let mut r = 1.0;
            while r <= reach {
                let (x, y) = at(along * r * 1.6 - 0.5, out * r * 1.6 - 0.5);
                chunk(cv, budget, x, y, if r + 1.0 > reach { rim } else { core });
                r += 1.0;
            }
        }
        let (x, y) = at(-0.5, -0.5);
        chunk(cv, budget, x, y, hot);
    }
    // Then the face stays lit across the line a tick or two, breaking up.
    let lit = if heavy { 2 } else { 1 };
    if (1..=lit).contains(&age) {
        let span = (1.5 + w) as i32;
        for k in -span..=span {
            if (k + i32::from(age)) % 2 == 0 {
                continue;
            }
            let (x, y) = at(k as f32, -0.5);
            put(cv, budget, x, y, if age == 1 { rim } else { cool });
        }
    }
    // A ring opening off the face, on its outward side, dotted.
    let rings = 2 + u8::from(b.weight >= 2) + u8::from(heavy);
    if (1..=rings).contains(&age) {
        let r = 2.0 + f32::from(age) * (1.4 + 0.5 * w);
        let n = (r * 2.2) as i32;
        for i in 0..=n {
            if (i + i32::from(age)) % 2 == 1 {
                continue;
            }
            let a = normal - PI * 0.55 + PI * 1.1 * i as f32 / n as f32;
            let ink = if age + 1 >= rings { cool } else { rim };
            put(cv, budget, (px + a.cos() * r).round() as i32, (py + a.sin() * r * 0.85).round() as i32, ink);
        }
    }
    // Shards sprayed back off the face, more and faster the heavier it was:
    // they fly, slow, and fall to the floor.
    let n = 5 + 3 * u32::from(b.weight);
    for i in 0..n {
        let r = |s: u32| unit_hash(i as i32, seed as i32, s);
        // Most fly back off the face; a few glance along it.
        let spread = if i % 4 == 3 { 1.45 } else { 0.9 };
        let p = Piece {
            o: (px + nx, py + ny),
            angle: normal + (r(1) - 0.5) * 2.0 * spread,
            speed: (1.1 + 1.9 * r(2)) * (0.7 + 0.3 * w),
            fall: if ramp.gas { -0.06 } else if ramp.solid { 0.34 } else { 0.2 },
            life: 4.0 + (r(3) * 4.0).floor() + w,
        };
        let age = f32::from(age);
        if i % 3 == 0 && age < 2.0 {
            // The first few go as two-pixel sparks.
            let (x, y) = p.at(age + 0.5);
            chunk(cv, budget, x.round() as i32, y.round() as i32, if age < 1.0 { hot } else { core });
        } else {
            piece(cv, budget, p, age + 0.5, ramp, if i % 3 == 0 { hot } else { core });
        }
    }
}

// ─── hits ────────────────────────────────────────────────────────────────────

/// The sparks of a knight's hit, by what struck: iron (a head, a bolt, a
/// ball, a blade) throws hot sparks, a holy shot gold, a monster's shot
/// its own colours.
fn hit_ramp(kind: Shot) -> Ramp {
    match kind {
        Shot::Arrow | Shot::Bolt | Shot::Blade => Ramp { inks: ['9', '6', '@', 'o', 'P'], solid: false, gas: false },
        Shot::Sacred => Ramp { inks: ['9', '5', '6', '@', 'o'], solid: false, gas: false },
        _ => ramp(kind),
    }
}

/// Every shot that struck: a monster, the stone, or nothing (popped). A
/// volley that strikes together throws one spray: a hit beside another of
/// the same moment keeps its star and throws two sparks, not its own.
fn impacts(cv: &mut Img, run: &Run, budget: &mut Budget) {
    let mut drawn: Vec<(f32, f32, u8)> = Vec::new();
    for i in run.feel.impacts.iter().rev() {
        // A library boss's shot bursts in its own delivered bank.
        if let Some(art) = i.boss_art {
            let (x, y) = ((i.x * UNIT).floor() as i32, (i.y * UNIT).floor() as i32);
            super::boss_art::impact(cv, art, x, y, i.ttl);
            continue;
        }
        let age = (IMPACT_TICKS - 1).saturating_sub(i.ttl);
        let crowded = drawn
            .iter()
            .any(|&(x, y, a)| a.abs_diff(age) <= 1 && (x - i.x).hypot(y - i.y) * UNIT < 6.0);
        drawn.push((i.x, i.y, age));
        let ramp = hit_ramp(i.kind);
        let [hot, core, rim, cool, _] = ramp.inks;
        let (px, py) = (i.x * UNIT, i.y * UNIT);
        let (ix, iy) = (px.floor() as i32, py.floor() as i32);
        let seed = hash(ix, iy, i.kind as u32 + 7);
        let popped = i.dx == 0.0 && i.dy == 0.0;
        if popped {
            // A shot a bomb cleared bursts where it hung: a pop and a ring.
            if age == 0 {
                chunk(cv, budget, ix - 1, iy - 1, hot);
                for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                    put(cv, budget, ix + dx, iy + dy, core);
                }
            } else if age <= 3 {
                let r = 1.5 + f32::from(age) * 1.5;
                for k in 0..8 {
                    if (k + i32::from(age)) % 2 == 0 {
                        let a = k as f32 / 8.0 * TAU;
                        let ink = if age <= 1 { rim } else { cool };
                        put(cv, budget, (px + a.cos() * r).round() as i32, (py + a.sin() * r).round() as i32, ink);
                    }
                }
            }
            continue;
        }
        let (dx, dy) = (i.dx, i.dy);
        let back = (-dy).atan2(-dx);
        let heavy = matches!(i.kind, Shot::Ball | Shot::Sacred);
        if age == 0 && crowded {
            // One of a volley: the hot middle alone.
            chunk(cv, budget, ix - 1, iy - 1, core);
        } else if age == 0 {
            // A hot star on the struck side: long points across the shot and
            // back up it, short ones between.
            fx::glow(cv, ix, iy, glow_hue(i.kind));
            let long = if heavy { 5 } else { 4 };
            for (sx, sy, reach) in [(-dy, dx, long), (dy, -dx, long), (-dx, -dy, long - 1), (dx, dy, 2)] {
                for r in 2..=reach {
                    let ink = if r == reach { rim } else { core };
                    put(cv, budget, (px + sx * r as f32).round() as i32, (py + sy * r as f32).round() as i32, ink);
                }
            }
            for (sx, sy) in [(-dx - dy, -dy + dx), (-dx + dy, -dy - dx)] {
                put(cv, budget, (px + sx * 1.6).round() as i32, (py + sy * 1.6).round() as i32, rim);
            }
            chunk(cv, budget, ix - 1, iy - 1, hot);
        } else if age <= 2 && !crowded {
            // Then a ring off it, dotted.
            let r = 1.0 + f32::from(age) * 1.8;
            for k in 0..10 {
                if (k + i32::from(age)) % 2 == 0 {
                    let a = back - PI * 0.6 + PI * 1.2 * k as f32 / 9.0;
                    put(cv, budget, (px + a.cos() * r).round() as i32, (py + a.sin() * r).round() as i32, if age == 1 { rim } else { cool });
                }
            }
        }
        // Sparks: most thrown back out of the hit, a few on through it.
        let n = match i.kind {
            _ if crowded => 2,
            Shot::Ball | Shot::Sacred => 7,
            Shot::Bolt => 6,
            _ => 5,
        };
        for k in 0..n {
            let r = |s: u32| unit_hash(k, seed as i32, s);
            let through = k % 3 == 2;
            let p = Piece {
                o: (px, py),
                angle: if through { dy.atan2(dx) + (r(1) - 0.5) * 1.0 } else { back + (r(1) - 0.5) * 2.2 },
                speed: (1.5 + 1.8 * r(2)) * if heavy { 1.25 } else { 1.0 },
                fall: 0.22,
                life: 3.0 + (r(3) * 3.0).floor(),
            };
            let t = f32::from(age) + 0.6;
            if k % 3 == 0 && age == 1 {
                // The first sparks off it go as two-pixel chunks.
                let (x, y) = p.at(t);
                chunk(cv, budget, x.round() as i32, y.round() as i32, core);
            } else {
                piece(cv, budget, p, t, ramp, if k == 0 { hot } else { core });
            }
        }
    }
}

// ─── deaths ──────────────────────────────────────────────────────────────────

std::thread_local! {
    /// Each kind's three most-worn colours, read off its sprite once.
    static COLOURS: std::cell::RefCell<Vec<(EnemyKind, [Rgb; 3])>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The colours a monster of `kind` comes apart in: the three its sprite
/// wears most, brightest first.
fn colours_of(kind: EnemyKind) -> [Rgb; 3] {
    if let Some(found) = COLOURS.with(|c| c.borrow().iter().find(|(k, _)| *k == kind).map(|(_, c)| *c)) {
        return found;
    }
    let sprite = sprites::enemy(kind, 0);
    let mut counts: Vec<(Rgb, u32)> = Vec::new();
    for c in sprite.pixels() {
        let lum = u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]);
        if sprite.w == 0 || c == BLACK || lum < 90 {
            continue;
        }
        match counts.iter_mut().find(|(k, _)| *k == c) {
            Some((_, n)) => *n += 1,
            None => counts.push((c, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let fallback = [[148, 150, 140], [99, 101, 92], [61, 58, 54]];
    let mut out: [Rgb; 3] = std::array::from_fn(|i| counts.get(i).map_or(fallback[i], |c| c.0));
    out.sort_by_key(|c| std::cmp::Reverse(u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2])));
    COLOURS.with(|c| c.borrow_mut().push((kind, out)));
    out
}

/// Every monster that fell: its flash, then its pieces, dust and smoke.
fn deaths(cv: &mut Img, run: &Run, budget: &mut Budget) {
    for d in run.feel.deaths.iter().rev() {
        let (px, py) = (d.x * UNIT, d.y * UNIT);
        let (x, y) = (px.floor() as i32, py.floor() as i32);
        let r = (d.r * UNIT).max(2.0);
        let age = DEATH_TICKS.saturating_sub(d.ttl);
        if age < DEATH_HOLD {
            // The held frame: a shaded flash disc, as the hit-stop holds.
            let ri = r.round() as i32;
            let held = fx::stamp(fx::Stamp::Death, ri, ri, age);
            cv.stamp(&held, x - ri, y - ri);
            continue;
        }
        let k = f32::from(age - DEATH_HOLD);
        let seed = hash(x, y, 13);
        let colours = d.kind.map_or([[148, 150, 140], [99, 101, 92], [61, 58, 54]], colours_of);
        // A ring of dust where it stood, opening on the floor.
        if (1.0..8.0).contains(&k) {
            let rr = r * 0.8 + k * 1.7;
            let n = (rr * 3.0) as i32;
            for i in 0..n {
                if (i + age as i32) % 3 == 0 {
                    continue;
                }
                let a = i as f32 / n as f32 * TAU;
                let ink = if k < 3.0 { 'G' } else if k < 5.5 { 'j' } else { 'g' };
                put(cv, budget, (px + a.cos() * rr).round() as i32, (py + r * 0.45 + a.sin() * rr * 0.4).round() as i32, ink);
            }
        }
        // The pop: a thin ring the size of it, the first two ticks.
        if k < 2.0 {
            let rr = r + 1.0 + k * 2.5;
            let n = (rr * 3.0) as i32;
            for i in (0..n).step_by(2) {
                let a = i as f32 / n as f32 * TAU;
                put(cv, budget, (px + a.cos() * rr).round() as i32, (py + a.sin() * rr * 0.85).round() as i32, if k < 1.0 { 'c' } else { 'V' });
            }
        }
        // A breath of dark smoke going up off it, quickly gone.
        if (1.0..7.0).contains(&k) {
            for m in 0..3 {
                let rise = (k - 1.0) * 1.2 + m as f32 * 1.5;
                let (sx, sy) = (px + (m as f32 - 1.0) * r * 0.45, py - r * 0.2 - rise);
                ball(cv, budget, (sx, sy), 1.2 + (k - 1.0) * 0.2 + r * 0.1, (k - 1.0) / 6.0, ASH);
            }
        }
        // Its pieces, thrown up and out, landing and lying a moment.
        let n = 8 + (r / 3.0).min(8.0) as i32;
        let floor = py + r * 0.45;
        for i in 0..n {
            let h = |s: u32| unit_hash(i, seed as i32, s);
            let up = -PI * 0.5 + (h(1) - 0.5) * 3.2;
            let speed = (1.4 + 2.2 * h(2)) * (0.8 + r / 16.0);
            let (mut gx, mut gy) = (px + up.cos() * r * 0.4, py + up.sin() * r * 0.4);
            // Flight: out, up and down to the floor, where it skids to rest.
            let t = k + 0.5;
            let fly = (speed * (t - 0.04 * t * t)).max(0.0);
            gx += up.cos() * fly;
            gy += up.sin() * fly + 0.5 * 0.42 * t * t;
            let rest = floor + (h(3) - 0.3) * r * 0.5;
            if gy > rest {
                gy = rest;
            }
            if k > 8.0 && h(4) < 0.5 {
                continue;
            }
            let c = colours[i as usize % 3];
            let c = if k > 6.5 { shade(c, 0.6) } else { c };
            let (cx, cy) = (gx.round() as i32, gy.round() as i32);
            set(cv, budget, cx, cy, c);
            if i % 3 == 0 || r > 6.0 {
                set(cv, budget, cx + 1, cy, c);
            }
            set(cv, budget, cx, cy + 1, shade(c, 0.55));
        }
    }
}

// ─── bursts and bombs ────────────────────────────────────────────────────────

/// Smoke's greys: its lit top, its body, its shadow, and as it thins.
const SMOKE_GREY: [char; 4] = ['J', 'G', 'j', 'g'];
/// The darker smoke of a thing that fell.
const ASH: [char; 4] = ['G', 'j', 'g', 'X'];

/// A ball of smoke or dust round `(cx, cy)`, `r` pixels, `k` of the way
/// (0 to 1) through its life: solid and lit on its upper left, darkening as
/// it goes, and breaking up from its rim at the last.
fn ball(cv: &mut Img, budget: &mut Budget, at: (f32, f32), r: f32, k: f32, inks: [char; 4]) {
    flat(cv, budget, at, r, 1.0, k, inks);
}

/// As `ball`, `squash` as deep as it is wide: dust lying low on the floor.
fn flat(cv: &mut Img, budget: &mut Budget, (cx, cy): (f32, f32), r: f32, squash: f32, k: f32, inks: [char; 4]) {
    let ri = r.ceil() as i32;
    let (x0, y0) = (cx.round() as i32, cy.round() as i32);
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let d = ((dx * dx) as f32 + (dy * dy) as f32 / (squash * squash)).sqrt() / r.max(0.5);
            if d > 1.0 {
                continue;
            }
            let (x, y) = (x0 + dx, y0 + dy);
            // At the last it breaks up, from the rim in.
            if k > 0.7 && bayer(x, y) < (k - 0.7) * 3.3 + (d - 1.0) * 0.6 + 0.3 {
                continue;
            }
            let lit = (dx + dy) as f32 / r.max(1.0);
            let ink = if k > 0.75 {
                inks[3]
            } else if k < 0.5 && lit < -0.55 && d > 0.25 {
                inks[0]
            } else if d > 0.8 || lit > 0.55 || k > 0.5 {
                inks[2]
            } else {
                inks[1]
            };
            put(cv, budget, x, y, ink);
        }
    }
}

/// A fireball round `(cx, cy)`, `r` pixels, `phase` 0 (a white-hot pin) to
/// 4 (embers).
fn fireball(cv: &mut Img, (cx, cy): (i32, i32), r: i32, phase: u8) {
    if r <= 0 {
        return;
    }
    let fire = fx::stamp(fx::Stamp::Fireball, r, r, phase);
    cv.stamp(&fire, cx - r, cy - r - r / 3);
}

/// An explosion `size` pixels across round `(px, py)`, `age` ticks in: a
/// few fireballs billowing out of each other, lit one after another, each
/// burning from a white-hot pin down to embers and going up as smoke.
fn explosion(cv: &mut Img, budget: &mut Budget, (px, py): (f32, f32), size: f32, age: u8, seed: u32) {
    let lobes = 3 + (size / 5.0) as i32;
    let t = f32::from(age);
    // Smoke first: what has burned out goes up behind what still burns.
    for i in 0..lobes {
        let h = |s: u32| unit_hash(i, seed as i32, s);
        let start = 3.0 + h(4) * 1.5;
        if t < start {
            continue;
        }
        let a = h(1) * TAU;
        let d = size * 0.3 * (0.3 + h(2));
        let k = ((t - start) / 7.0).min(1.0);
        let at = (px + a.cos() * d * 1.2, py + a.sin() * d * 0.6 - size * 0.3 - (t - start) * 1.1);
        ball(cv, budget, at, size * (0.18 + 0.12 * h(3)) + (t - start) * 0.3, k, SMOKE_GREY);
    }
    for i in 0..lobes {
        let h = |s: u32| unit_hash(i, seed as i32, s);
        // Each lobe lights a moment after the one before it.
        let lit = t - (i as f32 * 0.5).floor();
        if !(0.0..5.0).contains(&lit) {
            continue;
        }
        let a = h(1) * TAU;
        let d = if i == 0 { 0.0 } else { size * 0.3 * (0.3 + h(2)) * (0.6 + 0.2 * lit).min(1.0) };
        let grow = if lit < 1.0 { 0.65 } else if lit < 3.0 { 1.0 } else { 1.0 - (lit - 2.0) * 0.2 };
        let r = (size * (0.28 + 0.14 * h(3)) * grow).round() as i32;
        let at = ((px + a.cos() * d).round() as i32, (py + a.sin() * d * 0.6 - lit * 0.8).round() as i32);
        fireball(cv, at, r, lit as u8);
    }
}

/// Where a handgonne's ball burst: a fireball, a shock ring, cinders and
/// smoke.
fn blasts(cv: &mut Img, run: &Run, budget: &mut Budget) {
    for &(x, y, left) in run.blasts.iter().rev() {
        let age = (BLAST_TICKS - 1).saturating_sub(left);
        let k = f32::from(age);
        let (px, py) = (x * UNIT, y * UNIT);
        let (cx, cy) = (px.floor() as i32, py.floor() as i32);
        let seed = hash(cx, cy, 21);
        if age == 0 {
            fx::glow(cv, cx, cy, fx::Hue::Warm);
        }
        // The shock ring, thin and running ahead of the fire.
        if age <= 5 {
            let r = 5.0 + k * 3.0;
            let n = (r * 3.0) as i32;
            for i in 0..n {
                if (i + i32::from(age)) % 2 == 1 {
                    continue;
                }
                let a = i as f32 / n as f32 * TAU;
                let ink = ['c', '@', 'o', 'o', 'R', 'S'][usize::from(age)];
                put(cv, budget, (px + a.cos() * r).round() as i32, (py + a.sin() * r * 0.65).round() as i32, ink);
            }
        }
        // Cinders thrown up, falling.
        for i in 0..7 {
            let h = |s: u32| unit_hash(i, seed as i32, s);
            let p = Piece {
                o: (px, py - 2.0),
                angle: -PI * 0.5 + (h(1) - 0.5) * 2.6,
                speed: 1.6 + 1.8 * h(2),
                fall: 0.3,
                life: 5.0 + (h(3) * 4.0).floor(),
            };
            piece(cv, budget, p, k + 0.5, ramp(Shot::Ember), '6');
        }
        // The fire over it all.
        explosion(cv, budget, (px, py), 12.0, age, seed);
    }
}

/// A knight's bomb: its wave through the room (a bright edge, a dark ring
/// behind it) and the fire where they stand, behind them: the knight is
/// never lost in their own bomb.
fn bombs(cv: &mut Img, run: &Run, budget: &mut Budget) {
    let feet = if run.side_on() { crate::rules::ledge::FOOT } else { 0.0 };
    for hero in run.players.values().filter(|h| h.hp > 0 && h.privy == 0 && h.bomb_cooldown > BOMB_REARM - 12) {
        let age = BOMB_REARM - hero.bomb_cooldown;
        let k = age as f32;
        let (px, py) = (hero.x * UNIT, (hero.y + feet) * UNIT);
        let (cx, cy) = (px.floor() as i32, py.floor() as i32);
        // The wave: its bright edge, a darker ring a step behind.
        let r = (3.0 + k * 3.2) * UNIT;
        let n = (r * 2.2) as i32;
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU;
            let (ca, sa) = (a.cos(), a.sin() * 0.85);
            let edge = if k < 5.0 { if i % 2 == 0 { '6' } else { '5' } } else if i % 2 == 0 { '@' } else { 'o' };
            put(cv, budget, cx + (ca * r).round() as i32, cy + (sa * r).round() as i32, edge);
            if i % 2 == 0 {
                let back = r - 3.0;
                put(cv, budget, cx + (ca * back).round() as i32, cy + (sa * back).round() as i32, if k < 6.0 { 'R' } else { 'P' });
            }
        }
        // The fire where they stood, going up as it burns down.
        if age < 4 {
            fx::glow(cv, cx, cy - 6, fx::Hue::Warm);
        }
        explosion(cv, budget, (px, py - 4.0), 22.0, age as u8, hash(cx, cy, 33));
    }
}

// ─── muzzles and smoke ───────────────────────────────────────────────────────

/// A crossbow's or handgonne's muzzle: a star along the aim, side spikes,
/// a spark or two thrown on.
fn muzzles(cv: &mut Img, run: &Run, budget: &mut Budget) {
    for p in run.feel.puffs.iter().rev().filter(|p| !p.smoke && p.dust == 0) {
        let age = (FLASH_TICKS - 1).saturating_sub(p.ttl);
        let (px, py) = (p.x * UNIT, p.y * UNIT - 1.0);
        let (x, y) = (px.floor() as i32, py.floor() as i32);
        // A handgonne's flash has smoke beside it: the bigger one.
        let big = run.feel.puffs.iter().any(|o| o.smoke && (o.x - p.x).abs() < 0.3 && (o.y - p.y).abs() < 0.3 && o.ttl + 2 >= SMOKE_TICKS);
        let reach = if big { 6.0 } else { 4.0 };
        let (dx, dy) = (p.dx, p.dy);
        let along = |k: f32, side: f32| ((px + dx * k - dy * side).round() as i32, (py + dy * k + dx * side).round() as i32);
        if age == 0 {
            fx::glow(cv, x, y, fx::Hue::Warm);
            // The spike along the aim, hot at the mouth.
            let mut k = 0.0;
            while k <= reach {
                let (sx, sy) = along(k, 0.0);
                let ink = if k < 1.5 { '9' } else if k < reach * 0.6 { '6' } else { '@' };
                put(cv, budget, sx, sy, ink);
                if big && k < reach * 0.6 {
                    let (sx, sy) = along(k, 1.0);
                    put(cv, budget, sx, sy, '6');
                }
                k += 1.0;
            }
            // Side spikes, forward of the mouth.
            for side in [-1.0f32, 1.0] {
                for s in 1..=(if big { 3 } else { 2 }) {
                    let (sx, sy) = along(s as f32 * 0.7, side * s as f32);
                    put(cv, budget, sx, sy, if s == 1 { '6' } else { '@' });
                }
            }
            chunk(cv, budget, x - 1, y - 1, '9');
        } else {
            // Burning down: a short tongue and sparks flying on.
            for k in 1..=2 {
                let (sx, sy) = along(k as f32 + 1.0, 0.0);
                put(cv, budget, sx, sy, if k == 1 { '@' } else { 'o' });
            }
            for side in [-1.0f32, 1.0] {
                let (sx, sy) = along(reach + 1.0, side * 1.5);
                put(cv, budget, sx, sy, '6');
            }
        }
    }
}

/// A handgonne's smoke: small lit puffs along the aim, swelling a little
/// and thinning out.
fn smoke(cv: &mut Img, run: &Run, budget: &mut Budget) {
    for p in run.feel.puffs.iter().filter(|p| p.smoke) {
        let age = SMOKE_TICKS.saturating_sub(p.ttl);
        let k = f32::from(age) / f32::from(SMOKE_TICKS);
        let (px, py) = (p.x * UNIT, p.y * UNIT - 2.0);
        for m in 0..2 {
            let m = m as f32;
            let at = (px + p.dx * (3.0 + m * 4.0) * (0.6 + k), py + p.dy * (3.0 + m * 4.0) * (0.6 + k) - m * 1.5 - k * 4.0);
            let swell = (k * 4.0).min(1.0);
            ball(cv, budget, at, 1.5 + swell * 2.5 - m * 0.8 + k, k, SMOKE_GREY);
        }
    }
}

// ─── dust ────────────────────────────────────────────────────────────────────

/// Dust's greys, a step darker than smoke: it is the floor's own.
const DUST_GREY: [char; 4] = ['G', 'j', 'g', 'g'];

/// A roll's dust at the knight's feet.
fn dust(cv: &mut Img, run: &Run, budget: &mut Budget) {
    for p in run.feel.puffs.iter().filter(|p| p.dust > 0) {
        let age = (DUST_TICKS - 1).saturating_sub(p.ttl);
        let k = f32::from(age) / f32::from(DUST_TICKS);
        let t = f32::from(age);
        let (px, py) = (p.x * UNIT, p.y * UNIT);
        let (dx, dy) = (p.dx, p.dy);
        // Across the roll, on the floor.
        let (ax, ay) = (-dy, dx * 0.4);
        let seed = hash(px as i32, py as i32, u32::from(p.dust));
        match p.dust {
            KICK => {
                // Kicked up behind, low and spreading.
                for m in 0..3 {
                    let m = m as f32;
                    let back = 2.0 + m * 2.5 + t * (0.4 + m * 0.2);
                    let side = if m == 1.0 { 0.0 } else { (m - 1.0) * 1.5 };
                    let at = (px - dx * back + ax * side, py - dy * back * 0.6 + ay * side - 1.0 - t * 0.15);
                    flat(cv, budget, at, 1.6 + t * 0.2 - m * 0.2, 0.65, k, DUST_GREY);
                }
                // Grit thrown back.
                for i in 0..4 {
                    let h = |s: u32| unit_hash(i, seed as i32, s);
                    let p = Piece {
                        o: (px, py - 1.0),
                        angle: (-dy).atan2(-dx) + (h(1) - 0.5) * 1.8 - 0.4,
                        speed: 1.0 + h(2),
                        fall: 0.2,
                        life: 4.0 + (h(3) * 3.0).floor(),
                    };
                    piece(cv, budget, p, t + 0.5, Ramp { inks: ['J', 'G', 'G', 'j', 'g'], solid: true, gas: false }, 'J');
                }
            }
            LAND => {
                // Thrown out round the feet where the knight comes up.
                for m in 0..4 {
                    let a = m as f32 / 4.0 * TAU + 0.5;
                    let spread = 3.0 + t * 0.7;
                    let at = (px + dx * 1.5 + a.cos() * spread, py + a.sin() * spread * 0.4 - 1.0);
                    flat(cv, budget, at, 1.3 + t * 0.15, 0.6, k, DUST_GREY);
                }
            }
            _ => {
                // A wisp left along the roll.
                flat(cv, budget, (px - dx * 2.0, py - 1.0), 1.4 + t * 0.12, 0.5, k.max(0.3), DUST_GREY);
            }
        }
    }
}

// ─── finds ───────────────────────────────────────────────────────────────────

/// A find's colours by its rarity: common ivory, rare teal, relic gold.
fn find_inks(tier: u8) -> [char; 4] {
    match tier {
        2 => ['9', '6', '@', 'o'],
        1 => ['3', '2', '1', 'q'],
        _ => ['c', 'V', 'U', 'u'],
    }
}

/// Every find just taken off the floor: a star, a ring and rising motes.
fn glints(cv: &mut Img, run: &Run, budget: &mut Budget) {
    for g in run.feel.glints.iter().rev() {
        let age = (GLINT_TICKS - 1).saturating_sub(g.ttl);
        let t = f32::from(age);
        let [hot, core, rim, cool] = find_inks(g.tier);
        let (px, py) = (g.x * UNIT, g.y * UNIT);
        let (x, y) = (px.floor() as i32, py.floor() as i32);
        // A star where it lay, shrinking.
        if age <= 3 {
            let reach = 5 - i32::from(age) + i32::from(g.tier);
            for r in 1..=reach {
                let ink = if r <= 1 { hot } else if r < reach { core } else { rim };
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    put(cv, budget, x + dx * r, y - 4 + dy * r, ink);
                }
            }
            if age <= 1 {
                chunk(cv, budget, x - 1, y - 5, hot);
            }
        }
        // A ring opening on the floor.
        if age <= 7 {
            let r = 3.0 + t * 1.5;
            for i in 0..14 {
                if (i + i32::from(age)) % 2 == 1 {
                    continue;
                }
                let a = i as f32 / 14.0 * TAU;
                put(cv, budget, (px + a.cos() * r).round() as i32, (py + a.sin() * r * 0.45).round() as i32, if age < 4 { rim } else { cool });
            }
        }
        // Motes rising in a turn round where it was.
        for m in 0..(5 + 2 * i32::from(g.tier)) {
            let start = (m % 3) as f32;
            if t < start {
                continue;
            }
            let lift = (t - start) * 1.4 + 2.0;
            let a = m as f32 * 1.3 + t * 0.45;
            let (mx, my) = (px + a.cos() * (3.0 + m as f32 * 0.5), py - 2.0 - lift);
            let k = (t - start) / (f32::from(GLINT_TICKS) - start);
            let ink = if k < 0.3 { hot } else if k < 0.6 { core } else if k < 0.85 { rim } else { cool };
            put(cv, budget, mx.round() as i32, my.round() as i32, ink);
        }
    }
}

/// A rare or relic card on the floor catches the light now and then: a
/// small star at its corner.
fn twinkles(cv: &mut Img, run: &Run, budget: &mut Budget) {
    for item in &run.room().items {
        let Some(card) = run.book.get(&item.card) else {
            continue;
        };
        let ink = match card.rarity {
            crate::rules::cards::Rarity::Relic => '9',
            crate::rules::cards::Rarity::Rare => '3',
            _ => continue,
        };
        let (x, y) = (item.x * UNIT, item.y * UNIT);
        let (x, y) = (x.floor() as i32 + 6, y.floor() as i32 - 9);
        let phase = (run.tick as u32 + hash(x, y, 5) % 70) % 70;
        let size = match phase {
            0 | 4 => 1,
            1 | 3 => 2,
            2 => 3,
            _ => continue,
        };
        for r in 1..=size {
            let c = if r == size { '2' } else { ink };
            let c = if ink == '9' && r == size { '6' } else { c };
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                put(cv, budget, x + dx * r, y + dy * r, c);
            }
        }
        put(cv, budget, x, y, ink);
    }
}

/// The halo a burst of `kind` lifts the dark round it with.
fn glow_hue(kind: Shot) -> fx::Hue {
    match kind {
        Shot::Ember | Shot::Flame | Shot::Sacred | Shot::Ball | Shot::Missile | Shot::Ore => fx::Hue::Warm,
        Shot::Hex | Shot::Spore => fx::Hue::Gas,
        _ => fx::Hue::Cold,
    }
}

/// Ticks a block's light lasts in a window.
pub(super) const BLOCK_LIGHT: u8 = 3;

/// The window's light off each fresh block: where, how strong, its hue.
pub(super) fn block_lights(run: &Run) -> impl Iterator<Item = (f32, f32, f32, super::room_scene::Hue)> + '_ {
    run.feel.blocks.iter().filter(|b| b.age() < BLOCK_LIGHT).map(move |b| {
        let lift = shield_lift(run, b);
        let k = 1.0 - f32::from(b.age()) / f32::from(BLOCK_LIGHT);
        let hue = match glow_hue(b.kind) {
            fx::Hue::Warm => super::room_scene::Hue::Fire,
            fx::Hue::Gas => super::room_scene::Hue::Gas,
            fx::Hue::Cold => super::room_scene::Hue::Cold,
        };
        (b.x * UNIT, b.y * UNIT - lift, 0.35 + 0.15 * f32::from(b.weight) * k, hue)
    })
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__vfx__tests.rs"]
mod tests;
