//! The one wish scroll's front page. Six rows, and which words belong on which.
//! A wish typed on the wrong row is named, not bent into a kind that row cannot make.

/// One thing a wish can make, in the order the scroll offers them (1..6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubRow {
    Power = 0,
    Reforge = 1,
    Look = 2,
    Creature = 3,
    Person = 4,
    Weapon = 5,
}

pub const ROWS: [HubRow; 6] = [
    HubRow::Power,
    HubRow::Reforge,
    HubRow::Look,
    HubRow::Creature,
    HubRow::Person,
    HubRow::Weapon,
];

/// The six-row hub opens in play only when this switch is exactly `1`.
pub fn enabled() -> bool {
    std::env::var("ANGEL_WISH_HUB").ok().as_deref() == Some("1")
}

impl HubRow {
    /// Every row, in the order the scroll offers them.
    pub const ALL: [Self; 6] = ROWS;

    pub fn from_index(n: usize) -> Option<Self> {
        ROWS.get(n).copied()
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// The key the redirect offers: row 0 is PRESS 1.
    pub fn press(self) -> char {
        char::from(b'1' + self as u8)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Power => "A POWER",
            Self::Reforge => "A REFORGE",
            Self::Look => "A LOOK",
            Self::Creature => "A CREATURE",
            Self::Person => "A PERSON",
            Self::Weapon => "A WEAPON",
        }
    }

    pub fn kind_word(self) -> &'static str {
        match self {
            Self::Power => "POWER",
            Self::Reforge => "REFORGE",
            Self::Look => "LOOK",
            Self::Creature => "CREATURE",
            Self::Person => "PERSON",
            Self::Weapon => "WEAPON",
        }
    }

    pub fn need(self) -> &'static str {
        match self {
            Self::Power => "SPELL OR SHOT",
            Self::Reforge => "OFFENCE, DEFENCE",
            Self::Look => "AT THE MIRROR",
            Self::Creature => "MINION OR BOSS",
            Self::Person => "HERO OR FOLK",
            Self::Weapon => "FOR THE WALL",
        }
    }

    /// The row's own name, and the need line, as one short column of lettering.
    pub fn label(self) -> String {
        format!("{}  {}", self.title(), self.need())
    }

    /// Why this row is shut, in a few words.
    pub fn grey(self, gate: WishGate) -> &'static str {
        match self {
            Self::Power => "NO WISH HELD",
            Self::Reforge => {
                if gate.sanctuary {
                    "REFORGE SPENT THIS FLOOR"
                } else {
                    "ONLY IN A SANCTUARY"
                }
            }
            Self::Look => "ONLY AT THE MIRROR",
            Self::Creature | Self::Person | Self::Weapon => "SIX ALREADY MADE",
        }
    }

    /// The genie's one short line beside the stage for this row.
    pub fn quip(self, gate: WishGate) -> &'static str {
        if !gate.open(self) {
            return match self {
                Self::Power => "Rub the lamp first.",
                Self::Reforge => {
                    if gate.sanctuary {
                        "This floor's forge is cold."
                    } else {
                        "Find a sanctuary."
                    }
                }
                Self::Look => "Ask at the mirror.",
                Self::Creature | Self::Person | Self::Weapon => "Six is plenty today.",
            };
        }
        match self {
            Self::Power => "A spell, a shot, a guard?",
            Self::Reforge => "Give the edge a new shape.",
            Self::Look => "A beard? A new colour?",
            Self::Creature => "A friend, or a fright?",
            Self::Person => "Someone for the world.",
            Self::Weapon => "Steel for the wall.",
        }
    }
}

/// What this knight may open right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WishGate {
    /// Holds a wish (or a free one) to spend on a power.
    pub power: bool,
    /// Standing in a Sanctuary.
    pub sanctuary: bool,
    /// That Sanctuary's reforge is still unspent.
    pub reforge: bool,
    /// At the tavern mirror, wishing a look.
    pub look: bool,
    /// Makings already sealed, of six.
    pub made: u8,
    /// Wishes this knight holds (plain, super, fairy, star), for the scroll to show.
    pub wishes: u8,
}

impl WishGate {
    pub const CAP: u8 = 6;

    pub fn open(self, row: HubRow) -> bool {
        match row {
            HubRow::Power => self.power,
            HubRow::Reforge => self.reforge,
            HubRow::Look => self.look,
            HubRow::Creature | HubRow::Person | HubRow::Weapon => self.made < Self::CAP,
        }
    }

    /// The row T lands on: a held wish, else a Sanctuary, else the mirror.
    pub fn fitting(self) -> HubRow {
        if self.power {
            HubRow::Power
        } else if self.reforge {
            HubRow::Reforge
        } else if self.look {
            HubRow::Look
        } else {
            HubRow::Power
        }
    }

    /// "WISHES 2", for beside A POWER while a wish is held.
    pub fn wishes_label(self) -> Option<String> {
        (self.wishes > 0).then(|| format!("WISHES {}", self.wishes))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Spell,
    Summon,
    Shot,
    Guard,
    Weapon,
    Creature,
    Person,
    Look,
}

fn offer(kind: Kind) -> HubRow {
    match kind {
        Kind::Spell | Kind::Summon | Kind::Shot | Kind::Guard => HubRow::Power,
        Kind::Weapon => HubRow::Weapon,
        Kind::Creature => HubRow::Creature,
        Kind::Person => HubRow::Person,
        Kind::Look => HubRow::Look,
    }
}

fn allows(row: HubRow, kind: Kind) -> bool {
    match row {
        // A power is a spell, a summon, a shot or a guard. Not a creature, a person or a wall weapon.
        HubRow::Power => matches!(kind, Kind::Spell | Kind::Summon | Kind::Shot | Kind::Guard),
        // A Sanctuary reforge is offence or defence: a weapon or a guard. Not a minion or a summon.
        HubRow::Reforge => matches!(kind, Kind::Weapon | Kind::Guard),
        HubRow::Look => matches!(kind, Kind::Look),
        HubRow::Creature => matches!(kind, Kind::Creature),
        HubRow::Person => matches!(kind, Kind::Person),
        HubRow::Weapon => matches!(kind, Kind::Weapon),
    }
}

/// The row that can make these words, when the words name a kind.
pub fn named_row(words: &str) -> Option<HubRow> {
    named(words).map(offer)
}

/// One line when `words` name a kind `from` cannot make. None when they fit, or name nothing.
pub fn redirect_line(from: HubRow, words: &str) -> Option<String> {
    let kind = named(words)?;
    if allows(from, kind) {
        return None;
    }
    let row = offer(kind);
    Some(format!("THAT IS A {}: PRESS {}", row.kind_word(), row.press()))
}

/// A Sanctuary reforge must not be sent for these words.
pub fn blocks_reforge(words: &str) -> bool {
    redirect_line(HubRow::Reforge, words).is_some()
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

fn has(ws: &[String], list: &[&str]) -> bool {
    ws.iter().any(|w| list.iter().any(|n| w == n))
}

fn named(text: &str) -> Option<Kind> {
    let ws = tokens(text);
    if ws.is_empty() {
        return None;
    }
    // Role words before the phrasebook, so "crab minions" is a creature even beside "summon".
    if has(
        &ws,
        &[
            "minion", "minions", "pet", "sidekick", "monster", "monsters", "boss", "bosses",
        ],
    ) {
        return Some(Kind::Creature);
    }
    if has(
        &ws,
        &[
            "folk", "villager", "villagers", "person", "people", "npc", "hero", "heroes", "knight",
        ],
    ) {
        return Some(Kind::Person);
    }
    if has(&ws, &["look", "outfit", "costume", "beard", "haircut", "clothes"]) {
        return Some(Kind::Look);
    }
    if has(&ws, &["summon", "summons"]) {
        return Some(Kind::Summon);
    }
    if let Some(found) = crate::rules::phrasebook::find_for(text, &[]) {
        let said = found.says.iter().flatten().any(|line| {
            let line = line.to_ascii_lowercase();
            line.starts_with("summon")
        });
        if found.kind == "summon" || said {
            return Some(Kind::Summon);
        }
        let guard = found.kind == "guard"
            || found
                .says
                .iter()
                .flatten()
                .any(|line| line.to_ascii_lowercase().starts_with("guard"));
        if guard {
            return Some(Kind::Guard);
        }
        return Some(Kind::Spell);
    }
    if has(&ws, &["guard", "shield", "buckler", "armour", "armor", "ward"]) {
        return Some(Kind::Guard);
    }
    if has(&ws, &["shot", "shoot", "bolt", "blast"]) {
        return Some(Kind::Shot);
    }
    if has(&ws, &["spell", "cast", "magic"]) {
        return Some(Kind::Spell);
    }
    if has(
        &ws,
        &[
            "sword", "blade", "axe", "bow", "staff", "spear", "dagger", "hammer", "mace", "club",
            "whip", "flail", "lance", "halberd", "scythe", "trident", "katana", "saber", "sabre",
            "rapier", "claymore", "crossbow", "longbow", "shortbow", "warhammer", "pike", "glaive",
            "bardiche", "partisan", "javelin", "knife", "dirk", "stiletto", "cutlass", "falchion",
            "scimitar", "kris", "kukri", "machete", "cleaver", "hatchet", "tomahawk", "pickaxe",
            "mattock", "maul", "mallet", "cudgel", "baton", "truncheon", "shillelagh", "quarterstaff",
            "bo", "nunchaku", "cestus", "gauntlet", "knuckles", "claw", "wand", "rod", "scepter",
            "sceptre", "orb", "tome", "grimoire", "spellbook", "focus", "relic", "idol", "totem",
            "charm", "amulet", "talisman", "gun", "pistol", "rifle", "musket", "blunderbuss",
            "cannon", "sling", "atlatl", "chakram", "boomerang", "shuriken",
        ],
    ) {
        return Some(Kind::Weapon);
    }
    // A creature's name with no role word still names a creature ("a crab"), not a reforge.
    if has(
        &ws,
        &[
            "crab", "crabs", "imp", "imps", "beast", "beasts", "dragon", "drake", "slime", "goblin",
            "wolf", "spider",
        ],
    ) {
        return Some(Kind::Creature);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_row_opens_only_where_it_should() {
        let shut = WishGate::default();
        assert!(!shut.open(HubRow::Power));
        assert!(!shut.open(HubRow::Reforge));
        assert!(!shut.open(HubRow::Look));
        assert!(shut.open(HubRow::Creature));
        assert!(shut.open(HubRow::Person));
        assert!(shut.open(HubRow::Weapon));
        assert_eq!(HubRow::Power.grey(shut), "NO WISH HELD");
        assert_eq!(HubRow::Reforge.grey(shut), "ONLY IN A SANCTUARY");
        assert_eq!(HubRow::Look.grey(shut), "ONLY AT THE MIRROR");

        let held = WishGate { power: true, ..shut };
        assert!(held.open(HubRow::Power));
        assert_eq!(held.fitting(), HubRow::Power);

        let sanct = WishGate { sanctuary: true, reforge: true, ..shut };
        assert!(sanct.open(HubRow::Reforge));
        assert_eq!(sanct.fitting(), HubRow::Reforge);
        let spent = WishGate { sanctuary: true, reforge: false, ..shut };
        assert!(!spent.open(HubRow::Reforge));
        assert_eq!(HubRow::Reforge.grey(spent), "REFORGE SPENT THIS FLOOR");

        let mirror = WishGate { look: true, ..shut };
        assert!(mirror.open(HubRow::Look));
        assert_eq!(mirror.fitting(), HubRow::Look);

        // A held wish wins over a Sanctuary, as T's fitting row.
        let both = WishGate { power: true, reforge: true, sanctuary: true, ..shut };
        assert_eq!(both.fitting(), HubRow::Power);

        let full = WishGate { made: 6, ..shut };
        assert!(!full.open(HubRow::Creature));
        assert!(!full.open(HubRow::Person));
        assert!(!full.open(HubRow::Weapon));
        assert_eq!(HubRow::Weapon.grey(full), "SIX ALREADY MADE");
    }

    #[test]
    fn the_wrong_row_is_named_and_a_reforge_refuses_a_minion() {
        assert_eq!(
            redirect_line(HubRow::Reforge, "crab minions").as_deref(),
            Some("THAT IS A CREATURE: PRESS 4")
        );
        assert!(blocks_reforge("crab minions"));
        assert!(blocks_reforge("summon a storm"));
        assert!(blocks_reforge("a hero of the village"));
        // Offence and defence stay on the reforge row.
        assert!(!blocks_reforge("a heavier sword"));
        assert!(!blocks_reforge("a round shield"));
        // A power row keeps a spell and a summon, and will not swallow a creature.
        assert!(redirect_line(HubRow::Power, "fire bolt").is_none() || named_row("fire bolt") == Some(HubRow::Power));
        assert_eq!(
            redirect_line(HubRow::Power, "three minions").as_deref(),
            Some("THAT IS A CREATURE: PRESS 4")
        );
        assert!(redirect_line(HubRow::Reforge, "hello there").is_none());
    }
}
