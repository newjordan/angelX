//! The realm's identity: what its places and surfaces are called, and the
//! palette they wear.
//!
//! Behavior, rendering algorithms, classifiers, cache keys, and environment
//! controls live in their owning modules. This leaf module contains only realm
//! identity data and trivial table accessors.

use crate::world_viz::Building;
use ratatui::style::Color;

/// Display and rendering identity for one world landmark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LandmarkIdentity {
    pub(crate) kind: Building,
    pub(crate) display_name: &'static str,
    pub(crate) rgb: (u8, u8, u8),
    pub(crate) index: u8,
    pub(crate) arrive_activity: &'static str,
    pub(crate) facade_material: u8,
    pub(crate) facade_span: i32,
    pub(crate) landmark_height: f32,
    pub(crate) district_fraction: (f64, f64),
}

/// The realm's landmarks, in the stable order used by the travel renderer.
pub(crate) const LANDMARKS: [LandmarkIdentity; 8] = [
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

pub(crate) const fn landmark(kind: Building) -> &'static LandmarkIdentity {
    &LANDMARKS[kind as usize]
}

pub(crate) const fn district_fractions() -> [(f64, f64); 8] {
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
pub(crate) const STAGE_TITLE_REINFORCE_PREFIX: &str = " Realm / Tiltyard · ";
pub(crate) const STAGE_TITLE_REINFORCE_SUFFIX: &str = " ";
pub(crate) const STAGE_TITLE_QUEST_BOARD: &str = " Realm / Quest Board ";
pub(crate) const STAGE_TITLE_ROUNDTABLE_PREFIX: &str = " Realm / Round Table · ";
pub(crate) const STAGE_TITLE_SMITHY: &str = " Realm / Smithy / Workshop ";
pub(crate) const STAGE_TITLE_RAYTRACE: &str = " Tools / Raytrace ";
pub(crate) const STAGE_TITLE_QUINTAIN_PREFIX: &str = " Realm / Quintain · ";
pub(crate) const STAGE_TITLE_OBSERVATORY_PREFIX: &str = " Realm / Observatory · ";
pub(crate) const STAGE_TITLE_DYNAMIC_SUFFIX: &str = " ";
pub(crate) const STAGE_TITLE_VAULT: &str = " Realm / Vault ";
pub(crate) const STAGE_TITLE_ARRIVAL: &str = "ARRIVAL";
pub(crate) const STAGE_TITLE_EXPLORE: &str = "RIDE";
pub(crate) const STAGE_TITLE_REALM: &str = "REALM";
/// The world pane's kind while the adventure owns it (Z5). Off Castle Town the
/// pane is not a ride around the realm, it is the quest — and `World::title()`
/// already names the region, so the title reads `◈ Scryglass · QUEST · The
/// Mines · …`.
pub(crate) const STAGE_TITLE_QUEST: &str = "QUEST";
pub(crate) const VILLAGE_HEADING_PREFIX: &str = "◇ the village of ";
pub(crate) const VILLAGE_HEADING_SUFFIX: &str = " — as above, so below\n";

// Toponym parts. Selection remains in world_viz so this module stays data-only.
pub(crate) const TOWN_HEAD: [&str; 12] = [
    "Cam", "Tin", "Ast", "Car", "Lyon", "Ben", "Gal", "Cor", "Ava", "Bed", "Mal", "Dol",
];
pub(crate) const TOWN_MID: [&str; 6] = ["e", "a", "o", "i", "er", "or"];
pub(crate) const TOWN_TAIL: [&str; 8] =
    ["lot", "gel", "fort", "mere", "gard", "wick", "shire", "dun"];

// Display-only quest flavor. Theme keys remain in questmap's classifier table.
pub(crate) const QUEST_ARENA_OVERWORLD: &str = "the king's road";
pub(crate) const QUEST_ARENA_DUNGEON: &str = "the undercroft";
pub(crate) const QUEST_ARENA_VOYAGE: &str = "the grail sea";

pub(crate) const QUEST_PARTY_OVERWORLD: [&str; 8] = [
    "Sir Kay",
    "Sir Bedivere",
    "Sir Gawain",
    "Sir Percival",
    "Sir Bors",
    "Sir Lamorak",
    "Sir Tristan",
    "Sir Galahad",
];
pub(crate) const QUEST_PARTY_DUNGEON: [&str; 8] = [
    "Lancelot", "Galahad", "Gawain", "Percival", "Tristan", "Bors", "Kay", "Bedivere",
];
pub(crate) const QUEST_PARTY_VOYAGE: [&str; 8] = [
    "the Petrel",
    "the Cormorant",
    "the Kestrel",
    "the Albatross",
    "the Gannet",
    "the Fulmar",
    "the Skua",
    "the Tern",
];

pub(crate) const QUEST_LEAGUE: &str = "league";
pub(crate) const QUEST_LEAGUES: &str = "leagues";
pub(crate) const QUEST_CHAMBER: &str = "chamber";
pub(crate) const QUEST_CHAMBERS: &str = "chambers";
pub(crate) const QUEST_DAY_OUT: &str = "day out";
pub(crate) const QUEST_DAYS_OUT: &str = "days out";
pub(crate) const QUEST_FALLS: &str = "falls";
pub(crate) const QUEST_TURNS_BACK: &str = "turns back";
pub(crate) const QUEST_REACHES_CASTLE: &str = "reaches the castle";
pub(crate) const QUEST_OVERWORLD_WANDERING: &str = "still wandering when the chronicle ends";
pub(crate) const QUEST_CASTLE_WON: &str = "the castle is won";
pub(crate) const QUEST_CASTLE_UNTAKEN: &str = "the castle stands untaken";
pub(crate) const QUEST_HITS_DEAD_END: &str = "hits a dead end";
pub(crate) const QUEST_DOUBLES_BACK: &str = "doubles back";
pub(crate) const QUEST_LIFTS_TREASURE: &str = "lifts the treasure";
pub(crate) const QUEST_TORCH_LIT: &str = "torch still lit when the chronicle ends";
pub(crate) const QUEST_TREASURE_CLAIMED: &str = "the treasure is claimed";
pub(crate) const QUEST_VAULT_SEALED: &str = "the vault stays sealed";
pub(crate) const QUEST_WRECKS: &str = "wrecks";
pub(crate) const QUEST_COMES_ABOUT: &str = "comes about";
pub(crate) const QUEST_MAKES_PORT: &str = "makes port";
pub(crate) const QUEST_STILL_AT_SEA: &str = "still at sea when the chronicle ends";
pub(crate) const QUEST_LANDFALL: &str = "landfall";
pub(crate) const QUEST_NO_LANDFALL: &str = "no landfall";

// Camelot-noir palette anchors.
pub(crate) const HUD_BLUE: Color = Color::Rgb(91, 190, 255);
pub(crate) const HUD_DIM: Color = Color::Rgb(58, 111, 151);
pub(crate) const HUD_TEXT: Color = Color::Rgb(184, 228, 255);
pub(crate) const HUD_PHOSPHOR: Color = Color::Rgb(86, 232, 255);
pub(crate) const HUD_GOLD: Color = Color::Rgb(255, 207, 92);
pub(crate) const HUD_DANGER: Color = Color::Rgb(255, 103, 126);
pub(crate) const HUD_VERIFIED: Color = Color::Rgb(99, 241, 169);
/// Warning amber: caution without blame. Same value the loop pane's
/// `WARNING_AMBER` has used since the RL state graph shipped, hoisted here
/// so the quest HUD and the loop viz cannot drift apart.
pub(crate) const HUD_AMBER: Color = Color::Rgb(255, 204, 63);

/// Regal purple: the operator's speaker label in the primary conversation pane
/// — distinct from the agent's phosphor cyan and the knight's candlelight gold.
pub(crate) const HUD_PURPLE: Color = Color::Rgb(200, 130, 255);

pub(crate) const NOIR_NIGHT_HAZE: [u8; 3] = [9, 13, 24];
pub(crate) const NOIR_MOON_SILVER: [u8; 3] = [172, 192, 220];
pub(crate) const NOIR_SKY_ZENITH: [u8; 3] = [3, 5, 14];
pub(crate) const NOIR_SKY_HORIZON: [u8; 3] = [22, 34, 60];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landmark_table_rows_match_pre_extraction_literals() {
        let expected = [
            (
                Building::Keep,
                "Keep courtyard",
                (255, 207, 92),
                0,
                "resting in the keep",
                4,
                2,
                1.8,
                (0.50, 0.55),
            ),
            (
                Building::Gatehouse,
                "Gatehouse",
                (255, 103, 126),
                1,
                "at the gatehouse",
                4,
                2,
                1.15,
                (0.08, 0.50),
            ),
            (
                Building::Rookery,
                "Rookery",
                (91, 190, 255),
                2,
                "sealing scrolls at the rookery",
                3,
                1,
                1.6,
                (0.30, 0.50),
            ),
            (
                Building::Scriptorium,
                "Scriptorium",
                (184, 228, 255),
                3,
                "studying in the scriptorium",
                3,
                1,
                1.2,
                (0.55, 0.18),
            ),
            (
                Building::Smithy,
                "Smithy",
                (255, 207, 92),
                4,
                "at work in the smithy",
                3,
                1,
                1.25,
                (0.80, 0.45),
            ),
            (
                Building::Chapel,
                "Chapel",
                (255, 103, 126),
                5,
                "praying in the chapel",
                4,
                1,
                1.5,
                (0.68, 0.80),
            ),
            (
                Building::RoundTable,
                "Round Table",
                (99, 241, 169),
                6,
                "council at the Round Table",
                4,
                2,
                1.1,
                (0.35, 0.82),
            ),
            (
                Building::Observatory,
                "Observatory",
                (86, 232, 255),
                7,
                "charting the heavens at the observatory",
                4,
                2,
                1.3,
                (0.18, 0.20),
            ),
        ];
        for (row, expected) in LANDMARKS.iter().zip(expected) {
            assert_eq!(
                (
                    row.kind,
                    row.display_name,
                    row.rgb,
                    row.index,
                    row.arrive_activity,
                    row.facade_material,
                    row.facade_span,
                    row.landmark_height,
                    row.district_fraction
                ),
                expected
            );
        }
    }
}
