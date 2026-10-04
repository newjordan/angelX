//! Side-on rooms: the camera turns to the side and knights feel gravity.
//!
//! A body's position is its middle; its feet are `FOOT` below. Up on the
//! move axis jumps (once more in the air), down drops through a plank,
//! the roll runs flat along the ground. Pits hurt and put the knight back
//! on the last firm footing. Fliers fly as ever; walkers fall.

use super::*;

/// From a body's middle to its feet (and to its head).
pub(crate) const FOOT: f32 = 1.0;
const HALF_W: f32 = 0.45;
const GRAVITY: f32 = 95.0;
const JUMP: f32 = 31.0;
/// The second jump, in the air, is a little weaker.
const AIR_JUMP: f32 = 28.0;
const FALL_MAX: f32 = 40.0;
const AIR_JUMPS: u8 = 1;
/// Ticks a dropped knight passes through planks.
const DROP: u32 = 8;
const PIT_HURT: u32 = 20;
/// How fast walkers fall, per tick.
const WALKER_FALL: f32 = 0.8;

impl Grid<'_> {
    fn tile_side(&self, x: f32, y: f32) -> Tile {
        self.tile_at(x, y)
    }

    /// Stone to a body moving sideways or rising.
    fn solid(&self, x: f32, y: f32) -> bool {
        match self.tile_side(x, y) {
            Tile::Wall | Tile::Block => true,
            Tile::Door => self.barred,
            _ => false,
        }
    }

    /// A body centred at `(x, y)` overlaps stone.
    fn body_blocked(&self, x: f32, y: f32) -> bool {
        [-HALF_W, HALF_W].into_iter().any(|dx| {
            [-FOOT + 0.05, 0.0, FOOT - 0.05]
                .into_iter()
                .any(|dy| self.solid(x + dx, y + dy))
        })
    }

    /// Feet moving down from `from` to `to`: the top they land on, if one
    /// lies between. Planks hold only feet that come from above them.
    fn landing(&self, x: f32, from: f32, to: f32, through_planks: bool) -> Option<f32> {
        let top = (to / TILE_UNITS).floor() * TILE_UNITS;
        if from > top + 0.001 || to < top {
            return None;
        }
        [-HALF_W, HALF_W]
            .into_iter()
            .find_map(|dx| match self.tile_side(x + dx, top + 0.01) {
                Tile::Wall | Tile::Block => Some(top),
                Tile::Ledge if !through_planks => Some(top),
                _ => None,
            })
    }

    /// Something to stand on right under feet at `feet`.
    fn underfoot(&self, x: f32, feet: f32, through_planks: bool) -> bool {
        self.landing(x, feet, feet + 0.02, through_planks).is_some()
    }

    fn pit(&self, x: f32, feet: f32) -> bool {
        self.tile_side(x, feet - 0.01) == Tile::Hazard
    }
}

/// One knight's step side-on: `dx` across (running or rolling), then the
/// jump, the fall and the landing. Returns true when the knight fell in a
/// pit this step.
pub(super) fn walk(hero: &mut Hero, input: &Input, grid: &Grid, dx: f32, rolling: bool) -> bool {
    let jump = input.move_y < 0;
    if jump && !hero.jump_held && !rolling {
        if hero.grounded {
            hero.vy = -JUMP;
            hero.grounded = false;
        } else if hero.air_jumps > 0 {
            hero.air_jumps -= 1;
            hero.vy = -AIR_JUMP;
        }
    }
    hero.jump_held = jump;
    hero.dropping = hero.dropping.saturating_sub(1);
    if input.move_y > 0 && hero.grounded && !grid.underfoot(hero.x, hero.y + FOOT, true) {
        // Standing on a plank, not stone: down drops through it.
        hero.dropping = DROP;
        hero.grounded = false;
    }
    if !grid.body_blocked(hero.x + dx, hero.y) {
        hero.x += dx;
    }
    if rolling {
        // The roll runs flat; gravity waits for it to end.
        hero.vy = 0.0;
    } else {
        hero.vy = (hero.vy + GRAVITY * DT).min(FALL_MAX);
    }
    let dy = hero.vy * DT;
    let feet = hero.y + FOOT;
    if dy > 0.0 {
        match grid.landing(hero.x, feet, feet + dy, hero.dropping > 0) {
            Some(top) => {
                hero.y = top - FOOT;
                hero.vy = 0.0;
                hero.grounded = true;
                hero.air_jumps = AIR_JUMPS;
            }
            None => {
                hero.y += dy;
                hero.grounded = false;
            }
        }
    } else if dy < 0.0 {
        let head = hero.y - FOOT + dy;
        if [-HALF_W, HALF_W]
            .into_iter()
            .any(|ex| grid.solid(hero.x + ex, head))
        {
            // A bonk: the head meets stone and the rise ends.
            hero.y = ((head / TILE_UNITS).floor() + 1.0) * TILE_UNITS + FOOT;
            hero.vy = 0.0;
        } else {
            hero.y += dy;
        }
        hero.grounded = false;
    } else {
        hero.grounded = grid.underfoot(hero.x, feet, hero.dropping > 0);
        if hero.grounded {
            hero.air_jumps = AIR_JUMPS;
        }
    }
    let feet = hero.y + FOOT;
    if grid.pit(hero.x, feet) {
        // The grace after a fall covers the next one: holding on toward
        // the pit costs one bite, not five.
        if hero.invulnerable == 0 {
            hero.hp = hero.hp.saturating_sub(PIT_HURT);
        }
        if let Some((x, y)) = hero.footing {
            (hero.x, hero.y) = (x, y);
        }
        hero.vy = 0.0;
        hero.dash_ticks = 0;
        hero.invulnerable = hero.invulnerable.max(HZ);
        return true;
    }
    // Firm footing: both feet on something, for a pit to put you back on.
    if hero.grounded
        && [-HALF_W, HALF_W]
            .into_iter()
            .all(|ex| grid.underfoot(hero.x + ex * 2.0, feet, false))
    {
        hero.footing = Some((hero.x, hero.y));
    }
    false
}

/// A walker side-on: it keeps the ground it walks on, falls off what it
/// walks past, and a pit is the end of it.
pub(super) fn fall(enemy: &mut Enemy, grid: &Grid) {
    let feet = enemy.y + FOOT;
    match grid.landing(enemy.x, feet, feet + WALKER_FALL, false) {
        Some(top) => enemy.y = top - FOOT,
        None => enemy.y += WALKER_FALL,
    }
    if grid.pit(enemy.x, enemy.y + FOOT) {
        enemy.hp = 0;
    }
}

/// The ground under column `x`: the top of the highest stone or plank
/// below `from`, as a body's middle standing on it.
pub(super) fn stand_on(room: &Room, x: f32, from: f32) -> f32 {
    let col = (x / TILE_UNITS).floor() as i32;
    let mut row = (from / TILE_UNITS).floor() as i32;
    while row < room.rows as i32 {
        if matches!(room.tile(col, row), Tile::Wall | Tile::Block | Tile::Ledge) && row > 0 {
            return row as f32 * TILE_UNITS - FOOT;
        }
        row += 1;
    }
    from
}

impl Run {
    /// Whether the party stands in a side-on room.
    pub(crate) fn side_on(&self) -> bool {
        self.room().kind == RoomKind::Ledge
    }

    /// Into a side-on hall: the party comes in at its low west doorway, on
    /// the ground, falling still.
    pub(super) fn arrive_side_on(&mut self) {
        let room = &self.dungeon.rooms[self.at];
        let door_row = (0..room.rows as i32)
            .find(|&r| room.tile(0, r) == Tile::Door)
            .unwrap_or(room.rows as i32 / 2);
        let from = door_row as f32 * TILE_UNITS;
        let spots: Vec<(f32, f32)> = (0..self.players.len())
            .map(|slot| {
                let x = 3.3 + slot as f32 * 1.6;
                (x, stand_on(room, x, from))
            })
            .collect();
        for (hero, (x, y)) in self.players.values_mut().zip(spots) {
            (hero.x, hero.y) = (x, y);
            hero.vy = 0.0;
            hero.grounded = true;
            hero.air_jumps = AIR_JUMPS;
            hero.footing = Some((x, y));
            hero.aim_x = 1.0;
            hero.aim_y = 0.0;
        }
    }

    /// A monster's place side-on: walkers on a ledge or the ground well
    /// away from the door, fliers in the open air above.
    pub(super) fn perch(&mut self, flies: bool) -> (f32, f32) {
        let room = &self.dungeon.rooms[self.at];
        let (cols, rows) = (room.cols, room.rows);
        for _ in 0..64 {
            let col = 12 + self.rng.below(cols - 14) as i32;
            let x = (col as f32 + 0.5) * TILE_UNITS;
            if flies {
                let row = 2 + self.rng.below(rows / 2 - 1) as i32;
                if room.tile(col, row) == Tile::Floor {
                    return (x, (row as f32 + 0.5) * TILE_UNITS);
                }
                continue;
            }
            // A standing place: the first stone or plank under open air.
            let start = (1 + self.rng.below(rows - 3)) as f32 * TILE_UNITS;
            let y = stand_on(room, x, start);
            let feet = y + FOOT;
            let ground = room.tile(col, (feet / TILE_UNITS) as i32);
            if room.tile(col, (y / TILE_UNITS) as i32) == Tile::Floor
                && matches!(ground, Tile::Block | Tile::Ledge)
            {
                return (x, y);
            }
        }
        (room.width() / 2.0, TILE_UNITS * 3.0)
    }
}
