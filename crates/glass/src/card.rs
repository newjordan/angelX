//! The card: the window's one foreground dialog — `/setup` and the
//! approval ask. Clear glass like the showcase (a hairline round a faint
//! dim, no slab, no shadow), the squire riding its roof, and its words
//! native atlas text from a cell grid the cockpit inks.
//!
//! The cockpit hands over what the card says (`Host::set_card`) with each
//! frame; how it moves is the window's, a function of time since the card
//! came, since its page last changed and since it went: nothing the
//! cockpit has to tick. A first launch opens on a short title: the name
//! on a line that grows, the squire walking it, the line becoming the
//! card's top edge as the card unfolds under him.

use ratatui_core::{buffer::Buffer, layout::Rect};
use std::sync::Arc;

use crate::graphics::Picture;

/// What the squire does on the roof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub enum Knight {
    /// Sitting, swinging his legs now and then.
    #[default]
    Sit,
    /// A greeting.
    Wave,
    /// Holding something up: a key to paste.
    Present,
    /// Up on his feet, pacing while something runs.
    Busy,
    /// Thrown off balance: something went wrong.
    Oops,
    /// Hopping for joy: it worked.
    Cheer,
    /// On his feet, keeping watch: an approval waits.
    Guard,
}

/// How the card comes in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub enum Entrance {
    /// The squire walks up and kicks it open, as he does the showcase.
    #[default]
    Kick,
    /// The first launch: the title, then the card unfolds.
    Title,
    /// At once, already on the roof: an ask that must not wait.
    Pop,
}

/// A picture that plays on the card: a short film's frames laid out
/// `cols` to a row in one picture (`sheet`), shown over the card's cells
/// `at` (from the card's top-left) and coming up with that row's words.
/// It loops at `fps` from when the card first shows its sheet, or with
/// `once` plays through and holds its last frame. One frame holds still.
#[derive(Clone, Debug)]
pub struct Film {
    pub sheet: Arc<Picture>,
    /// One frame's size in the sheet's pixels.
    pub frame: (u32, u32),
    pub frames: u16,
    pub cols: u16,
    pub fps: f32,
    pub once: bool,
    pub at: Rect,
}

impl Film {
    /// Which frame shows `t` seconds after the film first showed.
    pub fn frame_at(&self, t: f32) -> u16 {
        let frames = self.frames.max(1);
        if self.fps <= 0.0 || frames == 1 {
            return 0;
        }
        let n = (t.max(0.0) * self.fps) as u64;
        if self.once { n.min(u64::from(frames - 1)) as u16 } else { (n % u64::from(frames)) as u16 }
    }

    /// Where frame `n` lies in the sheet, `[x, y, width, height]` in pixels.
    pub fn source(&self, n: u16) -> [f32; 4] {
        let cols = self.cols.max(1);
        let (w, h) = (self.frame.0 as f32, self.frame.1 as f32);
        [f32::from(n % cols) * w, f32::from(n / cols) * h, w, h]
    }
}

/// One frame of the card. `area` is the whole card in cells, its padding
/// included; `buffer` its cells from the top-left. The first
/// [`TITLE_ROWS`] rows are left clear for the title, which the window
/// sets large.
#[derive(Clone, Debug)]
pub struct Card {
    pub area: Rect,
    pub buffer: Buffer,
    pub title: String,
    /// Which page this is: a new serial plays the page's words in.
    pub serial: u64,
    /// The chosen row, under which the glass pill glides.
    pub cursor: Option<u16>,
    /// The columns the pill spans on its row, `(first, width)`: a card with
    /// choices side by side glides it across as well as down. `None` runs
    /// it the card's width.
    pub cursor_cols: Option<(u16, u16)>,
    /// Pictures playing on the card, under its words.
    pub films: Vec<Film>,
    /// The row that takes typing: a hairline under it and a caret.
    pub field: Option<u16>,
    pub knight: Knight,
    /// Something runs: a light travels the card's lower edge.
    pub busy: bool,
    pub entrance: Entrance,
}

/// Rows at the card's top the title is set in.
pub const TITLE_ROWS: u16 = 4;

impl Card {
    pub fn new(area: Rect, buffer: Buffer) -> Option<Self> {
        (area.width > 0
            && area.height > TITLE_ROWS
            && area.width <= 160
            && area.height <= 64
            && buffer.area == Rect::new(0, 0, area.width, area.height))
        .then_some(Self {
            area,
            buffer,
            title: String::new(),
            serial: 0,
            cursor: None,
            cursor_cols: None,
            films: Vec::new(),
            field: None,
            knight: Knight::Sit,
            busy: false,
            entrance: Entrance::Kick,
        })
    }

    /// The cell under a pointer on the card. The card lies flat and
    /// where its cells are, so this is the cell itself, or `None` off it.
    pub fn hit(&self, col: u16, row: u16) -> Option<(u16, u16)> {
        let inside = col >= self.area.x
            && col < self.area.right()
            && row >= self.area.y
            && row < self.area.bottom();
        inside.then_some((col, row))
    }
}

// --- the timeline -------------------------------------------------------------
//
// Seconds. The kick is the squire's own (`squire::IMPACT`); the title
// sequence is laid out here.

/// The title: the name fades up, its line grows out from the middle, the
/// squire walks the line, then the name lifts away and the card unfolds
/// down from the line.
pub(crate) const NAME_IN: (f32, f32) = (0.10, 0.75);
pub(crate) const LINE_GROW: (f32, f32) = (0.30, 1.05);
pub(crate) const WALK: (f32, f32) = (0.70, 1.75);
pub(crate) const NAME_OUT: (f32, f32) = (1.85, 2.25);
pub(crate) const UNFOLD: (f32, f32) = (2.00, 2.55);
/// A pop: the card scales up from just under its size and clears.
pub(crate) const POP: f32 = 0.22;
/// Going: the card fades, settling a little as it goes.
pub(crate) const CLOSE: f32 = 0.32;
/// A page's words: each row a beat after the one above.
pub(crate) const ROW_DELAY: f32 = 0.022;
pub(crate) const ROW_IN: f32 = 0.30;
/// How far a row rises as it comes, in cells of height.
pub(crate) const ROW_RISE: f32 = 0.45;
/// The card's height follows a page's over this long.
pub(crate) const RESIZE: f32 = 0.26;

/// `t` through `(from, to)`, clamped to 0..1.
pub(crate) fn span(t: f32, (from, to): (f32, f32)) -> f32 {
    ((t - from) / (to - from).max(1e-6)).clamp(0.0, 1.0)
}

pub(crate) fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

pub(crate) fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// Out past the mark and back, gently: the card settling into place.
pub(crate) fn ease_out_back(t: f32) -> f32 {
    let (c1, t) = (1.2, t.clamp(0.0, 1.0) - 1.0);
    1.0 + (c1 + 1.0) * t.powi(3) + c1 * t.powi(2)
}

/// When the card's words may start to come in, after it arrives.
pub(crate) fn words_from(entrance: Entrance) -> f32 {
    match entrance {
        Entrance::Kick => crate::squire::IMPACT + 0.08,
        Entrance::Title => UNFOLD.0 + 0.22,
        Entrance::Pop => 0.06,
    }
}

/// How long the entrance runs, the squire's settling included.
pub(crate) fn entrance_end(entrance: Entrance) -> f32 {
    match entrance {
        Entrance::Kick => crate::squire::INTRO_END,
        Entrance::Title => UNFOLD.1 + 0.1,
        Entrance::Pop => POP,
    }
}

/// A row's place in its page's arrival, `t` seconds after the words may
/// start: how far in (0 to 1), eased. Row `row` waits its beat.
pub(crate) fn row_in(t: f32, row: u16) -> f32 {
    ease_out(span(t - ROW_DELAY * f32::from(row), (0.0, ROW_IN)))
}

/// The pill's glide: a critically damped spring toward `target`, stepped
/// by `dt`. Returns the new place and speed.
pub(crate) fn glide(at: f32, speed: f32, target: f32, dt: f32) -> (f32, f32) {
    const OMEGA: f32 = 26.0;
    let dt = dt.clamp(0.0, 0.05);
    let mut at = at;
    let mut speed = speed;
    // Fixed small steps keep it stable at any display rate.
    let steps = (dt / (1.0 / 240.0)).ceil().max(1.0) as u32;
    let h = dt / steps as f32;
    for _ in 0..steps {
        let force = -OMEGA * OMEGA * (at - target) - 2.0 * OMEGA * speed;
        speed += force * h;
        at += speed * h;
    }
    if (at - target).abs() < 0.05 && speed.abs() < 0.5 {
        (target, 0.0)
    } else {
        (at, speed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_is_bounded_and_hits_its_own_cells() {
        let area = Rect::new(10, 5, 80, 20);
        let card = Card::new(area, Buffer::empty(Rect::new(0, 0, 80, 20))).unwrap();
        assert_eq!(card.hit(10, 5), Some((10, 5)));
        assert_eq!(card.hit(89, 24), Some((89, 24)));
        assert_eq!(card.hit(90, 10), None);
        assert_eq!(card.hit(9, 10), None);
        assert!(Card::new(area, Buffer::empty(Rect::new(0, 0, 79, 20))).is_none());
        assert!(Card::new(Rect::new(0, 0, 161, 20), Buffer::empty(Rect::new(0, 0, 161, 20))).is_none());
        assert!(Card::new(Rect::new(0, 0, 40, TITLE_ROWS), Buffer::empty(Rect::new(0, 0, 40, TITLE_ROWS))).is_none());
    }

    #[test]
    fn a_film_loops_or_holds_and_finds_its_frames_in_the_sheet() {
        let sheet = Arc::new(Picture { id: 1, width: 40, height: 20, rgba: vec![0; 40 * 20 * 4], cols: 0, rows: 0 });
        let mut film = Film { sheet, frame: (10, 10), frames: 6, cols: 4, fps: 10.0, once: false, at: Rect::new(0, 0, 4, 2) };
        assert_eq!(film.frame_at(0.0), 0);
        assert_eq!(film.frame_at(0.55), 5);
        assert_eq!(film.frame_at(0.65), 0, "a loop comes round");
        assert_eq!(film.source(5), [10.0, 10.0, 10.0, 10.0], "the second of the second row");
        film.once = true;
        assert_eq!(film.frame_at(9.0), 5, "once holds the last frame");
        film.fps = 0.0;
        assert_eq!(film.frame_at(9.0), 0, "a still holds its first");
    }

    #[test]
    fn rows_arrive_in_order_and_all_arrive() {
        assert_eq!(row_in(0.0, 0), 0.0);
        assert!(row_in(0.1, 0) > row_in(0.1, 3));
        assert_eq!(row_in(ROW_IN + ROW_DELAY * 30.0, 30), 1.0);
    }

    #[test]
    fn the_pill_settles_on_its_row_at_any_display_rate() {
        for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
            let (mut at, mut speed) = (0.0, 0.0);
            let mut overshoot: f32 = 0.0;
            for _ in 0..(1.0 / dt) as u32 {
                (at, speed) = glide(at, speed, 100.0, dt);
                overshoot = overshoot.max(at - 100.0);
            }
            assert_eq!((at, speed), (100.0, 0.0), "dt {dt}");
            assert!(overshoot < 0.5, "critically damped: no swing past the row");
        }
        // A stall never throws it.
        let (at, _) = glide(0.0, 0.0, 100.0, 5.0);
        assert!(at.is_finite() && at <= 100.0);
    }

    #[test]
    fn the_title_unfolds_the_card_after_the_walk() {
        assert!(WALK.1 <= UNFOLD.0 + 0.01 && NAME_IN.1 < NAME_OUT.0);
        assert!(words_from(Entrance::Title) > UNFOLD.0);
        assert!(words_from(Entrance::Pop) < words_from(Entrance::Kick));
        for e in [Entrance::Kick, Entrance::Title, Entrance::Pop] {
            assert!(entrance_end(e) >= words_from(e) - 0.1);
        }
    }
}
