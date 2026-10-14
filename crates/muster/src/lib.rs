//! The muster: the crowd test game. Every player is a knight on one field
//! 4096 pixels square; the stick walks, FIRE waves, USE plants or lifts
//! your banner. Sixty sheep wander among them so interest management has
//! work beyond the players. The server ticks at 20 Hz and replicates in
//! entity mode; the client predicts its own knight through the same
//! `Knight::step` the server runs, and draws 320×180 in software.
//!
//! The client half (`client`, `rules`) builds for wasm; the server half
//! and `muster-host` are native.

pub mod client;
mod font;
pub mod rules;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;

pub use client::MusterClient;
pub use rules::MusterCmd;
#[cfg(not(target_arch = "wasm32"))]
pub use server::MusterServer;

/// Raised when the welcome's params change: an old page then says to reload.
pub const PARAMS_VERSION: u64 = 1;
