//! How a floor below ramps, room by room, from its entrance to its stairs.
//! A playtester's dials: the first rooms past the entrance hold a few plain
//! monsters, each room deeper holds more and more kinds, the floor's chief's
//! camp waits mid-floor (led by an elite who carries the Boss Key, no boss),
//! and the floor's guardian holds the stairs, its one boss room, at the end.
//!
//! The ramp is in how many monsters and which, never in how tough: health,
//! damage and the party's scaling (`stakes`) are as they were.

pub(super) const CAMP_FOES: [[u8; 6]; 6] = [
    [2, 4, 6, 7, 8, 8],
    [3, 5, 7, 8, 9, 10],
    [4, 6, 8, 9, 10, 11],
    [5, 7, 9, 10, 11, 12],
    [8, 11, 14, 16, 18, 19],
    [9, 12, 15, 17, 19, 20],
];

pub(super) const STAIRS_FROM: usize = 4;

pub(super) const GROW_TRIES: usize = 48;

pub(super) const CHIEF_FROM: usize = 2;

pub(super) const ELITE_FROM: usize = 2;

pub(super) const CLAN_HEAD_FROM: u8 = 4;

pub(super) fn camp_foes(depth: u32, doors: usize) -> u8 {
    let row = &CAMP_FOES[(depth.clamp(1, 6) - 1) as usize];
    row[doors.clamp(1, row.len()) - 1]
}
