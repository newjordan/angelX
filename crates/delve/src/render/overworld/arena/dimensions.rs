//! Powers change dimensions (docs/WORLD-3D.md, "Powers change
//! dimensions"): the hook, not yet the whole of it.
//!
//! A knight's shot in a window is a few solid shapes (`shot_meshes`) lying
//! low over the floor, as the 2D black dungeon has them. It is drawn
//! [`Dimension::Raised`] instead, stood up in the room's space at a
//! knight's chest, larger and rolling about its flight so its depth shows,
//! when the floor it flies on stands up in 3D (`world3d::room::stands`), or
//! when its knight's powers have climbed a wish's ladder to
//! [`RAISED_TIER`] (the tier that is taught, not wished). A monster's shot
//! keeps low. One function decides it ([`of_shot`]); what each dimension
//! looks like is [`Dimension::lift`], [`Dimension::scale`] and
//! [`Dimension::roll`]; more dimensions, and powers that change in other
//! ways than their shots, hang off the same two places.

use crate::rules::{Hero, Projectile, Run};

pub const RAISED_TIER: u8 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dimension {
    #[default]
    Low,
    Raised,
}

impl Dimension {
    pub fn lift(self) -> f32 {
        match self {
            Dimension::Low => super::shot_meshes::LIFT,
            Dimension::Raised => 9.0,
        }
    }

    pub fn scale(self) -> f32 {
        match self {
            Dimension::Low => 1.0,
            Dimension::Raised => 1.3,
        }
    }

    pub fn roll(self) -> f32 {
        match self {
            Dimension::Low => 0.0,
            Dimension::Raised => std::f32::consts::TAU / 48.0,
        }
    }
}

pub fn wish_tier(hero: &Hero) -> u8 {
    hero.deck
        .iter()
        .filter_map(|card| card.strip_prefix("wish-")?.rsplit_once('-')?.1.parse::<u8>().ok())
        .max()
        .unwrap_or(0)
}

pub struct Dimensions {
    deep: bool,
    taught: Vec<u32>,
}

impl Dimensions {
    pub fn of(run: &Run) -> Dimensions {
        Dimensions {
            deep: crate::world3d::room::stands_here(run),
            taught: run.players.iter().filter(|(_, hero)| wish_tier(hero) >= RAISED_TIER).map(|(&id, _)| id).collect(),
        }
    }

    pub fn shot(&self, shot: &Projectile) -> Dimension {
        let owner = u32::from(shot.traits.owner);
        if !shot.hostile && (self.deep || (owner > 0 && self.taught.contains(&owner))) {
            Dimension::Raised
        } else {
            Dimension::Low
        }
    }
}

pub fn of_shot(run: &Run, shot: &Projectile) -> Dimension {
    Dimensions::of(run).shot(shot)
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__dimensions_tests.rs"]
mod tests;
