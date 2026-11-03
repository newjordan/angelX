//! Kiln: a recipe in, the realm's pixels out, at any grain (docs/KILN.md).
//!
//! The shared ground every part of kiln stands on lives here: the realm's
//! inks and ramps ([`palette`]), the grain a picture is drawn at
//! ([`grain`]) and the indexed picture with depth that shapes draw into
//! ([`raster`]). Everything is deterministic: the same recipe gives the same
//! pixels on every machine.
//!
//! On it stand the hands: a figure's [`recipe`] (docs/KILN-RECIPE.md) of
//! lit volumes ([`shape`], [`light`]) on a [`rig`], drawn with its line work
//! ([`line`]) by [`render`]; the effects ([`fx`]: emitters, smears, rings,
//! bolts, flames, glows; docs/KILN-FX.md); and the eyes: shots,
//! descriptions, lints and comparisons ([`look`]), and the game's formats
//! ([`export`]). With the `encode` feature, the PNG and GIF writers
//! ([`encode`]).

#[cfg(feature = "encode")]
pub mod encode;
pub mod export;
pub mod fx;
pub mod grain;
pub mod light;
pub mod line;
pub mod look;
pub mod num;
pub mod palette;
pub mod raster;
pub mod recipe;
pub mod render;
pub mod rig;
pub mod shape;

pub use grain::Grain;
pub use palette::{Ink, Palette, Ramp};
pub use raster::Raster;
pub use recipe::Recipe;
