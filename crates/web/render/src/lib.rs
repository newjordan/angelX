//! The browser seat's renderer: `angel-gpu`, the core the native window draws
//! with, behind a canvas, as a small wasm module beside the game's
//! (docs/GLASS_CORE.md, decision 6).
//!
//! The page hands it what it hands the legacy presenter today: a stage frame
//! (`angel-stage`'s wire format, which the game's wasm already makes) or a
//! finished one. It decodes the stage (`angel_stage::wire::decode`), turns its
//! quads back into layers and lets `angel-gpu` draw them the way the window
//! does: nothing of the tilt, stand, slide or blend maths is written here.
//!
//! Two builds (cargo features `webgpu` and `webgl`, see `Cargo.toml`) are
//! made by `crates/web/build.sh` and picked by `shell/boot.js`. On the host
//! only [`place`], the box a frame is shown in, is compiled, so its tests run
//! with everything else.

pub mod place;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Renderer;
