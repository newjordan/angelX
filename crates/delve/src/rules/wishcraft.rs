//! Wishes made literal: what a player's words promise, done in the game and
//! shown on the field. A wish for bigger arrows makes the arrows bigger,
//! drawn larger and striking wider; fire sets what they hit burning, frost
//! slows it, venom poisons it, a hard shot shoves it back, a boomerang
//! comes home to the hand, a fast one flies faster. Each is a card word
//! (`cards::SPECS`) that a held card adds to its knight's shots
//! (`ShotTraits`), never a stand-in damage bump.

use super::*;

/// How much wider a shot strikes, units a size step. The renderer draws a
/// shot `2 + size` times its sprite (`shot_scale`).
pub const SIZE_REACH: f32 = 0.3;
/// How much further a hit shoves, units a knock step.
pub const KNOCK_STEP: f32 = 0.6;
/// A chilling hit's ticks at N 1 (the Lich's chill on a knight), and how
/// fast a chilled monster walks.
pub const CHILL_FOE: u32 = lich::CHILL;
pub const CHILL_PACE: f32 = lich::CHILL_PACE;
/// A boomerang flies its whole range out, then has this long besides to
/// find the hand.
const BOOMERANG_HOME: u32 = 2 * HZ;

/// The colour a branded shot is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Burn,
    Chill,
    Poison,
}

impl Projectile {
    /// It carries a wish's look: bigger, tinted, or coming back.
    pub fn wished(&self) -> bool {
        let t = &self.traits;
        t.size > 0 || t.burn > 0 || t.chill > 0 || t.poison > 0 || t.boomerang
    }

    /// How many times its sprite it is drawn: twice, and once more a size.
    pub fn shot_scale(&self) -> i32 {
        2 + i32::from(self.traits.size.min(3))
    }

    /// How much wider than a point it strikes.
    pub fn reach(&self) -> f32 {
        SIZE_REACH * f32::from(self.traits.size.min(3))
    }

    /// The tint a branded shot shows: its fire, frost or venom.
    pub fn tint(&self) -> Option<Tint> {
        let t = &self.traits;
        if t.burn > 0 {
            Some(Tint::Burn)
        } else if t.chill > 0 {
            Some(Tint::Chill)
        } else if t.poison > 0 {
            Some(Tint::Poison)
        } else {
            None
        }
    }
}

/// This tick's knights' shots as they leave, shaped by their wishes: a
/// burning shot is aflame (it lights gas as Fire Arrows do), a fast one
/// flies faster as far, a boomerang flies out to turn for home.
pub(super) fn shape(shots: &mut [Projectile]) {
    for shot in shots.iter_mut().filter(|s| !s.hostile && s.traits.owner > 0) {
        if shot.traits.burn > 0 {
            shot.traits.fire = true;
        }
        if shot.traits.velocity > 0 {
            let k = (100.0 + f32::from(shot.traits.velocity)) / 100.0;
            (shot.vx, shot.vy) = (shot.vx * k, shot.vy * k);
            shot.ttl = (shot.ttl as f32 / k).ceil() as u32;
        }
        if shot.traits.boomerang && shot.traits.back == 0 {
            let out = shot.ttl.clamp(4, u32::from(u16::MAX));
            shot.traits.back = 1;
            shot.traits.out = out as u16;
            shot.ttl = out * 2 + BOOMERANG_HOME;
        }
    }
}

/// How far a hit shoves, past its kind's own kick.
pub(super) fn knock(bullet: &Projectile) -> f32 {
    feel::knock(bullet.kind) + KNOCK_STEP * f32::from(bullet.traits.knock.min(3))
}

/// A chilling hit: the monster walks slowly for a while. A guardian
/// shrugs it, as it stands its ground against a shove.
pub(super) fn chill(enemy: &mut Enemy, bullet: &Projectile) {
    if bullet.traits.chill > 0 && !enemy.is_boss() {
        let ticks = CHILL_FOE * u32::from(bullet.traits.chill.min(3));
        enemy.chilled = enemy.chilled.max(ticks);
    }
}

/// A chilled monster's step: part of the way it went, and its chill wears
/// off a tick (`tune::foe_step`).
pub(super) fn slow(enemy: &mut Enemy, from: (f32, f32), grid: &Grid) {
    if enemy.chilled == 0 {
        return;
    }
    enemy.chilled -= 1;
    tune::foe_step(enemy, from, CHILL_PACE, grid);
}

/// A boomerang that stops on a monster turns for home instead of falling;
/// true when it turned.
pub(super) fn turn_home(bullet: &mut Projectile) -> bool {
    if bullet.traits.boomerang && bullet.traits.back == 1 {
        bullet.traits.back = 2;
        bullet.traits.out = 0;
        return true;
    }
    false
}

/// A homing boomerang's speed back to the hand: its own (the thrown
/// blade's is the blade's).
pub(super) fn home_speed(bullet: &Projectile, blade: f32) -> f32 {
    if bullet.traits.boomerang {
        bullet.vx.hypot(bullet.vy).max(1.0)
    } else {
        blade
    }
}

impl Run {
    /// Poisoning hits this tick: (the foe, the knight, how strong). The
    /// gas skill's poison, N times over.
    pub(super) fn poison_hits(&mut self, hits: Vec<(u32, u8, u8)>) {
        for (foe, owner, n) in hits {
            let owner = u32::from(owner);
            let base = self
                .players
                .get(&owner)
                .map_or(skills::POISON_BASE, |h| h.scaled(skills::POISON_BASE));
            skills::afflict(
                &mut self.works.banes,
                foe,
                skills::Bane::Poison,
                base * u32::from(n.clamp(1, 3)),
                owner,
            );
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_wishcraft__tests.rs"]
mod tests;
