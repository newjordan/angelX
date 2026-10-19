//! Lanternward's stage: the Underkeep from above, one knight's own view.
//! Every actor and prop is a labelled block from the slot bank
//! (`assets/lanternward/slots.json`) until the painted frames arrive; the
//! knights are the Delve's own. `layers` is what a seat sees (384 by 224),
//! `terminal_layers` what the terminal's cell painter draws (the same
//! picture; the terminal prints its words over the top and bottom rows),
//! `kingdom` the door on the walking map. Art never sets a hitbox: the rules
//! say where everything is.
use super::super::ink::{Img, text_width};
use super::roundtable::label;
use crate::rules::{Run, lanternward as rules};

mod bank;
mod hud;
mod scene;

pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    (scene::draw(run, focus), Img::new(384, 224))
}

/// The cells draw the same room; the terminal's own text is printed over the
/// picture's top rows and its last two (`cockpit .../art/roundtable/lanternward.rs`).
pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    layers(run, focus)
}

/// The door on the walking map: a stone arch over a dark stair, a lantern
/// hung in it, the name over it and E under it. The lantern burns once the
/// Beacon has been lit.
pub(super) fn kingdom(im: &mut Img, run: &Run, origin: (i32, i32)) {
    let (x, y) = (
        rules::ENTRY.0 * 16 + 8 - origin.0,
        rules::ENTRY.1 * 16 + 8 - origin.1,
    );
    let name = rules::NAME.to_uppercase();
    if x < -(text_width(&name) / 2) - 8 || x > 392 + text_width(&name) / 2 || !(-24..248).contains(&y) {
        return;
    }
    // Posts, lintel, the dark between.
    im.rect(x - 9, y - 12, 4, 17, 'j');
    im.rect(x + 6, y - 12, 4, 17, 'j');
    im.rect(x - 10, y - 15, 21, 4, 'G');
    im.rect(x - 10, y - 15, 21, 1, 'J');
    im.rect(x - 5, y - 11, 11, 16, 'k');
    for step in 0..3 {
        im.rect(x - 4, y - 7 + step * 4, 9, 1, 'g');
    }
    // The hanging lantern.
    im.rect(x, y - 11, 1, 3, 'h');
    im.rect(x - 2, y - 8, 5, 6, 'g');
    let lit = run.home.roundtable[rules::SLOT] & rules::BEGUN != 0;
    im.rect(x - 1, y - 7, 3, 4, if lit { '6' } else { 'X' });
    if lit {
        im.put(x, y - 6, 'w');
        for (dx, dy) in [(-4, -6), (4, -6), (-3, -2), (3, -2)] {
            im.put(x + dx, y + dy, '4');
        }
    }
    label(im, x - text_width(&name) / 2, y - 28, &name, 'H');
    label(im, x - 2, y + 8, "E", '6');
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__lanternward__tests.rs"]
mod tests;
