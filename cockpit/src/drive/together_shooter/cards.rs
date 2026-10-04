//! Cards: every pickup in the delve is a card, and a card is a short text
//! file anyone can write.
//!
//! A card file names the card, says how it is used (`take`, `hold`, `play`
//! `arm`, `guard` or `spell`), lists at most three effects from a fixed vocabulary, and may
//! draw its own art in realm inks. The game checks every file the way the
//! Forge checks runes: unknown words are rejected, numbers are clamped with
//! a note, and a deck's held bonuses never pass the caps below. Cards are
//! data, never code, so a friend's card cannot do anything the vocabulary
//! does not allow.
//!
//! Built-in cards live in `cockpit/assets/dungeon/cards/`; a workspace adds
//! its own under `.angel/dungeon/cards/`. A run keeps its own copy of the
//! book, so a saved delve and a guest's frame never depend on files that
//! changed later.

use super::loot::Rng;
use super::{Pack, Weapon};
use crate::drive::together_forge as forge;
use crate::drive::together_realm::Spoil;
use serde::{Deserialize, Serialize};

pub(crate) const MAX_BYTES: usize = 4096;
/// Play cards a knight can carry; number keys 1–4 play them.
pub(crate) const HAND: usize = 4;
pub(crate) const SPELL_SLOTS: usize = 3;
/// Seconds, bounded to prevent overflow and zero-recharge spells.
pub(crate) const SPELL_COOLDOWN: (u32, u32) = (1, 120);
pub(crate) const MAX_EFFECTS: usize = 3;
/// Card art is at most this many ink pixels on a side.
pub(crate) const ART: usize = 16;
pub(crate) const MAX_BOOK: usize = 96;
pub(crate) const MAX_NAME: usize = 22;
pub(crate) const MAX_TEXT: usize = 64;

/// How a card is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    /// Spent the moment it is picked up.
    Take,
    /// Kept in the deck; its bonus lasts the whole run.
    Hold,
    /// Kept in the hand (four slots) until a number key plays it.
    Play,
    /// Reusable; casting starts this slot's recharge.
    Spell,
    /// The knight's weapon; taking it leaves the old one on the floor.
    Arm,
    /// The knight's defence on the guard key: a dodge roll or a shield.
    Guard,
}

/// What the guard key does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Guard {
    /// A quick tumble nothing can touch.
    #[default]
    Roll,
    /// Held up: shots from the front glance off; slow, and no shooting.
    Shield,
    /// Held up: a barrier across the aim that stops monsters' shots and lets
    /// friends' shots through, stronger; it drains mana while held.
    Wall(Wall),
}

/// A held wall's measure: how wide (arena units), how much mana a second it
/// drains, and how much harder friendly shots hit after passing through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Wall {
    pub(crate) width: u32,
    pub(crate) drain: u32,
    pub(crate) empower: u32,
}

impl Wall {
    pub(crate) const WIDTH: (u32, u32) = (2, 8);
    pub(crate) const DRAIN: (u32, u32) = (10, 50);
    pub(crate) const EMPOWER: (u32, u32) = (0, 40);
}

impl Kind {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Kind::Take => "take",
            Kind::Hold => "hold",
            Kind::Play => "play",
            Kind::Spell => "spell",
            Kind::Arm => "arm",
            Kind::Guard => "guard",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Rarity {
    Common,
    Rare,
    Relic,
}

impl Rarity {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Rare => "rare",
            Rarity::Relic => "relic",
        }
    }

    /// Default weights in monster drops and in chests.
    fn weights(self) -> (u32, u32) {
        match self {
            Rarity::Common => (10, 2),
            Rarity::Rare => (3, 4),
            Rarity::Relic => (0, 2),
        }
    }
}

/// One effect line. Which kinds may carry it, and its bounds, are in `SPECS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Effect {
    Cast(Shape),
    Heal(u32),
    Gold(u32),
    Bombs(u32),
    MaxHp(u32),
    Nova(u32),
    Ward(u32),
    Damage(u32),
    Rate(u32),
    Speed(u32),
    Pierce(u32),
    Shots(u32),
    Armor(u32),
    Vamp(u32),
    /// Every Nth shot flies three wide (the smallest N held wins).
    Volley(u32),
    /// Shots bounce off walls N times.
    Bounce(u32),
    /// Shots curve toward the nearest monster (N: how hard).
    Homing(u32),
    /// A hit sparks on to N more monsters nearby.
    Chain(u32),
    /// Each hit mends the most hurt friend by N.
    Mend(u32),
    /// N morningstars spin close round the knight.
    Orbit(u32),
    /// Shots burst where they hit, N% of their damage to all around.
    Burst(u32),
    /// Spoils carried out of the delve for the realm's treasury.
    Bone(u32),
    Wax(u32),
    Ore(u32),
    Gem(u32),
    Ember(u32),
    Scale(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Shape {
    Bolt,
    Ring,
    Meteor,
}

impl Shape {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Bolt => "bolt",
            Self::Ring => "ring",
            Self::Meteor => "meteor",
        }
    }
}

struct Spec {
    word: &'static str,
    low: u32,
    high: u32,
    /// For a held card, the most the whole deck may add up to.
    cap: u32,
    hold: bool,
    spend: bool,
    make: fn(u32) -> Effect,
    says: &'static str,
}

const CAST_SPEC: Spec = Spec {
    word: "cast",
    low: 0,
    high: 0,
    cap: 0,
    hold: false,
    spend: false,
    make: |_| Effect::Cast(Shape::Bolt),
    says: "launch bolt, ring or meteor with your shot traits",
};

const SPECS: &[Spec] = &[
    Spec {
        word: "heal",
        low: 5,
        high: 100,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Heal,
        says: "restore N health",
    },
    Spec {
        word: "gold",
        low: 10,
        high: 500,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Gold,
        says: "N score",
    },
    Spec {
        word: "bombs",
        low: 1,
        high: 3,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Bombs,
        says: "N bomb charges",
    },
    Spec {
        word: "max_hp",
        low: 5,
        high: 40,
        cap: 0,
        hold: true,
        spend: true,
        make: Effect::MaxHp,
        says: "N more maximum health, for good",
    },
    Spec {
        word: "nova",
        low: 10,
        high: 120,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Nova,
        says: "N damage to every monster in the room",
    },
    Spec {
        word: "ward",
        low: 1,
        high: 5,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Ward,
        says: "untouchable for N seconds",
    },
    Spec {
        word: "damage",
        low: 5,
        high: 50,
        cap: 100,
        hold: true,
        spend: false,
        make: Effect::Damage,
        says: "shots and swings hit N% harder",
    },
    Spec {
        word: "rate",
        low: 5,
        high: 40,
        cap: 60,
        hold: true,
        spend: false,
        make: Effect::Rate,
        says: "shoot and swing N% faster",
    },
    Spec {
        word: "speed",
        low: 5,
        high: 30,
        cap: 40,
        hold: true,
        spend: false,
        make: Effect::Speed,
        says: "walk N% faster",
    },
    Spec {
        word: "pierce",
        low: 1,
        high: 2,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Pierce,
        says: "shots pass through N more monsters",
    },
    Spec {
        word: "shots",
        low: 1,
        high: 2,
        cap: 2,
        hold: true,
        spend: false,
        make: Effect::Shots,
        says: "N extra shots in a fan",
    },
    Spec {
        word: "armor",
        low: 1,
        high: 2,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Armor,
        says: "each piece turns aside 3 damage per hit",
    },
    Spec {
        word: "vamp",
        low: 1,
        high: 10,
        cap: 10,
        hold: true,
        spend: false,
        make: Effect::Vamp,
        says: "mend N health for each monster slain",
    },
    Spec {
        word: "volley",
        low: 1,
        high: 8,
        cap: 8,
        hold: true,
        spend: false,
        make: Effect::Volley,
        says: "every Nth shot flies three wide (1: every shot)",
    },
    Spec {
        word: "bounce",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Bounce,
        says: "shots bounce off walls N times",
    },
    Spec {
        word: "homing",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Homing,
        says: "shots curve toward the nearest monster, harder with N",
    },
    Spec {
        word: "chain",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Chain,
        says: "a hit sparks on to N more monsters nearby, for half",
    },
    Spec {
        word: "mend",
        low: 1,
        high: 5,
        cap: 8,
        hold: true,
        spend: false,
        make: Effect::Mend,
        says: "each hit mends the most hurt friend by N",
    },
    Spec {
        word: "orbit",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Orbit,
        says: "N morningstars spin close round you, striking and parrying",
    },
    Spec {
        word: "burst",
        low: 15,
        high: 50,
        cap: 60,
        hold: true,
        spend: false,
        make: Effect::Burst,
        says: "shots burst where they hit, N% of their damage to all around",
    },
    Spec {
        word: "bone",
        low: 1,
        high: 3,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Bone,
        says: "N bone to carry home (spoils)",
    },
    Spec {
        word: "wax",
        low: 1,
        high: 3,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Wax,
        says: "N candle-wax to carry home (spoils)",
    },
    Spec {
        word: "ore",
        low: 1,
        high: 3,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Ore,
        says: "N ore to carry home (spoils)",
    },
    Spec {
        word: "gem",
        low: 1,
        high: 2,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Gem,
        says: "N gems to carry home (spoils)",
    },
    Spec {
        word: "ember",
        low: 1,
        high: 2,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Ember,
        says: "N embers to carry home (spoils)",
    },
    Spec {
        word: "scale",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Scale,
        says: "a dragon scale to carry home (spoils)",
    },
];

impl Effect {
    /// The spoil this effect carries home, if it is one.
    pub(crate) fn spoil(self) -> Option<(Spoil, u32)> {
        Some(match self {
            Effect::Gold(v) => (Spoil::Gold, v),
            Effect::Bone(v) => (Spoil::Bone, v),
            Effect::Wax(v) => (Spoil::Wax, v),
            Effect::Ore(v) => (Spoil::Ore, v),
            Effect::Gem(v) => (Spoil::Gem, v),
            Effect::Ember(v) => (Spoil::Ember, v),
            Effect::Scale(v) => (Spoil::Scale, v),
            _ => return None,
        })
    }

    fn parts(self) -> (&'static str, u32) {
        let value = match self {
            Effect::Cast(_) => return ("cast", 0),
            Effect::Heal(v)
            | Effect::Gold(v)
            | Effect::Bombs(v)
            | Effect::MaxHp(v)
            | Effect::Nova(v)
            | Effect::Ward(v)
            | Effect::Damage(v)
            | Effect::Rate(v)
            | Effect::Speed(v)
            | Effect::Pierce(v)
            | Effect::Shots(v)
            | Effect::Armor(v)
            | Effect::Vamp(v)
            | Effect::Volley(v)
            | Effect::Bounce(v)
            | Effect::Homing(v)
            | Effect::Chain(v)
            | Effect::Mend(v)
            | Effect::Orbit(v)
            | Effect::Burst(v)
            | Effect::Bone(v)
            | Effect::Wax(v)
            | Effect::Ore(v)
            | Effect::Gem(v)
            | Effect::Ember(v)
            | Effect::Scale(v) => v,
        };
        let word = SPECS
            .iter()
            .find(|s| std::mem::discriminant(&(s.make)(value)) == std::mem::discriminant(&self))
            .map_or("?", |s| s.word);
        (word, value)
    }

    /// Short rules text: `damage +25%`, `heal 35`.
    pub(crate) fn label(self) -> String {
        if let Effect::Cast(shape) = self {
            return format!("cast {}", shape.word());
        }
        let (word, value) = self.parts();
        match self {
            Effect::Damage(_) | Effect::Rate(_) | Effect::Speed(_) => format!("{word} +{value}%"),
            Effect::Ward(_) => format!("ward {value}s"),
            Effect::MaxHp(_) => format!("max hp +{value}"),
            Effect::Pierce(_) | Effect::Shots(_) | Effect::Armor(_) => format!("{word} +{value}"),
            Effect::Volley(1) => "every shot three wide".into(),
            Effect::Volley(_) => format!("every {value}th shot three wide"),
            _ => format!("{word} {value}"),
        }
    }
}

/// A weapon card: one of the delve's arms, or Forge runes checked at tier I.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Arm {
    Plain(Weapon),
    Forged(forge::Weapon),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Card {
    /// The file name without `.card`: lowercase letters, digits and dashes.
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: Kind,
    pub(crate) rarity: Rarity,
    pub(crate) text: String,
    /// Who made it; empty for the delve's own cards.
    pub(crate) by: String,
    pub(crate) drop: u32,
    pub(crate) chest: u32,
    pub(crate) effects: Vec<Effect>,
    pub(crate) arm: Option<Arm>,
    /// Ink rows, at most 16 by 16; empty means the kind's own glyph.
    pub(crate) art: Vec<String>,
    /// The only delve it drops in; none drops anywhere.
    #[serde(default)]
    pub(crate) only_in: Option<Pack>,
    /// What a guard card puts on the guard key.
    #[serde(default)]
    pub(crate) guard: Option<Guard>,
    /// Recharge seconds; older non-spell cards deserialize with zero.
    #[serde(default)]
    pub(crate) cooldown: u32,
}

impl Card {
    /// A material card: it carries spoils home and drops on its own roll.
    pub(crate) fn is_spoil(&self) -> bool {
        self.effects
            .iter()
            .any(|e| e.spoil().is_some_and(|(s, _)| s != Spoil::Gold))
    }

    fn drops_in(&self, pack: Pack) -> bool {
        self.only_in.is_none_or(|p| p == pack)
    }

    /// The card's art rows, or its kind's glyph when it drew none.
    pub(crate) fn art_rows(&self) -> Vec<&str> {
        if self.art.is_empty() {
            glyph(self.kind).to_vec()
        } else {
            self.art.iter().map(String::as_str).collect()
        }
    }

    /// One line of rules: effects, or the weapon's numbers.
    pub(crate) fn rules(&self) -> String {
        match &self.arm {
            Some(Arm::Plain(weapon)) => {
                let arms = weapon.arms();
                format!(
                    "{} dmg · {:.1}s{}",
                    arms.damage,
                    arms.cooldown as f32 / super::HZ as f32,
                    if arms.pierce > 0 { " · pierces" } else { "" }
                )
            }
            Some(Arm::Forged(weapon)) => weapon.summary(),
            None if self.guard.is_some() => match self.guard {
                Some(Guard::Shield) => "hold space: block shots in front".into(),
                Some(Guard::Wall(w)) => format!(
                    "hold space: a {}-wide wall · {} mana/s{}",
                    w.width,
                    w.drain,
                    if w.empower > 0 {
                        format!(" · friends' shots +{}%", w.empower)
                    } else {
                        String::new()
                    }
                ),
                _ => "space: roll, untouchable".into(),
            },
            None => {
                let mut labels: Vec<_> = self.effects.iter().map(|e| e.label()).collect();
                if self.kind == Kind::Spell {
                    labels.push(format!("cooldown {}s", self.cooldown));
                }
                labels.join(" · ")
            }
        }
    }
}

/// Art for a card that drew none: a rune for its kind.
fn glyph(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Take => &[
            "...55...", "..5665..", ".566665.", "56666665", "56666665", ".566665.", "..5665..",
            "...55...",
        ],
        Kind::Hold => &[
            ".hHHHHh.", "hH4444Hh", "H4h44h4H", "H444444H", "H44hh44H", ".H4444H.", "..H44H..",
            "...HH...",
        ],
        Kind::Play | Kind::Spell => &[
            "..2222..", ".233332.", "23322332", "23233232", "23233232", "23322332", ".233332.",
            "..2222..",
        ],
        Kind::Guard => &[
            ".hHHHHh.", "hH4444Hh", "H444444H", "H44HH44H", "H444444H", ".H4444H.", "..H44H..",
            "...HH...",
        ],
        Kind::Arm => &[
            "......Hh", ".....Hh.", "....Hh..", "...Hh...", "4.Hh....", ".4h.....", "a.4.....",
            "aa......",
        ],
    }
}

/// What the checker accepted, with every value it changed.
#[derive(Debug)]
pub(crate) struct Checked {
    pub(crate) card: Card,
    pub(crate) notes: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct Rejected {
    pub(crate) errors: Vec<String>,
}

impl Rejected {
    pub(crate) fn fix_it(&self) -> String {
        format!(
            "The card was rejected:\n{}\nFix these and try again (/dungeon cards shows the format).",
            self.errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// A card's id from its file name: lowercase, dashes, at most 32 characters.
pub(crate) fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(32).collect()
}

/// Check one card file. `id` usually comes from the file name.
pub(crate) fn check(id: &str, raw: &str) -> Result<Checked, Rejected> {
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    if raw.len() > MAX_BYTES {
        return Err(Rejected {
            errors: vec![format!("the file is over {MAX_BYTES} bytes")],
        });
    }
    let mut id = slug(id);
    let (mut name, mut kind, mut rarity) = (None::<String>, None::<Kind>, Rarity::Common);
    let (mut text, mut by) = (String::new(), String::new());
    let (mut drop, mut chest) = (None::<u32>, None::<u32>);
    let mut effects = Vec::new();
    let mut runes = Vec::new();
    let mut plain = None;
    let mut only_in = None;
    let mut guard = None;
    let mut cooldown = None;
    let script = super::script::read(raw);
    let art = script.art;
    if script.art_blocks > 1 {
        errors.push("only one art block".into());
    }
    for line in &script.lines {
        let (n, word, rest) = (line.n, line.word.as_str(), line.rest);
        match word {
            "name" => name = Some(super::script::clean(rest, 64)),
            "text" => text = super::script::clean(rest, 256),
            "by" => by = super::script::clean(rest, 24),
            "kind" => {
                kind = match rest.to_ascii_lowercase().as_str() {
                    "take" => Some(Kind::Take),
                    "hold" => Some(Kind::Hold),
                    "play" => Some(Kind::Play),
                    "spell" => Some(Kind::Spell),
                    "arm" => Some(Kind::Arm),
                    "guard" => Some(Kind::Guard),
                    other => {
                        errors.push(format!(
                            "line {n}: kind `{other}` — use take, hold, play, arm, guard or spell"
                        ));
                        None
                    }
                }
            }
            "cooldown" => {
                if cooldown.is_some() {
                    errors.push(format!("line {n}: cooldown appears twice"));
                }
                match rest.trim_end_matches('s').parse::<u32>() {
                    Ok(value) => {
                        let clamped = value.clamp(SPELL_COOLDOWN.0, SPELL_COOLDOWN.1);
                        if clamped != value {
                            notes.push(format!("cooldown {value} clamped to {clamped}s"));
                        }
                        cooldown = Some(clamped);
                    }
                    Err(_) => errors.push(format!("line {n}: cooldown takes whole seconds 1–120")),
                }
            }
            "cast" => {
                let shape = match rest.to_ascii_lowercase().as_str() {
                    "bolt" => Some(Shape::Bolt),
                    "ring" => Some(Shape::Ring),
                    "meteor" => Some(Shape::Meteor),
                    _ => None,
                };
                match shape {
                    Some(shape) => effects.push((n, &CAST_SPEC, Effect::Cast(shape))),
                    None => errors.push(format!("line {n}: cast takes bolt, ring or meteor")),
                }
            }
            "rarity" => match rest.to_ascii_lowercase().as_str() {
                "common" => rarity = Rarity::Common,
                "rare" => rarity = Rarity::Rare,
                "relic" => rarity = Rarity::Relic,
                other => errors.push(format!(
                    "line {n}: rarity `{other}` — use common, rare or relic"
                )),
            },
            "where" => match rest.to_ascii_lowercase().as_str() {
                "crypt" | "the crypt" => only_in = Some(Pack::Crypt),
                "mines" | "the mines" => only_in = Some(Pack::Cavern),
                "keep" | "dragon keep" => only_in = Some(Pack::Hellforge),
                "any" | "anywhere" => only_in = None,
                other => errors.push(format!(
                    "line {n}: where `{other}` — use crypt, mines, keep or anywhere"
                )),
            },
            "drop" | "chest" => match rest.parse::<u32>() {
                Ok(value) => {
                    let clamped = value.min(100);
                    if clamped != value {
                        notes.push(format!("{word} {value} clamped to 100"));
                    }
                    if word == "drop" {
                        drop = Some(clamped);
                    } else {
                        chest = Some(clamped);
                    }
                }
                Err(_) => errors.push(format!("line {n}: {word} takes a weight 0–100")),
            },
            "arm" => match rest.to_ascii_lowercase().as_str() {
                "bow" => plain = Some(Weapon::Bow),
                "crossbow" => plain = Some(Weapon::Crossbow),
                "handgonne" => plain = Some(Weapon::Handgonne),
                other => errors.push(format!(
                    "line {n}: arm `{other}` — use bow, crossbow or handgonne, or Forge rune lines"
                )),
            },
            "bolt" | "spread" | "melee" | "throw" | "look" => runes.push(format!("{word} {rest}")),
            "guard" => {
                let lower = rest.to_ascii_lowercase();
                let mut parts = lower.split_whitespace();
                match parts.next().unwrap_or("") {
                    "roll" => guard = Some(Guard::Roll),
                    "shield" => guard = Some(Guard::Shield),
                    "wall" => {
                        let mut value = |key: &str, (low, high): (u32, u32), default: u32| {
                            let raw = lower
                                .split_whitespace()
                                .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
                                .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok());
                            let v = raw.map_or(default, |v| v.round().max(0.0) as u32);
                            let clamped = v.clamp(low, high);
                            if clamped != v {
                                notes.push(format!("wall {key} {v} clamped to {clamped}"));
                            }
                            clamped
                        };
                        let wall = Wall {
                            width: value("width", Wall::WIDTH, 4),
                            drain: value("drain", Wall::DRAIN, 25),
                            empower: value("empower", Wall::EMPOWER, 0),
                        };
                        // A wider, kinder wall drinks more: at least 5 mana a
                        // second per unit of width, plus a third of its empower.
                        let floor = wall.width * 5 + wall.empower / 3;
                        let wall = if wall.drain < floor {
                            notes.push(format!("wall drain raised to {floor}: a wall that wide and strong costs more to hold"));
                            Wall { drain: floor.min(Wall::DRAIN.1), ..wall }
                        } else {
                            wall
                        };
                        guard = Some(Guard::Wall(wall));
                    }
                    other => errors.push(format!(
                        "line {n}: guard `{other}` — roll, shield, or wall width=2-8 drain=10-50 empower=0-40"
                    )),
                }
            }
            _ => {
                if let Some(spec) = SPECS.iter().find(|s| s.word == word) {
                    let value = rest
                        .trim_start_matches('+')
                        .trim_end_matches(['%', 's'])
                        .parse::<u32>();
                    match value {
                        Ok(value) => {
                            let clamped = value.clamp(spec.low, spec.high);
                            if clamped != value {
                                notes.push(format!(
                                    "{} {value} is outside {}–{}, clamped to {clamped}",
                                    spec.word, spec.low, spec.high
                                ));
                            }
                            effects.push((n, spec, (spec.make)(clamped)));
                        }
                        Err(_) => errors.push(format!(
                            "line {n}: {} takes a whole number {}–{}",
                            spec.word, spec.low, spec.high
                        )),
                    }
                } else {
                    errors.push(format!(
                        "line {n}: unknown word `{word}`; a card has name, kind, rarity, text, by, drop, chest, art, cooldown, cast, effects ({}), and for arm cards `arm <weapon>` or Forge runes",
                        SPECS.iter().map(|s| s.word).collect::<Vec<_>>().join(", ")
                    ));
                }
            }
        }
    }
    let name = match name {
        Some(name) if !name.trim().is_empty() => {
            let name = name.trim().to_string();
            if name.chars().count() > MAX_NAME {
                notes.push(format!("name cut to {MAX_NAME} characters"));
            }
            name.chars().take(MAX_NAME).collect()
        }
        _ => {
            errors.push("the card needs a `name`".into());
            String::new()
        }
    };
    if id.is_empty() {
        // A card sent without a file takes its id from its name.
        id = slug(&name);
    }
    if id.is_empty() && !name.is_empty() {
        errors.push("the card's name needs a letter or digit".into());
    }
    if text.chars().count() > MAX_TEXT {
        notes.push(format!("text cut to {MAX_TEXT} characters"));
        text = text.chars().take(MAX_TEXT).collect();
    }
    let Some(kind) = kind else {
        errors.push("the card needs a `kind`: take, hold, play, arm, guard or spell".into());
        return Err(Rejected { errors });
    };
    if kind == Kind::Spell && cooldown.is_none() {
        errors.push("a spell needs `cooldown <seconds>` (1–120)".into());
    } else if kind != Kind::Spell && cooldown.is_some() {
        errors.push("`cooldown` belongs on `kind spell`".into());
    }
    let mut arm = None;
    if kind == Kind::Guard {
        if guard.is_none() {
            errors.push("a guard card needs `guard roll` or `guard shield`".into());
        }
        if !effects.is_empty() || plain.is_some() || !runes.is_empty() {
            errors.push("a guard card only says what the guard key does".into());
        }
    } else if guard.is_some() {
        errors.push("`guard` belongs on `kind guard`".into());
    } else if kind == Kind::Arm {
        if !effects.is_empty() {
            errors.push("an arm card is a weapon; put effects on a hold card".into());
        }
        match (plain, runes.is_empty()) {
            (Some(_), false) => errors.push("use `arm <weapon>` or rune lines, not both".into()),
            (Some(weapon), true) => arm = Some(Arm::Plain(weapon)),
            (None, false) => {
                let file = format!("name {name}\n{}", runes.join("\n"));
                match forge::check(&file, forge::START_LOC) {
                    Ok(checked) => {
                        notes.extend(checked.notes);
                        arm = Some(Arm::Forged(checked.weapon));
                    }
                    Err(rejected) => errors.extend(rejected.errors),
                }
            }
            (None, true) => errors.push(
                "an arm card needs `arm bow|crossbow|handgonne` or Forge runes (bolt/melee/spread/look)".into(),
            ),
        }
    } else {
        if plain.is_some() || !runes.is_empty() {
            errors.push(format!(
                "weapon lines belong on `kind arm`, not `kind {}`",
                kind.word()
            ));
        }
        if effects.is_empty() {
            errors.push(format!("a {} card needs at least one effect", kind.word()));
        }
        if effects.len() > MAX_EFFECTS {
            errors.push(format!(
                "at most {MAX_EFFECTS} effects per card ({} given)",
                effects.len()
            ));
        }
        for (n, spec, effect) in &effects {
            let fits = if kind == Kind::Spell {
                matches!(
                    effect,
                    Effect::Heal(_)
                        | Effect::Bombs(_)
                        | Effect::Nova(_)
                        | Effect::Ward(_)
                        | Effect::Cast(_)
                )
            } else if kind == Kind::Hold {
                spec.hold
            } else {
                spec.spend
            };
            if !fits {
                errors.push(format!(
                    "line {n}: {} works on {} cards, not {}",
                    spec.word,
                    if spec.word == "cast" {
                        "spell"
                    } else if spec.hold {
                        "hold"
                    } else {
                        "take or play"
                    },
                    kind.word()
                ));
            }
        }
        let mut seen = Vec::new();
        for (n, spec, _) in &effects {
            if seen.contains(&spec.word) {
                errors.push(format!("line {n}: {} appears twice", spec.word));
            }
            seen.push(spec.word);
        }
    }
    if art.len() > ART || art.iter().any(|row| row.chars().count() > ART) {
        errors.push(format!("art is at most {ART} by {ART} inks"));
    }
    if !art.is_empty() && art.iter().all(|row| row.chars().all(|c| c == '.')) {
        errors.push("art has no inked pixel; use realm inks (see /dungeon cards)".into());
    }
    if !errors.is_empty() {
        return Err(Rejected { errors });
    }
    let (default_drop, default_chest) = rarity.weights();
    Ok(Checked {
        card: Card {
            id,
            name,
            kind,
            rarity,
            text,
            by,
            drop: drop.unwrap_or(default_drop),
            chest: chest.unwrap_or(default_chest),
            effects: effects.into_iter().map(|(_, _, e)| e).collect(),
            arm,
            art,
            only_in,
            guard,
            cooldown: cooldown.unwrap_or(0),
        },
        notes,
    })
}

/// What a knight's held cards add, already capped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Bonus {
    pub(crate) damage: u32,
    pub(crate) rate: u32,
    pub(crate) speed: u32,
    pub(crate) pierce: u32,
    pub(crate) shots: u32,
    pub(crate) armor: u32,
    pub(crate) vamp: u32,
    /// Every Nth shot flies three wide; 0 for none.
    #[serde(default)]
    pub(crate) volley: u32,
    #[serde(default)]
    pub(crate) bounce: u32,
    #[serde(default)]
    pub(crate) homing: u32,
    #[serde(default)]
    pub(crate) chain: u32,
    #[serde(default)]
    pub(crate) mend: u32,
    #[serde(default)]
    pub(crate) orbit: u32,
    #[serde(default)]
    pub(crate) burst: u32,
}

impl Bonus {
    pub(crate) fn of<'a>(held: impl IntoIterator<Item = &'a Card>) -> Bonus {
        let mut bonus = Bonus::default();
        for effect in held.into_iter().flat_map(|c| c.effects.iter().copied()) {
            let slot = match effect {
                Effect::Damage(v) => Some((&mut bonus.damage, v)),
                Effect::Rate(v) => Some((&mut bonus.rate, v)),
                Effect::Speed(v) => Some((&mut bonus.speed, v)),
                Effect::Pierce(v) => Some((&mut bonus.pierce, v)),
                Effect::Shots(v) => Some((&mut bonus.shots, v)),
                Effect::Armor(v) => Some((&mut bonus.armor, v)),
                Effect::Vamp(v) => Some((&mut bonus.vamp, v)),
                Effect::Bounce(v) => Some((&mut bonus.bounce, v)),
                Effect::Homing(v) => Some((&mut bonus.homing, v)),
                Effect::Chain(v) => Some((&mut bonus.chain, v)),
                Effect::Mend(v) => Some((&mut bonus.mend, v)),
                Effect::Orbit(v) => Some((&mut bonus.orbit, v)),
                Effect::Burst(v) => Some((&mut bonus.burst, v)),
                // Volleys don't add up: the tightest cadence held counts.
                Effect::Volley(v) => {
                    bonus.volley = if bonus.volley == 0 {
                        v
                    } else {
                        bonus.volley.min(v)
                    };
                    None
                }
                _ => None,
            };
            if let Some((slot, value)) = slot {
                let cap = SPECS
                    .iter()
                    .find(|s| s.word == effect.parts().0)
                    .map_or(0, |s| s.cap);
                *slot = (*slot + value).min(cap);
            }
        }
        bonus
    }
}

/// The cards a run can find, sorted by id so every roll replays.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Book {
    /// Content changes, including replacement, require a whole mirror update.
    #[serde(default)]
    pub(crate) revision: usize,
    pub(crate) cards: Vec<Card>,
}

impl Default for Book {
    fn default() -> Self {
        Book::builtin()
    }
}

/// The delve's own cards, as files a friend can copy.
pub(crate) const BUILTIN: &[(&str, &str)] = &[
    (
        "gold",
        include_str!("../../../assets/dungeon/cards/gold.card"),
    ),
    (
        "potion",
        include_str!("../../../assets/dungeon/cards/potion.card"),
    ),
    (
        "bomb",
        include_str!("../../../assets/dungeon/cards/bomb.card"),
    ),
    (
        "mail",
        include_str!("../../../assets/dungeon/cards/mail.card"),
    ),
    (
        "heart",
        include_str!("../../../assets/dungeon/cards/heart.card"),
    ),
    (
        "bow",
        include_str!("../../../assets/dungeon/cards/bow.card"),
    ),
    (
        "crossbow",
        include_str!("../../../assets/dungeon/cards/crossbow.card"),
    ),
    (
        "handgonne",
        include_str!("../../../assets/dungeon/cards/handgonne.card"),
    ),
    (
        "ember-quiver",
        include_str!("../../../assets/dungeon/cards/ember-quiver.card"),
    ),
    (
        "twin-string",
        include_str!("../../../assets/dungeon/cards/twin-string.card"),
    ),
    (
        "bodkin-heads",
        include_str!("../../../assets/dungeon/cards/bodkin-heads.card"),
    ),
    (
        "swift-spurs",
        include_str!("../../../assets/dungeon/cards/swift-spurs.card"),
    ),
    (
        "quick-draw",
        include_str!("../../../assets/dungeon/cards/quick-draw.card"),
    ),
    (
        "leech-fang",
        include_str!("../../../assets/dungeon/cards/leech-fang.card"),
    ),
    (
        "holy-water",
        include_str!("../../../assets/dungeon/cards/holy-water.card"),
    ),
    (
        "thunder-scroll",
        include_str!("../../../assets/dungeon/cards/thunder-scroll.card"),
    ),
    (
        "knights-blade",
        include_str!("../../../assets/dungeon/cards/knights-blade.card"),
    ),
    (
        "bone",
        include_str!("../../../assets/dungeon/cards/bone.card"),
    ),
    (
        "wax",
        include_str!("../../../assets/dungeon/cards/wax.card"),
    ),
    (
        "ore",
        include_str!("../../../assets/dungeon/cards/ore.card"),
    ),
    (
        "gem",
        include_str!("../../../assets/dungeon/cards/gem.card"),
    ),
    (
        "ember",
        include_str!("../../../assets/dungeon/cards/ember.card"),
    ),
    (
        "scale",
        include_str!("../../../assets/dungeon/cards/scale.card"),
    ),
    (
        "dodge-roll",
        include_str!("../../../assets/dungeon/cards/dodge-roll.card"),
    ),
    (
        "kite-shield",
        include_str!("../../../assets/dungeon/cards/kite-shield.card"),
    ),
];

impl Book {
    pub(crate) fn builtin() -> Book {
        let mut book = Book {
            cards: Vec::new(),
            revision: 0,
        };
        for (id, raw) in BUILTIN {
            let checked = check(id, raw).expect("built-in cards pass their own checker");
            book.insert(checked.card);
        }
        book
    }

    pub(crate) fn get(&self, id: &str) -> Option<&Card> {
        self.cards
            .binary_search_by(|c| c.id.as_str().cmp(id))
            .ok()
            .map(|i| &self.cards[i])
    }

    /// Add or replace a card; false when the book is full.
    pub(crate) fn insert(&mut self, card: Card) -> bool {
        match self.cards.binary_search_by(|c| c.id.cmp(&card.id)) {
            Ok(i) => {
                if self.cards[i] != card {
                    self.revision = self.revision.wrapping_add(1);
                }
                self.cards[i] = card;
                true
            }
            Err(i) if self.cards.len() < MAX_BOOK => {
                self.cards.insert(i, card);
                self.revision = self.revision.wrapping_add(1);
                true
            }
            Err(_) => false,
        }
    }

    fn roll(&self, rng: &mut Rng, keep: impl Fn(&Card) -> u32) -> Option<String> {
        let table: Vec<(usize, u32)> = self
            .cards
            .iter()
            .enumerate()
            .map(|(i, c)| (i, keep(c)))
            .filter(|&(_, w)| w > 0)
            .collect();
        (!table.is_empty()).then(|| self.cards[rng.pick(&table)].id.clone())
    }

    /// An ordinary drop: any card but the delve's materials.
    pub(crate) fn roll_drop(&self, rng: &mut Rng, pack: Pack) -> Option<String> {
        self.roll(rng, |c| {
            if c.is_spoil() || !c.drops_in(pack) {
                0
            } else {
                c.drop
            }
        })
    }

    /// The delve's own material, on its separate roll.
    pub(crate) fn roll_spoil(&self, rng: &mut Rng, pack: Pack) -> Option<String> {
        self.roll(rng, |c| {
            if c.is_spoil() && c.drops_in(pack) {
                c.drop
            } else {
                0
            }
        })
    }

    /// A chest holds one prize plus two ordinary drops.
    pub(crate) fn roll_chest(&self, rng: &mut Rng, pack: Pack) -> Vec<String> {
        let prize = self.roll(rng, |c| if c.drops_in(pack) { c.chest } else { 0 });
        let rest = [self.roll_drop(rng, pack), self.roll_drop(rng, pack)];
        prize
            .into_iter()
            .chain(rest.into_iter().flatten())
            .collect()
    }
}

/// Read every `*.card` in `dir` into `book`. Returns one line per file.
pub(crate) fn load_dir(dir: &std::path::Path, book: &mut Book) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "card") && p.is_file())
        .collect();
    paths.sort();
    let mut report = Vec::new();
    for path in paths.into_iter().take(MAX_BOOK) {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let raw = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() as usize <= MAX_BYTES => std::fs::read_to_string(&path),
            _ => {
                report.push(format!("{stem}: over {MAX_BYTES} bytes, skipped"));
                continue;
            }
        };
        match raw.map_err(|e| e.to_string()) {
            Ok(raw) => match check(stem, &raw) {
                Ok(checked) => {
                    let name = checked.card.name.clone();
                    if book.insert(checked.card) {
                        report.push(if checked.notes.is_empty() {
                            format!("{stem}: {name}")
                        } else {
                            format!("{stem}: {name} ({})", checked.notes.join("; "))
                        });
                    } else {
                        report.push(format!("{stem}: the book is full ({MAX_BOOK} cards)"));
                    }
                }
                Err(rejected) => report.push(format!("{stem}: {}", rejected.errors.join("; "))),
            },
            Err(error) => report.push(format!("{stem}: {error}")),
        }
    }
    report
}

/// The card format, for a friend or their AI.
/// The effects a held card (and so a known wish's tier) may carry: word,
/// range and meaning, a line each.
pub(crate) fn hold_effects() -> String {
    SPECS
        .iter()
        .filter(|s| s.hold && s.cap > 0)
        .map(|s| format!("{} {}-{}: {}", s.word, s.low, s.high, s.says))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn rules() -> String {
    let effects = SPECS
        .iter()
        .map(|s| {
            format!(
                "  {:<7} {}–{}  {}{}  ({})",
                s.word,
                s.low,
                s.high,
                match (s.hold, s.spend) {
                    (true, true) => "any card",
                    (true, false) => "hold",
                    _ => "take/play",
                },
                if s.cap > 0 {
                    format!(", deck cap {}", s.cap)
                } else {
                    String::new()
                },
                s.says
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "A delve card is a text file `<id>.card` in .angel/dungeon/cards/ (the file name is its id).\n\
name   <up to {MAX_NAME} chars>\n\
kind   take | hold | play | arm | guard | spell   (take: spent on pickup · hold: lasts the run · play: hand slot, keys 1–4 · spell: reusable slot, Z/B/N · arm: your weapon · guard: on space, `guard roll`, `guard shield`, or `guard wall width=2-8 drain=10-50 empower=0-40` — a held barrier that stops monsters' shots, lets friends' shots through (+empower%), and drains mana per second)\n\
rarity common | rare | relic\n\
text   <up to {MAX_TEXT} chars of flavour>\n\
by     <your name>\n\
drop   <0–100 weight in monster drops>   chest <0–100 weight as a chest prize>\n\
where  crypt | mines | keep | anywhere   (which delve it drops in)\n\
<effect> <number>   at most {MAX_EFFECTS}:\n{effects}\n\
Spell cards: `cooldown <1–120 seconds>` is required; use heal, bombs, nova, ward or `cast bolt|ring|meteor`. Shapes carry your shot traits.\n\
Arm cards: `arm bow|crossbow|handgonne`, or Forge rune lines (bolt/spread/melee/look; see /dungeon forge).\n\
art    then up to {ART} rows of up to {ART} inks; `.` is black paper. Inks:\n\
  greys  k K Z X g j G J h i H   greens f D F E e l N m L A M y C Y\n\
  wood   n b I B p P r R o O t T   stone  s x S u U v V W q Q z\n\
  signal 0 1 2 3 w (blues) a 4 @ 5 6 $ c 9 (golds) 8 7 (reds) — signal inks glow; use them for what matters\n\
New spells in .angel/dungeon/cards hot-load into the host's first empty slot with a flourish; full slots drop them at the host's feet. /dungeon cards reload reads the folder again; /dungeon card <file> adds one and drops it at your feet."
    )
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_cards__tests.rs"]
mod tests;
