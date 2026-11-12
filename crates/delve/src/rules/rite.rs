//! The rite of passage (the operator, 2026-11-07: "when a new player joins
//! the game, they have to make a new weapon or minion as a right of passage",
//! "every time you play you add to the world", and then: "they use the new
//! menu we built [the Creator]. and dont restrict them to character
//! selection, and still start in the bar, this would just be a pop up when
//! loading in").
//!
//! A new knight (one the realm has no rite and no record for: no prowess, no
//! kept gear) owes the realm one thing of their own making. They arrive in the
//! Sozzled Griffin as ever; their screen opens a short invitation to the
//! Creator (`client::creation`), which they can put off and open again with
//! one key. Nothing is held for it: no stair is shut, no party waits, no wish
//! is spent, no one is made to choose a hero. Every builder is open, a
//! weapon, a minion, a monster, a boss, a hero, and Folk, a new role for the
//! world's own people (a barman, a smith, a pilgrim).
//!
//! **The realm keeps it** ([`Rites`], `Home::rites`, appended and read
//! leniently, so older saves load): the maker, what it is ([`Kind`]), the
//! sealed recipe ([`Made`]), its name and the day. Nothing leaves the record.
//! A made thing is also taken into the world, credited to its maker, within
//! bounds; the oldest roll off the display, never off the record:
//!
//! - a **weapon** hangs from the Rite Wall in the Delve's courtyard
//!   (`render::overworld::arena::rite_wall`) and lies in the delve's loot and
//!   chests under the maker's name ([`Run::stock_rites`]);
//! - a **minion** turns up on a delve floor as the party's friendly ally
//!   (`made::meet`);
//! - a **monster** or a **boss** joins the foes of a delve floor with its
//!   maker's name on its health bar (`made::meet`), bounded by its budget;
//! - a **folk** stands, or wanders, at its post (the Griffin's taproom or the
//!   courtyard) and says its line on E;
//! - a **hero** is a knight of the realm who walks the courtyard and says a
//!   line on E.
//!
//! Anyone, new or old, may open the Creator and contribute again (up to
//! [`MOST_PER_KNIGHT`]); only a new name owes a first.

use super::creation::{Creation, Role, WeaponCreation, WeaponKind};
use super::*;

/// Whether the game asks a new knight for a rite: on unless
/// `ANGEL_DELVE_RITE=0`. The game sets `Run::rite` from it; bare runs keep it
/// off. (It was off by default for a few hours of 2026-11-07, until the
/// Creator's pop-up replaced the wish scroll's rite and its stair block.)
pub fn on() -> bool {
    std::env::var_os("ANGEL_DELVE_RITE").is_none_or(|v| v != "0")
}

/// The action kind that carries a made thing from the seat to its host (`realm-host`'s `together_gear`).
pub const ACTION: u32 = 10;

/// What the invitation says: the Creator's own scroll language.
pub const INVITE: &str = "YOUR RITE: MAKE SOMETHING FOR THE REALM";
/// What a new knight is told as they arrive.
pub const TOLD: &str = "Your rite: make something for the realm. Press C.";

/// The most the display shows of each kind at once (the newest): the wall's
/// pegs, a place's folk, a floor's friends and foes. Older ones roll off the
/// display, never from the record.
pub const MOST_WEAPONS: usize = 5;
pub const MOST_MINIONS: usize = 6;
pub const MOST_FOLK: usize = 4;
pub const MOST_HEROES: usize = 3;
/// The most made foes a delve floor meets: monsters and bosses.
pub const MOST_FOES: usize = 2;
/// The most rites the record holds (a save's bound, far past any realm's
/// players), and the most one knight may add.
pub const MOST_KEPT: usize = 2048;
pub const MOST_PER_KNIGHT: usize = 6;
/// The longest words kept with a rite.
pub const WORDS_MAX: usize = 120;
/// How long the grant's flourish shows, in ticks.
pub const FLOURISH_TICKS: u64 = 5 * HZ as u64 / 2;
/// What a rite weapon's loot copy weighs in a drop and a chest (a card's usual).
const LOOT_DROP: u32 = 2;
const LOOT_CHEST: u32 = 3;
/// A made foe's health: its recipe's health, but never past these (a
/// monster's budget is 40 power, a boss's 120).
const MONSTER_HP: u32 = 140;
const BOSS_HP: u32 = 900;

/// What a rite made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Weapon,
    Minion,
    Monster,
    Boss,
    Hero,
    Folk,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Weapon => "weapon",
            Kind::Minion => "minion",
            Kind::Monster => "monster",
            Kind::Boss => "boss",
            Kind::Hero => "hero",
            Kind::Folk => "folk",
        }
    }

    pub fn of_role(role: Role) -> Kind {
        match role {
            Role::Weapon => Kind::Weapon,
            Role::Minion => Kind::Minion,
            Role::Monster => Kind::Monster,
            Role::Boss => Kind::Boss,
            Role::Hero => Kind::Hero,
            Role::Folk => Kind::Folk,
        }
    }
}

/// The sealed recipe of what was made, as the Creator drew it: the renderer
/// draws a creature or a weapon from it, anywhere.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Made {
    Creature(Creation),
    Weapon(WeaponCreation),
}

impl Made {
    pub fn kind(&self) -> Kind {
        match self {
            Made::Creature(c) => Kind::of_role(c.role),
            Made::Weapon(_) => Kind::Weapon,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Made::Creature(c) => &c.name,
            Made::Weapon(w) => &w.name,
        }
    }

    /// Whether it is a thing the Creator would seal: within its budget, in its ranges.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Made::Creature(c) => {
                if c.role.is_weapon() {
                    return Err("a weapon is made at the forge".into());
                }
                c.validate()
            }
            Made::Weapon(w) => w.validate(),
        }
    }

    /// A line for the wall's tag and the herald: what it is made of.
    pub fn summary(&self) -> String {
        match self {
            Made::Creature(c) if c.role == Role::Folk => c.calling.line.clone(),
            Made::Creature(c) => {
                let parts: Vec<&str> = c.parts.iter().filter_map(|p| super::creation::part(&p.part_id)).map(|p| p.name).collect();
                format!("{} of {} power: {}", c.role.name(), c.spent(), parts.join(", "))
            }
            Made::Weapon(w) => format!("A {} of {} power", w.kind.name().to_lowercase(), w.spent()),
        }
    }

    fn credit(&mut self, maker: &str, day: u32) {
        match self {
            Made::Creature(c) => {
                c.maker = maker.to_string();
                c.created = u64::from(day);
            }
            Made::Weapon(w) => {
                w.maker = maker.to_string();
                w.created = u64::from(day);
            }
        }
    }
}

/// One knight's rite, as the realm keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rite {
    /// The maker.
    pub knight: String,
    pub kind: Kind,
    /// What it says of itself: a folk's line, a creature's parts.
    #[serde(default)]
    pub words: String,
    /// A wish scroll's card (a rite before the Creator, 2026-11-07); none now.
    #[serde(default)]
    pub card: String,
    /// The piece's name.
    #[serde(default)]
    pub name: String,
    /// The realm's day it was made on.
    #[serde(default)]
    pub day: u32,
    /// The sealed recipe the Creator made (none for a wish scroll's card).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub made: Option<Made>,
}

impl Rite {
    fn valid(&self) -> bool {
        !self.knight.is_empty()
            && self.knight.len() <= 128
            && self.words.len() <= 4 * WORDS_MAX
            && self.card.len() <= 128
            && self.name.len() <= 128
            && (!self.card.is_empty() || self.made.is_some())
            && self.made.as_ref().is_none_or(|m| m.kind() == self.kind && m.validate().is_ok())
    }

    /// The made creature, if this is one.
    pub fn creature(&self) -> Option<&Creation> {
        match &self.made {
            Some(Made::Creature(c)) => Some(c),
            _ => None,
        }
    }

    /// The made weapon, if this is one.
    pub fn weapon(&self) -> Option<&WeaponCreation> {
        match &self.made {
            Some(Made::Weapon(w)) => Some(w),
            _ => None,
        }
    }
}

/// Every rite of the realm, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rites {
    #[serde(default)]
    pub kept: Vec<Rite>,
}

impl Rites {
    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    /// Whether the realm has a rite for `knight` (names are not case-bound).
    pub fn has(&self, knight: &str) -> bool {
        self.kept.iter().any(|r| r.knight.eq_ignore_ascii_case(knight))
    }

    /// How many `knight` has made.
    pub fn count_of(&self, knight: &str) -> usize {
        self.kept.iter().filter(|r| r.knight.eq_ignore_ascii_case(knight)).count()
    }

    /// The newest `most` rites of a kind with their place in the record,
    /// oldest first: what the display shows. The rest roll off the display,
    /// never the record.
    pub fn newest(&self, kind: Kind, most: usize) -> Vec<(usize, &Rite)> {
        let mut all: Vec<(usize, &Rite)> = self.kept.iter().enumerate().filter(|(_, r)| r.kind == kind).collect();
        all.drain(..all.len().saturating_sub(most));
        all
    }

    /// Keep a rite while there is room for it and its maker.
    pub fn record(&mut self, rite: Rite) -> bool {
        if self.count_of(&rite.knight) >= MOST_PER_KNIGHT || self.kept.len() >= MOST_KEPT || !rite.valid() {
            return false;
        }
        self.kept.push(rite);
        true
    }

    /// Take in the rites of `other` this record has not got (a run's and a
    /// realm's records meet): true when any was new.
    pub fn merge(&mut self, other: &Rites) -> bool {
        let mut changed = false;
        for rite in &other.kept {
            if !self.kept.contains(rite) {
                changed |= self.record(rite.clone());
            }
        }
        changed
    }

    /// Whether the record is one a save may carry.
    pub fn valid(&self) -> bool {
        self.kept.len() <= MOST_KEPT && self.kept.iter().all(Rite::valid)
    }
}

/// The id a rite weapon's loot copy has in the book: its place in the record.
pub fn loot_id(index: usize) -> String {
    format!("rite-{index}")
}

/// The card a made weapon is, in the loot: its kind's plain card (the arm or
/// guard the game already has), under the maker's name.
fn base_card(kind: WeaponKind) -> &'static str {
    match kind {
        WeaponKind::Blade => "knights-blade",
        WeaponKind::Haft => "great-maul",
        WeaponKind::Bow => "bow",
        WeaponKind::Shield => "kite-shield",
        WeaponKind::Staff => "spark-staff",
        WeaponKind::Wand => "sigil-staff",
    }
}

/// A made foe's health: its recipe's, bounded by its role.
pub fn made_hp(creation: &Creation) -> u32 {
    let cap = if creation.role == Role::Boss { BOSS_HP } else { MONSTER_HP };
    u32::from(creation.stats().hp).clamp(10, cap)
}

impl Run {
    /// Whether knight `id` owes the realm their first rite and the game asks it.
    pub fn owes_rite(&self, id: u32) -> bool {
        self.rite && self.players.get(&id).is_some_and(|h| h.rite_owed)
    }

    /// Knight `id` has just arrived (or taken a new name): if the realm has
    /// no rite for them, they owe one. Their screen opens the Creator's
    /// invitation; nothing else changes for them.
    pub(super) fn owe_rite(&mut self, id: u32) {
        if !self.rite || self.identity {
            return;
        }
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        if hero.absent || hero.rite_owed || self.home.rites.has(&hero.name) {
            return;
        }
        // Only a new player owes it (the operator: "when a new player joins
        // the game"): a knight the realm already knows, by their prowess or
        // their kept gear, came before the rite and is not asked.
        if self.home.knights.contains_key(&talents::profile_key(hero))
            || self.home.wardrobe.knights.contains_key(&hero.name)
        {
            return;
        }
        hero.rite_owed = true;
        hero.rite_at = 0;
        let name = hero.name.to_uppercase();
        self.tell(id, TOLD);
        if self.notices.len() < 8 {
            self.notices.push(format!("{name} makes their rite: something for the realm."));
        }
        self.cues.push("rite_owed".into());
    }

    /// The rite's upkeep, each tick: a name the realm has a rite for owes none.
    pub(super) fn keep_rite(&mut self) {
        if !self.rite {
            return;
        }
        let kept: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.rite_owed && self.home.rites.has(&h.name))
            .map(|(&id, _)| id)
            .collect();
        for id in kept {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.rite_owed = false;
            }
        }
    }

    /// Knight `id` brings a thing of the Creator to life for the realm. It is
    /// checked again here (the Creator's own budget and ranges), credited to
    /// the knight's name, kept by the realm and taken into the world. The
    /// first one is their rite. Returns the line the party hears.
    pub fn contribute(&mut self, id: u32, mut made: Made) -> Result<String, String> {
        let day = self.home.countryside.squires.day;
        let Some(hero) = self.players.get(&id) else {
            return Err("no such knight".into());
        };
        if hero.absent {
            return Err("no knight is here".into());
        }
        let maker = hero.name.clone();
        let who = hero.shown_name();
        made.credit(&maker, day);
        made.validate()?;
        if self.home.rites.count_of(&maker) >= MOST_PER_KNIGHT {
            return Err(format!("the realm holds {MOST_PER_KNIGHT} of your makings already"));
        }
        let kind = made.kind();
        let rite = Rite {
            knight: maker,
            kind,
            words: made.summary().chars().take(WORDS_MAX).collect(),
            card: String::new(),
            name: made.name().chars().take(24).collect(),
            day,
            made: Some(made),
        };
        let name = rite.name.clone();
        if !self.home.rites.record(rite) {
            return Err("the realm cannot keep it".into());
        }
        if let Some(hero) = self.players.get_mut(&id) {
            hero.rite_owed = false;
            hero.rite_at = self.tick;
        }
        self.stock_rites();
        let line = format!("{who} brings {name} to life: a {} for the realm.", kind.word());
        self.found = Some((self.tick, id, line.clone()));
        if self.notices.len() < 8 {
            self.notices.push(line.clone());
        }
        self.cues.push(format!("rite_made:{}", kind.word()));
        self.sounds.push("wheel_land");
        Ok(line)
    }

    /// The weapons on show, with their loot copies in the book: each shown
    /// rite weapon as a copy of its kind's plain card (`rite-<n>`), the
    /// maker's name on it, weighed as a card's usual in drops and chests.
    pub fn stock_rites(&mut self) {
        let shown: Vec<(usize, Rite)> =
            self.home.rites.newest(Kind::Weapon, MOST_WEAPONS).into_iter().map(|(n, r)| (n, r.clone())).collect();
        for (n, rite) in shown {
            let id = loot_id(n);
            if self.book.get(&id).is_some() {
                continue;
            }
            let Some(weapon) = rite.weapon() else {
                continue;
            };
            let Some(mut card) = self.book.get(base_card(weapon.kind)).cloned() else {
                continue;
            };
            card.id = id;
            card.name = rite.name.clone();
            card.by = rite.knight.clone();
            card.text = format!("{}. Made in the Creator by {}.", rite.words, rite.knight);
            card.drop = LOOT_DROP;
            card.chest = LOOT_CHEST;
            self.book.insert(card);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/rite__tests.rs"]
pub(crate) mod tests;
