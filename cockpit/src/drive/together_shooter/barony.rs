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
use crate::drive::together_realm::{Spoil, Spoils};

/// Where King Brannoc is in his story.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Court {
    /// Not come up the stair yet.
    #[default]
    Absent,
    /// Camped by the Winding Stair, waiting to be spoken to.
    Camped,
    /// Met: his missions begin.
    Met,
    /// His hall stands and he has sworn to the crown: a baron of the realm.
    Sworn,
}

/// A work the realm paid for, and the labour his dwarves have put in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Work {
    pub(crate) id: String,
    /// Crew-seconds of labour done (each dwarf at work adds one a second).
    #[serde(default)]
    pub(crate) labour: u32,
}

/// One line of the baron's ledger: what moved, which way.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) what: String,
    #[serde(default)]
    pub(crate) spoils: Spoils,
    /// True: the realm paid him. False: he paid the realm.
    #[serde(default)]
    pub(crate) paid_in: bool,
}

/// How many recent lines the ledger keeps (the totals are kept apart).
pub(crate) const LEDGER_LINES: usize = 8;

/// The relationship, in spoils: what the realm has paid into his works,
/// what he has paid back, and the latest lines.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Ledger {
    #[serde(default)]
    pub(crate) paid_in: Spoils,
    #[serde(default)]
    pub(crate) paid_back: Spoils,
    #[serde(default)]
    pub(crate) lines: Vec<Entry>,
}

impl Ledger {
    fn write(&mut self, what: String, spoils: &Spoils, paid_in: bool) {
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

/// The barony as the realm keeps it, inside `Home` (so a friend's mirror
/// sees it with the cellar). Every field defaults, so an older realm save
/// loads as a realm the King has not come to yet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Barony {
    #[serde(default)]
    pub(crate) court: Court,
    /// Works paid for, in the order they were paid: the crew builds the
    /// first unfinished one.
    #[serde(default)]
    pub(crate) works: Vec<Work>,
    /// The King's mission now, and how far along it is.
    #[serde(default)]
    pub(crate) mission: Option<super::bounties::Pinned>,
    /// Missions done, by id.
    #[serde(default)]
    pub(crate) done: Vec<String>,
    /// Forges relit, by id.
    #[serde(default)]
    pub(crate) lit: Vec<String>,
    #[serde(default)]
    pub(crate) ledger: Ledger,
}

/// Where a work stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Site {
    /// The gate-hall's ruin at the mine-head, in the world.
    KingsHall,
    /// Off the first hall of every floor of a delve.
    Forge(Pack),
}

/// A work the realm can pay for: its price, how much labour it takes, what
/// must come first, and where it stands.
pub(crate) struct WorkDef {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    /// What it does, on its ledger board (at most 30 characters).
    pub(crate) says: &'static str,
    pub(crate) price: &'static [(Spoil, u32)],
    /// Crew-seconds.
    pub(crate) labour: u32,
    /// The mission that must be done first (`None`: meeting him is enough).
    pub(crate) after: Option<&'static str>,
    pub(crate) site: Site,
}

use Spoil::{Bone, Ember, Gem, Gold, Ore};

/// The works. A perfect bot delve to Dragon Keep banks about 2,200 gold,
/// 14 ore, 7 gems and 46 embers (`haul_probe`), and Tobbin's dearest rung is
/// 1,200 gold: the King's Hall is most of two such delves, the Ore-Forge
/// most of one. Labour: one delve to Dragon Keep is about half an hour of
/// realm time, so three dwarves raise the hall in about one.
pub(crate) const WORKS: [WorkDef; 2] = [
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
];

pub(crate) fn work(id: &str) -> Option<&'static WorkDef> {
    WORKS.iter().find(|w| w.id == id)
}

/// One of Caer Dwfn's forges: the hall it stands in, the work that rebuilds
/// it, the fire that relights it, and what it gives once lit.
pub(crate) struct Forge {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) pack: Pack,
    /// Its rebuilding work (`None`: not rebuildable yet; Phase 2).
    pub(crate) work: Option<&'static str>,
    /// What relights it, carried up the stair.
    pub(crate) relight: &'static [(Spoil, u32)],
    /// Its specialty, for the war table.
    pub(crate) makes: &'static str,
    /// What it adds to the tribute for every floor the party clears.
    pub(crate) tribute: &'static [(Spoil, u32)],
}

/// The forges of Caer Dwfn, one a hall, the King's Great Forge last: the
/// crown of the chain, relit only with the first fire. Phase 1 rebuilds and
/// relights the Ore-Forge; the rest stand on the war table as lost.
pub(crate) const FORGES: [Forge; 6] = [
    Forge {
        id: "ore-forge",
        name: "The Ore-Forge",
        pack: Pack::Cavern,
        work: Some("ore-forge"),
        relight: &[(Ember, 8)],
        makes: "smelts the Mines' ore; dwarf-forged mail",
        tribute: &[(Gem, 1), (Gold, 60)],
    },
    Forge {
        id: "bell-forge",
        name: "The Bell-Forge",
        pack: Pack::Crypt,
        work: None,
        relight: &[],
        makes: "bells and candles for the dead",
        tribute: &[],
    },
    Forge {
        id: "ember-forge",
        name: "The Ember-Forge",
        pack: Pack::Hellforge,
        work: None,
        relight: &[],
        makes: "fire-arms from the dragon's coals",
        tribute: &[],
    },
    Forge {
        id: "type-foundry",
        name: "The Type-Foundry",
        pack: Pack::Archive,
        work: None,
        relight: &[],
        makes: "type and plates for Pip's maps",
        tribute: &[],
    },
    Forge {
        id: "spore-kiln",
        name: "The Spore-Kiln",
        pack: Pack::Fungal,
        work: None,
        relight: &[],
        makes: "Maud's draughts, fired",
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

pub(crate) fn forge(id: &str) -> Option<&'static Forge> {
    FORGES.iter().find(|f| f.id == id)
}

/// The forge of a delve's halls, if it has one the realm can rebuild.
pub(crate) fn forge_of(pack: Pack) -> Option<&'static Forge> {
    FORGES.iter().find(|f| f.pack == pack && f.work.is_some())
}

/// What a mission asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Goal {
    /// So many fight rooms cleared in a delve's halls.
    Clear(Pack),
    /// A guardian felled, by its id.
    Fell(&'static str),
}

/// One of the King's missions.
pub(crate) struct Mission {
    pub(crate) id: &'static str,
    pub(crate) title: &'static str,
    /// What he asks, in his words.
    pub(crate) asks: &'static str,
    pub(crate) goal: Goal,
    pub(crate) need: u32,
    pub(crate) pays: &'static [(Spoil, u32)],
    /// The forge that must be lit before he asks it (`None`: none).
    pub(crate) after_lit: Option<&'static str>,
}

pub(crate) const MISSIONS: [Mission; 2] = [
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

pub(crate) fn mission(id: &str) -> Option<&'static Mission> {
    MISSIONS.iter().find(|m| m.id == id)
}

/// Tribute for every floor the party clears, once the Upper Workings are
/// his again: the miners follow the knights down and dig what they free.
/// A Mines floor brings a bot about 10 ore (`haul_probe`); his miners add a
/// third again, and a little gold.
pub(crate) const WORKINGS_TRIBUTE: &[(Spoil, u32)] = &[(Ore, 3), (Gold, 30)];

/// His dwarves at work on a site: three of his own, and one more for every
/// mission done (they come home from the halls the knights free).
pub(crate) fn crew(barony: &Barony) -> u32 {
    3 + barony.done.len() as u32
}

/// How much mail the Ore-Forge's shirts are worth: one piece, three
/// damage turned from every hit.
pub(crate) const FORGED_MAIL: u32 = 1;

/// The realm seconds between labour marks: the run reports time in chunks,
/// so the realm is not written every tick.
pub(crate) const SHIFT_SECS: u32 = 10;

/// How a knight holds F at the King's plates, or a forge's: as long as a
/// station's.
pub(crate) const HOLD: u32 = BUY_HOLD;

fn spoils(list: &[(Spoil, u32)]) -> Spoils {
    let mut s = Spoils::default();
    for &(spoil, n) in list {
        s.add(spoil, n);
    }
    s
}

impl WorkDef {
    pub(crate) fn cost(&self) -> Spoils {
        spoils(self.price)
    }
}

impl Forge {
    pub(crate) fn fire(&self) -> Spoils {
        spoils(self.relight)
    }
}

impl Mission {
    pub(crate) fn reward(&self) -> Spoils {
        spoils(self.pays)
    }
}

impl Barony {
    pub(crate) fn here(&self) -> bool {
        self.court >= Court::Camped
    }

    pub(crate) fn has_done(&self, mission: &str) -> bool {
        self.done.iter().any(|m| m == mission)
    }

    pub(crate) fn is_lit(&self, forge: &str) -> bool {
        self.lit.iter().any(|f| f == forge)
    }

    /// A work's labour so far, if it was paid for.
    pub(crate) fn labour(&self, id: &str) -> Option<u32> {
        self.works.iter().find(|w| w.id == id).map(|w| w.labour)
    }

    /// How far a work stands, 0 to 1000 (per mille), if paid for.
    pub(crate) fn progress(&self, id: &str) -> Option<u32> {
        let def = work(id)?;
        self.labour(id)
            .map(|l| (u64::from(l) * 1000 / u64::from(def.labour.max(1))).min(1000) as u32)
    }

    pub(crate) fn built(&self, id: &str) -> bool {
        self.progress(id) == Some(1000)
    }

    /// The work the crew is on: the first paid for and unfinished.
    pub(crate) fn building(&self) -> Option<&'static WorkDef> {
        self.works
            .iter()
            .filter_map(|w| work(&w.id).map(|d| (w, d)))
            .find(|(w, d)| w.labour < d.labour)
            .map(|(_, d)| d)
    }

    /// The Upper Workings are his: the Mines' forge is found, its miners
    /// at work.
    pub(crate) fn holds_workings(&self) -> bool {
        self.has_done("workings")
    }

    /// Can a work be paid for now? Why not, if not.
    pub(crate) fn may_commission(&self, def: &WorkDef) -> Result<(), String> {
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

    /// Miners at work below: two once the Upper Workings are his, two more
    /// for every forge lit.
    pub(crate) fn miners(&self) -> u32 {
        if self.holds_workings() {
            3 + 2 * self.lit.len() as u32
        } else {
            0
        }
    }

    /// What one floor cleared pays him and the realm.
    pub(crate) fn tribute(&self) -> Spoils {
        let mut t = Spoils::default();
        if self.holds_workings() {
            t.merge(&spoils(WORKINGS_TRIBUTE));
        }
        for f in FORGES.iter().filter(|f| self.is_lit(f.id)) {
            t.merge(&spoils(f.tribute));
        }
        t
    }

    /// Pin the next mission, if none is pinned and one is ready. Returns it.
    pub(crate) fn pin(&mut self) -> Option<&'static Mission> {
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

/// What the barony looks like, for a drawing kept between frames: the
/// court, each work to the percent, the forges lit, the missions done.
pub(crate) fn print(b: &Barony) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (b.court as u8).hash(&mut h);
    for w in &b.works {
        (w.id.as_str(), b.progress(&w.id).unwrap_or(0) / 10).hash(&mut h);
    }
    b.lit.hash(&mut h);
    b.done.hash(&mut h);
    h.finish()
}

/// The mark a cleared fight room leaves, by the delve it is in.
pub(crate) fn clear_mark(pack: Pack) -> String {
    format!("clear:{pack:?}")
}

/// What `settle` did, for the cockpit to say and announce.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Settled {
    /// The realm changed and wants saving.
    pub(crate) changed: bool,
    /// (chorus cue, notice line), in order.
    pub(crate) said: Vec<(String, String)>,
    /// Achievements earned.
    pub(crate) earned: Vec<&'static str>,
}

impl Settled {
    fn say(&mut self, cue: &str, line: String) {
        self.said.push((cue.into(), line));
    }
}

/// The cockpit's half of the barony, once a frame: King Brannoc comes when
/// a floor of the Mines has been cleared (`mines_cleared`, the realm's
/// count) or the party has been below the second floor; what the run marked
/// is paid, worked and counted. `who` names a knight by id for the ledger.
pub(crate) fn settle(
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
    // Labour: the crew on the first unfinished work.
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
    // Paid works, from their plates.
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
    // Forges relit, with fire carried up from below.
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
    // His mission: what the run did toward it.
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

/// Tribute for the floors a delve just cleared (one entry each), paid into
/// the treasury and written in the ledger. What was paid, if anything.
pub(crate) fn pay_tribute(home: &mut Home, treasury: &mut Spoils, floors: usize) -> Option<Spoils> {
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

// ── The run's half: plates, greetings, marks ─────────────────────────────

/// The King's camp at the mine-head, before his hall stands: where he
/// stands, the plate before him (his mission is read there), his fire and
/// tent, his two guards (tiles).
pub(crate) const CAMP_KING: (f32, f32) = (8.0, 8.9);
pub(crate) const KING_PLATE: (i32, i32, i32, i32) = (7, 10, 2, 1);
pub(crate) const CAMP_FIRE: (f32, f32) = (5.9, 10.2);
pub(crate) const CAMP_TENT: (f32, f32) = (3.6, 10.6);
pub(crate) const CAMP_GUARDS: [(f32, f32); 2] = [(9.9, 8.7), (4.9, 12.2)];
/// The gate-hall's great door, in the ruin's south wall (walk-on, open
/// once the hall stands), and the plate before it where the rebuilding is
/// paid for (tiles).
pub(crate) const HALL_DOOR: (i32, i32, i32, i32) = (5, 6, 2, 1);
pub(crate) const HALL_PLATE: (i32, i32, i32, i32) = (5, 7, 2, 1);
/// The barony's stockpile in the yard, where the porters drop the ore
/// (tiles, the middle of the heap).
pub(crate) const STOCKPILE_AT: (f32, f32) = (9.6, 11.6);
/// How near the King a knight comes before he speaks.
const KING_REACH: f32 = 4.0;

/// The King's Hall, inside: his seat on its dais against the north wall,
/// the Great Forge at the east end (cold until the first fire), the war
/// table with the forges of Caer Dwfn on it, the ledger's lectern, four
/// pillars, and the great door out (tiles).
pub(crate) const SEAT_DAIS: (i32, i32, i32, i32) = (10, 1, 4, 2);
pub(crate) const SEAT_AT: (f32, f32) = (12.0, 3.1);
pub(crate) const SEAT_PLATE: (i32, i32, i32, i32) = (11, 4, 2, 1);
pub(crate) const GREAT_FORGE: (i32, i32, i32, i32) = (19, 3, 4, 6);
pub(crate) const WAR_TABLE: (i32, i32, i32, i32) = (4, 8, 4, 2);
pub(crate) const TABLE_PLATE: (i32, i32, i32, i32) = (5, 10, 2, 1);
pub(crate) const LECTERN: (i32, i32, i32, i32) = (3, 3, 1, 1);
pub(crate) const LECTERN_PLATE: (i32, i32, i32, i32) = (3, 4, 1, 1);
pub(crate) const PILLARS: [(i32, i32); 4] = [(7, 3), (16, 3), (7, 7), (16, 7)];
pub(crate) const HALL_EXIT: (i32, i32, i32, i32) = (11, 12, 2, 1);
/// The hall's guards either side of the seat.
pub(crate) const HALL_GUARDS: [(f32, f32); 2] = [(9.0, 3.4), (15.0, 3.4)];

/// A forge-hall below (tiles): the furnace in the middle, two anvils, and
/// the plate before its mouth where it is paid for and relit.
pub(crate) const FURNACE: (i32, i32, i32, i32) = (10, 3, 4, 4);
pub(crate) const ANVILS: [(i32, i32); 2] = [(7, 8), (16, 8)];
pub(crate) const FORGE_PLATE: (i32, i32, i32, i32) = (11, 8, 2, 1);

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

/// Is a standing knight on `plate`? Which one.
pub(crate) fn on_plate(run: &Run, plate: (i32, i32, i32, i32)) -> Option<u32> {
    run.players
        .iter()
        .find(|(_, h)| h.hp > 0 && !h.stone && inside(plate, h.x, h.y))
        .map(|(&id, _)| id)
}

/// Where the King is (tiles), and in which room: at his camp by the ruin,
/// then on his seat in his hall.
pub(crate) fn king_at(b: &Barony) -> Option<(RoomKind, (f32, f32))> {
    match b.court {
        Court::Absent => None,
        Court::Sworn => Some((RoomKind::KingsHall, SEAT_AT)),
        _ => Some((RoomKind::MineHead, CAMP_KING)),
    }
}

/// The plate his mission is read at, wherever he is.
pub(crate) fn king_plate(b: &Barony) -> (i32, i32, i32, i32) {
    if b.court == Court::Sworn {
        SEAT_PLATE
    } else {
        KING_PLATE
    }
}

impl Run {
    /// One tick of the barony, wherever the party is: realm time for the
    /// works, the King's greeting, and the plates at the ruin and the
    /// forges.
    pub(super) fn tick_barony(&mut self, inputs: &BTreeMap<u32, Input>) {
        // Labour goes on wherever the party is: realm time is play time.
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

    /// Holding F on `plate` marks `what` (with the knight's id) once the
    /// hold fills, like a station's plate.
    fn hold_plate(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        plate: (i32, i32, i32, i32),
        what: &str,
    ) {
        let mut asked = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            if !fire {
                hero.buy_spent = false;
            }
            let on = hero.hp > 0 && !hero.stone && inside(plate, hero.x, hero.y);
            if on && fire && !hero.buy_spent {
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

    /// The cockpit's word on the barony changed: what stands now shows.
    /// Called from `rebuild_home` with the barony before.
    pub(super) fn barony_news(&mut self, before: &Barony) {
        let now = &self.home.barony;
        for f in &now.lit {
            if !before.lit.contains(f) {
                self.shake = self.shake.max(18);
                self.sounds.push("wheel_land");
                self.found = Some((self.tick, 0, "The forge is lit: the hammers ring".into()));
            }
        }
        for w in &now.works {
            let was = before.works.iter().find(|b| b.id == w.id).map(|b| b.labour);
            let Some(def) = work(&w.id) else { continue };
            if w.labour >= def.labour && was.is_some_and(|l| l < def.labour) {
                self.shake = self.shake.max(10);
                self.sounds.push("rock_land");
            }
            if was.is_none() {
                self.sounds.push("door_open");
            }
        }
    }
}

// ── Layout: the King's Hall inside, a forge-hall below ───────────────────

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for row in r..r + h {
        for col in c..c + w {
            room.set(col as usize, row as usize, tile);
        }
    }
}

/// The King's Hall, inside: one room of the world, reached only by its
/// great door at the mine-head (it has no doorways of its own).
pub(crate) fn kings_hall_room(cell: (i32, i32)) -> Room {
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

/// A forge-hall below: its furnace in the middle and two anvils, a door
/// toward the hall it hangs off.
pub(crate) fn forge_room(cell: (i32, i32), door: usize) -> Room {
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
    room.open_door(door);
    room
}

/// Once the King holds a delve's upper halls, every floor of it has its
/// forge-hall, off the entrance (or the first hall with room beside it).
/// Chosen without dice: the first free neighbour, west, east, south, north,
/// so every other room stays as the seed laid it. Called before the floor's
/// politics are drawn, so they see the room. True if a room was added.
pub(crate) fn add_forge_hall(floor: &mut Floor, barony: &Barony) -> bool {
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
#[path = "../../../../tests/cockpit/app/together_barony__tests.rs"]
mod tests;
