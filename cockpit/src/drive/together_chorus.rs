//! The Delve's chorus: the Herald, Old Blaise, Wren, Tobbin, the Shoggoth and
//! the guardians, speaking at the moments the game marks with cues.
//!
//! The script is `assets/dungeon/voices/chorus.txt` (`who | cue | id | words`),
//! recorded by `voice.py` into `<who>/<id>.mp3`. A line without a recording
//! still shows as a subtitle, so anyone can add lines.
//!
//! A cue picks its own line when one exists (`boss_rise:cinderjaw`), else the
//! general one (`boss_rise`), rotating through every speaker who has a line
//! for it. Big moments let the Herald call it and one other voice answer.
//! Cooldowns keep busy rooms from turning into chatter; a higher-priority
//! moment may interrupt the queue, a lower one is simply let go.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
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
        "herald" => "The Herald",
        "blaise" => "Old Blaise",
        "wren" => "Wren",
        "tobbin" => "Tobbin",
        "shoggoth" => "The Shoggoth",
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
        "warden" => "The Waxen Warden",
        "cinderjaw" => "Cinderjaw",
        "leviathan" => "The Late-Fee Leviathan",
        "beaumains" => "Beaumains",
        "groom" => "Wat the groom",
        "palamedes" => "Sir Palamedes",
        "lancelot" => "Sir Lancelot",
        "brannoc" => "King Brannoc",
        other => other,
    }
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
    // A small secret's card is a find, not a vault in the wall.
    if cue.starts_with("secret_found:") {
        return 6;
    }
    match base(cue) {
        "victory" | "wipe" | "grail" => 9,
        "the_deep" | "homeward" => 8,
        "boss_rise" | "boss_fall" | "lair" | "mimic" | "sanctuary" => 8,
        "wave_last" | "mimic_fall" => 7,
        "wave" => 6,
        "great_hall" => 5,
        "trap_spikes" | "trap_rocks" | "trap_fire" => 5,
        "boss_rage" | "descend" | "wish_raised" => 7,
        "slay4" | "slay3" | "knight_down" | "card_relic" => 6,
        "slay2" | "first_blood" | "flawless" | "revive" | "run_start" => 5,
        "card_arm" | "card_rare" | "low_hp" | "treasury_ready" => 4,
        "wish_granted" => 6,
        "second_wind" => 7,
        "wheel" => 8,
        "ult" => 7,
        "goblin" | "goblin_escaped" | "goblin_caught" => 6,
        "achievement" => 6,
        "grubbins_sold" => 5,
        "box_opened" => 6,
        "fan_box" => 6,
        "bounty_paid" => 6,
        "hooked" => 7,
        "trophy_new" => 6,
        "dare" => 6,
        "dare_kept" => 7,
        "dare_broken" => 5,
        "pit_rise" | "pit_fall" | "talisman_used" => 8,
        "talisman" => 7,
        "tallow" => 5,
        "caged" | "rescued" => 6,
        "rune" => 6,
        "rune_up" => 5,
        "all_random" => 6,
        "hexed" => 6,
        "secret_found" => 8,
        "crack_seen" | "secret_hint" => 5,
        "snibbet" => 7,
        "merlin" => 3,
        "dug" => 7,
        "bar" => 4,
        "tavern_dry" => 4,
        "siege_worthy" => 8,
        "siege_wanting" => 6,
        "round_poured" => 6,
        "round_drunk" => 5,
        "rimeleap" => 6,
        "frost_leap" => 4,
        "ravage" => 6,
        "ravaged" => 5,
        "hire_asked" | "hired" => 6,
        "black_hole" => 7,
        "channel_broken" => 6,
        "beaumains_down" => 5,
        "beaumains_up" => 4,
        "blink" | "sceptre" | "censer" => 5,
        "song" => 5,
        "song_asked" => 6,
        "rescue_home" => 6,
        "tallow_hiss" | "tallow_fetch" => 3,
        "pit_slam" => 4,
        "level_up" | "lesson_learned" => 6,
        "brood_hatch" | "hook_thrown" => 4,
        "audience_million" | "audience_prime" => 7,
        "fan_box_open" => 4,
        "necro_raise" | "shaman_wards" | "boar_dazed" => 4,
        "collapse" => 8,
        "collapse_warn" => 7,
        "wheel_spin" => 6,
        "wheel_again" => 3,
        "built" => 6,
        "home" => 5,
        "joust_unhorse" | "joust_fell" => 8,
        "joust_start" | "joust_won" | "joust_lost" | "joust_drawn" => 7,
        "joust_smite" | "joust_broke" | "joust_clean" | "joust_shield" | "joust_miss" => 6,
        "joust_spur" | "stable_select" | "stable_tend" | "the_gate" => 5,
        "entrance" => 4,
        "brannoc_arrives" | "brannoc_sworn" | "forge_relit" => 8,
        "mission_done" | "work_done" => 7,
        "mission" | "commissioned" => 6,
        "work_stage" => 5,
        "tribute" => 4,
        "cant_afford" => 4,
        "npc" => 3,
        "bond" | "banked" | "room_clear" => 3,
        _ => 2,
    }
}

/// The shortest gap between two lines for the same kind of moment.
fn cooldown(cue: &str) -> Duration {
    Duration::from_secs(match base(cue) {
        "room_fight" => 40,
        "room_clear" => 30,
        "bond" | "banked" => 45,
        "card_rare" | "card_arm" => 12,
        "low_hp" => 20,
        "wave" => 10,
        "great_hall" => 60,
        "trap_spikes" | "trap_rocks" | "trap_fire" => 150,
        "slay2" => 6,
        "npc" => 12,
        "wheel_again" => 8,
        "necro_raise" | "shaman_wards" | "boar_dazed" => 25,
        "cant_afford" => 6,
        "fan_box_open" => 20,
        "brood_hatch" => 25,
        "hook_thrown" => 30,
        "pit_slam" => 30,
        "tallow_hiss" => 20,
        "tallow_fetch" => 30,
        "rune_up" => 20,
        "hexed" => 12,
        "crack_seen" => 60,
        "rimeleap" => 15,
        "frost_leap" => 20,
        "ravage" => 15,
        "ravaged" => 20,
        "beaumains_down" | "beaumains_up" => 30,
        "black_hole" | "channel_broken" => 15,
        "blink" | "censer" => 10,
        _ => 0,
    })
}

/// Moments big enough for the Herald to call and another voice to answer.
/// A vault is one; a small secret's card (`secret_found:cup`) is not.
fn duet(cue: &str) -> bool {
    if cue.starts_with("secret_found:") {
        return false;
    }
    matches!(
        base(cue),
        "run_start"
            | "victory"
            | "wipe"
            | "boss_rise"
            | "boss_fall"
            | "lair"
            | "descend"
            | "wish_raised"
            | "mimic"
            | "sanctuary"
            | "home"
            | "built"
            | "wheel"
            | "the_deep"
            | "homeward"
            | "grail"
            | "audience_million"
            | "audience_prime"
            | "pit_rise"
            | "pit_fall"
            | "secret_found"
            | "joust_start"
            | "joust_unhorse"
            | "joust_won"
            | "brannoc_arrives"
            | "brannoc_sworn"
            | "forge_relit"
    )
}

/// What is being said now, for the subtitle and the guest page.
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
    pub(crate) muted: bool,
    /// Playback level, 0–1. Voices sit under the game, not over it.
    pub(crate) volume: f32,
    dir: Option<PathBuf>,
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
            muted: std::env::var("ANGEL_DELVE_VOICES").is_ok_and(|v| v == "0"),
            volume: crate::drive::together_audio::VOICE_LEVEL,
            dir: voices_dir(),
        }
    }
}

/// Where the recordings are: the resource checkout first, then the source tree.
pub(crate) fn voices_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .map(|d| PathBuf::from(d).join("cockpit/assets/dungeon/voices")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/dungeon/voices")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|d| d.join("chorus.txt").is_file())
}

/// The recording for a line, if it was recorded.
pub(crate) fn recording(dir: Option<&std::path::Path>, line: &Line) -> Option<PathBuf> {
    let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !ok(&line.who) || !ok(&line.id) {
        return None;
    }
    let path = dir?.join(&line.who).join(format!("{}.mp3", line.id));
    path.is_file().then_some(path)
}

impl Chorus {
    /// The line a moment would speak next, without speaking it.
    fn pick(&mut self, cue: &str, skip_herald: bool) -> Option<Line> {
        let exact: Vec<&Line> = self
            .lines
            .iter()
            .filter(|l| l.cue == cue && !(skip_herald && l.who == "herald"))
            .collect();
        let pool: Vec<&Line> = if exact.is_empty() {
            self.lines
                .iter()
                .filter(|l| l.cue == base(cue) && !(skip_herald && l.who == "herald"))
                .collect()
        } else {
            exact
        };
        if pool.is_empty() {
            return None;
        }
        let turn = self
            .turns
            .entry(format!("{cue}|{skip_herald}"))
            .or_insert(0);
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
        // A busy voice lets small moments pass rather than pile up.
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
        let mut lines = Vec::new();
        if duet(cue) {
            let herald = self
                .lines
                .iter()
                .find(|l| l.who == "herald" && (l.cue == cue || l.cue == base(cue)))
                .cloned();
            lines.extend(herald);
            lines.extend(self.pick(cue, true));
        } else {
            lines.extend(self.pick(cue, false));
        }
        for line in lines {
            if self.queue.len() < 3 {
                self.queue.push_back((line, rank));
            }
        }
    }

    /// Say a given line now, ahead of anything queued: a joined friend's
    /// angelX speaks the host's chorus as it happens there.
    pub(crate) fn say(&mut self, line: Line, now: Instant) {
        self.queue.clear();
        if let Some((said, _)) = self.speaking.as_mut() {
            said.until = now;
        }
        self.queue.push_back((line, 9));
    }

    /// Start the next queued line when the current one is done. Returns the
    /// line that just started, for the guest page.
    pub(crate) fn tick(&mut self, now: Instant) -> Option<Said> {
        if self.speaking.as_ref().is_some_and(|(s, _)| now < s.until) {
            return None;
        }
        let (line, rank) = self.queue.pop_front()?;
        let file = recording(self.dir.as_deref(), &line);
        // 64 kbit/s recordings: bytes tell the length; a subtitle reads ~14 chars/s.
        let seconds = file
            .as_ref()
            .and_then(|f| std::fs::metadata(f).ok())
            .map_or(line.words.chars().count() as f32 / 14.0 + 1.0, |m| {
                m.len() as f32 / 8000.0
            });
        if let Some(file) = file.as_ref().filter(|_| !self.muted) {
            play(file, self.volume);
        }
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

/// Play a recording without waiting for it. Tests never make a sound.
fn play(path: &std::path::Path, volume: f32) {
    if cfg!(test) {
        return;
    }
    let level = volume.clamp(0.0, 1.0);
    let pw = format!("{level:.2}");
    let pa = format!("{}", (level * 65536.0) as u32);
    let mpv = format!("--volume={}", (level * 100.0) as u32);
    // afplay: every Mac has it.
    let players: [Vec<&str>; 4] = [
        vec!["pw-play", "--volume", &pw],
        vec!["paplay", "--volume", &pa],
        vec!["mpv", "--no-video", "--really-quiet", &mpv],
        vec!["afplay", "-v", &pw],
    ];
    for player in players {
        let spawned = std::process::Command::new(player[0])
            .args(&player[1..])
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        if let Ok(mut child) = spawned {
            // Reap it off the game thread so no zombie is left behind.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return;
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_chorus__tests.rs"]
mod tests;
