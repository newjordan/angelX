//! Deeds: the harness's tool calls as the realm's living traffic.
//!
//! Each kind of work has its own country, and a call in flight puts someone
//! to work there until its result comes back:
//!
//! - trials (tests, builds, checks): a cargo wagon from the Smithy waits at
//!   the Lists gate for the verdict;
//! - the outside world (web, push, fetch): a courier rides out of the
//!   Gatehouse and is gone for as long as the answer takes;
//! - commits: a raven lifts off the Rookery with the sealed scroll;
//! - searches: field hands work the crop rows in the south-east fields, and
//!   each harvest goes up the road to the Scriptorium with a messenger;
//! - research: the Observatory's glass sweeps the sky, and an owl carries
//!   each finding across the realm to the Scriptorium;
//! - shell errands: villagers carry sacks from their cottages to the granary;
//! - memory: a monk walks the chapel yard with a candle;
//! - edits: sparks fly off the Smithy anvil.
//!
//! What a deed achieves stays in the realm for the session, where it was
//! earned: pennants on the Lists fence (verified green passed, amber failed),
//! ravens on the Rookery roost, blades on the Smithy rack, books by the
//! Scriptorium, stooks in the fields, sacks at the granary, candles on the
//! chapel step, stars over the Observatory. Nothing moves that the harness
//! did not do, and nothing stays that it did not finish.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::OnceLock;

use super::super::{Deed, herald};
use super::live::{route, step_cost};
use super::map::{Place, TILE, place_px, place_tile};
use crate::agent::harness::ToolEventId;

/// Trials the Lists fence remembers; older pennants come down.
pub(crate) const PENNANTS: usize = 8;
/// World ticks (40 Hz) a wagon stands showing its verdict before it turns
/// for home.
const VERDICT_HOLD: u64 = 100;
/// A raven is aloft at least this long, so a quick commit is still seen.
const FLIGHT: u64 = 120;
/// The least time a field hand or a monk is seen at work, however quick
/// the call.
const WORK: u64 = 80;
/// Messengers or owls on the road at once; more findings share their loads.
const ON_THE_ROAD: usize = 4;
/// World ticks a villager spends inside the granary delivering its sack.
const DELIVER: u64 = 30;
/// World ticks a villager spends indoors before going out on the next errand.
const HOME_BEAT: u64 = 16;
/// World ticks the anvil throws sparks after a strike.
pub(crate) const SPARKS: u64 = 16;

/// Who goes out for a deed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Errand {
    Wagon,
    Courier,
    Raven,
    Hand,
    Messenger,
    Villager,
    Monk,
    Owl,
}

impl Errand {
    fn of(deed: Deed) -> Option<Errand> {
        match deed {
            Deed::Trial => Some(Errand::Wagon),
            Deed::Dispatch => Some(Errand::Courier),
            Deed::Seal => Some(Errand::Raven),
            Deed::Seek => Some(Errand::Hand),
            Deed::Errand => Some(Errand::Villager),
            Deed::Memory => Some(Errand::Monk),
            _ => None,
        }
    }

    /// How many can be out at once; later calls join the least busy.
    fn crew(self) -> usize {
        match self {
            Errand::Hand => 3,
            Errand::Villager => 4,
            _ => 1,
        }
    }

    /// World pixels per world tick.
    fn pace(self) -> f32 {
        match self {
            Errand::Wagon => 1.0,
            Errand::Courier => 2.4,
            Errand::Raven => 1.6,
            Errand::Hand => 1.2,
            Errand::Messenger => 2.6,
            Errand::Villager => 1.4,
            Errand::Monk => 0.9,
            Errand::Owl => 2.2,
        }
    }
}

/// One figure out on a deed, as the map draws it (world pixels, feet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Wayfarer {
    pub(crate) errand: Errand,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) facing_left: bool,
    pub(crate) moving: bool,
    /// `Some(passed)` once every call it went out for has come back.
    pub(crate) verdict: Option<bool>,
    /// Bearing its load: a raven's scroll, a villager's sack, a messenger's
    /// sheaf, an owl's finding.
    pub(crate) carrying: bool,
}

/// The session's record, kept in the realm where each deed was done.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Record {
    /// Trials, newest last: `true` passed. The fence keeps [`PENNANTS`].
    pub(crate) pennants: Vec<bool>,
    /// Commits sealed and flown.
    pub(crate) ravens: u32,
    /// Distinct files forged.
    pub(crate) blades: u32,
    /// Distinct files studied.
    pub(crate) books: u32,
    /// Searches harvested.
    pub(crate) stooks: u32,
    /// Errands delivered.
    pub(crate) sacks: u32,
    /// Memories kept or recalled.
    pub(crate) candles: u32,
    /// Research findings.
    pub(crate) stars: u32,
}

#[derive(Clone, Debug)]
struct Call {
    deed: Deed,
    object: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Leg {
    /// Heading out, or at the work and waiting on it.
    Out,
    /// Every call is back; the wagon shows its verdict until `until`.
    Verdict { passed: bool, until: u64 },
    /// Going home — or, for a messenger or owl, to the Scriptorium —
    /// carrying the verdict if there is one; gone on arrival.
    Home { verdict: Option<bool> },
    /// Indoors until `until` — delivering at the granary, or a beat at home
    /// before the next errand — then on to `next`.
    Inside { until: u64, next: After },
}

/// Where a villager goes when it comes back out.
#[derive(Clone, Copy, Debug, PartialEq)]
enum After {
    Home(Option<bool>),
    Out,
}

#[derive(Clone, Debug)]
struct Party {
    errand: Errand,
    /// Which of a crew's rows or cottages this one works from.
    slot: usize,
    calls: BTreeSet<ToolEventId>,
    /// Set once every call is back: whether all of them passed.
    settled: Option<bool>,
    failed: bool,
    x: f32,
    y: f32,
    path: VecDeque<(f32, f32)>,
    leg: Leg,
    facing_left: bool,
    since: u64,
    /// Steps taken on a working round (a crop row, the chapel yard).
    round: usize,
    /// Errands that came in while this villager was on its way home: it
    /// finishes the walk, goes in, and comes back out with them.
    again: BTreeSet<ToolEventId>,
    /// Owed another trip, and whether its errands have all passed so far.
    owes: Option<bool>,
    /// Errands done and not yet delivered: sacks bound for the granary.
    load: u32,
}

/// Every deed in flight and the record of those finished.
#[derive(Clone, Debug, Default)]
pub(crate) struct Deeds {
    calls: BTreeMap<ToolEventId, Call>,
    out: Vec<Party>,
    record: Record,
    forged: BTreeSet<String>,
    studied: BTreeSet<String>,
    struck: Option<u64>,
}

// ─── the realm's places of work ─────────────────────────────────────────────

fn feet((tx, ty): (i32, i32)) -> (f32, f32) {
    ((tx * TILE + TILE / 2) as f32, ((ty + 1) * TILE - 2) as f32)
}

fn tile_of((x, y): (f32, f32)) -> (i32, i32) {
    (
        (x / TILE as f32).floor() as i32,
        (y / TILE as f32).floor() as i32,
    )
}

fn road(from: (f32, f32), to: (i32, i32)) -> VecDeque<(f32, f32)> {
    route(tile_of(from), to).into_iter().map(feet).collect()
}

fn stand(place: Place) -> (i32, i32) {
    place.stand_world()
}

/// The Rookery roost in authored pixels: its left end, and the base it
/// stands on in front of the tower.
pub(crate) const ROOST: (i32, i32) = (25 * TILE, 15 * TILE - 2);

/// Where the `n`th raven sits on the roost (realm pixels, feet).
pub(crate) fn raven_perch(n: u32) -> (f32, f32) {
    let (x, base) = ROOST;
    let (x, y) = place_px(x + 4 + (n % 5) as i32 * 5, base - 9);
    (x as f32, y as f32)
}

/// The Rookery door, where a raven goes in when it has nothing to seal.
fn rookery_door() -> (f32, f32) {
    feet(stand(Place::Rookery))
}

/// Where the raven wheels while its commit is sealed.
fn wheel(since: u64, tick: u64) -> (f32, f32) {
    let (px, py) = raven_perch(0);
    let a = (tick.saturating_sub(since)) as f32 * 0.06;
    (px + 26.0 * a.cos(), py - 22.0 + 12.0 * a.sin())
}

/// Where the wagon draws up at the Lists: beside the knight's stand, not on it.
fn wagon_bay() -> (i32, i32) {
    let (sx, sy) = stand(Place::Lists);
    [(sx - 2, sy), (sx - 1, sy + 1), (sx, sy + 1)]
        .into_iter()
        .find(|&(x, y)| step_cost(x, y).is_some())
        .unwrap_or((sx, sy))
}

/// The lane the courier takes off the map, and the point past its edge.
fn gate_edge() -> ((i32, i32), (f32, f32)) {
    let (_, gy) = stand(Place::Gatehouse);
    let (_, y) = feet((0, gy));
    ((0, gy), (-2.0 * TILE as f32, y))
}

/// The crossroads in the south-east fields, where the hands come and go.
fn field_gate() -> (i32, i32) {
    place_tile(39, 27)
}

/// The crop rows the field hands work, one to a hand: its two ends.
const ROWS: [((i32, i32), (i32, i32)); 3] = [
    ((34, 25), (37, 25)),
    ((41, 24), (45, 24)),
    ((41, 28), (44, 28)),
];

fn row(slot: usize, end: usize) -> (i32, i32) {
    let (a, b) = ROWS[slot % ROWS.len()];
    let (tx, ty) = if end.is_multiple_of(2) { a } else { b };
    place_tile(tx, ty)
}

/// Where the stooks stand in the fields (authored tiles), in the order the
/// harvests set them.
pub(crate) const STOOKS: [(i32, i32); 12] = [
    (35, 24),
    (42, 25),
    (36, 26),
    (44, 25),
    (42, 29),
    (34, 26),
    (35, 30),
    (43, 28),
    (37, 24),
    (45, 25),
    (34, 29),
    (44, 29),
];

/// The cottage doors the villagers come from, one to a villager.
fn cottage_door(slot: usize) -> (i32, i32) {
    const DOORS: [(i32, i32); 4] = [(19, 26), (26, 26), (22, 26), (29, 26)];
    let (tx, ty) = DOORS[slot % DOORS.len()];
    place_tile(tx, ty)
}

/// The granary door, where every errand's sack is delivered.
fn granary_door() -> (i32, i32) {
    place_tile(28, 31)
}

/// The chapel yard: open ground about the Chapel door that the monk walks.
fn chapel_yard() -> &'static [(i32, i32)] {
    static YARD: OnceLock<Vec<(i32, i32)>> = OnceLock::new();
    YARD.get_or_init(|| {
        let (sx, sy) = stand(Place::Chapel);
        [(-1, 0), (2, 0), (1, 1), (0, 1), (-2, 1)]
            .into_iter()
            .map(|(dx, dy)| (sx + dx, sy + dy))
            .filter(|&(x, y)| step_cost(x, y).is_some())
            .take(3)
            .collect()
    })
}

/// The Observatory dome, where the glass looks out and the owls leave from.
pub(crate) fn dome() -> (f32, f32) {
    let (tx, ty, tw, _) = Place::Observatory.footprint();
    let (x, y) = place_px(tx * TILE + tw * TILE / 2, ty * TILE + 4);
    (x as f32, y as f32)
}

/// Where findings are brought: the Scriptorium door.
fn scriptorium() -> (i32, i32) {
    stand(Place::Scriptorium)
}

// ─── one party on the road ───────────────────────────────────────────────────

impl Party {
    fn new(errand: Errand, slot: usize, tick: u64) -> Party {
        let home = match errand {
            Errand::Wagon => feet(stand(Place::Smithy)),
            Errand::Courier => feet(stand(Place::Gatehouse)),
            Errand::Raven => raven_perch(0),
            Errand::Hand => feet(field_gate()),
            Errand::Villager => feet(cottage_door(slot)),
            Errand::Monk => feet(stand(Place::Chapel)),
            Errand::Messenger => feet(field_gate()),
            Errand::Owl => dome(),
        };
        let mut party = Party {
            errand,
            slot,
            calls: BTreeSet::new(),
            settled: None,
            failed: false,
            x: home.0,
            y: home.1,
            path: VecDeque::new(),
            leg: Leg::Out,
            facing_left: false,
            since: tick,
            round: 0,
            again: BTreeSet::new(),
            owes: None,
            load: 0,
        };
        party.head_out();
        party
    }

    /// A finding on its way to the Scriptorium: from a harvest in the
    /// fields by road, from the Observatory by air.
    fn bearer(errand: Errand, from: (f32, f32), tick: u64) -> Party {
        let mut party = Party::new(errand, 0, tick);
        (party.x, party.y) = from;
        party.leg = Leg::Home {
            verdict: Some(true),
        };
        party.path = match errand {
            Errand::Owl => VecDeque::from([feet(scriptorium())]),
            _ => road(from, scriptorium()),
        };
        party
    }

    fn at(&self) -> (f32, f32) {
        (self.x, self.y)
    }

    fn head_out(&mut self) {
        self.leg = Leg::Out;
        self.settled = None;
        self.failed = false;
        self.path = match self.errand {
            Errand::Wagon => road(self.at(), wagon_bay()),
            Errand::Courier => {
                let (lane, beyond) = gate_edge();
                let mut path = road(self.at(), lane);
                path.push_back(beyond);
                path
            }
            Errand::Hand => road(self.at(), row(self.slot, 0)),
            Errand::Villager => road(self.at(), granary_door()),
            Errand::Raven | Errand::Monk | Errand::Messenger | Errand::Owl => VecDeque::new(),
        };
    }

    /// Turn for home. A raven with a sealed commit flies to `perch`.
    fn head_home(&mut self, verdict: Option<bool>, perch: (f32, f32)) {
        self.leg = Leg::Home { verdict };
        self.path = match self.errand {
            Errand::Raven => match verdict {
                Some(true) => VecDeque::from([perch]),
                _ => VecDeque::from([rookery_door()]),
            },
            Errand::Courier if self.x < 0.0 => {
                let (lane, _) = gate_edge();
                let mut path = VecDeque::from([feet(lane)]);
                path.extend(route(lane, stand(Place::Gatehouse)).into_iter().map(feet));
                path
            }
            Errand::Courier => road(self.at(), stand(Place::Gatehouse)),
            Errand::Wagon => road(self.at(), stand(Place::Smithy)),
            Errand::Hand => road(self.at(), field_gate()),
            Errand::Villager => road(self.at(), cottage_door(self.slot)),
            Errand::Monk => road(self.at(), stand(Place::Chapel)),
            Errand::Messenger | Errand::Owl => std::mem::take(&mut self.path),
        };
    }

    /// Whether a settled party may act on its verdict yet.
    fn ready(&self, tick: u64) -> bool {
        match self.errand {
            Errand::Raven => tick >= self.since + FLIGHT,
            Errand::Hand | Errand::Monk => tick >= self.since + WORK,
            // A villager delivers at the granary before turning home.
            Errand::Villager => self.path.is_empty(),
            _ => true,
        }
    }

    /// One world tick. Returns whether the party is still out, and how many
    /// sacks it delivered to the granary this tick.
    fn step(&mut self, tick: u64, perch: (f32, f32)) -> (bool, u32) {
        let mut delivered = 0;
        if self.leg == Leg::Out
            && let Some(passed) = self.settled
            && self.ready(tick)
        {
            match self.errand {
                Errand::Villager if self.load > 0 => {
                    // It goes in with the sacks; a failed errand's sack comes
                    // back out with it.
                    delivered = std::mem::take(&mut self.load);
                    self.leg = Leg::Inside {
                        until: tick + DELIVER,
                        next: After::Home(Some(passed)),
                    };
                }
                Errand::Wagon => {
                    self.path.clear();
                    self.leg = Leg::Verdict {
                        passed,
                        until: tick + VERDICT_HOLD,
                    };
                }
                Errand::Hand | Errand::Monk => self.head_home(None, perch),
                _ => self.head_home(Some(passed), perch),
            }
        }
        match self.leg {
            Leg::Out => match self.errand {
                Errand::Raven => {
                    let (x, y) = wheel(self.since, tick);
                    self.facing_left = x < self.x;
                    (self.x, self.y) = (x, y);
                    return (true, delivered);
                }
                Errand::Hand if self.path.is_empty() => {
                    self.round += 1;
                    self.path = road(self.at(), row(self.slot, self.round));
                }
                Errand::Monk if self.path.is_empty() => {
                    let yard = chapel_yard();
                    if let Some(&next) = yard.get(self.round % yard.len().max(1)) {
                        self.round += 1;
                        self.path = road(self.at(), next);
                    }
                }
                _ => {}
            },
            Leg::Verdict { passed, until } if tick >= until => self.head_home(Some(passed), perch),
            Leg::Verdict { .. } => return (true, delivered),
            Leg::Inside { until, .. } if tick < until => return (true, delivered),
            Leg::Inside { next, .. } => match next {
                After::Home(verdict) => self.head_home(verdict, perch),
                After::Out => self.come_back_out(tick),
            },
            Leg::Home { .. } => {}
        }
        self.walk();
        if matches!(self.leg, Leg::Home { .. }) && self.path.is_empty() {
            if self.owes.is_none() {
                return (false, delivered);
            }
            // Home, but owed another errand: a beat indoors, then out again.
            self.leg = Leg::Inside {
                until: tick + HOME_BEAT,
                next: After::Out,
            };
        }
        (true, delivered)
    }

    /// Out of the door again with the errands that came in on the way home.
    fn come_back_out(&mut self, tick: u64) {
        let all_passed = self.owes.take().unwrap_or(true);
        self.since = tick;
        self.head_out();
        self.calls = std::mem::take(&mut self.again);
        if self.calls.is_empty() {
            self.settled = Some(all_passed);
        }
        self.failed = !all_passed;
    }

    fn walk(&mut self) {
        let mut stride = self.errand.pace();
        while stride > 0.0 {
            let Some(&(gx, gy)) = self.path.front() else {
                break;
            };
            let (dx, dy) = (gx - self.x, gy - self.y);
            if dx.abs() > 0.01 {
                self.facing_left = dx < 0.0;
            }
            let d = (dx * dx + dy * dy).sqrt();
            if d <= stride {
                (self.x, self.y) = (gx, gy);
                stride -= d;
                self.path.pop_front();
            } else {
                self.x += dx / d * stride;
                self.y += dy / d * stride;
                stride = 0.0;
            }
        }
    }

    fn shown(&self) -> Option<Wayfarer> {
        // A courier past the edge is out in the world, not on the map.
        // Whoever is indoors is out of sight too.
        if self.x < 0.0 || matches!(self.leg, Leg::Inside { .. }) {
            return None;
        }
        let verdict = match self.leg {
            Leg::Out | Leg::Inside { .. } => None,
            Leg::Verdict { passed, .. } => Some(passed),
            Leg::Home { verdict } => verdict,
        };
        let carrying = match self.errand {
            Errand::Raven => self.leg == Leg::Out,
            Errand::Villager => verdict != Some(true),
            Errand::Messenger | Errand::Owl | Errand::Monk => true,
            _ => false,
        };
        Some(Wayfarer {
            errand: self.errand,
            x: self.x,
            y: self.y,
            facing_left: self.facing_left,
            moving: !self.path.is_empty() || self.errand == Errand::Raven,
            verdict,
            carrying,
        })
    }
}

// ─── the ledger of deeds ─────────────────────────────────────────────────────

impl Deeds {
    /// A tool call starts: note what it is and send someone out for it.
    pub(crate) fn begin(&mut self, id: &ToolEventId, name: &str, args: &str, tick: u64) {
        let told = herald(name, args);
        if told.deed == Deed::Forge {
            self.struck = Some(tick);
        }
        self.calls.insert(
            id.clone(),
            Call {
                deed: told.deed,
                object: told.object,
            },
        );
        let Some(errand) = Errand::of(told.deed) else {
            return;
        };
        let crew: Vec<usize> = (0..self.out.len())
            .filter(|&i| self.out[i].errand == errand)
            .collect();
        if crew.len() < errand.crew() {
            let slot = (0..)
                .find(|slot| !crew.iter().any(|&i| self.out[i].slot == *slot))
                .unwrap_or(0);
            let mut party = Party::new(errand, slot, tick);
            party.calls.insert(id.clone());
            self.out.push(party);
            return;
        }
        // The crew is all out: the least busy one still at work takes it.
        // A field hand heading home turns back to the rows; a villager
        // finishes the walk home, goes in, and comes back out for it.
        let Some(pick) = crew.iter().copied().min_by_key(|&i| {
            let party = &self.out[i];
            (
                party.leg != Leg::Out || party.settled.is_some(),
                party.calls.len(),
            )
        }) else {
            return;
        };
        let at_work = self.out[pick].leg == Leg::Out && self.out[pick].settled.is_none();
        if errand == Errand::Villager && !at_work {
            let Some(pick) = crew.into_iter().min_by_key(|&i| self.out[i].again.len()) else {
                return;
            };
            let party = &mut self.out[pick];
            party.again.insert(id.clone());
            party.owes = Some(party.owes.unwrap_or(true));
            return;
        }
        let party = &mut self.out[pick];
        if party.leg != Leg::Out || party.settled.is_some() {
            if party.leg != Leg::Out {
                party.since = tick;
            }
            party.head_out();
        }
        party.calls.insert(id.clone());
    }

    /// A call's result lands: record what it earned and call its party home
    /// once nothing else keeps it out.
    pub(crate) fn settle(&mut self, id: &ToolEventId, passed: bool, tick: u64) {
        let Some(call) = self.calls.remove(id) else {
            return;
        };
        let record = &mut self.record;
        match call.deed {
            Deed::Forge if passed => {
                self.forged.insert(call.object);
                record.blades = self.forged.len() as u32;
            }
            Deed::Study if passed => {
                self.studied.insert(call.object);
                record.books = self.studied.len() as u32;
            }
            Deed::Trial => {
                record.pennants.push(passed);
                let excess = record.pennants.len().saturating_sub(PENNANTS);
                record.pennants.drain(..excess);
            }
            Deed::Seek if passed => {
                record.stooks += 1;
                // The harvest goes up the road from whoever worked it.
                let from = self
                    .out
                    .iter()
                    .find(|p| p.calls.contains(id))
                    .map_or_else(|| feet(field_gate()), Party::at);
                self.send(Party::bearer(Errand::Messenger, from, tick));
            }
            // A sack counts once it reaches the granary (see `step`).
            Deed::Errand if passed => match self
                .out
                .iter_mut()
                .find(|p| p.calls.contains(id) || p.again.contains(id))
            {
                Some(party) => party.load += 1,
                None => record.sacks += 1,
            },
            Deed::Memory if passed => record.candles += 1,
            Deed::Research if passed => {
                record.stars += 1;
                self.send(Party::bearer(Errand::Owl, dome(), tick));
            }
            _ => {}
        }
        if let Some(party) = self.out.iter_mut().find(|party| party.again.contains(id)) {
            party.again.remove(id);
            party.owes = party.owes.map(|ok| ok && passed);
            return;
        }
        if let Some(party) = self.out.iter_mut().find(|party| party.calls.contains(id)) {
            party.calls.remove(id);
            party.failed |= !passed;
            if party.calls.is_empty() {
                party.settled = Some(!party.failed);
            }
        }
    }

    /// The turn is over: calls still out will never report, so everyone at
    /// work comes home empty-handed and nothing is recorded for them.
    /// Findings already on the road still arrive.
    pub(crate) fn abandon(&mut self) {
        self.calls.clear();
        let perch = raven_perch(self.record.ravens);
        for party in &mut self.out {
            // Errands that finished still count, delivered or not.
            self.record.sacks += std::mem::take(&mut party.load);
            party.again.clear();
            party.owes = None;
            party.calls.clear();
            party.settled = None;
            if !matches!(party.leg, Leg::Home { .. }) {
                party.head_home(None, perch);
            }
        }
    }

    /// Put a finding on the road, unless the road already carries its fill.
    fn send(&mut self, bearer: Party) {
        let on_road = self
            .out
            .iter()
            .filter(|p| p.errand == bearer.errand)
            .count();
        if on_road < ON_THE_ROAD {
            self.out.push(bearer);
        }
    }

    /// Advance everyone out one world tick.
    pub(crate) fn step(&mut self, tick: u64) {
        let perch = raven_perch(self.record.ravens);
        let mut perched = 0;
        let mut delivered = 0;
        self.out.retain_mut(|party| {
            let (out, sacks) = party.step(tick, perch);
            delivered += sacks;
            if !out
                && party.errand == Errand::Raven
                && party.leg
                    == (Leg::Home {
                        verdict: Some(true),
                    })
            {
                perched += 1;
            }
            out
        });
        self.record.ravens += perched;
        self.record.sacks += delivered;
    }

    /// A trial is underway: the wagon is out and nothing has come back yet.
    pub(crate) fn trial_underway(&self) -> bool {
        self.out
            .iter()
            .any(|party| party.errand == Errand::Wagon && party.leg == Leg::Out)
    }

    /// Research is out: the Observatory's glass sweeps the sky.
    pub(crate) fn stargazing(&self) -> bool {
        self.calls.values().any(|call| call.deed == Deed::Research)
    }

    pub(crate) fn wayfarers(&self) -> Vec<Wayfarer> {
        self.out.iter().filter_map(Party::shown).collect()
    }

    pub(crate) fn record(&self) -> &Record {
        &self.record
    }

    /// Ticks since the anvil was struck, while its sparks still fly.
    pub(crate) fn sparks(&self, tick: u64) -> Option<u32> {
        let since = tick.checked_sub(self.struck?)?;
        (since < SPARKS).then_some(since as u32)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/overworld__deeds_tests.rs"]
mod tests;
