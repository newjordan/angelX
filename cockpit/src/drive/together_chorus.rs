//! The Delve's dialogue: what a character says when the knights do something
//! with them, shown as a subtitle. Nobody speaks aloud and nobody narrates
//! the fight; the knights make their own noises (`together_shooter::chatter`).
//!
//! The script is `assets/dungeon/voices/chorus.txt` (`who | cue | id | words`).
//! A cue picks its own line when one exists (`npc:grubbins`), else the
//! general one (`npc`), rotating through every speaker who has a line for
//! it. Cooldowns keep a busy room from turning into chatter; a weightier
//! moment may cut a line short, a lighter one is simply let go.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

const SCRIPT: &str = include_str!("../../assets/dungeon/voices/chorus.txt");

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Line {
    pub(crate) who: String,
    pub(crate) cue: String,
    pub(crate) id: String,
    pub(crate) words: String,
}

/// A character's name as the subtitle shows it.
pub(crate) fn name(who: &str) -> &str {
    match who {
        "blaise" => "Old Blaise",
        "wren" => "Wren",
        "tobbin" => "Tobbin",
        "fortune" => "Dame Fortune",
        "grubbins" => "Grubbins",
        "snibbet" => "Snibbet",
        "dinadan" => "Sir Dinadan",
        "merlin" => "Merlin",
        "kay" => "Sir Kay",
        "ector" => "Sir Ector",
        "mabel" => "Mabel",
        "anselm" => "Brother Anselm",
        "pip" => "Pip",
        "maud" => "Maud",
        "tallow" => "Lady Tallow",
        "beaumains" => "Beaumains",
        "groom" => "Wat the groom",
        "palamedes" => "Sir Palamedes",
        "lancelot" => "Sir Lancelot",
        "brannoc" => "King Brannoc",
        other => other,
    }
}

/// Whose name the subtitle shows (`name`), read back: the character's id,
/// for their colour. A name nobody has is its own.
pub(crate) fn who(shown: &str) -> &str {
    script_names().find(|&w| name(w) == shown).unwrap_or(shown)
}

/// Every character with a line in the script.
fn script_names() -> impl Iterator<Item = &'static str> {
    SCRIPT
        .lines()
        .filter_map(|row| row.trim().split('|').next())
        .map(str::trim)
        .filter(|w| !w.is_empty() && !w.starts_with('#'))
}

pub(crate) fn script() -> Vec<Line> {
    SCRIPT
        .lines()
        .map(str::trim)
        .filter(|row| !row.is_empty() && !row.starts_with('#'))
        .filter_map(|row| {
            let mut parts = row.splitn(4, '|').map(str::trim);
            Some(Line {
                who: parts.next()?.into(),
                cue: parts.next()?.into(),
                id: parts.next()?.into(),
                words: parts.next()?.into(),
            })
        })
        .collect()
}

fn base(cue: &str) -> &str {
    cue.split(':').next().unwrap_or(cue)
}

/// How much a moment matters: a higher one may cut a lower one short.
fn priority(cue: &str) -> u8 {
    match base(cue) {
        "wheel" | "sanctuary" | "brannoc_arrives" | "brannoc_sworn" | "forge_relit" => 8,
        "snibbet" | "mission_done" | "work_done" => 7,
        "joust_start" | "joust_won" | "joust_lost" => 7,
        "mission" | "commissioned" | "dare" | "wheel_spin" | "caged" | "rescued" => 6,
        "trophy_new" | "lesson_learned" | "hire_asked" | "song_asked" | "built" => 6,
        "round_poured" => 6,
        "work_stage" | "song" | "grubbins_sold" | "stable_select" | "stable_tend" => 5,
        "reforged" => 5,
        "tribute" | "cant_afford" | "bar" => 4,
        "npc" | "merlin" | "wheel_again" => 3,
        _ => 2,
    }
}

/// The shortest gap between two lines for the same kind of moment.
fn cooldown(cue: &str) -> Duration {
    Duration::from_secs(match base(cue) {
        "npc" => 12,
        "wheel_again" => 8,
        "cant_afford" => 6,
        _ => 0,
    })
}

/// What is being said now, for the subtitle and joined friends.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Said {
    pub(crate) line: Line,
    pub(crate) seq: u64,
    pub(crate) until: Instant,
}

pub(crate) struct Chorus {
    lines: Vec<Line>,
    turns: HashMap<String, usize>,
    last: HashMap<String, Instant>,
    queue: VecDeque<(Line, u8)>,
    speaking: Option<(Said, u8)>,
    seq: u64,
}

impl Default for Chorus {
    fn default() -> Self {
        Chorus {
            lines: script(),
            turns: HashMap::new(),
            last: HashMap::new(),
            queue: VecDeque::new(),
            speaking: None,
            seq: 0,
        }
    }
}

impl Chorus {
    /// The line a moment would speak next, without speaking it.
    fn pick(&mut self, cue: &str) -> Option<Line> {
        let exact: Vec<&Line> = self.lines.iter().filter(|l| l.cue == cue).collect();
        let pool: Vec<&Line> = if exact.is_empty() {
            self.lines.iter().filter(|l| l.cue == base(cue)).collect()
        } else {
            exact
        };
        if pool.is_empty() {
            return None;
        }
        let turn = self.turns.entry(cue.to_string()).or_insert(0);
        let line = pool[*turn % pool.len()].clone();
        *turn += 1;
        Some(line)
    }

    /// A moment happened. Queues what is said about it, if anything.
    pub(crate) fn cue(&mut self, cue: &str, now: Instant) {
        let rank = priority(cue);
        if self
            .last
            .get(base(cue))
            .is_some_and(|at| now.saturating_duration_since(*at) < cooldown(cue))
        {
            return;
        }
        // A busy subtitle lets small moments pass rather than pile up.
        let busy = self.speaking.as_ref().is_some_and(|(s, _)| now < s.until);
        if (busy || !self.queue.is_empty()) && rank <= 4 {
            return;
        }
        if self.queue.iter().any(|(_, r)| *r > rank) {
            return;
        }
        if busy && self.speaking.as_ref().is_some_and(|(_, r)| *r < rank) {
            // A bigger moment cuts in: what was queued is dropped.
            self.queue.clear();
            if let Some((said, _)) = self.speaking.as_mut() {
                said.until = now;
            }
        }
        self.last.insert(base(cue).to_string(), now);
        if let Some(line) = self.pick(cue).filter(|_| self.queue.len() < 3) {
            self.queue.push_back((line, rank));
        }
    }

    /// Show a given line now, ahead of anything queued: a joined friend's
    /// angelX shows the host's dialogue as it happens there.
    pub(crate) fn say(&mut self, line: Line, now: Instant) {
        self.queue.clear();
        if let Some((said, _)) = self.speaking.as_mut() {
            said.until = now;
        }
        self.queue.push_back((line, 9));
    }

    /// Start the next queued line when the current one is done. Returns the
    /// line that just started, for joined friends.
    pub(crate) fn tick(&mut self, now: Instant) -> Option<Said> {
        if self.speaking.as_ref().is_some_and(|(s, _)| now < s.until) {
            return None;
        }
        let (line, rank) = self.queue.pop_front()?;
        // A subtitle reads at about 14 characters a second.
        let seconds = line.words.chars().count() as f32 / 14.0 + 1.0;
        self.seq += 1;
        let said = Said {
            line,
            seq: self.seq,
            until: now + Duration::from_secs_f32(seconds + 0.35),
        };
        self.speaking = Some((said.clone(), rank));
        Some(said)
    }

    /// The subtitle to show now: who is speaking and what, held a moment after.
    pub(crate) fn subtitle(&self, now: Instant) -> Option<&Line> {
        self.speaking
            .as_ref()
            .filter(|(s, _)| now < s.until + Duration::from_secs(2))
            .map(|(s, _)| &s.line)
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_chorus__tests.rs"]
mod tests;
