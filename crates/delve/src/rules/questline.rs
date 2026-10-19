//! The questlines: a mission tree per power, with a teacher in the world.
//!
//! Level 3 and above of any wish, and level III of any skill, is never
//! found, bought or wished for: the power's teacher must be found at their
//! place in the realm, and their questline walked to its end. The rule's
//! gate is the second rank — a teacher opens only to a knight already
//! holding the power's second tier, so nobody meets a teacher early by
//! accident.
//!
//! One plain text file per power, in `cockpit/assets/dungeon/questlines/`,
//! checked like cards with line-numbered errors: teacher, place, look,
//! lines, hook, steps (each with a counter and a target), trial, reward
//! and ties. Each file also carries the teacher's figure (`art` lines,
//! the cards' own ink alphabet) and where they stand (`room`, `at`).
//! Progress is kept per knight name in the realm save, beside the school's
//! rank, so an old save loads with no progress and loses nothing.
//!
//! A questline whose counters the realm does not count yet says so in its
//! file (`missing <counter>` for each) and stays unfinishable, but still
//! loads, still draws its teacher and still speaks its hook.

use super::home::Home;
use super::*;
use std::collections::BTreeSet;

/// The questline files that ship, one per power, by power id.
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
];

/// The most steps a questline may ask, and the most a counter may need.
pub const MAX_STEPS: usize = 8;
/// The figure's bounds, in ink cells (the cards' own art alphabet).
pub const ART_W: usize = 9;
pub const ART_H: usize = 7;

/// One step of a questline: a deed the realm counts, and how far it must
/// reach. Steps are walked in order; a step's counter may be shared with
/// another questline's step (the realm counts the deed, not the tree).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub counter: String,
    pub need: u32,
    pub line: String,
    /// The step's own line when it completes.
    pub say: String,
    /// A cued step: standing with the teacher serves it.
    #[serde(default)]
    pub cue: bool,
}

/// The trial: the last step, of the power itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    pub name: String,
    pub counter: String,
    pub need: u32,
    pub line: String,
}

/// One questline, whole: everything a teacher offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Questline {
    /// The power this tree teaches: the phrasebook's wish id (`fork`) or
    /// the skill bar's own id (`gas`).
    pub power: String,
    pub teacher: String,
    /// The room the teacher lives in, as the realm says it (`room`).
    pub room: String,
    /// Where they stand, in tiles (`at <x> <y>`).
    pub at: (i32, i32),
    pub place: String,
    pub look: String,
    /// The teacher's two lines: idle, and teaching (`line`, `teach`).
    pub idle: String,
    pub teach: String,
    pub hook: String,
    pub steps: Vec<Step>,
    pub trial: Trial,
    pub reward: String,
    pub title: String,
    pub keepsake: String,
    pub ties: Vec<String>,
    /// The teacher's figure, row by row, in the cards' ink alphabet.
    pub art: Vec<String>,
    /// Counters this file names that the realm does not count yet: the
    /// questline stays unfinishable until they exist.
    pub missing: Vec<String>,
    /// Parse-only: the art block is open (rows follow bare).
    #[serde(skip)]
    pub art_open: bool,
}

impl Default for Trial {
    fn default() -> Self {
        Trial {
            name: String::new(),
            counter: String::new(),
            need: 1,
            line: String::new(),
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

/// The files' errors, one per line, like the card checker's.
pub type Notes = Vec<String>;

impl Questline {
    /// Parse one questline file, plain text, one directive a line:
    /// `power teacher place room at look line teach hook step counter
    /// need say trial reward title keepsake tie missing art`. Blank lines
    /// and `#` are nothing. Every error carries its line number.
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
                // `power wish:fork 3` or `power skill:gas 3`: the kind,
                // the realm's own id, and the level it teaches (the file
                // says 3; the engine keeps kind:id).
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
                    // A step is one line, pipe-separated, like the cards'
                    // plain rows: `step Name | counter | need | say | cue`.
                    // A counter the realm does not count yet is named with
                    // `needs`, and the step says so.
                    // `step Name | desc | counter need | say | flag`
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
                    // One line, like a step: `trial Name | desc | counter need | line`.
                    let mut parts = rest.split('|').map(str::trim);
                    q.trial.name = parts.next().unwrap_or_default().to_string();
                    let _desc = parts.next().unwrap_or_default();
                    let counter = parts.next().unwrap_or_default().to_string();
                    q.trial.line = parts.next().unwrap_or_default().to_string();
                    let (word, need) = match counter.split_once(' ') {
                        Some((w, n)) => (w.trim(), n.trim()),
                        None => (counter.as_str(), "1"),
                    };
                    q.trial.counter = word.to_string();
                    q.trial.need = need.parse().unwrap_or(1);
                    if q.trial.counter.is_empty() {
                        notes.push(format!("{id}:{n}: the trial wants a counter"));
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
                    // One prose line naming the other lines it touches.
                    q.ties.push(rest.to_string());
                }
                "missing" => {
                    q.art_open = false;
                    q.missing.push(rest.to_string());
                }
                // The teacher's figure follows `art`, one row a line.
                // Rows also follow bare (no leading word): the art block.
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
        // The whole shape, checked once, plainly.
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

    /// The room the teacher lives in.
    pub fn kind(&self) -> RoomKind {
        room_of(&self.room).unwrap_or(RoomKind::Kingdom)
    }
}

/// The realm's room names, as the files say them.
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
        _ => return None,
    })
}

/// The realm's record of one knight's walking of one questline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Walk {
    /// Counters earned, by counter name (shared counters pool here too).
    #[serde(default)]
    pub counters: BTreeMap<String, u32>,
    /// Steps finished, by position from one; each stays finished.
    #[serde(default)]
    pub done: BTreeSet<u32>,
    /// The trial, finished.
    #[serde(default)]
    pub trial_done: bool,
    /// The keepsake handed over, once.
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

    /// A step's count, capped at its need.
    pub fn step_count(&self, s: &Step) -> u32 {
        self.count(&s.counter).min(s.need)
    }

    /// The first step not finished, if any: where the knight stands.
    pub fn at<'q>(&self, q: &'q Questline) -> Option<(usize, &'q Step)> {
        q.steps
            .iter()
            .enumerate()
            .find(|(i, _)| !self.done.contains(&(*i as u32 + 1)))
            .map(|(i, s)| (i + 1, s))
    }

    /// All steps done and the trial's counter at its need. A questline
    /// that waits on counters the realm does not count is never ready.
    pub fn trial_ready(&self, q: &Questline) -> bool {
        self.done.len() >= q.steps.len()
            && q.missing.is_empty()
            && self.count(&q.trial.counter) >= q.trial.need
    }

    pub fn finished(&self) -> bool {
        self.trial_done
    }
}

/// The realm's questline records: every knight by name, each questline by
/// its power id, exactly as the school keeps its ranks.
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

    /// A skill's taught level, if the skill's questline was finished: III.
    /// Nothing else is kept here — I and II come from the run itself.
    pub fn level(&self, who: &str, which: usize) -> Option<usize> {
        let word = ["gas", "fire", "mine", "wall", "shield"][which.min(4)];
        self.walk(who, &format!("skill:{word}"))
            .finished()
            .then_some(3)
    }
}

/// Every questline that ships, checked, by power id. A file that does not
/// pass its own checker is turned away with its notes (the tests hold the
/// shipped files to zero); the game still runs with the files that pass.
pub fn book() -> &'static Book {
    static BOOK: std::sync::OnceLock<Book> = std::sync::OnceLock::new();
    BOOK.get_or_init(load)
}

/// The loaded book, its files and their notes, for the tests and the
/// checker's own report.
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

/// The book of questlines: the files that passed, and why any did not.
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

#[cfg(test)]
#[path = "../../tests/rules/together_questline__tests.rs"]
mod tests;

/// One knight's whole questline record, all powers at once: what the realm
/// save keeps beside the school's standing, by knight name.
pub type Progress = Ledger;

/// The run's own questline state: whose reading is open at which teacher,
/// and counter notes queued from inside the tick, paid in at the tick's
/// safe point (the school's board shape).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// The reading knight.
    #[serde(default)]
    pub owner: Option<u32>,
    /// The teacher's power id.
    #[serde(default)]
    pub power: Option<String>,
    /// Counters counted this tick, waiting for `flush`: (knight, counter, n).
    #[serde(skip)]
    pub pending: Vec<(u32, String, u32)>,
    /// Cued steps already served this visit, so a cue counts once: the
    /// step counter's name, by knight.
    #[serde(skip)]
    pub served: BTreeSet<(u32, String)>,
}

impl Board {
    pub fn note(&mut self, id: u32, counter: &str, n: u32) {
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
    }
}

impl Run {
    /// Queue a counter note from inside a loop that holds the knights or
    /// the foes, like the school's board.
    pub fn note_quest(&mut self, id: u32, counter: &str, n: u32) {
        self.questline.note(id, counter, n);
    }

    /// E at a teacher: the knight standing within reach of a teacher in
    /// this room hears their line for where they stand — the hook, the
    /// current step and its progress, the trial, or the closing line and
    /// the keepsake. Host-authoritative, on the press only.
    pub fn tick_teachers(&mut self, inputs: &BTreeMap<u32, Input>) {
        let mut said: Vec<(u32, String)> = Vec::new();
        let seats: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        for (id, x, y) in seats {
            let Some(i) = inputs.get(&id) else {
                continue;
            };
            if !i.valid() || !i.interact {
                continue;
            }
            let kind = self.room().kind;
            // Near a teacher of this room: within two paces of their spot.
            let near = book().lines.iter().find(|q| {
                q.kind() == kind
                    && (x - q.at.0 as f32).abs() <= 4.0
                    && (y - q.at.1 as f32).abs() <= 4.0
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

    /// Pay the tick's queued counter notes into the realm's records, and
    /// finish any step or trial they complete. Host-authoritative: called
    /// once a tick, after the combat loops let go.
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
            let key = Home::school_key(hero);
            let walks = self.home.questlines.knights.entry(key).or_default();
            for (counter, n) in counts {
                // Every questline that names this counter hears it, so
                // shared counters (beans, punt, ore) pool across lines.
                for q in &book.lines {
                    if !q.steps.iter().any(|s| s.counter == counter)
                        && q.trial.counter != counter
                    {
                        continue;
                    }
                    let walk = walks.entry(q.power.clone()).or_default();
                    let was = walk.count(&counter);
                    walk.counters.insert(counter.clone(), was + n);
                    // Fold the finished steps in: a step whose counter has
                    // reached its need is done, and stays done.
                    for (i, s) in q.steps.iter().enumerate() {
                        if walk.count(&s.counter) >= s.need {
                            walk.done.insert(i as u32 + 1);
                        }
                    }
                    if walk.at(q).is_none() && q.missing.is_empty() {
                        // steps already done; the trial may now be ready
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

    /// A step is finished when its counter has reached its need: fold the
    /// finished steps in whenever the walk is read (the walk's `done` set
    /// is a cache the counters drive).
    pub fn quest_step_done(&mut self, id: u32, power: &str) {
        let Some(q) = book().get(power).cloned() else {
            return;
        };
        let Some(hero) = self.players.get(&id) else {
            return;
        };
        let key = Home::school_key(hero);
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

    /// E at a teacher: their line for where this knight stands — the hook,
    /// the current step and its progress, the trial, or the closing line
    /// and the keepsake. One line, host-authoritative.
    pub fn speak(&mut self, id: u32, power: &str) -> Option<String> {
        let q = book().get(power).cloned()?;
        let hero = self.players.get(&id)?;
        let key = Home::school_key(hero);
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
        // The rule's gate: the second rank, held.
        if !self.quest_gate(id, &q) {
            return Some(format!(
                "\"{}\" (the gift opens to the power's second rank, already burning in the hand)"
                , q.idle
            ));
        }
        // All steps done: the trial.
        if walk.at(&q).is_none() {
            let have = walk.count(&q.trial.counter);
            return Some(format!(
                "\"{} — {}/{}. {}\"",
                q.trial.name, have.min(q.trial.need), q.trial.need, q.trial.line
            ));
        }
        // A cued step: serve its cue once a visit.
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

    /// The rule's gate: the knight holds the power's second rank. For a
    /// wish, the second tier is in the deck; for a skill, its level is II.
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
            _ => false,
        }
    }

    /// The tier a wish's grant stops at: 3 needs the questline finished.
    pub fn wish_quest_finished(&self, hero: &Hero, wish: &str) -> bool {
        self.home
            .questlines
            .walk(&Home::school_key(hero), &format!("wish:{wish}"))
            .finished()
    }

    /// A skill's level III needs its questline finished.
    pub fn skill_quest_finished(&self, hero: &Hero, word: &str) -> bool {
        self.home
            .questlines
            .walk(&Home::school_key(hero), &format!("skill:{word}"))
            .finished()
    }

    /// Every teacher standing in `room` this tick, with where they stand.
    /// Teachers do not move: their questline says where they live.
    pub fn teachers_in(&self, room: RoomKind) -> Vec<(String, (i32, i32))> {
        book()
            .lines
            .iter()
            .filter(|q| q.kind() == room)
            .map(|q| (q.power.clone(), q.at))
            .collect()
    }
}

/// The skill bar's words, as the files say them.
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
