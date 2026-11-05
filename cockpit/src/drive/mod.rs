//! Unattended controllers that run the engine over many turns.

pub(crate) mod campaign;
pub(crate) mod comp_mode;
#[allow(dead_code)]
// Contract-first scaffold; consumers land in Wave 1 (board-sync is the first live path).
pub(crate) mod competition;
pub(crate) mod conductor;
pub(crate) mod continual_harness;
#[allow(dead_code)]
pub(crate) mod deli;
pub(crate) mod goal;
pub(crate) mod graph_ctl;
pub(crate) mod guardian;
pub(crate) mod habits;
pub(crate) mod handoff_rl;
pub(crate) mod iterate;
pub(crate) mod labyrinth;
pub(crate) mod loop_ctl;
pub(crate) mod loop_dialog;
pub(crate) mod loop_summary;
pub(crate) mod realm_services;
pub(crate) mod realm_support;
#[allow(dead_code)] // reinforce-loop core; wired to DICE/GEPA in a follow-up
pub(crate) mod reinforce;
pub(crate) mod research_workspace;
pub(crate) mod rl_ctl;
pub(crate) mod round_table;
pub(crate) mod science;
pub(crate) mod self_loop;
pub(crate) mod settlement_input;
pub(crate) use angel_delve::realm::forge as together_forge;
pub(crate) use angel_realm_host::chivalry;
pub(crate) use angel_realm_host::together;
pub(crate) use angel_realm_host::together_abilities;
pub(crate) use angel_realm_host::together_appearance;
pub(crate) use angel_realm_host::together_audio;
pub(crate) use angel_realm_host::together_avatar;
pub(crate) use angel_realm_host::together_chorus;
pub(crate) use angel_realm_host::together_door;
pub(crate) use angel_realm_host::together_dungeon;
pub(crate) use angel_realm_host::together_files;
pub(crate) use angel_realm_host::together_gear;
pub(crate) use angel_realm_host::together_grants;
pub(crate) use angel_realm_host::together_join;
pub(crate) use angel_realm_host::together_loadouts;
pub(crate) use angel_realm_host::together_media;
pub(crate) use angel_realm_host::together_painter;
pub(crate) use angel_realm_host::together_realm;
pub(crate) use angel_realm_host::together_sheets;
pub(crate) mod together_rules;
pub(crate) use angel_realm_host::together_frames;
pub(crate) use angel_realm_host::together_visuals;
#[cfg(test)]
mod deathmatch_rules;
pub(crate) mod together_salvage;
pub(crate) mod together_settlement;
// The Delve's rules are `angel_delve`'s.
pub(crate) use angel_delve::rules as together_shooter;
pub(crate) use angel_realm_host::together_seat;
// King Cinder's tick as it was, held against the leg that replaced it.
#[cfg(test)]
#[path = "bannerpeak_rules_tests.rs"]
mod bannerpeak_rules_tests;
// Griffinwing's tick as it was, held against the module that replaced it.
#[cfg(test)]
#[path = "../../../tests/cockpit/drive/griffinwing_rules__tests.rs"]
mod griffinwing_rules_tests;
pub(crate) use angel_realm_host::together_sim;
pub(crate) mod together_web;

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
    angel_delve::world3d::room::set_mode(angel_delve::world3d::room::Mode::from_env(
        std::env::var("ANGEL_WORLD3D").ok().as_deref(),
    ));
    // ANGEL_BEND_RULES=1: the rules ported to Bend modules run as those modules.
    together_rules::arm_from_env();
}

// The Delve's tests that need the cockpit: its realm and its save, its
// screens, its sounds and its keys.
#[cfg(test)]
#[path = "../../../tests/cockpit/app/delve_moonfall_rules__tests.rs"]
mod delve_moonfall_rules_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/delve_rooms_rules__tests.rs"]
mod delve_rooms_rules_tests;
// The Lightgallop's rules as a Bend module, against the Rust they came from.
#[cfg(test)]
#[path = "../../../tests/cockpit/drive/together_abilities__tests.rs"]
mod together_abilities_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_boss_gates__tests.rs"]
mod together_boss_gates_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_feel__tests.rs"]
mod together_feel_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_floor_plates__tests.rs"]
mod together_floor_plates_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_hireling__tests.rs"]
mod together_hireling_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_layout__tests.rs"]
mod together_layout_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_lightgallop__tests.rs"]
mod together_lightgallop_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_polish__tests.rs"]
mod together_polish_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_secrets__tests.rs"]
mod together_secrets_tests;
// The whole party on the engine, headless: it plays the host's terminal and so
// needs the cockpit's `App`.
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_sim__e2e.rs"]
mod together_sim_e2e_tests;

/// The 3D races' moved rules: the oracle and its parity with the Bend leg.
#[cfg(test)]
mod delve_rules_tests;
