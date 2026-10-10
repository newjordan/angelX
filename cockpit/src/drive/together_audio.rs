//! The Delve's sound: effects the game names, and music looping under play.
//!
//! The sounds are files in `assets/dungeon/sfx/` and `assets/dungeon/music/`
//! (see `sfx/README.md` for what each is and the levels). Effects are
//! throttled per name and capped in number, so rapid fire never piles into
//! noise; music is one looping player that is replaced or stopped, never
//! doubled. Everything plays through a helper process (`pw-play`, `mpv`,
//! `ffplay`), so a missing player only means silence. Tests make no sound.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Where the chorus's voices sit in the mix: near the music, a touch above.
pub(crate) const VOICE_LEVEL: f32 = 0.28;
/// Music runs low under play.
const MUSIC_LEVEL: f32 = 0.35;
/// Short effects cut through on their transients.
const SFX_LEVEL: f32 = 0.5;
/// Effects sounding at once, at most.
const MAX_AT_ONCE: usize = 5;
/// Effects are encoded at 96 kbit/s: bytes tell their length.
const SFX_BYTES_PER_SECOND: f32 = 12_000.0;

/// Every effect the game may name.
pub(crate) const SFX: &[&str] = &[
    "shot_bow",
    "shot_crossbow",
    "shot_handgonne",
    "shot_bolt",
    "swing",
    "hit",
    "kill",
    "hero_hurt",
    "roll",
    "shield_block",
    "bomb",
    "card_pickup",
    "spoil_pickup",
    "door_seal",
    "door_open",
    "wave_gate",
    "spike",
    "rock_land",
    "vent_fire",
    "chest_open",
    "mimic",
    "boss_rise",
    "boss_fall",
    "descend",
    "play_card",
    "wall_up",
    "mana_empty",
    "wheel_spin",
    "wheel_land",
];

/// Every music track.
pub(crate) const TRACKS: &[&str] = &["crypt", "mines", "keep", "boss", "sanctuary", "menu"];

/// The shortest gap between two of the same effect.
fn gap(name: &str) -> Duration {
    Duration::from_millis(match name {
        "shot_bow" | "shot_crossbow" | "shot_handgonne" | "shot_bolt" => 70,
        "hit" => 60,
        "kill" => 90,
        "swing" | "roll" | "shield_block" | "spike" | "vent_fire" => 150,
        "boss_rise" | "boss_fall" | "descend" | "door_seal" | "door_open" => 800,
        "wheel_spin" | "wheel_land" => 1500,
        _ => 200,
    })
}

/// The Delve's sound folder: the resource checkout first, then the source tree.
pub(crate) fn sound_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .map(|d| PathBuf::from(d).join("cockpit/assets/dungeon")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/dungeon")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|d| d.join("sfx").is_dir() || d.join("music").is_dir())
}

#[derive(Default)]
pub(crate) struct Audio {
    /// When each effect last played.
    last: HashMap<&'static str, Instant>,
    /// When the effects now sounding end.
    sounding: Vec<Instant>,
    /// The track wanted, and the player looping it.
    track: Option<&'static str>,
    player: Option<Child>,
    muted: bool,
    /// Effects started (for the tests: they make no sound).
    played: usize,
    dir: Option<PathBuf>,
    looked: bool,
}

impl Audio {
    fn dir(&mut self) -> Option<&Path> {
        if !self.looked {
            self.looked = true;
            self.dir = sound_dir();
        }
        self.dir.as_deref()
    }

    /// Play effect `name`, unless it just played or enough are sounding.
    pub(crate) fn sfx(&mut self, name: &str, now: Instant) {
        let Some(&name) = SFX.iter().find(|&&s| s == name) else {
            return;
        };
        if self.muted {
            return;
        }
        if self
            .last
            .get(name)
            .is_some_and(|&at| now.saturating_duration_since(at) < gap(name))
        {
            return;
        }
        self.sounding.retain(|&end| end > now);
        if self.sounding.len() >= MAX_AT_ONCE {
            return;
        }
        let file = self
            .dir()
            .map(|d| d.join("sfx").join(format!("{name}.mp3")));
        let seconds = file
            .as_ref()
            .and_then(|f| std::fs::metadata(f).ok())
            .map_or(0.6, |m| m.len() as f32 / SFX_BYTES_PER_SECOND);
        self.last.insert(name, now);
        self.sounding
            .push(now + Duration::from_secs_f32(seconds.min(2.0)));
        self.played += 1;
        if let Some(file) = file.filter(|f| f.is_file()) {
            play_once(&file, SFX_LEVEL);
        }
    }

    /// Loop `track` (same track: nothing changes; `None`: stop).
    pub(crate) fn music(&mut self, track: Option<&str>) {
        let track = match track {
            None => None,
            Some(name) => match TRACKS.iter().find(|&&t| t == name) {
                Some(&t) => Some(t),
                // An unknown track keeps what is playing.
                None => return,
            },
        };
        if track == self.track {
            return;
        }
        self.track = track;
        self.restart();
    }

    /// Stop the music (the game is collapsed or closed).
    pub(crate) fn stop(&mut self) {
        self.track = None;
        self.kill();
    }

    pub(crate) fn set_muted(&mut self, muted: bool) {
        if muted == self.muted {
            return;
        }
        self.muted = muted;
        self.restart();
    }

    /// The track wanted now.
    #[cfg(test)]
    pub(crate) fn track(&self) -> Option<&'static str> {
        self.track
    }

    fn restart(&mut self) {
        self.kill();
        if self.muted || cfg!(test) {
            return;
        }
        let Some(track) = self.track else {
            return;
        };
        let Some(file) = self
            .dir()
            .map(|d| d.join("music").join(format!("{track}.mp3")))
            .filter(|f| f.is_file())
        else {
            return;
        };
        self.player = play_looping(&file, MUSIC_LEVEL);
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.player.take() {
            let _ = child.kill();
            // Reap it off the game thread.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.kill();
    }
}

fn quiet(command: &mut Command) -> &mut Command {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
}

/// One effect, fire and forget.
fn play_once(path: &Path, level: f32) {
    if cfg!(test) {
        return;
    }
    let level = level.clamp(0.0, 1.0);
    let pw = format!("{level:.2}");
    let pa = format!("{}", (level * 65536.0) as u32);
    let mpv = format!("--volume={}", (level * 100.0) as u32);
    // afplay: every Mac has it.
    let players: [Vec<&str>; 4] = [
        vec!["pw-play", "--volume", &pw],
        vec!["paplay", "--volume", &pa],
        vec!["mpv", "--no-video", "--really-quiet", "--no-terminal", &mpv],
        vec!["afplay", "-v", &pw],
    ];
    for player in players {
        if let Ok(mut child) = quiet(Command::new(player[0]).args(&player[1..]).arg(path)).spawn() {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return;
        }
    }
}

/// A looping player for one track; the caller keeps it to kill it.
fn play_looping(path: &Path, level: f32) -> Option<Child> {
    let percent = ((level.clamp(0.0, 1.0) * 100.0) as u32).to_string();
    let mpv_volume = format!("--volume={percent}");
    let players: [Vec<&str>; 2] = [
        vec![
            "mpv",
            "--no-video",
            "--really-quiet",
            "--no-terminal",
            "--loop-file=inf",
            &mpv_volume,
        ],
        vec![
            "ffplay",
            "-nodisp",
            "-loglevel",
            "quiet",
            "-loop",
            "0",
            "-volume",
            &percent,
        ],
    ];
    players.into_iter().find_map(|player| {
        quiet(Command::new(player[0]).args(&player[1..]).arg(path))
            .spawn()
            .ok()
    })
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_audio__tests.rs"]
mod tests;
