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

pub mod client;
pub mod realm;
pub mod render;
pub mod rules;
pub mod settings;
pub mod wire;
pub mod world;
pub mod world3d;
