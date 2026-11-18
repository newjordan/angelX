//! Hearthlight — the town's slow living layer.
//!
//! The world so far is *reactive*: it mirrors tool calls, loops and storms.
//! Hearthlight is the layer that *lives*: three resources that mean what they
//! say, a prosperity ladder that physically grows the hamlet, and a day/night
//! clock that sends the keepers home at dusk. All of it is pure display — it
//! reads telemetry the cockpit already has (tool outcomes, ollama model count,
//! village head/forge state) and it feeds nothing back into the model or the
//! agent loop (the standing invariant).
//!
//! Economy:
//! - `light`  — work done. Tool successes feed it; it is the town's memory of
//!   effort, spent slowly into `warmth` while local models serve.
//! - `warmth` — local presence. Rises toward a target set by attached local
//!   models (ollama count + lit fleet heads + a hot forge); decays when the
//!   local fleet sleeps. This is the "grows if local models are attached" half
//!   of the charter.
//! - `grain`  — dataset stock (samples + coverage), the village's bread.
//!
//! `prosperity` is the long score: it integrates warmth + grain + light into
//! tiers that unlock garden → well → lanterns → banners in the hamlet.
//!
//! Persistence: same JSON file as `WorldRewards` (`~/.angelX/world.json`),
//! atomically written, throttled — identical discipline to the rewards save.
//!
//! Efficiency budget: everything here is O(1) integer math on beats that are
//! 64 ticks apart (~a few seconds); rendering reads fields directly. No
//! allocation, no timers, no threads — when the pane parks, the clock simply
//! does not advance, which is both free and honest.

use serde::{Deserialize, Serialize};

pub const HEARTH_BEAT_TICKS: u64 = 64;

const HEARTH_SAVE_EVERY_BEATS: u64 = 8;

pub const DAY_BEATS: u64 = 64;

pub const TIER_GARDEN: u32 = 10;
pub const TIER_WELL: u32 = 24;
pub const TIER_LANTERNS: u32 = 42;
pub const TIER_BANNERS: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

impl DayPhase {
    pub fn of_beat(beat: u64) -> Self {
        let t = beat % DAY_BEATS;
        let q = DAY_BEATS / 4;
        if t < q {
            DayPhase::Dawn
        } else if t < q * 2 {
            DayPhase::Day
        } else if t < q * 3 {
            DayPhase::Dusk
        } else {
            DayPhase::Night
        }
    }

    pub fn light(self, beat: u64) -> f32 {
        let q = DAY_BEATS / 4;
        let frac = (beat % q) as f32 / q as f32;
        match self {
            DayPhase::Dawn => 0.55 + 0.45 * frac,
            DayPhase::Day => 1.0,
            DayPhase::Dusk => 1.0 - 0.45 * frac,
            DayPhase::Night => 0.55,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DayPhase::Dawn => "dawn",
            DayPhase::Day => "day",
            DayPhase::Dusk => "dusk",
            DayPhase::Night => "night",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HearthState {
    #[serde(default)]
    pub light: u32,
    #[serde(default)]
    pub warmth: u32,
    #[serde(default)]
    pub prosperity: u32,
    #[serde(default)]
    pub beats: u64,
    #[serde(default, skip)]
    pub last_save_beat: u64,
}

impl HearthState {
    fn warmth_target(ollama_models: u32, heads_lit: u32, forge_hot: bool) -> u32 {
        let mut t = ollama_models.min(12) * 6 + heads_lit.min(6) * 6;
        if forge_hot {
            t += 18;
        }
        t.min(100)
    }

    pub fn beat(
        &mut self,
        tool_wins: u32,
        ollama_models: u32,
        heads_lit: u32,
        forge_hot: bool,
        grain: u32,
    ) {
        self.beats += 1;
        self.light = (self.light + tool_wins.min(8)).min(999);
        let target = Self::warmth_target(ollama_models, heads_lit, forge_hot);
        if self.warmth < target {
            self.warmth += ((target - self.warmth) / 8).max(1);
        } else if self.warmth > target {
            self.warmth -= ((self.warmth - target) / 8).max(1);
        }
        if self.warmth >= 20 && self.light > 0 {
            let spend = (self.light / 16).clamp(1, 4);
            self.light -= spend;
            let gain = spend * (1 + self.warmth / 40) + grain.min(50) / 25;
            self.prosperity = (self.prosperity + gain).min(999);
        } else if grain >= 30 && self.prosperity > 0 {
            self.prosperity = self.prosperity.saturating_sub(0);
        }
    }

    pub fn tier(&self) -> u32 {
        match self.prosperity {
            p if p >= TIER_BANNERS => 4,
            p if p >= TIER_LANTERNS => 3,
            p if p >= TIER_WELL => 2,
            p if p >= TIER_GARDEN => 1,
            _ => 0,
        }
    }

    pub fn phase(&self) -> DayPhase {
        DayPhase::of_beat(self.beats)
    }

    pub fn flame_bits(&self) -> u32 {
        match self.warmth {
            w if w >= 60 => 3,
            w if w >= 30 => 2,
            w if w >= 10 => 1,
            _ => 0,
        }
    }

    pub fn due_to_save(&self) -> bool {
        self.beats >= self.last_save_beat + HEARTH_SAVE_EVERY_BEATS
    }

    pub fn note_saved(&mut self) {
        self.last_save_beat = self.beats;
    }

    pub fn note_victory(&mut self) {
        self.light = (self.light + 4).min(999);
    }

    pub fn note_promotion(&mut self) {
        self.prosperity = (self.prosperity + 40).min(999);
        self.light = (self.light + 10).min(999);
    }

    pub fn render_bucket(&self, seed: u64) -> u32 {
        let light = (self.phase().light(self.beats) * 4.0) as u32;
        light
            | (self.flame_bits().min(3) << 3)
            | (self.tier() << 5)
            | (crate::world_viz::life::weather_bucket(self.beats, seed) << 8)
    }

    pub fn daylight(&self) -> f32 {
        self.phase().light(self.beats)
    }
}

#[cfg(test)]
#[path = "../tests/hearth__tests.rs"]
mod tests;
