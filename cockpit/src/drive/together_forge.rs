//! The forge: a player's own model writes a weapon as rune lines, and this
//! module checks, clamps and compiles it. Runes are data from a fixed
//! vocabulary; nothing a model writes ever runs. See docs/DELVE.md.

use serde::{Deserialize, Serialize};

/// Every knight holds this much LOC until the LOC economy lands.
pub(crate) const START_LOC: u32 = 6;
pub(crate) const MAX_BYTES: usize = 4096;
const MAX_LINES: usize = 64;
const MAX_LINE: usize = 80;
/// Tier I caps: paper damage per second and per volley-plus-swing.
const DPS_CAP: f32 = 70.0;
const BURST_CAP: u32 = 100;
const MAX_SHOTS: u32 = 3;
/// A blade reaches up to two tiles: close work that still reads as a sweep
/// (one tile meant standing in a monster to hit it).
const MAX_REACH: f32 = 4.0;
const LOCKED: &[(&str, &str)] = &[
    ("pierce", "II"),
    ("parry", "II"),
    ("lunge", "II"),
    ("homing", "III"),
    ("on_hit", "II"),
];
const COLOURS: &[&str] = &["ember", "ice", "gold", "venom", "void", "rose", "steel"];
const SHAPES: &[&str] = &["orb", "shard", "arrow", "star", "blade", "wave"];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Bolt {
    pub(crate) damage: u32,
    pub(crate) speed: f32,
    pub(crate) every: u32,
    pub(crate) range: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Spread {
    pub(crate) shots: u32,
    pub(crate) arc: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Melee {
    pub(crate) damage: u32,
    pub(crate) reach: f32,
    pub(crate) arc: f32,
    pub(crate) every: u32,
}

/// A thrown blade that flies out to `range` and comes back to the hand,
/// cutting every monster it passes on both legs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Throw {
    pub(crate) damage: u32,
    pub(crate) range: f32,
    pub(crate) every: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Weapon {
    pub(crate) name: String,
    pub(crate) colour: String,
    pub(crate) shape: String,
    pub(crate) bolt: Option<Bolt>,
    pub(crate) spread: Option<Spread>,
    pub(crate) melee: Option<Melee>,
    #[serde(default)]
    pub(crate) throw: Option<Throw>,
    /// LOC the runes spend.
    pub(crate) loc: u32,
    /// The weapon as canonical runes, after clamping: what the model is shown
    /// as "my current weapon" next time.
    pub(crate) runes: String,
}

impl Weapon {
    #[cfg(test)]
    pub(crate) fn starter() -> Self {
        check(
            "name Bow\nlook gold arrow\nbolt damage=32 speed=12 every=15 range=48",
            START_LOC,
        )
        .expect("the starter wand is legal")
        .weapon
    }

    pub(crate) fn kind(&self) -> &'static str {
        match (
            self.bolt.is_some(),
            self.melee.is_some() || self.throw.is_some(),
        ) {
            (true, true) => "hybrid",
            (false, true) => "melee",
            _ => "ranged",
        }
    }

    pub(crate) fn shots(&self) -> u32 {
        self.spread.map_or(1, |spread| spread.shots)
    }

    /// Paper damage, counting every shot as a hit.
    pub(crate) fn dps(&self) -> f32 {
        let ranged = self.bolt.map_or(0.0, |bolt| {
            (bolt.damage * self.shots()) as f32 * 30.0 / bolt.every as f32
        });
        let melee = self
            .melee
            .map_or(0.0, |melee| melee.damage as f32 * 30.0 / melee.every as f32);
        let throw = self
            .throw
            .map_or(0.0, |throw| throw.damage as f32 * 30.0 / throw.every as f32);
        ranged + melee + throw
    }

    pub(crate) fn burst(&self) -> u32 {
        self.bolt.map_or(0, |bolt| bolt.damage * self.shots())
            + self.melee.map_or(0, |melee| melee.damage)
            + self.throw.map_or(0, |throw| throw.damage)
    }

    pub(crate) fn summary(&self) -> String {
        format!(
            "{} · {} · {:.0} dmg/s · {}/{} LOC",
            self.name,
            self.kind(),
            self.dps(),
            self.loc,
            START_LOC
        )
    }

    fn canonical(&self) -> String {
        let mut lines = vec![
            format!("name {}", self.name),
            format!("look {} {}", self.colour, self.shape),
        ];
        if let Some(bolt) = self.bolt {
            lines.push(format!(
                "bolt damage={} speed={} every={} range={}",
                bolt.damage, bolt.speed, bolt.every, bolt.range
            ));
        }
        if let Some(spread) = self.spread {
            lines.push(format!("spread shots={} arc={}", spread.shots, spread.arc));
        }
        if let Some(melee) = self.melee {
            lines.push(format!(
                "melee damage={} reach={} arc={} every={}",
                melee.damage, melee.reach, melee.arc, melee.every
            ));
        }
        if let Some(throw) = self.throw {
            lines.push(format!(
                "throw damage={} range={} every={}",
                throw.damage, throw.range, throw.every
            ));
        }
        lines.join("\n")
    }
}

#[derive(Debug)]
pub(crate) struct Checked {
    pub(crate) weapon: Weapon,
    /// Every value the forge changed, in words a model can act on.
    pub(crate) notes: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct Rejected {
    pub(crate) errors: Vec<String>,
}

impl Rejected {
    /// The message a player carries back to their model.
    pub(crate) fn fix_it(&self) -> String {
        format!(
            "The forge rejected that weapon:\n{}\nFix these and reply with only the corrected rune file.",
            self.errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// Chat models wrap answers in prose and code fences; only the first fenced
/// block counts when there is one.
fn rune_text(raw: &str) -> &str {
    let Some(open) = raw.find("```") else {
        return raw;
    };
    let body = &raw[open + 3..];
    let body = body.split_once('\n').map_or("", |(_, rest)| rest);
    body.find("```").map_or(body, |close| &body[..close])
}

fn parse_number(key: &str, raw: &str, line: usize, errors: &mut Vec<String>) -> Option<f32> {
    let trimmed = raw.trim_end_matches(['%', 's']);
    match trimmed.parse::<f32>() {
        Ok(value) if value.is_finite() => Some(value),
        _ => {
            errors.push(format!("line {line}: {key}={raw} is not a number"));
            None
        }
    }
}

struct Args {
    pairs: Vec<(String, String)>,
}

impl Args {
    fn parse(rest: &str) -> Self {
        // Models write `key=value`, `key = value`, `key= value` and `key =value`.
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        let mut pairs = Vec::new();
        let mut index = 0;
        while index < tokens.len() {
            let token = tokens[index];
            let next = tokens.get(index + 1).copied();
            let after = tokens.get(index + 2).copied();
            let (key, value, used) = match token.split_once('=') {
                Some((key, "")) if !key.is_empty() => (key, next.unwrap_or(""), 2),
                Some((key, value)) if !key.is_empty() => (key, value, 1),
                _ => match next {
                    Some("=") => (token, after.unwrap_or(""), 3),
                    Some(next) if next.starts_with('=') => (token, &next[1..], 2),
                    _ => (token, "", 1),
                },
            };
            pairs.push((key.to_ascii_lowercase(), value.to_owned()));
            index += used;
        }
        Self { pairs }
    }

    fn take(
        &self,
        verb: &str,
        key: &str,
        (low, high, default): (f32, f32, f32),
        line: usize,
        errors: &mut Vec<String>,
        notes: &mut Vec<String>,
    ) -> f32 {
        let Some((_, raw)) = self.pairs.iter().find(|(name, _)| name == key) else {
            notes.push(format!("{verb}: no {key} given, used {default}"));
            return default;
        };
        let Some(value) = parse_number(key, raw, line, errors) else {
            return default;
        };
        let clamped = value.clamp(low, high);
        if clamped != value {
            notes.push(format!(
                "{verb}: {key} {value} is outside {low}–{high}, clamped to {clamped}"
            ));
        }
        clamped
    }

    fn unknown(&self, verb: &str, known: &[&str], line: usize, errors: &mut Vec<String>) {
        for (key, _) in &self.pairs {
            if !known.contains(&key.as_str()) {
                errors.push(format!(
                    "line {line}: {verb} has no `{key}`; it takes {}",
                    known.join(", ")
                ));
            }
        }
    }
}

/// Check one rune file against `loc` LOC at tier I.
pub(crate) fn check(raw: &str, loc: u32) -> Result<Checked, Rejected> {
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    if raw.len() > MAX_BYTES {
        return Err(Rejected {
            errors: vec![format!("the file is over {MAX_BYTES} bytes")],
        });
    }
    let text = rune_text(raw);
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > MAX_LINES {
        errors.push(format!("the file has over {MAX_LINES} lines"));
    }
    let mut name = None;
    let mut look = None;
    let mut bolt = None;
    let mut spread = None;
    let mut melee = None;
    let mut throw = None;
    let mut cost = 0;
    for (index, line) in lines.iter().enumerate() {
        let at = index + 1;
        let line = line.trim().trim_start_matches(['-', '*']).trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.chars().count() > MAX_LINE {
            errors.push(format!("line {at}: longer than {MAX_LINE} characters"));
            continue;
        }
        let (verb, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let verb = verb.to_ascii_lowercase();
        let rest = rest.trim();
        match verb.as_str() {
            "name" => {
                let text: String = rest.chars().filter(|c| !c.is_control()).take(32).collect();
                if text.trim().is_empty() {
                    errors.push(format!("line {at}: name needs some text"));
                }
                name = Some(text.trim().to_owned());
            }
            "look" => {
                let parts: Vec<String> = rest
                    .split_whitespace()
                    .map(str::to_ascii_lowercase)
                    .collect();
                match parts.as_slice() {
                    [colour, shape]
                        if COLOURS.contains(&colour.as_str())
                            && SHAPES.contains(&shape.as_str()) =>
                    {
                        look = Some((colour.clone(), shape.clone()));
                    }
                    _ => errors.push(format!(
                        "line {at}: look takes a colour ({}) and a shape ({})",
                        COLOURS.join(" "),
                        SHAPES.join(" ")
                    )),
                }
            }
            "bolt" | "spread" | "melee" | "throw" => {
                cost += 1;
                let args = Args::parse(rest);
                let duplicate = match verb.as_str() {
                    "bolt" => bolt.is_some(),
                    "spread" => spread.is_some(),
                    "throw" => throw.is_some(),
                    _ => melee.is_some(),
                };
                if duplicate {
                    errors.push(format!("line {at}: only one `{verb}` rule is allowed"));
                    continue;
                }
                match verb.as_str() {
                    "bolt" => {
                        args.unknown(
                            "bolt",
                            &["damage", "speed", "every", "range"],
                            at,
                            &mut errors,
                        );
                        let mut take = |key, bounds| {
                            args.take("bolt", key, bounds, at, &mut errors, &mut notes)
                        };
                        bolt = Some(Bolt {
                            damage: take("damage", (4.0, 100.0, 32.0)).round() as u32,
                            speed: take("speed", (8.0, 20.0, 12.0)),
                            every: take("every", (15.0, 60.0, 15.0)).round() as u32,
                            range: take("range", (6.0, 48.0, 28.0)),
                        });
                    }
                    "spread" => {
                        args.unknown("spread", &["shots", "arc"], at, &mut errors);
                        let mut take = |key, bounds| {
                            args.take("spread", key, bounds, at, &mut errors, &mut notes)
                        };
                        spread = Some(Spread {
                            shots: take("shots", (2.0, MAX_SHOTS as f32, 3.0)).round() as u32,
                            arc: take("arc", (5.0, 90.0, 30.0)),
                        });
                    }
                    "throw" => {
                        args.unknown("throw", &["damage", "range", "every"], at, &mut errors);
                        let mut take = |key, bounds| {
                            args.take("throw", key, bounds, at, &mut errors, &mut notes)
                        };
                        throw = Some(Throw {
                            damage: take("damage", (8.0, 50.0, 20.0)).round() as u32,
                            range: take("range", (4.0, 14.0, 9.0)),
                            every: take("every", (15.0, 60.0, 36.0)).round() as u32,
                        });
                    }
                    _ => {
                        args.unknown(
                            "melee",
                            &["damage", "reach", "arc", "every"],
                            at,
                            &mut errors,
                        );
                        let mut take = |key, bounds| {
                            args.take("melee", key, bounds, at, &mut errors, &mut notes)
                        };
                        melee = Some(Melee {
                            damage: take("damage", (10.0, 60.0, 30.0)).round() as u32,
                            reach: take("reach", (1.0, MAX_REACH, 1.5)),
                            arc: take("arc", (30.0, 360.0, 120.0)),
                            every: take("every", (8.0, 45.0, 18.0)).round() as u32,
                        });
                    }
                }
            }
            other => {
                if let Some((_, tier)) = LOCKED.iter().find(|(rule, _)| *rule == other) {
                    errors.push(format!(
                        "line {at}: `{other}` unlocks at tier {tier}; this raid uses tier I (bolt, spread, melee, throw)"
                    ));
                } else {
                    errors.push(format!(
                        "line {at}: `{other}` is not a rune; use name, look, bolt, spread, melee or throw"
                    ));
                }
            }
        }
    }
    if bolt.is_none() && melee.is_none() && throw.is_none() {
        errors.push("a weapon needs a `bolt`, `melee` or `throw` line".into());
    }
    if throw.is_some() && bolt.is_some() {
        errors.push("a thrown blade and a bolt don't share a hand: drop one".into());
    }
    if spread.is_some() && bolt.is_none() {
        errors.push("`spread` needs a `bolt` line to spread".into());
    }
    if cost > loc {
        errors.push(format!(
            "the runes use {cost} LOC but you hold {loc}; drop {} rule line(s)",
            cost - loc
        ));
    }
    if !errors.is_empty() {
        return Err(Rejected { errors });
    }
    let (colour, shape) = look.unwrap_or_else(|| ("steel".into(), "orb".into()));
    let mut weapon = Weapon {
        name: name
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Nameless Blade".into()),
        colour,
        shape,
        bolt,
        spread,
        melee,
        throw,
        loc: cost,
        runes: String::new(),
    };
    fit_caps(&mut weapon, &mut notes);
    weapon.runes = weapon.canonical();
    Ok(Checked { weapon, notes })
}

/// Lower the weapon until its paper damage fits the caps: every damage scaled
/// by the same factor first, then bolt damage, melee damage, slower cadences
/// and fewer shots one step at a time. Always terminates.
fn fit_caps(weapon: &mut Weapon, notes: &mut Vec<String>) {
    let before = weapon.clone();
    let scale = (DPS_CAP / weapon.dps()).min(BURST_CAP as f32 / weapon.burst().max(1) as f32);
    if scale < 1.0 {
        if let Some(bolt) = weapon.bolt.as_mut() {
            bolt.damage = ((bolt.damage as f32 * scale).floor() as u32).max(4);
        }
        if let Some(melee) = weapon.melee.as_mut() {
            melee.damage = ((melee.damage as f32 * scale).floor() as u32).max(10);
        }
    }
    while weapon.dps() > DPS_CAP || weapon.burst() > BURST_CAP {
        if let Some(bolt) = weapon.bolt.as_mut().filter(|bolt| bolt.damage > 4) {
            bolt.damage -= 1;
        } else if let Some(melee) = weapon.melee.as_mut().filter(|melee| melee.damage > 10) {
            melee.damage -= 1;
        } else if let Some(throw) = weapon.throw.as_mut().filter(|throw| throw.damage > 8) {
            throw.damage -= 1;
        } else if let Some(bolt) = weapon.bolt.as_mut().filter(|bolt| bolt.every < 60) {
            bolt.every += 1;
        } else if let Some(melee) = weapon.melee.as_mut().filter(|melee| melee.every < 45) {
            melee.every += 1;
        } else if let Some(spread) = weapon.spread.as_mut().filter(|spread| spread.shots > 2) {
            spread.shots -= 1;
        } else {
            break;
        }
    }
    let pairs = [
        (
            "bolt damage",
            before.bolt.map(|b| b.damage as f32),
            weapon.bolt.map(|b| b.damage as f32),
        ),
        (
            "melee damage",
            before.melee.map(|m| m.damage as f32),
            weapon.melee.map(|m| m.damage as f32),
        ),
        (
            "bolt every",
            before.bolt.map(|b| b.every as f32),
            weapon.bolt.map(|b| b.every as f32),
        ),
        (
            "melee every",
            before.melee.map(|m| m.every as f32),
            weapon.melee.map(|m| m.every as f32),
        ),
        (
            "spread shots",
            before.spread.map(|s| s.shots as f32),
            weapon.spread.map(|s| s.shots as f32),
        ),
    ];
    let changes: Vec<String> = pairs
        .into_iter()
        .filter_map(|(label, was, now)| match (was, now) {
            (Some(was), Some(now)) if was != now => Some(format!("{label} {was} → {now}")),
            _ => None,
        })
        .collect();
    if !changes.is_empty() {
        notes.push(format!(
            "lowered to fit the limits ({DPS_CAP} dmg/s, {BURST_CAP} per volley and swing): {}",
            changes.join(", ")
        ));
    }
}

/// The instructions a player gives their model, filled in for this knight.
pub(crate) fn rules(loc: u32, current_runes: &str) -> String {
    format!(
        "Forge a weapon for my knight in The Delve, a slow-shot co-op dungeon.
Reply with ONLY a rune file inside one code block. No explanation.

THE RUNE FILE
- One rule per line. Lines starting with # are comments.
- `name <text>` and `look <colour> <shape>` are free. Every other line costs 1 LOC.
  I hold {loc} LOC.
- colours: {colours}
- shapes: {shapes}

RULES (use bolt, melee, or both for a hybrid)
bolt   damage=4-100 speed=8-20 every=15-60 range=6-48
       A shot along my aim. every = ticks between shots (30 ticks = 1 second).
       speed and range are in arena units; the arena is 48 wide and 28 tall.
spread shots=2-{max_shots} arc=5-90
       Needs bolt. Fires that many bolts fanned across arc degrees.
melee  damage=10-60 reach=1.0-{max_reach:.1} arc=30-360 every=8-45
throw  damage=8-50 range=4-14 every=15-60   (a blade that flies out and back; not with bolt)
       A swing at the nearest enemy in reach, else along my aim.
With both bolt and melee, fire swings when an enemy is in reach and shoots otherwise.

LIMITS (counting every shot as a hit)
- damage per second = bolt damage x shots x 30 / every + melee damage x 30 / every
  must stay at or under {dps_cap}.
- one volley plus one swing (bolt damage x shots + melee damage) must stay at or under {burst_cap}.
The game clamps anything over a limit, so stay inside them to get what you design.

EXAMPLES
```
name  Ember Fan
look  ember star
bolt   damage=10 speed=12 every=15 range=30
spread shots=3 arc=30
```
```
name  Glass Halberd
look  ice blade
melee damage=34 reach=2.0 arc=140 every=16
```
```
name  Venom Choir
look  venom wave
bolt  damage=8 speed=12 every=20 range=24
melee damage=24 reach=1.8 arc=180 every=20
```

MY CURRENT WEAPON
```
{runes}
```
",
        colours = COLOURS.join(" "),
        shapes = SHAPES.join(" "),
        max_shots = MAX_SHOTS,
        max_reach = MAX_REACH,
        dps_cap = DPS_CAP,
        burst_cap = BURST_CAP,
        runes = current_runes,
    )
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_forge__tests.rs"]
mod tests;
