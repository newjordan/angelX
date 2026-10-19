//! The look a lit room is drawn through (docs/REALM-MONOLITH.md, "Rooms,
//! lit"): one clean, nearly monotone, dark ramp of eight entries for each
//! kind of room, an ember ramp lamplight moves what it lights toward, a
//! cool key light (a high window's) with its shadows, and the room's lamps,
//! brighter while something happens there (light = activity).
//!
//! - **Palettes.** A delve's chambers each have their stone's ramp (the
//!   Crypt's night slate, the Mines' cold slate, Dragon Keep's wine, the
//!   Archive's drowned teal, the Fungal Deep's violet, the Unknown's void,
//!   the Swamp's moss); the kingdom's interiors theirs by kind (the Keep's
//!   slate, a tavern's smoke, a chapel's limestone, a forge's gunmetal, a
//!   hall's granite, the tower's night, the College's verdigris, the
//!   Undercroft's ochre stone); its open places theirs by ground (meadow,
//!   dune, snow, marsh, scree, earth). Warm is only where something burns,
//!   and what burns is its place's: embers, but for the Fungal Deep's spore
//!   green and the Archive's candle gold.
//! - **The key light.** From the front left, so the far wall's face and what
//!   stands in the room catch it and their shadows fall back and to the
//!   right, where the camera sees them; outdoors the low sun of the castle
//!   proof. Indoors the ambient is low and the lamps carry the room; under
//!   the ground the light is low and hard, over almost no ambient, so the
//!   shadows run long and the dark between them is deep.
//! - **Lamps.** The set's own (`lamp` lines: its torches, braziers, hearths
//!   and lava), at most [`angel_stage::look::LAMP_MAX`] nearest the view,
//!   flickering; a fight room's burn full while it is fought, and dim to
//!   embers once it is cleared. Under the ground they burn strong and throw
//!   small, a bright core and a falloff the dither shows, so a room is a few
//!   pools in the dark and not a wash.
//! - **The reveal** opens what stands between the eye and the knight's
//!   chest; a metre is about 11 stage pixels.

use angel_stage::look::{Dither, Lamp, Look, Reveal, Shadow, Sun};

use super::blockout::{Field, Setting};
use crate::rules::{Pack, RoomKind};

/// An eight-entry ramp, dark to light.
pub type Ramp = [[u8; 3]; 8];

/// What a room is lit as: its palette and its key light.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tone {
    /// A delve's chambers, side rooms and halls, in its stone.
    Delve(Pack),
    /// The Keep's halls and the kingdom's grander interiors.
    Keep,
    /// A tavern.
    Tavern,
    /// A chapel or a priory.
    Chapel,
    /// A smithy or the King's forge-hall.
    Forge,
    /// The halls of dressed stone: the Colosseum, the Gander hall, the
    /// Training Yard, the Trophy Hall.
    Hall,
    /// The wizard's tower and its study.
    Tower,
    /// The College's rooms and its courtyard.
    College,
    /// The company's home below the world.
    Undercroft,
    /// An open place of the kingdom, by its ground.
    Open(Ground),
}

/// An open place's ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ground {
    Meadow,
    Dune,
    Snow,
    Marsh,
    Scree,
    Earth,
}

impl Tone {
    /// Every tone, in the order of their numbers.
    pub const ALL: [Tone; 21] = [
        Tone::Delve(Pack::Crypt),
        Tone::Delve(Pack::Cavern),
        Tone::Delve(Pack::Hellforge),
        Tone::Delve(Pack::Archive),
        Tone::Delve(Pack::Fungal),
        Tone::Delve(Pack::Unknown),
        Tone::Delve(Pack::Swamp),
        Tone::Keep,
        Tone::Tavern,
        Tone::Chapel,
        Tone::Forge,
        Tone::Hall,
        Tone::Tower,
        Tone::College,
        Tone::Undercroft,
        Tone::Open(Ground::Meadow),
        Tone::Open(Ground::Dune),
        Tone::Open(Ground::Snow),
        Tone::Open(Ground::Marsh),
        Tone::Open(Ground::Scree),
        Tone::Open(Ground::Earth),
    ];

    /// Its name, as the proof shots and the log say it.
    pub fn name(self) -> &'static str {
        match self {
            Tone::Delve(Pack::Crypt) => "crypt",
            Tone::Delve(Pack::Cavern) => "mines",
            Tone::Delve(Pack::Hellforge) => "dragon-keep",
            Tone::Delve(Pack::Archive) => "archive",
            Tone::Delve(Pack::Fungal) => "fungal",
            Tone::Delve(Pack::Unknown) => "unknown",
            Tone::Delve(Pack::Swamp) => "swamp",
            Tone::Keep => "keep",
            Tone::Tavern => "tavern",
            Tone::Chapel => "chapel",
            Tone::Forge => "forge",
            Tone::Hall => "hall",
            Tone::Tower => "tower",
            Tone::College => "college",
            Tone::Undercroft => "undercroft",
            Tone::Open(Ground::Meadow) => "meadow",
            Tone::Open(Ground::Dune) => "dune",
            Tone::Open(Ground::Snow) => "snow",
            Tone::Open(Ground::Marsh) => "marsh",
            Tone::Open(Ground::Scree) => "scree",
            Tone::Open(Ground::Earth) => "earth",
        }
    }

    /// The tone of a room of kind `kind` dressed `id` in setting `setting`.
    pub fn of(kind: RoomKind, id: &str, setting: Setting) -> Tone {
        match setting {
            Setting::Dungeon(pack) => return Tone::Delve(pack),
            Setting::Outdoor { field, .. } => return Tone::Open(Ground::of(field)),
            Setting::Courtyard { .. } if id.starts_with("college-") => return Tone::College,
            Setting::Courtyard { field } => return Tone::Open(Ground::of(field)),
            Setting::Indoor | Setting::Hall => {}
        }
        if id.starts_with("tower-") || id == "wizard-study" {
            return Tone::Tower;
        }
        if id.starts_with("college-") {
            return Tone::College;
        }
        match kind {
            RoomKind::Tavern => Tone::Tavern,
            RoomKind::Priory | RoomKind::Sanctuary => Tone::Chapel,
            RoomKind::Forge => Tone::Forge,
            _ if id == "smithy" => Tone::Forge,
            RoomKind::Home
            | RoomKind::Stockpile
            | RoomKind::Muster
            | RoomKind::Workshop
            | RoomKind::Quarters
            | RoomKind::RealmHall
            | RoomKind::Fortune
            | RoomKind::Threshold => Tone::Undercroft,
            _ if matches!(setting, Setting::Hall) => Tone::Hall,
            RoomKind::Yard | RoomKind::Trophies => Tone::Hall,
            _ => Tone::Keep,
        }
    }

    /// Its ramp, dark to light.
    pub fn ramp(self) -> Ramp {
        match self {
            // Night slate: the castle proof's indigo with a breath of
            // violet, so the crypt is the Keep's cousin and not its twin.
            Tone::Delve(Pack::Crypt) => [
                [0x0d, 0x0d, 0x1a],
                [0x14, 0x15, 0x28],
                [0x1c, 0x1e, 0x38],
                [0x27, 0x29, 0x4a],
                [0x33, 0x36, 0x5d],
                [0x44, 0x47, 0x74],
                [0x5b, 0x5e, 0x8d],
                [0x7c, 0x80, 0xad],
            ],
            // Slate: wet rock under a cold light, so a lamp is warm against it.
            Tone::Delve(Pack::Cavern) => [
                [0x0b, 0x0d, 0x10],
                [0x11, 0x15, 0x19],
                [0x19, 0x1e, 0x24],
                [0x23, 0x2a, 0x31],
                [0x2f, 0x38, 0x41],
                [0x3f, 0x4a, 0x55],
                [0x53, 0x60, 0x6d],
                [0x71, 0x82, 0x91],
            ],
            // Wine: soot and old brick gone to mauve, so the lava is the only red.
            Tone::Delve(Pack::Hellforge) => [
                [0x11, 0x06, 0x08],
                [0x1a, 0x0a, 0x0c],
                [0x25, 0x0f, 0x12],
                [0x32, 0x17, 0x1a],
                [0x40, 0x1f, 0x22],
                [0x51, 0x2b, 0x2d],
                [0x67, 0x3a, 0x3c],
                [0x82, 0x4e, 0x50],
            ],
            // Drowned teal: stone the water got into.
            Tone::Delve(Pack::Archive) => [
                [0x06, 0x0f, 0x0f],
                [0x0a, 0x17, 0x17],
                [0x10, 0x21, 0x20],
                [0x17, 0x2e, 0x2c],
                [0x21, 0x3d, 0x3b],
                [0x2d, 0x50, 0x4d],
                [0x3d, 0x68, 0x63],
                [0x56, 0x8b, 0x84],
            ],
            // Violet: a forest of mushrooms in the dark.
            Tone::Delve(Pack::Fungal) => [
                [0x13, 0x0a, 0x17],
                [0x1e, 0x10, 0x24],
                [0x2b, 0x17, 0x31],
                [0x3a, 0x21, 0x42],
                [0x4d, 0x2d, 0x56],
                [0x64, 0x3d, 0x6f],
                [0x80, 0x50, 0x8b],
                [0xaa, 0x6d, 0xb6],
            ],
            // Void: near-black ultramarine, cold to the last entry.
            Tone::Delve(Pack::Unknown) => [
                [0x06, 0x08, 0x11],
                [0x0a, 0x0d, 0x1c],
                [0x10, 0x14, 0x27],
                [0x18, 0x1d, 0x35],
                [0x21, 0x29, 0x46],
                [0x2e, 0x38, 0x5a],
                [0x3f, 0x4b, 0x74],
                [0x5a, 0x69, 0x9b],
            ],
            // Moss: the mire under Sir Damp's Marsh.
            Tone::Delve(Pack::Swamp) => [
                [0x0c, 0x0f, 0x0a],
                [0x14, 0x19, 0x10],
                [0x1d, 0x24, 0x17],
                [0x28, 0x30, 0x1e],
                [0x35, 0x3f, 0x27],
                [0x45, 0x52, 0x32],
                [0x5b, 0x6a, 0x42],
                [0x7b, 0x8b, 0x59],
            ],
            // Slate: the castle proof's indigo.
            Tone::Keep => [
                [0x0e, 0x0f, 0x19],
                [0x14, 0x17, 0x27],
                [0x1b, 0x1f, 0x36],
                [0x24, 0x2a, 0x47],
                [0x2f, 0x36, 0x58],
                [0x3e, 0x46, 0x6b],
                [0x54, 0x5c, 0x82],
                [0x77, 0x7f, 0xa2],
            ],
            // Smoke: a tavern's soot-dark timber and plaster.
            Tone::Tavern => [
                [0x10, 0x0e, 0x0d],
                [0x19, 0x16, 0x14],
                [0x24, 0x20, 0x1d],
                [0x31, 0x2b, 0x27],
                [0x40, 0x39, 0x33],
                [0x53, 0x4a, 0x42],
                [0x6d, 0x62, 0x57],
                [0x91, 0x84, 0x75],
            ],
            // Limestone: cool blue-grey.
            Tone::Chapel => [
                [0x0d, 0x0f, 0x13],
                [0x16, 0x19, 0x1e],
                [0x20, 0x24, 0x2b],
                [0x2b, 0x31, 0x3a],
                [0x39, 0x40, 0x4b],
                [0x4b, 0x54, 0x61],
                [0x64, 0x6f, 0x7d],
                [0x89, 0x94, 0xa1],
            ],
            // Gunmetal: iron and soot.
            Tone::Forge => [
                [0x0c, 0x0d, 0x0e],
                [0x15, 0x16, 0x18],
                [0x1f, 0x21, 0x24],
                [0x2a, 0x2d, 0x31],
                [0x37, 0x3b, 0x40],
                [0x48, 0x4d, 0x53],
                [0x60, 0x66, 0x6d],
                [0x83, 0x89, 0x90],
            ],
            // Granite: a warm-neutral grey.
            Tone::Hall => [
                [0x0e, 0x0e, 0x0e],
                [0x17, 0x17, 0x17],
                [0x21, 0x21, 0x20],
                [0x2d, 0x2c, 0x2b],
                [0x3b, 0x3a, 0x38],
                [0x4e, 0x4c, 0x49],
                [0x67, 0x64, 0x5f],
                [0x8b, 0x87, 0x7f],
            ],
            // Night: violet-blue.
            Tone::Tower => [
                [0x0c, 0x0b, 0x16],
                [0x13, 0x11, 0x22],
                [0x1b, 0x18, 0x30],
                [0x25, 0x20, 0x40],
                [0x31, 0x2a, 0x52],
                [0x41, 0x37, 0x68],
                [0x58, 0x4b, 0x84],
                [0x7b, 0x6c, 0xa8],
            ],
            // Verdigris: green-slate.
            Tone::College => [
                [0x0b, 0x10, 0x0f],
                [0x12, 0x19, 0x18],
                [0x1a, 0x24, 0x22],
                [0x23, 0x30, 0x2d],
                [0x2e, 0x3e, 0x3a],
                [0x3d, 0x51, 0x4b],
                [0x53, 0x6b, 0x63],
                [0x75, 0x8f, 0x85],
            ],
            // Ochre stone: the company's cellar halls.
            Tone::Undercroft => [
                [0x0f, 0x0d, 0x0b],
                [0x18, 0x15, 0x11],
                [0x23, 0x1f, 0x18],
                [0x2f, 0x29, 0x20],
                [0x3e, 0x36, 0x2a],
                [0x51, 0x47, 0x37],
                [0x6b, 0x5e, 0x49],
                [0x8f, 0x7f, 0x63],
            ],
            // The castle proof's meadow: sage greys.
            Tone::Open(Ground::Meadow) => [
                [0x0e, 0x12, 0x10],
                [0x17, 0x1f, 0x1b],
                [0x22, 0x2d, 0x27],
                [0x2f, 0x3c, 0x33],
                [0x40, 0x4f, 0x42],
                [0x55, 0x65, 0x52],
                [0x70, 0x7e, 0x67],
                [0x96, 0x9f, 0x84],
            ],
            // Dune: pale ochre in shadow.
            Tone::Open(Ground::Dune) => [
                [0x12, 0x10, 0x0d],
                [0x1d, 0x1a, 0x15],
                [0x2a, 0x25, 0x1d],
                [0x39, 0x32, 0x27],
                [0x4a, 0x42, 0x33],
                [0x60, 0x56, 0x43],
                [0x7c, 0x70, 0x58],
                [0xa1, 0x93, 0x75],
            ],
            // Snow: blue ice, the lightest ramp.
            Tone::Open(Ground::Snow) => [
                [0x0f, 0x12, 0x18],
                [0x19, 0x1e, 0x27],
                [0x25, 0x2c, 0x38],
                [0x34, 0x3d, 0x4c],
                [0x47, 0x52, 0x63],
                [0x5f, 0x6b, 0x7e],
                [0x7f, 0x8b, 0x9d],
                [0xa8, 0xb2, 0xc0],
            ],
            // Marsh: olive.
            Tone::Open(Ground::Marsh) => [
                [0x0e, 0x10, 0x0b],
                [0x17, 0x1a, 0x12],
                [0x22, 0x26, 0x1a],
                [0x2e, 0x33, 0x23],
                [0x3d, 0x43, 0x2e],
                [0x50, 0x57, 0x3c],
                [0x69, 0x71, 0x4f],
                [0x8c, 0x94, 0x6a],
            ],
            // Scree: grey-green stone.
            Tone::Open(Ground::Scree) => [
                [0x0e, 0x10, 0x10],
                [0x17, 0x1a, 0x1a],
                [0x22, 0x26, 0x25],
                [0x2e, 0x33, 0x31],
                [0x3d, 0x43, 0x40],
                [0x50, 0x57, 0x53],
                [0x69, 0x71, 0x6c],
                [0x8d, 0x95, 0x8e],
            ],
            // Earth: umber.
            Tone::Open(Ground::Earth) => [
                [0x11, 0x0e, 0x0b],
                [0x1b, 0x16, 0x12],
                [0x27, 0x20, 0x19],
                [0x34, 0x2b, 0x22],
                [0x44, 0x38, 0x2c],
                [0x58, 0x49, 0x39],
                [0x73, 0x60, 0x4b],
                [0x98, 0x81, 0x66],
            ],
        }
    }

    /// The ramp water is seen through in it: a dark teal by default, its own
    /// in the drowned and the mired places.
    pub fn water(self) -> Ramp {
        match self {
            Tone::Delve(Pack::Swamp) | Tone::Open(Ground::Marsh) => [
                [0x08, 0x0b, 0x09],
                [0x0d, 0x12, 0x0f],
                [0x13, 0x1a, 0x16],
                [0x1a, 0x23, 0x1f],
                [0x22, 0x2e, 0x29],
                [0x2c, 0x3c, 0x35],
                [0x3c, 0x50, 0x46],
                [0x55, 0x6b, 0x5d],
            ],
            _ => [
                [0x08, 0x0d, 0x11],
                [0x0c, 0x15, 0x1b],
                [0x11, 0x1d, 0x25],
                [0x17, 0x27, 0x31],
                [0x1f, 0x33, 0x3e],
                [0x2a, 0x42, 0x4d],
                [0x3a, 0x56, 0x5f],
                [0x55, 0x73, 0x79],
            ],
        }
    }

    /// The ramp lamplight moves what it lights toward: embers, but for the
    /// Fungal Deep's spore glow and the Archive's candlelight.
    pub fn warm(self) -> Ramp {
        match self {
            // Sickly green, as a spore lamp burns.
            Tone::Delve(Pack::Fungal) => [
                [0x09, 0x15, 0x03],
                [0x11, 0x21, 0x06],
                [0x1c, 0x31, 0x0c],
                [0x2a, 0x43, 0x13],
                [0x3d, 0x59, 0x1d],
                [0x57, 0x75, 0x2b],
                [0x7a, 0x9a, 0x3f],
                [0xa6, 0xc6, 0x59],
            ],
            // Pale gold, as a candle burns.
            Tone::Delve(Pack::Archive) => [
                [0x15, 0x11, 0x06],
                [0x21, 0x1c, 0x0b],
                [0x31, 0x2a, 0x12],
                [0x43, 0x3b, 0x1d],
                [0x59, 0x4f, 0x2a],
                [0x76, 0x6a, 0x3d],
                [0x9b, 0x8d, 0x58],
                [0xc8, 0xb9, 0x7c],
            ],
            _ => EMBER,
        }
    }

    /// The colour its fires burn.
    pub fn flame(self) -> [u8; 3] {
        match self {
            Tone::Delve(Pack::Fungal) => SPORE,
            Tone::Delve(Pack::Archive) => CANDLE,
            _ => FLAME,
        }
    }

    /// Whether it is out under the sky: the low sun, not a high window.
    pub fn open(self) -> bool {
        matches!(self, Tone::Open(_))
    }

    /// Its key light: toward it, its strength, and the ambient under it.
    pub fn key(self) -> ([f32; 3], f32, f32) {
        match self {
            // The castle proof's sun: low in the west-south-west.
            Tone::Open(_) => ([-0.66, 0.52, 0.46], 1.05, 0.26),
            // Under the ground: a hard cool light low from the front left,
            // as through a slot in the stone, over almost no ambient, so
            // what stands throws long shadows and the lamps carry the room.
            Tone::Delve(_) => ([-0.70, 0.36, 0.50], 1.2, 0.10),
            // A high window to the front left.
            _ => ([-0.52, 0.72, 0.46], 0.7, 0.24),
        }
    }

    /// How far its shadows run, of the view's reach: a delve's low light
    /// throws them far across the floor.
    pub fn shadow_reach(self) -> f32 {
        match self {
            Tone::Delve(_) => 1.6,
            _ => 1.0,
        }
    }

    /// What its lamps are scaled by, strength and reach: a delve's burn strong
    /// and throw small against its dark, a bright core and a falloff the
    /// dither shows.
    pub fn lamp_gain(self) -> (f32, f32) {
        match self {
            Tone::Delve(_) => (1.5, 0.72),
            _ => (1.0, 1.0),
        }
    }
}

impl Ground {
    pub fn of(field: Field) -> Ground {
        match field {
            Field::Grass => Ground::Meadow,
            Field::Sand => Ground::Dune,
            Field::Snow => Ground::Snow,
            Field::Marsh => Ground::Marsh,
            Field::Scree => Ground::Scree,
            Field::Earth => Ground::Earth,
        }
    }
}

/// Where lamplight moves what it lights: embers.
pub const EMBER: Ramp = [
    [0x17, 0x10, 0x0e],
    [0x29, 0x19, 0x12],
    [0x41, 0x25, 0x17],
    [0x5d, 0x33, 0x1d],
    [0x7f, 0x45, 0x23],
    [0xa3, 0x5e, 0x2b],
    [0xc6, 0x83, 0x3f],
    [0xe6, 0xb2, 0x66],
];

/// The light's glint on water (a look's first accent), and the colours a
/// fire and a lit window keep (`Kind::Fire`, `Kind::Window`).
pub const GLINT: [u8; 3] = [0xd9, 0xd6, 0xb8];
/// The crust of lava and of a bed of coals, cracked with `FLAME`.
pub const EMBER_DEEP: [u8; 3] = [0x7c, 0x2a, 0x14];
pub const FLAME: [u8; 3] = [0xe8, 0x74, 0x2c];
pub const WINDOW: [u8; 3] = [0xee, 0xae, 0x55];
/// The Fungal Deep's fires and the Archive's: spore green and candle gold.
pub const SPORE: [u8; 3] = [0xb4, 0xe0, 0x5c];
pub const CANDLE: [u8; 3] = [0xf2, 0xc2, 0x5a];

/// A metre in stage pixels: a knight stands about two.
pub const METRE: f32 = 11.0;
/// How far round the knight's chest the reveal opens what stands in the
/// way, in stage pixels.
pub const REVEAL: f32 = 18.0;
/// How high a knight's chest is over its feet, in stage pixels.
pub const CHEST: f32 = 13.0;

/// How a room's lamps burn: full while it is fought, steady where people
/// live and work, low embers once a fight is won.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Activity {
    /// A fight is on: the braziers burn full.
    Fought,
    /// Lived in, or a room with no fight: steady.
    Steady,
    /// A fight room cleared: embers.
    Cleared,
}

impl Activity {
    /// How strong its lamps burn, of the set's own strength.
    pub fn strength(self) -> f32 {
        match self {
            Activity::Fought => 1.0,
            Activity::Steady => 0.8,
            Activity::Cleared => 0.42,
        }
    }

    /// How far its lamps reach, of the set's own reach.
    pub fn reach(self) -> f32 {
        match self {
            Activity::Fought => 1.0,
            Activity::Steady => 0.92,
            Activity::Cleared => 0.72,
        }
    }
}

/// Lamp `k`'s flicker at `t` seconds: a few hundredths either way.
pub fn flicker(t: f32, k: usize) -> f32 {
    let k = k as f32 * 1.73;
    1.0 + 0.06 * (t * 7.3 + k).sin() + 0.04 * (t * 12.1 + 2.0 * k).sin()
}

/// The look a room of tone `tone` is drawn through: `lamps` its lamps where
/// they stand on the stage, burning as the room does (the nearest the
/// view's middle `centre` kept, at most `LAMP_MAX`), flickering at `t`
/// seconds; the shadows round `centre` as far as `reach`; the knight's chest
/// at `chest` kept in sight.
pub fn look(tone: Tone, lamps: &[Lamp], t: f32, centre: [f32; 3], reach: f32, chest: Option<[f32; 3]>) -> Look {
    let mut look = Look::new(tone.ramp().to_vec());
    look.accent = vec![GLINT, tone.flame(), WINDOW, EMBER_DEEP];
    look.dither = Dither::Bayer4;
    look.metre = METRE;
    look.time = t;
    let (toward, strength, ambient) = tone.key();
    let (burn, throw) = tone.lamp_gain();
    look.sun = Some(Sun { toward, strength, ambient, shadow: Some(Shadow { focus: centre, reach: reach * tone.shadow_reach() }), rise: 0.0 });
    look.warm = Some(tone.warm().to_vec());
    look.water = Some(tone.water().to_vec());
    let mut near: Vec<(usize, &Lamp)> = lamps.iter().enumerate().collect();
    let far = |l: &Lamp| (l.at[0] - centre[0]).powi(2) + (l.at[2] - centre[2]).powi(2) - l.reach * l.reach;
    near.sort_by(|a, b| far(a.1).total_cmp(&far(b.1)));
    look.lamps = near
        .into_iter()
        .take(angel_stage::look::LAMP_MAX)
        .map(|(k, l)| Lamp { at: l.at, reach: l.reach * throw, strength: l.strength * flicker(t, k) * burn })
        .collect();
    look.reveal = chest.map(|at| Reveal { at, radius: REVEAL });
    look
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ramp_runs_dark_to_light_and_stays_dark_and_nearly_monotone() {
        for tone in Tone::ALL {
            for ramp in [tone.ramp(), tone.water()] {
                let luma = |c: [u8; 3]| 0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2]);
                assert!(ramp.windows(2).all(|w| luma(w[1]) > luma(w[0])), "{}: dark to light", tone.name());
                assert!(luma(ramp[0]) < 20.0 && luma(ramp[7]) < 185.0, "{}: dark, its lightest no white", tone.name());
                // Nearly monotone: one hue, dark to light, every entry with
                // any colour no further from the ramp's middle than the
                // castle proof's meadow drifts (48 degrees).
                let hue = |c: [u8; 3]| -> Option<f32> {
                    let [r, g, b] = c.map(f32::from);
                    let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
                    if hi - lo < 4.0 {
                        return None;
                    }
                    let h = if hi == r {
                        ((g - b) / (hi - lo)).rem_euclid(6.0)
                    } else if hi == g {
                        (b - r) / (hi - lo) + 2.0
                    } else {
                        (r - g) / (hi - lo) + 4.0
                    };
                    Some(h * 60.0)
                };
                let middle = hue(ramp[4]).unwrap_or(0.0);
                for c in ramp {
                    if let Some(h) = hue(c) {
                        let off = (h - middle).abs().min(360.0 - (h - middle).abs());
                        assert!(off <= 55.0, "{}: {c:?} is off its ramp's hue by {off:.0} degrees", tone.name());
                    }
                }
            }
        }
    }

    #[test]
    fn a_delves_lamps_burn_strong_and_throw_small_pools() {
        // Strong, so the dark is deep round them, and small, so a room is lit
        // by a few pools and not a wash.
        let lamp = Lamp { at: [0.0, 20.0, 0.0], reach: 104.0, strength: 1.0 };
        let delve = look(Tone::Delve(Pack::Crypt), &[lamp], 0.0, [0.0; 3], 400.0, None);
        let keep = look(Tone::Keep, &[lamp], 0.0, [0.0; 3], 400.0, None);
        assert!(delve.lamps[0].strength > 1.3 * keep.lamps[0].strength, "{:?}", delve.lamps[0]);
        assert!(delve.lamps[0].reach < 0.8 * lamp.reach, "{:?}", delve.lamps[0]);
        assert_eq!(keep.lamps[0].reach, lamp.reach, "the kingdom's lamps are as the set made them");
    }

    #[test]
    fn each_delve_has_a_colour_of_its_own_and_a_light_to_match() {
        let delves = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp].map(Tone::Delve);
        let far = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).map(|(&x, y)| (f32::from(x) - f32::from(y)).powi(2)).sum::<f32>().sqrt();
        for (i, &a) in delves.iter().enumerate() {
            for &b in &delves[i + 1..] {
                for k in [5, 6] {
                    assert!(far(a.ramp()[k], b.ramp()[k]) > 14.0, "{} and {} are told apart at step {k}", a.name(), b.name());
                }
            }
        }
        let luma = |c: [u8; 3]| 0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2]);
        for tone in delves {
            // The light it burns by keeps the ember ramp's brightness at every
            // step, its colour its own, and its look lets the flame keep it.
            let warm = tone.warm();
            for k in 0..8 {
                assert!((luma(warm[k]) - luma(EMBER[k])).abs() < 14.0, "{} warm step {k}: {:?}", tone.name(), warm[k]);
            }
            assert!(warm.windows(2).all(|w| luma(w[1]) > luma(w[0])), "{}: dark to light", tone.name());
            let lit = look(tone, &[], 0.0, [0.0; 3], 400.0, None);
            assert_eq!(lit.warm.as_deref(), Some(&warm[..]));
            assert_eq!(lit.accent[1], tone.flame(), "{}: its flame is an accent", tone.name());
            assert!(lit.validate().is_ok());
        }
        assert_ne!(Tone::Delve(Pack::Fungal).flame(), FLAME);
        assert_ne!(Tone::Delve(Pack::Archive).warm(), EMBER);
        assert_eq!(Tone::Delve(Pack::Crypt).warm(), EMBER);
        assert_eq!(Tone::Delve(Pack::Crypt).flame(), FLAME);
    }

    #[test]
    fn a_rooms_look_lights_its_room_with_its_lamps_burning_as_it_is_fought() {
        let lamps: Vec<Lamp> = (0..24).map(|k| Lamp { at: [k as f32 * 40.0, 20.0, 30.0], reach: 60.0, strength: 1.0 }).collect();
        let burning = |a: Activity| -> Vec<Lamp> { lamps.iter().map(|l| Lamp { strength: l.strength * a.strength(), reach: l.reach * a.reach(), ..*l }).collect() };
        let fought = look(Tone::Delve(Pack::Crypt), &burning(Activity::Fought), 0.0, [0.0, 0.0, 0.0], 400.0, Some([10.0, 13.0, 10.0]));
        assert!(fought.lights() && fought.validate().is_ok(), "{:?}", fought.validate());
        assert_eq!(fought.lamps.len(), angel_stage::look::LAMP_MAX, "the nearest sixteen");
        assert!(fought.lamps.iter().all(|l| l.at[0] <= (angel_stage::look::LAMP_MAX - 1) as f32 * 40.0), "nearest the middle");
        let cleared = look(Tone::Delve(Pack::Crypt), &burning(Activity::Cleared), 0.0, [0.0, 0.0, 0.0], 400.0, None);
        assert!(cleared.lamps[0].strength < fought.lamps[0].strength * 0.5, "a won fight's braziers dim");
        assert_eq!(fought.metre, METRE);
        assert!(fought.reveal.is_some() && cleared.reveal.is_none());
    }
}
