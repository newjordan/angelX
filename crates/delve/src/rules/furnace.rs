//! Cinderjaw's furnace: the guardian whose fight is a rule of its own.
//!
//! Its iron lets a blow through at `ARMOR_TAKES` until it breathes
//! (`Windup::Inhale`). The breath vents the furnace: for `VENT` ticks past
//! its usual recovery it stands open, and its core takes `VENT_TAKES`. One
//! knight draws the breath; the rest are at its chest when the door swings.
//! From the middle of the fight it is stoked: each leap it lands leaves the
//! floor burning and breaks the stone round it (`rubble`). In its rage it runs molten: the iron is gone, every blow
//! lands whole, and burning ground follows it about. Every number is fixed.
use super::*;
use bosses::{Stage, Windup};
use skills::Blaze;

pub const ID: &str = "cinderjaw";
/// The share of a blow its iron lets through, in hundredths.
pub const ARMOR_TAKES: u32 = 30;
/// The share its open core takes, in hundredths.
pub const VENT_TAKES: u32 = 150;
/// Ticks the furnace stands open after a breath, past `signatures::RECOVER`.
/// Halved 2026-10-25 by the duels (`together_boss_rules__tests`): at three
/// seconds a party that plays the rule finished in one breath, no slower
/// than the guardian before its rule; at a second and a half it takes a
/// tenth to a half longer, and a party that ignores the breath longer still.
pub const VENT: u32 = 3 * HZ / 2;
/// A stoked landing's burning ground.
pub const PIT_LIFE: u32 = 5 * HZ;
pub const PIT_RADIUS: f32 = 1.8;
/// Its molten wake: a patch each `WAKE_EVERY` ticks it walks.
pub const WAKE_EVERY: u32 = HZ / 2;
pub const WAKE_LIFE: u32 = 5 * HZ / 2;
pub const WAKE_RADIUS: f32 = 0.9;
/// The most burning ground of its own it keeps at once; the oldest goes.
pub const BLAZES_MOST: usize = 10;
/// Ticks its word for a new stage of the fight stays over it (`Enemy::cry`).
pub const CRY: u32 = 2 * HZ;
/// The fewest ticks between two clangs of its iron, counted from the last
/// (`Run::sound_every`): the first glancing blow always rings, and four
/// knights landing three hits a second hear it twice a second, not five.
const CLANG_EVERY: u64 = 15;

pub fn is(boss: &Boss) -> bool {
    boss.id == ID
}

/// Its furnace stands open: spent after a breath.
pub fn venting(enemy: &Enemy, boss: &Boss) -> bool {
    is(boss)
        && enemy.stage == signatures::DAZED
        && signatures::winding(enemy, boss).is_some_and(|up| up.kind == Windup::Inhale)
}

/// Past its rage: no iron left.
pub fn molten(enemy: &Enemy, boss: &Boss) -> bool {
    is(boss) && boss.stage(enemy.hp, enemy.max_hp) == Stage::Rage
}

pub fn armored(enemy: &Enemy, boss: &Boss) -> bool {
    is(boss) && !venting(enemy, boss) && !molten(enemy, boss)
}

/// The share of a blow it takes now, in hundredths.
pub fn takes(enemy: &Enemy, boss: &Boss) -> u32 {
    if !is(boss) || molten(enemy, boss) {
        100
    } else if venting(enemy, boss) {
        VENT_TAKES
    } else {
        ARMOR_TAKES
    }
}

/// Ticks its breath leaves it open, past the usual recovery.
pub fn vent_for(boss: &Boss, up: &bosses::ChargeUp) -> u32 {
    if is(boss) && up.kind == Windup::Inhale { VENT } else { 0 }
}

/// Its own flame never burns it.
pub fn fireproof(enemy: &Enemy, bosses: &[Boss]) -> bool {
    enemy
        .boss
        .and_then(|i| bosses.get(usize::from(i)))
        .is_some_and(is)
}

impl Run {
    /// The furnace on a tick, before its blows are marked (`mark_hits`):
    /// what the tick took from its health weighed again by `takes`, its
    /// word for a new stage of the fight, a stoked landing's pit and a
    /// molten wake.
    pub(super) fn tend_furnace(&mut self) {
        let mut clang = false;
        let mut blazes = Vec::new();
        for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
            let Some(boss) = enemy.boss.and_then(|i| self.bosses.get(usize::from(i))) else {
                continue;
            };
            if !is(boss) {
                continue;
            }
            enemy.cry = enemy.cry.saturating_sub(1);
            enemy.hint = enemy.hint.saturating_sub(1);
            if enemy.seen_hp > enemy.hp {
                // A killing blow stands: only the living are weighed.
                let dealt = enemy.seen_hp - enemy.hp;
                let share = takes(enemy, boss);
                let weighed = (dealt * share / 100).max(1);
                clang |= share < 100;
                if share < 100 {
                    // "IRON" beside it, the first few times its iron turns a blow.
                    signatures::teach(enemy);
                }
                let before = boss.stage(enemy.seen_hp, enemy.max_hp);
                enemy.hp = enemy.seen_hp.saturating_sub(weighed).max(1);
                let now = boss.stage(enemy.hp, enemy.max_hp);
                if now > before {
                    enemy.cry = CRY;
                    self.sounds.push(if now == Stage::Rage { "armour_fall" } else { "vent_fire" });
                }
            }
            let up = signatures::winding(enemy, boss);
            let stage = boss.stage(enemy.hp, enemy.max_hp);
            if enemy.stage == signatures::DAZED
                && enemy.timer == 0
                && stage >= Stage::Middle
                && up.is_some_and(|u| u.kind == Windup::Leap)
            {
                blazes.push((enemy.x, enemy.y, PIT_RADIUS, PIT_LIFE, true));
            }
            if stage == Stage::Rage
                && enemy.stage == 0
                && enemy.frozen == 0
                && enemy.age.is_multiple_of(WAKE_EVERY)
            {
                blazes.push((enemy.x, enemy.y, WAKE_RADIUS, WAKE_LIFE, false));
            }
        }
        if clang {
            self.sound_every("clang", CLANG_EVERY);
        }
        // A stoked landing breaks the stone round it too (`rubble`), once.
        for (x, y, radius, life, landing) in blazes {
            if self.light_furnace_ground(x, y, radius, life) && landing {
                self.shatter((x, y), signatures::LANDING + rubble::SHATTER_PAST);
            }
        }
    }

    /// Burning ground of its own, never twice on one spot, the oldest gone
    /// past `BLAZES_MOST`; true if it caught.
    fn light_furnace_ground(&mut self, x: f32, y: f32, radius: f32, life: u32) -> bool {
        let blazes = &mut self.works.blazes;
        if blazes
            .iter()
            .any(|b| b.hostile() && b.age < 2 && (b.x - x).hypot(b.y - y) < 0.5)
        {
            return false;
        }
        // A knight's own fire is never put out for it.
        while blazes.iter().filter(|b| b.hostile()).count() >= BLAZES_MOST || blazes.len() >= 24 {
            let Some(i) = blazes.iter().position(Blaze::hostile) else {
                return false;
            };
            blazes.remove(i);
        }
        blazes.push(Blaze {
            x,
            y,
            age: 0,
            life,
            owner: 0,
            power: skills::BURN_BASE,
            radius,
        });
        true
    }
}

#[cfg(test)]
#[path = "../../tests/rules/furnace__tests.rs"]
mod tests;
