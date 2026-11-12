//! The Delve: the party's dungeon, the kingdom above it, and how both are
//! drawn. One crate for the host and every client (a browser's wasm, a
//! joined angelX), so the same rules and the same pictures run everywhere.
//! See `docs/ENGINE.md`.
//!
//! - [`rules`]: the game, a `Run` stepped by its inputs.
//! - [`render`]: a run (or the realm's overworld) drawn into an image.
//! - [`world`]: the kingdom's map and geography the rules walk on, and the
//!   great houses.
//! - [`world3d`]: rooms rebuilt as blocks with a painted plate cast on them
//!   (docs/WORLD-3D.md), behind the host's switch.
//! - [`realm`]: the plain data the party's realm keeps between runs.
//! - [`settings`]: what the host sets once, before play.
//! - [`client`]: the Delve as an engine client: snapshots in, a seat's own
//!   screen out, its hands as commands.
//! - [`wire`]: the run on the engine's wire: its tree, each seat's share of
//!   it, its events and the welcome's params.
//!
//! No files, environment, clocks or processes: the host reads those and
//! hands over plain data. Threads are only a native speed-up; a wasm build
//! draws the same pixels on one thread.

#[cfg(feature = "renderer")]
pub mod client;
// A beginner's first session, played and measured (docs/FIRST-SESSION.md).
#[cfg(all(feature = "renderer", any(test, feature = "testing")))]
#[path = "../tests/first_session__bot.rs"]
pub mod first_session;
#[cfg(feature = "renderer")]
pub mod garage;
pub mod realm;
pub use angel_kingdom as kingdom;

#[cfg(feature = "renderer")]
pub mod render;
pub mod rules;
pub mod scripted;
pub mod settings;
#[cfg(test)]
mod test_alloc;
#[cfg(feature = "renderer")]
pub mod wire;
pub mod world;
#[cfg(feature = "renderer")]
pub mod world3d;

/// A stand-in for a wish handler: one number the rules hand a running game.
pub fn probe_value() -> u32 {
    2
}

// Shared palette/noise primitives are also used by model-free rules and map
// generation. The worker builds these without compiling the renderer.
#[path = "render/overworld/ink.rs"]
pub mod ink;
// The motion foundation's eases, beats, springs and curves (docs/MOTION.md)
// are pure arithmetic, so the rules may time things by them too.
#[path = "render/motion.rs"]
pub mod motion;
