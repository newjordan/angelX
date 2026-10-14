//! The two unruly guardians' bounded, deterministic stage machines.
use super::*;
use foes::Deeds;

pub const RIPPER: &str = "the-big-ripper";
pub const ENGINE: &str = "grinwick";
pub const INHALE: u8 = 1;
pub const TELL: u8 = 2;
pub const DASH: u8 = 3;
pub const DAZED: u8 = 4;
pub const JUMP: u8 = 5;
pub const BREATH: u32 = 24;
pub const RIP_TELL: u32 = 21;
pub const RAIL_TELL: u32 = 30;
pub const RIP_SPEED: f32 = 40.0;
pub const PULL_SPEED: f32 = 1.8;
pub const PULL_REACH: f32 = 11.0;
pub const WALL_AREA: f32 = 160.0;
pub const WALL_MAX: usize = 32;
pub const TENDER: u8 = 9;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rail {
    pub a: (f32, f32),
    pub b: (f32, f32),
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub active: bool,
    pub cycle: u32,
    pub left: u8,
    pub bounces: u8,
    pub rails: Vec<Rail>,
    pub rail: u8,
    pub jump_from: (f32, f32),
    pub babies: bool,
}
impl State {
    pub fn quiet(&self) -> bool {
        self == &Self::default()
    }
    pub fn valid(&self) -> bool {
        self.left <= 3
            && self.bounces <= 2
            && self.rail <= 2
            && self.rails.len() <= 3
            && self
                .rails
                .iter()
                .all(|r| [r.a.0, r.a.1, r.b.0, r.b.1].into_iter().all(f32::is_finite))
            && self.jump_from.0.is_finite()
            && self.jump_from.1.is_finite()
    }
}
pub fn special(boss: &Boss) -> bool {
    matches!(boss.id.as_str(), RIPPER | ENGINE)
}
pub fn shooting(enemy: &Enemy, boss: &Boss) -> bool {
    !special(boss) || enemy.stage == 0
}
pub fn swell(enemy: &Enemy) -> f32 {
    match enemy.stage {
        INHALE => match enemy.timer / BREATH {
            0 => 1.0,
            1 => 1.35,
            2 => 1.7,
            _ => 2.0,
        },
        TELL => 2.0,
        DASH => 2.0 - (enemy.timer as f32 / 33.0).min(1.0),
        _ => 1.0,
    }
}
/// A seeded candidate with the longest clear launch corridor. Sampling all
/// quadrants favours open room without turning the rip into aimed pursuit.
pub(super) fn rip_direction(enemy: &Enemy, grid: &Grid, seed: u64) -> (f32, f32) {
    let salt = mix(seed ^ u64::from(enemy.id) << 32 ^ u64::from(enemy.signature.cycle));
    let start = (salt % 65536) as f32 / 65536.0 * std::f32::consts::TAU;
    (0..8)
        .map(|k| {
            let angle = start + k as f32 * std::f32::consts::TAU / 8.0;
            let dir = (angle.cos(), angle.sin());
            let reach = (1..=16)
                .take_while(|&n| {
                    grid.clear(
                        enemy.x + dir.0 * n as f32,
                        enemy.y + dir.1 * n as f32,
                        enemy.radius(),
                        Mover::Walker,
                    )
                })
                .count();
            (reach, dir)
        })
        .max_by_key(|(reach, _)| *reach)
        .map(|(_, d)| d)
        .unwrap_or((1.0, 0.0))
}
fn stage(enemy: &mut Enemy, next: u8) {
    enemy.stage = next;
    enemy.timer = 0;
}
fn begin_rip(enemy: &mut Enemy, grid: &Grid, seed: u64) {
    enemy.signature.cycle = enemy.signature.cycle.wrapping_add(1);
    enemy.dir = rip_direction(enemy, grid, seed);
    enemy.signature.bounces = 0;
    stage(enemy, TELL);
}
fn dash(enemy: &mut Enemy, grid: &Grid, speed: f32, bounce: bool, deeds: &mut Deeds) -> bool {
    let from = (enemy.x, enemy.y);
    // Small collision steps leave a continuous trail and cannot tunnel stone.
    let stride = speed * DT / 4.0;
    let mut stopped = false;
    for _ in 0..4 {
        let next = (
            enemy.x + enemy.dir.0 * stride,
            enemy.y + enemy.dir.1 * stride,
        );
        if grid.clear(next.0, next.1, enemy.radius(), Mover::Walker) {
            (enemy.x, enemy.y) = next;
        } else if bounce && enemy.signature.bounces < 2 {
            let x_blocked = !grid.clear(next.0, enemy.y, enemy.radius(), Mover::Walker);
            let y_blocked = !grid.clear(enemy.x, next.1, enemy.radius(), Mover::Walker);
            if x_blocked {
                enemy.dir.0 = -enemy.dir.0;
            }
            if y_blocked {
                enemy.dir.1 = -enemy.dir.1;
            }
            if !x_blocked && !y_blocked {
                enemy.dir = (-enemy.dir.0, -enemy.dir.1);
            }
            enemy.signature.bounces += 1;
        } else {
            stopped = true;
            break;
        }
    }
    deeds.sweeps.push((
        from,
        (enemy.x, enemy.y),
        enemy.radius() + HERO_RADIUS,
        24,
        enemy.dir,
    ));
    stopped
}
pub(super) fn act(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    deeds: &mut Deeds,
) -> bool {
    if !special(boss) {
        return false;
    }
    enemy.signature.active = true;
    enemy.timer += 1;
    if boss.id == RIPPER {
        let raging = (enemy.hp as f32) < enemy.max_hp as f32 * 0.4;
        let radius = boss.radius * swell(enemy);
        // Each breath pushes his growing belly clear of nearby stone.
        if (radius > enemy.radius() || enemy.stage == TELL)
            && !grid.clear(enemy.x, enemy.y, radius, Mover::Walker)
        {
            let mut best = None;
            for dy in -8..=8 {
                for dx in -8..=8 {
                    let spot = (enemy.x + dx as f32 * 0.5, enemy.y + dy as f32 * 0.5);
                    if grid.clear(spot.0, spot.1, radius, Mover::Walker) {
                        let distance = dx * dx + dy * dy;
                        if best.is_none_or(|(d, _)| distance < d) {
                            best = Some((distance, spot));
                        }
                    }
                }
            }
            if let Some((_, spot)) = best {
                (enemy.x, enemy.y) = spot;
            }
        }
        enemy.size = Some(radius);
        if enemy.stage == 0 {
            let (ux, uy) = unit(
                grid.room.width() / 2.0 - enemy.x,
                grid.room.height() / 2.0 - enemy.y,
            );
            (enemy.x, enemy.y) = grid.slide(
                (enemy.x, enemy.y),
                (ux * 3.0 * DT, uy * 3.0 * DT),
                enemy.radius(),
                Mover::Walker,
            );
        }
        match enemy.stage {
            0 if enemy.timer >= 72 => {
                stage(enemy, INHALE);
            }
            INHALE => {
                deeds.pulls.push((enemy.x, enemy.y));
                if enemy.timer >= BREATH * 3 {
                    enemy.size = Some(boss.radius * 2.0);
                    enemy.signature.left = if raging { 3 } else { 1 };
                    begin_rip(enemy, grid, seed);
                }
            }
            TELL if enemy.timer >= RIP_TELL => {
                stage(enemy, DASH);
                deeds.sounds.push("gas_release");
                deeds.shake = 12;
            }
            DASH => {
                let stop = dash(enemy, grid, RIP_SPEED, true, deeds);
                deeds.clouds.push(skills::Cloud {
                    x: enemy.x,
                    y: enemy.y,
                    vx: 0.0,
                    vy: 0.0,
                    age: 0,
                    owner: 0,
                    power: 4,
                    tint: if raging { 3 } else { 2 },
                });
                if stop || enemy.timer >= 33 {
                    enemy.signature.left = enemy.signature.left.saturating_sub(1);
                    if enemy.signature.left > 0 {
                        enemy.size = Some(boss.radius * 2.0);
                        begin_rip(enemy, grid, seed);
                    } else {
                        stage(enemy, DAZED);
                    }
                }
            }
            DAZED if enemy.timer >= 42 => {
                stage(enemy, 0);
            }
            _ => {}
        }
    } else {
        engine(enemy, boss, heroes, grid, seed, deeds);
    }
    true
}
fn lay_rails(enemy: &mut Enemy, heroes: &[(f32, f32)], grid: &Grid, seed: u64) {
    let target = heroes
        .first()
        .copied()
        .unwrap_or((grid.room.width() / 2.0, grid.room.height() / 2.0));
    let aim = (target.1 - enemy.y).atan2(target.0 - enemy.x);
    let jitter = (mix(seed ^ u64::from(enemy.signature.cycle)) % 100) as f32 * 0.004 - 0.2;
    enemy.signature.rails.clear();
    for k in 0..3 {
        let a = (enemy.x, enemy.y);
        let angle = aim + jitter + (k as f32 - 1.0) * 0.72;
        let dir = (angle.cos(), angle.sin());
        // Rails follow traversable floor, ending before a wall or pillar.
        let mut b = a;
        for n in 1..=120 {
            let p = (a.0 + dir.0 * n as f32 * 0.5, a.1 + dir.1 * n as f32 * 0.5);
            if !grid.clear(p.0, p.1, enemy.radius(), Mover::Walker) {
                break;
            }
            b = p;
        }
        enemy.signature.rails.push(Rail { a, b });
    }
    // A larger engine can leave a side branch very short beside a wall.
    // Start on the longest clear corridor; each later branch is still warned.
    let reach = |r: &Rail| (r.b.0 - r.a.0).hypot(r.b.1 - r.a.1);
    enemy
        .signature
        .rails
        .sort_by(|a, b| reach(b).total_cmp(&reach(a)));
    enemy.signature.rail = 0;
    enemy.signature.left = if enemy.hp * 2 < enemy.max_hp { 3 } else { 1 };
    enemy.signature.cycle = enemy.signature.cycle.wrapping_add(1);
    stage(enemy, TELL);
}
fn engine(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    deeds: &mut Deeds,
) {
    if enemy.hp * 2 < enemy.max_hp && !enemy.signature.babies {
        enemy.signature.babies = true;
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_2;
            deeds.spawns.push((
                EnemyKind::Spiderling,
                enemy.x + a.cos() * (enemy.radius() + 2.0),
                enemy.y + a.sin() * (enemy.radius() + 2.0),
                TENDER,
            ));
        }
    }
    match enemy.stage {
        0 => {
            let (tx, ty) = heroes.first().copied().unwrap_or((enemy.x, enemy.y));
            let (ux, uy) = unit(tx - enemy.x, ty - enemy.y);
            let wobble = (enemy.timer as f32 * 0.3).sin() * 0.45;
            enemy.dir = unit(ux - uy * wobble, uy + ux * wobble);
            (enemy.x, enemy.y) = grid.slide(
                (enemy.x, enemy.y),
                (enemy.dir.0 * boss.speed * DT, enemy.dir.1 * boss.speed * DT),
                enemy.radius(),
                Mover::Walker,
            );
            if enemy.timer == 36 {
                deeds.slams.push((enemy.x, enemy.y));
            }
            if enemy.timer >= 105 {
                lay_rails(enemy, heroes, grid, seed);
                deeds.sounds.push("engine_whistle");
            }
        }
        TELL if enemy.timer >= RAIL_TELL => {
            if let Some(r) = enemy.signature.rails.get(enemy.signature.rail as usize) {
                enemy.dir = unit(r.b.0 - r.a.0, r.b.1 - r.a.1);
            }
            stage(enemy, DASH);
            deeds.shake = 8;
        }
        DASH => {
            let stopped = dash(enemy, grid, 38.0, false, deeds);
            let arrived = enemy
                .signature
                .rails
                .get(enemy.signature.rail as usize)
                .is_none_or(|r| (r.b.0 - enemy.x).hypot(r.b.1 - enemy.y) < 1.4);
            if stopped || arrived || enemy.timer >= 45 {
                enemy.signature.left = enemy.signature.left.saturating_sub(1);
                if enemy.signature.left > 0 {
                    enemy.signature.rail = (enemy.signature.rail + 1).min(2);
                    enemy.signature.jump_from = (enemy.x, enemy.y);
                    stage(enemy, JUMP);
                } else {
                    stage(enemy, DAZED);
                }
            }
        }
        JUMP => {
            if let Some(r) = enemy.signature.rails.get(enemy.signature.rail as usize) {
                let t = (enemy.timer as f32 / 24.0).min(1.0);
                enemy.x = enemy.signature.jump_from.0 + (r.a.0 - enemy.signature.jump_from.0) * t;
                enemy.y = enemy.signature.jump_from.1 + (r.a.1 - enemy.signature.jump_from.1) * t;
            }
            if enemy.timer >= 24 {
                stage(enemy, TELL);
                deeds.sounds.push("engine_whistle");
            }
        }
        DAZED if enemy.timer >= 36 => {
            enemy.signature.rails.clear();
            stage(enemy, 0);
        }
        _ => {}
    }
}
pub fn rear_bonus(enemy: &Enemy, bosses: &[Boss], from: (f32, f32), damage: u32) -> u32 {
    if enemy.stage == DAZED
        && enemy
            .boss
            .and_then(|i| bosses.get(i as usize))
            .is_some_and(|b| b.id == RIPPER)
        && (from.0 - enemy.x) * enemy.dir.0 + (from.1 - enemy.y) * enemy.dir.1 < 0.0
    {
        damage / 2
    } else {
        0
    }
}
/// Gas trail area is bounded by the sum of whole segment disks, so overlaps
/// can only reduce coverage. Oldest walls yield first; player gas stays put.
pub fn add_wall(clouds: &mut Vec<skills::Cloud>, cloud: skills::Cloud) {
    let area = |c: &skills::Cloud| std::f32::consts::PI * c.wall_radius().powi(2);
    while clouds.iter().filter(|c| c.wall()).count() >= WALL_MAX
        || clouds.iter().filter(|c| c.wall()).map(area).sum::<f32>() + area(&cloud) > WALL_AREA
        || clouds.len() >= 48
    {
        let Some(i) = clouds.iter().position(skills::Cloud::wall) else {
            return;
        };
        clouds.remove(i);
    }
    clouds.push(cloud);
}
impl Run {
    pub(super) fn signature_deeds(
        &mut self,
        clouds: Vec<skills::Cloud>,
        pulls: Vec<(f32, f32)>,
        sweeps: Vec<((f32, f32), (f32, f32), f32, u32, (f32, f32))>,
    ) {
        for cloud in clouds {
            add_wall(&mut self.works.clouds, cloud);
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        for (x, y) in pulls {
            for h in self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && h.rolling().is_none())
            {
                let d = (h.x - x).hypot(h.y - y);
                if d < PULL_REACH && d > 0.1 {
                    let (ux, uy) = unit(x - h.x, y - h.y);
                    (h.x, h.y) = grid.slide(
                        (h.x, h.y),
                        (ux * PULL_SPEED * DT, uy * PULL_SPEED * DT),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                }
            }
            for p in &mut self.projectiles {
                let d = (p.x - x).hypot(p.y - y);
                if d < PULL_REACH && d > 0.1 {
                    let (ux, uy) = unit(x - p.x, y - p.y);
                    p.vx += ux * 2.4 * DT;
                    p.vy += uy * 2.4 * DT;
                }
            }
        }
        for (a, b, r, damage, _) in sweeps {
            for h in self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && h.invulnerable == 0)
            {
                if hit_segment(a.0, a.1, b.0, b.1, h.x, h.y, r) {
                    h.hurt(damage);
                    self.sounds.push("hero_hurt");
                }
            }
        }
    }
}
#[cfg(test)]
#[path = "../../tests/rules/together_signatures__tests.rs"]
mod tests;
