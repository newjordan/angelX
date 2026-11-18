//! The overworld as the cockpit lives it: the realm's top-down pixel map,
//! with the live world walking on it.
//!
//! The map itself (its scene, staging and drawing) is the Delve's
//! (`angel_delve::render::overworld`), read here as this module's own. The
//! cockpit's parts are what turns its work into the map's facts: the
//! knight's walk and the scene the live world builds (`live`), the deeds'
//! traffic (`deeds`), the outcome receipts (`outcomes`), the graph garden
//! (`garden`) and the visitor's walk (`visitor`), and the overlay that
//! draws the garden and the visitor's door sign over the map.

#![cfg_attr(not(test), allow(dead_code))]

pub use angel_delve::render::overworld::*;

#[allow(hidden_glob_reexports)]
mod deeds;
pub mod garden;
mod live;
#[allow(hidden_glob_reexports)]
mod outcomes;
mod visitor;

pub use deeds::Deeds;
#[cfg(test)]
pub use glass::{GLASS_H, GLASS_W, picture_from_rgba};
pub use live::{Duel, Walker, soldier_state};
pub use outcomes::Outcomes;
pub use visitor::{DoorOpens, Opens, Visitor};

#[cfg(test)]
use kit::Tool;
#[cfg(test)]
use map::{Place, Realm};
#[cfg(test)]
use scene::{Joust, Knight, Soldier, Ward, Weather};

#[derive(Debug)]
pub struct MiniViz {
    pub garden: garden::GardenFrame,
}

impl Overlay for MiniViz {
    fn paint_ground(&self, cv: &mut Img, view: View) {
        garden::paint_ground(cv, &self.garden, view);
    }

    fn stage(&self, props: &mut Vec<scene::Prop>, cues: &mut Vec<scene::Prop>) {
        garden::stage(&self.garden, props, cues);
    }

    fn paint_top(&self, cv: &mut Img, scene: &Scene, view: View) {
        visitor::paint_cue(cv, scene, view);
    }

    fn key(&self, mut h: &mut dyn std::hash::Hasher) {
        use std::hash::Hash;
        (
            self.garden.epoch,
            self.garden.tick,
            self.garden.motion as u8,
        )
            .hash(&mut h);
    }
}

pub fn glass_enabled() -> bool {
    std::env::var("ANGEL_WORLD_GLASS").map_or(true, |raw| {
        !matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    })
}

pub fn map_enabled() -> bool {
    std::env::var("ANGEL_WORLD_MAP").map_or(true, |raw| {
        !matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "3d" | "dotmax" | "off" | "0" | "false" | "no"
        )
    })
}

#[cfg(test)]
#[path = "../../tests/world_viz/overworld__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/world_viz/overworld__castles_tests.rs"]
mod castles_tests;
#[cfg(test)]
#[path = "../../tests/world_viz/overworld__school_tests.rs"]
mod school_tests;
