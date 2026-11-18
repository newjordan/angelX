//! The questlines: a mission tree per power, with a teacher in the world.
//!
//! Level 3 and above of any wish, and level III of any skill, is never
//! found, bought or wished for: the power's teacher must be found at their
//! place in the realm, and their questline walked to its end. The rule's
//! gate is the second level — a teacher opens only to a knight already
//! holding the power's second tier, so nobody meets a teacher early by
//! accident.
//!
//! One plain text file per power, in `cockpit/assets/dungeon/questlines/`,
//! checked like cards with line-numbered errors: teacher, place, look,
//! lines, hook, steps (each with a counter and a target), trial, reward
//! and ties. Each file also carries the teacher's figure (`art` lines,
//! the cards' own ink alphabet) and where they stand (`room`, `at`).
//! Progress is kept per knight name in the realm save, so an old save loads
//! with no progress and loses nothing.
//!
//! A questline whose counters the realm does not count yet says so in its
//! file (`missing <counter>` for each) and stays unfinishable, but still
//! loads, still draws its teacher and still speaks its hook.
//!
//! A kingdom questline (`power kingdom:<id>`) teaches no power and opens to
//! every knight. Its steps count the campaign (`campaign`): real work done
//! outside the realm, read from the campaign's board by the host and never
//! raised by anything in the game. Its progress is the campaign's, one for
//! the whole realm; each knight keeps only what the teacher has told them
//! and the keepsake handed over (`docs/HEART-QUEST.md`). Its teacher may
//! keep a realm hall of the kingdom's own (`room realm-hall`, `hall <name>`).
//!
//! A Bomrekedan questline (`power bomrekedan:<id>`, the `bom-*.txt` files)
//! teaches no power either and opens to every knight; its teacher stands on
//! the level of the fort where they work (`room BomForges`, ...). Its steps
//! are the realm's, kept per knight like a power's, and its trial may be
//! cued like a step (`cue` in the trial's fifth field): standing with the
//! teacher serves it, once a visit.

use super::campaign::Campaign;
use super::home::Home;
use super::*;
use std::collections::BTreeSet;

static FILES: &[(&str, &str)] = &[
    ("volley", include_str!("../../../../cockpit/assets/dungeon/questlines/volley.txt")),
    ("bounce", include_str!("../../../../cockpit/assets/dungeon/questlines/bounce.txt")),
    ("homing", include_str!("../../../../cockpit/assets/dungeon/questlines/homing.txt")),
    ("chain", include_str!("../../../../cockpit/assets/dungeon/questlines/chain.txt")),
    ("mend", include_str!("../../../../cockpit/assets/dungeon/questlines/mend.txt")),
    ("pierce", include_str!("../../../../cockpit/assets/dungeon/questlines/pierce.txt")),
    ("quick", include_str!("../../../../cockpit/assets/dungeon/questlines/quick.txt")),
    ("heavy", include_str!("../../../../cockpit/assets/dungeon/questlines/heavy.txt")),
    ("speed", include_str!("../../../../cockpit/assets/dungeon/questlines/speed.txt")),
    ("stout", include_str!("../../../../cockpit/assets/dungeon/questlines/stout.txt")),
    ("armor", include_str!("../../../../cockpit/assets/dungeon/questlines/armor.txt")),
    ("vamp", include_str!("../../../../cockpit/assets/dungeon/questlines/vamp.txt")),
    ("orbit", include_str!("../../../../cockpit/assets/dungeon/questlines/orbit.txt")),
    ("powder", include_str!("../../../../cockpit/assets/dungeon/questlines/powder.txt")),
    ("fart", include_str!("../../../../cockpit/assets/dungeon/questlines/fart.txt")),
    ("shots", include_str!("../../../../cockpit/assets/dungeon/questlines/shots.txt")),
    ("fork", include_str!("../../../../cockpit/assets/dungeon/questlines/fork.txt")),
    ("gas", include_str!("../../../../cockpit/assets/dungeon/questlines/gas.txt")),
    ("fire", include_str!("../../../../cockpit/assets/dungeon/questlines/fire.txt")),
    ("mine", include_str!("../../../../cockpit/assets/dungeon/questlines/mine.txt")),
    ("wall", include_str!("../../../../cockpit/assets/dungeon/questlines/wall.txt")),
    ("shield", include_str!("../../../../cockpit/assets/dungeon/questlines/shield.txt")),
    ("heart", include_str!("../../../../cockpit/assets/dungeon/questlines/heart.txt")),
    ("bom-steel", include_str!("../../../../cockpit/assets/dungeon/questlines/bom-steel.txt")),
    ("bom-still", include_str!("../../../../cockpit/assets/dungeon/questlines/bom-still.txt")),
    ("bom-erinys", include_str!("../../../../cockpit/assets/dungeon/questlines/bom-erinys.txt")),
    ("bom-ratfolk", include_str!("../../../../cockpit/assets/dungeon/questlines/bom-ratfolk.txt")),
    ("bom-kekkit", include_str!("../../../../cockpit/assets/dungeon/questlines/bom-kekkit.txt")),
    ("bom-ivy", include_str!("../../../../cockpit/assets/dungeon/questlines/bom-ivy.txt")),
    ("bom-machines", include_str!("../../../../cockpit/assets/dungeon/questlines/bom-machines.txt")),
];

pub const MAX_STEPS: usize = 8;
pub const TEACHER_REACH: f32 = 2.0;
pub const ART_W: usize = 9;
pub const ART_H: usize = 7;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub counter: String,
    pub need: u32,
    pub line: String,
    pub say: String,
    #[serde(default)]
    pub cue: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    pub name: String,
    pub counter: String,
    pub need: u32,
    pub line: String,
    #[serde(default)]
    pub cue: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Questline {
    pub power: String,
    pub teacher: String,
    pub room: String,
    pub at: (i32, i32),
    #[serde(default)]
    pub hall: String,
    pub place: String,
    pub look: String,
    pub idle: String,
    pub teach: String,
    pub hook: String,
    pub steps: Vec<Step>,
    pub trial: Trial,
    pub reward: String,
    pub title: String,
    pub keepsake: String,
    pub ties: Vec<String>,
    pub art: Vec<String>,
    pub missing: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub art_open: bool,
}

impl Default for Trial {
    fn default() -> Self {
        Trial {
            name: String::new(),
            counter: String::new(),
            need: 1,
            line: String::new(),
            cue: false,
        }
    }
}

impl Default for Questline {
    fn default() -> Self {
        Questline {
            power: String::new(),
            teacher: String::new(),
            room: String::new(),
            at: (0, 0),
            hall: String::new(),
            place: String::new(),
            look: String::new(),
            idle: String::new(),
            teach: String::new(),
            hook: String::new(),
            steps: Vec::new(),
            trial: Trial::default(),
            reward: String::new(),
            title: String::new(),
            keepsake: String::new(),
            ties: Vec::new(),
            art: Vec::new(),
            missing: Vec::new(),
            art_open: false,
        }
    }
}

pub type Notes = Vec<String>;

impl Questline {
    pub fn parse(id: &str, raw: &str) -> Result<Questline, Notes> {
        let mut notes: Notes = Vec::new();
        let mut q = Questline {
            power: id.to_string(),
            ..Default::default()
        };
        let mut step: Option<Step> = None;
        let mut trial = false;
        for (n, line) in raw.lines().enumerate() {
            let n = n + 1;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (word, rest) = match line.split_once(' ') {
                Some((w, r)) => (w, r.trim()),
                None => (line, ""),
            };
            let mut close = |step: &mut Option<Step>| {
                if let Some(s) = step.take() {
                    q.steps.push(s);
                }
            };
            match word {
                "power" => {
                    q.power = rest
                        .split(' ')
                        .next()
                        .unwrap_or_default()
                        .to_string();
                }
                "teacher" => q.teacher = rest.to_string(),
                "place" => q.place = rest.to_string(),
                "room" => {
                    if room_of(rest).is_none() {
                        notes.push(format!("{id}:{n}: the realm has no room `{rest}`"));
                    }
                    q.room = rest.to_string();
                }
                "hall" => {
                    if rest.is_empty() || rest.len() > 64 {
                        notes.push(format!("{id}:{n}: hall wants the hall's name, like `hall Lantern Hall`"));
                    }
                    q.hall = rest.to_string();
                }
                "at" => match rest.split_once(' ') {
                    Some((x, y)) if x.parse::<i32>().is_ok() && y.parse::<i32>().is_ok() => {
                        q.at = (x.parse().unwrap(), y.parse().unwrap());
                    }
                    _ => notes.push(format!("{id}:{n}: at wants two numbers, like `at 18 9`")),
                },
                "look" => q.look = rest.to_string(),
                "line" => q.idle = rest.to_string(),
                "teach" => q.teach = rest.to_string(),
                "hook" => q.hook = rest.to_string(),
                "step" => {
                    close(&mut step);
                    q.art_open = false;
                    if trial {
                        notes.push(format!("{id}:{n}: a step after the trial"));
                    }
                    let mut parts = rest.split('|').map(str::trim);
                    let name = parts.next().unwrap_or_default().to_string();
                    let _desc = parts.next().unwrap_or_default().to_string();
                    let counted = parts.next().unwrap_or_default().to_string();
                    let say = parts.next().unwrap_or_default().to_string();
                    let flag = parts.next().unwrap_or_default().to_string();
                    let (counter, need) = match counted.split_once(' ') {
                        Some((c, n)) => (c.trim().to_string(), n.trim()),
                        None => (counted.clone(), "1"),
                    };
                    let cue = flag == "cue" || flag.starts_with("cue");
                    let needs = flag == "needs" || counted.starts_with("needs ");
                    let mut missing = false;
                    if needs {
                        q.missing.push(counter.trim().to_string());
                        missing = true;
                    }
                    if counter.is_empty()
                        || !counter
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                    {
                        notes.push(format!(
                            "{id}:{n}: the step wants a counter, like `one_leg_stands`"
                        ));
                    }
                    if let Some(note) = campaign_note(&counter) {
                        notes.push(format!("{id}:{n}: {note}"));
                    }
                    let need = match need.parse::<u32>() {
                        Ok(v) if (1..10_000).contains(&v) => v,
                        _ => {
                            notes.push(format!(
                                "{id}:{n}: the step's need wants a number from 1 to 9999"
                            ));
                            1
                        }
                    };
                    let say = if missing {
                        format!("{say} (this step waits on the realm counting {counter})")
                    } else {
                        say
                    };
                    step = Some(Step {
                        name,
                        counter,
                        need,
                        line: say.clone(),
                        say,
                        cue,
                    });
                }
                "counter" => {
                    if let Some(s) = step.as_mut() {
                        s.counter = rest.to_string();
                    } else if trial {
                        q.trial.counter = rest.to_string();
                    } else {
                        notes.push(format!("{id}:{n}: counter outside a step or the trial"));
                    }
                }
                "need" => match rest.parse::<u32>() {
                    Ok(v) if (1..10_000).contains(&v) => {
                        if let Some(s) = step.as_mut() {
                            s.need = v;
                        } else if trial {
                            q.trial.need = v;
                        } else {
                            notes.push(format!("{id}:{n}: need outside a step or the trial"));
                        }
                    }
                    _ => notes.push(format!("{id}:{n}: need wants a number from 1 to 9999")),
                },
                "say" => {
                    if let Some(s) = step.as_mut() {
                        s.line = rest.to_string();
                    } else if trial {
                        q.trial.line = rest.to_string();
                    } else {
                        notes.push(format!("{id}:{n}: say outside a step or the trial"));
                    }
                }
                "trial" => {
                    close(&mut step);
                    q.art_open = false;
                    trial = true;
                    let mut parts = rest.split('|').map(str::trim);
                    q.trial.name = parts.next().unwrap_or_default().to_string();
                    let _desc = parts.next().unwrap_or_default();
                    let counter = parts.next().unwrap_or_default().to_string();
                    q.trial.line = parts.next().unwrap_or_default().to_string();
                    q.trial.cue = parts.next().unwrap_or_default().starts_with("cue");
                    let (word, need) = match counter.split_once(' ') {
                        Some((w, n)) => (w.trim(), n.trim()),
                        None => (counter.as_str(), "1"),
                    };
                    q.trial.counter = word.to_string();
                    q.trial.need = need.parse().unwrap_or(1);
                    if q.trial.counter.is_empty() {
                        notes.push(format!("{id}:{n}: the trial wants a counter"));
                    }
                    if let Some(note) = campaign_note(&q.trial.counter) {
                        notes.push(format!("{id}:{n}: {note}"));
                    }
                }
                "reward" => {
                    q.art_open = false;
                    close(&mut step);
                    q.reward = rest.to_string();
                }
                "title" => {
                    q.art_open = false;
                    q.title = rest.to_string();
                }
                "keepsake" => {
                    q.art_open = false;
                    q.keepsake = rest.to_string();
                }
                "tie" => {
                    q.art_open = false;
                    q.ties.push(rest.to_string());
                }
                "ties" => {
                    q.art_open = false;
                    q.ties.push(rest.to_string());
                }
                "missing" => {
                    q.art_open = false;
                    q.missing.push(rest.to_string());
                }
                _ if q.art_open && !word.chars().all(|c| c.is_ascii_lowercase() || c == ':') => {
                    if line.chars().count() <= ART_W && q.art.len() < ART_H {
                        q.art.push(line.to_string());
                    } else {
                        notes.push(format!(
                            "{id}:{n}: art rows are at most {ART_W} wide and {ART_H} tall"
                        ));
                    }
                }
                "art" => {
                    q.art_open = true;
                }
                _ => notes.push(format!("{id}:{n}: unknown word `{word}`")),
            }
        }
        if let Some(s) = step.take() {
            q.steps.push(s);
        }
        for what in ["teacher", "place", "room", "look", "hook", "reward", "title", "keepsake"] {
            if q.field(what).is_empty() {
                notes.push(format!("{id}: {what} is missing"));
            }
        }
        if q.idle.is_empty() {
            notes.push(format!("{id}: the teacher wants a line"));
        }
        if q.steps.is_empty() {
            notes.push(format!("{id}: a questline wants at least one step"));
        }
        if q.steps.len() > MAX_STEPS {
            notes.push(format!("{id}: at most {MAX_STEPS} steps"));
        }
        if q.trial.name.is_empty() {
            notes.push(format!("{id}: the trial is missing"));
        }
        for (i, s) in q.steps.iter().enumerate() {
            if s.counter.is_empty() {
                notes.push(format!("{id}: step {} ({}) has no counter", i + 1, s.name));
            }
        }
        if trial && q.trial.counter.is_empty() {
            notes.push(format!("{id}: the trial has no counter"));
        }
        if q.kind() == RoomKind::RealmHall && q.hall.is_empty() {
            notes.push(format!("{id}: a teacher in a realm hall wants `hall <name>`: the halls are many"));
        }
        if !q.hall.is_empty() && q.kind() != RoomKind::RealmHall {
            notes.push(format!("{id}: a hall is a realm hall's: `room realm-hall`"));
        }
        let kingdom = q.is_kingdom();
        for counter in q.steps.iter().map(|s| &s.counter).chain([&q.trial.counter]) {
            let theirs = super::campaign::reserved(counter);
            if theirs && !kingdom {
                notes.push(format!("{id}: `{counter}` is the campaign's: only a kingdom questline counts it"));
            }
            if kingdom && !theirs {
                notes.push(format!("{id}: a kingdom questline counts the campaign's counters only, not `{counter}`"));
            }
        }
        if notes.is_empty() {
            Ok(q)
        } else {
            Err(notes)
        }
    }

    fn field(&self, what: &str) -> &str {
        match what {
            "teacher" => &self.teacher,
            "place" => &self.place,
            "room" => &self.room,
            "look" => &self.look,
            "hook" => &self.hook,
            "reward" => &self.reward,
            "title" => &self.title,
            "keepsake" => &self.keepsake,
            _ => "",
        }
    }

    pub fn kind(&self) -> RoomKind {
        room_of(&self.room).unwrap_or(RoomKind::Kingdom)
    }

    pub fn is_kingdom(&self) -> bool {
        self.power.starts_with("kingdom:")
    }

    pub fn is_bomrekedan(&self) -> bool {
        self.power.starts_with("bomrekedan:")
    }
}

fn campaign_note(counter: &str) -> Option<String> {
    (super::campaign::reserved(counter) && !super::campaign::is_counter(counter)).then(|| {
        format!(
            "`{counter}` is not one of the campaign's counters ({})",
            super::campaign::COUNTERS.join(", ")
        )
    })
}

fn room_of(word: &str) -> Option<RoomKind> {
    use RoomKind::*;
    Some(match word {
        "old-watch" | "Crag" => Forest,
        "tavern" | "Tavern" => Tavern,
        "eyrie" | "Aviary" => Aviary,
        "causeway" | "Causeway" => Causeway,
        "priory" | "Priory" => Priory,
        "minehead" | "MineHead" => MineHead,
        "mummers" | "Mummers" => Mummers,
        "cliffs" | "Cliffs" => Cliffs,
        "toll-bridge" | "TollBridge" => TollBridge,
        "windmill" | "Windmill" => Windmill,
        "lists" | "Lists" => Lists,
        "threshold" | "Threshold" => Threshold,
        "hippodrome" | "Hippodrome" => Hippodrome,
        "saltworks" | "Saltworks" => Saltworks,
        "abbey" | "SunkenAbbey" => SunkenAbbey,
        "stables" | "Stables" => Stables,
        "millpond" | "Millpond" => Millpond,
        "marsh" | "Marsh" => Marsh,
        "lighthouse" | "Lighthouse" => Lighthouse,
        "drowned-archive" | "Archive" => Lair,
        "hedge-maze" | "HedgeMaze" => HedgeMaze,
        "gate" | "Gate" => Gate,
        "realm-hall" | "RealmHall" => RealmHall,
        "BomGate" | "the-gate" => BomGate,
        "BomGreatHall" | "the-great-hall" => BomGreatHall,
        "BomFarms" | "the-farms" => BomFarms,
        "BomWorkshops" | "the-workshops" => BomWorkshops,
        "BomNobleHalls" | "the-noble-halls" => BomNobleHalls,
        "BomTemples" | "the-temples" => BomTemples,
        "BomGuildhalls" | "the-guildhalls" => BomGuildhalls,
        "BomDormitories" | "the-dormitories" => BomDormitories,
        "BomBarracks" | "the-barracks" => BomBarracks,
        "BomForges" | "the-forges" => BomForges,
        _ => return None,
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Walk {
    #[serde(default)]
    pub counters: BTreeMap<String, u32>,
    #[serde(default)]
    pub done: BTreeSet<u32>,
    #[serde(default)]
    pub trial_done: bool,
    #[serde(default)]
    pub keepsake: bool,
}

impl Walk {
    pub fn valid(&self) -> bool {
        self.counters.values().all(|&n| n < 100_000) && self.done.len() <= MAX_STEPS
    }

    pub fn count(&self, counter: &str) -> u32 {
        self.counters.get(counter).copied().unwrap_or(0)
    }

    pub fn step_count(&self, s: &Step) -> u32 {
        self.count(&s.counter).min(s.need)
    }

    pub fn at<'q>(&self, q: &'q Questline) -> Option<(usize, &'q Step)> {
        q.steps
            .iter()
            .enumerate()
            .find(|(i, _)| !self.done.contains(&(*i as u32 + 1)))
            .map(|(i, s)| (i + 1, s))
    }

    pub fn trial_ready(&self, q: &Questline) -> bool {
        self.done.len() >= q.steps.len()
            && q.missing.is_empty()
            && self.count(&q.trial.counter) >= q.trial.need
    }

    pub fn finished(&self) -> bool {
        self.trial_done
    }
}

pub fn campaign_walk(q: &Questline, campaign: &Campaign) -> Walk {
    let mut walk = Walk::default();
    for counter in q.steps.iter().map(|s| &s.counter).chain([&q.trial.counter]) {
        walk.counters.insert(counter.clone(), campaign.count(counter));
    }
    for (i, s) in q.steps.iter().enumerate() {
        if walk.count(&s.counter) >= s.need {
            walk.done.insert(i as u32 + 1);
        }
    }
    walk.trial_done = walk.trial_ready(q);
    walk
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub knights: BTreeMap<String, BTreeMap<String, Walk>>,
}

impl Ledger {
    pub fn valid(&self) -> bool {
        self.knights
            .values()
            .all(|w| w.values().all(Walk::valid))
    }

    pub fn walk(&self, who: &str, power: &str) -> Walk {
        self.knights
            .get(who)
            .and_then(|w| w.get(power))
            .cloned()
            .unwrap_or_default()
    }

    pub fn level(&self, who: &str, which: usize) -> Option<usize> {
        let word = ["gas", "fire", "mine", "wall", "shield"][which.min(4)];
        self.walk(who, &format!("skill:{word}"))
            .finished()
            .then_some(3)
    }
}

pub fn book() -> &'static Book {
    static BOOK: std::sync::OnceLock<Book> = std::sync::OnceLock::new();
    BOOK.get_or_init(load)
}

pub fn load() -> Book {
    let mut lines = Vec::new();
    let mut notes = Vec::new();
    for (id, raw) in FILES {
        match Questline::parse(id, raw) {
            Ok(q) => lines.push(q),
            Err(n) => notes.extend(n),
        }
    }
    Book { lines, notes }
}

#[derive(Clone, Debug, Default)]
pub struct Book {
    pub lines: Vec<Questline>,
    pub notes: Vec<String>,
}

impl Book {
    pub fn get(&self, power: &str) -> Option<&Questline> {
        self.lines.iter().find(|q| q.power == power)
    }
}

pub fn quest_note(q: &Questline, walk: &Walk) -> String {
    if walk.finished() {
        return format!("done — {}", q.title);
    }
    match walk.at(q) {
        None => format!(
            "the trial: {} ({}/{})",
            q.trial.name,
            walk.count(&q.trial.counter).min(q.trial.need),
            q.trial.need
        ),
        Some((n, step)) => format!(
            "step {n}/{} — {} ({}/{}) at {}",
            q.steps.len(),
            step.name,
            walk.count(&step.counter).min(step.need),
            step.need,
            q.place
        ),
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_questline__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/heart_quest__tests.rs"]
mod heart_tests;

#[cfg(test)]
#[path = "../../tests/rules/bomrekedan__quests__tests.rs"]
mod bomrekedan_tests;

pub type Progress = Ledger;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    #[serde(default)]
    pub owner: Option<u32>,
    #[serde(default)]
    pub power: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub pending: Vec<(u32, String, u32)>,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub served: BTreeSet<(u32, String)>,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub held: BTreeSet<u32>,
}

impl Board {
    pub fn note(&mut self, id: u32, counter: &str, n: u32) {
        if super::campaign::reserved(counter) {
            return;
        }
        if n > 0 && self.pending.len() < 128 {
            self.pending.push((id, counter.to_string(), n));
        }
    }

    pub fn close(&mut self) {
        self.owner = None;
        self.power = None;
    }

    pub fn valid(&self) -> bool {
        self.pending.len() <= 128
            && self.owner.is_none_or(|id| (1..=MAX_PLAYERS).contains(&id))
            && self.held.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
    }
}

impl Run {
    pub fn note_quest(&mut self, id: u32, counter: &str, n: u32) {
        self.questline.note(id, counter, n);
    }

    pub fn teacher_here(&self, q: &Questline) -> bool {
        q.kind() == self.room().kind
            && (q.hall.is_empty()
                || self.kingdom.return_to.as_ref().is_some_and(|back| back.name == q.hall))
    }

    pub fn quest_walk(&self, key: &str, q: &Questline) -> Walk {
        let saved = self.home.questlines.walk(key, &q.power);
        if !q.is_kingdom() {
            return saved;
        }
        let mut walk = campaign_walk(q, &self.campaign);
        walk.keepsake = saved.keepsake;
        walk
    }

    pub fn quest_news(&self, key: &str, q: &Questline) -> bool {
        if !q.is_kingdom() {
            return false;
        }
        let told = self.home.questlines.walk(key, &q.power);
        let now = campaign_walk(q, &self.campaign);
        now.done.iter().any(|n| !told.done.contains(n)) || (now.trial_done && !told.keepsake)
    }

    pub fn quest_sheet(&self, hero: &Hero, power: &str) -> Option<String> {
        let q = book().get(power)?;
        Some(quest_note(q, &self.quest_walk(&Home::profile_key(hero), q)))
    }

    pub fn hall_door(&self, hall: &str) -> Option<(i32, i32)> {
        map::geography(&self.kingdom.wishes)
            .sites
            .iter()
            .find(|s| s.room == RoomKind::RealmHall && s.name == hall)
            .map(|s| s.stand)
    }

    pub fn tick_teachers(&mut self, inputs: &BTreeMap<u32, Input>) {
        let mut said: Vec<(u32, String)> = Vec::new();
        let down: BTreeSet<u32> = self
            .players
            .keys()
            .copied()
            .filter(|id| inputs.get(id).is_some_and(|i| i.valid() && i.interact))
            .collect();
        let held = std::mem::replace(&mut self.questline.held, down.clone());
        let seats: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(id, h)| down.contains(*id) && !held.contains(*id) && h.hp > 0 && !h.stone && h.privy == 0)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        for (id, x, y) in seats {
            let near = book().lines.iter().find(|q| {
                self.teacher_here(q)
                    && (x / TILE_UNITS - q.at.0 as f32).abs() <= TEACHER_REACH
                    && (y / TILE_UNITS - q.at.1 as f32).abs() <= TEACHER_REACH
            });
            let Some(q) = near else {
                continue;
            };
            if let Some(line) = self.speak(id, &q.power.clone()) {
                said.push((id, line));
            }
        }
        for (id, line) in said {
            self.found = Some((self.tick, id, line));
        }
    }

    pub fn flush_questlines(&mut self) {
        let pending = std::mem::take(&mut self.questline.pending);
        if pending.is_empty() {
            return;
        }
        let book = book();
        let mut by_knight: BTreeMap<u32, BTreeMap<String, u32>> = BTreeMap::new();
        for (id, counter, n) in &pending {
            *by_knight
                .entry(*id)
                .or_default()
                .entry(counter.clone())
                .or_insert(0) += n;
        }
        let mut finished: Vec<(u32, String)> = Vec::new();
        for (id, counts) in by_knight {
            let Some(hero) = self.players.get(&id) else {
                continue;
            };
            let key = Home::profile_key(hero);
            let walks = self.home.questlines.knights.entry(key).or_default();
            for (counter, n) in counts {
                for q in &book.lines {
                    if q.is_kingdom() {
                        continue;
                    }
                    if !q.steps.iter().any(|s| s.counter == counter)
                        && q.trial.counter != counter
                    {
                        continue;
                    }
                    let walk = walks.entry(q.power.clone()).or_default();
                    let was = walk.count(&counter);
                    walk.counters.insert(counter.clone(), was + n);
                    for (i, s) in q.steps.iter().enumerate() {
                        if walk.count(&s.counter) >= s.need {
                            walk.done.insert(i as u32 + 1);
                        }
                    }
                    if walk.at(q).is_none() && q.missing.is_empty() {
                    }
                    if walk.trial_ready(q) && !walk.trial_done {
                        walk.trial_done = true;
                        finished.push((id, q.power.clone()));
                    }
                }
            }
        }
        for (id, power) in finished {
            if let Some(q) = book.get(&power) {
                self.found = Some((
                    self.tick,
                    id,
                    format!(
                        "THE {} QUESTLINE IS WALKED TO ITS END — {}'s trial is passed",
                        q.teacher.to_uppercase(),
                        q.trial.name
                    ),
                ));
            }
        }
    }

    pub fn quest_step_done(&mut self, id: u32, power: &str) {
        let Some(q) = book().get(power).cloned().filter(|q| !q.is_kingdom()) else {
            return;
        };
        let Some(hero) = self.players.get(&id) else {
            return;
        };
        let key = Home::profile_key(hero);
        let Some(walk) = self.home.questlines.knights.get_mut(&key) else {
            return;
        };
        let Some(walk) = walk.get_mut(power) else {
            return;
        };
        for (i, s) in q.steps.iter().enumerate() {
            if !walk.done.contains(&(i as u32 + 1)) && walk.count(&s.counter) >= s.need {
                walk.done.insert(i as u32 + 1);
            }
        }
    }

    pub fn speak(&mut self, id: u32, power: &str) -> Option<String> {
        let q = book().get(power).cloned()?;
        if q.is_kingdom() {
            return self.speak_kingdom(id, &q);
        }
        let hero = self.players.get(&id)?;
        let key = Home::profile_key(hero);
        let walk = self.home.questlines.walk(&key, power);
        if walk.finished() {
            let keepsake = if walk.keepsake {
                String::new()
            } else {
                format!(" They press {} into your hand.", q.keepsake)
            };
            if !walk.keepsake {
                let walks = self
                    .home
                    .questlines
                    .knights
                    .entry(key)
                    .or_default()
                    .entry(power.to_string())
                    .or_default();
                walks.keepsake = true;
            }
            return Some(format!(
                "\"{} — the line is walked.{}\"{}",
                q.title, keepsake,
                if keepsake.is_empty() { "" } else { "" }
            ));
        }
        if !self.quest_gate(id, &q) {
            return Some(format!(
                "\"{}\" (the gift opens to the power's second level, already burning in the hand)"
                , q.idle
            ));
        }
        if walk.at(&q).is_none() {
            let have = walk.count(&q.trial.counter);
            let trial_key = (id, q.trial.counter.clone());
            if q.trial.cue && q.missing.is_empty() && !self.questline.served.contains(&trial_key) {
                self.questline.served.insert(trial_key);
                self.questline.note(id, &q.trial.counter, 1);
                self.flush_questlines();
                return Some(format!(
                    "\"{} — {}/{}. {}\"",
                    q.trial.name, (have + 1).min(q.trial.need), q.trial.need, q.trial.line
                ));
            }
            return Some(format!(
                "\"{} — {}/{}. {}\"",
                q.trial.name, have.min(q.trial.need), q.trial.need, q.trial.line
            ));
        }
        let Some((_n, step)) = walk.at(&q) else {
            return None;
        };
        let have = walk.step_count(step).min(step.need);
        if step.cue && !self.questline.served.contains(&(id, step.counter.clone())) {
            self.questline.served.insert((id, step.counter.clone()));
            self.questline.note(id, &step.counter, 1);
            self.flush_questlines();
            return Some(format!(
                "\"{} — {}/{}. {}\"",
                step.name, have.min(step.need) + 1, step.need, step.say
            ));
        }
        Some(format!(
            "\"{} — {}/{}. {}\"",
            step.name, have, step.need, step.say
        ))
    }

    fn speak_kingdom(&mut self, id: u32, q: &Questline) -> Option<String> {
        let key = Home::profile_key(self.players.get(&id)?);
        let now = campaign_walk(q, &self.campaign);
        let walks = self.home.questlines.knights.entry(key).or_default();
        let met = walks.contains_key(&q.power);
        let told = walks.entry(q.power.clone()).or_default();
        if !met {
            return Some(q.hook.clone());
        }
        if let Some(&n) = now.done.iter().find(|n| !told.done.contains(n)) {
            told.done.insert(n);
            let step = &q.steps[n as usize - 1];
            return Some(format!("\"{} — {}/{}. {}\"", step.name, step.need, step.need, step.say));
        }
        if now.trial_done {
            told.trial_done = true;
            let first = !told.keepsake;
            told.keepsake = true;
            return Some(if first {
                format!(
                    "\"{} — {}/{}. {}\" {} presses {} into your hand: you are {}.",
                    q.trial.name, q.trial.need, q.trial.need, q.trial.line, q.teacher, q.keepsake, q.title
                )
            } else {
                format!("\"{} — the line is walked. {}\"", q.title, q.teach)
            });
        }
        Some(match now.at(q) {
            Some((_, step)) => format!(
                "\"{} — {}/{}. {}\"",
                step.name,
                now.step_count(step),
                step.need,
                step.line
            ),
            None => format!(
                "\"{} — {}/{}. {}\"",
                q.trial.name,
                now.count(&q.trial.counter).min(q.trial.need),
                q.trial.need,
                q.trial.line
            ),
        })
    }

    pub fn quest_gate(&self, id: u32, q: &Questline) -> bool {
        let Some(hero) = self.players.get(&id) else {
            return false;
        };
        match q.power.split_once(':') {
            Some(("wish", wish)) => crate::rules::phrasebook::book()
                .iter()
                .find(|w| &w.id == wish)
                .is_some_and(|w| {
                    (0..w.tiers.len()).rev().any(|t| {
                        t >= 1 && hero.deck.contains(&w.card_id(t))
                    })
                }),
            Some(("skill", word)) => {
                let which = skill_of(word).unwrap_or(usize::MAX);
                which != usize::MAX && self.skill_level(id, which) >= 2
            }
            Some(("kingdom", _)) => true,
            Some(("bomrekedan", _)) => true,
            _ => false,
        }
    }

}

pub fn skill_of(word: &str) -> Option<usize> {
    Some(match word {
        "gas" => skills::GAS,
        "fire" => skills::FIRE,
        "mine" => skills::MINE,
        "wall" => skills::WALL,
        "shield" => skills::SHIELD,
        _ => return None,
    })
}
