//! The Delve's music as playlists (cockpit/assets/dungeon/music; "The Quiet
//! Roads", the project's own soundtrack, MiniMax Music 3 and YuE2). A place
//! plays its list one track after another, each for its own length, so a room
//! or a floor never loops one cue: the run's clock says which track is on
//! (`play`). The lengths are the files' own, in whole seconds.

/// A track: its name (the file's stem) and how long it plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Track {
    pub name: &'static str,
    pub secs: u16,
}

/// Up in the realm: the village's tunes and the peaceful score.
pub const HOME: &[Track] = &[
    Track { name: "village-wakes", secs: 72 },
    Track { name: "village-green", secs: 139 },
    Track { name: "quiet-hills", secs: 53 },
    Track { name: "bluebells", secs: 43 },
    Track { name: "heather", secs: 40 },
    Track { name: "pear-tree", secs: 50 },
    Track { name: "qr-p-minimax-001", secs: 73 },
    Track { name: "qr-p-minimax-003", secs: 34 },
    Track { name: "qr-p-minimax-005", secs: 48 },
    Track { name: "qr-p-minimax-008", secs: 42 },
    Track { name: "qr-p-minimax-010", secs: 52 },
    Track { name: "qr-p-minimax-011", secs: 54 },
    Track { name: "qr-p-minimax-012", secs: 38 },
    Track { name: "qr-p-minimax-014", secs: 34 },
    Track { name: "qr-p-minimax-015", secs: 45 },
    Track { name: "qr-p-minimax-020", secs: 67 },
    Track { name: "qr-p-minimax-021", secs: 65 },
    Track { name: "qr-p-minimax-022", secs: 59 },
    Track { name: "qr-p-minimax-023", secs: 53 },
    Track { name: "qr-p-minimax-024", secs: 53 },
    Track { name: "qr-p-minimax-025", secs: 58 },
    Track { name: "qr-p-minimax-026", secs: 52 },
    Track { name: "qr-p-minimax-027", secs: 58 },
    Track { name: "qr-p-minimax-028", secs: 72 },
    Track { name: "qr-p-minimax-029", secs: 52 },
    Track { name: "qr-p-minimax-030", secs: 63 },
    Track { name: "qr-p-minimax-031", secs: 52 },
    Track { name: "qr-p-minimax-032", secs: 66 },
    Track { name: "qr-p-minimax-033", secs: 62 },
    Track { name: "qr-p-minimax-034", secs: 59 },
    Track { name: "qr-p-minimax-035", secs: 44 },
    Track { name: "qr-p-minimax-036", secs: 64 },
    Track { name: "qr-p-minimax-037", secs: 48 },
    Track { name: "qr-p-minimax-038", secs: 43 },
    Track { name: "qr-p-minimax-039", secs: 47 },
    Track { name: "qr-p-minimax-040", secs: 34 },
    Track { name: "qr-p-minimax-041", secs: 66 },
    Track { name: "qr-p-minimax-042", secs: 47 },
    Track { name: "qr-p-minimax-043", secs: 50 },
    Track { name: "qr-p-minimax-044", secs: 46 },
    Track { name: "qr-p-minimax-045", secs: 63 },
    Track { name: "qr-p-minimax-046", secs: 37 },
    Track { name: "qr-p-minimax-047", secs: 48 },
    Track { name: "qr-p-minimax-048", secs: 58 },
    Track { name: "qr-p-minimax-049", secs: 88 },
    Track { name: "qr-p-minimax-050", secs: 63 },
    Track { name: "qr-p-minimax-051", secs: 66 },
    Track { name: "qr-p-minimax-052", secs: 37 },
    Track { name: "qr-p-minimax-053", secs: 81 },
    Track { name: "qr-p-minimax-054", secs: 54 },
    Track { name: "qr-p-minimax-055", secs: 50 },
    Track { name: "qr-p-minimax-056", secs: 44 },
    Track { name: "qr-p-minimax-057", secs: 52 },
    Track { name: "qr-p-minimax-058", secs: 65 },
    Track { name: "qr-p-minimax-059", secs: 59 },
    Track { name: "qr-p-minimax-060", secs: 48 },
    Track { name: "qr-p-minimax-061", secs: 98 },
    Track { name: "qr-p-minimax-062", secs: 32 },
    Track { name: "qr-p-minimax-063", secs: 40 },
    Track { name: "qr-p-yue2-002", secs: 289 },
    Track { name: "qr-p-yue2-003", secs: 179 },
    Track { name: "qr-p-yue2-004", secs: 214 },
    Track { name: "qr-p-yue2-005", secs: 153 },
    Track { name: "qr-p-yue2-008", secs: 141 },
    Track { name: "qr-p-yue2-009", secs: 249 },
    Track { name: "qr-p-yue2-010", secs: 98 },
];

/// A Sanctuary: the fireside's tunes and the peaceful score.
pub const REST: &[Track] = &[
    Track { name: "candles", secs: 70 },
    Track { name: "last-light", secs: 62 },
    Track { name: "fireside-cup", secs: 58 },
    Track { name: "small-hours", secs: 89 },
    Track { name: "library-rain", secs: 53 },
    Track { name: "keepers-cottage", secs: 72 },
    Track { name: "roadside-lantern", secs: 123 },
    Track { name: "sanctuary", secs: 69 },
    Track { name: "qr-p-minimax-001", secs: 73 },
    Track { name: "qr-p-minimax-003", secs: 34 },
    Track { name: "qr-p-minimax-005", secs: 48 },
    Track { name: "qr-p-minimax-008", secs: 42 },
    Track { name: "qr-p-minimax-010", secs: 52 },
    Track { name: "qr-p-minimax-011", secs: 54 },
    Track { name: "qr-p-minimax-012", secs: 38 },
    Track { name: "qr-p-minimax-014", secs: 34 },
    Track { name: "qr-p-minimax-015", secs: 45 },
    Track { name: "qr-p-minimax-020", secs: 67 },
    Track { name: "qr-p-minimax-021", secs: 65 },
    Track { name: "qr-p-minimax-022", secs: 59 },
    Track { name: "qr-p-minimax-023", secs: 53 },
    Track { name: "qr-p-minimax-024", secs: 53 },
    Track { name: "qr-p-minimax-025", secs: 58 },
    Track { name: "qr-p-minimax-026", secs: 52 },
    Track { name: "qr-p-minimax-027", secs: 58 },
    Track { name: "qr-p-minimax-028", secs: 72 },
    Track { name: "qr-p-minimax-029", secs: 52 },
    Track { name: "qr-p-minimax-030", secs: 63 },
    Track { name: "qr-p-minimax-031", secs: 52 },
    Track { name: "qr-p-minimax-032", secs: 66 },
    Track { name: "qr-p-minimax-033", secs: 62 },
    Track { name: "qr-p-minimax-034", secs: 59 },
    Track { name: "qr-p-minimax-035", secs: 44 },
    Track { name: "qr-p-minimax-036", secs: 64 },
    Track { name: "qr-p-minimax-037", secs: 48 },
    Track { name: "qr-p-minimax-038", secs: 43 },
    Track { name: "qr-p-minimax-039", secs: 47 },
    Track { name: "qr-p-minimax-040", secs: 34 },
    Track { name: "qr-p-minimax-041", secs: 66 },
    Track { name: "qr-p-minimax-042", secs: 47 },
    Track { name: "qr-p-minimax-043", secs: 50 },
    Track { name: "qr-p-minimax-044", secs: 46 },
    Track { name: "qr-p-minimax-045", secs: 63 },
    Track { name: "qr-p-minimax-046", secs: 37 },
    Track { name: "qr-p-minimax-047", secs: 48 },
    Track { name: "qr-p-minimax-048", secs: 58 },
    Track { name: "qr-p-minimax-049", secs: 88 },
    Track { name: "qr-p-minimax-050", secs: 63 },
    Track { name: "qr-p-minimax-051", secs: 66 },
    Track { name: "qr-p-minimax-052", secs: 37 },
    Track { name: "qr-p-minimax-053", secs: 81 },
    Track { name: "qr-p-minimax-054", secs: 54 },
    Track { name: "qr-p-minimax-055", secs: 50 },
    Track { name: "qr-p-minimax-056", secs: 44 },
    Track { name: "qr-p-minimax-057", secs: 52 },
    Track { name: "qr-p-minimax-058", secs: 65 },
    Track { name: "qr-p-minimax-059", secs: 59 },
    Track { name: "qr-p-minimax-060", secs: 48 },
    Track { name: "qr-p-minimax-061", secs: 98 },
    Track { name: "qr-p-minimax-062", secs: 32 },
    Track { name: "qr-p-minimax-063", secs: 40 },
    Track { name: "qr-p-yue2-002", secs: 289 },
    Track { name: "qr-p-yue2-003", secs: 179 },
    Track { name: "qr-p-yue2-004", secs: 214 },
    Track { name: "qr-p-yue2-005", secs: 153 },
    Track { name: "qr-p-yue2-008", secs: 141 },
    Track { name: "qr-p-yue2-009", secs: 249 },
    Track { name: "qr-p-yue2-010", secs: 98 },
];

/// Down a floor: the packs' old themes and the dungeon score.
pub const DUNGEON: &[Track] = &[
    Track { name: "crypt", secs: 54 },
    Track { name: "mines", secs: 72 },
    Track { name: "keep", secs: 81 },
    Track { name: "qr-d-minimax-001", secs: 48 },
    Track { name: "qr-d-minimax-002", secs: 41 },
    Track { name: "qr-d-minimax-003", secs: 46 },
    Track { name: "qr-d-minimax-004", secs: 62 },
    Track { name: "qr-d-minimax-005", secs: 54 },
    Track { name: "qr-d-minimax-006", secs: 69 },
    Track { name: "qr-d-minimax-007", secs: 49 },
    Track { name: "qr-d-minimax-008", secs: 80 },
    Track { name: "qr-d-minimax-009", secs: 67 },
    Track { name: "qr-d-minimax-010", secs: 48 },
    Track { name: "qr-d-minimax-011", secs: 52 },
    Track { name: "qr-d-minimax-012", secs: 72 },
    Track { name: "qr-d-minimax-013", secs: 50 },
    Track { name: "qr-d-minimax-014", secs: 69 },
    Track { name: "qr-d-minimax-015", secs: 52 },
    Track { name: "qr-d-minimax-016", secs: 71 },
    Track { name: "qr-d-minimax-017", secs: 41 },
    Track { name: "qr-d-minimax-018", secs: 59 },
    Track { name: "qr-d-minimax-019", secs: 38 },
    Track { name: "qr-d-minimax-020", secs: 47 },
    Track { name: "qr-d-minimax-021", secs: 57 },
    Track { name: "qr-d-minimax-022", secs: 59 },
    Track { name: "qr-d-minimax-023", secs: 71 },
    Track { name: "qr-d-minimax-024", secs: 27 },
    Track { name: "qr-d-minimax-025", secs: 51 },
    Track { name: "qr-d-minimax-026", secs: 45 },
    Track { name: "qr-d-minimax-027", secs: 36 },
    Track { name: "qr-d-minimax-028", secs: 29 },
    Track { name: "qr-d-minimax-029", secs: 87 },
    Track { name: "qr-d-minimax-030", secs: 49 },
    Track { name: "qr-d-yue2-001", secs: 202 },
    Track { name: "qr-d-yue2-002", secs: 313 },
    Track { name: "qr-d-yue2-004", secs: 194 },
    Track { name: "qr-d-yue2-005", secs: 198 },
];

/// A guardian, a boss or a dragon: the battle cues.
pub const BOSS: &[Track] = &[
    Track { name: "boss", secs: 123 },
    Track { name: "qr-e-minimax-001", secs: 38 },
    Track { name: "qr-e-minimax-002", secs: 55 },
    Track { name: "qr-e-minimax-003", secs: 54 },
    Track { name: "qr-e-minimax-004", secs: 74 },
    Track { name: "qr-e-minimax-005", secs: 66 },
    Track { name: "qr-e-yue2-001", secs: 255 },
];

/// The track `list` plays at run tick `tick` (`HZ` a second), for a place
/// named by `seed`: the list end to end, each track for its length, begun
/// at a point of its own for each seed so two places do not play in step.
pub fn play(list: &'static [Track], seed: u64, tick: u64) -> &'static str {
    let hz = u64::from(super::HZ);
    let total: u64 = list.iter().map(|t| u64::from(t.secs.max(1)) * hz).sum();
    if total == 0 {
        return list.first().map_or("", |t| t.name);
    }
    let mut at = (tick + seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) % total) % total;
    for track in list {
        let len = u64::from(track.secs.max(1)) * hz;
        if at < len {
            return track.name;
        }
        at -= len;
    }
    list[0].name
}

/// The quiet after each of a delve's tracks, in seconds, least and most:
/// the dark sounds on its own a while, so its score never runs into one
/// long song (the operator, 2026-10-18: "i dont want to hear the dungeon
/// song on repeat").
pub const QUIET: (u64, u64) = (20, 50);

/// A number of `seed`'s own for track `i`.
fn mix(seed: u64, i: usize) -> u64 {
    let x = (seed ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 31)
}

/// The track `list` plays at run tick `tick` for a place named by `seed`,
/// or nothing in a quiet: its tracks in an order of the seed's own, each for
/// its length and then a quiet of its own (`QUIET`), begun at a point of the
/// seed's own.
pub fn play_sparse(list: &'static [Track], seed: u64, tick: u64) -> Option<&'static str> {
    let hz = u64::from(super::HZ);
    let mut order: Vec<usize> = (0..list.len()).collect();
    order.sort_by_key(|&i| mix(seed, i));
    let quiet = |i: usize| (QUIET.0 + mix(seed ^ 0x51, i) % (QUIET.1 - QUIET.0 + 1)) * hz;
    let total: u64 = order.iter().map(|&i| u64::from(list[i].secs.max(1)) * hz + quiet(i)).sum();
    if total == 0 {
        return None;
    }
    let mut at = (tick + seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) % total) % total;
    for &i in &order {
        let len = u64::from(list[i].secs.max(1)) * hz;
        if at < len {
            return Some(list[i].name);
        }
        at -= len;
        let rest = quiet(i);
        if at < rest {
            return None;
        }
        at -= rest;
    }
    None
}

/// Every track the game may name, each once: what a player of the run's
/// music knows (`music <name>`).
pub fn known(name: &str) -> Option<&'static str> {
    [HOME, REST, DUNGEON, BOSS].into_iter().flatten().map(|t| t.name).find(|&t| t == name)
}

#[cfg(test)]
#[path = "../../tests/rules/music__tests.rs"]
mod tests;
