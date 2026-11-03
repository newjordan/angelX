//! Foes in the Demonic Dungeon monsters (VEXED, CC BY 4.0;
//! `cockpit/assets/dungeon/LICENSES.md`), three frames each, drawn as the
//! pack shades them (no material bake over the top).
use super::super::ink::Img;
use crate::rules::EnemyKind;

include!("pack_foes_frames.rs");

/// A foe's frame on `tick`, if the pack draws it.
pub(super) fn frame(kind: EnemyKind, tick: u32) -> Option<Img> {
    let frames = match kind {
        EnemyKind::Skeleton => &SKELETON,
        EnemyKind::Bat => &BAT,
        EnemyKind::Mimic => &MIMIC,
        EnemyKind::Hexer => &HEXER,
        EnemyKind::Imp => &IMP,
        EnemyKind::Slime => &SLIME,
        EnemyKind::Ward => &WARD,
        EnemyKind::Spiderling => &SPIDERLING,
        _ => return None,
    };
    let f = (tick / 8) as usize % frames.len();
    Some(super::foe_material::pose(192 + kind as u8, [f as u32, 0, 0, 0], || Img::from_rows(frames[f])))
}
