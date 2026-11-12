//! The race itself: cars, physics, hazards, jumps, forks, laps, rival pilots
//! and the cup. Std and serde only, driven by `Drive`; `knight_racers.rs`
//! turns the Delve's `Input` into a `Drive`.
use super::course::*;
use super::flow::Flow;
use super::pods::{pods, seat_pod, PodSpec};
use super::tune::Tune;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A tuning number. In tests `ANGEL_LAW_KNOBS="NAME=1.5,OTHER=2"` overrides it so a sweep needs no
/// rebuild; the game always runs the default.
#[cfg(test)]
pub fn knob(name: &str, default: f32) -> f32 {
    static KNOBS: std::sync::OnceLock<std::collections::HashMap<String, f32>> = std::sync::OnceLock::new();
    KNOBS.get_or_init(|| std::env::var("ANGEL_LAW_KNOBS").unwrap_or_default().split(',').filter_map(|kv| { let (k, v) = kv.split_once('=')?; Some((k.trim().to_string(), v.trim().parse().ok()?)) }).collect())
        .get(name).copied().unwrap_or(default)
}
#[cfg(not(test))]
#[inline(always)]
pub fn knob(_name: &str, default: f32) -> f32 { default }

#[path = "engines.rs"]
pub mod engines;
#[path = "contact.rs"]
pub mod contact;
#[path = "pilot.rs"]
pub mod pilot;

pub const HZ: f32 = 30.;
pub const SEATS: u32 = 6;
/// Ticks of grid countdown before the cars may move.
pub const COUNTDOWN: u32 = 90;
/// Grid slots: distance behind the line and lateral offset. Two abreast, the right lane half a row
/// (12 m) back, rows 24 m apart so the lens 22 m behind a pod never has another between it and its pod,
/// and 11.2 m between lanes on a road 24 m wide (the widest pod is 5.6 m across).
pub const GRID: [(f32, f32); 6] = [(-6., -5.6), (-18., 5.6), (-30., -5.6), (-42., 5.6), (-54., -5.6), (-66., 5.6)];
/// Points for first to sixth in a cup race.
pub const POINTS: [u32; 6] = [10, 7, 5, 3, 2, 1];
const GRAVITY: f32 = 14.;

/// What a seat presses: steer (-1 right, +1 left), gas, brake, boost, and E.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drive { pub steer: i8, pub gas: bool, pub brake: bool, pub boost: bool, pub interact: bool }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Car {
    /// Distance along the course, metres from the line (negative on the grid).
    pub d: f32,
    /// Across the road, metres left of the centreline of the road it is on.
    pub y: f32,
    pub v: f32, pub vy: f32, pub boost: f32,
    pub finish: Option<f32>,
    /// Presentation follows the actual simulation, not the held key. Defaults
    /// keep races saved before the instrument/boost upgrade loadable.
    #[serde(default)]
    pub boosting: bool,
    #[serde(default)]
    pub boost_locked: bool,
    #[serde(default)]
    pub impact: u32,
    /// A CPU rival on a solo grid. A joined knight clears this and is never driven for them.
    #[serde(default)]
    pub cpu: bool,
    /// Index into `pods()`.
    #[serde(default)]
    pub pod: u8,
    /// 0 on the main line, n on the n-th fork of the course.
    #[serde(default)]
    pub route: u8,
    /// Airborne over a gap or a landing: metres above the road (below it, falling).
    #[serde(default)]
    pub lift: f32,
    #[serde(default)]
    pub vz: f32,
    #[serde(default)]
    pub flying: bool,
    /// Times this car has dropped into a chasm.
    #[serde(default)]
    pub fell: u32,
    /// Which way the nose points, in radians off the road's direction (0 along it, pi back
    /// against it). A pod only turns about held still on the brake; at speed it straightens.
    #[serde(default)]
    pub yaw: f32,
    /// Heat of the two nacelles, 0 cold to 1 blown. Index 0 is the left engine as the picture
    /// shows it (the pod's lesser-`y` side), index 1 the right. See `engines`.
    #[serde(default)]
    pub heat: [f32; 2],
    /// Wear on each nacelle: damage the squires patch when the pod is off the gas. An engine
    /// cannot cool below its wear.
    #[serde(default)]
    pub wear: [f32; 2],
    /// Ticks left sitting out a blowout, and which nacelle blew (1 the first, 2 the second).
    #[serde(default)]
    pub crash: u32,
    #[serde(default)]
    pub blown_side: u8,
    /// Blowouts this race.
    #[serde(default)]
    pub blowouts: u32,
    /// How deep the pod sits in another pod's wake, 0..1: it builds in the wake and fades slowly
    /// after the pod leaves it (the slingshot). `wake` is the seat whose wake it is, 0 for none.
    #[serde(default)]
    pub draft: f32,
    #[serde(default)]
    pub wake: u8,
    /// A sideways velocity from a slam or a shove, which steering cannot cancel; it dies away.
    #[serde(default)]
    pub kick: f32,
    /// Ticks left of a slam's marker, the side it came from (+1 left, -1 right, 0 front or rear),
    /// and slams so far this race.
    #[serde(default)]
    pub slam: u32,
    #[serde(default)]
    pub slam_side: i8,
    #[serde(default)]
    pub slams: u32,
    /// The pod's place last tick, ticks left of the "passed a rival" cheer, and places gained so far.
    #[serde(default)]
    pub place: u8,
    #[serde(default)]
    pub cheer: u32,
    #[serde(default)]
    pub passes: u32,
    /// The start: ticks the gas has been held on the grid, and how the flag found it:
    /// 1 a rocket start, -1 flooded engines, 0 a plain start; the ticks that start has left to run.
    #[serde(default)]
    pub revs: u32,
    #[serde(default)]
    pub launch: i8,
    #[serde(default)]
    pub launch_t: u32,
    /// The parts the knight fitted, or the class a rival races in: multiples of the pod's spec.
    #[serde(default)]
    pub tune: Tune,
}
impl Car {
    fn grid(slot: usize, id: u32) -> Self {
        Self { d: GRID[slot].0, y: GRID[slot].1, v: 0., vy: 0., boost: 1., finish: None, boosting: false, boost_locked: false, impact: 0, cpu: false, pod: seat_pod(id), route: 0, lift: 0., vz: 0., flying: false, fell: 0, yaw: 0.,
            heat: [0.; 2], wear: [0.; 2], crash: 0, blown_side: 0, blowouts: 0, draft: 0., wake: 0, kick: 0., slam: 0, slam_side: 0, slams: 0, place: 0, cheer: 0, passes: 0, revs: 0, launch: 0, launch_t: 0, tune: Tune::STOCK }
    }
    fn valid(&self, course: &Course) -> bool {
        let line = course.lap() * course.laps() as f32;
        [self.d, self.y, self.v, self.vy, self.boost, self.lift, self.vz, self.yaw, self.draft, self.kick].iter().all(|n| n.is_finite())
            && self.heat.iter().chain(&self.wear).all(|h| h.is_finite() && (0. ..=1.).contains(h))
            && self.crash <= engines::BLOWOUT_TICKS
            && (0. ..=1.).contains(&self.draft)
            && self.kick.abs() <= 60.
            && self.slam <= contact::SLAM_TICKS
            && self.cheer <= 90
            && self.yaw.abs() <= std::f32::consts::PI + 1e-3
            && self.d >= -100. && self.d <= line + 100.
            && self.y.abs() <= 30.
            && (0. ..=72.).contains(&self.v)
            && (0. ..=1.).contains(&self.boost)
            && self.impact <= 12
            && (-16. ..=60.).contains(&self.lift)
            && self.vz.abs() <= 80.
            && (self.route as usize) <= course.forks().len()
            && (self.pod as usize) < pods().len()
            && self.finish.is_none_or(|t| t.is_finite() && t >= 0.)
    }
    /// The pod's spec with this car's tune on it: the numbers it drives by.
    pub fn spec(&self) -> PodSpec { pods()[self.pod as usize % pods().len()].tuned(&self.tune) }
    /// The lap this car is on, counting from 1.
    pub fn lap(&self, course: &Course) -> u32 { ((self.d.max(0.) / course.lap()) as u32 + 1).min(course.laps()) }
    /// Where on the lap the car is, in `0..lap`.
    pub fn s(&self, course: &Course) -> f32 { self.d.rem_euclid(course.lap()) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub cars: BTreeMap<u32, Car>,
    pub ticks: u32,
    /// After the flag, E starts a fresh grid without leaving the room.
    pub finished: bool,
    /// The course being raced. Races saved before the courses existed ran the proving circuit.
    #[serde(default)]
    pub course: CourseId,
    /// Which race of the cup this is, counting from 0.
    #[serde(default)]
    pub round: u8,
    /// Cup points by seat across the races so far.
    #[serde(default)]
    pub points: BTreeMap<u32, u32>,
    /// Whether this race's points are already in `points`.
    #[serde(default)]
    pub scored: bool,
    /// The game around the race: modes, menus, the cup, the purse (`flow.rs`).
    #[serde(default)]
    pub flow: Flow,
    /// Laps this race runs, when fewer than the course's own (0: the course's). The crossroads
    /// between floors runs one.
    #[serde(default)]
    pub laps: u32,
    /// Metres this race runs when it stops short of its laps (0: the laps). The crossroads runs
    /// a quarter of one lap.
    #[serde(default)]
    pub length: f32,
    /// Ticks the race runs after the flag before it is called (0: no limit). The screen counts
    /// it down.
    #[serde(default)]
    pub limit: u32,
}

impl Session {
    /// An empty session on the first course of the cup.
    pub fn blank() -> Self {
        Self { cars: BTreeMap::new(), ticks: 0, finished: false, course: cup()[0], round: 0, points: BTreeMap::new(), scored: false, flow: Flow::default(), laps: 0, length: 0., limit: 0 }
    }
    /// The same, on a chosen course.
    pub fn blank_on(course: CourseId) -> Self {
        Self { course, round: cup().iter().position(|c| *c == course).unwrap_or(0) as u8, ..Self::blank() }
    }
    pub fn course(&self) -> &'static Course { course(self.course) }
    /// The laps this race runs: the course's, or fewer when the session says so.
    pub fn laps(&self) -> u32 {
        let own = self.course().laps();
        if self.laps == 0 { own } else { self.laps.min(own) }
    }
    /// Metres from the line to the flag: the laps, or the shorter length the session sets.
    pub fn distance(&self) -> f32 {
        let laps = self.course().lap() * self.laps() as f32;
        if self.length > 0. { self.length.min(laps) } else { laps }
    }
    /// Seconds before the race is called, for a race with a limit (none on the grid's count).
    pub fn left(&self) -> Option<f32> {
        (self.limit > 0).then(|| (COUNTDOWN + self.limit).saturating_sub(self.ticks.max(COUNTDOWN)) as f32 / HZ)
    }
    /// The limit has run out.
    pub fn time_up(&self) -> bool { self.left() == Some(0.) }
    /// Keep only knights still inside the race. Waiting outside at the
    /// kingdom gate must not remain a ghost racer in the finish condition.
    /// CPU rivals survive a solo join; a real knight taking that seat clears the flag.
    pub fn join_ids(&mut self, ids: impl IntoIterator<Item = u32>) {
        let ids: BTreeSet<_> = ids.into_iter().filter(|id| (1..=SEATS).contains(id)).collect();
        self.cars.retain(|id, car| ids.contains(id) || car.cpu);
        for id in ids {
            match self.cars.entry(id) {
                std::collections::btree_map::Entry::Occupied(mut e) => e.get_mut().cpu = false,
                std::collections::btree_map::Entry::Vacant(e) => { e.insert(Car::grid((id - 1) as usize, id)); }
            }
        }
        self.finished = !self.cars.is_empty() && self.cars.values().all(|c| c.finish.is_some());
    }
    /// A lone knight gets five rivals. Two or more humans race each other; the ghosts leave.
    pub fn fill_rivals(&mut self, humans: usize) {
        if humans != 1 {
            self.cars.retain(|_, car| !car.cpu);
            return;
        }
        for id in 1..=SEATS {
            if self.cars.contains_key(&id) { continue; }
            let mut car = Car::grid((id - 1) as usize, id);
            car.cpu = true;
            self.cars.insert(id, car);
        }
    }
    pub fn valid(&self) -> bool {
        let course = self.course();
        self.cars.len() <= SEATS as usize
            && self.cars.iter().all(|(id, car)| (1..=SEATS).contains(id) && car.valid(course))
            && (!self.finished || self.cars.values().all(|c| c.finish.is_some()))
            && self.points.keys().all(|id| (1..=SEATS).contains(id))
    }

    /// Seats in running order: finishers by time, then everyone else by distance.
    pub fn order(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.cars.keys().copied().collect();
        ids.sort_by(|a, b| {
            let (ca, cb) = (&self.cars[a], &self.cars[b]);
            match (ca.finish, cb.finish) {
                (Some(x), Some(y)) => x.total_cmp(&y).then(a.cmp(b)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => cb.d.total_cmp(&ca.d).then(a.cmp(b)),
            }
        });
        ids
    }
    /// A seat's place in the running order, 1 for the leader.
    pub fn place(&self, id: u32) -> usize { self.order().iter().position(|n| *n == id).map_or(self.cars.len(), |p| p + 1) }
    /// A seat takes a different pod, on the grid only.
    pub fn choose_pod(&mut self, id: u32, pod: u8) {
        if self.ticks > 0 && !self.finished { return; }
        if let Some(c) = self.cars.get_mut(&id) { c.pod = pod % pods().len() as u8; }
    }
    /// Cup standings, best first: seat and points.
    pub fn standings(&self) -> Vec<(u32, u32)> {
        let mut rows: Vec<(u32, u32)> = self.cars.keys().map(|id| (*id, self.points.get(id).copied().unwrap_or(0))).collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        rows
    }
    /// The last race of the cup is run and won.
    pub fn cup_over(&self) -> bool { self.finished && cup().last() == Some(&self.course) }

    fn score(&mut self) {
        if self.scored || !self.finished { return; }
        self.scored = true;
        for (place, id) in self.order().into_iter().enumerate() {
            *self.points.entry(id).or_insert(0) += POINTS[place.min(POINTS.len() - 1)];
        }
    }
    /// E at the flag: the next course of the cup, or the practice run again; after the
    /// last course the cup starts over.
    fn restart(&mut self) {
        if cup().contains(&self.course) {
            let next = self.round as usize + 1;
            if next < cup().len() { self.round = next as u8; } else { self.round = 0; self.points.clear(); }
            self.course = cup()[self.round as usize];
        }
        for (id, car) in &mut self.cars {
            let (cpu, pod) = (car.cpu, car.pod);
            *car = Car::grid((*id - 1) as usize, *id);
            car.cpu = cpu;
            car.pod = pod;
        }
        self.ticks = 0;
        self.finished = false;
        self.scored = false;
    }

    /// What every CPU rival presses this tick: each pilot reads the whole field (see `pilot`).
    pub fn bot_drives(&self) -> BTreeMap<u32, Drive> {
        let course = self.course();
        let leader = self.cars.values().filter(|c| c.finish.is_none()).map(|c| c.d).fold(f32::MIN, f32::max);
        self.cars.iter().filter(|(_, c)| c.cpu).map(|(id, c)| {
            let mut input = pilot::drive(course, &self.cars, *id, self.ticks, pilot::CPU_SLOP);
            // A rival a long way behind is still in the race, not deleted by the draw distance:
            // it presses on, as far as its engines allow.
            if leader - c.d > 400. && c.finish.is_none() && self.ticks > COUNTDOWN && c.hottest() < 0.4 && !c.blown() { input.boost = true; input.gas = true; input.brake = false; }
            (*id, input)
        }).collect()
    }

    /// Every car driven by a pilot, human seats too: for tests and demos. The pilots drive them
    /// as CPU rivals do, with the CPU's slips; see `drive_clean` for a driver with none.
    pub fn bot_drives_all(&self) -> BTreeMap<u32, Drive> {
        let mut all = self.clone();
        for c in all.cars.values_mut() { c.cpu = true; }
        all.bot_drives()
    }

    /// What a pilot with no slips (a good human, say) presses for seat `id`.
    pub fn drive_clean(&self, id: u32) -> Drive { pilot::drive(self.course(), &self.cars, id, self.ticks, 0.) }

    /// The grid: the clock runs, the cars wait, and the gas is timed. Held in the last
    /// `LAUNCH_WINDOW` ticks it is a rocket start, held earlier it floods the engines.
    fn on_the_grid(&mut self, inputs: &BTreeMap<u32, Drive>) {
        for (id, c) in &mut self.cars {
            let i = inputs.get(id).copied().unwrap_or_default();
            if i.gas || i.boost { c.revs = c.revs.saturating_add(1); } else { c.revs = 0; }
        }
    }

    /// The flag drops: each car's start is judged by how long its gas was held.
    fn flag(&mut self) {
        for c in self.cars.values_mut() {
            (c.launch, c.launch_t) = match c.revs {
                0 => (0, 0),
                1..=LAUNCH_WINDOW => (1, ROCKET_TICKS),
                _ => { engines::dump(c, None, FLOOD_HEAT, 0.); (-1, FLOOD_TICKS) }
            };
            c.revs = 0;
        }
    }

    /// How hard each CPU rival is let to run: a rubber band, mild and capped. A rival behind the
    /// pod the race is run against (the best human, or the leader when there is none) may run up
    /// to `RUBBER_UP` over its pod's speed, one ahead of it down to `RUBBER_DOWN` under: never
    /// enough to bring a good run back, enough to keep the field in sight of itself.
    pub fn paces(&self) -> BTreeMap<u32, f32> {
        let running = || self.cars.values().filter(|c| c.finish.is_none());
        let human = running().filter(|c| !c.cpu).map(|c| c.d).fold(f32::MIN, f32::max);
        let reference = if human > f32::MIN { human } else { running().map(|c| c.d).fold(f32::MIN, f32::max) };
        self.cars.iter().filter(|(_, c)| c.cpu && c.finish.is_none()).map(|(id, c)| {
            let assist = ((reference - c.d) / knob("RUBBER_SPAN", RUBBER_SPAN)).clamp(-1., 1.);
            (*id, knob("CPU_SKILL", CPU_SKILL) * (1. + if assist > 0. { knob("RUBBER_UP", RUBBER_UP) * assist } else { knob("RUBBER_DOWN", RUBBER_DOWN) * assist }))
        }).collect()
    }

    pub fn step_drive(&mut self, inputs: &BTreeMap<u32, Drive>) {
        if self.finished {
            if self.cars.keys().any(|id| inputs.get(id).is_some_and(|i| i.interact)) { self.restart(); }
            return;
        }
        self.ticks = self.ticks.saturating_add(1);
        // A short grid countdown, shared by all seats.
        if self.ticks <= COUNTDOWN { self.on_the_grid(inputs); return; }
        if self.ticks == COUNTDOWN + 1 { self.flag(); }
        let course = self.course();
        let lap = course.lap();
        let dt = 1. / HZ;
        let time = (self.ticks - COUNTDOWN) as f32 * dt;
        let tick = self.ticks - COUNTDOWN;
        let finish_at = self.distance();
        // The wake each pod rides this tick, where every pod was, and how hard the rivals may run.
        let wakes: BTreeMap<u32, (f32, u32)> = self.cars.keys().map(|&id| (id, contact::wake_of(lap, &self.cars, id))).collect();
        let before: BTreeMap<u32, (f32, f32)> = self.cars.iter().map(|(id, c)| (*id, (c.d, c.y))).collect();
        let paces = self.paces();
        for (id, c) in &mut self.cars {
            c.impact = c.impact.saturating_sub(1);
            c.slam = c.slam.saturating_sub(1);
            c.cheer = c.cheer.saturating_sub(1);
            if c.finish.is_some() { c.boosting = false; continue; }
            let input = inputs.get(id).copied().unwrap_or_default();
            drive_car(course, c, input, tick, wakes.get(id).copied().unwrap_or((0., 0)), paces.get(id).copied().unwrap_or(1.));
            if c.d >= finish_at {
                c.finish = Some(time - (c.d - finish_at) / c.v.max(1.));
                c.d = finish_at;
                c.v = 0.;
                c.lift = 0.; c.vz = 0.; c.flying = false;
            }
        }
        contact::resolve(lap, &mut self.cars, &before);
        // A shove never puts a pod through the wall.
        for c in self.cars.values_mut().filter(|c| c.finish.is_none()) {
            let fr = course.frame(c.route, c.d);
            c.y = c.y.clamp(-fr.wr, fr.wl);
        }
        // Places: a pod that gains one after the first seconds has passed someone.
        let racing = self.ticks > COUNTDOWN + 90;
        for (n, id) in self.order().into_iter().enumerate() {
            let c = self.cars.get_mut(&id).unwrap();
            let place = n as u8 + 1;
            if racing && c.finish.is_none() && c.place != 0 && place < c.place { c.cheer = 45; c.passes += 1; }
            c.place = place;
        }
        self.finished = !self.cars.is_empty() && self.cars.values().all(|c| c.finish.is_some());
        self.score();
    }
}

/// Distance between two positions on a lap, either way round.
fn along(lap: f32, d: f32, s: f32) -> f32 {
    let mut ds = (d - s).rem_euclid(lap);
    if ds > lap * 0.5 { ds = lap - ds; }
    ds
}

fn hits(lap: f32, c: &Car, b: &Hazard) -> bool {
    along(lap, c.d, b.s) < b.half_l + 1.7 && (c.y - b.y).abs() < b.half_w + 1.05
}

fn on_strip(lap: f32, d: f32, y: f32, p: &Strip) -> bool {
    let s = d.rem_euclid(lap);
    s >= p.s0 && s <= p.s1 && (y - p.y).abs() <= p.half_w
}

/// The hazards a car on `route` can meet.
fn hazards_on<'a>(course: &'a Course, route: u8) -> &'a [Hazard] {
    match route.checked_sub(1).and_then(|i| course.forks().get(i as usize)) {
        Some(f) => &f.hazards,
        None => course.hazards(),
    }
}
fn strips_on<'a>(course: &'a Course, route: u8) -> &'a [Strip] {
    match route.checked_sub(1).and_then(|i| course.forks().get(i as usize)) {
        Some(f) => &f.strips,
        None => course.strips(),
    }
}

/// A pod turns about only below this speed, braking; its rate in radians a second, and how
/// fast it straightens once it is under way.
const TURN_SPEED: f32 = 5.;
const TURN_RATE: f32 = 2.8;
const STRAIGHTEN: f32 = 2.5;
/// A pod reversing down the grid stops here, behind the back row.
const BACK_WALL: f32 = -90.;

/// The draft: how fast it builds in a wake and fades out of it (a second), and what it is worth at
/// full: metres a second over the pod's top speed, and a share more push.
const DRAFT_RISE: f32 = 1.6;
const DRAFT_FALL: f32 = 0.55;
pub const DRAFT_TOP: f32 = 5.5;
const DRAFT_PUSH: f32 = 0.55;
/// The rubber band: a CPU rival's speed may be raised by `RUBBER_UP` or lowered by `RUBBER_DOWN` of
/// its pod's, over a `RUBBER_SPAN` of metres behind or ahead of the pod the race is run against.
pub const RUBBER_UP: f32 = 0.04;
/// What a CPU rival does with its pod: a share of its paper speed. A clean human run on the same
/// pods beats the field by a few seconds a race instead of trading places with it.
pub const CPU_SKILL: f32 = 0.975;
pub const RUBBER_DOWN: f32 = 0.025;
const RUBBER_SPAN: f32 = 400.;
/// The start. Gas held for the last `LAUNCH_WINDOW` ticks of the grid is a rocket start: twice the push
/// for `ROCKET_TICKS`. Held longer it floods the engines: a third of the push for `FLOOD_TICKS`, and heat.
pub const LAUNCH_WINDOW: u32 = 12;
const ROCKET_TICKS: u32 = 36;
const FLOOD_TICKS: u32 = 40;
const FLOOD_HEAT: f32 = 0.12;
/// Heat a nacelle takes in a second grinding along a wall, and from a slag block or a flame jet.
const WALL_GRIND: f32 = 0.10;
const SLAG_HEAT: f32 = 0.20;
const SLAG_WEAR: f32 = 0.07;
const JET_HEAT: f32 = 0.14;
const JET_WEAR: f32 = 0.03;

/// One car, one tick. `wake` is how deep a pod's wake this pod is in (0..1) and whose it is; `pace`
/// scales the pod's top speed (1 for a human).
fn drive_car(course: &Course, c: &mut Car, input: Drive, tick: u32, wake: (f32, u32), pace: f32) {
    let dt = 1. / HZ;
    let lap = course.lap();
    let pod = c.spec();
    // A blown pod skids and then sits out; it presses nothing.
    let sitting = c.crash > 0;
    if sitting { engines::sit_out(c); }
    let input = if sitting { Drive::default() } else { input };
    // The draft builds in a wake and lingers after the pod leaves it: the slingshot.
    c.wake = if wake.0 > 0. { wake.1 as u8 } else { 0 };
    if wake.0 > c.draft { c.draft = (c.draft + DRAFT_RISE * dt).min(wake.0); } else { c.draft = (c.draft - knob("DRAFT_FALL", DRAFT_FALL) * dt).max(wake.0); }
    if sitting { c.draft = 0.; }
    let steer = f32::from(input.steer.clamp(-1, 1));
    let brake = input.brake;
    // A depleted engine must cool to a useful charge. Without this
    // latch, holding Space alternated boost/recharge every other tick.
    if c.boost_locked && c.boost >= 0.32 { c.boost_locked = false; }
    let boosting = input.boost && c.boost > 0. && !brake && !c.boost_locked;
    c.boosting = boosting;
    let gas = input.gas || boosting;
    if boosting { c.boost = (c.boost - 0.30 * dt).max(0.); } else { c.boost = (c.boost + 0.11 * dt).min(1.); }
    if c.boost <= 0. { c.boost_locked = true; }
    // Hot engines give less push and a lower top speed; the draft adds to both; a start is a rocket or a flood.
    let launch = if c.launch_t > 0 { c.launch_t -= 1; if c.launch > 0 { 2. } else { 0.33 } } else { 1. };
    let top = (if boosting { pod.boost } else { pod.top }) * c.cap_share() * pace + knob("DRAFT_TOP", DRAFT_TOP) * c.draft;
    let fr = course.frame(c.route, c.d);
    if c.flying {
        // Momentum only: no thrust, no grip. Drag still bleeds speed.
        c.v = (c.v - (0.004 * c.v * c.v) * dt).max(0.);
    } else {
        if brake { c.v = (c.v - 46. * dt).max(0.); }
        else if gas && !sitting {
            if c.v < top {
                let push = (if boosting { 62.4 } else { 26. }) * pod.accel * c.push() * launch * (1. + knob("DRAFT_PUSH", DRAFT_PUSH) * c.draft);
                c.v += push * (1. - c.v / top) * dt;
            }
        }
        else if !sitting { c.v = (c.v - (3. + 0.004 * c.v * c.v) * dt).max(0.); }
        let padded = strips_on(course, c.route).iter().any(|p| on_strip(lap, c.d, c.y, p));
        let cap = if padded { (top + 7.).min(64.).max(top) } else if fr.g < -0.015 { (top + 4.).min(66.).max(top) } else { top };
        if !brake && !sitting { c.v = (c.v - fr.g * 42. * dt).clamp(0., cap); }
        if c.v > cap && !sitting { c.v = (c.v - 16. * dt).max(cap); }
    }
    // Steering and the bend.
    let mut slick = 1.;
    for h in hazards_on(course, c.route) {
        if h.kind == HazardKind::Slick && hits(lap, c, h) { slick = 0.45; }
    }
    let air = if c.flying { 0.3 } else { 1. };
    let lat = 13. * pod.grip * slick * air * (0.4 + 0.6 * (c.v / 16.).min(1.)) * if boosting { 0.8 } else { 1. };
    let cmd = steer * lat;
    c.vy += (cmd - c.vy).clamp(-90. * dt * pod.grip.max(0.7), 90. * dt * pod.grip.max(0.7));
    // Banking into the bend takes some of the push off the car.
    let banked = (fr.bank * fr.k.signum() / 0.28).clamp(0., 0.55);
    let drift = 0.30 * c.v * c.v * fr.k * (1. - banked) / pod.grip / slick.sqrt();
    // A limping pair drags the pod toward its weaker engine while it is under power.
    let pull = if gas && !c.flying && !sitting { c.pull_drift(c.v) } else { 0. };
    c.y += (c.vy - if c.flying { 0. } else { drift } + pull) * dt;
    // A shove from another pod, which steering cannot cancel.
    let (shove, kick) = contact::kick_step(c.kick);
    c.y += shove;
    c.kick = kick;
    // Wall catches the engine, but does not trap it at a dead stop. The engine on that side takes the heat.
    let edge = if c.y >= 0. { fr.wl } else { fr.wr };
    if c.y.abs() > edge {
        let side = usize::from(c.y > 0.);
        let into = (c.vy + c.kick) * c.y.signum();
        c.y = c.y.signum() * edge;
        c.v *= 1. - 0.12 / pod.mass;
        // A fresh strike costs more than a scrape along the wall; a lighter pod takes more of it.
        if c.impact < 7 {
            let hard = (into - 2.).max(0.) / pod.mass.sqrt();
            engines::dump(c, Some(side), 0.010 * hard + 0.015, 0.006 * hard);
        }
        engines::dump(c, Some(side), WALL_GRIND * dt, 0.);
        c.vy = -c.y.signum() * 4.;
        c.kick = 0.;
        c.impact = 12;
    }
    // Turning about: held still on the brake, the pod yaws on the spot. Under way it
    // straightens to the road, or, nose against it, to the road the other way.
    if !c.flying {
        if brake && steer != 0. && c.v < TURN_SPEED { c.yaw += steer * TURN_RATE * dt; }
        else if c.v > TURN_SPEED + 3. {
            let aim = if c.yaw.cos() >= 0. { 0. } else { std::f32::consts::PI.copysign(c.yaw) };
            c.yaw += (aim - c.yaw).clamp(-STRAIGHTEN * dt, STRAIGHTEN * dt);
        }
        if c.yaw > std::f32::consts::PI { c.yaw -= std::f32::consts::TAU; }
        if c.yaw < -std::f32::consts::PI { c.yaw += std::f32::consts::TAU; }
    }
    // Nose across the road, a pod drives sideways; nose against it, backwards.
    c.y += c.v * c.yaw.sin() * dt;
    let before = c.d;
    let h0 = course.height(before);
    c.d += c.v * dt * fr.scale / (1. - fr.k * c.y).max(0.4) * c.yaw.cos();
    // The grid has a back wall.
    if c.d < BACK_WALL { c.d = BACK_WALL; c.v = 0.; }
    // Hazards that are up this tick.
    for h in hazards_on(course, c.route) {
        if h.kind == HazardKind::Slick || !h.live(tick) || c.flying || !hits(lap, c, h) { continue; }
        let fresh = c.impact < 7;
        match h.kind {
            HazardKind::Slag | HazardKind::Rockfall => {
                let sign = if c.y >= h.y { 1. } else { -1. };
                // A pod that is pushing against a block at a crawl is not struck by it again and again.
                if fresh && c.v > 6. { engines::dump(c, Some(engines::near(c.y, h.y)), SLAG_HEAT / pod.mass.sqrt(), SLAG_WEAR / pod.mass.sqrt()); }
                c.y += sign * 0.55;
                c.v *= 1. - 0.42 / pod.mass.sqrt();
                c.vy = sign * 6.5;
                c.impact = c.impact.max(8);
            }
            HazardKind::Jet => {
                if c.impact < 6 {
                    c.v *= 1. - 0.30 / pod.mass.sqrt();
                    c.boost = (c.boost - 0.25).max(0.);
                    if c.boost <= 0. { c.boost_locked = true; }
                    // The jet's flame licks both engines.
                    engines::dump(c, None, JET_HEAT, JET_WEAR);
                    c.impact = 10;
                }
            }
            HazardKind::Slick => {}
        }
    }
    if c.boosting && c.boost <= 0. { c.boosting = false; }
    let strips = strips_on(course, c.route);
    if strips.iter().any(|p| on_strip(lap, c.d, c.y, p)) && !brake && !c.flying && !sitting { c.v = c.v.max((top + 6.).min(64.).max(top)); }
    // The engines: what the gas, the boost and the lot put in, and a blowout if it is too much.
    engines::heat_step(c, &engines::Load { gas: gas && !sitting, brake, boosting: c.boosting, flying: c.flying });
    engines::blow_if_due(c);
    fork_route(course, c, before);
    jump(course, c, before, h0);
}


/// Joining a fork at its gore by the side the car is on, and rejoining the main road after.
fn fork_route(course: &Course, c: &mut Car, before: f32) {
    let lap = course.lap();
    let (s_old, s_new) = (before.rem_euclid(lap), c.d.rem_euclid(lap));
    if c.route == 0 {
        // A pod facing back down the road does not turn into a fork it is leaving.
        if c.yaw.cos() < 0. { return; }
        if let Some((i, f)) = course.fork_at(s_new) {
            if !f.spans(s_old) && c.y * f.side > 0. {
                c.route = i as u8 + 1;
                let fr = course.frame(c.route, c.d);
                c.y = (c.y - f.side * f.off0).clamp(-fr.wr, fr.wl);
            }
        }
    } else if let Some(f) = course.forks().get(c.route as usize - 1) {
        if !f.spans(s_new) {
            c.route = 0;
            let fr = course.frame(0, c.d);
            c.y = (c.y + f.side * f.off1).clamp(-fr.wr, fr.wl);
        }
    }
}

/// Take-off at a gap's lip, the flight, the landing, or the drop into the chasm.
fn jump(course: &Course, c: &mut Car, before: f32, h0: f32) {
    if c.route != 0 { return; }
    let lap = course.lap();
    let s = c.d.rem_euclid(lap);
    if !c.flying {
        if let Some(gap) = course.gap_at(s) {
            // The ramp throws the car along its own slope.
            c.flying = true;
            c.vz = c.v * gap.ramp;
            c.lift = 0.;
        }
        return;
    }
    let h1 = course.height(c.d);
    c.vz -= GRAVITY / HZ;
    c.lift += c.vz / HZ + h0 - h1;
    let _ = before;
    if course.gap_at(s).is_some() {
        if c.lift < -9. { fall(course, c); }
    } else if c.lift <= 0. {
        if c.lift < -0.9 {
            // Came up short: the far lip.
            fall(course, c);
        } else {
            if c.vz < -6. { c.impact = c.impact.max(8); c.v *= 0.95; }
            c.flying = false; c.lift = 0.; c.vz = 0.;
        }
    }
}

/// Dropped into a chasm: back up the road for another run at it, a good few seconds poorer.
fn fall(course: &Course, c: &mut Car) {
    let lap = course.lap();
    let s = c.d.rem_euclid(lap);
    let lip = course.gaps().iter().filter(|g| g.s0 <= s).map(|g| g.s0).fold(f32::MIN, f32::max);
    // Back a good run-up, rolling: enough road to build the speed the leap wants.
    let back = if lip > f32::MIN { s - lip + 150. } else { 150. };
    c.d -= back;
    c.y = 0.;
    c.vy = 0.;
    c.v = 26.;
    c.boost = c.boost.max(0.6);
    engines::haul_back(c);
    c.impact = 12;
    c.flying = false; c.lift = 0.; c.vz = 0.;
    c.yaw = 0.;
    c.fell += 1;
}

// ---- Rival pilots ----------------------------------------------------------

/// What an empty seat presses when it is a CPU rival with the road to itself. A live human seat
/// must not come through here: idle input stays idle. With rivals on the road use `Session::bot_drives`.
pub fn bot_drive_on(course: &Course, id: u32, car: &Car, ticks: u32) -> Drive {
    let mut alone = car.clone();
    alone.cpu = true;
    pilot::drive(course, &BTreeMap::from([(id, alone)]), id, ticks, pilot::CPU_SLOP)
}


/// Which rivals take a fork: a fixed split, so the field divides the same way every race.
pub(super) fn fork_taken(id: u32, i: usize, f: &Fork) -> bool {
    let pick = (id as usize + i) % 3;
    if f.shortcut { pick != 1 } else { pick == 0 }
}
