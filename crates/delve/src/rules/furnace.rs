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
pub const ARMOR_TAKES: u32 = 30;
pub const VENT_TAKES: u32 = 150;
pub const VENT: u32 = 3 * HZ / 2;
pub const PIT_LIFE: u32 = 5 * HZ;
pub const PIT_RADIUS: f32 = 1.8;
pub const WAKE_EVERY: u32 = HZ / 2;
pub const WAKE_LIFE: u32 = 5 * HZ / 2;
pub const WAKE_RADIUS: f32 = 0.9;
pub const BLAZES_MOST: usize = 10;
pub const CRY: u32 = 2 * HZ;
const CLANG_EVERY: u64 = 15;

pub fn is(boss: &Boss) -> bool {
    boss.id == ID
}

pub fn venting(enemy: &Enemy, boss: &Boss) -> bool {
    is(boss)
        && enemy.stage == signatures::DAZED
        && signatures::winding(enemy, boss).is_some_and(|up| up.kind == Windup::Inhale)
}

pub fn molten(enemy: &Enemy, boss: &Boss) -> bool {
    is(boss) && boss.stage(enemy.hp, enemy.max_hp) == Stage::Rage
}

pub fn armored(enemy: &Enemy, boss: &Boss) -> bool {
    is(boss) && !venting(enemy, boss) && !molten(enemy, boss)
}

pub fn takes(enemy: &Enemy, boss: &Boss) -> u32 {
    if !is(boss) || molten(enemy, boss) {
        100
    } else if venting(enemy, boss) {
        VENT_TAKES
    } else {
        ARMOR_TAKES
    }
}

pub fn vent_for(boss: &Boss, up: &bosses::ChargeUp) -> u32 {
    if is(boss) && up.kind == Windup::Inhale { VENT } else { 0 }
}

pub fn fireproof(enemy: &Enemy, bosses: &[Boss]) -> bool {
    enemy
        .boss
        .and_then(|i| bosses.get(usize::from(i)))
        .is_some_and(is)
}

impl Run {
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
                let dealt = enemy.seen_hp - enemy.hp;
                let share = takes(enemy, boss);
                let weighed = (dealt * share / 100).max(1);
                clang |= share < 100;
                if share < 100 {
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
        for (x, y, radius, life, landing) in blazes {
            if self.light_furnace_ground(x, y, radius, life) && landing {
                self.shatter((x, y), signatures::LANDING + rubble::SHATTER_PAST);
            }
        }
    }

    fn light_furnace_ground(&mut self, x: f32, y: f32, radius: f32, life: u32) -> bool {
        let blazes = &mut self.works.blazes;
        if blazes
            .iter()
            .any(|b| b.hostile() && b.age < 2 && (b.x - x).hypot(b.y - y) < 0.5)
        {
            return false;
        }
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
