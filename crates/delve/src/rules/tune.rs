//! The Delve's server knobs (`angel_net::knob`): numbers in the rules a
//! developer moves while playing, for everyone at once.
//!
//! They live in the run (`Run::tune`), so they reach every client with the
//! rest of the tick's state and a client's prediction steps its own knight
//! by the same pace the server does. At its defaults a `Tune` is today's
//! code exactly (`together_tune__tests`), and it is never written then: a
//! save and the wire carry it only once a knob has moved.

use super::{Enemy, EnemyKind, Grid, Hero, Mover, Projectile};
use std::collections::BTreeMap;
use angel_net::knob::{Knob, Side};
use serde::{Deserialize, Serialize};

/// The rules' tuned numbers. Defaults are the code's constants.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tune {
    /// A knight's walking pace, units a second (`HERO_SPEED`).
    pub hero_speed: f32,
    /// The kingdom's roads multiply it (`kingdom::ROAD_PACE`).
    pub road_pace: f32,
    /// A roll's speed out of the tuck (`ROLL_SPEED`).
    pub roll_speed: f32,
    /// How often knights attack, in percent: 200 attacks twice as often.
    pub fire_rate: u32,
    /// Knights' shots fly at this times their speed.
    pub shot_speed: f32,
    /// Every monster's step, times this.
    pub foe_pace: f32,
    /// What monsters deal, in percent.
    pub foe_hit: u32,
    /// Monsters' health, in percent. Never below 100: a party that finds
    /// it too easy gets more pressure, never weaker foes.
    pub foe_health: u32,
    /// A room's monsters, in percent of its roster: more join, never fewer.
    pub foe_count: u32,
    /// How much of the way to its knight the kingdom camera eases a tick.
    pub camera_follow: f32,
    /// After a hit, how long a knight can't be hurt again, in seconds
    /// (`HIT_WARD` ticks): it blinks while it lasts.
    pub hit_ward: f32,
}

use super::kingdom::{CAMERA_FOLLOW, ROAD_PACE};
use super::{HERO_SPEED, HZ, ROLL_SPEED};
use super::hero::HIT_WARD;

impl Default for Tune {
    fn default() -> Tune {
        Tune {
            hero_speed: HERO_SPEED,
            road_pace: ROAD_PACE,
            roll_speed: ROLL_SPEED,
            fire_rate: 100,
            shot_speed: 1.0,
            foe_pace: 1.0,
            foe_hit: 100,
            foe_health: 100,
            foe_count: 100,
            camera_follow: CAMERA_FOLLOW,
            hit_ward: HIT_WARD as f32 / HZ as f32,
        }
    }
}

/// A float as the panel shows it: six decimals, so 0.22 is not 0.2199999988.
fn shown(v: f32) -> f64 {
    (f64::from(v) * 1e6).round() / 1e6
}

impl Tune {
    pub fn is_default(&self) -> bool {
        *self == Tune::default()
    }

    /// Every knob, where it stands.
    pub fn knobs(&self) -> Vec<Knob> {
        let d = Tune::default();
        let server = |name, group, label, constant| Knob::new(name, group, label, constant, Side::Server);
        vec![
            server("hero_speed", "knight", "Walk pace (units/s)", "HERO_SPEED (crates/delve/src/rules.rs)")
                .range(4.0, 30.0, 0.5)
                .default(shown(d.hero_speed))
                .at(shown(self.hero_speed)),
            server("road_pace", "knight", "Road pace, times", "ROAD_PACE (crates/delve/src/rules/kingdom.rs)")
                .range(1.0, 3.0, 0.1)
                .default(shown(d.road_pace))
                .at(shown(self.road_pace)),
            server("roll_speed", "knight", "Roll speed (units/s)", "ROLL_SPEED (crates/delve/src/rules.rs)")
                .range(10.0, 50.0, 1.0)
                .default(shown(d.roll_speed))
                .at(shown(self.roll_speed)),
            server("fire_rate", "knight", "Attack rate (%)", "every weapon's cooldown (Hero::cooldown, rules/hero.rs)")
                .range(25.0, 400.0, 5.0)
                .default(f64::from(d.fire_rate))
                .at(f64::from(self.fire_rate)),
            server("shot_speed", "knight", "Shot speed, times", "each weapon's shot speed (rules/items.rs, the forge)")
                .range(0.25, 3.0, 0.05)
                .default(shown(d.shot_speed))
                .at(shown(self.shot_speed)),
            server("foe_pace", "foes", "Monster pace, times", "each monster's step (rules/monsters.rs and its kin)")
                .range(0.25, 3.0, 0.05)
                .default(shown(d.foe_pace))
                .at(shown(self.foe_pace)),
            server("foe_hit", "foes", "Monster damage (%)", "DEPTH_HIT and PARTY_HIT (crates/delve/src/rules/stakes.rs)")
                .range(25.0, 400.0, 5.0)
                .default(f64::from(d.foe_hit))
                .at(f64::from(self.foe_hit)),
            server("foe_health", "foes", "Monster health (%)", "DEPTH_HEALTH (crates/delve/src/rules/stakes.rs)")
                .range(100.0, 400.0, 5.0)
                .default(f64::from(d.foe_health))
                .at(f64::from(self.foe_health)),
            server("foe_count", "foes", "Monsters a room (%)", "each room's roster (rules/layout.rs)")
                .range(100.0, 300.0, 25.0)
                .default(f64::from(d.foe_count))
                .at(f64::from(self.foe_count)),
            server("camera_follow", "camera", "Kingdom camera easing", "follow_camera's 0.22 (crates/delve/src/rules/kingdom.rs)")
                .range(0.05, 1.0, 0.01)
                .default(shown(d.camera_follow))
                .at(shown(self.camera_follow)),
            server("hit_ward_s", "knight", "Ward after a hit (s)", "HIT_WARD (crates/delve/src/rules/hero.rs)")
                .range(0.0, 1.0, 0.05)
                .default(shown(d.hit_ward))
                .at(shown(self.hit_ward)),
        ]
    }

    /// Set one knob (already held to its bounds and step): whether it is one.
    pub fn set(&mut self, name: &str, value: f64) -> bool {
        let f = value as f32;
        let pct = value.round().max(0.0) as u32;
        match name {
            "hero_speed" => self.hero_speed = f,
            "road_pace" => self.road_pace = f,
            "roll_speed" => self.roll_speed = f,
            "fire_rate" => self.fire_rate = pct.max(1),
            "shot_speed" => self.shot_speed = f,
            "foe_pace" => self.foe_pace = f,
            "foe_hit" => self.foe_hit = pct,
            "foe_health" => self.foe_health = pct.max(100),
            "foe_count" => self.foe_count = pct.max(100),
            "camera_follow" => self.camera_follow = f,
            "hit_ward_s" => self.hit_ward = f.max(0.0),
            _ => return false,
        }
        true
    }

    /// The ward after a hit, in ticks: `HIT_WARD` at its default.
    pub fn hit_ward_ticks(&self) -> u32 {
        (self.hit_ward * HZ as f32).round().max(0.0) as u32
    }

    /// A cooldown of `ticks` at this attack rate: the same number at 100%.
    pub fn cooldown(&self, ticks: u32) -> u32 {
        if self.fire_rate == 100 || ticks == 0 {
            ticks
        } else {
            (ticks * 100 / self.fire_rate).max(1)
        }
    }

    /// A share in percent scaled by a percent knob: the same at 100%.
    pub fn percent(share: u32, knob: u32) -> u32 {
        if knob == 100 { share } else { share * knob / 100 }
    }

    /// Whether every number a run reads is a number: what a run read off a
    /// save or a wire must be before it plays.
    pub fn valid(&self) -> bool {
        [self.hero_speed, self.road_pace, self.roll_speed, self.shot_speed, self.foe_pace, self.camera_follow]
            .iter()
            .all(|v| v.is_finite() && *v > 0.0 && *v <= 100.0)
            && self.fire_rate > 0
            && self.foe_health >= 100
            && self.foe_count >= 100
            && self.hit_ward.is_finite()
            && (0.0..=1.0).contains(&self.hit_ward)
    }
}

/// A monster's step this tick longer or shorter by the pace knob: from
/// where it stood to where its own move put it, scaled. A move farther
/// than this in a tick (a leap, a burrow, a blink) stays as it was.
const LONGEST_STEP: f32 = 1.0;

pub(super) fn foe_step(enemy: &mut Enemy, from: (f32, f32), pace: f32, grid: &Grid) {
    let (dx, dy) = (enemy.x - from.0, enemy.y - from.1);
    let d = dx.hypot(dy);
    if d == 0.0 || d > LONGEST_STEP {
        return;
    }
    if pace > 1.0 {
        // On from where it got to, through the walls' rules.
        let mover = if enemy.kind.flies() { Mover::Flier } else { Mover::Walker };
        (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), (dx * (pace - 1.0), dy * (pace - 1.0)), enemy.radius(), mover);
    } else {
        // Part of the way it went, which was open.
        (enemy.x, enemy.y) = (from.0 + dx * pace, from.1 + dy * pace);
    }
}

/// The monsters a room's roster gains at `count` percent: its own kinds in
/// turn, never a guardian, a dragon or a pit tyrant.
pub(super) fn more_foes(roster: &[EnemyKind], count: u32) -> Vec<EnemyKind> {
    if count <= 100 {
        return Vec::new();
    }
    let kinds: Vec<EnemyKind> = roster
        .iter()
        .copied()
        .filter(|k| !matches!(k, EnemyKind::Dragon | EnemyKind::PitTyrant | EnemyKind::Boss))
        .collect();
    let extra = (roster.len() as u32 * (count - 100)).div_ceil(100) as usize;
    kinds.iter().copied().cycle().take(if kinds.is_empty() { 0 } else { extra }).collect()
}

/// After the knights' step: the attack-rate knob on each cooldown a knight
/// set this tick (`cooled` held them before it), and the shot-speed knob
/// on the knights' shots fired in it.
pub(super) fn tune_knights(players: &mut BTreeMap<u32, Hero>, tune: &Tune, cooled: &[(u32, u32)], shots: &mut [Projectile]) {
    for &(id, before) in cooled {
        if let Some(hero) = players.get_mut(&id) {
            let now = hero.fire_cooldown;
            // Fired: a cooldown set where it ran out (it counts down by one
            // a tick otherwise, or stands while the knight is held).
            if now != before && now > before.saturating_sub(1) {
                hero.fire_cooldown = tune.cooldown(now);
            }
        }
    }
    if tune.shot_speed != 1.0 {
        for shot in shots.iter_mut().filter(|s| !s.hostile) {
            shot.vx *= tune.shot_speed;
            shot.vy *= tune.shot_speed;
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_tune__tests.rs"]
mod tests;
