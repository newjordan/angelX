//! Where Griffinwing's words sit in the 384 by 224 picture. The HUD's plates
//! each have a box that `hud.rs` both draws and reports, and the floating
//! labels (the wing's names, a foe's name, the Wyrm's, a gate's, a bolt's
//! "REFLECT") are laid out here by a placer that keeps every one of them in
//! the frame, off every other label, off the reticle and its brackets and off
//! the HUD. The drawing and the tests ask the same questions of the same
//! boxes, so a rule asserted about a box is a rule about what is on screen.
use super::hud::{self, Ctx};
use crate::render::overworld::ink::text_width;

pub const W: i32 = 384;
pub const H: i32 = 224;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Box2 {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Box2 {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Box2 {
        Box2 { x, y, w, h }
    }

    pub fn hits(&self, o: &Box2) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    pub fn within(&self, o: &Box2) -> bool {
        self.x >= o.x
            && self.y >= o.y
            && self.x + self.w <= o.x + o.w
            && self.y + self.h <= o.y + o.h
    }

    pub fn grow(&self, n: i32) -> Box2 {
        Box2::new(self.x - n, self.y - n, self.w + 2 * n, self.h + 2 * n)
    }

    pub fn union(&self, o: &Box2) -> Box2 {
        let (x0, y0) = (self.x.min(o.x), self.y.min(o.y));
        let (x1, y1) = (
            (self.x + self.w).max(o.x + o.w),
            (self.y + self.h).max(o.y + o.h),
        );
        Box2::new(x0, y0, x1 - x0, y1 - y0)
    }
}

pub const FRAME: Box2 = Box2::new(0, 0, W, H);

pub fn plate_of(x: i32, y: i32, text: &str) -> Box2 {
    Box2::new(x - 1, y - 1, text_width(text) + 2, 9)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    Moment,
    Craft,
    Foe,
    Thing,
}

#[derive(Clone, Debug)]
pub struct Want {
    pub rank: Rank,
    pub text: String,
    pub ink: char,
    pub x: i32,
    pub y: i32,
    pub near: i32,
    pub reach: i32,
}

#[derive(Clone, Debug)]
pub struct Placed {
    pub text: String,
    pub ink: char,
    pub at: (i32, i32),
    pub plate: Box2,
}

pub struct Placer {
    taken: Vec<Box2>,
    pub placed: Vec<Placed>,
    pub dropped: usize,
}

impl Placer {
    pub fn new(taken: Vec<Box2>) -> Placer {
        Placer {
            taken,
            placed: Vec::new(),
            dropped: 0,
        }
    }

    fn free(&self, b: &Box2) -> bool {
        b.within(&FRAME.grow(-1)) && !self.taken.iter().any(|t| t.hits(&b.grow(1)))
    }

    pub fn place(&mut self, want: &Want) -> bool {
        let w = text_width(&want.text);
        let home = (want.x - w / 2, want.y);
        let side = w / 2 + 4;
        let mut tries: Vec<(i32, i32)> = Vec::new();
        for row in 0..=6 {
            let up = row * 10;
            tries.push((home.0, home.1 - up));
            tries.push((home.0 + side, home.1 - up));
            tries.push((home.0 - side, home.1 - up));
        }
        for row in 1..=3 {
            let down = 8 + row * 10;
            tries.push((home.0, home.1 + down));
            tries.push((home.0 + side, home.1 + down));
            tries.push((home.0 - side, home.1 + down));
        }
        for (x, y) in tries {
            if (x - home.0).abs() > want.reach + side || (y - home.1).abs() > want.reach {
                continue;
            }
            let x = x.clamp(2, W - 2 - w);
            let plate = plate_of(x, y, &want.text);
            if self.free(&plate) {
                self.taken.push(plate);
                self.placed.push(Placed {
                    text: want.text.clone(),
                    ink: want.ink,
                    at: (x, y),
                    plate,
                });
                return true;
            }
        }
        self.dropped += 1;
        false
    }
}

pub fn plan(c: &Ctx) -> Placer {
    let mut taken: Vec<Box2> = hud::boxes(c).into_iter().map(|(_, b)| b).collect();
    taken.extend(hud::reticle_boxes(c));
    taken.extend(hud::bar_boxes(c));
    let mut placer = Placer::new(taken);
    let mut wants = hud::wants(c);
    wants.sort_by_key(|w| (w.rank, w.near));
    for w in &wants {
        placer.place(w);
    }
    placer
}
