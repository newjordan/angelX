//! The parts catalogue, as data: every slot of a racer, every part that
//! fits it, what the part does to the racer's stats and the shape its mesh
//! is built from (`meshes`). A part is named in a saved build by its `id`,
//! never by its place in these tables, so the tables may grow and reorder
//! without breaking a save.
//!
//! The four racers of the knight racers example (Excalibur, Bastion, Wraith,
//! Crown) are the starting kit: every part they wear is marked `kit`. The
//! rest are the garage's own designs.

use crate::render::world3d::mesh::mat;
use serde::{Deserialize, Serialize};

/// Where on a racer a part goes. `Engine` is the choice that matters most
/// before the race; the rest trim it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    /// The pair of gothic engines that tow the chariot.
    Engine,
    /// The cockpit or chariot body the knight stands in.
    Chassis,
    /// Hover discs, wheels, treads or runners under the chariot.
    Running,
    /// The exhaust pipes along each engine's spine.
    Exhaust,
    /// Horns, fins or a mast on the engines.
    Crest,
    /// Plating over the engines and the chariot.
    Armour,
    /// The body colour.
    Paint,
    /// The accent colour: bands, rims, the pennant.
    Trim,
}

impl Slot {
    pub const ALL: [Slot; 8] = [
        Slot::Engine,
        Slot::Chassis,
        Slot::Running,
        Slot::Exhaust,
        Slot::Crest,
        Slot::Armour,
        Slot::Paint,
        Slot::Trim,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The word a rack's sign and the stat panel use.
    pub fn name(self) -> &'static str {
        match self {
            Slot::Engine => "ENGINES",
            Slot::Chassis => "CHARIOTS",
            Slot::Running => "RUNNING GEAR",
            Slot::Exhaust => "EXHAUST",
            Slot::Crest => "CRESTS",
            Slot::Armour => "ARMOUR",
            Slot::Paint => "PAINT",
            Slot::Trim => "TRIM",
        }
    }

    /// The same, for a line of text that is not a sign.
    pub fn word(self) -> &'static str {
        match self {
            Slot::Engine => "engine",
            Slot::Chassis => "chariot",
            Slot::Running => "running gear",
            Slot::Exhaust => "exhaust",
            Slot::Crest => "crest",
            Slot::Armour => "armour",
            Slot::Paint => "paint",
            Slot::Trim => "trim",
        }
    }

    /// Whether the slot changes what the racer does, or only how it looks.
    pub fn cosmetic(self) -> bool {
        matches!(self, Slot::Paint | Slot::Trim)
    }
}

/// What a part adds to the racer. Whole numbers on the scale of 0 to 100
/// (see `Stats`): fixed numbers, never formulas, so a build's stats are the
/// same on every machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fx {
    pub speed: i16,
    pub accel: i16,
    pub handling: i16,
    pub armour: i16,
    /// Weight taxes speed, acceleration and handling (`Stats`).
    pub weight: i16,
}

const fn fx(speed: i16, accel: i16, handling: i16, armour: i16, weight: i16) -> Fx {
    Fx { speed, accel, handling, armour, weight }
}

/// What an engine is built like. The pair is mirrored across the racer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EngineKind {
    /// One long n-gonal barrel with a ram on its nose.
    Barrel,
    /// Two slim barrels side by side on a cross-brace.
    Twin,
    /// A short fat drum with a halo of turbine blades round its throat.
    Turbine,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineShape {
    pub kind: EngineKind,
    pub len: f32,
    pub r: f32,
    pub sides: usize,
    pub spikes: usize,
    /// Clear air between the two engines.
    pub gap: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChassisKind {
    /// A tub with a wedge nose.
    Wedge,
    /// A high-sided box wagon.
    Wagon,
    /// A low, narrow skiff with a pointed prow.
    Skiff,
    /// A tub with a tall throne at the back and a gilt rail.
    Throne,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChassisShape {
    pub kind: ChassisKind,
    pub size: f32,
    pub lance: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Hover,
    Wheels,
    Treads,
    Runners,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExhaustShape {
    pub pipes: usize,
    pub height: f32,
    /// Pipes that leave the engine's flank low and sideways, not up.
    pub side: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrestShape {
    pub horns: bool,
    pub fins: bool,
    pub mast: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmourKind {
    Bare,
    Studs,
    Scales,
    Bastion,
    Barding,
}

/// The shape a part's mesh is built from, by slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Engine(EngineShape),
    Chassis(ChassisShape),
    Running(RunKind),
    Exhaust(ExhaustShape),
    Crest(CrestShape),
    Armour(ArmourKind),
    /// The material the body is painted in (a world3d material id, so the
    /// race's own rasterizer paints it the same way).
    Paint(u8),
    Trim(u8),
}

#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub id: &'static str,
    pub slot: Slot,
    pub name: &'static str,
    pub blurb: &'static str,
    pub fx: Fx,
    /// Worn by one of the four starting racers.
    pub kit: bool,
    pub shape: Shape,
}

const fn eng(kind: EngineKind, len: f32, r: f32, sides: usize, spikes: usize, gap: f32) -> Shape {
    Shape::Engine(EngineShape { kind, len, r, sides, spikes, gap })
}

const fn chs(kind: ChassisKind, size: f32, lance: bool) -> Shape {
    Shape::Chassis(ChassisShape { kind, size, lance })
}

const fn exh(pipes: usize, height: f32, side: bool) -> Shape {
    Shape::Exhaust(ExhaustShape { pipes, height, side })
}

const fn crest(horns: bool, fins: bool, mast: bool) -> Shape {
    Shape::Crest(CrestShape { horns, fins, mast })
}

/// The bare racer every build starts from, before its parts.
pub const BASE: Fx = fx(30, 30, 30, 10, 30);
/// The weight at which a racer is neither taxed nor spared.
pub const NEUTRAL_WEIGHT: i32 = 50;

pub static PARTS: &[Part] = &[
    // ---------------------------------------------------------- engines
    Part { id: "gryphon_six", slot: Slot::Engine, name: "Gryphon Six", blurb: "Hexagonal barrel, long and level. Goes where it is pointed.", fx: fx(16, 8, 4, 0, 12), kit: true, shape: eng(EngineKind::Barrel, 4.2, 0.7, 6, 4, 1.3) },
    Part { id: "bastion_block", slot: Slot::Engine, name: "Bastion Block", blurb: "A square iron stump. Slow to sing, hard to dent.", fx: fx(6, 12, -2, 8, 18), kit: true, shape: eng(EngineKind::Barrel, 3.4, 0.95, 4, 4, 1.6) },
    Part { id: "wraith_needle", slot: Slot::Engine, name: "Wraith Needle", blurb: "Pentagonal and thin as a rumour. Fast, and it knows it.", fx: fx(22, 4, 6, -4, 6), kit: true, shape: eng(EngineKind::Barrel, 4.8, 0.58, 5, 3, 1.2) },
    Part { id: "crown_cathedral", slot: Slot::Engine, name: "Crown Cathedral", blurb: "Six spikes round the collar. Heavy, proud, well-rounded.", fx: fx(12, 8, 0, 6, 16), kit: true, shape: eng(EngineKind::Barrel, 3.8, 0.8, 6, 6, 1.4) },
    Part { id: "hellhound_twin", slot: Slot::Engine, name: "Hellhound Twin", blurb: "Two barrels, one temper. Leaps off the line.", fx: fx(10, 20, -4, 2, 14), kit: false, shape: eng(EngineKind::Twin, 3.6, 0.5, 6, 2, 1.1) },
    Part { id: "black_sun", slot: Slot::Engine, name: "Black Sun Turbine", blurb: "A halo of blades round a drum. Sharp in the bends.", fx: fx(4, 14, 14, 0, 10), kit: false, shape: eng(EngineKind::Turbine, 2.8, 1.05, 8, 0, 1.5) },
    // ---------------------------------------------------------- chariots
    Part { id: "lancer_wedge", slot: Slot::Chassis, name: "Lancer Wedge", blurb: "Wedge-nosed tub with a lance and a banner.", fx: fx(2, 0, 6, 2, 8), kit: true, shape: chs(ChassisKind::Wedge, 1.8, true) },
    Part { id: "bastion_wagon", slot: Slot::Chassis, name: "Bastion Wagon", blurb: "High iron sides. The knight is hard to find.", fx: fx(-2, 0, -4, 12, 16), kit: true, shape: chs(ChassisKind::Wagon, 2.0, false) },
    Part { id: "wraith_skiff", slot: Slot::Chassis, name: "Wraith Skiff", blurb: "A narrow prow and a lance. Mostly knight.", fx: fx(4, 2, 10, -2, 4), kit: true, shape: chs(ChassisKind::Skiff, 1.7, true) },
    Part { id: "crown_throne", slot: Slot::Chassis, name: "Crown Throne", blurb: "A tall seat and a gilt rail. Rides as it looks.", fx: fx(0, 0, 2, 8, 12), kit: true, shape: chs(ChassisKind::Throne, 1.9, false) },
    // ---------------------------------------------------------- running gear
    Part { id: "hover_discs", slot: Slot::Running, name: "Hover Discs", blurb: "Steel pads on a cushion of blue. Light and slippery.", fx: fx(2, 0, 4, 0, 4), kit: true, shape: Shape::Running(RunKind::Hover) },
    Part { id: "war_wheels", slot: Slot::Running, name: "War Wheels", blurb: "Spoked iron with spiked tyres. Bites the road.", fx: fx(0, 6, 2, 2, 10), kit: false, shape: Shape::Running(RunKind::Wheels) },
    Part { id: "iron_treads", slot: Slot::Running, name: "Iron Treads", blurb: "Siege treads. Nothing stops them, little hurries them.", fx: fx(-2, 2, -2, 6, 16), kit: false, shape: Shape::Running(RunKind::Treads) },
    Part { id: "ice_runners", slot: Slot::Running, name: "Glide Runners", blurb: "Long steel blades. Fast and straight, loose in a bend.", fx: fx(6, 0, -4, 0, 2), kit: false, shape: Shape::Running(RunKind::Runners) },
    // ---------------------------------------------------------- exhaust
    Part { id: "triple_organ", slot: Slot::Exhaust, name: "Triple Organ", blurb: "Three stacks, a blue band on each.", fx: fx(3, 3, 0, 0, 4), kit: true, shape: exh(3, 1.4, false) },
    Part { id: "twin_spire", slot: Slot::Exhaust, name: "Twin Spire", blurb: "Two tall stacks. Breathes well, shouts louder.", fx: fx(1, 4, 0, 0, 3), kit: true, shape: exh(2, 1.9, false) },
    Part { id: "low_gutter", slot: Slot::Exhaust, name: "Low Gutter", blurb: "Three short stacks laid close. Cheats the wind.", fx: fx(4, 1, 2, 0, 3), kit: true, shape: exh(3, 1.1, false) },
    Part { id: "grand_organ", slot: Slot::Exhaust, name: "Grand Organ", blurb: "Two towering pipes. The cathedral's own voice.", fx: fx(2, 5, -1, 0, 6), kit: true, shape: exh(2, 2.2, false) },
    Part { id: "side_dragons", slot: Slot::Exhaust, name: "Side Dragons", blurb: "Four pipes out the flank. Low, loud, quick to spool.", fx: fx(0, 7, 2, 0, 5), kit: false, shape: exh(4, 0.8, true) },
    // ---------------------------------------------------------- crests
    Part { id: "bare_crest", slot: Slot::Crest, name: "Bare Cowl", blurb: "Nothing on top. Nothing to catch.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: crest(false, false, false) },
    Part { id: "ram_horns", slot: Slot::Crest, name: "Ram Horns", blurb: "Steel horns swept forward. For the shoulder charge.", fx: fx(0, 0, -1, 3, 3), kit: true, shape: crest(true, false, false) },
    Part { id: "gale_fins", slot: Slot::Crest, name: "Gale Fins", blurb: "Blade fins along the spine. Settles the tail.", fx: fx(3, 0, 5, 0, 2), kit: true, shape: crest(false, true, false) },
    Part { id: "crown_crest", slot: Slot::Crest, name: "Crown Crest", blurb: "Horns and fins together. Worn by kings.", fx: fx(2, 0, 3, 2, 5), kit: true, shape: crest(true, true, false) },
    Part { id: "pennon_mast", slot: Slot::Crest, name: "Pennon Mast", blurb: "A tall mast and a pennant. Mostly for show.", fx: fx(0, 0, 1, 0, 1), kit: false, shape: crest(false, false, true) },
    // ---------------------------------------------------------- armour
    Part { id: "bare_plate", slot: Slot::Armour, name: "No Plating", blurb: "Bare iron under the paint.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: Shape::Armour(ArmourKind::Bare) },
    Part { id: "studded_hide", slot: Slot::Armour, name: "Studded Hide", blurb: "Boiled hide and brass studs. A little cover, no fuss.", fx: fx(0, 0, 0, 6, 4), kit: false, shape: Shape::Armour(ArmourKind::Studs) },
    Part { id: "scale_mail", slot: Slot::Armour, name: "Scale Mail", blurb: "Overlapping scales down both engines.", fx: fx(-1, -1, 0, 12, 10), kit: false, shape: Shape::Armour(ArmourKind::Scales) },
    Part { id: "bastion_plate", slot: Slot::Armour, name: "Bastion Plate", blurb: "Slab plate on every flank. The race will notice.", fx: fx(-3, -3, -3, 20, 20), kit: false, shape: Shape::Armour(ArmourKind::Bastion) },
    Part { id: "spiked_barding", slot: Slot::Armour, name: "Spiked Barding", blurb: "Plate with spikes, for anyone who tries to pass.", fx: fx(-2, -2, -4, 16, 16), kit: false, shape: Shape::Armour(ArmourKind::Barding) },
    // ---------------------------------------------------------- paint
    Part { id: "black_iron", slot: Slot::Paint, name: "Black Iron", blurb: "Unpainted forge black.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: Shape::Paint(mat::STONE_DARK) },
    Part { id: "excalibur_blue", slot: Slot::Paint, name: "Excalibur Blue", blurb: "The deep blue of a drawn sword.", fx: fx(0, 0, 0, 0, 0), kit: false, shape: Shape::Paint(mat::CLOTH_BLUE) },
    Part { id: "banner_red", slot: Slot::Paint, name: "Banner Red", blurb: "Dyed with something best not asked about.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: Shape::Paint(mat::BANNER) },
    Part { id: "sun_gold", slot: Slot::Paint, name: "Sunforged Gold", blurb: "Gilt over everything. Hard to miss.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: Shape::Paint(mat::CLOTH_GOLD) },
    Part { id: "bone_white", slot: Slot::Paint, name: "Bone White", blurb: "Scraped and bleached, ready for the grave.", fx: fx(0, 0, 0, 0, 0), kit: false, shape: Shape::Paint(mat::BONE) },
    Part { id: "violet_dusk", slot: Slot::Paint, name: "Violet Dusk", blurb: "The colour of the sky a minute after the sun goes.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: Shape::Paint(mat::ROOF) },
    // ---------------------------------------------------------- trim
    Part { id: "blue_glow", slot: Slot::Trim, name: "Blue Glow", blurb: "Cold light in every seam.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: Shape::Trim(mat::CRYSTAL) },
    Part { id: "brass_trim", slot: Slot::Trim, name: "Brass", blurb: "Rails and bands in old brass.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: Shape::Trim(mat::BRASS) },
    Part { id: "ember_trim", slot: Slot::Trim, name: "Ember", blurb: "Lit from inside, like a forge door.", fx: fx(0, 0, 0, 0, 0), kit: false, shape: Shape::Trim(mat::EMBER) },
    Part { id: "gilt_trim", slot: Slot::Trim, name: "Gilt", blurb: "Gold leaf on the bands.", fx: fx(0, 0, 0, 0, 0), kit: false, shape: Shape::Trim(mat::CLOTH_GOLD) },
    Part { id: "silver_trim", slot: Slot::Trim, name: "Silver", blurb: "Polished plate on the bands.", fx: fx(0, 0, 0, 0, 0), kit: true, shape: Shape::Trim(mat::ARMOR) },
    Part { id: "crimson_trim", slot: Slot::Trim, name: "Crimson", blurb: "A red edge. A warning.", fx: fx(0, 0, 0, 0, 0), kit: false, shape: Shape::Trim(mat::BANNER) },
];

/// The part with this id, if the catalogue has one.
pub fn part(id: &str) -> Option<&'static Part> {
    PARTS.iter().find(|p| p.id == id)
}

/// The parts that fit a slot, in the catalogue's order (the rack's order).
pub fn options(slot: Slot) -> impl Iterator<Item = &'static Part> + Clone {
    PARTS.iter().filter(move |p| p.slot == slot)
}

/// How many parts a slot has.
pub fn count(slot: Slot) -> usize {
    options(slot).count()
}

/// The part in position `i` of a slot's rack.
pub fn nth(slot: Slot, i: usize) -> Option<&'static Part> {
    options(slot).nth(i)
}

/// Where in its slot's rack a part stands.
pub fn position(p: &Part) -> usize {
    options(p.slot).position(|q| q.id == p.id).unwrap_or(0)
}

/// Two parts are the same part when they have the same id.
impl PartialEq for Part {
    fn eq(&self, other: &Part) -> bool {
        self.id == other.id
    }
}

impl Eq for Part {}
