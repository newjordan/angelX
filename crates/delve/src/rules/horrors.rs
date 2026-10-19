//! The Swamp of Horrors' own company: three things that live in the mire,
//! each with a tell that shows before it lands (`Enemy::stage` and `timer`
//! are the tell, so every seat's picture shows the same warning).
//!
//! - **Bog hands** sink and travel under the mire toward a knight, where no
//!   shot finds them and their touch does nothing. Ripples ring the spot for
//!   `HAND_TELL` ticks; then the hand rises and grabs: whoever stands in the
//!   ring is hurt and held (`GRIP` ticks at `GRIP_PACE`). Risen, shots find
//!   it, until it sinks again. Step out of the ripples.
//! - **Drowned knights** wade the mire as if it were dry ground. One lifts
//!   its rusted blade for `DROWNED_WINDUP` ticks (the cleave's arc drawn on
//!   the floor, locked `DROWNED_LOCK` in), cleaves in front of it, then
//!   stands with the blade stuck in the mud: the moment to hit back.
//! - **Will-o'-wisps** keep off over the water. One flares for `WISP_LURE`
//!   ticks and draws every knight near toward it, then bursts in a ring of
//!   pale wisps and dims. Walk against the pull, or put a wall between.
//!
//! Their numbers are the named constants below; what they make comes out of
//! each one's step as `Doings`, for the run to make so (`Run::apply_horrors`).

use super::*;

/// Health: a hand, a drowned knight, a wisp.
pub const HAND_HP: u32 = 70;
pub const DROWNED_HP: u32 = 200;
pub const WISP_HP: u32 = 45;

/// A bog hand: how fast it travels under the mire and for how long, the
/// ripples' tell, how long it stays up, the grip's reach and its hit.
pub const HAND_SWIM: f32 = 5.0;
pub const HAND_SWIM_TICKS: u32 = 45;
pub const HAND_TELL: u32 = 30;
pub const HAND_UP: u32 = 75;
pub const HAND_REACH: f32 = 1.9;
pub const HAND_HIT: u32 = 14;
/// A held knight: ticks of the grip, and the pace it leaves them.
pub const GRIP: u32 = 45;
pub const GRIP_PACE: f32 = 0.3;

/// A drowned knight: its wade, how near it comes (a blade's length off), how
/// near it lifts the blade, the wind-up and when its aim locks, the
/// cleave's reach and hit, and the blade stuck after.
pub const DROWNED_PACE: f32 = 2.6;
pub const DROWNED_KEEP: f32 = 2.2;
pub const DROWNED_NEAR: f32 = 3.4;
pub const DROWNED_WINDUP: u32 = 28;
pub const DROWNED_LOCK: u32 = 18;
pub const DROWNED_REACH: f32 = 2.2;
pub const DROWNED_HIT: u32 = 20;
pub const DROWNED_STUCK: u32 = 45;

/// A will-o'-wisp: the distance it keeps, its drift, the rest between lures,
/// the lure's tell, its reach and pull (units a second), the burst's ring
/// and the dim after.
pub const WISP_NEAR: f32 = 7.0;
pub const WISP_FAR: f32 = 11.0;
const WISP_DRIFT: f32 = 3.0;
pub const WISP_REST: u32 = 90;
pub const WISP_LURE: u32 = 45;
pub const LURE_REACH: f32 = 12.0;
pub const LURE_PULL: f32 = 2.4;
pub const WISP_RING: u32 = 8;
const WISP_SHOT_SPEED: f32 = 5.5;
pub const WISP_HIT: u32 = 10;
pub const WISP_DIM: u32 = 60;

impl EnemyKind {
    /// One of the swamp's own company.
    pub fn horror(self) -> bool {
        matches!(self, EnemyKind::BogHand | EnemyKind::Drowned | EnemyKind::Wisp)
    }
}

impl Enemy {
    /// A bog hand under the mire: travelling, or rippling before it rises.
    pub fn sunk(&self) -> bool {
        self.kind == EnemyKind::BogHand && self.stage < 2
    }
}

/// What the swamp's company did this tick.
#[derive(Default)]
pub(super) struct Doings {
    /// Hands rising: where.
    grabs: Vec<(f32, f32)>,
    /// Wisps luring: where.
    lures: Vec<(f32, f32)>,
}

/// One of the swamp company's steps. False for any other monster.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) -> bool {
    use EnemyKind::*;
    if !enemy.kind.horror() {
        return false;
    }
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let seen = target.is_some();
    let (tx, ty) = target.unwrap_or((enemy.x, enemy.y + 1.0));
    let (cx, cy) = unit(tx - enemy.x, ty - enemy.y);
    let dist = (tx - enemy.x).hypot(ty - enemy.y);
    let mover = if enemy.kind.flies() {
        Mover::Flier
    } else {
        Mover::Wader
    };
    let r = enemy.radius();
    let walk = |enemy: &mut Enemy, (dx, dy): (f32, f32), speed: f32| {
        (enemy.x, enemy.y) = grid.slide(
            (enemy.x, enemy.y),
            (dx * speed * DT, dy * speed * DT),
            r,
            mover,
        );
    };
    enemy.timer += 1;
    match (enemy.kind, enemy.stage) {
        // Under the mire, travelling toward the nearest knight.
        (BogHand, 0) => {
            if seen {
                walk(enemy, (cx, cy), HAND_SWIM);
            }
            if seen && (dist < 1.0 || enemy.timer >= HAND_SWIM_TICKS) {
                (enemy.stage, enemy.timer) = (1, 0);
            }
        }
        // The ripples: it will rise here.
        (BogHand, 1) => {
            if enemy.timer >= HAND_TELL {
                deeds.swamp.grabs.push((enemy.x, enemy.y));
                deeds.cues.push("bog_grab");
                (enemy.stage, enemy.timer) = (2, 0);
            }
        }
        // Risen, clutching: shots find it now.
        (BogHand, _) => {
            if enemy.timer >= HAND_UP {
                (enemy.stage, enemy.timer) = (0, 0);
            }
        }
        // Wading in, to a blade's length off.
        (Drowned, 0) => {
            if seen {
                enemy.dir = (cx, cy);
                if dist > DROWNED_KEEP {
                    walk(enemy, (cx, cy), DROWNED_PACE);
                }
            }
            if seen && dist < DROWNED_NEAR && enemy.timer >= 30 {
                (enemy.stage, enemy.timer) = (1, 0);
                enemy.dir = (cx, cy);
            }
        }
        // The blade goes up; the arc shows where it falls.
        (Drowned, 1) => {
            if enemy.timer < DROWNED_LOCK && seen {
                enemy.dir = (cx, cy);
            }
            if enemy.timer >= DROWNED_WINDUP {
                let (dx, dy) = enemy.dir;
                deeds.rams.push((
                    enemy.x + dx * 1.4,
                    enemy.y + dy * 1.4,
                    DROWNED_REACH,
                    DROWNED_HIT,
                    (dx, dy),
                ));
                deeds.cues.push("drowned_cleave");
                deeds.sounds.push("swing");
                (enemy.stage, enemy.timer) = (2, 0);
            }
        }
        // The blade stuck in the mud.
        (Drowned, _) => {
            if enemy.timer >= DROWNED_STUCK {
                (enemy.stage, enemy.timer) = (0, 0);
            }
        }
        // Over the water, keeping off.
        (Wisp, 0) => {
            let side = if enemy.id.is_multiple_of(2) { 1.0 } else { -1.0 };
            let way = if !seen {
                (0.0, 0.0)
            } else if dist < WISP_NEAR {
                (-cx, -cy)
            } else if dist > WISP_FAR {
                (cx, cy)
            } else {
                (-cy * side, cx * side)
            };
            walk(enemy, way, WISP_DRIFT);
            if seen && dist < 14.0 && enemy.timer >= WISP_REST + (enemy.id % 4) * 12 {
                (enemy.stage, enemy.timer) = (1, 0);
            }
        }
        // The flare: it draws the knights in.
        (Wisp, 1) => {
            deeds.swamp.lures.push((enemy.x, enemy.y));
            if enemy.timer >= WISP_LURE {
                for k in 0..WISP_RING {
                    let a = k as f32 * std::f32::consts::TAU / WISP_RING as f32
                        + (enemy.id % 8) as f32 * 0.2;
                    shots.push(Projectile {
                        x: enemy.x,
                        y: enemy.y,
                        vx: a.cos() * WISP_SHOT_SPEED,
                        vy: a.sin() * WISP_SHOT_SPEED,
                        hostile: true,
                        look: None,
                        kind: Shot::Wisp,
                        damage: WISP_HIT,
                        pierce: 0,
                        last_hit: None,
                        empowered: false,
                        traits: Default::default(),
                        ttl: 4 * HZ,
                    });
                }
                deeds.cues.push("wisp_burst");
                (enemy.stage, enemy.timer) = (2, 0);
            }
        }
        // Dim, drifting back out.
        (Wisp, _) => {
            if seen && dist < WISP_FAR {
                walk(enemy, (-cx, -cy), WISP_DRIFT);
            }
            if enemy.timer >= WISP_DIM {
                (enemy.stage, enemy.timer) = (0, 0);
            }
        }
        _ => {}
    }
    true
}

impl Run {
    /// Make what the swamp's company did this tick so.
    pub(super) fn apply_horrors(&mut self, doings: Doings) {
        for (x, y) in doings.grabs {
            for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                if hero.invulnerable == 0 && (hero.x - x).hypot(hero.y - y) < HAND_REACH {
                    hero.hurt(HAND_HIT);
                    hero.mired = GRIP;
                    self.sounds.push("hero_hurt");
                }
            }
            self.sounds.push("rock_land");
            self.fx.push(super::fx::Fx::Shake(4));
        }
        if doings.lures.is_empty() {
            return;
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        for (x, y) in doings.lures {
            for h in self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && h.rolling().is_none())
            {
                let d = (h.x - x).hypot(h.y - y);
                if d < LURE_REACH && d > 0.6 {
                    let (ux, uy) = unit(x - h.x, y - h.y);
                    (h.x, h.y) = grid.slide(
                        (h.x, h.y),
                        (ux * LURE_PULL * DT, uy * LURE_PULL * DT),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                }
            }
        }
    }

    /// A bog hand's grip lets go.
    pub(super) fn tick_grip(&mut self) {
        for hero in self.players.values_mut() {
            hero.mired = hero.mired.saturating_sub(1);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_horrors__tests.rs"]
mod tests;
