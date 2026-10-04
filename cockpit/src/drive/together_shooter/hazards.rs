//! Monster doors and traps.
//!
//! **Waves.** A fight room holds back more than it shows: one wave a floor
//! deep pours in through the doorways, each announced by its doorway glowing
//! for a second. The room stays sealed until the last wave is down. A thinned
//! room calls the next wave early, so the pressure never sags.
//!
//! **Traps,** one kind per delve, all readable before they hurt:
//! - the Crypt's spike plates rattle, then strike knights and walking
//!   monsters alike (a dash passes through);
//! - the Mines' loose ceiling sheds rocks: a shadow grows where one will land,
//!   and a landing rock hurts monsters too, so they can be lured under it;
//! - Dragon Keep's wall vents glow, then breathe a line of fire across the room.
//!
//! **Mimics.** Some treasure chests have teeth.

use super::*;

/// Floor-spike rhythm: down, rattling, up.
pub(crate) const SPIKE_PERIOD: u32 = 96;
const SPIKE_RATTLE: u32 = 54;
const SPIKE_UP: u32 = 72;
/// Vents glow for the last second of each breath cycle, then fire.
pub(crate) const VENT_PERIOD: u32 = 150;
pub(crate) const VENT_GLOW: u32 = 30;
/// Ticks between a rock's shadow appearing and its landing.
pub(crate) const ROCK_FALL: u32 = 40;
const ROCK_DUST: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Spikes {
    Down,
    Rattle,
    Up,
}

pub(crate) fn spikes(tick: u64, phase: u32) -> Spikes {
    match (tick as u32 + phase) % SPIKE_PERIOD {
        t if t >= SPIKE_UP => Spikes::Up,
        t if t >= SPIKE_RATTLE => Spikes::Rattle,
        _ => Spikes::Down,
    }
}

/// Ticks into a vent's breath cycle; the last `VENT_GLOW` glow, 0..18 fire.
pub(crate) fn vent_beat(tick: u64, phase: u32) -> u32 {
    (tick as u32 + phase) % VENT_PERIOD
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Trap {
    /// A 2x2-tile plate of floor spikes from tile `(col, row)`.
    Spikes { col: i32, row: i32, phase: u32 },
    /// A wall vent on `side` (north, east, south, west) at `lane` (arena
    /// units along that wall), breathing fire straight across.
    Vent { side: usize, lane: f32, phase: u32 },
    /// The ceiling sheds rocks near the knights.
    Rockfall,
}

/// A rock on its way down (`fall` ticks left), or its dust settling.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Rock {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) fall: u32,
    pub(crate) dust: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Waves {
    pub(crate) left: u32,
    pub(crate) total: u32,
    timer: u32,
    /// The doorway glowing before a wave comes through, and ticks left.
    pub(crate) gate: Option<(usize, u32)>,
    turn: usize,
}

impl Waves {
    pub(crate) fn pending(&self) -> bool {
        self.left > 0 || self.gate.is_some()
    }
}

/// Where a wave stands as it comes through doorway `side`: just inside the
/// doorway, and the way into the room.
pub(crate) fn doorway(room: &Room, side: usize) -> ((f32, f32), (f32, f32)) {
    let (w, h) = (room.width(), room.height());
    match side {
        0 => ((w / 2.0, 2.0), (0.0, 1.0)),
        1 => ((w - 2.0, h / 2.0), (-1.0, 0.0)),
        2 => ((w / 2.0, h - 2.0), (0.0, -1.0)),
        _ => ((2.0, h / 2.0), (1.0, 0.0)),
    }
}

impl Run {
    /// Fill a freshly entered room with its waves and traps.
    pub(super) fn arm_room(&mut self) {
        self.traps.clear();
        self.rocks.clear();
        self.waves = Waves::default();
        self.spiked = false;
        let room = &self.dungeon.rooms[self.at];
        if room.cleared {
            return;
        }
        let (kind, depth, pack) = (room.kind, self.dungeon.depth, self.dungeon.pack);
        let waves = match kind {
            RoomKind::Fight => depth.min(3),
            RoomKind::Stairs => 1,
            _ => 0,
        };
        if waves > 0 {
            self.waves = Waves {
                left: waves,
                total: waves,
                timer: 7 * HZ,
                gate: None,
                turn: self.rng.below(4),
            };
        }
        let trapped = match kind {
            RoomKind::Fight => self.rng.chance(65),
            RoomKind::Hall => self.rng.chance(45),
            RoomKind::Stairs | RoomKind::Lair => true,
            _ => false,
        };
        if !trapped {
            return;
        }
        if kind == RoomKind::Hall {
            // A passage's trap lies across the passage itself.
            let doors = self.room().doors;
            let (mc, mr) = (self.room().cols as i32 / 2, self.room().rows as i32 / 2);
            let side = (0..4)
                .filter(|&d| doors[d])
                .nth(self.rng.below(2))
                .unwrap_or(0);
            let (col, row) = match side {
                0 => (mc - 1, mr - 5),
                1 => (mc + 6, mr - 1),
                2 => (mc - 1, mr + 3),
                _ => (mc - 8, mr - 1),
            };
            let phase = self.rng.below(SPIKE_PERIOD as usize) as u32;
            self.traps.push(Trap::Spikes { col, row, phase });
            return;
        }
        match pack {
            Pack::Crypt => {
                let area = self.room().cols * self.room().rows / (COLS * ROWS);
                let plates = 1 + self.rng.below(2) + area;
                self.place_spikes(plates);
            }
            Pack::Cavern => {
                self.traps.push(Trap::Rockfall);
                self.cues.push("trap_rocks".into());
            }
            Pack::Hellforge => {
                let (cols, rows) = (self.room().cols, self.room().rows);
                for _ in 0..(1 + cols * rows / (COLS * ROWS)).min(4) {
                    let side = if self.rng.chance(50) { 1 } else { 3 };
                    // Never across a doorway's lane, so a way in stays safe.
                    let lanes: Vec<usize> =
                        (3..rows - 3).filter(|r| r.abs_diff(rows / 2) > 2).collect();
                    let row = lanes[self.rng.below(lanes.len())];
                    let lane = (row as f32 + 0.5) * TILE_UNITS;
                    let phase = self.rng.below(VENT_PERIOD as usize) as u32;
                    if !self
                        .traps
                        .iter()
                        .any(|t| matches!(t, Trap::Vent { lane: l, .. } if *l == lane))
                    {
                        self.traps.push(Trap::Vent { side, lane, phase });
                    }
                }
                self.place_spikes(1);
                self.cues.push("trap_fire".into());
            }
        }
    }

    fn place_spikes(&mut self, plates: usize) {
        let room = &self.dungeon.rooms[self.at];
        let mut placed: Vec<(i32, i32)> = Vec::new();
        for _ in 0..60 {
            if placed.len() >= plates {
                break;
            }
            let col = 2 + self.rng.below(room.cols - 5) as i32;
            let row = 2 + self.rng.below(room.rows - 5) as i32;
            let floor =
                (0..2).all(|dr| (0..2).all(|dc| room.tile(col + dc, row + dr) == Tile::Floor));
            // Keep the doorway lanes and the room's middle clear.
            let lane = (col - (room.cols as i32 / 2 - 1)).abs() <= 2
                || (row - (room.rows as i32 / 2 - 1)).abs() <= 1;
            let apart = placed
                .iter()
                .all(|&(c, r)| (c - col).abs() > 3 || (r - row).abs() > 3);
            if floor && !lane && apart {
                placed.push((col, row));
            }
        }
        for (col, row) in placed {
            let phase = self.rng.below(SPIKE_PERIOD as usize) as u32;
            self.traps.push(Trap::Spikes { col, row, phase });
        }
    }

    /// Doorways glow, then pour monsters in; the last wave says so.
    pub(super) fn tick_waves(&mut self) {
        if self.phase != Phase::Fighting || !self.waves.pending() {
            return;
        }
        if let Some((side, left)) = self.waves.gate {
            if left > 0 {
                self.waves.gate = Some((side, left - 1));
                return;
            }
            self.waves.gate = None;
            self.pour(side);
            self.waves.left = self.waves.left.saturating_sub(1);
            self.waves.timer = 9 * HZ;
            return;
        }
        if self.waves.left == 0 {
            return;
        }
        let living = self.enemies.iter().filter(|e| e.boss.is_none()).count();
        self.waves.timer = self.waves.timer.saturating_sub(1);
        if living <= 1 {
            self.waves.timer = self.waves.timer.min(HZ / 2);
        }
        if self.waves.timer > 0 {
            return;
        }
        let doors: Vec<usize> = (0..4).filter(|&d| self.room().doors[d]).collect();
        let doors = if doors.is_empty() {
            vec![0, 1, 2, 3]
        } else {
            doors
        };
        // Never pour a wave onto the party: skip doorways a knight stands by,
        // or use the one farthest from them all.
        let knights: Vec<(f32, f32)> = self
            .players
            .values()
            .filter(|h| h.hp > 0)
            .map(|h| (h.x, h.y))
            .collect();
        let distance = |side: usize| {
            let ((x, y), _) = doorway(&self.dungeon.rooms[self.at], side);
            knights
                .iter()
                .map(|&(hx, hy)| (hx - x).hypot(hy - y))
                .fold(f32::MAX, f32::min)
        };
        let fair: Vec<usize> = doors
            .iter()
            .copied()
            .filter(|&d| distance(d) > 12.0)
            .collect();
        let side = if fair.is_empty() {
            doors
                .iter()
                .copied()
                .max_by(|&a, &b| distance(a).total_cmp(&distance(b)))
                .unwrap_or(0)
        } else {
            fair[self.waves.turn % fair.len()]
        };
        self.waves.turn += 1;
        self.waves.gate = Some((side, HZ));
        self.sounds.push("wave_gate");
        let last = self.waves.left == 1 && self.waves.total > 1;
        self.cues
            .push(if last { "wave_last" } else { "wave" }.into());
    }

    /// One wave through doorway `side`, in two files walking in.
    fn pour(&mut self, side: usize) {
        let count = (2 + self.dungeon.depth as usize + self.players.len() - 1).min(6);
        let ((x0, y0), (ix, iy)) = doorway(&self.dungeon.rooms[self.at], side);
        let (lx, ly) = (iy, ix);
        let roster = self.dungeon.pack.roster();
        for i in 0..count {
            let kind = self.rng.pick(roster);
            let deep = 1.0 + (i / 2) as f32 * 1.8;
            let side_step = if i % 2 == 0 { -1.2 } else { 1.2 };
            let (x, y) = (
                x0 + ix * deep + lx * side_step,
                y0 + iy * deep + ly * side_step,
            );
            let room = &self.dungeon.rooms[self.at];
            let col = (x / TILE_UNITS) as i32;
            let row = (y / TILE_UNITS) as i32;
            let ok = room.tile(col, row) == Tile::Floor
                || (kind.flies() && room.tile(col, row) == Tile::Hazard);
            let (x, y) = if ok {
                (x, y)
            } else {
                (x0 + ix * 2.5, y0 + iy * 2.5)
            };
            self.spawn_at(kind, x, y);
        }
    }

    /// Spikes, vents and falling rocks, after everything has moved.
    pub(super) fn tick_traps(&mut self) {
        let tick = self.tick;
        for trap in self.traps.clone() {
            match trap {
                Trap::Spikes { col, row, phase } => {
                    if spikes(tick, phase) != Spikes::Up {
                        continue;
                    }
                    let (x0, y0) = (col as f32 * TILE_UNITS, row as f32 * TILE_UNITS);
                    let inside = |x: f32, y: f32| {
                        (x0..x0 + 2.0 * TILE_UNITS).contains(&x)
                            && (y0..y0 + 2.0 * TILE_UNITS).contains(&y)
                    };
                    for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                        if inside(hero.x, hero.y) && hero.invulnerable == 0 {
                            hero.hurt(14);
                            self.sounds.push("spike");
                            if !self.spiked {
                                self.spiked = true;
                                self.cues.push("trap_spikes".into());
                            }
                        }
                    }
                    // Monsters on the plate take it once per strike.
                    if (tick as u32 + phase) % SPIKE_PERIOD == SPIKE_UP {
                        for enemy in self
                            .enemies
                            .iter_mut()
                            .filter(|e| !e.kind.flies() && e.boss.is_none())
                        {
                            if inside(enemy.x, enemy.y) {
                                enemy.hp = enemy.hp.saturating_sub(24);
                            }
                        }
                    }
                }
                Trap::Vent { side, lane, phase } => {
                    let beat = vent_beat(tick, phase);
                    if beat < 18
                        && beat.is_multiple_of(3)
                        && self.projectiles.len() < MAX_PROJECTILES
                    {
                        if beat == 0 {
                            self.sounds.push("vent_fire");
                        }
                        let ((x, y), (dx, dy)) = match side {
                            // Just inside the wall, or the wall would swallow them.
                            1 => (
                                (self.dungeon.rooms[self.at].width() - 2.2, lane),
                                (-1.0, 0.0),
                            ),
                            3 => ((2.2, lane), (1.0, 0.0)),
                            0 => ((lane, 2.2), (0.0, 1.0)),
                            _ => (
                                (lane, self.dungeon.rooms[self.at].height() - 2.2),
                                (0.0, -1.0),
                            ),
                        };
                        self.projectiles.push(Projectile {
                            x,
                            y,
                            vx: dx * 10.0,
                            vy: dy * 10.0,
                            hostile: true,
                            kind: Shot::Ember,
                            look: None,
                            damage: 12,
                            pierce: 0,
                            last_hit: None,
                            empowered: false,
                            traits: Default::default(),
                            ttl: 6 * HZ,
                        });
                    }
                }
                Trap::Rockfall => {
                    let living: Vec<(f32, f32)> = self
                        .players
                        .values()
                        .filter(|h| h.hp > 0 && !h.stone)
                        .map(|h| (h.x, h.y))
                        .collect();
                    if (tick + 17).is_multiple_of(u64::from(5 * HZ / 2))
                        && !living.is_empty()
                        && self.rocks.len() < 4
                    {
                        let (hx, hy) = living[self.rng.below(living.len())];
                        let jitter = |rng: &mut Rng| (rng.below(51) as f32 / 10.0) - 2.5;
                        let (w, h) = (self.room().width(), self.room().height());
                        let x = (hx + jitter(&mut self.rng)).clamp(3.0, w - 3.0);
                        let y = (hy + jitter(&mut self.rng)).clamp(3.0, h - 3.0);
                        self.rocks.push(Rock {
                            x,
                            y,
                            fall: ROCK_FALL,
                            dust: 0,
                        });
                    }
                }
            }
        }
        let mut landed = 0;
        for rock in &mut self.rocks {
            if rock.fall > 0 {
                rock.fall -= 1;
                if rock.fall == 0 {
                    rock.dust = ROCK_DUST;
                    landed += 1;
                    for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                        if (hero.x - rock.x).hypot(hero.y - rock.y) < 1.4 {
                            hero.hurt(16);
                        }
                    }
                    for enemy in &mut self.enemies {
                        if (enemy.x - rock.x).hypot(enemy.y - rock.y) < 1.4 + enemy.radius() * 0.5 {
                            enemy.hp = enemy.hp.saturating_sub(40);
                        }
                    }
                }
            } else {
                rock.dust = rock.dust.saturating_sub(1);
            }
        }
        if landed > 0 {
            self.sounds.push("rock_land");
            self.shake = self.shake.max(4);
        }
        self.rocks.retain(|r| r.fall > 0 || r.dust > 0);
    }

    /// A chest that bites: one chest in four, decided when it is touched.
    pub(super) fn wake_mimic(&mut self) -> bool {
        let Some(chest) = self.dungeon.rooms[self.at].chest.filter(|c| !c.open) else {
            return false;
        };
        if !self.rng.chance(25) {
            return false;
        }
        self.dungeon.rooms[self.at].chest = None;
        self.dungeon.rooms[self.at].cleared = false;
        self.spawn_at(EnemyKind::Mimic, chest.x, chest.y);
        self.phase = Phase::Fighting;
        self.cues.push("mimic".into());
        self.sounds.push("mimic");
        true
    }
}
