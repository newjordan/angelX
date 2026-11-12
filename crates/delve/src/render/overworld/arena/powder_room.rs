//! The Powder Room as a seat sees it (`rules::powder`): the room is a
//! powder store, its stone and crates laid with the room's scenery
//! (`walls`); kegs with their fuses, the red lanes of a keg about to go,
//! the soot of old blasts and the lanterns drawn live under the knights
//! (`set`); the blasts themselves, fireballs, flames, smoke, flying
//! planks and sparks (`boom`); a caught knight's flash and flight home
//! (`fx`); the strip, the store's bar, the little map and the title,
//! countdown and callouts over the view (`hud`, `moments`, with every word
//! kept apart by `words`); and once the minute is out, the live finish,
//! the store going up, and then the ruin's yard with the podium (`finish`).

use super::*;
use crate::rules::powder::Powder;

mod bits;
mod boom;
mod finish;
mod fx;
mod hud;
mod moments;
mod set;
pub(super) mod walls;
pub(crate) mod words;

/// A test's switch to draw the room as it was before the store (for the
/// frame-time bench's other side).
#[cfg(test)]
pub(crate) static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The powder store in the room the party stands in.
fn here(run: &Run) -> Option<&Powder> {
    #[cfg(test)]
    if OFF.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    run.powder.as_ref().filter(|p| p.room == run.at)
}

/// The milliseconds each part of the store takes to draw one frame for
/// `focus`, for the frame-time bench: the live set and the blasts under the
/// figures, the blasts over them, the strip and the moments.
#[cfg(test)]
pub(crate) fn profile(run: &Run, focus: Option<u32>) -> [(&'static str, f64); 3] {
    use std::time::Instant;
    let mut out = [("floor", 0.0), ("over", 0.0), ("screen", 0.0)];
    let (w, h) = super::room_px(run.room());
    let mut cv = Img::new(w, h);
    let view = camera::view_origin(run, focus);
    let t = Instant::now();
    floor(&mut cv, run, focus, view);
    out[0].1 = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    over(&mut cv, run, focus, view);
    out[1].1 = t.elapsed().as_secs_f64() * 1000.0;
    let mut sc = Img::new(NATIVE_W, NATIVE_H);
    let t = Instant::now();
    screen(&mut sc, run, focus, view);
    out[2].1 = t.elapsed().as_secs_f64() * 1000.0;
    out
}

/// Whether the room the party stands in is a powder store (its stone and
/// crates are laid by `walls`, with the room's scenery).
pub(super) fn walled(run: &Run) -> bool {
    here(run).is_some()
}

/// Whether the skills bar keeps out of the view while the store is played:
/// the little map and the strip have the edges.
pub(super) fn tucks_tray(run: &Run) -> bool {
    here(run).is_some_and(|p| !p.done || finish::age(p, run.tick as u32).is_some())
}

/// How far down the view the top marks reach, for boards that settle
/// under them.
pub(super) fn strip_bottom(run: &Run) -> Option<i32> {
    here(run).map(|_| hud::BOTTOM)
}

/// The store's set, live, over the scenery and under the figures: kegs and
/// their lanes, soot, lanterns, and the blasts' light and rings on the stone.
pub(super) fn floor(cv: &mut Img, run: &Run, focus: Option<u32>, view: (i32, i32)) {
    let Some(p) = here(run) else {
        return;
    };
    set::under(cv, run, p, view);
    finish::ground(cv, run, p, focus, view);
}

/// What the blasts do, over the figures (room pixels).
pub(super) fn over(cv: &mut Img, run: &Run, focus: Option<u32>, view: (i32, i32)) {
    let Some(p) = here(run) else {
        return;
    };
    boom::over(cv, &p.show.blasts, run.tick as u32, view, run.tick);
    fx::over(cv, run, p, focus, view);
    finish::over(cv, run, p, focus, view);
}

/// The view's marks: the strip and the store's bar, the little map and the
/// moments over the view; once the minute is out, the live finish and then
/// the yard.
pub(super) fn screen(over: &mut Img, run: &Run, focus: Option<u32>, origin: (i32, i32)) {
    let Some(p) = here(run) else {
        return;
    };
    let now = run.tick as u32;
    let finish = finish::age(p, now);
    let mut led = words::Ledger::default();
    if let Some(e) = finish.filter(|&e| e >= finish::FINALE) {
        over.stamp(&finish::scene(&mut led, run, p, e, focus), 0, 0);
        led.keep();
        return;
    }
    moments::danger(over, run, p, focus, now);
    hud::strip(&mut led, over, run, p, focus);
    hud::minimap(over, run, p, focus, origin);
    hud::hand(over, p, focus, now);
    // Once the minute is out the finish has the screen to itself.
    if p.show.ended.is_none() {
        moments::last_seconds(&mut led, over, p);
    }
    if !p.done {
        moments::opening(&mut led, over, run, p);
    }
    if p.show.ended.is_none() {
        moments::callouts(&mut led, over, run, p, focus, now);
    }
    moments::fly_ups(&mut led, over, p, focus, origin, now);
    moments::sootings(&mut led, over, p, origin, now);
    if let Some(e) = finish {
        finish::finale(&mut led, over, run, p, e, focus);
    }
    led.keep();
}
