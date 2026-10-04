//! A friend's mirror of the host's delve, and smooth motion between ticks.
//!
//! The host's run is the only game. A friend's angelX holds a copy of it:
//! the whole run when the floor (or its book) changes, then each tick only
//! what moves — knights, monsters, shots, the room's loot and doors, its
//! traps — a couple of kilobytes. The friend draws that copy itself, on its
//! own screen, as often as its terminal takes frames.
//!
//! Between two ticks a view may stand anywhere: `Pose` remembers where
//! things stood on the tick before, and `between` places them part of the
//! way on, so 30 ticks a second draw as 60 smooth frames.

use super::*;

/// What one tick changed: everything a view draws, without the floor's
/// layout, the book or the guardians (those come with the whole run).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Live {
    pub(crate) raid_id: u64,
    pub(crate) depth: u32,
    pub(crate) tick: u64,
    pub(crate) phase: Phase,
    pub(crate) players: BTreeMap<u32, Hero>,
    pub(crate) enemies: Vec<Enemy>,
    pub(crate) projectiles: Vec<Projectile>,
    pub(crate) score: u32,
    pub(crate) at: usize,
    pub(crate) room: RoomLive,
    pub(crate) waves: Waves,
    pub(crate) traps: Vec<Trap>,
    pub(crate) rocks: Vec<Rock>,
    pub(crate) hallowed: Option<u64>,
    pub(crate) wishing: bool,
    pub(crate) shake: u32,
    pub(crate) found: Option<(u64, u32, String)>,
    #[serde(default)]
    pub(crate) sparks: Vec<Spark>,
    #[serde(default)]
    pub(crate) window: Option<overclass::Window>,
    #[serde(default)]
    pub(crate) blasts: Vec<(f32, f32, u8)>,
    #[serde(default)]
    pub(crate) boons: Vec<(u32, u64)>,
}

/// The parts of the party's room that change while they stand in it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RoomLive {
    pub(crate) kind: RoomKind,
    pub(crate) cleared: bool,
    pub(crate) visited: bool,
    pub(crate) items: Vec<Item>,
    pub(crate) chest: Option<layout::Chest>,
}

/// Where everything stood on one tick, for drawing part of the way on.
#[derive(Clone, Debug, Default)]
pub(crate) struct Pose {
    tick: u64,
    at: usize,
    players: Vec<(u32, f32, f32)>,
    enemies: Vec<(u32, f32, f32)>,
}

impl Run {
    /// Which whole run a mirror needs: a new raid, floor or book.
    pub(crate) fn mirror_key(&self) -> (u64, u32, usize) {
        (self.raid_id, self.dungeon.depth, self.book.revision)
    }

    /// This tick's changes, for a friend's mirror.
    pub(crate) fn live(&self) -> Live {
        let room = self.room();
        Live {
            raid_id: self.raid_id,
            depth: self.dungeon.depth,
            tick: self.tick,
            phase: self.phase,
            players: self.players.clone(),
            enemies: self.enemies.clone(),
            projectiles: self.projectiles.clone(),
            score: self.score,
            at: self.at,
            room: RoomLive {
                kind: room.kind,
                cleared: room.cleared,
                visited: room.visited,
                items: room.items.clone(),
                chest: room.chest,
            },
            waves: self.waves.clone(),
            traps: self.traps.clone(),
            rocks: self.rocks.clone(),
            hallowed: self.hallowed,
            wishing: self.wishing,
            shake: self.shake,
            found: self.found.clone(),
            sparks: self.sparks.clone(),
            window: self.window.clone(),
            blasts: self.blasts.clone(),
            boons: self.boons.clone(),
        }
    }

    /// Take a tick's changes into this mirror. False when they belong to
    /// another raid or floor (the whole run must come first).
    pub(crate) fn apply_live(&mut self, live: Live) -> bool {
        if live.raid_id != self.raid_id
            || live.depth != self.dungeon.depth
            || live.at >= self.dungeon.rooms.len()
        {
            return false;
        }
        self.tick = live.tick;
        self.phase = live.phase;
        self.players = live.players;
        self.enemies = live.enemies;
        self.projectiles = live.projectiles;
        self.score = live.score;
        self.at = live.at;
        let room = &mut self.dungeon.rooms[live.at];
        room.kind = live.room.kind;
        room.cleared = live.room.cleared;
        room.visited = live.room.visited;
        room.items = live.room.items;
        room.chest = live.room.chest;
        self.waves = live.waves;
        self.traps = live.traps;
        self.rocks = live.rocks;
        self.hallowed = live.hallowed;
        self.wishing = live.wishing;
        self.shake = live.shake;
        self.found = live.found;
        self.sparks = live.sparks;
        self.window = live.window;
        self.blasts = live.blasts;
        self.boons = live.boons;
        true
    }

    /// Where everything stands now.
    pub(crate) fn pose(&self) -> Pose {
        Pose {
            tick: self.tick,
            at: self.at,
            players: self.players.iter().map(|(&id, h)| (id, h.x, h.y)).collect(),
            enemies: self.enemies.iter().map(|e| (e.id, e.x, e.y)).collect(),
        }
    }

    /// This run drawn `alpha` (0–1) of the way from `before` (the tick just
    /// gone) to now. Only motion is blended, and only across one tick in
    /// the same room: a door, a stair or a long gap draws as it stands.
    pub(crate) fn between(&self, before: &Pose, alpha: f32) -> Run {
        let mut view = self.clone();
        view.blend(before, alpha);
        view
    }

    /// `f` with this run drawn part of the way from `before`, then put back
    /// as it was: no copy of the whole run per frame.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn drawn_between<T>(
        &mut self,
        before: &Pose,
        alpha: f32,
        f: impl FnOnce(&Run) -> T,
    ) -> T {
        self.drawn_ahead(before, alpha, None, f)
    }

    /// As `drawn_between`, with knight `own.0` drawn where their own held
    /// keys (`own.1`) will have taken them `own.2` seconds after this tick:
    /// a friend's own knight answers their keys at once, and the host's
    /// next tick corrects it.
    pub(crate) fn drawn_ahead<T>(
        &mut self,
        before: &Pose,
        alpha: f32,
        own: Option<(u32, Input, f32)>,
        f: impl FnOnce(&Run) -> T,
    ) -> T {
        let now = self.pose();
        let shots: Vec<(f32, f32)> = self.projectiles.iter().map(|p| (p.x, p.y)).collect();
        self.blend(before, alpha);
        if let Some((id, input, ahead)) = own {
            self.predict(id, input, ahead);
        }
        let out = f(self);
        for (id, x, y) in &now.players {
            if let Some(hero) = self.players.get_mut(id) {
                (hero.x, hero.y) = (*x, *y);
            }
        }
        for (id, x, y) in &now.enemies {
            if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == *id) {
                (enemy.x, enemy.y) = (*x, *y);
            }
        }
        for (shot, (x, y)) in self.projectiles.iter_mut().zip(shots) {
            (shot.x, shot.y) = (x, y);
        }
        out
    }

    /// Walk knight `id` on from the tick's place by `input` for `seconds`
    /// (at most a few ticks), as the sim walks: their pace, sliding along
    /// stone. Side-on halls and rolls are left to the host.
    fn predict(&mut self, id: u32, input: Input, seconds: f32) {
        if self.side_on() || !input.valid() {
            return;
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: !self.enemies.is_empty(),
        };
        let Some(hero) = self.players.get_mut(&id).filter(|h| h.hp > 0 && !h.stone) else {
            return;
        };
        if hero.dash_ticks > 0 {
            return;
        }
        let (mx, my) = unit(input.move_x as f32, input.move_y as f32);
        if mx == 0.0 && my == 0.0 {
            return;
        }
        let pace = HERO_SPEED * (100 + hero.bonus.speed) as f32 / 100.0;
        let pace = if input.dash && hero.guard != cards::Guard::Roll {
            pace * SHIELD_PACE
        } else {
            pace
        };
        let ticks = (seconds.clamp(0.0, 0.25) / DT).round() as u32;
        let mut at = (hero.x, hero.y);
        for _ in 0..ticks {
            at = grid.slide(
                at,
                (mx * pace * DT, my * pace * DT),
                HERO_RADIUS,
                Mover::Hero,
            );
        }
        (hero.x, hero.y) = at;
    }

    fn blend(&mut self, before: &Pose, alpha: f32) {
        let view = self;
        if before.tick + 1 != view.tick || before.at != view.at {
            return;
        }
        let k = 1.0 - alpha.clamp(0.0, 1.0);
        let back = |now: f32, then: f32| {
            // A jump of more than a few units is a teleport (a pit, a
            // door): it is not drawn sliding.
            if (now - then).abs() > 4.0 {
                now
            } else {
                now - (now - then) * k
            }
        };
        for (id, x, y) in &before.players {
            if let Some(hero) = view.players.get_mut(id) {
                (hero.x, hero.y) = (back(hero.x, *x), back(hero.y, *y));
            }
        }
        for (id, x, y) in &before.enemies {
            if let Some(enemy) = view.enemies.iter_mut().find(|e| e.id == *id) {
                (enemy.x, enemy.y) = (back(enemy.x, *x), back(enemy.y, *y));
            }
        }
        for shot in &mut view.projectiles {
            shot.x -= shot.vx * DT * k;
            shot.y -= shot.vy * DT * k;
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_mirror__tests.rs"]
mod tests;
