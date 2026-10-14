//! Floor bosses: a `.boss` file per guardian, written by people or models.
//!
//! A boss stands in a floor's stairs room; the way down stays barred until it
//! falls. Like cards, a boss is checked data — a movement, up to three attack
//! patterns from a fixed vocabulary, a rage threshold, spoils it drops and its
//! own pixel art — with every number clamped and the whole fight held under a
//! bullet budget, so a boss can be fierce but never unfair or unbounded.
//!
//! An attack can also name any of the monsters' patterns (`patterns`), and
//! a `phases` line chains two to four of them, a few volleys each, round
//! and round. A name the vocabulary lacks is rejected.

use super::Pack;
use super::Shot;
use super::patterns::Emitter;
use crate::realm::{Spoil, Spoils};
use serde::{Deserialize, Serialize};

pub const MAX_BYTES: usize = 8192;
pub const ART: usize = 32;
/// Hostile shots per second, all attacks together, before rage.
const BULLET_BUDGET: f32 = 14.0;

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
}

impl Phases {
    /// Shots a second over a whole turn of the chain.
    fn per_second(&self) -> f32 {
        let all: u32 = self.chain.iter().map(|e| e.shots()).sum();
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
}

impl Attack {
    fn per_second(&self) -> f32 {
        match self.pattern {
            // Tentacles, not shots: the budget is for what flies.
            Pattern::Ravage => 0.0,
            Pattern::Named(emitter) => emitter.shots() as f32 * 30.0 / self.every.max(1) as f32,
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
    pub drops: Spoils,
    pub art: Vec<String>,
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
                other => errors.push(format!("line {n}: where `{other}` — crypt, mines, archive, fungal, hellforge or unknown")),
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
                let (shots, every) = match pattern {
                    Pattern::Aimed => ((1.0, 3.0, 1.0), (20.0, 120.0, 45.0)),
                    Pattern::Fan => ((3.0, 9.0, 5.0), (30.0, 150.0, 60.0)),
                    Pattern::Ring => ((6.0, 20.0, 12.0), (45.0, 240.0, 120.0)),
                    Pattern::Spiral => ((2.0, 4.0, 3.0), (6.0, 30.0, 10.0)),
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
                attacks.push(Attack {
                    pattern,
                    shots: arg(&kv, "shots", shots, &what, &mut notes) as u32,
                    arc: arg(&kv, "arc", arc, &what, &mut notes),
                    speed: arg(&kv, "speed", (3.0, 10.0, 6.0), &what, &mut notes),
                    every: arg(&kv, "every", every, &what, &mut notes) as u32,
                    damage: arg(&kv, "damage", (8.0, 20.0, 14.0), &what, &mut notes) as u32,
                    bolt,
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
                phases = Some(Phases {
                    chain,
                    every: arg(&kv, "every", (20.0, 150.0, 50.0), "phases", &mut notes) as u32,
                    volleys: arg(&kv, "volleys", (1.0, 6.0, 2.0), "phases", &mut notes) as u32,
                    damage: arg(&kv, "damage", (8.0, 20.0, 14.0), "phases", &mut notes) as u32,
                    bolt,
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
                "line {n}: unknown word `{other}`; a boss has name, text, by, where, hp, size, move, attack, phases, rage, drops, art"
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
    if art.is_empty() || art.len() > ART || art.iter().any(|r| r.chars().count() > ART) {
        errors.push(format!(
            "the boss needs `art`: 1 to {ART} rows of up to {ART} inks"
        ));
    }
    if !errors.is_empty() {
        return Err(Rejected { errors });
    }
    // Hold the whole fight under the bullet budget by slowing every volley.
    let rate: f32 = attacks.iter().map(Attack::per_second).sum::<f32>()
        + phases.as_ref().map_or(0.0, Phases::per_second);
    if rate > BULLET_BUDGET {
        let slow = rate / BULLET_BUDGET;
        for attack in &mut attacks {
            attack.every = ((attack.every as f32 * slow).ceil() as u32).min(400);
        }
        if let Some(phases) = &mut phases {
            phases.every = ((phases.every as f32 * slow).ceil() as u32).min(400);
        }
        notes.push(format!("{rate:.1} shots a second is over the budget of {BULLET_BUDGET}; volleys slowed {slow:.2}x"));
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
where  crypt | mines | archive | fungal | hellforge | unknown\n\
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
All attacks together may fire at most {BULLET_BUDGET} shots a second; faster designs are slowed.\n\
rage   0–60% — below this share of health it attacks half again as often\n\
drops  spoils, e.g. `bone 4, wax 2, gold 150` (gold ≤300, others ≤6 each; no scales or bonds)\n\
art    then up to {ART} rows of up to {ART} inks, `.` = black paper. Inks (each group dark→light):\n\
  greys k K Z X g j G J h i H · greens f D F E e l N m L A M y C Y · wood n b I B p P r R o O t T\n\
  stone s x S u U v V W q Q z · signal (glows: eyes, fire, gems only) 0 1 2 3 w a 4 @ 5 6 $ c 9 8 7",
        names = pattern_names()
    )
}

#[cfg(test)]
#[path = "../../tests/rules/together_bosses__tests.rs"]
mod tests;
