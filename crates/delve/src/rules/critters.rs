//! The Delve's third company: eleven small things with one trick each, and
//! every trick shows itself before it lands.
//!
//! - **Biters** are a swarm of tiny things that circle, crouch (a shiver and
//!   a flash), then pounce in a straight line. Nothing about one is
//!   dangerous; four at once are a lesson in spacing.
//! - **Moles** dig under the floor, untouchable, trailing a mound. A crack
//!   ring opens under where a knight stood; it bursts a moment later. Move.
//! - **Pavises** carry a tower shield that turns every shot from the front.
//!   Its bash is wound up behind the shield; after it, the shield is down.
//! - **Leeches** rear, lunge and latch on, draining a knight to mend
//!   themselves. A roll or a spin shakes one off.
//! - **Marksmen** draw a long line across the floor, hold it still for the
//!   last moment, and loose one fast bolt down it. Leave the line.
//! - **Mirrorlings** watch what the knights shoot, then answer with a copy
//!   of the last one, after a flash.
//! - **Spitters** swell their throats and lob a glob at the spot ringed on
//!   the floor; it leaves a puddle that stings whoever stands in it.
//! - **Ringers** raise a bell, then ring it: every monster near is quickened
//!   for a few seconds. Silence the bell first.
//! - **Geodes** glow and burst a ring of shards, and when one falls it
//!   splits into two **Shards** that skitter.
//! - **Cinderhounds** crouch, mark a line with their gaze, rush down it
//!   dropping embers, and stand winded after.
//!
//! What they make comes out of each monster's step as `Doings`, for the run
//! to make so (`Run::apply_critters`).

use super::*;

/// Ticks a puddle stays, a glob takes to land.
pub const PUDDLE_STAYS: u32 = 6 * HZ;
pub const GLOB_FALL: u32 = 30;
pub const PUDDLE_REACH: f32 = 1.5;
/// How long a bell's quickening lasts, and how far its ring carries.
pub const RUNG_TICKS: u32 = 5 * HZ;
pub const BELL_REACH: f32 = 10.0;
/// A mole's crack ring, and how far the burst reaches.
pub const MOLE_TELL: u32 = 26;
const MOLE_REACH: f32 = 2.2;
/// A pavise's wind-up, and the arc its shield covers, in degrees.
pub const PAVISE_WINDUP: u32 = 22;
pub const PAVISE_ARC: f32 = 70.0;
/// A raised shield that has turned this share of its bearer's health in
/// shots, in percent, is knocked aside (the user, 2026-10-18: "shield guys
/// should have a damage threshold they drop their shield for a second").
pub const GUARD_BREAK: u32 = 34;
/// How long a knocked-aside shield stays down: a second.
pub const GUARD_DOWN: u32 = HZ;
/// How long a pavise pants with its shield low after a bash.
const PAVISE_PANT: u32 = 44;
/// A marksman's aim: how long, and when its line stops following.
pub const AIM_TICKS: u32 = 42;
pub const AIM_LOCK: u32 = 28;
/// A leech's rear, a mirrorling's flash, a spitter's swell, a ringer's
/// raise, a geode's glow, a hound's crouch and a biter's crouch.
pub const LEECH_REAR: u32 = 16;
pub const MIRROR_FLASH: u32 = 20;
pub const SPIT_SWELL: u32 = 22;
pub const BELL_RAISE: u32 = 26;
pub const GEODE_GLOW: u32 = 24;
pub const HOUND_CROUCH: u32 = 22;
pub const HOUND_LOCK: u32 = 14;
pub const BITER_CROUCH: u32 = 14;
/// A hound's rush, a biter's pounce, a leech's lunge: ticks.
const HOUND_RUSH: u32 = 22;
const BITER_POUNCE: u32 = 9;
const LEECH_LUNGE: u32 = 12;

impl EnemyKind {
    /// One of the third company.
    pub fn critter(self) -> bool {
        use EnemyKind::*;
        matches!(
            self,
            Biter
                | Shard
                | Mole
                | Pavise
                | Leech
                | Marksman
                | Mirrorling
                | Spitter
                | Ringer
                | Geode
                | Hound
        )
    }
}

impl Enemy {
    /// A mole under the floor: nothing shoots it, nothing touches it.
    pub fn buried(&self) -> bool {
        (self.kind == EnemyKind::Mole && matches!(self.stage, 1 | 2)) || self.sunk()
    }

    /// What its plain touch does: twelve, but the swarm's little ones less.
    pub fn touch(&self) -> u32 {
        match self.kind {
            EnemyKind::Biter => 5,
            EnemyKind::Shard => 7,
            _ => 12,
        }
    }

    /// A leech riding a knight.
    pub fn latched(&self) -> bool {
        self.kind == EnemyKind::Leech && self.stage == 3
    }

    /// Whether the plain touch of this body hurts: not a buried mole, not a
    /// leech already on a knight (it drains instead).
    pub fn hurts_by_touch(&self) -> bool {
        !self.buried()
            && !self.latched()
            // A charge-up's dash strikes with its sweep, once a knight.
            && !(self.signature.blow != 0 && self.stage == signatures::DASH)
            && !(self.signature.active
                && matches!(
                    self.stage,
                    signatures::TELL | signatures::JUMP | signatures::DAZED | signatures::WIND
                ))
    }

    /// The pavise's shield is up, and which way it faces.
    pub fn shield_up(&self) -> Option<(f32, f32)> {
        (self.kind == EnemyKind::Pavise && self.stage < 3).then_some(self.dir)
    }
}

/// A shot of `damage` turned by `enemy`'s raised shield: it counts toward a
/// guard break. Returns whether this one knocked the shield aside, which
/// leaves the bearer shield-low for `GUARD_DOWN`, as after its own bash.
pub(super) fn guard_struck(enemy: &mut Enemy, damage: u32) -> bool {
    enemy.guard = enemy.guard.saturating_add(damage.max(1));
    if enemy.guard * 100 < enemy.max_hp.max(1) * GUARD_BREAK {
        return false;
    }
    enemy.guard = 0;
    enemy.stage = 3;
    enemy.timer = PAVISE_PANT.saturating_sub(GUARD_DOWN);
    true
}

/// Whether a knight's shot, flying `(vx, vy)`, meets this monster's raised
/// shield on its face and is turned.
pub(super) fn blocks(enemy: &Enemy, vx: f32, vy: f32) -> bool {
    let Some((fx, fy)) = enemy.shield_up() else {
        return false;
    };
    let speed = vx.hypot(vy).max(0.001);
    // The shot comes against the way the shield faces.
    -(vx * fx + vy * fy) / speed >= PAVISE_ARC.to_radians().cos()
}

/// A quickened monster takes half again its stride, for as long as a bell's
/// ring lasts.
pub(super) fn quicken(enemy: &mut Enemy, from: (f32, f32), grid: &Grid) {
    if enemy.rung == 0 {
        return;
    }
    enemy.rung -= 1;
    if enemy.boss.is_some()
        || matches!(
            enemy.kind,
            EnemyKind::Mole | EnemyKind::Leech | EnemyKind::Ward | EnemyKind::Dummy
        )
    {
        return;
    }
    let (dx, dy) = (enemy.x - from.0, enemy.y - from.1);
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    let mover = if enemy.kind.flies() {
        Mover::Flier
    } else {
        Mover::Walker
    };
    (enemy.x, enemy.y) = grid.slide(
        (enemy.x, enemy.y),
        (dx * 0.5, dy * 0.5),
        enemy.radius(),
        mover,
    );
}

/// A glob in the air: from where it was spat to where it will land.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glob {
    pub from: (f32, f32),
    pub x: f32,
    pub y: f32,
    pub fall: u32,
}

/// A puddle a glob left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Puddle {
    pub x: f32,
    pub y: f32,
    pub left: u32,
}

/// A short-lived picture of something that just happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fx {
    /// A mole breaking the floor.
    Erupt,
    /// A shield bash.
    Bash,
    /// A bell's ring spreading.
    Ring,
    /// A glob landing.
    Splat,
    /// A geode cracking apart.
    Crack,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flash {
    pub fx: Fx,
    pub x: f32,
    pub y: f32,
    pub dir: (f32, f32),
    pub left: u8,
}

pub const FLASH_TICKS: u8 = 14;

/// The run's third-company state: globs and puddles, and the flashes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Field {
    pub globs: Vec<Glob>,
    pub puddles: Vec<Puddle>,
    pub flashes: Vec<Flash>,
}

impl Field {
    pub(super) fn clear(&mut self) {
        *self = Field::default();
    }

    fn flash(&mut self, fx: Fx, x: f32, y: f32, dir: (f32, f32)) {
        if self.flashes.len() < 24 {
            self.flashes.push(Flash {
                fx,
                x,
                y,
                dir,
                left: FLASH_TICKS,
            });
        }
    }
}

/// What the third company did this tick.
#[derive(Default)]
pub(super) struct Doings {
    /// Moles breaking the floor, where.
    erupts: Vec<(f32, f32)>,
    /// Shield bashes: from, its way.
    bashes: Vec<((f32, f32), (f32, f32))>,
    /// Globs spat: from, to.
    globs: Vec<((f32, f32), (f32, f32))>,
    /// Bells rung: where, by whom.
    rings: Vec<(f32, f32, u32)>,
    /// Hounds that hit someone on a rush: where, reach, way.
    rams: Vec<(f32, f32, f32, (f32, f32))>,
}

/// One of the third company's steps. False for any other monster.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) -> bool {
    use EnemyKind::*;
    if !enemy.kind.critter() {
        return false;
    }
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let (tx, ty) = target.unwrap_or((enemy.x, enemy.y + 1.0));
    let (cx, cy) = unit(tx - enemy.x, ty - enemy.y);
    let dist = (tx - enemy.x).hypot(ty - enemy.y);
    let seen = target.is_some();
    let mover = if enemy.kind.flies() {
        Mover::Flier
    } else {
        Mover::Walker
    };
    let r = enemy.radius();
    let walk = |enemy: &mut Enemy, (dx, dy): (f32, f32), speed: f32| {
        let (x, y) = grid.slide(
            (enemy.x, enemy.y),
            (dx * speed * DT, dy * speed * DT),
            r,
            mover,
        );
        let moved = (x - enemy.x).hypot(y - enemy.y);
        (enemy.x, enemy.y) = (x, y);
        moved
    };
    // Keep between `near` and `far` of the nearest knight, sidling between.
    let keep_off = |enemy: &mut Enemy, near: f32, far: f32, speed: f32| {
        let side = if enemy.id.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        let way = if !seen {
            (0.0, 0.0)
        } else if dist < near {
            (-cx, -cy)
        } else if dist > far {
            (cx, cy)
        } else {
            (-cy * side, cx * side)
        };
        walk(enemy, way, speed);
    };
    let fire = |shots: &mut Vec<Projectile>,
                x: f32,
                y: f32,
                angle: f32,
                speed: f32,
                kind: Shot,
                damage: u32,
                ttl: u32| {
        shots.push(Projectile {
            x,
            y,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed,
            hostile: true,
            look: None,
            kind,
            damage,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: Default::default(),
            ttl,
        });
    };
    enemy.timer += 1;
    let beat = enemy.timer + enemy.id * 7;
    match enemy.kind {
        // The swarm: circle close, shiver, pounce in a line.
        Biter | Shard => {
            let (circle, speed, pounce) = if enemy.kind == Biter {
                (3.6, 5.2, 17.0)
            } else {
                (4.5, 7.2, 19.0)
            };
            match enemy.stage {
                0 => {
                    let wobble = ((enemy.age as f32) * 0.25 + enemy.id as f32).sin() * 0.9;
                    let way = if dist > circle + 1.0 {
                        (cx - cy * wobble, cy + cx * wobble)
                    } else {
                        let side = if enemy.id.is_multiple_of(2) {
                            1.0
                        } else {
                            -1.0
                        };
                        (-cy * side + cx * 0.2, cx * side + cy * 0.2)
                    };
                    let (ux, uy) = unit(way.0, way.1);
                    if seen {
                        walk(enemy, (ux, uy), speed);
                    }
                    if enemy.timer >= 60 + (enemy.id % 7) * 8 && seen && dist < 10.0 {
                        (enemy.stage, enemy.timer) = (1, 0);
                    }
                }
                1 => {
                    if enemy.timer < BITER_CROUCH - 2 {
                        enemy.dir = (cx, cy);
                    }
                    if enemy.timer >= BITER_CROUCH {
                        (enemy.stage, enemy.timer) = (2, 0);
                    }
                }
                _ => {
                    let moved = walk(enemy, enemy.dir, pounce);
                    if enemy.timer >= BITER_POUNCE || moved < pounce * DT * 0.3 {
                        (enemy.stage, enemy.timer) = (0, 0);
                    }
                }
            }
        }
        // Digs, cracks the floor under where you were, bursts.
        Mole => match enemy.stage {
            // Surfaced and blinking in the light.
            0 => {
                if enemy.timer >= 44 && seen {
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            }
            // Under the floor, travelling.
            1 => {
                walk(enemy, (cx, cy), 7.0);
                if enemy.timer >= 36 || (seen && dist < 1.2) {
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            // The crack ring opens where it stopped.
            _ => {
                if enemy.timer >= MOLE_TELL {
                    deeds.crit.erupts.push((enemy.x, enemy.y));
                    deeds.cues.push("mole_erupt");
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // Faces the knight behind a tower shield; winds up, bashes, drops it.
        Pavise => match enemy.stage {
            0 => {
                if seen {
                    enemy.dir = (cx, cy);
                    walk(enemy, (cx, cy), 2.3);
                }
                if enemy.timer >= 45 && seen && dist < 3.4 {
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            }
            1 => {
                if enemy.timer >= PAVISE_WINDUP {
                    (enemy.stage, enemy.timer) = (2, 0);
                    deeds.crit.bashes.push(((enemy.x, enemy.y), enemy.dir));
                }
            }
            2 => {
                walk(enemy, enemy.dir, 9.0);
                if enemy.timer >= 6 {
                    (enemy.stage, enemy.timer) = (3, 0);
                }
            }
            // Shield low, panting: the moment to hit it from the front.
            _ => {
                enemy.guard = 0;
                if enemy.timer >= PAVISE_PANT {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // Crawls, rears, lunges, clings.
        Leech => match enemy.stage {
            0 => {
                if seen {
                    walk(enemy, (cx, cy), 3.2);
                }
                if enemy.timer >= 40 && seen && dist < 5.0 {
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            }
            1 => {
                if enemy.timer < LEECH_REAR - 3 {
                    enemy.dir = (cx, cy);
                }
                if enemy.timer >= LEECH_REAR {
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            2 => {
                walk(enemy, enemy.dir, 15.0);
                if seen && dist < r + HERO_RADIUS + 0.2 {
                    (enemy.stage, enemy.timer) = (3, 0);
                } else if enemy.timer >= LEECH_LUNGE {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
            // Riding a knight; the run drains and shakes it (`tick_leeches`).
            3 => {
                if seen {
                    enemy.x = tx + 0.15;
                    enemy.y = ty - 0.25;
                } else {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
            // Flung off, stunned.
            _ => {
                if enemy.timer >= 40 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // Draws a long line, holds it still, looses one bolt.
        Marksman => match enemy.stage {
            0 => {
                // No line to a knight: close in until there is one.
                let blind = seen && !clear_line(grid, (enemy.x, enemy.y), (tx, ty));
                if blind {
                    walk(enemy, (cx, cy), 2.4);
                } else {
                    keep_off(enemy, 10.0, 16.0, 2.0);
                }
                if enemy.timer >= 80 + (enemy.id % 5) * 8
                    && seen
                    && dist < 24.0
                    && clear_line(grid, (enemy.x, enemy.y), (tx, ty))
                {
                    (enemy.stage, enemy.timer) = (1, 0);
                    enemy.dir = (cx, cy);
                }
            }
            1 => {
                if enemy.timer < AIM_LOCK {
                    enemy.dir = (cx, cy);
                }
                if enemy.timer >= AIM_TICKS {
                    let (dx, dy) = enemy.dir;
                    fire(
                        shots,
                        enemy.x,
                        enemy.y,
                        dy.atan2(dx),
                        30.0,
                        Shot::Bolt,
                        26,
                        3 * HZ,
                    );
                    deeds.cues.push("marksman_shot");
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            _ => {
                if enemy.timer >= 22 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // Watches the knights' shots; after a flash, hands the last one back.
        Mirrorling => {
            // Low three bits: which shot it holds (0 none); bit 3: casting.
            let held = enemy.stage & 7;
            let casting = enemy.stage & 8 != 0;
            if !casting {
                keep_off(enemy, 7.0, 11.0, 3.0);
                // The newest knight's shot this tick, if any, is its copy.
                if let Some(seen_shot) = shots.iter().rev().find(|s| {
                    !s.hostile && s.ttl > 0 && s.traits.back == 0 && s.kind != Shot::Sacred
                }) {
                    let code = match seen_shot.kind {
                        Shot::Bolt => 2,
                        Shot::Ball => 3,
                        _ => 1,
                    };
                    enemy.stage = code;
                    enemy.dir = (seen_shot.vx.hypot(seen_shot.vy), seen_shot.damage as f32);
                } else if held != 0 && enemy.timer >= 100 && seen && dist < 20.0 {
                    enemy.stage |= 8;
                    enemy.timer = 0;
                }
            } else {
                if enemy.timer >= MIRROR_FLASH {
                    let kind = match held {
                        2 => Shot::Bolt,
                        3 => Shot::Ball,
                        _ => Shot::Arrow,
                    };
                    let speed = enemy.dir.0.clamp(8.0, 20.0);
                    let damage = (enemy.dir.1 as u32 / 2).clamp(8, 18);
                    let aim = (ty - enemy.y).atan2(tx - enemy.x);
                    fire(shots, enemy.x, enemy.y, aim, speed, kind, damage, 4 * HZ);
                    deeds.cues.push("mirror_copy");
                    enemy.stage = 0;
                    enemy.timer = 0;
                }
            }
        }
        // Swells its throat; spits where you stand; the spot is ringed.
        Spitter => match enemy.stage {
            0 => {
                keep_off(enemy, 6.5, 10.0, 2.4);
                if enemy.timer >= 72 + (enemy.id % 5) * 7 && seen && dist < 17.0 {
                    (enemy.stage, enemy.timer) = (1, 0);
                    // The spot it means, kept in `dir`.
                    enemy.dir = (tx, ty);
                }
            }
            1 => {
                if enemy.timer >= SPIT_SWELL {
                    deeds.crit.globs.push(((enemy.x, enemy.y), enemy.dir));
                    deeds.cues.push("spitter_spit");
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            _ => {
                if enemy.timer >= 18 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // Raises the bell; rings it; every monster near is quickened.
        Ringer => match enemy.stage {
            0 => {
                keep_off(enemy, 9.0, 13.0, 2.2);
                if enemy.timer >= 100 + (enemy.id % 4) * 9 && seen {
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            }
            1 => {
                if enemy.timer >= BELL_RAISE {
                    deeds.crit.rings.push((enemy.x, enemy.y, enemy.id));
                    deeds.cues.push("bell_rung");
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            _ => {
                if enemy.timer >= 22 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // Trundles; glows; throws a ring of shards.
        Geode => match enemy.stage {
            0 => {
                if seen {
                    walk(enemy, (cx, cy), 1.6);
                }
                if enemy.timer >= 96 + (enemy.id % 5) * 8 && seen && dist < 15.0 {
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            }
            1 => {
                if enemy.timer >= GEODE_GLOW {
                    let turn = (beat % 12) as f32 * 0.1;
                    for i in 0..8 {
                        let a = turn + i as f32 * std::f32::consts::TAU / 8.0;
                        fire(shots, enemy.x, enemy.y, a, 6.5, Shot::Orb, 11, 5 * HZ);
                    }
                    deeds.cues.push("geode_burst");
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            _ => {
                if enemy.timer >= 24 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // Circles; crouches and fixes its gaze; rushes, dropping embers.
        Hound => match enemy.stage {
            0 => {
                let side = if enemy.id.is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                let way = if !seen {
                    (0.0, 0.0)
                } else if dist > 9.0 {
                    (cx, cy)
                } else if dist < 5.0 {
                    (-cx, -cy)
                } else {
                    (-cy * side, cx * side)
                };
                walk(enemy, way, 5.2);
                if enemy.timer >= 66 + (enemy.id % 5) * 6 && seen && dist < 17.0 {
                    (enemy.stage, enemy.timer) = (1, 0);
                    enemy.dir = (cx, cy);
                }
            }
            1 => {
                if enemy.timer < HOUND_LOCK {
                    enemy.dir = (cx, cy);
                }
                if enemy.timer >= HOUND_CROUCH {
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            2 => {
                let moved = walk(enemy, enemy.dir, 22.0);
                deeds.crit.rams.push((enemy.x, enemy.y, r + 0.7, enemy.dir));
                if enemy.timer.is_multiple_of(4) {
                    fire(shots, enemy.x, enemy.y, 0.0, 0.0, Shot::Ember, 8, 50);
                }
                if enemy.timer >= HOUND_RUSH || moved < 22.0 * DT * 0.3 {
                    (enemy.stage, enemy.timer) = (3, 0);
                }
            }
            _ => {
                if enemy.timer >= 38 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        _ => {}
    }
    true
}

/// Whether nothing solid lies between two points.
fn clear_line(grid: &Grid, a: (f32, f32), b: (f32, f32)) -> bool {
    let steps = ((b.0 - a.0).hypot(b.1 - a.1) / 0.5).ceil().max(1.0) as i32;
    (1..steps).all(|k| {
        let t = k as f32 / steps as f32;
        !grid.stops_body(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, super::patterns::BODY)
    })
}

impl Run {
    /// Make what the third company did this tick so.
    pub(super) fn apply_critters(&mut self, doings: Doings) {
        for (x, y) in doings.erupts {
            let room = &self.dungeon.rooms[self.at];
            let grid = Grid { room, barred: true };
            for hero in self.players.values_mut().filter(|h| h.targetable()) {
                if (hero.x - x).hypot(hero.y - y) < MOLE_REACH {
                    hero.hurt(22);
                    let (ux, uy) = unit(hero.x - x, hero.y - y);
                    let (ux, uy) = if ux == 0.0 && uy == 0.0 {
                        (0.0, 1.0)
                    } else {
                        (ux, uy)
                    };
                    (hero.x, hero.y) = grid.slide(
                        (hero.x, hero.y),
                        (ux * 2.0, uy * 2.0),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                    self.sounds.push("hero_hurt");
                }
            }
            self.critters.flash(Fx::Erupt, x, y, (0.0, 0.0));
            self.sounds.push("rock_land");
            self.fx.push(super::fx::Fx::Shake(5));
        }
        for ((x, y), (dx, dy)) in doings.bashes {
            let room = &self.dungeon.rooms[self.at];
            let grid = Grid { room, barred: true };
            let mut held = Vec::new();
            for (&id, hero) in self.players.iter_mut().filter(|(_, h)| h.targetable()) {
                let (rx, ry) = (hero.x - x, hero.y - y);
                let d = rx.hypot(ry);
                if d < 2.8 && (rx * dx + ry * dy) / d.max(0.001) > 0.3 {
                    // A raised shield facing the bash takes it: steel on
                    // steel at its face (`shield`).
                    if super::shield::turns(hero, (dx, dy)) {
                        let at = super::shield::face(hero, (hero.x, hero.y), (x, y));
                        held.push((id, at, (at.0 - hero.x, at.1 - hero.y)));
                    } else {
                        hero.hurt(20);
                        (hero.x, hero.y) = grid.slide(
                            (hero.x, hero.y),
                            (dx * 2.6, dy * 2.6),
                            HERO_RADIUS,
                            Mover::Hero,
                        );
                        self.sounds.push("hero_hurt");
                    }
                }
            }
            for (id, at, out) in held {
                self.shield_block(id, at, out, (dx, dy), Shot::Blade, 3);
            }
            self.critters.flash(Fx::Bash, x, y, (dx, dy));
            self.sounds.push("shield_block");
            self.fx.push(super::fx::Fx::Shake(4));
        }
        for (from, (x, y)) in doings.globs {
            if self.critters.globs.len() < 8 {
                self.critters.globs.push(Glob {
                    from,
                    x,
                    y,
                    fall: GLOB_FALL,
                });
            }
        }
        for (x, y, who) in doings.rings {
            for other in self
                .enemies
                .iter_mut()
                .filter(|e| e.hp > 0 && e.id != who && e.boss.is_none())
                .filter(|e| !matches!(e.kind, EnemyKind::Dummy | EnemyKind::Ward))
            {
                if (other.x - x).hypot(other.y - y) < BELL_REACH {
                    other.rung = RUNG_TICKS;
                }
            }
            self.critters.flash(Fx::Ring, x, y, (0.0, 0.0));
            self.sounds.push("wave_gate");
        }
        for (x, y, reach, (dx, dy)) in doings.rams {
            let room = &self.dungeon.rooms[self.at];
            let grid = Grid { room, barred: true };
            for hero in self.players.values_mut().filter(|h| h.targetable()) {
                if hero.invulnerable == 0 && (hero.x - x).hypot(hero.y - y) < reach {
                    hero.hurt(18);
                    let (sx, sy) = (-dy, dx);
                    let side = if (hero.x - x) * sx + (hero.y - y) * sy >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    (hero.x, hero.y) = grid.slide(
                        (hero.x, hero.y),
                        ((dx + sx * side) * 1.4, (dy + sy * side) * 1.4),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                    self.sounds.push("hero_hurt");
                    self.fx.push(super::fx::Fx::Shake(5));
                }
            }
        }
    }

    /// Globs fall and leave puddles; puddles sting; flashes fade; leeches
    /// drain the knights they ride, and let go of one who rolls or spins.
    pub(super) fn tick_critters(&mut self) {
        let mut landed = Vec::new();
        for glob in &mut self.critters.globs {
            glob.fall = glob.fall.saturating_sub(1);
            if glob.fall == 0 {
                landed.push((glob.x, glob.y));
            }
        }
        self.critters.globs.retain(|g| g.fall > 0);
        for (x, y) in landed {
            for hero in self.players.values_mut().filter(|h| h.targetable()) {
                if (hero.x - x).hypot(hero.y - y) < PUDDLE_REACH {
                    hero.hurt(10);
                }
            }
            if self.critters.puddles.len() < 10 {
                self.critters.puddles.push(Puddle {
                    x,
                    y,
                    left: PUDDLE_STAYS,
                });
            }
            self.critters.flash(Fx::Splat, x, y, (0.0, 0.0));
            self.sounds.push("spike");
        }
        for puddle in &mut self.critters.puddles {
            puddle.left = puddle.left.saturating_sub(1);
        }
        self.critters.puddles.retain(|p| p.left > 0);
        for puddle in &self.critters.puddles {
            for hero in self.players.values_mut().filter(|h| h.targetable()) {
                if (hero.x - puddle.x).hypot(hero.y - puddle.y) < PUDDLE_REACH
                    && hero.rolling().is_none()
                {
                    hero.hurt(5);
                }
            }
        }
        for flash in &mut self.critters.flashes {
            flash.left = flash.left.saturating_sub(1);
        }
        self.critters.flashes.retain(|f| f.left > 0);
        // Leeches.
        let standing: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.targetable())
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let barred = self.barred();
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred,
        };
        for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0 && e.latched() && !e.asleep(self.tick)) {
            let Some(&(id, ..)) = standing.iter().min_by(|a, b| {
                (a.1 - enemy.x)
                    .hypot(a.2 - enemy.y)
                    .total_cmp(&(b.1 - enemy.x).hypot(b.2 - enemy.y))
            }) else {
                continue;
            };
            let Some(hero) = self.players.get_mut(&id) else {
                continue;
            };
            if hero.rolling().is_some() || hero.spin > 0 {
                // Shaken off, and thrown a little way.
                enemy.stage = 4;
                enemy.timer = 0;
                // Thrown clear of the knight, but never into a wall: the
                // full throw if it lands open, half if not, else it drops.
                let away = if hero.aim_x < 0.0 { 1.8 } else { -1.8 };
                if let Some(dx) = [away, away / 2.0]
                    .into_iter()
                    .find(|dx| grid.clear(enemy.x + dx, enemy.y, enemy.radius(), Mover::Walker))
                {
                    enemy.x += dx;
                }
                self.sounds.push("roll");
            } else if enemy.timer > 0 && enemy.timer.is_multiple_of(12) && hero.invulnerable == 0 {
                let before = hero.hp;
                hero.hp = hero.hp.saturating_sub(3).max(1);
                let took = before - hero.hp;
                enemy.hp = (enemy.hp + took * 2).min(enemy.max_hp);
                if took > 0 {
                    self.sounds.push("hero_hurt");
                }
            }
        }
    }

    /// Swarms come in fours: each biter a room started with brings three
    /// more.
    pub(super) fn swarm_up(&mut self) {
        let biters: Vec<(f32, f32)> = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Biter && e.stage == 0)
            .map(|e| (e.x, e.y))
            .collect();
        for (x, y) in biters {
            for (dx, dy) in [(1.0, 0.4), (-1.0, 0.6), (0.2, -1.0)] {
                if self.enemies.len() >= 24 {
                    return;
                }
                let room = &self.dungeon.rooms[self.at];
                let (px, py) = (
                    (x + dx).clamp(2.5, room.width() - 2.5),
                    (y + dy).clamp(2.5, room.height() - 2.5),
                );
                if room.tile((px / TILE_UNITS) as i32, (py / TILE_UNITS) as i32).ground()
                    && self.spawn_at(EnemyKind::Biter, px, py).is_some()
                    && let Some(enemy) = self.enemies.last_mut()
                {
                    // A brood is already a swarm.
                    enemy.stage = 0;
                }
            }
        }
    }

    /// A geode that falls splits in two: shards, skittering at once.
    pub(super) fn geode_splits(&mut self, x: f32, y: f32) {
        self.critters.flash(Fx::Crack, x, y, (0.0, 0.0));
        for side in [-1.0f32, 1.0] {
            if self
                .spawn_staged(EnemyKind::Shard, x + side * 0.9, y, 0)
                .is_some()
                && let Some(enemy) = self.enemies.last_mut()
            {
                enemy.age = TELEGRAPH;
            }
        }
        self.cues.push("geode_split".into());
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_critters__tests.rs"]
mod tests;
