pub(crate) use angel_delve_cart::*;

#[cfg(test)]
pub(crate) use angel_realm_host::{identity, village};
#[cfg(test)]
pub(crate) use angel_delve::world::routing;
#[cfg(test)]
pub(crate) use angel_realm_host::control::dungeon_reforge;
#[cfg(test)]
pub(crate) use angel_realm_host::control::ReforgeDraft;
#[cfg(test)]
pub(crate) use angel_realm_host::loop_summary;
#[cfg(test)]
pub(crate) use angel_realm_host::chivalry;
#[cfg(test)]
pub(crate) use angel_realm_host::together_abilities;
#[cfg(test)]
pub(crate) use angel_realm_host::together_media;
#[cfg(test)]
pub(crate) use angel_realm_host::together_files;
#[cfg(test)]
pub(crate) use angel_realm_host::together_gear;
#[cfg(test)]
pub(crate) use angel_realm_host::together_grants;
#[cfg(test)]
pub(crate) use angel_realm_host::together_realm;
#[cfg(test)]
pub(crate) use angel_delve::rules as together_shooter;
#[cfg(test)]
pub(crate) use angel_realm_host::together_seat;
#[cfg(test)]
pub(crate) use angel_realm_host::together_sim;

#[cfg(test)]
mod realm_test_support;
#[cfg(test)]
mod deathmatch_rules;
// King Cinder's tick as it was, held against the leg that replaced it.
#[cfg(test)]
#[path = "bannerpeak_rules_tests.rs"]
mod bannerpeak_rules_tests;
// Griffinwing's tick as it was, held against the module that replaced it.
#[cfg(test)]
#[path = "../../../tests/cockpit/drive/griffinwing_rules__tests.rs"]
mod griffinwing_rules_tests;

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
#[path = "../../../tests/cockpit/app/together_lightgallop__tests.rs"]
mod together_lightgallop_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_floor_plates__tests.rs"]
mod together_floor_plates_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/drive/together_abilities__tests.rs"]
mod together_abilities_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_layout__tests.rs"]
mod together_layout_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_secrets__tests.rs"]
mod together_secrets_tests;
// The whole party on the engine, headless: it plays the host's terminal and so
// needs the cockpit's `App`.
#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_sim__e2e.rs"]
mod together_sim_e2e_tests;

#[cfg(test)]
mod houses_tests;
#[cfg(test)]
mod loop_summary_tests;
#[cfg(test)]
mod cart_drive_tests;
#[cfg(test)]
mod realm_services_tests;
#[cfg(test)]
mod together_rules_tests;
#[cfg(test)]
mod together_salvage_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/world_viz/lightgallop__tests.rs"]
mod lightgallop_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/world_viz/lanternward__tests.rs"]
mod lanternward_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/world_viz/roundtable__tests.rs"]
mod roundtable_tests;
#[cfg(test)]
#[path = "../../../tests/cockpit/world_viz/overworld__art_cells_tests.rs"]
mod art_cells_tests;
// The world's tests that need the cockpit (a `LoopState`, the scryglass, the
// realm's catalog, a guest's picture): they run here, against its public paths.
#[cfg(test)]
#[path = "../../../tests/cockpit/world_viz/coupled__tests.rs"]
mod coupled_tests;

/// The 3D races' moved rules: the oracle and its parity with the Bend leg.
#[cfg(test)]
mod delve_rules_tests;
