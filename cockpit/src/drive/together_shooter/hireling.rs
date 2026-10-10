//! A hireling, after every tavern's sellswords: Beaumains, the kitchen
//! knight, keeps the west table in Maud's tavern. Sir Kay named him for his
//! soft kitchen hands; Malory says he was Sir Gareth all along and told
//! nobody. For a wage he goes down the stair with the party for one delve.
//! He keeps a few steps off whatever is nearest and throws carving knives at
//! it, steps out of the way of what flies at him, and follows the party
//! through the doors. Felled, he sits the fight out and gets up after it.
//! He isn't one of the party's knights: a guardian doesn't grow for him, he
//! can't lose the delve, and he isn't company for Sir Dinadan's songs.

use super::*;

/// Where he waits in the tavern (tiles), the plate that hires him, and his
/// wage.
pub(crate) const BEAUMAINS_AT: (f32, f32) = (4.3, 9.0);
pub(crate) const HIRE_PLATE: (i32, i32, i32, i32) = (5, 10, 2, 1);
pub(crate) const WAGE: u32 = 120;
/// A knife every four-fifths of a second, how fast it flies, and how far
/// he can see to throw one.
const KNIFE_EVERY: u32 = 4 * HZ / 5;
const KNIFE_SPEED: f32 = 13.0;
const SIGHT: f32 = 16.0;
/// The distance he keeps from what he fights, and his pace beside a
/// knight's.
const KEEP: (f32, f32) = (5.0, 9.0);
const PACE: f32 = 0.85;
/// How near a knight he walks when there's nothing to fight.
const HEEL: f32 = 3.0;

/// His health and his knife's weight, both growing with the floor.
fn health(depth: u32) -> u32 {
    70 + 20 * depth
}

fn knife(depth: u32) -> u32 {
    6 + 2 * depth
}

/// Beaumains, in the delve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Hireling {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) hp: u32,
    pub(crate) max_hp: u32,
    /// A hit's grace, as a knight has.
    pub(crate) invulnerable: u32,
    pub(crate) cooldown: u32,
    /// Which way he faces, and which way he sidles while he keeps his
    /// distance.
    pub(crate) facing: f32,
    pub(crate) side: f32,
}

impl Hireling {
    pub(super) fn new(depth: u32, (x, y): (f32, f32)) -> Hireling {
        let hp = health(depth);
        Hireling {
            x,
            y,
            hp,
            max_hp: hp,
            invulnerable: 0,
            cooldown: KNIFE_EVERY,
            facing: 1.0,
            side: 1.0,
        }
    }

    /// Felled, and sitting this fight out.
    pub(crate) fn down(&self) -> bool {
        self.hp == 0
    }

    pub(super) fn hurt(&mut self, damage: u32) {
        if self.invulnerable == 0 && self.hp > 0 {
            self.hp = self.hp.saturating_sub(damage);
            self.invulnerable = 24;
        }
    }
}

/// Whether a point stands on the plate that hires him.
pub(crate) fn hire_at(x: f32, y: f32) -> bool {
    let (c, r, w, h) = HIRE_PLATE;
    let (col, row) = (x / TILE_UNITS, y / TILE_UNITS);
    col >= c as f32 && col < (c + w) as f32 && row >= r as f32 && row < (r + h) as f32
}

impl Run {
    /// Down the stair: Beaumains, if he was hired, comes too. The realm
    /// forgets the wage (the cockpit hears the mark).
    pub(super) fn hire(&mut self) {
        if self.home.hire.take().is_none() {
            return;
        }
        let at = self
            .players
            .values()
            .next()
            .map_or((4.0, 4.0), |h| (h.x - 1.5, h.y + 1.0));
        self.hireling = Some(Hireling::new(self.dungeon.depth, at));
        *self.marks.entry("hired".into()).or_default() += 1;
        self.cues.push("hired:beaumains".into());
    }

    /// Into a new room: he comes through the door at the party's heels, on
    /// open floor beside the first knight.
    pub(super) fn hireling_follows(&mut self) {
        let Some(lead) = self.players.values().find(|h| h.hp > 0).map(|h| (h.x, h.y)) else {
            return;
        };
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        let Some(hire) = self.hireling.as_mut() else {
            return;
        };
        let spot = [
            (-1.5, 1.0),
            (1.5, 1.0),
            (0.0, 1.5),
            (-1.5, -1.0),
            (1.5, -1.0),
        ]
        .into_iter()
        .map(|(dx, dy)| (lead.0 + dx, lead.1 + dy))
        .find(|&(x, y)| grid.clear(x, y, HERO_RADIUS, Mover::Hero))
        .unwrap_or(lead);
        (hire.x, hire.y) = spot;
        hire.invulnerable = hire.invulnerable.max(20);
    }

    /// Each tick of a delve: he fights, or follows, or sits it out.
    pub(super) fn tick_hireling(&mut self) {
        let Some(mut hire) = self.hireling.take() else {
            return;
        };
        self.drive_hireling(&mut hire);
        self.hireling = Some(hire);
    }

    fn drive_hireling(&mut self, hire: &mut Hireling) {
        hire.invulnerable = hire.invulnerable.saturating_sub(1);
        hire.cooldown = hire.cooldown.saturating_sub(1);
        if hire.down() {
            if !self.foes_left() {
                // The fight is over: up again, sore.
                hire.hp = hire.max_hp / 2;
                hire.invulnerable = 20;
                self.cues.push("beaumains_up".into());
            }
            return;
        }
        // What finds him: monsters' shots (a hex just glances off a kitchen
        // boy), and monsters close enough to touch.
        for shot in self
            .projectiles
            .iter_mut()
            .filter(|p| p.hostile && p.ttl > 0)
        {
            if (shot.x - hire.x).hypot(shot.y - hire.y) < HERO_RADIUS + 0.16 {
                shot.ttl = 0;
                if shot.kind != Shot::Hex {
                    hire.hurt(shot.damage);
                }
            }
        }
        if self.enemies.iter().any(|e| {
            e.hp > 0
                && e.age >= TELEGRAPH
                && e.frozen == 0
                && e.kind != EnemyKind::Dummy
                && (hire.x - e.x).hypot(hire.y - e.y) < e.radius() + 0.5
        }) {
            hire.hurt(12);
        }
        if hire.down() {
            self.cues.push("beaumains_down".into());
            self.sounds.push("hero_hurt");
            return;
        }
        let target = self
            .enemies
            .iter()
            .filter(|e| {
                e.hp > 0
                    && e.age >= TELEGRAPH
                    && e.kind != EnemyKind::Dummy
                    && (e.x - hire.x).hypot(e.y - hire.y) < SIGHT
            })
            .min_by(|a, b| {
                (a.x - hire.x)
                    .hypot(a.y - hire.y)
                    .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
            })
            .map(|e| (e.x, e.y));
        let mut way = (0.0, 0.0);
        if let Some((tx, ty)) = target {
            let (ux, uy) = unit(tx - hire.x, ty - hire.y);
            let d = (tx - hire.x).hypot(ty - hire.y);
            hire.facing = ux;
            if self.tick.is_multiple_of(u64::from(3 * HZ)) {
                hire.side = -hire.side;
            }
            way = if d < KEEP.0 {
                (-ux, -uy)
            } else if d > KEEP.1 {
                (ux, uy)
            } else {
                (-uy * hire.side, ux * hire.side)
            };
            if hire.cooldown == 0 && self.projectiles.len() < MAX_PROJECTILES {
                hire.cooldown = KNIFE_EVERY;
                self.projectiles.push(Projectile {
                    x: hire.x,
                    y: hire.y,
                    vx: ux * KNIFE_SPEED,
                    vy: uy * KNIFE_SPEED,
                    hostile: false,
                    look: None,
                    kind: Shot::Blade,
                    damage: knife(self.dungeon.depth),
                    pierce: 0,
                    last_hit: None,
                    empowered: false,
                    traits: Default::default(),
                    ttl: 2 * HZ,
                });
                self.sounds.push("swing");
            }
        } else if let Some((lx, ly)) = self
            .players
            .values()
            .filter(|h| h.hp > 0)
            .map(|h| (h.x, h.y))
            .min_by(|a, b| {
                (a.0 - hire.x)
                    .hypot(a.1 - hire.y)
                    .total_cmp(&(b.0 - hire.x).hypot(b.1 - hire.y))
            })
            && (lx - hire.x).hypot(ly - hire.y) > HEEL
        {
            way = unit(lx - hire.x, ly - hire.y);
            hire.facing = way.0;
        }
        // Out of the way of the nearest shot coming at him: off its line,
        // on the side he's already on.
        if let Some(shot) = self
            .projectiles
            .iter()
            .filter(|p| p.hostile && p.ttl > 0)
            .filter(|p| {
                let (rx, ry) = (hire.x - p.x, hire.y - p.y);
                rx.hypot(ry) < 3.5 && rx * p.vx + ry * p.vy > 0.0
            })
            .min_by(|a, b| {
                (a.x - hire.x)
                    .hypot(a.y - hire.y)
                    .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
            })
        {
            let (vx, vy) = unit(shot.vx, shot.vy);
            let (rx, ry) = (hire.x - shot.x, hire.y - shot.y);
            let along = rx * vx + ry * vy;
            let (px, py) = (rx - along * vx, ry - along * vy);
            way = if px.hypot(py) < 0.05 {
                (-vy, vx)
            } else {
                unit(px, py)
            };
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let pace = HERO_SPEED * PACE * DT;
        (hire.x, hire.y) = grid.slide(
            (hire.x, hire.y),
            (way.0 * pace, way.1 * pace),
            HERO_RADIUS,
            Mover::Hero,
        );
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_hireling__tests.rs"]
mod tests;
