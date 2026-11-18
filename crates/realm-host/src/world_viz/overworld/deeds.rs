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
use super::live::route;
use super::map::{Place, TILE, place_px, place_tile};
use crate::telemetry::ToolEventId;
use angel_delve::world::kingdom::step_cost;

pub use angel_delve::render::overworld::deeds::{
    Errand, PENNANTS, ROOST, Record, SPARKS, Wayfarer,
};

const VERDICT_HOLD: u64 = 100;
const FLIGHT: u64 = 120;
const WORK: u64 = 80;
const ON_THE_ROAD: usize = 4;
const DELIVER: u64 = 30;
const HOME_BEAT: u64 = 16;

trait Errands: Sized {
    fn of(deed: Deed) -> Option<Self>;
    fn crew(self) -> usize;
    fn pace(self) -> f32;
}

impl Errands for Errand {
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

    fn crew(self) -> usize {
        match self {
            Errand::Hand => 3,
            Errand::Villager => 4,
            _ => 1,
        }
    }

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

#[derive(Clone, Debug)]
struct Call {
    deed: Deed,
    object: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Leg {
    Out,
    Verdict { passed: bool, until: u64 },
    Home { verdict: Option<bool> },
    Inside { until: u64, next: After },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum After {
    Home(Option<bool>),
    Out,
}

#[derive(Clone, Debug)]
struct Party {
    errand: Errand,
    slot: usize,
    calls: BTreeSet<ToolEventId>,
    settled: Option<bool>,
    failed: bool,
    x: f32,
    y: f32,
    path: VecDeque<(f32, f32)>,
    leg: Leg,
    facing_left: bool,
    since: u64,
    round: usize,
    again: BTreeSet<ToolEventId>,
    owes: Option<bool>,
    load: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Deeds {
    calls: BTreeMap<ToolEventId, Call>,
    out: Vec<Party>,
    record: Record,
    forged: BTreeSet<String>,
    studied: BTreeSet<String>,
    struck: Option<u64>,
}

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

pub fn raven_perch(n: u32) -> (f32, f32) {
    let (x, base) = ROOST;
    let (x, y) = place_px(x + 4 + (n % 5) as i32 * 5, base - 9);
    (x as f32, y as f32)
}

fn rookery_door() -> (f32, f32) {
    feet(stand(Place::Rookery))
}

fn wheel(since: u64, tick: u64) -> (f32, f32) {
    let (px, py) = raven_perch(0);
    let a = (tick.saturating_sub(since)) as f32 * 0.06;
    (px + 26.0 * a.cos(), py - 22.0 + 12.0 * a.sin())
}

fn wagon_bay() -> (i32, i32) {
    let (sx, sy) = stand(Place::Lists);
    [(sx - 2, sy), (sx - 1, sy + 1), (sx, sy + 1)]
        .into_iter()
        .find(|&(x, y)| step_cost(x, y).is_some())
        .unwrap_or((sx, sy))
}

fn gate_edge() -> ((i32, i32), (f32, f32)) {
    let (_, gy) = stand(Place::Gatehouse);
    let (_, y) = feet((0, gy));
    ((0, gy), (-2.0 * TILE as f32, y))
}

fn field_gate() -> (i32, i32) {
    place_tile(39, 27)
}

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

fn cottage_door(slot: usize) -> (i32, i32) {
    const DOORS: [(i32, i32); 4] = [(19, 26), (26, 26), (22, 26), (29, 26)];
    let (tx, ty) = DOORS[slot % DOORS.len()];
    place_tile(tx, ty)
}

fn granary_door() -> (i32, i32) {
    place_tile(28, 31)
}

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

pub fn dome() -> (f32, f32) {
    let (tx, ty, tw, _) = Place::Observatory.footprint();
    let (x, y) = place_px(tx * TILE + tw * TILE / 2, ty * TILE + 4);
    (x as f32, y as f32)
}

fn scriptorium() -> (i32, i32) {
    stand(Place::Scriptorium)
}

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

    fn ready(&self, tick: u64) -> bool {
        match self.errand {
            Errand::Raven => tick >= self.since + FLIGHT,
            Errand::Hand | Errand::Monk => tick >= self.since + WORK,
            Errand::Villager => self.path.is_empty(),
            _ => true,
        }
    }

    fn step(&mut self, tick: u64, perch: (f32, f32)) -> (bool, u32) {
        let mut delivered = 0;
        if self.leg == Leg::Out
            && let Some(passed) = self.settled
            && self.ready(tick)
        {
            match self.errand {
                Errand::Villager if self.load > 0 => {
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
            self.leg = Leg::Inside {
                until: tick + HOME_BEAT,
                next: After::Out,
            };
        }
        (true, delivered)
    }

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

impl Deeds {
    pub fn destination(&self, id: &ToolEventId) -> Option<Place> {
        Some(match self.calls.get(id)?.deed {
            Deed::Study => Place::Scriptorium,
            Deed::Seek => Place::Fields,
            Deed::Forge => Place::Smithy,
            Deed::Trial => Place::Lists,
            Deed::Chronicle | Deed::Seal => Place::Rookery,
            Deed::Dispatch => Place::Gatehouse,
            Deed::Council => Place::RoundTable,
            Deed::Memory => Place::Chapel,
            Deed::Research => Place::Observatory,
            Deed::Errand => Place::Village,
        })
    }

    pub fn begin(&mut self, id: &ToolEventId, name: &str, args: &str, tick: u64) {
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

    pub fn settle(&mut self, id: &ToolEventId, passed: bool, tick: u64) {
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
                let from = self
                    .out
                    .iter()
                    .find(|p| p.calls.contains(id))
                    .map_or_else(|| feet(field_gate()), Party::at);
                self.send(Party::bearer(Errand::Messenger, from, tick));
            }
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

    pub fn abandon(&mut self) {
        self.calls.clear();
        let perch = raven_perch(self.record.ravens);
        for party in &mut self.out {
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

    pub fn step(&mut self, tick: u64) {
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

    pub fn trial_underway(&self) -> bool {
        self.out
            .iter()
            .any(|party| party.errand == Errand::Wagon && party.leg == Leg::Out)
    }

    pub fn stargazing(&self) -> bool {
        self.calls.values().any(|call| call.deed == Deed::Research)
    }

    pub fn wayfarers(&self) -> Vec<Wayfarer> {
        self.out.iter().filter_map(Party::shown).collect()
    }

    pub fn record(&self) -> &Record {
        &self.record
    }

    pub fn sparks(&self, tick: u64) -> Option<u32> {
        let since = tick.checked_sub(self.struck?)?;
        (since < SPARKS).then_some(since as u32)
    }
}

#[cfg(test)]
#[path = "../../../tests/world_viz/overworld__deeds_tests.rs"]
mod tests;
