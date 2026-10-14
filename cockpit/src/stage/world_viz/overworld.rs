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

pub(crate) use angel_delve::render::overworld::*;

mod deeds;
pub(crate) mod garden;
mod live;
mod outcomes;
mod visitor;

// The overworld tests read these through `use super::*`; live.rs imports glass directly.
pub(crate) use deeds::Deeds;
#[cfg(test)]
pub(crate) use glass::{GLASS_H, GLASS_W, picture_from_rgba};
pub(crate) use live::{Duel, Walker, soldier_state};
pub(crate) use outcomes::Outcomes;
pub(crate) use visitor::{DoorOpens, Opens, Visitor};

#[cfg(test)]
use kit::Tool;
#[cfg(test)]
use map::{Place, Realm};
#[cfg(test)]
use scene::{Joust, Knight, Soldier, Ward, Weather};

/// What the cockpit lays over the map: its graph garden, and the walking
/// visitor's door sign.
#[derive(Debug)]
pub(crate) struct MiniViz {
    pub(crate) garden: garden::GardenFrame,
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

/// Whether travel and arrival show their glass over the map
/// (`ANGEL_WORLD_GLASS=off` keeps the map bare).
pub(crate) fn glass_enabled() -> bool {
    std::env::var("ANGEL_WORLD_GLASS").map_or(true, |raw| {
        !matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    })
}

/// Whether the Realm route shows this map (the default) or the Dotmax 3D
/// ride as before (`ANGEL_WORLD_MAP=3d`).
pub(crate) fn map_enabled() -> bool {
    std::env::var("ANGEL_WORLD_MAP").map_or(true, |raw| {
        !matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "3d" | "dotmax" | "off" | "0" | "false" | "no"
        )
    })
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__tests.rs"]
mod tests;

// The Delve's drawing tests that need the cockpit: its realm, a guest's
// picture.
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__castles_tests.rs"]
mod castles_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__school_tests.rs"]
mod school_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__wish_placement_tests.rs"]
mod wish_placement_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__plates_tests.rs"]
mod plates_tests;
