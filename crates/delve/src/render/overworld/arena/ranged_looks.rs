//! Ranged parts: a small palette, a body and a wake, shared by every floor.
//! The wake is cosmetic; only the outlined nose describes a foe's hurt area.
use super::super::*;
use crate::rules::{Projectile, Shot, patterns};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in super::super) enum Body {
    Arrow,
    Bolt,
    Ball,
    Magic,
    Shard,
}

#[derive(Clone, Copy)]
pub(in super::super) struct Look {
    pub body: Body,
    pub length: f32,
    pub width: f32,
    pub core: char,
    pub edge: char,
    pub wake: char,
    pub muzzle: f32,
    pub kick: f32,
}

const ARROW: Look = Look {
    body: Body::Arrow,
    length: 10.0,
    width: 1.0,
    core: 'H',
    edge: 'J',
    wake: 'g',
    muzzle: 3.0,
    kick: 1.0,
};
const BOLT: Look = Look {
    body: Body::Bolt,
    length: 7.0,
    width: 2.0,
    core: 'H',
    edge: 'R',
    wake: 'g',
    muzzle: 5.0,
    kick: 2.0,
};
const BALL: Look = Look {
    body: Body::Ball,
    length: 4.0,
    width: 4.0,
    core: 'H',
    edge: 'g',
    wake: 'G',
    muzzle: 8.0,
    kick: 3.0,
};
const MAGIC: Look = Look {
    body: Body::Magic,
    length: 6.0,
    width: 3.5,
    core: '@',
    edge: 'R',
    wake: 'g',
    muzzle: 5.0,
    kick: 1.0,
};
const SHARD: Look = Look {
    body: Body::Shard,
    length: 6.0,
    width: 2.0,
    core: 'H',
    edge: 'J',
    wake: 'g',
    muzzle: 3.0,
    kick: 1.0,
};

pub(in super::super) fn look(kind: Shot) -> Look {
    match kind {
        Shot::Arrow => ARROW,
        Shot::Bolt => BOLT,
        Shot::Ball => BALL,
        Shot::Blade | Shot::Bone | Shot::Ore | Shot::Page => SHARD,
        Shot::Frost => Look {
            core: 'V',
            edge: 'Q',
            ..MAGIC
        },
        Shot::Hex | Shot::Spore => Look {
            core: 'Y',
            edge: 'e',
            ..MAGIC
        },
        Shot::Wisp | Shot::Orb | Shot::Eye => Look {
            core: '3',
            edge: 'q',
            ..MAGIC
        },
        _ => MAGIC,
    }
}

pub(in super::super) fn form(shot: &Projectile) -> Option<room_scene::ShotForm> {
    use room_scene::{ForgedColour, ForgedShape, ShotForm};
    if !enabled()
        || shot.hostile
        || shot.sprite_card().is_some()
        || shot.traits.borrowed
        || shot.tint().is_some()
    {
        return None;
    }
    if let Some((colour, shape)) = &shot.look {
        if matches!(shape.as_str(), "note" | "stone") {
            return None;
        }
        return Some(class_looks::class_of(colour).map_or_else(
            || ShotForm::Forged(ForgedColour::of(colour), ForgedShape::of(shape)),
            ShotForm::Class,
        ));
    }
    Some(ShotForm::Kind(shot.kind))
}

fn form_look(form: room_scene::ShotForm) -> Look {
    use room_scene::{ClassShot, ForgedColour, ForgedShape, ShotForm};
    match form {
        ShotForm::Kind(kind) => look(kind),
        ShotForm::Class(ClassShot::Arcane) => Look {
            core: '3',
            edge: 'q',
            ..MAGIC
        },
        ShotForm::Class(ClassShot::Holy) => MAGIC,
        ShotForm::Forged(colour, shape) => {
            let mut ink = match shape {
                ForgedShape::Arrow => ARROW,
                ForgedShape::Shard => SHARD,
                _ => MAGIC,
            };
            (ink.core, ink.edge) = match colour {
                ForgedColour::Ice => ('3', 'q'),
                ForgedColour::Venom => ('Y', 'e'),
                ForgedColour::Void => ('n', 'q'),
                ForgedColour::Rose => ('I', 'n'),
                _ => ('@', 'R'),
            };
            ink
        }
        _ => MAGIC,
    }
}

pub(in super::super) fn projectile_look(shot: &Projectile) -> Look {
    let mut ink = form(shot).map_or_else(|| look(shot.kind), form_look);
    if let Some(tint) = shot.tint() {
        use crate::rules::wishcraft::Tint;
        (ink.core, ink.edge) = match tint {
            Tint::Burn => ('@', 'R'),
            Tint::Chill => ('3', 'q'),
            Tint::Poison => ('Y', 'e'),
        };
    }
    ink
}

fn pixel(cv: &mut Img, p: (i32, i32), way: (f32, f32), along: f32, side: f32, ink: char) {
    cv.put(
        p.0 + (way.0 * along - way.1 * side).round() as i32,
        p.1 + (way.1 * along + way.0 * side).round() as i32,
        ink,
    );
}

#[cfg(test)]
thread_local! { static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

#[cfg(test)]
pub(super) fn before(value: bool) {
    BEFORE.with(|v| v.set(value));
}

fn enabled() -> bool {
    #[cfg(test)]
    if BEFORE.with(|v| v.get()) {
        return false;
    }
    super::super::slot_art::enabled()
}

pub(super) fn flight(cv: &mut Img, _run: &Run, shot: &Projectile) -> bool {
    if !enabled() {
        return false;
    }
    if !shot.in_flight() {
        return false;
    }
    if shot.hostile && shot.traits.path.form == patterns::Form::Lob {
        return false;
    }
    let p = at(shot.x, shot.y);
    let speed = shot.vx.hypot(shot.vy);
    let way = if speed > 0.001 {
        (shot.vx / speed, shot.vy / speed)
    } else {
        (1.0, 0.0)
    };
    if shot.hostile {
        // Same danger grammar for every room and species. Keep the rim inside
        // the collision disc; a dim wake is never another solid bullet.
        let radius = (patterns::girth(shot) * UNIT).max(0.5);
        let r = radius.ceil() as i32;
        for y in -r..=r {
            for x in -r..=r {
                // Cover cells touched by the disc, at most half a pixel beyond it.
                let d = ((x.abs() as f32 - 0.5).max(0.0)).hypot((y.abs() as f32 - 0.5).max(0.0));
                if d <= radius {
                    cv.put(p.0 + x, p.1 + y, '8');
                }
            }
        }
        let inner = (radius - 1.0).max(0.0);
        for y in -r..=r {
            for x in -r..=r {
                if (x as f32).hypot(y as f32) <= inner {
                    cv.put(p.0 + x, p.1 + y, '@');
                }
            }
        }
        // A detached, dim wake is direction, not a larger collision body.
        for k in 0..6 {
            pixel(
                cv,
                p,
                way,
                -radius - 2.0 - k as f32,
                0.0,
                if k < 2 { 'a' } else { '8' },
            );
        }
        cv.put(p.0, p.1, '6');
        if shot.ttl <= 2 {
            for (dx, dy) in [(-3, 0), (3, 0), (0, -3), (0, 3)] {
                cv.put(p.0 + dx, p.1 + dy, 'a');
            }
        }
        return true;
    }
    if shot.sprite_card().is_some() || shot.traits.borrowed {
        return false;
    }
    if shot
        .look
        .as_ref()
        .is_some_and(|(_, shape)| matches!(shape.as_str(), "note" | "stone"))
    {
        return false;
    }
    let ink = projectile_look(shot);
    let size = shot.scale() * if shot.empowered { 1.25 } else { 1.0 };
    body(
        cv,
        p,
        way,
        speed * UNIT,
        ink,
        size,
        shot.empowered || shot.traits.size > 0,
    );
    true
}

fn body(
    cv: &mut Img,
    p: (i32, i32),
    way: (f32, f32),
    speed: f32,
    ink: Look,
    size: f32,
    bright: bool,
) {
    let n = (ink.length * size).round().max(3.0) as i32;
    let width = (ink.width * size).round().max(1.0) as i32;
    // A frame's travel plus the shaft, capped so a rail never becomes a beam.
    let trail = ((speed / 60.0).ceil() as i32 + n / 2).clamp(3, 18);
    for k in (n / 2..trail + n / 2).rev() {
        pixel(cv, p, way, -k as f32, 0.0, ink.wake);
    }
    match ink.body {
        Body::Arrow | Body::Bolt | Body::Shard => {
            for k in -n / 2..=n / 2 {
                for s in 0..width {
                    pixel(cv, p, way, k as f32, s as f32, ink.edge);
                }
                pixel(cv, p, way, k as f32, 0.0, ink.core);
            }
            if ink.body != Body::Shard {
                let back = -n as f32 * 0.4;
                for side in [-1.0, 1.0] {
                    pixel(
                        cv,
                        p,
                        way,
                        back,
                        side,
                        if ink.body == Body::Arrow { 'H' } else { 'R' },
                    );
                    pixel(cv, p, way, back + 1.0, side * 2.0 * size, ink.edge);
                }
                pixel(cv, p, way, n as f32 / 2.0, -1.0, 'H');
                pixel(cv, p, way, n as f32 / 2.0 - 1.0, 1.0, ink.edge);
            }
        }
        Body::Ball | Body::Magic => {
            let r = (ink.width * size * 0.65).max(1.0);
            let halo = if ink.body == Body::Magic { r + 1.0 } else { r };
            for y in -(halo.ceil() as i32)..=halo.ceil() as i32 {
                for x in -(halo.ceil() as i32)..=halo.ceil() as i32 {
                    let d = (x as f32).hypot(y as f32);
                    if d <= halo {
                        cv.put(
                            p.0 + x,
                            p.1 + y,
                            if d > r {
                                ink.wake
                            } else if d > r * 0.6 {
                                ink.edge
                            } else {
                                ink.core
                            },
                        );
                    }
                }
            }
            if ink.body == Body::Ball {
                cv.put(p.0 - 1, p.1 - 1, 'H');
                pixel(cv, p, way, -r - 3.0, -1.0, 'G');
                pixel(cv, p, way, -r - 5.0, 1.0, 'g');
            }
        }
    }
    if bright {
        for s in [-1.0, 1.0] {
            pixel(cv, p, way, 1.0, s * (width as f32 + 1.0), '6');
        }
    }
}

pub(super) fn recoil(run: &Run, id: u32, hero: &crate::rules::Hero) -> f32 {
    if hero.weapon == crate::rules::Weapon::Fists {
        return 0.0;
    }
    let mut kick = 0.0f32;
    super::events::each(run, |e| {
        if e.mark == super::events::Mark::Release
            && !e.hostile
            && u32::from(e.owner) == id
            && e.age() < 4
        {
            kick = kick.max(look(e.kind).kick * (1.0 - f32::from(e.age()) / 4.0).powi(2));
        }
    });
    kick
}

pub(super) fn muzzle(cv: &mut Img, run: &Run, puff: &crate::rules::feel::Puff) -> bool {
    if !enabled() {
        return false;
    }
    // A puff has no owner; match its fresh release rather than guessing from
    // the closest knight (who may be holding an entirely different weapon).
    let Some(release) = run
        .feel
        .ranged
        .iter()
        .rev()
        .filter(|e| {
            !e.hostile
                && e.mark == super::events::Mark::Release
                && e.age() < 3
                && e.dx * puff.dx + e.dy * puff.dy > 0.9
                && (e.x - puff.x).hypot(e.y - puff.y) < 0.75
        })
        .min_by(|a, b| {
            (a.x - puff.x)
                .hypot(a.y - puff.y)
                .total_cmp(&(b.x - puff.x).hypot(b.y - puff.y))
        })
    else {
        return false;
    };
    let ink = look(release.kind);
    let age = crate::rules::feel::FLASH_TICKS.saturating_sub(puff.ttl) as f32;
    let p = at(puff.x, puff.y);
    let way = (puff.dx, puff.dy);
    let length = (ink.muzzle * (1.0 - age / 4.0)).max(1.0) as i32;
    for k in 0..length {
        let width = if ink.body == Body::Ball {
            (length - k) / 3
        } else {
            0
        };
        for s in -width..=width {
            pixel(
                cv,
                p,
                way,
                k as f32,
                s as f32,
                if k < 2 { '6' } else { ink.core },
            );
        }
    }
    for side in [-1.0, 1.0] {
        pixel(cv, p, way, 1.0, side * (3.0 - age * 0.5), ink.edge);
    }
    true
}

pub(super) fn accents(cv: &mut Img, run: &Run) {
    if !enabled() {
        return;
    }
    // A wisp off every non-powder release; powder already has its own smoke.
    for p in run.feel.puffs.iter().filter(|p| !p.smoke && p.dust == 0) {
        let age = crate::rules::feel::FLASH_TICKS.saturating_sub(p.ttl) as f32;
        let pos = at(p.x - p.dx * 0.15, p.y - p.dy * 0.15);
        pixel(cv, pos, (p.dx, p.dy), -age, 2.0 + age, 'g');
    }
    for hero in run.players.values().filter(|h| h.hero_kit.charging) {
        let p = at(hero.x + hero.aim_x * 0.7, hero.y + hero.aim_y * 0.7);
        let r = 2.0 + hero.hero_kit.charge.min(30) as f32 / 15.0;
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            cv.put(
                p.0 + (a.cos() * r).round() as i32,
                p.1 + (a.sin() * r).round() as i32,
                if k % 2 == 0 { 'R' } else { '6' },
            );
        }
    }
}

pub(super) fn events(cv: &mut Img, run: &Run) {
    if !enabled() {
        return;
    }
    use super::events::Mark;
    super::events::each(run, |e| {
        let age = f32::from(e.age());
        let p = at(e.x, e.y);
        let way = (e.dx, e.dy);
        let ink = look(e.kind);
        match e.mark {
            Mark::Release if e.hostile && age < 4.0 => {
                // Spawn beat, outside the body and broken, never a safe delay.
                let r = e.radius * UNIT + 2.0 + age * 0.5;
                for k in 0..4 {
                    let a = k as f32 * std::f32::consts::FRAC_PI_2;
                    cv.put(
                        p.0 + (a.cos() * r).round() as i32,
                        p.1 + (a.sin() * r).round() as i32,
                        'R',
                    );
                }
            }
            Mark::Release if !e.hostile => {
                // Bow release, crossbow snap, powder flash, spell bloom.
                if age < 3.0 {
                    let length = (ink.muzzle * (1.0 - age / 4.0)) as i32;
                    for k in 0..length {
                        pixel(
                            cv,
                            p,
                            way,
                            k as f32,
                            0.0,
                            if k < 2 { '6' } else { ink.core },
                        );
                    }
                    if ink.body == Body::Magic {
                        for s in [-1.0, 1.0] {
                            pixel(cv, p, way, 1.0, s * (2.0 + age), ink.edge);
                        }
                    }
                }
                if age < 10.0 {
                    for k in 0..2 {
                        pixel(
                            cv,
                            p,
                            way,
                            -age * 0.3 - k as f32,
                            2.0 + age * 0.2,
                            if age < 5.0 { 'G' } else { 'g' },
                        );
                    }
                }
                if ink.body == Body::Ball {
                    // Powder wad ejected beside the mouth, bounces and rests.
                    let t = age.min(10.0);
                    let bounce = (t * (10.0 - t) * 0.3).max(0.0);
                    let side = 3.0 + t * 0.45;
                    let x = p.0 + (-e.dy * side - e.dx * t * 0.25).round() as i32;
                    let y = p.1 + (e.dx * side - e.dy * t * 0.25 - bounce).round() as i32;
                    cv.put(x, y, if age < 14.0 { 'o' } else { 'g' });
                    if age < 12.0 && (age as i32) % 2 == 0 {
                        pixel(cv, p, way, -age * 0.3, age * 0.2, 'g');
                    }
                }
            }
            Mark::Wall => {
                // Stone dust is neutral; chips fall back rather than explode.
                for k in 0..4 {
                    let side = (k as f32 - 1.5) * (1.0 + age * 0.35);
                    pixel(
                        cv,
                        p,
                        way,
                        -age * 0.4,
                        side,
                        if age < 6.0 { 'J' } else { 'g' },
                    );
                }
                if matches!(ink.body, Body::Arrow | Body::Bolt) {
                    let t = age.min(10.0);
                    let lift = (t * (10.0 - t) * 0.25).max(0.0);
                    for k in 0..3 {
                        pixel(
                            cv,
                            (p.0, p.1 - lift.round() as i32),
                            way,
                            -t * 0.35 - k as f32,
                            3.0 + t * 0.2,
                            if age < 14.0 { 'o' } else { 'g' },
                        );
                    }
                }
            }
            Mark::Flesh => {
                // The flesh burst is gore's job. Confirm at the shooter's hand.
                if age < 4.0
                    && let Some(hero) = run.players.get(&u32::from(e.owner))
                {
                    let p = at(hero.x + hero.aim_x * 0.6, hero.y + hero.aim_y * 0.6);
                    for s in [-1.0, 1.0] {
                        pixel(cv, p, (hero.aim_x, hero.aim_y), 0.0, s * (2.0 + age), '6');
                    }
                }
            }
            Mark::Pop if age < 6.0 => {
                let r = 1.0 + age * 0.65;
                for k in 0..4 {
                    let a = k as f32 * std::f32::consts::FRAC_PI_2;
                    cv.put(
                        p.0 + (a.cos() * r).round() as i32,
                        p.1 + (a.sin() * r).round() as i32,
                        if age < 2.0 { '6' } else { 'g' },
                    );
                }
            }
            _ => {}
        }
    });
}

/// One fallback sprite per class, shared by the stage's sheet.
pub(in super::super) fn mark(form: room_scene::ShotForm) -> Img {
    let mut mark = Img::new(25, 25);
    body(
        &mut mark,
        (12, 12),
        (1.0, 0.0),
        0.0,
        form_look(form),
        0.5,
        false,
    );
    mark
}

pub(in super::super) fn replay(cv: &mut Img, shot: &room_scene::SceneShot) {
    let speed = shot.vx.hypot(shot.vy).max(0.001);
    body(
        cv,
        (shot.x.floor() as i32, shot.y.floor() as i32),
        (shot.vx / speed, shot.vy / speed),
        speed,
        form_look(shot.form),
        shot.scale,
        shot.empowered,
    );
}
