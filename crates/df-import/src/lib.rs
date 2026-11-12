//! Frank's Dwarf Fortress fortress, Bomrekedan, as game data for the Delve's
//! Bomrekedan expansion. Reads the export that
//! `tools/angelx-kingdom-export.lua` writes and produces the level files,
//! places, people, races and resources. Every output is sorted, so a re-run
//! on the same export is byte-identical.

pub mod input;
pub mod kingdom;
pub mod levelfile;
pub mod links;
pub mod rocks;
pub mod text;

pub use kingdom::run;
