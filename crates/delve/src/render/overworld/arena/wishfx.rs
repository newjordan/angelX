//! A wish's own magic, played from its recipe (`rules::vfx`): what trails
//! its knight's shots, what they throw when loosed, how they break on a hit
//! and on a kill, and at a wish's highest tiers what it gives off about the
//! knight who wears it — a fire wish smoulders, an ice wish frosts the air,
//! a poop wish keeps its flies.
//!
//! Everything keys on what the run already holds: the knight's cards, the
//! shots in the air, the shot feedback the snapshot carries
//! (`rules::ranged`), the deaths, the tick. A recipe is read from the card
//! (or made from its words) a frame at a time and kept in a small cache, so
//! nothing new travels and every seat draws the same.

use super::pfx::{self, Mote, Pen, Ramp, Shape, Spray, ease, ramps, unit};
use super::{UNIT, ranged_events};
use crate::rules::cards::{Card, Kind};
use crate::rules::vfx::{self, Ease, Form, Layer, Motion, Palette, Recipe, When};
use crate::rules::{HZ, Hero, Run};
use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::TAU;

/// The most motes the recipes paint in a pass, all knights together.
const BUDGET: u32 = 900;

/// A palette's inks.
pub(super) fn ramp(p: Palette) -> Ramp {
    match p {
        Palette::Fire => ramps::FIRE,
        Palette::Ember => ramps::EMBER,
        Palette::Frost => ramps::FROST,
        Palette::Holy => ramps::HOLY,
        Palette::Gold => ramps::GOLD,
        Palette::Arcane => ramps::ARCANE,
        Palette::Genie => ramps::GENIE,
        Palette::Storm => ramps::STORM,
        Palette::Poison => ramps::POISON,
        Palette::Nature => ramps::NATURE,
        Palette::Blood => ramps::BLOOD,
        Palette::Rose => ramps::ROSE,
        Palette::Poop => ramps::POOP,
        Palette::Brew => ramps::BREW,
        Palette::Void => ramps::VOID,
        Palette::Water => ramps::WATER,
        Palette::Bone => ramps::BONE,
        Palette::Smoke => ramps::SMOKE,
        Palette::Earth => ramps::EARTH,
    }
}

/// The mote a form is drawn as; none for the shapes.
fn shape(f: Form) -> Option<Shape> {
    Some(match f {
        Form::Sparks => Shape::Spark,
        Form::Embers => Shape::Ember,
        Form::Motes => Shape::Dot,
        Form::Droplets => Shape::Droplet,
        Form::Shards => Shape::Shard,
        Form::Flakes => Shape::Flake,
        Form::Bubbles => Shape::Bubble,
        Form::Notes => Shape::Note,
        Form::Stars => Shape::Star,
        Form::Petals => Shape::Petal,
        Form::Flies => Shape::Fly,
        Form::Coins => Shape::Coin,
        Form::Hearts => Shape::Heart,
        Form::Worms => Shape::Worm,
        Form::Smoke => Shape::Smoke,
        Form::Ring | Form::Glyph | Form::Arc | Form::Rays => return None,
    })
}

fn eased(e: Ease, t: f32) -> f32 {
    match e {
        Ease::Out => ease::out(t),
        Ease::In => ease::inward(t),
        Ease::Both => ease::both(t),
        Ease::Back => ease::back(t),
        Ease::Spring => ease::spring(t),
    }
}

/// Forms whose motes have weight.
fn heavy(f: Form) -> bool {
    matches!(f, Form::Shards | Form::Droplets | Form::Coins | Form::Embers | Form::Petals)
}

thread_local! {
    /// Recipes by card id, made once a thread (a card's recipe is a pure
    /// function of the card), and let go when the book has moved on.
    static RECIPES: RefCell<HashMap<String, (u64, Recipe)>> = RefCell::new(HashMap::new());
}

/// A card's recipe, from the cache.
pub(super) fn recipe(card: &Card) -> Recipe {
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (&card.name, &card.text, card.effects.len(), &card.looks.vfx, card.kind.word()).hash(&mut h);
        h.finish()
    };
    RECIPES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((k, r)) = cache.get(&card.id)
            && *k == key
        {
            return r.clone();
        }
        if cache.len() > 128 {
            cache.clear();
        }
        let r = vfx::of(card);
        cache.insert(card.id.clone(), (key, r.clone()));
        r
    })
}

/// The wish a knight's shots wear: the newest wish they hold, else the
/// wish their weapon is. Its card, recipe and tier.
pub(super) fn worn_wish<'a>(run: &'a Run, hero: &Hero) -> Option<&'a Card> {
    let wish = |c: &&Card| crate::rules::combos::is_wish(c);
    hero.deck
        .iter()
        .rev()
        .filter_map(|id| run.book.get(id))
        .find(|c| wish(c) && matches!(c.kind, Kind::Hold) && dresses_shots(c))
        .or_else(|| hero.arm.as_deref().and_then(|id| run.book.get(id)).filter(wish))
}

/// Whether a held card has anything to do with the knight's shots: a
/// wish for armour changes how they stand, not how they shoot.
fn dresses_shots(card: &Card) -> bool {
    use crate::rules::cards::Effect::*;
    card.looks.dresses_shots()
        || !card.looks.vfx.is_empty()
        || card.effects.iter().any(|e| {
            matches!(
                e,
                Damage(_) | Rate(_) | Pierce(_) | Shots(_) | Volley(_) | Bounce(_) | Homing(_) | Chain(_) | Burst(_) | Fork(_)
                    | Fart(_) | Size(_) | Knock(_) | Burn(_) | Chill(_) | Poison(_) | Boomerang(_) | Velocity(_) | Vamp(_) | Mend(_)
            )
        })
}

/// The wish a knight holds at its highest tier, the card of that tier and
/// how it stands: 1 (tier IV, a soft ambient) or 2 (tier V, a full aura).
/// A ladder shorter than five gives its top tier one step more.
fn ambient_wish<'a>(run: &'a Run, hero: &Hero) -> Option<(&'a Card, u8)> {
    let mut best: Option<(&Card, u32)> = None;
    let held = hero.deck.iter().map(String::as_str).chain(hero.arm.as_deref()).chain(hero.guard_card.as_deref());
    for id in held {
        let (Some(card), Some(tier)) = (run.book.get(id), vfx::tier(id)) else { continue };
        let tier = tier + top_bonus(id, tier);
        if best.is_none_or(|(_, t)| tier > t) {
            best = Some((card, tier));
        }
    }
    let (card, tier) = best?;
    (tier >= 4).then_some((card, if tier >= 5 { 2 } else { 1 }))
}

thread_local! {
    /// Each known wish's ladder length (`phrasebook`), read once a thread.
    static LADDERS: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

/// One step more for the top tier of a ladder shorter than five.
fn top_bonus(card: &str, tier: u32) -> u32 {
    let Some(wish) = crate::rules::dead_wishes::wish_of(card) else { return 0 };
    let len = LADDERS.with(|l| {
        let mut l = l.borrow_mut();
        if let Some(&n) = l.get(wish) {
            return n;
        }
        if l.len() > 256 {
            l.clear();
        }
        let n = crate::rules::phrasebook::get(wish).map_or(0, |w| w.tiers.len());
        l.insert(wish.to_string(), n);
        n
    });
    u32::from((3..crate::rules::phrasebook::MAX_TIERS).contains(&len) && tier as usize == len)
}

/// A seed for a knight's effects: their seat and their wish.
fn seed_of(id: u32, card: &Card) -> u32 {
    card.id.bytes().fold(id.wrapping_mul(0x9e37_79b9), |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

/// Play layer `l` at `at`, `age` ticks after it began, thrown along `dir`,
/// `scale` times its size. Its life is fitted inside the moment the run
/// keeps for it (`vfx::kept`): a layer written longer plays faster, and
/// ends, rather than being cut off in flight when its record goes.
#[allow(clippy::too_many_arguments)]
pub(super) fn play(pen: &mut Pen, l: &Layer, at: (f32, f32), dir: (f32, f32), age: f32, seed: u32, tick: u64, scale: f32) {
    let life = f32::from(l.life.max(1)) * scale.clamp(0.6, 1.6);
    let life = vfx::kept(l.on).map_or(life, |(kept, _)| life.min(f32::from(kept)));
    if age < 0.0 || age >= life {
        return;
    }
    let k = age / life;
    let r = ramp(l.palette);
    let speed = f32::from(l.speed) * 0.5;
    let count = if pen.calm { u32::from(l.count).div_ceil(2) } else { u32::from(l.count) };
    let rot = tick as f32 * f32::from(l.spin) * 0.02 + unit(seed as i32, 7, 0x51) * TAU;
    match l.form {
        Form::Ring => {
            let reach = (5.0 + speed * 3.0) * scale;
            let rr = if l.ease == Ease::In { reach * (1.0 - ease::out(k)) + 1.0 } else { reach * eased(l.ease, k) };
            pen.ring(at, rr, 1.2 + scale * 0.6, 0.62, r, ease::inward(k));
        }
        Form::Glyph => {
            let rr = (7.0 + speed * 1.5) * scale;
            let open = eased(l.ease, (k * 2.6).min(1.0));
            let fade = ((k - 0.55) / 0.45).max(0.0);
            pen.glyph(at, rr * open.max(0.4), 0.55, rot, open, 5, r, fade, seed);
        }
        Form::Arc => {
            if k > 0.55 {
                return;
            }
            let flick = (tick / 2) as u32;
            for i in 0..count.min(4) {
                let a = unit(i as i32, (seed ^ flick) as i32, 0xa1) * TAU;
                let len = (6.0 + speed * 2.5) * scale * (0.6 + unit(i as i32, seed as i32, 0xa2) * 0.6);
                let end = (at.0 + a.cos() * len, at.1 + a.sin() * len * 0.7);
                pen.arc(at, end, seed ^ flick.wrapping_mul(31) ^ i, 2.0 * scale, r, k * 1.6);
            }
        }
        Form::Rays => {
            pfx::rays(pen, at, count.max(4).min(12), (5.0 + speed * 2.0) * scale, 0.8, rot, r, eased(l.ease, k).clamp(0.0, 0.999).max(k));
        }
        Form::Smoke => {
            for i in 0..count.min(4) {
                let off = ((unit(i as i32, seed as i32, 0x5a) - 0.5) * 8.0 * scale, (unit(i as i32, seed as i32, 0x5b) - 0.5) * 4.0);
                let base = (at.0 + off.0, at.1 + off.1);
                pen.wisp(base, age, (0.0, life * 0.35), life * 0.8, (8.0 + speed * 2.0) * scale, 2.5, r.cooler(1), seed.wrapping_add(i));
            }
        }
        form => {
            let Some(sh) = shape(form) else { return };
            let gravity = match l.motion {
                Motion::Fall => 0.14,
                Motion::Rise => -0.07,
                Motion::Bounce => 0.18,
                _ if heavy(form) => 0.08,
                _ => 0.0,
            };
            let spray = Spray {
                seed,
                n: count,
                at,
                angle: dir.1.atan2(dir.0),
                spread: f32::from(l.spread).to_radians(),
                speed: (speed * 0.55 * scale, speed * 1.25 * scale),
                life: (life * 0.55, life),
                drag: 0.14,
                gravity,
                squash: 0.8,
                stagger: 0.0,
            };
            // A layer's ease runs its clock: a back eases out past its
            // mark and comes home.
            let t = life * eased(l.ease, k).max(0.0);
            let t = if l.ease == Ease::Out { age } else { t };
            spray.each(t, |mut m| {
                moved(&mut m, l, at, t);
                pfx::mote(pen, &m, sh, r, l.size, tick);
            });
        }
    }
}

/// A mote moved its layer's way about where it was thrown from.
fn moved(m: &mut Mote, l: &Layer, at: (f32, f32), age: f32) {
    let w = f32::from(l.wobble.max(1));
    let phase = unit(m.i as i32, m.seed as i32, 0x3c) * TAU;
    let (vx, vy) = m.vel;
    let v = vx.hypot(vy).max(1e-3);
    let (nx, ny) = (-vy / v, vx / v);
    match l.motion {
        Motion::Wobble => {
            let s = (age * 0.7 + phase).sin() * w * 0.6;
            m.at = (m.at.0 + nx * s, m.at.1 + ny * s);
        }
        Motion::Zigzag => {
            let tri = ((age * 0.5 + phase) % 2.0 - 1.0).abs() * 2.0 - 1.0;
            m.at = (m.at.0 + nx * tri * w * 0.7, m.at.1 + ny * tri * w * 0.7);
        }
        Motion::Spiral | Motion::Curl => {
            let turn = f32::from(l.spin.max(1)) * age * if l.motion == Motion::Spiral { 0.09 } else { 0.035 };
            let (dx, dy) = (m.at.0 - at.0, m.at.1 - at.1);
            let (c, s) = (turn.cos(), turn.sin());
            m.at = (at.0 + dx * c - dy * s, at.1 + dx * s + dy * c);
        }
        Motion::Bounce => {
            let hop = ((age * 0.45 + phase).sin()).abs() * w.max(3.0) * (1.0 - m.k);
            m.at.1 -= hop;
        }
        Motion::Rise | Motion::Fall | Motion::Straight => {}
    }
}

/// The recipes' pass: trails, loosings, hits and kills, then the worn
/// ambients. Over the figures, under nothing.
pub(super) fn over(cv: &mut super::super::ink::Img, run: &Run) {
    let mut pen = Pen::new(cv, BUDGET);
    trails(&mut pen, run);
    events(&mut pen, run);
    kills(&mut pen, run);
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && h.privy == 0 && !h.absent) {
        if let Some((card, level)) = ambient_wish(run, hero) {
            ambient(&mut pen, run, id, hero, card, level, false);
        }
    }
}

/// Under the figures: the full aura's ring on the floor.
pub(super) fn under(cv: &mut super::super::ink::Img, run: &Run) {
    let mut pen = Pen::new(cv, BUDGET / 3);
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && h.privy == 0 && !h.absent) {
        if let Some((card, level)) = ambient_wish(run, hero) {
            ambient(&mut pen, run, id, hero, card, level, true);
        }
    }
}

/// The knight's recipe and seed, if they wear a wish.
fn worn(run: &Run, id: u32) -> Option<(Recipe, u32, u32)> {
    let hero = run.players.get(&id)?;
    let card = worn_wish(run, hero)?;
    let tier = vfx::tier(&card.id).unwrap_or(1);
    Some((recipe(card), seed_of(id, card), tier))
}

/// Behind each friendly shot whose knight wears a wish: its trail layer,
/// tied to the path so it never swims, and a small glow at its head in
/// the wish's colour.
fn trails(pen: &mut Pen, run: &Run) {
    let mut known: [Option<Option<(Recipe, u32, u32)>>; 9] = Default::default();
    for shot in run.projectiles.iter().filter(|p| !p.hostile) {
        let owner = u32::from(shot.traits.owner);
        let slot = owner as usize;
        if slot >= known.len() {
            continue;
        }
        let worn = known[slot].get_or_insert_with(|| worn(run, owner));
        let Some((recipe, seed, tier)) = worn.as_ref() else { continue };
        let Some(l) = recipe.first(When::Trail) else { continue };
        trail(pen, run, shot, l, *seed, *tier);
    }
}

fn trail(pen: &mut Pen, run: &Run, shot: &crate::rules::Projectile, l: &Layer, seed: u32, tier: u32) {
    let head = (shot.x * UNIT, shot.y * UNIT);
    let v = shot.vx.hypot(shot.vy);
    if v < 0.5 {
        return;
    }
    let (ux, uy) = (shot.vx / v, shot.vy / v);
    let (nx, ny) = (-uy, ux);
    let per_tick = v * UNIT / HZ as f32;
    let r = ramp(l.palette);
    pen.glow(head, 4.0 + tier.min(5) as f32 * 0.4, 1.0, r.cooler(1), 0.6);
    let sh = shape(l.form).unwrap_or(Shape::Dot);
    // A thread of the wish's colour straight back off the shot, so the
    // motes read as one trail.
    let reach = (f32::from(l.life) * per_tick).min(10.0);
    let pts = [head, (head.0 - ux * reach * 0.5, head.1 - uy * reach * 0.5), (head.0 - ux * reach, head.1 - uy * reach)];
    pen.ribbon(&pts, r.cooler(1), 1.0);
    // Where along its line the shot is: a phase that rides the path.
    let along = shot.x * ux * UNIT + shot.y * uy * UNIT;
    let wide = matches!(sh, Shape::Note | Shape::Heart | Shape::Bubble | Shape::Coin | Shape::Worm | Shape::Fly);
    let n = (u32::from(l.count) * if wide { 1 } else { 2 }).min(12);
    let gap = if wide { 7.0 } else { 3.0 } + f32::from(l.size);
    let size = if matches!(sh, Shape::Dot | Shape::Droplet | Shape::Ember | Shape::Petal) { l.size.max(2) } else { l.size };
    let life = f32::from(l.life);
    let w = f32::from(l.wobble);
    for j in 1..=n {
        let back = j as f32 * gap;
        let ago = back / per_tick.max(0.3);
        if ago >= life {
            break;
        }
        let k = ago / life;
        let p = along - back;
        let side = match l.motion {
            Motion::Wobble => (p * 0.32 + seed as f32).sin() * w.max(1.5),
            Motion::Zigzag => (((p * 0.12) % 2.0).abs() - 1.0) * 2.0 * w.max(1.5),
            Motion::Spiral => (p * 0.45).sin() * w.max(2.0) * (0.4 + k),
            Motion::Curl => (p * 0.18).sin() * 2.0 * k,
            Motion::Bounce => -(p * 0.25).sin().abs() * 3.0,
            Motion::Rise => 0.0,
            Motion::Fall | Motion::Straight => (unit(j as i32, seed as i32, (p as i32) as u32 & 0xff) - 0.5) * 2.0 * k,
        };
        let lift = match l.motion {
            Motion::Rise => -ago * 0.35,
            Motion::Fall => ago * 0.3,
            _ => 0.0,
        };
        let at = (head.0 - ux * back + nx * side, head.1 - uy * back + ny * side + lift);
        // Thinning toward the tail.
        if j > 2 && super::super::ink::bayer(at.0 as i32, at.1 as i32) < k * 0.6 {
            continue;
        }
        let m = Mote { i: j, at, vel: (ux * per_tick, uy * per_tick), k, seed: seed ^ (p as i32 / 7) as u32 };
        pfx::mote(pen, &m, sh, r, size, run.tick);
    }
}

/// Loosings and hits, from the shot feedback the run carries.
fn events(pen: &mut Pen, run: &Run) {
    use ranged_events::Mark;
    let mut known: [Option<Option<(Recipe, u32, u32)>>; 9] = Default::default();
    ranged_events::each(run, |e| {
        if e.hostile {
            return;
        }
        let slot = usize::from(e.owner);
        if slot >= known.len() {
            return;
        }
        let worn = known[slot].get_or_insert_with(|| worn(run, u32::from(e.owner)));
        let Some((recipe, seed, tier)) = worn.as_ref() else { return };
        let when = match e.mark {
            Mark::Release => When::Fire,
            Mark::Flesh | Mark::Wall | Mark::Pop => When::Hit,
        };
        let at = (e.x * UNIT, e.y * UNIT);
        let dir = if when == When::Fire { (e.dx, e.dy) } else { (-e.dx, -e.dy) };
        let s = seed ^ hash_point(e.x, e.y);
        let scale = 0.85 + (*tier).min(5) as f32 * 0.07 * if e.mark == Mark::Flesh { 1.0 } else { 0.6 };
        for l in recipe.on(when) {
            play(pen, l, at, dir, f32::from(e.age()), s ^ l.form as u32, run.tick, scale);
        }
    });
}

/// A point's seed: where it happened.
fn hash_point(x: f32, y: f32) -> u32 {
    super::super::ink::hash((x * 8.0) as i32, (y * 8.0) as i32, 0x77)
}

/// A kill: a death where a knight's shot struck flesh a moment before,
/// thrown in that knight's kill layers, and marked in the wish's colours
/// whatever its recipe says: a flash where it fell, a star of rays, a
/// shock with a dark wake running off over the floor, and at a high tier
/// a spark of it going up as the body goes.
fn kills(pen: &mut Pen, run: &Run) {
    use crate::rules::feel::DEATH_TICKS;
    use ranged_events::Mark;
    for d in &run.feel.deaths {
        let age = DEATH_TICKS.saturating_sub(d.ttl);
        let mut owner = None;
        ranged_events::each(run, |e| {
            if owner.is_none() && !e.hostile && e.mark == Mark::Flesh && (e.x - d.x).hypot(e.y - d.y) < d.r + 1.2 && i32::from(e.age()).abs_diff(i32::from(age)) <= 2 {
                owner = Some(u32::from(e.owner));
            }
        });
        let Some((recipe, seed, tier)) = owner.and_then(|o| worn(run, o)) else { continue };
        let at = (d.x * UNIT, d.y * UNIT - 4.0);
        let s = seed ^ hash_point(d.x, d.y);
        let r = recipe.first(When::Kill).or_else(|| recipe.first(When::Hit)).map_or(ramps::ARCANE, |l| ramp(l.palette));
        let age = f32::from(age);
        let big = 1.0 + tier.min(5) as f32 * 0.08 + d.r * 0.15;
        if age < 5.0 {
            pen.glow(at, 10.0 * big, 1.0, r, 1.0 - age / 5.0);
        }
        pfx::rays(pen, at, 10, 16.0 * big, 0.85, unit(s as i32, 4, 0x4b) * TAU, r, age / 11.0);
        pen.shock((at.0, at.1 + 4.0), 4.0 + ease::out(age / 12.0) * 22.0 * big, 1.8, 0.5, r, ease::inward((age / 14.0).min(1.0)));
        if tier >= 3 && (6.0..32.0).contains(&age) {
            let t = (age - 6.0) / 26.0;
            pen.twinkle((at.0 + (t * 9.0).sin() * 2.0, at.1 - 4.0 - t * 22.0), 2.0, t, r);
        }
        for l in recipe.on(When::Kill) {
            play(pen, l, at, (0.0, -1.0), age, s, run.tick, 1.15 + tier.min(5) as f32 * 0.06);
        }
    }
}

/// The knight's feet and middle, in native pixels.
pub(super) fn body(run: &Run, hero: &Hero) -> ((f32, f32), (f32, f32)) {
    let lift = if run.side_on() { crate::rules::ledge::FOOT } else { 0.0 };
    let feet = (hero.x * UNIT, (hero.y + lift) * UNIT);
    (feet, (feet.0, feet.1 - 9.0))
}

/// What a wish worn at its highest tiers gives off. Level 1 (tier IV): a
/// soft glow about the knight, a few slow motes of its kind drifting off,
/// now and then a rune that flickers and goes. Level 2 (tier V): all that,
/// a slow ring on the floor under them, its motes orbiting, a shimmer
/// running over the figure and sparkles at every footfall. `floor` draws
/// the ring alone (under the figures); otherwise the rest (over them).
#[allow(clippy::too_many_arguments)]
fn ambient(pen: &mut Pen, run: &Run, id: u32, hero: &Hero, card: &Card, level: u8, floor: bool) {
    let recipe = recipe(card);
    // A hand-written recipe with no aura of its own takes its words' one.
    let Some(l) = recipe.first(When::Aura).copied().or_else(|| vfx::generated(card).first(When::Aura).copied()) else { return };
    let r = ramp(l.palette);
    let seed = seed_of(id, card);
    let tick = run.tick;
    let t = tick as f32;
    let (feet, mid) = body(run, hero);
    if floor {
        if level >= 2 {
            let rot = t * 0.012 * if l.spin < 0 { -1.0 } else { 1.0 };
            let breathe = 0.85 + 0.15 * (t * 0.07).sin();
            pen.glyph((feet.0, feet.1 + 1.0), 13.0, 0.42, rot, 1.0, 6, r, if pen.calm { 0.5 } else { 0.4 * (1.0 - breathe) }, seed);
            pen.glow((feet.0, feet.1 + 1.0), 15.0, 0.42, r.cooler(1), 0.55);
        }
        return;
    }
    // A soft glow on what they wear, behind them.
    let breath = 0.85 + 0.15 * (t * 0.09 + seed as f32).sin();
    pen.glow(mid, if level >= 2 { 12.0 } else { 9.0 }, 1.1, r.cooler(1), breath * if level >= 2 { 0.7 } else { 0.5 });
    let sh = shape(l.form).unwrap_or(Shape::Dot);
    // Motes drifting off: each on a loop of its own, let go in turn.
    let n = if level >= 2 { u32::from(l.count).clamp(5, 8) } else { 4 };
    let n = if pen.calm { n.div_ceil(2) } else { n };
    let life = f32::from(l.life).max(24.0);
    for i in 0..n {
        let u = |s: u32| unit(i as i32, seed as i32, s);
        let start = u(1) * life;
        let age = (t + start) % life;
        let k = age / life;
        let loop_n = ((t + start) / life) as i32;
        let jitter = |s: u32| unit(i as i32 + loop_n * 17, seed as i32, s) - 0.5;
        let (dx, dy, vx, vy) = match l.motion {
            Motion::Rise => (jitter(2) * 12.0 + (age * 0.2 + u(3) * TAU).sin() * 2.0, -age * 0.55, 0.0, -0.5),
            Motion::Fall => (jitter(2) * 12.0, -6.0 + age * 0.4, 0.0, 0.4),
            Motion::Curl | Motion::Wobble => {
                // A fly's loop: round and round the knight, never tidy.
                let a = u(4) * TAU + age * 0.16 * if i % 2 == 0 { 1.0 } else { -1.3 };
                let rr = 8.0 + 3.0 * (age * 0.21 + u(5) * 6.0).sin();
                (a.cos() * rr, a.sin() * rr * 0.6 - 2.0, -a.sin(), a.cos())
            }
            _ => {
                let a = u(4) * TAU + age * 0.09;
                (a.cos() * 10.0, a.sin() * 4.0 - age * 0.15, -a.sin(), a.cos() * 0.4)
            }
        };
        let orbit = level >= 2 && !matches!(l.motion, Motion::Curl | Motion::Wobble);
        let at = if orbit {
            // Tier V: they go round, a ring of them, and rise a little.
            let a = i as f32 / n as f32 * TAU + t * 0.06 * if l.spin < 0 { -1.0 } else { 1.0 };
            (mid.0 + a.cos() * 11.0, mid.1 + a.sin() * 4.5 + 2.0 - ease::bump(k) * 3.0)
        } else {
            (mid.0 + dx, mid.1 + dy)
        };
        let m = Mote { i, at, vel: (vx, vy), k: if orbit { 0.1 + 0.4 * (1.0 - ease::bump(k)) } else { k * 0.8 }, seed: seed ^ i };
        let size = if matches!(sh, Shape::Dot | Shape::Ember | Shape::Droplet | Shape::Petal) { 2 } else { 1 };
        pfx::mote(pen, &m, sh, r, size, tick);
    }
    // A rune flickers near them now and then.
    let beat = (tick / 37) as i32;
    if !pen.calm && unit(beat, seed as i32, 0x9a) < if level >= 2 { 0.8 } else { 0.45 } && tick % 37 < 6 {
        let a = unit(beat, seed as i32, 0x9b) * TAU;
        let at = (mid.0 + a.cos() * 9.0, mid.1 - 4.0 + a.sin() * 5.0);
        pen.twinkle(at, 2.0, (tick % 37) as f32 / 6.0, r);
    }
    if level >= 2 && !pen.calm {
        // A shimmer running over the figure, top to bottom.
        let band = ((t * 0.6) % 26.0) - 4.0;
        for k in 0..9 {
            let h = super::super::ink::hash(k, (tick / 2) as i32, seed);
            let (x, y) = (mid.0 as i32 - 4 + (h % 9) as i32, feet.1 as i32 - 20 + band as i32 + (k % 2));
            if pen.cv.get(x, y).is_some() {
                pen.light(x, y, r.step(1));
            }
        }
        // Sparkles at each footfall as they go.
        let step = (hero.stride * 0.9) as i32;
        for k in 0..2 {
            let pop = ((hero.stride * 0.9) - step as f32 + k as f32 * 0.5) % 1.0;
            let side = if (step + k) % 2 == 0 { -3.0 } else { 3.0 };
            if hero.stride > 0.0 {
                pen.twinkle((feet.0 + side, feet.1 - 1.0 - pop * 3.0), 1.5, pop, r);
            }
        }
    }
}
