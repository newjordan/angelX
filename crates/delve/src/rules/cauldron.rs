//! The cauldron: when a floor's guardian falls, a black iron pot rises
//! beside its stairs, already on the boil. A knight steps up, presses E,
//! picks two of their spells with the spell keys (1–4), and presses E again
//! to throw them in. The pot roils, puffs, and a new spell rises out of it:
//! one spell where there were two, themed by what went in (fire and frost
//! make a Steam Burst). Now and then it misfires into something silly and
//! harmless — the brewer a frog for five seconds, a burst of confetti, the
//! hiccups — and hands both cards back.
//!
//! Once a knight a floor, and free: a guardian's fall buys every knight one
//! brew, the way it buys every knight a super wish, so nobody stands at the
//! pot feeding it gold and no seat waits on another's purse. A brew is
//! decided by the run's seed, the floor, the knight and the two cards (in
//! either order), so the host and every seat agree and a brew replays.
//!
//! What it makes is an ordinary spell card in the run's book, written in the
//! card vocabulary and passed through the card checker (`cards::check`),
//! so it obeys every bound a hand-written spell does. A brew is at most
//! about two Cabbage Storms' worth of power a second (`POWER_BUDGET`, held by
//! the tests), and it takes two slots' spells to make, so it is never a
//! way past the four-slot limit.
//!
//! The recipe's spell is a stand-in. The moment it rises the run asks the
//! host's model for a spell of the brew's own (`Brewing`, drained by the
//! cockpit): it is shown both cards whole, what the knight holds besides,
//! and every spell name and effect mix already taken, and writes one new
//! card, a true fusion of the two. If that card passes the card checker
//! and the pot's own bounds (`faults`: its power budget, a name and a mix
//! of its own, no cabbages unless a cabbage went in), it takes the
//! stand-in's place in the knight's slot and goes into the run's book
//! under an id of its own, so it saves with the run and is never handed to
//! another knight's brew. A model that answers badly leaves the stand-in.

use super::cards::{self, Card, Effect, Rarity, Shape};
use super::*;

pub const REACH: f32 = 3.6;
pub const LET_GO: f32 = REACH + 2.0;
pub const GIRTH: f32 = 2.0;
pub const GIRTH_NORTH: f32 = 1.0;
pub const SLIDE: f32 = 0.18;
pub const RISE_DELAY: u32 = 3 * HZ;
pub const RISE_TICKS: u32 = HZ;
pub const SPLASH: [u32; 2] = [9, 18];
pub const PUFF: u32 = 2 * HZ;
pub const RISEN: u32 = PUFF + HZ;
pub const SHOWN: u32 = PUFF + 4 * HZ;
pub const FROG_TICKS: u32 = 5 * HZ;
pub const HICCUP_TICKS: u32 = 6 * HZ;
pub const MISFIRE: u64 = 12;
pub const COMMON: u64 = 62;
pub const RARE: u64 = 92;
pub const POWER_BUDGET: f32 = 29.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Essence {
    Fire,
    Frost,
    Garden,
    Trick,
    Life,
    Arcane,
}

impl Essence {
    pub const ALL: [Essence; 6] = [
        Essence::Fire,
        Essence::Frost,
        Essence::Garden,
        Essence::Trick,
        Essence::Life,
        Essence::Arcane,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Essence::Fire => "fire",
            Essence::Frost => "frost",
            Essence::Garden => "garden",
            Essence::Trick => "trick",
            Essence::Life => "life",
            Essence::Arcane => "arcane",
        }
    }

    pub fn of(card: &Card) -> Essence {
        let words = format!("{} {}", card.id, card.name).to_ascii_lowercase();
        let says = |list: &[&str]| list.iter().any(|w| words.contains(w));
        if says(&["fire", "flame", "ember", "burn", "blaze", "cinder", "meteor", "steam"]) {
            return Essence::Fire;
        }
        if says(&["frost", "ice", "rime", "snow", "cold", "freez", "chill", "hail", "winter"]) {
            return Essence::Frost;
        }
        if let Some((colour, _)) = &card.looks.style {
            match colour.as_str() {
                "ember" => return Essence::Fire,
                "ice" => return Essence::Frost,
                "venom" => return Essence::Garden,
                "void" => return Essence::Trick,
                "gold" | "rose" => return Essence::Life,
                "steel" => return Essence::Arcane,
                _ => {}
            }
        }
        for effect in &card.effects {
            match effect {
                Effect::Cast(Shape::Meteor) | Effect::Nova(_) | Effect::Bombs(_) => {
                    return Essence::Fire;
                }
                Effect::Area(area) => {
                    use cards::Element;
                    return match area.element {
                        Element::Fire => Essence::Fire,
                        Element::Frost => Essence::Frost,
                        Element::Poison | Element::Earth => Essence::Garden,
                        Element::Holy => Essence::Life,
                        Element::Storm | Element::Arcane => Essence::Arcane,
                    };
                }
                Effect::Cabbages(_) | Effect::Hedge(_) | Effect::Frogs(_) => {
                    return Essence::Garden;
                }
                Effect::Imposture(_) | Effect::Summon(..) | Effect::Excommunicate(_) => {
                    return Essence::Trick;
                }
                Effect::Patience(_) => return Essence::Life,
                Effect::Vesper(_) => return Essence::Arcane,
                Effect::Heal(_) | Effect::Ward(_) => return Essence::Life,
                Effect::Cast(_) => return Essence::Arcane,
                _ => {}
            }
        }
        Essence::Arcane
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Line {
    N(&'static str, u32),
    Cast(&'static str),
    One(&'static str),
    Summon(u32, &'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    pub pair: (Essence, Essence),
    pub id: &'static str,
    pub name: &'static str,
    pub text: &'static str,
    pub look: (&'static str, &'static str),
    pub cooldown: u32,
    pub lines: &'static [Line],
}

use Essence::{Arcane, Fire, Frost, Garden, Life, Trick};

pub const RECIPES: [Recipe; 21] = [
    Recipe { pair: (Fire, Fire), id: "wildfire", name: "Wildfire", text: "The room catches. You do not.", look: ("ember", "star"), cooldown: 16, lines: &[Line::N("nova", 40), Line::N("bombs", 1)] },
    Recipe { pair: (Fire, Frost), id: "steam-burst", name: "Steam Burst", text: "Scalding steam bursts out of you, and hides you a breath.", look: ("ice", "wave"), cooldown: 14, lines: &[Line::N("nova", 35), Line::N("ward", 1)] },
    Recipe { pair: (Fire, Garden), id: "hot-cabbages", name: "Hot Cabbages", text: "Six cabbages, roasted on the way down.", look: ("ember", "orb"), cooldown: 16, lines: &[Line::N("cabbages", 6), Line::N("nova", 20)] },
    Recipe { pair: (Fire, Trick), id: "smokescreen", name: "Smokescreen", text: "A puff, a bang, and a knight who is not you.", look: ("void", "orb"), cooldown: 28, lines: &[Line::One("imposture"), Line::N("bombs", 1)] },
    Recipe { pair: (Fire, Life), id: "phoenix-tea", name: "Phoenix Tea", text: "Hot enough to mend you and singe the room.", look: ("gold", "star"), cooldown: 20, lines: &[Line::N("heal", 20), Line::N("nova", 25)] },
    Recipe { pair: (Fire, Arcane), id: "comet", name: "Comet", text: "A heavy burning ball, and the room shakes.", look: ("ember", "orb"), cooldown: 15, lines: &[Line::Cast("meteor"), Line::N("nova", 20)] },
    Recipe { pair: (Frost, Frost), id: "deep-winter", name: "Deep Winter", text: "A ring of ice shards, and a coat of rime on you.", look: ("ice", "shard"), cooldown: 16, lines: &[Line::Cast("ring"), Line::N("ward", 2)] },
    Recipe { pair: (Frost, Garden), id: "frost-hedge", name: "Frost Hedge", text: "A hedge of white thorn, and a cold you hide in.", look: ("ice", "shard"), cooldown: 24, lines: &[Line::One("hedge"), Line::N("ward", 2)] },
    Recipe { pair: (Frost, Trick), id: "ice-double", name: "Ice Double", text: "A knight of ice steps out, shedding shards.", look: ("ice", "shard"), cooldown: 28, lines: &[Line::One("imposture"), Line::Cast("ring")] },
    Recipe { pair: (Frost, Life), id: "cool-compress", name: "Cool Compress", text: "Cold water on a hot wound.", look: ("ice", "orb"), cooldown: 22, lines: &[Line::N("heal", 30), Line::N("ward", 1)] },
    Recipe { pair: (Frost, Arcane), id: "hailstorm", name: "Hailstorm", text: "Hail flies every way at once.", look: ("ice", "shard"), cooldown: 13, lines: &[Line::Cast("ring"), Line::N("ward", 1)] },
    Recipe { pair: (Garden, Garden), id: "kitchen-garden", name: "Kitchen Garden", text: "A crop of cabbages and the hedge to keep them in.", look: ("venom", "orb"), cooldown: 24, lines: &[Line::N("cabbages", 6), Line::One("hedge")] },
    Recipe { pair: (Garden, Trick), id: "scarecrow", name: "Scarecrow", text: "A false knight in a field of falling cabbages.", look: ("venom", "orb"), cooldown: 26, lines: &[Line::One("imposture"), Line::N("cabbages", 5)] },
    Recipe { pair: (Garden, Life), id: "herb-garden", name: "Herb Garden", text: "A hedge rises, and the herbs in it mend you.", look: ("venom", "star"), cooldown: 26, lines: &[Line::One("hedge"), Line::N("heal", 25)] },
    Recipe { pair: (Garden, Arcane), id: "bramble-imps", name: "Bramble Imps", text: "Three thorny imps hunt and burst.", look: ("venom", "arrow"), cooldown: 14, lines: &[Line::Summon(3, "swarm")] },
    Recipe { pair: (Trick, Trick), id: "mirror-hall", name: "Mirror Hall", text: "A false you, and two guards who catch shots.", look: ("void", "star"), cooldown: 30, lines: &[Line::One("imposture"), Line::Summon(2, "guard")] },
    Recipe { pair: (Trick, Life), id: "second-self", name: "Second Self", text: "Your double draws the fire while you mend.", look: ("rose", "star"), cooldown: 28, lines: &[Line::One("imposture"), Line::N("heal", 20)] },
    Recipe { pair: (Trick, Arcane), id: "pocket-tower", name: "Pocket Tower", text: "A small tower that shoots, and a moment's cover.", look: ("steel", "arrow"), cooldown: 18, lines: &[Line::Summon(1, "turret"), Line::N("ward", 1)] },
    Recipe { pair: (Life, Life), id: "abbey-kettle", name: "Abbey Kettle", text: "Strong tea and a stronger blessing.", look: ("gold", "star"), cooldown: 24, lines: &[Line::N("heal", 40), Line::N("ward", 2)] },
    Recipe { pair: (Life, Arcane), id: "ward-lattice", name: "Ward Lattice", text: "A lattice of light holds you, and flies apart.", look: ("gold", "wave"), cooldown: 20, lines: &[Line::N("ward", 3), Line::Cast("ring")] },
    Recipe { pair: (Arcane, Arcane), id: "twin-moons", name: "Twin Moons", text: "Two moons circle you, and stars fly out.", look: ("steel", "star"), cooldown: 18, lines: &[Line::Summon(2, "guard"), Line::Cast("bolt")] },
];

pub fn recipe(a: Essence, b: Essence) -> &'static Recipe {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    RECIPES
        .iter()
        .find(|r| r.pair == (a, b))
        .expect("every pair of essences has a recipe")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Common,
    Potent,
    Perfect,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Common, Tier::Potent, Tier::Perfect];

    pub fn rarity(self) -> Rarity {
        match self {
            Tier::Common => Rarity::Common,
            Tier::Potent => Rarity::Rare,
            Tier::Perfect => Rarity::Relic,
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Tier::Common => "",
            Tier::Potent => "Potent ",
            Tier::Perfect => "Perfect ",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Tier::Common => "",
            Tier::Potent => "-potent",
            Tier::Perfect => "-perfect",
        }
    }

    fn grow(self, n: u32) -> u32 {
        match self {
            Tier::Common => n,
            Tier::Potent => (n * 5).div_ceil(4),
            Tier::Perfect => (n * 3).div_ceil(2),
        }
    }

    fn recharge(self, s: u32) -> u32 {
        match self {
            Tier::Perfect => s.saturating_sub(2),
            _ => s,
        }
    }
}

pub fn card_text(recipe: &Recipe, tier: Tier) -> String {
    let floor = recipe
        .lines
        .iter()
        .map(|line| match *line {
            Line::Summon(n, how) => {
                tier.grow(n).min(most(how))
                    * match how {
                        "swarm" => 4,
                        "guard" => 5,
                        _ => 10,
                    }
            }
            _ => 0,
        })
        .max()
        .unwrap_or(0);
    let mut out = format!(
        "name {}{}\nkind spell\nrarity {}\ntext {}\ndrop 0\nchest 0\ncooldown {}\nlook {} {}\n",
        tier.prefix(),
        recipe.name,
        tier.rarity().word(),
        recipe.text,
        tier.recharge(recipe.cooldown).max(floor),
        recipe.look.0,
        recipe.look.1,
    );
    for line in recipe.lines {
        let said = match *line {
            Line::N(word, n) => format!("{word} {}", tier.grow(n).min(most(word))),
            Line::Cast(shape) => format!("cast {shape}"),
            Line::One(word) => format!("{word} 1"),
            Line::Summon(n, how) => format!("summon {} {how}", tier.grow(n).min(most(how))),
        };
        out.push_str(&said);
        out.push('\n');
    }
    out
}

fn most(word: &str) -> u32 {
    match word {
        "cabbages" => 8,
        "nova" => 120,
        "heal" => 100,
        "ward" => 5,
        "bombs" => 3,
        "swarm" => 6,
        "guard" => 4,
        "turret" => 2,
        _ => u32::MAX,
    }
}

pub fn brewed_card(recipe: &Recipe, tier: Tier) -> Card {
    let id = format!("brew-{}{}", recipe.id, tier.suffix());
    cards::check(&id, &card_text(recipe, tier))
        .expect("every brew passes the card checker")
        .card
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Misfire {
    Frog,
    Confetti,
    Hiccups,
}

impl Misfire {
    pub const ALL: [Misfire; 3] = [Misfire::Frog, Misfire::Confetti, Misfire::Hiccups];

    pub fn says(self) -> &'static str {
        match self {
            Misfire::Frog => "Misfire! Ribbit.",
            Misfire::Confetti => "Misfire! Confetti.",
            Misfire::Hiccups => "Misfire! Hic.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Spell { card: String, name: String, tier: Tier },
    Misfire(Misfire),
}

impl Outcome {
    pub fn says(&self) -> String {
        match self {
            Outcome::Spell { name, .. } => name.clone(),
            Outcome::Misfire(m) => m.says().into(),
        }
    }
}

pub fn decide(seed: u64, depth: u32, knight: u32, a: &Card, b: &Card) -> (Outcome, Option<Card>) {
    let (first, second) = if a.id <= b.id { (a, b) } else { (b, a) };
    let roll = mix(seed
        ^ u64::from(depth).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ u64::from(knight) << 48
        ^ fnv(&first.id)
        ^ fnv(&second.id).rotate_left(17));
    let r = roll % 100;
    if r < MISFIRE {
        let m = Misfire::ALL[((roll >> 16) % Misfire::ALL.len() as u64) as usize];
        return (Outcome::Misfire(m), None);
    }
    let tier = if r < COMMON {
        Tier::Common
    } else if r < RARE {
        Tier::Potent
    } else {
        Tier::Perfect
    };
    let card = brewed_card(recipe(Essence::of(a), Essence::of(b)), tier);
    (
        Outcome::Spell {
            card: card.id.clone(),
            name: card.name.clone(),
            tier,
        },
        Some(card),
    )
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

pub fn power(card: &Card) -> f32 {
    let points: f32 = card
        .effects
        .iter()
        .map(|e| match *e {
            Effect::Nova(v) => 3.0 * v as f32,
            Effect::Cabbages(n) => 34.0 * n as f32,
            Effect::Cast(Shape::Bolt) => 32.0,
            Effect::Cast(Shape::Ring) => 96.0,
            Effect::Cast(Shape::Meteor) => 96.0,
            Effect::Area(area) => {
                let base = match area.form {
                    cards::Form::Wave | cards::Form::Nova => 120.0,
                    cards::Form::Burst | cards::Form::Field => 140.0,
                    cards::Form::Cone | cards::Form::Pillar => 100.0,
                    cards::Form::Line => 110.0,
                };
                base * (1.0 + 0.33 * f32::from(area.size.saturating_sub(1)))
            }
            Effect::Heal(v) => 2.0 * v as f32,
            Effect::Ward(v) => 25.0 * v as f32,
            Effect::Bombs(v) => 40.0 * v as f32,
            Effect::Hedge(_) | Effect::Imposture(_) | Effect::Frogs(_) => 90.0,
            Effect::Vesper(_) => 40.0,
            Effect::Patience(_) => 60.0,
            Effect::Excommunicate(_) => 70.0,
            Effect::Summon(n, b) => {
                n as f32
                    * match b {
                        imps::Behaviour::Swarm => 30.0,
                        imps::Behaviour::Guard => 25.0,
                        imps::Behaviour::Turret => 80.0,
                    }
            }
            _ => 0.0,
        })
        .sum();
    points / card.cooldown.max(1) as f32
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brewing {
    pub knight: u32,
    pub raid: u64,
    pub depth: u32,
    pub cards: [String; 2],
    pub placeholder: String,
}

impl Brewing {
    pub fn card_id(&self) -> String {
        format!("brewed-{:06x}-f{}-k{}", self.raid & 0xff_ffff, self.depth, self.knight)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Taken {
    pub id: String,
    pub name: String,
    pub mix: Vec<String>,
}

pub fn effect_mix(card: &Card) -> Vec<String> {
    let mut words: Vec<String> = card
        .effects
        .iter()
        .map(|e| match *e {
            Effect::Cast(shape) => format!("cast {}", shape.word()),
            Effect::Area(area) => format!("cast {} {}", area.form.word(), area.element.word()),
            Effect::Summon(_, behaviour) => format!("summon {}", behaviour.word()),
            _ => e.parts().0.to_string(),
        })
        .collect();
    words.sort();
    words.dedup();
    words
}

fn bare(name: &str) -> String {
    let words: Vec<String> = name
        .split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    let mut words: &[String] = &words;
    if let Some((first, rest)) = words.split_first()
        && (first == "potent" || first == "perfect")
    {
        words = rest;
    }
    if let Some((last, rest)) = words.split_last()
        && ["i", "ii", "iii", "iv", "v"].contains(&last.as_str())
    {
        words = rest;
    }
    words.concat()
}

pub fn taken(book: &cards::Book) -> Vec<Taken> {
    let mut out: Vec<Taken> = book
        .cards
        .iter()
        .filter(|c| c.kind == cards::Kind::Spell)
        .map(|c| Taken { id: c.id.clone(), name: c.name.clone(), mix: effect_mix(c) })
        .collect();
    for recipe in &RECIPES {
        let card = brewed_card(recipe, Tier::Common);
        if !out.iter().any(|t| t.id == card.id) {
            out.push(Taken { id: card.id.clone(), name: recipe.name.into(), mix: effect_mix(&card) });
        }
    }
    for wish in phrasebook::book().iter().filter(|w| w.kind == "spell") {
        let id = wish.card_id(0);
        if out.iter().any(|t| t.id == id) {
            continue;
        }
        if let Ok(card) = wish.card(0) {
            out.push(Taken { id, name: wish.name.clone(), mix: effect_mix(&card) });
        }
    }
    out
}

pub fn power_note() -> String {
    format!(
        "A spell's power is what it does at once over its cooldown in seconds: nova 3 a point (it hits every monster about), cabbages 34 each, cast bolt 32, cast ring 96, cast meteor 96, heal 2 a point, ward 25 a second, bombs 40 each, hedge 90, imposture 90, summon swarm 30 each, guard 25 each, turret 80 each; summed, then divided by the cooldown. A brew may come to at most {POWER_BUDGET} (two Cabbage Storms): nova 40 with bombs 1 on a 16 second cooldown is (120 + 40) / 16 = 10.0."
    )
}

pub fn faults(card: &Card, inputs: [&Card; 2], taken: &[Taken]) -> Vec<String> {
    let mut faults = Vec::new();
    if card.kind != cards::Kind::Spell {
        faults.push(format!("a brew is a spell: `kind spell`, not `kind {}`", card.kind.word()));
    }
    let p = power(card);
    if p > POWER_BUDGET {
        faults.push(format!(
            "it comes to {p:.1} power a second; a brew may come to at most {POWER_BUDGET}: lower its numbers or lengthen its cooldown"
        ));
    }
    let name = bare(&card.name);
    if let Some(same) = inputs
        .iter()
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .chain(taken.iter().map(|t| (t.id.as_str(), t.name.as_str())))
        .find(|(id, n)| *id != card.id && bare(n) == name)
    {
        faults.push(format!("the name {} is taken ({}): give it a name of its own", card.name, same.1));
    }
    let own = effect_mix(card);
    if let Some(same) = inputs
        .iter()
        .map(|c| (c.id.clone(), c.name.clone(), effect_mix(c)))
        .chain(taken.iter().map(|t| (t.id.clone(), t.name.clone(), t.mix.clone())))
        .find(|(id, _, m)| *id != card.id && *m == own)
    {
        faults.push(format!(
            "its effects ({}) are {}'s mix: fuse the two into a combination no spell has",
            own.join(", "),
            same.1
        ));
    }
    let cabbage = |c: &Card| c.effects.iter().any(|e| matches!(e, Effect::Cabbages(_)));
    if cabbage(card) && !inputs.iter().any(|c| cabbage(c)) {
        faults.push("no cabbages: no cabbage went in".into());
    }
    if card.art.is_empty() {
        faults.push("draw its card icon under `art`".into());
    }
    if let Some(pet) = pet_shot(inputs) {
        let hunts = card
            .effects
            .iter()
            .any(|e| matches!(e, Effect::Summon(_, super::imps::Behaviour::Swarm)));
        if !hunts || card.looks.summon.is_none() {
            faults.push(format!(
                "{} and a shot went in, so the brew fires homing {}: `summon N swarm` and a `draw summon` of it",
                pet.name, pet.name
            ));
        }
    }
    faults
}

const CREATURES: [&str; 32] = [
    "cat", "kitten", "kitty", "dog", "puppy", "pup", "hound", "pet", "crab", "rat", "mouse", "bird",
    "owl", "frog", "toad", "snake", "bee", "wasp", "spider", "wolf", "fox", "beast", "critter",
    "monkey", "pig", "goose", "chicken", "duck", "bat", "lizard", "hamster", "ferret",
];

pub fn pet_shot(inputs: [&Card; 2]) -> Option<&Card> {
    let creature = |c: &Card| {
        c.effects.iter().any(|e| matches!(e, Effect::Summon(..)))
            || format!("{} {} {}", c.id, c.name, c.text)
                .to_ascii_lowercase()
                .split(|ch: char| !ch.is_ascii_alphabetic())
                .any(|w| CREATURES.contains(&w) || CREATURES.iter().any(|c| w.strip_suffix('s') == Some(c)))
    };
    let shoots = |c: &Card| c.effects.iter().any(|e| matches!(e, Effect::Cast(_) | Effect::Area(_))) || c.looks.shot.is_some();
    match inputs {
        [a, b] if creature(a) && shoots(b) && !creature(b) => Some(a),
        [a, b] if creature(b) && shoots(a) && !creature(a) => Some(b),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stir {
    pub knight: u32,
    #[serde(default)]
    pub picks: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brew {
    pub knight: u32,
    pub at: u64,
    pub slots: [u8; 2],
    pub cards: [String; 2],
    pub outcome: Outcome,
    #[serde(default)]
    pub done: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cauldron {
    pub raid: u64,
    pub depth: u32,
    pub room: usize,
    pub x: f32,
    pub y: f32,
    pub rose: u64,
    #[serde(default)]
    pub brewed: Vec<u32>,
    #[serde(default)]
    pub stirs: Vec<Stir>,
    #[serde(default)]
    pub brews: Vec<Brew>,
    #[serde(default)]
    pub keys: Vec<(u32, u8, bool)>,
}

impl Cauldron {
    pub fn stir(&self, knight: u32) -> Option<&Stir> {
        self.stirs.iter().find(|s| s.knight == knight)
    }

    pub fn brewed(&self, knight: u32) -> bool {
        self.brewed.contains(&knight)
    }

    pub fn brew(&self, knight: u32) -> Option<&Brew> {
        self.brews.iter().find(|b| b.knight == knight)
    }

    pub fn showing(&self, tick: u64) -> Option<&Brew> {
        self.brews
            .iter()
            .filter(|b| tick.saturating_sub(b.at) <= u64::from(SHOWN))
            .max_by_key(|b| b.at)
    }

    fn last_keys(&self, knight: u32) -> (u8, bool) {
        self.keys
            .iter()
            .find(|k| k.0 == knight)
            .map_or((0, false), |k| (k.1, k.2))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    Brew,
    Pick(usize),
    Brewed,
    Short,
}

impl Run {
    pub fn cauldron_here(&self) -> Option<&Cauldron> {
        self.cauldron.as_ref().filter(|c| {
            c.raid == self.raid_id && c.depth == self.dungeon.depth && c.room == self.at && !self.at_home_now()
        })
    }

    fn cauldron_here_mut(&mut self) -> Option<&mut Cauldron> {
        let (raid, depth, at, home) = (self.raid_id, self.dungeon.depth, self.at, self.at_home_now());
        self.cauldron
            .as_mut()
            .filter(|c| c.raid == raid && c.depth == depth && c.room == at && !home)
    }

    pub fn cauldron_ask(&self, id: u32) -> Option<Ask> {
        let pot = self.cauldron_here()?;
        if self.tick < pot.rose + u64::from(RISE_TICKS) {
            return None;
        }
        let hero = self.players.get(&id).filter(|h| h.hp > 0 && !h.stone)?;
        if (hero.x - pot.x).hypot(hero.y - pot.y) > REACH && pot.stir(id).is_none() {
            return None;
        }
        Some(if let Some(stir) = pot.stir(id) {
            Ask::Pick(stir.picks.len())
        } else if pot.brewed(id) {
            Ask::Brewed
        } else if hero.spells.iter().flatten().filter(|id| super::kingdom_gear::item(id).is_none()).count() < 2 {
            Ask::Short
        } else {
            Ask::Brew
        })
    }

    pub(super) fn raise_cauldron(&mut self) {
        if self.at_home_now() {
            return;
        }
        let Some((x, y)) = self.cauldron_spot() else {
            return;
        };
        self.cauldron = Some(Cauldron {
            raid: self.raid_id,
            depth: self.dungeon.depth,
            room: self.at,
            x,
            y,
            rose: self.tick + u64::from(RISE_DELAY),
            ..Cauldron::default()
        });
        self.cues.push("cauldron".into());
    }

    fn cauldron_spot(&self) -> Option<(f32, f32)> {
        let room = self.room();
        let inside = |c: i32, r: i32| match self.open_here() {
            Some(open) => {
                let (x, y, w, h) = open.chambers[usize::from(open.guardian)].rect;
                let (x, y, w, h) = (i32::from(x), i32::from(y), i32::from(w), i32::from(h));
                c > x && r > y && c < x + w - 1 && r < y + h - 1
            }
            None => true,
        };
        let mut stairs = (0.0, 0.0, 0.0);
        for r in 0..room.rows as i32 {
            for c in 0..room.cols as i32 {
                if room.tile(c, r) == Tile::Stairs && inside(c, r) {
                    stairs.0 += c as f32 + 0.5;
                    stairs.1 += r as f32 + 0.5;
                    stairs.2 += 1.0;
                }
            }
        }
        let (sc, sr) = if stairs.2 > 0.0 {
            (stairs.0 / stairs.2, stairs.1 / stairs.2)
        } else if let Some(open) = self.open_here() {
            let (x, y) = open.chambers[usize::from(open.guardian)].centre();
            (x / TILE_UNITS, y / TILE_UNITS)
        } else {
            (room.cols as f32 / 2.0, room.rows as f32 / 2.0)
        };
        let mut keep_off: Vec<(f32, f32, f32)> = room.items.iter().map(|i| (i.x, i.y, 2.5)).collect();
        if room.kind == RoomKind::Sanctuary {
            let (ax, ay) = room.altar();
            keep_off.push((ax, ay, 6.0));
            keep_off.extend(room.privy().map(|(x, y)| (x, y, 5.0)));
            keep_off.extend(room.field_banner().map(|(x, y)| (x, y, 4.0)));
        }
        keep_off.extend(self.wish_altar().map(|(x, y)| (x, y, 6.0)));
        if stairs.2 > 0.0 {
            keep_off.push((sc * TILE_UNITS, sr * TILE_UNITS, 4.5));
        }
        const AROUND: [(f32, f32); 10] = [
            (-4.5, 0.0),
            (4.5, 0.0),
            (-4.5, -1.5),
            (4.5, -1.5),
            (-4.5, 1.5),
            (4.5, 1.5),
            (-3.0, -3.5),
            (3.0, -3.5),
            (-6.0, 0.0),
            (6.0, 0.0),
        ];
        AROUND.iter().find_map(|&(dc, dr)| {
            let (c, r) = ((sc + dc).floor() as i32, (sr + dr).floor() as i32);
            let clear = (-1..=1).all(|ec| (-1..=1).all(|er| room.tile(c + ec, r + er) == Tile::Floor && inside(c + ec, r + er)));
            let (x, y) = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
            let free = keep_off.iter().all(|&(kx, ky, d)| (kx - x).hypot(ky - y) >= d);
            (clear && free).then_some((x, y))
        })
    }

    pub(super) fn tend_cauldron(&mut self, inputs: &BTreeMap<u32, Input>) -> Option<BTreeMap<u32, Input>> {
        let rose = self.cauldron_here()?.rose;
        let tick = self.tick;
        if tick < rose {
            return None;
        }
        if tick == rose {
            self.sounds.push("hand_rise");
            self.fx.push(Fx::Shake(4));
        }
        let due: Vec<usize> = {
            let pot = self.cauldron_here()?;
            (0..pot.brews.len())
                .filter(|&i| !pot.brews[i].done && tick >= pot.brews[i].at + u64::from(PUFF))
                .collect()
        };
        for i in due {
            self.boil_over(i);
        }
        let mut taken: Option<BTreeMap<u32, Input>> = None;
        let ids: Vec<u32> = self.players.keys().copied().collect();
        let mut keys = Vec::with_capacity(ids.len());
        for id in ids {
            let input = inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
            keys.push((id, input.cast, input.interact));
            if self.brew_keys(id, input) {
                let mut held = input;
                (held.cast, held.interact, held.take) = (0, false, false);
                taken.get_or_insert_with(|| inputs.clone()).insert(id, held);
            }
        }
        let (x, y) = {
            let pot = self.cauldron_here_mut()?;
            pot.keys = keys;
            (pot.x, pot.y)
        };
        let y = y - GIRTH_NORTH;
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let Some(hero) = self.players.get(&id) else { continue };
            if hero.hp == 0 {
                continue;
            }
            let (dx, dy) = (hero.x - x, hero.y - y);
            let d = dx.hypot(dy);
            if d >= GIRTH {
                continue;
            }
            let (ux, uy) = if d < 0.01 { (0.0, 1.0) } else { (dx / d, dy / d) };
            let side = if dx.abs() > 0.05 { dx.signum() } else if dy >= 0.0 { 1.0 } else { -1.0 };
            let prefer = if (-uy) * side >= 0.0 { (-uy, ux) } else { (uy, -ux) };
            let walk = inputs.get(&id).copied().map(|i| (f32::from(i.move_x), f32::from(i.move_y))).unwrap_or((0.0, 0.0));
            let other = (-prefer.0, -prefer.1);
            let (tx, ty) = if walk.0 != 0.0 || walk.1 != 0.0 {
                let with = walk.0 * prefer.0 + walk.1 * prefer.1;
                let against = walk.0 * other.0 + walk.1 * other.1;
                if against > with { other } else { prefer }
            } else {
                prefer
            };
            let a = SLIDE / GIRTH;
            let (rx, ry) = (ux + tx * a, uy + ty * a);
            let n = rx.hypot(ry).max(0.01);
            let hero = self.players.get_mut(&id).unwrap();
            (hero.x, hero.y) = (x + rx / n * GIRTH, y + ry / n * GIRTH);
        }
        taken
    }

    fn brew_keys(&mut self, id: u32, input: Input) -> bool {
        let Some(pot) = self.cauldron_here() else {
            return false;
        };
        let (was_cast, was_use) = pot.last_keys(id);
        let pressed_use = input.interact && !was_use;
        let pressed_cast = (input.cast != was_cast && input.cast != 0).then_some(input.cast);
        let stirring = pot.stir(id).is_some();
        let (px, py) = (pot.x, pot.y);
        let Some(hero) = self.players.get(&id) else {
            return false;
        };
        let able = hero.hp > 0 && !hero.stone && !hero.frog() && hero.tossed == 0 && hero.privy == 0;
        let far = (hero.x - px).hypot(hero.y - py);
        if stirring {
            if !able || far > LET_GO {
                self.end_stir(id);
                return false;
            }
            if let Some(key) = pressed_cast {
                self.pick(id, key - 1);
            }
            if pressed_use {
                let picks = self.cauldron_here().and_then(|p| p.stir(id)).map(|s| s.picks.clone()).unwrap_or_default();
                if picks.len() == 2 {
                    self.throw_in(id, [picks[0], picks[1]]);
                } else {
                    self.end_stir(id);
                    self.sounds.push("door_pass");
                }
            }
            return true;
        }
        if pressed_use && able && far <= REACH && self.cauldron_ask(id) == Some(Ask::Brew) {
            if let Some(pot) = self.cauldron_here_mut() {
                pot.stirs.push(Stir { knight: id, picks: Vec::new() });
            }
            self.sounds.push("hand_grab");
            return true;
        }
        false
    }

    fn end_stir(&mut self, id: u32) {
        if let Some(pot) = self.cauldron_here_mut() {
            pot.stirs.retain(|s| s.knight != id);
        }
    }

    fn pick(&mut self, id: u32, slot: u8) {
        let learned = self
            .players
            .get(&id)
            .is_some_and(|h| h.spells.get(usize::from(slot)).and_then(Option::as_deref)
                .is_some_and(|id| super::kingdom_gear::item(id).is_none()));
        if !learned {
            return;
        }
        let Some(stir) = self.cauldron_here_mut().and_then(|p| p.stirs.iter_mut().find(|s| s.knight == id)) else {
            return;
        };
        if let Some(i) = stir.picks.iter().position(|&s| s == slot) {
            stir.picks.remove(i);
        } else {
            stir.picks.push(slot);
            if stir.picks.len() > 2 {
                stir.picks.remove(0);
            }
        }
        self.sounds.push("card_pickup");
    }

    fn throw_in(&mut self, id: u32, slots: [u8; 2]) {
        let Some(hero) = self.players.get(&id) else {
            return;
        };
        let names = slots.map(|s| hero.spells[usize::from(s)].clone());
        let [Some(a), Some(b)] = names else {
            return;
        };
        if super::kingdom_gear::item(&a).is_some() || super::kingdom_gear::item(&b).is_some() {
            return;
        }
        let (Some(ca), Some(cb)) = (self.book.get(&a), self.book.get(&b)) else {
            return;
        };
        let (outcome, _) = decide(self.seed, self.dungeon.depth, id, ca, cb);
        if let Some(hero) = self.players.get_mut(&id) {
            for s in slots {
                let s = usize::from(s);
                if let Some(gone) = hero.spells[s].take() {
                    for bar in &mut hero.bar {
                        if matches!(bar, Some(cards::BarSlot::Spell(b)) if *b == gone) {
                            *bar = None;
                        }
                    }
                    hero.deck.retain(|c| *c != gone || !c.starts_with("wish-"));
                }
                hero.spell_cooldowns[s] = 0;
            }
            hero.rebonus(&self.book);
        }
        let tick = self.tick;
        if let Some(pot) = self.cauldron_here_mut() {
            pot.stirs.retain(|s| s.knight != id);
            pot.brewed.push(id);
            pot.brews.push(Brew {
                knight: id,
                at: tick,
                slots,
                cards: [a, b],
                outcome,
                done: false,
            });
        }
        self.sounds.push("potion");
        self.cues.push("cauldron_brew".into());
    }

    fn boil_over(&mut self, i: usize) {
        let Some(brew) = self.cauldron_here_mut().map(|p| {
            p.brews[i].done = true;
            p.brews[i].clone()
        }) else {
            return;
        };
        let id = brew.knight;
        self.sounds.push("gas_release");
        self.fx.push(Fx::Shake(6));
        match &brew.outcome {
            Outcome::Spell { name, .. } => {
                let (a, b) = (self.book.get(&brew.cards[0]).cloned(), self.book.get(&brew.cards[1]).cloned());
                let card = match (a, b) {
                    (Some(a), Some(b)) => decide(self.seed, self.dungeon.depth, id, &a, &b).1,
                    _ => None,
                };
                let placed = card.is_some_and(|card| self.book.insert(card));
                if placed {
                    self.give_back(id, brew.slots[0], &outcome_card(&brew.outcome));
                    self.brewing.push(Brewing {
                        knight: id,
                        raid: self.raid_id,
                        depth: self.dungeon.depth,
                        cards: brew.cards.clone(),
                        placeholder: outcome_card(&brew.outcome),
                    });
                    self.sounds.push("wish_grant");
                    self.found = Some((self.tick, id, format!("brewed {name}")));
                } else {
                    self.give_back(id, brew.slots[0], &brew.cards[0]);
                    self.give_back(id, brew.slots[1], &brew.cards[1]);
                }
            }
            Outcome::Misfire(m) => {
                self.give_back(id, brew.slots[0], &brew.cards[0]);
                self.give_back(id, brew.slots[1], &brew.cards[1]);
                if let Some(hero) = self.players.get_mut(&id) {
                    match m {
                        Misfire::Frog => {
                            hero.hexed = hero.hexed.max(FROG_TICKS + hexer::HEX_IMMUNE);
                            (hero.shielding, hero.walling, hero.dash_ticks) = (false, false, 0);
                        }
                        Misfire::Confetti | Misfire::Hiccups => {}
                    }
                }
                self.sounds.push("helmet_pop");
                self.found = Some((self.tick, id, m.says().into()));
            }
        }
    }

    pub fn brewed(&mut self, ask: &Brewing, mut card: Card) -> Result<String, Vec<String>> {
        card.id = ask.card_id();
        let (Some(a), Some(b)) = (self.book.get(&ask.cards[0]), self.book.get(&ask.cards[1])) else {
            return Err(vec!["the cards that went in are gone from the book".into()]);
        };
        let faults = faults(&card, [a, b], &taken(&self.book));
        if !faults.is_empty() {
            return Err(faults);
        }
        super::brew_jobs::install(self, ask, card).map_err(|error| vec![error])
    }

    fn give_back(&mut self, id: u32, slot: u8, card: &str) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        let slot = usize::from(slot);
        let free = if hero.spells[slot].is_none() {
            Some(slot)
        } else {
            hero.spells.iter().position(Option::is_none)
        };
        match free {
            Some(s) => {
                hero.spells[s] = Some(card.to_string());
                hero.spell_cooldowns[s] = 0;
                if card.starts_with("wish-") && !hero.deck.iter().any(|c| c == card) {
                    hero.deck.push(card.to_string());
                    hero.rebonus(&self.book);
                }
                if let Some(bar) = hero.bar.iter_mut().find(|b| b.is_none()) {
                    *bar = Some(cards::BarSlot::Spell(card.to_string()));
                }
            }
            None => {
                let (x, y) = (hero.x, hero.y + 2.0);
                self.drop_item(card.to_string(), x, y, Some(id));
            }
        }
    }
}

fn outcome_card(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Spell { card, .. } => card.clone(),
        Outcome::Misfire(_) => String::new(),
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_cauldron__tests.rs"]
mod tests;
