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

// ------------------------------------------------------------------ bay grid

/// The centre aisle, which the south door opens onto.
const AISLE_X: f32 = 4.5;
/// Horizontal spacing between station lanes.
const LANE_PITCH: f32 = 0.95;
/// Depth of the first station row, set just clear of the back wall.
const ROW0_Y: f32 = 2.0;
/// Depth spacing between station rows.
const ROW_PITCH: f32 = 0.7;

/// Where a station sits. Lanes run outward from the aisle (negative to the
/// left), rows run from the back wall toward the door.
///
/// The two pitches were fitted to the hand-placed rooms this table replaced:
/// every one of the 32 original sprites lands on a bay within 0.45 cells, so
/// the grid follows the art rather than the art following the grid. `nudge`
/// carries that remainder. It is art tuning, not structure — a station with no
/// nudge is the normal case, and new stations should need none.
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

// --------------------------------------------------------------- station kit

/// How a station answers the room's flicker clock.
#[derive(Clone, Copy)]
pub(super) enum Light {
    /// Never lit.
    Dark,
    /// Always lit.
    Steady,
    /// Lit on the even phase.
    Flicker,
    /// Lit on the odd phase, so a pair alternates.
    Counterflicker,
}

/// One reusable piece of furniture. The station kit below is also the art
/// backlog: every entry names a silhouette that has to read at pane size.
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

/// The same station on the opposite flicker phase, for facing pairs.
const fn offbeat(s: Station) -> Station {
    Station {
        id: s.id,
        name: s.name,
        size: s.size,
        light: Light::Counterflicker,
    }
}

// Hall and hearth
const HEARTH: Station = station(KEEP_HEARTH_ID, "long hearth", 3.7, 0.82, Light::Steady);
const BANNER: Station = station(BANNER_ID, "banner", 0.85, 1.55, Light::Dark);
// Gate
const PORTCULLIS: Station = station(PORTCULLIS_ID, "portcullis", 4.8, 2.2, Light::Dark);
const BRAZIER: Station = station(BRAZIER_ID, "brazier", 0.9, 1.15, Light::Flicker);
// Rookery
const PERCH: Station = station(PERCH_ID, "perch", 1.1, 1.15, Light::Dark);
const RAVEN: Station = station(RAVEN_ID, "raven", 0.62, 0.62, Light::Dark);
const RAVEN_ROOSTING: Station = station(RAVEN_ID, "roosting raven", 0.78, 0.62, Light::Steady);
// Teaching floor
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
// Forge
const FORGE_FIRE: Station = station(SMITHY_FIRE_ID, "forge fire", 0.9, 1.15, Light::Flicker);
const ANVIL: Station = station(ANVIL_ID, "anvil", 1.35, 0.8, Light::Steady);
// Chapel
const ROSE_WINDOW: Station = station(ROSE_GLOW_ID, "rose window", 2.7, 2.15, Light::Steady);
const ALTAR: Station = station(ALTAR_ID, "altar", 2.25, 1.15, Light::Dark);
const TAPER: Station = station(SMITHY_FIRE_ID, "chapel taper", 0.48, 1.25, Light::Flicker);
// Council
const COUNCIL_TABLE: Station = station(ROUND_TABLE_ID, "round table", 3.2, 0.78, Light::Steady);
const SEAT: Station = station(COUNCIL_SEAT_ID, "council seat", 0.48, 0.7, Light::Dark);
// Observatory
const TELESCOPE: Station = station(TELESCOPE_ID, "telescope", 2.7, 2.35, Light::Steady);
const CHART_STAND: Station = station(CHART_STAND_ID, "chart stand", 1.35, 1.25, Light::Steady);

/// The named Scriptorium tutor kit. These six silhouettes are the stable
/// vocabulary of the teaching floor; the room may gain more stations later,
/// but none of these may collapse back into a generic desk or billboard.
#[cfg(test)]
const SCRIPTORIUM_TUTOR_STATIONS: [Station; 6] = [
    LECTERN,
    GEAR_BENCH,
    STAR_GLOBE,
    MEMORY_LOOM,
    BOTANICAL_TABLE,
    DIALOGUE_HEARTH,
];

/// A station dropped into a bay.
#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub(super) station: Station,
    pub(super) bay: Bay,
}

const fn put(station: Station, bay: Bay) -> Placement {
    Placement { station, bay }
}

// ------------------------------------------------------------- place records

/// A place: a shell to stand in and the stations that furnish it.
pub(super) struct Place {
    pub(super) building: Building,
    pub(super) shell: u8,
    pub(super) stations: &'static [Placement],
}

/// Hearth hall in a broad stone shell.
const KEEP: &[Placement] = &[
    put(HEARTH, nudged(0, 2, 0.0, -0.30)),
    put(BANNER, nudged(-3, 0, 0.35, 0.0)),
    put(BANNER, nudged(3, 0, -0.35, 0.0)),
];

/// Gate passage, enclosed in dressed stone.
const GATEHOUSE: &[Placement] = &[
    put(PORTCULLIS, nudged(0, 1, 0.0, -0.25)),
    put(BRAZIER, nudged(-2, 3, -0.35, -0.10)),
    put(offbeat(BRAZIER), nudged(2, 3, 0.35, -0.10)),
];

/// Timber loft for the rookery perches.
const ROOKERY: &[Placement] = &[
    put(PERCH, nudged(-3, 1, 0.45, 0.0)),
    put(PERCH, bay(0, 1)),
    put(PERCH, nudged(3, 1, -0.45, 0.0)),
    put(RAVEN, nudged(-2, 2, 0.40, -0.15)),
    put(RAVEN_ROOSTING, nudged(2, 0, -0.30, -0.10)),
];

/// The teaching floor. Its stations are the curriculum, so this is the place
/// most expected to grow: new tutors take free bays in rows 2 and 3.
const SCRIPTORIUM: &[Placement] = &[
    put(SHELF_BAY, nudged(0, 0, 0.0, -0.45)),
    // Resident and curriculum plates sit against the shelf wall rather than
    // masquerading as floor furniture.
    put(TUTOR_BENCH, nudged(-2, 0, 0.15, 0.10)),
    put(BOOK_TABLE, nudged(2, 0, -0.15, 0.10)),
    // From the camera these pairs occupy outer, middle, and inner screen
    // bands. The depth and bay nudges keep all six silhouettes independently
    // visible instead of lining them up where near furniture hides far.
    put(LECTERN, nudged(-1, 2, -0.45, 0.0)),
    put(GEAR_BENCH, nudged(1, 2, 0.45, 0.0)),
    put(STAR_GLOBE, nudged(-2, 3, 0.20, 0.0)),
    put(MEMORY_LOOM, nudged(2, 3, -0.20, 0.0)),
    put(BOTANICAL_TABLE, nudged(-1, 4, 0.45, 0.0)),
    put(DIALOGUE_HEARTH, nudged(1, 4, -0.45, 0.0)),
];

/// Timber workshop shell; forge furniture is authored as billboards.
const SMITHY: &[Placement] = &[
    put(FORGE_FIRE, nudged(-3, 0, 0.35, 0.20)),
    put(offbeat(FORGE_FIRE), nudged(3, 0, -0.35, 0.20)),
    put(ANVIL, nudged(0, 3, 0.0, 0.10)),
];

/// Stone chapel shell.
const CHAPEL: &[Placement] = &[
    put(ROSE_WINDOW, nudged(0, 0, 0.0, 0.15)),
    put(ALTAR, nudged(0, 2, 0.0, -0.30)),
    put(TAPER, nudged(-2, 2, -0.05, -0.15)),
    put(offbeat(TAPER), nudged(2, 2, 0.05, -0.15)),
];

/// Stone council chamber; table and seats stay readable at tiny sizes.
///
/// The near-centre bay (lane 0, row 5) is deliberately left open. It is the
/// seat the camera stands at, so filling it would mass a dark chair across the
/// middle of the table and close off the vista the room is built around.
const ROUND_TABLE: &[Placement] = &[
    put(COUNCIL_TABLE, nudged(0, 3, 0.0, -0.10)),
    put(SEAT, nudged(-2, 1, 0.0, 0.30)),
    put(SEAT, nudged(0, 1, 0.0, -0.20)),
    put(SEAT, nudged(2, 1, 0.0, 0.30)),
    put(SEAT, nudged(-2, 4, 0.0, 0.20)),
    put(SEAT, nudged(2, 4, 0.0, 0.20)),
];

/// Stone observatory; its apparent ceiling is the open telescope shaft.
const OBSERVATORY: &[Placement] = &[
    put(TELESCOPE, nudged(0, 1, -0.10, 0.35)),
    put(CHART_STAND, nudged(2, 2, -0.45, 0.30)),
];

/// Every place in the realm. Adding one is a record, not a match arm.
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

/// A bare shell: solid wall ring in one material, with one centered south door.
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

/// Build the drawable scene for one place. Everything a place needs is in its
/// record, so a place that gains a station gains it here with no code change.
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
mod tests {
    use super::*;
    use crate::world_viz::World;

    const ROOMS: [(Building, u8); 8] = [
        (Building::Keep, KEEP_HEARTH_ID),
        (Building::Gatehouse, PORTCULLIS_ID),
        (Building::Rookery, RAVEN_ID),
        (Building::Scriptorium, LECTERN_ID),
        (Building::Smithy, ANVIL_ID),
        (Building::Chapel, ALTAR_ID),
        (Building::RoundTable, ROUND_TABLE_ID),
        (Building::Observatory, TELESCOPE_ID),
    ];

    #[test]
    fn every_supported_building_has_exactly_one_furnished_place() {
        for (building, _) in ROOMS {
            let matches = PLACES
                .iter()
                .filter(|place| place.building == building)
                .count();
            assert_eq!(matches, 1, "{building:?} should have exactly one place");
            assert!(
                supports(building),
                "{building:?} has a place but reads as unsupported"
            );
        }
        assert_eq!(PLACES.len(), ROOMS.len());
        for place in PLACES {
            assert!(
                !place.stations.is_empty(),
                "{:?} is an empty room",
                place.building
            );
        }
    }

    #[test]
    fn no_nudge_escapes_the_bay_grid() {
        // A nudge is art tuning on top of a bay. Once one grows past half a
        // bay the station belongs in a different bay, and the grid has stopped
        // describing the room.
        const MAX_NUDGE: f32 = 0.5;
        for place in PLACES {
            for placed in place.stations {
                let (dx, dy) = placed.bay.nudge;
                assert!(
                    dx.abs() <= MAX_NUDGE && dy.abs() <= MAX_NUDGE,
                    "{:?} {} nudged ({dx}, {dy}) past {MAX_NUDGE}",
                    place.building,
                    placed.station.name
                );
            }
        }
    }

    #[test]
    fn every_station_stands_inside_its_shell_and_clear_of_the_entry() {
        // Walls occupy cells 0 and 8, so the open floor is (1.0, 8.0). The
        // camera stands at y 7.25 and the door is behind it; stations must
        // leave that approach clear or the player walks into furniture.
        const ENTRY_CLEARANCE_Y: f32 = 6.0;
        for place in PLACES {
            for placed in place.stations {
                let (x, y) = placed.bay.xy();
                let half_width = placed.station.size.0 / 2.0;
                let name = placed.station.name;
                assert!(
                    x - half_width > 1.0 && x + half_width < 8.0,
                    "{:?} {name} spans {:.2}..{:.2}, outside the walls",
                    place.building,
                    x - half_width,
                    x + half_width
                );
                assert!(
                    y > 1.0 && y < ENTRY_CLEARANCE_Y,
                    "{:?} {name} sits at depth {y:.2}, not inside 1.0..{ENTRY_CLEARANCE_Y}",
                    place.building
                );
            }
        }
    }

    #[test]
    fn station_art_kinds_are_shared_only_where_intended() {
        // Prop art dispatches on `id - PROP_ID_BASE` with no knowledge of the
        // room, so two stations sharing an id draw the identical silhouette.
        // Sharing is legitimate while a station is waiting for its own plate;
        // it is a bug when it happens by accident. Pin the known set so a new
        // collision has to be declared here before it can ship.
        const APPROVED_SHARING: [(u8, &[&str]); 2] = [
            // One flame silhouette serves every open flame in the realm.
            (SMITHY_FIRE_ID, &["forge fire", "chapel taper"]),
            // One bird, two poses. Kind 10 already spreads or settles its
            // wings on `lit`, so the pair is the plate working as intended.
            (RAVEN_ID, &["raven", "roosting raven"]),
        ];

        let mut by_id: std::collections::BTreeMap<u8, std::collections::BTreeSet<&str>> =
            std::collections::BTreeMap::new();
        for place in PLACES {
            for placed in place.stations {
                by_id
                    .entry(placed.station.id)
                    .or_default()
                    .insert(placed.station.name);
            }
        }

        for (id, names) in by_id {
            if names.len() == 1 {
                continue;
            }
            let approved = APPROVED_SHARING
                .iter()
                .find(|(approved_id, _)| *approved_id == id)
                .map(|(_, names)| {
                    names
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                });
            assert_eq!(
                Some(names.clone()),
                approved,
                "stations {names:?} all draw art kind {} — declare the sharing or give one its own plate",
                id.saturating_sub(PROP_ID_BASE)
            );
        }
    }

    #[test]
    fn scriptorium_contains_each_named_tutor_station_once() {
        let tutor_names: std::collections::BTreeSet<_> = SCRIPTORIUM_TUTOR_STATIONS
            .iter()
            .map(|station| station.name)
            .collect();
        let tutor_ids: std::collections::BTreeSet<_> = SCRIPTORIUM_TUTOR_STATIONS
            .iter()
            .map(|station| station.id)
            .collect();
        assert_eq!(tutor_names.len(), SCRIPTORIUM_TUTOR_STATIONS.len());
        assert_eq!(
            tutor_ids.len(),
            SCRIPTORIUM_TUTOR_STATIONS.len(),
            "every tutor station needs its own silhouette"
        );
        for station in SCRIPTORIUM_TUTOR_STATIONS {
            let placements = SCRIPTORIUM
                .iter()
                .filter(|placed| placed.station.id == station.id)
                .count();
            assert_eq!(
                placements, 1,
                "Scriptorium should contain one {}",
                station.name
            );
        }
    }

    #[test]
    fn a_place_that_gains_a_station_shows_it_without_a_code_change() {
        // The growth path: append to a place's station list, nothing else.
        let grown_stations: &'static [Placement] = Box::leak(
            SCRIPTORIUM
                .iter()
                .copied()
                .chain(std::iter::once(put(TUTOR_BENCH, bay(0, 2))))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let grown = Place {
            building: Building::Scriptorium,
            shell: T,
            stations: grown_stations,
        };

        let before = compose(place_for(Building::Scriptorium).unwrap(), 8, 0.0125);
        let after = compose(&grown, 8, 0.0125);
        assert_eq!(after.0.sprites.len(), before.0.sprites.len() + 1);
    }

    #[test]
    fn interior_authored_rooms_have_centerpieces_and_one_south_door() {
        for (building, centerpiece) in ROOMS {
            let (map, _) = scene(building, 0, 0.0).unwrap();
            assert!(map.sprites.iter().any(|sprite| sprite.id == centerpiece));
            assert_eq!(
                map.cells
                    .iter()
                    .filter(|&&cell| cell == MATERIAL_DOOR)
                    .count(),
                1
            );
            assert_eq!(map.cells[8 * ROOM_SIDE as usize + 4], MATERIAL_DOOR);
            assert!(map.terrain.iter().all(|&height| height == 0.0));
        }
    }

    #[test]
    fn interior_entry_rejects_a_knight_not_at_the_destination() {
        let mut world = World::new(7);
        world.target = Building::Smithy;
        world.enter_interior();
        assert_eq!(world.interior, None);
    }

    fn settle_at(world: &mut World, building: Building) {
        world.target = building;
        let dest = world.dest();
        world.avatar = dest;
        world.avatar_vis = (dest.0 as f32, dest.1 as f32);
        world.settle_ticks = super::super::cinematics::ARRIVAL_HOLD_TICKS;
    }

    #[test]
    fn room_plate_ignores_first_person_flicker_and_leaving_restores_outdoor_key() {
        let _view = crate::world_viz::world3d::pin(crate::world_viz::world3d::WorldView::Mesh3d);
        let mut world = World::new(17);
        settle_at(&mut world, Building::Smithy);
        let outdoor = world.cinematic_key();
        world.enter_interior();
        let inside = world.cinematic_key();
        assert_ne!(inside, outdoor);
        world.tick += super::super::cinematics::INTERIOR_FRAME_HOLD_TICKS;
        assert_eq!(
            world.cinematic_key(),
            inside,
            "room plate does not paint the first-person flicker clock"
        );
        // The outdoor key legitimately animates with the clock (the settled
        // first-person plate re-keys on the two-pose rider canter, keyed off
        // `tick`), so restore the clock before the round-trip check — otherwise
        // this asserts a tick-stability the outdoor key no longer has. Leaving
        // toggles only `interior`, so at the original tick the key is identical.
        world.tick -= super::super::cinematics::INTERIOR_FRAME_HOLD_TICKS;
        world.leave_interior();
        assert_eq!(world.cinematic_key(), outdoor);
    }

    #[test]
    fn interior_scene_is_deterministic_and_differs_from_outdoors() {
        let mut a = World::new(23);
        let mut b = World::new(23);
        settle_at(&mut a, Building::RoundTable);
        settle_at(&mut b, Building::RoundTable);
        let outdoor = a.travel_scene();
        a.enter_interior();
        b.enter_interior();
        let first = a.travel_scene();
        let second = b.travel_scene();
        assert_ne!(first.0.cells, outdoor.0.cells);
        assert_eq!(first.0.cells, second.0.cells);
        assert_eq!(
            first.1.heading_rad.to_bits(),
            second.1.heading_rad.to_bits()
        );
    }

    #[test]
    fn a_new_journey_auto_exits_the_room() {
        let mut world = World::new(29);
        settle_at(&mut world, Building::Smithy);
        world.enter_interior();
        assert_eq!(world.interior, Some(Building::Smithy));
        world.target = Building::RoundTable;
        world.tick();
        assert_eq!(world.interior, None);
    }
}
