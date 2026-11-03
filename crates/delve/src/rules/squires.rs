//! The Training Yard's company: a knight keeps a roster, but takes one
//! squire down the stair. Wages and lessons come from the realm's purse.

use super::*;
use super::hireling::{AllyKind, Hireling};

pub const TYPES: [Kind; 5] = [Kind::Ninja, Kind::Healer, Kind::Ranger, Kind::Shield, Kind::Alchemist];
pub const MAX_LEVEL: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind { Ninja, Healer, Ranger, Shield, Alchemist }
impl Kind {
    pub fn name(self) -> &'static str { match self { Self::Ninja => "Ninja", Self::Healer => "Healer", Self::Ranger => "Ranger", Self::Shield => "Shield-bearer", Self::Alchemist => "Alchemist" } }
    pub fn ability(self, level: u8) -> &'static str { match (self, level >= 3) {
        (Self::Ninja, false) => "Stars / flank", (Self::Ninja, true) => "Backstab / vanish",
        (Self::Healer, false) => "Mend / cleanse", (Self::Healer, true) => "Skull vigil",
        (Self::Ranger, false) => "Arrows / range", (Self::Ranger, true) => "Hunter's mark",
        (Self::Shield, false) => "Block / rally", (Self::Shield, true) => "Iron guard",
        (Self::Alchemist, false) => "Bomb / potion", (Self::Alchemist, true) => "Wide blast",
    } }
    fn ally(self) -> AllyKind { match self { Self::Ninja => AllyKind::Hound, Self::Healer => AllyKind::Linnet, Self::Shield => AllyKind::Warden, _ => AllyKind::Beaumains } }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squire { pub id: u32, pub kind: Kind, pub name: String, pub level: u8 }
impl Squire {
    pub fn health(&self) -> u32 { (if self.kind == Kind::Shield { 90 } else { 48 }) + 14 * u32::from(self.level) }
    pub fn power(&self) -> u32 { 5 + 4 * u32::from(self.level) }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company { pub roster: Vec<Squire>, pub active: Option<u32> }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Yard {
    pub companies: BTreeMap<String, Company>,
    pub board: u8,
    pub hall: u8,
    pub training: u8,
    pub season: u32,
    pub next_id: u32,
    #[serde(default)]
    pub receipts: BTreeMap<String, u64>,
}
impl Yard {
    pub fn capacity(&self) -> usize { 2 + usize::from(self.hall.min(3)) * 2 }
    pub fn ceiling(&self) -> u8 { (2 + self.training).min(MAX_LEVEL) }
    pub fn offers(&self) -> Vec<Squire> {
        const NAMES: [&str; 10] = ["Ash", "Pip", "Rowan", "Kit", "Briar", "Wren", "Moss", "Finch", "Ember", "Thorn"];
        (0..(3 + usize::from(self.board.min(2)))).map(|i| {
            let roll = self.season as usize + i;
            // Every board has three choices; the rare shield and alchemist
            // visit as the company turns over. Upgrades show all five.
            Squire { id: i as u32, kind: TYPES[roll % TYPES.len()], name: NAMES[(roll * 3 + i) % NAMES.len()].into(), level: self.board.clamp(1, 3) }
        }).collect()
    }
    pub fn wage(&self, recruit: &Squire) -> u32 { (70 + 30 * u32::from(recruit.level)) * (100 - 15 * u32::from(self.hall.min(3))) / 100 }
    pub fn lesson(&self, s: &Squire) -> u32 { 45 * u32::from(s.level) * (100 - 15 * u32::from(self.training.min(3))) / 100 }
    pub fn upgrade_cost(&self, step: usize) -> Option<u32> { let level = *[self.board, self.hall, self.training].get(step)?; (level < 3).then_some([180, 360, 650][usize::from(level.min(2))]) }
    pub fn apply(&mut self, treasury: &mut Spoils, owner: &str, action: Action) -> Result<String, String> {
        let pay = |purse: &mut Spoils, cost| -> Result<(), String> { if purse.get(Spoil::Gold) < cost { Err(format!("Need {cost} gold")) } else { purse.take(&Spoils(BTreeMap::from([(Spoil::Gold, cost)]))); Ok(()) } };
        match action {
            Action::Hire(slot) => {
                let mut s = self.offers().get(slot).cloned().ok_or("Recruit has left")?;
                let company = self.companies.get(owner).cloned().unwrap_or_default();
                if company.roster.len() >= self.capacity() { return Err("Roster full: build hall".into()); }
                pay(treasury, self.wage(&s))?;
                self.next_id = self.next_id.saturating_add(1);
                s.id = self.next_id;
                let name = s.name.clone();
                let company = self.companies.entry(owner.into()).or_default();
                company.active.get_or_insert(s.id);
                company.roster.push(s);
                Ok(format!("{name} joined {owner}'s company"))
            }
            Action::Select(slot) => {
                let company = self.companies.get_mut(owner).ok_or("Hire a squire")?;
                let id = company.roster.get(slot).ok_or("Empty roster slot")?.id;
                company.active = Some(id);
                Ok("Squire ready".into())
            }
            Action::Rest => { let company = self.companies.get_mut(owner).ok_or("Hire a squire")?; company.active = None; Ok("Squire resting".into()) }
            Action::Train(slot) => {
                let s = self.companies.get(owner).and_then(|c| c.roster.get(slot)).ok_or("Empty roster slot")?;
                if s.level >= self.ceiling() { return Err(if s.level == MAX_LEVEL { "Master squire" } else { "Build training yard" }.into()); }
                pay(treasury, self.lesson(s))?;
                let s = &mut self.companies.get_mut(owner).unwrap().roster[slot];
                s.level += 1;
                Ok(format!("{} reached level {}", s.name, s.level))
            }
            Action::Upgrade(step) => {
                let cost = self.upgrade_cost(step).ok_or("Building complete")?;
                pay(treasury, cost)?;
                match step { 0 => self.board += 1, 1 => self.hall += 1, _ => self.training += 1 }
                Ok("Squire yard improved".into())
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action { Hire(usize), Select(usize), Train(usize), Rest, Upgrade(usize) }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Companion { pub owner: u32, pub squire: Squire, pub body: Hireling, #[serde(default)] pub smoke: u32, #[serde(default)] pub vigil: u32, #[serde(default)] pub rescued: bool }

#[cfg(test)]
#[path = "../../tests/rules/together_squires__tests.rs"]
mod tests;

/// Separate plates keep the same E / touch-use controls as Sir Ector.
pub fn plates() -> Vec<(Action, (i32, i32, i32, i32))> {
    let mut out = Vec::new();
    for i in 0..5 { out.push((Action::Hire(i), (2 + i as i32 * 4, 3, 2, 1))); }
    for i in 0..8 {
        out.push((Action::Select(i), (1 + i as i32 * 3, 9, 2, 1)));
        out.push((Action::Train(i), (1 + i as i32 * 3, 10, 2, 1)));
    }
    out.push((Action::Rest, (10, 12, 2, 1)));
    for i in 0..3 { out.push((Action::Upgrade(i), (13 + i as i32 * 3, 12, 2, 1))); }
    out
}
pub fn action_at(x: f32, y: f32) -> Option<Action> {
    let (x, y) = (x / TILE_UNITS, y / TILE_UNITS);
    plates().into_iter().find(|(_, (c, r, w, h))| x >= *c as f32 && y >= *r as f32 && x < (c+w) as f32 && y < (r+h) as f32).map(|(a, _)| a)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Request { owner: String, action: Action, season: u32, game: String, serial: u64 }

/// Marks are deltas on the host. The receipt also makes a restored mark
/// harmless, and ordered ticks preserve hire -> train -> select requests.
pub fn settle(yard: &mut Yard, treasury: &mut Spoils, marks: &BTreeMap<String, u32>) -> activity::Settled {
    let mut result = activity::Settled::default();
    let mut requests: Vec<Request> = marks.keys().filter_map(|key| key.strip_prefix("squire:").and_then(|s| serde_json::from_str(s).ok())).collect();
    requests.sort_by(|a, b| a.serial.cmp(&b.serial).then(a.owner.cmp(&b.owner)));
    for r in requests {
        if r.season != yard.season { continue; }
        let receipt = format!("{}:{}", r.game, r.owner);
        if yard.receipts.get(&receipt).is_some_and(|&n| n >= r.serial) { continue; }
        yard.receipts.insert(receipt, r.serial);
        result.changed = true;
        let answer = if r.season != yard.season { Err("Recruits changed: choose again".into()) } else { yard.apply(treasury, &r.owner, r.action) };
        result.said.push(("squire_yard".into(), answer.unwrap_or_else(|s| s)));
    }
    for key in marks.keys().filter_map(|k| k.strip_prefix("squire_turn:")) {
        let Some((before, after)) = key.split_once(':').and_then(|(a,b)| Some((a.parse::<u32>().ok()?,b.parse::<u32>().ok()?))) else { continue; };
        if yard.season == before && after == before.wrapping_add(1) {
            yard.season = after;
            yard.receipts.clear();
            result.changed = true;
        }
    }
    result
}

impl Run {
    pub(super) fn tick_squire_yard(&mut self, inputs: &BTreeMap<u32, Input>) {
        let mut requests = Vec::new();
        for (&id, hero) in &mut self.players {
            let previous = self.squire_yard_holds.remove(&id);
            if super::squire_hall::plate_at(hero.x, hero.y).is_some() {
                if previous.is_some() { hero.buying = 0; }
                continue;
            }
            let pressing = inputs.get(&id).filter(|i| i.valid()).is_some_and(|i| i.interact);
            let action = action_at(hero.x, hero.y).filter(|_| hero.hp > 0 && !hero.stone && !hero.absent);
            if !pressing { hero.buy_spent = false; hero.buying = 0; }
            let Some(action) = action else {
                if previous.is_some() { hero.buying = 0; }
                continue;
            };
            self.squire_yard_holds.insert(id, action);
            if previous != Some(action) { hero.buying = 0; }
            if !pressing { continue; }
            if hero.buy_spent { continue; }
            hero.buying = hero.buying.saturating_add(1);
            if hero.buying >= home::BUY_HOLD {
                hero.buying = 0;
                hero.buy_spent = true;
                requests.push((id, super::talents::knight_key(hero), action));
            }
        }
        for (id, owner, action) in requests {
            let r = Request { owner: owner.clone(), action, season: self.home.squires.season, game: format!("{}:{}", self.seed, self.raid_id), serial: self.tick.saturating_add(1) };
            let answer = self.home.squires.apply(&mut self.treasury, &owner, action);
            // The same transaction is predicted and then revalidated against
            // the host's real purse by activity settlement.
            let mark = format!("squire:{}", serde_json::to_string(&r).unwrap());
            self.marks.insert(mark, 1);
            self.found = Some((self.tick, id, answer.unwrap_or_else(|s| s)));
        }
    }

    pub(super) fn deploy_squires(&mut self) {
        self.squires.clear();
        self.squire_mark = None;
        for (&owner, hero) in &self.players {
            if hero.absent { continue; }
            let key = super::talents::knight_key(hero);
            let Some(company) = self.home.squires.companies.get(&key) else { continue; };
            let Some(squire) = company.roster.iter().find(|s| Some(s.id) == company.active).cloned() else { continue; };
            let mut body = Hireling::minion(squire.kind.ally(), squire.power(), self.dungeon.depth, (hero.x - 1.0, hero.y + 1.0), owner);
            body.max_hp = squire.health(); body.hp = body.max_hp;
            self.squires.push(Companion { owner, squire, body, smoke: 0, vigil: 0, rescued: false });
        }
        let before = self.home.squires.season;
        self.home.squires.season = before.wrapping_add(1);
        self.marks.insert(format!("squire_turn:{before}:{}", self.home.squires.season), 1);
    }

    pub(super) fn squires_follow(&mut self) {
        self.lodge_squires_follow();
        self.squire_mark = None;
        for companion in &mut self.squires {
            let Some(hero) = self.players.get(&companion.owner) else { continue; };
            (companion.body.x, companion.body.y) = (hero.x, hero.y);
            if companion.body.down() { companion.body.hp = companion.body.max_hp / 2; }
            companion.body.invulnerable = 20;
            companion.vigil = 0; companion.rescued = false;
        }
    }

    pub(super) fn tick_squires(&mut self) {
        self.tick_lodge_squires();
        if self.at_home_now() { return; }
        if let Some(mark) = &mut self.squire_mark { mark.ticks = mark.ticks.saturating_sub(1); }
        if self.squire_mark.as_ref().is_some_and(|m| m.ticks == 0) { self.squire_mark = None; }
        let mut squires = std::mem::take(&mut self.squires);
        for s in &mut squires {
            if !self.players.get(&s.owner).is_some_and(|h| !h.absent) { continue; }
            // Squires do not inherit card summons' wishes or commands.
            s.body.summoner = None;
            let alive = self.ally_survives(&mut s.body, Some(s.owner));
            s.body.summoner = Some(s.owner);
            if !alive { s.vigil = 0; continue; }
            s.smoke = s.smoke.saturating_sub(1);
            self.squire_tactics(s);
        }
        squires.retain(|s| self.players.contains_key(&s.owner));
        self.squires = squires;
    }
}

impl Run {
    fn squire_tactics(&mut self, s: &mut Companion) {
        let kind = s.squire.kind;
        let level = s.squire.level;
        let power = s.squire.power();
        let ready = s.body.cooldown == 0;
        let target = self.enemies.iter().enumerate()
            .filter(|(_, e)| e.hp > 0 && e.age >= TELEGRAPH && !e.asleep(self.tick) && e.kind != EnemyKind::Dummy)
            .filter(|(_, e)| (e.x - s.body.x).hypot(e.y - s.body.y) < 18.0)
            .min_by(|(_, a), (_, b)| (a.x - s.body.x).hypot(a.y - s.body.y).total_cmp(&(b.x - s.body.x).hypot(b.y - s.body.y)))
            .map(|(i, e)| (i, e.x, e.y));
        let owner = self.players.get(&s.owner).map(|h| (h.x, h.y));
        let mut goal = owner;
        let mut range = 2.5;
        if let Some((_, x, y)) = target {
            match kind {
                Kind::Ninja => {
                    // A fast diagonal approach puts the squire beyond the
                    // foe's line to its knight, rather than in their way.
                    let (ox, oy) = owner.unwrap_or((s.body.x, s.body.y));
                    let (ux, uy) = unit(x - ox, y - oy);
                    goal = Some((x + ux * 1.2 - uy, y + uy * 1.2 + ux)); range = 0.5;
                }
                Kind::Shield => {
                    let (ox, oy) = owner.unwrap_or((s.body.x, s.body.y));
                    let (ux, uy) = unit(x - ox, y - oy);
                    goal = Some((ox + ux * 2.0, oy + uy * 2.0)); range = 0.5;
                }
                _ => { goal = Some((x, y)); range = if kind == Kind::Healer { 10.0 } else { 7.0 }; }
            }
        }
        // A trained healer keeps vigil beside an uncarried skull. One
        // rescue per room, five seconds uninterrupted: never a free wipe.
        if kind == Kind::Healer && level >= 3 && !s.rescued {
            let skull = self.players.keys().filter_map(|&id| self.skull_lying(id).map(|at| (id, at)))
                .min_by(|(_, a), (_, b)| (a.0-s.body.x).hypot(a.1-s.body.y).total_cmp(&(b.0-s.body.x).hypot(b.1-s.body.y)));
            if let Some((id, at)) = skull {
                goal = Some(at); range = 1.0;
                if (at.0-s.body.x).hypot(at.1-s.body.y) < 2.0 {
                    s.vigil += 1;
                    if s.vigil >= 5 * HZ {
                        let hero = self.players.get_mut(&id).unwrap();
                        hero.hp = (hero.max_hp / 4).max(1);
                        (hero.x, hero.y) = at;
                        hero.skull_at = None; hero.possessed = None; hero.invulnerable = 2 * HZ;
                        s.rescued = true; s.vigil = 0;
                        self.cues.push("squire_rescue".into());
                    }
                } else { s.vigil = 0; }
            } else { s.vigil = 0; }
        }
        if ready {
            match kind {
                Kind::Healer => {
                    let patient = self.players.iter().filter(|(_, h)| h.hp > 0 && !h.absent && (h.hp < h.max_hp || h.hexed > 0 || h.chilled > 0))
                        .filter(|(_, h)| (h.x-s.body.x).hypot(h.y-s.body.y) < 18.0)
                        .min_by_key(|(_, h)| u64::from(h.hp) * 1000 / u64::from(h.max_hp.max(1))).map(|(&id, _)| id);
                    if let Some(id) = patient {
                        let hero = self.players.get_mut(&id).unwrap();
                        hero.mend(power, super::stakes::Mend::Party);
                        hero.hexed = 0; hero.chilled = 0;
                        s.body.cooldown = 2 * HZ;
                        self.cues.push("squire_mend".into());
                    }
                }
                Kind::Ninja => {
                    if let Some((i, x, y)) = target {
                        let close = (x-s.body.x).hypot(y-s.body.y) < self.enemies[i].radius() + 1.5;
                        if close {
                            let (ox, oy) = owner.unwrap_or((s.body.x, s.body.y));
                            let behind = (s.body.x-x)*(ox-x)+(s.body.y-y)*(oy-y) < 0.0;
                            self.squire_hit(s.owner, i, power * if level >= 3 && behind { 3 } else { 1 });
                            s.body.cooldown = HZ / 2;
                            if level >= 3 { s.smoke = HZ / 3; s.body.invulnerable = HZ / 3; }
                            self.cues.push("squire_backstab".into());
                        } else if self.squire_shot(s, (x,y), Shot::Blade, power) { s.body.cooldown = HZ / 2; }
                    }
                }
                Kind::Ranger => {
                    if let Some((i, x, y)) = target {
                        if self.squire_shot(s, (x,y), Shot::Arrow, power) {
                            s.body.cooldown = 3 * HZ / 4;
                            if level >= 3 { self.squire_mark = Some(Mark { foe: self.enemies[i].id, ticks: 2 * HZ }); }
                        }
                    }
                }
                Kind::Shield => {
                    if let Some((i, x, y)) = target {
                        if (x-s.body.x).hypot(y-s.body.y) < self.enemies[i].radius()+2.0 {
                            self.squire_hit(s.owner, i, power);
                            s.body.cooldown = HZ;
                            if level >= 3 { s.body.invulnerable = HZ / 2; }
                        }
                    }
                }
                Kind::Alchemist => {
                    if let Some((_, x, y)) = target {
                        let radius = if level >= 3 { 3.5 } else { 2.0 };
                        let foes: Vec<_> = self.enemies.iter().enumerate().filter(|(_,e)| e.hp > 0 && e.kind != EnemyKind::Dummy && (e.x-x).hypot(e.y-y) <= radius).map(|(i,_)| i).collect();
                        for i in foes { self.squire_hit(s.owner, i, power); }
                        if self.blasts.len() < 16 { self.blasts.push((x,y,BLAST_TICKS)); }
                        if let Some(h) = self.players.get_mut(&s.owner).filter(|h| h.hp > 0 && (h.x-s.body.x).hypot(h.y-s.body.y) < 12.0) { h.mend(power / 2, super::stakes::Mend::Party); }
                        s.body.cooldown = 3 * HZ;
                        self.cues.push("squire_bomb".into());
                    }
                }
            }
        }
        if let Some((x,y)) = goal {
            let d = (x-s.body.x).hypot(y-s.body.y);
            let mut direction = unit(x-s.body.x,y-s.body.y);
            if target.is_some() && !matches!(kind, Kind::Ninja | Kind::Shield) && d < range - 2.0 { direction = (-direction.0, -direction.1); }
            else if d <= range { direction = (0.0,0.0); }
            let speed = HERO_SPEED * DT * if kind == Kind::Ninja { 1.4 } else { 0.95 };
            let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
            (s.body.x,s.body.y) = grid.slide((s.body.x,s.body.y),(direction.0*speed,direction.1*speed),HERO_RADIUS,Mover::Hero);
            if direction.0 != 0.0 { s.body.facing = direction.0; }
        }
    }

    fn squire_hit(&mut self, owner: u32, i: usize, damage: u32) {
        let foe = &mut self.enemies[i];
        let dealt = damage.min(foe.hp);
        foe.hp -= dealt; foe.wits.foe = owner;
        self.feel.credit(owner,foe.id,dealt,self.tick);
    }
    fn squire_shot(&mut self, s: &Companion, to: (f32,f32), kind: Shot, damage: u32) -> bool {
        if self.projectiles.len() >= MAX_PROJECTILES { return false; }
        let (ux,uy) = unit(to.0-s.body.x,to.1-s.body.y);
        self.projectiles.push(Projectile { x:s.body.x,y:s.body.y,vx:ux*16.0,vy:uy*16.0,hostile:false,kind,look:None,damage,pierce:0,last_hit:None,ttl:2*HZ,empowered:false,traits:ShotTraits { owner:s.owner as u8,..Default::default() } });
        true
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mark { pub foe: u32, pub ticks: u32 }
