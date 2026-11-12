//! The continuous line, drawn flat: a tile of rollerway per piece and way,
//! the swape's arm, a good riding the rollers and the blinking mark over a
//! jam. 16 px a tile, a pure function of its arguments, shared like the rest
//! of the kit. The ways are counted as a knight's face is: 0 north, 1 east,
//! 2 south, 3 west.
//!
//! Every piece is drawn once facing east and turned: a [`Pen`] puts the same
//! pixel where the turned tile wants it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::{FRAMES, Load, State};
use crate::kingdom::Piece;
use crate::render::overworld::ink::{Img, hash};

/// The pictures are sixteen wide; a house or a trestle rises over its tile.
pub const TILE: i32 = 16;
/// Pixels a splitter house, a merge or a gate stands above its tile, and a swape's post.
pub const RISE: i32 = 6;
/// Slots a tile holds, as the sim counts them.
pub const SLOTS: usize = 3;

type Made<K> = OnceLock<Mutex<HashMap<K, Arc<Img>>>>;

fn once<K: std::hash::Hash + Eq + Copy>(memory: &'static Made<K>, key: K, make: impl FnOnce() -> Img) -> Arc<Img> {
    let memory = memory.get_or_init(Default::default);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let made = Arc::new(make());
    if let Ok(mut m) = memory.lock() {
        m.insert(key, made.clone());
    }
    made
}

/// Puts pixels of a tile drawn facing east where the tile turned to `dir` wants them. The picture is
/// `h` tall and the tile is its bottom sixteen rows.
struct Pen {
    im: Img,
    dir: u8,
    rise: i32,
}

impl Pen {
    fn new(dir: u8, rise: i32) -> Pen {
        Pen { im: Img::new(TILE, TILE + rise), dir: dir % 4, rise }
    }

    /// `(x, y)` of the tile facing east, `up` pixels off the ground.
    fn at(&self, x: i32, y: i32, up: i32) -> (i32, i32) {
        let (tx, ty) = match self.dir {
            1 => (x, y),
            2 => (TILE - 1 - y, x),
            3 => (TILE - 1 - x, TILE - 1 - y),
            _ => (y, TILE - 1 - x),
        };
        (tx, ty + self.rise - up)
    }

    fn put(&mut self, x: i32, y: i32, ch: char) {
        self.lift(x, y, 0, ch);
    }

    fn lift(&mut self, x: i32, y: i32, up: i32, ch: char) {
        let (px, py) = self.at(x, y, up);
        self.im.put(px, py, ch);
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, up: i32, ch: char) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.lift(xx, yy, up, ch);
            }
        }
    }
}

/// A strip of log rollers along x from `x0` to `x1` (inclusive) between `y0` and `y1`, turning by `phase`,
/// `up` pixels off the ground.
fn strip(p: &mut Pen, x0: i32, x1: i32, y0: i32, y1: i32, up: i32, phase: i32) {
    // The side beams.
    for x in x0..=x1 {
        for y in [y0, y0 + 1, y1 - 1, y1] {
            let ch = if y == y0 || y == y1 { 'b' } else { 'P' };
            p.lift(x, y, up, ch);
        }
    }
    // The logs: across the way, each four pixels, a highlight turning along.
    for x in x0..=x1 {
        let k = (x + phase).rem_euclid(4);
        let ch = ['t', 'O', 'o', 'B'][k as usize];
        for y in y0 + 2..y1 - 1 {
            p.lift(x, y, up, ch);
        }
    }
}

/// An arrowhead pointing east centred on `(x, y)`.
fn chevron(p: &mut Pen, x: i32, y: i32, up: i32, ch: char) {
    for (dx, dy) in [(1, 0), (0, -1), (0, 1), (-1, -2), (-1, 2)] {
        p.lift(x + dx, y + dy, up, ch);
    }
}

fn stub(p: &mut Pen, x: i32, y: i32, w: i32, h: i32, up: i32) {
    p.rect(x, y, w, h, up, 'B');
    p.rect(x + 1, y + 1, (w - 2).max(1), (h - 2).max(1), up, 'b');
}

/// One tile of rollerway of `piece` facing `dir`. `moving` turns the rollers, `crewed` stands the hand-chain's
/// labourer in it, `frame` is the eight-step beat.
pub fn piece_flat(piece: Piece, dir: u8, moving: bool, crewed: bool, frame: u32) -> Arc<Img> {
    static MADE: Made<(Piece, u8, bool, bool, u32)> = OnceLock::new();
    let moving_frame = if moving { frame % FRAMES } else { 0 };
    let key = (piece, dir % 4, moving, crewed, if piece == Piece::Chain && crewed { moving_frame } else { moving_frame % 4 });
    once(&MADE, key, || draw_piece(piece, dir % 4, moving, crewed, key.4))
}

fn draw_piece(piece: Piece, dir: u8, moving: bool, crewed: bool, frame: u32) -> Img {
    match piece {
        Piece::Roller => {
            let mut p = Pen::new(dir, 0);
            strip(&mut p, 0, 15, 2, 13, 0, if moving { frame as i32 } else { 0 });
            chevron(&mut p, 9, 8, 0, 'b');
            p.im
        }
        Piece::Chain => {
            let mut p = Pen::new(dir, RISE);
            // A trodden way: packed earth between two staked ropes.
            for x in 0..16 {
                for y in 3..13 {
                    let n = hash(x, y, 77) % 7;
                    p.put(x, y, if n == 0 { 'h' } else if n < 3 { 'P' } else { 'o' });
                }
                p.put(x, 3, 'B');
                p.put(x, 12, 'B');
            }
            for x in [0, 8] {
                p.lift(x, 3, 3, 'b');
                p.lift(x, 12, 3, 'b');
                p.rect(x, 3, 1, 1, 0, 'b');
            }
            chevron(&mut p, 12, 8, 0, 'B');
            if crewed {
                // The labourer, standing in the way, passing goods hand to hand: drawn upright, whichever
                // way the chain runs, his arms swinging between the two sides of it.
                let man = labourer(if moving { frame % FRAMES } else { 0 }, dir);
                let rise = p.rise;
                p.im.stamp(&man, 8 - man.w / 2, rise + 11 - man.h + 1);
            } else {
                // Nobody at it: a stool by the way.
                p.rect(7, 7, 3, 2, 2, 'O');
                p.rect(7, 7, 1, 1, 0, 'B');
                p.rect(9, 7, 1, 1, 0, 'B');
            }
            p.im
        }
        Piece::Splitter | Piece::Merge | Piece::Gate => {
            let mut p = Pen::new(dir, RISE);
            let (roof, ridge) = match piece {
                Piece::Splitter => (['R', 'r', 'p'], 'o'),
                Piece::Merge => (['U', 'u', 'S'], 'v'),
                _ => (['O', 'o', 'P'], 'W'),
            };
            // The ways in and out: stubs of the same bed as a roller, the arrows say which.
            let ins: &[(i32, i32, bool)] = match piece {
                Piece::Merge => &[(0, 6, true), (6, 0, false), (6, 12, false)],
                _ => &[(0, 6, true)],
            };
            let outs: &[(i32, i32)] = match piece {
                Piece::Merge => &[(12, 6)],
                _ => &[(12, 6), (6, 0), (6, 12)],
            };
            for &(x, y, along) in ins {
                if along {
                    strip(&mut p, 0, 4, y - 1, y + 4, 0, 0);
                } else {
                    stub(&mut p, x, y, 5, 4, 0);
                    p.rect(x + 1, y + 1, 3, 2, 0, 'T');
                }
            }
            for &(x, y) in outs {
                if x == 12 {
                    strip(&mut p, 11, 15, y - 1, y + 4, 0, 0);
                } else {
                    stub(&mut p, x, y, 5, 4, 0);
                    p.rect(x + 1, y + 1, 3, 2, 0, 'T');
                }
            }
            // The house: walls, a pitched roof of boards, the ridge along the way.
            p.rect(3, 3, 10, 10, 0, 'B');
            p.rect(4, 4, 8, 8, 0, 'o');
            for x in 3..13_i32 {
                for y in 3..13_i32 {
                    let up = 2 + ((6 - (y - 8).abs()).max(0) / 2);
                    let n = hash(x, y, 91) % 5;
                    let ch = if (y - 8).abs() < 1 { ridge } else if n == 0 { roof[1] } else if (x + y) % 2 == 0 { roof[0] } else { roof[1] };
                    if x > 3 && x < 12 && y > 3 && y < 12 {
                        p.lift(x, y, up, if (y - 8).abs() >= 3 { roof[2] } else { ch });
                    }
                }
            }
            // The mark of the kind.
            match piece {
                Piece::Splitter => {
                    // A wedge parting the flow in three.
                    chevron(&mut p, 9, 6, 6, 'T');
                    chevron(&mut p, 9, 10, 6, 'T');
                }
                Piece::Merge => {
                    chevron(&mut p, 9, 8, 6, 'W');
                    p.rect(5, 5, 1, 6, 6, 'v');
                }
                _ => {
                    // The gate's flap and its bar: front first.
                    p.rect(10, 5, 1, 7, 6, 'W');
                    p.rect(8, 7, 3, 2, 6, 'T');
                    chevron(&mut p, 9, 8, 6, 'b');
                }
            }
            p.im
        }
        Piece::Trestle => {
            // The raised line runs the way it faces, on a trestle; the one under it a quarter turn right.
            let mut p = Pen::new(dir, 4);
            let phase = if moving { frame as i32 } else { 0 };
            // The line under: south by the canonical way (the right of east).
            for y in 0..16 {
                let k = (y + phase).rem_euclid(4);
                for x in 4..12 {
                    let ch = if x == 4 || x == 11 { 'b' } else if x == 5 || x == 10 { 'P' } else { ['t', 'O', 'o', 'B'][k as usize] };
                    p.put(x, y, ch);
                }
            }
            // Trestle legs at the near and far side, and the cross-beams.
            for x in [3, 12] {
                p.rect(x, 2, 1, 2, 0, 'B');
                p.rect(x, 12, 1, 2, 0, 'B');
                p.rect(x, 2, 1, 12, 1, 'I');
            }
            // The raised line, four pixels off the ground.
            strip(&mut p, 0, 15, 4, 11, 4, phase);
            chevron(&mut p, 9, 7, 4, 'b');
            // And the way of the line below, a small arrow on its bed.
            p.rect(7, 13, 2, 2, 0, 'T');
            p.rect(6, 14, 4, 1, 0, 'T');
            p.im
        }
    }
}

/// A labourer at the hand-chain, seven by eleven, facing the viewer, his arms out toward the way the goods
/// come from and go to.
fn labourer(step: u32, dir: u8) -> Img {
    let mut m = Img::new(9, 11);
    // He bobs a pixel on the odd steps, as a man does passing a weight on.
    let bob = (step % 2) as i32;
    // Head, hair, face.
    m.rect(3, 0 - bob + 1, 3, 1, 'B');
    m.rect(3, 1 - bob + 1, 3, 2, 'c');
    m.put(3, 1 - bob + 1, 'B');
    // Tunic and belt.
    m.rect(3, 3 - bob + 1, 3, 4, 'u');
    m.rect(3, 3 - bob + 1, 3, 1, 'U');
    m.rect(3, 5 - bob + 1, 3, 1, 'b');
    // Legs and boots.
    m.rect(3, 7, 1, 3, 'B');
    m.rect(5, 7, 1, 3, 'B');
    m.rect(3, 10, 1, 1, 'k');
    m.rect(5, 10, 1, 1, 'k');
    // Arms: the hands pass a good from one side to the other, a quarter of the round each.
    let along_x = dir % 2 == 1;
    let swing = (step / 2) % 2 == 0;
    let (hand_a, hand_b) = if along_x { ((0, 4), (8, 4)) } else { ((1, 5), (7, 5)) };
    let (ax, ay) = if swing { hand_a } else { hand_b };
    m.line(4, 5 - bob, ax, ay - bob, 'c');
    let (bx, by) = if swing { hand_b } else { hand_a };
    m.line(4, 5 - bob, bx, by - 1 - bob, 'c');
    // The good in the hand that is out in front.
    m.rect(ax.min(7), ay - 1 - bob, 2, 2, 'O');
    m
}

/// A swape: the post and its pivot arm, `facing` the way the goods go (the arm swings from behind it to its
/// front). Working it swings through the eight frames; starved and backed-up flags stand on the post.
pub fn swape_flat(facing: u8, state: State, frame: u32) -> Arc<Img> {
    static MADE: Made<(u8, State, u32)> = OnceLock::new();
    let frame = if state == State::Working { frame % FRAMES } else { 0 };
    once(&MADE, (facing % 4, state, frame), || draw_swape(facing % 4, state, frame))
}

fn draw_swape(dir: u8, state: State, frame: u32) -> Img {
    let mut p = Pen::new(dir, RISE + 6);
    let building = if let State::Building(step) = state { Some(step) } else { None };
    // The footing: a low platform of boards round the post.
    p.rect(3, 3, 10, 10, 0, 'B');
    p.rect(4, 4, 8, 8, 0, 'P');
    for y in (5..12).step_by(2) {
        p.rect(4, y, 8, 1, 0, 'o');
    }
    if building == Some(0) {
        // Pegged out.
        for (x, y) in [(3, 3), (12, 3), (3, 12), (12, 12)] {
            p.rect(x, y, 1, 1, 3, 'O');
        }
        return p.im;
    }
    // The post, rising from the middle of the footing.
    let top = 9;
    for up in 0..top {
        p.lift(7, 7, up, if up % 3 == 2 { 'B' } else { 'I' });
        p.lift(8, 7, up, 'p');
    }
    p.rect(6, 6, 4, 2, top, 'b');
    if let Some(s) = building.filter(|s| *s < 3) {
        if s >= 2 {
            // The arm lying across the footing, not yet hung.
            p.rect(2, 8, 12, 1, 1, 'O');
        }
        return p.im;
    }
    // The arm swings round the post between its two ends: over what it lifts from (behind it) and what it
    // lifts to (in front), by the side. Eight steps a round; at rest it hangs over the front.
    use std::f32::consts::PI;
    let (phi, lowered) = match state {
        State::Working => {
            let k = (frame % FRAMES) as f32;
            // 0: over the back, basket down; 1-3 swinging over; 4: over the front, down; 5-7 swinging back.
            let t = if k <= 4.0 { k / 4.0 } else { (8.0 - k) / 4.0 };
            (PI * (1.0 - t), k == 0.0 || k == 4.0 || k == 1.0 && false)
        }
        State::BackedUp(_) => (0.0, false),
        State::Starved(_) => (PI, false),
        _ => (PI * 0.5, false),
    };
    let reach = 6.5;
    let (px, py) = (7.5_f32, 7.5_f32);
    let tip = (px + reach * phi.cos(), py + reach * phi.sin() * 0.9);
    let heel = (px - 3.0 * phi.cos(), py - 3.0 * phi.sin() * 0.9);
    let a_up = top + 1;
    let (sx, sy) = p.at(7, 7, a_up);
    let (tx, ty) = p.at(tip.0.round() as i32, tip.1.round() as i32, a_up);
    let (hx, hy) = p.at(heel.0.round() as i32, heel.1.round() as i32, a_up);
    p.im.line(hx, hy, tx, ty, 'T');
    p.im.line(hx, hy + 1, tx, ty + 1, 'O');
    p.im.line(hx, hy + 2, tx, ty + 2, 'B');
    // The counterweight at the heel.
    p.im.rect(hx - 2, hy, 4, 4, 'G');
    p.im.rect(hx - 2, hy, 4, 1, 'h');
    // The hook's rope and its basket, hanging a little lower when it is down to take or leave a good.
    let drop = if lowered { 5 } else { 3 };
    p.im.line(tx, ty + 2, tx, ty + drop, 'b');
    p.im.rect(tx - 2, ty + drop, 5, 3, 'B');
    p.im.rect(tx - 2, ty + drop, 5, 1, 'o');
    p.im.rect(tx - 2, ty + drop + 2, 5, 1, 'b');
    let carrying = state == State::Working && (1..=3).contains(&(frame % FRAMES));
    if carrying {
        p.im.rect(tx - 1, ty + drop - 2, 3, 2, 'w');
        p.im.rect(tx - 1, ty + drop - 1, 3, 1, 'H');
    }
    // A flag on the post: amber when it has nowhere to put what it lifts, red when it has nothing to lift.
    match state {
        State::BackedUp(_) => {
            p.im.rect(sx + 1, sy - 4, 3, 2, '4');
            p.im.put(sx, sy - 4, 'b');
        }
        State::Starved(_) => {
            p.im.rect(sx + 1, sy - 4, 3, 2, '7');
            p.im.put(sx, sy - 4, 'b');
        }
        _ => {}
    }
    p.im
}

/// The light, middle and dark inks of a good's crate: each family of goods its own colour, so that stone,
/// lime, planks, bread and iron are told apart on a moving line.
fn tones(load: Load) -> (char, char, char) {
    match load {
        Load::Stone => ('h', 'J', 'G'),
        Load::Rubble => ('J', 'G', 'g'),
        Load::Ashlar => ('H', 'i', 'h'),
        Load::Lime => ('w', 'H', 'i'),
        Load::Mortar => ('$', 'o', 'B'),
        Load::Sand => ('T', 't', 'o'),
        Load::Clay => ('R', 'r', 'B'),
        Load::Brick => ('R', '7', 'p'),
        Load::Timber => ('o', 'B', 'I'),
        Load::Planks => ('T', 'O', 'o'),
        Load::Charcoal => ('G', 'g', 'K'),
        Load::Coal => ('J', 'X', 'K'),
        Load::Ore => ('R', 'p', 'b'),
        Load::Iron => ('V', 'U', 'u'),
        Load::Lead => ('U', 'u', 'S'),
        Load::Slate => ('u', 'S', 'x'),
        Load::Grain => ('6', '5', '4'),
        Load::Flour => ('9', 'c', '$'),
        Load::Bread => ('t', 'R', 'B'),
        Load::Ale => ('@', 'R', 'b'),
        Load::Hemp => ('y', 'M', 'l'),
        Load::Rope => ('T', 'O', 'P'),
        Load::Hides => ('O', 'r', 'B'),
        Load::Wool => ('9', 'h', 'G'),
        Load::Cloth => ('^', ']', ')'),
        Load::Water => ('3', 'z', 'Q'),
        Load::Glass => ('w', '3', 'z'),
        Load::Crate => ('O', 'r', 'b'),
    }
}

/// A good riding the rollers, six by four: a little crate in its colours, lit from above, dark under it.
pub fn good_flat(load: Load) -> Arc<Img> {
    static MADE: Made<Load> = OnceLock::new();
    once(&MADE, load, || {
        let (light, mid, dark) = tones(load);
        let mut im = Img::new(6, 4);
        im.rect(1, 0, 4, 1, light);
        im.rect(0, 1, 6, 1, mid);
        im.rect(0, 2, 6, 1, dark);
        im.put(0, 1, 'k');
        im.put(5, 1, 'k');
        im.put(0, 2, 'k');
        im.put(5, 2, 'k');
        im.rect(1, 3, 4, 1, 'k');
        im
    })
}

/// The mark over the head of a jam: a red ring and a bang, blinking.
pub fn jam_flat(frame: u32) -> Arc<Img> {
    static MADE: Made<bool> = OnceLock::new();
    let on = (frame / 4) % 2 == 0;
    once(&MADE, on, || {
        let mut im = Img::new(11, 11);
        im.ellipse(5.5, 5.5, 5.0, 5.0, if on { '7' } else { 'a' });
        im.ellipse(5.5, 5.5, 3.6, 3.6, 'k');
        im.rect(5, 2, 1, 4, 'w');
        im.put(5, 7, 'w');
        im
    })
}

/// Where in its tile (pixels from its top-left corner, the way the tile faces counted in) the middle of a
/// good in slot `slot` of a lane running `dir` stands, with `back` slots' worth of the way taken off: the
/// drawing lags the sim by a second and the stream slides forward through it.
pub fn slot_at(dir: u8, slot: usize, back: f32) -> (f32, f32) {
    let along = (slot as f32 + 0.5 - back) * TILE as f32 / SLOTS as f32;
    match dir % 4 {
        1 => (along, 8.0),
        2 => (8.0, along),
        3 => (TILE as f32 - along, 8.0),
        _ => (8.0, TILE as f32 - along),
    }
}

/// A bay on the rail: a plank platform on the side that faces the building it serves (`side`, counted as a
/// knight's face is), a post with a lantern that is lit while a wagon is working it.
pub fn bay_flat(side: u8, held: bool, frame: u32) -> Arc<Img> {
    static MADE: Made<(u8, bool, bool)> = OnceLock::new();
    let lit = held && (frame / 2) % 2 == 0;
    once(&MADE, (side % 4, held, lit), || {
        let mut p = Pen::new(side % 4, 6);
        // The deck, along the side of the rail nearest the building, planks across.
        for y in 2..14 {
            for x in 10..16 {
                p.put(x, y, if (y % 3) == 0 { 'B' } else if x == 10 { 'P' } else { 'o' });
            }
        }
        p.rect(10, 2, 6, 1, 0, 'B');
        p.rect(10, 13, 6, 1, 0, 'B');
        // Post and lantern at the end of the deck.
        for up in 0..6 {
            p.lift(10, 2, up, 'I');
        }
        p.lift(10, 2, 6, if lit { '6' } else if held { '4' } else { 'T' });
        p.lift(11, 2, 6, if held { '4' } else { 'o' });
        p.im
    })
}

/// A switch's lever: a short post and a brass handle on the ground beside the rails where they part.
pub fn switch_flat() -> Arc<Img> {
    static MADE: Made<()> = OnceLock::new();
    once(&MADE, (), || {
        let mut im = Img::new(TILE, TILE + 4);
        im.rect(11, 18, 3, 2, 'g');
        im.rect(12, 14, 1, 5, 'I');
        im.rect(11, 12, 3, 2, 'T');
        im.put(12, 11, 'W');
        im
    })
}

/// A small amber tag over a wagon that waits for the track; the red ring of a jam is [`jam_flat`].
pub fn waiting_flat(frame: u32) -> Arc<Img> {
    static MADE: Made<bool> = OnceLock::new();
    let on = (frame / 3) % 2 == 0;
    once(&MADE, on, || {
        let mut im = Img::new(7, 5);
        im.rect(0, 0, 7, 5, 'k');
        im.rect(1, 1, 5, 3, if on { '4' } else { 'a' });
        im.rect(3, 4, 1, 1, 'k');
        im
    })
}

#[cfg(test)]
#[path = "../../../../tests/render/rollers_kit__tests.rs"]
mod tests;
