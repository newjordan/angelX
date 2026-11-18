//! The three-day clock. A loop runs from dawn of the first day to the dawn
//! the moon falls on: seventy-two hours, `day` ticks to each twenty-four
//! (the `moonfall_day` number). Hours are labelled as a town reads them:
//! the loop begins at 06:00 and the Third Midnight is the 66th hour.
use serde::{Deserialize, Serialize};

pub const DAYS: u32 = 3;
pub const SPAN: u32 = 24 * DAYS;
pub const FINAL: u32 = 66;

pub fn end(day: u32) -> u32 {
    day * DAYS
}

pub fn ticks(day: u32, h2: u32) -> u32 {
    ((u64::from(h2) * u64::from(day) + 24) / 48) as u32
}

pub fn half(clock: u32, day: u32) -> u32 {
    (u64::from(clock) * 48 / u64::from(day.max(1))) as u32
}

pub fn final_start(day: u32) -> u32 {
    ticks(day, FINAL * 2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

pub fn stamp(clock: u32, day: u32) -> Stamp {
    let minutes = (u64::from(clock) * 1440 / u64::from(day.max(1))) as u32;
    let since_dawn = minutes + 6 * 60;
    Stamp {
        day: (minutes / 1440 + 1).min(DAYS),
        hour: (since_dawn / 60) % 24,
        minute: since_dawn % 60,
    }
}

impl Stamp {
    pub fn text(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

pub fn night(clock: u32, day: u32) -> bool {
    let h = stamp(clock, day).hour;
    !(6..18).contains(&h)
}

pub fn bridge_up(clock: u32, day: u32) -> bool {
    let h = stamp(clock, day).hour;
    !(6..20).contains(&h)
}

pub fn remaining_minutes(clock: u32, day: u32) -> u32 {
    (u64::from(end(day).saturating_sub(clock)) * 1440 / u64::from(day.max(1))) as u32
}

pub fn next_dusk(clock: u32, day: u32) -> u32 {
    next_mark(clock, day, 12)
}
pub fn next_dawn(clock: u32, day: u32) -> u32 {
    next_mark(clock, day, 24)
}
fn next_mark(clock: u32, day: u32, phase: u32) -> u32 {
    let mut hour = phase;
    loop {
        let t = ticks(day, hour * 2);
        if t > clock || hour >= SPAN + 24 {
            return t;
        }
        hour += 24;
    }
}

pub fn light(clock: u32, day: u32) -> u32 {
    let m = (u64::from(clock) * 1440 / u64::from(day.max(1))) as u32 + 360;
    let m = m % 1440;
    let d = if m > 720 { 1440 - m } else { m };
    (d * 16 / 720).min(16)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Skip {
    Dusk,
    Dawn,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loop_runs_from_the_first_dawn_to_the_fourth() {
        let d = 3000;
        assert_eq!(stamp(0, d), Stamp { day: 1, hour: 6, minute: 0 });
        assert_eq!(stamp(d / 2, d), Stamp { day: 1, hour: 18, minute: 0 });
        assert_eq!(stamp(d, d), Stamp { day: 2, hour: 6, minute: 0 });
        assert_eq!(stamp(final_start(d), d), Stamp { day: 3, hour: 0, minute: 0 });
        assert_eq!(end(d), 9000);
        assert_eq!(remaining_minutes(final_start(d), d), 6 * 60);
    }

    #[test]
    fn dusk_and_dawn_are_the_next_ones_after_now() {
        let d = 3000;
        assert_eq!(next_dusk(0, d), d / 2);
        assert_eq!(next_dusk(d / 2, d), d / 2 + d);
        assert_eq!(next_dawn(0, d), d);
        assert_eq!(next_dawn(d / 2, d), d);
        assert!(!night(0, d) && night(d / 2, d) && night(d - 1, d));
        assert!(!bridge_up(d / 2, d) && bridge_up(d * 5 / 8, d));
        assert!(light(d / 4, d) == 16 && light(d * 3 / 4, d) == 0);
    }
}
