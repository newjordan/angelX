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

/// One beat every 64 world ticks (~4s at a lively 16 ticks/s; the world is
/// idle-parked most of the time, so beats follow the render clock).
pub(crate) const HEARTH_BEAT_TICKS: u64 = 64;

/// Persist at most this often (beats), matching the rewards writer's throttle.
const HEARTH_SAVE_EVERY_BEATS: u64 = 8;

/// Day length in beats. 64 beats ≈ 4–5 minutes of an awake world; a full day
/// is ~4.5 min of active rendering — long enough to feel like weather, short
/// enough that a working session sees dusk fall.
pub(crate) const DAY_BEATS: u64 = 64;

/// Prosperity tiers — the hamlet's physical growth ladder.
pub(crate) const TIER_GARDEN: u32 = 10;
pub(crate) const TIER_WELL: u32 = 24;
pub(crate) const TIER_LANTERNS: u32 = 42;
pub(crate) const TIER_BANNERS: u32 = 64;

/// Phase of the world's day, derived from `beat % DAY_BEATS`. Rendering maps
/// this to a light scalar; schedules (keepers, lamps, fireflies) key off it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

impl DayPhase {
    pub(crate) fn of_beat(beat: u64) -> Self {
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

    /// Terrain light multiplier, blended across the phase so dawn/dusk ease.
    /// 1.0 = full day, ~0.55 = deep night. Cheap arithmetic on the render path.
    pub(crate) fn light(self, beat: u64) -> f32 {
        let q = DAY_BEATS / 4;
        let frac = (beat % q) as f32 / q as f32; // position inside the phase
        match self {
            // Dawn brightens from night to day.
            DayPhase::Dawn => 0.55 + 0.45 * frac,
            DayPhase::Day => 1.0,
            // Dusk dims from day to night.
            DayPhase::Dusk => 1.0 - 0.45 * frac,
            DayPhase::Night => 0.55,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            DayPhase::Dawn => "dawn",
            DayPhase::Day => "day",
            DayPhase::Dusk => "dusk",
            DayPhase::Night => "night",
        }
    }
}

/// The persistent hearth state. Serialized into the same world.json the
/// rewards already use — unknown fields are ignored by older loads both ways
/// (`serde(default)` everywhere).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HearthState {
    /// Work done, fed by tool successes (spends into warmth when local serve).
    #[serde(default)]
    pub light: u32,
    /// Local presence: attached local models make the town warm.
    #[serde(default)]
    pub warmth: u32,
    /// Long score — integrates warmth/grain/light into the growth tiers.
    #[serde(default)]
    pub prosperity: u32,
    /// Total beats the world has lived (drives the day clock).
    #[serde(default)]
    pub beats: u64,
    /// Beat of the last persisted write (throttle).
    #[serde(default, skip)]
    pub last_save_beat: u64,
}

impl HearthState {
    /// Warmth target from the local fleet: each ollama model and each lit
    /// head warms the town; a hot forge warms it most of all. 0..=100-ish.
    fn warmth_target(ollama_models: u32, heads_lit: u32, forge_hot: bool) -> u32 {
        let mut t = ollama_models.min(12) * 6 + heads_lit.min(6) * 6;
        if forge_hot {
            t += 18;
        }
        t.min(100)
    }

    /// Advance one beat. Inputs are the telemetry the world already holds;
    /// `tool_wins` is successes since the last beat, `grain` the village's
    /// dataset stock. Pure integer math, no allocation.
    pub(crate) fn beat(
        &mut self,
        tool_wins: u32,
        ollama_models: u32,
        heads_lit: u32,
        forge_hot: bool,
        grain: u32,
    ) {
        self.beats += 1;
        // Work feeds light. Cap so a long history doesn't max the meter.
        self.light = (self.light + tool_wins.min(8)).min(999);
        // Warmth eases toward the fleet-set target (1/8 per beat — ~30s to
        // notice a model arrive, forgiving when one leaves).
        let target = Self::warmth_target(ollama_models, heads_lit, forge_hot);
        if self.warmth < target {
            self.warmth += ((target - self.warmth) / 8).max(1);
        } else if self.warmth > target {
            self.warmth -= ((self.warmth - target) / 8).max(1);
        }
        // While the town is warm, work converts into prosperity: light is
        // spent, warmth is the multiplier, grain feeds the ladder gently.
        if self.warmth >= 20 && self.light > 0 {
            let spend = (self.light / 16).clamp(1, 4);
            self.light -= spend;
            let gain = spend * (1 + self.warmth / 40) + grain.min(50) / 25;
            self.prosperity = (self.prosperity + gain).min(999);
        } else if grain >= 30 && self.prosperity > 0 {
            // A well-fed town never fully falls — it holds what it built.
            self.prosperity = self.prosperity.saturating_sub(0);
        }
    }

    /// Prosperity tier 0..=4 (garden, well, lanterns, banners).
    pub(crate) fn tier(&self) -> u32 {
        match self.prosperity {
            p if p >= TIER_BANNERS => 4,
            p if p >= TIER_LANTERNS => 3,
            p if p >= TIER_WELL => 2,
            p if p >= TIER_GARDEN => 1,
            _ => 0,
        }
    }

    pub(crate) fn phase(&self) -> DayPhase {
        DayPhase::of_beat(self.beats)
    }

    /// The dimmer bits: how much of the warmth pool is burning as lamps right
    /// now — 0 cold, 1 ember, 2 banked, 3 roaring. Counts the night lamps.
    pub(crate) fn flame_bits(&self) -> u32 {
        match self.warmth {
            w if w >= 60 => 3,
            w if w >= 30 => 2,
            w if w >= 10 => 1,
            _ => 0,
        }
    }

    /// Should a save be written this beat? Throttled like the rewards writer.
    pub(crate) fn due_to_save(&self) -> bool {
        self.beats >= self.last_save_beat + HEARTH_SAVE_EVERY_BEATS
    }

    pub(crate) fn note_saved(&mut self) {
        self.last_save_beat = self.beats;
    }

    /// A green turn: the town's faith in the work pays out light (in
    /// addition to the per-tool trickle that fed `tool_wins`).
    pub(crate) fn note_victory(&mut self) {
        self.light = (self.light + 4).min(999);
    }

    /// A promotion: the fleet got smarter — the whole town feels it.
    pub(crate) fn note_promotion(&mut self) {
        self.prosperity = (self.prosperity + 40).min(999);
        self.light = (self.light + 10).min(999);
    }

    /// The coarse render bucket the terrain cache keys on: the day-clock
    /// light level (5 steps), flame-dimmer bits, the prosperity tier, and the
    /// living weather (world-seeded — two realms never share a sky). When
    /// this value changes the world looks different — anything finer rides
    /// the per-frame layer instead.
    pub(crate) fn render_bucket(&self, seed: u64) -> u32 {
        let light = (self.phase().light(self.beats) * 4.0) as u32; // 0..=4
        light
            | (self.flame_bits().min(3) << 3)
            | (self.tier() << 5)
            | (crate::stage::world_viz::life::weather_bucket(self.beats, seed) << 8)
    }

    /// The exact light multiplier for the current beat (1.0 day → 0.55 night).
    #[allow(dead_code)]
    pub(crate) fn daylight(&self) -> f32 {
        self.phase().light(self.beats)
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/hearth__tests.rs"]
mod tests;
