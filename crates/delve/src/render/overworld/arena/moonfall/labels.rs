//! Where Moonfall's words stand over the town. A building's plate, a
//! villager's name, a knight's seat number, the herald's cry and the bell's
//! banner are each a want with a home place; once the town is drawn,
//! `Layout::place` settles every want and `draw` paints them over the town
//! (after the sky's light, so they read at any hour).
//!
//! The policy, the same state always giving the same picture:
//!
//! 1. The HUD's rows (the Ledger's banner lines, the Final Hours box, the
//!    hold bar) are keep-outs no word is placed over, and a panel, card or
//!    the ritual's glass is a keep-out that also hides: a word that would
//!    touch one is left out, never half covered.
//! 2. Building and prop plates go first and keep their place. Only a HUD row
//!    or another plate can move one: its fixed short form (`abbreviation`)
//!    where it stands, then up or down a row, then aside, never over the
//!    picture it names; a plate with no such place is left out.
//! 3. The bell's banner takes a free row of the town view clear of the bell's
//!    picture, then the herald's cry takes a free spot, then each villager's
//!    name, then each knight's seat number (the viewer's own first): above
//!    the head where it is free, else stacked higher, beside the head, or
//!    under the feet, preferring a spot that covers no one's body.
//! 4. Every rectangle is shifted wholly inside the town view, never clipped.
//! 5. No two rectangles intersect, and a pixel of the town shows between any
//!    two, so words never read as one. A name, number or plate that finds no free
//!    spot is left out; the bell's banner and the herald's cry, which must
//!    show, take the least crowded spot clear of every panel.
use super::super::super::ink::{Img, Rgb, ink, text_width};
use super::{VIEW_H, VIEW_Y};

/// The frame the words live in: the width of the picture and the town view's
/// height (the clock bar owns the rows below it).
pub(super) const FRAME_W: i32 = 384;
pub(super) const FRAME_H: i32 = VIEW_Y + VIEW_H;

/// One row of words: a plate is nine pixels tall and a pixel apart.
const ROW: i32 = 10;

/// How much of a picture must be in view for its plate to show.
const SEEN: i32 = 12;

/// The least contrast (against the plate's black) a word's ink may have; an
/// ink under it is drawn as a pale word in a frame of that ink.
pub(super) const MIN_CONTRAST: f32 = 4.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub(super) const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    pub(super) fn right(self) -> i32 {
        self.x + self.w
    }

    pub(super) fn bottom(self) -> i32 {
        self.y + self.h
    }

    pub(super) fn hits(self, o: Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }

    fn overlap(self, o: Rect) -> i32 {
        let (w, h) = (self.right().min(o.right()) - self.x.max(o.x), self.bottom().min(o.bottom()) - self.y.max(o.y));
        if w > 0 && h > 0 { w * h } else { 0 }
    }

    /// It, a margin of `by` pixels wider on every side.
    pub(super) fn grown(self, by: i32) -> Rect {
        Rect::new(self.x - by, self.y - by, self.w + 2 * by, self.h + 2 * by)
    }

    fn moved(self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    fn union(self, o: Rect) -> Rect {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        Rect::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }

    /// Whether it lies wholly inside the town view.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn in_frame(self) -> bool {
        self.x >= 0 && self.y >= 0 && self.right() <= FRAME_W && self.bottom() <= FRAME_H
    }

    /// The shift that brings it wholly inside the town view.
    fn shift_in(self) -> (i32, i32) {
        let sx = if self.x < 0 { -self.x } else if self.right() > FRAME_W { FRAME_W - self.right() } else { 0 };
        let sy = if self.y < 0 { -self.y } else if self.bottom() > FRAME_H { FRAME_H - self.bottom() } else { 0 };
        (sx, sy)
    }

    /// It, shifted wholly inside the town view.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn fitted(self) -> Rect {
        let (sx, sy) = self.shift_in();
        self.moved(sx, sy)
    }
}

fn luminance(c: Rgb) -> f32 {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.039_28 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// The contrast of two colours, 1 (none) to 21.
pub(super) fn contrast_of(a: Rgb, b: Rgb) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The contrast of the ink `ch` against a plate's black.
pub(super) fn contrast(ch: char) -> f32 {
    match (ink(ch), ink('k')) {
        (Some(a), Some(k)) => contrast_of(a, k),
        _ => 1.0,
    }
}

/// The fixed short form of a long plate, used only when the whole name finds
/// no free place.
pub(super) fn abbreviation(name: &str) -> Option<&'static str> {
    Some(match name {
        "NORTH BELL TOWER" => "NORTH TOWER",
        "SOUTH BELL TOWER" => "SOUTH TOWER",
        "HERALD HOUSE" => "HERALD",
        "EAST GATE" => "GATE",
        _ => return None,
    })
}

/// One plate of words and where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Part {
    pub text: String,
    pub ink: char,
    /// The whole dark plate, its margin and edge included.
    pub rect: Rect,
    /// The words' top-left from the plate's.
    pub pad: (i32, i32),
    /// The ink of the plate's frame, when it has one.
    pub edge: Option<char>,
}

impl Part {
    /// Words on a dark plate with their top-left at `(x, y)`. An ink too dim
    /// against the black is drawn pale inside a frame of that ink.
    pub(super) fn plate(text: &str, ink: char, x: i32, y: i32) -> Part {
        let (ink, edge) = if contrast(ink) < MIN_CONTRAST { ('T', Some(ink)) } else { (ink, None) };
        Part { text: text.to_string(), ink, rect: Rect::new(x - 1, y - 1, text_width(text) + 2, 9), pad: (1, 1), edge }
    }

    /// Words in a framed box with its top-left at `(x, y)`.
    fn boxed(text: &str, ink: char, edge: char, x: i32, y: i32) -> Part {
        Part { text: text.to_string(), ink, rect: Rect::new(x, y, text_width(text) + 12, 15), pad: (6, 4), edge: Some(edge) }
    }

    fn moved(&self, dx: i32, dy: i32) -> Part {
        Part { rect: self.rect.moved(dx, dy), ..self.clone() }
    }
}

/// What a want names, in the order they are settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Plate,
    Banner,
    Speech,
    Folk,
    Knight,
}

struct Want {
    kind: Kind,
    prio: u32,
    parts: Vec<Part>,
    /// A fixed short form of the same words, centred on the same column.
    alt: Option<Vec<Part>>,
    /// What the words name, for placing a name beside or under it.
    body: Rect,
}

/// Every want settled, and the words left out for want of a place.
pub(super) struct Placement {
    pub placed: Vec<Placed>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub left_out: Vec<(Kind, String)>,
}

/// A want settled: its plates where they stand.
#[derive(Clone, Debug)]
pub(super) struct Placed {
    #[cfg_attr(not(test), allow(dead_code))]
    pub kind: Kind,
    pub parts: Vec<Part>,
    /// Where its whole words wanted to stand before anything moved them.
    #[cfg_attr(not(test), allow(dead_code))]
    pub home: Rect,
    /// What it names: a plate's picture, a name's or number's body.
    #[cfg_attr(not(test), allow(dead_code))]
    pub body: Rect,
}

#[derive(Default)]
pub(super) struct Layout {
    /// Rectangles the words keep out of; a modal one also hides a word that
    /// would touch it.
    keep: Vec<(Rect, bool)>,
    /// Pictures the bell's banner keeps off.
    art: Vec<Rect>,
    /// The bodies of villagers and knights, which a name prefers not to cover.
    bodies: Vec<Rect>,
    wants: Vec<Want>,
}

impl Layout {
    pub(super) fn keep(&mut self, r: Rect, modal: bool) {
        self.keep.push((r, modal));
    }

    pub(super) fn keeps(&self) -> Vec<(Rect, bool)> {
        self.keep.clone()
    }

    pub(super) fn art(&mut self, r: Rect) {
        self.art.push(r);
    }

    pub(super) fn body(&mut self, r: Rect) {
        self.bodies.push(r);
    }

    /// A building's or a prop's plate, its words centred on column `cx`, the
    /// top of the words at row `y`. `anchor` is the picture it names: with
    /// less than a tile of it inside the town view there is no plate.
    pub(super) fn plate(&mut self, cx: i32, y: i32, text: &str, ink: char, anchor: Rect) {
        let seen = (anchor.right().min(FRAME_W) - anchor.x.max(0), anchor.bottom().min(FRAME_H) - anchor.y.max(0));
        if seen.0 < SEEN || seen.1 < SEEN {
            return;
        }
        let at = |t: &str| vec![Part::plate(t, ink, cx - text_width(t) / 2, y)];
        let alt = abbreviation(text).map(at);
        let parts = at(text);
        self.wants.push(Want { kind: Kind::Plate, prio: 0, parts, alt, body: anchor });
    }

    /// A name or seat number over `body`, centred on `cx` at row `y`. `prio`
    /// orders those of a kind: the lowest keeps its home first.
    pub(super) fn tag(&mut self, kind: Kind, prio: u32, cx: i32, y: i32, text: &str, ink: char, body: Rect) {
        let parts = vec![Part::plate(text, ink, cx - text_width(text) / 2, y)];
        self.wants.push(Want { kind, prio, parts, alt: None, body });
    }

    /// Lines of speech over `body`, kept together.
    pub(super) fn speech(&mut self, parts: Vec<Part>, body: Rect) {
        if !parts.is_empty() {
            self.wants.push(Want { kind: Kind::Speech, prio: 0, parts, alt: None, body });
        }
    }

    /// A banner across the town view, in a frame of `edge`.
    pub(super) fn banner(&mut self, text: &str, ink: char, edge: char) {
        let part = Part::boxed(text, ink, edge, FRAME_W / 2 - (text_width(text) + 12) / 2, VIEW_Y + 16);
        let body = part.rect;
        self.wants.push(Want { kind: Kind::Banner, prio: 0, parts: vec![part], alt: None, body });
    }

    /// Settle every want.
    pub(super) fn place(mut self) -> Placement {
        let mut wants = std::mem::take(&mut self.wants);
        wants.sort_by_key(|w| (w.kind, w.prio));
        let mut taken: Vec<Rect> = Vec::new();
        let mut placement = Placement { placed: Vec::new(), left_out: Vec::new() };
        for want in &wants {
            let Some(parts) = self.settle(want, &taken) else {
                placement.left_out.push((want.kind, want.parts[0].text.clone()));
                continue;
            };
            taken.extend(parts.iter().map(|p| p.rect));
            placement.placed.push(Placed { kind: want.kind, parts, home: bounds(&want.parts), body: want.body });
        }
        placement
    }

    fn settle(&self, want: &Want, taken: &[Rect]) -> Option<Vec<Part>> {
        let variants: Vec<&Vec<Part>> = std::iter::once(&want.parts).chain(want.alt.iter()).collect();
        // The same moves for every form of the words: the whole name where it
        // stands, then its short form there, then each a row up or down.
        let moves = offsets(want.kind, &want.parts, want.body);
        let blocked = |spot: &[Part]| {
            spot.iter().any(|p| {
                let apart = p.rect.grown(1);
                self.keep.iter().any(|&(r, _)| apart.hits(r))
                    || taken.iter().any(|&r| apart.hits(r))
                    || (want.kind == Kind::Banner && self.art.iter().any(|&r| apart.hits(r)))
            })
        };
        // What a place should also keep off: a plate, the picture it names; a
        // name, number, cry or banner, the bodies of others.
        let covers = |spot: &[Part]| {
            spot.iter().any(|p| match want.kind {
                Kind::Plate => p.rect.hits(want.body),
                _ => self.bodies.iter().any(|&r| r != want.body && p.rect.hits(r)),
            })
        };
        for &(dx, dy) in &moves {
            for parts in &variants {
                let spot = fitted(parts, dx, dy);
                if !blocked(&spot) && !covers(&spot) {
                    return Some(spot);
                }
            }
        }
        match want.kind {
            // A plate that cannot keep clear of its picture stands at home
            // or not at all.
            Kind::Plate => {
                return variants.iter().map(|parts| fitted(parts, 0, 0)).find(|spot| !blocked(spot));
            }
            Kind::Folk | Kind::Knight => {
                return moves.iter().flat_map(|&(dx, dy)| variants.iter().map(move |parts| fitted(parts, dx, dy))).find(|spot| !blocked(spot));
            }
            Kind::Banner | Kind::Speech => {}
        }
        if let Some(spot) = moves.iter().map(|&(dx, dy)| fitted(&want.parts, dx, dy)).find(|spot| !blocked(spot)) {
            return Some(spot);
        }
        // No free place at all: the least crowded one, unless it would sit
        // under a panel.
        let crowd = |spot: &Vec<Part>| -> i32 {
            spot.iter()
                .map(|p| {
                    let over = |r: &Rect| p.rect.overlap(*r);
                    self.keep.iter().map(|(r, _)| over(r)).sum::<i32>() + taken.iter().map(over).sum::<i32>()
                })
                .sum()
        };
        moves
            .iter()
            .map(|&(dx, dy)| fitted(&want.parts, dx, dy))
            .filter(|s| !s.iter().any(|p| self.keep.iter().any(|&(r, modal)| modal && p.rect.grown(1).hits(r))))
            .min_by_key(crowd)
    }
}

/// The union of some plates.
pub(super) fn bounds(parts: &[Part]) -> Rect {
    parts.iter().skip(1).fold(parts[0].rect, |u, p| u.union(p.rect))
}

/// `parts` moved by `(dx, dy)` and then shifted wholly into the frame.
fn fitted(parts: &[Part], dx: i32, dy: i32) -> Vec<Part> {
    let (sx, sy) = bounds(parts).moved(dx, dy).shift_in();
    parts.iter().map(|p| p.moved(dx + sx, dy + sy)).collect()
}

/// The moves a want tries, best first, from where its words stand.
fn offsets(kind: Kind, parts: &[Part], body: Rect) -> Vec<(i32, i32)> {
    let u = bounds(parts);
    let mut at: Vec<(i32, i32)> = Vec::new();
    match kind {
        // Up or down a row, then aside by half and by a whole plate.
        Kind::Plate => {
            let half = u.w / 2 + 1;
            for dx in [0, -half, half, -2 * half, 2 * half] {
                for dy in [0, -ROW, ROW, -2 * ROW, 2 * ROW, -3 * ROW, 3 * ROW] {
                    at.push((dx, dy));
                }
            }
            at.sort_by_key(|&(dx, dy)| dx.abs() + 2 * dy.abs());
        }
        // The free rows of the town view, from the one under the sky out.
        Kind::Banner => {
            let from = VIEW_Y + 16;
            let mut rows: Vec<i32> = (VIEW_Y + 4..=FRAME_H - u.h - 2).collect();
            rows.sort_by_key(|&y| (y - from).abs());
            at.extend(rows.into_iter().map(|y| (0, y - u.y)));
        }
        Kind::Speech => {
            let below = body.bottom() + 2 - u.y;
            at.extend([(0, 0), (0, -ROW), (0, -2 * ROW), (0, -3 * ROW), (0, below), (0, below + ROW), (0, below + 2 * ROW)]);
        }
        Kind::Folk | Kind::Knight => {
            let right = (body.right() + 1 - u.x, body.y + 1 - u.y);
            let left = (body.x - 1 - u.w - u.x, body.y + 1 - u.y);
            let below = body.bottom() + 2 - u.y;
            at.extend([
                (0, 0),
                (0, -ROW),
                right,
                left,
                (0, below),
                (0, -2 * ROW),
                (0, below + ROW),
                (right.0, right.1 - ROW),
                (left.0, left.1 - ROW),
                (right.0, right.1 + ROW),
                (left.0, left.1 + ROW),
                (0, -3 * ROW),
                (0, below + 2 * ROW),
                (right.0 + u.w + 1, right.1),
                (left.0 - u.w - 1, left.1),
                (0, -4 * ROW),
                (0, below + 3 * ROW),
            ]);
        }
    }
    at
}

/// Paint every placed plate.
pub(super) fn draw(im: &mut Img, placed: &[Placed]) {
    for part in placed.iter().flat_map(|p| &p.parts) {
        let r = part.rect;
        im.rect(r.x, r.y, r.w, r.h, 'k');
        if let Some(edge) = part.edge {
            im.frame(r.x, r.y, r.w, r.h, edge);
        }
        im.text(r.x + part.pad.0, r.y + part.pad.1, &part.text, part.ink);
    }
}
