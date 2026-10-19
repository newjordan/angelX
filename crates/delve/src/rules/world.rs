//! The world above the Delve: rooms of the realm itself that the party walks
//! with the Delve's own keys, and the entrances that lead down from them.
//!
//! The overworld map is walked by the harness (its knight goes where the
//! agent's work is) and has no doors. These rooms are the walkable side of
//! its places: the Delve's gate courtyard, the stables, the lists, and the
//! Mines' mouth. They sit on the home floor (depth 0) beside the Undercroft,
//! in their own corner of its grid, so co-op, the mirror and checkpoints
//! carry them like any home room.
//!
//! A room of the world may hold **entrances**: stairs or a door that, stood
//! on for a moment, take the party to another room (the gate's stair down to
//! the Undercroft, the Undercroft's stair back up) or straight into a delve
//! (the Mines' shaft). Every entrance is data in `ENTRANCES`, keyed by the
//! room it stands in.
//!
//! The Undercroft's own geometry (`layout::undercroft`) is left alone: a
//! saved loop settlement replays it byte for byte. The world, and the
//! Undercroft's stair up to it, are added to the run's own copy of the floor
//! (`raise_world`), after any settlement is admitted.

use super::*;

/// The surface includes authored rooms in addition to a bounded saved settlement.
pub const HOME_ROOM_LIMIT: usize = 128;

/// World rooms sit north of the Undercroft, beyond a loop settlement's
/// excavation (within six cells of the cellar, never reaching row -5).
pub const ROOMS: [(RoomKind, (i32, i32)); 64] = [
    super::ranch::CELLS[0],
    super::ranch::CELLS[1],
    (RoomKind::Gate, (2, -8)),
    (RoomKind::CollegeGrounds, (8, -12)),
    (RoomKind::CollegeHall, (9, -12)),
    (RoomKind::CollegeClass, (10, -12)),
    (RoomKind::CollegeLibrary, (8, -13)),
    (RoomKind::CollegeLab, (9, -13)),
    (RoomKind::TowerEntry, (10, -13)),
    (RoomKind::TowerStairs, (8, -14)),
    (RoomKind::TowerStars, (9, -14)),
    (RoomKind::WizardStudy, (10, -14)),

    (RoomKind::Stables, (1, -8)),
    (RoomKind::Lists, (3, -8)),
    (RoomKind::MineHead, (2, -9)),
    // Inside the King's Hall: no doorway joins it to its neighbours on the
    // grid; its great door is an entrance.
    (RoomKind::KingsHall, (2, -11)),
    (RoomKind::Forest, (2, -7)),
    (RoomKind::Village, (1, -7)),
    (RoomKind::Fairground, (3, -7)),
    (RoomKind::Windmill, (1, -6)),
    (RoomKind::Ruins, (2, -6)),
    (RoomKind::Riverbank, (3, -6)),
    (RoomKind::Meadow, (2, -5)),
    (RoomKind::Orchard, (1, -5)),
    (RoomKind::Marsh, (0, -5)),
    (RoomKind::Downs, (0, -6)),
    (RoomKind::Cliffs, (0, -7)),
    (RoomKind::Causeway, (4, -7)),
    (RoomKind::Priory, (4, -6)),
    (RoomKind::Crag, (0, -8)),
    (RoomKind::Rookery, (1, -9)),
    (RoomKind::Observatory, (0, -9)),
    (RoomKind::Strand, (5, -7)),
    (RoomKind::Saltworks, (5, -8)),
    (RoomKind::Lighthouse, (4, -8)),
    (RoomKind::Timberyard, (-1, -7)),
    (RoomKind::HedgeMaze, (-1, -6)),
    (RoomKind::Bothy, (-1, -5)),
    (RoomKind::Millpond, (-2, -7)),
    (RoomKind::Sluice, (-2, -6)),
    (RoomKind::SunkenAbbey, (-2, -5)),
    (RoomKind::Belfry, (6, -7)),
    (RoomKind::Aqueduct, (6, -6)),
    (RoomKind::Aviary, (5, -6)),
    (RoomKind::Mummers, (2, -10)),
    (RoomKind::Laundry, (3, -10)),
    (RoomKind::Caravan, (3, -9)),
    (RoomKind::Hippodrome, (4, -9)),
    (RoomKind::Wheelwright, (4, -10)),
    (RoomKind::TollBridge, (5, -9)),
    (RoomKind::GooseGreen, (-3, -7)),
    (RoomKind::Kitewright, (-3, -8)),
    (RoomKind::SignalHill, (-2, -8)),
    // The Court of Deeds: its door is on the kingdom's map, by the Keep.
    (RoomKind::LandOffice, (7, -9)),
    // Longbow country, north of the wind-country flags.
    (RoomKind::LongbowGreen, (-2, -9)),
    (RoomKind::Fletchery, (-1, -8)),
    (RoomKind::Scarecrow, (-3, -9)),
    // The town's row, south of the quay: market, bathhouse, smithy.
    (RoomKind::Market, (3, -5)),
    (RoomKind::Bathhouse, (4, -5)),
    (RoomKind::Smithy, (5, -5)),
    // The Griffin Eyrie, up on the Mountain: its only way in is its door
    // on the kingdom's map, and its gate leads back out there.
    (RoomKind::Eyrie, (1, -10)),
    // The Colosseum's entrance hall and its two arenas: its door is on
    // the kingdom's map, its gates are plates (`colosseum`).
    super::colosseum::CELLS[0],
    super::colosseum::CELLS[1],
    super::colosseum::CELLS[2],
];

/// Authored two-way roads. Never dig into an occupied settlement cell.
pub const TRAILS: &[(RoomKind, RoomKind)] = &[
    (RoomKind::LongbowGreen, RoomKind::SignalHill),
    (RoomKind::LongbowGreen, RoomKind::Scarecrow),
    (RoomKind::Scarecrow, RoomKind::Kitewright),
    (RoomKind::Fletchery, RoomKind::SignalHill),
    (RoomKind::Fletchery, RoomKind::Timberyard),
    (RoomKind::Market, RoomKind::Riverbank),
    (RoomKind::Market, RoomKind::Meadow),
    (RoomKind::Market, RoomKind::Bathhouse),
    (RoomKind::Bathhouse, RoomKind::Smithy),
    (RoomKind::Millpond, RoomKind::GooseGreen),
    (RoomKind::GooseGreen, RoomKind::Kitewright),
    (RoomKind::Kitewright, RoomKind::SignalHill),
    (RoomKind::SignalHill, RoomKind::Millpond),
    (RoomKind::Caravan, RoomKind::Hippodrome),
    (RoomKind::Hippodrome, RoomKind::Wheelwright),
    (RoomKind::Wheelwright, RoomKind::Laundry),
    (RoomKind::Hippodrome, RoomKind::TollBridge),
    (RoomKind::TollBridge, RoomKind::Saltworks),
    (RoomKind::Hippodrome, RoomKind::Lighthouse),
    (RoomKind::MineHead, RoomKind::Mummers),
    (RoomKind::Mummers, RoomKind::Laundry),
    (RoomKind::Laundry, RoomKind::Caravan),
    (RoomKind::Caravan, RoomKind::MineHead),
    (RoomKind::Caravan, RoomKind::Lists),
    (RoomKind::Strand, RoomKind::Belfry),
    (RoomKind::Belfry, RoomKind::Aqueduct),
    (RoomKind::Aqueduct, RoomKind::Aviary),
    (RoomKind::Aviary, RoomKind::Priory),
    (RoomKind::Strand, RoomKind::Aviary),
    (RoomKind::Timberyard, RoomKind::Millpond),
    (RoomKind::Millpond, RoomKind::Sluice),
    (RoomKind::Sluice, RoomKind::SunkenAbbey),
    (RoomKind::Sluice, RoomKind::HedgeMaze),
    (RoomKind::SunkenAbbey, RoomKind::Bothy),
    (RoomKind::Cliffs, RoomKind::Timberyard),
    (RoomKind::Timberyard, RoomKind::HedgeMaze),
    (RoomKind::HedgeMaze, RoomKind::Bothy),
    (RoomKind::Downs, RoomKind::HedgeMaze),
    (RoomKind::Marsh, RoomKind::Bothy),
    (RoomKind::Causeway, RoomKind::Strand),
    (RoomKind::Strand, RoomKind::Saltworks),
    (RoomKind::Lighthouse, RoomKind::Causeway),
    (RoomKind::Saltworks, RoomKind::Lighthouse),
    (RoomKind::Stables, RoomKind::Crag),
    (RoomKind::Crag, RoomKind::Cliffs),
    (RoomKind::Crag, RoomKind::Observatory),
    (RoomKind::Observatory, RoomKind::Rookery),
    (RoomKind::Rookery, RoomKind::Stables),
    (RoomKind::Rookery, RoomKind::MineHead),
    (RoomKind::Fairground, RoomKind::Causeway),
    (RoomKind::Causeway, RoomKind::Priory),
    (RoomKind::Priory, RoomKind::Riverbank),
    (RoomKind::Windmill, RoomKind::Downs),
    (RoomKind::Downs, RoomKind::Cliffs),
    (RoomKind::Cliffs, RoomKind::Village),
    (RoomKind::Gate, RoomKind::Stables),
    (RoomKind::Gate, RoomKind::Lists),
    (RoomKind::Gate, RoomKind::MineHead),
    (RoomKind::Gate, RoomKind::Forest),
    (RoomKind::Stables, RoomKind::Village),
    (RoomKind::Lists, RoomKind::Fairground),
    (RoomKind::Village, RoomKind::Forest),
    (RoomKind::Forest, RoomKind::Fairground),
    (RoomKind::Village, RoomKind::Windmill),
    (RoomKind::Forest, RoomKind::Ruins),
    (RoomKind::Fairground, RoomKind::Riverbank),
    (RoomKind::Windmill, RoomKind::Ruins),
    (RoomKind::Ruins, RoomKind::Riverbank),
    (RoomKind::Ruins, RoomKind::Meadow),
    (RoomKind::Windmill, RoomKind::Orchard),
    (RoomKind::Orchard, RoomKind::Meadow),
    (RoomKind::Orchard, RoomKind::Marsh),
];

/// Where an entrance takes the party.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dest {
    /// Another room on the home floor, arriving at this point (tiles).
    Room(RoomKind, (f32, f32)),
    /// Straight down into the first floor of a delve.
    Delve(Pack),
}

/// A way down (or up): its tiles in the room it stands in, where it goes,
/// and what its board says.
#[derive(Clone, Copy)]
pub struct Entrance {
    pub id: &'static str,
    pub room: RoomKind,
    /// The walk-on stair tiles: column, row, width, height.
    pub at: (i32, i32, i32, i32),
    pub to: Dest,
    pub label: &'static str,
    /// The barony's work that must stand before it opens, if any.
    pub needs: Option<&'static str>,
}

/// The Delve's gate in the courtyard: its arch, and the stair in its mouth.
pub const GATE_ARCH: (i32, i32, i32, i32) = (4, 1, 5, 4);
pub const GATE_STAIR: (i32, i32, i32, i32) = (5, 4, 2, 1);
/// Where the party comes up into the courtyard, and where a new delve
/// begins: in front of the gate, on the road (tiles).
pub const GATE_FRONT: (f32, f32) = (6.0, 6.6);
pub const COURTYARD_ARRIVAL: (f32, f32) = (12.0, 10.4);
/// The Undercroft's stair up to the gate, in its north wall east of the
/// Training Yard's door, and where one arrives below it.
pub const CELLAR_STAIR: (i32, i32, i32, i32) = (13, 1, 2, 1);
pub const CELLAR_FRONT: (f32, f32) = (14.0, 3.4);
/// The Mines' mouth at the mine-head: its timbered adit in the north rock,
/// the shaft's stair in its mouth.
pub const ADIT: (i32, i32, i32, i32) = (10, 1, 4, 3);
pub const SHAFT: (i32, i32, i32, i32) = (11, 3, 2, 1);

pub const ENTRANCES: [Entrance; 59] = [
    super::ranch::DOORS[0],
    super::ranch::DOORS[1],
    super::ranch::DOORS[2],
    super::ranch::DOORS[3],
    super::college::DOORS[0],
    super::college::DOORS[1],
    super::college::DOORS[2],
    super::college::DOORS[3],
    super::college::DOORS[4],
    super::college::DOORS[5],
    super::college::DOORS[6],
    super::college::DOORS[7],
    super::college::DOORS[8],
    super::college::DOORS[9],
    super::college::DOORS[10],
    super::college::DOORS[11],
    super::college::DOORS[12],
    super::college::DOORS[13],
    super::college::DOORS[14],
    super::college::DOORS[15],
    super::college::DOORS[16],
    super::college::DOORS[17],
    super::colosseum::DOORS[0],
    super::colosseum::DOORS[1],
    super::colosseum::DOORS[2],
    super::colosseum::DOORS[3],
    super::colosseum::DOORS[4],

    Entrance { id: "race-coach", room: RoomKind::Hippodrome, at: super::hippodrome::COACH_GATE,
        to: Dest::Room(RoomKind::Stables, (19.5, 8.2)),
        label: "SUSPICIOUSLY FAST COACH TO STABLES", needs: Some("country:racecoach") },
    Entrance { id: "stable-race-coach", room: RoomKind::Stables, at: super::hippodrome::STABLE_COACH,
        to: Dest::Room(RoomKind::Hippodrome, (19.5, 8.2)),
        label: "SUSPICIOUSLY FAST COACH TO HIPPODROME", needs: Some("country:racecoach") },
    Entrance {
        id: "caravan-out", room: RoomKind::Caravan, at: super::mummers::WAGON,
        to: Dest::Room(RoomKind::Fairground, (21.0, 9.0)),
        label: "THE WHEELY INN TO FOOL'S GREEN", needs: Some("country:caravan"),
    },
    Entrance {
        id: "caravan-back", room: RoomKind::Fairground, at: super::mummers::FAIR_WAGON,
        to: Dest::Room(RoomKind::Caravan, (20.0, 9.0)),
        label: "THE WHEELY INN TO CARAVAN CAMP", needs: Some("country:caravan"),
    },
    Entrance {
        id: "gate",
        room: RoomKind::Gate,
        at: GATE_STAIR,
        to: Dest::Room(RoomKind::Home, CELLAR_FRONT),
        label: "DOWN TO THE UNDERCROFT",
        needs: None,
    },
    Entrance {
        id: "cellar",
        room: RoomKind::Home,
        at: CELLAR_STAIR,
        to: Dest::Room(RoomKind::Gate, GATE_FRONT),
        label: "UP TO THE GATE",
        needs: None,
    },
    Entrance {
        id: "shaft",
        room: RoomKind::MineHead,
        at: SHAFT,
        to: Dest::Delve(Pack::Cavern),
        label: "DOWN THE SHAFT INTO THE MINES",
        needs: None,
    },
    Entrance {
        id: "hall",
        room: RoomKind::MineHead,
        at: super::barony::HALL_DOOR,
        to: Dest::Room(RoomKind::KingsHall, (12.0, 10.6)),
        label: "INTO THE KING'S HALL",
        needs: Some("hall"),
    },
    Entrance {
        id: "meadow-coach",
        room: RoomKind::Meadow,
        at: super::countryside::COACH,
        to: Dest::Room(RoomKind::Gate, COURTYARD_ARRIVAL),
        label: "COACH BACK TO THE GATE",
        needs: Some("country:inn"),
    },
    Entrance {
        id: "hall-door",
        room: RoomKind::KingsHall,
        at: super::barony::HALL_EXIT,
        to: Dest::Room(RoomKind::MineHead, (6.0, 8.4)),
        label: "OUT TO THE MINE-HEAD",
        needs: None,
    },
    Entrance {
        id: "gate-coach",
        room: RoomKind::Gate,
        at: super::countryside::COACH,
        to: Dest::Room(RoomKind::Meadow, (12.0, 9.0)),
        label: "COACH TO THE BENT SPOON INN",
        needs: Some("country:inn"),
    },
    Entrance {
        id: "marsh-ferry",
        room: RoomKind::Marsh,
        at: super::countryside::MARSH_FERRY,
        to: Dest::Room(RoomKind::Riverbank, (7.5, 8.0)),
        label: "SIR DAMP'S FERRY TO THE QUAY",
        needs: Some("country:ferry"),
    },
    Entrance {
        id: "swamp-plate",
        room: RoomKind::Marsh,
        at: super::swamp::PLATE,
        to: Dest::Delve(Pack::Swamp),
        label: "DOWN INTO THE SWAMP OF HORRORS",
        needs: None,
    },
    Entrance {
        id: "quay-ferry",
        room: RoomKind::Riverbank,
        at: super::countryside::QUAY_FERRY,
        to: Dest::Room(RoomKind::Marsh, (5.5, 7.0)),
        label: "SIR DAMP'S FERRY TO THE MARSH",
        needs: Some("country:ferry"),
    },
    Entrance {
        id: "windmill-lift",
        room: RoomKind::Windmill,
        at: super::countryside::LIFT,
        to: Dest::Room(RoomKind::Cliffs, super::countryside::LIFT_FRONT),
        label: "GOAT LIFT TO GULLIBLE CLIFFS",
        needs: Some("country:lift"),
    },
    Entrance {
        id: "cliffs-lift",
        room: RoomKind::Cliffs,
        at: super::countryside::LIFT,
        to: Dest::Room(RoomKind::Windmill, super::countryside::LIFT_FRONT),
        label: "GOAT LIFT TO WONKY WINDMILL",
        needs: Some("country:lift"),
    },
    Entrance {
        id: "lists-post",
        room: RoomKind::Lists,
        at: super::countryside::POST_FLIGHT,
        to: Dest::Room(RoomKind::Priory, (19.5, 8.2)),
        label: "PIGEON POST TO PUDDINGSTONE PRIORY",
        needs: Some("country:post"),
    },
    Entrance {
        id: "priory-post",
        room: RoomKind::Priory,
        at: super::countryside::POST_FLIGHT,
        to: Dest::Room(RoomKind::Lists, (19.5, 9.0)),
        label: "PIGEON POST TO THE JOUSTING LISTS",
        needs: Some("country:post"),
    },
    Entrance {
        id: "observatory-ropeway",
        room: RoomKind::Observatory,
        at: super::countryside::ROPEWAY,
        to: Dest::Room(RoomKind::MineHead, (19.5, 8.2)),
        label: "ROPEWAY TO THE MINE-HEAD",
        needs: Some("country:ropeway"),
    },
    Entrance {
        id: "mine-ropeway",
        room: RoomKind::MineHead,
        at: super::countryside::ROPEWAY,
        to: Dest::Room(RoomKind::Observatory, (19.5, 8.2)),
        label: "ROPEWAY TO CUCKOO OBSERVATORY",
        needs: Some("country:ropeway"),
    },
    Entrance {
        id: "saltworks-barge",
        room: RoomKind::Saltworks,
        at: super::countryside::BARGE,
        to: Dest::Room(RoomKind::Riverbank, (4.0, 7.8)),
        label: "HARBOUR BARGE: WASHBUCKET QUAY",
        needs: Some("country:harbour"),
    },
    Entrance {
        id: "quay-barge",
        room: RoomKind::Riverbank,
        at: super::countryside::QUAY_BARGE,
        to: Dest::Room(RoomKind::Saltworks, (19.5, 8.0)),
        label: "HARBOUR BARGE: SALTSPITE WORKS",
        needs: Some("country:harbour"),
    },
    Entrance {
        id: "abbey-boat",
        room: RoomKind::SunkenAbbey,
        at: super::wetland::BOAT,
        to: Dest::Room(RoomKind::Orchard, (19.5, 6.5)),
        label: "PUNT TO APPLE APOLOGY ORCHARD",
        needs: Some("country:punthouse"),
    },
    Entrance {
        id: "orchard-boat",
        room: RoomKind::Orchard,
        at: super::wetland::ORCHARD_BOAT,
        to: Dest::Room(RoomKind::SunkenAbbey, (19.5, 8.2)),
        label: "PUNT TO THE SUNKEN ABBEY",
        needs: Some("country:punthouse"),
    },
    Entrance {
        id: "aviary-mail",
        room: RoomKind::Aviary,
        at: super::aerie::FLIGHT,
        to: Dest::Room(RoomKind::Rookery, (5.0, 8.2)),
        label: "AIR-MAIL CART TO ROOKERY PASS",
        needs: Some("country:aerie"),
    },
    Entrance {
        id: "rookery-mail",
        room: RoomKind::Rookery,
        at: super::aerie::ROOKERY_FLIGHT,
        to: Dest::Room(RoomKind::Aviary, (19.5, 8.2)),
        label: "AIR-MAIL CART TO PIGEON PARLIAMENT",
        needs: Some("country:aerie"),
    },
    Entrance {
        id: "bothy-wagon",
        room: RoomKind::Bothy,
        at: super::woodland::WAGON,
        to: Dest::Room(RoomKind::Orchard, (9.0, 9.2)),
        label: "WAGON TO ORCHARD OF MINOR DISPUTES",
        needs: Some("country:bothy"),
    },
    Entrance {
        id: "orchard-wagon",
        room: RoomKind::Orchard,
        at: super::woodland::ORCHARD_WAGON,
        to: Dest::Room(RoomKind::Bothy, (19.5, 8.2)),
        label: "WAGON TO BADGER'S BOTHY",
        needs: Some("country:bothy"),
    },
    Entrance { id: "kite-out", room: RoomKind::Kitewright, at: super::windcountry::KITE,
        to: Dest::Room(RoomKind::Windmill, (15.0, 6.2)),
        label: "EXTREMELY UNWISE KITE TO WINDMILL", needs: Some("country:kitepost") },
    Entrance { id: "kite-back", room: RoomKind::Windmill, at: super::windcountry::MILL_KITE,
        to: Dest::Room(RoomKind::Kitewright, (19.5, 8.2)),
        label: "EXTREMELY UNWISE KITE TO KITEWRIGHT", needs: Some("country:kitepost") },
    Entrance { id: "quiver-out", room: RoomKind::Fletchery, at: super::bowcountry::COACH,
        to: Dest::Room(RoomKind::Village, super::bowcountry::VILLAGE_FRONT),
        label: "QUIVER COACH TO BROADSHEET ROW", needs: Some("country:quivercoach") },
    Entrance { id: "quiver-back", room: RoomKind::Village, at: super::bowcountry::VILLAGE_COACH,
        to: Dest::Room(RoomKind::Fletchery, (19.5, 8.2)),
        label: "QUIVER COACH TO THE FLETCHERY", needs: Some("country:quivercoach") },
];

/// Does the entrance stand open, by what the realm has built?
pub fn open(e: &Entrance, home: &super::home::Home) -> bool {
    e.needs.is_none_or(|w| {
        if let Some(id) = w.strip_prefix("country:") {
            super::countryside::PLOTS
                .iter()
                .position(|p| p.id == id)
                .is_some_and(|i| home.countryside.has(i))
        } else {
            home.barony.built(w)
        }
    })
}

fn inside((col, row, w, h): (i32, i32, i32, i32), c: i32, r: i32) -> bool {
    c >= col && c < col + w && r >= row && r < row + h
}

/// The entrance whose tiles hold tile `(col, row)` of a room of `kind`.
pub fn entrance_at(kind: RoomKind, col: i32, row: i32) -> Option<&'static Entrance> {
    ENTRANCES
        .iter()
        .find(|e| e.room == kind && inside(e.at, col, row))
}

/// The entrance a standing knight is on, if any.
pub fn entrance_under(run: &Run) -> Option<(&'static Entrance, u32)> {
    let kind = run.room().kind;
    run.players.iter().find_map(|(&id, h)| {
        (h.hp > 0 && !h.stone)
            .then(|| {
                entrance_at(
                    kind,
                    (h.x / TILE_UNITS).floor() as i32,
                    (h.y / TILE_UNITS).floor() as i32,
                )
            })
            .flatten()
            .map(|e| (e, id))
    })
}

/// The middle of an entrance's tiles, in arena units.
pub fn entrance_centre(e: &Entrance) -> (f32, f32) {
    let (c, r, w, h) = e.at;
    (
        (c as f32 + w as f32 / 2.0) * TILE_UNITS,
        (r as f32 + h as f32 / 2.0) * TILE_UNITS,
    )
}

impl RoomKind {
    /// A room of the world above the Delve.
    pub fn in_world(self) -> bool {
        matches!(
            self,
            RoomKind::Kingdom
                | RoomKind::RealmHall
                | RoomKind::LandOffice
                | RoomKind::Gate
                | RoomKind::Stables
                | RoomKind::Lists
                | RoomKind::MineHead
                | RoomKind::KingsHall
                | RoomKind::Forest
                | RoomKind::Village
                | RoomKind::Fairground
                | RoomKind::Riverbank
                | RoomKind::Windmill
                | RoomKind::Ruins
                | RoomKind::Meadow
                | RoomKind::Orchard
                | RoomKind::Marsh
                | RoomKind::Downs
                | RoomKind::Cliffs
                | RoomKind::Causeway
                | RoomKind::Priory
                | RoomKind::Crag
                | RoomKind::Rookery
                | RoomKind::Observatory
                | RoomKind::Strand
                | RoomKind::Saltworks
                | RoomKind::Lighthouse
                | RoomKind::Timberyard
                | RoomKind::HedgeMaze
                | RoomKind::Bothy
                | RoomKind::Millpond
                | RoomKind::Sluice
                | RoomKind::SunkenAbbey
                | RoomKind::Belfry
                | RoomKind::Aqueduct
                | RoomKind::Aviary
                | RoomKind::Mummers
                | RoomKind::Laundry
                | RoomKind::Caravan
                | RoomKind::Hippodrome
                | RoomKind::Wheelwright
                | RoomKind::TollBridge
                | RoomKind::GooseGreen
                | RoomKind::Kitewright
                | RoomKind::SignalHill
                | RoomKind::CollegeGrounds
                | RoomKind::CollegeHall
                | RoomKind::CollegeClass
                | RoomKind::CollegeLibrary
                | RoomKind::CollegeLab
                | RoomKind::TowerEntry
                | RoomKind::TowerStairs
                | RoomKind::TowerStars
                | RoomKind::WizardStudy
                | RoomKind::Ranch
                | RoomKind::Pasture
                | RoomKind::LongbowGreen
                | RoomKind::Fletchery
                | RoomKind::Scarecrow
                | RoomKind::Market
                | RoomKind::Bathhouse
                | RoomKind::Smithy
                | RoomKind::Eyrie
                | RoomKind::ColosseumHall
                | RoomKind::KegYard
                | RoomKind::GanderHall
        )
    }
}

fn walled(cell: (i32, i32), kind: RoomKind) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = kind;
    for row in 1..ROWS - 1 {
        for col in 1..COLS - 1 {
            room.set(col, row, Tile::Floor);
        }
    }
    room
}

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for row in r..r + h {
        for col in c..c + w {
            room.set(col as usize, row as usize, tile);
        }
    }
}

/// The well in the courtyard, and the notice post by the road.
pub const WELL: (i32, i32, i32, i32) = (17, 3, 2, 2);
pub const POST: (i32, i32, i32, i32) = (15, 10, 1, 1);

/// The Delve's gate courtyard: the gate's arch over its stair in the north
/// west, a well, the road in from the realm to the south, and doors to the
/// mine-head (north), the stables (west) and the lists (east).
pub fn gate_room(cell: (i32, i32)) -> Room {
    let mut room = walled(cell, RoomKind::Gate);
    fill(&mut room, GATE_ARCH, Tile::Block);
    fill(&mut room, GATE_STAIR, Tile::Stairs);
    fill(&mut room, WELL, Tile::Block);
    fill(&mut room, POST, Tile::Block);
    fill(&mut room, super::countryside::COACH, Tile::Stairs);
    room.open_door(0);
    room.open_door(1);
    room.open_door(3);
    room
}

/// The stable block along the north wall, its three stalls' doors open to
/// the yard, and the paddock fence to the south.
pub const STABLE_BLOCK: (i32, i32, i32, i32) = (2, 1, 20, 3);
pub const TROUGH: (i32, i32, i32, i32) = (3, 6, 2, 1);
pub const PADDOCK: (i32, i32, i32, i32) = (4, 10, 16, 1);

pub fn stables_room(cell: (i32, i32)) -> Room {
    let mut room = walled(cell, RoomKind::Stables);
    fill(&mut room, STABLE_BLOCK, Tile::Block);
    fill(&mut room, TROUGH, Tile::Block);
    fill(&mut room, PADDOCK, Tile::Block);
    room.open_door(1);
    room
}

/// The lists: the tilt down the middle, the stands along the north wall,
/// a pavilion at either end.
pub const TILT: (i32, i32, i32, i32) = (3, 7, 18, 1);
pub const STANDS: (i32, i32, i32, i32) = (3, 1, 18, 2);
pub const PAVILIONS: [(i32, i32, i32, i32); 2] = [(1, 10, 2, 2), (21, 1, 2, 2)];

pub fn lists_room(cell: (i32, i32)) -> Room {
    let mut room = walled(cell, RoomKind::Lists);
    fill(&mut room, TILT, Tile::Block);
    fill(&mut room, STANDS, Tile::Block);
    for p in PAVILIONS {
        fill(&mut room, p, Tile::Block);
    }
    room.open_door(3);
    room
}

/// The fallen gate-hall of Caer Dwfn at the mine-head, west of the adit;
/// the spoil heaps east of it.
pub const OLD_HALL: (i32, i32, i32, i32) = (2, 2, 8, 5);
pub const SPOIL: (i32, i32, i32, i32) = (17, 2, 4, 2);

pub fn mine_head_room(cell: (i32, i32)) -> Room {
    let mut room = walled(cell, RoomKind::MineHead);
    fill(&mut room, ADIT, Tile::Block);
    fill(&mut room, SHAFT, Tile::Stairs);
    fill(&mut room, OLD_HALL, Tile::Block);
    fill(&mut room, super::barony::HALL_DOOR, Tile::Stairs);
    fill(&mut room, SPOIL, Tile::Block);
    fill(&mut room, (11, 1, 3, 3), Tile::Floor);
    room.open_door(2);
    room
}

fn world_room(kind: RoomKind, cell: (i32, i32)) -> Room {
    match kind {
        kind if super::college::is_room(kind) => super::college::room(kind, cell),
        kind if super::ranch::is_room(kind) => super::ranch::room(kind, cell),
        kind if super::colosseum::is_room(kind) => super::colosseum::room(kind, cell),
        RoomKind::Gate => gate_room(cell),
        RoomKind::Stables => stables_room(cell),
        RoomKind::Lists => lists_room(cell),
        RoomKind::KingsHall => super::barony::kings_hall_room(cell),
        RoomKind::LandOffice => super::holdings::land_office_room(cell),
        RoomKind::Forest => {
            let mut room = walled(cell, kind);
            for (col, row) in super::forest::TREES {
                room.set(col as usize, row as usize, Tile::Block);
            }
            room.open_door(0);
            room
        }
        RoomKind::Village
        | RoomKind::Fairground
        | RoomKind::Riverbank
        | RoomKind::Windmill
        | RoomKind::Ruins
        | RoomKind::Meadow
        | RoomKind::Orchard
        | RoomKind::Marsh
        | RoomKind::Downs
        | RoomKind::Cliffs
        | RoomKind::Causeway
        | RoomKind::Priory
        | RoomKind::Crag
        | RoomKind::Rookery
        | RoomKind::Observatory
        | RoomKind::Strand
        | RoomKind::Saltworks
        | RoomKind::Lighthouse
        | RoomKind::Timberyard
        | RoomKind::HedgeMaze
        | RoomKind::Bothy
        | RoomKind::Millpond
        | RoomKind::Sluice
        | RoomKind::SunkenAbbey
        | RoomKind::Belfry
        | RoomKind::Aqueduct
        | RoomKind::Aviary
                | RoomKind::Mummers
                | RoomKind::Laundry
                | RoomKind::Caravan
                 | RoomKind::Hippodrome
                 | RoomKind::Wheelwright
                 | RoomKind::TollBridge
                 | RoomKind::GooseGreen
                 | RoomKind::Kitewright
                 | RoomKind::SignalHill
                 | RoomKind::LongbowGreen
                 | RoomKind::Fletchery
                 | RoomKind::Scarecrow
                 | RoomKind::Market
                 | RoomKind::Bathhouse
                 | RoomKind::Smithy
                 | RoomKind::Eyrie => super::countryside::room(walled(cell, kind)),
        _ => mine_head_room(cell),
    }
}

/// Add the world to a home floor that lacks it: its rooms, and the
/// Undercroft's stair up. True if anything was added. Only the home floor
/// (depth 0) with an Undercroft gets a world, and a cell already taken is
/// left as it is.
pub fn raise_world(floor: &mut Floor) -> bool {
    if floor.depth != 0 {
        return false;
    }
    let Some(cellar) = floor.rooms.iter().position(|r| r.kind == RoomKind::Home) else {
        return false;
    };
    let mut changed = false;
    let (c, r, w, h) = CELLAR_STAIR;
    for row in r..r + h {
        for col in c..c + w {
            if floor.rooms[cellar].tile(col, row) != Tile::Stairs {
                floor.rooms[cellar].set(col as usize, row as usize, Tile::Stairs);
                changed = true;
            }
        }
    }
    for (kind, cell) in ROOMS {
        if floor.rooms.len() >= HOME_ROOM_LIMIT || floor.rooms.iter().any(|r| r.cell == cell) {
            continue;
        }
        floor.rooms.push(world_room(kind, cell));
        changed = true;
    }
    // New travel plates also migrate into existing authored surface rooms.
    // Never touch an unrelated room that happens to occupy a reserved cell.
    for e in ENTRANCES.iter().filter(|e| e.room.in_world()) {
        let cell = ROOMS.iter().find(|(kind, _)| *kind == e.room).unwrap().1;
        if let Some(room) = floor
            .rooms
            .iter_mut()
            .find(|r| r.kind == e.room && r.cell == cell)
        {
            let (c, r, w, h) = e.at;
            for y in r..r + h {
                for x in c..c + w {
                    if room.tile(x, y) != Tile::Stairs {
                        room.set(x as usize, y as usize, Tile::Stairs);
                        changed = true;
                    }
                }
            }
        }
    }
    // Migrate existing checkpoints too, but only join correctly reserved rooms.
    for &(a, b) in TRAILS {
        let reserved = |kind| ROOMS.iter().find(|(k, _)| *k == kind).map(|(_, c)| *c);
        let endpoint = |kind| {
            floor
                .rooms
                .iter()
                .position(|r| r.kind == kind && Some(r.cell) == reserved(kind))
        };
        let (Some(ai), Some(bi)) = (endpoint(a), endpoint(b)) else {
            continue;
        };
        let ac = floor.rooms[ai].cell;
        let bc = floor.rooms[bi].cell;
        let Some(dir) = layout::DIRS
            .iter()
            .position(|&(x, y)| (ac.0 + x, ac.1 + y) == bc)
        else {
            continue;
        };
        for (index, side) in [(ai, dir), (bi, (dir + 2) % 4)] {
            if !floor.rooms[index].doors[side] {
                floor.rooms[index].open_door(side);
                changed = true;
            }
            // A north road from the caravan must clear all four arriving
            // knights, not just the two centre tiles of the grandstand. The
            // outer radius grazes columns 9 and 14. Keep the tilting rail.
            if floor.rooms[index].kind == RoomKind::Lists && side == 0 {
                for y in 1..3 {for x in 9..15 {
                    if floor.rooms[index].tile(x,y) == Tile::Block {
                        floor.rooms[index].set(x as usize,y as usize,Tile::Floor); changed=true;
                    }
                }}
            }
            // The lift on Cliffs touches the west edge of the north fan.
            if floor.rooms[index].kind == RoomKind::Cliffs && side == 0 {
                for y in 1..4 {
                    for x in 9..11 {
                        if floor.rooms[index].tile(x, y) == Tile::Block {
                            floor.rooms[index].set(x as usize, y as usize, Tile::Floor);
                            changed = true;
                        }
                    }
                }
            }
            // The adit and pre-mummers spoil pile block the newly opened
            // north fan. Clear its full four-knight width on fresh and saved
            // floors, leaving the shaft's Stairs and great door untouched.
            if floor.rooms[index].kind == RoomKind::MineHead && side == 0 {
                for y in 1..4 {
                    for x in 9..15 {
                        if floor.rooms[index].tile(x, y) == Tile::Block {
                            floor.rooms[index].set(x as usize, y as usize, Tile::Floor);
                            changed = true;
                        }
                    }
                }
            }
            // Clear the stable wall's full four-knight arrival fan in old saves
            // as well as new floors;
            // unrelated authored/settlement rooms are not touched.
            if floor.rooms[index].kind == RoomKind::Stables && side == 0 {
                for y in 1..5 {
                    for x in 9..15 {
                        if floor.rooms[index].tile(x, y) == Tile::Block {
                            floor.rooms[index].set(x as usize, y as usize, Tile::Floor);
                            changed = true;
                        }
                    }
                }
            }
        }
    }
    if changed {
        floor.world_revision = floor.world_revision.wrapping_add(1);
    }
    changed
}

/// The home floor as it lies below the world: without the world's rooms or
/// the Undercroft's stair up to them. A saved loop settlement's floor is
/// this, exactly.
#[cfg(any(test, feature = "testing"))]
pub fn below_the_world(floor: &Floor) -> Floor {
    let mut below = floor.clone();
    below.world_revision = 0;
    below.rooms.retain(|r| !r.kind.in_world());
    if let Some(cellar) = below.rooms.iter().position(|r| r.kind == RoomKind::Home) {
        let (c, r, w, h) = CELLAR_STAIR;
        for row in r..r + h {
            for col in c..c + w {
                below.rooms[cellar].set(col as usize, row as usize, Tile::Floor);
            }
        }
    }
    below
}

/// A room of the home floor by kind.
pub fn room_of(floor: &Floor, kind: RoomKind) -> Option<usize> {
    floor.rooms.iter().position(|r| r.kind == kind)
}

impl Run {
    /// One tick in a room of the world (the lists tick at the top of the
    /// step, before the saddled knight's keys are taken from them).
    pub(super) fn tick_world(&mut self, inputs: &BTreeMap<u32, Input>) {
        // The kingdom, its rooms' games and the pastimes, from the registry
        // (`activity::WORLD`).
        self.tick_world_games(inputs);
    }

    /// Walk the party into room `index` and stand them around `at` (tiles),
    /// side by side.
    pub fn arrive(&mut self, index: usize, at: (f32, f32)) {
        self.enter(index, None);
        let ids: Vec<u32> = self.players.keys().copied().collect();
        let count = ids.len() as f32;
        for (slot, id) in ids.into_iter().enumerate() {
            let offset = (slot as f32 - (count - 1.0) / 2.0) * super::stand_apart(count);
            if let Some(hero) = self.players.get_mut(&id) {
                (hero.x, hero.y) = (at.0 * TILE_UNITS + offset, at.1 * TILE_UNITS);
            }
        }
        self.tallow_follows();
        self.hireling_follows();
        // A knight who arrives standing on stairs steps off before they work.
        self.stairs_held = true;
        self.record_kingdom_visit();
    }

    /// A new delve, begun in the world: in the gate courtyard, the gate's
    /// stair before the party, the Undercroft below it.
    #[cfg(any(test, feature = "testing"))]
    pub fn come_to_the_gate(&mut self) {
        raise_world(&mut self.dungeon);
        if let Some(gate) = room_of(&self.dungeon, RoomKind::Gate) {
            self.arrive(gate, COURTYARD_ARRIVAL);
            // The welcome home waits for the Undercroft itself.
            self.cues.retain(|c| c != "home");
            self.cues.push("the_gate".into());
        }
    }

    /// Through an entrance: to another room, or down into a delve.
    pub fn take_entrance(&mut self, e: &Entrance) {
        if self.returns_to_kingdom(e.id) {
            self.leave_kingdom_place();
            return;
        }
        match e.to {
            Dest::Room(kind, at) => {
                let Some(index) = room_of(&self.dungeon, kind) else {
                    return;
                };
                let from_surface = self.room().kind.in_world();
                self.arrive(index, at);
                if kind == RoomKind::Home && from_surface {
                    self.begin_floor_plate(super::floor_plates::Kind::Gate);
                }
                self.sounds.push("door_open");
                self.cues.push(if kind == RoomKind::Home {
                    "home".to_string()
                } else {
                    format!("entrance:{}", e.id)
                });
            }
            Dest::Delve(pack) => {
                // The shaft goes to the first floor of its delve, whatever
                // landing Wren has drawn on the Winding Stair.
                let landing = self.home.landing;
                self.home.landing = 1;
                self.dungeon.pack = pack;
                self.cues.push(format!("entrance:{}", e.id));
                self.descend();
                self.home.landing = landing;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_world__tests.rs"]
mod tests;
