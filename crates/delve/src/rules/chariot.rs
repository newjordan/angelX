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

pub mod show;
use show::Kind;
use super::juice::Weight;

pub const LAPS: u32 = 3;
pub const LAP: f32 = 1200.0;
pub const GATE_TICKS: u32 = 3 * HZ;
pub const RESULT_TICKS: u32 = 11 * HZ;
pub const LANE_TILES: f32 = 1.3;
pub const MOST_TEAMS: usize = 6;
pub const PURSE: [u32; 4] = [90, 45, 20, 0];

#[derive(Clone, Copy, Debug)]
pub struct Track {
    pub name: &'static str,
    pub lap: f32,
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

const CORNER_BEND: f32 = 0.075;

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

pub fn pos_on(track: &Track, dist: f32, lane: f32) -> (f32, f32) {
    let a = param(track, dist);
    let (x, y) = centre(track, a);
    let phi = angle_at(track, a);
    let (nx, ny) = (phi.sin(), -phi.cos());
    (x + nx * lane * LANE_TILES, y + ny * lane * LANE_TILES)
}

pub fn bend_on(track: &Track, dist: f32) -> f32 {
    let a = param(track, dist);
    let da = (60.0 / track.lap) * std::f32::consts::TAU;
    let d = angle_at(track, a + da) - angle_at(track, a - da);
    let (x0, y0) = centre(track, a - da);
    let (x1, y1) = centre(track, a + da);
    let ground = (x1 - x0).hypot(y1 - y0).max(0.5);
    d.sin().atan2(d.cos()).abs() / ground
}

pub fn tangent_on(track: &Track, dist: f32) -> (f32, f32) {
    let phi = angle_at(track, param(track, dist));
    (phi.cos(), phi.sin())
}

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Racer {
    #[serde(default)]
    pub team: u8,
    pub name: String,
    pub house: String,
    pub blood: Bloodline,
    pub dist: f32,
    pub lane: f32,
    pub speed: f32,
    pub wind: f32,
    pub wrecked: u32,
    #[serde(default)]
    pub whipped: bool,
    pub caltrops: u8,
    pub place: Option<u8>,
    #[serde(default)]
    pub crash_spin: u8,
    #[serde(default)]
    pub boost_flame: u8,
    #[serde(default)]
    pub lap_flash: u8,
    #[serde(default)]
    pub cabbage_splat: u8,
    #[serde(default)]
    pub splat_x: i32,
    #[serde(default)]
    pub splat_y: i32,
    #[serde(default)]
    pub coast: f32,
    #[serde(default)]
    pub lap_at: u32,
    #[serde(default)]
    pub best: u32,
    #[serde(default)]
    pub line: f32,
    #[serde(default)]
    pub jolt: u8,
}

impl Racer {
    fn new(name: String, house: &str, blood: Bloodline, lane: f32) -> Self {
        Self {
            team: 0,
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
            crash_spin: 0,
            boost_flame: 0,
            lap_flash: 0,
            cabbage_splat: 0,
            splat_x: 0,
            splat_y: 0,
            coast: 0.0,
            lap_at: 0,
            best: 0,
            line: 0.0,
            jolt: 0,
        }
    }
    pub fn lap(&self, track: &Track) -> u32 {
        ((self.dist / track.lap) as u32).min(LAPS)
    }
    pub fn around_on(&self, track: &Track) -> f32 {
        self.dist.rem_euclid(track.lap) / track.lap
    }
    pub fn cornering(&self, track: &Track) -> bool {
        bend_on(track, self.dist) > CORNER_BEND
    }
    fn top(&self) -> f32 {
        (7.0 + f32::from(self.blood.pace) * 0.55) * (0.72 + 0.28 * self.wind)
    }
    fn corner_top(&self, bend: f32) -> f32 {
        4.2 + f32::from(self.blood.corner) * 0.42 + (1.0 - self.lane.abs()) * 0.5
            - ((bend - CORNER_BEND).max(0.0) * 18.0).min(1.6)
    }
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
    fn lash(&mut self) {
        self.speed += 0.09;
        self.wind = (self.wind - 0.0022 - 0.0009 * f32::from(9 - self.blood.stamina)).max(0.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Off,
    Whip,
    CaltropDropped,
    CaltropHit(u8),
    Shoulder(u8),
    Lead(u8),
    Crash(u8),
    FinalLap(u8),
    Finished(u8, u8),
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chariot {
    pub driver: u32,
    pub horse: String,
    pub racers: Vec<Racer>,
    pub traps: Vec<(f32, f32)>,
    pub phase: Phase,
    pub t: u32,
    pub seed: u64,
    #[serde(default)]
    pub track: usize,
    #[serde(default)]
    held: (bool, bool),
    #[serde(default)]
    pub friends: Vec<(u8, u32)>,
    #[serde(default)]
    friend_held: Vec<(bool, bool)>,
    #[serde(default)]
    pub clock: u32,
    #[serde(default, skip_serializing_if = "show::Show::quiet")]
    pub show: show::Show,
}

impl Chariot {
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
        for (i, r) in racers.iter_mut().enumerate() {
            r.team = i as u8;
        }
        Self {
            driver,
            horse: horse.name.clone(),
            racers,
            traps: Vec::new(),
            phase: Phase::Gate,
            t: 0,
            seed,
            track: (mix(seed ^ 0x7AC7) % TRACKS.len() as u64) as usize,
            held: (false, false),
            friends: Vec::new(),
            friend_held: Vec::new(),
            clock: 0,
            show: show::Show::default(),
        }
    }

    pub fn seat_friend(&mut self, seat: u32, name: &str) {
        let index = self.friends.len() + 1;
        if index >= MOST_TEAMS {
            return;
        }
        while self.racers.len() <= index {
            let mut blood = bloodline("the keep");
            blood_season(&mut blood, 0);
            let mut team = Racer::new("the Keep".into(), ["tower", "marsh"][self.racers.len() % 2], blood, 0.6);
            team.team = self.racers.len() as u8;
            self.racers.push(team);
            let n = self.racers.len() as f32;
            for (i, r) in self.racers.iter_mut().enumerate() {
                r.lane = -0.9 + 1.8 * i as f32 / (n - 1.0);
            }
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

    pub fn racer_of(&self, seat: u32) -> Option<usize> {
        if seat == self.driver {
            return Some(0);
        }
        self.friends.iter().find(|(_, s)| *s == seat).map(|(i, _)| usize::from(*i))
    }

    pub fn seats(&self) -> Vec<(u32, usize)> {
        std::iter::once((self.driver, 0))
            .chain(self.friends.iter().map(|&(i, s)| (s, usize::from(i))))
            .filter(|(id, _)| *id != 0)
            .collect()
    }

    pub fn course(&self) -> &'static Track {
        &TRACKS[self.track.min(TRACKS.len() - 1)]
    }

    pub fn player(&self) -> &Racer {
        &self.racers[0]
    }

    pub fn order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.racers.len()).collect();
        order.sort_by(|&a, &b| {
            let (ra, rb) = (&self.racers[a], &self.racers[b]);
            ra.place
                .unwrap_or(u8::MAX)
                .cmp(&rb.place.unwrap_or(u8::MAX))
                .then(rb.dist.partial_cmp(&ra.dist).unwrap_or(std::cmp::Ordering::Equal))
                .then(ra.team.cmp(&rb.team))
        });
        order
    }

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

    #[cfg(any(test, feature = "testing"))]
    pub fn tick(&mut self, input: Input) -> Vec<Event> {
        self.tick_with(input, &[])
    }

    fn wreck_by_barge(&mut self, lose: usize, win: usize, pace: f32, now: u32, events: &mut Vec<Event>) {
        let hard = if pace >= 8.0 { Weight::Kill } else { Weight::Crushing };
        let (wt, lt) = (self.racers[win].team, self.racers[lose].team);
        let at = [self.racers[lose].dist, self.racers[lose].lane];
        self.racers[lose].wrecked = HZ / 2;
        self.racers[lose].speed = 0.0;
        self.racers[lose].crash_spin = 10;
        events.push(Event::Crash(lose as u8));
        self.show.put(Kind::Wreck, lt, wt, (pace * 10.0) as u32, at, hard, now);
    }

    pub fn tick_with(&mut self, input: Input, friend_inputs: &[Input]) -> Vec<Event> {
        let mut events = Vec::new();
        self.t += 1;
        self.clock += 1;
        let now = self.clock;
        self.show.tick(now);
        for r in self.racers.iter_mut().filter(|r| r.place.is_some()) {
            r.speed *= 0.965;
            r.coast += r.speed.max(0.0);
        }
        match self.phase {
            Phase::Gate => {
                if self.t >= GATE_TICKS {
                    self.phase = Phase::Racing;
                    self.t = 0;
                    events.push(Event::Off);
                    self.show.put(Kind::Off, 0, 0, 0, [0.0, 0.0], Weight::Graze, now);
                }
                return events;
            }
            Phase::Done => return events,
            Phase::Racing => {}
        }
        let course = self.course();
        let count = self.racers.len();
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
        let driven: Vec<bool> = (0..count).map(|i| drivers.iter().any(|d| d.0 == i)).collect();
        for (index, keys, was) in drivers {
            let (cracked, trap) = self.racers[index].hands(keys, was);
            let (team, at) = (self.racers[index].team, [self.racers[index].dist, self.racers[index].lane]);
            if cracked || (self.racers[index].whipped && (now + 5 * index as u32) % 14 == 0) {
                if cracked {
                    events.push(Event::Whip);
                }
                self.show.put(Kind::Whip, team, 0, 0, at, Weight::Graze, now);
            }
            if let Some(trap) = trap {
                self.traps.push(trap);
                events.push(Event::CaltropDropped);
                self.show.put(Kind::Iron, team, 0, 0, [trap.0, trap.1], Weight::Graze, now);
            }
        }
        for i in 0..count {
            if driven[i] {
                continue;
            }
            let (seed, t) = (self.seed, self.t);
            let hash = mix(seed ^ (i as u64) << 16 ^ u64::from(t / HZ));
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
            let was_whipping = r.whipped;
            r.whipped = r.wrecked == 0 && r.wind > 0.4 && hash % 7 < 4;
            if r.whipped {
                r.lash();
                if !was_whipping {
                    let (team, at) = (r.team, [r.dist, r.lane]);
                    self.show.put(Kind::Whip, team, 0, 0, at, Weight::Graze, now);
                }
            }
            let r = &mut self.racers[i];
            let drop_itch = mix(seed ^ 0xCA17 ^ (i as u64) << 20 ^ u64::from(t));
            if tailgated && r.caltrops > 0 && r.wrecked == 0 && drop_itch % 251 == 0 {
                r.caltrops -= 1;
                let trap = (r.dist - 6.0, r.lane);
                let team = r.team;
                self.traps.push(trap);
                events.push(Event::CaltropDropped);
                self.show.put(Kind::Iron, team, 0, 0, [trap.0, trap.1], Weight::Graze, now);
            }
        }
        for i in 0..count {
            self.racers[i].jolt = self.racers[i].jolt.saturating_sub(1);
        }
        for i in 0..count {
            for j in i + 1..count {
                let live = |r: &Racer| r.place.is_none() && r.wrecked == 0;
                if !live(&self.racers[i]) || !live(&self.racers[j]) {
                    continue;
                }
                let gap = (self.racers[i].dist - self.racers[j].dist).abs();
                let apart = self.racers[i].lane - self.racers[j].lane;
                if gap >= BARGE_GAP || apart.abs() >= BARGE_LANES {
                    continue;
                }
                let side = if apart >= 0.0 { 1.0 } else { -1.0 };
                if !(driven[i] || driven[j]) {
                    self.racers[i].lane = (self.racers[i].lane + side * 0.012).clamp(-1.0, 1.0);
                    self.racers[j].lane = (self.racers[j].lane - side * 0.012).clamp(-1.0, 1.0);
                    continue;
                }
                let corner = self.racers[i].cornering(course) || self.racers[j].cornering(course);
                let nerve = i32::from(self.racers[i].blood.nerve) - i32::from(self.racers[j].blood.nerve);
                let (win, lose) = if nerve >= 0 { (i, j) } else { (j, i) };
                let pace = self.racers[i].speed.max(self.racers[j].speed);
                let crash = corner
                    && nerve != 0
                    && mix(self.seed ^ u64::from(now) ^ ((i as u64) << 40) ^ ((j as u64) << 48)) % 11 == 0;
                if self.racers[i].jolt > 0 || self.racers[j].jolt > 0 {
                    self.racers[i].lane = (self.racers[i].lane + side * 0.015).clamp(-1.0, 1.0);
                    self.racers[j].lane = (self.racers[j].lane - side * 0.015).clamp(-1.0, 1.0);
                    if crash {
                        self.wreck_by_barge(lose, win, pace, now, &mut events);
                    }
                    continue;
                }
                let weight = if (corner && pace >= 6.0) || nerve.abs() >= 4 {
                    Weight::Crushing
                } else if pace >= 6.0 || corner {
                    Weight::Solid
                } else {
                    Weight::Graze
                };
                let away = if self.racers[lose].lane >= self.racers[win].lane { 1.0 } else { -1.0 };
                self.racers[lose].speed *= if corner { LOSER_CORNER } else { LOSER_STRAIGHT };
                self.racers[win].speed *= WINNER;
                self.racers[lose].lane = (self.racers[lose].lane + away * 0.1).clamp(-1.0, 1.0);
                self.racers[win].lane = (self.racers[win].lane - away * 0.03).clamp(-1.0, 1.0);
                self.racers[i].jolt = 10;
                self.racers[j].jolt = 10;
                let (wt, lt) = (self.racers[win].team, self.racers[lose].team);
                let at = [self.racers[lose].dist, self.racers[lose].lane];
                self.show.put(Kind::Barge, wt, lt, (pace * 10.0) as u32, at, weight, now);
                events.push(Event::Shoulder(lose as u8));
                if crash {
                    self.wreck_by_barge(lose, win, pace, now, &mut events);
                }
            }
        }
        let mut hits = Vec::new();
        for (at, lane) in &self.traps {
            for (i, r) in self.racers.iter().enumerate() {
                if r.wrecked == 0 && (r.dist - at).abs() < 2.5 && (r.lane - lane).abs() < 0.4 {
                    hits.push(i);
                }
            }
        }
        for &i in &hits {
            let pace = self.racers[i].speed;
            self.racers[i].wrecked = HZ * 3 / 4;
            self.racers[i].speed = 0.0;
            self.racers[i].crash_spin = 10;
            if i != 0 {
                let (sx, sy) = pos_on(course, self.racers[i].dist, self.racers[i].lane);
                self.racers[i].cabbage_splat = 4;
                self.racers[i].splat_x = sx as i32;
                self.racers[i].splat_y = sy as i32;
            }
            let weight = if pace >= 8.0 {
                Weight::Kill
            } else if pace >= 5.0 {
                Weight::Crushing
            } else {
                Weight::Solid
            };
            let (team, at) = (self.racers[i].team, [self.racers[i].dist, self.racers[i].lane]);
            self.show.put(Kind::Wreck, team, 255, (pace * 10.0) as u32, at, weight, now);
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
        let total = course.lap * LAPS as f32;
        let mut placed = self.racers.iter().filter(|o| o.place.is_some()).count() as u8;
        let mut crossed: Vec<usize> = Vec::new();
        for (index, r) in self.racers.iter_mut().enumerate() {
            if r.place.is_some() {
                continue;
            }
            r.crash_spin = r.crash_spin.saturating_sub(1);
            r.boost_flame = r.boost_flame.saturating_sub(1);
            r.lap_flash = r.lap_flash.saturating_sub(1);
            r.cabbage_splat = r.cabbage_splat.saturating_sub(1);
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
                let prev_dist = r.dist;
                r.dist += r.speed.max(0.0);
                let (before, after) = ((prev_dist / course.lap) as u32, (r.dist / course.lap) as u32);
                if after > before && after < LAPS {
                    r.lap_flash = 6;
                    let ticks = now.saturating_sub(r.lap_at);
                    r.lap_at = now;
                    r.best = if r.best == 0 { ticks } else { r.best.min(ticks) };
                    let (team, at) = (r.team, [r.dist, r.lane]);
                    self.show.put(Kind::Lap, team, 0, after, at, Weight::Graze, now);
                    if self.show.best_lap == 0 || ticks < self.show.best_lap {
                        if self.show.best_lap != 0 {
                            self.show.put(Kind::Fastest, team, 0, ticks * 10 / HZ, at, Weight::Graze, now);
                        }
                        self.show.best_lap = ticks;
                    }
                    if after + 1 == LAPS {
                        self.show.put(Kind::Final, team, 0, 0, at, Weight::Graze, now);
                    }
                }
                if bend <= CORNER_BEND && !r.whipped {
                    r.wind = (r.wind + 0.0005 * f32::from(r.blood.stamina)).min(1.0);
                }
            }
            if r.dist >= total {
                let over = r.dist - total;
                r.line = now as f32 - over / r.speed.max(0.5);
                r.dist = total;
                let ticks = now.saturating_sub(r.lap_at);
                r.best = if r.best == 0 { ticks } else { r.best.min(ticks) };
                if self.show.best_lap != 0 && ticks < self.show.best_lap {
                    let (team, at) = (r.team, [r.dist, r.lane]);
                    self.show.put(Kind::Fastest, team, 0, ticks * 10 / HZ, at, Weight::Graze, now);
                    self.show.best_lap = ticks;
                }
                crossed.push(index);
            }
        }
        crossed.sort_by(|&a, &b| self.racers[a].line.total_cmp(&self.racers[b].line));
        for index in crossed {
            placed += 1;
            let place = placed;
            self.racers[index].place = Some(place);
            events.push(Event::Finished(index as u8, place));
            let (team, at) = (self.racers[index].team, [self.racers[index].dist, self.racers[index].lane]);
            let weight = if place == 1 { Weight::Finale } else { Weight::Graze };
            self.show.put(Kind::Finish, team, 0, u32::from(place), at, weight, now);
            if place == 2 {
                if let Some(first) = self.racers.iter().find(|r| r.place == Some(1)) {
                    let margin = self.racers[index].line - first.line;
                    if margin < show::PHOTO {
                        let hundredths = (margin.max(0.0) * 100.0 / HZ as f32) as u32;
                        let first_team = first.team;
                        self.show.put(Kind::Photo, first_team, team, hundredths, at, Weight::Kill, now);
                    }
                }
            }
        }
        let order = self.order();
        let leader = order[0];
        if self.racers[leader].place.is_none() && self.racers[leader].wrecked == 0 {
            self.racers[leader].boost_flame = 2;
        }
        let before = self.show.leader;
        self.show.watch(&self.racers, now);
        if self.show.leader != before && self.show.latest(Kind::Lead, now) == Some(0) {
            if let Some(i) = self.racers.iter().position(|r| Some(r.team) == self.show.leader) {
                events.push(Event::Lead(i as u8));
            }
        }
        for i in 0..count {
            if self.racers[i].lap(course) + 1 == LAPS && self.racers[i].dist.rem_euclid(course.lap) < 8.0 && self.t > HZ {
                events.push(Event::FinalLap(i as u8));
            }
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

const LOSER_CORNER: f32 = 0.82;
const LOSER_STRAIGHT: f32 = 0.9;
const WINNER: f32 = 0.97;
const BARGE_GAP: f32 = 22.0;
const BARGE_LANES: f32 = 0.34;

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

#[cfg(any(test, feature = "testing"))]
pub fn track_at(racer: &Racer) -> (f32, f32) {
    track_at_dist(racer.dist, racer.lane)
}

pub fn track_at_dist(dist: f32, lane: f32) -> (f32, f32) {
    let a = (dist.rem_euclid(LAP) / LAP) * std::f32::consts::TAU;
    let out = lane * 1.1;
    (
        12.0 + (9.0 + out) * a.cos(),
        7.0 + (4.4 + out * 0.5) * a.sin(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    East,
    West,
    North,
    South,
}

#[cfg(any(test, feature = "testing"))]
pub fn heading(racer: &Racer) -> Heading {
    heading_at(racer.dist)
}

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
                    self.sounds.push("gold");
                    self.crown_the_charioteer(&race);
                }
                Event::Lost => {
                    self.cues.push("chariot_lost".into());
                    self.crown_the_charioteer(&race);
                }
                Event::Finished(0, place) if race.driver != 0 => {
                    let name = race.horse.replace(' ', "_");
                    let place = place.min(4);
                    *self
                        .marks
                        .entry(format!("chariot:{place}:{name}"))
                        .or_default() += 1;
                }
                _ => {}
            }
        }
        let seated: Vec<u8> = race.seats().iter().filter_map(|&(_, i)| race.racers.get(i)).map(|r| r.team).collect();
        let held = race.show.held(race.clock, |f| {
            matches!(f.kind, Kind::Finish | Kind::Photo) || seated.contains(&f.team) || seated.contains(&f.other)
        });
        if let Some(weight) = held {
            self.hold_for(weight);
        }
        self.finish_hit_hold();
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

pub fn settle_chariot(
    stable: &mut Stable,
    marks: &BTreeMap<String, u32>,
) -> (Vec<(String, String)>, u32) {
    let mut said = Vec::new();
    let mut gold = 0;
    for (mark, &times) in marks {
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
