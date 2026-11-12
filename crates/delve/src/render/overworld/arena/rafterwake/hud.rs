//! Rafterwake's screen plan: where the tower window, the race cards and the foot strip stand on
//! the 384 by 224 canvas, every word on it as a `Word` that claims its box, and the cards, the
//! clock, the pot and the keys painted from the same plan. The words test reads the plan, so the
//! picture and the test see one geometry.
use crate::render::overworld::{
    chrome,
    ink::{Img, text_width},
};
use super::super::{juice, knights};
use crate::rules::rafterwake::{BELL_DECKS, BELL_LIFE, Lane, LENGTH, Q, Session, INTRO};
use crate::rules::{HZ, Run};

pub(super) type Rect = (i32, i32, i32, i32);

pub(super) const W: i32 = 384;
pub(super) const H: i32 = 224;
/// The top bar's height, the tower window under it and its left pad (tower x to window x).
pub(super) const BAR: i32 = 22;
pub(super) const WIN: Rect = (0, BAR, 278, 174);
pub(super) const PAD: i32 = 12;
/// The farthest the view scrolls down (the scaffold's foot) and how far above the boots it looks.
pub(super) const CAM_MAX: i32 = 400;
pub(super) const LEAD: i32 = 106;
/// The race column: its left edge, a card's height and the room for six.
pub(super) const SIDE: i32 = 285;
pub(super) const CARD_H: i32 = 25;
pub(super) const CARD_W: i32 = 94;
/// The column's frame: the brass plate round the cards and the crew strip.
const COLUMN: Rect = (280, BAR, 104, 176);
/// The foot strip.
pub(super) const FOOT: i32 = 198;

/// Do two boxes share a pixel?
pub(super) fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Is the box wholly on the canvas?
pub(super) fn inside((x, y, w, h): Rect) -> bool {
    x >= 0 && y >= 0 && x + w <= W && y + h <= H
}

/// How the seat `me` calls a knight, at most `take` letters: `YOU` for themselves and only
/// themselves; any other knight by their own name (`KNIGHT n` for one who still carries the
/// default `You`, or none).
pub(super) fn called(run: &Run, id: u32, me: u32, take: usize) -> String {
    if id == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}").chars().take(take.max(8)).collect::<String>();
    }
    full.chars().take(take).collect()
}

/// What the seat `me` sees: the run, the session, its own tower.
#[derive(Clone, Copy)]
pub(super) struct View<'a> {
    pub run: &'a Run,
    pub s: &'a Session,
    pub me: u32,
    pub d: &'a Lane,
    pub now: u32,
}

impl<'a> View<'a> {
    pub fn new(run: &'a Run, focus: Option<u32>) -> View<'a> {
        let s = run.rafterwake.as_ref().expect("a round");
        let me = focus.filter(|id| s.lanes.contains_key(id)).unwrap_or_else(|| *s.lanes.keys().next().expect("a seat"));
        View { run, s, me, d: &s.lanes[&me], now: run.tick as u32 }
    }
    /// Ticks the round has run (the countdown is not part of it).
    pub fn elapsed(&self) -> u32 {
        LENGTH - self.s.left
    }
    /// Ticks into the title and countdown, while they run.
    pub fn intro(&self) -> Option<u32> {
        (self.s.go > 0).then(|| INTRO - self.s.go)
    }
    pub fn over(&self) -> bool {
        self.s.left == 0
    }
    /// The view's top row, in tower pixels: it follows the knight's boots.
    pub fn cam(&self) -> i32 {
        cam_of(self.d.y / Q)
    }
    pub fn name(&self, id: u32, take: usize) -> String {
        called(self.run, id, self.me, take)
    }
    /// Seats in the order of the race, first first.
    pub fn order(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.s.lanes.keys().copied().collect();
        ids.sort_by_key(|&id| (self.s.score(id), id));
        ids
    }
}

pub(super) fn cam_of(y: i32) -> i32 {
    (y - LEAD).clamp(0, CAM_MAX)
}

/// The cloth ink of a seat, for its pip, its tag and its stripe.
pub(super) fn cloth(id: u32) -> char {
    knights::seat_cloth(id)
}

/// How a word is set: the 5x7 pen, or the kit's big letters.
#[derive(Clone, Copy, Debug)]
pub(super) enum Face {
    Pen(char),
    Big { scale: i32, fill: char, edge: char },
}

/// A word on the screen: where, what, how it is set, and whose it is.
#[derive(Clone, Debug)]
pub(super) struct Word {
    pub x: i32,
    pub y: i32,
    pub text: String,
    pub face: Face,
    pub kind: &'static str,
    pub who: Option<u32>,
}

impl Word {
    pub fn pen(x: i32, y: i32, text: impl Into<String>, ink: char, kind: &'static str) -> Word {
        Word { x, y, text: text.into(), face: Face::Pen(ink), kind, who: None }
    }
    pub fn big(x: i32, y: i32, text: impl Into<String>, scale: i32, fill: char, kind: &'static str) -> Word {
        Word { x, y, text: text.into(), face: Face::Big { scale, fill, edge: 'k' }, kind, who: None }
    }
    pub fn of(mut self, who: u32) -> Word {
        self.who = Some(who);
        self
    }
    pub fn rect(&self) -> Rect {
        match self.face {
            Face::Pen(_) => (self.x, self.y, text_width(&self.text), 7),
            Face::Big { scale, .. } => (self.x, self.y, text_width(&self.text) * scale + 4, 7 * scale + 4),
        }
    }
    pub fn paint(&self, im: &mut Img) {
        match self.face {
            Face::Pen(c) => {
                im.text(self.x, self.y, &self.text, c);
            }
            Face::Big { scale, fill, edge } => im.stamp(&juice::big_text_img(&self.text, scale, fill, edge), self.x, self.y),
        }
    }
}

/// Centre a word on `cx`.
pub(super) fn centred(cx: i32, y: i32, text: &str, ink: char, kind: &'static str) -> Word {
    Word::pen(cx - text_width(text) / 2, y, text, ink, kind)
}

/// Right-align a word on `right`.
pub(super) fn right(right: i32, y: i32, text: &str, ink: char, kind: &'static str) -> Word {
    Word::pen(right - text_width(text), y, text, ink, kind)
}

// ─── the cards ───────────────────────────────────────────────────────────────

/// The box of the `n`th card of the race column.
pub(super) fn card_rect(n: usize) -> Rect {
    (SIDE, BAR + 3 + n as i32 * CARD_H, CARD_W, CARD_H - 2)
}

/// The crew strip under the cards.
pub(super) const CREW: Rect = (SIDE, BAR + 3 + 6 * CARD_H + 2, CARD_W, 17);

/// A plate of the chrome slot's brass, `w` by `h`, its middle black: `None` where the art is off.
fn skin(key: &'static str, w: i32, h: i32) -> Option<&'static Img> {
    use std::sync::{Mutex, OnceLock};
    type Skin = (&'static str, i32, i32, bool, Option<&'static Img>);
    static SKINS: OnceLock<Mutex<Vec<Skin>>> = OnceLock::new();
    let on = super::super::slot_art::enabled();
    let mut all = SKINS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap();
    if let Some(s) = all.iter().find(|s| s.0 == key && s.1 == w && s.2 == h && s.3 == on) {
        return s.4;
    }
    let src = super::slot_image("rafterwake.chrome", key, &Img::new(1, 1));
    let made = (src.w > 1).then(|| {
        let mut p = super::plate(&src, w, h);
        p.rect(3, 3, w - 6, h - 6, 'k');
        &*Box::leak(Box::new(p))
    });
    all.push((key, w, h, on, made));
    made
}

/// Where deck pip `i` (0 to 11) of the card `r` stands.
fn pip_rect(r: Rect, i: i32) -> Rect {
    (r.0 + 8 + i * 7, r.1 + 15, 6, 5)
}

/// The tier of a deck: the lower decks cool steel, the middle ochre, the top ember.
pub(super) fn tier(deck: i32) -> (char, char) {
    match deck {
        ..=4 => ('Q', 'z'),
        5..=8 => ('o', 'T'),
        _ => ('R', '@'),
    }
}

/// The words of the race column: rank, name and score of each seat.
fn card_words(v: &View) -> Vec<Word> {
    let mut out = Vec::new();
    for (n, id) in v.order().into_iter().enumerate() {
        let r = card_rect(n);
        let lane = &v.s.lanes[&id];
        let place = v.s.place(id);
        let gold = place == 1 && lane.score > 0;
        out.push(Word::pen(r.0 + 8, r.1 + 4, place.to_string(), if gold { 'O' } else { 'h' }, "rank").of(id));
        out.push(Word::pen(r.0 + 16, r.1 + 4, v.name(id, 8), if id == v.me { 'T' } else { 'i' }, "card name").of(id));
        out.push(right(r.0 + r.2 - 4, r.1 + 4, &lane.score.to_string(), if id == v.me { 'T' } else { 'h' }, "card score").of(id));
    }
    out
}

/// Paint the cards' frames, stripes and pips (the words go over them).
fn paint_cards(im: &mut Img, v: &View) {
    match skin("panel", COLUMN.2, COLUMN.3) {
        Some(p) => im.stamp(p, COLUMN.0, COLUMN.1),
        None => chrome::inset_panel(im, COLUMN, chrome::Tone::BRASS),
    }
    for (n, id) in v.order().into_iter().enumerate() {
        let r = card_rect(n);
        let lane = &v.s.lanes[&id];
        let own = id == v.me;
        let tone = if own { chrome::Tone::BRASS } else { chrome::Tone::SLATE };
        chrome::inset_panel(im, r, tone);
        // The seat's stripe down the card's edge, in its cloth.
        im.rect(r.0 + 2, r.1 + 3, 3, r.3 - 6, cloth(id));
        for i in 0..12 {
            let p = pip_rect(r, i);
            let reached = lane.checkpoint as i32 > i;
            let (dim, lit) = tier(i + 1);
            if reached {
                im.rect(p.0, p.1, p.2, p.3, lit);
                im.rect(p.0, p.1 + p.3 - 1, p.2, 1, dim);
            } else {
                im.rect(p.0, p.1, p.2, p.3, 'x');
            }
            // The keeper's pip: its health, as it falls.
            if i == 11 && lane.height >= 370 && lane.keeper.hp > 0 {
                let mark = if lane.keeper.hp <= 2 { 'c' } else { '7' };
                im.rect(p.0 + 1, p.1 + 1, p.2 - 2, p.3 - 2, mark);
            }
        }
        if lane.finish.is_some() {
            // A finished knight: a crown mark where the keeper's pip stood.
            im.rect(r.0 + 8 + 11 * 7, r.1 + 15, 6, 5, '5');
        }
    }
}

// ─── the bar ─────────────────────────────────────────────────────────────────

/// The clock's ink: cream, then ember in the last ten seconds, flickering in the last five.
fn clock_ink(secs: u32, now: u32) -> char {
    match secs {
        0..=5 if now / 3 % 2 == 0 => 'c',
        0..=10 => '7',
        _ => 'T',
    }
}

fn bar_words(v: &View) -> Vec<Word> {
    let secs = v.s.left.div_ceil(HZ);
    let mut out = Vec::new();
    out.push(Word::big(8, 2, secs.to_string(), 2, clock_ink(secs, v.now), "clock"));
    out.push(centred(139, 8, "RAFTERWAKE", 'o', "title"));
    let pot = format!("{}G", v.s.pot());
    let w = text_width(&pot) * 2 + 4;
    out.push(Word::big(W - 6 - w, 2, pot, 2, 'O', "pot"));
    out
}

fn paint_bar(im: &mut Img, _v: &View) {
    match skin("header", W - 4, BAR - 1) {
        Some(p) => im.stamp(p, 2, 0),
        None => chrome::inset_panel(im, (0, 0, W, BAR - 2), chrome::Tone::BRASS),
    }
}

// ─── the crew strip ──────────────────────────────────────────────────────────

fn crew_words(v: &View) -> Vec<Word> {
    if v.s.bell > 0 {
        vec![Word::pen(CREW.0 + 30, CREW.1 + 4, "HOP HIGH", 'O', "bell").of(0)]
    } else {
        Vec::new()
    }
}

/// The bell's icon, 9 wide and 9 tall, swinging on `swing` (-1, 0, 1).
fn bell_icon(im: &mut Img, x: i32, y: i32, ink: char, swing: i32) {
    const ROWS: [&str; 9] = [
        "...###...",
        "..#####..",
        "..#####..",
        ".#######.",
        ".#######.",
        "#########",
        "#########",
        ".........",
        "....#....",
    ];
    for (r, row) in ROWS.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch == '#' {
                im.put(x + c as i32 + if r > 4 { swing } else { 0 }, y + r as i32, ink);
            }
        }
    }
}

fn paint_crew(im: &mut Img, v: &View) {
    let r = CREW;
    chrome::inset_panel(im, r, chrome::Tone::SLATE);
    let ringing = v.s.bell > 0;
    let swing = if ringing { [-1, 0, 1, 0][(v.now / 3 % 4) as usize] } else { 0 };
    bell_icon(im, r.0 + 6, r.1 + 3, if ringing { 'O' } else { 'G' }, swing);
    if ringing {
        // The time the high hops have left, a bar draining under the words.
        let k = v.s.bell as f32 / BELL_LIFE as f32;
        im.rect(r.0 + 30, r.1 + 12, (66.0 * k) as i32, 2, 'O');
        return;
    }
    // Three notches: the crew's average deck on its way to 3, 6 and 9.
    let crew = v.s.crew_deck() as i32;
    let bar = (r.0 + 22, r.1 + 6, 72, 4);
    im.rect(bar.0, bar.1, bar.2, bar.3, 'x');
    im.rect(bar.0, bar.1, (bar.2 * crew.min(90) / 90).max(0), bar.3, if v.s.bells > 0 { 'O' } else { 'o' });
    for (n, deck) in BELL_DECKS.iter().enumerate() {
        let x = bar.0 + bar.2 * *deck as i32 / 9 - 1;
        let rung = (n as u8) < v.s.bells;
        im.rect(x, bar.1 - 2, 2, bar.3 + 4, if rung { 'T' } else { 'U' });
    }
}

// ─── the foot ────────────────────────────────────────────────────────────────

/// A key's cap, 11 by 11, with its mark.
fn keycap(im: &mut Img, x: i32, y: i32, mark: &[&str], lit: bool) {
    chrome::keycap(im, (x, y, 11, 11));
    for (r, row) in mark.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch == '#' {
                im.put(x + 3 + c as i32, y + 3 + r as i32, if lit { 'T' } else { 'h' });
            }
        }
    }
}

const LEFT: [&str; 5] = ["..#..", ".##..", "#####", ".##..", "..#.."];
const RIGHT: [&str; 5] = ["..#..", "..##.", "#####", "..##.", "..#.."];
const UP: [&str; 5] = ["..#..", ".###.", "#####", "..#..", "..#.."];
const FKEY: [&str; 5] = ["#####", "#....", "####.", "#....", "#...."];

/// How lit the keys are: bright through the first twelve seconds, then dim, then bright again
/// at once for a second if the knight's hands have been idle on the ground.
fn keys_lit(v: &View) -> bool {
    v.elapsed() < 12 * HZ || v.intro().is_some()
}

fn foot_words(v: &View) -> Vec<Word> {
    let lit = keys_lit(v);
    let k = if lit { 'T' } else { 'J' };
    let d = v.d;
    let mut out = vec![
        Word::pen(31, 206, "RUN", k, "key"),
        Word::pen(66, 206, "HOP", k, "key"),
        Word::pen(103, 206, "CLIMB", k, "key"),
    ];
    out.push(Word::pen(160, 206, "DECK", 'h', "foot"));
    let shown = d.checkpoint.to_string();
    out.push(Word::big(188, 202, shown, 2, tier(d.checkpoint as i32).1, "deck"));
    out.push(Word::pen(222, 206, format!("FALLS {}", d.falls), 'h', "foot"));
    let best = v.run.home.countryside.games.rafterwake.best;
    out.push(right(W - 8, 206, &format!("BEST {best}"), 'h', "foot"));
    out
}

fn paint_foot(im: &mut Img, v: &View) {
    match skin("header", W - 4, H - FOOT) {
        Some(p) => im.stamp(p, 2, FOOT),
        None => chrome::inset_panel(im, (0, FOOT, W, H - FOOT), chrome::Tone::SLATE),
    }
    let lit = keys_lit(v);
    keycap(im, 5, 204, &LEFT, lit);
    keycap(im, 17, 204, &RIGHT, lit);
    keycap(im, 52, 204, &FKEY, lit);
    keycap(im, 89, 204, &UP, lit);
}

// ─── everything that stands still ────────────────────────────────────────────

/// The words that are always there, in a round that is on.
pub(super) fn fixed_words(v: &View) -> Vec<Word> {
    let mut out = bar_words(v);
    out.extend(card_words(v));
    out.extend(crew_words(v));
    out.extend(foot_words(v));
    out
}

/// Paint the frames, icons and pips of the screen's fixed parts; words come after.
pub(super) fn paint_frames(im: &mut Img, v: &View) {
    paint_bar(im, v);
    paint_cards(im, v);
    paint_crew(im, v);
    paint_foot(im, v);
}
