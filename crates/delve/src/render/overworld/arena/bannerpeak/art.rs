//! The inked sprites: the knights, the snuffers, the loaf, from `art_data` (made by
//! `assets/bannerpeak/art/emit.py`). A frame is cropped to its ink with its offset from the
//! feet; here it gets its seat's four tones, a flip for the way it faces, and the squash and
//! stretch a body wants in the air. Frames are cached per thread: a seat's colours and a facing
//! are made once.
use super::super::super::ink::Img;
use super::art_data::{BIG, FOES, Frame, SMALL, Set};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// A frame ready to stamp: its picture and where its top-left sits from the feet
/// (the bottom centre of the body's box).
pub(super) struct Sprite {
    pub img: Img,
    pub dx: i32,
    pub dy: i32,
}

/// A seat's four tones, shade to spec: the plume, the tabard and the shield's field.
pub(super) const SEAT_TONES: [[char; 4]; 6] = [
    ['q', 'Q', 'z', 'V'],
    ['?', ';', '%', '+'],
    ['E', 'm', 'y', 'Y'],
    ['p', 'R', 'O', 't'],
    [')', '[', ']', '^'],
    ['0', '1', '2', '3'],
];

pub(super) fn tones(seat: u32) -> [char; 4] {
    SEAT_TONES[(seat.max(1) as usize - 1) % 6]
}

fn find(sets: &'static [Set], name: &str) -> Option<&'static Set> {
    sets.iter().find(|s| s.name == name)
}

type Key = (usize, u8, bool);

thread_local! {
    static CACHE: RefCell<HashMap<Key, Rc<Sprite>>> = RefCell::new(HashMap::new());
}

/// Frame `index` of `name` in `sets`, in the tones of `seat`, mirrored when `flip`.
fn get(sets: &'static [Set], name: &str, index: usize, seat: u32, flip: bool) -> Option<Rc<Sprite>> {
    let set = find(sets, name)?;
    let frame: &'static Frame = set.frames.get(index % set.frames.len().max(1))?;
    let key = (frame as *const Frame as usize, seat.min(255) as u8, flip);
    if let Some(hit) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Some(hit);
    }
    let mut img = Img::from_rows(frame.rows);
    if seat > 0 {
        let t = tones(seat);
        img = img.recolor(&[('1', t[0]), ('2', t[1]), ('3', t[2]), ('4', t[3])]);
    }
    let (mut dx, dy) = (i32::from(frame.dx), i32::from(frame.dy));
    if flip {
        dx = -(dx + img.w);
        img = img.flip_h();
    }
    let sprite = Rc::new(Sprite { img, dx, dy });
    CACHE.with(|c| c.borrow_mut().insert(key, sprite.clone()));
    Some(sprite)
}

/// A knight's frame. `big` is the cuirassed, taller knight (the flame knight too).
pub(super) fn knight(big: bool, name: &str, index: usize, seat: u32, flip: bool) -> Option<Rc<Sprite>> {
    get(if big { BIG } else { SMALL }, name, index, seat, flip)
}

/// A foe's, the cap's or the loaf's frame (no seat).
pub(super) fn foe(name: &str, index: usize, flip: bool) -> Option<Rc<Sprite>> {
    get(FOES, name, index, 0, flip)
}

/// How many frames `name` has among the knights.
pub(super) fn frames(big: bool, name: &str) -> usize {
    find(if big { BIG } else { SMALL }, name).map_or(0, |s| s.frames.len())
}

/// The sprite squashed or stretched about its feet: `sx` and `sy` are the share of its width
/// and height it keeps. Nearest neighbour, so every pixel stays a pixel; the feet stay put.
pub(super) fn stretched(s: &Sprite, sx: f32, sy: f32) -> Sprite {
    let (w, h) = (s.img.w, s.img.h);
    let (nw, nh) = (((w as f32 * sx).round() as i32).max(1), ((h as f32 * sy).round() as i32).max(1));
    if nw == w && nh == h {
        return Sprite { img: s.img.clone(), dx: s.dx, dy: s.dy };
    }
    let mut img = Img::new(nw, nh);
    for y in 0..nh {
        let sy_ = (y * h / nh).min(h - 1);
        for x in 0..nw {
            let sx_ = (x * w / nw).min(w - 1);
            if let Some(c) = s.img.get(sx_, sy_) {
                img.set(x, y, c);
            }
        }
    }
    let bottom = s.dy + h;
    let nb = (bottom as f32 * sy).round() as i32;
    Sprite { img, dx: (s.dx as f32 * sx).round() as i32, dy: nb - nh }
}

/// `s` stamped with its feet at `(x, y)`.
pub(super) fn stamp(im: &mut Img, s: &Sprite, x: i32, y: i32) {
    im.stamp(&s.img, x + s.dx, y + s.dy);
}
