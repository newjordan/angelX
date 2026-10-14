//! The loot goblin: small, quick and light-fingered.
//!
//! One turns up in about a room in three, more often the deeper the delve
//! goes, and now and then two. It does not fight. It goes for what lies on
//! the floor, then for a knight with something to lose, and a touch takes
//! some of their carried gold, gems, ember or ore, or a card from their hand.
//! Then it runs: away from the knights, round their shots, for a door,
//! and when its time is up (eight to twelve seconds) it is through it, and
//! whatever it took is gone. Killed, it bursts: everything it took comes
//! back, with a pile of gold on top, and sometimes a prize.
//!
//! The goblin's own steps are here. Its sack (`Enemy::stash` for what it took
//! from knights, `Enemy::loot` for what it found) grows as it fills.

use super::*;
use crate::realm::{Spoil, Spoils};

/// Ticks a goblin runs after a theft before it is through a door; spread by
/// its id, so a pair do not leave together.
pub const RUN_TICKS: u32 = 8 * HZ;
/// Ticks a goblin that has taken nothing waits for something to take.
pub const IDLE_TICKS: u32 = 16 * HZ;
const SCAVENGE_SPEED: f32 = 7.5;
const FLEE_SPEED: f32 = 10.5;
/// How long "THIEF!" hangs over a goblin that has just stolen.
pub const CRY_TICKS: u32 = 2 * HZ;
/// The most thefts one goblin makes.
const THEFTS: usize = 4;
/// What a goblin may take from a knight's pack.
const TAKES: [Spoil; 4] = [Spoil::Gold, Spoil::Gem, Spoil::Ember, Spoil::Ore];
/// Percent of a pile one touch takes, at least one.
const TAKE_SHARE: u32 = 40;
/// Percent of fight rooms with a goblin in them, by floor, and of those with
/// a second.
const GOBLIN_ROOMS: [u32; 6] = [30, 30, 35, 40, 45, 50];
const GOBLIN_PAIRS: [u32; 6] = [10, 15, 20, 25, 30, 35];

impl Enemy {
    /// How full the sack is, from 0 (empty) to 3 (bursting).
    pub fn burden(&self) -> u8 {
        let things = self.stash.0.values().filter(|&&n| n > 0).count() + self.loot.len();
        match things {
            0 => 0,
            1 => 1,
            2 | 3 => 2,
            _ => 3,
        }
    }

    /// Running, with something in the sack.
    pub fn fleeing(&self) -> bool {
        self.kind == EnemyKind::Goblin && self.stage == 1
    }
}

/// Where a room's doors are, a step inside each.
fn exits(room: &Room) -> Vec<(f32, f32)> {
    let (w, h) = (room.width(), room.height());
    [
        (w / 2.0, 0.3),
        (w - 0.3, h / 2.0),
        (w / 2.0, h - 0.3),
        (0.3, h / 2.0),
    ]
    .into_iter()
    .zip(room.doors)
    .filter(|&(_, open)| open)
    .map(|(at, _)| at)
    .collect()
}

impl Run {
    /// Now and then a fight room has a loot goblin in it, and more rarely two.
    pub(super) fn maybe_goblin(&mut self) {
        // A camp is garrisoned; a goblin has no business in it.
        if self.room().kind != RoomKind::Fight
            || self
                .boss_gates
                .as_ref()
                .is_some_and(|g| g.economy.camp(self.at).is_some())
        {
            return;
        }
        let index = (self.dungeon.depth.clamp(1, 6) - 1) as usize;
        if !self.rng.chance(GOBLIN_ROOMS[index]) {
            return;
        }
        let (w, h) = (self.room().width(), self.room().height());
        let mut spots = vec![(w * 0.8, h * 0.25)];
        if self.rng.chance(GOBLIN_PAIRS[index]) {
            spots.push((w * 0.2, h * 0.25));
        }
        for (x, y) in spots {
            let room = self.room();
            if room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
                && self.spawn_at(EnemyKind::Goblin, x, y).is_some()
            {
                self.cues.push("goblin".into());
            }
        }
    }

    /// A step for every goblin in the room: scavenge, steal, run, get away.
    pub(super) fn tick_thieves(&mut self) {
        let ids: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| {
                e.kind == EnemyKind::Goblin && e.hp > 0 && e.age >= TELEGRAPH && e.frozen == 0
            })
            .map(|e| e.id)
            .collect();
        for id in ids {
            self.goblin_step(id);
        }
    }

    fn goblin_step(&mut self, id: u32) {
        let Some(at) = self.enemies.iter().position(|e| e.id == id) else {
            return;
        };
        let (gx, gy, timer, fleeing) = {
            let g = &mut self.enemies[at];
            g.cry = g.cry.saturating_sub(1);
            (g.x, g.y, g.timer, g.stage == 1)
        };
        // Who could be robbed, and who is nearest.
        let knights: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
            .map(|(&hid, h)| (hid, h.x, h.y))
            .collect();
        let nearest = knights.iter().copied().min_by(|a, b| {
            (a.1 - gx)
                .hypot(a.2 - gy)
                .total_cmp(&(b.1 - gx).hypot(b.2 - gy))
        });
        let doors = exits(self.room());
        let wobble = (timer as f32 * 0.31 + id as f32).sin();
        let run = RUN_TICKS + (id % 5) * HZ;
        let (way, speed) = if fleeing {
            if timer >= run {
                return self.goblin_gets_away(id);
            }
            // Away from the nearest knight, toward the door that knight is
            // farthest from, round a shot coming at it.
            let (nx, ny) = nearest.map_or((gx, gy + 1.0), |(_, x, y)| (x, y));
            let door = doors.iter().copied().max_by(|a, b| {
                let score =
                    |d: &(f32, f32)| (d.0 - nx).hypot(d.1 - ny) - 0.7 * (d.0 - gx).hypot(d.1 - gy);
                score(a).total_cmp(&score(b))
            });
            if let Some((dx, dy)) = door
                && (dx - gx).hypot(dy - gy) < 1.6
            {
                return self.goblin_gets_away(id);
            }
            let (ax, ay) = unit(gx - nx, gy - ny);
            let (tx, ty) = door.map_or((0.0, 0.0), |(dx, dy)| unit(dx - gx, dy - gy));
            let (mut vx, mut vy) = (
                tx + ax * 0.6 - ay * wobble * 0.5,
                ty + ay * 0.6 + ax * wobble * 0.5,
            );
            for shot in self.projectiles.iter().filter(|p| !p.hostile && p.ttl > 0) {
                let (sx, sy) = (gx - shot.x, gy - shot.y);
                let d = sx.hypot(sy);
                let speed = shot.vx.hypot(shot.vy).max(0.01);
                if d < 3.5 && (shot.vx * sx + shot.vy * sy) / (speed * d.max(0.01)) > 0.6 {
                    let (px, py) = (-shot.vy / speed, shot.vx / speed);
                    let side = if px * sx + py * sy >= 0.0 { 1.0 } else { -1.0 };
                    vx += px * side * 1.6;
                    vy += py * side * 1.6;
                }
            }
            (unit(vx, vy), FLEE_SPEED)
        } else {
            if timer >= IDLE_TICKS {
                return self.goblin_gets_away(id);
            }
            // Loot on the floor first, then a knight with something to lose,
            // else a wary distance.
            let item = self
                .room()
                .items
                .iter()
                .enumerate()
                .map(|(i, item)| (i, item.x, item.y))
                .min_by(|a, b| {
                    (a.1 - gx)
                        .hypot(a.2 - gy)
                        .total_cmp(&(b.1 - gx).hypot(b.2 - gy))
                });
            let mark = self
                .players
                .iter()
                .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0 && h.stealable())
                .map(|(_, h)| (h.x, h.y))
                .min_by(|a, b| {
                    (a.0 - gx)
                        .hypot(a.1 - gy)
                        .total_cmp(&(b.0 - gx).hypot(b.1 - gy))
                });
            if let Some((i, x, y)) = item
                && (x - gx).hypot(y - gy) < 1.0
            {
                self.goblin_grabs(at, i);
                return;
            }
            let goal = item
                .map(|(_, x, y)| (x, y))
                .or(mark)
                .or(nearest.map(|(_, x, y)| (gx + (gx - x), gy + (gy - y))));
            let (ux, uy) = goal.map_or((0.0, 0.0), |(x, y)| unit(x - gx, y - gy));
            (
                unit(ux - uy * wobble * 0.4, uy + ux * wobble * 0.4),
                SCAVENGE_SPEED,
            )
        };
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let radius = self.enemies[at].radius();
        let mut to = grid.slide(
            (gx, gy),
            (way.0 * speed * DT, way.1 * speed * DT),
            radius,
            Mover::Walker,
        );
        if (to.0 - gx).hypot(to.1 - gy) < speed * DT * 0.3 {
            // Cornered: slip along the wall.
            to = grid.slide(
                (gx, gy),
                (-way.1 * speed * DT, way.0 * speed * DT),
                radius,
                Mover::Walker,
            );
        }
        let g = &mut self.enemies[at];
        (g.x, g.y) = to;
        if way.0 != 0.0 || way.1 != 0.0 {
            g.dir = way;
        }
        // A touch robs a knight (a rolling one is not there to touch).
        let touch = radius + HERO_RADIUS + 0.3;
        let victim = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0 && h.dash_ticks == 0)
            .find(|(_, h)| (h.x - to.0).hypot(h.y - to.1) < touch && h.stealable())
            .map(|(&hid, _)| hid);
        if let Some(hid) = victim {
            self.goblin_steals(at, hid);
        }
    }

    /// A goblin picks up a card lying on the floor, into its sack.
    fn goblin_grabs(&mut self, goblin: usize, item: usize) {
        let at = self.at;
        let taken = self.dungeon.rooms[at].items.swap_remove(item);
        let g = &mut self.enemies[goblin];
        g.loot.push(taken.card);
        self.cues.push("goblin_grab".into());
        self.sounds.push("spoil_pickup");
    }

    /// A goblin takes something from a knight: a share of a pile of gold,
    /// gems, ember or ore, or a card from their hand. Then it runs.
    fn goblin_steals(&mut self, goblin: usize, hero: u32) {
        if self.enemies[goblin].loot.len() + self.enemies[goblin].stash.0.len() >= THEFTS + 2 {
            return;
        }
        let Some(knight) = self.players.get(&hero) else {
            return;
        };
        let piles: Vec<(Spoil, u32)> = TAKES
            .iter()
            .map(|&s| (s, knight.carried.get(s)))
            .filter(|&(_, n)| n > 0)
            .collect();
        let cards = knight.hand.len();
        let pick = self.rng.below(piles.len() + cards);
        let name;
        if let Some(&(spoil, n)) = piles.get(pick) {
            let taken = (n * TAKE_SHARE / 100).max(1);
            let mut price = Spoils::default();
            price.add(spoil, taken);
            if let Some(knight) = self.players.get_mut(&hero) {
                knight.carried.take(&price);
            }
            self.enemies[goblin].stash.add(spoil, taken);
            name = format!("{taken} {}", spoil.word());
        } else {
            let slot = pick - piles.len();
            let knight = self.players.get_mut(&hero).expect("checked");
            let card = knight.hand.remove(slot.min(knight.hand.len() - 1));
            name = self
                .book
                .get(&card)
                .map_or_else(|| card.clone(), |c| c.name.clone());
            self.enemies[goblin].loot.push(card);
        }
        let g = &mut self.enemies[goblin];
        if g.stage != 1 {
            (g.stage, g.timer) = (1, 0);
        }
        g.cry = CRY_TICKS;
        let who = self
            .players
            .get(&hero)
            .map_or_else(String::new, |h| h.name.clone());
        self.cues.push("thief".into());
        self.sounds.push("hero_hurt");
        self.found = Some((self.tick, hero, format!("THIEF! took {name}")));
        let _ = who;
    }

    /// A goblin gets through a door: what it carries is gone.
    pub(super) fn goblin_gets_away(&mut self, id: u32) {
        if let Some(g) = &mut self.boss_gates {
            g.economy.retire(id);
        }
        let Some(i) = self.enemies.iter().position(|e| e.id == id) else {
            return;
        };
        let goblin = self.enemies.remove(i);
        if self.blasts.len() < 16 {
            self.blasts.push((goblin.x, goblin.y, BLAST_TICKS));
        }
        self.cues.push("goblin_escaped".into());
        self.notice("sticky_fingers");
        let took = goblin.burden() > 0;
        self.found = Some((
            self.tick,
            0,
            if took {
                "The loot goblin got away with it".into()
            } else {
                "The loot goblin got away".into()
            },
        ));
    }

    /// A goblin dies: its sack bursts. What it took from the knights goes
    /// back to the nearest, what it found lies where it fell, and a pile of
    /// gold and now and then a prize come on top.
    pub(super) fn goblin_bursts(&mut self, x: f32, y: f32, stash: Spoils, loot: Vec<String>) {
        if let Some(hero) = self
            .players
            .values_mut()
            .filter(|h| h.hp > 0 && !h.stone)
            .min_by(|a, b| {
                (a.x - x)
                    .hypot(a.y - y)
                    .total_cmp(&(b.x - x).hypot(b.y - y))
            })
        {
            hero.carried.merge(&stash);
        }
        let depth = self.dungeon.depth;
        let pile = 5 + depth.min(6);
        let mut spill: Vec<String> = loot;
        spill.extend(std::iter::repeat_n("gold".to_string(), pile as usize));
        let pack = self.dungeon.pack;
        if self.rng.chance(40)
            && let Some(prize) = self.book.roll_chest(&mut self.rng, pack).into_iter().next()
        {
            spill.push(prize);
        }
        let n = spill.len().max(1) as f32;
        for (k, card) in spill.into_iter().enumerate() {
            let a = k as f32 / n * std::f32::consts::TAU + 0.4;
            let (w, h) = (self.room().width(), self.room().height());
            self.drop_item(
                card,
                (x + a.cos() * 2.0).clamp(2.0, w - 2.0),
                (y + a.sin() * 1.5).clamp(2.0, h - 2.0),
                None,
            );
        }
        if !stash.is_empty() {
            self.found = Some((
                self.tick,
                0,
                format!("The goblin bursts: {} back", stash.label()),
            ));
        }
        self.cues.push("goblin_caught".into());
        self.notice("not_today_goblin");
    }
}

impl Hero {
    /// Something a goblin could take: a pile of gold, gems, ember or ore, or
    /// a card in hand.
    pub fn stealable(&self) -> bool {
        !self.hand.is_empty() || TAKES.iter().any(|&s| self.carried.get(s) > 0)
    }
}
