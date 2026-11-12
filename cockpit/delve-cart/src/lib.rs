// The miniworld and the rest of the stage's realm live in the realm host
// (`crates/realm-host`); each keeps its old path here.
pub(crate) use angel_realm_host::{identity, knight_cast, village};
pub mod cart;
pub mod houses;
pub(crate) mod realm_hooks;
pub mod world_viz;

mod delve_hud;
pub mod delve_intro_viz;
pub(crate) mod joined_viz;
pub mod shooter_viz;
pub(crate) mod together_viz;

pub(crate) use angel_realm_host::game_pipeline;
pub(crate) use angel_delve::world::routing;

#[allow(unused_imports)]
pub(crate) use angel_realm_host::control::{
    DungeonView, Forge, Hosting, MAX_FRIENDS, MAX_WATCHED_FRIENDS, UPGRADES_PER_PAGE, Walk,
    known_wishes,
};

pub(crate) mod loop_summary;
pub(crate) mod realm_services;
pub mod settlement_input;
pub(crate) use angel_realm_host::chivalry;
pub(crate) use angel_realm_host::together;
pub(crate) use angel_realm_host::together_audio;
pub(crate) use angel_realm_host::together_appearance;
pub(crate) use angel_realm_host::together_abilities;
pub(crate) use angel_realm_host::together_loadouts;
pub(crate) use angel_realm_host::together_media;
pub(crate) use angel_realm_host::together_chorus;
pub(crate) use angel_realm_host::together_dungeon;
pub(crate) use angel_realm_host::together_grants;
#[cfg(feature = "glass")]
pub(crate) use angel_realm_host::together_sheets;
pub(crate) use angel_realm_host::together_join;
pub(crate) use angel_realm_host::together_realm;
pub mod together_rules;
pub mod together_salvage;
// The Delve's rules are `angel_delve`'s.
pub(crate) use angel_delve::rules as together_shooter;
pub(crate) use angel_realm_host::together_seat;
pub(crate) use angel_realm_host::together_sim;
pub mod together_web;

/// The Delve's settings, read from the environment once (`ANGEL_DELVE_FEEL=1` keeps the feel
/// ledger, open floors by default (`ANGEL_DELVE_OPEN=0` for flip-screen rooms); every floor 2D,
/// `ANGEL_WORLD3D=deep` the deep floors in 3D, `ANGEL_WORLD3D=1` every room,
/// docs/WORLD-3D.md "Deeper floors") and handed to `angel_delve`, which
/// reads no environment itself.
pub(crate) fn delve_settings_from_env() {
    angel_delve::settings::set(angel_delve::settings::Settings {
        feel_ledger: std::env::var("ANGEL_DELVE_FEEL").is_ok_and(|v| v == "1"),
        open_floors: std::env::var("ANGEL_DELVE_OPEN").map_or(true, |v| v != "0"),
    });
    angel_delve::world3d::room::set_mode(angel_delve::world3d::room::Mode::from_env(std::env::var("ANGEL_WORLD3D").ok().as_deref()));
    // ANGEL_BEND_RULES=1: the rules ported to Bend modules run as those modules.
    together_rules::arm_from_env();
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_boss_gates__tests.rs"]
mod together_boss_gates_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_feel__tests.rs"]
mod together_feel_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_hireling__tests.rs"]
mod together_hireling_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_polish__tests.rs"]
mod together_polish_tests;
