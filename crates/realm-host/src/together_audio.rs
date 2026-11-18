//! The Delve's sound: effects the game names, the knights' voices, and music
//! looping under play.
//!
//! The sounds are files in `assets/dungeon/sfx/` and `assets/dungeon/music/`
//! (see `sfx/README.md` for what each is and the levels); the knights' grunts
//! and chatter are effects too, under `sfx/knights/`. Effects are
//! throttled per name and capped in number, so rapid fire never piles into
//! noise; music is one looping player that is replaced or stopped, never
//! doubled. Everything plays through a helper process (`pw-play`, `mpv`,
//! `ffplay`), so a missing player only means silence. Tests make no sound.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

mod live;
mod bard_stream;

pub const VOICE_LEVEL: f32 = SFX_LEVEL;
const MUSIC_LEVEL: f32 = 0.15;
const SFX_LEVEL: f32 = 0.5;
const MAX_AT_ONCE: usize = 5;
const SFX_BYTES_PER_SECOND: f32 = 12_000.0;

pub const SFX: &[&str] = &[
    "shot_bow",
    "shot_crossbow",
    "shot_handgonne",
    "shot_bolt",
    "shot_arcane",
    "shot_spark",
    "swing",
    "hit",
    "kill",
    "hero_hurt",
    "roll",
    "shield_block",
    "clang",
    "bomb",
    "card_pickup",
    "spoil_pickup",
    "door_seal",
    "door_open",
    "area_leave",
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
    "gas_release",
    "engine_whistle",
    "spell_fire",
    "skill_barrier",
    "wish_grant",
    "spell_cast",
    "wall_up",
    "mana_empty",
    "wheel_spin",
    "wheel_land",
    "knight_hit",
    "knight_hit_big",
    "hit_blade",
    "hit_axe",
    "hit_arrow",
    "hit_bolt",
    "hit_blast",
    "hit_magic",
    "step_stone",
    "step_gravel",
    "step_ash",
    "step_wet",
    "step_moss",
    "step_wood",
    "door_pass",
    "gold",
    "potion",
    "holy_water",
    "bomb_toss",
    "shield_up",
    "axe_spin",
    "helmet_pop",
    "armour_fall",
    "boss_roar",
    "boss_rage",
    "boss_death",
    "knight_fall",
    "bones_clatter",
    "crack_open",
    "hand_rise",
    "hand_grab",
    "hand_drag",
    "ground_close",
    "hell_gate_rise",
    "hell_gate_enter",
    "circle_toll",
    "gore_kick",
    "gore_thunk",
    "gore_chop",
    "gore_splat",
    "gore_bones",
    "gore_burst",
    "hit_note",
    "mirage_bing_bong",
    "swing_sword",
    "swing_axe",
    "swing_maul",
    "swing_spear",
    "swing_cleaver",
    "hit_flesh_light",
    "hit_flesh_medium",
    "hit_flesh_heavy",
    "parry",
    "parry_whizz",
    "gore_behead",
    "gore_head_thud",
    "gore_bisect",
    "gore_slump",
    "cast_fire",
    "cast_frost",
    "cast_holy",
    "cast_storm",
    "cast_poison",
    "cast_arcane",
    "cast_earth",
    "impact_fire",
    "impact_frost",
    "impact_holy",
    "impact_storm",
    "impact_poison",
    "impact_arcane",
    "impact_earth",
    "lamp_rub",
    "genie_appear",
    "wish_land",
    "wish_land_grand",
    "wish_land_mythic",
    "fairy_wish",
    "level_up",
    "rung_up",
    "relic_find",
    "deepest_yet",
    "sky_lit",
];

pub fn names() -> impl Iterator<Item = &'static str> {
    SFX.iter()
        .copied()
        .chain(angel_delve::rules::bard::names().iter().copied())
        .chain(super::together_shooter::chatter::names().iter().copied())
}

fn is_voice(name: &str) -> bool {
    name.starts_with("knights/") || name == "mirage_bing_bong"
}

pub const TRACKS: &[&str] = &["crypt", "mines", "keep", "boss", "sanctuary", "menu"];

fn gap(name: &str) -> Duration {
    Duration::from_millis(match name {
        "gas_release" => 1500,
        "spell_fire" | "skill_barrier" => 500,
        "wish_grant" => 1000,
        "spell_cast" => 250,
        "shot_bow" | "shot_crossbow" | "shot_handgonne" | "shot_bolt" | "shot_arcane"
        | "shot_spark" => 70,
        "hit" | "hit_blade" | "hit_axe" | "hit_arrow" | "hit_bolt" | "hit_blast" | "hit_magic" | "hit_note" => 60,
        n if angel_delve::rules::bard::is_slice(n) => 300,
        n if angel_delve::rules::bard::is_bard_sound(n) => 800,
        "step_stone" | "step_gravel" | "step_ash" | "step_wet" | "step_moss" | "step_wood" => 210,
        "knight_hit" | "knight_hit_big" | "gold" | "bomb_toss" | "shield_up" => 120,
        "boss_roar" | "boss_rage" | "boss_death" | "crack_open" | "hand_rise" | "hand_grab"
        | "hand_drag" | "ground_close" | "hell_gate_rise" | "hell_gate_enter" | "circle_toll"
        | "knight_fall" => 800,
        "mirage_bing_bong" => 1200,
        "kill" => 90,
        "gore_kick" | "gore_thunk" | "gore_chop" | "gore_splat" | "gore_bones" | "gore_burst" => 110,
        "swing" | "roll" | "shield_block" | "clang" | "spike" | "vent_fire" => 150,
        "boss_rise" | "boss_fall" | "descend" | "door_seal" | "door_open" | "area_leave" => 800,
        "wheel_spin" | "wheel_land" => 1500,
        "swing_sword" | "swing_axe" | "swing_maul" | "swing_spear" | "swing_cleaver" => 150,
        "hit_flesh_light" | "hit_flesh_medium" | "hit_flesh_heavy" => 70,
        "parry" => 250,
        "parry_whizz" => 150,
        "gore_behead" | "gore_head_thud" | "gore_bisect" | "gore_slump" => 110,
        "cast_fire" | "cast_frost" | "cast_holy" | "cast_storm" | "cast_poison" | "cast_arcane"
        | "cast_earth" => 250,
        "impact_fire" | "impact_frost" | "impact_holy" | "impact_storm" | "impact_poison"
        | "impact_arcane" | "impact_earth" => 90,
        "lamp_rub" => 600,
        "genie_appear" => 800,
        "wish_land" | "wish_land_grand" | "wish_land_mythic" => 1000,
        "fairy_wish" => 1500,
        "level_up" | "rung_up" | "relic_find" | "deepest_yet" | "sky_lit" => 1000,
        _ => 200,
    })
}

pub fn missing(line: &str) {
    use std::io::IsTerminal;
    tracing::warn!("{line}");
    if !std::io::stderr().is_terminal() {
        eprintln!("angel: {line}");
    }
}

fn roots() -> Vec<PathBuf> {
    [
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
        angel_live::host::runtime_paths::source_cockpit_dir()
            .and_then(Path::parent)
            .map(Path::to_path_buf),
        Some(crate::root()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn resource(place: &str) -> Option<PathBuf> {
    roots().into_iter().map(|root| root.join(place)).find(|d| d.is_dir())
}

pub fn sound_dir() -> Option<PathBuf> {
    roots()
        .into_iter()
        .map(|root| root.join("cockpit/assets/dungeon"))
        .find(|d| d.join("sfx").is_dir() || d.join("music").is_dir())
}

#[cfg(any(test, feature = "testing"))]
pub fn sfx_file(name: &str) -> Option<PathBuf> {
    let name = names().find(|&s| s == name)?;
    let file = sound_dir()?.join("sfx").join(format!("{name}.mp3"));
    file.is_file().then_some(file)
}

pub struct Voices {
    pub level: f32,
    pub muted: bool,
}

impl Default for Voices {
    fn default() -> Self {
        Voices {
            level: VOICE_LEVEL,
            muted: std::env::var("ANGEL_DELVE_VOICES").is_ok_and(|v| v == "0"),
        }
    }
}

#[derive(Default)]
pub struct Audio {
    live:live::Playback,
    last: HashMap<&'static str, Instant>,
    sounding: Vec<Instant>,
    track: Option<&'static str>,
    player: Option<Child>,
    muted: bool,
    pub voices: Voices,
    played: usize,
    dir: Option<PathBuf>,
    looked: bool,
}

impl Audio {
    pub fn pump_live(&mut self, now: Instant) {
        for voice in self.live.poll(now) {
            self.sfx_voice(voice, now);
        }
    }
    pub fn sfx_from(
        &mut self,
        name: &str,
        now: Instant,
        voice: Option<super::together_media::Voice>,
        origin: Option<(String, angel_serve::client::Trust)>,
    ) {
        if self.muted {
            return;
        }
        if let Some(voice) = voice {
            self.sfx_voice(voice, now);
        } else if super::together_media::parse_name(name).is_some() {
            if let Some(voice) = self.live.request(name, origin, now) {
                self.sfx_voice(voice, now);
            }
        } else {
            self.sfx_line(name, now);
        }
    }
    fn sfx_voice(&mut self, voice: super::together_media::Voice, now: Instant) {
        if self.muted || voice.level == 0 {
            return;
        }
        self.sounding.retain(|&end| end > now);
        if self.sounding.len() >= MAX_AT_ONCE || !self.live.admit(&voice, now) {
            return;
        }
        self.sounding.push(now + voice.sound.duration);
        self.played += 1;
        self.live.play(voice);
    }
    fn dir(&mut self) -> Option<&Path> {
        if !self.looked {
            self.looked = true;
            self.dir = sound_dir();
            match &self.dir {
                None => missing("no Delve sounds beside this binary (cockpit/assets/dungeon): the game plays silent"),
                Some(dir) if !dir.join("music").is_dir() => missing(&format!(
                    "no Delve music in {}: the game plays without it",
                    dir.display()
                )),
                Some(_) => {}
            }
            if !cfg!(any(test, feature = "testing"))
                && let Some(dir) = &self.dir
            {
                bard_stream::warm(&dir.join("sfx"));
            }
        }
        self.dir.as_deref()
    }

    pub fn sfx_line(&mut self, file: &str, now: Instant) {
        if file == angel_delve::wire::sound_file(angel_delve::rules::bard::REST) {
            self.sfx(angel_delve::rules::bard::REST, now);
        } else if let Some(name) = names().find(|n| angel_delve::wire::sound_file(n) == file) {
            self.sfx(name, now);
        }
    }

    pub fn sfx(&mut self, name: &str, now: Instant) {
        if name == angel_delve::rules::bard::REST {
            if !cfg!(any(test, feature = "testing")) {
                bard_stream::rest();
            }
            return;
        }
        let Some(name) = names().find(|&s| s == name) else {
            return;
        };
        if self.muted || (is_voice(name) && self.voices.muted) {
            return;
        }
        if self
            .last
            .get(name)
            .is_some_and(|&at| now.saturating_duration_since(at) < gap(name))
        {
            return;
        }
        if !cfg!(any(test, feature = "testing"))
            && angel_delve::rules::bard::is_slice(name)
            && let Some(dir) = self.dir().map(|d| d.join("sfx"))
            && bard_stream::heard(&dir, name, SFX_LEVEL, now)
        {
            self.last.insert(name, now);
            self.played += 1;
            return;
        }
        self.sounding.retain(|&end| end > now);
        let song = angel_delve::rules::bard::is_bard_sound(name);
        if self.sounding.len() >= MAX_AT_ONCE && !song {
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
            let level = if is_voice(name) { self.voices.level } else { SFX_LEVEL };
            play_once(&file, level);
        }
    }

    pub fn music(&mut self, track: Option<&str>) {
        let track = match track {
            None => None,
            Some(name) => match TRACKS.iter().copied().find(|&t| t == name).or_else(|| angel_delve::rules::music::known(name)) {
                Some(t) => Some(t),
                None => return,
            },
        };
        if track == self.track {
            return;
        }
        self.track = track;
        self.restart();
    }

    pub fn stop(&mut self) {
        self.live.clear();
        self.sounding.clear();
        self.track = None;
        self.kill();
        if !cfg!(any(test, feature = "testing")) {
            bard_stream::rest();
        }
    }

    pub fn set_muted(&mut self, muted: bool) {
        if muted == self.muted {
            return;
        }
        self.muted = muted;
        if muted && !cfg!(any(test, feature = "testing")) {
            bard_stream::rest();
        }
        self.restart();
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn track(&self) -> Option<&'static str> {
        self.track
    }

    fn restart(&mut self) {
        self.kill();
        if self.muted || cfg!(any(test, feature = "testing")) {
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

fn play_once(path: &Path, level: f32) {
    if cfg!(any(test, feature = "testing")) {
        return;
    }
    static PLAYER: std::sync::OnceLock<Option<std::sync::mpsc::Sender<(PathBuf, f32)>>> =
        std::sync::OnceLock::new();
    let player = PLAYER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<(PathBuf, f32)>();
        std::thread::Builder::new()
            .name("delve-sfx".into())
            .spawn(move || {
                let mut playing: Vec<Child> = Vec::new();
                for (path, level) in rx {
                    playing.retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_)) | Err(_)));
                    if let Some(child) = once_player(&path, level) {
                        playing.push(child);
                    }
                }
            })
            .ok()
            .map(|_| tx)
    });
    if let Some(player) = player {
        let _ = player.send((path.to_path_buf(), level));
    }
}

fn once_player(path: &Path, level: f32) -> Option<Child> {
    let level = level.clamp(0.0, 1.0);
    let pw = format!("{level:.2}");
    let pa = format!("{}", (level * 65536.0) as u32);
    let mpv = format!("--volume={}", (level * 100.0) as u32);
    let players: [Vec<&str>; 4] = [
        vec!["pw-play", "--volume", &pw],
        vec!["paplay", "--volume", &pa],
        vec!["mpv", "--no-video", "--really-quiet", "--no-terminal", &mpv],
        vec!["afplay", "-v", &pw],
    ];
    for player in players {
        if let Ok(child) = quiet(Command::new(player[0]).args(&player[1..]).arg(path)).spawn() {
            return Some(child);
        }
    }
    None
}
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
#[path = "../tests/together_audio__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/together_audio_delve__tests.rs"]
mod delve_tests;
