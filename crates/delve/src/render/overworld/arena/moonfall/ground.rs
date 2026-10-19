//! The fen town on the ground: tiles, houses, props and everyone standing
//! on them, as labelled blocks. Nothing here decides what is solid.
use super::super::super::ink::{Img, hash, text_width};
use super::labels::{Kind, Layout, Part, Rect};
use super::{Cam, VIEW_H, VIEW_Y, bank};
use crate::rules::moonfall::{
    Session,
    folk::{CAST, Folk},
    state::{DITCH_FILLED, Item, Knight, Mask, WispMode},
    town::{self, COLS, HOUSES, LAMPS, ROWS, TILE, Tile},
};

/// Draw `s` on a dark plate so it reads over any ground.
pub(super) fn plate(im: &mut Img, x: i32, y: i32, s: &str, ink: char) {
    im.rect(x - 1, y - 1, text_width(s) + 2, 9, 'k');
    im.text(x, y, s, ink);
}

pub(super) fn plate_centred(im: &mut Img, cx: i32, y: i32, s: &str, ink: char) {
    plate(im, cx - text_width(s) / 2, y, s, ink);
}

fn on_screen(im: &Img, x: i32, y: i32, w: i32, h: i32) -> bool {
    x + w > 0 && x < im.w && y + h > VIEW_Y - 24 && y < VIEW_Y + VIEW_H + 24
}

/// The ground: every tile in view.
pub(super) fn tiles(im: &mut Img, s: &Session, cam: Cam, tick: u32, shown: u32) {
    let up = crate::rules::moonfall::clock::bridge_up(shown, s.nums.day);
    let filled = s.world.has(DITCH_FILLED);
    let painted = bridge_painted();
    for ty in (cam.y / TILE).max(0)..=((cam.y + VIEW_H) / TILE).min(ROWS - 1) {
        for tx in (cam.x / TILE).max(0)..=((cam.x + 384) / TILE).min(COLS - 1) {
            let (sx, sy) = (tx * TILE - cam.x, ty * TILE - cam.y + VIEW_Y);
            tile(im, tx, ty, sx, sy, tick, up, filled, painted);
        }
    }
}

/// Whether the north bridge's pictures have all arrived: the leaf flat, rising
/// and upright. Then its tiles are the canal's water and the leaf is stamped
/// over them (`bridge`), whatever the hour.
fn bridge_painted() -> bool {
    ["down", "rising", "up"].iter().all(|state| bank::frame("bridge", state, 0).is_some())
}

/// The bridge's tiles on the map: its first and last column, and the row
/// below its last (where its hinge is).
fn bridge_tiles() -> (i32, i32, i32) {
    static AT: std::sync::OnceLock<(i32, i32, i32)> = std::sync::OnceLock::new();
    *AT.get_or_init(|| {
        let (mut first, mut last, mut below) = (COLS, 0, 0);
        for ty in 0..ROWS {
            for tx in 0..COLS {
                if town::tile(tx, ty) == Tile::Bridge {
                    (first, last, below) = (first.min(tx), last.max(tx), below.max(ty + 1));
                }
            }
        }
        (first, last, below)
    })
}

/// How many ticks the leaf takes to lift or to lower: its three middle
/// drawings, six ticks each.
const LIFT: u32 = 18;

/// The picture of the north bridge now. The rules raise it at 20:00 and lower
/// it at 06:00 (`clock::bridge_up`) and that is when it stands solid or open;
/// the leaf follows: upright or flat, and for the first eighteen ticks after
/// the hour it changes the three lifts, going up or coming down. Not at the
/// loop's first dawn, where it simply lies.
pub(super) fn bridge_leaf(s: &Session, tick: u32, shown: u32) -> Option<&'static Img> {
    use crate::rules::moonfall::clock;
    let day = u64::from(s.nums.day.max(1));
    let up = clock::bridge_up(shown, s.nums.day);
    // The tick the rules last changed it: the first at or after 20:00 (840
    // minutes after dawn) or 06:00 of the calendar day the clock is in.
    let minutes = u64::from(shown) * 1440 / day;
    let hour = minutes - minutes % 1440 + if up { 840 } else { 0 };
    let since = shown - ((hour * day).div_ceil(1440) as u32).min(shown);
    if shown > since && since < LIFT {
        return bank::frame("bridge", "rising", if up { since } else { LIFT - 1 - since });
    }
    bank::frame("bridge", if up { "up" } else { "down" }, tick)
}

fn speckle(im: &mut Img, tx: i32, ty: i32, sx: i32, sy: i32, ink: char, n: u32, seed: u32) {
    for i in 0..n {
        let h = hash(tx * 7 + i as i32, ty * 11 + 3, seed);
        im.put(sx + (h % 15) as i32, sy + ((h >> 8) % 15) as i32, ink);
    }
}

#[allow(clippy::too_many_arguments)]
fn tile(im: &mut Img, tx: i32, ty: i32, sx: i32, sy: i32, tick: u32, up: bool, filled: bool, painted_bridge: bool) {
    let t = match town::tile(tx, ty) {
        // The painted leaf lies over the canal.
        Tile::Bridge if painted_bridge => Tile::Water,
        t => t,
    };
    match t {
        Tile::Grass | Tile::Wall(_) => {
            im.rect(sx, sy, TILE, TILE, 'm');
            speckle(im, tx, ty, sx, sy, 'M', 5, 1);
            speckle(im, tx, ty, sx, sy, 'l', 4, 2);
            if hash(tx, ty, 9) % 9 == 0 {
                im.put(sx + 6, sy + 7, 'Y');
                im.put(sx + 7, sy + 7, 'Y');
            }
        }
        Tile::Cobble | Tile::Door => {
            im.rect(sx, sy, TILE, TILE, 'G');
            for row in 0..2 {
                let y = sy + row * 8;
                im.rect(sx, y, TILE, 1, 'j');
                let off = if row == 0 { 0 } else { 4 };
                for k in 0..2 {
                    im.rect(sx + off + k * 8, y, 1, 8, 'j');
                }
            }
            speckle(im, tx, ty, sx, sy, 'J', 3, 3);
            if t == Tile::Door {
                im.rect(sx + 3, sy + 4, 10, 8, 'B');
                im.frame(sx + 3, sy + 4, 10, 8, 'I');
            }
        }
        Tile::Water => {
            im.rect(sx, sy, TILE, TILE, 'q');
            let phase = (tick / 14 + hash(tx, ty, 4) % 4) % 4;
            for k in 0..2u32 {
                let h = hash(tx * 3 + k as i32, ty, 5);
                let (rx, ry) = ((h % 9) as i32, ((h >> 5) % 13) as i32);
                let w = 4 + phase as i32;
                im.rect(sx + rx, sy + ry, w, 1, 'Q');
            }
        }
        Tile::Bridge => {
            if up {
                im.rect(sx, sy, TILE, TILE, 'q');
                im.rect(sx, sy + TILE - 4, TILE, 4, 'o');
                im.rect(sx, sy, TILE, 1, 'B');
            } else {
                im.rect(sx, sy, TILE, TILE, 'o');
                for k in 0..4 {
                    im.rect(sx, sy + k * 4, TILE, 1, 'r');
                }
                im.rect(sx, sy, 2, TILE, 'B');
                im.rect(sx + TILE - 2, sy, 2, TILE, 'B');
            }
        }
        Tile::Plank => {
            im.rect(sx, sy, TILE, TILE, 'q');
            im.rect(sx + 3, sy, 10, TILE, 'P');
            for k in 0..4 {
                im.rect(sx + 3, sy + 1 + k * 4, 10, 1, 'r');
            }
            im.rect(sx + 3, sy, 1, TILE, 'B');
            im.rect(sx + 12, sy, 1, TILE, 'B');
        }
        Tile::Reeds => {
            im.rect(sx, sy, TILE, TILE, 'F');
            speckle(im, tx, ty, sx, sy, 'e', 5, 6);
            for k in 0..4 {
                let h = hash(tx * 5 + k, ty, 7);
                let (x, y) = ((h % 14) as i32, 2 + ((h >> 6) % 8) as i32);
                im.rect(sx + x, sy + y, 1, 6, 'l');
                im.put(sx + x, sy + y - 1, 'Y');
            }
        }
        Tile::Hedge => {
            im.rect(sx, sy, TILE, TILE, 'D');
            speckle(im, tx, ty, sx, sy, 'E', 22, 8);
            speckle(im, tx, ty, sx, sy, 'l', 8, 9);
        }
        Tile::Ditch => {
            if filled {
                im.rect(sx, sy, TILE, TILE, 'I');
                for k in 0..4 {
                    im.rect(sx, sy + k * 4, TILE, 1, 'b');
                }
                im.frame(sx, sy, TILE, TILE, 'B');
            } else {
                im.rect(sx, sy, TILE, TILE, 'n');
                im.rect(sx + 1, sy + 1, TILE - 2, TILE - 2, 'b');
                im.rect(sx + 3, sy + 4, TILE - 6, TILE - 8, 'n');
            }
        }
        Tile::Gate => {
            im.rect(sx, sy, TILE, TILE, 'G');
            for k in 0..4 {
                im.rect(sx + 1 + k * 4, sy, 2, TILE, 'P');
            }
            im.rect(sx, sy + 3, TILE, 2, 'B');
            im.rect(sx, sy + 11, TILE, 2, 'B');
        }
    }
}

struct Look {
    roof: char,
    ridge: char,
    wall: char,
    trim: char,
}

fn look(ch: char) -> Look {
    let (roof, ridge, wall, trim) = match ch {
        'S' => ('B', 'I', 'T', 'O'),
        'U' => ('O', 'o', 'P', 'B'),
        'A' => ('x', 's', 'U', 'V'),
        'B' => ('R', 'p', 'T', 'O'),
        'F' => ('g', 'K', 'J', 'h'),
        'H' => ('Q', 'q', 'V', 'W'),
        'I' => ('p', 'I', '$', 'O'),
        _ => ('P', 'B', 'c', 'T'),
    };
    Look { roof, ridge, wall, trim }
}

/// Whether it is dark: before seven in the morning or from five in the evening.
fn dark(s: &Session, shown: u32) -> bool {
    let h = crate::rules::moonfall::clock::stamp(shown, s.nums.day).hour;
    !(7..17).contains(&h)
}

/// Whether the window of a household is lit: they are up and it is dark.
fn lit_window(s: &Session, ch: char, shown: u32) -> bool {
    let h = crate::rules::moonfall::clock::stamp(shown, s.nums.day).hour;
    if !dark(s, shown) {
        return false;
    }
    let who = match ch {
        'A' => Some(Folk::Mira),
        'B' | 'F' => Some(Folk::Nessa),
        'H' => Some(Folk::Pella),
        'S' => Some(Folk::Orin),
        'U' => Some(Folk::Tovin),
        _ => None,
    };
    match who {
        // Awake and out, or at their door: a candle left in the window.
        Some(f) => {
            let fig = crate::rules::moonfall::folk::figure(f, shown, s.nums.day, s.world.shod);
            !(fig.arrived && !fig.visible) || (18..23).contains(&h)
        }
        None => (18..23).contains(&h),
    }
}

/// The slot of a building that has painted pictures to come, by its map letter.
/// The south tower (`Z`) has none yet: its painted door is on the south face
/// and the stage's door on the north.
fn painted_slot(ch: char) -> Option<&'static str> {
    match ch {
        'A' => Some("archive"),
        'B' => Some("bakery"),
        'F' => Some("forge"),
        'I' => Some("inn"),
        'H' => Some("herald-house"),
        'S' => Some("scriptorium"),
        'C' => Some("cottage"),
        'U' => Some("bridge-hut"),
        'N' => Some("tower-north"),
        _ => None,
    }
}

/// The picture of a building that stands now: its windows lit when the
/// household is up in the dark (the same rule the block's windows follow), the
/// night's when it is dark and nobody is up, the day's otherwise. A tower has
/// no household: its picture is the lit one (the door ajar) when its door is
/// unlocked, as the block's slits were. Only when all three are painted: a
/// building never changes style with the hour.
fn painted_house(s: &Session, ch: char, tick: u32, shown: u32) -> Option<&'static Img> {
    let id = painted_slot(ch)?;
    if ["day", "night", "lit"].iter().any(|state| bank::frame(id, state, 0).is_none()) {
        return None;
    }
    let up = if ch == 'N' { s.world.has(crate::rules::moonfall::state::NORTH_OPEN) } else { lit_window(s, ch, shown) };
    let state = if up {
        "lit"
    } else if dark(s, shown) {
        "night"
    } else {
        "day"
    };
    bank::frame(id, state, tick)
}

/// Where a painted building's top-left corner goes: its bottom on the south
/// edge of the footprint, its door over the doorstep tile the map keeps under
/// it (else its middle on the footprint's middle).
fn painted_corner(hs: &town::House, pic: &Img, x: i32, y: i32) -> (i32, i32) {
    let row = hs.y + hs.h;
    let door = (hs.x..hs.x + hs.w)
        .find(|&c| town::tile(c, row) == Tile::Door)
        .map(|c| (c - hs.x) * TILE + TILE / 2);
    (x + door.unwrap_or(hs.w * TILE / 2) - pic.w / 2, y + hs.h * TILE - pic.h)
}

/// Where the north tower's flame stands in its painted picture, from the
/// picture's bottom middle: the mouth of the basket on the roof.
const TOWER_FLAME: (i32, i32) = (1, -89);
/// The lock's gem on the painted door, from the same point.
const TOWER_GEM: (i32, i32) = (3, -17);

/// What the block tower drew on top of itself and the painted one keeps: the
/// flame in its basket (or the faint glow that asks for it) and the door's
/// lock, gold when locked and aqua when open.
fn painted_tower(im: &mut Img, s: &Session, foot: (i32, i32), tick: u32) {
    flame(im, foot.0 + TOWER_FLAME.0, foot.1 + TOWER_FLAME.1, s.flame(0), tick, s.tower_ready(0), 1);
    let unlocked = s.world.has(crate::rules::moonfall::state::NORTH_OPEN);
    im.rect(foot.0 + TOWER_GEM.0, foot.1 + TOWER_GEM.1, 2, 2, if unlocked { '2' } else { '6' });
}

pub(super) fn houses(im: &mut Img, labels: &mut Layout, s: &Session, cam: Cam, tick: u32, shown: u32) {
    for h in &HOUSES {
        let (x, y) = (h.x * TILE - cam.x, h.y * TILE - cam.y + VIEW_Y);
        let (w, hh) = (h.w * TILE, h.h * TILE);
        let pic = painted_house(s, h.ch, tick, shown);
        // A painted house may stand much taller than its footprint.
        let up = pic.map_or(20, |p| (p.h - hh).max(0) + 20);
        if !on_screen(im, x, y - up, w, hh + up) {
            continue;
        }
        // The plate's middle and its row: over the footprint for a block, over
        // the painted picture (which may stand taller) for a painted house.
        let (mut plate_x, mut plate_y) = (x + w / 2, y - 11);
        // The picture or footprint the plate names.
        let mut anchor = Rect::new(x, y, w, hh);
        match pic {
            None if matches!(h.ch, 'N' | 'Z') => tower(im, s, h.ch, x, y, w, hh, tick),
            None => house(im, s, h, cam, shown),
            Some(pic) => {
                let (px, py) = painted_corner(h, pic, x, y);
                im.stamp(pic, px, py);
                if h.ch == 'N' {
                    painted_tower(im, s, (px + pic.w / 2, py + pic.h), tick);
                }
                let top = (0..pic.h).find(|&row| (0..pic.w).any(|col| pic.get(col, row).is_some()));
                let above = (py + top.unwrap_or(0)).min(y) - 11;
                // Over the picture, even up in the sky band; a picture that
                // stands off the top of the screen wears its plate under its
                // footprint instead of over its roof.
                (plate_x, plate_y) = (px + pic.w / 2, if above >= 1 { above } else { y + hh + 2 });
                let top = top.unwrap_or(0);
                anchor = Rect::new(px, py + top, pic.w, pic.h - top);
            }
        }
        if !h.name.is_empty() {
            labels.plate(plate_x, if pic.is_some() { plate_y } else { plate_y.max(VIEW_Y + 1) }, h.name, 'T', anchor);
        }
    }
}

fn house(im: &mut Img, s: &Session, hs: &town::House, cam: Cam, shown: u32) {
    let (ch, x, y) = (hs.ch, hs.x * TILE - cam.x, hs.y * TILE - cam.y + VIEW_Y);
    let (w, h) = (hs.w * TILE, hs.h * TILE);
    let l = look(ch);
    // A shadow on the ground to the south.
    for k in 0..3 {
        im.rect(x + 2, y + h + k, w, 1, if k == 0 { 'f' } else { 'D' });
    }
    // The south wall, one tile tall, with its windows and door.
    im.rect(x, y + h - TILE, w, TILE, l.wall);
    im.rect(x, y + h - 2, w, 2, l.trim);
    let lit = lit_window(s, ch, shown);
    let mut wx = x + 6;
    while wx + 6 <= x + w - 4 {
        im.rect(wx, y + h - 12, 5, 6, if lit { '5' } else { 's' });
        im.frame(wx - 1, y + h - 13, 7, 8, 'k');
        wx += 14;
    }
    // The roof.
    im.rect(x, y, w, h - TILE, l.roof);
    for k in (2..h - TILE).step_by(4) {
        im.rect(x, y + k, w, 1, l.ridge);
    }
    im.rect(x, y, w, 1, l.ridge);
    im.rect(x, y + (h - TILE) / 2 - 1, w, 2, l.ridge);
    im.rect(x, y, 1, h - TILE, l.ridge);
    im.rect(x + w - 1, y, 1, h - TILE, l.ridge);
    // A chimney.
    if ch != 'U' {
        im.rect(x + w - 12, y + 3, 5, 6, 'g');
    }
    // A door over its step: the door tile south of the wall.
    let row = hs.y + hs.h;
    if let Some(c) = (hs.x..hs.x + hs.w).find(|&c| town::tile(c, row) == Tile::Door) {
        let dx = c * TILE - cam.x + 4;
        im.rect(dx, y + h - 13, 8, 12, 'b');
        im.frame(dx, y + h - 13, 8, 12, 'k');
        im.put(dx + 6, y + h - 7, '4');
    }
}

#[allow(clippy::too_many_arguments)]
fn tower(im: &mut Img, s: &Session, ch: char, x: i32, y: i32, w: i32, h: i32, tick: u32) {
    // The south tower's door faces north, over the ditch; the north one's south.
    let up = ch == 'Z';
    im.rect(x, y + h, w, 3, 'D');
    im.rect(x, y, w, h, 'u');
    im.rect(x, y, w, 3, 'v');
    im.rect(x + w - 6, y + 3, 6, h - 3, 'S');
    // Stones.
    for row in (6..h).step_by(8) {
        im.rect(x, y + row, w, 1, 'S');
        for col in (0..w).step_by(10) {
            im.rect(x + col + (row / 8 % 2) * 5, y + row, 1, 8, 'S');
        }
    }
    // Battlements.
    for col in (0..w).step_by(8) {
        im.rect(x + col, y - 4, 5, 4, 'v');
        im.rect(x + col, y - 4, 5, 1, 'W');
    }
    // The flame bowl on the roof.
    let (bx, by) = (x + w / 2, y + h / 2 - 6);
    im.ellipse(bx as f32, by as f32, 9.0, 5.0, 'g');
    im.ellipse(bx as f32, by as f32 - 1.0, 7.0, 3.5, 'j');
    let i = if ch == 'N' { 0 } else { 1 };
    flame(im, bx, by - 2, s.flame(i), tick, s.tower_ready(i), 1);
    // The door.
    let dy = if up { y + 1 } else { y + h - 14 };
    im.rect(x + w / 2 - 5, dy, 10, 14, 'b');
    im.frame(x + w / 2 - 5, dy, 10, 14, 'k');
    let unlocked = if ch == 'N' {
        s.world.has(crate::rules::moonfall::state::NORTH_OPEN)
    } else {
        s.world.has(crate::rules::moonfall::state::SOUTH_OPEN)
    };
    im.put(x + w / 2 + 3, dy + 7, if unlocked { '2' } else { '6' });
    let slit = if unlocked { '2' } else { 's' };
    for col in [w / 4, w * 3 / 4] {
        im.rect(x + col - 1, y + h / 2 - 4, 2, 6, slit);
    }
}

/// The painted Moonwell stands on the south edge of the two by two solid tiles
/// the rules keep under the well: half their height below the well's middle.
const WELL_FOOT: i32 = 16;
/// The cold bowl in that picture, from the foot, centre of its mouth.
const WELL_BOWL: (i32, i32) = (-11, -29);

/// A flame: a lit one dances; a ready bowl glows faint to say "now".
pub(super) fn flame(im: &mut Img, x: i32, y: i32, lit: bool, tick: u32, ready: bool, scale: i32) {
    if lit {
        let sway = ((tick / 4) % 3) as i32 - 1;
        im.ellipse(x as f32, y as f32 - 4.0 * scale as f32, 5.0 * scale as f32, 8.0 * scale as f32, '@');
        im.ellipse((x + sway) as f32, y as f32 - 5.0 * scale as f32, 3.5 * scale as f32, 6.0 * scale as f32, '5');
        im.ellipse(x as f32, y as f32 - 4.0 * scale as f32, 1.8 * scale as f32, 3.5 * scale as f32, 'w');
    } else if ready && tick / 10 % 2 == 0 {
        im.ellipse(x as f32, y as f32 - 1.0, 3.0, 2.0, '4');
    }
}

/// Lamp posts, the crate, the well, the bell, the podium, the pickups.
pub(super) fn props(im: &mut Img, labels: &mut Layout, s: &Session, cam: Cam, tick: u32, shown: u32) {
    let to = |px: i32, py: i32| (px - cam.x, py - cam.y + VIEW_Y);
    // The north bridge: its leaf stands on the south edge of its tiles, its
    // hinge, over the canal `tile` drew.
    if bridge_painted() {
        let (first, last, below) = bridge_tiles();
        let (bx, by) = to((first + last + 1) * TILE / 2, below * TILE);
        if on_screen(im, bx - 28, by - 36, 56, 36) {
            if let Some(pic) = bridge_leaf(s, tick, shown) {
                im.stamp(pic, bx - pic.w / 2, by - pic.h);
            }
        }
    }
    // The Moonwell: painted when its frames have arrived (bottom centre on the
    // south edge of its solid tiles), else its ring and water in code.
    let (wx, wy) = to(town::WELL.0, town::WELL.1);
    if on_screen(im, wx - 24, wy - 32, 48, 64) {
        let state = if s.flame(2) {
            "lit"
        } else if s.memory.has_keep(crate::rules::moonfall::ledger::OIL) {
            "oiled"
        } else {
            "idle"
        };
        if let Some(pic) = bank::frame("moonwell", state, tick) {
            let foot = wy + WELL_FOOT;
            im.stamp(pic, wx - pic.w / 2, foot - pic.h);
            // The cold bowl glows faint to say "now", as it does in code.
            if !s.flame(2) && s.tower_ready(2) && tick / 10 % 2 == 0 {
                im.ellipse((wx + WELL_BOWL.0) as f32, (foot + WELL_BOWL.1) as f32, 3.0, 2.0, '4');
            }
        } else {
            im.ellipse(wx as f32, wy as f32, 17.0, 14.0, 'J');
            im.ellipse(wx as f32, wy as f32 - 1.0, 15.0, 12.0, 'G');
            im.ellipse(wx as f32, wy as f32 - 1.0, 11.0, 8.0, if s.flame(2) { '@' } else { 'q' });
            if s.flame(2) {
                flame(im, wx, wy, true, tick, true, 1);
            } else {
                im.rect(wx - 6, wy - 3, 4, 1, 'Q');
                if s.memory.has_keep(crate::rules::moonfall::ledger::OIL) {
                    im.ellipse(wx as f32, wy as f32 - 1.0, 8.0, 5.0, 'o');
                    if tick / 12 % 2 == 0 {
                        im.put(wx + 2, wy - 2, '4');
                    }
                    flame(im, wx, wy, false, tick, s.tower_ready(2), 1);
                } else if s.tower_ready(2) {
                    flame(im, wx, wy, false, tick, true, 1);
                }
            }
            labels.plate(wx, wy + 16, "MOONWELL", 'V', Rect::new(wx - 17, wy - 14, 34, 28));
        }
    }
    // The Bell post, west of the well.
    let (bx, by) = to(town::BELL_POST.0, town::BELL_POST.1);
    if on_screen(im, bx - 12, by - 38, 24, 48) {
        // Painted: still on its post until the ritual, then it rings, a damped
        // swing from the first toll; the block swings a few pixels all through it.
        let ritual = match s.phase {
            crate::rules::moonfall::state::Phase::Rewind { t, .. } => Some(t),
            _ => None,
        };
        let pic = if bank::frame("bell", "idle", 0).is_some() && bank::frame("bell", "ring", 0).is_some() {
            match ritual {
                Some(t) => bank::frame("bell", "ring", t),
                None => bank::frame("bell", "idle", tick),
            }
        } else {
            None
        };
        if let Some(pic) = pic {
            // The foot is where the block's posts end.
            im.stamp(pic, bx - pic.w / 2, by + 2 - pic.h);
        } else {
            im.rect(bx - 10, by - 22, 2, 24, 'P');
            im.rect(bx + 8, by - 22, 2, 24, 'P');
            im.rect(bx - 10, by - 22, 20, 2, 'P');
            let swing = if ritual.is_some() { ((tick / 2) % 4) as i32 - 2 } else { 0 };
            im.rect(bx - 4 + swing, by - 20, 8, 3, 'o');
            im.rect(bx - 6 + swing, by - 17, 12, 9, 'O');
            im.rect(bx - 7 + swing, by - 9, 14, 2, 't');
            im.put(bx + swing, by - 6, 'k');
        }
        // The bell's own picture: the banner of the ritual keeps off it.
        let art = Rect::new(bx - 12, by - 36, 24, 38);
        labels.art(art);
        labels.plate(bx, by + 6, "BELL", 'T', art);
    }
    // Street lanterns: painted when unlit, lit and snuffing have arrived, else
    // the block. A lit one flickers through its three flames (each a little
    // out of step with its neighbours); a wisp leaning into it plays the
    // snuffing drawings over the last eighteen ticks before it goes out.
    let lamps = ["unlit", "lit", "snuffing"].iter().all(|state| bank::frame("lantern", state, 0).is_some());
    for (n, &(tx, ty)) in LAMPS.iter().enumerate() {
        let (lx, ly) = to(tx * TILE + 8, ty * TILE + 14);
        if !on_screen(im, lx - 8, ly - 34, 16, 40) {
            continue;
        }
        if lamps {
            let pic = if !s.lantern_lit(n as u8) {
                bank::frame("lantern", "unlit", 0)
            } else if let Some(left) = s.world.wisps.iter().find_map(|w| match w.mode {
                WispMode::Snuff(t) if usize::from(w.aim) == n && t <= 18 => Some(t),
                _ => None,
            }) {
                bank::frame("lantern", "snuffing", 18 - left)
            } else {
                bank::frame("lantern", "lit", tick + 5 * n as u32)
            };
            if let Some(pic) = pic {
                // Its foot where the block's post ends.
                im.stamp(pic, lx - pic.w / 2, ly - pic.h);
            }
            continue;
        }
        im.rect(lx - 1, ly - 18, 2, 18, 'g');
        im.rect(lx - 3, ly - 2, 6, 2, 'j');
        im.rect(lx - 4, ly - 26, 8, 9, 'J');
        im.rect(lx - 3, ly - 25, 6, 7, 'g');
        let lit = s.lantern_lit(n as u8);
        let _ = shown;
        if lit {
            let flick = (tick / 5 + n as u32) % 3 == 0;
            im.rect(lx - 3, ly - 25, 6, 7, '@');
            im.rect(lx - 2, ly - 24, 4, 5, if flick { '5' } else { '6' });
            im.put(lx, ly - 22, 'w');
        } else if s.lantern_scheduled(n as u8) {
            im.rect(lx - 3, ly - 25, 6, 7, 's');
        }
        im.rect(lx - 5, ly - 28, 10, 2, 'g');
    }
    // The ink pot and the reed stack.
    let (ix, iy) = to(town::INK_POT.0, town::INK_POT.1);
    if !s.world.has(crate::rules::moonfall::state::INK_TAKEN) && on_screen(im, ix - 8, iy - 8, 16, 16) {
        let known = s.memory.knows(crate::rules::moonfall::ledger::Fact::OrinLost);
        im.rect(ix - 3, iy - 2, 6, 5, 'k');
        im.rect(ix - 2, iy - 4, 4, 2, 'x');
        if known && tick / 12 % 2 == 0 {
            im.put(ix + 3, iy - 5, '6');
        }
    }
    let (rx, ry) = to(town::REED_STACK.0, town::REED_STACK.1);
    if on_screen(im, rx - 10, ry - 10, 20, 20) {
        for k in -2..=2 {
            im.rect(rx + k * 2, ry - 6 + k.abs(), 2, 9, 'Y');
            im.rect(rx + k * 2, ry - 6 + k.abs(), 2, 2, 'T');
        }
        im.rect(rx - 6, ry + 1, 12, 2, 'P');
    }
    // The crate.
    for &(cx, cy) in &s.world.crates {
        let (x, y) = to(cx * TILE, cy * TILE);
        if on_screen(im, x, y, TILE, TILE) {
            im.rect(x + 1, y + 1, TILE - 2, TILE - 2, 'O');
            im.frame(x + 1, y + 1, TILE - 2, TILE - 2, 'B');
            im.line(x + 2, y + 2, x + TILE - 3, y + TILE - 3, 'r');
            im.line(x + TILE - 3, y + 2, x + 2, y + TILE - 3, 'r');
            labels.plate(x + TILE / 2, y - 10, "CRATE", 'O', Rect::new(x, y, TILE, TILE));
        }
    }
    // The herald's podium: painted when idle and cry have arrived (the pennant
    // hangs limp, and flaps while the herald cries), else the block.
    let (px, py) = to(town::Spot::Podium.px().0, town::Spot::Podium.px().1 + 4);
    if on_screen(im, px - 14, py - 24, 28, 36) {
        let painted = bank::frame("podium", "idle", 0).is_some() && bank::frame("podium", "cry", 0).is_some();
        let state = if s.cry_active(Folk::Pella) { "cry" } else { "idle" };
        if let Some(pic) = bank::frame("podium", state, tick).filter(|_| painted) {
            // Its bottom on the block's bottom edge.
            im.stamp(pic, px - pic.w / 2, py + 4 - pic.h);
        } else {
            im.rect(px - 9, py - 6, 18, 10, 'o');
            im.rect(px - 9, py - 6, 18, 2, 'T');
            im.rect(px - 9, py + 2, 18, 2, 'B');
        }
        labels.plate(px, py + 7, "PODIUM", 't', Rect::new(px - 13, py - 22, 26, 26));
    }
    // Gate posts and the road beyond.
    let (gx, gy) = to(35 * TILE, 14 * TILE);
    if on_screen(im, gx, gy - 12, 16, 60) {
        labels.plate(gx - 8 + text_width("EAST GATE") / 2, gy - 12, "EAST GATE", 'T', Rect::new(gx, gy, 16, 48));
    }
}

pub(super) fn seat_ink(seat: u32) -> char {
    ['2', '@', 'C', '7', 'z', 'Y'][(seat.max(1) as usize - 1) % 6]
}

/// Everyone standing in the town, back to front.
pub(super) fn actors(im: &mut Img, labels: &mut Layout, s: &Session, cam: Cam, tick: u32, shown: u32, me: u32) {
    enum A<'a> {
        Folk(crate::rules::moonfall::folk::Figure),
        Knight(u32, &'a Knight),
        Wisp(usize),
    }
    let mut all: Vec<(i32, A)> = Vec::new();
    for f in CAST {
        let fig = crate::rules::moonfall::folk::figure(f, shown, s.nums.day, s.world.shod);
        if fig.visible {
            all.push((fig.y, A::Folk(fig)));
        }
    }
    for (&id, seat) in &s.seats {
        all.push((seat.knight.px().1 + if id == me { 1 } else { 0 }, A::Knight(id, &seat.knight)));
    }
    for (i, w) in s.world.wisps.iter().enumerate() {
        all.push((w.y / town::Q + 6, A::Wisp(i)));
    }
    all.sort_by_key(|(y, _)| *y);
    for (_, a) in all {
        match a {
            A::Folk(fig) => folk_block(im, labels, s, &fig, cam, tick),
            A::Knight(id, k) => knight_block(im, labels, id, k, cam, tick, id == me),
            A::Wisp(i) => wisp_block(im, s, i, cam, tick),
        }
    }
}

fn folk_block(im: &mut Img, labels: &mut Layout, s: &Session, fig: &crate::rules::moonfall::folk::Figure, cam: Cam, tick: u32) {
    let (x, y) = (fig.x - cam.x, fig.y - cam.y + VIEW_Y);
    if !on_screen(im, x - 14, y - 30, 28, 34) {
        return;
    }
    let ink = fig.folk.ink();
    if fig.folk == Folk::Aster {
        // A horse, tired when unshod, and its rider.
        let lame = s.world.shod.is_none();
        let bob = if fig.moving { ((tick / 6) % 2) as i32 } else { 0 };
        im.rect(x - 13, y - 12 + bob, 26, 9, 'o');
        im.rect(x - 14, y - 15 + bob, 6, 6, 'o');
        im.rect(x - 15, y - 17 + bob, 2, 3, 'B');
        for lx in [-11, -6, 6, 11] {
            im.rect(x + lx, y - 4, 2, if lame && lx == 11 { 2 } else { 5 }, 'B');
        }
        im.rect(x - 4, y - 24 + bob, 8, 12, ink);
        im.rect(x - 3, y - 29 + bob, 6, 5, 'c');
        im.rect(x + 6, y - 30 + bob, 1, 14, 'P');
        im.rect(x + 5, y - 33 + bob, 3, 3, '@');
        let body = Rect::new(x - 15, y - 34, 29, 34);
        labels.body(body);
        labels.tag(Kind::Folk, fig.folk.index() as u32, x, y - 42, "ASTER VALE", ink, body);
        return;
    }
    let step = if fig.moving { ((tick / 6) % 2) as i32 } else { 0 };
    // Feet, body, head, with a mark of facing.
    im.rect(x - 4, y - 2, 3, 2 + step, 'k');
    im.rect(x + 1, y - 2, 3, 2 + (1 - step), 'k');
    im.rect(x - 5, y - 14, 10, 12, ink);
    im.frame(x - 5, y - 14, 10, 12, 'k');
    im.rect(x - 3, y - 21, 6, 6, 'c');
    im.frame(x - 3, y - 21, 6, 6, 'k');
    match fig.face {
        1 => {}
        2 => im.put(x - 2, y - 19, 'k'),
        3 => im.put(x + 1, y - 19, 'k'),
        _ => {
            im.put(x - 2, y - 19, 'k');
            im.put(x + 1, y - 19, 'k');
        }
    }
    let body = Rect::new(x - 6, y - 22, 12, 22);
    labels.body(body);
    labels.tag(Kind::Folk, fig.folk.index() as u32, x, y - 31, fig.folk.short(), ink, body);
    // A little tell of what they do for a living.
    match fig.folk {
        Folk::Mira => im.rect(x + 6, y - 16, 1, 12, '6'),
        Folk::Pella => im.rect(x + 5, y - 16, 4, 3, '6'),
        Folk::Tovin => im.rect(x + 6, y - 22, 1, 20, 'P'),
        Folk::Nessa => im.rect(x - 6, y - 8, 12, 2, 'T'),
        Folk::Orin => im.rect(x + 5, y - 14, 4, 5, 'W'),
        Folk::Aster => {}
    }
}

fn knight_block(im: &mut Img, labels: &mut Layout, id: u32, k: &Knight, cam: Cam, tick: u32, mine: bool) {
    let (px, py) = k.px();
    let (x, y) = (px - cam.x, py - cam.y + VIEW_Y);
    if !on_screen(im, x - 14, y - 30, 28, 34) {
        return;
    }
    let ink = seat_ink(id);
    // The viewer's own number keeps its place first, then the lowest seats.
    let prio = if mine { 0 } else { id };
    if k.down > 0 {
        im.rect(x - 8, y - 5, 16, 6, ink);
        im.frame(x - 8, y - 5, 16, 6, 'k');
        let body = Rect::new(x - 8, y - 5, 16, 6);
        labels.body(body);
        labels.tag(Kind::Knight, prio, x, y - 15, "OUT", 'h', body);
        return;
    }
    // A hurt knight blinks.
    if k.hurt > 0 && (tick / 3) % 2 == 0 && k.hurt < 80 {
        return;
    }
    let rolling = k.roll > 0;
    let stride = if k.stride > 0 { ((k.stride / 5) % 2) as i32 } else { 0 };
    let lift = if rolling { 5 } else { 0 };
    // A shadow.
    im.rect(x - 5, y - 1, 10, 2, 'f');
    im.rect(x - 4, y - 3 - lift / 2, 3, 3 + stride, 'k');
    im.rect(x + 1, y - 3 - lift / 2, 3, 3 + (1 - stride), 'k');
    let body = y - 15 - lift / 2;
    im.rect(x - 5, body, 10, 12, ink);
    im.frame(x - 5, body, 10, 12, 'k');
    // The mask changes the helm.
    let helm = match k.mask {
        Mask::Bare => 'h',
        Mask::Swift => 'z',
        Mask::Stout => 'R',
    };
    im.rect(x - 4, body - 7, 8, 7, helm);
    im.frame(x - 4, body - 7, 8, 7, 'k');
    match k.mask {
        Mask::Swift => {
            im.rect(x - 7, body - 5, 3, 1, 'W');
            im.rect(x + 4, body - 5, 3, 1, 'W');
        }
        Mask::Stout => im.rect(x - 5, body - 3, 10, 2, 'B'),
        Mask::Bare => {}
    }
    // Eyes where the knight faces.
    match k.face {
        1 => {}
        2 => im.put(x - 2, body - 4, 'k'),
        3 => im.put(x + 1, body - 4, 'k'),
        _ => {
            im.put(x - 2, body - 4, 'k');
            im.put(x + 1, body - 4, 'k');
        }
    }
    // What is in the hands.
    if k.held != Item::Empty {
        im.rect(x - 3, body - 12, 6, 5, k.held.ink());
        im.frame(x - 3, body - 12, 6, 5, 'k');
    }
    if mine {
        im.put(x, body - 15 - (tick / 10 % 2) as i32, 'H');
    }
    let top = body - if k.held != Item::Empty { 12 } else { 7 };
    let shape = Rect::new(x - 7, top, 14, y - top);
    labels.body(shape);
    labels.tag(Kind::Knight, prio, x, body - if k.held != Item::Empty { 22 } else { 17 }, &format!("{id}"), ink, shape);
}

fn wisp_block(im: &mut Img, s: &Session, i: usize, cam: Cam, tick: u32) {
    let w = &s.world.wisps[i];
    let (x, y) = (w.x / town::Q - cam.x, w.y / town::Q - cam.y + VIEW_Y);
    if !on_screen(im, x - 10, y - 20, 20, 30) {
        return;
    }
    // Where it is going: a few faint dots along the way, so it can be read
    // and stepped around.
    if w.mode == WispMode::Hunt {
        if let Some(&(_, at)) = s.lights().iter().find(|&&(id, _)| id == w.aim) {
            let (tx, ty) = (at.0 - cam.x, at.1 - cam.y + VIEW_Y);
            for k in 1..=4 {
                let (dx, dy) = ((tx - x) * k / 12, (ty - y) * k / 12);
                if dx.abs() + dy.abs() < 64 {
                    im.put(x + dx, y - 8 + dy, '3');
                }
            }
        }
    }
    let bob = ((tick / 6 + i as u32) % 4) as i32 - 1;
    let y = y - 8 + bob;
    // A pale hood over a wick-stub tail, two eyes.
    im.ellipse(x as f32, y as f32, 6.0, 7.0, '3');
    im.ellipse(x as f32, y as f32 + 1.0, 5.0, 6.0, 'W');
    let tail = (tick / 4 + i as u32) % 3;
    im.rect(x - 1, y + 6, 2, 3 + tail as i32, 'u');
    im.rect(x - 3, y - 1, 2, 3, 's');
    im.rect(x + 1, y - 1, 2, 3, 's');
    im.put(x - 2, y, '2');
    im.put(x + 2, y, '2');
    match w.mode {
        WispMode::Snuff(t) => {
            // It leans into the light and pulses.
            if t / 3 % 2 == 0 {
                im.frame(x - 8, y - 8, 17, 17, '3');
            }
        }
        WispMode::Shy(_) => {
            im.rect(x - 3, y - 5, 6, 1, 'h');
        }
        WispMode::Hunt => {}
    }
}

/// Speech over the villagers who are crying something.
pub(super) fn cries(labels: &mut Layout, s: &Session, cam: Cam, shown: u32) {
    let Some((who, topic, _)) = s.cry else { return };
    let fig = crate::rules::moonfall::folk::figure(who, shown, s.nums.day, s.world.shod);
    if !fig.visible {
        return;
    }
    let lines = s.words(who, topic);
    let (x, y) = (fig.x - cam.x, fig.y - cam.y + VIEW_Y - 40);
    let parts = lines
        .iter()
        .take(3)
        .enumerate()
        .map(|(n, line)| {
            let width = text_width(line);
            let lx = (x - width / 2).clamp(2, 384 - width - 2);
            Part::plate(line, 'T', lx, y - 10 * (lines.len().min(3) - n) as i32)
        })
        .collect();
    labels.speech(parts, Rect::new(x - 6, y + 18, 12, 22));
}
