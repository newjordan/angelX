//! Bard's Lullaby: a moving song, one sleep per enemy per cast.
use super::*;
use std::collections::BTreeSet;

pub const SONG_TICKS: u32 = 2 * HZ;
pub const SLEEP_TICKS: u32 = 2 * HZ;
pub const BOSS_SLEEP_TICKS: u32 = HZ / 2;
/// Distance between centres in world units, in this room, through scenery.
pub const SONG_REACH: f32 = 7.0;

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
                    // The Bard's boards: every foe the song reaches.
                    self.school.note(song.owner, "bard-lull", 1);
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
