//! Two model-free, walk-to-play games in the clearing. The host owns both;
//! all living friends contribute, and the ordinary save/mirror carries them.

use super::layout::TILE_UNITS;
use super::*;

pub const TREES: [(i32, i32); 8] = [
    (2, 2),
    (8, 2),
    (10, 5),
    (2, 8),
    (9, 11),
    (14, 2),
    (22, 3),
    (22, 10),
];
pub const LANTERN: (f32, f32) = (4.0, 10.5);
pub const MEMORY: (f32, f32) = (18.0, 10.5);
pub const RUNES: [(f32, f32); 3] = [(15.0, 5.5), (18.5, 3.5), (21.0, 7.0)];
const FLIES: [(f32, f32); 8] = [
    (3.5, 4.0),
    (6.0, 3.5),
    (8.5, 4.5),
    (7.5, 6.5),
    (4.5, 6.0),
    (3.5, 8.5),
    (6.5, 9.0),
    (9.0, 8.5),
];
pub const FLY_COUNT: usize = FLIES.len();
pub const HUNT_TIME: u32 = HZ * 30;
pub const SONG_LEN: u8 = 4;
const SHOW_TIME: u32 = HZ * (SONG_LEN as u32 + 1);
const ALL_FLIES: u16 = (1 << FLY_COUNT) - 1;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forest {
    pub hunt_left: u32,
    pub caught: u16,
    pub best: u8,
    pub hunts: u32,
    pub singing: bool,
    pub show_left: u32,
    pub note: u8,
    pub songs: u32,
    // Occupied pads last tick: standing still cannot replay or input twice.
    held: u8,
}

impl Forest {
    pub fn valid(&self) -> bool {
        self.hunt_left <= HUNT_TIME
            && self.caught & !ALL_FLIES == 0
            && self.best <= FLY_COUNT as u8
            && self.show_left <= SHOW_TIME
            && self.note <= SONG_LEN
            && self.held & !31 == 0
            && (self.singing || self.show_left == 0)
            && (!self.singing || self.note < SONG_LEN)
    }

    pub fn leave(&mut self) {
        self.hunt_left = 0;
        self.caught = 0;
        self.singing = false;
        self.show_left = 0;
        self.note = 0;
        self.held = 0;
    }

    pub fn melody(&self, note: u8) -> usize {
        ([0, 2, 1, 2][usize::from(note.min(SONG_LEN - 1))] + (self.songs % 3) as usize)
            % RUNES.len()
    }

    /// Each note shines for two thirds of a second, separated by darkness.
    pub fn lit_rune(&self) -> Option<usize> {
        if !self.singing || self.show_left == 0 {
            return None;
        }
        let elapsed = SHOW_TIME.saturating_sub(self.show_left);
        let note = elapsed / HZ;
        (note < SONG_LEN as u32 && elapsed % HZ < HZ * 2 / 3).then(|| self.melody(note as u8))
    }
}

/// Small deterministic loops, away from tree collision. Arena units, not tiles.
pub fn firefly(index: usize, tick: u64) -> (f32, f32) {
    let (x, y) = FLIES[index];
    let t =
        (tick % (HZ as u64 * 12)) as f32 / HZ as f32 * std::f32::consts::TAU / 12.0 + index as f32;
    (
        x * TILE_UNITS + t.cos() * 0.6,
        y * TILE_UNITS + t.sin() * 0.6,
    )
}

impl Run {
    pub(super) fn tick_forest(&mut self) {
        if self.forest_road_showing() || self.roundtable_showing() { return; }
        let near = |x: f32, y: f32| {
            self.players
                .values()
                .any(|h| h.hp > 0 && !h.stone && h.privy == 0 && (h.x - x).hypot(h.y - y) < 1.25)
        };
        let mut occupied = 0u8;
        for (i, (x, y)) in [LANTERN, MEMORY].into_iter().chain(RUNES).enumerate() {
            if near(x * TILE_UNITS, y * TILE_UNITS) {
                occupied |= 1 << i;
            }
        }
        let pressed = occupied & !self.forest.held;
        self.forest.held = occupied;
        if pressed & 1 != 0 && self.forest.hunt_left == 0 {
            self.forest.hunt_left = HUNT_TIME;
            self.forest.caught = 0;
            self.sounds.push("play_card");
        }
        if pressed & 2 != 0 && !self.forest.singing {
            self.forest.singing = true;
            self.forest.show_left = SHOW_TIME;
            self.forest.note = 0;
            self.sounds.push("play_card");
        }
        if self.forest.hunt_left > 0 {
            self.forest.hunt_left -= 1;
            for i in 0..FLY_COUNT {
                let (x, y) = firefly(i, self.tick);
                if self.forest.caught & (1 << i) == 0 && near(x, y) {
                    self.forest.caught |= 1 << i;
                    self.sounds.push("spoil_pickup");
                }
            }
            let caught = self.forest.caught.count_ones() as u8;
            self.forest.best = self.forest.best.max(caught);
            if self.forest.caught == ALL_FLIES {
                self.forest.hunt_left = 0;
                self.forest.hunts = self.forest.hunts.saturating_add(1);
                self.score = self.score.saturating_add(100);
                self.marks.insert(super::countryside::GROVE_MARKS[0].into(), 1);
                self.found = Some((self.tick, 0, "All eight fireflies! +40 gold".into()));
                self.sounds.push("play_card");
            } else if self.forest.hunt_left == 0 {
                self.found = Some((
                    self.tick,
                    0,
                    format!("Fireflies: {caught}/8. Step off to retry."),
                ));
            }
        }
        if !self.forest.singing {
            return;
        }
        if self.forest.show_left > 0 {
            self.forest.show_left -= 1;
            return;
        }
        let notes = pressed >> 2;
        if notes == 0 {
            return;
        }
        // Two friends striking different notes together is ambiguous, not a
        // free pass through the sequence. Step off and choose one together.
        if notes.count_ones() > 1 {
            self.found = Some((
                self.tick,
                0,
                "One rune at a time! Step off and try that note again.".into(),
            ));
            return;
        }
        if notes != 1 << self.forest.melody(self.forest.note) {
            self.forest.singing = false;
            self.found = Some((
                self.tick,
                0,
                "The grove falls quiet. Return to the memory stone to retry.".into(),
            ));
            return;
        }
        self.forest.note += 1;
        self.sounds.push("spoil_pickup");
        if self.forest.note == SONG_LEN {
            self.forest.singing = false;
            self.forest.songs = self.forest.songs.saturating_add(1);
            self.score = self.score.saturating_add(100);
            self.marks.insert(super::countryside::GROVE_MARKS[1].into(), 1);
            self.found = Some((
                self.tick,
                0,
                "The woodland remembers your song! +40 gold".into(),
            ));
            self.sounds.push("play_card");
        }
    }
}
