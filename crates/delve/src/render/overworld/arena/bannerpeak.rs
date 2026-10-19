//! Bannerpeak's stage: labelled block placeholders until the art wave draws
//! the real thing (docs/ROUNDTABLE-ADVENTURES.md, assets/bannerpeak/ART.md).
//! `layers` is what a seat sees (384 by 224, base and fx), `terminal_layers`
//! what the terminal's cell painter draws, `kingdom` the door on the walking
//! map. Every actor is a block in its own function named for its art slot, so
//! a painted frame can take its place without touching the sim: art never sets
//! a hitbox, the boxes come from the rules.
use super::super::ink::{Img, text_width};
use super::roundtable::{self as shell, Look};
use crate::rules::bannerpeak::{
    self as rules,
    body::{Pose, Q},
    boss::{self, Beat as BossBeat},
    foes::{BOULDER, CapState, FoeKind, ItemKind},
    knight::{Knight, Size},
    layout::{self, cell_centre, info, section_of, world},
    session::{BANNER_NAMES, BOARD, FX_LIFE, FxKind, Phase, SUMMIT_BANNERS, Session, View},
    world::{Grid, TILE, Tile},
};
use crate::rules::{HZ, Run, classes};

mod bank;
mod sky;

const LOOK: Look = Look {
    name: rules::NAME,
    backdrop: 'g',
    edge: 'v',
    accent: 'O',
};

const W: i32 = 384;
const H: i32 = 224;

/// A seat's own colour: bright inks apart from each other on the dark ground.
fn seat_ink(seat: u32) -> char {
    ['z', 'O', 'y', 'T', 'V', 'C'][(seat.max(1) as usize - 1) % 6]
}
/// A banner's colour on the pole and in the strip.
fn banner_ink(n: usize) -> char {
    ['z', 'R', 'y', 'O', 'V', 'T', '3', '6'][n % 8]
}

fn text(im: &mut Img, x: i32, y: i32, s: &str, c: char) {
    im.rect(x - 1, y - 1, text_width(s) + 2, 9, 'k');
    im.text(x, y, s, c);
}
fn text_centre(im: &mut Img, cx: i32, y: i32, s: &str, c: char) {
    text(im, cx - text_width(s) / 2, y, s, c);
}
/// `s` at `k` times the font's size, centred on `cx`.
fn large(im: &mut Img, cx: i32, y: i32, s: &str, c: char, k: i32) {
    let mut words = Img::new(text_width(s), 7);
    words.text(0, 0, s, c);
    let x = cx - words.w * k / 2;
    im.rect(x - 2, y - 2, words.w * k + 4, 7 * k + 4, 'k');
    im.stamp_times(&words, x, y, k);
}

/// A small integer hash, for stars and sparkle that must not move.
fn mix(a: i32, b: i32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 13)
}
/// A triangle wave of period `p`, 0 to `p / 2`.
fn tri(x: i32, p: i32) -> i32 {
    let k = x.rem_euclid(p);
    if k < p / 2 { k } else { p - k }
}

pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    layers_with_words(run, focus, true)
}

/// The picture with or without its pixel words: the terminal paints its own
/// text rows over the cells, where a 5 by 7 font cannot be read.
fn layers_with_words(run: &Run, focus: Option<u32>, words: bool) -> (Img, Img) {
    let Some(s) = run.bannerpeak.as_ref() else {
        return shell::stub_layers(run, focus, &LOOK);
    };
    let id = focus
        .filter(|id| s.seats.contains_key(id))
        .or_else(|| s.seats.keys().next().copied())
        .unwrap_or(1);
    let me = s.seats.get(&id);
    let cam = me.map_or((0, 0), |k| k.cam);
    let mut im = Img::black(W, H);
    let mut fx = Img::new(W, H);
    let tick = s.elapsed;
    // The painted sky when it is there, the code's mountains until then.
    if !sky::paint(&mut im, cam) {
        scenery(&mut im, cam);
    }
    tiles(&mut im, s, cam, tick);
    props(&mut im, s, cam, tick);
    things(&mut im, s, cam, tick);
    // Knights: the viewer's on top.
    let mut order: Vec<_> = s.seats.iter().collect();
    order.sort_by_key(|(seat, _)| **seat == id);
    for (&seat, k) in order {
        knight(&mut im, run, s, seat, k, cam, seat == id, tick);
    }
    effects(&mut fx, s, cam, tick);
    hud(&mut im, run, s, id, tick);
    overlays(&mut im, s, me, id, tick, words);
    (im, fx)
}

pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    layers_with_words(run, focus, false)
}

// ------------------------------------------------------------ scenery

fn tint(x_tile: i32) -> (char, char, char) {
    // Far peak, mid hill, near ground-mist, by section.
    match section_of(x_tile) {
        0 => ('x', 's', 'K'),
        1 | 2 => ('x', 'D', 'f'),
        3 => ('s', 'K', 'K'),
        4 => ('u', 'S', 'x'),
        _ => ('X', 'g', 'K'),
    }
}

fn scenery(im: &mut Img, cam: (i32, i32)) {
    for y in 0..H {
        let wy = cam.1 + y;
        let ch = if wy < 200 {
            'K'
        } else if wy < 360 {
            's'
        } else if wy < 520 {
            'x'
        } else if wy < 680 {
            'S'
        } else {
            'u'
        };
        im.rect(0, y, W, 1, ch);
    }
    // Stars, high up only.
    for n in 0..60 {
        let wx = (mix(n, 1) % 1400) as i32;
        let wy = (mix(n, 2) % 300) as i32;
        let (x, y) = (wx - cam.0 / 6, wy - cam.1 / 6);
        if (0..W).contains(&x) && (0..H).contains(&y) && cam.1 < 500 {
            im.put(x, y, if n % 5 == 0 { 'h' } else { 'j' });
        }
    }
    let (far, mid, _) = tint((cam.0 + 192) / TILE);
    // Far peaks: slow, tall.
    for x in 0..W {
        let wx = x + cam.0 / 5;
        let peak = 36 + tri(wx, 210) / 2 + tri(wx * 2 + 37, 74) / 3;
        let top = 176 - peak - cam.1 / 8;
        if top < H {
            im.rect(x, top.max(0), 1, H - top.max(0), far);
        }
    }
    // Hills: closer and lower.
    for x in 0..W {
        let wx = x + cam.0 / 2;
        let hill = 14 + tri(wx, 150) / 3 + tri(wx * 3 + 11, 46) / 4;
        let top = 196 - hill - cam.1 / 4;
        if top < H {
            im.rect(x, top.max(0), 1, H - top.max(0), mid);
        }
    }
}

// -------------------------------------------------------------- tiles

/// Ticks since the block at `(tx, ty)` was struck from below, while its bump
/// is on show.
fn bump_age(s: &Session, tx: i32, ty: i32) -> Option<i32> {
    let (cx, top) = (tx * TILE + TILE / 2, ty * TILE);
    s.fx
        .iter()
        .find(|f| f.k == FxKind::Bump && f.x == cx && f.y == top)
        .map(|f| (FX_LIFE - f.t) as i32)
}

fn bump_lift(s: &Session, tx: i32, ty: i32) -> i32 {
    bump_age(s, tx, ty).map_or(0, |age| [0, 2, 4, 5, 4, 2, 0, 0][(age as usize).min(7)])
}

fn tiles(im: &mut Img, s: &Session, cam: (i32, i32), tick: u32) {
    let view = View {
        world: world(),
        changed: &s.changed,
    };
    let (tx0, ty0) = (cam.0.div_euclid(TILE), cam.1.div_euclid(TILE));
    for ty in ty0..=ty0 + H / TILE + 1 {
        for tx in tx0..=tx0 + W / TILE + 1 {
            let t = view.tile(tx, ty);
            if t == Tile::Air {
                continue;
            }
            let (x, y) = (tx * TILE - cam.0, ty * TILE - cam.1);
            let up = view.tile(tx, ty - 1);
            let lift = bump_lift(s, tx, ty);
            if let Some(pic) = painted_tile(t, tick, bump_age(s, tx, ty)) {
                im.stamp(&pic.img, x, y - lift);
                continue;
            }
            draw_tile(im, t, x, y - lift, up.solid(), tx, ty, tick);
        }
    }
}

/// The painted picture of a tile, when its slot is painted whole. A rune block
/// pulses at rest, flares and fades through the eight ticks of its bump (`age`
/// ticks since the strike) and rests dull once spent; a hatch is shut (the
/// rules keep no open hatch to show).
fn painted_tile(t: Tile, tick: u32, age: Option<i32>) -> Option<&'static bank::Pic> {
    match t {
        Tile::Stone => bank::pic("tiles", "stone", 0),
        Tile::Snow => bank::pic("tiles", "snow", 0),
        Tile::Ledge => bank::pic("tiles", "ledge", 0),
        Tile::Spikes => bank::pic("tiles", "spikes", 0),
        Tile::Hatch => bank::pic("tiles", "hatch", 0),
        Tile::Coin | Tile::Power | Tile::Many(_) | Tile::Spent => match age {
            Some(age) if (0..8).contains(&age) => bank::at("rune-block", "bump", age as u32),
            _ if t == Tile::Spent => bank::pic("rune-block", "spent", 0),
            _ => bank::at("rune-block", "idle", tick),
        },
        _ => None,
    }
}

fn draw_tile(im: &mut Img, t: Tile, x: i32, y: i32, covered: bool, tx: i32, ty: i32, tick: u32) {
    let n = TILE;
    match t {
        Tile::Air => {}
        Tile::Stone => {
            im.rect(x, y, n, n, 'G');
            // Mortar and a lit top edge.
            im.rect(x, y + n / 2, n, 1, 'j');
            let off = if ty % 2 == 0 { 0 } else { 8 };
            im.rect(x + (off + 8) % n, y, 1, n / 2, 'j');
            im.rect(x + off % n, y + n / 2, 1, n / 2, 'j');
            if !covered {
                im.rect(x, y, n, 2, 'J');
                im.rect(x, y + 2, n, 1, 'j');
            }
        }
        Tile::Snow => {
            im.rect(x, y, n, n, 'V');
            im.rect(x, y + 5, n, n - 5, 'v');
            if !covered {
                im.rect(x, y, n, 3, 'W');
            }
            if mix(tx, ty) % 3 == 0 {
                im.rect(x + 4, y + 9, 3, 1, 'W');
            }
        }
        Tile::Ledge => {
            im.rect(x, y, n, 5, 'o');
            im.rect(x, y, n, 1, 't');
            im.rect(x, y + 4, n, 1, 'r');
            im.rect(x + 2, y + 5, 2, 3, 'r');
            im.rect(x + n - 4, y + 5, 2, 3, 'r');
        }
        Tile::Coin | Tile::Power | Tile::Many(_) => {
            im.rect(x, y, n, n, 't');
            im.frame(x, y, n, n, 'o');
            im.frame(x + 1, y + 1, n - 2, n - 2, 'T');
            // A rune cut in it: a stem with two branches, the sign of a gift.
            let c = if let Tile::Power = t { 'p' } else { 'r' };
            im.rect(x + 7, y + 3, 2, 10, c);
            im.line(x + 8, y + 4, x + 12, y + 7, c);
            im.line(x + 8, y + 8, x + 12, y + 11, c);
            im.line(x + 8, y + 5, x + 12, y + 8, c);
            if let Tile::Many(_) = t {
                im.rect(x + 3, y + 12, 3, 2, c);
            }
        }
        Tile::Spent => {
            im.rect(x, y, n, n, 'X');
            im.frame(x, y, n, n, 'g');
            im.rect(x + 3, y + 3, 2, 2, 'g');
            im.rect(x + n - 5, y + n - 5, 2, 2, 'g');
        }
        Tile::Brick => {
            im.rect(x, y, n, n, 'R');
            for r in 0..4 {
                im.rect(x, y + r * 4, n, 1, 'p');
            }
            im.rect(x + 8, y, 1, 4, 'p');
            im.rect(x + 4, y + 4, 1, 4, 'p');
            im.rect(x + 12, y + 4, 1, 4, 'p');
            im.rect(x + 8, y + 8, 1, 4, 'p');
            im.rect(x + 4, y + 12, 1, 4, 'p');
            im.rect(x + 12, y + 12, 1, 4, 'p');
        }
        Tile::Cracked => {
            im.rect(x, y, n, n, 'j');
            im.frame(x, y, n, n, 'g');
            im.line(x + 3, y, x + 6, y + 7, 'k');
            im.line(x + 6, y + 7, x + 4, y + 11, 'k');
            im.line(x + 6, y + 7, x + 12, y + 10, 'k');
            im.line(x + 12, y + 10, x + 13, y + 15, 'k');
        }
        Tile::Water => {
            im.rect(x, y, n, n, 'q');
            if !covered {
                im.rect(x, y, n, 3, 'Q');
            }
            let phase = (tick / 6) as i32;
            for k in 0..2 {
                let wx = x + (tri(tx * 16 + phase * 2 + k * 8, 16)) % 14;
                im.rect(wx, y + 1 + k * 5, 4, 1, 'z');
            }
        }
        Tile::Spikes => {
            for k in 0..4 {
                let bx = x + k * 4;
                im.rect(bx, y + 10, 4, 6, 'i');
                im.rect(bx + 1, y + 6, 2, 4, 'H');
                im.put(bx + 1, y + 5, 'H');
            }
        }
        Tile::Hatch => {
            im.rect(x, y, n, n, 'B');
            im.frame(x, y, n, n, 'b');
            im.rect(x + 2, y + 2, n - 4, 3, 'o');
            im.frame(x + 5, y + 6, 6, 6, 't');
            im.line(x + 3, y + 8, x + 12, y + 8, 'b');
        }
        Tile::Gate(k) => {
            // A tapestry over a door, the count it opens at on the top.
            im.rect(x, y, n, n, 'p');
            im.rect(x, y, 2, n, 'T');
            im.rect(x + n - 2, y, 2, n, 'T');
            im.rect(x + 3, y, 1, n, 'R');
            im.rect(x + n - 4, y, 1, n, 'R');
            if mix(tx, ty) % 2 == 0 {
                im.rect(x + 6, y + 6, 4, 4, 'O');
            }
            if ty % 4 == 0 {
                let label = if k == 0 { "2" } else { "7" };
                im.text(x + 5, y + 4, label, '6');
            }
        }
    }
}

// -------------------------------------------------------------- props

fn on_screen(x: i32, y: i32, cam: (i32, i32), pad: i32) -> bool {
    x - cam.0 > -pad && x - cam.0 < W + pad && y - cam.1 > -pad && y - cam.1 < H + pad
}

fn props(im: &mut Img, s: &Session, cam: (i32, i32), tick: u32) {
    let i = info();
    // Signboards.
    for &(cx, cy, g) in &i.signs {
        let (x, foot) = (cx * TILE + 8, (cy + 1) * TILE);
        if !on_screen(x, foot, cam, 80) {
            continue;
        }
        let (sx, sy) = (x - cam.0, foot - cam.1);
        im.rect(sx - 1, sy - 12, 2, 12, 'P');
        let words = layout::sign_text(g);
        let w = text_width(words) + 6;
        im.rect(sx - w / 2, sy - 28, w, 14, 'B');
        im.frame(sx - w / 2, sy - 28, w, 14, 'o');
        im.text(sx - w / 2 + 3, sy - 24, words, 'T');
    }
    // The goal pole and the Great Pole.
    let (px, base) = (i.pole.0 * TILE + 8, (i.pole.1 + 1) * TILE);
    if on_screen(px, base - 80, cam, 60) {
        let (sx, sy) = (px - cam.0, base - cam.1);
        let had = s.banners & 4 != 0;
        // Painted: a limp pennon before the grab, mid-height while a knight
        // slides, flying at the top once the banner is had.
        let sliding = s.seats.values().any(|k| k.pole > 0);
        let own = if sliding {
            bank::pic("flagpole", "slide", 0)
        } else if had {
            bank::at("flagpole", "fly", tick)
        } else {
            bank::pic("flagpole", "idle", 0)
        };
        if let Some(pic) = own {
            bank::stamp(im, pic, sx, sy);
        } else {
            im.rect(sx - 1, sy - 9 * TILE, 2, 9 * TILE, 'v');
            im.rect(sx - 2, sy - 9 * TILE - 3, 4, 4, '6');
            // Its flag, run up the pole once somebody has had it.
            let fy = if had { sy - 22 } else { sy - 9 * TILE + 3 };
            im.rect(sx + 1, fy, 14, 9, if had { 'z' } else { 'R' });
            im.frame(sx + 1, fy, 14, 9, 'k');
            im.text(sx + 4, fy + 1, "M", 'W');
            im.rect(sx - 6, sy - 4, 12, 4, 'j');
        }
    }
    great_pole(im, s, cam, tick);
    // Gates.
    for (n, &c) in i.gates.iter().enumerate() {
        let (cx, _) = cell_centre(c);
        let foot = (c.1 + 1) * TILE;
        if !on_screen(cx, foot, cam, 40) {
            continue;
        }
        let (sx, sy) = (cx - cam.0, foot - cam.1);
        let raised = s.gates & (1 << n) != 0;
        // Painted: the dull pennon low, the banner flying once raised (the
        // rules keep no clock for the climb between).
        let own = if raised { bank::at("gate", "up", tick) } else { bank::pic("gate", "down", 0) };
        if let Some(pic) = own {
            bank::stamp(im, pic, sx, sy);
            continue;
        }
        im.rect(sx - 1, sy - 26, 2, 26, 'P');
        im.rect(sx - 3, sy - 28, 6, 3, 'o');
        if raised {
            let flutter = (tick / 5 % 3) as i32;
            im.rect(sx + 1, sy - 26, 11 - flutter, 7, 'O');
            im.rect(sx + 1, sy - 26, 11 - flutter, 1, 'T');
            im.put(sx + 4, sy - 23, 'k');
        } else {
            im.rect(sx + 1, sy - 12, 9, 6, 'p');
        }
    }
    // Banner pickups.
    for b in 0..8usize {
        if s.banners & (1 << b) != 0 {
            continue;
        }
        let Some(c) = i.banners[b] else { continue };
        if b == 7 && !s.crown_down() {
            continue;
        }
        let (cx, _) = cell_centre(c);
        let foot = (c.1 + 1) * TILE;
        if !on_screen(cx, foot, cam, 40) {
            continue;
        }
        let (sx, sy) = (cx - cam.0, foot - cam.1);
        let bob = (tri(tick as i32, 40) / 7) - 1;
        pennant(im, sx, sy - 4 + bob, banner_ink(b), tick, 24);
    }
    // The bell chute over the snow door.
    for m in world().marks_of('j') {
        let (cx, _) = cell_centre((m.x, m.y));
        if on_screen(cx, m.y * TILE, cam, 40) {
            let (sx, sy) = (cx - cam.0, m.y * TILE - cam.1);
            im.rect(sx - 6, sy - 40, 12, 56, 'X');
            im.frame(sx - 6, sy - 40, 12, 56, 'g');
            text_centre(im, sx, sy - 54, "BELL", 'h');
        }
    }
    // The wheel, the logs.
    for m in layout::movers(s.elapsed) {
        let p = m.p;
        if !on_screen(p.x, p.y, cam, 60) {
            continue;
        }
        let (sx, sy) = (p.x - cam.0, p.y - cam.1);
        if m.id >= layout::WHEEL_ID {
            im.rect(sx, sy, p.w, 4, 'o');
            im.rect(sx, sy, p.w, 1, 't');
            im.rect(sx + 2, sy + 4, 2, 3, 'r');
            im.rect(sx + p.w - 4, sy + 4, 2, 3, 'r');
        } else {
            im.rect(sx, sy, p.w, 6, 'r');
            im.rect(sx, sy, p.w, 1, 'O');
            im.rect(sx + 1, sy + 6, p.w - 2, 2, 'p');
            im.rect(sx, sy + 2, 3, 2, 'o');
            im.rect(sx + p.w - 3, sy + 3, 3, 2, 'o');
        }
    }
    let (wx, wy) = layout::WHEEL;
    if on_screen(wx, wy, cam, 80) {
        let (sx, sy) = (wx - cam.0, wy - cam.1);
        let deg = (s.elapsed % layout::WHEEL_TURN) as i32 * 360 / layout::WHEEL_TURN as i32;
        if let Some(pic) = painted_wheel(deg) {
            bank::stamp(im, pic, sx, sy);
        } else {
            // A solid three pixel ring and two pixel spokes, in the planks' own
            // wood with a dark edge: a dotted one pixel line is lost on a busy
            // painted sky.
            for (r, c) in [(39, 'k'), (40, 'o'), (41, 'o'), (42, 'o'), (43, 'k')] {
                for d in 0..360 {
                    im.put(sx + r * cos_s(d) / 1024, sy + r * sin_s(d) / 1024, c);
                }
            }
            im.ellipse(sx as f32, sy as f32, 5.0, 5.0, 'k');
            im.ellipse(sx as f32, sy as f32, 4.0, 4.0, 'o');
            for k in 0..layout::WHEEL_BUCKETS {
                let phi = deg + k * 60;
                let (c, sn) = (cos_s(phi), sin_s(phi));
                let (ex, ey) = (sx + 40 * c / 1024, sy + 40 * sn / 1024);
                im.line(sx - 1, sy, ex - 1, ey, 'k');
                im.line(sx + 2, sy, ex + 2, ey, 'k');
                im.line(sx, sy, ex, ey, 'o');
                im.line(sx + 1, sy, ex + 1, ey, 'o');
            }
            text_centre(im, sx, sy - 54, "WHEEL", 'h');
        }
    }
}

/// How far each painted wheel frame is turned clockwise (degrees, from the
/// producer's measure of the sheet: not even steps), and the buckets are six,
/// so a turn repeats every sixty.
const WHEEL_FRAMES: [i32; 6] = [0, 9, 13, 21, 30, 37];

/// The painted frame whose buckets sit nearest the rules' buckets at `deg`
/// (the rules' bucket `k` is at `deg + 60 k` from east, clockwise; the painted
/// wheel's top bucket at its frame's angle from north). Never more than about
/// twelve degrees apart: art never moves a platform.
fn painted_wheel(deg: i32) -> Option<&'static bank::Pic> {
    let want = (deg + 30).rem_euclid(60);
    let (index, _) = WHEEL_FRAMES
        .iter()
        .copied()
        .chain([60])
        .enumerate()
        .map(|(n, a)| (n % WHEEL_FRAMES.len(), (a - want).abs()))
        .min_by_key(|&(_, gap)| gap)?;
    bank::pic("wheel", "turn", index)
}

fn sin_s(deg: i32) -> i32 {
    layout::sine(deg)
}
fn cos_s(deg: i32) -> i32 {
    layout::cosine(deg)
}

/// A pennant on a staff, its foot at `(x, foot)`.
fn pennant(im: &mut Img, x: i32, foot: i32, ink: char, tick: u32, tall: i32) {
    im.rect(x - 1, foot - tall, 2, tall, 'P');
    let flutter = (tick / 6 % 3) as i32;
    im.rect(x + 1, foot - tall + 1, 13 - flutter, 10, ink);
    im.frame(x + 1, foot - tall + 1, 13 - flutter, 10, 'k');
    // It shines: a glint slides across.
    let g = (tick as i32 * 3 / 2) % 40;
    if g < 13 {
        im.rect(x + 1 + g, foot - tall + 2, 1, 8, 'w');
    }
    im.rect(x - 2, foot - tall - 2, 4, 3, '6');
}

fn great_pole(im: &mut Img, s: &Session, cam: (i32, i32), tick: u32) {
    let i = info();
    let (cx, _) = cell_centre(i.great_pole);
    let foot = (i.great_pole.1 + 1) * TILE;
    if !on_screen(cx, foot - 100, cam, 100) {
        return;
    }
    let (sx, sy) = (cx - cam.0, foot - cam.1);
    let top = sy - 132;
    if let Some(pic) = bank::pic("great-pole", "idle", 0) {
        bank::stamp(im, pic, sx, sy);
    } else {
        im.rect(sx - 2, top, 4, 132, 'P');
        im.rect(sx - 8, sy - 6, 16, 6, 'j');
        im.rect(sx - 28, top + 4, 56, 3, 'o');
        im.rect(sx - 3, top - 4, 6, 6, '6');
    }
    // Eight places, four to a side, each lit when its banner is raised.
    for n in 0..8usize {
        let left = n % 2 == 0;
        let row = (n / 2) as i32;
        let y = top + 10 + row * 24;
        let (x0, w) = if left { (sx - 30, 26) } else { (sx + 4, 26) };
        let earned = s.banners & (1 << n) != 0;
        if earned {
            let flutter = ((tick / 7) as i32 + n as i32) % 3;
            im.rect(x0, y, w - flutter, 19, banner_ink(n));
            im.frame(x0, y, w - flutter, 19, 'k');
            let g = (tick as i32 * 2 + n as i32 * 9) % 60;
            if g < w {
                im.rect(x0 + g, y + 1, 1, 17, 'w');
            }
            im.text(x0 + w / 2 - 2, y + 6, &format!("{}", n + 1), 'k');
        } else {
            im.frame(x0, y, w, 19, 'g');
            im.text(x0 + w / 2 - 2, y + 6, &format!("{}", n + 1), 'g');
        }
    }
}

// ------------------------------------------------------- foes and things

fn things(im: &mut Img, s: &Session, cam: (i32, i32), tick: u32) {
    let i = info();
    // Gold sparks and the ring of eight.
    for (n, &c) in i.coins.iter().enumerate() {
        if s.taken.contains(&(n as u16)) {
            continue;
        }
        let (cx, cy) = cell_centre(c);
        if !on_screen(cx, cy, cam, 20) {
            continue;
        }
        let bob = tri((tick as i32 + n as i32 * 5) / 2, 16) / 4 - 1;
        let (x, y) = (cx - cam.0, cy - cam.1 + bob);
        match bank::at("spark", "bob", tick + n as u32 * 5) {
            Some(pic) => bank::stamp(im, pic, x, y),
            None => coin(im, x, y, tick as i32 + n as i32 * 3),
        }
    }
    for (n, &c) in i.sparks.iter().enumerate() {
        if s.sparks & (1 << n) != 0 {
            continue;
        }
        let (cx, cy) = cell_centre(c);
        if !on_screen(cx, cy, cam, 20) {
            continue;
        }
        let bob = tri(tick as i32 + n as i32 * 9, 40) / 6 - 2;
        let (x, y) = (cx - cam.0, cy - cam.1 + bob);
        let r = 6 + (tick as i32 / 10 % 2);
        im.ellipse(x as f32, y as f32, r as f32, r as f32, '6');
        im.ellipse(x as f32, y as f32, (r - 2) as f32, (r - 2) as f32, 'k');
        im.ellipse(x as f32, y as f32, 2.0, 2.0, 'w');
        let g = (tick as i32 / 3 + n as i32) % 12;
        im.put(x - 6 + g, y - 6, 'w');
    }
    // Foes.
    for f in &s.foes {
        let (l, t, r, b) = f.rect();
        if !on_screen(l, t, cam, 40) {
            continue;
        }
        let (x, y) = (l - cam.0, t - cam.1);
        let w = r - l;
        let h = b - t;
        if !f.alive() {
            pop(im, x + w / 2, y + h / 2, f.dead);
            continue;
        }
        match f.kind {
            FoeKind::Snuffer | FoeKind::Capped => {
                // A hooded wisp with a wick-stub tail and two pale eyes.
                im.rect(x, y + 4, w, h - 4, 'S');
                im.rect(x + 2, y + 2, w - 4, 3, 'S');
                im.rect(x + 4, y, w - 8, 3, 'S');
                im.frame(x, y + 4, w, h - 4, 'x');
                let eye = if f.dir > 0 { 1 } else { 0 };
                im.rect(x + 3 + eye * 3, y + 6, 2, 3, 'w');
                im.rect(x + 8 + eye * 2 - 1, y + 6, 2, 3, 'w');
                let tail_x = if f.dir > 0 { x - 2 } else { x + w };
                im.rect(tail_x, y + h - 5, 2, 4, 'O');
                // Feet that shuffle.
                let step = (f.t / 6 % 2) as i32;
                im.rect(x + 1 + step, b - cam.1 - 2, 4, 2, 'x');
                im.rect(x + w - 5 - step, b - cam.1 - 2, 4, 2, 'x');
                if f.kind == FoeKind::Capped {
                    im.rect(x - 1, y - 2, w + 2, 6, 'B');
                    im.rect(x + 1, y - 4, w - 2, 3, 'B');
                    im.rect(x - 1, y + 2, w + 2, 2, 'b');
                    im.rect(x + 3, y - 3, 3, 1, 'o');
                }
            }
            FoeKind::Wisp => {
                im.ellipse((x + w / 2) as f32, (y + h / 2) as f32, 6.0, 6.0, 'V');
                im.ellipse((x + w / 2) as f32, (y + h / 2 + 1) as f32, 4.0, 4.0, 'W');
                im.rect(x + 3, y + 4, 2, 3, 'k');
                im.rect(x + 7, y + 4, 2, 3, 'k');
                let flick = (f.t / 4 % 3) as i32;
                im.rect(x + 4 + flick, y + h, 2, 4, 'O');
            }
        }
    }
    // Items.
    for it in &s.items {
        let (l, t, r, b) = it.rect();
        if !on_screen(l, t, cam, 30) {
            continue;
        }
        let (x, y) = (l - cam.0, t - cam.1);
        let (w, h) = (r - l, b - t);
        match it.kind {
            ItemKind::Loaf => {
                im.ellipse((x + w / 2) as f32, (y + h / 2 + 1) as f32, 6.0, 5.0, 'o');
                im.rect(x + 1, y + h / 2, w - 2, h / 2, 'o');
                im.rect(x + 3, y + 2, w - 6, 1, 't');
                im.line(x + 3, y + 3, x + 5, y + 6, 'r');
                im.line(x + 7, y + 3, x + 9, y + 6, 'r');
                im.frame(x, y + 2, w, h - 2, 'b');
            }
            ItemKind::Flame => {
                // Painted: the flame kindles over the sixteen ticks it rises
                // out of its block, then flickers.
                let own = if it.rising() { bank::at("flame", "rise", it.t) } else { bank::at("flame", "idle", tick) };
                if let Some(pic) = own {
                    bank::stamp(im, pic, x + w / 2, y + h);
                } else {
                    let k = (tick / 3 % 3) as i32;
                    im.rect(x + 3, y + 8, 5, 6, 'R');
                    im.rect(x + 2 + k % 2, y + 4, 6, 6, 'O');
                    im.rect(x + 4, y + 1 + k, 3, 6, 'T');
                    im.rect(x + 4, y + 8, 3, 3, 'w');
                }
            }
            ItemKind::Cap => {
                let on = it.cap;
                im.rect(x, y + 3, w, h - 3, 'B');
                im.rect(x + 2, y, w - 4, 4, 'B');
                im.rect(x, y + h - 3, w, 3, 'b');
                im.rect(x + 3, y + 1, 4, 1, 'o');
                if on == CapState::Slide {
                    im.rect(x - it.dir as i32 * 3, y + 4, 2, 1, 'h');
                }
            }
        }
    }
    for b in &s.bolts {
        let (l, t, r, bt) = b.rect();
        if !on_screen(l, t, cam, 10) {
            continue;
        }
        im.rect(l - cam.0, t - cam.1, r - l, bt - t, 'O');
        im.rect(l - cam.0 + 1, t - cam.1 + 1, 4, 4, 'T');
    }
    cinder(im, s, cam, tick);
    for b in &s.boulders {
        let (bw, bh) = BOULDER;
        let x = b.x.div_euclid(Q) - cam.0;
        let foot = layout::SNOW_FLOOR * TILE - cam.1;
        if x < -40 || x > W + 40 {
            continue;
        }
        // Painted: it rolls toward the west, so the frames (a quarter turn
        // clockwise each) play backwards; its foot is on the floor.
        let turns = bank::frames("boulder", "roll");
        if turns > 0 {
            let seen = bank::index_at("boulder", "roll", b.t).unwrap_or(0);
            if let Some(pic) = bank::pic("boulder", "roll", (turns - seen) % turns) {
                bank::stamp(im, pic, x, foot);
                continue;
            }
        }
        let spin = (b.t as i32 / 3) % 8;
        im.ellipse((x) as f32, (foot - bh / 2) as f32, (bw / 2) as f32, (bh / 2) as f32, 'X');
        im.ellipse((x) as f32, (foot - bh / 2) as f32, (bw / 2 - 3) as f32, (bh / 2 - 3) as f32, 'g');
        im.rect(x - 2 + spin % 3, foot - bh + 4, 4, 5, 'o');
        im.rect(x - 8, foot - bh / 2, 16, 2, 'j');
        text_centre(im, x, foot - bh - 10, "BELL", 'T');
    }
}

/// King Cinder: a hulking block with a crown of embers; what is marked to hurt
/// is framed (amber while it tells, red while it strikes).
fn cinder(im: &mut Img, s: &Session, cam: (i32, i32), tick: u32) {
    let b = &s.boss;
    let f = boss::floor();
    if !on_screen(b.x, f - 60, cam, 140) {
        return;
    }
    // Marked ground first, under him.
    for h in &b.hazards {
        let (l, t, r, bt) = h.rect();
        let (x, y) = (l - cam.0, t - cam.1);
        let (w, hh) = (r - l, bt - t);
        if h.tell > 0 {
            // A warning that closes in as the blow nears.
            if tick / 3 % 2 == 0 {
                im.frame(x, y, w, hh, 'O');
            }
            im.rect(x, y + hh - 2, w, 2, 'O');
        } else if h.vx != 0 {
            // A wave along the floor.
            im.rect(x, y, w, hh, '7');
            im.rect(x + 2, y + 2, (w - 4).max(1), (hh - 4).max(1), 'O');
        } else {
            im.rect(x, y, w, hh, '7');
            im.rect(x + w / 4, y, (w / 2).max(1), hh, 'O');
            im.rect(x + w / 2 - 1, y, 2, hh, 'T');
        }
    }
    let (bl, bt, br, bb) = b.body();
    let (x, y) = (bl - 4 - cam.0, bt - cam.1);
    let w = br - bl + 8;
    let h = bb - bt;
    let hit = b.flinch > 0;
    let skin = match b.beat {
        BossBeat::Rage => if tick / 2 % 2 == 0 { 'O' } else { '7' },
        BossBeat::Down => 'X',
        _ => if hit { 'H' } else { 'p' },
    };
    // The body, hunched, with arms; he kneels when the crown is low.
    im.rect(x, y + 6, w, h - 6, skin);
    im.frame(x, y + 6, w, h - 6, 'k');
    im.rect(x + 2, y, w - 4, 10, skin);
    im.frame(x + 2, y, w - 4, 10, 'k');
    // Eyes: banked coals.
    let glow = if b.beat == BossBeat::Dormant { 'x' } else if b.beat == BossBeat::Down { 'g' } else { '6' };
    im.rect(x + 8, y + 4, 4, 3, glow);
    im.rect(x + w - 12, y + 4, 4, 3, glow);
    // Arms: raised on a tell, down on a strike.
    let up = b.beat == BossBeat::Tell;
    let ay = if up { y - 10 } else { y + h / 2 };
    im.rect(x - 8, ay, 8, 18, skin);
    im.rect(x + w, ay, 8, 18, skin);
    im.frame(x - 8, ay, 8, 18, 'k');
    im.frame(x + w, ay, 8, 18, 'k');
    // The crown, plated: one plate to a seat.
    if !b.defeated() {
        let top = y - 2;
        let n = i32::from(b.plates.max(1));
        let cw = 36;
        let step = (cw / n).clamp(4, 12);
        for k in 0..i32::from(b.plates) {
            let cx = x + w / 2 - (n * step) / 2 + k * step;
            im.rect(cx, top - 10, step - 1, 10, '6');
            im.rect(cx, top - 10, step - 1, 2, 'w');
            im.frame(cx, top - 10, step - 1, 10, 'k');
        }
        if b.beat == BossBeat::Wince && tick / 6 % 2 == 0 {
            // The window: the crown is low, now.
            text_centre(im, x + w / 2, top - 24, "SLAM", 'T');
        }
    }
    text_centre(im, x + w / 2, y + h + 3, "KING CINDER", 'h');
}

fn coin(im: &mut Img, x: i32, y: i32, t: i32) {
    let w = [6, 5, 2, 5][(t / 4 % 4) as usize];
    im.rect(x - w / 2, y - 4, w.max(2), 8, '5');
    im.rect(x - w / 2, y - 4, w.max(2), 1, '6');
    if w > 3 {
        im.rect(x - 1, y - 2, 2, 4, '4');
    }
}

fn pop(im: &mut Img, x: i32, y: i32, dead: u32) {
    // Cold smoke, and a spark of returned light.
    let age = (crate::rules::bannerpeak::foes::POP - dead) as i32;
    let r = 3 + age / 2;
    for k in 0..6 {
        let dx = [-1, 1, -2, 2, 0, 0][k] * r;
        let dy = [-1, -1, 1, 1, -2, 2][k] * r / 2;
        im.rect(x + dx - 2, y + dy - 2, 4, 4, if age < 6 { 'h' } else { 'j' });
    }
    if age < 8 {
        im.rect(x - 1, y - 6 - age, 3, 3, '6');
    }
}

// ------------------------------------------------------------- knights

fn knight(
    im: &mut Img,
    run: &Run,
    s: &Session,
    seat: u32,
    k: &Knight,
    cam: (i32, i32),
    mine: bool,
    tick: u32,
) {
    let ink = seat_ink(seat);
    if let Some(b) = k.bubble {
        let (x, y) = (b.x.div_euclid(Q) - cam.0, b.y.div_euclid(Q) - cam.1);
        let wob = tri(tick as i32 + seat as i32 * 7, 24) / 6 - 2;
        im.ellipse(x as f32, (y + wob) as f32, 13.0, 13.0, 'W');
        im.ellipse(x as f32, (y + wob) as f32, 11.0, 11.0, 'z');
        im.ellipse(x as f32, (y + wob) as f32, 9.0, 9.0, 'k');
        im.rect(x - 4, y + wob - 5, 8, 10, ink);
        im.frame(x - 4, y + wob - 5, 8, 10, 'k');
        text_centre(im, x, y + wob - 24, &format!("{seat}"), ink);
        // A fading bubble blinks.
        if b.t > 270 && tick / 4 % 2 == 0 {
            im.ellipse(x as f32, (y + wob) as f32, 13.0, 13.0, 'k');
        }
        return;
    }
    if k.pit > 0 {
        return;
    }
    // A hit knight blinks.
    if k.hurt > 0 && tick / 3 % 2 == 0 && k.pose == 0 {
        return;
    }
    let (fx_px, fy_px) = k.body.foot();
    let (sx, sy) = (fx_px - cam.0, fy_px - cam.1);
    if sx < -40 || sx > W + 40 || sy < -60 || sy > H + 60 {
        return;
    }
    let (w, h) = k.body.dims();
    let face = i32::from(k.body.face);
    let pose = if k.pole > 0 { Pose::Pole } else if k.pose > 0 { Pose::Item } else { k.body.pose };
    // The block: its box, a visor on the facing side, a helm on top.
    let (bw, bh, lean) = match pose {
        Pose::Dive | Pose::Belly => (w + 8, h.min(12), 0),
        Pose::Wall => (w, h, -face * 2),
        Pose::Slide => (w + 4, h, face * 2),
        Pose::Land => (w + 3, h - 2, 0),
        _ => (w, h, 0),
    };
    let x0 = sx - bw / 2 + lean;
    let y0 = sy - bh;
    // The shadow of a knight in the air.
    if !k.body.ground {
        if let Some(ground) = ground_below(s, fx_px, fy_px) {
            let gy = ground - cam.1;
            if gy > sy && gy - sy < 120 {
                im.rect(sx - 4, gy - 1, 8, 1, 'k');
            }
        }
    }
    let body_ink = if mine { ink } else { dim(ink) };
    im.rect(x0, y0, bw, bh, body_ink);
    im.frame(x0, y0, bw, bh, 'k');
    // Helm: lighter, and the visor slot toward the way faced.
    let helm = if k.big() { 8 } else { 6 };
    im.rect(x0 + 1, y0 + 1, bw - 2, helm.min(bh - 2), 'H');
    let vx = if face > 0 { x0 + bw - 5 } else { x0 + 2 };
    im.rect(vx, y0 + 3, 3, 2, 'k');
    // A flame knight carries a lantern.
    if k.size == Size::Flame {
        let lx = if face > 0 { x0 + bw } else { x0 - 4 };
        im.rect(lx, y0 + bh / 2, 4, 5, 'O');
        im.rect(lx + 1, y0 + bh / 2 - 2, 2, 3, 'T');
    }
    // Pose marks: the picture of what the knight is doing.
    match pose {
        Pose::Spin => {
            let a = (tick / 2 % 4) as i32;
            im.line(x0 - 3, y0 + a * 3, x0 + bw + 2, y0 + bh - a * 3, 'w');
            // The third jump leaves a trail.
            for n in 1..=3 {
                let ox = -face * n * 6;
                im.frame(x0 + ox, y0 + n * 2, bw, bh, 'h');
            }
        }
        Pose::Second => {
            for n in 1..=2 {
                im.frame(x0 - face * n * 4, y0 + n, bw, bh, 'j');
            }
        }
        Pose::Long => im.rect(x0 - face * 10, y0 + bh / 2, 8, 1, 'h'),
        Pose::Back => im.line(x0, y0 + bh, x0 + bw, y0, 'w'),
        Pose::Slam | Pose::Wind => {
            im.rect(x0 - 2, y0 - 5, bw + 4, 2, 'w');
            im.rect(x0 + bw / 2 - 1, y0 - 9, 2, 5, 'w');
        }
        Pose::Dive => im.rect(x0 - face * 8, y0 + 4, 7, 1, 'h'),
        Pose::Skid => {
            im.rect(x0 - face * 3, sy - 2, 6, 2, 'h');
        }
        Pose::Brace => {
            // The shield, raised.
            let sxp = if face > 0 { x0 + bw } else { x0 - 5 };
            im.rect(sxp, y0 - 1, 5, bh + 2, 'u');
            im.frame(sxp, y0 - 1, 5, bh + 2, 'k');
            im.rect(x0 - 2, y0 - 3, bw + 4, 3, 'U');
            im.frame(x0 - 2, y0 - 3, bw + 4, 3, 'k');
        }
        Pose::Item => {
            im.rect(x0 + bw / 2 - 1, y0 - 22, 2, 18, 'P');
            im.rect(x0 + bw / 2 + 1, y0 - 22, 10, 8, '6');
            im.rect(x0 - 3, y0 + 3, 3, 2, body_ink);
            im.rect(x0 + bw, y0 + 3, 3, 2, body_ink);
        }
        Pose::Pole => {
            let px = x0 + bw / 2 + face * 4;
            im.rect(px, y0 - 6, 2, bh + 8, 'v');
        }
        Pose::Run | Pose::RunB => {
            let step = if pose == Pose::Run { 0 } else { 3 };
            im.rect(x0 + 1 + step, sy - 3, 4, 3, 'k');
            im.rect(x0 + bw - 5 - step, sy - 3, 4, 3, 'k');
        }
        Pose::Wall => {
            let wx = if k.body.wall > 0 { x0 + bw } else { x0 - 2 };
            im.rect(wx, y0 + 2, 2, bh - 2, 'w');
        }
        _ => {}
    }
    // Who it is: the seat's number over their head.
    let who = format!("{seat}");
    text_centre(im, sx, y0 - 12 - if pose == Pose::Item { 12 } else { 0 }, &who, ink);
    if mine {
        // An arrow over your own knight for the first moments.
        if s.elapsed < 150 && tick / 8 % 2 == 0 {
            im.rect(sx - 2, y0 - 22, 5, 2, 'W');
            im.rect(sx - 1, y0 - 20, 3, 2, 'W');
            im.put(sx, y0 - 18, 'W');
        }
    } else if let Some(hero) = run.players.get(&seat) {
        // Friends carry their class on the ground.
        let _ = classes::of(hero);
    }
}

fn dim(c: char) -> char {
    match c {
        'z' => 'Q',
        'O' => 'o',
        'y' => 'A',
        'T' => 'O',
        'V' => 'U',
        'C' => 'e',
        _ => c,
    }
}

fn ground_below(s: &Session, x: i32, y: i32) -> Option<i32> {
    let view = s.view();
    for ty in y.div_euclid(TILE)..y.div_euclid(TILE) + 9 {
        let t = view.tile(x.div_euclid(TILE), ty);
        if t.solid() || t.one_way() {
            return Some(ty * TILE);
        }
    }
    None
}

// -------------------------------------------------------------- effects

fn effects(fx: &mut Img, s: &Session, cam: (i32, i32), tick: u32) {
    let _ = tick;
    for f in &s.fx {
        let age = (FX_LIFE - f.t) as i32;
        let (x, y) = (f.x - cam.0, f.y - cam.1);
        if !(-20..W + 20).contains(&x) || !(-20..H + 20).contains(&y) {
            continue;
        }
        match f.k {
            FxKind::Coin => {
                // A coin pinged up out of the block, spinning.
                let up = [4, 9, 13, 16, 18, 18, 16, 12, 6][age.min(8) as usize];
                fx.rect(x - 2, y - up, 4, 6, '5');
                fx.rect(x - 2, y - up, 4, 1, '6');
            }
            FxKind::Dust => {
                for d in [-1i32, 1] {
                    let r = 2 + age / 2;
                    fx.rect(x + d * (3 + age) - r / 2, y - r, r, r, 'h');
                }
            }
            FxKind::Ring => {
                // A dust ring racing out along the ground.
                let r = 4 + age * 5;
                ring(fx, x, y - 1, r, r / 4 + 2, if age < 8 { 'W' } else { 'h' });
                if age > 2 {
                    ring(fx, x, y - 1, r - 5, (r - 5) / 4 + 1, 'h');
                }
            }
            FxKind::Debris => {
                for (d, up) in [(-1, 3), (1, 4), (0, 5)] {
                    let t = age;
                    fx.rect(x + d * t * 2, y - up * t + t * t / 2, 3, 3, 'R');
                }
            }
            FxKind::Gleam => {
                let r = 2 + age;
                fx.rect(x - r, y, r * 2 + 1, 1, 'w');
                fx.rect(x, y - r, 1, r * 2 + 1, 'w');
                fx.rect(x - 1, y - 1, 3, 3, '6');
            }
            FxKind::Smoke => {
                let r = 3 + age;
                fx.rect(x - r, y - age, r * 2, r, 'j');
            }
            FxKind::Pop | FxKind::Bump => {}
        }
    }
}

/// The outline of an ellipse, a dot every few degrees.
fn ring(im: &mut Img, cx: i32, cy: i32, rx: i32, ry: i32, ink: char) {
    for deg in (0..360).step_by(5) {
        im.put(
            cx + rx * layout::cosine(deg) / 1024,
            cy + ry * layout::sine(deg) / 1024,
            ink,
        );
    }
}

// ----------------------------------------------------------------- HUD

fn hud(im: &mut Img, run: &Run, s: &Session, id: u32, tick: u32) {
    let _ = (run, tick);
    let me = s.seats.get(&id);
    im.rect(0, 0, W, 19, 'k');
    im.rect(0, 19, W, 1, 'g');
    let Some(k) = me else { return };
    // Gold sparks.
    coin(im, 9, 9, 0);
    im.text(18, 5, &format!("×{}", k.coins), '6');
    // The pie: eight wedges, the first of them at the top right.
    pie(im, 56, 9, k.pips);
    if k.size != Size::Small {
        im.text(68, 5, if k.size == Size::Flame { "FLAME" } else { "BIG" }, 'O');
    }
    // The eight banners.
    let bx = 112;
    for n in 0..8usize {
        let x = bx + n as i32 * 12;
        if s.banners & (1 << n) != 0 {
            im.rect(x, 3, 9, 12, banner_ink(n));
            im.frame(x, 3, 9, 12, 'k');
            im.rect(x + 2, 4, 1, 10, 'w');
        } else {
            im.frame(x, 3, 9, 12, 'g');
        }
    }
    im.text(bx + 100, 5, &format!("{}/8", s.count()), 'H');
    // The ring of sparks.
    im.text(242, 5, &format!("SPARKS {}/8", s.sparks.count_ones()), '6');
    // The seats: a number and a pie each.
    if s.seats.len() > 1 {
        for (n, (&seat, k)) in s.seats.iter().enumerate() {
            let x = 316 + (n as i32 % 3) * 22;
            let y = 3 + (n as i32 / 3) * 8;
            let ink = seat_ink(seat);
            let label = if k.bubble.is_some() { "O".to_string() } else { format!("{}", k.pips) };
            im.text(x, y, &format!("{seat}:{label}"), ink);
        }
    }
    // Keys, at the bottom.
    im.rect(0, 213, W, 11, 'k');
    im.text(
        4,
        215,
        "SPACE JUMP  S DUCK OR SLAM  F DIVE OR CARRY  E SHIELD",
        'h',
    );
}

/// A pie of eight wedges, `pips` of them full, drawn on the eighths of a circle.
fn pie(im: &mut Img, cx: i32, cy: i32, pips: u8) {
    for dy in -7..=7i32 {
        for dx in -7..=7i32 {
            if dx * dx + dy * dy > 49 {
                continue;
            }
            // Eighth: clockwise from straight up.
            let (ax, ay) = (dx.abs(), dy.abs());
            let steep = ay > ax;
            let wedge = match (dx >= 0, dy <= 0, steep) {
                (true, true, true) => 0,
                (true, true, false) => 1,
                (true, false, false) => 2,
                (true, false, true) => 3,
                (false, false, true) => 4,
                (false, false, false) => 5,
                (false, true, false) => 6,
                (false, true, true) => 7,
            };
            let full = (wedge as u8) < pips;
            let edge = dx * dx + dy * dy > 36;
            let ch = if edge {
                'k'
            } else if full {
                'R'
            } else {
                'X'
            };
            im.put(cx + dx, cy + dy, ch);
            if full && !edge && (dx + dy) % 3 == 0 {
                im.put(cx + dx, cy + dy, 'O');
            }
        }
    }
    // The spokes between wedges.
    for (ex, ey) in [(0, -6), (5, -5), (6, 0), (5, 5), (0, 6), (-5, 5), (-6, 0), (-5, -5)] {
        im.line(cx, cy, cx + ex, cy + ey, 'k');
    }
}

fn overlays(im: &mut Img, s: &Session, me: Option<&Knight>, id: u32, tick: u32, words: bool) {
    let _ = id;
    // The title card, for the first few seconds.
    if words && s.elapsed < 150 && s.phase == Phase::Play {
        large(im, W / 2, 40, "RAISE EIGHT BANNERS", 'T', 2);
    }
    // A banner just raised.
    if let Some(a) = s.announce {
        let name = BANNER_NAMES[usize::from(a.banner)];
        im.rect(48, 26, W - 96, 36, 'k');
        im.frame(48, 26, W - 96, 36, banner_ink(usize::from(a.banner)));
        if words {
            text_centre(im, W / 2, 31, &format!("BANNER {} OF 8", s.count()), banner_ink(usize::from(a.banner)));
            large(im, W / 2, 43, name, 'W', 1);
        }
    }
    if let Some(k) = me {
        if words && k.bubble.is_some() {
            text_centre(im, W / 2, 26, "IN A BUBBLE: A D W S STEER TO A FRIEND", 'z');
        }
    }
    // Doors: what they wait for.
    if words && s.phase == Phase::Play && s.elapsed > 150 && s.announce.is_none() {
        if let Some(k) = me {
            let (fx, fy) = k.body.foot();
            let i = info();
            let (ax, ay) = cell_centre(i.arch);
            if s.doors & 1 == 0 && (fx - ax).abs() < 80 && (fy - ay).abs() < 40 {
                text_centre(im, W / 2, 26, "TWO BANNERS OPEN THIS DOOR", 'T');
            }
        }
    }
    if s.phase == Phase::Select {
        select_panel(im, s, tick, words);
    }
    if words && s.phase == Phase::Finale {
        let t = s.phase_t;
        let lines = ["EIGHT BANNERS FLY", "THE COURT IS LIT"];
        large(im, W / 2, 40, lines[usize::from(t > 120)], 'T', 2);
        if s.completed.is_some() && tick / 12 % 2 == 0 {
            text_centre(im, W / 2, 190, "E  RETURN TO THE KINGDOM", 'O');
        }
    }
    if words && s.phase == Phase::Play && s.completed.is_some() {
        text_centre(im, W / 2, 190, "E  RETURN TO THE KINGDOM", 'O');
    }
}

fn select_panel(im: &mut Img, s: &Session, tick: u32, words: bool) {
    let (x, y, w, h) = (84, 38, 216, if words { 138 } else { 100 });
    im.rect(x, y, w, h, 'k');
    im.frame(x, y, w, h, 'O');
    im.frame(x + 2, y + 2, w - 4, h - 4, 'o');
    if !words {
        // The terminal's own rows say the lines; the box is all this draws.
        return;
    }
    text_centre(im, W / 2, y + 8, "MOONWELL MOUNTAIN", 'T');
    for (n, (name, banner)) in BOARD.iter().enumerate() {
        let ly = y + 26 + n as i32 * 17;
        let on = usize::from(s.cursor) == n;
        let raised = s.banners & (1 << banner) != 0;
        if on {
            im.rect(x + 8, ly - 3, w - 16, 14, 'p');
            if tick / 6 % 2 == 0 {
                im.text(x + 12, ly, ">", 'T');
            }
        }
        let locked = n == 5 && s.doors & 2 == 0;
        let ink = if locked { 'g' } else if on { 'W' } else { 'h' };
        im.text(x + 24, ly, name, ink);
        // The banner it earns: lit when raised.
        let bx = x + w - 30;
        if raised {
            im.rect(bx, ly - 2, 9, 11, banner_ink(usize::from(*banner)));
            im.frame(bx, ly - 2, 9, 11, 'k');
        } else {
            im.frame(bx, ly - 2, 9, 11, 'g');
        }
        if locked {
            let need = format!("{}/{}", s.door_count(), SUMMIT_BANNERS);
            im.text(bx - 36, ly, &need, 'g');
        }
    }
    let left = (rules::session::SELECT_TICKS.saturating_sub(s.phase_t)) / HZ;
    text_centre(im, W / 2, y + h - 12, &format!("W S CHOOSE  E GO  {left}"), 'h');
}

/// The door on the kingdom map (art slot `bannerpeak.door`): a pole gate with
/// eight pennons on its bar, dull until the eight have flown once, the name
/// over it and E under it. Drawn only while its tile is in view.
pub(super) fn kingdom(im: &mut Img, run: &Run, origin: (i32, i32)) {
    let (x, y) = (
        rules::ENTRY.0 * 16 + 8 - origin.0,
        rules::ENTRY.1 * 16 + 8 - origin.1,
    );
    let name = rules::NAME.to_uppercase();
    let half = text_width(&name) / 2;
    if x < -half - 14 || x > W + 8 + half || !(-30..H + 24).contains(&y) {
        return;
    }
    let flown = run.home.roundtable[rules::SLOT] & rules::BEGUN != 0;
    // Two posts and a bar between them.
    im.rect(x - 14, y - 10, 3, 18, 'P');
    im.rect(x + 12, y - 10, 3, 18, 'P');
    im.rect(x - 15, y - 12, 31, 3, 'P');
    im.frame(x - 15, y - 12, 31, 3, 'k');
    // Eight pennons, each its own colour once the mountain has been won.
    for n in 0..8 {
        let ink = if flown { banner_ink(n) } else { 'g' };
        let px = x - 12 + n as i32 * 3 + 1;
        im.rect(px, y - 9, 2, 5 + (n as i32 % 2), ink);
    }
    // The way in, a dark door between the posts.
    im.rect(x - 7, y - 2, 15, 10, 'k');
    text_centre(im, x, y - 25, &name, 'H');
    text_centre(im, x, y + 11, "E", LOOK.accent);
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__bannerpeak__tests.rs"]
mod tests;
