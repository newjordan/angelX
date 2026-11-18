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

#[cfg(test)]
pub(crate) static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn here(run: &Run) -> Option<&BatShoot> {
    #[cfg(test)]
    if OFF.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    run.bats.as_ref().filter(|b| b.room == run.at)
}

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

pub(super) fn walled(run: &Run) -> bool {
    here(run).is_some()
}

pub(super) fn strip_bottom(run: &Run) -> Option<i32> {
    here(run).map(|_| hud::BOTTOM)
}

pub(super) fn floor(cv: &mut Img, run: &Run, view: (i32, i32)) {
    let Some(shoot) = here(run) else {
        return;
    };
    set::under(cv, run, shoot, view);
    fx::under(cv, run, shoot, view);
}

fn near((x, y): (i32, i32), (vx, vy): (i32, i32), pad: i32) -> bool {
    x >= vx - pad && x < vx + NATIVE_W + pad && y >= vy - pad && y < vy + NATIVE_H + pad
}

pub(super) fn over(cv: &mut Img, run: &Run, focus: Option<u32>, view: (i32, i32)) {
    let Some(shoot) = here(run) else {
        return;
    };
    fx::over(cv, run, shoot, focus, view);
}

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
