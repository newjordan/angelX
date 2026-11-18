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

#[cfg(test)]
pub(crate) static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const DARE_ZONE: (i32, i32, i32, i32) = (0, 160, 168, 32);

fn here(run: &Run) -> Option<&Hop> {
    #[cfg(test)]
    if OFF.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    run.hop.as_ref().filter(|h| h.room == run.at)
}

pub(super) fn walled(run: &Run) -> bool {
    here(run).is_some()
}

pub(super) fn tucks_tray(run: &Run) -> bool {
    here(run).is_some_and(|h| !h.done || finish::age(h, run.tick as u32).is_some())
}

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

#[cfg(test)]
pub(crate) fn strip_top() -> i32 {
    hud::TOP
}

#[cfg(test)]
pub(crate) fn chip_xs(run: &Run) -> Vec<(u32, i32)> {
    here(run).map(|h| hud::chips(h).iter().map(|c| (c.id, c.x)).collect()).unwrap_or_default()
}

pub(super) fn floor(cv: &mut Img, run: &Run) {
    let Some(h) = here(run) else {
        return;
    };
    set::under(cv, run, h);
}

pub(super) fn over(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(h) = here(run) else {
        return;
    };
    fx::over(cv, run, h, focus);
}

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
    if h.show.ended.is_none() {
        moments::last_seconds(&mut led, over, h);
    }
    if !h.done {
        moments::opening(&mut led, over, run, h);
    }
    if h.show.ended.is_none() {
        moments::callouts(&mut led, over, run, h, focus, now);
    }
    if h.show.ended.is_none() {
        moments::fly_ups(&mut led, over, run, h, focus, now);
        moments::gloops(&mut led, over, h, focus, now);
    }
    if let Some(e) = finish {
        finish::finale(&mut led, over, run, h, e, focus);
    }
    led.keep();
}
