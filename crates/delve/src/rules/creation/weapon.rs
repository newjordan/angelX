//! The Weapon creation class (SPEC-CREATOR 1b): its own recipe, never a part of a creature.
//!
//! Parts, joints and anchors come from the arms library (`assets/creator/weapons/weapons.json`,
//! packed by `scripts/creator/build-weapon-data.py`). Assembly, stats and abilities follow
//! `army/arms/WEAPON-BUILDER.md` section 4 exactly, so a library weapon rebuilt from its own parts
//! reproduces its baked numbers (a test holds this for all 31). No pixels are read here: the
//! renderer draws the `Assembly` this module places. Nothing is ever typed in: a weapon's powers
//! are derived from its parts.

use super::{Ability, Form, School, Spell};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub const WEAPON_VERSION: u8 = 1;
/// One weapon's share of a hero's 50 points (SPEC-CREATOR 5, wishit `WEAPON_BUDGET`).
pub const WEAPON_BUDGET: u16 = 20;

#[derive(Clone, Debug, Deserialize)]
pub struct Joint {
    pub pos: [i32; 2],
    #[serde(default)]
    pub class: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct PartStat {
    #[serde(default)]
    pub dmg: i32,
    #[serde(default)]
    pub spd: i32,
    #[serde(default)]
    pub blk: i32,
    #[serde(default)]
    pub sp: i32,
    #[serde(default)]
    pub cost: i32,
}

/// One library part: native pixel size, joints, anchors and the numbers it carries.
#[derive(Clone, Debug, Deserialize)]
pub struct KitPart {
    pub id: String,
    pub slot: String,
    pub class: String,
    pub prox: Option<Joint>,
    pub dist: Option<Joint>,
    #[serde(default)]
    pub anchors: BTreeMap<String, [i32; 2]>,
    pub mass: f32,
    #[serde(default)]
    pub stat: PartStat,
    #[serde(default)]
    pub abilities: Vec<String>,
    #[serde(default)]
    pub materials: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub size: [i32; 2],
    pub alpha_cols: [i32; 2],
    /// Where its pixels sit in `parts.png`.
    pub rect: [i32; 4],
}

impl KitPart {
    pub fn short(&self) -> &str {
        self.id.strip_prefix("w5w.").unwrap_or(&self.id)
    }
    pub fn joint_class(&self) -> u8 {
        self.prox.as_ref().and_then(|j| j.class).unwrap_or(0)
    }
    pub fn school(&self) -> Option<School> {
        School::ALL
            .into_iter()
            .find(|s| self.tags.iter().any(|t| t == &s.name().to_lowercase()))
    }
    pub fn cost(&self) -> u16 {
        self.stat.cost.max(0) as u16
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct KitRecipePart {
    pub part_id: String,
    #[serde(default)]
    pub offset: [i32; 2],
    #[serde(default)]
    pub flipped: bool,
    #[serde(default)]
    pub length: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KitGem {
    pub part_id: String,
    pub on: String,
    pub pos: [i32; 2],
}

#[derive(Clone, Debug, Deserialize)]
pub struct KitStats {
    pub damage: i32,
    pub speed: i32,
    pub reach: f32,
    pub mass: f32,
    pub block: i32,
    pub spell_power: i32,
    pub cost: i32,
    #[serde(default)]
    pub range: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KitAbility {
    pub id: String,
    pub from_part: Option<String>,
    pub level: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KitSpell {
    pub school: String,
    pub form: String,
    pub power: i32,
}

/// A library weapon as baked: the standard the builder is tested against.
#[derive(Clone, Debug, Deserialize)]
pub struct KitWeapon {
    pub id: String,
    pub name: String,
    pub weapon_class: String,
    pub hold: String,
    pub parts: Vec<KitRecipePart>,
    #[serde(default)]
    pub gems: Vec<KitGem>,
    pub stats: KitStats,
    #[serde(default)]
    pub abilities: Vec<KitAbility>,
    #[serde(default)]
    pub spells: Vec<KitSpell>,
    #[serde(default)]
    pub school: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Kit {
    pub ramps: BTreeMap<String, Vec<String>>,
    pub parts: Vec<KitPart>,
    pub weapons: Vec<KitWeapon>,
}

pub fn kit() -> &'static Kit {
    static KIT: OnceLock<Kit> = OnceLock::new();
    KIT.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/creator/weapons/weapons.json"))
            .expect("the embedded weapon kit")
    })
}

impl Kit {
    pub fn part(&self, id: &str) -> Option<&KitPart> {
        self.parts.iter().find(|p| p.id == id)
    }
    pub fn part_index(&self, id: &str) -> Option<usize> {
        self.parts.iter().position(|p| p.id == id)
    }
}

/// What the player chooses first: the type fixes the slot chain and the joint class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponKind {
    #[default]
    Blade,
    Haft,
    Bow,
    Shield,
    Staff,
    Wand,
}

impl WeaponKind {
    pub const ALL: [Self; 6] = [
        Self::Blade,
        Self::Haft,
        Self::Bow,
        Self::Shield,
        Self::Staff,
        Self::Wand,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Blade => "Blade",
            Self::Haft => "Haft",
            Self::Bow => "Bow",
            Self::Shield => "Shield",
            Self::Staff => "Staff",
            Self::Wand => "Wand",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Self::Blade => "Sword or cleaver",
            Self::Haft => "Mace, axe, spear",
            Self::Bow => "Short or long",
            Self::Shield => "Face, rim, boss",
            Self::Staff => "A school's head",
            Self::Wand => "A small focus",
        }
    }
    /// The chain, pommel end to tip. A bow's grip and string are fixed.
    pub fn slots(self) -> &'static [&'static str] {
        match self {
            Self::Blade => &["pommel", "grip", "guard", "blade"],
            Self::Haft | Self::Staff => &["pommel", "haft", "head"],
            Self::Bow => &["limb"],
            Self::Shield => &["face", "rim", "boss"],
            Self::Wand => &["haft", "head"],
        }
    }
    pub fn allows_gem(self) -> bool {
        matches!(self, Self::Blade | Self::Haft)
    }
    pub fn is_focus(self) -> bool {
        matches!(self, Self::Staff | Self::Wand)
    }
    pub fn forms(self) -> &'static [Form] {
        match self {
            Self::Wand => &[Form::Bolt, Form::Beam],
            Self::Staff => &[Form::Bolt, Form::Nova, Form::Ward, Form::Aura, Form::Summon],
            _ => &[],
        }
    }
}

pub fn slot_label(slot: &str, kind: WeaponKind, class: u8) -> &'static str {
    match slot {
        "pommel" if kind == WeaponKind::Haft && class == 11 => "Butt",
        "pommel" => "Pommel",
        "grip" => "Grip",
        "guard" => "Guard",
        "blade" => "Blade",
        "haft" if kind == WeaponKind::Staff || kind == WeaponKind::Wand => "Shaft",
        "haft" => "Haft",
        "head" => "Head",
        "limb" => "Limbs",
        "face" => "Face",
        "rim" => "Rim",
        "boss" => "Boss",
        _ => "Part",
    }
}

/// What the picker calls a library part: "w5w.pommel-skull" reads "skull pommel".
pub fn part_name(p: &KitPart) -> String {
    let mut t: Vec<&str> = p.short().split('-').collect();
    let slot = t.remove(0);
    // A head is named by what it is: "head-axe-war" is a war axe head.
    if matches!(t.first(), Some(&("axe" | "hammer" | "club" | "mace" | "spear" | "staff" | "wand" | "scythe"))) {
        t.reverse();
    }
    if t.is_empty() {
        return slot.to_string();
    }
    let slot = if slot == "shaft" { "shaft" } else { slot };
    format!("{} {slot}", t.join(" "))
}

/// A library material ramp: the paint remaps whole ramps, index for index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    #[default]
    Plate,
    Gold,
    Bone,
    Leather,
    Cloth,
    Shell,
    Glow,
    Skin,
    Chitin,
    Eye,
    FleshInner,
    GagPink,
}

impl Material {
    pub const METALS: [Self; 6] = [
        Self::Plate,
        Self::Gold,
        Self::Bone,
        Self::Shell,
        Self::Chitin,
        Self::Glow,
    ];
    pub const BINDINGS: [Self; 5] = [Self::Leather, Self::Cloth, Self::Bone, Self::Skin, Self::Plate];
    pub fn ramp(self) -> &'static str {
        match self {
            Self::Plate => "plate",
            Self::Gold => "gold",
            Self::Bone => "bone",
            Self::Leather => "leather",
            Self::Cloth => "cloth",
            Self::Shell => "shell",
            Self::Glow => "glow",
            Self::Skin => "skin",
            Self::Chitin => "chitin",
            Self::Eye => "eye",
            Self::FleshInner => "flesh_inner",
            Self::GagPink => "gag_pink",
        }
    }
    pub fn from_ramp(name: &str) -> Option<Self> {
        [
            Self::Plate,
            Self::Gold,
            Self::Bone,
            Self::Leather,
            Self::Cloth,
            Self::Shell,
            Self::Glow,
            Self::Skin,
            Self::Chitin,
            Self::Eye,
            Self::FleshInner,
            Self::GagPink,
        ]
        .into_iter()
        .find(|m| m.ramp() == name)
    }
    /// What the Paint page calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Plate => "Steel",
            Self::Gold => "Gold",
            Self::Bone => "Bone",
            Self::Leather => "Leather",
            Self::Cloth => "Cloth",
            Self::Shell => "Rust",
            Self::Glow => "Frost",
            Self::Skin => "Moss",
            Self::Chitin => "Violet",
            Self::Eye => "Pearl",
            Self::FleshInner => "Blood",
            Self::GagPink => "Rose",
        }
    }
    /// The ramp a school's glow is painted in.
    pub fn of_school(school: School) -> Self {
        match school {
            School::Fire => Self::Cloth,
            School::Ice => Self::Glow,
            School::Lightning => Self::Eye,
            School::Arcane => Self::GagPink,
            School::Holy => Self::Gold,
            School::Shadow => Self::Chitin,
            School::Nature => Self::Skin,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponPaint {
    /// Every steel pixel (the `plate` ramp).
    pub metal: Material,
    /// Every wood, rag and leather pixel; Leather leaves them as made.
    pub binding: Material,
    /// Tint of the magic parts (a staff's head, a gem); `None` leaves them as made.
    pub glow: Option<School>,
    /// A single part's metal, such as a gold guard.
    pub slot_metal: BTreeMap<String, Material>,
}

impl Default for WeaponPaint {
    fn default() -> Self {
        Self {
            metal: Material::Plate,
            binding: Material::Leather,
            glow: None,
            slot_metal: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponPart {
    pub slot: String,
    pub part_id: String,
    /// Percent; 100 as made. Preview-sized only: the bake redraws the whole weapon.
    pub scale: u16,
}

impl WeaponPart {
    pub fn new(slot: &str, id: &str) -> Self {
        Self {
            slot: slot.into(),
            part_id: id.into(),
            scale: 100,
        }
    }
}

/// The editor's choices: no stats, abilities or costs are ever typed in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponCreation {
    pub id: String,
    pub name: String,
    pub maker: String,
    pub created: u64,
    pub kind: WeaponKind,
    pub parts: Vec<WeaponPart>,
    /// Length in native pixels of the part that stretches (a haft, or a straight blade): a plain
    /// middle slice is cut or repeated, so a longer blade keeps its tip and its fuller.
    pub stretch: Option<u16>,
    /// An enchant gem on a blade or a haft.
    pub gem: Option<School>,
    pub paint: WeaponPaint,
}

impl Default for WeaponCreation {
    fn default() -> Self {
        Self::of_kind(WeaponKind::Blade)
    }
}

/// Where each part sits, relative to the top-left of the assembled canvas (as `weapon.json` says).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    /// Index into `kit().parts`.
    pub kit: usize,
    pub slot: String,
    pub off: (i32, i32),
    pub flipped: bool,
    pub scale: u16,
    pub len: Option<u16>,
    pub z: u8,
    pub gem: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembly {
    /// In draw order, back to front; gems last.
    pub placed: Vec<Placed>,
    pub size: (i32, i32),
    pub grip: (i32, i32),
    pub grip2: Option<(i32, i32)>,
    /// The last opaque column, measured on the parts' own alpha.
    pub tip_x: i32,
    pub cast: Option<(i32, i32)>,
    /// Joint positions along the axis, for the ghost the picker draws.
    pub joints: Vec<(i32, i32)>,
}

/// The source column of each output column when a haft is resized to `len` (a plain middle
/// slice is removed or repeated): shared by the geometry here and the pixels in the renderer.
pub struct HaftCut {
    pub cols: Vec<i32>,
    w: i32,
    d: i32,
    a1: i32,
    newx: Vec<i32>,
    keep: Vec<bool>,
}

impl HaftCut {
    pub fn bounds(w: i32) -> (i32, i32) {
        let band = |a: f32, b: f32| ((w as f32 * b) as i32 - (w as f32 * a) as i32).max(0);
        (w - band(0.36, 0.58) - band(0.70, 0.92), w + 60)
    }
    pub fn new(w: i32, len: i32) -> Self {
        let (lo, hi) = Self::bounds(w);
        let len = len.clamp(lo, hi);
        let d = len - w;
        let mut keep = vec![true; w as usize];
        let (a0, a1) = ((w as f32 * 0.36) as i32, (w as f32 * 0.58) as i32);
        let mut cols: Vec<i32> = Vec::new();
        if d < 0 {
            let mut left = -d;
            for (b0, b1) in [(a0, a1), ((w as f32 * 0.70) as i32, (w as f32 * 0.92) as i32)] {
                let take = left.min(b1 - b0);
                for x in b0..b0 + take {
                    keep[x as usize] = false;
                }
                left -= take;
            }
            cols = (0..w).filter(|&x| keep[x as usize]).collect();
        } else {
            cols.extend(0..a1);
            let mut need = d;
            while need > 0 {
                let take = need.min(a1 - a0);
                cols.extend(a0..a0 + take);
                need -= take;
            }
            cols.extend(a1..w);
        }
        let mut newx = Vec::with_capacity(w as usize);
        let mut n = -1;
        for &k in &keep {
            n += i32::from(k);
            newx.push(n);
        }
        Self {
            cols,
            w,
            d,
            a1,
            newx,
            keep,
        }
    }
    pub fn len(&self) -> i32 {
        self.cols.len() as i32
    }
    pub fn is_empty(&self) -> bool {
        self.cols.is_empty()
    }
    /// A source x (a joint or an anchor) in the resized part.
    pub fn map(&self, x: i32) -> i32 {
        let x = x.clamp(0, self.w - 1);
        if self.d >= 0 {
            if x >= self.a1 { x + self.d } else { x }
        } else if self.keep[x as usize] {
            self.newx[x as usize]
        } else {
            self.newx[(x + 1).min(self.w - 1) as usize]
        }
    }
}

/// One part as placed: size, joints and anchors after a haft cut or a preview scale.
struct Fitted {
    size: (i32, i32),
    prox: Option<(i32, i32)>,
    dist: Option<(i32, i32)>,
    anchors: BTreeMap<String, (i32, i32)>,
    last_col: i32,
}

fn fit(p: &KitPart, flipped: bool, scale: u16, len: Option<u16>) -> Fitted {
    let w = p.size[0];
    let pt = |q: [i32; 2]| (q[0], q[1]);
    let mut f = Fitted {
        size: (w, p.size[1]),
        prox: p.prox.as_ref().map(|j| pt(j.pos)),
        dist: p.dist.as_ref().map(|j| pt(j.pos)),
        anchors: p.anchors.iter().map(|(k, v)| (k.clone(), pt(*v))).collect(),
        last_col: p.alpha_cols[1],
    };
    if let (Some(len), "haft" | "blade") = (len, p.slot.as_str()) {
        let cut = HaftCut::new(w, i32::from(len));
        let m = |q: (i32, i32)| (cut.map(q.0), q.1);
        f.size.0 = cut.len();
        f.prox = f.prox.map(m);
        f.dist = f.dist.map(m);
        f.anchors.values_mut().for_each(|a| *a = m(*a));
        f.last_col = cut.len() - 1;
    }
    if scale != 100 {
        let s = |v: i32| (v * i32::from(scale) + 50) / 100;
        let m = |q: (i32, i32)| (s(q.0), s(q.1));
        f.size = (s(f.size.0).max(1), s(f.size.1).max(1));
        f.prox = f.prox.map(m);
        f.dist = f.dist.map(m);
        f.anchors.values_mut().for_each(|a| *a = m(*a));
        f.last_col = s(f.last_col + 1) - 1;
    }
    if flipped {
        let w = f.size.0;
        let m = |q: (i32, i32)| (w - 1 - q.0, q.1);
        f.prox = f.prox.map(m);
        f.dist = f.dist.map(m);
        f.anchors.values_mut().for_each(|a| *a = m(*a));
        f.last_col = w - 1;
    }
    f
}

fn z_of(slot: &str) -> u8 {
    match slot {
        "rim" => 1,
        "boss" | "guard" | "pommel" => 2,
        "gem" => 4,
        _ => 0,
    }
}

/// Everything derived from a weapon's parts, as integers so a snapshot stays comparable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponStats {
    pub damage: i32,
    pub speed: i32,
    /// Native px from the grip to the tip, in tenths of a tile (as `weapon.json` says `reach`).
    pub reach_tenths: i32,
    pub mass_hundredths: i32,
    pub block: i32,
    pub spell_power: i32,
    pub cost: i32,
    pub range: Option<i32>,
}

impl WeaponStats {
    pub fn reach(&self) -> f32 {
        self.reach_tenths as f32 / 10.0
    }
    pub fn mass(&self) -> f32 {
        self.mass_hundredths as f32 / 100.0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponPowers {
    pub stats: WeaponStats,
    pub abilities: Vec<Ability>,
    pub spells: Vec<Spell>,
    pub school: Option<School>,
}

/// A sealed weapon: the recipe plus what the library derives from it. Art is baked later.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponV1 {
    pub version: u8,
    pub id: String,
    pub name: String,
    pub maker: String,
    pub created: u64,
    pub role: super::Role,
    pub kind: WeaponKind,
    pub parts: Vec<WeaponPart>,
    pub stretch: Option<u16>,
    pub gem: Option<School>,
    pub paint: WeaponPaint,
    pub powers: WeaponPowers,
    pub art: Option<String>,
}

/// What a part costs at a preview scale: size is power, a part drawn bigger costs in proportion
/// (rounded up), a smaller one in proportion (rounded down).
pub fn scaled_cost(base: i32, scale: u16) -> i32 {
    (base * i32::from(scale) + if scale > 100 { 99 } else { 0 }) / 100
}

/// Python's `round`: halves to even, as the arms tools derived their numbers.
fn round_even(v: f32) -> i32 {
    let r = v.round();
    if (v - v.trunc()).abs() == 0.5 && r as i32 % 2 != 0 {
        r as i32 - v.signum() as i32
    } else {
        r as i32
    }
}

impl WeaponCreation {
    /// A fresh weapon of a type, from its first compatible part in every slot.
    pub fn of_kind(kind: WeaponKind) -> Self {
        let mut w = Self {
            id: "local-weapon".into(),
            name: "Wished blade".into(),
            maker: "You".into(),
            created: 0,
            kind,
            parts: Vec::new(),
            stretch: None,
            gem: None,
            paint: WeaponPaint::default(),
        };
        w.name = match kind {
            WeaponKind::Blade => "Wished blade",
            WeaponKind::Haft => "Wished mace",
            WeaponKind::Bow => "Wished bow",
            WeaponKind::Shield => "Wished shield",
            WeaponKind::Staff => "Wished staff",
            WeaponKind::Wand => "Wished wand",
        }
        .into();
        let defaults: &[(&str, &str)] = match kind {
            WeaponKind::Blade => &[
                ("pommel", "pommel-steel"),
                ("grip", "grip-leather"),
                ("guard", "guard-knight"),
                ("blade", "blade-longsword"),
            ],
            WeaponKind::Haft => &[
                ("pommel", "pommel-steel"),
                ("haft", "haft-short-wood"),
                ("head", "head-mace-flanged"),
            ],
            WeaponKind::Bow => &[("limb", "limb-short")],
            WeaponKind::Shield => &[
                ("face", "face-heater"),
                ("rim", "rim-heater"),
                ("boss", "boss-dome"),
            ],
            WeaponKind::Staff => &[
                ("pommel", "pommel-steel"),
                ("haft", "haft-staff-gnarled"),
                ("head", "head-staff-fire"),
            ],
            WeaponKind::Wand => &[("haft", "shaft-wand-ivory"), ("head", "head-wand-ice")],
        };
        w.parts = defaults
            .iter()
            .map(|(s, id)| WeaponPart::new(s, &format!("w5w.{id}")))
            .collect();
        if matches!(kind, WeaponKind::Staff) {
            w.stretch = Some(124);
        }
        w
    }

    /// A library weapon as a recipe (the builder's start from "pick one of the arms library").
    pub fn from_library(id: &str) -> Option<Self> {
        let k = kit();
        let lw = k.weapons.iter().find(|w| w.id == id)?;
        let kind = match lw.weapon_class.as_str() {
            "sword" | "cleaver" => WeaponKind::Blade,
            "mace" | "axe" | "hammer" | "club" | "spear" | "scythe" => WeaponKind::Haft,
            "bow" => WeaponKind::Bow,
            "shield" => WeaponKind::Shield,
            "staff" => WeaponKind::Staff,
            _ => WeaponKind::Wand,
        };
        let mut parts: Vec<WeaponPart> = Vec::new();
        let mut stretch = None;
        for rp in &lw.parts {
            let p = k.part(&rp.part_id)?;
            if kind == WeaponKind::Bow && (rp.flipped || p.slot == "grip") {
                continue;
            }
            if p.slot == "haft" {
                stretch = rp.length.map(|l| l as u16);
            }
            let slot = if p.slot == "blade" && kind == WeaponKind::Haft { "head" } else { &p.slot };
            parts.push(WeaponPart::new(slot, &p.id));
        }
        let gem = lw
            .gems
            .first()
            .and_then(|g| k.part(&g.part_id))
            .and_then(KitPart::school);
        Some(Self {
            id: lw.id.clone(),
            name: lw.name.clone(),
            maker: "You".into(),
            created: 0,
            kind,
            parts,
            stretch,
            gem,
            paint: WeaponPaint::default(),
        })
    }

    /// The chosen part's cost in `slot`, at its scale.
    pub fn part_cost(&self, slot: &str) -> Option<i32> {
        let wp = self.chosen(slot)?;
        Some(scaled_cost(kit().part(&wp.part_id)?.stat.cost, wp.scale))
    }

    pub fn chosen(&self, slot: &str) -> Option<&WeaponPart> {
        self.parts.iter().find(|p| p.slot == slot)
    }

    /// The joint class every part of the chain must share, driven by the head.
    pub fn joint_class(&self) -> u8 {
        match self.kind {
            WeaponKind::Wand => 6,
            WeaponKind::Haft => self
                .chosen("head")
                .and_then(|p| kit().part(&p.part_id))
                .map_or(8, KitPart::joint_class),
            _ => 8,
        }
    }

    fn fits(&self, slot: &str, p: &KitPart) -> bool {
        let class = self.joint_class();
        let c = p.class.as_str();
        match (self.kind, slot) {
            (WeaponKind::Blade, "pommel" | "grip" | "guard") => p.slot == slot && c == "sword",
            (WeaponKind::Blade, "blade") => p.slot == "blade" && matches!(c, "sword" | "cleaver"),
            (WeaponKind::Haft, "pommel") => {
                p.slot == "pommel" && matches!(c, "sword" | "polearm") && p.joint_class() == class
            }
            (WeaponKind::Haft, "haft") => {
                p.slot == "haft" && matches!(c, "mace" | "polearm") && p.joint_class() == class
            }
            (WeaponKind::Haft, "head") => {
                (p.slot == "head" && matches!(c, "mace" | "axe" | "hammer" | "club" | "spear"))
                    || (p.slot == "blade" && c == "scythe")
            }
            (WeaponKind::Bow, "limb") => p.slot == "limb",
            (WeaponKind::Shield, "face" | "boss") => p.slot == slot,
            (WeaponKind::Shield, "rim") => {
                p.slot == "rim"
                    && self
                        .chosen("face")
                        .and_then(|f| kit().part(&f.part_id))
                        .is_some_and(|f| f.class == p.class)
            }
            (WeaponKind::Staff, "pommel") => p.slot == "pommel" && c == "sword",
            (WeaponKind::Staff, "haft") => p.slot == "haft" && c == "staff",
            (WeaponKind::Staff, "head") => p.slot == "head" && c == "staff",
            (WeaponKind::Wand, "haft") => p.slot == "haft" && c == "wand",
            (WeaponKind::Wand, "head") => p.slot == "head" && c == "wand",
            _ => false,
        }
    }

    /// The library parts a slot may take now: only what mates, so nothing refused later.
    pub fn candidates(&self, slot: &str) -> Vec<&'static KitPart> {
        kit().parts.iter().filter(|p| self.fits(slot, p)).collect()
    }

    /// Only a shield's rim may be left empty.
    pub fn optional(&self, slot: &str) -> bool {
        self.kind == WeaponKind::Shield && slot == "rim"
    }

    pub fn equip(&mut self, slot: &str, id: Option<&str>) -> Result<(), String> {
        if !self.kind.slots().contains(&slot) {
            return Err("This weapon has no such slot".into());
        }
        let Some(id) = id else {
            if self.optional(slot) {
                self.parts.retain(|p| p.slot != slot);
                return Ok(());
            }
            return Err("This slot cannot be empty".into());
        };
        let part = kit().part(id).ok_or("Unknown part")?;
        let mut next = self.clone();
        if slot == "head" && self.kind == WeaponKind::Haft {
            // The head sets the joint class; a butt and a haft of another class would not mate.
            next.parts.retain(|p| p.slot != "head");
            next.parts.push(WeaponPart::new("head", &part.id));
            for s in ["pommel", "haft"] {
                let keep = next
                    .chosen(s)
                    .and_then(|p| kit().part(&p.part_id))
                    .is_some_and(|p| next.fits(s, p));
                if !keep {
                    let first = next.candidates(s).first().map(|p| p.id.clone());
                    next.parts.retain(|p| p.slot != s);
                    if let Some(first) = first {
                        next.parts.push(WeaponPart::new(s, &first));
                    }
                    if s == "haft" {
                        next.stretch = None;
                    }
                }
            }
        } else {
            if !next.fits(slot, part) {
                return Err("That part does not mate here".into());
            }
            next.parts.retain(|p| p.slot != slot);
            next.parts.push(WeaponPart::new(slot, &part.id));
        }
        // A shield's rim must match its face.
        if self.kind == WeaponKind::Shield && slot == "face" {
            let ok = next
                .chosen("rim")
                .and_then(|r| kit().part(&r.part_id))
                .is_some_and(|r| next.fits("rim", r));
            if !ok {
                next.parts.retain(|p| p.slot != "rim");
                if let Some(first) = next.candidates("rim").first() {
                    next.parts.push(WeaponPart::new("rim", &first.id));
                }
            }
        }
        let order = self.kind.slots();
        next.parts
            .sort_by_key(|p| order.iter().position(|s| *s == p.slot).unwrap_or(99));
        if !next.kind.allows_gem() || next.gem_host_of().is_none() {
            next.gem = None;
        }
        if next.name.starts_with("Wished ") {
            next.name = next.generic_name();
        }
        // A new part may stretch less or more than the last: keep the length it can have.
        if let (Some(len), Some((_, lo, hi))) = (next.stretch, next.natural_len()) {
            next.stretch = Some(i32::from(len).clamp(lo, hi) as u16);
        }
        *self = next;
        Ok(())
    }

    /// "Wished axe": a plain name drawn from what the weapon is, until the player names it.
    pub fn generic_name(&self) -> String {
        let tip = self
            .chosen(match self.kind {
                WeaponKind::Blade => "blade",
                WeaponKind::Haft | WeaponKind::Staff | WeaponKind::Wand => "head",
                WeaponKind::Bow => "limb",
                WeaponKind::Shield => "face",
            })
            .and_then(|p| kit().part(&p.part_id));
        let noun = match (self.kind, tip.map(|p| p.class.as_str())) {
            (WeaponKind::Blade, Some("cleaver")) => "cleaver",
            (WeaponKind::Blade, _) => "blade",
            (WeaponKind::Haft, Some(c)) if c != "polearm" => c,
            (WeaponKind::Haft, _) => "mace",
            (WeaponKind::Bow, _) => "bow",
            (WeaponKind::Shield, _) => "shield",
            (WeaponKind::Staff, _) => "staff",
            (WeaponKind::Wand, _) => "wand",
        };
        format!("Wished {noun}")
    }

    pub fn set_kind(&mut self, kind: WeaponKind) {
        if kind != self.kind {
            let paint = self.paint.clone();
            let (name, maker, id, created) =
                (self.name.clone(), self.maker.clone(), self.id.clone(), self.created);
            *self = Self::of_kind(kind);
            self.paint = paint;
            self.maker = maker;
            self.id = id;
            self.created = created;
            let generic = |n: &str| n.starts_with("Wished ");
            if !generic(&name) {
                self.name = name;
            }
        }
    }

    /// The slot whose part may stretch: a haft or shaft, or a straight blade (not a scythe).
    pub fn stretch_slot(&self) -> Option<&'static str> {
        match self.kind {
            WeaponKind::Blade => Some("blade"),
            WeaponKind::Haft | WeaponKind::Staff | WeaponKind::Wand => Some("haft"),
            _ => None,
        }
    }

    pub fn set_stretch(&mut self, len: Option<u16>) -> Result<(), String> {
        let (w, lo, hi) = self.natural_len().ok_or("This weapon has nothing to stretch")?;
        let _ = w;
        self.stretch = len.map(|l| i32::from(l).clamp(lo, hi) as u16);
        Ok(())
    }

    /// (natural length, shortest, longest) of the stretching part.
    pub fn natural_len(&self) -> Option<(i32, i32, i32)> {
        let w = kit().part(&self.chosen(self.stretch_slot()?)?.part_id)?.size[0];
        let (lo, hi) = HaftCut::bounds(w);
        Some((w, lo, hi))
    }

    /// The enchant gem's school, or the focus's own.
    pub fn school(&self) -> Option<School> {
        if self.kind.is_focus() {
            self.chosen("head")
                .and_then(|p| kit().part(&p.part_id))
                .and_then(KitPart::school)
        } else {
            self.gem
        }
    }

    /// Which placed part hosts the gem: the first with a socket among guard, head, pommel.
    fn gem_host_of(&self) -> Option<&WeaponPart> {
        ["guard", "head", "pommel"].iter().find_map(|s| {
            let p = self.chosen(s)?;
            kit().part(&p.part_id)?
                .anchors
                .contains_key("gem_socket")
                .then_some(p)
        })
    }

    pub fn gem_part(&self) -> Option<&'static KitPart> {
        let school = self.gem?;
        kit().part(&format!("w5w.gem-{}-s", school.name().to_lowercase()))
    }

    pub fn can_enchant(&self) -> bool {
        self.kind.allows_gem() && self.gem_host_of().is_some()
    }

    pub fn set_gem(&mut self, gem: Option<School>) -> Result<(), String> {
        if gem.is_some() && !self.can_enchant() {
            return Err("This weapon has no socket for a gem".into());
        }
        self.gem = gem;
        Ok(())
    }

    /// Place every part on the axis, proximal onto distal, as `assemble.py` does.
    pub fn assemble(&self) -> Option<Assembly> {
        let k = kit();
        let mut placed: Vec<(usize, (i32, i32), Fitted, bool, u16, Option<u16>, String)> = Vec::new();
        let mut joints: Vec<(i32, i32)> = Vec::new();
        match self.kind {
            WeaponKind::Bow => {
                let limb = k.part_index(&self.chosen("limb")?.part_id)?;
                let grip = k.part_index("w5w.bow-grip")?;
                let (gp, lp) = (&k.parts[grip], &k.parts[limb]);
                let g = fit(gp, false, 100, None);
                let up = fit(lp, false, 100, None);
                let low = fit(lp, true, 100, None);
                let (gd, gpx) = (g.dist?, g.prox?);
                let up_off = (gd.0 - up.prox?.0, gd.1 - up.prox?.1);
                let low_off = (gpx.0 - low.prox?.0, gpx.1 - low.prox?.1);
                joints.extend([gpx, gd]);
                placed.push((limb, low_off, low, true, 100, None, "limb".into()));
                placed.push((limb, up_off, up, false, 100, None, "limb".into()));
                placed.push((grip, (0, 0), g, false, 100, None, "grip".into()));
            }
            WeaponKind::Shield => {
                let mut base: Option<(i32, i32)> = None;
                for wp in &self.parts {
                    let ix = k.part_index(&wp.part_id)?;
                    let f = fit(&k.parts[ix], false, wp.scale, None);
                    let c = *f.anchors.get("stack_center")?;
                    let b = *base.get_or_insert(c);
                    placed.push((ix, (b.0 - c.0, b.1 - c.1), f, false, wp.scale, None, wp.slot.clone()));
                }
            }
            _ => {
                let mut prev: Option<((i32, i32), (i32, i32))> = None;
                for slot in self.kind.slots() {
                    let wp = self.chosen(slot)?;
                    let ix = k.part_index(&wp.part_id)?;
                    let len = (self.stretch_slot() == Some(*slot)).then_some(self.stretch).flatten();
                    let f = fit(&k.parts[ix], false, wp.scale, len);
                    let off = match prev {
                        None => (0, 0),
                        Some((poff, pdist)) => {
                            let j = (poff.0 + pdist.0, poff.1 + pdist.1);
                            joints.push(j);
                            let px = f.prox?;
                            (j.0 - px.0, j.1 - px.1)
                        }
                    };
                    if let Some(d) = f.dist {
                        prev = Some((off, d));
                    }
                    placed.push((ix, off, f, false, wp.scale, len, wp.slot.clone()));
                }
            }
        }
        // The enchant gem sits on its host's socket.
        let mut gem_placed = None;
        if let (Some(gp), Some(host)) = (self.gem_part(), self.gem_host_of()) {
            let h = placed.iter().find(|q| q.6 == host.slot)?;
            let sock = *h.2.anchors.get("gem_socket")?;
            let f = fit(gp, false, 100, None);
            let px = f.prox?;
            let off = (h.1 .0 + sock.0 - px.0, h.1 .1 + sock.1 - px.1);
            gem_placed = Some((k.part_index(&gp.id)?, off, f, false, 100u16, None::<u16>, "gem".to_string()));
        }
        let all = || placed.iter().chain(gem_placed.iter());
        let lo = (
            all().map(|q| q.1 .0).min()? - 2,
            all().map(|q| q.1 .1).min()? - 2,
        );
        let hi = (
            all().map(|q| q.1 .0 + q.2.size.0).max()? + 2,
            all().map(|q| q.1 .1 + q.2.size.1).max()? + 2,
        );
        let rel = |q: &(usize, (i32, i32), Fitted, bool, u16, Option<u16>, String)| (q.1 .0 - lo.0, q.1 .1 - lo.1);
        let tip_x = all().map(|q| q.1 .0 + q.2.last_col).max()? - lo.0;
        // The hand: a grip's own anchor (one hand), or a fraction along a haft (two hands).
        let two = matches!(self.kind, WeaponKind::Staff)
            || (self.kind == WeaponKind::Haft && self.joint_class() == 11);
        let gq = placed.iter().find(|q| q.6 == "grip" || q.6 == "haft");
        let (mut grip, mut grip2) = ((0, 0), None);
        if let Some(q) = gq {
            let o = rel(q);
            let (a, b) = (q.2.prox.unwrap_or((0, 0)), q.2.dist.unwrap_or((0, 0)));
            if two {
                let at = |t: f32| {
                    (
                        o.0 + (a.0 as f32 + (b.0 - a.0) as f32 * t).round() as i32,
                        o.1 + (a.1 as f32 + (b.1 - a.1) as f32 * t).round() as i32,
                    )
                };
                grip = at(0.22);
                grip2 = Some(at(0.62));
            } else if let Some(g) = q.2.anchors.get("grip") {
                grip = (o.0 + g.0, o.1 + g.1);
            } else {
                grip = (o.0 + (a.0 + b.0) / 2, o.1 + (a.1 + b.1) / 2);
            }
        }
        if self.kind == WeaponKind::Shield {
            grip = placed.first().map(|q| (rel(q).0 + q.2.anchors["stack_center"].0, rel(q).1 + q.2.anchors["stack_center"].1))?;
        }
        if self.kind == WeaponKind::Bow {
            let g = placed.iter().find(|q| q.6 == "grip")?;
            let o = rel(g);
            let a = *g.2.anchors.get("grip")?;
            grip = (o.0 + a.0, o.1 + a.1);
        }
        let cast = placed.iter().find(|q| q.6 == "head").and_then(|q| {
            let a = q.2.anchors.get("cast_anchor")?;
            let o = rel(q);
            Some((o.0 + a.0, o.1 + a.1))
        });
        let joints = joints.into_iter().map(|j| (j.0 - lo.0, j.1 - lo.1)).collect();
        let mut order: Vec<usize> = (0..placed.len()).collect();
        order.sort_by_key(|&i| (z_of(&placed[i].6), i));
        let mk = |q: &(usize, (i32, i32), Fitted, bool, u16, Option<u16>, String), gem: bool| Placed {
            kit: q.0,
            slot: q.6.clone(),
            off: rel(q),
            flipped: q.3,
            scale: q.4,
            len: q.5,
            z: z_of(&q.6),
            gem,
        };
        let mut out: Vec<Placed> = order.into_iter().map(|i| mk(&placed[i], false)).collect();
        if let Some(g) = &gem_placed {
            out.push(mk(g, true));
        }
        Some(Assembly {
            placed: out,
            size: (hi.0 - lo.0, hi.1 - lo.1),
            grip,
            grip2,
            tip_x,
            cast,
            joints,
        })
    }

    /// Stats, abilities and spells, derived from parts the way `assemble.py` derives them.
    pub fn powers(&self) -> WeaponPowers {
        let k = kit();
        let mut parts: Vec<(&KitPart, u16)> = Vec::new();
        if self.kind == WeaponKind::Bow {
            parts.push((&k.parts[k.part_index("w5w.bow-grip").unwrap_or(0)], 100));
        }
        for wp in &self.parts {
            if let Some(p) = k.part(&wp.part_id) {
                parts.push((p, wp.scale));
            }
        }
        let gem = self.gem_part();
        let assembly = self.assemble();
        let mut s = PartStat::default();
        for (p, scale) in parts.iter().chain(gem.map(|g| (g, 100)).iter()) {
            s.dmg += p.stat.dmg;
            s.spd += p.stat.spd;
            s.blk += p.stat.blk;
            s.sp += p.stat.sp;
            s.cost += scaled_cost(p.stat.cost, *scale);
        }
        // Mass counts both bow limbs; damage and cost count one.
        let mut mass: f32 = parts
            .iter()
            .map(|(p, sc)| p.mass * (f32::from(*sc) / 100.0).powi(2))
            .sum();
        if self.kind == WeaponKind::Bow {
            mass += self
                .chosen("limb")
                .and_then(|l| k.part(&l.part_id))
                .map_or(0.0, |l| l.mass);
        }
        mass += gem.map_or(0.0, |g| g.mass);
        let school = self.school();
        let reach_px = match (&assembly, self.kind) {
            (_, WeaponKind::Shield) | (None, _) => 0.0,
            (Some(a), WeaponKind::Bow) => {
                let up = a.placed.iter().find(|q| q.slot == "limb" && !q.flipped);
                let low = a.placed.iter().find(|q| q.slot == "limb" && q.flipped);
                match (up, low) {
                    (Some(u), Some(l)) => {
                        let nock = |q: &Placed| {
                            let p = &k.parts[q.kit];
                            let n = p.anchors.get("nock").map_or(0, |n| n[0]);
                            q.off.0 + if q.flipped { p.size[0] - 1 - n } else { n }
                        };
                        (nock(u) - nock(l)).abs() as f32 / 2.0
                    }
                    _ => 0.0,
                }
            }
            (Some(a), _) => (a.tip_x - a.grip.0) as f32,
        };
        let kf = if self.kind == WeaponKind::Shield { 0.18 } else { 0.45 };
        let speed = round_even(11.0 - kf * mass + s.spd as f32).clamp(1, 12);
        // `reach` is the grip-to-tip distance in tenths of a tile: ten native pixels to the tile.
        let reach_tenths = round_even(reach_px);
        let mut stats = WeaponStats {
            damage: s.dmg,
            speed,
            reach_tenths,
            mass_hundredths: (mass * 100.0).round() as i32,
            block: s.blk,
            spell_power: s.sp,
            cost: s.cost,
            range: None,
        };
        if self.kind == WeaponKind::Bow {
            stats.range = Some(round_even(stats.reach() * 1.6 + s.dmg as f32));
        }
        // Abilities: the union over parts (chain order, then the gem), a level from the whole damage.
        let level = 1 + i32::from(s.dmg >= 7) + i32::from(s.dmg >= 10);
        let mut seen: Vec<&str> = Vec::new();
        let mut abilities: Vec<Ability> = Vec::new();
        let ordered: Vec<&KitPart> = parts.iter().map(|(p, _)| *p).chain(gem).collect();
        let mut spell_part: Option<&str> = None;
        for p in &ordered {
            for a in &p.abilities {
                if seen.contains(&a.as_str()) {
                    continue;
                }
                seen.push(a);
                if a == "spell_power" {
                    spell_part.get_or_insert(&p.id);
                    continue;
                }
                if a == "enchant" {
                    continue;
                }
                abilities.push(Ability {
                    id: a.clone(),
                    from_part: p.id.clone(),
                    level: level as u8,
                });
            }
        }
        let mut spells = Vec::new();
        if self.kind.is_focus() && let Some(school) = school {
            let power = 1 + i32::from(s.sp >= 7) + i32::from(s.sp >= 10);
            spells = self
                .kind
                .forms()
                .iter()
                .map(|&form| Spell {
                    school,
                    form,
                    element: school,
                    power: power as u8,
                })
                .collect();
            abilities.push(Ability {
                id: "spell_power".into(),
                from_part: spell_part.unwrap_or("w5w.head").into(),
                level: power as u8,
            });
        } else if let Some(school) = school
            && !seen.contains(&format!("enchant:{}", school.name().to_lowercase()).as_str())
        {
            abilities.push(Ability {
                id: format!("enchant:{}", school.name().to_lowercase()),
                from_part: gem.map_or_else(String::new, |g| g.id.clone()),
                level: (1 + i32::from(s.sp >= 2)) as u8,
            });
        }
        WeaponPowers {
            stats,
            abilities,
            spells,
            school,
        }
    }

    /// Budget points spent (the parts' costs).
    pub fn spent(&self) -> u16 {
        self.powers().stats.cost.max(0) as u16
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.chars().count() > 24 {
            return Err("Give it a name of 1-24 letters".into());
        }
        for slot in self.kind.slots() {
            match self.chosen(slot) {
                None if self.optional(slot) => {}
                None => return Err(format!("It needs a {}", slot_label(slot, self.kind, self.joint_class()).to_lowercase())),
                Some(wp) => {
                    let p = kit().part(&wp.part_id).ok_or("Unknown part")?;
                    if !self.fits(slot, p) {
                        return Err("A part does not mate its neighbour".into());
                    }
                    if !(50..=200).contains(&wp.scale) {
                        return Err("Part scale is out of range".into());
                    }
                }
            }
        }
        if self.parts.len() > self.kind.slots().len() {
            return Err("Too many parts".into());
        }
        if self.gem.is_some() && !self.can_enchant() {
            return Err("This weapon has no socket for a gem".into());
        }
        if let (Some(len), Some((_, lo, hi))) = (self.stretch, self.natural_len())
            && !(lo..=hi).contains(&i32::from(len))
        {
            return Err("The haft length is out of range".into());
        }
        if self.assemble().is_none() {
            return Err("The parts do not assemble".into());
        }
        if self.spent() > WEAPON_BUDGET {
            return Err(format!(
                "{} over budget; trim parts or the gem",
                self.spent() - WEAPON_BUDGET
            ));
        }
        Ok(())
    }

    pub fn seal(&self) -> Result<WeaponV1, String> {
        self.validate()?;
        Ok(WeaponV1 {
            version: WEAPON_VERSION,
            id: self.id.clone(),
            name: self.name.clone(),
            maker: self.maker.clone(),
            created: self.created,
            role: super::Role::Weapon,
            kind: self.kind,
            parts: self.parts.clone(),
            stretch: self.stretch,
            gem: self.gem,
            paint: self.paint.clone(),
            powers: self.powers(),
            art: None,
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/weapon__tests.rs"]
mod tests;
