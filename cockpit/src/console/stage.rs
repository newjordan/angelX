use super::Shelf;
use angel_plug::stage::{StageArt, StagePlaces, StageRules, StageWorld};

impl StageWorld for Shelf {}

impl StagePlaces for Shelf {
    type Place = crate::ui::scryglass::Building;
}

impl StageArt for Shelf {}

impl StageRules for Shelf {}
