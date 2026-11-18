//! The things that stand on the rail, as the pictures draw them; whether a
//! craft has flown through one is the module's (`cockpit/live/griffinwing-rules`).
use super::*;

pub const SHUTTER_PERIOD: u32 = 150;
pub const SHUTTER_MIN: i32 = dm(5);

pub fn shutter_gap(p: &Prop, t: u32, wx: i32) -> i32 {
    let k = tri(t + u32::from(p.phase), SHUTTER_PERIOD, 1000) + 1000;
    SHUTTER_MIN + (wx + dm(10) - SHUTTER_MIN) * k / 2000
}
