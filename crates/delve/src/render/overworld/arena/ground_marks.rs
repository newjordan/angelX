//! The marks on a pack's bare floor, a pixel at a time: the Crypt's worn
//! flags, the Mines' pebbles, the Keep's ash, the Archive's pages, the
//! Fungal Deep's moss and the Unknown's lattice.

use super::*;

/// Crypt flags: a worn joint here and there, and worn chips a few to a
/// patch. Every hall of the realm is floored with them, so the marks are
/// worked out once a block of the floor and kept.
pub(super) fn flags(x: i32, y: i32) -> Option<char> {
    const B: i32 = 64;
    const N: i32 = 16;
    const INKS: [Option<char>; 4] = [None, Some('x'), Some('S'), Some('s')];
    static BLOCKS: [OnceLock<Box<[u8]>>; (N * N) as usize] = [const { OnceLock::new() }; (N * N) as usize];
    if !((0..B * N).contains(&x) && (0..B * N).contains(&y)) {
        return flags_at(x, y);
    }
    let marks = BLOCKS[((y / B) * N + x / B) as usize].get_or_init(|| {
        let (x0, y0) = (x / B * B, y / B * B);
        (0..B * B)
            .map(|i| match flags_at(x0 + i % B, y0 + i / B) {
                Some('x') => 1,
                Some('S') => 2,
                Some('s') => 3,
                _ => 0,
            })
            .collect()
    });
    INKS[marks[((y % B) * B + x % B) as usize] as usize]
}

fn flags_at(x: i32, y: i32) -> Option<char> {
    let (lx, ly) = (x.rem_euclid(TILE), y.rem_euclid(TILE));
    let stone = hash(x.div_euclid(TILE), y.div_euclid(TILE), 81);
    let (a, b) = ((stone >> 8) as i32 % 6, (stone >> 12) as i32 % 6);
    if (ly == 0 && lx > 2 + a && lx < 7 + a && stone.is_multiple_of(5))
        || (lx == 0 && ly > 3 + b && ly < 8 + b && (stone >> 4).is_multiple_of(7))
    {
        return Some('x');
    }
    // Worn chips in the flags, a few to a patch.
    chip(x, y, 83, 68, 84, 'S', 's')
}

/// A small mark at `(x, y)`, or none: cells of six by five, every other
/// row shifted, each holding one with chance `p` in 256 at its own place in
/// the cell, two or three pixels wide and two high, its top row `top` and
/// the rounded row under it `body`. Grouped marks with black paper
/// between, never a lone pixel. With a `patch` seed the chance rises and
/// falls in slow patches, none in the bare stretches and all of `p` where
/// the marks collect; the patch is looked up only on a mark's own pixels.
pub(super) fn chip(x: i32, y: i32, seed: u32, p: u32, patch: u32, top: char, body: char) -> Option<char> {
    let cy = y.div_euclid(5);
    let sx = x + cy.rem_euclid(2) * 3;
    let (cx, px, py) = (sx.div_euclid(6), sx.rem_euclid(6), y.rem_euclid(5));
    let h = hash(cx, cy, seed);
    let roll = h & 0xff;
    if roll >= p {
        return None;
    }
    let (ox, oy, w) = (((h >> 8) % 4) as i32, ((h >> 10) % 3) as i32, 2 + ((h >> 12) % 2) as i32);
    if px < ox || px >= ox + w || py < oy || py > oy + 1 {
        return None;
    }
    if patch != 0 {
        let n = vnoise((cx * 6) as f32 / 26.0, (cy * 5) as f32 / 20.0, patch);
        if roll as f32 >= p as f32 * ((n - 0.3) / 0.4).clamp(0.0, 1.0) {
            return None;
        }
    }
    if py == oy {
        Some(top)
    } else {
        (w == 2 || px < ox + w - 1).then_some(body)
    }
}

/// The Mines' pebbles, strewn like the roads up to the mine mouth: small
/// stones lit on top, gathered in drifts.
pub(super) fn pebbles(x: i32, y: i32) -> Option<char> {
    chip(x, y, 85, 16, 86, 'o', 'B').or_else(|| chip(x, y, 89, 96, 86, 'P', 'b'))
}

/// The Keep's ash in drifts, with an ember cooling in it now and then.
pub(super) fn ash(x: i32, y: i32) -> Option<char> {
    chip(x, y, 88, 5, 0, 'p', 'b').or_else(|| chip(x, y, 87, 88, 90, 'g', 'X'))
}

/// The Archive's floor: worn flags, and pages that drifted off the shelves.
pub(super) fn pages(x: i32, y: i32) -> Option<char> {
    let h = hash(x / 3, y / 2, 91);
    if h.is_multiple_of(97) && (x % 3 != 2) {
        return Some(if (y / 2) % 2 == 0 { 'T' } else { 't' });
    }
    flags(x, y).filter(|_| hash(x / 6, y / 5, 92).is_multiple_of(2))
}

/// The Fungal Deep's floor: moss in tufts, lit on top, and now and then a
/// spore that glows.
pub(super) fn moss(x: i32, y: i32) -> Option<char> {
    chip(x, y, 95, 4, 0, 'Y', 'l').or_else(|| chip(x, y, 93, 140, 94, 'm', 'l'))
}

/// The Unknown's floor: hardly a floor at all — a lattice of faint light,
/// as if the world were still being worked out.
pub(super) fn lattice(x: i32, y: i32) -> Option<char> {
    let on = (x.rem_euclid(TILE) == 0 && y.rem_euclid(4) == 0)
        || (y.rem_euclid(TILE) == 0 && x.rem_euclid(4) == 0);
    if on && hash(x / TILE, y / TILE, 96).is_multiple_of(3) {
        return Some('0');
    }
    (hash(x, y, 97).is_multiple_of(400)).then_some('2')
}

pub(super) fn floor_mark(pack: Pack, x: i32, y: i32) -> Option<char> {
    match pack {
        Pack::Crypt => flags(x, y),
        Pack::Cavern => pebbles(x, y),
        Pack::Hellforge => ash(x, y),
        Pack::Archive => pages(x, y),
        Pack::Fungal => moss(x, y),
        Pack::Unknown => lattice(x, y),
        Pack::Swamp => swamp_art::ground(x, y),
    }
}
