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
pub const WARD_TAKES: u32 = 50;
pub const BACK_TAKES: u32 = 110;
pub const CRY: u32 = 2 * HZ;
const WARD_RING_EVERY: u64 = 15;

pub fn is(boss: &Boss) -> bool {
    boss.id == ID
}

pub fn cracked(enemy: &Enemy, boss: &Boss) -> bool {
    is(boss) && boss.stage(enemy.hp, enemy.max_hp) == Stage::Rage
}

pub fn ward(enemy: &Enemy, boss: &Boss) -> u32 {
    if !is(boss) || cracked(enemy, boss) { 100 } else { WARD_TAKES }
}

pub fn turn_rate(enemy: &Enemy, boss: &Boss) -> f32 {
    match boss.stage(enemy.hp, enemy.max_hp) {
        Stage::Open => 70.0,
        Stage::Middle => 90.0,
        Stage::Rage => 110.0,
    }
}

pub fn behind(enemy: &Enemy, from: (f32, f32)) -> bool {
    (from.0 - enemy.x) * enemy.dir.0 + (from.1 - enemy.y) * enemy.dir.1 < 0.0
}

pub fn rear_extra(enemy: &Enemy, boss: &Boss, from: (f32, f32), dealt: u32) -> u32 {
    if !is(boss) || !behind(enemy, from) {
        return 0;
    }
    let through = BACK_TAKES * 100 / ward(enemy, boss);
    dealt * through.saturating_sub(100) / 100
}

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
                let share = ward(enemy, boss);
                let dealt = enemy.seen_hp - enemy.hp;
                let was = boss.stage(enemy.seen_hp, enemy.max_hp);
                enemy.hp = enemy.seen_hp.saturating_sub((dealt * share / 100).max(1)).max(1);
                rang |= share < 100;
                if share < 100 && !knights.iter().any(|&k| behind(enemy, k)) {
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
