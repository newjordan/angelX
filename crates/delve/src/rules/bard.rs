//! Bard's Lullaby: a moving song, one sleep per enemy per cast.
use super::*;
use std::collections::BTreeSet;

pub const SONG_TICKS: u32 = 2 * HZ;
pub const SLEEP_TICKS: u32 = 2 * HZ;
pub const BOSS_SLEEP_TICKS: u32 = HZ / 2;
/// Distance between centres in world units, in this room, through scenery.
pub const SONG_REACH: f32 = 7.0;

/// The stream is a deep techno sax pulse: each volley blows the next stab
/// of this riff, one file a pitch (`sfx/sax_*.mp3`, `synth_sfx.py`). Two
/// bars of sixteenths in A minor, bouncing off the low A: a held stream (a
/// volley every four ticks, 112.5 BPM) plays it in a little over four
/// seconds and goes round again.
pub const RIFF: [&str; 32] = [
    "sax_a2", "sax_a2", "sax_a3", "sax_a2", "sax_c3", "sax_a2", "sax_a3", "sax_a2",
    "sax_e3", "sax_a2", "sax_a3", "sax_a2", "sax_d3", "sax_c3", "sax_a2", "sax_g2",
    "sax_a2", "sax_a2", "sax_a3", "sax_a2", "sax_c3", "sax_a2", "sax_a3", "sax_a2",
    "sax_g3", "sax_e3", "sax_d3", "sax_c3", "sax_d3", "sax_e3", "sax_g3", "sax_a3",
];

/// Every pitch the riff blows, low to high: the mixers' list.
pub const SAX: [&str; 7] = ["sax_g2", "sax_a2", "sax_c3", "sax_d3", "sax_e3", "sax_g3", "sax_a3"];

/// The pulse: a sub kick under every fourth volley, four on the floor.
pub const KICK: &str = "bard_kick";
/// Held this many volleys without a break (two seconds), the bass drops
/// in: a wobble under every kick, wub wub.
pub const WUB_AT: u32 = 16;
pub const WUB: &str = "bard_wub";
/// Held a whole riff (four seconds and a little), the horn overloads: a
/// riser and a drop once, then glitches stuttering on the off-beats.
pub const GLITCH_AT: u32 = 32;
pub const OVERLOAD: &str = "bard_overload";
pub const GLITCHES: [&str; 4] = ["bard_glitch_0", "bard_glitch_1", "bard_glitch_2", "bard_glitch_3"];
/// A note that lands: an open hi-hat over the groove.
pub const HIT: &str = "hit_note";
/// The Lullaby: the same sax, slow and breathy (R).
pub const LULLABY: &str = "bard_lullaby";

/// The stab a knight's next volley blows, `loosed` volleys in
/// (`Hero::loosed` counts them before the volley flies).
pub fn riff_note(loosed: u32) -> &'static str {
    RIFF[loosed as usize % RIFF.len()]
}

/// The kick under that volley, on the beat.
pub fn pulse(loosed: u32) -> Option<&'static str> {
    loosed.is_multiple_of(4).then_some(KICK)
}

/// The sounds of a Note Stream volley leaving, `loosed` volleys in and
/// the `groove`th without a break: the riff's stab and the kick on the
/// beat; the wub under the kick once the groove is in; the overload as
/// it tips over, then a glitch on each off-beat.
pub fn volley_sounds(loosed: u32, groove: u32, sounds: &mut Vec<&'static str>) {
    sounds.push(riff_note(loosed));
    sounds.extend(pulse(loosed));
    if groove >= WUB_AT && loosed.is_multiple_of(4) {
        sounds.push(WUB);
    }
    if groove == GLITCH_AT {
        sounds.push(OVERLOAD);
    } else if groove > GLITCH_AT && loosed % 4 == 2 {
        sounds.push(GLITCHES[(loosed / 4) as usize % GLITCHES.len()]);
    }
}

/// How far into the groove a knight is: 0 playing (or not), 1 the wub
/// is in, 2 glitching.
pub fn stage(groove: u32) -> u8 {
    u8::from(groove >= WUB_AT) + u8::from(groove >= GLITCH_AT)
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
