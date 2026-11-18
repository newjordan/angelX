//! Wishes made literal: what a player's words promise, done in the game and
//! shown on the field. A wish for bigger arrows makes the arrows bigger,
//! drawn larger and striking wider; fire sets what they hit burning, frost
//! slows it, venom poisons it, a hard shot shoves it back, a boomerang
//! comes home to the hand, a fast one flies faster. Each is a card word
//! (`cards::SPECS`) that a held card adds to its knight's shots
//! (`ShotTraits`), never a stand-in damage bump.

use super::*;

pub const SIZE_REACH: f32 = 0.3;

pub const BASE_PCT: u32 = 55;
pub const BEAT_REF: u32 = 12;
pub const RATE_MIN_PCT: u32 = 50;
pub const RATE_MAX_PCT: u32 = 150;
pub const FLOOR_PCT: u32 = 30;
pub const SIZE_STEP_PCT: u32 = 50;
pub const HIT_REACH: f32 = 1.2;

pub fn rate_pct(beat: u8) -> u32 {
    if beat == 0 {
        return 100;
    }
    (10_000 * u32::from(beat) / BEAT_REF).isqrt().clamp(RATE_MIN_PCT, RATE_MAX_PCT)
}

#[cfg(test)]
thread_local! {
    pub static LEGACY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn legacy() -> bool {
    #[cfg(test)]
    return LEGACY.with(std::cell::Cell::get);
    #[cfg(not(test))]
    false
}

pub const KNOCK_STEP: f32 = 0.6;
pub const CHILL_FOE: u32 = lich::CHILL;
pub const CHILL_PACE: f32 = lich::CHILL_PACE;
const BOOMERANG_HOME: u32 = 2 * HZ;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Burn,
    Chill,
    Poison,
}

impl Projectile {
    pub fn wished(&self) -> bool {
        let t = &self.traits;
        t.size > 0 || t.burn > 0 || t.chill > 0 || t.poison > 0 || t.boomerang
    }

    pub fn sized(&self) -> bool {
        !self.hostile && self.traits.owner > 0 && !self.traits.borrowed
    }

    pub fn scale(&self) -> f32 {
        if !self.sized() || legacy() {
            return 1.0;
        }
        let size = 100 + SIZE_STEP_PCT * u32::from(self.traits.size.min(3));
        let pct = BASE_PCT * rate_pct(self.traits.beat) * size;
        (pct.max(FLOOR_PCT * 10_000)) as f32 / 1_000_000.0
    }

    pub fn reach(&self) -> f32 {
        if !self.sized() || legacy() {
            return SIZE_REACH * f32::from(self.traits.size.min(3));
        }
        HIT_REACH * (self.scale() - BASE_PCT as f32 / 100.0).max(0.0)
    }

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

pub fn shape(shots: &mut [Projectile]) {
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

pub fn knock(bullet: &Projectile) -> f32 {
    feel::knock(bullet.kind) + KNOCK_STEP * f32::from(bullet.traits.knock.min(3))
}

pub(super) fn chill(enemy: &mut Enemy, bullet: &Projectile) {
    if bullet.traits.chill > 0 && !enemy.is_boss() {
        let ticks = CHILL_FOE * u32::from(bullet.traits.chill.min(3));
        enemy.chilled = enemy.chilled.max(ticks);
    }
}

pub(super) fn slow(enemy: &mut Enemy, from: (f32, f32), grid: &Grid) {
    if enemy.chilled == 0 {
        return;
    }
    enemy.chilled -= 1;
    tune::foe_step(enemy, from, CHILL_PACE, grid);
}

pub fn turn_home(bullet: &mut Projectile) -> bool {
    if bullet.traits.boomerang && bullet.traits.back == 1 {
        bullet.traits.back = 2;
        bullet.traits.out = 0;
        return true;
    }
    false
}

pub(super) fn home_speed(bullet: &Projectile, blade: f32) -> f32 {
    if bullet.traits.boomerang {
        bullet.vx.hypot(bullet.vy).max(1.0)
    } else {
        blade
    }
}

impl Run {
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
