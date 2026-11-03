//! The delve packs dressed whole in the Demonic Dungeon (VEXED, CC BY 4.0;
//! `cockpit/assets/dungeon/LICENSES.md`). There the room's kit
//! (`scripts/delve/pack_tiles.py`, `pack_kit.py`), its spikes and the pack's
//! foes stand together, and nowhere else, so the pack's look never mixes
//! with another floor's.
use super::super::ink::Img;
use crate::rules::{EnemyKind, Pack, Run};

include!("pack_traps_frames.rs");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Style {
    Teal,
    Red,
    Cave,
}

/// The pack's style where it dresses the whole floor.
pub(super) fn style(pack: Pack) -> Option<Style> {
    match pack {
        Pack::Crypt => Some(Style::Teal),
        Pack::Hellforge => Some(Style::Red),
        Pack::Cavern => Some(Style::Cave),
        _ => None,
    }
}

/// A foe as the room it stands in draws it.
pub(super) fn enemy(run: &Run, kind: EnemyKind, tick: u32) -> Img {
    style(run.dungeon.pack)
        .and_then(|_| super::pack_foes::frame(kind, tick))
        .unwrap_or_else(|| super::sprites::enemy(kind, tick))
}

fn set(style: Style) -> [&'static [&'static str; 8]; 3] {
    match style {
        Style::Teal => [&GREY_HOLE, &GREY_TIP, &GREY_UP],
        Style::Red => [&RED_HOLE, &RED_TIP, &RED_UP],
        Style::Cave => [&ORANGE_HOLE, &ORANGE_TIP, &ORANGE_UP],
    }
}

/// A spike trap at rest: sixteen of the pack's holes over its two by two
/// tiles.
pub(super) fn spike_plate(style: Style) -> Img {
    let mut im = Img::new(32, 32);
    let hole = Img::from_rows(set(style)[0]);
    for j in 0..4 {
        for i in 0..4 {
            im.stamp(&hole, i * 8, j * 8);
        }
    }
    im
}

/// The spikes over the plate: tips rattling in half the holes, turn about,
/// before they strike, or all of them up.
pub(super) fn spikes_out(style: Style, up: bool, tick: u32) -> Img {
    let [_, tip, spike] = set(style);
    let (tip, spike) = (Img::from_rows(tip), Img::from_rows(spike));
    let mut im = Img::new(32, 32);
    for j in 0..4 {
        for i in 0..4 {
            if up {
                im.stamp(&spike, i * 8, j * 8);
            } else if (tick / 2 + (i + j) as u32).is_multiple_of(2) {
                im.stamp(&tip, i * 8, j * 8);
            }
        }
    }
    im
}
