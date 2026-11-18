//! The Barony: King Brannoc Onehorn of Caer Dwfn, and the underground he
//! takes back with the realm's help.
//!
//! Caer Dwfn was a dwarf-kingdom. Its halls ran down from the mine-head
//! into the Mines and below, and every hall had a forge. A hundred years
//! ago the goblin clans came up from below, Cinderjaw took the first fire,
//! the forges went cold one by one and the gate-hall at the mine-head fell
//! in. Brannoc, the last of its kings, comes up the Mines' shaft once the
//! party has cleared a floor of the Mines, and camps by the ruin.
//!
//! He rules the reclaimed underground as a baron of the realm:
//! - **missions** he gives the knights reclaim his halls a piece at a time;
//! - **works** the realm pays for (and pays a lot for) are built by his
//!   dwarves over realm time, never at once: the King's Hall, his
//!   gate-hall at the mine-head rebuilt, and each hall's forge below;
//! - **forges**, once rebuilt, are relit with fire carried up from Dragon
//!   Keep, and each relit forge has its own output;
//! - **tribute** comes back to the treasury from the halls he holds, every
//!   floor the party clears; the **ledger** keeps what went in and what came
//!   back.
//!
//! The run only marks (`barony:*` and `clear:<pack>` marks, see
//! `Run::marks`); the cockpit pays, works and saves through `settle`, a pure
//! function of the realm's state, so the rules are tested without a
//! cockpit. Every number here is a fixed one, picked from the haul probe in
//! the balance tests (see docs/BARONY.md).

use super::home::{BUY_HOLD, Home};
use super::*;
use crate::realm::{Spoil, Spoils};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Court {
    #[default]
    Absent,
    Camped,
    Met,
    Sworn,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Work {
    pub id: String,
    #[serde(default)]
    pub labour: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub what: String,
    #[serde(default)]
    pub spoils: Spoils,
    #[serde(default)]
    pub paid_in: bool,
}

pub const LEDGER_LINES: usize = 8;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub paid_in: Spoils,
    #[serde(default)]
    pub paid_back: Spoils,
    #[serde(default)]
    pub lines: Vec<Entry>,
}

impl Ledger {
    pub(crate) fn write(&mut self, what: String, spoils: &Spoils, paid_in: bool) {
        if paid_in {
            self.paid_in.merge(spoils);
        } else {
            self.paid_back.merge(spoils);
        }
        self.lines.push(Entry {
            what,
            spoils: spoils.clone(),
            paid_in,
        });
        let over = self.lines.len().saturating_sub(LEDGER_LINES);
        self.lines.drain(..over);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Barony {
    #[serde(default)]
    pub court: Court,
    #[serde(default)]
    pub works: Vec<Work>,
    #[serde(default)]
    pub mission: Option<super::bounties::Pinned>,
    #[serde(default)]
    pub done: Vec<String>,
    #[serde(default)]
    pub lit: Vec<String>,
    #[serde(default)]
    pub ledger: Ledger,
    #[serde(default, skip_serializing_if = "super::reclaim::Halls::is_empty")]
    pub halls: super::reclaim::Halls,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    KingsHall,
    Forge(Pack),
}

pub struct WorkDef {
    pub id: &'static str,
    pub name: &'static str,
    pub says: &'static str,
    pub price: &'static [(Spoil, u32)],
    pub labour: u32,
    pub after: Option<&'static str>,
    pub site: Site,
}

use Spoil::{Bone, Ember, Gem, Gold, Ore, Wax};

pub const WORKS: [WorkDef; 6] = [
    WorkDef {
        id: "hall",
        name: "The King's Hall",
        says: "rebuild Caer Dwfn's gate-hall",
        price: &[(Gold, 3000), (Ore, 40), (Bone, 20), (Gem, 6)],
        labour: 3 * 30 * 60,
        after: None,
        site: Site::KingsHall,
    },
    WorkDef {
        id: "ore-forge",
        name: "The Ore-Forge",
        says: "rebuild the Mines' forge",
        price: &[(Gold, 2000), (Ore, 30), (Gem, 4)],
        labour: 3 * 20 * 60,
        after: Some("workings"),
        site: Site::Forge(Pack::Cavern),
    },
    WorkDef {
        id: "bell-forge",
        name: "The Bell-Forge",
        says: "rebuild the Crypt's forge",
        price: &[(Gold, 2400), (Bone, 20), (Wax, 10)],
        labour: 3 * 20 * 60,
        after: Some("workings"),
        site: Site::Forge(Pack::Crypt),
    },
    WorkDef {
        id: "ember-forge",
        name: "The Ember-Forge",
        says: "rebuild Dragon Keep's forge",
        price: &[(Gold, 3000), (Ore, 30), (Gem, 6)],
        labour: 3 * 25 * 60,
        after: Some("workings"),
        site: Site::Forge(Pack::Hellforge),
    },
    WorkDef {
        id: "type-foundry",
        name: "The Type-Foundry",
        says: "rebuild the Archive's foundry",
        price: &[(Gold, 2800), (Ore, 20), (Gem, 6)],
        labour: 3 * 25 * 60,
        after: Some("workings"),
        site: Site::Forge(Pack::Archive),
    },
    WorkDef {
        id: "spore-kiln",
        name: "The Spore-Kiln",
        says: "rebuild the Fungal Deep's kiln",
        price: &[(Gold, 3200), (Bone, 20), (Gem, 8)],
        labour: 3 * 30 * 60,
        after: Some("workings"),
        site: Site::Forge(Pack::Fungal),
    },
];

pub fn work(id: &str) -> Option<&'static WorkDef> {
    WORKS.iter().find(|w| w.id == id)
}

pub struct Forge {
    pub id: &'static str,
    pub name: &'static str,
    pub pack: Pack,
    pub work: Option<&'static str>,
    pub relight: &'static [(Spoil, u32)],
    pub makes: &'static str,
    pub tribute: &'static [(Spoil, u32)],
}

pub const FORGES: [Forge; 6] = [
    Forge {
        id: "ore-forge",
        name: "The Ore-Forge",
        pack: Pack::Cavern,
        work: Some("ore-forge"),
        relight: &[(Ember, 8)],
        makes: "smelts the Mines' ore into ingots; dwarf-forged mail",
        tribute: &[(Gem, 1), (Gold, 60)],
    },
    Forge {
        id: "bell-forge",
        name: "The Bell-Forge",
        pack: Pack::Crypt,
        work: Some("bell-forge"),
        relight: &[(Ember, 10)],
        makes: "casts bells that call the folk to a holding's wall",
        tribute: &[],
    },
    Forge {
        id: "ember-forge",
        name: "The Ember-Forge",
        pack: Pack::Hellforge,
        work: Some("ember-forge"),
        relight: &[(Ember, 14)],
        makes: "tempers blades that arm a holding's men",
        tribute: &[],
    },
    Forge {
        id: "type-foundry",
        name: "The Type-Foundry",
        pack: Pack::Archive,
        work: Some("type-foundry"),
        relight: &[(Ember, 10)],
        makes: "casts the type that prints Wren's extra notices",
        tribute: &[],
    },
    Forge {
        id: "spore-kiln",
        name: "The Spore-Kiln",
        pack: Pack::Fungal,
        work: Some("spore-kiln"),
        relight: &[(Ember, 12)],
        makes: "fires spore-brew into the granary",
        tribute: &[],
    },
    Forge {
        id: "great-forge",
        name: "The Great Forge",
        pack: Pack::Unknown,
        work: None,
        relight: &[],
        makes: "the crown: lit only with the first fire",
        tribute: &[],
    },
];

pub fn forge(id: &str) -> Option<&'static Forge> {
    FORGES.iter().find(|f| f.id == id)
}

pub fn forge_of(pack: Pack) -> Option<&'static Forge> {
    FORGES.iter().find(|f| f.pack == pack && f.work.is_some())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    Clear(Pack),
    Fell(&'static str),
}

pub struct Mission {
    pub id: &'static str,
    pub title: &'static str,
    pub asks: &'static str,
    pub goal: Goal,
    pub need: u32,
    pub pays: &'static [(Spoil, u32)],
    pub after_lit: Option<&'static str>,
}

pub const MISSIONS: [Mission; 2] = [
    Mission {
        id: "workings",
        title: "The Upper Workings",
        asks: "Clear the Mines' halls of goblins, eight of them, and me miners go back to the ore.",
        goal: Goal::Clear(Pack::Cavern),
        need: 8,
        pays: &[(Gold, 150), (Ore, 10)],
        after_lit: None,
    },
    Mission {
        id: "first-fire",
        title: "The First Fire",
        asks: "Cinderjaw's been chewing me grandsire's fire for a hundred years. Fell him and bring it home.",
        goal: Goal::Fell("cinderjaw"),
        need: 1,
        pays: &[(Gem, 4)],
        after_lit: Some("ore-forge"),
    },
];

pub fn mission(id: &str) -> Option<&'static Mission> {
    MISSIONS.iter().find(|m| m.id == id)
}

pub const WORKINGS_TRIBUTE: &[(Spoil, u32)] = &[(Ore, 3), (Gold, 30)];

pub fn crew(barony: &Barony) -> u32 {
    3 + barony.done.len() as u32 + barony.halls.crew()
}

pub const FORGED_MAIL: u32 = 1;

pub const SHIFT_SECS: u32 = 10;

pub const HOLD: u32 = BUY_HOLD;

fn spoils(list: &[(Spoil, u32)]) -> Spoils {
    let mut s = Spoils::default();
    for &(spoil, n) in list {
        s.add(spoil, n);
    }
    s
}

impl WorkDef {
    pub fn cost(&self) -> Spoils {
        spoils(self.price)
    }
}

impl Forge {
    pub fn fire(&self) -> Spoils {
        spoils(self.relight)
    }
}

impl Mission {
    pub fn reward(&self) -> Spoils {
        spoils(self.pays)
    }
}

impl Barony {
    pub fn here(&self) -> bool {
        self.court >= Court::Camped
    }

    pub fn has_done(&self, mission: &str) -> bool {
        self.done.iter().any(|m| m == mission)
    }

    pub fn is_lit(&self, forge: &str) -> bool {
        self.lit.iter().any(|f| f == forge)
    }

    pub fn labour(&self, id: &str) -> Option<u32> {
        self.works.iter().find(|w| w.id == id).map(|w| w.labour)
    }

    pub fn progress(&self, id: &str) -> Option<u32> {
        let def = work(id)?;
        self.labour(id)
            .map(|l| (u64::from(l) * 1000 / u64::from(def.labour.max(1))).min(1000) as u32)
    }

    pub fn built(&self, id: &str) -> bool {
        self.progress(id) == Some(1000)
    }

    pub fn building(&self) -> Option<&'static WorkDef> {
        self.works
            .iter()
            .filter_map(|w| work(&w.id).map(|d| (w, d)))
            .find(|(w, d)| w.labour < d.labour)
            .map(|(_, d)| d)
    }

    pub fn holds_workings(&self) -> bool {
        self.has_done("workings")
    }

    pub fn may_commission(&self, def: &WorkDef) -> Result<(), String> {
        if self.court < Court::Met {
            return Err("King Brannoc has not been met yet".into());
        }
        if self.labour(def.id).is_some() {
            return Err(format!("{} is paid for already", def.name));
        }
        if let Some(m) = def.after.filter(|m| !self.has_done(m)) {
            let title = mission(m).map_or(m, |m| m.title);
            return Err(format!("{} waits on the King's mission: {title}", def.name));
        }
        Ok(())
    }

    pub fn miners(&self) -> u32 {
        if self.holds_workings() {
            3 + 2 * self.lit.len() as u32
        } else {
            0
        }
    }

    pub fn tribute(&self) -> Spoils {
        let mut t = Spoils::default();
        if self.holds_workings() {
            t.merge(&spoils(WORKINGS_TRIBUTE));
        }
        for f in FORGES.iter().filter(|f| self.is_lit(f.id)) {
            t.merge(&spoils(f.tribute));
        }
        t.merge(&self.halls.rent());
        t
    }

    pub fn pin(&mut self) -> Option<&'static Mission> {
        if self.court < Court::Met || self.mission.is_some() {
            return None;
        }
        let next = MISSIONS
            .iter()
            .find(|m| !self.has_done(m.id) && m.after_lit.is_none_or(|f| self.is_lit(f)))?;
        self.mission = Some(super::bounties::Pinned {
            id: next.id.into(),
            have: 0,
        });
        Some(next)
    }
}

pub fn print(b: &Barony) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (b.court as u8).hash(&mut h);
    for w in &b.works {
        (w.id.as_str(), b.progress(&w.id).unwrap_or(0) / 10).hash(&mut h);
    }
    b.lit.hash(&mut h);
    b.done.hash(&mut h);
    b.halls.won.iter().for_each(|w| w.id.hash(&mut h));
    h.finish()
}

pub fn clear_mark(pack: Pack) -> String {
    format!("clear:{pack:?}")
}

#[derive(Debug, Default, PartialEq)]
pub struct Settled {
    pub changed: bool,
    pub said: Vec<(String, String)>,
    pub earned: Vec<&'static str>,
}

impl Settled {
    fn say(&mut self, cue: &str, line: String) {
        self.said.push((cue.into(), line));
    }
}

pub fn settle(
    home: &mut Home,
    treasury: &mut Spoils,
    mines_cleared: u32,
    marks: &BTreeMap<String, u32>,
    who: &dyn Fn(u32) -> String,
) -> Settled {
    let mut out = Settled::default();
    let b = &mut home.barony;
    if b.court == Court::Absent && (mines_cleared > 0 || home.deepest >= 3) {
        b.court = Court::Camped;
        out.changed = true;
        out.say(
            "brannoc_arrives",
            "A dwarf king has come up the Mines' shaft and camps at the mine-head, by the ruin of his gate-hall: King Brannoc of Caer Dwfn".into(),
        );
    }
    if b.court == Court::Camped && marks.contains_key("barony:met") {
        b.court = Court::Met;
        out.changed = true;
    }
    let secs = marks.get("barony:labour").copied().unwrap_or(0);
    if secs > 0
        && let Some(def) = b.building()
    {
        let crew = crew(b);
        let w = b
            .works
            .iter_mut()
            .find(|w| w.id == def.id)
            .expect("building");
        let before = w.labour * 4 / def.labour.max(1);
        w.labour = (w.labour + secs * crew).min(def.labour);
        let after = w.labour * 4 / def.labour.max(1);
        out.changed = true;
        if w.labour >= def.labour {
            out.say(
                &format!("work_done:{}", def.id),
                format!("{} stands, built by King Brannoc's dwarves", def.name),
            );
            if def.id == "hall" {
                b.court = Court::Sworn;
                out.earned.push("fealty");
                out.say(
                    "brannoc_sworn",
                    "The King's Hall stands at the mine-head: King Brannoc kneels in it and swears Caer Dwfn to the crown, a baron of the realm".into(),
                );
            }
        } else if after > before {
            out.say(
                &format!("work_stage:{}", def.id),
                format!("{}: {}% built", def.name, after * 25),
            );
        }
    }
    for (mark, _) in marks.iter().filter(|(m, _)| m.starts_with("barony:order:")) {
        let mut parts = mark.trim_start_matches("barony:order:").split(':');
        let (Some(id), knight) = (parts.next(), parts.next().and_then(|k| k.parse().ok())) else {
            continue;
        };
        let Some(def) = work(id) else {
            continue;
        };
        if let Err(why) = b.may_commission(def) {
            out.say("cant_afford", why);
            continue;
        }
        let cost = def.cost();
        if !treasury.covers(&cost) {
            out.say(
                "cant_afford",
                format!("{}: {}", def.name, treasury.shortfall(&cost)),
            );
            continue;
        }
        treasury.take(&cost);
        b.works.push(Work {
            id: def.id.into(),
            labour: 0,
        });
        let by = knight.map(who).unwrap_or_else(|| "The realm".into());
        b.ledger
            .write(format!("{by} paid for {}", def.name), &cost, true);
        out.changed = true;
        out.say(
            &format!("commissioned:{}", def.id),
            format!(
                "{by} paid {} for {}: King Brannoc's dwarves start work",
                cost.label(),
                def.name
            ),
        );
    }
    for (mark, _) in marks
        .iter()
        .filter(|(m, _)| m.starts_with("barony:relight:"))
    {
        let mut parts = mark.trim_start_matches("barony:relight:").split(':');
        let (Some(id), knight) = (parts.next(), parts.next().and_then(|k| k.parse().ok())) else {
            continue;
        };
        let Some(f) = forge(id) else {
            continue;
        };
        if b.is_lit(f.id) || f.work.is_none_or(|w| !b.built(w)) {
            continue;
        }
        let fire = f.fire();
        if !treasury.covers(&fire) {
            out.say(
                "cant_afford",
                format!("{} needs fire: {}", f.name, treasury.shortfall(&fire)),
            );
            continue;
        }
        treasury.take(&fire);
        b.lit.push(f.id.into());
        let by = knight.map(who).unwrap_or_else(|| "The realm".into());
        b.ledger
            .write(format!("{by} brought fire for {}", f.name), &fire, true);
        out.changed = true;
        out.earned.push("hammers_ring");
        out.say(
            &format!("forge_relit:{}", f.id),
            format!("{} burns again. The hammers ring in Caer Dwfn", f.name),
        );
    }
    if let Some(pinned) = b.mission.as_mut()
        && let Some(m) = mission(&pinned.id)
    {
        let add = match m.goal {
            Goal::Clear(pack) => marks.get(&clear_mark(pack)).copied().unwrap_or(0),
            Goal::Fell(id) => marks.get(&format!("trophy:{id}")).copied().unwrap_or(0),
        };
        if add > 0 {
            pinned.have = (pinned.have + add).min(m.need);
            out.changed = true;
        }
        if pinned.have >= m.need {
            b.mission = None;
            b.done.push(m.id.into());
            let reward = m.reward();
            treasury.merge(&reward);
            b.ledger
                .write(format!("{} done: his thanks", m.title), &reward, false);
            out.say(
                &format!("mission_done:{}", m.id),
                format!(
                    "King Brannoc's mission done: {}, for {}",
                    m.title,
                    reward.label()
                ),
            );
        }
    }
    if let Some(m) = b.pin() {
        out.changed = true;
        out.say(
            &format!("mission:{}", m.id),
            format!("King Brannoc asks: {}. \"{}\"", m.title, m.asks),
        );
    }
    out
}

pub fn pay_tribute(home: &mut Home, treasury: &mut Spoils, floors: usize) -> Option<Spoils> {
    super::goods::rent_days(home, treasury, floors);
    let b = &mut home.barony;
    let one = b.tribute();
    if floors == 0 || one.is_empty() {
        return None;
    }
    let mut paid = Spoils::default();
    for _ in 0..floors {
        paid.merge(&one);
    }
    treasury.merge(&paid);
    let what = if floors == 1 {
        "Tribute: a floor cleared".to_string()
    } else {
        format!("Tribute: {floors} floors cleared")
    };
    b.ledger.write(what, &paid, false);
    Some(paid)
}

pub const CAMP_KING: (f32, f32) = (8.0, 8.9);
pub const KING_PLATE: (i32, i32, i32, i32) = (7, 10, 2, 1);
pub const CAMP_FIRE: (f32, f32) = (5.9, 10.2);
pub const CAMP_TENT: (f32, f32) = (3.6, 10.6);
pub const CAMP_GUARDS: [(f32, f32); 2] = [(9.9, 8.7), (4.9, 12.2)];
pub const HALL_DOOR: (i32, i32, i32, i32) = (5, 6, 2, 1);
pub const HALL_PLATE: (i32, i32, i32, i32) = (5, 7, 2, 1);
pub const STOCKPILE_AT: (f32, f32) = (9.6, 11.6);
const KING_REACH: f32 = 4.0;

pub const SEAT_DAIS: (i32, i32, i32, i32) = (10, 1, 4, 2);
pub const SEAT_AT: (f32, f32) = (12.0, 3.1);
pub const SEAT_PLATE: (i32, i32, i32, i32) = (11, 4, 2, 1);
pub const GREAT_FORGE: (i32, i32, i32, i32) = (19, 3, 4, 6);
pub const WAR_TABLE: (i32, i32, i32, i32) = (4, 8, 4, 2);
pub const TABLE_PLATE: (i32, i32, i32, i32) = (5, 10, 2, 1);
pub const LECTERN: (i32, i32, i32, i32) = (3, 3, 1, 1);
pub const LECTERN_PLATE: (i32, i32, i32, i32) = (3, 4, 1, 1);
pub const PILLARS: [(i32, i32); 4] = [(7, 3), (16, 3), (7, 7), (16, 7)];
pub const HALL_EXIT: (i32, i32, i32, i32) = (11, 12, 2, 1);
pub const HALL_GUARDS: [(f32, f32); 2] = [(9.0, 3.4), (15.0, 3.4)];

pub const FURNACE: (i32, i32, i32, i32) = (10, 3, 4, 4);
pub const ANVILS: [(i32, i32); 2] = [(7, 8), (16, 8)];
pub const FORGE_PLATE: (i32, i32, i32, i32) = (11, 8, 2, 1);

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

pub fn on_plate(run: &Run, plate: (i32, i32, i32, i32)) -> Option<u32> {
    run.players
        .iter()
        .find(|(_, h)| h.hp > 0 && !h.stone && inside(plate, h.x, h.y))
        .map(|(&id, _)| id)
}

pub fn king_at(b: &Barony) -> Option<(RoomKind, (f32, f32))> {
    match b.court {
        Court::Absent => None,
        Court::Sworn => Some((RoomKind::KingsHall, SEAT_AT)),
        _ => Some((RoomKind::MineHead, CAMP_KING)),
    }
}

pub fn king_plate(b: &Barony) -> (i32, i32, i32, i32) {
    if b.court == Court::Sworn {
        SEAT_PLATE
    } else {
        KING_PLATE
    }
}

impl Run {
    pub(super) fn tick_barony(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.home.barony.building().is_some()
            && self.tick.is_multiple_of(u64::from(SHIFT_SECS * HZ))
        {
            *self.marks.entry("barony:labour".into()).or_default() += SHIFT_SECS;
        }
        let kind = self.room().kind;
        if let Some((room, (kx, ky))) = king_at(&self.home.barony)
            && room == kind
            && self.at_home_now()
        {
            let (kx, ky) = (kx * TILE_UNITS, ky * TILE_UNITS);
            let near = self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - kx).hypot(h.y - ky) < KING_REACH);
            if near && !self.king_near {
                if self.home.barony.court == Court::Camped {
                    self.cues.push("npc:brannoc_meet".into());
                    *self.marks.entry("barony:met".into()).or_default() += 1;
                } else if kind == RoomKind::KingsHall {
                    self.cues.push("npc:brannoc_hall".into());
                } else {
                    self.cues.push("npc:brannoc".into());
                }
            }
            self.king_near = near;
        }
        match kind {
            RoomKind::MineHead if self.at_home_now() => {
                let b = &self.home.barony;
                if b.court >= Court::Met && b.labour("hall").is_none() {
                    self.hold_plate(inputs, HALL_PLATE, "barony:order:hall");
                }
            }
            RoomKind::Forge => {
                let b = &self.home.barony;
                let Some(f) = forge_of(self.dungeon.pack) else {
                    return;
                };
                let w = f.work.and_then(work);
                if let Some(w) = w.filter(|w| b.labour(w.id).is_none()) {
                    self.hold_plate(inputs, FORGE_PLATE, &format!("barony:order:{}", w.id));
                } else if w.is_some_and(|w| b.built(w.id)) && !b.is_lit(f.id) {
                    self.hold_plate(inputs, FORGE_PLATE, &format!("barony:relight:{}", f.id));
                }
            }
            _ => {}
        }
    }

    pub(super) fn hold_plate(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        plate: (i32, i32, i32, i32),
        what: &str,
    ) {
        let mut asked = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let pressing = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.interact);
            if !pressing {
                hero.buy_spent = false;
            }
            let on = hero.hp > 0 && !hero.stone && inside(plate, hero.x, hero.y);
            if on && pressing && !hero.buy_spent {
                hero.buying = hero.buying.saturating_add(1);
                if hero.buying >= HOLD {
                    hero.buying = 0;
                    hero.buy_spent = true;
                    asked.push(id);
                }
            } else if on {
                hero.buying = 0;
            }
        }
        for id in asked {
            *self.marks.entry(format!("{what}:{id}")).or_default() += 1;
            self.sounds.push("play_card");
        }
    }

    pub(super) fn barony_news(&mut self, before: &Barony) {
        self.reclaim_news(&before.halls);
        let now = &self.home.barony;
        for f in &now.lit {
            if !before.lit.contains(f) {
                self.fx.push(Fx::Shake(18));
                self.sounds.push("wheel_land");
                self.found = Some((self.tick, 0, "The forge is lit: the hammers ring".into()));
            }
        }
        for w in &now.works {
            let was = before.works.iter().find(|b| b.id == w.id).map(|b| b.labour);
            let Some(def) = work(&w.id) else { continue };
            if w.labour >= def.labour && was.is_some_and(|l| l < def.labour) {
                self.fx.push(Fx::Shake(10));
                self.sounds.push("rock_land");
            }
            if was.is_none() {
                self.sounds.push("door_open");
            }
        }
    }
}

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for row in r..r + h {
        for col in c..c + w {
            room.set(col as usize, row as usize, tile);
        }
    }
}

pub fn kings_hall_room(cell: (i32, i32)) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = RoomKind::KingsHall;
    fill(
        &mut room,
        (1, 1, COLS as i32 - 2, ROWS as i32 - 2),
        Tile::Floor,
    );
    fill(&mut room, SEAT_DAIS, Tile::Block);
    fill(&mut room, GREAT_FORGE, Tile::Block);
    fill(&mut room, WAR_TABLE, Tile::Block);
    fill(&mut room, LECTERN, Tile::Block);
    for &(c, r) in &PILLARS {
        fill(&mut room, (c, r, 1, 1), Tile::Block);
    }
    fill(&mut room, HALL_EXIT, Tile::Stairs);
    room
}

pub fn forge_room(cell: (i32, i32), door: usize) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = RoomKind::Forge;
    fill(
        &mut room,
        (1, 1, COLS as i32 - 2, ROWS as i32 - 2),
        Tile::Floor,
    );
    fill(&mut room, FURNACE, Tile::Block);
    for &(c, r) in &ANVILS {
        fill(&mut room, (c, r, 1, 1), Tile::Block);
    }
    super::goods::bins(&mut room);
    room.open_door(door);
    room
}

pub fn add_forge_hall(floor: &mut Floor, barony: &Barony) -> bool {
    if forge_of(floor.pack).is_none() || !barony.holds_workings() {
        return false;
    }
    if floor.depth == 0
        || floor.rooms.len() >= 64
        || floor.rooms.iter().any(|r| r.kind == RoomKind::Forge)
    {
        return false;
    }
    let taken = |c: (i32, i32)| floor.rooms.iter().any(|r| r.cell == c);
    let mut pick = None;
    'hosts: for (i, room) in floor.rooms.iter().enumerate() {
        let host = matches!(
            room.kind,
            RoomKind::Start
                | RoomKind::Sanctuary
                | RoomKind::Hall
                | RoomKind::Treasure
                | RoomKind::Fight
        ) && !room.great();
        if !host {
            continue;
        }
        for dir in [3usize, 1, 2, 0] {
            let (dx, dy) = DIRS[dir];
            let cell = (room.cell.0 + dx, room.cell.1 + dy);
            if !taken(cell) && (-16..=16).contains(&cell.0) && (-16..=16).contains(&cell.1) {
                pick = Some((i, dir, cell));
                break 'hosts;
            }
        }
    }
    let Some((host, dir, cell)) = pick else {
        return false;
    };
    floor.rooms[host].open_door(dir);
    floor.rooms.push(forge_room(cell, (dir + 2) % 4));
    true
}

#[cfg(test)]
#[path = "../../tests/rules/together_barony__tests.rs"]
mod tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../../tests/rules/together_barony__fixtures.rs"]
pub mod testing;
