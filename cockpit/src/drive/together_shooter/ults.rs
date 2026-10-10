//! Each knight's ultimate: one signature power, charged by fighting and cast
//! with R.
//!
//! The charge fills a little every tick of a fight and a lot for every
//! point of damage the knight deals. A full charge casts once and empties.
//! Every knight of the company carries their own:
//!
//! | Knight | Ultimate |
//! |---|---|
//! | Percival | Pilgrim's Arrow: one great arrow through everything, stunning |
//! | Lynette | Lady's Veil: the whole party untouchable for four seconds |
//! | Gareth | Trebuchet: two volleys of shells where he aims |
//! | Galahad | Bladewind: blinks from monster to monster, cutting |
//! | the Composer | Grand Chord: pulls every monster near into a stunned knot |
//! | the Dispatcher | Sealed Writs: one each for the nearest six |
//! | the Loop Knight | Stillhour: time stops in a sphere, for monsters and their shots |
//! | the Money Knight | Midas Touch: the nearest monsters turn to gold |
//! | the Competition Knight | Assassinate: a second's aim, then the room's toughest monster takes it |
//! | the Scryglass Knight | Phantasm: two images of the knight that fire with them and draw fire |

use super::*;

/// A full charge.
pub(crate) const ULT_FULL: u32 = 1000;
/// Charge per tick of a fight (a minute of fighting alone fills it twice).
const PER_TICK: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Ult {
    PilgrimsArrow,
    LadysVeil,
    Trebuchet,
    Bladewind,
    GrandChord,
    SealedWrits,
    Stillhour,
    MidasTouch,
    Assassinate,
    Phantasm,
}

impl Ult {
    pub(crate) const ALL: [Ult; 10] = [
        Ult::PilgrimsArrow,
        Ult::LadysVeil,
        Ult::Trebuchet,
        Ult::Bladewind,
        Ult::GrandChord,
        Ult::SealedWrits,
        Ult::Stillhour,
        Ult::MidasTouch,
        Ult::Assassinate,
        Ult::Phantasm,
    ];

    /// The ultimate a knight of the company carries.
    pub(crate) fn of(knight: Option<&str>) -> Ult {
        match knight.unwrap_or("percival") {
            "lynette" => Ult::LadysVeil,
            "gareth" => Ult::Trebuchet,
            "galahad" => Ult::Bladewind,
            "composer" => Ult::GrandChord,
            "dispatcher" => Ult::SealedWrits,
            "loop" => Ult::Stillhour,
            "money" => Ult::MidasTouch,
            "competition" => Ult::Assassinate,
            "scryglass" => Ult::Phantasm,
            _ => Ult::PilgrimsArrow,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Ult::PilgrimsArrow => "Pilgrim's Arrow",
            Ult::LadysVeil => "Lady's Veil",
            Ult::Trebuchet => "Trebuchet",
            Ult::Bladewind => "Bladewind",
            Ult::GrandChord => "Grand Chord",
            Ult::SealedWrits => "Sealed Writs",
            Ult::Stillhour => "Stillhour",
            Ult::MidasTouch => "Midas Touch",
            Ult::Assassinate => "Assassinate",
            Ult::Phantasm => "Phantasm",
        }
    }

    /// The word in a cue: `ult:pilgrims_arrow`.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Ult::PilgrimsArrow => "pilgrims_arrow",
            Ult::LadysVeil => "ladys_veil",
            Ult::Trebuchet => "trebuchet",
            Ult::Bladewind => "bladewind",
            Ult::GrandChord => "grand_chord",
            Ult::SealedWrits => "sealed_writs",
            Ult::Stillhour => "stillhour",
            Ult::MidasTouch => "midas_touch",
            Ult::Assassinate => "assassinate",
            Ult::Phantasm => "phantasm",
        }
    }
}

/// A shell of Trebuchet on its way: where it lands, ticks until it does.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Strike {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) fall: u32,
    pub(crate) damage: u32,
    pub(crate) owner: u32,
}

/// Trebuchet's shells land this far apart in time, and burst this wide.
pub(crate) const STRIKE_FALL: u32 = 36;
const STRIKE_REACH: f32 = 3.2;

/// A Stillhour: time stands still inside it for monsters and their shots.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Sphere {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) r: f32,
    pub(crate) left: u32,
}

/// An image of a knight from Phantasm: it keeps its place beside them,
/// fires with them, and draws the monsters' eyes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Phantom {
    pub(crate) owner: u32,
    pub(crate) x: f32,
    pub(crate) y: f32,
    /// Its place beside its knight.
    pub(crate) dx: f32,
    pub(crate) dy: f32,
    pub(crate) left: u32,
}

pub(crate) const PHANTOM_TICKS: u32 = 8 * HZ;
/// What a phantom's copy of a shot carries, in percent.
const PHANTOM_SHARE: u32 = 40;

impl Run {
    /// The knights' charges fill while the party fights.
    pub(super) fn charge_ults(&mut self, credits: &[(u32, u32)]) {
        let fighting = self.phase == Phase::Fighting;
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            if fighting {
                hero.ult_charge = (hero.ult_charge + PER_TICK).min(ULT_FULL);
            }
        }
        for &(owner, damage) in credits {
            if let Some(hero) = self.players.get_mut(&owner) {
                hero.ult_charge = (hero.ult_charge + damage / 2).min(ULT_FULL);
            }
        }
    }

    /// Cast each knight's ultimate whose key was pressed on a full charge.
    pub(super) fn cast_ults(&mut self, casts: Vec<u32>) {
        for id in casts {
            let Some(hero) = self.players.get_mut(&id) else {
                continue;
            };
            let ult = hero.ult();
            hero.ult_charge = 0;
            let (hx, hy, ax, ay) = (hero.x, hero.y, hero.aim_x, hero.aim_y);
            let owner = id as u8;
            match ult {
                Ult::PilgrimsArrow => {
                    let damage = hero.scaled(160);
                    self.projectiles.push(Projectile {
                        x: hx + ax * 0.8,
                        y: hy + ay * 0.8,
                        vx: ax * 24.0,
                        vy: ay * 24.0,
                        hostile: false,
                        kind: Shot::Sacred,
                        look: None,
                        damage,
                        pierce: u8::MAX,
                        last_hit: None,
                        empowered: false,
                        ttl: 3 * HZ,
                        traits: ShotTraits {
                            owner,
                            stun: 45,
                            ..ShotTraits::default()
                        },
                    });
                }
                Ult::LadysVeil => {
                    for knight in self.players.values_mut().filter(|h| h.hp > 0) {
                        knight.angel = 4 * HZ;
                        knight.invulnerable = knight.invulnerable.max(4 * HZ);
                        knight.hp = (knight.hp + 15).min(knight.max_hp);
                    }
                }
                Ult::Trebuchet => {
                    let damage = hero.scaled(70);
                    let (w, h) = (self.room().width(), self.room().height());
                    let (tx, ty) = (
                        (hx + ax * 9.0).clamp(3.0, w - 3.0),
                        (hy + ay * 9.0).clamp(3.0, h - 3.0),
                    );
                    for volley in 0..2u32 {
                        for k in 0..4 {
                            let a = k as f32 * std::f32::consts::FRAC_PI_2 + volley as f32 * 0.78;
                            let r = 1.2 + volley as f32 * 1.6;
                            self.strikes.push(Strike {
                                x: (tx + a.cos() * r).clamp(2.5, w - 2.5),
                                y: (ty + a.sin() * r).clamp(2.5, h - 2.5),
                                fall: STRIKE_FALL + volley * 22 + k * 3,
                                damage,
                                owner: id,
                            });
                        }
                    }
                }
                Ult::Bladewind => {
                    hero.slashes = 6;
                    hero.slash_wait = 0;
                    hero.invulnerable = hero.invulnerable.max(2 * HZ);
                }
                Ult::GrandChord => {
                    let damage = hero.scaled(40);
                    let (px, py) = (hx + ax * 3.2, hy + ay * 3.2);
                    let room = &self.dungeon.rooms[self.at];
                    let grid = Grid { room, barred: true };
                    let mut ring = 0;
                    for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                        if (enemy.x - hx).hypot(enemy.y - hy) > 11.0 {
                            continue;
                        }
                        enemy.hp = enemy.hp.saturating_sub(damage);
                        enemy.frozen = enemy.frozen.max(75);
                        let anchored = enemy.boss.is_some()
                            || matches!(enemy.kind, EnemyKind::Dragon | EnemyKind::Dummy);
                        if anchored {
                            continue;
                        }
                        let a = ring as f32 * 1.1;
                        ring += 1;
                        let (nx, ny) = (px + a.cos() * 1.4, py + a.sin() * 1.4);
                        let mover = if enemy.kind.flies() {
                            Mover::Flier
                        } else {
                            Mover::Walker
                        };
                        if grid.clear(nx, ny, enemy.radius(), mover) && self.sparks.len() < 32 {
                            self.sparks.push(Spark {
                                from: (enemy.x, enemy.y),
                                to: (nx, ny),
                                ttl: 8,
                            });
                            (enemy.x, enemy.y) = (nx, ny);
                        }
                    }
                    self.shake = self.shake.max(10);
                }
                Ult::SealedWrits => {
                    let damage = hero.scaled(55);
                    let mut targets: Vec<(f32, f32, f32)> = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0)
                        .map(|e| ((e.x - hx).hypot(e.y - hy), e.x, e.y))
                        .collect();
                    targets.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let aim = ay.atan2(ax);
                    for k in 0..6 {
                        let (dx, dy) = targets
                            .get(k % targets.len().max(1))
                            .map(|&(_, x, y)| (x - hx, y - hy))
                            .unwrap_or((ax, ay));
                        let (ux, uy) = unit(dx, dy);
                        // Each leaves at its own angle, then turns home.
                        let fan = aim + (k as f32 - 2.5) * 0.45;
                        let (vx, vy) = unit(fan.cos() + ux, fan.sin() + uy);
                        self.projectiles.push(Projectile {
                            x: hx,
                            y: hy,
                            vx: vx * 15.0,
                            vy: vy * 15.0,
                            hostile: false,
                            kind: Shot::Missile,
                            look: None,
                            damage,
                            pierce: 0,
                            last_hit: None,
                            empowered: false,
                            ttl: 4 * HZ,
                            traits: ShotTraits {
                                owner,
                                homing: 8,
                                burst: 40,
                                ..ShotTraits::default()
                            },
                        });
                    }
                }
                Ult::Stillhour => {
                    let (w, h) = (self.room().width(), self.room().height());
                    self.spheres.push(Sphere {
                        x: (hx + ax * 4.5).clamp(4.0, w - 4.0),
                        y: (hy + ay * 4.5).clamp(4.0, h - 4.0),
                        r: 6.5,
                        left: 4 * HZ,
                    });
                }
                Ult::MidasTouch => {
                    let mut near: Vec<(f32, usize)> = self
                        .enemies
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.hp > 0 && e.kind != EnemyKind::Dummy)
                        .map(|(i, e)| ((e.x - hx).hypot(e.y - hy), i))
                        .filter(|&(d, _)| d < 13.0)
                        .collect();
                    near.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let mut gilded = Vec::new();
                    for &(_, i) in near.iter().take(3) {
                        let enemy = &mut self.enemies[i];
                        if enemy.boss.is_some() || enemy.kind == EnemyKind::Dragon {
                            enemy.hp = enemy.hp.saturating_sub(enemy.max_hp * 15 / 100);
                        } else {
                            enemy.hp = 0;
                        }
                        gilded.push((enemy.x, enemy.y));
                    }
                    for (x, y) in gilded {
                        if self.blasts.len() < 16 {
                            self.blasts.push((x, y, BLAST_TICKS));
                        }
                        self.drop_item("gold".into(), x, y, None);
                    }
                    self.sounds.push("spoil_pickup");
                }
                Ult::Assassinate => {
                    let target = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0)
                        .max_by_key(|e| (e.boss.is_some(), e.hp))
                        .map(|e| e.id);
                    hero.aiming = target.map(|t| (t, HZ));
                }
                Ult::Phantasm => {
                    let (px, py) = (-ay, ax);
                    self.phantoms.retain(|p| p.owner != id);
                    for side in [-1.0f32, 1.0] {
                        self.phantoms.push(Phantom {
                            owner: id,
                            x: hx + px * side * 2.6,
                            y: hy + py * side * 2.6,
                            dx: px * side * 2.6,
                            dy: py * side * 2.6,
                            left: PHANTOM_TICKS,
                        });
                    }
                }
            }
            self.cues.push(format!("ult:{}", ult.word()));
            self.notice("my_ult");
            self.found = Some((self.tick, id, format!("{}!", ult.name())));
            self.sounds.push("boss_rise");
            self.shake = self.shake.max(6);
        }
    }

    /// The ultimates that play out over time: Bladewind's cuts, the
    /// sniper's aim, Trebuchet's shells, Stillhours and phantoms.
    pub(super) fn tick_ults(&mut self) {
        // Bladewind: every few ticks, a blink and a cut.
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let hero = &self.players[&id];
            if hero.slashes == 0 || hero.hp == 0 {
                continue;
            }
            if hero.slash_wait > 0 {
                self.players.get_mut(&id).unwrap().slash_wait -= 1;
                continue;
            }
            let (hx, hy) = (hero.x, hero.y);
            let damage = hero.scaled(55);
            let target = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0 && (e.x - hx).hypot(e.y - hy) < 14.0)
                .min_by(|a, b| {
                    let d = |e: &&Enemy| (e.x - hx).hypot(e.y - hy) + (e.id % 3) as f32;
                    d(a).total_cmp(&d(b))
                })
                .map(|e| (e.id, e.x, e.y, e.radius()));
            let Some((tid, tx, ty, tr)) = target else {
                let hero = self.players.get_mut(&id).unwrap();
                hero.slashes = 0;
                continue;
            };
            let room = &self.dungeon.rooms[self.at];
            let grid = Grid { room, barred: true };
            let (ux, uy) = unit(hx - tx, hy - ty);
            let spots = [
                (tx + ux * (tr + 0.9), ty + uy * (tr + 0.9)),
                (tx - uy * (tr + 0.9), ty + ux * (tr + 0.9)),
                (tx + uy * (tr + 0.9), ty - ux * (tr + 0.9)),
                (tx - ux * (tr + 0.9), ty - uy * (tr + 0.9)),
            ];
            let landing = spots
                .into_iter()
                .find(|&(x, y)| grid.clear(x, y, HERO_RADIUS, Mover::Hero))
                .unwrap_or((hx, hy));
            if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == tid) {
                enemy.hp = enemy.hp.saturating_sub(damage);
            }
            if self.sparks.len() < 32 {
                self.sparks.push(Spark {
                    from: (hx, hy),
                    to: landing,
                    ttl: 7,
                });
            }
            let hero = self.players.get_mut(&id).unwrap();
            (hero.x, hero.y) = landing;
            (hero.aim_x, hero.aim_y) = unit(tx - landing.0, ty - landing.1);
            hero.slashes -= 1;
            hero.slash_wait = 4;
            hero.swing = 6;
            hero.sword = true;
            hero.invulnerable = hero.invulnerable.max(8);
            self.sounds.push("swing");
            self.sounds.push("hit");
        }
        // Assassinate: the aim holds a second, then the shot.
        for id in self.players.keys().copied().collect::<Vec<_>>() {
            let Some((target, left)) = self.players[&id].aiming else {
                continue;
            };
            let Some((tx, ty)) = self
                .enemies
                .iter()
                .find(|e| e.id == target && e.hp > 0)
                .map(|e| (e.x, e.y))
            else {
                self.players.get_mut(&id).unwrap().aiming = None;
                continue;
            };
            let hero = self.players.get_mut(&id).unwrap();
            if left > 1 {
                hero.aiming = Some((target, left - 1));
                continue;
            }
            hero.aiming = None;
            let (ux, uy) = unit(tx - hero.x, ty - hero.y);
            (hero.aim_x, hero.aim_y) = (ux, uy);
            let damage = hero.scaled(320);
            let (x, y) = (hero.x + ux * 0.8, hero.y + uy * 0.8);
            self.projectiles.push(Projectile {
                x,
                y,
                vx: ux * 40.0,
                vy: uy * 40.0,
                hostile: false,
                kind: Shot::Sacred,
                look: None,
                damage,
                // Straight down the line it was aimed, through anything in it.
                pierce: u8::MAX,
                last_hit: None,
                empowered: false,
                ttl: 2 * HZ,
                traits: ShotTraits {
                    owner: id as u8,
                    ..ShotTraits::default()
                },
            });
            self.sounds.push("shot_handgonne");
            self.shake = self.shake.max(6);
        }
        // Trebuchet's shells.
        let mut landed = Vec::new();
        for strike in &mut self.strikes {
            strike.fall = strike.fall.saturating_sub(1);
            if strike.fall == 0 {
                landed.push(*strike);
            }
        }
        self.strikes.retain(|s| s.fall > 0);
        for strike in landed {
            for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                if (enemy.x - strike.x).hypot(enemy.y - strike.y) <= STRIKE_REACH + enemy.radius() {
                    enemy.hp = enemy.hp.saturating_sub(strike.damage);
                }
            }
            if self.blasts.len() < 16 {
                self.blasts.push((strike.x, strike.y, BLAST_TICKS));
            }
            self.sounds.push("bomb");
            self.shake = self.shake.max(5);
        }
        // Stillhours: time stands still inside.
        for sphere in &mut self.spheres {
            sphere.left = sphere.left.saturating_sub(1);
        }
        self.spheres.retain(|s| s.left > 0);
        for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
            if self
                .spheres
                .iter()
                .any(|s| (enemy.x - s.x).hypot(enemy.y - s.y) < s.r)
            {
                enemy.frozen = enemy.frozen.max(2);
            }
        }
        // Phantoms keep their place beside their knight, and fade.
        for phantom in &mut self.phantoms {
            phantom.left = phantom.left.saturating_sub(1);
            match self.players.get(&phantom.owner).filter(|h| h.hp > 0) {
                Some(hero) => (phantom.x, phantom.y) = (hero.x + phantom.dx, hero.y + phantom.dy),
                None => phantom.left = 0,
            }
        }
        self.phantoms.retain(|p| p.left > 0);
        for hero in self.players.values_mut() {
            hero.angel = hero.angel.saturating_sub(1);
        }
    }
}

/// Copies of this tick's knight shots from each of their phantoms, at a
/// share of the damage.
pub(super) fn phantom_shots(
    phantoms: &[Phantom],
    heroes: &BTreeMap<u32, Hero>,
    shots: &mut Vec<Projectile>,
) {
    let mut copies = Vec::new();
    for phantom in phantoms {
        let Some(hero) = heroes.get(&phantom.owner) else {
            continue;
        };
        for shot in shots.iter().filter(|s| {
            !s.hostile && u32::from(s.traits.owner) == phantom.owner && s.kind != Shot::Blade
        }) {
            let mut copy = shot.clone();
            copy.x += phantom.x - hero.x;
            copy.y += phantom.y - hero.y;
            copy.damage = (shot.damage * PHANTOM_SHARE / 100).max(1);
            copies.push(copy);
        }
    }
    shots.extend(copies);
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_ults__tests.rs"]
mod tests;
