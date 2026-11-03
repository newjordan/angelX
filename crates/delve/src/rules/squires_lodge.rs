//! Name-owned, treasury-paid companions. The lodge's ledger lives in Home;
//! each hero carries only their current dungeon body. Neither is a party seat.
//! All choices and clocks are deterministic; no gameplay RNG is consumed.
use super::hireling::{AllyKind, Hireling};
use super::*;
use crate::realm::{Spoil, Spoils};
use std::collections::BTreeSet;

pub const MAX_BUILDING: u8 = 3;
pub const MAX_ROSTER: usize = 12;
pub const MAX_LEVEL: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Ninja,
    Healer,
    Ranger,
    Shieldbearer,
    Alchemist,
}
impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Ninja,
        Self::Healer,
        Self::Ranger,
        Self::Shieldbearer,
        Self::Alchemist,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ninja => "Ninja",
            Self::Healer => "Healer",
            Self::Ranger => "Ranger",
            Self::Shieldbearer => "Shieldbearer",
            Self::Alchemist => "Alchemist",
        }
    }
    pub fn health(self, level: u8) -> u32 {
        (match self {
            Self::Shieldbearer => 160,
            Self::Ninja => 75,
            _ => 95,
        }) + 15 * u32::from(level.saturating_sub(1).min(9))
    }
    pub fn damage(self, level: u8) -> u32 {
        (match self {
            Self::Ninja => 14,
            Self::Healer => 6,
            Self::Ranger => 18,
            Self::Shieldbearer => 10,
            Self::Alchemist => 22,
        }) + 3 * u32::from(level.saturating_sub(1).min(9))
    }
}
/// Training milestones shared by combat and the lodge's ability preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ability {
    Flank,
    Stars,
    Backstab,
    Vanish,
    Heal,
    Cleanse,
    Escort,
    Arrows,
    Mark,
    Guard,
    Bash,
    Bombs,
    Splash,
    Potions,
}
impl Ability {
    pub fn name(self) -> &'static str {
        match self {
            Self::Flank => "FLANK",
            Self::Stars => "STARS",
            Self::Backstab => "BACKSTAB",
            Self::Vanish => "VANISH",
            Self::Heal => "HEAL",
            Self::Cleanse => "CLEANSE",
            Self::Escort => "ESCORT",
            Self::Arrows => "ARROWS",
            Self::Mark => "MARKED FOE",
            Self::Guard => "GUARD",
            Self::Bash => "SHIELD BASH",
            Self::Bombs => "BOMBS",
            Self::Splash => "SPLASH",
            Self::Potions => "POTIONS",
        }
    }
}
impl Kind {
    pub fn abilities(self) -> &'static [(Ability, u8)] {
        use Ability::*;
        match self {
            Self::Ninja => &[(Flank, 1), (Stars, 2), (Backstab, 2), (Vanish, 3)],
            Self::Healer => &[(Heal, 1), (Cleanse, 2), (Escort, 3)],
            Self::Ranger => &[(Arrows, 1), (Mark, 2)],
            Self::Shieldbearer => &[(Guard, 1), (Bash, 2)],
            Self::Alchemist => &[(Bombs, 1), (Splash, 2), (Potions, 3)],
        }
    }
    /// Only abilities this member can actually use, with the next milestones
    /// available separately through `abilities` for training previews.
    pub fn ability_summary(self, level: u8) -> String {
        self.abilities()
            .iter()
            .filter(|&&(_, at)| level >= at)
            .map(|&(ability, _)| ability.name())
            .collect::<Vec<_>>()
            .join(" / ")
    }
    pub fn unlocked(self, level: u8, ability: Ability) -> bool {
        self.abilities()
            .iter()
            .any(|&(a, at)| a == ability && level >= at)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Building {
    Recruiting,
    Hiring,
    Training,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Hire(u64),
    Train(u64),
    Select(Option<u64>),
    Upgrade(Building),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NotHome,
    NoKnight,
    Funds,
    Missing,
    AlreadyHired,
    RosterFull,
    MaxLevel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recruit {
    pub id: u64,
    pub name: String,
    pub kind: Kind,
    pub level: u8,
}
pub type Member = Recruit;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roster {
    pub members: Vec<Member>,
    pub active: Option<u64>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Guild {
    pub recruiting: u8,
    pub hiring: u8,
    pub training: u8,
    pub delves: u64,
    pub rent_days: u64,
    pub rosters: BTreeMap<String, Roster>,
}

/// Normalized *player name*, deliberately not Hero.knight (a class id shared
/// by different players). Seats and class changes never transfer a roster.
pub fn owner_key(hero: &Hero) -> String {
    hero.name.trim().to_lowercase()
}
fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}
impl Guild {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
    /// One board per departure, not per floor/room/tick or UI visit.
    pub fn advance_delve(&mut self) {
        self.delves = self.delves.saturating_add(1);
    }
    /// Driver calls exactly once after rent-day settlement.
    pub fn rent_day(&mut self) {
        self.rent_days = self.rent_days.saturating_add(1);
    }
    pub fn recruits(&self, owner: &str) -> Vec<Recruit> {
        const FIRST: [&str; 16] = [
            "Ada", "Bram", "Cora", "Dara", "Edda", "Finn", "Gwen", "Hugh", "Iris", "Jory", "Kit",
            "Lark", "Mina", "Nell", "Orrin", "Pip",
        ];
        const LAST: [&str; 16] = [
            "Ash", "Brook", "Clover", "Dusk", "Elm", "Flint", "Glen", "Hawthorn", "Ivy", "Juniper",
            "Kestrel", "Linden", "Moss", "Nettle", "Oak", "Reed",
        ];
        let owner = owner.trim().to_lowercase();
        (0..5 + usize::from(self.recruiting.min(MAX_BUILDING)))
            .map(|i| {
                let id = hash(&format!(
                    "squire:{owner}:{}:{}:{i}",
                    self.delves, self.rent_days
                ));
                Recruit {
                    id,
                    name: format!(
                        "{} {}",
                        FIRST[(id & 15) as usize],
                        LAST[((id >> 8) & 15) as usize]
                    ),
                    kind: Kind::ALL[i % 5],
                    level: 1 + self.recruiting.min(MAX_BUILDING) / 2,
                }
            })
            .collect()
    }
    pub fn roster_cap(&self) -> usize {
        3 * (1 + usize::from(self.hiring.min(MAX_BUILDING)))
    }
    pub fn hire_price(&self, kind: Kind) -> u32 {
        let base = match kind {
            Kind::Ninja => 150,
            Kind::Healer => 180,
            Kind::Ranger => 140,
            Kind::Shieldbearer => 160,
            Kind::Alchemist => 170,
        };
        base * (100 - 15 * u32::from(self.hiring.min(MAX_BUILDING))) / 100
    }
    pub fn training_price(&self, member: &Member) -> u32 {
        80 * u32::from(member.level.max(1))
    }
    pub fn training_cap(&self) -> u8 {
        2 + self.training.min(MAX_BUILDING) * 2
    }
    pub fn level(&self, building: Building) -> u8 {
        match building {
            Building::Recruiting => self.recruiting,
            Building::Hiring => self.hiring,
            Building::Training => self.training,
        }
    }
    pub fn upgrade_price(&self, building: Building) -> Option<u32> {
        let level = self.level(building);
        (level < MAX_BUILDING).then_some(250 * (u32::from(level) + 1))
    }
}

/// Runtime uses the same health/grace/motion body as existing allies, without
/// occupying Run.hireling or minions. Its kind here is not an AllyKind variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Companion {
    pub owner: String,
    pub member: Member,
    pub body: Hireling,
    pub vanished: u32,
    pub special: u32,
    pub support: u32,
    pub mark: Option<u32>,
    pub mark_left: u32,
    /// Skull being escorted, still pickable by real knights.
    #[serde(default)]
    pub skull: Option<u32>,
    pub floor: u32,
}
impl Companion {
    fn new(owner: String, member: Member, depth: u32, at: (f32, f32), seat: u32) -> Self {
        let mut body = Hireling::minion(AllyKind::Beaumains, 0, depth, at, seat);
        body.max_hp = member.kind.health(member.level);
        body.hp = body.max_hp;
        Self {
            owner,
            member,
            body,
            vanished: 0,
            special: 0,
            support: 0,
            mark: None,
            mark_left: 0,
            skull: None,
            floor: depth,
        }
    }
    pub fn kind(&self) -> Kind {
        self.member.kind
    }
    pub fn down(&self) -> bool {
        self.body.down()
    }
    pub(super) fn shot_radius(&self) -> f32 {
        if self.member.kind == Kind::Shieldbearer {
            2.4
        } else {
            HERO_RADIUS
        }
    }
    pub(super) fn take_shot(&mut self, damage: u32) {
        self.body
            .intercept(damage, self.member.kind == Kind::Shieldbearer);
    }
    pub fn damage(&self) -> u32 {
        self.kind().damage(self.member.level)
    }
    pub fn valid(&self) -> bool {
        self.body.x.is_finite()
            && self.body.y.is_finite()
            && self.body.hp <= self.body.max_hp
            && self.body.max_hp == self.kind().health(self.member.level)
            && (1..=MAX_LEVEL).contains(&self.member.level)
    }
    /// Shield intercepts damage at its owner's shoulder, after the owner's
    /// armor. Taking a hit still costs the squire HP; no permanent immunity.
    pub(super) fn guard(&mut self, x: f32, y: f32, damage: u32) -> u32 {
        if self.kind() != Kind::Shieldbearer
            || self.down()
            || (self.body.x - x).hypot(self.body.y - y) > 3.5
        {
            return damage;
        }
        let caught = (damage / 2).min(self.body.hp);
        self.body.hp -= caught;
        damage - caught
    }
}

/// Pure snapshot for all renderers; seat ink matches existing knight cloth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub owner: u32,
    pub owner_name: String,
    pub owner_ink: char,
    pub id: u64,
    pub name: String,
    pub kind: Kind,
    pub level: u8,
    pub x: f32,
    pub y: f32,
    pub facing: f32,
    pub hp: u32,
    pub max_hp: u32,
    pub damage: u32,
    pub downed: bool,
    pub vanished: bool,
    pub cooldown: u32,
    pub mark: Option<u32>,
}
impl Run {
    pub fn squire_roster(&self, seat: u32) -> Option<&Roster> {
        self.home
            .squires_lodge
            .rosters
            .get(&owner_key(self.players.get(&seat)?))
    }
    pub fn squire_recruits(&self, seat: u32) -> Vec<Recruit> {
        self.players
            .get(&seat)
            .map_or_else(Vec::new, |h| self.home.squires_lodge.recruits(&owner_key(h)))
    }
    pub fn squire_action(&mut self, seat: u32, action: Action) -> Result<(), Error> {
        if !self.at_home_now() {
            return Err(Error::NotHome);
        }
        let hero = self
            .players
            .get(&seat)
            .filter(|h| h.hp > 0 && !h.absent && !h.away)
            .ok_or(Error::NoKnight)?;
        let key = owner_key(hero);
        // Validate against immutable ledger; a failed payment changes nothing.
        let guild = &self.home.squires_lodge;
        let roster = guild.rosters.get(&key);
        let mut recruit = None;
        let cost = match action {
            Action::Hire(id) => {
                if roster.is_some_and(|r| r.members.iter().any(|m| m.id == id)) {
                    return Err(Error::AlreadyHired);
                }
                if roster.is_some_and(|r| r.members.len() >= self.home.squires_lodge.roster_cap()) {
                    return Err(Error::RosterFull);
                }
                let offer = guild
                    .recruits(&key)
                    .into_iter()
                    .find(|r| r.id == id)
                    .ok_or(Error::Missing)?;
                let cost = guild.hire_price(offer.kind);
                recruit = Some(offer);
                cost
            }
            Action::Train(id) => {
                let member = roster
                    .and_then(|r| r.members.iter().find(|m| m.id == id))
                    .ok_or(Error::Missing)?;
                if member.level >= guild.training_cap() {
                    return Err(Error::MaxLevel);
                }
                guild.training_price(member)
            }
            Action::Select(id) => {
                if id
                    .is_some_and(|id| !roster.is_some_and(|r| r.members.iter().any(|m| m.id == id)))
                {
                    return Err(Error::Missing);
                }
                0
            }
            Action::Upgrade(b) => guild.upgrade_price(b).ok_or(Error::MaxLevel)?,
        };
        let price = Spoils(BTreeMap::from([(Spoil::Gold, cost)]));
        if !self.treasury.covers(&price) {
            return Err(Error::Funds);
        }
        self.treasury.take(&price);
        let guild = &mut self.home.squires_lodge;
        match action {
            Action::Hire(_) => {
                let r = guild.rosters.entry(key).or_default();
                let member = recruit.unwrap();
                r.active = Some(member.id);
                r.members.push(member);
            }
            Action::Train(id) => {
                guild
                    .rosters
                    .get_mut(&key)
                    .unwrap()
                    .members
                    .iter_mut()
                    .find(|m| m.id == id)
                    .unwrap()
                    .level += 1;
            }
            Action::Select(id) => {
                guild.rosters.entry(key).or_default().active = id;
            }
            Action::Upgrade(b) => match b {
                Building::Recruiting => guild.recruiting += 1,
                Building::Hiring => guild.hiring += 1,
                Building::Training => guild.training += 1,
            },
        }
        self.sync_squires();
        Ok(())
    }
    pub fn squire_views(&self) -> Vec<View> {
        self.players
            .iter()
            .filter(|(_, h)| !h.absent && !h.away)
            .filter_map(|(&id, h)| {
                let s = h.squire.as_ref().filter(|s| s.owner == owner_key(h))?;
                Some(View {
                    owner: id,
                    owner_name: s.owner.clone(),
                    owner_ink: match id {
                        1 => '7',
                        2 => '1',
                        3 => 'A',
                        4 => '4',
                        5 => '2',
                        _ => 'c',
                    },
                    id: s.member.id,
                    name: s.member.name.clone(),
                    kind: s.kind(),
                    level: s.member.level,
                    x: s.body.x,
                    y: s.body.y,
                    facing: s.body.facing,
                    hp: s.body.hp,
                    max_hp: s.body.max_hp,
                    damage: s.damage(),
                    downed: s.down(),
                    vanished: s.vanished > 0,
                    cooldown: s.body.cooldown,
                    mark: s.mark,
                })
            })
            .collect()
    }
    /// Visible companion bodies are ordinary decoys, not party members. This
    /// iterator is shared by authoritative foe sight and warning previews.
    pub(super) fn squire_decoys(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.players
            .iter()
            .filter(|(seat, h)| {
                !h.absent && !h.away && !h.stone && !self.exits.outside.contains_key(seat)
            })
            .filter_map(|(_, h)| h.squire.as_ref().filter(|s| s.owner == owner_key(h)))
            .filter(|s| !s.down() && s.vanished == 0)
            .map(|s| (s.body.x, s.body.y))
    }
    pub(super) fn sync_squires(&mut self) {
        if self.home.squires_lodge.rosters.is_empty() {
            for hero in self.players.values_mut() {
                hero.squire = None;
            }
            return;
        }
        // Reconcile the whole identity at once: presence changes transfer the
        // existing body, including wounds and every clock, rather than respawn it.
        let barred = self.barred();
        let mut bodies = BTreeMap::new();
        for (&id, h) in &mut self.players {
            if let Some(s) = h.squire.take() {
                bodies.entry(s.owner.clone()).or_insert((id, s));
            }
        }
        let keys: BTreeSet<_> = self.players.values().map(owner_key).collect();
        for key in keys {
            let Some(member) = self
                .home
                .squires_lodge
                .rosters
                .get(&key)
                .and_then(|r| {
                    r.active
                        .and_then(|id| r.members.iter().find(|m| m.id == id))
                })
                .cloned()
            else {
                continue;
            };
            let existing = bodies
                .remove(&key)
                .filter(|(_, s)| s.member.id == member.id);
            // Detached heroes are outside the working set but still own their
            // bodies globally. Never mint another while that body is away.
            if existing.is_none()
                && self
                    .exits
                    .squire_owners
                    .get(&key)
                    .is_some_and(|seat| !self.players.contains_key(seat))
            {
                continue;
            }
            let eligible = |id: &u32, h: &Hero| {
                owner_key(h) == key && !h.absent && !h.away && !self.exits.outside.contains_key(id)
            };
            let seat = existing
                .as_ref()
                .and_then(|(id, _)| {
                    self.players
                        .get(id)
                        .filter(|h| owner_key(h) == key && !h.absent && !h.away)
                        .map(|_| *id)
                })
                .or_else(|| {
                    self.players
                        .iter()
                        .find(|(id, h)| eligible(id, h))
                        .map(|(&id, _)| id)
                })
                // With nobody present, keep the dormant body for reconnection.
                .or_else(|| {
                    existing.as_ref().and_then(|(id, _)| {
                        self.players
                            .get(id)
                            .filter(|h| owner_key(h) == key)
                            .map(|_| *id)
                    })
                });
            let Some(seat) = seat else { continue };
            let h = self.players.get_mut(&seat).unwrap();
            let mut s = existing.map(|(_, s)| s).unwrap_or_else(|| {
                let grid = Grid {
                    room: &self.dungeon.rooms[self.at],
                    barred,
                };
                let at = hireling::beside(&grid, (h.x, h.y));
                Companion::new(key, member.clone(), self.dungeon.depth, at, seat)
            });
            s.member = member;
            s.body.summoner = Some(seat);
            s.body.max_hp = s.member.kind.health(s.member.level);
            s.body.hp = s.body.hp.min(s.body.max_hp);
            h.squire = Some(s);
        }
    }

    pub(super) fn lodge_squires_follow(&mut self) {
        self.sync_squires();
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        for hero in self.players.values_mut() {
            if let Some(s) = &mut hero.squire {
                (s.body.x, s.body.y) = hireling::beside(&grid, (hero.x, hero.y));
                if s.floor != self.dungeon.depth {
                    s.body.hp = s.body.max_hp;
                } else if s.down() {
                    s.body.hp = (s.body.max_hp / 2).max(1);
                }
                s.floor = self.dungeon.depth;
                s.body.invulnerable = 20;
                s.mark = None;
                s.mark_left = 0;
                s.skull = None;
            }
        }
    }
    pub(super) fn tick_lodge_squires(&mut self) {
        self.sync_squires();
        if self.at_home_now() || matches!(self.phase, Phase::Wiped | Phase::Won) {
            return;
        }
        let seats: Vec<u32> = self.players.keys().copied().collect();
        for seat in seats {
            if self.players[&seat].absent
                || self.players[&seat].away
                || self.players[&seat].stone
                || self.exits.outside.contains_key(&seat)
            {
                continue;
            }
            let Some(mut s) = self.players.get_mut(&seat).and_then(|h| h.squire.take()) else {
                continue;
            };
            self.drive_lodge_squire(seat, &mut s);
            self.players.get_mut(&seat).unwrap().squire = Some(s);
        }
    }
    fn drive_lodge_squire(&mut self, seat: u32, s: &mut Companion) {
        s.body.invulnerable = s.body.invulnerable.saturating_sub(1);
        s.body.cooldown = s.body.cooldown.saturating_sub(1);
        s.vanished = s.vanished.saturating_sub(1);
        s.special = s.special.saturating_sub(1);
        s.support = s.support.saturating_sub(1);
        s.mark_left = s.mark_left.saturating_sub(1);
        if s.mark_left == 0 {
            s.mark = None;
        }
        if s.down() {
            if !self.fight_on() {
                s.body.hp = (s.body.max_hp / 2).max(1);
                s.body.invulnerable = HZ;
            }
            return;
        }
        let owner = &self.players[&seat];
        let owner_at = (owner.x, owner.y);
        let kind = s.kind();
        // Vanish evades shots and melee. A shield deliberately catches wider
        // shots; all other squires share the existing hireling's hit grace.
        if s.vanished == 0 {
            for shot in self
                .projectiles
                .iter_mut()
                .filter(|p| p.hostile && p.ttl > 0 && !p.traits.path.waiting())
            {
                if let Some(beam) = &shot.traits.path.beam {
                    if beam.burning(shot.traits.path.age)
                        && beam.crosses((shot.x, shot.y), (s.body.x, s.body.y))
                    {
                        s.take_shot(shot.damage);
                    }
                }
            }
            if self.enemies.iter().any(|e| {
                e.hp > 0
                    && e.age >= TELEGRAPH
                    && e.frozen == 0
                    && !e.asleep(self.tick)
                    && e.kind != EnemyKind::Dummy
                    && (e.x - s.body.x).hypot(e.y - s.body.y) < e.radius() + HERO_RADIUS
            }) {
                s.body.hurt(if kind == Kind::Shieldbearer { 4 } else { 9 });
            }
        }
        if s.down() {
            return;
        }
        if kind == Kind::Ninja
            && kind.unlocked(s.member.level, Ability::Vanish)
            && s.special == 0
            && s.body.hp * 2 < s.body.max_hp
        {
            s.vanished = 2 * HZ;
            s.special = 8 * HZ;
        }
        let target = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.hp > 0
                    && e.age >= TELEGRAPH
                    && !e.dormant
                    && e.kind != EnemyKind::Dummy
                    && (e.x - owner_at.0).hypot(e.y - owner_at.1) < 20.0
                    && self.squire_line((s.body.x, s.body.y), (e.x, e.y))
            })
            .min_by(|(_, a), (_, b)| {
                ((a.x - s.body.x).hypot(a.y - s.body.y))
                    .total_cmp(&(b.x - s.body.x).hypot(b.y - s.body.y))
            })
            .map(|(i, _)| i);
        let mut destination = owner_at;
        let mut keep = 2.5;
        match kind {
            Kind::Healer => {
                // A settled reachable skull is a finite errand, ahead of
                // repeat healing (including its cooldown). No revival here.
                let escort = kind
                    .unlocked(s.member.level, Ability::Escort)
                    .then(|| self.squire_skull_assist(seat, s))
                    .flatten();
                if let Some(at) = escort {
                    destination = at;
                    keep = 0.3;
                } else if let Some(at) = self
                    .lodge_squire_patient(s, 15.0)
                    .map(|id| (id, (self.players[&id].x, self.players[&id].y)))
                {
                    destination = at.1;
                    keep = 5.0;
                    if s.support == 0
                        && (s.body.x - at.1.0).hypot(s.body.y - at.1.1) <= 9.0
                        && self.squire_line((s.body.x, s.body.y), at.1)
                    {
                        let h = self.players.get_mut(&at.0).unwrap();
                        h.mend(12 + 3 * u32::from(s.member.level), stakes::Mend::Passive);
                        if kind.unlocked(s.member.level, Ability::Cleanse) {
                            h.hexed = 0;
                            h.chilled = 0;
                            h.mired = 0;
                        }
                        s.support = 2 * HZ;
                        self.cues.push("squire:heal".into());
                    }
                }
                if let Some(i) = target {
                    let e = &self.enemies[i];
                    if escort.is_none() && (s.body.x - e.x).hypot(s.body.y - e.y) < 5.0 {
                        destination = (s.body.x + (s.body.x - e.x), s.body.y + (s.body.y - e.y));
                        keep = 0.0;
                    }
                    if s.body.cooldown == 0 {
                        self.lodge_squire_shot(seat, s, i, Shot::Bolt, s.damage(), false);
                        s.body.cooldown = 2 * HZ;
                    }
                }
            }
            Kind::Alchemist => {
                if kind.unlocked(s.member.level, Ability::Potions)
                    && s.support == 0
                    && let Some(id) = self.lodge_squire_patient(s, 8.0)
                {
                    let h = self.players.get_mut(&id).unwrap();
                    h.mend(20 + 4 * u32::from(s.member.level), stakes::Mend::Passive);
                    s.support = 5 * HZ;
                    self.cues.push("squire:potion".into());
                }
                if let Some(i) = target {
                    let e = &self.enemies[i];
                    destination = (e.x, e.y);
                    keep = 7.0;
                    if s.body.cooldown == 0 {
                        self.lodge_squire_shot(
                            seat,
                            s,
                            i,
                            Shot::Ball,
                            s.damage(),
                            kind.unlocked(s.member.level, Ability::Splash),
                        );
                        s.body.cooldown = 3 * HZ;
                        self.cues.push("squire:bomb".into());
                    }
                }
            }
            Kind::Ranger => {
                if let Some(i) = target {
                    let e = &self.enemies[i];
                    destination = (e.x, e.y);
                    keep = 8.0;
                    if s.body.cooldown == 0 {
                        let marking = kind.unlocked(s.member.level, Ability::Mark);
                        let marked = marking && s.mark == Some(e.id) && s.mark_left > 0;
                        s.mark = marking.then_some(e.id);
                        s.mark_left = if marking { 5 * HZ } else { 0 };
                        self.lodge_squire_shot(
                            seat,
                            s,
                            i,
                            Shot::Arrow,
                            s.damage() * if marked { 3 } else { 2 } / 2,
                            false,
                        );
                        s.body.cooldown = HZ;
                        if marking {
                            self.cues.push("squire:mark".into());
                        }
                    }
                }
            }
            Kind::Shieldbearer => {
                if let Some(i) = target {
                    let e = &self.enemies[i];
                    let way = unit(e.x - owner_at.0, e.y - owner_at.1);
                    destination = (owner_at.0 + way.0 * 2.3, owner_at.1 + way.1 * 2.3);
                    keep = 0.2;
                    if (e.x - s.body.x).hypot(e.y - s.body.y) < 2.0 + e.radius() {
                        if s.body.cooldown == 0 {
                            self.squire_strike(seat, s, i, s.damage());
                            s.body.cooldown = HZ;
                        }
                        // Trained bash briefly holds and shoves ordinary foes;
                        // never push a boss through walls or chain-stun it.
                        if kind.unlocked(s.member.level, Ability::Bash)
                            && s.special == 0
                            && !self.enemies[i].is_boss()
                        {
                            self.enemies[i].frozen = self.enemies[i].frozen.max(HZ / 2);
                            s.special = 3 * HZ;
                            self.cues.push("squire:bash".into());
                            let grid = Grid {
                                room: &self.dungeon.rooms[self.at],
                                barred: self.barred(),
                            };
                            let e = &mut self.enemies[i];
                            let radius = e.radius();
                            (e.x, e.y) = grid.slide(
                                (e.x, e.y),
                                (way.0 * 0.12, way.1 * 0.12),
                                radius,
                                Mover::Walker,
                            );
                        }
                    }
                }
            }
            Kind::Ninja => {
                if let Some(i) = target {
                    let e = &self.enemies[i];
                    let toward_owner = unit(owner_at.0 - e.x, owner_at.1 - e.y);
                    destination = (e.x - toward_owner.0 * 1.2, e.y - toward_owner.1 * 1.2);
                    keep = 0.1;
                    let distance = (s.body.x - e.x).hypot(s.body.y - e.y);
                    if s.body.cooldown == 0 {
                        if distance < e.radius() + 2.0 {
                            let behind = (s.body.x - e.x) * toward_owner.0
                                + (s.body.y - e.y) * toward_owner.1
                                < 0.0;
                            let backstab = kind.unlocked(s.member.level, Ability::Backstab)
                                && (behind || s.vanished > 0);
                            self.squire_strike(
                                seat,
                                s,
                                i,
                                s.damage() * if backstab { 3 } else { 1 },
                            );
                            s.body.cooldown = HZ * 3 / 4;
                            if backstab {
                                self.cues.push("squire:backstab".into());
                            }
                        } else if kind.unlocked(s.member.level, Ability::Stars) {
                            self.lodge_squire_shot(seat, s, i, Shot::Blade, s.damage(), false);
                            s.body.cooldown = HZ;
                            self.cues.push("squire:star".into());
                        }
                    }
                }
            }
        }
        let at = (s.body.x, s.body.y);
        let distance = (destination.0 - at.0).hypot(destination.1 - at.1);
        let mut way = if distance > keep {
            unit(destination.0 - at.0, destination.1 - at.1)
        } else {
            (0.0, 0.0)
        };
        if matches!(kind, Kind::Ranger | Kind::Alchemist)
            && target.is_some()
            && distance < keep - 1.5
        {
            way = unit(at.0 - destination.0, at.1 - destination.1);
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let pace = HERO_SPEED
            * DT
            * if kind == Kind::Ninja {
                1.25
            } else if kind == Kind::Shieldbearer {
                0.8
            } else {
                0.95
            };
        (s.body.x, s.body.y) =
            grid.slide(at, (way.0 * pace, way.1 * pace), HERO_RADIUS, Mover::Hero);
        if way.0 != 0.0 {
            s.body.facing = way.0;
        }
        // A doorway/large floor can strand a follower; a safe catch-up never
        // teleports through a barred fight to engage distant monsters.
        if target.is_none() && (s.body.x - owner_at.0).hypot(s.body.y - owner_at.1) > 24.0 {
            (s.body.x, s.body.y) = hireling::beside(&grid, owner_at);
        }
        self.squire_deliver_skull(seat, s);
    }
    fn lodge_squire_patient(&self, s: &Companion, reach: f32) -> Option<u32> {
        self.players
            .iter()
            .filter(|(_, h)| {
                h.hp > 0
                    && !h.absent
                    && !h.away
                    && !h.stone
                    && (h.hp < h.max_hp || h.hexed > 0 || h.chilled > 0 || h.mired > 0)
                    && (h.x - s.body.x).hypot(h.y - s.body.y) <= reach
                    && self.squire_line((s.body.x, s.body.y), (h.x, h.y))
            })
            .min_by(|(ai, a), (bi, b)| {
                (u64::from(a.hp) * u64::from(b.max_hp))
                    .cmp(&(u64::from(b.hp) * u64::from(a.max_hp)))
                    .then(ai.cmp(bi))
            })
            .map(|(&id, _)| id)
    }
    /// Walk a settled skull back to a living owner. The skull stays in the
    /// ordinary ground ledger, so any real player can still pick it up.
    fn squire_skull_assist(&mut self, seat: u32, s: &mut Companion) -> Option<(f32, f32)> {
        let owner = &self.players[&seat];
        if owner.hp == 0 || owner.carrying_skull.is_some() {
            s.skull = None;
            return None;
        }
        if let Some(id) = s.skull {
            if self.skull_lying(id).is_none() {
                s.skull = None;
                return None;
            }
            return Some((owner.x, owner.y));
        }
        let found = self
            .players
            .iter()
            .filter(|(_, h)| h.hp == 0 && h.fell > 0 && self.tick >= h.fell + skulls::SKULL_SETTLE)
            .filter_map(|(&id, _)| self.skull_lying(id).map(|at| (id, at)))
            .filter(|(id, at)| {
                (at.0 - owner.x).hypot(at.1 - owner.y) < 15.0
                    && !self
                        .players
                        .values()
                        .filter_map(|h| h.squire.as_ref())
                        .any(|other| other.skull == Some(*id))
                    && self.squire_line((s.body.x, s.body.y), *at)
            })
            .min_by(|a, b| {
                (a.1.0 - s.body.x)
                    .hypot(a.1.1 - s.body.y)
                    .total_cmp(&(b.1.0 - s.body.x).hypot(b.1.1 - s.body.y))
            });
        if let Some((id, at)) = found {
            if (at.0 - s.body.x).hypot(at.1 - s.body.y) < skulls::SKULL_REACH {
                s.skull = Some(id);
            }
            Some(at)
        } else {
            None
        }
    }
    fn squire_deliver_skull(&mut self, seat: u32, s: &mut Companion) {
        let Some(id) = s.skull else {
            return;
        };
        if self.skull_lying(id).is_none() {
            s.skull = None;
            return;
        }
        let owner = &self.players[&seat];
        let near = (owner.x - s.body.x).hypot(owner.y - s.body.y) < skulls::SKULL_REACH;
        if near && owner.hp > 0 && owner.carrying_skull.is_none() {
            self.players.get_mut(&seat).unwrap().carrying_skull = Some(id);
            self.players.get_mut(&id).unwrap().skull_at = None;
            s.skull = None;
            self.cues.push("squire:skull".into());
        } else {
            self.players.get_mut(&id).unwrap().skull_at = Some((s.body.x, s.body.y));
        }
    }
    fn squire_line(&self, from: (f32, f32), to: (f32, f32)) -> bool {
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let steps = ((to.0 - from.0).hypot(to.1 - from.1) * 3.0).ceil().max(1.0) as usize;
        (1..=steps).all(|i| {
            let t = i as f32 / steps as f32;
            grid.clear(
                from.0 + (to.0 - from.0) * t,
                from.1 + (to.1 - from.1) * t,
                0.05,
                Mover::Hero,
            )
        })
    }
    fn squire_strike(&mut self, seat: u32, _s: &Companion, index: usize, damage: u32) {
        let e = &mut self.enemies[index];
        let dealt = damage.min(e.hp);
        e.hp -= dealt;
        e.wits.foe = seat;
        self.feel.credit(seat, e.id, dealt, self.tick);
        self.sounds.push("swing");
    }
    fn lodge_squire_shot(
        &mut self,
        seat: u32,
        s: &Companion,
        index: usize,
        kind: Shot,
        damage: u32,
        bomb: bool,
    ) {
        if self.projectiles.len() >= MAX_PROJECTILES {
            return;
        }
        let e = &self.enemies[index];
        let way = unit(e.x - s.body.x, e.y - s.body.y);
        self.projectiles.push(Projectile {
            x: s.body.x,
            y: s.body.y,
            vx: way.0 * 16.0,
            vy: way.1 * 16.0,
            hostile: false,
            look: None,
            kind,
            damage,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: ShotTraits {
                owner: seat as u8,
                burst: if bomb { 100 } else { 0 },
                ..Default::default()
            },
            ttl: 2 * HZ,
        });
    }
}

#[cfg(test)]
#[path = "../../tests/rules/squires_lodge__tests.rs"]
mod tests;
