//! Ground marks: the realm drawn on black paper.
//!
//! The ground is never filled. Each terrain leaves sparse marks — grass
//! tufts, road pebbles, wave dashes, stone joints — and everything between
//! them stays black. Water and lava move, slowly: wave dashes drift,
//! streams run downhill, falls pour and lava creeps, one pixel per ambient
//! step. Every mark is a pure function of the world pixel and that step.

use super::ink::{hash, vnoise};
use super::map::{Realm, TILE};

fn road_like(t: u8) -> bool {
    matches!(t, b'=' | b':' | b'H')
}

/// The ink at a world pixel at ambient step `tick`, or `None` for black paper.
pub fn mark(realm: &Realm, wx: i32, wy: i32, tick: u32) -> Option<char> {
    let (tx, ty) = (wx.div_euclid(TILE), wy.div_euclid(TILE));
    let (lx, ly) = (wx.rem_euclid(TILE), wy.rem_euclid(TILE));
    let t = tick as i32;
    match realm.at(tx, ty) {
        b'~' => water(realm, wx, wy, t),
        b'w' => Some(stream(realm, wx, wy, (tx, ty), t)),
        b'f' => Some(fall(realm, wx, wy, (tx, ty), t, false)),
        b'L' => Some(lava(realm, wx, wy, (tx, ty), t)),
        b'F' => Some(fall(realm, wx, wy, (tx, ty), t, true)),
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
pub fn pool_ink(realm: &Realm, wx: i32, wy: i32, fire: bool) -> char {
    match realm.at_px(wx, wy) {
        b'~' | b'w' | b'f' => 'q',
        b'L' | b'F' => 'p',
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

pub fn meadow(wx: i32, wy: i32) -> Option<char> {
    let (cx, cy) = (wx.div_euclid(7), wy.div_euclid(6));
    let (lx, ly) = (wx.rem_euclid(7), wy.rem_euclid(6));
    let h = hash(cx, cy, 21);
    if h % 5 < 2 {
        let (ox, oy) = (((h >> 3) % 4) as i32, ((h >> 6) % 4) as i32);
        let bright = (h >> 9).is_multiple_of(3);
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
    bed(wx, wy, (tx, ty), (lx, ly), |dx, dy| road_like(realm.at(tx + dx, ty + dy)))
}

/// A lane cut to a place's door: the road's pebbled bed, edged where the
/// tile beside it (by `open(dx, dy)`) is not road.
pub fn lane(wx: i32, wy: i32, open: impl Fn(i32, i32) -> bool) -> Option<char> {
    let (tx, ty) = (wx.div_euclid(TILE), wy.div_euclid(TILE));
    bed(wx, wy, (tx, ty), (wx.rem_euclid(TILE), wy.rem_euclid(TILE)), open)
}

fn bed(
    wx: i32,
    wy: i32,
    (tx, ty): (i32, i32),
    (lx, ly): (i32, i32),
    open: impl Fn(i32, i32) -> bool,
) -> Option<char> {
    let rag = |a: i32, b: i32, s: u32| (hash(a, b, s) % 3) as i32 + 1;
    let mut edge = false;
    // How far in from the road's nearest edge this pixel lies.
    let mut inward = i32::MAX;
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
            inward = inward.min(dist);
        }
    }
    // A wide road is trodden smooth down its middle: the stones lie along
    // its verges, and the crown is dust.
    let middle = inward > 7;
    if edge {
        return (!hash(wx, wy, 16).is_multiple_of(3)).then_some('I');
    }
    // A pebbled bed: small stones in loose clusters, each lit along its top
    // over a darker body and a shadowed foot, and patches of trodden dust
    // between them; never an even speckle.
    let (cx, cy) = (wx.div_euclid(5), wy.div_euclid(4));
    let (px, py) = (wx.rem_euclid(5), wy.rem_euclid(4));
    let h = hash(cx, cy, 15);
    if h % 5 < if middle { 0 } else { 3 } {
        let (ox, oy) = (((h >> 4) % 3) as i32, ((h >> 7) % 2) as i32);
        let bright = !middle && (h >> 9).is_multiple_of(4);
        if py == oy && (px == ox || px == ox + 1) {
            return Some(if bright && px == ox { 'o' } else { 'r' });
        }
        if py == oy + 1 && (px == ox || px == ox + 1) {
            return Some('P');
        }
        if py == oy + 1 && px == ox + 2 {
            return Some('b');
        }
    }
    let dust = vnoise(wx as f32 / 7.0, wy as f32 / 5.0, 19);
    if middle {
        return (dust > 0.5 && hash(wx, wy, 18) % 5 == 0).then_some('b');
    }
    (dust > 0.62 && hash(wx, wy, 18) % 3 == 0).then_some('b')
}

/// A footpath tile: a narrow ribbon of trodden earth wandering through the
/// tile, joined to whichever neighbours `joined(dx, dy)` says are path or
/// road, its edges ragged. `None` outside the ribbon (the ground shows
/// through); `Some(None)` inside it where nothing is drawn, `Some(Some(ink))`
/// a speck of worn earth.
pub fn trail(wx: i32, wy: i32, on: u8, joined: impl Fn(i32, i32) -> bool) -> Option<Option<char>> {
    let (tx, ty) = (wx.div_euclid(TILE), wy.div_euclid(TILE));
    let (lx, ly) = (wx.rem_euclid(TILE), wy.rem_euclid(TILE));
    let rag = |a: i32, b: i32, s: u32| (hash(a, b, s) % 3) as i32;
    // A ribbon is eight pixels across with the edges worn ragged; a side
    // with no neighbour closes it off.
    let mut inside = true;
    for (open, dist, a, b, salt) in [
        (joined(-1, 0), lx, wy, tx, 25),
        (joined(1, 0), TILE - 1 - lx, wy, tx, 26),
        (joined(0, -1), ly, wx, ty, 27),
        (joined(0, 1), TILE - 1 - ly, wx, ty, 28),
    ] {
        if !open && dist < 5 + rag(a, b, salt) {
            inside = false;
        }
    }
    // Where it runs straight, the ribbon is trimmed to the middle of the
    // tile across, so it never fills a whole tile edge to edge.
    let across = |open_a: bool, open_b: bool, d: i32| open_a && open_b && !(5..TILE - 5).contains(&d);
    if across(joined(-1, 0), joined(1, 0), ly) || across(joined(0, -1), joined(0, 1), lx) {
        inside = false;
    }
    if !inside {
        return None;
    }
    // Worn earth is brown on grass and ash, a darker grey on sand, where
    // the dunes' own speckle is already brown.
    let n = hash(wx, wy, 29) % 100;
    let (a, b) = if on == b's' { ('X', 'g') } else { ('b', 'I') };
    Some(match n {
        0..=29 => Some(a),
        30..=35 => Some(b),
        _ => None,
    })
}

pub fn cobble(wx: i32, wy: i32) -> Option<char> {
    let row = wy.div_euclid(5);
    let off = if row % 2 == 1 { 3 } else { 0 };
    let (lx, ly) = ((wx + off).rem_euclid(6), wy.rem_euclid(5));
    let stone = hash((wx + off).div_euclid(6), row, 17);
    if lx == 0 || ly == 0 || stone.is_multiple_of(5) {
        return None;
    }
    if (lx == 1 || lx == 5) && (ly == 1 || ly == 4) {
        return None;
    }
    Some(if ly == 1 {
        'g'
    } else if stone.is_multiple_of(3) {
        'X'
    } else {
        'K'
    })
}

/// Still water: a pale shoreline and wave dashes drifting east. The bank
/// wanders in long bends and every outer corner of a lake is worn round,
/// so no water is cut in straight runs and right angles.
fn water(realm: &Realm, wx: i32, wy: i32, t: i32) -> Option<char> {
    let wet = |x: i32, y: i32| matches!(realm.at(x, y), b'~' | b'w' | b'f');
    let (tx, ty) = (wx.div_euclid(TILE), wy.div_euclid(TILE));
    let mut mask = 0usize;
    for (i, (dx, dy)) in NEIGHBOURS.iter().enumerate() {
        if !wet(tx + dx, ty + dy) {
            mask |= 1 << i;
        }
    }
    let (lx, ly) = (wx.rem_euclid(TILE), wy.rem_euclid(TILE));
    let depth = bank_table(mask)[(ly * TILE + lx) as usize] as f32;
    if depth == 0.0 {
        return None;
    }
    if depth < 8.0 {
        // Smooth noise keeps near its middle: stretched, so the bank bends.
        let n = ((vnoise(wx as f32 / 12.0, wy as f32 / 12.0, 6) - 0.5) * 3.0).clamp(-1.0, 1.0);
        let s = depth - 1.0 - (1.4 + n * 1.4);
        if s < 0.0 {
            return None;
        }
        if s < 1.0 {
            return Some('Q');
        }
        if s < 2.0 {
            return Some('q');
        }
    }
    let dx = wx - t / 2;
    let (cx, cy) = (dx.div_euclid(9), wy.div_euclid(6));
    let h = hash(cx, cy, 9);
    if h.is_multiple_of(2) {
        let (ox, oy) = (((h >> 4) % 5) as i32, ((h >> 8) % 5) as i32);
        let (lx, ly) = (dx.rem_euclid(9), wy.rem_euclid(6));
        if ly == oy && lx >= ox && lx < ox + 4 {
            return Some(if lx == ox + 1 { 'Q' } else { 'q' });
        }
    }
    None
}

/// A water tile's eight neighbours, the bit order of `bank_table`'s mask.
const NEIGHBOURS: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// How deep each pixel of a water tile lies from its bank, in pixels (0 on
/// the dry ground a worn corner leaves), for the tile's dry neighbours
/// `mask` (one bit per `NEIGHBOURS` entry). The banks are measured over the
/// three by three tiles round it, and every outer corner is worn to a
/// round: what lies a radius from every bank, grown back out by the
/// radius. Worked out once per pattern of neighbours.
fn bank_table(mask: usize) -> &'static [u8] {
    static TABLES: [std::sync::OnceLock<Vec<u8>>; 256] = [const { std::sync::OnceLock::new() }; 256];
    TABLES[mask & 255].get_or_init(|| {
        let n = 3 * TILE;
        let far = u32::MAX / 4;
        let dry = |x: i32, y: i32| {
            let (cx, cy) = (x.div_euclid(TILE) - 1, y.div_euclid(TILE) - 1);
            NEIGHBOURS.iter().position(|&d| d == (cx, cy)).is_some_and(|i| mask & (1 << i) != 0)
        };
        let at = |x: i32, y: i32| (y * n + x) as usize;
        let chamfer = |d: &mut Vec<u32>| {
            for y in 0..n {
                for x in 0..n {
                    for (dx, dy, k) in [(-1, 0, 3), (0, -1, 3), (-1, -1, 4), (1, -1, 4)] {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx >= 0 && ny >= 0 && nx < n {
                            d[at(x, y)] = d[at(x, y)].min(d[at(nx, ny)].saturating_add(k));
                        }
                    }
                }
            }
            for y in (0..n).rev() {
                for x in (0..n).rev() {
                    for (dx, dy, k) in [(1, 0, 3), (0, 1, 3), (1, 1, 4), (-1, 1, 4)] {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx >= 0 && nx < n && ny < n {
                            d[at(x, y)] = d[at(x, y)].min(d[at(nx, ny)].saturating_add(k));
                        }
                    }
                }
            }
        };
        let mut d: Vec<u32> = (0..n * n).map(|i| if dry(i % n, i / n) { 0 } else { far }).collect();
        chamfer(&mut d);
        let round = 5.0;
        let mut core: Vec<u32> = d.iter().map(|&v| if v as f32 / 3.0 >= round { 0 } else { far }).collect();
        chamfer(&mut core);
        (0..TILE * TILE)
            .map(|i| {
                let i = at(TILE + i % TILE, TILE + i / TILE);
                let along = core[i] as f32 / 3.0;
                let depth = if core[i] == 0 { d[i] as f32 / 3.0 } else if along <= round { round - along } else { 0.0 };
                if depth <= 0.0 { 0 } else { depth.ceil().min(15.0) as u8 }
            })
            .collect()
    })
}

pub fn sand(wx: i32, wy: i32) -> Option<char> {
    match hash(wx, wy, 23) % 9 {
        0 => Some('P'),
        1 => Some('I'),
        _ => None,
    }
}

/// Bog: standing pools with a dark rim and short ripples on them, and
/// rushes in small clumps between, never a hatch or a speckle.
fn swamp(wx: i32, wy: i32) -> Option<char> {
    let n = vnoise(wx as f32 / 9.0, wy as f32 / 7.0, 29);
    if n > 0.64 {
        // A pool of dark standing water, a short ripple on it here and there.
        let (cx, cy) = (wx.div_euclid(7), wy.div_euclid(4));
        let h = hash(cx, cy, 30);
        let (ox, oy) = (((h >> 4) % 4) as i32, ((h >> 7) % 4) as i32);
        let (lx, ly) = (wx.rem_euclid(7), wy.rem_euclid(4));
        let ripple = n > 0.68 && h % 3 == 0 && ly == oy && lx >= ox && lx < ox + 3;
        return Some(if ripple { 'q' } else { 'x' });
    }
    // A clump of rushes: two or three stems, dark at the foot, lit at the tip.
    let (cx, cy) = (wx.div_euclid(6), wy.div_euclid(5));
    let h = hash(cx, cy, 31);
    if h % 3 != 0 {
        return None;
    }
    let (ox, oy) = (1 + ((h >> 4) % 3) as i32, ((h >> 6) % 2) as i32);
    let (lx, ly) = (wx.rem_euclid(6), wy.rem_euclid(5));
    let stem = lx == ox || (lx == ox + 2 && (h >> 8) % 2 == 0);
    match ly - oy {
        0 if stem => Some(if (h >> 9) % 3 == 0 { 'N' } else { 'e' }),
        1 | 2 if stem => Some('E'),
        _ => None,
    }
}

/// A sown field: rows of young plants five pixels apart, each plant a small
/// tuft lit on its top and set a little off the plants beside it, a gap
/// here and there, dark furrows between the rows. Never a run of stripes.
fn crops(wx: i32, wy: i32) -> Option<char> {
    let (row, ly) = (wy.div_euclid(5), wy.rem_euclid(5));
    let along = wx + row * 2;
    let (cx, lx) = (along.div_euclid(4), along.rem_euclid(4));
    let h = hash(cx, row, 33);
    if h % 7 == 0 {
        return None;
    }
    let ox = ((h >> 4) % 2) as i32;
    match (lx - ox, ly) {
        (0, 1) => Some(if (h >> 8) % 3 == 0 { 'L' } else { 'l' }),
        (1, 1) if (h >> 10) % 2 == 0 => Some('l'),
        (0, 2) | (1, 2) => Some('E'),
        (0, 3) if (h >> 12) % 3 == 0 => Some('F'),
        _ => None,
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

/// Plank bridges: weathered boards at dusk, a darker seam between them.
pub fn planks(wx: i32, wy: i32) -> char {
    if wy.rem_euclid(4) == 3 {
        'b'
    } else if hash(wx / 5, wy / 4, 3).is_multiple_of(4) {
        'r'
    } else {
        'P'
    }
}

/// Which way a watercourse runs through tile `(tx, ty)`: down when it
/// continues above or below, east otherwise.
fn runs_down(realm: &Realm, (tx, ty): (i32, i32), course: &[u8]) -> bool {
    course.contains(&realm.at(tx, ty - 1)) || course.contains(&realm.at(tx, ty + 1))
}

/// A stream: dark water with pale streaks sliding along its course.
fn stream(realm: &Realm, wx: i32, wy: i32, tile: (i32, i32), t: i32) -> char {
    let down = runs_down(realm, tile, b"wf~");
    let (along, across) = if down { (wy - t, wx) } else { (wx - t, wy) };
    let lane = hash(across, 0, 431) % 5;
    match (along + (hash(across, 1, 432) % 11) as i32).rem_euclid(9) {
        0 if lane < 2 => 'z',
        0 | 1 if lane < 3 => 'Q',
        _ if (wx + wy) % 2 == 0 => 'q',
        _ => 'S',
    }
}

/// A fall pouring down a cliff face — water, or lava — with foam or
/// spatter where it lands.
fn fall(realm: &Realm, wx: i32, wy: i32, (tx, ty): (i32, i32), t: i32, molten: bool) -> char {
    let same = if molten { b'F' } else { b'f' };
    let landing = realm.at(tx, ty + 1) != same && wy.rem_euclid(TILE) >= TILE - 3;
    let phase = (wy - t * 2 + (hash(wx, 0, 441) % 7) as i32).rem_euclid(6);
    // Mostly dark water with streaks sliding down it; spray where it lands.
    let streak = hash(wx, 1, 444).is_multiple_of(3);
    match (molten, landing, phase) {
        (false, true, _) => match hash(wx, wy + t, 442) % 4 {
            0 => 'V',
            1 => 'z',
            _ => 'Q',
        },
        (true, true, _) => match hash(wx, wy + t, 443) % 4 {
            0 => 'o',
            1 => 'R',
            _ => 'p',
        },
        (false, false, 0) if streak => 'z',
        (false, false, 0 | 1) => 'Q',
        (false, false, 2 | 3) => 'q',
        (false, false, _) => 'S',
        (true, false, 0) if streak => 'o',
        (true, false, 0 | 1) => 'R',
        (true, false, 2 | 3) => 'p',
        (true, false, _) => 'B',
    }
}

/// Lava: a warm crust creeping along its course, glowing through cracks.
fn lava(realm: &Realm, wx: i32, wy: i32, tile: (i32, i32), t: i32) -> char {
    let down = runs_down(realm, tile, b"LF");
    let (along, across) = if down {
        (wy - t / 2, wx)
    } else {
        (wx - t / 2, wy)
    };
    // Mostly dark crust; the warm melt shows through in slow-moving seams.
    let crust = vnoise(along as f32 / 6.0, across as f32 / 3.0, 451);
    if crust > 0.62 {
        'n'
    } else if crust > 0.5 {
        'b'
    } else if crust > 0.38 {
        'B'
    } else if crust > 0.24 {
        'p'
    } else if crust > 0.14 {
        'R'
    } else {
        'o'
    }
}
