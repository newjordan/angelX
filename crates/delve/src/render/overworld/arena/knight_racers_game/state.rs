//! What the screen needs to know about the race beyond one frame: who is where
//! (the order every instrument agrees on) and each seat's lap clock.
//!
//! The course accessors live here too. They are the one seam between the
//! screen and the rules: when a race carries its own course, only this file
//! changes.
use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

pub const GRID_TICKS: u32 = rules::race::COUNTDOWN;
pub const HZ: f32 = rules::race::HZ;

pub fn lap_len(race: &rules::Session) -> f32 { race.course().lap() }
pub fn laps(race: &rules::Session) -> u32 { race.laps() }
pub fn course_name(race: &rules::Session) -> &'static str { race.course().name() }
pub fn zone(race: &rules::Session, s: f32) -> &'static str { race.course().zone(s.rem_euclid(lap_len(race))).1 }
pub fn course_pose(race: &rules::Session, s: f32) -> (f32, f32, f32) { race.course().pose(s) }
pub fn course_len(race: &rules::Session) -> f32 { race.distance() }

pub fn race_time(ticks: u32) -> f32 { ticks.saturating_sub(GRID_TICKS) as f32 / HZ }

pub fn standings(race: &rules::Session) -> Vec<(u32, &rules::Car)> {
    race.order().into_iter().map(|id| (id, &race.cars[&id])).collect()
}

pub fn place_of(order: &[(u32, &rules::Car)], seat: u32) -> usize {
    order.iter().position(|(id, _)| *id == seat).map_or(1, |i| i + 1)
}

pub fn ordinal(n: usize) -> &'static str {
    match n { 1 => "ST", 2 => "ND", 3 => "RD", _ => "TH" }
}

pub fn across(race: &rules::Session, n: usize) -> (Option<(u32, f32)>, Option<(u32, f32)>) {
    let mut times: Vec<(u32, f32)> = race.cars.iter().filter_map(|(id, c)| c.marks.get(n.wrapping_sub(1)).map(|t| (*id, *t))).collect();
    times.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    (times.first().copied(), times.get(1).copied())
}

pub fn course_short(race: &rules::Session) -> &'static str { race.course.short() }

pub fn stamp(t: f32) -> String {
    if !t.is_finite() { return "--:--.--".into(); }
    let t = t.max(0.);
    let cs = (t * 100.).round() as u32;
    format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

pub fn lap_of(race: &rules::Session, car: &rules::Car) -> (u32, f32) {
    let len = lap_len(race);
    let d = car.d.max(0.);
    let n = ((d / len).floor() as u32).min(laps(race).saturating_sub(1));
    (n, d - n as f32 * len)
}

#[derive(Clone, Default, Debug)]
pub struct SeatClock {
    pub starts: Vec<f32>,
    pub times: Vec<f32>,
    last_d: f32,
    reversing: u32,
    reversed_at: Option<u32>,
    against: u32,
    along: u32,
    facing_back: bool,
}

const WRONG_ENTER: f32 = 1.75;
const WRONG_EXIT: f32 = 1.2;
const WRONG_HOLD: u32 = 8;

pub fn heading_off(car: &rules::Car) -> f32 {
    let a = car.yaw - car.vy * 0.012;
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl SeatClock {
    pub fn best(&self) -> Option<f32> { self.times.iter().copied().filter(|t| t.is_finite()).min_by(f32::total_cmp) }
    pub fn last(&self) -> Option<f32> { self.times.last().copied().filter(|t| t.is_finite()) }
    pub fn lap_time(&self, now: f32) -> f32 { now - self.starts.last().copied().unwrap_or(0.) }
}

#[derive(Clone, Default)]
pub struct Clocks {
    ticks: u32,
    pub seats: BTreeMap<u32, SeatClock>,
}

thread_local! { static CLOCKS: RefCell<Clocks> = RefCell::new(Clocks::default()); }

impl Clocks {
    fn update(&mut self, race: &rules::Session) {
        if race.ticks < self.ticks || race.ticks <= GRID_TICKS { *self = Clocks::default(); }
        let advanced = race.ticks != self.ticks;
        self.ticks = race.ticks;
        let (len, count, now) = (lap_len(race), laps(race), race_time(race.ticks));
        for (&id, car) in &race.cars {
            let sc = self.seats.entry(id).or_insert_with(|| SeatClock { starts: vec![0.], ..SeatClock::default() });
            if advanced {
                if car.d + 0.02 < sc.last_d && car.finish.is_none() {
                    sc.reversing += 1;
                    if sc.reversing >= 4 { sc.reversed_at = Some(race.ticks); }
                } else {
                    sc.reversing = 0;
                }
            }
            sc.last_d = car.d;
            if advanced && car.finish.is_none() {
                let off = heading_off(car).abs();
                if off > WRONG_ENTER { sc.against += 1; sc.along = 0; }
                else if off < WRONG_EXIT { sc.along += 1; sc.against = 0; }
                else { sc.against = 0; sc.along = 0; }
                if sc.against >= WRONG_HOLD { sc.facing_back = true; }
                if sc.along >= WRONG_HOLD { sc.facing_back = false; }
            }
            let reached = if car.finish.is_some() { count } else { ((car.d.max(0.) / len).floor() as u32).min(count) };
            while (sc.times.len() as u32) < reached {
                let j = sc.times.len() as u32 + 1;
                let begin = sc.starts.last().copied().unwrap_or(0.);
                let at = if j < reached { f32::NAN }
                    else if j == count && car.finish.is_some() { car.finish.unwrap_or(now) }
                    else { now - (car.d - j as f32 * len).max(0.) / car.v.max(1.) };
                sc.times.push((at - begin).max(0.));
                if j < count { sc.starts.push(at); }
            }
        }
    }
}

pub fn observe(race: &rules::Session) -> Clocks {
    CLOCKS.with(|c| {
        let mut c = c.borrow_mut();
        c.update(race);
        c.clone()
    })
}

pub fn wrong_way(clocks: &Clocks, race: &rules::Session, seat: u32) -> bool {
    clocks.seats.get(&seat).is_some_and(|c| c.facing_back || c.reversed_at.is_some_and(|t| race.ticks.saturating_sub(t) < 24))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> rules::Session {
        let mut run = crate::rules::Run::roundtable_for_test("knight_racers").unwrap();
        let mut race = run.knight_racers.take().unwrap();
        race.join_ids(1..=6);
        race
    }

    #[test]
    fn standings_put_finishers_first_and_ties_to_the_lower_seat() {
        let mut race = session();
        race.cars.get_mut(&3).unwrap().d = 900.;
        race.cars.get_mut(&5).unwrap().d = 900.;
        race.cars.get_mut(&6).unwrap().finish = Some(80.);
        race.cars.get_mut(&2).unwrap().finish = Some(70.);
        let order: Vec<u32> = standings(&race).into_iter().map(|(id, _)| id).collect();
        assert_eq!(order[..4], [2, 6, 3, 5]);
    }

    #[test]
    fn lap_clock_times_each_lap_from_where_the_line_is_crossed() {
        let mut race = session();
        race.ticks = 0;
        observe(&race);
        let len = lap_len(&race);
        let mut t = 91;
        let mut d = 0.;
        while d < len * 1.5 {
            race.ticks = t;
            let car = race.cars.get_mut(&1).unwrap();
            car.d = d; car.v = 40.;
            observe(&race);
            d += 40. / 30.;
            t += 1;
        }
        let clocks = observe(&race);
        let seat = &clocks.seats[&1];
        assert_eq!(seat.starts.len(), 2, "a second lap has begun");
        let crossed = seat.starts[1];
        let truth = len / 40.;
        assert!((crossed - truth).abs() < 0.05, "line crossed at {crossed}, true {truth}");
        assert!(clocks.seats[&1].times.len() == 1);
        race.ticks = 3;
        race.cars.get_mut(&1).unwrap().d = -6.;
        let clocks = observe(&race);
        assert!(clocks.seats[&1].times.is_empty());
    }

    #[test]
    fn going_backwards_raises_the_wrong_way_flag_and_it_clears() {
        let mut race = session();
        race.ticks = 100;
        observe(&race);
        for k in 0..8u32 {
            race.ticks = 101 + k;
            let car = race.cars.get_mut(&1).unwrap();
            car.d = 400. - k as f32 * 1.0;
            observe(&race);
        }
        let clocks = observe(&race);
        assert!(wrong_way(&clocks, &race, 1));
        race.ticks += 40;
        assert!(!wrong_way(&clocks, &race, 1));
    }

    fn drive(steer: i8, gas: bool, brake: bool) -> BTreeMap<u32, rules::Drive> {
        [(1, rules::Drive { steer, gas, brake, ..Default::default() })].into()
    }

    fn tick(race: &mut rules::Session, steer: i8, gas: bool, brake: bool) -> bool {
        race.step_drive(&drive(steer, gas, brake));
        let clocks = observe(race);
        wrong_way(&clocks, race, 1)
    }

    #[test]
    fn a_pod_turned_about_is_flagged_after_a_hold_and_cleared_when_it_faces_the_road_again() {
        let mut race = session();
        race.cars.retain(|id, _| *id == 1);
        race.ticks = 100;
        { let c = race.cars.get_mut(&1).unwrap(); c.d = 400.; c.v = 0.; c.y = 0.; }
        observe(&race);
        for _ in 0..30 { assert!(!tick(&mut race, 0, true, false)); }
        let mut flagged_at = None;
        for n in 0..260 {
            let on = tick(&mut race, 1, false, true);
            let yaw = race.cars[&1].yaw;
            if on && flagged_at.is_none() { flagged_at = Some((n, yaw)); }
            if yaw.abs() > 3.0 { break; }
        }
        let (n, yaw) = flagged_at.expect("a pod turned right round is going the wrong way");
        assert!(yaw.abs() > 1.75, "flagged only once the nose is well against the road: yaw {yaw} at {n}");
        assert!(race.cars[&1].yaw.abs() > 3.0, "the pod turned right about");
        for _ in 0..40 { assert!(tick(&mut race, 0, true, false)); }
        assert!(race.cars[&1].d < 400., "gas takes a pod facing back down the road backwards");
        let mut cleared = false;
        for _ in 0..400 {
            let on = tick(&mut race, 1, false, true);
            if !on { cleared = true; assert!(race.cars[&1].yaw.abs() < 1.3, "cleared with the nose at {}", race.cars[&1].yaw); break; }
        }
        assert!(cleared, "turning back clears the flag");
        assert!(race.valid());
    }

    #[test]
    fn a_nose_near_the_edge_of_the_band_does_not_flicker() {
        let mut race = session();
        race.cars.retain(|id, _| *id == 1);
        race.ticks = 100;
        observe(&race);
        for k in 0..60u32 {
            race.ticks = 101 + k;
            race.cars.get_mut(&1).unwrap().yaw = if k % 2 == 0 { 1.5 } else { -1.5 };
            let clocks = observe(&race);
            assert!(!wrong_way(&clocks, &race, 1), "1.5 rad is not wrong way");
        }
        for k in 0..5u32 {
            race.ticks = 200 + k;
            race.cars.get_mut(&1).unwrap().yaw = 2.5;
            let clocks = observe(&race);
            assert!(!wrong_way(&clocks, &race, 1), "a moment's glance back is not the wrong way");
        }
        for k in 0..12u32 {
            race.ticks = 210 + k;
            race.cars.get_mut(&1).unwrap().yaw = 2.5;
            observe(&race);
        }
        let clocks = observe(&race);
        assert!(wrong_way(&clocks, &race, 1));
        for k in 0..30u32 {
            race.ticks = 230 + k;
            race.cars.get_mut(&1).unwrap().yaw = 1.5;
            let clocks = observe(&race);
            assert!(wrong_way(&clocks, &race, 1), "1.5 rad keeps the flag once up");
        }
    }

    #[test]
    fn stamps_read_minutes_seconds_hundredths() {
        assert_eq!(stamp(0.), "0:00.00");
        assert_eq!(stamp(61.234), "1:01.23");
        assert_eq!(stamp(599.999), "10:00.00");
    }
}
