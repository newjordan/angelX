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

/// A rectangle in the picture, in dots.
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

    /// Whether the two share a dot.
    pub fn hits(&self, o: &Box2) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    /// Whether this lies wholly inside `o`.
    pub fn within(&self, o: &Box2) -> bool {
        self.x >= o.x
            && self.y >= o.y
            && self.x + self.w <= o.x + o.w
            && self.y + self.h <= o.y + o.h
    }

    /// Grown by `n` dots on every side.
    pub fn grow(&self, n: i32) -> Box2 {
        Box2::new(self.x - n, self.y - n, self.w + 2 * n, self.h + 2 * n)
    }

    /// The smallest box holding both.
    pub fn union(&self, o: &Box2) -> Box2 {
        let (x0, y0) = (self.x.min(o.x), self.y.min(o.y));
        let (x1, y1) = (
            (self.x + self.w).max(o.x + o.w),
            (self.y + self.h).max(o.y + o.h),
        );
        Box2::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// The whole picture.
pub const FRAME: Box2 = Box2::new(0, 0, W, H);

/// The plate a line of words sits on: a dot round it, nine tall.
pub fn plate_of(x: i32, y: i32, text: &str) -> Box2 {
    Box2::new(x - 1, y - 1, text_width(text) + 2, 9)
}

/// What a label is for; the placer lays the most telling first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    /// A word the moment is about: a bolt turned, a gap, a roar.
    Moment,
    /// The Wyrm, a rider, another knight.
    Craft,
    /// A foe's name.
    Foe,
    /// A gate, an arch, the beacon.
    Thing,
}

/// A label wanted near a point: `x` is its centre, `y` the top of its words
/// where it would sit were nothing in the way.
#[derive(Clone, Debug)]
pub struct Want {
    pub rank: Rank,
    pub text: String,
    pub ink: char,
    pub x: i32,
    pub y: i32,
    /// Which comes first within a rank (the lower: a nearer craft).
    pub near: i32,
    /// How near the point it may be moved, in dots (the ring and the side).
    pub reach: i32,
}

/// A label that found its place: the words' corner and the plate under them.
#[derive(Clone, Debug)]
pub struct Placed {
    pub text: String,
    pub ink: char,
    pub at: (i32, i32),
    pub plate: Box2,
}

/// The ground the labels may not cover: the HUD, the reticle, each other.
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

    /// Lay one label: where it wants to be, else a step up (a row at a time),
    /// to the side of that, else under the point; the nearest free place within
    /// its reach. A label with no room is left out, never laid on another.
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
            // Keep the words in the frame by sliding them sideways.
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

/// Every floating label for this seat's picture, laid out: the wants in
/// order of rank, nearest first, each placed clear of the HUD, the reticle and
/// the labels before it.
pub fn plan(c: &Ctx) -> Placer {
    let mut taken: Vec<Box2> = hud::boxes(c).into_iter().map(|(_, b)| b).collect();
    taken.extend(hud::reticle_boxes(c));
    taken.extend(hud::bar_boxes(c));
    let mut placer = Placer::new(taken);
    let mut wants = hud::wants(c);
    // Stable: what ties keeps the order the stage listed it in, so the
    // picture does not shuffle from tick to tick.
    wants.sort_by_key(|w| (w.rank, w.near));
    for w in &wants {
        placer.place(w);
    }
    placer
}
