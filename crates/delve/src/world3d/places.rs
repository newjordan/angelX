//! The second wave, the kingdom's places: every room of `world::ROOMS` the
//! pilot did not take (`pilot`), blocked out from the game's own layouts and
//! dressed with what the painters draw on their blocked tiles.
//!
//! Each place is the room of the home floor as a run raises it
//! (`world::raise_world`). A dressing names a region and what stands on the
//! blocked tiles in it, so the game's clearing of a doorway's fan, which
//! leaves a plot's footprint part open, never leaves a tile unnamed
//! (`blockout::clipped`). A plot of the realm's buildings is shown as it
//! starts: a staked site (`Prop::Site`).
//!
//! What the painters draw on open ground a knight walks through (bunting,
//! flowers, signs, lamps, the crowd) is left to the paint and told in each
//! item's notes.

use super::blockout::{Dressing, Edge, Field, Fix, Kind, Leaf, Prop, Rect, Setting};
use super::blockout::Side;
use super::pilot::{Item, Source};
use crate::rules::RoomKind;

/// An item of the kingdom: a room, its id, its in-game name, its setting,
/// what it is; then the dressing's fields over the empty one.
macro_rules! place {
    ($id:literal, $kind:ident, $title:literal, $setting:expr, $what:literal; $($field:ident: $val:expr),* $(,)?) => {
        Item {
            source: Source::Place(RoomKind::$kind),
            dressing: Dressing { id: $id, title: $title, setting: $setting, what: $what, $($field: $val,)* ..Dressing::BASE },
        }
    };
}

const fn open(edge: Edge, field: Field) -> Setting {
    Setting::Outdoor { edge, field }
}

const OAK: Setting = open(Edge::Wood(Leaf::Oak), Field::Grass);
const PINE: Setting = open(Edge::Wood(Leaf::Pine), Field::Grass);
const COPPER: Setting = open(Edge::Wood(Leaf::Copper), Field::Grass);

/// The cross of roads most of the places keep: north to south and east to
/// west (`world::roads`).
const CROSS: &[Rect] = &[(11, 0, 2, 14), (0, 6, 24, 2)];

/// A staked plot of the realm's buildings (`countryside::PLOTS`).
const fn site(rect: Rect) -> (Rect, Prop) {
    (rect, Prop::Site)
}

const fn cottage(rect: Rect, walls: Kind, roof: Kind, windows: u8) -> (Rect, Prop) {
    (rect, Prop::Cottage { walls, roof, windows })
}

const fn table(rect: Rect, top: Kind, height: f32) -> (Rect, Prop) {
    (rect, Prop::Table { top, height })
}

const fn case(rect: Rect, face: Kind, height: f32) -> (Rect, Prop) {
    (rect, Prop::Case { body: Kind::Wood, face, height })
}

const fn column(rect: Rect, kind: Kind, height: f32, lamp: bool) -> (Rect, Prop) {
    (rect, Prop::Column { kind, height, lamp })
}

const fn tower(rect: Rect, walls: Kind, cap: Kind, height: f32) -> (Rect, Prop) {
    (rect, Prop::Tower { walls, cap, height, sails: false, lamp: false })
}

pub const PLACES: [Item; 58] = [
    // ------------------------------------------------ the surface, west ----
    place!("gate", Gate, "Gate Courtyard", open(Edge::Wood(Leaf::Oak), Field::Grass),
        "The Delve's gate courtyard: a stone arch over the stair down to the Undercroft in the north-west, a well, the notice post by the road, doors to the mine-head, the stables and the lists.";
        roads: &[(11, 0, 2, 14), (0, 6, 24, 2), (5, 5, 2, 1)],
        props: &[
            // The gate's arch hall, and the two piers either side of the stair, torches burning.
            cottage((4, 1, 5, 3), Kind::Stone, Kind::Slate, 0),
            column((4, 4, 1, 1), Kind::Stone, 26.0, true),
            column((7, 4, 2, 1), Kind::Stone, 26.0, true),
            ((17, 3, 2, 2), Prop::Well),
            column((15, 10, 1, 1), Kind::Wood, 18.0, false),
        ],
        notes: &[
            "Painted on open ground, not blocks: the stair down under the arch (5,4), the College's school and tower facades over their door plate (20,4), the coach stop (19,10).",
        ]),
    place!("lists", Lists, "Jousting Lists", OAK,
        "The jousting lists: raked sand inside a fence with a striped tilt down the middle, tiered stands along the north, a pavilion at each end, the realm's red and the rival's blue.";
        roads: &[(9, 0, 6, 3), (11, 3, 2, 4), (0, 6, 3, 2), (1, 8, 12, 2), (11, 8, 2, 6)],
        props: &[
            ((3, 1, 6, 2), Prop::Stand(Kind::Wood, Side::North)),
            ((15, 1, 6, 2), Prop::Stand(Kind::Wood, Side::North)),
            ((21, 1, 2, 2), Prop::Tent(Kind::Cloth)),
            ((1, 10, 2, 2), Prop::Tent(Kind::Cloth)),
            ((3, 7, 18, 1), Prop::Rail),
        ],
        notes: &["Painted on open ground, not blocks: the sand of the lists, a canopy of red and gold over the stands, the pigeon post's plate (19,10)."]),
    place!("mine-head", MineHead, "Mine-Head", open(Edge::Rock, Field::Scree),
        "The mine-head under the mountain: the adit's timbered mouth with rails and the shaft going down, the fallen gate-hall of Caer Dwfn to the west, spoil heaps and a cart to the east.";
        roads: &[(9, 0, 6, 3), (11, 4, 2, 10), (0, 6, 13, 2), (12, 9, 9, 2)],
        props: &[
            ((1, 1, 10, 6), Prop::Ruin),
            ((16, 1, 6, 4), Prop::Boulder),
        ],
        notes: &["Painted on open ground, not blocks: the adit's mouth and its rails, the shaft (11,3), the ore cart."]),
    place!("kings-hall", KingsHall, "King's Hall", Setting::Indoor,
        "King Brannoc's hall inside: a dais of pale stone with his seat and banner, the war table with the map of the forges, the ledger on its lectern, four pillars, the cold Great Forge, the great door in the south wall.";
        props: &[
            ((10, 1, 4, 2), Prop::Dais),
            ((19, 3, 4, 6), Prop::Furnace { lit: false }),
            table((4, 8, 4, 2), Kind::Plaster, 10.0),
            table((3, 3, 1, 1), Kind::Plaster, 12.0),
            column((7, 3, 1, 1), Kind::Stone, 34.0, false),
            column((16, 3, 1, 1), Kind::Stone, 34.0, false),
            column((7, 7, 1, 1), Kind::Stone, 34.0, false),
            column((16, 7, 1, 1), Kind::Stone, 34.0, false),
            ((11, 4, 2, 8), Prop::Deck(Kind::Rug)),
        ],
        fixtures: &[(Fix::Lamp, 56.0), (Fix::Lamp, 328.0)],
        notes: &["Painted on open floor, not blocks: the runner from the great door to the dais, braziers along the walls."]),
    place!("land-office", LandOffice, "The Court of Deeds", Setting::Indoor,
        "The realm's Court of Deeds: the great notice board on a timber frame along the north wall, the ledger wall of its own down the west, the clerk's desk, a banded strongbox; the door is in the south wall.";
        props: &[
            case((2, 1, 20, 3), Kind::Plaster, 28.0),
            case((1, 4, 6, 9), Kind::Cloth, 30.0),
            table((10, 5, 4, 1), Kind::Cloth, 11.0),
            ((20, 5, 2, 2), Prop::Crate),
        ],
        notes: &["Painted on open floor, not blocks: the runner from the door to the desk, banners on the east wall, lamps by the door."]),
    place!("bathhouse", Bathhouse, "The Royal Soak", Setting::Indoor,
        "The town's bathhouse: a copper over its fire for the hot water in the north-east corner, a towel bench along the south, four tubs on a tiled dais, washing lines of towels across the south.";
        props: &[
            ((21, 1, 2, 3), Prop::Cauldron),
            table((2, 11, 5, 1), Kind::Cloth, 8.0),
        ],
        notes: &["Painted on open floor, not blocks: four tubs on a tiled dais at (5,3.6) (9.5,3.6) (14.5,3.6) (19,3.6), washing lines, the price on a board by the door."]),
    // ------------------------------------------------------ the ranch ----
    place!("ranch", Ranch, "The Horse Ranch", open(Edge::Wood(Leaf::Oak), Field::Earth),
        "The horse ranch beside the stables: the barn, Helios's stall with the stud book in front of it, the hitching rail where the draft horse is groomed, hay, and the war horses' paddock; fenced, in a tree line.";
        props: &[
            ((2, 1, 8, 4), Prop::Barn { doors: &[] }),
            cottage((12, 1, 5, 3), Kind::Wood, Kind::Thatch, 0),
            table((16, 4, 1, 1), Kind::Cloth, 12.0),
            ((3, 7, 5, 2), Prop::Pen { floor: Kind::Ground, drying: false }),
            ((10, 10, 2, 1), Prop::Rick),
            ((14, 7, 9, 5), Prop::Pen { floor: Kind::Grass, drying: false }),
        ],
        notes: &["Painted on open ground, not blocks: the gates in the fence (19,1) and (1,12), the stud book (13,4), trodden mud."]),
    place!("pasture", Pasture, "The Flower Fields", open(Edge::Wood(Leaf::Oak), Field::Grass),
        "The flower fields beyond the ranch: open grass in flower under a low sun, an old oak in the north-west, a trough in the south-east, a fence with its gate home in the south.";
        props: &[
            ((3, 2, 3, 3), Prop::Tree),
            ((18, 10, 2, 1), Prop::Trough { depth: 9.0, height: 8.0 }),
        ],
        notes: &["Painted on open ground, not blocks: flowers over the grass, the gate home (6,12)."]),
    // ------------------------------------------------------ the College ----
    place!("college-grounds", CollegeGrounds, "College Courtyard", Setting::Courtyard { field: Field::Grass },
        "The College of the Labyrinth's courtyard in stone walls: the school's long front in the north, the wizard's tall tower in the east, a fountain in the middle, flagged paths to the doors, grass between.";
        paved: &[(8, 1, 8, 12), (1, 10, 22, 3)],
        props: &[
            cottage((5, 1, 7, 2), Kind::Stone, Kind::Slate, 3),
            tower((17, 2, 5, 5), Kind::Stone, Kind::Slate, 40.0),
            ((10, 6, 3, 2), Prop::Fountain),
        ],
        notes: &["Painted on open ground, not blocks: shrubs by the paths, two outdoor tables (3,11.5) (18,11.5), the door plates into the halls."]),
    place!("college-hall", CollegeHall, "Great Hall", Setting::Hall,
        "The College's great hall: two long refectory tables with benches, two stone pillars, a rug down the middle, a hearth and two banners on the north wall.";
        props: &[
            table((4, 8, 5, 2), Kind::Cloth, 10.0),
            table((15, 8, 5, 2), Kind::Cloth, 10.0),
            column((7, 5, 1, 1), Kind::Stone, 32.0, false),
            column((16, 5, 1, 1), Kind::Stone, 32.0, false),
            ((10, 6, 4, 6), Prop::Deck(Kind::Rug)),
        ],
        fixtures: &[(Fix::Hearth, 170.0), (Fix::Banner, 104.0), (Fix::Banner, 270.0)],
        notes: &["Painted on open floor, not blocks: candles and books on the tables."]),
    place!("college-class", CollegeClass, "Chalk Classroom", Setting::Hall,
        "The chalk classroom: the master's slate board along the north with its chalked lesson, five long student desks with books, a shelf and a cupboard in the south-west and south-east corners, banners on the north wall.";
        props: &[
            ((6, 2, 12, 2), Prop::Case { body: Kind::Wood, face: Kind::Slate, height: 26.0 }),
            table((4, 6, 4, 1), Kind::Plaster, 10.0),
            table((10, 6, 4, 1), Kind::Plaster, 10.0),
            table((16, 6, 4, 1), Kind::Plaster, 10.0),
            table((4, 9, 4, 1), Kind::Plaster, 10.0),
            table((16, 9, 4, 1), Kind::Plaster, 10.0),
        ],
        fixtures: &[(Fix::Banner, 40.0), (Fix::Banner, 336.0)],
        notes: &["Painted on open floor, not blocks: candles on the desks (58,69) (325,69), a shelf and a door at the west and east walls."]),
    place!("college-library", CollegeLibrary, "Floating Library", Setting::Hall,
        "The floating library: tall bookcases along the north and in the two side bays, two reading tables with lamps in the south, a long rug in the middle, books drifting in the air.";
        props: &[
            case((2, 2, 6, 2), Kind::Cloth, 30.0),
            case((10, 2, 4, 2), Kind::Cloth, 30.0),
            case((16, 2, 6, 2), Kind::Cloth, 30.0),
            case((3, 7, 3, 2), Kind::Cloth, 24.0),
            case((18, 7, 3, 2), Kind::Cloth, 24.0),
            table((6, 9, 2, 1), Kind::Plaster, 10.0),
            table((16, 9, 2, 1), Kind::Plaster, 10.0),
            ((7, 5, 10, 4), Prop::Deck(Kind::Rug)),
        ],
        notes: &["Painted on open floor, not blocks: three books floating over the rug (130,190,250 by 100), rolling ladders (139,245), reading lamps."]),
    place!("college-lab", CollegeLab, "Alchemy Lab", Setting::Hall,
        "The alchemy lab: shelves of jars along the north, three iron cauldrons on their fires in the middle, a long work table in the south, green pools of spill on the flags.";
        props: &[
            ((3, 2, 5, 2), Prop::Case { body: Kind::Wood, face: Kind::Water, height: 28.0 }),
            ((16, 2, 5, 2), Prop::Case { body: Kind::Wood, face: Kind::Water, height: 28.0 }),
            ((4, 6, 3, 2), Prop::Cauldron),
            ((10, 6, 3, 2), Prop::Cauldron),
            ((17, 6, 3, 2), Prop::Cauldron),
            table((9, 9, 6, 1), Kind::Plaster, 11.0),
        ],
        notes: &["Painted on open floor, not blocks: pools of green round the cauldrons, labelled jars on the table (176,173)."]),
    place!("tower-entry", TowerEntry, "Tower Entry", Setting::Hall,
        "The foot of Bartosz's tower: round flagged floor, an odd bookshelf that slides aside in the west, a map table in the north, a reading table in the south-east, a stair up in the east.";
        props: &[
            case((3, 4, 4, 1), Kind::Cloth, 24.0),
            table((10, 3, 4, 1), Kind::Plaster, 11.0),
            table((15, 9, 4, 1), Kind::Plaster, 10.0),
            ((10, 6, 4, 6), Prop::Deck(Kind::Rug)),
        ],
        fixtures: &[(Fix::Banner, 76.0), (Fix::Window, 270.0)],
        notes: &["Painted on open floor, not blocks: a low table and a candle at (49,155), the stair up (19,3)."]),
    place!("tower-stairs", TowerStairs, "Spiral Landing", Setting::Hall,
        "The tower's spiral landing: the stairwell's stone ring round dark water in the middle, a bookshelf in the north-west, a table in the south-east, stairs up to the north-east and down to the south-west.";
        props: &[
            ((10, 5, 3, 3), Prop::Shaft),
            case((3, 3, 4, 2), Kind::Cloth, 26.0),
            table((16, 9, 4, 1), Kind::Plaster, 10.0),
        ],
        fixtures: &[(Fix::Window, 154.0), (Fix::Banner, 260.0)],
        notes: &["Painted on open floor, not blocks: candles by the shaft, the stair plates (19,3) and (4,12)."]),
    place!("tower-stars", TowerStars, "Star Observatory", Setting::Hall,
        "The star observatory at the top of the tower: a round star dial on the floor, the star-map table in the north, a bookshelf in the north-west, a brass telescope on its legs in the south-east, three lit windows on the north wall.";
        props: &[
            table((10, 3, 4, 2), Kind::Plaster, 11.0),
            case((3, 3, 3, 2), Kind::Cloth, 24.0),
            ((16, 8, 4, 2), Prop::Telescope),
        ],
        fixtures: &[(Fix::Window, 70.0), (Fix::Window, 188.0), (Fix::Window, 270.0)],
        notes: &["Painted on open floor, not blocks: the dial of stars over the floor, the stairs (19,3) and (4,12)."]),
    place!("wizard-study", WizardStudy, "Bartosz's Study", Setting::Hall,
        "The wizard Bartosz's study: bookcases floor to ceiling either side of the north, his big map table between them, a lectern with the bestiary in the south-west, a rug, a hearth and two windows glowing.";
        props: &[
            case((3, 2, 5, 2), Kind::Cloth, 30.0),
            case((16, 2, 5, 2), Kind::Cloth, 30.0),
            table((10, 3, 4, 2), Kind::Plaster, 11.0),
            table((5, 8, 2, 1), Kind::Cloth, 10.0),
            ((8, 5, 7, 7), Prop::Deck(Kind::Rug)),
        ],
        fixtures: &[(Fix::Window, 149.0), (Fix::Window, 242.0)],
        notes: &["Painted on open floor, not blocks: a hearth with a fire (16,5.5), jars on the shelves, three pinned creature sketches, a familiar."]),
    // ------------------------------------------------- the Colosseum ----
    place!("colosseum-hall", ColosseumHall, "The Colosseum", Setting::Hall,
        "The Colosseum's entrance hall: an arcade of slate stone along the north wall with two arched gates (one for each arena), a board on two posts before each gate, a worn runner from the great door in the south.";
        props: &[
            ((1, 1, 22, 1), Prop::Case { body: Kind::Slate, face: Kind::Stone, height: 32.0 }),
            ((1, 2, 3, 1), Prop::Case { body: Kind::Slate, face: Kind::Stone, height: 32.0 }),
            ((6, 2, 12, 1), Prop::Case { body: Kind::Slate, face: Kind::Stone, height: 32.0 }),
            ((20, 2, 3, 1), Prop::Case { body: Kind::Slate, face: Kind::Stone, height: 32.0 }),
            ((2, 7, 7, 1), Prop::Case { body: Kind::Wood, face: Kind::Plaster, height: 16.0 }),
            ((15, 7, 7, 1), Prop::Case { body: Kind::Wood, face: Kind::Plaster, height: 16.0 }),
        ],
        fixtures: &[(Fix::Lamp, 112.0), (Fix::Lamp, 272.0)],
        notes: &["Painted on open floor, not blocks: banners on the gates, champions' niches in the arcade, the runner, the gate plates (4,2) (18,2)."]),
    place!("keg-yard", KegYard, "Powder Keg Yard", Setting::Courtyard { field: Field::Sand },
        "The Powder Keg yard: a courtyard of worn flags with posts and bales, a low parapet round it, tiered stands of slate north, east (stepping back) and south, the knights' bench in the west.";
        paved: &[(6, 2, 12, 10)],
        props: &[
            ((6, 1, 17, 1), Prop::Stand(Kind::Slate, Side::North)),
            ((18, 2, 5, 10), Prop::Stand(Kind::Slate, Side::East)),
            ((6, 12, 17, 1), Prop::Stand(Kind::Slate, Side::South)),
        ],
        notes: &["Painted on open floor, not blocks: the posts and bales in the courtyard, the knights' bench and a rack of spare kegs along the west wall, a stone dais under the start mark, the gate home (2,12)."]),
    place!("gander-hall", GanderHall, "Gander Joust Hall", Setting::Hall,
        "The Gander Joust hall: a long hall of coursed slate with arches along its back wall and the folk in a gallery above; the game is played side-on (ledges, a tar pit and riders are the game's), and the gate home is low in the west wall.";
        notes: &["The blockout is the hall's shell only: the game draws it side-on, so the ledges, the tar and the riders are not blocks. Pillars stand at both ends, the gallery rail above."]),
    // ---------------------------------------------------- the countryside ----
    place!("forest", Forest, "Firefly Grove", open(Edge::Wood(Leaf::Pine), Field::Grass),
        "The firefly grove, a woodland clearing: old dark oaks and yews standing alone in a meadow with a lantern pad and rune circles on the ground, roads crossing.";
        roads: &[(11, 0, 2, 14), (0, 6, 24, 2), (3, 10, 16, 1)],
        props: &[((1, 1, 22, 12), Prop::Tree)],
        notes: &["Painted on open ground, not blocks: eight fireflies, three rune lights, the lantern."]),
    place!("fairground", Fairground, "Fool's Green", OAK,
        "Fool's Green, the festival green: two stalls with striped awnings over boarded counters in the north, bunting strung over the green, the roads crossing.";
        roads: CROSS,
        props: &[
            ((3, 1, 5, 2), Prop::Stall { north: false, goods: Kind::Cloth }),
            ((17, 1, 4, 2), Prop::Stall { north: false, goods: Kind::Cloth }),
        ],
        notes: &["Painted on open ground, not blocks: bunting at row 3, the games' rings, the fair wagon's plate (21,10)."]),
    place!("windmill", Windmill, "Wonky Windmill", OAK,
        "The wonky windmill: a stone mill under a thatch cap with four sails, a boarded granary under thatch, standing wheat, and a staked plot for the goose-proof bakery.";
        roads: &[(11, 0, 2, 10), (0, 6, 24, 2)],
        props: &[
            ((6, 2, 4, 3), Prop::Tower { walls: Kind::Stone, cap: Kind::Thatch, height: 30.0, sails: true, lamp: false }),
            ((17, 2, 4, 1), Prop::Barn { doors: &[] }),
            site((3, 10, 5, 2)),
        ],
        notes: &["Painted on open ground, not blocks: standing wheat (3..6, 4..6), the goat lift's plate (3,9), the kite's plate (14,4)."]),
    place!("ruins", Ruins, "The Old Watch", OAK,
        "The old watch: broken walls of the ruined watchtower with moss on their tops, rubble at their feet and a doorway into the dark in each; a staked plot for the wayfarer's beacon.";
        roads: CROSS,
        props: &[
            ((4, 2, 2, 1), Prop::Ruin),
            ((6, 2, 4, 3), Prop::Ruin),
            ((18, 3, 2, 2), Prop::Ruin),
            ((6, 10, 2, 2), Prop::Ruin),
            site((17, 10, 4, 2)),
        ],
        notes: &["Painted on open ground, not blocks: rubble heaps at the feet of the walls."]),
    place!("riverbank", Riverbank, "Washbucket Quay", OAK,
        "Washbucket Quay: a river along the whole north with a boarded landing stage on piles in the middle of it, wash lines and drying socks along the quay, a ferry and a barge landing in the south.";
        roads: &[(11, 0, 2, 7), (0, 6, 24, 2)],
        props: &[((9, 1, 6, 3), Prop::Deck(Kind::Wood))],
        notes: &["Painted on open ground, not blocks: wash lines with drying clothes between posts (3..9, 4), the ferry plate (7,11) and the barge plate (3,11)."]),
    place!("meadow", Meadow, "Wayfarer Meadow", OAK,
        "Wayfarer Meadow: open flowering grass with the road running through, a boarded coach stop and a bench in the south-east, a staked plot for the Bent Spoon Inn in the north-west.";
        roads: &[(11, 0, 2, 10), (7, 9, 14, 2)],
        props: &[site((3, 2, 6, 4))],
        notes: &["Painted on open ground, not blocks: flowers, the coach platform (19,10), a boulder, a hay bale."]),
    place!("downs", Downs, "Wheyward Downs", OAK,
        "Wheyward Downs, goat pasture: open grass with roads crossing and two long hay ricks in the north, each with its board.";
        roads: &[(11, 0, 2, 12), (11, 6, 13, 2), (3, 5, 9, 2)],
        props: &[
            ((4, 1, 4, 2), Prop::Rick),
            ((16, 1, 4, 2), Prop::Rick),
        ],
        notes: &["Painted on open ground, not blocks: goats, a signpost."]),
    place!("cliffs", Cliffs, "Gullible Cliffs", open(Edge::Rock, Field::Scree),
        "Gullible Cliffs, a railed headland: grass and grey scree to a lip railed with timber, the sea far below at the south-west and south-east, a staked plot for the counterweight goat lift in the north.";
        roads: &[(11, 0, 2, 14), (3, 6, 21, 2), (16, 7, 2, 3)],
        props: &[
            ((1, 10, 8, 1), Prop::Rail),
            ((15, 10, 8, 1), Prop::Rail),
            site((6, 1, 3, 3)),
        ],
        notes: &["Painted on open ground, not blocks: the cliff face dropping in shadow to the sea, the lift's plate (3,9)."]),
    // --------------------------------------------------- the eastern road ----
    place!("causeway", Causeway, "Toll Troll Causeway", OAK,
        "The toll troll causeway: a broad paved causeway of setts between shallow water north and south, dressed stone kerbs along the water, the troll's sandwich-invoice toll booth.";
        paved: &[(1, 5, 22, 4)],
        notes: &["Painted on open ground, not blocks: the dressed kerbs along the water (rows 3 and 10), signs, the troll."]),
    place!("priory", Priory, "Puddingstone Priory", OAK,
        "Puddingstone Priory: two stone ranges under slate with glowing windows, bushes between the doors, a boarded post for the pigeon flight, a staked plot for the pigeon post.";
        props: &[
            cottage((3, 2, 6, 3), Kind::Stone, Kind::Slate, 3),
            cottage((4, 11, 5, 2), Kind::Stone, Kind::Slate, 3),
            site((16, 2, 5, 3)),
        ],
        notes: &["Painted on open ground, not blocks: clipped bushes (12,4) (14,4), the flight plate (19,10)."]),
    // ----------------------------------------------------- the north ridge ----
    place!("crag", Crag, "Candlewick Crag", open(Edge::Rock, Field::Snow),
        "Candlewick Crag, a snowy ridge: four snow-capped outcrops of dark rock, a rope course strung between posts across the middle, roads crossing in snow.";
        roads: CROSS,
        props: &[
            ((3, 1, 5, 2), Prop::Outcrop { cap: Some(Kind::Plaster) }),
            ((16, 1, 5, 2), Prop::Outcrop { cap: Some(Kind::Plaster) }),
            ((3, 11, 6, 2), Prop::Outcrop { cap: Some(Kind::Plaster) }),
            ((16, 11, 5, 2), Prop::Outcrop { cap: Some(Kind::Plaster) }),
        ],
        notes: &["The ground is snow (plaster-coloured in the capture). Painted on open ground, not blocks: the rope course (rows 4 and 10)."]),
    place!("rookery", Rookery, "Rookery Pass", open(Edge::Rock, Field::Scree),
        "Rookery Pass: the Royal Bird Office and its pigeon-holed loft, two slate-roofed stone houses on the ridge, scree and grass between, a flight plate in the south-west.";
        roads: CROSS,
        props: &[
            cottage((3, 2, 6, 3), Kind::Stone, Kind::Slate, 0),
            cottage((16, 2, 4, 3), Kind::Stone, Kind::Slate, 0),
        ],
        notes: &["Painted on open ground, not blocks: pigeon-holes on the house fronts, the flight plate (4,10), signs."]),
    place!("observatory", Observatory, "Cuckoo Observatory", open(Edge::Rock, Field::Snow),
        "Cuckoo Observatory: a stone tower under a slate dome with its telescope out, a timber weather shed in the south-west, a staked plot for the ropeway, snow underfoot.";
        roads: CROSS,
        props: &[
            tower((6, 1, 4, 3), Kind::Stone, Kind::Slate, 34.0),
            cottage((3, 11, 5, 2), Kind::Wood, Kind::Slate, 1),
            site((16, 2, 5, 3)),
        ],
        notes: &["The ground is snow (plaster-coloured in the capture). Painted on open ground, not blocks: forecast markers, the ropeway plate (19,10)."]),
    // ------------------------------------------------------ the coast ----
    place!("strand", Strand, "Soggy Bottom Strand", open(Edge::Wood(Leaf::Oak), Field::Sand),
        "Soggy Bottom Strand: a sandy beach footpath between the sea in the north-west and north-east and in the south, driftwood and a beached rowing boat by the water.";
        roads: CROSS,
        notes: &["Painted on open ground, not blocks: a sign and shells on the sand, driftwood and a beached boat (19,5)."]),
    place!("saltworks", Saltworks, "Saltspite Works", open(Edge::Wood(Leaf::Oak), Field::Sand),
        "Saltspite Works: two salt pans of water in the north with stone kerbs and salt heaped on the dry side, a staked plot for the crab-approved harbour in the south-west, sand underfoot.";
        roads: CROSS,
        props: &[site((3, 10, 5, 2))],
        notes: &["Painted on open ground, not blocks: salt heaps, kerbs round the pans (rows 2..4), the barge plate (19,10)."]),
    place!("lighthouse", Lighthouse, "The Bickering Beacon", open(Edge::Wood(Leaf::Oak), Field::Sand),
        "The bickering beacon: the keeper's cottage and the beacon tower beside it, its lamp lit, on a sandy peninsula with the sea south-west and south-east.";
        roads: CROSS,
        props: &[
            cottage((4, 2, 3, 3), Kind::Stone, Kind::Slate, 1),
            ((7, 2, 2, 3), Prop::Tower { walls: Kind::Stone, cap: Kind::Slate, height: 40.0, sails: false, lamp: true }),
        ],
        notes: &["Painted on open ground, not blocks: a harbour sign, rope."]),
    // ----------------------------------------------------- the woodland ----
    place!("timberyard", Timberyard, "Splinter's Timberyard", COPPER,
        "Splinter's Timberyard: four stacks of cut logs, ringed ends toward the viewer, in the corners of a copper wood, a grass lane with full-width logs to jump between.";
        roads: CROSS,
        props: &[
            ((3, 1, 5, 2), Prop::Logs),
            ((16, 1, 5, 2), Prop::Logs),
            ((3, 11, 5, 2), Prop::Logs),
            ((16, 11, 5, 2), Prop::Logs),
        ],
        notes: &["Painted on open ground, not blocks: five logs lying across the lane (numbered jumps), cart ruts, LOGS signs."]),
    place!("hedge-maze", HedgeMaze, "The Hedge of Grievances", open(Edge::Hedge, Field::Grass),
        "The Hedge of Grievances, a maze: clipped hedge walls of five different lengths standing in a clearing inside a hedge, an acorn hidden in it.";
        roads: CROSS,
        props: &[
            ((6, 2, 1, 4), Prop::Hedge),
            ((6, 8, 1, 4), Prop::Hedge),
            ((12, 4, 1, 3), Prop::Hedge),
            ((12, 9, 1, 3), Prop::Hedge),
            ((15, 8, 6, 1), Prop::Hedge),
        ],
        notes: &["Painted on open ground, not blocks: the acorn (18,11)."]),
    place!("bothy", Bothy, "Badger's Bothy", COPPER,
        "Badger's Bothy: the badger's stone bothy under thatch with a chimney and smoke, a barrel by the door, in a copper wood; a staked plot for the badger's board-and-breakfast.";
        roads: CROSS,
        props: &[
            cottage((3, 2, 5, 3), Kind::Stone, Kind::Thatch, 1),
            site((16, 2, 5, 3)),
        ],
        notes: &["Painted on open ground, not blocks: the stew pot, a BADGER sign, the wagon plate (19,10)."]),
    // -------------------------------------------------------- the wetland ----
    place!("millpond", Millpond, "The Royal Dog-Paddle", open(Edge::Wood(Leaf::Oak), Field::Marsh),
        "The Royal Dog-Paddle, a millpond: two ponds of open water in the middle joined by a dry stone bridge with a walled strip between them, mud and grass round them.";
        roads: CROSS,
        notes: &["Painted on open ground, not blocks: the stone bridge between the ponds (11..13, 4..9), a sign."]),
    place!("sluice", Sluice, "The Sluice of Excuses", open(Edge::Wood(Leaf::Oak), Field::Marsh),
        "The Sluice of Excuses: four pools of open water in the four corners, three valves on stone footings that open the sluice, mud and grass between.";
        roads: CROSS,
        notes: &["Painted on open ground, not blocks: three valves on stone footings, a sign."]),
    place!("sunken-abbey", SunkenAbbey, "The Sunken Abbey", open(Edge::Wood(Leaf::Oak), Field::Marsh),
        "The Sunken Abbey: the choir's four columns standing up out of the flooded north-west, pools in both southern corners, a staked plot for the punt house, mud and grass.";
        roads: CROSS,
        props: &[
            column((3, 4, 1, 1), Kind::Stone, 30.0, false),
            column((5, 4, 1, 1), Kind::Stone, 30.0, false),
            column((7, 4, 1, 1), Kind::Stone, 30.0, false),
            column((8, 4, 1, 1), Kind::Stone, 30.0, false),
            site((16, 2, 5, 3)),
        ],
        notes: &["Painted on open ground, not blocks: arches between the columns, the boat plate (19,10)."]),
    // --------------------------------------------------------- the aerie ----
    place!("belfry", Belfry, "The Bashful Belfry", PINE,
        "The Bashful Belfry: two stone bell towers under slate, bells hung in their open lanterns, a rope course on timber posts between them, in a pine wood.";
        roads: CROSS,
        props: &[
            tower((3, 2, 2, 3), Kind::Stone, Kind::Slate, 44.0),
            tower((19, 2, 2, 3), Kind::Stone, Kind::Slate, 44.0),
        ],
        notes: &["Painted on open ground, not blocks: the rope course on posts (6,18 by 4..10)."]),
    place!("aqueduct", Aqueduct, "Unnecessary Aqueduct", PINE,
        "The Unnecessary Aqueduct: stone piers in the north and south, a high channel of water carried on arches between the two northern piers (painted over the piers, above headroom), in a pine wood.";
        roads: CROSS,
        props: &[
            column((7, 2, 2, 3), Kind::Stone, 38.0, false),
            column((15, 2, 2, 3), Kind::Stone, 38.0, false),
            column((7, 10, 2, 2), Kind::Stone, 38.0, false),
            column((15, 10, 2, 2), Kind::Stone, 38.0, false),
        ],
        notes: &["Painted on open ground, not blocks: a sign."]),
    place!("aviary", Aviary, "Pigeon Parliament", OAK,
        "Pigeon Parliament: the pigeons' dovecote, boarded and pigeon-holed under a tiled roof, in the north-west; a staked plot for the air-mail monstrosity; the flight plate in the south-east.";
        roads: CROSS,
        props: &[
            cottage((3, 2, 5, 2), Kind::Wood, Kind::Roof, 0),
            site((16, 2, 5, 3)),
        ],
        notes: &["Painted on open ground, not blocks: pigeons, three perches (9,4) (13,9.5) (8,11.5), the flight plate (19,10)."]),
    // ------------------------------------------------------- Mummers' way ----
    place!("mummers", Mummers, "Mummers' Hollow", OAK,
        "Mummers' Hollow: the players' stage of boards with a curtained backdrop and footlights, a painted flat of a castle propped up in the north-east, a heap of costume trunks in the south-west, a broad half-pipe ramp.";
        roads: CROSS,
        props: &[
            ((3, 1, 6, 2), Prop::Stage),
            ((16, 1, 5, 2), Prop::Case { body: Kind::Wood, face: Kind::Stone, height: 28.0 }),
            ((4, 11, 4, 2), Prop::Crate),
        ],
        notes: &["Painted on open ground, not blocks: the half-pipe's U-ramp (rows 9..12), a NO HORSES ON RAMP sign."]),
    place!("laundry", Laundry, "The Royal Spin Cycle", OAK,
        "The Royal Spin Cycle, the king's laundry: four big coopered wash vats of soapy water with iron hoops, in the four quarters of the yard, a cross of road between them.";
        roads: CROSS,
        props: &[
            ((4, 2, 4, 3), Prop::Vat),
            ((16, 2, 4, 3), Prop::Vat),
            ((4, 9, 4, 3), Prop::Vat),
            ((16, 9, 4, 3), Prop::Vat),
        ],
        notes: &["Painted on open ground, not blocks: lines of the king's socks, soap suds."]),
    place!("caravan", Caravan, "The Caravan of Questionable Comfort", OAK,
        "The Caravan of Questionable Comfort: the inn on wheels, a boarded van under a canvas tilt, in the north-west; a heap of lost property (crates, barrels, sacks) in the south-west; a staked plot for the Wheely Inn.";
        roads: CROSS,
        props: &[
            ((3, 2, 5, 3), Prop::Van),
            ((3, 10, 5, 2), Prop::Crate),
            site((16, 2, 5, 3)),
        ],
        notes: &["Painted on open ground, not blocks: a boot, INN ON WHEELS and LOST PROPERTY signs, the wagon plate (19,10)."]),
    // ------------------------------------------------------ the racing way ----
    place!("hippodrome", Hippodrome, "The Rusty Hippodrome", OAK,
        "The Rusty Hippodrome: an oval of rutted dirt track round a grass infield, two timber grandstands in the north, the judges' box at the south of the infield, a staked plot for the fast coach in the south-east.";
        roads: CROSS,
        props: &[
            ((3, 2, 6, 2), Prop::Stand(Kind::Wood, Side::North)),
            ((15, 2, 6, 2), Prop::Stand(Kind::Wood, Side::North)),
            cottage((8, 9, 2, 2), Kind::Wood, Kind::Roof, 0),
            site((16, 11, 5, 2)),
        ],
        notes: &["Painted on open ground, not blocks: the oval track, the start line at the east end, the bookie's booth (17,4.5), race-day bunting, the coach gate (19,12)."]),
    place!("wheelwright", Wheelwright, "Square-Wheel Works", OAK,
        "Square-Wheel Works: the wheelwright's stone shop under tiles, a rack of square wheels in the north-east, a timber stack in the south and a bench with a wheel on a trestle in the south-east.";
        roads: CROSS,
        props: &[
            cottage((3, 2, 5, 3), Kind::Stone, Kind::Roof, 2),
            ((15, 2, 6, 2), Prop::Case { body: Kind::Wood, face: Kind::Trunk, height: 16.0 }),
            ((7, 10, 3, 2), Prop::Logs),
            table((15, 11, 3, 1), Kind::Wood, 9.0),
        ],
        notes: &["Painted on open ground, not blocks: signs, a mechanic at (5,9.5)."]),
    place!("toll-bridge", TollBridge, "The Toll Bridge to Nowhere", OAK,
        "The Toll Bridge to Nowhere: two toll houses (slate and stone in the north-west, thatch and timber in the north-east), the toll booth in the south-middle, a stack of crates and barrels, a rail along the road.";
        roads: CROSS,
        props: &[
            cottage((3, 2, 6, 3), Kind::Stone, Kind::Slate, 2),
            cottage((15, 2, 6, 3), Kind::Wood, Kind::Thatch, 2),
            cottage((8, 9, 2, 3), Kind::Stone, Kind::Roof, 0),
            ((14, 10, 2, 2), Prop::Crate),
            ((3, 11, 4, 1), Prop::Rail),
        ],
        notes: &["Painted on open ground, not blocks: a FREE FOOTPATH sign, a river that is scenery, a rail along row 7."]),
    // ----------------------------------------------------- the wind country ----
    place!("goose-green", GooseGreen, "Sir Honks-a-Lot Picnic Green", OAK,
        "Sir Honks-a-Lot Picnic Green: a striped picnic marquee in the north-west, a long hedge with its bench in the north-east, the goose's own thatched shed in the south, flowers along the middle.";
        roads: CROSS,
        props: &[
            ((3, 2, 5, 2), Prop::Tent(Kind::Cloth)),
            ((15, 2, 6, 2), Prop::Hedge),
            cottage((7, 10, 3, 2), Kind::Wood, Kind::Thatch, 0),
        ],
        notes: &["Painted on open ground, not blocks: flowers along row 3, signs, the goose and the sail."]),
    place!("kitewright", Kitewright, "The Knotty Kitewright", COPPER,
        "The Knotty Kitewright: the kite workshop under tiles in the north-west, a drying yard in the north-east with kite cloth pegged on a cord between two poles, hay baskets stacked in the south, a basket stand; two kites flying.";
        roads: CROSS,
        props: &[
            cottage((3, 2, 6, 3), Kind::Wood, Kind::Roof, 2),
            ((15, 2, 6, 2), Prop::Pen { floor: Kind::Grass, drying: true }),
            ((8, 10, 3, 2), Prop::Rick),
            ((15, 11, 3, 1), Prop::Crate),
        ],
        notes: &["Painted on open ground, not blocks: two kites up on strings, signs, the kite post's plate (19,10)."]),
    place!("signal-hill", SignalHill, "The Hill of Extremely Loud Flags", PINE,
        "The Hill of Extremely Loud Flags: the signal hut in slate and stone in the north-west, the flag store with flags on poles in the north-east, two rocky outcrops in the south, in a pine wood.";
        roads: CROSS,
        props: &[
            cottage((3, 2, 4, 2), Kind::Stone, Kind::Slate, 1),
            ((16, 2, 5, 2), Prop::Flags),
            ((7, 10, 3, 2), Prop::Outcrop { cap: None }),
            ((15, 10, 3, 2), Prop::Outcrop { cap: None }),
        ],
        notes: &["Painted on open ground, not blocks: signs."]),
    // ------------------------------------------------------ the bow country ----
    place!("longbow-green", LongbowGreen, "Longbow Green", OAK,
        "Longbow Green, the archers' green: a turf bank along the north with three straw butts on it, tall timber poles with a rope across them for the popinjay, the judge's tally booth under a striped awning in the south-east, quivers in barrels in the south-west.";
        props: &[
            ((4, 1, 16, 2), Prop::Butts),
            ((21, 9, 2, 2), Prop::Stall { north: false, goods: Kind::Plaster }),
            ((2, 11, 2, 1), Prop::Crate),
        ],
        notes: &["Painted on open ground, not blocks: the two poles (6,0..) (18,0..) with their rope, the chalked shooting line, distance pegs."]),
    place!("fletchery", Fletchery, "The Fletchery", OAK,
        "The Fletchery: the fletcher's open shed with a shingled roof on posts over a bench of feathers, a fenced goose pen in the north-east, an arrow-shaft drying rack in the south-west; a plate for the quiver coach.";
        roads: &[(0, 6, 21, 2), (11, 8, 2, 6), (19, 8, 2, 2)],
        props: &[
            ((3, 1, 6, 3), Prop::Shed),
            ((15, 2, 6, 2), Prop::Pen { floor: Kind::Ground, drying: false }),
            ((5, 10, 3, 2), Prop::Case { body: Kind::Wood, face: Kind::Crop, height: 18.0 }),
        ],
        notes: &["Painted on open ground, not blocks: feathers blown about the yard, geese, the coach plate (19,10)."]),
    place!("scarecrow", Scarecrow, "The Scarecrow's Field", OAK,
        "The Scarecrow's Field: a wheat field in four patches of stooks, rows of wheat in the north and south corners, the scarecrow on his post in the middle with crows about.";
        roads: &[(11, 6, 13, 2), (11, 8, 2, 6)],
        props: &[
            ((3, 2, 4, 2), Prop::Stooks),
            ((16, 2, 5, 2), Prop::Stooks),
            ((3, 10, 4, 2), Prop::Stooks),
            ((16, 10, 4, 2), Prop::Stooks),
        ],
        notes: &["Painted on open ground, not blocks: the scarecrow on his post (12,5), crows, furrows between the stooks."]),
    // ---------------------------------------------------------- the eyrie ----
    place!("eyrie", Eyrie, "The Griffin Eyrie", open(Edge::Rock, Field::Scree),
        "The Griffin Eyrie, up on the mountain: crags either side of the griffins' perch, a stone stump crowned with a nest, a take-off ring scuffed into the turf, the griffin master's brazier; the yard gate leads back down.";
        roads: &[(11, 6, 2, 8)],
        props: &[
            ((1, 1, 7, 3), Prop::Outcrop { cap: None }),
            ((16, 1, 7, 3), Prop::Outcrop { cap: None }),
            ((10, 1, 4, 2), Prop::Outcrop { cap: None }),
            ((2, 10, 4, 3), Prop::Outcrop { cap: None }),
        ],
        notes: &["Painted on open ground, not blocks: the perch with its nest (12,4), the take-off ring, feathers, the master's brazier."]),
];
