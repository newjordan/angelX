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

pub fn on() -> bool {
    std::env::var_os("ANGEL_DELVE_RITE").is_none_or(|v| v != "0")
}

pub const ACTION: u32 = 10;

pub const INVITE: &str = "YOUR RITE: MAKE SOMETHING FOR THE REALM";
pub const TOLD: &str = "Your rite: make something for the realm. Press C.";

pub const MOST_WEAPONS: usize = 5;
pub const MOST_MINIONS: usize = 6;
pub const MOST_FOLK: usize = 4;
pub const MOST_HEROES: usize = 3;
pub const MOST_FOES: usize = 2;
pub const MOST_KEPT: usize = 2048;
pub const MOST_PER_KNIGHT: usize = 6;
pub const WORDS_MAX: usize = 120;
pub const FLOURISH_TICKS: u64 = 5 * HZ as u64 / 2;
const LOOT_DROP: u32 = 2;
const LOOT_CHEST: u32 = 3;
const MONSTER_HP: u32 = 140;
const BOSS_HP: u32 = 900;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rite {
    pub knight: String,
    pub kind: Kind,
    #[serde(default)]
    pub words: String,
    #[serde(default)]
    pub card: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub day: u32,
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

    pub fn creature(&self) -> Option<&Creation> {
        match &self.made {
            Some(Made::Creature(c)) => Some(c),
            _ => None,
        }
    }

    pub fn weapon(&self) -> Option<&WeaponCreation> {
        match &self.made {
            Some(Made::Weapon(w)) => Some(w),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rites {
    #[serde(default)]
    pub kept: Vec<Rite>,
}

impl Rites {
    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    pub fn has(&self, knight: &str) -> bool {
        self.kept.iter().any(|r| r.knight.eq_ignore_ascii_case(knight))
    }

    pub fn count_of(&self, knight: &str) -> usize {
        self.kept.iter().filter(|r| r.knight.eq_ignore_ascii_case(knight)).count()
    }

    pub fn newest(&self, kind: Kind, most: usize) -> Vec<(usize, &Rite)> {
        let mut all: Vec<(usize, &Rite)> = self.kept.iter().enumerate().filter(|(_, r)| r.kind == kind).collect();
        all.drain(..all.len().saturating_sub(most));
        all
    }

    pub fn record(&mut self, rite: Rite) -> bool {
        if self.count_of(&rite.knight) >= MOST_PER_KNIGHT || self.kept.len() >= MOST_KEPT || !rite.valid() {
            return false;
        }
        self.kept.push(rite);
        true
    }

    pub fn merge(&mut self, other: &Rites) -> bool {
        let mut changed = false;
        for rite in &other.kept {
            if !self.kept.contains(rite) {
                changed |= self.record(rite.clone());
            }
        }
        changed
    }

    pub fn valid(&self) -> bool {
        self.kept.len() <= MOST_KEPT && self.kept.iter().all(Rite::valid)
    }
}

pub fn loot_id(index: usize) -> String {
    format!("rite-{index}")
}

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

pub fn made_hp(creation: &Creation) -> u32 {
    let cap = if creation.role == Role::Boss { BOSS_HP } else { MONSTER_HP };
    u32::from(creation.stats().hp).clamp(10, cap)
}

impl Run {
    pub fn owes_rite(&self, id: u32) -> bool {
        self.rite && self.players.get(&id).is_some_and(|h| h.rite_owed)
    }

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
