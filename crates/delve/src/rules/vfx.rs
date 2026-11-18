//! A card's effect recipe: how its magic moves on the field, as data.
//!
//! A card may say how its shots trail, what they throw when loosed, how
//! they break on a hit and on a kill, what a spell draws as it is cast,
//! and what a wish worn at its highest tiers gives off about its knight.
//! Each is a `vfx` line, a layer of the recipe:
//!
//! ```text
//! vfx <when> <form> [palette] [motion] [ease] [count=N] [speed=N] [spread=N] [life=N] [size=N] [wobble=N] [spin=N]
//! vfx hit shards frost back count=12 speed=5 spread=360 life=16
//! vfx trail worms poop wobble count=3 wobble=2
//! ```
//!
//! - **when**: `cast` `fire` `trail` `hit` `kill` `aura`
//! - **form**: `sparks` `embers` `motes` `droplets` `shards` `flakes`
//!   `bubbles` `notes` `stars` `petals` `flies` `coins` `hearts` `worms`
//!   `smoke`, and the shapes `ring` `glyph` `arc` `rays`
//! - **palette**: `fire` `ember` `frost` `holy` `gold` `arcane` `genie`
//!   `storm` `poison` `nature` `blood` `rose` `poop` `brew` `void` `water`
//!   `bone` `smoke` `earth`, each a ramp of the realm's own inks (the
//!   arcane, a brew and the void in its violet dye, a rose in its pink)
//! - **motion**: `straight` `wobble` `spiral` `zigzag` `curl` `bounce`
//!   `rise` `fall`; **ease**: `out` `in` `both` `back` `spring`
//!
//! Every number is held to its bounds ([`Layer::check`]): a recipe can ask
//! for a great deal, and never for more than a frame will draw. A card has
//! at most [`MOST_LAYERS`]. A line that does not read is set aside with a
//! note, never the card: a wish is never refused over how it looks, and a
//! card with no recipe is given one from its own words ([`generated`]).
//!
//! Recipes are data. They change what is drawn and nothing else; a card
//! without one serializes exactly as it did.

use super::cards::{Card, Effect, Kind};
use serde::{Deserialize, Serialize};

pub const MOST_LAYERS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    Cast,
    Fire,
    Trail,
    Hit,
    Kill,
    Aura,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    Sparks,
    Embers,
    Motes,
    Droplets,
    Shards,
    Flakes,
    Bubbles,
    Notes,
    Stars,
    Petals,
    Flies,
    Coins,
    Hearts,
    Worms,
    Smoke,
    Ring,
    Glyph,
    Arc,
    Rays,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Palette {
    Fire,
    Ember,
    Frost,
    Holy,
    Gold,
    Arcane,
    Genie,
    Storm,
    Poison,
    Nature,
    Blood,
    Rose,
    Poop,
    Brew,
    Void,
    Water,
    Bone,
    Smoke,
    Earth,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    #[default]
    Straight,
    Wobble,
    Spiral,
    Zigzag,
    Curl,
    Bounce,
    Rise,
    Fall,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    #[default]
    Out,
    In,
    Both,
    Back,
    Spring,
}

macro_rules! words {
    ($ty:ty { $($v:ident => $w:literal),* $(,)? }) => {
        impl $ty {
            pub const ALL: &'static [$ty] = &[$(<$ty>::$v),*];
            pub fn word(self) -> &'static str {
                match self { $(<$ty>::$v => $w),* }
            }
            pub fn parse(word: &str) -> Option<$ty> {
                match word { $($w => Some(<$ty>::$v),)* _ => None }
            }
            pub fn words() -> String {
                Self::ALL.iter().map(|v| v.word()).collect::<Vec<_>>().join(" ")
            }
        }
    };
}

words!(When { Cast => "cast", Fire => "fire", Trail => "trail", Hit => "hit", Kill => "kill", Aura => "aura" });
words!(Form {
    Sparks => "sparks", Embers => "embers", Motes => "motes", Droplets => "droplets", Shards => "shards",
    Flakes => "flakes", Bubbles => "bubbles", Notes => "notes", Stars => "stars", Petals => "petals",
    Flies => "flies", Coins => "coins", Hearts => "hearts", Worms => "worms", Smoke => "smoke",
    Ring => "ring", Glyph => "glyph", Arc => "arc", Rays => "rays",
});
words!(Palette {
    Fire => "fire", Ember => "ember", Frost => "frost", Holy => "holy", Gold => "gold", Arcane => "arcane",
    Genie => "genie", Storm => "storm", Poison => "poison", Nature => "nature", Blood => "blood",
    Rose => "rose", Poop => "poop", Brew => "brew", Void => "void", Water => "water", Bone => "bone",
    Smoke => "smoke", Earth => "earth",
});
words!(Motion {
    Straight => "straight", Wobble => "wobble", Spiral => "spiral", Zigzag => "zigzag", Curl => "curl",
    Bounce => "bounce", Rise => "rise", Fall => "fall",
});
words!(Ease { Out => "out", In => "in", Both => "both", Back => "back", Spring => "spring" });

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Layer {
    pub on: When,
    pub form: Form,
    pub palette: Palette,
    #[serde(default, skip_serializing_if = "is_default")]
    pub motion: Motion,
    #[serde(default, skip_serializing_if = "is_default")]
    pub ease: Ease,
    pub count: u8,
    pub speed: u8,
    pub spread: u16,
    pub life: u8,
    pub size: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub wobble: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub spin: i8,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

pub fn most(on: When) -> u8 {
    match on {
        When::Trail => 6,
        When::Fire => 12,
        When::Hit => 16,
        When::Kill | When::Cast => 24,
        When::Aura => 8,
    }
}

pub fn kept(on: When) -> Option<(u8, f32)> {
    match on {
        When::Fire => Some((super::ranged::LIFE, FIRE_SCALE)),
        When::Hit => Some((super::ranged::LIFE, HIT_SCALE)),
        When::Kill => Some((super::feel::DEATH_TICKS, KILL_SCALE)),
        When::Cast | When::Trail | When::Aura => None,
    }
}

pub const FIRE_SCALE: f32 = 1.06;
pub const HIT_SCALE: f32 = 1.2;
pub const KILL_SCALE: f32 = 1.45;

pub fn fits(on: When) -> Option<u8> {
    let (kept, scale) = kept(on)?;
    Some(((f32::from(kept) / scale).floor() as u8).max(LIFE.0))
}

pub const SPEED: (u8, u8) = (0, 8);
pub const SPREAD: (u16, u16) = (0, 360);
pub const LIFE: (u8, u8) = (4, 40);
pub const SIZE: (u8, u8) = (1, 3);
pub const WOBBLE: (u8, u8) = (0, 8);
pub const SPIN: (i8, i8) = (-8, 8);

impl Layer {
    pub fn new(on: When, form: Form, palette: Palette) -> Layer {
        let (count, speed, spread, life) = match on {
            When::Cast => (12, 4, 360, 24),
            When::Fire => (5, 4, 70, 8),
            When::Trail => (3, 1, 50, 10),
            When::Hit => (10, 4, 360, 14),
            When::Kill => (16, 5, 360, 22),
            When::Aura => (4, 1, 360, 36),
        };
        Layer { on, form, palette, motion: Motion::Straight, ease: Ease::Out, count, speed, spread, life, size: 1, wobble: 0, spin: 0 }
    }

    pub fn check(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        let mut clamp = |name: &str, v: i32, (lo, hi): (i32, i32)| -> i32 {
            let c = v.clamp(lo, hi);
            if c != v {
                notes.push(format!("vfx {name} {v} clamped to {c}"));
            }
            c
        };
        self.count = clamp("count", self.count.into(), (1, most(self.on).into())) as u8;
        self.speed = clamp("speed", self.speed.into(), (SPEED.0.into(), SPEED.1.into())) as u8;
        self.spread = clamp("spread", self.spread.into(), (SPREAD.0.into(), SPREAD.1.into())) as u16;
        self.life = clamp("life", self.life.into(), (LIFE.0.into(), LIFE.1.into())) as u8;
        self.size = clamp("size", self.size.into(), (SIZE.0.into(), SIZE.1.into())) as u8;
        self.wobble = clamp("wobble", self.wobble.into(), (WOBBLE.0.into(), WOBBLE.1.into())) as u8;
        self.spin = clamp("spin", self.spin.into(), (SPIN.0.into(), SPIN.1.into())) as i8;
        notes
    }

    pub fn read(rest: &str) -> Result<(Layer, Vec<String>), String> {
        let lower = rest.to_ascii_lowercase();
        let mut words = lower.split_whitespace();
        let on = words.next().and_then(When::parse).ok_or_else(|| format!("vfx starts with when it plays: {}", When::words()))?;
        let form = words.next().and_then(Form::parse).ok_or_else(|| format!("vfx {} needs a form: {}", on.word(), Form::words()))?;
        let mut layer = Layer::new(on, form, Palette::Arcane);
        for word in words {
            if let Some((key, value)) = word.split_once('=') {
                let number = || value.parse::<i32>().map_err(|_| format!("vfx {key}={value}: a whole number"));
                let byte = |v: i32| v.clamp(0, 255) as u8;
                match key {
                    "count" => layer.count = byte(number()?),
                    "speed" => layer.speed = byte(number()?),
                    "spread" => layer.spread = number()?.clamp(0, 9999) as u16,
                    "life" => layer.life = byte(number()?),
                    "size" => layer.size = byte(number()?),
                    "wobble" => layer.wobble = byte(number()?),
                    "spin" => layer.spin = number()?.clamp(-128, 127) as i8,
                    "palette" => layer.palette = Palette::parse(value).ok_or_else(|| format!("vfx palette={value}: one of {}", Palette::words()))?,
                    "motion" => layer.motion = Motion::parse(value).ok_or_else(|| format!("vfx motion={value}: one of {}", Motion::words()))?,
                    "ease" => layer.ease = Ease::parse(value).ok_or_else(|| format!("vfx ease={value}: one of {}", Ease::words()))?,
                    _ => return Err(format!("vfx: unknown setting `{key}` (count speed spread life size wobble spin palette motion ease)")),
                }
            } else if let Some(p) = Palette::parse(word) {
                layer.palette = p;
            } else if let Some(m) = Motion::parse(word) {
                layer.motion = m;
            } else if let Some(e) = Ease::parse(word) {
                layer.ease = e;
            } else {
                return Err(format!("vfx: `{word}` is no palette, motion or ease"));
            }
        }
        let notes = layer.check();
        Ok((layer, notes))
    }

    pub fn line(&self) -> String {
        let mut out = format!("vfx {} {} {}", self.on.word(), self.form.word(), self.palette.word());
        if self.motion != Motion::Straight {
            out.push_str(&format!(" {}", self.motion.word()));
        }
        if self.ease != Ease::Out {
            out.push_str(&format!(" {}", self.ease.word()));
        }
        out.push_str(&format!(" count={} speed={} spread={} life={} size={}", self.count, self.speed, self.spread, self.life, self.size));
        if self.wobble != 0 {
            out.push_str(&format!(" wobble={}", self.wobble));
        }
        if self.spin != 0 {
            out.push_str(&format!(" spin={}", self.spin));
        }
        out
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Recipe {
    pub layers: Vec<Layer>,
}

impl Recipe {
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn valid(&self) -> bool {
        self.layers.len() <= MOST_LAYERS && self.layers.iter().all(|l| {
            let mut c = *l;
            c.check().is_empty()
        })
    }

    pub fn on(&self, on: When) -> impl Iterator<Item = &Layer> {
        self.layers.iter().filter(move |l| l.on == on)
    }

    pub fn first(&self, on: When) -> Option<&Layer> {
        self.on(on).next()
    }
}

pub fn of(card: &Card) -> Recipe {
    if card.looks.vfx.is_empty() { generated(card) } else { card.looks.vfx.clone() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Fire,
    Frost,
    Holy,
    Storm,
    Poison,
    Arcane,
    Earth,
}

pub const CAST_SOUNDS: [&str; 7] = ["cast_fire", "cast_frost", "cast_holy", "cast_storm", "cast_poison", "cast_arcane", "cast_earth"];
pub const IMPACT_SOUNDS: [&str; 7] = ["impact_fire", "impact_frost", "impact_holy", "impact_storm", "impact_poison", "impact_arcane", "impact_earth"];

impl Voice {
    pub fn cast(self) -> &'static str {
        CAST_SOUNDS[self as usize]
    }

    pub fn impact(self) -> &'static str {
        IMPACT_SOUNDS[self as usize]
    }
}

impl Palette {
    pub fn voice(self) -> Voice {
        match self {
            Palette::Fire | Palette::Ember => Voice::Fire,
            Palette::Frost | Palette::Water => Voice::Frost,
            Palette::Holy | Palette::Gold | Palette::Rose => Voice::Holy,
            Palette::Storm => Voice::Storm,
            Palette::Poison | Palette::Poop | Palette::Brew => Voice::Poison,
            Palette::Earth | Palette::Bone | Palette::Nature => Voice::Earth,
            Palette::Arcane | Palette::Genie | Palette::Void | Palette::Blood | Palette::Smoke => Voice::Arcane,
        }
    }
}

pub fn palette(card: &Card) -> Palette {
    let recipe = of(card);
    recipe.first(When::Cast).or_else(|| recipe.layers.first()).map_or(Palette::Arcane, |l| l.palette)
}

pub fn cast_sound(card: &Card) -> &'static str {
    if let Some(area) = super::zones::area_of(card) {
        return super::zones::voice_of(area.element).cast();
    }
    palette(card).voice().cast()
}

pub fn impact_sound(card: &Card) -> &'static str {
    if let Some(area) = super::zones::area_of(card) {
        return super::zones::voice_of(area.element).impact();
    }
    palette(card).voice().impact()
}

pub fn tier(card: &str) -> Option<u32> {
    super::dead_wishes::wish_of(card)?;
    card.rsplit_once('-')?.1.parse().ok()
}

fn words(card: &Card) -> Vec<String> {
    words_of(card, true)
}

fn words_of(card: &Card, text: bool) -> Vec<String> {
    let id = super::dead_wishes::wish_of(&card.id).unwrap_or(&card.id);
    let mut out = Vec::new();
    let sources = [card.name.as_str(), id, if text { card.text.as_str() } else { "" }];
    for source in sources {
        for w in source.split(|c: char| !c.is_ascii_alphanumeric()) {
            let w = w.to_ascii_lowercase();
            if w.is_empty() || w.chars().all(|c| c.is_ascii_digit()) || ["i", "ii", "iii", "iv", "v", "vi"].contains(&w.as_str()) {
                continue;
            }
            let w = if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") { w[..w.len() - 1].to_string() } else { w };
            if !out.contains(&w) {
                out.push(w);
            }
        }
    }
    out
}

const PALETTES: &[(Palette, &[&str])] = &[
    (Palette::Poop, &["poop", "turd", "poo", "dung", "manure", "sewage", "diarrhea", "diarrhoea", "fart", "toilet", "crap", "stool"]),
    (Palette::Brew, &["purple", "drank", "lean", "grape", "syrup", "potion", "brew", "wine", "violet", "lavender", "plum", "elixir", "lilac", "mauve", "amethyst", "aubergine", "eggplant", "orchid", "indigo"]),
    (Palette::Blood, &["blood", "vamp", "vampire", "vampiric", "steal", "leech", "crimson", "life", "drain", "red", "hemo", "gore"]),
    (Palette::Fire, &["fire", "flame", "flaming", "dragon", "burn", "burning", "lava", "magma", "inferno", "blaze", "hot", "phoenix", "pyro", "furnace", "sun", "solar", "kiln", "torch"]),
    (Palette::Ember, &["ember", "ash", "coal", "cinder", "smoulder", "smolder"]),
    (Palette::Frost, &["ice", "icy", "frost", "frozen", "freeze", "freezing", "snow", "cold", "glacier", "winter", "chill", "rime", "blizzard", "hail", "crystal"]),
    (Palette::Storm, &["spark", "lightning", "thunder", "storm", "electric", "shock", "volt", "zap", "tesla", "static", "plasma", "laser"]),
    (Palette::Poison, &["poison", "venom", "toxic", "acid", "slime", "plague", "rot", "stink", "gas", "sludge", "ooze", "goo", "snot", "booger", "radioactive"]),
    (Palette::Holy, &["holy", "angel", "angelic", "divine", "blessed", "sacred", "radiant", "heaven", "heavenly", "light", "saint", "pilgrim", "halo", "mending", "heal", "healing", "prayer"]),
    (Palette::Gold, &["gold", "golden", "coin", "money", "lucky", "luck", "treasure", "rich", "midas", "cash", "dollar", "jackpot", "bling"]),
    (Palette::Void, &["shadow", "dark", "darkness", "void", "night", "ghost", "phantom", "curse", "cursed", "death", "doom", "abyss", "spooky", "black"]),
    (Palette::Water, &["water", "bubble", "sea", "ocean", "wave", "rain", "tide", "fish", "aqua", "splash", "soap", "foam", "wet"]),
    (Palette::Rose, &["butt", "buttcheek", "cheek", "booty", "bum", "rose", "love", "heart", "kiss", "cherry", "pink", "valentine", "cupid", "romance", "candy", "bubblegum", "magenta", "fuchsia", "blush", "flamingo", "raspberry"]),
    (Palette::Nature, &["nature", "leaf", "vine", "thorn", "tree", "flower", "root", "moss", "petal", "bloom", "garden", "forest", "bee", "honey", "seed", "cabbage", "grass", "frog"]),
    (Palette::Bone, &["bone", "skull", "skeleton", "grave", "undead", "tomb", "crypt", "zombie", "lich"]),
    (Palette::Arcane, &["arcane", "magic", "magical", "wizard", "mystic", "rune", "cosmic", "moon", "astral", "spell", "fairy", "fae", "dream", "wish"]),
    (Palette::Genie, &["genie", "lamp", "djinn", "teal", "spirit", "soul"]),
    (Palette::Smoke, &["smoke", "dust", "wind", "air", "feather", "cloud", "fog", "mist", "steam"]),
    (Palette::Earth, &["earth", "rock", "stone", "mud", "dirt", "sand", "clay", "boulder", "hammer"]),
];

const FORMS: &[(Form, &[&str])] = &[
    (Form::Worms, &["worm", "snake", "noodle", "spaghetti", "serpent", "eel", "tentacle", "wiggle", "squiggle"]),
    (Form::Flies, &["fly", "flie", "gnat", "bug", "insect", "mosquito", "poop", "turd", "dung", "stink", "fart"]),
    (Form::Bubbles, &["bubble", "soap", "foam", "fizz", "soda", "gum", "bubblegum", "slime"]),
    (Form::Notes, &["note", "music", "musical", "song", "melody", "sax", "saxophone", "tune", "bard", "sing", "choir", "jazz", "rap"]),
    (Form::Coins, &["coin", "money", "cash", "gold", "golden", "treasure", "dollar", "lucky", "jackpot"]),
    (Form::Hearts, &["heart", "love", "kiss", "valentine", "cupid", "romance"]),
    (Form::Petals, &["petal", "rose", "flower", "blossom", "bloom", "cherry", "leaf", "garden"]),
    (Form::Flakes, &["snow", "snowflake", "flake", "blizzard", "winter", "frost"]),
    (Form::Shards, &["shatter", "shard", "glass", "crystal", "diamond", "ice", "bone", "splinter", "spike", "thorn"]),
    (Form::Droplets, &["drop", "drip", "drank", "juice", "blood", "water", "rain", "goo", "ooze", "sauce", "syrup", "milk", "potion", "acid", "venom", "lean"]),
    (Form::Smoke, &["smoke", "gas", "steam", "fog", "mist", "cloud", "ash", "dust"]),
    (Form::Stars, &["star", "sparkle", "glitter", "twinkle", "fairy", "holy", "angel", "magic", "wish", "cosmic", "divine"]),
    (Form::Embers, &["ember", "cinder", "coal", "kiln"]),
    (Form::Sparks, &["spark", "fire", "flame", "lightning", "electric", "thunder", "shock", "zap", "laser"]),
];

const MOTIONS: &[(Motion, &[&str])] = &[
    (Motion::Wobble, &["worm", "snake", "wiggle", "wobble", "drunk", "drank", "lean", "jelly", "noodle", "jiggle", "buttcheek", "cheek", "butt", "squiggle"]),
    (Motion::Spiral, &["spiral", "twist", "swirl", "vortex", "tornado", "drill", "whirl", "spin", "galaxy"]),
    (Motion::Zigzag, &["zigzag", "lightning", "zap", "thunder", "storm", "electric", "shock", "jagged"]),
    (Motion::Curl, &["seek", "seeker", "seeking", "homing", "hunt", "hunting", "bee", "heat", "guided", "tracking"]),
    (Motion::Bounce, &["bounce", "bouncy", "rubber", "ricochet", "ball", "bubble", "boing", "jump"]),
    (Motion::Rise, &["ghost", "spirit", "soul", "smoke", "steam", "rising", "holy", "angel", "fairy", "float"]),
    (Motion::Fall, &["rain", "drip", "blood", "goo", "heavy", "anvil", "meteor"]),
];

fn first_of<T: Copy>(table: &[(T, &[&str])], words: &[String]) -> Option<T> {
    words.iter().find_map(|w| table.iter().find(|(_, list)| list.contains(&w.as_str())).map(|(v, _)| *v)).or_else(|| {
        words.iter().find_map(|w| table.iter().find(|(_, list)| list.iter().any(|k| k.len() >= 4 && w.len() > k.len() && w.contains(k))).map(|(v, _)| *v))
    })
}

fn last_of<T: Copy>(table: &[(T, &[&str])], words: &[String]) -> Option<T> {
    let name: Vec<String> = words.iter().rev().cloned().collect();
    first_of(table, &name)
}

fn palette_of_effects(card: &Card) -> Option<Palette> {
    card.effects.iter().find_map(|e| {
        Some(match e {
            Effect::Burn(_) | Effect::Burst(_) => Palette::Fire,
            Effect::Cast(super::cards::Shape::Meteor) => Palette::Fire,
            Effect::Area(area) => {
                use super::cards::Element;
                match area.element {
                    Element::Fire => Palette::Fire,
                    Element::Frost => Palette::Frost,
                    Element::Storm => Palette::Storm,
                    Element::Poison => Palette::Poison,
                    Element::Holy => Palette::Holy,
                    Element::Arcane => Palette::Arcane,
                    Element::Earth => Palette::Earth,
                }
            }
            Effect::Chill(_) | Effect::Moth(_) => Palette::Frost,
            Effect::Poison(_) | Effect::Fart(_) => Palette::Poison,
            Effect::Vamp(_) | Effect::Mend(_) | Effect::Leech(_) | Effect::Rip(_) => Palette::Blood,
            Effect::Chain(_) | Effect::Cast(super::cards::Shape::Ring) => Palette::Storm,
            Effect::Gold(_) => Palette::Gold,
            Effect::Heal(_) | Effect::Ward(_) | Effect::Censer(_) | Effect::Linnet(_) => Palette::Holy,
            Effect::Nova(_) | Effect::Cast(_) | Effect::Blink(_) => Palette::Arcane,
            Effect::Pierce(_) => Palette::Void,
            Effect::Ghoul(_) | Effect::Warden(_) | Effect::Totem(_) => Palette::Bone,
            Effect::Cabbages(_) | Effect::Hedge(_) | Effect::Fork(_) => Palette::Nature,
            Effect::Knock(_) => Palette::Smoke,
            Effect::Armor(_) | Effect::MaxHp(_) => Palette::Earth,
            _ => return None,
        })
    })
}

fn form_of_palette(p: Palette) -> Form {
    match p {
        Palette::Fire => Form::Sparks,
        Palette::Ember => Form::Embers,
        Palette::Frost => Form::Shards,
        Palette::Holy | Palette::Arcane | Palette::Genie => Form::Stars,
        Palette::Gold => Form::Coins,
        Palette::Storm => Form::Sparks,
        Palette::Poison => Form::Bubbles,
        Palette::Nature => Form::Petals,
        Palette::Blood | Palette::Brew => Form::Droplets,
        Palette::Rose => Form::Hearts,
        Palette::Poop => Form::Flies,
        Palette::Void => Form::Motes,
        Palette::Water => Form::Bubbles,
        Palette::Bone => Form::Shards,
        Palette::Smoke => Form::Smoke,
        Palette::Earth => Form::Shards,
    }
}

fn impact_of(p: Palette) -> Layer {
    let (form, ease) = match p {
        Palette::Fire | Palette::Ember | Palette::Water | Palette::Earth => (Form::Ring, Ease::Out),
        Palette::Frost | Palette::Bone => (Form::Ring, Ease::Back),
        Palette::Storm => (Form::Arc, Ease::Out),
        Palette::Poison | Palette::Poop | Palette::Smoke | Palette::Brew => (Form::Smoke, Ease::Out),
        Palette::Holy | Palette::Gold | Palette::Rose => (Form::Rays, Ease::Back),
        Palette::Void => (Form::Ring, Ease::In),
        Palette::Arcane | Palette::Genie => (Form::Glyph, Ease::Back),
        Palette::Blood | Palette::Nature => (Form::Rays, Ease::Out),
    };
    let mut l = Layer::new(When::Hit, form, p);
    l.ease = ease;
    l.count = if form == Form::Smoke { 3 } else { 6 };
    l.life = if form == Form::Smoke { fits(When::Hit).unwrap_or(LIFE.1) } else { 10 };
    l
}

fn seed(words: &[String]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for w in words {
        for b in w.bytes().chain([b' ']) {
            h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
        }
    }
    h
}

pub fn generated(card: &Card) -> Recipe {
    let words = words(card);
    let s = seed(&words);
    let pick = |shift: u32, n: u32| (s >> shift) % n.max(1);
    let palette = first_of(PALETTES, &words).or_else(|| palette_of_effects(card)).unwrap_or_else(|| {
        let all = Palette::ALL;
        all[pick(3, all.len() as u32) as usize]
    });
    let form = last_of(FORMS, &words_of(card, false)).or_else(|| first_of(FORMS, &words)).unwrap_or_else(|| form_of_palette(palette));
    let motion = first_of(MOTIONS, &words).or_else(|| {
        card.effects.iter().find_map(|e| match e {
            Effect::Homing(_) => Some(Motion::Curl),
            Effect::Bounce(_) => Some(Motion::Bounce),
            Effect::Fork(_) | Effect::Chain(_) => Some(Motion::Zigzag),
            Effect::Boomerang(_) => Some(Motion::Spiral),
            _ => None,
        })
    });
    let motion = motion.unwrap_or(match palette {
        Palette::Smoke | Palette::Genie | Palette::Holy => Motion::Rise,
        Palette::Blood | Palette::Water => Motion::Fall,
        _ => Motion::Straight,
    });
    let spin = [-3, -2, 2, 3][pick(11, 4) as usize];
    let mut layers = Vec::new();
    let mut trail = Layer::new(When::Trail, trail_form(form), palette);
    trail.motion = motion;
    trail.count = 3 + pick(5, 3) as u8;
    trail.life = 8 + pick(7, 7) as u8;
    trail.spread = 30 + 10 * pick(9, 6) as u16;
    trail.wobble = match motion {
        Motion::Wobble | Motion::Zigzag => 2 + pick(13, 2) as u8,
        Motion::Spiral => 3,
        _ => 0,
    };
    trail.spin = spin;
    layers.push(trail);
    let mut fire = Layer::new(When::Fire, burst_form(form), palette);
    fire.count = 4 + pick(15, 3) as u8;
    fire.spread = 50 + 10 * pick(17, 4) as u16;
    layers.push(fire);
    let mut hit = Layer::new(When::Hit, burst_form(form), palette);
    hit.count = 8 + pick(19, 5) as u8;
    hit.speed = 3 + pick(21, 3) as u8;
    hit.life = (12 + pick(23, 6) as u8).min(fits(When::Hit).unwrap_or(LIFE.1));
    hit.ease = if pick(25, 2) == 0 { Ease::Back } else { Ease::Out };
    hit.motion = if matches!(motion, Motion::Rise | Motion::Fall) { motion } else { Motion::Straight };
    hit.size = 1 + u8::from(matches!(form, Form::Bubbles | Form::Hearts | Form::Coins) && pick(27, 2) == 0);
    layers.push(hit);
    layers.push(impact_of(palette));
    let mut kill = hit;
    kill.on = When::Kill;
    kill.count = 18 + pick(29, 6) as u8;
    kill.speed = (hit.speed + 3).min(SPEED.1);
    kill.life = fits(When::Kill).unwrap_or(LIFE.1);
    layers.push(kill);
    let mut aura = Layer::new(if card.kind == Kind::Spell { When::Cast } else { When::Aura }, ambient_form(form, palette), palette);
    aura.motion = match form {
        Form::Flies | Form::Notes | Form::Hearts => Motion::Curl,
        _ if matches!(palette, Palette::Fire | Palette::Ember | Palette::Smoke | Palette::Genie | Palette::Holy) => Motion::Rise,
        _ if matches!(palette, Palette::Blood | Palette::Water | Palette::Frost) => Motion::Fall,
        _ => Motion::Spiral,
    };
    aura.spin = spin;
    if card.kind == Kind::Spell {
        aura.form = Form::Glyph;
        aura.count = 5 + pick(31, 2) as u8;
    }
    layers.push(aura);
    layers.truncate(MOST_LAYERS);
    Recipe { layers }
}

fn trail_form(form: Form) -> Form {
    match form {
        Form::Ring | Form::Glyph | Form::Rays => Form::Motes,
        Form::Arc => Form::Sparks,
        Form::Coins | Form::Hearts => Form::Stars,
        other => other,
    }
}

fn burst_form(form: Form) -> Form {
    match form {
        Form::Ring | Form::Glyph | Form::Rays | Form::Arc => Form::Sparks,
        Form::Smoke => Form::Motes,
        other => other,
    }
}

fn ambient_form(form: Form, palette: Palette) -> Form {
    match (form, palette) {
        (Form::Flies | Form::Notes | Form::Hearts | Form::Coins | Form::Petals | Form::Bubbles | Form::Flakes, _) => form,
        (_, Palette::Fire) => Form::Embers,
        (_, Palette::Frost) => Form::Flakes,
        (_, Palette::Poop) => Form::Flies,
        (_, Palette::Smoke | Palette::Poison) => Form::Smoke,
        (_, Palette::Blood | Palette::Brew | Palette::Water) => Form::Droplets,
        (_, Palette::Storm) => Form::Sparks,
        _ => Form::Motes,
    }
}

pub fn brief() -> String {
    format!(
        "VFX (optional, how the magic moves; never needed): up to {MOST_LAYERS} lines `vfx <when> <form> [palette] [motion] [ease] [count=N speed=N spread=N life=N size=N wobble=N spin=N]`. \
when: {}. form: {}. palette: {}. motion: {}. ease: {}. \
Bounds: count 1-6 trail, 1-12 fire, 1-16 hit, 1-24 kill/cast, 1-8 aura; speed 0-8 (half pixels a tick); spread 0-360 degrees; life 4-40 ticks; size 1-3; wobble 0-8; spin -8-8. \
Numbers past a bound are held to it. A line that does not read is set aside and the card keeps the look its words give it. \
Example: `vfx trail worms poop wobble count=3 wobble=2` / `vfx hit shards frost back count=12 speed=5` / `vfx aura flies poop count=4`.",
        When::words(),
        Form::words(),
        Palette::words(),
        Motion::words(),
        Ease::words(),
    )
}

#[cfg(test)]
#[path = "../../tests/rules/vfx__tests.rs"]
mod tests;
