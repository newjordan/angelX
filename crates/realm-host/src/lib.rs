//! The realm's host side, cut out of the cockpit binary crate so an edit to
//! the game and an edit to the cockpit stop rebuilding each other
//! (`docs/architecture/modularity-map.md`).
//!
//! What lives here needs nothing of the cockpit. The Delve's browser page is
//! built by this crate's `build.rs` (a nested wasm build of `crates/web/delve`)
//! and embedded in [`together_web`]; where shipped files sit and how a missing
//! one is reported are lent by the cockpit ([`together_web::Places`]).
//!
//! The stage is here too: the miniworld ([`world_viz`]) and what it draws on
//! (`hearth`, `village`, `knight_cast`, `identity`, the settlement ledger
//! `together_settlement`, the subtitle script `together_chorus` and the loop
//! facts `loop_summary`). The few answers only the cockpit has are lent as
//! [`hooks`].
//!
//! The delve's host is here too: its sim and server ([`together_sim`]), the
//! seats that play on it ([`together_seat`], [`together_join`]), the realm and
//! its save ([`together_realm`]), the wish grants and painters, the live
//! loadouts, abilities and appearances, the sounds, sheets and files it reads.
//! What only the cockpit can do for them (a model to ask, an owned child
//! process, the window's paint door) is lent as [`services`].
//!
//! The Delve's controls (keys, commands, the cadence of the cockpit's frames)
//! are here as well ([`control`]), written on a [`host::Session`]: the game's
//! state ([`host::GameHost`]) with the cockpit's context for one call
//! (`angel_live::realm::HostCx`). The cockpit drives it through
//! `angel_live::realm::RealmHost`.

pub mod card_pages;
pub mod chivalry;
pub mod control;
pub mod delve_intro;
pub mod game_pipeline;
pub mod hearth;
pub mod hooks;
pub mod host;
pub mod hud_palette;
pub mod identity;
pub mod knight_cast;
pub mod loop_summary;
pub mod plug;
pub mod realm_standing;
pub mod retro_noise;
pub mod services;
pub mod together;
pub mod together_abilities;
pub mod together_appearance;
pub mod together_audio;
pub mod together_avatar;
pub mod together_chorus;
pub use angel_games::door as together_door;
pub mod together_dungeon;
pub mod together_files;
pub mod together_frames;
pub mod together_gear;
pub mod together_grants;
pub mod together_join;
pub mod together_loadouts;
pub mod together_media;
pub mod together_motion;
pub mod together_looker;
pub mod together_painter;
pub mod together_realm;
pub mod together_seat;
pub mod together_settlement;
pub mod together_sheets;
pub mod together_sim;
pub mod together_visuals;
pub mod together_web;
pub mod village;
pub mod world_viz;

/// The host stress kit's timing lines (`scripts/stress/host-kit`): with
/// `ANGEL_STRESS_LOG=1` the checkpoint, the claim cycle and the save gate
/// say on stderr how long each step took. Read once; off, it is a load.
pub(crate) fn stress_log() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ANGEL_STRESS_LOG").is_some_and(|v| v == "1"))
}

/// The shared data types the game and the cockpit both read.
pub use angel_live::telemetry;

/// The Delve's rules and the forge's runes are `angel_delve`'s; the moved
/// modules read them by these names.
pub use angel_delve::realm::forge as together_forge;
pub use angel_delve::rules as together_shooter;

/// Where the cockpit's shipped assets sit (`cockpit/assets/...`): the install
/// paths the cockpit lent the Kernel's host services at startup. This crate's
/// own tests (and a cockpit test build, which asks for `testing`) lend the
/// source tree they were built in.
pub(crate) fn cockpit_dir() -> std::path::PathBuf {
    lend_source_tree();
    angel_live::host::runtime_paths::cockpit_dir()
}

/// The package or checkout the shipped files sit in (the parent of
/// `cockpit/`), found as the cockpit finds it.
pub(crate) fn root() -> std::path::PathBuf {
    lend_source_tree();
    angel_live::host::runtime_paths::root()
}

fn lend_source_tree() {
    #[cfg(any(test, feature = "testing"))]
    angel_live::host::runtime_paths::install(angel_live::host::runtime_paths::BuildPaths {
        source_cockpit_dir: Some(concat!(env!("CARGO_MANIFEST_DIR"), "/../../cockpit")),
        bundle_identity: "unbound",
    });
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../tests/together_barony__tests.rs"]
mod together_barony_tests;
#[cfg(test)]
#[path = "../tests/together_classes__tests.rs"]
mod together_classes_tests;
#[cfg(test)]
#[path = "../tests/together_votes__tests.rs"]
mod together_votes_tests;

mod together_salvage;
