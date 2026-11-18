//! The Bard: its tracks (the stream plays the station's trance) and its Lullaby, a moving song, one sleep per enemy per cast.
use super::*;
use std::collections::BTreeSet;

pub const SONG_TICKS: u32 = 2 * HZ;
pub const SLEEP_TICKS: u32 = 2 * HZ;
pub const BOSS_SLEEP_TICKS: u32 = HZ / 2;
pub const SONG_REACH: f32 = 7.0;

pub const SONGS: usize = 8;
pub const VOLLEYS_PER_BEAT: u32 = 3;
pub const TICKS_PER_COUNT: u32 = 4;
pub const REST: &str = "bard_rest";
pub const BEATS: u32 = 96;
pub const LOOP_FROM: u32 = 32;
pub const BASS_AT: u32 = 13;
pub const DROP_AT: u32 = 37;
pub const HIT: &str = "hit_note";

const LETTERS: [char; SONGS] = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];

static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();

pub fn names() -> &'static [&'static str] {
    NAMES.get_or_init(|| {
        let leak = |name: String| -> &'static str { Box::leak(name.into_boxed_str()) };
        let slices = (0..SONGS).flat_map(|song| (0..BEATS).map(move |beat| format!("bard_{}_{beat:03}", LETTERS[song])));
        let lullabies = (0..SONGS).map(|song| format!("bard_{}_lull", LETTERS[song]));
        slices.chain(lullabies).map(leak).collect()
    })
}

pub fn is_bard_sound(name: &str) -> bool {
    name.starts_with("bard_")
}

pub fn heard_by_all() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("ANGEL_BARD_HEARD").is_ok_and(|v| v == "all"))
}

pub fn is_slice(name: &str) -> bool {
    is_bard_sound(name) && name != REST && !name.ends_with("_lull")
}

pub fn slice(song: usize, beat: u32) -> &'static str {
    names()[song % SONGS * BEATS as usize + beat_at(beat) as usize]
}

pub fn lullaby(song: usize) -> &'static str {
    names()[SONGS * BEATS as usize + song % SONGS]
}

pub fn beat_at(beats: u32) -> u32 {
    if beats < BEATS {
        beats
    } else {
        LOOP_FROM + (beats - LOOP_FROM) % (BEATS - LOOP_FROM)
    }
}

pub fn on_beat(groove: u32) -> bool {
    groove != 0 && (groove - 1).is_multiple_of(VOLLEYS_PER_BEAT)
}

pub fn song_of(knight: u32, songs: u32) -> usize {
    (knight.wrapping_sub(1) as usize).wrapping_add(songs as usize) % SONGS
}

pub fn strum(knight: u32, hero: &mut Hero, held: bool, sounds: &mut Vec<(u32, &'static str)>) -> bool {
    if !held {
        if hero.strum != 0 {
            sounds.push((knight, REST));
        }
        hero.strum = 0;
        hero.groove = 0;
        return false;
    }
    hero.strum = hero.strum.saturating_add(1);
    if !(hero.strum - 1).is_multiple_of(TICKS_PER_COUNT) {
        return false;
    }
    hero.groove = if hero.groove == 0 {
        hero.tune.saturating_mul(VOLLEYS_PER_BEAT).saturating_add(1)
    } else {
        hero.groove.saturating_add(1)
    };
    if !on_beat(hero.groove) {
        return false;
    }
    let beat = (hero.groove - 1) / VOLLEYS_PER_BEAT;
    hero.tune = beat.saturating_add(1);
    sounds.push((knight, slice(song_of(knight, hero.songs), beat)));
    hero.groove == DROP_AT
}

pub fn stage(groove: u32) -> u8 {
    u8::from(groove >= BASS_AT) + u8::from(groove >= DROP_AT)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Song {
    pub owner: u32,
    pub until: u64,
    pub reached: BTreeSet<u32>,
}

pub(super) fn awake(tick: &u64) -> bool { *tick == 0 }

impl Run {
    pub(super) fn tick_bard_songs(&mut self) {
        self.bard_songs.retain(|s| s.until > self.tick);
        for song in &mut self.bard_songs {
            let Some(hero) = self.players.get(&song.owner).filter(|h| h.hp > 0 && !h.stone) else {
                song.until = self.tick;
                continue;
            };
            for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                if (enemy.x - hero.x).hypot(enemy.y - hero.y) <= SONG_REACH
                    && song.reached.insert(enemy.id)
                {
                    let hold = if enemy.is_boss() { BOSS_SLEEP_TICKS } else { SLEEP_TICKS };
                    enemy.sleep_until = enemy.sleep_until.max(self.tick + u64::from(hold) + 1);
                }
            }
        }
        self.bard_songs.retain(|s| s.until > self.tick);
        for enemy in &mut self.enemies {
            if !enemy.asleep(self.tick) { enemy.sleep_until = 0; }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_bard__tests.rs"]
mod tests;
