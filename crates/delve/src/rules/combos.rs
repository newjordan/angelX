//! Wish combos: the strongest things a wish can do come only of the right
//! pair of wishes (the operator, 2026-10-18: "the super powerful effects
//! require certain wish combos, so homing missiles requires a creature of
//! some form, and a bullet effect of some form etc.").
//!
//! Every wish belongs to one or more [`Family`]s, read off its card: its
//! effects, its looks, and the words of its name and scroll. A knight's
//! families are those of every wish they hold. The super effects
//! ([`RECIPES`]) are given only to a knight who holds both of a recipe's
//! families; until then a wish's super effect stands in as
//! [`STAND_IN`] in the hundred more damage, and the scroll says what it
//! waits on. Found cards (loot) are not wishes and need no recipe.

use super::cards::{Card, Effect, Kind};
use std::collections::BTreeSet;

/// What a wish is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    Creature,
    Projectile,
    Fire,
    Frost,
    Storm,
    Venom,
    Body,
    Swift,
    Spirit,
    Shadow,
    Machine,
    Nature,
}

impl Family {
    pub const ALL: [Family; 12] = [
        Family::Creature,
        Family::Projectile,
        Family::Fire,
        Family::Frost,
        Family::Storm,
        Family::Venom,
        Family::Body,
        Family::Swift,
        Family::Spirit,
        Family::Shadow,
        Family::Machine,
        Family::Nature,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Family::Creature => "Creature",
            Family::Projectile => "Projectile",
            Family::Fire => "Fire",
            Family::Frost => "Frost",
            Family::Storm => "Storm",
            Family::Venom => "Venom",
            Family::Body => "Body",
            Family::Swift => "Swift",
            Family::Spirit => "Spirit",
            Family::Shadow => "Shadow",
            Family::Machine => "Machine",
            Family::Nature => "Nature",
        }
    }

    /// The words in a wish's name or scroll that make it of this family
    /// (a plural's `s` is dropped before they are matched).
    pub fn words(self) -> &'static [&'static str] {
        match self {
            Family::Creature => &[
                "creature", "beast", "familiar", "pet", "minion", "fan", "frog", "toad", "worm",
                "snake", "serpent", "rat", "bat", "bird", "crow", "raven", "owl", "dog", "hound",
                "wolf", "cat", "fox", "bear", "pony", "horse", "unicorn", "dragon", "imp", "devil",
                "demon", "goblin", "spider", "bee", "wasp", "fly", "fish", "shark", "eel", "squid",
                "sperm", "swimmer", "monkey", "goose", "duck", "pig", "cow", "chicken", "slime",
            ],
            Family::Projectile => &[
                "bullet", "shot", "arrow", "bolt", "missile", "dart", "ball", "projectile",
                "ammo", "shell", "rocket", "bomb", "spear", "javelin", "knife", "blade", "orb",
            ],
            Family::Fire => &["fire", "flame", "flaming", "burn", "burning", "blaze", "ember", "lava", "magma", "hot", "inferno", "sun", "flare"],
            Family::Frost => &["ice", "icy", "frost", "frozen", "freeze", "freezing", "snow", "cold", "chill", "glacier", "winter"],
            Family::Storm => &["storm", "lightning", "thunder", "shock", "electric", "spark", "static", "volt", "zap", "wind", "tempest"],
            Family::Venom => &["poison", "venom", "toxic", "acid", "fart", "poop", "turd", "stink", "gas", "rot", "plague", "slime", "filth", "sewage"],
            Family::Body => &["armor", "armour", "mail", "plate", "shield", "helm", "helmet", "feet", "boot", "tuxedo", "dress", "wear", "suit", "coat", "cape", "robe", "crown", "tiara", "muscle", "skin"],
            Family::Swift => &["fast", "faster", "quick", "quicker", "speed", "speedy", "swift", "rapid", "haste", "rollerblade", "skate", "fleet", "time", "clock"],
            Family::Spirit => &["holy", "spirit", "soul", "angel", "drain", "draining", "life", "heal", "healing", "blood", "vampire", "prayer", "blessed", "divine", "ghostly"],
            Family::Shadow => &["shadow", "dark", "darkness", "ghost", "phantom", "night", "void", "phase", "invisible", "shade", "curse"],
            Family::Machine => &["machine", "gun", "cannon", "turret", "robot", "gear", "clockwork", "engine", "laser", "steel", "iron", "mech", "drone"],
            Family::Nature => &["nature", "corn", "flower", "vine", "tree", "leaf", "root", "seed", "plant", "thorn", "mushroom", "moss", "bloom", "petal", "fruit", "cabbage"],
        }
    }
}

/// A super effect, the two families that give it, and its name.
pub struct Recipe {
    pub effect: &'static str,
    pub name: &'static str,
    pub needs: [Family; 2],
}

/// The super effects and what each is made of.
pub const RECIPES: &[Recipe] = &[
    Recipe { effect: "homing", name: "Homing", needs: [Family::Creature, Family::Projectile] },
    Recipe { effect: "chain", name: "Chain Lightning", needs: [Family::Storm, Family::Projectile] },
    Recipe { effect: "burst", name: "Blast", needs: [Family::Fire, Family::Projectile] },
    Recipe { effect: "pierce", name: "Piercing", needs: [Family::Shadow, Family::Projectile] },
    Recipe { effect: "fork", name: "Splitting", needs: [Family::Nature, Family::Projectile] },
    Recipe { effect: "bounce", name: "Ricochet", needs: [Family::Machine, Family::Projectile] },
    Recipe { effect: "orbit", name: "Orbit", needs: [Family::Creature, Family::Body] },
    Recipe { effect: "volley", name: "Volley", needs: [Family::Swift, Family::Projectile] },
];

/// What a locked super effect gives instead, in the hundred more damage.
pub const STAND_IN: u32 = 5;

/// The recipe a held effect word needs, if it is a super effect.
pub fn recipe(effect: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|r| r.effect == effect)
}

/// A wish's card: one the phrasebook granted.
pub fn is_wish(card: &Card) -> bool {
    card.id.starts_with("wish-")
}

/// The card word of a held effect.
pub fn effect_word(effect: &Effect) -> Option<&'static str> {
    Some(match effect {
        Effect::Homing(_) => "homing",
        Effect::Chain(_) => "chain",
        Effect::Burst(_) => "burst",
        Effect::Pierce(_) => "pierce",
        Effect::Fork(_) => "fork",
        Effect::Bounce(_) => "bounce",
        Effect::Orbit(_) => "orbit",
        Effect::Volley(_) => "volley",
        _ => return None,
    })
}

fn said(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") {
                w[..w.len() - 1].to_string()
            } else {
                w.to_string()
            }
        })
        .collect()
}

/// The families one wish's card is of.
pub fn families(card: &Card) -> BTreeSet<Family> {
    let mut out = BTreeSet::new();
    let words = said(&format!("{} {}", card.name, card.text));
    for family in Family::ALL {
        if family.words().iter().any(|w| words.iter().any(|s| s == w)) {
            out.insert(family);
        }
    }
    for effect in &card.effects {
        let family = match effect {
            Effect::Shots(_) | Effect::Size(_) | Effect::Velocity(_) | Effect::Knock(_) | Effect::Boomerang(_) => Family::Projectile,
            Effect::Burn(_) => Family::Fire,
            Effect::Chill(_) => Family::Frost,
            Effect::Chain(_) => Family::Storm,
            Effect::Poison(_) | Effect::Fart(_) => Family::Venom,
            Effect::Armor(_) | Effect::MaxHp(_) => Family::Body,
            Effect::Speed(_) | Effect::Rate(_) => Family::Swift,
            Effect::Vamp(_) | Effect::Mend(_) => Family::Spirit,
            Effect::Summon(..) => Family::Creature,
            _ if effect.summon().is_some() => Family::Creature,
            _ => continue,
        };
        out.insert(family);
    }
    if card.looks.summon.is_some() {
        out.insert(Family::Creature);
    }
    if card.looks.worn.is_some() {
        out.insert(Family::Body);
    }
    if card.looks.shot.is_some() && card.kind == Kind::Hold {
        out.insert(Family::Projectile);
    }
    out
}

/// The families of every wish among `held`.
pub fn held_families<'a>(held: impl IntoIterator<Item = &'a Card>) -> BTreeSet<Family> {
    held.into_iter().filter(|c| is_wish(c)).flat_map(families).collect()
}

/// Whether a knight of `families` is given super effect `effect`: it is not
/// a super effect, or they hold both its families.
pub fn unlocked(effect: &str, families: &BTreeSet<Family>) -> bool {
    recipe(effect).is_none_or(|r| r.needs.iter().all(|f| families.contains(f)))
}

/// The recipes `families` make.
pub fn made(families: &BTreeSet<Family>) -> Vec<&'static Recipe> {
    RECIPES.iter().filter(|r| r.needs.iter().all(|f| families.contains(f))).collect()
}

/// A short list: "Creature, Projectile".
pub fn list(families: &BTreeSet<Family>) -> String {
    families.iter().map(|f| f.word()).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
#[path = "../../tests/rules/together_combos__tests.rs"]
mod tests;
