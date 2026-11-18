//! Shared interior shells, stations and geometry for room interactions.
//!
//! A place has a composable geometry record. Each one is a data record: a shell
//! material plus a list of station modules dropped into bays. Growing a place
//! means appending a station to its list; adding a place means adding a
//! record. Neither touches the composition code below, which is what lets the
//! realm keep gaining rooms and keep filling the rooms it already has.
//!
//! Maps are row-major 9x9 const data: `0` is flat floor and non-zero cells
//! are shared wall materials. The south wall has one centered door.

use super::Building;
use super::cinematics::PROP_ID_BASE;
use super::raycast::{
    MATERIAL_DOOR, MATERIAL_STONE, MATERIAL_TIMBER, RayMap, RaySprite, RayView, TERRAIN_MEADOW,
};

pub(super) const ROOM_SIDE: u32 = 9;
pub(super) const SMITHY_FIRE_ID: u8 = PROP_ID_BASE + 1;
pub(super) const ROUND_TABLE_ID: u8 = PROP_ID_BASE + 2;
pub(super) const EXIT_DOORWAY_ID: u8 = PROP_ID_BASE + 4;
pub(super) const BANNER_ID: u8 = PROP_ID_BASE + 5;
pub(super) const BRAZIER_ID: u8 = PROP_ID_BASE;
pub(super) const PERCH_ID: u8 = PROP_ID_BASE + 4;
pub(super) const LECTERN_ID: u8 = PROP_ID_BASE + 7;
pub(super) const TELESCOPE_ID: u8 = PROP_ID_BASE + 3;
pub(super) const KEEP_HEARTH_ID: u8 = PROP_ID_BASE + 8;
pub(super) const PORTCULLIS_ID: u8 = PROP_ID_BASE + 9;
pub(super) const RAVEN_ID: u8 = PROP_ID_BASE + 10;
pub(super) const ALTAR_ID: u8 = PROP_ID_BASE + 11;
pub(super) const ROSE_GLOW_ID: u8 = PROP_ID_BASE + 12;
pub(super) const LIBRARY_SHELVES_ID: u8 = PROP_ID_BASE + 13;
pub(super) const LIBRARY_TUTORS_ID: u8 = PROP_ID_BASE + 14;
pub(super) const LIBRARY_BOOKS_ID: u8 = PROP_ID_BASE + 15;
pub(super) const ANVIL_ID: u8 = PROP_ID_BASE + 16;
pub(super) const COUNCIL_SEAT_ID: u8 = PROP_ID_BASE + 17;
pub(super) const GEAR_BENCH_ID: u8 = PROP_ID_BASE + 18;
pub(super) const STAR_GLOBE_ID: u8 = PROP_ID_BASE + 19;
pub(super) const MEMORY_LOOM_ID: u8 = PROP_ID_BASE + 20;
pub(super) const BOTANICAL_TABLE_ID: u8 = PROP_ID_BASE + 21;
pub(super) const DIALOGUE_HEARTH_ID: u8 = PROP_ID_BASE + 22;
pub(super) const CHART_STAND_ID: u8 = PROP_ID_BASE + 23;

const T: u8 = MATERIAL_TIMBER;
const S: u8 = MATERIAL_STONE;
const D: u8 = MATERIAL_DOOR;

const AISLE_X: f32 = 4.5;
const LANE_PITCH: f32 = 0.95;
const ROW0_Y: f32 = 2.0;
const ROW_PITCH: f32 = 0.7;

#[derive(Clone, Copy)]
pub(super) struct Bay {
    lane: i8,
    row: u8,
    nudge: (f32, f32),
}

const fn bay(lane: i8, row: u8) -> Bay {
    Bay {
        lane,
        row,
        nudge: (0.0, 0.0),
    }
}

const fn nudged(lane: i8, row: u8, dx: f32, dy: f32) -> Bay {
    Bay {
        lane,
        row,
        nudge: (dx, dy),
    }
}

impl Bay {
    fn xy(self) -> (f32, f32) {
        (
            AISLE_X + self.lane as f32 * LANE_PITCH + self.nudge.0,
            ROW0_Y + self.row as f32 * ROW_PITCH + self.nudge.1,
        )
    }
}

#[derive(Clone, Copy)]
pub(super) enum Light {
    Dark,
    Steady,
    Flicker,
    Counterflicker,
}

#[derive(Clone, Copy)]
pub(super) struct Station {
    pub(super) id: u8,
    pub(super) name: &'static str,
    pub(super) size: (f32, f32),
    pub(super) light: Light,
}

const fn station(id: u8, name: &'static str, w: f32, h: f32, light: Light) -> Station {
    Station {
        id,
        name,
        size: (w, h),
        light,
    }
}

const fn offbeat(s: Station) -> Station {
    Station {
        id: s.id,
        name: s.name,
        size: s.size,
        light: Light::Counterflicker,
    }
}

const HEARTH: Station = station(KEEP_HEARTH_ID, "long hearth", 3.7, 0.82, Light::Steady);
const BANNER: Station = station(BANNER_ID, "banner", 0.85, 1.55, Light::Dark);
const PORTCULLIS: Station = station(PORTCULLIS_ID, "portcullis", 4.8, 2.2, Light::Dark);
const BRAZIER: Station = station(BRAZIER_ID, "brazier", 0.9, 1.15, Light::Flicker);
const PERCH: Station = station(PERCH_ID, "perch", 1.1, 1.15, Light::Dark);
const RAVEN: Station = station(RAVEN_ID, "raven", 0.62, 0.62, Light::Dark);
const RAVEN_ROOSTING: Station = station(RAVEN_ID, "roosting raven", 0.78, 0.62, Light::Steady);
const SHELF_BAY: Station = station(
    LIBRARY_SHELVES_ID,
    "curriculum bay",
    6.8,
    3.65,
    Light::Steady,
);
const TUTOR_BENCH: Station = station(LIBRARY_TUTORS_ID, "tutor bench", 1.85, 1.55, Light::Steady);
const BOOK_TABLE: Station = station(LIBRARY_BOOKS_ID, "book table", 1.85, 1.55, Light::Steady);
const LECTERN: Station = station(LECTERN_ID, "scroll lectern", 0.85, 0.90, Light::Steady);
const GEAR_BENCH: Station = station(GEAR_BENCH_ID, "gear bench", 0.95, 0.80, Light::Steady);
const STAR_GLOBE: Station = station(STAR_GLOBE_ID, "star globe", 0.72, 1.05, Light::Steady);
const MEMORY_LOOM: Station = station(MEMORY_LOOM_ID, "memory loom", 0.72, 1.15, Light::Steady);
const BOTANICAL_TABLE: Station = station(
    BOTANICAL_TABLE_ID,
    "botanical table",
    0.78,
    0.68,
    Light::Dark,
);
const DIALOGUE_HEARTH: Station = station(
    DIALOGUE_HEARTH_ID,
    "dialogue hearth",
    0.78,
    0.72,
    Light::Flicker,
);
const FORGE_FIRE: Station = station(SMITHY_FIRE_ID, "forge fire", 0.9, 1.15, Light::Flicker);
const ANVIL: Station = station(ANVIL_ID, "anvil", 1.35, 0.8, Light::Steady);
const ROSE_WINDOW: Station = station(ROSE_GLOW_ID, "rose window", 2.7, 2.15, Light::Steady);
const ALTAR: Station = station(ALTAR_ID, "altar", 2.25, 1.15, Light::Dark);
const TAPER: Station = station(SMITHY_FIRE_ID, "chapel taper", 0.48, 1.25, Light::Flicker);
const COUNCIL_TABLE: Station = station(ROUND_TABLE_ID, "round table", 3.2, 0.78, Light::Steady);
const SEAT: Station = station(COUNCIL_SEAT_ID, "council seat", 0.48, 0.7, Light::Dark);
const TELESCOPE: Station = station(TELESCOPE_ID, "telescope", 2.7, 2.35, Light::Steady);
const CHART_STAND: Station = station(CHART_STAND_ID, "chart stand", 1.35, 1.25, Light::Steady);

#[cfg(test)]
const SCRIPTORIUM_TUTOR_STATIONS: [Station; 6] = [
    LECTERN,
    GEAR_BENCH,
    STAR_GLOBE,
    MEMORY_LOOM,
    BOTANICAL_TABLE,
    DIALOGUE_HEARTH,
];

#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub(super) station: Station,
    pub(super) bay: Bay,
}

const fn put(station: Station, bay: Bay) -> Placement {
    Placement { station, bay }
}

pub(super) struct Place {
    pub(super) building: Building,
    pub(super) shell: u8,
    pub(super) stations: &'static [Placement],
}

const KEEP: &[Placement] = &[
    put(HEARTH, nudged(0, 2, 0.0, -0.30)),
    put(BANNER, nudged(-3, 0, 0.35, 0.0)),
    put(BANNER, nudged(3, 0, -0.35, 0.0)),
];

const GATEHOUSE: &[Placement] = &[
    put(PORTCULLIS, nudged(0, 1, 0.0, -0.25)),
    put(BRAZIER, nudged(-2, 3, -0.35, -0.10)),
    put(offbeat(BRAZIER), nudged(2, 3, 0.35, -0.10)),
];

const ROOKERY: &[Placement] = &[
    put(PERCH, nudged(-3, 1, 0.45, 0.0)),
    put(PERCH, bay(0, 1)),
    put(PERCH, nudged(3, 1, -0.45, 0.0)),
    put(RAVEN, nudged(-2, 2, 0.40, -0.15)),
    put(RAVEN_ROOSTING, nudged(2, 0, -0.30, -0.10)),
];

const SCRIPTORIUM: &[Placement] = &[
    put(SHELF_BAY, nudged(0, 0, 0.0, -0.45)),
    put(TUTOR_BENCH, nudged(-2, 0, 0.15, 0.10)),
    put(BOOK_TABLE, nudged(2, 0, -0.15, 0.10)),
    put(LECTERN, nudged(-1, 2, -0.45, 0.0)),
    put(GEAR_BENCH, nudged(1, 2, 0.45, 0.0)),
    put(STAR_GLOBE, nudged(-2, 3, 0.20, 0.0)),
    put(MEMORY_LOOM, nudged(2, 3, -0.20, 0.0)),
    put(BOTANICAL_TABLE, nudged(-1, 4, 0.45, 0.0)),
    put(DIALOGUE_HEARTH, nudged(1, 4, -0.45, 0.0)),
];

const SMITHY: &[Placement] = &[
    put(FORGE_FIRE, nudged(-3, 0, 0.35, 0.20)),
    put(offbeat(FORGE_FIRE), nudged(3, 0, -0.35, 0.20)),
    put(ANVIL, nudged(0, 3, 0.0, 0.10)),
];

const CHAPEL: &[Placement] = &[
    put(ROSE_WINDOW, nudged(0, 0, 0.0, 0.15)),
    put(ALTAR, nudged(0, 2, 0.0, -0.30)),
    put(TAPER, nudged(-2, 2, -0.05, -0.15)),
    put(offbeat(TAPER), nudged(2, 2, 0.05, -0.15)),
];

const ROUND_TABLE: &[Placement] = &[
    put(COUNCIL_TABLE, nudged(0, 3, 0.0, -0.10)),
    put(SEAT, nudged(-2, 1, 0.0, 0.30)),
    put(SEAT, nudged(0, 1, 0.0, -0.20)),
    put(SEAT, nudged(2, 1, 0.0, 0.30)),
    put(SEAT, nudged(-2, 4, 0.0, 0.20)),
    put(SEAT, nudged(2, 4, 0.0, 0.20)),
];

const OBSERVATORY: &[Placement] = &[
    put(TELESCOPE, nudged(0, 1, -0.10, 0.35)),
    put(CHART_STAND, nudged(2, 2, -0.45, 0.30)),
];

pub(super) const PLACES: &[Place] = &[
    Place {
        building: Building::Keep,
        shell: S,
        stations: KEEP,
    },
    Place {
        building: Building::Gatehouse,
        shell: S,
        stations: GATEHOUSE,
    },
    Place {
        building: Building::Rookery,
        shell: T,
        stations: ROOKERY,
    },
    Place {
        building: Building::Scriptorium,
        shell: T,
        stations: SCRIPTORIUM,
    },
    Place {
        building: Building::Smithy,
        shell: T,
        stations: SMITHY,
    },
    Place {
        building: Building::Chapel,
        shell: S,
        stations: CHAPEL,
    },
    Place {
        building: Building::RoundTable,
        shell: S,
        stations: ROUND_TABLE,
    },
    Place {
        building: Building::Observatory,
        shell: S,
        stations: OBSERVATORY,
    },
];

fn place_for(building: Building) -> Option<&'static Place> {
    PLACES.iter().find(|place| place.building == building)
}

fn shell_cells(material: u8) -> Vec<u8> {
    let side = ROOM_SIDE as usize;
    let mut cells = vec![0u8; side * side];
    for i in 0..side {
        cells[i] = material;
        cells[(side - 1) * side + i] = material;
        cells[i * side] = material;
        cells[i * side + side - 1] = material;
    }
    cells[(side - 1) * side + side / 2] = D;
    cells
}

pub(super) fn supports(building: Building) -> bool {
    place_for(building).is_some()
}

pub(super) fn scene(building: Building, tick: u64, sway: f32) -> Option<(RayMap, RayView)> {
    Some(compose(place_for(building)?, tick, sway))
}

fn compose(place: &Place, tick: u64, sway: f32) -> (RayMap, RayView) {
    let fire_lit = (tick / 2).is_multiple_of(2);
    let mut sprites: Vec<RaySprite> = place
        .stations
        .iter()
        .map(|placed| {
            let (x, y) = placed.bay.xy();
            let (w, h) = placed.station.size;
            let lit = match placed.station.light {
                Light::Dark => false,
                Light::Steady => true,
                Light::Flicker => fire_lit,
                Light::Counterflicker => !fire_lit,
            };
            sprite(x, y, w, h, placed.station.id, lit)
        })
        .collect();
    sprites.push(doorway());
    (
        RayMap {
            width: ROOM_SIDE,
            height: ROOM_SIDE,
            cells: shell_cells(place.shell),
            terrain: vec![0.0; (ROOM_SIDE * ROOM_SIDE) as usize],
            terrain_kind: vec![TERRAIN_MEADOW; (ROOM_SIDE * ROOM_SIDE) as usize],
            sprites,
        },
        RayView {
            x: 4.5,
            y: 7.25,
            heading_rad: -std::f32::consts::FRAC_PI_2 + sway,
            look_yaw: 0.0,
            fov_rad: 1.05,
            bob: 0.0,
            eye_h: 0.0,
        },
    )
}

fn sprite(x: f32, y: f32, width: f32, height: f32, id: u8, lit: bool) -> RaySprite {
    RaySprite {
        x,
        y,
        width,
        height,
        id,
        lit,
    }
}

fn doorway() -> RaySprite {
    sprite(4.5, 7.82, 1.0, 1.65, EXIT_DOORWAY_ID, false)
}

#[cfg(test)]
#[path = "../../tests/world_viz/interiors__tests.rs"]
mod tests;
