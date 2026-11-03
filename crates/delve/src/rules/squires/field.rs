//! One companion at each knight's heels. Their books stay in town; these
//! actors belong to this delve, not to the slots used by summoned minions.

use super::super::hireling::{AllyKind, Hireling, beside};
use super::*;

/// Six tiles from their own knight, not from whichever friend is nearest.
const LEASH: f32 = 6.0 * TILE_UNITS;
const REACH: f32 = 1.5;
/// Soft room for a knight's silhouette, in world units, not tiles. The
/// correction is deliberately short: companions yield, never shove owners.
const BREATHING_ROOM: f32 = 2.0;
const SEPARATION_STEP: f32 = 0.24;
const MEND_EVERY: u32 = 2 * HZ;
const SPECIAL_EVERY: u32 = 5 * HZ;

fn level(s: &Companion) -> u32 {
    u32::from(s.squire.level.clamp(1, 10))
}

impl Run {
    /// Call after the knights land on a new dungeon floor (including the
    /// first descent). One selected squire per present owner, at most six;
    /// neither the tavern's hireling nor any minion is displaced. A floor
    /// restores the actor, but never changes the persistent roster.
    pub fn deploy_company_squires(&mut self) {
        self.squire_field.companions.clear();
        if self.at_home_now() || self.dungeon.rooms.get(self.at).is_none() {
            return;
        }
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let mut companions = BTreeMap::new();
        for (&owner, hero) in self.players.iter().filter(|(_, h)| !h.absent).take(6) {
            let Some(roster) = self.home.squire_company.rosters.get(&hero.name) else {
                continue;
            };
            let Some(squire) = roster.squires.iter().find(|s| Some(s.id) == roster.active) else {
                continue;
            };
            let tier = u32::from(squire.level.clamp(1, 10));
            let kind = match squire.kind {
                Kind::Ninja => AllyKind::Hound,
                Kind::Healer => AllyKind::Linnet,
                Kind::Ranger => AllyKind::Beaumains,
                Kind::Shield => AllyKind::Brawler,
                Kind::Alchemist => AllyKind::Brownie,
            };
            let mut actor = Hireling::minion(
                kind,
                6 + tier * 3 + self.dungeon.depth * 2,
                self.dungeon.depth,
                beside(&grid, (hero.x, hero.y)),
                owner,
            );
            // No fraction of a monster's health enters these stats. More
            // friends do not make a squire hit harder or live longer.
            actor.max_hp = (48 + tier * 16 + self.dungeon.depth * 8)
                * if squire.kind == Kind::Shield { 2 } else { 1 };
            actor.hp = actor.max_hp;
            companions.insert(
                owner,
                Companion {
                    squire: squire.clone(),
                    actor,
                    ability: 0,
                    effect: 0,
                    mark: None,
                },
            );
        }
        self.squire_field.companions = companions;
    }

    /// A doorway is not a new enlistment. Keep wounds, come beside the
    /// owner on legal ground, and forget the last room's target.
    pub fn company_squires_follow(&mut self) {
        if self.at_home_now() {
            self.squire_field.companions.clear();
            return;
        }
        let mut companions = std::mem::take(&mut self.squire_field.companions);
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        for (&owner, s) in &mut companions {
            if let Some(hero) = self.players.get(&owner).filter(|h| !h.absent) {
                (s.actor.x, s.actor.y) = beside(&grid, (hero.x, hero.y));
                s.actor.invulnerable = s.actor.invulnerable.max(20);
                s.mark = None;
                s.effect = 0;
                if !self.fight_on() && s.actor.down() {
                    s.actor.hp = s.actor.max_hp / 2;
                }
            }
        }
        companions.retain(|id, _| self.players.get(id).is_some_and(|h| !h.absent));
        self.squire_field.companions = companions;
    }

    /// The hireling calls this even when its own slot is empty. BTree
    /// owner order breaks every tie, including two healers' choice of patient.
    pub fn tick_company_squires(&mut self) {
        if self.at_home_now() {
            self.squire_field.companions.clear();
            return;
        }
        let mut companions = std::mem::take(&mut self.squire_field.companions);
        companions.retain(|id, _| self.players.get(id).is_some_and(|h| !h.absent));
        for (&owner, s) in &mut companions {
            self.drive_squire(owner, s);
        }
        self.squire_field.companions = companions;
        self.separate_squires();
    }

    /// Drive temporarily takes the field out of Run. Separate afterwards,
    /// with every peer available; BTree owner order also breaks exact-pile
    /// ties deterministically. Only squire positions are written back.
    fn separate_squires(&mut self) {
        if self.squire_field.companions.is_empty() {
            return;
        }
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let fixed: Vec<_> = self
            .players
            .values()
            .filter(|h| !h.absent && h.hp > 0)
            .map(|h| (h.x, h.y))
            .chain(
                self.hireling
                    .iter()
                    .chain(self.minions.raised.iter())
                    .filter(|a| !a.down())
                    .map(|a| (a.x, a.y)),
            )
            .collect();
        let mut peers: Vec<_> = self.squire_field
            .companions
            .iter()
            .filter(|(_, s)| !s.actor.down())
            .map(|(&id, s)| (id, (s.actor.x, s.actor.y)))
            .collect();
        for i in 0..peers.len() {
            let (owner, at) = peers[i];
            let hero = &self.players[&owner];
            let crowding = |point: (f32, f32)| -> f32 {
                fixed
                    .iter()
                    .copied()
                    .chain(
                        peers
                            .iter()
                            .enumerate()
                            .filter(|(j, _)| *j != i)
                            .map(|(_, (_, at))| *at),
                    )
                    .map(|p| {
                        (BREATHING_ROOM - (point.0 - p.0).hypot(point.1 - p.1))
                            .max(0.0)
                            .powi(2)
                    })
                    .sum()
            };
            let mut best = at;
            let mut cost = crowding(at);
            if cost <= f32::EPSILON {
                continue;
            }
            // Trying directions as well as radial repulsion lets a squire
            // slide along stone, and avoids zero vectors in an exact pile.
            // All candidate steps are shorter than the body's radius.
            for step in [SEPARATION_STEP, SEPARATION_STEP / 2.0] {
                for direction in 0..16 {
                    let angle =
                        ((direction + (owner % 16) * 3) % 16) as f32 * std::f32::consts::TAU / 16.0;
                    let next = grid.slide(
                        at,
                        (angle.cos() * step, angle.sin() * step),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                    if !grid.clear(next.0, next.1, HERO_RADIUS, Mover::Hero)
                        || (next.0 - hero.x).hypot(next.1 - hero.y) > LEASH
                    {
                        continue;
                    }
                    let next_cost = crowding(next);
                    if next_cost + f32::EPSILON < cost {
                        best = next;
                        cost = next_cost;
                    }
                }
            }
            // Later peers see earlier corrections, not an empty field or
            // six identical stale spawn positions.
            peers[i].1 = best;
        }
        for (owner, at) in peers {
            let actor = &mut self.squire_field.companions.get_mut(&owner).unwrap().actor;
            (actor.x, actor.y) = at;
        }
    }

    fn drive_squire(&mut self, owner: u32, s: &mut Companion) {
        let hero = &self.players[&owner];
        let heel = (hero.x, hero.y);
        s.actor.cooldown = s.actor.cooldown.saturating_sub(1);
        s.actor.invulnerable = s.actor.invulnerable.saturating_sub(1);
        s.ability = s.ability.saturating_sub(1);
        s.effect = s.effect.saturating_sub(1);
        if s.effect == 0
            || !self
                .enemies
                .iter()
                .any(|e| Some(e.id) == s.mark && e.hp > 0)
        {
            s.mark = None;
        }
        if s.actor.down() {
            if !self.fight_on() {
                s.actor.hp = s.actor.max_hp / 2;
                s.actor.invulnerable = 20;
                (s.actor.x, s.actor.y) = beside(
                    &Grid {
                        room: self.room(),
                        barred: self.barred(),
                    },
                    heel,
                );
            }
            return;
        }
        // A knight can dash or be placed beyond the leash. Recall to a
        // legal tile at the heel, never to a point across a wall from it.
        if (s.actor.x - heel.0).hypot(s.actor.y - heel.1) > LEASH {
            (s.actor.x, s.actor.y) = beside(
                &Grid {
                    room: self.room(),
                    barred: self.barred(),
                },
                heel,
            );
        }
        let vanished = s.squire.kind == Kind::Ninja && level(s) >= 3 && s.effect > 0;
        if !vanished {
            // Shots (including beams and lobs) resolve in flight. Touch
            // is resolved here, as for the existing hireling actor.
            if self.enemies.iter().any(|e| {
                e.hp > 0
                    && !e.dormant
                    && e.age >= TELEGRAPH
                    && e.frozen == 0
                    && !e.asleep(self.tick)
                    && (e.x - s.actor.x).hypot(e.y - s.actor.y) < e.radius() + HERO_RADIUS
                    && Grid {
                        room: self.room(),
                        barred: self.barred(),
                    }
                    .shot_stop((s.actor.x, s.actor.y), (e.x, e.y))
                    .is_none()
            }) {
                s.actor.hurt(8 + self.dungeon.depth * 2);
            }
        }
        if s.actor.down() {
            return;
        }
        let tier = level(s).min(3);
        if s.squire.kind == Kind::Healer {
            if self.squire_mend(s, heel, tier >= 2) {
                return;
            }
            if let Some(id) = self.squire_patient(s, heel, tier >= 2) {
                let patient = &self.players[&id];
                let goal = (patient.x, patient.y);
                if (goal.0 - s.actor.x).hypot(goal.1 - s.actor.y) > 6.0 {
                    self.squire_walk(s, heel, goal);
                    return;
                }
            }
            if tier >= 3 && self.squire_skull(s, heel) {
                return;
            }
        } else if s.squire.kind == Kind::Alchemist {
            // A potion does not use the bomb's throwing hand/cooldown.
            self.squire_mend(s, heel, tier >= 2);
        }
        let target = self.squire_quarry(s, heel);
        let mut goal = heel;
        if let Some(index) = target {
            let foe = &self.enemies[index];
            let at = (foe.x, foe.y);
            let distance = (at.0 - s.actor.x).hypot(at.1 - s.actor.y);
            match s.squire.kind {
                Kind::Ninja => {
                    // The side away from the foe's chosen knight is its
                    // back. Close on that flank rather than a ranged orbit.
                    let front = self.players.get(&foe.wits.foe).map_or(heel, |h| (h.x, h.y));
                    let away = unit(at.0 - front.0, at.1 - front.1);
                    goal = (at.0 + away.0 * 1.2, at.1 + away.1 * 1.2);
                    if s.actor.cooldown == 0 {
                        if distance <= REACH + foe.radius() {
                            let back = (s.actor.x - at.0) * (front.0 - at.0)
                                + (s.actor.y - at.1) * (front.1 - at.1)
                                < 0.0;
                            self.company_squire_hit(owner, index, s.actor.power * if back { 2 } else { 1 });
                            s.actor.cooldown = HZ / 2;
                        } else if distance <= 7.0 {
                            self.company_squire_shot(
                                owner,
                                s,
                                at,
                                Shot::Blade,
                                s.actor.power / 2 + 1,
                                15.0,
                            );
                            s.actor.cooldown = HZ;
                        }
                    }
                    if tier >= 2 && s.ability == 0 && distance < 4.0 {
                        s.ability = SPECIAL_EVERY;
                        s.effect = if tier >= 3 { HZ } else { HZ / 2 };
                        s.actor.invulnerable = s.actor.invulnerable.max(s.effect);
                        self.cues.push("squire_smoke".into());
                    }
                }
                Kind::Shield => {
                    // Stand between the threat and the owner; unlike an
                    // invincible wall the shield's body pays for each shot.
                    let toward = unit(at.0 - heel.0, at.1 - heel.1);
                    goal = (at.0 - toward.0 * 1.2, at.1 - toward.1 * 1.2);
                    if distance < REACH + foe.radius() && s.actor.cooldown == 0 {
                        self.company_squire_hit(owner, index, s.actor.power);
                        s.actor.cooldown = 3 * HZ / 4;
                    }
                }
                Kind::Ranger => {
                    goal = ranged_goal(s, heel, at, 5.0);
                    if tier >= 2 && s.ability == 0 {
                        s.mark = Some(foe.id);
                        s.effect = if tier >= 3 { 5 * HZ } else { 3 * HZ };
                        s.ability = SPECIAL_EVERY;
                        self.cues.push("squire_mark".into());
                    }
                    if s.actor.cooldown == 0 {
                        let weight = s.actor.power
                            * if s.mark == Some(self.enemies[index].id) {
                                125 + tier.saturating_sub(2) * 25
                            } else {
                                100
                            }
                            / 100;
                        self.company_squire_shot(owner, s, at, Shot::Arrow, weight, 16.0);
                        s.actor.cooldown = HZ;
                    }
                }
                Kind::Alchemist => {
                    goal = ranged_goal(s, heel, at, 4.0);
                    if s.actor.cooldown == 0 {
                        // A short-fused bomb: every blast victim must be
                        // visible from its centre as well as inside the leash.
                        let grid = Grid {
                            room: self.room(),
                            barred: self.barred(),
                        };
                        let victims: Vec<_> = self
                            .enemies
                            .iter()
                            .enumerate()
                            .filter(|(_, e)| {
                                e.hp > 0
                                    && !e.dormant
                                    && e.age >= TELEGRAPH
                                    && e.kind != EnemyKind::Dummy
                                    && (e.x - at.0).hypot(e.y - at.1) <= 2.0 + e.radius()
                                    && (e.x - heel.0).hypot(e.y - heel.1) <= LEASH
                                    && grid.shot_stop(at, (e.x, e.y)).is_none()
                            })
                            .map(|(i, _)| i)
                            .collect();
                        for i in victims {
                            self.company_squire_hit(owner, i, s.actor.power);
                            if self.enemies[i].hp == 0 {
                                self.feel
                                    .blow(self.enemies[i].id, owner, gore::Blow::Blast { at });
                            }
                        }
                        if self.blasts.len() < 16 {
                            self.blasts.push((at.0, at.1, BLAST_TICKS));
                        }
                        self.sounds.push("bomb");
                        s.actor.cooldown = 2 * HZ;
                    }
                }
                Kind::Healer => goal = ranged_goal(s, heel, at, 6.0),
            }
        }
        self.squire_walk(s, heel, goal);
    }

    fn squire_quarry(&self, s: &Companion, heel: (f32, f32)) -> Option<usize> {
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        self.enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.hp > 0
                    && !e.dormant
                    && e.age >= TELEGRAPH
                    && e.kind != EnemyKind::Dummy
                    && (e.x - heel.0).hypot(e.y - heel.1) <= LEASH
                    && grid.shot_stop((s.actor.x, s.actor.y), (e.x, e.y)).is_none()
            })
            .min_by(|(_, a), (_, b)| {
                let marked = |e: &Enemy| if Some(e.id) == s.mark { 0 } else { 1 };
                marked(a)
                    .cmp(&marked(b))
                    .then_with(|| {
                        (a.x - s.actor.x)
                            .hypot(a.y - s.actor.y)
                            .total_cmp(&(b.x - s.actor.x).hypot(b.y - s.actor.y))
                    })
                    .then(a.id.cmp(&b.id))
            })
            .map(|(i, _)| i)
    }

    fn squire_walk(&self, s: &mut Companion, heel: (f32, f32), goal: (f32, f32)) {
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let (dx, dy) = (goal.0 - s.actor.x, goal.1 - s.actor.y);
        if dx.hypot(dy) < 0.8 {
            return;
        }
        let dir = unit(dx, dy);
        let pace = self.tune.hero_speed
            * if s.squire.kind == Kind::Ninja {
                1.15
            } else {
                0.85
            }
            * DT;
        let next = grid.slide(
            (s.actor.x, s.actor.y),
            (dir.0 * pace, dir.1 * pace),
            HERO_RADIUS,
            Mover::Hero,
        );
        let distance = (next.0 - heel.0).hypot(next.1 - heel.1);
        // Do not undo the yielding pass by walking back into the owner
        // every tick. A companion already crowded may still walk out.
        if distance <= LEASH
            && (distance >= BREATHING_ROOM
                || distance >= (s.actor.x - heel.0).hypot(s.actor.y - heel.1))
        {
            (s.actor.x, s.actor.y) = next;
        }
    }

    fn company_squire_hit(&mut self, owner: u32, index: usize, weight: u32) {
        let foe = &mut self.enemies[index];
        let damage = weight.min(foe.hp);
        foe.hp -= damage;
        foe.wits.foe = owner;
        self.feel.credit(owner, foe.id, damage, self.tick);
        if foe.hp == 0 && damage > 0 {
            self.feel.blow(
                foe.id,
                owner,
                gore::Blow::Swing {
                    dir: (0.0, 0.0),
                    forged: false,
                    fists: true,
                    stroke: super::super::melee::Stroke::Fist,
                    combo: 1,
                },
            );
        }
        // Removal, camp casualties, rewards and room completion all stay
        // with the normal end-of-tick settlement, just like minion melee.
    }

    fn company_squire_shot(
        &mut self,
        owner: u32,
        s: &Companion,
        at: (f32, f32),
        kind: Shot,
        damage: u32,
        speed: f32,
    ) {
        if self.projectiles.len() >= MAX_PROJECTILES {
            return;
        }
        let dir = unit(at.0 - s.actor.x, at.1 - s.actor.y);
        self.projectiles.push(Projectile {
            x: s.actor.x,
            y: s.actor.y,
            vx: dir.0 * speed,
            vy: dir.1 * speed,
            hostile: false,
            kind,
            look: None,
            damage,
            pierce: 0,
            last_hit: None,
            ttl: HZ,
            empowered: false,
            traits: ShotTraits {
                owner: owner as u8,
                ..Default::default()
            },
        });
    }

    fn squire_patient(&self, s: &Companion, heel: (f32, f32), cleanse: bool) -> Option<u32> {
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        self.players
            .iter()
            .filter(|(_, h)| {
                h.hp > 0
                    && !h.absent
                    && !h.stone
                    && (h.hp < h.max_hp
                        || (cleanse && (h.hexed > 0 || h.chilled > 0 || h.mired > 0)))
                    && (h.x - heel.0).hypot(h.y - heel.1) <= LEASH
                    && grid.shot_stop((s.actor.x, s.actor.y), (h.x, h.y)).is_none()
            })
            .min_by(|(aid, a), (bid, b)| {
                (u64::from(a.hp) * u64::from(b.max_hp.max(1)))
                    .cmp(&(u64::from(b.hp) * u64::from(a.max_hp.max(1))))
                    .then(aid.cmp(bid))
            })
            .map(|(&id, _)| id)
    }

    fn squire_mend(&mut self, s: &mut Companion, heel: (f32, f32), cleanse: bool) -> bool {
        let potion = s.squire.kind == Kind::Alchemist;
        if (potion && s.ability > 0) || (!potion && s.actor.cooldown > 0) {
            return false;
        }
        let patient = self.squire_patient(s, heel, cleanse).filter(|id| {
            let h = &self.players[id];
            (h.x - s.actor.x).hypot(h.y - s.actor.y) <= 6.0
        });
        let Some(id) = patient else { return false };
        let trained = level(s);
        let hero = self.players.get_mut(&id).unwrap();
        // This is a scheduled party mend, not passive lifesteal: the
        // leech budget would flatten every paid level to two HP.
        hero.mend(6 + trained * 3, stakes::Mend::Party);
        if cleanse {
            hero.hexed = 0;
            hero.chilled = 0;
            hero.mired = 0;
        }
        if potion {
            s.ability = SPECIAL_EVERY;
        } else {
            s.actor.cooldown = MEND_EVERY;
        }
        self.cues.push("squire_mend".into());
        true
    }

    /// A third-tier healer ferries a settled skull toward a standing
    /// friend, or wards a nearby carrier. It cannot raise the fallen:
    /// E and a weakened monster are still needed by the skull rules.
    fn squire_skull(&mut self, s: &mut Companion, heel: (f32, f32)) -> bool {
        if s.ability > 0 {
            return false;
        }
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let skull = self
            .players
            .iter()
            .filter(|(_, h)| {
                h.hp == 0
                    && !h.absent
                    && h.fell != 0
                    && self.tick.saturating_sub(h.fell) >= skulls::SKULL_SETTLE
            })
            .filter_map(|(&id, _)| self.skull_lying(id).map(|at| (id, at)))
            .find(|(_, at)| {
                (at.0 - heel.0).hypot(at.1 - heel.1) <= LEASH
                    && grid.shot_stop((s.actor.x, s.actor.y), *at).is_none()
            });
        if let Some((id, at)) = skull {
            if (at.0 - s.actor.x).hypot(at.1 - s.actor.y) > skulls::SKULL_REACH {
                self.squire_walk(s, heel, at);
                return true;
            }
            let friend = self
                .players
                .iter()
                .filter(|(_, h)| {
                    h.hp > 0
                        && !h.absent
                        && !h.stone
                        && h.carrying_skull.is_none()
                        && (h.x - heel.0).hypot(h.y - heel.1) <= LEASH
                        && grid.shot_stop(at, (h.x, h.y)).is_none()
                })
                .min_by(|(aid, a), (bid, b)| {
                    (a.x - at.0)
                        .hypot(a.y - at.1)
                        .total_cmp(&(b.x - at.0).hypot(b.y - at.1))
                        .then(aid.cmp(bid))
                })
                .map(|(_, h)| (h.x, h.y));
            if let Some(friend) = friend {
                let dir = unit(friend.0 - at.0, friend.1 - at.1);
                let step = 1.0_f32.min((friend.0 - at.0).hypot(friend.1 - at.1));
                let next = grid.slide(at, (dir.0 * step, dir.1 * step), HERO_RADIUS, Mover::Hero);
                self.players.get_mut(&id).unwrap().skull_at = Some(next);
                s.ability = HZ;
                self.cues.push("squire_skull".into());
                return true;
            }
        }
        let carrier = self
            .players
            .iter()
            .find(|(_, h)| {
                h.hp > 0
                    && !h.absent
                    && h.carrying_skull.is_some()
                    && (h.x - heel.0).hypot(h.y - heel.1) <= LEASH
                    && (h.x - s.actor.x).hypot(h.y - s.actor.y) <= 6.0
                    && grid.shot_stop((s.actor.x, s.actor.y), (h.x, h.y)).is_none()
            })
            .map(|(&id, _)| id);
        if let Some(id) = carrier {
            let hero = self.players.get_mut(&id).unwrap();
            hero.invulnerable = hero.invulnerable.max(HZ / 2);
            s.ability = SPECIAL_EVERY;
            return true;
        }
        false
    }
}

impl Companion {
    /// Smoke hides only the third-tier ninja; all other standing squires
    /// are ordinary targets, including the healer at the back of the fight.
    pub fn targetable(&self) -> bool {
        !self.actor.down()
            && !(self.squire.kind == Kind::Ninja && level(self) >= 3 && self.effect > 0)
    }
}

fn ranged_goal(s: &Companion, heel: (f32, f32), foe: (f32, f32), keep: f32) -> (f32, f32) {
    let dir = unit(heel.0 - foe.0, heel.1 - foe.1);
    let goal = (foe.0 + dir.0 * keep, foe.1 + dir.1 * keep);
    if (goal.0 - heel.0).hypot(goal.1 - heel.1) <= LEASH {
        goal
    } else {
        (s.actor.x, s.actor.y)
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/together_squire_field__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/rules/together_squire_measure__tests.rs"]
mod measure_tests;
