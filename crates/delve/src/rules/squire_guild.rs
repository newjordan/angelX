//! Permanent, per-knight squires. The guild owns recruitment and training;
//! dungeon bodies live in the existing ally pack, never the loot/player list.
//! Commands are replayed at realm settlement, just like the holdings, so a
//! predicted town purchase cannot bypass the real treasury. No random stream
//! is consumed by recruitment, AI, or training.
use super::*;
use std::collections::BTreeSet;
use crate::realm::{Spoil, Spoils};
use super::hireling::{AllyKind, Hireling};

pub const CADENCE: u32 = 90 * HZ;
pub const MAX_LEVEL: u8 = 5;
pub const MAX_WORKS: u8 = 3;
pub const ROSTER_MAX: usize = 5;
pub const TYPES: [AllyKind; 5] = [AllyKind::Ninja, AllyKind::Healer, AllyKind::Ranger, AllyKind::Shieldbearer, AllyKind::Alchemist];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Work { Board, Hall, Yard }
impl Work {
    pub fn name(self) -> &'static str { match self { Self::Board => "RECRUITING BOARD", Self::Hall => "HIRING HALL", Self::Yard => "TRAINING YARD" } }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Training { pub ready_at: u32, pub delve: u32 }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squire {
    pub id: u64,
    pub kind: AllyKind,
    pub name: String,
    pub vitality: u32,
    pub attack: u32,
    #[serde(default = "one")]
    pub level: u8,
    #[serde(default)]
    pub training: Option<Training>,
}
fn one() -> u8 { 1 }
impl Squire {
    pub fn hp(&self) -> u32 { self.vitality + u32::from(self.level.saturating_sub(1)) * 14 }
    pub fn damage(&self) -> u32 { self.attack + u32::from(self.level.saturating_sub(1)) * 3 }
    pub fn wage(&self) -> u32 { 90 + self.vitality / 2 + self.attack * 2 }
    pub fn ability(&self) -> &'static str {
        match (self.kind, self.level) {
            (AllyKind::Ninja, 3..) => "STARS / BACKSTAB / SMOKE",
            (AllyKind::Ninja, 2) => "STARS / BACKSTAB",
            (AllyKind::Ninja, _) => "THROWING STARS",
            (AllyKind::Healer, 3..) => "HEAL / CLEANSE / SKULL ASSIST",
            (AllyKind::Healer, 2) => "HEAL / CLEANSE",
            (AllyKind::Healer, _) => "LOWEST HEALTH HEAL",
            (AllyKind::Ranger, 2..) => "ARROWS / HUNTER MARK",
            (AllyKind::Ranger, _) => "ARROWS",
            (AllyKind::Shieldbearer, 3..) => "SHIELD / GUARD / RALLY",
            (AllyKind::Shieldbearer, 2) => "SHIELD / GUARD",
            (AllyKind::Shieldbearer, _) => "SHIELD BASH",
            (AllyKind::Alchemist, 3..) => "ACID / FROST / TONIC",
            (AllyKind::Alchemist, 2) => "ACID / FROST",
            _ => "ACID FLASK",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roster { pub members: Vec<Squire>, pub active: Option<u64> }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Guild {
    pub board: u8,
    pub hall: u8,
    pub yard: u8,
    /// Town ticks only: paused while fighting or disconnected.
    pub clock: u32,
    pub delves: u32,
    /// Settlement receipt. Applied command logs are safe to see again.
    pub revision: u32,
    pub hired: BTreeSet<u64>,
    pub rosters: BTreeMap<String, Roster>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Upgrade(Work),
    Hire { knight: String, recruit: u64 },
    Select { knight: String, recruit: Option<u64> },
    Train { knight: String, recruit: u64 },
    Depart,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Receipt { revision: u32, clock: u32, command: Command }
fn fee(n: u32) -> Spoils { let mut p = Spoils::default(); p.add(Spoil::Gold, n); p }
impl Guild {
    pub fn level(&self, work: Work) -> u8 { match work { Work::Board => self.board, Work::Hall => self.hall, Work::Yard => self.yard } }
    pub fn upgrade_price(&self, work: Work) -> Option<u32> {
        let l = self.level(work); (l < MAX_WORKS).then_some(180 * (u32::from(l) + 1))
    }
    pub fn capacity(&self) -> usize { (2 + usize::from(self.hall)).min(ROSTER_MAX) }
    pub fn cycle(&self) -> u32 { self.clock / CADENCE + self.delves }
    /// Every board includes all five professions; improvements add applicants
    /// and stronger base stats, independently of hall capacity and yard speed.
    pub fn recruits(&self) -> Vec<Squire> {
        const NAMES: [&str; 16] = ["Ash", "Bram", "Clover", "Dusk", "Ember", "Fern", "Gale", "Hazel", "Iris", "Jun", "Kestrel", "Lark", "Moss", "Nettle", "Orin", "Pip"];
        (0..5 + usize::from(self.board) * 2).map(|i| {
            let mix = (u64::from(self.cycle()) + 1).wrapping_mul(0x9e3779b9).wrapping_add(i as u64 * 0x85ebca6b);
            let kind = TYPES[(i + self.cycle() as usize % 5) % 5];
            let base = match kind { AllyKind::Ninja => (58, 12), AllyKind::Healer => (68, 14), AllyKind::Ranger => (65, 15), AllyKind::Shieldbearer => (115, 10), _ => (72, 13) };
            Squire { id: u64::from(self.cycle()) * 16 + i as u64,
                kind, name: format!("{} {}", NAMES[(mix as usize >> 8) % NAMES.len()], kind.name()),
                vitality: base.0 + u32::from(self.board) * 8 + (mix % 9) as u32,
                attack: base.1 + u32::from(self.board) * 2 + (mix % 3) as u32,
                level: 1, training: None }
        }).filter(|s| !self.hired.contains(&s.id)).collect()
    }
    pub fn training_price(&self, member: &Squire) -> u32 { 80 * u32::from(member.level) }
    pub fn training_ticks(&self, member: &Squire) -> u32 { 20 * HZ * u32::from(member.level) / (u32::from(self.yard) + 1) }
    pub fn advance(&mut self, clock: u32) {
        self.clock = self.clock.max(clock);
        let cycle = self.cycle();
        self.hired.retain(|id| *id / 16 >= u64::from(cycle));
        for roster in self.rosters.values_mut() {
            for s in &mut roster.members {
                if s.training.as_ref().is_some_and(|t| t.ready_at <= self.clock && t.delve <= self.delves) {
                    s.level = (s.level + 1).min(MAX_LEVEL); s.training = None;
                }
            }
        }
    }
    pub fn apply(&mut self, command: &Command, treasury: &mut Spoils) -> Result<(), String> {
        match command {
            Command::Upgrade(work) => {
                let gold = self.upgrade_price(*work).ok_or("ALREADY BUILT IN FULL")?;
                if !treasury.covers(&fee(gold)) { return Err(format!("NEED {gold} GOLD")); }
                treasury.take(&fee(gold));
                match work { Work::Board => self.board += 1, Work::Hall => self.hall += 1, Work::Yard => self.yard += 1 }
            }
            Command::Hire { knight, recruit } => {
                let s = self.recruits().into_iter().find(|s| s.id == *recruit).ok_or("APPLICANT HAS LEFT")?;
                if self.rosters.get(knight).is_some_and(|r| r.members.len() >= self.capacity()) { return Err("ROSTER FULL: UPGRADE HIRING HALL".into()); }
                if !treasury.covers(&fee(s.wage())) { return Err(format!("NEED {} GOLD", s.wage())); }
                treasury.take(&fee(s.wage())); self.hired.insert(s.id);
                let roster = self.rosters.entry(knight.clone()).or_default();
                if roster.active.is_none() { roster.active = Some(s.id); }
                roster.members.push(s);
            }
            Command::Select { knight, recruit } => {
                let r = self.rosters.get_mut(knight).ok_or("NO SQUIRES HIRED")?;
                if recruit.is_some_and(|id| !r.members.iter().any(|s| s.id == id)) { return Err("NOT YOUR SQUIRE".into()); }
                r.active = *recruit;
            }
            Command::Train { knight, recruit } => {
                let s = self.rosters.get(knight).and_then(|r| r.members.iter().find(|s| s.id == *recruit)).ok_or("NOT YOUR SQUIRE")?;
                if s.training.is_some() { return Err("ALREADY TRAINING".into()); }
                if s.level >= MAX_LEVEL { return Err("FULLY TRAINED".into()); }
                let gold = self.training_price(s);
                let training = Training { ready_at: self.clock.saturating_add(self.training_ticks(s)), delve: self.delves.saturating_add(1) };
                if !treasury.covers(&fee(gold)) { return Err(format!("NEED {gold} GOLD")); }
                treasury.take(&fee(gold));
                self.rosters.get_mut(knight).unwrap().members.iter_mut().find(|s| s.id == *recruit).unwrap().training = Some(training);
            }
            Command::Depart => { self.delves = self.delves.saturating_add(1); self.advance(self.clock); }
        }
        Ok(())
    }
}

/// Replays only new receipts in sequence, paying the realm, not the prediction.
/// Clock is absolute and monotonic, so both time and completed delves survive
/// reloads and repeated save passes without double training or double wages.
pub fn settle(guild: &mut Guild, treasury: &mut Spoils, marks: &BTreeMap<String, u32>) -> super::activity::Settled {
    let before = guild.clone();
    let mut receipts: Vec<Receipt> = marks.keys().filter_map(|k| k.strip_prefix("squires:event:").and_then(|s| serde_json::from_str(s).ok())).collect();
    receipts.sort_by_key(|r| r.revision);
    let mut said = Vec::new();
    for r in receipts {
        if r.revision <= guild.revision { continue; }
        if r.revision != guild.revision.saturating_add(1) { break; }
        guild.advance(r.clock);
        match guild.apply(&r.command, treasury) {
            Ok(()) => said.push(("squires".into(), "The squires' guild records your order".into())),
            Err(e) => said.push(("squires".into(), e)),
        }
        guild.revision = r.revision;
    }
    if let Some(clock) = marks.get("squires:clock") { guild.advance(*clock); }
    super::activity::Settled { changed: before != *guild, said }
}

/// Five non-overlapping plates in the existing yard.
/// Left/right (or A/D) browse; hold E confirms, once per release.
pub const PLATES: [(i32, i32, i32, i32); 5] = [(2, 6, 1, 1), (4, 6, 1, 1), (9, 6, 1, 1), (20, 6, 1, 1), (22, 6, 1, 1)];
pub const LABELS: [&str; 5] = ["RECRUITS", "ROSTER", "BOARD", "HALL", "TRAINING"];
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Menu { pub plate: usize, pub cursor: usize, pub held: u32, pub pressed: bool, pub direction: i8, pub notice: String }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark { pub enemy: u32, pub owner: u32, pub left: u32 }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Field { pub menus: BTreeMap<u32, Menu>, pub marks: Vec<Mark> }
impl Field { pub fn is_empty(&self) -> bool { self == &Self::default() } }
pub fn plate_at(x: f32, y: f32) -> Option<usize> {
    PLATES.iter().position(|&(c,r,w,h)| x >= c as f32 * TILE_UNITS && x < (c+w) as f32 * TILE_UNITS && y >= r as f32 * TILE_UNITS && y < (r+h) as f32 * TILE_UNITS)
}
impl Run {
    /// The sole command entrance used by UI and tests. Persistent changes are
    /// town-only, belong to a live named seat, and enter the realm receipt log.
    pub fn squire_command(&mut self, owner: u32, command: Command) -> Result<(), String> {
        if self.dungeon.depth != 0 || !matches!(self.phase, Phase::Exploring | Phase::Fighting) { return Err("CHOOSE SQUIRES BEFORE THE DELVE".into()); }
        let hero = self.players.get(&owner).filter(|h| h.hp > 0 && !h.absent).ok_or("KNIGHT NOT PRESENT")?;
        let who = match &command { Command::Hire { knight, .. } | Command::Select { knight, .. } | Command::Train { knight, .. } => Some(knight), _ => None };
        if who.is_some_and(|k| k != &hero.name) || matches!(command, Command::Depart) { return Err("NOT YOUR SQUIRE".into()); }
        self.record_squire_command(command)
    }
    fn record_squire_command(&mut self, command: Command) -> Result<(), String> {
        self.home.squire_guild.apply(&command, &mut self.treasury)?;
        self.home.squire_guild.revision = self.home.squire_guild.revision.saturating_add(1);
        let r = Receipt { revision: self.home.squire_guild.revision, clock: self.home.squire_guild.clock, command };
        self.marks.insert(format!("squires:event:{}", serde_json::to_string(&r).expect("squire receipt")), 1);
        self.cues.push("squires:order".into());
        Ok(())
    }
    pub(super) fn tick_squire_guild(&mut self, inputs: &BTreeMap<u32, Input>) {
        for mark in &mut self.squire_guild.marks { mark.left = mark.left.saturating_sub(1); }
        self.squire_guild.marks.retain(|m| m.left > 0 && self.enemies.iter().any(|e| e.id == m.enemy && e.hp > 0));
        if self.dungeon.depth != 0 { self.squire_guild.menus.clear(); return; }
        let next = self.home.squire_guild.clock.max(self.marks.get("squires:clock").copied().unwrap_or(0)).saturating_add(1);
        self.home.squire_guild.advance(next);
        // The realm keeps the guild's clock at its refresh boundaries only:
        // a mark each tick made every tick a home claim, and the host
        // settled (and saved) thirty times a second until play froze. A
        // guild order carries the clock in its own receipt
        // (`record_squire_command`), so nothing ordered waits for a boundary.
        if next % CADENCE == 0 {
            self.marks.insert("squires:clock".into(), next);
        }
        if self.room().kind != RoomKind::SquireGuild { self.squire_guild.menus.clear(); return; }
        let mut commands = Vec::new();
        for (&id, hero) in &self.players {
            let Some(plate) = plate_at(hero.x, hero.y).filter(|_| hero.hp > 0 && !hero.absent) else { self.squire_guild.menus.remove(&id); continue; };
            let input = inputs.get(&id).copied().unwrap_or_default();
            let rows = self.squire_choices(id, plate);
            let menu = self.squire_guild.menus.entry(id).or_insert_with(|| Menu { plate, ..Default::default() });
            if menu.plate != plate { *menu = Menu { plate, ..Default::default() }; }
            let dir = if input.move_x > 0 { 1 } else if input.move_x < 0 { -1 } else { 0 };
            if dir != 0 && dir != menu.direction { menu.cursor = (menu.cursor as isize + dir as isize).rem_euclid(rows.len().max(1) as isize) as usize; menu.notice.clear(); }
            menu.direction = dir;
            menu.cursor = menu.cursor.min(rows.len().saturating_sub(1));
            if !input.interact { menu.held = 0; menu.pressed = false; }
            else if !menu.pressed {
                menu.held = (menu.held + 1).min(home::BUY_HOLD);
                if menu.held >= home::BUY_HOLD { menu.pressed = true; if let Some(command) = rows.get(menu.cursor).cloned() { commands.push((id, command)); } }
            }
        }
        for (id, command) in commands {
            let result = self.squire_command(id, command);
            if let Some(menu) = self.squire_guild.menus.get_mut(&id) { menu.notice = match result { Ok(()) => "ORDER RECORDED".into(), Err(e) => e }; }
        }
    }
    pub fn squire_choices(&self, owner: u32, plate: usize) -> Vec<Command> {
        let Some(hero) = self.players.get(&owner) else { return Vec::new(); };
        let knight = hero.name.clone();
        let roster = self.home.squire_guild.rosters.get(&knight);
        match plate {
            0 => self.home.squire_guild.recruits().into_iter().map(|s| Command::Hire { knight: knight.clone(), recruit: s.id }).collect(),
            1 => std::iter::once(Command::Select { knight: knight.clone(), recruit: None }).chain(roster.into_iter().flat_map(|r| &r.members).map(|s| Command::Select { knight: knight.clone(), recruit: Some(s.id) })).collect(),
            2 => vec![Command::Upgrade(Work::Board)],
            3 => vec![Command::Upgrade(Work::Hall)],
            4 => std::iter::once(Command::Upgrade(Work::Yard)).chain(roster.into_iter().flat_map(|r| &r.members).map(|s| Command::Train { knight: knight.clone(), recruit: s.id })).collect(),
            _ => Vec::new(),
        }
    }
    pub(super) fn depart_squires(&mut self) {
        let _ = self.record_squire_command(Command::Depart);
        self.squire_guild.menus.clear(); self.squire_guild.marks.clear();
        self.minions.raised.retain(|m| !m.kind.guild_squire());
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: true };
        for (&id, hero) in &self.players {
            if hero.absent { continue; }
            let Some(s) = self.home.squire_guild.rosters.get(&hero.name).and_then(|r| r.members.iter().find(|s| Some(s.id) == r.active)) else { continue; };
            let mut ally = Hireling::minion(s.kind, s.damage() | (u32::from(s.level) << 16), self.dungeon.depth, hireling::beside(&grid, (hero.x, hero.y)), id);
            ally.hp = s.hp(); ally.max_hp = s.hp(); ally.invulnerable = HZ;
            self.minions.raised.push(ally);
            self.cues.push(format!("squires:depart:{id}:{}", s.kind.word()));
        }
    }
    pub(super) fn squire_guild_follow(&mut self) {
        self.squire_guild.marks.clear();
        for ally in self.minions.raised.iter_mut().filter(|m| m.kind.guild_squire()) {
            if ally.down() { ally.hp = (ally.max_hp / 2).max(1); }
            ally.cooldown = 0; ally.taunt = 0;
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_squire_guild__tests.rs"]
mod tests;

impl Run {
    fn guild_squire_target(&self, ally: &Hireling, reach: f32) -> Option<usize> {
        self.enemies.iter().enumerate().filter(|(_,e)| e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy && (e.x-ally.x).hypot(e.y-ally.y) <= reach)
            .min_by(|(_,a),(_,b)| (a.x-ally.x).hypot(a.y-ally.y).total_cmp(&(b.x-ally.x).hypot(b.y-ally.y)).then(a.id.cmp(&b.id))).map(|(i,_)| i)
    }
    fn guild_squire_hit(&mut self, ally: &Hireling, index: usize, damage: u32) {
        let foe = &mut self.enemies[index];
        let damage = damage.min(foe.hp);
        foe.hp -= damage;
        if let Some(owner) = ally.summoner { foe.wits.foe = owner; self.feel.credit(owner, foe.id, damage, self.tick); }
    }
    fn guild_squire_shot(&mut self, ally: &Hireling, target: usize, damage: u32, kind: Shot, speed: f32, mut traits: ShotTraits) {
        if self.projectiles.len() >= MAX_PROJECTILES { return; }
        let e = &self.enemies[target];
        let (ux,uy) = unit(e.x-ally.x, e.y-ally.y);
        traits.owner = ally.summoner.unwrap_or(1) as u8;
        self.projectiles.push(Projectile { x: ally.x, y: ally.y, vx: ux*speed, vy: uy*speed, hostile: false, look: None, kind, damage,
            pierce: 0, last_hit: None, empowered: false, traits, ttl: 2*HZ });
    }
    /// Called by the minion driver's normal cooldown/damage/recovery loop.
    pub(super) fn squire_help(&mut self, ally: &mut Hireling) {
        let level = (ally.power >> 16).clamp(1, u32::from(MAX_LEVEL));
        let damage = (ally.power & 0xffff).max(1);
        match ally.kind {
            AllyKind::Healer => {
                let target = self.players.iter().filter(|(_,h)| h.hp > 0 && !h.absent && (h.x-ally.x).hypot(h.y-ally.y) <= 18.0)
                    .filter(|(_,h)| h.hp < h.max_hp || (level >= 2 && (h.hexed > 0 || h.chilled > 0 || h.mired > 0)))
                    .min_by(|(ia,a),(ib,b)| (u64::from(a.hp)*u64::from(b.max_hp)).cmp(&(u64::from(b.hp)*u64::from(a.max_hp))).then(ia.cmp(ib))).map(|(&id,_)| id);
                if let Some(id) = target {
                    let h = self.players.get_mut(&id).unwrap();
                    h.mend(damage, stakes::Mend::Party);
                    if level >= 2 { h.hexed = 0; h.chilled = 0; h.mired = 0; }
                    ally.cooldown = 2*HZ;
                    self.cues.push(format!("squires:heal:{id}"));
                }
                if level >= 3 {
                    // Bring an uncarried skull within pickup reach of a living
                    // knight. This is assistance, never a free resurrection.
                    let carried: Vec<u32> = self.players.values().filter_map(|h| h.carrying_skull).collect();
                    let skull = self.players.iter().filter(|&(&id,h)| h.hp == 0 && !h.absent && !carried.contains(&id))
                        .filter_map(|(&id,h)| h.skull_at.map(|at| (id,at)))
                        .filter(|(_,at)| (at.0-ally.x).hypot(at.1-ally.y) < 8.0).next();
                    let living = ally.summoner.and_then(|id| self.players.get(&id)).filter(|h| h.hp > 0).or_else(|| self.players.values().find(|h| h.hp > 0 && !h.absent)).map(|h| (h.x,h.y));
                    if let (Some((id,at)),Some(to)) = (skull,living) {
                        let (ux,uy) = unit(to.0-at.0,to.1-at.1);
                        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
                        let at = grid.slide(at,(ux*1.5,uy*1.5),HERO_RADIUS,Mover::Hero);
                        self.players.get_mut(&id).unwrap().skull_at = Some(at);
                        ally.cooldown = ally.cooldown.max(HZ/3);
                        self.cues.push(format!("squires:skull:{id}"));
                    }
                }
            }
            AllyKind::Ninja => {
                let Some(i) = self.guild_squire_target(ally, 17.0) else { return; };
                let e = &self.enemies[i];
                let close = (e.x-ally.x).hypot(e.y-ally.y) < e.radius()+1.7;
                let behind = ally.summoner.and_then(|id| self.players.get(&id)).is_some_and(|h| (h.x-e.x)*(ally.x-e.x)+(h.y-e.y)*(ally.y-e.y) < 0.0);
                if level >= 2 && close && behind {
                    self.guild_squire_hit(ally,i,damage*3); self.cues.push("squires:backstab".into());
                } else { self.guild_squire_shot(ally,i,damage,Shot::Blade,18.0,ShotTraits::default()); }
                if level >= 3 && (ally.hp*2 < ally.max_hp || self.projectiles.iter().any(|p| p.hostile && (p.x-ally.x).hypot(p.y-ally.y) < 4.0)) {
                    ally.taunt = HZ/2; ally.invulnerable = HZ/2; self.cues.push("squires:smoke".into());
                }
                ally.cooldown = HZ/2;
            }
            AllyKind::Ranger => {
                let Some(i) = self.guild_squire_target(ally, 24.0) else { return; };
                let id = self.enemies[i].id;
                if level >= 2 {
                    self.squire_guild.marks.retain(|m| m.owner != ally.summoner.unwrap_or(1));
                    self.squire_guild.marks.push(Mark { enemy: id, owner: ally.summoner.unwrap_or(1), left: 3*HZ });
                    self.cues.push(format!("squires:mark:{id}"));
                }
                self.guild_squire_shot(ally,i,damage,Shot::Arrow,20.0, ShotTraits::default());
                ally.cooldown = 4*HZ/5;
            }
            AllyKind::Shieldbearer => {
                if level >= 2 { ally.taunt = 2*HZ; }
                if let Some(i) = self.guild_squire_target(ally, 2.8) {
                    self.guild_squire_hit(ally,i,damage);
                    if !hireling::great(&self.enemies[i]) { self.enemies[i].frozen = self.enemies[i].frozen.max(HZ/3); }
                }
                if level >= 3 {
                    if let Some(h) = ally.summoner.and_then(|id| self.players.get_mut(&id)).filter(|h| h.hp > 0 && (h.x-ally.x).hypot(h.y-ally.y) < 5.0) { h.invulnerable = h.invulnerable.max(HZ/4); }
                }
                ally.cooldown = 2*HZ;
            }
            AllyKind::Alchemist => {
                let Some(i) = self.guild_squire_target(ally, 15.0) else { return; };
                let centre = (self.enemies[i].x,self.enemies[i].y);
                let victims: Vec<usize> = self.enemies.iter().enumerate().filter(|(_,e)| e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy && (e.x-centre.0).hypot(e.y-centre.1) < 2.8).map(|(i,_)| i).collect();
                for i in victims { self.guild_squire_hit(ally,i,damage); if level >= 2 && !hireling::great(&self.enemies[i]) { self.enemies[i].frozen = self.enemies[i].frozen.max(HZ/2); } }
                if level >= 3 {
                    if let Some(h) = ally.summoner.and_then(|id| self.players.get_mut(&id)).filter(|h| h.hp > 0) { h.mend(damage/2, stakes::Mend::Party); }
                }
                self.cues.push("squires:flask".into()); ally.cooldown = 2*HZ;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn squire_guild_legacy_and_combined_saves_preserve_both_models() {
        let mut run = Run::at_home(8, 1, None, home::Home::default(), Spoils::default());
        run.home.squire_guild.clock = 42;
        run.squire_guild.menus.insert(1, Menu { plate: 2, ..Default::default() });
        let mut legacy = serde_json::to_value(&run).unwrap();
        let obj = legacy.as_object_mut().unwrap();
        let field = obj.remove("squire_guild").unwrap();
        obj.insert("squires".into(), field);
        let home = obj.get_mut("home").unwrap().as_object_mut().unwrap();
        let guild = home.remove("squire_guild").unwrap();
        home.insert("squires".into(), guild);
        let loaded: Run = serde_json::from_value(legacy).unwrap();
        assert_eq!(loaded.home.squire_guild.clock, 42);
        assert_eq!(loaded.squire_guild.menus[&1].plate, 2);
        let roundtrip: Run = serde_json::from_value(serde_json::to_value(&loaded).unwrap()).unwrap();
        assert_eq!(roundtrip.home.squire_guild, loaded.home.squire_guild);
        assert_eq!(roundtrip.squire_guild, loaded.squire_guild);
        assert_eq!(roundtrip.home.squires, loaded.home.squires);
    }

    #[test]
    fn squire_guild_plates_do_not_overlap_other_squire_controls() {
        let run = Run::at_home(8, 1, None, home::Home::default(), Spoils::default());
        // The guild has its own room off the Yard (`squire_rooms`).
        let room = run.dungeon.rooms.iter().find(|r| r.kind == RoomKind::SquireGuild).unwrap();
        for &(c, r, w, h) in &PLATES {
            for x in c..c + w {
                for y in r..r + h {
                    assert_eq!(room.tile(x, y), Tile::Floor);
                    let (x, y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS);
                    assert!(super::super::squires::action_at(x, y).is_none());
                    assert!(super::super::squire_hall::plate_at(x, y).is_none());
                    assert!(super::super::countryside_squires::plate(x, y).is_none());
                    assert!(super::super::lane_squires::action_at(&Default::default(), x, y).is_none());
                }
            }
        }
    }
}
