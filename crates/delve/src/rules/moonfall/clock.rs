//! The three-day clock. A loop runs from dawn of the first day to the dawn
//! the moon falls on: seventy-two hours, `day` ticks to each twenty-four
//! (the `moonfall_day` number). Hours are labelled as a town reads them:
//! the loop begins at 06:00 and the Third Midnight is the 66th hour.
use serde::{Deserialize, Serialize};

pub const DAYS: u32 = 3;
/// Hours in a loop.
pub const SPAN: u32 = 24 * DAYS;
/// The hour the Final Hours begin: the Third Midnight (hours since dawn).
pub const FINAL: u32 = 66;

/// Ticks in a loop.
pub fn end(day: u32) -> u32 {
    day * DAYS
}

/// The tick `h2` half-hours after the first dawn.
pub fn ticks(day: u32, h2: u32) -> u32 {
    // To the nearest tick, so 06:30 reads 06:30.
    ((u64::from(h2) * u64::from(day) + 24) / 48) as u32
}

/// Half-hours since the first dawn.
pub fn half(clock: u32, day: u32) -> u32 {
    (u64::from(clock) * 48 / u64::from(day.max(1))) as u32
}

/// The tick the Final Hours begin.
pub fn final_start(day: u32) -> u32 {
    ticks(day, FINAL * 2)
}

/// A time as the town reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// 1 to 3: dawn to dawn.
    pub day: u32,
    /// 0 to 23.
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

/// Whether the lamps burn: from dusk (18:00) to dawn (06:00).
pub fn night(clock: u32, day: u32) -> bool {
    let h = stamp(clock, day).hour;
    !(6..18).contains(&h)
}

/// The bridge is raised from 20:00 to 06:00.
pub fn bridge_up(clock: u32, day: u32) -> bool {
    let h = stamp(clock, day).hour;
    !(6..20).contains(&h)
}

/// Hours left before the moon falls, in thousandths of the loop's end
/// (the Final Hours countdown reads it as hours and minutes).
pub fn remaining_minutes(clock: u32, day: u32) -> u32 {
    (u64::from(end(day).saturating_sub(clock)) * 1440 / u64::from(day.max(1))) as u32
}

/// The next dusk (18:00) or dawn (06:00) strictly after `clock`.
pub fn next_dusk(clock: u32, day: u32) -> u32 {
    next_mark(clock, day, 12)
}
pub fn next_dawn(clock: u32, day: u32) -> u32 {
    next_mark(clock, day, 24)
}
/// The next tick at hour `phase` (hours since dawn, mod 24) after `clock`.
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

/// What the stage and the rules both need to say about the sky: 0 at
/// midnight, 1 at noon, in sixteenths, drawn smoothly.
pub fn light(clock: u32, day: u32) -> u32 {
    let m = (u64::from(clock) * 1440 / u64::from(day.max(1))) as u32 + 360;
    // Minutes into the calendar day; noon is 720.
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
