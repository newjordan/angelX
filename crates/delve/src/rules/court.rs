//! The Relic Liege's court: a crowned guardian that never willingly shows
//! its back.
//!
//! It turns, no faster than `turn_rate`, to face the nearest knight. Its
//! relic ward lets a blow struck inside its gaze through at `WARD_TAKES`;
//! a blow at its back lands at `BACK_TAKES`. One knight holds its gaze while
//! the rest go round; a knight alone circles closer in than it can turn.
//! Spent after a charge-up it stands still, its back open. In its rage the
//! ward cracks: the front takes a blow whole and the back still more.
//!
//! A blow's side is known only where it is struck (`signatures::rear_bonus`,
//! from the swing's or the shot's origin), so the back's share is paid
//! there and the ward weighs every blow after (`tend_court`).
use super::*;
use bosses::Stage;

pub const ID: &str = "crypt-relic-liege";
/// The share of a blow its ward lets through, in hundredths.
pub const WARD_TAKES: u32 = 50;
/// The share a blow at its back lands, in hundredths. Cut from 150 on
/// 2026-10-25 by the duels (`together_boss_rules__tests`): with one knight
/// holding its gaze the rest stand at its back all fight, and at half again
/// a party cleared it faster than before it had a ward.
pub const BACK_TAKES: u32 = 110;
/// Ticks its word for the broken ward stays over it (`Enemy::cry`).
pub const CRY: u32 = 2 * HZ;
/// The fewest ticks between two rings of its ward, counted from the last
/// (`Run::sound_every`): the first blow on the ward always rings, and a
/// party's steady fire rings it twice a second, not four times.
const WARD_RING_EVERY: u64 = 15;

pub fn is(boss: &Boss) -> bool {
    boss.id == ID
}

/// Its ward is broken: its rage.
pub fn cracked(enemy: &Enemy, boss: &Boss) -> bool {
    is(boss) && boss.stage(enemy.hp, enemy.max_hp) == Stage::Rage
}

/// The share of any blow its ward lets through now, in hundredths.
pub fn ward(enemy: &Enemy, boss: &Boss) -> u32 {
    if !is(boss) || cracked(enemy, boss) { 100 } else { WARD_TAKES }
}

/// Degrees a second it turns at its stage of the fight.
pub fn turn_rate(enemy: &Enemy, boss: &Boss) -> f32 {
    match boss.stage(enemy.hp, enemy.max_hp) {
        Stage::Open => 70.0,
        Stage::Middle => 90.0,
        Stage::Rage => 110.0,
    }
}

/// `from` stands at its back: behind the line across its shoulders.
pub fn behind(enemy: &Enemy, from: (f32, f32)) -> bool {
    (from.0 - enemy.x) * enemy.dir.0 + (from.1 - enemy.y) * enemy.dir.1 < 0.0
}

/// What a blow at its back adds where it is struck, before its ward weighs
/// the whole: the back lands at `BACK_TAKES` through the ward.
pub fn rear_extra(enemy: &Enemy, boss: &Boss, from: (f32, f32), dealt: u32) -> u32 {
    if !is(boss) || !behind(enemy, from) {
        return 0;
    }
    let through = BACK_TAKES * 100 / ward(enemy, boss);
    dealt * through.saturating_sub(100) / 100
}

/// It turns toward the nearest knight, held to its rate; spent after a
/// charge-up it stands as it is.
pub(super) fn turn(enemy: &mut Enemy, boss: &Boss, heroes: &[(f32, f32)]) {
    if !is(boss) || enemy.stage == signatures::DAZED {
        return;
    }
    let Some(&(tx, ty)) = heroes.iter().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    }) else {
        return;
    };
    let want = (ty - enemy.y).atan2(tx - enemy.x);
    if enemy.dir == (0.0, 0.0) {
        enemy.dir = (want.cos(), want.sin());
        return;
    }
    let now = enemy.dir.1.atan2(enemy.dir.0);
    let off = (want - now).sin().atan2((want - now).cos());
    let step = turn_rate(enemy, boss).to_radians() * DT;
    let a = now + off.clamp(-step, step);
    enemy.dir = (a.cos(), a.sin());
}

impl Run {
    /// The ward on a tick, before its blows are marked (`mark_hits`): what
    /// the tick took from its health weighed again by `ward`, and its word
    /// when the ward breaks.
    pub(super) fn tend_court(&mut self) {
        let mut rang = false;
        let knights: Vec<(f32, f32)> = self.players.values().filter(|h| h.hp > 0).map(|h| (h.x, h.y)).collect();
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
                let share = ward(enemy, boss);
                let dealt = enemy.seen_hp - enemy.hp;
                let was = boss.stage(enemy.seen_hp, enemy.max_hp);
                enemy.hp = enemy.seen_hp.saturating_sub((dealt * share / 100).max(1)).max(1);
                rang |= share < 100;
                if share < 100 && !knights.iter().any(|&k| behind(enemy, k)) {
                    // "WARDED" beside it, the first few times its ward turns
                    // a blow and nobody stands at its back.
                    signatures::teach(enemy);
                }
                if was != Stage::Rage && cracked(enemy, boss) {
                    enemy.cry = CRY;
                    self.sounds.push("crack_open");
                }
            }
        }
        if rang {
            self.sound_every("shield_block", WARD_RING_EVERY);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/court__tests.rs"]
mod tests;
