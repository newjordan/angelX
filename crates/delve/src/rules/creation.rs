//! A creature's editable recipe. No Run, equipment, renderer, IO or job runner.
//!
//! The first plan is an upright biped. A draft may exceed its role's budget
//! while being edited; only a validated draft can be sealed. The v3 snapshot
//! derives powers and stats afresh, so neither can be supplied by the editor.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub mod drive;
mod session;
mod weapon;
pub mod wish;
pub use drive::{Act, Blow, Drive, DriveInput, Ground, Hurt, Pose, Shot, Timing, Way};
pub use session::{Action, Chat, ChatLine, Session, Step, Who};
pub use weapon::*;

pub const VERSION: u8 = 3;
pub const MAX_PARTS: usize = 6;
pub const MAX_SPELLS: usize = 2;
pub const MAX_ATLAS_SIDE: u16 = 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    Hero,
    Minion,
    Monster,
    Boss,
    /// Weapons are a creation class of their own (SPEC-CREATOR 1b): never a creature's anatomy.
    Weapon,
    /// Someone for the world itself (2026-11-07, the rite): a barman, a smith, a pilgrim. It stands or
    /// wanders where its maker puts it and says its line (`Calling`); it is no fighter.
    Folk,
}

impl Role {
    pub const ALL: [Self; 6] = [
        Self::Hero,
        Self::Minion,
        Self::Monster,
        Self::Boss,
        Self::Weapon,
        Self::Folk,
    ];
    pub const CREATURES: [Self; 5] = [Self::Hero, Self::Minion, Self::Monster, Self::Boss, Self::Folk];
    pub fn name(self) -> &'static str {
        match self {
            Self::Hero => "Hero",
            Self::Minion => "Minion",
            Self::Monster => "Monster",
            Self::Boss => "Boss",
            Self::Weapon => "Weapon",
            Self::Folk => "Folk",
        }
    }
    pub fn budget(self) -> u16 {
        match self {
            Self::Hero => 50,
            Self::Minion => 20,
            Self::Monster => 40,
            Self::Boss => 120,
            Self::Weapon => WEAPON_BUDGET,
            Self::Folk => 15,
        }
    }
    pub fn purpose(self) -> &'static str {
        match self {
            Self::Hero => "A soul for your adventures",
            Self::Minion => "A little friend to summon",
            Self::Monster => "A creature of the dungeon",
            Self::Boss => "A legend worth facing",
            Self::Weapon => "A blade, a staff, a shield",
            Self::Folk => "A barman, a smith, a pilgrim",
        }
    }
    pub fn is_weapon(self) -> bool {
        self == Self::Weapon
    }
    /// The looks this role can wear: the art that shows a creature until its own bake exists. Only a
    /// Hero has any yet; every other role shows the stand-in boss.
    pub fn looks(self) -> &'static [Look] {
        match self {
            // A Folk is a person of the world: it wears the Hero's looks (a barman in a knight's
            // body is not an ogre) until the Creator has looks of its own for it.
            Self::Hero | Self::Folk => &HERO_LOOKS,
            _ => &[],
        }
    }
}

/// One look of a role: the baked set the renderer's registry serves under this id
/// (`render/creation/baked.rs`, `REGISTRY`; a test holds the two tables together) and what the
/// scroll calls it. The first look of a role is its default, the one a recipe with no look wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub id: &'static str,
    pub name: &'static str,
}

/// The Hero's looks: the 2D knight (the T-pose rig route, test data from lane H), the A6 crusader (the
/// night pipeline's finished knight: eight ways, every clip) and the frog knight (a wish carried through
/// the same pipeline).
pub const HERO_LOOKS: [Look; 3] = [
    Look { id: "knight", name: "Knight (2D)" },
    Look { id: "a6-crusader", name: "Crusader (A6)" },
    Look { id: "frog-knight", name: "Frog Knight" },
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    #[default]
    BipedUpright,
}

/// Sliders are integer percentages (0..100); their neutral point is 50.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sculpt {
    pub height: u8,
    pub bulk: u8,
    pub limb_len: u8,
    pub limb_thick: u8,
    pub head_size: u8,
    pub spine: [SpineSegment; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpineSegment {
    pub length: u8,
    pub bend: i8,
}

impl Default for Sculpt {
    fn default() -> Self {
        Self {
            height: 50,
            bulk: 50,
            limb_len: 50,
            limb_thick: 50,
            head_size: 50,
            spine: [SpineSegment {
                length: 50,
                bend: 0,
            }; 3],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Head,
    Horns,
    Armour,
    Hands,
    Tail,
    Charm,
}

impl Slot {
    pub const ALL: [Self; 6] = [
        Self::Head,
        Self::Horns,
        Self::Armour,
        Self::Hands,
        Self::Tail,
        Self::Charm,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Horns => "Horns",
            Self::Armour => "Armour",
            Self::Hands => "Hands",
            Self::Tail => "Tail",
            Self::Charm => "Charm",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pigment {
    #[default]
    Ochre,
    Moss,
    Slate,
    Russet,
    Bone,
    Charcoal,
}

impl Pigment {
    pub const ALL: [Self; 6] = [
        Self::Ochre,
        Self::Moss,
        Self::Slate,
        Self::Russet,
        Self::Bone,
        Self::Charcoal,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ochre => "Ochre",
            Self::Moss => "Moss",
            Self::Slate => "Slate",
            Self::Russet => "Russet",
            Self::Bone => "Bone",
            Self::Charcoal => "Charcoal",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    #[default]
    Plain,
    Bands,
    Speckles,
}

impl Pattern {
    pub const ALL: [Self; 3] = [Self::Plain, Self::Bands, Self::Speckles];
    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "Plain",
            Self::Bands => "Bands",
            Self::Speckles => "Speckles",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paint {
    pub scheme: Pigment,
    pub accent: Pigment,
    pub pattern: Pattern,
    pub overrides: BTreeMap<Slot, Pigment>,
}

impl Default for Paint {
    fn default() -> Self {
        Self {
            scheme: Pigment::Ochre,
            accent: Pigment::Russet,
            pattern: Pattern::Plain,
            overrides: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartChoice {
    pub part_id: String,
    pub mount: Slot,
    /// Degrees and percentage, bounded by validation before a bake.
    pub angle: i16,
    pub scale: u16,
    pub mirrored: bool,
    pub paint_override: Option<Pigment>,
}

impl PartChoice {
    pub fn of(part: &Part) -> Self {
        Self {
            part_id: part.id.into(),
            mount: part.slot,
            angle: 0,
            scale: 100,
            mirrored: true,
            paint_override: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub hp: u16,
    pub speed: u16,
    pub melee: u16,
    pub magic: u16,
    pub armour: u16,
}

pub struct Part {
    pub id: &'static str,
    pub name: &'static str,
    pub slot: Slot,
    pub cost: u16,
    pub ability: Option<(&'static str, u8)>,
    pub stats: Stats,
}

const fn stats(hp: u16, melee: u16, magic: u16, armour: u16) -> Stats {
    Stats {
        hp,
        speed: 0,
        melee,
        magic,
        armour,
    }
}

/// Small, deliberately authored catalogue. IDs are stable recipe references.
/// Hands are natural anatomy. Held weapons are a separate creation class
/// (SPEC-CREATOR §1b), attached at runtime through a baked grip handle.
pub const PARTS: &[Part] = &[
    Part {
        id: "ember-mask",
        name: "Ember mask",
        slot: Slot::Head,
        cost: 3,
        ability: None,
        stats: stats(4, 0, 1, 0),
    },
    Part {
        id: "stone-brow",
        name: "Stone brow",
        slot: Slot::Head,
        cost: 4,
        ability: None,
        stats: stats(8, 0, 0, 1),
    },
    Part {
        id: "seer-eye",
        name: "Seer's eye",
        slot: Slot::Head,
        cost: 7,
        ability: Some(("second_sight", 1)),
        stats: stats(0, 0, 4, 0),
    },
    Part {
        id: "ram-horns",
        name: "Ram horns",
        slot: Slot::Horns,
        cost: 6,
        ability: Some(("charge", 1)),
        stats: stats(0, 3, 0, 0),
    },
    Part {
        id: "crown-horns",
        name: "Crown horns",
        slot: Slot::Horns,
        cost: 12,
        ability: Some(("charge", 2)),
        stats: stats(6, 5, 0, 0),
    },
    Part {
        id: "iron-plate",
        name: "Iron plate",
        slot: Slot::Armour,
        cost: 6,
        ability: None,
        stats: stats(8, 0, 0, 4),
    },
    Part {
        id: "bark-mail",
        name: "Bark mail",
        slot: Slot::Armour,
        cost: 4,
        ability: Some(("root_guard", 1)),
        stats: stats(6, 0, 0, 2),
    },
    Part {
        id: "royal-mail",
        name: "Royal mail",
        slot: Slot::Armour,
        cost: 18,
        ability: Some(("bulwark", 2)),
        stats: stats(20, 0, 0, 9),
    },
    Part {
        id: "raking-claws",
        name: "Raking claws",
        slot: Slot::Hands,
        cost: 5,
        ability: Some(("slash", 1)),
        stats: stats(0, 4, 0, 0),
    },
    Part {
        id: "ember-palms",
        name: "Ember palms",
        slot: Slot::Hands,
        cost: 8,
        ability: Some(("spellcraft", 1)),
        stats: stats(0, 1, 5, 0),
    },
    Part {
        id: "stone-fists",
        name: "Stone fists",
        slot: Slot::Hands,
        cost: 16,
        ability: Some(("slam", 2)),
        stats: stats(0, 10, 0, 0),
    },
    Part {
        id: "whip-tail",
        name: "Whip tail",
        slot: Slot::Tail,
        cost: 5,
        ability: Some(("sweep", 1)),
        stats: stats(0, 2, 0, 0),
    },
    Part {
        id: "lantern-tail",
        name: "Lantern tail",
        slot: Slot::Tail,
        cost: 8,
        ability: Some(("lantern", 1)),
        stats: stats(0, 0, 3, 0),
    },
    Part {
        id: "wish-bell",
        name: "Wish bell",
        slot: Slot::Charm,
        cost: 4,
        ability: Some(("rally", 1)),
        stats: stats(0, 0, 2, 0),
    },
    Part {
        id: "soul-ember",
        name: "Soul ember",
        slot: Slot::Charm,
        cost: 10,
        ability: Some(("rekindle", 1)),
        stats: stats(10, 0, 3, 0),
    },
];

pub fn part(id: &str) -> Option<&'static Part> {
    PARTS.iter().find(|p| p.id == id)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum School {
    #[default]
    Fire,
    Ice,
    Lightning,
    Arcane,
    Holy,
    Shadow,
    Nature,
}

impl School {
    pub const ALL: [Self; 7] = [
        Self::Fire,
        Self::Ice,
        Self::Lightning,
        Self::Arcane,
        Self::Holy,
        Self::Shadow,
        Self::Nature,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Fire => "Fire",
            Self::Ice => "Ice",
            Self::Lightning => "Lightning",
            Self::Arcane => "Arcane",
            Self::Holy => "Holy",
            Self::Shadow => "Shadow",
            Self::Nature => "Nature",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    #[default]
    Bolt,
    Nova,
    Beam,
    Ward,
    Summon,
    Aura,
}

impl Form {
    pub const ALL: [Self; 6] = [
        Self::Bolt,
        Self::Nova,
        Self::Beam,
        Self::Ward,
        Self::Summon,
        Self::Aura,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Bolt => "Bolt",
            Self::Nova => "Nova",
            Self::Beam => "Beam",
            Self::Ward => "Ward",
            Self::Summon => "Summon",
            Self::Aura => "Aura",
        }
    }
    pub fn cost(self) -> u16 {
        match self {
            Self::Bolt => 4,
            Self::Nova => 7,
            Self::Beam => 6,
            Self::Ward => 5,
            Self::Summon => 10,
            Self::Aura => 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spell {
    pub school: School,
    pub form: Form,
    pub element: School,
    pub power: u8,
}

impl Default for Spell {
    fn default() -> Self {
        Self {
            school: School::Fire,
            form: Form::Bolt,
            element: School::Fire,
            power: 1,
        }
    }
}

impl Spell {
    pub fn cost(&self) -> u16 {
        self.form.cost() * u16::from(self.power)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ability {
    pub id: String,
    pub from_part: String,
    pub level: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Powers {
    pub abilities: Vec<Ability>,
    pub spells: Vec<Spell>,
}

/// Where a made Folk stands in the world: the Sozzled Griffin's taproom, or the Delve's courtyard
/// (the Rite Wall's lawn), where the whole party walks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Post {
    #[default]
    Griffin,
    Courtyard,
}

impl Post {
    pub const ALL: [Self; 2] = [Self::Griffin, Self::Courtyard];
    pub fn name(self) -> &'static str {
        match self {
            Self::Griffin => "Sozzled Griffin",
            Self::Courtyard => "Courtyard",
        }
    }
}

/// What a made Folk does where it stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Habit {
    #[default]
    Stands,
    Wanders,
}

impl Habit {
    pub const ALL: [Self; 2] = [Self::Stands, Self::Wanders];
    pub fn name(self) -> &'static str {
        match self {
            Self::Stands => "Stands",
            Self::Wanders => "Wanders",
        }
    }
}

/// The lines a Folk can say on E, picked on the Calling page; a wish may put its own in (`LINE_MAX`).
pub const FOLK_LINES: [&str; 6] = [
    "Welcome, traveller. Mind the step.",
    "The delve gives, and the delve takes.",
    "Another round for the brave!",
    "I saw lights down the old stair last night.",
    "Come back with all your fingers.",
    "Nothing to see here. Everything to hear.",
];
pub const LINE_MAX: usize = 60;

/// A Folk's own few behaviours (SPEC-CREATOR, the rite): where it is, whether it stays put, what it
/// says. Other roles carry the plain default and never use it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calling {
    pub post: Post,
    pub habit: Habit,
    pub line: String,
}

impl Default for Calling {
    fn default() -> Self {
        Self { post: Post::Griffin, habit: Habit::Stands, line: FOLK_LINES[0].into() }
    }
}

impl Calling {
    fn plain(&self) -> bool {
        *self == Self::default()
    }
}

/// The editor owns choices only. Derived combat data is made at the boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Creation {
    pub id: String,
    pub name: String,
    pub maker: String,
    /// Host-provided creation time. The offline editor uses a logical tick.
    pub created: u64,
    pub role: Role,
    pub plan: Plan,
    pub sculpt: Sculpt,
    pub parts: Vec<PartChoice>,
    pub paint: Paint,
    pub spells: Vec<Spell>,
    /// The art id that shows this creation while it has no bake of its own, one of its role's
    /// looks (`Role::looks`). `None` wears the role's first look, so a plain recipe is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    /// A Folk's post, habit and line.
    #[serde(default, skip_serializing_if = "Calling::plain")]
    pub calling: Calling,
}

impl Default for Creation {
    fn default() -> Self {
        Self {
            id: "local-emberkin".into(),
            name: "Emberkin".into(),
            maker: "You".into(),
            created: 0,
            role: Role::Hero,
            plan: Plan::BipedUpright,
            sculpt: Sculpt::default(),
            parts: ["ember-mask", "iron-plate", "raking-claws"]
                .map(|id| PartChoice::of(part(id).unwrap()))
                .into(),
            paint: Paint::default(),
            spells: Vec::new(),
            look: None,
            calling: Calling::default(),
        }
    }
}

/// A v3 recipe for the future bake boundary. No atlas or gate is invented:
/// absent art and motions explicitly mean this is still an unbaked creation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureV3 {
    pub version: u8,
    pub id: String,
    pub name: String,
    pub maker: String,
    pub created: u64,
    pub role: Role,
    pub plan: Plan,
    pub sculpt: Sculpt,
    pub parts: Vec<PartChoice>,
    pub paint: Paint,
    pub powers: Powers,
    pub stats: Stats,
    pub art: Option<Art>,
    pub motions: BTreeMap<String, String>,
    pub provenance: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Art {
    pub atlas: String,
    pub atlas_json: String,
    pub normal: Option<String>,
    pub material: Option<String>,
}

impl Creation {
    pub fn chosen(&self, slot: Slot) -> Option<&PartChoice> {
        self.parts.iter().find(|p| p.mount == slot)
    }

    /// The looks this creation's role can wear (none for the roles that show the stand-in boss).
    pub fn looks(&self) -> &'static [Look] {
        self.role.looks()
    }

    /// Which of its role's looks it wears now: its own if the role has it, else the first.
    pub fn look_at(&self) -> usize {
        self.look
            .as_deref()
            .and_then(|id| self.looks().iter().position(|l| l.id == id))
            .unwrap_or(0)
    }

    /// Wear the role's `n`th look; false (and nothing changes) if the role has no such look. The
    /// first look is the default, so wearing it clears the field and a plain recipe stays plain.
    pub fn set_look(&mut self, n: usize) -> bool {
        let Some(look) = self.looks().get(n) else {
            return false;
        };
        self.look = (n > 0).then(|| look.id.to_owned());
        true
    }

    /// Replacing a mount cannot leave duplicate hands or an old part's power.
    pub fn equip(&mut self, slot: Slot, id: Option<&str>) -> Result<(), String> {
        let chosen = id
            .map(|id| {
                part(id)
                    .filter(|p| p.slot == slot)
                    .ok_or_else(|| "Part does not fit this mount".to_string())
            })
            .transpose()?;
        self.parts.retain(|p| p.mount != slot);
        if let Some(p) = chosen {
            self.parts.push(PartChoice::of(p));
        }
        self.parts.sort_by_key(|p| p.mount);
        Ok(())
    }

    pub fn spent(&self) -> u16 {
        self.parts
            .iter()
            .filter_map(|p| part(&p.part_id))
            .map(|p| p.cost)
            .chain(self.spells.iter().map(Spell::cost))
            .fold(0u16, u16::saturating_add)
    }

    pub fn powers(&self) -> Powers {
        Powers {
            abilities: self
                .parts
                .iter()
                .filter_map(|choice| {
                    let p = part(&choice.part_id).filter(|p| p.slot == choice.mount)?;
                    let (id, level) = p.ability?;
                    Some(Ability {
                        id: id.into(),
                        from_part: p.id.into(),
                        level,
                    })
                })
                .collect(),
            spells: self.spells.clone(),
        }
    }

    pub fn stats(&self) -> Stats {
        let hp = match self.role {
            Role::Minion => 24,
            Role::Hero => 60,
            Role::Monster => 45,
            Role::Boss => 180,
            Role::Folk => 30,
            Role::Weapon => 0,
        };
        let mut s = Stats {
            hp,
            speed: 100,
            melee: 2,
            magic: 0,
            armour: 0,
        };
        for p in self
            .parts
            .iter()
            .filter_map(|c| part(&c.part_id).filter(|p| p.slot == c.mount))
        {
            s.hp = s.hp.saturating_add(p.stats.hp);
            s.melee = s.melee.saturating_add(p.stats.melee);
            s.magic = s.magic.saturating_add(p.stats.magic);
            s.armour = s.armour.saturating_add(p.stats.armour);
        }
        s.hp =
            s.hp.saturating_add(u16::from(self.sculpt.bulk.min(100)) / 10);
        s.speed = 110 - u16::from(self.sculpt.bulk.min(100)) / 5;
        s
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.role.is_weapon() {
            return Err("Weapons are made at the forge, not on a creature".into());
        }
        if self.name.trim().is_empty() || self.name.chars().count() > 24 {
            return Err("Give it a name of 1-24 letters".into());
        }
        let s = &self.sculpt;
        if [s.height, s.bulk, s.limb_len, s.limb_thick, s.head_size]
            .iter()
            .any(|&v| v > 100)
            || s.spine
                .iter()
                .any(|s| s.length > 100 || !(-30..=30).contains(&s.bend))
        {
            return Err("Body sliders must stay in range".into());
        }
        if self.parts.len() > MAX_PARTS || self.spells.len() > MAX_SPELLS {
            return Err("Too many parts or spells".into());
        }
        let mut slots = BTreeSet::new();
        for choice in &self.parts {
            let Some(p) = part(&choice.part_id) else {
                return Err("Unknown part".into());
            };
            if p.slot != choice.mount || !slots.insert(choice.mount) {
                return Err("One fitting part per mount".into());
            }
            if !(-45..=45).contains(&choice.angle) || !(50..=150).contains(&choice.scale) {
                return Err("Part transform is out of range".into());
            }
        }
        if !slots.contains(&Slot::Head) {
            return Err("Your creature needs a head".into());
        }
        if self.role == Role::Folk
            && (self.calling.line.trim().is_empty()
                || self.calling.line.chars().count() > LINE_MAX
                || self.calling.line.chars().any(char::is_control)
                || !self.spells.is_empty())
        {
            return Err("A folk needs a line to say, and no spells".into());
        }
        if self.spells.iter().any(|s| !(1..=3).contains(&s.power)) {
            return Err("Spell power must be 1-3".into());
        }
        if self.spent() > self.role.budget() {
            return Err(format!(
                "{} over budget; trim parts or magic",
                self.spent() - self.role.budget()
            ));
        }
        Ok(())
    }

    pub fn seal(&self) -> Result<CreatureV3, String> {
        self.validate()?;
        Ok(CreatureV3 {
            version: VERSION,
            id: self.id.clone(),
            name: self.name.clone(),
            maker: self.maker.clone(),
            created: self.created,
            role: self.role,
            plan: self.plan,
            sculpt: self.sculpt.clone(),
            parts: self.parts.clone(),
            paint: self.paint.clone(),
            powers: self.powers(),
            stats: self.stats(),
            art: None,
            motions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        })
    }
}

#[cfg(test)]
#[path = "../../tests/rules/creation__tests.rs"]
mod tests;
