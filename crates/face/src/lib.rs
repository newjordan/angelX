//! angelface: the goddess who watches over the work (docs/ANGELFACE.md).
//!
//! A rigged, blendshaped head ([`model`], read by [`glb`]) drawn in glass's
//! chrome ([`draw`]), held and moved by her [`mind`], and speaking from
//! Audio2Face [`track`]s. The cockpit runs her on her own thread and feeds
//! her what happens; nothing here touches a window.

pub mod draw;
pub mod glb;
pub mod limb;
pub mod mind;
pub mod model;
pub mod pose;
pub mod track;

pub use draw::Drawing;
pub use mind::{Cue, Mind, Moment};
pub use model::Goddess;
pub use pose::HeadPose;
pub use track::Track;
