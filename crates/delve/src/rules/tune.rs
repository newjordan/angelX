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
    /// Shared room hit-stop: enabled, durations and cooldown in transport ticks.
    pub hitstop: u32,
    pub hitstop_boss_kill: u32,
    pub hitstop_elite_kill: u32,
    pub hitstop_phase: u32,
    pub hitstop_heavy: u32,
    pub hitstop_cooldown: u32,
    pub cinder_fall: u32,
    pub cinder_slide: u32,
    pub cinder_lock: u32,
    pub cinder_tell: u32,
    pub cinder_attack: u32,
    pub pennon_gravity: u32,
    pub pennon_jump: u32,
    pub pennon_run: u32,
    pub pennon_coyote: u32,
    pub pennon_buffer: u32,
    pub rafter_gravity: u32,
    pub rafter_jump: u32,
    pub rafter_run: u32,
    pub rafter_coyote: u32,
    pub rafter_buffer: u32,
    pub brass_stride: u32,
    pub brass_combo: u32,
    pub brass_tell: u32,
    pub brass_shove: u32,
    pub brass_stop: u32,
    pub latch_tell: u32,
    pub latch_window: u32,
    pub latch_rest: u32,
    pub latch_recover: u32,
    pub latch_stop: u32,
    pub moss_launch: u32,
    pub moss_nudge: u32,
    pub moss_bounce: u32,
    pub moss_gravity: u32,
    pub moss_flipper: u32,
    /// The Round Table adventures' five numbers each, in
    /// `roundtable::ADVENTURES` order (their own modules name and bound them).
    pub roundtable: [[u32; 5]; 5],

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
            hitstop: 1,
            hitstop_boss_kill: 6,
            hitstop_elite_kill: 3,
            hitstop_phase: 4,
            hitstop_heavy: 2,
            hitstop_cooldown: 14,
            cinder_fall: super::cinder::FALL,
            cinder_slide: super::cinder::SLIDE,
            cinder_lock: super::cinder::LOCK,
            cinder_tell: super::cinder::TELL,
            cinder_attack: super::cinder::ATTACK,
            pennon_gravity: super::pennon::GRAVITY,
            pennon_jump: super::pennon::JUMP,
            pennon_run: super::pennon::RUN,
            pennon_coyote: super::pennon::COYOTE,
            pennon_buffer: super::pennon::BUFFER,
            rafter_gravity: super::rafterwake::GRAVITY,
            rafter_jump: super::rafterwake::JUMP,
            rafter_run: super::rafterwake::RUN,
            rafter_coyote: super::rafterwake::COYOTE,
            rafter_buffer: super::rafterwake::BUFFER,
            brass_stride: super::brasswake::STRIDE,
            brass_combo: super::brasswake::COMBO,
            brass_tell: super::brasswake::TELL,
            brass_shove: super::brasswake::SHOVE,
            brass_stop: super::brasswake::STOP,
            latch_tell: super::latch::TELL,
            latch_window: super::latch::WINDOW,
            latch_rest: super::latch::REST,
            latch_recover: super::latch::RECOVER,
            latch_stop: super::latch::STOP,
            moss_launch: super::mossbell::LAUNCH,
            moss_nudge: super::mossbell::NUDGE,
            moss_bounce: super::mossbell::BOUNCE,
            moss_gravity: super::mossbell::GRAVITY,
            moss_flipper: super::mossbell::FLIPPER,
            roundtable: super::roundtable::starts(),
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
        let mut knobs = vec![
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
            server("cinder_fall", "Cinder Stack", "Fall (ticks/cell)", "cinder::FALL").range(6.0, 60.0, 1.0).default(d.cinder_fall as f64).at(self.cinder_fall as f64),
            server("cinder_slide", "Cinder Stack", "Slide repeat (ticks)", "cinder::SLIDE").range(2.0, 15.0, 1.0).default(d.cinder_slide as f64).at(self.cinder_slide as f64),
            server("cinder_lock", "Cinder Stack", "Grounded lock (ticks)", "cinder::LOCK").range(3.0, 30.0, 1.0).default(d.cinder_lock as f64).at(self.cinder_lock as f64),
            server("cinder_tell", "Cinder Stack", "Slag wind-up (ticks)", "cinder::TELL").range(23.0, 60.0, 1.0).default(d.cinder_tell as f64).at(self.cinder_tell as f64),
            server("cinder_attack", "Cinder Stack", "Slag rest (ticks)", "cinder::ATTACK").range(90.0, 300.0, 1.0).default(d.cinder_attack as f64).at(self.cinder_attack as f64),
            server("pennon_gravity", "Pennon Rush", "Gravity", "pennon::GRAVITY").range(80.0, 160.0, 1.0).default(d.pennon_gravity as f64).at(self.pennon_gravity as f64),
            server("pennon_jump", "Pennon Rush", "Jump impulse", "pennon::JUMP").range(1280.0, 1900.0, 1.0).default(d.pennon_jump as f64).at(self.pennon_jump as f64),
            server("pennon_run", "Pennon Rush", "Run speed", "pennon::RUN").range(384.0, 896.0, 1.0).default(d.pennon_run as f64).at(self.pennon_run as f64),
            server("pennon_coyote", "Pennon Rush", "Coyote ticks", "pennon::COYOTE").range(0.0, 8.0, 1.0).default(d.pennon_coyote as f64).at(self.pennon_coyote as f64),
            server("pennon_buffer", "Pennon Rush", "Jump buffer ticks", "pennon::BUFFER").range(1.0, 10.0, 1.0).default(d.pennon_buffer as f64).at(self.pennon_buffer as f64),
            server("rafter_gravity", "Rafterwake", "Gravity", "rafterwake::GRAVITY").range(80.0, 160.0, 1.0).default(d.rafter_gravity as f64).at(self.rafter_gravity as f64),
            server("rafter_jump", "Rafterwake", "Jump impulse", "rafterwake::JUMP").range(1280.0, 1900.0, 1.0).default(d.rafter_jump as f64).at(self.rafter_jump as f64),
            server("rafter_run", "Rafterwake", "Run speed", "rafterwake::RUN").range(384.0, 896.0, 1.0).default(d.rafter_run as f64).at(self.rafter_run as f64),
            server("rafter_coyote", "Rafterwake", "Coyote ticks", "rafterwake::COYOTE").range(0.0, 8.0, 1.0).default(d.rafter_coyote as f64).at(self.rafter_coyote as f64),
            server("rafter_buffer", "Rafterwake", "Jump buffer ticks", "rafterwake::BUFFER").range(1.0, 10.0, 1.0).default(d.rafter_buffer as f64).at(self.rafter_buffer as f64),
            server("brass_stride", "Brasswake", "Walking pixels", "brasswake::STRIDE").range(1.0, 4.0, 1.0).default(d.brass_stride as f64).at(self.brass_stride as f64),
            server("brass_combo", "Brasswake", "Punch spacing (ticks)", "brasswake::COMBO").range(10.0, 30.0, 1.0).default(d.brass_combo as f64).at(self.brass_combo as f64),
            server("brass_tell", "Brasswake", "Danger tell (ticks)", "brasswake::TELL").range(23.0, 60.0, 1.0).default(d.brass_tell as f64).at(self.brass_tell as f64),
            server("brass_shove", "Brasswake", "Hit shove (pixels)", "brasswake::SHOVE").range(4.0, 18.0, 1.0).default(d.brass_shove as f64).at(self.brass_shove as f64),
            server("brass_stop", "Brasswake", "Contact freeze (ticks)", "brasswake::STOP").range(2.0, 8.0, 1.0).default(d.brass_stop as f64).at(self.brass_stop as f64),
            server("latch_tell", "Latch Duel", "Wind-up (ticks)", "latch::TELL").range(23.0, 60.0, 1.0).default(d.latch_tell as f64).at(self.latch_tell as f64),
            server("latch_window", "Latch Duel", "Counter window (ticks)", "latch::WINDOW").range(2.0, 15.0, 1.0).default(d.latch_window as f64).at(self.latch_window as f64),
            server("latch_rest", "Latch Duel", "Breath (ticks)", "latch::REST").range(12.0, 60.0, 1.0).default(d.latch_rest as f64).at(self.latch_rest as f64),
            server("latch_recover", "Latch Duel", "Recovery (ticks)", "latch::RECOVER").range(9.0, 27.0, 1.0).default(d.latch_recover as f64).at(self.latch_recover as f64),
            server("latch_stop", "Latch Duel", "Hit freeze (ticks)", "latch::STOP").range(2.0, 8.0, 1.0).default(d.latch_stop as f64).at(self.latch_stop as f64),
            server("moss_flipper", "Mossbell", "Paddle lift", "mossbell::FLIPPER").range(800.0, 1800.0, 1.0).default(d.moss_flipper as f64).at(self.moss_flipper as f64),
            server("moss_gravity", "Mossbell", "Gravity", "mossbell::GRAVITY").range(16.0, 48.0, 1.0).default(d.moss_gravity as f64).at(self.moss_gravity as f64),
            server("moss_bounce", "Mossbell", "Bounce percent", "mossbell::BOUNCE").range(60.0, 95.0, 1.0).default(d.moss_bounce as f64).at(self.moss_bounce as f64),
            server("moss_nudge", "Mossbell", "Nudge", "mossbell::NUDGE").range(80.0, 320.0, 1.0).default(d.moss_nudge as f64).at(self.moss_nudge as f64),
            server("moss_launch", "Mossbell", "Launch", "mossbell::LAUNCH").range(1000.0, 1800.0, 1.0).default(d.moss_launch as f64).at(self.moss_launch as f64),
            server("hitstop", "hit-stop", "Room hit-stop on", "rules::hitstop").range(0.0, 1.0, 1.0).default(d.hitstop as f64).at(self.hitstop as f64),
            server("hitstop_boss_kill", "hit-stop", "Boss kill (ticks)", "rules::hitstop").range(0.0, 12.0, 1.0).default(d.hitstop_boss_kill as f64).at(self.hitstop_boss_kill as f64),
            server("hitstop_elite_kill", "hit-stop", "Large monster kill (ticks)", "rules::hitstop").range(0.0, 12.0, 1.0).default(d.hitstop_elite_kill as f64).at(self.hitstop_elite_kill as f64),
            server("hitstop_phase", "hit-stop", "Boss phase or stagger (ticks)", "rules::hitstop").range(0.0, 12.0, 1.0).default(d.hitstop_phase as f64).at(self.hitstop_phase as f64),
            server("hitstop_heavy", "hit-stop", "Heavy boss hit (ticks)", "rules::hitstop").range(0.0, 12.0, 1.0).default(d.hitstop_heavy as f64).at(self.hitstop_heavy as f64),
            server("hitstop_cooldown", "hit-stop", "Impact cooldown (ticks)", "rules::hitstop").range(0.0, 60.0, 1.0).default(d.hitstop_cooldown as f64).at(self.hitstop_cooldown as f64),
        ];
        knobs.extend(super::roundtable::knobs(self));
        knobs
    }

    /// Set one knob (already held to its bounds and step): whether it is one.
    pub fn set(&mut self, name: &str, value: f64) -> bool {
        let f = value as f32;
        let pct = value.round().max(0.0) as u32;
        match name {
            "hitstop" => self.hitstop = pct.min(1),
            "hitstop_boss_kill" => self.hitstop_boss_kill = pct.min(12),
            "hitstop_elite_kill" => self.hitstop_elite_kill = pct.min(12),
            "hitstop_phase" => self.hitstop_phase = pct.min(12),
            "hitstop_heavy" => self.hitstop_heavy = pct.min(12),
            "hitstop_cooldown" => self.hitstop_cooldown = pct.min(60),
            "moss_launch" => self.moss_launch = pct.clamp(1000, 1800),
            "moss_nudge" => self.moss_nudge = pct.clamp(80, 320),
            "moss_bounce" => self.moss_bounce = pct.clamp(60, 95),
            "moss_gravity" => self.moss_gravity = pct.clamp(16, 48),
            "moss_flipper" => self.moss_flipper = pct.clamp(800, 1800),
            "rafter_gravity" => self.rafter_gravity = pct.clamp(80, 160),
            "pennon_gravity" => self.pennon_gravity = pct.clamp(80, 160),
            "rafter_jump" => self.rafter_jump = pct.clamp(1280, 1900),
            "pennon_jump" => self.pennon_jump = pct.clamp(1280, 1900),
            "rafter_run" => self.rafter_run = pct.clamp(384, 896),
            "pennon_run" => self.pennon_run = pct.clamp(384, 896),
            "rafter_coyote" => self.rafter_coyote = pct.clamp(0, 8),
            "pennon_coyote" => self.pennon_coyote = pct.clamp(0, 8),
            "rafter_buffer" => self.rafter_buffer = pct.clamp(1, 10),
            "pennon_buffer" => self.pennon_buffer = pct.clamp(1, 10),
            "brass_stride" => self.brass_stride = pct.clamp(1, 4),
            "brass_combo" => self.brass_combo = pct.clamp(10, 30),
            "brass_tell" => self.brass_tell = pct.clamp(23, 60),
            "brass_shove" => self.brass_shove = pct.clamp(4, 18),
            "brass_stop" => self.brass_stop = pct.clamp(2, 8),
            "latch_tell" => self.latch_tell = pct.clamp(23, 60),
            "latch_window" => self.latch_window = pct.clamp(2, 15),
            "latch_rest" => self.latch_rest = pct.clamp(12, 60),
            "latch_recover" => self.latch_recover = pct.clamp(9, 27),
            "latch_stop" => self.latch_stop = pct.clamp(2, 8),

            "hero_speed" => self.hero_speed = f,
            "cinder_fall" => self.cinder_fall = pct.clamp(6, 60),
            "cinder_slide" => self.cinder_slide = pct.clamp(2, 15),
            "cinder_lock" => self.cinder_lock = pct.clamp(3, 30),
            "cinder_tell" => self.cinder_tell = pct.clamp(23, 60),
            "cinder_attack" => self.cinder_attack = pct.clamp(90, 300),
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
            _ => return super::roundtable::set(self, name, pct),
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
            && self.hitstop <= 1
            && [self.hitstop_boss_kill, self.hitstop_elite_kill, self.hitstop_phase, self.hitstop_heavy].iter().all(|&n| n <= 12)
            && self.hitstop_cooldown <= 60
            && (800..=1800).contains(&self.moss_flipper)
            && (16..=48).contains(&self.moss_gravity)
            && (60..=95).contains(&self.moss_bounce)
            && (80..=320).contains(&self.moss_nudge)
            && (1000..=1800).contains(&self.moss_launch)
            && (80..=160).contains(&self.pennon_gravity)
            && (1280..=1900).contains(&self.pennon_jump)
            && (384..=896).contains(&self.pennon_run)
            && (0..=8).contains(&self.pennon_coyote)
            && (1..=10).contains(&self.pennon_buffer)
            && (80..=160).contains(&self.rafter_gravity)
            && (1280..=1900).contains(&self.rafter_jump)
            && (384..=896).contains(&self.rafter_run)
            && (0..=8).contains(&self.rafter_coyote)
            && (1..=10).contains(&self.rafter_buffer)
            && (1..=4).contains(&self.brass_stride)
            && (10..=30).contains(&self.brass_combo)
            && (23..=60).contains(&self.brass_tell)
            && (4..=18).contains(&self.brass_shove)
            && (2..=8).contains(&self.brass_stop)
            && (23..=60).contains(&self.latch_tell)
            && (2..=15).contains(&self.latch_window)
            && (12..=60).contains(&self.latch_rest)
            && (9..=27).contains(&self.latch_recover)
            && (2..=8).contains(&self.latch_stop)
            && (6..=60).contains(&self.cinder_fall)
            && (2..=15).contains(&self.cinder_slide)
            && (3..=30).contains(&self.cinder_lock)
            && (23..=60).contains(&self.cinder_tell)
            && (90..=300).contains(&self.cinder_attack)
            && super::roundtable::numbers_valid(self)
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
