//! Fortune's dare as a card: two lines of plain words and her purse in
//! pictures, a coin and a gem, set low in the view's left corner where no
//! game keeps a banner or a count (the strips run along the top, the boards
//! top right, the calls in the middle, the tray below). It slides in off the
//! left edge, stays its few seconds, and slides out; it never covers another
//! game's words, and it keeps out of a finished game's own screen.

use super::super::super::chrome;
use super::super::super::ink::{Img, text_width};
use super::claims;
use crate::motion::{ease, share};
use crate::rules::HZ;
use crate::rules::Run;
use crate::rules::dares::{CALLED, Dare};

const SLIDE: f32 = 8.0;
const MARGIN: i32 = 4;
const TRAY: i32 = 32;

pub(super) fn card(dare: &Dare) -> Img {
    let (name, rule) = (dare.kind.card_name(), dare.kind.card_rule());
    let text = text_width(name).max(text_width(rule));
    let w = 5 + 11 + 5 + text + 6 + 12 + 4;
    let h = 2 * 9 + 7;
    let mut im = Img::new(w + 1, h + 1);
    chrome::panel(&mut im, (0, 0, w, h), chrome::Tone::EMBER);
    let wheel = Img::from_rows(&[
        "...ooooo...",
        ".oo5.O.5oo.",
        ".o.5.O.5.o.",
        "o5..5O5..5o",
        "o..5.5.5..o",
        "oOOO5666OOo",
        "o..5.5.5..o",
        "o5..5O5..5o",
        ".o.5.O.5.o.",
        ".oo5.O.5oo.",
        "...ooooo...",
    ]);
    im.stamp(&wheel, 5, (h - wheel.h) / 2);
    let x = 5 + wheel.w + 5;
    im.text(x, 4, name, 'i');
    im.text(x, 13, rule, 'h');
    let (px, py) = (w - 4 - 11, (h - 5) / 2);
    im.stamp(&Img::from_rows(&[".666.", "65556", "65456", "65556", ".666."]), px, py);
    im.stamp(&Img::from_rows(&["..3..", ".323.", "32223", ".323.", "..3.."]), px + 7, py);
    im
}

pub(super) fn rest(card: &Img, h: i32) -> (i32, i32) {
    (MARGIN, h - TRAY - MARGIN - card.h)
}

pub(super) fn age(run: &Run) -> Option<(Dare, f32)> {
    let after = (run.tune.dare_after * HZ as f32).round().max(0.0) as u64;
    let dare = run.dare.clone()?;
    let from = dare.since + after;
    (run.tick >= from && run.tick < from + CALLED).then(|| {
        let age = (run.tick - from) as f32;
        (dare, age)
    })
}

fn finished(run: &Run) -> bool {
    let here = run.at;
    run.folly.as_ref().is_some_and(|f| f.room == here && f.done)
        || run.bats.as_ref().is_some_and(|f| f.room == here && f.done)
        || run.powder.as_ref().is_some_and(|f| f.room == here && f.done)
        || run.hop.as_ref().is_some_and(|f| f.room == here && f.done)
        || run.gantry.as_ref().is_some_and(|f| f.room == here && f.done)
}

pub(crate) fn board(cv: &mut Img, run: &Run) {
    let Some((dare, age)) = age(run) else { return };
    if finished(run) {
        return;
    }
    let im = card(&dare);
    let (rx, ry) = rest(&im, cv.h);
    let enter = ease::out(share(age, 0.0, SLIDE));
    let leave = ease::in_quad(share(age, CALLED as f32 - SLIDE, SLIDE));
    let x = rx - ((im.w + rx) as f32 * ((1.0 - enter) + leave)).round() as i32;
    #[cfg(any(test, feature = "testing"))]
    if x + im.w > 0 {
        crate::ink::said::board(im.w, im.h);
    }
    cv.stamp(&im, x, ry);
    if enter >= 1.0 && leave <= 0.0 {
        claims::note_img("dare", &im, rx, ry);
    }
}
