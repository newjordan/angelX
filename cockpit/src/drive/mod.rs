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
pub(crate) mod habits;
pub(crate) mod handoff_rl;
pub(crate) mod iterate;
pub(crate) mod labyrinth;
pub(crate) mod loop_ctl;
pub(crate) mod loop_dialog;
#[allow(dead_code)] // reinforce-loop core; wired to DICE/GEPA in a follow-up
pub(crate) mod reinforce;
pub(crate) mod research_workspace;
pub(crate) mod round_table;
pub(crate) mod rl_ctl;
pub(crate) mod science;
pub(crate) mod self_loop;
pub(crate) mod chivalry;
pub(crate) mod together;
pub(crate) mod together_audio;
pub(crate) mod together_avatar;
pub(crate) mod together_chorus;
pub(crate) mod together_dungeon;
pub(crate) use angel_delve::realm::forge as together_forge;
pub(crate) mod together_door;
pub(crate) mod together_files;
pub(crate) mod together_gear;
pub(crate) mod together_sheets;
pub(crate) mod together_join;
pub(crate) mod together_realm;
pub(crate) mod together_settlement;
// The Delve's rules are `angel_delve`'s.
pub(crate) use angel_delve::rules as together_shooter;
pub(crate) mod together_seat;
pub(crate) mod together_sim;
pub(crate) mod together_web;

/// The Delve's settings, read from the environment once (`ANGEL_ART=v2`
/// draws with the second art kit, `ANGEL_DELVE_FEEL=1` keeps the feel
/// ledger, `ANGEL_DELVE_OPEN=1` lays every delve floor open; the deep floors
/// stand up in 3D, `ANGEL_WORLD3D=1` every room, `ANGEL_WORLD3D=0` none,
/// docs/WORLD-3D.md "Deeper floors") and handed to `angel_delve`, which
/// reads no environment itself.
pub(crate) fn delve_settings_from_env() {
    angel_delve::settings::set(angel_delve::settings::Settings {
        art_v2: std::env::var_os("ANGEL_ART").is_some_and(|v| v == "v2"),
        feel_ledger: std::env::var("ANGEL_DELVE_FEEL").is_ok_and(|v| v == "1"),
        open_floors: std::env::var("ANGEL_DELVE_OPEN").is_ok_and(|v| v == "1"),
    });
    angel_delve::world3d::room::set_mode(angel_delve::world3d::room::Mode::from_env(std::env::var("ANGEL_WORLD3D").ok().as_deref()));
}

// The Delve's tests that need the cockpit: its realm and its save, its
// screens, its sounds and its keys.
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_barony__tests.rs"]
mod together_barony_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_boss_gates__tests.rs"]
mod together_boss_gates_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_classes__tests.rs"]
mod together_classes_tests;
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
#[path = "../../../tests/cockpit/app/together_polish__tests.rs"]
mod together_polish_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_secrets__tests.rs"]
mod together_secrets_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_votes__tests.rs"]
mod together_votes_tests;
