//! The Belfry Shoot as a seat sees it (`rules::folly`): the room is a bell
//! tower's loft, its stone and boards laid with the room's scenery (`walls`)
//! and the great bell, lanterns, dust and roosting bats drawn live over it
//! (`set`); a shot's flash, feathers and numbers around the bats and the
//! knights (`fx`); the strip, the table's bar and the title, countdown,
//! callouts and last seconds over the view (`hud`, `moments`); and once the
//! minute is out the live finish and then the loft with the podium
//! (`finish`).

use super::*;
use crate::rules::folly::BatShoot;

mod bits;
mod finish;
mod fx;
mod hud;
mod moments;
mod set;
pub(super) mod walls;

/// A test's switch to draw the room as it was before the belfry (for the
/// frame-time bench's other side).
#[cfg(test)]
pub(crate) static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The shoot in the room the party stands in.
fn here(run: &Run) -> Option<&BatShoot> {
    #[cfg(test)]
    if OFF.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    run.bats.as_ref().filter(|b| b.room == run.at)
}

/// The milliseconds each part of the belfry takes to draw one frame for
/// `focus`, for the frame-time bench: the live set and the shots under the
/// figures, the shots over them, the strip and the moments.
#[cfg(test)]
pub(crate) fn profile(run: &Run, focus: Option<u32>) -> [(&'static str, f64); 3] {
    use std::time::Instant;
    let mut out = [("floor", 0.0), ("over", 0.0), ("screen", 0.0)];
    let (w, h) = super::room_px(run.room());
    let mut cv = Img::new(w, h);
    let t = Instant::now();
    floor(&mut cv, run, (0, 0));
    out[0].1 = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    over(&mut cv, run, focus, (0, 0));
    out[1].1 = t.elapsed().as_secs_f64() * 1000.0;
    let mut sc = Img::new(NATIVE_W, NATIVE_H);
    let t = Instant::now();
    screen(&mut sc, run, focus, (0, 0));
    out[2].1 = t.elapsed().as_secs_f64() * 1000.0;
    out
}

/// Whether the room the party stands in is a belfry (its stone and boards
/// are laid by `walls`, with the room's scenery).
pub(super) fn walled(run: &Run) -> bool {
    here(run).is_some()
}

/// How far down the view the top marks reach, for boards that settle
/// under them.
pub(super) fn strip_bottom(run: &Run) -> Option<i32> {
    here(run).map(|_| hud::BOTTOM)
}

/// The belfry's set, live, over the scenery and under the figures, and the
/// dust and rings the shots raise on the boards.
pub(super) fn floor(cv: &mut Img, run: &Run, view: (i32, i32)) {
    let Some(shoot) = here(run) else {
        return;
    };
    set::under(cv, run, shoot, view);
    fx::under(cv, run, shoot, view);
}

/// Whether room pixel `(x, y)` is near enough the view `view` to be worth
/// drawing (`pad` pixels of slack for what reaches out of its point).
fn near((x, y): (i32, i32), (vx, vy): (i32, i32), pad: i32) -> bool {
    x >= vx - pad && x < vx + NATIVE_W + pad && y >= vy - pad && y < vy + NATIVE_H + pad
}

/// What the shots do, over the figures (room pixels).
pub(super) fn over(cv: &mut Img, run: &Run, focus: Option<u32>, view: (i32, i32)) {
    let Some(shoot) = here(run) else {
        return;
    };
    fx::over(cv, run, shoot, focus, view);
}

/// The view's marks: the strip and the table's bar, the moments over the
/// view; once the minute is out, the live finish and then the loft.
pub(super) fn screen(over: &mut Img, run: &Run, focus: Option<u32>, origin: (i32, i32)) {
    let Some(shoot) = here(run) else {
        return;
    };
    let now = run.tick as u32;
    let finish = finish::age(shoot, now);
    if let Some(e) = finish.filter(|&e| e >= finish::FINALE) {
        over.stamp(&finish::scene(run, shoot, e, focus), 0, 0);
        return;
    }
    moments::danger(over, run, shoot, focus, now);
    hud::strip(over, run, shoot, focus);
    moments::fly_ups(over, shoot, focus, origin, now);
    // Once the minute is out the finish has the screen to itself.
    if shoot.show.ended.is_none() {
        moments::last_seconds(over, shoot);
        moments::callouts(over, run, shoot, focus, now);
    }
    if !shoot.done {
        moments::opening(over, run, shoot);
    }
    if let Some(e) = finish {
        finish::finale(over, run, shoot, e, focus);
    }
}
