//! The lists' words. Every box of words on a seat's screen is claimed here
//! in one ledger: no two may meet and none may leave the 384 by 224 canvas,
//! so the rule holds by the way the screen is built, and the test over
//! whole bouts (`the_lists_words_never_meet`) reads the same boxes the
//! drawing used. What cannot be placed clear of another box is not drawn as
//! words (a number crossing a banner becomes a crumb of its colour).
//!
//! `YOU` names only the knight looking: `called`.

use super::super::super::chrome::{self, Tone};
use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Tone as Mood};
use super::super::{NATIVE_H, NATIVE_W};
use crate::motion::{self, ease};
use crate::rules::Run;

pub(super) type Rect = (i32, i32, i32, i32);

pub(super) fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

pub(super) fn inside(r: Rect) -> bool {
    r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= NATIVE_W && r.1 + r.3 <= NATIVE_H
}

/// One claimed box: what claimed it, where, and the words in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Claim {
    pub what: &'static str,
    pub rect: Rect,
    pub text: String,
}

#[derive(Default)]
pub(super) struct Ledger {
    boxes: Vec<Claim>,
}

#[cfg(test)]
thread_local! {
    static LAST: std::cell::RefCell<Vec<Claim>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The boxes the last screen claimed (for the test over whole bouts).
#[cfg(test)]
pub(crate) fn last() -> Vec<Claim> {
    LAST.with(|l| l.borrow().clone())
}

impl Ledger {
    /// Whether `r` is whole in the frame and meets no claimed box.
    pub fn free(&self, r: Rect) -> bool {
        inside(r) && !self.boxes.iter().any(|c| meets(c.rect, r))
    }

    pub fn claim(&mut self, what: &'static str, rect: Rect, text: &str) {
        self.boxes.push(Claim { what, rect, text: text.to_string() });
    }

    /// Keep this screen's boxes for a test to read.
    pub fn keep(self) {
        #[cfg(test)]
        LAST.with(|l| *l.borrow_mut() = self.boxes);
    }

    /// Small words on a dark plate, the plate's top-left at `(x, y)`.
    /// Drawn and claimed (the plate is the box) if it is free; returns
    /// whether it was.
    pub fn plate(&mut self, im: &mut Img, what: &'static str, (x, y): (i32, i32), text: &str, ink: char) -> bool {
        let r = (x, y, text_width(text) + 7, 11);
        if !self.free(r) {
            return false;
        }
        self.claim(what, r, text);
        chrome::panel(im, r, Tone::CHIP);
        im.text(x + 3, y + 2, text, ink);
        true
    }

    /// Small words with no plate, claimed as their own box.
    pub fn text(&mut self, im: &mut Img, what: &'static str, (x, y): (i32, i32), text: &str, ink: char) -> bool {
        let r = (x, y, text_width(text), 7);
        if !self.free(r) {
            return false;
        }
        self.claim(what, r, text);
        im.text(x, y, text, ink);
        true
    }

    /// Big words (the kit's, edged) centred on `cx`, top at `y`.
    #[allow(clippy::too_many_arguments)]
    pub fn big(&mut self, im: &mut Img, what: &'static str, cx: i32, y: i32, text: &str, scale: i32, fill: char, edge: char) -> bool {
        let img = juice::big_text_img(text, scale, fill, edge);
        // The letters are what must stay clear, not the edge round them.
        let r = (cx - img.w / 2 + 2, y + 2, img.w - 4, img.h - 4);
        if !self.free(r) {
            return false;
        }
        self.claim(what, r, text);
        im.stamp(&img, cx - img.w / 2, y);
        true
    }

    /// The kit's banner as a claim: its rails, its words and the line under
    /// them, as one box by the kit's own geometry.
    #[allow(clippy::too_many_arguments)]
    pub fn banner(&mut self, im: &mut Img, what: &'static str, y: i32, title: &str, sub: &str, age: u32, life: u32, tone: Mood, cap: i32) {
        if age >= life {
            return;
        }
        self.claim(what, banner_rect(y, title, sub, age, life, cap), &format!("{title} {sub}"));
        juice::banner(im, y, title, sub, age, life, tone, cap);
    }

    /// The kit's countdown beat, claimed as the box of its letters.
    #[allow(clippy::too_many_arguments)]
    pub fn count(&mut self, im: &mut Img, what: &'static str, at: (i32, i32), label: &str, age: u32, life: u32, go: bool) {
        if age >= life {
            return;
        }
        let rest = (150 / text_width(label).max(5)).clamp(4, 9);
        let scale = ((rest as f32 * 1.7).round() as i32).max(1);
        let (w, h) = (text_width(label) * scale + 4, 7 * scale + 4);
        self.claim(what, (at.0 - w / 2, at.1 - h / 2, w, h), label);
        juice::countdown(im, at, label, age, life, go);
    }

    /// A number that flies from `from` to `to`; if its path meets a claimed
    /// box it is drawn as a bare crumb of `ink`.
    #[allow(clippy::too_many_arguments)]
    pub fn fly(&mut self, im: &mut Img, what: &'static str, text: &str, from: (i32, i32), to: (i32, i32), age: u32, life: u32, ink: char) {
        if age >= life {
            return;
        }
        let t = age as f32 / life as f32;
        let (x, y) = motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 26.0, ease::both(t));
        let scale = 1 + i32::from(t < 0.5);
        let (w, h) = (text_width(text) * scale + 4, 7 * scale + 4);
        let r = (x as i32 - w / 2, y as i32 - h / 2, w, h);
        if self.free(r) {
            self.claim(what, r, text);
            juice::fly_up(im, text, from, to, age, life, ink);
        } else {
            im.rect(x as i32 - 1, y as i32 - 1, 3, 3, ink);
        }
    }

    /// A number that pops up off a place and fades, if its box is free.
    #[allow(clippy::too_many_arguments)]
    pub fn pop(&mut self, im: &mut Img, what: &'static str, text: &str, at: (i32, i32), age: u32, life: u32, scale: i32, ink: char) {
        if age >= life {
            return;
        }
        let hop = (ease::out((age as f32 / 7.0).min(1.0)) * 12.0) as i32 + (age as i32 * 3 / 5);
        let (w, h) = (text_width(text) * scale + 4, 7 * scale + 4);
        let r = (at.0 - w / 2, at.1 - hop - h, w, h);
        if self.free(r) {
            self.claim(what, r, text);
            juice::pop_number(im, text, at, age, life, scale, ink);
        } else {
            im.rect(at.0 - 1, at.1 - hop - 2, 3, 3, ink);
        }
    }
}

/// A banner's box by the kit's own geometry (`juice::banner`): its rails,
/// its title and the line under it, with the lift of its dissolve.
pub(super) fn banner_rect(y: i32, title: &str, sub: &str, age: u32, life: u32, cap: i32) -> Rect {
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

/// How the seat `me` calls a knight, at most `take` letters: `YOU` for
/// themselves and only themselves; any other knight by their own name
/// (`KNIGHT n` for one who still carries the default `You`, or none).
pub(super) fn called(run: &Run, id: u32, me: Option<u32>, take: usize) -> String {
    if Some(id) == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}");
    }
    full.chars().take(take).collect()
}

/// A key on the board: its letter in a little cap. Claimed by the row.
pub(super) fn keycap(im: &mut Img, x: i32, y: i32, key: &str, lit: bool) -> i32 {
    let w = text_width(key) + 5;
    chrome::keycap(im, (x, y, w, 11));
    im.text(x + 2, y + 2, key, if lit { '6' } else { 'i' });
    w
}

/// A row of key prompts, each some caps and a word or two, on a dark strip,
/// centred on `cx` with its top at `y`: claimed as one box. `lit` caps
/// glow. Returns its box, or none if it could not be placed.
pub(super) fn prompts(led: &mut Ledger, im: &mut Img, cx: i32, y: i32, items: &[(&[&str], &str)], lit: bool) -> Option<Rect> {
    let gap = 10;
    let widths: Vec<i32> = items
        .iter()
        .map(|(caps, word)| {
            caps.iter().map(|k| text_width(k) + 7).sum::<i32>() + text_width(word) + 2
        })
        .collect();
    let row = widths.iter().sum::<i32>() + gap * (items.len() as i32 - 1).max(0);
    let w = row + 10;
    let r = (cx - w / 2, y, w, 15);
    if !led.free(r) {
        return None;
    }
    let words: Vec<String> = items.iter().map(|(caps, word)| format!("{} {word}", caps.join(" "))).collect();
    led.claim("prompts", r, &words.join(" "));
    chrome::panel(im, r, Tone::CHIP);
    let mut x = r.0 + 5 + (w - 10 - row) / 2;
    for ((caps, word), width) in items.iter().zip(&widths) {
        let mut cx = x;
        for k in caps.iter() {
            cx += keycap(im, cx, y + 2, k, lit) + 2;
        }
        im.text(cx, y + 5, word, 'O');
        x += width + gap;
    }
    Some(r)
}
