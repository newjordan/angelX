//! Ground marks: the realm drawn on black paper.
//!
//! The ground is never filled. Each terrain leaves sparse marks — grass
//! tufts, road pebbles, wave dashes, stone joints — and everything between
//! them stays black. Every mark is a pure function of the world pixel.

use super::ink::{bayer, hash, vnoise};
use super::map::{Realm, TILE};

fn road_like(t: u8) -> bool {
    matches!(t, b'=' | b':' | b'H')
}

/// The ink at a world pixel, or `None` for black paper.
pub(crate) fn mark(realm: &Realm, wx: i32, wy: i32) -> Option<char> {
    let (tx, ty) = (wx.div_euclid(TILE), wy.div_euclid(TILE));
    let (lx, ly) = (wx.rem_euclid(TILE), wy.rem_euclid(TILE));
    match realm.at(tx, ty) {
        b'~' => water(realm, wx, wy),
        b'=' => road(realm, wx, wy, (tx, ty), (lx, ly)),
        b':' => cobble(wx, wy),
        b's' => sand(wx, wy),
        b'%' => swamp(wx, wy),
        b'c' => crops(wx, wy),
        b'a' => ash(wx, wy),
        b'd' => {
            let near_swamp = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|(dx, dy)| realm.at(tx + dx, ty + dy) == b'%');
            if near_swamp {
                swamp(wx, wy)
            } else {
                ash(wx, wy)
            }
        }
        b'H' => Some(planks(wx, wy)),
        _ => meadow(wx, wy),
    }
}

/// What a light pool shows on bare paper: a dim ink matching the ground.
pub(crate) fn pool_ink(realm: &Realm, wx: i32, wy: i32, fire: bool) -> char {
    match realm.at_px(wx, wy) {
        b'~' => 'q',
        b'=' | b's' => {
            if fire {
                'B'
            } else {
                'I'
            }
        }
        b':' => 'X',
        b'a' | b'd' => 'g',
        _ => {
            if fire {
                'I'
            } else {
                'F'
            }
        }
    }
}

fn meadow(wx: i32, wy: i32) -> Option<char> {
    let (cx, cy) = (wx.div_euclid(7), wy.div_euclid(6));
    let (lx, ly) = (wx.rem_euclid(7), wy.rem_euclid(6));
    let h = hash(cx, cy, 21);
    if h % 5 < 2 {
        let (ox, oy) = (((h >> 3) % 4) as i32, ((h >> 6) % 4) as i32);
        let bright = (h >> 9) % 3 == 0;
        if ly == oy && (lx == ox || lx == ox + 2) {
            return Some(if bright { 'l' } else { 'E' });
        }
        if ly == oy + 1 && lx == ox + 1 {
            return Some('F');
        }
    }
    if h % 97 == 7 && lx == 3 && ly == 2 {
        return Some('o');
    }
    None
}

fn road(
    realm: &Realm,
    wx: i32,
    wy: i32,
    (tx, ty): (i32, i32),
    (lx, ly): (i32, i32),
) -> Option<char> {
    let open = |dx: i32, dy: i32| road_like(realm.at(tx + dx, ty + dy));
    let rag = |a: i32, b: i32, s: u32| (hash(a, b, s) % 3) as i32 + 1;
    let mut edge = false;
    for (gone, dist, reach) in [
        (!open(-1, 0), lx, rag(wy, tx, 11)),
        (!open(1, 0), TILE - 1 - lx, rag(wy, tx, 12)),
        (!open(0, -1), ly, rag(wx, ty, 13)),
        (!open(0, 1), TILE - 1 - ly, rag(wx, ty, 14)),
    ] {
        if gone {
            if dist < reach {
                return meadow(wx, wy);
            }
            if dist == reach {
                edge = true;
            }
        }
    }
    if edge {
        return (hash(wx, wy, 16) % 3 != 0).then_some('I');
    }
    // A pebbled bed: sparse, warm, low.
    match hash(wx, wy, 15) % 64 {
        0..=5 => Some('P'),
        6..=7 => Some('r'),
        8 => Some('o'),
        _ => ((wx + wy) % 2 == 0 && bayer(wx, wy) < 0.18).then_some('b'),
    }
}

fn cobble(wx: i32, wy: i32) -> Option<char> {
    let row = wy.div_euclid(5);
    let off = if row % 2 == 1 { 3 } else { 0 };
    let (lx, ly) = ((wx + off).rem_euclid(6), wy.rem_euclid(5));
    let stone = hash((wx + off).div_euclid(6), row, 17);
    if lx == 0 || ly == 0 || stone % 5 == 0 {
        return None;
    }
    if (lx == 1 || lx == 5) && (ly == 1 || ly == 4) {
        return None;
    }
    Some(if ly == 1 {
        'g'
    } else if stone % 3 == 0 {
        'X'
    } else {
        'K'
    })
}

fn water(realm: &Realm, wx: i32, wy: i32) -> Option<char> {
    let land = |x: i32, y: i32| realm.at_px(x, y) != b'~';
    let mut shore = i32::MAX;
    for k in 1..=4 {
        if land(wx - k, wy) || land(wx + k, wy) || land(wx, wy - k) || land(wx, wy + k) {
            shore = k;
            break;
        }
    }
    let wobble = (hash(wx / 3, wy / 3, 5) % 2) as i32;
    match shore.saturating_sub(wobble) {
        i32::MIN..=1 => return Some('Q'),
        2 => return ((wx + wy) % 2 == 0).then_some('q'),
        _ => {}
    }
    let (cx, cy) = (wx.div_euclid(9), wy.div_euclid(6));
    let h = hash(cx, cy, 9);
    if h % 2 == 0 {
        let (ox, oy) = (((h >> 4) % 5) as i32, ((h >> 8) % 5) as i32);
        let (lx, ly) = (wx.rem_euclid(9), wy.rem_euclid(6));
        if ly == oy && lx >= ox && lx < ox + 4 {
            return Some(if lx == ox + 1 { 'Q' } else { 'q' });
        }
    }
    None
}

fn sand(wx: i32, wy: i32) -> Option<char> {
    match hash(wx, wy, 23) % 9 {
        0 => Some('P'),
        1 => Some('I'),
        _ => None,
    }
}

fn swamp(wx: i32, wy: i32) -> Option<char> {
    let n = vnoise(wx as f32 / 9.0, wy as f32 / 7.0, 29);
    if n > 0.62 {
        return if n > 0.70 {
            ((wx + wy) % 3 == 0).then_some('q')
        } else {
            Some('x')
        };
    }
    match hash(wx, wy, 31) % 11 {
        0 => Some('e'),
        1 => Some('E'),
        2 if wy % 3 == 0 => Some('N'),
        _ => None,
    }
}

fn crops(wx: i32, wy: i32) -> Option<char> {
    match wy.rem_euclid(4) {
        0 => Some(if hash(wx, wy, 33) % 3 == 0 { 'L' } else { 'l' }),
        1 => (wx % 2 == 0).then_some('E'),
        _ => (hash(wx, wy, 34) % 13 == 0).then_some('I'),
    }
}

fn ash(wx: i32, wy: i32) -> Option<char> {
    match hash(wx, wy, 37) % 14 {
        0 => Some('X'),
        1 => Some('g'),
        2 if wx % 5 == 0 => Some('K'),
        _ => None,
    }
}

fn planks(wx: i32, wy: i32) -> char {
    if wy.rem_euclid(4) == 3 {
        'b'
    } else if hash(wx / 5, wy / 4, 3) % 4 == 0 {
        'r'
    } else {
        'R'
    }
}
