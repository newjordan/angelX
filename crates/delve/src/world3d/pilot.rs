//! The pilot: ten fixed single-view rooms, blocked out from the game's own
//! layouts. Six are rooms of the kingdom's places (`world::ROOMS`, entered
//! from the kingdom), indoor and out; four are the dungeon's hand-drawn
//! one-screen chambers (`rooms::small`), one from each of four delves.
//!
//! - **smithy**, The Dented Anvil: a hall, its hearth, anvil, rack and
//!   quench trough (`town`).
//! - **market**, Haggler's Square: a hall of three stalls and the cross.
//! - **stables**: two thatched wings either side of the road north, a
//!   trough, the paddock fence, the ranch gate; a wood round it.
//! - **village**, Broadsheet Row: five cottages of three roofs and two
//!   walls, roads crossing (`pastimes::scenery`).
//! - **orchard**: four apple trees, a punt landing and a wagon stop, in
//!   copper woods.
//! - **marsh**, Sir Damp's Marsh: open water north and south-west, the
//!   ferry's jetty, the plate down into the Swamp, a staked site.
//! - **crypt-bone-chapel**: the Crypt's nave pillars and tombs.
//! - **cavern-boulder-rill**: the Mines' boulders round a sink pool.
//! - **hellforge-lava-ford**: Dragon Keep's lava tongue and its ford.
//! - **fungal-fairy-ring**: the Fungal Deep's broken ring of mushrooms
//!   round a spore pool.
//!
//! A place room is taken from the home floor exactly as a run raises it
//! (`world::raise_world`: its roads' doors and cleared arrival fans
//! included); a chamber is its quadrant mirrored by the game's own
//! `layout::mirror`, with all four doors open.

use super::blockout::{Dressing, Edge, Field, Kind, Leaf, Prop, Setting};
use crate::rules::{
    Pack, Room, RoomKind, Tile, barony, bowcountry, countryside, layout, nooks, ranch, rooms, swamp,
    town::{self, ANVIL, BARREL, CROSS, HEARTH, RACK},
    wetland, woodland, world,
};

/// Where an item's room comes from.
#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// A room of the world, from the home floor.
    Place(RoomKind),
    /// A delve's hand-drawn one-screen chamber: its pack, which of its
    /// `SMALL` quadrants, its doors (north, east, south, west).
    Chamber { pack: Pack, small: usize, doors: [bool; 4] },
    /// A delve's bigger hand-drawn chamber: its pack, which of its
    /// `all_great` quadrants; all four doors open.
    Great { pack: Pack, index: usize },
    /// A nook below ground (`rules::nooks`), its doorway on the west.
    Nook(RoomKind),
    /// The Sanctuary: a safe room with the reforge altar, four doors.
    Sanctuary,
    /// The vault behind a cracked wall: pillars at its corners.
    Vault,
    /// The King's forge-hall below: a furnace and two anvils.
    Forge,
}

/// One item of the pilot: a room and what its tiles are.
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub source: Source,
    pub dressing: Dressing,
}

impl Item {
    pub fn id(&self) -> &'static str {
        self.dressing.id
    }

    /// The room as the game lays it.
    pub fn room(&self) -> Room {
        match self.source {
            Source::Place(kind) => {
                let mut floor = layout::undercroft(Pack::Crypt);
                world::raise_world(&mut floor);
                // Maud's tavern is dug out west of the Trophy Hall.
                if kind == RoomKind::Tavern {
                    layout::dig_tavern(&mut floor);
                }
                let i = world::room_of(&floor, kind).expect("a room of the world");
                floor.rooms[i].clone()
            }
            Source::Chamber { pack, small, doors } => {
                let mut room = Room::solid_rock((0, 0));
                room.kind = RoomKind::Fight;
                room.doors = doors;
                let quadrant: Vec<Vec<u8>> = rooms::small(pack)[small].iter().map(|r| r.as_bytes().to_vec()).collect();
                layout::mirror(&mut room, doors, &quadrant);
                room
            }
            Source::Great { pack, index } => {
                let quadrant: Vec<Vec<u8>> = rooms::all_great(pack)[index].iter().map(|r| r.as_bytes().to_vec()).collect();
                let (cols, rows) = (quadrant[0].len() * 2 + 2, quadrant.len() * 2 + 2);
                let mut room = Room::kingdom(vec![Tile::Wall; cols * rows], cols, rows);
                room.kind = RoomKind::Fight;
                layout::mirror(&mut room, [true; 4], &quadrant);
                room.doors = [true; 4];
                room
            }
            Source::Nook(kind) => nooks::room(kind, (0, 0), 3),
            Source::Sanctuary => {
                let mut room = Room::solid_rock((0, 0));
                room.kind = RoomKind::Sanctuary;
                let (cols, rows) = (room.cols, room.rows);
                for row in 1..rows - 1 {
                    for col in 1..cols - 1 {
                        room.set(col, row, Tile::Floor);
                    }
                }
                for dir in 0..4 {
                    room.open_door(dir);
                }
                room
            }
            Source::Vault => {
                // `layout::vault`: pillars at the corners, one doorway.
                let mut room = Room::solid_rock((0, 0));
                room.kind = RoomKind::Secret;
                let (cols, rows) = (room.cols, room.rows);
                for row in 1..rows - 1 {
                    for col in 1..cols - 1 {
                        room.set(col, row, Tile::Floor);
                    }
                }
                for (col, row) in [(4, 3), (cols - 5, 3), (4, rows - 4), (cols - 5, rows - 4)] {
                    room.set(col, row, Tile::Block);
                }
                room.open_door(2);
                room
            }
            Source::Forge => barony::forge_room((0, 0), 2),
        }
    }

    /// Its blockout set: the generated room, with its landmark overlay
    /// (`assets/world3d/<item>/landmark.set`) appended when it has one.
    pub fn set(&self) -> String {
        self.set_of(&self.room(), &super::blockout::landmark_folder())
    }

    /// Its blockout set as the generator makes it of `room` (the item's own,
    /// [`Item::room`]), with the landmark overlay the asset folder `folder`
    /// holds for it (`<folder>/<item>/landmark.set`), if any.
    pub fn set_of(&self, room: &Room, folder: &std::path::Path) -> String {
        let landmark = super::blockout::landmark_in(folder, self.id());
        super::blockout::room_set_with(room, &self.dressing, landmark.as_deref())
    }

    /// What kind of place it is for the art lane: `outdoor`, `interior` or
    /// `dungeon`.
    pub fn setting_word(&self) -> &'static str {
        match self.dressing.setting {
            Setting::Outdoor { .. } | Setting::Courtyard { .. } => "outdoor",
            Setting::Indoor | Setting::Hall => "interior",
            Setting::Dungeon(_) => "dungeon",
        }
    }

    /// A painter's brief for the item: what the place is, what each kind of
    /// block stands for, where the light is, and the mood.
    pub fn painter_notes(&self) -> String {
        use super::blockout::{Kind, Role, blockout};
        let d = &self.dressing;
        let groups = blockout(&self.room(), d);
        // The blocks, by what they stand for, in the order they come.
        let mut words: Vec<(String, usize)> = Vec::new();
        for g in groups.iter().filter(|g| g.role == Role::Prop && g.rect.is_some()) {
            let word = g.name.split(" at ").next().unwrap_or(&g.name).to_string();
            if matches!(word.as_str(), "plate" | "rug" | "deck") && !self.setting_word().eq("outdoor") {
                continue;
            }
            match words.iter_mut().find(|(w, _)| *w == word) {
                Some((_, n)) => *n += 1,
                None => words.push((word, 1)),
            }
        }
        let blocks = words
            .iter()
            .map(|(w, n)| if *n > 1 { format!("{w} x{n}") } else { w.clone() })
            .collect::<Vec<_>>()
            .join(", ");
        let wet = groups.iter().find(|g| g.role == Role::Hazard && !g.solids.is_empty()).map(|g| {
            match g.solids[0].kind {
                Kind::Fire => "lava",
                Kind::Mud => "spore pools",
                _ => "water",
            }
        });
        // Light: the fires and windows the set has, and where they stand.
        let mut lights: Vec<String> = Vec::new();
        let mut light = |what: &str| {
            if !lights.iter().any(|l| l == what) {
                lights.push(what.to_string());
            }
        };
        for g in &groups {
            let lit = g.solids.iter().any(|s| matches!(s.kind, Kind::Fire | Kind::Window));
            if !lit {
                continue;
            }
            match g.role {
                Role::Door => light("torches at the doorways"),
                Role::Wall => light("braziers on the wall corners"),
                Role::Hazard => light("the lava glow"),
                Role::Prop if g.rect.is_some() => {
                    let word = g.name.split(" at ").next().unwrap_or(&g.name).to_string();
                    light(&match word.as_str() {
                        "hearth" | "furnace" | "cauldron" | "beacon" => format!("the {word}'s fire"),
                        "column" | "pier" => "torches on the piers".to_string(),
                        "tower" | "mill" | "cottage" => "lit windows".to_string(),
                        _ => format!("the {word}'s lamps"),
                    });
                }
                _ => {}
            }
        }
        for &(fix, _) in d.fixtures {
            light(match fix {
                super::blockout::Fix::Window => "windows in the far wall",
                super::blockout::Fix::Banner => continue,
                super::blockout::Fix::Hearth => "the hearth in the far wall",
                super::blockout::Fix::Lamp => "lamps on the far wall",
            });
        }
        let mut out = d.what.trim().to_string();
        if !blocks.is_empty() {
            out.push_str(&format!(" Blocks: {blocks}."));
        }
        if let Some(wet) = wet {
            out.push_str(&format!(" Hazard tiles are {wet} sunk below the floor line."));
        }
        if lights.is_empty() {
            out.push_str(" Dusk, and dark: no lamp, window or fire among the blocks, so none beyond what the paint adds.");
        } else {
            out.push_str(&format!(" Dusk, and dark: light only at {}.", lights.join(", ")));
        }
        // What the game paints on open ground, which the blocks leave to
        // the paint (and where its lamps are).
        let painted: Vec<&str> = d
            .notes
            .iter()
            .filter_map(|n| n.strip_prefix("Painted on open ground, not blocks: ").or_else(|| n.strip_prefix("Painted on open floor, not blocks: ")))
            .collect();
        if !painted.is_empty() {
            out.push_str(&format!(" Painted over open floor, not blocks: {}", painted.join(" ")));
        }
        let other: Vec<&str> = d.notes.iter().copied().filter(|n| !n.starts_with("Painted on open") && !n.starts_with("Torches flank")).collect();
        for n in other {
            out.push(' ');
            out.push_str(n);
        }
        out
    }
}

/// The stable block's stall doors (`world::stall_doors`), x and width.
const STALL_DOORS: [(f32, f32); 3] = [(80.0, 22.0), (181.0, 22.0), (282.0, 22.0)];

pub const PILOT: [Item; 10] = [
    Item {
        source: Source::Place(RoomKind::Smithy),
        dressing: Dressing {
            id: "smithy",
            title: "The Dented Anvil, the town's smithy (RoomKind::Smithy)",
            setting: Setting::Indoor,
            roads: &[],
            props: &[
                (HEARTH, Prop::Hearth),
                (ANVIL, Prop::Anvil),
                (RACK, Prop::Rack),
                (BARREL, Prop::Trough { depth: 18.0, height: 10.0 }),
                // A stack of bar iron, waiting; the coal heap.
                ((7, 10, 2, 1), Prop::Low(Kind::Metal, "bar-iron")),
                ((16, 11, 2, 1), Prop::Heap(Kind::Rock, "coal")),
            ],
            notes: &[
                "Painted on open floor, not blocks: the blade rack (3,9), soot round the anvil.",
                "Old Dint the smith is a sprite, at the anvil's east.",
            ],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Place(RoomKind::Market),
        dressing: Dressing {
            id: "market",
            title: "Haggler's Square, the town's market hall (RoomKind::Market)",
            setting: Setting::Indoor,
            roads: &[],
            props: &[
                (town::STALLS[0].counter, Prop::Stall { north: false, goods: Kind::Rock }),
                (town::STALLS[1].counter, Prop::Stall { north: false, goods: Kind::Plaster }),
                (town::STALLS[2].counter, Prop::Stall { north: true, goods: Kind::Metal }),
                (CROSS, Prop::Cross),
                // Hay by the north wall.
                ((9, 2, 2, 1), Prop::Heap(Kind::Crop, "hay")),
            ],
            notes: &[
                "Painted on open floor, not blocks: the bunting across the hall, crates and barrels by the south wall.",
                "Magda, the chandler and Soup Ned are sprites behind their counters.",
            ],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Place(RoomKind::Stables),
        dressing: Dressing {
            id: "stables",
            title: "The stables (RoomKind::Stables)",
            setting: Setting::Outdoor { edge: Edge::Wood(Leaf::Oak), field: Field::Grass },
            // `world::roads(Stables)`.
            roads: &[(9, 0, 6, 5), (0, 6, 24, 2), (2, 4, 20, 2), (21, 6, 3, 2), (19, 4, 2, 4), (20, 7, 2, 5), (11, 11, 10, 1), (11, 11, 2, 3)],
            props: &[
                // The stable block (`world::STABLE_BLOCK`), its middle cleared
                // for the road north: two wings, an arch between them.
                ((2, 1, 7, 3), Prop::Barn { doors: &STALL_DOORS }),
                ((15, 1, 7, 3), Prop::Barn { doors: &STALL_DOORS }),
                ((9, 1, 6, 1), Prop::Arch),
                (world::TROUGH, Prop::Trough { depth: 9.0, height: 8.0 }),
                // The paddock fence (`world::PADDOCK`), up to the coach's plate.
                ((4, 10, 15, 1), Prop::Fence),
                (ranch::STABLES_GATE, Prop::Gate),
            ],
            notes: &[
                "Painted on open ground, not blocks: the lantern on its post (12,4), hay bales and a saddle (18,6).",
            ],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Place(RoomKind::Village),
        dressing: Dressing {
            id: "village",
            title: "Broadsheet Row, the village (RoomKind::Village)",
            setting: Setting::Outdoor { edge: Edge::Wood(Leaf::Oak), field: Field::Grass },
            roads: &[(11, 0, 2, 14), (0, 6, 24, 2)],
            props: &[
                // `pastimes::scenery`: roofs thatch, tile, slate in turn; every
                // other cottage of stone; a window to each 24 pixels.
                ((3, 1, 4, 2), Prop::Cottage { walls: Kind::Stone, roof: Kind::Thatch, windows: 2 }),
                ((7, 1, 2, 2), Prop::Cottage { walls: Kind::Plaster, roof: Kind::Roof, windows: 1 }),
                ((16, 1, 5, 2), Prop::Cottage { walls: Kind::Stone, roof: Kind::Slate, windows: 3 }),
                ((3, 10, 4, 3), Prop::Cottage { walls: Kind::Plaster, roof: Kind::Thatch, windows: 2 }),
                ((15, 10, 4, 3), Prop::Cottage { walls: Kind::Stone, roof: Kind::Roof, windows: 2 }),
                (bowcountry::VILLAGE_COACH, Prop::Plate(Kind::Wood)),
            ],
            notes: &[
                "Painted on open ground, not blocks: the fence along row 9, a barrel (9,2) and a crate (21,2).",
                "The mailboxes of the broadsheet round are sprites.",
            ],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Place(RoomKind::Orchard),
        dressing: Dressing {
            id: "orchard",
            title: "The orchard of minor disputes (RoomKind::Orchard)",
            setting: Setting::Outdoor { edge: Edge::Wood(Leaf::Copper), field: Field::Grass },
            roads: &[(11, 0, 2, 8), (0, 6, 24, 2), (5, 7, 2, 3)],
            props: &[
                ((4, 2, 3, 3), Prop::Tree),
                ((17, 2, 3, 3), Prop::Tree),
                ((4, 10, 3, 2), Prop::Tree),
                ((17, 10, 3, 2), Prop::Tree),
                (wetland::ORCHARD_BOAT, Prop::Plate(Kind::Wood)),
                (woodland::ORCHARD_WAGON, Prop::Plate(Kind::Wood)),
            ],
            notes: &[
                "Painted on open ground, not blocks: the scarecrow (14,7), apples in the grass, the lamp post (6,9).",
            ],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Place(RoomKind::Marsh),
        dressing: Dressing {
            id: "marsh",
            title: "Sir Damp's Marsh (RoomKind::Marsh)",
            setting: Setting::Outdoor { edge: Edge::Wood(Leaf::Oak), field: Field::Marsh },
            roads: &[(4, 6, 20, 2), (5, 7, 2, 3), (16, 5, 2, 3), (16, 8, 5, 2)],
            props: &[
                (countryside::MARSH_FERRY, Prop::Jetty),
                (swamp::PLATE, Prop::Plate(Kind::Stone)),
                // Sir Damp's Ferry Office, staked out until the realm builds it.
                (countryside::PLOTS[3].footprint, Prop::Site),
                // Rushes standing in the water (`countryside::scenery`).
                ((3, 2, 1, 1), Prop::Reeds),
                ((8, 3, 1, 1), Prop::Reeds),
                ((19, 2, 1, 1), Prop::Reeds),
                ((14, 2, 1, 1), Prop::Reeds),
                ((2, 11, 1, 1), Prop::Reeds),
                ((10, 12, 1, 1), Prop::Reeds),
                ((6, 12, 1, 1), Prop::Reeds),
            ],
            notes: &[
                "Painted on open ground, not blocks: the swamp's notice post (15.8,8), the lamp post (17,5.5), lily pads.",
            ],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Chamber { pack: Pack::Crypt, small: 0, doors: [true; 4] },
        dressing: Dressing {
            id: "crypt-bone-chapel",
            title: "The Bone Chapel, a chamber of the Crypt",
            setting: Setting::Dungeon(Pack::Crypt),
            roads: &[],
            props: &[],
            notes: &["Torches flank every doorway; bars close them while the fight lasts (sprites)."],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Chamber { pack: Pack::Cavern, small: 0, doors: [true; 4] },
        dressing: Dressing {
            id: "cavern-boulder-rill",
            title: "The Boulder Rill, a chamber of the Mines",
            setting: Setting::Dungeon(Pack::Cavern),
            roads: &[],
            props: &[],
            notes: &["Torches flank every doorway; bars close them while the fight lasts (sprites)."],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Chamber { pack: Pack::Hellforge, small: 2, doors: [true; 4] },
        dressing: Dressing {
            id: "hellforge-lava-ford",
            title: "The Lava Ford, a chamber of Dragon Keep",
            setting: Setting::Dungeon(Pack::Hellforge),
            roads: &[],
            props: &[],
            notes: &["Torches flank every doorway; bars close them while the fight lasts (sprites)."],
            ..Dressing::BASE
        },
    },
    Item {
        source: Source::Chamber { pack: Pack::Fungal, small: 0, doors: [true; 4] },
        dressing: Dressing {
            id: "fungal-fairy-ring",
            title: "The Fairy Ring, a chamber of the Fungal Deep",
            setting: Setting::Dungeon(Pack::Fungal),
            roads: &[],
            props: &[],
            notes: &["Torches flank every doorway; bars close them while the fight lasts (sprites)."],
            ..Dressing::BASE
        },
    },
];

/// An item by its id: of the pilot, or of the second wave ([`all`]).
pub fn item(id: &str) -> Option<&'static Item> {
    all().iter().find(|i| i.id() == id)
}

/// Every item ([`every`]), made once and kept.
pub fn all() -> &'static [Item] {
    static ALL: std::sync::OnceLock<Vec<Item>> = std::sync::OnceLock::new();
    ALL.get_or_init(every)
}

/// Item `id`'s place in [`all`]: the same in every build of the game, so a
/// window and a page across a wire number an item's pictures alike.
pub fn slot(id: &str) -> Option<u32> {
    all().iter().position(|i| i.id() == id).map(|at| at as u32)
}

/// Every item: the pilot, then the second wave (every other room of the
/// kingdom's places, every chamber the dungeon masters drew, the side
/// rooms).
pub fn every() -> Vec<Item> {
    let mut all: Vec<Item> = PILOT.to_vec();
    all.extend(super::places::PLACES);
    all.extend(super::chambers::chambers());
    all.extend(super::chambers::side_rooms());
    all
}
