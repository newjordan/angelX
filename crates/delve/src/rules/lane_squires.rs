//! A knight's company: recruited in the yard, kept in the realm's Home.
//! Squires never take a summoned ally's place, carry loot, or count as a
//! living knight. Each knight may bring one; a fallen squire rests a room.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Ninja,
    Healer,
    Ranger,
    Shield,
    Alchemist,
}
impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Ninja,
        Self::Healer,
        Self::Ranger,
        Self::Shield,
        Self::Alchemist,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ninja => "NINJA",
            Self::Healer => "HEALER",
            Self::Ranger => "RANGER",
            Self::Shield => "SHIELD",
            Self::Alchemist => "ALCHEMIST",
        }
    }
    pub fn health(self, level: u8) -> u32 {
        (if self == Self::Shield { 85 } else { 45 }) + u32::from(level) * 12
    }
    pub fn power(self, level: u8) -> u32 {
        (if self == Self::Ninja { 10 } else { 8 }) + u32::from(level) * 4
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recruit {
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub level: u8,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roster {
    #[serde(default)]
    pub members: Vec<Recruit>,
    #[serde(default)]
    pub active: Option<u32>,
    #[serde(default)]
    pub drills: BTreeMap<u32, u32>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    #[serde(default)]
    pub board: u8,
    #[serde(default)]
    pub hall: u8,
    #[serde(default)]
    pub yard: u8,
    #[serde(default)]
    pub day: u32,
    #[serde(default)]
    pub offers: Vec<Recruit>,
    #[serde(default)]
    pub knights: BTreeMap<String, Roster>,
    #[serde(default)]
    pub next_id: u32,
    #[serde(default)]
    pub season: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Building {
    Board,
    Hall,
    Yard,
}
impl Company {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn cap(&self) -> usize {
        3 + self.hall.min(3) as usize
    }
    pub fn max_level(&self) -> u8 {
        2 + self.yard.min(3) * 2
    }
    pub fn offer_count(&self) -> usize {
        3 + self.board.min(3) as usize
    }
    pub fn wage(&self, recruit: &Recruit) -> u32 {
        (90 + u32::from(recruit.level) * 30) * (100 - u32::from(self.hall.min(3)) * 15) / 100
    }
    pub fn level(&self, building: Building) -> u8 {
        match building {
            Building::Board => self.board,
            Building::Hall => self.hall,
            Building::Yard => self.yard,
        }
        .min(3)
    }
    pub fn upgrade_price(&self, building: Building) -> Option<u32> {
        let level = self.level(building);
        (level < 3).then_some(180 * (u32::from(level) + 1))
    }
    pub fn upgrade(&mut self, building: Building, gold: &mut u32) -> bool {
        let Some(price) = self.upgrade_price(building) else {
            return false;
        };
        if *gold < price {
            return false;
        }
        *gold -= price;
        match building {
            Building::Board => self.board += 1,
            Building::Hall => self.hall += 1,
            Building::Yard => self.yard += 1,
        }
        true
    }
    /// One board per delve, stable across reconnects and every client's step.
    pub fn refresh(&mut self, season: u64) {
        if self.season == Some(season) {
            return;
        }
        self.season = Some(season);
        self.day = self.day.saturating_add(1);
        self.offers.clear();
        for roster in self.knights.values_mut() {
            for recruit in &roster.members {
                let drills = roster.drills.entry(recruit.id).or_default();
                *drills = drills
                    .saturating_add(1 + u32::from(self.yard.min(3)))
                    .min(8);
            }
        }
        const NAMES: [&str; 12] = [
            "Pip", "Nell", "Ash", "Kit", "Wren", "Bram", "Moss", "Tansy", "Rook", "Fern", "Hob",
            "Lark",
        ];
        for n in 0..self.offer_count() {
            self.next_id = self.next_id.saturating_add(1);
            let roll = season
                .wrapping_mul(6364136223846793005)
                .wrapping_add(u64::from(self.day) * 17 + n as u64);
            // The first board rotates all five trades; better boards add
            // choices and trained recruits, rather than locking healing out.
            self.offers.push(Recruit {
                id: self.next_id,
                name: NAMES[(roll % NAMES.len() as u64) as usize].into(),
                kind: Kind::ALL[(roll % 5) as usize],
                level: 1 + self.board.min(3) / 2,
            });
        }
    }
    pub fn hire(&mut self, who: &str, offer: usize, gold: &mut u32) -> bool {
        let Some(recruit) = self.offers.get(offer).cloned() else {
            return false;
        };
        let price = self.wage(&recruit);
        if *gold < price
            || self
                .knights
                .get(who)
                .is_some_and(|r| r.members.len() >= self.cap())
        {
            return false;
        }
        *gold -= price;
        self.offers.remove(offer);
        let roster = self.knights.entry(who.into()).or_default();
        roster.drills.insert(recruit.id, 1);
        roster.members.push(recruit);
        true
    }
    pub fn select(&mut self, who: &str, member: usize) -> bool {
        let Some(roster) = self.knights.get_mut(who) else {
            return false;
        };
        let Some(recruit) = roster.members.get(member) else {
            return false;
        };
        roster.active = if roster.active == Some(recruit.id) {
            None
        } else {
            Some(recruit.id)
        };
        true
    }
    pub fn training_price(&self, recruit: &Recruit) -> u32 {
        45 * u32::from(recruit.level)
    }
    pub fn train(&mut self, who: &str, gold: &mut u32) -> bool {
        let max = self.max_level();
        let Some(roster) = self.knights.get_mut(who) else {
            return false;
        };
        let Some(recruit) = roster
            .members
            .iter_mut()
            .find(|r| Some(r.id) == roster.active)
        else {
            return false;
        };
        let price = 45 * u32::from(recruit.level);
        let drills = roster.drills.entry(recruit.id).or_default();
        if recruit.level >= max || *gold < price || *drills == 0 {
            return false;
        }
        *gold -= price;
        *drills -= 1;
        recruit.level += 1;
        true
    }
    pub fn active(&self, who: &str) -> Option<&Recruit> {
        let roster = self.knights.get(who)?;
        roster.members.iter().find(|r| Some(r.id) == roster.active)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Squire {
    pub owner: u32,
    pub recruit: Recruit,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub cooldown: u32,
    pub grace: u32,
    pub smoke: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Hire(usize),
    Select(usize),
    Upgrade(Building),
    Train,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    Refresh(u64),
    Act(String, Action),
}

pub fn plates(company: &Company) -> Vec<(Action, (i32, i32, i32, i32))> {
    let mut out = Vec::new();
    for n in 0..company.offer_count() {
        out.push((Action::Hire(n), (2 + n as i32 * 3, 3, 2, 1)));
    }
    for (building, x) in [
        (Building::Board, 2),
        (Building::Hall, 10),
        (Building::Yard, 18),
    ] {
        out.push((Action::Upgrade(building), (x, 5, 3, 1)));
    }
    for n in 0..company.cap() {
        out.push((Action::Select(n), (2 + n as i32 * 2, 9, 2, 1)));
    }
    out.push((Action::Train, (10, 11, 4, 1)));
    out
}

pub fn action_at(company: &Company, x: f32, y: f32) -> Option<Action> {
    plates(company)
        .into_iter()
        .find(|(_, (c, r, w, h))| {
            let (x, y) = (x / TILE_UNITS, y / TILE_UNITS);
            x >= *c as f32 && x < (c + w) as f32 && y >= *r as f32 && y < (r + h) as f32
        })
        .map(|(a, _)| a)
}

impl Company {
    pub fn act(&mut self, who: &str, action: Action, gold: &mut u32) -> bool {
        match action {
            Action::Hire(n) => self.hire(who, n, gold),
            Action::Select(n) => self.select(who, n),
            Action::Upgrade(b) => self.upgrade(b, gold),
            Action::Train => self.train(who, gold),
        }
    }
    /// Replay the sim's requests against the real treasury, in player order.
    pub fn settle(&mut self, treasury: &mut Spoils, orders: &[Order]) -> bool {
        if orders.is_empty() {
            return false;
        }
        let before = self.clone();
        let initial_gold = treasury.get(crate::realm::Spoil::Gold);
        let mut gold = initial_gold;
        for order in orders {
            match order {
                Order::Refresh(seed) => self.refresh(*seed),
                Order::Act(who, action) => {
                    self.act(who, *action, &mut gold);
                }
            }
        }
        if gold != initial_gold {
            treasury.0.insert(crate::realm::Spoil::Gold, gold);
        }
        *self != before
    }
}

impl Run {
    pub(super) fn tick_lane_squire_yard(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::SquireLanes {
            self.lane_squire_holds.clear();
            return;
        }
        let season = mix(self.seed ^ self.raid_id.wrapping_mul(0x5851_f42d));
        if self.home.lane_squires.season != Some(season) {
            self.home.lane_squires.refresh(season);
            self.lane_squire_orders.push(Order::Refresh(season));
        }
        for (&id, hero) in &self.players {
            let pressing = inputs.get(&id).is_some_and(|i| i.interact)
                && !self.exits.outside.contains_key(&id);
            let action = action_at(&self.home.lane_squires, hero.x, hero.y);
            if !pressing || hero.hp == 0 || hero.away || hero.stone || action.is_none() {
                self.lane_squire_holds.remove(&id);
                continue;
            }
            let action = action.unwrap();
            let hold = self.lane_squire_holds.entry(id).or_insert((action, 0));
            if hold.0 != action {
                *hold = (action, 0);
            }
            hold.1 = hold.1.saturating_add(1);
            if hold.1 != home::BUY_HOLD {
                continue;
            }
            let who = home::Home::knight_key(hero);
            let mut gold = self.treasury.get(crate::realm::Spoil::Gold);
            if self.home.lane_squires.act(&who, action, &mut gold) {
                self.treasury.0.insert(crate::realm::Spoil::Gold, gold);
                self.lane_squire_orders.push(Order::Act(who, action));
                self.cues.push("squire:ready".into());
            } else {
                self.cues.push("squire:not_ready".into());
            }
        }
    }

    pub(super) fn lane_squires_follow(&mut self) {
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        for squire in &mut self.lane_squires {
            let Some(hero) = self.players.get(&squire.owner) else {
                continue;
            };
            (squire.x, squire.y) = hireling::beside(&grid, (hero.x, hero.y));
            squire.hp = squire.recruit.kind.health(squire.recruit.level);
            squire.grace = HZ;
        }
    }

    pub(super) fn tick_lane_squires(&mut self) {
        if self.at_home_now() {
            self.lane_squires.clear();
            return;
        }
        let selected: Vec<_> = self
            .players
            .iter()
            .filter(|(id, h)| !h.away && !h.stone && !self.exits.outside.contains_key(id))
            .filter_map(|(&owner, h)| {
                self.home
                    .lane_squires
                    .active(&home::Home::knight_key(h))
                    .cloned()
                    .map(|r| (owner, r))
            })
            .collect();
        self.lane_squires.retain(|s| {
            selected
                .iter()
                .any(|(id, r)| *id == s.owner && r.id == s.recruit.id)
        });
        for (owner, recruit) in selected {
            if self.lane_squires.iter().any(|s| s.owner == owner) {
                continue;
            }
            let hero = &self.players[&owner];
            let grid = Grid {
                room: &self.dungeon.rooms[self.at],
                barred: true,
            };
            let (x, y) = hireling::beside(&grid, (hero.x, hero.y));
            self.lane_squires.push(Squire {
                owner,
                hp: recruit.kind.health(recruit.level),
                recruit,
                x,
                y,
                cooldown: HZ / 2,
                grace: HZ,
                smoke: 0,
            });
        }
        let mut squires = std::mem::take(&mut self.lane_squires);
        for s in &mut squires {
            self.drive_lane_squire(s);
        }
        self.lane_squires = squires;
    }

    fn lane_squire_hit(&mut self, owner: u32, index: usize, damage: u32) {
        let foe = &mut self.enemies[index];
        let dealt = damage.min(foe.hp);
        foe.hp -= dealt;
        foe.wits.foe = owner;
        self.feel.credit(owner, foe.id, dealt, self.tick);
        self.feel
            .blow(foe.id, owner, gore::Blow::Blast { at: (foe.x, foe.y) });
    }

    fn drive_lane_squire(&mut self, s: &mut Squire) {
        let kind = s.recruit.kind;
        let level = s.recruit.level;
        let max_hp = kind.health(level);
        s.grace = s.grace.saturating_sub(1);
        s.smoke = s.smoke.saturating_sub(1);
        s.cooldown = s.cooldown.saturating_sub(1);
        if s.hp == 0 {
            if !self.fight_on() {
                s.hp = max_hp / 2;
                s.grace = HZ;
            }
            return;
        }
        let Some(owner) = self.players.get(&s.owner) else {
            return;
        };
        let home = (owner.x, owner.y);
        let target = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.hp > 0
                    && e.age >= TELEGRAPH
                    && !e.buried()
                    && e.kind != EnemyKind::Dummy
                    && sight(self.room(), (s.x, s.y), (e.x, e.y))
            })
            .min_by(|(_, a), (_, b)| {
                (a.x - s.x)
                    .hypot(a.y - s.y)
                    .total_cmp(&(b.x - s.x).hypot(b.y - s.y))
            })
            .map(|(i, e)| (i, e.x, e.y));
        let in_room: std::collections::BTreeSet<u32> = self
            .players
            .keys()
            .filter(|id| !self.exits.outside.contains_key(id))
            .copied()
            .collect();
        let mut goal = home;
        if let Some((_, x, y)) = target {
            let distance = (x - s.x).hypot(y - s.y);
            let (ux, uy) = unit(x - s.x, y - s.y);
            match kind {
                Kind::Ninja => {
                    // Approach the far side of the quarry, away from its knight.
                    let (fx, fy) = unit(x - home.0, y - home.1);
                    goal = (x + fx * 1.1, y + fy * 1.1);
                }
                Kind::Shield => {
                    goal = ((home.0 + x) / 2.0, (home.1 + y) / 2.0);
                }
                _ => {
                    goal = if distance < 5.0 {
                        (s.x - ux * 3.0, s.y - uy * 3.0)
                    } else if distance > 8.0 {
                        (x - ux * 6.0, y - uy * 6.0)
                    } else {
                        (s.x, s.y)
                    };
                }
            }
        }
        if kind == Kind::Healer {
            if let Some(h) = self
                .players
                .iter()
                .filter(|(id, h)| {
                    in_room.contains(id) && h.hp > 0 && !h.away && !h.stone && h.hp < h.max_hp
                })
                .map(|(_, h)| h)
                .min_by_key(|h| u64::from(h.hp) * 10000 / u64::from(h.max_hp.max(1)))
            {
                if (h.x - s.x).hypot(h.y - s.y) > 7.0 {
                    goal = (h.x, h.y);
                }
            } else if level >= 4 {
                if let Some((x, y)) = self
                    .players
                    .iter()
                    .filter(|(id, h)| in_room.contains(id) && h.hp == 0 && !h.away)
                    .map(|(_, h)| h)
                    .filter_map(|h| h.skull_at)
                    .min_by(|a, b| {
                        (a.0 - s.x)
                            .hypot(a.1 - s.y)
                            .total_cmp(&(b.0 - s.x).hypot(b.1 - s.y))
                    })
                {
                    goal = (x, y);
                }
            }
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        if (goal.0 - s.x).hypot(goal.1 - s.y) > 0.8 {
            let (ux, uy) = unit(goal.0 - s.x, goal.1 - s.y);
            let pace = HERO_SPEED * DT * if kind == Kind::Ninja { 1.25 } else { 0.9 };
            (s.x, s.y) = grid.slide(
                (s.x, s.y),
                (ux * pace, uy * pace),
                HERO_RADIUS * 0.65,
                Mover::Hero,
            );
        }
        // Shots and bodies use the same grace as allies. Squires never eat
        // loot or keep a wiped party in play; shields physically intercept.
        if s.grace == 0 && s.smoke == 0 {
            let shot = self
                .projectiles
                .iter_mut()
                .find(|p| p.hostile && p.ttl > 0 && (p.x - s.x).hypot(p.y - s.y) < 0.8);
            let damage = if let Some(p) = shot {
                p.ttl = 0;
                p.damage
            } else {
                self.enemies
                    .iter()
                    .filter(|e| {
                        e.hp > 0
                            && e.kind != EnemyKind::Dummy
                            && (e.x - s.x).hypot(e.y - s.y) < e.radius() + 0.45
                    })
                    .map(|_| 12)
                    .next()
                    .unwrap_or(0)
            };
            if damage > 0 {
                s.hp = s.hp.saturating_sub(if kind == Kind::Shield && level >= 2 {
                    damage.div_ceil(2)
                } else {
                    damage
                });
                s.grace = HZ / 2;
                if kind == Kind::Ninja && level >= 4 && s.hp > 0 {
                    s.smoke = HZ;
                }
            }
        }
        if s.hp == 0 || s.cooldown > 0 {
            return;
        }
        let power = kind.power(level);
        if kind == Kind::Healer {
            let lowest = self
                .players
                .iter()
                .filter(|(id, h)| {
                    in_room.contains(id)
                        && h.hp > 0
                        && !h.away
                        && !h.stone
                        && (h.x - s.x).hypot(h.y - s.y) < 9.0
                        && sight(self.room(), (s.x, s.y), (h.x, h.y))
                })
                .filter(|(_, h)| {
                    h.hp < h.max_hp || (level >= 2 && (h.hexed > 0 || h.chilled > 0 || h.mired > 0))
                })
                .min_by_key(|(_, h)| u64::from(h.hp) * 10000 / u64::from(h.max_hp.max(1)))
                .map(|(&id, _)| id);
            if let Some(id) = lowest {
                let h = self.players.get_mut(&id).unwrap();
                h.hp = h.hp.saturating_add(power).min(h.max_hp);
                if level >= 2 {
                    h.hexed = 0;
                    h.chilled = 0;
                    h.mired = 0;
                }
                s.cooldown = 2 * HZ;
                self.sounds.push("potion");
            } else if level >= 4 {
                let reachable_skulls: std::collections::BTreeSet<u32> = self
                    .players
                    .iter()
                    .filter(|(id, h)| {
                        in_room.contains(id)
                            && !h.away
                            && h.hp == 0
                            && h.skull_at
                                .is_some_and(|at| sight(self.room(), (s.x, s.y), at))
                    })
                    .map(|(&id, _)| id)
                    .collect();
                if let Some((_, h)) = self.players.iter_mut().find(|(id, h)| {
                    reachable_skulls.contains(id)
                        && h.hp == 0
                        && h.skull_at
                            .is_some_and(|(x, y)| (x - s.x).hypot(y - s.y) < 2.5)
                }) {
                    h.hp = (h.max_hp / 5).max(1);
                    if let Some((x, y)) = h.skull_at.take() {
                        h.x = x;
                        h.y = y;
                    }
                    h.possessed = None;
                    h.invulnerable = 2 * HZ;
                    s.cooldown = 10 * HZ;
                }
            }
            return;
        }
        let Some((index, x, y)) = target else { return };
        let distance = (x - s.x).hypot(y - s.y);
        if distance > 16.0 || !sight(self.room(), (s.x, s.y), (x, y)) {
            return;
        }
        if kind == Kind::Ninja && distance < 2.2 || kind == Kind::Shield && distance < 2.5 {
            let flank = (s.x - x) * (home.0 - x) + (s.y - y) * (home.1 - y) < 0.0;
            self.lane_squire_hit(
                s.owner,
                index,
                if kind == Kind::Ninja && level >= 2 && flank {
                    power * 2
                } else {
                    power
                },
            );
            s.cooldown = HZ / 2;
        } else if kind == Kind::Alchemist {
            if level >= 2 {
                let hits: Vec<_> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| {
                        e.hp > 0
                            && (e.x - x).hypot(e.y - y) < 2.8
                            && sight(self.room(), (x, y), (e.x, e.y))
                    })
                    .map(|(i, _)| i)
                    .collect();
                for i in hits {
                    self.lane_squire_hit(s.owner, i, power);
                }
            } else {
                self.lane_squire_hit(s.owner, index, power);
            }
            if let Some(h) = self
                .players
                .get_mut(&s.owner)
                .filter(|h| h.hp > 0 && (h.x - s.x).hypot(h.y - s.y) < 7.0)
            {
                h.hp = h.hp.saturating_add(power / 2).min(h.max_hp);
            }
            s.cooldown = 3 * HZ;
            self.sounds.push("bomb");
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
        } else if kind != Kind::Shield && self.projectiles.len() < MAX_PROJECTILES {
            let (ux, uy) = unit(x - s.x, y - s.y);
            self.projectiles.push(Projectile {
                x: s.x,
                y: s.y,
                vx: ux * 14.0,
                vy: uy * 14.0,
                hostile: false,
                kind: if kind == Kind::Ninja {
                    Shot::Blade
                } else {
                    Shot::Arrow
                },
                look: None,
                damage: power,
                pierce: 0,
                last_hit: None,
                ttl: 2 * HZ,
                empowered: false,
                traits: ShotTraits {
                    owner: u8::try_from(s.owner).unwrap_or(0),
                    ..Default::default()
                },
            });
            if kind == Kind::Ranger && level >= 2 && !hireling::great(&self.enemies[index]) {
                self.enemies[index].chilled = self.enemies[index].chilled.max(HZ);
                self.minions.quarry.insert(s.owner, self.enemies[index].id);
            }
            s.cooldown = if kind == Kind::Ninja { HZ / 2 } else { HZ };
        }
    }
}

/// The same tile geometry as walking and shots; no healing or bomb through
/// a wall just because its target lies inside a radius.
fn sight(room: &Room, from: (f32, f32), to: (f32, f32)) -> bool {
    let distance = (to.0 - from.0).hypot(to.1 - from.1);
    let steps = (distance / (TILE_UNITS * 0.2)).ceil() as usize;
    (0..=steps.max(1)).all(|n| {
        let t = n as f32 / steps.max(1) as f32;
        let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        matches!(
            room.tile(
                (x / TILE_UNITS).floor() as i32,
                (y / TILE_UNITS).floor() as i32
            ),
            Tile::Floor | Tile::Door
        )
    })
}
