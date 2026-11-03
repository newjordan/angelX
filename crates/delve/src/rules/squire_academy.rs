//! The yard's living companions, not the Court's holding-garrison retainers.
//! One persistent roster per knight (`talents::knight_key`), one selected
//! squire per delve. Everything, including recruitment rotation and training,
//! advances on the authoritative simulation, never on wall-clock time.
use super::*;
mod combat;
use super::hireling::{AllyKind, Hireling};

pub const MAX_LEVEL: u8 = 5;
pub const MAX_BUILDING: u8 = 3;
pub const RECRUIT_EVERY: u64 = 3; // completed departures, not client clock
pub const TRAIN_SECONDS: u32 = 10;
pub const RECRUIT_AT: [(f32, f32); 5] = [(2., 7.5), (4., 7.5), (6., 7.5), (8., 7.5), (10., 7.5)];
pub const MANAGE_AT: [(f32, f32); 3] = [(12., 7.5), (14., 7.5), (16., 7.5)];
pub const BUILD_AT: [(f32, f32); 3] = [(18., 7.5), (20., 7.5), (22., 7.5)];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind { Ninja, Healer, Ranger, ShieldBearer, Alchemist }
impl Kind {
    pub fn ally(self) -> AllyKind { match self {
        Self::Ninja => AllyKind::SquireNinja, Self::Healer => AllyKind::SquireHealer,
        Self::Ranger => AllyKind::SquireRanger, Self::ShieldBearer => AllyKind::SquireShieldBearer,
        Self::Alchemist => AllyKind::SquireAlchemist,
    } }

    pub const ALL: [Self; 5] = [Self::Ninja, Self::Healer, Self::Ranger, Self::ShieldBearer, Self::Alchemist];
    pub fn name(self) -> &'static str { match self { Self::Ninja => "Ninja", Self::Healer => "Healer", Self::Ranger => "Ranger", Self::ShieldBearer => "Shield bearer", Self::Alchemist => "Alchemist" } }
    pub fn abilities(self, level: u8) -> &'static str { match (self, level) {
        (Self::Ninja, 1) => "flank / backstab", (Self::Ninja, 2) => "backstab / stars", (Self::Ninja, _) => "backstab / stars / smoke",
        (Self::Healer, 1) => "ranged lowest-HP heal", (Self::Healer, 2) => "heal / cleanse", (Self::Healer, _) => "heal / cleanse / skull revive",
        (Self::Ranger, 1) => "long-range arrows", (Self::Ranger, _) => "arrows / prey mark",
        (Self::ShieldBearer, 1) => "bodyblock / shield bash", (Self::ShieldBearer, _) => "bodyblock / bash / taunt",
        (Self::Alchemist, 1) => "area bombs", (Self::Alchemist, _) => "bombs / healing potions",
    } }
    pub fn stats(self, level: u8, yard: u8) -> Stats {
        let (hp, damage, reach, every) = match self {
            Self::Ninja => (75, 16, 2., HZ), Self::Healer => (65, 10, 14., 2 * HZ),
            Self::Ranger => (70, 10, 22., HZ), Self::ShieldBearer => (145, 12, 2.5, HZ), Self::Alchemist => (80, 13, 13., 2 * HZ),
        };
        Stats { hp: hp + u32::from(level.saturating_sub(1)) * 18 + u32::from(yard) * 10, damage: damage + u32::from(level.saturating_sub(1)) * 4, reach, every }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Stats { pub hp: u32, pub damage: u32, pub reach: f32, pub every: u32 }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recruit { pub id: String, pub name: String, pub kind: Kind, pub price: u32 }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squire {
    pub recruit: Recruit,
    pub level: u8,
    #[serde(default)] pub delves: u32,
    #[serde(default)] pub training: u32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roster { #[serde(default)] pub members: Vec<Squire>, #[serde(default)] pub selected: Option<String> }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Academy {
    #[serde(default)] pub board: u8,
    #[serde(default)] pub hall: u8,
    #[serde(default)] pub yard: u8,
    #[serde(default)] pub departures: u64,
    /// Cumulative spend is settled once by the host, like a treasury receipt.
    #[serde(default)] pub paid: u32,
    #[serde(default)] pub rosters: BTreeMap<String, Roster>,
}
impl Academy {
    pub fn cap(&self) -> usize { 3 + usize::from(self.board.min(MAX_BUILDING)) }
    pub fn level_cap(&self) -> u8 { (2 + self.hall).min(MAX_LEVEL) }
    pub fn cycle(&self) -> u64 { self.departures / RECRUIT_EVERY }
    pub fn recruits(&self) -> Vec<Recruit> {
        const NAMES: [[&str; 3]; 5] = [["Kestrel", "Sable", "Juniper"], ["Miri", "Anwen", "Clement"], ["Rowan", "Ash", "Briar"], ["Pip", "Tamsin", "Bram"], ["Fenn", "Ada", "Sorrel"]];
        Kind::ALL.into_iter().enumerate().map(|(i, kind)| Recruit {
            id: format!("{}:{i}", self.cycle()), name: NAMES[i][self.cycle() as usize % 3].into(), kind,
            price: (100 + i as u32 * 15).saturating_sub(u32::from(self.board) * 10),
        }).collect()
    }
    pub fn valid(&self) -> bool {
        self.board <= MAX_BUILDING && self.hall <= MAX_BUILDING && self.yard <= MAX_BUILDING && self.rosters.len() <= 256
            && self.rosters.iter().all(|(key, r)| key.len() <= 128 && r.members.len() <= self.cap()
                && r.members.iter().enumerate().all(|(i, s)| (1..=MAX_LEVEL).contains(&s.level) && s.recruit.name.len() <= 40 && s.recruit.id.len() <= 40
                    && s.training <= TRAIN_SECONDS * HZ * u32::from(MAX_LEVEL) && r.members[..i].iter().all(|m| m.recruit.id != s.recruit.id))
                && r.selected.as_ref().is_none_or(|id| r.members.iter().any(|s| &s.recruit.id == id)))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Companion { #[serde(default)] pub reviving: Option<(u32, u32)>, pub body: Hireling, pub name: String, pub level: u8, #[serde(default)] pub special: u32 }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Field {
    #[serde(default)] pub active: BTreeMap<u32, Companion>,
    /// Enemy id -> (owner, ticks). Marks grant only this companion bonus damage.
    #[serde(default)] pub marks: BTreeMap<u32, (u32, u32)>,
    #[serde(default)] pub interact: BTreeMap<u32, bool>,
    #[serde(default)] pub notices: BTreeMap<u32, String>,
}

/// Transfer the sim's persistent state and charge only its new receipts. The
/// cockpit's save rollback also rolls this back; no client can spend directly.
fn gold(amount: u32) -> Spoils { let mut spoils = Spoils::default(); spoils.add(Spoil::Gold, amount); spoils }

pub fn settle(saved: &mut Academy, treasury: &mut Spoils, current: &Academy) -> bool {
    if saved == current || current.paid < saved.paid || !current.valid() { return false; }
    let cost = current.paid - saved.paid;
    if treasury.get(Spoil::Gold) < cost { return false; }
    treasury.take(&gold(cost));
    *saved = current.clone();
    true
}

impl Run {
    fn academy_squire_key(&self, id: u32) -> Option<String> { self.players.get(&id).map(super::talents::knight_key) }
    fn squire_pay(&mut self, gold: u32) -> Result<(), String> {
        if self.treasury.get(Spoil::Gold) < gold { return Err(format!("Need {gold} treasury gold")); }
        self.treasury.take(&self::gold(gold));
        self.home.squire_academy.paid = self.home.squire_academy.paid.saturating_add(gold);
        Ok(())
    }
    /// The same server-validated actions power the yard's plates and tests.
    pub fn academy_squire_action(&mut self, id: u32, action: Action) -> Result<String, String> {
        if !self.at_home_now() || self.room().kind != RoomKind::SquireAcademy { return Err("Visit the Academy, off the training yard".into()); }
        let hero = self.players.get(&id).filter(|h| h.hp > 0 && !h.stone && !h.absent).ok_or("Knight unavailable")?;
        let key = super::talents::knight_key(hero);
        if key.len() > 128 || (!self.home.squire_academy.rosters.contains_key(&key) && self.home.squire_academy.rosters.len() >= 256) { return Err("Realm roster ledger full".into()); }
        match action {
            Action::Hire(i) => {
                let recruit = self.home.squire_academy.recruits().get(i).cloned().ok_or("No recruit")?;
                let cap = self.home.squire_academy.cap();
                let r = self.home.squire_academy.rosters.get(&key);
                if r.is_some_and(|r| r.members.len() >= cap) { return Err(format!("Roster full ({cap}); upgrade board or dismiss")); }
                if r.is_some_and(|r| r.members.iter().any(|s| s.recruit.id == recruit.id)) { return Err("Already hired this recruit; board refreshes every 3 delves".into()); }
                self.squire_pay(recruit.price)?;
                let r = self.home.squire_academy.rosters.entry(key).or_default();
                let name = recruit.name.clone();
                r.selected = Some(recruit.id.clone());
                r.members.push(Squire { recruit, level: 1, delves: 0, training: 0 });
                Ok(format!("Hired {name}; selected for next delve"))
            }
            Action::Select => {
                let r = self.home.squire_academy.rosters.get_mut(&key).ok_or("Hire a squire first")?;
                if r.members.is_empty() { return Err("Hire a squire first".into()); }
                let next = r.selected.as_ref().and_then(|id| r.members.iter().position(|s| &s.recruit.id == id)).map_or(0, |i| i + 1);
                r.selected = r.members.get(next).map(|s| s.recruit.id.clone());
                Ok(r.members.get(next).map_or("No active squire".into(), |s| format!("{} selected", s.recruit.name)))
            }
            Action::Train => {
                let r = self.home.squire_academy.rosters.get(&key).ok_or("Hire and select a squire")?;
                let s = r.members.iter().find(|s| Some(&s.recruit.id) == r.selected.as_ref()).ok_or("Select a squire")?;
                if s.training > 0 { return Err("Training already underway".into()); }
                if s.level >= self.home.squire_academy.level_cap() { return Err("Upgrade training hall to raise level cap (maximum 5)".into()); }
                if s.delves < u32::from(s.level) { return Err(format!("Needs {} delves; has {}", s.level, s.delves)); }
                let cost = 80 * u32::from(s.level);
                self.squire_pay(cost)?;
                let r = self.home.squire_academy.rosters.get_mut(&key).unwrap();
                let s = r.members.iter_mut().find(|s| Some(&s.recruit.id) == r.selected.as_ref()).unwrap();
                s.training = TRAIN_SECONDS * HZ * u32::from(s.level);
                Ok(format!("{} training: {}g, {}s in town", s.recruit.name, cost, s.training / HZ))
            }
            Action::Dismiss => {
                let r = self.home.squire_academy.rosters.get_mut(&key).ok_or("No roster")?;
                let pos = r.members.iter().position(|s| Some(&s.recruit.id) == r.selected.as_ref()).ok_or("Select a squire to dismiss")?;
                let s = r.members.remove(pos); r.selected = None;
                Ok(format!("{} dismissed (no refund)", s.recruit.name))
            }
            Action::Upgrade(i) => {
                let level = match i { 0 => self.home.squire_academy.board, 1 => self.home.squire_academy.hall, 2 => self.home.squire_academy.yard, _ => return Err("No building".into()) };
                if level >= MAX_BUILDING { return Err("Building at maximum level 3".into()); }
                let cost = 250 * (u32::from(level) + 1);
                self.squire_pay(cost)?;
                match i { 0 => self.home.squire_academy.board += 1, 1 => self.home.squire_academy.hall += 1, _ => self.home.squire_academy.yard += 1 }
                Ok(format!("{} level {} ({cost}g)", ["Recruit board", "Training hall", "Squire yard"][i], level + 1))
            }
        }
    }
    pub(super) fn tick_academy_yard(&mut self, inputs: &BTreeMap<u32, Input>) {
        let ids: Vec<_> = self.players.keys().copied().collect();
        for id in ids {
            let held = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            let was = self.squire_academy.interact.insert(id, held).unwrap_or(false);
            if !held || was { continue; }
            let h = &self.players[&id];
            let near = |at: (f32, f32)| (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 0.45 * TILE_UNITS;
            let action = RECRUIT_AT.iter().position(|&p| near(p)).map(Action::Hire)
                .or_else(|| BUILD_AT.iter().position(|&p| near(p)).map(Action::Upgrade))
                .or_else(|| MANAGE_AT.iter().position(|&p| near(p)).map(|i| [Action::Select, Action::Train, Action::Dismiss][i]));
            if let Some(action) = action { let text = self.academy_squire_action(id, action).unwrap_or_else(|e| e); self.squire_academy.notices.insert(id, text); }
        }
    }
    pub(super) fn embark_squires(&mut self) {
        self.squire_academy.active.clear(); self.squire_academy.marks.clear();
        let ids: Vec<_> = self.players.keys().copied().collect();
        for id in ids {
            let Some(key) = self.academy_squire_key(id) else { continue; };
            let Some(r) = self.home.squire_academy.rosters.get_mut(&key) else { continue; };
            let Some(s) = r.members.iter_mut().find(|s| Some(&s.recruit.id) == r.selected.as_ref() && s.training == 0) else { continue; };
            s.delves = s.delves.saturating_add(1);
            let stats = s.recruit.kind.stats(s.level, self.home.squire_academy.yard);
            let h = &self.players[&id];
            let mut body = Hireling::new(self.dungeon.depth, (h.x, h.y));
            body.kind = s.recruit.kind.ally(); body.summoner = Some(id);
            body.hp = stats.hp; body.max_hp = stats.hp; body.power = u32::from(s.level);
            self.squire_academy.active.insert(id, Companion { body, name: s.recruit.name.clone(), level: s.level, special: 0, reviving: None });
        }
        self.home.squire_academy.departures = self.home.squire_academy.departures.saturating_add(1);
    }
    pub(super) fn academy_squires_follow(&mut self) {
        self.squire_academy.interact.clear();
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        for (&id, c) in &mut self.squire_academy.active {
            if let Some(h) = self.players.get(&id) {
                (c.body.x, c.body.y) = super::hireling::beside(&grid, (h.x, h.y));
                if c.body.hp == 0 { c.body.hp = (c.body.max_hp / 2).max(1); }
                c.body.invulnerable = 20;
            }
        }
        self.squire_academy.marks.clear();
    }
    pub(super) fn tick_academy_squires(&mut self) {
        if self.at_home_now() {
            for r in self.home.squire_academy.rosters.values_mut() { for s in &mut r.members {
                if s.training > 0 { s.training -= 1; if s.training == 0 { s.level = (s.level + 1).min(MAX_LEVEL); } }
            } }
            return;
        }
        self.squire_academy.marks.retain(|_, (_, ticks)| { *ticks = ticks.saturating_sub(1); *ticks > 0 });
        let mut active = std::mem::take(&mut self.squire_academy.active);
        for (&id, c) in &mut active {
            if self.players.get(&id).is_none_or(|h| h.absent) { continue; }
            c.special = c.special.saturating_sub(1);
            if c.body.hp == 0 { c.reviving = None; }
            // Reuse collision, downing, room recovery, deterministic movement.
            self.drive_hireling(&mut c.body);
            if c.body.hp > 0 { self.squire_special(c); }
        }
        self.squire_academy.active = active;
    }
}
impl Field {
    pub fn valid(&self) -> bool {
        self.active.len() <= MAX_PLAYERS as usize && self.marks.len() <= 512
            && self.active.iter().all(|(&id, c)| (1..=MAX_PLAYERS).contains(&id)
                && c.body.kind.squire().is_some() && c.body.summoner == Some(id)
                && c.body.x.is_finite() && c.body.y.is_finite() && c.body.hp <= c.body.max_hp
                && (1..=MAX_LEVEL).contains(&c.level) && c.body.power == u32::from(c.level) && c.name.len() <= 40)
            && self.interact.len() <= MAX_PLAYERS as usize && self.notices.len() <= MAX_PLAYERS as usize
            && self.notices.values().all(|n| n.len() <= 256)
    }
    pub(super) fn roared(&self, enemy: &Enemy, seen: &mut Vec<(f32, f32)>) {
        if super::hireling::great(enemy) { return; }
        if let Some(c) = self.active.values().filter(|c| c.body.kind == AllyKind::SquireShieldBearer
            && c.body.hp > 0 && c.body.taunt > 0
            && (c.body.x - enemy.x).hypot(c.body.y - enemy.y) < super::hireling::ROAR_REACH + enemy.radius())
            .min_by(|a, b| (a.body.x - enemy.x).hypot(a.body.y - enemy.y).total_cmp(&(b.body.x - enemy.x).hypot(b.body.y - enemy.y))) {
            seen.clear(); seen.push((c.body.x, c.body.y));
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Action { Hire(usize), Select, Train, Dismiss, Upgrade(usize) }

#[cfg(test)]
#[path = "../../tests/rules/together_squire_academy__tests.rs"]
mod tests;
