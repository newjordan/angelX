//! The second wave, the dungeon: every hand-drawn chamber of the seven delves
//! (`rules::rooms::*`, one-screen and bigger) the pilot did not take, and the
//! side rooms below ground (the Sanctuary, the vault, the King's forge-hall,
//! the six nooks).
//!
//! A chamber is a quadrant mirrored by the game's own `layout::mirror` with
//! all four doors open, in its pack's stone. What its blocked tiles are comes
//! from its pack, as the game paints them (`arena::room_floor`): a tomb in
//! the Crypt, a shelf in the Archive, a mushroom in the Fungal Deep, a shard
//! in the Unknown, reeds in the Swamp, rock elsewhere; the layout's own
//! comment says what the template means by them.

use super::blockout::{Dressing, Fix, Kind, Prop, Rect, Setting};
use super::pilot::{Item, Source};
use crate::rules::{Pack, RoomKind};

/// One chamber template of `rooms`: which pack's, whether a bigger room's,
/// which of its quadrants, and what it is.
struct Def {
    pack: Pack,
    great: bool,
    index: usize,
    id: &'static str,
    title: &'static str,
    what: &'static str,
}

const TORCHES: &[&str] = &["Torches flank every doorway; bars close them while the fight lasts (sprites)."];

/// Every chamber template but the four of the pilot.
const DEFS: [Def; 56] = [
    Def { pack: Pack::Crypt, great: false, index: 1, id: "crypt-slid-tombs", title: "The Slid Tombs", what: "A chamber of the Crypt: sarcophagi dragged out of line into a diagonal; slalom between them." },
    Def { pack: Pack::Crypt, great: false, index: 2, id: "crypt-weeping-vault", title: "The Weeping Vault", what: "A chamber of the Crypt: a flooded vault in the corner, a pillar pair across it; shoot over the water." },
    Def { pack: Pack::Crypt, great: false, index: 3, id: "crypt-ossuary-wall", title: "The Ossuary Wall", what: "A chamber of the Crypt: niched bones and loose stones in an open hall; a duel at range." },
    Def { pack: Pack::Crypt, great: true, index: 0, id: "crypt-chapel-apse", title: "The Chapel Apse", what: "A bigger chamber of the Crypt: a broken apse arch off the corner, its floor flooded; the dead wait in the curve." },
    Def { pack: Pack::Crypt, great: true, index: 1, id: "crypt-reliquary-aisle", title: "The Reliquary Aisle", what: "A bigger chamber of the Crypt: three reliquaries a side down the aisle and a casket at the crossing; long lanes." },
    Def { pack: Pack::Crypt, great: true, index: 2, id: "crypt-drowned-ossuary", title: "The Drowned Ossuary", what: "A bigger chamber of the Crypt: two sunken tombs and sliding water; wade the dry diagonal." },
    Def { pack: Pack::Crypt, great: true, index: 3, id: "crypt-flooded-gallery", title: "The Flooded Gallery", what: "A bigger chamber of the Crypt: a moat straight across the gallery and shelf-butts beyond it; fight over the water." },
    Def { pack: Pack::Crypt, great: true, index: 4, id: "crypt-sepulchre-field", title: "The Sepulchre Field", what: "A bigger chamber of the Crypt: tombs set on the half-step so the ranks interleave; no straight lane through." },
    Def { pack: Pack::Crypt, great: true, index: 5, id: "crypt-processional", title: "The Processional", what: "A bigger chamber of the Crypt: pillar pairs and casket butlers along the walk, pooled crossings between; circle the pillars." },
    Def { pack: Pack::Crypt, great: true, index: 6, id: "crypt-crypt-beneath-the-crypt", title: "The Crypt Beneath the Crypt", what: "A bigger chamber of the Crypt: older tombs on the half-step and a black channel below; mind the water." },
    Def { pack: Pack::Crypt, great: true, index: 7, id: "crypt-catacomb-shelves", title: "The Catacomb Shelves", what: "A bigger chamber of the Crypt: shelf-racks stepping down the diagonal into a pooled undercroft; slide fight." },
    Def { pack: Pack::Crypt, great: true, index: 8, id: "crypt-bone-cathedral", title: "The Bone Cathedral", what: "A bigger chamber of the Crypt: a tomb rank, a broken reliquary screen and a high altar at the crossing." },
    Def { pack: Pack::Cavern, great: false, index: 1, id: "cavern-stalactite-garden", title: "The Stalactite Garden", what: "A chamber of the Mines: lone stalagmites with a shallow pool at the foot, a dancing arena of single blocks and short hops." },
    Def { pack: Pack::Cavern, great: false, index: 2, id: "cavern-sunken-gallery", title: "The Sunken Gallery", what: "A chamber of the Mines: an S-shaped pool threading the middle, knights shoot across it and circle either bank between rubble stubs." },
    Def { pack: Pack::Cavern, great: false, index: 3, id: "cavern-crystal-nook", title: "The Crystal Nook", what: "A chamber of the Mines: a tight cluster of crystal spars by the wall with a black pool at its foot; the rest is open floor for the fight." },
    Def { pack: Pack::Cavern, great: true, index: 0, id: "cavern-boulder-field", title: "The Boulder Field", what: "A bigger chamber of the Mines: an open arena where two boulder clumps flank a reed pool; long sight lines, hard cover to sprint between." },
    Def { pack: Pack::Cavern, great: true, index: 1, id: "cavern-collapsed-gallery", title: "The Collapsed Gallery", what: "A bigger chamber of the Mines: fallen gallery columns lie in broken rows; water gathers at their feet and knights fight down the aisles." },
    Def { pack: Pack::Cavern, great: true, index: 2, id: "cavern-underground-lake", title: "The Underground Lake", what: "A bigger chamber of the Mines: a broad lake mid-room with two stone islets; arrows fly over the water while the party skirts the shore." },
    Def { pack: Pack::Cavern, great: true, index: 3, id: "cavern-drowned-hall", title: "The Drowned Hall", what: "A bigger chamber of the Mines: two moat bands cut the great hall into strips; boulder pairs stand on the dry causeways between." },
    Def { pack: Pack::Cavern, great: true, index: 4, id: "cavern-rockslide", title: "The Rockslide", what: "A bigger chamber of the Mines: one shoulder of the hall has come down in a diagonal scree with a pool caught at its foot; fight on the flat." },
    Def { pack: Pack::Cavern, great: true, index: 5, id: "cavern-meander", title: "The Meander", what: "A bigger chamber of the Mines: an old stream bed winds corner to corner; boulders bank its turns and both shores stay in touch round the ends." },
    Def { pack: Pack::Hellforge, great: false, index: 0, id: "hellforge-anvil-nook", title: "The Anvil Nook", what: "A chamber of Dragon Keep: an anvil and a slag block in a small yard; a tight two-cover scrap." },
    Def { pack: Pack::Hellforge, great: false, index: 1, id: "hellforge-slag-heap", title: "The Slag Heap", what: "A chamber of Dragon Keep: one cooling heap in the open; kite round it, dodge across the yard." },
    Def { pack: Pack::Hellforge, great: false, index: 3, id: "hellforge-forge-ring", title: "The Forge Ring", what: "A chamber of Dragon Keep: a quenching ring open at both ends; step in, step out, never stay." },
    Def { pack: Pack::Hellforge, great: true, index: 0, id: "hellforge-furnace-floor", title: "The Furnace Floor", what: "A bigger chamber of Dragon Keep: furnace mouths banked along the wall and a lava moat below; heat between covers." },
    Def { pack: Pack::Hellforge, great: true, index: 1, id: "hellforge-lava-channel", title: "The Lava Channel", what: "A bigger chamber of Dragon Keep: a wide channel with three stone bridges and an anvil on the far side." },
    Def { pack: Pack::Hellforge, great: true, index: 2, id: "hellforge-slag-yard", title: "The Slag Yard", what: "A bigger chamber of Dragon Keep: cooling heaps and a shifting lava pond; no lane stays safe." },
    Def { pack: Pack::Hellforge, great: true, index: 3, id: "hellforge-great-anvil", title: "The Great Anvil", what: "A bigger chamber of Dragon Keep: the master's anvil at the centre and work blocks to the sides; circle the stem." },
    Def { pack: Pack::Hellforge, great: true, index: 4, id: "hellforge-lava-river-halls", title: "The Lava River Halls", what: "A bigger chamber of Dragon Keep: a broad river with twin fords and open ends; hold the bridges." },
    Def { pack: Pack::Hellforge, great: true, index: 5, id: "hellforge-furnace-hall", title: "The Furnace Hall", what: "A bigger chamber of Dragon Keep: furnace banks along the wall, a moat before them, an anvil on the floor." },
    Def { pack: Pack::Hellforge, great: true, index: 6, id: "hellforge-great-forge-ring", title: "The Great Forge Ring", what: "A bigger chamber of Dragon Keep: a ring of anvils about a lava throat; the centre is the high ground." },
    Def { pack: Pack::Hellforge, great: true, index: 7, id: "hellforge-slag-flats", title: "The Slag Flats", what: "A bigger chamber of Dragon Keep: ridges of slag stepping across the flats and ponds between; a running fight." },
    Def { pack: Pack::Archive, great: false, index: 0, id: "archive-reading-alcove", title: "The Reading Alcove", what: "A chamber of the Archive: shelf-butts and a reading desk in a small hall; corner the loremasters." },
    Def { pack: Pack::Archive, great: false, index: 1, id: "archive-flooded-stacks", title: "The Flooded Stacks", what: "A chamber of the Archive: water in the corner seeping under the shelves; dry ground fights." },
    Def { pack: Pack::Archive, great: false, index: 2, id: "archive-map-table", title: "The Map Table", what: "A chamber of the Archive: one wide table with a clerk's stool; an open duel around it." },
    Def { pack: Pack::Archive, great: true, index: 0, id: "archive-cartographers-hall", title: "The Cartographer's Hall", what: "A bigger chamber of the Archive: the great table under its lamp, stools at the corners; plan the route." },
    Def { pack: Pack::Archive, great: true, index: 1, id: "archive-sunken-reading-room", title: "The Sunken Reading Room", what: "A bigger chamber of the Archive: a flooded floor and desks hauled to the walls; shoot across the water." },
    Def { pack: Pack::Archive, great: true, index: 2, id: "archive-drowned-stacks", title: "The Drowned Stacks", what: "A bigger chamber of the Archive: water down one wall, shelves marching away from it; slip-gaps everywhere." },
    Def { pack: Pack::Archive, great: true, index: 3, id: "archive-flooded-aisle", title: "The Flooded Aisle", what: "A bigger chamber of the Archive: water down the middle of the hall, shelves banked to either side." },
    Def { pack: Pack::Archive, great: true, index: 4, id: "archive-index-hall", title: "The Index Hall", what: "A bigger chamber of the Archive: catalogue desks in clumps, water in the corners; many small fights." },
    Def { pack: Pack::Archive, great: true, index: 5, id: "archive-scriptorium-deep", title: "The Scriptorium Deep", what: "A bigger chamber of the Archive: copyists' desks, an ink channel across, and water in the corner." },
    Def { pack: Pack::Fungal, great: false, index: 1, id: "fungal-spore-sump", title: "The Spore Sump", what: "A chamber of the Fungal Deep: a sunken sump of spores by the wall with stumps scattered on the dry ground; approach from either side." },
    Def { pack: Pack::Fungal, great: false, index: 2, id: "fungal-root-web", title: "The Root Web", what: "A chamber of the Fungal Deep: braided roots in short runs with wide gaps between them; room to slip a whole party through at speed." },
    Def { pack: Pack::Fungal, great: false, index: 3, id: "fungal-toadstool-nook", title: "The Toadstool Nook", what: "A chamber of the Fungal Deep: two giant toadstools crowd a corner over a spore slick; the open floor beside them is where the fight goes." },
    Def { pack: Pack::Fungal, great: true, index: 0, id: "fungal-spore-flats", title: "The Spore Flats", what: "A bigger chamber of the Fungal Deep: open flats pocked with sumps and stumps; a running fight with little cover and much dodging." },
    Def { pack: Pack::Fungal, great: true, index: 1, id: "fungal-ring-garden", title: "The Ring Garden", what: "A bigger chamber of the Fungal Deep: mushroom rings dotted round the chamber, spore pools between them; weave the rings or hold the open rim." },
    Def { pack: Pack::Fungal, great: true, index: 2, id: "fungal-bloom-meadow", title: "The Bloom Meadow", what: "A bigger chamber of the Fungal Deep: blooming mushrooms in five-petal clusters across a meadow of spore puddles; a bright, open shooting gallery." },
    Def { pack: Pack::Unknown, great: false, index: 0, id: "unknown-antechamber-of-the-spiral", title: "The Antechamber of the Spiral", what: "A chamber of the Unknown: outer pillars of a spiral stand detached, its centre gone; a calm ring before the descent begins." },
    Def { pack: Pack::Unknown, great: false, index: 1, id: "unknown-void-puddle", title: "The Void Puddle", what: "A chamber of the Unknown: a sliver of the void sits mid-floor; two shards lean against the wall. Cross the line of fire or go around." },
    Def { pack: Pack::Unknown, great: false, index: 2, id: "unknown-tilted-slab", title: "The Tilted Slab", what: "A chamber of the Unknown: one great slab tips into a void sink; the floor slopes to a fight under its shadow." },
    Def { pack: Pack::Unknown, great: false, index: 3, id: "unknown-hollow-twin", title: "The Hollow Twin", what: "A chamber of the Unknown: two hollow columns hum off-axis, void seeping at their feet; nothing here is where you left it." },
    Def { pack: Pack::Unknown, great: true, index: 0, id: "unknown-descent-of-nine", title: "The Descent of Nine", what: "A bigger chamber of the Unknown: pillar pairs walk in a slow spiral toward the middle; every ring is two tiles apart so the maze never closes." },
    Def { pack: Pack::Unknown, great: true, index: 1, id: "unknown-void-moat", title: "The Void Moat", what: "A bigger chamber of the Unknown: an island floats in a void moat, reached by stepping stones; the far bank circles all the way round." },
    Def { pack: Pack::Unknown, great: true, index: 2, id: "unknown-geometry-of-the-deep", title: "The Geometry of the Deep", what: "A bigger chamber of the Unknown: shards of some older room — a colonnade, a ring, a moat — meet and refuse to agree." },
    Def { pack: Pack::Unknown, great: true, index: 3, id: "unknown-thin-orbit", title: "The Thin Orbit", what: "A bigger chamber of the Unknown: sparse single pillars circle a void bead; open space rules, cover is a rumour you duck behind once." },
    Def { pack: Pack::Unknown, great: true, index: 4, id: "unknown-regolith-field", title: "The Regolith Field", what: "A bigger chamber of the Unknown: angular shards lie where they fell in twin ranks; the void pools in the seam between the ranks." },
];

/// The chambers: a set for each template of `rooms::*` the pilot left.
pub fn chambers() -> Vec<Item> {
    DEFS.iter()
        .map(|d| Item {
            source: if d.great {
                Source::Great { pack: d.pack, index: d.index }
            } else {
                Source::Chamber { pack: d.pack, small: d.index, doors: [true; 4] }
            },
            dressing: Dressing {
                id: d.id,
                title: d.title,
                setting: Setting::Dungeon(d.pack),
                what: d.what,
                notes: TORCHES,
                ..Dressing::BASE
            },
        })
        .collect()
}

const fn table(rect: Rect, top: Kind, height: f32) -> (Rect, Prop) {
    (rect, Prop::Table { top, height })
}

const fn case(rect: Rect, body: Kind, face: Kind, height: f32) -> (Rect, Prop) {
    (rect, Prop::Case { body, face, height })
}

const fn column(rect: Rect, kind: Kind, height: f32) -> (Rect, Prop) {
    (rect, Prop::Column { kind, height, lamp: false })
}

/// The side rooms below ground: the Sanctuary, the vault, the King's
/// forge-hall, the six nooks, and the rooms of the Undercroft that the game
/// draws by hand (the company's home, Dame Fortune's hall, the Training Yard,
/// the Trophy Hall, Maud's tavern).
pub fn side_rooms() -> Vec<Item> {
    SIDE_ROOMS.to_vec()
}

const SIDE_ROOMS: [Item; 14] = [
        Item {
            source: Source::Sanctuary,
            dressing: Dressing {
                id: "sanctuary",
                title: "The Sanctuary",
                setting: Setting::Dungeon(Pack::Crypt),
                what: "A Sanctuary, the safe room at the top of a deeper floor: open flags and four doors, the reforge altar (Tobbin's travelling anvil on a plinth, candles either side) in the middle, a privy booth in the north-west corner, a field banner down the east side.",
                props: &[((10, 5, 4, 4), Prop::Deck(Kind::Stone))],
                notes: &[
                    "Painted on open floor, not blocks: the altar's wishing dais at the middle (12,7) with its arcane ring and the scroll, a timber privy booth in the 2x2 corner from the north-west (2,2), the field banner on a pole, braziers in the corners.",
                ],
                ..Dressing::BASE
            },
        },
        Item {
            source: Source::Vault,
            dressing: Dressing {
                id: "vault",
                title: "The Vault",
                setting: Setting::Dungeon(Pack::Crypt),
                what: "A vault behind a cracked wall: a small treasure room with a stone pillar at each corner, one doorway in the south wall, a banded chest in its middle with gold heaped round it.",
                props: &[
                    column((4, 3, 1, 1), Kind::Stone, 34.0),
                    column((19, 3, 1, 1), Kind::Stone, 34.0),
                    column((4, 10, 1, 1), Kind::Stone, 34.0),
                    column((19, 10, 1, 1), Kind::Stone, 34.0),
                ],
                notes: &["Painted on open floor, not blocks: the chest at the middle (12,7), eight heaps of gold round it, braziers in the corners."],
                ..Dressing::BASE
            },
        },
        Item {
            source: Source::Forge,
            dressing: Dressing {
                id: "forge-hall",
                title: "The Forge-Hall",
                setting: Setting::Dungeon(Pack::Hellforge),
                what: "The King's forge-hall below a held floor: a great furnace in the middle, two anvils either side below it, one door in the south wall, the plate before its mouth where it is paid for and relit.",
                props: &[
                    ((10, 3, 4, 4), Prop::Furnace { lit: true }),
                    ((7, 8, 1, 1), Prop::Anvil),
                    ((16, 8, 1, 1), Prop::Anvil),
                ],
                notes: &["Painted on open floor, not blocks: the smith's tools, sparks, the plate before the furnace."],
                ..Dressing::BASE
            },
        },
        // ----------------------------------------- the Undercroft, the company's home ----
        Item {
            source: Source::Place(RoomKind::Home),
            dressing: Dressing {
                id: "undercroft",
                title: "The Undercroft",
                setting: Setting::Indoor,
                what: "The Undercroft, the company's home below the world: flagged floor, a runner up to the Winding Stair's parapet in the middle, four stations in the corners (Tobbin's forge north-west, Wren's map table north-east, Blaise's hearth south-west, the armour rack south-east), the chapel screen down the west wall, Wren's bounty board on posts.",
                props: &[
                    ((2, 1, 4, 2), Prop::Furnace { lit: true }),
                    table((18, 1, 4, 2), Kind::Plaster, 11.0),
                    ((2, 11, 4, 2), Prop::Cauldron),
                    case((18, 11, 4, 2), Kind::Wood, Kind::Metal, 22.0),
                    case((1, 5, 1, 4), Kind::Wood, Kind::Cloth, 26.0),
                    ((10, 5, 4, 4), Prop::Plinth { kind: Kind::Stone, height: 8.0 }),
                    case((15, 9, 3, 1), Kind::Wood, Kind::Plaster, 20.0),
                    ((11, 6, 2, 2), Prop::Plate(Kind::Stone)),
                ],
                notes: &["Painted on open floor, not blocks: a runner from the foot of the room up to the stair's mouth (11,8), banners, the purchase plates before each station, the cellar stair up to the Delve's gate in the north wall."],
                ..Dressing::BASE
            },
        },
        Item {
            source: Source::Place(RoomKind::Fortune),
            dressing: Dressing {
                id: "fortune-hall",
                title: "Dame Fortune's Hall",
                setting: Setting::Indoor,
                what: "Dame Fortune's hall: her great wheel on its timber A-frame against the north wall with red drapes either side, a stone pedestal for the Herald's coffer in the north-west, two benches for the audience in the south, a round medallion inlaid in the floor before the wheel; the doorway home is in the west wall.",
                props: &[
                    ((9, 1, 6, 6), Prop::Wheel),
                    ((3, 3, 2, 2), Prop::Plinth { kind: Kind::Stone, height: 12.0 }),
                    table((3, 10, 7, 2), Kind::Wood, 9.0),
                    table((14, 10, 7, 2), Kind::Wood, 9.0),
                ],
                notes: &["Painted on open floor, not blocks: the medallion before the wheel, Fortune beside the wheel at (7.6,7.2), the lever (11,8), the plates, loot boxes on the coffer."],
                ..Dressing::BASE
            },
        },
        Item {
            source: Source::Place(RoomKind::Yard),
            dressing: Dressing {
                id: "training-yard",
                title: "The Training Yard",
                setting: Setting::Hall,
                what: "The Training Yard, a straw-strewn practice hall north of the Undercroft: three quintains to strike down the middle, archery butts and blade racks on the north wall, Grubbins' stall under a striped awning in the south-east, one door in the south wall.",
                props: &[((17, 9, 6, 2), Prop::Stall { north: false, goods: Kind::Cloth })],
                notes: &["Painted on open floor, not blocks: straw on the flags, three quintains at (6.5,6.5) (12,6.5) (17.5,6.5), archery butts and blade racks on the north wall, torches at the way back down."],
                ..Dressing::BASE
            },
        },
        Item {
            source: Source::Place(RoomKind::Trophies),
            dressing: Dressing {
                id: "trophy-hall",
                title: "The Trophy Hall",
                setting: Setting::Hall,
                what: "The Trophy Hall, a long hall south of the Undercroft: plinths of pale stone along its walls with a brass plate on each, a bigger one for the dragon in the middle, a red runner from the door to the Grail's dais; each guardian the realm has felled stands on its plinth as a bronze statuette, a dust sheet over the empty ones.",
                props: &[((1, 1, 22, 12), Prop::Plinth { kind: Kind::Stone, height: 10.0 })],
                notes: &["Painted on open floor, not blocks: the realm's red runner, Sir Kay by the door (8.6,2.6), the Grail's candles; the west wall's doorway is rubble until the wing is dug."],
                ..Dressing::BASE
            },
        },
        Item {
            source: Source::Place(RoomKind::Tavern),
            dressing: Dressing {
                id: "tavern",
                title: "The Siege Perilous",
                setting: Setting::Hall,
                what: "The Siege Perilous, Maud's tavern: a plank floor, the bar along the north wall with taps and shelves of bottles, two tables with stools, the rumour board on posts in the north-east, Sir Dinadan's low stage with footlights in the east, the gilt high-backed Siege Perilous on its stone step in the south-west corner (a chair on open floor, not a block); lanterns on the walls.",
                props: &[
                    table((3, 3, 12, 1), Kind::Metal, 11.0),
                    table((5, 7, 3, 2), Kind::Cloth, 10.0),
                    table((12, 7, 3, 2), Kind::Cloth, 10.0),
                    case((17, 2, 3, 1), Kind::Wood, Kind::Plaster, 20.0),
                    ((17, 8, 5, 1), Prop::Stage),
                ],
                fixtures: &[(Fix::Lamp, 72.0), (Fix::Lamp, 200.0), (Fix::Lamp, 304.0)],
                notes: &["Painted on open floor, not blocks: stools, barrels in the corner, the taps' plates (4,4) (8,4) (12,4), the chair itself, a faint light about the Siege, the west wall's rubble doorway."],
                ..Dressing::BASE
            },
        },
        nook(RoomKind::Kitchen, "nook-kitchen", "Grubb's Kitchen", Pack::Cavern,
            "Grubb's Kitchen, a nook of the Mines and the Fungal Deep: a goblin cook's stew pot on its fire in the middle, a shelf of crocks in the north-west, sacks and barrels in the north-east, two trestle tables laid for supper in the south, hams on hooks.",
            &[
                ((11, 3, 2, 2), Prop::Cauldron),
                case((2, 1, 5, 2), Kind::Wood, Kind::Cloth, 22.0),
                ((18, 1, 4, 2), Prop::Crate),
                table((4, 9, 5, 1), Kind::Cloth, 9.0),
                table((15, 9, 5, 1), Kind::Cloth, 9.0),
            ],
            &[]),
        nook(RoomKind::Reading, "nook-reading", "The Quiet Room", Pack::Archive,
            "The Quiet Room, a nook of the Archive and the Unknown: shelves of spines along the north, eight lecterns each with an open book in two rows of four (three are mimics), a long rug down the middle, candles at the sides, a SHHH sign on the north wall.",
            &[
                ((3, 3, 18, 8), Prop::Table { top: Kind::Cloth, height: 12.0 }),
                case((2, 1, 7, 1), Kind::Wood, Kind::Cloth, 28.0),
                case((15, 1, 7, 1), Kind::Wood, Kind::Cloth, 28.0),
                ((10, 2, 4, 11), Prop::Deck(Kind::Rug)),
            ],
            &[(Fix::Lamp, 56.0), (Fix::Lamp, 328.0)]),
        nook(RoomKind::Bellows, "nook-bellows", "The Great Bellows", Pack::Hellforge,
            "The Great Bellows, a nook of the Mines and Dragon Keep: a cold forge with a hood and flue on the north wall between two great leather bellows, a quench trough, an anvil and a heap of coal in the south.",
            &[
                ((9, 1, 6, 3), Prop::Furnace { lit: false }),
                table((4, 2, 3, 2), Kind::Cloth, 12.0),
                table((17, 2, 3, 2), Kind::Cloth, 12.0),
            ],
            &[]),
        nook(RoomKind::Ossuary, "nook-ossuary", "The Bone Dominoes", Pack::Crypt,
            "The Bone Dominoes, a nook of the Crypt and the Unknown: a winding line of skulls and long bones on end round a bone coffer with iron bands and a lock, two skull stacks flanking it, gaps chalked as a knight's outline.",
            &[
                case((11, 6, 2, 2), Kind::Bone, Kind::Metal, 14.0),
                column((7, 6, 1, 2), Kind::Bone, 14.0),
                column((16, 6, 1, 2), Kind::Bone, 14.0),
            ],
            &[]),
        nook(RoomKind::Still, "nook-still", "Mother Mort's Still", Pack::Hellforge,
            "Mother Mort's Still, a nook of Dragon Keep and the Fungal Deep: a cauldron on its fire in the middle with three labelled jars (bone, wax, ore), crock shelves in the north-west and north-east, herbs drying on a line.",
            &[
                ((11, 3, 2, 2), Prop::Cauldron),
                case((2, 1, 5, 2), Kind::Wood, Kind::Cloth, 22.0),
                case((17, 1, 5, 2), Kind::Wood, Kind::Cloth, 22.0),
            ],
            &[]),
        nook(RoomKind::Loft, "nook-loft", "The Bell Loft", Pack::Crypt,
            "The Bell Loft, a nook of the Crypt and the Archive: four bells hung from a beam on two posts above the north, two pews in the south, candles at the sides, a ghost choirmaster's room.",
            &[
                column((5, 3, 1, 1), Kind::Metal, 18.0),
                column((9, 3, 1, 1), Kind::Metal, 18.0),
                column((14, 3, 1, 1), Kind::Metal, 18.0),
                column((18, 3, 1, 1), Kind::Metal, 18.0),
                table((4, 9, 5, 1), Kind::Wood, 9.0),
                table((15, 9, 5, 1), Kind::Wood, 9.0),
            ],
            &[]),
];

/// A nook: its room, its pack's stone, and what stands in it.
const fn nook(
    kind: RoomKind,
    id: &'static str,
    title: &'static str,
    pack: Pack,
    what: &'static str,
    props: &'static [(Rect, Prop)],
    fixtures: &'static [super::blockout::Fixture],
) -> Item {
    Item {
        source: Source::Nook(kind),
        dressing: Dressing {
            id,
            title,
            setting: Setting::Dungeon(pack),
            what,
            props,
            fixtures,
            notes: &["A nook hangs off a fight room by one doorway, in the west wall here; torches flank it. Its furniture is the room's whole business."],
            ..Dressing::BASE
        },
    }
}
