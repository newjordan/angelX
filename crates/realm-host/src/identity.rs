//! The realm's identity: what its places and surfaces are called. The HUD
//! palette they wear lives in `ui::hud`.
//!
//! Behavior, rendering algorithms, classifiers, cache keys, and environment
//! controls live in their owning modules. This leaf module contains only realm
//! identity data and trivial table accessors.

use crate::world_viz::Building;

/// Display and rendering identity for one world landmark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandmarkIdentity {
    pub kind: Building,
    pub display_name: &'static str,
    pub rgb: (u8, u8, u8),
    pub index: u8,
    pub arrive_activity: &'static str,
    pub facade_material: u8,
    pub facade_span: i32,
    pub landmark_height: f32,
    pub district_fraction: (f64, f64),
}

/// The realm's landmarks, in the stable order used by the travel renderer.
pub const LANDMARKS: [LandmarkIdentity; 8] = [
    LandmarkIdentity {
        kind: Building::Keep,
        display_name: "Keep courtyard",
        rgb: (255, 207, 92),
        index: 0,
        arrive_activity: "resting in the keep",
        facade_material: 4,
        facade_span: 2,
        landmark_height: 1.8,
        district_fraction: (0.50, 0.55),
    },
    LandmarkIdentity {
        kind: Building::Gatehouse,
        display_name: "Gatehouse",
        rgb: (255, 103, 126),
        index: 1,
        arrive_activity: "at the gatehouse",
        facade_material: 4,
        facade_span: 2,
        landmark_height: 1.15,
        district_fraction: (0.08, 0.50),
    },
    LandmarkIdentity {
        kind: Building::Rookery,
        display_name: "Rookery",
        rgb: (91, 190, 255),
        index: 2,
        arrive_activity: "sealing scrolls at the rookery",
        facade_material: 3,
        facade_span: 1,
        landmark_height: 1.6,
        district_fraction: (0.30, 0.50),
    },
    LandmarkIdentity {
        kind: Building::Scriptorium,
        display_name: "Scriptorium",
        rgb: (184, 228, 255),
        index: 3,
        arrive_activity: "studying in the scriptorium",
        facade_material: 3,
        facade_span: 1,
        landmark_height: 1.2,
        district_fraction: (0.55, 0.18),
    },
    LandmarkIdentity {
        kind: Building::Smithy,
        display_name: "Smithy",
        rgb: (255, 207, 92),
        index: 4,
        arrive_activity: "at work in the smithy",
        facade_material: 3,
        facade_span: 1,
        landmark_height: 1.25,
        district_fraction: (0.80, 0.45),
    },
    LandmarkIdentity {
        kind: Building::Chapel,
        display_name: "Chapel",
        rgb: (255, 103, 126),
        index: 5,
        arrive_activity: "praying in the chapel",
        facade_material: 4,
        facade_span: 1,
        landmark_height: 1.5,
        district_fraction: (0.68, 0.80),
    },
    LandmarkIdentity {
        kind: Building::RoundTable,
        display_name: "Round Table",
        rgb: (99, 241, 169),
        index: 6,
        arrive_activity: "council at the Round Table",
        facade_material: 4,
        facade_span: 2,
        landmark_height: 1.1,
        district_fraction: (0.35, 0.82),
    },
    LandmarkIdentity {
        kind: Building::Observatory,
        display_name: "Observatory",
        rgb: (86, 232, 255),
        index: 7,
        arrive_activity: "charting the heavens at the observatory",
        facade_material: 4,
        facade_span: 2,
        landmark_height: 1.3,
        district_fraction: (0.18, 0.20),
    },
];

pub const fn landmark(kind: Building) -> &'static LandmarkIdentity {
    &LANDMARKS[kind as usize]
}

pub const fn district_fractions() -> [(f64, f64); 8] {
    [
        LANDMARKS[0].district_fraction,
        LANDMARKS[1].district_fraction,
        LANDMARKS[2].district_fraction,
        LANDMARKS[3].district_fraction,
        LANDMARKS[4].district_fraction,
        LANDMARKS[5].district_fraction,
        LANDMARKS[6].district_fraction,
        LANDMARKS[7].district_fraction,
    ]
}

// Stage chrome and world-flavor copy. Keep byte-identical until the rename wave.
pub const STAGE_TITLE_REINFORCE_PREFIX: &str = " Realm / Tiltyard · ";
pub const STAGE_TITLE_REINFORCE_SUFFIX: &str = " ";
pub const STAGE_TITLE_QUEST_BOARD: &str = " Realm / Quest Board ";
pub const STAGE_TITLE_ROUNDTABLE_PREFIX: &str = " Realm / Round Table · ";
pub const STAGE_TITLE_SMITHY: &str = " Realm / Smithy / Workshop ";
pub const STAGE_TITLE_RAYTRACE: &str = " Tools / Raytrace ";
pub const STAGE_TITLE_QUINTAIN_PREFIX: &str = " Realm / Quintain · ";
pub const STAGE_TITLE_OBSERVATORY_PREFIX: &str = " Realm / Observatory · ";
pub const STAGE_TITLE_DYNAMIC_SUFFIX: &str = " ";
pub const STAGE_TITLE_VAULT: &str = " Realm / Vault ";
pub const STAGE_TITLE_ARRIVAL: &str = "ARRIVAL";
pub const STAGE_TITLE_EXPLORE: &str = "RIDE";
pub const STAGE_TITLE_REALM: &str = "REALM";
/// The world pane's kind while the adventure owns it (Z5). Off Castle Town the
/// pane is not a ride around the realm, it is the quest — and `World::title()`
/// already names the region, so the title reads `◈ Scryglass · QUEST · The
/// Mines · …`.
pub const STAGE_TITLE_QUEST: &str = "QUEST";
pub const VILLAGE_HEADING_PREFIX: &str = "◇ the village of ";
pub const VILLAGE_HEADING_SUFFIX: &str = " — as above, so below\n";

// Toponym parts. Selection remains in world_viz so this module stays data-only.
pub const TOWN_HEAD: [&str; 12] = [
    "Cam", "Tin", "Ast", "Car", "Lyon", "Ben", "Gal", "Cor", "Ava", "Bed", "Mal", "Dol",
];
pub const TOWN_MID: [&str; 6] = ["e", "a", "o", "i", "er", "or"];
pub const TOWN_TAIL: [&str; 8] =
    ["lot", "gel", "fort", "mere", "gard", "wick", "shire", "dun"];

// Display-only quest flavor. Theme keys remain in questmap's classifier table.
pub const QUEST_ARENA_OVERWORLD: &str = "the king's road";
pub const QUEST_ARENA_DUNGEON: &str = "the undercroft";
pub const QUEST_ARENA_VOYAGE: &str = "the grail sea";

pub const QUEST_PARTY_OVERWORLD: [&str; 8] = [
    "Sir Kay",
    "Sir Bedivere",
    "Sir Gawain",
    "Sir Percival",
    "Sir Bors",
    "Sir Lamorak",
    "Sir Tristan",
    "Sir Galahad",
];
pub const QUEST_PARTY_DUNGEON: [&str; 8] = [
    "Lancelot", "Galahad", "Gawain", "Percival", "Tristan", "Bors", "Kay", "Bedivere",
];
pub const QUEST_PARTY_VOYAGE: [&str; 8] = [
    "the Petrel",
    "the Cormorant",
    "the Kestrel",
    "the Albatross",
    "the Gannet",
    "the Fulmar",
    "the Skua",
    "the Tern",
];

pub const QUEST_LEAGUE: &str = "league";
pub const QUEST_LEAGUES: &str = "leagues";
pub const QUEST_CHAMBER: &str = "chamber";
pub const QUEST_CHAMBERS: &str = "chambers";
pub const QUEST_DAY_OUT: &str = "day out";
pub const QUEST_DAYS_OUT: &str = "days out";
pub const QUEST_FALLS: &str = "falls";
pub const QUEST_TURNS_BACK: &str = "turns back";
pub const QUEST_REACHES_CASTLE: &str = "reaches the castle";
pub const QUEST_OVERWORLD_WANDERING: &str = "still wandering when the chronicle ends";
pub const QUEST_CASTLE_WON: &str = "the castle is won";
pub const QUEST_CASTLE_UNTAKEN: &str = "the castle stands untaken";
pub const QUEST_HITS_DEAD_END: &str = "hits a dead end";
pub const QUEST_DOUBLES_BACK: &str = "doubles back";
pub const QUEST_LIFTS_TREASURE: &str = "lifts the treasure";
pub const QUEST_TORCH_LIT: &str = "torch still lit when the chronicle ends";
pub const QUEST_TREASURE_CLAIMED: &str = "the treasure is claimed";
pub const QUEST_VAULT_SEALED: &str = "the vault stays sealed";
pub const QUEST_WRECKS: &str = "wrecks";
pub const QUEST_COMES_ABOUT: &str = "comes about";
pub const QUEST_MAKES_PORT: &str = "makes port";
pub const QUEST_STILL_AT_SEA: &str = "still at sea when the chronicle ends";
pub const QUEST_LANDFALL: &str = "landfall";
pub const QUEST_NO_LANDFALL: &str = "no landfall";

#[cfg(test)]
#[path = "../tests/identity__tests.rs"]
mod tests;
