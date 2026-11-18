//! A scene's paint order, said out loud.
//!
//! Each painter names the layer it belongs to; the list paints layer by
//! layer, and within a layer in the order painters were added. Where a call
//! sits in the scene function no longer decides what covers what: a new
//! painter adds one line with its layer. Inside `Figures`, monsters and
//! knights are a crowd sorted by where they stand (`figures_with`).

use super::Img;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Layer {
    Ground,
    Figures,
    Over,
    Boards,
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

    pub(super) fn split(self, last: Layer) -> (DrawList<'a>, DrawList<'a>) {
        let (low, high) = self
            .commands
            .into_iter()
            .partition(|(layer, _)| *layer <= last);
        (DrawList { commands: low }, DrawList { commands: high })
    }

    pub(super) fn paint(mut self, cv: &mut Img) {
        self.commands.sort_by_key(|(layer, _)| *layer);
        for (_, paint) in self.commands {
            paint(cv);
        }
    }
}
