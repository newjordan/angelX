//! Follies: a room of the delve that plays like another game. The first is
//! the Wax Maze, a nod to Pac-Man in our own crypt: the room's pillars make
//! a maze strewn with candle wax, four wraiths roam it, and every knight in
//! the room munches at once. The wax is shared, so friends race each other
//! for the corridors; each knight counts their own.
//!
//! Each wraith has a temperament: the Hound chases the nearest knight, the
//! Cutter heads for where that knight is going, the Flanker comes from the
//! far side, and the Drifter wanders. Holy water in the corners turns them
//! pale and fleeing for a while, and a pale wraith caught is worth wax and a
//! bone. A wraith that touches a knight sends them back to the door and
//! spills some of their wax; nobody is hurt.
//!
//! The even floors' folly is the Belfry Shoot instead (below).
//!
//! Stakes: each knight antes a tenth of the gold they carry. When the wax is
//! gone or the candle burns down, the most wax takes the pot and the crown;
//! everyone keeps a gold for every five drops they ate.
use super::*;
use std::collections::BTreeSet;

/// How long the maze lasts, and the title card before it starts (ticks).
pub const MAZE_TIME: u32 = 75 * HZ;
pub const TITLE: u32 = 2 * HZ;
/// How long holy water keeps the wraiths pale.
pub const PALE: u32 = 6 * HZ;
/// Ticks a wraith takes to cross a tile, calm and pale.
pub const WRAITH_STEP: u32 = 6;
pub const PALE_STEP: u32 = 10;
/// Wax a wraith's touch spills, and wax a pale wraith caught is worth.
pub const SPILL: u16 = 5;
pub const CATCH: u16 = 5;
/// A caught wraith waits in the middle this long before it roams again.
pub const REST: u32 = 3 * HZ;
/// A knight's ante, in tenths of the gold they carry.
pub const ANTE_TENTHS: u32 = 1;
/// Drops of wax that make a gold to keep.
pub const WAX_PER_GOLD: u16 = 5;
/// After a touch, how long a knight is left alone.
pub const SPOOKED: u32 = HZ;

/// A wraith's temperament.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Temper {
    Hound,
    Cutter,
    Flanker,
    Drifter,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wraith {
    pub temper: Temper,
    /// The tile it is on and the tile it is going to.
    pub at: (i32, i32),
    pub to: (i32, i32),
    /// Ticks into the step from `at` to `to`.
    pub step: u32,
    /// Ticks left resting in the middle after being caught.
    pub rest: u32,
}

impl Wraith {
    /// Where it is now, in tiles (fractional, between `at` and `to`).
    pub fn pos(&self, pale: bool) -> (f32, f32) {
        let span = if pale { PALE_STEP } else { WRAITH_STEP } as f32;
        let t = (self.step as f32 / span).min(1.0);
        (
            self.at.0 as f32 + (self.to.0 - self.at.0) as f32 * t + 0.5,
            self.at.1 as f32 + (self.to.1 - self.at.1) as f32 * t + 0.5,
        )
    }
}

/// One Wax Maze, as it stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaxMaze {
    /// The room it is played in.
    pub room: usize,
    /// Ticks since it began (the title card first).
    pub t: u32,
    /// Wax still lying on each tile, by tile index (`y * cols + x`).
    pub wax: BTreeSet<u16>,
    pub holy: BTreeSet<u16>,
    pub wraiths: Vec<Wraith>,
    /// Ticks the wraiths stay pale.
    pub pale: u32,
    /// Each knight's drops eaten, and their ante in gold.
    pub eaten: BTreeMap<u32, u16>,
    pub ante: BTreeMap<u32, u32>,
    /// Knights left alone after a wraith's touch, until this tick.
    pub spooked: BTreeMap<u32, u32>,
    /// Where a spooked knight goes back to (units).
    pub door: (i32, i32),
    pub done: bool,
}

/// Whether tile `(x, y)` of a room `cols` x `rows` is a maze corridor: the
/// ring inside the walls, and every third row and column; the rest are the
/// crypt's pillars, two by two.
pub fn corridor(x: i32, y: i32, cols: i32, rows: i32) -> bool {
    (1..cols - 1).contains(&x)
        && (1..rows - 1).contains(&y)
        && (x == 1 || x == cols - 2 || y == 1 || y == rows - 2 || x % 3 == 0 || y % 3 == 0)
}

impl WaxMaze {
    pub fn valid(&self) -> bool {
        self.t <= TITLE + MAZE_TIME + HZ
            && self.wax.len() <= 1024
            && self.holy.len() <= 8
            && self.wraiths.len() <= 4
            && self.pale <= PALE
            && self.eaten.len() <= MAX_PLAYERS as usize
            && self.ante.len() <= MAX_PLAYERS as usize
            && self.spooked.len() <= MAX_PLAYERS as usize
            && self.eaten.keys().chain(self.ante.keys()).all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.wraiths.iter().all(|w| w.rest <= REST && w.step <= PALE_STEP)
    }
    /// Seconds left on the candle.
    pub fn left(&self) -> u32 {
        (TITLE + MAZE_TIME).saturating_sub(self.t).min(MAZE_TIME).div_ceil(HZ)
    }
    pub fn titled(&self) -> bool {
        self.t < TITLE
    }
}

impl Run {
    /// One hall on each floor of the delve is the floor's folly: the seed's
    /// pick, never the start, a boss's camp or the way down.
    pub(super) fn pick_folly(&mut self) {
        self.folly = None;
        self.bats = None;
        self.powder = None;
        self.hop = None;
        self.folly_room = None;
        if self.dungeon.depth == 0 {
            return;
        }
        let camps = |i: usize| {
            self.boss_gates
                .as_ref()
                .is_some_and(|g| g.economy.camp(i).is_some() || g.guardian == i)
        };
        let fights: Vec<usize> = (0..self.dungeon.rooms.len())
            .filter(|&i| {
                let r = &self.dungeon.rooms[i];
                r.kind == RoomKind::Hall && !r.great() && !r.cleared && i != self.at && !camps(i)
            })
            .collect();
        if fights.is_empty() {
            return;
        }
        let pick = (mix(self.seed ^ self.raid_id.rotate_left(9) ^ u64::from(self.dungeon.depth)) % fights.len() as u64) as usize;
        self.folly_room = Some(fights[pick]);
    }

    /// The party walks into the folly: pillars rise into a maze, wax is
    /// strewn, the wraiths wait in the middle, and each knight antes.
    pub(super) fn start_wax_maze(&mut self, index: usize) {
        let room = &mut self.dungeon.rooms[index];
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let mut wax = BTreeSet::new();
        for y in 1..rows - 1 {
            for x in 1..cols - 1 {
                if corridor(x, y, cols, rows) {
                    room.set(x as usize, y as usize, Tile::Floor);
                    wax.insert((y * cols + x) as u16);
                } else {
                    room.set(x as usize, y as usize, Tile::Block);
                }
            }
        }
        let corners = [(1, 1), (cols - 2, 1), (1, rows - 2), (cols - 2, rows - 2)];
        let holy: BTreeSet<u16> = corners.iter().map(|&(x, y)| (y * cols + x) as u16).collect();
        for h in &holy {
            wax.remove(h);
        }
        // The middle crossing is the wraiths' rest; no wax there.
        let (cx, cy) = (cols / 2 / 3 * 3, rows / 2 / 3 * 3);
        wax.remove(&((cy * cols + cx) as u16));
        let tempers = [Temper::Hound, Temper::Cutter, Temper::Flanker, Temper::Drifter];
        let wraiths = tempers
            .iter()
            .enumerate()
            .map(|(i, &temper)| Wraith {
                temper,
                at: (cx, cy),
                to: (cx, cy),
                step: 0,
                rest: i as u32 * HZ,
            })
            .collect();
        let (ante, scores) = take_antes(&mut self.players);
        let eaten = scores.keys().map(|&id| (id, 0u16)).collect();
        let door = self
            .players
            .values()
            .next()
            .map_or((0, 0), |h| ((h.x * 16.0) as i32, (h.y * 16.0) as i32));
        // The room's stones changed: friends' mirrors take the whole floor.
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        self.folly = Some(WaxMaze {
            room: index,
            t: 0,
            wax,
            holy,
            wraiths,
            pale: 0,
            eaten,
            ante,
            spooked: BTreeMap::new(),
            door,
            done: false,
        });
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// The maze's tick: the knights eat, the wraiths roam, and the end.
    pub(super) fn tick_folly(&mut self) {
        let Some(mut maze) = self.folly.take() else {
            return;
        };
        if maze.room != self.at || maze.done {
            return;
        }
        maze.t += 1;
        if maze.titled() {
            self.folly = Some(maze);
            return;
        }
        let (cols, rows) = (self.room().cols as i32, self.room().rows as i32);
        let tile = |x: f32, y: f32| ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
        // Eating, and the holy water.
        let mut chomps = 0;
        for (&id, hero) in self.players.iter().filter(|(_, h)| h.hp > 0 && !h.stone) {
            let (tx, ty) = tile(hero.x, hero.y);
            let at = (ty * cols + tx) as u16;
            if maze.wax.remove(&at) {
                *maze.eaten.entry(id).or_default() += 1;
                chomps += 1;
            }
            if maze.holy.remove(&at) {
                maze.pale = PALE;
                self.sounds.push("holy_water");
            }
        }
        if chomps > 0 && self.tick % 4 == 0 {
            self.sounds.push("spoil_pickup");
        }
        maze.pale = maze.pale.saturating_sub(1);
        let pale = maze.pale > 0;
        // The wraiths: a tile at a time, choosing at each crossing.
        let knights: Vec<(u32, (i32, i32), (f32, f32))> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(&id, h)| (id, tile(h.x, h.y), (h.aim_x, h.aim_y)))
            .collect();
        let step = if pale { PALE_STEP } else { WRAITH_STEP };
        let seed = self.seed;
        let t = maze.t;
        for (k, w) in maze.wraiths.iter_mut().enumerate() {
            if w.rest > 0 {
                w.rest -= 1;
                continue;
            }
            w.step += 1;
            if w.step < step {
                continue;
            }
            w.step = 0;
            w.at = w.to;
            let target = if pale {
                // Fleeing: to the corner farthest from the nearest knight.
                let near = knights
                    .iter()
                    .min_by_key(|(_, p, _)| (p.0 - w.at.0).abs() + (p.1 - w.at.1).abs())
                    .map_or((cols / 2, rows / 2), |(_, p, _)| *p);
                [(1, 1), (cols - 2, 1), (1, rows - 2), (cols - 2, rows - 2)]
                    .into_iter()
                    .max_by_key(|c| (c.0 - near.0).abs() + (c.1 - near.1).abs())
                    .unwrap()
            } else {
                let near = knights
                    .iter()
                    .min_by_key(|(_, p, _)| (p.0 - w.at.0).abs() + (p.1 - w.at.1).abs());
                match (w.temper, near) {
                    (_, None) => (cols / 2, rows / 2),
                    (Temper::Hound, Some((_, p, _))) => *p,
                    (Temper::Cutter, Some((_, p, aim))) => (
                        p.0 + (aim.0 * 4.0).round() as i32,
                        p.1 + (aim.1 * 4.0).round() as i32,
                    ),
                    (Temper::Flanker, Some((_, p, _))) => (cols - 1 - p.0, p.1),
                    (Temper::Drifter, Some(_)) => {
                        let h = mix(seed ^ (k as u64) << 8 ^ u64::from(t / (2 * HZ)));
                        ((h % cols as u64) as i32, ((h >> 16) % rows as u64) as i32)
                    }
                }
            };
            let back = (w.at.0 * 2 - w.to.0, w.at.1 * 2 - w.to.1);
            let options: Vec<(i32, i32)> = [(0, -1), (1, 0), (0, 1), (-1, 0)]
                .into_iter()
                .map(|(dx, dy)| (w.at.0 + dx, w.at.1 + dy))
                .filter(|&(x, y)| corridor(x, y, cols, rows))
                .collect();
            let pick = options
                .iter()
                .filter(|&&p| p != back || options.len() == 1)
                .min_by_key(|p| (p.0 - target.0).pow(2) + (p.1 - target.1).pow(2))
                .copied();
            w.to = pick.unwrap_or(w.at);
        }
        // Touches.
        let middle = (cols / 2 / 3 * 3, rows / 2 / 3 * 3);
        let door = (maze.door.0 as f32 / 16.0, maze.door.1 as f32 / 16.0);
        for w in maze.wraiths.iter_mut().filter(|w| w.rest == 0) {
            let (wx, wy) = w.pos(pale);
            for (&id, hero) in self.players.iter_mut().filter(|(_, h)| h.hp > 0 && !h.stone) {
                if (hero.x / TILE_UNITS - wx).hypot(hero.y / TILE_UNITS - wy) > 0.9 {
                    continue;
                }
                if pale {
                    *maze.eaten.entry(id).or_default() += CATCH;
                    hero.carried.add(Spoil::Bone, 1);
                    *w = Wraith { temper: w.temper, at: middle, to: middle, step: 0, rest: REST };
                    self.sounds.push("hit_magic");
                    break;
                }
                if maze.spooked.get(&id).is_some_and(|&until| maze.t < until) {
                    continue;
                }
                let eaten = maze.eaten.entry(id).or_default();
                *eaten = eaten.saturating_sub(SPILL);
                maze.spooked.insert(id, maze.t + SPOOKED);
                (hero.x, hero.y) = door;
                hero.invulnerable = hero.invulnerable.max(SPOOKED);
                self.sounds.push("hero_hurt");
            }
        }
        if maze.wax.is_empty() || maze.t >= TITLE + MAZE_TIME {
            self.finish_wax_maze(&mut maze);
        }
        self.folly = Some(maze);
    }

    /// The podium: everyone keeps a gold a five drops, the most wax takes
    /// the pot and the crown, and the room is done.
    fn finish_wax_maze(&mut self, maze: &mut WaxMaze) {
        maze.done = true;
        let pot: u32 = maze.ante.values().sum();
        let best = maze
            .eaten
            .iter()
            .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
            .map(|(&id, _)| id);
        let rivals = maze.eaten.len() > 1;
        for (&id, &n) in &maze.eaten {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.carried.add(Spoil::Gold, u32::from(n / WAX_PER_GOLD));
                if rivals {
                    hero.laurel = Some(id) == best;
                }
                if Some(id) == best {
                    hero.carried.add(Spoil::Gold, pot);
                }
            }
        }
        if let Some(room) = self.dungeon.rooms.get_mut(maze.room) {
            room.cleared = true;
        }
        let name = best
            .and_then(|id| self.players.get(&id))
            .map_or_else(|| "NOBODY".to_string(), |h| h.name.to_uppercase());
        let most = best.and_then(|id| maze.eaten.get(&id)).copied().unwrap_or(0);
        self.found = Some((self.tick, 0, format!("{name} ATE THE MOST WAX: {most}!")));
        self.sounds.push("play_card");
        self.cues.push("pastime_won".into());
    }
}

/// The Belfry Shoot, a nod to Duck Hunt: on the even floors the folly hall
/// fills with bats, a flock every two seconds, for a minute. Everyone shoots
/// with their own arms; whoever deals the bats the most harm takes the pot
/// and the crown. The bats bite back, as bats do.
pub const BAT_TIME: u32 = 60 * HZ;
pub const BAT_EVERY: u32 = 2 * HZ;
pub const BAT_FLOCK: u32 = 3;
pub const BAT_ALIVE: usize = 12;
/// Harm dealt that makes a gold to keep.
pub const HARM_PER_GOLD: u32 = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatShoot {
    pub room: usize,
    pub t: u32,
    /// Harm each knight has dealt the bats, and their ante.
    pub scores: BTreeMap<u32, u32>,
    pub ante: BTreeMap<u32, u32>,
    pub done: bool,
}

impl BatShoot {
    pub fn valid(&self) -> bool {
        self.t <= TITLE + BAT_TIME + HZ
            && self.scores.len() <= MAX_PLAYERS as usize
            && self.ante.len() <= MAX_PLAYERS as usize
            && self.scores.keys().chain(self.ante.keys()).all(|id| (1..=MAX_PLAYERS).contains(id))
    }
    pub fn left(&self) -> u32 {
        (TITLE + BAT_TIME).saturating_sub(self.t).min(BAT_TIME).div_ceil(HZ)
    }
    pub fn titled(&self) -> bool {
        self.t < TITLE
    }
}

/// Each knight antes a tenth of the gold they carry; the ante and a fresh
/// score line for everyone standing.
pub(super) fn take_antes(players: &mut BTreeMap<u32, Hero>) -> (BTreeMap<u32, u32>, BTreeMap<u32, u32>) {
    let mut ante = BTreeMap::new();
    let mut scores = BTreeMap::new();
    for (&id, hero) in players.iter_mut().filter(|(_, h)| h.hp > 0 && !h.stone) {
        let gold = hero.carried.get(Spoil::Gold) * ANTE_TENTHS / 10;
        if gold > 0 {
            let mut price = Spoils::default();
            price.add(Spoil::Gold, gold);
            hero.carried.take(&price);
        }
        ante.insert(id, gold);
        scores.insert(id, 0);
    }
    (ante, scores)
}

impl Run {
    /// A folly is being played in the room the party stands in: it is not
    /// cleared, and its doors hold, until the podium.
    pub fn folly_running(&self) -> bool {
        self.folly.as_ref().is_some_and(|f| f.room == self.at && !f.done)
            || self.bats.as_ref().is_some_and(|b| b.room == self.at && !b.done)
            || self.gantry.as_ref().is_some_and(|g| g.room == self.at && !g.done)
            || self.powder.as_ref().is_some_and(|p| p.room == self.at && !p.done)
            || self.hop.as_ref().is_some_and(|h| h.room == self.at && !h.done)
            || self.race3d.as_ref().is_some_and(|r| r.room == self.at && !r.done)
    }

    /// Into the even floors' folly: the belfry's bats wake.
    pub(super) fn start_bat_shoot(&mut self, index: usize) {
        let (ante, scores) = take_antes(&mut self.players);
        self.bats = Some(BatShoot {
            room: index,
            t: 0,
            scores,
            ante,
            done: false,
        });
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// Harm dealt this tick, credited to whoever dealt it.
    pub(super) fn credit_bats(&mut self, credits: &[(u32, u32)]) {
        let Some(shoot) = self.bats.as_mut().filter(|b| b.room == self.at && !b.done && !b.titled()) else {
            return;
        };
        for &(id, harm) in credits {
            if let Some(score) = shoot.scores.get_mut(&id) {
                *score = score.saturating_add(harm);
            }
        }
    }

    /// The shoot's tick: a flock every two seconds, then the podium.
    pub(super) fn tick_bats(&mut self) {
        let Some(mut shoot) = self.bats.take() else {
            return;
        };
        if shoot.room != self.at || shoot.done {
            self.bats = Some(shoot);
            return;
        }
        shoot.t += 1;
        if !shoot.titled() && (shoot.t - TITLE).is_multiple_of(BAT_EVERY) {
            // Down from the rafters: open floor in the room's upper half,
            // spread by the seed.
            let room = self.room();
            let open: Vec<(i32, i32)> = (1..room.rows as i32 / 2)
                .flat_map(|y| (1..room.cols as i32 - 1).map(move |x| (x, y)))
                .filter(|&(x, y)| room.tile(x, y) == Tile::Floor)
                .collect();
            for k in 0..BAT_FLOCK {
                if self.enemies.len() >= BAT_ALIVE || open.is_empty() {
                    break;
                }
                let spot = mix(self.seed ^ u64::from(shoot.t) << 4 ^ u64::from(k));
                let (x, y) = open[(spot % open.len() as u64) as usize];
                self.spawn_body(EnemyKind::Bat, (x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS);
            }
        }
        if shoot.t >= TITLE + BAT_TIME {
            shoot.done = true;
            self.enemies.retain(|e| e.kind != EnemyKind::Bat);
            let pot: u32 = shoot.ante.values().sum();
            let best = shoot
                .scores
                .iter()
                .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
                .map(|(&id, _)| id);
            let rivals = shoot.scores.len() > 1;
            for (&id, &n) in &shoot.scores {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, n / HARM_PER_GOLD);
                    if rivals {
                        hero.laurel = Some(id) == best;
                    }
                    if Some(id) == best {
                        hero.carried.add(Spoil::Gold, pot);
                    }
                }
            }
            if let Some(room) = self.dungeon.rooms.get_mut(shoot.room) {
                room.cleared = true;
            }
            let name = best
                .and_then(|id| self.players.get(&id))
                .map_or_else(|| "NOBODY".to_string(), |h| h.name.to_uppercase());
            self.found = Some((self.tick, 0, format!("{name} SHOT THE MOST BATS!")));
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
        self.bats = Some(shoot);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_folly__tests.rs"]
mod tests;
