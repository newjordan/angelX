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
    /// New monsters: kind, where, and their stage (a slime's size).
    pub(super) spawns: Vec<(EnemyKind, f32, f32, u8)>,
    pub(super) kegs: Vec<(f32, f32)>,
    /// Bombs thrown: from, to.
    pub(super) lobs: Vec<((f32, f32), (f32, f32))>,
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
}

/// A sapper's keg: ticks until it blows.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Keg {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) fuse: u32,
}

pub(crate) const KEG_FUSE: u32 = 3 * HZ;
/// A keg is armed (a knight stepping by sets it off) once this much fuse is left.
pub(crate) const KEG_ARMED: u32 = 2 * HZ;
const KEG_REACH: f32 = 2.8;

/// A hob's bomb in the air, from where it was thrown to where it lands.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Lob {
    pub(crate) from: (f32, f32),
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) fall: u32,
}

pub(crate) const LOB_FALL: u32 = 36;
const LOB_REACH: f32 = 2.4;

/// A slime's health and size by its stage (0 big, 1 split once, 2 small).
pub(crate) fn slime(stage: u8) -> (u32, f32) {
    match stage {
        0 => (80, 1.1),
        1 => (36, 0.8),
        _ => (16, 0.55),
    }
}

/// How long a loot goblin stays before slipping away, and a serpent ward
/// before it crumbles.
pub(crate) const GOBLIN_STAYS: u32 = 12 * HZ;
pub(crate) const WARD_STANDS: u32 = 7 * HZ;
/// A warboar's wind-up and its charge's speed.
pub(crate) const BOAR_WINDUP: u32 = 24;
/// How many times a necromancer raises a pair of skeletons.
pub(crate) const NECRO_RAISES: u8 = 3;
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
        Sapper | Necromancer | Warboar | Slime | Goblin | Hob | Shaman | Ward
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
            if beat % 75 == 30 && target.is_some() {
                for k in -1..=1 {
                    fire(enemy.x, enemy.y, aim + k as f32 * 0.25, 4.5, Shot::Orb, 14);
                }
            }
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
        Goblin => {
            let wobble = ((enemy.age as f32) * 0.2 + enemy.id as f32).sin() * 0.7;
            let (ux, uy) = unit(-cx - cy * wobble, -cy + cx * wobble);
            if walk(enemy, (ux, uy), 5.6) < 0.05 {
                // Cornered: slip along the wall.
                walk(enemy, (-uy, ux), 5.6);
            }
            if enemy.timer >= GOBLIN_STAYS {
                deeds.escaped.push(enemy.id);
            }
        }
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
            if beat % 60 == 20 && target.is_some() {
                fire(enemy.x, enemy.y, aim, 7.5, Shot::Ember, 12);
            }
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
        self.hunt(deeds.hooks, deeds.webs, deeds.rot);
        self.tick_slams(deeds.slams);
        self.tick_ravage(deeds.ravages);
        self.tick_holes(deeds.holes);
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
                    self.shake = self.shake.max(6);
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
            if !g.economy.can_emit()
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
        let party = self.players.len().max(1) as u32;
        if let Some(enemy) = self.enemies.last_mut() {
            enemy.stage = stage;
            if kind == EnemyKind::Slime {
                let (hp, radius) = slime(stage);
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
            self.shake = self.shake.max(5);
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
            self.shake = self.shake.max(4);
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
                EnemyKind::Goblin => {
                    for k in 0..4 {
                        let a = k as f32 * std::f32::consts::FRAC_PI_2 + 0.4;
                        self.drop_item("gold".into(), x + a.cos() * 1.6, y + a.sin() * 1.2, None);
                    }
                    let pack = self.dungeon.pack;
                    if let Some(prize) =
                        self.book.roll_chest(&mut self.rng, pack).into_iter().next()
                    {
                        self.drop_item(prize, x, y + 1.0, None);
                    }
                    self.cues.push("goblin_caught".into());
                    self.notice("not_today_goblin");
                }
                _ => {}
            }
        }
    }

    /// Now and then a fight room has a loot goblin in it too.
    pub(super) fn maybe_goblin(&mut self) {
        if self.room().kind != RoomKind::Fight || !self.rng.chance(12) {
            return;
        }
        let (w, h) = (self.room().width(), self.room().height());
        let (x, y) = (w * 0.8, h * 0.25);
        let room = self.room();
        if room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
            && self.spawn_at(EnemyKind::Goblin, x, y).is_some()
        {
            self.cues.push("goblin".into());
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_foes__tests.rs"]
mod tests;
