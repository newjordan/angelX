//! The axe spin: the dwarf's guard. Space turns her on the spot with both
//! axes out, and everything within reach is cut twice, once as each axe
//! comes round (each sweep one stroke, `aoe`: the nearest three in full).
//! Shots in the circle are cut down. She walks on while it lasts, slowly,
//! and takes a breath before the next.

use super::*;

pub const TICKS: u32 = 14;
pub const REACH: f32 = 3.0;
const DAMAGE: u32 = 26;
pub const REARM: u32 = 2 * HZ + HZ / 2;
pub(super) const PACE: f32 = 0.8;

pub fn sweeping(age: u32) -> bool {
    age == 0 || age == TICKS / 2
}

pub(super) fn sweep(
    owner: u32,
    hero: &Hero,
    enemies: &mut [Enemy],
    projectiles: &mut [Projectile],
    sounds: &mut Vec<&'static str>,
) -> (u32, u32) {
    let damage = hero.scaled(DAMAGE);
    let (hx, hy) = (hero.x, hero.y);
    let (mut dealt, mut cut) = (0, 0);
    for (i, took) in super::aoe::strike(enemies, (hx, hy), damage, |e| {
        (e.x - hx).hypot(e.y - hy) <= REACH + e.radius()
    }) {
        enemies[i].wits.foe = owner;
        dealt += took;
        cut += u32::from(took > 0);
        sounds.push("hit");
    }
    for shot in projectiles.iter_mut().filter(|p| p.hostile) {
        if (shot.x - hero.x).hypot(shot.y - hero.y) <= REACH + 0.6 {
            shot.ttl = 0;
        }
    }
    (dealt, cut)
}
