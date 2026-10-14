//! A scene's paint order, said out loud.
//!
//! Each painter names the layer it belongs to; the list paints layer by
//! layer, and within a layer in the order painters were added. Where a call
//! sits in the scene function no longer decides what covers what: a new
//! painter adds one line with its layer. Inside `Figures`, monsters and
//! knights are a crowd sorted by where they stand (`figures_with`).

use super::Img;

/// Back to front.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Layer {
    /// On the floor: hazards, rooms' fixtures and folk, pools, pots, rings.
    Ground,
    /// What stands: loot, the fallen, monsters and knights, shots.
    Figures,
    /// Over the figures: sparks, flourishes, prompts, names, bursts.
    Over,
    /// The room's own boards and plaques, over everything in it.
    Boards,
    /// The screen's own marks after the camera's crop: arrows to the way
    /// on, standings, banners, panels.
    Screen,
}

type Paint<'a> = Box<dyn FnOnce(&mut Img) + 'a>;

#[derive(Default)]
pub(super) struct DrawList<'a> {
    commands: Vec<(Layer, Paint<'a>)>,
}

impl<'a> DrawList<'a> {
    pub(super) fn add(&mut self, layer: Layer, paint: impl FnOnce(&mut Img) + 'a) {
        self.commands.push((layer, Box::new(paint)));
    }

    /// Paint every command into `cv`, back layer first.
    pub(super) fn paint(mut self, cv: &mut Img) {
        // A stable sort: one layer keeps the order its painters came in.
        self.commands.sort_by_key(|(layer, _)| *layer);
        for (_, paint) in self.commands {
            paint(cv);
        }
    }
}
