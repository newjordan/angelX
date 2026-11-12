//! Pennon Rush's strips and words: the clock, the rail every knight runs
//! along, the pot, a chip for each knight, the captain's bar, the combo and
//! the three key prompts. Small words only, and every box of them is noted
//! (`claim`), so a test can see that none meets another and none leaves the
//! picture.

use super::super::super::chrome;
use super::super::super::ink::{Img, text_width};
use super::super::juice;
use super::super::{NATIVE_H, NATIVE_W, knights};
use crate::motion::ease;
use crate::rules::pennon::{Lane, Q, Session, WIDTH, show::HOLD};
use crate::rules::{HZ, Run};

/// The world's window on the canvas: rows `VIEW_TOP..VIEW_BOTTOM`.
pub(super) const VIEW_TOP: i32 = 14;
pub(super) const VIEW_BOTTOM: i32 = 202;
/// Screen row of a world row: the walkway (`FLOOR`) stands on `FLOOR + OY`.
pub(super) const OY: i32 = 52;
/// Where the rail runs along the top strip.
const RAIL: (i32, i32) = (46, 300);

#[cfg(test)]
thread_local! {
    /// Every box of words drawn this frame (tests only): no two may meet and
    /// none may leave the picture.
    pub(super) static CLAIMS: std::cell::RefCell<Vec<(&'static str, (i32, i32, i32, i32), String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Note a box of words on this frame, for the tests that look for words
/// meeting.
pub(super) fn claim(what: &'static str, rect: (i32, i32, i32, i32), text: &str) {
    #[cfg(test)]
    CLAIMS.with(|c| c.borrow_mut().push((what, rect, text.to_string())));
    #[cfg(not(test))]
    let _ = (what, rect, text);
}

/// As `claim`, for words the kit draws with a two-pixel edge round them: the
/// letters are what must stay clear, not the edge.
pub(super) fn claim_big(what: &'static str, (x, y, w, h): (i32, i32, i32, i32), text: &str) {
    claim(what, (x + 1, y + 1, w - 2, h - 2), text);
}

/// Forget the frame's claims (tests, before a draw).
#[cfg(test)]
pub(super) fn forget() {
    CLAIMS.with(|c| c.borrow_mut().clear());
}

/// The small words of a frame: drawn into the picture, or handed to the
/// terminal to write itself.
#[derive(Default)]
pub(super) struct Words {
    pub list: Vec<(i32, i32, String, char)>,
}

impl Words {
    pub fn say(&mut self, what: &'static str, x: i32, y: i32, text: &str, ink: char) {
        claim(what, (x - 1, y - 1, text_width(text) + 2, 9), text);
        self.list.push((x, y, text.to_string(), ink));
    }

    pub fn draw(&self, im: &mut Img) {
        for (x, y, t, c) in &self.list {
            im.text(*x, *y, t, *c);
        }
    }

    pub fn plain(&self) -> Vec<(i32, i32, String)> {
        self.list.iter().map(|(x, y, t, _)| (*x, *y, t.clone())).collect()
    }
}

/// How the seat `me` calls a knight, at most `take` letters: `YOU` for
/// themselves and only themselves; any other knight by their own name (`KNIGHT
/// n` for one who still carries the default `You`, or none).
pub(super) fn called(run: &Run, id: u32, me: Option<u32>, take: usize) -> String {
    if Some(id) == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}");
    }
    full.chars().take(take.max(1)).collect()
}

// ─── the strip's small pictures ──────────────────────────────────────────────

fn glass() -> Img {
    Img::from_rows(&["OOOOO", ".ttt.", "..t..", "..t..", ".ttt.", "OOOOO"])
}

fn coin() -> Img {
    Img::from_rows(&[".OO.", "O55O", "O5oO", ".OO."])
}

fn shield(lit: bool) -> Img {
    if lit {
        Img::from_rows(&["99999", "9666O", "9666O", ".666.", "..O.."])
    } else {
        Img::from_rows(&["jjjjj", "jgggj", "jgggj", ".jgj.", "..j.."])
    }
}

fn crown() -> Img {
    Img::from_rows(&["O.O.O", "OOOOO", "o555o"])
}

/// A pennon on the rail: a pole and a cloth, lit while it flies.
fn flag(captain: bool, lit: bool) -> Img {
    match (captain, lit) {
        (true, true) => Img::from_rows(&["O", "O", "OOO", "OOOO", "OOO", "O", "O", "O"]),
        (true, false) => Img::from_rows(&["g", "g", "gg", "g", "g", "g", "g", "g"]),
        (false, true) => Img::from_rows(&["R", "R", "RR", "R", "R", "R"]),
        (false, false) => Img::from_rows(&["g", "g", "g", "g", "g", "g"]),
    }
}

// ─── the layout ──────────────────────────────────────────────────────────────

/// One knight's chip in the bottom strip.
#[derive(Clone, Copy, Debug)]
pub(super) struct Chip {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub name: usize,
}

/// The bottom strip's top edge: one row of chips for four knights, two for
/// five or six.
pub(super) fn strip_top(seats: usize) -> i32 {
    if seats > 4 { NATIVE_H - 28 } else { NATIVE_H - 22 }
}

/// Where the number of a chip sits: right-aligned to the chip's edge.
fn score_x(c: &Chip, text: &str) -> i32 {
    c.x + c.w - 7 - text_width(text)
}

/// The chips, left to right in seat order, fitted to the strip.
pub(super) fn chips(ids: &[u32]) -> Vec<Chip> {
    let n = ids.len().max(1) as i32;
    let cols = if n > 4 { (n + 1) / 2 } else { n };
    let w = (NATIVE_W - 8) / cols;
    let top = strip_top(ids.len());
    // The pip and the shield take 13, the number four digits and two to
    // spare; the name gets the rest of the chip.
    let name = (((w - 13 - 25 - 4) + 1) / 6).clamp(1, 12) as usize;
    ids.iter()
        .enumerate()
        .map(|(i, &id)| {
            let (col, row) = (i as i32 % cols, i as i32 / cols);
            Chip { id, x: 4 + col * w, y: top + 6 + row * 11, w, name }
        })
        .collect()
}

/// Where a knight's flying score lands: just over its number.
pub(super) fn anchor(ids: &[u32], id: u32) -> (i32, i32) {
    chips(ids)
        .iter()
        .find(|c| c.id == id)
        .map_or((NATIVE_W / 2, strip_top(ids.len()) - 9), |c| (c.x + c.w - 7 - 12, strip_top(ids.len()) - 9))
}

/// A world column on the rail.
fn rail_x(wx: i32) -> i32 {
    RAIL.0 + wx.clamp(0, WIDTH) * (RAIL.1 - RAIL.0) / WIDTH
}

/// The top strip: the clock, the rail with every knight's pip and the
/// pennons of this knight's lane, and the pot.
pub(super) fn top(im: &mut Img, s: &Session, d: &Lane, focus: u32, words: &mut Words, now: u32) {
    chrome::panel(im, (0, 0, NATIVE_W, 14), chrome::Tone::SLATE);
    let secs = s.left.div_ceil(HZ);
    let live = s.hold == 0 && s.left > 0;
    let low = live && secs <= 10;
    let blink = low && now / 6 % 2 == 0;
    im.stamp(&glass(), 6, 4);
    let clock = if s.hold > 0 { (crate::rules::pennon::LENGTH / HZ).to_string() } else { secs.to_string() };
    words.say("clock", 14, 4, &clock, if blink { '7' } else if low { 'R' } else { 'O' });
    // The rail: a groove, the lane's pennons, then the knights' pips.
    let (a, b) = RAIL;
    im.rect(a, 6, b - a, 3, 's');
    im.rect(a, 7, b - a, 1, 'x');
    for f in &d.foes {
        im.stamp(&flag(f.captain, f.hp > 0), rail_x(f.x), if f.captain { 0 } else { 2 });
    }
    let mut order: Vec<u32> = s.lanes.keys().copied().collect();
    order.sort_by_key(|&id| (id == focus, id));
    for id in order {
        let Some(l) = s.lanes.get(&id) else { continue };
        let x = rail_x(l.x / Q);
        let ink = knights::seat_mark(id);
        if id == focus {
            im.rect(x - 2, 4, 5, 7, 'k');
            im.rect(x - 1, 5, 3, 5, ink);
            im.rect(x, 3, 1, 1, 'H');
        } else {
            im.rect(x - 1, 5, 3, 5, 'k');
            im.rect(x, 6, 1, 3, ink);
        }
    }
    // The pot, with a coin.
    let pot = format!("{}G", crate::rules::pennon::GOLD_PER_SEAT * s.lanes.len() as u32);
    let px = NATIVE_W - 6 - text_width(&pot);
    im.stamp(&coin(), px - 7, 5);
    words.say("pot", px, 4, &pot, 'O');
    // The table's rally: three pips toward the next ward for all.
    let rallied = s.show.calls.iter().any(|c| matches!(c.cry, crate::rules::pennon::show::Cry::Rally) && c.age(s.show.t) < 14);
    let filled = if rallied { 3 } else { s.show.felled % crate::rules::pennon::show::RALLY_EVERY };
    let x0 = px - 7 - 4 - 3 * 6 - 8;
    im.stamp(&shield(rallied), x0, 4);
    for k in 0..3 {
        let on = k < filled as i32;
        im.rect(x0 + 8 + k * 6, 6, 5, 4, 'k');
        im.rect(x0 + 9 + k * 6, 7, 3, 2, if on { if rallied && now / 3 % 2 == 0 { '9' } else { 'O' } } else { 's' });
    }
}

/// The bottom strip: a chip for each knight, the leader crowned, a shield
/// lit for the knights who hold a ward and a red tick for each fall.
pub(super) fn bottom(im: &mut Img, run: &Run, s: &Session, focus: u32, words: &mut Words) {
    let ids: Vec<u32> = s.lanes.keys().copied().collect();
    let top = strip_top(ids.len());
    chrome::panel(im, (0, top, NATIVE_W, NATIVE_H - top), chrome::Tone::SLATE);
    let best = s.lanes.values().map(|l| l.score).max().unwrap_or(0);
    for (n, chip) in chips(&ids).into_iter().enumerate() {
        let Some(l) = s.lanes.get(&chip.id) else { continue };
        let you = chip.id == focus;
        // A hairline between chips, so a number reads with its own name.
        if chip.x > 4 && n > 0 {
            im.rect(chip.x - 3, chip.y - 4, 1, 14, 'g');
        }
        let ink = knights::seat_mark(chip.id);
        im.rect(chip.x, chip.y + 1, 5, 5, 'k');
        im.rect(chip.x + 1, chip.y + 2, 3, 3, ink);
        let lead = s.lanes.len() > 1 && l.score == best && best > 0;
        if lead {
            im.stamp(&crown(), chip.x, chip.y - 3);
        }
        im.stamp(&shield(l.ward > 0), chip.x + 6, chip.y);
        for k in 0..l.falls.min(3) as i32 {
            im.rect(chip.x + 6 + k * 2, chip.y - 3, 1, 2, '7');
        }
        let name = called(run, chip.id, Some(focus), chip.name);
        words.say("chip-name", chip.x + 13, chip.y, &name, if you { 'T' } else { 'i' });
        let num = l.score.to_string();
        let swell = you
            && s.show
                .hits
                .iter()
                .filter(|h| h.seat == focus && h.kill)
                .filter_map(|h| h.age(s.show.t).checked_sub(super::fx::FLY))
                .any(|a| a < 6);
        words.say(
            "chip-score",
            score_x(&chip, &num),
            chip.y - i32::from(swell),
            &num,
            if swell { '9' } else if lead { 'T' } else { 'h' },
        );
    }
}

/// The captain's bar under the top strip: three stretches, one for each of
/// his armour; the chip just lost shows pale for a moment.
pub(super) fn captain(im: &mut Img, d: &Lane, last: Option<u32>) {
    let Some(f) = d.foes.get(3).filter(|f| f.hp > 0 && f.attack > 0) else { return };
    let (x, y, w, h) = (122, 17, 140, 5);
    im.rect(x - 1, y - 1, w + 2, h + 2, 'k');
    im.rect(x, y, w, h, 's');
    let fill = w * i32::from(f.hp) / 18;
    let pulse = last.is_some_and(|a| a < 6);
    if let Some(a) = last.filter(|&a| a < 14) {
        let lost = w * (i32::from(f.hp) + 1).min(18) / 18;
        let keep = lost - (lost - fill) * (a as i32).min(10) / 10;
        im.rect(x + fill, y, (keep - fill).max(0), h, 'H');
    }
    for (i, c) in ['R', 'o', 'O'].into_iter().enumerate() {
        let (a, b) = (w * i as i32 / 3, w * (i as i32 + 1) / 3);
        let to = fill.min(b);
        if to > a {
            im.rect(x + a, y, to - a, h, if pulse { 'T' } else { c });
            im.rect(x + a, y, to - a, 1, 't');
        }
    }
    for k in 1..3 {
        im.rect(x + w * k / 3, y - 1, 1, h + 2, 'k');
    }
    im.stamp(&flag(true, true), x - 7, y - 2);
}

/// The run of hits, big, at the top left: it swells when it grows and the
/// last one shatters and falls when the knight does.
pub(super) fn combo(im: &mut Img, d: &Lane, grown: Option<u32>, lost: Option<(u32, u32)>) {
    if d.chain >= 2 && d.down == 0 {
        let big = grown.is_some_and(|a| a < 5);
        let t = juice::big_text_img(&format!("X{}", d.chain), if big { 3 } else { 2 }, if big { 'T' } else { 'O' }, 'B');
        let at = (6, 20);
        im.stamp(&t, at.0, at.1);
        claim_big("combo", (at.0, at.1, t.w, t.h), &format!("X{}", d.chain));
        // The run's fuse: five pips toward the next call.
        let into = (d.chain % 5) as i32;
        for k in 0..5 {
            im.rect(at.0 + 2 + k * 5, at.1 + 28, 4, 2, if k < into { 'O' } else { 's' });
        }
    } else if let Some((chain, age)) = lost.filter(|&(c, a)| c >= 2 && a < 22) {
        let mut t = juice::big_text_img(&format!("X{chain}"), 2, '7', 'n');
        juice::dissolve(&mut t, age as f32 / 22.0);
        let at = (6, 20 + (age as i32 * age as i32) / 18);
        im.stamp(&t, at.0, at.1);
        claim_big("combo-lost", (at.0, at.1, t.w, t.h), &format!("X{chain}"));
    }
}

/// The keys, for a knight's first moments: a cap and a word each.
pub(super) fn keys(im: &mut Img, words: &mut Words, y: i32) {
    let rows: [(&str, &str); 4] = [("E", "JUMP"), ("F", "SHOOT"), ("ARROWS", "AIM"), ("S", "DUCK")];
    let cap = |k: &str| text_width(k) + 6;
    let widths: Vec<i32> = rows.iter().map(|(k, w)| cap(k) + 3 + text_width(w)).collect();
    let gap = 12;
    let total: i32 = widths.iter().sum::<i32>() + gap * (rows.len() as i32 - 1);
    let mut x = (NATIVE_W - total) / 2;
    for ((key, word), w) in rows.iter().zip(&widths) {
        chrome::panel(im, (x, y - 2, cap(key), 11), chrome::Tone::KEYCAP);
        words.say("keycap", x + 3, y, key, 'T');
        words.say("key", x + cap(key) + 3, y, word, 'i');
        x += w + gap;
    }
}

/// The world column at the left of this knight's window: the knight a third
/// in. A knight who has broken the siege watches the front-runner still
/// running: the window drifts over to them.
pub(super) fn camera(s: &Session, d: &Lane) -> i32 {
    let at = |x: i32| (x - 140).clamp(0, WIDTH - NATIVE_W);
    let own = at(d.x / Q);
    let Some(fin) = d.finish else { return own };
    let Some(lead) = s.lanes.values().filter(|l| l.finish.is_none()).map(|l| l.x / Q).max() else { return own };
    let since = s.show.t.saturating_sub(HOLD).saturating_sub(fin);
    let k = ease::out((since as f32 / 45.0).min(1.0));
    own + ((at(lead) - own) as f32 * k) as i32
}

/// Whether a lane has nothing struck yet: the prompts still help.
pub(super) fn green(d: &Lane) -> bool {
    d.foes.iter().all(|f| f.hp == if f.captain { 18 } else { 3 }) && d.falls == 0
}



