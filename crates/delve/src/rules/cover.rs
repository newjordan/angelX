//! How much of the screen the knights' shots cover, and the party's share
//! of it (the operator, 2026-10-18: "we need a balance system of some sort
//! so bullets cant just take up 100% of the room and bounce around").
//!
//! A friendly shot covers a square [`SHOT_SIDE`] units across, wider by its
//! size wish's reach on each side (`Projectile::reach`): what it is drawn
//! at, near enough. The party's shots together may cover [`PARTY_COVER`]
//! in the hundred of what the eye sees (one screen, or the room when it is
//! smaller). Past it, the oldest friendly shots go first, so a new volley
//! always flies and the room never fills.
//!
//! That share is switched off for now ([`PARTY_COVER_ON`], the operator,
//! 2026-10-22: "we can get rid of the 12% rule for now"): the cure is
//! smaller shots instead (`wishcraft::Projectile::scale`, a shot is drawn
//! smaller the faster its arm fires). One change, `PARTY_COVER_ON = true`,
//! brings it back.
//!
//! Every friendly shot lives at most [`SHOT_LIFE`] ticks, and comes off
//! the stone at most [`BOUNCES`] times; that holds with the share off.

use super::*;

pub const SHOT_SIDE: f32 = 2.0;
pub const PARTY_COVER: f32 = 12.0;
pub const PARTY_COVER_ON: bool = false;
pub const SHOT_LIFE: u32 = 3 * HZ;
pub const BOUNCES: u8 = 2;

pub fn counted(p: &Projectile) -> bool {
    !p.hostile && p.traits.owner > 0 && p.in_flight() && !p.traits.boomerang && p.traits.back == 0
}

pub fn area(shot: &Projectile) -> f32 {
    let side = if super::wishcraft::legacy() { SHOT_SIDE + 2.0 * shot.reach() } else { SHOT_SIDE * shot.scale() };
    side * side
}

pub fn view(room: &layout::Room) -> f32 {
    let screen = layout::COLS as f32 * layout::TILE_UNITS * layout::ROWS as f32 * layout::TILE_UNITS;
    (room.width() * room.height()).min(screen).max(1.0)
}

pub fn covered(shots: &[Projectile], view: f32) -> f32 {
    let total: f32 = shots.iter().filter(|p| counted(p) && p.ttl > 0).map(area).sum();
    total * 100.0 / view
}

pub fn hold(shots: &mut Vec<Projectile>, view: f32) -> usize {
    #[cfg(test)]
    if OFF.with(std::cell::Cell::get) {
        return 0;
    }
    for p in shots.iter_mut().filter(|p| counted(p)) {
        p.ttl = p.ttl.min(SHOT_LIFE);
        p.traits.bounce = p.traits.bounce.min(BOUNCES);
    }
    #[cfg(test)]
    let on = PARTY_COVER_ON || super::wishcraft::LEGACY.with(std::cell::Cell::get);
    #[cfg(not(test))]
    let on = PARTY_COVER_ON;
    if on { retire(shots, view, PARTY_COVER) } else { 0 }
}

pub fn retire(shots: &mut Vec<Projectile>, view: f32, share: f32) -> usize {
    let budget = share * view / 100.0;
    let mut total: f32 = shots.iter().filter(|p| counted(p)).map(area).sum();
    if total <= budget {
        return 0;
    }
    let mut retired = 0;
    for p in shots.iter_mut().filter(|p| counted(p)) {
        if total <= budget {
            break;
        }
        total -= area(p);
        p.ttl = 0;
        retired += 1;
    }
    shots.retain(|p| p.ttl > 0);
    retired
}

#[cfg(test)]
thread_local! {
    pub static OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
#[path = "../../tests/rules/together_cover__tests.rs"]
mod tests;

#[cfg(all(test, feature = "bend-cases-combat"))]
#[path = "../../tests/rules/cover_bend_cases__tests.rs"]
mod cover_bend_cases_tests;
