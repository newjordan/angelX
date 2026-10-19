//! Fixed-step, model-free dungeon crawl: knights against the monsters and
//! demons of a freshly generated dungeon. One step is 1/30 second; input
//! arrival never advances time. All entity counts and per-frame work are
//! bounded.
//!
//! The loop: walk into a room, its doors bar shut, fight what lives there,
//! pick up what it drops, take the stairs down. Each floor is new (see
//! `layout`), the third ends in the dragon's lair. Knights fire quick streams,
//! piercing rails and close fans; monsters answer with slow shots worth reading.

pub mod aerie;
pub mod aoe;
pub mod bodies;
#[cfg(test)]
#[path = "../tests/rules/together_audience__tests.rs"]
mod audience_retirement_tests;
pub mod barony;
pub mod bard;
pub mod peasant;
pub mod bestiary;
mod boss_ecology;
mod boss_gates;
mod boss_population;
mod pacing;
pub mod bosses;
pub mod bounties;
pub mod bowcountry;
pub mod cards;
pub mod cauldron;
pub mod cat;
pub mod chariot;
pub mod chatter;
pub mod classes;
pub mod command;
pub mod coastal;
pub mod crown;
pub mod college;
pub mod colosseum;
pub mod countryside;
pub mod critters;
pub mod dares;
pub mod depths;
pub mod desperate;
mod encounter_catalog;
pub mod exhibits;
pub mod exits;
mod mend;
pub mod feats;
pub mod fellowship;
pub mod feel;
pub mod floor_map;
pub mod foes;
pub mod folly;
pub mod folk;
pub mod fortune;
pub mod frogs;
pub mod gantry;
pub mod gear;
pub mod guide;
pub mod hazards;
mod hero;
pub mod hexer;
pub mod hippodrome;
pub mod hireling;
pub mod holdings;
pub mod hollow;
pub mod home;
pub mod hud;
pub mod hunters;
pub mod imps;
pub mod items;
pub mod joust;
pub mod keyring;
pub mod kingdom;
pub mod tune;
pub mod hitstop;
pub mod knights;
pub mod layout;
pub mod ledge;
pub mod lich;
pub mod looks;
mod loot;
pub mod merchant;
pub mod merlin;
pub mod mirror;
pub mod music;
mod monsters;
pub mod mummers;
pub mod outlaw;
pub mod overclass;
pub mod pastimes;
pub mod cinder;
pub mod latch;
pub mod brasswake;
pub mod rafterwake;
pub mod pennon;
pub mod mossbell;
pub mod patterns;
pub mod perils;
pub mod gauntlets;
pub mod phrasebook;
pub mod artistry;
pub mod combos;
pub mod cover;
pub mod questline;
pub mod pit;
pub mod pitstop;
pub mod pots;
pub mod hop;
pub mod activity;
pub mod fx;
pub mod map;
pub use fx::Fx;
pub mod race3d;
pub mod powder;
pub mod presence;
pub mod again;
pub use presence::Hold;
pub mod press;
pub mod race_wagers;
pub mod ranch;
pub mod raids;
pub mod rescues;
pub mod rooms;
mod risen;
pub mod runes;
pub mod script;
pub mod secrets;
pub mod shield;
pub mod signatures;
pub mod skills;
pub mod skulls;
pub mod spectacle;
mod spells;
pub mod spin;
pub mod stakes;
pub mod thief;
pub mod wetland;
pub mod swamp;
pub mod horrors;
pub mod windcountry;
pub mod wishcraft;
pub mod woodland;
use stakes::FURY;
pub mod floor_plates;
pub mod floor_stats;
pub mod forest;
pub mod forest_road;
// The Round Table adventures (docs/ROUNDTABLE-ADVENTURES.md): the shared shell,
// then one module each. Set in the prep commit; a builder adds no line here.
pub mod roundtable;

pub mod lanternward;

pub mod moonfall;

pub mod bannerpeak;

pub mod griffinwing;

pub mod lightgallop;
pub mod talents;
pub mod tavern;
pub mod tide;
pub mod nooks;
pub mod town;
pub mod tray;
pub mod trinkets;
pub mod trophies;
pub mod ults;
pub mod votes;
pub mod works;
pub mod school;
pub mod world;
pub mod yard;

use crate::realm::{Haul, Spoil, Spoils};
pub use boss_gates::Gates as BossGateState;
pub use bosses::Boss;
pub use cards::{Book, Card};
pub use hazards::{Rock, Trap, Waves};
pub use hero::Hero;
pub use layout::{COLS, DIRS, Floor, Pack, ROWS, Room, RoomKind, TILE_UNITS, Tile};
pub use loot::{Item, Rng, Weapon};
pub use monsters::{Enemy, EnemyKind};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const WIDTH: f32 = 48.0;
pub const HEIGHT: f32 = 28.0;
pub const HZ: u32 = 30;
/// Dragon Keep's floor: the dragon, and the old delve's end.
pub const FLOORS: u32 = 3;
/// The bottom of the deep below it: the Unknown.
pub const DEEPEST: u32 = 6;
/// The first floor of the deep below the dragon (the Archive): from here
/// down the dungeon stands up in 3D (`world3d::room`, docs/WORLD-3D.md
/// "Deeper floors").
pub const DEEP_FLOOR: u32 = 4;
pub const MAX_PROJECTILES: usize = 384;
/// The most shots one knight may have in flight at once: their arm's
/// volleys and what those fork and split into, however many wishes stack
/// (`tests/client_stacked_wishes__tests.rs`). Six knights at it leave
/// room under `MAX_PROJECTILES` for the monsters' (82 at most in that
/// bench); a plain arm keeps 5 aloft, the delve's own wish ladders stacked
/// on a handgonne 42 (95th in the hundred), on a bow 155.
pub const KNIGHT_SHOTS: usize = 48;
/// Forks and splits spawn only while their knight has fewer than this in
/// flight, so a fresh volley always finds room (a stacked bow's is 15).
pub const KNIGHT_CHILDREN: usize = 32;

/// Knight `id`'s shots in flight among `shots`.
pub fn in_flight(shots: &[Projectile], id: u32) -> usize {
    shots
        .iter()
        .filter(|p| !p.hostile && u32::from(p.traits.owner) == id)
        .count()
}

/// Of the shots loosed from `from` on, at most `room` fly: the volley's
/// middle, so a knight at `KNIGHT_SHOTS` still shoots where they aim.
fn fit_volley(shots: &mut Vec<Projectile>, from: usize, room: usize) {
    let loosed = shots.len().saturating_sub(from);
    if loosed <= room {
        return;
    }
    let cut = loosed - room;
    shots.truncate(shots.len() - (cut - cut / 2));
    shots.drain(from..from + cut / 2);
}
pub const MAX_BOMBS: u32 = 5;
pub const MAX_ARMOR: u32 = 2;
/// A party of six: the host and five invited friends. A host who plays no
/// knight (`Hero::absent`) leaves seats 2 to 6 for five friends.
pub const MAX_PLAYERS: u32 = 6;
/// A knight whose player is here but has pressed nothing for this long is
/// away too (`Run::away`): a host gone back to their code with the game
/// still on screen. (A player not here at all, a page closed or hidden, is
/// away from the moment the host's server says so: `Run::set_away`.) The
/// party neither waits on an away knight nor is moved by one; it carries
/// them.
pub const AWAY_TICKS: u32 = 20 * HZ;
const MAX_ITEMS: usize = 24;
const DT: f32 = 1.0 / HZ as f32;
/// How far apart a party of `count` stands in a row when it walks in: 2.4
/// a knight up to four, then no wider a row than those four make (the
/// doorway's clear ground), so a fifth and sixth stand closer.
pub(crate) fn stand_apart(count: f32) -> f32 {
    if count > 4.0 { 3.0 * 2.4 / (count - 1.0) } else { 2.4 }
}
pub const HERO_RADIUS: f32 = 0.42;
const HERO_SPEED: f32 = 13.0;
/// The dodge roll: a short tumble, untouchable throughout, then a rearm.
const ROLL_TICKS: u32 = 10;
const ROLL_SPEED: f32 = 27.0;
const ROLL_REARM: u32 = 36;
/// How much of its speed a knight keeps behind a raised shield.
const SHIELD_PACE: f32 = 0.45;
/// Mana: what a held wall drinks. It flows back after a short rest.
pub const MAX_MANA: f32 = 100.0;
const MANA_REST: u32 = HZ / 2;
const MANA_FLOW: f32 = 22.0;
/// A wall stands this far ahead of the knight holding it.
const WALL_REACH: f32 = 1.7;
const WALL_PACE: f32 = 0.6;

/// Ticks a knight walks one way with no aim held before facing it: half a
/// second.
pub const TURN_TO_WALK: u32 = HZ / 2;

/// Down to this floor a boss stands alone in its room: no camp troops,
/// escort or party extras come in with it (its own summons still do).
pub const BOSS_ALONE_TO: u32 = 3;

/// How far a boss rises from every knight in the room (units; a tile is
/// two): a party coming in through a door never stands on top of one.
pub const BOSS_CLEARANCE: f32 = 12.0;

/// What a knight's guard key did on a tick, for the sim to sound out.
#[derive(Clone, Copy, Debug, Default)]
struct Guarded {
    pub rolled: bool,
    pub spun: bool,
    pub wall_raised: bool,
    pub mana_out: bool,
    /// A spin's age on this tick, before it ran down: when its axes cut.
    pub spin_age: Option<u32>,
}

impl Hero {
    /// A knight's own keys, the first half of its going on a tick: the
    /// roll's rearm runs down, the walk and the aim are read, and the guard
    /// key acts (a roll starts, a shield or a wall goes up, a spin turns),
    /// a whirl keeps the spin going and mana rests. The sim steps it
    /// (`Run::play`), and a client drawing its own knight ahead steps it
    /// the same (`Run::predict`). Returns the walk.
    fn own_keys(&mut self, input: &Input, side: bool, shield_free: bool) -> ((f32, f32), Guarded) {
        self.flee_ticks = self.flee_ticks.saturating_sub(1);
        let input = if self.flee_ticks > 0 {
            &Input { move_x: input.move_x, move_y: input.move_y, aim_x: input.aim_x, aim_y: input.aim_y, ..Default::default() }
        } else { input };
        let mut guarded = Guarded::default();
        self.dash_cooldown = self.dash_cooldown.saturating_sub(1);
        // In the stocks a knight can aim and throw back, but not walk.
        let (mx, my) = if self.stocked {
            (0.0, 0.0)
        } else {
            unit(input.move_x as f32, input.move_y as f32)
        };
        let (ax, ay) = unit(input.aim_x as f32, input.aim_y as f32);
        let walk = if self.stocked { (0, 0) } else { (input.move_x.signum(), input.move_y.signum()) };
        if ax != 0.0 || ay != 0.0 || walk == (0, 0) || walk != self.walk_dir {
            self.walk_ticks = 0;
        } else {
            self.walk_ticks = self.walk_ticks.saturating_add(1);
        }
        self.walk_dir = if ax != 0.0 || ay != 0.0 { (0, 0) } else { walk };
        if ax != 0.0 || ay != 0.0 {
            self.aim_x = ax;
            self.aim_y = ay;
        } else if side && mx != 0.0 {
            // Side-on, a knight faces the way they run.
            (self.aim_x, self.aim_y) = (mx.signum(), 0.0);
        } else if self.walk_ticks >= TURN_TO_WALK {
            // Top-down, a knight who keeps walking one way with no aim held
            // turns to face it.
            (self.aim_x, self.aim_y) = (mx, my);
        }
        self.shielding = false;
        match self.guard {
            cards::Guard::Roll => {
                if input.dash && self.dash_cooldown == 0 && self.dash_ticks == 0 {
                    // Roll where you're going, or where you aim when standing.
                    let (rx, ry) = if side {
                        // Side-on, a roll runs along the ground.
                        let along = if mx != 0.0 { mx } else { self.aim_x };
                        (if along < 0.0 { -1.0 } else { 1.0 }, 0.0)
                    } else if mx != 0.0 || my != 0.0 {
                        (mx, my)
                    } else {
                        (self.aim_x, self.aim_y)
                    };
                    self.dash_ticks = ROLL_TICKS;
                    self.dash_x = rx;
                    self.dash_y = ry;
                    self.invulnerable = self.invulnerable.max(ROLL_TICKS + 1);
                    self.dash_cooldown = ROLL_REARM;
                    guarded.rolled = true;
                }
            }
            // A shield in the air is not in the hand.
            cards::Guard::Shield => self.shielding = input.dash && shield_free,
            cards::Guard::Spin => {
                if input.dash && self.dash_cooldown == 0 && self.spin == 0 {
                    self.spin = spin::TICKS;
                    self.dash_cooldown = spin::REARM;
                    guarded.spun = true;
                }
            }
            cards::Guard::Wall(wall) => {
                // Held while there is mana; once spent, it must be let go
                // and raised again.
                if !input.dash {
                    self.wall_spent = false;
                }
                let up = input.dash && !self.wall_spent && self.mana > 0.0;
                if up {
                    guarded.wall_raised = !self.walling;
                    self.mana = (self.mana - wall.drain as f32 * DT).max(0.0);
                    self.mana_rest = 0;
                    if self.mana <= 0.0 {
                        self.wall_spent = true;
                        guarded.mana_out = true;
                    }
                }
                self.walling = up && self.mana > 0.0;
            }
        }
        if self.whirl > 0 {
            self.whirl -= 1;
            if self.spin == 0 {
                self.spin = spin::TICKS;
            }
        }
        if self.spin > 0 {
            guarded.spin_age = Some(spin::TICKS - self.spin);
            self.spin -= 1;
        }
        if !self.walling {
            self.mana_rest = self.mana_rest.saturating_add(1);
            if self.mana_rest > MANA_REST {
                self.mana = (self.mana + MANA_FLOW * DT).min(MAX_MANA);
            }
        }
        ((mx, my), guarded)
    }

    /// The second half: a Fae Dagger's blink lands (played last tick:
    /// there, now), then the step this tick takes along `walk`, as
    /// (dx, dy, speed): a roll running down, fast out of the tuck and
    /// slowing as the knight comes up; a spin's, a shield's or a wall's
    /// pace; or the walk at `pace`, read where the knight then stands.
    /// Also whether it blinked.
    fn own_step(
        &mut self,
        (mx, my): (f32, f32),
        grid: &Grid,
        roll_speed: f32,
        pace: impl FnOnce(&Hero) -> f32,
    ) -> ((f32, f32, f32), bool) {
        let blinked = self.blink > 0;
        if blinked {
            items::blink(self, grid);
        }
        let pace = pace(self);
        let step = if self.flee_ticks > 0 {
            let (dx, dy) = if mx == 0.0 && my == 0.0 { (self.aim_x, self.aim_y) } else { (mx, my) };
            (dx, dy, pace * peasant::SPRINT_PACE)
        } else if self.dash_ticks > 0 {
            self.dash_ticks -= 1;
            let ease = 0.55 + 0.45 * self.dash_ticks as f32 / ROLL_TICKS as f32;
            (self.dash_x, self.dash_y, roll_speed * ease)
        } else if self.spin > 0 {
            (mx, my, pace * spin::PACE)
        } else if self.shielding {
            (mx, my, pace * SHIELD_PACE)
        } else if self.walling {
            (mx, my, pace * WALL_PACE)
        } else {
            (mx, my, pace)
        };
        (step, blinked)
    }
}
/// What a knight's ranged arm let fly on a tick it fired (`Hero::loose`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Loosed {
    /// A forged blade, thrown to come back.
    Throw,
    /// A forged weapon's bolts, this many.
    Bolts(u32),
    /// A plain arm's shots, this many, along its aim.
    Arm(u32, (f32, f32)),
}

impl Hero {
    /// The shots this knight's ranged arm looses on a tick its fire key
    /// fires, into `shots`, with its cooldown set: a forged weapon's thrown
    /// blade or its bolts, else the plain arm's (`Weapon::arms`), with every
    /// card, temper and volley it carries. Fists and a forged weapon's swing
    /// strike rather than shoot; the sim does those itself. The sim fires by
    /// it (`Run::play`), and so does a client drawing its own knight's shots
    /// ahead of the server (`Run::predict_fire`), so the two loose alike.
    /// `burning`: the bow's Fire Arrows are lit; `rng`: the tick's dice,
    /// for how a stream strays. `None` for a forged weapon that neither
    /// throws nor shoots. `flying`: the knight's shots in flight already
    /// (`in_flight`): what is loosed past `KNIGHT_SHOTS` stays in the
    /// quiver, the volley's middle flying (`fit_volley`).
    pub(super) fn loose(
        &mut self,
        id: u32,
        book: &Book,
        burning: bool,
        rng: u64,
        flying: usize,
        shots: &mut Vec<Projectile>,
    ) -> Option<Loosed> {
        let from = shots.len();
        let loosed = self.loose_all(id, book, burning, rng, shots);
        fit_volley(shots, from, KNIGHT_SHOTS.saturating_sub(flying));
        loosed
    }

    /// `loose`, every shot the arm and its cards make.
    fn loose_all(
        &mut self,
        id: u32,
        book: &Book,
        burning: bool,
        rng: u64,
        shots: &mut Vec<Projectile>,
    ) -> Option<Loosed> {
        if let Some(weapon) = &self.forged {
            if let Some(throw) = weapon.throw {
                // The blade flies out along the aim and comes back.
                self.fire_cooldown = self.cooldown(throw.every);
                self.swing = 6;
                self.sword = false;
                let (ux, uy) = (self.aim_x, self.aim_y);
                let out = (throw.range / BLADE_SPEED * HZ as f32).ceil() as u16;
                shots.push(Projectile {
                    x: self.x + ux * 0.6,
                    y: self.y + uy * 0.6,
                    vx: ux * BLADE_SPEED,
                    vy: uy * BLADE_SPEED,
                    hostile: false,
                    kind: Shot::Blade,
                    look: self.shot_look(book),
                    damage: self.scaled(throw.damage),
                    pierce: u8::MAX,
                    last_hit: None,
                    empowered: false,
                    ttl: u32::from(out) * 2 + 3 * HZ,
                    traits: ShotTraits {
                        back: 1,
                        owner: id as u8,
                        out,
                        ..ShotTraits::of(&self.bonus)
                    },
                });
                return Some(Loosed::Throw);
            }
            let bolt = weapon.bolt?;
            self.fire_cooldown = self.cooldown(bolt.every);
            // (Hero::volley, by field: the forged weapon is borrowed.)
            self.loosed = self.loosed.wrapping_add(1);
            let volley = self.bonus.volley > 0 && self.loosed % self.bonus.volley == 0;
            let dressed = self.shot_look(book);
            let count = weapon.shots() + self.bonus.shots + if volley { 2 } else { 0 };
            let traits = ShotTraits {
                owner: id as u8,
                ..ShotTraits::of(&self.bonus)
            };
            let arc = weapon
                .spread
                .map_or(0.0, |spread| spread.arc.to_radians())
                .max(if volley { VOLLEY_FAN } else { FAN } * (count - 1) as f32);
            let angle = self.aim_y.atan2(self.aim_x);
            // However it was forged, its shots fly fast.
            let speed = bolt.speed.max(cards::FORGED_SPEED);
            for i in 0..count {
                let theta = angle
                    + if count > 1 {
                        arc * (i as f32 / (count - 1) as f32 - 0.5)
                    } else {
                        0.0
                    };
                shots.push(Projectile {
                    x: self.x + theta.cos() * 0.6,
                    y: self.y + theta.sin() * 0.6,
                    vx: theta.cos() * speed,
                    vy: theta.sin() * speed,
                    hostile: false,
                    kind: Shot::Bolt,
                    damage: self.scaled(bolt.damage),
                    pierce: self.bonus.pierce as u8,
                    last_hit: None,
                    empowered: false,
                    ttl: (bolt.range / speed * HZ as f32).ceil() as u32,
                    look: dressed.clone().or_else(|| {
                        Some((weapon.colour.clone(), weapon.shape.clone()))
                    }),
                    traits: traits.clone(),
                });
            }
            return Some(Loosed::Bolts(count));
        }
        let arms = self.weapon.arms();
        let temper = self.temper.min(feel::TEMPER_MAX);
        self.fire_cooldown = self.cooldown(arms.cooldown);
        let volley = self.volley();
        let extra = self.bonus.shots + if volley { 2 } else { 0 };
        // A card's dressing first; else the class's own shot (a Bard's
        // notes, a Peasant's stones).
        let dressed = self.shot_look(book).or_else(|| match self.weapon {
            Weapon::Notes => Some(("gold".into(), "note".into())),
            Weapon::Stones => Some(("stone".into(), "stone".into())),
            _ => classes::shot_look(self),
        });
        let fan = if volley { VOLLEY_FAN } else { FAN };
        let base = ShotTraits {
            owner: id as u8,
            temper,
            fire: burning,
            arm: true,
            stone: self.weapon == Weapon::Stones,
            ..ShotTraits::of(&self.bonus)
        };
        // A stream strays a little, so it looks alive.
        let wobble = (rng % 2001) as f32 / 1000.0 - 1.0;
        let angle = self.aim_y.atan2(self.aim_x) + wobble * arms.jitter;
        let (ax, ay) = (angle.cos(), angle.sin());
        // Each pull: the arm's own shape, then tempering's.
        let mut pull: Vec<(f32, f32, ShotTraits)> = Vec::new();
        match self.weapon {
            Weapon::Fists => unreachable!("fists use the punch branch"),
            Weapon::Notes | Weapon::Stones => {
                let count = arms.shots + extra;
                for i in 0..count {
                    let k = i as f32 / (count - 1).max(1) as f32 - 0.5;
                    pull.push((angle + arms.arc.to_radians() * k, 0.0, base.clone()));
                }
            }
            Weapon::Handgonne => {
                // Each extra shot a card gives is another fan's worth.
                let balls =
                    (arms.shots + if temper >= 1 { 2 } else { 0 }) * (1 + extra);
                let arc = arms.arc.to_radians() + fan * extra as f32;
                let traits = ShotTraits {
                    bounce: base.bounce.max(u8::from(temper >= 2)),
                    ..base
                };
                for i in 0..balls {
                    let k = i as f32 / (balls - 1).max(1) as f32 - 0.5;
                    pull.push((angle + arc * k, 0.0, traits.clone()));
                }
            }
            Weapon::Bow | Weapon::Crossbow => {
                // A tempered bow looses streams side by side.
                let lanes: &[f32] = match (self.weapon, temper) {
                    (Weapon::Bow, 0) => &[0.0],
                    (Weapon::Bow, 1) => &[-0.3, 0.3],
                    (Weapon::Bow, _) => &[-0.55, 0.0, 0.55],
                    _ => &[0.0],
                };
                let count = 1 + extra;
                for &lane in lanes {
                    let traits = ShotTraits {
                        // The outer streams of a thrice-tempered bow home.
                        homing: if temper >= 3 && lane != 0.0 {
                            base.homing.max(2)
                        } else {
                            base.homing
                        },
                        // A thrice-tempered rail chains.
                        chain: if self.weapon == Weapon::Crossbow && temper >= 3 {
                            base.chain.max(2)
                        } else {
                            base.chain
                        },
                        ..base.clone()
                    };
                    for i in 0..count {
                        let theta = angle + fan * (i as f32 - (count - 1) as f32 / 2.0);
                        pull.push((theta, lane, traits.clone()));
                    }
                }
            }
        }
        let count = pull.len() as u32;
        for (theta, lane, traits) in pull {
            let (ux, uy) = (theta.cos(), theta.sin());
            shots.push(Projectile {
                x: self.x + ux * 0.6 - ay * lane,
                y: self.y + uy * 0.6 + ax * lane,
                vx: ux * arms.speed,
                vy: uy * arms.speed,
                hostile: false,
                kind: match self.weapon {
                    Weapon::Fists => unreachable!("fists use the punch branch"),
                    Weapon::Bow | Weapon::Notes | Weapon::Stones => Shot::Arrow,
                    Weapon::Crossbow => Shot::Bolt,
                    Weapon::Handgonne => Shot::Ball,
                },
                look: dressed.clone(),
                damage: self.scaled(arms.damage),
                pierce: arms.pierce.saturating_add(self.bonus.pierce as u8),
                last_hit: None,
                empowered: false,
                traits,
                ttl: arms.ttl,
            });
        }
        Some(Loosed::Arm(count, (ax, ay)))
    }
}
const BOMB_DAMAGE: u32 = 60;
/// A fallen knight rises at the stairs with this share of their health,
/// in percent.
pub const REVIVE_SHARE: u32 = 25;
/// How far round the knight a bomb clears the monsters' shots.
pub const BOMB_CLEAR: f32 = 9.0;
pub const BOMB_REARM: u32 = HZ;
/// The vigil, for a player stepping away: how far the statue mends, how
/// often, and how long its key rests after raising it and after waking.
pub const VIGIL_REACH: f32 = 7.0;
const VIGIL_MEND_EVERY: u32 = HZ / 3;
const VIGIL_SETTLE: u32 = HZ;
const VIGIL_REARM: u32 = 5 * HZ;
const PICKUP_REACH: f32 = 1.3;
/// How near a weapon must lie for the take key to swap to it: on it, or
/// beside it.
pub const TAKE_REACH: f32 = 2.4;
/// A held number key plays one card, then waits this long.
const PLAY_REARM: u32 = HZ / 2;
/// The fan between extra shots from held cards, in radians.
const FAN: f32 = 0.2;
/// A volley flies wider: three shots across about forty degrees.
const VOLLEY_FAN: f32 = 0.35;
/// How far a chain hit's spark reaches for the next monster.
const CHAIN_REACH: f32 = 6.5;
/// How long a burst's ring shows.
pub const BLAST_TICKS: u8 = 9;
/// How far a handgonne ball's burst reaches.
const BURST_REACH: f32 = 2.5;
/// How fast a thrown blade flies, out and back.
const BLADE_SPEED: f32 = 20.0;
/// A morningstar's orbit: its distance from the knight, its spin (radians
/// a second), how often it strikes what it touches, and how hard.
pub const ORBIT_RADIUS: f32 = 2.4;
pub const ORBIT_SPIN: f32 = 5.0;
/// Morningstars strike on a beat, every this many ticks: each monster they
/// touched since the last beat is struck once, as one stroke (`aoe`).
pub const ORBIT_EVERY: u32 = 10;
/// What each strike deals.
const ORBIT_DAMAGE: u32 = 12;
/// How fast a homing shot turns, radians a second per point of homing.
const HOMING_TURN: f32 = 2.2;
/// Every knight's sword: a close arc that also cuts shots out of the air.
const SWORD_DAMAGE: u32 = 30;
const SWORD_REACH: f32 = 2.2;
const SWORD_ARC: f32 = 130.0;
const SWORD_REARM: u32 = 18;
/// A Sanctuary's privy: how near its door a knight steps in (`pitstop`).
const PRIVY_REACH: f32 = 1.2;
/// A fresh room's monsters hold still and silent this long.
pub const TELEGRAPH: u32 = HZ;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// The picture tick for a duel counter or table stroke. Repeats clear it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latch_tick: Option<u64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leave_activity: bool,
    /// Menu held in a mini-game; the host counts consecutive fixed steps.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leave_held: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub activity_menu: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vote: Option<votes::Command>,
    pub move_x: i8,
    pub move_y: i8,
    pub aim_x: i8,
    pub aim_y: i8,
    pub fire: bool,
    pub dash: bool,
    pub bomb: bool,
    /// A sword swing: every knight carries a blade beside their weapon.
    #[serde(default)]
    pub swing: bool,
    /// Hand slot 1–4 to play; 0 plays nothing.
    #[serde(default)]
    pub play: u8,
    /// Spell slot 1–4; 0 casts nothing.
    #[serde(default)]
    pub cast: u8,
    /// The vigil key: its press turns the knight to warded stone, or wakes
    /// it. Left out when unpressed, so an older host reads the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vigil: bool,
    /// The ultimate's key (R): its press casts a full charge.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ult: bool,
    /// The take key (P): its press swaps to the weapon lying beside the
    /// knight. Left out when unpressed, so an older host reads the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub take: bool,
    /// The interact key (E): plates, levers, cages, the folk and their
    /// booths. Fire only shoots. Left out when unpressed, so an older host
    /// reads the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub interact: bool,
    /// The skill key held: 1 Fart Cloud, 2 Fire Pit, 3 Mines, 4 Barrier,
    /// 5 Shield Throw (0 none).
    /// Left out when unpressed, so an older host reads the rest.
    #[serde(default, skip_serializing_if = "skills::unpressed")]
    pub skill: u8,
    /// A fresh key press dismisses only this seat’s transition picture.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skip_plate: bool,
    /// Asks to go back to the kingdom once the run is over (`again`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub again: bool,
}

impl Input {
    pub fn valid(self) -> bool {
        [self.move_x, self.move_y, self.aim_x, self.aim_y]
            .into_iter()
            .all(|axis| (-1..=1).contains(&axis))
            && usize::from(self.play) <= cards::HAND
            && usize::from(self.cast) <= cards::SPELL_SLOTS
            && usize::from(self.skill) <= skills::SKILLS
            && self.vote.is_none_or(votes::Command::valid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Monsters in the room; its doors are barred.
    Fighting,
    /// The room is clear: doors open, loot to gather, stairs to find.
    Exploring,
    Won,
    Wiped,
}

/// What a projectile is, for its damage and its look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shot {
    /// A thrown blade, spinning out and back.
    Blade,
    Arrow,
    Bolt,
    Ball,
    Bone,
    Orb,
    Ember,
    /// Pilgrim's Arrow and Assassinate: one great shot.
    Sacred,
    /// The Hexer's bolt: it hurts nobody, and makes a frog of a knight.
    Hex,
    /// The Lich's frost: it chills, and Rimeleap leaps knight to knight.
    Frost,
    /// A heat-seeking missile.
    Missile,
    /// The monsters' pattern shots, one look to a delve (`patterns`): the
    /// Crypt's pale wisps, the Mines' ore, Dragon Keep's flames, the
    /// Archive's torn pages, the Fungal Deep's spores, the Unknown's eyes.
    Wisp,
    Ore,
    Flame,
    Page,
    Spore,
    Eye,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Projectile {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub hostile: bool,
    pub kind: Shot,
    #[serde(default)]
    pub look: Option<(String, String)>,
    pub damage: u32,
    pub pierce: u8,
    last_hit: Option<u32>,
    pub ttl: u32,
    /// Passed through a friend's wall: it hits harder and burns gold.
    #[serde(default)]
    pub empowered: bool,
    /// What the shooter's held cards gave this shot.
    #[serde(default)]
    pub traits: ShotTraits,
}

/// A knight's shot's gifts from held cards: walls it may bounce off, how
/// hard it curves toward monsters, how many it sparks on to, how much each
/// hit mends a friend. A monster's pattern shot carries its `path` here
/// too, and comes off a wall when it has a `bounce`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShotTraits {
    pub bounce: u8,
    pub homing: u8,
    pub chain: u8,
    pub mend: u8,
    /// A thrown blade: 1 flying out (`out` ticks left), 2 coming back to
    /// knight `owner`'s hand.
    #[serde(default)]
    pub back: u8,
    #[serde(default)]
    pub owner: u8,
    #[serde(default)]
    pub out: u16,
    /// Bursts where it hits: this percent of its damage to all around.
    #[serde(default)]
    pub burst: u8,
    /// Monsters it has struck so far, for the share the next one takes
    /// (`aoe`).
    #[serde(default)]
    pub struck: u8,
    /// Stuns what it hits for this many ticks.
    #[serde(default)]
    pub stun: u8,
    /// A monster's pattern shot: what it does after it leaves
    /// (`patterns::Path`); empty for every other shot.
    #[serde(default)]
    pub path: patterns::Path,
    /// Its one trick is spent: a monster's shot has grazed a knight, a
    /// knight's rail has turned off a flying shield (`feel`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub once: bool,
    /// How often the knight's arm was tempered when it left (`feel`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub temper: u8,
    /// Fire Arrows: it sets what it hits burning.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fire: bool,
    /// A plain arm's shot, rather than a spell or a forged bolt.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub arm: bool,
    /// A lobbed stone: gameplay identity is independent of cosmetic dressing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stone: bool,
    /// Foes this rail has passed through; each may be struck only once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pierced: Vec<u32>,
    /// Forks it has left: stopping on a monster, it splits in two (`fork`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fork: u8,
    /// Fart Bombs: what it hits gets a cloud (`skills::fart_hits`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fart: u8,
    /// What a wish made literal (`wishcraft`): a bigger shot, a harder
    /// shove, a burning, chilling or poisoning hit, a shot that comes back,
    /// a faster one (percent).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub size: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub knock: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub burn: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub chill: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub poison: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub boomerang: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub velocity: u8,
    /// Part of a guardian's charge-up (`signatures::tag`): it strikes as
    /// that charge-up's blow, once a knight (`Hero::blow`).
    #[serde(default, skip_serializing_if = "zero")]
    pub blow: u32,
    /// A monster's shot loosed by a knight in its body (`bodies`): a lob
    /// comes down on the monsters, a beam burns them, and what turns after
    /// someone turns after them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub borrowed: bool,
}

/// A forked shot's turn off its parent's line, each way (radians), and the
/// least it flies on for.
const FORK_TURN: f32 = 0.6;
const FORK_TTL: u32 = HZ;

fn zero(n: &u32) -> bool {
    *n == 0
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl ShotTraits {
    fn of(bonus: &cards::Bonus) -> ShotTraits {
        ShotTraits {
            bounce: bonus.bounce.min(8) as u8,
            homing: bonus.homing.min(8) as u8,
            chain: bonus.chain.min(8) as u8,
            mend: bonus.mend.min(16) as u8,
            burst: bonus.burst.min(100) as u8,
            fork: bonus.fork.min(3) as u8,
            fart: bonus.fart.min(3) as u8,
            size: bonus.size.min(3) as u8,
            knock: bonus.knock.min(3) as u8,
            burn: bonus.burn.min(3) as u8,
            chill: bonus.chill.min(3) as u8,
            poison: bonus.poison.min(3) as u8,
            boomerang: bonus.boomerang > 0,
            velocity: bonus.velocity.min(100) as u8,
            ..ShotTraits::default()
        }
    }
}

/// A chain hit's spark, from one monster to the next, for a few frames.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Spark {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub ttl: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    #[serde(default)]
    pub playtest: votes::State,
    pub raid_id: u64,
    /// Knights with nothing kept at home start in their smallclothes and
    /// find their gear (`gear`). The game sets it; bare runs start dressed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub naked_start: bool,
    pub tick: u64,
    /// Shared impact hold; transport ticks continue while room time stands.
    #[serde(default, skip_serializing_if = "hitstop::State::quiet")]
    pub hitstop: hitstop::State,
    pub phase: Phase,
    /// Cosmetic playback; never participates in travel or combat gates.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub floor_plates: BTreeMap<u32, floor_plates::Playing>,
    #[serde(default)]
    pub exits: exits::State,
    pub players: BTreeMap<u32, Hero>,
    pub enemies: Vec<Enemy>,
    /// An open floor's companies while the party is off its map, in a side
    /// room (`depths`): back where they were when it returns. The server's
    /// own (`wire::PRIVATE`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parked: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub score: u32,
    pub dungeon: Floor,
    /// None preserves the current floor of a legacy save; new floors get a graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss_gates: Option<boss_gates::Gates>,
    /// Snapshot of the durable loop site admitted at the start of this Delve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_site: Option<String>,
    /// Public, generic stand geometry only. Private provenance lives in Site.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settlement_exhibits: Vec<exhibits::Marker>,
    /// Index of the room the party stands in.
    pub at: usize,
    #[serde(default)]
    pub kingdom: kingdom::State,
    /// The knobs a developer moved (`tune`); written only once one has.
    #[serde(default, skip_serializing_if = "tune::Tune::is_default")]
    pub tune: tune::Tune,
    /// Spoils banked by this run and not yet handed to the realm.
    #[serde(default)]
    pub bank: Vec<Haul>,
    /// Delve packs whose floor this run cleared, not yet handed to the realm.
    #[serde(default)]
    pub reclaimed: Vec<Pack>,
    /// The Undercroft as the realm has built it, and the treasury it can
    /// spend: the cockpit's word, refreshed whenever either changes.
    #[serde(default)]
    pub home: home::Home,
    #[serde(default)]
    pub treasury: Spoils,
    /// Travelling peddler approach latch; never an estate purchase order.
    #[serde(default)]
    pub merchant: merchant::State,
    /// Purchases asked for at the Undercroft's plates, for the cockpit.
    #[serde(skip)]
    pub orders: Vec<home::Order>,
    /// Purchases at the surface peddler, for the cockpit to pay from the
    /// realm's treasury.
    #[serde(skip)]
    pub peddled: Vec<merchant::Purchase>,
    /// Which keepers had a knight beside them last tick (one bit each).
    #[serde(skip)]
    greeted: u8,
    /// How this delve goes, as Fortune's wheel decided it.
    #[serde(default)]
    pub mode: fortune::Mode,
    /// Every floor of this delve is laid open (`depths`), whatever the
    /// wheel says: the host's switch (`settings::Settings::open_floors`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open_floors: bool,
    /// The wheel's spin this delve, if it was pulled.
    #[serde(default)]
    pub spin: Option<fortune::Spin>,
    /// The spin whose result has been announced.
    #[serde(default)]
    mode_said: Option<u64>,
    /// The Collapse: the tick this floor falls in.
    #[serde(default)]
    pub collapse_at: Option<u64>,
    /// The tick the party came down into the deep ([`DEEP_FLOOR`] and
    /// below) from above it, for the beat a window plays as the dungeon
    /// stands up around it (`world3d::room::beat`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_beat: Option<u64>,
    /// Ultimates playing out: Trebuchet's shells, Stillhours, phantoms.
    #[serde(default)]
    pub strikes: Vec<ults::Strike>,
    #[serde(default)]
    pub spheres: Vec<ults::Sphere>,
    #[serde(default)]
    pub phantoms: Vec<ults::Phantom>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bard_songs: Vec<bard::Song>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scarecrows: Vec<peasant::Scarecrow>,
    /// Creatures cards summoned (`imps`).
    #[serde(default)]
    pub imps: Vec<imps::Imp>,
    /// The floor's cauldron, risen when its guardian fell (`cauldron`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cauldron: Option<cauldron::Cauldron>,
    /// The pack of the delve's first floor, for the floors below it.
    #[serde(default)]
    pub first: Pack,
    /// The light home, once the dragon is slain: step into it to go home
    /// with everything.
    #[serde(default)]
    pub light: Option<(f32, f32)>,
    /// A great end reached this tick, for the realm: a dragon, or the Grail.
    #[serde(skip)]
    pub triumph: Option<Triumph>,
    /// Achievements the game noticed this tick, for the realm to keep.
    #[serde(skip)]
    pub feats: Vec<&'static str>,
    /// A new achievement's banner: when, and which.
    #[serde(default)]
    pub banner: Option<(u64, String)>,
    /// Achievements earned down in this delve, in the order earned: kept
    /// quiet through the fighting and shown together when the match ends.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deeds: Vec<String>,
    /// A box just opened at the coffer: when, its grade, what was in it.
    #[serde(default)]
    pub unboxed: Option<(u64, feats::Tier, String)>,
    /// Slimes slain in this room, for a family pruned.
    #[serde(skip)]
    slimes_slain: u32,
    /// Grubbins' wares this delve (an empty id: sold).
    #[serde(default)]
    pub stall: Vec<String>,
    /// Ticks a knight has stood on the Winding Stair at home.
    #[serde(default)]
    pub descending: u32,
    /// The entrance that hold is for: stepping onto a neighbouring plate
    /// starts its own count.
    #[serde(skip)]
    descend_for: Option<&'static str>,
    /// Whoever waits in a cage on this floor.
    #[serde(default)]
    pub captive: Option<rescues::Captive>,
    /// Which of the people at home a knight is standing by (each speaks
    /// once per approach).
    #[serde(skip)]
    met: u8,
    /// This floor's folly room, and the folly when one is being played.
    #[serde(default)]
    pub folly_room: Option<usize>,
    #[serde(default)]
    pub folly: Option<folly::WaxMaze>,
    #[serde(default)]
    pub bats: Option<folly::BatShoot>,
    /// This floor's Keg Gantry hall, and the climb when one is on.
    #[serde(default)]
    pub gantry_room: Option<usize>,
    #[serde(default)]
    pub gantry: Option<gantry::Gantry>,
    #[serde(default)]
    pub powder: Option<powder::Powder>,
    /// Book Hop, while a hall plays it.
    #[serde(default)]
    pub hop: Option<hop::Hop>,
    /// This floor's race hall, and the race when one is on (the Shaft Run,
    /// the Ore Cart or the Griffin Run).
    #[serde(default)]
    pub race3d_room: Option<usize>,
    #[serde(default)]
    pub race3d: Option<race3d::Race3d>,
    /// The pots broken on this floor, by room (a bit per pot), and the
    /// shards of the last few lying about.
    #[serde(default)]
    pub pots_broken: BTreeMap<usize, u8>,
    /// The bays of a held floor's steward's hall used on this floor (a bit
    /// per bay): each gives its boon once a floor.
    #[serde(default)]
    pub holding_used: u16,
    /// The raid being fought in a holding's hall, if one is.
    #[serde(default)]
    pub holding_raid: Option<raids::RaidFight>,
    /// The lamp-oil thief on the Strand, while the coast errand wants it.
    #[serde(default)]
    pub crab: Option<coastal::Crab>,
    /// The badger cook's mushrooms, while the woods errand wants them.
    #[serde(default)]
    pub forage: Option<woodland::Forage>,
    /// The sluice valves' open ticks left, while the wetland errand wants them.
    #[serde(default)]
    pub valves: Option<[u32; 3]>,
    /// The pigeons with the Parliament's papers, while the minutes are out.
    #[serde(default)]
    pub pigeons: Option<aerie::Pigeons>,
    /// The goose and the picnic sail, while the wind-country job wants it.
    #[serde(default)]
    pub heist: Option<windcountry::Heist>,
    /// The town's stalls, tubs and anvil (`town`).
    #[serde(default)]
    pub town: town::Town,
    /// The floor's nook below ground and what is under way in it (`nooks`).
    #[serde(default)]
    pub nook: Option<nooks::Nook>,
    /// Broken pots' shards settling. Saves leave them out; the wire's tree
    /// carries them beside the run, for drawing (`wire::publish`).
    #[serde(default, skip_serializing)]
    pub pot_shards: Vec<(f32, f32, u32)>,
    /// Lady Tallow, Blaise's cat, when she has come along.
    #[serde(default)]
    pub cat: Option<cat::Cat>,
    /// Fortune's dare on this floor, and how many the party has kept this
    /// delve.
    #[serde(default)]
    pub dare: Option<dares::Dare>,
    #[serde(default)]
    pub dares_kept: u32,
    /// A power rune in this room (welling up a few seconds into a fight),
    /// and how many the party has taken this delve.
    #[serde(default)]
    pub rune: Option<runes::Rune>,
    #[serde(default)]
    pub runes_taken: u32,
    /// Knights hexed into frogs this delve.
    #[serde(default)]
    pub hexes: u32,
    /// Maud's round, drunk at the top of the stair for this delve.
    #[serde(default)]
    pub round: Option<String>,
    /// Sir Dinadan's song for this delve.
    #[serde(default)]
    pub song: Option<String>,
    /// Kills and moments since the cockpit last looked, for Wren's
    /// bounties. Saves leave them out (the cockpit journals the intents it
    /// keeps); the renderer reads them, so the wire's tree carries them
    /// beside the run (`wire::publish`).
    #[serde(skip)]
    pub marks: BTreeMap<String, u32>,
    /// The small secrets already lifted (`cup:{depth}`), and those
    /// the Herald has already hinted at: each happens once, saves included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifted: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hinted: Vec<String>,
    /// Sappers' kegs burning down, and hobs' bombs in the air.
    #[serde(default)]
    pub kegs: Vec<foes::Keg>,
    #[serde(default)]
    pub lobs: Vec<foes::Lob>,
    /// The Broodmarrows' children in flight.
    #[serde(default)]
    pub flings: Vec<foes::Fling>,
    /// The Whirlgraves' open wells.
    #[serde(default)]
    pub wells: Vec<foes::Well>,
    /// The Flesher's hooks in the air, and the Silkmother's webs.
    #[serde(default)]
    pub hooks: Vec<hunters::Hook>,
    #[serde(default)]
    pub webs: Vec<hunters::Web>,
    /// The Pit Tyrant's slams, gathering.
    #[serde(default)]
    pub slams: Vec<pit::Slam>,
    /// A guardian's Ravages, rippling and bursting.
    #[serde(default)]
    pub ravages: Vec<tide::Ravage>,
    /// Beaumains, hired for this delve.
    #[serde(default)]
    pub hireling: Option<hireling::Hireling>,
    /// A bout at the lists, while one is ridden.
    #[serde(default)]
    pub joust: Option<joust::Joust>,
    /// A chariot race at the Hippodrome, while one is run.
    #[serde(default)]
    pub chariot: Option<chariot::Chariot>,
    /// The clearing's shared games and personal-best ledger.
    #[serde(default)]
    pub forest: forest::Forest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forest_road: Option<forest_road::Session>,
    /// The Round Table adventures, while one is run: each a session in the
    /// Dark Forest's room, opened at its kingdom tile (`roundtable`).
    /// Lanternward (rules/lanternward.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanternward: Option<lanternward::Session>,
    /// Moonfall (rules/moonfall.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moonfall: Option<moonfall::Session>,
    /// Bannerpeak (rules/bannerpeak.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bannerpeak: Option<bannerpeak::Session>,
    /// Griffinwing (rules/griffinwing.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub griffinwing: Option<griffinwing::Session>,
    /// Lightgallop (rules/lightgallop.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lightgallop: Option<lightgallop::Session>,
    #[serde(default)]
    pub college: college::Dialogue,
    /// A master's board at a school banner, while a knight reads it.
    #[serde(default)]
    pub school: school::Board,
    /// The teachers' questlines: whose reading is open, and this tick's
    /// queued counter notes.
    #[serde(default)]
    pub questline: questline::Board,
    /// Shared, model-free festival round; records live in the realm's home.
    #[serde(default)]
    pub pastimes: pastimes::State,
    #[serde(default)]
    pub cinder: Option<cinder::Session>,
    #[serde(default)]
    pub latch: Option<latch::Session>,
    #[serde(default)]
    pub brasswake: Option<brasswake::Session>,
    #[serde(default)]
    pub rafterwake: Option<rafterwake::Session>,
    #[serde(default)]
    pub pennon: Option<pennon::Session>,
    /// Host-only input age allowance from the existing network measurement.
    #[serde(skip)]
    pub latch_input_age: BTreeMap<u32, u64>,
    #[serde(default)]
    pub mossbell: Option<mossbell::Session>,
    #[serde(skip)]
    build_hold: Option<(usize, u32, u32)>,
    /// Which knights (a bit each) held E at the signal hill last tick: a
    /// lever answers a press, not a hold.
    #[serde(skip)]
    lever_held: u8,
    /// The same for the new rooms (longbow country and the town).
    #[serde(skip)]
    rooms_held: u8,
    /// A knight stood by the groom last tick (he speaks once per approach).
    #[serde(skip)]
    groom_near: bool,
    /// A knight stood by King Brannoc last tick.
    #[serde(skip)]
    king_near: bool,
    /// The Hollow Ones' Black Holes, warning and pulling.
    #[serde(default)]
    pub holes: Vec<hollow::Hole>,
    /// The third company's globs, puddles and flashes.
    #[serde(default)]
    pub critters: critters::Field,
    /// The party's pit-stop times at the Sanctuary's privy.
    #[serde(default)]
    pub pits: pitstop::Board,
    /// Frogs about the room, and the figures they became in a Sanctuary.
    #[serde(default)]
    pub frogs: frogs::Field,
    /// The floors' guardians; its own copy, so a save replays.
    #[serde(default = "bosses::builtin")]
    pub bosses: Vec<Boss>,
    /// The cards this run can find; its own copy, so a save replays.
    #[serde(default)]
    pub book: Book,
    /// The last card taken or played, by whom and when, for the notice line.
    #[serde(default)]
    pub found: Option<(u64, u32, String)>,
    /// Moments the chorus may speak about, drained by the cockpit.
    #[serde(skip)]
    pub cues: Vec<String>,
    /// Sounds this step made, drained by the cockpit's mixer.
    #[serde(skip)]
    pub sounds: Vec<&'static str>,
    /// Hostile shots loosed since the run began, bursts included: what the
    /// boss census counts.
    #[serde(skip)]
    pub hostile_shots: u64,
    /// Ticks of screen shake left (a guardian's fall, a bomb).
    #[serde(default)]
    pub shake: u32,
    /// The last kill's tick and the kills since, for slay streaks.
    #[serde(skip)]
    streak: (u64, u32),
    /// A knight was hurt in this room (no flawless clear).
    #[serde(skip)]
    room_hurt: bool,
    #[serde(skip)]
    blooded: bool,
    /// Waves still to pour through this room's doorways.
    #[serde(default)]
    pub waves: Waves,
    /// This room's traps, and the rocks falling in it.
    #[serde(default)]
    pub traps: Vec<Trap>,
    #[serde(default)]
    pub rocks: Vec<Rock>,
    /// Spikes have bitten a knight in this room (said once).
    #[serde(skip)]
    spiked: bool,
    /// The skills' gas, fire, mines and barricades, the rooms' own perils,
    /// the Boss Key and the Wanted hall's showdown.
    #[serde(default)]
    pub works: works::Works,
    /// Wishes just given (`overclass`), for their sign, until the next
    /// fight. The wishes themselves are each knight's (`Hero::wishes`).
    #[serde(default)]
    pub window: Option<overclass::Window>,
    /// The wishes knights made in their own words this run, by wish id:
    /// who made it (knight, name). The card's `by` line and the sheet's
    /// YOUR WISHES are theirs (`overclass`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub makers: BTreeMap<String, (u32, String)>,
    /// Lines everyone is shown, drained by the server (`wire::take_notices`).
    #[serde(skip)]
    pub notices: Vec<String>,
    /// The demon hand's claims, the hell gate and its circles.
    #[serde(default)]
    pub spectacle: spectacle::Spectacle,
    /// The knights' grunts and few words, and when they last spoke.
    #[serde(skip)]
    pub chatter: chatter::Chatter,
    /// Chain hits' sparks, fading.
    #[serde(default)]
    pub sparks: Vec<Spark>,
    /// What this tick showed, for `present` to draw (`fx`).
    #[serde(skip)]
    pub fx: Vec<Fx>,
    /// Handgonne bursts: where, and ticks left.
    #[serde(default)]
    pub blasts: Vec<(f32, f32, u8)>,
    /// How the arms feel: sparks, stuck shafts, smoke, deaths, a room's
    /// end in coins (`feel`).
    #[serde(default, skip_serializing_if = "feel::Feel::quiet")]
    pub feel: feel::Feel,
    /// The floor as it is played, knight by knight (`floor_stats`). The
    /// server's own (`wire::PRIVATE`).
    #[serde(default)]
    pub floor_tally: floor_stats::Tally,
    /// The last floor closed: its results, as the plate shows them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_results: Option<floor_stats::Results>,
    /// Wishes just granted: to which knight, and when.
    #[serde(default)]
    pub boons: Vec<(u32, u64)>,
    /// A wish is being forged at the Sanctuary's dais (set by the cockpit
    /// while angelX writes it): its ring burns brighter.
    #[serde(default)]
    pub wishing: bool,
    /// The tick a Sanctuary opened around the party, for its sign.
    #[serde(default)]
    pub hallowed: Option<u64>,
    /// The stairs wait for a knight to step off them first: a guardian
    /// falling at your feet never drops you past the Sanctuary.
    #[serde(default)]
    stairs_held: bool,
    /// The room holds while the party arrives (`Run::hold`): its foes and
    /// their shots wait and no knight is hurt, until every player here has
    /// the room on screen, or nobody is here at all. The host's server sets
    /// it each tick; the rules alone never do.
    #[serde(default)]
    pub held: Hold,
    /// Knights standing in a doorway or on the stairs the rest of the
    /// party has yet to reach (`fellowship`): the way waits for everyone.
    #[serde(default, skip_serializing_if = "fellowship::State::is_empty")]
    pub fellowship: fellowship::State,
    /// How many knights the monsters' health was last scaled for
    /// (`Run::rescale_party`); 0 for every knight in the party, which is
    /// how a run is written while all of them are here.
    #[serde(default, skip_serializing_if = "zero")]
    scaled_for: u32,
    /// Once the run has fallen or is won: who has asked to go back to the
    /// kingdom, and when the party goes (`again::Again`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub again: Option<again::Again>,
    /// Each knight's ticks since it last pressed or held anything (`step`):
    /// the server's own, never saved nor sent.
    #[serde(skip)]
    idle: BTreeMap<u32, u32>,
    /// The server's own: never sent to a client (`wire::PRIVATE`), so a
    /// client's run reads them back as their defaults. A client never steps
    /// the rules.
    #[serde(default)]
    next_id: u32,
    #[serde(default = "unseeded")]
    rng: Rng,
    /// Sent: the renderer places the merchant's camp and the pots by it.
    #[serde(default)]
    seed: u64,
}

/// The dice of a run read without them: a client's copy (`wire::PRIVATE`).
fn unseeded() -> Rng {
    Rng::new(0)
}

/// A great end a delve reached, for the realm's tally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Triumph {
    /// The dragon slain.
    Dragon,
    /// The Unknown met, and the Grail found.
    Grail,
}

fn unit(x: f32, y: f32) -> (f32, f32) {
    let length = x.hypot(y);
    if length > 0.0 {
        (x / length, y / length)
    } else {
        (0.0, 0.0)
    }
}

/// Whether segment `p`–`q` crosses segment `a`–`b`.
fn crosses(p: (f32, f32), q: (f32, f32), a: (f32, f32), b: (f32, f32)) -> bool {
    let side = |o: (f32, f32), u: (f32, f32), v: (f32, f32)| {
        (u.0 - o.0) * (v.1 - o.1) - (u.1 - o.1) * (v.0 - o.0)
    };
    let (d1, d2) = (side(a, b, p), side(a, b, q));
    let (d3, d4) = (side(p, q, a), side(p, q, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn hit_segment(ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32, radius: f32) -> bool {
    let dx = bx - ax;
    let dy = by - ay;
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((x - ax) * dx + (y - ay) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (x - ax - t * dx).hypot(y - ay - t * dy) <= radius
}

/// A floor of the Delve for another view to walk (the loop's crawl in the
/// realm's 3D pane): floor `depth` of `pack`, laid out from `seed`.
pub fn crawl_floor(depth: u32, pack: Pack, seed: u64) -> Floor {
    layout::floor(depth, pack, &mut Rng::new(seed))
}

fn mix(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// Who is moving, for what stops them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mover {
    Hero,
    Walker,
    Flier,
    /// Walks as a walker does, but the bog does not hold it: the swamp's
    /// own horrors (`horrors`).
    Wader,
}

/// The current room's tiles as movement and shots see them.
struct Grid<'a> {
    room: &'a Room,
    barred: bool,
}

impl Grid<'_> {
    fn tile_at(&self, x: f32, y: f32) -> Tile {
        // Off the edge reads as the edge tile, so an open doorway lets a
        // knight step through it and out of the room.
        let col = ((x / TILE_UNITS).floor() as i32).clamp(0, self.room.cols as i32 - 1);
        let row = ((y / TILE_UNITS).floor() as i32).clamp(0, self.room.rows as i32 - 1);
        self.room.tile(col, row)
    }

    fn open(&self, x: f32, y: f32, mover: Mover) -> bool {
        match self.tile_at(x, y) {
            Tile::Floor | Tile::Stairs | Tile::Ledge | Tile::Bog => true,
            Tile::Wall | Tile::Block => false,
            Tile::Hazard => mover == Mover::Flier,
            Tile::Door => mover == Mover::Hero && !self.barred,
        }
    }

    fn clear(&self, x: f32, y: f32, r: f32, mover: Mover) -> bool {
        [(-r, -r), (r, -r), (-r, r), (r, r)]
            .into_iter()
            .all(|(dx, dy)| self.open(x + dx, y + dy, mover))
    }

    /// Move with wall sliding, one axis at a time. Feet in the bog go at
    /// `swamp::BOG_PACE`; wings and the swamp's own waders do not.
    fn slide(&self, (x, y): (f32, f32), (dx, dy): (f32, f32), r: f32, mover: Mover) -> (f32, f32) {
        let (dx, dy) = if matches!(mover, Mover::Hero | Mover::Walker)
            && self.tile_at(x, y) == Tile::Bog
        {
            (dx * swamp::BOG_PACE, dy * swamp::BOG_PACE)
        } else {
            (dx, dy)
        };
        let x = if self.clear(x + dx, y, r, mover) {
            x + dx
        } else {
            x
        };
        let y = if self.clear(x, y + dy, r, mover) {
            y + dy
        } else {
            y
        };
        (x, y)
    }

    fn stops_shot(&self, x: f32, y: f32) -> bool {
        if !(0.0..self.room.width()).contains(&x) || !(0.0..self.room.height()).contains(&y) {
            return true;
        }
        match self.tile_at(x, y) {
            Tile::Wall | Tile::Block => true,
            Tile::Door => self.barred,
            _ => false,
        }
    }

    /// A fast friendly shot stops at the first stone it crosses, including
    /// a thin wall between two open endpoints.
    fn shot_stop(&self, from: (f32, f32), to: (f32, f32)) -> Option<(f32, f32)> {
        self.shot_stop_body(from, to, 0.0)
    }

    /// A shot whose body is `body` units round its middle: stopped by stone
    /// at its middle, and by a prop (a tomb, a pillar, a shelf, a crate,
    /// furniture: anything a knight cannot walk into that is not the
    /// room's own wall or door) wherever its body touches one. What a
    /// knight walks over (water, lava, a pit, a plate) never stops it.
    fn stops_body(&self, x: f32, y: f32, body: f32) -> bool {
        self.stops_shot(x, y)
            || (body > 0.0
                && [(body, 0.0), (-body, 0.0), (0.0, body), (0.0, -body)]
                    .into_iter()
                    .any(|(dx, dy)| self.tile_at(x + dx, y + dy) == Tile::Block))
    }

    /// `shot_stop` for a shot with a body (`stops_body`): where it first
    /// touches stone or a prop on its way from `from` to `to`.
    fn shot_stop_body(&self, from: (f32, f32), to: (f32, f32), body: f32) -> Option<(f32, f32)> {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let steps = (dx.hypot(dy) / 0.25).ceil().clamp(1.0, 256.0) as u32;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let at = (from.0 + dx * t, from.1 + dy * t);
            if self.stops_body(at.0, at.1, body) {
                return Some(at);
            }
        }
        None
    }
}

impl Run {
    #[cfg(any(test, feature = "testing"))]
    pub fn settlement_walkable_for_test(&self, x: f32, y: f32) -> bool {
        Grid {
            room: self.room(),
            barred: self.barred(),
        }
        .clear(x, y, HERO_RADIUS, Mover::Hero)
    }

    pub fn valid_snapshot(&self) -> bool {
        (0..=DEEPEST).contains(&self.dungeon.depth)
            && self.kingdom.valid()
            && self.tune.valid()
            && self.hitstop.valid()
            && self.exits.valid(self.dungeon.rooms.len())
            && self
                .exits
                .outside
                .keys()
                .all(|id| self.players.contains_key(id))
            && self
                .players
                .values()
                .all(|h| h.camera.is_none_or(|(x, y)| x.is_finite() && y.is_finite()))
            && self.forest.valid()
            && self.forest_road.as_ref().is_none_or(|s| s.valid())
            && roundtable::valid(self)
            && self.college.valid()
            && self.home.college.valid()
            && self.home.school.valid()
            && self.school.valid()
            && self.pastimes.valid()
            && self.cinder.as_ref().is_none_or(|s| s.valid())
            && self.latch.as_ref().is_none_or(|s| s.valid())
            && self.brasswake.as_ref().is_none_or(|s| s.valid())
            && self.rafterwake.as_ref().is_none_or(|s| s.valid())
            && self.pennon.as_ref().is_none_or(|s| s.valid())
            && self.mossbell.as_ref().is_none_or(|s| s.valid())
            && self.home.countryside.valid()
            && self.home.privy_wall.len() <= pitstop::WALL
            && self.home.privy_wall.iter().all(|l| l.len() <= 64)
            && self.home.ranch.valid()
            && self.folly.as_ref().is_none_or(|f| f.valid() && f.room < self.dungeon.rooms.len())
            && self.bats.as_ref().is_none_or(|b| b.valid() && b.room < self.dungeon.rooms.len())
            && self.gantry.as_ref().is_none_or(|g| g.valid() && g.room < self.dungeon.rooms.len())
            && self.gantry_room.is_none_or(|i| i < self.dungeon.rooms.len())
            && self.powder.as_ref().is_none_or(|p| p.valid() && p.room < self.dungeon.rooms.len())
            && self.hop.as_ref().is_none_or(|h| h.valid() && h.room < self.dungeon.rooms.len())
            && self.race3d.as_ref().is_none_or(|r| r.valid() && r.room < self.dungeon.rooms.len())
            && self.race3d_room.is_none_or(|i| i < self.dungeon.rooms.len())
            && self.pots_broken.len() <= 64
            && self.pots_broken.keys().all(|&i| i < self.dungeon.rooms.len())
            && self.holding_used < 1 << holdings::SLOTS
            && self.holding_raid.as_ref().is_none_or(|f| f.valid(self.dungeon.rooms.len()))
            && self.home.holdings.valid()
            && self.crab.as_ref().is_none_or(coastal::Crab::valid)
            && self.forage.as_ref().is_none_or(woodland::Forage::valid)
            && self.valves.is_none_or(|v| v.iter().all(|&t| t <= wetland::VALVE_OPEN))
            && self.pigeons.as_ref().is_none_or(aerie::Pigeons::valid)
            && self.heist.as_ref().is_none_or(windcountry::Heist::valid)
            && self.joust.as_ref().is_none_or(joust::Joust::valid)
            && self.town.valid()
            && self.nook.as_ref().is_none_or(|n| n.valid(self.dungeon.rooms.len()))
            && self.players.values().all(|h| h.soak <= town::SOAK)
            && self.folly_room.is_none_or(|i| i < self.dungeon.rooms.len())
            && !self.dungeon.rooms.is_empty()
            && self.dungeon.rooms.len()
                <= if self.at_home_now() {
                    world::HOME_ROOM_LIMIT
                } else {
                    64
                }
            && self.at < self.dungeon.rooms.len()
            && self.settlement_exhibits.len() <= 32
            && (self.settlement_exhibits.is_empty() || self.settlement_site.is_some())
            && self.dungeon.rooms.iter().all(Room::valid_snapshot)
            && self.dungeon.open.as_ref().is_none_or(|o| o.valid(&self.dungeon.rooms))
            && self.parked.len() <= 128
            && self.dungeon.valid_secret()
            && self
                .light
                .is_none_or(|(x, y)| x.is_finite() && y.is_finite())
            && self.settlement_exhibits.iter().enumerate().all(|(i, m)| {
                m.valid(&self.dungeon)
                    && !self.settlement_exhibits[..i]
                        .iter()
                        .any(|prior| (prior.room, prior.col, prior.row) == (m.room, m.col, m.row))
            })
            && self.players.contains_key(&1)
            && self.players.len() <= MAX_PLAYERS as usize
            && self
                .players
                .iter()
                .all(|(id, h)| (1..=MAX_PLAYERS).contains(id) && h.x.is_finite() && h.y.is_finite())
            && self.enemies.len() <= 128
            && self.projectiles.len() <= MAX_PROJECTILES
            && self
                .projectiles
                .iter()
                .all(|p| p.traits.pierced.len() <= 128)
            && self.bosses.len() <= 64
            && self.enemies.iter().enumerate().all(|(i, e)| {
                e.boss
                    .is_none_or(|b| usize::from(b) < self.bosses.len() && e.kind == EnemyKind::Boss)
                    && e.signature.valid()
                    && e.hoard.len() <= boss_ecology::ROSTER_CAP
                    && !self.enemies[..i].iter().any(|a| a.id == e.id)
            })
            && self.valid_boss_gates()
            && self.works.valid()
            && self.merchant.valid()
            && self.feel.valid()
    }

    pub fn new(seed: u64, raid_id: u64, guest: Option<&str>) -> Self {
        let mut rng = Rng::new(mix(seed ^ raid_id.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        let pack = if rng.chance(50) {
            Pack::Crypt
        } else {
            Pack::Cavern
        };
        let open_floors = crate::settings::get().open_floors;
        let dungeon = if open_floors {
            layout::open_floor(1, pack, 1 + usize::from(guest.is_some()), &mut rng)
        } else {
            layout::floor(1, pack, &mut rng)
        };
        let mut players = BTreeMap::new();
        for (id, name) in std::iter::once((1, "You")).chain(guest.map(|name| (2, name))) {
            let x = if guest.is_some() {
                16.0 + id as f32 * 5.0
            } else {
                WIDTH / 2.0
            };
            players.insert(id, Hero::new(name, x, HEIGHT - 6.0));
        }
        let mut run = Self {
            playtest: votes::State::default(),
            raid_id,
            naked_start: false,
            questline: questline::Board::default(),
            kingdom: kingdom::State::default(),
            tick: 0,
            hitstop: Default::default(),
            phase: Phase::Exploring,
            floor_plates: BTreeMap::new(),
            players,
            enemies: Vec::new(),
            parked: Vec::new(),
            projectiles: Vec::new(),
            score: 0,
            dungeon,
            boss_gates: None,
            settlement_site: None,
            settlement_exhibits: Vec::new(),
            at: 0,
            bank: Vec::new(),
            reclaimed: Vec::new(),
            home: home::Home::default(),
            treasury: Spoils::default(),
            merchant: merchant::State::default(),
            orders: Vec::new(),
            peddled: Vec::new(),
            greeted: 0,
            mode: fortune::Mode::default(),
            open_floors,
            spin: None,
            mode_said: None,
            collapse_at: None,
            deep_beat: None,
            strikes: Vec::new(),
            spheres: Vec::new(),
            phantoms: Vec::new(),
            bard_songs: Vec::new(),
            scarecrows: Vec::new(),
            imps: Vec::new(),
            cauldron: None,
            kegs: Vec::new(),
            lobs: Vec::new(),
            flings: Vec::new(),
            wells: Vec::new(),
            hooks: Vec::new(),
            webs: Vec::new(),
            slams: Vec::new(),
            ravages: Vec::new(),
            hireling: None,
            joust: None,
            chariot: None,
            forest: forest::Forest::default(),
            forest_road: None,
            lanternward: None,

            moonfall: None,

            bannerpeak: None,

            griffinwing: None,

            lightgallop: None,

            college: college::Dialogue::default(),
            school: school::Board::default(),
            pastimes: pastimes::State::default(),
            cinder: None,
            latch: None,
            brasswake: None,
            rafterwake: None,
            pennon: None,
            latch_input_age: BTreeMap::new(),
            mossbell: None,
            exits: exits::State::default(),
            build_hold: None,
            lever_held: 0,
            rooms_held: 0,
            groom_near: false,
            king_near: false,
            holes: Vec::new(),
            critters: critters::Field::default(),
            pits: pitstop::Board::default(),
            frogs: frogs::Field::default(),
            first: pack,
            light: None,
            triumph: None,
            feats: Vec::new(),
            banner: None,
            deeds: Vec::new(),
            unboxed: None,
            slimes_slain: 0,
            stall: Vec::new(),
            descending: 0,
            descend_for: None,
            captive: None,
            met: 0,
            cat: None,
            folly_room: None,
            folly: None,
            bats: None,
            gantry_room: None,
            gantry: None,
            powder: None,
            hop: None,
            race3d_room: None,
            race3d: None,
            pots_broken: BTreeMap::new(),
            holding_used: 0,
            holding_raid: None,
            crab: None,
            forage: None,
            valves: None,
            pigeons: None,
            heist: None,
            town: town::Town::default(),
            nook: None,
            pot_shards: Vec::new(),
            dare: None,
            dares_kept: 0,
            rune: None,
            runes_taken: 0,
            hexes: 0,
            round: None,
            song: None,
            marks: BTreeMap::new(),
            lifted: Vec::new(),
            hinted: Vec::new(),
            bosses: bosses::builtin(),
            book: Book::builtin(),
            found: None,
            cues: vec!["run_start".into()],
            sounds: Vec::new(),
            hostile_shots: 0,
            shake: 0,
            streak: (0, 0),
            room_hurt: false,
            blooded: false,
            waves: Waves::default(),
            traps: Vec::new(),
            rocks: Vec::new(),
            spiked: false,
            works: works::Works::default(),
            window: None,
            makers: BTreeMap::new(),
            notices: Vec::new(),
            sparks: Vec::new(),
            fx: Vec::new(),
            blasts: Vec::new(),
            feel: feel::Feel::default(),
            floor_tally: floor_stats::Tally::default(),
            floor_results: None,
            boons: Vec::new(),
            spectacle: spectacle::Spectacle::default(),
            chatter: chatter::Chatter::default(),
            wishing: false,
            hallowed: None,
            stairs_held: false,
            held: Hold::Open,
            fellowship: fellowship::State::default(),
            scaled_for: 0,
            again: None,
            tune: tune::Tune::default(),
            idle: BTreeMap::new(),
            next_id: 0,
            rng,
            seed,
        };
        run.populate_boss_gates();
        run.stock_gear_cache();
        run.enter(0, None);
        run
    }

    pub fn active(&self) -> bool {
        matches!(self.phase, Phase::Fighting | Phase::Exploring)
    }

    pub fn floor(&self) -> u32 {
        self.dungeon.depth
    }

    pub fn room(&self) -> &Room {
        &self.dungeon.rooms[self.at]
    }

    /// Doors stay barred while anything in the room still lives.
    pub fn barred(&self) -> bool {
        // On an open map only the guardian's lock-in bars a door.
        if let Some(open) = self.open_here() {
            return open.chambers[usize::from(open.guardian)].sealed;
        }
        self.foes_left()
            || (self.phase == Phase::Fighting && self.waves.pending())
            || self.folly.as_ref().is_some_and(|f| f.room == self.at && !f.done)
            || self.bats.as_ref().is_some_and(|b| b.room == self.at && !b.done)
            || self.gantry.as_ref().is_some_and(|g| g.room == self.at && !g.done)
            || self.powder.as_ref().is_some_and(|p| p.room == self.at && !p.done)
            || self.hop.as_ref().is_some_and(|h| h.room == self.at && !h.done)
            || self.race3d.as_ref().is_some_and(|r| r.room == self.at && !r.done)
    }

    /// Something in the room still fights (the yard's quintains never do).
    pub fn foes_left(&self) -> bool {
        self.enemies.iter().any(|e| e.kind != EnemyKind::Dummy)
    }

    /// The party still stands in the Undercroft or the first floor's
    /// entrance room.
    pub fn at_entrance(&self) -> bool {
        self.dungeon.depth == 0 || (self.dungeon.depth == 1 && self.at == 0)
    }

    /// The last pickup, for a few seconds after it was taken.
    pub fn found_line(&self) -> Option<String> {
        let (at, id, card) = self.found.as_ref()?;
        (self.tick.saturating_sub(*at) < u64::from(3 * HZ)).then(|| {
            match self.players.get(id) {
                Some(hero) => format!("{}: {card}", hero.name),
                // Id 0 is the dungeon itself: a guardian falling.
                None => card.clone(),
            }
        })
    }

    /// Hand every knight's carried spoils to the bank: all of them, or half
    /// when the party fell or turned back.
    fn bank_all(&mut self, whole: bool, why: &str) {
        if whole { self.keep_all_gear(); }
        self.bank_coins();
        let pay = self.mode.spoils();
        for (&id, hero) in &mut self.players {
            let carried = std::mem::take(&mut hero.carried);
            let spoils = if whole { carried } else { carried.half() };
            let spoils = spoils.scaled(pay);
            if !spoils.is_empty() {
                self.bank.push(Haul {
                    hero: id,
                    spoils,
                    why: why.to_string(),
                });
            }
        }
    }

    /// The party turns back from an unfinished run: half comes home.
    pub fn retreat(&mut self) {
        if self.active() {
            self.keep_all_gear();
            self.bank_all(false, "the party turned back");
        }
    }

    /// Add a card to this run's book and drop one at knight `id`'s feet.
    pub fn add_card(&mut self, card: Card, id: u32) -> bool {
        let card_id = card.id.clone();
        let spell = card.kind == cards::Kind::Spell;
        let name = card.name.clone();
        if !self.book.insert(card) {
            return false;
        }
        if spell {
            let card = self.book.get(&card_id).unwrap();
            if self
                .players
                .get_mut(&id)
                .is_some_and(|hero| hero.equip_spell(card))
            {
                self.spell_flourish(id, &name);
                return true;
            }
            // Updating an equipped spell refreshes its rules, not its timer.
            if self
                .players
                .get(&id)
                .is_some_and(|hero| hero.spells.iter().flatten().any(|s| s == &card_id))
            {
                return true;
            }
        }
        if let Some(hero) = self.players.get(&id) {
            let (x, y) = (hero.x, hero.y + 2.0);
            let bottom = self.room().height() - 3.0;
            self.drop_item(card_id, x, y.min(bottom), Some(id));
        }
        true
    }

    /// Player 2 joins beside player 1, or takes a new name if already here.
    /// Monsters in the room grow to the party's size, as fresh rooms do.
    pub fn join(&mut self, name: &str) {
        self.join_seat(2, name);
    }

    /// Knight `id` (2 to `MAX_PLAYERS`) joins beside player 1, or takes a new name if
    /// already here.
    pub fn join_seat(&mut self, id: u32, name: &str) {
        if !(2..=MAX_PLAYERS).contains(&id) {
            return;
        }
        if let Some(hero) = self.players.get_mut(&id) {
            let name = Hero::new(name, 0.0, 0.0).name;
            if hero.name != name {
                self.playtest.seats.remove(&id);
            }
            let changed = hero.name != name;
            hero.name = name;
            if changed { self.restore_gear(id); self.stock_gear_cache(); }
            return;
        }
        let (x, y) = self
            .players
            .iter()
            .find(|(seat, _)| !self.exits.outside.contains_key(seat))
            .map_or((WIDTH / 2.0, HEIGHT - 6.0), |(_, h)| (h.x, h.y));
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let (x, y) = [(2.4, 0.0), (-2.4, 0.0), (0.0, 2.4), (0.0, -2.4)]
            .into_iter()
            .map(|(dx, dy)| (x + dx, y + dy))
            .find(|&(x, y)| grid.clear(x, y, HERO_RADIUS, Mover::Hero))
            .unwrap_or((x, y));
        let before = self.scaled();
        self.players.insert(id, Hero::new(name, x, y));
        self.restore_gear(id);
        self.stock_gear_cache();
        if let Some(plate) = self.floor_plate(Some(1)) {
            self.floor_plates.insert(id, plate);
        }
        self.rescale_party(before);
        self.deal_home(id);
        if !self.at_home_now() {
            self.harden();
        }
        self.wishes_for_newcomer(id);
        if self.party_len() >= 4 {
            self.notice("party_of_four");
        }
    }

    /// A knight leaves the party; what they dropped is free to take again.
    pub fn leave(&mut self, id: u32) {
        let before = self.scaled();
        self.floor_plates.remove(&id);
        self.exits.outside.remove(&id);
        self.playtest.seats.remove(&id);
        if let Some(race) = &mut self.race3d { race.riders.remove(&id); }
        self.keep_gear(id);
        let Some(hero) = self.players.remove(&id) else {
            return;
        };
        if !hero.carried.is_empty() {
            self.bank.push(Haul {
                hero: id,
                spoils: hero.carried,
                why: format!("{} went home", hero.name),
            });
        }
        for room in &mut self.dungeon.rooms {
            for item in &mut room.items {
                if item.held_off == Some(id) {
                    item.held_off = None;
                }
            }
        }
        self.rescale_party(before);
        if !self.players.is_empty()
            && self
                .players
                .keys()
                .all(|i| self.exits.outside.contains_key(i))
            && let Some(back) = self.exits.outside.values().next().cloned()
        {
            self.finish_activity_return(&back);
        }
    }

    /// The knights who count as the party: every one whose player is here
    /// (`Hero::away` and `Hero::absent` count for nothing: monsters are
    /// sized, and hit, for the players who are playing).
    pub fn party_len(&self) -> usize {
        self.players.values().filter(|h| !h.away && !h.absent).count()
    }

    /// The party size the monsters stand scaled for.
    fn scaled(&self) -> u32 {
        if self.scaled_for == 0 { (self.players.len() as u32).max(1) } else { self.scaled_for }
    }

    /// Monsters scaled from `before` knights to the party as it is now
    /// (`party_len`), once a knight joins, leaves, goes away or comes back.
    fn rescale_party(&mut self, before: u32) {
        let now = (self.party_len() as u32).max(1);
        if now != before.max(1) {
            self.rescale(before, now);
        }
        self.scaled_for = if now as usize == self.players.len() { 0 } else { now };
    }

    /// Monster health follows the party size, keeping each one's share of
    /// damage already dealt.
    fn rescale(&mut self, before: u32, now: u32) {
        let now = now.max(1);
        let before = before.max(1);
        let (was, is) = (
            stakes::party_fury(before as usize),
            stakes::party_fury(now as usize),
        );
        for enemy in &mut self.enemies {
            enemy.max_hp = enemy.base_hp() * now;
            enemy.hp = (enemy.hp * now).div_ceil(before).clamp(1, enemy.max_hp);
            enemy.fury = enemy.fury.saturating_sub(was).saturating_add(is);
        }
    }

    /// Walk the party into room `index` through its door `from` (the side
    /// they enter on), or onto a new floor's entrance with `None`.
    pub fn enter(&mut self, index: usize, from: Option<usize>) {
        let came_from = self.at;
        // Off an open floor's map, its companies wait where they are.
        if index != self.at && self.open_map() {
            self.parked = std::mem::take(&mut self.enemies).into_iter().filter(|e| e.hp > 0).collect();
        }
        self.remember_activity_entry(index);
        if self
            .dungeon
            .rooms
            .get(self.at)
            .is_some_and(|r| r.kind == RoomKind::Fortune)
            && index != self.at
        {
            if let Some(spin) = &mut self.spin {
                spin.settled = true;
                self.mode = fortune::Mode::WHEEL[usize::from(spin.wedge)];
                self.mode_said = Some(spin.started);
            }
        }
        self.joust = None;
        self.chariot = None;
        self.latch = None;
        self.brasswake = None;
        self.rafterwake = None;
        self.pennon = None;
        self.mossbell = None;
        self.forest.leave();
        self.forest_road = None;
        roundtable::clear(self);
        self.college = Default::default();
        self.pastimes.leave();
        self.build_hold = None;
        // Leaving/recreating a room discards its extra bodies, never refunds
        // provision. Active serialized bodies instead keep their journal IDs.
        if let Some(g) = &mut self.boss_gates {
            for e in &self.enemies {
                g.economy.retire(e.id);
            }
        }
        // Humphrey is never more than a room behind the party, and never
        // arrives mid-game: entering a room restarts his dwell.
        if self.dungeon.rooms[index].kind.in_world()
            || self.enemies.iter().any(|e| e.kind == EnemyKind::Risen)
        {
            self.home.countryside.humphrey_dig = risen::FOLLOW;
        }
        self.bank_coins();
        self.feel.clear_room();
        self.at = index;
        self.projectiles.clear();
        self.imps.clear();
        self.enemies.clear();
        self.strikes.clear();
        self.spheres.clear();
        self.phantoms.clear();
        self.bard_songs.clear();
        self.scarecrows.clear();
        for h in self.players.values_mut() { h.flee_ticks = 0; }
        self.kegs.clear();
        self.lobs.clear();
        self.flings.clear();
        self.wells.clear();
        self.hooks.clear();
        self.webs.clear();
        self.slams.clear();
        self.ravages.clear();
        self.holes.clear();
        self.critters.clear();
        self.frogs.wild.clear();
        self.frogs.shown.clear();
        self.slimes_slain = 0;
        if self.dungeon.rooms[index].kind != RoomKind::Lair {
            self.light = None;
        }
        let living: Vec<u32> = self.players.keys().copied().collect();
        let count = living.len() as f32;
        let (w, h) = (
            self.dungeon.rooms[index].width(),
            self.dungeon.rooms[index].height(),
        );
        // Onto an open map: out of the door just taken, or at the arrival.
        let landing = self
            .open_landing(index, came_from)
            .map(|at| self.open_spots(index, at, living.len()));
        for (slot, id) in living.into_iter().enumerate() {
            let offset = (slot as f32 - (count - 1.0) / 2.0) * stand_apart(count);
            let (x, y) = match from {
                _ if landing.is_some() => landing.as_ref().map_or((0.0, 0.0), |spots| spots[slot]),
                Some(0) => (w / 2.0 + offset, 3.3),
                Some(1) => (w - 3.3, h / 2.0 + offset),
                Some(2) => (w / 2.0 + offset, h - 3.3),
                Some(_) => (3.3, h / 2.0 + offset),
                // Home: on the rug below the Winding Stair's mouth.
                // Home: at the foot of the rug, a walk from the stair.
                None if self.dungeon.depth == 0 => (w / 2.0 + offset * 1.4, h - 4.5),
                None => (w / 2.0 + offset * 2.0, h - 6.0),
            };
            let hero = self.players.get_mut(&id).expect("listed hero");
            (hero.x, hero.y) = (x, y);
            // A skull dropped in the room before goes back to its bones.
            hero.skull_at = None;
            hero.invulnerable = hero.invulnerable.max(20);
            hero.dash_ticks = 0;
            hero.privy = 0;
            hero.pit.stop = None;
            hero.pit.result = None;
            // Dressed as they are: what they own, as their health allows.
            hero.dress();
            hero.gear_was = hero.gear;
            hero.pop = 0;
            hero.buying = 0;
            hero.buy_spent = false;
        }
        if matches!(
            self.dungeon.rooms[index].kind,
            RoomKind::Ledge | RoomKind::Wanted
        ) {
            self.arrive_side_on();
        }
        self.tallow_follows();
        self.hireling_follows();
        let sealed = self.guardian_locked();
        let first_visit = !self.dungeon.rooms[index].visited;
        if self.open_map() {
            self.enter_open(index, first_visit);
            return;
        }
        let folly_here = self.folly_room == Some(index)
            && !self.dungeon.rooms[index].cleared
            && !sealed
            && self.folly.is_none()
            && self.bats.is_none()
            && self.powder.is_none()
            && self.hop.is_none();
        if folly_here {
            self.dungeon.rooms[index].visited = true;
            // The halls' follies take turns down the delve: the Wax Maze,
            // the Belfry Shoot, then the Powder Room and Book Hop by turns.
            match self.dungeon.depth % 3 {
                1 => self.start_wax_maze(index, from),
                2 => self.start_bat_shoot(index),
                _ if self.dungeon.depth % 6 == 0 => self.start_book_hop(index),
                _ => self.start_powder(index),
            }
        }
        let gantry_here = self.gantry_room == Some(index)
            && !self.dungeon.rooms[index].cleared
            && !sealed
            && self.gantry.is_none();
        if gantry_here {
            self.dungeon.rooms[index].visited = true;
            self.start_gantry(index);
        }
        let race_here = self.race3d_room == Some(index)
            && !self.dungeon.rooms[index].cleared
            && !sealed
            && self.race3d.is_none();
        if race_here {
            self.dungeon.rooms[index].visited = true;
            self.start_race3d(index);
        }
        // A folly left mid-game and come back to plays on as it stood: its
        // room raises no company.
        let resumed = self.folly_running();
        let room = &mut self.dungeon.rooms[index];
        room.visited = true;
        if !room.cleared && !sealed && !folly_here && !gantry_here && !race_here && !resumed {
            let roster = room.roster.clone();
            let kind = room.kind;
            let pack = self.dungeon.pack;
            // The swamp's guardian is the Mire King, in its last hall only:
            // the stairs rooms above it are its toughest fights.
            let guarded = kind == RoomKind::Stairs
                && !(pack == Pack::Swamp && self.dungeon.depth < swamp::FLOORS);
            // A delve meets one of its pack's guardians, the same one for
            // the whole delve.
            let guardians: Vec<usize> = (0..self.bosses.len())
                .filter(|&i| self.bosses[i].only_in == pack)
                .collect();
            let pick = (mix(self.seed ^ self.raid_id.rotate_left(17))
                % guardians.len().max(1) as u64) as usize;
            if kind == RoomKind::Wanted {
                // The showdown's outlaw is raised by its own hall.
                self.raise_outlaw();
            } else if self
                .boss_gates
                .as_ref()
                .is_some_and(|g| g.economy.camp(index).is_some())
            {
                let (apex, defeated, gate) = {
                    let g = self.boss_gates.as_ref().unwrap();
                    (g.guardian_boss, g.guardian_defeated, g.guardian == index)
                };
                let troops = self
                    .boss_gates
                    .as_mut()
                    .unwrap()
                    .economy
                    .deploy(index)
                    .unwrap();
                let faction = self
                    .boss_gates
                    .as_ref()
                    .unwrap()
                    .economy
                    .camp(index)
                    .unwrap()
                    .faction;
                // A chief's camp is led by its toughest, an elite who carries
                // the Boss Key; the floor's one boss is its guardian.
                let carrier = self.boss_gates.as_ref().unwrap().key_carrier(index);
                // On the first floors that boss stands alone: its troops
                // never come, and the floor's books count them gone.
                let alone = self.dungeon.depth <= BOSS_ALONE_TO
                    && gate
                    && !defeated
                    && (kind == RoomKind::Lair || apex.is_some());
                // So does the key's carrier: on the first floors its camp is
                // its spoils, not its company (a carrier still to come).
                let lone_carrier = self.dungeon.depth <= BOSS_ALONE_TO
                    && carrier.is_some_and(|c| troops.iter().any(|&(slot, _, _)| slot == c));
                for (slot, kind, troop_faction) in troops {
                    if alone || (lone_carrier && carrier != Some(slot)) {
                        self.boss_gates.as_mut().unwrap().economy.casualty(index, slot);
                        continue;
                    }
                    self.spawn(kind);
                    if let Some(e) = self.enemies.last_mut() {
                        e.camp_slot = Some(slot);
                        e.faction = Some(troop_faction);
                        if carrier == Some(slot) {
                            e.raise_elite();
                        }
                    }
                }
                if gate && !defeated {
                    if kind == RoomKind::Lair {
                        self.spawn(EnemyKind::Dragon);
                        self.enemies.last_mut().unwrap().faction = Some(faction);
                    } else if let Some(boss) = apex {
                        self.spawn_boss(usize::from(boss));
                        self.enemies.last_mut().unwrap().faction = Some(faction);
                    }
                }
                // The hoard is the camp's whole company, not just the troops
                // this entry deployed: a chief met again after the party left
                // still carries the spoils.
                if alone || lone_carrier {
                    let hoard = self
                        .boss_gates
                        .as_ref()
                        .unwrap()
                        .economy
                        .camp(index)
                        .unwrap()
                        .roster
                        .clone();
                    let holder = if alone {
                        self.enemies.last_mut()
                    } else {
                        self.enemies.iter_mut().find(|e| e.camp_slot.is_some() && e.camp_slot == carrier)
                    };
                    if let Some(holder) = holder {
                        holder.hoard = hoard;
                    }
                }
            } else if guarded && let Some(&boss) = guardians.get(pick) {
                self.spawn_boss(boss);
                // A guardian keeps a lighter escort, below the first floors;
                // on them it stands alone with the escort's spoils.
                if self.dungeon.depth > BOSS_ALONE_TO {
                    for kind in roster.into_iter().take(2) {
                        self.spawn(kind);
                    }
                } else if let Some(guardian) = self.enemies.last_mut() {
                    guardian.hoard = roster.into_iter().take(2).collect();
                }
            } else {
                // The monsters-a-room knob: more of the room's own kinds
                // join it, never fewer.
                let more = tune::more_foes(&roster, self.tune.foe_count);
                for kind in roster {
                    if kind == EnemyKind::PitTyrant
                        && self
                            .boss_gates
                            .as_ref()
                            .is_some_and(|g| g.optional_pit == Some(index) && g.pit_defeated)
                    {
                        continue;
                    }
                    if kind != EnemyKind::Dragon
                        || !self
                            .boss_gates
                            .as_ref()
                            .is_some_and(|g| g.guardian == index && g.guardian_defeated)
                    {
                        self.spawn(kind);
                    }
                }
                for kind in more {
                    self.spawn(kind);
                }
            }
        }
        self.swarm_up();
        self.maybe_frog(index, first_visit);
        if !self.enemies.is_empty() && (self.boss_gates.is_none() || first_visit) {
            self.deepen();
            self.maybe_goblin();
        }
        if let Some(caged) = self
            .captive
            .as_ref()
            .filter(|c| c.room == index && c.freed.is_none())
        {
            self.cues.push(format!("caged:{}", caged.who));
        }
        if self.dungeon.rooms[index].kind == RoomKind::Yard {
            self.raise_quintains();
        }
        self.phase = if self.foes_left()
            || (!sealed
                && !self.room().cleared
                && self.boss_gates.as_ref().is_some_and(|g| {
                    g.economy.camp(index).is_some() || g.optional_pit == Some(index)
                })) {
            Phase::Fighting
        } else {
            Phase::Exploring
        };
        self.room_hurt = false;
        if self.phase == Phase::Fighting {
            self.sounds.push("door_seal");
            // A fight closes the window: wishes come between fights.
            self.window = None;
        }
        if let Some(boss) = self.enemies.iter().find_map(|e| e.boss) {
            let id = self.bosses[usize::from(boss)].id.clone();
            self.cues.push(format!("boss_rise:{id}"));
            self.sounds.push("boss_rise");
        } else if self.enemies.iter().any(|e| e.kind == EnemyKind::Dragon) {
            self.cues.push("lair".into());
        } else if self.enemies.iter().any(|e| e.kind == EnemyKind::PitTyrant) {
            self.cues.push("pit_rise".into());
            self.sounds.push("boss_rise");
        } else if self.phase == Phase::Fighting && self.room().great() {
            self.cues.push("great_hall".into());
        } else if self.phase == Phase::Fighting {
            self.cues.push("room_fight".into());
        }
        if sealed {
            self.rune = None;
            self.window = None;
            self.waves = Waves::default();
            self.traps.clear();
            self.rocks.clear();
            self.light = None;
            self.show_boss_lock();
        } else {
            self.place_rune();
            self.arm_room();
        }
        self.works_enter();
        self.notice_crack(first_visit);
        if self.room().kind == RoomKind::Sanctuary {
            self.hallow();
        }
    }

    /// Where knight `id`'s morningstars are this tick (top-down: a circle;
    /// side-on too).
    pub fn orbits(&self, id: u32) -> Vec<(f32, f32)> {
        self.orbits_at(id, self.tick)
    }

    /// Where knight `id`'s morningstars were (or are) at `tick`, about where
    /// the knight stands now.
    fn orbits_at(&self, id: u32, tick: u64) -> Vec<(f32, f32)> {
        let Some(hero) = self.players.get(&id).filter(|h| h.hp > 0 && !h.stone) else {
            return Vec::new();
        };
        let n = hero.bonus.orbit.min(cards::cap("orbit"));
        (0..n)
            .map(|k| {
                let a = tick as f32 * ORBIT_SPIN * DT
                    + k as f32 * std::f32::consts::TAU / n as f32
                    + id as f32;
                (
                    hero.x + a.cos() * ORBIT_RADIUS,
                    hero.y + a.sin() * ORBIT_RADIUS,
                )
            })
            .collect()
    }

    /// Morningstars knock monsters' shots out of the air on every tick, and
    /// strike on the beat: each monster they touched since the last beat,
    /// once, as one stroke from the knight (`aoe`).
    fn swing_orbits(&mut self) {
        let beat = self.tick.is_multiple_of(u64::from(ORBIT_EVERY));
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let balls = self.orbits(id);
            if balls.is_empty() {
                continue;
            }
            for &(bx, by) in &balls {
                for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
                    if (shot.x - bx).hypot(shot.y - by) <= 0.9 {
                        shot.ttl = 0;
                    }
                }
            }
            if !beat {
                continue;
            }
            let swept: Vec<(f32, f32)> = (0..u64::from(ORBIT_EVERY))
                .flat_map(|back| self.orbits_at(id, self.tick.saturating_sub(back)))
                .collect();
            let hero = &self.players[&id];
            let (at, damage) = ((hero.x, hero.y), hero.scaled(ORBIT_DAMAGE));
            let struck = aoe::strike(&mut self.enemies, at, damage, |e| {
                swept
                    .iter()
                    .any(|&(bx, by)| (e.x - bx).hypot(e.y - by) <= e.radius() + 0.8)
            });
            if !struck.is_empty() {
                self.sounds.push("hit");
            }
            for (i, damage) in struck {
                let enemy = &mut self.enemies[i];
                enemy.wits.foe = id;
                self.feel.credit(id, enemy.id, damage, self.tick);
            }
        }
    }

    /// Chain hits spark on: each to the nearest monsters within reach that
    /// it has not touched, for half its damage.
    fn spark(&mut self, chains: Vec<(u32, f32, f32, u32, u8, u8)>) {
        for (first, x, y, damage, links, owner) in chains {
            let mut touched = vec![first];
            let mut at = (x, y);
            for _ in 0..links {
                let next = self
                    .enemies
                    .iter_mut()
                    .filter(|e| e.hp > 0 && !touched.contains(&e.id))
                    .filter(|e| (e.x - at.0).hypot(e.y - at.1) <= CHAIN_REACH)
                    .min_by(|a, b| {
                        (a.x - at.0)
                            .hypot(a.y - at.1)
                            .total_cmp(&(b.x - at.0).hypot(b.y - at.1))
                    });
                let Some(enemy) = next else {
                    break;
                };
                let actual = damage.max(1).min(enemy.hp);
                enemy.hp = enemy.hp.saturating_sub(damage.max(1));
                enemy.wits.foe = u32::from(owner);
                self.feel
                    .credit(u32::from(owner), enemy.id, actual, self.tick);
                touched.push(enemy.id);
                self.fx.push(Fx::Spark(Spark {
                    from: at,
                    to: (enemy.x, enemy.y),
                    ttl: 6,
                }));
                at = (enemy.x, enemy.y);
            }
        }
    }

    /// A quiet room: wounds close a little, mana fills, and the altar's
    /// sign says what it is for.
    fn hallow(&mut self) {
        // The well of the floor: a knight in a borrowed body is themselves.
        self.unpossess_all();
        for hero in self.players.values_mut() {
            if hero.hp > 0 {
                hero.mend(hero.max_hp / 5, stakes::Mend::Rest);
            }
            hero.mana = MAX_MANA;
        }
        self.hallowed = Some(self.tick);
        self.cues.push("sanctuary".into());
        self.frogs_arrive();
    }

    /// Where a boss (or the dragon) rises in the room: at the head of the
    /// room, unless the party has just come in there. Then the far end, or
    /// a quarter in from a side, whichever first lies [`BOSS_CLEARANCE`]
    /// from every knight;
    /// failing all of them, the one farthest from the nearest knight.
    fn boss_anchor(&self) -> (f32, f32) {
        let room = self.room();
        let (w, h) = (room.width(), room.height());
        let anchors = [(w / 2.0, 7.0), (w / 2.0, h - 7.0), (w / 4.0, h / 2.0), (w * 3.0 / 4.0, h / 2.0)];
        let nearest = |(x, y): (f32, f32)| {
            self.players
                .values()
                .filter(|hero| !hero.absent)
                .map(|hero| (hero.x - x).hypot(hero.y - y))
                .fold(f32::MAX, f32::min)
        };
        anchors
            .iter()
            .copied()
            .find(|&a| nearest(a) >= BOSS_CLEARANCE)
            .unwrap_or_else(|| {
                anchors.iter().copied().max_by(|&a, &b| nearest(a).total_cmp(&nearest(b))).unwrap_or(anchors[0])
            })
    }

    /// Place a monster on open floor, well away from the knights.
    fn spawn(&mut self, kind: EnemyKind) {
        if self.side_on() {
            let (x, y) = self.perch(kind.flies());
            if self.boss_gates.as_ref().is_some_and(|g| {
                g.economy.camp(self.at).is_none() && !g.encounter(self.at, kind, None)
            }) {
                // Non-camp initial rosters (entrance/ledge/etc.) also use durable
                // provision, not a raw reentry path around finite accounting.
                self.spawn_at(kind, x, y);
            } else {
                self.spawn_body(kind, x, y);
            }
            return;
        }
        // On an open map a newcomer comes into the party's chamber, awake.
        if self.open_map() {
            if let Some(((x, y), home)) = self.open_spawn_spot()
                && self.spawn_body(kind, x, y).is_some()
                && let Some(e) = self.enemies.last_mut()
            {
                e.home = Some(home);
            }
            return;
        }
        let room = &self.dungeon.rooms[self.at];
        let (x, y) = if kind == EnemyKind::Dragon {
            self.boss_anchor()
        } else {
            let mut spot = (room.width() / 2.0, 6.0);
            for attempt in 0..64 {
                let col = 2 + self.rng.below(room.cols - 4) as i32;
                let row = 2 + self.rng.below(room.rows - 4) as i32;
                let (x, y) = (
                    (col as f32 + 0.5) * TILE_UNITS,
                    (row as f32 + 0.5) * TILE_UNITS,
                );
                let far = self
                    .players
                    .values()
                    .all(|hero| (hero.x - x).hypot(hero.y - y) > 12.0 - attempt as f32 / 8.0);
                let taken = self
                    .enemies
                    .iter()
                    .any(|e| (e.x - x).abs() < 1.0 && (e.y - y).abs() < 1.0);
                if room.tile(col, row).ground() && far && !taken {
                    spot = (x, y);
                    break;
                }
            }
            spot
        };
        if self
            .boss_gates
            .as_ref()
            .is_some_and(|g| g.economy.camp(self.at).is_none() && !g.encounter(self.at, kind, None))
        {
            // Non-camp initial rosters (entrance/ledge/etc.) also use durable
            // provision, not a raw reentry path around finite accounting.
            self.spawn_at(kind, x, y);
        } else {
            self.spawn_body(kind, x, y);
        }
    }

    /// A monster at a chosen spot, sized to the party.
    fn spawn_at(&mut self, kind: EnemyKind, x: f32, y: f32) -> Option<u32> {
        if self.boss_gates.is_some() {
            self.spawn_staged(kind, x, y, 0)
        } else {
            self.spawn_body(kind, x, y)
        }
    }

    /// Raw body insertion, for admitted roster/encounter bodies or a provisioned
    /// emission. Callers account only the returned successful insertion.
    fn spawn_body(&mut self, kind: EnemyKind, x: f32, y: f32) -> Option<u32> {
        if self.boss_gates.is_some() && self.enemies.len() >= boss_ecology::ENTITY_CAP {
            return None;
        }
        // Giant's Feast: every ordinary monster a giant.
        let giant = self.mode == fortune::Mode::GiantsFeast
            && !matches!(kind, EnemyKind::Dragon | EnemyKind::Boss);
        // The floor and the party's strength set how tough it is.
        let base = (if giant { kind.hp() * 9 / 5 } else { kind.hp() }) * self.toughness() / 100;
        let hp = base * self.party_len().max(1) as u32;
        self.next_id = self.next_id.checked_add(1)?;
        self.enemies.push(Enemy {
            hoard: Vec::new(),
            faction: None,
            camp_slot: None,
            x,
            y,
            hp,
            max_hp: hp,
            kind,
            age: 0,
            id: self.next_id,
            origin_x: x,
            origin_y: y,
            boss: None,
            size: giant.then(|| kind.radius() * 1.4),
            base_hp: Some(base),
            raged: false,
            hurt: 0,
            seen_hp: hp,
            dread: 0,
            elite: false,
            stash: Default::default(),
            loot: Vec::new(),
            cry: 0,
            fury: FURY[(self.dungeon.depth.clamp(1, 6) - 1) as usize]
                + stakes::party_fury(self.party_len()),
            rung: 0,
            chilled: 0,
            pattern_volleys: 0,
            frozen: 0,
            sleep_until: 0,
            stage: 0,
            timer: 0,
            dir: (0.0, 0.0),
            guard: 0,
            wits: press::Wits::default(),
            signature: signatures::State::default(),
            home: None,
            dormant: false,
        });
        Some(self.next_id)
    }

    /// The floor's guardian takes the top of the stairs room.
    fn spawn_boss(&mut self, index: usize) {
        if self.boss_gates.is_some() && self.enemies.len() >= boss_ecology::ENTITY_CAP {
            return;
        }
        let boss = &self.bosses[index];
        // A guardian is tougher than the floor's rank and file.
        let base = boss.hp * self.toughness() / 100 * self.boss_health() / 100;
        let hp = base * self.party_len().max(1) as u32;
        let room = self.room();
        let (mut x, mut y) = self.boss_anchor();
        // Threshold scenery and chamber pillars must not bury an anchor boss.
        let col = (x / TILE_UNITS) as i32;
        let row = (y / TILE_UNITS) as i32;
        let grid = Grid {
            room,
            barred: self.barred(),
        };
        let open = |c: i32, r: i32| {
            if boss.id == signatures::ENGINE {
                room.tile(c, r) == Tile::Floor
                    && grid.clear(
                        (c as f32 + 0.5) * TILE_UNITS,
                        (r as f32 + 0.5) * TILE_UNITS,
                        boss.radius,
                        Mover::Walker,
                    )
            } else {
                (-1..=1).all(|dc| (-1..=1).all(|dr| room.tile(c + dc, r + dr) == Tile::Floor))
            }
        };
        let clear = if boss.id == signatures::ENGINE {
            room.tile(col, row) == Tile::Floor && grid.clear(x, y, boss.radius, Mover::Walker)
        } else {
            open(col, row)
        };
        if !clear
            && let Some((c, r)) = (3..room.rows as i32 - 1)
                .flat_map(|r| (1..room.cols as i32 - 1).map(move |c| (c, r)))
                .filter(|&(c, r)| open(c, r))
                .min_by_key(|&(c, r)| (c - col).abs() + (r - row).abs())
        {
            x = (c as f32 + 0.5) * TILE_UNITS;
            y = (r as f32 + 0.5) * TILE_UNITS;
        }
        self.next_id += 1;
        self.enemies.push(Enemy {
            hoard: Vec::new(),
            faction: None,
            camp_slot: None,
            x,
            y,
            hp,
            max_hp: hp,
            kind: EnemyKind::Boss,
            age: 0,
            id: self.next_id,
            origin_x: x,
            origin_y: y,
            boss: Some(index as u8),
            size: Some(boss.radius),
            base_hp: Some(base),
            raged: false,
            hurt: 0,
            seen_hp: hp,
            dread: 0,
            elite: false,
            stash: Default::default(),
            loot: Vec::new(),
            cry: 0,
            // A guardian's volleys keep their shape on every floor; only
            // the head count thickens them.
            fury: stakes::party_fury(self.party_len()),
            rung: 0,
            chilled: 0,
            pattern_volleys: 0,
            frozen: 0,
            sleep_until: 0,
            stage: 0,
            timer: 0,
            dir: (0.0, 0.0),
            guard: 0,
            wits: press::Wits::default(),
            signature: signatures::State::default(),
            home: None,
            dormant: false,
        });
    }

    /// How many knights the floors are laid out for: every seat with a
    /// knight in it, the host's empty seat not counted.
    pub fn party_size(&self) -> usize {
        self.players.values().filter(|h| !h.absent).count()
    }

    fn descend(&mut self) {
        // Floors are shared. Returned knights can walk or rejoin at their
        // entry; replacing their floor while they are outside loses it.
        if !self.exits.outside.is_empty() {
            self.found = Some((self.tick, 0, "Friends are outside: E at their entry rejoins the party before taking the next floor.".into()));
            return;
        }
        self.exits = exits::State::default();
        let from_home = self.dungeon.depth == 0;
        // Guard the transition itself, not just the stair tile/UI.
        if !from_home
            && self
                .boss_gates
                .as_ref()
                .is_some_and(|g| !g.unlocked() || !self.dungeon.rooms[g.guardian].cleared)
        {
            if self.guardian_locked() {
                self.show_boss_lock();
            }
            return;
        }
        if !from_home && self.dungeon.depth >= DEEPEST {
            return;
        }
        // The swamp ends in the Mire King's hall: no way down from it.
        if !from_home && self.dungeon.pack == Pack::Swamp && self.dungeon.depth >= swamp::FLOORS {
            return;
        }
        let (depth, pack) = if from_home {
            // Down the Winding Stair: to the landing chosen, in the delve
            // chosen for the top.
            let depth = self.home.landing().clamp(1, DEEPEST - 1);
            self.first = self.dungeon.pack;
            (depth, Pack::at(depth, self.first))
        } else {
            // The floor behind is closed: its results open the next one's
            // plate.
            self.close_floor(floor_stats::End::Cleared);
            self.settle_dare();
            self.unwashed_toll();
            self.lift_gifts();
            if self.dungeon.depth != FLOORS {
                // A dragon's floor was reclaimed and banked when it fell.
                self.reclaimed.push(self.dungeon.pack);
                self.bank_all(true, &format!("floor {} cleared", self.dungeon.depth));
            }
            // Through a hell gate the delve starts over, a circle deeper.
            let depth = if self.spectacle.circling {
                1
            } else {
                self.dungeon.depth + 1
            };
            let first = if self.dungeon.depth == 1 {
                self.dungeon.pack
            } else {
                self.first
            };
            self.first = first;
            (depth, Pack::at(depth, first))
        };
        if from_home {
            self.notice("first_delve");
        } else {
            // The school's boards: a floor behind them. Whoever never fell
            // stands a step nearer the school's highest honours.
            self.school_stairs();
            if self.mode == fortune::Mode::GlassJaw {
                self.notice("fragile");
            }
            if self.collapse_at.is_some_and(|at| self.tick >= at) {
                self.notice("out_of_time");
            }
        }
        if depth == FLOORS + 1 {
            self.notice("below_the_bottom");
        }
        self.settlement_site = None;
        self.settlement_exhibits.clear();
        // Down out of the upper floors into the deep: the dungeon stands up.
        if depth >= DEEP_FLOOR && self.dungeon.depth < DEEP_FLOOR {
            self.deep_beat = Some(self.tick);
        }
        self.dungeon = if self.wants_open() {
            layout::open_floor(depth, pack, self.party_size(), &mut self.rng)
        } else {
            layout::floor_for_party(self.mode, depth, pack, self.party_size(), &mut self.rng)
        };
        self.parked.clear();
        self.stock_gear_cache();
        self.at = 0;
        self.kingdom.return_to = None;
        // A delve the King holds has its forge-hall off its first hall.
        barony::add_forge_hall(&mut self.dungeon, &self.home.barony);
        // A floor the realm holds has its steward's hall, as built.
        holdings::add_holding_hall(&mut self.dungeon, &self.home.holdings);
        // Every floor below keeps a nook, a lived-in dead end; the wheel's
        // one-hall horde and straight gauntlet keep their bare shapes.
        if !matches!(self.mode, fortune::Mode::HoldTheStair | fortune::Mode::Gauntlet) {
            nooks::add_nook(&mut self.dungeon);
        }
        self.ready_nook();
        self.holding_used = 0;
        self.holding_raid = None;
        self.populate_boss_gates();
        self.cage_someone();
        self.pick_folly();
        self.pick_gantry();
        self.pick_race3d();
        if self.dungeon.open.is_some() {
            // The rooms' games wait for open floors (docs/OPEN-FLOORS.md).
            self.folly_room = None;
            self.gantry_room = None;
            self.race3d_room = None;
        }
        self.pots_broken.clear();
        self.pips_map();
        if from_home {
            self.kit_out();
            self.harden();
            self.deal_ults();
            self.bring_tallow();
            self.residents_help();
            self.runes_taken = 0;
            self.hexes = 0;
            self.drink_round();
            self.sing_song();
            self.hire();
            self.iron();
            self.cues.push("run_start".into());
            // A delve starts: three wishes each, fresh (`overclass`).
            for hero in self.players.values_mut() {
                (hero.wishes, hero.great_wishes) = (0, 0);
            }
            self.give_wishes(overclass::START_WISHES, 0);
        } else {
            self.refill_winds();
            // A new floor: two more wishes each.
            self.give_wishes(overclass::FLOOR_WISHES, 0);
        }
        self.collapse_at = (self.mode == fortune::Mode::Collapse)
            .then(|| self.tick + u64::from(fortune::Mode::collapse_secs(depth) * HZ));
        self.cues.push(format!("descend:{depth}"));
        self.sounds.push("descend");
        if self.players.values().any(|h| h.hp == 0) {
            self.cues.push("revive".into());
        }
        let carried: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp == 0)
            .map(|(&id, _)| id)
            .collect();
        for hero in self.players.values_mut() {
            // The stairs bring a fallen companion back to their feet.
            if hero.hp == 0 {
                hero.hp = (hero.max_hp * REVIVE_SHARE / 100).max(1);
            } else {
                hero.mend(10, stakes::Mend::Rest);
            }
        }
        // The school's boards: the stairs carried a fallen friend up; the
        // realm lets the first living knight carry the deed.
        if !carried.is_empty() && let Some(&id) = self.players.keys().next() {
            self.school.note(id, "healer-carry", carried.len() as u32);
        }
        self.half_charge();
        self.enter(0, None);
        self.call_dare();
        self.begin_floor_plate(floor_plates::Kind::of(pack));
        if from_home {
            self.floor_results = None;
            self.floor_tally.new_delve(self.tick);
            self.deeds.clear();
        } else {
            self.floor_tally.next_floor(self.tick);
            for plate in self.floor_plates.values_mut() {
                plate.results = floor_stats::SHOWN;
            }
        }
    }

    /// One fixed step of the run, and of the show watching it.
    /// Deterministic simulation: no clock, ambient randomness, network or
    /// model calls.
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>) {
        // Leaving follows transport time, even while an impact holds the scene.
        let mut transport_inputs = inputs.clone();
        self.tick_leave_hold(&mut transport_inputs);
        let Some(inputs) = self.hitstop.inputs(&self.tune, &self.players, &transport_inputs) else {
            return;
        };
        let inputs = &inputs;
        // A knight's wishes are held to the budget of the floor they stand on.
        self.keep_wish_floor();
        // Who is here may have changed since the last tick: the monsters
        // stand scaled for the knights who are.
        let before = self.scaled();
        self.rescale_party(before);
        self.count_idle(inputs);
        // Retreat is processed before reins, overlays, or combat. Discard the
        // rest of a departing seat's controls, including a held interact key.
        let mut inputs = inputs.clone();
        for (&id, input) in &mut inputs {
            if input.valid() && input.leave_activity && self.leave_activity(id) {
                *input = Input::default();
            }
        }
        self.close_behind_the_away();
        let detached = self.detached_seats(&inputs);
        if self.players.is_empty() {
            self.players.extend(detached);
            return;
        }
        self.tick_floor_plates(&inputs);
        let inputs = self.ballot_inputs(&inputs);
        let inputs = &inputs;
        let (live, heard, sounded) = (self.active(), self.cues.len(), self.sounds.len());
        let seen = live.then(|| self.seen());
        let was_over = self.over();
        self.advance(inputs);
        if self.turbo() && self.hitstop.left == 0 {
            self.advance(inputs);
        }
        self.flush_school();
        self.flush_questlines();
        self.tick_teachers(&inputs);
        self.follow_kingdom_cameras();
        if let Some(seen) = seen {
            self.watch(seen);
        }
        if live {
            self.watch_dare(heard, sounded);
            self.mark_moments(heard);
            self.mark_trophies(heard);
        }
        if self.phase == Phase::Wiped && detached.values().any(|h| h.hp > 0) {
            self.phase = if self.barred() && !self.open_map() {
                Phase::Fighting
            } else {
                Phase::Exploring
            };
        }
        self.players.extend(detached);
        if !was_over && self.over() {
            self.close_run();
        }
        self.stand_verdict();
        if !self.active() {
            self.floor_plates.clear();
        }
        // What the step's watchers showed after the tick (`fx`).
        self.show_fx();
    }

    /// Each knight's idle ticks: back to none on any key, one more on a
    /// tick with no key or no seat at all.
    fn count_idle(&mut self, inputs: &BTreeMap<u32, Input>) {
        let ids: Vec<u32> = self.players.keys().copied().collect();
        self.idle.retain(|id, _| ids.contains(id));
        for id in ids {
            let pressing = inputs.get(&id).is_some_and(|i| i.valid() && *i != Input::default());
            let idle = self.idle.entry(id).or_insert(0);
            *idle = if pressing { 0 } else { idle.saturating_add(1) };
        }
    }

    /// Whether knight `id` is away: its player is not here (`set_away`, from
    /// what the host's server knows of the seat: a page loading or hidden, a
    /// link gone quiet, an empty seat), or is here but has pressed nothing
    /// for `AWAY_TICKS`. Away knights go where the party goes and hold up
    /// nothing: no door waits for them, no stair or doorway is theirs to
    /// take, and a place closes when only they are left in it.
    pub fn away(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| h.away) || self.idle.get(&id).is_some_and(|&t| t >= AWAY_TICKS)
    }

    /// Whether F belongs to a festival round playing in this room (the
    /// courier, the turnip) rather than to the party's arms.
    fn arcade(&self) -> bool {
        self.at_home_now()
            && self
                .pastimes
                .round
                .as_ref()
                .is_some_and(|r| r.game.room() == self.room().kind)
    }

    fn advance(&mut self, inputs: &BTreeMap<u32, Input>) {
        if !self.active() {
            return;
        }
        self.play(inputs);
        // What the tick showed: its shakes and sparks (`fx`).
        self.present();
        self.finish_hitstop();
    }

    /// One tick of the rules.
    fn play(&mut self, inputs: &BTreeMap<u32, Input>) {
        // A ride under way (the lists, the Hippodrome, a race hall) plays
        // and takes the keys of the knights it seats (`activity::RIDES`).
        let reined = self.ride(inputs);
        let inputs = reined.as_ref().unwrap_or(inputs);
        // The cauldron answers its own keys first (`cauldron`).
        let stirred = self.tend_cauldron(inputs);
        let inputs = stirred.as_ref().unwrap_or(inputs);
        self.fade();
        let boss_beats: Vec<_> = self.enemies.iter()
            .filter(|e| e.is_boss() && e.hp > 0)
            .map(|e| (e.id, e.raged, e.frozen, self.boss_phase(e)))
            .collect();
        self.tick += 1;
        // Who waits in a way out is said afresh each tick (`travel`).
        self.fellowship.clear();
        // A room holding for its party wards every knight through the tick.
        self.ward_while_held();
        self.set_stakes();
        self.tick_thieves();
        let before: Vec<(u32, u32)> = self.players.values().map(|h| (h.hp, h.max_hp)).collect();
        let mut shots = Vec::with_capacity(32);
        let mut bombs = 0;
        let mut nova = 0;
        let mut blast_owner = 0;
        // Where knights' bombs went off and draughts were drunk this tick,
        // for the small secrets that answer them.
        let mut booms: Vec<(f32, f32)> = Vec::new();
        let mut draughts: Vec<(f32, f32)> = Vec::new();
        // Damage each knight dealt this tick, for their ultimate's charge,
        // and the ultimates cast.
        let mut credits: Vec<(u32, u32)> = Vec::new();
        let mut casts: Vec<u32> = Vec::new();
        // Cards' creatures called this tick: the knight and the card.
        let mut summons: Vec<(u32, String)> = Vec::new();
        let side = self.side_on();
        // F belongs to the courier/turnip while a festival round is playing;
        // do not also spray the party's ordinary weapon across its markers.
        // The Oathbound's oath: what one twin takes, both feel — but the
        // oath keeps its last cruelty, flooring the survivor at a single
        // wound when the other is felled by a blow meant for one. A stroke
        // that reaches both breaks no oath; it ends it.
        self.tick_oathbound();
        let arcade = self.arcade();
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let depth = self.dungeon.depth;
        // Where each knight stood before this tick's step, for how fast it
        // runs: a pattern's lead aims where a knight is going.
        let stood: BTreeMap<u32, (f32, f32)> = self
            .players
            .iter()
            .map(|(&id, h)| (id, (h.x, h.y)))
            .collect();
        let fighting = self.fighting();
        // The kingdom's roads, for the trot along them.
        let roads = (self.room().kind == RoomKind::Kingdom)
            .then(|| map::geography(&self.kingdom.wishes));
        let admitted = self.pit_admissions();
        let mut pit_events: Vec<(u32, pitstop::Event)> = Vec::new();
        // The attack-rate knob scales each cooldown a knight sets this tick.
        let cooled: Vec<(u32, u32)> = if self.tune.fire_rate == 100 {
            Vec::new()
        } else {
            self.players.iter().map(|(&id, h)| (id, h.fire_cooldown)).collect()
        };
        self.scarecrows.retain(|d| d.collapse_until > self.tick);
        for (id, hero) in &mut self.players {
            let input = inputs
                .get(id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            // The vigil answers its key's press, from stone too, and holds
            // with no key at all: its player has stepped away. A knight
            // wakes from it any time, but never kneels to it mid-fight.
            Run::wake_on_first_move(hero, &input);
            hero.vigil_rearm = hero.vigil_rearm.saturating_sub(1);
            if input.vigil
                && !hero.vigil_key
                && hero.vigil_rearm == 0
                && hero.hp > 0
                && (hero.vigil || !fighting)
            {
                hero.vigil = !hero.vigil;
                hero.stone = hero.vigil;
                hero.vigil_rearm = if hero.vigil {
                    VIGIL_SETTLE
                } else {
                    VIGIL_REARM
                };
                (hero.shielding, hero.walling, hero.dash_ticks) = (false, false, 0);
                self.sounds.push("wall_up");
            }
            hero.vigil_key = input.vigil;
            if hero.hp == 0 || hero.stone {
                continue;
            }
            // Hexed: a frog hops about, and does nothing else (aiming is
            // shooting, so not even that). Thrown by a Ravage: not even that.
            let input = if hero.tossed > 0 {
                Input::default()
            } else if hero.frog() {
                Input {
                    move_x: input.move_x,
                    move_y: input.move_y,
                    ..Input::default()
                }
            } else {
                input
            };
            if hero.privy > 0 || hero.pit.stop.is_some() {
                // At the privy: the pit stop's keys, and nothing else.
                let mut mine = Vec::new();
                pitstop::step(hero, input, admitted.contains(id), &mut mine);
                pit_events.extend(mine.into_iter().map(|e| (*id, e)));
                continue;
            }
            // The result mime owns this seat until its window has eased shut.
            if hero.pit.result.is_some() {
                continue;
            }
            let taking = input.interact || input.take;
            hero.taking = taking && !hero.take_key;
            hero.take_key = taking;
            let pressed = input.ult && !hero.ult_key;
            hero.ult_key = input.ult;
            if pressed
                && hero.ult_charge >= ults::ULT_FULL
                && hero.slashes == 0
                && hero.aiming.is_none()
                && hero.flee_ticks == 0
            {
                if hero.ult() == ults::Ult::Desertion {
                    peasant::start(hero, *id, self.tick, &mut self.scarecrows);
                    // The Peasant's boards: every Desertion run.
                    self.school.note(*id, "peasant-desert", 1);
                }
                casts.push(*id);
            }
            if hero.slashes > 0 {
                // Mid-Bladewind: the blade does the moving.
                continue;
            }
            hero.invulnerable = hero.invulnerable.saturating_sub(1);
            hero.bomb_cooldown = hero.bomb_cooldown.saturating_sub(1);
            hero.fire_cooldown = hero.fire_cooldown.saturating_sub(1);
            hero.swing = hero.swing.saturating_sub(1);
            // The knight's own going, as its client draws it ahead too
            // (`Run::predict`): the walk, the aim and the guard key.
            let shield_free = !self.works.flights.iter().any(|f| f.owner == *id);
            let ((mx, my), guarded) = hero.own_keys(&input, side, shield_free);
            let (ax, ay) = unit(input.aim_x as f32, input.aim_y as f32);
            if guarded.rolled {
                self.sounds.push("roll");
            }
            if guarded.spun {
                self.sounds.push("swing");
            }
            if guarded.wall_raised {
                self.sounds.push("wall_up");
            }
            if guarded.mana_out {
                self.sounds.push("mana_empty");
            }
            // The axes come round: cut on the first tick and again at the half.
            if let Some(age) = guarded.spin_age
                && spin::sweeping(age)
            {
                let heavy = hero.whirl > 0 && self.enemies.iter().any(|e| {
                    e.hp > 0 && e.is_boss()
                        && (e.x - hero.x).hypot(e.y - hero.y) <= spin::REACH + e.radius()
                });
                let dealt = spin::sweep(
                    *id,
                    hero,
                    &mut self.enemies,
                    &mut self.projectiles,
                    &mut self.sounds,
                );
                if heavy && dealt > 0 {
                    self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                }
                if dealt > 0 {
                    credits.push((*id, dealt));
                    // The Berserker's boards: a spin that cut.
                    self.school.note(*id, "berserk-spin", 1);
                }
            }
            let rolled_before = hero.dash_ticks;
            // Sir Dinadan's Lay of Haste among the rest, to a knight who hears it.
            let ((dx, dy, speed), blinked) = hero.own_step((mx, my), &grid, self.tune.roll_speed, |h| {
                fx::ground_pace(h, &self.webs, self.song.as_deref(), roads.as_deref(), &self.tune)
            });
            if blinked {
                self.sounds.push("roll");
            }
            if side {
                let rolling = hero.dash_ticks > 0;
                if ledge::walk(hero, &input, &grid, dx * speed * DT, rolling) {
                    self.sounds.push("hero_hurt");
                }
                // Down on firm ground is a crouch: shots at the chest pass over.
                let duck = input.move_y > 0 && hero.grounded && hero.dropping == 0 && !rolling;
                self.works.kits.entry(*id).or_default().duck = duck;
            } else if let Some(to) = self
                .folly
                .as_mut()
                .filter(|m| m.room == self.at)
                .and_then(|m| {
                    // A roll is a burst along the lanes, steered by the keys.
                    let way = if (mx, my) != (0.0, 0.0) { (mx, my) } else { (dx, dy) };
                    m.glide(*id, grid.room, (hero.x, hero.y), way, speed * DT)
                })
            {
                // In the Wax Maze a knight glides its lanes, and faces the
                // way they go unless they aim.
                let moved = (to.0 - hero.x, to.1 - hero.y);
                (hero.x, hero.y) = to;
                if ax == 0.0 && ay == 0.0 && moved.0.abs().max(moved.1.abs()) > 1e-4 {
                    (hero.aim_x, hero.aim_y) = unit(moved.0, moved.1);
                }
            } else {
                (hero.x, hero.y) = grid.slide(
                    (hero.x, hero.y),
                    (dx * speed * DT, dy * speed * DT),
                    HERO_RADIUS,
                    Mover::Hero,
                );
            }
            // A roll's dust at the knight's feet: kicked up as it starts,
            // left along it, thrown out where it lands (`feel`).
            let feet = (hero.x, hero.y + if side { ledge::FOOT } else { 0.0 });
            let way = hero.roll_dir();
            if guarded.rolled {
                self.feel.dust(feet.0, feet.1, way, feel::KICK);
            } else if rolled_before == 1 && hero.dash_ticks == 0 {
                self.feel.dust(feet.0, feet.1, way, feel::LAND);
            } else if hero.dash_ticks > 0 && hero.dash_ticks % 3 == 0 {
                self.feel.dust(feet.0, feet.1, way, feel::TRAIL);
            }
            let input = if hero.flee_ticks > 0 { Input::default() } else { input };
            for timer in &mut hero.spell_cooldowns {
                *timer = timer.saturating_sub(1);
            }
            hero.body_rearm = hero.body_rearm.saturating_sub(1);
            if hero.possessed.is_some() {
                // A borrowed body casts its own spell on any spell key, and
                // the knight's spell cards wait (`bodies`).
                if (1..=cards::SPELL_SLOTS).contains(&usize::from(input.cast)) {
                    let salt = mix(self.seed ^ self.tick.wrapping_mul(0x9e37) ^ u64::from(*id) << 40);
                    if let Some(name) = bodies::cast(hero, *id, &grid, &self.enemies, salt, &mut shots) {
                        self.found = Some((self.tick, *id, format!("cast {name}")));
                        self.sounds.push("spell_cast");
                    }
                }
            } else if let Some(slot) = usize::from(input.cast)
                .checked_sub(1)
                .filter(|s| *s < cards::SPELL_SLOTS)
                && hero.spell_cooldowns[slot] == 0
                && let Some(card) = hero.spells[slot]
                    .as_deref()
                    .and_then(|id| self.book.get(id))
                    .filter(|c| c.kind == cards::Kind::Spell)
            {
                hero.spell_cooldowns[slot] = card
                    .cooldown
                    .clamp(cards::SPELL_COOLDOWN.0, cards::SPELL_COOLDOWN.1)
                    * HZ;
                let before = nova;
                hero.spend(card, &mut self.score, &mut nova);
                if nova > before {
                    blast_owner = *id;
                }
                let bounds = (grid.room.width(), grid.room.height());
                for effect in &card.effects {
                    match effect {
                        cards::Effect::Cast(shape) => {
                            let look =
                                Projectile::look_of(card).or_else(|| hero.shot_look(&self.book));
                            let start = shots.len();
                            hero.spell_shots(*shape, *id, &mut shots);
                            if look.is_some() {
                                for shot in &mut shots[start..] {
                                    shot.look = look.clone();
                                }
                            }
                        }
                        // The second grimoire: the garden and the counterfeit.
                        cards::Effect::Cabbages(count) => {
                            spells::cabbage_storm(&mut self.strikes, hero, *count, bounds, *id)
                        }
                        cards::Effect::Imposture(_) => {
                            spells::call_imposture(&mut self.phantoms, hero, *id)
                        }
                        cards::Effect::Hedge(_) => {
                            spells::hedge_maze(&mut self.works.barricades, hero, *id, bounds)
                        }
                        cards::Effect::Summon(..) => summons.push((*id, card.id.clone())),
                        _ => {}
                    }
                }
                self.found = Some((self.tick, *id, format!("cast {}", card.name)));
                self.sounds.push("spell_cast");
                self.school.note(*id, "mage-cast", 1);
            }
            hero.play_cooldown = hero.play_cooldown.saturating_sub(1);
            let slot = usize::from(input.play);
            if slot >= 1 && slot <= hero.hand.len() && hero.play_cooldown == 0 {
                hero.play_cooldown = PLAY_REARM;
                let summon = self.book.get(&hero.hand[slot - 1]).and_then(Card::summon);
                // A minion takes the ally slot from another minion, never
                // from Beaumains: the card stays in hand.
                let beaumains = self
                    .hireling
                    .as_ref()
                    .is_some_and(|h| h.kind == hireling::AllyKind::Beaumains);
                if summon.is_some() && beaumains {
                    self.found = Some((
                        self.tick,
                        *id,
                        "Beaumains will not share the road with it".into(),
                    ));
                } else {
                    let id_card = hero.hand.remove(slot - 1);
                    if let Some(card) = self.book.get(&id_card) {
                        let before = nova;
                        hero.spend(card, &mut self.score, &mut nova);
                        if nova > before {
                            blast_owner = *id;
                        }
                        if card.imps().is_some() {
                            summons.push((*id, card.id.clone()));
                        }
                        if let Some((kind, power)) = summon {
                            let at = hireling::beside(&grid, (hero.x, hero.y));
                            self.hireling =
                                Some(hireling::Hireling::minion(kind, power, depth, at, *id));
                            self.cues.push(format!("summoned:{}", kind.word()));
                        }
                        if card
                            .effects
                            .iter()
                            .any(|e| matches!(e, cards::Effect::Heal(_)))
                        {
                            draughts.push((hero.x, hero.y));
                        }
                        self.found = Some((self.tick, *id, format!("played {}", card.name)));
                        self.sounds.push("play_card");
                        // The Herald calls the items by name.
                        for effect in &card.effects {
                            match effect {
                                cards::Effect::Blink(_) => self.cues.push("blink".into()),
                                cards::Effect::Immune(_) => self.cues.push("sceptre".into()),
                                _ => {}
                            }
                        }
                    }
                }
            }
            if input.bomb && hero.bombs > 0 && hero.bomb_cooldown == 0 {
                hero.bombs -= 1;
                hero.bomb_cooldown = BOMB_REARM;
                hero.invulnerable = hero.invulnerable.max(HZ / 2);
                bombs += 1;
                blast_owner = *id;
                booms.push((hero.x, hero.y));
                self.fx.push(Fx::Shake(8));
                self.sounds.push("bomb");
            }
            hero.sword_cooldown = hero.sword_cooldown.saturating_sub(1);
            if input.swing
                && !hero.unarmed()
                && hero.possessed.is_none()
                && hero.sword_cooldown == 0
                && hero.dash_ticks == 0
            {
                hero.sword_cooldown = hero.cooldown(SWORD_REARM);
                hero.swing = 8;
                hero.sword = true;
                self.sounds.push("swing");
                let angle = hero.aim_y.atan2(hero.aim_x);
                let half = SWORD_ARC.to_radians() / 2.0;
                let within = |dx: f32, dy: f32, reach: f32| {
                    let delta = (dy.atan2(dx) - angle)
                        .sin()
                        .atan2((dy.atan2(dx) - angle).cos())
                        .abs();
                    dx.hypot(dy) <= reach && delta <= half
                };
                let damage = hero.scaled(SWORD_DAMAGE);
                let (hx, hy) = (hero.x, hero.y);
                for (i, dealt) in aoe::strike(&mut self.enemies, (hx, hy), damage, |e| {
                    within(e.x - hx, e.y - hy, SWORD_REACH + e.radius())
                }) {
                    let bonus =
                        signatures::rear_bonus(&self.enemies[i], &self.bosses, (hx, hy), dealt)
                            .min(self.enemies[i].hp);
                    self.enemies[i].hp -= bonus;
                    self.enemies[i].wits.foe = *id;
                    credits.push((*id, dealt + bonus));
                    self.sounds.push("hit");
                }
                // The blade parries: hostile shots in its arc are cut down.
                for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
                    if within(shot.x - hero.x, shot.y - hero.y, SWORD_REACH + 0.6) {
                        shot.ttl = 0;
                    }
                }
            }
            let guarded = hero.flee_ticks > 0 || hero.dash_ticks > 0 || hero.shielding || hero.walling || hero.spin > 0;
            if !arcade
                && (input.fire || ax != 0.0 || ay != 0.0)
                && hero.fire_cooldown == 0
                && !guarded
            {
                if hero.possessed.is_some() {
                    // A borrowed body looses what its monster looses
                    // (`bodies`), never the knight's own arm.
                    let salt = mix(self.seed ^ self.tick.wrapping_mul(0x9e37) ^ u64::from(*id) << 48);
                    let pack = self.dungeon.pack;
                    match bodies::loose(hero, *id, pack, &grid, &mut self.enemies, salt, &mut shots) {
                        Some(bodies::Loosed::Bite(struck)) => {
                            self.sounds.push("swing");
                            for (i, dealt) in struck {
                                let enemy = &self.enemies[i];
                                credits.push((*id, dealt));
                                self.feel.impact(enemy.x, enemy.y, Shot::Sacred, (enemy.x - hero.x, enemy.y - hero.y));
                                self.sounds.push("hit");
                            }
                        }
                        Some(bodies::Loosed::Volley(n)) if n > 0 => self.sounds.push("shot_spark"),
                        _ => {}
                    }
                } else if hero.unarmed() {
                    hero.fire_cooldown = 9;
                    hero.swing = 5;
                    hero.sword = false;
                    self.sounds.push("swing");
                    let (hx, hy) = (hero.x, hero.y);
                    let angle = hero.aim_y.atan2(hero.aim_x);
                    for (i, dealt) in aoe::strike(&mut self.enemies, (hx, hy), hero.scaled(6), |e| {
                        let (dx, dy) = (e.x - hx, e.y - hy);
                        let delta = (dy.atan2(dx) - angle).sin().atan2((dy.atan2(dx) - angle).cos()).abs();
                        dx.hypot(dy) <= 1.25 + e.radius() && delta <= 0.7
                            && grid.open((hx + e.x) / 2.0, (hy + e.y) / 2.0, Mover::Hero)
                    }) {
                        let enemy = &mut self.enemies[i];
                        enemy.wits.foe = *id;
                        credits.push((*id, dealt));
                        self.feel.impact(enemy.x, enemy.y, Shot::Sacred, (enemy.x - hx, enemy.y - hy));
                        self.sounds.push("hit");
                        if !enemy.is_boss() && !matches!(enemy.kind, EnemyKind::Dummy | EnemyKind::Ward) {
                            if enemy.wits.flinch == 0 {
                                enemy.frozen = enemy.frozen.max(feel::JOLT_HIT);
                                enemy.wits.flinch = feel::FLINCH;
                            }
                            let (ux, uy) = unit(enemy.x - hx, enemy.y - hy);
                            let mover = if enemy.kind.flies() { Mover::Flier } else { Mover::Walker };
                            (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), (ux * 0.22, if side { 0.0 } else { uy * 0.22 }), enemy.radius(), mover);
                        }
                    }
                } else if let Some(weapon) = &hero.forged {
                    let target = self.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                        ((a.x - hero.x).hypot(a.y - hero.y))
                            .total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
                    });
                    let melee = weapon.melee.filter(|m| {
                        (weapon.bolt.is_none() && weapon.throw.is_none())
                            || target.is_some_and(|e| {
                                (e.x - hero.x).hypot(e.y - hero.y) <= m.reach + e.radius()
                            })
                    });
                    if let Some(melee) = melee {
                        if let Some(enemy) = target.filter(|e| {
                            (e.x - hero.x).hypot(e.y - hero.y) <= melee.reach + e.radius()
                        }) {
                            (hero.aim_x, hero.aim_y) = unit(enemy.x - hero.x, enemy.y - hero.y);
                        }
                        hero.fire_cooldown = hero.cooldown(melee.every);
                        let damage = hero.scaled(melee.damage);
                        hero.swing = 6;
                        hero.sword = false;
                        self.sounds.push("swing");
                        let angle = hero.aim_y.atan2(hero.aim_x);
                        let (hx, hy) = (hero.x, hero.y);
                        for (i, dealt) in aoe::strike(&mut self.enemies, (hx, hy), damage, |e| {
                            let (dx, dy) = (e.x - hx, e.y - hy);
                            let delta = (dy.atan2(dx) - angle)
                                .sin()
                                .atan2((dy.atan2(dx) - angle).cos())
                                .abs();
                            dx.hypot(dy) <= melee.reach + e.radius()
                                && delta <= melee.arc.to_radians() / 2.0
                                && grid.open((hx + e.x) / 2.0, (hy + e.y) / 2.0, Mover::Hero)
                        }) {
                            let bonus = signatures::rear_bonus(
                                &self.enemies[i],
                                &self.bosses,
                                (hx, hy),
                                dealt,
                            )
                            .min(self.enemies[i].hp);
                            self.enemies[i].hp -= bonus;
                            self.enemies[i].wits.foe = *id;
                            credits.push((*id, dealt + bonus));
                        }
                        // The blade parries: shots in its sweep are cut down.
                        for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
                            let (dx, dy) = (shot.x - hero.x, shot.y - hero.y);
                            let delta = (dy.atan2(dx) - angle)
                                .sin()
                                .atan2((dy.atan2(dx) - angle).cos())
                                .abs();
                            if dx.hypot(dy) <= melee.reach + 0.6
                                && delta <= melee.arc.to_radians() / 2.0
                            {
                                shot.ttl = 0;
                            }
                        }
                    } else {
                        // A forged weapon's throw or bolts (`Hero::loose`).
                        let rng = mix(self.seed ^ self.tick.wrapping_mul(0x9e37) ^ u64::from(*id) << 48);
                        let flying = in_flight(&self.projectiles, *id);
                        match hero.loose(*id, &self.book, false, rng, flying, &mut shots) {
                            Some(Loosed::Throw) => self.sounds.push("swing"),
                            Some(Loosed::Bolts(count)) => {
                                self.sounds.push("shot_bolt");
                                self.floor_tally.line(*id).shots += count;
                                if feel::ledger_on() {
                                    self.feel.ledger.entry(*id).or_default().shots += count;
                                }
                            }
                            Some(Loosed::Arm(..)) | None => {}
                        }
                    }
                } else {
                    // A Mage's bolt and a Healer's spark sound their own.
                    self.sounds.push(classes::shot_sound(hero).unwrap_or(match hero.weapon {
                        Weapon::Fists => unreachable!("fists use the punch branch"),
                        Weapon::Bow => "shot_bow",
                        Weapon::Notes | Weapon::Stones => "shot_spark",
                        Weapon::Crossbow => "shot_crossbow",
                        Weapon::Handgonne => "shot_handgonne",
                    }));
                    let burning = hero.weapon == Weapon::Bow
                        && self.works.kits.get(id).is_some_and(|k| k.burning > 0);
                    let rng = mix(self.seed ^ self.tick.wrapping_mul(0x9e37) ^ u64::from(*id) << 48);
                    let flying = in_flight(&self.projectiles, *id);
                    let Some(Loosed::Arm(count, (ax, ay))) = hero.loose(*id, &self.book, burning, rng, flying, &mut shots) else {
                        unreachable!("a plain arm looses its own shots")
                    };
                    self.floor_tally.line(*id).shots += count;
                    if feel::ledger_on() {
                        self.feel.ledger.entry(*id).or_default().shots += count;
                    }
                    let muzzle = (hero.x + ax * 0.9, hero.y + ay * 0.9);
                    match hero.weapon {
                        Weapon::Fists => unreachable!("fists use the punch branch"),
                        Weapon::Bow | Weapon::Notes | Weapon::Stones => {}
                        Weapon::Crossbow => self.feel.puff(muzzle.0, muzzle.1, (ax, ay), false),
                        Weapon::Handgonne => {
                            self.feel.puff(muzzle.0, muzzle.1, (ax, ay), false);
                            self.feel.puff(muzzle.0, muzzle.1, (ax, ay), true);
                            self.fx.push(Fx::Shake(feel::SHAKE_SHOT));
                            // The kick: back along the aim, never through stone.
                            let up = if side { 0.0 } else { -ay * feel::RECOIL };
                            let kick = (-ax * feel::RECOIL, up);
                            (hero.x, hero.y) =
                                grid.slide((hero.x, hero.y), kick, HERO_RADIUS, Mover::Hero);
                        }
                    }
                }
            }
        }
        tune::tune_knights(&mut self.players, &self.tune, &cooled, &mut shots);
        wishcraft::shape(&mut shots);
        if bombs > 0 {
            // A bomb clears the monsters' shots around the knight who threw
            // it, not the whole room: each pops where it hung.
            let feel = &mut self.feel;
            self.projectiles.retain(|p| {
                let kept = !p.hostile
                    || booms
                        .iter()
                        .all(|&(x, y)| (p.x - x).hypot(p.y - y) > BOMB_CLEAR);
                if !kept {
                    feel.impact(p.x, p.y, p.kind, (0.0, 0.0));
                }
                kept
            });
        }
        // A bomb or a Thunder Scroll is spent when used and hits for a fixed
        // sum, whatever the knight holds: it still reaches the whole room,
        // outside the area rule (`aoe`).
        let blast = BOMB_DAMAGE * bombs + nova;
        // On an open map a blast fills what is round it, not the floor.
        let open_blast: Option<Vec<(f32, f32)>> = self.open_map().then(|| {
            booms
                .iter()
                .copied()
                .chain(self.players.get(&blast_owner).filter(|_| nova > 0).map(|h| (h.x, h.y)))
                .collect()
        });
        if blast > 0 {
            for enemy in &mut self.enemies {
                if open_blast
                    .as_ref()
                    .is_some_and(|at| !at.iter().any(|&(x, y)| (enemy.x - x).hypot(enemy.y - y) <= depths::OPEN_BLAST))
                {
                    continue;
                }
                let actual = blast.min(enemy.hp);
                enemy.hp = enemy.hp.saturating_sub(blast);
                if actual > 0 && enemy.is_boss() {
                    self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                }
                if blast_owner > 0 && actual > 0 {
                    enemy.wits.foe = blast_owner;
                    self.feel.credit(blast_owner, enemy.id, actual, self.tick);
                    // The Gunner's boards: every foe a bomb felled.
                    self.school.note(blast_owner, "gunner-bombs", 1);
                }
            }
        }
        ults::phantom_shots(&self.phantoms, &self.players, &mut shots);
        let mut deeds = foes::Deeds::default();
        // Monsters see knights (not one under an Invisibility rune), and the
        // images Phantasm made of them. Each hunts its own knight (`press`).
        let knights: press::Knights = self
            .players
            .iter()
            .filter(|(_, hero)| {
                hero.targetable() && !hero.has_rune(runes::RuneKind::Invisibility)
            })
            .map(|(&id, hero)| (id, hero.x, hero.y))
            .collect();
        let decoys: Vec<(f32, f32)> = self
            .phantoms
            .iter()
            .map(|p| (p.x, p.y))
            .chain(
                self.hireling
                    .iter()
                    .filter(|h| !h.down())
                    .map(|h| (h.x, h.y)),
            )
            .collect();
        // How fast each of those runs, units a second (images and the
        // hireling count as standing still).
        let knight_paces: Vec<(f32, f32)> = self
            .players
            .iter()
            .filter(|(_, hero)| {
                hero.targetable() && !hero.has_rune(runes::RuneKind::Invisibility)
            })
            .map(|(id, hero)| {
                let (x, y) = stood.get(id).copied().unwrap_or((hero.x, hero.y));
                let most = self.tune.hero_speed * 1.5;
                (
                    ((hero.x - x) / DT).clamp(-most, most),
                    ((hero.y - y) / DT).clamp(-most, most),
                )
            })
            .collect();
        let mut living: Vec<(f32, f32)> = Vec::with_capacity(knights.len() + decoys.len());
        let heads = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone)
            .count();
        let mut paces = Vec::with_capacity(knights.len() + decoys.len());
        let holding = self.holding();
        // An open map's leashes: each chamber's stone and a little round it.
        let leashes = self.leashes();
        // What the room's hostile shots weigh against a guardian's caps.
        let standing: u32 = self
            .projectiles
            .iter()
            .filter(|p| p.hostile)
            .map(signatures::weight)
            .sum();
        let wanted = self.dungeon.rooms[self.at].kind == RoomKind::Wanted;
        // The charge-ups still about: a guardian winds up no other until
        // everything of its last is gone (`signatures::due`).
        let lingering: Vec<u32> = self
            .projectiles
            .iter()
            .map(|p| p.traits.blow)
            .chain(self.ravages.iter().map(|r| r.blow))
            .filter(|&blow| blow != 0)
            .collect();
        for enemy in &mut self.enemies {
            enemy.signature.lingering =
                enemy.signature.blow != 0 && lingering.contains(&enemy.signature.blow);
            if enemy.hp == 0 {
                continue;
            }
            // The room holds for its party (`presence`): every foe waits,
            // its telegraph and its stun with it.
            if holding {
                continue;
            }
            // Waiting in its chamber for a knight to come (`depths`).
            if enemy.dormant {
                enemy.age = enemy.age.saturating_add(1);
                continue;
            }
            if enemy.asleep(self.tick) || press::held(enemy) {
                // Stunned, or held in a Stillhour: nothing moves.
                continue;
            }
            peasant::sights(self.tick, &self.scarecrows, enemy, &knights, &decoys, &mut living);
            // Lead patterns use the same targets as ordinary attacks, with
            // each selected knight's pace in the matching slot. Decoys stand still.
            paces.clear();
            paces.extend(living.iter().map(|&(x, y)| {
                knights
                    .iter()
                    .position(|&(_, kx, ky)| kx == x && ky == y)
                    .and_then(|i| knight_paces.get(i).copied())
                    .unwrap_or((0.0, 0.0))
            }));
            let sight = patterns::Sight {
                knights: &living,
                paces: &paces,
                pack: self.dungeon.pack,
                depth,
                heads,
            };
            enemy.age += 1;
            if enemy.age < TELEGRAPH {
                continue;
            }
            let rest = enemy.y;
            let fired = shots.len();
            let from = (enemy.x, enemy.y);
            monsters::act(
                enemy,
                &living,
                &grid,
                self.seed,
                &self.bosses,
                &mut shots,
                &mut deeds,
            );
            if self.tune.foe_pace != 1.0 {
                tune::foe_step(enemy, from, self.tune.foe_pace, &grid);
            }
            // A guardian in its own room fans nothing out: a party meets
            // more of it in the room at once (`signatures::hold`), never
            // thicker volleys.
            let guardian = enemy.boss.is_some() && !wanted;
            if enemy.fury > 0 && shots.len() > fired && !guardian {
                stakes::fan_out(enemy.fury, &mut shots, fired);
            }
            // Its pattern, thickened by its own numbers, never fanned.
            patterns::volley(enemy, &sight, &grid, self.seed, &self.bosses, &mut shots);
            // A library guardian's shots wear its own bank; a dart, an orb
            // or a lob keeps the look of what it does (`patterns::Form`),
            // its landing ring and its girth in sight.
            if let Some(art) = enemy.boss
                .and_then(|i| self.bosses.get(usize::from(i)))
                .and_then(|b| bosses::Art::for_id(&b.id))
            {
                for shot in shots[fired..].iter_mut().filter(|s| s.traits.path.form == patterns::Form::Plain) {
                    shot.look = art.look();
                }
            }
            // A guardian in its own room is held to its stage's caps.
            if let Some(boss) = enemy.boss.and_then(|b| self.bosses.get(usize::from(b)))
                && !wanted
            {
                let before: u32 = shots[..fired].iter().map(signatures::weight).sum();
                signatures::hold(
                    enemy,
                    boss,
                    &mut shots,
                    fired,
                    &mut deeds.bursts,
                    standing + before,
                    heads,
                );
            }
            shots.append(&mut deeds.bursts);
            critters::quicken(enemy, from, &grid);
            wishcraft::slow(enemy, from, &grid);
            // A barricade stops whatever walks into it.
            skills::shove(enemy, &self.works.barricades);
            if side && !enemy.kind.flies() && !outlaw::own_gravity(enemy, &grid) {
                // Side-on, walkers walk; the ground decides their height.
                enemy.y = rest;
                ledge::fall(enemy, &grid);
            }
            // Never farther from home than its leash.
            if let Some(&(x0, y0, x1, y1)) = enemy.home.and_then(|h| leashes.get(usize::from(h))) {
                let out = |(x, y): (f32, f32)| (x0 - x).max(0.0) + (x - x1).max(0.0) + (y0 - y).max(0.0) + (y - y1).max(0.0);
                if out((enemy.x, enemy.y)) > out(from) {
                    (enemy.x, enemy.y) = from;
                }
            }
        }
        let loosed = self.projectiles.len();
        self.projectiles.extend(
            shots
                .into_iter()
                .take(MAX_PROJECTILES.saturating_sub(self.projectiles.len())),
        );
        self.hostile_shots += hostile_past(&self.projectiles, loosed);
        // Swept segments prevent fast shots tunnelling through small targets.
        let walls: Vec<((f32, f32), (f32, f32), u32)> = self
            .players
            .values()
            .filter(|h| h.walling && h.hp > 0 && !h.stone)
            .filter_map(|h| h.wall_span().map(|(a, b)| (a, b, h.wall_empower())))
            .collect();
        let mut chains: Vec<(u32, f32, f32, u32, u8, u8)> = Vec::new();
        let mut bursts: Vec<(u32, f32, f32, u32, u8)> = Vec::new();
        // Where Fart Bombs hit this tick: the spot, the knight, how strong.
        let mut farts: Vec<(f32, f32, u8, u8)> = Vec::new();
        // Each knight whose shot mends hit something this tick, and by how
        // much: a tick's hits mend once, however many they were.
        let mut mends: BTreeMap<u8, u32> = BTreeMap::new();
        let mut hexed: Vec<u32> = Vec::new();
        // Who Rimeleap can leap to: the knights standing.
        let standing: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.targetable())
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let mut leaps = 0u32;
        let hands: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        // What a borrowed shot turns after: the monsters (`bodies`).
        let foes_at: Vec<(u32, f32, f32)> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0 && !e.buried())
            .map(|e| (e.id, e.x, e.y))
            .collect();
        let spheres = self.spheres.clone();
        let mut overkill = false;
        // What pattern shots burst into this tick.
        let mut pops: Vec<Projectile> = Vec::new();
        // The arms' feel this tick (`feel`): knights grazed, rails split
        // and turned, foes set burning, clouds a ball lit.
        let mut grazes: Vec<u32> = Vec::new();
        let mut splits: Vec<Projectile> = Vec::new();
        let mut burns: Vec<(u32, u8, u8)> = Vec::new();
        let mut poisons: Vec<(u32, u8, u8)> = Vec::new();
        let mut turned: Vec<u8> = Vec::new();
        let mut kegs: Vec<(usize, u8)> = Vec::new();
        let shields: Vec<(f32, f32)> = self.works.flights.iter().map(|f| (f.x, f.y)).collect();
        let clouds: Vec<(f32, f32, f32)> = self
            .works
            .clouds
            .iter()
            .map(|c| (c.x, c.y, c.radius()))
            .collect();
        let tick = self.tick;
        let kingdom = self.room().kind == RoomKind::Kingdom;
        let holding = self.holding();
        for bullet in &mut self.projectiles {
            // Time stands still in a Stillhour for the monsters' shots, and
            // in a room holding for its party.
            if bullet.hostile
                && (holding
                    || spheres
                        .iter()
                        .any(|s| (bullet.x - s.x).hypot(bullet.y - s.y) < s.r))
            {
                continue;
            }
            // A beam warns, then burns down its line; it never flies. A
            // borrowed one burns the monsters (`bodies`).
            if bullet.traits.path.beam.is_some() && bullet.traits.path.wait == 0 {
                if bullet.traits.borrowed {
                    let owner = u32::from(bullet.traits.owner);
                    for (i, dealt) in patterns::shine_foes(bullet, &mut self.enemies, &grid) {
                        let enemy = &mut self.enemies[i];
                        enemy.wits.foe = owner;
                        credits.push((owner, dealt));
                        self.feel.impact(enemy.x, enemy.y, bullet.kind, (0.0, 0.0));
                        self.sounds.push("hit");
                    }
                } else {
                    patterns::shine(bullet, &mut self.players, &grid, &mut self.sounds, &mut self.feel);
                }
                continue;
            }
            // A lob flies over everything and comes down on its mark; a
            // borrowed one on the monsters there.
            if bullet.traits.path.form == patterns::Form::Lob && !bullet.traits.path.waiting() {
                if bullet.traits.borrowed {
                    if let Some(struck) = patterns::lob_foes(bullet, &mut self.enemies) {
                        self.feel.impact(bullet.x, bullet.y, bullet.kind, (0.0, 0.0));
                        let owner = u32::from(bullet.traits.owner);
                        for (i, dealt) in struck {
                            self.enemies[i].wits.foe = owner;
                            credits.push((owner, dealt));
                            self.sounds.push("hit");
                        }
                    }
                } else if patterns::lob(bullet, &mut self.players, &mut self.sounds) {
                    // It bursts where it came down.
                    self.feel.impact(bullet.x, bullet.y, bullet.kind, (0.0, 0.0));
                }
                continue;
            }
            let old_x = bullet.x;
            let old_y = bullet.y;
            // How near stone and props its body comes (`patterns::body`).
            let body = patterns::body(bullet);
            // A pattern shot waits, or changes speed, turns, weaves, stops
            // or bursts, before it moves.
            let marks = if bullet.traits.borrowed { &foes_at } else { &standing };
            if patterns::steer(bullet, marks, &mut pops) == patterns::Steer::Hold {
                if bullet.ttl == 0 {
                    self.feel.boss_impact(bullet);
                }
                continue;
            }
            // A thrown blade: out for its range, then home to the hand.
            match bullet.traits.back {
                1 => {
                    bullet.traits.out = bullet.traits.out.saturating_sub(1);
                    if bullet.traits.out == 0 {
                        bullet.traits.back = 2;
                        bullet.last_hit = None;
                    }
                }
                2 => match hands.iter().find(|h| h.0 == u32::from(bullet.traits.owner)) {
                    Some(&(_, hx, hy)) => {
                        let (dx, dy) = (hx - bullet.x, hy - bullet.y);
                        let d = dx.hypot(dy);
                        if d < 1.2 {
                            bullet.ttl = 0;
                            continue;
                        }
                        let speed = wishcraft::home_speed(bullet, BLADE_SPEED);
                        (bullet.vx, bullet.vy) = (dx / d * speed, dy / d * speed);
                    }
                    None => bullet.ttl = 0,
                },
                _ => {}
            }
            // A homing shot turns toward the nearest monster, a little a tick.
            if !bullet.hostile
                && bullet.traits.homing > 0
                && let Some(target) = self.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                    (a.x - bullet.x)
                        .hypot(a.y - bullet.y)
                        .total_cmp(&(b.x - bullet.x).hypot(b.y - bullet.y))
                })
            {
                let speed = bullet.vx.hypot(bullet.vy);
                let now = bullet.vy.atan2(bullet.vx);
                let want = (target.y - bullet.y).atan2(target.x - bullet.x);
                let turn = (want - now).sin().atan2((want - now).cos());
                let most = HOMING_TURN * f32::from(bullet.traits.homing) * DT;
                let next = now + turn.clamp(-most, most);
                (bullet.vx, bullet.vy) = (next.cos() * speed, next.sin() * speed);
            }
            bullet.x += bullet.vx * DT;
            bullet.y += bullet.vy * DT;
            let stone = if !bullet.hostile && bullet.traits.back != 2 {
                grid.shot_stop_body((old_x, old_y), (bullet.x, bullet.y), body)
            } else {
                None
            };
            if let Some((x, y)) = stone {
                (bullet.x, bullet.y) = (x, y);
            }
            bullet.ttl = bullet.ttl.saturating_sub(1);
            let rail = !bullet.hostile && bullet.kind == Shot::Bolt && bullet.traits.arm;
            if rail {
                self.feel.tracer(
                    (old_x, old_y),
                    (bullet.x, bullet.y),
                    bullet.traits.temper >= 1,
                );
                // Ricochet: a rail meeting a flying shield turns off it to
                // the nearest monster it has not struck.
                if !bullet.traits.once
                    && let Some(&(sx, sy)) = shields.iter().find(|&&(sx, sy)| {
                        hit_segment(old_x, old_y, bullet.x, bullet.y, sx, sy, 0.9)
                    })
                    && let Some(foe) = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0 && !bullet.traits.pierced.contains(&e.id))
                        .min_by(|a, b| {
                            (a.x - sx)
                                .hypot(a.y - sy)
                                .total_cmp(&(b.x - sx).hypot(b.y - sy))
                        })
                {
                    let speed = bullet.vx.hypot(bullet.vy);
                    let (ux, uy) = unit(foe.x - sx, foe.y - sy);
                    (bullet.x, bullet.y) = (sx, sy);
                    (bullet.vx, bullet.vy) = (ux * speed, uy * speed);
                    bullet.traits.once = true;
                    bullet.ttl = bullet.ttl.max(HZ / 2);
                    turned.push(bullet.traits.owner);
                }
            }
            // Fire and embers flash gas too, including the Ripper's walls.
            let fire = matches!(bullet.kind, Shot::Ember | Shot::Flame) || bullet.traits.fire;
            if fire {
                for (i, &(cx, cy, r)) in clouds.iter().enumerate() {
                    if hit_segment(old_x, old_y, bullet.x, bullet.y, cx, cy, r)
                        && !kegs.iter().any(|k| k.0 == i)
                    {
                        kegs.push((i, bullet.traits.owner));
                    }
                }
            }
            let powder = !bullet.hostile && bullet.kind == Shot::Ball && bullet.traits.arm;
            if !bullet.hostile && !fire && !powder {
                if let Some(c) = self.works.clouds.iter().find(|c| {
                    c.wall() && hit_segment(old_x, old_y, bullet.x, bullet.y, c.x, c.y, c.radius())
                }) {
                    if c.age < HZ / 2 {
                        bullet.ttl = 0;
                        continue;
                    }
                    bullet.x = old_x + (bullet.x - old_x) * 0.55;
                    bullet.y = old_y + (bullet.y - old_y) * 0.55;
                }
            }
            // Powder Keg: a handgonne ball through a cloud of gas lights it.
            if !bullet.hostile && bullet.kind == Shot::Ball && bullet.traits.arm {
                for (i, &(cx, cy, r)) in clouds.iter().enumerate() {
                    let inside = hit_segment(old_x, old_y, bullet.x, bullet.y, cx, cy, r);
                    if inside && !kegs.iter().any(|k| k.0 == i) {
                        kegs.push((i, bullet.traits.owner));
                    }
                }
            }
            // A wall stops what monsters throw and lets friends' shots
            // through, stronger.
            if let Some(&(_, _, empower)) = walls
                .iter()
                .find(|&&(a, b, _)| crosses((old_x, old_y), (bullet.x, bullet.y), a, b))
            {
                if bullet.hostile {
                    bullet.ttl = 0;
                    self.sounds.push("shield_block");
                    continue;
                } else if !bullet.empowered {
                    bullet.empowered = true;
                    bullet.damage = bullet.damage * (100 + empower) / 100;
                }
            }
            if bullet.hostile && self.scarecrows.iter().any(|d| d.active(self.tick)
                && hit_segment(old_x, old_y, bullet.x, bullet.y, d.x, d.y, peasant::DECOY_RADIUS))
            {
                bullet.ttl = 0;
                continue;
            }
            if bullet.hostile
                && let Some(phantom) = self.phantoms.iter_mut().find(|p| {
                    p.left > 0 && hit_segment(old_x, old_y, bullet.x, bullet.y, p.x, p.y, 0.6)
                })
            {
                // An image takes the shot meant for its knight, and is gone.
                phantom.left = 0;
                bullet.ttl = 0;
                continue;
            }
            // A friend's shot at a knight in the stocks is a tomato.
            if !bullet.hostile
                && kingdom
                && let Some(hero) = self.players.iter_mut().find_map(|(&id, h)| {
                    (h.stocked
                        && id != u32::from(bullet.traits.owner)
                        && hit_segment(old_x, old_y, bullet.x, bullet.y, h.x, h.y, HERO_RADIUS + 0.16))
                    .then_some(h)
                })
            {
                hero.splats = hero.splats.saturating_add(1).min(99);
                bullet.ttl = 0;
                self.sounds.push("hit");
                self.score = self.score.saturating_add(1);
                continue;
            }
            if bullet.hostile {
                // The knight it meets first along its step: at a raised
                // shield's face, or at the body (`shield`). A Peasant
                // deserting is no one's mark (`peasant`).
                let (from, to) = ((old_x, old_y), (bullet.x, bullet.y));
                let mut first: Option<(u32, shield::Meet)> = None;
                for (&id, hero) in self.players.iter().filter(|(_, h)| h.targetable()) {
                    // Rimeleap never strikes the knight it just left.
                    if bullet.kind == Shot::Frost && bullet.last_hit == Some(id) {
                        continue;
                    }
                    let crouch = if self.works.kits.get(&id).is_some_and(|k| k.duck) {
                        skills::DUCK_DROP
                    } else {
                        0.0
                    };
                    // A dart strikes close, a heavy orb wide (`patterns::girth`).
                    let centre = (hero.x, hero.y + crouch);
                    if let Some(meet) = shield::meets(hero, centre, from, to, HERO_RADIUS + patterns::girth(bullet))
                        && first.is_none_or(|(_, f)| meet.t < f.t)
                    {
                        first = Some((id, meet));
                    }
                }
                // A graze: close, and past, once a shot (`feel`).
                for (&id, hero) in self.players.iter().filter(|(_, h)| h.targetable()) {
                    if bullet.traits.once || first.is_some_and(|(met, _)| met == id) {
                        continue;
                    }
                    if bullet.kind == Shot::Frost && bullet.last_hit == Some(id) {
                        continue;
                    }
                    let crouch = if self.works.kits.get(&id).is_some_and(|k| k.duck) {
                        skills::DUCK_DROP
                    } else {
                        0.0
                    };
                    let reach = HERO_RADIUS + patterns::girth(bullet);
                    let (x, y) = (hero.x, hero.y + crouch);
                    if hero.invulnerable == 0
                        && !hit_segment(old_x, old_y, bullet.x, bullet.y, x, y, reach)
                        && hit_segment(old_x, old_y, bullet.x, bullet.y, x, y, reach + feel::GRAZE)
                    {
                        bullet.traits.once = true;
                        grazes.push(id);
                    }
                }
                if let Some((id, meet)) = first {
                    bullet.ttl = 0;
                    if meet.shield {
                        // Stopped at the shield's face, never in the knight.
                        (bullet.x, bullet.y) = meet.at;
                    }
                    // A boss's shot bursts in its own bank where it lands.
                    self.feel.boss_impact(bullet);
                    if meet.shield {
                        let way = unit(bullet.vx, bullet.vy);
                        let out = self
                            .players
                            .get(&id)
                            .map_or((-way.0, -way.1), |h| (meet.at.0 - h.x, meet.at.1 - h.y));
                        let weight = shield::weight(bullet);
                        self.sounds.push("shield_block");
                        self.feel.block(id, meet.at, out, way, bullet.kind, weight);
                    } else if let Some(hero) = self.players.get_mut(&id) {
                        if hero.invulnerable == 0 && bullet.kind == Shot::Hex {
                            hexed.push(id);
                        } else if bullet.kind == Shot::Frost {
                            if hero.invulnerable == 0 {
                                hero.hurt(bullet.damage);
                                self.sounds.push("hero_hurt");
                            }
                            if hero.immune == 0 {
                                hero.chilled = lich::CHILL;
                            }
                            // Rimeleap leaps on to the nearest other knight.
                            if bullet.pierce > 0
                                && let Some((_, nx, ny)) =
                                    lich::next_link(&standing, id, (hero.x, hero.y))
                            {
                                let speed = bullet.vx.hypot(bullet.vy).max(1.0);
                                let (ux, uy) = unit(nx - hero.x, ny - hero.y);
                                (bullet.x, bullet.y) = (hero.x, hero.y);
                                (bullet.vx, bullet.vy) = (ux * speed, uy * speed);
                                bullet.pierce -= 1;
                                bullet.last_hit = Some(id);
                                bullet.ttl = 4 * HZ;
                                leaps += 1;
                            }
                        } else if bullet.traits.blow != 0 {
                            if hero.blow(bullet.damage, bullet.traits.blow) {
                                self.sounds.push("hero_hurt");
                            }
                        } else if hero.invulnerable == 0 {
                            hero.hurt(bullet.damage);
                            self.sounds.push("hero_hurt");
                        }
                    }
                }
            } else {
                // A fast rail may cross several bodies in one tick. Visit
                // them in flight order and keep every foe it has touched.
                let mut order: Vec<usize> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.hp > 0 && !e.buried())
                    .map(|(i, _)| i)
                    .collect();
                if rail {
                    order.sort_by(|&a, &b| {
                        let along =
                            |e: &Enemy| (e.x - old_x) * bullet.vx + (e.y - old_y) * bullet.vy;
                        along(&self.enemies[a]).total_cmp(&along(&self.enemies[b]))
                    });
                }
                for i in order {
                    let enemy = &mut self.enemies[i];
                    // A tempered rail is thicker.
                    let thick = if rail && bullet.traits.temper >= 1 {
                        0.35
                    } else {
                        0.0
                    };
                    if bullet.last_hit != Some(enemy.id)
                        && (!rail || !bullet.traits.pierced.contains(&enemy.id))
                        && hit_segment(
                            old_x,
                            old_y,
                            bullet.x,
                            bullet.y,
                            enemy.x,
                            enemy.y,
                            enemy.radius() + thick + bullet.reach(),
                        )
                    {
                        if critters::blocks(enemy, bullet.vx, bullet.vy) {
                            // A tower shield turns it, but each shot it
                            // turns tells: enough and it is knocked aside.
                            bullet.ttl = 0;
                            if critters::guard_struck(enemy, bullet.damage) {
                                self.sounds.push("clang");
                            } else {
                                self.sounds.push("shield_block");
                            }
                            break;
                        }
                        // A shot that passes through counts the monsters it
                        // meets, as one stroke (`aoe`).
                        let dealt = aoe::part(bullet.damage, usize::from(bullet.traits.struck));
                        let dealt = dealt
                            + signatures::rear_bonus(enemy, &self.bosses, (old_x, old_y), dealt);
                        let first_hit = bullet.traits.struck == 0;
                        // One arrow, two foes: a pierce counts for the
                        // school's boards the moment it meets its second.
                        if bullet.traits.struck == 1 && enemy.hp == 0 {
                            let owner = u32::from(bullet.traits.owner);
                            self.school.note(owner, "archer-pierce", 1);
                        }
                        bullet.traits.struck = bullet.traits.struck.saturating_add(1);
                        if rail {
                            bullet.traits.pierced.push(enemy.id);
                        }
                        let actual = dealt.min(enemy.hp);
                        enemy.hp = enemy.hp.saturating_sub(dealt);
                        if actual > 0 && enemy.is_boss()
                            && (bullet.empowered || matches!(bullet.kind, Shot::Sacred | Shot::Missile))
                        {
                            self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                        }
                        bullet.last_hit = Some(enemy.id);
                        if bullet.traits.owner > 0 {
                            let owner = u32::from(bullet.traits.owner);
                            credits.push((owner, actual));
                            // It turns on whoever hurt it (`press`).
                            enemy.wits.foe = owner;
                            self.floor_tally.line(owner).hits +=
                                u32::from(first_hit && (bullet.traits.arm || bullet.look.is_some()));
                            if feel::ledger_on() {
                                self.feel.wounded.entry(enemy.id).or_insert(tick);
                                let tally = self.feel.ledger.entry(owner).or_default();
                                tally.hits += u32::from(
                                    first_hit && (bullet.traits.arm || bullet.look.is_some()),
                                );
                            }
                        }
                        // The hit lands: sparks, a jolt, a shove (`feel`).
                        let (ux, uy) = unit(bullet.vx, bullet.vy);
                        self.feel.impact(
                            enemy.x - ux * enemy.radius(),
                            enemy.y - uy * enemy.radius(),
                            bullet.kind,
                            (ux, uy),
                        );
                        let anchored = enemy.asleep(self.tick) || enemy.is_boss()
                            || matches!(enemy.kind, EnemyKind::Dummy | EnemyKind::Ward);
                        if enemy.hp > 0 && !anchored {
                            if enemy.wits.flinch == 0 {
                                enemy.frozen = enemy.frozen.max(feel::JOLT_HIT);
                                enemy.wits.flinch = feel::FLINCH;
                            }
                            // A Peasant's stone shoves its own way; else a
                            // wish's knock (`wishcraft`).
                            let push = if peasant::stone(bullet) { peasant::STONE_KNOCK } else { wishcraft::knock(bullet) };
                            if push > 0.0 {
                                let (ux, uy) = unit(bullet.vx, bullet.vy);
                                let mover = if enemy.kind.flies() {
                                    Mover::Flier
                                } else {
                                    Mover::Walker
                                };
                                let shove = (ux * push, if side { 0.0 } else { uy * push });
                                (enemy.x, enemy.y) =
                                    grid.slide((enemy.x, enemy.y), shove, enemy.radius(), mover);
                            }
                        }
                        if enemy.hp == 0
                            && matches!(bullet.kind, Shot::Bolt | Shot::Ball | Shot::Sacred)
                        {
                            self.fx.push(Fx::Shake(feel::SHAKE_KILL));
                        }
                        // The Gunner's boards: a foe a handgonne ball felled.
                        if enemy.hp == 0 && bullet.kind == Shot::Ball && bullet.traits.arm {
                            let owner = u32::from(bullet.traits.owner);
                            self.school.note(owner, "gunner-ball", 1);
                        }
                        if bullet.traits.fire || bullet.traits.burn > 0 {
                            burns.push((enemy.id, bullet.traits.owner, bullet.traits.burn.max(1)));
                        }
                        wishcraft::chill(enemy, bullet);
                        if bullet.traits.poison > 0 {
                            poisons.push((enemy.id, bullet.traits.owner, bullet.traits.poison));
                        }
                        // A twice-tempered rail splits on its first kill.
                        if rail && enemy.hp == 0 && bullet.traits.temper >= 2 {
                            bullet.traits.temper = 1;
                            for turn in [-0.45f32, 0.45] {
                                let (c, s) = (turn.cos(), turn.sin());
                                let mut child = bullet.clone();
                                (child.vx, child.vy) =
                                    (bullet.vx * c - bullet.vy * s, bullet.vx * s + bullet.vy * c);
                                (child.x, child.y) = (enemy.x, enemy.y);
                                child.damage = bullet.damage / 2;
                                child.ttl = HZ / 2;
                                child.traits.once = true;
                                splits.push(child);
                            }
                        }
                        if bullet.damage >= 300 {
                            overkill = true;
                        }
                        if bullet.traits.stun > 0 {
                            enemy.frozen = enemy.frozen.max(u32::from(bullet.traits.stun));
                        }
                        self.sounds.push("hit");
                        // A burst and a spark follow what the hit dealt.
                        if bullet.kind == Shot::Ball
                            && (!bullet.traits.arm || bullet.traits.temper >= 3)
                        {
                            // A thrice-tempered handgonne's ball bursts: half
                            // to all around.
                            bursts.push((
                                enemy.id,
                                enemy.x,
                                enemy.y,
                                dealt / 2,
                                bullet.traits.owner,
                            ));
                        } else if bullet.traits.burst > 0 {
                            let part = dealt * u32::from(bullet.traits.burst) / 100;
                            bursts.push((
                                enemy.id,
                                enemy.x,
                                enemy.y,
                                part.max(1),
                                bullet.traits.owner,
                            ));
                        }
                        if bullet.traits.chain > 0 {
                            chains.push((
                                enemy.id,
                                enemy.x,
                                enemy.y,
                                dealt / 2,
                                bullet.traits.chain,
                                bullet.traits.owner,
                            ));
                        }
                        if bullet.traits.mend > 0 {
                            let mend = mends.entry(bullet.traits.owner).or_default();
                            *mend = (*mend).max(u32::from(bullet.traits.mend));
                        }
                        if bullet.traits.fart > 0 && bullet.traits.owner > 0 {
                            farts.push((enemy.x, enemy.y, bullet.traits.owner, bullet.traits.fart));
                        }
                        if bullet.pierce == 0 && wishcraft::turn_home(bullet) {
                            // A boomerang turns home off what it struck.
                            break;
                        } else if bullet.pierce == 0 {
                            // Fork: where it stops, two fly on at an angle,
                            // past the one it struck, as new shots.
                            if bullet.traits.fork > 0 && bullet.traits.back == 0 {
                                for turn in [-FORK_TURN, FORK_TURN] {
                                    let (c, s) = (turn.cos(), turn.sin());
                                    let mut child = bullet.clone();
                                    (child.vx, child.vy) = (
                                        bullet.vx * c - bullet.vy * s,
                                        bullet.vx * s + bullet.vy * c,
                                    );
                                    (child.x, child.y) = (enemy.x, enemy.y);
                                    child.ttl = bullet.ttl.max(FORK_TTL);
                                    child.last_hit = Some(enemy.id);
                                    child.traits.fork -= 1;
                                    child.traits.struck = 0;
                                    splits.push(child);
                                }
                            }
                            bullet.ttl = 0;
                        } else {
                            bullet.pierce -= 1;
                        }
                        if !rail || bullet.ttl == 0 {
                            break;
                        }
                    }
                }
            }
            if stone.is_some()
                || (bullet.traits.back != 2 && grid.stops_body(bullet.x, bullet.y, body))
                || grid.stops_shot(bullet.x, bullet.y)
            {
                if bullet.traits.back == 1 {
                    // A thrown blade turns home off the stone; coming back
                    // it passes over everything.
                    (bullet.x, bullet.y) = (old_x, old_y);
                    bullet.traits.back = 2;
                    bullet.last_hit = None;
                } else if bullet.traits.back == 2 {
                } else if bullet.traits.bounce > 0 && bullet.ttl > 0 {
                    // Off the stone it came against: across, along, or both
                    // in a corner. It may hit the same monster again. A
                    // monster's bouncing pattern shot comes off the same way.
                    let across = grid.stops_body(bullet.x, old_y, body);
                    let along = grid.stops_body(old_x, bullet.y, body);
                    if across || !along {
                        bullet.vx = -bullet.vx;
                    }
                    if along || !across {
                        bullet.vy = -bullet.vy;
                    }
                    (bullet.x, bullet.y) = (old_x, old_y);
                    bullet.traits.bounce -= 1;
                    bullet.last_hit = None;
                } else {
                    // Arrows and bolts stick in the stone till the room ends.
                    if !bullet.hostile
                        && matches!(bullet.kind, Shot::Arrow | Shot::Bolt)
                        && bullet.look.is_none()
                    {
                        self.feel
                            .stick(bullet.x, bullet.y, (bullet.vx, bullet.vy), bullet.kind);
                    } else if bullet.hostile && bosses::Art::of_shot(bullet).is_some() {
                        // A library guardian's shot breaks in its own bank.
                        self.feel.boss_impact(bullet);
                    } else {
                        // A monster's shot breaks on stone with a spark too.
                        self.feel.impact(bullet.x, bullet.y, bullet.kind, (bullet.vx, bullet.vy));
                    }
                    bullet.ttl = 0;
                }
            }
        }
        self.projectiles.retain(|p| p.ttl > 0);
        let room = MAX_PROJECTILES.saturating_sub(self.projectiles.len());
        let popped = self.projectiles.len();
        self.projectiles.extend(pops.into_iter().take(room));
        // A knight's forks and splits, while they have room under
        // `KNIGHT_CHILDREN` (a fork of a fork of a fork, on every shot of a
        // stacked volley, is eight a shot).
        let mut flying = [0usize; 256];
        for p in self.projectiles.iter().filter(|p| !p.hostile) {
            flying[usize::from(p.traits.owner)] += 1;
        }
        self.projectiles.extend(
            splits
                .into_iter()
                .filter(|s| {
                    let owner = usize::from(s.traits.owner);
                    if s.hostile || owner == 0 {
                        return true;
                    }
                    flying[owner] += 1;
                    flying[owner] <= KNIGHT_CHILDREN
                })
                .take(MAX_PROJECTILES.saturating_sub(self.projectiles.len())),
        );
        self.hostile_shots += hostile_past(&self.projectiles, popped);
        // The party's shots keep to their share of the screen (`cover`).
        let view = cover::view(self.room());
        cover::hold(&mut self.projectiles, view);
        self.land_synergies(burns, turned, kegs);
        self.poison_hits(poisons);
        if overkill {
            self.notice("overkill");
        }
        self.phantoms.retain(|p| p.left > 0);
        self.cast_ults(casts);
        for (owner, card) in summons {
            self.call_imps(owner, &card);
        }
        self.tick_ults();
        self.tick_bard_songs();
        self.charge_ults(&credits);
        self.credit_bats(&credits);
        self.apply_deeds(deeds);
        self.tick_works(inputs);
        self.shoot_kegs();
        self.tick_kegs_and_lobs();
        self.tick_flings();
        self.tick_wells();
        self.tick_critters();
        self.tick_frogs();
        self.tick_hunters();
        self.tick_tallow();
        self.pet_tallow(inputs);
        // The follies and the rooms' small games (`activity::FOLLIES`).
        self.tick_follies(inputs);
        self.tick_pots();
        self.tick_cage(inputs);
        self.tick_runes();
        self.tick_hexes();
        self.tick_chill();
        self.tick_grip();
        self.tick_skulls();
        self.tick_items();
        self.tick_song();
        self.tick_hireling();
        self.tick_snibbet();
        self.small_secrets(&booms, &draughts);
        self.spark(chains);
        self.fart_hits(farts);
        for (hit, x, y, damage, owner) in bursts {
            let struck = aoe::strike(&mut self.enemies, (x, y), damage, |e| {
                e.id != hit && (e.x - x).hypot(e.y - y) <= BURST_REACH + e.radius()
            });
            for (i, damage) in struck {
                let enemy = &mut self.enemies[i];
                enemy.wits.foe = u32::from(owner);
                self.feel
                    .credit(u32::from(owner), enemy.id, damage, self.tick);
            }
            self.fx.push(Fx::Shake(4));
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
        }
        for blast in &mut self.blasts {
            blast.2 = blast.2.saturating_sub(1);
        }
        self.blasts.retain(|b| b.2 > 0);
        let now = self.tick;
        self.boons
            .retain(|b| now.saturating_sub(b.1) < u64::from(HZ));
        for id in hexed {
            self.hex(id);
        }
        if leaps > 0 {
            self.cues.push("frost_leap".into());
        }
        if bombs > 0 && self.open_map() {
            // On an open map only a bomb by the crack brings it down.
            for &at in &booms {
                self.blast_wall_open(at);
            }
        } else if bombs > 0 {
            // A knight's bomb fills the room: a cracked wall comes down.
            self.blast_wall(None);
        }
        for mend in mends.into_iter() {
            let (mender, mend) = mend;
            // The Healer's boards: mending done, a point at a time.
            if mend > 0 && mender > 0 {
                self.school.note(u32::from(mender), "healer-mend", mend);
            }
            // The most hurt knight standing is mended.
            if let Some(hero) = self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && h.hp < h.max_hp)
                .min_by_key(|h| h.hp * 100 / h.max_hp.max(1))
            {
                hero.leech(mend, stakes::Mend::Passive);
            }
        }
        self.swing_orbits();
        self.tick_imps();
        self.projectiles.retain(|p| p.ttl > 0);
        self.pit_events(pit_events);
        self.visit_privy();
        // The Undercroft's rooms, the barony, the holdings, the College, the
        // ranch and the merchant (`activity::PLACES`).
        self.tick_places(inputs);
        self.tick_traps();
        self.tick_collapse();
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            let touch = self
                .enemies
                .iter()
                .filter(|e| {
                    e.hp > 0
                        && e.age >= TELEGRAPH
                        && !e.dormant
                        && e.frozen == 0
                        && !e.asleep(self.tick)
                        && e.kind != EnemyKind::Dummy
                        && e.kind != EnemyKind::Goblin
                        && e.hurts_by_touch()
                        && (hero.x - e.x).hypot(hero.y - e.y) < e.radius() + 0.5
                })
                .map(Enemy::touch)
                .max();
            if let Some(damage) = touch {
                hero.hurt(damage);
            }
        }
        // The school's boards: whoever fell this floor is out of the
        // nobody-fell missions.
        let fell: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp == 0)
            .map(|(&id, _)| id)
            .collect();
        for id in fell {
            self.school_fell(id);
        }
        // A knight keeping vigil mends the living knights within its reach.
        if self.tick.is_multiple_of(u64::from(VIGIL_MEND_EVERY)) {
            let wards: Vec<(f32, f32)> = self
                .players
                .values()
                .filter(|h| h.vigil && h.hp > 0)
                .map(|h| (h.x, h.y))
                .collect();
            for hero in self.players.values_mut() {
                if hero.hp > 0
                    && hero.hp < hero.max_hp
                    && !hero.vigil
                    && wards
                        .iter()
                        .any(|&(x, y)| (hero.x - x).hypot(hero.y - y) < VIGIL_REACH)
                {
                    hero.mend(1, stakes::Mend::Passive);
                }
            }
        }
        self.second_winds();
        for hero in self.players.values_mut() {
            hero.dress();
        }
        self.mark_hits();
        for ((was, max), (&id, hero)) in before.into_iter().zip(self.players.iter()) {
            if hero.hp < was {
                // The floor's tally: the blow taken, and a fall.
                if self.dungeon.depth > 0 {
                    let line = self.floor_tally.line(id);
                    line.taken = line.taken.saturating_add(was - hero.hp);
                    line.falls += u32::from(hero.hp == 0);
                }
                self.room_hurt = true;
                if hero.hp == 0 {
                    self.cues.push("knight_down".into());
                } else if was * 10 >= max * 3 && hero.hp * 10 < max * 3 {
                    self.cues.push("low_hp".into());
                }
            }
        }
        for enemy in self.enemies.iter_mut().filter(|e| !e.raged && e.hp > 0) {
            if let Some(boss) = enemy.boss.and_then(|i| self.bosses.get(usize::from(i)))
                && boss.rage > 0.0
                && (enemy.hp as f32) < boss.rage * enemy.max_hp as f32
            {
                enemy.raged = true;
                self.cues.push(format!("boss_rage:{}", boss.id));
            }
        }
        for (id, raged, frozen, phase) in boss_beats {
            if let Some(e) = self.enemies.iter().find(|e| e.id == id && e.hp > 0)
                && ((!raged && e.raged) || (frozen == 0 && e.frozen > 0) || self.boss_phase(e) != phase)
            {
                self.hitstop.request(&self.tune, hitstop::Impact::Phase);
            }
        }
        self.tick_feel(&credits, &grazes);
        self.settle_kills();
        self.gather();
        self.tick_waves();
        // Beaten by raiders in a castle's hall at home: knocked down, not
        // killed.
        if self.raid_rout() {
            return;
        }
        // A party falls only when every knight someone plays has fallen. A
        // knight turned to stone because its player is away is still in the
        // run, waiting for them, and keeps it up: one knight's fall never
        // ends a run while a friend's knight stands (the fallen one's skull
        // waits to be carried). Only a seat nobody plays (`absent`, a host
        // with no knight of their own) is left out.
        if self.players.values().filter(|h| !h.absent).all(|h| h.hp == 0)
            && self.players.values().any(|h| h.hp == 0)
        {
            self.phase = Phase::Wiped;
            self.close_ledger("wipe");
            self.projectiles.clear();
            self.cues.push("wipe".into());
            self.bank_all(false, "the party fell");
            for id in self.players.keys().copied().collect::<Vec<_>>() { self.restore_gear(id); }
            return;
        }
        // An open map's fights are its chambers' (`depths`).
        if self.open_map() {
            self.tick_open();
            if self.phase == Phase::Exploring {
                self.travel();
            }
            return;
        }
        if !self.foes_left()
            && self.phase == Phase::Fighting
            && !self.waves.pending()
            && self.boss_room_can_clear()
            && !self.folly_running()
        {
            let first_clear = !self.room().cleared;
            // The monsters' shots still in the air turn to gold.
            self.cancel_shots();
            self.close_ledger("clear");
            self.projectiles.clear();
            self.rocks.clear();
            self.ravages.clear();
            self.holes.clear();
            self.dungeon.rooms[self.at].cleared = true;
            // For King Brannoc's missions: a hall of a delve freed.
            if self.dungeon.depth > 0 {
                *self
                    .marks
                    .entry(barony::clear_mark(self.dungeon.pack))
                    .or_default() += 1;
            }
            self.fair_chest();
            // Two knights standing together earn the realm a bond: one a
            // room for the party, carried by the first of them.
            let standing = self
                .players
                .values()
                .filter(|h| h.hp > 0 && !h.stone)
                .count();
            if standing >= 2 && first_clear {
                if let Some(hero) = self.players.values_mut().find(|h| h.hp > 0 && !h.stone) {
                    hero.carried.add(Spoil::Bond, 1);
                }
                self.cues.push("bond".into());
                self.notice("bonded");
            }
            if !self.room_hurt {
                self.notice("flawless");
                // The school's boards: a room cleared without a scratch,
                // by every knight standing.
                let clean = self
                    .players
                    .iter()
                    .filter(|(_, h)| h.hp > 0 && !h.stone)
                    .map(|(&id, _)| id)
                    .collect::<Vec<_>>();
                for id in clean {
                    self.school.note(id, "floor-untouched", 1);
                    self.school.note(id, "healer-unfallen", 1);
                    self.school.note(id, "shield-unmoved", 1);
                }
            }
            self.cues.push(
                if self.room_hurt {
                    "room_clear"
                } else {
                    "flawless"
                }
                .into(),
            );
            self.sounds.push("door_open");
            // The floor's boss fell here: a wish window, while the floor
            // has one (`overclass`). Asked before the stairs become a well.
            let boss = self.floor_boss_here();
            // The first floor's guardian guarded a Sanctuary: every run
            // reaches a reforge before the descent.
            if self.room().kind == RoomKind::Stairs && self.dungeon.depth == 1 {
                self.dungeon.rooms[self.at].kind = RoomKind::Sanctuary;
                self.stairs_held = true;
                self.hallow();
            }
            if self.room().kind == RoomKind::Lair {
                // The dragon is slain: its floor reclaimed and banked, and
                // the way splits — the stairs into the deep, or the light home.
                if let Some(hero) = self.players.values_mut().find(|h| h.hp > 0 && !h.stone) {
                    hero.carried.add(Spoil::Scale, 1);
                }
                self.reclaimed.push(self.dungeon.pack);
                self.cues.push("victory".into());
                self.fx.push(Fx::Shake(36));
                self.sounds.push("boss_fall");
                self.bank_all(true, "the dragon is slain");
                self.triumph = Some(Triumph::Dragon);
                self.notice("dragonslayer");
                self.unwashed_check();
                let room = &mut self.dungeon.rooms[self.at];
                room.open_stairs();
                self.light = Some((room.width() / 2.0, room.height() / 2.0 - 4.0));
                self.raise_hell_gate();
                self.stairs_held = true;
                self.cues.push("the_deep".into());
            }
            if self.in_mire_kings_hall() {
                // The Mire King is fallen: the pen opens, and once the
                // white horse is out, the light leads home.
                self.mire_king_falls();
            }
            if self.room().kind == RoomKind::Threshold {
                // The Unknown met: the Grail, and home.
                self.phase = Phase::Won;
                for hero in self.players.values_mut().filter(|h| h.hp > 0) {
                    hero.carried.add(Spoil::Gem, 3);
                    hero.carried.add(Spoil::Scale, 1);
                }
                self.reclaimed.push(self.dungeon.pack);
                self.cues.push("grail".into());
                self.fx.push(Fx::Shake(24));
                self.sounds.push("wheel_land");
                self.bank_all(true, "the Grail");
                self.triumph = Some(Triumph::Grail);
                self.notice("the_grail");
                self.unwashed_check();
                return;
            }
            self.phase = Phase::Exploring;
            if boss {
                // The floor's boss fell: a super wish each (`overclass`).
                self.give_wishes(0, 1);
                // And the cauldron rises by the stairs (`cauldron`).
                self.raise_cauldron();
            }
        }
        if self.phase == Phase::Exploring {
            self.travel();
        }
    }

    fn settle_kills(&mut self) {
        // An outlaw felled after the hall's tick is paid before it is reaped.
        self.pay_outlaw();
        let mut camp_deaths = Vec::new();
        let mut carrier_deaths: Vec<(u8, f32, f32, EnemyKind)> = Vec::new();
        let mut emission_deaths = Vec::new();
        let mut fallen: Vec<(EnemyKind, f32, f32)> = Vec::new();
        let mut hoards: Vec<(Vec<EnemyKind>, f32, f32)> = Vec::new();
        let mut guardians: Vec<(usize, f32, f32)> = Vec::new();
        let mut mimics = Vec::new();
        let mut foes_fallen = Vec::new();
        let mut goblins_fallen = Vec::new();
        // The Wanted hall keeps its own company: not the floor's provision.
        let showdown = matches!(self.room().kind, RoomKind::Wanted | RoomKind::Holding);
        self.enemies.retain(|e| {
            let camp_duplicate = e.camp_slot.is_some_and(|slot| {
                camp_deaths.contains(&slot)
                    || self
                        .boss_gates
                        .as_ref()
                        .is_none_or(|g| !g.economy.alive_slot(self.at, slot, e.kind))
            });
            if e.hp == 0
                && !camp_duplicate
                && let Some(slot) = e.camp_slot
            {
                camp_deaths.push(slot);
                carrier_deaths.push((slot, e.x, e.y, e.kind));
            }
            let emission_duplicate = !showdown
                && self.boss_gates.as_ref().is_some_and(|g| {
                    e.camp_slot.is_none()
                        && !g.encounter(self.at, e.kind, e.boss)
                        && (emission_deaths.contains(&e.id)
                            || !g.economy.alive_emission(self.at, e.id, e.kind, e.stage))
                });
            if e.hp == 0
                && !emission_duplicate
                && e.camp_slot.is_none()
                && self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.economy.emission(e.id).is_some())
            {
                emission_deaths.push(e.id);
            }
            let duplicate = camp_duplicate
                || emission_duplicate
                || self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.encounter(self.at, e.kind, e.boss))
                    && (e
                        .boss
                        .is_some_and(|b| guardians.iter().any(|&(i, _, _)| i == usize::from(b)))
                        || (matches!(e.kind, EnemyKind::Dragon | EnemyKind::PitTyrant)
                            && fallen.iter().any(|&(k, _, _)| k == e.kind)));
            if e.hp == 0 && !duplicate && e.kind == EnemyKind::Mimic {
                mimics.push((e.x, e.y));
            }
            if e.hp == 0
                && !duplicate
                && matches!(
                    e.kind,
                    EnemyKind::Slime | EnemyKind::Goblin | EnemyKind::Geode
                )
            {
                if e.kind == EnemyKind::Goblin {
                    goblins_fallen.push((e.x, e.y, e.stash.clone(), e.loot.clone()));
                } else {
                    foes_fallen.push((e.kind, e.x, e.y, e.stage));
                }
            }
            if e.hp == 0
                && !duplicate
                && !self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.already_defeated(self.at, e.kind, e.boss))
            {
                if e.wits.foe > 0 && self.players.contains_key(&e.wits.foe) {
                    self.floor_tally.line(e.wits.foe).kills += 1;
                }
                if feel::ledger_on() && e.wits.foe > 0 {
                    let tally = self.feel.ledger.entry(e.wits.foe).or_default();
                    tally.kills += 1;
                    if let Some(first) = self.feel.wounded.remove(&e.id) {
                        tally.ttk += self.tick.saturating_sub(first);
                    }
                    // The school's boards count what felled it, by class.
                    self.school.note(e.wits.foe, "sword-duel", 1);
                    self.school.note(e.wits.foe, "berserk-spin", 1);
                    self.school.note(e.wits.foe, "mage-staff", 1);
                    self.school.note(e.wits.foe, "shield-hold", 1);
                    self.school.note(e.wits.foe, "bard-fell", 1);
                    self.school.note(e.wits.foe, "peasant-fell", 1);
                    if e.boss.is_some() {
                        self.school.note(e.wits.foe, "sword-guardian", 1);
                        self.school.note(e.wits.foe, "mage-guardian", 1);
                        self.school.note(e.wits.foe, "bard-guardian", 1);
                        self.school.note(e.wits.foe, "peasant-guardian", 1);
                    }
                }
                if e.is_boss() {
                    self.hitstop.request(&self.tune, hitstop::Impact::BossKill);
                } else if hitstop::elite(e) {
                    self.hitstop.request(&self.tune, hitstop::Impact::EliteKill);
                }
                fallen.push((e.kind, e.x, e.y));
                if !e.hoard.is_empty() {
                    hoards.push((e.hoard.clone(), e.x, e.y));
                }
                if e.kind == EnemyKind::Boss
                    && let Some(boss) = e.boss
                {
                    guardians.push((usize::from(boss), e.x, e.y));
                }
            }
            e.hp > 0
        });
        if let Some(g) = &mut self.boss_gates {
            for slot in camp_deaths {
                g.economy.casualty(self.at, slot);
            }
            for id in emission_deaths {
                g.economy.retire(id);
            }
        }
        // Actual corpse settlement is the only source of leader-loss evidence.
        let was_open = self
            .boss_gates
            .as_ref()
            .is_some_and(boss_gates::Gates::unlocked);
        let mut unsealed = false;
        let mut carrier_fell = None;
        if let Some(g) = &mut self.boss_gates {
            for &(boss, _, _) in &guardians {
                g.record(self.at, EnemyKind::Boss, Some(boss as u8));
            }
            // The chief's camp's elite fell with the Boss Key: its route is won.
            for &(slot, x, y, kind) in &carrier_deaths {
                if g.carrier_fell(self.at, slot) {
                    carrier_fell = Some((x, y, kind));
                }
            }
            if fallen.iter().any(|&(kind, _, _)| kind == EnemyKind::Dragon) {
                g.record(self.at, EnemyKind::Dragon, None);
            }
            if fallen
                .iter()
                .any(|&(kind, _, _)| kind == EnemyKind::PitTyrant)
            {
                g.record(self.at, EnemyKind::PitTyrant, None);
            }
            if !was_open && g.unlocked() {
                self.cues.push("boss_unsealed".into());
                unsealed = true;
            }
        }
        // The key carrier whose fall opened the route drops the Boss Key.
        if let Some((_, _, kind)) = carrier_fell {
            self.cues
                .push(format!("leader_fall:{}", keyring::bearer_name(kind)));
        }
        if unsealed
            && let Some((x, y)) = carrier_fell
                .map(|(x, y, _)| (x, y))
                .or(guardians.last().map(|&(_, x, y)| (x, y)))
        {
            self.drop_boss_key(x, y);
        }
        // A mimic coughs up the treasure it was pretending to be.
        for (x, y) in mimics {
            let pack = self.dungeon.pack;
            let loot = self.book.roll_chest(&mut self.rng, pack);
            for (i, card) in loot.into_iter().enumerate() {
                let angle = std::f32::consts::FRAC_PI_2 + (i as f32 - 1.0) * 1.1;
                let bottom = self.room().height() - 3.0;
                self.drop_item(
                    card,
                    x + angle.cos() * 2.6,
                    (y + angle.sin() * 2.2).clamp(3.0, bottom),
                    None,
                );
            }
            self.cues.push("mimic_fall".into());
        }
        self.leave_corpses(&fallen, &guardians);
        // Every death holds a white frame, then bursts (`feel`).
        for &(kind, x, y) in &fallen {
            self.feel.death(x, y, kind);
        }
        // Humphrey falls apart, politely, and starts reassembling.
        if fallen.iter().any(|&(kind, ..)| kind == EnemyKind::Risen) {
            let c = &mut self.home.countryside;
            c.humphrey_felled = c.humphrey_felled.saturating_add(1).min(9);
            c.humphrey_dig = risen::RISE_AGAIN;
            self.found = Some((
                self.tick,
                0,
                format!(
                    "HUMPHREY THE RISEN falls apart, politely (+{} bounty). Give him a minute; \
                     he is reassembling.",
                    EnemyKind::Risen.bounty()
                ),
            ));
            self.sounds.push("boss_fall");
            self.fx.push(Fx::Shake(10));
        }
        // A guardian scatters its spoils as cards, plus one prize.
        for (index, x, y) in guardians {
            let Some(boss) = self.bosses.get(index).cloned() else {
                continue;
            };
            let mut drops: Vec<String> = Vec::new();
            for (&spoil, &n) in &boss.drops.0 {
                let copies = if spoil == Spoil::Gold {
                    n.div_ceil(50)
                } else {
                    n
                };
                drops.extend((0..copies).map(|_| spoil.word().to_string()));
            }
            let pack = self.dungeon.pack;
            drops.extend(
                self.book
                    .roll_chest(&mut self.rng, pack)
                    .into_iter()
                    .take(1),
            );
            let count = drops.len().max(1) as f32;
            for (i, card) in drops.into_iter().enumerate() {
                if self.book.get(&card).is_none() {
                    continue;
                }
                let angle = i as f32 / count * std::f32::consts::TAU;
                let (w, h) = (self.room().width(), self.room().height());
                let (cx, cy) = (
                    x + angle.cos() * 3.2,
                    (y + angle.sin() * 2.4).clamp(3.0, h - 3.0),
                );
                self.drop_item(card, cx.clamp(3.0, w - 3.0), cy, None);
            }
            self.found = Some((self.tick, 0, format!("{} falls", boss.name)));
            self.notice("bigger_they_are");
            self.fx.push(Fx::Shake(24));
            self.sounds.push("boss_fall");
            self.cues.push(format!("boss_fall:{}", boss.id));
        }
        let slain = fallen.len() as u32;
        if slain > 0 {
            self.sounds.push("kill");
        }
        if slain > 0 {
            if !self.blooded {
                self.blooded = true;
                self.cues.push("first_blood".into());
            }
            let count = if self.tick.saturating_sub(self.streak.0) <= u64::from(2 * HZ) {
                self.streak.1 + slain
            } else {
                slain
            };
            self.streak = (self.tick, count);
            match count {
                2 => self.cues.push("slay2".into()),
                3 => self.cues.push("slay3".into()),
                n if n >= 4 && n - slain < 4 => self.cues.push("slay4".into()),
                _ => {}
            }
        }
        let red = self.song.as_deref() == Some("red");
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            // The Red Ballad mends a knight who hears it, kill by kill, but
            // no leech stacks past the held cards' cap; and a tick's kills
            // mend once, however many fell to one blow.
            let vamp = (hero.bonus.vamp
                + if red && hero.singing {
                    tavern::SONG_VAMP
                } else {
                    0
                })
            .min(cards::cap("vamp"));
            if slain > 0 {
                hero.leech(vamp, stakes::Mend::Passive);
            }
        }
        let pack = self.dungeon.pack;
        self.slimes_slain += foes_fallen
            .iter()
            .filter(|f| f.0 == EnemyKind::Slime)
            .count() as u32
            + fallen.iter().filter(|f| f.0 == EnemyKind::Slime).count() as u32
                * u32::from(foes_fallen.is_empty());
        if self.slimes_slain >= 7 {
            self.notice("splitting_headache");
        }
        self.fallen_foes(&foes_fallen);
        for (x, y, stash, loot) in goblins_fallen {
            self.goblin_bursts(x, y, stash, loot);
        }
        for &(kind, ..) in &fallen {
            self.mark_kill(kind);
        }
        let kinds: Vec<EnemyKind> = fallen.iter().map(|&(kind, ..)| kind).collect();
        self.mark_experience(&kinds);
        self.pit_fallen(&fallen);
        let generous = if self.mode == fortune::Mode::GiantsFeast {
            2
        } else {
            1
        };
        for (kind, x, y) in fallen {
            if !matches!(kind, EnemyKind::Dummy | EnemyKind::Ward) { self.early_gear_drop(x, y); }
            self.score += kind.bounty() * generous;
            if self
                .rng
                .chance((loot::drop_chance(kind) * generous).min(100))
            {
                let card = if kind == EnemyKind::Dragon && self.book.get("heart").is_some() {
                    Some("heart".to_string())
                } else {
                    self.book.roll_drop(&mut self.rng, pack)
                };
                if let Some(card) = card {
                    self.drop_item(card, x, y, None);
                }
            }
            if self
                .rng
                .chance((loot::spoil_chance(kind) * generous).min(100))
                && let Some(card) = self.book.roll_spoil(&mut self.rng, pack)
            {
                self.drop_item(card, x + 0.9, y + 0.5, None);
            }
        }
        // A boss that stood alone leaves what its company would have.
        for (hoard, x, y) in hoards {
            for (n, kind) in hoard.into_iter().enumerate() {
                let (dx, dy) = ((n % 3) as f32 * 1.4 - 1.4, (n / 3) as f32 * 1.2 + 1.0);
                if self.rng.chance((loot::drop_chance(kind) * generous).min(100))
                    && let Some(card) = self.book.roll_drop(&mut self.rng, pack)
                {
                    self.drop_item(card, x + dx, y + dy, None);
                }
                if self.rng.chance((loot::spoil_chance(kind) * generous).min(100))
                    && let Some(card) = self.book.roll_spoil(&mut self.rng, pack)
                {
                    self.drop_item(card, x + dx + 0.9, y + dy + 0.5, None);
                }
            }
        }
    }

    fn drop_item(&mut self, card: String, x: f32, y: f32, held_off: Option<u32>) {
        let most = if self.open_map() { depths::OPEN_ITEMS } else { MAX_ITEMS };
        let room = &mut self.dungeon.rooms[self.at];
        if room.items.len() < most {
            room.items.push(Item {
                card,
                x,
                y,
                held_off,
                took: Vec::new(),
            });
        }
    }

    /// Knights pick up the cards they walk over and open chests they touch.
    fn gather(&mut self) {
        let touched = self.dungeon.rooms[self.at]
            .chest
            .filter(|c| !c.open)
            .is_some_and(|chest| {
                self.players
                    .values()
                    .any(|h| h.hp > 0 && !h.stone && (h.x - chest.x).hypot(h.y - chest.y) < 1.8)
            });
        if touched {
            self.wake_mimic();
        }
        let room = &mut self.dungeon.rooms[self.at];
        if let Some(chest) = room.chest.as_mut().filter(|c| !c.open)
            && self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - chest.x).hypot(h.y - chest.y) < 1.8)
        {
            chest.open = true;
            self.sounds.push("chest_open");
            let (cx, cy) = (chest.x, chest.y);
            let pack = self.dungeon.pack;
            for (i, card) in self
                .book
                .roll_chest(&mut self.rng, pack)
                .into_iter()
                .enumerate()
            {
                let angle = std::f32::consts::FRAC_PI_2 + (i as f32 - 1.0) * 1.1;
                room.items.push(Item {
                    card,
                    x: cx + angle.cos() * 3.0,
                    y: cy + angle.sin() * 2.6,
                    held_off: None, took: Vec::new(),
                });
            }
        }
        let mut dropped = Vec::new();
        let mut nova = 0;
        let beaumains = self
            .hireling
            .as_ref()
            .is_some_and(|h| h.kind == hireling::AllyKind::Beaumains);
        // Shared loot: a card lies until every knight here has taken their
        // own (an absent host's seat, or a knight left as stone, waits for
        // no share).
        let party: Vec<u32> = self.players.iter().filter(|(_, h)| !h.absent && !h.stone).map(|(&id, _)| id).collect();
        for (&id, hero) in self
            .players
            .iter_mut()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
        {
            for item in &mut room.items {
                if item.held_off == Some(id)
                    && (hero.x - item.x).hypot(hero.y - item.y) > PICKUP_REACH + 0.6
                {
                    item.held_off = None;
                }
            }
            let book = &self.book;
            // A weapon is swapped to on the take key, never by walking over
            // it: the nearest one beside the knight, set down or not.
            let wanted = std::mem::take(&mut hero.taking)
                .then(|| {
                    room.items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| {
                            !item.took.contains(&id)
                                && book.get(&item.card).is_some_and(|c| {
                                    gear::manual(c) && hero.takes(c) && !school::locked(&self.home.school, hero, c)
                                })
                        })
                        .map(|(i, item)| (i, (hero.x - item.x).hypot(hero.y - item.y)))
                        .filter(|&(_, d)| d < TAKE_REACH)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(i, _)| i)
                })
                .flatten();
            // While Beaumains walks with the party a summoning card would
            // only clog a hand: it stays where it lies.
            let Some(index) = wanted.or_else(|| {
                room.items.iter().position(|item| {
                    item.held_off.is_none()
                        && !item.took.contains(&id)
                        && (hero.x - item.x).hypot(hero.y - item.y) < PICKUP_REACH
                        && book.get(&item.card).is_some_and(|card| {
                            !gear::manual(card)
                                && hero.takes(card)
                                // The school's locks: a card above the
                                // knight's rank stays where it lies, for
                                // someone who can.
                                && !school::locked(&self.home.school, hero, card)
                                && !(beaumains && card.summon().is_some())
                        })
                })
            }) else {
                continue;
            };
            // Shared loot: the knight takes their own; the card lies on for
            // the rest until every knight has (`loot::shared`).
            let lies_on = book.get(&room.items[index].card).is_some_and(loot::shared) && {
                room.items[index].took.push(id);
                !party.iter().all(|p| room.items[index].took.contains(p))
            };
            let item = if lies_on { room.items[index].clone() } else { room.items.swap_remove(index) };
            let Some(card) = book.get(&item.card) else {
                continue;
            };
            let mut took = "took";
            if gear::piece(&card.id) { hero.wear_piece(&card.id); }
            match card.kind {
                // Helm and mail are worn, not carried: their protection is the piece.
                cards::Kind::Hold if gear::piece(&card.id) => {}
                cards::Kind::Take => {
                    hero.spend(card, &mut self.score, &mut nova);
                    if card.id == "talisman" {
                        hero.talisman = true;
                        self.cues.push("talisman".into());
                    }
                }
                cards::Kind::Play => hero.hand.push(card.id.clone()),
                cards::Kind::Hold => {
                    hero.deck.push(card.id.clone());
                    hero.rebonus(book);
                    for effect in &card.effects {
                        if let cards::Effect::MaxHp(v) = *effect {
                            hero.max_hp += v;
                            hero.hp = (hero.hp + v).min(hero.max_hp);
                        }
                    }
                }
                // The arm already in hand: taking its twin tempers it, up to
                // the most; past that it stays where it lay (`feel`).
                cards::Kind::Arm if hero.tempers(card) => {
                    if hero.temper < feel::TEMPER_MAX {
                        hero.temper += 1;
                        took = "tempered";
                        self.cues.push(format!("temper:{}", hero.temper));
                        if hero.temper == feel::TEMPER_MAX {
                            self.school.note(id, "sword-temper", 1);
                        }
                    } else {
                        if !lies_on {
                            dropped.push((item.card.clone(), item.x, item.y, id));
                        }
                        continue;
                    }
                }
                cards::Kind::Arm => {
                    if let Some(old) = hero.arm.take() {
                        dropped.push((old, item.x, item.y, id));
                    }
                    hero.equip(card);
                }
                cards::Kind::Spell => {
                    hero.equip_spell(card);
                }
                cards::Kind::Guard => {
                    if let Some(old) = hero.guard_card.take() {
                        dropped.push((old, item.x, item.y, id));
                    }
                    hero.guard = card.guard.unwrap_or_default();
                    hero.guard_card = Some(card.id.clone());
                }
            }
            self.found = Some((self.tick, id, if gear::piece(&card.id) { format!("{}: worn", card.name) } else { format!("{took} {}", card.name) }));
            // A glint rises where it lay, brighter for a rarer find.
            let tier = match (card.kind, card.rarity) {
                (_, cards::Rarity::Relic) => 2,
                (cards::Kind::Spell, _) | (_, cards::Rarity::Rare) => 1,
                _ => 0,
            };
            self.feel.glint(item.x, item.y, tier);
            self.sounds.push(if card.is_spoil() {
                "spoil_pickup"
            } else {
                "card_pickup"
            });
            // Spoils are counted, not cheered; other finds are named.
            match (card.kind, card.rarity) {
                _ if card.is_spoil() => {}
                (cards::Kind::Arm, _) => self.cues.push(format!("card_arm:{}", card.id)),
                (cards::Kind::Guard, _) => self.cues.push(format!("card_guard:{}", card.id)),
                (_, cards::Rarity::Relic) => self.cues.push("card_relic".into()),
                (_, cards::Rarity::Rare) => self.cues.push(format!("card_rare:{}", card.id)),
                _ => {}
            }
        }
        for (card, x, y, id) in dropped {
            room.items.push(Item {
                card,
                x,
                y,
                held_off: Some(id), took: Vec::new(),
            });
        }
        if nova > 0 {
            let open = self.open_map();
            let knights: Vec<(f32, f32)> = self.players.values().filter(|h| h.hp > 0).map(|h| (h.x, h.y)).collect();
            for enemy in &mut self.enemies {
                if open && !knights.iter().any(|&(x, y)| (enemy.x - x).hypot(enemy.y - y) <= depths::OPEN_BLAST) {
                    continue;
                }
                enemy.hp = enemy.hp.saturating_sub(nova);
            }
        }
    }

    /// Through an open doorway into the next room, down the stairs, or
    /// into the light home.
    fn travel(&mut self) {
        if self.room().kind == RoomKind::Kingdom {
            return;
        }
        if self.step_into_gate() {
            return;
        }
        if let Some((lx, ly)) = self.light
            && self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - lx).hypot(h.y - ly) < 1.6)
        {
            // Keep the finite, validated exit marker as victory evidence for
            // saves and sequential mirrors. Won no longer runs travel.
            self.phase = Phase::Won;
            self.cues.push("homeward".into());
            self.sounds.push("descend");
            self.bank_all(true, "home with the dragon's hoard");
            return;
        }
        // Through a door off an open map into its side room.
        if self.open_doors() {
            return;
        }
        let mut exit = None;
        let mut exit_seat = None;
        let mut stairs_seat = None;
        let mut stairs = false;
        // At home, stairs that belong to an entrance go where it goes; only
        // the Undercroft's Winding Stair goes down into the delve.
        let mut entrance: Option<&'static world::Entrance> = None;
        let home = self.at_home_now();
        let room = self.room();
        // The fellowship: who counts, and who stands in which way out. A
        // doorway or the stairs move the party only when every knight who
        // counts stands in that same one (`fellowship`).
        let mut counted = 0;
        let mut in_door: BTreeMap<u32, usize> = BTreeMap::new();
        let mut on_stairs: BTreeMap<u32, Option<&'static str>> = BTreeMap::new();
        // An away knight standing in a doorway or on a stair takes the party
        // nowhere, and does not hold the stair for those who come after.
        for (&id, hero) in self
            .players
            .iter()
            .filter(|(id, h)| h.hp > 0 && !h.stone && !h.absent && !self.away(**id))
        {
            counted += 1;
            let dir = if hero.y < 0.6 {
                Some(0)
            } else if hero.x > room.width() - 0.6 {
                Some(1)
            } else if hero.y > room.height() - 0.6 {
                Some(2)
            } else if hero.x < 0.6 {
                Some(3)
            } else {
                None
            };
            if dir.is_some() {
                exit = dir;
                exit_seat = Some(id);
            }
            if let Some(dir) = dir {
                // Out of a side-on hall, any way out is its one doorway back.
                in_door.insert(id, room.way_back().unwrap_or(dir));
            }
            let col = (hero.x / TILE_UNITS) as i32;
            let row = (hero.y / TILE_UNITS) as i32;
            if room.tile(col, row) == Tile::Stairs {
                let gate = home
                    .then(|| world::entrance_at(room.kind, col, row))
                    .flatten();
                // A door the realm has not built yet is only a wall.
                if gate.is_some_and(|e| !world::open(e, &self.home)) {
                    continue;
                }
                // Home stairs that are no entrance are the Winding Stair's
                // only in the Undercroft itself.
                if gate.is_some() || !home || room.kind == RoomKind::Home {
                    stairs = true;
                    if entrance.is_none() {
                        stairs_seat = Some(id);
                    }
                    entrance = entrance.or(gate);
                    on_stairs.insert(id, gate.map(|e| e.id));
                }
            }
        }
        // A Colosseum arena's gate stays shut while its heat runs.
        if self.arena_heat_on() {
            stairs = false;
            entrance = None;
            exit = None;
        }
        if !stairs {
            self.stairs_held = false;
            self.descending = 0;
        }
        // Each entrance keeps its own hold: a knight stepping from one plate
        // onto the next begins again rather than taking the wrong one.
        let held_for = entrance.map(|e| e.id);
        if self.descend_for != held_for {
            self.descend_for = held_for;
            self.descending = 0;
        }
        // Out of a side-on hall, any way out is its one doorway back.
        let exit = exit.map(|dir| self.room().way_back().unwrap_or(dir));
        // A way out is open to the party when all of it stands there. A
        // retreat to the kingdom is a seat's own decision, and waits for no
        // one (`exits`).
        let at_stairs = on_stairs.values().filter(|&&g| g == held_for).count() == counted;
        let retreat = entrance.is_some_and(|e| self.returns_to_kingdom(e.id));
        let stairs_ready = at_stairs || retreat;
        // At home the Winding Stair is taken on purpose: a knight stands on
        // it a moment (its ring fills) rather than brushing past it. The ring
        // fills only while the party is on it.
        let deliberate = if self.at_home_now() && stairs && !self.stairs_held {
            if stairs_ready {
                self.descending += 1;
            } else {
                self.descending = 0;
            }
            self.descending >= home::DESCEND_HOLD
        } else {
            true
        };
        // An open floor's stairs wait for its guardian to fall.
        if stairs && self.open_guarded() {
            stairs = false;
        }
        if stairs && self.guardian_locked() {
            // Do not let a stair attempt swallow a simultaneous retreat.
            if self.tick.is_multiple_of(u64::from(HZ)) {
                self.show_boss_lock();
            }
            stairs = false;
        }
        if stairs && !self.stairs_held && !stairs_ready {
            self.fellowship.wait(on_stairs.keys().map(|&id| (id, fellowship::STAIRS)));
        }
        if stairs && !self.stairs_held && deliberate && stairs_ready {
            self.descending = 0;
            match entrance {
                Some(e) if self.returns_to_kingdom(e.id) => {
                    if let Some(id) = stairs_seat {
                        self.leave_activity(id);
                    }
                }
                Some(e) => self.take_entrance(e),
                None => self.descend(),
            }
        } else if exit.is_some() && self.kingdom.return_to.is_some() && self.room().kind.in_world()
        {
            if let Some(id) = exit_seat {
                self.leave_activity(id);
            }
        } else if let Some(dir) = exit
            && let Some(next) = self.dungeon.neighbour(self.at, dir)
        {
            // The boss door stays shut until the key is carried to it.
            if self.boss_door_holds(next, dir) {
                self.fellowship.clear();
                return;
            }
            let returning = exits::room(self.room().kind)
                && !self.room().kind.in_world()
                && self.activity_return().is_some_and(|back| back.at == next);
            if returning {
                if let Some(id) = exit_seat {
                    self.leave_activity(id);
                }
            } else if in_door.values().filter(|&&d| d == dir).count() == counted {
                self.enter(next, Some((dir + 2) % 4));
            } else {
                self.fellowship.wait(in_door.iter().map(|(&id, &d)| (id, d as u8)));
            }
        }
    }
}

/// The hostile shots past `from` in `projectiles`.
fn hostile_past(projectiles: &[Projectile], from: usize) -> u64 {
    projectiles[from.min(projectiles.len())..]
        .iter()
        .filter(|p| p.hostile)
        .count() as u64
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    /// Room `index` with its interior cleared of stone and pits: an open
    /// floor for fixtures that walk, aim or summon across it.
    pub fn open_room_for_test(&mut self, index: usize) {
        let room = &mut self.dungeon.rooms[index];
        for row in 1..room.rows - 1 {
            for col in 1..room.cols - 1 {
                room.set(col, row, Tile::Floor);
            }
        }
    }

    pub fn enter_for_test(&mut self, index: usize) {
        // Unrelated combat/wave/art fixtures explicitly exercise the retained
        // legacy encounter path. Policy tests call production `enter` instead.
        self.boss_gates = None;
        let builtin = bosses::builtin();
        self.bosses
            .retain(|b| builtin.iter().any(|n| n.id == b.id) || encounter_catalog::custom(b));
        self.enter(index, Some(2));
    }
    pub fn exhibit_pattern_for_test(
        &mut self,
        emitter: patterns::Emitter,
        kind: Shot,
        from: (f32, f32),
        to: (f32, f32),
    ) {
        let grid = Grid { room: self.room(), barred: false };
        let aim = patterns::Aim {
            from, at: to, pace: (8.0, 0.0), grid: &grid,
            count: 1, salt: 0x5eed, step: 1, kind, damage: 12, guardian: None,
        };
        let mut shots = Vec::new();
        patterns::emit(emitter, &aim, &mut shots);
        self.projectiles = shots;
    }

    pub fn spawn_at_for_test(&mut self, kind: EnemyKind, x: f32, y: f32) {
        // Unrelated combat/card/song fixtures explicitly use legacy actors,
        // just like enter_for_test. Ecology regressions call production paths.
        self.boss_gates = None;
        self.spawn_at(kind, x, y);
        if let Some(enemy) = self.enemies.last_mut() {
            enemy.age = TELEGRAPH;
        }
    }

    /// Every boss a floor of `pack` can raise (its guardians, its route
    /// chiefs and its apex), by index into `bosses`.
    pub fn pack_bosses_for_test(&mut self, pack: Pack) -> Vec<usize> {
        encounter_catalog::leaders(&mut self.bosses, pack, 0);
        if pack == Pack::Unknown {
            // The threshold's heart.
            encounter_catalog::apex(&mut self.bosses, pack, RoomKind::Threshold, &[]);
        }
        (0..self.bosses.len())
            .filter(|&i| self.bosses[i].only_in == pack)
            .collect()
    }

    /// Boss `index` stands at `(x, y)`, past its telegraph.
    pub fn spawn_boss_at_for_test(&mut self, index: usize, x: f32, y: f32) {
        self.spawn_boss(index);
        if let Some(enemy) = self.enemies.last_mut() {
            (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (x, y, x, y);
            enemy.age = TELEGRAPH;
        }
    }

    /// A ring of harmless shots thrown out from `(x, y)`, one of every kind
    /// (a gallery of how shots look in flight).
    pub fn volley_for_test(&mut self, x: f32, y: f32, turn: f32) {
        const KINDS: [Shot; 17] = [
            Shot::Blade, Shot::Arrow, Shot::Bolt, Shot::Ball, Shot::Bone, Shot::Orb, Shot::Ember,
            Shot::Sacred, Shot::Hex, Shot::Frost, Shot::Missile, Shot::Wisp, Shot::Ore, Shot::Flame,
            Shot::Page, Shot::Spore, Shot::Eye,
        ];
        for (i, kind) in KINDS.into_iter().enumerate() {
            let a = turn + i as f32 / KINDS.len() as f32 * std::f32::consts::TAU;
            let speed = 6.0 + (i % 3) as f32 * 3.0;
            self.projectiles.push(Projectile {
                x,
                y,
                vx: a.cos() * speed,
                vy: a.sin() * speed,
                hostile: false,
                kind,
                look: None,
                damage: 0,
                pierce: u8::MAX,
                last_hit: None,
                ttl: 3 * HZ,
                empowered: i % 5 == 0,
                traits: ShotTraits::default(),
            });
        }
    }

    pub fn clear_prerequisites_for_test(&mut self) {
        if let Some(g) = &mut self.boss_gates {
            for i in 0..g.leaders.len() {
                let room = g.leaders[i].room;
                for (slot, _, _) in g.economy.deploy(room).unwrap() {
                    g.economy.casualty(room, slot);
                }
                let w = &mut g.leaders[i];
                w.defeated = true;
                self.dungeon.rooms[w.room].visited = true;
                self.dungeon.rooms[w.room].cleared = true;
                g.economy.lose(i as u8);
            }
        }
    }

    pub fn descend_for_test(&mut self) {
        // Floor-setup helper, not a test of the production travel guard.
        self.clear_prerequisites_for_test();
        if let Some(g) = &mut self.boss_gates {
            g.guardian_defeated = true;
            self.dungeon.rooms[g.guardian].visited = true;
            self.dungeon.rooms[g.guardian].cleared = true;
        }
        self.descend();
        // This helper sets up a floor for unrelated combat/art tests.
        // Production transition tests call the actual travel/descend path.
        self.floor_plates.clear();
    }

    pub fn kit_out_for_test(&mut self) {
        self.kit_out();
    }

    /// No more waves, traps or falling rocks in this room.
    pub fn calm_for_test(&mut self) {
        self.waves = Waves::default();
        self.traps.clear();
        self.rocks.clear();
    }

    /// Advance one simulation tick, allowing an existing impact hold to end.
    /// Timing tests call `step` directly to count transport ticks instead.
    pub fn step_sim_for_test(&mut self, inputs: &BTreeMap<u32, Input>) {
        while self.hitstop.left > 0 { self.step(&BTreeMap::new()); }
        self.step(inputs);
    }

    pub fn clear_for_test(&mut self) {
        self.calm_for_test();
        // Some kinds do not go quietly: a geode splits, a slime divides.
        // Sweep until the room is actually empty, so "cleared" is true.
        for _ in 0..6 {
            for enemy in &mut self.enemies {
                enemy.hp = 0;
            }
            self.step_sim_for_test(&BTreeMap::new());
            if self.enemies.is_empty() {
                break;
            }
        }
        while self.hitstop.left > 0 { self.step(&BTreeMap::new()); }
    }

    pub fn lair_for_test(&mut self) {
        while self.dungeon.depth < FLOORS {
            self.descend_for_test();
        }
        let lair = self
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Lair)
            .expect("the last floor has a lair");
        self.clear_prerequisites_for_test();
        self.enter(lair, Some(2));
    }
}

#[cfg(test)]
#[path = "../tests/rules/together_shooter__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/rules/together_ledge__tests.rs"]
mod ledge_tests;

#[cfg(test)]
#[path = "../tests/rules/together_balance__tests.rs"]
mod balance_tests;

#[cfg(test)]
#[path = "../tests/rules/together_deep__tests.rs"]
mod deep_tests;

#[cfg(test)]
#[path = "../tests/rules/together_polish__tests.rs"]
mod polish_tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../tests/rules/together_polish__fixtures.rs"]
pub mod polish_testing;

#[cfg(test)]
#[path = "../tests/rules/together_balance_sim__tests.rs"]
pub(crate) mod balance_sim_tests;

#[cfg(test)]
#[path = "../tests/rules/together_stakes__tests.rs"]
mod stakes_tests;

#[cfg(test)]
#[path = "../tests/rules/together_boss_gates__tests.rs"]
mod boss_gate_tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../tests/rules/together_boss_gates__fixtures.rs"]
pub mod boss_gate_testing;

#[cfg(test)]
#[path = "../tests/rules/together_pacing__tests.rs"]
mod pacing_tests;

#[cfg(test)]
#[path = "../tests/rules/together_skills__tests.rs"]
mod skills_tests;
#[cfg(test)]
#[path = "../tests/rules/together_spectacle__tests.rs"]
mod spectacle_tests;

#[cfg(test)]
#[path = "../tests/rules/together_party_rooms__tests.rs"]
mod party_rooms_tests;

#[cfg(test)]
#[path = "../tests/rules/together_cinder__tests.rs"]
mod cinder_tests;

#[cfg(test)]
#[path = "../tests/rules/together_latch__tests.rs"]
mod latch_tests;

#[cfg(test)]
#[path = "../tests/rules/together_mossbell__tests.rs"]
mod mossbell_tests;

#[cfg(test)]
#[path = "../tests/rules/together_brasswake__tests.rs"]
mod brasswake_tests;
#[cfg(test)]
#[path = "../tests/rules/together_rafterwake__tests.rs"]
mod rafterwake_tests;
#[cfg(test)]
#[path = "../tests/rules/together_pennon__tests.rs"]
mod pennon_tests;

#[cfg(test)]
#[path = "../tests/rules/together_six_seats__tests.rs"]
mod six_seats_tests;
