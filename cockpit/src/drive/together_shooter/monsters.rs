//! The dungeon's monsters and demons, and how each one moves and fights —
//! a guardian's from its `.boss` file.

use super::*;

/// The dungeon's monsters and demons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EnemyKind {
    /// Erratic flier that bites.
    Bat,
    /// Wandering archer: single aimed bones.
    Skeleton,
    /// Drifting shade: slow rings of spirit orbs.
    Wraith,
    /// Small fire demon: chases, spits three embers.
    Imp,
    /// Greater demon: fans of fire and the occasional ring.
    Demon,
    /// The lair's master.
    Dragon,
    /// A floor's guardian, drawn and armed by its `.boss` file.
    Boss,
    /// A treasure chest with teeth: hops, bites, spits coins.
    Mimic,
}

impl EnemyKind {
    pub(crate) fn radius(self) -> f32 {
        match self {
            EnemyKind::Bat | EnemyKind::Imp => 0.6,
            EnemyKind::Skeleton => 0.7,
            EnemyKind::Wraith => 0.8,
            EnemyKind::Demon => 1.2,
            EnemyKind::Dragon => 1.8,
            EnemyKind::Boss => 1.4,
            EnemyKind::Mimic => 0.9,
        }
    }

    pub(super) fn hp(self) -> u32 {
        match self {
            EnemyKind::Bat => 30,
            EnemyKind::Skeleton => 70,
            EnemyKind::Wraith => 80,
            EnemyKind::Imp => 50,
            EnemyKind::Demon => 220,
            EnemyKind::Dragon => 1100,
            EnemyKind::Boss => 600,
            EnemyKind::Mimic => 220,
        }
    }

    pub(crate) fn flies(self) -> bool {
        matches!(self, EnemyKind::Bat | EnemyKind::Wraith | EnemyKind::Dragon)
    }

    pub(super) fn bounty(self) -> u32 {
        match self {
            EnemyKind::Bat => 50,
            EnemyKind::Skeleton | EnemyKind::Imp => 100,
            EnemyKind::Wraith => 120,
            EnemyKind::Demon => 300,
            EnemyKind::Dragon => 2000,
            EnemyKind::Boss => 800,
            EnemyKind::Mimic => 250,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Enemy {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) hp: u32,
    pub(crate) max_hp: u32,
    pub(crate) kind: EnemyKind,
    pub(crate) age: u32,
    pub(super) id: u32,
    pub(super) origin_x: f32,
    pub(super) origin_y: f32,
    /// A boss's index in the run's `bosses`.
    #[serde(default)]
    pub(crate) boss: Option<u8>,
    #[serde(default)]
    pub(super) size: Option<f32>,
    #[serde(default)]
    pub(super) base_hp: Option<u32>,
    /// A guardian past its rage threshold.
    #[serde(default)]
    pub(super) raged: bool,
}

impl Enemy {
    pub(crate) fn radius(&self) -> f32 {
        self.size.unwrap_or_else(|| self.kind.radius())
    }

    pub(super) fn base_hp(&self) -> u32 {
        self.base_hp.unwrap_or_else(|| self.kind.hp())
    }
}

/// One monster's step: move, then maybe shoot.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    bosses: &[Boss],
    shots: &mut Vec<Projectile>,
) {
    if let Some(boss) = enemy.boss.and_then(|i| bosses.get(usize::from(i))) {
        act_boss(enemy, boss, heroes, grid, seed, shots);
        return;
    }
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let (tx, ty) = target.unwrap_or((enemy.x, enemy.y));
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    let (cx, cy) = unit(tx - enemy.x, ty - enemy.y);
    let age = enemy.age + enemy.id * 13;
    let spin = age as f32 * 0.021 + (seed % 31) as f32 * 0.1;
    let mover = if enemy.kind.flies() {
        Mover::Flier
    } else {
        Mover::Walker
    };
    let step = |speed: f32, (dx, dy): (f32, f32)| (dx * speed * DT, dy * speed * DT);
    let motion = match enemy.kind {
        EnemyKind::Bat => {
            let wobble = (age as f32 * 0.31).sin() * 0.9;
            step(6.5, unit(cx - cy * wobble, cy + cx * wobble))
        }
        EnemyKind::Skeleton => {
            // Wander between points near home, re-chosen every three seconds.
            let h = mix(seed ^ u64::from(enemy.id) << 20 ^ u64::from(age / 90));
            let gx = enemy.origin_x + ((h & 0xff) as f32 / 255.0 - 0.5) * 12.0;
            let gy = enemy.origin_y + (((h >> 8) & 0xff) as f32 / 255.0 - 0.5) * 8.0;
            step(2.2, unit(gx - enemy.x, gy - enemy.y))
        }
        EnemyKind::Wraith => step(1.8, (cx, cy)),
        EnemyKind::Imp => step(4.5, (cx, cy)),
        EnemyKind::Demon => step(1.8, (cx, cy)),
        EnemyKind::Dragon => {
            let gx = enemy.origin_x + (age as f32 * 0.02).sin() * 12.0;
            let gy = enemy.origin_y + (age as f32 * 0.014).sin() * 2.0;
            (gx - enemy.x, gy - enemy.y)
        }
        EnemyKind::Boss => (0.0, 0.0),
        // Hops: a lunge, then a pause to chew.
        EnemyKind::Mimic if age % 36 < 14 => step(7.5, (cx, cy)),
        EnemyKind::Mimic => (0.0, 0.0),
    };
    (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), motion, enemy.radius(), mover);
    let mut fire = |angle: f32, speed: f32, kind: Shot, damage: u32| {
        shots.push(Projectile {
            x: enemy.x,
            y: enemy.y,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed,
            hostile: true,
            look: None,
            kind,
            damage,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: Default::default(),
            ttl: 8 * HZ,
        });
    };
    match enemy.kind {
        EnemyKind::Bat => {}
        EnemyKind::Skeleton if age.is_multiple_of(55) => fire(aim, 9.0, Shot::Bone, 12),
        EnemyKind::Wraith if age.is_multiple_of(80) => {
            for i in 0..10 {
                fire(
                    spin + i as f32 * std::f32::consts::TAU / 10.0,
                    5.0,
                    Shot::Orb,
                    12,
                );
            }
        }
        EnemyKind::Imp if age.is_multiple_of(70) => {
            for i in -1..=1 {
                fire(aim + i as f32 * 0.22, 7.0, Shot::Ember, 12);
            }
        }
        EnemyKind::Demon => {
            if age.is_multiple_of(60) {
                for i in -2..=2 {
                    fire(aim + i as f32 * 0.16, 7.0, Shot::Ember, 16);
                }
            }
            if age % 150 == 75 {
                for i in 0..12 {
                    fire(
                        spin + i as f32 * std::f32::consts::TAU / 12.0,
                        4.5,
                        Shot::Orb,
                        16,
                    );
                }
            }
        }
        EnemyKind::Mimic if age.is_multiple_of(80) => {
            // It spits the coins it was hiding.
            for i in -2..=2 {
                fire(aim + i as f32 * 0.2, 7.0, Shot::Ball, 10);
            }
        }
        EnemyKind::Dragon => {
            if age.is_multiple_of(8) {
                for i in 0..3 {
                    fire(
                        spin * 2.0 + i as f32 * std::f32::consts::TAU / 3.0,
                        6.0,
                        Shot::Ember,
                        16,
                    );
                }
            }
            if age.is_multiple_of(75) {
                for i in -3..=3 {
                    fire(aim + i as f32 * 0.13, 8.0, Shot::Ember, 16);
                }
            }
        }
        _ => {}
    }
}

/// A guardian's step: its file's movement, then each of its attacks on their
/// own beat, half again as often once its health falls below its rage.
fn act_boss(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    shots: &mut Vec<Projectile>,
) {
    use bosses::{Bolt, Move, Pattern};
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let (tx, ty) = target.unwrap_or((enemy.x, enemy.y + 1.0));
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    let age = enemy.age;
    let spin = age as f32 * 0.035 + (seed % 17) as f32 * 0.1;
    let toward = |gx: f32, gy: f32, speed: f32, enemy: &Enemy| {
        let (dx, dy) = (gx - enemy.x, gy - enemy.y);
        let (ux, uy) = unit(dx, dy);
        let reach = dx.hypot(dy).min(speed * DT);
        (ux * reach, uy * reach)
    };
    let motion = match boss.moves {
        Move::Chase => toward(tx, ty, boss.speed, enemy),
        Move::Drift => {
            let h = mix(seed ^ u64::from(enemy.id) << 20 ^ u64::from(age / 120));
            let gx = enemy.origin_x + ((h & 0xff) as f32 / 255.0 - 0.5) * 18.0;
            let gy = enemy.origin_y + (((h >> 8) & 0xff) as f32 / 255.0) * 9.0;
            toward(gx, gy, boss.speed, enemy)
        }
        Move::Hover => {
            let gx = grid.room.width() / 2.0 + (age as f32 * 0.012).sin() * 16.0;
            let gy = enemy.origin_y + (age as f32 * 0.03).sin() * 1.5;
            toward(gx, gy, boss.speed.max(1.0), enemy)
        }
        Move::Anchor => (0.0, 0.0),
    };
    let mover = if boss.moves == Move::Hover {
        Mover::Flier
    } else {
        Mover::Walker
    };
    (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), motion, enemy.radius(), mover);
    let raging = boss.rage > 0.0 && (enemy.hp as f32) < boss.rage * enemy.max_hp as f32;
    for (index, attack) in boss.attacks.iter().enumerate() {
        let every = if raging {
            (attack.every * 2 / 3).max(4)
        } else {
            attack.every.max(4)
        };
        if !(age + index as u32 * 11).is_multiple_of(every) {
            continue;
        }
        let kind = match attack.bolt {
            Bolt::Bone => Shot::Bone,
            Bolt::Orb => Shot::Orb,
            Bolt::Ember => Shot::Ember,
        };
        let n = attack.shots.max(1);
        let angles: Vec<f32> = match attack.pattern {
            Pattern::Aimed => (0..n)
                .map(|i| aim + (i as f32 - (n - 1) as f32 / 2.0) * 0.06)
                .collect(),
            Pattern::Fan => {
                let arc = attack.arc.to_radians();
                (0..n)
                    .map(|i| aim + arc * (i as f32 / (n - 1).max(1) as f32 - 0.5))
                    .collect()
            }
            Pattern::Ring => (0..n)
                .map(|i| spin + i as f32 * std::f32::consts::TAU / n as f32)
                .collect(),
            Pattern::Spiral => (0..n)
                .map(|i| spin * 3.0 + i as f32 * std::f32::consts::TAU / n as f32)
                .collect(),
        };
        for angle in angles {
            shots.push(Projectile {
                x: enemy.x,
                y: enemy.y,
                vx: angle.cos() * attack.speed,
                vy: angle.sin() * attack.speed,
                hostile: true,
                look: None,
                kind,
                damage: attack.damage,
                pierce: 0,
                last_hit: None,
                empowered: false,
                traits: Default::default(),
                ttl: 8 * HZ,
            });
        }
    }
}
