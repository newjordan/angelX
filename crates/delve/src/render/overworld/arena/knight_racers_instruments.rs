//! Camera and arcade instruments for the live six-seat pod race.
//! All animation comes from simulation ticks: remote seats and replays agree.
//!
//! The game itself lives in `knight_racers_game/`: the pod roster, the camera,
//! the HUD, the race flow and the clocks. This file mounts them and is the
//! one door `layers` uses.
use super::*;

#[path = "knight_racers_game/pods.rs"]
pub(super) mod pods;
#[path = "knight_racers_game/camera.rs"]
pub(super) mod camera;
#[path = "knight_racers_game/fx.rs"]
pub(super) mod fx;
#[path = "knight_racers_game/state.rs"]
pub(super) mod state;
#[path = "knight_racers_game/damage_fx.rs"]
mod damage_fx;
#[path = "knight_racers_game/garage.rs"]
mod garage;
#[path = "knight_racers_game/hud.rs"]
mod hud;
#[path = "knight_racers_game/flow.rs"]
mod flow;
#[path = "knight_racers_game/ui/mod.rs"]
mod ui;
#[cfg(test)]
#[path = "knight_racers_game/proof.rs"]
mod proof;
#[cfg(test)]
#[path = "knight_racers_game/law_proof.rs"]
mod law_proof;
#[cfg(test)]
#[path = "knight_racers_game/feel_proof.rs"]
mod feel_proof;

#[cfg(test)]
pub(super) use camera::chase;
pub(super) use camera::shot;
/// The feel marks (plumes, sparks, dust, streaks, the boost's vignette) painted over the raster,
/// under the instruments.
pub(super) use fx::draw as feel;
pub(in super::super) use garage::garage;

/// Paint the instruments and the race's moments for `seat` over the picture.
/// `depth` is the raster's depth buffer, so a tag never floats over a crest.
pub(super) fn instruments(im: &mut Img, race: &rules::Session, car: &rules::Car, seat: u32, view: &View3, depth: &[f32]) {
    // A menu over the race (the title, the modes, the courses) has the screen to itself.
    if ui::overlay(im, race, seat) { return; }
    let clocks = state::observe(race);
    let order = state::standings(race);
    let ctx = hud::Ctx { race, car, seat, view, depth, order: &order, clocks: &clocks };
    hud::draw(im, &ctx);
    flow::draw(im, &ctx);
    ui::ghost(im, race, car, seat, view, depth);
}

/// A whole screen that takes the place of the race frame: pod select, garage, records, podium.
pub(super) fn screens(race: &rules::Session, focus: Option<u32>) -> Option<Img> { ui::full(race, focus) }
