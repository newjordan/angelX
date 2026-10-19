//! Drawing: a run, or the realm's overworld, into a picture. Pure: the same
//! input draws the same bytes.

pub mod art;
pub mod escape;
pub mod braille;
pub mod overworld;
pub mod world3d;

#[cfg(any(test, feature = "testing"))]
pub mod arcade;

#[cfg(test)]
#[path = "../arcade_gate.rs"]
mod arcade_floor;
#[cfg(test)]
#[path = "../tests/render/arcade__gates.rs"]
mod arcade_tests;
