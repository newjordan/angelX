//! Wishes made literal: what a player's words promise, done in the game and
//! shown on the field. A wish for bigger arrows makes the arrows bigger,
//! drawn larger and striking wider; fire sets what they hit burning, frost
//! slows it, venom poisons it, a hard shot shoves it back, a boomerang
//! comes home to the hand, a fast one flies faster. Each is a card word
//! (`cards::SPECS`) that a held card adds to its knight's shots
//! (`ShotTraits`), never a stand-in damage bump.

use super::*;

/// How much wider a shot strikes, units a size step, for a shot that is not
/// a knight's own (a knight's is `HIT_REACH` of its `scale`).
pub const SIZE_REACH: f32 = 0.3;

// A knight's shot is smaller the faster its arm fires, and bigger the
// slower (the operator, 2026-10-22: "the players art made bullets are
// always too big anyways, and then when they get double or triple shot and
// bounce, the screen is nothing but bullets... reduce the size of bullets
// considerably, and tie bullet size to weapon firing speed as a ratio").
//
//   scale = BASE x clamp(sqrt(beat / BEAT_REF), RATE_MIN, RATE_MAX) x size wish
//
// where `beat` is the ticks between two pulls of the arm that loosed it,
// rate and quick draws counted (`Hero::cooldown`). 1.0 is the size a shot
// was drawn at before. All of it is whole numbers until the last division,
// so the host and every seat, native or in a browser, agree to the bit.
/// Percent of its old size a shot is drawn at before the beat and the wish.
pub const BASE_PCT: u32 = 55;
/// The beat a shot is neither smaller nor bigger for, ticks: a stone's.
pub const BEAT_REF: u32 = 12;
/// The least and most the beat may make of it, percent.
pub const RATE_MIN_PCT: u32 = 50;
pub const RATE_MAX_PCT: u32 = 150;
/// The smallest a shot is ever drawn or hits, percent of its old size: a
/// needle is still seen and still connects.
pub const FLOOR_PCT: u32 = 30;
/// Percent of its old size each size step adds.
pub const SIZE_STEP_PCT: u32 = 50;
/// How wide a shot strikes past a monster's body, units, for each whole
/// `scale` it is drawn bigger than `BASE_PCT` (the size it is drawn at with
/// no beat and no wish). At that size, and under it, it strikes as a point
/// (the monster's own body is its target), so a fast needle still connects
/// and the plain shot hits as it always did; over it, what is drawn is what
/// hits: a slow heavy ball or a wished shot strikes as wide as it looks.
pub const HIT_REACH: f32 = 1.2;

/// What the beat makes of a shot's size, percent: the square root of the
/// beat over `BEAT_REF`, held between `RATE_MIN_PCT` and `RATE_MAX_PCT`. A
/// shot with no arm behind it (`beat` 0: a spell, a ring) is as it was.
pub fn rate_pct(beat: u8) -> u32 {
    if beat == 0 {
        return 100;
    }
    (10_000 * u32::from(beat) / BEAT_REF).isqrt().clamp(RATE_MIN_PCT, RATE_MAX_PCT)
}

#[cfg(test)]
thread_local! {
    /// On, a sim or a frame is made as the shots were before they scaled
    /// (`scale` 1, the old hit, the party's share held): the same run,
    /// before and after, in one build.
    pub static LEGACY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn legacy() -> bool {
    #[cfg(test)]
    return LEGACY.with(std::cell::Cell::get);
    #[cfg(not(test))]
    false
}

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

    /// A knight's own shot, sized by its arm's beat and its wishes: not a
    /// monster's, not a monster's shot a knight loosed from its body.
    pub fn sized(&self) -> bool {
        !self.hostile && self.traits.owner > 0 && !self.traits.borrowed
    }

    /// How big it is drawn and strikes, as a share of what a shot was
    /// before (1.0): half of it, smaller the faster its arm fires and
    /// bigger the slower (`rate_pct`), and bigger a size step at a time.
    /// Never under `FLOOR_PCT`. A monster's shot is 1.0.
    pub fn scale(&self) -> f32 {
        if !self.sized() || legacy() {
            return 1.0;
        }
        let size = 100 + SIZE_STEP_PCT * u32::from(self.traits.size.min(3));
        let pct = BASE_PCT * rate_pct(self.traits.beat) * size;
        (pct.max(FLOOR_PCT * 10_000)) as f32 / 1_000_000.0
    }

    /// How much wider than a point it strikes: `HIT_REACH` for each whole
    /// `scale` it is drawn over `BASE_PCT`, so what is drawn is what hits.
    pub fn reach(&self) -> f32 {
        if !self.sized() || legacy() {
            return SIZE_REACH * f32::from(self.traits.size.min(3));
        }
        HIT_REACH * (self.scale() - BASE_PCT as f32 / 100.0).max(0.0)
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

/// How far a hit shoves, past its kind's own kick.
pub fn knock(bullet: &Projectile) -> f32 {
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
pub fn turn_home(bullet: &mut Projectile) -> bool {
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
