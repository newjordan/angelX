//! Bomrekedan drawn (`rules::world::bomrekedan`): each level of the fort as
//! its tiles lie, dressed with what stands on them. Smoothed stone underfoot
//! (grass on the surface), every natural wall in the rock it is cut from
//! (ore and gem seams in the mine's ramp, coal and obsidian in charcoal),
//! water on the surface and the dark below, the doors in timber and the
//! stairs as steps, the way out ringed in gold.
//!
//! Over the ground, the DRESS grid (the README's keys) in the realm's kit:
//! each workshop, furnace and the trade depot drawn once over its place's
//! rect (a forge, a loom, a cooper's barrels, a granary's sacks by the
//! workshop's kind), windmills by threes, farm plots in rows, stockpiles
//! strewn with barrels, crates and sacks, and every bed, table, chair,
//! statue, cabinet, bookcase, chest, rack, well, altar and shrub as a small
//! piece of its own, nearer rows over farther. A key the painter does not
//! know leaves a small mark. The drawing is kept: a level is far too big to
//! draw five times a second (`scenery::torch_step`).
//!
//! The fort's people (`bomrekedan::people`) are drawn every frame over it,
//! each in their own colours, by `figures`.

use super::super::ink::{Img, hash};
use super::super::kit::{self, RockKind, Tiles, Trade};
use super::super::material::{self as mat, Leaf, ramp};
use super::nooks::{IRON, LINEN, all};
use super::{TILE, flags, props, stand};
use crate::rules::world::bomrekedan::people::{self, Person, Race};
use crate::rules::world::bomrekedan::{self, DOORS, Level};
use crate::rules::{Room, Run, Tile};
use std::sync::OnceLock;

const T: i32 = TILE;

/// The level at rest.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let mut cv = Img::black(cols * T, rows * T);
    let Some(level) = bomrekedan::level(room.kind) else {
        return cv;
    };
    let tiles = Tiles::get();
    for row in 0..rows {
        for col in 0..cols {
            let (x, y) = (col * T, row * T);
            match room.tile(col, row) {
                Tile::Wall => cv.stamp(tiles.rock(rock(level.rock_at(col, row)), hash(col, row, 7)), x, y),
                Tile::Door => door(&mut cv, x, y),
                Tile::Stairs => steps(&mut cv, x, y),
                Tile::Hazard if !level.surface() => chasm(&mut cv, x, y, room.tile(col, row - 1) != Tile::Hazard),
                // What stands on a blocked tile is drawn over its ground.
                Tile::Block if dressed(level.dress_at(col, row)) => ground(&mut cv, level, x, y, col, row),
                Tile::Block => mass(&mut cv, x, y),
                _ => ground(&mut cv, level, x, y, col, row),
            }
        }
    }
    if level.surface()
        && let Some(pond) = wet_bounds(room)
    {
        let wet = |x: i32, y: i32| room.tile(x.div_euclid(T), y.div_euclid(T)) == Tile::Hazard;
        mat::water_field(&mut cv, pond, &wet, 61, 0);
    }
    lay_flat(&mut cv, level, room);
    let mut pieces = pieces(level, room);
    // Row by row, so a nearer piece stands over a farther.
    pieces.sort_by_key(|p| (p.foot, p.x));
    for p in &pieces {
        p.draw(&mut cv, tiles, !level.surface());
    }
    // The way out to the kingdom, ringed in gold.
    for e in DOORS.iter().filter(|e| e.room == room.kind && e.id == "bomrekedan-out") {
        let (c, r, w, h) = e.at;
        let (x, y, w, h) = (c * T, r * T, w * T, h * T);
        cv.line(x, y, x + w - 1, y, '5');
        cv.line(x, y + h - 1, x + w - 1, y + h - 1, '5');
        cv.line(x, y, x, y + h - 1, '5');
        cv.line(x + w - 1, y, x + w - 1, y + h - 1, '5');
    }
    stair_cues(&mut cv, room);
    cv
}

/// How far a tile sits from the nearest lamp (a stair, a door, a furnace,
/// a workshop, an altar, a well). `0` is the lamp. The surface is open sky
/// and stays `0`. Past the pools the stone is dim, never a black void.
fn shade_at(level: &Level, col: i32, row: i32) -> u8 {
    if level.surface() {
        return 0;
    }
    let (w, h) = level.size;
    if col < 0 || row < 0 || col as usize >= w || row as usize >= h {
        return 8;
    }
    lights(level)[row as usize * w + col as usize]
}

fn lights(level: &Level) -> &'static [u8] {
    static ALL: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    let all = ALL.get_or_init(|| bomrekedan::levels().iter().map(light_field).collect());
    &all[bomrekedan::index(level.kind).unwrap_or(0)]
}

fn light_field(level: &Level) -> Vec<u8> {
    let (w, h) = level.size;
    let mut dist = vec![8u8; w * h];
    let mut q = Vec::new();
    let at = |c: usize, r: usize| r * w + c;
    for r in 0..h {
        for c in 0..w {
            let (x, y) = (c as i32, r as i32);
            let lamp = matches!(level_tile(level, x, y), Tile::Stairs | Tile::Door)
                || matches!(level.dress_at(x, y), b'F' | b'W' | b'o' | b'w');
            if lamp {
                dist[at(c, r)] = 0;
                q.push((c, r));
            }
        }
    }
    let mut head = 0;
    while head < q.len() {
        let (c, r) = q[head];
        head += 1;
        let d = dist[at(c, r)];
        if d >= 7 {
            continue;
        }
        for (dc, dr) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nc, nr) = (c as i32 + dc, r as i32 + dr);
            if nc < 0 || nr < 0 || nc as usize >= w || nr as usize >= h {
                continue;
            }
            let (nc, nr) = (nc as usize, nr as usize);
            if level_tile(level, nc as i32, nr as i32) == Tile::Wall || dist[at(nc, nr)] <= d + 1 {
                continue;
            }
            dist[at(nc, nr)] = d + 1;
            q.push((nc, nr));
        }
    }
    dist
}

fn level_tile(level: &Level, x: i32, y: i32) -> Tile {
    let (w, h) = level.size;
    if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
        return Tile::Wall;
    }
    level.tiles[y as usize * w + x as usize]
}

/// Amber rims and a chevron on every stair but the gold way out, so a shaft
/// reads as travel before a knight is standing on it.
fn stair_cues(cv: &mut Img, room: &Room) {
    for e in DOORS.iter().filter(|e| e.room == room.kind && e.id != "bomrekedan-out") {
        let (c, r, w, h) = e.at;
        let (x, y, pw, ph) = (c * T, r * T, w * T, h * T);
        if pw < 4 || ph < 4 {
            continue;
        }
        cv.frame(x + 1, y + 1, pw - 2, ph - 2, 'O');
        let mid = x + pw / 2;
        let lip = y + ph - 3;
        cv.put(mid - 3, lip - 1, 'V');
        cv.put(mid, lip, 'V');
        cv.put(mid + 3, lip - 1, 'V');
    }
}

/// The pixels the surface's water is laid over: its tiles' bounds and a
/// tile of dry ground round them, inside the level (`None`: no water).
/// Water lies only on wet pixels, its banks reckoned from the nearest dry
/// ground, and that ring of dry ground is nearer than anything beyond it:
/// the same water as laid over the whole level, over a fifth of the Gate's
/// pixels (the banks' distance field was most of its bake).
fn wet_bounds(room: &Room) -> Option<(i32, i32, i32, i32)> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let wet = (0..rows).flat_map(|r| (0..cols).map(move |c| (c, r))).filter(|&(c, r)| room.tile(c, r) == Tile::Hazard);
    let (c0, r0, c1, r1) = wet.fold(None, |b: Option<(i32, i32, i32, i32)>, (c, r)| {
        Some(b.map_or((c, r, c, r), |(c0, r0, c1, r1)| (c0.min(c), r0.min(r), c1.max(c), r1.max(r))))
    })?;
    let (c0, r0, c1, r1) = ((c0 - 1).max(0), (r0 - 1).max(0), (c1 + 1).min(cols - 1), (r1 + 1).min(rows - 1));
    Some((c0 * T, r0 * T, (c1 - c0 + 1) * T, (r1 - r0 + 1) * T))
}

/// Whether the painter has its own art for DRESS key `ch` (the README's
/// keys); any other key gets a small mark.
pub(super) fn dressed(ch: u8) -> bool {
    matches!(
        ch,
        b'b' | b't' | b'c' | b's' | b'k' | b'K' | b'h' | b'a' | b'r' | b'W' | b'F' | b'f' | b'p' | b'P' | b'g' | b'm' | b'w' | b'o' | b'C' | b'd' | b'n' | b'T' | b'"' | b'=' | b'x' | b'A' | b'j' | b'+'
    )
}

/// The ramp a natural wall is drawn in, by what it is made of (the
/// README's ROCK keys).
fn rock(made: u8) -> RockKind {
    match made {
        b'k' | b'o' => RockKind::Char,
        b'i' | b'c' | b't' | b'g' | b'v' | b'P' | b'n' | b'j' | b'J' | b'x' | b'a' => RockKind::Ore,
        _ => RockKind::Slate,
    }
}

/// A tile of open ground: grass on the surface, smoothed flags below.
/// Underground, a lamp (stair, door, furnace, workshop, altar, well) warms
/// the flags within seven tiles; farther stone is dim, and a wall drops a
/// two-pixel skirt onto the floor in front of it.
fn ground(cv: &mut Img, level: &Level, x: i32, y: i32, col: i32, row: i32) {
    let shade = shade_at(level, col, row);
    for py in y..y + T {
        for px in x..x + T {
            let ch = if level.surface() {
                match hash(px / 3, py / 3, 23) % 11 {
                    0 => 'm',
                    1..=3 => 'l',
                    4 => 'F',
                    _ => 'E',
                }
            } else {
                let base = flags(px, py).unwrap_or(if hash(px / 8, py / 8, 29).is_multiple_of(5) { 'X' } else { 'g' });
                lit_flag(base, shade, px, py)
            };
            cv.put(px, py, ch);
        }
    }
    if !level.surface() && level_tile(level, col, row - 1) == Tile::Wall {
        for px in x + 1..x + T - 1 {
            cv.put(px, y, 'x');
            cv.put(px, y + 1, 'X');
        }
    }
}

/// Near a lamp the flags go warm. Mid distance keeps them. Far off they dim.
fn lit_flag(ch: char, shade: u8, px: i32, py: i32) -> char {
    match shade {
        0..=2 => match ch {
            'g' | 'x' => 'T',
            'X' => 'O',
            'i' => 'V',
            other => other,
        },
        3..=5 => ch,
        _ => match hash(px, py, 41) % 5 {
            0 => 'x',
            1 => 'X',
            _ => 'g',
        },
    }
}

/// Something solid standing on the floor: a dark mass, lit on top.
fn mass(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y, T, T, 'K');
    cv.rect(x + 1, y + 1, T - 2, T - 3, 'G');
    cv.rect(x + 1, y + 1, T - 2, 3, 'J');
    cv.line(x + 1, y + T - 2, x + T - 2, y + T - 2, 'X');
}

/// A door: planks in a frame, a stud.
fn door(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y, T, T, 'n');
    cv.rect(x + 2, y + 2, T - 4, T - 4, 'B');
    for k in (x + 5..x + T - 2).step_by(4) {
        cv.line(k, y + 2, k, y + T - 3, 'r');
    }
    cv.put(x + T - 5, y + T / 2, 'O');
}

/// A stair: steps going down, pale treads over dark risers.
fn steps(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y, T, T, 'x');
    for k in 0..T / 4 {
        let sy = y + k * 4;
        cv.line(x + 1, sy, x + T - 2, sy, 'V');
        cv.line(x + 1, sy + 1, x + T - 2, sy + 1, 'U');
    }
}

/// Open air, a pit or deep water underground: the dark, a lit lip on its
/// north side.
fn chasm(cv: &mut Img, x: i32, y: i32, lip: bool) {
    cv.rect(x, y, T, T, 'k');
    for k in 0..3 {
        let h = hash(x + k, y, 31);
        cv.put(x + (h % T as u32) as i32, y + ((h >> 8) % T as u32) as i32, 'K');
    }
    if lip {
        cv.rect(x, y, T, 2, 'G');
    }
}

// ─── what lies flat ──────────────────────────────────────────────────────────

/// The kit's garden bed, sampled under every farm plot so the rows run on
/// across a field.
fn garden_bed() -> &'static Img {
    static BED: OnceLock<Img> = OnceLock::new();
    BED.get_or_init(kit::garden)
}

/// What lies on the ground and is walked over: farm plots in rows, a
/// stockpile's corner marks, the planks of a hatch or bridge.
fn lay_flat(cv: &mut Img, level: &Level, room: &Room) {
    let bed = garden_bed();
    let (iw, ih) = (bed.w - 2, bed.h - 2);
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            let (x, y) = (col * T, row * T);
            match level.dress_at(col, row) {
                b'f' => {
                    for py in y..y + T {
                        for px in x..x + T {
                            if let Some(c) = bed.get(1 + px.rem_euclid(iw), 1 + py.rem_euclid(ih)) {
                                cv.set(px, py, c);
                            }
                        }
                    }
                }
                b'p' => {
                    // The pile's bounds, a dot at each corner of each tile.
                    for (dx, dy) in [(1, 1), (T - 2, 1), (1, T - 2), (T - 2, T - 2)] {
                        cv.put(x + dx, y + dy, 'j');
                    }
                }
                b'=' => {
                    mat::boards(cv, (x, y + 1, T, T - 2), (col + row) % 2 == 0, hash(col, row, 211));
                    cv.line(x, y + 1, x + T - 1, y + 1, 'n');
                }
                _ => {}
            }
        }
    }
}

// ─── what stands ─────────────────────────────────────────────────────────────

/// One thing standing on a level, drawn with its foot at `foot` (pixels).
struct Piece {
    foot: i32,
    x: i32,
    what: What,
}

enum What {
    /// A workshop, a furnace or the depot over its place's rect (tiles), by
    /// its `kingdom.json` kind.
    Shop((i32, i32, i32, i32), &'static str),
    /// A windmill over the three by three at a tile.
    Mill(i32, i32),
    /// A tree on the surface.
    Tree(i32, i32),
    /// One tile's piece, by its DRESS key.
    Thing(i32, i32, u8),
}

/// Everything standing on the level: the places' workshops and furnaces
/// first, each taking its tiles; windmills by threes; then each tile's own.
fn pieces(level: &Level, room: &Room) -> Vec<Piece> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let mut claimed = vec![false; (cols * rows) as usize];
    let at = |c: i32, r: i32| (r * cols + c) as usize;
    let mut out = Vec::new();
    for p in bomrekedan::places_on(level.kind) {
        let kind = p.kind.as_str();
        if !(kind.starts_with("workshop:") || kind.starts_with("furnace:") || kind == "depot") {
            continue;
        }
        let (c, r, w, h) = (p.rect.x, p.rect.y, p.rect.w, p.rect.h);
        let mut any = false;
        for y in r.max(0)..(r + h).min(rows) {
            for x in c.max(0)..(c + w).min(cols) {
                if matches!(level.dress_at(x, y), b'W' | b'F') {
                    claimed[at(x, y)] = true;
                    any = true;
                }
            }
        }
        if any {
            out.push(Piece { foot: (r + h) * T, x: c * T, what: What::Shop((c, r, w, h), kind) });
        }
    }
    // Windmills and leftover workshops, three by three where they fit.
    for key in [b'm', b'W'] {
        for r in 0..rows {
            for c in 0..cols {
                let block = (0..3).all(|dy| (0..3).all(|dx| c + dx < cols && r + dy < rows && level.dress_at(c + dx, r + dy) == key && !claimed[at(c + dx, r + dy)]));
                if !block {
                    continue;
                }
                for dy in 0..3 {
                    for dx in 0..3 {
                        claimed[at(c + dx, r + dy)] = true;
                    }
                }
                let what = if key == b'm' { What::Mill(c, r) } else { What::Shop((c, r, 3, 3), "workshop") };
                out.push(Piece { foot: (r + 3) * T, x: c * T, what });
            }
        }
    }
    for r in 0..rows {
        for c in 0..cols {
            if claimed[at(c, r)] {
                continue;
            }
            let ch = level.dress_at(c, r);
            let what = match ch {
                b' ' | b'f' | b'=' => continue,
                b'T' => What::Tree(c, r),
                // Not every tile of a pile holds something.
                b'p' if hash(c, r, 223) % 9 > 3 => continue,
                _ => What::Thing(c, r, ch),
            };
            out.push(Piece { foot: (r + 1) * T, x: c * T, what });
        }
    }
    out
}

impl Piece {
    fn draw(&self, cv: &mut Img, tiles: &Tiles, underground: bool) {
        match self.what {
            What::Shop(rect, kind) => shop(cv, rect, kind, underground),
            What::Mill(c, r) => {
                let im = kit::windmill(hash(c, r, 227) % 40);
                cv.stamp(&im, c * T + (3 * T - im.w) / 2, (r + 3) * T - im.h);
            }
            What::Tree(c, r) => {
                let im = tiles.tree(hash(c, r, 19));
                cv.stamp(im, c * T + (T - im.w) / 2, r * T + T - im.h);
            }
            What::Thing(c, r, ch) => thing(cv, c * T, r * T, ch, hash(c, r, 229)),
        }
    }
}

/// A workshop, a furnace or the trade depot over its rect (tiles).
fn shop(cv: &mut Img, (c, r, w, h): (i32, i32, i32, i32), kind: &str, underground: bool) {
    let (x, y, pw, ph) = (c * T, r * T, w * T, h * T);
    let seed = hash(c, r, 231);
    if kind == "depot" {
        // The depot: a striped stall on a boarded floor, goods about it.
        mat::boards(cv, (x + 2, y + ph / 2, pw - 4, ph / 2 - 2), false, seed);
        props::stall(cv, (x + 8, y + 18, pw - 16, ph - 30), ('T', 'R'), y + ph - 30, y + 12, seed);
        cv.stamp(&kit::barrels(), x + 2, y + ph - 16);
        cv.stamp(&kit::sacks(5), x + pw - 23, y + ph - 13);
        cv.stamp(&kit::handcart(), x + pw / 2 - 12, y + ph - 15);
        return;
    }
    if kind.starts_with("furnace:") {
        props::hearth(cv, (x, y, pw, ph), 2, seed);
        return;
    }
    let trade = match kind.trim_start_matches("workshop:") {
        "metalsmithsforge" => Trade::Smith,
        "clothiers" | "weavers" | "leatherworks" | "dyers" => Trade::Weaver,
        "carpenters" | "still" | "bowyers" => Trade::Cooper,
        "kitchen" | "farmers" | "butchers" | "fishery" | "quern" | "millstone" => Trade::Granary,
        _ => Trade::Smith,
    };
    let active = matches!(kind, "workshop:metalsmithsforge");
    let mut im = kit::workshop(trade, active);
    if underground {
        open_the_roof(&mut im, matches!(trade, Trade::Smith));
    }
    cv.stamp(&im, x + (pw - im.w) / 2, y + ph - im.h - 1);
    // A mason's, a jeweller's or a mechanic's bench shows its work.
    match kind {
        "workshop:masons" => mat::blocks(cv, (x + pw - 16, y + ph - 12, 14, 10), ramp::STONE, (5, 3), seed),
        "workshop:jewelers" => {
            for k in 0..3 {
                cv.put(x + pw - 12 + k * 3, y + ph - 6, ['z', 'Q', 'T'][k as usize]);
            }
        }
        "workshop:mechanics" => gear(cv, x + pw - 9, y + ph - 9, seed),
        _ => {}
    }
}

/// An underground shop keeps the bay and loses the outdoor roof: a stone
/// lintel, a timber beam, and a hanging lamp. The image stays the same size,
/// so the stamp still sits on the floor.
fn open_the_roof(im: &mut Img, smith: bool) {
    let top = if smith { 6 } else { 0 };
    let wall_y0 = top + 15 - 1;
    for y in 0..wall_y0.min(im.h) {
        for x in 0..im.w {
            im.clear(x, y);
        }
    }
    if wall_y0 <= 2 || wall_y0 >= im.h {
        return;
    }
    for x in 1..im.w - 1 {
        im.put(x, wall_y0 - 2, 'S');
        im.put(x, wall_y0 - 1, if x % 4 == 0 { 'X' } else { 'S' });
    }
    let beam = wall_y0 / 2;
    for x in 2..im.w - 2 {
        im.put(x, beam, 'B');
    }
    im.put(im.w / 2, beam + 2, 'O');
    im.put(im.w / 2, beam + 3, 'V');
}

/// One tile's piece at `(x, y)` (the tile's top left, pixels), by its
/// DRESS key.
fn thing(cv: &mut Img, x: i32, y: i32, ch: u8, seed: u32) {
    let (cx, foot) = (x + T / 2, y + T - 1);
    match ch {
        b'b' => bed(cv, x, y, seed),
        b't' => table(cv, x, y, seed),
        b'c' => chair(cv, x, y, seed),
        b's' => statue(cv, cx, foot, seed),
        b'k' => cabinet(cv, x, y, seed),
        b'K' => {
            // A bookcase: a timber frame, two shelves of books.
            mat::block(cv, (x + 1, y, T - 2, T - 1), ramp::TIMBER, true, seed);
            let books = kit::books(4);
            cv.stamp(&books, x + 2, y + 1);
            cv.stamp(&books, x + 2, y + 8);
        }
        b'h' => {
            mat::shadow(cv, ((cx + 2) as f32, foot as f32), (7.0, 2.0), &all);
            cv.stamp(&kit::chest(true), x + 2, y + 6);
        }
        b'a' => armour_stand(cv, cx, foot),
        b'r' => {
            let im = kit::blade_rack(4);
            cv.stamp(&im, x + (T - im.w) / 2, foot - im.h + 1);
        }
        b'W' => mat::crate_box(cv, cx, foot, seed),
        b'F' => {
            let im = kit::brazier(0, seed);
            cv.stamp(&im, x + (T - im.w) / 2, foot - im.h + 1);
        }
        b'p' => match seed % 3 {
            0 => mat::barrel(cv, cx, foot),
            1 => mat::crate_box(cv, cx, foot, seed),
            _ => cv.stamp(&kit::sacks(2), x - 3, y + 2),
        },
        b'P' => pump(cv, x, y),
        // A machine's gear, or a windmill's tile not in a three by three.
        b'g' | b'm' => gear(cv, cx, y + T / 2, seed),
        b'w' => {
            let im = kit::well();
            cv.stamp(&im, x + (T - im.w) / 2, foot - im.h + 1);
        }
        b'o' => {
            // An altar: a dressed stone block, candles on it.
            mat::block(cv, (x + 1, y + 5, T - 2, T - 6), ramp::STONE, false, seed);
            cv.line(x + 1, y + 5, x + T - 2, y + 5, 'i');
            let candles = kit::candles(4, 0);
            cv.stamp(&candles, x + (T - candles.w) / 2, y + 1);
        }
        b'C' => props::slab_down(cv, x + 2, y + 8, (1, 0)),
        b'd' => {
            // A display case: glass over a plinth, the piece inside.
            mat::block(cv, (x + 2, y + 10, T - 4, 5), ramp::STONE, false, seed);
            cv.rect(x + 3, y + 2, T - 6, 8, 'q');
            cv.frame(x + 3, y + 2, T - 6, 8, 'V');
            cv.rect(x + 6, y + 5, 4, 3, ['O', 'z', 'T'][(seed % 3) as usize]);
        }
        b'n' => {
            mat::hay(cv, cx, foot, 4.5, seed);
            cv.put(cx - 1, foot - 5, 'T');
        }
        b'"' => mat::bush(cv, cx, foot - 1, 3.0 + (seed % 3) as f32 * 0.5, if seed % 4 == 0 { Leaf::Oak } else { Leaf::Hedge }, seed),
        b'x' => {
            // A trap or a chain: iron links across the tile.
            for k in 0..4 {
                cv.frame(x + 1 + k * 4, y + 6 + (k % 2), 4, 3, if k % 2 == 0 { 'J' } else { 'G' });
            }
        }
        b'A' => props::butt(cv, cx, foot - 15),
        b'j' => {
            // A traction bench: a board on legs, a strap and a winch.
            mat::boards(cv, (x + 1, y + 7, T - 2, 4), false, seed);
            cv.line(x + 2, y + 11, x + 2, foot, 'b');
            cv.line(x + T - 3, y + 11, x + T - 3, foot, 'b');
            cv.line(x + 6, y + 6, x + 6, y + 11, 'o');
            cv.ellipse((x + T - 4) as f32, (y + 5) as f32, 2.0, 2.0, 'g');
        }
        b'+' => {
            // Work not yet done: scaffold poles braced across, blocks
            // waiting.
            cv.line(x + 2, y + 1, x + 2, foot, 'B');
            cv.line(x + T - 3, y + 1, x + T - 3, foot, 'B');
            cv.line(x + 2, y + 2, x + T - 3, foot - 2, 'r');
            cv.line(x + T - 3, y + 2, x + 2, foot - 2, 'r');
            mat::blocks(cv, (x + 4, foot - 4, 8, 4), ramp::STONE, (4, 2), seed);
        }
        _ => {
            // A thing the painter has no art for: a small mark.
            cv.put(cx, foot - 2, 'h');
            cv.put(cx + 1, foot - 2, 'J');
        }
    }
}

/// A bed: a timber frame, a pillow at its head, the blanket turned down.
fn bed(cv: &mut Img, x: i32, y: i32, seed: u32) {
    mat::boards(cv, (x + 2, y + 1, T - 4, T - 2), true, seed);
    let blanket = ['Q', 'R', 'E', 'u'][(seed % 4) as usize];
    cv.rect(x + 3, y + 6, T - 6, T - 8, blanket);
    cv.line(x + 3, y + 6, x + T - 4, y + 6, mat::pick(LINEN, 0.8));
    cv.rect(x + 4, y + 2, T - 8, 3, mat::pick(LINEN, 0.95));
    cv.line(x + 4, y + 4, x + T - 5, y + 4, mat::pick(LINEN, 0.4));
}

/// A table: a boarded top on two legs, its shadow under it.
fn table(cv: &mut Img, x: i32, y: i32, seed: u32) {
    mat::shadow(cv, ((x + 9) as f32, (y + 14) as f32), (7.0, 2.0), &all);
    mat::boards(cv, (x + 1, y + 4, T - 2, 6), false, seed);
    cv.line(x + 1, y + 4, x + T - 2, y + 4, 'o');
    cv.line(x + 3, y + 10, x + 3, y + 14, 'b');
    cv.line(x + T - 4, y + 10, x + T - 4, y + 14, 'b');
}

/// A chair: a seat, a back rail, legs.
fn chair(cv: &mut Img, x: i32, y: i32, seed: u32) {
    let (l, r) = (x + 4, x + T - 5);
    cv.rect(l, y + 2, r - l + 1, 2, 'B');
    cv.line(l, y + 2, l, y + 13, 'b');
    cv.line(r, y + 2, r, y + 13, 'b');
    mat::boards(cv, (l, y + 8, r - l + 1, 3), false, seed);
    cv.line(l, y + 8, r, y + 8, 'o');
}

/// A statue on its plinth: a figure in dressed stone, lit from the
/// north-west.
fn statue(cv: &mut Img, cx: i32, foot: i32, seed: u32) {
    mat::shadow(cv, ((cx + 3) as f32, foot as f32), (7.0, 2.0), &all);
    mat::block(cv, (cx - 6, foot - 4, 12, 4), ramp::STONE, false, seed);
    let stone = if seed % 3 == 0 { ramp::SLATE } else { ramp::STONE };
    mat::lobe(cv, (cx as f32, (foot - 9) as f32), (3.5, 5.0), stone, seed, 1.5, 0.05, &all);
    mat::lobe(cv, (cx as f32, (foot - 16) as f32), (2.5, 2.5), stone, seed ^ 5, 1.5, 0.0, &all);
}

/// A cabinet: boarded doors in a frame, a ring pull.
fn cabinet(cv: &mut Img, x: i32, y: i32, seed: u32) {
    mat::boards(cv, (x + 2, y + 1, T - 4, T - 2), true, seed);
    cv.frame(x + 2, y + 1, T - 4, T - 2, 'b');
    cv.line(x + T / 2, y + 2, x + T / 2, y + T - 3, 'n');
    cv.put(x + T / 2 - 2, y + T / 2, 'O');
    cv.put(x + T / 2 + 1, y + T / 2, 'O');
}

/// An armour stand: a helm over a mail shirt on a post.
fn armour_stand(cv: &mut Img, cx: i32, foot: i32) {
    mat::shadow(cv, ((cx + 2) as f32, foot as f32), (5.0, 1.5), &all);
    cv.line(cx, foot - 4, cx, foot, 'b');
    cv.line(cx - 3, foot, cx + 3, foot, 'b');
    for yy in foot - 11..foot - 4 {
        for xx in cx - 3..=cx + 3 {
            let lit = (xx + yy) % 2 == 0;
            cv.put(xx, yy, mat::pick(IRON, if lit { 0.75 } else { 0.4 } - (xx - cx + 3) as f32 * 0.04));
        }
    }
    mat::lobe(cv, (cx as f32, (foot - 13) as f32), (2.5, 2.5), IRON, 172, 1.5, 0.0, &all);
}

/// A screw pump: an iron pipe at a slant, its thread lit.
fn pump(cv: &mut Img, x: i32, y: i32) {
    for k in 0..T {
        let (px, py) = (x + k, y + T - 3 - k * 3 / 4);
        cv.rect(px, py - 2, 1, 5, 'g');
        if k % 3 == 0 {
            cv.put(px, py - 1, 'J');
        }
    }
    cv.rect(x + T - 4, y, 3, 4, 'X');
}

/// A gear on its axle: a toothed iron wheel.
fn gear(cv: &mut Img, cx: i32, cy: i32, seed: u32) {
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4 + (seed % 3) as f32 * 0.2;
        cv.put(cx + (a.cos() * 6.0).round() as i32, cy + (a.sin() * 6.0).round() as i32, 'G');
    }
    cv.ellipse(cx as f32, cy as f32, 5.0, 5.0, 'X');
    cv.ellipse(cx as f32, cy as f32, 4.0, 4.0, 'j');
    cv.ellipse(cx as f32, cy as f32, 1.5, 1.5, 'J');
}

// ─── the people ──────────────────────────────────────────────────────────────

/// A person's figure, two frames of a walk, rows of keys: `H` hair, `S`
/// skin, `E` eyes, `B` beard, `R` cloth, `D` boots.
const SHORT: [&str; 10] = [
    "...HHH....",
    "..HSSSH...",
    "..SESES...",
    "...BBB....",
    "..RRRRR...",
    ".SRRRRRS..",
    "..RRRRR...",
    "..RRRRR...",
    "..DD.DD...",
    "..D...D...",
];
const TALL: [&str; 12] = [
    "...HHH....",
    "..HSSSH...",
    "..SESES...",
    "...SSS....",
    "..RRRRR...",
    ".SRRRRRS..",
    ".SRRRRRS..",
    "..RRRRR...",
    "..RRRRR...",
    "..RR.RR...",
    "..D...D...",
    "..D...D...",
];

/// Cloth by the work the fort has them do; no signal ink.
fn cloth(job: &str) -> char {
    match job {
        "guard" => 'J',
        "mason" => 'u',
        "performer" => '[',
        "crafter" => 'R',
        "farmer" => 'm',
        "smith" => 'X',
        "carpenter" => 'o',
        "mechanic" => 'G',
        "doctor" => 'V',
        "hauler" => 'P',
        "herder" => 'L',
        "miner" => 'B',
        "noble" => ']',
        "jeweler" => 'z',
        "butcher" => 'p',
        "fisher" => 'Q',
        "clerk" => 'S',
        "priest" => '^',
        "cook" => 'T',
        "brewer" => 'r',
        _ => 'U',
    }
}

/// A person's two frames, built once for everyone.
fn looks() -> &'static [[Img; 2]] {
    static LOOKS: OnceLock<Vec<[Img; 2]>> = OnceLock::new();
    LOOKS.get_or_init(|| people::all().iter().map(|p| [look(p, 0), look(p, 1)]).collect())
}

fn look(p: &Person, frame: u32) -> Img {
    let seed = hash(p.id as i32, 3, 241);
    let hair = ['B', 'r', 'R', 'o', 'g', 'b', 'h'][(seed % 7) as usize];
    let (skin, hair, eyes, tall) = match p.race {
        Race::Dwarf => (['t', 'o', 'O'][(seed >> 4) as usize % 3], hair, 'k', false),
        Race::Human => (['t', 'o', 'r'][(seed >> 4) as usize % 3], hair, 'k', true),
        // The erinyes: ash-pale, black-haired, dark-eyed.
        Race::Erinys => ('V', 'k', '(', true),
        // The gnolls: brown fur, a darker mane.
        Race::Gnoll => ('r', 'b', 'k', true),
        Race::Other => ('O', hair, 'k', true),
    };
    let beard = if p.race == Race::Dwarf && !p.female { hair } else { skin };
    let cloth = match p.race {
        Race::Erinys => '(',
        _ => cloth(&p.job),
    };
    let rows: &[&str] = if tall { &TALL } else { &SHORT };
    let n = rows.len();
    let rows: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            // The second frame of the walk: the feet in, mid-stride.
            let row = if frame == 1 && i >= n - 2 { "...D.D...." } else { row };
            row.chars()
                .map(|c| match c {
                    'H' => hair,
                    'S' => skin,
                    'E' => eyes,
                    'B' => beard,
                    'R' => cloth,
                    'D' => 'n',
                    other => other,
                })
                .collect()
        })
        .collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    Img::from_rows(&refs)
}

/// The people of the level the party stands on, where they are this tick,
/// nearer over farther.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    let here = people::on(kind);
    if here.is_empty() {
        return;
    }
    let looks = looks();
    let mut stood: Vec<(i32, i32, &Img)> = here
        .iter()
        .map(|p| {
            let (x, y) = p.at(run.tick);
            let walk = p.walking(run.tick);
            let frame = walk.map_or(0, |_| (run.tick / 6 % 2) as usize);
            let im = &looks[p.slot][frame];
            ((x * T as f32) as i32, (y * T as f32) as i32 + 5, im)
        })
        .collect();
    stood.sort_by_key(|&(x, y, _)| (y, x));
    for (x, y, im) in stood {
        cv.put(x + 4, y + im.h - 2, 'x');
        cv.put(x + im.w - 5, y + im.h - 2, 'x');
        stand(cv, im, x, y);
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__bomrekedan__shots.rs"]
mod tests;
