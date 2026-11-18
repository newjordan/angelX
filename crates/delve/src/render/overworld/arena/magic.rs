//! Magic on the field: every spell from its cast to its last ember, and
//! the wish moments — the genie's lamp rubbed, a wish landing on its
//! knight, the fairies paying out in the Gnome's Garden.
//!
//! A spell is drawn from what the run already holds: its slot's recharge
//! says how many ticks ago it was cast (`Hero::spell_wait` less what is
//! left), its shots are the ones marked with its slot (`ShotTraits::spell`,
//! so a knight's own bolt loosed in the same breath is never taken for
//! the spell's), and where they broke is in the shot feedback, which
//! carries the same mark (`rules::ranged`).
//! Each cast has its beats: a rune circle opening under the caster with an
//! overshoot while power is drawn up out of it, the release at the hand,
//! what the spell is (a comet of a bolt, a ring of sparks, a falling star,
//! a nova, a mending, a ward), and its landing with what is thrown after.
//! Its colours are its own words' (`rules::vfx`): a firebolt burns, a frost
//! nova is ice, a mending light is gold.
//!
//! Nothing here is kept between frames or rolled: the same run, the same
//! pixels, on the host, a mirror and a browser guest alike.

use super::pfx::{self, Beats, Pen, Ramp, Shape, Spray, ease, ramps, unit};
use super::{UNIT, ranged_events, wishfx};
use crate::rules::cards::{Card, Effect, Shape as Cast};
use crate::rules::vfx::When;
use crate::rules::{HZ, Hero, Run, Shot};
use std::f32::consts::{PI, TAU};

const BUDGET: u32 = 1600;
const CAST_TICKS: u32 = 40;
const WARD_TICKS: u32 = 3 * HZ;
const LANDED_TICKS: u64 = 75;
const RUB_TICKS: u64 = 60;
const FAIRY_TICKS: u64 = 120;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bolt,
    Ring,
    Meteor,
    Nova,
    Heal,
    Ward,
    Other,
}

fn kind(card: &Card) -> Kind {
    for e in &card.effects {
        match e {
            Effect::Cast(Cast::Bolt) => return Kind::Bolt,
            Effect::Cast(Cast::Ring) => return Kind::Ring,
            Effect::Cast(Cast::Meteor) => return Kind::Meteor,
            _ => {}
        }
    }
    for e in &card.effects {
        match e {
            Effect::Nova(_) => return Kind::Nova,
            Effect::Heal(_) | Effect::Censer(_) => return Kind::Heal,
            Effect::Ward(_) | Effect::Immune(_) => return Kind::Ward,
            _ => {}
        }
    }
    Kind::Other
}

fn colours(card: &Card) -> Ramp {
    let recipe = wishfx::recipe(card);
    let p = recipe.first(When::Cast).or_else(|| recipe.layers.first()).map(|l| l.palette);
    p.map_or(ramps::ARCANE, wishfx::ramp)
}

struct Casting<'a> {
    id: u32,
    hero: &'a Hero,
    card: &'a Card,
    slot: usize,
    age: u32,
}

impl Casting<'_> {
    fn mark(&self) -> u8 {
        self.slot as u8 + 1
    }
}

fn castings(run: &Run) -> Vec<Casting<'_>> {
    let mut out = Vec::new();
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && h.privy == 0 && !h.absent) {
        for slot in 0..crate::rules::cards::SPELL_SLOTS {
            if hero.module_spells[slot].is_some() || hero.spell_cooldowns[slot] == 0 {
                continue;
            }
            let Some(card) = hero.spells[slot].as_deref().and_then(|c| run.book.get(c)) else { continue };
            let Some(age) = hero.spell_wait(card).checked_sub(hero.spell_cooldowns[slot]) else { continue };
            let window = if kind(card) == Kind::Ward { WARD_TICKS } else { CAST_TICKS };
            if age < window && out.len() < 16 {
                out.push(Casting { id, hero, card, slot, age });
            }
        }
    }
    out
}

fn seed(id: u32, card: &Card, salt: u32) -> u32 {
    card.id.bytes().fold(id.wrapping_mul(0x9e37_79b9) ^ salt, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

fn hand(run: &Run, hero: &Hero) -> (f32, f32) {
    let (_, mid) = wishfx::body(run, hero);
    let l = hero.aim_x.hypot(hero.aim_y);
    let (ux, uy) = if l > 0.01 { (hero.aim_x / l, hero.aim_y / l) } else { (1.0, 0.0) };
    (mid.0 + ux * 6.0, mid.1 + uy * 4.0 - 1.0)
}

pub(super) fn under(cv: &mut super::super::ink::Img, run: &Run) {
    let mut pen = Pen::new(cv, BUDGET / 2);
    for c in castings(run) {
        sigil(&mut pen, run, &c);
    }
    spell_scorches(&mut pen, run);
    landed_floor(&mut pen, run);
    fairy_floor(&mut pen, run);
}

pub(super) fn over(cv: &mut super::super::ink::Img, run: &Run) {
    let mut pen = Pen::new(cv, BUDGET);
    for c in castings(run) {
        release(&mut pen, run, &c);
        match kind(c.card) {
            Kind::Bolt | Kind::Ring | Kind::Meteor => comets(&mut pen, run, &c),
            Kind::Nova => nova(&mut pen, run, &c),
            Kind::Heal => mending(&mut pen, run, &c),
            Kind::Ward => ward(&mut pen, run, &c),
            Kind::Other => {}
        }
        if kind(c.card) == Kind::Ring {
            spark_ring(&mut pen, run, &c);
        }
    }
    spell_hits(&mut pen, run);
    blast_waves(&mut pen, run);
    thunder(&mut pen, run);
    lamp(&mut pen, run);
    landed(&mut pen, run);
    fairies(&mut pen, run);
}

fn sigil(pen: &mut Pen, run: &Run, c: &Casting) {
    if kind(c.card) == Kind::Ward && c.age >= CAST_TICKS {
        return;
    }
    if kind(c.card) == Kind::Ward {
        let (feet, _) = wishfx::body(run, c.hero);
        let age = c.age as f32;
        let open = ease::back((age / 8.0).min(1.0));
        let fade = ((age - 16.0) / 24.0).clamp(0.0, 1.0);
        pen.glyph((feet.0, feet.1 + 1.0), 14.0 * open.max(0.3), 0.42, -age * 0.05, open, 6, colours(c.card), fade, seed(c.id, c.card, 5));
        return;
    }
    let ramp = colours(c.card);
    let (feet, _) = wishfx::body(run, c.hero);
    let beats = Beats::new(0.0, 6.0, 6.0, 24.0);
    let age = c.age as f32;
    if age >= beats.total() {
        return;
    }
    let big = if matches!(kind(c.card), Kind::Nova | Kind::Meteor) { 15.0 } else { 12.0 };
    let r = big * beats.size(age);
    let open = ease::back((age / 7.0).min(1.0)).min(1.0);
    let fade = ((age - 14.0) / 22.0).clamp(0.0, 1.0);
    let rot = age * 0.07 + unit(c.id as i32, 2, 0x51) * TAU;
    let sides = 5 + (seed(c.id, c.card, 0) % 2);
    pen.glow((feet.0, feet.1 + 1.0), r + 4.0, 0.42, ramp.cooler(2), 0.5 * (1.0 - fade));
    pen.glyph((feet.0, feet.1 + 1.0), r, 0.42, rot, open, sides, ramp, fade, seed(c.id, c.card, 1));
}

fn release(pen: &mut Pen, run: &Run, c: &Casting) {
    if c.age >= 16 {
        return;
    }
    let ramp = colours(c.card);
    let at = hand(run, c.hero);
    let (feet, _) = wishfx::body(run, c.hero);
    let age = c.age as f32;
    for i in 0..10u32 {
        let start = i as f32 * 0.5;
        let t = (age - start) / 8.0;
        if !(0.0..1.0).contains(&t) {
            continue;
        }
        let a = i as f32 / 10.0 * TAU + unit(i as i32, c.id as i32, 0x21) * 0.6;
        let from = (feet.0 + a.cos() * 12.0, feet.1 + 1.0 + a.sin() * 5.0);
        let e = ease::inward(t);
        let p = (from.0 + (at.0 - from.0) * e, from.1 + (at.1 - from.1) * e - ease::bump(t) * 4.0);
        let dir = (at.0 - from.0, at.1 - from.1);
        pen.streak(p, dir, 2.0 + e * 3.0, ramp, 0.1 + 0.4 * (1.0 - t));
    }
    if pen.calm && c.age > 3 {
        return;
    }
    let k = age / 9.0;
    if k < 1.0 {
        let p = 1.0 - ease::out(k) * 0.8;
        pen.glow(at, 7.0 + 3.0 * ease::bump(k * 0.8), 1.0, ramp, p);
        pfx::rays(pen, at, 8, 10.0, 1.0, age * 0.05 + 0.4, ramp, k);
    }
}

fn comets(pen: &mut Pen, run: &Run, c: &Casting) {
    let k = kind(c.card);
    let shot = if k == Kind::Meteor { Shot::Ball } else { Shot::Bolt };
    let ramp = colours(c.card);
    let recipe = wishfx::recipe(c.card);
    let wobble = recipe.first(When::Trail).map_or(0.0, |l| f32::from(l.wobble));
    for p in run.projectiles.iter().filter(|p| {
        !p.hostile && u32::from(p.traits.owner) == c.id && p.kind == shot && p.traits.spell == c.mark()
    }) {
        let head = (p.x * UNIT, p.y * UNIT);
        let v = p.vx.hypot(p.vy).max(0.1);
        let (ux, uy) = (p.vx / v, p.vy / v);
        let (nx, ny) = (-uy, ux);
        let per = v * UNIT / HZ as f32;
        let flown = c.age as f32 * per;
        let big = k == Kind::Meteor;
        let n = if big { 14 } else if k == Kind::Ring { 5 } else { 10 };
        let gap = if big { 2.2 } else { 2.0 };
        let mut pts = [(0.0f32, 0.0f32); 14];
        for (j, pt) in pts.iter_mut().enumerate().take(n) {
            let back = (j as f32 * gap).min(flown);
            let along = flown - back;
            let sway = (along * 0.3 + c.id as f32).sin() * wobble.min(3.0) * (j as f32 / n as f32);
            *pt = (head.0 - ux * back + nx * sway, head.1 - uy * back + ny * sway);
        }
        pen.ribbon(&pts[..n], ramp, if big { 2.8 } else if k == Kind::Ring { 1.0 } else { 2.2 });
        pen.glow(head, if big { 11.0 } else if k == Kind::Ring { 4.5 } else { 7.5 }, 1.0, ramp, 1.0);
        let core = if big { 2 } else if k == Kind::Ring { 0 } else { 1 };
        let (hx, hy) = (head.0.round() as i32, head.1.round() as i32);
        for dy in -core..=core {
            for dx in -core..=core {
                if dx * dx + dy * dy <= core * core + core / 2 {
                    pen.light(hx + dx, hy + dy, if dx == 0 && dy == 0 { ramp.step(0) } else { ramp.step(1) });
                }
            }
        }
        let shed = if big { 6 } else { 3 };
        for i in 0..shed {
            let age = (c.age + i * 2) % 8;
            let from_t = c.age.saturating_sub(age) as f32;
            let o = (head.0 - ux * (c.age as f32 - from_t) * per, head.1 - uy * (c.age as f32 - from_t) * per);
            let s = Spray {
                seed: seed(c.id, c.card, 40 + i + c.age.saturating_sub(age) * 7),
                n: 1,
                at: o,
                angle: (-uy).atan2(-ux),
                spread: 1.6,
                speed: (0.4, 1.2),
                life: (6.0, 9.0),
                drag: 0.2,
                gravity: if big { 0.12 } else { 0.05 },
                squash: 1.0,
                stagger: 0.0,
            };
            s.each(age as f32, |m| pfx::mote(pen, &m, Shape::Ember, ramp.cooler(1), 1, run.tick));
        }
        if big {
            for i in 0..8u32 {
                let back = 8.0 + i as f32 * 5.0;
                if back > flown {
                    continue;
                }
                let ago = back / per;
                let k = ago / 26.0;
                if k >= 1.0 {
                    continue;
                }
                let m = pfx::Mote { i, at: (head.0 - ux * back, head.1 - uy * back - ago * 0.4), vel: (0.0, -0.4), k, seed: i };
                pfx::mote(pen, &m, Shape::Smoke, ramps::SMOKE.cooler(1), 2, run.tick);
            }
        }
    }
}

fn spark_ring(pen: &mut Pen, run: &Run, c: &Casting) {
    let ramp = colours(c.card);
    let (_, mid) = wishfx::body(run, c.hero);
    let age = c.age as f32;
    let per = 14.0 * UNIT / HZ as f32;
    let r = 4.0 + age * per;
    let fade = (age / 22.0).clamp(0.0, 1.0);
    pen.shock(mid, r, 1.6, 0.8, ramp, ease::inward(fade));
    if age < 8.0 {
        pen.ring(mid, r * 0.6, 1.0, 0.8, ramp.cooler(1), age / 8.0);
    }
}

fn nova(pen: &mut Pen, run: &Run, c: &Casting) {
    let ramp = colours(c.card);
    let (feet, mid) = wishfx::body(run, c.hero);
    let age = c.age as f32;
    let beats = Beats::new(0.0, 5.0, 5.0, 20.0);
    if age >= beats.total() {
        return;
    }
    let reach = 70.0;
    let r = reach * ease::out(age / 16.0);
    let fade = ease::inward((age / 26.0).min(1.0));
    pen.shock((feet.0, feet.1 - 2.0), r, 3.0, 0.62, ramp, fade);
    if age < 10.0 {
        pen.ring((feet.0, feet.1 - 2.0), r * 0.55, 2.0, 0.62, ramp.cooler(1), age / 10.0);
        pen.glow(mid, 16.0 * beats.size(age), 0.9, ramp, beats.power(age));
    }
    let s = Spray {
        seed: seed(c.id, c.card, 7),
        n: if pen.calm { 12 } else { 24 },
        at: mid,
        angle: 0.0,
        spread: TAU,
        speed: (2.0, 4.5),
        life: (12.0, 22.0),
        drag: 0.1,
        gravity: 0.05,
        squash: 0.65,
        stagger: 0.0,
    };
    let sh = match colours(c.card) {
        r if r == ramps::FROST => Shape::Shard,
        r if r == ramps::FIRE => Shape::Spark,
        _ => Shape::Spark,
    };
    s.each(age, |m| pfx::mote(pen, &m, sh, ramp, 2, run.tick));
}

fn mending(pen: &mut Pen, run: &Run, c: &Casting) {
    let ramp = colours(c.card);
    let (feet, mid) = wishfx::body(run, c.hero);
    let age = c.age as f32;
    let k = age / CAST_TICKS as f32;
    if age < 14.0 {
        let r = 20.0 * (1.0 - ease::out(age / 14.0)) + 3.0;
        pen.ring((feet.0, feet.1 + 1.0), r, 1.4, 0.45, ramp, age / 14.0 * 0.5);
    }
    if age < 12.0 {
        let t = age / 12.0;
        let top = mid.1 - 46.0;
        let low = top + (feet.1 - top) * ease::out((t * 2.0).min(1.0));
        let half = 3.0 * (1.0 - t) + 0.5;
        let mut y = top;
        while y < low {
            for dx in -(half.ceil() as i32)..=(half.ceil() as i32) {
                let x = mid.0.round() as i32 + dx;
                let yy = y.round() as i32;
                let edge = dx.abs() as f32 / half.max(0.5);
                if super::super::ink::bayer(x, yy) > (1.0 - edge) * (1.0 - t * 0.6) {
                    continue;
                }
                pen.light(x, yy, ramp.at(edge * 0.5 + t * 0.4));
            }
            y += 1.0;
        }
    }
    pen.glow(mid, 11.0, 1.2, ramp.cooler(1), 0.75 * ease::bump(k.min(1.0)));
    let s = Spray {
        seed: seed(c.id, c.card, 3),
        n: if pen.calm { 8 } else { 16 },
        at: (feet.0, feet.1),
        angle: -PI / 2.0,
        spread: 2.2,
        speed: (0.2, 0.7),
        life: (18.0, 30.0),
        drag: 0.05,
        gravity: -0.035,
        squash: 1.0,
        stagger: 0.8,
    };
    s.each(age, |mut m| {
        m.at.0 += (unit(m.i as i32, 3, 0x11) - 0.5) * 14.0 + (age * 0.2 + m.i as f32).sin();
        pfx::mote(pen, &m, if m.i % 3 == 0 { Shape::Star } else { Shape::Dot }, ramp, 1, run.tick);
    });
    for i in 0..4u32 {
        let t = (age - 4.0 - i as f32 * 5.0) / 10.0;
        if !(0.0..1.0).contains(&t) {
            continue;
        }
        let a = unit(i as i32, c.id as i32, 0x31) * TAU;
        let at = (mid.0 + a.cos() * 8.0, mid.1 - 4.0 + a.sin() * 5.0 - t * 8.0);
        let (x, y) = (at.0.round() as i32, at.1.round() as i32);
        let ch = ramp.at(t * 0.7);
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            pen.light(x + dx, y + dy, ch);
        }
    }
}

fn ward(pen: &mut Pen, run: &Run, c: &Casting) {
    let ramp = colours(c.card);
    let (_, mid) = wishfx::body(run, c.hero);
    let age = c.age as f32;
    let centre = (mid.0, mid.1 + 1.0);
    if c.age < CAST_TICKS {
        let size = ease::back((age / 8.0).min(1.0));
        let fade = ((age - 16.0) / 24.0).clamp(0.0, 1.0);
        pen.ring(centre, 13.0 * size, 1.0, 1.0, ramp, 0.15 + fade * 0.6);
        for i in 0..14 {
            let a = PI * 1.05 + i as f32 * 0.07;
            let rr = 11.5 * size;
            pen.light((centre.0 + a.cos() * rr).round() as i32, (centre.1 + a.sin() * rr).round() as i32, ramp.at(0.1 + fade * 0.6));
        }
        if age < 6.0 {
            pen.ring(centre, 13.0 + age * 3.0, 1.4, 1.0, ramp.cooler(1), age / 6.0);
        }
    }
    if c.hero.invulnerable > 0 {
        let t = run.tick as f32;
        let n = 44;
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU;
            if (i + (run.tick / 3) as i32) % 4 != 0 {
                continue;
            }
            pen.light((centre.0 + a.cos() * 13.0).round() as i32, (centre.1 + a.sin() * 13.0).round() as i32, ramp.step(3));
        }
        let g = t * 0.12;
        pen.twinkle((centre.0 + g.cos() * 13.0, centre.1 + g.sin() * 13.0), 2.0, (t * 0.05) % 1.0, ramp);
    }
}

fn spell_hits(pen: &mut Pen, run: &Run) {
    use ranged_events::Mark;
    let casts = castings(run);
    ranged_events::each(run, |e| {
        if e.hostile || e.mark == Mark::Release {
            return;
        }
        let Some(c) = casts.iter().find(|c| {
            c.id == u32::from(e.owner)
                && e.spell == c.mark()
                && u32::from(e.age()) <= c.age
                && matches!((kind(c.card), e.kind), (Kind::Bolt | Kind::Ring, Shot::Bolt) | (Kind::Meteor, Shot::Ball))
        }) else {
            return;
        };
        let ramp = colours(c.card);
        let at = (e.x * UNIT, e.y * UNIT);
        let age = f32::from(e.age());
        let k = kind(c.card);
        let big = if k == Kind::Meteor { 2.2 } else if k == Kind::Ring { 0.7 } else { 1.0 };
        let s = seed(c.id, c.card, super::super::ink::hash((e.x * 8.0) as i32, (e.y * 8.0) as i32, 3));
        if age < 5.0 {
            pen.glow(at, 8.0 * big, 1.0, ramp, 1.0 - age / 5.0);
        }
        if k != Kind::Ring {
            pfx::rays(pen, at, 8, 9.0 * big, 0.8, unit(s as i32, 1, 1) * TAU, ramp, age / (7.0 + 3.0 * big));
            pen.shock(at, 3.0 + ease::out(age / 12.0) * 12.0 * big, 1.2 + big * 0.5, 0.65, ramp, ease::inward((age / 14.0).min(1.0)));
        }
        let spray = Spray {
            seed: s,
            n: (10.0 * big) as u32,
            at,
            angle: (-e.dy).atan2(-e.dx),
            spread: if big > 1.5 { TAU } else { 2.6 },
            speed: (1.4 * big.sqrt(), 3.6 * big.sqrt()),
            life: (8.0, 16.0),
            drag: 0.12,
            gravity: 0.12,
            squash: 0.8,
            stagger: 0.0,
        };
        spray.each(age, |m| pfx::mote(pen, &m, Shape::Spark, ramp, 1, run.tick));
        if big >= 1.0 {
            for i in 0..(big * 2.0) as u32 {
                let off = ((unit(i as i32, s as i32, 2) - 0.5) * 10.0 * big, (unit(i as i32, s as i32, 3) - 0.5) * 4.0);
                pen.wisp((at.0 + off.0, at.1 + off.1), age, (2.0, 9.0), 16.0, 10.0 * big.sqrt(), 2.5, ramps::SMOKE.cooler(1), s ^ i);
            }
        }
    });
}

fn blast_waves(pen: &mut Pen, run: &Run) {
    use crate::rules::BLAST_TICKS;
    for &(x, y, left) in run.blasts.iter().rev().take(8) {
        let age = f32::from((BLAST_TICKS - 1).saturating_sub(left));
        let k = age / f32::from(BLAST_TICKS);
        let at = (x * UNIT, y * UNIT);
        pen.shock((at.0, at.1 + 1.0), 4.0 + ease::out(k) * 22.0, 1.6, 0.55, ramps::EMBER, ease::inward(k));
        let s = super::super::ink::hash((x * 8.0) as i32, (y * 8.0) as i32, 0xb1);
        let spray = Spray { gravity: 0.2, angle: -PI / 2.0, spread: 2.2, ..Spray::round(s, if pen.calm { 5 } else { 10 }, (at.0, at.1 - 3.0), (1.5, 3.5), (7.0, 12.0)) };
        spray.each(age, |m| pfx::mote(pen, &m, Shape::Spark, ramps::EMBER, 1, run.tick));
    }
}

fn spell_scorches(pen: &mut Pen, run: &Run) {
    use ranged_events::Mark;
    let casts = castings(run);
    ranged_events::each(run, |e| {
        if e.hostile || e.mark == Mark::Release || e.kind != Shot::Ball {
            return;
        }
        let Some(c) = casts.iter().find(|c| c.id == u32::from(e.owner) && e.spell == c.mark() && kind(c.card) == Kind::Meteor && u32::from(e.age()) <= c.age) else {
            return;
        };
        let ramp = colours(c.card);
        let age = f32::from(e.age());
        let at = (e.x * UNIT, e.y * UNIT + 2.0);
        let fade = (age / 20.0).clamp(0.0, 1.0);
        pen.glyph(at, 16.0 * ease::back((age / 5.0).min(1.0)), 0.45, age * 0.02, 1.0, 5, ramp.cooler(1), 0.2 + fade * 0.8, seed(c.id, c.card, 77));
    });
}

fn thunder(pen: &mut Pen, run: &Run) {
    use crate::rules::feel::BOLT_TICKS;
    for (n, b) in run.feel.bolts.iter().enumerate() {
        let age = f32::from(b.age());
        let at = (b.x * UNIT, b.y * UNIT);
        let s = super::super::ink::hash((b.x * 8.0) as i32, (b.y * 8.0) as i32, 0x7b) ^ n as u32;
        if b.call {
            let open = ease::back((age / 6.0).min(1.0));
            pen.glyph((at.0, at.1 + 1.0), 13.0 * open, 0.42, age * 0.09, open, 6, ramps::STORM, (age / f32::from(BOLT_TICKS)).powi(2), s);
            continue;
        }
        if age < 2.0 || (3.0..5.0).contains(&age) {
            let flick = (run.tick / 2) as u32;
            let top = (at.0 + (unit(s as i32, flick as i32, 1) - 0.5) * 24.0, at.1 - 78.0);
            pen.arc(top, (at.0, at.1 - 3.0), s ^ flick, 6.0, ramps::STORM, 0.0);
        }
        if age < 12.0 {
            pen.shock((at.0, at.1 + 1.0), 3.0 + ease::out(age / 10.0) * 14.0, 1.2, 0.5, ramps::STORM, age / 12.0);
        }
        let spray = Spray::round(s, if pen.calm { 4 } else { 8 }, (at.0, at.1 - 2.0), (1.0, 3.0), (6.0, 12.0));
        let spray = Spray { gravity: 0.12, angle: -PI / 2.0, spread: 2.8, ..spray };
        spray.each(age, |m| pfx::mote(pen, &m, Shape::Spark, ramps::STORM, 1, run.tick));
    }
}

const GENIE: [&str; 34] = [
    ".........##.........",
    "........#++#........",
    ".......######.......",
    "......#++++++#......",
    ".....#++++++++#.....",
    ".....#++o++o++#.....",
    ".....#++++++++#.....",
    "......#++++++#......",
    ".......#++++#.......",
    "....####++++####....",
    "..##++++++++++++##..",
    ".#++++++++++++++++#.",
    "#++##++++++++++##++#",
    "#+#..#++++++++#..#+#",
    "#+#..#++++++++#..#+#",
    "#++#.##########.#++#",
    ".#+++##++++++##+++#.",
    "..##++++++++++++##..",
    "....#++++++++++#....",
    ".....#++++++++#.....",
    ".....#+++++++#......",
    "......#++++++#......",
    "......#+++++#.......",
    ".......#++++#.......",
    ".......#+++#........",
    "........#+++#.......",
    ".........#+++#......",
    "..........#++#......",
    "..........#++#......",
    ".........#++#.......",
    "........#++#........",
    "........#+#.........",
    ".........#+#........",
    "..........#.........",
];

fn genie(pen: &mut Pen, base: (f32, f32), show: f32, tick: u64) {
    if show <= 0.02 {
        return;
    }
    let h = GENIE.len() as i32;
    let (x0, y0) = (base.0.round() as i32 - 10, base.1.round() as i32 - h + 1);
    let t = tick as f32;
    let risen = (show * 1.25 * h as f32) as i32;
    let band = (h as f32 - (t * 0.6) % (h as f32 + 10.0)) as i32;
    for (dy, row) in GENIE.iter().enumerate() {
        let dy = dy as i32;
        if h - dy > risen {
            continue;
        }
        let fresh = ((risen - (h - dy)) as f32 / 4.0).min(1.0) * show.min(1.0);
        let sway = ((dy as f32 * 0.45 - t * 0.22).sin() * (dy as f32 / h as f32).powi(2) * 2.0).round() as i32;
        for (dx, c) in row.chars().enumerate() {
            let (x, y) = (x0 + dx as i32 + sway, y0 + dy);
            let thr = super::super::ink::bayer(x, y + (tick / 2) as i32);
            let lit = (dy - band).abs() <= 1;
            match c {
                '#' if thr < fresh => pen.light(x, y, if lit { 'w' } else if (dy + dx as i32) % 6 == 0 { '2' } else { '3' }),
                '+' if thr < fresh * 0.7 => pen.light(x, y, if lit { '3' } else if dy < 9 { '1' } else if thr < 0.3 { '1' } else { 'q' }),
                'o' if fresh > 0.5 => pen.light(x, y, if (tick / 24) % 6 == 0 { '2' } else { 'w' }),
                _ => {}
            }
        }
    }
}

fn lamp(pen: &mut Pen, run: &Run) {
    let Some(spout) = super::altar::spout(run) else { return };
    let Some(rubbed) = run.lamp.rubbed.values().filter(|&&t| run.tick.saturating_sub(t) < RUB_TICKS).max().copied() else {
        return;
    };
    let age = run.tick.saturating_sub(rubbed) as f32;
    let s = (rubbed as u32).wrapping_mul(0x9e37);
    for i in 0..3u32 {
        let t = (age - i as f32 * 3.0) / 6.0;
        let at = (spout.0 - 5.0 + i as f32 * 3.0, spout.1 + 2.0 - (i % 2) as f32);
        pen.twinkle(at, 2.0, t, ramps::HOLY);
    }
    for i in 0..3u32 {
        let base = (spout.0 + 1.0 + i as f32, spout.1 - 1.0);
        pen.wisp(base, age - 3.0 - i as f32 * 2.0, (0.0, 26.0), 20.0, 16.0 + i as f32 * 5.0, 2.5 + i as f32, ramps::GENIE.cooler(1), s ^ i);
    }
    let show = if age < 14.0 {
        0.0
    } else if age < 24.0 {
        ease::out((age - 14.0) / 10.0)
    } else if age < 40.0 {
        1.0
    } else {
        1.0 - ease::both((age - 40.0) / 20.0)
    };
    let base = (spout.0 + 1.0, spout.1 - 2.0);
    if show > 0.0 {
        pen.glow((base.0, base.1 - 20.0), 18.0, 1.3, ramps::GENIE.cooler(2), 0.45 * show);
    }
    genie(pen, base, show, run.tick);
    let k = (age - 18.0) / 16.0;
    if (0.0..1.0).contains(&k) {
        let at = (base.0, base.1 - 26.0);
        pfx::rays(pen, at, 8, 12.0, 1.0, 0.3, ramps::GENIE, k);
        let spray = Spray::round(s ^ 0x5, if pen.calm { 6 } else { 12 }, at, (1.0, 2.6), (10.0, 18.0));
        spray.each(age - 18.0, |m| pfx::mote(pen, &m, Shape::Star, if m.i % 2 == 0 { ramps::GENIE } else { ramps::HOLY }, 1, run.tick));
    }
}

fn granted_tier(run: &Run, hero: &Hero, name: &str) -> u32 {
    let held = hero.deck.iter().map(String::as_str).chain(hero.spells.iter().flatten().map(String::as_str)).chain(hero.arm.as_deref());
    held.filter_map(|id| run.book.get(id)).filter(|c| c.name == name).filter_map(|c| crate::rules::vfx::tier(&c.id)).max().unwrap_or(1)
}

fn landed(pen: &mut Pen, run: &Run) {
    for l in run.lamp.landed.iter().rev().take(4) {
        let age = run.tick.saturating_sub(l.tick);
        if age >= LANDED_TICKS {
            continue;
        }
        let Some(hero) = run.players.get(&l.knight).filter(|h| h.hp > 0 && !h.absent && h.privy == 0) else { continue };
        let tier = granted_tier(run, hero, &l.name);
        let grand = 1.0 + (tier.saturating_sub(1)).min(4) as f32 * 0.12;
        let s = (l.tick as u32).wrapping_mul(0x85eb) ^ l.knight;
        let (_, mid) = wishfx::body(run, hero);
        let age = age as f32;
        let beats = Beats::new(6.0, 4.0, 6.0, 20.0);
        if age < beats.anticipation {
            for i in 0..14u32 {
                let a = i as f32 / 14.0 * TAU + age * 0.05;
                let t = ease::inward(age / beats.anticipation);
                let r = 30.0 * grand * (1.0 - t) + 2.0;
                let p = (mid.0 + a.cos() * r, mid.1 + a.sin() * r * 0.7);
                pen.streak(p, (-a.cos(), -a.sin() * 0.7), 2.0 + t * 4.0, ramps::HOLY, 0.2);
            }
            pen.glow(mid, 6.0 + age, 1.0, ramps::HOLY, 0.3 + 0.5 * age / beats.anticipation);
            continue;
        }
        let since = beats.since_action(age);
        if since < 14.0 {
            pen.glow(mid, 14.0 * grand * beats.size(age).max(0.4), 1.0, ramps::HOLY, 0.8 * beats.power(age));
            pfx::rays(pen, mid, 12, 22.0 * grand, 0.9, 0.2, ramps::HOLY, since / 14.0);
        }
        let spray = Spray {
            seed: s,
            n: if pen.calm { 10 } else { (18.0 * grand) as u32 },
            at: mid,
            angle: -PI / 2.0,
            spread: 4.4,
            speed: (1.5, 4.0),
            life: (18.0, 30.0),
            drag: 0.08,
            gravity: 0.09,
            squash: 1.0,
            stagger: 0.0,
        };
        spray.each(since, |m| {
            let r = if m.i % 3 == 0 { ramps::GENIE } else { ramps::GOLD };
            pfx::mote(pen, &m, if m.i % 4 == 0 { Shape::Coin } else { Shape::Star }, r, 1, run.tick);
        });
        rising(pen, mid, since, LANDED_TICKS as f32 - beats.anticipation, s, if pen.calm { 6 } else { 14 });
    }
}

fn rising(pen: &mut Pen, mid: (f32, f32), since: f32, length: f32, s: u32, n: u32) {
    for i in 0..n {
        let start = i as f32 * 3.1 % 14.0;
        let t = (since - start) / 22.0;
        if !(0.0..1.0).contains(&t) || since > length {
            continue;
        }
        let x = mid.0 + (unit(i as i32, s as i32, 0x41) - 0.5) * 22.0 + (t * 6.0 + i as f32).sin() * 2.0;
        let y = mid.1 + 6.0 - t * 30.0;
        let r = if i % 3 == 0 { ramps::GENIE } else { ramps::HOLY };
        pen.twinkle((x, y), 2.0 + (i % 2) as f32, t, r);
        if t > 0.2 && !pen.calm {
            pen.light(x.round() as i32, (y + 3.0).round() as i32, r.step(4));
        }
    }
}

fn landed_floor(pen: &mut Pen, run: &Run) {
    for l in run.lamp.landed.iter().rev().take(4) {
        let age = run.tick.saturating_sub(l.tick) as f32;
        if age >= LANDED_TICKS as f32 || age < 6.0 {
            continue;
        }
        let Some(hero) = run.players.get(&l.knight).filter(|h| h.hp > 0 && !h.absent && h.privy == 0) else { continue };
        let (feet, _) = wishfx::body(run, hero);
        let since = age - 6.0;
        let open = ease::back((since / 8.0).min(1.0));
        let fade = ((since - 20.0) / 40.0).clamp(0.0, 1.0);
        pen.glyph((feet.0, feet.1 + 1.0), 15.0 * open, 0.42, since * 0.04, open, 6, ramps::HOLY, fade, l.tick as u32);
        pen.shock((feet.0, feet.1 + 1.0), 4.0 + ease::out(since / 14.0) * 30.0, 2.0, 0.45, ramps::GOLD, ease::inward((since / 18.0).min(1.0)));
    }
}

fn fairy_age(run: &Run) -> Option<f32> {
    let nook = run.nook.as_ref()?;
    let garden = nook.garden.as_ref()?;
    if garden.party != crate::rules::nooks::gnome::Party::Held {
        return None;
    }
    let (at, _, line) = run.found.as_ref()?;
    if line != crate::rules::nooks::gnome::HELD_LINE {
        return None;
    }
    let age = run.tick.saturating_sub(*at).checked_sub(super::garden::FAIRY_LEAD)?;
    (age < FAIRY_TICKS).then_some(age as f32)
}

fn garden_knights(run: &Run) -> impl Iterator<Item = (u32, &Hero)> {
    run.players.iter().filter(|(_, h)| h.hp > 0 && !h.absent && h.privy == 0).map(|(&id, h)| (id, h)).take(8)
}

fn fairy_at(f: u32, id: u32, mid: (f32, f32), age: f32) -> (f32, f32) {
    let (hx, hy) = super::garden::fairy_home();
    let a0 = f as f32 / 3.0 * TAU + id as f32;
    let from = (hx as f32 + a0.cos() * 60.0, hy as f32 + a0.sin() * 20.0);
    let over = (mid.0, mid.1 - 12.0);
    if age < 30.0 {
        let t = ease::both(age / 30.0);
        let ctl = ((from.0 + over.0) / 2.0 + (f as f32 - 1.0) * 30.0, from.1.min(over.1) - 40.0);
        let u = 1.0 - t;
        return (u * u * from.0 + 2.0 * u * t * ctl.0 + t * t * over.0, u * u * from.1 + 2.0 * u * t * ctl.1 + t * t * over.1);
    }
    let t = age - 30.0;
    let a = t * 0.2 + f as f32 * TAU / 3.0;
    let r = 9.0 + 2.0 * (t * 0.3).sin();
    let up = if t > 40.0 { (t - 40.0) * (t - 40.0) * 0.06 } else { 0.0 };
    (over.0 + a.cos() * r, over.1 + a.sin() * r * 0.45 - up)
}

fn fairies(pen: &mut Pen, run: &Run) {
    let Some(age) = fairy_age(run) else { return };
    for (id, hero) in garden_knights(run) {
        let (_, mid) = wishfx::body(run, hero);
        for f in 0..3u32 {
            let at = fairy_at(f, id, mid, age);
            if age > 70.0 && super::super::ink::bayer(at.0 as i32, at.1 as i32) < (age - 70.0) / 30.0 {
                continue;
            }
            let mut pts = [(0.0f32, 0.0f32); 10];
            for (j, p) in pts.iter_mut().enumerate() {
                *p = fairy_at(f, id, mid, (age - j as f32 * 1.2).max(0.0));
            }
            pen.ribbon(&pts, if f == 1 { ramps::GENIE } else { ramps::HOLY }, 1.0);
            for j in [3usize, 6, 9] {
                let tw = ((age + j as f32 * 2.0 + f as f32 * 3.0) % 12.0) / 12.0;
                pen.twinkle(pts[j], 1.5, tw, ramps::HOLY);
            }
            pen.glow(at, 5.0, 1.0, ramps::HOLY.cooler(1), 0.6);
            let im = super::garden::fairy(((run.tick / 3) as usize + f as usize) % 2);
            pen.cv.stamp(&im, at.0.round() as i32 - 3, at.1.round() as i32 - 3);
        }
        let since = age - 30.0;
        if (0.0..16.0).contains(&since) {
            pen.glow((mid.0, mid.1 - 6.0), 13.0, 1.0, ramps::HOLY, 0.8 * (1.0 - since / 16.0));
            pfx::rays(pen, (mid.0, mid.1 - 8.0), 10, 18.0, 0.9, 0.5, ramps::HOLY, since / 16.0);
        }
        if since >= 0.0 {
            let spray = Spray {
                seed: id.wrapping_mul(0x3a1) ^ 0xfa,
                n: if pen.calm { 8 } else { 16 },
                at: (mid.0, mid.1 - 8.0),
                angle: -PI / 2.0,
                spread: 5.0,
                speed: (1.2, 3.2),
                life: (16.0, 28.0),
                drag: 0.08,
                gravity: 0.07,
                squash: 1.0,
                stagger: 0.0,
            };
            spray.each(since, |m| pfx::mote(pen, &m, Shape::Star, if m.i % 2 == 0 { ramps::GENIE } else { ramps::HOLY }, 1, run.tick));
            rising(pen, mid, since, 90.0, id ^ 0xfa1, if pen.calm { 5 } else { 9 });
        }
    }
}

fn fairy_floor(pen: &mut Pen, run: &Run) {
    let Some(age) = fairy_age(run) else { return };
    let since = age - 30.0;
    if since < 0.0 {
        return;
    }
    for (id, hero) in garden_knights(run) {
        let (feet, _) = wishfx::body(run, hero);
        let open = ease::back((since / 8.0).min(1.0));
        let fade = ((since - 30.0) / 50.0).clamp(0.0, 1.0);
        pen.glyph((feet.0, feet.1 + 1.0), 14.0 * open, 0.42, since * 0.03, open, 5, ramps::NATURE, fade, id ^ 0xfa);
        for i in 0..10u32 {
            let a = i as f32 / 10.0 * TAU + since * 0.03;
            let t = ((since - i as f32 * 1.5) / 40.0).clamp(0.0, 1.0);
            if t <= 0.0 || t >= 1.0 {
                continue;
            }
            let at = (feet.0 + a.cos() * 14.0 * open, feet.1 + 1.0 + a.sin() * 6.0 * open);
            pen.twinkle(at, 1.0, t, if i % 2 == 0 { ramps::HOLY } else { ramps::NATURE });
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__magic__shots.rs"]
mod shots;
