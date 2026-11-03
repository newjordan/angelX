//! The company's squires: a shared roster, with one companion for each knight.
use super::*;

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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squire {
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub level: u8,
    pub drills: u32,
}
impl Squire {
    pub fn health(&self) -> u32 {
        48 + u32::from(self.level) * 12
            + if self.kind == Kind::Shieldbearer {
                40
            } else {
                0
            }
    }
    pub fn power(&self) -> u32 {
        6 + u32::from(self.level) * 3
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Company {
    pub day: u32,
    pub revision: BTreeMap<String, u32>,
    pub hired: Vec<usize>,
    pub board: u8,
    pub hall: u8,
    pub yard: u8,
    pub roster: Vec<Squire>,
    pub selected: BTreeMap<String, u32>,
    pub next: u32,
}
impl Company {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
    pub fn cap(&self) -> usize {
        3 + usize::from(self.hall.min(3)) * 2
    }
    pub fn offers(&self) -> Vec<Squire> {
        const NAMES: [&str; 10] = [
            "Pip", "Moss", "Wren", "Ash", "Nell", "Kit", "Bram", "Fern", "Rowan", "Flint",
        ];
        (0..3 + usize::from(self.board.min(3)))
            .map(|i| {
                let n = (u64::from(self.day) * 3 + i as u64) as usize;
                Squire {
                    id: i as u32,
                    name: NAMES[n % NAMES.len()].into(),
                    kind: Kind::ALL[n % if self.board == 0 { 3 } else { 5 }],
                    level: 1 + self.board.min(3) / 2,
                    drills: 0,
                }
            })
            .collect()
    }
    pub fn wage(&self, recruit: &Squire) -> u32 {
        (60 + u32::from(recruit.level) * 20).saturating_sub(u32::from(self.hall.min(3)) * 10)
    }
    pub fn hire(&mut self, offer: usize, gold: &mut Spoils) -> Result<String, String> {
        if self.roster.len() >= self.cap() {
            return Err("Company full. Improve hiring hall.".into());
        }
        if self.hired.contains(&offer) {
            return Err("Recruit already hired. Return next delve.".into());
        }
        let mut recruit = self.offers().get(offer).cloned().ok_or("Recruit gone.")?;
        let wage = self.wage(&recruit);
        if self.next == u32::MAX {
            return Err("Company register full.".into());
        }
        pay(gold, wage)?;
        self.next += 1;
        recruit.id = self.next;
        let name = recruit.name.clone();
        self.roster.push(recruit);
        self.hired.push(offer);
        Ok(format!("{name} joined the company."))
    }
    pub fn upgrade(&mut self, building: usize, gold: &mut Spoils) -> Result<String, String> {
        let level = match building {
            0 => &mut self.board,
            1 => &mut self.hall,
            2 => &mut self.yard,
            _ => return Err("No building.".into()),
        };
        if *level >= 3 {
            return Err("Fully built.".into());
        }
        pay(gold, 120 * (u32::from(*level) + 1))?;
        *level += 1;
        Ok("Squire yard improved.".into())
    }
}
fn pay(gold: &mut Spoils, price: u32) -> Result<(), String> {
    let mut cost = Spoils::default();
    cost.add(Spoil::Gold, price);
    if !gold.covers(&cost) {
        return Err(format!("Need {price} treasury gold."));
    }
    gold.take(&cost);
    Ok(())
}

/// A yard plate is chosen by walking onto it; E confirms, as at Ector's lecterns.
// The yard's other squire companies use rows 3, 4, 5 and 9–12.
// Keep these independent orders on their own row so one hold cannot buy twice.
pub const PLATES: [(f32, f32); 11] = [
    (2.0, 1.5), (4.0, 1.5), (6.0, 1.5), (8.0, 1.5), (10.0, 1.5),
    (12.0, 1.5), (14.0, 1.5), (16.0, 1.5), (18.0, 1.5), (20.0, 1.5),
    (22.0, 1.5),
];
pub fn plate(x: f32, y: f32) -> Option<usize> {
    PLATES.iter().position(|&(px, py)| {
        (x / TILE_UNITS - px).abs() < 1.0 && (y / TILE_UNITS - py).abs() < 0.65
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Companion {
    pub owner: u32,
    pub recruit: Squire,
    pub body: hireling::Hireling,
    #[serde(default)]
    pub room: usize,
    #[serde(default)]
    pub depth: u32,
    #[serde(default)]
    pub ability: u32,
    #[serde(default)]
    pub smoke: u32,
    #[serde(default)]
    pub trick: u32,
    #[serde(default)]
    pub marked: Option<u32>,
}

impl Run {
    pub(super) fn tick_countryside_squire_yard(&mut self, inputs: &BTreeMap<u32, Input>) {
        let company = &self.home.countryside.squires;
        for (&id, hero) in &mut self.players {
            let pressing = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            let Some(at) = plate(hero.x, hero.y)
                .filter(|_| pressing && hero.hp > 0 && !hero.stone && hero.privy == 0)
            else {
                self.countryside_squire_holds.remove(&id);
                continue;
            };
            let (previous, hold) = self.countryside_squire_holds.entry(id).or_insert((at, 0));
            if *previous != at {
                *previous = at;
                *hold = 0;
            }
            *hold = hold.saturating_add(1).min(home::BUY_HOLD + 1);
            if *hold != home::BUY_HOLD {
                continue;
            }
            let key = home::Home::knight_key(hero);
            // Names are hex encoded so punctuation cannot become an action delimiter.
            let key: String = key.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
            let version = company.revision.get(&key).copied().unwrap_or(0);
            let action = match at {
                0..=5 => format!("hire:{at}"),
                6 => "select:0".into(),
                7 => "train:0".into(),
                8..=10 => format!("upgrade:{}", at - 8),
                _ => unreachable!(),
            };
            self.marks
                .insert(format!("country:squire:{}:{key}:{action}", version), id);
        }
    }
}

pub fn settle(
    company: &mut Company,
    treasury: &mut Spoils,
    marks: &BTreeMap<String, u32>,
    news: &mut Vec<String>,
) {
    let day_mark = format!("country:squire-day:{}", company.day);
    if marks.contains_key(&day_mark) {
        company.day = company.day.saturating_add(1);
        company.hired.clear();
        for squire in &mut company.roster {
            if company.selected.values().any(|id| *id == squire.id) {
                squire.drills = squire.drills.saturating_add(1);
            }
        }
    }
    for mark in marks.keys().filter(|s| s.starts_with("country:squire:")) {
        let fields: Vec<_> = mark.split(':').collect();
        if !intent_key(mark) || fields.len() != 6 {
            continue;
        }
        let Ok(version) = fields[2].parse::<u32>() else {
            continue;
        };
        let raw = fields[3];
        if version != company.revision.get(raw).copied().unwrap_or(0) {
            continue;
        }
        if raw.len() > 512 || raw.len() % 2 != 0 {
            continue;
        }
        let Some(bytes) = (0..raw.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).ok())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Ok(knight) = String::from_utf8(bytes) else {
            continue;
        };
        let Ok(index) = fields[5].parse::<usize>() else {
            continue;
        };
        let result = match fields[4] {
            "hire" => company.hire(index, treasury),
            "upgrade" => company.upgrade(index, treasury),
            "select" => company.cycle(&knight),
            "train" => company.train(&knight, treasury),
            _ => continue,
        };
        company
            .revision
            .insert(raw.into(), version.saturating_add(1));
        news.push(result.unwrap_or_else(|e| e));
    }
}

impl Company {
    pub fn active(&self, knight: &str) -> Option<&Squire> {
        let id = self.selected.get(knight)?;
        self.roster.iter().find(|s| s.id == *id)
    }
    pub fn cycle(&mut self, knight: &str) -> Result<String, String> {
        let available: Vec<_> = self
            .roster
            .iter()
            .filter(|s| {
                !self
                    .selected
                    .iter()
                    .any(|(k, id)| k != knight && *id == s.id)
            })
            .collect();
        if available.is_empty() {
            return Err("Hire a free squire first.".into());
        }
        let next = match self
            .selected
            .get(knight)
            .and_then(|id| available.iter().position(|s| s.id == *id))
        {
            Some(n) if n + 1 == available.len() => {
                self.selected.remove(knight);
                return Ok("Squire resting.".into());
            }
            Some(n) => available[n + 1],
            None => available[0],
        };
        self.selected.insert(knight.into(), next.id);
        Ok(format!("{} will follow you.", next.name))
    }
    pub fn train(&mut self, knight: &str, gold: &mut Spoils) -> Result<String, String> {
        let id = *self.selected.get(knight).ok_or("Select a squire first.")?;
        let squire = self
            .roster
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or("Squire missing.")?;
        if squire.level >= 2 + self.yard.min(3) * 2 {
            return Err("Improve training yard.".into());
        }
        if squire.drills < u32::from(squire.level).saturating_sub(u32::from(self.yard)) {
            return Err("Needs a delve's practice.".into());
        }
        pay(gold, 35 * u32::from(squire.level))?;
        squire.drills = 0;
        squire.level += 1;
        Ok(format!("{} trained to {}.", squire.name, squire.level))
    }
}

impl Run {
    pub(super) fn tick_countryside_squires(&mut self) {
        if self.dungeon.depth == 0 || self.deathmatch.is_some() {
            self.countryside_squires.clear();
            return;
        }
        let mut companions = std::mem::take(&mut self.countryside_squires);
        for (&owner, hero) in &self.players {
            if hero.absent || hero.stone || companions.iter().any(|c| c.owner == owner) {
                continue;
            }
            let Some(recruit) = self
                .home
                .countryside
                .squires
                .active(&home::Home::knight_key(hero))
                .cloned()
            else {
                continue;
            };
            let kind = match recruit.kind {
                Kind::Ninja => hireling::AllyKind::Hound,
                Kind::Healer => hireling::AllyKind::Linnet,
                Kind::Ranger | Kind::Alchemist => hireling::AllyKind::Beaumains,
                Kind::Shieldbearer => hireling::AllyKind::Warden,
            };
            let mut body = hireling::Hireling::minion(
                kind,
                0,
                self.dungeon.depth,
                (hero.x - 1.0, hero.y),
                owner,
            );
            body.hp = recruit.health();
            body.max_hp = body.hp;
            companions.push(Companion {
                owner,
                recruit,
                body,
                room: self.at,
                depth: self.dungeon.depth,
                ability: 0,
                smoke: 0,
                trick: 0,
                marked: None,
            });
        }
        companions.retain(|c| self.players.get(&c.owner).is_some_and(|h| !h.absent));
        let marked = companions.iter().find_map(|c| c.marked);
        for c in &mut companions {
            let Some(hero) = self.players.get(&c.owner) else {
                continue;
            };
            if c.room != self.at || c.depth != self.dungeon.depth {
                let grid = Grid {
                    room: &self.dungeon.rooms[self.at],
                    barred: self.barred(),
                };
                (c.body.x, c.body.y) = hireling::beside(&grid, (hero.x, hero.y));
                c.room = self.at;
                c.depth = self.dungeon.depth;
                if c.body.down() {
                    c.body.hp = c.body.max_hp / 2;
                }
                c.body.invulnerable = HZ;
            }
            c.ability = c.ability.saturating_sub(1);
            c.smoke = c.smoke.saturating_sub(1);
            c.trick = c.trick.saturating_sub(1);
            // Reuse the allies' movement, damage and downed recovery, not their attacks.
            c.body.cooldown = 2;
            self.drive_hireling(&mut c.body);
            if c.recruit.kind == Kind::Healer && c.recruit.level >= 3 && !c.body.down() {
                let skull = self
                    .players
                    .keys()
                    .copied()
                    .filter_map(|id| self.skull_lying(id))
                    .min_by(|a, b| {
                        ((a.0 - c.body.x).hypot(a.1 - c.body.y))
                            .total_cmp(&(b.0 - c.body.x).hypot(b.1 - c.body.y))
                    });
                if let Some((x, y)) = skull {
                    let (ux, uy) = unit(x - c.body.x, y - c.body.y);
                    let grid = Grid {
                        room: &self.dungeon.rooms[self.at],
                        barred: self.barred(),
                    };
                    (c.body.x, c.body.y) = grid.slide(
                        (c.body.x, c.body.y),
                        (ux * HERO_SPEED * DT, uy * HERO_SPEED * DT),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                }
            }
            if c.body.down() || c.ability > 0 {
                continue;
            }
            self.countryside_squire_action(c, marked);
        }
        self.countryside_squires = companions;
    }

    fn countryside_squire_action(&mut self, c: &mut Companion, marked: Option<u32>) {
        let at = (c.body.x, c.body.y);
        let level = c.recruit.level;
        let power = c.recruit.power();
        if c.recruit.kind == Kind::Healer {
            let target = self
                .players
                .iter()
                .filter(|(_, h)| {
                    h.hp > 0
                        && !h.absent
                        && (h.hp < h.max_hp || (level >= 2 && (h.hexed > 0 || h.chilled > 0)))
                        && (h.x - at.0).hypot(h.y - at.1) < 12.0
                })
                .min_by_key(|(_, h)| u64::from(h.hp) * 10000 / u64::from(h.max_hp.max(1)))
                .map(|(&id, _)| id);
            if let Some(id) = target {
                let h = self.players.get_mut(&id).unwrap();
                h.mend(power, super::stakes::Mend::Party);
                if level >= 2 {
                    h.hexed = 0;
                    h.chilled = 0;
                }
                self.cues.push("linnet_mend".into());
                c.ability = 2 * HZ;
            } else if level >= 3 {
                let skull = self.players.keys().copied().find(|&id| {
                    self.skull_lying(id)
                        .is_some_and(|(x, y)| (x - at.0).hypot(y - at.1) < 3.0)
                });
                if let Some(id) = skull {
                    let (x, y) = self.skull_lying(id).unwrap();
                    let h = self.players.get_mut(&id).unwrap();
                    h.hp = (h.max_hp / 4).max(1);
                    h.x = x;
                    h.y = y;
                    h.skull_at = None;
                    h.invulnerable = 2 * HZ;
                    self.cues.push("linnet_mend".into());
                    c.ability = 10 * HZ;
                }
            }
            return;
        }
        let target = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy && !hireling::great(e)
            })
            .filter(|(_, e)| (e.x - at.0).hypot(e.y - at.1) < 16.0)
            .min_by(|(_, a), (_, b)| {
                u8::from(marked != Some(a.id))
                    .cmp(&u8::from(marked != Some(b.id)))
                    .then_with(|| {
                        ((a.x - at.0).hypot(a.y - at.1)).total_cmp(&(b.x - at.0).hypot(b.y - at.1))
                    })
            })
            .map(|(i, _)| i);
        let Some(i) = target else { return };
        let (x, y) = (self.enemies[i].x, self.enemies[i].y);
        let d = (x - at.0).hypot(y - at.1);
        match c.recruit.kind {
            Kind::Ninja if d < 2.8 => {
                let back = self.enemies[i].wits.foe != 0 && self.enemies[i].wits.foe != c.owner;
                self.countryside_squire_hit(c.owner, i, power * if back { 3 } else { 1 });
                if level >= 2 && c.trick == 0 {
                    c.body.invulnerable = HZ;
                    c.smoke = HZ;
                    c.trick = 6 * HZ;
                }
                c.ability = HZ / 2;
                self.sounds.push("swing");
            }
            Kind::Shieldbearer if d < 3.4 => {
                self.countryside_squire_hit(c.owner, i, power);
                if level >= 2 {
                    self.enemies[i].frozen = self.enemies[i].frozen.max(HZ / 3);
                }
                c.body.taunt = 2 * HZ;
                c.ability = HZ;
                self.sounds.push("swing");
            }
            Kind::Alchemist => {
                let targets: Vec<_> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| {
                        e.hp > 0 && !hireling::great(e) && (e.x - x).hypot(e.y - y) < 2.4
                    })
                    .map(|(i, _)| i)
                    .collect();
                for index in targets {
                    self.countryside_squire_hit(c.owner, index, power);
                }
                if self.blasts.len() < 16 {
                    self.blasts.push((x, y, BLAST_TICKS));
                }
                self.sounds.push("bomb");
                if level >= 2 {
                    for h in self
                        .players
                        .values_mut()
                        .filter(|h| h.hp > 0 && (h.x - at.0).hypot(h.y - at.1) < 5.0)
                    {
                        h.mend(power / 2, super::stakes::Mend::Party);
                    }
                }
                c.ability = 3 * HZ;
            }
            Kind::Ranger | Kind::Ninja if self.projectiles.len() < MAX_PROJECTILES => {
                let (ux, uy) = unit(x - at.0, y - at.1);
                self.projectiles.push(Projectile {
                    x: at.0,
                    y: at.1,
                    vx: ux * 15.0,
                    vy: uy * 15.0,
                    hostile: false,
                    kind: if c.recruit.kind == Kind::Ranger {
                        Shot::Arrow
                    } else {
                        Shot::Blade
                    },
                    look: None,
                    damage: power,
                    pierce: u8::from(level >= 3),
                    last_hit: None,
                    ttl: HZ,
                    empowered: false,
                    traits: ShotTraits {
                        owner: c.owner as u8,
                        ..Default::default()
                    },
                });
                if level >= 2 && c.recruit.kind == Kind::Ranger {
                    c.marked = Some(self.enemies[i].id);
                }
                self.sounds.push("swing");
                c.ability = HZ;
            }
            _ => {}
        }
    }
    fn countryside_squire_hit(&mut self, owner: u32, index: usize, damage: u32) {
        let e = &mut self.enemies[index];
        let dealt = damage.min(e.hp);
        e.hp -= dealt;
        e.wits.foe = owner;
        self.feel.credit(owner, e.id, dealt, self.tick);
    }
}

/// The country journal's room for the squire company's own intents (a
/// day's muster and a hire, upgrade, selection or lesson at a plate). They
/// stand only while a company holds them.
pub const INTENT_SLOTS: usize = 7;

pub fn intent_key(key: &str) -> bool {
    if let Some(day) = key.strip_prefix("country:squire-day:") {
        return day.parse::<u32>().is_ok();
    }
    let f: Vec<_> = key.split(':').collect();
    f.len() == 6
        && f[0] == "country"
        && f[1] == "squire"
        && f[2].parse::<u32>().is_ok()
        && !f[3].is_empty()
        && f[3].len() <= 512
        && f[3].len() % 2 == 0
        && f[3].bytes().all(|b| b.is_ascii_hexdigit())
        && match f[4] {
            "hire" => f[5].parse::<usize>().is_ok_and(|i| i < 6),
            "upgrade" => f[5].parse::<usize>().is_ok_and(|i| i < 3),
            "select" | "train" => f[5] == "0",
            _ => false,
        }
}

#[cfg(test)]
#[path = "../../tests/rules/squires__tests.rs"]
mod tests;

pub(super) fn decoys(squires: &[Companion]) -> impl Iterator<Item = (f32, f32)> + '_ {
    squires
        .iter()
        .filter(|c| !c.body.down() && c.smoke == 0)
        .map(|c| (c.body.x, c.body.y))
}
pub(super) fn roared(squires: &[Companion], enemy: &Enemy, seen: &mut Vec<(f32, f32)>) {
    if hireling::great(enemy) {
        return;
    }
    if let Some(c) = squires
        .iter()
        .filter(|c| c.recruit.kind == Kind::Shieldbearer && c.body.hp > 0 && c.body.taunt > 0)
        .filter(|c| {
            (c.body.x - enemy.x).hypot(c.body.y - enemy.y) < hireling::ROAR_REACH + enemy.radius()
        })
        .min_by(|a, b| {
            ((a.body.x - enemy.x).hypot(a.body.y - enemy.y))
                .total_cmp(&(b.body.x - enemy.x).hypot(b.body.y - enemy.y))
        })
    {
        seen.clear();
        seen.push((c.body.x, c.body.y));
    }
}

impl Company {
    pub fn valid(&self) -> bool {
        self.board <= 3
            && self.hall <= 3
            && self.yard <= 3
            && self.roster.len() <= self.cap()
            && self.roster.iter().all(|s| {
                s.id > 0
                    && s.id <= self.next
                    && !s.name.is_empty()
                    && s.name.len() <= 64
                    && (1..=8).contains(&s.level)
            })
            && self
                .roster
                .iter()
                .enumerate()
                .all(|(i, s)| self.roster[..i].iter().all(|other| other.id != s.id))
            && self.hired.len() <= 6
            && self.hired.iter().all(|i| *i < 6)
            && self.selected.len() <= 128
            && self.revision.len() <= 128
            && self.selected.keys().all(|k| k.len() <= 256)
            && self
                .revision
                .keys()
                .all(|k| k.len() <= 512 && k.bytes().all(|b| b.is_ascii_hexdigit()))
            && self
                .selected
                .values()
                .all(|id| self.roster.iter().any(|s| s.id == *id))
            && self
                .selected
                .values()
                .enumerate()
                .all(|(i, id)| self.selected.values().take(i).all(|other| other != id))
    }
}
pub fn valid(run: &Run) -> bool {
    run.countryside_squires.len() <= MAX_PLAYERS as usize
        && run.countryside_squires.iter().enumerate().all(|(i, c)| {
            (1..=MAX_PLAYERS).contains(&c.owner)
                && run.players.contains_key(&c.owner)
                && run.countryside_squires[..i]
                    .iter()
                    .all(|other| other.owner != c.owner && other.recruit.id != c.recruit.id)
                && c.body.x.is_finite()
                && c.body.y.is_finite()
                && c.body.facing.is_finite()
                && c.body.side.is_finite()
                && c.body.max_hp == c.recruit.health()
                && c.body.hp <= c.body.max_hp
                && (1..=8).contains(&c.recruit.level)
                && c.recruit.name.len() <= 64
                && c.room < run.dungeon.rooms.len()
                && c.depth <= DEEPEST
        })
}

impl Run {
    pub(super) fn countryside_squires_follow(&mut self) {
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        for c in &mut self.countryside_squires {
            if let Some(h) = self.players.get(&c.owner) {
                (c.body.x, c.body.y) = hireling::beside(&grid, (h.x, h.y));
                c.room = self.at;
                c.depth = self.dungeon.depth;
                if c.body.down() {
                    c.body.hp = c.body.max_hp / 2;
                }
                c.body.invulnerable = HZ;
            }
        }
    }
}
