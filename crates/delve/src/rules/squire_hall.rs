//! Permanent companies use the realm's book, the town's hold-E plates and
//! the hireling's body. They neither take loot nor occupy summon slots.
use super::*;
use crate::realm::squires::{Building, Kind, Squire};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Companion {
    pub owner: u32,
    pub squire: Squire,
    pub body: hireling::Hireling,
    #[serde(default)]
    pub aid: u32,
    #[serde(default)]
    pub smoke: u32,
    #[serde(default)]
    pub mark: Option<(u32, u32)>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Party {
    pub companions: Vec<Companion>,
    pub offers: BTreeMap<u32, usize>,
    /// Which action owns a seat's partial hold; ordinary yard counters must
    /// not carry progress into a different treasury-spending action.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub holds: BTreeMap<u32, usize>,
}
impl Party {
    pub fn is_empty(&self) -> bool {
        self.companions.is_empty() && self.offers.is_empty() && self.holds.is_empty()
    }
    pub(super) fn decoys<'a>(&'a self, run: &'a Run) -> impl Iterator<Item = (f32, f32)> + 'a {
        self.companions
            .iter()
            .filter(|c| run.squire_owner_here(c.owner) && !c.body.down() && c.smoke == 0)
            .map(|c| (c.body.x, c.body.y))
    }
    pub fn valid(&self) -> bool {
        let mut owners = std::collections::BTreeSet::new();
        self.companions.iter().all(|c| {
            owners.insert(c.owner)
                && c.body.x.is_finite()
                && c.body.y.is_finite()
                && c.body.hp <= c.body.max_hp
                && c.body.max_hp > 0
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Command {
    Refresh(u64),
    Hire(String, u32),
    Train(String, u32),
    Select(String, Option<u32>),
    Upgrade(u8),
}

/// Nine small plates on the north side of the existing Training Yard.
/// Each building has two daily actions and one improvement plate.
pub const PLATES: [(i32, i32, i32, i32); 9] = [
    (2, 4, 2, 1),
    (4, 4, 2, 1),
    (6, 4, 2, 1),
    (9, 4, 2, 1),
    (11, 4, 2, 1),
    (13, 4, 2, 1),
    (16, 4, 2, 1),
    (18, 4, 2, 1),
    (20, 4, 2, 1),
];
pub const LABELS: [&str; 9] = [
    "Next", "Hire", "Build", "Select", "Rest", "Build", "Train", "Roster", "Build",
];
fn clear_line(room: &Room, a: (f32, f32), b: (f32, f32)) -> bool {
    let grid = Grid { room, barred: true };
    let steps = ((b.0 - a.0).hypot(b.1 - a.1) * 2.0).ceil().max(1.0) as u32;
    (1..steps).all(|i| {
        let t = i as f32 / steps as f32;
        grid.open(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, Mover::Hero)
    })
}
pub fn plate_at(x: f32, y: f32) -> Option<usize> {
    PLATES.iter().position(|&(c, r, w, h)| {
        x >= c as f32 * TILE_UNITS
            && x < (c + w) as f32 * TILE_UNITS
            && y >= r as f32 * TILE_UNITS
            && y < (r + h) as f32 * TILE_UNITS
    })
}
fn building(i: u8) -> Building {
    match i {
        0 => Building::Recruit,
        1 => Building::Hire,
        _ => Building::Train,
    }
}

/// Same authority boundary as holdings and town sales: only the host's
/// realm settles a request and spends its treasury. Names are JSON strings,
/// so punctuation in a knight's name cannot turn into another command.
pub fn settle(realm: &mut activity::Realm<'_>, marks: &BTreeMap<String, u32>) -> activity::Settled {
    let mut result = activity::Settled::default();
    for key in marks.keys().filter(|k| k.starts_with("squire:")) {
        let Some(json) = key.splitn(4, ':').nth(3) else {
            continue;
        };
        let Ok(command) = serde_json::from_str::<Command>(json) else {
            continue;
        };
        let before = realm.home.squire_hall.clone();
        let c = &mut realm.home.squire_hall;
        let outcome = match command {
            Command::Refresh(day) => {
                c.refresh(day);
                Ok(())
            }
            Command::Hire(who, id) => c.hire(&who, id, realm.treasury),
            Command::Train(who, id) => c.train(&who, id, realm.treasury),
            Command::Select(who, id) => c.select(&who, id),
            Command::Upgrade(i) => c.upgrade(building(i), realm.treasury),
        };
        result.changed |= before != *c;
        result.said.push((
            "squires".into(),
            outcome.map_or_else(str::to_owned, |_| "Squire company updated".into()),
        ));
    }
    result
}
impl Run {
    /// Companions share their owner's activity and presence, including while
    /// outside seats are temporarily detached during `step`.
    pub(crate) fn squire_owner_here(&self, owner: u32) -> bool {
        self.players.get(&owner).is_some_and(|h| !h.absent)
            && !self.exits.outside.contains_key(&owner)
    }

    fn squire_order(&mut self, id: u32, command: Command) {
        let key = format!(
            "squire:{:020}:{id:010}:{}",
            self.tick,
            serde_json::to_string(&command).unwrap()
        );
        self.marks.insert(key, 1);
    }
    pub(super) fn tick_squire_hall(&mut self, inputs: &BTreeMap<u32, Input>) {
        let day = self.home.squire_hall.cadence;
        if !self.home.squire_hall.ready {
            self.home.squire_hall.refresh(day);
            self.squire_order(0, Command::Refresh(day));
        }
        let mut actions = Vec::new();
        for (&id, hero) in &mut self.players {
            if super::squires::action_at(hero.x, hero.y).is_some() {
                if self.squire_hall.holds.remove(&id).is_some() { hero.buying = 0; }
                continue;
            }
            let pressing = inputs
                .get(&id)
                .filter(|i| i.valid())
                .is_some_and(|i| i.interact);
            let previous = self.squire_hall.holds.remove(&id);
            if !pressing {
                hero.buy_spent = false;
            }
            let Some(plate) = plate_at(hero.x, hero.y) else {
                if previous.is_some() {
                    hero.buying = 0;
                }
                continue;
            };
            self.squire_hall.holds.insert(id, plate);
            if previous != Some(plate) || !pressing || hero.hp == 0 || hero.stone {
                hero.buying = 0;
            }
            if hero.hp == 0 || hero.stone || !pressing || hero.buy_spent {
                continue;
            }
            hero.buying += 1;
            if hero.buying >= home::BUY_HOLD {
                hero.buying = 0;
                hero.buy_spent = true;
                actions.push((id, home::Home::knight_key(hero), plate));
            }
        }
        for (id, who, plate) in actions {
            let c = &self.home.squire_hall;
            let roster = c.rosters.get(&who);
            let active = roster.and_then(|r| r.active);
            let next = || {
                roster.and_then(|r| {
                    if r.squires.is_empty() {
                        return None;
                    }
                    let i = r
                        .squires
                        .iter()
                        .position(|s| Some(s.id) == r.active)
                        .map_or(0, |i| (i + 1) % r.squires.len());
                    Some(r.squires[i].id)
                })
            };
            let command = match plate {
                0 => {
                    let n = c.offers.len().max(1);
                    let cursor = self.squire_hall.offers.entry(id).or_default();
                    *cursor = (*cursor + 1) % n;
                    None
                }
                1 => c
                    .offers
                    .get(self.squire_hall.offers.get(&id).copied().unwrap_or(0) % c.offers.len().max(1))
                    .map(|s| Command::Hire(who.clone(), s.id)),
                2 => Some(Command::Upgrade(0)),
                3 | 7 => Some(Command::Select(who.clone(), next())),
                4 => Some(Command::Select(who.clone(), None)),
                5 => Some(Command::Upgrade(1)),
                6 => active.map(|s| Command::Train(who.clone(), s)),
                8 => Some(Command::Upgrade(2)),
                _ => None,
            };
            if let Some(command) = command {
                self.squire_order(id, command);
            }
        }
    }
    /// A room transition carries each selected companion through beside its
    /// owner. Downed companions recover here, or when the fight is over.
    pub(super) fn hall_squires_follow(&mut self) {
        for (owner, _) in std::mem::take(&mut self.squire_yard_holds) {
            if let Some(h) = self.players.get_mut(&owner) { h.buying = 0; }
        }
        for (owner, _) in std::mem::take(&mut self.squire_hall.holds) {
            if let Some(h) = self.players.get_mut(&owner) {
                h.buying = 0;
            }
        }
        if self.dungeon.depth == 0 {
            self.squire_hall.companions.clear();
            return;
        }
        let mut old = std::mem::take(&mut self.squire_hall.companions);
        for (&owner, hero) in self
            .players
            .iter()
            .filter(|(id, h)| !h.absent && !self.exits.outside.contains_key(id))
        {
            let who = home::Home::knight_key(hero);
            let Some(s) = self
                .home
                .squire_hall
                .rosters
                .get(&who)
                .and_then(|r| r.squires.iter().find(|s| Some(s.id) == r.active))
                .cloned()
            else {
                continue;
            };
            let grid = Grid {
                room: self.room(),
                barred: true,
            };
            let at = hireling::beside(&grid, (hero.x, hero.y));
            let mut c = if let Some(i) = old
                .iter()
                .position(|c| c.owner == owner && c.squire.id == s.id)
            {
                old.remove(i)
            } else {
                let mut body = hireling::Hireling::minion(
                    hireling::AllyKind::Beaumains,
                    0,
                    self.dungeon.depth,
                    at,
                    owner,
                );
                body.max_hp = s.health();
                body.hp = body.max_hp;
                Companion {
                    owner,
                    squire: s,
                    body,
                    aid: 0,
                    smoke: 0,
                    mark: None,
                }
            };
            (c.body.x, c.body.y) = at;
            if c.body.down() {
                c.body.hp = (c.body.max_hp / 2).max(1);
            }
            c.body.invulnerable = HZ;
            c.mark = None;
            self.squire_hall.companions.push(c);
        }
    }
    pub(super) fn squire_delve(&mut self) {
        if !self.home.squire_hall.ready {
            return;
        }
        let day = self.home.squire_hall.cadence.saturating_add(1);
        self.home.squire_hall.refresh(day);
        self.squire_order(0, Command::Refresh(day));
    }
    pub(super) fn tick_hall_squires(&mut self) {
        if self.dungeon.depth == 0 || self.at_home_now() {
            return;
        }
        let mut companions = std::mem::take(&mut self.squire_hall.companions);
        for c in &mut companions {
            self.drive_hall_squire(c);
        }
        self.squire_hall.companions = companions;
    }
    fn hall_squire_hit(&mut self, c: &Companion, index: usize, damage: u32) {
        let foe = &mut self.enemies[index];
        if foe.hp == 0 || foe.buried() {
            return;
        }
        let dealt = damage.min(foe.hp);
        foe.hp -= dealt;
        foe.wits.foe = c.owner;
        self.feel.credit(c.owner, foe.id, dealt, self.tick);
    }
    fn drive_hall_squire(&mut self, c: &mut Companion) {
        if !self.squire_owner_here(c.owner) {
            return;
        }
        let owner = &self.players[&c.owner];
        let heel = (owner.x, owner.y);
        c.body.invulnerable = c.body.invulnerable.saturating_sub(1);
        c.body.cooldown = c.body.cooldown.saturating_sub(1);
        c.aid = c.aid.saturating_sub(1);
        c.smoke = c.smoke.saturating_sub(1);
        if let Some((_, ttl)) = &mut c.mark {
            *ttl = ttl.saturating_sub(1);
            if *ttl == 0 {
                c.mark = None;
            }
        }
        if c.body.down() {
            if !self.fight_on() {
                c.body.hp = (c.body.max_hp / 2).max(1);
                c.body.invulnerable = HZ;
            }
            return;
        }
        let kind = c.squire.kind;
        let level = c.squire.level;
        let power = c.squire.power();
        // Flying shots meet companions in `flight`, in swept entry order.
        // Beams burn in place: each beam can strike each owner's companion
        // once, independently of its knight and the companion's hit ward.
        if c.smoke == 0 {
            for shot in self.projectiles.iter_mut().filter(|p| {
                p.hostile && p.ttl > 0 && !p.traits.path.waiting() && p.traits.path.beam.is_some()
            }) {
                let beam = shot.traits.path.beam.as_ref().unwrap();
                if !shot.traits.squire_struck.contains(&c.owner)
                    && beam.burning(shot.traits.path.age)
                    && beam.crosses((shot.x, shot.y), (c.body.x, c.body.y))
                {
                    shot.traits.squire_struck.push(c.owner);
                    c.body.hurt(if kind == Kind::Shield {
                        shot.damage.div_ceil(2)
                    } else {
                        shot.damage
                    });
                }
            }
        }
        if c.smoke == 0
            && self.enemies.iter().any(|e| {
                e.hp > 0
                    && e.hurts_by_touch()
                    && e.kind != EnemyKind::Dummy
                    && e.age >= TELEGRAPH
                    && e.frozen == 0
                    && !e.asleep(self.tick)
                    && (e.x - c.body.x).hypot(e.y - c.body.y) < e.radius() + 0.5
            })
        {
            c.body.hurt(if kind == Kind::Shield { 6 } else { 12 });
        }
        if c.body.down() {
            return;
        }
        let target = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.hp > 0
                    && !e.buried()
                    && e.kind != EnemyKind::Dummy
                    && (e.x - heel.0).hypot(e.y - heel.1) < 22.0
            })
            .min_by(|(_, a), (_, b)| {
                (a.x - c.body.x)
                    .hypot(a.y - c.body.y)
                    .total_cmp(&(b.x - c.body.x).hypot(b.y - c.body.y))
            })
            .map(|(i, e)| (i, e.x, e.y));
        let mut goal = heel;
        let mut pace = 0.85;
        if let Some((i, x, y)) = target {
            let d = (x - c.body.x).hypot(y - c.body.y);
            let u = unit(x - c.body.x, y - c.body.y);
            c.body.facing = u.0;
            match kind {
                Kind::Ninja => {
                    pace = 1.35;
                    let back = unit(x - heel.0, y - heel.1);
                    goal = (x + back.0 * 1.7, y + back.1 * 1.7);
                    if level >= 3 && c.aid == 0 && d < 5.0 {
                        c.smoke = HZ;
                        c.aid = 6 * HZ;
                    }
                    if c.body.cooldown == 0 {
                        let behind =
                            (c.body.x - x) * (heel.0 - x) + (c.body.y - y) * (heel.1 - y) < 0.0;
                        if level >= 2
                            && d < 2.6
                            && behind
                            && clear_line(self.room(), (c.body.x, c.body.y), (x, y))
                        {
                            self.hall_squire_hit(c, i, power * 3);
                            c.body.cooldown = HZ / 2;
                        } else if d < 14.0 {
                            self.hall_squire_shot(c, (x, y), Shot::Blade, power, 18.0);
                        }
                    }
                }
                Kind::Healer => {
                    goal = (x - u.0 * 9.0, y - u.1 * 9.0);
                }
                Kind::Ranger => {
                    goal = (x - u.0 * 10.0, y - u.1 * 10.0);
                    if level >= 2 && c.aid == 0 {
                        c.mark = Some((self.enemies[i].id, 3 * HZ));
                        c.aid = 5 * HZ;
                    }
                    if c.body.cooldown == 0 && d < 19.0 {
                        self.hall_squire_shot(
                            c,
                            (x, y),
                            Shot::Arrow,
                            power
                                + if c.mark.is_some_and(|(id, _)| id == self.enemies[i].id) {
                                    power / 2
                                } else {
                                    0
                                },
                            23.0,
                        );
                    }
                }
                Kind::Shield => {
                    // Stand on the incoming line, between owner and foe.
                    let line = unit(x - heel.0, y - heel.1);
                    goal = (heel.0 + line.0 * 2.0, heel.1 + line.1 * 2.0);
                    if d < 2.7
                        && c.body.cooldown == 0
                        && clear_line(self.room(), (c.body.x, c.body.y), (x, y))
                    {
                        self.hall_squire_hit(c, i, power);
                        self.enemies[i].frozen = self.enemies[i].frozen.max(HZ / 4);
                        c.body.cooldown = HZ;
                    }
                    if level >= 2
                        && c.aid == 0
                        && (c.body.x - heel.0).hypot(c.body.y - heel.1) < 3.5
                    {
                        let h = self.players.get_mut(&c.owner).unwrap();
                        h.invulnerable = h.invulnerable.max(HZ / 3);
                        c.aid = 5 * HZ;
                    }
                }
                Kind::Alchemist => {
                    goal = (x - u.0 * 7.0, y - u.1 * 7.0);
                    if c.body.cooldown == 0
                        && d < 12.0
                        && clear_line(self.room(), (c.body.x, c.body.y), (x, y))
                    {
                        let hits = aoe::reached(&self.enemies, (x, y), |e| {
                            !e.buried()
                                && (e.x - x).hypot(e.y - y) < 2.8 + e.radius()
                                && clear_line(self.room(), (x, y), (e.x, e.y))
                        });
                        for (rank, j) in hits.into_iter().enumerate() {
                            self.hall_squire_hit(c, j, aoe::part(power, rank));
                        }
                        c.body.cooldown = 2 * HZ;
                        self.boom(x, y, skills::BoomKind::Flash, 2.8);
                        self.sounds.push("bomb");
                    }
                }
            }
        }
        // Revival is movement intent, not an ability cooldown. Ordinary
        // mending may postpone the revive, but must never abandon the skull.
        if kind == Kind::Healer && level >= 3 {
            let from = (c.body.x, c.body.y);
            let fallen = self
                .players
                .iter()
                .filter(|(id, h)| h.hp == 0 && !h.absent && !self.exits.outside.contains_key(id))
                .filter_map(|(&id, _)| self.skull_lying(id).map(|at| (id, at)))
                .filter(|(_, at)| clear_line(self.room(), from, *at))
                .min_by(|(ai, a), (bi, b)| {
                    (a.0 - from.0)
                        .hypot(a.1 - from.1)
                        .total_cmp(&(b.0 - from.0).hypot(b.1 - from.1))
                        .then(ai.cmp(bi))
                });
            if let Some((id, at)) = fallen {
                goal = at;
                if c.aid == 0 && (from.0 - at.0).hypot(from.1 - at.1) < 2.0 {
                    let h = self.players.get_mut(&id).unwrap();
                    h.hp = (h.max_hp / 5).max(1);
                    h.x = at.0;
                    h.y = at.1;
                    h.skull_at = None;
                    h.invulnerable = 2 * HZ;
                    c.aid = 12 * HZ;
                }
            }
        }
        if kind == Kind::Healer || (kind == Kind::Alchemist && level >= 2) {
            if c.aid == 0 {
                let wounded = self
                    .players
                    .iter()
                    .filter(|(_, h)| {
                        !h.stone
                            && h.privy == 0
                            && clear_line(self.room(), (c.body.x, c.body.y), (h.x, h.y))
                            && h.hp > 0
                            && !h.absent
                            && (h.hp < h.max_hp
                                || (level >= 2 && (h.hexed > 0 || h.chilled > 0 || h.mired > 0)))
                            && (h.x - c.body.x).hypot(h.y - c.body.y) < 12.0
                    })
                    .min_by(|(ai, a), (bi, b)| {
                        (u64::from(a.hp) * u64::from(b.max_hp))
                            .cmp(&(u64::from(b.hp) * u64::from(a.max_hp)))
                            .then(ai.cmp(bi))
                    })
                    .map(|(&id, _)| id);
                if let Some(id) = wounded {
                    let h = self.players.get_mut(&id).unwrap();
                    h.mend(power, stakes::Mend::Party);
                    if kind == Kind::Healer && level >= 2 {
                        h.hexed = 0;
                        h.chilled = 0;
                        h.mired = 0;
                    }
                    c.aid = 2 * HZ;
                }
            }
        }
        let delta = (goal.0 - c.body.x, goal.1 - c.body.y);
        if delta.0.hypot(delta.1) > 1.0 {
            let u = unit(delta.0, delta.1);
            let step = HERO_SPEED / HZ as f32 * pace;
            let grid = Grid {
                room: self.room(),
                barred: true,
            };
            (c.body.x, c.body.y) = grid.slide(
                (c.body.x, c.body.y),
                (u.0 * step, u.1 * step),
                HERO_RADIUS * 0.7,
                Mover::Hero,
            );
        }
    }
    fn hall_squire_shot(
        &mut self,
        c: &mut Companion,
        at: (f32, f32),
        kind: Shot,
        damage: u32,
        speed: f32,
    ) {
        if self.projectiles.len() >= MAX_PROJECTILES {
            return;
        }
        let u = unit(at.0 - c.body.x, at.1 - c.body.y);
        self.projectiles.push(Projectile {
            x: c.body.x,
            y: c.body.y,
            vx: u.0 * speed,
            vy: u.1 * speed,
            hostile: false,
            look: None,
            kind,
            damage,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: ShotTraits {
                owner: u8::try_from(c.owner).unwrap_or(0),
                squire: true,
                ..Default::default()
            },
            ttl: 2 * HZ,
        });
        c.body.cooldown = if c.squire.kind == Kind::Ninja {
            HZ / 2
        } else {
            HZ
        };
    }
}
#[cfg(test)]
#[path = "../../tests/rules/squire_hall__tests.rs"]
mod tests;
