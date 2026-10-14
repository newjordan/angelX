//! Raids: when a holding's walls run low, raiders take it. They stop its
//! rent and sack a room every rent day until the party walks into its hall
//! (a floor's steward's hall below, a castle's hall in the kingdom) and
//! fights them off, every seated knight at once. The holding fights with
//! them, bay by bay:
//!
//! - its trade rooms buy off a raider each before the fight;
//! - its forge arms the party (mail from the smithy and armory, a bomb
//!   from the fletcher);
//! - its chapel heals, a little every few seconds for each chapel room;
//! - its tavern brawls: each tavern room hurls a mutton bone at the
//!   nearest raider; an archery range looses an arrow.
//!
//! Won, the cockpit frees the holding, mends its walls and pays the
//! raiders' purse into the treasury. A floor's wipe is a wipe; in a
//! castle's hall at home the party is beaten, not killed: the raiders sack
//! a room and stay.

use super::holdings::{self, Deed, Family, Holding, SLOTS};
use super::*;

/// A raid being fought: whose hall, which room of the floor, how strong.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaidFight {
    pub key: String,
    pub room: usize,
    pub level: u8,
    pub started: u64,
}

impl RaidFight {
    pub fn valid(&self, rooms: usize) -> bool {
        self.room < rooms && self.key.len() <= 64 && self.level <= holdings::RAID_MAX
    }
}

/// Where raiders come in (tiles): the open middle of a hall, clear of its
/// bays and its table.
pub const RAID_SPOTS: [(i32, i32); 8] = [(4, 6), (19, 6), (4, 8), (19, 8), (7, 7), (16, 7), (9, 9), (14, 9)];
/// A chapel's mend every so often, a tavern's thrown bone and a range's arrow:
/// how often, how hard, how far they reach (units).
pub const CHAPEL: (u64, u32) = (90, 4);
pub const STOOL: (u64, u32, f32) = (60, 18, 14.0);
pub const ARROW: (u64, u32, f32) = (45, 12, 20.0);
/// A castle's bays hang as a tapestry in its hall: its top-left corner
/// (native pixels), and each bay's pitch.
pub const TAPESTRY: (i32, i32) = (35, 113);
pub const TAPESTRY_PITCH: i32 = 30;

/// The holding whose hall the party stands in, by its key.
pub fn hall_key(run: &Run) -> Option<String> {
    match run.room().kind {
        RoomKind::Holding => holdings::held_floor(&run.home.holdings, run.dungeon.pack).map(|h| h.key.clone()),
        RoomKind::RealmHall => holdings::castle_here(run).map(|i| Deed::Castle(i).key()),
        _ => None,
    }
}

/// Bay `i`'s front, where its defence comes from (units).
pub fn bay_spot(run: &Run, i: usize) -> (f32, f32) {
    if run.room().kind == RoomKind::Holding {
        let (c, r, w, h) = holdings::bay(i).1;
        ((c as f32 + w as f32 / 2.0) * TILE_UNITS, (r as f32 + h as f32 / 2.0) * TILE_UNITS)
    } else {
        let px = TAPESTRY.0 + 14 + i as i32 * TAPESTRY_PITCH + 14;
        (px as f32 / 8.0, (TAPESTRY.1 + 24) as f32 / 8.0)
    }
}

pub fn won_mark(key: &str) -> String {
    format!("holdings:raid:won:{key}")
}

pub fn lost_mark(key: &str) -> String {
    format!("holdings:raid:lost:{key}")
}

/// The standing rooms of `family` at a holding, by bay.
fn standing(held: &Holding, family: Family) -> Vec<(usize, &'static holdings::RoomDef)> {
    held.rooms
        .iter()
        .enumerate()
        .take(SLOTS)
        .filter(|(_, r)| r.standing())
        .filter_map(|(i, r)| holdings::room_def(&r.id).map(|d| (i, d)))
        .filter(|(_, d)| d.family == family)
        .collect()
}

impl Run {
    /// The raid in the hall the party stands in: begun on the way in, its
    /// bays' defences while it lasts, freed when the last raider falls.
    pub(super) fn tick_raid(&mut self) {
        if let Some(fight) = &self.holding_raid
            && fight.room != self.at
        {
            self.holding_raid = None;
        }
        let Some(key) = hall_key(self) else {
            return;
        };
        let Some(held) = self.home.holdings.holding(&key).cloned() else {
            return;
        };
        match self.holding_raid.clone() {
            None if held.raid > 0
                && self.phase != Phase::Fighting
                && !self.marks.contains_key(&won_mark(&key))
                && !self.marks.contains_key(&lost_mark(&key)) =>
            {
                self.start_raid(&key, &held)
            }
            Some(fight) if !self.foes_left() => {
                *self.marks.entry(won_mark(&fight.key)).or_default() += 1;
                self.holding_raid = None;
                self.cues.push("raid_won".into());
                self.sounds.push("wheel_land");
                self.fx.push(Fx::Shake(12));
                self.found = Some((self.tick, 0, "RAIDERS ROUTED".into()));
            }
            Some(fight) => self.defend(&held, &fight),
            None => {}
        }
    }

    fn start_raid(&mut self, key: &str, held: &Holding) {
        let bought = standing(held, Family::Trade).len() as u32;
        let n = super::crown::raiders(held.raid, bought);
        let (pack, depth) = if self.room().kind == RoomKind::Holding {
            (self.dungeon.pack, self.dungeon.depth)
        } else {
            (Pack::Cavern, 2)
        };
        let roster: Vec<EnemyKind> = pack
            .roster_at(depth)
            .iter()
            .map(|&(k, _)| k)
            .filter(|k| !matches!(k, EnemyKind::Slime | EnemyKind::Mimic | EnemyKind::Dragon | EnemyKind::Boss))
            .collect();
        if roster.is_empty() {
            return;
        }
        for (k, &(c, r)) in RAID_SPOTS.iter().enumerate().take(n as usize) {
            let kind = roster[k % roster.len()];
            self.spawn_body(kind, (c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
        }
        // The forge arms the party.
        for (_, def) in standing(held, Family::Forge) {
            for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                match def.id {
                    "fletcher" => hero.bombs += 1,
                    "armory" => hero.armor += 2,
                    _ => hero.armor += 1,
                }
            }
        }
        self.dungeon.rooms[self.at].cleared = false;
        self.phase = Phase::Fighting;
        self.window = None;
        self.holding_raid = Some(RaidFight { key: key.to_string(), room: self.at, level: held.raid, started: self.tick });
        self.sounds.push("door_seal");
        self.cues.push("raid".into());
        self.fx.push(Fx::Shake(10));
        self.found = Some((self.tick, 0, "RAIDERS! HOLD THE HALL".into()));
        if bought > 0 {
            self.cues.push("raid_bribed".into());
        }
    }

    /// The bays fight: the chapel mends, the tavern hurls bones, the range
    /// looses arrows.
    fn defend(&mut self, held: &Holding, fight: &RaidFight) {
        let t = self.tick.saturating_sub(fight.started);
        if t > 0 && t.is_multiple_of(CHAPEL.0) {
            let n = standing(held, Family::Chapel).len() as u32;
            if n > 0 {
                for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                    hero.mend(CHAPEL.1 * n, stakes::Mend::Party);
                }
                self.sounds.push("holy_water");
            }
        }
        let mut shots = Vec::new();
        if t > 0 && t.is_multiple_of(STOOL.0) {
            for (i, _) in standing(held, Family::Tavern) {
                shots.push((i, Shot::Bone, STOOL.1, STOOL.2));
            }
        }
        if t > 0 && t.is_multiple_of(ARROW.0) {
            for (i, def) in standing(held, Family::Barracks) {
                if def.id == "range" {
                    shots.push((i, Shot::Arrow, ARROW.1, ARROW.2));
                }
            }
        }
        for (i, kind, damage, reach) in shots {
            let (x, y) = bay_spot(self, i);
            let Some((tx, ty)) = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0 && (e.x - x).hypot(e.y - y) <= reach)
                .min_by(|a, b| (a.x - x).hypot(a.y - y).total_cmp(&(b.x - x).hypot(b.y - y)))
                .map(|e| (e.x, e.y))
            else {
                continue;
            };
            if self.projectiles.len() >= MAX_PROJECTILES {
                break;
            }
            let (ux, uy) = unit(tx - x, ty - y);
            self.projectiles.push(Projectile {
                x,
                y,
                vx: ux * 12.0,
                vy: uy * 12.0,
                hostile: false,
                look: None,
                kind,
                damage,
                pierce: 0,
                last_hit: None,
                empowered: false,
                traits: Default::default(),
                ttl: 2 * HZ,
            });
            self.sounds.push("swing");
        }
    }

    /// Beaten in a castle's hall at home: the knights are knocked down, not
    /// killed. The raiders sack a room and stay; the party gets up sore.
    /// True if it was this (and not a wipe).
    pub(super) fn raid_rout(&mut self) -> bool {
        let Some(fight) = self.holding_raid.clone() else {
            return false;
        };
        if !self.at_home_now()
            || !(self.players.values().all(|h| h.hp == 0 || h.stone) && self.players.values().any(|h| h.hp == 0))
        {
            return false;
        }
        *self.marks.entry(lost_mark(&fight.key)).or_default() += 1;
        self.holding_raid = None;
        self.enemies.clear();
        self.projectiles.clear();
        for hero in self.players.values_mut() {
            if hero.hp == 0 {
                hero.hp = (hero.max_hp / 4).max(1);
            }
        }
        self.dungeon.rooms[self.at].cleared = true;
        self.phase = Phase::Exploring;
        self.cues.push("raid_lost".into());
        self.sounds.push("door_open");
        self.found = Some((self.tick, 0, "THE RAIDERS HOLD IT".into()));
        true
    }
}
