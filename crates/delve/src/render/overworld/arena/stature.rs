//! How tall a foe stands. One rule for the whole bestiary, measured against
//! the knight, who is sixteen pixels tall:
//!
//! - a monster is never under `FOE_MIN` pixels tall. At the terminal's
//!   half-blocks (two map pixels to a cell across, four down) that is still a
//!   figure several cells high, never a dot. The art is drawn at least that
//!   tall at rest; a pose or a stage that would draw it smaller (a slime's
//!   last split) is held at the kind's own scale instead.
//! - a boss stands at least `CHIEF` pixels, a knight and a half: a route
//!   chief, the Wanted outlaw, the Pit's beast.
//! - the floor's guardian stands at least `GUARDIAN` pixels, two knights.
//!
//! Art is never shrunk, and is enlarged (nearest neighbour, so its pixels
//! stay its own) only as far as its floor or its hit radius asks.

use super::super::ink::Img;
use super::{UNIT, enlarge, sprites};
use crate::rules::{Enemy, EnemyKind, Run};

pub const KNIGHT: i32 = crate::rules::content::stature::KNIGHT;
pub const FOE_MIN: i32 = crate::rules::content::stature::FOE_MIN;
pub const CHIEF: i32 = crate::rules::content::stature::CHIEF;
pub const GUARDIAN: i32 = crate::rules::content::stature::GUARDIAN;

pub(super) fn floor(run: &Run, enemy: &Enemy) -> i32 {
    if enemy.kind == EnemyKind::Dragon || (enemy.boss.is_some() && run.guardian_room()) {
        GUARDIAN
    } else if enemy.is_boss() {
        CHIEF
    } else {
        FOE_MIN
    }
}

pub(super) fn monster(kind: EnemyKind, im: Img, k: f32) -> Img {
    let mut k = k.max(1.0);
    if im.h as f32 * k < FOE_MIN as f32 {
        let rest = sprites::enemy(kind, 0).h.max(1);
        k = k.max(FOE_MIN as f32 / rest as f32);
    }
    if k > 1.0 { enlarge(&im, k) } else { im }
}

pub(super) fn boss(art: &Img, radius: f32, floor: i32) -> Img {
    let strike = radius * 2.0 * UNIT / art.w.max(art.h).max(1) as f32;
    let stand = floor as f32 / art.h.max(1) as f32;
    let k = strike.max(stand);
    if k > 1.0 {
        enlarge(art, k)
    } else {
        art.clone()
    }
}

pub(super) fn boss_figure(run: &Run, index: u8) -> Option<Img> {
    let boss = run.bosses.get(usize::from(index))?;
    let art = if let Some(im) = super::claim_bosses::body(&boss.id) {
        im
    } else if boss.id == crate::rules::signatures::RIPPER {
        sprites::ripper()
    } else if boss.id == crate::rules::swamp::KING {
        super::swamp_art::mire_king(false, run.tick as u32)
    } else if boss.id == "mother-of-spores" {
        super::boss_art::mother_body()
    } else if let Some(art) = crate::rules::bosses::Art::for_id(&boss.id) {
        super::boss_art::body(art)
    } else {
        let rows: Vec<&str> = boss.art.iter().map(String::as_str).collect();
        super::autorig::boss_still(run, index, &rows).unwrap_or_else(|| Img::from_rows(&rows))
    };
    let floor = if run.guardian_room() { GUARDIAN } else { CHIEF };
    let art = if boss.id == crate::rules::signatures::RIPPER {
        super::foe_material::ripper(&art)
    } else {
        super::foe_material::dress(&art, EnemyKind::Boss)
    };
    Some(self::boss(&art, boss.radius, floor))
}
