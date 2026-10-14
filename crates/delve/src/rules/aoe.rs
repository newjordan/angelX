//! One rule for every blow that lands on a crowd.
//!
//! A sweep, a spin, a burst or a blast used to land in full on everything
//! it reached, so a knight in the thick of a room did as much to ten
//! monsters as to one, and a leech on each of them mended faster than the
//! room could hurt. Now one stroke lands in full on the `FULL` monsters
//! nearest where it struck, and every monster past them takes `REST` in
//! the hundred of it.
//!
//! A stroke is one swing of a blade, one sweep of the axe spin, one beat
//! of a knight's morningstars, one burst of a ball, a bolt or a shell, one
//! firebomb, mine or gas flash, one Grand Chord, one Trebuchet shell. A
//! shot that passes through monsters counts them in the order it meets
//! them. Gas and burning ground poison and burn the nearest `FULL` in full
//! and the rest at `REST`. What a stroke does besides its damage (a stun,
//! a pull, a parried shot) still reaches all of them.
//!
//! A bomb and a Thunder Scroll stay room-wide: each is spent when used and
//! hits for a fixed sum whatever the knight holds, so no build multiplies
//! it. Morningstars strike on a beat (`ORBIT_EVERY`), not on every tick
//! they touch; lifesteal has its own limit beside the other mends
//! (`stakes`).

use super::*;

/// One stroke lands in full on at most this many monsters, nearest first.
pub const FULL: usize = 3;
/// Each monster past them takes this share of it, in percent.
pub const REST: u32 = 25;

/// What the monster `rank` places from a stroke (0 the nearest) takes of
/// its `damage`.
pub fn part(damage: u32, rank: usize) -> u32 {
    if rank < FULL || whole() {
        damage
    } else {
        (damage * REST).div_ceil(100)
    }
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// For the balance harness only: every stroke lands whole on all it
    /// reaches, as before the rule, to measure the rule against.
    pub static WHOLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(any(test, feature = "testing"))]
fn whole() -> bool {
    WHOLE.with(std::cell::Cell::get)
}

#[cfg(not(any(test, feature = "testing")))]
fn whole() -> bool {
    false
}

/// The living monsters a stroke from `at` reaches, nearest first.
pub(super) fn reached(
    enemies: &[Enemy],
    at: (f32, f32),
    reaches: impl Fn(&Enemy) -> bool,
) -> Vec<usize> {
    let mut near: Vec<(f32, usize)> = enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| e.hp > 0 && reaches(e))
        .map(|(i, e)| ((e.x - at.0).hypot(e.y - at.1), i))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    near.into_iter().map(|(_, i)| i).collect()
}

/// One stroke of `damage` from `at` on every living monster `reaches`
/// catches, shared as the rule says. Returns each monster struck, nearest
/// first, and what it took.
pub(super) fn strike(
    enemies: &mut [Enemy],
    at: (f32, f32),
    damage: u32,
    reaches: impl Fn(&Enemy) -> bool,
) -> Vec<(usize, u32)> {
    reached(enemies, at, reaches)
        .into_iter()
        .enumerate()
        .map(|(rank, i)| {
            let dealt = part(damage, rank);
            enemies[i].hp = enemies[i].hp.saturating_sub(dealt);
            (i, dealt)
        })
        .collect()
}
