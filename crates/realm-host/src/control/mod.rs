//! The game's controls: keys, commands and the cadence of the Delve and the
//! walk on the map, written on [`Session`](crate::host::Session) (the game's
//! state with the cockpit's context for one call). They came out of the
//! cockpit's `app/control/dungeon*.rs` and `overworld_walk.rs`, where they
//! were `impl App` blocks; what they read of the cockpit's state is now
//! `self.cx.facts()` and what they change there is `self.cx.…`.

pub(crate) use crate::host::Session;
pub(crate) use crossterm::event::{self, KeyCode, KeyModifiers};
#[allow(unused_imports)]
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::Arc;
pub(crate) use angel_live::realm::Move;

mod dungeon;
pub mod dungeon_brew;
mod dungeon_chivalry;
mod dungeon_gear;
mod dungeon_intro;
mod dungeon_join;
mod dungeon_realm;
pub mod dungeon_reforge;
mod dungeon_salvage;
mod dungeon_session;
mod dungeon_shooter;
mod overworld_walk;
mod queries;

pub use dungeon::{DungeonView, Hosting, MAX_FRIENDS, MAX_WATCHED_FRIENDS};
pub use dungeon_reforge::{Forge, Lesson, MOST_IN_FLIGHT, Pick, UPGRADES_PER_PAGE, known_wishes};
#[cfg(any(test, feature = "testing"))]
pub use dungeon_reforge::bench_learn_brief;
pub use dungeon_reforge::ReforgeDraft;
pub use dungeon_realm::{economy, host_name};
pub use overworld_walk::Walk;
