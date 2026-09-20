//! Seeded world-gen helpers (module-breakup: the seed cluster, extracted
//! from `world_viz.rs`).
//!
//! Pure and deterministic: the cute town-name generator, fnv1a, district
//! hashing/banner colors, and the scenery roll — no World state.

/// Cute deterministic town name: two seed-picked syllables + a fusion suffix.
pub(crate) fn town_name(seed: u64) -> String {
    // An Arthurian castle-name generator: head + connective + tail, seed-indexed
    // so every project keeps the same realm name forever. Already capitalized
    // (the heads are), and never hyphenated — e.g. Camelot, Tingel, Astermere.
    let h = crate::identity::TOWN_HEAD[(seed % 12) as usize];
    let m = crate::identity::TOWN_MID[((seed / 12) % 6) as usize];
    let t = crate::identity::TOWN_TAIL[((seed / 72) % 8) as usize];
    format!("{h}{m}{t}")
}

pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub(crate) fn district_hash(seed: u64, name: &str) -> u64 {
    let mut hash = fnv1a(&seed.to_le_bytes());
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Bright, moderately saturated ink remains readable over the muted map
/// ramps, while palette selection remains wholly deterministic.
pub(crate) fn district_banner_color(hash: u64) -> (u8, u8, u8) {
    const BANNERS: [(u8, u8, u8); 8] = [
        (244, 114, 182),
        (96, 165, 250),
        (250, 204, 21),
        (74, 222, 128),
        (192, 132, 252),
        (251, 146, 60),
        (45, 212, 191),
        (248, 113, 113),
    ];
    BANNERS[((hash >> 32) as usize) % BANNERS.len()]
}
