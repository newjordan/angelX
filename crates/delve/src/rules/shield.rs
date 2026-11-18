//! A raised shield is a barrier at its face.
//!
//! - **The face.** A knight holding the guard key with a shield in hand
//!   (`Hero::shielding`) holds up a face: an arc `SHIELD_REACH` out from
//!   their middle, `SHIELD_ARC` degrees either side of the aim, about where
//!   the drawn shield stands (`render::overworld::arena::knights`). A
//!   monster's shot that crosses it is stopped there, at the face, and
//!   never reaches the body.
//! - **One check.** Every path a hostile shot takes to a knight asks the
//!   same two questions here: `meets` (a shot in flight, swept from where
//!   it was to where it is) and `turns` (what comes along a line: a beam, a
//!   bash). A shot that reaches the body coming at the face is turned too,
//!   as it always was, so a raised shield never lets through what it
//!   stopped before.
//! - **Special shots.** A hex, a frost (and its Rimeleap) or a bursting
//!   pattern shot is stopped like any other: no frog, no chill, no leap. A
//!   beam is stopped at the face for as long as it burns into it, and
//!   reaches no further down its line (`patterns::shine`). What falls from
//!   above (a lobbed glob, a fling, a keg, a mole's eruption) and what runs
//!   a knight down (a ram, a guardian's rip) are not shots and are not
//!   turned, as before.
//! - **What it shows.** Each stopped shot leaves one `feel::Block`: shards
//!   in the shot's colours off the face, a flash on the shield, the shield
//!   knocked back a pixel or two, a heavier shot a bigger burst.
//! - **What it takes.** A kite shield wears as a pavise's tower shield
//!   does (`critters::guard_struck`): what it turns counts, and at
//!   `critters::GUARD_BREAK` percent of the knight's health it is knocked
//!   aside with a clang, and stays down for `SHIELD_DOWN` before it can be
//!   raised again, its wear gone. Left alone for `SHIELD_CALM` (a volley's
//!   length), the wear drains away over `SHIELD_MEND` (`Wear`). The user,
//!   2026-10-21: "we need a shield bar, with auto-refill. its currently a
//!   bit OP in how much damage it can sustain so have it match the shield
//!   guys sustain".

use super::*;

pub const SHIELD_REACH: f32 = 0.9;
pub const SHIELD_ARC: f32 = 65.0;
pub const SHIELD_DOWN: u32 = critters::GUARD_DOWN;
pub const SHIELD_CALM: u32 = HZ;
pub const SHIELD_MEND: u32 = 2 * HZ;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meet {
    pub t: f32,
    pub shield: bool,
    pub at: (f32, f32),
}

pub fn turns(hero: &Hero, (dx, dy): (f32, f32)) -> bool {
    let speed = dx.hypot(dy);
    hero.shielding
        && speed > 0.0
        && -(dx * hero.aim_x + dy * hero.aim_y) / speed >= SHIELD_ARC.to_radians().cos()
}

pub fn face(hero: &Hero, centre: (f32, f32), toward: (f32, f32)) -> (f32, f32) {
    let (mut ux, mut uy) = unit(toward.0 - centre.0, toward.1 - centre.1);
    if ux * hero.aim_x + uy * hero.aim_y < SHIELD_ARC.to_radians().cos() {
        (ux, uy) = (hero.aim_x, hero.aim_y);
    }
    (centre.0 + ux * SHIELD_REACH, centre.1 + uy * SHIELD_REACH)
}

pub fn entry(a: (f32, f32), b: (f32, f32), c: (f32, f32), r: f32) -> Option<f32> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (fx, fy) = (a.0 - c.0, a.1 - c.1);
    let start = fx * fx + fy * fy - r * r;
    if start <= 0.0 {
        return Some(0.0);
    }
    let along = dx * dx + dy * dy;
    if along <= 0.0 {
        return None;
    }
    let half = fx * dx + fy * dy;
    let disc = half * half - along * start;
    if disc < 0.0 {
        return None;
    }
    let t = (-half - disc.sqrt()) / along;
    (0.0..=1.0).contains(&t).then_some(t)
}

pub fn meets(hero: &Hero, centre: (f32, f32), a: (f32, f32), b: (f32, f32), body: f32) -> Option<Meet> {
    let cos_arc = SHIELD_ARC.to_radians().cos();
    let at_face = hero
        .shielding
        .then(|| entry(a, b, centre, SHIELD_REACH))
        .flatten()
        .filter(|&t| t > 0.0)
        .map(|t| (t, (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)))
        .filter(|&(_, p)| {
            let (ux, uy) = unit(p.0 - centre.0, p.1 - centre.1);
            ux * hero.aim_x + uy * hero.aim_y >= cos_arc
        });
    if let Some((t, at)) = at_face {
        return Some(Meet { t, shield: true, at });
    }
    if !hit_segment(a.0, a.1, b.0, b.1, centre.0, centre.1, body) {
        return None;
    }
    let t = entry(a, b, centre, body).unwrap_or(0.0);
    if turns(hero, (b.0 - a.0, b.1 - a.1)) {
        return Some(Meet { t, shield: true, at: face(hero, centre, a) });
    }
    Some(Meet { t, shield: false, at: b })
}

pub fn weight(shot: &Projectile) -> u8 {
    let by_damage = 1 + u8::from(shot.damage >= 14) + u8::from(shot.damage >= 22);
    (by_damage + u8::from(shot.traits.path.bursts())).min(3)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wear {
    #[serde(default, skip_serializing_if = "zero")]
    pub worn: u32,
    #[serde(default, skip_serializing_if = "zero")]
    pub calm: u32,
    #[serde(default, skip_serializing_if = "zero")]
    pub down: u32,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub turned: u32,
}

impl Wear {
    pub fn fresh(&self) -> bool {
        self.worn == 0 && self.calm == 0 && self.down == 0
    }

    pub fn most(max_hp: u32) -> u32 {
        max_hp.max(1) * critters::GUARD_BREAK
    }

    pub fn left(&self, max_hp: u32) -> f32 {
        if self.down > 0 {
            return 0.0;
        }
        1.0 - (self.worn as f32 / Wear::most(max_hp) as f32).min(1.0)
    }

    pub fn back(&self) -> f32 {
        1.0 - self.down.min(SHIELD_DOWN) as f32 / SHIELD_DOWN as f32
    }

    pub fn showing(&self) -> bool {
        self.worn > 0 || self.down > 0
    }

    pub(super) fn rest(&mut self, max_hp: u32) {
        if self.down > 0 {
            self.down -= 1;
        } else if self.calm > 0 {
            self.calm -= 1;
        } else {
            self.worn = self.worn.saturating_sub(Wear::most(max_hp).div_ceil(SHIELD_MEND));
        }
    }

    pub(super) fn take(&mut self, damage: u32, max_hp: u32) -> bool {
        self.turned = self.turned.saturating_add(damage);
        self.worn = self.worn.saturating_add(damage.max(1) * 100);
        self.calm = SHIELD_CALM;
        if self.worn < Wear::most(max_hp) {
            return false;
        }
        *self = Wear {
            down: SHIELD_DOWN,
            turned: self.turned,
            ..Wear::default()
        };
        true
    }
}

impl Hero {
    pub fn shield_turned(&mut self, damage: u32) -> bool {
        if self.guard != cards::Guard::Shield {
            self.shield_wear.turned = self.shield_wear.turned.saturating_add(damage);
            return false;
        }
        let knocked = self.shield_wear.take(damage, self.max_hp);
        if knocked {
            self.shielding = false;
        }
        knocked
    }
}

impl Run {
    pub(super) fn shield_block(&mut self, id: u32, at: (f32, f32), out: (f32, f32), way: (f32, f32), kind: Shot, weight: u8) {
        self.sounds.push("shield_block");
        self.feel.block(id, at, out, way, kind, weight);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_shield__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-home"))]
#[path = "../../tests/rules/shield_bend_cases__tests.rs"]
mod shield_bend_cases_tests;
