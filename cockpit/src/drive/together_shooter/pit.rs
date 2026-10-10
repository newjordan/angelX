//! The Pit: on a floor or two below the first, a dead
//! end off a fight room opens on a sunken pit, and the Pit Tyrant sleeps in
//! it. Nobody has to go in. Whoever does meets a huge, slow beast that
//! breathes embers and slams the ground round itself (a red ring gathers
//! first; step out of it). Felled, it leaves the Talisman: a knight who carries
//! it, falling, rises at once, whole.

use super::foes::Deeds;
use super::*;

/// A slam gathering: where, and ticks until it lands.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Slam {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) left: u32,
}

/// The slam's wind-up, its reach and its weight; how often it comes (and
/// how often once the beast is below a third of its health).
pub(crate) const SLAM_WINDUP: u32 = 30;
pub(crate) const SLAM_REACH: f32 = 4.5;
const SLAM_DAMAGE: u32 = 35;
const SLAM_EVERY: u32 = 9 * HZ / 2;
const SLAM_EVERY_ENRAGED: u32 = 3 * HZ;

/// The Pit Tyrant's step. False for any other monster.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    shots: &mut Vec<Projectile>,
    deeds: &mut Deeds,
) -> bool {
    if enemy.kind != EnemyKind::PitTyrant {
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
    let enraged = enemy.hp * 3 < enemy.max_hp;
    let every = if enraged {
        SLAM_EVERY_ENRAGED
    } else {
        SLAM_EVERY
    };
    if enemy.stage == 0 {
        (enemy.x, enemy.y) = grid.slide(
            (enemy.x, enemy.y),
            (cx * 1.6 * DT, cy * 1.6 * DT),
            enemy.radius(),
            Mover::Walker,
        );
        if enemy.timer >= every && target.is_some() && dist < SLAM_REACH + 5.0 {
            (enemy.stage, enemy.timer) = (1, 0);
            deeds.slams.push((enemy.x, enemy.y));
            deeds.cues.push("pit_slam");
        }
    } else if enemy.timer >= SLAM_WINDUP {
        (enemy.stage, enemy.timer) = (0, 0);
    }
    // Its breath: a fan of embers at the nearest knight.
    let beat = enemy.timer + enemy.id * 7;
    if enemy.stage == 0 && beat % 90 == 45 && target.is_some() {
        let aim = (ty - enemy.y).atan2(tx - enemy.x);
        for k in -2..=2 {
            let angle = aim + k as f32 * 0.18;
            shots.push(Projectile {
                x: enemy.x,
                y: enemy.y,
                vx: angle.cos() * 7.0,
                vy: angle.sin() * 7.0,
                hostile: true,
                look: None,
                kind: Shot::Ember,
                damage: 14,
                pierce: 0,
                last_hit: None,
                empowered: false,
                traits: Default::default(),
                ttl: 8 * HZ,
            });
        }
    }
    true
}

/// The Talisman, as the run's book holds it: taken, it mends its knight
/// whole and stays with them until they fall.
pub(crate) fn talisman_card() -> Card {
    let raw = "name The Talisman
kind take
rarity relic
by the Pit
text Fall, and rise again at once, whole. Once.
drop 0
chest 0
heal 200
art
..66666666..
.6555555556.
.6555@@5556.
.655@@@@556.
.6555@@5556.
.6555555556.
..65555556..
...655556...
....6556....
.....66.....
";
    cards::check("talisman", raw)
        .expect("the Talisman passes its own checker")
        .card
}

impl Run {
    /// Slams gather and land: a knight in the ring is struck and thrown
    /// clear.
    pub(super) fn tick_slams(&mut self, slams: Vec<(f32, f32)>) {
        for (x, y) in slams {
            if self.slams.len() < 4 {
                self.slams.push(Slam {
                    x,
                    y,
                    left: SLAM_WINDUP,
                });
            }
        }
        let mut landed = Vec::new();
        for slam in &mut self.slams {
            slam.left = slam.left.saturating_sub(1);
            if slam.left == 0 {
                landed.push((slam.x, slam.y));
            }
        }
        self.slams.retain(|s| s.left > 0);
        for (x, y) in landed {
            let room = &self.dungeon.rooms[self.at];
            let grid = Grid { room, barred: true };
            for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                let d = (hero.x - x).hypot(hero.y - y);
                if d < SLAM_REACH && hero.invulnerable == 0 {
                    hero.hurt(SLAM_DAMAGE);
                    self.room_hurt = true;
                    let (ux, uy) = unit(hero.x - x, hero.y - y);
                    (hero.x, hero.y) = grid.slide(
                        (hero.x, hero.y),
                        (ux * (SLAM_REACH - d + 1.0), uy * (SLAM_REACH - d + 1.0)),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                }
            }
            self.shake = self.shake.max(14);
            self.sounds.push("boss_fall");
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
        }
    }

    /// The Pit Tyrant felled: the Talisman where it fell, a ring of gold, and a
    /// prize from the chest's own book.
    pub(super) fn pit_fallen(&mut self, fallen: &[(EnemyKind, f32, f32)]) {
        for &(_, x, y) in fallen.iter().filter(|f| f.0 == EnemyKind::PitTyrant) {
            self.book.insert(talisman_card());
            self.drop_item("talisman".into(), x, y, None);
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                self.drop_item("gold".into(), x + a.cos() * 3.0, y + a.sin() * 2.2, None);
            }
            let pack = self.dungeon.pack;
            if let Some(card) = self.book.roll_chest(&mut self.rng, pack).into_iter().next() {
                self.drop_item(card, x, y + 3.0, None);
            }
            self.shake = self.shake.max(30);
            self.sounds.push("boss_fall");
            self.cues.push("pit_fall".into());
            self.notice("pit_tyrant");
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_pit__tests.rs"]
mod tests;
