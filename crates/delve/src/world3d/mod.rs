//! The world in 3D (docs/WORLD-3D.md): the Delve's places rebuilt as plain
//! blocks, captured from the game camera, painted over by the art lane, and
//! a painted plate cast back onto them from the camera it was painted for,
//! with the Delve's figures standing among them. Characters stay sprites;
//! only the world is blocked out.
//!
//! - [`blockout`]: one room of the game, as its rules lay it, made into a
//!   set file (`room_set`).
//! - [`chunks`]: the kingdom map cut into chunks of 24 x 16 tiles, each a
//!   set (`chunk_set`): ground by terrain, groves, rock, water, bridges,
//!   and a [`kit`] building on every place, castle and structure. Their
//!   sets live in `assets/world3d/chunk-<cx>-<cz>/`.
//! - [`kingdom`]: the kingdom drawn as its chunks, each set with its own
//!   plate, as many as a knight's view takes in; plates read as they come
//!   near and a few kept decoded.
//! - [`kit`]: parametric buildings from the primitive shapes, sized to a
//!   footprint and facing its door.
//! - [`pilot`]: the first ten items: six rooms of the kingdom's places and
//!   four of the dungeon's themed chambers, each with what the painters say
//!   its tiles are. Their sets live in `assets/world3d/<item>/<item>.set`.
//! - [`places`]: the rest of the kingdom's places (`world::ROOMS`).
//! - [`chambers`]: every other hand-drawn chamber of the seven delves
//!   (`rooms::*`), and the side rooms (Sanctuary, vault, forge-hall, nooks).
//!   [`pilot::every`] lists all of them; every set lives in
//!   `assets/world3d/<item>/<item>.set`.
//! - [`assets`]: the items as the asset folder holds them (read at run
//!   time, none compiled in), or as a host sent them; a room's set and plate.
//! - [`fit`]: whether a room is an item's room, and how it differs from it.
//! - [`lift`]: a lit room changing dimension (docs/REALM-MONOLITH.md, "Rooms,
//!   lit"): its masses lifting from cards on the floor into their solids,
//!   its flames catching last; its floor rising into relief as the sun comes
//!   down and its shadows grow from nothing, its lamps' pools blooming; the
//!   room tipping from flat to its tilt. Pure functions of time, applied to a
//!   frame the stage maker made.
//! - [`lots`]: what a run stands in the kingdom that no chunk's set has (the
//!   party's builds on its lots, the run's finds), as extra blocks.
//! - [`room`]: whether the host draws rooms so ([`enabled`]), and what a
//!   room hands a window and a terminal.
//! - [`plain`]: a room with no item, stood up as its own blockout in its
//!   theme's patches; an open floor's map in pieces.
//! - [`theme`]: the kinds of place, and each one's best patches of its
//!   painted items.
//! - [`realm`]: the kingdom's chunks in the monolith look
//!   (docs/REALM-MONOLITH.md), drawn lit with no plates: one ground
//!   ([`realm_ground`]), the monolith's building language ([`realm_mono`]),
//!   places ([`realm_place`]) and castles ([`realm_castle`]), and the look
//!   of a view ([`realm_look`]).
//! - [`rooms_lit`]: every room in the monolith look (docs/REALM-MONOLITH.md,
//!   "Rooms, lit"), the default 3D: its own blockout made monolithic
//!   ([`rooms_lit_mass`]), its ground and lamps, toned and lit through its
//!   kind of place's look ([`rooms_lit_look`]), no plate.

pub mod assets;
pub mod blockout;
pub mod chambers;
pub mod chunks;
pub mod fit;
pub mod kingdom;
pub mod kit;
pub mod lift;
pub mod lots;
pub mod pilot;
pub mod places;
pub mod plain;
pub mod realm;
pub mod realm_castle;
pub mod realm_ground;
pub mod realm_look;
pub mod realm_mono;
pub mod realm_place;
pub mod room;
pub mod rooms_lit;
pub mod rooms_lit_look;
pub mod rooms_lit_mass;
pub mod theme;

pub use room::{enabled, set_enabled};
