//! Floor bosses: a `.boss` file per guardian, written by people or models.
//!
//! A boss stands in a floor's stairs room; the way down stays barred until it
//! falls. Like cards, a boss is checked data — a movement, up to three attack
//! patterns from a fixed vocabulary, a rage threshold, spoils it drops and its
//! own pixel art — with every number clamped, so a boss can be fierce but
//! never unfair or unbounded.
//!
//! An attack can also name any of the monsters' patterns (`patterns`), and
//! a `phases` line chains two to four of them, a few volleys each, round
//! and round. A name the vocabulary lacks is rejected.
//!
//! The fight builds by stages of its health (`Stage`): each line joins at
//! its `from=`, and each stage has fixed caps on shots a second and shots
//! in the room per knight, held every tick (`signatures::hold`). From the
//! middle on, a guardian's `windup` lines are its signature moments:
//! charge-ups wound up where everyone can see them (`signatures::wind`).

use super::Pack;
use super::Shot;
use super::patterns::Emitter;
use crate::realm::{Spoil, Spoils};
use serde::{Deserialize, Serialize};

pub const MAX_BYTES: usize = 8192;
pub const ART: usize = 32;

/// Appearance only: shared by a guardian's bolts, split children and hits.
/// The shot's kind, path and collision core keep their original meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Art {
    Cinderjaw,
    Index,
    Mother,
    Waxen,
    Choir,
    Marshal,
    TaxKeeper,
    RelicLiege,
    IronTusk,
    RedPick,
    Foreman,
    Leviathan,
    Bellkeeper,
    Rootwarden,
    SporeCrown,
    Grinwick,
    AshCastellan,
    ChainKeeper,
    FreeCaptain,
    FuelReeve,
    SlagSovereign,
    SalvageClaimant,
    ShoggothHeart,
    BigRipper,
    MemoryWarden,
    SilenceWarden,
}

impl Art {
    pub const ALL: [Self; 26] = [Self::Cinderjaw, Self::Index, Self::Mother, Self::Waxen, Self::Choir, Self::Marshal, Self::TaxKeeper, Self::RelicLiege, Self::IronTusk, Self::RedPick, Self::Foreman, Self::Leviathan, Self::Bellkeeper, Self::Rootwarden, Self::SporeCrown, Self::Grinwick, Self::AshCastellan, Self::ChainKeeper, Self::FreeCaptain, Self::FuelReeve, Self::SlagSovereign, Self::SalvageClaimant, Self::ShoggothHeart, Self::BigRipper, Self::MemoryWarden, Self::SilenceWarden];

    pub fn for_id(id: &str) -> Option<Self> {
        match id {
            "cinderjaw" => Some(Self::Cinderjaw),
            "the-index" => Some(Self::Index),
            "mother-of-spores" => Some(Self::Mother),
            "waxen-warden" => Some(Self::Waxen),
            "the-bone-choir" => Some(Self::Choir),
            "crypt-tithe-marshal" => Some(Self::Marshal),
            "crypt-tax-keeper" => Some(Self::TaxKeeper),
            "crypt-relic-liege" => Some(Self::RelicLiege),
            "iron-tusk-chief" => Some(Self::IronTusk),
            "red-pick-chief" => Some(Self::RedPick),
            "the-foreman" => Some(Self::Foreman),
            "late-fee-leviathan" => Some(Self::Leviathan),
            "archive-bellkeeper" => Some(Self::Bellkeeper),
            "fungal-rootwarden" => Some(Self::Rootwarden),
            "fungal-spore-crown" => Some(Self::SporeCrown),
            "grinwick" => Some(Self::Grinwick),
            "hellforge-ash-castellan" => Some(Self::AshCastellan),
            "hellforge-chain-keeper" => Some(Self::ChainKeeper),
            "hellforge-free-captain" => Some(Self::FreeCaptain),
            "hellforge-fuel-reeve" => Some(Self::FuelReeve),
            "hellforge-slag-sovereign" => Some(Self::SlagSovereign),
            "salvage-claimant" => Some(Self::SalvageClaimant),
            "shoggoth-heart" => Some(Self::ShoggothHeart),
            "the-big-ripper" => Some(Self::BigRipper),
            "unknown-memory-warden" => Some(Self::MemoryWarden),
            "unknown-silence-warden" => Some(Self::SilenceWarden),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Cinderjaw => "cinderjaw",
            Self::Index => "the-index",
            Self::Mother => "mother-of-spores",
            Self::Waxen => "waxen-warden",
            Self::Choir => "the-bone-choir",
            Self::Marshal => "crypt-tithe-marshal",
            Self::TaxKeeper => "crypt-tax-keeper",
            Self::RelicLiege => "crypt-relic-liege",
            Self::IronTusk => "iron-tusk-chief",
            Self::RedPick => "red-pick-chief",
            Self::Foreman => "the-foreman",
            Self::Leviathan => "late-fee-leviathan",
            Self::Bellkeeper => "archive-bellkeeper",
            Self::Rootwarden => "fungal-rootwarden",
            Self::SporeCrown => "fungal-spore-crown",
            Self::Grinwick => "grinwick",
            Self::AshCastellan => "hellforge-ash-castellan",
            Self::ChainKeeper => "hellforge-chain-keeper",
            Self::FreeCaptain => "hellforge-free-captain",
            Self::FuelReeve => "hellforge-fuel-reeve",
            Self::SlagSovereign => "hellforge-slag-sovereign",
            Self::SalvageClaimant => "salvage-claimant",
            Self::ShoggothHeart => "shoggoth-heart",
            Self::BigRipper => "the-big-ripper",
            Self::MemoryWarden => "unknown-memory-warden",
            Self::SilenceWarden => "unknown-silence-warden",

        }
    }

    pub fn of_shot(shot: &super::Projectile) -> Option<Self> {
        let (family, id) = shot.look.as_ref()?;
        (family == "boss-art").then(|| Self::for_id(id)).flatten()
    }

    pub fn look(self) -> Option<(String, String)> {
        Some(("boss-art".into(), self.id().into()))
    }
}

/// A guardian leaves its opening below this share of its health.
pub const MIDDLE_FROM: f32 = 0.7;
/// A guardian whose file names no rage fights its last below this share.
pub const LAST_FROM: f32 = 0.35;
/// Hostile shots a second a guardian may loose in its opening: a few, read one by one.
pub const OPEN_PER_SECOND: u32 = 3;
/// Hostile shots a second in the middle of its fight.
pub const MIDDLE_PER_SECOND: u32 = 5;
/// Hostile shots a second in its rage, the most it ever looses.
pub const RAGE_PER_SECOND: u32 = 7;
/// The most hostile shots in the room per knight standing, in its opening.
pub const OPEN_ON_SCREEN: u32 = 8;
/// The most hostile shots in the room per knight standing, in the middle.
pub const MIDDLE_ON_SCREEN: u32 = 12;
/// The most hostile shots in the room per knight standing, in its rage.
pub const RAGE_ON_SCREEN: u32 = 16;
/// The most shots one volley holds; a mine or a split counts the ring it bursts into.
pub const VOLLEY_MOST: u32 = 10;
/// The fewest ticks a charge-up winds up before it can harm: 0.8 s.
pub const WIND_LEAST: u32 = 24;
/// The most charge-up lines a guardian has.
pub const WINDUPS_MOST: usize = 2;

/// How far a guardian's fight has come, by its health: its opening, the
/// middle, and its rage (below its `rage`, or `LAST_FROM` if it names none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    #[default]
    Open,
    Middle,
    Rage,
}

impl Stage {
    pub const ALL: [Stage; 3] = [Stage::Open, Stage::Middle, Stage::Rage];

    fn named(word: &str) -> Option<Stage> {
        Some(match word {
            "open" | "opening" => Stage::Open,
            "middle" => Stage::Middle,
            "rage" => Stage::Rage,
            _ => return None,
        })
    }

    /// Hostile shots a second it may loose, and the most it may have in
    /// the room per knight standing.
    pub fn caps(self) -> (u32, u32) {
        match self {
            Stage::Open => (OPEN_PER_SECOND, OPEN_ON_SCREEN),
            Stage::Middle => (MIDDLE_PER_SECOND, MIDDLE_ON_SCREEN),
            Stage::Rage => (RAGE_PER_SECOND, RAGE_ON_SCREEN),
        }
    }
}

/// A guardian's charge-up: a wind-up anyone can read, then the blow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Windup {
    /// It crouches with a lane shown on the floor, then dashes down it.
    Charge,
    /// It rears with the floor rippling round it, then the shockwave
    /// rings burst out, one lane left clear (`tide::Ravage`).
    Slam,
    /// A charging line grows from it, then a beam sweeps across the arc.
    Beam,
    /// A shadow grows where a knight stood, then it lands there in a burst.
    Leap,
    /// It draws the room in, shots and knights, then breathes out a burst.
    Inhale,
}

impl Windup {
    pub const ALL: [Windup; 5] = [Windup::Charge, Windup::Slam, Windup::Beam, Windup::Leap, Windup::Inhale];

    pub fn name(self) -> &'static str {
        match self {
            Windup::Charge => "charge",
            Windup::Slam => "slam",
            Windup::Beam => "beam",
            Windup::Leap => "leap",
            Windup::Inhale => "inhale",
        }
    }

    /// The word over it as it winds up, if its file names none.
    fn cry(self) -> &'static str {
        match self {
            Windup::Charge => "GRRRR",
            Windup::Slam => "HNNNG",
            Windup::Beam => "hmmmmm",
            Windup::Leap => "HUP",
            Windup::Inhale => "HUUUP",
        }
    }
}

/// One charge-up line of a `.boss` file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChargeUp {
    pub kind: Windup,
    /// Ticks it winds up, harmless, before the blow (never under `WIND_LEAST`).
    pub wind: u32,
    /// Ticks from one charge-up's end to the next one's start.
    pub every: u32,
    pub damage: u32,
    /// The burst's shots (leap, inhale) or the slam's rings.
    pub shots: u32,
    /// A dash's or a burst's speed, or how fast a slam's rings spread.
    pub speed: f32,
    /// A beam's sweep, or a slam's clear lane, in degrees.
    pub arc: f32,
    /// The stage of the fight it starts in.
    pub from: Stage,
    pub bolt: Bolt,
    /// The word over it as it winds up.
    pub cry: String,
}

impl ChargeUp {
    /// The most shots its blow may throw.
    pub fn burst(&self) -> u32 {
        match self.kind {
            Windup::Leap | Windup::Inhale => self.shots,
            Windup::Beam => 1,
            Windup::Charge | Windup::Slam => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Move {
    /// Walks straight at the nearest knight.
    Chase,
    /// Wanders slowly around where it started.
    Drift,
    /// Sweeps side to side across the top of the room.
    Hover,
    /// Holds its ground.
    Anchor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    /// Shots at the nearest knight, in a tight line.
    Aimed,
    /// A fan of shots centred on the nearest knight.
    Fan,
    /// A full ring, turning a little each time.
    Ring,
    /// Rotating arms that fire constantly.
    Spiral,
    /// Tentacles burst up through the floor in
    /// widening rings round the boss, one lane left clear (`shots` rings,
    /// `arc` the lane's width, `speed` how fast they spread). No shots.
    Ravage,
    /// One of the monsters' patterns, by name (`patterns::Emitter`): its
    /// count, speed and shape are its own.
    Named(Emitter),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bolt {
    Bone,
    Orb,
    Ember,
    Wisp,
    Ore,
    Flame,
    Page,
    Spore,
    Eye,
    /// The look of the boss's own delve: what a named pattern throws when
    /// its file says nothing.
    Own,
}

impl Bolt {
    fn named(word: &str) -> Option<Bolt> {
        Some(match word {
            "bone" => Bolt::Bone,
            "orb" => Bolt::Orb,
            "ember" => Bolt::Ember,
            "wisp" => Bolt::Wisp,
            "ore" => Bolt::Ore,
            "flame" => Bolt::Flame,
            "page" => Bolt::Page,
            "spore" => Bolt::Spore,
            "eye" => Bolt::Eye,
            _ => return None,
        })
    }

    /// The shot it throws, in a boss of `pack`.
    pub fn shot(self, pack: Pack) -> Shot {
        match self {
            Bolt::Bone => Shot::Bone,
            Bolt::Orb => Shot::Orb,
            Bolt::Ember => Shot::Ember,
            Bolt::Wisp => Shot::Wisp,
            Bolt::Ore => Shot::Ore,
            Bolt::Flame => Shot::Flame,
            Bolt::Page => Shot::Page,
            Bolt::Spore => Shot::Spore,
            Bolt::Eye => Shot::Eye,
            Bolt::Own => match pack {
                Pack::Crypt => Shot::Wisp,
                Pack::Cavern => Shot::Ore,
                Pack::Hellforge => Shot::Flame,
                Pack::Archive => Shot::Page,
                Pack::Fungal => Shot::Spore,
                Pack::Unknown => Shot::Eye,
                Pack::Swamp => Shot::Wisp,
            },
        }
    }
}

/// A chain of named patterns walked in turn: `volleys` volleys of each,
/// `every` ticks apart, then the next, round and round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phases {
    pub chain: Vec<Emitter>,
    pub every: u32,
    pub volleys: u32,
    pub damage: u32,
    pub bolt: Bolt,
    /// The stage of the fight it starts in.
    #[serde(default)]
    pub from: Stage,
}

impl Phases {
    /// Shots a second over a whole turn of the chain.
    fn per_second(&self) -> f32 {
        let all: u32 = self.chain.iter().map(|e| e.guardian_shots()).sum();
        all as f32 / self.chain.len().max(1) as f32 * 30.0 / self.every.max(1) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attack {
    pub pattern: Pattern,
    pub shots: u32,
    /// Fan width in degrees (fan), or a ravage's clear lane.
    pub arc: f32,
    pub speed: f32,
    /// Ticks between volleys (30 ticks = one second).
    pub every: u32,
    pub damage: u32,
    pub bolt: Bolt,
    /// The stage of the fight it starts in.
    #[serde(default)]
    pub from: Stage,
}

impl Attack {
    fn per_second(&self) -> f32 {
        match self.pattern {
            // Tentacles, not shots: the budget is for what flies.
            Pattern::Ravage => 0.0,
            Pattern::Named(emitter) => emitter.guardian_shots() as f32 * 30.0 / self.every.max(1) as f32,
            _ => self.shots as f32 * 30.0 / self.every.max(1) as f32,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Boss {
    pub id: String,
    pub name: String,
    pub text: String,
    pub by: String,
    pub only_in: Pack,
    pub hp: u32,
    pub radius: f32,
    pub moves: Move,
    pub speed: f32,
    pub attacks: Vec<Attack>,
    /// Named patterns it walks through in turn, if its file chains any.
    #[serde(default)]
    pub phases: Option<Phases>,
    /// Below this share of health the boss attacks half again as often.
    pub rage: f32,
    /// Its charge-ups, each from its stage of the fight on.
    #[serde(default)]
    pub windups: Vec<ChargeUp>,
    pub drops: Spoils,
    pub art: Vec<String>,
}

impl Boss {
    /// Its stage of the fight at `hp` of `max_hp`.
    pub fn stage(&self, hp: u32, max_hp: u32) -> Stage {
        let share = hp as f32 / max_hp.max(1) as f32;
        let rage = if self.rage > 0.0 { self.rage } else { LAST_FROM };
        if share < rage {
            Stage::Rage
        } else if share < MIDDLE_FROM {
            Stage::Middle
        } else {
            Stage::Open
        }
    }

    /// Shots a second its file asks for in `stage`.
    pub fn per_second(&self, stage: Stage) -> f32 {
        stage_rate(&self.attacks, self.phases.as_ref(), stage)
    }
}

/// Shots a second in `stage` from every line begun by then, at the rage
/// pace (half again as often) in its rage.
fn stage_rate(attacks: &[Attack], phases: Option<&Phases>, stage: Stage) -> f32 {
    let lines = attacks
        .iter()
        .filter(|a| a.from <= stage)
        .map(Attack::per_second)
        .sum::<f32>()
        + phases
            .filter(|p| p.from <= stage)
            .map_or(0.0, Phases::per_second);
    if stage == Stage::Rage { lines * 1.5 } else { lines }
}

#[derive(Debug)]
pub struct Rejected {
    pub errors: Vec<String>,
}

fn number(raw: &str) -> Option<f32> {
    raw.trim_end_matches(['%', 's'])
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

/// `key=value` pairs after a word, clamped into range with a note.
fn arg(
    pairs: &[(String, String)],
    key: &str,
    (low, high, default): (f32, f32, f32),
    what: &str,
    notes: &mut Vec<String>,
) -> f32 {
    let Some(value) = pairs
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| number(v))
    else {
        return default;
    };
    let clamped = value.clamp(low, high);
    if clamped != value {
        notes.push(format!("{what} {key} {value} clamped to {clamped}"));
    }
    clamped
}

fn pairs(rest: &str) -> (Vec<String>, Vec<(String, String)>) {
    let mut words = Vec::new();
    let mut pairs = Vec::new();
    for token in rest.split_whitespace() {
        match token.split_once('=') {
            Some((k, v)) => pairs.push((k.to_ascii_lowercase(), v.to_string())),
            None => words.push(token.to_ascii_lowercase()),
        }
    }
    (words, pairs)
}

/// Check a `.boss` file.
pub fn check(id: &str, raw: &str) -> Result<(Boss, Vec<String>), Rejected> {
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    if raw.len() > MAX_BYTES {
        return Err(Rejected {
            errors: vec![format!("the file is over {MAX_BYTES} bytes")],
        });
    }
    let (mut name, mut text, mut by) = (String::new(), String::new(), String::new());
    let mut only_in = None;
    let (mut hp, mut radius, mut moves, mut speed, mut rage) = (600.0, 1.4, Move::Drift, 2.0, 0.0);
    // The native spider engine occupies its whole rail corridor, legs included.
    let max_radius = if id == super::signatures::ENGINE {
        4.8
    } else {
        2.0
    };
    let mut attacks = Vec::new();
    let mut phases: Option<Phases> = None;
    // Each line's `from=`, if it names one: attacks in turn, then phases.
    let mut attack_from: Vec<Option<Stage>> = Vec::new();
    let mut phases_from: Option<Stage> = None;
    let mut windups: Vec<ChargeUp> = Vec::new();
    let from_of = |kv: &[(String, String)], n: usize, errors: &mut Vec<String>| {
        kv.iter().find(|(k, _)| k == "from").and_then(|(_, v)| {
            let stage = Stage::named(&v.to_ascii_lowercase());
            if stage.is_none() {
                errors.push(format!("line {n}: from `{v}` — open, middle or rage"));
            }
            stage
        })
    };
    let mut drops = Spoils::default();
    let script = super::script::read(raw);
    let art = script.art;
    let clean = super::script::clean;
    for line in &script.lines {
        let (n, word, rest) = (line.n, line.word.as_str(), line.rest);
        match word {
            "name" => name = clean(rest, 28),
            "text" => text = clean(rest, 80),
            "by" => by = clean(rest, 24),
            "where" => match rest.to_ascii_lowercase().as_str() {
                "crypt" | "the crypt" => only_in = Some(Pack::Crypt),
                "mines" | "the mines" => only_in = Some(Pack::Cavern),
                "archive" | "the archive" | "the drowned archive" => only_in = Some(Pack::Archive),
                "fungal" | "the fungal deep" | "fungal deep" => only_in = Some(Pack::Fungal),
                "hellforge" | "dragon keep" => only_in = Some(Pack::Hellforge),
                "unknown" | "the unknown" => only_in = Some(Pack::Unknown),
                "swamp" | "the swamp" | "swamp of horrors" | "the swamp of horrors" => {
                    only_in = Some(Pack::Swamp)
                }
                other => errors.push(format!("line {n}: where `{other}` — crypt, mines, archive, fungal, hellforge, unknown or swamp")),
            },
            "hp" => match number(rest) {
                Some(v) => {
                    hp = v.clamp(300.0, 900.0);
                    if hp != v {
                        notes.push(format!("hp {v} clamped to {hp}"));
                    }
                }
                None => errors.push(format!("line {n}: hp takes a number 300–900")),
            },
            "size" => match number(rest) {
                Some(v) => {
                    radius = v.clamp(0.8, max_radius);
                    if radius != v {
                        notes.push(format!("size {v} clamped to {radius}"));
                    }
                }
                None => errors.push(format!("line {n}: size takes a number 0.8–{max_radius}")),
            },
            "move" => {
                let (words, kv) = pairs(rest);
                moves = match words.first().map(String::as_str) {
                    Some("chase") => Move::Chase,
                    Some("drift") => Move::Drift,
                    Some("hover") => Move::Hover,
                    Some("anchor") => Move::Anchor,
                    _ => {
                        errors.push(format!("line {n}: move chase|drift|hover|anchor speed=S"));
                        Move::Drift
                    }
                };
                speed = arg(&kv, "speed", (0.0, 5.0, 2.0), "move", &mut notes);
            }
            "attack" => {
                let (words, kv) = pairs(rest);
                let pattern = match words.first().map(String::as_str) {
                    Some("aimed") => Pattern::Aimed,
                    Some("fan") => Pattern::Fan,
                    Some("ring") => Pattern::Ring,
                    Some("spiral") => Pattern::Spiral,
                    Some("ravage") => Pattern::Ravage,
                    Some(word) if Emitter::named(word).is_some() => {
                        Pattern::Named(Emitter::named(word).expect("just found"))
                    }
                    other => {
                        errors.push(format!("line {n}: attack `{}` — aimed, fan, ring, spiral, ravage or a pattern: {}", other.unwrap_or(""), pattern_names()));
                        continue;
                    }
                };
                let named = matches!(pattern, Pattern::Named(_));
                let bolt = match kv.iter().find(|(k, _)| k == "shot").map(|(_, v)| v.as_str()) {
                    None if named => Bolt::Own,
                    None => Bolt::Orb,
                    Some(word) => Bolt::named(word).unwrap_or_else(|| {
                        errors.push(format!("line {n}: shot `{word}` — {SHOT_NAMES}"));
                        Bolt::Orb
                    }),
                };
                // Volleys stay small (`VOLLEY_MOST`): pressure is in what
                // a guardian does, not in walls of shots.
                let (shots, every) = match pattern {
                    Pattern::Aimed => ((1.0, 3.0, 1.0), (20.0, 120.0, 45.0)),
                    Pattern::Fan => ((3.0, 7.0, 5.0), (30.0, 150.0, 60.0)),
                    Pattern::Ring => ((4.0, VOLLEY_MOST as f32, 8.0), (45.0, 240.0, 120.0)),
                    Pattern::Spiral => ((2.0, 3.0, 2.0), (10.0, 30.0, 15.0)),
                    Pattern::Ravage => ((3.0, 6.0, 4.0), (150.0, 360.0, 240.0)),
                    // A pattern's count is its own; only its beat is the file's.
                    Pattern::Named(e) => {
                        let own = e.shots() as f32;
                        ((own, own, own), (30.0, 240.0, 90.0))
                    }
                };
                let arc = if pattern == Pattern::Ravage {
                    (30.0, 90.0, 50.0)
                } else {
                    (10.0, 120.0, 40.0)
                };
                let what = format!("attack {}", words[0]);
                attack_from.push(from_of(&kv, n, &mut errors));
                attacks.push(Attack {
                    pattern,
                    shots: arg(&kv, "shots", shots, &what, &mut notes) as u32,
                    arc: arg(&kv, "arc", arc, &what, &mut notes),
                    speed: arg(&kv, "speed", (3.0, 10.0, 6.0), &what, &mut notes),
                    every: arg(&kv, "every", every, &what, &mut notes) as u32,
                    damage: arg(&kv, "damage", (8.0, 20.0, 14.0), &what, &mut notes) as u32,
                    bolt,
                    from: Stage::Open,
                });
            }
            "phases" => {
                let (words, kv) = pairs(rest);
                let mut chain = Vec::new();
                for word in &words {
                    match Emitter::named(word) {
                        Some(e) => chain.push(e),
                        None => errors.push(format!("line {n}: phases `{word}` is not a pattern: {}", pattern_names())),
                    }
                }
                if !(2..=4).contains(&words.len()) {
                    errors.push(format!("line {n}: phases chains 2 to 4 patterns ({} given)", words.len()));
                }
                if phases.is_some() {
                    errors.push(format!("line {n}: one `phases` line to a boss"));
                }
                let bolt = match kv.iter().find(|(k, _)| k == "shot").map(|(_, v)| v.as_str()) {
                    None => Bolt::Own,
                    Some(word) => Bolt::named(word).unwrap_or_else(|| {
                        errors.push(format!("line {n}: shot `{word}` — {SHOT_NAMES}"));
                        Bolt::Own
                    }),
                };
                phases_from = from_of(&kv, n, &mut errors);
                phases = Some(Phases {
                    chain,
                    every: arg(&kv, "every", (20.0, 150.0, 50.0), "phases", &mut notes) as u32,
                    volleys: arg(&kv, "volleys", (1.0, 6.0, 2.0), "phases", &mut notes) as u32,
                    damage: arg(&kv, "damage", (8.0, 20.0, 14.0), "phases", &mut notes) as u32,
                    bolt,
                    from: Stage::Open,
                });
            }
            "windup" => {
                let (words, kv) = pairs(rest);
                let Some(kind) = words
                    .first()
                    .and_then(|w| Windup::ALL.into_iter().find(|k| k.name() == w))
                else {
                    errors.push(format!("line {n}: windup `{}` — charge, slam, beam, leap or inhale", words.first().map_or("", String::as_str)));
                    continue;
                };
                let what = format!("windup {}", kind.name());
                let bolt = match kv.iter().find(|(k, _)| k == "shot").map(|(_, v)| v.as_str()) {
                    None => Bolt::Own,
                    Some(word) => Bolt::named(word).unwrap_or_else(|| {
                        errors.push(format!("line {n}: shot `{word}` — {SHOT_NAMES}"));
                        Bolt::Own
                    }),
                };
                // Its shots, speed and arc mean what its kind makes of them.
                let (shots, speed, arc) = match kind {
                    Windup::Charge => ((0.0, 0.0, 0.0), (18.0, 36.0, 28.0), (0.0, 0.0, 0.0)),
                    Windup::Slam => ((1.0, 3.0, 2.0), (3.0, 10.0, 6.0), (40.0, 90.0, 60.0)),
                    Windup::Beam => ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0), (30.0, 150.0, 90.0)),
                    Windup::Leap => ((0.0, 8.0, 6.0), (3.0, 8.0, 5.0), (0.0, 0.0, 0.0)),
                    Windup::Inhale => ((4.0, 8.0, 8.0), (3.0, 8.0, 5.0), (0.0, 0.0, 0.0)),
                };
                // A slam's rings ripple for as long as the floor warns.
                let wind = if kind == Windup::Slam {
                    (super::tide::WARN as f32, super::tide::WARN as f32, super::tide::WARN as f32)
                } else {
                    (WIND_LEAST as f32, 60.0, 30.0)
                };
                let cry = kv
                    .iter()
                    .find(|(k, _)| k == "cry")
                    .map(|(_, v)| clean(v, 10))
                    .filter(|c| !c.trim().is_empty())
                    .unwrap_or_else(|| kind.cry().to_string());
                windups.push(ChargeUp {
                    kind,
                    wind: arg(&kv, "wind", wind, &what, &mut notes) as u32,
                    every: arg(&kv, "every", (120.0, 600.0, 240.0), &what, &mut notes) as u32,
                    damage: arg(&kv, "damage", (8.0, 20.0, 16.0), &what, &mut notes) as u32,
                    shots: arg(&kv, "shots", shots, &what, &mut notes) as u32,
                    speed: arg(&kv, "speed", speed, &what, &mut notes),
                    arc: arg(&kv, "arc", arc, &what, &mut notes),
                    from: from_of(&kv, n, &mut errors).unwrap_or(Stage::Middle),
                    bolt,
                    cry,
                });
            }
            "rage" => match number(rest) {
                Some(v) => rage = (if v > 1.0 { v / 100.0 } else { v }).clamp(0.0, 0.6),
                None => errors.push(format!("line {n}: rage takes a share of health, e.g. 40%")),
            },
            "drops" => {
                for part in rest.split([',', '·']) {
                    let mut it = part.split_whitespace();
                    match (it.next().and_then(Spoil::from_word), it.next().and_then(|v| v.parse::<u32>().ok())) {
                        (Some(s), Some(v)) if !matches!(s, Spoil::Scale | Spoil::Bond) => drops.add(s, v.min(if s == Spoil::Gold { 300 } else { 6 })),
                        _ if part.trim().is_empty() => {}
                        _ => errors.push(format!("line {n}: drops `{}` — gold, bone, wax, ore, gem or ember with a count", part.trim())),
                    }
                }
            }
            other => errors.push(format!(
                "line {n}: unknown word `{other}`; a boss has name, text, by, where, hp, size, move, attack, phases, windup, rage, drops, art"
            )),
        }
    }
    if name.trim().is_empty() {
        errors.push("the boss needs a `name`".into());
    }
    let Some(only_in) = only_in else {
        errors.push("the boss needs `where`: crypt, mines, archive, fungal, hellforge or unknown".into());
        return Err(Rejected { errors });
    };
    if attacks.is_empty() && phases.is_none() {
        errors.push("the boss needs at least one `attack` or a `phases` chain".into());
    }
    if attacks.len() > 3 {
        errors.push(format!("at most 3 attacks ({} given)", attacks.len()));
    }
    if windups.len() > WINDUPS_MOST {
        errors.push(format!("at most {WINDUPS_MOST} windups ({} given)", windups.len()));
    }
    if art.is_empty() || art.len() > ART || art.iter().any(|r| r.chars().count() > ART) {
        errors.push(format!(
            "the boss needs `art`: 1 to {ART} rows of up to {ART} inks"
        ));
    }
    if !errors.is_empty() {
        return Err(Rejected { errors });
    }
    // The fight builds: unless a line says `from=`, the first attack opens
    // it (the phases, if there is no attack), the rest join in the middle.
    for (k, (attack, from)) in attacks.iter_mut().zip(&attack_from).enumerate() {
        attack.from = from.unwrap_or(if k == 0 { Stage::Open } else { Stage::Middle });
    }
    if let Some(phases) = &mut phases {
        phases.from = phases_from.unwrap_or(if attacks.is_empty() { Stage::Open } else { Stage::Middle });
    }
    // Each stage's lines, at its pace, held under its budget of shots a
    // second by slowing them.
    for stage in Stage::ALL {
        let rate = stage_rate(&attacks, phases.as_ref(), stage);
        let budget = stage.caps().0 as f32;
        if rate > budget {
            let slow = rate / budget;
            for attack in attacks.iter_mut().filter(|a| a.from <= stage) {
                attack.every = ((attack.every as f32 * slow).ceil() as u32).min(400);
            }
            if let Some(phases) = phases.as_mut().filter(|p| p.from <= stage) {
                phases.every = ((phases.every as f32 * slow).ceil() as u32).min(400);
            }
            notes.push(format!(
                "{stage:?}: {rate:.1} shots a second is over the budget of {budget}; volleys slowed {slow:.2}x"
            ));
        }
    }
    let id = if id.is_empty() {
        super::cards::slug(&name)
    } else {
        super::cards::slug(id)
    };
    Ok((
        Boss {
            id,
            name: name.trim().into(),
            text,
            by,
            only_in,
            hp: hp as u32,
            radius,
            moves,
            speed,
            attacks,
            phases,
            rage,
            windups,
            drops,
            art,
        },
        notes,
    ))
}

/// The shots a file may name.
const SHOT_NAMES: &str = "bone, orb, ember, wisp, ore, flame, page, spore or eye";

fn pattern_names() -> String {
    Emitter::ALL
        .iter()
        .map(|e| e.name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Checked delivery definitions; this catalogue does not add bosses to the live population.
pub const CHECKED: &[(&str, &str)] = &[
    ("archive-bellkeeper", include_str!("../../../../cockpit/assets/dungeon/bosses/archive-bellkeeper.boss")),
    ("cinderjaw", include_str!("../../../../cockpit/assets/dungeon/bosses/cinderjaw.boss")),
    ("crypt-relic-liege", include_str!("../../../../cockpit/assets/dungeon/bosses/crypt-relic-liege.boss")),
    ("crypt-tax-keeper", include_str!("../../../../cockpit/assets/dungeon/bosses/crypt-tax-keeper.boss")),
    ("crypt-tithe-marshal", include_str!("../../../../cockpit/assets/dungeon/bosses/crypt-tithe-marshal.boss")),
    ("fungal-rootwarden", include_str!("../../../../cockpit/assets/dungeon/bosses/fungal-rootwarden.boss")),
    ("fungal-spore-crown", include_str!("../../../../cockpit/assets/dungeon/bosses/fungal-spore-crown.boss")),
    ("grinwick", include_str!("../../../../cockpit/assets/dungeon/bosses/grinwick.boss")),
    ("hellforge-ash-castellan", include_str!("../../../../cockpit/assets/dungeon/bosses/hellforge-ash-castellan.boss")),
    ("hellforge-chain-keeper", include_str!("../../../../cockpit/assets/dungeon/bosses/hellforge-chain-keeper.boss")),
    ("hellforge-free-captain", include_str!("../../../../cockpit/assets/dungeon/bosses/hellforge-free-captain.boss")),
    ("hellforge-fuel-reeve", include_str!("../../../../cockpit/assets/dungeon/bosses/hellforge-fuel-reeve.boss")),
    ("hellforge-slag-sovereign", include_str!("../../../../cockpit/assets/dungeon/bosses/hellforge-slag-sovereign.boss")),
    ("iron-tusk-chief", include_str!("../../../../cockpit/assets/dungeon/bosses/iron-tusk-chief.boss")),
    ("late-fee-leviathan", include_str!("../../../../cockpit/assets/dungeon/bosses/late-fee-leviathan.boss")),
    ("mother-of-spores", include_str!("../../../../cockpit/assets/dungeon/bosses/mother-of-spores.boss")),
    ("red-pick-chief", include_str!("../../../../cockpit/assets/dungeon/bosses/red-pick-chief.boss")),
    ("salvage-claimant", include_str!("../../../../cockpit/assets/dungeon/bosses/salvage-claimant.boss")),
    ("shoggoth-heart", include_str!("../../../../cockpit/assets/dungeon/bosses/shoggoth-heart.boss")),
    ("the-big-ripper", include_str!("../../../../cockpit/assets/dungeon/bosses/the-big-ripper.boss")),
    ("the-bone-choir", include_str!("../../../../cockpit/assets/dungeon/bosses/the-bone-choir.boss")),
    ("the-foreman", include_str!("../../../../cockpit/assets/dungeon/bosses/the-foreman.boss")),
    ("the-index", include_str!("../../../../cockpit/assets/dungeon/bosses/the-index.boss")),
    ("unknown-memory-warden", include_str!("../../../../cockpit/assets/dungeon/bosses/unknown-memory-warden.boss")),
    ("unknown-silence-warden", include_str!("../../../../cockpit/assets/dungeon/bosses/unknown-silence-warden.boss")),
    ("waxen-warden", include_str!("../../../../cockpit/assets/dungeon/bosses/waxen-warden.boss")),
];

pub fn checked(id: &str) -> Option<Boss> {
    let (_, raw) = CHECKED.iter().find(|(name, _)| *name == id)?;
    Some(check(id, raw).expect("checked boss definition").0)
}

/// The delve's own bosses.
pub const BUILTIN: &[(&str, &str)] = &[
    (
        "the-big-ripper",
        include_str!("../../../../cockpit/assets/dungeon/bosses/the-big-ripper.boss"),
    ),
    (
        "grinwick",
        include_str!("../../../../cockpit/assets/dungeon/bosses/grinwick.boss"),
    ),
    (
        "waxen-warden",
        include_str!("../../../../cockpit/assets/dungeon/bosses/waxen-warden.boss"),
    ),
    (
        "cinderjaw",
        include_str!("../../../../cockpit/assets/dungeon/bosses/cinderjaw.boss"),
    ),
    (
        "the-index",
        include_str!("../../../../cockpit/assets/dungeon/bosses/the-index.boss"),
    ),
    (
        "mother-of-spores",
        include_str!("../../../../cockpit/assets/dungeon/bosses/mother-of-spores.boss"),
    ),
    (
        "the-bone-choir",
        include_str!("../../../../cockpit/assets/dungeon/bosses/the-bone-choir.boss"),
    ),
    (
        "the-foreman",
        include_str!("../../../../cockpit/assets/dungeon/bosses/the-foreman.boss"),
    ),
    (
        "late-fee-leviathan",
        include_str!("../../../../cockpit/assets/dungeon/bosses/late-fee-leviathan.boss"),
    ),
    (
        "the-mire-king",
        include_str!("../../../../cockpit/assets/dungeon/bosses/the-mire-king.boss"),
    ),
];

pub fn builtin() -> Vec<Boss> {
    BUILTIN
        .iter()
        .map(|(id, raw)| {
            check(id, raw)
                .expect("built-in bosses pass their own checker")
                .0
        })
        .collect()
}

/// Read one `.boss` file's text into `bosses`: a workspace's own guardian
/// takes its delve's place. The line the reload reports for it.
pub fn read_into(bosses: &mut Vec<Boss>, stem: &str, raw: &str) -> String {
    match check(stem, raw) {
        Ok((boss, notes)) => {
            let line = if notes.is_empty() {
                format!("{stem}: {}", boss.name)
            } else {
                format!("{stem}: {} ({})", boss.name, notes.join("; "))
            };
            bosses.retain(|b| b.only_in != boss.only_in);
            bosses.push(boss);
            line
        }
        Err(rejected) => format!("{stem}: {}", rejected.errors.join("; ")),
    }
}

/// The boss format, for a person or a model writing one.
pub fn rules() -> String {
    format!(
        "A delve boss is a text file `<id>.boss`. It guards the stairs room of its delve; the way down stays barred until it falls.\n\
name   <up to 28 chars>\n\
text   <one line of lore, up to 80 chars>\n\
by     <who made it>\n\
where  crypt | mines | archive | fungal | hellforge | unknown | swamp\n\
hp     300–900 (for one knight; it grows with the party)\n\
size   0.8–2.0 (hit radius in arena units; native grinwick allows 4.8; a knight is 0.42, the room is 48 by 28)\n\
move   chase | drift | hover | anchor  speed=0–5\n\
attack <pattern> shots=N speed=3–10 every=T damage=8–20 shot=bone|orb|ember|wisp|ore|flame|page|spore|eye   (up to 3 lines; T in ticks, 30 = one second)\n\
  aimed  shots 1–3, every 20–120 — at the nearest knight\n\
  fan    shots 3–9, arc=10–120 degrees, every 30–150 — centred on the nearest knight\n\
  ring   shots 6–20, every 45–240 — a full turning ring\n\
  spiral shots 2–4 (arms), every 6–30 — constant rotating streams\n\
  ravage shots 3–6 (rings), arc=30–90 (the lane left clear), every 150–360 — tentacles burst up in widening rings round the boss, speed=how fast they spread; no shot\n\
  or a named pattern, every 30–240 (its count and speed are its own; shot defaults to the delve's own look):\n\
  {names}\n\
phases <pattern> <pattern> [<pattern> <pattern>]  every=20–150 volleys=1–6 damage=8–20 shot=...\n\
  2–4 named patterns walked in turn, `volleys` volleys of each, then round again (one line)\n\
Any attack or phases line may say from=open|middle|rage: the stage of the fight it starts in. The fight\n\
  opens above {middle}% health, then the middle, then rage. Unsaid, the first attack opens (the phases\n\
  line, if there is no attack) and the rest join in the middle.\n\
  Shots a second, at most: opening {OPEN_PER_SECOND}, middle {MIDDLE_PER_SECOND}, rage {RAGE_PER_SECOND} (faster designs are slowed); in the room at once,\n\
  per knight: {OPEN_ON_SCREEN}, {MIDDLE_ON_SCREEN}, {RAGE_ON_SCREEN}. A volley holds at most {VOLLEY_MOST} shots.\n\
windup charge|slam|beam|leap|inhale  wind=T every=T damage=8–20 from=middle cry=WORD   (up to {WINDUPS_MOST} lines)\n\
  a charge-up: it winds up for `wind` ticks ({WIND_LEAST}–60, harmless and shown), then the blow; `every` (120–600)\n\
  ticks between one's end and the next. From the middle of the fight on, unless from= says otherwise.\n\
  charge speed=18–36        a lane shown on the floor, then it dashes down it\n\
  slam   shots=1–3 (rings) speed=3–10 arc=40–90 (the clear lane)   the floor ripples, then shockwave rings\n\
  beam   arc=30–150 (its sweep)   a charging line, then a beam sweeps the arc\n\
  leap   shots=0–8 speed=3–8   a shadow grows where a knight stood, then it lands there in a burst\n\
  inhale shots=4–8 speed=3–8   it draws the room in, shots and knights, then breathes out a burst\n\
rage   0–60% — below this share of health it attacks half again as often\n\
drops  spoils, e.g. `bone 4, wax 2, gold 150` (gold ≤300, others ≤6 each; no scales or bonds)\n\
art    then up to {ART} rows of up to {ART} inks, `.` = black paper. Inks (each group dark→light):\n\
  greys k K Z X g j G J h i H · greens f D F E e l N m L A M y C Y · wood n b I B p P r R o O t T\n\
  stone s x S u U v V W q Q z · signal (glows: eyes, fire, gems only) 0 1 2 3 w a 4 @ 5 6 $ c 9 8 7",
        names = pattern_names(),
        middle = (MIDDLE_FROM * 100.0) as u32,
    )
}

#[cfg(test)]
#[path = "../../tests/rules/together_bosses__tests.rs"]
mod tests;
