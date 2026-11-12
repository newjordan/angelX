//! The screens' small kit, in the HUD's family: black plates with a dim frame, 5x7 text with a
//! shadow where it is big, a lit bar behind the row under the cursor, twelve-segment bars, and
//! the key hints along the foot. Nothing here knows about the game.
use super::*;
pub(super) use super::super::hud::{centred, plate, shout, text_w};

/// Segments in a stat bar, and the width of each with its gap.
pub const SEGS: i32 = 12;
pub const SEG_W: i32 = 7;

/// Dots darkened to a checkerboard over a box: the picture shows through, the words read over it.
pub fn veil(im: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    for j in y.max(0)..(y + h).min(224) {
        for i in x.max(0)..(x + w).min(384) {
            if (i + j) % 2 == 0 { im.put(i, j, 'k'); }
        }
    }
}

/// The key hints along the foot of the screen.
pub fn hints(im: &mut Img, line: &str) {
    im.rect(0, 215, 384, 9, 'k');
    im.text(7, 216, line, 't');
}

/// A line of words that came from the game (a shop's answer, a lock), above the hints, if it is up.
pub fn note(im: &mut Img, race: &rules::Session, seat: u32) { note_at(im, race, seat, 192) }

/// The same, centred on `cx`.
pub fn note_at(im: &mut Img, race: &rules::Session, seat: u32, cx: i32) {
    let Some(sf) = race.flow.seats.get(&seat) else { return };
    if race.flow.now >= sf.note_until || sf.note.is_empty() { return; }
    let w = text_w(&sf.note, 1) + 14;
    plate(im, cx - w / 2, 199, w, 13);
    centred(im, cx, 202, &sf.note, 1, 'O');
}

/// A list row: a lit bar and an arrow when it is the one under the cursor.
pub fn row(im: &mut Img, x: i32, y: i32, w: i32, h: i32, on: bool) {
    if on { im.rect(x, y, w, h, 'g'); im.rect(x, y, 2, h, 'T'); }
}

/// A stat bar of `value`, 0 to 1, labelled to its left: a change of `delta` shows as segments
/// added (green when more is good, orange when it is not) or taken away.
pub fn bar(im: &mut Img, x: i32, y: i32, label: &str, value: f32, delta: f32, more_is_good: bool) {
    bar_at(im, x, x + 46, y, label, value, delta, more_is_good);
}

/// The same with the bar starting at `lx`.
#[allow(clippy::too_many_arguments)]
pub fn bar_at(im: &mut Img, x: i32, lx: i32, y: i32, label: &str, value: f32, delta: f32, more_is_good: bool) {
    im.text(x, y, label, 't');
    let seg = |v: f32| (v.clamp(0., 1.) * SEGS as f32).round() as i32;
    let (now, then) = (seg(value), seg(value + delta));
    let (more, less) = if more_is_good { ('y', 'O') } else { ('O', 'y') };
    for i in 0..SEGS {
        let ink = if i < now.min(then) { 'T' }
            else if i < then { more }
            else if i < now { less }
            else { 'g' };
        im.rect(lx + i * SEG_W, y, SEG_W - 2, 6, ink);
    }
}

/// Gold with thousands marked: 1250 is 1,250.
pub fn gold(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(c);
    }
    out
}

/// A little coin and a sum, right-aligned to `right`.
pub fn purse(im: &mut Img, right: i32, y: i32, n: u32, ink: char) {
    let s = gold(n);
    let w = text_w(&s, 1);
    im.text(right - w, y, &s, ink);
    im.rect(right - w - 9, y + 1, 6, 6, '6');
    im.rect(right - w - 8, y + 2, 4, 4, '4');
}

/// Words broken to `cols` characters a line.
pub fn wrap(s: &str, cols: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in s.split_whitespace() {
        let line = lines.last_mut().unwrap();
        if !line.is_empty() && line.len() + 1 + word.len() > cols { lines.push(word.to_string()); }
        else { if !line.is_empty() { line.push(' '); } line.push_str(word); }
    }
    lines
}

/// `m:ss.cc` of milliseconds, or dashes.
pub fn time_ms(ms: u32) -> String {
    if ms == 0 { "-:--.--".into() } else { super::super::state::stamp(ms as f32 / 1000.) }
}

/// A little gold cup, 9 by 9 dots.
pub fn trophy(im: &mut Img, x: i32, y: i32, ink: char) {
    im.rect(x + 1, y, 7, 4, ink);
    im.rect(x + 2, y + 4, 5, 2, ink);
    im.rect(x + 4, y + 6, 1, 2, ink);
    im.rect(x + 2, y + 8, 5, 1, ink);
    im.rect(x, y + 1, 1, 2, ink);
    im.rect(x + 8, y + 1, 1, 2, ink);
}

/// A padlock, 7 by 8 dots.
pub fn lock(im: &mut Img, x: i32, y: i32, ink: char) {
    im.rect(x + 1, y, 5, 1, ink);
    im.rect(x + 1, y + 1, 1, 2, ink);
    im.rect(x + 5, y + 1, 1, 2, ink);
    im.rect(x, y + 3, 7, 5, ink);
}
