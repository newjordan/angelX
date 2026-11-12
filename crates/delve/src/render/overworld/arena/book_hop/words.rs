//! Who has the words. Every piece of text Book Hop puts on a seat's screen
//! is a box claimed here, in one order of rank, and no two boxes may meet
//! and none may leave the 384 by 224 frame: the strip, the title and its
//! countdown, the calls, the tags over the knights, the numbers that fly and
//! pop. What cannot be placed without meeting another box is not drawn as
//! words (a number crossing a banner is a crumb of its knight's ink), so
//! the rule holds by the way the screen is built, and the test over whole
//! runs (`the_words_never_meet_at_one_to_six_seats`) reads the same boxes
//! the drawing used.
//!
//! All the words are drawn in the screen's own layer (`screen`), after the
//! world's, so a world mark never owns a box. (Copied from the Powder
//! Room's ledger, which is its own.)

use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Tone};
use super::super::{NATIVE_H, NATIVE_W};
use crate::motion::{self, ease};

/// A box: left, top, width, height.
pub(crate) type Rect = (i32, i32, i32, i32);

/// One claimed box, and what claimed it (for a test's message).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Claim {
    pub what: &'static str,
    pub rect: Rect,
}

pub(crate) fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

pub(crate) fn inside(r: Rect) -> bool {
    r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= NATIVE_W && r.1 + r.3 <= NATIVE_H
}

/// The boxes claimed on one screen, so far.
#[derive(Default)]
pub(super) struct Ledger {
    boxes: Vec<Claim>,
    #[cfg(test)]
    said: Vec<(&'static str, String, Rect)>,
}

#[cfg(test)]
thread_local! {
    static LAST: std::cell::RefCell<Vec<Claim>> = const { std::cell::RefCell::new(Vec::new()) };
    static SAID: std::cell::RefCell<Vec<(&'static str, String, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The words the last screen put up (for the test of who is `YOU`).
#[cfg(test)]
pub(crate) fn said() -> Vec<(&'static str, String, Rect)> {
    SAID.with(|l| l.borrow().clone())
}

/// The boxes the last screen claimed (for the test over whole runs).
#[cfg(test)]
pub(crate) fn last() -> Vec<Claim> {
    LAST.with(|l| l.borrow().clone())
}

impl Ledger {
    /// Whether `r` is whole in the frame and meets no claimed box.
    pub fn free(&self, r: Rect) -> bool {
        inside(r) && !self.boxes.iter().any(|c| meets(c.rect, r))
    }

    /// Claim `r` for `what`.
    pub fn claim(&mut self, what: &'static str, r: Rect) {
        self.boxes.push(Claim { what, rect: r });
    }

    /// Claim `r` for the words `text`, which the test that reads them back
    /// will see beside the box.
    #[allow(unused_variables)]
    pub fn say(&mut self, what: &'static str, r: Rect, text: &str) {
        self.claim(what, r);
        #[cfg(test)]
        self.said.push((what, text.to_string(), r));
    }

    /// Keep this screen's boxes for a test to read.
    pub fn keep(self) {
        #[cfg(test)]
        {
            SAID.with(|l| *l.borrow_mut() = self.said);
            LAST.with(|l| *l.borrow_mut() = self.boxes);
        }
    }

    /// Small words at `(x, y)`: drawn and claimed. Returns the pen's x
    /// after the last letter.
    pub fn small(&mut self, over: &mut Img, what: &'static str, x: i32, y: i32, s: &str, ink: char) -> i32 {
        self.say(what, (x, y, text_width(s), 7), s);
        over.text(x, y, s, ink)
    }

    /// Big words centred on `cx` with the top at `y`, in `fill` with an
    /// edge: drawn and claimed (the picture is two pixels wider each side
    /// than its letters).
    #[allow(clippy::too_many_arguments)]
    pub fn big(&mut self, over: &mut Img, what: &'static str, cx: i32, y: i32, s: &str, scale: i32, fill: char, edge: char) {
        let img = juice::big_text_img(s, scale, fill, edge);
        self.say(what, (cx - img.w / 2, y, img.w, img.h), s);
        over.stamp(&img, cx - img.w / 2, y);
    }

    /// The kit's banner (`juice::banner`) as a claim: its rails, its words
    /// and the line under them, as one box, by the kit's own geometry.
    #[allow(clippy::too_many_arguments)]
    pub fn banner(&mut self, over: &mut Img, what: &'static str, y: i32, title: &str, sub: &str, age: u32, life: u32, tone: Tone, cap: i32) {
        if age >= life {
            return;
        }
        let rect = banner_rect(y, title, sub, age, life, cap);
        self.say(what, rect, &format!("{title}|{sub}"));
        juice::banner(over, y, title, sub, age, life, tone, cap);
    }

    /// The kit's countdown beat, claimed as the box of its digits.
    pub fn count(&mut self, over: &mut Img, what: &'static str, at: (i32, i32), label: &str, age: u32, life: u32, go: bool) {
        if age >= life {
            return;
        }
        let rest = (150 / text_width(label).max(5)).clamp(4, 9);
        // The beat springs to 1.7 times its size at most, then shrinks away.
        let scale = ((rest as f32 * 1.7).round() as i32).max(1);
        let (w, h) = (text_width(label) * scale + 4, 7 * scale + 4);
        self.say(what, (at.0 - w / 2, at.1 - h / 2, w, h), label);
        juice::countdown(over, at, label, age, life, go);
    }

    /// A number that flies from `from` to `to`; if its path meets a
    /// claimed box now it is drawn as a bare crumb of `ink`, with no words.
    #[allow(clippy::too_many_arguments)]
    pub fn fly(&mut self, over: &mut Img, what: &'static str, text: &str, from: (i32, i32), to: (i32, i32), age: u32, life: u32, ink: char) {
        if age >= life {
            return;
        }
        let t = age as f32 / life as f32;
        let (x, y) = motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 26.0, ease::both(t));
        let scale = 1 + i32::from(t < 0.5);
        let (w, h) = (text_width(text) * scale + 4, 7 * scale + 4);
        let r = (x as i32 - w / 2, y as i32 - h / 2, w, h);
        if self.free(r) {
            self.say(what, r, text);
            juice::fly_up(over, text, from, to, age, life, ink);
        } else {
            over.rect(x as i32 - 1, y as i32 - 1, 3, 3, ink);
        }
    }

    /// A number that pops up off a place and fades (`juice::pop_number`),
    /// drawn if its box is free of every claimed word, else not at all.
    #[allow(clippy::too_many_arguments)]
    pub fn pop(&mut self, over: &mut Img, what: &'static str, text: &str, at: (i32, i32), age: u32, life: u32, scale: i32, ink: char) {
        if age >= life {
            return;
        }
        let hop = (ease::out((age as f32 / 7.0).min(1.0)) * 12.0) as i32 + (age as i32 * 3 / 5);
        let (w, h) = (text_width(text) * scale + 4, 7 * scale + 4);
        let r = (at.0 - w / 2, at.1 - hop - h, w, h);
        if self.free(r) {
            self.say(what, r, text);
            juice::pop_number(over, text, at, age, life, scale, ink);
        }
    }
}

/// A banner's box by the kit's own geometry (`juice::banner`): its rails,
/// its title and the line under it, with the lift of its dissolve.
pub(crate) fn banner_rect(y: i32, title: &str, sub: &str, age: u32, life: u32, cap: i32) -> Rect {
    let tw = text_width(title).max(1);
    let rest = ((NATIVE_W - 24) / tw).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / tw).max(rest);
    let tail = life.saturating_sub(14);
    let out = if age > tail { (age - tail) as f32 / 14.0 } else { 0.0 };
    let lift = (ease::in_quad(out) * 10.0) as i32;
    let text_w = tw * scale + 4;
    let top = y - lift - (scale - rest) * 3;
    let half = text_w / 2 + 8;
    let (mut x0, mut x1) = (NATIVE_W / 2 - half, NATIVE_W / 2 + half);
    let y0 = top.min(y - 5 - lift);
    let mut y1 = (top + 7 * scale + 4).max(y + 7 * rest + 10 - lift);
    if !sub.is_empty() && age >= 4 {
        let sw = text_width(sub) + 4;
        x0 = x0.min(NATIVE_W / 2 - sw / 2);
        x1 = x1.max(NATIVE_W / 2 - sw / 2 + sw);
        y1 = y1.max(y + 7 * rest + 12 - lift + 11);
    }
    (x0, y0, x1 - x0, y1 - y0)
}
