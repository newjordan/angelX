//! Floor bosses: a `.boss` file per guardian, written by people or models.
//!
//! A boss stands in a floor's stairs room; the way down stays barred until it
//! falls. Like cards, a boss is checked data — a movement, up to three attack
//! patterns from a fixed vocabulary, a rage threshold, spoils it drops and its
//! own pixel art — with every number clamped and the whole fight held under a
//! bullet budget, so a boss can be fierce but never unfair or unbounded.

use super::Pack;
use crate::drive::together_realm::{Spoil, Spoils};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_BYTES: usize = 8192;
pub(crate) const ART: usize = 32;
/// Hostile shots per second, all attacks together, before rage.
const BULLET_BUDGET: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Move {
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
pub(crate) enum Pattern {
    /// Shots at the nearest knight, in a tight line.
    Aimed,
    /// A fan of shots centred on the nearest knight.
    Fan,
    /// A full ring, turning a little each time.
    Ring,
    /// Rotating arms that fire constantly.
    Spiral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Bolt {
    Bone,
    Orb,
    Ember,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Attack {
    pub(crate) pattern: Pattern,
    pub(crate) shots: u32,
    /// Fan width in degrees (fan only).
    pub(crate) arc: f32,
    pub(crate) speed: f32,
    /// Ticks between volleys (30 ticks = one second).
    pub(crate) every: u32,
    pub(crate) damage: u32,
    pub(crate) bolt: Bolt,
}

impl Attack {
    fn per_second(&self) -> f32 {
        self.shots as f32 * 30.0 / self.every.max(1) as f32
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Boss {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) text: String,
    pub(crate) by: String,
    pub(crate) only_in: Pack,
    pub(crate) hp: u32,
    pub(crate) radius: f32,
    pub(crate) moves: Move,
    pub(crate) speed: f32,
    pub(crate) attacks: Vec<Attack>,
    /// Below this share of health the boss attacks half again as often.
    pub(crate) rage: f32,
    pub(crate) drops: Spoils,
    pub(crate) art: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct Rejected {
    pub(crate) errors: Vec<String>,
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
pub(crate) fn check(id: &str, raw: &str) -> Result<(Boss, Vec<String>), Rejected> {
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
    let mut attacks = Vec::new();
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
                other => errors.push(format!("line {n}: where `{other}` — crypt or mines (Dragon Keep keeps its dragon)")),
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
                    radius = v.clamp(0.8, 2.0);
                    if radius != v {
                        notes.push(format!("size {v} clamped to {radius}"));
                    }
                }
                None => errors.push(format!("line {n}: size takes a number 0.8–2.0")),
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
                    other => {
                        errors.push(format!("line {n}: attack `{}` — aimed, fan, ring or spiral", other.unwrap_or("")));
                        continue;
                    }
                };
                let bolt = match kv.iter().find(|(k, _)| k == "shot").map(|(_, v)| v.as_str()) {
                    None | Some("orb") => Bolt::Orb,
                    Some("bone") => Bolt::Bone,
                    Some("ember") => Bolt::Ember,
                    Some(other) => {
                        errors.push(format!("line {n}: shot `{other}` — bone, orb or ember"));
                        Bolt::Orb
                    }
                };
                let (shots, every) = match pattern {
                    Pattern::Aimed => ((1.0, 3.0, 1.0), (20.0, 120.0, 45.0)),
                    Pattern::Fan => ((3.0, 9.0, 5.0), (30.0, 150.0, 60.0)),
                    Pattern::Ring => ((6.0, 20.0, 12.0), (45.0, 240.0, 120.0)),
                    Pattern::Spiral => ((2.0, 4.0, 3.0), (6.0, 30.0, 10.0)),
                };
                let what = format!("attack {}", words[0]);
                attacks.push(Attack {
                    pattern,
                    shots: arg(&kv, "shots", shots, &what, &mut notes) as u32,
                    arc: arg(&kv, "arc", (10.0, 120.0, 40.0), &what, &mut notes),
                    speed: arg(&kv, "speed", (3.0, 10.0, 6.0), &what, &mut notes),
                    every: arg(&kv, "every", every, &what, &mut notes) as u32,
                    damage: arg(&kv, "damage", (8.0, 20.0, 14.0), &what, &mut notes) as u32,
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
                "line {n}: unknown word `{other}`; a boss has name, text, by, where, hp, size, move, attack, rage, drops, art"
            )),
        }
    }
    if name.trim().is_empty() {
        errors.push("the boss needs a `name`".into());
    }
    let Some(only_in) = only_in else {
        errors.push("the boss needs `where crypt` or `where mines`".into());
        return Err(Rejected { errors });
    };
    if attacks.is_empty() {
        errors.push("the boss needs at least one `attack`".into());
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
    let rate: f32 = attacks.iter().map(Attack::per_second).sum();
    if rate > BULLET_BUDGET {
        let slow = rate / BULLET_BUDGET;
        for attack in &mut attacks {
            attack.every = ((attack.every as f32 * slow).ceil() as u32).min(400);
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
            rage,
            drops,
            art,
        },
        notes,
    ))
}

/// The delve's own bosses.
pub(crate) const BUILTIN: &[(&str, &str)] = &[
    (
        "waxen-warden",
        include_str!("../../../assets/dungeon/bosses/waxen-warden.boss"),
    ),
    (
        "cinderjaw",
        include_str!("../../../assets/dungeon/bosses/cinderjaw.boss"),
    ),
];

pub(crate) fn builtin() -> Vec<Boss> {
    BUILTIN
        .iter()
        .map(|(id, raw)| {
            check(id, raw)
                .expect("built-in bosses pass their own checker")
                .0
        })
        .collect()
}

/// Read every `*.boss` in `dir` into `bosses`: a workspace's own guardian
/// takes its delve's place. Returns one line per file.
pub(crate) fn load_dir(dir: &std::path::Path, bosses: &mut Vec<Boss>) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "boss") && p.is_file())
        .collect();
    paths.sort();
    let mut report = Vec::new();
    for path in paths.into_iter().take(8) {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let raw = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() as usize <= MAX_BYTES => {
                std::fs::read_to_string(&path).unwrap_or_default()
            }
            _ => {
                report.push(format!("{stem}: over {MAX_BYTES} bytes, skipped"));
                continue;
            }
        };
        match check(&stem, &raw) {
            Ok((boss, notes)) => {
                let line = if notes.is_empty() {
                    format!("{stem}: {}", boss.name)
                } else {
                    format!("{stem}: {} ({})", boss.name, notes.join("; "))
                };
                bosses.retain(|b| b.only_in != boss.only_in);
                bosses.push(boss);
                report.push(line);
            }
            Err(rejected) => report.push(format!("{stem}: {}", rejected.errors.join("; "))),
        }
    }
    report
}

/// The boss format, for a person or a model writing one.
pub(crate) fn rules() -> String {
    format!(
        "A delve boss is a text file `<id>.boss`. It guards the stairs room of its delve; the way down stays barred until it falls.\n\
name   <up to 28 chars>\n\
text   <one line of lore, up to 80 chars>\n\
by     <who made it>\n\
where  crypt | mines\n\
hp     300–900 (for one knight; it grows with the party)\n\
size   0.8–2.0 (hit radius in arena units; a knight is 0.42, the room is 48 by 28)\n\
move   chase | drift | hover | anchor  speed=0–5\n\
attack <pattern> shots=N speed=3–10 every=T damage=8–20 shot=bone|orb|ember   (up to 3 lines; T in ticks, 30 = one second)\n\
  aimed  shots 1–3, every 20–120 — at the nearest knight\n\
  fan    shots 3–9, arc=10–120 degrees, every 30–150 — centred on the nearest knight\n\
  ring   shots 6–20, every 45–240 — a full turning ring\n\
  spiral shots 2–4 (arms), every 6–30 — constant rotating streams\n\
All attacks together may fire at most {BULLET_BUDGET} shots a second; faster designs are slowed.\n\
rage   0–60% — below this share of health it attacks half again as often\n\
drops  spoils, e.g. `bone 4, wax 2, gold 150` (gold ≤300, others ≤6 each; no scales or bonds)\n\
art    then up to {ART} rows of up to {ART} inks, `.` = black paper. Inks (each group dark→light):\n\
  greys k K Z X g j G J h i H · greens f D F E e l N m L A M y C Y · wood n b I B p P r R o O t T\n\
  stone s x S u U v V W q Q z · signal (glows: eyes, fire, gems only) 0 1 2 3 w a 4 @ 5 6 $ c 9 8 7"
    )
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_bosses__tests.rs"]
mod tests;
