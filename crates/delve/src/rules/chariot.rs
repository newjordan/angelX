//! Chariot racing at the Hippodrome: the realm's Rock n' Roll Racing.
//!
//! Four chariots, three laps of the oval, and no clean racing: whip for
//! speed until the team blows, change lanes to block, drop caltrops for
//! whoever rides your tail, and shoulder a rival at the corners where the
//! inside lane runs short and hot. The player's horse is a real horse of
//! the stable (`chivalry::Horse`): its house blood — seasoned by how much
//! that model house has actually served — is its pace, stamina, cornering
//! and nerve. Rivals are drawn from the other market houses the same way.
//!
//! Everything is a pure function of the inputs and the seed, like the
//! joust, so a mirror and a test see the same race. The announcer is the
//! realm's own Larry: his lines are state, called off events, never dice.

use super::*;
use crate::realm::chivalry::{Bloodline, Horse, MARKET_HOUSES, Stable, bloodline};

/// Laps of the oval, and its length in paces (one lap is the whole field).
pub const LAPS: u32 = 3;
pub const LAP: f32 = 1200.0;
/// Ticks the field holds at the gate before the off.
pub const GATE_TICKS: u32 = 2 * HZ;
/// How long the result stands before the driver steps down.
pub const RESULT_TICKS: u32 = 5 * HZ;
/// The purse, by finishing place.
pub const PURSE: [u32; 4] = [90, 45, 20, 0];

/// A course the coach can take the field to: a closed parametric curve in
/// its own tile country, far bigger than the Hippodrome room — the race
/// view follows the player's chariot round it. Distance runs the curve's
/// parameter, the lane pushes along its normal, and corners are wherever
/// the curve actually bends, so every course has its own rhythm of
/// straights, sweepers and hairpins.
#[derive(Clone, Copy, Debug)]
pub struct Track {
    pub name: &'static str,
    /// One lap, in paces.
    pub lap: f32,
    /// The course's country, in tiles.
    pub map: (f32, f32),
    kind: TrackKind,
}

#[derive(Clone, Copy, Debug)]
enum TrackKind {
    Oval,
    Serpent,
    Eight,
    Clover,
    Hairpin,
}

pub const TRACKS: [Track; 5] = [
    Track { name: "Rusty Oval", lap: 1200.0, map: (46.0, 28.0), kind: TrackKind::Oval },
    Track { name: "The Serpent", lap: 1750.0, map: (66.0, 38.0), kind: TrackKind::Serpent },
    Track { name: "Millrace Eight", lap: 1650.0, map: (62.0, 36.0), kind: TrackKind::Eight },
    Track { name: "Three-Leaf Clover", lap: 1850.0, map: (64.0, 44.0), kind: TrackKind::Clover },
    Track { name: "The Hairpin", lap: 1950.0, map: (72.0, 36.0), kind: TrackKind::Hairpin },
];

/// The bend (radians of heading change per tile of ground, over a
/// 120-pace window) at which a stretch of course counts as a corner.
const CORNER_BEND: f32 = 0.075;

/// The centreline of a course at parameter `a` (radians), in tiles.
fn centre(track: &Track, a: f32) -> (f32, f32) {
    let (cx, cy) = (track.map.0 / 2.0, track.map.1 / 2.0);
    match track.kind {
        TrackKind::Oval => (cx + 17.0 * a.cos(), cy + 9.5 * a.sin()),
        TrackKind::Serpent => (
            cx + 26.0 * a.cos(),
            cy + 12.0 * a.sin() + 2.6 * (3.0 * a + 0.7).sin(),
        ),
        TrackKind::Eight => (
            cx + 24.0 * a.sin(),
            cy + 25.0 * a.sin() * a.cos(),
        ),
        TrackKind::Clover => {
            let r = 1.0 + 0.26 * (3.0 * a).cos();
            (cx + 23.0 * r * a.cos(), cy + 14.5 * r * a.sin())
        }
        TrackKind::Hairpin => {
            let spike = |v: f32| v.signum() * v.abs().powf(2.0 / 3.2);
            (cx + 30.0 * spike(a.cos()), cy + 12.0 * spike(a.sin()))
        }
    }
}

fn angle_at(track: &Track, a: f32) -> f32 {
    let d = 0.01;
    let (x0, y0) = centre(track, a - d);
    let (x1, y1) = centre(track, a + d);
    (y1 - y0).atan2(x1 - x0)
}

fn param(track: &Track, dist: f32) -> f32 {
    (dist.rem_euclid(track.lap) / track.lap) * std::f32::consts::TAU
}

/// Where a chariot stands on a course (tiles): the centreline, pushed
/// along its normal by the lane — negative lanes toward the inside of
/// the bend the course is taking.
pub fn pos_on(track: &Track, dist: f32, lane: f32) -> (f32, f32) {
    let a = param(track, dist);
    let (x, y) = centre(track, a);
    let phi = angle_at(track, a);
    let (nx, ny) = (phi.sin(), -phi.cos());
    (x + nx * lane * 1.1, y + ny * lane * 1.1)
}

/// How hard the course bends at `dist`: heading change per tile of
/// ground over the 120 paces around it, so a bend means the same thing
/// on the fast and slow stretches of a curve. Straights sit near 0.03;
/// tight ends run past 0.15.
pub fn bend_on(track: &Track, dist: f32) -> f32 {
    let a = param(track, dist);
    let da = (60.0 / track.lap) * std::f32::consts::TAU;
    let d = angle_at(track, a + da) - angle_at(track, a - da);
    let (x0, y0) = centre(track, a - da);
    let (x1, y1) = centre(track, a + da);
    let ground = (x1 - x0).hypot(y1 - y0).max(0.5);
    d.sin().atan2(d.cos()).abs() / ground
}

/// The direction of travel on a course at `dist`, as a unit vector.
pub fn tangent_on(track: &Track, dist: f32) -> (f32, f32) {
    let phi = angle_at(track, param(track, dist));
    (phi.cos(), phi.sin())
}

/// The heading of travel on a course at `dist`, for the sprite sheets.
pub fn heading_on(track: &Track, dist: f32) -> Heading {
    let phi = angle_at(track, param(track, dist));
    let (vx, vy) = (phi.cos(), phi.sin());
    if vy.abs() > vx.abs() {
        if vy > 0.0 { Heading::South } else { Heading::North }
    } else if vx > 0.0 {
        Heading::East
    } else {
        Heading::West
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Gate,
    Racing,
    Done,
}

/// One chariot in the field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Racer {
    pub name: String,
    pub house: String,
    pub blood: Bloodline,
    /// Paces run, over all laps.
    pub dist: f32,
    /// Lateral place on the track, -1 (inside rail) to 1 (outer wall).
    pub lane: f32,
    pub speed: f32,
    /// 0 (blown) to 1 (fresh). The whip spends it; corners test it.
    pub wind: f32,
    /// Ticks lost to a crash or a caltrop, wheels locked.
    pub wrecked: u32,
    /// Whipped this tick: a whipped team gains no wind back.
    #[serde(default)]
    pub whipped: bool,
    pub caltrops: u8,
    /// Finished place, once over the line.
    pub place: Option<u8>,
}

impl Racer {
    fn new(name: String, house: &str, blood: Bloodline, lane: f32) -> Self {
        Self {
            name,
            house: house.into(),
            blood,
            dist: 0.0,
            lane,
            speed: 0.0,
            wind: 1.0,
            wrecked: 0,
            whipped: false,
            caltrops: 3,
            place: None,
        }
    }
    pub fn lap(&self, track: &Track) -> u32 {
        ((self.dist / track.lap) as u32).min(LAPS)
    }
    /// Where on this lap of `track` the chariot is, 0 to 1.
    pub fn around_on(&self, track: &Track) -> f32 {
        self.dist.rem_euclid(track.lap) / track.lap
    }
    /// In a corner of `track`: wherever the course bends hard — long
    /// enough for cornering blood and nerve to tell, while the
    /// straights still belong to pace.
    pub fn cornering(&self, track: &Track) -> bool {
        bend_on(track, self.dist) > CORNER_BEND
    }
    /// Top pace on the straight, from blood and wind.
    fn top(&self) -> f32 {
        (7.0 + f32::from(self.blood.pace) * 0.55) * (0.72 + 0.28 * self.wind)
    }
    /// Top pace through a bend of `bend` radians per tile: cornering
    /// blood holds the line, and the harder the bend, the more it must
    /// give back.
    fn corner_top(&self, bend: f32) -> f32 {
        4.2 + f32::from(self.blood.corner) * 0.42 + (1.0 - self.lane.abs()) * 0.5
            - ((bend - CORNER_BEND).max(0.0) * 18.0).min(1.6)
    }
    /// A driver's hands for one tick: steer toward the held side, whip
    /// while held, and drop one caltrop on the press. `was` is the (whip,
    /// caltrop) keys held last tick. Returns (the whip was first cracked
    /// this tick, a caltrop to leave on the course).
    fn hands(&mut self, input: Input, was: (bool, bool)) -> (bool, Option<(f32, f32)>) {
        self.lane = (self.lane + f32::from(input.move_x) * 0.03).clamp(-1.0, 1.0);
        self.whipped = input.dash && self.wrecked == 0;
        if self.whipped {
            self.lash();
        }
        let trap = if input.bomb && !was.1 && self.caltrops > 0 && self.wrecked == 0 {
            self.caltrops -= 1;
            Some((self.dist - 6.0, self.lane))
        } else {
            None
        };
        (self.whipped && !was.0, trap)
    }
    /// One lash of the whip: speed now, paid for in wind at the stamina's
    /// rate. The same lash for the player's hand and a rival's — the
    /// player's edge is judgment, not a stronger arm.
    fn lash(&mut self) {
        self.speed += 0.09;
        self.wind = (self.wind - 0.0022 - 0.0009 * f32::from(9 - self.blood.stamina)).max(0.0);
    }
}

/// What happened on a tick, for the announcer and the sound board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Off,
    Whip,
    CaltropDropped,
    CaltropHit(u8),
    Shoulder(u8),
    Crash(u8),
    FinalLap(u8),
    Finished(u8, u8),
    Won,
    Lost,
}

/// A race at the Hippodrome, as it stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chariot {
    /// Zero after the starter forfeits; that team then races itself.
    pub driver: u32,
    /// The herd horse the driver brought, by name (its record is settled
    /// under this name; a mount-name means the old three, unrecorded).
    pub horse: String,
    pub racers: [Racer; 4],
    pub traps: Vec<(f32, f32)>,
    pub phase: Phase,
    pub t: u32,
    pub seed: u64,
    /// Which course in `TRACKS` this race runs (the seed's pick; an older
    /// race with no course recorded runs the Rusty Oval).
    #[serde(default)]
    pub track: usize,
    /// Keys held last tick, so a held key acts once.
    #[serde(default)]
    held: (bool, bool),
    /// Friends in the room when the race began each drive a rival team:
    /// (racer index, seat), and their keys held last tick.
    #[serde(default)]
    pub friends: Vec<(u8, u32)>,
    #[serde(default)]
    friend_held: Vec<(bool, bool)>,
}

impl Chariot {
    /// The field: the player's horse first, then one chariot per other
    /// market house, blood seasoned by each house's service in `stable`.
    pub fn field(driver: u32, horse: &Horse, stable: &Stable, seed: u64) -> Self {
        let mut racers = vec![Racer::new(
            horse.name.clone(),
            &horse.house,
            horse.blood,
            -0.6,
        )];
        for (i, house) in MARKET_HOUSES
            .iter()
            .filter(|h| **h != horse.house)
            .take(3)
            .enumerate()
        {
            let mut blood = bloodline(house);
            blood_season(&mut blood, stable.yard_tier(house));
            racers.push(Racer::new(
                format!("{} yard", house),
                house,
                blood,
                -0.2 + 0.4 * i as f32,
            ));
        }
        while racers.len() < 4 {
            let mut blood = bloodline("the keep");
            blood_season(&mut blood, 0);
            racers.push(Racer::new("the Keep".into(), "keep", blood, 0.6));
        }
        Self {
            driver,
            horse: horse.name.clone(),
            racers: racers.try_into().unwrap(),
            traps: Vec::new(),
            phase: Phase::Gate,
            t: 0,
            seed,
            // The coachman's whim: each seed names its course, so a mirror
            // and a test agree on where the race runs.
            track: (mix(seed ^ 0x7AC7) % TRACKS.len() as u64) as usize,
            held: (false, false),
            friends: Vec::new(),
            friend_held: Vec::new(),
        }
    }

    /// Friend `seat`, named `name`, takes the reins of the next rival team.
    pub fn seat_friend(&mut self, seat: u32, name: &str) {
        let index = self.friends.len() + 1;
        if index >= self.racers.len() {
            return;
        }
        self.racers[index].name = name.into();
        self.friends.push((index as u8, seat));
        self.friend_held.push((false, false));
    }

    pub(super) fn depart(&mut self, seat: u32) {
        if self.driver == seat { self.driver = 0; }
        if let Some(k) = self.friends.iter().position(|&(_, id)| id == seat) {
            self.friends.remove(k);
            if k < self.friend_held.len() { self.friend_held.remove(k); }
        }
    }

    /// The racer seat `seat` drives, if any (0 for the one who boarded).
    pub fn racer_of(&self, seat: u32) -> Option<usize> {
        if seat == self.driver {
            return Some(0);
        }
        self.friends.iter().find(|(_, s)| *s == seat).map(|(i, _)| usize::from(*i))
    }

    /// Every knight at the reins, with the racer each drives: the one who
    /// boarded on racer 0, then the friends on their teams.
    pub fn seats(&self) -> Vec<(u32, usize)> {
        std::iter::once((self.driver, 0))
            .chain(self.friends.iter().map(|&(i, s)| (s, usize::from(i))))
            .filter(|(id, _)| *id != 0)
            .collect()
    }

    /// The course this race runs.
    pub fn course(&self) -> &'static Track {
        &TRACKS[self.track.min(TRACKS.len() - 1)]
    }

    pub fn player(&self) -> &Racer {
        &self.racers[0]
    }

    /// Running order, first to last (indices into `racers`).
    pub fn order(&self) -> [usize; 4] {
        let mut order = [0, 1, 2, 3];
        order.sort_by(|&a, &b| {
            self.racers[b]
                .dist
                .partial_cmp(&self.racers[a].dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        order
    }

    /// The announcer's current line, called off the state of the race.
    pub fn call(&self) -> &'static str {
        match self.phase {
            Phase::Gate => "LET THE CARNAGE BEGIN!",
            Phase::Done => match self.player().place {
                Some(1) => "HOME YARD WINS",
                Some(4) => "LAST PLACE",
                _ => "PURSE PAID",
            },
            Phase::Racing => {
                let p = self.player();
                if p.wrecked > 0 {
                    "IN THE WALL"
                } else if p.lap(self.course()) + 1 == LAPS && p.place.is_none() {
                    "FINAL LAP"
                } else if self.order()[0] == 0 {
                    "YOU LEAD"
                } else if p.wind < 0.25 {
                    "TEAM BLOWN"
                } else {
                    "ROLLING"
                }
            }
        }
    }

    /// One tick of the race. `input` drives racer 0; rivals drive
    /// themselves off the seed. Returns what happened, in order.
    #[cfg(any(test, feature = "testing"))]
    pub fn tick(&mut self, input: Input) -> Vec<Event> {
        self.tick_with(input, &[])
    }

    /// As `tick`, with friends' keys too, in `friends` order.
    pub fn tick_with(&mut self, input: Input, friend_inputs: &[Input]) -> Vec<Event> {
        let mut events = Vec::new();
        self.t += 1;
        match self.phase {
            Phase::Gate => {
                if self.t >= GATE_TICKS {
                    self.phase = Phase::Racing;
                    self.t = 0;
                    events.push(Event::Off);
                }
                return events;
            }
            Phase::Done => return events,
            Phase::Racing => {}
        }
        let course = self.course();
        // The drivers' hands: steer, whip, and the caltrop bag — the
        // player's on racer 0, then each friend's on their own team.
        let mut drivers = Vec::new();
        if self.driver != 0 { drivers.push((0, input, self.held)); }
        self.held = (input.dash, input.bomb);
        for (k, &(index, _)) in self.friends.iter().enumerate() {
            let fin = friend_inputs.get(k).copied().unwrap_or_default();
            let was = self.friend_held.get(k).copied().unwrap_or_default();
            if let Some(held) = self.friend_held.get_mut(k) {
                *held = (fin.dash, fin.bomb);
            }
            drivers.push((usize::from(index), fin, was));
        }
        for (index, keys, was) in drivers {
            let (cracked, trap) = self.racers[index].hands(keys, was);
            if cracked {
                events.push(Event::Whip);
            }
            if let Some(trap) = trap {
                self.traps.push(trap);
                events.push(Event::CaltropDropped);
            }
        }
        // Rivals: chase the leader, take the inside at corners, whip like
        // drivers (hard while the team has wind, resting it back on the
        // straights), drop iron for a tailgater, and never drive the same
        // race twice (seed, not dice).
        for i in 0..4 {
            if (i == 0 && self.driver != 0) || self.friends.iter().any(|(f, _)| usize::from(*f) == i) {
                continue;
            }
            let (seed, t) = (self.seed, self.t);
            let hash = mix(seed ^ (i as u64) << 16 ^ u64::from(t / HZ));
            // Is anyone riding this rival's tail, close enough to eat iron?
            let tailgated = {
                let me = &self.racers[i];
                self.racers.iter().enumerate().any(|(j, o)| {
                    j != i && me.dist - o.dist > 4.0 && me.dist - o.dist < 16.0 && (me.lane - o.lane).abs() < 0.5
                })
            };
            let r = &mut self.racers[i];
            let target = if r.cornering(course) {
                -0.55
            } else {
                f32::from((hash % 5) as i8 - 2) * 0.3
            };
            r.lane += (target - r.lane).clamp(-0.025, 0.025);
            r.whipped = r.wrecked == 0 && r.wind > 0.4 && hash % 7 < 4;
            if r.whipped {
                r.lash();
            }
            let drop_itch = mix(seed ^ 0xCA17 ^ (i as u64) << 20 ^ u64::from(t));
            if tailgated && r.caltrops > 0 && r.wrecked == 0 && drop_itch % 251 == 0 {
                r.caltrops -= 1;
                self.traps.push((r.dist - 6.0, r.lane));
                events.push(Event::CaltropDropped);
            }
        }
        // Shouldering: alongside a rival in a corner, nerve decides who
        // gives the rail — and the loser may not keep his wheels.
        for i in 1..4 {
            let (near, lanes) = {
                let p = &self.racers[0];
                (
                    (p.dist - self.racers[i].dist).abs() < 9.0,
                    (p.lane - self.racers[i].lane).abs(),
                )
            };
            if near && lanes < 0.45 && self.racers[0].cornering(course) && self.racers[i].wrecked == 0 {
                let nerve =
                    i32::from(self.racers[0].blood.nerve) - i32::from(self.racers[i].blood.nerve);
                if nerve >= 0 {
                    self.racers[i].speed *= 0.82;
                    events.push(Event::Shoulder(i as u8));
                } else if mix(self.seed ^ self.t as u64) % 11 == 0 && self.racers[0].wrecked == 0 {
                    self.racers[0].wrecked = HZ / 2;
                    self.racers[0].speed = 0.0;
                    events.push(Event::Crash(0));
                }
            }
        }
        // Caltrops wait where they fell, for anyone but their owner.
        let mut hits = Vec::new();
        for (at, lane) in &self.traps {
            for (i, r) in self.racers.iter().enumerate() {
                if r.wrecked == 0 && (r.dist - at).abs() < 2.5 && (r.lane - lane).abs() < 0.4 {
                    hits.push(i);
                }
            }
        }
        for &i in &hits {
            self.racers[i].wrecked = HZ * 3 / 4;
            self.racers[i].speed = 0.0;
            events.push(if i == 0 {
                Event::CaltropHit(0)
            } else {
                Event::CaltropHit(i as u8)
            });
        }
        if !hits.is_empty() {
            self.traps
                .retain(|(at, _)| !hits.iter().any(|&i| (self.racers[i].dist - at).abs() < 3.0));
        }
        // The running itself: corners cap the pace, blown teams fade, and
        // the wind comes back on the straights at the stamina's pace.
        let total = course.lap * LAPS as f32;
        let mut placed = self.racers.iter().filter(|o| o.place.is_some()).count() as u8;
        for (index, r) in self.racers.iter_mut().enumerate() {
            if r.place.is_some() {
                continue;
            }
            if r.wrecked > 0 {
                r.wrecked -= 1;
            } else {
                let bend = bend_on(course, r.dist);
                let cap = if bend > CORNER_BEND {
                    r.corner_top(bend)
                } else {
                    r.top()
                };
                r.speed += (cap - r.speed).clamp(-0.22, 0.045);
                r.dist += r.speed.max(0.0);
                if bend <= CORNER_BEND && !r.whipped {
                    r.wind = (r.wind + 0.0005 * f32::from(r.blood.stamina)).min(1.0);
                }
            }
            if r.dist >= total {
                r.dist = total;
                placed += 1;
                let place = placed;
                r.place = Some(place);
                events.push(Event::Finished(index as u8, place));
            }
        }
        for i in 0..4 {
            if self.racers[i].lap(course) + 1 == LAPS && self.racers[i].dist.rem_euclid(course.lap) < 8.0 && self.t > HZ {
                events.push(Event::FinalLap(i as u8));
            }
            let _ = i;
        }
        if self.racers.iter().all(|r| r.place.is_some()) {
            self.phase = Phase::Done;
            self.t = 0;
            events.push(if self.player().place == Some(1) {
                Event::Won
            } else {
                Event::Lost
            });
        }
        events
    }
}

fn blood_season(blood: &mut Bloodline, tier: u8) {
    blood.season(tier);
}

fn mix(mut h: u64) -> u64 {
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ h >> 31
}

/// Where a chariot stands in the Hippodrome room (tiles): the oval, with
/// the lane pushing out from the rail. For the driver, the rivals and the
/// renderer alike.
#[cfg(any(test, feature = "testing"))]
pub fn track_at(racer: &Racer) -> (f32, f32) {
    track_at_dist(racer.dist, racer.lane)
}

/// The oval at any distance and lane — a dropped caltrop's address, as
/// much as a chariot's.
pub fn track_at_dist(dist: f32, lane: f32) -> (f32, f32) {
    let a = (dist.rem_euclid(LAP) / LAP) * std::f32::consts::TAU;
    let out = lane * 1.1;
    (
        12.0 + (9.0 + out) * a.cos(),
        7.0 + (4.4 + out * 0.5) * a.sin(),
    )
}

/// Which way a chariot points on the oval, so the renderer can pick its
/// sheet: along the straights it runs east or west (west while
/// `around() < 0.5`, by the tangent's sign), and round the ends it comes
/// at the viewer or away from them instead of riding sideways.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    East,
    West,
    /// Away from the viewer, up the west end.
    North,
    /// At the viewer, down the east end.
    South,
}

#[cfg(any(test, feature = "testing"))]
pub fn heading(racer: &Racer) -> Heading {
    heading_at(racer.dist)
}

/// The heading at any distance round the oval (the renderer's gate grid
/// draws chariots a few paces behind their true mark).
#[cfg(any(test, feature = "testing"))]
pub fn heading_at(dist: f32) -> Heading {
    let a = (dist.rem_euclid(LAP) / LAP) * std::f32::consts::TAU;
    let (vx, vy) = (-9.0 * a.sin(), 4.4 * a.cos());
    if vy.abs() > vx.abs() {
        if vy > 0.0 { Heading::South } else { Heading::North }
    } else if vx > 0.0 {
        Heading::East
    } else {
        Heading::West
    }
}

impl Run {
    /// Board a chariot at the Hippodrome coach and race. The driver brings
    /// the selected grown herd horse, or the best eligible one; otherwise a keep nag runs
    /// unrecorded — buy or breed a horse to make the race count.
    pub fn board_chariot(&mut self, id: u32) {
        let stable = &self.home.stable;
        let horse = stable.selected_herd.and_then(|i| stable.herd.get(i))
            .filter(|h| h.raceable(self.home.ranch.wins))
            .or_else(|| stable.herd.iter()
                .filter(|h| h.raceable(self.home.ranch.wins))
                .max_by_key(|h| h.value(stable)))
            .cloned()
            .unwrap_or_else(|| Horse {
                name: "Yard Nag".into(),
                house: "keep".into(),
                blood: bloodline("keep"),
                ..Horse::default()
            });
        let mut race = Chariot::field(id, &horse, stable, self.seed ^ self.tick as u64);
        // Everyone in the room races: friends take the rival teams.
        for (&seat, hero) in &self.players {
            if seat != id && hero.hp > 0 && !hero.stone {
                race.seat_friend(seat, &hero.name);
            }
        }
        self.chariot = Some(race);
        *self.marks.entry("chariot:start".into()).or_default() += 1;
        self.cues.push("chariot_start".into());
        self.sounds.push("door_open");
    }

    /// One tick at the Hippodrome: the coach plate, and a race if one runs.
    pub(super) fn tick_hippodrome_race(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.chariot.is_none() {
            let mut driver = None;
            for (&id, hero) in self.players.iter_mut() {
                let interact = inputs
                    .get(&id)
                    .copied()
                    .filter(|i| i.valid())
                    .is_some_and(|i| i.interact);
                if !interact {
                    hero.buy_spent = false;
                }
                let (c, r, w, h) = super::hippodrome::COACH;
                let on = hero.hp > 0
                    && !hero.stone
                    && hero.x / TILE_UNITS >= c as f32
                    && hero.x / TILE_UNITS < (c + w) as f32
                    && hero.y / TILE_UNITS >= r as f32
                    && hero.y / TILE_UNITS < (r + h) as f32;
                if on && interact && !hero.buy_spent {
                    // Space is also a dodge outside the race. Board the
                    // drift ride immediately so that dodge cannot carry a
                    // driver off the coach plate during the E hold.
                    hero.buying = if inputs.get(&id).is_some_and(|i| i.dash) {
                        super::home::BUY_HOLD
                    } else { hero.buying.saturating_add(1) };
                    if hero.buying >= super::home::BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        driver = driver.or(Some(id));
                    }
                } else if on {
                    hero.buying = 0;
                }
            }
            if let Some(id) = driver {
                if inputs.get(&id).is_some_and(|i| i.dash) { self.board_drift(); }
                else { self.board_chariot(id); }
            }
            return;
        }
        let Some(mut race) = self.chariot.take() else {
            return;
        };
        let input = inputs
            .get(&race.driver)
            .copied()
            .filter(|i| i.valid())
            .unwrap_or_default();
        let friend_inputs: Vec<Input> = race
            .friends
            .iter()
            .map(|(_, seat)| inputs.get(seat).copied().filter(|i| i.valid()).unwrap_or_default())
            .collect();
        for event in race.tick_with(input, &friend_inputs) {
            match event {
                Event::Off => {
                    self.cues.push("chariot_off".into());
                    self.sounds.push("descend");
                }
                Event::Crash(who) | Event::CaltropHit(who) => {
                    self.fx.push(Fx::Shake(9));
                    self.sounds.push("knight_hit_big");
                    self.cues.push(if who == 0 {
                        "chariot_wrecked".into()
                    } else {
                        "chariot_rival_wrecked".into()
                    });
                }
                Event::Won => {
                    self.cues.push("chariot_won".into());
                    // The purse, in sound ("pastime_won" is a cue, not an SFX).
                    self.sounds.push("gold");
                    self.crown_the_charioteer(&race);
                }
                Event::Lost => {
                    self.cues.push("chariot_lost".into());
                    self.crown_the_charioteer(&race);
                }
                Event::Finished(0, place) if race.driver != 0 => {
                    let name = race.horse.replace(' ', "_");
                    *self
                        .marks
                        .entry(format!("chariot:{place}:{name}"))
                        .or_default() += 1;
                }
                _ => {}
            }
        }
        // Seat each driver in the room at the same point of their lap:
        // the race view itself is the stage's big course, but the knights
        // in the world ride the little grounds oval in step with it.
        let seats = race.seats();
        for &(seat, index) in &seats {
            let racer = &race.racers[index];
            let (tx, ty) = track_at_dist(racer.around_on(race.course()) * LAP, racer.lane);
            if let Some(hero) = self.players.get_mut(&seat) {
                (hero.x, hero.y) = (tx * TILE_UNITS, ty * TILE_UNITS);
                (hero.aim_x, hero.aim_y) = (1.0, 0.0);
            }
        }
        if race.phase == Phase::Done && race.t >= RESULT_TICKS {
            let (c, r, w, h) = super::hippodrome::COACH;
            for (k, &(seat, _)) in seats.iter().enumerate() {
                if let Some(hero) = self.players.get_mut(&seat) {
                    hero.x = (c as f32 + w as f32 / 2.0 + k as f32 - 1.0) * TILE_UNITS;
                    hero.y = (r as f32 + h as f32 + 0.6) * TILE_UNITS;
                    hero.buy_spent = true;
                }
            }
            return;
        }
        self.chariot = Some(race);
    }
}

impl Run {
    /// With friends in the race, the best-placed knight wears the crown and
    /// the rest a cabbage, as at any game against friends.
    fn crown_the_charioteer(&mut self, race: &Chariot) {
        if race.friends.is_empty() {
            return;
        }
        let seats = race.seats();
        let Some(&(best, _)) = seats
            .iter()
            .min_by_key(|(_, index)| race.racers[*index].place.unwrap_or(u8::MAX))
        else {
            return;
        };
        for &(seat, _) in &seats {
            if let Some(hero) = self.players.get_mut(&seat) {
                hero.laurel = seat == best;
                hero.booby = (seat != best).then_some(super::pastimes::Game::Chariot);
            }
        }
        let name = self.players.get(&best).map_or_else(|| "A knight".into(), |h| h.name.to_uppercase());
        self.found = Some((self.tick, 0, format!("{name} WINS THE RACE!")));
    }
}

/// What a race marked, settled into the stable: the horse's record and
/// the purse in gold (paid by the caller, like the coach commission).
pub fn settle_chariot(
    stable: &mut Stable,
    marks: &BTreeMap<String, u32>,
) -> (Vec<(String, String)>, u32) {
    let mut said = Vec::new();
    let mut gold = 0;
    for (mark, &times) in marks {
        // The same horse in the same place twice in one outing is paid twice.
        let times = times.clamp(1, 20);
        let Some(rest) = mark.strip_prefix("chariot:") else {
            continue;
        };
        let mut it = rest.splitn(2, ':');
        let (Some(place), Some(name)) = (it.next(), it.next()) else {
            continue;
        };
        let Ok(place) = place.parse::<usize>() else {
            continue;
        };
        if !(1..=4).contains(&place) { continue; }
        let name = name.replace('_', " ");
        let Some(horse) = stable.herd.iter_mut().find(|h| h.name == name && !h.name.eq_ignore_ascii_case("Helios")) else {
            continue;
        };
        horse.starts += times;
        if place == 1 {
            horse.wins += times;
        } else if place == 2 {
            horse.seconds += times;
        }
        let purse = PURSE.get(place - 1).copied().unwrap_or(0) * times;
        horse.earned += purse;
        gold += purse;
        said.push((
            "chariot_result".into(),
            format!(
                "{} came {} at the Hippodrome: {} gold, and the record stands at {} wins from {} starts.",
                horse.name,
                ["first", "second", "third", "last"][place.min(4) - 1],
                purse,
                horse.wins,
                horse.starts
            ),
        ));
    }
    (said, gold)
}

#[cfg(test)]
#[path = "../../tests/rules/together_chariot__tests.rs"]
mod tests;
