//! Peasant tuning and Desertion. All durations use simulation ticks.
use super::*;
pub const PACE: f32 = 1.15;
pub const SPRINT_PACE: f32 = 1.8;
pub const SPRINT_TICKS: u32 = 2 * HZ;
pub const DECOY_TICKS: u32 = 3 * HZ;
pub const COLLAPSE_TICKS: u32 = 9;
pub const DECOY_RADIUS: f32 = 0.6;
pub const STONE_KNOCK: f32 = 0.28;
pub const STONE_HEIGHT: f32 = 1.5;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scarecrow {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub until: u64,
    pub collapse_until: u64,
}
impl Scarecrow { pub fn active(&self, tick: u64) -> bool { tick < self.until } }
pub(super) fn idle(n: &u32) -> bool { *n == 0 }
impl Hero {
    pub fn peasant(&self) -> bool { classes::of(self).is_some_and(|c| c.kit == "peasant") }
    pub fn targetable(&self) -> bool { self.hp > 0 && !self.stone && self.flee_ticks == 0 }
}
/// Start before the cast tick's movement. own_keys consumes the extra tick,
/// leaving sixty movement/damage/targeting ticks including this one.
pub(super) fn start(hero: &mut Hero, owner: u32, tick: u64, decoys: &mut Vec<Scarecrow>) {
    hero.flee_ticks = SPRINT_TICKS + 1;
    hero.invulnerable = hero.invulnerable.max(SPRINT_TICKS + 1);
    hero.dash_ticks = 0;
    hero.spin = 0;
    hero.whirl = 0;
    hero.swing = 0;
    hero.shielding = false;
    hero.walling = false;
    decoys.retain(|d| d.owner != owner);
    let until = tick + u64::from(DECOY_TICKS);
    decoys.push(Scarecrow { owner, x: hero.x, y: hero.y, until,
        collapse_until: until + u64::from(COLLAPSE_TICKS) });
}
/// Scarecrows taunt this room while standing. With several, each enemy's
/// ordinary AI chooses the nearest decoy position.
pub(super) fn sights(tick: u64, decoys: &[Scarecrow], enemy: &Enemy,
    knights: &[(u32, f32, f32)], images: &[(f32, f32)], out: &mut Vec<(f32, f32)>) {
    out.clear();
    out.extend(decoys.iter().filter(|d| d.active(tick)).map(|d| (d.x, d.y)));
    if out.is_empty() { press::sights(enemy, knights, images, out); }
}
pub fn stone(p: &Projectile) -> bool {
    !p.hostile && p.traits.stone
}
/// Visual lob elevation above the authoritative ground sweep and shadow.
/// Scenery and enemies still stop the stone along that sweep.
pub fn stone_height(ttl: u32) -> f32 {
    let flight = Weapon::Stones.arms().ttl as f32;
    let t = (ttl as f32 / flight).clamp(0.0, 1.0);
    4.0 * STONE_HEIGHT * t * (1.0 - t)
}
#[cfg(test)]
#[path = "../../tests/rules/together_peasant__tests.rs"]
mod tests;
