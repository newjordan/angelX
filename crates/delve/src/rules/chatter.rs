//! The knights' chatter: nobody narrates, the knights make their own noises.
//! At the big moments one of them says a few words of their made-up tongue,
//! and each grunts as they roll, take a hit, fall or loose an ultimate.
//!
//! Every noise is a named effect (`knights/<voice>_<clip>_<take>`, recorded
//! by `assets/dungeon/knights.py`), so the host, joined friends and browser
//! seats all hear the same knight say the same thing. A knight's voice
//! follows their seat: the same knight always sounds the same.
//!
//! It is meant to stay small and rare. One knight speaks at a time, an
//! ordinary moment waits out a long quiet, a big one only a breath, and each
//! knight's grunts keep their distance; a fall always sounds.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::{HZ, Run};

/// The knights' four voices, taken by seat.
const VOICES: [&str; 4] = ["a", "b", "c", "d"];
/// What each voice can say: chatter for moments, grunts for the body.
const CLIPS: [&str; 12] = [
    "cheer", "ooh", "uhoh", "aww", "thanks", "go", "hello", "hehe", "hup", "oof", "hyah", "waah",
];
/// Takes of every clip.
const TAKES: usize = 2;

/// After any chatter, an ordinary moment waits this long for a word.
pub const QUIET: u64 = 25 * HZ as u64;
/// Even big moments leave a breath between two lines.
pub const BREATH: u64 = 3 * HZ as u64;
/// The party grunts at most this often, falls aside: one knight at a time.
const GRUNT_GAP: u64 = 4 * HZ as u64;
/// A knight's "oof" at most this often, however hard the fight.
const OOF_GAP: u64 = 6 * HZ as u64;
/// A knight's "hup" as they roll, at most this often.
const HUP_GAP: u64 = 12 * HZ as u64;

/// Every knight clip's effect name, for the mixer and the guest page.
pub fn names() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = Vec::new();
        for voice in VOICES {
            for clip in CLIPS {
                for take in 1..=TAKES {
                    let name = format!("knights/{voice}_{clip}_{take}");
                    names.push(&*Box::leak(name.into_boxed_str()));
                }
            }
        }
        names
    })
}

/// Knight `id`'s take of `clip`.
pub fn clip(id: u32, clip: &str, take: usize) -> &'static str {
    let voice = id.saturating_sub(1) as usize % VOICES.len();
    let at = CLIPS.iter().position(|c| *c == clip).unwrap_or(0);
    names()[(voice * CLIPS.len() + at) * TAKES + take % TAKES]
}

/// What the knights say about a moment, and whether it is big enough to
/// speak after only a breath.
fn mood(cue: &str) -> Option<(&'static str, bool)> {
    let base = cue.split(':').next().unwrap_or(cue);
    Some(match base {
        "victory" | "boss_fall" | "outlaw_fall" | "grail" | "bounty_paid" | "joust_won" => {
            ("cheer", true)
        }
        "wipe" | "knight_down" => ("aww", true),
        "boss_rise" | "lair" | "outlaw_rise" | "mimic" => ("uhoh", true),
        "revive" | "second_wind" => ("thanks", true),
        "run_start" | "descend" | "the_deep" => ("go", true),
        "pastime_won" | "goblin_caught" | "dare_kept" | "flawless" | "slay4" => ("hehe", false),
        "card_relic" | "card_rare" | "secret_found" | "key_drop" | "wish_granted"
        | "achievement" | "trophy_new" | "box_opened" | "level_up" => ("ooh", false),
        "low_hp" | "wave_last" | "collapse_warn" | "trap_spikes" | "trap_rocks" | "trap_fire"
        | "trap_axes" | "trap_hammers" | "trap_boulder" | "trap_saw" | "gorge_fall" => {
            ("uhoh", false)
        }
        "home" | "built" | "rescued" | "hired" | "brannoc_arrives" | "npc" => ("hello", false),
        "pastime_lost" | "key_lost" | "dare_broken" | "joust_lost" | "goblin_escaped" => {
            ("aww", false)
        }
        _ => return None,
    })
}

/// A knight as the last tick left them.
#[derive(Clone, Copy, Debug, Default)]
struct Body {
    hp: u32,
    dash: u32,
    charge: u32,
}

/// Who spoke when, whose turn it is, and each knight's last grunts.
#[derive(Clone, Debug, Default)]
pub struct Chatter {
    said: Option<u64>,
    turn: usize,
    grunted: Option<u64>,
    bodies: BTreeMap<u32, Body>,
    oof: BTreeMap<u32, u64>,
    hup: BTreeMap<u32, u64>,
}

impl Run {
    /// Once a tick, after the tick: the knights' grunts, then a word about
    /// the biggest moment the tick cued (from cue `from` on), if the quiet
    /// allows one.
    pub(super) fn chatter(&mut self, from: usize) {
        let tick = self.tick;
        let take = (tick % TAKES as u64) as usize;
        let mut noises = Vec::new();
        let mut quiet = self.chatter.grunted.is_none_or(|at| tick >= at + GRUNT_GAP);
        for (&id, hero) in &self.players {
            let now = Body {
                hp: hero.hp,
                dash: hero.dash_ticks,
                charge: hero.ult_charge,
            };
            let Some(was) = self.chatter.bodies.insert(id, now) else {
                continue;
            };
            let free = |last: Option<&u64>, gap| last.is_none_or(|&at| tick >= at + gap);
            let grunt = if was.hp > 0 && now.hp == 0 {
                noises.push(clip(id, "waah", take));
                continue;
            } else if !quiet {
                continue;
            } else if now.hp < was.hp && free(self.chatter.oof.get(&id), OOF_GAP) {
                self.chatter.oof.insert(id, tick);
                "oof"
            } else if was.charge >= super::ults::ULT_FULL && now.charge == 0 {
                "hyah"
            } else if now.dash > was.dash && free(self.chatter.hup.get(&id), HUP_GAP) {
                self.chatter.hup.insert(id, tick);
                "hup"
            } else {
                continue;
            };
            noises.push(clip(id, grunt, take));
            self.chatter.grunted = Some(tick);
            quiet = false;
        }
        self.chatter
            .bodies
            .retain(|id, _| self.players.contains_key(id));
        self.sounds.extend(noises);

        let from = from.min(self.cues.len());
        let moods: Vec<_> = self.cues[from..]
            .iter()
            .filter_map(|cue| mood(cue))
            .collect();
        let Some(&(clip_name, big)) = moods.iter().find(|(_, big)| *big).or(moods.first()) else {
            return;
        };
        let rest = if big { BREATH } else { QUIET };
        if self.chatter.said.is_some_and(|at| tick < at + rest) {
            return;
        }
        let standing: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0)
            .map(|(&id, _)| id)
            .collect();
        let speakers = if standing.is_empty() {
            self.players.keys().copied().collect()
        } else {
            standing
        };
        let Some(&id) = speakers.get(self.chatter.turn % speakers.len().max(1)) else {
            return;
        };
        let take = self.chatter.turn / speakers.len() % TAKES;
        self.chatter.turn += 1;
        self.chatter.said = Some(tick);
        self.sounds.push(clip(id, clip_name, take));
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_chatter__tests.rs"]
mod tests;
