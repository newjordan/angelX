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

pub type Ramp = [[u8; 3]; 8];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tone {
    Delve(Pack),
    Keep,
    Tavern,
    Chapel,
    Forge,
    Hall,
    Tower,
    College,
    Undercroft,
    Open(Ground),
}

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

    pub fn ramp(self) -> Ramp {
        match self {
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

    pub fn warm(self) -> Ramp {
        match self {
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

    pub fn flame(self) -> [u8; 3] {
        match self {
            Tone::Delve(Pack::Fungal) => SPORE,
            Tone::Delve(Pack::Archive) => CANDLE,
            _ => FLAME,
        }
    }

    pub fn open(self) -> bool {
        matches!(self, Tone::Open(_))
    }

    pub fn key(self) -> ([f32; 3], f32, f32) {
        match self {
            Tone::Open(_) => ([-0.66, 0.52, 0.46], 1.05, 0.26),
            Tone::Delve(Pack::Hellforge) => ([-0.70, 0.36, 0.50], 0.62, 0.21),
            Tone::Delve(_) => ([-0.70, 0.36, 0.50], 0.90, 0.32),
            _ => ([-0.52, 0.72, 0.46], 0.7, 0.24),
        }
    }

    pub fn rise(self) -> f32 {
        match self {
            Tone::Delve(_) => 16.0,
            _ => 0.0,
        }
    }

    pub fn shadow_reach(self) -> f32 {
        match self {
            Tone::Delve(_) => 1.6,
            _ => 1.0,
        }
    }

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

pub const GLINT: [u8; 3] = [0xd9, 0xd6, 0xb8];
pub const EMBER_DEEP: [u8; 3] = [0x7c, 0x2a, 0x14];
pub const FLAME: [u8; 3] = [0xe8, 0x74, 0x2c];
pub const WINDOW: [u8; 3] = [0xee, 0xae, 0x55];
pub const SPORE: [u8; 3] = [0xb4, 0xe0, 0x5c];
pub const CANDLE: [u8; 3] = [0xf2, 0xc2, 0x5a];

pub const METRE: f32 = 11.0;
pub const REVEAL: f32 = 18.0;
pub const CHEST: f32 = 13.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Activity {
    Fought,
    Steady,
    Cleared,
}

impl Activity {
    pub fn strength(self) -> f32 {
        match self {
            Activity::Fought => 1.0,
            Activity::Steady => 0.8,
            Activity::Cleared => 0.42,
        }
    }

    pub fn reach(self) -> f32 {
        match self {
            Activity::Fought => 1.0,
            Activity::Steady => 0.92,
            Activity::Cleared => 0.72,
        }
    }
}

pub fn flicker(t: f32, k: usize) -> f32 {
    let k = k as f32 * 1.73;
    1.0 + 0.06 * (t * 7.3 + k).sin() + 0.04 * (t * 12.1 + 2.0 * k).sin()
}

pub fn look(tone: Tone, lamps: &[Lamp], t: f32, centre: [f32; 3], reach: f32, chest: Option<[f32; 3]>) -> Look {
    let mut look = Look::new(tone.ramp().to_vec());
    look.accent = vec![GLINT, tone.flame(), WINDOW, EMBER_DEEP];
    look.dither = Dither::Bayer4;
    look.metre = METRE;
    look.time = t;
    let (toward, strength, ambient) = tone.key();
    let (burn, throw) = tone.lamp_gain();
    look.sun = Some(Sun { toward, strength, ambient, shadow: Some(Shadow { focus: centre, reach: reach * tone.shadow_reach() }), rise: tone.rise() });
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

pub fn lit_cards_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| !std::env::var("ANGEL_STAGE_LIT_CARDS").is_ok_and(|v| v == "0"))
}

pub fn card_normals_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| !std::env::var("ANGEL_STAGE_CARD_NORMALS").is_ok_and(|v| v == "0"))
}

pub fn dof_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| !std::env::var("ANGEL_STAGE_DOF").is_ok_and(|v| v == "0"))
}

pub fn with_figures(look: &mut Look, feet: &[[f32; 3]]) {
    if lit_cards_on() {
        let normals = if card_normals_on() { angel_stage::look::Cards::ROOM.normals } else { 0.0 };
        look.cards = Some(angel_stage::look::Cards { normals, ..angel_stage::look::Cards::ROOM });
    }
    if dof_on() {
        look.dof = Some(angel_stage::look::Dof::room(focus(feet)));
    }
}

pub fn focus(feet: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let near = feet.iter().copied().max_by(|a, b| a[2].total_cmp(&b[2]));
    let far = feet.iter().copied().min_by(|a, b| a[2].total_cmp(&b[2]));
    near.into_iter().chain(far).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_focus_is_the_nearest_and_farthest_feet_down_the_floor() {
        let feet = [[10.0, 0.0, 80.0], [200.0, 0.0, 30.0], [90.0, 0.0, 150.0], [5.0, 0.0, 100.0]];
        assert_eq!(focus(&feet), vec![[90.0, 0.0, 150.0], [200.0, 0.0, 30.0]]);
        assert!(focus(&[]).is_empty());
        let mut l = look(Tone::Delve(Pack::Crypt), &[], 0.0, [0.0; 3], 100.0, None);
        assert_eq!((l.cards, l.dof.is_some()), (None, false), "a room's look alone has neither");
        with_figures(&mut l, &feet);
        if lit_cards_on() && card_normals_on() {
            assert_eq!(l.cards, Some(angel_stage::look::Cards::ROOM));
        }
        if dof_on() {
            assert_eq!(l.dof.as_ref().map(|d| d.focus.len()), Some(2));
        }
        assert_eq!(l.validate(), Ok(()));
    }

    #[test]
    fn every_ramp_runs_dark_to_light_and_stays_dark_and_nearly_monotone() {
        for tone in Tone::ALL {
            for ramp in [tone.ramp(), tone.water()] {
                let luma = |c: [u8; 3]| 0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2]);
                assert!(ramp.windows(2).all(|w| luma(w[1]) > luma(w[0])), "{}: dark to light", tone.name());
                assert!(luma(ramp[0]) < 20.0 && luma(ramp[7]) < 185.0, "{}: dark, its lightest no white", tone.name());
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
