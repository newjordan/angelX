//! The Hollow One: stones that float round a void, in
//! the Unknown. It keeps its distance and throws cold bolts, and every so
//! often it opens a Black Hole where a knight stands. A swirl warns of it for
//! a second; then for three it drags every knight near it toward its middle
//! (a knight walking out can beat the pull), and its core burns whoever is
//! in it. The Hollow One holds still while it keeps the hole open: hit it
//! hard enough and the channel breaks, and the hole closes.

use super::*;

/// The hole's warning, how long it pulls, how far it reaches, how hard it
/// pulls (a knight walks at 13), and the burning core.
pub(crate) const HOLE_WARN: u32 = HZ;
pub(crate) const HOLE_PULL: u32 = 3 * HZ;
pub(crate) const HOLE_REACH: f32 = 7.0;
const PULL: f32 = 7.5;
pub(crate) const HOLE_CORE: f32 = 1.6;
const CORE_BURN: u32 = 8;
/// Damage that breaks the Hollow One's channel.
pub(crate) const BREAK: u32 = 120;
/// A hole every eleven seconds, a bolt every two.
const HOLE_EVERY: u32 = 11 * HZ;
const BOLT_EVERY: u32 = 2 * HZ;

/// A Black Hole: where, how long its warning and its pull have left, and
/// the Hollow One keeping it open (with its health when it began, to know
/// when a blow breaks the channel).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Hole {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) warn: u32,
    pub(crate) left: u32,
    pub(crate) owner: u32,
    pub(crate) owner_hp: u32,
}

/// The Hollow One's step. False for any other monster.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) -> bool {
    if enemy.kind != EnemyKind::Hollow {
        return false;
    }
    enemy.timer += 1;
    // Keeping a hole open: still, until the hole's time is out.
    if enemy.stage == 1 {
        if enemy.timer >= HOLE_WARN + HOLE_PULL {
            (enemy.stage, enemy.timer) = (0, 0);
        }
        return true;
    }
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let Some((tx, ty)) = target else {
        return true;
    };
    let (cx, cy) = unit(tx - enemy.x, ty - enemy.y);
    let dist = (tx - enemy.x).hypot(ty - enemy.y);
    // Float between nine and thirteen steps off, drifting sideways between.
    let side = if enemy.id.is_multiple_of(2) { 1.0 } else { -1.0 };
    let way = if dist < 9.0 {
        (-cx, -cy)
    } else if dist > 13.0 {
        (cx, cy)
    } else {
        (-cy * side, cx * side)
    };
    (enemy.x, enemy.y) = grid.slide(
        (enemy.x, enemy.y),
        (way.0 * 1.6 * DT, way.1 * 1.6 * DT),
        enemy.radius(),
        Mover::Flier,
    );
    let beat = enemy.timer + enemy.id * 17;
    if beat % HOLE_EVERY == HOLE_EVERY / 2 && dist < 16.0 {
        deeds.holes.push((tx, ty, enemy.id));
        deeds.cues.push("black_hole");
        (enemy.stage, enemy.timer) = (1, 0);
        return true;
    }
    if beat.is_multiple_of(BOLT_EVERY) {
        let aim = (ty - enemy.y).atan2(tx - enemy.x);
        shots.push(Projectile {
            x: enemy.x,
            y: enemy.y,
            vx: aim.cos() * 8.0,
            vy: aim.sin() * 8.0,
            hostile: true,
            look: None,
            kind: Shot::Orb,
            damage: 11,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: Default::default(),
            ttl: 6 * HZ,
        });
    }
    true
}

impl Run {
    /// Holes open, warn, pull and burn, and close: when their time is out,
    /// when the Hollow One keeping one falls, or when a blow breaks its
    /// channel.
    pub(super) fn tick_holes(&mut self, opened: Vec<(f32, f32, u32)>) {
        for (x, y, owner) in opened {
            let owner_hp = self
                .enemies
                .iter()
                .find(|e| e.id == owner)
                .map_or(0, |e| e.hp);
            if self.holes.len() < 3 {
                self.holes.push(Hole {
                    x,
                    y,
                    warn: HOLE_WARN,
                    left: HOLE_PULL,
                    owner,
                    owner_hp,
                });
            }
        }
        let room = &self.dungeon.rooms[self.at];
        let grid = Grid { room, barred: true };
        let mut broken = false;
        for hole in &mut self.holes {
            let keeper = self.enemies.iter().find(|e| e.id == hole.owner && e.hp > 0);
            match keeper {
                None => {
                    hole.warn = 0;
                    hole.left = 0;
                    continue;
                }
                Some(k) if k.hp + BREAK <= hole.owner_hp => {
                    hole.warn = 0;
                    hole.left = 0;
                    broken = true;
                    continue;
                }
                _ => {}
            }
            if hole.warn > 0 {
                hole.warn -= 1;
                continue;
            }
            hole.left = hole.left.saturating_sub(1);
            let burn = hole.left % 10 == 0;
            for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                let (dx, dy) = (hole.x - hero.x, hole.y - hero.y);
                let d = dx.hypot(dy);
                if d > HOLE_REACH {
                    continue;
                }
                // The pull: stronger toward the middle (none at the middle).
                if d > 0.05 {
                    let pull = PULL * (1.0 - 0.4 * d / HOLE_REACH) * DT;
                    let (ux, uy) = unit(dx, dy);
                    (hero.x, hero.y) = grid.slide(
                        (hero.x, hero.y),
                        (ux * pull.min(d), uy * pull.min(d)),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                }
                if burn && d < HOLE_CORE {
                    hero.hurt(CORE_BURN);
                    self.room_hurt = true;
                }
            }
        }
        if broken {
            self.cues.push("channel_broken".into());
            self.sounds.push("shield_block");
            self.notice("event_horizon");
        }
        self.holes.retain(|h| h.warn > 0 || h.left > 0);
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_hollow__tests.rs"]
mod tests;
