//! Deterministic weather, town growth, and activity state shared by the
//! retained ride and room plates. Rendering lives in their own modules.

#![cfg_attr(not(test), allow(dead_code))]

use super::*;
use crate::ui::retro_kit;

// The one vivid color in the realm's signal-residue layer.  It is deliberately

/// Growth tiers 5–8 live past the hamlet's (garden/well/lanterns/banners at
/// 10/24/42/64 — `hearth.rs`). Each is a visible landmark of its own.
pub(crate) const TIER_DOCKS: u32 = 96;
pub(crate) const TIER_WINDMILL: u32 = 132;
pub(crate) const TIER_MARKET: u32 = 172;
pub(crate) const TIER_KEEP_TOWERS: u32 = 216;

/// The full town ladder: the hamlet's tiers 1–4 (`hearth.tier`) extended with
/// the realm's own structures 5–8. Everything growth-related keys on this —
/// the hearth's own ladder stays the hamlet's business.
pub(crate) fn town_tier(prosperity: u32) -> u32 {
    match prosperity {
        p if p >= TIER_KEEP_TOWERS => 8,
        p if p >= TIER_MARKET => 7,
        p if p >= TIER_WINDMILL => 6,
        p if p >= TIER_DOCKS => 5,
        p if p >= crate::stage::hearth::TIER_BANNERS => 4,
        p if p >= crate::stage::hearth::TIER_LANTERNS => 3,
        p if p >= crate::stage::hearth::TIER_WELL => 2,
        p if p >= crate::stage::hearth::TIER_GARDEN => 1,
        _ => 0,
    }
}

/// The realm's slow sky, folded from the persisted beat clock: fair spells,
/// drifting cloud decks, and soft drizzle. One fbm sample per beat — the
/// weather is continuous across sessions because the clock is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Weather {
    Fair,
    Clouds,
    Drizzle,
    Clearing,
}

/// Recovery is visible long enough to survive normal redraw pacing, but is
/// bounded in the persisted hearth clock rather than wall time.
pub(crate) const CLEARING_BEATS: u64 = 6;

impl Weather {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Weather::Fair => "fair",
            Weather::Clouds => "clouds",
            Weather::Drizzle => "drizzle",
            Weather::Clearing => "clearing",
        }
    }

    fn bucket(self) -> u32 {
        match self {
            Weather::Fair => 0,
            Weather::Clouds => 1,
            Weather::Drizzle => 2,
            Weather::Clearing => 3,
        }
    }
}

/// Weather at a given beat: a slow two-octave walk, salted per world so every
/// realm keeps its own climate. Fair is the mode — clouds and drizzle are
/// passing moods, like the quota storms but gentler.
pub(crate) fn weather_for_beats(beats: u64, seed: u64) -> Weather {
    let t = beats as f32;
    let walk = 0.68 * retro_kit::value_noise(t * 0.021, 0.5, seed ^ 0x0005_EA50)
        + 0.32 * retro_kit::value_noise(t * 0.093, 7.25, seed ^ 0xA117);
    if walk > 0.82 {
        Weather::Drizzle
    } else if walk > 0.62 {
        Weather::Clouds
    } else {
        Weather::Fair
    }
}

/// The coarse cache-bucket contribution: two weather bits ride the hearth's
/// render bucket so the terrain memo can never serve a stale sky.
pub(crate) fn weather_bucket(beats: u64, seed: u64) -> u32 {
    weather_for_beats(beats, seed).bucket()
}

/// A healthy fleet head walking the cottage↔granary path. The zero-allocation
/// Manhattan out-and-back is staggered by cottage order and driven only by the
/// slow render bucket, so equal worlds always produce equal positions.
pub(super) fn villager_walk_position(
    cottage: (usize, usize),
    granary: (usize, usize),
    cottage_index: usize,
    bucket: u64,
) -> (f32, f32) {
    let dx = granary.0 as i32 - cottage.0 as i32;
    let dy = granary.1 as i32 - cottage.1 as i32;
    let x_len = dx.unsigned_abs() as usize;
    let outward = x_len + dy.unsigned_abs() as usize;
    if outward == 0 {
        return (cottage.0 as f32 + 0.5, cottage.1 as f32 + 0.5);
    }
    let cycle = outward * 2;
    let mut step = (bucket as usize + cottage_index * 3) % cycle;
    if step > outward {
        step = cycle - step;
    }
    let x_steps = step.min(x_len) as i32;
    let y_steps = step.saturating_sub(x_len) as i32;
    (
        (cottage.0 as i32 + dx.signum() * x_steps) as f32 + 0.5,
        (cottage.1 as i32 + dy.signum() * y_steps) as f32 + 0.5,
    )
}

/// Golden-hour warmth: peaks in the dawn/dusk light band, absent at full day
/// and full night. A pure function of the daylight scalar, so identical cache
/// buckets always share one tint.
pub(crate) fn golden_amount(daylight: f32) -> f32 {
    (1.0 - (daylight - 0.765).abs() / 0.185).clamp(0.0, 1.0)
}

/// Night grade: how far this cell slides toward moon-blue as daylight falls
/// below the night band.
pub(crate) fn night_grade_amount(daylight: f32) -> f32 {
    ((0.64 - daylight) / 0.12).clamp(0.0, 1.0)
}

/// The deterministic build plan for the current tier: which growth structures
/// have sites, and where. Computed once per tier and cached — the island and
/// its landmarks never move, so only a tier crossing re-plans.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct GrowthLayout {
    /// Shore root of the jetty: a land tile touching water, nearest the anchor.
    docks: Option<((usize, usize), (i32, i32))>,
    /// A breezy rise for the windmill, a walk south-west of the anchor.
    windmill: Option<(usize, usize)>,
    /// Two market stalls on the keep's plaza.
    market: Option<(usize, usize)>,
}

impl World {
    /// Outcome weather overlays the pure hearth clock. Red can only worsen the
    /// clock's weather; it can never turn a naturally wet beat bright.
    pub(super) fn outcome_weather(&self) -> Weather {
        if self.clearing_until_beat > self.hearth.beats {
            return Weather::Clearing;
        }
        let clock = weather_for_beats(self.hearth.beats, self.seed);
        match self.outcome_err_streak {
            3.. => Weather::Drizzle,
            2 => match clock {
                Weather::Fair => Weather::Clouds,
                weather => weather,
            },
            _ => clock,
        }
    }

    /// Replace only the existing two weather bits in HearthState's render
    /// bucket. No finer outcome state enters terrain or cinematic caches.
    pub(super) fn outcome_hearth_render_bucket(&self) -> u32 {
        let clock_weather = weather_for_beats(self.hearth.beats, self.seed).bucket();
        let bucket = self.hearth.render_bucket(self.seed);
        bucket ^ ((clock_weather ^ self.outcome_weather().bucket()) << 8)
    }

    /// Where the town's growth anchors: the hamlet forge when the fleet is
    /// mirrored in, else the knight's keep.
    fn growth_anchor(&self) -> (usize, usize) {
        match &self.village {
            Some(v) => v.forge_pos,
            None => self.building_pos(Building::Keep),
        }
    }

    /// The plan for `tier` and its next construction site, memoized on the
    /// current tier (world geometry is static after generation, so the tier is
    /// the whole key).
    fn growth_layout(&self) -> GrowthLayout {
        let tier = life::town_tier(self.renown.min(u64::from(u32::MAX)) as u32);
        if let Some((cached_tier, layout)) = *self.growth_cache.borrow()
            && cached_tier == tier
        {
            return layout;
        }
        // Plan one rung ahead so the next structure has a stable site while it
        // is still scaffolding. Drawing remains gated by the earned tier.
        let layout = self.plan_growth(tier.saturating_add(1).min(8));
        *self.growth_cache.borrow_mut() = Some((tier, layout));
        layout
    }

    fn plan_growth(&self, tier: u32) -> GrowthLayout {
        let mut layout = GrowthLayout::default();
        if tier < 5 {
            return layout;
        }
        let anchor = self.growth_anchor();
        let mut taken: Vec<(usize, usize)> = self.buildings.iter().map(|&(_, p)| p).collect();
        taken.push(self.quintain);
        taken.push(anchor);

        // The dock roots itself on the land tile nearest the anchor that
        // touches open water; the jetty planks march from it toward the sea.
        // (cell, direction, score) — a one-off local; a named alias would not aid
        // readability here.
        #[allow(clippy::type_complexity)]
        let mut best: Option<((usize, usize), (i32, i32), f64)> = None;
        for y in 1..WORLD_H - 1 {
            for x in 1..WORLD_W - 1 {
                if !self.is_land(x, y) {
                    continue;
                }
                for dir in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x as i32 + dir.0, y as i32 + dir.1);
                    if nx < 0 || ny < 0 {
                        continue;
                    }
                    let (nxu, nyu) = (nx as usize, ny as usize);
                    if nxu >= WORLD_W || nyu >= WORLD_H {
                        continue;
                    }
                    if !matches!(self.at(nxu, nyu), Biome::Water | Biome::DeepWater) {
                        continue;
                    }
                    let d = (x as f64 - anchor.0 as f64).powi(2)
                        + ((y as f64 - anchor.1 as f64) * 2.0).powi(2);
                    if best.is_none_or(|(_, _, bd)| d < bd) {
                        best = Some(((x, y), dir, d));
                    }
                }
            }
        }
        if let Some((root, dir, _)) = best {
            layout.docks = Some((root, dir));
        }

        if tier >= 6 {
            let spot = self.nearest_land(
                anchor.0 as f64 - 6.0,
                (anchor.1 as f64 - 4.0).max(1.0),
                &taken,
            );
            taken.push(spot);
            layout.windmill = Some(spot);
        }
        if tier >= 7 {
            let keep = self.building_pos(Building::Keep);
            let spot = self.nearest_land(keep.0 as f64 + 2.0, keep.1 as f64 + 2.0, &taken);
            layout.market = Some(spot);
        }
        layout
    }

    /// Growth fanfare: a tier crossing announces itself once — fireworks over
    /// the town and a gold flash naming what rose. Session-side only; the
    /// persisted prosperity keeps boot-time worlds from re-announcing history.
    pub(crate) fn note_growth_announcements(&mut self) {
        let tier = life::town_tier(self.renown.min(u64::from(u32::MAX)) as u32);
        if tier <= self.growth_announced {
            return;
        }
        self.growth_announced = tier;
        let note = match tier {
            1 => "the town plants a garden",
            2 => "the town digs a well",
            3 => "lanterns light the street",
            4 => "banners fly over the hamlet",
            5 => "a little dock rides at anchor",
            6 => "a windmill turns on the hill",
            7 => "market stalls open on the plaza",
            _ => "the keep stands tall",
        };
        self.sparkle_until = self.tick + 30;
        self.light_fireworks(34);
        self.flash_gain(note.to_string(), 46);
    }

    /// First-person camera shifting: once the knight has settled and the
    /// arrival dissolve has played, the saddle camera sways in a slow arc —
    /// the facade keeps shifting against the sky instead of sitting frozen.
    /// Zero while travelling or pre-settle, so rides and dissolves stay true.
    pub(super) fn orbit_sway(&self) -> f32 {
        if self.riding() || self.settle_ticks < cinematics::ARRIVAL_HOLD_TICKS {
            return 0.0;
        }
        let ramp = (self.settle_ticks.min(180) as f32 / 180.0) * 0.30;
        ramp * (self.tick as f32 / 56.0).sin()
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/life__tests.rs"]
mod tests;
