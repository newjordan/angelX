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

use super::hireling::AllyKind;
use super::loot::Rng;
use super::{Pack, Weapon};
use crate::realm::forge;
use crate::realm::Spoil;
use serde::{Deserialize, Serialize};

pub const MAX_BYTES: usize = 4096;
/// A card file with its sprites (`looks`): the card's own lines still keep
/// to `MAX_BYTES`; the sprite rows may take the rest.
pub const MAX_FILE_BYTES: usize = 24 * 1024;
/// Play cards a knight can carry; quaff plays the first.
pub const HAND: usize = 4;
/// Spells a knight can learn; keys 1–4 cast them.
pub const SPELL_SLOTS: usize = 4;
/// Seconds, bounded to prevent overflow and zero-recharge spells.
pub const SPELL_COOLDOWN: (u32, u32) = (1, 120);
pub const MAX_EFFECTS: usize = 3;
/// Card art is at most this many ink pixels on a side.
pub const ART: usize = 16;
/// The book's room: every built-in card, and room beside them for the
/// wishes, brews and forgings a run learns. Each tier of a wish is a card of
/// its own, so a party wishing all evening filled 127 and was told "the book
/// is full" (2026-10-23); a thousand keeps a run's save well under the
/// restore's limit.
pub const MAX_BOOK: usize = 1024;
pub const MAX_NAME: usize = 22;
/// The slowest a forged arm's bolt may fly, in arena units a second.
pub const FORGED_SPEED: f32 = 36.0;
pub const MAX_TEXT: usize = 64;

/// How a card is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
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
pub enum Guard {
    /// A quick tumble nothing can touch.
    #[default]
    Roll,
    /// Held up: shots from the front glance off; slow, and no shooting.
    Shield,
    /// Held up: a barrier across the aim that stops monsters' shots and lets
    /// friends' shots through, stronger; it drains mana while held.
    Wall(Wall),
    /// Pressed: a spin on the spot that cuts everything round the knight
    /// and every shot in reach; a breath's wait before the next.
    Spin,
}

/// A held wall's measure: how wide (arena units), how much mana a second it
/// drains, and how much harder friendly shots hit after passing through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wall {
    pub width: u32,
    pub drain: u32,
    pub empower: u32,
}

impl Wall {
    pub const WIDTH: (u32, u32) = (2, 8);
    pub const DRAIN: (u32, u32) = (10, 50);
    pub const EMPOWER: (u32, u32) = (0, 40);
}

impl Kind {
    pub fn word(self) -> &'static str {
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
pub enum Rarity {
    Common,
    Rare,
    Relic,
}

impl Rarity {
    pub fn word(self) -> &'static str {
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

/// Older saves and hosts kept three spell slots; pad their fourth slot.
pub fn slots<'de, D, T>(d: D) -> Result<[T; SPELL_SLOTS], D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    let mut list: Vec<T> = Vec::deserialize(d)?;
    if !(3..=SPELL_SLOTS).contains(&list.len()) {
        return Err(serde::de::Error::custom(
            "expected three or four spell slots",
        ));
    }
    list.resize_with(SPELL_SLOTS, T::default);
    list.try_into()
        .map_err(|_| serde::de::Error::custom("spell slots"))
}

/// One slot of the four-slot spell bar: a spell that recharges, or a
/// draught that is spent on use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarSlot {
    Spell(String),
    Draught(String),
}

/// One effect line. Which kinds may carry it, and its bounds, are in `SPECS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Cast(Shape),
    /// Cabbage Storm: N cabbages fall round the aim (a spell's cast).
    Cabbages(u32),
    /// The Imposture: one counterfeit knight answers and draws fire
    /// (a spell's cast).
    Imposture(u32),
    /// Hedge Maze: a square of hedge walls rises round the aim, with one
    /// gap facing the caster (a spell's cast).
    Hedge(u32),
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
    /// A knight's hits mend the most hurt friend by N, once a tick, within
    /// the lifesteal limit (`stakes::LEECH_PER_SECOND`).
    Mend(u32),
    /// N morningstars spin close round the knight.
    Orbit(u32),
    /// Shots burst where they hit, N% of their damage around, as one
    /// stroke (`aoe`).
    Burst(u32),
    /// Shots fork where they stop on a monster: two fly on at an angle,
    /// past the one struck, and fork again N-1 more times (Path of Exile's
    /// Fork; 2 is its Awakened Fork).
    Fork(u32),
    /// A hit leaves a fart cloud on what it struck, oftener with N.
    Fart(u32),
    /// Shots are N sizes bigger: drawn larger, and they hit what a bigger
    /// shot would (`wishcraft`).
    Size(u32),
    /// Hits shove monsters N steps further back.
    Knock(u32),
    /// Hits set monsters burning, N times as hot (Fire Arrows' burn).
    Burn(u32),
    /// Hits chill monsters: they walk slowly for N times the Lich's chill.
    Chill(u32),
    /// Hits poison monsters, N times as strong as the gas skill's.
    Poison(u32),
    /// Shots fly out and come back to the hand, like the thrown blade.
    Boomerang(u32),
    /// Shots fly N% faster, as far.
    Velocity(u32),
    /// Spoils carried out of the delve for the realm's treasury.
    Bone(u32),
    Wax(u32),
    Ore(u32),
    Gem(u32),
    Ember(u32),
    Scale(u32),
    /// Blink N steps along the aim (the Fae Dagger).
    Blink(u32),
    /// N seconds untouched by hex, chill, web and hook (the Pendragon Sceptre).
    Immune(u32),
    /// Mend every knight near by N (the Censer).
    Censer(u32),
    /// Summons, on play cards only: a minion answers and takes the ally
    /// slot from any minion already in it (never from Beaumains). The
    /// hearth bell's brownie catches a shot.
    Brownie(u32),
    /// The rime whistle's moth freezes a foe for N ticks.
    Moth(u32),
    /// The salted thread's wisp shoves a foe back.
    Wisp(u32),
    /// The glass needle's mite turns a shot around.
    Mite(u32),
    /// The cinder wick's ash sprite pulls a foe in.
    AshSprite(u32),
    /// The lantern mote snuffs a hostile shot.
    Mote(u32),
    /// N creatures of the card's own look answer, behaving as the menu
    /// says (`imps`): a swarm that hunts and bursts, guards that catch
    /// shots, or a turret that looses bolts (a spell's cast, or a play
    /// card's).
    Summon(u32, super::imps::Behaviour),
    /// The reed flute's newt pins a foe for a beat.
    Newt(u32),
    /// The choir crumb's linnet mends a wounded knight.
    Linnet(u32),
    /// The marrow sip's leech sips a foe and mends the knight who called it.
    Leech(u32),
    /// A champion's own spell (`champions`): the card names the feat, and
    /// on any other card it does nothing. Appended, so saved names keep.
    Feat(u32),
    /// The Gravecaller's: a ghoul claws up and fights (a spell's cast, or a
    /// play card's).
    Ghoul(u32),
    /// A barrow warden stands up with his shield and takes the shots meant
    /// for his caller.
    Warden(u32),
    /// A bone totem is planted where the caster aims and spits grave-fire.
    Totem(u32),
    /// Soul Rip: every foe near the aim takes N, and what is torn mends the
    /// caster and their dead (a spell's cast).
    Rip(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Bolt,
    Ring,
    Meteor,
}

impl Shape {
    pub fn word(self) -> &'static str {
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

const SUMMON_SPEC: Spec = Spec {
    word: "summon",
    low: 1,
    high: 6,
    cap: 0,
    hold: false,
    spend: true,
    make: |n| Effect::Summon(n, super::imps::Behaviour::Swarm),
    says: "N creatures of the card's own look answer: `summon N swarm` (hunt and burst, up to 6), `summon N guard` (circle you and each catch one shot, up to 4) or `summon N turret` (stand and loose bolts, up to 2); spell or play cards; a spell's cooldown is at least 4s a swarmer, 5s a guard, 10s a turret",
};

const SPECS: &[Spec] = &[
    Spec {
        word: "cabbages",
        low: 3,
        high: 8,
        cap: 0,
        hold: false,
        spend: false,
        make: Effect::Cabbages,
        says: "N cabbages fall round your aim, thumping where they land",
    },
    Spec {
        word: "imposture",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: false,
        make: Effect::Imposture,
        says: "a counterfeit you steps out and draws the monsters' fire",
    },
    Spec {
        word: "hedge",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: false,
        make: Effect::Hedge,
        says: "a hedge rises round your aim, with a gap for you",
    },
    Spec {
        word: "blink",
        low: 4,
        high: 14,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Blink,
        says: "blink N steps along your aim",
    },
    Spec {
        word: "immune",
        low: 2,
        high: 8,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Immune,
        says: "N seconds untouched by hex, chill, web and hook",
    },
    Spec {
        word: "censer",
        low: 10,
        high: 80,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Censer,
        says: "mend every knight near you by N",
    },
    Spec {
        word: "brownie",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Brownie,
        says: "a hearth brownie answers and catches a shot for you",
    },
    Spec {
        word: "moth",
        low: 30,
        high: 90,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Moth,
        says: "a rime moth answers and freezes one foe for N ticks",
    },
    Spec {
        word: "wisp",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Wisp,
        says: "a salt wisp answers and shoves one foe back",
    },
    Spec {
        word: "mite",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Mite,
        says: "a glass mite answers and turns one shot around",
    },
    Spec {
        word: "sprite",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::AshSprite,
        says: "an ash sprite answers and pulls one foe in",
    },
    Spec {
        word: "newt",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Newt,
        says: "a reed newt answers and pins one foe for a beat",
    },
    Spec {
        word: "linnet",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Linnet,
        says: "a linnet answers and mends the nearest wounded knight",
    },
    Spec {
        word: "leech",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Leech,
        says: "a leech answers and sips the nearest foe to mend you",
    },
    Spec {
        word: "mote",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Mote,
        says: "a lantern mote answers and snuffs one hostile shot",
    },
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
        cap: 40,
        hold: true,
        spend: false,
        make: Effect::Damage,
        says: "shots and swings hit N% harder",
    },
    Spec {
        word: "rate",
        low: 5,
        high: 40,
        cap: 30,
        hold: true,
        spend: false,
        make: Effect::Rate,
        says: "shoot and swing N% faster",
    },
    Spec {
        word: "speed",
        low: 5,
        high: 30,
        cap: 25,
        hold: true,
        spend: false,
        make: Effect::Speed,
        says: "walk N% faster",
    },
    Spec {
        word: "pierce",
        low: 1,
        high: 2,
        cap: 2,
        hold: true,
        spend: false,
        make: Effect::Pierce,
        says: "shots pass through N more monsters",
    },
    Spec {
        word: "shots",
        low: 1,
        high: 3,
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
        cap: 2,
        hold: true,
        spend: false,
        make: Effect::Armor,
        says: "mail: one piece turns aside 6 in the hundred of every hit, two (the most cards give) 11",
    },
    Spec {
        word: "vamp",
        low: 1,
        high: 10,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Vamp,
        says: "mend N health as monsters fall, at most 2 a second",
    },
    Spec {
        word: "volley",
        low: VOLLEY_FLOOR,
        high: 8,
        cap: 8,
        hold: true,
        spend: false,
        make: Effect::Volley,
        says: "every Nth shot flies three wide (5, every fifth, is the tightest)",
    },
    Spec {
        word: "bounce",
        low: 1,
        high: 3,
        cap: 2,
        hold: true,
        spend: false,
        make: Effect::Bounce,
        says: "shots bounce off walls N times",
    },
    Spec {
        word: "homing",
        low: 1,
        high: 3,
        cap: 2,
        hold: true,
        spend: false,
        make: Effect::Homing,
        says: "shots curve toward the nearest monster, harder with N",
    },
    Spec {
        word: "chain",
        low: 1,
        high: 3,
        cap: 1,
        hold: true,
        spend: false,
        make: Effect::Chain,
        says: "a hit sparks on to N more monsters nearby, for half",
    },
    Spec {
        word: "mend",
        low: 1,
        high: 5,
        cap: 2,
        hold: true,
        spend: false,
        make: Effect::Mend,
        says: "your hits mend the most hurt friend by N, at most 2 a second",
    },
    Spec {
        word: "orbit",
        low: 1,
        high: 3,
        cap: 2,
        hold: true,
        spend: false,
        make: Effect::Orbit,
        says: "N morningstars spin close round you, striking 3 times a second and parrying",
    },
    Spec {
        word: "burst",
        low: 15,
        high: 50,
        cap: 25,
        hold: true,
        spend: false,
        make: Effect::Burst,
        says: "shots burst where they hit, N% of their damage to the three nearest (a quarter of that past them)",
    },
    Spec {
        word: "fork",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Fork,
        says: "a shot that stops on a monster forks into two that fly on past it, N times over",
    },
    Spec {
        word: "fart",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Fart,
        says: "your hits leave a fart cloud on what they strike, oftener with N",
    },
    Spec {
        word: "size",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Size,
        says: "your shots are N sizes bigger: drawn larger, and they hit wider",
    },
    Spec {
        word: "knock",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Knock,
        says: "your hits shove monsters N steps further back",
    },
    Spec {
        word: "burn",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Burn,
        says: "your shots are aflame: hits set monsters burning, hotter with N",
    },
    Spec {
        word: "chill",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Chill,
        says: "your shots are frost: hits slow monsters to a crawl, longer with N",
    },
    Spec {
        word: "poison",
        low: 1,
        high: 3,
        cap: 3,
        hold: true,
        spend: false,
        make: Effect::Poison,
        says: "your shots are venom: hits poison monsters, stronger with N",
    },
    Spec {
        word: "boomerang",
        low: 1,
        high: 1,
        cap: 1,
        hold: true,
        spend: false,
        make: Effect::Boomerang,
        says: "your shots fly out, turn, and come back to your hand, striking again on the way",
    },
    Spec {
        word: "velocity",
        low: 10,
        high: 60,
        cap: 60,
        hold: true,
        spend: false,
        make: Effect::Velocity,
        says: "your shots fly N% faster, as far",
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
    Spec {
        word: "feat",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: false,
        make: Effect::Feat,
        says: "a champion's own spell, cast by the card's name; on any other card it does nothing",
    },
    Spec {
        word: "ghoul",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Ghoul,
        says: "a ghoul claws up beside you and fights",
    },
    Spec {
        word: "warden",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Warden,
        says: "a barrow warden takes the shots meant for you and roars foes onto himself",
    },
    Spec {
        word: "totem",
        low: 1,
        high: 1,
        cap: 0,
        hold: false,
        spend: true,
        make: Effect::Totem,
        says: "a bone totem is planted where you aim and spits grave-fire",
    },
    Spec {
        word: "rip",
        low: 10,
        high: 40,
        cap: 0,
        hold: false,
        spend: false,
        make: Effect::Rip,
        says: "every foe near your aim takes N, and what is torn mends you and your dead",
    },
];

impl Effect {
    /// The spoil this effect carries home, if it is one.
    pub fn spoil(self) -> Option<(Spoil, u32)> {
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

    /// The minion this effect calls, if it is a summons.
    pub fn summon(self) -> Option<AllyKind> {
        Some(match self {
            Effect::Brownie(_) => AllyKind::Brownie,
            Effect::Moth(_) => AllyKind::Moth,
            Effect::Wisp(_) => AllyKind::Wisp,
            Effect::Mite(_) => AllyKind::Mite,
            Effect::AshSprite(_) => AllyKind::AshSprite,
            Effect::Mote(_) => AllyKind::Mote,
            Effect::Newt(_) => AllyKind::Newt,
            Effect::Linnet(_) => AllyKind::Linnet,
            Effect::Leech(_) => AllyKind::Leech,
            Effect::Ghoul(_) => AllyKind::Ghoul,
            Effect::Warden(_) => AllyKind::Warden,
            Effect::Totem(_) => AllyKind::Totem,
            _ => return None,
        })
    }

    pub(super) fn parts(self) -> (&'static str, u32) {
        let value = match self {
            Effect::Cast(_) => return ("cast", 0),
            Effect::Cabbages(v) => return ("cabbages", v),
            Effect::Summon(v, _) => return ("summon", v),
            Effect::Imposture(_) => return ("imposture", 1),
            Effect::Hedge(_) => return ("hedge", 1),
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
            | Effect::Fork(v)
            | Effect::Fart(v)
            | Effect::Size(v)
            | Effect::Knock(v)
            | Effect::Burn(v)
            | Effect::Chill(v)
            | Effect::Poison(v)
            | Effect::Boomerang(v)
            | Effect::Velocity(v)
            | Effect::Bone(v)
            | Effect::Wax(v)
            | Effect::Ore(v)
            | Effect::Gem(v)
            | Effect::Ember(v)
            | Effect::Scale(v)
            | Effect::Blink(v)
            | Effect::Immune(v)
            | Effect::Censer(v)
            | Effect::Brownie(v)
            | Effect::Moth(v)
            | Effect::Wisp(v)
            | Effect::Mite(v)
            | Effect::AshSprite(v)
            | Effect::Mote(v)
            | Effect::Newt(v)
            | Effect::Linnet(v)
            | Effect::Leech(v)
            | Effect::Feat(v) => v,
            | Effect::Ghoul(v)
            | Effect::Warden(v)
            | Effect::Totem(v)
            | Effect::Rip(v) => v,
        };
        let word = SPECS
            .iter()
            .find(|s| std::mem::discriminant(&(s.make)(value)) == std::mem::discriminant(&self))
            .map_or("?", |s| s.word);
        (word, value)
    }

    /// Short rules text: `damage +25%`, `heal 35`.
    pub fn label(self) -> String {
        if let Effect::Cast(shape) = self {
            return format!("cast {}", shape.word());
        }
        if let Effect::Summon(n, behaviour) = self {
            return format!("summon {n} {}", behaviour.word());
        }
        let (word, value) = self.parts();
        match self {
            Effect::Damage(_) | Effect::Rate(_) | Effect::Speed(_) | Effect::Velocity(_) => {
                format!("{word} +{value}%")
            }
            Effect::Boomerang(_) => word.to_string(),
            Effect::Ward(_) => format!("ward {value}s"),
            Effect::MaxHp(_) => format!("max hp +{value}"),
            Effect::Pierce(_) | Effect::Shots(_) | Effect::Armor(_) => format!("{word} +{value}"),
            Effect::Volley(_) => format!("every {value}th shot three wide"),
            _ => format!("{word} {value}"),
        }
    }
}

/// A weapon card: one of the delve's arms, or Forge runes checked at tier I.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arm {
    Plain(Weapon),
    Forged(forge::Weapon),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    /// The file name without `.card`: lowercase letters, digits and dashes.
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub rarity: Rarity,
    pub text: String,
    /// Who made it; empty for the delve's own cards.
    pub by: String,
    pub drop: u32,
    pub chest: u32,
    pub effects: Vec<Effect>,
    pub arm: Option<Arm>,
    /// Ink rows, at most 16 by 16; empty means the kind's own glyph.
    pub art: Vec<String>,
    /// The only delve it drops in; none drops anywhere.
    #[serde(default)]
    pub only_in: Option<Pack>,
    /// What a guard card puts on the guard key.
    #[serde(default)]
    pub guard: Option<Guard>,
    /// Recharge seconds; older non-spell cards deserialize with zero.
    #[serde(default)]
    pub cooldown: u32,
    /// What it shows on the field: worn over the knight, its shots, what
    /// orbits, what it summons, an aura (`looks`).
    #[serde(default, skip_serializing_if = "super::looks::Looks::is_empty")]
    pub looks: super::looks::Looks,
    /// The ultimate a held card bestows (`ult <name>`): the identity
    /// start's third wish (`identity`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ult: Option<super::ults::Ult>,
}

impl Card {
    /// A material card: it carries spoils home and drops on its own roll.
    pub fn is_spoil(&self) -> bool {
        self.effects
            .iter()
            .any(|e| e.spoil().is_some_and(|(s, _)| s != Spoil::Gold))
    }

    /// A bottle: a card that mends or wards whoever drinks it.
    pub fn drinks(&self) -> bool {
        self.effects.iter().any(|e| matches!(e, Effect::Heal(_) | Effect::Ward(_)))
    }

    /// Written magic: a spell, or a card that says it is a scroll (the
    /// Thunder Scroll). It lies on the floor as a scroll or a tome
    /// (`pickup_cards::plate_for`), and lands with a flutter and a shimmer.
    pub fn scroll(&self) -> bool {
        self.kind == Kind::Spell || format!("{} {}", self.id, self.name).to_ascii_lowercase().contains("scroll")
    }

    fn drops_in(&self, pack: Pack) -> bool {
        // A deep delve sheds the materials of its kin.
        self.only_in.is_none_or(|p| p == pack || p == pack.kin())
    }

    /// The minion a play card calls, and the card's number for it.
    pub fn summon(&self) -> Option<(AllyKind, u32)> {
        if self.kind != Kind::Play {
            return None;
        }
        self.effects
            .iter()
            .find_map(|e| e.summon().map(|kind| (kind, e.parts().1)))
    }

    /// The minion a card calls, a spell's or a play card's, and the card's
    /// number for it (`minions`).
    pub fn minion(&self) -> Option<(AllyKind, u32)> {
        if !matches!(self.kind, Kind::Play | Kind::Spell) {
            return None;
        }
        self.effects
            .iter()
            .find_map(|e| e.summon().map(|kind| (kind, e.parts().1)))
    }

    /// A spell's soul rip, and what it tears from each foe.
    pub fn rip(&self) -> Option<u32> {
        self.effects.iter().find_map(|e| match *e {
            Effect::Rip(n) if self.kind == Kind::Spell => Some(n),
            _ => None,
        })
    }

    /// The creatures a card's `summon` calls: how many and how they behave.
    pub fn imps(&self) -> Option<(u32, super::imps::Behaviour)> {
        self.effects.iter().find_map(|e| match *e {
            Effect::Summon(n, behaviour) => Some((n, behaviour)),
            _ => None,
        })
    }

    /// The card's art rows, or its kind's glyph when it drew none.
    pub fn art_rows(&self) -> Vec<&str> {
        if self.art.is_empty() {
            glyph(self.kind).to_vec()
        } else {
            self.art.iter().map(String::as_str).collect()
        }
    }

    /// One line of rules: effects, or the weapon's numbers.
    pub fn rules(&self) -> String {
        match &self.arm {
            Some(Arm::Plain(weapon)) => {
                let arms = weapon.arms();
                let pull = if arms.shots > 1 {
                    format!("{}×{}", arms.shots, arms.damage)
                } else {
                    arms.damage.to_string()
                };
                format!(
                    "{pull} dmg · {:.1}/s{}",
                    super::HZ as f32 / arms.cooldown as f32,
                    match arms.pierce {
                        0 => "",
                        u8::MAX => " · pierces all",
                        _ => " · pierces",
                    }
                )
            }
            Some(Arm::Forged(weapon)) => weapon.summary(),
            None if self.guard.is_some() => match self.guard {
                Some(Guard::Shield) => "hold space: block shots in front".into(),
                Some(Guard::Spin) => "space: spin, cutting all around".into(),
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

    /// One effect as the card line that says it: `nova 40`, `cast ring`,
    /// `summon 3 swarm`.
    pub fn line(effect: Effect) -> String {
        match effect {
            Effect::Cast(shape) => format!("cast {}", shape.word()),
            Effect::Summon(n, behaviour) => format!("summon {n} {}", behaviour.word()),
            _ => {
                let (word, value) = effect.parts();
                format!("{word} {value}")
            }
        }
    }

    /// The card as the card file that says it, for a brief to show it
    /// whole: its name, kind, numbers, flavour, effects, looks and art as
    /// the checker took them. A weapon's or a guard's own lines are left
    /// out (no brief shows those).
    pub fn file(&self) -> String {
        let mut out = format!(
            "name {}\nkind {}\nrarity {}\ntext {}\n",
            self.name,
            self.kind.word(),
            self.rarity.word(),
            self.text
        );
        if !self.by.is_empty() {
            out.push_str(&format!("by {}\n", self.by));
        }
        out.push_str(&format!("drop {}\nchest {}\n", self.drop, self.chest));
        if self.kind == Kind::Spell {
            out.push_str(&format!("cooldown {}\n", self.cooldown));
        }
        for effect in &self.effects {
            out.push_str(&Card::line(*effect));
            out.push('\n');
        }
        if let Some(ult) = self.ult {
            out.push_str(&format!("ult {}\n", ult.word()));
        }
        if let Some((colour, shape)) = &self.looks.style {
            out.push_str(&format!("look {colour} {shape}\n"));
        }
        for layer in &self.looks.vfx.layers {
            out.push_str(&layer.line());
            out.push('\n');
        }
        for slot in super::looks::Slot::ALL {
            if let Some(sprite) = self.looks.get(slot) {
                out.push_str(&sprite.to_block(slot));
                out.push('\n');
            }
        }
        if !self.art.is_empty() {
            out.push_str("art\n");
            for row in &self.art {
                out.push_str(row);
                out.push('\n');
            }
        }
        out
    }

    /// Complete authored definition for validation and implementation handoff.
    /// `file` remains the compact brief excerpt used by existing spell callers.
    pub fn definition_file(&self) -> String {
        let mut out = String::new();
        if let Some(arm) = &self.arm {
            match arm {
                Arm::Plain(weapon) => out.push_str(&format!(
                    "arm {}\n",
                    match weapon {
                        Weapon::Fists => "fists",
                        Weapon::Bow => "bow",
                        Weapon::Crossbow => "crossbow",
                        Weapon::Handgonne => "handgonne",
                        Weapon::Notes => "notes",
                        Weapon::Stones => "stones",
                    }
                )),
                Arm::Forged(weapon) => {
                    for line in weapon
                        .runes
                        .lines()
                        .filter(|line| !line.starts_with("name "))
                    {
                        out.push_str(line);
                        out.push('\n');
                    }
                }
            }
        }
        if let Some(guard) = self.guard {
            out.push_str(&match guard {
                Guard::Roll => "guard roll\n".into(),
                Guard::Shield => "guard shield\n".into(),
                Guard::Spin => "guard spin\n".into(),
                Guard::Wall(w) => format!(
                    "guard wall width={} drain={} empower={}\n",
                    w.width, w.drain, w.empower
                ),
            });
        }
        if let Some(pack) = self.only_in {
            out.push_str(&format!(
                "where {}\n",
                match pack {
                    Pack::Crypt => "crypt",
                    Pack::Cavern => "mines",
                    Pack::Hellforge => "keep",
                    other => other.name(),
                }
            ));
        }
        out.push_str(&self.file());
        out
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
pub struct Checked {
    pub card: Card,
    pub notes: Vec<String>,
}

#[derive(Debug)]
pub struct Rejected {
    pub errors: Vec<String>,
}

impl Rejected {
    pub fn fix_it(&self) -> String {
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
pub fn slug(name: &str) -> String {
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

/// Words a card or a wish never carries: the game is played in public, by
/// anyone, so everything in it stays safe for work. Sex and the body's
/// private parts, slurs, the strongest swearing. Toilet humour (a fart, a
/// poop) is the game's own and stays. Whole words only, any case; kept in
/// rot13 so the list itself reads clean.
const NOT_FOR_WORK: &[&str] = &[
    "frk", "frkl", "frkhny", "cravf", "cravfrf", "qvpx", "qvpxf", "pbpx", "pbpxf", "fpuybat",
    "crpxre", "jrvare", "jvrare", "grfgvpyr", "grfgvpyrf", "fpebghz", "onyyfnpx", "ahgfnpx",
    "obbo", "obbof", "obbovrf", "gvgf", "gvggl", "gvggvrf", "avccyr", "avccyrf", "intvan",
    "chffl", "pyvg", "fcrez", "frzra", "phz", "wvmm", "betnfz", "ubeal", "rerpgvba", "obare",
    "qvyqb", "ivoengbe", "pbaqbz", "cbea", "cbeab", "ahqr", "ahqrf", "fgevccre", "ubbxre",
    "juber", "fyhg", "zvys", "uragnv", "afsj", "nany", "nahf", "ohggubyr", "nffubyr",
    "cbjreobggbz", "oqfz", "srgvfu", "xvaxl", "yvatrevr", "cnagvrf", "gubat", "travgny",
    "travgnyf", "znfgheongr", "jnax", "oybjwbo", "unaqwbo", "evzwbo", "shpx", "shpxvat",
    "shpxre", "zbgureshpxre", "fuvg", "fuvggl", "ohyyfuvg", "ovgpu", "ovgpurf", "phag",
    "avttre", "avttn", "snttbg", "snt", "ergneq", "ergneqrq", "genaal", "fcvp", "puvax", "xvxr",
    "qlxr",
];

/// The first word of `text` the game never carries (`NOT_FOR_WORK`), if any.
pub fn not_for_work(text: &str) -> Option<String> {
    let rot13 = |w: &str| -> String {
        w.chars()
            .map(|c| match c.to_ascii_lowercase() {
                l @ 'a'..='m' => (l as u8 + 13) as char,
                l @ 'n'..='z' => (l as u8 - 13) as char,
                l => l,
            })
            .collect()
    };
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .find(|w| NOT_FOR_WORK.contains(&rot13(w).as_str()))
        .map(str::to_lowercase)
}

/// Check one card file. `id` usually comes from the file name.
pub fn check(id: &str, raw: &str) -> Result<Checked, Rejected> {
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    if raw.len() > MAX_FILE_BYTES {
        return Err(Rejected {
            errors: vec![format!(
                "the file is over {MAX_FILE_BYTES} bytes with its sprites"
            )],
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
    let mut ult = None;
    let mut cooldown = None;
    let script = super::script::read(raw);
    let art = script.art;
    if script.art_blocks > 1 {
        errors.push("only one art block".into());
    }
    if script.text_bytes > MAX_BYTES {
        return Err(Rejected {
            errors: vec![format!(
                "the card's lines are over {MAX_BYTES} bytes (sprite rows aside)"
            )],
        });
    }
    let mut looks = super::looks::Looks::default();
    for (n, header, rows) in &script.sprites {
        // `draw <slot> [at=<place>] [paint=<sha256> [normal=<sha256>]
        // [pixel]] [hd=<sha256>] [move=<words>]`: a worn piece drawn small is
        // put on the knight at the place named; a painted one names its
        // picture in the paint store (and its normal map's, and whether it is
        // pixel art); a moving one its motion words.
        let mut parts = header.split_whitespace();
        let word = parts.next().unwrap_or("");
        let (mut at, mut paint, mut normal, mut moves, mut placed) = (None, None, None, None, None);
        let (mut hd, mut pixel) = (None, None);
        let mut pal: Option<Vec<(char, [u8; 3])>> = None;
        let mut bad = None;
        for part in parts {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            let seen = match key {
                "at" if word == "worn" => at.replace(value).is_some(),
                // Already put at its place on the canvas (a sheet cut, a
                // card written back out): only the place is kept.
                "placed" if word == "worn" && super::looks::place(value).is_some() => {
                    placed.replace(value).is_some()
                }
                "paint" if super::looks::paint_name(value) => {
                    paint.replace(value.to_string()).is_some()
                }
                "paint" => {
                    bad = Some(format!(
                        "line {n}: paint=`{value}` — a painting is named by its sha256, 64 hex digits"
                    ));
                    break;
                }
                "normal" if super::looks::paint_name(value) => {
                    normal.replace(value.to_string()).is_some()
                }
                "normal" => {
                    bad = Some(format!(
                        "line {n}: normal=`{value}` — a normal map is named by its sha256, 64 hex digits"
                    ));
                    break;
                }
                // A painting shown only deep enough down (`Sprite::hd`).
                "hd" if super::looks::paint_name(value) => hd.replace(value.to_string()).is_some(),
                "hd" => {
                    bad = Some(format!(
                        "line {n}: hd=`{value}` — a painting is named by its sha256, 64 hex digits"
                    ));
                    break;
                }
                "pixel" if value.is_empty() && !part.contains('=') => pixel.replace(true).is_some(),
                // The sprite's own inks, any colour (`Sprite::pal`).
                "pal" => match super::looks::parse_pal(value) {
                    Ok(list) => pal.replace(list).is_some(),
                    Err(why) => {
                        bad = Some(format!("line {n}: {why}"));
                        break;
                    }
                },
                "move" => match super::looks::Move::parse_list(value) {
                    Ok(list) => moves.replace(list).is_some(),
                    Err(why) => {
                        bad = Some(format!("line {n}: {why}"));
                        break;
                    }
                },
                _ => {
                    bad = Some(format!(
                        "line {n}: `draw {header}` — a sprite line is `draw <slot>`, and may add `paint=<sha256>` (and `pixel`) and `move=<words>`, and `pal=<inks>` for colours of its own; a worn one `at=<place>`"
                    ));
                    break;
                }
            };
            if seen {
                bad = Some(format!("line {n}: `{key}=` appears twice on `draw {word}`"));
                break;
            }
        }
        if bad.is_none() && normal.is_some() && paint.is_none() {
            bad = Some(format!("line {n}: `normal=` is a painting's, beside its `paint=`"));
        }
        if bad.is_none() && pixel.is_some() && paint.is_none() {
            bad = Some(format!("line {n}: `pixel` is a painting's, beside its `paint=`"));
        }
        if let Some(why) = bad {
            errors.push(why);
            continue;
        }
        let rows = match at.map(|place| {
            super::looks::Sprite::from_strip(super::looks::Slot::Worn, rows)
                .and_then(|small| super::looks::place_on_body(&small.frames, place))
        }) {
            None => rows.clone(),
            Some(Ok(frames)) => super::looks::Sprite {
                frames,
                ..Default::default()
            }
            .to_block(super::looks::Slot::Worn)
                .lines()
                .skip(1)
                .map(str::to_string)
                .collect(),
            Some(Err(why)) => {
                errors.push(format!("line {n}: {why}"));
                continue;
            }
        };
        let rows = &rows;
        match super::looks::Slot::parse(word) {
            None => errors.push(format!(
                "line {n}: draw `{word}` — use worn, shot, orbit, summon or aura"
            )),
            Some(slot) if looks.get(slot).is_some() => {
                errors.push(format!("line {n}: draw {} appears twice", slot.word()))
            }
            Some(slot) => match super::looks::Sprite::from_strip(slot, rows) {
                Ok(mut sprite) => {
                    sprite.pal = pal.unwrap_or_default();
                    if let Some(c) = sprite.unnamed_ink() {
                        errors.push(format!(
                            "line {n}: draw {} uses its own ink `{c}` but its pal= gives it no colour",
                            slot.word()
                        ));
                        continue;
                    }
                    if slot == super::looks::Slot::Worn {
                        if let Err(why) = super::artistry::fit_worn(&mut sprite, at.or(placed).unwrap_or("whole")) {
                            errors.push(format!("line {n}: {why}"));
                            continue;
                        }
                    }
                    *looks.slot_mut(slot) = Some(super::looks::Sprite {
                        // Shown whole wherever it is, a painting needs no
                        // deep-floor twin.
                        hd: if paint.is_some() { None } else { hd },
                        paint,
                        normal,
                        pixel: pixel.unwrap_or(false),
                        moves: moves.unwrap_or_default(),
                        at: at.or(placed).and_then(super::looks::place).map(str::to_string),
                        ..sprite
                    })
                }
                Err(why) => errors.push(format!("line {n}: {why}")),
            },
        }
    }
    let mut style_line = None;
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
                "notes" => plain = Some(Weapon::Notes),
                "stones" => plain = Some(Weapon::Stones),
                other => errors.push(format!(
                    "line {n}: arm `{other}` — use bow, crossbow, handgonne or notes, or Forge rune lines"
                )),
            },
            "bolt" | "spread" | "melee" | "throw" => runes.push(format!("{word} {rest}")),
            // On a weapon, a rune; on any other card, the colour and shape
            // its knight's shots take.
            // How its magic moves: a line that does not read is set aside
            // with a note, never the card (`vfx`).
            "vfx" => match super::vfx::Layer::read(rest) {
                Ok((layer, moved)) if looks.vfx.layers.len() < super::vfx::MOST_LAYERS => {
                    looks.vfx.layers.push(layer);
                    notes.extend(moved);
                }
                Ok(_) => notes.push(format!("line {n}: a card's look has at most {} vfx lines; this one is set aside", super::vfx::MOST_LAYERS)),
                Err(why) => notes.push(format!("line {n}: {why}; set aside")),
            },
            "look" => {
                runes.push(format!("{word} {rest}"));
                style_line = Some((n, rest.to_ascii_lowercase()));
            }
            "sheet" => errors.push(format!(
                "line {n}: `sheet` names a picture in .angel/dungeon/sheets; it is cut into a `draw` block when the card loads from the workspace — a card sent on its own draws its sprite as rows"
            )),
            "summon" => {
                let lower = rest.to_ascii_lowercase();
                let mut parts = lower.split_whitespace();
                let count = parts.next().and_then(|v| v.parse::<u32>().ok());
                let behaviour = match parts.next() {
                    None => Some(super::imps::Behaviour::Swarm),
                    Some(word) => super::imps::Behaviour::parse(word),
                };
                match (count, behaviour) {
                    (Some(count), Some(behaviour)) if parts.next().is_none() => {
                        let most = behaviour.most();
                        let clamped = count.clamp(1, most);
                        if clamped != count {
                            notes.push(format!(
                                "summon {count} {} clamped to {clamped}",
                                behaviour.word()
                            ));
                        }
                        effects.push((n, &SUMMON_SPEC, Effect::Summon(clamped, behaviour)));
                    }
                    _ => errors.push(format!(
                        "line {n}: summon takes a count and swarm, guard or turret (`summon 4 swarm`)"
                    )),
                }
            }
            "ult" => match super::ults::Ult::parse(rest) {
                Some(found) if ult.is_none() => ult = Some(found),
                Some(_) => errors.push(format!("line {n}: ult appears twice")),
                None => errors.push(format!("line {n}: ult `{rest}` — use {}", super::ults::Ult::words())),
            },
            "guard" => {
                let lower = rest.to_ascii_lowercase();
                let mut parts = lower.split_whitespace();
                match parts.next().unwrap_or("") {
                    "roll" => guard = Some(Guard::Roll),
                    "shield" => guard = Some(Guard::Shield),
                    "spin" => guard = Some(Guard::Spin),
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
                        "line {n}: guard `{other}` — roll, shield, spin, or wall width=2-8 drain=10-50 empower=0-40"
                    )),
                }
            }
            _ => {
                if let Some(spec) = SPECS.iter().find(|s| s.word == word) {
                    // A word with one value (`boomerang`) may stand alone.
                    let value = if rest.trim().is_empty() && spec.low == spec.high {
                        Ok(spec.low)
                    } else {
                        rest.trim_start_matches('+')
                            .trim_end_matches(['%', 's'])
                            .parse::<u32>()
                    };
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
    if let Some(word) = [&id, &name, &text].into_iter().find_map(|s| not_for_work(s)) {
        errors.push(format!(
            "`{word}` is not for this game: every card stays safe for work (a rude ask is made a harmless, funny stand-in)"
        ));
    }
    if text.chars().count() > MAX_TEXT {
        notes.push(format!("text cut to {MAX_TEXT} characters"));
        text = text.chars().take(MAX_TEXT).collect();
    }
    let Some(kind) = kind else {
        errors.push("the card needs a `kind`: take, hold, play, arm, guard or spell".into());
        return Err(Rejected { errors });
    };
    // A held card's number counts no higher than the deck may add up to:
    // past the cap it would only promise what the knight never gets.
    if kind == Kind::Hold {
        for (_, spec, effect) in effects.iter_mut() {
            let value = effect.parts().1;
            if spec.cap > 0 && value > spec.cap {
                notes.push(format!(
                    "{} {value} is over what a deck may hold ({}), clamped to {}",
                    spec.word, spec.cap, spec.cap
                ));
                *effect = (spec.make)(spec.cap);
            }
        }
    }
    // `look <colour> <shape>` off a weapon: the colour and shape the
    // knight's shots take.
    if kind != Kind::Arm {
        runes.retain(|r| !r.starts_with("look "));
        if let Some((n, rest)) = style_line.take() {
            let mut parts = rest.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(colour), Some(shape))
                    if forge::valid_look(colour, shape) && parts.next().is_none() =>
                {
                    looks.style = Some((colour.to_string(), shape.to_string()));
                }
                _ => errors.push(format!(
                    "line {n}: look takes a colour ({}) and a shape ({})",
                    forge::COLOURS.join(" "),
                    forge::SHAPES.join(" ")
                )),
            }
        }
    }
    // What a card shows must be something its kind keeps on the field.
    {
        use super::looks::Slot;
        let fits = |slot: Slot| match slot {
            Slot::Worn | Slot::Aura => matches!(kind, Kind::Hold | Kind::Guard | Kind::Arm),
            Slot::Shot => matches!(kind, Kind::Hold | Kind::Arm | Kind::Spell),
            Slot::Orbit => kind == Kind::Hold,
            Slot::Summon => effects
                .iter()
                .any(|(_, _, e)| matches!(e, Effect::Summon(..))),
        };
        for slot in Slot::ALL {
            if looks.get(slot).is_some() && !fits(slot) {
                errors.push(match slot {
                    Slot::Summon => "a `draw summon` needs a `summon` effect on the card".into(),
                    Slot::Orbit => "`draw orbit` belongs on a hold card".into(),
                    _ => format!(
                        "`draw {}` does not show on a {} card",
                        slot.word(),
                        kind.word()
                    ),
                });
            }
        }
        if looks.style.is_some() && !matches!(kind, Kind::Hold | Kind::Spell) {
            errors.push(format!("`look` does not show on a {} card", kind.word()));
        }
    }
    // A summoning spell recharges no faster than its creatures earn.
    if let (Kind::Spell, Some(seconds)) = (kind, cooldown) {
        let floor = effects
            .iter()
            .map(|(_, _, e)| match *e {
                Effect::Summon(n, behaviour) => n * behaviour.seconds_each(),
                _ => 0,
            })
            .max()
            .unwrap_or(0)
            .min(SPELL_COOLDOWN.1);
        if seconds < floor {
            notes.push(format!(
                "cooldown raised to {floor}s: that many creatures take longer to call"
            ));
            cooldown = Some(floor);
        }
    }
    if kind == Kind::Spell && cooldown.is_none() {
        errors.push("a spell needs `cooldown <seconds>` (1–120)".into());
    } else if kind != Kind::Spell && cooldown.is_some() {
        errors.push("`cooldown` belongs on `kind spell`".into());
    }
    if ult.is_some() && kind != Kind::Hold {
        errors.push("`ult` belongs on `kind hold`".into());
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
                        let mut weapon = checked.weapon;
                        // A homemade arm is never slow: its shots fly at
                        // least as fast as the stock arms' (`feel`).
                        if let Some(bolt) = weapon.bolt.as_mut() {
                            bolt.speed = bolt.speed.max(FORGED_SPEED);
                        }
                        arm = Some(Arm::Forged(weapon));
                    }
                    Err(rejected) => errors.extend(rejected.errors),
                }
            }
            (None, true) => errors.push(
                "an arm card needs `arm bow|crossbow|handgonne|notes` or Forge runes (bolt/melee/spread/look)".into(),
            ),
        }
    } else {
        if plain.is_some() || !runes.is_empty() {
            errors.push(format!(
                "weapon lines belong on `kind arm`, not `kind {}`",
                kind.word()
            ));
        }
        // A held worn picture may be ink alone (the bathroom mirror).
        // It is still checked by the ordinary sprite/card painter pipeline.
        if effects.is_empty() && ult.is_none() && !(kind == Kind::Hold && looks.worn.is_some()) {
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
                        | Effect::Blink(_)
                        | Effect::Bombs(_)
                        | Effect::Nova(_)
                        | Effect::Ward(_)
                        | Effect::Cast(_)
                        | Effect::Cabbages(_)
                        | Effect::Imposture(_)
                        | Effect::Hedge(_)
                        | Effect::Summon(..)
                        | Effect::Feat(_)
                        | Effect::Ghoul(_)
                        | Effect::Warden(_)
                        | Effect::Totem(_)
                        | Effect::Rip(_)
                )
            } else if matches!(effect, Effect::Summon(..)) {
                kind == Kind::Play
            } else if kind == Kind::Hold {
                spec.hold
            } else if effect.summon().is_some() {
                // A minion answers a played card, not one walked over.
                kind == Kind::Play
            } else {
                spec.spend
            };
            if !fits {
                errors.push(format!(
                    "line {n}: {} works on {} cards, not {}",
                    spec.word,
                    if spec.word == "cast" || spec.word == "rip" {
                        "spell"
                    } else if matches!(spec.word, "summon" | "ghoul" | "warden" | "totem") {
                        "spell or play"
                    } else if spec.hold {
                        "hold"
                    } else if effect.summon().is_some() {
                        "play"
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
    if let Some(sprite) = looks.worn.as_mut().filter(|s| s.at.is_none()) {
        let at = super::looks::default_worn_part(&name);
        if at != "whole" {
            sprite.at = Some(at.into());
            if let Err(why) = super::artistry::fit_worn(sprite, at) { errors.push(why); }
        }
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
            ult,
            cooldown: cooldown.unwrap_or(0),
            looks,
        },
        notes,
    })
}

/// What a knight's held cards add, already capped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bonus {
    pub damage: u32,
    pub rate: u32,
    pub speed: u32,
    pub pierce: u32,
    pub shots: u32,
    pub armor: u32,
    pub vamp: u32,
    /// Every Nth shot flies three wide; 0 for none.
    #[serde(default)]
    pub volley: u32,
    #[serde(default)]
    pub bounce: u32,
    #[serde(default)]
    pub homing: u32,
    #[serde(default)]
    pub chain: u32,
    #[serde(default)]
    pub mend: u32,
    #[serde(default)]
    pub orbit: u32,
    #[serde(default)]
    pub burst: u32,
    #[serde(default)]
    pub fork: u32,
    #[serde(default)]
    pub fart: u32,
    /// What wishes made literal (`wishcraft`): bigger shots, harder
    /// shoves, burning, chilling and poisoning hits, shots that come back,
    /// faster shots. Left off the wire at zero.
    #[serde(default, skip_serializing_if = "nothing")]
    pub size: u32,
    #[serde(default, skip_serializing_if = "nothing")]
    pub knock: u32,
    #[serde(default, skip_serializing_if = "nothing")]
    pub burn: u32,
    #[serde(default, skip_serializing_if = "nothing")]
    pub chill: u32,
    #[serde(default, skip_serializing_if = "nothing")]
    pub poison: u32,
    #[serde(default, skip_serializing_if = "nothing")]
    pub boomerang: u32,
    #[serde(default, skip_serializing_if = "nothing")]
    pub velocity: u32,
}

fn nothing(n: &u32) -> bool {
    *n == 0
}

/// What each copy of one card is worth, in percent: the first whole, the
/// second half, the third a quarter, and nothing after.
const COPIES: [f32; 3] = [1.0, 0.5, 0.25];

/// The held words, in the order `Bonus::of` sums them.
const WORDS: [&str; 22] = [
    "damage", "rate", "speed", "pierce", "shots", "armor", "vamp", "bounce", "homing", "chain",
    "mend", "orbit", "burst", "fork", "fart", "size", "knock", "burn", "chill", "poison",
    "boomerang", "velocity",
];

/// Wishes of one family (two wishes for faster shots) are worth less each:
/// the strongest whole, the next half, the next a quarter, none after.
pub const WISH_STEPS: [f32; 3] = [1.0, 0.5, 0.25];

/// The most a knight's wishes add up to of each word, under the card cap
/// (`cap`): a wish is a gift, not the whole build.
pub fn wish_cap(word: &str) -> u32 {
    match word {
        "damage" => 25,
        "rate" => 20,
        "speed" => 20,
        "velocity" => 40,
        "burst" => 15,
        // More shots multiply every other gift: one, from wishes.
        "shots" => 1,
        _ => cap(word).min(2),
    }
}

/// The most a wish can ever give of held word `word`, from all of a
/// knight's wishes together (`wish_cap`, under the card cap): `None` for a
/// word wishes are not capped on (`max_hp`, `volley`).
pub fn wish_most(word: &str) -> Option<u32> {
    WORDS.contains(&word).then(|| wish_cap(word).min(cap(word)))
}

/// How much a knight's wishes may give together, by the floor they stand
/// on (the last for every floor below): in whole words at their wish cap.
/// Floor 1's three wishes fit; a deep knight's twenty do not, and what they
/// hold is given in proportion.
pub const WISH_BUDGET: [f32; 7] = [3.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

/// A word counted in whole steps (pierce, shots, armour ...), not percent.
fn whole(word: &str) -> bool {
    cap(word) <= 3
}

impl Bonus {
    /// What `held` adds, with no floor's budget on its wishes.
    pub fn of<'a>(held: impl IntoIterator<Item = &'a Card>) -> Bonus {
        Bonus::within(held, None)
    }

    /// What `held` adds for a knight on `floor`: found cards as they come
    /// (copies of one worth less, `COPIES`), wishes by their own clamps
    /// (`WISH_STEPS`, `wish_cap`, `WISH_BUDGET` when a floor is named), a
    /// wish's super effect only with its recipe (`combos`), all of it
    /// under the card caps.
    pub fn within<'a>(held: impl IntoIterator<Item = &'a Card>, floor: Option<u32>) -> Bonus {
        let held: Vec<&Card> = held.into_iter().collect();
        let families = super::combos::held_families(held.iter().copied());
        let mut seen: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        let mut sums = [0.0f32; 22];
        let mut wished: [Vec<f32>; 22] = Default::default();
        let mut bonus = Bonus::default();
        for card in held {
            let wish = super::combos::is_wish(card);
            let copy = seen.entry(card.id.as_str()).or_insert(0);
            let worth = COPIES.get(*copy).copied().unwrap_or(0.0);
            *copy += 1;
            for effect in card.effects.iter().copied() {
                if wish
                    && let Some(word) = super::combos::effect_word(&effect)
                    && !super::combos::unlocked(word, &families)
                {
                    // Not yet: it hits a little harder till its recipe is held.
                    wished[0].push(super::combos::STAND_IN as f32);
                    continue;
                }
                let slot = match effect {
                    Effect::Damage(v) => Some((0, v)),
                    Effect::Rate(v) => Some((1, v)),
                    Effect::Speed(v) => Some((2, v)),
                    Effect::Pierce(v) => Some((3, v)),
                    Effect::Shots(v) => Some((4, v)),
                    Effect::Armor(v) => Some((5, v)),
                    Effect::Vamp(v) => Some((6, v)),
                    Effect::Bounce(v) => Some((7, v)),
                    Effect::Homing(v) => Some((8, v)),
                    Effect::Chain(v) => Some((9, v)),
                    Effect::Mend(v) => Some((10, v)),
                    Effect::Orbit(v) => Some((11, v)),
                    Effect::Burst(v) => Some((12, v)),
                    Effect::Fork(v) => Some((13, v)),
                    Effect::Fart(v) => Some((14, v)),
                    Effect::Size(v) => Some((15, v)),
                    Effect::Knock(v) => Some((16, v)),
                    Effect::Burn(v) => Some((17, v)),
                    Effect::Chill(v) => Some((18, v)),
                    Effect::Poison(v) => Some((19, v)),
                    Effect::Boomerang(v) => Some((20, v)),
                    Effect::Velocity(v) => Some((21, v)),
                    // Volleys don't add up: the tightest cadence held
                    // counts, and none is tighter than `VOLLEY_FLOOR`.
                    Effect::Volley(v) => {
                        let v = v.max(VOLLEY_FLOOR);
                        bonus.volley = if bonus.volley == 0 {
                            v
                        } else {
                            bonus.volley.min(v)
                        };
                        None
                    }
                    _ => None,
                };
                if let Some((i, value)) = slot {
                    if wish {
                        wished[i].push(value as f32);
                    } else {
                        sums[i] += value as f32 * worth;
                    }
                }
            }
        }
        // Each word's wishes, strongest first, each further one worth less,
        // to the word's wish cap; then the floor's budget over them all.
        let mut share = [0.0f32; 22];
        let mut power = 0.0;
        for i in 0..22 {
            let mut v = std::mem::take(&mut wished[i]);
            v.sort_by(|a, b| b.total_cmp(a));
            let sum: f32 = v.iter().zip(WISH_STEPS).map(|(v, k)| v * k).sum();
            let top = wish_cap(WORDS[i]) as f32;
            share[i] = sum.min(top);
            if top > 0.0 {
                power += share[i] / top;
            }
        }
        if let Some(floor) = floor {
            let budget = WISH_BUDGET[(floor as usize).min(WISH_BUDGET.len() - 1)];
            if power > budget {
                let k = budget / power;
                share.iter_mut().for_each(|s| *s *= k);
            }
        }
        let capped = |i: usize| {
            let word = WORDS[i];
            let value = if whole(word) {
                // A whole step needs three quarters of one.
                let found = if word == "armor" { sums[i].floor() } else { sums[i].round() };
                found + (share[i] + 0.25).floor()
            } else {
                (sums[i] + share[i]).round()
            };
            (value as u32).min(cap(word))
        };
        bonus.damage = capped(0);
        bonus.rate = capped(1);
        bonus.speed = capped(2);
        bonus.pierce = capped(3);
        bonus.shots = capped(4);
        // Mail is worn in whole pieces: copies that add up to part of one
        // add none.
        bonus.armor = capped(5);
        bonus.vamp = capped(6);
        bonus.bounce = capped(7);
        bonus.homing = capped(8);
        bonus.chain = capped(9);
        bonus.mend = capped(10);
        bonus.orbit = capped(11);
        bonus.burst = capped(12);
        bonus.fork = capped(13);
        bonus.fart = capped(14);
        bonus.size = capped(15);
        bonus.knock = capped(16);
        bonus.burn = capped(17);
        bonus.chill = capped(18);
        bonus.poison = capped(19);
        // A shot comes back or it doesn't: any copy held is the whole of it.
        bonus.boomerang = u32::from(sums[20] + share[20] > 0.0).min(cap("boomerang"));
        bonus.velocity = capped(21);
        bonus
    }
}

/// No volley comes round oftener than every this many shots.
const VOLLEY_FLOOR: u32 = 5;

/// The numbers a card's `word` line takes, low to high, if it is a word.
pub fn range(word: &str) -> Option<(u32, u32)> {
    SPECS.iter().find(|s| s.word == word).map(|s| (s.low, s.high))
}

/// The most a knight's held cards may add up to of effect `word`.
pub fn cap(word: &str) -> u32 {
    SPECS.iter().find(|s| s.word == word).map_or(0, |s| s.cap)
}

/// The cards a run can find, sorted by id so every roll replays.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "SavedBook")]
pub struct Book {
    /// Content changes, including replacement, require a whole mirror update.
    #[serde(default)]
    pub revision: usize,
    pub cards: Vec<Card>,
}

/// A book as a save kept it. Its own cards stay as they were; the delve's
/// built-in cards it never knew are added, so a card a newer delve lays
/// out (a secret's) is never one an older save cannot pick up.
#[derive(Deserialize)]
struct SavedBook {
    #[serde(default)]
    revision: usize,
    cards: Vec<Card>,
}

impl From<SavedBook> for Book {
    fn from(saved: SavedBook) -> Book {
        let mut book = Book {
            revision: saved.revision,
            cards: saved.cards,
        };
        for (id, raw) in BUILTIN {
            if book.get(id).is_none()
                && let Ok(checked) = check(id, raw)
            {
                book.insert(checked.card);
            }
        }
        book
    }
}

impl Default for Book {
    fn default() -> Self {
        Book::builtin()
    }
}

/// The delve's own cards, as files a friend can copy.
/// What all the spells together weigh on an ordinary drop and as a chest's
/// prize: what the book's first nine spells weighed before the 10-30
/// catalogue grew them to ninety, so a delve still finds two or three.
pub const SPELL_DROP_SHARE: u32 = 27;
pub const SPELL_CHEST_SHARE: u32 = 15;

pub const BUILTIN: &[(&str, &str)] = &[
    ("cellar-key", include_str!("../../../../cockpit/assets/dungeon/cards/cellar-key.card")),
    ("cinder-service", include_str!("../../../../cockpit/assets/dungeon/cards/cinder-service.card")),
    ("moonshine-prism", include_str!("../../../../cockpit/assets/dungeon/cards/moonshine-prism.card")),
    ("last-call-fork", include_str!("../../../../cockpit/assets/dungeon/cards/last-call-fork.card")),
    ("mothwing-tea", include_str!("../../../../cockpit/assets/dungeon/cards/mothwing-tea.card")),
    ("astral-barspoon", include_str!("../../../../cockpit/assets/dungeon/cards/astral-barspoon.card")),
    ("phoenix-matchbook", include_str!("../../../../cockpit/assets/dungeon/cards/phoenix-matchbook.card")),
    ("undertow-anchor", include_str!("../../../../cockpit/assets/dungeon/cards/undertow-anchor.card")),
    ("stormglass-dial", include_str!("../../../../cockpit/assets/dungeon/cards/stormglass-dial.card")),
    ("abyssal-bellhelm", include_str!("../../../../cockpit/assets/dungeon/cards/abyssal-bellhelm.card")),
    ("pinwheel-break", include_str!("../../../../cockpit/assets/dungeon/cards/pinwheel-break.card")),
    ("ghostlight-lantern", include_str!("../../../../cockpit/assets/dungeon/cards/ghostlight-lantern.card")),
    ("kraken-corkscrew", include_str!("../../../../cockpit/assets/dungeon/cards/kraken-corkscrew.card")),
    ("widows-toast", include_str!("../../../../cockpit/assets/dungeon/cards/widows-toast.card")),
    ("blackcap-reserve", include_str!("../../../../cockpit/assets/dungeon/cards/blackcap-reserve.card")),
    ("crescendo-clef", include_str!("../../../../cockpit/assets/dungeon/cards/crescendo-clef.card")),
    ("viper-decanter", include_str!("../../../../cockpit/assets/dungeon/cards/viper-decanter.card")),
    ("roulette-spur", include_str!("../../../../cockpit/assets/dungeon/cards/roulette-spur.card")),
    ("railway-shaker", include_str!("../../../../cockpit/assets/dungeon/cards/railway-shaker.card")),
    ("crabclaw-pauldron", include_str!("../../../../cockpit/assets/dungeon/cards/crabclaw-pauldron.card")),
    ("last-call-buckler", include_str!("../../../../cockpit/assets/dungeon/cards/last-call-buckler.card")),
    ("witching-turntable", include_str!("../../../../cockpit/assets/dungeon/cards/witching-turntable.card")),
    ("candlefish-skewer", include_str!("../../../../cockpit/assets/dungeon/cards/candlefish-skewer.card")),
    ("lobster-telegraph", include_str!("../../../../cockpit/assets/dungeon/cards/lobster-telegraph.card")),
    ("comet-dumpling", include_str!("../../../../cockpit/assets/dungeon/cards/comet-dumpling.card")),
    ("mantis-tongs", include_str!("../../../../cockpit/assets/dungeon/cards/mantis-tongs.card")),
    ("sawtooth-menu", include_str!("../../../../cockpit/assets/dungeon/cards/sawtooth-menu.card")),
    ("stormpipe-organ", include_str!("../../../../cockpit/assets/dungeon/cards/stormpipe-organ.card")),
    ("matchstick-ferry", include_str!("../../../../cockpit/assets/dungeon/cards/matchstick-ferry.card")),
    ("dragonfruit-kebab", include_str!("../../../../cockpit/assets/dungeon/cards/dragonfruit-kebab.card")),
    ("cuckoo-breach", include_str!("../../../../cockpit/assets/dungeon/cards/cuckoo-breach.card")),
    ("lobster-caliper", include_str!("../../../../cockpit/assets/dungeon/cards/lobster-caliper.card")),
    ("cinderpear-bisque", include_str!("../../../../cockpit/assets/dungeon/cards/cinderpear-bisque.card")),
    ("stormglass-eclair", include_str!("../../../../cockpit/assets/dungeon/cards/stormglass-eclair.card")),
    ("comet-siphon", include_str!("../../../../cockpit/assets/dungeon/cards/comet-siphon.card")),
    ("oyster-aegis", include_str!("../../../../cockpit/assets/dungeon/cards/oyster-aegis.card")),
    ("last-call-censer", include_str!("../../../../cockpit/assets/dungeon/cards/last-call-censer.card")),
    ("gallows-sun", include_str!("../../../../cockpit/assets/dungeon/cards/gallows-sun.card")),
    ("rookstep-sigil", include_str!("../../../../cockpit/assets/dungeon/cards/rookstep-sigil.card")),
    ("escapement-heart", include_str!("../../../../cockpit/assets/dungeon/cards/escapement-heart.card")),
    ("tempest-skate", include_str!("../../../../cockpit/assets/dungeon/cards/tempest-skate.card")),
    ("borrowed-regular", include_str!("../../../../cockpit/assets/dungeon/cards/borrowed-regular.card")),
    ("widowglass-hourglass", include_str!("../../../../cockpit/assets/dungeon/cards/widowglass-hourglass.card")),
    ("saints-saltcellar", include_str!("../../../../cockpit/assets/dungeon/cards/saints-saltcellar.card")),
    ("parlour-labyrinth", include_str!("../../../../cockpit/assets/dungeon/cards/parlour-labyrinth.card")),
    ("siege-samovar", include_str!("../../../../cockpit/assets/dungeon/cards/siege-samovar.card")),
    ("last-call-saucers", include_str!("../../../../cockpit/assets/dungeon/cards/last-call-saucers.card")),
    ("vesper-apiary", include_str!("../../../../cockpit/assets/dungeon/cards/vesper-apiary.card")),
    ("sanguine-ampoule", include_str!("../../../../cockpit/assets/dungeon/cards/sanguine-ampoule.card")),
    ("tollkeepers-visor", include_str!("../../../../cockpit/assets/dungeon/cards/tollkeepers-visor.card")),
    ("needlewake-spool", include_str!("../../../../cockpit/assets/dungeon/cards/needlewake-spool.card")),
    ("redshift-cuff", include_str!("../../../../cockpit/assets/dungeon/cards/redshift-cuff.card")),
    ("starfall-warrant", include_str!("../../../../cockpit/assets/dungeon/cards/starfall-warrant.card")),
    ("breachbell-tonic", include_str!("../../../../cockpit/assets/dungeon/cards/breachbell-tonic.card")),
    ("glassway-ticket", include_str!("../../../../cockpit/assets/dungeon/cards/glassway-ticket.card")),
    ("undertow-carillon", include_str!("../../../../cockpit/assets/dungeon/cards/undertow-carillon.card")),
    ("cinder-votive", include_str!("../../../../cockpit/assets/dungeon/cards/cinder-votive.card")),
    ("pearlwake-flask", include_str!("../../../../cockpit/assets/dungeon/cards/pearlwake-flask.card")),
    ("railwake-prism", include_str!("../../../../cockpit/assets/dungeon/cards/railwake-prism.card")),
    ("embercrab-bento", include_str!("../../../../cockpit/assets/dungeon/cards/embercrab-bento.card")),
    ("lantern-jury", include_str!("../../../../cockpit/assets/dungeon/cards/lantern-jury.card")),
    ("blackberry-buckler", include_str!("../../../../cockpit/assets/dungeon/cards/blackberry-buckler.card")),
    ("powderplum-tart", include_str!("../../../../cockpit/assets/dungeon/cards/powderplum-tart.card")),
    ("mothlight-censer", include_str!("../../../../cockpit/assets/dungeon/cards/mothlight-censer.card")),
    ("oyster-mortgage", include_str!("../../../../cockpit/assets/dungeon/cards/oyster-mortgage.card")),
    ("firefly-sorbet", include_str!("../../../../cockpit/assets/dungeon/cards/firefly-sorbet.card")),
    ("widow-kite", include_str!("../../../../cockpit/assets/dungeon/cards/widow-kite.card")),
    ("cinder-seahorse", include_str!("../../../../cockpit/assets/dungeon/cards/cinder-seahorse.card")),
    ("pearl-diver-fork", include_str!("../../../../cockpit/assets/dungeon/cards/pearl-diver-fork.card")),
    ("scorpion-tap", include_str!("../../../../cockpit/assets/dungeon/cards/scorpion-tap.card")),
    ("mooncrab-cracker", include_str!("../../../../cockpit/assets/dungeon/cards/mooncrab-cracker.card")),
    ("undertow-bell", include_str!("../../../../cockpit/assets/dungeon/cards/undertow-bell.card")),
    ("phantom-cue", include_str!("../../../../cockpit/assets/dungeon/cards/phantom-cue.card")),
    ("harpoon-ratchet", include_str!("../../../../cockpit/assets/dungeon/cards/harpoon-ratchet.card")),
    ("railjaw-stapler", include_str!("../../../../cockpit/assets/dungeon/cards/railjaw-stapler.card")),
    ("copperhead-spool", include_str!("../../../../cockpit/assets/dungeon/cards/copperhead-spool.card")),
    ("orchard-hourglass", include_str!("../../../../cockpit/assets/dungeon/cards/orchard-hourglass.card")),
    ("eventide-fan", include_str!("../../../../cockpit/assets/dungeon/cards/eventide-fan.card")),
    ("kettle-eclipse", include_str!("../../../../cockpit/assets/dungeon/cards/kettle-eclipse.card")),
    ("corkscrew-comet", include_str!("../../../../cockpit/assets/dungeon/cards/corkscrew-comet.card")),
    ("copperhead-siphon", include_str!("../../../../cockpit/assets/dungeon/cards/copperhead-siphon.card")),
    ("last-orders-ladle", include_str!("../../../../cockpit/assets/dungeon/cards/last-orders-ladle.card")),
    ("coatcheck-comet", include_str!("../../../../cockpit/assets/dungeon/cards/coatcheck-comet.card")),
    ("last-call-carom", include_str!("../../../../cockpit/assets/dungeon/cards/last-call-carom.card")),
    ("velvet-intermission", include_str!("../../../../cockpit/assets/dungeon/cards/velvet-intermission.card")),
    ("masquerade-matchbox", include_str!("../../../../cockpit/assets/dungeon/cards/masquerade-matchbox.card")),
    ("confetti-antidote", include_str!("../../../../cockpit/assets/dungeon/cards/confetti-antidote.card")),
    ("velvet-rope-ticket", include_str!("../../../../cockpit/assets/dungeon/cards/velvet-rope-ticket.card")),
    ("permafrost-flywheel", include_str!("../../../../cockpit/assets/dungeon/cards/permafrost-flywheel.card")),
    ("thundercomb", include_str!("../../../../cockpit/assets/dungeon/cards/thundercomb.card")),
    ("queenstorm-decree", include_str!("../../../../cockpit/assets/dungeon/cards/queenstorm-decree.card")),
    ("bottled-squall-honey", include_str!("../../../../cockpit/assets/dungeon/cards/bottled-squall-honey.card")),
    ("zero-hour-bell", include_str!("../../../../cockpit/assets/dungeon/cards/zero-hour-bell.card")),
    ("umbra-switchfork", include_str!("../../../../cockpit/assets/dungeon/cards/umbra-switchfork.card")),
    ("last-platform-signal", include_str!("../../../../cockpit/assets/dungeon/cards/last-platform-signal.card")),
    ("eclipse-punchcard", include_str!("../../../../cockpit/assets/dungeon/cards/eclipse-punchcard.card")),
    ("keeltooth-spur", include_str!("../../../../cockpit/assets/dungeon/cards/keeltooth-spur.card")),
    ("drydock-salute", include_str!("../../../../cockpit/assets/dungeon/cards/drydock-salute.card")),
    ("widowglass-ampoule", include_str!("../../../../cockpit/assets/dungeon/cards/widowglass-ampoule.card")),
    ("mothwake-lantern", include_str!("../../../../cockpit/assets/dungeon/cards/mothwake-lantern.card")),
    ("orrery-heart", include_str!("../../../../cockpit/assets/dungeon/cards/orrery-heart.card")),
    ("eventide-key", include_str!("../../../../cockpit/assets/dungeon/cards/eventide-key.card")),
    ("stormrail-ticket", include_str!("../../../../cockpit/assets/dungeon/cards/stormrail-ticket.card")),
    ("thunderhead-gavel", include_str!("../../../../cockpit/assets/dungeon/cards/thunderhead-gavel.card")),
    ("rimewheel-bearing", include_str!("../../../../cockpit/assets/dungeon/cards/rimewheel-bearing.card")),
    ("whiteout-parasol", include_str!("../../../../cockpit/assets/dungeon/cards/whiteout-parasol.card")),
    ("redoubt-tuning-fork", include_str!("../../../../cockpit/assets/dungeon/cards/redoubt-tuning-fork.card")),
    ("velvet-rebuttal", include_str!("../../../../cockpit/assets/dungeon/cards/velvet-rebuttal.card")),
    ("plague-switchboard", include_str!("../../../../cockpit/assets/dungeon/cards/plague-switchboard.card")),
    ("quarantine-star", include_str!("../../../../cockpit/assets/dungeon/cards/quarantine-star.card")),
    ("cinderpin-escapement", include_str!("../../../../cockpit/assets/dungeon/cards/cinderpin-escapement.card")),
    ("furnace-curtain", include_str!("../../../../cockpit/assets/dungeon/cards/furnace-curtain.card")),
    ("widows-spindle", include_str!("../../../../cockpit/assets/dungeon/cards/widows-spindle.card")),
    ("last-call-thunder", include_str!("../../../../cockpit/assets/dungeon/cards/last-call-thunder.card")),
    ("sundial-mortar", include_str!("../../../../cockpit/assets/dungeon/cards/sundial-mortar.card")),
    ("adder-hourglass", include_str!("../../../../cockpit/assets/dungeon/cards/adder-hourglass.card")),
    (
        "gold",
        include_str!("../../../../cockpit/assets/dungeon/cards/gold.card"),
    ),
    (
        "potion",
        include_str!("../../../../cockpit/assets/dungeon/cards/potion.card"),
    ),
    (
        "bomb",
        include_str!("../../../../cockpit/assets/dungeon/cards/bomb.card"),
    ),
    (
        "mail",
        include_str!("../../../../cockpit/assets/dungeon/cards/mail.card"),
    ),
    (
        "heart",
        include_str!("../../../../cockpit/assets/dungeon/cards/heart.card"),
    ),
    ("iron-helm", include_str!("../../../../cockpit/assets/dungeon/cards/iron-helm.card")),
    ("body-armour", include_str!("../../../../cockpit/assets/dungeon/cards/body-armour.card")),
    (
        "bow",
        include_str!("../../../../cockpit/assets/dungeon/cards/bow.card"),
    ),
    (
        "crossbow",
        include_str!("../../../../cockpit/assets/dungeon/cards/crossbow.card"),
    ),
    ("thrown-stones", include_str!("../../../../cockpit/assets/dungeon/cards/thrown-stones.card")),
    ("the-ball", include_str!("../../../../cockpit/assets/dungeon/cards/the-ball.card")),
    ("note-stream", include_str!("../../../../cockpit/assets/dungeon/cards/note-stream.card")),
    (
        "handgonne",
        include_str!("../../../../cockpit/assets/dungeon/cards/handgonne.card"),
    ),
    (
        "ember-quiver",
        include_str!("../../../../cockpit/assets/dungeon/cards/ember-quiver.card"),
    ),
    (
        "cabbage-storm",
        include_str!("../../../../cockpit/assets/dungeon/cards/cabbage-storm.card"),
    ),
    (
        "the-imposture",
        include_str!("../../../../cockpit/assets/dungeon/cards/the-imposture.card"),
    ),
    (
        "hedge-maze",
        include_str!("../../../../cockpit/assets/dungeon/cards/hedge-maze.card"),
    ),
    (
        "twin-string",
        include_str!("../../../../cockpit/assets/dungeon/cards/twin-string.card"),
    ),
    (
        "awakened-fork",
        include_str!("../../../../cockpit/assets/dungeon/cards/awakened-fork.card"),
    ),
    (
        "fork",
        include_str!("../../../../cockpit/assets/dungeon/cards/fork.card"),
    ),
    (
        "fart-bombs",
        include_str!("../../../../cockpit/assets/dungeon/cards/fart-bombs.card"),
    ),
    (
        "bodkin-heads",
        include_str!("../../../../cockpit/assets/dungeon/cards/bodkin-heads.card"),
    ),
    (
        "swift-spurs",
        include_str!("../../../../cockpit/assets/dungeon/cards/swift-spurs.card"),
    ),
    (
        "quick-draw",
        include_str!("../../../../cockpit/assets/dungeon/cards/quick-draw.card"),
    ),
    (
        "leech-fang",
        include_str!("../../../../cockpit/assets/dungeon/cards/leech-fang.card"),
    ),
    (
        "holy-water",
        include_str!("../../../../cockpit/assets/dungeon/cards/holy-water.card"),
    ),
    (
        "thunder-scroll",
        include_str!("../../../../cockpit/assets/dungeon/cards/thunder-scroll.card"),
    ),
    (
        "knights-blade",
        include_str!("../../../../cockpit/assets/dungeon/cards/knights-blade.card"),
    ),
    (
        "bone",
        include_str!("../../../../cockpit/assets/dungeon/cards/bone.card"),
    ),
    (
        "wax",
        include_str!("../../../../cockpit/assets/dungeon/cards/wax.card"),
    ),
    (
        "ore",
        include_str!("../../../../cockpit/assets/dungeon/cards/ore.card"),
    ),
    (
        "gem",
        include_str!("../../../../cockpit/assets/dungeon/cards/gem.card"),
    ),
    (
        "ember",
        include_str!("../../../../cockpit/assets/dungeon/cards/ember.card"),
    ),
    (
        "scale",
        include_str!("../../../../cockpit/assets/dungeon/cards/scale.card"),
    ),
    (
        "dodge-roll",
        include_str!("../../../../cockpit/assets/dungeon/cards/dodge-roll.card"),
    ),
    (
        "twin-axes",
        include_str!("../../../../cockpit/assets/dungeon/cards/twin-axes.card"),
    ),
    (
        "axe-spin",
        include_str!("../../../../cockpit/assets/dungeon/cards/axe-spin.card"),
    ),
    (
        "kite-shield",
        include_str!("../../../../cockpit/assets/dungeon/cards/kite-shield.card"),
    ),
    (
        "fae-dagger",
        include_str!("../../../../cockpit/assets/dungeon/cards/fae-dagger.card"),
    ),
    (
        "pendragon-sceptre",
        include_str!("../../../../cockpit/assets/dungeon/cards/pendragon-sceptre.card"),
    ),
    (
        "censer",
        include_str!("../../../../cockpit/assets/dungeon/cards/censer.card"),
    ),
    (
        "hearth-bell",
        include_str!("../../../../cockpit/assets/dungeon/cards/hearth-bell.card"),
    ),
    (
        "rime-whistle",
        include_str!("../../../../cockpit/assets/dungeon/cards/rime-whistle.card"),
    ),
    (
        "salt-thread",
        include_str!("../../../../cockpit/assets/dungeon/cards/salt-thread.card"),
    ),
    (
        "glass-needle",
        include_str!("../../../../cockpit/assets/dungeon/cards/glass-needle.card"),
    ),
    (
        "cinder-wick",
        include_str!("../../../../cockpit/assets/dungeon/cards/cinder-wick.card"),
    ),
    (
        "lantern-mote",
        include_str!("../../../../cockpit/assets/dungeon/cards/lantern-mote.card"),
    ),
    (
        "reed-flute",
        include_str!("../../../../cockpit/assets/dungeon/cards/reed-flute.card"),
    ),
    (
        "choir-crumb",
        include_str!("../../../../cockpit/assets/dungeon/cards/choir-crumb.card"),
    ),
    (
        "marrow-sip",
        include_str!("../../../../cockpit/assets/dungeon/cards/marrow-sip.card"),
    ),
    (
        "chapel-cincture",
        include_str!("../../../../cockpit/assets/dungeon/cards/chapel-cincture.card"),
    ),
    (
        "college-orb",
        include_str!("../../../../cockpit/assets/dungeon/cards/college-orb.card"),
    ),
    (
        "crescent-quiver",
        include_str!("../../../../cockpit/assets/dungeon/cards/crescent-quiver.card"),
    ),
    (
        "deeps-harness",
        include_str!("../../../../cockpit/assets/dungeon/cards/deeps-harness.card"),
    ),
    (
        "forge-matchcord",
        include_str!("../../../../cockpit/assets/dungeon/cards/forge-matchcord.card"),
    ),
    (
        "gate-aegis",
        include_str!("../../../../cockpit/assets/dungeon/cards/gate-aegis.card"),
    ),
    (
        "grave-axes",
        include_str!("../../../../cockpit/assets/dungeon/cards/grave-axes.card"),
    ),
    (
        "oathshield",
        include_str!("../../../../cockpit/assets/dungeon/cards/oathshield.card"),
    ),
    (
        "rookery-longbow",
        include_str!("../../../../cockpit/assets/dungeon/cards/rookery-longbow.card"),
    ),
    (
        "round-table-blade",
        include_str!("../../../../cockpit/assets/dungeon/cards/round-table-blade.card"),
    ),
    (
        "stock-blade",
        include_str!("../../../../cockpit/assets/dungeon/cards/stock-blade.card"),
    ),
    (
        "stock-staff",
        include_str!("../../../../cockpit/assets/dungeon/cards/stock-staff.card"),
    ),
    (
        "spark-staff",
        include_str!("../../../../cockpit/assets/dungeon/cards/spark-staff.card"),
    ),
    (
        "stock-maul",
        include_str!("../../../../cockpit/assets/dungeon/cards/stock-maul.card"),
    ),
    (
        "sigil-staff",
        include_str!("../../../../cockpit/assets/dungeon/cards/sigil-staff.card"),
    ),
    (
        "smithys-dragonmouth",
        include_str!("../../../../cockpit/assets/dungeon/cards/smithys-dragonmouth.card"),
    ),
    (
        "table-gauntlet",
        include_str!("../../../../cockpit/assets/dungeon/cards/table-gauntlet.card"),
    ),
    (
        "vestal-censer",
        include_str!("../../../../cockpit/assets/dungeon/cards/vestal-censer.card"),
    ),
    // The champions at the bar: their arms and their spells (`champions`).
    ("far-sightline", include_str!("../../../../cockpit/assets/dungeon/cards/far-sightline.card")),
    ("far-brass-rain", include_str!("../../../../cockpit/assets/dungeon/cards/far-brass-rain.card")),
    ("far-steady", include_str!("../../../../cockpit/assets/dungeon/cards/far-steady.card")),
    ("gale-power", include_str!("../../../../cockpit/assets/dungeon/cards/gale-power.card")),
    ("gale-shackle", include_str!("../../../../cockpit/assets/dungeon/cards/gale-shackle.card")),
    ("gale-windrun", include_str!("../../../../cockpit/assets/dungeon/cards/gale-windrun.card")),
    ("ember-wave", include_str!("../../../../cockpit/assets/dungeon/cards/ember-wave.card")),
    ("ember-pillar", include_str!("../../../../cockpit/assets/dungeon/cards/ember-pillar.card")),
    ("ember-kindle", include_str!("../../../../cockpit/assets/dungeon/cards/ember-kindle.card")),
    ("horizon-rifle", include_str!("../../../../cockpit/assets/dungeon/cards/horizon-rifle.card")),
    ("reedwake-bow", include_str!("../../../../cockpit/assets/dungeon/cards/reedwake-bow.card")),
    ("kilnheart-staff", include_str!("../../../../cockpit/assets/dungeon/cards/kilnheart-staff.card")),
    ("great-maul", include_str!("../../../../cockpit/assets/dungeon/cards/great-maul.card")),
    ("storm-hammer", include_str!("../../../../cockpit/assets/dungeon/cards/storm-hammer.card")),
    ("groundslam", include_str!("../../../../cockpit/assets/dungeon/cards/groundslam.card")),
    ("brute-force", include_str!("../../../../cockpit/assets/dungeon/cards/brute-force.card")),
    ("cleaver", include_str!("../../../../cockpit/assets/dungeon/cards/cleaver.card")),
    ("meat-hook", include_str!("../../../../cockpit/assets/dungeon/cards/meat-hook.card")),
    ("rot", include_str!("../../../../cockpit/assets/dungeon/cards/rot.card")),
    ("reaver-axes", include_str!("../../../../cockpit/assets/dungeon/cards/reaver-axes.card")),
    ("hurl-axe", include_str!("../../../../cockpit/assets/dungeon/cards/hurl-axe.card")),
    ("leap", include_str!("../../../../cockpit/assets/dungeon/cards/leap.card")),
    ("bloodlust", include_str!("../../../../cockpit/assets/dungeon/cards/bloodlust.card")),
    ("war-lance", include_str!("../../../../cockpit/assets/dungeon/cards/war-lance.card")),
    ("javelin", include_str!("../../../../cockpit/assets/dungeon/cards/javelin.card")),
    ("vault", include_str!("../../../../cockpit/assets/dungeon/cards/vault.card")),
    ("brace", include_str!("../../../../cockpit/assets/dungeon/cards/brace.card")),
    ("chain-haul", include_str!("../../../../cockpit/assets/dungeon/cards/chain-haul.card")),
    ("flask-bandolier", include_str!("../../../../cockpit/assets/dungeon/cards/flask-bandolier.card")),
    ("miasma-flask", include_str!("../../../../cockpit/assets/dungeon/cards/miasma-flask.card")),
    ("quickfire", include_str!("../../../../cockpit/assets/dungeon/cards/quickfire.card")),
    ("elixir", include_str!("../../../../cockpit/assets/dungeon/cards/elixir.card")),
    // The Gravecaller's staff and his four spells (`minions`).
    (
        "grave-staff",
        include_str!("../../../../cockpit/assets/dungeon/cards/grave-staff.card"),
    ),
    (
        "raise-ghoul",
        include_str!("../../../../cockpit/assets/dungeon/cards/raise-ghoul.card"),
    ),
    (
        "barrow-warden",
        include_str!("../../../../cockpit/assets/dungeon/cards/barrow-warden.card"),
    ),
    (
        "bone-totem",
        include_str!("../../../../cockpit/assets/dungeon/cards/bone-totem.card"),
    ),
    (
        "soul-rip",
        include_str!("../../../../cockpit/assets/dungeon/cards/soul-rip.card"),
    ),
    ("eclipse-toll", include_str!("../../../../cockpit/assets/dungeon/cards/eclipse-toll.card")),
    ("mercy-lantern", include_str!("../../../../cockpit/assets/dungeon/cards/mercy-lantern.card")),
    ("cinder-tithe", include_str!("../../../../cockpit/assets/dungeon/cards/cinder-tithe.card")),
    ("last-light", include_str!("../../../../cockpit/assets/dungeon/cards/last-light.card")),
    ("noon-shatter", include_str!("../../../../cockpit/assets/dungeon/cards/noon-shatter.card")),
    ("borrowed-minute", include_str!("../../../../cockpit/assets/dungeon/cards/borrowed-minute.card")),
    ("dialkeepers-ring", include_str!("../../../../cockpit/assets/dungeon/cards/dialkeepers-ring.card")),
    ("storm-receipt", include_str!("../../../../cockpit/assets/dungeon/cards/storm-receipt.card")),
    ("eye-of-the-squall", include_str!("../../../../cockpit/assets/dungeon/cards/eye-of-the-squall.card")),
    ("fulgurite-spurs", include_str!("../../../../cockpit/assets/dungeon/cards/fulgurite-spurs.card")),
    ("keelbreaker", include_str!("../../../../cockpit/assets/dungeon/cards/keelbreaker.card")),
    ("pressure-seal", include_str!("../../../../cockpit/assets/dungeon/cards/pressure-seal.card")),
    ("undertow-coil", include_str!("../../../../cockpit/assets/dungeon/cards/undertow-coil.card")),
    ("comet-glass", include_str!("../../../../cockpit/assets/dungeon/cards/comet-glass.card")),
    ("bellkeepers-mail", include_str!("../../../../cockpit/assets/dungeon/cards/bellkeepers-mail.card")),
    ("wayfarer-wick", include_str!("../../../../cockpit/assets/dungeon/cards/wayfarer-wick.card")),
    ("lastlight-postmark", include_str!("../../../../cockpit/assets/dungeon/cards/lastlight-postmark.card")),
    ("deadletter-flare", include_str!("../../../../cockpit/assets/dungeon/cards/deadletter-flare.card")),
    ("pallbearer-lanterns", include_str!("../../../../cockpit/assets/dungeon/cards/pallbearer-lanterns.card")),
    ("foxstep-spindle", include_str!("../../../../cockpit/assets/dungeon/cards/foxstep-spindle.card")),
    ("glassway-key", include_str!("../../../../cockpit/assets/dungeon/cards/glassway-key.card")),
    ("switchglass-escapement", include_str!("../../../../cockpit/assets/dungeon/cards/switchglass-escapement.card")),
    ("starfall-writ", include_str!("../../../../cockpit/assets/dungeon/cards/starfall-writ.card")),
    ("needlewake-sight", include_str!("../../../../cockpit/assets/dungeon/cards/needlewake-sight.card")),
    ("bankshot-medallion", include_str!("../../../../cockpit/assets/dungeon/cards/bankshot-medallion.card")),
    ("vipers-hourglass", include_str!("../../../../cockpit/assets/dungeon/cards/vipers-hourglass.card")),
    ("stormrail-conductor", include_str!("../../../../cockpit/assets/dungeon/cards/stormrail-conductor.card")),
    ("winterfang-bayonet", include_str!("../../../../cockpit/assets/dungeon/cards/winterfang-bayonet.card")),
    ("choirwire-coil", include_str!("../../../../cockpit/assets/dungeon/cards/choirwire-coil.card")),
    ("mothglass-lens", include_str!("../../../../cockpit/assets/dungeon/cards/mothglass-lens.card")),
    ("whiteout-muzzle", include_str!("../../../../cockpit/assets/dungeon/cards/whiteout-muzzle.card")),
    ("copperhead-volley", include_str!("../../../../cockpit/assets/dungeon/cards/copperhead-volley.card")),
    ("mercy-trigger", include_str!("../../../../cockpit/assets/dungeon/cards/mercy-trigger.card")),
    ("bastion-bell", include_str!("../../../../cockpit/assets/dungeon/cards/bastion-bell.card")),
    ("orrery-collapse", include_str!("../../../../cockpit/assets/dungeon/cards/orrery-collapse.card")),
    ("needle-cathedra", include_str!("../../../../cockpit/assets/dungeon/cards/needle-cathedra.card")),
    ("stormglass-writ", include_str!("../../../../cockpit/assets/dungeon/cards/stormglass-writ.card")),
    ("hearthglass-pear", include_str!("../../../../cockpit/assets/dungeon/cards/hearthglass-pear.card")),
    ("wake-lantern", include_str!("../../../../cockpit/assets/dungeon/cards/wake-lantern.card")),
    ("undertow-benediction", include_str!("../../../../cockpit/assets/dungeon/cards/undertow-benediction.card")),
    ("fulgurite-fork", include_str!("../../../../cockpit/assets/dungeon/cards/fulgurite-fork.card")),
    ("arcwell-salute", include_str!("../../../../cockpit/assets/dungeon/cards/arcwell-salute.card")),
    ("rimewheel-escapement", include_str!("../../../../cockpit/assets/dungeon/cards/rimewheel-escapement.card")),
    ("kilnwing-reprieve", include_str!("../../../../cockpit/assets/dungeon/cards/kilnwing-reprieve.card")),
    ("vesper-knell", include_str!("../../../../cockpit/assets/dungeon/cards/vesper-knell.card")),
    ("moonmilk-flask", include_str!("../../../../cockpit/assets/dungeon/cards/moonmilk-flask.card")),
    ("votive-aegis", include_str!("../../../../cockpit/assets/dungeon/cards/votive-aegis.card")),
    ("brineheart-fig", include_str!("../../../../cockpit/assets/dungeon/cards/brineheart-fig.card")),
    ("firefly-prism", include_str!("../../../../cockpit/assets/dungeon/cards/firefly-prism.card")),
    ("arcneedle-spindle", include_str!("../../../../cockpit/assets/dungeon/cards/arcneedle-spindle.card")),
    ("vaultkey-spur", include_str!("../../../../cockpit/assets/dungeon/cards/vaultkey-spur.card")),
    ("wayfarer-stitch", include_str!("../../../../cockpit/assets/dungeon/cards/wayfarer-stitch.card")),
    ("undertow-crown", include_str!("../../../../cockpit/assets/dungeon/cards/undertow-crown.card")),
    ("cinder-parasol", include_str!("../../../../cockpit/assets/dungeon/cards/cinder-parasol.card")),
    ("mercy-procession", include_str!("../../../../cockpit/assets/dungeon/cards/mercy-procession.card")),
    ("plague-divers-rebreather", include_str!("../../../../cockpit/assets/dungeon/cards/plague-divers-rebreather.card")),
    ("redshift-ripcord", include_str!("../../../../cockpit/assets/dungeon/cards/redshift-ripcord.card")),

    ("wishbone-reticle", include_str!("../../../../cockpit/assets/dungeon/cards/wishbone-reticle.card")),
    ("halo-breach", include_str!("../../../../cockpit/assets/dungeon/cards/halo-breach.card")),
    ("wake-engine", include_str!("../../../../cockpit/assets/dungeon/cards/wake-engine.card")),
    ("rook-mortar", include_str!("../../../../cockpit/assets/dungeon/cards/rook-mortar.card")),
    ("saints-tourniquet", include_str!("../../../../cockpit/assets/dungeon/cards/saints-tourniquet.card")),
    ("thunderhead-phial", include_str!("../../../../cockpit/assets/dungeon/cards/thunderhead-phial.card")),
    ("blacktide-censer", include_str!("../../../../cockpit/assets/dungeon/cards/blacktide-censer.card")),
    ("duelists-mainspring", include_str!("../../../../cockpit/assets/dungeon/cards/duelists-mainspring.card")),
    ("stormglass-vespers", include_str!("../../../../cockpit/assets/dungeon/cards/stormglass-vespers.card")),
    ("orchard-of-ashes", include_str!("../../../../cockpit/assets/dungeon/cards/orchard-of-ashes.card")),
    ("gallows-bell", include_str!("../../../../cockpit/assets/dungeon/cards/gallows-bell.card")),
    ("moth-sanctuary", include_str!("../../../../cockpit/assets/dungeon/cards/moth-sanctuary.card")),
    ("crown-of-cinders", include_str!("../../../../cockpit/assets/dungeon/cards/crown-of-cinders.card")),
    ("frostbite-ampoule", include_str!("../../../../cockpit/assets/dungeon/cards/frostbite-ampoule.card")),
    ("railglass-prism", include_str!("../../../../cockpit/assets/dungeon/cards/railglass-prism.card")),
    ("gravewire-spool", include_str!("../../../../cockpit/assets/dungeon/cards/gravewire-spool.card")),
    ("sunken-mint", include_str!("../../../../cockpit/assets/dungeon/cards/sunken-mint.card")),
    ("emberwake-charge", include_str!("../../../../cockpit/assets/dungeon/cards/emberwake-charge.card")),
    ("choir-grenade", include_str!("../../../../cockpit/assets/dungeon/cards/choir-grenade.card")),
    ("phoenix-suture", include_str!("../../../../cockpit/assets/dungeon/cards/phoenix-suture.card")),
    ("abyssal-compass", include_str!("../../../../cockpit/assets/dungeon/cards/abyssal-compass.card")),
    ("powder-saint", include_str!("../../../../cockpit/assets/dungeon/cards/powder-saint.card")),
    ("rustwing-locket", include_str!("../../../../cockpit/assets/dungeon/cards/rustwing-locket.card")),
    ("meteor-cordial", include_str!("../../../../cockpit/assets/dungeon/cards/meteor-cordial.card")),
    ("thorn-testament", include_str!("../../../../cockpit/assets/dungeon/cards/thorn-testament.card")),
    ("glasshouse-mercy", include_str!("../../../../cockpit/assets/dungeon/cards/glasshouse-mercy.card")),
    ("widowseed-cage", include_str!("../../../../cockpit/assets/dungeon/cards/widowseed-cage.card")),
    ("storm-tab", include_str!("../../../../cockpit/assets/dungeon/cards/storm-tab.card")),
    ("velvet-rope", include_str!("../../../../cockpit/assets/dungeon/cards/velvet-rope.card")),
    ("ricochet-decanter", include_str!("../../../../cockpit/assets/dungeon/cards/ricochet-decanter.card")),
];

impl Book {
    pub fn builtin() -> Book {
        let mut book = Book {
            // The compiled catalog's size is known: avoid repeated vector growth
            // as the expanding world-loot catalog is loaded.
            cards: Vec::with_capacity(BUILTIN.len()),
            revision: 0,
        };
        for (id, raw) in BUILTIN {
            let checked = check(id, raw).expect("built-in cards pass their own checker");
            book.insert(checked.card);
        }
        book
    }

    pub fn get(&self, id: &str) -> Option<&Card> {
        self.cards
            .binary_search_by(|c| c.id.as_str().cmp(id))
            .ok()
            .map(|i| &self.cards[i])
    }

    /// Add or replace a card; false when the book is full.
    pub fn insert(&mut self, card: Card) -> bool {
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
        // Two passes avoid a temporary weighted table on every loot roll.
        // Keep catalog order and exactly one RNG draw for nonempty tables so
        // existing seeded raids replay identically. `keep` is a pure weight
        // lookup at each call site.
        // A damaged save can carry any weight (`drop` up to u32::MAX, the
        // restore fuzz); the total saturates so the roll cannot overflow.
        let total: u32 = self.cards.iter().map(&keep).fold(0u32, u32::saturating_add);
        if total == 0 {
            return None;
        }
        let mut roll = rng.below(total as usize) as u32;
        for card in &self.cards {
            let weight = keep(card);
            if roll < weight {
                return Some(card.id.clone());
            }
            roll -= weight;
        }
        None
    }

    /// A roll in which the spells, together, weigh `share` against the
    /// rest's own weights, however many spells the book holds; each spell
    /// keeps its own weight within the share.
    fn roll_sharing(&self, rng: &mut Rng, share: u32, keep: impl Fn(&Card) -> u32) -> Option<String> {
        let spells: u32 = self.cards.iter().filter(|c| c.kind == Kind::Spell).map(&keep).fold(0u32, u32::saturating_add);
        if spells == 0 {
            return self.roll(rng, keep);
        }
        self.roll(rng, |c| if c.kind == Kind::Spell { keep(c).saturating_mul(share) } else { keep(c).saturating_mul(spells) })
    }

    /// An ordinary drop: any card but the delve's materials.
    pub fn roll_drop(&self, rng: &mut Rng, pack: Pack) -> Option<String> {
        self.roll_sharing(rng, SPELL_DROP_SHARE, |c| {
            if c.is_spoil() || !c.drops_in(pack) {
                0
            } else {
                c.drop
            }
        })
    }

    /// The delve's own material, on its separate roll.
    pub fn roll_spoil(&self, rng: &mut Rng, pack: Pack) -> Option<String> {
        self.roll(rng, |c| {
            if c.is_spoil() && c.drops_in(pack) {
                c.drop
            } else {
                0
            }
        })
    }

    /// A chest holds one prize plus two ordinary drops.
    pub fn roll_chest(&self, rng: &mut Rng, pack: Pack) -> Vec<String> {
        let prize = self.roll_sharing(rng, SPELL_CHEST_SHARE, |c| if c.drops_in(pack) { c.chest } else { 0 });
        let rest = [self.roll_drop(rng, pack), self.roll_drop(rng, pack)];
        prize
            .into_iter()
            .chain(rest.into_iter().flatten())
            .collect()
    }
}

/// Read one card file's text (its pictures already cut into rows) into
/// `book`. The line the reload reports for it.
pub fn read_into(book: &mut Book, stem: &str, raw: &str) -> String {
    match check(stem, raw) {
        Ok(checked) => {
            let name = checked.card.name.clone();
            if book.insert(checked.card) {
                if checked.notes.is_empty() {
                    format!("{stem}: {name}")
                } else {
                    format!("{stem}: {name} ({})", checked.notes.join("; "))
                }
            } else {
                format!("{stem}: the book is full ({MAX_BOOK} cards)")
            }
        }
        Err(rejected) => format!("{stem}: {}", rejected.errors.join("; ")),
    }
}

/// The card format, for a friend or their AI.
/// The effects a held card (and so a known wish's tier) may carry: word,
/// range and meaning, a line each. The top of each range is the most a
/// wish can give (`wish_most`): past it a tier is turned away.
pub fn hold_effects() -> String {
    SPECS
        .iter()
        .filter(|s| s.hold)
        .map(|s| {
            let high = if s.cap > 0 { s.high.min(s.cap) } else { s.high };
            let high = wish_most(s.word).map_or(high, |most| high.min(most));
            format!("{} {}-{high}: {}", s.word, s.low, s.says)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn rules() -> String {
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
                    _ if matches!((s.make)(s.low), Effect::Ghoul(_) | Effect::Warden(_) | Effect::Totem(_)) => "spell/play",
                    _ if matches!((s.make)(s.low), Effect::Rip(_)) => "spell",
                    _ if (s.make)(s.low).summon().is_some() => "play",
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
kind   take | hold | play | arm | guard | spell   (take: spent on pickup · hold: lasts the run · play: hand, Z quaffs the first · spell: reusable slot, keys 1–4 · arm: your weapon · guard: on space, `guard roll`, `guard shield`, or `guard wall width=2-8 drain=10-50 empower=0-40` — a held barrier that stops monsters' shots, lets friends' shots through (+empower%), and drains mana per second)\n\
rarity common | rare | relic\n\
text   <up to {MAX_TEXT} chars of flavour>\n\
by     <your name>\n\
drop   <0–100 weight in monster drops>   chest <0–100 weight as a chest prize>\n\
where  crypt | mines | keep | anywhere   (which delve it drops in)\n\
<effect> <number>   at most {MAX_EFFECTS}:\n{effects}\n\
Spell cards: `cooldown <1–120 seconds>` is required; use heal, bombs, nova, ward or `cast bolt|ring|meteor`. Shapes carry your shot traits.\n\
Arm cards: `arm bow|crossbow|handgonne|notes`, or Forge rune lines (bolt/spread/melee/look; see /dungeon forge).\n\
Summons (spell or play cards): `summon N swarm|guard|turret` — {summon}.\n\
Looks on the field: `draw <slot>` then rows of inks, 1–{frames} frames side by side with `|` between (a worn piece drawn small may add `at=<place>`: {places}):\n{slots}\n\
`sheet <slot> <file.png> [frames=N] [size=N] [at=<place>]` names a picture in .angel/dungeon/sheets/ (a strip of up to {frames} frames on a flat background, at most 2 MiB) that is cut into those rows when the card loads. `paint <slot> <file.png> [frames=N] [size=N] [at=<place>] [move=<words>]` is the same for a painting (a PNG on a transparent background, at most 4 MiB): cut into rows too, and kept whole, so the top level of a wish shows the picture itself (`draw <slot> paint=<sha256>`); `normal=<file_n.png>` adds its normal map (its size, green up), lit pixel by pixel in a lit room (`normal=<sha256>`); `pixel` says it is pixel art, drawn with hard pixel edges; rows of inks right under a paint line are its inks in place of the cut. `move=` sets a look moving: {moves}, comma-separated. Hold and spell cards may also say `look <colour> <shape>` for their knight's shots.\n\
art    then up to {ART} rows of up to {ART} inks; `.` is black paper. Inks:\n\
  greys  k K Z X g j G J h i H   greens f D F E e l N m L A M y C Y\n\
  wood   n b I B p P r R o O t T   stone  s x S u U v V W q Q z\n\
  violet d ( ) [ ] ^   pink   ! ? ; % +\n\
  signal 0 1 2 3 w (blues) a 4 @ 5 6 $ c 9 (golds) 8 7 (reds) — signal inks glow; use them for what matters\n\
New spells in .angel/dungeon/cards hot-load into the host's first empty slot with a flourish; full slots drop them at the host's feet. /dungeon cards reload reads the folder again; /dungeon card <file> adds one and drops it at your feet.",
        summon = SUMMON_SPEC.says,
        frames = super::looks::MAX_FRAMES,
        places = super::looks::PLACES,
        slots = super::looks::slots(),
        moves = super::looks::Move::words(),
    )
}

#[cfg(test)]
#[path = "../../tests/rules/together_cards__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-home"))]
#[path = "../../tests/rules/cards_bend_cases__tests.rs"]
mod cards_bend_cases_tests;
