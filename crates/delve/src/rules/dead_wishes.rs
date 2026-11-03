//! A summoner's wishes for his dead (the user, 2026-10-23, while friends
//! played the Gravecaller: "the summoner needs to be able to wish things
//! for his minions, and or himself, maybe a toggle?"; one had said "make
//! my ghouls bright pink" and been written a summon spell).
//!
//! On a summoner's open scroll, 4 turns whom the wish is for: him, his
//! dead, or both ([`Target`]). It opens on his dead when the words name
//! them ([`named`]), else on him. A wish for the dead is granted as any
//! wish is (the same card up the same ladder, spent, refused and given
//! back the same way) and kept as theirs (`Hero::dead_wishes`):
//!
//! - its held words go on each of his ghouls, wardens, totems and
//!   zombies, not on him ([`Gifts`]): harder and quicker blows, a quicker
//!   pace, more health, mail that turns a share of every hit, blows that
//!   mend; the totem's grave-fire takes the shot words (pierce, homing,
//!   more bolts, fire, frost...);
//! - its worn look is drawn on each of them, fitted to their bodies; a
//!   look over the whole body of a wish that is only a colour ("Bright
//!   Pink") colours their bodies instead ([`colours_body`]).
//!
//! For both, each gets the look and half of every number, rounded up. A
//! wish for the dead that comes out as a spell (or an arm, or a guard) is
//! his, as any wish is: a spell takes his wish slot.

use super::hireling::{AllyKind, Hireling};
use super::*;
use std::borrow::Cow;

/// Whom a wish is for, on a summoner's scroll.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// The knight himself, as every wish was.
    #[default]
    You,
    /// His raised dead.
    Dead,
    /// Both: each the look, and half of every number.
    Both,
}

impl Target {
    /// The next on the scroll's toggle: you, your dead, both, and round.
    pub fn next(self) -> Target {
        match self {
            Target::You => Target::Dead,
            Target::Dead => Target::Both,
            Target::Both => Target::You,
        }
    }

    /// As the scroll writes it.
    pub fn word(self) -> &'static str {
        match self {
            Target::You => "you",
            Target::Dead => "your dead",
            Target::Both => "both",
        }
    }

    /// His dead have a share of it.
    pub fn theirs(self) -> bool {
        self != Target::You
    }

    /// His share of a number `n`, and theirs.
    pub fn his(self, n: u32) -> u32 {
        match self {
            Target::You => n,
            Target::Dead => 0,
            Target::Both => n.div_ceil(2),
        }
    }

    pub fn their(self, n: u32) -> u32 {
        match self {
            Target::You => 0,
            Target::Dead => n,
            Target::Both => n.div_ceil(2),
        }
    }
}

/// Words that name a summoner's dead, whole words or phrases: a wish in
/// them opens on his dead.
const DEAD: [&str; 30] = [
    "ghoul", "ghouls", "zombie", "zombies", "skeleton", "skeletons", "skelly", "skellies", "minion",
    "minions", "warden", "wardens", "totem", "totems", "undead", "army", "armies", "horde", "summons",
    "pet", "pets", "thrall", "thralls", "servants", "my dead", "the dead", "his dead", "our dead",
    "raised dead", "corpses",
];

/// Whether `words` name a summoner's dead ("make my ghouls bright pink").
pub fn named(words: &str) -> bool {
    let plain: String = words
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let plain = format!(" {} ", plain.split_whitespace().collect::<Vec<_>>().join(" "));
    DEAD.iter().any(|w| plain.contains(&format!(" {w} ")))
}

/// Colour words a look's name may carry: a whole-body look of a wish that
/// names one colours a body (`colours_body`).
const COLOURS: [&str; 46] = [
    "red", "crimson", "scarlet", "ruby", "blood", "pink", "rose", "rosy", "magenta", "fuchsia",
    "purple", "violet", "lilac", "lavender", "blue", "azure", "navy", "cyan", "teal", "turquoise",
    "green", "emerald", "jade", "lime", "yellow", "gold", "golden", "amber", "orange", "copper",
    "bronze", "brown", "black", "white", "silver", "grey", "gray", "ivory", "pale", "dark",
    "rainbow", "neon", "glowing", "colour", "mauve", "blush",
];

/// The two dyes a colour's name can ask for, purple and pink: the realm
/// had neither until its violet and pink banks, and a body wished either
/// colour is drawn in its dye.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dye {
    Violet,
    Pink,
}

/// The words of each dye, its own and its neighbours'.
const DYES: [(Dye, &[&str]); 2] = [
    (Dye::Violet, &["purple", "violet", "lilac", "lavender", "mauve", "plum", "amethyst", "grape", "aubergine", "orchid", "indigo"]),
    (Dye::Pink, &["pink", "rose", "rosy", "magenta", "fuchsia", "blush", "flamingo", "bubblegum"]),
];

/// The dye a name asks for, if a word of it is a dye's ("Bright Pink",
/// "Lavender II"): its first such word's, so "Pink and Purple" is pink.
pub fn dye_of(name: &str) -> Option<Dye> {
    name.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .find_map(|w| DYES.iter().find(|(_, words)| words.contains(&w)).map(|&(dye, _)| dye))
}

/// How a name shades its colour: `2` for a pale one ("Pale Pink", "Soft
/// Lilac"), `1` for a bright one ("Bright Pink", "Hot Pink"), `-1` for a
/// deep one ("Dark Purple"), else `0`.
pub fn lightness_of(name: &str) -> i8 {
    let lower = name.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).collect();
    let said = |list: &[&str]| words.iter().any(|w| list.contains(w));
    if said(&["pale", "light", "soft", "pastel", "baby"]) {
        2
    } else if said(&["bright", "hot", "neon", "glowing"]) {
        1
    } else if said(&["deep", "dark"]) {
        -1
    } else {
        0
    }
}

/// Whether a name (a card's, "Bright Pink") names a colour.
pub fn names_colour(name: &str) -> bool {
    name.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| COLOURS.contains(&w) || w == "color" || w == "coloured" || w == "colored")
}

/// The words a colour's name may carry besides its colours: its tier, its
/// shade and the asking ("Make Me Bright Pink II").
const COLOUR_ONLY: [&str; 36] = [
    "i", "ii", "iii", "iv", "v", "vi", "bright", "deep", "hot", "light", "soft", "make", "me",
    "my", "all", "over", "whole", "body", "skin", "knight", "shade", "tone", "dye", "dyed",
    "paint", "painted", "and", "of", "the", "a", "in", "to", "be", "color", "coloured", "colored",
];

/// Whether a name is a colour and nothing else ("Bright Pink II"): a name
/// with a thing in it ("Purple Drank Armor") is the thing, in that colour.
pub(crate) fn only_colour(name: &str) -> bool {
    let lower = name.to_lowercase();
    names_colour(name)
        && lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .all(|w| COLOURS.contains(&w) || COLOUR_ONLY.contains(&w))
}

/// Whether held card `card`'s worn look colours a body rather than sits
/// on it: drawn over the whole figure (placed on no part of it, or on the
/// body), on a card whose name is only a colour.
pub fn colours_body(card: &Card) -> bool {
    card.looks
        .worn
        .as_ref()
        .is_some_and(|s| s.at.as_deref().is_none_or(|at| at == "body" || at == "whole"))
        && only_colour(&card.name)
}

/// The wish a ladder's card is a tier of (`wish-<id>-<tier>`).
pub fn wish_of(card: &str) -> Option<&str> {
    let (id, tier) = card.strip_prefix("wish-")?.rsplit_once('-')?;
    (!id.is_empty() && !tier.is_empty() && tier.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// The health a card adds.
fn health_of(card: &Card) -> u32 {
    card.effects
        .iter()
        .map(|e| match *e {
            cards::Effect::MaxHp(v) => v,
            _ => 0,
        })
        .sum()
}

/// `card` at half its numbers, rounded up: a wish for both, his share and
/// theirs. A volley comes round half as often.
fn halved(card: &Card) -> Card {
    use cards::Effect::*;
    let half = |v: u32| v.div_ceil(2);
    let mut card = card.clone();
    for effect in &mut card.effects {
        *effect = match *effect {
            MaxHp(v) => MaxHp(half(v)),
            Damage(v) => Damage(half(v)),
            Rate(v) => Rate(half(v)),
            Speed(v) => Speed(half(v)),
            Pierce(v) => Pierce(half(v)),
            Shots(v) => Shots(half(v)),
            Armor(v) => Armor(half(v)),
            Vamp(v) => Vamp(half(v)),
            Volley(v) => Volley(v.saturating_mul(2)),
            Bounce(v) => Bounce(half(v)),
            Homing(v) => Homing(half(v)),
            Chain(v) => Chain(half(v)),
            Mend(v) => Mend(half(v)),
            Orbit(v) => Orbit(half(v)),
            Burst(v) => Burst(half(v)),
            Fork(v) => Fork(half(v)),
            Fart(v) => Fart(half(v)),
            Size(v) => Size(half(v)),
            Knock(v) => Knock(half(v)),
            Burn(v) => Burn(half(v)),
            Chill(v) => Chill(half(v)),
            Poison(v) => Poison(half(v)),
            Velocity(v) => Velocity(half(v)),
            other => other,
        };
    }
    card
}

/// What a summoner's wishes give each of his dead: the held words, counted
/// as a knight's are (each word's wish cap, the floor's budget), and
/// health besides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gifts {
    pub bonus: cards::Bonus,
    pub hp: u32,
}

/// A point of vamp mends one of the dead this much for each blow it lands.
pub const DEAD_VAMP: u32 = 3;

impl Gifts {
    /// Nothing given: left out of a save and the wire.
    pub fn none(&self) -> bool {
        *self == Gifts::default()
    }

    /// What `held` gives each of the dead on `floor`.
    pub(super) fn of<'a>(held: impl IntoIterator<Item = &'a Card>, floor: Option<u32>) -> Gifts {
        let held: Vec<&Card> = held.into_iter().collect();
        let hp = held.iter().map(|c| health_of(c)).sum();
        Gifts { bonus: cards::Bonus::within(held, floor), hp }
    }

    /// A blow's weight, harder by its damage.
    pub fn hit(&self, weight: u32) -> u32 {
        weight * (100 + self.bonus.damage) / 100
    }

    /// The rest after a blow, quicker by its rate.
    pub fn rest(&self, ticks: u32) -> u32 {
        (ticks * 100 / (100 + self.bonus.rate)).max(1)
    }

    /// Its pace, quicker by its speed.
    pub fn pace(&self) -> f32 {
        (100 + self.bonus.speed) as f32 / 100.0
    }

    /// What a hit of `damage` takes, after its mail turns a share aside.
    pub fn taken(&self, damage: u32) -> u32 {
        let turned = super::hero::armor_turns(self.bonus.armor);
        if damage == 0 { 0 } else { (damage * (100 - turned) / 100).max(1) }
    }

    /// The totem's grave-fire as the shot words make it: its traits, how
    /// far it pierces, and how many bolts fly.
    pub fn fire(&self) -> (ShotTraits, u8, u32) {
        let mut traits = ShotTraits::of(&self.bonus);
        // Its own mends are its blows' (`hireling`); nothing comes home.
        traits.mend = 0;
        traits.boomerang = false;
        traits.fire = traits.burn > 0;
        (traits, self.bonus.pierce.min(8) as u8, 1 + self.bonus.shots.min(2))
    }
}

impl Hero {
    /// Whom this knight's wish `wish` (by id) is for: a summoner's may be
    /// his dead's; every other knight's is their own.
    pub fn wished_for(&self, wish: &str) -> Target {
        if self.dead_wishes.is_empty() || !minions::summoner(self) {
            return Target::You;
        }
        self.dead_wishes.get(wish).copied().unwrap_or_default()
    }

    /// Whom held card `card` (by id) is for: a wish's card goes with its
    /// wish.
    pub fn card_for(&self, card: &str) -> Target {
        if self.dead_wishes.is_empty() {
            return Target::You;
        }
        wish_of(card).map_or(Target::You, |w| self.wished_for(w))
    }

    /// What the knight holds for himself, and what for his dead; a wish for
    /// both at half its numbers in each.
    pub(super) fn held_split<'a>(&self, book: &'a Book) -> (Vec<Cow<'a, Card>>, Vec<Cow<'a, Card>>) {
        let (mut his, mut theirs) = (Vec::new(), Vec::new());
        for card in self.deck.iter().filter_map(|id| book.get(id)) {
            match self.card_for(&card.id) {
                Target::You => his.push(Cow::Borrowed(card)),
                Target::Dead => theirs.push(Cow::Borrowed(card)),
                Target::Both => {
                    let half = halved(card);
                    his.push(Cow::Owned(half.clone()));
                    theirs.push(Cow::Owned(half));
                }
            }
        }
        (his, theirs)
    }

    /// The held cards his dead wear, oldest first, the newest three: a
    /// wish's for them or for both, with a worn look.
    pub fn dead_looks<'a>(&self, book: &'a Book) -> Vec<&'a Card> {
        if self.dead_wishes.is_empty() || !minions::summoner(self) {
            return Vec::new();
        }
        let all: Vec<&Card> = self
            .deck
            .iter()
            .filter_map(|id| book.get(id))
            .filter(|c| c.kind == cards::Kind::Hold && c.looks.worn.is_some() && self.card_for(&c.id).theirs())
            .collect();
        all[all.len().saturating_sub(3)..].to_vec()
    }

    /// The look his totem's grave-fire takes: the newest held card for his
    /// dead that dresses shots, if any.
    pub fn dead_shot_look(&self, book: &Book) -> Option<(String, String)> {
        if self.dead_wishes.is_empty() || !minions::summoner(self) {
            return None;
        }
        self.deck
            .iter()
            .rev()
            .filter_map(|id| book.get(id))
            .find(|c| c.kind == cards::Kind::Hold && c.looks.dresses_shots() && self.card_for(&c.id).theirs())
            .and_then(Projectile::look_of)
    }
}

/// Whether `kind` is one of a summoner's own dead: what his wishes for
/// them reach.
pub fn his_own(kind: AllyKind) -> bool {
    kind.golem() || matches!(kind, AllyKind::Ghoul | AllyKind::Warden | AllyKind::Totem | AllyKind::Zombie | AllyKind::Skeleton)
}

impl Run {
    /// Knight `id` turns whom the wish on their open scroll is for (4):
    /// you, your dead, both. Only a summoner's scroll has the toggle.
    pub fn turn_wish(&mut self, id: u32) -> bool {
        let Some(target) = self
            .players
            .get_mut(&id)
            .and_then(|h| h.offer.as_mut())
            .and_then(|o| o.target.as_mut())
        else {
            return false;
        };
        *target = target.next();
        self.sounds.push("card_pickup");
        true
    }

    /// The words knight `id` spoke for the scroll open on their view, when
    /// it opened on a known wish found in them (the scroll keeps only its
    /// name): words that name a summoner's dead turn it to them.
    pub fn heard(&mut self, id: u32, words: &str) {
        if named(words)
            && let Some(target) = self
                .players
                .get_mut(&id)
                .and_then(|h| h.offer.as_mut())
                .and_then(|o| o.target.as_mut())
        {
            *target = Target::Dead;
        }
    }

    /// Knight `id`'s wish `wish` is to be granted for `target`: kept as his
    /// dead's (or both's), or as his alone again, and the health its tier
    /// already held gave goes with it. Only a summoner's held wish may be
    /// his dead's; any other is the knight's own. Whom it was for, to put
    /// back when the grant is refused.
    pub fn aim_wish(&mut self, id: u32, wish: &phrasebook::Wish, target: Target) -> Target {
        let Some(hero) = self.players.get_mut(&id) else {
            return Target::You;
        };
        let was = hero.wished_for(&wish.id);
        let target = if minions::summoner(hero) && wish.kind == "hold" { target } else { Target::You };
        if target == was {
            return was;
        }
        let held: u32 = hero
            .deck
            .iter()
            .filter(|c| wish_of(c) == Some(wish.id.as_str()))
            .filter_map(|c| self.book.get(c))
            .map(health_of)
            .sum();
        let (before, after) = (was.his(held), target.his(held));
        if after >= before {
            hero.max_hp += after - before;
            hero.hp = (hero.hp + after - before).min(hero.max_hp);
        } else {
            hero.max_hp = hero.max_hp.saturating_sub(before - after).max(1);
            hero.hp = hero.hp.min(hero.max_hp);
        }
        if target == Target::You {
            hero.dead_wishes.remove(&wish.id);
        } else {
            hero.dead_wishes.insert(wish.id.clone(), target);
        }
        hero.rebonus(&self.book);
        was
    }

    /// What his wishes give minion `hire`: its caller's gifts to his dead,
    /// none for any other.
    pub(super) fn gifts_for(&self, hire: &Hireling) -> Gifts {
        hire.summoner
            .filter(|_| his_own(hire.kind))
            .and_then(|id| self.players.get(&id))
            .map(|h| h.dead_gifts)
            .unwrap_or_default()
    }

    /// Each summoner's dead as hardy as his wishes make them: their own
    /// health and the gifts' besides. Health gained is mended with it, a
    /// felled one stays down, and health lost comes off the top.
    pub(super) fn fit_dead(&mut self) {
        let mut raised = std::mem::take(&mut self.minions.raised);
        for minion in &mut raised {
            let Some(base) = minion.kind.health() else {
                continue;
            };
            let most = self.minion_spell_health(minion, base) + self.gifts_for(minion).hp;
            if most > minion.max_hp && !minion.down() {
                minion.hp += most - minion.max_hp;
            }
            minion.max_hp = most;
            minion.hp = minion.hp.min(most);
        }
        raised.append(&mut self.minions.raised);
        self.minions.raised = raised;
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_dead_wishes__tests.rs"]
mod tests;
