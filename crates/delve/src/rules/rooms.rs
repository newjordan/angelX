//! Rooms drawn by hand, a pack at a time: the dungeon masters' layouts,
//! laid beside the packs' own templates (`layout::Pack::templates`) and in
//! place of the pillared grid (`layout::great_quadrant`) for the bigger
//! rooms a party is given (`layout::party_room`).
//!
//! A room is drawn as its top-left quadrant and mirrored both ways
//! (`layout::mirror`): `.` floor, `#` a block, `~` a hazard pool. The
//! column next to the vertical mirror and the row next to the horizontal
//! one are the doorway lanes and stay open at their ends. Each pack's file
//! holds:
//!
//! - `SMALL`: quadrants of a one-screen room, 11 by 6.
//! - `GREAT`: quadrants of the bigger rooms, any of the sizes in
//!   [`SIZES`]; a room draws from those of its own size.
//!
//! Every one is checked: the right size, only those three marks, the
//! doorway lanes open, the middle open (the last 3x3 of the quadrant: the
//! stairs and a guardian stand there), every floor tile reachable from
//! every doorway, and enough floor to fight on (`together_rooms__tests`).

use super::layout::{Pack, Template};

mod archive;
mod cavern;
mod crypt;
mod fungal;
mod hellforge;
mod swamp;
mod unknown;

/// The quadrant sizes of the bigger rooms (interior halves less the lanes):
/// a party's rooms (34x20, 42x24, 48x28) and the great halls the packs lay
/// (32x20, 36x20, 40x24).
pub const SIZES: [(usize, usize); 6] = [(15, 9), (16, 9), (17, 9), (19, 11), (20, 11), (23, 13)];

/// How often, in percent, a bigger room of a size someone has drawn is one
/// of theirs rather than the pillared grid.
pub const AUTHORED_SHARE: u32 = 75;

/// The least share of a room's interior, in percent, left as floor.
pub const LEAST_FLOOR: usize = 55;

/// A pack's hand-drawn one-screen quadrants.
pub fn small(pack: Pack) -> &'static [Template] {
    match pack {
        Pack::Crypt => crypt::SMALL,
        Pack::Cavern => cavern::SMALL,
        Pack::Hellforge => hellforge::SMALL,
        Pack::Archive => archive::SMALL,
        Pack::Fungal => fungal::SMALL,
        Pack::Unknown => unknown::SMALL,
        Pack::Swamp => swamp::SMALL,
    }
}

/// Every quadrant a pack has drawn for the bigger rooms, of any size.
pub fn all_great(pack: Pack) -> &'static [&'static [&'static str]] {
    match pack {
        Pack::Crypt => crypt::GREAT,
        Pack::Cavern => cavern::GREAT,
        Pack::Hellforge => hellforge::GREAT,
        Pack::Archive => archive::GREAT,
        Pack::Fungal => fungal::GREAT,
        Pack::Unknown => unknown::GREAT,
        Pack::Swamp => swamp::GREAT,
    }
}

/// A pack's hand-drawn quadrants `w` wide and `h` high.
pub fn great(pack: Pack, w: usize, h: usize) -> Vec<&'static [&'static str]> {
    all_great(pack)
        .iter()
        .copied()
        .filter(|q| q.len() == h && q.iter().all(|r| r.len() == w))
        .collect()
}

#[cfg(test)]
#[path = "../../tests/rules/together_rooms__tests.rs"]
mod tests;
