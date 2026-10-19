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

use super::*;

/// How far ahead of a knight's middle a raised shield's face stands, in
/// arena units: past the body's reach (`HERO_RADIUS` and a shot's
/// thickness, 0.58), at the outer edge of the shield as it is drawn held
/// out to the side (seven pixels).
pub const SHIELD_REACH: f32 = 0.9;
/// A shield's face spans this many degrees either side of its aim.
pub const SHIELD_ARC: f32 = 65.0;

/// What a shot in flight meets at a knight, and where along its step this
/// tick (0 where it was, 1 where it is).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meet {
    pub t: f32,
    /// Stopped by the raised shield, at `at` on its face.
    pub shield: bool,
    pub at: (f32, f32),
}

/// Whether `hero`'s raised shield faces what comes along `(dx, dy)` (the
/// way it travels).
pub fn turns(hero: &Hero, (dx, dy): (f32, f32)) -> bool {
    let speed = dx.hypot(dy);
    hero.shielding
        && speed > 0.0
        && -(dx * hero.aim_x + dy * hero.aim_y) / speed >= SHIELD_ARC.to_radians().cos()
}

/// The point on `hero`'s shield face (round `centre`) that looks toward
/// `toward`; the face's middle when that is off the arc.
pub fn face(hero: &Hero, centre: (f32, f32), toward: (f32, f32)) -> (f32, f32) {
    let (mut ux, mut uy) = unit(toward.0 - centre.0, toward.1 - centre.1);
    if ux * hero.aim_x + uy * hero.aim_y < SHIELD_ARC.to_radians().cos() {
        (ux, uy) = (hero.aim_x, hero.aim_y);
    }
    (centre.0 + ux * SHIELD_REACH, centre.1 + uy * SHIELD_REACH)
}

/// Where a step from `a` to `b` first comes within `r` of `c`, as a share
/// of the step; 0 when it starts there.
fn entry(a: (f32, f32), b: (f32, f32), c: (f32, f32), r: f32) -> Option<f32> {
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

/// What a hostile shot stepping from `a` to `b` meets at `hero`, whose
/// middle is at `centre` and whose body a shot touches within `body`:
/// the raised shield's face first, or the body (turned there if it came
/// at the face). `None` if it passes them by.
pub fn meets(hero: &Hero, centre: (f32, f32), a: (f32, f32), b: (f32, f32), body: f32) -> Option<Meet> {
    let cos_arc = SHIELD_ARC.to_radians().cos();
    // The face: crossing into the shield's reach within its arc.
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
        // Already inside the face's reach, coming at it: stopped on the
        // face, toward where it came from.
        return Some(Meet { t, shield: true, at: face(hero, centre, a) });
    }
    Some(Meet { t, shield: false, at: b })
}

/// How heavy a shot is to a shield, 1 to 3: by what it would have dealt,
/// and a bursting pattern shot a step heavier.
pub fn weight(shot: &Projectile) -> u8 {
    let by_damage = 1 + u8::from(shot.damage >= 14) + u8::from(shot.damage >= 22);
    (by_damage + u8::from(shot.traits.path.bursts())).min(3)
}

impl Run {
    /// Knight `id`'s raised shield stopped something of `kind` coming along
    /// `way`, at `at` on its face (which looks out along `out`): the clang,
    /// and the block for the picture.
    pub(super) fn shield_block(&mut self, id: u32, at: (f32, f32), out: (f32, f32), way: (f32, f32), kind: Shot, weight: u8) {
        self.sounds.push("shield_block");
        self.feel.block(id, at, out, way, kind, weight);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_shield__tests.rs"]
mod tests;
