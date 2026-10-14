//! Dangers of the rooms themselves, each a slow cycle you can read before it
//! bites, and each hurting monsters as gladly as knights, so a clever knight
//! can lure the company into them.
//!
//! - **Fire jets** hiss and spit sparks for a second, then roar with a
//!   column of flame, then gutter.
//! - **Gas seeps** bubble, then breathe out a cloud (the same bad air a
//!   knight's Gas throws) that drifts and thins. Flame set to it flashes it.
//! - **Ember cracks** glow along a seam of the floor, then burst into a
//!   line of fire.
//! - **Crumbling floors** shed dust and crack, then fall away under whoever
//!   stands on them and settle again as rubble.
//!
//! They scale with depth: the first two floors keep to the gentlest, a
//! room in a hundred is unlucky, and the deep adds fire and more of each.
//! Their places are drawn from the floor's own seed and shape, never from
//! the run's dice, so no other room is reshuffled by them.
//!
//! The numbers are the named constants at the top.

use super::skills::{Cloud, GAS_LIFE};
use super::*;

/// What the first floors risk: the share of rooms (percent) that hold a
/// peril, by depth (1, 2, 3 and below), and the most one room holds.
const ODDS: [u32; 3] = [35, 55, 75];
const MOST: [usize; 3] = [1, 1, 2];
/// How near a doorway a peril may lie, so no one walks into one.
const DOOR_CLEAR: f32 = 6.0;

/// Fire jet: the cycle, when it hisses, when it roars and when it gutters;
/// its reach; and what it does to a knight and to a monster each beat.
pub const JET_PERIOD: u32 = 150;
pub const JET_WARN: u32 = 90;
pub const JET_LIVE: u32 = 120;
pub const JET_END: u32 = 146;
pub const JET_REACH: f32 = 1.5;
const JET_EVERY: u64 = 5;
const JET_FOE_HARM: u32 = 16;
/// Gas seep: the cycle, when it bubbles, when it breathes; the cloud it
/// leaves (life, reach, poison a beat for monsters) and what the air does
/// to a knight a beat.
pub const SEEP_PERIOD: u32 = 210;
pub const SEEP_WARN: u32 = 150;
pub const SEEP_PUFF: u32 = 180;
const SEEP_LIFE: u32 = 5 * HZ;
const SEEP_POISON: u32 = 4;
const SEEP_CLOUDS: usize = 3;
/// Ember crack: the cycle, glow, burst and cool; the seam's half length and
/// half width.
pub const CRACK_PERIOD: u32 = 240;
pub const CRACK_WARN: u32 = 150;
pub const CRACK_LIVE: u32 = 200;
pub const CRACK_END: u32 = 224;
pub const CRACK_LONG: f32 = 3.2;
pub const CRACK_WIDE: f32 = 1.0;
const CRACK_EVERY: u64 = 6;
const CRACK_FOE_HARM: u32 = 20;
/// Crumbling floor: the cycle, when it cracks, when it falls, when it is
/// only rubble; the patch's half size; and what the fall does.
pub const CRUMBLE_PERIOD: u32 = 300;
pub const CRUMBLE_WARN: u32 = 190;
pub const CRUMBLE_FALL: u32 = 250;
pub const CRUMBLE_RUBBLE: u32 = 275;
pub const CRUMBLE_HALF: f32 = 2.0;
const CRUMBLE_HARM: u32 = 18;
const CRUMBLE_FOE_HARM: u32 = 40;
/// What the air and the flames do to a knight in one beat.
pub const GAS_HARM: u32 = 5;
pub const FIRE_HARM: u32 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Peril {
    /// A fire jet at `(x, y)`.
    Jet { x: f32, y: f32, phase: u32 },
    /// A gas seep.
    Seep { x: f32, y: f32, phase: u32 },
    /// A seam of ember in the floor, lying along x (`across`) or y.
    Crack {
        x: f32,
        y: f32,
        phase: u32,
        across: bool,
    },
    /// A patch of floor (four by four units) that gives way.
    Crumble { x: f32, y: f32, phase: u32 },
}

impl Peril {
    pub fn at(&self) -> (f32, f32) {
        match *self {
            Peril::Jet { x, y, .. }
            | Peril::Seep { x, y, .. }
            | Peril::Crack { x, y, .. }
            | Peril::Crumble { x, y, .. } => (x, y),
        }
    }

    fn phase(&self) -> u32 {
        match *self {
            Peril::Jet { phase, .. }
            | Peril::Seep { phase, .. }
            | Peril::Crack { phase, .. }
            | Peril::Crumble { phase, .. } => phase,
        }
    }

    fn period(&self) -> u32 {
        match self {
            Peril::Jet { .. } => JET_PERIOD,
            Peril::Seep { .. } => SEEP_PERIOD,
            Peril::Crack { .. } => CRACK_PERIOD,
            Peril::Crumble { .. } => CRUMBLE_PERIOD,
        }
    }

    /// Ticks into the cycle on this tick of the run.
    pub fn beat(&self, tick: u64) -> u32 {
        (tick as u32).wrapping_add(self.phase()) % self.period()
    }

    /// How far along its warning it is (0 to 1), once it has begun to warn
    /// and until it fires.
    pub fn warning(&self, tick: u64) -> Option<f32> {
        let beat = self.beat(tick);
        let (from, to) = match self {
            Peril::Jet { .. } => (JET_WARN, JET_LIVE),
            Peril::Seep { .. } => (SEEP_WARN, SEEP_PUFF),
            Peril::Crack { .. } => (CRACK_WARN, CRACK_LIVE),
            Peril::Crumble { .. } => (CRUMBLE_WARN, CRUMBLE_FALL),
        };
        (from..to)
            .contains(&beat)
            .then(|| (beat - from) as f32 / (to - from) as f32)
    }

    /// How far into its strike it is (0 to 1) while it burns or has fallen.
    pub fn striking(&self, tick: u64) -> Option<f32> {
        let beat = self.beat(tick);
        let (from, to) = match self {
            Peril::Jet { .. } => (JET_LIVE, JET_END),
            Peril::Seep { .. } => (SEEP_PUFF, SEEP_PUFF + 30),
            Peril::Crack { .. } => (CRACK_LIVE, CRACK_END),
            Peril::Crumble { .. } => (CRUMBLE_FALL, CRUMBLE_RUBBLE),
        };
        (from..to)
            .contains(&beat)
            .then(|| (beat - from) as f32 / (to - from) as f32)
    }

    /// Whether `(px, py)` stands where this peril strikes.
    fn strikes(&self, px: f32, py: f32, slack: f32) -> bool {
        let (x, y) = self.at();
        match *self {
            Peril::Jet { .. } => (px - x).hypot(py - y) <= JET_REACH + slack,
            Peril::Seep { .. } => false,
            Peril::Crack { across, .. } => {
                let (along, over) = if across {
                    ((px - x).abs(), (py - y).abs())
                } else {
                    ((py - y).abs(), (px - x).abs())
                };
                along <= CRACK_LONG + slack && over <= CRACK_WIDE + slack
            }
            Peril::Crumble { .. } => {
                (px - x).abs() <= CRUMBLE_HALF + slack && (py - y).abs() <= CRUMBLE_HALF + slack
            }
        }
    }
}

/// The weighted menu of a pack at a depth: what its rooms may hold.
fn menu(pack: Pack, depth: u32) -> Vec<(u8, u32)> {
    // 0 jet, 1 seep, 2 crack, 3 crumble
    match pack {
        Pack::Crypt => vec![(3, 4), (1, 3), (0, if depth >= 2 { 2 } else { 0 })],
        Pack::Cavern => vec![
            (1, 4),
            (3, if depth >= 2 { 3 } else { 0 }),
            (0, if depth >= 3 { 2 } else { 0 }),
        ],
        Pack::Hellforge => vec![(0, 4), (2, 3), (1, 2)],
        Pack::Archive => vec![(1, 4), (3, 3), (0, if depth >= 5 { 1 } else { 0 })],
        Pack::Fungal => vec![(1, 5), (3, 3), (2, if depth >= 5 { 1 } else { 0 })],
        Pack::Unknown => vec![(2, 3), (1, 2), (3, 2), (0, 2)],
    }
}

impl Run {
    /// A room's perils, laid out when the party walks in.
    pub(super) fn arm_perils(&mut self) {
        self.works.perils.clear();
        let depth = self.dungeon.depth;
        let room = self.room();
        if depth == 0
            || self.side_on()
            || !matches!(
                room.kind,
                RoomKind::Fight | RoomKind::Hall | RoomKind::Stairs | RoomKind::Lair
            )
            || self.mode == fortune::Mode::Gauntlet
        {
            return;
        }
        let tier = (depth.min(3) - 1) as usize;
        // Dedicated generator: nothing here draws from the run's dice.
        let mut rng = Rng::new(mix(self.seed
            ^ self.raid_id.rotate_left(13)
            ^ u64::from(depth) << 40
            ^ (self.at as u64) << 24
            ^ 0x9e11));
        // The boss rooms stay as they are on the first floors.
        let odds = match room.kind {
            RoomKind::Stairs | RoomKind::Lair if depth < 3 => 0,
            RoomKind::Hall => ODDS[tier] / 2,
            _ => ODDS[tier],
        };
        if !rng.chance(odds) {
            return;
        }
        let count =
            1 + if MOST[tier] > 1 && rng.chance(40) {
                1
            } else {
                0
            } + if depth >= 5 && rng.chance(35) { 1 } else { 0 };
        let menu = menu(self.dungeon.pack, depth);
        if menu.iter().all(|&(_, w)| w == 0) {
            return;
        }
        let doors: Vec<(f32, f32)> = (0..4)
            .filter(|&d| room.doors[d])
            .map(|d| hazards::doorway(room, d).0)
            .collect();
        let mut laid: Vec<Peril> = Vec::new();
        for _ in 0..count.min(3) {
            let kind = rng.pick(&menu.iter().filter(|m| m.1 > 0).copied().collect::<Vec<_>>());
            for _ in 0..40 {
                let col = 4 + rng.below(room.cols.saturating_sub(8).max(1)) as i32;
                let row = 4 + rng.below(room.rows.saturating_sub(8).max(1)) as i32;
                let across = rng.chance(50);
                // The floor it needs, and where its middle lies.
                let (cells, x, y): (Vec<(i32, i32)>, f32, f32) = match kind {
                    0 | 1 => (
                        (-1..=1)
                            .flat_map(|dr| (-1..=1).map(move |dc| (col + dc, row + dr)))
                            .collect(),
                        (col as f32 + 0.5) * TILE_UNITS,
                        (row as f32 + 0.5) * TILE_UNITS,
                    ),
                    2 => (
                        (-2..=2)
                            .map(|d| {
                                if across {
                                    (col + d, row)
                                } else {
                                    (col, row + d)
                                }
                            })
                            .collect(),
                        (col as f32 + 0.5) * TILE_UNITS,
                        (row as f32 + 0.5) * TILE_UNITS,
                    ),
                    _ => (
                        vec![
                            (col - 1, row - 1),
                            (col, row - 1),
                            (col - 1, row),
                            (col, row),
                        ],
                        col as f32 * TILE_UNITS,
                        row as f32 * TILE_UNITS,
                    ),
                };
                let clear = cells.iter().all(|&(c, r)| room.tile(c, r) == Tile::Floor);
                let far = doors
                    .iter()
                    .all(|&(dx, dy)| (dx - x).hypot(dy - y) > DOOR_CLEAR)
                    && laid
                        .iter()
                        .all(|p| (p.at().0 - x).hypot(p.at().1 - y) > 6.0);
                if !clear || !far {
                    continue;
                }
                let period = match kind {
                    0 => JET_PERIOD,
                    1 => SEEP_PERIOD,
                    2 => CRACK_PERIOD,
                    _ => CRUMBLE_PERIOD,
                };
                let phase = rng.below(period as usize) as u32;
                laid.push(match kind {
                    0 => Peril::Jet { x, y, phase },
                    1 => Peril::Seep { x, y, phase },
                    2 => Peril::Crack {
                        x,
                        y,
                        phase,
                        across,
                    },
                    _ => Peril::Crumble { x, y, phase },
                });
                break;
            }
        }
        self.works.perils = laid;
    }

    /// Jets roar, seeps breathe, seams burst and floors fall: whatever
    /// stands there is hurt, monster or knight.
    pub(super) fn tick_perils(&mut self) {
        let tick = self.tick;
        for peril in self.works.perils.clone() {
            let beat = peril.beat(tick);
            if let Peril::Seep { x, y, .. } = peril {
                let hostile = self.works.clouds.iter().filter(|c| c.hostile()).count();
                if beat == SEEP_PUFF && hostile < SEEP_CLOUDS {
                    self.works.clouds.push(Cloud {
                        x,
                        y,
                        vx: 0.0,
                        vy: -0.3,
                        age: GAS_LIFE - SEEP_LIFE,
                        owner: 0,
                        power: SEEP_POISON,
                        tint: 1,
                    });
                    self.sounds.push("vent_fire");
                }
                continue;
            }
            let (beats, every) = match peril {
                Peril::Jet { .. } => (peril.striking(tick).is_some(), JET_EVERY),
                Peril::Crack { .. } => (peril.striking(tick).is_some(), CRACK_EVERY),
                _ => (beat == CRUMBLE_FALL, 1),
            };
            if !beats || !tick.is_multiple_of(every) {
                continue;
            }
            if beat == JET_LIVE && matches!(peril, Peril::Jet { .. }) || beat == CRACK_LIVE {
                self.sounds.push("vent_fire");
            }
            let (px, py) = peril.at();
            if matches!(peril, Peril::Crumble { .. }) {
                self.sounds.push("rock_land");
                self.fx.push(Fx::Shake(5));
                self.boom(px, py, skills::BoomKind::Dust, CRUMBLE_HALF);
            } else if matches!(peril, Peril::Jet { .. }) && beat == JET_LIVE {
                self.boom(px, py, skills::BoomKind::Burst, JET_REACH);
            }
            let (knight, foe) = match peril {
                Peril::Jet { .. } => (FIRE_HARM, JET_FOE_HARM),
                Peril::Crack { .. } => (FIRE_HARM, CRACK_FOE_HARM),
                _ => (CRUMBLE_HARM, CRUMBLE_FOE_HARM),
            };
            let ground = matches!(peril, Peril::Crumble { .. });
            let hits: Vec<u32> = self
                .players
                .iter()
                .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
                .filter(|(_, h)| peril.strikes(h.x, h.y, HERO_RADIUS))
                .map(|(&id, _)| id)
                .collect();
            for id in hits {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.hurt(knight);
                }
            }
            for enemy in self
                .enemies
                .iter_mut()
                .filter(|e| e.hp > 0 && e.kind != EnemyKind::Dummy && !(ground && e.kind.flies()))
            {
                if peril.strikes(enemy.x, enemy.y, enemy.radius() * 0.5) {
                    enemy.hp = enemy.hp.saturating_sub(foe);
                }
            }
        }
    }
}
