//! The Lich: an ice-crowned skeleton in a slate robe who
//! keeps his distance in the Drowned Archive and the Unknown. He throws cold
//! bolts, and every few seconds Rimeleap: a slow pale orb that chills the
//! knight it finds, then leaps to another knight close by, and on, five
//! times, never straight back to the one it just left. Alone, a knight takes
//! it once. A party had better spread out. A raised shield ends the chain.

use super::*;

/// How long a knight stays chilled, and how fast a chilled knight walks.
pub(crate) const CHILL: u32 = 3 * HZ / 2;
pub(crate) const CHILL_PACE: f32 = 0.6;
/// Rimeleap's leaps, and how far a leap reaches.
pub(crate) const BOUNCES: u8 = 5;
pub(crate) const BOUNCE_REACH: f32 = 10.0;
/// The Lich's bolt and his Rimeleap, on their beats.
const BOLT_EVERY: u32 = 5 * HZ / 2;
const CHAIN_EVERY: u32 = 6 * HZ;

/// The Lich's step. False for any other monster.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) -> bool {
    if enemy.kind != EnemyKind::Lich {
        return false;
    }
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    enemy.timer += 1;
    let Some((tx, ty)) = target else {
        return true;
    };
    let (cx, cy) = unit(tx - enemy.x, ty - enemy.y);
    let dist = (tx - enemy.x).hypot(ty - enemy.y);
    // Keep between ten and fourteen steps off, drifting sideways between.
    let side = if enemy.id.is_multiple_of(2) { 1.0 } else { -1.0 };
    let way = if dist < 10.0 {
        (-cx, -cy)
    } else if dist > 14.0 {
        (cx, cy)
    } else {
        (-cy * side, cx * side)
    };
    (enemy.x, enemy.y) = grid.slide(
        (enemy.x, enemy.y),
        (way.0 * 1.8 * DT, way.1 * 1.8 * DT),
        enemy.radius(),
        Mover::Walker,
    );
    let beat = enemy.timer + enemy.id * 13;
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    // His bolt is plain cold: only Rimeleap chills, and leaps.
    let (kind, speed, damage, bounces) = if beat % CHAIN_EVERY == CHAIN_EVERY / 2 {
        deeds.cues.push("rimeleap");
        (Shot::Frost, 5.0, 16, BOUNCES)
    } else if beat.is_multiple_of(BOLT_EVERY) {
        (Shot::Orb, 8.0, 10, 0)
    } else {
        return true;
    };
    shots.push(Projectile {
        x: enemy.x,
        y: enemy.y,
        vx: aim.cos() * speed,
        vy: aim.sin() * speed,
        hostile: true,
        look: None,
        kind,
        damage,
        pierce: bounces,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 6 * HZ,
    });
    true
}

/// Where Rimeleap leaps from a knight it just struck at `(x, y)`: the
/// nearest other knight standing within reach, if any.
pub(super) fn next_link(
    hands: &[(u32, f32, f32)],
    struck: u32,
    (x, y): (f32, f32),
) -> Option<(u32, f32, f32)> {
    hands
        .iter()
        .copied()
        .filter(|&(id, hx, hy)| id != struck && (hx - x).hypot(hy - y) < BOUNCE_REACH)
        .min_by(|a, b| {
            (a.1 - x)
                .hypot(a.2 - y)
                .total_cmp(&(b.1 - x).hypot(b.2 - y))
        })
}

impl Run {
    /// Chill wears off.
    pub(super) fn tick_chill(&mut self) {
        for hero in self.players.values_mut() {
            hero.chilled = hero.chilled.saturating_sub(1);
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_lich__tests.rs"]
mod tests;
