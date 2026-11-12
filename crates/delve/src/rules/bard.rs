//! The Bard: its tracks (the stream plays the station's trance) and its Lullaby, a moving song, one sleep per enemy per cast.
use super::*;
use std::collections::BTreeSet;

pub const SONG_TICKS: u32 = 2 * HZ;
pub const SLEEP_TICKS: u32 = 2 * HZ;
pub const BOSS_SLEEP_TICKS: u32 = HZ / 2;
/// Distance between centres in world units, in this room, through scenery.
pub const SONG_REACH: f32 = 7.0;

/// The stream plays Knight Shift Radio's trance. A volley leaves every 4
/// ticks (30 Hz), so three of them are exactly 0.4 s: a beat at 150 BPM, a
/// bar (four beats) 1.6 s. Each song is an excerpt of the station's uplifting
/// trance, time-stretched with its pitch kept so its beats fall on that
/// grid, then cut into one-beat slices (`sfx/bard_<song>_<beat>.mp3`, made
/// by `assets/dungeon/bard_tracks`): the stream names the next slice on the
/// first volley of every beat, so the song plays forward in step with the
/// fire, and when the fire stops the last slice (0.4 s) is all that is left.
pub const SONGS: usize = 8;
pub const VOLLEYS_PER_BEAT: u32 = 3;
/// Slices in a song: 24 bars.
pub const BEATS: u32 = 96;
/// Held past the last slice, the song goes round again from here (bar 9: the
/// second 8-bar phrase of the drop).
pub const LOOP_FROM: u32 = 32;
/// Held a bar (the 13th volley, 1.6 s), the bassline comes in under the
/// high synth: the slices are the lead and the bass mixed.
pub const BASS_AT: u32 = 13;
/// Held three bars (the 37th volley, 4.8 s), the drums drop in: the overload,
/// the song's drop. The slices from here are the lead, the bass and the
/// drums.
pub const DROP_AT: u32 = 37;
/// A note that lands: an open hi-hat over the groove.
pub const HIT: &str = "hit_note";

const LETTERS: [char; SONGS] = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];

static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();

/// Every file the Bard names, for the mixers: each song's slices in order,
/// then each song's Lullaby.
pub fn names() -> &'static [&'static str] {
    NAMES.get_or_init(|| {
        let leak = |name: String| -> &'static str { Box::leak(name.into_boxed_str()) };
        let slices = (0..SONGS).flat_map(|song| (0..BEATS).map(move |beat| format!("bard_{}_{beat:03}", LETTERS[song])));
        let lullabies = (0..SONGS).map(|song| format!("bard_{}_lull", LETTERS[song]));
        slices.chain(lullabies).map(leak).collect()
    })
}

/// Whether `name` is one of the Bard's: the party never hears these
/// (`Run::seat_sounds`), and the mixers let them through their caps.
pub fn is_bard_sound(name: &str) -> bool {
    name.starts_with("bard_")
}

/// Whether `name` is a song's slice rather than its Lullaby.
pub fn is_slice(name: &str) -> bool {
    is_bard_sound(name) && !name.ends_with("_lull")
}

/// The one-beat slice of `song` at `beat` (round past the last, back to
/// `LOOP_FROM`).
pub fn slice(song: usize, beat: u32) -> &'static str {
    names()[song % SONGS * BEATS as usize + beat_at(beat) as usize]
}

/// `song`'s Lullaby: a breakdown phrase from it, lead and pads only.
pub fn lullaby(song: usize) -> &'static str {
    names()[SONGS * BEATS as usize + song % SONGS]
}

/// Which beat of its song a count of beats is, looping the last phrases.
pub fn beat_at(beats: u32) -> u32 {
    if beats < BEATS {
        beats
    } else {
        LOOP_FROM + (beats - LOOP_FROM) % (BEATS - LOOP_FROM)
    }
}

/// Whether volley `groove` (counting from 1, the first of an unbroken run)
/// is the first of a beat: the slice of the beat is named on it.
pub fn on_beat(groove: u32) -> bool {
    groove != 0 && (groove - 1).is_multiple_of(VOLLEYS_PER_BEAT)
}

/// The song a knight plays: seats start on the first songs in turn, and each
/// song it has moved on from (`Hero::songs`) is the next.
pub fn song_of(knight: u32, songs: u32) -> usize {
    (knight.wrapping_sub(1) as usize).wrapping_add(songs as usize) % SONGS
}

/// The sounds of `knight`'s Note Stream volley leaving, the `groove`th
/// without a break: on the first of each beat the next slice of its song,
/// the high synth alone, with the bass from `BASS_AT` and the drums from
/// `DROP_AT` (the slices are mixed that way). The Bard's music is its own:
/// only the knight playing it hears it (`Run::seat_sounds`).
pub fn volley_sounds(knight: u32, songs: u32, groove: u32, sounds: &mut Vec<(u32, &'static str)>) {
    if on_beat(groove) {
        sounds.push((knight, slice(song_of(knight, songs), (groove - 1) / VOLLEYS_PER_BEAT)));
    }
}

/// How far into the groove a knight is: 0 playing (or not), 1 the bass is
/// in, 2 the drums have dropped.
pub fn stage(groove: u32) -> u8 {
    u8::from(groove >= BASS_AT) + u8::from(groove >= DROP_AT)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Song {
    pub owner: u32,
    /// First tick the song has ended.
    pub until: u64,
    /// Do not refresh a sleeper every tick, or when it re-enters the ring.
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
                    // Ultimates resolve after monsters act: the following
                    // `hold` complete simulation ticks are asleep.
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
