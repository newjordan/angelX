//! How a floor below ramps, room by room, from its entrance to its stairs.
//! A playtester's dials: the first rooms past the entrance hold a few plain
//! monsters, each room deeper holds more and more kinds, the floor's chief's
//! camp waits mid-floor (led by an elite who carries the Boss Key, no boss),
//! and the floor's guardian holds the stairs, its one boss room, at the end.
//!
//! The ramp is in how many monsters and which, never in how tough: health,
//! damage and the party's scaling (`stakes`) are as they were.

/// The monsters a fight room's camp musters, by floor (rows, floor 1 first)
/// and by doors from the entrance (columns: one door, two, ... six or more).
/// The floor's own extras and each knight past the first add to every room
/// alike (`stakes::EXTRA_FOES`, `stakes::EXTRA_PER_KNIGHT`), so the ramp holds.
pub(super) const CAMP_FOES: [[u8; 6]; 6] = [
    [2, 4, 6, 7, 8, 8],
    [3, 5, 7, 8, 9, 10],
    [4, 6, 8, 9, 10, 11],
    [5, 7, 9, 10, 11, 12],
    [8, 11, 14, 16, 18, 19],
    [9, 12, 15, 17, 19, 20],
];

/// The fewest doors between a floor's entrance and its stairs: a floor that
/// grows shorter is grown again, so every floor has rooms enough to ramp.
pub(super) const STAIRS_FROM: usize = 4;

/// How many times a floor is grown again for want of depth before the
/// deepest of them stands.
pub(super) const GROW_TRIES: usize = 48;

/// The fewest doors between the entrance and the floor's chief: no chief in
/// the entrance's own ring of rooms.
pub(super) const CHIEF_FROM: usize = 2;

/// The fewest doors between the entrance and a room that may hold an elite.
pub(super) const ELITE_FROM: usize = 2;

/// The fewest monsters in a rival clan's camp before its head (a sapper or
/// a hob) leads it: the first small camps are plain.
pub(super) const CLAN_HEAD_FROM: u8 = 4;

/// What a fight room `doors` from the entrance of floor `depth` musters.
pub(super) fn camp_foes(depth: u32, doors: usize) -> u8 {
    let row = &CAMP_FOES[(depth.clamp(1, 6) - 1) as usize];
    row[doors.clamp(1, row.len()) - 1]
}
