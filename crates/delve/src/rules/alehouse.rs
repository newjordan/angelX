//! The Sozzled Griffin: the great tavern where every delve begins. The
//! whole party wakes here together, in a hall bigger than a screen: a long
//! bar and its keeper, a hearth with a suspicious stew over it, tables and
//! benches, a stage with a lute player, kegs, a dartboard, a notice board,
//! an arm-wrestling table, two patrons asleep and a cat.
//!
//! **The pick.** Every knight of the company sits somewhere in the hall,
//! shown by the generic name of their calling (the Archer, the Mage, the
//! Healer ...) and a line of what they do, never a personal name. Walk up
//! and press E to be that knight: their arm, guard, hand, bombs and health,
//! and their class if they have one. A knight sits with one player at a
//! time: the first to the table keeps them, and a player may swap for
//! another until the party walks out. Under the identity start (`identity`)
//! nobody picks: a shape wishes itself into being, and the patrons only
//! sit.
//!
//! **The shops.** The keeper pours three drinks for the gold each knight
//! carries (the purse every knight is handed on arrival, [`PURSE`]), and
//! a fence in the corner sells one bazaar curio cheaply (`bazaar`). The
//! bazaar itself is the room through the east door.
//!
//! **The rest.** Each toy answers E with something funny and sometimes a
//! small real thing (a coin, a bomb, a second wind). All of it is fixed by
//! the seat's number and the clock, never by chance.
//!
//! The rooms are the world's own (`world`): `RoomKind` interiors on the
//! home floor in their own cells, joined by `Entrance` plates. The front
//! door goes back out to the kingdom at the Delve's gate (the visit's return,
//! `Run::returns_to_kingdom`), which is where a delve began before the
//! tavern.

use super::bazaar;
use super::classes::CLASSES;
use super::home::{BUY_HOLD, hold_bar};
use super::world::{Dest, Entrance};
use super::*;

pub const NAME: &str = "The Sozzled Griffin";

/// The room bounds in tiles: the great hall and its small east washroom.
pub const HALL: (usize, usize) = (44, 20);
/// The small washroom off the hall's east wall (col, row, w, h), including
/// its walls. The original hall and its fixtures keep their coordinates.
pub const BATHROOM: (i32, i32, i32, i32) = (35, 0, 9, 9);
/// An open two-tile doorway, walked through normally; not an entrance plate.
pub const BATHROOM_DOOR: (i32, i32, i32, i32) = (35, 5, 1, 2);
/// Solid washstand and the bucket on the floor. The jug sits on the stand;
/// the towel and mirror hang on the wall and need no floor collision.
pub const BATHROOM_BASIN: (i32, i32, i32, i32) = (38, 2, 2, 1);
pub const BATHROOM_BUCKET: (i32, i32, i32, i32) = (41, 5, 1, 1);
/// Wall mirror centre, in tile coordinates in RoomKind::Alehouse. This is
/// the art location; use MIRROR_STAND for the clear floor before the basin.
pub const MIRROR_AT: (f32, f32) = (39.0, 1.125);
/// Clear floor before the mirror (tiles). Multiply by TILE_UNITS for rules
/// positions. A location seam only, with no interaction or appearance logic.
pub const MIRROR_STAND: (f32, f32) = (39.0, 3.6);
/// The cells of the two rooms on the home floor, north east of the
/// Colosseum. No doorway joins them on the grid; the plates do.
pub const CELLS: [(RoomKind, (i32, i32)); 2] =
    [(RoomKind::Alehouse, (16, -12)), (RoomKind::Bazaar, (17, -12))];

pub fn is_room(kind: RoomKind) -> bool {
    matches!(kind, RoomKind::Alehouse | RoomKind::Bazaar | RoomKind::MusterTent)
}

/// Where patron `i` drinks: the company in the tavern, the recruits in the
/// muster tent at the mine-head (`muster_tent`).
pub fn patron_room(i: usize) -> RoomKind {
    if super::recruit_ults::recruit(PATRONS[i].kit) { RoomKind::MusterTent } else { RoomKind::Alehouse }
}

/// The gold in each knight's purse on arriving, to spend in the tavern and
/// the bazaar, and what they carry down after.
pub const PURSE: u32 = 150;
/// How near (arena units) a knight stands to use a thing.
pub const REACH: f32 = 3.0;
/// The home genie's lamp, and the open aisle where a knight rubs it (tiles).
pub const LAMP_AT: (f32, f32) = (10.5, 4.9);
pub const LAMP_STAND: (f32, f32) = (10.5, 5.7);
pub const LAMP_WISHES: u8 = 3;
/// How long a line stays on a knight's board, in ticks.
pub const SAID_TICKS: u64 = 5 * HZ as u64;

/// Where the party stands coming in from the start, and from the bazaar's
/// door (tiles).
pub const ARRIVAL: (f32, f32) = (17.5, 16.7);
pub const FROM_BAZAAR: (f32, f32) = (29.5, 16.7);

/// The hall's fixtures (col, row, w, h). The back wall is two rows of
/// timber; the hearth, bar, kegs, stage, tables and the arm-wrestling table
/// stand on the floor.
pub const BACK_WALL: (i32, i32, i32, i32) = (1, 1, 34, 2);
pub const HEARTH: (i32, i32, i32, i32) = (4, 1, 6, 3);
pub const BAR: (i32, i32, i32, i32) = (13, 4, 16, 2);
pub const KEGS: (i32, i32, i32, i32) = (30, 1, 4, 3);
pub const STAGE: (i32, i32, i32, i32) = (29, 7, 6, 3);
pub const WRESTLE_TABLE: (i32, i32, i32, i32) = (30, 13, 3, 2);
pub const CRATES: (i32, i32, i32, i32) = (1, 16, 3, 2);
/// Barrels stacked along the south wall.
pub const BARRELS: [(i32, i32, i32, i32); 2] = [(5, 17, 2, 2), (27, 17, 2, 2)];
/// The notice board on its posts, west.
pub const NOTICE: (i32, i32, i32, i32) = (1, 5, 2, 2);
pub const TABLES: [(i32, i32, i32, i32); 5] =
    [(8, 9, 4, 2), (15, 9, 4, 2), (22, 9, 4, 2), (11, 14, 4, 2), (18, 14, 4, 2)];
/// The doors: east to the bazaar, south to the kingdom.
pub const BAZAAR_DOOR: (i32, i32, i32, i32) = (34, 14, 1, 3);
pub const FRONT_DOOR: (i32, i32, i32, i32) = (17, 18, 2, 1);
/// The tavern from outside: the great oak inn by the well in the Delve's
/// courtyard, its door (tiles), and where one comes out before it.
pub const COURTYARD_DOOR: (i32, i32, i32, i32) = (20, 4, 2, 1);
pub const COURTYARD_FRONT: (f32, f32) = (21.0, 5.5);
/// The inn's walls in the courtyard (tiles), north of its door: no one
/// walks through them.
pub const INN: (i32, i32, i32, i32) = (18, 1, 5, 3);

/// The keeper behind the bar, the lute player on the stage, the fence in
/// the corner, the two wrestlers at their table (feet, tiles).
pub const KEEPER_AT: (f32, f32) = (20.5, 3.7);
pub const LUTE_AT: (f32, f32) = (32.0, 8.9);
pub const FENCE_AT: (f32, f32) = (3.4, 15.3);
pub const WRESTLERS_AT: [(f32, f32); 2] = [(29.2, 13.9), (33.4, 13.9)];

/// The keeper's three drinks, where a knight stands to ask for each.
pub const TAPS: [(f32, f32); 3] = [(16.5, 6.7), (20.5, 6.7), (24.5, 6.7)];

/// What the keeper pours: a name, what it does, its price in gold.
pub struct Pour {
    pub name: &'static str,
    pub does: &'static str,
    pub price: u32,
}

pub const POURS: [Pour; 3] = [
    Pour { name: "Draught of Mild Courage", does: "A POTION FOR YOUR HAND", price: 20 },
    Pour { name: "Bottle of Loud Opinions", does: "A BOMB FOR YOUR BELT", price: 14 },
    Pour { name: "Pint of Second Thoughts", does: "A SECOND WIND, ONE FALL", price: 45 },
];

/// One of the company, as the tavern sits them: the kit they wear, the
/// generic name of their calling, a line of what they do, and where they
/// sit (tiles): on the bench behind a table's long edge, so a knight walks
/// up to them from the aisle on its far side.
pub struct Patron {
    pub kit: &'static str,
    pub label: &'static str,
    pub pitch: &'static str,
    pub at: (f32, f32),
}

pub const PATRONS: [Patron; 64] = [
    Patron { kit: "waxwarden", label: "Exorcist", pitch: "Handgonne and kite shield. A gallows sun to shelter in, a rook's step out.", at: (2.75, 6.25) },
    Patron { kit: "orren", label: "Sexton", pitch: "Handgonne and kite shield. A siege bell rings; a procession takes the fire.", at: (4.25, 6.25) },
    Patron { kit: "morrow", label: "Thornkeeper", pitch: "Venom crossbow, a room of thorns, and mercy under glass.", at: (5.75, 6.25) },
    Patron { kit: "percival", label: "Archer", pitch: "A long bow, a quick roll and a potion for the road.", at: (8.7, 9.25) },
    Patron { kit: "lynette", label: "Shieldmaiden", pitch: "Crossbow and kite shield. Holds the line.", at: (10.1, 9.25) },
    Patron { kit: "gareth", label: "Gunner", pitch: "Handgonne and extra bombs. Loud, proud and close.", at: (11.5, 9.25) },
    Patron { kit: "galahad", label: "Swordsman", pitch: "A bright blade and a stout heart. Fights up close.", at: (15.7, 9.25) },
    Patron { kit: "ironbraid", label: "Berserker", pitch: "Two axes and a spin that cuts all round.", at: (17.1, 9.25) },
    Patron { kit: "peasant", label: "Peasant", pitch: "Bare feet and thrown stones. A scarecrow to hide behind.", at: (18.5, 9.25) },
    Patron { kit: "bard", label: "Bard", pitch: "Hold F for a cone of notes. R sings the near to sleep.", at: (22.7, 9.25) },
    Patron { kit: "scryglass", label: "Mage", pitch: "Weaves ten spells: 1 Ember, 2 Frost, 3 Storm, 4 casts the mix.", at: (24.1, 9.25) },
    Patron { kit: "loop", label: "Healer", pitch: "Censer, choir crumb and holy water. Keeps the party up.", at: (25.5, 9.25) },
    Patron { kit: "composer", label: "Knight", pitch: "A big blade, a kite shield and plenty of health.", at: (11.7, 14.25) },
    Patron { kit: "dispatcher", label: "Crossbowman", pitch: "Crossbow, a quick roll, three bombs and a potion.", at: (13.1, 14.25) },
    Patron { kit: "money", label: "Bombardier", pitch: "Handgonne, kite shield and a sack of bombs.", at: (14.5, 14.25) },
    Patron { kit: "competition", label: "Marksman", pitch: "Crossbow and a quick roll. Quick and precise.", at: (19.9, 14.25) },
    // The champions stand at the bar, a stride out from the counter so a
    // knight walks up between them and it, clear of the taps' stands.
    Patron { kit: "hammerer", label: "Hammerer", pitch: "A great maul. 1 thrown hammer, 2 slam, 3 brute force.", at: (13.8, 7.0) },
    Patron { kit: "butcher", label: "Butcher", pitch: "Cleaver and hook. 1 hook, 2 rot, 3 chain haul.", at: (18.6, 7.0) },
    Patron { kit: "alchemist", label: "Alchemist", pitch: "Lobbed fire. 1 foul air, 2 quickfire, 3 elixir.", at: (27.2, 7.0) },
    Patron { kit: "reaver", label: "Reaver", pitch: "Twin axes. 1 hurled axe, 2 leap, 3 bloodlust.", at: (9.6, 7.2) },
    Patron { kit: "lancer", label: "Lancer", pitch: "A war lance. 1 javelin, 2 vault, 3 brace.", at: (27.6, 12.2) },
    // The barrow's hero takes the far end of the same bench, a little
    // apart, as his kind do.
    Patron { kit: "gravecaller", label: "Gravecaller", pitch: "Aura; 1-3 dead, 4 Soul Rip; Space wall; G heel/hold golem; R Tombstone.", at: (21.5, 14.25) },
    Patron { kit: "farwatch", label: "Farwatch", pitch: "Long rifle. 1 sightline, 2 brass rain, 3 steady aim; R last light.", at: (5.2, 7.2) },
    Patron { kit: "galeweaver", label: "Galeweaver", pitch: "Swift bow. Hold 1 draw, 2 twin bind, 3 windrun; R rapid fire.", at: (24.1, 14.25) },
    Patron { kit: "cinderheart", label: "Cinderheart", pitch: "Fire wave, delayed pillar, cast-fed speed; R single-target flame.", at: (23.1, 7.2) },
    // The recruits drink in the muster tent at the mine-head, seven to a
    // table there (`muster_tent::seat`), not in the tavern.
    Patron { kit: "vesper", label: "Bellkeeper", pitch: "Ring Eclipse Toll through a crowd; Mercy Lantern wards and mends.", at: (7.25, 6.25) },
    Patron { kit: "orrin", label: "Notary", pitch: "A handgonne and falling stars. Cinder Tithe pays fire back as life.", at: (8.75, 6.25) },
    Patron { kit: "iona", label: "Lamplighter", pitch: "A bow behind a lantern ward. Last Light buys another breath.", at: (10.25, 6.25) },
    Patron { kit: "saffra", label: "Duelist", pitch: "Shatter noon with a handgonne; borrow a sheltered minute to recover.", at: (11.75, 6.25) },
    Patron { kit: "rill", label: "Courier", pitch: "Swift crossbow, thunder and a ward; recover in the storm's eye.", at: (17.75, 6.25) },
    Patron { kit: "brine", label: "Salvager", pitch: "Break the deck with Keelbreaker; seal your helm before the reply.", at: (19.25, 6.25) },
    Patron { kit: "nyra", label: "Stormrunner", pitch: "Foxstep across the firing lane and ring the storm behind you.", at: (20.75, 6.25) },
    Patron { kit: "tamsin", label: "Engineer", pitch: "A brass mortar holds the lane; funeral lanterns catch the return fire.", at: (22.25, 6.25) },
    Patron { kit: "elian", label: "Linebreaker", pitch: "A whalebone crossbow, a breach ring, and one borrowed minute.", at: (23.75, 6.25) },
    Patron { kit: "iole", label: "Entomancer", pitch: "Follow the moths: guided venom, and two lanterns to catch the reply.", at: (25.25, 6.25) },
    Patron { kit: "sorel", label: "Cantor", pitch: "Choirwire carries the shot; a siege bell holds the lane. No bombs.", at: (26.75, 6.25) },
    Patron { kit: "mariel", label: "Corsair", pitch: "Bank shots off the seawall. A crown that mends, a parasol that wards.", at: (2.75, 11.25) },
    Patron { kit: "veska", label: "Marshal", pitch: "Winterfang pierces and chills; a falling star marks where they gather.", at: (4.25, 11.25) },
    Patron { kit: "lys", label: "Sentinel", pitch: "Shield to the muzzle flash. A lantern splits each bolt into sparks.", at: (5.75, 11.25) },
    Patron { kit: "sabine", label: "Ranger", pitch: "Silver sight through the front rank, bottled thunder, then the aegis.", at: (7.25, 11.25) },
    Patron { kit: "maelis", label: "Warden", pitch: "Whiteout buys a step; the orrery fires every way. Roll, then the aegis.", at: (8.75, 11.25) },
    Patron { kit: "ivo", label: "Trickshot", pitch: "Bank the handgonne off the wall, roll from the reply, ring last call.", at: (10.25, 11.25) },
    Patron { kit: "nera", label: "Ferrier", pitch: "The fork jumps a bolt to the next in line; the salute clears the bank.", at: (11.75, 11.25) },
    Patron { kit: "bellweather", label: "Watchman", pitch: "A lantern finds your shots and slows pursuit; ring the bell, then roll.", at: (17.75, 11.25) },
    Patron { kit: "corvin", label: "Artillerist", pitch: "Line them up for the venom, then mark the crowd with the sundial mortar.", at: (19.25, 11.25) },
    Patron { kit: "mirelle", label: "Breacher", pitch: "Shoots through the front row. The spindle drinks; the bell buys a breath.", at: (20.75, 11.25) },
    Patron { kit: "coalglass", label: "Usher", pitch: "Bank a burning round past the door; pull the curtain when they close in.", at: (22.25, 11.25) },
    Patron { kit: "sablewire", label: "Signaler", pitch: "The switchboard carries venom through the crowd; a star marks the meet.", at: (23.75, 11.25) },
    Patron { kit: "oren", label: "Second", pitch: "A duel starts with distance. Push them off the line, then answer once.", at: (25.25, 11.25) },
    Patron { kit: "linet", label: "Concierge", pitch: "Bank frost off the wall, open the parasol as the room closes, then roll.", at: (26.75, 11.25) },
    Patron { kit: "fenella", label: "Apothecary", pitch: "Widowglass pierces the first patient; the lantern spreads it round.", at: (2.75, 15.25) },
    Patron { kit: "astrolabe", label: "Astronomer", pitch: "A little sun bends the shots and its satellite parries. Then the parasol.", at: (4.25, 15.25) },
    Patron { kit: "brasswrit", label: "Magistrate", pitch: "A ticket punches through one and storms the next. The gavel falls.", at: (5.75, 15.25) },
    Patron { kit: "nocturne", label: "Conductor", pitch: "Tickets, please. The fork pierces one and chains on; the signal follows.", at: (7.25, 15.25) },
    Patron { kit: "vespra", label: "Apiarist", pitch: "The comb bends your rounds and shares the sting. Do not tap the veil.", at: (8.75, 15.25) },
    Patron { kit: "bront", label: "Millwright", pitch: "Bank a frost bolt round the corner; the bell carries the chill outward.", at: (10.25, 15.25) },
    Patron { kit: "maribel", label: "Reveller", pitch: "The mortar plays the anthem. Aim at clear ground, then strike the match.", at: (11.75, 15.25) },
    Patron { kit: "nerissa", label: "Quartermaster", pitch: "The compass steadies your rhythm; the censer buys a heartbeat of ward.", at: (17.75, 15.25) },
    Patron { kit: "pearlrunner", label: "Pilot", pitch: "The siphon jumps four steps to open ground; the censer buys six breaths.", at: (19.25, 15.25) },
    Patron { kit: "cuckoosmith", label: "Gunsmith", pitch: "The caliper sends bolts through the queue; the cuckoo buys distance.", at: (20.75, 15.25) },
    Patron { kit: "mothporter", label: "Porter", pitch: "Three seconds of mothlight, then roll. The pearl sharpens the arrow.", at: (22.25, 15.25) },
    Patron { kit: "osric", label: "Stoker", pitch: "One unburnt match buys a crossing: the ferry jumps two steps.", at: (23.75, 15.25) },
    Patron { kit: "soundglass", label: "Diver", pitch: "Two seconds under the bell, then the tide returns. It wards, not heals.", at: (25.25, 15.25) },
    Patron { kit: "vesperhost", label: "Steward", pitch: "A kettle covers the aisle; the saucers catch three shots. Table for one?", at: (26.75, 15.25) },
];

/// The toys of the hall, each with a stand (tiles).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toy {
    Stew,
    Darts,
    Wrestle,
    Board,
    Hearth,
    Kegs,
    Lute,
    Snorer(u8),
    Cat,
    /// The bazaar's: the complaints box and the oracle in the fishbowl.
    Complaints,
    Fishbowl,
}

pub const TOYS: [(Toy, (f32, f32)); 8] = [
    (Toy::Stew, (6.9, 4.6)),
    (Toy::Darts, (2.7, 9.7)),
    (Toy::Wrestle, (31.5, 15.6)),
    (Toy::Board, (3.9, 6.0)),
    (Toy::Kegs, (31.5, 4.6)),
    (Toy::Lute, (32.0, 10.7)),
    (Toy::Snorer(0), (4.2, 12.6)),
    (Toy::Snorer(1), (31.4, 17.3)),
];
/// The hearth's own stand, beside the stew's.
pub const WARM_AT: (f32, f32) = (4.6, 4.6);

/// What E reaches in the hall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    Patron(usize),
    Lamp,
    /// A stock arm on the free rack, in catalogue order.
    Arm(usize),
    Pour(usize),
    Fence,
    /// A ware on a bazaar stall, by slot (`bazaar`).
    Ware(u8),
    Toy(Toy),
}

impl Spot {
    /// Pressed once, or held to buy or equip.
    pub fn held(self) -> bool {
        matches!(self, Spot::Lamp | Spot::Arm(_) | Spot::Pour(_) | Spot::Fence | Spot::Ware(_))
    }

    pub fn verb(self) -> &'static str {
        match self {
            Spot::Patron(_) => "BE THIS KNIGHT",
            Spot::Lamp => "RUB THE GENIE LAMP",
            Spot::Arm(_) => "EQUIP / SWAP (FREE)",
            Spot::Pour(_) | Spot::Fence | Spot::Ware(_) => "BUY",
            Spot::Toy(Toy::Complaints) => "COMPLAIN",
            Spot::Toy(Toy::Fishbowl) => "ASK THE FISH",
            Spot::Toy(Toy::Stew) => "TASTE THE STEW",
            Spot::Toy(Toy::Darts) => "THROW A DART",
            Spot::Toy(Toy::Wrestle) => "ARM-WRESTLE (10 GOLD)",
            Spot::Toy(Toy::Board) => "READ THE BOARD",
            Spot::Toy(Toy::Hearth) => "WARM YOUR HANDS",
            Spot::Toy(Toy::Kegs) => "TAP A KEG",
            Spot::Toy(Toy::Lute) => "ASK FOR A TUNE",
            Spot::Toy(Toy::Snorer(_)) => "POKE THE SLEEPER",
            Spot::Toy(Toy::Cat) => "PET THE CAT",
        }
    }
}

/// The cat's round of the hall, a loop of waypoints (tiles), and the ticks
/// it takes to saunter between two.
const CAT_ROUND: [(f32, f32); 8] = [
    (14.0, 7.2),
    (20.0, 7.2),
    (27.0, 7.2),
    (27.0, 12.6),
    (20.0, 12.6),
    (14.0, 12.6),
    (7.0, 12.6),
    (7.0, 7.4),
];
const CAT_LEG: u64 = 150;

/// Where the cat is on `tick` (tiles).
pub fn cat_at(tick: u64) -> (f32, f32) {
    let leg = (tick / CAT_LEG) as usize % CAT_ROUND.len();
    let k = (tick % CAT_LEG) as f32 / CAT_LEG as f32;
    let (a, b) = (CAT_ROUND[leg], CAT_ROUND[(leg + 1) % CAT_ROUND.len()]);
    (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k)
}

/// Six public basics, not a catalogue of champion or player-owned arms.
/// Blade, staff and maul are common stock with their own modest stats.
/// The three established ranged arms keep their existing cards and rarity;
/// no relic or champion/hero-private arm belongs on this wall.
pub const BASIC_ARMS: [&str; 6] = ["bow", "crossbow", "handgonne", "stock-blade", "stock-staff", "stock-maul"];

pub fn home_arms() -> &'static [Card] {
    static ARMS: std::sync::OnceLock<Vec<Card>> = std::sync::OnceLock::new();
    ARMS.get_or_init(|| {
        let book = cards::Book::builtin();
        BASIC_ARMS.iter().map(|id| book.cards.iter().find(|c| c.id == *id)
            .expect("builtin basic arm").clone()).collect()
    })
}

/// A compact wall south of the tables, leaving the central aisles open.
/// These are the floor positions in front of the six pegs, not obstacles.
pub fn arm_stand(slot: usize) -> (f32, f32) {
    (7.8 + slot as f32 * 1.5, 17.0)
}

pub const COMPANY_PAGE_SIZE: usize = 5;
pub const COMPANY_PAGES: usize = (PATRONS.len() - 3).div_ceil(COMPANY_PAGE_SIZE);

/// Every knight of the company sits in the hall at once, each at their own
/// seat; `page` is the old notice-board page a save may still carry.
pub fn patron_visible(_page: usize, i: usize) -> bool {
    i < PATRONS.len()
}

/// The company's recruits by name, champions aside (`heroes::KITS` have
/// their own stands).
pub fn company_roll_call(page: usize) -> String {
    PATRONS.iter().enumerate()
        .filter(|(i, p)| !heroes::KITS.contains(&p.kit) && patron_visible(page, *i))
        .map(|(_, p)| p.label)
        .collect::<Vec<_>>()
        .join(" / ")
}

/// The thing a knight at `(x, y)` (arena units) would reach with E on
/// `tick`: the nearest within [`REACH`].
pub fn spot_near(x: f32, y: f32, tick: u64) -> Option<Spot> {
    spot_near_on_page(x, y, tick, 0)
}

/// The same visibility contract used for drawing and authoritative actions.
pub fn spot_near_on_page(x: f32, y: f32, tick: u64, page: usize) -> Option<Spot> {
    spot_near_in(RoomKind::Alehouse, x, y, tick, page)
}

/// The thing a knight at `(x, y)` in a room of `kind` would reach: in the
/// muster tent only its recruits.
pub fn spot_near_in(kind: RoomKind, x: f32, y: f32, tick: u64, page: usize) -> Option<Spot> {
    let mut best: Option<(f32, Spot)> = None;
    let mut see = |spot: Spot, (sx, sy): (f32, f32)| {
        let d = (x - sx * TILE_UNITS).hypot(y - sy * TILE_UNITS);
        if d < REACH && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, spot));
        }
    };
    for (i, p) in PATRONS.iter().enumerate() {
        if patron_visible(page, i) && patron_room(i) == kind { see(Spot::Patron(i), p.at); }
    }
    if kind == RoomKind::MusterTent {
        return best.map(|(_, spot)| spot);
    }
    for (i, &at) in TAPS.iter().enumerate() {
        see(Spot::Pour(i), at);
    }
    for i in 0..home_arms().len() {
        see(Spot::Arm(i), arm_stand(i));
    }
    see(Spot::Lamp, LAMP_STAND);
    see(Spot::Fence, (FENCE_AT.0 + 1.4, FENCE_AT.1));
    for &(toy, at) in &TOYS {
        see(Spot::Toy(toy), at);
    }
    see(Spot::Toy(Toy::Hearth), WARM_AT);
    see(Spot::Toy(Toy::Cat), cat_at(tick));
    best.map(|(_, spot)| spot)
}

/// What a knight was last told, for their own board alone.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Said {
    pub at: u64,
    pub text: String,
    /// Good news (a coin, a pick) or plain.
    pub good: bool,
}

/// What a knight has tried of the hall's toys, each a few times at most.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tries {
    pub darts: u8,
    pub wrestles: u8,
    pub stew: u8,
    pub sleepers: u8,
    pub cat: u8,
    pub keg: u8,
}

/// The tavern's and the bazaar's moment-to-moment state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct House {
    /// Shared notice-board page; old visits begin on the first company page.
    #[serde(skip_serializing_if = "first_company_page")]
    pub company_page: usize,
    /// The seats handed their purse (a bit a seat).
    pub funded: u32,
    /// A run's claims belong to the player, not the patron they currently play.
    /// Changing kit, spending wishes, or returning home never refills the lamp.
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub lamp_claimed: std::collections::BTreeSet<u32>,
    pub tries: BTreeMap<u32, Tries>,
    pub said: BTreeMap<u32, Said>,
    /// The knights whose E is down, so a press counts once.
    pub held: Vec<u32>,
    /// An arm hold belongs to one rack slot; moving to another starts over.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub arm_holds: BTreeMap<u32, usize>,
    /// A lamp rub belongs to its stand, never a partly held drink or arm.
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub lamp_holds: std::collections::BTreeSet<u32>,
}

fn first_company_page(page: &usize) -> bool { *page == 0 }

impl House {
    pub fn valid(&self) -> bool {
        self.company_page < COMPANY_PAGES
            && self.lamp_holds.len() <= MAX_PLAYERS as usize
            && self.lamp_holds.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.arm_holds.len() <= MAX_PLAYERS as usize
            // Old checkpoints can hold one of the twenty aisle bays. The
            // next tick cancels it unless the knight is at a current peg.
            && self.arm_holds.iter().all(|(id, slot)| (1..=MAX_PLAYERS).contains(id) && *slot < 20)
            && self.lamp_claimed.len() <= MAX_PLAYERS as usize
            && self.lamp_claimed.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.tries.len() <= MAX_PLAYERS as usize
            && self.said.len() <= MAX_PLAYERS as usize
            && self.said.values().all(|s| s.text.len() <= 208)
            && self.held.len() <= MAX_PLAYERS as usize
    }

    pub fn is_default(&self) -> bool {
        *self == House::default()
    }
}

const fn door(
    id: &'static str,
    room: RoomKind,
    at: (i32, i32, i32, i32),
    to: RoomKind,
    front: (f32, f32),
    label: &'static str,
) -> Entrance {
    Entrance { id, room, at, to: Dest::Room(to, front), label, needs: None }
}

pub const DOORS: [Entrance; 5] = [
    door("alehouse-bazaar", RoomKind::Alehouse, BAZAAR_DOOR, RoomKind::Bazaar, bazaar::ARRIVAL, "THE BAZAAR"),
    // Out the front door: back to the kingdom at the Delve's gate (the visit's
    // return, `Run::returns_to_kingdom`); a party with no visit to return to
    // (an old save, or one that came in from the courtyard) comes out before
    // the tavern's own door in the Delve's courtyard.
    door("alehouse-out", RoomKind::Alehouse, FRONT_DOOR, RoomKind::Gate, COURTYARD_FRONT, "THE KINGDOM"),
    door("bazaar-alehouse", RoomKind::Bazaar, bazaar::TAVERN_DOOR, RoomKind::Alehouse, FROM_BAZAAR, "THE SOZZLED GRIFFIN"),
    door("bazaar-out", RoomKind::Bazaar, bazaar::GATE, RoomKind::Gate, world::COURTYARD_ARRIVAL, "THE KINGDOM"),
    // The way back in, any time: the green house by the courtyard's well.
    door("alehouse-in", RoomKind::Gate, COURTYARD_DOOR, RoomKind::Alehouse, ARRIVAL, "THE SOZZLED GRIFFIN"),
];

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for y in r..r + h {
        for x in c..c + w {
            room.set(x as usize, y as usize, tile);
        }
    }
}

/// The hall laid out: stone round the edge, open floor, the back wall,
/// the fixtures, and the doors' plates.
pub fn room(cell: (i32, i32)) -> Room {
    let mut r = Room::solid_rock(cell);
    r.kind = RoomKind::Alehouse;
    r.regrow(HALL.0, HALL.1);
    // Keep the old east wall intact except for the washroom doorway. Stone
    // below the annex is outside, not extra unadorned tavern floor.
    fill(&mut r, (BATHROOM.0, 0, BATHROOM.2, HALL.1 as i32), Tile::Wall);
    let (c, row, w, h) = BATHROOM;
    fill(&mut r, (c + 1, row + 1, w - 2, h - 2), Tile::Floor);
    fill(&mut r, BATHROOM_DOOR, Tile::Floor);
    for rect in [BATHROOM_BASIN, BATHROOM_BUCKET] {
        fill(&mut r, rect, Tile::Block);
    }
    for rect in [BACK_WALL, HEARTH, BAR, KEGS, STAGE, WRESTLE_TABLE, CRATES, NOTICE]
        .into_iter()
        .chain(BARRELS)
        .chain(TABLES)
    {
        fill(&mut r, rect, Tile::Block);
    }
    for e in DOORS.iter().filter(|e| e.room == RoomKind::Alehouse) {
        fill(&mut r, e.at, Tile::Stairs);
    }
    r
}

/// A short name for a patron's calling: the class if the kit has one.
pub fn label_of(kit: &str) -> Option<&'static str> {
    PATRONS.iter().find(|p| p.kit == kit).map(|p| p.label)
}

/// The bard announces a calling, not a house-dependent personal name.
/// Read the kit's real cards so new recruits automatically enter the songbook.
pub fn company_ballad(kit: &str, book: &Book) -> Option<String> {
    let knight = knights::knight(kit)?;
    let calling = label_of(kit).unwrap_or(knight.title);
    let arm = &book.get(knight.arm)?.name;
    let refrain = knight.spells.first()
        .and_then(|id| book.get(id))
        .map(|card| format!(" WHEN {} ANSWERS, THE RAFTERS SING BACK.", card.name))
        .unwrap_or_else(|| " THE CHORUS DUCKS BEFORE THE SECOND VERSE.".into());
    Some(format!("THE BARD STRIKES UP THE BALLAD OF THE {calling}: {arm} AT THE READY.{refrain}").to_uppercase())
}

/// Eight equipment-led verses keep repeat visits personal without inventing powers.
/// The caller supplies a deterministic beat; no reward or gameplay RNG is consumed.
pub fn company_verse(kit: &str, book: &Book, beat: usize) -> Option<String> {
    let knight = knights::knight(kit)?;
    let calling = label_of(kit).unwrap_or(knight.title);
    let line = match beat % 8 {
        0 => return company_ballad(kit, book),
        1 => format!("THE BARD TAPS A MARCH FOR THE {calling}. {} KEEPS TIME WHILE THE TABLES TAKE COVER.", book.get(knight.guard)?.name),
        2 => match knight.hand.first() {
            Some(id) => format!("A TOAST TO THE {calling}! {} IN THE PACK, AND A CHORUS TOO LOUD FOR THE ROAD.", book.get(id)?.name),
            None => format!("THE {calling} TRAVELS LIGHT. THE BARD LEAVES A REST WHERE THE LUGGAGE VERSE SHOULD BE."),
        },
        4 => format!("THE {calling} LEAVES {} BY THE HEARTH. THE INNKEEPER PUTS A COASTER UNDER IT AND CALLS THAT INSURANCE.", book.get(knight.arm)?.name),
        5 => format!("THE NIGHT WATCH ASKS THE {calling} ABOUT {}. THE BARD ADDS A DRUMROLL; THE WATCH ASKS HIM TO STOP HELPING.", book.get(knight.guard)?.name),
        6 => match knight.spells.first() {
            Some(id) => format!("THE {calling} WRITES {} ON THE REQUEST SLATE. THE BARD AGREES TO SING IT, NOT DEMONSTRATE IT.", book.get(id)?.name),
            None => format!("THE {calling} REQUESTS A QUIET EVENING. THE BARD HAS NEVER LEARNED THAT ONE."),
        },
        7 => match knight.hand.last() {
            Some(id) => format!("THE {calling} SETS {} BESIDE THE SUPPER PLATE. THE COOK INSISTS IT IS NOT AN INGREDIENT.", book.get(id)?.name),
            None => format!("THE {calling} HAS ROOM IN THE PACK FOR LEFTOVERS. THE COOK TAKES THIS AS A CHALLENGE."),
        },
        _ => match knight.spells.last() {
            Some(id) => format!("THE {calling} REQUESTS {}. THE BARD SINGS ITS NAME; THE INNKEEPER HIDES THE GOOD GLASSES.", book.get(id)?.name),
            None => format!("THE {calling} NEEDS NO SPELL FOR AN ENCORE. THE BARD MAKES DO WITH STRINGS AND QUESTIONABLE RHYMES."),
        },
    };
    Some(line.to_uppercase())
}

/// A recruitment receipt built from the same cards that `take_up` equips.
/// Slot numbers teach the controls; card names stay authoritative as kits evolve.
pub fn company_loadout(kit: &str, book: &Book) -> Option<String> {
    let knight = knights::knight(kit)?;
    let name = |id: &str| book.get(id).map(|card| card.name.as_str());
    let mut parts = vec![
        format!("WEAPON: {}", name(knight.arm)?),
        format!("DEFENSE: {}", name(knight.guard)?),
    ];
    for id in knight.hand {
        parts.push(format!("PACK: {}", name(id)?));
    }
    for (slot, id) in knight.spells.iter().enumerate() {
        parts.push(format!("SPELL {}: {}", slot + 1, name(id)?));
    }
    parts.push(format!("{} HEALTH; {} BOMBS", knight.max_hp, knight.bombs));
    Some(parts.join(". ").to_uppercase())
}

/// The patron whose knight `hero` is, if any.
pub fn patron_of(hero: &Hero) -> Option<usize> {
    let kit = hero.knight.as_deref()?;
    PATRONS.iter().position(|p| p.kit == kit)
}

impl Run {
    /// The party begins its delve in the tavern: the kingdom stands behind
    /// the front door, the Delve's gate before it. A run whose world has no
    /// tavern (a bare test floor) begins at the gate as it always did.
    pub fn walk_alehouse(&mut self) {
        self.raise_kingdom();
        super::world::raise_world(&mut self.dungeon);
        let Some(i) = super::world::room_of(&self.dungeon, RoomKind::Alehouse) else {
            self.walk_kingdom(None);
            return;
        };
        let geo = map::geography(&self.kingdom.wishes);
        let stand = map::gate_stand(&self.kingdom.wishes);
        let name = geo
            .sites
            .iter()
            .find(|s| s.room == RoomKind::Gate)
            .map_or_else(|| "The Delve".to_string(), |s| s.name.clone());
        let (x, y) = ((stand.0 as f32 + 0.5) * TILE_UNITS, (stand.1 as f32 + 0.7) * TILE_UNITS);
        let count = self.players.len() as f32;
        // The way back is the kingdom's gate, the party standing side by
        // side before it.
        let party = self
            .players
            .keys()
            .enumerate()
            .map(|(slot, &id)| {
                let offset = (slot as f32 - (count - 1.0) / 2.0) * stand_apart(count);
                (id, (x + offset, y))
            })
            .collect();
        self.kingdom.return_to = Some(kingdom::Return { x, y, name, root: RoomKind::Gate, party });
        self.arrive(i, ARRIVAL);
        self.kingdom.door_held = true;
        // The welcome is the tavern's.
        self.cues.retain(|c| c != "home");
        self.cues.push("alehouse".into());
        for h in self.players.values_mut() {
            h.camera = None;
        }
    }

    /// Whether the party stands in the tavern or the bazaar.
    pub fn in_alehouse(&self) -> bool {
        is_room(self.room().kind)
    }

    /// Who, if anyone, already plays the knight of patron `i`.
    pub fn patron_taken(&self, i: usize, except: u32) -> Option<u32> {
        self.players
            .iter()
            .find(|(id, h)| **id != except && h.knight.as_deref() == Some(PATRONS[i].kit))
            .map(|(&id, _)| id)
    }

    pub(super) fn say(&mut self, id: u32, text: impl Into<String>, good: bool) {
        let text: String = text.into();
        self.alehouse.said.insert(id, Said { at: self.tick, text: text.chars().take(200).collect(), good });
    }

    /// One tick in the tavern: the purses, then each knight's E.
    pub(super) fn tick_alehouse(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.fund_purses();
        let tick = self.tick;
        let kind = self.room().kind;
        let mut presses: Vec<(u32, Spot)> = Vec::new();
        let mut held = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let standing = hero.hp > 0 && !hero.stone && !hero.absent;
            let pressing = standing
                && inputs.get(&id).copied().filter(|i| i.valid()).is_some_and(|i| i.interact);
            if pressing {
                held.push(id);
            }
            let spot = standing
                .then(|| match kind {
                    RoomKind::Bazaar => bazaar::spot_near(hero.x, hero.y),
                    _ => spot_near_in(kind, hero.x, hero.y, tick, self.alehouse.company_page),
                })
                .flatten();
            let arm_slot = if pressing {
                match spot { Some(Spot::Arm(slot)) => Some(slot), _ => None }
            } else {
                None
            };
            let rubbing = pressing && spot == Some(Spot::Lamp);
            let was_rubbing = self.alehouse.lamp_holds.remove(&id);
            if rubbing {
                self.alehouse.lamp_holds.insert(id);
            }
            let previous_arm = self.alehouse.arm_holds.remove(&id);
            if previous_arm != arm_slot || was_rubbing != rubbing {
                hero.buying = 0;
            }
            if let Some(slot) = arm_slot {
                self.alehouse.arm_holds.insert(id, slot);
            }
            let Some(spot) = spot.filter(|_| pressing) else {
                hero.buying = 0;
                hero.buy_spent &= pressing;
                continue;
            };
            if spot.held() {
                if hero.buy_spent {
                    continue;
                }
                hero.buying = hero.buying.saturating_add(1);
                if hero.buying >= BUY_HOLD {
                    hero.buying = 0;
                    hero.buy_spent = true;
                    presses.push((id, spot));
                }
            } else {
                hero.buying = 0;
                if !self.alehouse.held.contains(&id) {
                    presses.push((id, spot));
                }
            }
        }
        self.alehouse.held = held;
        for (id, spot) in presses {
            self.act(id, spot);
        }
    }

    /// Every knight is handed a purse the first time they stand in the
    /// tavern (or the bazaar): the gold to spend here, carried on after.
    pub(super) fn fund_purses(&mut self) {
        for (&id, hero) in self.players.iter_mut() {
            let bit = 1u32 << (id & 31);
            if self.alehouse.funded & bit != 0 {
                continue;
            }
            self.alehouse.funded |= bit;
            hero.carried.add(Spoil::Gold, PURSE);
        }
    }

    fn act(&mut self, id: u32, spot: Spot) {
        match spot {
            Spot::Patron(i) => self.pick_patron(id, i),
            Spot::Lamp => self.rub_home_lamp(id),
            Spot::Arm(slot) => {
                if let (Some(card), Some(hero)) = (home_arms().get(slot), self.players.get_mut(&id)) {
                    hero.equip(card);
                    self.say(id, format!("{} EQUIPPED. FREE TO SWAP AGAIN.", card.name.to_uppercase()), true);
                    self.sounds.push("play_card");
                }
            }
            Spot::Pour(i) => self.buy_pour(id, i),
            Spot::Fence => self.buy_fence(id),
            Spot::Ware(slot) => self.buy_curio(id, slot),
            Spot::Toy(toy) => self.play_toy(id, toy),
        }
    }

    /// Reuse the altar's rub notification: the client opens the usual wish
    /// input and the host's existing genie/learner grants the words, not us.
    fn rub_home_lamp(&mut self, id: u32) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        let first = self.alehouse.lamp_claimed.insert(id);
        if first {
            hero.wishes = hero.wishes.saturating_add(LAMP_WISHES).min(overclass::MOST_WISHES);
        }
        if hero.offer.is_some() {
            self.say(id, "YOUR WISH SCROLL IS OPEN. CHOOSE HOW IT IS GRANTED.", false);
        } else if self.can_wish(id) {
            self.lamp.rubbed.insert(id, self.tick);
            self.say(id, if first { "THREE WISHES, ONCE THIS RUN. SAY YOUR WISH." } else { "SAY YOUR NEXT WISH. THE LAMP REMEMBERS." }, true);
            self.sounds.push("play_card");
        } else {
            self.say(id, "NO WISHES HELD. THE LAMP HAS GIVEN ITS THREE THIS RUN.", false);
        }
    }

    /// E at a patron: be that knight.
    pub fn pick_patron(&mut self, id: u32, i: usize) {
        if !patron_visible(self.alehouse.company_page, i) { return; }
        let p = &PATRONS[i];
        if self.identity {
            self.say(id, "A SHAPE WISHES ITSELF INTO BEING. THE TABLE CAN WAIT.", false);
            return;
        }
        if self.players.get(&id).and_then(patron_of) == Some(i) {
            // Their own arm lost (an older save swapped it on the floor):
            // the table arms them again.
            let arm = knights::knight(p.kit).map(|k| k.arm);
            if arm.is_some_and(|arm| self.players.get(&id).is_some_and(|h| h.arm.as_deref() != Some(arm)))
                && self.take_up(id, p.kit)
            {
                self.say(id, format!("THE {} ARMS YOU AGAIN.", p.label.to_uppercase()), true);
                return;
            }
            self.say(id, format!("YOU ARE THE {} ALREADY.", p.label.to_uppercase()), false);
            return;
        }
        if self.patron_taken(i, id).is_some() {
            self.say(id, format!("THE {} IS ADVENTURING WITH A FRIEND.", p.label.to_uppercase()), false);
            return;
        }
        if self.take_up(id, p.kit) {
            let name = self.players.get(&id).map_or_else(String::new, |h| h.name.to_uppercase());
            self.found = Some((self.tick, id, format!("{name}: NOW THE {}", p.label.to_uppercase())));
            let loadout = company_loadout(p.kit, &self.book).unwrap_or_default();
            self.say(id, format!("YOU ARE THE {}. {} {}", p.label.to_uppercase(), p.pitch.to_uppercase(), loadout), true);
            self.sounds.push("play_card");
            *self.marks.entry("alehouse_pick".into()).or_default() += 1;
            self.first_recruit(id, p.kit, p.label);
        }
    }

    /// Knight `id` becomes the knight of `kit`: their kit, and their class
    /// if the kit has one (its hand and its spells).
    pub fn take_up(&mut self, id: u32, kit: &str) -> bool {
        let Some(who) = knights::knight(kit) else {
            return false;
        };
        if !self.players.contains_key(&id) {
            return false;
        }
        if let Some(hero) = self.players.get_mut(&id) {
            hero.class = None;
            for slot in hero.spells.iter_mut() {
                if slot.as_deref().is_some_and(|s| CLASSES.iter().any(|c| c.spells.contains(&s)) || champions::is_spell(s)) {
                    *slot = None;
                }
            }
            hero.champ = champions::Stance::default();
            // A recruit's ultimate goes with the recruit (`recruit_ults`),
            // and a wish for one is let go with them.
            hero.recruit_ult = None;
        }
        let ult = recruit_ults::recruit(who.id).then(|| self.own_ult(Some(who.id)));
        if let Some(hero) = self.players.get_mut(&id) {
            hero.ult = ult;
        }
        self.outfit(id, who);
        if let (Some(class), Some(hero)) = (CLASSES.iter().find(|c| c.kit == who.id), self.players.get_mut(&id)) {
            hero.class = Some(class.name.to_string());
            hero.hand = class.hand.unwrap_or(who.hand).iter().map(|c| c.to_string()).collect();
            for &spell in class.spells {
                if let Some(slot) = hero.spells.iter_mut().find(|s| s.is_none()) {
                    *slot = Some(spell.to_string());
                }
            }
        }
        // A champion's three spells, on keys 1 to 3.
        if let (Some(champion), Some(hero)) = (champions::Champion::of_kit(who.id), self.players.get_mut(&id)) {
            // Whatever spell they carried keeps the fourth key.
            let kept = hero.spells.iter_mut().find_map(Option::take);
            for (k, spell) in champion.spells().into_iter().enumerate() {
                hero.spells[k] = Some(spell.to_string());
            }
            hero.spells[cards::SPELL_SLOTS - 1] = kept;
        }
        let book = self.book.clone();
        if let Some(hero) = self.players.get_mut(&id) {
            hero.rebonus(&book);
            hero.dress();
        }
        true
    }

    /// E held at one of the keeper's taps.
    fn buy_pour(&mut self, id: u32, i: usize) {
        let pour = &POURS[i];
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        let full = match i {
            0 => hero.hand.len() >= cards::HAND,
            1 => hero.bombs >= hero.bomb_limit(),
            _ => hero.winds >= 2,
        };
        if full {
            self.say(id, "NO ROOM FOR IT. THE KEEPER SHRUGS.", false);
            return;
        }
        if hero.carried.get(Spoil::Gold) < pour.price {
            let need = pour.price - hero.carried.get(Spoil::Gold);
            self.say(id, format!("NEEDS {need} MORE GOLD. THE KEEPER POINTS AT THE TAB."), false);
            return;
        }
        hero.spend_gold(pour.price);
        match i {
            0 => hero.hand.push("potion".into()),
            1 => hero.give_bombs(1),
            _ => hero.winds += 1,
        }
        self.say(id, format!("{}. {}", pour.name.to_uppercase(), pour.does), true);
        self.sounds.push("spoil_pickup");
    }

    /// E held at the fence: its one curio, at a price nobody asks about.
    fn buy_fence(&mut self, id: u32) {
        let (slot, _) = bazaar::fence(self.seed, self.raid_id);
        self.buy_curio(id, slot);
    }

    /// E at a toy.
    fn play_toy(&mut self, id: u32, toy: Toy) {
        let tick = self.tick;
        let seed = self.seed ^ self.raid_id.rotate_left(11);
        // Each knight's own dice: their seat, the clock and the seed.
        let roll = mix(seed ^ (u64::from(id) << 40) ^ tick.wrapping_mul(0x9e37_79b9)) as usize;
        let ballad = self.players.get(&id)
            .and_then(|hero| hero.knight.as_deref())
            .and_then(|kit| company_verse(kit, &self.book, roll));
        let tries = self.alehouse.tries.entry(id).or_default();
        let pick = |lines: &'static [&'static str]| lines[roll % lines.len()];
        let (text, good): (String, bool) = match toy {
            Toy::Stew => {
                if tries.stew >= 2 {
                    ("THE SPOON IS STUCK. SO IS THE STEW. IT IS THE SAME STEW.".into(), false)
                } else {
                    tries.stew += 1;
                    match roll % 5 {
                        0 => {
                            if let Some(h) = self.players.get_mut(&id) {
                                h.give_bombs(1);
                            }
                            ("THERE WAS A BOMB IN THE STEW. THERE IS NOW A BOMB IN YOU. +1 BOMB.".into(), true)
                        }
                        1 => {
                            if let Some(h) = self.players.get_mut(&id) {
                                h.carried.add(Spoil::Gold, 5);
                            }
                            ("A COIN AT THE BOTTOM. NOBODY ASKS WHOSE. +5 GOLD.".into(), true)
                        }
                        2 => ("IT WAS LOOKING BACK AT YOU. YOU FINISH IT ANYWAY.".into(), false),
                        3 => {
                            if let Some(h) = self.players.get_mut(&id) {
                                h.winds = (h.winds + 1).min(2);
                            }
                            ("WARM, SALTY, ODDLY BRAVE. A SECOND WIND, KEPT.".into(), true)
                        }
                        _ => ("IT TASTES LIKE YESTERDAY, BUT SOMEONE ELSE'S.".into(), false),
                    }
                }
            }
            Toy::Darts => {
                if tries.darts >= 3 {
                    ("THE DARTS ARE IN THE CAT NOW. NO MORE.".into(), false)
                } else {
                    tries.darts += 1;
                    match roll % 8 {
                        0 => {
                            if let Some(h) = self.players.get_mut(&id) {
                                h.carried.add(Spoil::Gold, 10);
                            }
                            ("BULLSEYE. THE BOARD PAYS OUT, REGRETFULLY. +10 GOLD.".into(), true)
                        }
                        1 | 2 => ("THE TREBLE TWENTY. YOU DO NOT KNOW WHAT THAT MEANS. IT IS GOOD.".into(), false),
                        3 => ("THE DART STICKS IN THE BEAM. THE BEAM IS UNHAPPY.".into(), false),
                        4 => ("THE DART GOES THROUGH A HAT. THE HAT WAS EMPTY. MOSTLY.".into(), false),
                        5 => ("A NEAT LITTLE HOLE IN SOMEONE'S MUG. NOBODY MENTIONS IT.".into(), false),
                        6 => ("YOU HIT THE WALL, THE FLOOR AND A STRANGER'S OPINION.".into(), false),
                        _ => ("THE DART COMES BACK. THE BOARD THREW IT.".into(), false),
                    }
                }
            }
            Toy::Wrestle => {
                let purse = self.players.get(&id).map_or(0, |h| h.carried.get(Spoil::Gold));
                if tries.wrestles >= 3 {
                    ("BOTH WRESTLERS ARE ON THEIR BACKS, SNORING. THE MATCH IS A DRAW.".into(), false)
                } else if purse < 10 {
                    ("TEN GOLD TO ENTER. THE WRESTLERS DO NOT TAKE THE LOOK ON YOUR FACE.".into(), false)
                } else {
                    tries.wrestles += 1;
                    if let Some(h) = self.players.get_mut(&id) {
                        h.spend_gold(10);
                    }
                    if roll % 3 == 0 {
                        if let Some(h) = self.players.get_mut(&id) {
                            h.carried.add(Spoil::Gold, 25);
                        }
                        ("YOU WIN. THE CROWD, WHICH IS TWO PEOPLE, ROARS. +15 GOLD NET.".into(), true)
                    } else {
                        (pick(&[
                            "YOUR ARM GOES THE WRONG WAY. -10 GOLD, AND A LITTLE PRIDE.",
                            "THE WRESTLER YAWNS AND WINS. -10 GOLD.",
                            "A SPLENDID LOSS. THE PERSON SITTING ON YOU AGREES. -10 GOLD.",
                        ])
                        .into(), false)
                    }
                }
            }
            Toy::Board => {
                let rumours = self.rumours();
                let notes = [
                    "WANTED: SOMEONE TO TAKE THE DELVE'S COMPLAINTS. PAYS IN COMPLAINTS.",
                    "LOST: ONE LEFT BOOT. REWARD: THE RIGHT ONE.",
                    "THE BARD IS NOT TAKING REQUESTS. THE BARD IS NOT BEING ASKED.",
                    "FOR SALE: A GOOD SWORD. NEVER USED. NEVER FOUND.",
                    "WILL THE KNIGHT WHO TOOK MY STOOL PLEASE RETURN MY STOOL.",
                ];
                let note = notes[(mix(seed) as usize + id as usize) % notes.len()];
                (format!("{note} | {}", rumours[0]), false)
            }
            Toy::Hearth => (
                pick(&[
                    "THE FIRE POPS. IT SOUNDS LIKE A LAUGH. IT IS ONE.",
                    "WARM TO THE BONES. THE BONES SAY THANK YOU.",
                    "A LOG SHIFTS AND SPITS A SPARK. YOU STEP BACK, BRAVELY.",
                ])
                .into(),
                false,
            ),
            Toy::Kegs => {
                if tries.keg >= 2 {
                    ("THE KEG IS EMPTY NOW. IT SAYS SO WITH A WHISTLE.".into(), false)
                } else {
                    tries.keg += 1;
                    (pick(&[
                        "A SPRAY OF FOAM. A SECOND SPRAY. THE KEG REGRETS EVERYTHING.",
                        "THE TAP COMES OFF IN YOUR HAND. SO DOES THE BARREL'S SELF-RESPECT.",
                        "A SMALL BOTTLE ROLLS OUT. IT SAYS 'NOT YET.' YOU PUT IT BACK.",
                    ])
                    .into(), false)
                }
            }
            Toy::Lute if ballad.is_some() => (ballad.unwrap(), false),
            Toy::Lute => (
                pick(&[
                    "THE LUTE PLAYER PLAYS A SONG ABOUT YOU. IT RHYMES NOTHING WITH ANYTHING.",
                    "A JIG. THE JIG IS ABOUT A GOAT. THE GOAT IS A SERIOUS CHARACTER.",
                    "A SONG OF DEEP DUNGEONS AND SHALLOW BANK ACCOUNTS.",
                    "THE FIELD SURGEON LEFT A SAINT'S TOURNIQUET ON EVERY DOOR. THE STORM COLLECTOR LEFT A THUNDERHEAD PHIAL. NOBODY KNOCKS ANYMORE.",
                    "THE FIFTH VERSE IS JUST THE WORD 'BATS'. THE ROOM NODS.",
                    "A BALLAD OF THE GRAVEWIRE SIGNALER. HER LAST MESSAGE STILL RINGS INSIDE A PORCELAIN SPOOL.",
                    "THE LANTERN COURIER NEVER CAME HOME. HER LASTLIGHT POSTMARK DID; HER DEADLETTER FLARE STILL SEEKS THE ADDRESS.",
                    "THE DIVER'S WALTZ: THREE STEPS DOWN, ONE COIN FOR THE SUNKEN MINT, NO STEPS BACK.",
                ])
                .into(),
                false,
            ),
            Toy::Snorer(n) => {
                let bit = 1u8 << n;
                if tries.sleepers & bit != 0 {
                    ("THEY ROLL OVER, MUTTER 'NOT TODAY,' AND VANISH BACK INTO THE SNORING.".into(), false)
                } else {
                    tries.sleepers |= bit;
                    if roll % 2 == 0 {
                        if let Some(h) = self.players.get_mut(&id) {
                            h.carried.add(Spoil::Gold, 6);
                        }
                        ("THEY SLEEP-TALK A SECRET: THE LOOSE COIN IN THEIR BOOT. +6 GOLD.".into(), true)
                    } else {
                        ("THEY MUMBLE ABOUT A BAT THE SIZE OF A HORSE. YOU HOPE THEY ARE KIDDING.".into(), false)
                    }
                }
            }
            Toy::Complaints => (
                pick(&[
                    "YOU POST A COMPLAINT. IT IS ANSWERED AT ONCE: 'NOTED.' IT IS NOT NOTED.",
                    "THE BOX HAS 40 COMPLAINTS AND ONE COMPLIMENT. THE COMPLIMENT IS FROM THE BOX.",
                    "YOU COMPLAIN ABOUT THE COMPLAINTS BOX. THE BOX SIGHS, FAINTLY.",
                ])
                .into(),
                false,
            ),
            Toy::Fishbowl => {
                let delve = self.dungeon.pack.name().to_uppercase();
                (
                    pick(&[
                        "THE FISH BLUBS ONCE, DEEPLY, ABOUT THE FUTURE.",
                        "THE FISH SAYS: DO NOT EAT THE SOUP.",
                        "THE FISH SAYS: BUY THE SPOON. THEN IT SAYS NOTHING ELSE FOR A WHILE.",
                    ])
                    .to_string()
                        + &format!(" IT POINTS AT {delve}."),
                    false,
                )
            }
            Toy::Cat => {
                if tries.cat >= 2 {
                    ("THE CAT HAS HAD ENOUGH OF YOU. HER EYES SAY SO, KINDLY.".into(), false)
                } else {
                    tries.cat += 1;
                    if roll % 2 == 0 {
                        if let Some(h) = self.players.get_mut(&id) {
                            h.carried.add(Spoil::Gold, 4);
                        }
                        ("PURRS, AND LEAVES A GIFT BY YOUR BOOT: A COIN, A TOOTH. +4 GOLD.".into(), true)
                    } else {
                        (pick(&[
                            "THE CAT ALLOWS IT. THE CAT ALLOWS A GREAT DEAL ON HER OWN TERMS.",
                            "SHE LOOKS AT YOUR WEAPON, THEN AT YOU. SHE KNOWS WHICH IS SHARPER.",
                        ])
                        .into(), false)
                    }
                }
            }
        };
        self.say(id, text, good);
        if good {
            self.sounds.push("spoil_pickup");
        }
    }
}

/// A hold's bar for a knight's board.
pub fn bar(buying: u32) -> String {
    hold_bar(buying, BUY_HOLD)
}

#[cfg(test)]
#[path = "../../tests/rules/alehouse__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/alehouse__bathroom.rs"]
mod bathroom_tests;
