//! The deep's hunters, the third company:
//!
//! - **The Flesher** shambles after the party in a cloud of rot that wears
//!   at whoever stands close. Every few seconds he stops, stares down a
//!   line (it shows, red, on the floor) and throws his meat hook down it.
//!   A knight it catches is reeled in to his cleaver. Step off the line, or
//!   roll through the hook.
//! - **The Silkmother** keeps her distance, spins webs under the knights
//!   (a knight in a web walks at a little over half pace) and hatches
//!   spiderlings, small and quick and many.
//!
//! The monsters' steps say what they do as `Deeds`; the run makes it so.

use super::foes::Deeds;
use super::*;

/// The Flesher's hook in flight: whose it is, where its head is, the way
/// it flies, how far it has gone, whether it is coming back, and the
/// knight it caught.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Hook {
    pub(crate) by: u32,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) dir: (f32, f32),
    pub(crate) out: f32,
    pub(crate) back: bool,
    pub(crate) caught: Option<u32>,
}

/// A web on the floor: where, and ticks left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Web {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) left: u32,
}

/// How far the hook flies, how fast out and back, the stare before the
/// throw, how often, and how hard the cleaver lands on a knight reeled in.
pub(crate) const HOOK_RANGE: f32 = 15.0;
const HOOK_SPEED: f32 = 22.0;
const REEL_SPEED: f32 = 16.0;
pub(crate) const HOOK_WINDUP: u32 = 20;
const HOOK_EVERY: u32 = 4 * HZ;
const HOOK_DAMAGE: u32 = 20;
/// The rot round the Flesher: its reach, and a point of health every few
/// ticks (it never fells a knight; the cleaver does that).
pub(crate) const ROT_REACH: f32 = 2.6;
const ROT_EVERY: u32 = 8;
/// A web's reach, how long it lasts, and the pace a knight keeps in one.
pub(crate) const WEB_REACH: f32 = 2.6;
pub(crate) const WEB_LASTS: u32 = 8 * HZ;
pub(crate) const WEB_PACE: f32 = 0.6;
/// The most spiderlings a room holds at once.
const BROOD: usize = 6;

/// One of the third company's steps. False for any other monster.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    deeds: &mut Deeds,
) -> bool {
    use EnemyKind::*;
    if !matches!(enemy.kind, Flesher | Silkmother | Spiderling) {
        return false;
    }
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let (tx, ty) = target.unwrap_or((enemy.x, enemy.y));
    let (cx, cy) = unit(tx - enemy.x, ty - enemy.y);
    let dist = (tx - enemy.x).hypot(ty - enemy.y);
    let r = enemy.radius();
    let walk = |enemy: &mut Enemy, (dx, dy): (f32, f32), speed: f32| {
        (enemy.x, enemy.y) = grid.slide(
            (enemy.x, enemy.y),
            (dx * speed * DT, dy * speed * DT),
            r,
            Mover::Walker,
        );
    };
    enemy.timer += 1;
    let beat = enemy.timer + enemy.id * 7;
    match enemy.kind {
        Flesher => {
            if beat.is_multiple_of(ROT_EVERY) {
                deeds.rot.push((enemy.x, enemy.y));
            }
            match enemy.stage {
                // Shamble after the party; when one is in reach, stare.
                0 => {
                    walk(enemy, (cx, cy), 1.6);
                    let ready = enemy.timer >= HOOK_EVERY + (enemy.id % 4) * 9;
                    if ready && target.is_some() && dist < HOOK_RANGE - 1.0 {
                        (enemy.stage, enemy.timer, enemy.dir) = (1, 0, (cx, cy));
                    }
                }
                // The stare: his line shows on the floor.
                1 => {
                    if enemy.timer >= HOOK_WINDUP {
                        deeds.hooks.push((enemy.id, enemy.x, enemy.y, enemy.dir));
                        deeds.cues.push("hook_thrown");
                        (enemy.stage, enemy.timer) = (2, 0);
                    }
                }
                // Reeling in, out of breath.
                _ => {
                    if enemy.timer >= 2 * HZ {
                        (enemy.stage, enemy.timer) = (0, 0);
                    }
                }
            }
        }
        Silkmother => {
            // Keep off, sidling; webs under the knights, spiderlings.
            let side = if enemy.id.is_multiple_of(2) { 1.0 } else { -1.0 };
            let way = if dist < 8.0 {
                (-cx, -cy)
            } else if dist > 13.0 {
                (cx, cy)
            } else {
                (-cy * side, cx * side)
            };
            walk(enemy, way, 2.4);
            if beat % 100 == 50 && target.is_some() && dist < 15.0 {
                deeds.webs.push((tx, ty));
            }
            if beat % 180 == 90 {
                for side in [-1.0f32, 1.0] {
                    deeds
                        .spawns
                        .push((Spiderling, enemy.x + side * 1.4, enemy.y + 0.8, 0));
                }
                deeds.cues.push("brood_hatch");
            }
        }
        _ => {
            // A spiderling scuttles straight in, zig-zagging.
            let zig = ((enemy.age as f32) * 0.5 + enemy.id as f32).sin() * 0.6;
            walk(enemy, unit(cx - cy * zig, cy + cx * zig), 5.4);
        }
    }
    true
}

impl Run {
    /// Hooks thrown, webs spun, the rot: make what the hunters did so.
    pub(super) fn hunt(
        &mut self,
        hooks: Vec<(u32, f32, f32, (f32, f32))>,
        webs: Vec<(f32, f32)>,
        rot: Vec<(f32, f32)>,
    ) {
        for (by, x, y, dir) in hooks {
            if self.hooks.len() < 4 {
                self.hooks.push(Hook {
                    by,
                    x,
                    y,
                    dir,
                    out: 0.0,
                    back: false,
                    caught: None,
                });
            }
        }
        for (x, y) in webs {
            if self.webs.len() >= 6 {
                self.webs.remove(0);
            }
            self.webs.push(Web {
                x,
                y,
                left: WEB_LASTS,
            });
        }
        for (x, y) in rot {
            for hero in self.players.values_mut().filter(|h| h.hp > 1 && !h.stone) {
                if (hero.x - x).hypot(hero.y - y) < ROT_REACH {
                    hero.hp -= 1;
                    self.room_hurt = true;
                }
            }
        }
    }

    /// Hooks fly and reel in; webs fade.
    pub(super) fn tick_hunters(&mut self) {
        for web in &mut self.webs {
            web.left = web.left.saturating_sub(1);
        }
        self.webs.retain(|w| w.left > 0);
        let mut done = Vec::new();
        for i in 0..self.hooks.len() {
            let mut hook = self.hooks[i];
            let Some((bx, by, br)) = self
                .enemies
                .iter()
                .find(|e| e.id == hook.by && e.hp > 0)
                .map(|e| (e.x, e.y, e.radius()))
            else {
                // The Flesher fell: his hook drops whatever it held.
                done.push(i);
                continue;
            };
            if hook.back {
                let (ux, uy) = unit(bx - hook.x, by - hook.y);
                let step = REEL_SPEED * DT;
                (hook.x, hook.y) = (hook.x + ux * step, hook.y + uy * step);
                let home = (bx - hook.x).hypot(by - hook.y) < br + 1.0;
                if let Some(hero) = hook.caught.and_then(|id| self.players.get_mut(&id)) {
                    (hero.x, hero.y) = (hook.x, hook.y);
                    if home {
                        hero.invulnerable = 0;
                        hero.hurt(HOOK_DAMAGE);
                        self.room_hurt = true;
                        self.shake = self.shake.max(6);
                        self.sounds.push("hero_hurt");
                    }
                }
                if home {
                    done.push(i);
                }
            } else {
                let step = HOOK_SPEED * DT;
                (hook.x, hook.y) = (hook.x + hook.dir.0 * step, hook.y + hook.dir.1 * step);
                hook.out += step;
                let room = &self.dungeon.rooms[self.at];
                let tile = room.tile((hook.x / TILE_UNITS) as i32, (hook.y / TILE_UNITS) as i32);
                if hook.out >= HOOK_RANGE || matches!(tile, Tile::Wall | Tile::Block) {
                    hook.back = true;
                }
                // The first knight it touches, unless rolling clear.
                let caught = self
                    .players
                    .iter()
                    .filter(|(_, h)| h.hp > 0 && !h.stone && h.dash_ticks == 0 && h.immune == 0)
                    .find(|(_, h)| (h.x - hook.x).hypot(h.y - hook.y) < 1.1)
                    .map(|(&id, _)| id);
                if let Some(id) = caught {
                    (hook.caught, hook.back) = (Some(id), true);
                    self.cues.push("hooked".into());
                    self.sounds.push("chest_open");
                    self.notice("fresh_meat");
                }
            }
            self.hooks[i] = hook;
        }
        for i in done.into_iter().rev() {
            self.hooks.remove(i);
        }
    }

    /// Whether a knight standing at `(x, y)` is in a web.
    pub(super) fn webbed(webs: &[Web], x: f32, y: f32) -> bool {
        webs.iter().any(|w| (w.x - x).hypot(w.y - y) < WEB_REACH)
    }

    /// The spiderlings a room holds now, for the brood's cap.
    pub(super) fn brood_full(&self) -> bool {
        self.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Spiderling && e.hp > 0)
            .count()
            >= BROOD
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_hunters__tests.rs"]
mod tests;
