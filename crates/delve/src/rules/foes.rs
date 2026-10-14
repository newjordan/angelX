//! The Delve's second company of monsters, each with something to read and
//! something to do about it:
//!
//! - **Sappers** plant kegs that blink and blow; a knight's shot sets one
//!   off early, and a keg hurts monsters as gladly as knights.
//! - **Necromancers** keep their distance and raise the dead: two
//!   skeletons every few seconds. Kill them first.
//! - **Warboars** paw the ground, mark a line, then charge down it; one
//!   that hits a wall stands dazed.
//! - **Slimes** split: big into two, each of those into two.
//! - **Loot goblins** run with a sack of gold and slip away if not caught.
//! - **Hobs** lob bombs at where a knight stands; a red ring marks where.
//! - **Shamans** box a knight in with four serpent wards that spit.
//!
//! What they make — kegs, bombs in the air, new monsters, a goblin gone —
//! comes out of each monster's step as `Deeds`, for the run to make so.

use super::*;

/// What monsters made happen this tick.
#[derive(Default)]
pub(super) struct Deeds {
    pub(super) clouds: Vec<skills::Cloud>,
    pub(super) pulls: Vec<(f32, f32)>,
    pub(super) sweeps: Vec<((f32, f32), (f32, f32), f32, u32, (f32, f32))>,
    pub(super) sounds: Vec<&'static str>,
    pub(super) shake: u32,
    /// New monsters: kind, where, and their stage (a slime's size).
    pub(super) spawns: Vec<(EnemyKind, f32, f32, u8)>,
    pub(super) kegs: Vec<(f32, f32)>,
    /// Bombs thrown: from, to.
    pub(super) lobs: Vec<((f32, f32), (f32, f32))>,
    /// The Broodmarrows' children thrown: from the shoulder, at a knight.
    pub(super) flings: Vec<((f32, f32), (f32, f32))>,
    /// The Whirlgraves' wells opened: where they lean.
    pub(super) wells: Vec<(f32, f32)>,
    /// A charge that runs knights down: where, its reach, its damage and
    /// the way it pushes.
    pub(super) rams: Vec<(f32, f32, f32, u32, (f32, f32))>,
    /// Goblins that got away.
    pub(super) escaped: Vec<u32>,
    pub(super) cues: Vec<&'static str>,
    /// The hunters': hooks thrown (by whom, from, which way), webs spun
    /// (where), the Flesher's rot (where).
    pub(super) hooks: Vec<(u32, f32, f32, (f32, f32))>,
    pub(super) webs: Vec<(f32, f32)>,
    pub(super) rot: Vec<(f32, f32)>,
    /// The Pit Tyrant's slams, gathering where it stands.
    pub(super) slams: Vec<(f32, f32)>,
    /// A guardian's Ravages, cast.
    pub(super) ravages: Vec<super::tide::Ravage>,
    /// Black Holes the Hollow Ones opened: where, and by whom.
    pub(super) holes: Vec<(f32, f32, u32)>,
    /// The third company's moves (`critters`).
    pub(super) crit: super::critters::Doings,
}

/// A sapper's keg: ticks until it blows.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keg {
    pub x: f32,
    pub y: f32,
    pub fuse: u32,
}

pub const KEG_FUSE: u32 = 3 * HZ;
/// A keg is armed (a knight stepping by sets it off) once this much fuse is left.
pub const KEG_ARMED: u32 = 2 * HZ;
const KEG_REACH: f32 = 2.8;

/// A hob's bomb in the air, from where it was thrown to where it lands.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lob {
    pub from: (f32, f32),
    pub x: f32,
    pub y: f32,
    pub fall: u32,
}

pub const LOB_FALL: u32 = 36;
const LOB_REACH: f32 = 2.4;

/// A Whirlgrave's open well: where it leans, and for how long it means
/// to keep leaning. Knights slide toward it; every shot in the room,
/// friend or foe, bends a little toward it too.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Well {
    pub x: f32,
    pub y: f32,
    pub left: u32,
}

/// A well stands this long, reaches this far, drags this hard, and bends
/// shots by this much a tick. Two to a room, no more.
pub const WELL_SPAN: u32 = 6 * HZ;
pub const WELL_REACH: f32 = 7.0;
const WELL_PULL: f32 = 0.6;
const WELL_TURN: f32 = 0.09;
pub const WELLS_MAX: usize = 2;

/// A Broodmarrow's impling in flight: torn from the marrow's shoulder,
/// thrown at a knight, landing crossways with its teeth out.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fling {
    pub from: (f32, f32),
    pub x: f32,
    pub y: f32,
    pub fall: u32,
}

/// How long an impling is in the air.
pub const FLING_FALL: u32 = 26;
/// The Broodmarrow: its wind-up, the wound each rip costs its own hide,
/// the rest between throws, the most children in flight, the most of its
/// children a room may hold, the landing bite, and its reach.
pub const BROOD_WINDUP: u32 = 34;
pub const BROOD_RIP: u32 = 14;
pub const BROOD_REST: u32 = 40;
pub const BROOD_FLINGS: usize = 6;
pub const BROOD_BROOD: usize = 8;
pub const FLING_BITE: u32 = 10;
pub const FLING_STOMP: f32 = 1.4;

/// A slime's health and size by its stage (0 big, 1 split once, 2 small).
pub fn slime(stage: u8) -> (u32, f32) {
    match stage {
        0 => (80, 1.1),
        1 => (36, 0.8),
        _ => (16, 0.55),
    }
}

/// How long a serpent ward stands before it crumbles.
pub const WARD_STANDS: u32 = 7 * HZ;
/// A warboar's wind-up and its charge's speed.
pub const BOAR_WINDUP: u32 = 24;
/// How many times a necromancer raises a pair of skeletons.
pub const NECRO_RAISES: u8 = 3;
const BOAR_SPEED: f32 = 21.0;

/// One of the second company's steps. False for any other monster.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    shots: &mut Vec<Projectile>,
    deeds: &mut Deeds,
) -> bool {
    use EnemyKind::*;
    if !matches!(
        enemy.kind,
        Sapper | Necromancer | Warboar | Slime | Goblin | Hob | Shaman | Ward | Risen
            | Broodmarrow | Oathbound | Whirlgrave
    ) {
        return false;
    }
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let (tx, ty) = target.unwrap_or((enemy.x, enemy.y + 1.0));
    let (cx, cy) = unit(tx - enemy.x, ty - enemy.y);
    let dist = (tx - enemy.x).hypot(ty - enemy.y);
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    let mover = if enemy.kind.flies() {
        Mover::Flier
    } else {
        Mover::Walker
    };
    let r = enemy.radius();
    let walk = |enemy: &mut Enemy, (dx, dy): (f32, f32), speed: f32| {
        let (x, y) = grid.slide(
            (enemy.x, enemy.y),
            (dx * speed * DT, dy * speed * DT),
            r,
            mover,
        );
        let moved = (x - enemy.x).hypot(y - enemy.y);
        (enemy.x, enemy.y) = (x, y);
        moved
    };
    let mut fire = |x: f32, y: f32, angle: f32, speed: f32, kind: Shot, damage: u32| {
        shots.push(Projectile {
            x,
            y,
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
    // Keep between `near` and `far` of the nearest knight, sidling between.
    let keep_off = |enemy: &mut Enemy, near: f32, far: f32, speed: f32| {
        let side = if enemy.id.is_multiple_of(2) { 1.0 } else { -1.0 };
        let way = if dist < near {
            (-cx, -cy)
        } else if dist > far {
            (cx, cy)
        } else {
            (-cy * side, cx * side)
        };
        walk(enemy, way, speed);
    };
    enemy.timer += 1;
    let beat = enemy.timer + enemy.id * 7;
    match enemy.kind {
        Sapper => {
            if enemy.stage == 0 {
                // Close in to a few steps off, plant, then run.
                let (gx, gy) = (tx - cx * 5.5, ty - cy * 5.5);
                let (ux, uy) = unit(gx - enemy.x, gy - enemy.y);
                if (gx - enemy.x).hypot(gy - enemy.y) > 0.5 {
                    walk(enemy, (ux, uy), 5.0);
                }
                if enemy.timer >= 100 + (enemy.id % 5) * 9 && target.is_some() && dist < 7.5 {
                    deeds.kegs.push((enemy.x, enemy.y));
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            } else {
                walk(enemy, (-cx, -cy), 6.5);
                if enemy.timer >= 40 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        }
        Necromancer => {
            keep_off(enemy, 8.0, 12.0, 2.0);
            // Bones for three raisings; then only its orbs.
            if beat % 150 == 60 && enemy.stage < NECRO_RAISES {
                enemy.stage += 1;
                for side in [-1.0f32, 1.0] {
                    deeds
                        .spawns
                        .push((Skeleton, enemy.x + side * 2.2, enemy.y + 1.2, 0));
                }
                deeds.cues.push("necro_raise");
            }
            // Its orbs are its signature (`patterns`).
        }
        Warboar => match enemy.stage {
            // Stalk, then mark the line.
            0 => {
                walk(enemy, (cx, cy), 2.6);
                if enemy.timer >= 60 && target.is_some() && dist < 18.0 {
                    (enemy.stage, enemy.timer, enemy.dir) = (1, 0, (cx, cy));
                }
            }
            1 => {
                if enemy.timer >= BOAR_WINDUP {
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            // Down the line until something stops it.
            2 => {
                let moved = walk(enemy, enemy.dir, BOAR_SPEED);
                deeds.rams.push((enemy.x, enemy.y, r + 0.7, 24, enemy.dir));
                if moved < BOAR_SPEED * DT * 0.3 {
                    (enemy.stage, enemy.timer) = (3, 0);
                    deeds.cues.push("boar_dazed");
                } else if enemy.timer > 45 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
            // Dazed against the wall: a moment to make it pay.
            _ => {
                if enemy.timer >= 50 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // Hops: a lunge, a squat.
        Slime if enemy.age % 40 < 12 => {
            walk(enemy, (cx, cy), 6.0 - f32::from(enemy.stage));
        }
        // Humphrey: a lurching shamble, a rising wind-up, then the shove.
        Risen => match enemy.stage {
            0 => {
                let lurch = if beat % 36 < 22 { 2.1 } else { 0.9 };
                walk(enemy, (cx, cy), lurch);
                enemy.dir = (cx, cy);
                if target.is_some() && dist < 3.6 && enemy.timer >= 24 {
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            }
            // Arms up, jaw loose: the party has a moment to step aside.
            1 => {
                enemy.dir = (cx, cy);
                if enemy.timer >= 20 {
                    (enemy.stage, enemy.timer, enemy.dir) = (2, 0, (cx, cy));
                    deeds.cues.push("risen_shove");
                }
            }
            2 => {
                let moved = walk(enemy, enemy.dir, 14.0);
                deeds.rams.push((enemy.x, enemy.y, r + 0.9, 12, enemy.dir));
                if moved < 14.0 * DT * 0.3 || enemy.timer >= 12 {
                    (enemy.stage, enemy.timer) = (3, 0);
                }
            }
            // Winded, hands on knees, deeply opposed to exercise.
            _ => {
                if enemy.timer >= 36 {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // The Broodmarrow: a hill of hide that answers arrows with
        // offspring. It waddles, stops, lights its seams, and tears a
        // child from its own back to throw it where a knight stands.
        Broodmarrow => match enemy.stage {
            // The waddle: never nearer than a spade's length, never far.
            0 => {
                keep_off(enemy, 4.5, 10.0, 1.6);
                if enemy.timer >= 80 && target.is_some() && dist < 17.0 {
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            }
            // The rip: it stands, its seams come alight, and something on
            // its back begins to squeal. A marrow bled to its last
            // measure still lights the seams (the tell stays honest) but
            // cannot pay, so no child comes.
            1 => {
                if enemy.timer >= BROOD_WINDUP {
                    if enemy.hp > BROOD_RIP {
                        enemy.hp -= BROOD_RIP;
                        let (sx, sy) = (enemy.x + cx * r * 0.7, enemy.y + cy * r * 0.7);
                        deeds.flings.push(((sx, sy), (tx, ty)));
                        deeds.cues.push("brood_rip");
                    }
                    (enemy.stage, enemy.timer) = (2, 0);
                }
            }
            // A parent's pride: it watches its child land, then waddles on.
            _ => {
                if enemy.timer >= BROOD_REST {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // The Whirlgrave: it drifts, plants itself, and leans. While it
        // leans, the room's geometry is a rumour.
        Whirlgrave => match enemy.stage {
            // The drift: never nearer than a shout, never gone.
            0 => {
                keep_off(enemy, 7.0, 11.0, 1.9);
                if enemy.timer % 130 == 60 && target.is_some() {
                    deeds.wells.push((enemy.x, enemy.y));
                    (enemy.stage, enemy.timer) = (1, 0);
                }
            }
            // The lean: it stands, and the well does the walking.
            _ => {
                if enemy.timer >= WELL_SPAN {
                    (enemy.stage, enemy.timer) = (0, 0);
                }
            }
        },
        // The Oathbound: their stride is their oath's business. While the
        // twin lives they hold formation; alone, they grieve at a pace no
        // grave should allow, with a shove in it.
        Oathbound => match enemy.stage {
            // Formation: never far from the knights, never crowded.
            0 => {
                keep_off(enemy, 2.0, 8.0, 2.4);
            }
            // The grief: half again the stride, and every so often a
            // two-handed shove that has stopped being a question.
            _ => {
                keep_off(enemy, 2.0, 8.0, 4.2);
                if enemy.timer % 70 == 40 && target.is_some() && dist < 5.0 {
                    deeds.rams.push((enemy.x, enemy.y, r + 0.9, 10, (cx, cy)));
                }
            }
        },
        // A goblin's own steps are the thief's (`thief.rs`).
        Goblin => {}
        Hob => {
            let h = mix(seed ^ u64::from(enemy.id) << 20 ^ u64::from(enemy.age / 75));
            let gx = enemy.origin_x + ((h & 0xff) as f32 / 255.0 - 0.5) * 10.0;
            let gy = enemy.origin_y + (((h >> 8) & 0xff) as f32 / 255.0 - 0.5) * 7.0;
            let (ux, uy) = unit(gx - enemy.x, gy - enemy.y);
            walk(enemy, (ux, uy), 2.4);
            if beat % 95 == 40 && target.is_some() && dist < 20.0 {
                deeds.lobs.push(((enemy.x, enemy.y), (tx, ty)));
            }
        }
        Shaman => {
            keep_off(enemy, 9.0, 13.0, 2.2);
            if beat % 180 == 90 && target.is_some() && dist < 15.0 {
                for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    deeds.spawns.push((Ward, tx + dx * 3.2, ty + dy * 3.2, 0));
                }
                deeds.cues.push("shaman_wards");
            }
            // Between wards it calls crossfire (`patterns`).
        }
        Ward => {
            if beat.is_multiple_of(40) && target.is_some() {
                fire(enemy.x, enemy.y, aim, 8.0, Shot::Orb, 8);
            }
            if enemy.timer >= WARD_STANDS {
                enemy.hp = 0;
            }
        }
        _ => {}
    }
    true
}

impl Run {
    /// Make what the monsters did this tick so.
    pub(super) fn apply_deeds(&mut self, deeds: Deeds) {
        self.signature_deeds(deeds.clouds, deeds.pulls, deeds.sweeps);
        self.sounds.extend(deeds.sounds);
        self.fx.push(Fx::Shake(deeds.shake));
        self.hunt(deeds.hooks, deeds.webs, deeds.rot);
        self.tick_slams(deeds.slams);
        self.tick_ravage(deeds.ravages);
        self.tick_holes(deeds.holes);
        self.apply_critters(deeds.crit);
        for (x, y) in deeds.kegs {
            if self.kegs.len() < 8 {
                self.kegs.push(Keg {
                    x,
                    y,
                    fuse: KEG_FUSE,
                });
            }
        }
        for (from, (x, y)) in deeds.lobs {
            if self.lobs.len() < 8 {
                self.lobs.push(Lob {
                    from,
                    x,
                    y,
                    fall: LOB_FALL,
                });
            }
        }
        for (x, y) in deeds.wells {
            if self.wells.len() < WELLS_MAX {
                self.wells.push(Well {
                    x,
                    y,
                    left: WELL_SPAN,
                });
                self.sounds.push("wave_gate");
            }
        }
        for (from, (x, y)) in deeds.flings {
            if self.flings.len() < BROOD_FLINGS {
                self.flings.push(Fling {
                    from,
                    x,
                    y,
                    fall: FLING_FALL,
                });
            }
        }
        for (x, y, reach, damage, (dx, dy)) in deeds.rams {
            let room = &self.dungeon.rooms[self.at];
            let grid = Grid { room, barred: true };
            for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                if hero.invulnerable == 0 && (hero.x - x).hypot(hero.y - y) < reach {
                    hero.hurt(damage);
                    // Run down and thrown aside.
                    let (sx, sy) = (-dy, dx);
                    let side = if (hero.x - x) * sx + (hero.y - y) * sy >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    (hero.x, hero.y) = grid.slide(
                        (hero.x, hero.y),
                        ((dx + sx * side) * 1.6, (dy + sy * side) * 1.6),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                    self.sounds.push("hero_hurt");
                    self.fx.push(Fx::Shake(6));
                }
            }
        }
        let wards = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Ward)
            .count();
        let raised = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Skeleton)
            .count();
        for (kind, x, y, stage) in deeds.spawns {
            if self.enemies.len() >= 40
                || (kind == EnemyKind::Ward && wards >= 8)
                || (kind == EnemyKind::Skeleton && raised >= 6)
                || (kind == EnemyKind::Spiderling && self.brood_full())
            {
                continue;
            }
            let room = self.room();
            let (w, h) = (room.width(), room.height());
            // The spot asked for, or the nearest floor round it.
            let spot = [
                (0.0, 0.0),
                (1.2, 0.0),
                (-1.2, 0.0),
                (0.0, 1.2),
                (0.0, -1.2),
                (1.2, 1.2),
                (-1.2, -1.2),
            ]
            .into_iter()
            .map(|(dx, dy)| ((x + dx).clamp(2.5, w - 2.5), (y + dy).clamp(2.5, h - 2.5)))
            .find(|&(x, y)| {
                room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
            });
            let Some((x, y)) = spot else {
                continue;
            };
            self.spawn_staged(kind, x, y, stage);
        }
        for id in deeds.escaped {
            if let Some(g) = &mut self.boss_gates {
                g.economy.retire(id);
            }
            if let Some(i) = self.enemies.iter().position(|e| e.id == id) {
                let goblin = self.enemies.remove(i);
                if self.blasts.len() < 16 {
                    self.blasts.push((goblin.x, goblin.y, BLAST_TICKS));
                }
                self.cues.push("goblin_escaped".into());
                self.notice("sticky_fingers");
                self.found = Some((self.tick, 0, "The loot goblin got away".into()));
            }
        }
        for cue in deeds.cues {
            self.cues.push(cue.into());
        }
    }

    /// A monster at a spot with its stage: a slime's size, a ward's turn.
    pub(super) fn spawn_staged(
        &mut self,
        kind: EnemyKind,
        x: f32,
        y: f32,
        stage: u8,
    ) -> Option<u32> {
        if let Some(g) = &self.boss_gates {
            if !g.economy.can_emit_for(self.party_len())
                || !super::boss_ecology::Economy::emission_kind(kind, stage)
                || self.guardian_locked()
                || !x.is_finite()
                || !y.is_finite()
            {
                return None;
            }
            let next = self.next_id.checked_add(1)?;
            if g.economy.events.iter().any(|e| {
                matches!(e,
                super::boss_ecology::Event::Emit { id, .. } if *id >= next)
            }) {
                return None;
            }
            let tile = self
                .room()
                .tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
            if tile != Tile::Floor && !(kind.flies() && tile == Tile::Hazard) {
                return None;
            }
        }
        let id = self.spawn_body(kind, x, y)?;
        let party = self.party_len().max(1) as u32;
        let tough = self.toughness();
        if let Some(enemy) = self.enemies.last_mut() {
            enemy.stage = stage;
            if kind == EnemyKind::Spiderling && stage == super::signatures::TENDER {
                enemy.size = Some(1.6);
            }
            if kind == EnemyKind::Slime {
                let (hp, radius) = slime(stage);
                let hp = hp * tough / 100;
                enemy.base_hp = Some(hp);
                enemy.size = Some(radius);
                enemy.max_hp = hp * party;
                enemy.hp = enemy.max_hp;
            }
        }
        // Only now is an actual body present. Failed spot/cap/id attempts do
        // not debit; all indirect callers share this floor-local provision.
        if let Some(g) = &mut self.boss_gates {
            g.economy.emit(self.at, id, kind, stage);
        }
        Some(id)
    }

    /// The Broodmarrows' children come down where they were thrown:
    /// biting what they land on, and taking up the fight as imps.
    pub(super) fn tick_flings(&mut self) {
        let mut landed = Vec::new();
        for fling in &mut self.flings {
            fling.fall = fling.fall.saturating_sub(1);
            if fling.fall == 0 {
                landed.push((fling.x, fling.y));
            }
        }
        self.flings.retain(|f| f.fall > 0);
        let imps = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Imp && e.hp > 0)
            .count();
        for (i, (x, y)) in landed.into_iter().enumerate() {
            // It stands up, small and furious — the family trade — on the
            // spot it landed or the nearest floor round it.
            let room = &self.dungeon.rooms[self.at];
            let (w, h) = (room.width(), room.height());
            let spot = [
                (0.0, 0.0),
                (1.2, 0.0),
                (-1.2, 0.0),
                (0.0, 1.2),
                (0.0, -1.2),
                (1.2, 1.2),
                (-1.2, -1.2),
            ]
            .into_iter()
            .map(|(dx, dy)| ((x + dx).clamp(2.5, w - 2.5), (y + dy).clamp(2.5, h - 2.5)))
            .find(|&(x, y)| {
                room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
            });
            // The landing itself bites, crossways, teeth already out.
            for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                if hero.invulnerable == 0 && (hero.x - x).hypot(hero.y - y) < FLING_STOMP {
                    hero.hurt(FLING_BITE);
                    self.sounds.push("hero_hurt");
                }
            }
            if imps + i < BROOD_BROOD
                && let Some((x, y)) = spot
            {
                self.spawn_staged(EnemyKind::Imp, x, y, 0);
            }
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
            self.sounds.push("rock_land");
            self.fx.push(Fx::Shake(3));
        }
    }

    /// Kegs burn down and blow; bombs come down. Both hurt knights; a keg
    /// hurts monsters too.
    pub(super) fn tick_kegs_and_lobs(&mut self) {
        let mut blown = Vec::new();
        for keg in &mut self.kegs {
            keg.fuse = keg.fuse.saturating_sub(1);
            let stepped = keg.fuse < KEG_ARMED
                && self
                    .players
                    .values()
                    .any(|h| h.hp > 0 && !h.stone && (h.x - keg.x).hypot(h.y - keg.y) < 1.1);
            if keg.fuse == 0 || stepped {
                blown.push((keg.x, keg.y));
                keg.fuse = 0;
            }
        }
        self.kegs.retain(|k| k.fuse > 0);
        for (x, y) in blown {
            for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                if (hero.x - x).hypot(hero.y - y) < KEG_REACH {
                    hero.hurt(18);
                }
            }
            let mut felled = 0;
            for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                if (enemy.x - x).hypot(enemy.y - y) < KEG_REACH + enemy.radius() {
                    enemy.hp = enemy.hp.saturating_sub(30);
                    felled += u32::from(enemy.hp == 0);
                }
            }
            if felled >= 3 {
                self.notice("demolitions");
            }
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
            self.sounds.push("bomb");
            self.fx.push(Fx::Shake(5));
            self.blast_wall(Some((x, y)));
        }
        let mut landed = Vec::new();
        for lob in &mut self.lobs {
            lob.fall = lob.fall.saturating_sub(1);
            if lob.fall == 0 {
                landed.push((lob.x, lob.y));
            }
        }
        self.lobs.retain(|l| l.fall > 0);
        for (x, y) in landed {
            for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                if (hero.x - x).hypot(hero.y - y) < LOB_REACH {
                    hero.hurt(14);
                }
            }
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
            self.sounds.push("bomb");
            self.fx.push(Fx::Shake(4));
            self.blast_wall(Some((x, y)));
        }
    }

    /// A knight's shot that reaches a keg sets it off.
    pub(super) fn shoot_kegs(&mut self) {
        for shot in self
            .projectiles
            .iter_mut()
            .filter(|p| !p.hostile && p.ttl > 0)
        {
            if let Some(keg) = self
                .kegs
                .iter_mut()
                .find(|k| k.fuse > 1 && (shot.x - k.x).hypot(shot.y - k.y) < 0.9)
            {
                keg.fuse = 1;
                shot.ttl = 0;
            }
        }
    }

    /// The second company's fallen: slimes split, goblins spill their gold.
    pub(super) fn fallen_foes(&mut self, fallen: &[(EnemyKind, f32, f32, u8)]) {
        for &(kind, x, y, stage) in fallen {
            match kind {
                EnemyKind::Slime if stage < 2 => {
                    for side in [-1.0f32, 1.0] {
                        if self
                            .spawn_staged(EnemyKind::Slime, x + side * 0.9, y, stage + 1)
                            .is_some()
                            && let Some(enemy) = self.enemies.last_mut()
                        {
                            // A fresh half doesn't sit still in the telegraph.
                            enemy.age = TELEGRAPH;
                        }
                    }
                }
                EnemyKind::Geode => self.geode_splits(x, y),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_foes__tests.rs"]
mod tests;

impl Run {
    /// One oath, shared wounds: every living Oathbound pair in the room
    /// bleeds to the lesser of the two (floored at one wound while a twin
    /// stands), and the first to stand alone begins to grieve.
    pub(super) fn tick_oathbound(&mut self) {
        let bound: Vec<(usize, u32)> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == EnemyKind::Oathbound && e.hp > 0)
            .map(|(i, e)| (i, e.hp))
            .collect();
        for &(i, hp) in &bound {
            if let Some(&(_, twin_hp)) = bound
                .iter()
                .find(|&&(j, _)| j != i)
            {
                let shared = hp.min(twin_hp).max(1);
                let e = &mut self.enemies[i];
                if e.hp > shared {
                    e.hp = shared;
                }
                if e.stage != 0 {
                    // A twin stood back up (a second pair, or a heal):
                    // the grief settles.
                    e.stage = 0;
                    e.timer = 0;
                }
            }
        }
        // The broken oath: alone now, and it was not alone last tick.
        for i in 0..self.enemies.len() {
            let alone = {
                let e = &self.enemies[i];
                e.kind == EnemyKind::Oathbound
                    && e.hp > 0
                    && e.stage == 0
                    && !self
                        .enemies
                        .iter()
                        .enumerate()
                        .any(|(j, o)| j != i && o.kind == EnemyKind::Oathbound && o.hp > 0)
            };
            if alone {
                let e = &mut self.enemies[i];
                // The oath's last cruelty: the shared wound comes due all
                // at once, and leaves exactly one wound standing.
                e.hp = e.hp.min(1);
                e.stage = 1;
                e.timer = 0;
                self.cues.push("oath_broken".into());
                self.sounds.push("boss_rage");
            }
        }
    }
}

impl Run {
    /// The Whirlgraves' wells: they close with their stone, drag what
    /// stands, and bend what flies — friend and foe the same. The drag
    /// uses the same slide as the Hollow One's holes; the bend is the
    /// homing shot's own arithmetic, pointed at the well.
    pub(super) fn tick_wells(&mut self) {
        if !self.enemies.iter().any(|e| e.kind == EnemyKind::Whirlgrave && e.hp > 0) {
            self.wells.clear();
            return;
        }
        for well in &mut self.wells {
            well.left = well.left.saturating_sub(1);
        }
        self.wells.retain(|w| w.left > 0);
        if self.wells.is_empty() {
            return;
        }
        let room = &self.dungeon.rooms[self.at];
        let grid = Grid { room, barred: true };
        let wells: Vec<(f32, f32)> = self.wells.iter().map(|w| (w.x, w.y)).collect();
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            for &(wx, wy) in &wells {
                let (dx, dy) = (wx - hero.x, wy - hero.y);
                let d = dx.hypot(dy);
                if d > 0.05 && d < WELL_REACH {
                    let pull = WELL_PULL * (1.0 - 0.4 * d / WELL_REACH) * DT;
                    let (ux, uy) = unit(dx, dy);
                    (hero.x, hero.y) = grid.slide(
                        (hero.x, hero.y),
                        (ux * pull.min(d), uy * pull.min(d)),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                }
            }
        }
        for bullet in &mut self.projectiles {
            for &(wx, wy) in &wells {
                let (dx, dy) = (wx - bullet.x, wy - bullet.y);
                let d = dx.hypot(dy);
                if d >= WELL_REACH || d < 0.05 {
                    continue;
                }
                let speed = bullet.vx.hypot(bullet.vy);
                let now = bullet.vy.atan2(bullet.vx);
                let want = dy.atan2(dx);
                let turn = (want - now).sin().atan2((want - now).cos());
                let fade = 1.0 - d / WELL_REACH;
                let next = now + (turn * fade * WELL_TURN).clamp(-WELL_TURN, WELL_TURN);
                (bullet.vx, bullet.vy) = (next.cos() * speed, next.sin() * speed);
            }
        }
    }
}
