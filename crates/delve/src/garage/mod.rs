//! The garage: where a knight builds their racer before the race, and the
//! race room's first room.
//!
//! The data, which the race reads:
//! - [`parts`]: the catalogue as data. Eight slots (engine, chariot, running
//!   gear, exhaust, crest, armour, paint, trim), every part with its stat
//!   effects and the shape its mesh is built from. The four racers of the
//!   knight racers example are the starting kit.
//! - [`racer`]: a knight's `Build` (one part id per slot; serde with defaults
//!   so an old save loads) and its `Stats`.
//! - [`ledger`]: every knight's racer kept by name, in plain data the host
//!   owns; [`store`] writes it down (atomically) after each swap and reads it
//!   back, in `.angel/dungeon/garage.json`. `Ledger::entrant(name)` is what
//!   the race takes: the build, its stats, and `Entrant::mesh` for its body.
//! - [`meshes`]: world3d meshes for every part and for the racer they make.
//!
//! The room, stepped by the host like the rest of the rules:
//! - [`shop`]: the shop floor: six lifts, racks round the walls, a bench, the
//!   one static camera and the lamps.
//! - [`room`]: the knights in it: they walk, stand at a rack, take a part
//!   with E (F puts it back, 1-4 fit a starting racer), and the ledger has it
//!   at once.
//!
//! Its picture (a pure function of the room):
//! - [`scene`]: one frame: the shop, the racers on their lifts with a swap
//!   playing (the part lifts off, the new one drops in, sparks), the knights,
//!   the part about to be taken, then the labels and stat panel over it;
//!   [`raster`] lights it per pixel under the shop's lamps, [`look`] is how
//!   its materials look, [`canvas`] draws the labels, [`dots`] makes braille.
//!
//! The race reads and writes it through [`bank`]: a knight's career (money, pods, records,
//! the parts they bought) and their time-trial [`ghost`]s are kept in the ledger by name, and
//! the bank carries them in and out of a running `knight_racers::Session` between its ticks.
//!
//! No clocks, no randomness: a swap, a save and a frame are functions of
//! their inputs. `examples/knight_garage.rs` dumps frames as PNGs.
//!
//! Not yet in live play: the room needs a place in the world, its state a
//! place on the wire, and a renderer for the window and the browser (see the
//! handoff). Nothing here touches `rules::Run`.

pub mod bank;
pub mod canvas;
pub mod dots;
pub mod ghost;
pub mod ledger;
pub mod look;
pub mod meshes;
pub mod parts;
pub mod prim;
pub mod raster;
pub mod racer;
pub mod room;
pub mod scene;
pub mod shop;
pub mod store;

pub use ledger::{Entrant, Ledger, Swap, SwapError};
pub use parts::{PARTS, Part, Slot};
pub use racer::{Build, Stats};
pub use bank::Bank;
pub use store::Store;

#[cfg(test)]
#[path = "../../tests/garage/garage__tests.rs"]
mod tests;
