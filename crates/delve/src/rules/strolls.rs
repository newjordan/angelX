//! The village on its feet: where each named soul of a holding is in a
//! steward's hall, and what they are doing, at a tick.
//!
//! Everyone drawn walking about the hall is somebody in the holding's
//! ledger (`villagers`): the roster says who, their trade says where they
//! work (their room's bay, the room going up, the walls, the table), their
//! age says whether they play or keep the fireside, and the holding's
//! occasion (`occasions`) and any seized workshop (`fey`) say when they
//! all stop for something else. Like the barony's folk (`folk`), where
//! anyone is is a pure function of the state and the tick: nothing about
//! it is saved or sent, a friend's mirror shows the same village, and a
//! test can ask where everyone is at any moment.
//!
//! Places are the hall's tiles (`holdings::bay`, `TABLE`); a figure's feet.

use super::fey;
use super::holdings::{self, Holding, SLOTS};
use super::occasions::{Broke, Occasion};
use super::villagers::{mix, seed_of, Trade, Villager, COMING_OF_AGE, ELDER};
use super::HZ;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Walk,
    Work,
    Idle,
    Seated,
    Play,
    Rage,
    Bear,
    Mourn,
    Toast,
    Vigil,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroller {
    pub who: usize,
    pub x: f32,
    pub y: f32,
    pub east: bool,
    pub pose: Pose,
    pub laden: bool,
}

pub const DAY: u64 = 40 * HZ as u64;
pub const TABLE_AT: (f32, f32) = (12.0, 5.5);
const YARD: (f32, f32) = (12.0, 8.6);
const POINTS: [(f32, f32); 5] = [(9.2, 6.6), (14.8, 6.6), (12.0, 8.4), (6.8, 7.0), (17.2, 7.0)];
const SEATS: [(f32, f32); 12] = [
    (9.8, 4.6),
    (14.2, 4.6),
    (9.8, 6.4),
    (14.2, 6.4),
    (10.9, 3.9),
    (13.1, 3.9),
    (10.9, 7.1),
    (13.1, 7.1),
    (8.6, 5.5),
    (15.4, 5.5),
    (11.0, 8.2),
    (13.0, 8.2),
];
const FIRESIDE: [(f32, f32); 4] = [(10.2, 6.2), (13.8, 6.2), (9.2, 5.0), (14.8, 5.0)];
pub const PROCESSION: ((f32, f32), (f32, f32)) = ((4.0, 8.4), (20.0, 8.4));
const CROSSING: u64 = 14 * HZ as u64;

pub fn station(held: &Holding, f: &Villager) -> (f32, f32) {
    match f.trade {
        t if fey::is_craft(t) => match fey::workshop(held, t) {
            Some(bay) => work_spot(bay),
            None => YARD,
        },
        Trade::Labourer => match held.rising() {
            Some(bay) => work_spot(bay),
            None => YARD,
        },
        _ => YARD,
    }
}

pub fn work_spot(bay: usize) -> (f32, f32) {
    let (c, r) = holdings::BAYS[bay.min(SLOTS - 1)];
    (c as f32 + 1.5, if r < 5 { 3.9 } else { 11.0 })
}

fn lerp(a: (f32, f32), b: (f32, f32), k: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k)
}

fn adult(f: &Villager) -> bool {
    f.trade != Trade::Child && f.age >= COMING_OF_AGE
}

pub fn bier(tick: u64) -> ((f32, f32), bool) {
    let leg = tick / CROSSING;
    let k = (tick % CROSSING) as f32 / CROSSING as f32;
    let (a, b) = PROCESSION;
    if leg.is_multiple_of(2) { (lerp(a, b, k), true) } else { (lerp(b, a, k), false) }
}

pub fn strollers(held: &Holding, tick: u64) -> Vec<Stroller> {
    let mut out = Vec::with_capacity(held.folk.len());
    let mourning = matches!(held.occasion, Some(Occasion::Mourning { .. }));
    let mut cortege = 0usize;
    let mut seat = 0usize;
    let mut fire = 0usize;
    for (i, f) in held.folk.iter().enumerate() {
        let h = mix(seed_of(&f.name), 3 + i as u32);
        let t = (tick + h % DAY) % DAY;
        let p = t as f32 / DAY as f32;
        let at = |(x, y): (f32, f32), east: bool, pose: Pose, laden: bool| Stroller { who: i, x, y, east, pose, laden };
        let station = station(held, f);
        let point = POINTS[(h >> 8) as usize % POINTS.len()];
        let s = match &held.occasion {
            Some(Occasion::Feast) => {
                let (x, y) = if adult(f) || f.age >= ELDER {
                    let s = SEATS[seat % SEATS.len()];
                    seat += 1;
                    s
                } else {
                    let a = (tick as f32 / 40.0) + h as f32;
                    (TABLE_AT.0 + a.cos() * 4.4, TABLE_AT.1 + 1.4 + a.sin() * 2.3)
                };
                let pose = if f.trade == Trade::Child { Pose::Play } else if (p * 4.0 + i as f32).fract() < 0.35 { Pose::Toast } else { Pose::Seated };
                at((x, y), x < TABLE_AT.0, pose, false)
            }
            Some(Occasion::Mourning { who, .. }) if mourning && adult(f) && f.age < ELDER && f.name != *who => {
                let ((bx, by), east) = bier(tick);
                let k = cortege;
                cortege += 1;
                if k < 2 {
                    let off = if k == 0 { -0.9 } else { 0.9 };
                    at((bx + if east { off } else { -off }, by), east, Pose::Bear, false)
                } else {
                    let back = 1.3 * (k - 1) as f32;
                    at((bx + if east { -back - 1.0 } else { back + 1.0 }, by + 0.2 * (k % 2) as f32), east, Pose::Mourn, false)
                }
            }
            Some(Occasion::Tantrum { who, broke, .. }) if f.name == *who => {
                let spot = match broke {
                    Broke::Course(bay) => work_spot(usize::from(*bay)),
                    _ => station,
                };
                let jitter = ((tick / 3) % 5) as f32 * 0.06 - 0.12;
                at((spot.0 + jitter, spot.1), (tick / 11) % 2 == 0, Pose::Rage, false)
            }
            _ => match f.trade {
                _ if f.trade == Trade::Child || f.age < COMING_OF_AGE => {
                    let a = (tick as f32 / 55.0) + h as f32;
                    let (x, y) = (TABLE_AT.0 + a.cos() * (2.6 + (h % 3) as f32 * 0.5), TABLE_AT.1 + 1.6 + a.sin() * 1.5);
                    at((x, y), a.sin() > 0.0, if (t / 20) % 3 == 0 { Pose::Idle } else { Pose::Play }, false)
                }
                _ if f.age >= ELDER => {
                    let s = FIRESIDE[fire % FIRESIDE.len()];
                    fire += 1;
                    at(s, s.0 > TABLE_AT.0, Pose::Seated, false)
                }
                Trade::Guard => {
                    let (a, b) = ((3.6, 8.4), (20.4, 8.4));
                    let span = DAY / 2;
                    let lap = (tick + h % span) % (span * 2);
                    let half = lap % span;
                    let walk = span * 7 / 10;
                    let k = (half.min(walk) as f32) / walk as f32;
                    let east = lap < span;
                    let (pos, moving) = if east { (lerp(a, b, k), half < walk) } else { (lerp(b, a, k), half < walk) };
                    at(pos, east, if moving { Pose::Walk } else { Pose::Idle }, false)
                }
                Trade::Labourer => match held.rising() {
                    Some(_) => {
                        if p < 0.15 {
                            at(YARD, true, Pose::Idle, false)
                        } else if p < 0.40 {
                            at(lerp(YARD, station, (p - 0.15) / 0.25), station.0 > YARD.0, Pose::Walk, true)
                        } else if p < 0.70 {
                            at(station, true, Pose::Work, true)
                        } else if p < 0.95 {
                            at(lerp(station, YARD, (p - 0.70) / 0.25), YARD.0 > station.0, Pose::Walk, false)
                        } else {
                            at(YARD, true, Pose::Idle, false)
                        }
                    }
                    None => stroll(i, station, point, p),
                },
                _ => {
                    let seized = held.seized.as_ref().is_some_and(|s| s.who == f.name && s.trade == f.trade);
                    let proud = held.works.iter().any(|m| m.maker == f.name && m.trade == f.trade);
                    if seized {
                        at(station, (tick / 17) % 2 == 0, Pose::Work, false)
                    } else if proud {
                        at(station, station.0 < TABLE_AT.0, Pose::Vigil, false)
                    } else {
                        stroll(i, station, point, p)
                    }
                }
            },
        };
        let watching = matches!(&held.occasion, Some(Occasion::Tantrum { who, .. }) if *who != f.name);
        let s = if watching && s.pose == Pose::Work { Stroller { pose: Pose::Idle, ..s } } else { s };
        out.push(s);
    }
    out
}

fn stroll(who: usize, station: (f32, f32), point: (f32, f32), p: f32) -> Stroller {
    let at = |(x, y): (f32, f32), east: bool, pose: Pose| Stroller { who, x, y, east, pose, laden: false };
    if p < 0.55 {
        at(station, station.0 < TABLE_AT.0, Pose::Work)
    } else if p < 0.65 {
        at(lerp(station, point, (p - 0.55) / 0.10), point.0 > station.0, Pose::Walk)
    } else if p < 0.80 {
        at(point, point.0 < TABLE_AT.0, Pose::Idle)
    } else if p < 0.90 {
        at(lerp(point, station, (p - 0.80) / 0.10), station.0 > point.0, Pose::Walk)
    } else {
        at(station, station.0 < TABLE_AT.0, Pose::Work)
    }
}

pub const SPEAK_REACH: f32 = 3.2;

pub fn near(held: &Holding, tick: u64, x: f32, y: f32) -> Option<usize> {
    strollers(held, tick)
        .into_iter()
        .map(|s| (s.who, (x - s.x * super::TILE_UNITS).hypot(y - s.y * super::TILE_UNITS)))
        .filter(|&(_, d)| d < SPEAK_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(who, _)| who)
}
