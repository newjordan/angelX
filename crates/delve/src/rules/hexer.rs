//! The Hexer: a hooded witch with a
//! frog-green orb on her staff, keeping her distance. Every few seconds she
//! looses a slow green bolt, and a knight it touches is a frog for a few
//! seconds (a frog in a tiny helmet): it can hop about, and do nothing else.
//! Between bolts she spits. A raised shield turns the bolt as it turns any
//! shot, and once a hex wears off the knight can't be hexed again for a
//! while.

use super::*;

pub const HEX_TICKS: u32 = 5 * HZ / 2;
pub const HEX_IMMUNE: u32 = 3 * HZ;
pub const FROG_PACE: f32 = 0.7;
const HEX_EVERY: u32 = 4 * HZ;
const SPIT_EVERY: u32 = 2 * HZ;
const RIBBIT: u32 = 3;

impl Hero {
    pub fn frog(&self) -> bool {
        self.hexed > HEX_IMMUNE
    }
}

pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    shots: &mut Vec<Projectile>,
) -> bool {
    if enemy.kind != EnemyKind::Hexer {
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
    enemy.timer += 1;
    let side = if enemy.id.is_multiple_of(2) { 1.0 } else { -1.0 };
    let way = if target.is_none() {
        (0.0, 0.0)
    } else if dist < 9.0 {
        (-cx, -cy)
    } else if dist > 13.0 {
        (cx, cy)
    } else {
        (-cy * side, cx * side)
    };
    (enemy.x, enemy.y) = grid.slide(
        (enemy.x, enemy.y),
        (way.0 * 2.2 * DT, way.1 * 2.2 * DT),
        enemy.radius(),
        Mover::Walker,
    );
    if target.is_none() {
        return true;
    }
    let beat = enemy.timer + enemy.id * 11;
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    let (kind, speed, damage) = if beat % HEX_EVERY == HEX_EVERY / 2 {
        (Shot::Hex, 6.0, 0)
    } else if beat.is_multiple_of(SPIT_EVERY) {
        (Shot::Orb, 7.0, 10)
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
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 6 * HZ,
    });
    true
}

impl Run {
    pub(super) fn hex(&mut self, id: u32) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        if hero.hexed > 0 || hero.immune > 0 {
            return;
        }
        hero.hexed = HEX_TICKS + HEX_IMMUNE;
        (hero.shielding, hero.walling, hero.dash_ticks) = (false, false, 0);
        self.hexes += 1;
        self.cues.push("hexed".into());
        self.sounds.push("mana_empty");
        self.found = Some((self.tick, id, "Ribbit.".into()));
        if self.hexes >= RIBBIT {
            self.notice("ribbit");
        }
    }

    pub(super) fn tick_hexes(&mut self) {
        for hero in self.players.values_mut() {
            hero.hexed = hero.hexed.saturating_sub(1);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_hexer__tests.rs"]
mod tests;
