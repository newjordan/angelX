//! Book Hop as a seat sees it (`rules::hop`, `rules::hop_show`): the hall is
//! the drowned archive, bookcases all round with owls along the top shelf,
//! a black river with its current and the pages lost on it, tomes drifting
//! in lanes with their wake, a landing between them and a lantern hung over
//! the far bank for each seat that must cross (`set`); a landing's thud, a
//! knight going under, a crossing's gold, the archive flaring when the table
//! has all crossed, an arrow over every knight (`fx`); the strip along the
//! foot, the title, countdown, callouts and last seconds over the view, with
//! every word kept apart by `words` (`hud`, `moments`); and once the minute
//! is out, the live finish, then the reading room with the podium
//! (`finish`). The skills bar is tucked away while it is played: the strip
//! has the foot.

use super::*;
use crate::rules::hop::Hop;

mod bits;
mod finish;
mod fx;
mod hud;
mod moments;
mod set;
pub(crate) mod words;

/// A test's switch to draw the room as it was before the archive (for the
/// frame-time bench's other side).
#[cfg(test)]
pub(crate) static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Where the Fortune dare card rests in a view (bottom left, above the
/// tray): no word of ours is ever laid there, so it never meets one.
const DARE_ZONE: (i32, i32, i32, i32) = (0, 160, 168, 32);

/// The hop in the room the party stands in.
fn here(run: &Run) -> Option<&Hop> {
    #[cfg(test)]
    if OFF.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    run.hop.as_ref().filter(|h| h.room == run.at)
}

/// Whether the room the party stands in is the drowned archive (its stone,
/// water and books are drawn here; the old board and floor stand down).
pub(super) fn walled(run: &Run) -> bool {
    here(run).is_some()
}

/// Whether the skills bar keeps out of the view while the hop is played:
/// the strip has the foot.
pub(super) fn tucks_tray(run: &Run) -> bool {
    here(run).is_some_and(|h| !h.done || finish::age(h, run.tick as u32).is_some())
}

/// The milliseconds each part of the archive takes to draw one frame for
/// `focus`, for the frame-time bench: the hall under the knights, the
/// moments over them, the strip and the words.
#[cfg(test)]
pub(crate) fn profile(run: &Run, focus: Option<u32>) -> [(&'static str, f64); 3] {
    use std::time::Instant;
    let mut out = [("floor", 0.0), ("over", 0.0), ("screen", 0.0)];
    let (w, h) = super::room_px(run.room());
    let mut cv = Img::new(w, h);
    let t = Instant::now();
    floor(&mut cv, run);
    out[0].1 = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    over(&mut cv, run, focus);
    out[1].1 = t.elapsed().as_secs_f64() * 1000.0;
    let mut sc = Img::new(NATIVE_W, NATIVE_H);
    let t = Instant::now();
    screen(&mut sc, run, focus, (0, 0));
    out[2].1 = t.elapsed().as_secs_f64() * 1000.0;
    out
}

/// The row the strip's top is on.
#[cfg(test)]
pub(crate) fn strip_top() -> i32 {
    hud::TOP
}

/// Where each knight's chip begins in the strip, for the test of who is `YOU`.
#[cfg(test)]
pub(crate) fn chip_xs(run: &Run) -> Vec<(u32, i32)> {
    here(run).map(|h| hud::chips(h).iter().map(|c| (c.id, c.x)).collect()).unwrap_or_default()
}

/// The hall, live, under the knights.
pub(super) fn floor(cv: &mut Img, run: &Run) {
    let Some(h) = here(run) else {
        return;
    };
    set::under(cv, run, h);
}

/// What the hop does over the knights (room pixels).
pub(super) fn over(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(h) = here(run) else {
        return;
    };
    fx::over(cv, run, h, focus);
}

/// The view's marks: the strip and the moments over the view; once the
/// minute is out, the live finish and then the reading room.
pub(super) fn screen(over: &mut Img, run: &Run, focus: Option<u32>, _origin: (i32, i32)) {
    let Some(h) = here(run) else {
        return;
    };
    let now = run.tick as u32;
    let finish = finish::age(h, now);
    let mut led = words::Ledger::default();
    if let Some(e) = finish.filter(|&e| e >= finish::FINALE) {
        over.stamp(&finish::scene(&mut led, run, h, e, focus), 0, 0);
        led.keep();
        return;
    }
    if !h.done {
        led.claim("dare", DARE_ZONE);
    }
    moments::danger(over, h, focus, now);
    hud::strip(&mut led, over, run, h, focus);
    // Once the minute is out the finish has the screen to itself.
    if h.show.ended.is_none() {
        moments::last_seconds(&mut led, over, h);
    }
    if !h.done {
        moments::opening(&mut led, over, run, h);
    }
    if h.show.ended.is_none() {
        moments::callouts(&mut led, over, run, h, focus, now);
    }
    // Once the minute is out the finish has the screen to itself.
    if h.show.ended.is_none() {
        moments::fly_ups(&mut led, over, run, h, focus, now);
        moments::gloops(&mut led, over, h, focus, now);
    }
    if let Some(e) = finish {
        finish::finale(&mut led, over, run, h, e, focus);
    }
    led.keep();
}
